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
//! # What is this module's own and not a port
//!
//! Only the joining. Every number comes off the disc through [`Phy`] and the stunt table; every
//! decision is made inside one of the four ported modules. The three places the wiring had to choose
//! are marked **the port's own** where they are written, and all three are in the journal.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::air;
use crate::hash::{self, State, Walk};
use crate::motion;
use crate::phy::Phy;
use crate::ride::{self, Controls, Ride};
use crate::state;
use crate::trick::{self, AngleTable, Set, Trick, Tricks, Wreck};
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
    /// The rider was put back on the bike this frame.
    pub respawned: bool,
}

/// A bike with somebody on it.
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
    /// What the animation driving asked for this frame: the one thing [`motion::Driver::frame`]
    /// answers, kept rather than dropped.
    ///
    /// The rider itself reads nothing out of it — the only field of the driving it needs is the
    /// crossup the header writes ([`motion::Driver::shown_crossup`]) — but a shell that draws the
    /// rider needs the blend the selection asked for, and running a second driver beside this one
    /// to get it would be two state machines where the original has one. Left at
    /// [`motion::Blend::Free`] until the first frame.
    pub animation: motion::Frame,

    /// The last trick asked for since the wheels left the ground, for [`Frame::landed`].
    flight: Trick,
    /// Whether the bike was off the ground at the end of the previous frame, which is the edge
    /// [`Frame::landed`] is taken on — [`crate::air::Air::was_airborne`] is a *substep*'s memory and
    /// says nothing about the frame before.
    airborne: bool,
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
            // A local human, freestyle, wrecks on. `settles` is false because a bike's class never
            // asks to settle (S5), which is why the automatic reset below is out of reach and the
            // rider's own chord is the way back.
            sense: state::Sense {
                ai_route: false,
                owner: Some(state::Owner { human: true, screen: false }),
                wrecks_enabled: true,
                settles: false,
            },
            racing: false,
            animation: motion::Frame { blend: motion::Blend::Free, restart: false },
            flight: Trick::NONE,
            airborne: false,
        }
    }

    /// Put the rider and the bike back at a place, as a respawn does.
    pub fn place(&mut self, at: Vf, facing: Vf) {
        self.ride.place(at, facing);
        self.tricks.reset();
        self.flight = Trick::NONE;
        self.airborne = false;
    }

    /// Which of the three states the rider is in.
    pub fn phase(&self) -> state::Phase {
        self.state.phase()
    }

    /// **One frame of the rider**, in the original's order. The six steps at the top of this file.
    pub fn frame(&mut self, p: &Phy, t: &Tables, c: &Controls, dt: F, world: &mut dyn World) -> Frame {
        let mut out = Frame::default();
        let Rider {
            ride, tricks, state, motion, set, clips, sense, racing, animation, flight, airborne,
        } = self;
        let sense = *sense;

        // 1 — the top of the player's update: the hand-reset latch is dropped.
        state.frame_start();

        // 2 — the physics, only while the rider is not down. The respawn is the machine's to order,
        // so the bike's own chord is taken out of the controls it is handed here and read below.
        let was_airborne = *airborne;
        if state.wreck.is_none() {
            ride.frame(p, &t.ride, &Controls { respawn: false, ..*c }, dt, world);
        }
        *airborne = ride.air.airborne;

        // 3 — the controller. **The port's own ordering:** the original reads the pad in its input
        // phase, before the update, and what it reads the stick off is the rate-limited copy that
        // phase has just written. The rate limit lives inside `Ride::frame`, so the read is here
        // instead — after the step that writes the stick and before the only three things that look
        // at a gesture, which is the same frame's worth of information either way.
        let down = state.wreck.is_some();
        tricks.gesture.read(
            &t.angles,
            ride.held.stick,
            trick::Buttons { trick: c.trick, dpad: c.dpad },
            down,
        );
        if c.respawn && state.chord_allowed(sense.ai_route) {
            state.take_chord();
            if state.request_reset(state::Kind::Wrecked, false, sense.owner) == Some(state::Reset::Now) {
                state.respawn();
                let (at, facing) = (ride.spawn, ride.spawn_facing);
                ride.place(at, facing);
                tricks.reset();
                *flight = Trick::NONE;
                out.respawned = true;
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
            lean: ride.held.stick.0,
            fore_aft: ride.held.stick.1,
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
        let air = &mut ride.air;
        let verdict = state.step(
            dt,
            sense,
            state::Params {
                max_wreck_time: p.wreck_times.max_wreck_time,
                cannonball_wreck_time: p.wreck_times.cannonball_wreck_time,
            },
            |allowed| {
                if !allowed.wrecks || !allowed.landing {
                    return None;
                }
                air::wreck_test(&p.air, air, &chassis, &step, &loads).map(Wreck::from)
            },
            // No wreck page: this shell has no front end to put one on.
            || false,
        );
        if let Some(reason) = verdict.wreck {
            out.wreck = Some(reason);
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
            // The machine has done its own half; the bike's placement is the caller's.
            let (at, facing) = (ride.spawn, ride.spawn_facing);
            ride.place(at, facing);
            tricks.reset();
            *flight = Trick::NONE;
            out.respawned = true;
        }
        out
    }
}

impl State for Rider {
    fn walk(&self, w: &mut Walk<'_>) {
        w.group("ride", |w| self.ride.walk(w));
        w.group("tricks", |w| hash::State::walk(&self.tricks, w));
        w.group("state", |w| hash::State::walk(&self.state, w));
        w.group("motion", |w| hash::State::walk(&self.motion, w));
        w.u8("flight", self.flight.0);
        w.bool("airborne", self.airborne);
    }
}
