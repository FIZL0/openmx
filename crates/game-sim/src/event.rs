//! **The freestyle event** (S92): the stunt competition's clock, its target and who wins.
//!
//! The freeride card (S79, [`crate::score`]) scores the same way in the event; what the event adds is
//! around it, in the original's *stunt competition* activity and the score manager it owns (its
//! *mode 3*, the one every freeride and freestyle activity runs):
//!
//! - **The join.** Each human rider's card is taken over: its flags start again (no combo open,
//!   not yet taking part), its freeride total is put aside as a whole number and the card's total
//!   starts from nothing.
//! - **The countdown.** 3.33 seconds in which no combo opens. The activity's listener is called
//!   every tick (its interval, 0.01 s, is shorter than a tick); the tick its elapsed time is past
//!   3.33 it sets the **time limit, 120 seconds**, on the manager and on every card. The tick the
//!   elapsed time reaches the countdown's length the manager starts running and every card takes
//!   part; the clock counts from the tick after.
//! - **The clock.** Each running tick every card that takes part and has time left loses the tick's
//!   dt; at zero (or under) the card is **out**: its time is set to 0 and no combo opens on it again,
//!   though one already open still scores and banks. The manager's *all done* flag is set once no
//!   card is left with time — judged at the top of the tick, so a tick after the last card ran out.
//! - **The target.** The track's `hit.dat` `[HitData] StuntCompetitionScore`, a whole number
//!   ([`target`]). Alone, the rider wins by finishing **above** it; the frame the total first reaches
//!   it the announcer says so ([`Competition::reached`]), and in the last five seconds with the
//!   target not reached a warning is up ([`Competition::warning`]).
//! - **The leader** is the rider whose total first passed the best so far — the best kept as a whole
//!   number, so a total of 100.7 passes one of 100.5. Two riders: the leader wins unless their totals
//!   are equal, a tie.
//! - **The end** of a rider ([`Competition::finish`]): the verdict, and the freeride total put back
//!   with the event's points on top.
//!
//! Each tick runs the activity's frame (leader, tie, warning, target) and then the manager's
//! (countdown or clock), both before the cards' own manager work, which [`crate::score::Score::grow`]
//! is.
//!
//! # Not ported
//!
//! The event's other modes (4, 5 and 6 — the trick-matching game with its timed nodes and strikes is
//! mode 5) are never set by any activity of the game; the journal has the scan. The HUD, the
//! announcer and the results screen are the shell's; this module only says when they speak.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::score::Score;
use ps2_float::{F, fpu};

/// The countdown's length, seconds.
pub const COUNTDOWN: F = F(3.33);
/// How often the countdown calls the activity's listener, seconds — less than a tick, so every tick.
pub const INTERVAL: F = F(0.01);
/// The time limit the listener sets once the countdown is past [`COUNTDOWN`], seconds.
pub const LIMIT: F = F(120.0);
/// The warning shows in the last this many seconds.
pub const WARNING: F = F(5.0);

/// The track's target, from its `hit.dat`: `StuntCompetitionScore` under `[HitData]`, read as the
/// original's ini reader reads an int (leading digits, optionally signed). `None` if it is not there.
pub fn target(hit_dat: &str) -> Option<i32> {
    let mut section = false;
    for line in hit_dat.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            section = line.eq_ignore_ascii_case("[HitData]");
            continue;
        }
        if !section {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        if key.trim().eq_ignore_ascii_case("StuntCompetitionScore") {
            let v = value.trim();
            let (sign, digits) = match v.strip_prefix('-') {
                Some(rest) => (-1, rest),
                None => (1, v.strip_prefix('+').unwrap_or(v)),
            };
            let n: String = digits.chars().take_while(char::is_ascii_digit).collect();
            return Some(sign * n.parse::<i32>().unwrap_or(0));
        }
    }
    None
}

/// One rider in the event: their card's event words and the activity's words about them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Seat {
    /// Seconds left (the card's clock).
    pub left: F,
    /// Seconds ridden while taking part.
    pub ridden: F,
    /// Taking part: set on every card when the manager starts running.
    pub taking_part: bool,
    /// Out: the card's time ran out.
    pub out: bool,
    /// The card's clock is held (the card's pause flag; nothing in the game is seen to set it).
    pub paused: bool,
    /// The target was reached and announced (alone only).
    pub reached: bool,
    /// The last-seconds warning is up.
    pub warning: bool,
    /// The freeride total put aside at the join, as the whole number it was cut to.
    pub saved: i32,
}

/// How a rider's event ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Win,
    Lose,
    Tie,
}

/// The stunt competition and its manager's clock.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Competition {
    /// The track's target ([`target`]).
    pub target: i32,
    /// The riders, in the order they joined.
    pub seats: Vec<Seat>,
    /// The time limit; 0 until the countdown sets it.
    pub limit: F,
    /// The countdown's length and listener interval, seconds.
    pub length: F,
    pub interval: F,
    /// The countdown's elapsed time and its listener's accumulator.
    pub elapsed: F,
    pub tally: F,
    /// The countdown is on.
    pub counting: bool,
    /// The manager is running: the clock counts and combos open.
    pub running: bool,
    /// Every card is out (judged at the top of a tick).
    pub over: bool,
    /// A card ran out.
    pub timed_out: bool,
    /// The end has been called (once, the tick `over` is first seen).
    pub ended: bool,
    /// Seconds the manager has run.
    pub clock: F,
    /// The leading seat; none until a total passes nothing (-1).
    pub leader: Option<usize>,
    /// Two riders with equal totals.
    pub tie: bool,
    /// What the listener did this tick, for the HUD: the countdown's first call, the "go" calls
    /// once a second has gone, the limit set.
    pub heard: Option<F>,
}

impl Competition {
    /// A competition on a track with this target, nobody in it.
    pub fn new(target: i32) -> Competition {
        Competition { target, ..Competition::default() }
    }

    /// **A human rider joins** with their card: the card's flags start again and its total is put
    /// aside. Returns the seat.
    pub fn join(&mut self, card: &mut Score) -> usize {
        let saved = card.total.to_i32();
        card.combo = false;
        card.scoring = false;
        card.growing = false;
        card.total = F::ZERO;
        self.seats.push(Seat { saved, ..Seat::default() });
        self.sync(self.seats.len() - 1, card);
        self.seats.len() - 1
    }

    /// **The start**: every card's time to 0, the countdown on. The manager is not running yet.
    pub fn start(&mut self) {
        for s in &mut self.seats {
            s.left = F::ZERO;
        }
        self.length = COUNTDOWN;
        self.interval = INTERVAL;
        self.tie = false;
        if F::ZERO < self.length {
            self.counting = true;
            self.elapsed = F::ZERO;
            self.tally = F::ZERO;
        } else {
            self.counting = false;
            self.begin();
        }
    }

    /// **A tick**: the activity's frame and then its manager's, `cards` in seat order. The cards'
    /// gates are left set for the riders' next tick.
    pub fn tick(&mut self, dt: F, cards: &mut [&mut Score]) {
        assert_eq!(cards.len(), self.seats.len(), "one card a seat");
        self.heard = None;
        // With nobody in it the activity's frame returns before its manager's.
        if self.seats.is_empty() {
            return;
        }
        self.frame(cards);
        if self.running {
            self.clock_tick(dt);
            self.clock = self.clock + dt;
        } else if self.counting {
            self.countdown(dt);
        }
        for (k, card) in cards.iter_mut().enumerate() {
            self.sync(k, card);
        }
    }

    /// **A rider's end**, once their card is out: the verdict, and the card's total with the freeride
    /// total put back. Returns the verdict and the points the profile adds (the event's, cut to a
    /// whole number, on the saved total).
    pub fn finish(&mut self, seat: usize, card: &mut Score) -> (Verdict, i32) {
        let target = F(fpu::from_i32(self.target));
        let count = self.seats.len();
        let lead = self.leader == Some(seat);
        let verdict = if self.tie {
            Verdict::Tie
        } else if (target < card.total && lead && count == 1) || (lead && count == 2) {
            Verdict::Win
        } else {
            Verdict::Lose
        };
        let saved = self.seats[seat].saved;
        let profile = saved.wrapping_add(card.total.to_i32());
        card.total = card.total + F(fpu::from_i32(saved));
        (verdict, profile)
    }

    /// The activity's frame: the leader, the tie, the warning and the target.
    fn frame(&mut self, cards: &[&mut Score]) {
        let target = F(fpu::from_i32(self.target));
        let count = self.seats.len();
        let mut best: i32 = -1;
        let mut last = None;
        for k in 0..count {
            let total = cards[k].total;
            if F(fpu::from_i32(best)) < total {
                best = total.to_i32();
                self.leader = Some(k);
            }
            if count == 2 {
                self.tie = total == cards[1 - k].total;
            }
            let s = &mut self.seats[k];
            if !s.reached && count == 1 {
                s.warning = !(s.left == F::ZERO || WARNING <= s.left || target <= total);
            }
            if !s.reached && target <= total && count == 1 {
                s.reached = true;
            }
            last = Some(k);
        }
        // Two riders: the original reads the time of the card its first loop ended on, for both.
        if count == 2 {
            let left = last.map_or(F::ZERO, |k| self.seats[k].left);
            for k in 0..2 {
                let ahead = self.leader == Some(k) && !self.tie;
                self.seats[k].warning = !(ahead || left == F::ZERO || WARNING <= left);
            }
        }
    }

    /// The countdown, while the manager is not running.
    fn countdown(&mut self, dt: F) {
        if F::ZERO < self.interval && (self.interval <= self.tally || self.tally == F::ZERO) {
            if self.interval <= self.tally {
                self.tally = self.tally - self.interval;
            }
            self.listen();
        }
        if F::ZERO < self.interval {
            self.tally = self.tally + dt;
        }
        if !(self.elapsed < self.length) {
            self.counting = false;
            self.begin();
        }
        self.elapsed = self.elapsed + dt;
    }

    /// The activity's countdown listener, at the countdown's elapsed time.
    fn listen(&mut self) {
        let at = self.elapsed;
        self.heard = Some(at);
        if COUNTDOWN < at {
            self.limit = LIMIT;
            for s in &mut self.seats {
                s.left = LIMIT;
            }
        }
    }

    /// The manager starts running: every card takes part.
    fn begin(&mut self) {
        self.running = true;
        for s in &mut self.seats {
            s.taking_part = true;
        }
    }

    /// Mode 3's clock, one running tick.
    fn clock_tick(&mut self, dt: F) {
        self.over = !self.seats.iter().any(|s| !s.out && s.taking_part);
        for s in &mut self.seats {
            if !s.taking_part || s.out {
                continue;
            }
            s.ridden = s.ridden + dt;
            if F::ZERO < self.limit {
                if !s.paused {
                    s.left = s.left - dt;
                }
                if s.left <= F::ZERO {
                    s.out = true;
                    self.timed_out = true;
                    s.left = F::ZERO;
                }
            }
        }
        if self.over && !self.ended {
            self.ended = true;
        }
    }

    /// The card's gates from its seat.
    fn sync(&self, k: usize, card: &mut Score) {
        let s = &self.seats[k];
        card.shut = !self.running || !s.taking_part;
        card.out = s.out;
    }
}
