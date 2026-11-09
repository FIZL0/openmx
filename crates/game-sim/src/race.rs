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
//!
//! Then the start itself (S13b), [`Race`]: the race manager's tick ahead of the riders — its music and
//! tip clocks, the watch over every rider while the gate is closed, and the score manager's countdown,
//! whose last tick drops the gate. A caught rider's bike is stopped dead and sent back along its nose
//! ([`push`]); at the drop every bike is let go ([`release`]). [`crate::Sim::race`] holds one on a race.
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

/// The start's countdown as the score manager runs it: how long it lasts (`+0xbc` of the manager)
/// and how long it has run (`+0xc4`). While it runs, the manager adds the tick's `dt` to `elapsed`
/// each tick until `elapsed` is no longer under `length`; that tick ends it and drops the gate
/// ([`Race::tick`]). The beeps and the light it shows on the way (every `+0xc0` seconds) are the
/// front end's and change nothing a tick reads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Countdown {
    pub length: F,
    pub elapsed: F,
    /// Still counting (the manager's `0x80` flag); cleared by the tick that drops the gate.
    pub running: bool,
}

/// A race's start (S13b): the race manager's tick as far as a tick reads it — its two clocks, the
/// start gate's watch over every rider while it is closed, and the countdown that opens it.
#[derive(Clone, Debug, PartialEq)]
pub struct Race {
    /// The start gate. [`flags::CLOSED`] until the drop.
    pub gate: Gate,
    /// Where each rider's score card last saw it, in player order: the watch's memory.
    pub cards: Vec<Seen>,
    /// The music's clock (`+0x4c0`): counted down while above zero, and the track starts when it runs
    /// out. The drop sets it to one second.
    pub music_in: F,
    /// The music clock is held: the track is still loading, or the race is paused. The front end's to
    /// say; a tick only reads it.
    pub music_held: bool,
    /// The clutch tip's clock (`+0x410`): counted down while above zero, and the "preload and clutch"
    /// tip shows when it runs out.
    pub tip_in: F,
    /// The length of the race's lesson stage, when it has one (`+0x470` names it, its `+0x24` is the
    /// length): with the tip clock at zero and not yet armed, it arms it to show the tip 2.05 s before
    /// the stage's end, but never sooner than a thousandth of a second from now.
    pub stage: Option<F>,
    /// The tip clock has been armed from the stage (`+0x474`); it is armed once.
    pub tip_armed: bool,
    pub countdown: Countdown,
    /// The word the drop draws for the scene (its `+0xf0`), eight to thirteen seconds.
    pub scene_clock: F,
    /// The race's draws: the watch's two per rider caught and the drop's one.
    pub rng: Rng,
}

/// What one [`Race::tick`] did that is not in the race itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    /// The music clock ran out: the track starts.
    pub music: bool,
    /// The tip clock ran out: the "preload and clutch" tip shows.
    pub tip: bool,
    /// The riders the closed gate caught, by player, in the order it caught them: each is to be
    /// [`push`]ed and given the penalty on its score card.
    pub caught: Vec<(usize, Caught)>,
    /// The countdown ended and the gate dropped: every rider is to be [`release`]d.
    pub dropped: bool,
}

impl Race {
    /// A race waiting at its start: the gate closed, every rider's card seeing it where it stands
    /// (`riders`, as [`Race::tick`] takes them), and a countdown of `length` seconds to run.
    pub fn new(mut gate: Gate, riders: &[(Vf, Vf)], length: F, rng: Rng) -> Race {
        gate.flags |= flags::CLOSED;
        Race {
            gate,
            cards: riders.iter().map(|&(at, _)| Seen { now: at, last: at }).collect(),
            music_in: F::ZERO,
            music_held: false,
            tip_in: F::ZERO,
            stage: None,
            tip_armed: false,
            countdown: Countdown { length, elapsed: F::ZERO, running: true },
            scene_clock: F::ZERO,
            rng,
        }
    }

    /// **The race manager's tick**, before the riders': each rider in `riders` is where its bike is
    /// and which way it faces (its transform's translation and forward row), in player order.
    ///
    /// In the original's order: the music clock, which only runs while above zero and not held, and
    /// starts the track on the tick it reaches zero or below (then reads zero); the gate's watch over
    /// every card while the gate is closed ([`Gate::watch`]); the tip clock, which runs the same way
    /// — or, at zero, is armed once from the lesson stage; and then the score manager's countdown,
    /// whose last tick drops the gate ([`Race::drop`]). The watch runs before the drop, so a rider
    /// through the gate on the drop's own tick is still caught.
    pub fn tick(&mut self, dt: F, riders: &[(Vf, Vf)]) -> Frame {
        let mut frame = Frame::default();
        if !(read(self.music_in) <= F::ZERO) && !self.music_held {
            self.music_in = F(fpu::sub(self.music_in.0, dt.0));
            if self.music_in <= F::ZERO {
                frame.music = true;
                self.music_in = F::ZERO;
            }
        }
        // A rider who joined since the start is seen where it stands first.
        for &(at, _) in &riders[self.cards.len().min(riders.len())..] {
            self.cards.push(Seen { now: at, last: at });
        }
        if self.gate.flags & flags::CLOSED != 0 {
            for (i, &(at, facing)) in riders.iter().enumerate() {
                if let Some(hit) = self.gate.watch(&mut self.cards[i], at, facing, &mut self.rng) {
                    frame.caught.push((i, hit));
                }
            }
        }
        if !(read(self.tip_in) <= F::ZERO) {
            self.tip_in = F(fpu::sub(self.tip_in.0, dt.0));
            if self.tip_in <= F::ZERO {
                frame.tip = true;
                self.tip_in = F::ZERO;
            }
        } else if let Some(length) = self.stage
            && !self.tip_armed
            && read(length) != F::ZERO
        {
            self.tip_armed = true;
            let lead = F(fpu::sub(fpu::sub(length.0, 0.75), 1.3));
            self.tip_in = F(0.001).max(lead);
        }
        let c = &mut self.countdown;
        if c.running {
            if !(read(c.elapsed) < read(c.length)) {
                c.running = false;
                self.drop();
                frame.dropped = true;
            }
            let c = &mut self.countdown;
            c.elapsed = F(fpu::add(c.elapsed.0, dt.0));
        }
        frame
    }

    /// **The drop**, the countdown's end: the music clock set to a second, the gate opened, and the
    /// scene's word drawn, `8 + 5 × unit`. The riders are released by whoever holds them
    /// ([`release`]).
    pub fn drop(&mut self) {
        self.music_in = F::ONE;
        self.gate.flags &= !flags::CLOSED;
        self.scene_clock = F(8.0) + F(5.0) * self.rng.unit();
    }
}

/// A caught rider's bike thrown back: the bike's message for it stops the chassis dead
/// ([`crate::chassis::stop`]) and then makes the push its velocity, `w` and all. The bike is not
/// moved; it rolls back from where it is.
pub fn push(ch: &mut crate::chassis::Chassis, push: Vf) {
    crate::chassis::stop(ch);
    ch.vel = push;
}

/// A rider's bike let go at the drop, the message every card's bike gets after the gate opens: the
/// chassis may move along and turn about every axis again. (A computer rider's also redraws its
/// start; the port's riders are all players.)
pub fn release(ch: &mut crate::chassis::Chassis) {
    ch.free = [1; 3];
    ch.ang_free = [1; 3];
}
