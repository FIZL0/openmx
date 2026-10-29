//! The car's engine and automatic gearbox (S149c) against the original's, two ways.
//!
//! - **The interpreter**: the engine step, the shift, the gear and reverse setters and the reset run
//!   in the original on the S149a images, each car's engine object filled with generated words —
//!   every running word, the constants around the file's, the pedals and the driven tires — and every
//!   word they write compared with the port's. The engine's smooth noise is hooked with a stand-in
//!   both sides share (it is proven on its own in tests/noise.rs).
//! - **The captured drives**: S149a's four drives, the baja, the buggy and the truck twice, from the
//!   file through the loader. A capture row is read after a frame's four physics substeps and the next
//!   frame's shift, so each pair of rows is four engine steps and a shift. Where all four substeps had
//!   the clutch in (the pedal, or the box's own shift), the engine reads no tire and the whole frame
//!   replays from the first row: the revs, the gear and every word the step writes but the sound's
//!   pitch. Elsewhere the last substep replays from what the row keeps of it: the revs before it (out
//!   of the slip, the wheel revs and the engine braking it wrote) and the driven wheels' spin (out of
//!   the wheel revs). Then the shift, from the row's revs, load, contacts and clutch.

use game_sim::archive::{Psr, Res};
use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::car::{self, Car, DriveTire, Drivetrain, Kind, Pedals, Shifted};
use ps2_float::{F, fpu};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::cell::Cell;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::rc::Rc;

/// The car in every S149a image and capture, and its engine and tire pointers.
const CAR: u32 = 0x00605270;
const ENGINE: u32 = 0x544;
const TIRES: u32 = 0x548;
/// The car's pedal words, which the engine reads through its pointers.
const THROTTLE: u32 = 0x3ec;
const CLUTCH: u32 = 0x3f0;
const BRAKE: u32 = 0x3f8;
/// The tire words the engine reads and writes.
const T_MAX_COMPRESSION: u32 = 0x3c;
const T_TOP_SPEED: u32 = 0xa0;
const T_CONTACT: u32 = 0xc0;
const T_COMPRESSION: u32 = 0xdc;
const T_SPIN: u32 = 0x1a4;
const T_DRIVE: u32 = 0x1f0;
const T_SLIP: u32 = 0x20c;

const CASES: usize = 12_000;
const BUDGET: u64 = 200_000;

struct Drive {
    kind: Kind,
    name: &'static str,
    image: &'static str,
}
const DRIVES: [Drive; 3] = [
    Drive { kind: Kind::Baja, name: "baja", image: "s149a-baja-switched.ee.bin" },
    Drive { kind: Kind::Buggy, name: "buggy", image: "s149a-buggy-switched.ee.bin" },
    Drive { kind: Kind::MonsterTruck, name: "truck", image: "s149a-truck-switched.ee.bin" },
];

// ---- The engine object's words, both ways ----

/// The running words and constants of an engine object, over a port drivetrain whose tables are
/// already right. `w` reads a word at an offset into the object.
fn fill(d: &mut Drivetrain, w: &dyn Fn(u32) -> u32) {
    let f = |o: u32| F::from_bits(w(o));
    d.four_wheel = w(0x38) != 0 && w(0x3c) != 0;
    d.idle_throttle = f(0x4c);
    d.engine_braking = f(0x50);
    d.final_gear_ratio = f(0x54);
    d.transmission_loss = f(0x58);
    d.inv_inertia = f(0x60);
    d.pump_torque_scale = f(0x64);
    d.max_k = f(0x68);
    d.launch_boost = f(0x6c);
    d.min_rpm = f(0x70);
    d.tcs = w(0x74) != 0;
    d.min_traction_slip = f(0x78);
    d.max_traction_slip = f(0x7c);
    d.nox_on = w(0x80) != 0;
    d.nox_left = f(0x84);
    d.nox_burn_rate = f(0x88);
    d.nox_hp_boost = f(0x8c);
    let s = &mut d.shift_points;
    s.low_load_rpm = f(0x90);
    d.high_load_rpm = f(0x94);
    s.low_load_up_shift_rpm = f(0x98);
    s.high_load_up_shift_rpm = f(0x9c);
    s.low_load_down_shift_rpm = f(0xa0);
    s.high_load_down_shift_rpm = f(0xa4);
    s.shift_time = f(0xa8);
    d.shift_timer = f(0xac);
    d.shifting_up = w(0xb0) != 0;
    s.shift_up_duration = f(0xb4);
    s.shift_up_throttle = f(0xb8);
    s.shift_down_duration = f(0xbc);
    s.shift_down_throttle = f(0xc0);
    d.shift_throttle_timer = f(0xc4);
    d.reverse = w(0xc8) != 0;
    d.gear = w(0xcc) as i32;
    d.ratio = f(0xe4);
    d.inv_ratio = f(0xe8);
    d.wheel_rpm = f(0xec);
    d.max_pump_torque = f(0xf0);
    d.converter_torque = f(0xf4);
    d.multiplication = f(0xf8);
    d.output_torque = f(0xfc);
    d.cursor_index = w(0x104);
    d.cursor_frac = f(0x108);
    d.engine_torque = f(0x110);
    d.braking_torque = f(0x114);
    d.drive_torque = [f(0x118), f(0x11c), f(0x120), f(0x124)];
    d.rpm = f(0x128);
    d.slip_rpm = f(0x12c);
    d.redline = f(0x130);
    d.sound_clock = f(0x134);
    d.pitch = f(0x138);
}

/// Every word [`fill`] reads, named, as the port holds it.
fn words(d: &Drivetrain) -> Vec<(&'static str, u32, u32)> {
    let b = |x: F| x.to_bits();
    let s = &d.shift_points;
    vec![
        ("nox left", 0x84, b(d.nox_left)),
        ("shift timer", 0xac, b(d.shift_timer)),
        ("shifting up", 0xb0, d.shifting_up as u32),
        ("shift throttle timer", 0xc4, b(d.shift_throttle_timer)),
        ("reverse", 0xc8, d.reverse as u32),
        ("gear", 0xcc, d.gear as u32),
        ("ratio", 0xe4, b(d.ratio)),
        ("1/ratio", 0xe8, b(d.inv_ratio)),
        ("wheel rpm", 0xec, b(d.wheel_rpm)),
        ("converter torque", 0xf4, b(d.converter_torque)),
        ("multiplication", 0xf8, b(d.multiplication)),
        ("output torque", 0xfc, b(d.output_torque)),
        ("cursor index", 0x104, d.cursor_index),
        ("cursor weight", 0x108, b(d.cursor_frac)),
        ("engine torque", 0x110, b(d.engine_torque)),
        ("engine braking", 0x114, b(d.braking_torque)),
        ("drive 0", 0x118, b(d.drive_torque[0])),
        ("drive 1", 0x11c, b(d.drive_torque[1])),
        ("drive 2", 0x120, b(d.drive_torque[2])),
        ("drive 3", 0x124, b(d.drive_torque[3])),
        ("rpm", 0x128, b(d.rpm)),
        ("slip rpm", 0x12c, b(d.slip_rpm)),
        ("sound clock", 0x134, b(d.sound_clock)),
        ("pitch", 0x138, b(d.pitch)),
        // Constants nothing writes: a port that wrote one would show here.
        ("launch boost", 0x6c, b(d.launch_boost)),
        ("shift time", 0xa8, b(s.shift_time)),
        ("redline", 0x130, b(d.redline)),
    ]
}

fn compare(what: &str, d: &Drivetrain, w: &dyn Fn(u32) -> u32, skip_words: &[&str]) -> Vec<String> {
    words(d)
        .into_iter()
        .filter(|(name, _, _)| !skip_words.contains(name))
        .filter(|&(_, off, got)| w(off) != got)
        .map(|(name, off, got)| {
            format!("{what}: {name} (+{off:#x}) is {:e} ({:#x}) in the game, {:e} ({got:#x}) in the port",
                    f32::from_bits(w(off)), w(off), f32::from_bits(got))
        })
        .collect()
}

fn floats(ee: &Ee, at: u32, n: u32) -> Vec<F> {
    (0..n).map(|k| F(ee.f32_at(at + 4 * k))).collect()
}

// ---- The interpreter ----

/// A deterministic generator for the cases.
struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    fn one_in(&mut self, n: u64) -> bool {
        self.next() % n == 0
    }
    /// Mostly `x` scaled by up to half either way, sometimes zero, sometimes negated or exact.
    fn around(&mut self, x: f32) -> f32 {
        match self.next() % 16 {
            0 => 0.0,
            1 => -x,
            2 | 3 => x,
            _ => x * self.range(0.5, 1.5),
        }
    }
    /// A pedal: often exactly 0 or 1.
    fn pedal(&mut self) -> f32 {
        match self.next() % 5 {
            0 | 1 => 0.0,
            2 => 1.0,
            _ => self.range(-0.1, 1.2),
        }
    }
    /// A timer: often run out, sometimes exactly 0, sometimes running.
    fn timer(&mut self, full: f32) -> f32 {
        match self.next() % 4 {
            0 => 0.0,
            1 => -self.range(0.0, 0.04),
            _ => self.range(-0.01, full),
        }
    }
}

/// The interpreter on a drive's image, with `$gp` as boot leaves it, and the engine object.
fn image(kit: &Kit, d: &Drive) -> Option<(Ee, u32)> {
    let path = kit.ctx("states").join(d.image);
    if !path.exists() {
        skip(&format!("{} is not there: S149a's boot.py into the {}, switched to, then `./kit state ram`", path.display(), d.name));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let e = ee.u32_at(CAR + ENGINE);
    assert_eq!(ee.u32_at(e + 0x40), CAR + THROTTLE, "no engine wired to the car at {CAR:#x} in {}", path.display());
    Some((ee, e))
}

/// The port's drivetrain for the engine object in RAM: its tables out of RAM, then its words.
fn from_ram(ee: &Ee, e: u32) -> Drivetrain {
    let n = ee.u32_at(e + 0xd0);
    let gears = ee.u32_at(e + 0xdc);
    let mut d = Drivetrain::new(&car::Engine {
        drive_train: 1,
        driven: [None; 4],
        rpm: floats(ee, ee.u32_at(e + 0xd4), n),
        hp: vec![],
        torque: floats(ee, ee.u32_at(e + 0xd8), n),
        engine_braking: F::ZERO,
        max_rpm: None,
        gear_ratios: floats(ee, ee.u32_at(e + 0xe0), gears),
        final_gear_ratio: F::ZERO,
        first: None,
        inertia: F::ZERO,
        inv_inertia: None,
        transmission_loss: F::ZERO,
        min_rpm: F::ZERO,
        idle_throttle: F::ZERO,
        max_k: F::ZERO,
        pump_torque_scale: F::ZERO,
        max_pump_torque: F::ZERO,
        shift: car::Shift {
            low_load_rpm: F::ZERO,
            low_load_up_shift_rpm: F::ZERO,
            low_load_down_shift_rpm: F::ZERO,
            high_load_rpm: None,
            high_load_up_shift_rpm: F::ZERO,
            high_load_down_shift_rpm: F::ZERO,
            shift_time: F::ZERO,
            shift_up_duration: F::ZERO,
            shift_up_throttle: F::ZERO,
            shift_down_duration: F::ZERO,
            shift_down_throttle: F::ZERO,
        },
        nox_volume: F::ZERO,
        nox_burn_rate: F::ZERO,
        nox_hp_boost: F::ZERO,
        tcs_enabled: 0,
        min_traction_slip: F::ZERO,
        max_traction_slip: F::ZERO,
    });
    fill(&mut d, &|o| ee.u32_at(e + o));
    d
}

fn tire_from_ram(ee: &Ee, t: u32) -> DriveTire {
    DriveTire {
        contact: ee.u32_at(t + T_CONTACT) != 0,
        spin: F(ee.f32_at(t + T_SPIN)),
        slip_ratio: F(ee.f32_at(t + T_SLIP)),
        compression: F(ee.f32_at(t + T_COMPRESSION)),
        max_compression: F(ee.f32_at(t + T_MAX_COMPRESSION)),
        drive_torque: F(ee.f32_at(t + T_DRIVE)),
        top_wheel_speed: F(ee.f32_at(t + T_TOP_SPEED)),
    }
}

/// The engine's four drive slots' tires, as the port takes them; an empty slot is a zeroed tire.
fn slots_from_ram(ee: &Ee, e: u32) -> [DriveTire; 4] {
    [0, 1, 2, 3].map(|k| match ee.u32_at(e + 0x30 + 4 * k) {
        0 => DriveTire::default(),
        t => tire_from_ram(ee, t),
    })
}

fn compare_tires(case: usize, ee: &Ee, e: u32, port: &[DriveTire; 4]) -> Vec<String> {
    let mut bad = vec![];
    for k in 0..4 {
        let t = ee.u32_at(e + 0x30 + 4 * k as u32);
        if t == 0 {
            continue;
        }
        for (name, off, got) in [("drive torque", T_DRIVE, port[k].drive_torque), ("top wheel speed", T_TOP_SPEED, port[k].top_wheel_speed)] {
            if ee.u32_at(t + off) != got.to_bits() {
                bad.push(format!("case {case}: slot {k}'s {name} is {:e} in the game, {:e} in the port", ee.f32_at(t + off), got.to_f32()));
            }
        }
    }
    bad
}

/// The stand-in both sides read for the engine's smooth noise.
fn noise(t: f32) -> f32 {
    let x = t.to_bits().wrapping_mul(0x9e37_79b9) >> 8;
    x as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
}

fn the_curve_table(ee: &Ee) -> CurveTable {
    let base = ee.u32_at(addr("curve_table_ptr").unwrap());
    let entries: Vec<[f32; 2]> =
        (0..CURVE_ENTRIES as u32).map(|k| [ee.f32_at(base + 8 * k), ee.f32_at(base + 8 * k + 4)]).collect();
    CurveTable::from_entries(&entries)
}

/// Fill the engine object and its tires with a generated case.
fn generate(ee: &mut Ee, e: u32, shipped: &[u32], tires: [u32; 4], pick: &mut Pick) {
    let f = |o: u32| f32::from_bits(shipped[o as usize / 4]);
    let n = ee.u32_at(e + 0xd0);
    let redline = f(0x130);
    let mut set = |ee: &mut Ee, o: u32, v: f32| ee.set_f32(e + o, v);
    for o in [0x4c, 0x50, 0x58, 0x60, 0x64, 0x68, 0x70, 0x78, 0x7c, 0x88, 0x8c, 0x90, 0x94, 0x98, 0x9c, 0xa0, 0xa4, 0xb4, 0xb8, 0xbc, 0xc0, 0xf0] {
        let v = pick.around(f(o));
        set(ee, o, v);
    }
    // Some ties, for the setters' equal-ends branches.
    if pick.one_in(8) {
        let v = ee.f32_at(e + 0x90);
        set(ee, 0x94, v);
    }
    if pick.one_in(8) {
        let v = ee.f32_at(e + 0x78);
        set(ee, 0x7c, v);
    }
    if pick.one_in(8) {
        set(ee, 0x70, redline);
    }
    let v = pick.range(0.5, 5.0);
    set(ee, 0x6c, if pick.one_in(3) { 1.0 } else { v });
    let v = pick.range(0.0, 2.0);
    set(ee, 0x84, if pick.one_in(3) { 0.0 } else { v });
    ee.set_u32(e + 0x74, (pick.next() % 2) as u32);
    ee.set_u32(e + 0x80, (pick.next() % 2) as u32);
    let v = pick.timer(1.0);
    set(ee, 0xac, v);
    ee.set_u32(e + 0xb0, (pick.next() % 2) as u32);
    let v = pick.timer(0.35);
    set(ee, 0xc4, if pick.one_in(2) { 0.0 } else { v });
    // Reverse now and then, with the ratio it makes.
    let gears = ee.u32_at(e + 0xdc);
    let gear = 1 + (pick.next() % gears as u64) as u32;
    let reverse = pick.one_in(8);
    ee.set_u32(e + 0xc8, reverse as u32);
    ee.set_u32(e + 0xcc, if reverse { 1 } else { gear });
    let ratios = ee.u32_at(e + 0xe0);
    let ratio = f(0x54) * if reverse { -ee.f32_at(ratios) } else { ee.f32_at(ratios + 4 * (gear - 1)) };
    set(ee, 0xe4, ratio);
    set(ee, 0xe8, 1.0 / ratio);
    let v = pick.range(-300.0, 1.2 * redline);
    set(ee, 0x128, if pick.one_in(16) { 0.0 } else if pick.one_in(16) { redline } else { v });
    let v = pick.range(-3000.0, 6000.0);
    set(ee, 0x12c, v);
    ee.set_u32(e + 0x104, (pick.next() % u64::from(n)) as u32);
    let v = pick.unit();
    set(ee, 0x108, v);
    let v = pick.range(0.0, 200.0);
    set(ee, 0x134, v);
    // A four-wheel drive now and then: the front pair in slots 2 and 3.
    let four = pick.one_in(4);
    ee.set_u32(e + 0x38, if four { tires[0] } else { 0 });
    ee.set_u32(e + 0x3c, if four { tires[1] } else { 0 });
    for t in tires {
        ee.set_u32(t + T_CONTACT, if pick.one_in(4) { 0 } else { 1 });
        let v = if pick.one_in(10) { 0.0 } else { pick.range(-40.0, 160.0) };
        ee.set_f32(t + T_SPIN, v);
        let v = pick.range(-1.5, 1.5);
        ee.set_f32(t + T_SLIP, v);
        let v = pick.range(0.0, 1.5);
        ee.set_f32(t + T_COMPRESSION, v);
        let v = pick.range(-50.0, 50.0);
        ee.set_f32(t + T_DRIVE, v);
    }
    let throttle = pick.pedal();
    let clutch = pick.pedal();
    let brake = if pick.one_in(3) { pick.range(0.0, 2.0) } else { 0.0 };
    ee.set_f32(CAR + THROTTLE, throttle);
    ee.set_f32(CAR + CLUTCH, clutch);
    ee.set_f32(CAR + BRAKE, brake);
}

fn pedals_from_ram(ee: &Ee) -> Pedals {
    Pedals { throttle: F(ee.f32_at(CAR + THROTTLE)), clutch: F(ee.f32_at(CAR + CLUTCH)), brake: F(ee.f32_at(CAR + BRAKE)) }
}

#[test]
fn the_engine_step_matches_the_original() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    let step = addr("car_engine_step").unwrap();
    for d in &DRIVES {
        let Some((mut ee, e)) = image(&kit, d) else { continue };
        ee.hooks.insert(addr("smooth_noise").unwrap(), Box::new(|ee| {
            let t = ee.f(12);
            ee.set_f(0, noise(t));
        }));
        let curve = the_curve_table(&ee);
        assert_eq!(ee.f32_at(addr("curve_table_invstep").unwrap()), 954.77045, "the curve table's scale is the port's");
        let shipped: Vec<u32> = (0..0x50).map(|k| ee.u32_at(e + 4 * k)).collect();
        let tires = [0, 1, 2, 3].map(|k| ee.u32_at(CAR + TIRES + 4 * k));
        let mut pick = Pick(0x6a09_e667_f3bc_c908 ^ d.kind as u64);
        let mut bad = vec![];
        for case in 0..CASES {
            generate(&mut ee, e, &shipped, tires, &mut pick);
            let dt = if pick.one_in(2) { 0.03333 / 4.0 } else { pick.range(0.0, 0.05) };
            let mut port = from_ram(&ee, e);
            let mut wheels = slots_from_ram(&ee, e);
            let pedals = pedals_from_ram(&ee);
            ee.call(step, &[u64::from(e)], &[dt], BUDGET);
            port.step(pedals, &mut wheels, F(dt), &curve, &mut |t: F| F(noise(t.to_f32())));
            bad.extend(compare(&format!("{} case {case}", d.name), &port, &|o| ee.u32_at(e + o), &[]));
            bad.extend(compare_tires(case, &ee, e, &wheels));
            assert!(bad.len() < 20, "{}", bad.join("\n"));
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
        println!("the {}'s engine step: {CASES} cases, bit-exact", d.name);
    }
}

#[test]
fn the_shift_matches_the_original() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    let shift = addr("car_engine_shift").unwrap();
    for d in &DRIVES {
        let Some((mut ee, e)) = image(&kit, d) else { continue };
        let sounds = Rc::new(Cell::new(0u32));
        let heard = sounds.clone();
        ee.hooks.insert(addr("car_shift_sound").unwrap(), Box::new(move |_| heard.set(heard.get() + 1)));
        let shipped: Vec<u32> = (0..0x50).map(|k| ee.u32_at(e + 4 * k)).collect();
        let tires = [0, 1, 2, 3].map(|k| ee.u32_at(CAR + TIRES + 4 * k));
        let mut pick = Pick(0xbb67_ae85_84ca_a73b ^ d.kind as u64);
        let redline = f32::from_bits(shipped[0x130 / 4]);
        let (mut ups, mut downs) = (0, 0);
        let mut bad = vec![];
        for case in 0..CASES {
            generate(&mut ee, e, &shipped, tires, &mut pick);
            // The shift points matter only near them: put the revs there often.
            if pick.one_in(2) {
                let point = ee.f32_at(e + [0x98, 0x9c, 0xa0, 0xa4][(pick.next() % 4) as usize]);
                let v = if pick.one_in(4) { point } else { point + pick.range(-300.0, 300.0) };
                ee.set_f32(e + 0x128, v.min(redline));
            }
            if pick.one_in(3) {
                ee.set_f32(e + 0xac, 0.0);
                ee.set_f32(CAR + CLUTCH, 0.0);
            }
            let dt = if pick.one_in(2) { 0.03333 } else { pick.range(0.0, 0.05) };
            let mut port = from_ram(&ee, e);
            let mut wheels = slots_from_ram(&ee, e);
            let clutch = F(ee.f32_at(CAR + CLUTCH));
            sounds.set(0);
            ee.set_u32(e + 0x13c, 1);
            ee.call(shift, &[u64::from(e)], &[dt], BUDGET);
            let shifted = port.shift(clutch, F(dt), &mut wheels);
            match shifted {
                Some(Shifted::Up) => ups += 1,
                Some(Shifted::Down) => downs += 1,
                None => {}
            }
            if sounds.get() != shifted.is_some() as u32 {
                bad.push(format!("{} case {case}: the game played {} shift sounds, the port shifted {shifted:?}", d.name, sounds.get()));
            }
            bad.extend(compare(&format!("{} case {case}", d.name), &port, &|o| ee.u32_at(e + o), &[]));
            bad.extend(compare_tires(case, &ee, e, &wheels));
            assert!(bad.len() < 20, "{}", bad.join("\n"));
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
        assert!(ups > 500 && downs > 500, "the {}'s cases shift too seldom: {ups} up, {downs} down", d.name);
        println!("the {}'s shift: {CASES} cases ({ups} up, {downs} down), bit-exact", d.name);
    }
}

#[test]
fn the_gear_setters_match_the_original() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    let (set_gear, set_reverse, reset) =
        (addr("car_engine_set_gear").unwrap(), addr("car_engine_set_reverse").unwrap(), addr("car_engine_reset").unwrap());
    for d in &DRIVES {
        let Some((mut ee, e)) = image(&kit, d) else { continue };
        let shipped: Vec<u32> = (0..0x50).map(|k| ee.u32_at(e + 4 * k)).collect();
        let tires = [0, 1, 2, 3].map(|k| ee.u32_at(CAR + TIRES + 4 * k));
        let mut pick = Pick(0x3c6e_f372_fe94_f82b ^ d.kind as u64);
        let mut bad = vec![];
        for case in 0..CASES {
            generate(&mut ee, e, &shipped, tires, &mut pick);
            let mut port = from_ram(&ee, e);
            let mut wheels = slots_from_ram(&ee, e);
            match case % 3 {
                0 => {
                    let gear = (pick.next() % 8) as i32 - 2;
                    ee.call(set_gear, &[u64::from(e), gear as i64 as u64], &[], BUDGET);
                    port.set_gear(gear, &mut wheels);
                }
                1 => {
                    let on = pick.one_in(2);
                    ee.call(set_reverse, &[u64::from(e), on as u64], &[], BUDGET);
                    port.set_reverse(on, &mut wheels);
                }
                _ => {
                    ee.call(reset, &[u64::from(e)], &[], BUDGET);
                    port.reset();
                }
            }
            bad.extend(compare(&format!("{} case {case}", d.name), &port, &|o| ee.u32_at(e + o), &[]));
            bad.extend(compare_tires(case, &ee, e, &wheels));
            assert!(bad.len() < 20, "{}", bad.join("\n"));
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
        println!("the {}'s gear setters: {CASES} cases, bit-exact", d.name);
    }
}

// ---- The captured drives ----

/// A car's file, out of `machine.res` in MX.PSR.
fn car_file(kit: &Kit, kind: Kind) -> Option<Vec<u8>> {
    let at = kit.ctx("iso").join("MX.PSR");
    let Ok(mut f) = File::open(&at) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", at.display()));
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
    // The archive spells its members lowercased and the loader does not, so the lookup folds.
    let e = res
        .entries
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(kind.phy()))
        .unwrap_or_else(|| panic!("machine.res has no {}", kind.phy()));
    Some(res.read(&bytes, e).expect("its bytes"))
}

/// One capture row: the frame counter, dt, and the car, engine and tire blocks.
struct Row {
    frame: u32,
    dt: F,
    car: Vec<u8>,
    engine: Vec<u8>,
    tires: [Vec<u8>; 4],
}

impl Row {
    fn w(b: &[u8], o: u32) -> u32 {
        u32::from_le_bytes(b[o as usize..o as usize + 4].try_into().unwrap())
    }
    fn e(&self, o: u32) -> u32 {
        Row::w(&self.engine, o)
    }
    fn ef(&self, o: u32) -> F {
        F::from_bits(self.e(o))
    }
    /// A car word: the block starts at +0x200.
    fn cf(&self, o: u32) -> F {
        F::from_bits(Row::w(&self.car, o - 0x200))
    }
    fn pedals(&self) -> Pedals {
        Pedals { throttle: self.cf(THROTTLE), clutch: self.cf(CLUTCH), brake: self.cf(BRAKE) }
    }
    /// The drive slots' tires as the row has them (slot `k`'s tire pointer matched to the car's).
    fn slots(&self) -> [DriveTire; 4] {
        let car_tires = [0, 1, 2, 3].map(|k| Row::w(&self.car, TIRES + 4 * k - 0x200));
        [0, 1, 2, 3].map(|k| {
            let p = self.e(0x30 + 4 * k);
            let Some(i) = car_tires.iter().position(|&t| t == p && p != 0) else { return DriveTire::default() };
            let t = &self.tires[i];
            let f = |o| F::from_bits(Row::w(t, o));
            DriveTire {
                contact: Row::w(t, T_CONTACT) != 0,
                spin: f(T_SPIN),
                slip_ratio: f(T_SLIP),
                compression: f(T_COMPRESSION),
                max_compression: f(T_MAX_COMPRESSION),
                drive_torque: f(T_DRIVE),
                top_wheel_speed: f(T_TOP_SPEED),
            }
        })
    }
}

fn hex(s: &str) -> Vec<u8> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    (0..s.len() / 2).map(|k| u8::from_str_radix(&s[2 * k..2 * k + 2], 16).unwrap()).collect()
}

fn rows(text: &str) -> Vec<Row> {
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap().split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name).unwrap_or_else(|| panic!("no {name} column"));
    let (fc, dt, car, engine) = (col("frame_counter"), col("dt"), col("car_raw"), col("engine_raw"));
    let tires = ["tire_fl_raw", "tire_fr_raw", "tire_rl_raw", "tire_rr_raw"].map(col);
    lines
        .map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            Row {
                frame: c[fc].parse().unwrap(),
                dt: F(c[dt].parse::<f32>().unwrap()),
                car: hex(c[car]),
                engine: hex(c[engine]),
                tires: tires.map(|k| hex(c[k])),
            }
        })
        .collect()
}

/// The substeps the dynamics world splits a frame of `dt` into, and each one's length.
fn substeps(dt: F) -> (i32, F) {
    let n = (dt * F(115.0)).to_i32() + 1;
    (n, dt / F(fpu::from_i32(n)))
}

/// Floats a few steps either side of `x`.
fn near(x: F, ulps: i32) -> impl Iterator<Item = F> {
    let b = x.to_bits() as i32;
    (-ulps..=ulps).map(move |k| F::from_bits((b + if x.to_f32() < 0.0 { -k } else { k }) as u32))
}

#[derive(Default)]
struct Tally {
    frames: usize,
    whole: usize,
    last: usize,
    shifts: usize,
    gaps: usize,
    still: usize,
    doubled: usize,
    no_spin: usize,
    bad: Vec<String>,
}

/// One drive: every pair of rows the game stepped between.
fn replay(name: &str, csv: &str, loaded: &Drivetrain, curve: &CurveTable) -> Tally {
    let rows = rows(csv);
    let mut t = Tally::default();
    // The words the replay cannot reach: the sound's pitch reads the suspension mid-frame.
    let unseen = ["pitch", "nox left"];
    for (i, pair) in rows.windows(2).enumerate() {
        let (r0, r1) = (&pair[0], &pair[1]);
        if r1.frame != r0.frame.wrapping_add(1) {
            t.gaps += 1;
            continue;
        }
        // Paused: the frame counter runs on and the world does not step.
        if r1.engine == r0.engine {
            t.still += 1;
            continue;
        }
        let at = format!("{name} row {}", i + 1);
        let mut d = loaded.clone();
        fill(&mut d, &|o| r0.e(o));
        // The sound clock counts the substeps the engine ran: a row the capture read a frame late
        // spans two frames, whose middle pedals no row has.
        let (n, h) = substeps(r1.dt);
        let mut clock = d.sound_clock;
        for _ in 0..n {
            clock = clock + h;
        }
        if clock.to_bits() != r1.e(0x134) {
            t.doubled += 1;
            continue;
        }
        t.frames += 1;
        // The car writes its pedals and the launch boost before the frame's substeps, so the second
        // row has the ones they read.
        let pedals = r1.pedals();
        d.launch_boost = r1.ef(0x6c);
        let mut wheels = r0.slots();

        // The shift-throttle timer through the frame, which says which substeps had the clutch in.
        let mut timer = d.shift_throttle_timer;
        let mut held = vec![];
        for _ in 0..n {
            held.push(!(timer <= F::ZERO) || !(pedals.clutch <= F::ZERO));
            if !(timer <= F::ZERO) {
                timer = timer - h;
            }
        }
        let mut after;
        if held.iter().all(|&x| x) {
            // The whole frame, from the first row.
            for _ in 0..n {
                d.step(pedals, &mut wheels, h, curve, &mut |_| F::ZERO);
            }
            after = d.clone();
            t.whole += 1;
        } else {
            // The last substep, from what the second row keeps of it.
            let (slip, wheel_rpm, braking) = (r1.ef(0x12c), r1.ef(0xec), r1.ef(0x114));
            let last_held = *held.last().unwrap();
            let pre: Vec<F> = if last_held {
                vec![wheel_rpm]
            } else {
                let guess = if wheel_rpm < F::ZERO { slip } else { slip + wheel_rpm };
                near(guess, 64)
                    .filter(|&x| (if wheel_rpm < F::ZERO { x } else { x - wheel_rpm }) == slip)
                    .filter(|&x| (x * (d.engine_braking * x)).to_bits() == braking.to_bits())
                    .collect()
            };
            // The spins that give these wheel revs: one tire spinning, the other still.
            let spin = if last_held {
                Some(F::ZERO)
            } else {
                let k = F(9.549296);
                let guess = F((f64::from(wheel_rpm.to_f32()) / (9.549296 * f64::from(d.ratio.to_f32()) * 0.5)) as f32);
                near(guess, 64).find(|&s| (k * (d.ratio * (F(0.5) * s))).to_bits() == wheel_rpm.to_bits())
            };
            let Some(spin) = spin else {
                t.no_spin += 1;
                continue;
            };
            if pre.is_empty() {
                t.bad.push(format!("{at}: no revs before the last substep give slip {slip:?}, wheel rpm {wheel_rpm:?}, braking {braking:?}"));
                continue;
            }
            let mut best: Option<(Drivetrain, Vec<String>)> = None;
            for &rpm in &pre {
                let mut c = d.clone();
                c.rpm = rpm;
                // The timer before the last substep.
                let mut x = d.shift_throttle_timer;
                for _ in 0..n - 1 {
                    if !(x <= F::ZERO) {
                        x = x - h;
                    }
                }
                c.shift_throttle_timer = x;
                for _ in 0..n - 1 {
                    c.sound_clock = c.sound_clock + h;
                }
                let mut w = r1.slots();
                w[0].spin = spin;
                w[1].spin = F::ZERO;
                c.step(pedals, &mut w, h, curve, &mut |_| F::ZERO);
                let bad = compare(&at, &c, &|o| r1.e(o), &[&unseen[..], &["shift timer", "shifting up", "shift throttle timer", "gear", "ratio", "1/ratio", "sound clock"]].concat());
                if best.as_ref().is_none_or(|b| bad.len() < b.1.len()) {
                    best = Some((c, bad));
                }
            }
            let (c, bad) = best.unwrap();
            if !bad.is_empty() {
                t.bad.extend(bad);
                continue;
            }
            after = c;
            // The words this replay does not carry from the first row come from the second.
            after.shift_timer = d.shift_timer;
            after.shift_throttle_timer = timer;
            t.last += 1;
        }

        // The next frame's shift, on the second row's revs, load and tires, with its clutch pedal.
        let mut tires = r1.slots();
        let gear = after.gear;
        after.shift(r1.pedals().clutch, r1.dt, &mut tires);
        if after.gear != gear {
            t.shifts += 1;
        }
        let skip_words: Vec<&str> = unseen.to_vec();
        t.bad.extend(compare(&at, &after, &|o| r1.e(o), &skip_words));
    }
    t
}

#[test]
fn rpm_gear_and_drive_torque_replay_the_captured_drives() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    let curve = CurveTable::build();
    let mut any = false;
    for (kind, name) in [(Kind::Baja, "baja"), (Kind::Buggy, "buggy"), (Kind::MonsterTruck, "truck"), (Kind::MonsterTruck, "truck-roll")] {
        let at = kit.ctx("capture").join("s149a").join(format!("{name}.csv"));
        let Ok(csv) = std::fs::read_to_string(&at) else {
            skip(&format!("no {} — S149a's capture.py writes it", at.display()));
            continue;
        };
        let Some(bytes) = car_file(&kit, kind) else { return };
        let car = Car::read_bytes(&bytes, kind);
        let loaded = Drivetrain::new(&car.engine);
        let t = replay(name, &csv, &loaded, &curve);
        println!(
            "{name}: {} frames — {} whole, {} last substep, {} shifts; {} paused, {} two frames long, {} gaps, {} without a spin to match",
            t.frames, t.whole, t.last, t.shifts, t.still, t.doubled, t.gaps, t.no_spin
        );
        assert!(t.bad.is_empty(), "{name}: {} words off\n{}", t.bad.len(), t.bad.iter().take(30).cloned().collect::<Vec<_>>().join("\n"));
        assert!(t.whole > 0 && t.last > 0 && t.shifts >= 4, "{name}: too little replayed");
        any = true;
    }
    if !any {
        skip("no S149a captures");
    }
}
