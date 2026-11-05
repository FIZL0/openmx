//! MD101 — MD80a's SLUS-21104 biplane flights, replayed; MD101a — and flown by the port.
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
//! **The port** (MD101a) flies them two ways:
//!
//! - its body step ([`Body::integrate_mxvatv`], [`Body::end_substep_mxvatv`]) in the interpreter's
//!   frame, in place of SLUS-21104's own on the plane's body: every row must still agree;
//! - a whole [`flight::Flight::new_mxvatv`] seeded off the interpreter's RAM every [`STRETCH`] frames
//!   and flown beside it on the same input words, its body word for word against the game's after
//!   each frame, as far as [`BIPLANE_AGREES_THROUGH`] and [`BIPLANE3_AGREES_THROUGH`] pin. Since
//!   MD101b it stands on Desert Oasis's ground ([`ground::Under`] over MC02's terrain, the object the
//!   tires' `+0x134` points at), and its tires are compared too, word for word.
//!
//! Skips without the images or the captures.

use game_sim::biplane::{self, Body};
use game_sim::flight;
use game_sim::ride;
use game_sim::handling::ground;
use game_sim::track;
use game_sim::input::{PlaneCommand, PlaneInput};
use ps2_float::{F, Vf};
use std::cell::Cell;
use std::rc::Rc;
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
fn interpret(f: &Flight, body: Stepper) -> Option<Verdict> {
    let mut ee = hooked(image(f.name)?);
    let body_at = ee.u32_at(f.plane + 0xb8);
    if let Stepper::Port(calls) = &body {
        port_body(&mut ee, body_at, calls.clone());
    }
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

/// Whether it ran (it skips without the images, the captures or the tapes).
fn interpreted(f: &Flight, body: Stepper) -> bool {
    let Some(v) = interpret(f, body) else { return false };
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
    true
}

#[test]
fn the_interpreter_flies_biplane_bit_exact() {
    interpreted(&BIPLANE, Stepper::Game);
}

#[test]
fn the_interpreter_flies_biplane3_bit_exact() {
    interpreted(&BIPLANE3, Stepper::Game);
}

// --- the port's body in the original's frame (MD101a) ------------------------------------------------

/// SLUS-21104's rigid body integrate and end of substep (the body vtable's +0x54 and +0x40, `# MD75`).
const BODY_INTEGRATE: u32 = 0x0049_6240;
const BODY_END_SUBSTEP: u32 = 0x0049_bde0;

/// Who steps the plane's body in an interpreter replay: the game's own code, or the port's
/// ([`Body::integrate_mxvatv`], [`Body::end_substep_mxvatv`]) in its place, counting the calls.
enum Stepper {
    Game,
    Port(Rc<Cell<(usize, usize)>>),
}

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| ee.f32_at(a + k)))
}

fn put_quad(ee: &mut Ee, a: u32, v: Vf) {
    for k in 0..4 {
        ee.set_u32(a + 4 * k as u32, v.0[k].to_bits());
    }
}

/// The plane's body's quadwords in SLUS-21104's layout (`# MD75`) as [`Body`]'s fields: the
/// correction and its peak are the shock's.
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

/// The plane's body off the image at `at`, after asserting the words the port holds as constants
/// (every axis free, the limits [`biplane::MXVATV_MAX_SPEED`] and [`biplane::MXVATV_MAX_RATE`], no
/// acceleration limits).
fn read_body(ee: &Ee, at: u32) -> Body {
    let max = f32::MAX.to_bits();
    let fixed = [(0x4, 1), (0x8, 1), (0xc, 1), (0x120, 1), (0x124, 1), (0x128, 1), (0xe4, max), (0x220, max)]
        .into_iter()
        .chain([0xe0].map(|o| (o, biplane::MXVATV_MAX_SPEED.to_bits())))
        .chain([0x210, 0x214, 0x218].map(|o| (o, biplane::MXVATV_MAX_RATE.to_bits())));
    for (o, w) in fixed {
        assert_eq!(ee.u32_at(at + o), w, "body +{o:#x}: the port holds {w:#x}");
    }
    let mut b = Body { enabled: ee.u32_at(at) != 0, inv_mass: F(ee.f32_at(at + 0x1c)), ..Body::default() };
    for (o, field) in BODY_QUADS {
        *field(&mut b) = quad(ee, at + o);
    }
    b
}

fn put_body(ee: &mut Ee, at: u32, b: &Body) {
    let mut b = *b;
    ee.set_u32(at, u32::from(b.enabled));
    ee.set_u32(at + 0x1c, b.inv_mass.to_bits());
    for (o, field) in BODY_QUADS {
        put_quad(ee, at + o, *field(&mut b));
    }
}

/// The game's body step on the plane's body at `at` replaced by the port's: each call reads the body
/// off RAM, steps it in the port, and writes it back. Any other body through either call fails.
fn port_body(ee: &mut Ee, at: u32, calls: Rc<Cell<(usize, usize)>>) {
    let c = calls.clone();
    ee.hooks.insert(BODY_INTEGRATE, Box::new(move |ee: &mut Ee| {
        assert_eq!(ee.r(4) as u32, at, "a body that is not the plane's is integrated");
        let mut b = read_body(ee, at);
        b.integrate_mxvatv(F(ee.f(12)));
        put_body(ee, at, &b);
        c.set((c.get().0 + 1, c.get().1));
    }));
    ee.hooks.insert(BODY_END_SUBSTEP, Box::new(move |ee: &mut Ee| {
        assert_eq!(ee.r(4) as u32, at, "a body that is not the plane's ends a substep");
        let mut b = read_body(ee, at);
        b.end_substep_mxvatv();
        put_body(ee, at, &b);
        calls.set((calls.get().0, calls.get().1 + 1));
    }));
}

/// A flight replayed with the port's body step in the game's place, which must come out as the
/// game's own replay does: four substeps a frame, every row's body, engine, aero and tires word for
/// word.
fn port_bodied(f: &Flight) {
    let calls = Rc::new(Cell::new((0, 0)));
    if !interpreted(f, Stepper::Port(calls.clone())) {
        return;
    }
    let (integrated, ended) = calls.get();
    println!("{}: the port stepped the body {integrated} times", f.name);
    assert!(integrated > 0, "{}: the game never integrated the plane's body", f.name);
    assert_eq!(integrated, ended, "{}: integrates against ends", f.name);
    assert_eq!(integrated % 4, 0, "{}: {integrated} substeps is not four a frame", f.name);
}

/// **The port's body flies biplane3** (MD101a): SLUS-21104's frame with [`Body::integrate_mxvatv`] and
/// [`Body::end_substep_mxvatv`] stepping the plane's body agrees with all 2338 rows.
#[test]
fn the_port_body_flies_biplane3_bit_exact() {
    port_bodied(&BIPLANE3);
}

/// **And biplane**, with its late rows as the game's own replay has them.
#[test]
fn the_port_body_flies_biplane_bit_exact() {
    port_bodied(&BIPLANE);
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

// --- the port flies them (MD101a) --------------------------------------------------------------------

/// Frames between seeds: the port is seeded off the interpreter's RAM every so many frames of the
/// flight (the first seed is the image itself) and flown on until the next.
const STRETCH: usize = 100;

/// **How far the port flies each stretch** with every body and tire word equal to the game's, frame by
/// frame (a ratchet: raise it as the port gains the original's ground collision and wreck machine;
/// MD101c). One entry per stretch, `STRETCH` the most; the last stretch of each flight ends with its
/// capture (biplane's 30 frames, biplane3's 39). Since MD101b the tires stand on MC02's ground and
/// every stretch agrees to its end but two of biplane3's: at frame 2059 the game's body takes a push
/// and a shock no tire gives (its tires agree on that frame), the hull meeting the terrain, and the
/// stretch from 2100 is seeded in that contact.
const BIPLANE_AGREES_THROUGH: &[usize] = &[100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 30];
const BIPLANE3_AGREES_THROUGH: &[usize] =
    &[100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 59, 0, 100, 39];

/// The tire's terrain object (`+0x134`), the one its probe hands the ground query.
const TIRE_TERRAIN: u32 = 0x134;

/// Desert Oasis's `mc02.res` off MX vs. ATV's disc (MD37a's copy), whose terrain file the tires stand on.
fn mc02_res() -> Option<Vec<u8>> {
    let path = Kit::load().ok()?.ctx("mxvatv-ps2/md37a").join("mc02.res");
    let bytes = std::fs::read(&path).ok();
    if bytes.is_none() {
        skip(&format!("no {} (Desert Oasis's Env/MC02 off MX vs. ATV's disc; memory: md37a ATV states live in s2)", path.display()));
    }
    bytes
}

/// MX vs. ATV's `Veh\biplane.phy` (MD80b's copy).
fn phy_file() -> Option<Vec<u8>> {
    let path = Kit::load().ok()?.ctx("mxvatv-ps2/md80b").join("biplane.phy");
    let bytes = std::fs::read(&path).ok();
    if bytes.is_none() {
        skip(&format!("no {} (the disc's Veh\\biplane.phy)", path.display()));
    }
    bytes
}

/// The plane's words a [`flight::Flight`] holds in its own fields (`# MD80b`), as offsets.
mod at {
    pub const BACK: u32 = 0x310;
    pub const FORWARD: u32 = 0x314;
    pub const THROTTLE: u32 = 0x318;
    pub const GROUNDED: u32 = 0x324;
    pub const WRECK: u32 = 0xc80;
    pub const WRECK_CLOCK: u32 = 0xc84;
    pub const FLOWN: u32 = 0x10f0;
    pub const STICKS: u32 = 0x10f4;
    pub const BRAKE: u32 = 0x1100;
    pub const SURFACES: u32 = 0x1104;
    pub const INTEGRAL: u32 = 0x12e0;
    pub const ENGINE: u32 = 0x1164;
    pub const AERO: u32 = 0x1168;
    pub const MODEL: u32 = 0x1320;
    pub const TIRES: u32 = 0x116c;
}

/// The input step's five words as the plane's frame reads them.
fn pad_of(ee: &Ee, plane: u32) -> biplane::Pad {
    let w = |o: u32| F(ee.f32_at(plane + o));
    biplane::Pad {
        forward: w(at::FORWARD),
        back: w(at::BACK),
        elevator: w(at::STICKS),
        aileron: w(at::STICKS + 4),
        rudder: w(at::STICKS + 8),
    }
}

/// **A [`flight::Flight::new_mxvatv`] off the game's RAM**: built from `biplane.phy` as the plane's model
/// word picks, then the body (MD75's layout, [`read_body`]), the pad, the surfaces, the flown gate,
/// the engine's words, the aero model's true-speed switch and the wreck word and clock, then
/// measured, and the three tires' words off `*(plane + 0x116c + 4k)` ([`read_tire`], MD101b). The
/// game's `rand()` and smooth noise are not SLUS-21104's (MD101c).
fn seeded(ee: &Ee, plane: u32, phy: &[u8], tables: &ride::Tables) -> flight::Flight {
    let p = game_sim::phy::Plane::read_mxvatv_bytes(phy, ee.u32_at(plane + at::MODEL) == 1);
    let mut f = flight::Flight::new_mxvatv(&p);
    let w = |o: u32| F(ee.f32_at(plane + o));
    f.body = read_body(ee, ee.u32_at(plane + 0xb8));
    f.pad = pad_of(ee, plane);
    let s = &mut f.surfaces;
    (s.throttle, s.brake, s.pitch_integral) = (w(at::THROTTLE), w(at::BRAKE), w(at::INTEGRAL));
    (s.elevator, s.elevator_command, s.aileron) = (w(at::SURFACES), w(at::SURFACES + 4), w(at::SURFACES + 8));
    (s.rudder, s.steer) = (w(at::SURFACES + 12), w(at::SURFACES + 16));
    f.attitude.flown = ee.u32_at(plane + at::FLOWN) != 0;
    let e = ee.u32_at(plane + at::ENGINE);
    let ew = |k: u32| F(ee.f32_at(e + 0x30 + 4 * k));
    let en = &mut f.engine;
    (en.min_efficiency, en.full_efficiency_speed, en.prop_rpm, en.prop_angle) = (ew(0), ew(1), ew(2), ew(3));
    (en.max_power, en.fade_altitude, en.max_altitude) = (ew(4), ew(5), ew(6));
    (en.power, en.rpm, en.idle_rpm, en.max_rpm) = (ew(8), ew(9), ew(10), ew(11));
    (en.thrust, en.note, en.clock) = (ew(12), ew(13), ew(14));
    f.aero.true_speed = ee.u32_at(ee.u32_at(plane + at::AERO) + 0x24) != 0;
    f.crash.reason = ee.u32_at(plane + at::WRECK);
    f.crash.clock = w(at::WRECK_CLOCK);
    if f.crash.reason == 0 {
        f.inv_inertia = f.body.inv_inertia;
    }
    for (k, t) in f.tires.iter_mut().enumerate() {
        read_tire(ee, ee.u32_at(plane + at::TIRES + 4 * k as u32), t);
    }
    f.remeasure(&tables.acos);
    f
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

/// A stretch flown: its first frame, how many frames the port's body agreed through, and the
/// first frame's differences where it parted (none if it agreed through to the end).
struct Stretch {
    from: usize,
    agreed: usize,
    grounded: bool,
    parted: Vec<String>,
    /// The first frame (from the stretch's start) on which a word the port keeps outside the body
    /// differed while the body still agreed ([`others_diff`]), and those words: where a parting
    /// starts.
    drift: Option<(usize, Vec<String>)>,
}

/// The surfaces, the engine's words and the aero model's last coefficients where the port's and
/// the game's differ. Not the engine's note: its wobble reads the game's smooth noise, whose tables
/// the port builds off its own `rand()` (it moves no body word).
fn others_diff(p: &flight::Flight, ee: &Ee, plane: u32) -> Vec<String> {
    let (e, a) = (ee.u32_at(plane + at::ENGINE), ee.u32_at(plane + at::AERO));
    let s = &p.surfaces;
    let en = &p.engine;
    let words = [
        ("throttle", s.throttle, plane + at::THROTTLE),
        ("brake", s.brake, plane + at::BRAKE),
        ("elevator", s.elevator, plane + at::SURFACES),
        ("elevator command", s.elevator_command, plane + at::SURFACES + 4),
        ("aileron", s.aileron, plane + at::SURFACES + 8),
        ("rudder", s.rudder, plane + at::SURFACES + 12),
        ("steer", s.steer, plane + at::SURFACES + 16),
        ("pitch integral", s.pitch_integral, plane + at::INTEGRAL),
        ("prop angle", en.prop_angle, e + 0x3c),
        ("power", en.power, e + 0x50),
        ("rpm", en.rpm, e + 0x54),
        ("thrust", en.thrust, e + 0x60),
        ("clock", en.clock, e + 0x68),
        ("lift", p.aero.lift, a + 0xdc),
        ("drag", p.aero.drag, a + 0xe0),
        ("side", p.aero.side, a + 0xe4),
        ("roll", p.aero.roll, a + 0xe8),
        ("pitch", p.aero.pitch, a + 0xec),
    ];
    words
        .into_iter()
        .filter(|&(_, v, at)| v.to_bits() != ee.u32_at(at))
        .map(|(name, v, at)| format!("{name}: {:#010x} ({}) want {:#010x} ({})", v.to_bits(), v.0, ee.u32_at(at), ee.f32_at(at)))
        .collect()
}

/// The tire's quadwords in its object (`# S39`'s layout, which SLUS-21104's tire keeps: its probe
/// reads and writes the same offsets) as [`biplane::Tire`]'s fields.
const TIRE_QUADS: [(u32, fn(&mut biplane::Tire) -> &mut Vf); 15] = [
    (0x50, |t| &mut t.offset),
    (0x60, |t| &mut t.center),
    (0xa0, |t| &mut t.bottom),
    (0xc0, |t| &mut t.ground),
    (0xd0, |t| &mut t.normal),
    (0xe0, |t| &mut t.contact),
    (0xf0, |t| &mut t.hub),
    (0x100, |t| &mut t.low),
    (0x110, |t| &mut t.contact_velocity),
    (0x140, |t| &mut t.last_normal),
    (0x170, |t| &mut t.slip),
    (0x180, |t| &mut t.friction),
    (0x190, |t| &mut t.frame[0]),
    (0x1a0, |t| &mut t.frame[1]),
    (0x1b0, |t| &mut t.frame[2]),
];

/// The tire's single words as [`biplane::Tire`]'s fields: the `.phy`'s and the strut's. The strut's
/// five `.phy` words sit 0xc lower than MX Unleashed's (`+0x30..+0x40`, after the brake and steer
/// pointers at `+0x24`/`+0x28`); the rest are where `# S39` has them.
const TIRE_WORDS: [(u32, fn(&mut biplane::Tire) -> &mut F); 16] = [
    (0x30, |t| &mut t.max_compression),
    (0x34, |t| &mut t.max_decompression_rate),
    (0x38, |t| &mut t.spring),
    (0x3c, |t| &mut t.bounce),
    (0x40, |t| &mut t.rebound),
    (0x8c, |t| &mut t.compression),
    (0x90, |t| &mut t.last_compression),
    (0x94, |t| &mut t.spring_force),
    (0x120, |t| &mut t.angle),
    (0x124, |t| &mut t.spin),
    (0x150, |t| &mut t.radius),
    (0x154, |t| &mut t.inv_radius),
    (0x158, |t| &mut t.max_braking),
    (0x15c, |t| &mut t.side_force),
    (0x160, |t| &mut t.brake_friction),
    (0x164, |t| &mut t.steering_moment),
];

/// The tire's flags as [`biplane::Tire`]'s fields.
const TIRE_FLAGS: [(u32, fn(&mut biplane::Tire) -> &mut bool); 3] =
    [(0x80, |t| &mut t.below), (0x84, |t| &mut t.held), (0x88, |t| &mut t.touching)];

/// The tire at `at` in the game's RAM onto the port's `t`: every state word, the `.phy`'s words
/// and the record byte (the brake and steer switches are the port's own).
fn read_tire(ee: &Ee, at: u32, t: &mut biplane::Tire) {
    for (o, field) in TIRE_QUADS {
        *field(t) = quad(ee, at + o);
    }
    for (o, field) in TIRE_WORDS {
        *field(t) = F(ee.f32_at(at + o));
    }
    for (o, field) in TIRE_FLAGS {
        *field(t) = ee.u32_at(at + o) != 0;
    }
    t.surface = ee.u32_at(at + 0xb0) as u8;
}

/// The words where the port's tire and the game's at `at` differ, by offset.
fn tire_diff(name: &str, t: &biplane::Tire, ee: &Ee, at: u32) -> Vec<String> {
    let mut t = *t;
    let mut out = Vec::new();
    let mut word = |o: u32, got: u32| {
        let want = ee.u32_at(at + o);
        if got != want {
            out.push(format!("{name}+{o:#x}: {got:#010x} ({}) want {want:#010x} ({})", f32::from_bits(got), f32::from_bits(want)));
        }
    };
    for (o, field) in TIRE_QUADS {
        let v = *field(&mut t);
        for k in 0..4 {
            word(o + 4 * k as u32, v.0[k].to_bits());
        }
    }
    for (o, field) in TIRE_WORDS {
        word(o, field(&mut t).to_bits());
    }
    for (o, field) in TIRE_FLAGS {
        word(o, u32::from(*field(&mut t)));
    }
    let got = u32::from(t.surface);
    let want = ee.u32_at(at + 0xb0) & 0xff;
    if got != want {
        out.push(format!("{name}+0xb0 (record byte): {got} want {want}"));
    }
    out
}

/// The three tires' differences, the port's `tires[k]` against `*(plane + 0x116c + 4k)`.
fn tires_diff(p: &flight::Flight, ee: &Ee, plane: u32) -> Vec<String> {
    (0..3).flat_map(|k| tire_diff(&format!("tire{k}"), &p.tires[k], ee, ee.u32_at(plane + at::TIRES + 4 * k as u32))).collect()
}

/// The interpreter replays the flight from its image as [`interpret`] does (fed off the tape); at
/// every [`STRETCH`]th frame a port plane is seeded off its RAM ([`seeded`]) and flown on beside it
/// on the same pad words, its body compared with the game's after each frame.
fn port_flies(f: &Flight) -> Option<Vec<Stretch>> {
    let (phy, mut ee, ticks) = (phy_file()?, hooked(image(f.name)?), tape(f.name)?);
    // Every frame up to the last row's (and the one after, for a late last row).
    let frames = by_frame(&ee, f.plane, &rows(f.name)?).iter().map(|&(j, _)| j).max()? + 1;
    let tables = ride::Tables::build();
    let s = sim(&ee);
    let body_at = ee.u32_at(f.plane + 0xb8);
    let dt = F(f32::from_bits(DT));
    let (res, plane_ee) = (mc02_res()?, &ee);
    let trn = ground::terrain_file(&res, "mc02").expect("mc02.res holds Env\\mc02\\MC02.ps2.trn");
    let map = track::Terrain::parse(&trn).unwrap();
    let mut ground = ground::Under(ground::terrain(&map));
    // Every tire stands on the one terrain object, the one SLUS-21104 builds over the map's file.
    let terrain_at = plane_ee.u32_at(plane_ee.u32_at(f.plane + at::TIRES) + TIRE_TERRAIN);
    for k in 0..3 {
        assert_eq!(plane_ee.u32_at(plane_ee.u32_at(f.plane + at::TIRES + 4 * k) + TIRE_TERRAIN), terrain_at, "{}: tire {k}'s terrain", f.name);
    }
    let read = ground::read_terrain(&|a| plane_ee.u32_at(a), terrain_at, Some(&map));
    assert_eq!(ground::terrain_words(&read, terrain_at), ground::terrain_words(&ground.0, terrain_at), "{}: the terrain object", f.name);
    let mut out: Vec<Stretch> = Vec::new();
    let mut port: Option<flight::Flight> = None;
    for j in 1..=frames {
        if (j - 1) % STRETCH == 0 {
            port = Some(seeded(&ee, f.plane, &phy, &tables));
            out.push(Stretch { from: j - 1, agreed: 0, grounded: ee.u32_at(f.plane + at::GROUNDED) != 0, parted: Vec::new(), drift: None });
        }
        feed(&mut ee, f.plane, &ticks, j);
        let pad = pad_of(&ee, f.plane);
        ee.set(28, GP);
        ee.call(FRAME, &[u64::from(s)], &[f32::from_bits(DT)], BUDGET);
        if let Some(p) = port.as_mut() {
            // The game's input step writes the plane's words before the frame that reads them; the
            // port keeps the pad a frame is handed for the next one, so it is put in first too.
            p.pad = pad;
            p.frame(dt, pad, &mut ground, &tables);
            let mut d = body_diff(&p.body, &read_body(&ee, body_at));
            d.extend(tires_diff(p, &ee, f.plane));
            let last = out.last_mut().unwrap();
            if d.is_empty() {
                last.agreed += 1;
                if last.drift.is_none() {
                    let o = others_diff(p, &ee, f.plane);
                    if !o.is_empty() {
                        last.drift = Some((last.agreed, o));
                    }
                }
            } else {
                last.parted = d;
                let b = read_body(&ee, body_at);
                last.parted.push(format!("(the game's body at {:?}, grounded word {})", b.position, ee.u32_at(f.plane + at::GROUNDED)));
                last.parted.extend(others_diff(p, &ee, f.plane));
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
            if s.grounded { "grounded" } else { "flying" },
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

/// **The port flies biplane** (MD101a): [`flight::Flight::new_mxvatv`] off the game's RAM at each
/// stretch, as far as [`BIPLANE_AGREES_THROUGH`].
#[test]
fn the_port_flies_biplane() {
    port_flown(&BIPLANE, BIPLANE_AGREES_THROUGH);
}

/// **And biplane3**, as far as [`BIPLANE3_AGREES_THROUGH`].
#[test]
fn the_port_flies_biplane3() {
    port_flown(&BIPLANE3, BIPLANE3_AGREES_THROUGH);
}
