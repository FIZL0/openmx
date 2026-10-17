//! The rider's own animation state machine: which clip the *body* asks for, as opposed to which clip
//! the bike asks for.
//!
//! [`crate::vehicle_anim`] ports the step that turns an animation state into a call on the motion
//! library. This is the function in front of it — the one that decides the state. The two together are
//! one vtable slot of one actor, and that actor is the **rider's body**: the spawn that allocates it
//! hands it the prefix `rider_` (or `ff_rider_`, which is the disc's other mode), the clip loader joins
//! that prefix to the twenty-four gesture names and looks the other thirty-six up bare, and those
//! thirty-six bare names are exactly the clips in a rider's own motion file. So
//! [`crate::vehicle_anim::POSE_CLIPS`] is a list of the rider's clips, not the bike's, and this module
//! is what asks for the ones nothing else in the port ever could: the get-ready family at the gate,
//! the wreck family when the body comes off, the reverse pair, and the look-around idles.
//!
//! ## The selection, in order
//!
//! [`Body::select`] is one call of the original's selection. It is a chain of `else if`s over the
//! request block the body is pointed at, and the first arm that matches is the only one that runs:
//!
//! 1. **the gate** — the request's cut flag is set, which is the start line. A sequence of its own,
//!    below.
//! 2. **the wreck** — the request says the body is off the bike. Still falling and it flails
//!    ([`vehicle_anim::HOLD`]); once the body has settled it grabs one of five limbs, drawn at random,
//!    and keeps the one it drew.
//! 3. **a gesture** — the request names one of the twenty-four gesture slots. That slot, unless the
//!    request also says no-hander, which has a clip of its own.
//! 4. **no gesture** — the airborne pose, the backflip, the reverse pair, or the idle-and-look-around
//!    that is everything else. That last one is the only part of this machine with a clock: standing
//!    still with the bars centred and the throttle shut, the body waits between fifteen and
//!    forty-five seconds and then plays one of ten idles, and goes back to riding when the idle ends.
//!
//! Then, every frame regardless of which arm ran: moving faster than the pose grid's first threshold
//! cancels the look timer, the step runs ([`vehicle_anim::Driver::frame`]), and the two fields the step
//! reads but never writes are latched from the request — which is where S25's unanswered question about
//! the writer of the step's sequence byte is answered.
//!
//! ## The gate
//!
//! The start-line sequence is four states of its own, kept in two fields and read off whether the clip
//! that is playing has run out:
//!
//! ```text
//!   nothing drawn yet      -> one of ten get-ready idles, drawn without repeating (`Pool`)
//!   that idle has ended    -> get_ready_breathe, and a wait of 3.5s less the request's own clock
//!   the wait has run out   -> one of three start poses
//!   the throttle is opened -> the cut flag is cleared and the body rides
//! ```
//!
//! ## Chance
//!
//! Four of the arms draw a random number, and every one of them sits behind the same lazily-installed
//! seed: the first draw anywhere in the program reads the console's clock and seeds the C library's
//! generator with it. That is a thing a simulation cannot do (see the crate's third and fourth rules),
//! so the port takes the clock as an input ([`Clock`]) and the generator and its no-repeat list as one
//! value ([`Pool`]) rather than as the three globals the original keeps them in.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::motion::{Clip, ClipId, Played, Player, Put, remap};
use crate::rng::Rng;
use crate::trick::SLOTS;
use crate::vehicle_anim::{
    self, AIR, AXIS_START, BACKFLIP, Frame, GET_READY, HOLD, Lag, Library, MIX_TAU, NO_HANDER,
    PHASE_TAU, Play, REVERSE_L, REVERSE_R, START_POS, TURN_RIDING,
};
use ps2_float::F;

// ---------------------------------------------------------------------------------------------------
// the states this machine, and only this machine, asks for
// ---------------------------------------------------------------------------------------------------

/// The first of the ten get-ready idles, which are the clips the gate opens with and the clips the
/// look-around timer fires.
pub const IDLE: i32 = GET_READY + 1;
/// How many of them there are: the length of the draw, and of [`Pool::used`].
pub const IDLES: i32 = 10;
/// The first of the five limbs a settled wreck grabs.
pub const HOLD_LIMB: i32 = HOLD + 1;
/// How many of those there are.
pub const HOLD_LIMBS: i32 = 5;
/// How many start poses the gate's last step draws from.
pub const START_POSES: i32 = 3;
/// How long the gate waits on `get_ready_breathe`, before the request's own clock and the ready rate
/// are taken off it.
const BREATHE: F = F(3.5);
/// The middle of each axis: the bars centred and the body not leaning, which is one of the four things
/// the look-around timer needs to be true.
const CENTRED: F = F(0.5);

// ---------------------------------------------------------------------------------------------------
// chance
// ---------------------------------------------------------------------------------------------------

/// The console's clock, as the first random draw of the program reads it.
///
/// The fields are four bytes of the CD drive's clock, which holds them in **binary-coded decimal** —
/// so the fifty-ninth second arrives here as `0x59`, which is eighty-nine. The port keeps them as the
/// bytes they are and leaves them to the caller: a replay hands over the numbers it recorded, and a
/// test hands over whatever it likes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Clock {
    pub second: u32,
    pub minute: u32,
    pub hour: u32,
    pub day: u32,
}

impl Clock {
    /// The seed itself. Note the hour's multiplier: it is three hundred and sixty, not three thousand
    /// six hundred, so two readings an hour apart can collide with two readings ten minutes apart.
    /// That is the original's arithmetic and the port keeps it.
    pub fn seed(self) -> u32 {
        self.second
            .wrapping_add(self.minute.wrapping_mul(60))
            .wrapping_add(self.hour.wrapping_mul(360))
            .wrapping_add(self.day.wrapping_mul(86_400))
    }
}

/// The three words this machine keeps outside the body: the generator every draw of it goes through,
/// whether anything has seeded it yet, and the no-repeat list the gate's ten idles are drawn from.
///
/// All three are globals in the original — the seed is shared with every other random draw in the
/// program, and so is the flag, which is why a body that is not the first thing to ask for a number
/// finds the generator already seeded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Pool {
    /// The idles drawn so far this round.
    pub used: [i32; IDLES as usize],
    /// How many of them are in [`Pool::used`]. Zero and ten both mean "start again".
    pub count: usize,
    /// The generator.
    pub rng: Rng,
    /// Something has seeded it. The original keeps the seed itself here and tests it against zero, so
    /// a clock that reads as exactly zero would be re-seeded on every draw.
    pub seeded: bool,
}

impl Pool {
    /// One draw of `count` values, seeding the generator off the clock first if nothing has.
    pub fn draw(&mut self, count: i32, clock: Clock) -> i32 {
        if !self.seeded {
            self.rng.seed(clock.seed());
            self.seeded = true;
        }
        self.rng.index(count)
    }

    /// A unit float out of the same generator, seeded the same way. Only the look timer's re-arm
    /// asks for one.
    pub fn unit(&mut self, clock: Clock) -> F {
        if !self.seeded {
            self.rng.seed(clock.seed());
            self.seeded = true;
        }
        self.rng.unit()
    }

    /// One of the ten get-ready idles, not one of the ones already drawn this round.
    ///
    /// An empty or a full list starts a new round of one. Otherwise it draws until the value is not in
    /// the list and then appends it — which is why the tenth draw of a round costs ten numbers on
    /// average and the first costs one.
    pub fn idle(&mut self, clock: Clock) -> i32 {
        if self.count == 0 || self.count == IDLES as usize {
            self.count = 1;
            let want = IDLE + self.draw(IDLES, clock);
            self.used[0] = want;
            return want;
        }
        loop {
            let want = IDLE + self.draw(IDLES, clock);
            if !self.used[..self.count].contains(&want) {
                self.used[self.count] = want;
                self.count += 1;
                return want;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// what the selection is handed
// ---------------------------------------------------------------------------------------------------

/// The body's own numbers that the selection reads and never writes.
///
/// Two more that it reads live on [`vehicle_anim::Params`], because the step reads them too: the pose
/// grid's first threshold, which doubles as the speed that cancels the look timer, and the ready rate,
/// which the gate takes off its own wait.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Params {
    /// Slower than this, either way, and the body counts as standing still. The disc's default is
    /// about one and a half, against the grid's first threshold of seven and a third — so the look
    /// timer only runs well inside the stopped row.
    pub stopped: F,
    /// The shortest wait between look-around idles, in seconds. Fifteen, as the actor is built.
    pub look_min: F,
    /// The longest. Forty-five.
    pub look_max: F,
}

impl Params {
    /// The three as the actor's constructor writes them, by their bit patterns, which is how the
    /// program image holds them: about one and a half, fifteen and forty-five.
    pub fn built() -> Params {
        Params {
            stopped: F(f32::from_bits(0x3fbb_bb84)),
            look_min: F(f32::from_bits(0x4170_0000)),
            look_max: F(f32::from_bits(0x4234_0000)),
        }
    }
}

/// What the rest of the rider asks of the body this frame: the flags and the two clocks of the request
/// block that only the selection reads.
///
/// The other half of the same block — where the body goes, which way the two axes want to move, the
/// number the pose grid cuts up, how far through a gesture it is, and the cut flag the gate reads and
/// clears — is [`vehicle_anim::Request`], which the step already models. Both halves are one struct in
/// the original; they are two here because the step was ported first and nothing of it needed to move.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ask {
    /// The wheels have left the ground.
    pub airborne: bool,
    /// A backflip is in progress, which outranks being airborne.
    pub backflip: bool,
    /// The gesture being held is a no-hander, which has a clip of its own instead of its slot's.
    pub no_hander: bool,
    /// The body is off the bike.
    pub wreck: bool,
    /// The gate's own clock, in seconds. The breathe-and-wait is three and a half less this, less the
    /// ready rate.
    pub countdown: F,
    /// The throttle. Non-zero at the gate ends the start sequence, and anything but shut stops the
    /// look-around timer.
    pub throttle: F,
    /// The body is going backwards. Non-zero picks the reverse pair, and which of the two it is comes
    /// off which side of centre the phase axis is.
    pub reversing: F,
    /// The gesture slot the request names, or [`SLOTS`] for none — which is the value that lets every
    /// arm past the gesture one run at all.
    pub gesture: i32,
}

// ---------------------------------------------------------------------------------------------------
// the machine
// ---------------------------------------------------------------------------------------------------

/// A rider's body: the state the selection keeps, the step's state it writes into, and the motion
/// object the body is.
///
/// [`Body::default`] is the actor's own constructor, not a field-by-field zero: the two that are not
/// zero there matter, and a body built with them wrong does not animate at all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    /// The step's state. The selection's whole output is [`vehicle_anim::Driver::want`]; everything
    /// else in here is the step's own.
    pub veh: vehicle_anim::Driver,
    /// The clip playing. The body is its own motion object in the original — the same pointer is the
    /// actor and the motion — so the clock and the finished flag are fields of it, and the selection
    /// reads that flag to find out whether the clip it asked for last has run out.
    pub player: Player,
    /// What is left of the gate's breathe-and-wait, in seconds. Counts down while it is positive.
    pub ready: F,
    /// The gate has drawn its idle. Set once and never cleared here, so the gate's first step happens
    /// once per body.
    pub picked: bool,
    /// What is left of the wait between look-around idles, in seconds. Re-armed from a random number
    /// between [`Params::look_min`] and [`Params::look_max`] whenever it is not positive.
    pub look: F,
    /// A look-around idle is playing, which is what stops the idle arm asking for the riding pose
    /// instead. Cleared when that idle ends.
    pub looking: bool,
    /// The wrecked body has come to rest. Written by the body's *mode* machine, which is not ported —
    /// the selection only reads it, to tell flailing from holding a limb.
    pub settled: bool,
}

/// What a frame of the body's animation left in its motion object that the frame itself does not
/// carry: the two answers [`Body::frame_clocked`] keeps and [`Body::frame`] throws away.
///
/// Both are the renderer's, and both are gone by the time the step has returned — which is why they
/// are carried out rather than read off the motion object afterwards:
///
/// * the clip-to-clip transition is wiped by the advance's own end-of-frame reset on the frame it
///   ends on, so a renderer that looked at the object would draw the new clip alone (V17);
/// * whether a put had nothing to fade from is a *return value* of
///   [`crate::motion::Player::play`], written nowhere.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Clocked {
    /// What the frame's advance did, including the transition that posed it
    /// ([`crate::motion::Played::fade`]).
    pub played: Played,
    /// The clip the step put on had nothing to fade out of, so whoever draws the body owes it a pose
    /// snapshot before the pose is written — the same debt [`Frame::restart`] carries.
    pub snapshot: bool,
}

impl Default for Body {
    /// The body as the actor's constructor leaves it. Four of its fields are not zero: the state asked
    /// for, the state running and the state before it are all the riding pose, the added-record
    /// index is negative, which is what "play the state machine rather than a record" is — a body
    /// built with a zero there plays record zero of an empty list, which is no clip at all — and the
    /// two eased axes are the two records the build allocates beside the object, centred and with
    /// their own time constants ([`vehicle_anim::MIX_TAU`]). A body built with zeroes there divides by
    /// a time constant of none and puts `-FLT_MAX` on both axes on its first frame, which is a pose
    /// grid stuck in one corner (V18 found it from the shell).
    fn default() -> Body {
        Body {
            veh: vehicle_anim::Driver {
                want: TURN_RIDING,
                state: TURN_RIDING,
                prev: TURN_RIDING,
                extra: -1,
                mix: Lag { value: AXIS_START, tau: MIX_TAU, ratio: F::ZERO },
                phase: Lag { value: AXIS_START, tau: PHASE_TAU, ratio: F::ZERO },
                ..Default::default()
            },
            player: Player::default(),
            ready: F::ZERO,
            picked: false,
            look: F::ZERO,
            looking: false,
            settled: false,
        }
    }
}

impl Body {
    /// Which state the body wants, written into [`vehicle_anim::Driver::want`].
    ///
    /// `r` is taken by reference and mutated for one reason: the gate's last step clears the request's
    /// own cut flag, which is the one thing in the original that ends the start-line sequence.
    pub fn select(
        &mut self,
        dt: F,
        p: &Params,
        vp: &vehicle_anim::Params,
        r: &mut vehicle_anim::Request,
        a: &Ask,
        pool: &mut Pool,
        clock: Clock,
    ) {
        let want = &mut self.veh.want;
        if r.cut {
            // 1 — the gate. Four steps in one chain, in the order they happen.
            if F::ZERO < self.ready {
                *want = GET_READY;
                self.ready = self.ready - dt;
            } else if !self.picked {
                *want = pool.idle(clock);
                self.picked = true;
            } else if !self.player.finished {
                if *want == GET_READY {
                    // The breathe has been asked for and its wait has run out: stand up.
                    *want = START_POS + pool.draw(START_POSES, clock);
                } else if a.throttle != F::ZERO {
                    // The only writer of the cut flag this side of the start line.
                    r.cut = false;
                    *want = TURN_RIDING;
                }
            } else {
                // Whatever was playing has ended. Breathe, for three and a half seconds less the
                // request's own clock and less the rate the group fades at.
                *want = GET_READY;
                self.ready = (BREATHE - a.countdown) - vp.rate_ready;
            }
        } else if a.wreck {
            // 2 — off the bike. Flailing, then one limb held for as long as it lasts.
            if !self.settled {
                *want = HOLD;
            } else if *want < HOLD_LIMB {
                *want = HOLD_LIMB + pool.draw(HOLD_LIMBS, clock);
            }
        } else if a.gesture != SLOTS as i32 {
            // 3 — a gesture. Its own slot, or the one clip that replaces it.
            *want = if a.no_hander { NO_HANDER } else { a.gesture };
        } else if a.backflip {
            *want = BACKFLIP;
        } else if a.airborne {
            *want = AIR;
        } else if a.reversing != F::ZERO {
            // Which side of centre the bars are on picks which of the pair.
            *want = if r.phase_axis < CENTRED { REVERSE_L } else { REVERSE_R };
        } else {
            // 4 — riding, or standing about. The clock is the only one in the machine.
            if r.band.abs() < p.stopped
                && r.phase_axis == CENTRED
                && r.mix_axis == CENTRED
                && a.throttle == F::ZERO
            {
                if F::ZERO < self.look {
                    self.look = self.look - dt;
                    if self.look < F::ZERO {
                        // A plain draw, *not* the gate's no-repeat one: the look-around can repeat
                        // itself and does not touch the list the gate keeps.
                        *want = IDLE + pool.draw(IDLES, clock);
                        self.looking = true;
                        // The idle is put on by the step below, and a clip that has run out stops the
                        // step advancing anything — so the flag is cleared here, in front of it.
                        self.player.finished = false;
                    }
                } else {
                    // Re-arm. The draw is a unit float remapped onto the two bounds and clamped back
                    // into them, which is the engine's shared remap with a degenerate input range it
                    // can never hit.
                    let unit = pool.unit(clock);
                    self.look = remap(unit, F::ZERO, F::ONE, p.look_min, p.look_max);
                }
            }
            if !self.looking {
                *want = TURN_RIDING;
            } else if self.player.finished {
                self.looking = false;
            }
        }
        // Every frame, whichever arm ran: moving off cancels the look timer. The threshold is the pose
        // grid's first one, and the test is signed — rolling backwards does not cancel it.
        if vp.band[0] < r.band {
            self.look = F::ZERO;
            self.looking = false;
        }
    }

    /// The two fields the step reads and never writes, taken off the request once the step has run.
    ///
    /// The step reads [`vehicle_anim::Params::start_phase`] as configuration — S25 had no caller to
    /// tell it otherwise — and this is its writer: it is *last* frame's progress through the gesture,
    /// so the phase a stunt cross-fade starts the outgoing clip at is where that clip was a frame ago.
    /// The sequence byte the step's restart test compares against is the other one, which answers the
    /// question S25's journal left open: the count is latched here, every frame, so a repeat of the
    /// state already running restarts it only on the frame the request's count moves.
    pub fn latch(&mut self, vp: &mut vehicle_anim::Params, r: &vehicle_anim::Request) {
        vp.start_phase = r.progress;
        self.veh.seq = r.seq as u8;
    }

    /// One whole frame of the body's animation: the selection, the step it hands its answer to, the
    /// clip the step asked to be put on, the library call, and the latch.
    pub fn frame(
        &mut self,
        dt: F,
        p: &Params,
        vp: &mut vehicle_anim::Params,
        r: &mut vehicle_anim::Request,
        a: &Ask,
        lib: &Library,
        pool: &mut Pool,
        clock: Clock,
        self_placed: bool,
        clips: &impl Fn(ClipId) -> Clip,
    ) -> Frame {
        self.frame_clocked(dt, p, vp, r, a, lib, pool, clock, self_placed, clips).0
    }

    /// The same frame, with what its two calls into the motion object answered ([`Clocked`]).
    ///
    /// [`Body::frame`] is this and then the answer thrown away, which is what the original does: both
    /// calls return into a frame that ignores them. A *renderer* cannot ignore them — the pose it
    /// draws is sampled out of the same two answers — and neither can be read back off the motion
    /// object once the step has returned, so whoever is going to draw the body calls this one.
    #[allow(clippy::too_many_arguments)]
    pub fn frame_clocked(
        &mut self,
        dt: F,
        p: &Params,
        vp: &mut vehicle_anim::Params,
        r: &mut vehicle_anim::Request,
        a: &Ask,
        lib: &Library,
        pool: &mut Pool,
        clock: Clock,
        self_placed: bool,
        clips: &impl Fn(ClipId) -> Clip,
    ) -> (Frame, Clocked) {
        self.select(dt, p, vp, r, a, pool, clock);
        let frame = self.veh.frame(dt, vp, r, lib, self_placed, |c| clips(c).duration);
        let snapshot = frame.play.is_some_and(|play| self.put_on(play));
        // The clip the library entry drives its clock off, which is the blend's first one or the one
        // already playing.
        let clip = match frame.blend.clock_clip(&self.player) {
            Some(id) => clips(id),
            None => Clip::new(F::ZERO, false),
        };
        let played = frame.blend.run(&mut self.player, dt, &clip);
        self.latch(vp, r);
        (frame, Clocked { played, snapshot })
    }

    /// Put the step's clip on, including the case its slot in the table is empty.
    ///
    /// The original hands the null handle straight to the engine's play, which writes every one of its
    /// fields and leaves "nothing playing" in the clock — so the arm is not a no-op for an empty slot,
    /// it stops whatever was playing. [`Put`] cannot carry a missing clip, so the port plays a handle
    /// and takes it back off: the same fields in the same order, because the clip the transition fades
    /// *from* is read before the new one is stored.
    ///
    /// Answers the one thing the play decides and the step does not look at: whether the transition it
    /// started had no clip to come out of, so that the pose it fades out of is the snapshotted one
    /// ([`Clocked::snapshot`]).
    fn put_on(&mut self, play: Play) -> bool {
        let (put, keep) = match play.put() {
            Some(put) => (put, true),
            None => (Put::clip(ClipId(0), play.duration, play.length), false),
        };
        // The duration of the clip already on, which only a keep-phase play reads and this one never
        // asks for.
        let snapshot = self.player.play(put, F::ZERO);
        if !keep {
            self.player.clip = None;
        }
        snapshot
    }
}
