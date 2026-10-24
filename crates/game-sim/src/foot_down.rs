//! **The rider's feet off the pegs** (V36): a stopped bike's rider stands with a foot down, a slow
//! one leaning into a turn puts the inside foot out.
//!
//! Neither is a clip of its own. The pose grid's stopped row, `turn_stop_*`, has both feet a foot
//! below the pegs and planted wide, and the riding row's `turn_riding_*` swings one foot out forward
//! at either end of its sweep; the clip machine already blends them by speed
//! ([`crate::vehicle_anim`], the `TURN_RIDING` arm). What decides whether the drawn rider *shows*
//! them is the riding ragdoll's two foot holds: while a foot is held it is pinned to its peg whatever
//! the clip says, and the original lets go of it here. That switch is the body actor's
//! ([`crate::ragdoll_body::Actor::feet`], proved tick by tick against the original in S109's
//! `tests/ragdoll_riding.rs`), and this is the same switch on its own, with no ragdoll under it, for
//! the drawn clip that is held on by `game-render`'s stand-in rather than by the ragdoll's pins.
//!
//! - Under the stand speed (the clutch's idle speed, 7.3333 ft/s), rolling backwards, or tucked,
//!   and not yawing at 1.5 rad/s or more nor looking round: **both feet let go**.
//! - Otherwise, in the riding grid under the slow speed (73.333 ft/s) with the bike tipped past
//!   `up.y < 0.9` and the grid's mix past 0.7 (or under 0.3): **one foot let go**, the other held at
//!   full weight, and the lifted one's fade restarted at two seconds.
//! - Otherwise both are held, each fading back in from wherever its fade was left: from 0.5 s after
//!   a stop (the weight starts back at 0.75) or 2 s after a lift (at 0), to one.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::math;
use crate::ragdoll_body::{Request, ramp};
use crate::ragdoll_riding::{SLOW_SPEED, STAND_SPEED};
use crate::ride::Ride;
use crate::vehicle_anim::{Driver, TURN_RIDING};
use ps2_float::{F, Vf, fpu};

/// How long a foot's hold takes to fade back in once a lift lets it go, in seconds.
const FOOT_FADE: F = F(2.0);

/// What the fade is left at when a stop lets both feet go: they come back from three quarters.
const STOP_FADE: F = F(0.5);

/// The yaw rate at which a slow bike keeps its feet on, in radians a second.
const TURNING: F = F(1.5);

/// The bike's up axis is tipped far enough for a foot to come off when its `y` is under this.
const TIPPED: F = F(0.9);

/// The grid's mix past which the right foot comes off, and under which the left one does.
const MIX_RIGHT: F = F(0.7);
const MIX_LEFT: F = F(0.3);

/// What the switch reads, each tick: the riding request's fields it looks at.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    /// The look-around is wanted whatever the speed.
    pub looking: bool,
    /// The look-around is off.
    pub tucked: bool,
    /// The bike's forward speed, in feet a second.
    pub speed: F,
    /// The bike's yaw rate (`rates.y`).
    pub yaw: F,
    /// The vehicle animation's state; only the riding grid lifts one foot.
    pub clip: i32,
    /// The grid's fore-and-aft mix (the request's `pitch`).
    pub mix: F,
    /// The bike's orientation.
    pub quat: Vf,
}

impl Input {
    /// What the shell's riding request hands the switch ([`crate::ragdoll_riding::request`]): the
    /// ride's forward speed, its rates and orientation, and the vehicle animation's state and mix.
    pub fn of(ride: &Ride, veh: &Driver) -> Input {
        let c = &ride.chassis;
        Input {
            speed: F(c.local_vel.z()),
            yaw: F(c.ang_vel.y()),
            clip: veh.state,
            mix: veh.mix.value,
            quat: c.quat,
            ..Input::default()
        }
    }

    /// The same fields read off a whole request, for a caller that has one.
    pub fn of_request(r: &Request, clip: i32) -> Input {
        Input {
            looking: r.looking,
            tucked: r.tucked,
            speed: r.speed,
            yaw: F(r.rates.y()),
            clip,
            mix: r.pitch,
            quat: r.quat,
        }
    }
}

/// **The two feet's holds**: left, then right.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Feet {
    /// The foot is held to its peg.
    pub held: [bool; 2],
    /// The time left on each foot's fade back in.
    pub fade: [F; 2],
    /// The weight each foot's hold is at (the pin's gain).
    pub weight: [F; 2],
}

impl Default for Feet {
    /// As the riding entry leaves them: both held at full weight, nothing fading.
    fn default() -> Feet {
        Feet { held: [true; 2], fade: [F::ZERO; 2], weight: [F::ONE; 2] }
    }
}

impl Feet {
    /// One tick of the switch, `dt` seconds after the last.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn step(&mut self, dt: F, i: &Input) {
        let (_, up) = math::quat_axes(i.quat);
        let on = i.looking
            || (!i.tucked && !(i.speed < F::ZERO) && (!(F(fpu::abs(i.yaw.0)) < TURNING) || !(i.speed < STAND_SPEED)));
        if !on {
            self.held = [false; 2];
            self.fade = [STOP_FADE; 2];
            return;
        }
        let slow = i.clip == TURN_RIDING && i.speed < SLOW_SPEED;
        if slow && !(i.mix <= MIX_RIGHT) && up[1] < TIPPED {
            self.lift(1, 0);
        } else if slow && i.mix < MIX_LEFT && up[1] < TIPPED {
            self.lift(0, 1);
        } else {
            self.held = [true; 2];
            for k in 0..2 {
                if !(self.fade[k] <= F::ZERO) {
                    self.fade[k] = self.fade[k] - dt;
                    self.weight[k] = ramp(self.fade[k], F::ZERO, FOOT_FADE, F::ONE, F::ZERO);
                }
            }
        }
    }

    fn lift(&mut self, off: usize, on: usize) {
        self.held[off] = false;
        self.fade[off] = FOOT_FADE;
        self.held[on] = true;
        self.weight[on] = F::ONE;
    }

    /// How hard each foot is held to its peg: its weight while held, nothing once let go.
    pub fn strength(&self) -> [f32; 2] {
        [0, 1].map(|k| if self.held[k] { self.weight[k].0 } else { 0.0 })
    }

    /// Whether a foot is off its peg.
    pub fn down(&self) -> bool {
        !self.held[0] || !self.held[1]
    }
}
