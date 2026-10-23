//! P136 — **Biplane max height** ([`Knob::PlaneCeiling`]) on the disc's `biplane.phy`.
//!
//! The plane's ceiling is its engine's altitude fade: the thrust's share falls in a straight line from
//! 1 at `y` 0 to 0 at `[Engine] MaxAltitude`. A full-throttle climb held at one airspeed (the pilot's
//! best-climb way: pull when fast, push when slow) climbs until the thinned thrust only holds the drag
//! at that speed, and levels off there. What this holds:
//! - at 100% the file is the original's to the bit, so the climb levels off where the original's does;
//! - at 200% it levels off about twice as high;
//! - Uncapped keeps climbing past the old MaxAltitude, at the rate it climbed low down;
//! - the knob is the flight's own through a session marker's put-back and through a change in flight
//!   (P98's retune), as the other plane knobs are.
//!
//! Skipped without a disc (`./kit extract MX.PSR`).

use game_sim::archive::{Psr, Res};
use game_sim::biplane::Pad;
use game_sim::flight::Flight;
use game_sim::modifiers::{self, Knob, Mods};
use game_sim::phy::{self, Plane};
use game_sim::ride::Tables;
use game_sim::world::Flat;
use ps2_float::{F, Vf};
use ps2_oracle::{Kit, skip};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

/// The 30 Hz clamp a level runs at.
const DT: F = F(game_sim::tick::DeltaTime::CAP_30);

/// The disc's biplane file, or `None` and a note.
fn the_plane() -> Option<Plane> {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/iso/MX.PSR");
    let path = if here.exists() {
        here
    } else {
        match Kit::load() {
            Ok(k) => k.ctx("iso").join("MX.PSR"),
            Err(why) => {
                skip(&why);
                return None;
            }
        }
    };
    let Ok(mut f) = File::open(&path) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", path.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find("machine.res").expect("machine.res is in MX.PSR");
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    let res = Res::parse(&bytes).expect("machine.res");
    let e = res.entries.iter().find(|e| e.name.eq_ignore_ascii_case(phy::BIPLANE_PHY)).expect("machine.res carries the plane's file");
    Some(Plane::read_bytes(&res.read(&bytes, e).expect("its bytes")))
}

/// A set with `knob` at `pct` for every player.
fn with(knob: Knob, pct: u16) -> Mods {
    let mut mods = Mods::original();
    for seat in 0..game_sim::MAX_PLAYERS {
        mods.set(seat, knob, pct);
    }
    mods
}

/// The airspeed the climb holds, ft/s.
const CLIMB_SPEED: f32 = 90.0;

/// The biplane on `mods`, from level flight at [`CLIMB_SPEED`] 50 ft up, at full throttle for `secs`
/// seconds, the stick held by a hand that keeps the wings level and the airspeed at [`CLIMB_SPEED`]
/// (the nose up when faster, down when slower): its height at each second.
fn climb(plane: &Plane, mods: &Mods, secs: usize) -> Vec<f32> {
    let tables = Tables::build();
    let mut plane = *plane;
    mods.plane(0, &mut plane);
    let mut f = Flight::new(&plane);
    f.attitude.flown = true;
    f.place(Vf::new(2048.0, 50.0, 2048.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), &tables.acos);
    f.body.velocity = Vf::new(0.0, 0.0, CLIMB_SPEED, 0.0);
    f.remeasure(&tables.acos);
    let mut ground = Flat::default();
    let (mut prev_roll, mut prev_nose) = (0.0f32, 0.0f32);
    let hz = (1.0 / DT.0).round() as usize;
    let mut heights = Vec::new();
    for i in 0..secs * hz {
        let roll = f.attitude.roll.to_f32();
        let roll_rate = (roll - prev_roll) / DT.0;
        prev_roll = roll;
        let aileron = (1.5 * roll + 0.4 * roll_rate).clamp(-1.0, 1.0);
        let nose = f.measure.forward.y();
        let nose_rate = (nose - prev_nose) / DT.0;
        prev_nose = nose;
        let speed = f.measure.airspeed.to_f32();
        let want = (0.03 * (speed - CLIMB_SPEED)).clamp(-0.2, 0.5);
        // A negative elevator is the stick back: the nose up.
        let elevator = (-3.0 * (want - nose) + 1.0 * nose_rate).clamp(-1.0, 1.0);
        let pad = Pad { forward: F::ONE, elevator: F(elevator), aileron: F(aileron), ..Pad::default() };
        f.frame(DT, pad, &mut ground, &tables);
        if (i + 1) % hz == 0 {
            heights.push(f.body.position.y());
        }
    }
    heights
}

#[test]
fn at_one_hundred_percent_the_ceiling_is_the_files() {
    let Some(stock) = the_plane() else { return };
    let mut mods = with(Knob::PlaneCeiling, modifiers::UNCAPPED);
    assert!(!mods.is_original());
    for seat in 0..game_sim::MAX_PLAYERS {
        mods.set(seat, Knob::PlaneCeiling, modifiers::ORIGINAL);
    }
    assert!(mods.is_original() && mods.text().is_empty());
    let mut p = stock;
    mods.plane(0, &mut p);
    // Debug shows every float to the bit.
    assert_eq!(format!("{p:?}"), format!("{stock:?}"), "Biplane max height at 100% touched the biplane");
    // The step past the percentages reads and writes, and goes no further.
    let top = with(Knob::PlaneCeiling, modifiers::UNCAPPED);
    assert_eq!(Mods::parse(&top.text()).unwrap(), top);
    assert!(Mods::parse(&format!("plane_ceiling={}", modifiers::UNCAPPED + 1)).is_err());
    assert!(Mods::parse(&format!("plane_turn={}", modifiers::UNCAPPED)).is_err(), "only the ceiling has the extra step");
    assert!(top.uncapped(0) && !with(Knob::PlaneCeiling, modifiers::MOST).uncapped(0));
    // The camera's: the fog's ceiling unless uncapped.
    assert_eq!(Mods::original().plane_eye_ceiling(0, F(200.0)), F(200.0));
    assert_eq!(with(Knob::PlaneCeiling, 200).plane_eye_ceiling(0, F(200.0)), F(200.0));
    assert_eq!(top.plane_eye_ceiling(0, F(200.0)), F::MAX);
    // Per seat: 2P's reaches 2P's plane alone.
    let mut two = Mods::original();
    two.set(1, Knob::PlaneCeiling, modifiers::UNCAPPED);
    let (mut a, mut b) = (stock, stock);
    two.plane(0, &mut a);
    two.plane(1, &mut b);
    assert_eq!(format!("{a:?}"), format!("{stock:?}"));
    assert_eq!(b.engine.max_altitude, F::MAX);
}

#[test]
fn the_climb_levels_off_k_times_as_high_and_uncapped_does_not() {
    let Some(stock) = the_plane() else { return };
    let top = stock.engine.max_altitude.to_f32();
    println!("the file's MaxAltitude: {top} ft");
    let secs = 600;
    let settle = |h: &[f32]| (h[h.len() - 1], h[h.len() - 1] - h[h.len() - 61]);
    let a = climb(&stock, &Mods::original(), secs);
    let (a_end, a_last) = settle(&a);
    println!("100%: {a_end:.1} ft after {secs} s, {a_last:.2} ft over the last minute; {:?}", a.iter().step_by(30).collect::<Vec<_>>());
    // Levelled off: under the fade's top, and all but still. When this was written the file's top
    // was 600 ft; 100% levelled at 445.7 ft inside a minute, 200% at 891.4 (x2.000), and uncapped
    // climbed 53.7 ft/s the whole ten minutes, to 32,084 ft.
    assert!(a_end < top && a_end > 0.3 * top, "100% ended at {a_end} ft, MaxAltitude {top}");
    assert!(a_last.abs() < 0.01 * a_end, "100% still moved {a_last} ft in the last minute");
    let b = climb(&stock, &with(Knob::PlaneCeiling, 200), secs);
    let (b_end, b_last) = settle(&b);
    println!("200%: {b_end:.1} ft after {secs} s, {b_last:.2} ft over the last minute (x{:.3})", b_end / a_end);
    assert!(((b_end / a_end) - 2.0).abs() < 0.02, "200% levelled off at {b_end} ft against {a_end}");
    assert!(b_last.abs() < 0.01 * b_end, "200% still moved {b_last} ft in the last minute");
    // Uncapped: still climbing past the old top, as fast as it climbed in its first half minute.
    let c = climb(&stock, &with(Knob::PlaneCeiling, modifiers::UNCAPPED), secs);
    let (c_end, c_last) = settle(&c);
    let early = (c[59] - c[29]) * 2.0;
    println!("uncapped: {c_end:.1} ft after {secs} s, {c_last:.1} ft over the last minute, {early:.1} early");
    assert!(c_end > 2.0 * top, "uncapped only reached {c_end} ft, the old MaxAltitude {top}");
    assert!(c_last > 0.9 * early, "uncapped climbed {c_last} ft in the last minute, {early} early");
}

#[test]
fn the_ceiling_survives_a_put_back_and_moves_in_flight() {
    let Some(stock) = the_plane() else { return };
    let tables = Tables::build();
    let mut f = Flight::new(&stock);
    f.attitude.flown = true;
    f.place(Vf::new(2048.0, 300.0, 2048.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), &tables.acos);
    f.body.velocity = Vf::new(0.0, 0.0, CLIMB_SPEED, 0.0);
    f.remeasure(&tables.acos);
    let mark = f.clone();
    let mut ground = Flat::default();
    for _ in 0..30 {
        f.frame(DT, Pad { forward: F::ONE, ..Pad::default() }, &mut ground, &tables);
    }
    // Moved to 200% in flight (the pause closing, P98): the fade's top doubles where the plane is.
    let twice = with(Knob::PlaneCeiling, 200);
    let at = f.body.position;
    twice.replane(0, &stock, &mut f);
    assert_eq!(f.engine.max_altitude, stock.engine.max_altitude * F(2.0));
    assert_eq!(f.body.position, at);
    // Back to the marker kept on the original's numbers: the place is the marker's, the ceiling the
    // plane's now.
    f.put_back(&mark);
    assert_eq!(f.body.position, mark.body.position);
    assert_eq!(f.engine.max_altitude, stock.engine.max_altitude * F(2.0), "the put-back took the ceiling back to the marker's");
    // Uncapped, and back again: the same.
    with(Knob::PlaneCeiling, modifiers::UNCAPPED).replane(0, &stock, &mut f);
    f.put_back(&mark);
    assert_eq!(f.engine.max_altitude, F::MAX);
    Mods::original().replane(0, &stock, &mut f);
    f.put_back(&mark);
    assert_eq!(format!("{:?}", f.engine), format!("{:?}", mark.engine), "back on the original's numbers, the marker's engine to the bit");
}
