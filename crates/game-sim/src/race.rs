//! The race activities (S13): the supercross and nationals races, their start gate, laps and results.
//!
//! The first piece is the gate itself (S13a). A race track's gates — the start gate the riders wait
//! behind, and the lines the activity counts crossings of — are all one kind of object: a rectangle
//! standing on the track, with a centre, the direction a rider is meant to pass it in, the direction
//! across it, a width and a height. [`Gate::crossed`] is the test every one of them uses: given where an
//! entity was last tick and where it is now, did that step pass through the rectangle, beside it, or not
//! reach it at all. The original calls it from a dozen places (the race manager's start gate, the score
//! cards' lines, the stunt and checkpoint activities); this is the one function behind all of them.
//!
//! While the start gate is closed, a rider that rolls through it early is thrown back behind it and
//! loses the gas for a moment: [`gate_push_back`] and [`gate_penalty`] are the two draws that decide by
//! how much, in the order the race manager's tick makes them. The penalty is what the score card counts
//! down ([`crate::score::Score::gate_penalty`]).
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::rng::Rng;
use ps2_float::{Dest, F, Vf, fpu};

/// The gate's flag bits (its `+0x30` word).
pub mod flags {
    /// The gate is live. A start gate holds this while it is closed; with it clear, every test answers
    /// [`super::Crossing::Open`].
    pub const CLOSED: u32 = 0x01;
    /// A step that passes through the rectangle counts whichever way it went. Without it, only a step
    /// along the gate's direction does.
    pub const EITHER_WAY: u32 = 0x04;
    /// Only the half of the rectangle on the positive side of the across direction is the gate; a step
    /// through the other half is beside it.
    pub const ONE_SIDE: u32 = 0x08;
    /// The gate is a whole plane: any step that crosses it counts, wherever, with no width or height.
    pub const PLANE: u32 = 0x10;
}

/// One gate, as the crossing test reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gate {
    /// [`flags`].
    pub flags: u32,
    /// The point in the middle of the gate's bottom edge (`+0x40`).
    pub centre: Vf,
    /// The direction a rider passes the gate in (`+0x50`); a unit vector on every gate seen.
    pub normal: Vf,
    /// The direction across the gate (`+0x60`), the width measured along it.
    pub across: Vf,
    /// The gate's whole width (`+0x70`). Zero or less: nothing but a plane gate ever crosses it.
    pub width: F,
    /// How high above its centre the gate reaches (`+0x74`).
    pub height: F,
    /// The squared distance from the centre within which an entity is tested at all (`+0x7c`); zero or
    /// less tests at any distance.
    pub reach_sq: F,
}

/// What [`Gate::crossed`] answers, as the original's numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Crossing {
    /// The step did not cross the gate's plane, was too far away, or did not move (0).
    Not = 0,
    /// The step passed through the gate (1).
    Through = 1,
    /// The step crossed the gate's plane outside the rectangle — beside it, or above it (2).
    Beside = 2,
    /// The gate is not live ([`flags::CLOSED`] clear) (4).
    Open = 4,
}

/// The FPU's three-term dot product: `mula` x, `madda` y, `madd` z.
fn dot3(a: Vf, b: Vf) -> F {
    F(fpu::madd(fpu::madda(fpu::mula(a.x(), b.x()), a.y(), b.y()), a.z(), b.z()))
}

/// A word as the FPU reads it out of memory (denormals flushed), for a field compared straight from the
/// gate object.
fn read(x: F) -> F {
    F(fpu::operand(x.0.to_bits()))
}

impl Gate {
    /// Did the step from `from` to `to` cross the gate?
    ///
    /// In order: a gate that is not live is [`Crossing::Open`]; an entity starting further from the
    /// centre than the reach is [`Crossing::Not`]; so is one that did not move. The step's crossing of
    /// the gate's plane is the fraction `t` of the way back from `to` to `from` at which it meets it,
    /// and a step that does not cross (`t` outside 0..1) is [`Crossing::Not`]. A [`flags::PLANE`] gate
    /// stops there. Otherwise the meeting point is placed across the gate, as a fraction of its half
    /// width, and above it: outside either is [`Crossing::Beside`]. Through the rectangle, a step
    /// going the gate's way — or either way with [`flags::EITHER_WAY`] — is [`Crossing::Through`], and
    /// one going the other way is [`Crossing::Not`].
    pub fn crossed(&self, from: Vf, to: Vf) -> Crossing {
        if self.flags & flags::CLOSED == 0 {
            return Crossing::Open;
        }
        let reach_sq = read(self.reach_sq);
        if F::ZERO < reach_sq {
            let off = self.centre.sub(from);
            if !(dot3(off, off) <= reach_sq) {
                return Crossing::Not;
            }
        }
        let step = from.sub(to);
        let length = F(fpu::sqrt(dot3(step, step).0));
        if length <= F::ZERO {
            return Crossing::Not;
        }
        let back = self.centre.sub(to);
        let t = F(fpu::div(dot3(back, self.normal).0, length.0));
        if t < F::ZERO || !(t <= F::ONE) {
            return Crossing::Not;
        }
        if self.flags & flags::PLANE != 0 {
            return Crossing::Through;
        }
        let width = read(self.width);
        if width <= F::ZERO {
            return Crossing::Not;
        }
        let at = to.add(step.mul_q(t.0)).sub(self.centre);
        let side = F(fpu::div(fpu::mul(2.0, dot3(at, self.across).0), width.0));
        if self.flags & flags::ONE_SIDE != 0 && side < F::ZERO {
            return Crossing::Beside;
        }
        if !(side <= F::ONE) || side < F(-1.0) {
            return Crossing::Beside;
        }
        let up = F(fpu::sub(at.y(), fpu::mul(read(F(self.across.y())).0, side.0)));
        if !(up <= read(self.height)) {
            return Crossing::Beside;
        }
        if self.flags & flags::EITHER_WAY != 0 || F::ZERO < dot3(step, self.normal) {
            Crossing::Through
        } else {
            Crossing::Not
        }
    }
}

/// Where a rider's score card last saw it, as the start-gate watch keeps it: `now` (`+0x60`) and the
/// tick before (`+0x70`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Seen {
    pub now: Vf,
    pub last: Vf,
}

/// A rider the closed start gate caught: the vector it is moved by, and its seconds of no gas (the
/// score card's [`crate::score::Score::gate_penalty`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Caught {
    pub push: Vf,
    pub penalty: F,
}

impl Gate {
    /// The race manager's start-gate watch, for one of a card's entities, once a tick while the gate is
    /// live: the card's `now` moves to `last` and the entity's position `at` becomes `now`; a step
    /// from `now` back to `last` that passes [`Gate::crossed`] throws the entity back along its
    /// `facing` by [`gate_push_back`] (the facing's w kept) and gives it [`gate_penalty`]. A gate that
    /// is not live leaves the card alone and draws nothing.
    pub fn watch(&self, seen: &mut Seen, at: Vf, facing: Vf, rng: &mut Rng) -> Option<Caught> {
        if self.flags & flags::CLOSED == 0 {
            return None;
        }
        seen.last = seen.now;
        seen.now = at;
        if self.crossed(seen.now, seen.last) != Crossing::Through {
            return None;
        }
        let push = facing.merge(Dest::XYZ, facing.mul_q(gate_push_back(rng).0));
        Some(Caught { push, penalty: gate_penalty(rng) })
    }
}

/// How far a rider caught through the closed start gate is thrown back: its facing is scaled by this,
/// `-5 - 5 × unit`, so between five and ten feet the other way. One `rand()`.
pub fn gate_push_back(rng: &mut Rng) -> F {
    F(-5.0) - F(5.0) * rng.unit()
}

/// The seconds of no gas a rider caught through the closed start gate gets: `1 + unit`, so one to two.
/// One `rand()`, drawn after [`gate_push_back`]'s.
pub fn gate_penalty(rng: &mut Rng) -> F {
    F::ONE + rng.unit()
}
