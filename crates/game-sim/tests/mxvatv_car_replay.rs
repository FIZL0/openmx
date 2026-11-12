//! MD107 — **MD39a's five car drives replayed bit-exact** through SLUS-21104's car frame.
//!
//! Each drive starts from its RAM image (`context/mxvatv-ps2/md39a/<car>.ee.bin`: the buggy's and
//! baja's are row 0 of the capture word for word, the other three's the frame before) and runs
//! SLUS-21104's per-frame sim update in the interpreter, the frame that steps the car (MD106), with
//! the car's input step's words fed from the tape (`context/recordings/md39a/drive.p2m2.ticks.csv`)
//! through the port's own input step ([`atv_car::input`], proved on the game's by
//! [`the_car_input_step_is_the_ports`]) at the drive's row lag (two for the buggy and baja, one for
//! the others; with the image's frame that is tape tick `j - 2` for frame `j` on all five). Nothing
//! of the capture's is laid on the machine. Each row's car object, body, engine and four tires must
//! come out word for word, but for those the sim frame never writes (the put-back spot, a position
//! breadcrumb and bit 0x400 of +0x44, all laid outside it). A row the recorder read while the frame
//! ran (torn) must hold every word at some moment about its frame: a frame's end, or the start or end
//! of the vehicle's force, a tire's step or one of its parts, the aero, or a force or moment add.
//!
//! Skips without the images, the captures or the tape.

use game_sim::atv_car::{self, Input, InputKeys};
use game_sim::bindings::Command;
use ps2_float::F;
use ps2_oracle::{Ee, Kit, skip};
use std::collections::HashMap;

/// The sim frame (the sim object's vt +0x40), the game's tick and `$gp` (research/oracle.txt `# F31`).
const FRAME: u32 = 0x0020_4040;
const DT: u32 = 0x3d08_850a;
const GP: u64 = 0x003d_baf0;
/// The sim object's vtable (+4).
const SIM_VTABLE: u32 = 0x003b_9320;
const BUDGET: u64 = 400_000_000;

type Row = HashMap<String, String>;

fn csv(path: std::path::PathBuf) -> Option<Vec<Row>> {
    let Ok(text) = std::fs::read_to_string(&path) else {
        skip(&format!("no {}", path.display()));
        return None;
    };
    let mut lines = text.lines();
    let head: Vec<String> = lines.next()?.split(',').map(String::from).collect();
    Some(lines.map(|l| head.iter().cloned().zip(l.split(',').map(String::from)).collect()).collect())
}

fn hex_words(s: &str) -> Vec<u32> {
    (0..s.len() / 8)
        .map(|i| {
            let b = |j: usize| u8::from_str_radix(&s[8 * i + 2 * j..8 * i + 2 * j + 2], 16).unwrap();
            u32::from_le_bytes([b(0), b(1), b(2), b(3)])
        })
        .collect()
}

/// The sim object: the one word in the image whose +4 is the sim's vtable.
fn sim(ee: &Ee) -> u32 {
    let hits: Vec<u32> = (0x0010_0000..0x01ff_fff0u32).step_by(4).filter(|&a| ee.u32_at(a + 4) == SIM_VTABLE).collect();
    assert_eq!(hits.len(), 1, "sim objects {hits:x?}");
    hits[0]
}

/// F31's hooks: the kernel's DIntr/EIntr, the vehicle's network snapshot send, the thread delay, the
/// sound driver's IOP call, every syscall stub.
fn hooked(mut ee: Ee) -> Ee {
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

/// The blocks compared, as (name, address in the image, bytes): the car object, its body, engine and
/// the tires' raw blocks.
fn blocks(ee: &Ee, car: u32) -> Vec<(String, u32, u32)> {
    let p = |o: u32| ee.u32_at(car + o);
    let mut v = vec![("obj".to_string(), car, 0x1800), ("body".to_string(), p(0xb8), 640), ("engine".to_string(), p(0x1250), 512)];
    for k in 0..4 {
        v.push((format!("tire{k}.raw"), p(0x1254 + 4 * k), 768));
    }
    v
}

fn row_block(row: &Row, name: &str) -> Vec<u32> {
    if name == "obj" {
        return hex_words(&row["obj0"]).into_iter().chain(hex_words(&row["obj1"])).collect();
    }
    hex_words(&row[name])
}

/// The words that differ from the row, as (block, offset, machine, capture), less those the frame
/// never writes.
fn diff(ee: &Ee, car: u32, row: &Row) -> Vec<(String, u32, u32, u32)> {
    let mut out = Vec::new();
    for (name, at, len) in blocks(ee, car) {
        let want = row_block(row, &name);
        for i in 0..len / 4 {
            let (off, got, w) = (4 * i, ee.u32_at(at + 4 * i), want[i as usize]);
            let outside = name == "obj" && (SPOT.contains(&off) || CRUMB.contains(&off) || (off == FLAGS && (got ^ w) == FLAG));
            if got != w && !outside {
                out.push((name.clone(), off, got, w));
            }
        }
    }
    out
}

/// The car's words the sim frame never writes: the put-back spot and its orientation (laid by the
/// put-back outside the frame), a breadcrumb of the car's position (dropped every so far driven,
/// outside the frame), and a flag word's bit 0x400.
const SPOT: std::ops::Range<u32> = 0xd38..0xd70;
const CRUMB: std::ops::Range<u32> = 0xd90..0xd9c;
const FLAGS: u32 = 0x44;
const FLAG: u32 = 0x400;

/// The game's tick counter (+2 a tick), in the image and on every row.
const VSYNC: u32 = 0x003f_a6d8;

/// A drive: its name, the player's car in its image, the car's vtable and the rows' lag behind the
/// tape (tests/mxvatv_car_captures.rs's).
struct Drive {
    name: &'static str,
    car: u32,
    vtable: u32,
    lag: usize,
}

const BUGGY: Drive = Drive { name: "buggy", car: 0x0159_8e10, vtable: 0x003c_ec60, lag: 2 };
const BAJA: Drive = Drive { name: "baja", car: 0x0159_8e10, vtable: 0x003c_ec60, lag: 2 };
const SANDRAIL: Drive = Drive { name: "sandrail", car: 0x01b1_58a0, vtable: 0x003c_ec60, lag: 1 };
const GOLF: Drive = Drive { name: "golf", car: 0x01b0_dac0, vtable: 0x003c_f600, lag: 1 };
const TRUCK: Drive = Drive { name: "truck", car: 0x0159_8e10, vtable: 0x003c_ff80, lag: 1 };

/// The car's words the input step reads and writes (research/oracle.txt `# MD107`).
mod at {
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

fn image(name: &str) -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2/md39a").join(format!("{name}.ee{}", ".bin"));
    if !path.exists() {
        skip(&format!("no {} (research/journal/2026-11-03-md39a-mxvatv-car-captures)", path.display()));
        return None;
    }
    Some(Ee::from_ram(&path).unwrap())
}

fn rows(name: &str) -> Option<Vec<Row>> {
    csv(Kit::load().ok()?.ctx("capture/md39a").join(format!("{name}{}", ".csv")))
}

fn tape() -> Option<Vec<Row>> {
    csv(Kit::load().ok()?.ctx("recordings/md39a").join(format!("drive.p2m2.ticks{}", ".csv")))
}

fn keys_of(ee: &Ee, car: u32) -> InputKeys {
    let f = |o: u32| F(ee.f32_at(car + o));
    InputKeys {
        max_analog_throttle: f(at::MAX_ANALOG_THROTTLE),
        digital_throttle_speed: f(at::DIGITAL_THROTTLE_SPEED),
        digital_side_slip_angle: f(at::DIGITAL_SIDE_SLIP_ANGLE),
    }
}

fn input_of(ee: &Ee, car: u32) -> Input {
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

fn put_input(ee: &mut Ee, car: u32, p: &Input) {
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

// --- the input step ---------------------------------------------------------------------------------

/// SLUS-21104's car input step (a0 the car, a1 the player's record: the player, the control
/// records, stick x, stick y), the gate it asks first (the player record's "is this pad live"), the
/// binding lookup and the front end it is handed (research/oracle.txt `# MD104`, `# MD107`).
const INPUT_STEP: u32 = 0x004c_a010;
const LIVE: u32 = 0x0024_9330;
const BINDING: u32 = 0x0011_8e10;
const FRONT_END: u32 = 0x003e_ab60;
/// The commands the step looks up: gas, brake, hand brake, clutch, the d-pad.
const ASKED: [u8; 8] = [0x0e, 0x0f, 0x18, 0x10, 0, 1, 2, 3];

/// A deterministic generator for the cases (tests/biplane.rs's).
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
    /// A stick: often a signed zero or an end, else anywhere in -1..1, now and then a tiny one.
    fn stick(&mut self) -> f32 {
        match self.next() % 10 {
            0 => 0.0,
            1 => -0.0,
            2 => 1.0,
            3 => -1.0,
            4 => (self.unit() - 0.5) * 1e-12,
            _ => 2.0 * self.unit() - 1.0,
        }
    }
    /// A control's strength: mostly off or full, sometimes between, sometimes -0 or negative.
    fn strength(&mut self) -> f32 {
        match self.next() % 9 {
            0..=3 => 0.0,
            4 | 5 => 1.0,
            6 => -0.0,
            7 => -self.unit(),
            _ => self.unit(),
        }
    }
    /// A key or a measure: a signed zero, the image's own, or something either side of it.
    fn near(&mut self, own: f32) -> f32 {
        match self.next() % 8 {
            0 => 0.0,
            1 => -0.0,
            2 => own,
            3 => -own,
            4 => 1e-30,
            _ => own * 3.0 * (self.unit() - 0.25),
        }
    }
}

/// **SLUS-21104's car input step is [`atv_car::input`]**: ten thousand cases on the buggy's image,
/// each a player (0 or 1), sticks with signed zeros, the ends and tiny values, a strength in every
/// control record the binding lookup can name and in the pad word's, the speed either side of
/// `DigitalThrottleSpeed`, the slip either side of `DigitalSideSlipAngle` (both signs),
/// `MaxAnalogThrottle` with signed zeros, and the words the step keeps (the d-pad flag, the first
/// gas, the stick) on or off. The image's own binding table answers the lookup; the nine words the
/// step writes must equal the port's bit for bit.
#[test]
fn the_car_input_step_is_the_ports() {
    let Some(mut ee) = image(BUGGY.name) else { return };
    let car = BUGGY.car;
    ee.hooks.insert(LIVE, Box::new(|ee: &mut Ee| ee.set(2, 1)));
    ee.set_u32(car + at::LOCAL, 1);
    let front = ee.u32_at(ee.u32_at(FRONT_END));
    let own = keys_of(&ee, car);
    let (record, controls) = (0x01f8_0000u32, 0x01f8_0100u32);
    let mut pick = Pick(0x6d64_3130_37);
    for case in 0..10_000 {
        let player = (pick.next() % 2) as u32;
        let (x, y) = (pick.stick(), pick.stick());
        // Every slot the lookup can land on, a miss (-1) included, and the pad word's (+0x31c).
        let strengths: Vec<f32> = (0..35).map(|_| pick.strength()).collect();
        for (k, &s) in strengths.iter().enumerate() {
            ee.set_f32(controls + 0x18 * k as u32 + 4, s);
        }
        for (k, v) in [player, controls + 0x18, x.to_bits(), y.to_bits()].into_iter().enumerate() {
            ee.set_u32(record + 4 * k as u32, v);
        }
        let keys = InputKeys {
            max_analog_throttle: F(pick.near(own.max_analog_throttle.0)),
            digital_throttle_speed: F(pick.near(own.digital_throttle_speed.0)),
            digital_side_slip_angle: F(pick.near(own.digital_side_slip_angle.0)),
        };
        let speed = F(pick.near(own.digital_throttle_speed.0));
        let side_slip = F(pick.near(own.digital_side_slip_angle.0));
        let before = Input {
            stick: F(pick.stick()),
            lean: F(pick.stick()),
            pad_word: F(pick.stick()),
            gas_pedal: F(pick.strength()),
            vehicle_brake: F(pick.strength()),
            hand_brake: F(pick.strength()),
            clutch: F(pick.strength()),
            dpad: pick.next() % 2 == 0,
            gassed: pick.next() % 2 == 0,
        };
        for (o, v) in [
            (at::MAX_ANALOG_THROTTLE, keys.max_analog_throttle),
            (at::DIGITAL_THROTTLE_SPEED, keys.digital_throttle_speed),
            (at::DIGITAL_SIDE_SLIP_ANGLE, keys.digital_side_slip_angle),
            (at::SPEED, speed),
            (at::SIDE_SLIP, side_slip),
        ] {
            ee.set_u32(car + o, v.to_bits());
        }
        put_input(&mut ee, car, &before);
        let slots: HashMap<u8, i32> = ASKED
            .into_iter()
            .map(|c| {
                ee.set(28, GP);
                ee.call(BINDING, &[u64::from(front), u64::from(c), 2, 10, u64::from(player)], &[], BUDGET);
                (c, ee.r(2) as u32 as i32)
            })
            .collect();
        let value = |c: Command| {
            let k = *slots.get(&c.0).unwrap_or_else(|| panic!("the port asks for command {c:?}"));
            assert!((-1..33).contains(&k), "case {case}: {c:?} looks up control {k}");
            F(strengths[(k + 1) as usize])
        };
        let mut want = before;
        atv_car::input(&keys, &mut want, speed, side_slip, (F(x), F(y)), F(strengths[34]), value);
        ee.set(28, GP);
        ee.call(INPUT_STEP, &[u64::from(car), u64::from(record)], &[], BUDGET);
        let got = input_of(&ee, car);
        assert_eq!(
            format!("{got:?}"),
            format!("{want:?}"),
            "case {case} (stick {x:e}, {y:e}, player {player}, speed {speed:?}, slip {side_slip:?}, keys {keys:?}, before {before:?}): the game, then the port"
        );
        for (name, g, w) in [
            ("stick", got.stick, want.stick),
            ("lean", got.lean, want.lean),
            ("pad word", got.pad_word, want.pad_word),
            ("gas", got.gas_pedal, want.gas_pedal),
            ("brake", got.vehicle_brake, want.vehicle_brake),
            ("hand brake", got.hand_brake, want.hand_brake),
            ("clutch", got.clutch, want.clutch),
        ] {
            assert_eq!(g.to_bits(), w.to_bits(), "case {case}: {name} {g:?} in the game, {w:?} in the port");
        }
    }
}

// --- the drives -------------------------------------------------------------------------------------

/// The button each command the input step asks is on in MX vs. ATV's shipped bindings, as the tape
/// names it; the clutch's is not held anywhere on this tape (it holds only cross, square and circle).
fn button(c: Command) -> Option<&'static str> {
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
fn feed(ee: &mut Ee, d: &Drive, ticks: &[Row], j: usize, back: usize) {
    let Some(t) = j.checked_sub(back).and_then(|t| ticks.get(t)) else { return };
    let held: Vec<&str> = t["buttons"].split(' ').collect();
    let stick = |k: &str| F(t[k].parse::<f32>().unwrap());
    let mut p = input_of(ee, d.car);
    let (speed, side_slip) = (F(ee.f32_at(d.car + at::SPEED)), F(ee.f32_at(d.car + at::SIDE_SLIP)));
    let value = |c: Command| if button(c).is_some_and(|b| held.contains(&b)) { F::ONE } else { F::ZERO };
    atv_car::input(&keys_of(ee, d.car), &mut p, speed, side_slip, (stick("lx"), stick("ly")), F::ZERO, value);
    put_input(ee, d.car, &p);
}

/// Every word compared off the machine, the two the frame never writes masked.
fn words(ee: &Ee, car: u32) -> Vec<u32> {
    blocks(ee, car).iter().flat_map(|(n, at, len)| (0..len / 4).map(move |i| masked(n, 4 * i, ee.u32_at(at + 4 * i)))).collect()
}

fn row_words(ee: &Ee, car: u32, row: &Row) -> Vec<u32> {
    blocks(ee, car).iter().flat_map(|(n, _, len)| row_block(row, n).into_iter().take((len / 4) as usize).enumerate().map(move |(i, w)| masked(n, 4 * i as u32, w))).collect()
}

fn masked(name: &str, off: u32, w: u32) -> u32 {
    match (name, off) {
        ("obj", o) if SPOT.contains(&o) || CRUMB.contains(&o) => 0,
        ("obj", FLAGS) => w & !FLAG,
        _ => w,
    }
}

/// What a replay came to: the rows that agreed with their own frame, the stale ones (the frame
/// before's), the late ones (the frame after's), the torn ones (read while a frame ran: every word
/// that is not its own frame's is one the game held at some point from the frame before's end to the
/// frame after's, a substep's start included, the recorder's poll landing mid-way through the game's
/// frame), and the first row that was none of these, with its differences against its own frame. No
/// word of the capture's is ever laid on the machine, so a row equal again after a torn one shows the
/// machine never left the drive.
#[derive(Default)]
struct Verdict {
    rows: usize,
    agreed: usize,
    stale: Vec<usize>,
    late: Vec<usize>,
    torn: Vec<usize>,
    /// The words torn rows held from another moment than their frame's end, as (word, rows).
    torn_words: std::collections::BTreeMap<String, usize>,
    /// Rows read inside a tire's step after it cleared a word and before it wrote it again.
    cleared: Vec<usize>,
    parted: Option<(usize, String)>,
}

/// One frame of the game's: the input step's words off the tape, then the sim's update.
fn step(ee: &mut Ee, s: u32, d: &Drive, ticks: &[Row], j: &mut usize, back: usize) {
    *j += 1;
    feed(ee, d, ticks, *j, back);
    ee.set(28, GP);
    ee.call(FRAME, &[u64::from(s)], &[f32::from_bits(DT)], BUDGET);
}

/// The Force body's calls the words are kept about, each substep: the vehicle's force (its first; a
/// jump to the vehicle base's), each tire's step and the aero (its last, before the body integrates).
const VEHICLE_FORCE: u32 = 0x004a_4ef0;
const VEHICLE_FORCE_TO: u32 = 0x0049_3f60;
const TIRE_STEP: u32 = 0x004b_5ca0;
const AERO: u32 = 0x004c_b200;
/// The rigid body's force and moment adds (a tire's step adds into the body's accumulators more than
/// once, and the recorder can read between two adds).
const ADD_FORCE: u32 = 0x0049_4000;
const ADD_MOMENT: u32 = 0x0049_7530;
/// The tire step's three parts (its contact, its forces, their hand-out).
const TIRE_PARTS: [u32; 3] = [0x004b_5cf0, 0x004b_62c0, 0x004b_6400];
/// The tire words its step clears before it writes them, two stores apart or more (the contact flag,
/// cleared and set again by the first part; the quad at +0x230, cleared whole by the third, then x and
/// z apart; research/oracle.txt `# MD107`): a row the recorder read in between holds a 0 no kept
/// moment does.
const CLEARED: [&str; 5] = [".raw+0xb0", ".raw+0x230", ".raw+0x234", ".raw+0x238", ".raw+0x23c"];

type Seen = std::rc::Rc<std::cell::RefCell<Vec<Vec<u32>>>>;

/// Hooks the Force body's calls to keep the compared words before and after each (into the returned
/// list), each run as it was: nothing the frame does changes.
fn watch_substeps(ee: &mut Ee, car: u32) -> Seen {
    let seen = Seen::default();
    for (at, to) in [(VEHICLE_FORCE, VEHICLE_FORCE_TO), (TIRE_STEP, TIRE_STEP), (AERO, AERO), (ADD_FORCE, ADD_FORCE), (ADD_MOMENT, ADD_MOMENT)].into_iter().chain(TIRE_PARTS.map(|p| (p, p))) {
        let keep = seen.clone();
        ee.hooks.insert(
            at,
            Box::new(move |ee: &mut Ee| {
                keep.borrow_mut().push(words(ee, car));
                let (sp, ra, stack) = (ee.r(29), ee.r(31), ee.stack);
                let ints: Vec<u64> = (4..12).map(|k| ee.r(k)).collect();
                let floats: Vec<f32> = (12..20).map(|k| ee.f(k)).collect();
                ee.stack = (sp as u32 - 0x100) & !0xf;
                ee.call(to, &ints, &floats, BUDGET);
                (ee.stack, ee.pc) = (stack, at);
                ee.set(29, sp);
                ee.set(31, ra);
                keep.borrow_mut().push(words(ee, car));
            }),
        );
    }
    seen
}

/// Replays a drive in the interpreter from its image, frame by frame; every row is compared with the
/// frame its vsync names. The buggy's and baja's images are their row 0; the other three's are the
/// frame before (their row 0 is the first frame stepped), so frame `j` shows row `j - 1` and its
/// input is tape tick `j - 1 - lag`: two frames back for every drive.
fn interpret(d: &Drive) -> Option<Verdict> {
    let mut ee = hooked(image(d.name)?);
    let (rows, ticks) = (rows(d.name)?, tape()?);
    assert_eq!(ee.u32_at(d.car + 4), d.vtable, "{}: no car at {:#x}", d.name, d.car);
    let s = sim(&ee);
    let behind = usize::from(!diff(&ee, d.car, &rows[0]).is_empty());
    let back = d.lag + behind;
    assert_eq!(back, 2, "{}: frame j's input is not tape tick j - 2", d.name);
    // Every drive's row 0 reads the vsync one tick on from the image's (the recorder's poll lands
    // on the next tick's count, or after the next frame).
    let v0 = rows[0]["vsync"].parse::<usize>().unwrap();
    assert_eq!(v0, ee.u32_at(VSYNC) as usize + 2, "{}: row 0 is not the tick after the image", d.name);
    let at: Vec<usize> = rows.iter().map(|row| (row["vsync"].parse::<usize>().unwrap() - v0) / 2 + behind).collect();
    let names: Vec<String> = blocks(&ee, d.car).iter().flat_map(|(n, _, len)| (0..len / 4).map(move |i| format!("{n}+{:#x}", 4 * i))).collect();
    let seen = watch_substeps(&mut ee, d.car);
    // Every frame's words at its end (the image's first, one past the last row's), and the words as
    // each of its substeps started (none kept for the image's).
    let (mut frames, mut subs, mut j) = (vec![words(&ee, d.car)], vec![Vec::new()], 0);
    let last = at.iter().copied().max().unwrap_or(0) + 1;
    let mut v = Verdict { rows: rows.len(), ..Verdict::default() };
    for (r, &f) in at.iter().enumerate() {
        while j < (f + 1).min(last) {
            step(&mut ee, s, d, &ticks, &mut j, back);
            frames.push(words(&ee, d.car));
            subs.push(std::mem::take(&mut *seen.borrow_mut()));
            // Only the frames about the row are looked at again.
            if j >= 3 {
                subs[j - 3] = Vec::new();
            }
        }
        let want = row_words(&ee, d.car, &rows[r]);
        let own = &frames[f];
        let before = f.checked_sub(1).map(|b| &frames[b]);
        let after = frames.get(f + 1);
        // Every moment from the frame before's end to the frame after's.
        let moments: Vec<&Vec<u32>> = before.into_iter().chain(&subs[f]).chain([own]).chain(subs.get(f + 1).into_iter().flatten()).chain(after).collect();
        if want == *own {
            v.agreed += 1;
        } else if before == Some(&want) {
            v.stale.push(r);
        } else if after == Some(&want) {
            v.late.push(r);
        } else if (0..want.len()).all(|i| moments.iter().any(|m| m[i] == want[i])) {
            v.torn.push(r);
            for i in (0..want.len()).filter(|&i| want[i] != own[i]) {
                *v.torn_words.entry(names[i].clone()).or_default() += 1;
            }
        } else {
            let unheld: Vec<usize> = (0..want.len()).filter(|&i| moments.iter().all(|m| m[i] != want[i])).collect();
            // Every word no kept moment holds is one the tire's step clears, one still the clear's 0
            // (another may be written already: row 686 of the sandrail's holds the new x, z still 0).
            let mid = unheld.iter().all(|&i| CLEARED.iter().any(|c| names[i].ends_with(c)));
            if mid && unheld.iter().any(|&i| want[i] == 0) {
                v.cleared.push(r);
                continue;
            }
            let bad: Vec<String> = unheld
                .iter()
                .take(40)
                .map(|&i| format!("{}: {:#010x} ({}) want {:#010x} ({})", names[i], own[i], f32::from_bits(own[i]), want[i], f32::from_bits(want[i])))
                .collect();
            v.parted = Some((r, format!("held at no moment about frame {f}:\n  {}", bad.join("\n  "))));
            break;
        }
    }
    Some(v)
}

fn interpreted(d: &Drive) {
    let Some(v) = interpret(d) else { return };
    let words: Vec<String> = v.torn_words.iter().map(|(w, n)| format!("{w} ({n})")).collect();
    println!(
        "{}: {} rows agree; stale rows {:?}, late rows {:?}, torn rows {:?}; words torn: {}",
        d.name,
        v.agreed,
        v.stale,
        v.late,
        v.torn,
        words.join(", ")
    );
    println!("{}: rows read between a tire's clear and its write {:?}", d.name, v.cleared);
    if let Some((r, dd)) = &v.parted {
        panic!("{} row {r} of {}: differs\n  {dd}", d.name, v.rows);
    }
    assert_eq!(v.agreed + v.stale.len() + v.late.len() + v.torn.len() + v.cleared.len(), v.rows, "{}: rows unaccounted for", d.name);
    assert!(v.stale.len() + v.late.len() <= v.rows / 20, "{}: {} stale and late rows of {}: re-record it", d.name, v.stale.len() + v.late.len(), v.rows);
}

/// **The buggy's drive replays bit-exact**: every row's car object, body, engine and tires, off the
/// image and the tape through the port's input step.
#[test]
fn the_buggys_drive_replays_bit_exact() {
    interpreted(&BUGGY);
}

/// **The baja's.**
#[test]
fn the_bajas_drive_replays_bit_exact() {
    interpreted(&BAJA);
}

/// **The sandrail's.**
#[test]
fn the_sandrails_drive_replays_bit_exact() {
    interpreted(&SANDRAIL);
}

/// **The golf cart's.**
#[test]
fn the_golf_carts_drive_replays_bit_exact() {
    interpreted(&GOLF);
}

/// **The monster truck's.**
#[test]
fn the_trucks_drive_replays_bit_exact() {
    interpreted(&TRUCK);
}


