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
    /// combo already open scores and banks as before. No backflip watch starts either; one already
    /// part way round goes on ([`Score::watch_flip`], S284).
    pub out: bool,
    /// **The start-gate penalty (P158),** seconds (the card's `+0x1c0`): while it is above 0 the bike's
    /// tick cuts the gas ([`crate::ride::Controls::gas_cut`]), and the manager's tick runs it down.
    /// The original's race sets it to 1 + a random fraction when a rider crosses its closed start
    /// gate ([`crate::race::Race::tick`], S13b), and the card's join sets it to 0.
    pub gate_penalty: F,
    /// **The port's own (P243): a crash locks the trick out.** The original banks a combo the frame
    /// both wheels touch, and a wreck a few ticks later on the same landing — the body slamming down,
    /// a tip, a fling — finds nothing open and keeps the points. With this up, a landing's bank waits
    /// [`LOCKOUT`] seconds in [`Score::pending`] first: a wreck in that time takes it away
    /// ([`Score::crashed`]), riding it out or opening the next combo banks it ([`Score::settle`]).
    /// Down, every call is the original's. Not hashed: it is the ride's setting, like the wreck switches.
    pub lockout: bool,
    /// A landed combo's points waiting out [`Score::settling`] before they go on the total (P243).
    pub pending: F,
    /// Seconds left before [`Score::pending`] is banked; zero when nothing waits.
    pub settling: F,
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
/// **The port's own (P243):** how long a landed combo waits before it banks, with
/// [`Score::lockout`] up — a wreck in that time loses it. A second (30 ticks) covers a landing's
/// own wreck — the body coming down onto the ground after the wheels, a tip or a fling off the
/// landing — which comes within a few ticks of the touch.
pub const LOCKOUT: F = F(1.0);

impl Score {
    /// Open a combo: the manager's start, on a card with none open.
    pub fn open(&mut self) {
        // Riding into the next combo: the last landing was clean (P243).
        self.settle();
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
        if ok && self.lockout {
            // Held back until the landing is ridden out (P243). The total is whole already, so its
            // rounding below changes nothing and [`Score::settle`] rounds the sum as this would have.
            self.pending = self.pending + sum * scale;
            self.settling = LOCKOUT;
        } else if ok {
            self.total = self.total + sum * scale;
        }
        self.combo_points = F::ZERO;
        self.total = F(fpu::from_i32((self.total / TOTAL_GRANULARITY).to_i32())) * TOTAL_GRANULARITY;
        self.nodes.clear();
    }

    /// **The landing was clean** (P243): the points [`Score::lockout`] held back go on the total,
    /// rounded down to the manager's granularity as [`Score::close`] rounds. Nothing when none wait.
    pub fn settle(&mut self) {
        if self.settling == F::ZERO && self.pending == F::ZERO {
            return;
        }
        self.total = self.total + self.pending;
        self.total = F(fpu::from_i32((self.total / TOTAL_GRANULARITY).to_i32())) * TOTAL_GRANULARITY;
        self.pending = F::ZERO;
        self.settling = F::ZERO;
    }

    /// **The rider crashed** (P243, the wreck entry): the combo is lost as [`Score::wrecked`] loses
    /// it, and with [`Score::lockout`] up so is a landing's bank still waiting — the crash came on
    /// the landing, and the trick does not count. Without the lockout nothing waits and this is
    /// [`Score::wrecked`].
    pub fn crashed(&mut self) {
        self.wrecked();
        self.pending = F::ZERO;
        self.settling = F::ZERO;
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

    /// **The backflip watch's step** (the end of the rider's tick): up in the air and riding, the
    /// chassis' attitude moves the watch on; otherwise a watch part way round is forgotten. A card
    /// whose event time is out ([`Score::out`], S284) does not start a watch — only one already past 0
    /// goes on, as the original's tick steps it only on a card not out or a watch above 0.
    pub fn watch_flip(&mut self, s: &Sense) {
        if !s.wrecked && s.airborne && (!self.out || 0 < self.flip) {
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
    }

    /// [`Score::frame`] with MX vs. ATV's front flip watch beside the backflip's (MD16):
    /// `front_flip` is the watch and what a front flip is worth. `None` is [`Score::frame`] exactly.
    ///
    /// The watch is the backflip's mirrored — **the port's own**, as the front flip is: nose down
    /// past level with the bike's up axis under it (1), then over with the forward axis pointing up
    /// again (2), then the right way up with the nose not past 0.79 up (3), which scores as a backflip
    /// round does: at once in the air when nothing is being scored, or on the landing.
    pub fn frame_with(&mut self, dt: F, s: &Sense, mut front_flip: Option<(&mut u8, F)>) {
        // A landing's bank waiting out the lockout (P243): the wreck entry before this tick took it
        // away if the rider came off, so one still here banks once its time is up.
        if F::ZERO < self.settling {
            self.settling = self.settling - dt;
            if self.settling <= F::ZERO {
                self.settle();
            }
        }
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
        self.watch_flip(s);
        if let Some((w, points)) = front_flip {
            if *w == 3 && !self.scoring {
                self.start_flat(FRONTFLIP, points);
                *w = 0;
            }
            step_front_watch(w, s);
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

/// **MX vs. ATV's card beside MX Unleashed's (S358)**: what a ride on a trick pack keeps besides the
/// [`Score`] it scores on, on [`crate::trick_pack::State::score`] — so a ride on the original's tricks
/// carries none of it, and its wire and hash are what they were.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Atv {
    /// The stunt table's repeat scales the banks have lowered ([`Score::atv_repeat`]), by the port's
    /// trick id, oldest first. A stunt not listed is at 1, as every stunt starts.
    pub scales: Vec<Scale>,
    /// The bank's clock: set to [`ATV_BANK`] while a stunt runs off one wheel, run down on the ground,
    /// and the combo banked when it runs out ([`Score::atv_frame`]).
    pub bank: F,
    /// **The stunt word as the tick starts** (S411): whether the frame before ended with a stunt
    /// held — a trick in progress, a node other than a wheel's still running, or a backflip's or a
    /// 360's node with its move still on. The bank's clock is held up on it, not on this frame's.
    pub held: bool,
    /// The wheelie's and the stoppie's time past their first second on the one wheel: the stunt
    /// starts when it is still 0, and the combo banks when the bike comes off the wheel with it over 0.
    pub wheel: [F; 2],
}

/// One stunt's repeat scale ([`Atv::scales`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Scale {
    pub id: u8,
    pub scale: F,
}

/// One stunt's two numbers off a trick pack's table (its `PointValue` and `HoldPointValue`): what its
/// node opens at, and what it grows at a second while it is held ([`Score::atv_start`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Values {
    pub point: F,
    pub hold: F,
}

/// The rows MX vs. ATV's stunts score off where a pack's table has none of that name: state 4's (R17).
/// Its tricks are all 1000 and 1000 a second, the backflip 2000 and 10 000, the 360s 2000 and 15 000.
pub const ATV_TRICK: Values = Values { point: F(1000.0), hold: F(1000.0) };
pub const ATV_BACKFLIP: Values = Values { point: F(2000.0), hold: F(10000.0) };
pub const ATV_WHEEL: Values = Values { point: F(1000.0), hold: F(1000.0) };
pub const ATV_THREE_SIXTY: Values = Values { point: F(2000.0), hold: F(15000.0) };
/// The least a stunt's node opens at, and grows at.
pub const ATV_LEAST: F = F(10.0);
/// MX vs. ATV's node granularity, and its manager's.
pub const ATV_GRANULARITY: F = F(1.0);
/// How long on the ground with no stunt running before the combo banks.
pub const ATV_BANK: F = F(0.2);
/// The stunt table's repeat numbers (state 4's table head, R17): a banked stunt's scale is
/// multiplied by [`ATV_REPEAT`], to no lower than [`ATV_REPEAT_FLOOR`]; banking one at full scale
/// first divides every other scale by [`ATV_RECOVER`], back up to 1 at most.
/// The recovery is one bit over 0.95 in the table (0x3f733334), and the oracle tells the two apart.
pub const ATV_RECOVER: F = F(f32::from_bits(0x3f73_3334));
pub const ATV_REPEAT: F = F(0.8);
pub const ATV_REPEAT_FLOOR: F = F(f32::from_bits(0x3ed1_b719));
/// A trick's hold window (S410): from its `HoldTime`, or [`ATV_HOLD_EARLY`] if that is less, to its
/// `HoldTime` and [`ATV_HOLD_SLACK`] (state 4's tuning word, 0.04).
pub const ATV_HOLD_EARLY: F = F(0.3);
pub const ATV_HOLD_SLACK: F = F(f32::from_bits(0x3d23_d70a));

/// **The marks MX vs. ATV's trick step makes on a running stunt's node** (S410), in its order: on
/// first, then off. See [`Score::atv_hold_marks`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AtvHoldMarks {
    pub on: bool,
    pub off: bool,
}
/// A node's flags past [`flag`]'s, MX vs. ATV's: it is still running, and it is held (grows).
pub mod atv_flag {
    pub const LIVE: u32 = 0x10;
    pub const MARKED: u32 = 0x20;
}

/// What the rider's tick hands [`Score::atv_frame`] for a pack ride: the rows its own stunts score
/// off, and whether a trick's clip is running.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AtvRows {
    pub backflip: Values,
    pub wheelie: Values,
    pub stoppie: Values,
    pub trick_running: bool,
    /// The bike's up and forward axes' heights as the frame started (S411): what the backflip's and
    /// the front flip's watches read, as the original's backflip reads the node it left the frame
    /// before.
    pub heights: (F, F),
    /// The backflip's or the 360's move is still on (S411): its stunt word stays held past the clip
    /// until the move itself ends, which on a landing is a frame or so after the wheels touch.
    pub move_on: bool,
    /// The backflip's torque is on this frame (S411): the original's backflip moves its clip on only
    /// under the flip's torque, so a bike pitched over in a 360 or off a lip starts no backflip.
    pub flip_torque: bool,
    /// The front flip's torque is on this frame: the port's own front flip watch moves on under it
    /// alone, as the backflip's does under its own, so a backflip's or a 360's back half starts none.
    pub front_torque: bool,
}

/// **MX vs. ATV's way (S358, R17)**, on a ride with a trick pack on. The card is the same as MX
/// Unleashed's — the original's two games share it, node for node — but the stunts score
/// differently: every stunt pushes its own node at its start, worth its `PointValue` and growing at
/// its `HoldPointValue` a second while it is held (both times the stunt's repeat scale, at least
/// [`ATV_LEAST`]), and the whole combo banks after [`ATV_BANK`] on the ground, at its sum times its
/// node count. [`Score::combo`] is the run open, [`Score::scoring`] a node still running; a node is
/// running ([`atv_flag::LIVE`]) until it ends and grows while it is held ([`atv_flag::MARKED`]). A
/// wreck loses the combo as [`Score::wrecked`] does: the original's cancel leaves the card exactly as
/// that does.
impl Score {
    /// A stunt's repeat scale.
    pub fn atv_scale(atv: &Atv, id: u8) -> F {
        atv.scales.iter().find(|s| s.id == id).map_or(F::ONE, |s| s.scale)
    }

    /// The stunts a start keeps running: the backflip, the 360s, the wheelie and the stoppie, and the
    /// port's front flip with them.
    pub fn atv_kept(id: u8) -> bool {
        matches!(id, BACKFLIP | WHEELIE | STOPPIE | FRONTFLIP | THREE_SIXTY_LEFT | THREE_SIXTY_RIGHT)
    }

    /// **A stunt started** (`id`, its row; `held`, whether its node grows from the start — the
    /// backflip's and the wheel's do, a trick's waits for its hold): any stunt but the kept ones first
    /// ends the running nodes from the newest back to the first kept one; then the combo opens unless
    /// one is, and the stunt's node is pushed and marked.
    pub fn atv_start(&mut self, atv: &Atv, id: u8, row: Values, held: bool) {
        if !Score::atv_kept(id) {
            let mut k = 0;
            while k < self.nodes.len() && self.nodes[k].flags & atv_flag::LIVE != 0 {
                let running = self.nodes[k].id;
                if Score::atv_kept(running) {
                    break;
                }
                self.atv_end(running);
                k += 1;
            }
        }
        if !self.combo && self.may_open() {
            self.open();
        }
        let scale = Score::atv_scale(atv, id);
        let base = (row.point * scale).max(ATV_LEAST);
        let rate = (row.hold * scale).max(ATV_LEAST);
        self.push(base, F::ZERO, rate, F::ONE, ATV_GRANULARITY, id, flag::FLOOR | flag::TIMED | atv_flag::LIVE);
        self.atv_mark(id, held);
    }

    /// **A stunt held or let go**: the last running node of `id` not held (or held) is marked (or not).
    pub fn atv_mark(&mut self, id: u8, on: bool) {
        let hit = self.nodes.iter().rposition(|n| {
            n.id == id && n.flags & atv_flag::LIVE != 0 && (n.flags & atv_flag::MARKED == 0) == on
        });
        if let Some(k) = hit {
            let n = &mut self.nodes[k];
            n.flags = if on { n.flags | atv_flag::MARKED } else { n.flags & !atv_flag::MARKED };
        }
    }

    /// **Which marks a running stunt's step makes** (S410), before its clip clock (`time`) runs on by
    /// `dt`: with the gesture of the stunt's own kind held (`same_kind`), a hold time (`hold`) over 0
    /// and the clock inside the hold window ([`ATV_HOLD_EARLY`], [`ATV_HOLD_SLACK`]), the node is
    /// marked; at or past `hold` the clip is held there and that is all. Otherwise the clock runs on,
    /// and unless the stunt is `special` (one of the kept ones, [`Score::atv_kept`], on a driven bike)
    /// a clock still short of its `duration` lets the node go (one at or past it ends the stunt,
    /// which the caller's clock tells).
    #[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
    pub fn atv_hold_marks(time: F, hold: F, duration: F, dt: F, same_kind: bool, special: bool) -> AtvHoldMarks {
        let early = hold.min(ATV_HOLD_EARLY);
        let on = same_kind && !(hold <= F::ZERO) && !(time < early) && time <= hold + ATV_HOLD_SLACK;
        if on && !(time < hold) {
            return AtvHoldMarks { on, off: false };
        }
        AtvHoldMarks { on, off: !special && time + dt < duration }
    }

    /// The marks [`Score::atv_hold_marks`] answered, made on `id`'s node in their order.
    pub fn atv_hold(&mut self, id: u8, marks: AtvHoldMarks) {
        if marks.on {
            self.atv_mark(id, true);
        }
        if marks.off {
            self.atv_mark(id, false);
        }
    }

    /// **A stunt ended** (`id`): with the combo open, its last running node finished, landed.
    pub fn atv_end(&mut self, id: u8) {
        if self.combo {
            let hit = self.nodes.iter().rposition(|n| n.id == id && n.flags & atv_flag::LIVE != 0);
            self.atv_finish(hit, F::ONE, true);
        }
    }

    /// The node `hit` stopped and let go, then landed (`ok`) its points times `x` and rounded, or
    /// not landed no longer counting; then whether a node still runs. Nothing without a running node.
    fn atv_finish(&mut self, hit: Option<usize>, x: F, ok: bool) {
        if !(self.scoring && self.combo) {
            return;
        }
        if let Some(n) = hit.and_then(|k| self.nodes.get_mut(k)) {
            n.flags &= !(atv_flag::LIVE | atv_flag::MARKED);
            if !ok {
                n.flags &= !flag::VALID;
            } else {
                n.points = n.points * x;
                n.round();
            }
        }
        self.scoring = self.nodes.iter().any(|n| n.flags & atv_flag::LIVE != 0);
    }

    /// **The combo banked** (the stunt end on every stunt): each running node finished, landed, and
    /// the combo tallied at its node count.
    pub fn atv_bank(&mut self, atv: &mut Atv) {
        if self.combo {
            self.atv_close(F::ONE, true);
            let count = self.count();
            self.atv_tally(atv, count, true);
        }
    }

    /// **The combo lost** (the stunt cancel on every stunt): each running node not landed, and the
    /// combo closed with nothing added. The card ends as [`Score::wrecked`] leaves it.
    pub fn atv_cancel(&mut self, atv: &mut Atv) {
        if self.combo {
            self.atv_close(F::ZERO, false);
            self.atv_tally(atv, F::ZERO, false);
        }
    }

    /// Every running node finished, oldest last.
    fn atv_close(&mut self, x: F, ok: bool) {
        for k in 0..self.nodes.len() {
            if self.nodes[k].flags & atv_flag::LIVE != 0 {
                self.atv_finish(Some(k), x, ok);
            }
        }
    }

    /// The combo tallied: the nodes' points summed (every node's, newest first), the sum times `x`
    /// added to the total when `ok`, the total floored to the manager's granularity, and with `ok`
    /// each banked stunt's repeat scale lowered ([`Score::atv_repeat`]) — once per node, so a stunt
    /// twice in the combo is lowered twice. With [`Score::lockout`] up the sum waits as
    /// [`Score::close`]'s does.
    fn atv_tally(&mut self, atv: &mut Atv, x: F, ok: bool) {
        self.combo = false;
        self.scoring = false;
        self.growing = false;
        let mut sum = F::ZERO;
        for n in &mut self.nodes {
            sum = sum + n.points;
            n.flags &= !(atv_flag::LIVE | atv_flag::MARKED);
        }
        if ok && self.lockout {
            self.pending = self.pending + sum * x;
            self.settling = LOCKOUT;
        } else if ok {
            self.total = fpu::Acc::adda(F::ZERO, self.total).madd(sum, x);
        }
        self.total = F(fpu::from_i32((self.total / ATV_GRANULARITY).to_i32())) * ATV_GRANULARITY;
        if ok {
            for k in 0..self.nodes.len() {
                Score::atv_repeat(atv, self.nodes[k].id);
            }
        }
        self.combo_points = F::ZERO;
        self.nodes.clear();
    }

    /// **A banked stunt's repeat scale lowered**: banked at full scale, every scale first goes back
    /// up by [`ATV_RECOVER`] (to 1 at most); then its own times [`ATV_REPEAT`], no lower than
    /// [`ATV_REPEAT_FLOOR`].
    pub fn atv_repeat(atv: &mut Atv, id: u8) {
        if Score::atv_scale(atv, id) == F::ONE {
            for s in &mut atv.scales {
                s.scale = (s.scale / ATV_RECOVER).min(F::ONE);
            }
            atv.scales.retain(|s| s.scale != F::ONE);
        }
        let scale = (Score::atv_scale(atv, id) * ATV_REPEAT).max(ATV_REPEAT_FLOOR);
        match atv.scales.iter_mut().find(|s| s.id == id) {
            Some(s) => s.scale = scale,
            None => atv.scales.push(Scale { id, scale }),
        }
    }

    /// **The manager's tick** for a pack ride: [`Score::grow`]'s, but every running node held grows
    /// — `(base + held × rate) × mult`, rounded — and not only the newest.
    pub fn atv_grow(&mut self, dt: F) {
        if F::ZERO < self.gate_penalty {
            self.gate_penalty = self.gate_penalty - dt;
        }
        if !self.combo || self.shut {
            return;
        }
        self.combo_time = self.combo_time + dt;
        if self.scoring {
            for n in self.nodes.iter_mut().filter(|n| n.flags & (atv_flag::LIVE | atv_flag::MARKED) == atv_flag::LIVE | atv_flag::MARKED) {
                n.held = n.held + dt;
                n.points = fpu::Acc::adda(F::ZERO, n.base).madd(n.held, n.rate);
                n.points = n.points * n.mult;
                n.round();
            }
        }
        let mut sum = F::ZERO;
        for n in self.nodes.iter().filter(|n| n.flags & flag::VALID != 0) {
            sum = sum + n.points;
        }
        self.combo_points = F(fpu::from_i32((sum / ATV_GRANULARITY).to_i32())) * ATV_GRANULARITY;
    }

    /// **The rider's tick** for a pack ride, after the trick clocks: the wheelie and stoppie (each
    /// starts its stunt after a second on the one wheel, and coming off it banks the combo), the
    /// bank's clock, then the backflip and the front flip — each starts its stunt, held, as the bike
    /// goes past upright and ends it when it comes round. `front_flip` is the front flip's watch.
    /// Answers whether the front flip came round.
    pub fn atv_frame(&mut self, atv: &mut Atv, dt: F, s: &Sense, rows: AtvRows, front_flip: &mut u8) -> bool {
        // A landing's bank waiting out the lockout (P243), as [`Score::frame_with`].
        if F::ZERO < self.settling {
            self.settling = self.settling - dt;
            if self.settling <= F::ZERO {
                self.settle();
            }
        }
        let (front, rear) = (s.front, s.rear);
        if s.wrecked {
            self.wheelie = F::ZERO;
            self.stoppie = F::ZERO;
        } else if !front && rear && !s.slow {
            self.atv_wheel(atv, dt, 0, WHEELIE, rows.wheelie);
        } else if front && !rear && !s.slow {
            self.atv_wheel(atv, dt, 1, STOPPIE, rows.stoppie);
        } else if (front && rear) || (!s.airborne && s.slow) || (s.airborne && F(0.1) < s.hang_time) {
            if F::ZERO < atv.wheel[0] || F::ZERO < atv.wheel[1] {
                self.atv_bank(atv);
            }
            self.wheelie = F::ZERO;
            self.stoppie = F::ZERO;
            atv.wheel = [F::ZERO; 2];
        }
        // The bank's clock (S411, against the original's ride): held up off one wheel on the stunt
        // word the frame started with, and run down every frame on the ground, held or not — so it
        // starts down a frame before the word goes, and banks on the sixth frame after.
        if atv.held && atv.wheel[0] <= F::ZERO && atv.wheel[1] <= F::ZERO {
            atv.bank = ATV_BANK;
        }
        if !s.airborne && F::ZERO < atv.bank {
            atv.bank = atv.bank - dt;
            if atv.bank <= F::ZERO {
                self.atv_bank(atv);
            }
        }
        // The backflip: started over the top, ended round; a watch forgotten part way ends it too.
        // Both watches read the heights the frame started with (S411).
        let s = &Sense { up_y: rows.heights.0, forward_y: rows.heights.1, ..*s };
        let was = self.flip;
        if rows.flip_torque || !s.airborne || s.wrecked {
            self.watch_flip(s);
        }
        // Not inside a front flip, whose back half passes the backflip's first step.
        if was == 0 && self.flip == 1 && *front_flip == 0 {
            self.atv_start(atv, BACKFLIP, rows.backflip, true);
        } else if was != 0 && was != 3 && (self.flip == 3 || self.flip == 0) {
            self.atv_end(BACKFLIP);
        }
        if self.flip == 3 {
            self.flip = 0;
        }
        let was = *front_flip;
        if rows.front_torque || !s.airborne || s.wrecked {
            step_front_watch(front_flip, s);
        }
        if was == 0 && *front_flip == 1 {
            self.atv_start(atv, FRONTFLIP, rows.backflip, true);
        } else if was != 0 && was != 3 && (*front_flip == 3 || *front_flip == 0) {
            self.atv_end(FRONTFLIP);
        }
        let round = *front_flip == 3;
        if round {
            *front_flip = 0;
        }
        // The stunt word the next frame starts with.
        let moves = [BACKFLIP, FRONTFLIP, THREE_SIXTY_LEFT, THREE_SIXTY_RIGHT];
        atv.held = rows.trick_running
            || self.nodes.iter().any(|n| {
                let live = n.flags & atv_flag::LIVE != 0;
                (live && n.id != WHEELIE && n.id != STOPPIE) || (rows.move_on && moves.contains(&n.id))
            });
        round
    }

    /// One wheel's clock run a frame (`k` 0 the wheelie, 1 the stoppie): past a second its time
    /// counts, and the stunt starts, held, the frame it is still 0.
    fn atv_wheel(&mut self, atv: &mut Atv, dt: F, k: usize, id: u8, row: Values) {
        let clock = if k == 0 { &mut self.wheelie } else { &mut self.stoppie };
        *clock = *clock + dt;
        if *clock <= ONE_WHEEL {
            return;
        }
        if atv.wheel[k] <= F::ZERO {
            self.atv_start(atv, id, row, true);
        }
        atv.wheel[k] = atv.wheel[k] + dt;
    }
}

/// The front flip watch's step (MD16): the backflip's mirrored, as [`Score::frame_with`] describes.
fn step_front_watch(w: &mut u8, s: &Sense) {
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

impl State for Atv {
    fn walk(&self, w: &mut Walk<'_>) {
        for s in &self.scales {
            w.u8("scale.id", s.id);
            w.f("scale", s.scale);
        }
        w.f("bank", self.bank);
        w.bool("held", self.held);
        w.f("wheel", self.wheel[0]);
        w.f("wheel", self.wheel[1]);
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
        // Only the lockout (P243) holds a bank back, so a ride without it hashes as before.
        if self.pending != F::ZERO || self.settling != F::ZERO {
            w.f("pending", self.pending);
            w.f("settling", self.settling);
        }
    }
}
