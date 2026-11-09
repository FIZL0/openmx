//! I8: the experimental stick dead zone ([`Knob::StickDeadZone`]), off by default.
//!
//! Off, the ride's stick consumers carry the original's ±0.3 ([`AxisRange::PLAYER`], I7) and nothing
//! about a ride moves: the bike's file keeps the original's range and the hands, the plane's input
//! and a whole ride's hashes are what they were before the knob existed. On, the bike's file carries
//! a narrower range, and a stick byte inside ±0.3 but outside the new zone reaches the rider.

use std::sync::Arc;

use game_sim::Sim;
use game_sim::flight;
use game_sim::input::{AxisRange, Manager, PADS, Report};
use game_sim::modifiers::{DEAD_ZONES, Knob, Mods, ORIGINAL};
use game_sim::ride::Controls;
use game_sim::world::Flat;
use ps2_float::F;

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);

/// The report byte of the left stick's x axis (bytes 4 to 7 are right x, right y, left x, left y).
const LEFT_X: usize = 6;

/// A byte inside the original's ±0.3 and outside a ±0.05 zone: it maps to about 0.098 of the travel.
const SMALL: u8 = 140;

fn pads(left_x: u8) -> [Report; PADS] {
    let mut r = [Report::missing(); PADS];
    r[0] = Report::idle();
    r[0].0[LEFT_X] = left_x;
    r
}

/// The stock ride with `mods` laid on its bike's file, the way the shell lays them on before a ride.
fn sim_on(mods: &Mods) -> Sim {
    let mut sim = Sim::new(8);
    let mut phy = (*sim.phy).clone();
    mods.bike(0, &mut phy);
    sim.phy = Arc::new(phy);
    sim
}

fn with_zone(z: u16) -> Mods {
    let mut m = Mods::original();
    (0..game_sim::MAX_PLAYERS).for_each(|s| m.set(s, Knob::StickDeadZone, z));
    m
}

/// Off is the original's: the range a ride is handed is [`AxisRange::PLAYER`] field for field, and
/// the hands read through it are [`Controls::through`]'s on every byte of both left-stick axes.
#[test]
fn off_is_the_original_range() {
    assert_eq!(Mods::original().get(0, Knob::StickDeadZone), ORIGINAL);
    assert_eq!(Mods::original().stick_dead_zone(0), None);
    assert_eq!(sim_on(&Mods::original()).phy.stick, AxisRange::PLAYER);
    assert_eq!(AxisRange::player_with_dead_zone(F(0.3)), AxisRange::PLAYER);
    let table = game_sim::bindings::Table::SHIPPED;
    for byte in 0..=255u8 {
        for at in [LEFT_X, LEFT_X + 1] {
            let mut input = Manager::connected();
            let mut r = pads(0x7f);
            r[0].0[at] = byte;
            input.tick(DT, &r);
            let pad = input.pad(0);
            let (was, now) = (Controls::through(pad, &table), Controls::through_stick(pad, &table, &AxisRange::PLAYER));
            assert_eq!((was.stick.0.to_bits(), was.stick.1.to_bits()), (now.stick.0.to_bits(), now.stick.1.to_bits()), "byte {byte}");
            assert_eq!(format!("{:?}", flight::pad(pad)), format!("{:?}", flight::pad_through(pad, &AxisRange::PLAYER)), "byte {byte}");
        }
    }
}

/// Off rides bit for bit as a ride with no modifiers laid on at all: the same hash every tick, on a
/// stick held in the original's dead zone and on one held past it.
#[test]
fn off_rides_the_same_hashes() {
    for byte in [SMALL, 200] {
        let (mut bare, mut off) = (Sim::new(8), sim_on(&Mods::original()));
        for tick in 0..60 {
            bare.frame(DT, &pads(byte), &mut Flat::default());
            off.frame(DT, &pads(byte), &mut Flat::default());
            assert_eq!(bare.hash(), off.hash(), "byte {byte}, tick {tick}");
        }
    }
}

/// On, a small stick reaches the rider: byte 140 is zero through the original's ±0.3 and through a
/// ±0.10, and past a ±0.05 it is the stretched segment's value, on the bike's hands and the plane's
/// input alike. A ride held on it steers off the original's line.
#[test]
fn on_a_small_stick_reaches_the_rider() {
    let mut off = sim_on(&Mods::original());
    let mut wide = sim_on(&with_zone(10));
    let mut narrow = sim_on(&with_zone(5));
    assert_eq!(narrow.phy.stick, AxisRange::player_with_dead_zone(F(0.05)));
    assert_eq!(off.poll(DT, &pads(SMALL)).stick.0, F::ZERO, "inside the original's zone");
    assert_eq!(wide.poll(DT, &pads(SMALL)).stick.0, F::ZERO, "inside ±0.10");
    let reached = narrow.poll(DT, &pads(SMALL)).stick.0;
    let v = F(-1.0) + (F(140.0) / F(255.0)) * F(2.0);
    assert_eq!(reached, (v - F(0.05)) * (F(1.0) / (F(1.0) - F(0.05))));
    assert!(reached > F(0.04) && reached < F(0.06), "{reached:?}");
    let plane = flight::pad_through(narrow.input.pad(0), &narrow.phy.stick);
    // The plane's input step cubes the stick (I6), so the bank is small, but it is there.
    assert!(plane.aileron < F::ZERO, "the plane banks on it too: {:?}", plane.aileron);
    assert_eq!(flight::pad(narrow.input.pad(0)).aileron, F::ZERO);

    // A whole ride on it: the narrow zone's steers away from the original's, which rides straight.
    let (mut bare, mut on) = (Sim::new(8), sim_on(&with_zone(5)));
    let mut parted = None;
    for tick in 0..90 {
        bare.frame(DT, &pads(SMALL), &mut Flat::default());
        on.frame(DT, &pads(SMALL), &mut Flat::default());
        if parted.is_none() && bare.hash() != on.hash() {
            parted = Some(tick);
        }
    }
    assert!(parted.is_some(), "the small stick never reached the ride");
}

/// The knob's text form is what a tape, a preset and a lobby carry: a player's own, read back the
/// same, and refused outside its choices. The Controls row's ring: off, then narrower, round to off.
#[test]
fn it_rides_in_the_text_form() {
    let m = with_zone(5);
    assert_eq!(m.text(), "stick_dead_zone=5");
    assert_eq!(Mods::parse(&m.text()), Ok(m));
    let mut one = Mods::original();
    one.set(1, Knob::StickDeadZone, 10);
    assert_eq!(one.text(), "stick_dead_zone.2=10");
    assert_eq!(Mods::parse("stick_dead_zone.2=10"), Ok(one));
    assert!(Mods::parse("stick_dead_zone=40").is_err());
    assert!(game_sim::modifiers::online::seat_own(Knob::StickDeadZone), "online each player brings their own");
    assert!(!Knob::MENU.contains(&Knob::StickDeadZone), "its row is the Controls tab's");
    let k = Knob::StickDeadZone;
    let mut v = ORIGINAL;
    let mut seen = vec![k.shown(v)];
    for _ in 0..DEAD_ZONES.len() {
        v = k.step(v, true);
        seen.push(k.shown(v));
    }
    assert_eq!(seen, ["Off", "0.25", "0.20", "0.15", "0.10", "0.05", "0.00"]);
    assert_eq!(k.step(v, true), ORIGINAL);
    assert_eq!(k.step(ORIGINAL, false), 0);
}
