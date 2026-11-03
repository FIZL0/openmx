//! MD39a's captures of MX vs. ATV's (SLUS-21104) five cars: one generated tape
//! (`context/recordings/md39a/drive.p2m2`, research/md39a/drive.pad) played from each car's
//! Desert Oasis Free Ride spawn in PCSX2, with the car object, its body, engine and four tires
//! read every tick (research/md39a/car.toml) into `context/capture/md39a/<car>.csv`. The state
//! each drive starts from is `context/mxvatv-ps2/md39a/<car>.p2s`, its RAM `<car>.ee.bin`
//! (research/journal/2026-11-03-md39a-mxvatv-car-captures). Without them every test here skips.
//!
//! The tests hold the captures to what they claim: the object is a car (its vtable is car
//! 0x003cf130's with a few slots overridden, research/oracle.txt # MD39), it is the named car (its
//! CofG is that car's file's), and the rows follow the tape.

use std::collections::HashMap;

use game_sim::car::{Car, Kind};
use ps2_oracle::{Kit, skip};

/// MXVehicleCar's vtable. The buggy, the baja and the sandrail are built as MXVehicleBuggy
/// (0x003cec60), the golf cart and the truck have their own classes.
const CAR_VTABLE: u32 = 0x003c_f130;
/// The EE's vsync counter, +2 a game tick.
const VSYNC: usize = 0x003f_a6d8;
/// Rows a drive has: the tape's 735 ticks less the 5 the recorder leaves for the movie's end.
const ROWS: usize = 730;

struct Drive {
    name: &'static str,
    phy: &'static str,
    kind: Kind,
    /// The player's car object (the class's other object in RAM is an idle second car).
    obj: u32,
    vtable: u32,
    /// Ticks a row lags the tape: the input words of row `i` are tape tick `i - lag`. Two for the
    /// buggy and baja states, one for the other three; steady through each drive.
    lag: usize,
}

const DRIVES: [Drive; 5] = [
    Drive { name: "buggy", phy: "buggy.phy", kind: Kind::Buggy, obj: 0x0159_8e10, vtable: 0x003c_ec60, lag: 2 },
    Drive { name: "baja", phy: "baja.phy", kind: Kind::Baja, obj: 0x0159_8e10, vtable: 0x003c_ec60, lag: 2 },
    Drive { name: "sandrail", phy: "sandrail.phy", kind: Kind::Buggy, obj: 0x01b1_58a0, vtable: 0x003c_ec60, lag: 1 },
    Drive { name: "golf", phy: "golfcart.phy", kind: Kind::Buggy, obj: 0x01b0_dac0, vtable: 0x003c_f600, lag: 1 },
    Drive { name: "truck", phy: "monstertruck.phy", kind: Kind::MonsterTruck, obj: 0x0159_8e10, vtable: 0x003c_ff80, lag: 1 },
];

fn u32_at(b: &[u8], a: usize) -> u32 {
    u32::from_le_bytes(b[a..a + 4].try_into().unwrap())
}

fn f32_at(b: &[u8], a: usize) -> f32 {
    f32::from_le_bytes(b[a..a + 4].try_into().unwrap())
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

/// A csv as header → column, every value a string.
fn table(text: &str) -> (HashMap<String, usize>, Vec<Vec<&str>>) {
    let mut lines = text.lines();
    let head = lines.next().unwrap_or("").split(',').enumerate().map(|(i, h)| (h.to_string(), i)).collect();
    (head, lines.map(|l| l.split(',').collect()).collect())
}

struct Got {
    ram: Vec<u8>,
    phy: Vec<u8>,
    csv: String,
    tape: String,
}

fn load(kit: &Kit, d: &Drive) -> Option<Got> {
    let paths = [
        kit.ctx("mxvatv-ps2/md39a").join(format!("{}.ee.bin", d.name)),
        kit.ctx("mxvatv-ps2/md39/phy").join(d.phy),
        kit.ctx("capture/md39a").join(format!("{}.csv", d.name)),
        kit.ctx("recordings/md39a/drive.p2m2.ticks.csv"),
    ];
    let mut read = paths.iter().map(std::fs::read);
    let mut next = || match read.next().unwrap() {
        Ok(b) => Some(b),
        Err(_) => None,
    };
    let got = (|| {
        Some(Got {
            ram: next()?,
            phy: next()?,
            csv: String::from_utf8(next()?).unwrap(),
            tape: String::from_utf8(next()?).unwrap(),
        })
    })();
    if got.is_none() {
        skip(&format!(
            "{}: one of {paths:?} is missing — research/journal/2026-11-03-md39a-mxvatv-car-captures makes them",
            d.name
        ));
    }
    got
}

fn drives() -> Option<Vec<(&'static Drive, Got)>> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    DRIVES.iter().map(|d| Some((d, load(&kit, d)?))).collect()
}

/// Each state's car is a car, and the one named: the object's vtable is car 0x003cf130's but for
/// 7 to 9 slots (the factory's, the loader's tail, drawing, the update's head), and its CofG, the
/// one field the buggy, baja and sandrail differ in at a glance, is the car's file's.
#[test]
fn each_state_holds_the_named_car() {
    let Some(drives) = drives() else { return };
    for (d, g) in &drives {
        let o = d.obj as usize;
        assert_eq!(u32_at(&g.ram, o + 4), d.vtable, "{}: the object's vtable", d.name);
        let slots = 144;
        let shared = (0..slots)
            .filter(|i| u32_at(&g.ram, d.vtable as usize + 4 * i) == u32_at(&g.ram, CAR_VTABLE as usize + 4 * i))
            .count();
        assert!(shared >= 135, "{}: shares only {shared} of car 0x003cf130's first {slots} slots", d.name);
        let car = Car::read_bytes(&g.phy, d.kind);
        let ram: Vec<u32> = (0..3).map(|k| u32_at(&g.ram, o + 0x10f0 + 4 * k)).collect();
        assert_eq!(ram, car.mass_properties.cofg.to_bits()[..3], "{}: CofG at +0x10f0 against {}", d.name, d.phy);
    }
}

/// Every row of each drive is a tick of the named car, one after another from the state.
#[test]
fn the_captures_load_and_name_the_cars_vtable() {
    let Some(drives) = drives() else { return };
    for (d, g) in &drives {
        let (h, rows) = table(&g.csv);
        assert_eq!(rows.len(), ROWS, "{}: rows", d.name);
        let start = u32_at(&g.ram, VSYNC);
        for (i, r) in rows.iter().enumerate() {
            assert_eq!(r[h["vsync"]].parse::<u32>().unwrap(), start + 2 + 2 * i as u32, "{} row {i}: vsync", d.name);
            assert_eq!(r[h["vptr"]].parse::<u32>().unwrap(), d.vtable, "{} row {i}: vptr", d.name);
            assert_eq!(u32_at(&unhex(r[h["obj0"]]), 4), d.vtable, "{} row {i}: the raw block's vptr", d.name);
        }
    }
}

/// The car's input words follow the tape, one or two ticks behind (`Drive::lag`): gas +0x314
/// (Cross), hand brake +0x1138 (Circle), stick steer +0x11c8 (left stick x), and the brake +0x310
/// is on whenever Square is (it is also on at rest). And the drive is the one asked for: the car
/// leaves the ground with all four tires, stops under the brake, and gets above 40 ft/s.
#[test]
fn the_drives_follow_the_tape() {
    let Some(drives) = drives() else { return };
    for (d, g) in &drives {
        let (th, ticks) = table(&g.tape);
        let (h, rows) = table(&g.csv);
        let (mut air, mut top, mut stopped) = (0, 0.0f32, false);
        for (i, r) in rows.iter().enumerate().skip(d.lag) {
            let t = &ticks[i - d.lag];
            let held = |b: &str| if t[th["buttons"]].split(' ').any(|x| x == b) { 1.0 } else { 0.0 };
            let lx: f32 = t[th["lx"]].parse().unwrap();
            let obj = unhex(r[h["obj0"]]);
            let obj1 = unhex(r[h["obj1"]]);
            let at = |off: usize| if off < 0xc00 { f32_at(&obj, off) } else { f32_at(&obj1, off - 0xc00) };
            assert_eq!(at(0x314), held("cross"), "{} row {i}: gas", d.name);
            assert_eq!(at(0x1138), held("circle"), "{} row {i}: hand brake", d.name);
            assert_eq!(at(0x11c8), lx, "{} row {i}: steer", d.name);
            if held("square") == 1.0 {
                assert_eq!(at(0x310), 1.0, "{} row {i}: brake", d.name);
            }
            let v: Vec<f32> = ["x", "y", "z"].iter().map(|c| r[h[&format!("vel.{c}")]].parse().unwrap()).collect();
            let speed = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            top = top.max(speed);
            stopped |= held("square") == 1.0 && speed < 1.0;
            if (0..4).all(|k| r[h[&format!("tire{k}.touch")]] == "0") {
                air += 1;
            }
        }
        assert!(air >= 10, "{}: only {air} rows with all four tires off", d.name);
        assert!(stopped, "{}: never stopped under the brake", d.name);
        assert!(top > 40.0, "{}: top speed {top}", d.name);
    }
}
