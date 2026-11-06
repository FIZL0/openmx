//! MD104 — MD80a's SLUS-21104 Kiowa flights, replayed, and flown by the port.
//!
//! The captures (`context/capture/md80a/{kiowa,kiowa3}.csv`, re-recorded by MD104 with
//! `research/md80a/fly.sh` from MD80a's Desert Oasis states) hold, every 30 Hz tick, the helicopter
//! object, its body, rotor and aero model. The images (`context/mxvatv-ps2/md80a/NAME.ee.bin`,
//! `research/md80a/ram.py NAME`) are each flight's start.
//!
//! **The interpreter** runs SLUS-21104's per-frame sim update off the image, as
//! tests/mxvatv_biplane_replay.rs does for the plane (the same frame, hooks and dt), with the
//! helicopter's input step's five words fed from each tape tick through the port's own input step
//! ([`Pad::from_sticks_mxvatv`], proved on the game's by [`the_kiowa_input_step_is_the_ports`]). Every
//! other word is the interpreter's, and each row's body, rotor, aero model and measure must come out
//! word for word.
//!
//! **The port** flies them too: a [`Kiowa`] and its [`Ride`] seeded off the interpreter's RAM every
//! [`STRETCH`] frames and flown beside it through [`Kiowa::tick_mxvatv`] on the same pad words, its
//! body word for word against the game's after each frame, as far as [`KIOWA_AGREES_THROUGH`] and
//! [`KIOWA3_AGREES_THROUGH`] pin.
//!
//! Skips without the images, the captures or the tapes.

use game_sim::biplane::{Body, World};
use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::kiowa::{self, Kiowa, Mxvatv, Pad, Ride};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::collections::HashMap;

/// The sim frame (the sim object's vt +0x40), the game's tick and `$gp` (research/oracle.txt `# F31`).
const FRAME: u32 = 0x0020_4040;
const DT: u32 = 0x3d08_850a;
const GP: u64 = 0x003d_baf0;
/// The sim object's vtable (+4), and the Kiowa's.
const SIM_VTABLE: u32 = 0x003b_9320;
const KIOWA_VTABLE: u32 = 0x003c_fad0;
const BUDGET: u64 = 400_000_000;

/// A flight: its name and the player's helicopter in its image.
struct Flight {
    name: &'static str,
    heli: u32,
}

const KIOWA: Flight = Flight { name: "kiowa", heli: 0x01ab_7d20 };
const KIOWA3: Flight = Flight { name: "kiowa3", heli: 0x014c_8f00 };

/// The helicopter's fields, by offset (`# MD80c`).
mod at {
    pub const BODY: u32 = 0xb8;
    pub const RECORD: u32 = 0xc8;
    pub const GRAVITY: u32 = 0xe8;
    pub const PLAYER: u32 = 0x684;
    pub const SPEED: u32 = 0x32c;
    pub const REASON: u32 = 0xc80;
    pub const CLOCK: u32 = 0xc84;
    pub const DOWN: u32 = 0xc8c;
    pub const CLIMB: u32 = 0x10f0;
    pub const DESCEND: u32 = 0x10f4;
    pub const PAD: u32 = 0x10fc; // lateral, longitudinal, pedal
    pub const ROTOR: u32 = 0x11c0;
    pub const AERO: u32 = 0x11c4;
    pub const WAS_TOUCHING: u32 = 0x1198;
}

type Row = HashMap<String, String>;

fn image(name: &str) -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2/md80a").join(format!("{name}.ee{}", ".bin"));
    if !path.exists() {
        skip(&format!("no {} (python3 -I research/md80a/ram.py {name})", path.display()));
        return None;
    }
    Some(Ee::from_ram(&path).unwrap())
}

fn csv(path: std::path::PathBuf) -> Option<Vec<Row>> {
    let Ok(text) = std::fs::read_to_string(&path) else {
        skip(&format!("no {} (research/md80a/fly.sh)", path.display()));
        return None;
    };
    let mut lines = text.lines();
    let head: Vec<String> = lines.next()?.split(',').map(String::from).collect();
    Some(lines.map(|l| head.iter().cloned().zip(l.split(',').map(String::from)).collect()).collect())
}

fn rows(name: &str) -> Option<Vec<Row>> {
    csv(Kit::load().ok()?.ctx("capture/md80a").join(format!("{name}{}", ".csv")))
}

/// A flight's tape (`research/md80a/fly.sh` writes it beside the `.p2m2`): per tick, the buttons held
/// and the left stick.
fn tape(name: &str) -> Option<Vec<Row>> {
    csv(Kit::load().ok()?.ctx("recordings/md80a").join(format!("{name}.p2m2.ticks{}", ".csv")))
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

/// The helicopter's own words compared on every row (`obj0`, `obj1` in the capture): the measure
/// (+0x32c..+0x3b0), the wreck words, the pad, the controller's commands and loop (+0x10f0..+0x1180)
/// and the touch flag. The rest of the object holds the vehicle base's words the frame does not
/// write, or pointers.
const HELI_WORDS: [(u32, u32); 4] = [(0x32c, 0x3b0), (0xc80, 0xc90), (0x10f0, 0x1180), (0x1198, 0x119c)];

/// The blocks compared, as (name, address in the image, bytes, the row's column, the column's start
/// in the helicopter).
fn blocks(ee: &Ee, heli: u32) -> Vec<(&'static str, u32, u32)> {
    let p = |o: u32| ee.u32_at(heli + o);
    vec![("body", p(at::BODY), 640), ("rotor", p(at::ROTOR), 512), ("aero", p(at::AERO), 512)]
}

/// Every word compared, as (what, address), and the same words off a row.
fn addresses(ee: &Ee, heli: u32) -> Vec<(String, u32)> {
    let mut out: Vec<(String, u32)> = Vec::new();
    for (name, at, len) in blocks(ee, heli) {
        out.extend((0..len / 4).map(|i| (format!("{name}+{:#x}", 4 * i), at + 4 * i)));
    }
    for (from, to) in HELI_WORDS {
        out.extend((from..to).step_by(4).map(|o| (format!("heli+{o:#x}"), heli + o)));
    }
    out
}

fn row_words(row: &Row) -> Vec<u32> {
    let mut out: Vec<u32> = ["body", "rotor", "aero"].iter().flat_map(|n| hex_words(&row[*n])).collect();
    let obj: Vec<u32> = hex_words(&row["obj0"]).into_iter().chain(hex_words(&row["obj1"])).collect();
    for (from, to) in HELI_WORDS {
        out.extend((from..to).step_by(4).map(|o| obj[o as usize / 4]));
    }
    out
}

fn words(ee: &Ee, heli: u32) -> Vec<u32> {
    addresses(ee, heli).iter().map(|&(_, a)| ee.u32_at(a)).collect()
}

fn diff(ee: &Ee, heli: u32, row: &Row) -> Vec<String> {
    addresses(ee, heli)
        .into_iter()
        .zip(row_words(row))
        .filter(|((_, a), w)| ee.u32_at(*a) != *w)
        .map(|((name, a), w)| {
            let got = ee.u32_at(a);
            format!("{name}: {got:#010x} ({}) want {w:#010x} ({})", f32::from_bits(got), f32::from_bits(w))
        })
        .collect()
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

/// The game's tick counter (+2 a tick), in the image and on every row.
const VSYNC: u32 = 0x003f_a6d8;

/// The button each command is on in MX vs. ATV's shipped bindings, as the tape names it.
fn button(c: game_sim::bindings::Command) -> &'static str {
    use game_sim::bindings::Command;
    match c {
        kiowa::command_mxvatv::CLIMB => "cross",
        kiowa::command_mxvatv::DESCEND => "square",
        kiowa::command_mxvatv::PEDAL_LEFT => "l1",
        kiowa::command_mxvatv::PEDAL_RIGHT => "r1",
        Command::LEFT => "left",
        Command::RIGHT => "right",
        Command::UP => "up",
        Command::DOWN => "down",
        c => panic!("the input step asks for command {c:?}"),
    }
}

/// The input step on a tape tick: a held button is a strength of one, the stick is the tape's.
fn pad_of_tick(tick: &Row) -> Pad {
    let held: Vec<&str> = tick["buttons"].split(' ').collect();
    let stick = |k: &str| F(tick[k].parse::<f32>().unwrap());
    Pad::from_sticks_mxvatv((stick("lx"), stick("ly")), |c| if held.contains(&button(c)) { F::ONE } else { F::ZERO })
}

fn put_pad(ee: &mut Ee, heli: u32, p: &Pad) {
    for (o, v) in [(at::CLIMB, p.climb), (at::DESCEND, p.descend), (at::PAD, p.lateral), (at::PAD + 4, p.longitudinal), (at::PAD + 8, p.pedal)] {
        ee.set_u32(heli + o, v.to_bits());
    }
}

fn pad_of(ee: &Ee, heli: u32) -> Pad {
    let w = |o: u32| F(ee.f32_at(heli + o));
    Pad { climb: w(at::CLIMB), descend: w(at::DESCEND), lateral: w(at::PAD), longitudinal: w(at::PAD + 4), pedal: w(at::PAD + 8) }
}

/// The input step's words for frame `j` (the `j`th from the image): tape tick `j - 2`'s, or the
/// image's own for the first two.
fn feed(ee: &mut Ee, heli: u32, ticks: &[Row], j: usize) {
    let Some(t) = j.checked_sub(2).and_then(|t| ticks.get(t)) else { return };
    put_pad(ee, heli, &pad_of_tick(t));
}

/// A flight's rows by the frame each shows (`j`, the frames stepped from the image): each row's
/// vsync is its frame's, or every row is one frame behind it (its first row is the image itself).
fn by_frame(ee: &Ee, heli: u32, rows: &[Row]) -> Vec<(usize, usize)> {
    let v0 = ee.u32_at(VSYNC) as usize;
    let behind = usize::from(diff(ee, heli, &rows[0]).is_empty());
    rows.iter()
        .enumerate()
        .map(|(r, row)| ((row["vsync"].parse::<usize>().unwrap() - v0) / 2 - behind, r))
        .collect()
}

/// What a replay came to: the rows that agreed, the stale ones (a row that shows the frame before
/// its own), the late ones (a row that shows the frame after its own), and the first row that was
/// none of these, with its differences against its own frame.
struct Verdict {
    rows: usize,
    agreed: usize,
    stale: Vec<usize>,
    late: Vec<usize>,
    parted: Option<(usize, Vec<String>)>,
}

/// One frame of the game's: the input step's words off the tape, then the sim's update.
fn step(ee: &mut Ee, s: u32, heli: u32, ticks: &[Row], j: &mut usize) {
    *j += 1;
    feed(ee, heli, ticks, *j);
    ee.set(28, GP);
    ee.call(FRAME, &[u64::from(s)], &[f32::from_bits(DT)], BUDGET);
}

/// Replays a flight in the interpreter from its image, frame by frame; every row present is
/// compared with the frame it shows.
fn interpret(f: &Flight) -> Option<Verdict> {
    let mut ee = hooked(image(f.name)?);
    let (rows, ticks) = (rows(f.name)?, tape(f.name)?);
    assert_eq!(ee.u32_at(f.heli + 4), KIOWA_VTABLE, "{}: no Kiowa at {:#x}", f.name, f.heli);
    let s = sim(&ee);
    let frames = by_frame(&ee, f.heli, &rows);
    let mut v = Verdict { rows: rows.len(), agreed: 0, stale: Vec::new(), late: Vec::new(), parted: None };
    let (mut before, mut j) = (words(&ee, f.heli), 0);
    for (k, &(at, r)) in frames.iter().enumerate() {
        while j < at {
            before = words(&ee, f.heli);
            step(&mut ee, s, f.heli, &ticks, &mut j);
        }
        let want = row_words(&rows[r]);
        if j > at && want == before {
            // The row before was read late, onto this row's own frame.
            v.agreed += 1;
            continue;
        }
        if want == words(&ee, f.heli) {
            v.agreed += 1;
        } else if want == before {
            v.stale.push(r);
        } else if frames.get(k + 1).is_none_or(|&(next, _)| next > at) && {
            let d = diff(&ee, f.heli, &rows[r]);
            before = words(&ee, f.heli);
            step(&mut ee, s, f.heli, &ticks, &mut j);
            if want == words(&ee, f.heli) {
                true
            } else {
                v.parted = Some((r, d));
                false
            }
        } {
            v.late.push(r);
        } else {
            v.parted.get_or_insert_with(|| (r, diff(&ee, f.heli, &rows[r])));
            break;
        }
    }
    Some(v)
}

fn interpreted(f: &Flight) {
    let Some(v) = interpret(f) else { return };
    println!("{}: {} rows agree, stale rows {:?}, late rows {:?}", f.name, v.agreed, v.stale, v.late);
    if let Some((r, d)) = &v.parted {
        panic!("{} row {r} of {}: {} words differ\n  {}", f.name, v.rows, d.len(), d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  "));
    }
    assert!(v.stale.len() <= 2, "{}: {} stale rows: re-record it", f.name, v.stale.len());
    assert!(v.late.len() <= v.rows / 5, "{}: {} late rows of {}: re-record it", f.name, v.late.len(), v.rows);
}

/// **The interpreter flies kiowa bit-exact**: every row's body, rotor, aero model and the
/// helicopter's measure, wreck, pad and controller words, off the image and the tape.
#[test]
fn the_interpreter_flies_kiowa_bit_exact() {
    interpreted(&KIOWA);
}

/// **And kiowa3** (the second model).
#[test]
fn the_interpreter_flies_kiowa3_bit_exact() {
    interpreted(&KIOWA3);
}

// --- the input step ---------------------------------------------------------------------------------

/// SLUS-21104's Kiowa input step (a0 the helicopter, a1 the player's record: the player, the control
/// records, stick x, stick y), the gate it asks first (the player record's "is this pad live"), the
/// binding lookup and the front end it is handed (research/oracle.txt `# MD104`).
const INPUT_STEP: u32 = 0x004d_1fa0;
const LIVE: u32 = 0x0024_9330;
const BINDING: u32 = 0x0011_8e10;
const FRONT_END: u32 = 0x003e_ab60;
/// The helicopter's word the step is skipped without.
const LOCAL: u32 = 0xbc4;

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
    /// A control's strength: mostly off or full, sometimes between, sometimes -0.
    fn strength(&mut self) -> f32 {
        match self.next() % 8 {
            0..=3 => 0.0,
            4 | 5 => 1.0,
            6 => -0.0,
            _ => self.unit(),
        }
    }
}

/// **SLUS-21104's Kiowa input step is [`Pad::from_sticks_mxvatv`]**: ten thousand cases on the kiowa
/// image, each a player (0 or 1), sticks with signed zeros, the ends and tiny values, and a strength
/// in every control record the binding lookup can name. The image's own binding table answers the
/// lookup (the commands the port asks are looked up the same way); the five words the step writes
/// must equal the port's bit for bit.
#[test]
fn the_kiowa_input_step_is_the_ports() {
    let Some(mut ee) = image("kiowa") else { return };
    let heli = KIOWA.heli;
    ee.hooks.insert(LIVE, Box::new(|ee: &mut Ee| ee.set(2, 1)));
    ee.set_u32(heli + LOCAL, 1);
    let front = ee.u32_at(ee.u32_at(FRONT_END));
    let (record, controls) = (0x01f8_0000u32, 0x01f8_0100u32);
    let mut pick = Pick(0x6d64_3130);
    for case in 0..10_000 {
        let player = (pick.next() % 2) as u32;
        let (x, y) = (pick.stick(), pick.stick());
        // Every slot the lookup can land on, a miss (-1) included.
        let strengths: Vec<f32> = (0..34).map(|_| pick.strength()).collect();
        for (k, &s) in strengths.iter().enumerate() {
            ee.set_f32(controls + 0x18 * k as u32 + 4, s);
        }
        for (k, v) in [player, controls + 0x18, x.to_bits(), y.to_bits()].into_iter().enumerate() {
            ee.set_u32(record + 4 * k as u32, v);
        }
        for o in [at::CLIMB, at::DESCEND, at::PAD, at::PAD + 4, at::PAD + 8] {
            ee.set_u32(heli + o, 0x7fc0_0001);
        }
        // The eight commands the step asks for, looked up in the image's own table.
        let slots: HashMap<u8, i32> = [0x1c, 0x1d, 0x1e, 0x1f, 0, 1, 2, 3]
            .into_iter()
            .map(|c: u8| {
                ee.set(28, GP);
                ee.call(BINDING, &[u64::from(front), u64::from(c), 2, 10, u64::from(player)], &[], BUDGET);
                (c, ee.r(2) as u32 as i32)
            })
            .collect();
        let value = |c: game_sim::bindings::Command| {
            let k = *slots.get(&c.0).unwrap_or_else(|| panic!("the port asks for command {c:?}"));
            assert!((-1..33).contains(&k), "case {case}: {c:?} looks up control {k}");
            F(strengths[(k + 1) as usize])
        };
        let want = Pad::from_sticks_mxvatv((F(x), F(y)), value);
        ee.set(28, GP);
        ee.call(INPUT_STEP, &[u64::from(heli), u64::from(record)], &[], BUDGET);
        let got = pad_of(&ee, heli);
        for (name, g, w) in [
            ("climb", got.climb, want.climb),
            ("descend", got.descend, want.descend),
            ("lateral", got.lateral, want.lateral),
            ("longitudinal", got.longitudinal, want.longitudinal),
            ("pedal", got.pedal, want.pedal),
        ] {
            assert_eq!(g.to_bits(), w.to_bits(), "case {case} ({x:e}, {y:e}, player {player}): {name} {g:?} in the game, {w:?} in the port");
        }
    }
}

// --- the crash test's strike (reason 0) --------------------------------------------------------------

/// SLUS-21104's Kiowa crash test (`# MD80c`), the terrain query its in-ground loop asks (a0 the
/// terrain, a1 the point: the answer comes back in the point's `y`) and the collision object's
/// contact search (`# MD104`).
const CRASH_TEST: u32 = 0x004d_5410;
const TERRAIN_QUERY: u32 = 0x0048_33c0;
const CONTACT_SEARCH: u32 = 0x0011_51c0;

/// The crash test's words besides [`at`]'s: the measure's matrix and the terrain it asks, the rotor's
/// collision object (its spheres: count, entries of 32 bytes with the centre at +0x10; its vtable at
/// +0xc and the matrix update at +0xcc; the hit flag, bit 5 of +0x126; the hit's surface through
/// +0x7c, +0x9c), the two masks and MinWreckSpeed.
const MATRIX: u32 = 0x370;
const TERRAIN: u32 = 0x3b4;
const ROTOR_OBJECT: u32 = 0x11c8;
const SPHERES: (u32, u32) = (0x178, 0x180);
const CRASH_MASK: u32 = 0x11cc;
const STRUCK_MASK: u32 = 0x11d0;
const MIN_WRECK_SPEED: u32 = 0x1194;
/// The surface's flags and the record's surface (+0x20c).
const SURFACE_FLAGS: u32 = 0x94;
const RECORD_SURFACE: u32 = 0x20c;

/// **The crash test of a helicopter still flying is [`kiowa::Crash::step_mxvatv`] with its strike
/// from [`kiowa::rotor_in_ground`] and the contact search's surface**: ten thousand cases on the kiowa
/// image, each a measure's matrix, the terrain there or not, some of the rotor object's spheres
/// (the image's own centres), each sphere's ground answer under, at or over it, a contact hit or not
/// on a surface whose flags may meet the struck mask, and the record touching or not on a surface
/// that may meet the crash mask, at a speed under, at or over MinWreckSpeed. The terrain query and the
/// contact search are hooked to give the case's answers (and record what they were asked), the
/// rotor object's matrix update to do nothing, the wreck check to say no; the wreck slot records
/// its reason. The points the game asks the terrain about must be the port's bit for bit, and the
/// game's wreck the port's.
#[test]
fn the_kiowa_strike_matches_the_original() {
    use game_sim::math::Mat4;
    use std::cell::RefCell;
    use std::rc::Rc;
    let Some(mut ee) = image("kiowa") else { return };
    let heli = KIOWA.heli;
    let vtable = ee.u32_at(heli + 4);
    let record = ee.u32_at(heli + at::RECORD);
    let obj = ee.u32_at(heli + ROTOR_OBJECT);
    assert_ne!(obj, 0, "the rotor's collision object");
    let terrain = ee.u32_at(heli + TERRAIN);
    assert_ne!(terrain, 0, "the measure's terrain");
    let (count, entries) = (ee.u32_at(obj + SPHERES.0), ee.u32_at(obj + SPHERES.1));
    assert!((1..64).contains(&count), "{count} rotor spheres");
    let centres: Vec<Vf> = (0..count).map(|k| Vf([0, 4, 8, 12].map(|o| ee.f32_at(entries + 32 * k + 0x10 + o)))).collect();
    let (crash_mask, struck_mask) = (ee.u32_at(heli + CRASH_MASK), ee.u32_at(heli + STRUCK_MASK));
    println!("{count} rotor spheres; the crash mask {crash_mask:#x}, the struck mask {struck_mask:#x}");

    // The case's answers, and what the game asked.
    let answers = Rc::new(RefCell::new(Vec::<f32>::new()));
    let asked = Rc::new(RefCell::new(Vec::<Vf>::new()));
    let (an, ak) = (answers.clone(), asked.clone());
    ee.hooks.insert(TERRAIN_QUERY, Box::new(move |ee: &mut Ee| {
        let p = ee.r(5) as u32;
        ak.borrow_mut().push(Vf([0, 4, 8, 12].map(|o| ee.f32_at(p + o))));
        let k = ak.borrow().len() - 1;
        ee.set_f32(p + 4, an.borrow()[k]);
        ee.set(2, 0);
    }));
    let hit = Rc::new(RefCell::new(None::<u32>));
    let h = hit.clone();
    // The hit's chain: obj+0x7c -> a block whose +0x9c -> a word holding the surface.
    let (block, cell, surface, record_surface) = (0x01f8_0000u32, 0x01f8_0200u32, 0x01f8_0300u32, 0x01f8_0400u32);
    ee.set_u32(obj + 0x7c, block);
    ee.set_u32(block + 0x9c, cell);
    ee.hooks.insert(CONTACT_SEARCH, Box::new(move |ee: &mut Ee| {
        assert_eq!(ee.r(6), 1, "the contact search's third argument");
        let flags = ee.u32_at(obj + 0x126) & !0x20;
        match *h.borrow() {
            Some(s) => {
                ee.set_u32(obj + 0x126, flags | 0x20);
                ee.set_u32(cell, s);
            }
            None => ee.set_u32(obj + 0x126, flags),
        }
        ee.set(2, 0);
    }));
    let updated = Rc::new(RefCell::new(0usize));
    let u = updated.clone();
    ee.hooks.insert(ee.u32_at(ee.u32_at(obj + 0xc) + 0xcc), Box::new(move |ee: &mut Ee| {
        assert_eq!(ee.r(5) as u32, heli + MATRIX, "the rotor object's matrix");
        *u.borrow_mut() += 1;
    }));
    ee.hooks.insert(ee.u32_at(vtable + 0x16c), Box::new(|ee: &mut Ee| ee.set(2, 0)));
    let wrecked = Rc::new(RefCell::new(Vec::<u32>::new()));
    let w = wrecked.clone();
    ee.hooks.insert(ee.u32_at(vtable + 0x368), Box::new(move |ee: &mut Ee| w.borrow_mut().push(ee.r(5) as u32)));

    let mut pick = Pick(0x6d64_3130_3473);
    let mut counts = [0usize; 4];
    for case in 0..10_000 {
        ee.set_u32(heli + at::REASON, 0);
        let n = (pick.next() % (u64::from(count) + 1)) as u32;
        ee.set_u32(obj + SPHERES.0, n);
        let has_terrain = pick.next() % 8 != 0;
        ee.set_u32(heli + TERRAIN, if has_terrain { terrain } else { 0 });
        let rows: [Vf; 4] = std::array::from_fn(|r| {
            let lane = |pick: &mut Pick| if r == 3 { 200.0 * (pick.unit() - 0.5) } else { 2.0 * pick.unit() - 1.0 };
            Vf([lane(&mut pick), lane(&mut pick), lane(&mut pick), if r == 3 { 1.0 } else { 0.0 }])
        });
        for (r, v) in rows.iter().enumerate() {
            for k in 0..4 {
                ee.set_f32(heli + MATRIX + 16 * r as u32 + 4 * k as u32, v.0[k]);
            }
        }
        let matrix = Mat4(rows);
        // Each sphere's ground: under, at or over the point the port works out.
        let mut port_points = Vec::new();
        let mut answer_of = Vec::new();
        let in_ground = has_terrain
            && kiowa::rotor_in_ground(&matrix, &centres[..n as usize], |p| {
                let y = match pick.next() % 4 {
                    0 => p.y(),
                    1 => p.y() + pick.unit(),
                    2 => f32::from_bits(p.y().to_bits().wrapping_add(1)),
                    _ => p.y() - 10.0 * pick.unit(),
                };
                port_points.push(p);
                answer_of.push(y);
                let mut q = p;
                q[1] = y;
                q
            });
        *answers.borrow_mut() = answer_of;
        asked.borrow_mut().clear();
        // The contact search's hit, and its surface's flags.
        let flags = |pick: &mut Pick, mask: u32| match pick.next() % 4 {
            0 => 0,
            1 => mask,
            2 => (pick.next() as u32) & !mask,
            _ => pick.next() as u32,
        };
        let struck = match pick.next() % 3 {
            0 => {
                *hit.borrow_mut() = None;
                false
            }
            1 => {
                *hit.borrow_mut() = Some(0);
                false
            }
            _ => {
                let f = flags(&mut pick, struck_mask);
                ee.set_u32(surface + SURFACE_FLAGS, f);
                *hit.borrow_mut() = Some(surface);
                f & struck_mask == 0
            }
        };
        let touching = pick.next() % 2 == 0;
        ee.set_u32(record + 0x34, u32::from(touching));
        let ignored = if pick.next() % 3 == 0 {
            ee.set_u32(record + RECORD_SURFACE, 0);
            false
        } else {
            let f = flags(&mut pick, crash_mask);
            ee.set_u32(record_surface + SURFACE_FLAGS, f);
            ee.set_u32(record + RECORD_SURFACE, record_surface);
            f & crash_mask != 0
        };
        let min = 40.0 * pick.unit();
        let speed = match pick.next() % 3 {
            0 => min,
            _ => 80.0 * pick.unit(),
        };
        ee.set_f32(heli + MIN_WRECK_SPEED, min);
        ee.set_f32(heli + at::SPEED, speed);
        wrecked.borrow_mut().clear();
        *updated.borrow_mut() = 0;

        ee.set(28, GP);
        ee.call(CRASH_TEST, &[u64::from(heli)], &[1.0 / 30.0], BUDGET);

        let mut crash = kiowa::Crash { min_wreck_speed: F(min), ..kiowa::Crash::default() };
        let c = kiowa::CrashContact { touching, ignored, speed: F(speed) };
        let (mut v, mut spin, mut scale, mut rng) = (Vf::default(), Vf::default(), F::ONE, game_sim::rng::Rng::from_seed(1));
        let out = crash.step_mxvatv(&c, None, Vf::default(), &mut v, &mut spin, &mut scale, &mut rng, F(1.0 / 30.0), || kiowa::Strike { in_ground, struck });

        let got = asked.borrow();
        assert_eq!(got.len(), port_points.len(), "case {case}: the terrain asked {} times, the port {}", got.len(), port_points.len());
        for (k, (g, p)) in got.iter().zip(&port_points).enumerate() {
            assert_eq!(g.0.map(f32::to_bits), p.0.map(f32::to_bits), "case {case}: sphere {k}'s point {g:?} in the game, {p:?} in the port");
        }
        assert_eq!(*updated.borrow(), 1, "case {case}: the rotor object's matrix update");
        let want: Vec<u32> = out.wreck.into_iter().collect();
        assert_eq!(*wrecked.borrow(), want, "case {case}: the game's wrecks against the port's (in ground {in_ground}, struck {struck}, touching {touching}, ignored {ignored}, {speed} against {min})");
        assert_eq!(crash.reason, out.wreck.unwrap_or(0), "case {case}: the port's reason");
        counts[0] += usize::from(in_ground);
        counts[1] += usize::from(!in_ground && struck);
        counts[2] += usize::from(!in_ground && !struck && out.wreck.is_some());
        counts[3] += usize::from(out.wreck.is_none());
    }
    println!("the strike: 10000 cases ({} in the ground, {} struck, {} too fast, {} flying on), bit-exact", counts[0], counts[1], counts[2], counts[3]);
}

// --- the port flies them -----------------------------------------------------------------------------

/// Frames between seeds: the port is seeded off the interpreter's RAM every so many frames of the
/// flight (the first seed is the image itself) and flown on until the next.
const STRETCH: usize = 100;

/// **How far the port flies each stretch** with every body word equal to the game's, frame by frame
/// (a ratchet: raise it as the port gains SLUS-21104's ground contact). One entry per stretch,
/// `STRETCH` the most; the last stretch of each flight ends with its capture. Every stretch seeded in
/// the air agrees to its end but the one each landing is in (kiowa's at frame 1092, kiowa3's at
/// 1191); the stretches seeded on the ground (the first, before the take-off, and those after the
/// landing) part on their first frame, the game's record touching: the port's [`Air`] has no
/// ground contact (MD112).
const KIOWA_AGREES_THROUGH: &[usize] = &[0, 100, 100, 100, 100, 100, 100, 100, 100, 100, 92, 0, 0];
const KIOWA3_AGREES_THROUGH: &[usize] = &[0, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 91, 0, 0];

/// The helicopter's further words the seed reads (`# MD80c`).
mod seed_at {
    pub const WRECK_TIME: u32 = 0xc88;
    pub const FRACTION: u32 = 0x10f8;
    pub const OUT: u32 = 0x1108; // collective, lateral, longitudinal, pedal, filtered
    pub const LAST_STICK: u32 = 0x111c;
    pub const FLOWN: u32 = 0x1124;
    pub const HOLD: u32 = 0x1138;
    pub const HOLD_TIMER: u32 = 0x113c;
    pub const INTEGRAL: u32 = 0x1154;
    pub const SPEED_COMMAND: u32 = 0x1170;
    pub const MIN_WRECK_SPEED: u32 = 0x1194;
    pub const FILTER: u32 = 0x11d8;
    pub const BETA: u32 = 0x330;
    pub const ALPHA: u32 = 0x334;
    pub const WORLD_RATES: u32 = 0x340;
    pub const LOCAL: u32 = 0x350;
    pub const EULER: u32 = 0x360;
    pub const MATRIX: u32 = 0x370;
    pub const FORWARD: u32 = 0xbe0;
    pub const UP: u32 = 0xbf0;
}

/// The controller's keys, by offset, in [`kiowa::Controller`]'s order (the rate commands after).
const CONTROLLER_KEYS: [u32; 23] = [
    0x1128, 0x112c, 0x1130, 0x1134, 0x1140, 0x1144, 0x1148, 0x114c, 0x1150, 0x1158, 0x115c, 0x1160, 0x1164, 0x1168,
    0x116c, 0x1174, 0x1178, 0x117c, 0x1180, 0x1184, 0x1188, 0x118c, 0x1190,
];
const MIN_RATE: u32 = 0x11a0;
const MAX_RATE: u32 = 0x11b0;
/// The aero model's keys, by offset, in [`kiowa::Aero`]'s order; the pedals'; the outputs.
const AERO_KEYS: [u32; 20] =
    [0x24, 0x28, 0x2c, 0x30, 0x3c, 0x54, 0x5c, 0x68, 0x6c, 0x74, 0x78, 0x7c, 0x80, 0x84, 0x88, 0x90, 0x94, 0xf4, 0xfc, 0x100];
const AERO_PEDAL: u32 = 0x98;
const AERO_OUT: u32 = 0x128;

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| ee.f32_at(a + k)))
}

/// The rigid body's quadwords in SLUS-21104's layout (`# MD75`) as [`Body`]'s fields.
const BODY_QUADS: [(u32, fn(&mut Body) -> &mut Vf); 20] = [
    (0x30, |b| &mut b.damping),
    (0x60, |b| &mut b.last_position),
    (0x70, |b| &mut b.position),
    (0x80, |b| &mut b.velocity),
    (0x90, |b| &mut b.last_force),
    (0xa0, |b| &mut b.force),
    (0xb0, |b| &mut b.accel),
    (0xc0, |b| &mut b.shock_peak),
    (0xd0, |b| &mut b.shock),
    (0x140, |b| &mut b.inv_inertia),
    (0x150, |b| &mut b.ang_damping),
    (0x170, |b| &mut b.last_quat),
    (0x190, |b| &mut b.quat),
    (0x1a0, |b| &mut b.cofg),
    (0x1b0, |b| &mut b.moment),
    (0x1c0, |b| &mut b.last_moment),
    (0x1d0, |b| &mut b.rates),
    (0x1e0, |b| &mut b.ang_accel),
    (0x1f0, |b| &mut b.twist_peak),
    (0x200, |b| &mut b.twist),
];

/// The helicopter's body off RAM at `at`, after asserting the words the port holds as constants
/// (every axis free, the biplane's limits, no acceleration limits).
fn read_body(ee: &Ee, at: u32) -> Body {
    let max = f32::MAX.to_bits();
    let fixed = [(0x4, 1), (0x8, 1), (0xc, 1), (0x120, 1), (0x124, 1), (0x128, 1), (0xe4, max), (0x220, max)]
        .into_iter()
        .chain([0xe0].map(|o| (o, game_sim::biplane::MXVATV_MAX_SPEED.to_bits())))
        .chain([0x210, 0x214, 0x218].map(|o| (o, game_sim::biplane::MXVATV_MAX_RATE.to_bits())));
    for (o, w) in fixed {
        assert_eq!(ee.u32_at(at + o), w, "body +{o:#x}: the port holds {w:#x}");
    }
    let mut b = Body { enabled: ee.u32_at(at) != 0, inv_mass: F(ee.f32_at(at + 0x1c)), ..Body::default() };
    for (o, field) in BODY_QUADS {
        *field(&mut b) = quad(ee, at + o);
    }
    b
}

/// The body's words where the port's and the game's differ, by offset.
fn body_diff(got: &Body, want: &Body) -> Vec<String> {
    let (mut got, mut want) = (*got, *want);
    let mut out = Vec::new();
    if (got.enabled, got.inv_mass.to_bits()) != (want.enabled, want.inv_mass.to_bits()) {
        out.push(format!("enabled/inv_mass {:?} want {:?}", (got.enabled, got.inv_mass), (want.enabled, want.inv_mass)));
    }
    for (o, field) in BODY_QUADS {
        let (g, w) = (*field(&mut got), *field(&mut want));
        for k in 0..4 {
            if g.0[k].to_bits() != w.0[k].to_bits() {
                out.push(format!("body+{:#x}: {:#010x} ({}) want {:#010x} ({})", o + 4 * k as u32, g.0[k].to_bits(), g.0[k], w.0[k].to_bits(), w.0[k]));
            }
        }
    }
    out
}

/// The image's own curve table (`atv_curve_table_ptr`, `# MD80c`).
fn the_curve_table(ee: &Ee) -> CurveTable {
    let base = ee.u32_at(ps2_oracle::addr("atv_curve_table_ptr").unwrap());
    let entries: Vec<[f32; 2]> = (0..CURVE_ENTRIES as u32).map(|k| [ee.f32_at(base + 8 * k), ee.f32_at(base + 8 * k + 4)]).collect();
    CurveTable::from_entries(&entries)
}

/// Whether the forces' gate is shut: the vehicle's player record missing, or its flags' bit 0 clear.
fn unmanned(ee: &Ee, heli: u32) -> bool {
    let player = ee.u32_at(heli + at::PLAYER);
    player == 0 || ee.u32_at(player + 0x50) & 1 == 0
}

/// **A [`Kiowa`], its [`Ride`] and [`Mxvatv`] off the game's RAM**: the controller's keys, loop and
/// commands, the rotor's and the aero model's keys and state, the pedal fade, the pad, the body, the
/// measure, the crash words, the flown flag, the gravity record's scales and vector, the forces' gate.
fn seeded(ee: &Ee, heli: u32) -> (Kiowa, Ride, Mxvatv) {
    let g = |a: u32| F(ee.f32_at(a));
    let h = |o: u32| g(heli + o);
    let k = CONTROLLER_KEYS.map(h);
    let v = |o: u32| [h(o), h(o + 4), h(o + 8)];
    let controller = kiowa::Controller {
        lateral_cyclic_gain: k[0],
        longitudinal_cyclic_gain: k[1],
        pedal_gain: k[2],
        max_altitude: k[3],
        hold_delay: k[4],
        collective_gain: k[5],
        climb_rate: k[6],
        descent_rate: k[7],
        altitude_hold_gain: k[8],
        integrator_gain: k[9],
        min_collective: k[10],
        max_pitch: k[11],
        max_roll: k[12],
        pitch_gain: k[13],
        roll_gain: k[14],
        speed_command_rate: k[15],
        min_speed_command: k[16],
        max_speed_command: k[17],
        speed_gain: k[18],
        min_feed_forward_speed: k[19],
        min_feed_forward_scale: k[20],
        max_feed_forward_speed: k[21],
        max_feed_forward_scale: k[22],
        min_rate: v(MIN_RATE),
        max_rate: v(MAX_RATE),
    };
    let filter = ee.u32_at(heli + seed_at::FILTER);
    let control = kiowa::Loop {
        speed_command: h(seed_at::SPEED_COMMAND),
        last_longitudinal: h(seed_at::LAST_STICK),
        hold_altitude: h(seed_at::HOLD),
        hold_timer: h(seed_at::HOLD_TIMER),
        integral: h(seed_at::INTEGRAL),
        filter: kiowa::Filter { value: g(filter), time: g(filter + 4), step: g(filter + 8) },
    };
    let o = |k: u32| h(seed_at::OUT + 4 * k);
    let commands = kiowa::Commands {
        collective: o(0),
        lateral: o(1),
        longitudinal: o(2),
        pedal: o(3),
        filtered: o(4),
        altitude_fraction: h(seed_at::FRACTION),
    };
    let r = ee.u32_at(heli + at::ROTOR);
    let rw = |o: u32| g(r + o);
    let rotor = kiowa::Rotor {
        max_lateral_cyclic: rw(0x3c),
        max_longitudinal_cyclic: rw(0x40),
        max_collective: rw(0x44),
        max_tail_pitch: rw(0x48),
        lift_curve_slope: rw(0x4c),
        blade_chord: rw(0x50),
        blades: rw(0x54),
        radius: rw(0x58),
        radius_squared: rw(0x5c),
        angular_rate: rw(0x60),
        max_moment: [rw(0xb0), rw(0xb4), rw(0xb8)],
    };
    let rotor_state = kiowa::RotorState {
        collective_pitch: rw(0xc0),
        tail_pitch: rw(0xc4),
        tilt: [rw(0xd0), rw(0xd4), rw(0xd8)],
        thrust: quad(ee, r + 0xf0),
        force: quad(ee, r + 0x100),
    };
    let a = ee.u32_at(heli + at::AERO);
    let ak = AERO_KEYS.map(|o| g(a + o));
    let aero = kiowa::Aero {
        density: ak[0],
        chord: ak[1],
        span: ak[2],
        area: ak[3],
        cl_alpha: ak[4],
        cd0: ak[5],
        cy_beta: ak[6],
        cl_beta: ak[7],
        cl_p: ak[8],
        cl_da: ak[9],
        cm0: ak[10],
        cm_alpha: ak[11],
        cm_q: ak[12],
        cm_de: ak[13],
        cn_beta: ak[14],
        cn_r: ak[15],
        cn_dr: ak[16],
        low_speed_drag: ak[17],
        low_speed_aero: ak[18],
        high_speed_aero: ak[19],
    };
    let ao = |k: u32| g(a + AERO_OUT + 4 * k);
    let aero_state = kiowa::AeroState { fade: ao(0), lift: ao(1), drag: ao(2), side: ao(3), roll: ao(4), pitch: ao(5), yaw: ao(6) };
    let ap = |k: u32| g(a + AERO_PEDAL + 4 * k);
    let pedal = kiowa::Pedal { cy_beta: ap(0), cn_beta: ap(1), cn_r: ap(2), timer: ap(3), time: ap(4) };
    let reason = ee.u32_at(heli + at::REASON);
    let k = Kiowa {
        controller,
        rotor,
        aero,
        pad: pad_of(ee, heli),
        commands,
        control,
        rotor_state,
        aero_state,
        wrecked: reason != 0,
    };
    let m = |o: u32| quad(ee, heli + o);
    let mut measure = kiowa::Measure {
        speed: h(at::SPEED),
        alpha: h(seed_at::ALPHA),
        beta: h(seed_at::BETA),
        world_rates: m(seed_at::WORLD_RATES),
        velocity: m(seed_at::LOCAL),
        forward: m(seed_at::FORWARD),
        up: m(seed_at::UP),
        euler: m(seed_at::EULER),
        ..kiowa::Measure::default()
    };
    measure.matrix = game_sim::math::Mat4(std::array::from_fn(|r| m(seed_at::MATRIX + 16 * r as u32)));
    measure.was_touching = ee.u32_at(heli + at::WAS_TOUCHING) != 0;
    let crash = kiowa::Crash {
        reason,
        down: ee.u32_at(heli + at::DOWN) != 0,
        min_wreck_speed: h(seed_at::MIN_WRECK_SPEED),
        wreck_time: h(seed_at::WRECK_TIME),
        clock: h(at::CLOCK),
    };
    let gravity = ee.u32_at(heli + at::GRAVITY);
    let ride = Ride {
        body: read_body(ee, ee.u32_at(heli + at::BODY)),
        measure,
        crash,
        flown: ee.u32_at(heli + seed_at::FLOWN) != 0,
        gravity_scale: g(gravity),
        throw_scale: g(gravity + 4),
        gravity: quad(ee, gravity + 0x40),
        area: None,
        touchdown_sound: false,
        rotor_angle: F::ZERO,
        rotor_hidden: false,
    };
    (k, ride, Mxvatv { pedal, unmanned: unmanned(ee, heli) })
}

/// The open air: no contact to resolve, the record never touching, no surface, no entries, the rotor
/// striking nothing; a wreck asked for is noted (the stretch has left what the port flies).
#[derive(Default)]
struct Air {
    wrecked: bool,
}

impl kiowa::Surroundings for Air {
    fn resolve(&mut self, _: &mut Body) {}
    fn touching(&self) -> bool {
        false
    }
    fn ignored(&self) -> bool {
        false
    }
    fn entries(&self) -> Vec<kiowa::Touch> {
        Vec::new()
    }
    fn strike(&mut self, _: &game_sim::math::Mat4) -> kiowa::Strike {
        kiowa::Strike::default()
    }
    fn crashed(&mut self, asked: &kiowa::Crashed, _: &mut Ride, _: &mut game_sim::rng::Rng) {
        self.wrecked |= asked.wreck.is_some();
    }
}

/// A stretch flown: its first frame, whether the game's record was touching there, how many frames
/// the port's body agreed through, and the first frame's differences where it parted.
struct Stretch {
    from: usize,
    touching: bool,
    agreed: usize,
    parted: Vec<String>,
    /// The first frame (from the stretch's start) on which a word the port keeps outside the body
    /// differed while the body still agreed, and those words.
    drift: Option<(usize, Vec<String>)>,
}

/// The controller's commands and loop, the rotor's and the aero model's state and the measure where
/// the port's and the game's differ.
fn others_diff(k: &Kiowa, ride: &Ride, ee: &Ee, heli: u32) -> Vec<String> {
    let (r, a) = (ee.u32_at(heli + at::ROTOR), ee.u32_at(heli + at::AERO));
    let c = &k.commands;
    let s = &ride.measure;
    let mut words = vec![
        ("collective", c.collective, heli + seed_at::OUT),
        ("lateral", c.lateral, heli + seed_at::OUT + 4),
        ("longitudinal", c.longitudinal, heli + seed_at::OUT + 8),
        ("pedal", c.pedal, heli + seed_at::OUT + 12),
        ("filtered", c.filtered, heli + seed_at::OUT + 16),
        ("altitude fraction", c.altitude_fraction, heli + seed_at::FRACTION),
        ("speed command", k.control.speed_command, heli + seed_at::SPEED_COMMAND),
        ("integral", k.control.integral, heli + seed_at::INTEGRAL),
        ("hold", k.control.hold_altitude, heli + seed_at::HOLD),
        ("collective pitch", k.rotor_state.collective_pitch, r + 0xc0),
        ("tail pitch", k.rotor_state.tail_pitch, r + 0xc4),
        ("fade", k.aero_state.fade, a + AERO_OUT),
        ("speed", s.speed, heli + at::SPEED),
        ("alpha", s.alpha, heli + seed_at::ALPHA),
        ("beta", s.beta, heli + seed_at::BETA),
    ];
    let mut lanes = Vec::new();
    for (name, v, o) in [
        ("local", s.velocity, seed_at::LOCAL),
        ("world rates", s.world_rates, seed_at::WORLD_RATES),
        ("euler", s.euler, seed_at::EULER),
        ("matrix 0", s.matrix.0[0], seed_at::MATRIX),
        ("matrix 1", s.matrix.0[1], seed_at::MATRIX + 0x10),
        ("matrix 2", s.matrix.0[2], seed_at::MATRIX + 0x20),
        ("matrix 3", s.matrix.0[3], seed_at::MATRIX + 0x30),
    ] {
        for (l, name) in ["x", "y", "z"].iter().map(|l| format!("{name}.{l}")).enumerate() {
            lanes.push((name, F(v.0[l]), heli + o + 4 * l as u32));
        }
    }
    words.extend(lanes.iter().map(|(n, v, a)| (n.as_str(), *v, *a)));
    words
        .into_iter()
        .filter(|&(_, v, at)| v.to_bits() != ee.u32_at(at))
        .map(|(name, v, at)| format!("{name}: {:#010x} ({}) want {:#010x} ({})", v.to_bits(), v.0, ee.u32_at(at), ee.f32_at(at)))
        .collect()
}

/// The interpreter replays the flight from its image as [`interpret`] does (fed off the tape); at
/// every [`STRETCH`]th frame a port helicopter is seeded off its RAM ([`seeded`]) and flown on
/// beside it through [`Kiowa::tick_mxvatv`] in the open air ([`Air`]) on the same pad words, its body
/// compared with the game's after each frame.
fn port_flies(f: &Flight) -> Option<Vec<Stretch>> {
    let (mut ee, ticks) = (hooked(image(f.name)?), tape(f.name)?);
    let frames = by_frame(&ee, f.heli, &rows(f.name)?).iter().map(|&(j, _)| j).max()? + 1;
    let tables = game_sim::ride::Tables::build();
    let curve = the_curve_table(&ee);
    let s = sim(&ee);
    let body_at = ee.u32_at(f.heli + at::BODY);
    let record = ee.u32_at(f.heli + at::RECORD);
    let dt = F(f32::from_bits(DT));
    let world = World { iterations: 1, rate: F(115.0) };
    let mut out: Vec<Stretch> = Vec::new();
    let mut port: Option<(Kiowa, Ride, Mxvatv)> = None;
    let mut rng = game_sim::rng::Rng::from_seed(1);
    let mut j = 0;
    while j < frames {
        if j % STRETCH == 0 {
            port = Some(seeded(&ee, f.heli));
            out.push(Stretch { from: j, touching: ee.u32_at(record + 0x34) != 0, agreed: 0, parted: Vec::new(), drift: None });
        }
        step(&mut ee, s, f.heli, &ticks, &mut j);
        let pad = pad_of(&ee, f.heli);
        if let Some((k, ride, m)) = port.as_mut() {
            let mut air = Air::default();
            k.tick_mxvatv(m, ride, &world, Some(pad), &curve, &tables.acos, &mut rng, dt, &mut air);
            let d = body_diff(&ride.body, &read_body(&ee, body_at));
            let last = out.last_mut().unwrap();
            if d.is_empty() {
                last.agreed += 1;
                if last.drift.is_none() {
                    let o = others_diff(k, ride, &ee, f.heli);
                    if !o.is_empty() {
                        last.drift = Some((last.agreed, o));
                    }
                }
            } else {
                last.parted = d;
                let b = read_body(&ee, body_at);
                last.parted.push(format!(
                    "(the game's body at {:?}, the record touching {}, the reason {}, the port's wreck {})",
                    b.position,
                    ee.u32_at(record + 0x34),
                    ee.u32_at(f.heli + at::REASON),
                    air.wrecked
                ));
                last.parted.extend(others_diff(k, ride, &ee, f.heli));
                port = None;
            }
        }
    }
    Some(out)
}

fn port_flown(f: &Flight, pinned: &[usize]) {
    let Some(stretches) = port_flies(f) else { return };
    for s in &stretches {
        println!(
            "{} from frame {} ({}): {} frames agree{}",
            f.name,
            s.from,
            if s.touching { "touching" } else { "flying" },
            s.agreed,
            s.parted.first().map(|d| format!(", then {d}")).unwrap_or_default()
        );
        if let Some((k, d)) = &s.drift {
            println!("  frame {k}: {}", d.join("; "));
        }
    }
    let got: Vec<usize> = stretches.iter().map(|s| s.agreed).collect();
    for (k, (&g, &p)) in got.iter().zip(pinned).enumerate() {
        assert!(g >= p, "{} stretch {k} (from frame {}): {g} frames agree, pinned {p}\n  {}", f.name, stretches[k].from, stretches[k].parted.join("\n  "));
    }
    assert_eq!(got.len(), pinned.len(), "{}: stretches {got:?}, pinned {pinned:?}: re-pin", f.name);
    assert_eq!(got, pinned, "{}: the port flies further than pinned: raise the ratchet", f.name);
}

/// **The port flies kiowa**: [`Kiowa::tick_mxvatv`] off the game's RAM at each stretch, as far as
/// [`KIOWA_AGREES_THROUGH`].
#[test]
fn the_port_flies_kiowa() {
    port_flown(&KIOWA, KIOWA_AGREES_THROUGH);
}

/// **And kiowa3**, as far as [`KIOWA3_AGREES_THROUGH`].
#[test]
fn the_port_flies_kiowa3() {
    port_flown(&KIOWA3, KIOWA3_AGREES_THROUGH);
}
