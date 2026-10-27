//! The rider on the bike: the trick machine, the wreck test and the ride state machine wired onto
//! [`crate::ride::Ride`] in the order the original's update runs them.
//!
//! [`crate::ride`] is the bike — the ground step, the suspension, the integrator and the airborne
//! control, all of it assembled and running. What it is missing is the person: nothing there turns a
//! stick-and-button gesture into a stunt, nothing runs the stunt's clocks, and nothing ever decides
//! that the rider came off. Those three are ported already ([`crate::trick`], [`crate::air`]'s wreck
//! test, [`crate::state`]) and this module is the one place they are joined to the bike, so that a
//! shell can hold one value and get a rider who can trick, land and wreck.
//!
//! # The frame
//!
//! The original's ridden actor runs these in a fixed order (`research/oracle.txt`, A5 — the riding
//! branch of the player update). [`Rider::frame`] is that order:
//!
//! 1. **The latch drops** — [`crate::state::Ride::frame_start`], the top of the player's own update.
//! 2. **The physics**, and only while the rider is not down: [`crate::ride::Ride::frame`]. The
//!    original skips its ground step whenever a wreck reason is set.
//! 3. **The controller** — [`crate::trick::Gesture::read`] turns the rate-limited stick and the three
//!    trick buttons into a group and a direction, and the reset chord is taken here.
//! 4. **The request** — [`crate::trick::Tricks::request`] asks whether this gesture names a stunt, and
//!    [`crate::trick::Tricks::idle`] fills in the two idle poses when it does not.
//! 5. **The clocks** — [`crate::trick::Tricks::update`], then the animation header
//!    ([`crate::motion::Driver::frame`]) on the same request, which is what writes the crossup the
//!    landing is judged on.
//! 6. **The machine** — [`crate::state::Ride::step`], carrying the airborne wreck test.
//! 7. **The wreck page** — not the player's but the screen's fader, which updates after it and puts
//!    the bike back three ticks after a wreck's reset asked for the page ([`crate::wreck_page`]).
//!
//! # One wreck test, not two
//!
//! [`crate::air::wreck_test`] and [`crate::trick::Tricks::landing`] are two ports of the same clause of
//! the original's wreck test, made in two tasks from two directions. The air one is the whole of it —
//! the g loads, the attitude limits, the crossup landing *and* all three trick endings, in the
//! original's order — so that is the one this module calls, with the trick machine's state handed to
//! it through [`crate::air::Loads`]. `Tricks::landing` is left where it is, proved, and unused here;
//! `research/journal/2026-10-16-p3-tricks-in-the-shell/` has why, and which of the two reads the
//! disputed parameter the way the loader stores it.
//!
//! # The body is a second actor
//!
//! The six steps above are the *player's* update, and the rider's own animation is not one of them:
//! the body is an actor of its own, stepped by the actor list, and [`crate::rider_anim`] is its
//! machine (S27). So it is here as [`Rider::body_frame`] — a second entry point, called after
//! [`Rider::frame`] by whoever holds the clip table — rather than a seventh step inside the frame.
//! What this module contributes is the mapping: which of the rider's own fields each half of the
//! request block the body reads is published from. That mapping is **the port's own** and is listed
//! on `body_frame`; the publisher in the original was not found (S25 left it unidentified too).
//!
//! # What is this module's own and not a port
//!
//! Only the joining. Every number comes off the disc through [`Phy`] and the stunt table; every
//! decision is made inside one of the four ported modules. The three places the wiring had to choose
//! are marked **the port's own** where they are written, and all three are in the journal. The body's
//! request block is a fourth, with a section of its own above and a table on [`Rider::body_frame`]:
//! that one is a choice because the original's publisher of the block was never found.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::air;
use crate::hash::{self, State, Walk};
use crate::motion;
use crate::off_map;
use crate::phy::{self, Phy};
use crate::ride::{self, Controls, Crash, Ride};
use crate::ragdoll_ride::{self, Down, Kit, Posed};
use crate::ragdoll_riding::{Holds, Riding};
use crate::rider_anim;
use crate::state;
use crate::wreck_sound;
use crate::wreck_page;
use crate::ghost::Ghost;
use crate::score::{self, Score};
use std::sync::Arc;
use crate::trick::{self, AngleTable, Set, Trick, Tricks, Wreck};
use crate::vehicle_anim;
use crate::world::World;
use ps2_float::{F, Vf};

/// The tables a rider needs built once: the bike's two, and the trick decoder's arc cosine.
///
/// The decoder's is its own table and not [`crate::air::AcosTable`] — same function, different grid
/// (30,001 entries against the air code's, built to a different step) — so both are here rather than
/// one being made to stand in for the other.
pub struct Tables {
    /// Everything [`Ride::frame`] reads.
    pub ride: ride::Tables,
    /// The table [`crate::trick::look`] classifies a stick direction with.
    pub angles: AngleTable,
}

impl Tables {
    pub fn build() -> Tables {
        Tables { ride: ride::Tables::build(), angles: AngleTable::build() }
    }
}

/// What one frame of the rider did, beside the state it wrote.
///
/// **The port's own.** The original tells the camera, the announcer and the race record instead of
/// answering anything; this is the same news in a form a shell can print and a fixture can assert on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Frame {
    /// A trick was asked for this frame, and started at once rather than joining the queue.
    pub started: Option<Trick>,
    /// The trick that was running when the wheels came back down, on a touchdown that was not a
    /// wreck. Bookkeeping, not a port: the original counts a landed trick in the scoring, which is a
    /// different task.
    pub landed: Option<Trick>,
    /// The rider came off this frame, for this reason.
    pub wreck: Option<Wreck>,
    /// The rider let go of the bars this frame: the wreck's `HandHoldTime` is up and the rider is off
    /// the bike, which is the frame the separation gesture would be played on.
    pub separated: bool,
    /// The rider was put back on the bike this frame.
    pub respawned: bool,
    /// The widths the rider was moved back over a seam at the end of the frame (MD29,
    /// [`Rider::wrap_round`]), `[x, z]`: what the shell moves its camera and whatever else it keeps
    /// in world feet by. `[0, 0]` on every world that does not wrap.
    pub seam: [i8; 2],
}

/// What the rider's own animation needs that the rider does not hold.
///
/// Two inputs, both of them someone else's: the gate's clock, which the body's get-ready sequence
/// counts its breathing off, and the wall clock the idle pool seeds its generator from the first
/// time it draws ([`rider_anim::Clock`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gate {
    /// Seconds left on the start-line clock, which the body takes off its own three and a half
    /// (`rider_anim`'s `BREATHE`). The original reads it out of the request block; this port has no
    /// race, so it is an argument.
    pub countdown: F,
    /// The clock the idle pool seeds itself off, the first time anything draws from it. A shell that
    /// does not care may leave it at zero (which seeds the generator with zero, and in the original
    /// would leave the seeded flag clear so that the next draw read the clock again — see
    /// [`rider_anim::Pool::seeded`]); a replay hands back the numbers it recorded.
    pub clock: rider_anim::Clock,
}

/// **The two files a wreck needs**, which are not the bike's own: `Wreckbike.phy`'s one rigid-body
/// part, and the `[Wrecks]` block of the rider's.
///
/// A [`Rider`] built without these cannot wreck and recover — [`crate::state::Sense::settles`] is
/// never set, the automatic reset is out of reach, and the hand chord is the only way back, which is
/// where this port was before S26. [`Rider::carries_wreck`] is how a shell hands them over.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wreckage {
    /// `Wreckbike.phy`, section `[PhysicsPart1]`.
    pub part: phy::Part,
    /// The rider's `[Wrecks]`, out of `rider\anim\mrider.phy`.
    pub wrecks: phy::Wrecks,
    /// The wreck body's inertia tensor, which comes from its model and not from either file:
    /// [`crate::ride::WRECK_INERTIA`] for the disc's own bike.
    pub inertia: Vf,
}

/// **The put-back's hold on the body** (S74): for the two ticks after the bike is set down, the body
/// is told the bike is placing it, and it sits on the bike straight off its clips rather than riding.
///
/// The original keeps the pair on the bike: a "placed" word, which the chord's put-back and the wreck
/// clock's both raise, and a copy of it as the last request saw it. The bike's update copies the first
/// into the body's request after its physics and then clears it if the request had already seen it,
/// so a put-back is held for exactly two request fills — this tick and the next for the chord, which
/// is taken before the physics, and the next two for the clock, which runs after. The same fill
/// copies the wreck, which is why the body still sees the rider down on the tick the clock puts them
/// back. Measured on nine put-backs in five captures (S74's journal).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PutBack {
    /// The bike has just been set down, and the body has not yet been told twice.
    pub placed: bool,
    /// `placed` as the last fill saw it.
    pub filled: bool,
    /// The body is held on the bike: what the last fill copied into the request.
    pub held: bool,
    /// The rider was down as of the last fill. The body's view of the wreck lags the machine by the
    /// fill, which sits before the machine's verdict.
    pub wreck: bool,
}

impl PutBack {
    /// The bike has been set down.
    pub fn place(&mut self) {
        self.placed = true;
    }

    /// The request's fill, and the end of the bike's update that follows it: what the body is told,
    /// and the word cleared once the body has been told on two fills running.
    pub fn fill(&mut self, wreck: bool) {
        self.held = self.placed;
        self.wreck = wreck;
        if self.placed && self.filled {
            self.placed = false;
        }
        self.filled = self.placed;
    }
}

/// A bike with somebody on it.
///
/// `Clone` because it is most of a [`crate::Sim`], and a snapshot of the simulation is a copy of
/// one ([`crate::Sim::clone`]).
#[derive(Clone, Debug)]
pub struct Rider {
    /// The bike: P2's assembled ride, stepped by this module and not by the shell.
    pub ride: Ride,
    /// The trick machine (S11): the gesture, the queue, the clocks.
    pub tricks: Tricks,
    /// The ride state machine (S5): riding, down, put back.
    pub state: state::Ride,
    /// The animation header (S6). The rider does not draw here, but the header writes the crossup
    /// pose the landing test reads, so it has to run.
    pub motion: motion::Driver,
    /// The pose the header showed last tick ([`motion::Driver::pose`]), as the state pack latches it
    /// in front of this tick's header (S111): what the riding body's request carries.
    pub packed_pose: F,
    /// The fore-aft the header showed last tick ([`motion::Driver::fore_aft`]), latched by the same
    /// pack (S112): the body's vehicle-animation request reads it as its mix axis.
    pub packed_fore_aft: F,
    /// The 24 stunts this rider can do, off the stunt table.
    pub set: Set,
    /// Which stunt clips the rider's animation set actually holds. Everything is present until a
    /// shell with a real motion library says otherwise ([`crate::motion::Library::present`]).
    pub clips: motion::Clips,
    /// What the state machine is told about this rider each frame: who is driving, and whether the
    /// session wrecks at all. [`Rider::new`] sets a local human in a freestyle session.
    pub sense: state::Sense,
    /// A racing event rather than freestyle, which is a different vertical g limit.
    pub racing: bool,
    /// The two wreck files, once a shell has handed them over ([`Rider::carries_wreck`]).
    pub wreckage: Option<Wreckage>,
    /// The wreck being ridden out, if the rider is down: the bike tumbling and the rider coming off
    /// it ([`Crash`]). `None` whenever the rider is up, and `None` for a rider with no
    /// [`Wreckage`] — which is a rider who stays down until the chord, as before S26.
    pub crash: Option<Crash>,
    /// The rider's ragdoll as they get on, once a shell has handed it over
    /// ([`Rider::carries_ragdoll`]). A constant, like [`Rider::wreckage`].
    pub ragdoll_kit: Option<Arc<Kit>>,
    /// The bike's two collision files (S78), once a shell has handed them over
    /// ([`Rider::carries_bike_col`]): what this bike strikes other riders with and what they strike.
    /// A constant, like [`Rider::wreckage`]; with none the bike is in nobody's way.
    pub bike_col: Option<Arc<crate::bike_col::BikeCol>>,
    /// The ragdoll of the wreck being ridden out (S53): the rider's bones, thrown off the bike and
    /// stepped while they are down. `None` when the rider is up, and for a rider with no kit or no
    /// [`Wreckage`], who is down as one point of [`Crash`]'s as before.
    pub ragdoll: Option<Down>,
    /// The skeleton as the shell last drew it (S65), which a wreck's bones are posed from and pull
    /// towards in place of the kit's fixed riding pose. An input like the controls: a shell that
    /// draws the body writes it every frame, and `None` keeps the kit's pose.
    pub skeleton: Option<Arc<Posed>>,
    /// The skeleton's nodes' drawn matrices as the last riding tick placed them (S70): what a wreck
    /// entered on the next tick measures each bone's velocity and spin from, as the original's pose
    /// measures them from the node's last drawn frame. Empty with no kit, while down, and on the
    /// tick of a put-back, when the next wreck starts its bones as [`Down::enter_posed`] does.
    pub drawn: Vec<crate::math::Mat4>,
    /// **The riding ragdoll** (S109; the rider's since S110): the body actor stepping the eighteen
    /// bones while the rider is on the bike, at the top of each frame for the tick the shell last
    /// drew. A wreck enters from its bodies ([`Riding::hand_over`]) as the original's does, and falls
    /// back on the animated skeleton ([`Rider::skeleton`], [`Rider::drawn`]) without it. `None` with no
    /// kit or no [`Rider::holds`], while down, and after a put-back until the next frame starts a
    /// fresh one.
    pub riding: Option<Box<Riding>>,
    /// The drawn bike's root node and the four holds (S110), which the riding ragdoll is held to. An
    /// input like [`Rider::skeleton`]: a shell that draws the chassis writes it every frame, it is
    /// never sent, and `None` runs no riding ragdoll.
    pub holds: Option<Arc<Holds>>,
    /// The rider's wreck sounds, as far as the pick goes: which was played last, so that the next
    /// wreck draws as the original does ([`wreck_sound`]).
    pub wreck_sounds: wreck_sound::Emitter,
    /// What the animation driving asked for this frame: the one thing [`motion::Driver::frame`]
    /// answers, kept rather than dropped.
    ///
    /// The rider itself reads nothing out of it — the only field of the driving it needs is the
    /// crossup the header writes ([`motion::Driver::shown_crossup`]) — but a shell that draws the
    /// rider needs the blend the selection asked for, and running a second driver beside this one
    /// to get it would be two state machines where the original has one. Left at
    /// [`motion::Blend::Free`] until the first frame.
    pub animation: motion::Frame,

    /// The rider's **own** animation machine (S27): the body actor's selection and the step it feeds.
    /// Run by [`Rider::body_frame`] and by nothing else — [`Rider::frame`] neither reads nor writes
    /// it, so a sim that does not draw costs nothing for having it here.
    pub body: rider_anim::Body,
    /// The body's three numbers ([`rider_anim::Params::built`]).
    pub body_numbers: rider_anim::Params,
    /// The fade rates and pose-grid thresholds the body's *step* reads.
    ///
    /// **Left at zero, and a shell is expected to fill it.** No loader of the body's five rate fields
    /// or of its three thresholds was found (S25's journal left the same hole, and S27 did not close
    /// it), so rather than invent eight numbers this starts at the default: every fade is a hard cut
    /// and the pose grid's rows all meet at zero. [`rider_anim::Body::latch`] writes `start_phase`
    /// here every frame, which is why the whole block is a field and not an argument.
    pub body_rates: vehicle_anim::Params,
    /// The idle pool: which of the ten gate idles have been used, and the generator they are drawn
    /// with.
    pub idles: rider_anim::Pool,
    /// What the body's clock did on the last [`Rider::body_frame`], for whoever draws the body: the
    /// transition that posed the frame and the pose snapshot its put asked for
    /// ([`rider_anim::Clocked`], and V17 on why neither can be read back off the motion object once
    /// the step has returned). Written by `body_frame` and by nothing else.
    pub body_clocked: rider_anim::Clocked,
    /// The riding body's two foot holds (V36, V68): [`crate::foot_down::Feet`], stepped by
    /// [`Rider::body_frame`] on every tick that is neither a wreck nor a pose off the clip table.
    /// Here rather than with the drawn body so that every rider whose body is run steps it, a ride's
    /// online seats too, and a rollback or a resync takes it back with the rest of the rider. Read
    /// by whoever draws the body; nothing in the simulation reads it, and it is not hashed.
    pub feet: crate::foot_down::Feet,
    /// The request block the body's selection and step read, as this module publishes it.
    /// [`Rider::body_frame`] rewrites every field it derives from the rider each frame and leaves the
    /// other two (`cut`, `offset`) alone, because the body itself is the only thing here that writes
    /// them. [`Rider::new`] raises `cut`: a rider is put on the bike at the gate; a free ride lowers
    /// it again ([`Rider::no_start_line`]).
    pub body_request: vehicle_anim::Request,

    /// The last trick asked for since the wheels left the ground, for [`Frame::landed`].
    pub(crate) flight: Trick,
    /// Whether the bike was off the ground at the end of the previous frame, which is the edge
    /// [`Frame::landed`] is taken on — [`crate::air::Air::was_airborne`] is a *substep*'s memory and
    /// says nothing about the frame before.
    pub(crate) airborne: bool,
    /// The bike's ghost after a put-back (P57, S63): see-through for three seconds, passing through
    /// the other riders, its clock held while it is inside one.
    pub ghost: Ghost,
    /// The map edge is armed on this bike (S73): [`off_map::armed`] of who the bike was built for and
    /// the event's league, which the bike's builder decides once and nothing changes afterwards. A
    /// constant of the session, like the stunt set: not sent with a snapshot, and not hashed.
    pub edge_armed: bool,
    /// The wreck page (S72): the screen's fade to black that a wreck's automatic reset asks for, and
    /// that puts the bike back when it is black.
    pub page: wreck_page::Page,
    /// The put-back's two-tick hold on the body (S74), filled by [`Rider::frame`] and read by
    /// [`Rider::body_frame`].
    pub put_back: PutBack,
    /// The bones as the original still draws them on the tick the wreck clock puts the rider back
    /// (S74): the body's request has not seen the put-back yet, so the body is still down and drawn
    /// thrown for one more tick while the bike is already standing on the put-back record.
    /// Presentation only — nothing simulated reads it, and the next [`Rider::frame`] drops it.
    pub ragdoll_shown: Option<Box<Down>>,
    /// The rider's points (S79): the combo being scored and the total banked.
    pub score: Score,
}

impl Rider {
    /// **Over a seam** (MD29, the port's own): on a world that wraps, a rider whose chassis has
    /// crossed a seam is moved back one width on that axis ([`Rider::shift`]), and the answer is the
    /// widths it was past, `[x, z]`: `1` past the far seam (moved back by the width, exactly), `-1`
    /// short of the near one (moved on by it, rounding to the far side's last bit). Nothing moves
    /// while the rider is down or the riding bodies are up: everything a ride asks is asked past a
    /// seam as well ([`crate::tiles::Tiled::fold`]), and the put-back sets the bike down inside the
    /// world, where the next riding tick looks again. Every peer asks after the same tick, from the
    /// same state, so a session stays in step.
    pub fn wrap_round(&mut self, world: &dyn World) -> [i8; 2] {
        let wrap = world.wrap();
        let Some(area) = world.area().filter(|_| wrap != [false, false]) else { return [0, 0] };
        if self.state.wreck.is_some() || self.crash.is_some() || self.ragdoll.is_some() || self.riding.is_some() {
            return [0, 0];
        }
        let at = self.ride.chassis.cofg_world;
        let lap = |on: bool, v: f32, side: F| -> i8 {
            if !on {
                0
            } else if F(v) >= side {
                1
            } else if v < 0.0 {
                -1
            } else {
                0
            }
        };
        let laps = [lap(wrap[0], at.x(), area.width), lap(wrap[1], at.z(), area.depth)];
        if laps == [0, 0] {
            return laps;
        }
        let by = |lap: i8, side: F| match lap {
            1 => F::ZERO - side,
            -1 => side,
            _ => F::ZERO,
        };
        self.shift([by(laps[0], area.width), by(laps[1], area.depth)], [area.width, area.depth]);
        laps
    }

    /// Move a riding rider's world-space state along `x` and `z` by `by` (MD29): the chassis' centre
    /// of gravity and the transform's origin, the wheels' world points, the drawn skeleton's
    /// origins, and the put-back record when it is past the world's `sides` on a moved axis — a
    /// record from before the crossing is a place in the world already. The put-back's mesh query
    /// gathers afresh: what it kept was gathered on the other side. Velocities, forces, normals and
    /// everything in the bike's own frame are not places and do not move.
    pub fn shift(&mut self, by: [F; 2], sides: [F; 2]) {
        let moved = |v: Vf| Vf::new((F(v.x()) + by[0]).0, v.y(), (F(v.z()) + by[1]).0, v.w());
        let ride = &mut self.ride;
        ride.chassis.cofg_world = moved(ride.chassis.cofg_world);
        ride.origin = moved(ride.origin);
        for wheel in [&mut ride.front, &mut ride.rear] {
            let w = &mut wheel.w;
            w.mount_world = moved(w.mount_world);
            w.probe_end_world = moved(w.probe_end_world);
            w.contact_point = moved(w.contact_point);
            w.load_point = moved(w.load_point);
        }
        let out = |v: f32, side: F| v < 0.0 || F(v) >= side;
        let back = ride.back.at;
        let past = (by[0] != F::ZERO && out(back.x(), sides[0])) || (by[1] != F::ZERO && out(back.z(), sides[1]));
        if past {
            ride.back.at = moved(back);
        }
        ride.back_search = None;
        for m in &mut self.drawn {
            m.0[3] = moved(m.0[3]);
        }
    }

    /// A rider on a bike, at a spawn.
    ///
    /// `set` is the rider's own 24 stunts: [`crate::trick::Stunts::selection`] of the player's slot,
    /// through [`Set::new`].
    pub fn new(p: &Phy, mounts: (Vf, Vf), at: Vf, facing: Vf, set: Set) -> Rider {
        Rider {
            ride: Ride::new(p, mounts, at, facing),
            tricks: Tricks::default(),
            state: state::Ride::default(),
            motion: motion::Driver::default(),
            packed_pose: motion::Driver::default().pose,
            packed_fore_aft: motion::Driver::default().fore_aft,
            set,
            clips: motion::Clips::default(),
            // A local human with a screen of their own, freestyle, wrecks on. `settles` starts false
            // and is written every frame from the wreck in progress ([`Crash::settles`]) once a shell
            // has handed over the two wreck files; a rider with none keeps it false, and for that one
            // the automatic reset is out of reach and the chord is the way back.
            sense: state::Sense {
                ai_route: false,
                owner: Some(state::Owner { human: true, screen: true }),
                wrecks_enabled: true,
                air_wrecks: true,
                settles: false,
            },
            racing: false,
            wreckage: None,
            crash: None,
            ragdoll_kit: None,
            bike_col: None,
            ragdoll: None,
            skeleton: None,
            drawn: Vec::new(),
            riding: None,
            holds: None,
            wreck_sounds: wreck_sound::Emitter::retail(),
            animation: motion::Frame { blend: motion::Blend::Free, restart: false },
            body: rider_anim::Body::default(),
            body_numbers: rider_anim::Params::built(),
            body_rates: vehicle_anim::Params::default(),
            idles: rider_anim::Pool::default(),
            body_clocked: rider_anim::Clocked::default(),
            feet: crate::foot_down::Feet::default(),
            // At the gate: the cut flag is what the body's selection takes its get-ready sequence
            // off, and the body is the only thing in the port that ever clears it (the throttle
            // opening). **The port's own** — in the original the flag is published by whatever owns
            // the request block, which is not identified.
            body_request: vehicle_anim::Request { cut: true, ..vehicle_anim::Request::default() },
            flight: Trick::NONE,
            airborne: false,
            ghost: Ghost::default(),
            // A player's bike on a free-ride area, the only event the port has.
            edge_armed: off_map::armed(off_map::Built::Player, off_map::League::FreeRide),
            page: wreck_page::Page::default(),
            put_back: PutBack::default(),
            ragdoll_shown: None,
            score: Score::default(),
        }
    }

    /// Put the rider and the bike back at a place, as a respawn does.
    pub fn place(&mut self, at: Vf, facing: Vf) {
        self.ride.place(at, facing);
        self.crash = None;
        self.ragdoll = None;
        self.ragdoll_shown = None;
        self.tricks.reset();
        self.flight = Trick::NONE;
        self.airborne = false;
    }

    /// **A marker's teleport** (P88, [`crate::markers`]): the whole bike put back as a marker kept
    /// it — its place, its heading, its speed — and the rider on it, as a put-back leaves one: up out
    /// of any wreck, the trick machine cleared, and ghosted, so it passes through whoever is there.
    pub fn teleport(&mut self, ride: Ride) {
        self.ride = ride;
        self.teleported();
    }

    /// A teleport onto a spot rather than a marker — another player, where they are: with the spot's
    /// motion (P111, a player riding) the bike rides on as theirs does, at their speed and spin
    /// ([`Ride::ride_as`]); with a flight (P141, a player flying the plane) it is thrown from the
    /// spot under the plane at the plane's velocity ([`Ride::launch`]) and falls as off a jump; with
    /// neither (a player down, or in a plane on the ground) it is set down there stopped, on their
    /// heading and their up. The rest as [`Rider::teleport`]: no wreck for the teleport itself.
    pub fn teleport_to(&mut self, spot: crate::markers::Spot) {
        match (&spot.motion, &spot.flight) {
            (Some(m), _) => self.ride.ride_as(&m.0),
            (None, Some(f)) => self.ride.launch(spot.at, spot.forward, spot.up, f.body.velocity),
            (None, None) => self.ride.place_on(spot.at, spot.forward, spot.up),
        }
        self.ride.unwreck();
        self.ride.drag_halved = false;
        self.teleported();
    }

    /// **The rider leaves the bike mid-trick** (P161, a machine switch): the wreck entry's half for
    /// the tricks and the points, and nothing for the bike — the running trick stopped and its queue
    /// emptied, the trick pack's spin and front flip watch cleared, and the combo lost as a bail
    /// loses it ([`Score::wrecked`]): nothing banked, the one-wheel clocks and the backflip watch back
    /// to nothing. **The port's own**: what the original's swap does to a trick in progress was not
    /// identified.
    pub fn trick_lost(&mut self) {
        self.tricks.reset();
        self.flight = Trick::NONE;
        self.score.wrecked();
        if let Some(t) = self.ride.tricks.as_deref_mut() {
            t.spin = air::Spin::default();
            t.front_watch = 0;
        }
    }

    /// **No start line** (V88): lowers the cut flag [`Rider::new`] raises, so the body rides from its
    /// first frame. A free ride has no gate, and left up the flag has the body play the start line's
    /// get-ready sequence — a breathe, a get-ready idle and a start pose, hands off the bars — until
    /// the throttle opens during an idle or a start pose, while the bike rolls away under it. A ride
    /// started in the plane hands the bike over with the flag still up, which is what the user saw
    /// as the body not animating for the first seconds after the switch back.
    ///
    /// **The port's own**, as the flag's raising is: who publishes the cut flag in the original was
    /// not identified, so whether its free ride clears it or never raises it is not known; the body
    /// it leaves is the riding one either way.
    pub fn no_start_line(&mut self) {
        self.body_request.cut = false;
    }

    /// What both teleports leave the rider as, which is what the put-backs in [`Rider::frame_among`]
    /// leave: the machine out of a wreck, no wreck body, no trick, a fresh drawn history, the ghost on.
    fn teleported(&mut self) {
        if self.state.wreck.is_some() {
            self.state.respawn();
        }
        self.crash = None;
        self.ragdoll = None;
        self.tricks.reset();
        self.flight = Trick::NONE;
        self.airborne = self.ride.air.airborne;
        self.drawn.clear();
        // A wreck page fading when the teleport came is dropped: there is no wreck left to put back.
        self.page = wreck_page::Page::default();
        self.ghost.start();
    }

    /// **Hand the rider the two files a wreck needs** — `Wreckbike.phy`'s part and the rider's
    /// `[Wrecks]` — and the wreck body's inertia tensor.
    ///
    /// Not an argument to [`Rider::new`] because a rider is rideable without them: everything a
    /// shell or a fixture does up to the first wreck is the same either way.
    pub fn carries_wreck(&mut self, part: phy::Part, wrecks: phy::Wrecks, inertia: Vf) {
        self.wreckage = Some(Wreckage { part, wrecks, inertia });
    }

    /// **Hand the rider their ragdoll** (S53): from then on a wreck throws the bones off the bike,
    /// and they are what the settle gate and the body's mode machine read while the rider is down.
    /// Needs [`Rider::carries_wreck`] too: the bones' hands are held to the wreck bike.
    pub fn carries_ragdoll(&mut self, kit: Arc<Kit>) {
        self.ragdoll_kit = Some(kit);
    }

    /// **The riding ragdoll's tick** (S110), for the tick the shell last drew: its request off the ride
    /// and [`Rider::holds`], its skeleton [`Rider::skeleton`]. Only while the rider is up and has a
    /// kit and holds; started afresh when there is none yet or the bike has gone where the bones did
    /// not ([`Riding::strayed`]).
    fn riding_tick(&mut self, dt: F, world: &mut dyn World) {
        let up = self.state.wreck.is_none() && self.crash.is_none() && self.ragdoll.is_none();
        let (Some(kit), Some(holds), true) = (self.ragdoll_kit.clone(), self.holds.clone(), up) else {
            self.riding = None;
            return;
        };
        let at = self.ride.position();
        let mut riding = self.riding.take().filter(|r| !r.strayed(at)).unwrap_or_else(|| Box::new(Riding::new(&kit)));
        riding.ride(&kit, self, &holds, dt, world);
        self.riding = Some(riding);
    }

    /// **Hand the bike its collision files** (S78): from then on, in a session with other riders, its
    /// body rays strike theirs and theirs strike it ([`crate::rider_contact`]).
    pub fn carries_bike_col(&mut self, col: Arc<crate::bike_col::BikeCol>) {
        self.bike_col = Some(col);
    }

    /// **The rider's main bone, in the world**: the point the body actor publishes onto the bike every
    /// tick, which the map-edge test asks about beside the chassis. While the rider is down it is the
    /// ragdoll's main bone, or the one point [`Crash`] has for a rider with no ragdoll. Riding, it is
    /// the riding pose's main bone on the bike — the pose the port has, not the animated one the
    /// original publishes. With neither a kit nor a wreck it is the empty vector, which the test skips.
    pub fn main_bone(ride: &Ride, kit: Option<&Kit>, ragdoll: Option<&Down>, crash: Option<&Crash>) -> Vf {
        match (ragdoll, crash, kit) {
            (Some(doll), _, _) => doll.main().position,
            (None, Some(down), _) => down.rider,
            (None, None, Some(kit)) => kit.main_bone(&ride.transform()),
            (None, None, None) => crate::r#move::NOTHING,
        }
    }

    /// **The bones to draw the body from**, if it is drawn thrown: the ragdoll while the rider is down,
    /// and on the tick the wreck clock puts them back, the bones as they were (S74,
    /// [`Rider::ragdoll_shown`]).
    pub fn drawn_ragdoll(&self) -> Option<&Down> {
        self.ragdoll.as_ref().or(self.ragdoll_shown.as_deref())
    }

    /// **The drawn rider jumps this tick**, so a shell that blends ticks should cut instead (S74): the
    /// bike set down on the put-back record (S66), and the body's first tick held on it, which is the
    /// same tick after a chord and the next one after the wreck clock ([`PutBack`]).
    pub fn drawn_jumps(&self) -> bool {
        self.ghost.put_back() || (self.put_back.held && self.put_back.placed)
    }

    /// Which of the three states the rider is in.
    pub fn phase(&self) -> state::Phase {
        self.state.phase()
    }

    /// **One frame of the rider**, in the original's order. The six steps at the top of this file.
    pub fn frame(&mut self, p: &Phy, t: &Tables, c: &Controls, dt: F, world: &mut dyn World) -> Frame {
        self.frame_among(p, t, c, dt, world, &[])
    }

    /// [`Rider::frame`] in a session with other riders: the same six steps, with the wreck test's
    /// contact list swept against `rivals` as well as the world ([`ride::hits_among`]), which is the
    /// one place another rider reaches into this one's frame. With none it is `frame`.
    pub fn frame_among(&mut self, p: &Phy, t: &Tables, c: &Controls, dt: F, world: &mut dyn World,
                       rivals: &[ride::Rival]) -> Frame {
        self.frame_bumped(p, t, c, dt, world, rivals, None)
    }

    /// [`Rider::frame_among`] with this tick's rider-against-rider contacts (S78), which the bike's
    /// ground step resolves ([`ride::Ride::frame_bumped`]). With none it is `frame_among`.
    #[allow(clippy::too_many_arguments)] // frame_among's, and the contacts
    pub fn frame_bumped(&mut self, p: &Phy, t: &Tables, c: &Controls, dt: F, world: &mut dyn World,
                        rivals: &[ride::Rival], bump: Option<&mut crate::rider_contact::Bump>) -> Frame {
        let mut out = Frame::default();
        // 0a — the riding ragdoll's tick for the frame the shell last drew (S110): the body actor runs
        // after the rider in the original's tick, so a wreck entered this tick starts from the bodies
        // it left.
        self.riding_tick(dt, world);
        let Rider {
            ride, tricks, state, motion, packed_pose, packed_fore_aft, set, clips, sense, racing, wreckage, crash, animation,
            flight, airborne, ragdoll_kit, ragdoll, skeleton, drawn, riding, wreck_sounds, idles, ghost, edge_armed, page,
            put_back, ragdoll_shown, score, bike_col, ..
        } = self;
        *ragdoll_shown = None;
        let mut sense = *sense;

        // 0 — the ghost's tick, the first thing in the bike's update: its clock holds while the
        // body is inside another rider's (`rivals` holds no ghosts, whose lists reject this one).
        ghost.step(dt, || crate::ghost::overlaps(&ride.rival(), rivals));

        // 1 — the top of the player's update: the hand-reset latch is dropped.
        state.frame_start();

        // 1b — the L1+R1 put-back, which the original takes in its input phase, before the update
        // (S66): the bike is set down on the put-back record and this frame's physics runs from there.
        if c.respawn && state.chord_allowed(sense.ai_route) {
            state.take_chord();
            // The original's reset request ends the rider's jump first, which is the points' wreck:
            // the combo is lost whether or not the put-back happens (S79).
            score.wrecked();
            if state.request_reset(state::Kind::Wrecked, false, sense.owner) == Some(state::Reset::Now) {
                state.respawn();
                ride.place_back();
                ride.unwreck();
                ride.drag_halved = false;
                *crash = None;
                *ragdoll = None;
                tricks.reset();
                *flight = Trick::NONE;
                out.respawned = true;
                ghost.start();
                put_back.place();
                // The put-back's unwreck cuts a trick short, which after the request finds nothing.
                score.aborted();
            }
        }

        // 1c — the skeleton's nodes on the bike where the last tick left it, and the drawn matrices
        // they were at the tick before (S70): a wreck entered this tick poses its bones from both, as
        // the original's entry does from the riding skeleton's nodes and their last drawn frame.
        let drawn_before = std::mem::take(drawn);
        let nodes = match ragdoll_kit.as_ref() {
            Some(kit) if state.wreck.is_none() => Some(ragdoll_ride::nodes(kit, skeleton.as_deref(), &ride.transform())),
            _ => None,
        };

        // The rider's slow test, which the original works out at the top of its tick on the speed the
        // bike's last update left: before this frame's moves it (S79).
        let slow = ride.air.contact_vel.z() < score::SLOW;

        // 2 — the physics, only while the rider is not down. The respawn is the machine's to order,
        // so the bike's own chord is taken out of the controls it is handed here and read below.
        let was_airborne = *airborne;
        // The settle gate the machine reads at the end of the bike's update, as the body's step left
        // it last tick: the rider's tick, which writes it, runs after the bike's (S65).
        let gate = ragdoll.as_ref().map(Down::settles);
        // A cannonball's record update, when this tick's has one: the wreck part's point, and the
        // wreck bike's translation and forward row (S77).
        let mut thrown_keep = None;
        // What of the record's list the body rays found on the cars (S137), for the wreck test.
        let cars = bump.as_deref().map_or_else(Vec::new, |b| crate::mover_contact::wreck_contacts(&b.contacts));
        // What the body rays found in the ground (P147), which the wreck test reads too.
        let mut in_ground = Vec::new();
        if state.wreck.is_none() {
            // The body rays against the world's props (S138), from where the last tick left the bike,
            // as its collision object searches them before the bikes are updated: what they touch goes
            // on the bump's list beside the other riders' frames, or on a list of its own.
            // Tipped past its up limit, the same rays look for the terrain first (P147): those
            // contacts head the list.
            let ground = bike_col.as_deref().map_or_else(Vec::new, |col| crate::rider_contact::ground(col, &ride.transform(), world, dt));
            in_ground = crate::rider_contact::ground_hits(&ground);
            let (found, movers) = match bike_col.as_deref().and_then(|col| crate::rider_contact::scenery(col, &ride.transform(), world, dt)) {
                Some((f, m)) => (Some(f), m),
                None => (None, Vec::new()),
            };
            let scenery = match &found {
                Some(f) => Some(!f.is_empty() || !ground.is_empty()),
                None => (!ground.is_empty()).then_some(true),
            };
            let mut alone = None;
            let bump = match (bump, found) {
                (Some(b), found) => {
                    // The cars are props too: a ray the car search (S137) already listed keeps that entry.
                    let found: Vec<_> = found.unwrap_or_default().into_iter().filter(|f| !b.contacts.iter().any(|c| c.other.is_none() && c.from == f.from && c.on_body == f.on_body)).collect();
                    b.contacts.splice(0..0, ground);
                    b.contacts.extend(found);
                    // The body rays' entries on a moving prop name it as their mover (S145); the car
                    // search's (S137) still name none, so they keep the scenery response.
                    b.movers = movers;
                    Some(b)
                }
                (None, found) => {
                    let mut list = ground;
                    list.extend(found.unwrap_or_default());
                    if list.is_empty() { None } else { Some(alone.insert(crate::rider_contact::Bump { movers, ..crate::rider_contact::alone(ride, p, list) })) }
                }
            };
            // The bike's limiter reads the no-hander as the rider's last tick left it (`+0xa44`).
            let c = Controls { respawn: false, hands_off: tricks.no_hander, ..*c };
            ride.frame_searched(p, &t.ride, &c, dt, world, bump, scenery);
        } else if let (Some(w), Some(down)) = (wreckage.as_ref(), crash.as_mut()) {
            // The physics of a rider who is down: the wreck body tumbling and the rider coming off
            // it. The original's is the same swap — the bike's own step is skipped while the reason
            // is set, and the wreck bike and the ragdoll are stepped instead.
            let held = down.holding;
            down.step(dt, &w.part, &w.wrecks, world);
            out.separated = held && !down.holding;
            // The put-back record goes on being kept while the rider is down, off the wreck body: its
            // centre is the probe and its forward row the heading (S66). A cannonball has its own
            // branch, on the wreck bike's transform and only while it touches something (S77); the
            // original takes it after the whole update, so it is kept below, after the verdict.
            if state.wreck != Some(Wreck::CANNONBALL) {
                let heading = crate::math::quat_to_mat4(down.bike.body.quat, down.origin).0[2];
                ride.keep_back(down.bike.body.position, heading, world, &t.ride.curve);
            } else if down.bike.record.touching {
                let node = &down.bike.record.node;
                thrown_keep = Some((down.bike.body.position, node.origin, node.rows[2]));
            }
            // The rider's bones, when there are any: they let go of the bars on their own clock.
            if let (Some(kit), Some(doll)) = (ragdoll_kit.as_ref(), ragdoll.as_mut()) {
                let held = doll.ragdoll.holds[0] || doll.ragdoll.holds[1];
                // The wreck bike leaves the broad phase at the put-back, which the original makes in
                // the bike's update, before the body's (S108): the page's last fade-out update, or
                // the machine's own reset, asked here of a copy of it on the settle gate it reads.
                let unwrecks = page.puts_back_next() || {
                    let mut s = sense;
                    s.settles = doll.settles();
                    let params = state::Params {
                        max_wreck_time: p.wreck_times.max_wreck_time,
                        cannonball_wreck_time: p.wreck_times.cannonball_wreck_time,
                    };
                    let mut machine = *state;
                    machine.step(dt, s, params, |_| None, || page.up()).reset == Some(state::Reset::Now)
                };
                doll.step_posed_with(kit, skeleton.as_deref(), dt, down, world, !unwrecks);
                out.separated = held && !(doll.ragdoll.holds[0] || doll.ragdoll.holds[1]);
            }
        }
        *airborne = ride.air.airborne;
        // The body's request is filled here, after the physics and before the machine's verdict (S74).
        put_back.fill(state.wreck.is_some());

        // 3 — the controller. **The port's own ordering:** the original reads the pad in its input
        // phase, before the update; the read is here instead, before the only three things that look
        // at a gesture, which is the same frame's worth of information either way. What it reads is
        // the player-range stick as the pad gave it — not the rate limiter's squared copy, which is
        // written after the read (P30).
        let down = state.wreck.is_some();
        tricks.gesture.read(
            &t.angles,
            c.stick,
            // Cross and Square only for a rider whose set has the extra stunts (MD16).
            trick::Buttons { trick: c.trick, dpad: c.dpad, extra: if set.has_extras() { c.extra } else { [false; trick::EXTRA_BUTTONS] } },
            down,
        );

        // 4 — the request, then the idle for the frames with nothing asked for. The gate in front of
        // the idle is the original's: it only applies on the riding branch, and a wrecked rider's
        // idle runs unconditionally.
        let mut bike = trick::Bike {
            on_ground: ride.air.on_ground,
            contact: ride.touching,
            wrecked: state.wreck.is_some(),
            fresh: state.fresh,
            speed: ride.chassis.speed,
        };
        let params = trick::Params {
            overlap: p.animation.stunt_overlap_time,
            no_hander_speed: p.air.no_hander_wreck_speed_limit,
        };
        let before = tricks.in_progress;
        if bike.wrecked || !tricks.request(set, bike, params) {
            tricks.idle(&mut bike);
        }
        // The idle's own tail, whatever was asked for: a held rider shows the pose straight on
        // (S111). The fill above copied the same word the original's idle reads.
        if put_back.held {
            motion.freeze();
        }
        state.fresh = bike.fresh;
        if tricks.restart && !before {
            out.started = Some(tricks.requested);
        }

        // 5 — the clocks, and then the animation header on the same request. The original works the
        // restart test out once, at the top, and both halves read it, so the request is built before
        // the clocks run and only its progress is taken from after them.
        let request = motion::Request {
            action: tricks.requested,
            force: tricks.restart,
            seq: tricks.seq as u8,
            prev_seq: tricks.prev_seq,
            frozen: false,
            progress: tricks.progress,
        };
        let clocked = tricks.update(dt, set, bike, false, ride::noise);
        // The clocks' three calls on the rider, which score (S79). The wheels are this frame's, the
        // wreck is the one the rider was in before this frame's machine.
        let at_clocks = Self::score_sense(ride, slow, state.wreck.is_some());
        if clocked.stopped_before {
            score.finished();
        }
        if let Some(id) = clocked.started {
            score.start(id.0, &at_clocks);
        }
        if clocked.stopped {
            score.finished();
        }
        // MX vs. ATV's 360 come round this frame (MD16), scored flat under its own id.
        if let (Some(moves), Some(t)) = (p.tricks.as_ref(), ride.tricks.as_deref_mut()) {
            if std::mem::take(&mut t.spin.came_round) {
                let (id, points) = if t.spin.dir >= 1 {
                    (score::THREE_SIXTY_RIGHT, moves.three_sixty_points.1)
                } else {
                    (score::THREE_SIXTY_LEFT, moves.three_sixty_points.0)
                };
                score.start_flat(id, points);
                // Told as a start, so the shell names it: it has no clip of its own to show it.
                out.started = out.started.or(Some(if t.spin.dir >= 1 { Trick::THREE_SIXTY_RIGHT } else { Trick::THREE_SIXTY_LEFT }));
            }
        }
        let pose = motion::Pose {
            lean: ride.held.lean.0,
            fore_aft: ride.held.lean.1,
            crossup: ride.air.crossup,
        };
        let asked = motion::Request { progress: tricks.progress, ..request };
        *packed_pose = motion.pose;
        *packed_fore_aft = motion.fore_aft;
        *animation = motion.frame(dt, p.motion(), asked, pose, *clips);

        // The word next frame's airborne control reads. The clocks run after the ground step, so what
        // a frame's air code sees is always the previous frame's verdict — which is why this is a
        // field on the bike and not the button in `Controls`.
        ride.stunt = tricks.in_progress;
        if tricks.in_progress && *flight == Trick::NONE {
            *flight = tricks.current;
        }

        // 6 — the machine, carrying the wreck test.
        let (_, up) = ride.axes();
        // The pitch and roll the attitude limits are measured on are the Euler pair the frame's
        // attachment walk left behind, after the physics (`Ride::frame`'s step 4b, S138).
        let euler = ride.chassis.euler;
        let chassis = air::Chassis {
            speed: ride.chassis.speed,
            pitch: F(euler.x()),
            roll: F(euler.z()),
            vel: ride.chassis.vel,
            ..air::Chassis::default()
        };
        // Only three of the step's fields reach the wreck test: the up axis the no-hander's roll is
        // measured on, the trick-in-progress word, and whether anything is touching. The rest are the
        // airborne control's and are already spent.
        // MX vs. ATV's 360 (MD16) holds the airborne pitch and roll limits off from the moment it
        // turns until the wheels are down, as a backflip's `flipping` does; false on the original's.
        let spinning = ride.tricks.as_ref().is_some_and(|t| t.spin.ramp);
        let step = air::Step { up, stunt: tricks.in_progress, touching: ride.touching, spinning, ..air::Step::default() };
        let g = ride.mv.g_peak;
        let loads = air::Loads {
            side: F(g.x()),
            vertical: F(g.y()),
            fore_aft: F(g.z()),
            landing_crossup: motion.shown_crossup,
            no_hander: tricks.no_hander,
            racing: *racing,
            // The id the rider is in against the id the throw-away sits at — the id and not the slot,
            // which is the original's own comparison and the quirk `trick.rs` documents.
            pose: i32::from(tricks.current.0),
            pose_wanted: set.throwaway as i32,
            pose_blend: tricks.progress,
            // `air.rs` names it for the verdict it feeds; what the original reads there is the trick
            // machine's "this one can be landed now".
            trick_wrecked: tricks.safe,
        };
        // **The settle gate.** The body's touching flag, as the previous tick's step left it
        // ([`Down::settles`], read above). A rider down as one point has [`Crash`]'s inferred one.
        sense.settles = match gate {
            Some(touching) => touching,
            None => crash.as_ref().is_some_and(Crash::settles),
        };
        // The wreck test's last clause reads the collision record's contact list: what the bike's
        // body rays hit. The port's is its head ray, swept at the frame's end (`ride::hits`). The
        // chassis' mass is not held, only its reciprocal; it is read only for an actor, and the
        // port's world has none.
        // A ghost's head ray sweeps the world alone: its body ignores the vehicle class (S63).
        let rivals = if ghost.on { &[][..] } else { rivals };
        // The ground's entries are the list's first (P147), the rider's head among them.
        let mut hits = in_ground;
        if state.wreck.is_none() {
            hits.extend(ride::hits_among(ride, world, rivals));
            hits.extend(cars);
        }
        let touched = ride.touching || !hits.is_empty();
        let (mass, height) = (F::ONE / ride.chassis.inv_mass, F(ride.chassis.cofg_world.y()));
        // The map edge (S59): armed on the bike by its builder (S73), on a map with an edge, and not
        // for a rider already shot off it. Its two points are the rider's main bone and the chassis.
        let edge = world.area().filter(|_| *edge_armed && state.wreck != Some(Wreck::CANNONBALL));
        let bone = Self::main_bone(ride, ragdoll_kit.as_deref(), ragdoll.as_ref(), crash.as_ref());
        // A world of several terrains has holes inside its rectangle as well (MD8), asked the same.
        let holed = edge.is_some() && off_map::off_world(&*world, bone, ride.chassis.cofg_world);
        // A world that wraps (MD29) has seams, not an edge, on its wrapped axes.
        let wrap = world.wrap();
        // MX vs. ATV's 360 landed part way round (MD16): a wreck off the back, the original's 0xb.
        // Cut short in the air instead (S154), it is the airborne attitude's wreck, which Air Wrecks
        // Off does not ask: the landing is left to the wreck test.
        let (spun, spun_aloft) = ride.tricks.as_deref_mut().map_or((None, false), |t| {
            (std::mem::take(&mut t.spin.wrecked).then_some(air::Wreck::Tail), std::mem::take(&mut t.spin.cut))
        });
        let thrown = &mut ride.chassis;
        let origin = &mut ride.origin;
        let air = &mut ride.air;
        let drag_halved = &mut ride.drag_halved;
        let verdict = state.step(
            dt,
            sense,
            state::Params {
                max_wreck_time: p.wreck_times.max_wreck_time,
                cannonball_wreck_time: p.wreck_times.cannonball_wreck_time,
            },
            |allowed| {
                if !allowed.wrecks {
                    return None;
                }
                // The first clause, and the only one that runs for a rider already down: off the
                // map, the bike is thrown back and nothing else is asked.
                if let Some(area) = edge.filter(|a| holed || off_map::outside_wrapped(a, wrap, bone, thrown.cofg_world)) {
                    off_map::throw(&area, thrown, origin, ragdoll_ride::generator(idles));
                    *drag_halved = true;
                    return Some(Wreck::CANNONBALL);
                }
                if !allowed.landing {
                    return None;
                }
                // Both can enter a wreck on one substep, and the entry stores its reason without
                // looking, so the hit's — asked second — is the one that stays.
                let landed = air::wreck_test_in(&p.air, air, &chassis, &step, &loads, allowed.air);
                // Never a 50cc bike's lower limit: the original's flag is the bike's name containing
                // "Race50" or "Pro50" (`track::physics_file`'s), and the rider is not told which file
                // it was built from — nor does `ride::hits` sweep the two shoulder rays it would add.
                let hit = air::hit_wreck(&p.air, air, touched, false, mass, height, &hits);
                let spun = spun.filter(|_| allowed.air || !spun_aloft);
                hit.or(landed).or(spun).map(Wreck::from)
            },
            // The screen's page is up from the request until its fade back in is over.
            || page.up(),
        );
        if let Some(reason) = verdict.wreck {
            out.wreck = Some(reason);
            // The wreck entered on the bike's side: `Wreckbike.phy`'s body takes the bike's place,
            // its velocity and its spin. A rider with no wreck files has none of this and lies there
            // until the chord, which is where the port was before S26.
            // A cannonball's entry draws the wreck body's spin instead of copying the bike's, whether
            // or not this rider has a wreck body to give it to.
            let spin = (reason == Wreck::CANNONBALL)
                .then(|| off_map::entry_spin(ragdoll_ride::generator(idles), ride.chassis.ang_vel.w()));
            *crash = wreckage
                .as_ref()
                .map(|w| Crash::begin(ride, reason, &w.part, w.inertia, &w.wrecks, spin));
            // The body actor's wreck entry: the bones off the bike, then the wreck sound picked.
            // Both draw from the game's one generator, which is the idle pool's.
            *ragdoll = match (ragdoll_kit.as_ref(), wreckage.as_ref(), crash.as_ref()) {
                (Some(kit), Some(w), Some(down)) => {
                    let rng = ragdoll_ride::generator(idles);
                    // From the riding bodies when there are some (S110), else from the animated
                    // skeleton's nodes and their last drawn frame (S70).
                    let handed = riding.as_deref().filter(|_| !out.respawned).and_then(|r| r.hand_over(kit));
                    let (now, before) = match handed {
                        Some((now, before)) => (now, before),
                        None => {
                            let now = nodes.clone().unwrap_or_else(|| ragdoll_ride::nodes(kit, skeleton.as_deref(), &ride.transform()));
                            let before = (drawn_before.len() == now.len()).then_some(&drawn_before[..]);
                            (now, before)
                        }
                    };
                    let doll = Down::enter_from(
                        kit,
                        &now,
                        before,
                        dt,
                        down,
                        down.throw,
                        i32::from(reason.0),
                        &w.wrecks,
                        rng,
                        world,
                    );
                    wreck_sounds.wreck(i32::from(reason.0), false, rng);
                    Some(doll)
                }
                _ => None,
            };
            // The caller's half of the entry, as far as this port goes: the running trick stops and
            // the queue is emptied, so a trick asked for on the way down does not fire on the way
            // out. The rest of the original's entry — the rider placed at the bike's rider point with
            // its drop velocity, the wheel loads zeroed, the camera and the rumble — is not ported.
            tricks.reset();
            *flight = Trick::NONE;
            // The rider's own wreck entry: the trick and the combo both lost (S79).
            score.wrecked();
            if let Some(t) = ride.tricks.as_deref_mut() {
                t.spin = air::Spin::default();
                t.front_watch = 0;
            }
        } else if was_airborne && !ride.air.airborne {
            out.landed = (*flight != Trick::NONE).then_some(*flight);
            *flight = Trick::NONE;
        }
        if verdict.reset == Some(state::Reset::Now) {
            // The machine has done its own half; the bike's placement is the caller's. It goes back
            // onto the put-back record (S66), which the wreck body kept up to date while the rider
            // was down — **where the wreck ended up**, as [`Crash::rest`] measured — and which still
            // holds the last spot ridden over for a rider put down without a wreck body under them.
            ride.place_back();
            ride.unwreck();
            // The vehicle's unwreck doubles a cannonball's drag back; only a throw halves it.
            ride.drag_halved = false;
            *crash = None;
            *ragdoll_shown = ragdoll.take().map(Box::new);
            tricks.reset();
            *flight = Trick::NONE;
            out.respawned = true;
            ghost.start();
            put_back.place();
            score.aborted();
        }
        // 7 — the wreck page's fader, which updates after the bike: a request raises it and its
        // first update is this tick's, and the draw after its third puts the bike back
        // ([`wreck_page`]). The put-back is the bike's respawn, the same as the machine's own.
        if verdict.reset == Some(state::Reset::Page) {
            page.raise();
        }
        if page.update() {
            // Onto the put-back record (S66), as the machine's own put-back above. A bike the chord
            // put back in the fade's first ticks is set down again (S84): the page's listener is
            // still on the fader, and its respawn places the riding bike on the record as this
            // tick's physics kept it, ghost started over — measured live, chords on both ticks.
            state.respawn();
            ride.place_back();
            ride.unwreck();
            ride.drag_halved = false;
            *crash = None;
            // the same respawn as the machine's own: the body held two fills, the bones drawn once more
            *ragdoll_shown = ragdoll.take().map(Box::new);
            tricks.reset();
            *flight = Trick::NONE;
            out.respawned = true;
            ghost.start();
            put_back.place();
        }
        // The cannonball's record update (S77), which the race manager runs after the bike's whole
        // update: kept while the wreck bike touches, so the put-back lands where it came down. A
        // tick that put the bike back has placed it off the record as it stood, and keeps no more
        // (**not measured**: no captured put-back tick had the wreck bike touching).
        if let Some((over, at, forward)) = thrown_keep.filter(|_| !out.respawned) {
            ride.keep_back_thrown(over, at, forward, world, &t.ride.curve);
        }
        // The nodes' drawn matrices for the next tick's entry: none once the rider is down or put
        // back, where the next riding tick starts a fresh history.
        if state.wreck.is_none() && !out.respawned {
            if let Some(nodes) = nodes {
                *drawn = ragdoll_ride::drawn(&nodes);
            }
        } else {
            // The riding body too (S110): the next one is placed afresh off the pose.
            *riding = None;
        }
        // 8 — the points (S79): the rider's tick, on the frame's wheels and wreck, and then the
        // manager's, which in the original runs after every rider's and touches only this one's card.
        let sensed = Self::score_sense(ride, slow, state.wreck.is_some());
        match (p.tricks.as_ref(), ride.tricks.as_deref_mut()) {
            (Some(moves), Some(t)) => {
                let was = t.front_watch;
                score.frame_with(dt, &sensed, Some((&mut t.front_watch, moves.front_flip_points)));
                // A front flip come round, told as a start as the 360 is.
                if was != 3 && t.front_watch == 3 {
                    out.started = out.started.or(Some(Trick::FRONTFLIP));
                }
            }
            _ => score.frame(dt, &sensed),
        }
        score.grow(dt);
        out
    }

    /// What the points read off the bike (S79): its wheels, its air, and the chassis' up and
    /// forward axes' heights for the backflip watch.
    fn score_sense(ride: &Ride, slow: bool, wrecked: bool) -> score::Sense {
        let m = ride.transform();
        score::Sense {
            front: ride.front.w.contact,
            rear: ride.rear.w.contact,
            airborne: ride.air.airborne,
            hang_time: ride.air.hang_time,
            slow,
            wrecked,
            up_y: F(m.0[1].y()),
            forward_y: F(m.0[2].y()),
        }
    }

    /// **One frame of the rider's own animation** (S27): the body actor's selection, the step it
    /// hands its answer to, and the clip clock.
    ///
    /// Not a step of [`Rider::frame`] — the body is an actor of its own (see the module header), so
    /// this is a second entry point. Call it once a frame, after `frame`, from whoever holds the clip
    /// table: `lib` is the body's sixty-entry table ([`vehicle_anim::Library::load`] with the
    /// *rider's* prefix, `rider_` or `ff_rider_`, which is the one thing S27 corrected about S25's
    /// identity of that loader) and `clip` answers what a handle's clip is, which only the shell that
    /// loaded them knows.
    ///
    /// The step is told the body places itself, which skips the request's placement: a body posed by
    /// one of its own clips is drawn at the bike's transform and bolts nothing (V8's measurement), so
    /// there is no offset for this port to want.
    ///
    /// # The mapping is the port's own
    ///
    /// The original's publisher of the request block was not found, here or in S25, so which of the
    /// rider's fields each of the body's inputs comes off is this module's choice. All of it reads
    /// the same frame's latched trick id (`Tricks::requested`) rather than the running one, because
    /// that is the id the frame's own animation header plays. The ones that are not off the trick
    /// machine are the bike's own words. One row is not a choice: V35 captured the original's request
    /// through recording2's backflips, and its backflip word is the bike's turning flag on all 1100
    /// ticks.
    ///
    /// | the body asks | this answers with |
    /// |---|---|
    /// | the gesture slot, or 24 for none | the latched id's own slot ([`Trick::slot`]) |
    /// | a backflip | the bike is turning one over, [`crate::air::Air::flipping`] (V35: the original's, from a capture) — but not a front flip (P159) |
    /// | airborne | the bike's own verdict, [`crate::air::Air::airborne`] |
    /// | the no-hander | [`crate::trick::Tricks::no_hander`] |
    /// | a wreck | the ride state machine had a reason as of the request's fill, a tick behind its verdict ([`PutBack::wreck`], S91) |
    /// | the throttle, reverse | the controls, unfiltered |
    /// | the phase axis | [`Rider::packed_pose`]: [`motion::Driver::pose`] as the state pack latched it, in front of this tick's header (S112) |
    /// | the mix axis | [`Rider::packed_fore_aft`], [`motion::Driver::fore_aft`] latched the same way; [`rider_anim::FULL_FORWARD`] through a front flip (P159) |
    /// | the band the grid's rows are cut out of | the contact velocity's forward lane, [`crate::air::Air::contact_vel`]'s z (S112) |
    /// | the progress and the request count | the trick clocks' |
    ///
    /// # The body's other machine
    ///
    /// The body actor has a second update of its own — its *mode* machine, which decides what is
    /// posing the body and is the one writer of [`rider_anim::Body::settled`] — and this is its
    /// caller too (V19): [`rider_anim::Body::mode`] runs first, on the wreck in progress, and then
    /// the selection and the step. So a body this port wrecks now settles by itself, which is what
    /// lets go of the flail, and `settled` is no longer an input a shell has to write.
    ///
    /// The three sensors the settle test reads come off [`Crash`], and they are **the port's own
    /// stand-ins** for a ragdoll this crate has no port of: the original asks its bone array whether
    /// any bone is touching and reads the main bone's height and velocity, and this hands over
    /// [`Crash::landed`] and the rider point's own height and velocity. A rider who is down with no
    /// wreck files under them has no [`Crash`] at all, so nothing settles and nothing changes for
    /// them: they lie there until the hand chord, exactly as before.
    pub fn body_frame(
        &mut self,
        dt: F,
        c: &Controls,
        gate: Gate,
        lib: &vehicle_anim::Library,
        clip: &impl Fn(motion::ClipId) -> motion::Clip,
    ) -> vehicle_anim::Frame {
        let Rider {
            ride,
            tricks,
            body,
            body_numbers,
            body_rates,
            body_request,
            body_clocked,
            packed_pose,
            packed_fore_aft,
            idles,
            crash,
            wreckage,
            ragdoll,
            put_back,
            feet,
            ..
        } = self;
        // A front flip (MD16, P159) leans the rider forward rather than playing the backflip's clip.
        let front = ride.tricks.as_ref().is_some_and(|t| air::front_flipping(&ride.air, t.flip_row));
        let ask = rider_anim::Ask {
            // The wheels are off the ground. The id would nearly do instead — `Trick::WHEELIE` *is*
            // `motion.rs`'s airborne idle — but it is only set on a flight with no trick asked for,
            // and the body's airborne pose is what a stoppie in the air should show too. For a
            // gesture the choice makes no difference: the gesture arm runs in front of this one.
            airborne: ride.air.airborne || front,
            // The bike's own turning flag (V35): recording2's request carries it on every captured
            // tick, raised the tick the flip torque first goes in and dropped the tick the rotation is
            // over. The trick id never says so — it is the airborne idle's through the whole flip.
            // MD16's front flip turns through the same flag and has no clip in either game: it is not
            // a backflip here, and holds the airborne lean full forward instead (P159).
            backflip: ride.air.flipping && !front,
            no_hander: tricks.no_hander,
            // Down as the request last saw it, and only that: the fill sits before the machine's
            // verdict, so on a wreck's entry tick the body is still riding and enters its wreck mode
            // the tick after (S91), and on the tick the wreck clock puts the rider back it is still
            // in the wreck (S74).
            wreck: put_back.wreck,
            countdown: gate.countdown,
            throttle: c.gas,
            reversing: ride.held.reverse,
            gesture: tricks.requested.slot().map_or_else(|| pack_state(lib, tricks.requested), |s| s as i32),
            spin: spin_state(lib, ride),
        };
        // The mode machine, in front of the animation: in the original the two are different slots
        // of the body actor's vtable, and the wreck mode is what writes `settled` for the selection
        // below to read.
        // The sensors are the bones' when the ride runs a ragdoll (S53), with its clocks.
        let thrown = match ragdoll.as_ref() {
            Some(doll) => Some(doll.sensed),
            None => crash.as_ref().map(|down| rider_anim::Thrown {
                throw: down.throw,
                touching: down.landed,
                vel: down.rider_vel,
                clearance: down.clearance(),
                launch: None,
            }),
        };
        let wrecks = wreckage.as_ref().map_or_else(phy::Wrecks::default, |w| w.wrecks);
        // Every field the rider derives. `cut` and `offset` are left as they are: the body's own
        // selection is the only writer of the first and nothing here wants the second.
        body_request.held = put_back.held;
        body_request.at_base = false;
        body_request.base = ride.position();
        body_request.rotation = ride.chassis.quat;
        // The block the body reads is the riding request (S111/S112): its axes are the pose and the
        // fore-aft the state pack latched before this tick's header, and its band is the contact
        // velocity's forward lane, all three copied by the request's fill.
        body_request.phase_axis = *packed_pose;
        body_request.mix_axis = *packed_fore_aft;
        body_request.band = F(ride.air.contact_vel.z());
        body_request.progress = tricks.progress;
        body_request.seq = tricks.seq;
        if front {
            body.hold_forward(body_request);
        }
        body.mode(dt, body_request, &ask, &wrecks, thrown, idles, gate.clock);
        // A ragdoll throws its own damping switch, in its settle test (S61, `Down::step`).
        let (frame, clocked) = body.frame_clocked(
            dt,
            body_numbers,
            body_rates,
            body_request,
            &ask,
            lib,
            idles,
            gate.clock,
            true,
            clip,
        );
        *body_clocked = clocked;
        // The foot switch runs where the body actor runs it (V36): on every tick that is neither a
        // wreck nor a pose off the clip table, off what the shell's riding request hands the ragdoll.
        if body.machine.mode != rider_anim::Mode::Wrecked && body.machine.mode != rider_anim::Mode::Posed {
            feet.step(dt, &crate::foot_down::Input::of(ride, &body.veh));
        }
        frame
    }
}

/// **MD21:** the state a trick pack's extra plays in, past the original's sixty — or no gesture, the
/// airborne lean MD16 showed, when the pack has no clip for it (or the trick is not an extra).
fn pack_state(lib: &vehicle_anim::Library, trick: Trick) -> i32 {
    trick
        .extra()
        .map(|k| vehicle_anim::PACK_EXTRA + k as i32)
        .filter(|&state| lib.clip(state).is_some())
        .unwrap_or(trick::SLOTS as i32)
}

/// **MD21:** MX vs. ATV's 360 clip, from the frame the bike turns past [`air::SPIN_CLIP`] until it
/// comes round or lands: `threesixtyR` for a 360 flicked left then right, `threesixtyL` the other way.
/// `None` when no 360 is past its angle, or the pack has no clip for this one.
fn spin_state(lib: &vehicle_anim::Library, ride: &Ride) -> Option<i32> {
    let spin = &ride.tricks.as_ref()?.spin;
    let state = if spin.dir >= 1 { vehicle_anim::SPIN_RIGHT } else { vehicle_anim::SPIN_LEFT };
    (spin.clip != 0 && lib.clip(state).is_some()).then_some(state)
}

impl State for Rider {
    /// The six fields the simulation runs on, by name. The animation is **not** walked — neither the
    /// header's frame (V8) nor the body and its pool (S27): nothing the rider simulates reads any of
    /// them, and hashing them would move every replay hash in the crate for a value that only the
    /// renderer looks at.
    fn walk(&self, w: &mut Walk<'_>) {
        w.group("ride", |w| self.ride.walk(w));
        w.group("tricks", |w| hash::State::walk(&self.tricks, w));
        w.group("state", |w| hash::State::walk(&self.state, w));
        w.group("motion", |w| hash::State::walk(&self.motion, w));
        w.u8("flight", self.flight.0);
        w.bool("airborne", self.airborne);
        // The wreck in progress, when there is one: a rider who is up hashes as one who has never
        // been down, which is what makes a respawned rider comparable with a fresh one.
        w.bool("crashing", self.crash.is_some());
        if let Some(down) = &self.crash {
            w.group("crash", |w| hash::State::walk(down, w));
        }
        // The bones, when the ride runs them (S53). A rider without a kit hashes as before.
        if let Some(doll) = &self.ragdoll {
            w.group("ragdoll", |w| hash::State::walk(doll, w));
        }
        // The riding ragdoll (S110), while a shell hands the rider its holds: a rider with none (every
        // online one, and every fixture) hashes as before.
        if let Some(r) = &self.riding {
            w.group("riding", |w| hash::State::walk(&**r, w));
        }
        // The page, while one is up (S72): a rider with none hashes as before.
        if self.page.up() {
            w.group("page", |w| hash::State::walk(&self.page, w));
        }
        // The wreck sounds' picks move only for a rider that runs a ragdoll (S53).
        if self.ragdoll_kit.is_some() {
            w.group("wreck_sounds", |w| {
                w.i32("crash_last", self.wreck_sounds.crash.last);
                w.i32("fall_last", self.wreck_sounds.fall.last);
            });
        }
        // The points (S79), once anything has scored or started to: a rider who never has hashes as
        // before, as a rider who is up hashes as one who has never been down.
        if self.score != Score::default() {
            w.group("score", |w| hash::State::walk(&self.score, w));
        }
    }
}
