//! **The points** (S79): what a trick, a wheelie, a stoppie and a backflip are worth, how they add up
//! into a combo, and when the combo is banked into the rider's total — or lost.
//!
//! The original keeps a *score card* per rider and one *score manager* the cards hang off. The card
//! holds a list of **nodes**, newest first, one per thing scored since the combo opened; the manager
//! grows the newest node once a frame while it is held, and its total granularity rounds what is
//! banked. All of it is ported here as one [`Score`] on the [`crate::rider::Rider`], so a replay or a
//! rollback carries it and the state hash covers it.
//!
//! # What scores, and when
//!
//! - **A trick** (ids 0–23) opens a node when its clock starts, if the bike is not on both wheels and
//!   the rider is up: nothing, growing at 2500 a second for as long as the trick runs, to the nearest
//!   ten below. When the trick's clock runs out the node is **multiplied by the number of nodes in the
//!   combo** — the second trick of a combo is worth twice its time, the third three times.
//! - **A backflip** (24) is a flat 10 000, multiplied the same way, scored the frame the rider lands
//!   (or the frame after the flip completes, if the bike is still in the air and no trick is running).
//!   The flip is a four-stage watch on the chassis' attitude ([`Score::flip`]).
//! - **A wheelie or a stoppie** (25, 26) opens a node after a second on the one wheel, growing at 1000
//!   a second; it is not multiplied.
//! - **The combo is banked** — every node's points added to [`Score::total`], to the manager's
//!   granularity — when both wheels are down, or when the rider is slow and off the ground's
//!   airborne word. A rider going up on one wheel after a jump has not banked yet.
//! - **A wreck banks nothing.** The running node is invalidated and the combo is closed without being
//!   added: a trick landed into a crash loses the whole combo, every trick of it, and not just its own
//!   points. So does the put-back chord while a combo is open.
//!
//! # Not ported
//!
//! The card's other fields — the event mode's running tallies, the best-wheelie records the rider
//! keeps beside it, the HUD's listeners — move no points and are left out; the journal lists them.
//! The original's manager hands out nodes from **one pool of a hundred shared by every rider**; here
//! each rider has its own hundred, which is the same for one rider and for any combo shorter than a
//! hundred nodes between all of them.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::hash::{State, Walk};
use ps2_float::{F, fpu};

/// The node's flags word, as far as the scoring reads it.
pub mod flag {
    /// The node counts: cleared when the trick it scored was not landed.
    pub const VALID: u32 = 1;
    /// Round to the nearest granularity.
    pub const ROUND: u32 = 2;
    /// Round down to the granularity, which wins over [`ROUND`].
    pub const FLOOR: u32 = 4;
    /// Matched on its held time as well as its id, by an event mode this port has not got (5). Every
    /// scorer sets it.
    pub const TIMED: u32 = 8;
}

/// One thing scored in the combo.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Node {
    /// [`flag`]'s bits.
    pub flags: u32,
    /// What scored it: a trick id, 24 the backflip, 25 a wheelie, 26 a stoppie.
    pub id: u8,
    /// The points it is worth now.
    pub points: F,
    /// What it is worth the moment it opens.
    pub base: F,
    /// Points a second while it is held.
    pub rate: F,
    /// What its held value is multiplied by while it grows.
    pub mult: F,
    /// A value every scorer passes as zero, kept because it is in the node.
    pub extra: F,
    /// Seconds it has been held.
    pub held: F,
    /// What its points are rounded to; nothing when not above zero.
    pub granularity: F,
}

impl Node {
    /// The node's own rounding, which both the growth and the finish apply.
    fn round(&mut self) {
        let g = self.granularity;
        if !(F::ZERO < g) {
            return;
        }
        if self.flags & flag::FLOOR != 0 {
            self.points = F(fpu::from_i32((self.points / g).to_i32())) * g;
        } else if self.flags & flag::ROUND != 0 {
            self.points = F(fpu::from_i32((self.points / g + F(0.5)).to_i32())) * g;
        }
    }
}

/// One rider's points: the card, and the rider's own three words beside it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Score {
    /// The combo's nodes, **newest first** — the order every sum walks them in.
    pub nodes: Vec<Node>,
    /// A combo is open.
    pub combo: bool,
    /// The newest node is still being scored.
    pub scoring: bool,
    /// The newest node grows each frame.
    pub growing: bool,
    /// Seconds the combo has been open.
    pub combo_time: F,
    /// The rider's banked points.
    pub total: F,
    /// The combo's points as they stand, to the granularity: what a HUD shows beside the total.
    pub combo_points: F,
    /// Seconds on the rear wheel alone.
    pub wheelie: F,
    /// Seconds on the front wheel alone.
    pub stoppie: F,
    /// The backflip watch: 0 nothing, 1 the bike's up axis tipped past level, 2 upside down, 3 the
    /// flip is round and waiting to be scored.
    pub flip: u8,
    /// **The event's gate (S92):** no combo opens — the event's manager is not running yet (the
    /// freestyle event's countdown) or the card has not joined it. An open combo still scores and
    /// banks, but it does not grow. Never set outside an event ([`crate::event`]).
    pub shut: bool,
    /// **Out of time (S92):** the event's clock ran out on this card. No combo opens — but a trick
    /// started while the backflip watch stands at 3 (round, not yet scored) still opens one, and a
    /// combo already open scores and banks as before.
    pub out: bool,
    /// **The start-gate penalty (P158),** seconds (the card's `+0x1c0`): while it is above 0 the bike's
    /// tick cuts the gas ([`crate::ride::Controls::gas_cut`]), and the manager's tick runs it down.
    /// The original's race sets it to 1 + a random fraction when a rider crosses its closed start
    /// gate; nothing in the port sets it yet, and the card's join sets it to 0.
    pub gate_penalty: F,
}

/// What the rider's tick reads to score: the bike's words as the frame left them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sense {
    /// The front wheel touches.
    pub front: bool,
    /// The rear wheel touches.
    pub rear: bool,
    /// The bike's airborne verdict.
    pub airborne: bool,
    /// Seconds in the air.
    pub hang_time: F,
    /// The bike is slower than [`SLOW`] — on the speed **the previous frame** left, which the original
    /// reads before the bike moves.
    pub slow: bool,
    /// The rider is down.
    pub wrecked: bool,
    /// The chassis' up axis, its height.
    pub up_y: F,
    /// The chassis' forward axis, its height.
    pub forward_y: F,
}

/// What a trick grows at, a second: the rider's per-trick bonus (zero for every trick in the
/// original) and this.
pub const TRICK_RATE: F = F(2500.0);
/// The backflip's flat points.
pub const BACKFLIP_POINTS: F = F(10000.0);
/// What a wheelie or a stoppie grows at, a second.
pub const WHEEL_RATE: F = F(1000.0);
/// Every node's granularity.
pub const GRANULARITY: F = F(10.0);
/// The manager's: what the total and the combo's points are rounded down to.
pub const TOTAL_GRANULARITY: F = F(1.0);
/// The speed below which a rider is slow, feet a second.
pub const SLOW: F = F(5.0);
/// How long on one wheel before it scores.
pub const ONE_WHEEL: F = F(1.0);
/// The nodes the manager has to hand out.
pub const POOL: usize = 100;

impl Score {
    /// Open a combo: the manager's start, on a card with none open.
    pub fn open(&mut self) {
        self.combo = true;
        self.combo_time = F::ZERO;
        self.nodes.clear();
    }

    /// Put a node at the head of the combo: needs a combo open and a node free.
    pub fn push(&mut self, base: F, extra: F, rate: F, mult: F, granularity: F, id: u8, flags: u32) {
        if !self.combo || self.nodes.len() >= POOL {
            return;
        }
        let node = Node {
            flags: flags | flag::VALID,
            id,
            points: base * mult,
            base,
            rate,
            mult,
            extra,
            held: F::ZERO,
            granularity,
        };
        self.nodes.insert(0, node);
        self.scoring = true;
    }

    /// The newest node grows, or stops growing.
    pub fn hold(&mut self, on: bool) {
        if !on {
            self.growing = false;
        } else if self.scoring {
            self.growing = true;
        }
    }

    /// The newest node is done: landed (`ok`) its points are multiplied by `mult` and rounded, and
    /// not landed it no longer counts.
    pub fn finish(&mut self, mult: F, ok: bool) {
        if !(self.scoring && self.combo) {
            return;
        }
        if let Some(head) = self.nodes.first_mut() {
            if !ok {
                head.flags &= !flag::VALID;
            } else {
                head.points = head.points * mult;
                head.round();
            }
        }
        self.scoring = false;
        self.growing = false;
    }

    /// Close the combo. Banked (`ok`), every node's points — valid or not — times `scale` go onto
    /// the total; not banked, nothing does. Either way the combo's points go back to zero and its
    /// nodes back to the manager.
    pub fn close(&mut self, scale: F, ok: bool) {
        if !self.combo {
            return;
        }
        self.combo = false;
        self.scoring = false;
        self.growing = false;
        let mut sum = F::ZERO;
        for n in &self.nodes {
            sum = sum + n.points;
        }
        if ok {
            self.total = self.total + sum * scale;
        }
        self.combo_points = F::ZERO;
        self.total = F(fpu::from_i32((self.total / TOTAL_GRANULARITY).to_i32())) * TOTAL_GRANULARITY;
        self.nodes.clear();
    }

    /// **A trick's clock started** (the rider's trick-start hook), `id` 0–23 or the backflip.
    pub fn start(&mut self, id: u8, s: &Sense) {
        if !(((!s.front || !s.rear) && !s.wrecked) || id == BACKFLIP) {
            return;
        }
        if !self.combo && (self.may_open() || (self.out && self.flip == 3)) {
            self.open();
        }
        let (base, rate) = if id == BACKFLIP { (BACKFLIP_POINTS, F::ZERO) } else { (F::ZERO, F::ZERO + TRICK_RATE) };
        self.push(base, F::ZERO, rate, F::ONE, GRANULARITY, id, flag::FLOOR | flag::TIMED);
        if id == BACKFLIP {
            let count = self.count();
            if let Some(head) = self.nodes.first() {
                self.combo_points = self.combo_points + head.points * count;
            }
            self.finish(count, true);
        } else {
            self.hold(true);
        }
    }

    /// **The port's own (MD16):** a move scored flat, as [`Score::start`] scores the backflip — on the
    /// ground or not, worth `points` at once, times the combo's length. MX vs. ATV's front flip and
    /// 360 are scored this way, each under its own id.
    pub fn start_flat(&mut self, id: u8, points: F) {
        if !self.combo && self.may_open() {
            self.open();
        }
        self.push(points, F::ZERO, F::ZERO, F::ONE, GRANULARITY, id, flag::FLOOR | flag::TIMED);
        let count = self.count();
        if let Some(head) = self.nodes.first() {
            self.combo_points = self.combo_points + head.points * count;
        }
        self.finish(count, true);
    }

    /// **A trick's clock ran out** (the rider's trick-stop hook): landed, times the combo's length.
    pub fn finished(&mut self) {
        if self.combo {
            let count = self.count();
            self.finish(count, true);
            self.hold(false);
        }
    }

    /// **A trick was cut short** — the put-back: the trick and the combo both lost.
    pub fn aborted(&mut self) {
        if self.combo {
            self.finish(F::ZERO, false);
            self.close(F::ZERO, false);
            self.hold(false);
        }
    }

    /// **The rider came off**: the trick and the combo both lost, and the one-wheel clocks and the
    /// backflip watch back to nothing.
    pub fn wrecked(&mut self) {
        if self.scoring {
            self.finish(F::ZERO, false);
            self.hold(false);
        }
        if self.combo {
            self.close(F::ZERO, false);
        }
        self.wheelie = F::ZERO;
        self.stoppie = F::ZERO;
        self.flip = 0;
    }

    /// **The rider's tick**: the one-wheel clocks, the bank, and the backflip watch, after the bike's
    /// update and the trick clocks have run.
    pub fn frame(&mut self, dt: F, s: &Sense) {
        self.frame_with(dt, s, None);
    }

    /// [`Score::frame`] with MX vs. ATV's front flip watch beside the backflip's (MD16):
    /// `front_flip` is the watch and what a front flip is worth. `None` is [`Score::frame`] exactly.
    ///
    /// The watch is the backflip's mirrored — **the port's own**, as the front flip is: nose down
    /// past level with the bike's up axis under it (1), then over with the forward axis pointing up
    /// again (2), then the right way up with the nose not past 0.79 up (3), which scores as a backflip
    /// round does: at once in the air when nothing is being scored, or on the landing.
    pub fn frame_with(&mut self, dt: F, s: &Sense, mut front_flip: Option<(&mut u8, F)>) {
        if !s.wrecked {
            let (front, rear) = (s.front, s.rear);
            if !front && rear && !s.slow {
                self.wheelie = self.wheelie + dt;
                if ONE_WHEEL < self.wheelie {
                    self.one_wheel(WHEELIE);
                }
            } else if front && !rear && !s.slow {
                self.stoppie = self.stoppie + dt;
                if ONE_WHEEL < self.stoppie {
                    self.one_wheel(STOPPIE);
                }
            } else if (front && rear) || (!s.airborne && s.slow) {
                // Down: the running node and then the combo are banked.
                if self.scoring {
                    self.finish(F::ONE, true);
                    self.hold(false);
                }
                if self.flip == 3 {
                    self.start(BACKFLIP, s);
                    self.flip = 0;
                }
                if let Some((w, points)) = front_flip.as_mut() {
                    if **w == 3 {
                        self.start_flat(FRONTFLIP, *points);
                        **w = 0;
                    }
                }
                if self.combo {
                    self.close(F::ONE, true);
                }
                self.wheelie = F::ZERO;
                self.stoppie = F::ZERO;
            } else if s.airborne && F(0.1) < s.hang_time {
                // Off the ground for good: a wheelie or a stoppie stops growing.
                if self.scoring && self.nodes.first().is_some_and(|n| n.id == WHEELIE || n.id == STOPPIE) {
                    self.finish(F::ONE, true);
                    self.hold(false);
                }
                self.wheelie = F::ZERO;
                self.stoppie = F::ZERO;
            }
        } else {
            self.wheelie = F::ZERO;
            self.stoppie = F::ZERO;
        }
        // A flip come round in the air with nothing running scores at once.
        if self.flip == 3 && !self.scoring {
            self.start(BACKFLIP, s);
            self.flip = 0;
        }
        if !s.wrecked && s.airborne {
            let (up, forward) = (s.up_y, s.forward_y);
            if F::ZERO < up && F(-0.79) < forward {
                // Right way up again: a flip that went over is round, any other is forgotten.
                match self.flip {
                    2 => self.flip = 3,
                    0 | 3 => {}
                    _ => self.flip = 0,
                }
            } else if up < F::ZERO && F::ZERO < forward && self.flip == 0 {
                self.flip = 1;
            } else if up < F::ZERO && forward < F::ZERO && self.flip == 1 {
                self.flip = 2;
            }
        } else if self.flip != 3 && self.flip != 0 {
            self.flip = 0;
        }
        if let Some((w, points)) = front_flip {
            if *w == 3 && !self.scoring {
                self.start_flat(FRONTFLIP, points);
                *w = 0;
            }
            if s.wrecked {
                *w = 0;
            } else if s.airborne {
                let (up, forward) = (s.up_y, s.forward_y);
                if F::ZERO < up && forward < F(0.79) {
                    match *w {
                        2 => *w = 3,
                        0 | 3 => {}
                        _ => *w = 0,
                    }
                } else if up < F::ZERO && forward < F::ZERO && *w == 0 {
                    *w = 1;
                } else if up < F::ZERO && F::ZERO < forward && *w == 1 {
                    *w = 2;
                }
            } else if *w != 3 && *w != 0 {
                *w = 0;
            }
        }
    }

    /// A second on one wheel: the combo opened if it is not, and the wheel's node if nothing is
    /// being scored.
    pub fn one_wheel(&mut self, id: u8) {
        if !self.combo && self.may_open() {
            self.open();
        }
        if !self.scoring {
            self.push(F::ZERO, F::ZERO, WHEEL_RATE, F::ONE, GRANULARITY, id, flag::FLOOR | flag::TIMED);
            self.hold(true);
        }
    }

    /// **The manager's tick**, after every rider's: the combo's clock, the newest node grown, and the
    /// combo's points summed over the nodes that count.
    pub fn grow(&mut self, dt: F) {
        // Every card's start-gate penalty first, joined or not (P158).
        if F::ZERO < self.gate_penalty {
            self.gate_penalty = self.gate_penalty - dt;
        }
        // An event's card that does not take part is passed over, and a manager not running does
        // not tick its cards at all (S92).
        if !self.combo || self.shut {
            return;
        }
        self.combo_time = self.combo_time + dt;
        if self.scoring && self.growing {
            if let Some(head) = self.nodes.first_mut() {
                head.held = head.held + dt;
                head.points = head.base + head.held * head.rate;
                head.points = head.points * head.mult;
                head.round();
            }
        }
        let mut sum = F::ZERO;
        for n in self.nodes.iter().filter(|n| n.flags & flag::VALID != 0) {
            sum = sum + n.points;
        }
        self.combo_points = F(fpu::from_i32((sum / TOTAL_GRANULARITY).to_i32())) * TOTAL_GRANULARITY;
    }

    /// The manager lets a combo open on the card: its event is running, the card takes part and its
    /// time has not run out (S92). Always, outside an event.
    fn may_open(&self) -> bool {
        !self.shut && !self.out
    }

    /// How many nodes the combo holds, as the float the multipliers are.
    fn count(&self) -> F {
        F(fpu::from_i32(self.nodes.len() as i32))
    }
}

/// The backflip's id.
pub const BACKFLIP: u8 = 24;
/// **The port's own (MD16):** the front flip's and MX vs. ATV's 360s' ids ([`crate::trick::Trick`]).
pub const FRONTFLIP: u8 = 29;
pub const THREE_SIXTY_LEFT: u8 = 30;
pub const THREE_SIXTY_RIGHT: u8 = 31;
/// The wheelie's id.
pub const WHEELIE: u8 = 25;
/// The stoppie's id.
pub const STOPPIE: u8 = 26;

impl State for Node {
    fn walk(&self, w: &mut Walk<'_>) {
        w.u32("flags", self.flags);
        w.u8("id", self.id);
        w.f("points", self.points);
        w.f("base", self.base);
        w.f("rate", self.rate);
        w.f("mult", self.mult);
        w.f("extra", self.extra);
        w.f("held", self.held);
        w.f("granularity", self.granularity);
    }
}

impl State for Score {
    fn walk(&self, w: &mut Walk<'_>) {
        w.list("nodes", &self.nodes);
        w.bool("combo", self.combo);
        w.bool("scoring", self.scoring);
        w.bool("growing", self.growing);
        w.f("combo_time", self.combo_time);
        w.f("total", self.total);
        w.f("combo_points", self.combo_points);
        w.f("wheelie", self.wheelie);
        w.f("stoppie", self.stoppie);
        w.u8("flip", self.flip);
        // Only an event's card ever sets them, so a freeride's hash is what it was before S92.
        if self.shut {
            w.bool("shut", self.shut);
        }
        if self.out {
            w.bool("out", self.out);
        }
    }
}
