//! The car's once-a-frame steps (S149d) against the original's, under the interpreter on S149a's
//! switched images: the pad into the car, the controls step, the centre of mass's travel and the
//! measure's yaw-rate hold. Each runs on every row of S149's captured drives of the baja, the buggy
//! and the monster truck (`context/capture/s149b/<car>.csv`, the car's raw block laid in as the
//! state the step starts from) and on generated states, and the port must write every word the
//! original writes, to the bit.
//!
//! The rest of the car step — the ground probes, the contacts, the class's force step, the engine —
//! is not in here: see research/journal/*-s149d-car-step for what the drive replay still waits on.

use game_sim::bindings::Command;
use game_sim::biplane::Body;
use game_sim::car::{self, Car, Controls, Kind, Reverse};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::cell::Cell;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::rc::Rc;

/// Where S149a's switch parks the car, in every drive.
const CAR: u32 = 0x00605270;
/// The car's raw block in a row starts at car +0x200.
const CAR_RAW: u32 = 0x200;
const BUDGET: u64 = 200_000;
/// Generated states per car and step.
const CASES: usize = 10_000;

/// The car object's words the steps read and write.
mod at {
    pub const CONTROLLER: u32 = 0x20;
    pub const BODY: u32 = 0x160;
    pub const COFG: u32 = 0x230;
    pub const COFG_AI: u32 = 0x240;
    pub const LAST_COFG: u32 = 0x250;
    pub const TRAVEL_FORWARD: u32 = 0x260;
    pub const TRAVEL_BACK: u32 = 0x264;
    pub const BRAKE: u32 = 0x3dc;
    pub const GAS: u32 = 0x3e0;
    pub const CLUTCH: u32 = 0x3e4;
    pub const PAD_WORD: u32 = 0x3e8;
    pub const THROTTLE: u32 = 0x3ec;
    pub const ENGAGED_CLUTCH: u32 = 0x3f0;
    pub const FRONT_BRAKE: u32 = 0x3f4;
    pub const REAR_BRAKE: u32 = 0x3f8;
    pub const HAND_BRAKE: u32 = 0x3fc;
    pub const STEER: u32 = 0x400;
    pub const REAR_STEER: u32 = 0x404;
    pub const MAX_ANALOG_THROTTLE: u32 = 0x408;
    pub const DIGITAL_THROTTLE_SPEED: u32 = 0x40c;
    pub const DIGITAL_SIDE_SLIP: u32 = 0x410;
    pub const HAND_BRAKE_FRICTION: u32 = 0x41c;
    pub const HAND_BRAKE_BRAKE: u32 = 0x420;
    pub const HAND_BRAKE_FADE_TIME: u32 = 0x424;
    pub const HAND_BRAKE_FADE: u32 = 0x428;
    pub const DRIVEN: u32 = 0x4ac;
    pub const SQUARE: u32 = 0x4b0;
    pub const DPAD: u32 = 0x4b4;
    pub const STICK: u32 = 0x4b8;
    pub const LEAN: u32 = 0x4bc;
    pub const STEER_INPUT: u32 = 0x4c0;
    pub const BRAKE_RATE: u32 = 0x4c4;
    pub const GAS_RATE: u32 = 0x4c8;
    pub const DPAD_ON: u32 = 0x4cc;
    pub const DPAD_OFF: u32 = 0x4d0;
    pub const X_AXIS_RATE: u32 = 0x4d4;
    pub const STEER_TABLE: u32 = 0x4d8;
    pub const WHEEL_TABLE: u32 = 0x4f0;
    pub const DPAD_TABLE: u32 = 0x508;
    pub const REAR_STEER_TABLE: u32 = 0x520;
    pub const YAW_LIMIT: u32 = 0x538;
    pub const HAND_BRAKE_YAW_LIMIT: u32 = 0x53c;
    pub const LIVE_YAW_LIMIT: u32 = 0x540;
    pub const ENGINE: u32 = 0x544;
    pub const REAR_TIRES: [u32; 2] = [0x550, 0x554];
    pub const GROUND_SPEED: u32 = 0x55c;
    pub const SIDE_SLIP: u32 = 0x564;
    pub const FORWARD_SPEED: u32 = 0x588;
    /// On the controller: a steering wheel; the controller is a human's.
    pub const WHEEL: u32 = 0x60;
    pub const HUMAN: u32 = 0x34;
    /// On the engine: in reverse.
    pub const REVERSING: u32 = 0xc8;
    /// On a tire: its friction scale.
    pub const FRICTION_SCALE: u32 = 0x9b4;
    /// On the pad: the word the input step copies, and a command's record.
    pub const PAD_COPY: u32 = 0x31c;
    pub const RECORD: u32 = 0x18;
    pub const PRESSURE: u32 = 4;
}

/// A row: its frame counter, dt, the body, and the car's and the engine's raw blocks.
struct Row {
    frame_counter: u32,
    dt: F,
    body: [[f32; 4]; 4],
    car: Vec<u8>,
    engine: Vec<u8>,
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
}

fn load_rows(path: &std::path::Path) -> Option<Vec<Row>> {
    let Ok(text) = std::fs::read_to_string(path) else {
        skip(&format!(
            "no {} — S149b's capture.sh records it (research/journal/2026-10-29-s149b-car-tire-model)",
            path.display()
        ));
        return None;
    };
    let mut lines = text.lines();
    let head: HashMap<&str, usize> = lines.next()?.split(',').enumerate().map(|(i, h)| (h, i)).collect();
    let rows = lines
        .map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            let col = |n: &str| c[head[n]];
            Row {
                frame_counter: col("frame_counter").parse().unwrap(),
                dt: F(col("dt").parse::<f32>().unwrap()),
                body: ["body_pos", "body_vel", "body_quat", "body_rate"]
                    .map(|b| ["x", "y", "z", "w"].map(|l| col(&format!("{b}.{l}")).parse::<f32>().unwrap())),
                car: unhex(col("car_raw")),
                engine: unhex(col("engine_raw")),
            }
        })
        .collect();
    Some(rows)
}

/// A car's file, out of `machine.res` in MX.PSR.
fn car_file(kit: &Kit, kind: Kind) -> Option<Vec<u8>> {
    use game_sim::archive::{Psr, Res};
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
    let e = res
        .entries
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(kind.phy()))
        .unwrap_or_else(|| panic!("machine.res has no {}", kind.phy()));
    Some(res.read(&bytes, e).expect("its bytes"))
}

/// A drive: its car's file, its rows, the image it runs on.
struct Drive {
    kit: Kit,
    car: Car,
    rows: Vec<Row>,
    ee: Ee,
}

fn drive(kind: Kind, csv: &str, image: &str) -> Option<Drive> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let rows = load_rows(&kit.ctx("capture").join("s149b").join(csv))?;
    let car = Car::read_bytes(&car_file(&kit, kind)?, kind);
    let path = kit.ctx("states").join(image);
    if !path.exists() {
        skip(&format!("no {} — S149a's switch recipe makes it", path.display()));
        return None;
    }
    let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    Some(Drive { kit, car, rows, ee })
}

/// A small deterministic generator, so a failure names a case that comes back.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn chance(&mut self, n: u32) -> bool {
        self.next() % n == 0
    }
    /// A float in `lo..hi`, its bits whatever the division leaves.
    fn range(&mut self, lo: f32, hi: f32) -> F {
        let t = (self.next() >> 8) as f32 / 16_777_216.0;
        F(lo + (hi - lo) * t)
    }
    fn pick(&mut self, from: &[f32]) -> F {
        F(from[self.next() as usize % from.len()])
    }
    /// One of a few values the code branches on, or anything in `lo..hi`.
    fn value(&mut self, special: &[f32], lo: f32, hi: f32) -> F {
        if self.chance(3) { self.pick(special) } else { self.range(lo, hi) }
    }
}

fn f(ee: &Ee, o: u32) -> F {
    F(ee.f32_at(CAR + o))
}
fn set(ee: &mut Ee, o: u32, v: F) {
    ee.set_u32(CAR + o, v.to_bits());
}
fn flag(ee: &mut Ee, o: u32, on: bool) {
    ee.set_u32(CAR + o, on as u32);
}
fn v(ee: &Ee, a: u32) -> Vf {
    Vf::from_bits([0, 4, 8, 12].map(|i| ee.u32_at(a + i)))
}
fn set_v(ee: &mut Ee, a: u32, x: Vf) {
    for (i, b) in x.to_bits().into_iter().enumerate() {
        ee.set_u32(a + 4 * i as u32, b);
    }
}
fn rear_tire(ee: &Ee, k: usize) -> u32 {
    ee.u32_at(CAR + at::REAR_TIRES[k])
}

/// The controls as the car holds them in RAM.
fn controls_of(ee: &Ee) -> Controls {
    Controls {
        driven: ee.u32_at(CAR + at::DRIVEN) != 0,
        square_steering: ee.u32_at(CAR + at::SQUARE) != 0,
        stick: f(ee, at::STICK),
        lean: f(ee, at::LEAN),
        dpad: ee.u32_at(CAR + at::DPAD) != 0,
        pad_word: f(ee, at::PAD_WORD),
        gas: f(ee, at::GAS),
        brake: f(ee, at::BRAKE),
        clutch: f(ee, at::CLUTCH),
        hand_brake: f(ee, at::HAND_BRAKE),
        steer_input: f(ee, at::STEER_INPUT),
        steer: f(ee, at::STEER),
        rear_steer: f(ee, at::REAR_STEER),
        front_brake: f(ee, at::FRONT_BRAKE),
        rear_brake: f(ee, at::REAR_BRAKE),
        throttle: f(ee, at::THROTTLE),
        engaged_clutch: f(ee, at::ENGAGED_CLUTCH),
        hand_brake_fade: f(ee, at::HAND_BRAKE_FADE),
        yaw_rate_limit: f(ee, at::LIVE_YAW_LIMIT),
        rear_friction: F(ee.f32_at(rear_tire(ee, 0) + at::FRICTION_SCALE)),
    }
}

/// Every field's bits, named, for a field-by-field compare.
fn words(c: &Controls) -> [(&'static str, u32); 20] {
    [
        ("driven", c.driven as u32),
        ("square_steering", c.square_steering as u32),
        ("stick", c.stick.to_bits()),
        ("lean", c.lean.to_bits()),
        ("dpad", c.dpad as u32),
        ("pad_word", c.pad_word.to_bits()),
        ("gas", c.gas.to_bits()),
        ("brake", c.brake.to_bits()),
        ("clutch", c.clutch.to_bits()),
        ("hand_brake", c.hand_brake.to_bits()),
        ("steer_input", c.steer_input.to_bits()),
        ("steer", c.steer.to_bits()),
        ("rear_steer", c.rear_steer.to_bits()),
        ("front_brake", c.front_brake.to_bits()),
        ("rear_brake", c.rear_brake.to_bits()),
        ("throttle", c.throttle.to_bits()),
        ("engaged_clutch", c.engaged_clutch.to_bits()),
        ("hand_brake_fade", c.hand_brake_fade.to_bits()),
        ("yaw_rate_limit", c.yaw_rate_limit.to_bits()),
        ("rear_friction", c.rear_friction.to_bits()),
    ]
}

/// Mismatches, the first few of each kind kept to print.
#[derive(Default)]
struct Tally {
    cases: usize,
    bad: usize,
    shown: Vec<String>,
}

impl Tally {
    fn compare(&mut self, case: &str, got: &Controls, want: &Controls) {
        self.cases += 1;
        let (g, w) = (words(got), words(want));
        let off: Vec<String> = g
            .iter()
            .zip(&w)
            .filter(|(a, b)| a.1 != b.1)
            .map(|(a, b)| {
                format!("{} {:#010x} ({}) want {:#010x} ({})", a.0, a.1, f32::from_bits(a.1), b.1, f32::from_bits(b.1))
            })
            .collect();
        self.miss(case, &off);
    }
    fn words(&mut self, case: &str, what: &str, got: &[u32], want: &[u32]) {
        self.cases += 1;
        let off: Vec<String> = (got != want).then(|| format!("{what} {got:08x?} want {want:08x?}")).into_iter().collect();
        self.miss(case, &off);
    }
    fn miss(&mut self, case: &str, off: &[String]) {
        if !off.is_empty() {
            self.bad += 1;
            if self.shown.len() < 12 {
                self.shown.push(format!("{case}: {}", off.join(", ")));
            }
        }
    }
    fn assert(&self, what: &str) {
        eprintln!("{what}: {} cases, {} off", self.cases, self.bad);
        assert!(self.bad == 0, "{what}: {} of {} cases off:\n  {}", self.bad, self.cases, self.shown.join("\n  "));
        assert!(self.cases > 0, "{what}: nothing compared");
    }
}

/// The file's values the steps read, against the words the car holds: the port reads its file the
/// way the original did, or every compare after this one means nothing.
fn the_file_is_the_cars(car: &Car, ee: &Ee) {
    let j = &car.joystick;
    let mut words: Vec<(&str, u32, u32)> = vec![
        ("SquareSteeringInput", at::SQUARE, j.square_steering_input as u32),
        ("XAxisRateLimit", at::X_AXIS_RATE, j.x_axis_rate_limit.to_bits()),
        ("XDPadRateLimitOn", at::DPAD_ON, j.x_dpad_rate_limit_on.to_bits()),
        ("XDPadRateLimitOff", at::DPAD_OFF, j.x_dpad_rate_limit_off.to_bits()),
        ("BrakeRateLimit", at::BRAKE_RATE, j.brake_rate_limit.to_bits()),
        ("GasRateLimit", at::GAS_RATE, j.gas_rate_limit.to_bits()),
        ("MaxAnalogThrottle", at::MAX_ANALOG_THROTTLE, j.max_analog_throttle.to_bits()),
        ("DigitalThrottleSpeed", at::DIGITAL_THROTTLE_SPEED, j.digital_throttle_speed.to_bits()),
        ("DigitalSideSlipAngle", at::DIGITAL_SIDE_SLIP, j.digital_side_slip_angle.to_bits()),
        ("YawRateLimit", at::YAW_LIMIT, car.yaw_rate_limit.to_bits()),
        ("HandBrakeYawRateLimit", at::HAND_BRAKE_YAW_LIMIT, car.hand_brake_yaw_rate_limit.to_bits()),
        ("HandBrakeFrictionScale", at::HAND_BRAKE_FRICTION, car.hand_brake_friction_scale.to_bits()),
        ("HandBrakeBrakeScale", at::HAND_BRAKE_BRAKE, car.hand_brake_brake_scale.to_bits()),
        ("HandBrakeFadeTime", at::HAND_BRAKE_FADE_TIME, car.hand_brake_fade_time.to_bits()),
        ("CofGTravelForward", at::TRAVEL_FORWARD, car.cofg_travel_forward.to_bits()),
        ("CofGTravelBack", at::TRAVEL_BACK, car.cofg_travel_back.to_bits()),
    ];
    for (name, o, table) in [("steer", at::STEER_TABLE, j.steer), ("wheel", at::WHEEL_TABLE, j.wheel_steer), ("dpad", at::DPAD_TABLE, j.dpad_steer)] {
        for (k, x) in table.iter().flatten().enumerate() {
            words.push((name, o + 4 * k as u32, x.to_bits()));
        }
    }
    for (k, x) in j.rear_steer.iter().flatten().enumerate() {
        words.push(("RearSteer", at::REAR_STEER_TABLE + 4 * k as u32, x.to_bits()));
    }
    for (k, x) in car.mass_properties.cofg.to_bits().into_iter().take(3).enumerate() {
        words.push(("CofG", at::COFG + 4 * k as u32, x));
    }
    for (k, x) in car.cofg_ai.to_bits().into_iter().take(3).enumerate() {
        words.push(("CofGAI", at::COFG_AI + 4 * k as u32, x));
    }
    let off: Vec<String> = words
        .iter()
        .filter(|(_, o, want)| ee.u32_at(CAR + o) != *want)
        .map(|(n, o, want)| format!("{n} (+{o:#x}) file {want:#010x}, car {:#010x}", ee.u32_at(CAR + o)))
        .collect();
    assert!(off.is_empty(), "the port's file is not the car's:\n  {}", off.join("\n  "));
}

/// The file's values onto the car in RAM, for a generated car.
fn lay_file(ee: &mut Ee, car: &Car) {
    let j = &car.joystick;
    flag(ee, at::SQUARE, j.square_steering_input != 0);
    for (o, x) in [
        (at::X_AXIS_RATE, j.x_axis_rate_limit),
        (at::DPAD_ON, j.x_dpad_rate_limit_on),
        (at::DPAD_OFF, j.x_dpad_rate_limit_off),
        (at::BRAKE_RATE, j.brake_rate_limit),
        (at::GAS_RATE, j.gas_rate_limit),
        (at::MAX_ANALOG_THROTTLE, j.max_analog_throttle),
        (at::DIGITAL_THROTTLE_SPEED, j.digital_throttle_speed),
        (at::DIGITAL_SIDE_SLIP, j.digital_side_slip_angle),
        (at::YAW_LIMIT, car.yaw_rate_limit),
        (at::HAND_BRAKE_YAW_LIMIT, car.hand_brake_yaw_rate_limit),
        (at::HAND_BRAKE_FRICTION, car.hand_brake_friction_scale),
        (at::HAND_BRAKE_BRAKE, car.hand_brake_brake_scale),
        (at::HAND_BRAKE_FADE_TIME, car.hand_brake_fade_time),
        (at::TRAVEL_FORWARD, car.cofg_travel_forward),
        (at::TRAVEL_BACK, car.cofg_travel_back),
    ] {
        set(ee, o, x);
    }
    for (o, table) in [(at::STEER_TABLE, j.steer), (at::WHEEL_TABLE, j.wheel_steer), (at::DPAD_TABLE, j.dpad_steer)] {
        for (k, &x) in table.iter().flatten().enumerate() {
            set(ee, o + 4 * k as u32, x);
        }
    }
    for (k, &x) in j.rear_steer.iter().flatten().enumerate() {
        set(ee, at::REAR_STEER_TABLE + 4 * k as u32, x);
    }
    set_v(ee, CAR + at::COFG, car.mass_properties.cofg);
    set_v(ee, CAR + at::COFG_AI, car.cofg_ai);
}

/// A table of three `(speed, scale)` points with speeds in order, sometimes two the same.
fn table(rng: &mut Rng) -> [[F; 2]; 3] {
    let a = rng.value(&[0.0], -20.0, 60.0);
    let b = if rng.chance(8) { a } else { F(a.0 + rng.range(0.0, 80.0).0) };
    let c = if rng.chance(8) { b } else { F(b.0 + rng.range(0.0, 80.0).0) };
    [a, b, c].map(|s| [s, rng.value(&[0.0, 1.0], -0.5, 1.5)])
}

/// A car file with every value the steps read made up.
fn made_up(car: &Car, rng: &mut Rng) -> Car {
    let mut c = car.clone();
    let j = &mut c.joystick;
    j.square_steering_input = rng.chance(2) as i32;
    j.x_axis_rate_limit = rng.value(&[0.0, 1000.0], 0.0, 20.0);
    j.x_dpad_rate_limit_on = rng.value(&[0.0], 0.0, 20.0);
    j.x_dpad_rate_limit_off = rng.value(&[0.0], 0.0, 20.0);
    j.brake_rate_limit = rng.value(&[0.0, 1000.0], 0.0, 20.0);
    j.gas_rate_limit = rng.value(&[0.0, 1000.0], 0.0, 20.0);
    j.max_analog_throttle = rng.value(&[0.0, 1.0], 0.0, 1.5);
    j.digital_throttle_speed = rng.value(&[0.0], 0.0, 80.0);
    j.digital_side_slip_angle = rng.value(&[0.0], 0.0, 1.5);
    j.steer = table(rng);
    j.wheel_steer = table(rng);
    j.dpad_steer = table(rng);
    let s1 = rng.value(&[0.0], 0.0, 60.0);
    let s2 = if rng.chance(6) { s1 } else { F(s1.0 + rng.range(0.0, 100.0).0) };
    j.rear_steer = [[s1, rng.value(&[1.0, 0.0], -1.0, 1.0)], [s2, rng.value(&[1.0, 0.0], -1.0, 1.0)]];
    c.yaw_rate_limit = rng.value(&[10.0], 0.0, 20.0);
    c.hand_brake_yaw_rate_limit = rng.value(&[3.0], 0.0, 20.0);
    c.hand_brake_friction_scale = rng.value(&[0.9, 1.0], 0.0, 2.0);
    c.hand_brake_brake_scale = rng.value(&[2.0, 1.0], 0.0, 3.0);
    c.hand_brake_fade_time = rng.value(&[0.15, 0.0], 0.0, 1.0);
    c.cofg_travel_forward = rng.value(&[0.0], -2.0, 2.0);
    c.cofg_travel_back = rng.value(&[0.0], -2.0, 2.0);
    c
}

const UNIT: &[f32] = &[0.0, -0.0, 1.0, -1.0];

/// The car's state the controls step reads made up, in RAM.
fn made_up_state(ee: &mut Ee, rng: &mut Rng) {
    flag(ee, at::DRIVEN, !rng.chance(10));
    flag(ee, at::DPAD, rng.chance(3));
    set(ee, at::STICK, rng.value(UNIT, -1.0, 1.0));
    set(ee, at::LEAN, rng.value(UNIT, -1.0, 1.0));
    set(ee, at::PAD_WORD, rng.value(&[0.0], -1.0, 1.0));
    set(ee, at::GAS, rng.value(&[0.0, 1.0, 0.4], 0.0, 1.0));
    set(ee, at::BRAKE, rng.pick(&[0.0, 1.0]));
    set(ee, at::CLUTCH, rng.pick(&[0.0, 1.0]));
    set(ee, at::HAND_BRAKE, if rng.chance(4) { F(1.0) } else { F(0.0) });
    set(ee, at::STEER_INPUT, rng.value(UNIT, -1.0, 1.0));
    set(ee, at::STEER, rng.range(-1.0, 1.0));
    set(ee, at::REAR_STEER, rng.range(-1.0, 1.0));
    set(ee, at::FRONT_BRAKE, rng.value(&[0.0, 1.0], 0.0, 1.0));
    set(ee, at::REAR_BRAKE, rng.value(&[0.0, 1.0], 0.0, 1.0));
    set(ee, at::THROTTLE, rng.value(&[0.0, 1.0, 0.4], 0.0, 1.0));
    set(ee, at::ENGAGED_CLUTCH, rng.pick(&[0.0, 1.0]));
    set(ee, at::HAND_BRAKE_FADE, rng.value(&[0.0, 0.15], -0.05, 0.2));
    set(ee, at::LIVE_YAW_LIMIT, rng.value(&[10.0, 3.0], 0.0, 20.0));
    set(ee, at::FORWARD_SPEED, rng.value(&[0.0], -40.0, 150.0));
    set(ee, at::GROUND_SPEED, rng.value(&[0.0, 10.0], 0.0, 150.0));
    let friction = rng.value(&[1.0, 0.9], 0.0, 2.0);
    for k in 0..2 {
        let t = rear_tire(ee, k);
        ee.set_u32(t + at::FRICTION_SCALE, friction.to_bits());
    }
}

/// The row's car and engine as the state a step starts from.
fn lay_row(ee: &mut Ee, row: &Row) {
    ee.load(CAR + CAR_RAW, &row.car);
    let engine = ee.u32_at(CAR + at::ENGINE);
    ee.load(engine, &row.engine);
}

fn controller(ee: &Ee) -> u32 {
    let c = ee.u32_at(CAR + at::CONTROLLER);
    assert!(c != 0, "the switched car has no controller");
    c
}

/// One controls step, the original's against the port's, from the state in RAM. `reverses` is what
/// the hook on the engine's reverse switch saw last.
fn one_controls_step(
    ee: &mut Ee,
    car: &Car,
    step: u32,
    reverses: &Rc<Cell<Option<u32>>>,
    dt: F,
    case: &str,
    tally: &mut Tally,
) {
    let wheel = ee.u32_at(controller(ee) + at::WHEEL) != 0;
    let engine = ee.u32_at(CAR + at::ENGINE);
    let reversing = ee.u32_at(engine + at::REVERSING) != 0;
    let mut port = controls_of(ee);
    let rear_rr = rear_tire(ee, 1);
    let asked = port.step(car, dt, wheel, f(ee, at::FORWARD_SPEED), f(ee, at::GROUND_SPEED), reversing);
    reverses.set(None);
    ee.call(step, &[CAR as u64], &[dt.0], BUDGET);
    let mut want = controls_of(ee);
    // Both rear tires take the friction; the second's must agree with the first's.
    let rr = F(ee.f32_at(rear_rr + at::FRICTION_SCALE));
    if rr.to_bits() != want.rear_friction.to_bits() {
        want.rear_friction = rr;
    }
    tally.compare(case, &port, &want);
    let asked = asked.map(|r| (r == Reverse::In) as u32);
    tally.words(case, "reverse", &asked.into_iter().collect::<Vec<_>>(), &reverses.get().into_iter().collect::<Vec<_>>());
}

fn the_controls_step(kind: Kind, csv: &str, image: &str) {
    let Some(mut d) = drive(kind, csv, image) else { return };
    let step = d.kit.addr("car_controls").unwrap_or_else(|e| panic!("{e}"));
    let set_reverse = d.kit.addr("car_engine_set_reverse").unwrap_or_else(|e| panic!("{e}"));
    let reverses = Rc::new(Cell::new(None));
    let seen = reverses.clone();
    d.ee.hooks.insert(set_reverse, Box::new(move |ee| seen.set(Some(ee.r(5) as u32))));
    let ee = &mut d.ee;
    let ctl = controller(ee);

    lay_row(ee, &d.rows[0]);
    the_file_is_the_cars(&d.car, ee);

    let mut tally = Tally::default();
    for row in &d.rows {
        for wheel in [false, true] {
            lay_row(ee, row);
            ee.set_u32(ctl + at::WHEEL, wheel as u32);
            one_controls_step(ee, &d.car, step, &reverses, row.dt, &format!("{csv} fc {} wheel {wheel}", row.frame_counter), &mut tally);
        }
    }
    tally.assert(&format!("{csv}: the controls step on every row"));

    let mut tally = Tally::default();
    let mut rng = Rng(0x5149_d000 ^ kind as u64);
    for case in 0..CASES {
        let row = &d.rows[rng.next() as usize % d.rows.len()];
        lay_row(ee, row);
        let car = if rng.chance(2) { made_up(&d.car, &mut rng) } else { d.car.clone() };
        lay_file(ee, &car);
        made_up_state(ee, &mut rng);
        ee.set_u32(ctl + at::WHEEL, rng.chance(4) as u32);
        let engine = ee.u32_at(CAR + at::ENGINE);
        ee.set_u32(engine + at::REVERSING, rng.chance(3) as u32);
        let dt = rng.value(&[1.0 / 30.0, 1.0 / 60.0, 0.0], 0.0, 0.1);
        one_controls_step(ee, &car, step, &reverses, dt, &format!("{csv} case {case}"), &mut tally);
    }
    tally.assert(&format!("{csv}: the controls step on generated states"));
}

/// The commands the input step asks the pad for.
const ASKED: [u8; 8] = [0x0e, 0x0f, 0x16, 0x10, 0x00, 0x01, 0x02, 0x03];

fn the_input_step(kind: Kind, csv: &str, image: &str) {
    let Some(mut d) = drive(kind, csv, image) else { return };
    let input = d.kit.addr("car_input").unwrap_or_else(|e| panic!("{e}"));
    let index = d.kit.addr("pad_control_index").unwrap_or_else(|e| panic!("{e}"));
    // The pad's record for a command is the command's own: the binding table is not what is tested.
    d.ee.hooks.insert(index, Box::new(|ee| ee.set(2, ee.r(5))));
    let pad = d.ee.scratch(0);
    let ee = &mut d.ee;
    let mut tally = Tally::default();
    let mut rng = Rng(0x5149_d100 ^ kind as u64);
    for case in 0..CASES + d.rows.len() {
        let (row, from_row) = match d.rows.get(case) {
            Some(row) => (row, true),
            None => (&d.rows[rng.next() as usize % d.rows.len()], false),
        };
        lay_row(ee, row);
        let car = if from_row || rng.chance(2) { d.car.clone() } else { made_up(&d.car, &mut rng) };
        lay_file(ee, &car);
        if !from_row {
            made_up_state(ee, &mut rng);
            set(ee, at::SIDE_SLIP, rng.value(&[0.0], -1.6, 1.6));
        }
        let mut pressure = [F(0.0); 0x50];
        for &c in &ASKED {
            pressure[c as usize] = if rng.chance(2) { F(0.0) } else { rng.value(&[1.0, -0.0, -1.0], 0.0, 1.0) };
        }
        for (c, p) in pressure.iter().enumerate() {
            ee.set_u32(pad + at::RECORD * c as u32 + at::PRESSURE, p.to_bits());
        }
        let pad_word = rng.range(-1.0, 1.0);
        ee.set_u32(pad + at::PAD_COPY, pad_word.to_bits());
        let stick = (rng.value(UNIT, -1.0, 1.0), rng.value(UNIT, -1.0, 1.0));

        let mut port = controls_of(ee);
        port.input(&car, stick, pad_word, f(ee, at::GROUND_SPEED), f(ee, at::SIDE_SLIP), |c: Command| pressure[c.0 as usize]);
        ee.call(input, &[CAR as u64, pad as u64, 0], &[stick.0 .0, stick.1 .0], BUDGET);
        tally.compare(&format!("{csv} case {case}"), &port, &controls_of(ee));
    }
    tally.assert(&format!("{csv}: the input step on every row and generated pads"));
}

fn the_cofg_travel(kind: Kind, csv: &str, image: &str) {
    let Some(mut d) = drive(kind, csv, image) else { return };
    let travel = d.kit.addr("car_cofg_travel").unwrap_or_else(|e| panic!("{e}"));
    let ee = &mut d.ee;
    let ctl = controller(ee);
    let human = ee.u32_at(ctl + at::HUMAN);
    let mut tally = Tally::default();
    let mut rng = Rng(0x5149_d200 ^ kind as u64);
    for case in 0..CASES + d.rows.len() {
        let (row, from_row) = match d.rows.get(case) {
            Some(row) => (row, true),
            None => (&d.rows[rng.next() as usize % d.rows.len()], false),
        };
        lay_row(ee, row);
        let car = if from_row || rng.chance(2) { d.car.clone() } else { made_up(&d.car, &mut rng) };
        lay_file(ee, &car);
        let at_body = ee.u32_at(CAR + at::BODY);
        let mut body = Body {
            position: Vf(row.body[0]),
            velocity: Vf(row.body[1]),
            quat: Vf(row.body[2]),
            rates: Vf(row.body[3]),
            ..Body::default()
        };
        body.last_position = if from_row {
            body.position
        } else {
            let p = body.position.0;
            Vf([0, 1, 2, 3].map(|i| p[i] + rng.range(-1.0, 1.0).0))
        };
        body.cofg = v(ee, at_body + 0x180);
        let mut last = v(ee, CAR + at::LAST_COFG);
        if !from_row {
            last = Vf([0, 1, 2, 3].map(|i| last.0[i] + rng.range(-0.5, 0.5).0));
            set(ee, at::LEAN, rng.value(UNIT, -1.0, 1.0));
        }
        let is_human = if from_row { human == 1 } else { rng.chance(2) };
        ee.set_u32(ctl + at::HUMAN, if is_human { 1 } else { 0 });
        set_v(ee, CAR + at::LAST_COFG, last);
        for (o, x) in [(0x50, body.last_position), (0x60, body.position), (0x70, body.velocity), (0x170, body.quat), (0x1b0, body.rates)] {
            set_v(ee, at_body + o, x);
        }
        let lean = f(ee, at::LEAN);

        car::travel_cofg(&car, &mut body, &mut last, is_human, lean);
        ee.call(travel, &[CAR as u64], &[], BUDGET);
        let mut got = vec![];
        let mut want = vec![];
        for (o, x) in [(0x180, body.cofg), (0x60, body.position), (0x50, body.last_position)] {
            got.extend(x.to_bits());
            want.extend(v(ee, at_body + o).to_bits());
        }
        got.extend(last.to_bits());
        want.extend(v(ee, CAR + at::LAST_COFG).to_bits());
        tally.words(&format!("{csv} case {case}"), "cofg, position, last position, last cofg", &got, &want);
    }
    ee.set_u32(ctl + at::HUMAN, human);
    tally.assert(&format!("{csv}: the centre of mass's travel"));
}

/// The measure's first step: the yaw rate held to the live limit. The original's measure runs on until
/// its first call after the hold (the engine's shift), where a hook reads the body's rates and ends the
/// call.
fn the_yaw_hold(kind: Kind, csv: &str, image: &str) {
    let Some(mut d) = drive(kind, csv, image) else { return };
    let measure = d.kit.addr("car_measure").unwrap_or_else(|e| panic!("{e}"));
    let (body_part, shift) = {
        // The body part's step is the measure's first call and the engine's shift its second: found
        // by their jals, not by a name.
        let mut a = measure;
        let mut jals = vec![];
        while jals.len() < 2 {
            let w = d.ee.u32_at(a);
            if w >> 26 == 3 {
                jals.push((w & 0x03ff_ffff) << 2);
            }
            a += 4;
        }
        (jals[0], jals[1])
    };
    let rates = Rc::new(Cell::new([0u32; 4]));
    let seen = rates.clone();
    d.ee.hooks.insert(body_part, Box::new(|_| {}));
    d.ee.hooks.insert(
        shift,
        Box::new(move |ee| {
            let body = ee.u32_at(CAR + at::BODY);
            seen.set([0, 4, 8, 12].map(|i| ee.u32_at(body + 0x1b0 + i)));
            // Out of the measure: its caller's return address is in its frame.
            let done = ee.u32_at(ee.r(29) as u32 + 0x20);
            ee.set(31, done as u64);
        }),
    );
    let ee = &mut d.ee;
    let mut tally = Tally::default();
    let mut rng = Rng(0x5149_d300 ^ kind as u64);
    for case in 0..CASES + d.rows.len() {
        let (row, from_row) = match d.rows.get(case) {
            Some(row) => (row, true),
            None => (&d.rows[rng.next() as usize % d.rows.len()], false),
        };
        lay_row(ee, row);
        let body = ee.u32_at(CAR + at::BODY);
        let mut r = Vf(row.body[3]);
        if !from_row {
            r.0[1] = rng.value(&[0.0, -0.0, 3.0, -3.0, 10.0, -10.0], -20.0, 20.0).0;
            set(ee, at::LIVE_YAW_LIMIT, rng.value(&[3.0, 10.0, 0.0], 0.0, 15.0));
        }
        set_v(ee, body + 0x1b0, r);
        car::hold_yaw_rate(&mut r, f(ee, at::LIVE_YAW_LIMIT));
        rates.set([0; 4]);
        ee.call(measure, &[CAR as u64], &[row.dt.0], BUDGET);
        tally.words(&format!("{csv} case {case}"), "rates", &r.to_bits(), &rates.get());
    }
    tally.assert(&format!("{csv}: the yaw rate hold"));
}

const DRIVES: [(Kind, &str, &str); 3] = [
    (Kind::Baja, "baja.csv", "s149a-baja-switched.ee.bin"),
    (Kind::Buggy, "buggy.csv", "s149a-buggy-switched.ee.bin"),
    (Kind::MonsterTruck, "truck.csv", "s149a-truck-switched.ee.bin"),
];

#[test]
fn the_controls_step_matches_the_original_on_every_car() {
    for (kind, csv, image) in DRIVES {
        the_controls_step(kind, csv, image);
    }
}

#[test]
fn the_input_step_matches_the_original_on_every_car() {
    for (kind, csv, image) in DRIVES {
        the_input_step(kind, csv, image);
    }
}

#[test]
fn the_cofg_travel_matches_the_original_on_every_car() {
    for (kind, csv, image) in DRIVES {
        the_cofg_travel(kind, csv, image);
    }
}

#[test]
fn the_yaw_hold_matches_the_original_on_every_car() {
    for (kind, csv, image) in DRIVES {
        the_yaw_hold(kind, csv, image);
    }
}
