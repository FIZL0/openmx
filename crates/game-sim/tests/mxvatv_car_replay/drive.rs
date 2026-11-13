//! MD107's drives, shared by the interpreter's replay (`tests/mxvatv_car_replay.rs`) and the port's
//! beside it (`tests/mxvatv_car_frame/drives.rs`, MD135): the images, the tape, F31's hooks and the
//! frame stepped with the input step's words off the tape.

use game_sim::atv_car::{self, Input, InputKeys};
use game_sim::bindings::Command;
use ps2_float::F;
use ps2_oracle::{Ee, Kit, skip};
use std::collections::HashMap;

/// The sim frame (the sim object's vt +0x40), the game's tick and `$gp` (research/oracle.txt `# F31`).
pub(crate) const FRAME: u32 = 0x0020_4040;
pub(crate) const DT: u32 = 0x3d08_850a;
pub(crate) const GP: u64 = 0x003d_baf0;
/// The sim object's vtable (+4).
pub(crate) const SIM_VTABLE: u32 = 0x003b_9320;
pub(crate) const BUDGET: u64 = 400_000_000;

pub(crate) type Row = HashMap<String, String>;

pub(crate) fn csv(path: std::path::PathBuf) -> Option<Vec<Row>> {
    let Ok(text) = std::fs::read_to_string(&path) else {
        skip(&format!("no {}", path.display()));
        return None;
    };
    let mut lines = text.lines();
    let head: Vec<String> = lines.next()?.split(',').map(String::from).collect();
    Some(lines.map(|l| head.iter().cloned().zip(l.split(',').map(String::from)).collect()).collect())
}

pub(crate) fn hex_words(s: &str) -> Vec<u32> {
    (0..s.len() / 8)
        .map(|i| {
            let b = |j: usize| u8::from_str_radix(&s[8 * i + 2 * j..8 * i + 2 * j + 2], 16).unwrap();
            u32::from_le_bytes([b(0), b(1), b(2), b(3)])
        })
        .collect()
}

/// The sim object: the one word in the image whose +4 is the sim's vtable.
pub(crate) fn sim(ee: &Ee) -> u32 {
    let hits: Vec<u32> = (0x0010_0000..0x01ff_fff0u32).step_by(4).filter(|&a| ee.u32_at(a + 4) == SIM_VTABLE).collect();
    assert_eq!(hits.len(), 1, "sim objects {hits:x?}");
    hits[0]
}

/// F31's hooks: the kernel's DIntr/EIntr, the vehicle's network snapshot send, the thread delay, the
/// sound driver's IOP call, every syscall stub.
pub(crate) fn hooked(mut ee: Ee) -> Ee {
    ee.hooks.insert(0x0034_8fa0, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    ee.hooks.insert(0x0034_8ff0, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    ee.hooks.insert(0x004e_7e70, Box::new(|_: &mut Ee| {}));
    ee.hooks.insert(0x0033_ff58, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    ee.hooks.insert(0x0018_2c90, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    for a in (0x0030_0000..0x0036_0000u32).step_by(4) {
        if ee.u32_at(a) >> 16 == 0x2403 && ee.u32_at(a + 4) == 0x0000_000c && ee.u32_at(a + 8) == 0x03e0_0008 {
            let n = ee.u32_at(a) & 0xffff;
            ee.hooks.insert(a, Box::new(move |ee: &mut Ee| {
                let v = if n == 0x2f { 1 } else { ee.r(4) };
                ee.set(2, v);
            }));
        }
    }
    ee
}


/// The game's tick counter (+2 a tick), in the image and on every row.
pub(crate) const VSYNC: u32 = 0x003f_a6d8;

/// A drive: its name, the player's car in its image, the car's vtable and the rows' lag behind the
/// tape (tests/mxvatv_car_captures.rs's).
pub(crate) struct Drive {
    pub(crate) name: &'static str,
    pub(crate) car: u32,
    pub(crate) vtable: u32,
    pub(crate) lag: usize,
}

pub(crate) const BUGGY: Drive = Drive { name: "buggy", car: 0x0159_8e10, vtable: 0x003c_ec60, lag: 2 };
pub(crate) const BAJA: Drive = Drive { name: "baja", car: 0x0159_8e10, vtable: 0x003c_ec60, lag: 2 };
pub(crate) const SANDRAIL: Drive = Drive { name: "sandrail", car: 0x01b1_58a0, vtable: 0x003c_ec60, lag: 1 };
pub(crate) const GOLF: Drive = Drive { name: "golf", car: 0x01b0_dac0, vtable: 0x003c_f600, lag: 1 };
pub(crate) const TRUCK: Drive = Drive { name: "truck", car: 0x0159_8e10, vtable: 0x003c_ff80, lag: 1 };

/// The car's words the input step reads and writes (research/oracle.txt `# MD107`).
pub(crate) mod at {
    pub const LOCAL: u32 = 0xbc4;
    pub const VEHICLE_BRAKE: u32 = 0x310;
    pub const GAS_PEDAL: u32 = 0x314;
    pub const SPEED: u32 = 0x32c;
    pub const SIDE_SLIP: u32 = 0x330;
    pub const CLUTCH: u32 = 0x1120;
    pub const PAD_WORD: u32 = 0x1124;
    pub const GASSED: u32 = 0x1128;
    pub const HAND_BRAKE: u32 = 0x1138;
    pub const MAX_ANALOG_THROTTLE: u32 = 0x1144;
    pub const DIGITAL_THROTTLE_SPEED: u32 = 0x1148;
    pub const DIGITAL_SIDE_SLIP_ANGLE: u32 = 0x114c;
    pub const DPAD: u32 = 0x11c4;
    pub const STICK: u32 = 0x11c8;
    pub const LEAN: u32 = 0x11cc;
}

pub(crate) fn image(name: &str) -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2/md39a").join(format!("{name}.ee{}", ".bin"));
    if !path.exists() {
        skip(&format!("no {} (research/journal/2026-11-03-md39a-mxvatv-car-captures)", path.display()));
        return None;
    }
    Some(Ee::from_ram(&path).unwrap())
}

pub(crate) fn rows(name: &str) -> Option<Vec<Row>> {
    csv(Kit::load().ok()?.ctx("capture/md39a").join(format!("{name}{}", ".csv")))
}

pub(crate) fn tape() -> Option<Vec<Row>> {
    csv(Kit::load().ok()?.ctx("recordings/md39a").join(format!("drive.p2m2.ticks{}", ".csv")))
}

pub(crate) fn keys_of(ee: &Ee, car: u32) -> InputKeys {
    let f = |o: u32| F(ee.f32_at(car + o));
    InputKeys {
        max_analog_throttle: f(at::MAX_ANALOG_THROTTLE),
        digital_throttle_speed: f(at::DIGITAL_THROTTLE_SPEED),
        digital_side_slip_angle: f(at::DIGITAL_SIDE_SLIP_ANGLE),
    }
}

pub(crate) fn input_of(ee: &Ee, car: u32) -> Input {
    let f = |o: u32| F(ee.f32_at(car + o));
    Input {
        stick: f(at::STICK),
        lean: f(at::LEAN),
        pad_word: f(at::PAD_WORD),
        gas_pedal: f(at::GAS_PEDAL),
        vehicle_brake: f(at::VEHICLE_BRAKE),
        hand_brake: f(at::HAND_BRAKE),
        clutch: f(at::CLUTCH),
        dpad: ee.u32_at(car + at::DPAD) != 0,
        gassed: ee.u32_at(car + at::GASSED) != 0,
    }
}

pub(crate) fn put_input(ee: &mut Ee, car: u32, p: &Input) {
    for (o, v) in [
        (at::STICK, p.stick),
        (at::LEAN, p.lean),
        (at::PAD_WORD, p.pad_word),
        (at::GAS_PEDAL, p.gas_pedal),
        (at::VEHICLE_BRAKE, p.vehicle_brake),
        (at::HAND_BRAKE, p.hand_brake),
        (at::CLUTCH, p.clutch),
    ] {
        ee.set_u32(car + o, v.to_bits());
    }
    ee.set_u32(car + at::DPAD, u32::from(p.dpad));
    ee.set_u32(car + at::GASSED, u32::from(p.gassed));
}

/// The button each command the input step asks is on in MX vs. ATV's shipped bindings, as the tape
/// names it; the clutch's is not held anywhere on this tape (it holds only cross, square and circle).
pub(crate) fn button(c: Command) -> Option<&'static str> {
    match c {
        Command::GAS => Some("cross"),
        Command::BRAKE => Some("square"),
        Command(0x18) => Some("circle"),
        Command::CLUTCH => None,
        Command::LEFT => Some("left"),
        Command::RIGHT => Some("right"),
        Command::UP => Some("up"),
        Command::DOWN => Some("down"),
        c => panic!("the input step asks for command {c:?}"),
    }
}

/// The input step's words for frame `j` (the `j`th from the image), off tape tick `j - back` (a held
/// button a strength of one, the stick the tape's, the pad word's control untouched); the image's
/// own before that.
pub(crate) fn feed(ee: &mut Ee, d: &Drive, ticks: &[Row], j: usize, back: usize) {
    let Some(t) = j.checked_sub(back).and_then(|t| ticks.get(t)) else { return };
    let held: Vec<&str> = t["buttons"].split(' ').collect();
    let stick = |k: &str| F(t[k].parse::<f32>().unwrap());
    let mut p = input_of(ee, d.car);
    let (speed, side_slip) = (F(ee.f32_at(d.car + at::SPEED)), F(ee.f32_at(d.car + at::SIDE_SLIP)));
    let value = |c: Command| if button(c).is_some_and(|b| held.contains(&b)) { F::ONE } else { F::ZERO };
    atv_car::input(&keys_of(ee, d.car), &mut p, speed, side_slip, (stick("lx"), stick("ly")), F::ZERO, value);
    put_input(ee, d.car, &p);
}

/// One frame of the game's: the input step's words off the tape, then the sim's update.
pub(crate) fn step(ee: &mut Ee, s: u32, d: &Drive, ticks: &[Row], j: &mut usize, back: usize) {
    *j += 1;
    feed(ee, d, ticks, *j, back);
    ee.set(28, GP);
    ee.call(FRAME, &[u64::from(s)], &[f32::from_bits(DT)], BUDGET);
}
