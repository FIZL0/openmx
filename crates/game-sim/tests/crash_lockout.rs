//! **A crash never scores the trick** (P243, the user's ask: "when we crash we don't get the trick
//! points, it should lock it out"). The original banks a combo the frame both wheels touch, and its
//! wreck entry only loses a combo still open, so a wreck a few ticks after the touch — the body coming
//! down after the wheels, a tip off the landing — keeps the points there. With
//! [`Score::lockout`] up (the app's riders) the bank waits [`score::LOCKOUT`] first and a wreck in that
//! time takes it; down, every one of these is the original's.
//!
//! On the card alone: straight into a wreck, a wreck a few ticks after the touch, a clean landing, the
//! next combo opening, and the event's join and end. On a ride: the stand-in bike dropped with a combo
//! open, landing clean and then slammed down again, through the rider's own wreck entry; and the
//! freestyle event's card never seeing a crashed trick.

use std::sync::Arc;

use game_sim::event::Competition;
use game_sim::phy::Phy;
use game_sim::ride::Controls;
use game_sim::rider::Tables;
use game_sim::score::{self, Score, Sense};
use game_sim::trick::Set;
use game_sim::world::Flat;
use game_sim::{Sim, stock};
use ps2_float::{F, Vf};

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);

fn air() -> Sense {
    Sense { airborne: true, hang_time: F(0.5), up_y: F(1.0), ..Sense::default() }
}

fn landed() -> Sense {
    Sense { front: true, rear: true, up_y: F(1.0), ..Sense::default() }
}

fn down() -> Sense {
    Sense { wrecked: true, ..landed() }
}

/// A card with a trick held `ticks` in the air and its clock run out: what it is worth.
fn trick(s: &mut Score, ticks: usize) -> F {
    s.start(0, &air());
    for _ in 0..ticks {
        s.frame(DT, &air());
        s.grow(DT);
    }
    s.finished();
    assert!(s.combo_points > F::ZERO, "the trick grew no points");
    s.combo_points
}

fn tick(s: &mut Score, sense: &Sense) {
    s.frame(DT, sense);
    s.grow(DT);
}

fn card(lockout: bool) -> Score {
    Score { lockout, total: F(500.0), ..Score::default() }
}

/// Straight into a wreck: the entry comes before the touch's bank, with the lockout or without.
#[test]
fn a_trick_landed_straight_into_a_wreck_scores_nothing() {
    for lockout in [false, true] {
        let mut s = card(lockout);
        trick(&mut s, 30);
        s.crashed();
        for _ in 0..60 {
            tick(&mut s, &down());
        }
        assert_eq!(s.total, F(500.0), "lockout {lockout}: the total moved");
        assert_eq!((s.pending, s.combo_points), (F::ZERO, F::ZERO));
        assert!(!s.combo && s.nodes.is_empty());
    }
}

/// A wreck a few ticks after the touch: the original keeps the bank, the lockout takes it — at any
/// tick of the window, and the total never shows it in between.
#[test]
fn a_wreck_soon_after_the_touch_takes_the_bank_with_the_lockout() {
    let mut original = card(false);
    let points = trick(&mut original, 30);
    tick(&mut original, &landed());
    tick(&mut original, &landed());
    original.crashed();
    assert_eq!(original.total, F(500.0) + points, "the original keeps a bank a later wreck finds");

    for after in [1, 3, 10, 29] {
        let mut s = card(true);
        let worth = trick(&mut s, 30);
        assert_eq!(worth, points);
        tick(&mut s, &landed());
        assert!(!s.combo, "the touch closed the combo");
        assert_eq!(s.pending, points, "the bank waits");
        for _ in 1..after {
            tick(&mut s, &landed());
            assert_eq!(s.total, F(500.0), "{after}: banked inside the window");
        }
        s.crashed();
        for _ in 0..60 {
            tick(&mut s, &down());
        }
        assert_eq!(s.total, F(500.0), "a wreck {after} ticks after the touch kept the trick");
        assert_eq!((s.pending, s.settling), (F::ZERO, F::ZERO));
    }
}

/// Ridden out, the same landing banks what the original banks, a second later; a wreck after that
/// is a wreck on its own and takes nothing. Opening the next combo banks it at once.
#[test]
fn a_clean_landing_still_banks() {
    let mut original = card(false);
    let points = trick(&mut original, 30);
    tick(&mut original, &landed());
    let banked = original.total;
    assert_eq!(banked, F(500.0) + points);

    let mut s = card(true);
    trick(&mut s, 30);
    let mut ticks = 0;
    while s.total != banked {
        tick(&mut s, &landed());
        ticks += 1;
        assert!(ticks <= 32, "never banked");
    }
    assert!((30..=32).contains(&ticks), "banked after {ticks} ticks, not about a second");
    s.crashed();
    assert_eq!(s.total, banked, "a wreck after the window took a bank");

    let mut next = card(true);
    trick(&mut next, 30);
    tick(&mut next, &landed());
    tick(&mut next, &landed());
    trick(&mut next, 10);
    assert_eq!(next.total, banked, "the next combo opening banks the last");
    next.crashed();
    assert_eq!(next.total, banked, "and a crash on the next takes only the next");
}

/// The event's card: a waiting freeride bank is banked before the join puts the total aside, and a
/// landing waiting when the rider's time is up counts at the end.
#[test]
fn the_event_banks_what_waits_at_the_join_and_the_end() {
    let mut s = card(true);
    let points = trick(&mut s, 30);
    tick(&mut s, &landed());
    let mut c = Competition::new(1000);
    let seat = c.join(&mut s);
    assert_eq!((s.total, s.pending), (F::ZERO, F::ZERO));
    let (_, profile) = c.finish(seat, &mut s);
    assert_eq!(profile, (F(500.0) + points).to_i32(), "the freeride bank was put aside with the total");

    let mut s = card(true);
    let mut c = Competition::new(1000);
    let seat = c.join(&mut s);
    // As the running manager leaves a card taking part.
    s.shut = false;
    let points = trick(&mut s, 30);
    tick(&mut s, &landed());
    let (_, profile) = c.finish(seat, &mut s);
    assert_eq!(profile, 500 + points.to_i32(), "the landing at the end counted");
}

/// The stand-in bike with its `[Wrecks]` numbers and wrecks on, the lockout as `lockout`.
fn session(lockout: bool) -> Sim {
    let tables = Arc::new(Tables::build());
    let limits = "
[Wrecks]
MaxWreckTime = 2
CannonballWreckTime = 5
FrontWreckSpeedLimit = 60
BackWreckSpeedLimit = 60
SideWreckSpeedLimit = 40
RacingDownWreckSpeedLimit = 40
FreestyleDownWreckSpeedLimit = 40
GroundPitchDownWreckAngle = 60
GroundPitchUpWreckAngle = 60
AirbornePitchDownWreckAngle = 120
AirbornePitchUpWreckAngle = 120
AirborneRollWreckAngle = 100
NoHanderWreckSpeedLimit = 400
NoHanderMaxRollAngle = 45
JoustWreckHeight = 2
CrossupWreck = 1
CrossupWreckTime = 0.3
";
    let phy = Phy::read_bytes(format!("{}{limits}", stock::BIKE).as_bytes(), &tables.ride.curve);
    let mut sim = Sim::ride(0x243, tables, phy, stock::SPAWN, stock::FACING, Set::default());
    sim.rider.sense.wrecks_enabled = true;
    sim.rider.score.lockout = lockout;
    sim
}

/// Dropped from 12 ft going forwards at 30 with a combo open on 3000 points, as a trick's would be:
/// up to the tick both wheels have banked it. Then the bike is slammed back down faster than the
/// down wreck's limit: the ticks from the bank to the wreck entry.
fn land_then_tip(sim: &mut Sim) -> usize {
    let at = Vf::new(stock::SPAWN.x(), stock::SPAWN.y() + 12.0, stock::SPAWN.z(), 1.0);
    let f = stock::FACING;
    sim.rider.ride.place_on(at, f, Vf::new(0.0, 1.0, 0.0, 0.0));
    sim.rider.ride.chassis.vel = Vf::new(f.x() * 30.0, 0.0, f.z() * 30.0, 0.0);
    let s = &mut sim.rider.score;
    s.open();
    s.push(F(3000.0), F::ZERO, F::ZERO, F::ONE, score::GRANULARITY, 0, score::flag::FLOOR);
    s.finish(F::ONE, true);
    let mut k = 0;
    while sim.rider.score.combo {
        let frame = sim.step(DT, &Controls::default(), &mut Flat::default());
        assert!(frame.wreck.is_none(), "the drop wrecked on its own");
        k += 1;
        assert!(k < 90, "never banked");
    }
    // Slammed back down: lifted a few feet and thrown at the ground faster than the down limit.
    let p = sim.bike_pos();
    sim.rider.ride.place_on(Vf::new(p.x(), p.y() + 3.0, p.z(), 1.0), f, Vf::new(0.0, 1.0, 0.0, 0.0));
    sim.rider.ride.chassis.vel = Vf::new(f.x() * 30.0, -80.0, f.z() * 30.0, 0.0);
    for after in 1..30 {
        if sim.step(DT, &Controls::default(), &mut Flat::default()).wreck.is_some() {
            return after;
        }
    }
    panic!("slammed down and never wrecked");
}

/// On a ride, through the rider's own wreck entry: the original keeps the landing's bank when the
/// bike comes down hard a few ticks later, the lockout loses it; and a clean landing banks either way.
#[test]
fn a_ride_that_wrecks_after_the_landing_loses_the_trick() {
    let mut original = session(false);
    let after = land_then_tip(&mut original);
    println!("wrecked {after} ticks after the bank");
    assert_eq!(original.rider.score.total, F(3000.0), "the original keeps it");

    let mut locked = session(true);
    assert_eq!(land_then_tip(&mut locked), after, "the lockout moved the ride");
    for _ in 0..60 {
        locked.step(DT, &Controls::default(), &mut Flat::default());
    }
    assert_eq!(locked.rider.score.total, F::ZERO, "the crash kept the trick");

    let mut clean = session(true);
    let at = Vf::new(stock::SPAWN.x(), stock::SPAWN.y() + 12.0, stock::SPAWN.z(), 1.0);
    let f = stock::FACING;
    clean.rider.ride.place_on(at, f, Vf::new(0.0, 1.0, 0.0, 0.0));
    clean.rider.ride.chassis.vel = Vf::new(f.x() * 30.0, 0.0, f.z() * 30.0, 0.0);
    let s = &mut clean.rider.score;
    s.open();
    s.push(F(3000.0), F::ZERO, F::ZERO, F::ONE, score::GRANULARITY, 0, score::flag::FLOOR);
    s.finish(F::ONE, true);
    for _ in 0..90 {
        assert!(clean.step(DT, &Controls::default(), &mut Flat::default()).wreck.is_none());
    }
    assert_eq!(clean.rider.score.total, F(3000.0), "a clean landing banks");
}
