//! MD101 — MD80a's SLUS-21104 biplane flights, replayed.
//!
//! The captures (`context/capture/md80a/{biplane,biplane3}.csv`, re-recorded by MD101 with
//! `research/md80a/fly.sh` from MD80a's Desert Oasis states) hold, every 30 Hz tick, the plane object,
//! its body, engine, aero model and three tires. The images (`context/mxvatv-ps2/md80a/NAME.ee.bin`,
//! `research/md80a/ram.py NAME`) are each flight's start.
//!
//! **The interpreter** runs SLUS-21104's per-frame sim update off the image, as `tests/mxvatv_bike_ride.rs`
//! does for the bike (the same frame, hooks and dt), with the plane's input step's stores fed from each
//! row: the throttle-down and -up commands (+0x310, +0x314) and the three stick commands (+0x10f4..).
//! The input step runs outside the frame, from the game's input pass. Every other word is the
//! interpreter's, and each row's body, engine, aero and tires must come out word for word.
//!
//! Skips without the images or the captures.

use game_sim::input::{PlaneCommand, PlaneInput};
use ps2_float::F;
use ps2_oracle::{Ee, Kit, skip};
use std::collections::HashMap;

/// The sim frame (the sim object's vt +0x40), the game's tick and `$gp` (research/oracle.txt `# F31`).
const FRAME: u32 = 0x0020_4040;
const DT: u32 = 0x3d08_850a;
const GP: u64 = 0x003d_baf0;
/// The sim object's vtable (+4), and the biplane's.
const SIM_VTABLE: u32 = 0x003b_9320;
const BIPLANE_VTABLE: u32 = 0x003c_e7b0;
const BUDGET: u64 = 400_000_000;


/// A flight: its name, the player's plane in its image, its row count.
struct Flight {
    name: &'static str,
    plane: u32,
}

const BIPLANE: Flight = Flight { name: "biplane", plane: 0x014d_e810 };
const BIPLANE3: Flight = Flight { name: "biplane3", plane: 0x01b1_3790 };

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

fn rows(name: &str) -> Option<Vec<Row>> {
    let path = Kit::load().ok()?.ctx("capture/md80a").join(format!("{name}{}", ".csv"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        skip(&format!("no {} (research/md80a/fly.sh)", path.display()));
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

/// The physics blocks, as (name, address in the image, bytes).
fn physics(ee: &Ee, plane: u32) -> Vec<(&'static str, u32, u32)> {
    let p = |o: u32| ee.u32_at(plane + o);
    vec![
        ("body", p(0xb8), 640),
        ("engine", p(0x1164), 512),
        ("aero", p(0x1168), 512),
        ("tire0.raw", p(0x116c), 512),
        ("tire1.raw", p(0x1170), 512),
        ("tire2.raw", p(0x1174), 512),
    ]
}

fn diff(ee: &Ee, plane: u32, row: &Row) -> Vec<String> {
    let mut out = Vec::new();
    for (name, at, len) in physics(ee, plane) {
        let want = hex_words(&row[name]);
        for i in 0..len / 4 {
            let got = ee.u32_at(at + 4 * i);
            let w = want[i as usize];
            if got != w {
                out.push(format!(
                    "{name}+{:#x}: {got:#010x} ({}) want {w:#010x} ({})",
                    4 * i,
                    f32::from_bits(got),
                    f32::from_bits(w)
                ));
            }
        }
    }
    out
}

/// F31's hooks: the kernel's DIntr/EIntr, the vehicle's network snapshot send, every syscall stub.
fn hooked(mut ee: Ee) -> Ee {
    ee.hooks.insert(0x0034_8fa0, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    ee.hooks.insert(0x0034_8ff0, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    ee.hooks.insert(0x004e_7e70, Box::new(|_: &mut Ee| {}));
    // The kernel's thread delay (it reads Status, then waits on an alarm): 0, no wait.
    ee.hooks.insert(0x0033_ff58, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    // The sound driver's call to the IOP: it waits on flags only the IOP's reply sets.
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

/// A flight's tape (`research/md80a/fly.sh` writes it beside the `.p2m2`): per tick, the buttons held
/// and the left stick.
fn tape(name: &str) -> Option<Vec<Row>> {
    let path = Kit::load().ok()?.ctx("recordings/md80a").join(format!("{name}.p2m2.ticks{}", ".csv"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        skip(&format!("no {} (research/md80a/fly.sh)", path.display()));
        return None;
    };
    let mut lines = text.lines();
    let head: Vec<String> = lines.next()?.split(',').map(String::from).collect();
    Some(lines.map(|l| head.iter().cloned().zip(l.split(',').map(String::from)).collect()).collect())
}

/// The button each command is on in MX vs. ATV's shipped bindings, as the tape names it.
fn button(c: PlaneCommand) -> &'static str {
    match c {
        PlaneCommand::ThrottleUp => "cross",
        PlaneCommand::ThrottleDown => "square",
        PlaneCommand::RudderLeft => "l1",
        PlaneCommand::RudderRight => "r1",
        PlaneCommand::BankLeft => "left",
        PlaneCommand::BankRight => "right",
        PlaneCommand::Dive => "up",
        PlaneCommand::Climb => "down",
    }
}

/// The input step on a tape tick ([`PlaneInput::from_sticks`], proved on SLUS-21104's own by
/// [`the_input_step_is_mx_unleasheds`]): a held button is a strength of one, the stick is the tape's.
fn input_of(tick: &Row) -> PlaneInput {
    let held: Vec<&str> = tick["buttons"].split(' ').collect();
    let stick = |k: &str| F(tick[k].parse::<f32>().unwrap());
    PlaneInput::from_sticks((stick("lx"), stick("ly")), |c| if held.contains(&button(c)) { F::ONE } else { F::ZERO })
}

/// The input step's words for frame `j` (the `j`th from the image): tape tick `j - 2`'s, or the
/// image's own for the first two.
fn feed(ee: &mut Ee, plane: u32, ticks: &[Row], j: usize) {
    let Some(t) = j.checked_sub(2).and_then(|t| ticks.get(t)) else { return };
    let i = input_of(t);
    for (o, v) in [(0x310, i.throttle_down), (0x314, i.throttle_up), (0x10f4, i.elevator), (0x10f8, i.aileron), (0x10fc, i.rudder)] {
        ee.set_u32(plane + o, v.to_bits());
    }
}

/// A flight's rows by the frame each shows (`j`, the frames stepped from the image). The recorder
/// reads at one phase a run: either each row's vsync is its frame's (`j = (vsync - image's) / 2`), or
/// every row is one frame behind it (biplane's: its first row is the image itself).
fn by_frame(ee: &Ee, plane: u32, rows: &[Row]) -> Vec<(usize, usize)> {
    let v0 = ee.u32_at(VSYNC) as usize;
    let behind = usize::from(diff(ee, plane, &rows[0]).is_empty());
    rows.iter()
        .enumerate()
        .map(|(r, row)| ((row["vsync"].parse::<usize>().unwrap() - v0) / 2 - behind, r))
        .collect()
}

/// What a replay came to: the rows that agreed, the stale ones (a row that shows the frame before
/// its own, a read that tore), the late ones (a row that shows the frame after its own: the
/// recorder read the vsync a frame before the blocks), and the first row that was none of these,
/// with its differences against its own frame.
struct Verdict {
    rows: usize,
    agreed: usize,
    stale: Vec<usize>,
    late: Vec<usize>,
    parted: Option<(usize, Vec<String>)>,
}

/// Replays a flight in the interpreter from its image, frame by frame, with the input step's words
/// off the tape ([`feed`]); every row present is compared with the frame it shows.
fn interpret(f: &Flight) -> Option<Verdict> {
    let mut ee = hooked(image(f.name)?);
    let (rows, ticks) = (rows(f.name)?, tape(f.name)?);
    assert_eq!(ee.u32_at(f.plane + 4), BIPLANE_VTABLE, "{}: no biplane at {:#x}", f.name, f.plane);
    let s = sim(&ee);
    let frames = by_frame(&ee, f.plane, &rows);
    let words = |ee: &Ee| -> Vec<u32> {
        physics(ee, f.plane).into_iter().flat_map(|(_, at, len)| (0..len / 4).map(move |i| at + 4 * i)).map(|a| ee.u32_at(a)).collect()
    };
    let names: Vec<&str> = physics(&ee, f.plane).iter().map(|b| b.0).collect();
    let row_words = |row: &Row| -> Vec<u32> { names.iter().flat_map(|n| hex_words(&row[*n])).collect() };
    let mut v = Verdict { rows: rows.len(), agreed: 0, stale: Vec::new(), late: Vec::new(), parted: None };
    let (mut before, mut j) = (words(&ee), 0);
    let step = |ee: &mut Ee, j: &mut usize| {
        *j += 1;
        feed(ee, f.plane, &ticks, *j);
        ee.set(28, GP);
        ee.call(FRAME, &[u64::from(s)], &[f32::from_bits(DT)], BUDGET);
    };
    for (k, &(at, r)) in frames.iter().enumerate() {
        while j < at {
            before = words(&ee);
            step(&mut ee, &mut j);
        }
        let want = row_words(&rows[r]);
        if j > at {
            // The row before was read late, onto this row's own frame.
            if want == before {
                v.agreed += 1;
                continue;
            }
        }
        if want == words(&ee) {
            v.agreed += 1;
        } else if want == before {
            v.stale.push(r);
        } else if frames.get(k + 1).is_none_or(|&(next, _)| next > at) && {
            let d = diff(&ee, f.plane, &rows[r]);
            before = words(&ee);
            step(&mut ee, &mut j);
            if want == words(&ee) {
                true
            } else {
                v.parted = Some((r, d));
                false
            }
        } {
            v.late.push(r);
        } else {
            v.parted.get_or_insert_with(|| (r, diff(&ee, f.plane, &rows[r])));
            break;
        }
    }
    Some(v)
}

fn interpreted(f: &Flight) {
    let Some(v) = interpret(f) else { return };
    println!("{}: {} rows agree, stale rows {:?}, late rows {:?}", f.name, v.agreed, v.stale, v.late);
    if let Some((r, d)) = &v.parted {
        panic!(
            "{} row {r} of {}: {} words differ\n  {}",
            f.name,
            v.rows,
            d.len(),
            d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  ")
        );
    }
    // A late row still equals the frame after its own bit for bit, and the rows after it stay on
    // their frames; biplane's take (recorded at a load of ~90) has 118 of 1058, biplane3's none.
    assert!(v.stale.len() <= 2, "{}: {} stale rows: re-record it", f.name, v.stale.len());
    assert!(v.late.len() <= v.rows / 5, "{}: {} late rows of {}: re-record it", f.name, v.late.len(), v.rows);
}

#[test]
fn the_interpreter_flies_biplane_bit_exact() {
    interpreted(&BIPLANE);
}

#[test]
fn the_interpreter_flies_biplane3_bit_exact() {
    interpreted(&BIPLANE3);
}

// --- the input step ---------------------------------------------------------------------------------


/// SLUS-21104's biplane input step (`atv_biplane_input`, a0 the plane, a1 the player's record: the
/// player, the control records, stick x, stick y), the gate it asks first (the player record's "is
/// this pad live"), the binding lookup and the front end it is handed.
const INPUT_STEP: u32 = 0x004c_2d50;
const LIVE: u32 = 0x0024_9330;
const BINDING: u32 = 0x0011_8e10;
const FRONT_END: u32 = 0x003e_ab60;
/// The plane's word the step is skipped without.
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

/// **SLUS-21104's input step is [`PlaneInput::from_sticks`]** with the commands numbered as
/// [`PlaneCommand::mxvatv`]: ten thousand cases on the biplane image, each a player (0 or 1), sticks
/// with signed zeros, the ends and tiny values, and a strength in every control record the binding
/// lookup can name. The image's own binding table answers the lookup; the five words the step writes
/// must equal the port's bit for bit.
#[test]
fn the_input_step_is_mx_unleasheds() {
    let Some(mut ee) = image("biplane") else { return };
    let plane = BIPLANE.plane;
    ee.hooks.insert(LIVE, Box::new(|ee: &mut Ee| ee.set(2, 1)));
    ee.set_u32(plane + LOCAL, 1);
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
        let base = controls + 0x18;
        for (k, v) in [player, base, x.to_bits(), y.to_bits()].into_iter().enumerate() {
            ee.set_u32(record + 4 * k as u32, v);
        }
        for o in [0x310, 0x314, 0x10f4, 0x10f8, 0x10fc] {
            ee.set_u32(plane + o, 0x7fc0_0001);
        }
        let mut slot = |c: PlaneCommand| {
            ee.set(28, GP);
            ee.call(BINDING, &[u64::from(front), u64::from(c.mxvatv()), 2, 10, u64::from(player)], &[], BUDGET);
            ee.r(2) as u32 as i32
        };
        let slots: Vec<(PlaneCommand, i32)> = PlaneCommand::ALL.iter().map(|&c| (c, slot(c))).collect();
        let value = |c: PlaneCommand| {
            let k = slots.iter().find(|s| s.0 == c).unwrap().1;
            assert!((-1..33).contains(&k), "case {case}: {c:?} looks up control {k}");
            F(strengths[(k + 1) as usize])
        };
        let want = PlaneInput::from_sticks((F(x), F(y)), value);
        ee.set(28, GP);
        ee.call(INPUT_STEP, &[u64::from(plane), u64::from(record)], &[], BUDGET);
        for (name, o, w) in [
            ("throttle down", 0x310, want.throttle_down),
            ("throttle up", 0x314, want.throttle_up),
            ("elevator", 0x10f4, want.elevator),
            ("aileron", 0x10f8, want.aileron),
            ("rudder", 0x10fc, want.rudder),
        ] {
            let got = ee.u32_at(plane + o);
            assert_eq!(got, w.to_bits(), "case {case} ({x:e}, {y:e}, player {player}): {name} {got:#x} in the game, {:#x} in the port", w.to_bits());
        }
    }
}
