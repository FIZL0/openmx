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
use crate::ragdoll_ride::{self, Down, Kit};
use crate::rider_anim;
use crate::state;
use crate::wreck_sound;
use crate::ghost::Ghost;
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
    /// The ragdoll of the wreck being ridden out (S53): the rider's bones, thrown off the bike and
    /// stepped while they are down. `None` when the rider is up, and for a rider with no kit or no
    /// [`Wreckage`], who is down as one point of [`Crash`]'s as before.
    pub ragdoll: Option<Down>,
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
    /// The request block the body's selection and step read, as this module publishes it.
    /// [`Rider::body_frame`] rewrites every field it derives from the rider each frame and leaves the
    /// other two (`cut`, `offset`) alone, because the body itself is the only thing here that writes
    /// them. [`Rider::new`] raises `cut`: a rider is put on the bike at the gate.
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
}

impl Rider {
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
            set,
            clips: motion::Clips::default(),
            // A local human, freestyle, wrecks on. `settles` starts false and is written every frame
            // from the wreck in progress ([`Crash::settles`]) once a shell has handed over the two
            // wreck files; a rider with none keeps it false, and for that one the automatic reset is
            // out of reach and the chord is the way back.
            sense: state::Sense {
                ai_route: false,
                owner: Some(state::Owner { human: true, screen: false }),
                wrecks_enabled: true,
                settles: false,
            },
            racing: false,
            wreckage: None,
            crash: None,
            ragdoll_kit: None,
            ragdoll: None,
            wreck_sounds: wreck_sound::Emitter::retail(),
            animation: motion::Frame { blend: motion::Blend::Free, restart: false },
            body: rider_anim::Body::default(),
            body_numbers: rider_anim::Params::built(),
            body_rates: vehicle_anim::Params::default(),
            idles: rider_anim::Pool::default(),
            body_clocked: rider_anim::Clocked::default(),
            // At the gate: the cut flag is what the body's selection takes its get-ready sequence
            // off, and the body is the only thing in the port that ever clears it (the throttle
            // opening). **The port's own** — in the original the flag is published by whatever owns
            // the request block, which is not identified.
            body_request: vehicle_anim::Request { cut: true, ..vehicle_anim::Request::default() },
            flight: Trick::NONE,
            airborne: false,
            ghost: Ghost::default(),
        }
    }

    /// Put the rider and the bike back at a place, as a respawn does.
    pub fn place(&mut self, at: Vf, facing: Vf) {
        self.ride.place(at, facing);
        self.crash = None;
        self.ragdoll = None;
        self.tricks.reset();
        self.flight = Trick::NONE;
        self.airborne = false;
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
        let mut out = Frame::default();
        let Rider {
            ride, tricks, state, motion, set, clips, sense, racing, wreckage, crash, animation,
            flight, airborne, ragdoll_kit, ragdoll, wreck_sounds, idles, ghost, ..
        } = self;
        let mut sense = *sense;

        // 0 — the ghost's tick, the first thing in the bike's update: its clock holds while the
        // body is inside another rider's (`rivals` holds no ghosts, whose lists reject this one).
        ghost.step(dt, || crate::ghost::overlaps(&ride.rival(), rivals));

        // 1 — the top of the player's update: the hand-reset latch is dropped.
        state.frame_start();

        // 2 — the physics, only while the rider is not down. The respawn is the machine's to order,
        // so the bike's own chord is taken out of the controls it is handed here and read below.
        let was_airborne = *airborne;
        if state.wreck.is_none() {
            ride.frame(p, &t.ride, &Controls { respawn: false, ..*c }, dt, world);
        } else if let (Some(w), Some(down)) = (wreckage.as_ref(), crash.as_mut()) {
            // The physics of a rider who is down: the wreck body tumbling and the rider coming off
            // it. The original's is the same swap — the bike's own step is skipped while the reason
            // is set, and the wreck bike and the ragdoll are stepped instead.
            let held = down.holding;
            down.step(dt, &w.part, &w.wrecks, world);
            out.separated = held && !down.holding;
            // The rider's bones, when there are any: they let go of the bars on their own clock.
            if let (Some(kit), Some(doll)) = (ragdoll_kit.as_ref(), ragdoll.as_mut()) {
                let held = doll.ragdoll.holds[0] || doll.ragdoll.holds[1];
                doll.step(kit, dt, down, world);
                out.separated = held && !(doll.ragdoll.holds[0] || doll.ragdoll.holds[1]);
            }
        }
        *airborne = ride.air.airborne;

        // 3 — the controller. **The port's own ordering:** the original reads the pad in its input
        // phase, before the update; the read is here instead, before the only three things that look
        // at a gesture, which is the same frame's worth of information either way. What it reads is
        // the player-range stick as the pad gave it — not the rate limiter's squared copy, which is
        // written after the read (P30).
        let down = state.wreck.is_some();
        tricks.gesture.read(
            &t.angles,
            c.stick,
            trick::Buttons { trick: c.trick, dpad: c.dpad },
            down,
        );
        if c.respawn && state.chord_allowed(sense.ai_route) {
            state.take_chord();
            if state.request_reset(state::Kind::Wrecked, false, sense.owner) == Some(state::Reset::Now) {
                state.respawn();
                let (at, facing) = match crash.as_ref() {
                    Some(down) => down.rest(),
                    None => (ride.spawn, ride.spawn_facing),
                };
                ride.place(at, facing);
                off_map::unwreck(&mut ride.chassis);
                *crash = None;
                *ragdoll = None;
                tricks.reset();
                *flight = Trick::NONE;
                out.respawned = true;
                ghost.start();
            }
        }

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
        tricks.update(dt, set, bike, false, ride::noise);
        let pose = motion::Pose {
            lean: ride.held.lean.0,
            fore_aft: ride.held.lean.1,
            crossup: ride.air.crossup,
        };
        let asked = motion::Request { progress: tricks.progress, ..request };
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
        // The pitch and roll the attitude limits are measured on are the Euler pair the frame's own
        // measure left behind, read off the chassis the same way `Ride::frame` reads them for the
        // airborne control.
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
        let step = air::Step { up, stunt: tricks.in_progress, touching: ride.touching, ..air::Step::default() };
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
        // **The settle gate.** The machine's one input the port could not fill in before: the rider,
        // having let go of the bars, is on the ground. See [`Crash`] for what was measured.
        sense.settles = match ragdoll.as_ref() {
            Some(doll) => doll.settles(),
            None => crash.as_ref().is_some_and(Crash::settles),
        };
        // The wreck test's last clause reads the collision record's contact list: what the bike's
        // body rays hit. The port's is its head ray, swept at the frame's end (`ride::hits`). The
        // chassis' mass is not held, only its reciprocal; it is read only for an actor, and the
        // port's world has none.
        // A ghost's head ray sweeps the world alone: its body ignores the vehicle class (S63).
        let rivals = if ghost.on { &[][..] } else { rivals };
        let hits = if state.wreck.is_none() { ride::hits_among(ride, world, rivals) } else { Vec::new() };
        let touched = ride.touching || !hits.is_empty();
        let (mass, height) = (F::ONE / ride.chassis.inv_mass, F(ride.chassis.cofg_world.y()));
        // The map edge (S59): armed on a player's bike, on a map with an edge, and not for a rider
        // already shot off it. Its two points are the rider's main bone and the chassis.
        let edge = world
            .area()
            .filter(|_| sense.owner.is_some_and(|o| o.human) && state.wreck != Some(Wreck::CANNONBALL));
        let bone = Self::main_bone(ride, ragdoll_kit.as_deref(), ragdoll.as_ref(), crash.as_ref());
        let thrown = &mut ride.chassis;
        let origin = &mut ride.origin;
        let air = &mut ride.air;
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
                if let Some(area) = edge.filter(|a| off_map::outside(a, bone, thrown.cofg_world)) {
                    off_map::throw(&area, thrown, origin, ragdoll_ride::generator(idles));
                    return Some(Wreck::CANNONBALL);
                }
                if !allowed.landing {
                    return None;
                }
                // Both can enter a wreck on one substep, and the entry stores its reason without
                // looking, so the hit's — asked second — is the one that stays.
                let landed = air::wreck_test(&p.air, air, &chassis, &step, &loads);
                // Never a 50cc bike's lower limit: the original's flag is the bike's name containing
                // "Race50" or "Pro50" (`track::physics_file`'s), and the rider is not told which file
                // it was built from — nor does `ride::hits` sweep the two shoulder rays it would add.
                let hit = air::hit_wreck(&p.air, air, touched, false, mass, height, &hits);
                hit.or(landed).map(Wreck::from)
            },
            // No wreck page: this shell has no front end to put one on.
            || false,
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
                    let m = ride.transform();
                    let doll = Down::enter(kit, &m, down, down.throw, i32::from(reason.0), &w.wrecks, rng, world);
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
        } else if was_airborne && !ride.air.airborne {
            out.landed = (*flight != Trick::NONE).then_some(*flight);
            *flight = Trick::NONE;
        }
        if verdict.reset == Some(state::Reset::Now) {
            // The machine has done its own half; the bike's placement is the caller's. It goes back
            // to **where the wreck ended up** and not to the spawn — see [`Crash::rest`], which is
            // measured — and to the spawn only for a rider who was put down without a wreck body
            // under them (no wreck files, or [`crate::state::Wreck::NO_CONTROL`] before the first).
            let (at, facing) = match crash.as_ref() {
                Some(down) => down.rest(),
                None => (ride.spawn, ride.spawn_facing),
            };
            ride.place(at, facing);
            off_map::unwreck(&mut ride.chassis);
            *crash = None;
            *ragdoll = None;
            tricks.reset();
            *flight = Trick::NONE;
            out.respawned = true;
            ghost.start();
        }
        out
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
    /// that is the id the frame's own animation header plays. The two that are not off the trick
    /// machine are the bike's own words:
    ///
    /// | the body asks | this answers with |
    /// |---|---|
    /// | the gesture slot, or 24 for none | the latched id's own slot ([`Trick::slot`]) |
    /// | a backflip | the latched id is [`Trick::BACKFLIP`] |
    /// | airborne | the bike's own verdict, [`crate::air::Air::airborne`] |
    /// | the no-hander | [`crate::trick::Tricks::no_hander`] |
    /// | a wreck | the ride state machine has a reason |
    /// | the throttle, reverse | the controls, unfiltered |
    /// | the phase axis | [`motion::Driver::pose`] — lean on the ground, crossup in the air, 0..1 |
    /// | the mix axis | [`motion::Driver::fore_aft`], the same 0..1 the airborne pair is mixed with |
    /// | the band the grid's rows are cut out of | the chassis's speed |
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
            state,
            motion,
            body,
            body_numbers,
            body_rates,
            body_request,
            body_clocked,
            idles,
            crash,
            wreckage,
            ragdoll,
            ..
        } = self;
        let ask = rider_anim::Ask {
            // The wheels are off the ground. The id would nearly do instead — `Trick::WHEELIE` *is*
            // `motion.rs`'s airborne idle — but it is only set on a flight with no trick asked for,
            // and the body's airborne pose is what a stoppie in the air should show too. For a
            // gesture the choice makes no difference: the gesture arm runs in front of this one.
            airborne: ride.air.airborne,
            backflip: tricks.requested == Trick::BACKFLIP,
            no_hander: tricks.no_hander,
            wreck: state.wreck.is_some(),
            countdown: gate.countdown,
            throttle: c.gas,
            reversing: c.reverse,
            gesture: tricks.requested.slot().map_or(trick::SLOTS as i32, |s| s as i32),
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
        body_request.held = false;
        body_request.at_base = false;
        body_request.base = ride.position();
        body_request.rotation = ride.chassis.quat;
        body_request.phase_axis = motion.pose;
        body_request.mix_axis = motion.fore_aft;
        body_request.band = ride.chassis.speed;
        body_request.progress = tricks.progress;
        body_request.seq = tricks.seq;
        body.mode(dt, body_request, &ask, &wrecks, thrown, idles, gate.clock);
        // The bones' damping switch: the settle test's stiff flag, handed to the ragdoll when it
        // changes, as the original's mode machine does.
        if let Some(doll) = ragdoll.as_mut() {
            if body.machine.stiff != doll.ragdoll.stiff {
                doll.ragdoll.damping(body.machine.stiff, doll.reason);
            }
        }
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
        frame
    }
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
        // The wreck sounds' picks move only for a rider that runs a ragdoll (S53).
        if self.ragdoll_kit.is_some() {
            w.group("wreck_sounds", |w| {
                w.i32("crash_last", self.wreck_sounds.crash.last);
                w.i32("fall_last", self.wreck_sounds.fall.last);
            });
        }
    }
}
