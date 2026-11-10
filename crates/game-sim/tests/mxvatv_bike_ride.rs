//! F31 — C15's SLUS-21104 ride and flip, replayed bit-exact by the original's own frame.
//!
//! The captures (`context/capture/c15/ride-2.csv`, `flip-1.csv`, re-recorded by F31 from
//! `context/mxvatv-ps2/state04.ee.bin` with `research/c15/tape.py` + `play.sh`) hold, every 30 Hz
//! tick, the bike object and its body, frame node, collision record, both wheels and the engine. Here
//! the interpreter runs SLUS-21104's per-frame sim update (the sim object's vt +0x40: the list's
//! vehicle frames, the physics world's step, the tail) off state04 at the game's tick (dt bits
//! 0x3d08850a, as MX Unleashed's), and every row's body, node, record, wheels and engine must come
//! out word for word.
//!
//! What is fed from the rows, and nothing else:
//! - the pad reader's outputs ([`READER`]): the reader runs outside this frame, from the game's
//!   input pass. Its words are the frame's inputs, and `ticks_land_one_row_later` ties them to the
//!   tape;
//! - the put-back spot ([`SPOT`]), which the game keeps outside this frame too. Row r's spot goes
//!   into frame r + 1. Without it the flip's put-back lands on state04's spot.
//!
//! Everything else in the bike object is the interpreter's. The words it gets wrong are written
//! outside this frame and read by nothing in it: the run stays bit-exact without them.
//!
//! Hooks: the kernel's DIntr/EIntr; every syscall stub (the heap's semaphores, the thread id); the
//! bike's network snapshot send (vt +0x544), which reads the EE timer and writes nothing of the bike's.
//!
//! The ride: gas, brake, both leans, never wrecks. The flip: the backflip on R1 with the stick, the
//! wreck at row 125 (the collision record swaps to the wreck part), and the put-back at row 193 (back
//! to the riding part, at the saved spot). Skips without the image or the captures.

use ps2_oracle::{Ee, Kit, skip};
use std::collections::HashMap;

const BIKE: u32 = 0x00e7_14e0;
/// The sim object (vtable 0x003b9320) whose vt +0x40 is the frame; the physics world is at +0x6c.
const SIM: u32 = 0x0066_59e0;
const FRAME: u32 = 0x0020_4040;
const DT: u32 = 0x3d08_850a;
const GP: u64 = 0x003d_baf0;

/// The pad reader's stores, bike-relative: brake/gas in, d-pad, stick and holds, clutch and reverse
/// in, +0x14c8, the put-back request, the trick byte, +0x17ac.
const READER: [(u32, u32); 7] =
    [(0x310, 8), (0x12a0, 0x18), (0x12c0, 8), (0x14c8, 4), (0x14d4, 4), (0x14ec, 4), (0x17ac, 4)];
/// The put-back spot and its orientation, bike-relative.
const SPOT: [(u32, u32); 1] = [(0xd38, 0x38)];

type Row = HashMap<String, String>;

fn image() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2").join(format!("state04.ee{}", ".bin"));
    if !path.exists() {
        skip("no context/mxvatv-ps2/state04 RAM image (F31's recipe: research/journal)");
        return None;
    }
    Some(Ee::from_ram(&path).unwrap())
}

fn csv(path: std::path::PathBuf) -> Option<Vec<Row>> {
    let Ok(text) = std::fs::read_to_string(&path) else {
        skip(&format!("no {}", path.display()));
        return None;
    };
    let mut lines = text.lines();
    let head: Vec<String> = lines.next()?.split(',').map(String::from).collect();
    Some(lines.map(|l| head.iter().cloned().zip(l.split(',').map(String::from)).collect()).collect())
}

fn rows(name: &str) -> Option<Vec<Row>> {
    csv(Kit::load().ok()?.ctx("capture/c15").join(format!("{name}{}", ".csv")))
}

fn ticks(tape: &str) -> Option<Vec<Row>> {
    csv(Kit::load().ok()?.ctx("recordings/c15").join(format!("{tape}.p2m2.ticks{}", ".csv")))
}

fn hex_words(s: &str) -> Vec<u32> {
    (0..s.len() / 8)
        .map(|i| {
            let b = |j: usize| u8::from_str_radix(&s[8 * i + 2 * j..8 * i + 2 * j + 2], 16).unwrap();
            u32::from_le_bytes([b(0), b(1), b(2), b(3)])
        })
        .collect()
}

/// The bike object's words in a row (obj0 then obj1).
fn obj(row: &Row) -> Vec<u32> {
    hex_words(&row["obj0"]).into_iter().chain(hex_words(&row["obj1"])).collect()
}

/// The physics blocks, as (name, address in the image, bytes).
fn physics(ee: &Ee) -> Vec<(&'static str, u32, u32)> {
    let p = |o: u32| ee.u32_at(BIKE + o);
    vec![
        ("body", p(0xb8), 640),
        ("node", p(0xe4), 256),
        ("record", p(0xc8), 256),
        ("front", p(0x17b0), 0x920),
        ("rear", p(0x17b4), 0x920),
        ("engine", p(0x142c), 512),
    ]
}

fn diff(ee: &Ee, row: &Row) -> Vec<String> {
    let mut out = Vec::new();
    for (name, at, len) in physics(ee) {
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

fn poke(ee: &mut Ee, row: &Row, words: &[(u32, u32)]) {
    let o = obj(row);
    for &(at, len) in words {
        for i in 0..len / 4 {
            ee.set_u32(BIKE + at + 4 * i, o[(at / 4 + i) as usize]);
        }
    }
}

fn hooked(mut ee: Ee) -> Ee {
    ee.hooks.insert(0x0034_8fa0, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    ee.hooks.insert(0x0034_8ff0, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    // F44a: the stunt packet's read of RCNT0's count (`lui $2, 0x1000` .. `lw $16, 0($2)`) reads the
    // counter's stand-in word instead (0 unless a test sets it), so the packet runs.
    let at = ee.scratch(COUNTER_K);
    assert_eq!([ee.u32_at(COUNTER_LUI), ee.u32_at(COUNTER_LW)], [0x3c02_1000, 0x8c50_0000], "the stunt packet's counter read");
    ee.set_u32(COUNTER_LUI, 0x3c02_0000 | (at.wrapping_add(0x8000) >> 16));
    ee.set_u32(COUNTER_LW, 0x8c50_0000 | (at & 0xffff));
    ee.set_u32(at, 0);
    // Every `addiu $3, $zero, n; syscall; jr $ra`: the answer is a0, the thread id (0x2f) 1.
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

/// Whether the collision record (+0xcc) is the wreck part (+0x1434) rather than the riding one
/// (+0x1438), off a bike-relative word reader.
fn wrecked(o: impl Fn(u32) -> u32) -> bool {
    let at = o(0xcc);
    assert!(at == o(0x1434) || at == o(0x1438), "record {at:#x} is neither part");
    at == o(0x1434)
}

fn row_wrecked(row: &Row) -> bool {
    let o = obj(row);
    wrecked(|at| o[at as usize / 4])
}

/// Replays a capture; answers, row by row, whether the interpreter left the bike wrecked.
fn replay(name: &str) -> Option<Vec<bool>> {
    let mut ee = hooked(image()?);
    let rows = rows(name)?;
    let mut out = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        poke(&mut ee, row, &READER);
        ee.set(28, GP);
        ee.call(FRAME, &[u64::from(SIM)], &[f32::from_bits(DT)], 400_000_000);
        let d = diff(&ee, row);
        assert!(
            d.is_empty(),
            "{name} row {r} (vsync {}): {} words differ\n  {}",
            row["vsync"],
            d.len(),
            d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  ")
        );
        let w = wrecked(|o| ee.u32_at(BIKE + o));
        assert_eq!(w, row_wrecked(row), "{name} row {r}: the wreck state");
        out.push(w);
        poke(&mut ee, row, &SPOT);
    }
    Some(out)
}

#[test]
fn ride_replays_bit_exact() {
    let Some(w) = replay("ride-2") else { return };
    assert_eq!(w.len(), 459);
    assert!(w.iter().all(|w| !w), "the ride never wrecks");
}

#[test]
fn flip_wrecks_and_is_put_back_bit_exact() {
    let Some(w) = replay("flip-1") else { return };
    assert_eq!(w.len(), 465);
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [125, 193], "wrecked on row 125, put back on row 193");
}

/// Whether a row's reader words are a tape tick's pad: cross the gas (held at 0 while wrecked),
/// square the brake, the left stick as it is, R1 the trick bit 4 (not while wrecked). The reader runs
/// before the frame, so "wrecked" is the row before's.
fn reads(before: &Row, row: &Row, tick: &Row) -> bool {
    let o = obj(row);
    let f = |at: u32| f32::from_bits(o[at as usize / 4]);
    let held = |k: &str| tick["buttons"].split(' ').any(|x| x == k);
    let up = !row_wrecked(before);
    let lever = |on: bool| if on { 1.0 } else { 0.0 };
    f(0x314) == lever(held("cross") && up)
        && f(0x310) == lever(held("square"))
        && f(0x12a8) == tick["lx"].parse::<f32>().unwrap()
        && f(0x12ac) == tick["ly"].parse::<f32>().unwrap()
        && (o[0x14ec / 4] & 4 != 0) == (held("r1") && up)
}

/// Tape tick t is read on row t + 1, and not on row t.
#[test]
fn ticks_land_one_row_later() {
    for (name, tape) in [("ride-2", "ride"), ("flip-1", "flip")] {
        let (Some(rows), Some(ticks)) = (rows(name), ticks(tape)) else { return };
        assert_eq!(ticks.len(), rows.len() + 1, "{name}: one tick past the last row");
        let n = rows.len() - 1;
        let bad: Vec<usize> = (0..n).filter(|&t| !reads(&rows[t], &rows[t + 1], &ticks[t])).collect();
        assert!(bad.is_empty(), "{name}: ticks {bad:?} are not on the next row");
        let own = (1..n).filter(|&t| reads(&rows[t - 1], &rows[t], &ticks[t])).count();
        assert!(own < n - 1, "{name}: the tape also fits its own row");
    }
}

// --- F31a: a Handling::MxVsAtv ride seeded from state 4's bike ---------------------------------------

use game_sim::atv_body::Body;
use game_sim::biplane::{Dynamics, List, World};
use game_sim::handling::seed::{self, Seed};
use ps2_float::{F, Vf};

/// The sim frame's pieces (research/oracle.txt `# F31`): the per-frame flag that skips the frame (its
/// object's `+0x225` bit 3), the `+0x24` list's getter, the tail after the world step, and the `+0x3c`
/// list's getter and its remove.
const SKIP_FLAG: u32 = 0x003e_ab60;
const LIST24_GET: u32 = 0x0020_8390;
const TAIL: u32 = 0x0012_8af0;
const LIST3C_GET: u32 = 0x0020_80f0;
const LIST3C_REMOVE: u32 = 0x0020_8040;
/// The physics world (the sim's `+0x6c`).
const WORLD: u32 = SIM + 0x6c;
const BUDGET: u64 = 400_000_000;

/// The seed: every word it stands for is the image's.
#[test]
fn the_seed_is_state4s_bike_word_for_word() {
    let Some(ee) = image() else { return };
    let s = seed::read(&|a| ee.u32_at(a), BIKE);
    assert_eq!(s.at.body, ee.u32_at(BIKE + 0xb8));
    let words = s.words();
    let bad: Vec<String> = words
        .iter()
        .filter(|&&(a, w)| ee.u32_at(a) != w)
        .map(|&(a, w)| format!("{a:#010x}: {w:#010x}, the image {:#010x}", ee.u32_at(a)))
        .collect();
    assert!(bad.is_empty(), "{} of {} words differ:\n  {}", bad.len(), words.len(), bad.join("\n  "));
    // It is the riding bike of C15's rides: on its wheels, in gear, the engine turning.
    assert!(s.body.enabled && s.wheels.iter().all(|w| w.contact && w.has_ground), "{:#?}", s.wheels);
    assert!(s.engine.gear >= 1 && s.engine.rpm > F::ZERO, "{:#?}", s.engine);
}

/// The world's lists, as SLUS-21104's world step reads them (`# MD75`): each `{count, _, array}`.
fn list(ee: &Ee, l: List) -> (u32, u32) {
    let at = WORLD
        + match l {
            List::Bodies => 0x50,
            List::Force => 0x68,
            List::Constraints => 0x80,
            List::Preforce => 0x98,
            List::Post => 0xb0,
        };
    (ee.u32_at(at), ee.u32_at(at + 8))
}

/// The world step over the seeded body: [`World::step`] and [`Body`]'s integrate and end of substep
/// are the port's; the listeners on the world's lists (the bike's PreProcess, Force and PostProcess,
/// and the others the game files there) are still the original's code, run by the interpreter on the
/// body the port holds. The body goes into RAM before each listener and is read back after it.
struct Hybrid<'a> {
    ee: &'a mut Ee,
    body: Body,
    at: u32,
    integrated: u32,
}

impl Hybrid<'_> {
    fn run(&mut self, f: u32, a0: u32, x: F) {
        for (a, w) in seed::body_words(&self.body, self.at) {
            self.ee.set_u32(a, w);
        }
        self.ee.set(28, GP);
        self.ee.call(f, &[u64::from(a0)], &[x.0], BUDGET);
        self.body = seed::body(&|a| self.ee.u32_at(a), self.at);
    }

    fn elt(&self, l: List, i: i32) -> u32 {
        self.ee.u32_at(list(self.ee, l).1 + 4 * i as u32)
    }
}

impl Dynamics for Hybrid<'_> {
    fn count(&self, l: List) -> i32 {
        list(self.ee, l).0 as i32
    }
    fn preforce(&mut self, i: i32, dt: F) {
        let e = self.elt(List::Preforce, i);
        self.run(self.ee.u32_at(self.ee.u32_at(e) + 8), e, dt);
    }
    fn force(&mut self, i: i32, h: F) {
        let e = self.elt(List::Force, i);
        self.run(self.ee.u32_at(self.ee.u32_at(e + 8) + 0xc), e, h);
    }
    fn enabled(&self, b: i32) -> bool {
        assert_eq!(self.elt(List::Bodies, b), self.at, "the world's body is the bike's");
        self.body.enabled
    }
    fn integrate(&mut self, _: i32, h: F) {
        self.body.integrate(h);
        self.integrated += 1;
    }
    fn end_substep(&mut self, _: i32) {
        self.body.end_substep();
    }
    fn constrain(&mut self, _: i32, _: bool) {
        panic!("state 4's world has no constraints");
    }
    fn post(&mut self, i: i32, dt: F) {
        let e = self.elt(List::Post, i);
        self.run(self.ee.u32_at(self.ee.u32_at(e) + 8), e, dt);
    }
}

/// One sim frame (the sim's vt `+0x40`, `0x00204040`) with the world step the port's: the `+0x24`
/// list's vt `+0x7c` on each, then its vt `+0x84(dt)` on each; the world step ([`Hybrid`]); the tail;
/// the `+0x3c` list's vt `+8(dt)` on each, one that answers non-zero taken out. Answers how many
/// times the port integrated the body.
fn hybrid_frame(ee: &mut Ee, s: &Seed, dt: F) -> (Body, u32) {
    let call = |ee: &mut Ee, f: u32, ints: &[u64], x: F| {
        ee.set(28, GP);
        ee.call(f, ints, &[x.0], BUDGET);
        ee.r(2)
    };
    assert_eq!(ee.u8_at(ee.u32_at(SKIP_FLAG) + 0x225) & 8, 0, "the frame is not skipped");
    for slot in [0x7c, 0x84] {
        for i in 0..ee.u32_at(SIM + 0x24) {
            let e = call(ee, LIST24_GET, &[u64::from(SIM + 0x24), u64::from(i)], F::ZERO) as u32;
            call(ee, ee.u32_at(ee.u32_at(e + 4) + slot), &[u64::from(e)], F(f32::from_bits(DT)));
        }
    }
    let world = World { iterations: ee.u32_at(WORLD) as i32, rate: F(ee.f32_at(WORLD + 4)) };
    let body = seed::body(&|a| ee.u32_at(a), s.at.body);
    let mut h = Hybrid { ee, body, at: s.at.body, integrated: 0 };
    assert_eq!(h.body, s.body, "the seed is the body in RAM");
    world.step(dt, &mut h);
    let (body, n) = (h.body, h.integrated);
    for (a, w) in seed::body_words(&body, s.at.body) {
        ee.set_u32(a, w);
    }
    call(ee, TAIL, &[u64::from(SIM)], dt);
    let mut i = 0;
    while i < ee.u32_at(SIM + 0x3c) {
        let e = call(ee, LIST3C_GET, &[u64::from(SIM + 0x3c), u64::from(i)], F::ZERO) as u32;
        if call(ee, ee.u32_at(ee.u32_at(e) + 8), &[u64::from(e)], dt) != 0 {
            call(ee, LIST3C_REMOVE, &[u64::from(SIM + 0x3c), u64::from(i)], F::ZERO);
        } else {
            i += 1;
        }
    }
    (body, n)
}

/// **The seeded bike's first frame is C15 ride-2's row 0**: the body the seed read, stepped by the
/// port's world step and integrate at the game's tick, is row 0's body block word for word, and the
/// node, collision record, wheels and engine the original's listeners leave around it are row 0's too.
/// The listeners are SLUS-21104's own code here (F31b ports them).
#[test]
fn the_seeded_bikes_first_frame_is_ride2_row_0() {
    let (Some(mut ee), Some(rows)) = (image().map(hooked), rows("ride-2")) else { return };
    let s = seed::read(&|a| ee.u32_at(a), BIKE);
    poke(&mut ee, &rows[0], &READER);
    let (body, n) = hybrid_frame(&mut ee, &s, F(f32::from_bits(DT)));
    assert_eq!(n, 4, "four substeps at 115 a second");
    let want = hex_words(&rows[0]["body"]);
    let parted: Vec<String> = seed::body_words(&body, s.at.body)
        .into_iter()
        .filter(|&(a, w)| want[((a - s.at.body) / 4) as usize] != w)
        .map(|(a, w)| format!("body+{:#x}: {w:#010x} want {:#010x}", a - s.at.body, want[((a - s.at.body) / 4) as usize]))
        .collect();
    assert!(parted.is_empty(), "the port's body is not row 0's:\n  {}", parted.join("\n  "));
    let d = diff(&ee, &rows[0]);
    assert!(d.is_empty(), "row 0: {} words differ\n  {}", d.len(), d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  "));
    assert_ne!(body, s.body, "the frame moved the bike");
}

// --- F31b1: the bike's PreProcess, the port's ----------------------------------------------------------

use game_sim::handling::bike;
use game_sim::handling::effects::{self, Effects};
use game_sim::handling::run;
use game_sim::quad::{self, Call, Moto, Quad};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// The bike's PreProcess (research/oracle.txt `# F31b1`): the preforce list's thunk hands it the bike.
const GROUND_HEAD: u32 = 0x004d_8b20;
/// Its callees that are not the body's: the input limiter and the head and tail through the bike's
/// vtable, the gearbox and the clutch direct.
const AUTO_SHIFT: u32 = 0x004e_5fe0;
const CLUTCH: u32 = 0x004e_6a60;
/// The bike's preforce element: the bike's `+0x30`.
const BIKE_PREFORCE: u32 = BIKE + 0x30;

fn vt(ee: &Ee, slot: u32) -> u32 {
    ee.u32_at(ee.u32_at(BIKE + 4) + slot)
}

fn the_wheels(ee: &Ee) -> [u32; 2] {
    seed::WHEELS.map(|o| ee.u32_at(BIKE + o))
}

/// The quad's [`quad::Body`] off the ATV body at `at` (`tests/atv_quad.rs`' map).
const QUAD_BODY: [u32; 8] = [0xa0, 0x1b0, 0x190, 0x1d0, 0x80, 0xc0, 0x70, 0xd0];

fn quad_body(ee: &Ee, at: u32) -> quad::Body {
    let v = |o: u32| seed::Words::v(&|a| ee.u32_at(a), at + o);
    let [force, moment, orientation, ang_vel, velocity, reset_each_frame, position, correction] = QUAD_BODY.map(v);
    quad::Body { force, moment, orientation, ang_vel, velocity, reset_each_frame, position, correction }
}

fn quad_body_words(b: &quad::Body, at: u32) -> Vec<(u32, u32)> {
    let vs = [b.force, b.moment, b.orientation, b.ang_vel, b.velocity, b.reset_each_frame, b.position, b.correction];
    let mut out = Vec::new();
    for (o, v) in QUAD_BODY.into_iter().zip(vs) {
        for (k, w) in v.to_bits().into_iter().enumerate() {
            out.push((at + o + 4 * k as u32, w));
        }
    }
    out
}

/// The bike in the interpreter's RAM as the port's head sees it, and the words it last read: the
/// port's changes go back word by word (a word it holds as it read stays as RAM has it, a flag word
/// that is neither 0 nor 1 included).
struct InRam<'a> {
    ee: &'a mut Ee,
    wheels: [u32; 2],
    record: u32,
    body: u32,
    last: Vec<(u32, u32)>,
}

impl InRam<'_> {
    fn words(&self, q: &Quad, b: &quad::Body) -> Vec<(u32, u32)> {
        let mut w = bike::quad_words(q, BIKE, self.wheels, self.record);
        w.extend(quad_body_words(b, self.body));
        w
    }

    fn read(&mut self) -> (Quad, quad::Body) {
        let ee = &*self.ee;
        let (q, b) = (bike::read_quad(&|a| ee.u32_at(a), BIKE), quad_body(ee, self.body));
        self.last = self.words(&q, &b);
        (q, b)
    }

    fn write(&mut self, q: &Quad, b: &quad::Body) {
        let now = self.words(q, b);
        for (&(a, w), &(la, lw)) in now.iter().zip(&self.last) {
            assert_eq!(a, la);
            if w != lw {
                self.ee.set_u32(a, w);
            }
        }
        self.last = now;
    }
}

/// Each step the head calls is the original's, run on the bike in RAM.
impl Moto for InRam<'_> {
    fn call(&mut self, c: Call, q: &mut Quad, body: &mut quad::Body) {
        self.write(q, body);
        let (f, x) = match c {
            Call::InputLimiter(dt) => (vt(self.ee, 0x34c), vec![dt.0]),
            Call::AutoShift { dt, inv_radius } => (AUTO_SHIFT, vec![dt.0, inv_radius.0]),
            Call::Clutch(dt) => (CLUTCH, vec![dt.0]),
            Call::HeadTail => (vt(self.ee, 0x4d0), vec![]),
            c => panic!("the bike's head calls no {c:?}"),
        };
        self.ee.set(28, GP);
        self.ee.call(f, &[u64::from(BIKE)], &x, BUDGET);
        (*q, *body) = self.read();
    }
}

/// [`bike::ground_head`] on the bike in RAM, with its callees the original's.
fn port_head(ee: &mut Ee, dt: F) {
    let body = ee.u32_at(BIKE + seed::BODY);
    let (wheels, record) = (the_wheels(ee), ee.u32_at(BIKE + seed::RECORD));
    let words = |ee: &Ee| (bike::read_params(&|a| ee.u32_at(a), BIKE), bike::read_held(&|a| ee.u32_at(a), BIKE));
    let (p, held) = words(ee);
    let mut axes = seed::body(&|a| ee.u32_at(a), body).angular_axes;
    let was = axes;
    let mut m = InRam { ee, wheels, record, body, last: Vec::new() };
    let (mut q, mut b) = m.read();
    bike::ground_head(&p, &mut q, &mut b, held, &mut axes, dt, &mut m);
    m.write(&q, &b);
    for k in 0..3 {
        if axes[k] != was[k] {
            m.ee.set_u32(body + 0x120 + 4 * k as u32, u32::from(axes[k]));
        }
    }
}

/// A frame as [`hybrid_frame`]'s, the bike's PreProcess the port's ([`port_head`]) and the other
/// riders' and the rest of the listeners the original's.
struct PortHead<'a>(Hybrid<'a>);

impl Dynamics for PortHead<'_> {
    fn count(&self, l: List) -> i32 {
        self.0.count(l)
    }
    fn preforce(&mut self, i: i32, dt: F) {
        if self.0.elt(List::Preforce, i) != BIKE_PREFORCE {
            return self.0.preforce(i, dt);
        }
        let h = &mut self.0;
        for (a, w) in seed::body_words(&h.body, h.at) {
            h.ee.set_u32(a, w);
        }
        port_head(h.ee, dt);
        h.body = seed::body(&|a| h.ee.u32_at(a), h.at);
    }
    fn force(&mut self, i: i32, h: F) {
        self.0.force(i, h)
    }
    fn enabled(&self, b: i32) -> bool {
        self.0.enabled(b)
    }
    fn integrate(&mut self, b: i32, h: F) {
        self.0.integrate(b, h)
    }
    fn end_substep(&mut self, b: i32) {
        self.0.end_substep(b)
    }
    fn constrain(&mut self, b: i32, last: bool) {
        self.0.constrain(b, last)
    }
    fn post(&mut self, i: i32, dt: F) {
        self.0.post(i, dt)
    }
}

/// One sim frame as [`hybrid_frame`] runs it, the world step [`PortHead`]'s. Answers how many times
/// the port's head ran.
fn port_head_frame(ee: &mut Ee, dt: F) -> u32 {
    let call = |ee: &mut Ee, f: u32, ints: &[u64], x: F| {
        ee.set(28, GP);
        ee.call(f, ints, &[x.0], BUDGET);
        ee.r(2)
    };
    assert_eq!(ee.u8_at(ee.u32_at(SKIP_FLAG) + 0x225) & 8, 0, "the frame is not skipped");
    for slot in [0x7c, 0x84] {
        for i in 0..ee.u32_at(SIM + 0x24) {
            let e = call(ee, LIST24_GET, &[u64::from(SIM + 0x24), u64::from(i)], F::ZERO) as u32;
            call(ee, ee.u32_at(ee.u32_at(e + 4) + slot), &[u64::from(e)], dt);
        }
    }
    let world = World { iterations: ee.u32_at(WORLD) as i32, rate: F(ee.f32_at(WORLD + 4)) };
    let at = ee.u32_at(BIKE + seed::BODY);
    let body = seed::body(&|a| ee.u32_at(a), at);
    let mut h = PortHead(Hybrid { ee, body, at, integrated: 0 });
    let heads = (0..h.count(List::Preforce)).filter(|&i| h.0.elt(List::Preforce, i) == BIKE_PREFORCE).count() as u32;
    world.step(dt, &mut h);
    let body = h.0.body;
    for (a, w) in seed::body_words(&body, at) {
        ee.set_u32(a, w);
    }
    call(ee, TAIL, &[u64::from(SIM)], dt);
    let mut i = 0;
    while i < ee.u32_at(SIM + 0x3c) {
        let e = call(ee, LIST3C_GET, &[u64::from(SIM + 0x3c), u64::from(i)], F::ZERO) as u32;
        if call(ee, ee.u32_at(ee.u32_at(e) + 8), &[u64::from(e)], dt) != 0 {
            call(ee, LIST3C_REMOVE, &[u64::from(SIM + 0x3c), u64::from(i)], F::ZERO);
        } else {
            i += 1;
        }
    }
    heads
}

/// [`replay`] with every frame [`port_head_frame`].
fn replay_port_head(name: &str) -> Option<Vec<bool>> {
    let mut ee = hooked(image()?);
    let rows = rows(name)?;
    let mut out = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        poke(&mut ee, row, &READER);
        assert_eq!(port_head_frame(&mut ee, F(f32::from_bits(DT))), 1, "the bike is filed once");
        let d = diff(&ee, row);
        assert!(
            d.is_empty(),
            "{name} row {r} (vsync {}), the port's head: {} words differ\n  {}",
            row["vsync"],
            d.len(),
            d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  ")
        );
        let w = wrecked(|o| ee.u32_at(BIKE + o));
        assert_eq!(w, row_wrecked(row), "{name} row {r}: the wreck state");
        out.push(w);
        poke(&mut ee, row, &SPOT);
    }
    Some(out)
}

/// **C15's ride, the bike's PreProcess the port's**: every row of ride-2 word for word.
#[test]
fn ride_replays_bit_exact_on_the_ports_head() {
    let Some(w) = replay_port_head("ride-2") else { return };
    assert_eq!(w.len(), 459);
    assert!(w.iter().all(|w| !w), "the ride never wrecks");
}

/// **C15's flip, the bike's PreProcess the port's**: wrecked on row 125 and put back on row 193, every
/// row word for word.
#[test]
fn flip_wrecks_and_is_put_back_bit_exact_on_the_ports_head() {
    let Some(w) = replay_port_head("flip-1") else { return };
    assert_eq!(w.len(), 465);
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [125, 193], "wrecked on row 125, put back on row 193");
}

struct Gen(u64);

#[allow(clippy::float_arithmetic)]
impl Gen {
    fn bits(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.bits() as f32 / 4294967296.0)
    }
    fn one_in(&mut self, n: u32) -> bool {
        self.bits() % n == 0
    }
    /// `at` one time in five, else anywhere in `lo..hi`; a zero comes signed either way.
    fn near(&mut self, at: f32, lo: f32, hi: f32) -> f32 {
        let x = if self.one_in(5) { at } else { self.f(lo, hi) };
        if x == 0.0 && self.one_in(2) { -0.0 } else { x }
    }
}

type Log = Rc<RefCell<Vec<Call>>>;

/// The words the bike's head may write: the bike, both wheels and the body.
fn head_regions(ee: &Ee) -> Vec<(u32, u32)> {
    let [front, rear] = the_wheels(ee);
    vec![(BIKE, 0x1800), (front, 0x920), (rear, 0x920), (ee.u32_at(BIKE + seed::BODY), 0x230)]
}

fn snap_regions(ee: &Ee, regions: &[(u32, u32)]) -> Vec<u32> {
    regions.iter().flat_map(|&(at, len)| (0..len).step_by(4).map(move |o| ee.u32_at(at + o))).collect()
}

fn restore_regions(ee: &mut Ee, regions: &[(u32, u32)], words: &[u32]) {
    let mut w = words.iter();
    for &(at, len) in regions {
        for o in (0..len).step_by(4) {
            ee.set_u32(at + o, *w.next().unwrap());
        }
    }
}

fn tally(t: &mut BTreeMap<&'static str, u32>, path: &'static str) {
    *t.entry(path).or_insert(0) += 1;
}

/// **The bike's PreProcess is the original's**: 12 000 bikes, each state 4's with the head's inputs
/// drawn at random (edges, ties and signed zeros among them), run through SLUS-21104's head and
/// through [`bike::ground_head`] from the same words. Every word of the bike, its wheels and its body
/// comes out the same, and the steps are called in the same order with the same arguments. The
/// original's steps are hooked to log and do nothing; the body's turn-lock virtual is its own.
#[test]
#[allow(clippy::float_arithmetic)]
fn the_bikes_pre_process_is_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    let (limiter, head_tail) = (vt(&ee, 0x34c), vt(&ee, 0x4d0));
    let log: Log = Rc::default();
    for (at, f) in [
        (limiter, (|ee: &Ee| Call::InputLimiter(F(ee.f(12)))) as fn(&Ee) -> Call),
        (AUTO_SHIFT, |ee| Call::AutoShift { dt: F(ee.f(12)), inv_radius: F(ee.f(13)) }),
        (CLUTCH, |ee| Call::Clutch(F(ee.f(12)))),
        (head_tail, |_| Call::HeadTail),
    ] {
        let l = log.clone();
        ee.hooks.insert(at, Box::new(move |ee: &mut Ee| {
            assert_eq!(ee.r(4) as u32, BIKE, "a step handed something other than the bike");
            l.borrow_mut().push(f(ee));
        }));
    }
    let regions = head_regions(&ee);
    let state = ee.u32_at(BIKE + 0xbc4);
    assert_ne!(state, 0, "state 4's bike has a vehicle state");
    let kind = ee.u32_at(state + 0x28);
    let image_words = snap_regions(&ee, &regions);
    let [front, rear] = the_wheels(&ee);
    let body = ee.u32_at(BIKE + seed::BODY);
    let mut g = Gen(0x0031_b001);
    let mut t = BTreeMap::new();
    let (pi, half_pi) = (std::f32::consts::PI, std::f32::consts::FRAC_PI_2);
    for k in 0..12_000 {
        restore_regions(&mut ee, &regions, &image_words);
        let (b, f) = (|ee: &mut Ee, a: u32, v: bool| ee.set_u32(a, u32::from(v)), |ee: &mut Ee, a: u32, v: f32| ee.set_f32(a, v));
        // The guards: no state, states 3 and 4, others; the balance words; the wreck reason; the hold.
        match g.bits() % 8 {
            0 => ee.set_u32(BIKE + 0xbc4, 0),
            1 => ee.set_u32(state + 0x28, 3),
            2 => ee.set_u32(state + 0x28, 4),
            n => ee.set_u32(state + 0x28, [kind, 0, 1, 2, 5][n as usize - 3]),
        }
        let bal = g.bits();
        ee.set_u32(BIKE + 0x1524, bal);
        ee.set_u32(BIKE + 0x152c, if g.one_in(4) { bal } else { g.bits() });
        ee.set_u32(BIKE + 0xc80, if g.one_in(4) { g.bits() | 1 } else { 0 });
        ee.set_u32(BIKE + 0xddc, u32::from(g.one_in(3)));
        for a in 0..3 {
            b(&mut ee, body + 0x120 + 4 * a, g.one_in(2));
        }
        b(&mut ee, BIKE + 0x324, g.one_in(2));
        // The steering: the fade's speeds (tied a sixth of the time), the lock, the full-lock option.
        let s1 = g.f(0.0, 40.0);
        f(&mut ee, BIKE + 0x1300, s1);
        f(&mut ee, BIKE + 0x1308, if g.one_in(6) { s1 } else { g.f(0.0, 80.0) });
        f(&mut ee, BIKE + 0x1304, g.f(0.0, 1.0));
        f(&mut ee, BIKE + 0x130c, g.f(0.0, 1.0));
        f(&mut ee, BIKE + 0xc30 + 8, g.f(-60.0, 60.0));
        f(&mut ee, front + 0x70, g.f(0.1, 0.8));
        b(&mut ee, BIKE + 0x12f0, g.one_in(2));
        f(&mut ee, BIKE + 0x12f4, g.f(0.0, 0.8));
        f(&mut ee, BIKE + 0x12f8, g.f(0.05, 1.0));
        let timer = g.near(0.0, 0.0, 1.5);
        f(&mut ee, BIKE + 0x12fc, timer);
        let lean = match g.bits() % 6 {
            0 => g.near(0.987, 0.987, 1.0),
            1 => -g.near(0.987, 0.987, 1.0),
            _ => g.near(0.0, -1.0, 1.0),
        };
        f(&mut ee, BIKE + 0x12e8, lean);
        f(&mut ee, BIKE + 0x12cc, g.f(0.0, 1.0));
        // The donut: the gate open half the time; the ramps' ends at their edges now and then.
        let open = g.one_in(2);
        b(&mut ee, BIKE + 0x1270, open || g.one_in(2));
        let top = g.f(5.0, 30.0);
        f(&mut ee, BIKE + 0x122c, top);
        f(&mut ee, BIKE + 0x32c, if open { g.f(0.0, top) } else { g.f(0.0, 40.0) });
        f(&mut ee, BIKE + 0x12bc, if open { g.near(0.9, 0.9, 1.0) } else { g.f(0.0, 1.0) });
        f(&mut ee, BIKE + 0x314, if open { g.f(0.05, 1.0) } else { g.near(0.0, 0.0, 1.0) });
        f(&mut ee, BIKE + 0x1230, g.near(0.0, -half_pi, 0.0));
        f(&mut ee, BIKE + 0x1234, g.near(-pi, -pi, -half_pi));
        f(&mut ee, BIKE + 0x12b8, g.near(-half_pi, -3.3, 3.3));
        // The springs' ends and the speed they map.
        for o in [0x137c, 0x1380, 0x1384, 0x1388] {
            f(&mut ee, BIKE + o, g.f(0.0, 20.0));
        }
        if g.one_in(4) {
            f(&mut ee, BIKE + 0x32c, g.near(14.6666, 0.0, 40.0));
        }
        f(&mut ee, rear + 0x838, g.f(0.5, 2.0));
        let dt = if g.one_in(2) { f32::from_bits(DT) } else { g.f(0.005, 0.04) };
        let before = snap_regions(&ee, &regions);

        let r = |a| ee.u32_at(a);
        let (p, held) = (bike::read_params(&r, BIKE), bike::read_held(&r, BIKE));
        let (mut q, mut qb) = (bike::read_quad(&r, BIKE), quad_body(&ee, body));
        let mut axes = seed::body(&r, body).angular_axes;
        let (q0, qb0, axes0) = (q, qb, axes);
        let mut calls = Vec::new();
        bike::ground_head(&p, &mut q, &mut qb, held, &mut axes, F(dt), &mut calls);
        if !q0.active {
            tally(&mut t, "off");
        } else if q0.balancing {
            tally(&mut t, if held { "balancing, held" } else { "balancing" });
        } else if q0.control_off {
            tally(&mut t, if held { "wrecked, held" } else { "wrecked" });
        } else {
            tally(&mut t, match (q0.on_ground, held) {
                (false, _) => "steer, in the air",
                (true, false) => "steer, on the ground",
                (true, true) => "steer, on the ground, held",
            });
            tally(&mut t, match (p.full_lock, q0.lean.abs() < F(0.987)) {
                (false, _) if p.steer_speed_1 == p.steer_speed_2 => "steer, flat fade",
                (false, _) => "steer, faded",
                (true, true) => "full lock, let go",
                (true, false) => "full lock, held",
            });
        }
        if q0.active && q.wheels[bike::REAR].donut != F::ZERO {
            tally(&mut t, "donut");
        }

        // The port's words: what it holds changed, written over the words as they were.
        let mut want = before.clone();
        let index = |a: u32| {
            let mut i = 0;
            for &(at, len) in &regions {
                if (at..at + len).contains(&a) {
                    return i + ((a - at) / 4) as usize;
                }
                i += (len / 4) as usize;
            }
            panic!("{a:#x} is in none of the regions")
        };
        let words = |q: &Quad, b: &quad::Body| {
            let mut w = bike::quad_words(q, BIKE, [front, rear], ee.u32_at(BIKE + seed::RECORD));
            w.extend(quad_body_words(b, body));
            w
        };
        for ((a, w), (_, w0)) in words(&q, &qb).into_iter().zip(words(&q0, &qb0)) {
            if w != w0 {
                want[index(a)] = w;
            }
        }
        for a in 0..3 {
            if axes[a] != axes0[a] {
                want[index(body + 0x120 + 4 * a as u32)] = u32::from(axes[a]);
            }
        }

        log.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(GROUND_HEAD, &[u64::from(BIKE)], &[dt], 1_000_000);
        let got = snap_regions(&ee, &regions);
        if let Some(i) = (0..got.len()).find(|&i| got[i] != want[i]) {
            let (mut at, mut base) = (i as u32 * 4, 0);
            for &(b, len) in &regions {
                if at < len {
                    base = b;
                    break;
                }
                at -= len;
            }
            panic!(
                "bike {k}: {:#x}+{at:#x} is {:#010x} ({}) in the original and {:#010x} ({}) in the port (was {:#010x})\n{q0:#?}\n{p:#?}",
                base,
                got[i],
                f32::from_bits(got[i]),
                want[i],
                f32::from_bits(want[i]),
                before[i],
            );
        }
        assert_eq!(*log.borrow(), calls, "bike {k}: the steps\n{q0:#?}");
        ee.set_u32(BIKE + 0xbc4, state);
        ee.set_u32(state + 0x28, kind);
    }
    for want in [
        "off",
        "balancing",
        "balancing, held",
        "wrecked",
        "wrecked, held",
        "steer, in the air",
        "steer, on the ground",
        "steer, on the ground, held",
        "steer, flat fade",
        "steer, faded",
        "full lock, let go",
        "full lock, held",
        "donut",
    ] {
        let n = t.get(want).copied().unwrap_or(0);
        assert!(n >= 30, "only {n} bike(s) went down {want:?}: {t:?}");
    }
}

// --- F31b2: the bike's Force body, the port's -----------------------------------------------------------

/// The bike's Force body (research/oracle.txt `# F31b2`), and what it calls: the vehicle's gravity
/// (through its thunk), the bike's air and engine steps, the wheels' steps, the contacts, the rev
/// limiter and the peak rates. Its own controllers (the stand-upright, the balance, the roll/pitch)
/// and the push are not here: the port runs those.
const GROUND_STEP: u32 = 0x004d_90f0;
const GRAVITY: u32 = 0x004a_4ef0;
const PRELOAD: u32 = 0x004e_60d0;
const BACKFLIP: u32 = 0x004e_4f20;
const SPIN_AIR: u32 = 0x004e_53d0;
const ENGINE_UPDATE: u32 = 0x0049_fbb0;
const AIR_DRAG: u32 = 0x004e_6c00;
const AIRBORNE_CONTROL: u32 = 0x004e_65d0;
const WHEEL_CLEAR: u32 = 0x004a_0c30;
const CONTACTS: u32 = 0x004d_ccd0;
const SUSPENSION: u32 = 0x004a_0cb0;
const TIRE_FORCES: u32 = 0x004a_16f0;
const WHEEL_INTEGRATE: u32 = 0x004a_1680;
const REV_LIMIT: u32 = 0x0049_ff40;
const PEAK_RATES: u32 = 0x004e_6c90;
/// The engine's smooth noise, which the balance calls.
const SMOOTH_NOISE: u32 = 0x0013_4580;
/// The force listener's element hands the bike's step `+0x3c` of the bike.
const BIKE_FORCE: u32 = BIKE + 0x3c;

/// The callee of `c`, and what it is handed in `a0`: the bike, a wheel or the engine.
fn step_callee(ee: &Ee, c: Call) -> (u32, u32) {
    let [front, rear] = the_wheels(ee);
    let wheel = |w: usize| match w {
        bike::FRONT => front,
        bike::REAR => rear,
        w => panic!("the bike has no wheel {w}"),
    };
    let engine = ee.u32_at(BIKE + seed::ENGINE);
    match c {
        Call::Gravity => (GRAVITY, BIKE),
        Call::Preload(_) => (PRELOAD, BIKE),
        Call::Backflip(_) => (BACKFLIP, BIKE),
        Call::SpinAir(_) => (SPIN_AIR, BIKE),
        Call::EngineUpdate(_) => (ENGINE_UPDATE, engine),
        Call::AirDrag(_) => (AIR_DRAG, BIKE),
        Call::AirborneControl(_) => (AIRBORNE_CONTROL, BIKE),
        Call::WheelClear(w) => (WHEEL_CLEAR, wheel(w)),
        Call::Contacts => (CONTACTS, BIKE),
        Call::Suspension(w, _) => (SUSPENSION, wheel(w)),
        Call::TireForces(w, _) => (TIRE_FORCES, wheel(w)),
        Call::WheelIntegrate(w, _) => (WHEEL_INTEGRATE, wheel(w)),
        Call::RevLimit => (REV_LIMIT, engine),
        Call::PeakRates => (PEAK_RATES, BIKE),
        c => panic!("the bike's Force body calls no {c:?}"),
    }
}

/// The bike in RAM as the port's Force body sees it: [`InRam`]'s words and the [`bike::Rider`]'s.
/// Each step is the original's, run on the bike in RAM, but for the two whose `quad::Steps` port
/// needs nothing the quad's types do not hold: the air drag (its drag the mass part's `+4`) and the
/// peak rates ([`game_sim::atv_moto`]'s). The noise is the original's too.
struct StepInRam<'a> {
    ram: InRam<'a>,
    rider: bike::Rider,
    rider_last: Vec<(u32, u32)>,
    /// The substep's `dt`, which the original leaves in `f12` for the calls that do not take it.
    dt: F,
}

impl StepInRam<'_> {
    fn read(&mut self) -> (Quad, quad::Body) {
        let out = self.ram.read();
        let ee = &*self.ram.ee;
        self.rider = bike::read_rider(&|a| ee.u32_at(a), BIKE, self.ram.body);
        self.rider_last = bike::rider_words(&self.rider, BIKE, self.ram.body);
        out
    }

    fn write(&mut self, q: &Quad, b: &quad::Body) {
        self.ram.write(q, b);
        let now = bike::rider_words(&self.rider, BIKE, self.ram.body);
        for (&(a, w), &(_, lw)) in now.iter().zip(&self.rider_last) {
            if w != lw {
                self.ram.ee.set_u32(a, w);
            }
        }
        self.rider_last = now;
    }
}

impl Moto for StepInRam<'_> {
    fn call(&mut self, c: Call, q: &mut Quad, body: &mut quad::Body) {
        match c {
            Call::AirDrag(_) => {
                let drag = F(self.ram.ee.f32_at(self.ram.ee.u32_at(BIKE + 0xe8) + 4));
                return game_sim::atv_moto::air_drag(drag, q, body);
            }
            Call::PeakRates => return game_sim::atv_moto::peak_rates(q, body),
            _ => {}
        }
        self.write(q, body);
        let (f, a0) = step_callee(self.ram.ee, c);
        let dt = match c {
            Call::Preload(x) | Call::Backflip(x) | Call::SpinAir(x) | Call::EngineUpdate(x) | Call::AirborneControl(x) => x,
            Call::Suspension(_, x) | Call::TireForces(_, x) | Call::WheelIntegrate(_, x) => x,
            _ => self.dt,
        };
        self.ram.ee.set(28, GP);
        self.ram.ee.call(f, &[u64::from(a0)], &[dt.0], BUDGET);
        (*q, *body) = self.read();
    }
}

impl bike::Rides for StepInRam<'_> {
    fn rider(&mut self) -> &mut bike::Rider {
        &mut self.rider
    }
    fn noise(&mut self, t: F) -> F {
        self.ram.ee.set(28, GP);
        self.ram.ee.call(SMOOTH_NOISE, &[], &[t.0], BUDGET);
        F(self.ram.ee.f(0))
    }
}

/// [`bike::ground_step`] on the bike in RAM, with its callees the original's.
fn port_step(ee: &mut Ee, dt: F) {
    let body = ee.u32_at(BIKE + seed::BODY);
    let (wheels, record) = (the_wheels(ee), ee.u32_at(BIKE + seed::RECORD));
    let p = bike::read_params(&|a| ee.u32_at(a), BIKE);
    let ram = InRam { ee, wheels, record, body, last: Vec::new() };
    let mut m = StepInRam { ram, rider: bike::Rider::default(), rider_last: Vec::new(), dt };
    let (mut q, mut b) = m.read();
    bike::ground_step(&p, &mut q, &mut b, dt, &mut m);
    m.write(&q, &b);
}

/// A frame as [`PortHead`]'s, the bike's Force body the port's ([`port_step`]) as well.
struct PortStep<'a>(PortHead<'a>, u32);

impl Dynamics for PortStep<'_> {
    fn count(&self, l: List) -> i32 {
        self.0.count(l)
    }
    fn preforce(&mut self, i: i32, dt: F) {
        self.0.preforce(i, dt)
    }
    fn force(&mut self, i: i32, h: F) {
        let e = self.0.0.elt(List::Force, i);
        if self.0.0.ee.u32_at(e + 0xc) != BIKE_FORCE {
            return self.0.force(i, h);
        }
        let x = &mut self.0.0;
        for (a, w) in seed::body_words(&x.body, x.at) {
            x.ee.set_u32(a, w);
        }
        port_step(x.ee, h);
        x.body = seed::body(&|a| x.ee.u32_at(a), x.at);
        self.1 += 1;
    }
    fn enabled(&self, b: i32) -> bool {
        self.0.enabled(b)
    }
    fn integrate(&mut self, b: i32, h: F) {
        self.0.integrate(b, h)
    }
    fn end_substep(&mut self, b: i32) {
        self.0.end_substep(b)
    }
    fn constrain(&mut self, b: i32, last: bool) {
        self.0.constrain(b, last)
    }
    fn post(&mut self, i: i32, dt: F) {
        self.0.post(i, dt)
    }
}

/// One sim frame as [`port_head_frame`] runs it, the world step [`PortStep`]'s. Answers how many
/// times the port's Force body ran.
fn port_step_frame(ee: &mut Ee, dt: F) -> u32 {
    let call = |ee: &mut Ee, f: u32, ints: &[u64], x: F| {
        ee.set(28, GP);
        ee.call(f, ints, &[x.0], BUDGET);
        ee.r(2)
    };
    assert_eq!(ee.u8_at(ee.u32_at(SKIP_FLAG) + 0x225) & 8, 0, "the frame is not skipped");
    for slot in [0x7c, 0x84] {
        for i in 0..ee.u32_at(SIM + 0x24) {
            let e = call(ee, LIST24_GET, &[u64::from(SIM + 0x24), u64::from(i)], F::ZERO) as u32;
            call(ee, ee.u32_at(ee.u32_at(e + 4) + slot), &[u64::from(e)], dt);
        }
    }
    let world = World { iterations: ee.u32_at(WORLD) as i32, rate: F(ee.f32_at(WORLD + 4)) };
    let at = ee.u32_at(BIKE + seed::BODY);
    let body = seed::body(&|a| ee.u32_at(a), at);
    let mut h = PortStep(PortHead(Hybrid { ee, body, at, integrated: 0 }), 0);
    world.step(dt, &mut h);
    let (body, steps) = (h.0.0.body, h.1);
    for (a, w) in seed::body_words(&body, at) {
        ee.set_u32(a, w);
    }
    call(ee, TAIL, &[u64::from(SIM)], dt);
    let mut i = 0;
    while i < ee.u32_at(SIM + 0x3c) {
        let e = call(ee, LIST3C_GET, &[u64::from(SIM + 0x3c), u64::from(i)], F::ZERO) as u32;
        if call(ee, ee.u32_at(ee.u32_at(e) + 8), &[u64::from(e)], dt) != 0 {
            call(ee, LIST3C_REMOVE, &[u64::from(SIM + 0x3c), u64::from(i)], F::ZERO);
        } else {
            i += 1;
        }
    }
    steps
}

/// [`replay`] with every frame [`port_step_frame`].
fn replay_port_step(name: &str) -> Option<Vec<bool>> {
    let mut ee = hooked(image()?);
    let rows = rows(name)?;
    let mut out = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        poke(&mut ee, row, &READER);
        assert_eq!(port_step_frame(&mut ee, F(f32::from_bits(DT))), 4, "the bike's Force body runs every substep");
        let d = diff(&ee, row);
        assert!(
            d.is_empty(),
            "{name} row {r} (vsync {}), the port's head and Force body: {} words differ\n  {}",
            row["vsync"],
            d.len(),
            d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  ")
        );
        let w = wrecked(|o| ee.u32_at(BIKE + o));
        assert_eq!(w, row_wrecked(row), "{name} row {r}: the wreck state");
        out.push(w);
        poke(&mut ee, row, &SPOT);
    }
    Some(out)
}

/// **C15's ride, the bike's PreProcess and Force body the port's**: every row of ride-2 word for word.
#[test]
fn ride_replays_bit_exact_on_the_ports_force_body() {
    let Some(w) = replay_port_step("ride-2") else { return };
    assert_eq!(w.len(), 459);
    assert!(w.iter().all(|w| !w), "the ride never wrecks");
}

/// **C15's flip, the bike's PreProcess and Force body the port's**: wrecked on row 125 and put back on
/// row 193, every row word for word.
#[test]
fn flip_wrecks_and_is_put_back_bit_exact_on_the_ports_force_body() {
    let Some(w) = replay_port_step("flip-1") else { return };
    assert_eq!(w.len(), 465);
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [125, 193], "wrecked on row 125, put back on row 193");
}

/// A stand-in for the smooth noise, both sides alike (`tests/mxvatv_step.rs`'), reaching `±1`.
fn fake_noise(x: f32) -> f32 {
    const WAVE: [f32; 8] = [0.0, 1.0, -1.0, 0.5, -0.25, 0.937_5, -0.812_5, 0.125];
    WAVE[(x.to_bits() >> 3) as usize & 7]
}

/// The port's side of the oracle: the steps logged, the rider held, the noise [`fake_noise`].
struct Logged {
    calls: Vec<Call>,
    rider: bike::Rider,
}

impl Moto for Logged {
    fn call(&mut self, c: Call, _: &mut Quad, _: &mut quad::Body) {
        self.calls.push(c);
    }
}

impl bike::Rides for Logged {
    fn rider(&mut self) -> &mut bike::Rider {
        &mut self.rider
    }
    fn noise(&mut self, t: F) -> F {
        F(fake_noise(t.0))
    }
}

/// **The bike's Force body is the original's**: 12 000 bikes, each state 4's with the body's inputs
/// drawn at random (the guards, the balance words, the rider's balance keys and state, the pitch
/// keys, the levers and speed, the wheels' contacts, normals and steering, the body's force, moment,
/// spin, quaternion and centre of gravity, the push), run through SLUS-21104's Force body and
/// through [`bike::ground_step`] from the same words. Every word of the bike, its wheels and its body
/// comes out the same, and the steps are called in the same order, on the same thing, with the same
/// `dt`. The original's steps are hooked to log and do nothing, and its noise to [`fake_noise`]; its
/// controllers, push and the body's local force virtual are its own.
#[test]
#[allow(clippy::float_arithmetic)]
fn the_bikes_force_body_is_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    let [front, rear] = the_wheels(&ee);
    let engine = ee.u32_at(BIKE + seed::ENGINE);
    let log: Log = Rc::default();
    let wheel = move |ee: &Ee| match ee.r(4) as u32 {
        a if a == front => bike::FRONT,
        a if a == rear => bike::REAR,
        a => panic!("a wheel step handed {a:#x}"),
    };
    let on = |a0: u32, ee: &Ee, what: &str| assert_eq!(ee.r(4) as u32, a0, "{what} handed something else");
    type Step = Box<dyn Fn(&Ee) -> Call>;
    let steps: Vec<(u32, Step)> = vec![
        (GRAVITY, Box::new(move |ee| (on(BIKE, ee, "the gravity"), Call::Gravity).1)),
        (PRELOAD, Box::new(move |ee| (on(BIKE, ee, "the preload"), Call::Preload(F(ee.f(12)))).1)),
        (BACKFLIP, Box::new(move |ee| (on(BIKE, ee, "the backflip"), Call::Backflip(F(ee.f(12)))).1)),
        (SPIN_AIR, Box::new(move |ee| (on(BIKE, ee, "the air spin"), Call::SpinAir(F(ee.f(12)))).1)),
        (ENGINE_UPDATE, Box::new(move |ee| (on(engine, ee, "the engine"), Call::EngineUpdate(F(ee.f(12)))).1)),
        (AIR_DRAG, Box::new(move |ee| (on(BIKE, ee, "the air drag"), Call::AirDrag(F(ee.f(12)))).1)),
        (AIRBORNE_CONTROL, Box::new(move |ee| (on(BIKE, ee, "the air control"), Call::AirborneControl(F(ee.f(12)))).1)),
        (WHEEL_CLEAR, Box::new(move |ee| Call::WheelClear(wheel(ee)))),
        (CONTACTS, Box::new(move |ee| (on(BIKE, ee, "the contacts"), Call::Contacts).1)),
        (SUSPENSION, Box::new(move |ee| Call::Suspension(wheel(ee), F(ee.f(12))))),
        (TIRE_FORCES, Box::new(move |ee| Call::TireForces(wheel(ee), F(ee.f(12))))),
        (WHEEL_INTEGRATE, Box::new(move |ee| Call::WheelIntegrate(wheel(ee), F(ee.f(12))))),
        (REV_LIMIT, Box::new(move |ee| (on(engine, ee, "the rev limiter"), Call::RevLimit).1)),
        (PEAK_RATES, Box::new(move |ee| (on(BIKE, ee, "the peak rates"), Call::PeakRates).1)),
    ];
    for (at, f) in steps {
        let l = log.clone();
        ee.hooks.insert(at, Box::new(move |ee: &mut Ee| l.borrow_mut().push(f(ee))));
    }
    ee.hooks.insert(SMOOTH_NOISE, Box::new(|ee: &mut Ee| {
        let x = ee.f(12);
        ee.set_f(0, fake_noise(x));
    }));
    let regions = head_regions(&ee);
    let state = ee.u32_at(BIKE + 0xbc4);
    let kind = ee.u32_at(state + 0x28);
    let image_words = snap_regions(&ee, &regions);
    let body = ee.u32_at(BIKE + seed::BODY);
    let mut g = Gen(0x0031_b002);
    let mut t = BTreeMap::new();
    for k in 0..12_000 {
        restore_regions(&mut ee, &regions, &image_words);
        let (b, f) = (|ee: &mut Ee, a: u32, v: bool| ee.set_u32(a, u32::from(v)), |ee: &mut Ee, a: u32, v: f32| ee.set_f32(a, v));
        let v = |ee: &mut Ee, g: &mut Gen, a: u32, r: f32| {
            for i in 0..3 {
                let x = g.near(0.0, -r, r);
                ee.set_f32(a + 4 * i, x);
            }
        };
        // The guards: no state, states 3 and 4, others; the wreck reason; the balance words.
        match g.bits() % 12 {
            0 => ee.set_u32(BIKE + 0xbc4, 0),
            1 => ee.set_u32(state + 0x28, 3),
            2 => ee.set_u32(state + 0x28, 4),
            n => ee.set_u32(state + 0x28, [kind, kind, kind, kind, 0, 1, 2, 5, 6][n as usize - 3]),
        }
        ee.set_u32(BIKE + 0xc80, if g.one_in(6) { g.bits() | 1 } else { 0 });
        let bal = g.bits();
        ee.set_u32(BIKE + 0x1524, bal);
        ee.set_u32(BIKE + 0x152c, if g.one_in(3) { bal } else { g.bits() });
        // The rider's balance keys and state, and the stick.
        for (o, lo, hi) in [
            (0x14fc, 0.0, 20.0),
            (0x1500, 0.1, 2.0),
            (0x1504, 2.0, 10.0),
            (0x1508, 0.0, 5.0),
            (0x1514, 0.0, 1.0),
            (0x151c, 0.0, 3.0),
            (0x1520, 0.0, 2.0),
            (0x1398, 0.0, 5.0),
            (0x138c, 0.0, 100.0),
            (0x1390, 0.0, 50.0),
            (0x1394, 0.0, 1.0),
            (0x14f8, 0.0, 10.0),
            (0x150c, -1.2, 1.2),
            (0x1510, -2.0, 2.0),
            (0x1518, 0.0, 30.0),
            (0x12a8, -1.0, 1.0),
        ] {
            let x = g.near(0.0, lo, hi);
            f(&mut ee, BIKE + o, x);
        }
        // The pitch keys and state, the clutch.
        for (o, lo, hi) in [
            (0x139c, 0.0, 3.0),
            (0x13a0, 0.0, 200.0),
            (0x13a4, 0.0, 50.0),
            (0x13a8, 0.0, 0.2),
            (0x13cc, 0.0, 3.0),
            (0x13d0, 0.0, 200.0),
            (0x13d4, 0.0, 5.0),
            (0x13d8, 0.0, 20.0),
            (0x13ac, -50.0, 50.0),
        ] {
            let x = g.f(lo, hi);
            f(&mut ee, BIKE + o, x);
        }
        b(&mut ee, BIKE + 0x13b0, g.one_in(2));
        f(&mut ee, BIKE + 0x12d4, if g.one_in(4) { g.f(0.0, 1.0) } else { 0.0 });
        // The step's words: on the ground, both wheels, the levers, the signed speed, the leans.
        b(&mut ee, BIKE + 0x324, !g.one_in(8));
        b(&mut ee, BIKE + 0x1270, !g.one_in(4));
        f(&mut ee, BIKE + 0x314, if g.one_in(4) { g.f(0.0, 1.0) } else { 0.0 });
        f(&mut ee, BIKE + 0x310, if g.one_in(2) { g.f(0.0, 1.0) } else { 0.0 });
        let speed = match g.bits() % 6 {
            0 => *[5.0, 15.0, 7.0, -10.0].get((g.bits() % 4) as usize).unwrap(),
            1 => g.f(-12.0, 12.0),
            _ => g.f(-10.0, 50.0),
        };
        f(&mut ee, BIKE + 0xc38, speed);
        f(&mut ee, BIKE + 0x12e8, g.f(-1.0, 1.0));
        let fore_aft = match g.bits() % 7 {
            5 | 6 => -1.0,
            0 => *[0.5, -1.0, -0.1].get((g.bits() % 3) as usize).unwrap(),
            1 => g.f(0.5, 1.0),
            2 => g.f(-1.0, -0.1),
            _ => g.f(-1.0, 1.0),
        };
        f(&mut ee, BIKE + 0x12ec, fore_aft);
        f(&mut ee, BIKE + 0x318, if g.one_in(4) { 0.0 } else { g.f(0.0, 1.0) });
        f(&mut ee, BIKE + 0x12cc, if g.one_in(2) { g.f(0.0, 1.0) } else { 0.0 });
        f(&mut ee, BIKE + 0x12d0, if g.one_in(4) { g.f(0.0, 1.0) } else { 0.0 });
        // The wheels: contact, normal, steering, the rear's pair flag as it was.
        for w in [front, rear] {
            b(&mut ee, w + 0xdc, g.bits() % 4 != 0);
            let n = [g.f(-0.5, 0.5), g.f(0.5, 1.0), g.f(-0.5, 0.5)];
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            for i in 0..3 {
                f(&mut ee, w + 0x130 + 4 * i as u32, n[i] / l);
            }
            b(&mut ee, w + 0x2c, g.one_in(2));
        }
        b(&mut ee, rear + 0xb4, g.one_in(2));
        // The body and the bike's frames.
        v(&mut ee, &mut g, body + 0xa0, 500.0);
        v(&mut ee, &mut g, body + 0x1b0, 500.0);
        v(&mut ee, &mut g, body + 0x1a0, 1.0);
        v(&mut ee, &mut g, body + 0x1d0, 5.0);
        v(&mut ee, &mut g, BIKE + 0x360, 1.0);
        v(&mut ee, &mut g, BIKE + 0x340, 5.0);
        let qt = [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0)];
        let l = qt.iter().map(|x| x * x).sum::<f32>().sqrt();
        for i in 0..4 {
            f(&mut ee, body + 0x190 + 4 * i as u32, qt[i] / l);
        }
        // The push: its flag, gate, gain and target.
        b(&mut ee, BIKE + 0xdd8, g.one_in(2));
        f(&mut ee, BIKE + 0x12c4, if g.one_in(3) { 0.0 } else { g.f(0.0, 1.0) });
        f(&mut ee, BIKE + 0x1240, g.f(0.0, 200.0));
        f(&mut ee, BIKE + 0x123c, g.f(0.0, 30.0));
        let dt = if g.one_in(2) { f32::from_bits(DT) / 4.0 } else { g.f(0.002, 0.02) };
        let before = snap_regions(&ee, &regions);

        let r = |a| ee.u32_at(a);
        let p = bike::read_params(&r, BIKE);
        let (mut q, mut qb) = (bike::read_quad(&r, BIKE), quad_body(&ee, body));
        let rider = bike::read_rider(&r, BIKE, body);
        let (q0, qb0) = (q, qb);
        let mut m = Logged { calls: Vec::new(), rider };
        bike::ground_step(&p, &mut q, &mut qb, F(dt), &mut m);
        if !q0.active {
            tally(&mut t, "off");
        } else if q0.control_off {
            tally(&mut t, "wrecked");
        } else {
            tally(&mut t, if q0.balancing { "balancing" } else { "riding" });
            if q0.balancing && !q0.wheels[bike::REAR].touching {
                tally(&mut t, "balancing, rear up");
            }
            if !q0.on_ground {
                tally(&mut t, "in the air");
            }
            match (q0.stoppie, q.stoppie) {
                (false, true) => tally(&mut t, "stoppie on"),
                (true, false) => tally(&mut t, "stoppie ended"),
                (true, true) => tally(&mut t, "stoppie held"),
                _ => {}
            }
            if q.pitch_integral != q0.pitch_integral {
                tally(&mut t, "wheelie");
            }
            if q0.push && q0.push_gate != F::ZERO {
                tally(&mut t, if q0.wheels[bike::REAR].touching { "pushed" } else { "push, rear up" });
            }
        }
        if q0.active {
            tally(&mut t, if q.wheels[bike::REAR].rear_pair { "rear pair set" } else { "rear pair cleared" });
        }

        let mut want = before.clone();
        let index = |a: u32| {
            let mut i = 0;
            for &(at, len) in &regions {
                if (at..at + len).contains(&a) {
                    return i + ((a - at) / 4) as usize;
                }
                i += (len / 4) as usize;
            }
            panic!("{a:#x} is in none of the regions")
        };
        let words = |q: &Quad, b: &quad::Body, r: &bike::Rider| {
            let mut w = bike::quad_words(q, BIKE, [front, rear], ee.u32_at(BIKE + seed::RECORD));
            w.extend(quad_body_words(b, body));
            w.extend(bike::rider_words(r, BIKE, body));
            w
        };
        for ((a, w), (_, w0)) in words(&q, &qb, &m.rider).into_iter().zip(words(&q0, &qb0, &rider)) {
            if w != w0 {
                want[index(a)] = w;
            }
        }

        log.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(GROUND_STEP, &[u64::from(BIKE)], &[dt], 1_000_000);
        let got = snap_regions(&ee, &regions);
        if let Some(i) = (0..got.len()).find(|&i| got[i] != want[i]) {
            let (mut at, mut base) = (i as u32 * 4, 0);
            for &(b, len) in &regions {
                if at < len {
                    base = b;
                    break;
                }
                at -= len;
            }
            panic!(
                "bike {k}: {:#x}+{at:#x} is {:#010x} ({}) in the original and {:#010x} ({}) in the port (was {:#010x})\n{q0:#?}\n{rider:#?}",
                base,
                got[i],
                f32::from_bits(got[i]),
                want[i],
                f32::from_bits(want[i]),
                before[i],
            );
        }
        assert_eq!(*log.borrow(), m.calls, "bike {k}: the steps\n{q0:#?}");
        ee.set_u32(BIKE + 0xbc4, state);
        ee.set_u32(state + 0x28, kind);
    }
    for want in [
        "off",
        "wrecked",
        "riding",
        "balancing",
        "balancing, rear up",
        "in the air",
        "stoppie on",
        "stoppie ended",
        "stoppie held",
        "wheelie",
        "pushed",
        "push, rear up",
        "rear pair set",
        "rear pair cleared",
    ] {
        let n = t.get(want).copied().unwrap_or(0);
        assert!(n >= 30, "only {n} bike(s) went down {want:?}: {t:?}");
    }
}

// --- F31b3: the bike's PostProcess, the port's ---------------------------------------------------------

/// The bike's PostProcess (research/oracle.txt `# F31b3`): the post list's thunk hands it the bike.
const AIR_STATE: u32 = 0x004d_9290;
/// The bike's post element: the bike's `+0x34`.
const BIKE_POST: u32 = BIKE + 0x34;
/// Its callees that are not the bike's vtable's: the vehicle's in-world test (also its vt `+0x120`),
/// the message post and the queue it is handed, and the node's placement, position and bounds.
const IN_WORLD: u32 = 0x0049_4200;
const POST_MESSAGE: u32 = 0x0027_2300;
const POST_QUEUE: u32 = 0x0044_2eb0;
const NODE_PLACE: u32 = 0x0019_4650;
const NODE_POSITION: u32 = 0x0019_4c20;
const NODE_BOUNDS: u32 = 0x0019_6e80;
/// The rodata `(0, 0, 0, 1)` the no-hander builds its frame at.
const FRAME_AT: u32 = 0x0039_6fc0;
/// The bike's vtable slots the PostProcess calls: the put-back, and the vehicle post's body reset,
/// respawn, spatial update and collision sync.
const PUT_BACK_SLOT: u32 = 0x3a8;
const POST_SLOTS: [(u32, Call); 4] =
    [(0x13c, Call::BodyReset), (0xe8, Call::Respawn), (0xe4, Call::SpatialUpdate), (0x124, Call::CollisionSync)];

/// The bike in RAM as the port's PostProcess sees it: [`StepInRam`]'s words and the [`bike::Post`]'s.
/// Each call is the original's, run on the bike in RAM with `dt` in `f12`.
struct PostInRam<'a> {
    step: StepInRam<'a>,
    post: bike::Post,
    post_last: Vec<(u32, u32)>,
}

impl PostInRam<'_> {
    fn read(&mut self) -> (Quad, quad::Body) {
        let out = self.step.read();
        let ee = &*self.step.ram.ee;
        self.post = bike::read_post(&|a| ee.u32_at(a), BIKE, self.step.ram.body);
        self.post_last = bike::post_words(&self.post, BIKE);
        out
    }

    fn write(&mut self, q: &Quad, b: &quad::Body) {
        self.step.write(q, b);
        let now = bike::post_words(&self.post, BIKE);
        for (&(a, w), &(_, lw)) in now.iter().zip(&self.post_last) {
            if w != lw {
                self.step.ram.ee.set_u32(a, w);
            }
        }
        self.post_last = now;
    }

    fn run(&mut self, f: u32, ints: &[u64], q: &mut Quad, body: &mut quad::Body) {
        self.write(q, body);
        let ee = &mut *self.step.ram.ee;
        ee.set(28, GP);
        ee.call(f, ints, &[self.step.dt.0], BUDGET);
        (*q, *body) = self.read();
    }
}

impl Moto for PostInRam<'_> {
    fn call(&mut self, c: Call, q: &mut Quad, body: &mut quad::Body) {
        let ee = &*self.step.ram.ee;
        let node = u64::from(ee.u32_at(BIKE + 0xe4));
        let (f, ints) = match c {
            Call::NodeBounds => (NODE_BOUNDS, vec![node]),
            Call::NodePosition => (NODE_POSITION, vec![node, 0, u64::from(BIKE + 0xbd0)]),
            c => match POST_SLOTS.iter().find(|s| s.1 == c) {
                Some(&(slot, _)) => (vt(ee, slot), vec![u64::from(BIKE)]),
                None => panic!("the bike's PostProcess calls no {c:?}"),
            },
        };
        self.run(f, &ints, q, body);
    }
}

impl bike::Rides for PostInRam<'_> {
    fn rider(&mut self) -> &mut bike::Rider {
        &mut self.step.rider
    }
    fn noise(&mut self, _: F) -> F {
        panic!("the bike's PostProcess makes no noise")
    }
}

impl bike::Posts for PostInRam<'_> {
    fn post(&mut self) -> &mut bike::Post {
        &mut self.post
    }
    fn out(&mut self, c: bike::Out, q: &mut Quad, body: &mut quad::Body) {
        let ee = &mut *self.step.ram.ee;
        let (f, ints) = match c {
            bike::Out::Place { orientation, at } => {
                assert_eq!(orientation.to_bits(), body.orientation.to_bits(), "the node is placed with the body's quaternion");
                let (node, to) = (ee.u32_at(BIKE + 0xe4), ee.scratch(0));
                for (k, w) in at.to_bits().into_iter().enumerate() {
                    ee.set_u32(to + 4 * k as u32, w);
                }
                (NODE_PLACE, vec![u64::from(node), 0, u64::from(self.step.ram.body + 0x190), u64::from(to)])
            }
            bike::Out::OutOfWorld(id) => {
                let (kind, msg) = bike::OUT_OF_WORLD;
                (POST_MESSAGE, vec![u64::from(ee.u32_at(POST_QUEUE)), u64::from(BIKE + 0x674), u64::from(kind), u64::from(msg), u64::from(id), 0])
            }
            bike::Out::PutBack { place, keep } => (vt(ee, PUT_BACK_SLOT), vec![u64::from(BIKE), u64::from(place), u64::from(keep)]),
        };
        self.run(f, &ints, q, body);
    }
}

/// [`bike::air_state`] on the bike in RAM, with its callees the original's.
fn port_post(ee: &mut Ee, dt: F) {
    let body = ee.u32_at(BIKE + seed::BODY);
    let (wheels, record) = (the_wheels(ee), ee.u32_at(BIKE + seed::RECORD));
    let ram = InRam { ee, wheels, record, body, last: Vec::new() };
    let step = StepInRam { ram, rider: bike::Rider::default(), rider_last: Vec::new(), dt };
    let mut m = PostInRam { step, post: bike::Post::default(), post_last: Vec::new() };
    let (mut q, mut b) = m.read();
    bike::air_state(&mut q, &mut b, dt, &mut m);
    m.write(&q, &b);
}

/// A frame as [`PortStep`]'s, the bike's PostProcess the port's ([`port_post`]) as well.
struct PortPost<'a>(PortStep<'a>, u32);

impl Dynamics for PortPost<'_> {
    fn count(&self, l: List) -> i32 {
        self.0.count(l)
    }
    fn preforce(&mut self, i: i32, dt: F) {
        self.0.preforce(i, dt)
    }
    fn force(&mut self, i: i32, h: F) {
        self.0.force(i, h)
    }
    fn enabled(&self, b: i32) -> bool {
        self.0.enabled(b)
    }
    fn integrate(&mut self, b: i32, h: F) {
        self.0.integrate(b, h)
    }
    fn end_substep(&mut self, b: i32) {
        self.0.end_substep(b)
    }
    fn constrain(&mut self, b: i32, last: bool) {
        self.0.constrain(b, last)
    }
    fn post(&mut self, i: i32, dt: F) {
        if self.0.0.0.elt(List::Post, i) != BIKE_POST {
            return self.0.post(i, dt);
        }
        let x = &mut self.0.0.0;
        for (a, w) in seed::body_words(&x.body, x.at) {
            x.ee.set_u32(a, w);
        }
        port_post(x.ee, dt);
        x.body = seed::body(&|a| x.ee.u32_at(a), x.at);
        self.1 += 1;
    }
}

/// One sim frame as [`port_step_frame`] runs it, the world step [`PortPost`]'s. Answers how many
/// times the port's Force body and PostProcess ran.
fn port_post_frame(ee: &mut Ee, dt: F) -> (u32, u32) {
    let call = |ee: &mut Ee, f: u32, ints: &[u64], x: F| {
        ee.set(28, GP);
        ee.call(f, ints, &[x.0], BUDGET);
        ee.r(2)
    };
    assert_eq!(ee.u8_at(ee.u32_at(SKIP_FLAG) + 0x225) & 8, 0, "the frame is not skipped");
    for slot in [0x7c, 0x84] {
        for i in 0..ee.u32_at(SIM + 0x24) {
            let e = call(ee, LIST24_GET, &[u64::from(SIM + 0x24), u64::from(i)], F::ZERO) as u32;
            call(ee, ee.u32_at(ee.u32_at(e + 4) + slot), &[u64::from(e)], dt);
        }
    }
    let world = World { iterations: ee.u32_at(WORLD) as i32, rate: F(ee.f32_at(WORLD + 4)) };
    let at = ee.u32_at(BIKE + seed::BODY);
    let body = seed::body(&|a| ee.u32_at(a), at);
    let mut h = PortPost(PortStep(PortHead(Hybrid { ee, body, at, integrated: 0 }), 0), 0);
    world.step(dt, &mut h);
    let (body, steps, posts) = (h.0.0.0.body, h.0.1, h.1);
    for (a, w) in seed::body_words(&body, at) {
        ee.set_u32(a, w);
    }
    call(ee, TAIL, &[u64::from(SIM)], dt);
    let mut i = 0;
    while i < ee.u32_at(SIM + 0x3c) {
        let e = call(ee, LIST3C_GET, &[u64::from(SIM + 0x3c), u64::from(i)], F::ZERO) as u32;
        if call(ee, ee.u32_at(ee.u32_at(e) + 8), &[u64::from(e)], dt) != 0 {
            call(ee, LIST3C_REMOVE, &[u64::from(SIM + 0x3c), u64::from(i)], F::ZERO);
        } else {
            i += 1;
        }
    }
    (steps, posts)
}

/// [`replay`] with every frame [`port_post_frame`].
fn replay_port_post(name: &str) -> Option<Vec<bool>> {
    let mut ee = hooked(image()?);
    let rows = rows(name)?;
    let mut out = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        poke(&mut ee, row, &READER);
        assert_eq!(port_post_frame(&mut ee, F(f32::from_bits(DT))), (4, 1), "the bike's Force body runs every substep, its PostProcess once");
        let d = diff(&ee, row);
        assert!(
            d.is_empty(),
            "{name} row {r} (vsync {}), the port's head, Force body and PostProcess: {} words differ\n  {}",
            row["vsync"],
            d.len(),
            d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  ")
        );
        let w = wrecked(|o| ee.u32_at(BIKE + o));
        assert_eq!(w, row_wrecked(row), "{name} row {r}: the wreck state");
        out.push(w);
        poke(&mut ee, row, &SPOT);
    }
    Some(out)
}

/// **C15's ride, the bike's PreProcess, Force body and PostProcess the port's**: every row of ride-2
/// word for word.
#[test]
fn ride_replays_bit_exact_on_the_ports_post_process() {
    let Some(w) = replay_port_post("ride-2") else { return };
    assert_eq!(w.len(), 459);
    assert!(w.iter().all(|w| !w), "the ride never wrecks");
}

/// **C15's flip, the bike's PreProcess, Force body and PostProcess the port's**: wrecked on row 125
/// and put back on row 193, every row word for word.
#[test]
fn flip_wrecks_and_is_put_back_bit_exact_on_the_ports_post_process() {
    let Some(w) = replay_port_post("flip-1") else { return };
    assert_eq!(w.len(), 465);
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [125, 193], "wrecked on row 125, put back on row 193");
}

/// A call the PostProcess makes: one of the quad's steps or one of the bike's own.
#[derive(Clone, Copy, Debug, PartialEq)]
enum PostCall {
    Step(Call),
    Out(bike::Out),
}

/// What the node's position answers in the oracle, both sides alike.
const NODE_SAYS: Vf = Vf::new(812.5, -36.25, 1904.75, 1.0);

/// The port's side of the PostProcess oracle: the calls logged, the rider and the post words held.
struct PostLogged {
    calls: Vec<PostCall>,
    rider: bike::Rider,
    post: bike::Post,
}

impl Moto for PostLogged {
    fn call(&mut self, c: Call, q: &mut Quad, _: &mut quad::Body) {
        if c == Call::NodePosition {
            q.node_pos = NODE_SAYS;
        }
        self.calls.push(PostCall::Step(c));
    }
}

impl bike::Rides for PostLogged {
    fn rider(&mut self) -> &mut bike::Rider {
        &mut self.rider
    }
    fn noise(&mut self, _: F) -> F {
        panic!("the bike's PostProcess makes no noise")
    }
}

impl bike::Posts for PostLogged {
    fn post(&mut self) -> &mut bike::Post {
        &mut self.post
    }
    fn out(&mut self, c: bike::Out, _: &mut Quad, _: &mut quad::Body) {
        self.calls.push(PostCall::Out(c));
    }
}

/// **The bike's PostProcess is the original's**: 12 000 bikes, each state 4's with the PostProcess's
/// inputs drawn at random (the guards, the balance state, the no-hander's keys, both wheels' contact,
/// ray and sweep words, normals and contact velocities, the rear's donut, the signed speed and lean,
/// the body's quaternion, velocity, spin, position, centre of gravity and on word, the airborne and
/// hang time, the flip, spin and authority, the flag byte, the world's floor and box, the post id),
/// run through SLUS-21104's PostProcess and through [`bike::air_state`] from the same words. Every
/// word of the bike, its wheels, its body and the record comes out the same, and the calls are made in
/// the same order with the same arguments. The original's callees outside the bike's own code (the
/// vtable's, the message post, the node's placement, position and bounds) are hooked to log and do
/// nothing (the position answers [`NODE_SAYS`]); the no-hander, the in-world test and the math they
/// call are its own.
#[test]
#[allow(clippy::float_arithmetic)]
fn the_bikes_post_process_is_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    assert_eq!(vt(&ee, 0x120), IN_WORLD, "the bike's in-world slot");
    assert_eq!([0, 4, 8, 12].map(|o| ee.f32_at(FRAME_AT + o)), [0.0, 0.0, 0.0, 1.0], "the no-hander's frame origin");
    let [front, rear] = the_wheels(&ee);
    let body = ee.u32_at(BIKE + seed::BODY);
    let node = ee.u32_at(BIKE + 0xe4);
    let record = ee.u32_at(BIKE + seed::RECORD);
    let world = ee.u32_at(BIKE + 0xe8);
    let log: Rc<RefCell<Vec<PostCall>>> = Rc::default();
    let mut hooked_at = Vec::new();
    for (slot, c) in POST_SLOTS {
        let (l, at) = (log.clone(), vt(&ee, slot));
        hooked_at.push(at);
        ee.hooks.insert(at, Box::new(move |ee: &mut Ee| {
            assert_eq!(ee.r(4) as u32, BIKE, "{c:?} handed something other than the bike");
            l.borrow_mut().push(PostCall::Step(c));
        }));
    }
    let l = log.clone();
    hooked_at.push(NODE_BOUNDS);
    ee.hooks.insert(NODE_BOUNDS, Box::new(move |ee: &mut Ee| {
        assert_eq!(ee.r(4) as u32, node, "the node's bounds handed another node");
        l.borrow_mut().push(PostCall::Step(Call::NodeBounds));
    }));
    let l = log.clone();
    hooked_at.push(NODE_POSITION);
    ee.hooks.insert(NODE_POSITION, Box::new(move |ee: &mut Ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5), ee.r(6) as u32), (node, 0, BIKE + 0xbd0), "the node position's arguments");
        for (k, w) in NODE_SAYS.to_bits().into_iter().enumerate() {
            ee.set_u32(BIKE + 0xbd0 + 4 * k as u32, w);
        }
        l.borrow_mut().push(PostCall::Step(Call::NodePosition));
    }));
    let l = log.clone();
    hooked_at.push(NODE_PLACE);
    ee.hooks.insert(NODE_PLACE, Box::new(move |ee: &mut Ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5), ee.r(6) as u32), (node, 0, body + 0x190), "the node placement's arguments");
        let v = |a: u64| seed::Words::v(&|x| ee.u32_at(x), a as u32);
        l.borrow_mut().push(PostCall::Out(bike::Out::Place { orientation: v(ee.r(6)), at: v(ee.r(7)) }));
    }));
    let l = log.clone();
    hooked_at.push(POST_MESSAGE);
    ee.hooks.insert(POST_MESSAGE, Box::new(move |ee: &mut Ee| {
        let (kind, msg) = bike::OUT_OF_WORLD;
        let want = (u64::from(ee.u32_at(POST_QUEUE)), u64::from(BIKE + 0x674), u64::from(kind), u64::from(msg), 0);
        assert_eq!((ee.r(4), ee.r(5), ee.r(6), ee.r(7), ee.r(9)), want, "the message's arguments");
        l.borrow_mut().push(PostCall::Out(bike::Out::OutOfWorld(ee.r(8) as u32)));
    }));
    let (l, put_back) = (log.clone(), vt(&ee, PUT_BACK_SLOT));
    hooked_at.push(put_back);
    ee.hooks.insert(put_back, Box::new(move |ee: &mut Ee| {
        assert_eq!(ee.r(4) as u32, BIKE, "the put-back handed something other than the bike");
        l.borrow_mut().push(PostCall::Out(bike::Out::PutBack { place: ee.r(5) != 0, keep: ee.r(6) != 0 }));
    }));
    let mut distinct = hooked_at.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), hooked_at.len(), "each hooked callee is its own function: {hooked_at:#x?}");

    let mut regions = head_regions(&ee);
    if !regions.iter().any(|&(at, len)| (at..at + len).contains(&(record + 0x24))) {
        regions.push((record + 0x24, 4));
    }
    let state = ee.u32_at(BIKE + 0xbc4);
    let kind = ee.u32_at(state + 0x28);
    let image_words = snap_regions(&ee, &regions);
    let world_words = [ee.u32_at(world + 0x98), ee.u32_at(world + 0xa8)];
    let area = ee.scratch(8);
    let mut g = Gen(0x0031_b003);
    let mut t = BTreeMap::new();
    for k in 0..12_000 {
        restore_regions(&mut ee, &regions, &image_words);
        let (b, f) = (|ee: &mut Ee, a: u32, v: bool| ee.set_u32(a, u32::from(v)), |ee: &mut Ee, a: u32, v: f32| ee.set_f32(a, v));
        let v = |ee: &mut Ee, g: &mut Gen, a: u32, r: f32| {
            for i in 0..3 {
                let x = g.near(0.0, -r, r);
                ee.set_f32(a + 4 * i, x);
            }
        };
        // The guards: no state, states 3 and 4, others; the wreck reason; the balance words.
        match g.bits() % 12 {
            0 => ee.set_u32(BIKE + 0xbc4, 0),
            1 => ee.set_u32(state + 0x28, 3),
            2 => ee.set_u32(state + 0x28, 4),
            n => ee.set_u32(state + 0x28, [kind, kind, kind, kind, 0, 1, 2, 5, 6][n as usize - 3]),
        }
        ee.set_u32(BIKE + 0xc80, if g.one_in(6) { g.bits() | 1 } else { 0 });
        let bal = g.bits();
        ee.set_u32(BIKE + 0x1524, bal);
        ee.set_u32(BIKE + 0x152c, if g.one_in(4) { bal } else { g.bits() });
        // The balance state the PostProcess clears, and the two words the put out of the world does.
        for o in [0x14f8, 0x150c, 0x1510, 0x1518, 0x868, 0xc1c, 0xd20] {
            ee.set_u32(BIKE + o, g.bits());
        }
        // The no-hander's keys: the stand-upright speeds (tied a sixth of the time), the roll speed
        // (zero a sixth), command, gain and donut scale.
        let s1 = g.f(0.0, 20.0);
        f(&mut ee, BIKE + 0x1374, s1);
        f(&mut ee, BIKE + 0x1378, if g.one_in(6) { s1 } else { s1 + g.f(0.5, 30.0) });
        f(&mut ee, BIKE + 0x1368, if g.one_in(6) { 0.0 } else { g.f(1.0, 60.0) });
        f(&mut ee, BIKE + 0x1360, g.f(0.0, 1.0));
        f(&mut ee, BIKE + 0x1370, g.f(0.0, 20.0));
        f(&mut ee, BIKE + 0x1238, g.f(0.0, 3.0));
        // The wheels: contact, ray hit and sweep (as words), normal (its `y` at the sweep's 0.17 now
        // and then), contact velocity; the rear's donut.
        for w in [front, rear] {
            b(&mut ee, w + 0xdc, g.bits() % 4 != 0);
            ee.set_u32(w + 0xd0, if g.one_in(2) { 0 } else { [1, g.bits()][(g.bits() % 2) as usize] });
            ee.set_u32(w + 0xd4, if g.one_in(3) { 0 } else { 1 });
            let mut n = [g.f(-0.9, 0.9), g.f(-0.3, 1.0), g.f(-0.9, 0.9)];
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            n = n.map(|x| x / l);
            if g.one_in(8) {
                n[1] = f32::from_bits(0x3e2e_147b);
            }
            for i in 0..3 {
                f(&mut ee, w + 0x130 + 4 * i as u32, n[i]);
            }
            v(&mut ee, &mut g, w + 0x190, 40.0);
        }
        f(&mut ee, rear + 0x38, if g.one_in(2) { g.near(0.0, -1.0, 0.0) } else { g.f(0.0, 1.5) });
        // The signed speed and the lean.
        let speed = match g.bits() % 6 {
            0 => *[0.0, -0.0, 5.0, 15.0, -10.0].get((g.bits() % 5) as usize).unwrap(),
            1 => g.f(-12.0, 12.0),
            _ => g.f(-10.0, 50.0),
        };
        f(&mut ee, BIKE + 0xc38, speed);
        f(&mut ee, BIKE + 0x12e8, g.near(0.0, -1.0, 1.0));
        // The body: on or off, its quaternion (level now and then), velocity (under the crossup's
        // 10 ft/s a third of the time), spin, position near the world's box, centre of gravity.
        ee.set_u32(body, u32::from(!g.one_in(8)));
        let qt = if g.one_in(8) {
            [0.0, 0.0, 0.0, 1.0]
        } else {
            let qt = [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0)];
            let l = qt.iter().map(|x| x * x).sum::<f32>().sqrt();
            qt.map(|x| x / l)
        };
        for i in 0..4 {
            f(&mut ee, body + 0x190 + 4 * i as u32, qt[i]);
        }
        let r = if g.one_in(3) { 6.0 } else { 60.0 };
        v(&mut ee, &mut g, body + 0x80, r);
        v(&mut ee, &mut g, body + 0x1d0, 5.0);
        v(&mut ee, &mut g, body + 0x70, 150.0);
        let r = if g.one_in(4) { 0.0 } else { 1.0 };
        v(&mut ee, &mut g, body + 0x1a0, r);
        // The air: airborne before, hang time, flipping, spinning, the authority and its time.
        b(&mut ee, BIKE + 0xde0, g.one_in(2));
        f(&mut ee, BIKE + 0x1190, g.f(0.0, 3.0));
        b(&mut ee, BIKE + 0x1198, g.one_in(5));
        b(&mut ee, BIKE + 0x11d4, g.one_in(5));
        f(&mut ee, BIKE + 0x1188, if g.one_in(2) { g.near(0.0, -1.0, 0.0) } else { g.f(0.0, 2.0) });
        f(&mut ee, BIKE + 0x118c, if g.one_in(4) { 0.0 } else { g.f(0.1, 2.0) });
        // The world test: the flag byte's box, floor, bounds and clear bits; the floor; the box or none.
        let flags = [0x00, 0x01, 0x40, 0x41][(g.bits() % 4) as usize] | (g.bits() & 0x06) as u8 | (g.bits() & 0xb8) as u8;
        let w44 = ee.u32_at(BIKE + 0x44);
        ee.set_u32(BIKE + 0x44, (w44 & !0xff00) | u32::from(flags) << 8);
        f(&mut ee, world + 0x98, g.near(0.0, -150.0, 150.0));
        if g.one_in(5) {
            ee.set_u32(world + 0xa8, 0);
        } else {
            ee.set_u32(world + 0xa8, area);
            for (o, sign) in [(0x30, -1.0), (0x40, 1.0)] {
                for i in 0..4 {
                    let x = sign * g.f(20.0, 140.0);
                    ee.set_f32(area + o + 4 * i, x);
                }
            }
        }
        let dt = if g.one_in(2) { f32::from_bits(DT) } else { g.f(0.002, 0.05) };
        let before = snap_regions(&ee, &regions);

        let r = |a| ee.u32_at(a);
        let (mut q, mut qb) = (bike::read_quad(&r, BIKE), quad_body(&ee, body));
        let rider = bike::read_rider(&r, BIKE, body);
        let post = bike::read_post(&r, BIKE, body);
        let (q0, qb0) = (q, qb);
        let mut m = PostLogged { calls: Vec::new(), rider, post };
        bike::air_state(&mut q, &mut qb, F(dt), &mut m);

        if !q0.active {
            tally(&mut t, "off");
        } else {
            if q0.control_off {
                tally(&mut t, "wrecked");
            } else if q0.balancing {
                tally(&mut t, "balancing");
            } else {
                let grounded = |w: &quad::Wheel, k: usize| w.touching && (post.ray_hit[k] || (post.swept[k] && !(w.normal.y() <= 0.17)));
                tally(&mut t, if grounded(&q0.wheels[bike::FRONT], 0) {
                    "no hands, front"
                } else if grounded(&q0.wheels[bike::REAR], 1) {
                    "no hands, rear"
                } else {
                    "no hands, no ground"
                });
                if qb.orientation.to_bits() != qb0.orientation.to_bits() {
                    tally(&mut t, "rolled");
                }
                if post.donut > F::ZERO {
                    tally(&mut t, "donut");
                }
            }
            if !q0.airborne && q.airborne {
                tally(&mut t, "took off");
            }
            tally(&mut t, if q.airborne { "in the air" } else if q.all_down { "both down" } else { "one down" });
            if !q0.flipping && !q0.spinning && q.speed_len > F(10.0) {
                tally(&mut t, "crossup");
            }
            if q0.authority > F::ZERO && q0.authority_time != F::ZERO {
                tally(&mut t, "crossup ramped");
            }
            if q.tipped {
                tally(&mut t, "tipped");
            }
            let calls = &m.calls;
            if calls.iter().any(|c| matches!(c, PostCall::Out(bike::Out::OutOfWorld(_)))) {
                tally(&mut t, "out of the world");
            }
            if !post.body_on {
                tally(&mut t, "body off");
            } else if calls.iter().any(|c| matches!(c, PostCall::Out(bike::Out::Place { .. }))) {
                tally(&mut t, "placed");
            } else {
                tally(&mut t, "reset");
            }
            if calls.contains(&PostCall::Step(Call::NodeBounds)) {
                tally(&mut t, "node bounds");
            }
            match (post.flags() & 0x41, post.bounds.area) {
                (0, _) => tally(&mut t, "anywhere"),
                (f, None) if f & 1 != 0 => tally(&mut t, "no box"),
                (f, _) if f & 1 != 0 => tally(&mut t, "boxed"),
                _ => tally(&mut t, "floored"),
            }
        }

        let mut want = before.clone();
        let index = |a: u32| {
            let mut i = 0;
            for &(at, len) in &regions {
                if (at..at + len).contains(&a) {
                    return i + ((a - at) / 4) as usize;
                }
                i += (len / 4) as usize;
            }
            panic!("{a:#x} is in none of the regions")
        };
        let words = |q: &Quad, b: &quad::Body, r: &bike::Rider, p: &bike::Post| {
            let mut w = bike::quad_words(q, BIKE, [front, rear], record);
            w.extend(quad_body_words(b, body));
            w.extend(bike::rider_words(r, BIKE, body));
            w.extend(bike::post_words(p, BIKE));
            w
        };
        for ((a, w), (_, w0)) in words(&q, &qb, &m.rider, &m.post).into_iter().zip(words(&q0, &qb0, &rider, &post)) {
            if w != w0 {
                want[index(a)] = w;
            }
        }

        log.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(AIR_STATE, &[u64::from(BIKE)], &[dt], 1_000_000);
        let got = snap_regions(&ee, &regions);
        if let Some(i) = (0..got.len()).find(|&i| got[i] != want[i]) {
            let (mut at, mut base) = (i as u32 * 4, 0);
            for &(b, len) in &regions {
                if at < len {
                    base = b;
                    break;
                }
                at -= len;
            }
            panic!(
                "bike {k}: {:#x}+{at:#x} is {:#010x} ({}) in the original and {:#010x} ({}) in the port (was {:#010x})\n{q0:#?}\n{post:#?}",
                base,
                got[i],
                f32::from_bits(got[i]),
                want[i],
                f32::from_bits(want[i]),
                before[i],
            );
        }
        assert_eq!(*log.borrow(), m.calls, "bike {k}: the calls\n{q0:#?}\n{post:#?}");
        ee.set_u32(BIKE + 0xbc4, state);
        ee.set_u32(state + 0x28, kind);
        ee.set_u32(world + 0x98, world_words[0]);
        ee.set_u32(world + 0xa8, world_words[1]);
    }
    for want in [
        "off",
        "wrecked",
        "balancing",
        "no hands, front",
        "no hands, rear",
        "no hands, no ground",
        "rolled",
        "donut",
        "took off",
        "in the air",
        "both down",
        "one down",
        "crossup",
        "crossup ramped",
        "tipped",
        "out of the world",
        "body off",
        "placed",
        "reset",
        "node bounds",
        "anywhere",
        "no box",
        "boxed",
        "floored",
    ] {
        let n = t.get(want).copied().unwrap_or(0);
        assert!(n >= 30, "only {n} bike(s) went down {want:?}: {t:?}");
    }
}

// --- F31b4a: the collision record's pass ---------------------------------------------------------

use game_sim::contact_search::Ground;
use game_sim::handling::ground;
use game_sim::handling::record::{self, Collides, Hit, Record, Struck, Swept};
use game_sim::track;

/// The record's two virtuals the sim's `+0x24` list runs (the record's vt `+0x7c` and `+0x84`).
const RECORD_CLEAR: u32 = 0x0020_4c30;
const RECORD_PASS: u32 = 0x0020_51f0;
/// The record's outside: the node's direction turn, position set and point placement; the sphere
/// probe's segment set; a probe's placement at its node; the near-field and the global search; the
/// terrain query.
const NODE_ROTATE: u32 = 0x0019_5230;
const NODE_SET: u32 = 0x0019_4b10;
const NODE_TIP: u32 = 0x0019_52a0;
const PROBE_SET: u32 = 0x001b_9290;
const PROBE_PLACE: u32 = 0x0011_4360;
const SEARCH_NEAR: u32 = 0x0011_51c0;
/// F31b5l: the bike's kind clear (vt `+0x39c`) and its show (vt `+0x398`), and the spatial
/// database's insert the show calls.
const KIND_CLEAR_FN: u32 = 0x004e_1910;
const SHOW_FN: u32 = 0x004e_1870;
const SPACE_INSERT: u32 = 0x0011_43e0;
const SEARCH_ALL: u32 = 0x0011_48e0;
const TERRAIN_QUERY: u32 = 0x0048_38f0;
/// The body's damping setters, its vtable's (`+0x110`) `+0x20` and `+0x68`.
const BODY_DAMP: [u32; 2] = [0x20, 0x68];

fn vf_at(ee: &Ee, a: u32) -> Vf {
    seed::Words::v(&|x| ee.u32_at(x), a)
}


fn set_vf(ee: &mut Ee, a: u32, v: Vf) {
    for (k, w) in v.to_bits().into_iter().enumerate() {
        ee.set_u32(a + 4 * k as u32, w);
    }
}

/// The record at `at` in the interpreter's RAM as [`record`]'s port sees its outside: each call is the
/// original's (scratch slots 0..6 hold the arguments), and the other records a world ray hands a
/// contact to are read on first use and held. With `terrain`, the terrain query is the port's
/// [`ground::under`]; without, the original's.
struct RecordInRam<'a, 't> {
    ee: &'a mut Ee,
    at: u32,
    terrain: Option<&'t game_sim::atv_ground::Terrain<'t>>,
    others: BTreeMap<u32, (Record, Record)>,
    queries: u32,
}

impl RecordInRam<'_, '_> {
    fn run(&mut self, f: u32, ints: &[u64]) {
        self.ee.set(28, GP);
        self.ee.call(f, ints, &[], BUDGET);
    }
    fn put(&mut self, k: u32, v: Vf) -> u64 {
        let a = self.ee.scratch(k);
        set_vf(self.ee, a, v);
        u64::from(a)
    }
    fn word(&self, o: u32) -> u32 {
        self.ee.u32_at(self.at + o)
    }
    /// What the object `obj` is to the record: its record, its mover when a mover's kind is 2, its
    /// category. Read whatever `obj` is, as the world rays do.
    fn struck(&self, obj: u32) -> Struck {
        let ee = &*self.ee;
        let m = ee.u32_at(obj + 0x8c);
        let mover = if m != 0 && ee.u32_at(m + 0x1a0) == record::MOVER_KIND { m } else { 0 };
        Struck { object: obj, other: ee.u32_at(obj + 0x90), mover, category: ee.u32_at(obj + 0x94) }
    }
    /// The probe at `probe` searched as the record searches: near-field through `+0xfc`, or global.
    fn search_with(&mut self, probe: u32, nearby: bool) {
        let list = self.word(0xfc);
        if nearby {
            self.run(SEARCH_NEAR, &[u64::from(probe), u64::from(list), 1]);
        } else {
            self.run(SEARCH_ALL, &[u64::from(probe), 0]);
        }
    }
    /// The words of the record (read as `read`) and of the others back into RAM where they changed
    /// (a flag word that is neither 0 nor 1 stays as RAM has it) and the last segment, and the body at
    /// `body`.
    fn finish(self, read: &Record, rec: &Record, body: Option<(&quad::Body, u32)>) {
        let RecordInRam { ee, at, others, .. } = self;
        let mut words = Vec::new();
        for (id, was, now) in std::iter::once((at, read, rec)).chain(others.iter().map(|(&id, (was, now))| (id, was, now))) {
            let list = ee.u32_at(id + 0xf8);
            let old: HashMap<u32, u32> = record::words(was, id, list).into_iter().collect();
            // The segment is in RAM as the glue last put it for the probe, so it goes back whole.
            let segment = id + 0xc0..id + 0xe0;
            words.extend(record::words(now, id, list).into_iter().filter(|(a, w)| old.get(a) != Some(w) || segment.contains(a)));
        }
        if let Some((b, a)) = body {
            words.extend(quad_body_words(b, a));
        }
        for (a, w) in words {
            ee.set_u32(a, w);
        }
    }
}

impl Collides for RecordInRam<'_, '_> {
    fn damp(&mut self, linear: Vf, angular: Vf) {
        let body = self.word(0xe4);
        for (slot, v) in BODY_DAMP.into_iter().zip([linear, angular]) {
            let p = self.put(0, v);
            let f = self.ee.u32_at(self.ee.u32_at(body + 0x110) + slot);
            self.run(f, &[u64::from(body), p]);
        }
    }
    fn node_rotate(&mut self, v: Vf) -> Vf {
        let (out, p) = (self.ee.scratch(1), self.put(0, v));
        let node = self.word(0xe8);
        self.run(NODE_ROTATE, &[u64::from(out), u64::from(node), p]);
        vf_at(self.ee, out)
    }
    fn node_position(&mut self) -> Vf {
        let node = self.word(0xe8);
        self.run(NODE_POSITION, &[u64::from(node), 0, u64::from(self.at + 0xd0)]);
        vf_at(self.ee, self.at + 0xd0)
    }
    fn node_place(&mut self, v: Vf) -> Vf {
        let (out, p) = (self.ee.scratch(1), self.put(0, v));
        let node = self.word(0xe8);
        self.run(NODE_TIP, &[u64::from(out), u64::from(node), p]);
        vf_at(self.ee, out)
    }
    fn node_move(&mut self, at: Vf) {
        let p = self.put(0, at);
        let node = self.word(0xe8);
        self.run(NODE_SET, &[u64::from(node), 0, p]);
    }
    fn node_up(&self) -> [F; 3] {
        let node = self.word(0xe8);
        [0x74, 0x84, 0x94].map(|o| F(self.ee.f32_at(node + o)))
    }
    fn chassis_velocity(&self) -> Vf {
        vf_at(self.ee, self.word(0xe0) + 0x250)
    }
    fn sweep(&mut self, from: Vf, to: Vf, nearby: bool) -> Option<Swept> {
        set_vf(self.ee, self.at + 0xc0, from);
        set_vf(self.ee, self.at + 0xd0, to);
        let probe = self.word(0x204);
        self.run(PROBE_SET, &[u64::from(probe), u64::from(self.at + 0xc0), 0]);
        self.run(PROBE_PLACE, &[u64::from(probe)]);
        self.search_with(probe, nearby);
        if self.ee.u8_at(probe + 0x126) & 0x20 == 0 {
            return None;
        }
        let hits = self.ee.u32_at(probe + 0x7c);
        let obj = self.ee.u32_at(self.ee.u32_at(hits + 0x9c));
        let struck = if obj == 0 { Struck::default() } else { self.struck(obj) };
        let normal = vf_at(self.ee, self.ee.u32_at(hits + 0x94));
        let point = vf_at(self.ee, self.ee.u32_at(hits + 0x98));
        Some(Swept { normal, point, struck })
    }
    fn place_part(&mut self) {
        let part = self.word(0x200);
        self.run(PROBE_PLACE, &[u64::from(part)]);
    }
    fn part_rays(&self) -> Vec<[Vf; 2]> {
        let part = self.word(0x200);
        let (n, rays) = (self.ee.u32_at(part + 0x178) as i32, self.ee.u32_at(part + 0x180));
        (0..n.max(0) as u32).map(|i| [vf_at(self.ee, rays + 0x20 * i), vf_at(self.ee, rays + 0x20 * i + 0x10)]).collect()
    }
    fn search(&mut self, nearby: bool) -> Vec<Hit> {
        let part = self.word(0x200);
        self.search_with(part, nearby);
        let h = self.ee.u32_at(part + 0x7c);
        let n = self.ee.u32_at(h + 0x8c) as i32;
        let [rays, normals, points, objects] = [0x90, 0x94, 0x98, 0x9c].map(|o| self.ee.u32_at(h + o));
        (0..n.max(0) as u32)
            .map(|i| Hit {
                from: vf_at(self.ee, rays + 0x20 * i),
                to: vf_at(self.ee, rays + 0x20 * i + 0x10),
                normal: vf_at(self.ee, normals + 0x10 * i),
                point: vf_at(self.ee, points + 0x10 * i),
                struck: self.struck(self.ee.u32_at(objects + 4 * i)),
            })
            .collect()
    }
    fn terrain_box(&self) -> [Vf; 2] {
        let t = self.word(0xec);
        [vf_at(self.ee, t + 0x30), vf_at(self.ee, t + 0x40)]
    }
    fn under(&mut self, g: &mut Ground) {
        self.queries += 1;
        if let Some(t) = self.terrain {
            return ground::under(t, g);
        }
        let buf = self.ee.scratch(2);
        set_vf(self.ee, buf, g.point);
        set_vf(self.ee, buf + 0x10, g.normal);
        self.ee.set_u32(buf + 0x20, u32::from_le_bytes(g.surfaces));
        set_vf(self.ee, buf + 0x30, g.weights);
        let terrain = self.word(0xec);
        self.run(TERRAIN_QUERY, &[u64::from(terrain), u64::from(buf)]);
        g.point = vf_at(self.ee, buf);
        g.normal = vf_at(self.ee, buf + 0x10);
        g.surfaces = self.ee.u32_at(buf + 0x20).to_le_bytes();
        g.weights = vf_at(self.ee, buf + 0x30);
    }
    fn velocity(&mut self, s: &Struck) -> Vf {
        if s.other != 0 {
            let b = self.ee.u32_at(s.other + 0xe4);
            return if b != 0 { vf_at(self.ee, b + 0x80) } else { vf_at(self.ee, self.ee.u32_at(s.other + 0xe0) + 0x250) };
        }
        let out = self.ee.scratch(1);
        let f = self.ee.u32_at(self.ee.u32_at(s.mover + 4) + 0xc0);
        self.run(f, &[u64::from(out), u64::from(s.mover)]);
        vf_at(self.ee, out)
    }
    fn other(&mut self, id: u32) -> &mut Record {
        let ee = &*self.ee;
        let o = self.others.entry(id).or_insert_with(|| {
            let r = record::read(&|a| ee.u32_at(a), id);
            (r.clone(), r)
        });
        &mut o.1
    }
}

/// The record at `at`'s virtual `slot` (`+0x7c` or `+0x84`) as the port's: [`record::clear`] or
/// [`record::pass`] on the record in RAM, its body the quad's off `+0xe4`. Answers how many terrain
/// queries it made.
fn port_record(ee: &mut Ee, at: u32, slot: u32, dt: F, terrain: Option<&game_sim::atv_ground::Terrain<'_>>) -> u32 {
    let mut rec = record::read(&|a| ee.u32_at(a), at);
    let read = rec.clone();
    let body_at = ee.u32_at(at + 0xe4);
    let mut body = (body_at != 0).then(|| quad_body(ee, body_at));
    let mut w = RecordInRam { ee, at, terrain, others: BTreeMap::new(), queries: 0 };
    if slot == 0x7c {
        record::clear(&mut rec, &mut w);
    } else {
        record::pass(&mut rec, at, body.as_mut(), dt, &mut w);
    }
    let queries = w.queries;
    w.finish(&read, &rec, body.as_ref().map(|b| (b, body_at)));
    queries
}

/// Nat05's terrain map off its `.res`, for the port's terrain query; `None` (with a skip) when it is
/// not there.
fn nat05_trn() -> Option<Vec<u8>> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let res = kit.ctx("mxvatv-ps2/nat05").join("nat05.res");
    if !res.exists() {
        skip(&format!("{} is not there: Nat05's nat05.res off MX vs. ATV's disc (Env/Nat05)", res.display()));
        return None;
    }
    ground::terrain_file(&std::fs::read(&res).unwrap(), "Nat05")
}

/// One sim frame as [`port_post_frame`] runs it, the bike's collision record's two virtuals the
/// port's ([`port_record`]) and the tail's walk and the bike's vehicle update too ([`port_tail`]). Answers the Force body and PostProcess counts, how many records the port
/// ran, and how many terrain queries.
fn port_record_frame(ee: &mut Ee, dt: F, map: &track::Terrain<'_>) -> (u32, u32, u32, u32) {
    let call = |ee: &mut Ee, f: u32, ints: &[u64], x: F| {
        ee.set(28, GP);
        ee.call(f, ints, &[x.0], BUDGET);
        ee.r(2)
    };
    assert_eq!(ee.u8_at(ee.u32_at(SKIP_FLAG) + 0x225) & 8, 0, "the frame is not skipped");
    let mine = ee.u32_at(BIKE + seed::RECORD);
    let (mut ported, mut queries) = (0, 0);
    for (slot, original) in [(0x7c, RECORD_CLEAR), (0x84, RECORD_PASS)] {
        for i in 0..ee.u32_at(SIM + 0x24) {
            let e = call(ee, LIST24_GET, &[u64::from(SIM + 0x24), u64::from(i)], F::ZERO) as u32;
            let f = ee.u32_at(ee.u32_at(e + 4) + slot);
            if e == mine {
                assert_eq!(f, original, "the bike's record's vt +{slot:#x}");
                let t = ee.u32_at(e + 0xec);
                let terrain = (t != 0).then(|| ground::read_terrain(&|a| ee.u32_at(a), t, Some(map)));
                queries += port_record(ee, e, slot, dt, terrain.as_ref());
                ported += 1;
            } else {
                call(ee, f, &[u64::from(e)], dt);
            }
        }
    }
    let world = World { iterations: ee.u32_at(WORLD) as i32, rate: F(ee.f32_at(WORLD + 4)) };
    let at = ee.u32_at(BIKE + seed::BODY);
    let body = seed::body(&|a| ee.u32_at(a), at);
    let mut h = PortPost(PortStep(PortHead(Hybrid { ee, body, at, integrated: 0 }), 0), 0);
    world.step(dt, &mut h);
    let (body, steps, posts) = (h.0.0.0.body, h.0.1, h.1);
    for (a, w) in seed::body_words(&body, at) {
        ee.set_u32(a, w);
    }
    let (bikes, others) = port_tail(ee, dt);
    assert_eq!((bikes, others.len()), (1, 3), "the tail steps the bike once, its two other leaves and the second's rider: {others:#x?}");
    let mut i = 0;
    while i < ee.u32_at(SIM + 0x3c) {
        let e = call(ee, LIST3C_GET, &[u64::from(SIM + 0x3c), u64::from(i)], F::ZERO) as u32;
        if call(ee, ee.u32_at(ee.u32_at(e) + 8), &[u64::from(e)], dt) != 0 {
            call(ee, LIST3C_REMOVE, &[u64::from(SIM + 0x3c), u64::from(i)], F::ZERO);
        } else {
            i += 1;
        }
    }
    (steps, posts, ported, queries)
}

/// [`replay`] with every frame [`port_record_frame`]. Answers the wreck state per row and the terrain
/// queries the port's record made.
fn replay_port_record(name: &str) -> Option<(Vec<bool>, u32)> {
    let trn = nat05_trn()?;
    let map = track::Terrain::parse(&trn).unwrap();
    let mut ee = hooked(image()?);
    let rows = rows(name)?;
    let (mut out, mut queries) = (Vec::new(), 0);
    for (r, row) in rows.iter().enumerate() {
        poke(&mut ee, row, &READER);
        let (steps, posts, ported, q) = port_record_frame(&mut ee, F(f32::from_bits(DT)), &map);
        assert_eq!((steps, posts, ported), (4, 1, 2), "the Force body every substep, the PostProcess once, the record's two virtuals");
        queries += q;
        let d = diff(&ee, row);
        assert!(
            d.is_empty(),
            "{name} row {r} (vsync {}), the port's record pass, head, Force body and PostProcess: {} words differ\n  {}",
            row["vsync"],
            d.len(),
            d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  ")
        );
        let w = wrecked(|o| ee.u32_at(BIKE + o));
        assert_eq!(w, row_wrecked(row), "{name} row {r}: the wreck state");
        out.push(w);
        poke(&mut ee, row, &SPOT);
    }
    Some((out, queries))
}

/// **C15's ride, the bike's collision record pass, PreProcess, Force body and PostProcess the
/// port's**: every row of ride-2 word for word.
#[test]
fn ride_replays_bit_exact_on_the_ports_record_pass() {
    let Some((w, queries)) = replay_port_record("ride-2") else { return };
    assert_eq!(w.len(), 459);
    assert!(w.iter().all(|w| !w), "the ride never wrecks");
    eprintln!("ride-2: {queries} terrain queries by the record");
}

/// **C15's flip, the record pass the port's as well**: wrecked on row 125 and put back on row 193,
/// every row word for word.
#[test]
fn flip_wrecks_and_is_put_back_bit_exact_on_the_ports_record_pass() {
    let Some((w, queries)) = replay_port_record("flip-1") else { return };
    assert_eq!(w.len(), 465);
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [125, 193], "wrecked on row 125, put back on row 193");
    eprintln!("flip-1: {queries} terrain queries by the record");
}

/// A call the record makes outside itself, as the oracle's hooks log it: the arguments' words.
#[derive(Clone, Debug, PartialEq)]
enum RecordCall {
    Damp(u32, [u32; 4]),
    Rotate([u32; 4]),
    Position,
    Move([u32; 4]),
    Tip([u32; 4]),
    Set([u32; 4], [u32; 4]),
    Place(u32),
    Search(u32, bool),
    Mover(u32),
    Query([u32; 4]),
}

/// The mover's velocity virtual in the oracle: no code of the image's, only the hook's address.
const FAKE_MOVER_VELOCITY: u32 = 0x0000_1000;

fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// **The bike's collision record's two virtuals are the original's**: 12 000 records, each state 4's
/// with every word the virtuals read drawn at random (the switches and touch flags, the rest damping,
/// the sweep's offset, lead, push and facing cosine, the last segment, body or chassis, the node's
/// up, the terrain's box or none, the list's count and capacity, near-field or global, the mask, the
/// active part and sphere probe or none, the part's rays, the probes' hits and what they struck:
/// objects with a record (the record itself among them), a body or a chassis, a mover of kind 2 or
/// another, a category), run through SLUS-21104's `+0x7c` then `+0x84` and through
/// [`record::clear`] then [`record::pass`] (through [`RecordInRam`], the original's callees). Every
/// word of the record, its list, the other record and its list, and the body comes out the same, and
/// the outside calls are made in the same order with the same arguments. The callees outside the
/// record are hooked to log: the node's turn and placement answer a fixed map of their argument, its
/// position the node's own `+0xa0`, the searches nothing (the hits are laid out beforehand), the
/// mover its `+0x10`, the terrain query a height near the point and a normal, surfaces and weights
/// hashed off it. The contact filing and the terrain service are the original's own.
#[test]
#[allow(clippy::float_arithmetic)]
fn the_records_pass_is_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    let rec = ee.u32_at(BIKE + seed::RECORD);
    let (body, node, part, probe) = (ee.u32_at(rec + 0xe4), ee.u32_at(rec + 0xe8), ee.u32_at(rec + 0x200), ee.u32_at(rec + 0x204));
    assert_eq!(body, ee.u32_at(BIKE + seed::BODY), "the record's body is the bike's");
    assert_eq!(
        [0x7c, 0x84].map(|s| ee.u32_at(ee.u32_at(rec + 4) + s)),
        [RECORD_CLEAR, RECORD_PASS],
        "the record's two virtuals"
    );
    let damp = BODY_DAMP.map(|s| ee.u32_at(ee.u32_at(body + 0x110) + s));
    let log: Rc<RefCell<Vec<RecordCall>>> = Rc::default();
    let vf = |ee: &Ee, a: u64| vf_at(ee, a as u32).to_bits();
    let mut hooked_at = Vec::new();
    for (slot, at) in BODY_DAMP.into_iter().zip(damp) {
        let l = log.clone();
        hooked_at.push(at);
        ee.hooks.insert(at, Box::new(move |ee: &mut Ee| {
            assert_eq!(ee.r(4) as u32, body, "the damping set on another body");
            l.borrow_mut().push(RecordCall::Damp(slot, vf(ee, ee.r(5))));
        }));
    }
    // The node's maps: a turn and a placement of the argument, nothing like the node's own.
    let turned = |v: [f32; 4]| [v[2] * 0.75 + 0.5, v[0] - 0.25, v[1] * 1.25, v[3]];
    let placed = |v: [f32; 4], at: Vf| [v[0] * 4.0 + at.x(), v[1] * 4.0 + at.y(), v[2] * 4.0 + at.z(), v[3]];
    let l = log.clone();
    hooked_at.push(NODE_ROTATE);
    ee.hooks.insert(NODE_ROTATE, Box::new(move |ee: &mut Ee| {
        assert_eq!(ee.r(5) as u32, node, "the turn's node");
        let v = vf_at(ee, ee.r(6) as u32);
        set_vf(ee, ee.r(4) as u32, Vf(turned(v.0)));
        l.borrow_mut().push(RecordCall::Rotate(v.to_bits()));
    }));
    let l = log.clone();
    hooked_at.push(NODE_POSITION);
    ee.hooks.insert(NODE_POSITION, Box::new(move |ee: &mut Ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5), ee.r(6) as u32), (node, 0, rec + 0xd0), "the node position's arguments");
        for o in [0, 4, 8] {
            let w = ee.u32_at(node + 0xa0 + o);
            ee.set_u32(rec + 0xd0 + o, w);
        }
        l.borrow_mut().push(RecordCall::Position);
    }));
    let l = log.clone();
    hooked_at.push(NODE_SET);
    ee.hooks.insert(NODE_SET, Box::new(move |ee: &mut Ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5)), (node, 0), "the node set's arguments");
        l.borrow_mut().push(RecordCall::Move(vf(ee, ee.r(6))));
    }));
    let l = log.clone();
    hooked_at.push(NODE_TIP);
    ee.hooks.insert(NODE_TIP, Box::new(move |ee: &mut Ee| {
        assert_eq!(ee.r(5) as u32, node, "the placement's node");
        let v = vf_at(ee, ee.r(6) as u32);
        let at = vf_at(ee, node + 0xa0);
        set_vf(ee, ee.r(4) as u32, Vf(placed(v.0, at)));
        l.borrow_mut().push(RecordCall::Tip(v.to_bits()));
    }));
    let l = log.clone();
    hooked_at.push(PROBE_SET);
    ee.hooks.insert(PROBE_SET, Box::new(move |ee: &mut Ee| {
        assert_eq!((ee.r(4) as u32, ee.r(6)), (probe, 0), "the sweep's probe");
        l.borrow_mut().push(RecordCall::Set(vf(ee, ee.r(5)), vf(ee, ee.r(5) + 0x10)));
    }));
    let l = log.clone();
    hooked_at.push(PROBE_PLACE);
    ee.hooks.insert(PROBE_PLACE, Box::new(move |ee: &mut Ee| l.borrow_mut().push(RecordCall::Place(ee.r(4) as u32))));
    let l = log.clone();
    hooked_at.push(SEARCH_NEAR);
    ee.hooks.insert(SEARCH_NEAR, Box::new(move |ee: &mut Ee| {
        assert_eq!((ee.r(5) as u32, ee.r(6)), (ee.u32_at(rec + 0xfc), 1), "the near-field search's list");
        l.borrow_mut().push(RecordCall::Search(ee.r(4) as u32, true));
    }));
    let l = log.clone();
    hooked_at.push(SEARCH_ALL);
    ee.hooks.insert(SEARCH_ALL, Box::new(move |ee: &mut Ee| {
        assert_eq!(ee.r(5), 0, "the global search's flag");
        l.borrow_mut().push(RecordCall::Search(ee.r(4) as u32, false));
    }));
    let l = log.clone();
    hooked_at.push(FAKE_MOVER_VELOCITY);
    ee.hooks.insert(FAKE_MOVER_VELOCITY, Box::new(move |ee: &mut Ee| {
        let v = vf_at(ee, ee.r(5) as u32 + 0x10);
        set_vf(ee, ee.r(4) as u32, v);
        l.borrow_mut().push(RecordCall::Mover(ee.r(5) as u32));
    }));
    let l = log.clone();
    hooked_at.push(TERRAIN_QUERY);
    ee.hooks.insert(TERRAIN_QUERY, Box::new(move |ee: &mut Ee| {
        assert_eq!(ee.r(4) as u32, ee.u32_at(rec + 0xec), "the query's terrain");
        let buf = ee.r(5) as u32;
        let p = vf_at(ee, buf);
        let [x, y, z, w] = p.to_bits();
        let h = mix(u64::from(x) ^ u64::from(z) << 32 ^ u64::from(y).wrapping_mul(0x9e37_79b9) ^ u64::from(w) << 7);
        let unit = |k: u32| ((h >> k) & 0xff) as f32 / 255.0;
        ee.set_f32(buf + 4, p.y() + unit(0) * 2.0 - 1.0);
        let n = [unit(8) - 0.5, 0.5 + unit(16), unit(24) - 0.5];
        for (k, c) in n.into_iter().enumerate() {
            ee.set_f32(buf + 0x10 + 4 * k as u32, c);
        }
        ee.set_u32(buf + 0x20, (h >> 32) as u32);
        let weights = match (h >> 60) % 4 {
            0 => [0.25; 4],
            1 => [0.0, unit(40), unit(40), 0.0],
            _ => [unit(36), unit(44), unit(52), unit(56)],
        };
        for (k, c) in weights.into_iter().enumerate() {
            ee.set_f32(buf + 0x30 + 4 * k as u32, c);
        }
        l.borrow_mut().push(RecordCall::Query(p.to_bits()));
    }));
    let mut distinct = hooked_at.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), hooked_at.len(), "each hooked callee is its own function: {hooked_at:#x?}");

    // Scratch: 0..6 the glue's; then the lists, the other record and its body and chassis, the
    // chassis, the movers and their vtable, the objects, the probes' hit records, the rays, the box.
    let mut next = 8;
    let mut take = |bytes: u32| {
        let a = ee.scratch(next);
        next += bytes.div_ceil(0x40);
        a
    };
    let (list, other, other_list) = (take(16 * record::CONTACT), take(0x210), take(16 * record::CONTACT));
    let (other_body, other_chassis, chassis) = (take(0x90), take(0x260), take(0x260));
    let movers = [take(0x1b0), take(0x1b0)];
    let mover_vt = take(0xd0);
    let objects: Vec<u32> = (0..5).map(|_| take(0xa0)).collect();
    let (sweep_hits, sweep_arrays) = (take(0xa0), take(0xc0));
    let (part_hits, part_arrays) = (take(0xa0), take(0x300));
    let (rays, terrain) = (take(8 * 0x20), take(0x60));
    assert!(next < 240, "{next} scratch slots");

    let regions = vec![(rec, 0x210), (body, 0x230), (part, 0x184), (probe, 0x130), (node, 0xb0), (rec + 0x210, 0)];
    let regions: Vec<_> = regions.into_iter().filter(|r| r.1 != 0).collect();
    let image_words = snap_regions(&ee, &regions);
    let compared = [(rec, 0x210), (list, 16 * record::CONTACT), (other, 0x210), (other_list, 16 * record::CONTACT), (body, 0x230)];
    let mut g = Gen(0x0031_b04a);
    let mut t = BTreeMap::new();
    for k in 0..12_000 {
        restore_regions(&mut ee, &regions, &image_words);
        let v = |ee: &mut Ee, g: &mut Gen, a: u32, r: f32| {
            for i in 0..3 {
                let x = g.near(0.0, -r, r);
                ee.set_f32(a + 4 * i, x);
            }
            let w = if g.one_in(3) { g.f(-2.0, 2.0) } else { 1.0 };
            ee.set_f32(a + 12, w);
        };
        let flag = |g: &mut Gen, n: u32| u32::from(!g.one_in(n));
        let junk = |ee: &mut Ee, g: &mut Gen, a: u32, len: u32| {
            for o in (0..len).step_by(4) {
                ee.set_u32(a + o, g.bits());
            }
        };
        junk(&mut ee, &mut g, list, 16 * record::CONTACT);
        junk(&mut ee, &mut g, other_list, 16 * record::CONTACT);
        // The record: its switches, touch flags, damping, sweep and last segment.
        for (o, n) in [(0x24, 2), (0x28, 4), (0x2c, 6), (0x30, 4), (0x44, 2)] {
            let w = flag(&mut g, n);
            ee.set_u32(rec + o, w);
        }
        // (The touch flags are written 0 and 1, and the port holds them as such.)
        for o in [0x34, 0x38, 0x3c, 0x40] {
            let w = g.bits() % 2;
            ee.set_u32(rec + o, w);
        }
        for o in [0x48, 0x50] {
            let x = g.f(0.0, 2.0);
            ee.set_f32(rec + o, x);
        }
        v(&mut ee, &mut g, rec + 0xa0, 2.0);
        for (o, lo, hi) in [(0xb0, 0.0, 3.0), (0xb4, 0.0, 3.0), (0xb8, -1.0, 1.0)] {
            let x = g.near(0.0, lo, hi);
            ee.set_f32(rec + o, x);
        }
        v(&mut ee, &mut g, rec + 0xc0, 50.0);
        v(&mut ee, &mut g, rec + 0xd0, 50.0);
        // Body or chassis; the body's quaternion, velocity (slow now and then), position.
        let has_body = !g.one_in(3);
        ee.set_u32(rec + 0xe4, if has_body { body } else { 0 });
        ee.set_u32(rec + 0xe0, chassis);
        let qt = [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0)];
        let len = qt.iter().map(|x| x * x).sum::<f32>().sqrt();
        for i in 0..4 {
            ee.set_f32(body + 0x190 + 4 * i, qt[i as usize] / len);
        }
        let r = if g.one_in(3) { 1.2 } else { 30.0 };
        v(&mut ee, &mut g, body + 0x80, r);
        v(&mut ee, &mut g, chassis + 0x250, r);
        v(&mut ee, &mut g, body + 0x70, 50.0);
        // The node: its up and its translation (what the hooks' position and placement read).
        for o in [0x74, 0x84, 0x94] {
            let x = g.f(-1.0, 1.0);
            ee.set_f32(node + o, x);
        }
        v(&mut ee, &mut g, node + 0xa0, 50.0);
        // The terrain's box about the node, or none.
        if g.one_in(6) {
            ee.set_u32(rec + 0xec, 0);
        } else {
            ee.set_u32(rec + 0xec, terrain);
            let c = vf_at(&ee, node + 0xa0);
            for (o, sign) in [(0x30, -1.0), (0x40, 1.0)] {
                for (i, at) in [(0, c.x()), (2, c.z())] {
                    let x = at + sign * g.f(-4.0, 14.0);
                    ee.set_f32(terrain + o + 4 * i, x);
                }
            }
        }
        // The list: count and capacity (full, past full, none).
        let cap = [-1, 0, 1, 2, 3, 14, 16][(g.bits() % 7) as usize];
        ee.set_u32(rec + 0xf4, cap as u32);
        ee.set_u32(rec + 0xf0, g.bits() % 4);
        ee.set_u32(rec + 0xf8, list);
        ee.set_u32(rec + 0xfc, if g.one_in(2) { 0 } else { g.bits() | 0x10 });
        let mask = if g.one_in(3) { 0 } else { g.bits() };
        ee.set_u32(rec + 0x108, mask);
        ee.set_u32(rec + 0x200, if g.one_in(8) { 0 } else { part });
        ee.set_u32(rec + 0x204, if g.one_in(8) { 0 } else { probe });
        ee.set_u32(rec + 0x20c, g.bits());
        // The other record: its flags, list, and body or chassis.
        junk(&mut ee, &mut g, other, 0x210);
        for o in [0x34, 0x38, 0x3c, 0x40] {
            let w = g.bits() % 2;
            ee.set_u32(other + o, w);
        }
        ee.set_u32(other + 0xf0, g.bits() % 3);
        ee.set_u32(other + 0xf4, [0, 2, 16][(g.bits() % 3) as usize]);
        ee.set_u32(other + 0xf8, other_list);
        ee.set_u32(other + 0xe4, if g.one_in(2) { 0 } else { other_body });
        ee.set_u32(other + 0xe0, other_chassis);
        v(&mut ee, &mut g, other_body + 0x80, r);
        v(&mut ee, &mut g, other_chassis + 0x250, r);
        // The movers, one of kind 2 and one not (now and then both 2), their velocity at +0x10.
        ee.set_u32(mover_vt + 0xc0, FAKE_MOVER_VELOCITY);
        for (i, &m) in movers.iter().enumerate() {
            ee.set_u32(m + 4, mover_vt);
            ee.set_u32(m + 0x1a0, if i == 0 || g.one_in(4) { 2 } else { g.bits() % 4 });
            v(&mut ee, &mut g, m + 0x10, r);
        }
        // The objects: a record (the other or this one) or none, a mover or none, a category.
        for &o in &objects {
            let rc = [0, 0, other, other, rec][(g.bits() % 5) as usize];
            let m = [0, 0, movers[0], movers[1]][(g.bits() % 4) as usize];
            ee.set_u32(o + 0x90, rc);
            ee.set_u32(o + 0x8c, m);
            ee.set_u32(o + 0x94, if g.one_in(2) { 1 << (g.bits() % 32) } else { 0 });
        }
        let object = |g: &mut Gen| objects[(g.bits() % objects.len() as u32) as usize];
        // The sphere probe's hit: its bit, its record's first normal, point and object (or none).
        let b126 = ee.u32_at(probe + 0x124) & !0x0020_0000;
        let hit = g.bits() % 3 != 0;
        ee.set_u32(probe + 0x124, b126 | if hit { 0x0020_0000 } else { 0 } | (g.bits() & 0x00df_0000));
        ee.set_u32(probe + 0x7c, sweep_hits);
        ee.set_u32(sweep_hits + 0x8c, g.bits() % 2);
        for (i, o) in [0x94, 0x98, 0x9c].into_iter().enumerate() {
            ee.set_u32(sweep_hits + o, sweep_arrays + 0x40 * i as u32);
        }
        let mut n = [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0)];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        n = n.map(|x| x / l);
        for i in 0..3 {
            ee.set_f32(sweep_arrays + 4 * i, n[i as usize]);
        }
        v(&mut ee, &mut g, sweep_arrays + 0x40, 50.0);
        let o = if g.one_in(4) { 0 } else { object(&mut g) };
        ee.set_u32(sweep_arrays + 0x80, o);
        // The part: its rays (some pointing up, some down), its hit record.
        let n_rays = g.bits() % 7;
        ee.set_u32(part + 0x178, n_rays);
        ee.set_u32(part + 0x180, rays);
        for i in 0..n_rays {
            v(&mut ee, &mut g, rays + 0x20 * i, 2.0);
            v(&mut ee, &mut g, rays + 0x20 * i + 0x10, 2.0);
        }
        ee.set_u32(part + 0x7c, part_hits);
        let n_hits = g.bits() % 5;
        ee.set_u32(part_hits + 0x8c, n_hits);
        for (i, o) in [0x90, 0x94, 0x98, 0x9c].into_iter().enumerate() {
            ee.set_u32(part_hits + o, part_arrays + 0xc0 * i as u32);
        }
        for i in 0..n_hits {
            v(&mut ee, &mut g, part_arrays + 0x20 * i, 50.0);
            v(&mut ee, &mut g, part_arrays + 0x20 * i + 0x10, 50.0);
            v(&mut ee, &mut g, part_arrays + 0xc0 + 0x10 * i, 1.0);
            v(&mut ee, &mut g, part_arrays + 0x180 + 0x10 * i, 50.0);
            let o = object(&mut g);
            ee.set_u32(part_arrays + 0x240 + 4 * i, o);
        }
        let dt = if g.one_in(2) { f32::from_bits(DT) } else { g.f(0.002, 0.05) };

        let before: Vec<Vec<u32>> = compared.iter().map(|&r| snap_regions(&ee, &[r])).collect();
        let r0 = record::read(&|a| ee.u32_at(a), rec);
        let snapshot = snap_regions(&ee, &regions);
        let scratch_words = |ee: &Ee| compared.iter().map(|&r| snap_regions(ee, &[r])).collect::<Vec<_>>();
        // The port first.
        log.borrow_mut().clear();
        port_record(&mut ee, rec, 0x7c, F(dt), None);
        let queries = port_record(&mut ee, rec, 0x84, F(dt), None);
        let want = scratch_words(&ee);
        let want_log = std::mem::take(&mut *log.borrow_mut());
        let r1 = record::read(&|a| ee.u32_at(a), rec);
        // Then the original, from the same words.
        restore_regions(&mut ee, &regions, &snapshot);
        for (&r, w) in compared.iter().zip(&before) {
            restore_regions(&mut ee, &[r], w);
        }
        for f in [RECORD_CLEAR, RECORD_PASS] {
            ee.set(28, GP);
            ee.call(f, &[u64::from(rec)], &[dt], 2_000_000);
        }
        let got = scratch_words(&ee);
        for (i, (g_, w_)) in got.iter().zip(&want).enumerate() {
            if let Some(j) = (0..g_.len()).find(|&j| g_[j] != w_[j]) {
                panic!(
                    "record {k}: region {i} ({:#x}) +{:#x} is {:#010x} in the original and {:#010x} in the port (was {:#010x})\n{want_log:x?}\n{r0:#?}",
                    compared[i].0,
                    4 * j,
                    g_[j],
                    w_[j],
                    before[i][j],
                );
            }
        }
        assert_eq!(*log.borrow(), want_log, "record {k}: the calls\n{r0:#?}");

        // The paths.
        if r0.damp && r0.body {
            tally(&mut t, "damped");
        }
        if r0.sweep && r0.probe {
            tally(&mut t, if r0.body { "sweep, body" } else { "sweep, chassis" });
            if want_log.iter().any(|c| matches!(c, RecordCall::Move(_))) {
                tally(&mut t, "pushed");
                if want_log.iter().any(|c| matches!(c, RecordCall::Mover(_))) {
                    tally(&mut t, "pushed past a mover");
                }
            } else if hit && o != 0 {
                tally(&mut t, "flying apart");
            }
        }
        if !r0.part || !r0.rays {
            tally(&mut t, "no rays");
        }
        if queries > 0 {
            tally(&mut t, "heightfield queried");
            if r1.on_ground {
                tally(&mut t, "on the ground");
            }
        }
        if want_log.iter().any(|c| matches!(c, RecordCall::Tip(_))) && queries == 0 {
            tally(&mut t, "outside the box");
        }
        if r1.on_object {
            tally(&mut t, "on an object");
        }
        if r1.contacts.len() > r0.capacity.max(0) as usize || r1.contacts.len() as i32 == r0.capacity && r0.capacity > 0 {
            tally(&mut t, "list full");
        }
        if r1.struck != r0.struck {
            tally(&mut t, "struck");
        }
        if r1.contacts.iter().any(|c| c.kind == 1) {
            tally(&mut t, "handed itself a contact");
        }
        if got[3] != before[3] {
            tally(&mut t, "handed another a contact");
        }
        if r1.contacts.iter().any(|c| c.mover != 0) {
            tally(&mut t, "filed a mover");
        }
        if r1.touching && r1.contacts.is_empty() && r0.mask != 0 {
            tally(&mut t, "touched, masked");
        }
    }
    for want in [
        "damped",
        "sweep, body",
        "sweep, chassis",
        "pushed",
        "pushed past a mover",
        "flying apart",
        "no rays",
        "heightfield queried",
        "on the ground",
        "outside the box",
        "on an object",
        "list full",
        "struck",
        "handed itself a contact",
        "handed another a contact",
        "filed a mover",
        "touched, masked",
    ] {
        let n = t.get(want).copied().unwrap_or(0);
        assert!(n >= 30, "only {n} record(s) went down {want:?}: {t:?}");
    }
    eprintln!("{t:?}");
}

// --- F31b5: the other riders' PreProcess leave the bike alone ----------------------------------------

/// [`PortPost`] with every preforce listener but the bike's watched: the body goes into RAM, the
/// bike object and its physics blocks are read before and after the listener, and the body the port
/// holds is read back. Each listener's element and code are kept.
struct Watched<'a> {
    inner: PortPost<'a>,
    bike: u32,
    ran: Vec<(u32, u32)>,
}

impl<'a> Watched<'a> {
    fn hybrid(&mut self) -> &mut Hybrid<'a> {
        &mut self.inner.0.0.0
    }

    fn snapshot(&mut self) -> Vec<u32> {
        let len = self.bike;
        let h = self.hybrid();
        let mut out: Vec<u32> = (0..len / 4).map(|i| h.ee.u32_at(BIKE + 4 * i)).collect();
        for (_, at, n) in physics(h.ee) {
            out.extend((0..n / 4).map(|i| h.ee.u32_at(at + 4 * i)));
        }
        out
    }
}

impl Dynamics for Watched<'_> {
    fn count(&self, l: List) -> i32 {
        self.inner.count(l)
    }
    fn preforce(&mut self, i: i32, dt: F) {
        let h = self.hybrid();
        let e = h.elt(List::Preforce, i);
        if e == BIKE_PREFORCE {
            return self.inner.preforce(i, dt);
        }
        let f = h.ee.u32_at(h.ee.u32_at(e) + 8);
        for (a, w) in seed::body_words(&h.body, h.at) {
            h.ee.set_u32(a, w);
        }
        let body = h.body;
        let before = self.snapshot();
        self.inner.preforce(i, dt);
        let after = self.snapshot();
        let parted: Vec<usize> = (0..before.len()).filter(|&k| before[k] != after[k]).collect();
        assert!(parted.is_empty(), "preforce {i} (element {e:#x}, code {f:#x}) wrote the bike's words {parted:?}");
        assert_eq!(self.hybrid().body, body, "preforce {i} (element {e:#x}) moved the bike's body");
        self.ran.push((e, f));
    }
    fn force(&mut self, i: i32, h: F) {
        self.inner.force(i, h)
    }
    fn enabled(&self, b: i32) -> bool {
        self.inner.enabled(b)
    }
    fn integrate(&mut self, b: i32, h: F) {
        self.inner.integrate(b, h)
    }
    fn end_substep(&mut self, b: i32) {
        self.inner.end_substep(b)
    }
    fn constrain(&mut self, c: i32, first: bool) {
        self.inner.constrain(c, first)
    }
    fn post(&mut self, i: i32, dt: F) {
        self.inner.post(i, dt)
    }
}

/// One sim frame as [`port_record_frame`] runs it, the world step [`Watched`]'s. Answers the other
/// preforce listeners that ran, in order.
fn watched_frame(ee: &mut Ee, dt: F, map: &track::Terrain<'_>, bike: u32) -> Vec<(u32, u32)> {
    let call = |ee: &mut Ee, f: u32, ints: &[u64], x: F| {
        ee.set(28, GP);
        ee.call(f, ints, &[x.0], BUDGET);
        ee.r(2)
    };
    assert_eq!(ee.u8_at(ee.u32_at(SKIP_FLAG) + 0x225) & 8, 0, "the frame is not skipped");
    let mine = ee.u32_at(BIKE + seed::RECORD);
    for slot in [0x7c, 0x84] {
        for i in 0..ee.u32_at(SIM + 0x24) {
            let e = call(ee, LIST24_GET, &[u64::from(SIM + 0x24), u64::from(i)], F::ZERO) as u32;
            if e == mine {
                let t = ee.u32_at(e + 0xec);
                let terrain = (t != 0).then(|| ground::read_terrain(&|a| ee.u32_at(a), t, Some(map)));
                port_record(ee, e, slot, dt, terrain.as_ref());
            } else {
                call(ee, ee.u32_at(ee.u32_at(e + 4) + slot), &[u64::from(e)], dt);
            }
        }
    }
    let world = World { iterations: ee.u32_at(WORLD) as i32, rate: F(ee.f32_at(WORLD + 4)) };
    let at = ee.u32_at(BIKE + seed::BODY);
    let body = seed::body(&|a| ee.u32_at(a), at);
    let inner = PortPost(PortStep(PortHead(Hybrid { ee, body, at, integrated: 0 }), 0), 0);
    let mut h = Watched { inner, bike, ran: Vec::new() };
    world.step(dt, &mut h);
    let (body, ran) = (h.inner.0.0.0.body, h.ran);
    for (a, w) in seed::body_words(&body, at) {
        ee.set_u32(a, w);
    }
    call(ee, TAIL, &[u64::from(SIM)], dt);
    let mut i = 0;
    while i < ee.u32_at(SIM + 0x3c) {
        let e = call(ee, LIST3C_GET, &[u64::from(SIM + 0x3c), u64::from(i)], F::ZERO) as u32;
        if call(ee, ee.u32_at(ee.u32_at(e) + 8), &[u64::from(e)], dt) != 0 {
            call(ee, LIST3C_REMOVE, &[u64::from(SIM + 0x3c), u64::from(i)], F::ZERO);
        } else {
            i += 1;
        }
    }
    ran
}

/// [`replay_port_record`] with every frame [`watched_frame`]. Answers the other listeners each frame
/// ran (the same every frame).
fn replay_watched(name: &str) -> Option<Vec<(u32, u32)>> {
    let trn = nat05_trn()?;
    let map = track::Terrain::parse(&trn).unwrap();
    let mut ee = hooked(image()?);
    let rows = rows(name)?;
    let bike = 4 * obj(&rows[0]).len() as u32;
    let mut ran = None;
    for (r, row) in rows.iter().enumerate() {
        poke(&mut ee, row, &READER);
        let these = watched_frame(&mut ee, F(f32::from_bits(DT)), &map, bike);
        assert_eq!(*ran.get_or_insert_with(|| these.clone()), these, "{name} row {r}: the same listeners every frame");
        let d = diff(&ee, row);
        assert!(d.is_empty(), "{name} row {r}: {} words differ\n  {}", d.len(), d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  "));
        assert_eq!(wrecked(|o| ee.u32_at(BIKE + o)), row_wrecked(row), "{name} row {r}: the wreck state");
        poke(&mut ee, row, &SPOT);
    }
    ran
}

/// **The other five riders' PreProcess never touch the bike**: once a frame on every row of C15
/// ride-2 and flip-1 (through its wreck and put-back), each preforce listener of state 4's world that
/// is not the bike's runs as SLUS-21104's code and leaves the bike object, its body, node, record,
/// wheels and engine word for word as they were; the replay around them is [`port_record_frame`]'s
/// and stays bit-exact. So F31b5's pure-Rust frame can leave them out of the bike's step.
#[test]
fn the_other_riders_pre_process_leave_the_bike_alone() {
    for name in ["ride-2", "flip-1"] {
        let Some(ran) = replay_watched(name) else { return };
        assert_eq!(ran.len(), 5, "{name}: five other listeners a frame: {ran:#x?}");
        let mut each = ran.clone();
        each.sort();
        each.dedup();
        assert_eq!(each.len(), 5, "{name}: five distinct listeners: {each:#x?}");
        assert!(each.iter().all(|&(e, _)| e != BIKE_PREFORCE));
        assert!(each.iter().all(|&(_, f)| f == ran[0].1), "{name}: one PreProcess for the five riders: {each:#x?}");
        eprintln!("{name}: the riders' PreProcess {:#x} over {each:#x?}", ran[0].1);
    }
}

// ---- F31b4a: the frame's tail and the bike's vehicle update, ported ----

use game_sim::handling::tail::{self, Listeners, Vehicles};

/// The bike's listener's step (its vt `+0x40`): a thunk to the vehicle update with the bike.
const VEHICLE_THUNK: u32 = 0x004d_e0a0;
const VEHICLE_UPDATE: u32 = 0x004d_a210;
/// The stuck timer (the bike's vt `+0x45c`), which the port runs as its own.
const STUCK: u32 = 0x004a_ad60;
/// The vehicle update's callees outside the bike's vtable: the pause lookup's name lookup and its
/// two names, the pad's rumble test, the network step and the vehicle's parts and listeners.
const LOOKUP: u32 = 0x0021_4b30;
const LOOKUP_ARGS: [u64; 3] = [0x0054_23e0, 0x0052_9b0c, 0];
const RUMBLE_READY: u32 = 0x0051_a250;
const NETWORK: u32 = 0x0025_e8d0;
const PARTS: u32 = 0x0049_4370;
/// The bike's vtable slots the update calls, as the port names them.
const TAIL_SLOTS: [(u32, tail::Out); 13] = [
    (0x3a4, tail::Out::Fade),
    (0x4f8, tail::Out::WreckFirst),
    (0x4e8, tail::Out::WreckPose),
    (0x4a0, tail::Out::WreckLast { reason: 0 }),
    (0x49c, tail::Out::WreckTest),
    (0x4f0, tail::Out::Lean),
    (0x3e4, tail::Out::Prepare),
    (0x35c, tail::Out::Ride),
    (0x378, tail::Out::Sync),
    (0x37c, tail::Out::Snapshot),
    (PUT_BACK_SLOT, tail::Out::PutBack),
    (0x4b8, tail::Out::TrickStart(0)),
    (0x4ac, tail::Out::TrickEnd),
];

fn slot_of(c: tail::Out) -> Option<u32> {
    TAIL_SLOTS.iter().find(|s| std::mem::discriminant(&s.1) == std::mem::discriminant(&c)).map(|s| s.0)
}

/// The bike in RAM as the port's vehicle update sees it: [`tail::Vehicle`]'s words, written back
/// word by word before each call and read again after it. Each call is the original's but the wreck
/// test's and the put-back's, which are the port's ([`port_wreck`]), and the fade's, lean's,
/// prepare's, ride's, sync's, snapshot's, network's and parts' (F39, [`port_update`]).
struct TailInRam<'a> {
    ee: &'a mut Ee,
    v: tail::Vehicle,
    last: Vec<(u32, u32)>,
}

impl TailInRam<'_> {
    fn read(&mut self) {
        let ee = &*self.ee;
        self.v = tail::read(&|a| ee.u32_at(a), BIKE);
        self.last = tail::words(&self.v, BIKE);
    }

    fn write(&mut self) {
        let now = tail::words(&self.v, BIKE);
        for (k, &(a, w)) in now.iter().enumerate() {
            if self.last.get(k) != Some(&(a, w)) {
                self.ee.set_u32(a, w);
            }
        }
        self.last = now;
    }

    fn run(&mut self, f: u32, ints: &[u64], x: f32) -> u64 {
        self.write();
        self.ee.set(28, GP);
        census(self.ee, |ee| ee.call(f, ints, &[x], BUDGET));
        let r = self.ee.r(2);
        self.read();
        r
    }
}

impl Vehicles for TailInRam<'_> {
    fn vehicle(&mut self) -> &mut tail::Vehicle {
        &mut self.v
    }
    fn skipped(&mut self) -> bool {
        self.ee.u8_at(self.ee.u32_at(SKIP_FLAG) + 0x225) & 8 != 0
    }
    fn paused(&mut self) -> bool {
        label("tail Paused");
        let m = self.run(LOOKUP, &LOOKUP_ARGS, 0.0);
        let state = self.ee.u32_at(BIKE + tail::STATE);
        let g = self.run(self.ee.u32_at(self.ee.u32_at(state + 4) + 0xac), &[u64::from(state), m], 0.0) as u32;
        let obj = if g == 0 { 0 } else { g - 0x2ec };
        self.ee.u32_at(self.ee.u32_at(obj + 0x378) + 0x14) != 0
    }
    fn rumble_ready(&mut self) -> bool {
        label("tail RumbleReady");
        let state = self.ee.u32_at(BIKE + tail::STATE);
        self.run(RUMBLE_READY, &[u64::from(self.ee.u32_at(state + 0x6ac))], 0.0) as u32 != 0
    }
    fn word(&mut self, at: u32) -> u32 {
        self.write();
        self.ee.u32_at(at)
    }
    fn out(&mut self, c: tail::Out, dt: F) {
        let r0 = self.ee.ran;
        self.out_(c, dt);
        if matches!(c, tail::Out::Message(_)) && in_rust("frame post") {
            // Posted in Rust: its tells are booked by listener ([`FramePost`]).
            return;
        }
        let kind = format!("{c:?}");
        book(format!("the tail: out {}", &kind[..kind.find([' ', '(']).unwrap_or(kind.len())]), self.ee.ran - r0);
    }
}

impl TailInRam<'_> {
    fn out_(&mut self, c: tail::Out, dt: F) {
        use tail::Out;
        label(&format!("tail {c:?}"));
        self.write();
        if matches!(c, Out::WreckTest | Out::PutBack) {
            port_wreck(self.ee, c, dt);
            self.read();
            return;
        }
        if matches!(c, Out::Fade | Out::Lean | Out::Prepare | Out::Ride | Out::Sync | Out::Snapshot | Out::Network | Out::Parts) {
            port_update(self.ee, c, dt);
            self.read();
            return;
        }
        let ee = &*self.ee;
        let bike = u64::from(BIKE);
        let (f, ints, x) = match c {
            Out::OwnListeners => (TAIL, vec![bike + 0xc], dt.0),
            Out::WreckLast { reason } => (vt(ee, 0x4a0), vec![bike, u64::from(reason)], dt.0),
            Out::WreckTest => (vt(ee, 0x49c), vec![bike, 0], dt.0),
            Out::FadeOver(o) => (ee.u32_at(ee.u32_at(o + 4) + 0x90), vec![u64::from(o)], dt.0),
            Out::Network => (NETWORK, vec![bike + 0x400], dt.0),
            Out::Parts => (PARTS, vec![bike], dt.0),
            Out::PutBack => (vt(ee, PUT_BACK_SLOT), vec![bike, 1, 1], dt.0),
            Out::TrickStart(id) => (vt(ee, 0x4b8), vec![bike, u64::from(id), 1], dt.0),
            Out::TrickEnd => (vt(ee, 0x4ac), vec![bike, u64::MAX], dt.0),
            Out::Rumble { soft, level } => {
                let o = ee.u32_at(ee.u32_at(BIKE + tail::STATE) + 0x5c);
                (ee.u32_at(ee.u32_at(o + 0x1c) + 0x4c), vec![u64::from(o), u64::from(soft), level as i64 as u64], tail::RUMBLE_TIME)
            }
            Out::RunOver { player, run } => (ee.u32_at(ee.u32_at(BIKE + 0x6a0) + 0x68), vec![bike + 0x680, u64::from(player), u64::from(run)], dt.0),
            Out::RunOverItem { item, player, run } => (ee.u32_at(ee.u32_at(item + 0x20) + 0x68), vec![u64::from(item), u64::from(player), u64::from(run)], dt.0),
            Out::RunOverRider { rider, player, run } => (ee.u32_at(ee.u32_at(rider + 0x1c) + 0x7c), vec![u64::from(rider), u64::from(player), u64::from(run)], dt.0),
            Out::Message(id) => {
                let (kind, msg) = tail::FRAME_MESSAGE;
                (POST_MESSAGE, vec![u64::from(ee.u32_at(POST_QUEUE)), bike + 0x674, u64::from(kind), u64::from(msg), u64::from(id), 0], dt.0)
            }
            c => (vt(ee, slot_of(c).unwrap()), vec![bike], dt.0),
        };
        if let Out::Message(id) = c
            && in_rust("frame post")
        {
            frame_post(self.ee, id, dt);
            self.read();
            return;
        }
        self.run(f, &ints, x);
    }
}

/// **F31b5l — the frame's message posted in Rust**: [`reset::post`] on state 4's queue for the bike's
/// poster, the message [`tail::FRAME_MESSAGE`] with the bike's id. Each listener the registration
/// tells is [`FramePost`]'s.
fn frame_post(ee: &mut Ee, id: u32, dt: F) {
    let (kind, msg) = tail::FRAME_MESSAGE;
    let queue = ee.u32_at(POST_QUEUE);
    reset::post(&mut FramePost { ee, x: dt.0 }, queue, BIKE + 0x674, [kind, msg, 0, id]);
}

/// The listeners state 4's registration for the bike's poster tells, by their receive (vt `+0xc`,
/// most a thunk to the object's own), as (receive, name): F31b5l. Eleven: one each but the fourth,
/// which is five. None is the bike's code. What each does with the frame's message (kind 8, id 4):
///
/// - the HUD: hands the bike's event record to its own gauges (the speed's length times a scale to
///   the speedometer, a flag word to a lamp): presentation, ~600 instructions a frame;
/// - the race progress: with its rider the bike, walks the bike along its track nodes (its own
///   `+0xec..+0xf8`, a node's distance), else steps another rider's: ~1,200 instructions a frame;
/// - the speed flag: only while its own vt `+0x3c` answers set, keeps whether the bike is under
///   29.3332 at its `+0x54` (inactive on state 4: 22 instructions);
/// - the menu list: only while its `+0x3c` is set (0 on state 4);
/// - the pickers (five), the camera's controller (ids 0x15, 7, 6, 0xd, 0xc, 0xb) and an empty
///   receive: nothing for id 4.
const FRAME_LISTENERS: [(u32, &str); 7] = [
    (0x002f_bc80, "hud"),
    (0x002a_2430, "race progress"),
    (0x002a_2440, "speed flag"),
    (0x002a_eed0, "menu list"),
    (0x002a_3420, "picker"),
    (0x0049_1270, "camera controller"),
    (0x0051_b2a0, "empty"),
];

/// The frame post's outside: the message laid out in scratch as the poster lays it on its stack, and
/// each listener told the original's ([`FRAME_LISTENERS`]; `$f12` the frame's `dt`, as the original
/// post leaves it), booked by name. Every tell is watched: it may not change a word of the bike or its
/// physics blocks; the tells that move `rand`'s state are counted. With `"frame post tells dropped"`
/// in [`IN_RUST`] no listener is told.
struct FramePost<'a> {
    ee: &'a mut Ee,
    x: f32,
}

impl Resets for FramePost<'_> {
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn set(&mut self, at: u32, w: u32) {
        self.ee.set_u32(at, w);
    }
    fn out(&mut self, c: reset::Out) -> u32 {
        match c {
            reset::Out::Message(words) => {
                let at = self.ee.scratch(MESSAGE_SCRATCH);
                for (k, w) in words.into_iter().enumerate() {
                    self.ee.set_u32(at + 4 * k as u32, w);
                }
                at
            }
            reset::Out::Tell { to, from, message } => {
                let f = self.ee.u32_at(self.ee.u32_at(to + 8) + 0xc);
                let Some(&(_, name)) = FRAME_LISTENERS.iter().find(|l| l.0 == f) else {
                    panic!("a frame post listener other than the known: {to:#x}, receive {f:#x}")
                };
                if in_rust("frame post tells dropped") {
                    return 0;
                }
                let (before, rand) = (bike_and_blocks(self.ee), rand_state(self.ee));
                let r0 = self.ee.ran;
                self.ee.set(28, GP);
                census(self.ee, |ee| ee.call(f, &[u64::from(to), u64::from(from), u64::from(message)], &[self.x], BUDGET));
                book(format!("the tail: the frame post tells the {name}"), self.ee.ran - r0);
                if rand_state(self.ee) != rand {
                    book(format!("the tail: the frame post's {name} tells that drew rand"), 1);
                }
                let parted: Vec<String> =
                    before.iter().zip(&bike_and_blocks(self.ee)).filter(|(b, a)| b != a).map(|((at, b), (_, a))| format!("{at:#x}: {b:#010x} -> {a:#010x}")).collect();
                assert!(parted.is_empty(), "the frame post's {name} ({to:#x}) wrote the bike's words:\n  {}", parted.join("\n  "));
                self.ee.r(2) as u32
            }
            c => unreachable!("the frame post makes no {c:?}"),
        }
    }
}

/// How far below the interpreter's stack top the calls' frames reach, at most: RAM there is left out
/// of the vehicle update's comparison, the port's calls sitting at other depths than the original's.
const STACK_USE: usize = 0x4_0000;

/// The sim's listener tree in RAM, walked by [`tail::walk`]: a group's step (the walk itself) is the
/// port's walk one level down, the bike's step is the port's [`tail::vehicle_update`] through
/// [`TailInRam`], any other listener's step is the original's. Counts the bike's steps.
///
/// Every bike step is checked: SLUS-21104's vehicle update runs first on a copy of RAM, and the
/// port's must leave all of RAM (but the stack) and the scratchpad as it did, byte for byte.
struct TreeInRam<'a> {
    ee: &'a mut Ee,
    bikes: u32,
    others: Vec<u32>,
}

impl Listeners for TreeInRam<'_> {
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn step(&mut self, e: u32, dt: F) {
        let f = self.ee.u32_at(self.ee.u32_at(e + 4) + 0x40);
        if f == TAIL {
            tail::walk(self, e, dt);
        } else if e == BIKE + 0xc {
            assert_eq!(f, VEHICLE_THUNK, "the bike's listener's step");
            assert_eq!(vt(self.ee, 0x45c), STUCK, "the bike's stuck timer");
            let (ram, spr) = (self.ee.ram.clone(), self.ee.spr.clone());
            self.ee.set(28, GP);
            let r0 = self.ee.ran;
            self.ee.call(VEHICLE_UPDATE, &[u64::from(BIKE)], &[dt.0], BUDGET);
            book("the tail: the original vehicle update beside the port", self.ee.ran - r0);
            let r0 = self.ee.ran;
            let (want, want_spr) = (std::mem::replace(&mut self.ee.ram, ram), std::mem::replace(&mut self.ee.spr, spr));
            let mut m = TailInRam { ee: &mut *self.ee, v: tail::Vehicle::default(), last: Vec::new() };
            m.read();
            tail::vehicle_update(&mut m, dt);
            m.write();
            book("the tail: the ported vehicle update", self.ee.ran - r0);
            let stack = (self.ee.stack as usize).saturating_sub(STACK_USE)..self.ee.ram.len().min(self.ee.stack as usize + 0x4000);
            // F31b5j: with the part walk's leaves (or one kind's own work) dropped, only the bike and
            // its blocks are the original's; F31b5l: so too with the frame post's tells dropped.
            let only_bike = IN_RUST.with(|r| {
                r.borrow().iter().any(|n| n.starts_with("part leaves dropped") || n.starts_with("part leaf dropped") || *n == "frame post tells dropped")
            });
            let bike: Vec<std::ops::Range<usize>> = if only_bike {
                let mut b = vec![BIKE as usize..BIKE as usize + 0x1800];
                b.extend(physics(self.ee).into_iter().map(|(_, at, n)| at as usize..(at + n) as usize));
                b
            } else {
                vec![0..want.len()]
            };
            let d: Vec<String> = (0..want.len())
                .step_by(4)
                .filter(|a| !stack.contains(a) && bike.iter().any(|r| r.contains(a)) && want[*a..*a + 4] != self.ee.ram[*a..*a + 4])
                .map(|a| format!("{a:#x}: port {:#010x}, original {:#010x}", self.ee.u32_at(a as u32), u32::from_le_bytes(want[a..a + 4].try_into().unwrap())))
                .collect();
            assert!(d.is_empty(), "the vehicle update: {} words of RAM differ from the original's\n  {}", d.len(), d.iter().take(20).cloned().collect::<Vec<_>>().join("\n  "));
            assert!(only_bike || want_spr == self.ee.spr, "the vehicle update: the scratchpad differs from the original's");
            self.bikes += 1;
        } else {
            let Some(&(_, _, writes)) = OTHER_LEAVES.iter().find(|l| (l.0, l.1) == (e, f)) else {
                panic!("a tail leaf other than the known: {e:#x}, step {f:#x}")
            };
            let before = bike_and_blocks(self.ee);
            self.others.push(e);
            self.ee.set(28, GP);
            let r0 = self.ee.ran;
            let walks = f == MANAGER_STEP;
            if dropped_others(true) && e != RIDER {
                // F31b5m: the first leaf and the manager's own work left out; the manager's walk
                // still steps the rider, below.
            } else if walks {
                // Its own work first, its closing walk stood in for; then that walk, the port's.
                let walked: Rc<RefCell<Vec<(u32, u32)>>> = Rc::default();
                let w = walked.clone();
                self.ee.hooks.insert(TAIL, Box::new(move |ee: &mut Ee| w.borrow_mut().push((ee.r(4) as u32, ee.f(12).to_bits()))));
                self.ee.call(f, &[u64::from(e)], &[dt.0], BUDGET);
                self.ee.hooks.remove(&TAIL);
                assert_eq!(*walked.borrow(), [(e, dt.to_bits())], "the manager's closing walk");
            } else if e == RIDER && in_rust("rider leaf") {
                rider_leaf(self.ee, dt);
            } else {
                self.ee.call(f, &[u64::from(e)], &[dt.0], BUDGET);
            }
            book(["the tail: the first other leaf", "the tail: the manager", "the tail: the rider"][OTHER_LEAVES.iter().position(|l| l.0 == e).unwrap()], self.ee.ran - r0);
            let after = bike_and_blocks(self.ee);
            let parted: Vec<String> = before
                .iter()
                .zip(&after)
                .filter(|((at, b), (_, a))| b != a && !writes.iter().any(|r| r.contains(&at.wrapping_sub(BIKE))))
                .map(|((at, b), (_, a))| format!("{at:#x}: {b:#010x} -> {a:#010x}"))
                .collect();
            assert!(parted.is_empty(), "tail leaf {e:#x} (step {f:#x}) wrote the bike's words:\n  {}", parted.join("\n  "));
            if walks {
                tail::walk(self, e, dt);
            }
        }
    }
}

/// The tail's listeners on state 4 other than the bike, as (listener, step, the bike's words it may
/// write): F39 watches each ([`TreeInRam`]). The first two are the sim's other two leaves, and leave
/// the bike and its blocks alone; the second's own walk steps the third, the rider, whose target is
/// the bike's record at `+0x730`, and whose step writes two windows of that record and nothing else
/// of the bike or its physics blocks: its `+0x1f0..+0x250` (the bike's `+0x920..+0x980`, through the
/// rider's own physics world's step) and, on the wreck, its `+0x140..+0x170` (the bike's
/// `+0x870..+0x8a0`: the rider's camera spots). Both replays together write exactly these words of
/// them: `+0x870..+0x888` but `+0x87c`, `+0x890..+0x898`, `+0x920`, `+0x930..+0x938`, `+0x940`,
/// and `+0x950..+0x97c` but `+0x95c`.
const OTHER_LEAVES: [(u32, u32, &[std::ops::Range<u32>]); 3] =
    [(0x00eb_6420, 0x001c_9540, &[]), (0x01ed_82d0, MANAGER_STEP, &[]), (0x010a_f350, 0x004f_2ad0, &[0x870..0x8a0, 0x920..0x980])];
/// The second leaf's step: its own work, then the walk of its own listeners.
const MANAGER_STEP: u32 = 0x002c_f9f0;

/// The bike object's words and its physics blocks' ([`physics`]), each with its address.
fn bike_and_blocks(ee: &Ee) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = (0..0x1800).step_by(4).map(|o| (BIKE + o, ee.u32_at(BIKE + o))).collect();
    for (_, at, n) in physics(ee) {
        out.extend((0..n).step_by(4).map(|o| (at + o, ee.u32_at(at + o))));
    }
    out
}

/// The frame's tail on the sim, the port's walk and vehicle update ([`TreeInRam`]). Answers how many
/// times the bike stepped and the other listeners stepped.
fn port_tail(ee: &mut Ee, dt: F) -> (u32, Vec<u32>) {
    let mut t = TreeInRam { ee, bikes: 0, others: Vec::new() };
    tail::walk(&mut t, SIM, dt);
    (t.bikes, t.others)
}

/// A call the vehicle update makes outside the bike's own code, as the oracle logs it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum TailCall {
    Out(tail::Out),
    Paused,
    Ready,
}

/// The port's side of the vehicle update's oracle: the calls logged, the answers drawn, the links
/// read off the RAM the original then runs on.
struct TailLogged<'a> {
    ee: &'a Ee,
    v: tail::Vehicle,
    calls: Vec<TailCall>,
    skipped: bool,
    paused: bool,
    ready: bool,
}

impl Vehicles for TailLogged<'_> {
    fn vehicle(&mut self) -> &mut tail::Vehicle {
        &mut self.v
    }
    fn skipped(&mut self) -> bool {
        self.skipped
    }
    fn paused(&mut self) -> bool {
        self.calls.push(TailCall::Paused);
        self.paused
    }
    fn rumble_ready(&mut self) -> bool {
        self.calls.push(TailCall::Ready);
        self.ready
    }
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn out(&mut self, c: tail::Out, _: F) {
        self.calls.push(TailCall::Out(c));
    }
}

/// **The bike's vehicle update is the original's**: 12 000 bikes, each state 4's with every word the
/// update, its own update, its base update and its stuck timer read drawn at random (the state and
/// its kind, the skip, the pause, the rumbler and its answer; the speeds with ties; each wheel's ray,
/// touch, roughness about the rumble's 0.73 and 0.95, and its surface's hold and limit (−1, ±0,
/// either sign); the put-back's words and countdown; the free and stuck words; on the ground,
/// airborne, the hang time about 0.1, the wreck reason; the wheelie and stoppie clocks about 1 and
/// their times, distances, bests and tricks; the run or none (its flags, step, top speed, landed and
/// its end's clock about 0, its items) and the player or none (its clock, its riders); the node's
/// axes, the ahead's `y` at ±1 and past; the body's position and velocity; the run's, lap's, frame's
/// and fade's clocks, the fade's object or none; the ride's clocks; `dt`), run through SLUS-21104's
/// vehicle update and through [`tail::vehicle_update`] from the same words. Every word of the bike,
/// its wheels, body, node and state, the run, the player and their lists comes out the same, and the
/// calls are made in the same order with the same arguments. The callees outside the four (the bike's
/// vtable's, the pause's lookups, the rumble's test and the rumbler, the fade's object, the run's,
/// items' and riders' virtuals, the network step, the parts, the walk, the message) are hooked to log
/// and do nothing.
#[test]
#[allow(clippy::float_arithmetic)]
fn the_bikes_vehicle_update_is_the_originals() {
    use std::cell::Cell;
    use tail::Out;
    let Some(mut ee) = image().map(hooked) else { return };
    assert_eq!(ee.u32_at(ee.u32_at(BIKE + 0x10) + 0x40), VEHICLE_THUNK, "the bike's listener's step");
    assert_eq!(vt(&ee, 0x45c), STUCK, "the bike's stuck timer");
    let [front, rear] = the_wheels(&ee);
    let body = ee.u32_at(BIKE + seed::BODY);
    let node = ee.u32_at(BIKE + seed::NODE);
    let state = ee.u32_at(BIKE + tail::STATE);
    let skip_at = ee.u32_at(SKIP_FLAG) + 0x225;
    let dt_bits = Rc::new(Cell::new(0u32));
    let (g_says, ready_says) = (Rc::new(Cell::new(0u32)), Rc::new(Cell::new(0u32)));
    let log: Rc<RefCell<Vec<TailCall>>> = Rc::default();
    let mut hooked_at = Vec::new();
    let mut hook = |ee: &mut Ee, at: u32, h: Box<dyn FnMut(&mut Ee)>| {
        hooked_at.push(at);
        ee.hooks.insert(at, h);
    };

    // The fake objects, in scratch: functions, vtables, the run, the player, items, riders.
    let fns: [u32; 5] = std::array::from_fn(|i| ee.scratch(240) + 4 * i as u32);
    let [rumble_fn, fade_fn, run_fn, item_fn, rider_fn] = fns;
    let vtable = |ee: &mut Ee, k: u32, slot: u32, f: u32| {
        let at = ee.scratch(k);
        ee.set_u32(at + slot, f);
        at
    };
    let v_rumble = vtable(&mut ee, 200, 0x4c, rumble_fn);
    let v_fade = vtable(&mut ee, 203, 0x90, fade_fn);
    let v_run = vtable(&mut ee, 206, 0x68, run_fn);
    let v_item = vtable(&mut ee, 209, 0x68, item_fn);
    let v_rider = vtable(&mut ee, 212, 0x7c, rider_fn);
    let run = ee.scratch(0);
    let player = ee.scratch(8);
    let items = [20, 21, 22].map(|k| ee.scratch(k));
    let riders = [24, 25, 26].map(|k| ee.scratch(k));
    let rumbler = ee.scratch(28);
    let fader = ee.scratch(29);
    let pause_flag = ee.scratch(31);
    let pauser = ee.scratch(30) - 0x378;
    let group = ee.scratch(32);
    const NAME: u64 = 0x00ab_cd00;

    for (slot, c) in TAIL_SLOTS {
        let (l, d) = (log.clone(), dt_bits.clone());
        let at = vt(&ee, slot);
        hook(&mut ee, at, Box::new(move |ee: &mut Ee| {
            assert_eq!(ee.r(4) as u32, BIKE, "{c:?} handed something other than the bike");
            let c = match c {
                Out::WreckLast { .. } => Out::WreckLast { reason: ee.r(5) as u32 },
                Out::TrickStart(_) => {
                    assert_eq!(ee.r(6), 1, "the trick start's third argument");
                    Out::TrickStart(ee.r(5) as u32)
                }
                Out::TrickEnd => {
                    assert_eq!(ee.r(5) as u32, u32::MAX, "the trick end's argument");
                    c
                }
                Out::WreckTest => {
                    assert_eq!(ee.r(5), 0, "the wreck test's argument");
                    c
                }
                Out::PutBack => {
                    assert_eq!((ee.r(5), ee.r(6)), (1, 1), "the put-back's arguments");
                    c
                }
                c => c,
            };
            if matches!(c, Out::Fade | Out::WreckPose | Out::WreckLast { .. } | Out::WreckTest | Out::Lean | Out::Ride | Out::PutBack) {
                assert_eq!(ee.f(12).to_bits(), d.get(), "{c:?} handed dt");
            }
            l.borrow_mut().push(TailCall::Out(c));
        }));
    }
    for (at, want, c) in [(TAIL, BIKE + 0xc, Out::OwnListeners), (NETWORK, BIKE + 0x400, Out::Network), (PARTS, BIKE, Out::Parts)] {
        let (l, d) = (log.clone(), dt_bits.clone());
        hook(&mut ee, at, Box::new(move |ee: &mut Ee| {
            assert_eq!((ee.r(4) as u32, ee.f(12).to_bits()), (want, d.get()), "{c:?}'s arguments");
            l.borrow_mut().push(TailCall::Out(c));
        }));
    }
    hook(&mut ee, LOOKUP, Box::new(move |ee: &mut Ee| {
        assert_eq!([ee.r(4), ee.r(5), ee.r(6)], LOOKUP_ARGS, "the pause lookup's names");
        ee.set(2, NAME);
    }));
    let (l, g) = (log.clone(), g_says.clone());
    let state_lookup = ee.u32_at(ee.u32_at(state + 4) + 0xac);
    hook(&mut ee, state_lookup, Box::new(move |ee: &mut Ee| {
        assert_eq!((ee.r(4) as u32, ee.r(5)), (state, NAME), "the state's lookup's arguments");
        l.borrow_mut().push(TailCall::Paused);
        ee.set(2, u64::from(g.get()));
    }));
    let (l, r) = (log.clone(), ready_says.clone());
    hook(&mut ee, RUMBLE_READY, Box::new(move |ee: &mut Ee| {
        assert_eq!(ee.r(4) as u32, ee.u32_at(state + 0x6ac), "the rumble test's argument");
        l.borrow_mut().push(TailCall::Ready);
        ee.set(2, u64::from(r.get()));
    }));
    let l = log.clone();
    hook(&mut ee, rumble_fn, Box::new(move |ee: &mut Ee| {
        assert_eq!((ee.r(4) as u32, ee.f(12)), (rumbler, tail::RUMBLE_TIME), "the rumbler and the rumble's time");
        l.borrow_mut().push(TailCall::Out(Out::Rumble { soft: ee.r(5) != 0, level: ee.r(6) as i32 }));
    }));
    let l = log.clone();
    hook(&mut ee, fade_fn, Box::new(move |ee: &mut Ee| l.borrow_mut().push(TailCall::Out(Out::FadeOver(ee.r(4) as u32)))));
    let l = log.clone();
    hook(&mut ee, run_fn, Box::new(move |ee: &mut Ee| {
        assert_eq!(ee.r(4) as u32, BIKE + 0x680, "the run's own virtual's object");
        l.borrow_mut().push(TailCall::Out(Out::RunOver { player: ee.r(5) as u32, run: ee.r(6) as u32 }));
    }));
    let l = log.clone();
    hook(&mut ee, item_fn, Box::new(move |ee: &mut Ee| {
        let (item, player, run) = (ee.r(4) as u32, ee.r(5) as u32, ee.r(6) as u32);
        l.borrow_mut().push(TailCall::Out(Out::RunOverItem { item, player, run }));
    }));
    let l = log.clone();
    hook(&mut ee, rider_fn, Box::new(move |ee: &mut Ee| {
        let (rider, player, run) = (ee.r(4) as u32, ee.r(5) as u32, ee.r(6) as u32);
        l.borrow_mut().push(TailCall::Out(Out::RunOverRider { rider, player, run }));
    }));
    let l = log.clone();
    hook(&mut ee, POST_MESSAGE, Box::new(move |ee: &mut Ee| {
        let (kind, msg) = tail::FRAME_MESSAGE;
        let want = (u64::from(ee.u32_at(POST_QUEUE)), u64::from(BIKE + 0x674), u64::from(kind), u64::from(msg), 0);
        assert_eq!((ee.r(4), ee.r(5), ee.r(6), ee.r(7), ee.r(9)), want, "the message's arguments");
        l.borrow_mut().push(TailCall::Out(Out::Message(ee.r(8) as u32)));
    }));
    let mut distinct = hooked_at.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), hooked_at.len(), "each hooked callee is its own function: {hooked_at:#x?}");
    assert!(!hooked_at.contains(&STUCK) && !hooked_at.contains(&VEHICLE_UPDATE));

    let regions = vec![
        (BIKE, 0x1800),
        (front, 0x920),
        (rear, 0x920),
        (body, 0x230),
        (node, 0x100),
        (state, 0x6b0),
        (skip_at & !3, 4),
        (ee.scratch(0), 33 * 0x40),
    ];
    let image_words = snap_regions(&ee, &regions);
    let kind = ee.u32_at(state + tail::KIND);
    let mut g = Gen(0x0031_b4a0);
    let mut t = BTreeMap::new();
    for k in 0..12_000 {
        restore_regions(&mut ee, &regions, &image_words);
        let f = |ee: &mut Ee, a: u32, x: f32| ee.set_f32(a, x);
        let v3 = |ee: &mut Ee, g: &mut Gen, a: u32, r: f32| {
            for i in 0..3 {
                let x = g.near(0.0, -r, r);
                ee.set_f32(a + 4 * i, x);
            }
        };
        let bit = |g: &mut Gen, n: u32| u32::from(!g.one_in(n));
        // The state, its kind, the skip, the pause, the rumbler and its answer.
        match g.bits() % 12 {
            0 => ee.set_u32(BIKE + tail::STATE, 0),
            n => ee.set_u32(state + tail::KIND, [kind, kind, kind, kind, kind, 3, 4, 3, 4, 0, 2, 5][n as usize]),
        }
        let skip = ee.u8_at(skip_at) & !8 | if g.one_in(6) { 8 } else { 0 };
        ee.write(skip_at, 1, u128::from(skip));
        g_says.set(pauser + 0x2ec);
        ee.set_u32(pauser + 0x378, pause_flag);
        ee.set_u32(pause_flag + 0x14, if g.one_in(10) { g.bits() | 1 } else { 0 });
        ee.set_u32(state + 0x5c, if g.one_in(5) { 0 } else { rumbler });
        ee.set_u32(rumbler + 0x1c, v_rumble);
        ee.set_u32(state + 0x6ac, g.bits());
        ready_says.set(if g.one_in(4) { 0 } else { [1, g.bits() | 1][(g.bits() % 2) as usize] });
        // The speeds.
        let speed = g.near(0.0, -10.0, 40.0);
        f(&mut ee, BIKE + 0xc38, speed);
        let slow = if g.one_in(6) { speed } else { g.f(0.0, 10.0) };
        f(&mut ee, BIKE + 0x174c, slow);
        let len = if g.one_in(6) { 10.0 } else { g.f(0.0, 30.0) };
        f(&mut ee, BIKE + 0x32c, len);
        // The wheels and their surfaces' entries.
        for w in [front, rear] {
            ee.write(w + 0x110, 1, u128::from(g.bits() % 4));
            ee.set_u32(w + 0xd0, if g.one_in(2) { 0 } else { [1, g.bits() | 1][(g.bits() % 2) as usize] });
            ee.set_u32(w + 0xdc, bit(&mut g, 3));
            let rough = match g.bits() % 6 {
                0 => 0.95,
                1 => 0.73,
                _ => g.f(0.0, 1.3),
            };
            f(&mut ee, w + 0xf0, rough);
        }
        for s in 0..4 {
            let e = BIKE + 0xdf4 + 12 * s;
            ee.set_u32(e, if g.one_in(3) { 0 } else { [1, g.bits() | 1][(g.bits() % 2) as usize] });
            let lim = match g.bits() % 8 {
                0 | 1 => -1.0,
                2 => 0.0,
                3 => -0.0,
                4 => g.f(-2.0, 0.0),
                _ => g.f(0.01, 1.0),
            };
            f(&mut ee, e + 4, lim);
        }
        // The put-back's words, the free and stuck words, the ground and air, the wreck reason.
        for o in [0x31c, 0x320, 0xc14, 0xc10, 0x324, 0xde0] {
            ee.set_u32(BIKE + o, if g.one_in(2) { 0 } else { [1, g.bits() | 1][(g.bits() % 2) as usize] });
        }
        f(&mut ee, BIKE + 0xdf0, g.f(0.0, 1.0));
        v3(&mut ee, &mut g, BIKE + 0xc00, 100.0);
        let hang = g.near(0.1, 0.0, 0.3);
        f(&mut ee, BIKE + 0x1190, hang);
        ee.set_u32(BIKE + 0xc80, if g.one_in(6) { g.bits() | 1 } else { 0 });
        // The wheelie's and stoppie's words.
        for o in [0x1744, 0x1748] {
            let x = g.near(1.0, 0.0, 1.5);
            f(&mut ee, BIKE + o, x);
        }
        for o in [0x704, 0x708] {
            let x = g.near(0.0, -0.5, 3.0);
            f(&mut ee, BIKE + o, x);
        }
        for o in [0x70c, 0x710, 0x714, 0x718, 0x71c, 0x720] {
            let x = g.f(0.0, 40.0);
            f(&mut ee, BIKE + o, x);
        }
        for o in [0x13c4, 0x13c8] {
            ee.set_u32(BIKE + o, g.bits());
        }
        // The run or none, its words and items; the player or none, its clock and riders.
        ee.set_u32(BIKE + 0x680, if g.one_in(5) { 0 } else { run });
        ee.set_u32(BIKE + 0x6a0, v_run);
        ee.set_u32(run + 8, g.bits() & 0x0040_0200 | g.bits() & 0x0f0f);
        f(&mut ee, run + 0x18, g.f(0.0, 2.0));
        let top = g.f(0.0, 60.0);
        f(&mut ee, run + 0x108, top);
        f(&mut ee, BIKE + 0x6fc, if g.one_in(6) { top } else { g.f(0.0, 60.0) });
        ee.set_u32(run + 0x40, bit(&mut g, 2));
        let left = match g.bits() % 5 {
            0 => 0.0,
            1 => g.f(-0.1, 0.0),
            2 => g.f(0.0, 2.0),
            _ => g.f(0.0, 0.06),
        };
        f(&mut ee, run + 0xc4, left);
        let n = g.bits() % 4;
        ee.set_u32(run + 0x1d4, if n > 0 { items[0] } else { 0 });
        for i in 0..3 {
            ee.set_u32(items[i] + 0x20, v_item);
            ee.set_u32(items[i] + 0x1c, if (i as u32) + 1 < n { items[i + 1] } else { 0 });
        }
        ee.set_u32(BIKE + 0x684, if g.one_in(5) { 0 } else { player });
        let clock = g.near(0.0, -1.0, 1.0);
        f(&mut ee, player + 0x29c, clock);
        let n = g.bits() % 4;
        ee.set_u32(player + 0x24, if n > 0 { riders[0] } else { 0 });
        for i in 0..3 {
            ee.set_u32(riders[i] + 0x1c, v_rider);
            let next = if (i as u32) + 1 < n { riders[i + 1] } else { 0 };
            if i == 0 {
                ee.set_u32(riders[0] + 4, next);
                ee.set_u32(riders[0] + 0x18, 0);
            } else {
                ee.set_u32(riders[i] + 0x18, next);
                ee.set_u32(riders[i] + 4, 0);
            }
        }
        for o in [0x6f8, 0x724] {
            let x = g.f(0.0, 100.0);
            f(&mut ee, BIKE + o, x);
        }
        ee.set_u32(BIKE + 0x6c8, g.bits() % 100);
        ee.set_u32(BIKE + 0x6cc, bit(&mut g, 2));
        // The node's axes, the ahead's `y` at and past ±1; the body's position and velocity.
        v3(&mut ee, &mut g, node + 0x80, 1.0);
        v3(&mut ee, &mut g, node + 0x90, 1.0);
        let y = match g.bits() % 6 {
            0 => -1.0,
            1 => 1.0,
            2 => g.f(-1.5, -1.0),
            3 => g.f(1.0, 1.5),
            _ => g.f(-1.0, 1.0),
        };
        f(&mut ee, node + 0x94, y);
        v3(&mut ee, &mut g, body + 0x70, 150.0);
        v3(&mut ee, &mut g, body + 0x80, 40.0);
        // The lap, frame and fade clocks, the fade's object or none; the ride's clocks.
        ee.set_u32(BIKE + 0x175c, bit(&mut g, 2));
        let lap = g.near(0.0, -0.1, 1.0);
        f(&mut ee, BIKE + 0x1760, lap);
        f(&mut ee, BIKE + 0x1750, g.f(0.0, 100.0));
        ee.set_u32(BIKE + 0xd20, g.bits());
        ee.set_u32(BIKE + 0x1770, [-1i32, 0, 1, 1, 2, 7][(g.bits() % 6) as usize] as u32);
        let fade = g.near(0.0, -0.1, 0.1);
        f(&mut ee, BIKE + 0x177c, fade);
        ee.set_u32(BIKE + 0x1778, group);
        ee.set_u32(group + 0x30, if g.one_in(3) { 0 } else { fader });
        ee.set_u32(fader + 4, v_fade);
        ee.set_u32(BIKE + 0x1650, g.bits());
        f(&mut ee, BIKE + 0xcf8, g.f(0.0, 100.0));
        for o in [0xd2c, 0xd30, 0xd34] {
            let x = g.near(0.0, -1.0, 1.0);
            f(&mut ee, BIKE + o, x);
        }
        for o in [0x12a8, 0x12ac, 0x310, 0x12c0] {
            ee.set_u32(BIKE + o, g.bits());
        }
        let dt = if g.one_in(2) { f32::from_bits(DT) } else { g.f(0.002, 0.05) };
        dt_bits.set(dt.to_bits());
        let before = snap_regions(&ee, &regions);

        let v0 = tail::read(&|a| ee.u32_at(a), BIKE);
        let skipped = ee.u8_at(skip_at) & 8 != 0;
        let paused = ee.u32_at(pause_flag + 0x14) != 0;
        let mut m = TailLogged { ee: &ee, v: v0.clone(), calls: Vec::new(), skipped, paused, ready: ready_says.get() != 0 };
        tail::vehicle_update(&mut m, F(dt));
        let (v, calls) = (m.v, m.calls);

        let has = |c: &dyn Fn(&TailCall) -> bool| calls.iter().any(c);
        if v0.state == 0 {
            tally(&mut t, "no state");
        } else if paused {
            tally(&mut t, "paused");
        } else {
            tally(&mut t, if skipped { "skipped" } else if has(&|c| *c == TailCall::Out(Out::OwnListeners)) { "down" } else { "wreck test" });
            if v0.countdown == 1 {
                tally(&mut t, "countdown over");
            }
            if has(&|c| matches!(c, TailCall::Out(Out::FadeOver(_)))) {
                tally(&mut t, "fade over");
            } else if !(v0.fade <= F::ZERO) && v.fade == F::ZERO {
                tally(&mut t, "fade over, no object");
            }
            tally(&mut t, if v.free != 0 { "free" } else if has(&|c| *c == TailCall::Out(Out::PutBack)) { "stuck, put back" } else { "stuck" });
            match calls.iter().find_map(|c| if let TailCall::Out(Out::Rumble { soft, .. }) = c { Some(*soft) } else { None }) {
                Some(true) => tally(&mut t, "rumble"),
                Some(false) => tally(&mut t, "rumble, strong"),
                None if has(&|c| *c == TailCall::Ready) => tally(&mut t, "rumble not ready"),
                None => {}
            }
            if v.wheelie_clock != v0.wheelie_clock && v.wheelie_clock != F::ZERO {
                tally(&mut t, "wheelie");
            }
            if v.stoppie_clock != v0.stoppie_clock && v.stoppie_clock != F::ZERO {
                tally(&mut t, "stoppie");
            }
            if v.best_wheelie_distance != v0.best_wheelie_distance || v.best_stoppie_distance != v0.best_stoppie_distance {
                tally(&mut t, "best distance");
            }
            if has(&|c| matches!(c, TailCall::Out(Out::TrickStart(_)))) {
                tally(&mut t, "trick start");
            }
            if has(&|c| *c == TailCall::Out(Out::TrickEnd)) {
                tally(&mut t, "trick end");
            }
            if v.run != 0 {
                tally(&mut t, if v0.wheels[0].touching { "distance, speed" } else { "distance, heading" });
                if v.distance == v0.distance {
                    tally(&mut t, "no distance");
                }
                if v.landings != v0.landings {
                    tally(&mut t, "landed");
                }
            }
            if has(&|c| matches!(c, TailCall::Out(Out::RunOver { .. }))) {
                tally(&mut t, "run over");
            }
            if has(&|c| matches!(c, TailCall::Out(Out::RunOverItem { .. }))) {
                tally(&mut t, "items told");
            }
            if calls.iter().filter(|c| matches!(c, TailCall::Out(Out::RunOverRider { .. }))).count() > 1 {
                tally(&mut t, "riders told");
            }
            if v0.lap != 0 && v.lap == 0 {
                tally(&mut t, "lap over");
            }
        }

        log.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(VEHICLE_UPDATE, &[u64::from(BIKE)], &[dt], BUDGET);
        let got = snap_regions(&ee, &regions);
        let mut want = before.clone();
        let index = |a: u32| {
            let mut i = 0;
            for &(at, len) in &regions {
                if (at..at + len).contains(&a) {
                    return i + ((a - at) / 4) as usize;
                }
                i += (len / 4) as usize;
            }
            panic!("{a:#x} is in none of the regions")
        };
        for (a, w) in tail::words(&v, BIKE) {
            want[index(a)] = w;
        }
        let at = |i: usize| {
            let mut i = i;
            for &(a, len) in &regions {
                if i < (len / 4) as usize {
                    return a + 4 * i as u32;
                }
                i -= (len / 4) as usize;
            }
            unreachable!()
        };
        let d: Vec<String> = (0..want.len())
            .filter(|&i| want[i] != got[i])
            .map(|i| format!("{:#x}: before {:#010x}, port {:#010x}, original {:#010x}", at(i), before[i], want[i], got[i]))
            .collect();
        assert!(d.is_empty(), "bike {k}: {} words differ\n  {}\nport calls {calls:?}\noriginal {:?}", d.len(), d.join("\n  "), log.borrow());
        assert_eq!(calls, *log.borrow(), "bike {k}: the calls");
    }
    eprintln!("{t:#?}");
    for (path, n) in &t {
        assert!(*n >= 20, "path {path} taken only {n} times");
    }
}

/// What a leaf's step does in the walk's oracle, both sides alike: with its `+0x28` not 0, it sets
/// its own next to `+0x28` − 1 (the walk reads the next only after the step).
fn walk_leaf(ee: &mut Ee, e: u32) {
    let cut = ee.u32_at(e + 0x28);
    if cut != 0 {
        ee.set_u32(e + tail::NEXT, cut - 1);
    }
}

/// The port's side of the walk's oracle: a group's step is the port's walk, a leaf's is
/// [`walk_leaf`], logged with its `dt`.
struct WalkLogged<'a> {
    ee: &'a mut Ee,
    log: Vec<(u32, u32)>,
}

impl Listeners for WalkLogged<'_> {
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn step(&mut self, e: u32, dt: F) {
        if self.ee.u32_at(self.ee.u32_at(e + 4) + 0x40) == TAIL {
            tail::walk(self, e, dt);
        } else {
            walk_leaf(self.ee, e);
            self.log.push((e, dt.to_bits()));
        }
    }
}

/// **The tail's walk is the original's**: 10 000 listener trees laid out in scratch, a group of up to
/// six listeners with their state bytes (on, held, gone, the rest) and wants drawn at random, groups
/// among them (one with up to four of its own, the others empty), the groups' step bits drawn too,
/// and leaves that cut or end their list as they step. SLUS-21104's walk and [`tail::walk`] step the
/// same leaves in the same order with the same `dt`, and leave the same words.
#[test]
fn the_tails_walk_is_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    let leaf_fn = ee.scratch(240);
    let v_leaf = ee.scratch(20);
    let v_group = ee.scratch(23);
    ee.set_u32(v_leaf + 0x40, leaf_fn);
    ee.set_u32(v_group + 0x40, TAIL);
    let log: Rc<RefCell<Vec<(u32, u32)>>> = Rc::default();
    let l = log.clone();
    ee.hooks.insert(leaf_fn, Box::new(move |ee: &mut Ee| {
        let e = ee.r(4) as u32;
        walk_leaf(ee, e);
        l.borrow_mut().push((e, ee.f(12).to_bits()));
    }));
    let root = ee.scratch(0);
    let top: [u32; 6] = std::array::from_fn(|i| ee.scratch(1 + i as u32));
    let sub: [u32; 4] = std::array::from_fn(|i| ee.scratch(7 + i as u32));
    let regions = [(root, 11 * 0x40)];
    let mut g = Gen(0x0031_b4a1);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        let flags = |g: &mut Gen| g.bits() & !2 | if g.one_in(3) { 0 } else { 2 };
        ee.set_u32(root + tail::GROUP_FLAGS, flags(&mut g));
        let lay = |ee: &mut Ee, g: &mut Gen, list: &[u32], n: usize| {
            for i in 0..list.len() {
                let e = list[i];
                ee.set_u32(e + 4, v_leaf);
                ee.set_u32(e + tail::NEXT, if i + 1 < n { list[i + 1] } else { 0 });
                ee.set_u32(e + tail::FIRST, 0);
                let mut b = g.bits() & 0xf2;
                b |= if g.one_in(3) { 0 } else { tail::ON };
                b |= if g.one_in(6) { tail::HELD } else { 0 };
                b |= if g.one_in(6) { tail::GONE } else { 0 };
                ee.set_u32(e + 0x20, g.bits() & 0xffff_00ff | b << 8);
                ee.set_u32(e + tail::WANTS, g.bits() & !2 | if g.one_in(3) { 0 } else { 2 });
                let later = if i + 1 < n { list[i + 1..n].to_vec() } else { Vec::new() };
                let cut = match g.bits() % 8 {
                    0 => 1,
                    1 if !later.is_empty() => later[(g.bits() as usize) % later.len()] + 1,
                    _ => 0,
                };
                ee.set_u32(e + 0x28, cut);
            }
        };
        let n = (g.bits() % 7) as usize;
        ee.set_u32(root + tail::FIRST, if n > 0 { top[0] } else { 0 });
        lay(&mut ee, &mut g, &top, n);
        let m = (g.bits() % 5) as usize;
        lay(&mut ee, &mut g, &sub, m);
        let mut with_sub = false;
        for &e in &top[..n] {
            if g.one_in(4) {
                ee.set_u32(e + 4, v_group);
                ee.set_u32(e + tail::GROUP_FLAGS, flags(&mut g));
                if !with_sub && m > 0 {
                    ee.set_u32(e + tail::FIRST, sub[0]);
                    with_sub = true;
                }
            }
        }
        let dt = F(if g.one_in(2) { f32::from_bits(DT) } else { g.f(0.002, 0.05) });
        let before = snap_regions(&ee, &regions);

        let mut w = WalkLogged { ee: &mut ee, log: Vec::new() };
        tail::walk(&mut w, root, dt);
        let port = std::mem::take(&mut w.log);
        let port_words = snap_regions(&ee, &regions);
        restore_regions(&mut ee, &regions, &before);
        log.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(TAIL, &[u64::from(root)], &[dt.0], BUDGET);
        assert_eq!(port, *log.borrow(), "tree {k}: the leaves stepped");
        assert_eq!(port_words, snap_regions(&ee, &regions), "tree {k}: the words left");

        tally(&mut t, if port.is_empty() { "nothing stepped" } else { "stepped" });
        if port.iter().any(|&(e, _)| sub.contains(&e)) {
            tally(&mut t, "a group's own stepped");
        }
        if port.len() > 2 {
            tally(&mut t, "three or more");
        }
        if port.iter().any(|&(e, _)| before[((e - root) / 4 + 10) as usize] != 0) {
            tally(&mut t, "a stepped leaf cut its list");
        }
    }
    eprintln!("{t:#?}");
    for (path, n) in &t {
        assert!(*n >= 20, "path {path} taken only {n} times");
    }
}

// --- F31b4b: the bike's wreck around the sim frame -----------------------------------------------------

use game_sim::handling::wreck::{self, Wrecks};

/// The wreck's functions in SLUS-21104 (research/oracle.txt `# F31b4b`): the bike's virtuals, then
/// the calls out they make.
const WRECK_TEST: u32 = 0x004e_24a0;
const WRECK_ENTER: u32 = 0x004e_2260;
const UNWRECK: u32 = 0x004e_2380;
const PART_TOGGLE: u32 = 0x004e_1870;
const TO_WRECK: u32 = 0x004e_2e10;
const TO_RIDE: u32 = 0x004e_2eb0;
const PUT_BACK: u32 = 0x004d_8940;
const RAND: u32 = 0x0035_61a8;
const BOUNDS: u32 = 0x0031_d410;
const UNHOOK: u32 = 0x0018_d540;
const ATTACH: u32 = 0x0011_43e0;
const ENGINE_RESET: u32 = 0x0049_f7f0;
const WHEEL_RESET: u32 = 0x004a_04c0;
const RIDER_SETTLE: u32 = 0x0026_6c90;
const BONE_POSE: u32 = 0x0019_4e00;
const BONE_PLACE: u32 = 0x0019_4da0;
const TERRAIN_NORMAL: u32 = 0x0048_33c0;
const GROUND_RESET: u32 = 0x0049_2ca0;
/// F31b5e: the ground reset's and the orient's (research/oracle.txt `# F31b5c`): the bike's orient
/// (vt +0xf0) and spatial update (vt +0xe4), the body's staged position, velocity and orientation
/// and their commit (its vt +0x10, +0x14, +0x58, +0x3c), and the node hung back in the scene.
const ORIENT: u32 = 0x0049_2e50;
const SPATIAL_UPDATE: u32 = 0x0049_2c40;
const BODY_POSITION: u32 = 0x0049_2b90;
const BODY_VELOCITY: u32 = 0x0049_2b70;
const BODY_ORIENTATION: u32 = 0x0049_2b30;
const BODY_COMMIT: u32 = 0x0049_3070;
const REHANG: u32 = 0x0019_6c10;
const TRAIL_RESET: u32 = 0x004f_a760;
const CAMERA_RESET: u32 = 0x004f_7590;
/// The bike's vtable slots the wreck calls, the port's own first (each with its function).
const WRECK_OWN: [(u32, u32); 7] = [
    (0x49c, WRECK_TEST),
    (0x368, WRECK_ENTER),
    (0x36c, UNWRECK),
    (0x398, PART_TOGGLE),
    (0x48c, TO_WRECK),
    (0x490, TO_RIDE),
    (PUT_BACK_SLOT, PUT_BACK),
];
/// The scratch slots the port's calls hand out for their buffers: the body's quadword, the terrain's
/// normal, the bounds test's two words, the put-back's two bone buffers.
const WRECK_SCRATCH: [u32; 5] = [61, 62, 63, 64, 65];
/// The settle's zero word, handed to each listener it tells (F31b5f).
const TOLD_SCRATCH: u32 = 66;

/// The bike in RAM as the wreck's port sees it: every word read and written in place, each call out
/// the original's, the bike's own virtuals among the wreck's the port's, and the engine's and wheels' resets and the
/// sync too (F31b5c, F39).
struct WreckInRam<'a> {
    ee: &'a mut Ee,
    /// What `$f12` is handed with the calls that take no float of their own.
    x: f32,
}

impl WreckInRam<'_> {
    fn call(&mut self, f: u32, ints: &[u64], x: f32) -> u32 {
        self.ee.set(28, GP);
        census(self.ee, |ee| ee.call(f, ints, &[x], BUDGET));
        self.ee.r(2) as u32
    }
    fn own(&self, slot: u32) {
        let f = WRECK_OWN.iter().find(|s| s.0 == slot).unwrap().1;
        assert_eq!(vt(self.ee, slot), f, "the bike's vt +{slot:#x}");
    }
}

impl Wrecks for WreckInRam<'_> {
    fn bike(&self) -> u32 {
        BIKE
    }
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn set(&mut self, at: u32, w: u32) {
        self.ee.set_u32(at, w);
    }
    fn normal(&mut self, terrain: u32, at: u32) -> Vf {
        let out = self.ee.scratch(WRECK_SCRATCH[1]);
        label("wreck Normal");
        self.call(TERRAIN_NORMAL, &[u64::from(terrain), u64::from(at), u64::from(out), 0, 0], self.x);
        Vf::from_bits([0, 4, 8, 12].map(|k| self.ee.u32_at(out + k)))
    }
    fn out(&mut self, c: wreck::Out) -> u32 {
        use wreck::Out;
        label(&format!("wreck {c:?}"));
        let bike = u64::from(BIKE);
        let x = self.x;
        let ee = &*self.ee;
        let slot = |s: u32| vt(ee, s);
        let body = ee.u32_at(BIKE + wreck::BODY);
        let body_fn = |s: u32| ee.u32_at(ee.u32_at(body + 0x110) + s);
        let part_fn = |p: u32, s: u32| ee.u32_at(ee.u32_at(p + 0x28) + s);
        let (f, ints, x) = match c {
            Out::Enter(r) => {
                self.own(0x368);
                wreck::enter(self, r);
                return 0;
            }
            Out::Unwreck(kind) => {
                self.own(0x36c);
                wreck::unwreck(self, kind);
                return 0;
            }
            Out::Part(on) => {
                self.own(0x398);
                wreck::part(self, on);
                return 0;
            }
            Out::ToWreck | Out::ToRide => {
                self.own(if c == Out::ToWreck { 0x48c } else { 0x490 });
                wreck::swap(self, c == Out::ToWreck);
                return 0;
            }
            Out::PutBack { snap, kind } => {
                self.own(PUT_BACK_SLOT);
                wreck::put_back(self, snap, kind);
                return 0;
            }
            // F31b5c's: the engine's and wheels' resets, and the sync (F39's).
            Out::Engine(at) => {
                wreck::engine_reset(self, at);
                return 0;
            }
            Out::Wheel(at) => {
                wreck::wheel_reset(self, at);
                return 0;
            }
            Out::Sync => {
                assert_eq!(vt(self.ee, 0x378), SYNC_FN, "the bike's vt +0x378");
                update::sync(&mut UpdateInRam { ee: &mut *self.ee, x });
                return 0;
            }
            Out::OutOfBounds => (slot(0x16c), vec![bike], x),
            Out::Bounds { node, at } => {
                let out = u64::from(ee.scratch(WRECK_SCRATCH[2]));
                (BOUNDS, vec![u64::from(node), u64::from(at), out + 4, out, 0], 150.0)
            }
            Out::Settle => {
                assert_eq!((slot(0x4a8), slot(0x53c), slot(0x540)), (SETTLE_FN, FLIP_CLIP_SET, FLIP_NEXT_SET), "the bike's vt +0x4a8, +0x53c and +0x540");
                wreck::settle(self);
                return 0;
            }
            Out::Stance(i) => (slot(0x4b0), vec![bike, i as i64 as u64], x),
            Out::Told(l) => {
                let zero = ee.scratch(TOLD_SCRATCH);
                let f = ee.u32_at(ee.u32_at(l + 0x20) + 0xa0);
                self.ee.set_u32(zero, 0);
                (f, vec![u64::from(l), 5, u64::from(zero), 0, 0], x)
            }
            Out::Rand => (RAND, vec![], x),
            Out::Post { id, arg } => (POST_MESSAGE, vec![u64::from(ee.u32_at(POST_QUEUE)), bike + 0x674, 8, u64::from(id), u64::from(arg), 0], x),
            Out::RumbleReady(at) => (RUMBLE_READY, vec![u64::from(at)], x),
            Out::Rumble(r) => (ee.u32_at(ee.u32_at(r + 0x1c) + 0x4c), vec![u64::from(r), 0, 200], 1.0),
            Out::BodyAt { slot, at } => (body_fn(slot), vec![u64::from(body), u64::from(at)], x),
            Out::BodyQuad { slot, q } => {
                let at = ee.scratch(WRECK_SCRATCH[0]);
                let f = body_fn(slot);
                for (k, w) in q.to_bits().into_iter().enumerate() {
                    self.ee.set_u32(at + 4 * k as u32, w);
                }
                (f, vec![u64::from(body), u64::from(at)], x)
            }
            Out::PartOn(p) => (part_fn(p, 0x20), vec![u64::from(p + 0x24)], x),
            Out::PartOff(p) => (part_fn(p, 0x18), vec![u64::from(p + 0x24)], x),
            Out::Unhook(at) => (UNHOOK, vec![u64::from(at)], x),
            Out::Attach { part, a, b } => (ATTACH, vec![u64::from(part), u64::from(a), u64::from(b), 1], x),
            Out::Engine(_) | Out::Wheel(_) | Out::Sync => unreachable!("the port's"),
            Out::Rider { a, b } => (RIDER_SETTLE, vec![u64::from(a), u64::from(b), 1, 1], x),
            Out::Pose { bone, buf } => (BONE_POSE, vec![u64::from(bone), 0, u64::from(ee.scratch(WRECK_SCRATCH[3] + buf))], x),
            Out::Place { part, buf } => (BONE_PLACE, vec![u64::from(part), u64::from(ee.scratch(WRECK_SCRATCH[3] + buf))], x),
            // F31b5e's: the ground reset, the orient and the body's and node's calls they make.
            Out::Ground => {
                wreck::ground(self);
                return 0;
            }
            Out::Orient { at, forward, up } => {
                assert_eq!(vt(self.ee, 0xf0), ORIENT, "the bike's vt +0xf0");
                wreck::orient(self, at, forward, up);
                return 0;
            }
            Out::BodyPosition(_) | Out::BodyVelocity(_) | Out::BodyOrientation { .. } | Out::BodyCommit => {
                let (slot, f) = match c {
                    Out::BodyPosition(_) => (0x10, BODY_POSITION),
                    Out::BodyVelocity(_) => (0x14, BODY_VELOCITY),
                    Out::BodyOrientation { .. } => (0x58, BODY_ORIENTATION),
                    _ => (0x3c, BODY_COMMIT),
                };
                assert_eq!(body_fn(slot), f, "the body's vt +{slot:#x}");
                match c {
                    Out::BodyPosition(at) => wreck::body_position(self, body, at),
                    Out::BodyVelocity(at) => wreck::body_velocity(self, body, at),
                    Out::BodyOrientation { q, spin } => wreck::body_orientation(self, body, q, spin),
                    _ => wreck::body_commit(self, body),
                }
                return 0;
            }
            Out::NodeSet { node, at } => {
                assert_eq!(node, ee.u32_at(BIKE + 0xe4), "the bike's node");
                on_scene(self.ee, |s| s.set_position(0, at)).expect("the bike's node has no parent");
                return 0;
            }
            Out::NodeAim { node, forward, up } => {
                assert_eq!(node, ee.u32_at(BIKE + 0xe4), "the bike's node");
                on_scene(self.ee, |s| s.aim(0, forward, up));
                return 0;
            }
            Out::Spatial | Out::NodeBounds(_) | Out::PartSync(_) => {
                let call = match c {
                    Out::Spatial => {
                        assert_eq!(slot(0xe4), SPATIAL_UPDATE, "the bike's vt +0xe4");
                        Call::SpatialUpdate
                    }
                    Out::NodeBounds(node) => {
                        assert_eq!(node, ee.u32_at(BIKE + 0xe4), "the bike's node");
                        Call::NodeBounds
                    }
                    Out::PartSync(p) if p == ee.u32_at(BIKE + 0xd0) => Call::PartSync(quad::Part::Frame),
                    Out::PartSync(p) if p == ee.u32_at(BIKE + 0xcc) => Call::PartSync(quad::Part::Riding),
                    c => panic!("{c:?}: neither the frame nor the riding part"),
                };
                wreck_steps(self.ee, call, x);
                return 0;
            }
            Out::Rehang { node, to } => (REHANG, vec![u64::from(node), u64::from(to), 1, 1], x),
            Out::Kind(set) => (slot(if set { 0x3a0 } else { 0x39c }), vec![bike], x),
            Out::Trail(at) => (TRAIL_RESET, vec![u64::from(at)], x),
            Out::Camera(at) => (CAMERA_RESET, vec![u64::from(at)], x),
            Out::Normal { .. } => unreachable!("asked through normal()"),
        };
        self.call(f, &ints, x)
    }
}

/// F31b5e: `c` (the spatial update, the node's bounds or a part sync) as [`quad::Steps`]' on the bike
/// [`steps::read_post`] reads, its words put back where they changed; a collidable's database move
/// it hands on the original's.
fn wreck_steps(ee: &mut Ee, c: Call, dt: f32) {
    if node_box_on_ram(ee, c) {
        return;
    }
    let m = |a| ee.u32_at(a);
    let (k, at, parts) = steps::read_sync(&m, BIKE, KINDS).unwrap_or_else(|e| panic!("{c:?}: the bike's words: {e}"));
    let last = steps::post_words(&k, &at, &parts, &m);
    let mut s = quad::Steps {
        bike: k,
        rest: Vec::<Call>::new(),
        ground: |_: Vf, _: &mut u8| -> (Vf, Vf) { panic!("the ground reset asks for no ground") },
        sweep: (),
    };
    s.call(c, &mut Quad::default(), &mut quad::Body::default());
    let quad::Steps { bike: k, rest, .. } = s;
    let now = steps::post_words(&k, &at, &parts, &|a| ee.u32_at(a));
    set_changed(ee, &now, &last);
    for r in rest {
        match r {
            Call::PartMove { part, centre, half, reach } => {
                label("wreck PartMove");
                let (c, h) = (ee.scratch(0), ee.scratch(1));
                set_vf(ee, c, centre);
                set_vf(ee, h, half);
                let entry = parts[game_sim::atv_sync::slot(part)] + 0x18;
                ee.set(28, GP);
                census(ee, |ee| ee.call(SPATIAL_MOVE, &[u64::from(entry), u64::from(c), u64::from(h)], &[reach.0], BUDGET));
            }
            r => panic!("the ground reset's steps hand on no {r:?} (dt {dt})"),
        }
    }
}

/// The vehicle update's wreck test (with no position) or put-back (snap and kind 1) as the port's,
/// on the bike in RAM.
fn port_wreck(ee: &mut Ee, c: tail::Out, dt: F) {
    let mut m = WreckInRam { ee, x: dt.0 };
    match c {
        tail::Out::WreckTest => {
            m.own(0x49c);
            wreck::wreck_test(&mut m, 0, dt);
        }
        tail::Out::PutBack => {
            m.own(PUT_BACK_SLOT);
            wreck::put_back(&mut m, 1, 1);
        }
        c => unreachable!("{c:?}"),
    }
}

/// **F31b5e — the ground reset is the original's**: 10,000 times on state 4's bike, its spot,
/// forward and up (`+0x50`, `+0x60`, `+0x70`, every lane, the axes anywhere but parallel), its
/// body's offset (`+0x1a0`) and its parts' flag (`+0x44` bit 0) drawn at random, and the riding
/// part there or not — SLUS-21104's ground reset in the interpreter, and
/// [`wreck::ground`] served as the replays serve it (the orient, the body's and the node's calls,
/// the spatial update and the part syncs the ports'). Every word of RAM, and the scratchpad, come
/// out the same.
#[test]
#[allow(clippy::float_arithmetic)]
fn the_ground_reset_is_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    let base = ee.ram.clone();
    let mut g = Gen(0x0031_b5e0);
    let mut t = BTreeMap::new();
    let mut off = Vec::new();
    let body = ee.u32_at(BIKE + wreck::BODY);
    let riding = ee.u32_at(BIKE + 0xcc);
    for k in 0..10_000 {
        ee.ram.clone_from(&base);
        let quad = |ee: &mut Ee, at: u32, v: [f32; 4]| {
            for (i, x) in v.into_iter().enumerate() {
                ee.set_f32(at + 4 * i as u32, x);
            }
        };
        let r = |g: &mut Gen, s: f32| [g.f(-s, s), g.f(-s, s), g.f(-s, s), g.near(1.0, -2.0, 2.0)];
        let at = r(&mut g, 500.0);
        let (fwd, up) = loop {
            let (a, b) = (r(&mut g, 3.0), r(&mut g, 3.0));
            let c = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
            let n = |v: &[f32]| v[..3].iter().map(|x| x * x).sum::<f32>();
            if n(&c) > 0.01 * n(&a) * n(&b) && n(&a) > 0.01 && n(&b) > 0.01 {
                break (a, b);
            }
        };
        quad(&mut ee, BIKE + 0x50, at);
        quad(&mut ee, BIKE + 0x60, fwd);
        quad(&mut ee, BIKE + 0x70, up);
        let off_by = r(&mut g, 1.5);
        quad(&mut ee, body + 0x1a0, off_by);
        // The node hung (`+0x45` bit 1) is not drawn: state 4's is not, and the unhook runs off it.
        let (on, ride) = (g.one_in(2), !g.one_in(4));
        let b44 = ee.u8_at(BIKE + 0x44) & !1 | u8::from(on);
        set_byte(&mut ee, BIKE + 0x44, u32::from(b44));
        assert_eq!(ee.u8_at(BIKE + 0x45) & 2, 0, "state 4's node is not hung");
        ee.set_u32(BIKE + 0xcc, if ride { riding } else { 0 });
        tally(&mut t, if on { "parts on" } else { "parts left" });
        tally(&mut t, if ride { "riding part" } else { "no riding part" });
        let (before, spr) = (ee.ram.clone(), ee.spr.clone());
        ee.set(28, GP);
        ee.call(GROUND_RESET, &[u64::from(BIKE)], &[0.0], BUDGET);
        let (want, want_spr) = (std::mem::replace(&mut ee.ram, before), std::mem::replace(&mut ee.spr, spr));
        wreck::ground(&mut WreckInRam { ee: &mut ee, x: 0.0 });
        let d = bytes_differ(&want, &ee.ram, ee.stack);
        if !d.is_empty() || want_spr != ee.spr {
            off.push(format!("{k} (on {on}, riding {ride}): RAM differs at {d:?}, scratchpad same {}", want_spr == ee.spr));
        }
    }
    eprintln!("{t:#?}");
    assert!(t.values().all(|n| *n >= 1000), "every path taken: {t:#?}");
    assert!(off.is_empty(), "{} differ:\n  {}", off.len(), off.iter().take(30).cloned().collect::<Vec<_>>().join("\n  "));
}

/// The wreck's oracle's fake world, at `WRECK_AREA`: the bike's vtable, the functions the hooks
/// stand for, the body's, parts' and rumbler's vtables, and the objects the bike's words point at.
const WRECK_AREA: u32 = 0x01c0_0000;
const W_VTABLE: u32 = WRECK_AREA;
const W_FNS: u32 = WRECK_AREA + 0x800;
const W_BODY_VT: u32 = WRECK_AREA + 0x900;
const W_PART_VT: u32 = WRECK_AREA + 0x980;
const W_RUMBLE_VT: u32 = WRECK_AREA + 0x9c0;
const W_DRIVER: u32 = WRECK_AREA + 0x1000;
const W_STATE: u32 = WRECK_AREA + 0x1100;
const W_RECORD: u32 = WRECK_AREA + 0x1800;
const W_OBJECT: u32 = WRECK_AREA + 0x1a40;
const W_STRIKER: u32 = WRECK_AREA + 0x1b40;
const W_RUN: u32 = WRECK_AREA + 0x1c40;
const W_AT: u32 = WRECK_AREA + 0x1d40;
const W_THROW: u32 = WRECK_AREA + 0x1d80;
const W_G: u32 = WRECK_AREA + 0x1e00;
const W_H: u32 = WRECK_AREA + 0x1e40;
const W_PARTS: [u32; 2] = [WRECK_AREA + 0x2100, WRECK_AREA + 0x2140];
const W_KINDS: u32 = WRECK_AREA + 0x2200;
const W_PLACED: u32 = WRECK_AREA + 0x2240;
const W_RUMBLER: u32 = WRECK_AREA + 0x2300;
const W_CONTACTS: u32 = WRECK_AREA + 0x3000;
const W_ACTORS: u32 = WRECK_AREA + 0x4000;
const W_LEN: u32 = 0x5000;

/// What the hooked calls answer: out of bounds, the position's test, the rumble's test, `rand` (a
/// seed and how many it has drawn), the terrain's normal.
#[derive(Clone, Copy, Debug, Default)]
struct Says {
    oob: u32,
    bounds: u32,
    ready: u32,
    seed: u64,
    drawn: u64,
    normal: [u32; 4],
}

impl Says {
    fn rand(&mut self) -> u32 {
        self.drawn += 1;
        let r = mix(self.seed.wrapping_add(self.drawn)) as u32;
        if self.seed & 1 == 0 { r & 0x7fff_ffff } else { r }
    }
}

/// The port's side of the wreck's oracles: words in RAM, the calls logged, the answers [`Says`]'.
struct WreckLogged<'a> {
    ee: &'a mut Ee,
    log: Vec<wreck::Out>,
    says: Says,
}

impl Wrecks for WreckLogged<'_> {
    fn bike(&self) -> u32 {
        BIKE
    }
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn set(&mut self, at: u32, w: u32) {
        self.ee.set_u32(at, w);
    }
    fn normal(&mut self, terrain: u32, at: u32) -> Vf {
        self.log.push(wreck::Out::Normal { terrain, at });
        Vf::from_bits(self.says.normal)
    }
    fn out(&mut self, c: wreck::Out) -> u32 {
        use wreck::Out;
        self.log.push(c);
        match c {
            Out::OutOfBounds => self.says.oob,
            Out::Bounds { .. } => self.says.bounds,
            Out::RumbleReady(_) => self.says.ready,
            Out::Rand => self.says.rand(),
            _ => 0,
        }
    }
}

/// The wreck's oracle rig: the image with every call the wreck makes hooked to log (and answer as
/// [`Says`] says), the bike's vtable, its body's and its parts' the fake ones at [`WRECK_AREA`].
struct WreckRig {
    ee: Ee,
    log: Rc<RefCell<Vec<wreck::Out>>>,
    says: Rc<RefCell<Says>>,
    regions: Vec<(u32, u32)>,
    body: u32,
}

fn wreck_rig() -> Option<WreckRig> {
    use wreck::Out;
    let mut ee = hooked(image()?);
    for (slot, f) in WRECK_OWN {
        assert_eq!(vt(&ee, slot), f, "the bike's vt +{slot:#x}");
    }
    let body = ee.u32_at(BIKE + wreck::BODY);
    for o in (0..W_LEN).step_by(4) {
        ee.set_u32(WRECK_AREA + o, 0);
    }
    let log: Rc<RefCell<Vec<Out>>> = Rc::default();
    let says: Rc<RefCell<Says>> = Rc::default();
    let poses: Rc<RefCell<Vec<u32>>> = Rc::default();
    let stack = ee.stack;
    let mut next = W_FNS;
    let mut hook = |ee: &mut Ee, vtable: Option<(u32, u32)>, at: u32, h: Box<dyn Fn(&mut Ee) -> Out>| {
        let at = match vtable {
            Some((vt, slot)) => {
                let f = next;
                next += 4;
                ee.set_u32(vt + slot, f);
                f
            }
            None => at,
        };
        let l = log.clone();
        ee.hooks.insert(at, Box::new(move |ee: &mut Ee| {
            let c = h(ee);
            l.borrow_mut().push(c);
        }));
    };
    let bike_slot = |ee: &Ee| assert_eq!(ee.r(4) as u32, BIKE, "a bike virtual handed something other than the bike");
    // The bike's own: each logs, and three answer.
    let s = says.clone();
    hook(&mut ee, Some((W_VTABLE, 0x16c)), 0, Box::new(move |ee| {
        bike_slot(ee);
        ee.set(2, u64::from(s.borrow().oob));
        Out::OutOfBounds
    }));
    for (slot, c) in [
        (0x368, Out::Enter(0)),
        (0x36c, Out::Unwreck(0)),
        (0x398, Out::Part(false)),
        (0x48c, Out::ToWreck),
        (0x490, Out::ToRide),
        (PUT_BACK_SLOT, Out::PutBack { snap: 0, kind: 0 }),
        (0x4a8, Out::Settle),
        (0x4b0, Out::Stance(0)),
        (0x3a0, Out::Kind(true)),
        (0x39c, Out::Kind(false)),
        (0x378, Out::Sync),
    ] {
        hook(&mut ee, Some((W_VTABLE, slot)), 0, Box::new(move |ee| {
            bike_slot(ee);
            let (a1, a2) = (ee.r(5) as u32, ee.r(6) as u32);
            match c {
                Out::Enter(_) => Out::Enter(a1),
                Out::Unwreck(_) => Out::Unwreck(a1),
                Out::Part(_) => Out::Part(a1 != 0),
                Out::PutBack { .. } => Out::PutBack { snap: a1, kind: a2 },
                Out::Stance(_) => Out::Stance(a1 as i32),
                c => c,
            }
        }));
    }
    // The body's, the parts', the rumbler's.
    for slot in [0x20, 0x68, 0x60] {
        hook(&mut ee, Some((W_BODY_VT, slot)), 0, Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, body, "the body's virtual's object");
            let at = ee.r(5) as u32;
            if (stack - 0x1_0000..stack).contains(&at) {
                Out::BodyQuad { slot, q: Vf::from_bits([0, 4, 8, 12].map(|k| ee.u32_at(at + k))) }
            } else {
                Out::BodyAt { slot, at }
            }
        }));
    }
    hook(&mut ee, Some((W_PART_VT, 0x20)), 0, Box::new(|ee| Out::PartOn(ee.r(4) as u32 - 0x24)));
    hook(&mut ee, Some((W_PART_VT, 0x18)), 0, Box::new(|ee| Out::PartOff(ee.r(4) as u32 - 0x24)));
    hook(&mut ee, Some((W_RUMBLE_VT, 0x4c)), 0, Box::new(|ee| {
        assert_eq!((ee.r(5), ee.r(6), ee.f(12)), (0, 200, 1.0), "the rumble's level, time and scale");
        Out::Rumble(ee.r(4) as u32)
    }));
    // The functions.
    let s = says.clone();
    hook(&mut ee, None, RAND, Box::new(move |ee| {
        let r = s.borrow_mut().rand();
        ee.set(2, u64::from(r));
        Out::Rand
    }));
    let s = says.clone();
    hook(&mut ee, None, BOUNDS, Box::new(move |ee| {
        assert_eq!((ee.r(8), ee.f(12)), (0, 150.0), "the bounds test's flag and reach");
        ee.set(2, u64::from(s.borrow().bounds));
        Out::Bounds { node: ee.r(4) as u32, at: ee.r(5) as u32 }
    }));
    hook(&mut ee, None, POST_MESSAGE, Box::new(|ee| {
        let want = (u64::from(ee.u32_at(POST_QUEUE)), u64::from(BIKE + 0x674), 8, 0);
        assert_eq!((ee.r(4), ee.r(5), ee.r(6), ee.r(9)), want, "the post's queue, sender, category and last word");
        Out::Post { id: ee.r(7) as u32, arg: ee.r(8) as u32 }
    }));
    let s = says.clone();
    hook(&mut ee, None, RUMBLE_READY, Box::new(move |ee| {
        ee.set(2, u64::from(s.borrow().ready));
        Out::RumbleReady(ee.r(4) as u32)
    }));
    hook(&mut ee, None, UNHOOK, Box::new(|ee| Out::Unhook(ee.r(4) as u32)));
    hook(&mut ee, None, ATTACH, Box::new(|ee| {
        assert_eq!(ee.r(7), 1, "the attach's last argument");
        Out::Attach { part: ee.r(4) as u32, a: ee.r(5) as u32, b: ee.r(6) as u32 }
    }));
    hook(&mut ee, None, ENGINE_RESET, Box::new(|ee| Out::Engine(ee.r(4) as u32)));
    hook(&mut ee, None, WHEEL_RESET, Box::new(|ee| Out::Wheel(ee.r(4) as u32)));
    hook(&mut ee, None, RIDER_SETTLE, Box::new(|ee| {
        assert_eq!((ee.r(6), ee.r(7)), (1, 1), "the rider settle's flags");
        Out::Rider { a: ee.r(4) as u32, b: ee.r(5) as u32 }
    }));
    let p = poses.clone();
    hook(&mut ee, None, BONE_POSE, Box::new(move |ee| {
        assert_eq!(ee.r(5), 0, "the pose's second argument");
        let mut p = p.borrow_mut();
        p.push(ee.r(6) as u32);
        Out::Pose { bone: ee.r(4) as u32, buf: p.len() as u32 - 1 }
    }));
    let p = poses.clone();
    hook(&mut ee, None, BONE_PLACE, Box::new(move |ee| {
        let buf = p.borrow().iter().position(|&b| b == ee.r(5) as u32).map_or(99, |i| i as u32);
        Out::Place { part: ee.r(4) as u32, buf }
    }));
    hook(&mut ee, None, GROUND_RESET, Box::new(move |ee| {
        bike_slot(ee);
        Out::Ground
    }));
    hook(&mut ee, None, TRAIL_RESET, Box::new(|ee| Out::Trail(ee.r(4) as u32)));
    hook(&mut ee, None, CAMERA_RESET, Box::new(|ee| Out::Camera(ee.r(4) as u32)));
    let s = says.clone();
    hook(&mut ee, None, TERRAIN_NORMAL, Box::new(move |ee| {
        assert_eq!((ee.r(7), ee.r(8)), (0, 0), "the normal's last two arguments");
        let out = ee.r(6) as u32;
        for (k, w) in s.borrow().normal.into_iter().enumerate() {
            ee.set_u32(out + 4 * k as u32, w);
        }
        Out::Normal { terrain: ee.r(4) as u32, at: ee.r(5) as u32 }
    }));
    // The fakes in place, and the poses' list cleared with every log.
    ee.set_u32(BIKE + 4, W_VTABLE);
    ee.set_u32(body + 0x110, W_BODY_VT);
    for p in W_PARTS {
        ee.set_u32(p + 0x28, W_PART_VT);
    }
    ee.set_u32(W_RUMBLER + 0x1c, W_RUMBLE_VT);
    let p = poses;
    let l = log.clone();
    ee.hooks.insert(WRECK_AREA + 0x7fc, Box::new(move |_: &mut Ee| {
        p.borrow_mut().clear();
        l.borrow_mut().clear();
    }));
    let regions = vec![
        (BIKE, 0x1800),
        (body, 0x230),
        (WRECK_AREA, W_LEN),
        (wreck::WRECKS_ON, 4),
        (wreck::STRUCK_CLASS, 4),
        (wreck::THROW_SCALE, 4),
    ];
    Some(WreckRig { ee, log, says, regions, body })
}

impl WreckRig {
    /// Runs the original's `f` and then the port's `port` from the same words and answers; asserts
    /// the words and the calls the same, and answers the calls.
    fn check(&mut self, k: usize, f: u32, ints: &[u64], x: f32, port: impl FnOnce(&mut WreckLogged)) -> Vec<wreck::Out> {
        let before = snap_regions(&self.ee, &self.regions);
        let says = *self.says.borrow();
        self.ee.call(WRECK_AREA + 0x7fc, &[], &[], 10);
        self.ee.set(28, GP);
        self.ee.call(f, ints, &[x], BUDGET);
        let want = std::mem::take(&mut *self.log.borrow_mut());
        let want_words = snap_regions(&self.ee, &self.regions);
        restore_regions(&mut self.ee, &self.regions, &before);
        let mut m = WreckLogged { ee: &mut self.ee, log: Vec::new(), says };
        port(&mut m);
        let got = m.log;
        assert_eq!(format!("{got:?}"), format!("{want:?}"), "case {k} ({f:#x}): the calls");
        let got_words = snap_regions(&self.ee, &self.regions);
        if got_words != want_words {
            let names: Vec<(u32, u32)> = self.regions.iter().flat_map(|&(at, len)| (0..len).step_by(4).map(move |o| (at, o))).collect();
            let d: Vec<String> = (0..got_words.len())
                .filter(|&i| got_words[i] != want_words[i])
                .map(|i| format!("{:#x}+{:#x}: port {:#010x}, original {:#010x}", names[i].0, names[i].1, got_words[i], want_words[i]))
                .collect();
            panic!("case {k} ({f:#x}): {} words differ\n  {}", d.len(), d.iter().take(20).cloned().collect::<Vec<_>>().join("\n  "));
        }
        want
    }
}

/// `n` random words at `at`.
fn wreck_words(ee: &mut Ee, g: &mut Gen, at: u32, n: u32) {
    for i in 0..n {
        let x = g.bits();
        ee.set_u32(at + 4 * i, x);
    }
}

fn wreck_quad(ee: &mut Ee, g: &mut Gen, at: u32, r: f32) {
    for i in 0..4 {
        let x = g.near(0.0, -r, r);
        ee.set_f32(at + 4 * i, x);
    }
}

/// **The wreck test is the original's**: 12 000 bikes, each state 4's with every word the wreck test
/// reads drawn at random — the driver and the state's kind, wrecks on or off (the game's and the
/// bike's), the out-of-bounds check, the reason it is down for (mostly none, 13 among them), a
/// position handed in or none (the origin among them) and the two bounds answers; MD63c's limits,
/// loads, pitch and roll, air words, stunt ids, pose blend and up; the record (touching, its object
/// and class), its contacts (mirrored, reaching about the size's 2.9 or 4.5, an actor higher or not
/// than the joust height, or scenery); the striker and its mask; the clock about the 0.5 and 2.5
/// windows less `dt`, `MaxWreckTime`, counting and settled; the run or none, the wreck count — run
/// through SLUS-21104's wreck test and through [`wreck::wreck_test`] from the same words. Every word
/// of the bike, its body and the fake world comes out the same, and the calls (the entries among
/// them, two and three in a frame) are made in the same order with the same arguments. The callees
/// are hooked to log and do nothing.
#[test]
fn the_wreck_test_is_the_originals() {
    let Some(mut rig) = wreck_rig() else { return };
    let image_words = snap_regions(&rig.ee, &rig.regions);
    let mut g = Gen(0x0031_b4b0);
    let mut t = BTreeMap::new();
    for k in 0..12_000 {
        let ee = &mut rig.ee;
        restore_regions(ee, &rig.regions, &image_words);
        let b = |g: &mut Gen, n: u32| u32::from(g.one_in(n));
        let put = |ee: &mut Ee, o: u32, v: f32| ee.set_f32(BIKE + o, v);
        // Who drives, wrecks on, out of bounds, the reason it is down for.
        ee.set_u32(BIKE + 0x48c, if g.one_in(5) { 0 } else { W_DRIVER });
        ee.set_u32(W_DRIVER + 0xe4, if g.one_in(6) { 0 } else { g.bits() | 1 });
        ee.set_u32(BIKE + wreck::STATE, if g.one_in(3) { 0 } else { W_STATE });
        ee.set_u32(W_STATE + 0x28, [1, 1, 0, 3][(g.bits() % 4) as usize]);
        ee.set_u32(wreck::WRECKS_ON, 1 - b(&mut g, 12));
        ee.set_u32(BIKE + 0xc1c, 1 - b(&mut g, 12));
        ee.set_u32(BIKE + 0x308, b(&mut g, 4));
        ee.set_u32(BIKE + 0x3b0, g.bits());
        let reason = if g.one_in(2) { 0 } else { [13, 13, 7, 14, 4, 15, 1, 5][(g.bits() % 8) as usize] };
        ee.set_u32(BIKE + wreck::REASON, reason);
        let at = match g.bits() % 4 {
            0 | 1 => 0,
            2 => wreck::ORIGIN,
            _ => {
                for (i, w) in [0, 0, 0, 0x3f80_0000].into_iter().enumerate() {
                    let w = if g.one_in(3) { w ^ 1 } else { w };
                    ee.set_u32(W_AT + 4 * i as u32, w);
                }
                W_AT
            }
        };
        // MD63c's limits, loads, attitude and air words (about the Free Ride image's limits).
        let lims = [
            (0x13e8, 18.333, 10.0, 30.0),
            (0x13ec, 22.0, 10.0, 30.0),
            (0x13f0, 19.067, 10.0, 30.0),
            (0x13f4, 70.4, 40.0, 90.0),
            (0x13f8, 70.4, 40.0, 90.0),
            (0x13fc, 1.309, 0.8, 1.6),
            (0x1400, 1.309, 0.8, 1.6),
            (0x1404, 1.484, 1.0, 1.7),
            (0x1408, 1.553, 1.0, 1.7),
            (0x140c, 1.553, 1.0, 1.7),
            (0x1410, 29.333, 20.0, 40.0),
            (0x1414, 0.5, 0.2, 0.9),
            (0x1418, 2.0, 0.5, 3.0),
            (0x1420, 0.99, 0.8, 1.0),
            (0x1424, 0.75, 0.3, 1.2),
        ];
        let mut lim = [0.0f32; 16];
        for (i, &(o, at, lo, hi)) in lims.iter().enumerate() {
            lim[i] = g.near(at, lo, hi);
            put(ee, o, lim[i]);
        }
        let calm = g.one_in(3);
        let load = |g: &mut Gen, lo: f32, hi: f32| {
            if calm {
                g.f(-5.0, 5.0)
            } else if g.one_in(14) {
                hi + g.f(0.0, 10.0)
            } else if g.one_in(13) {
                -lo - g.f(0.0, 10.0)
            } else if g.one_in(12) {
                if g.one_in(2) { hi } else { -lo }
            } else {
                g.f(-lo, hi)
            }
        };
        let fore_aft = load(&mut g, lim[0], lim[1]);
        let side = load(&mut g, lim[2], lim[2]);
        put(ee, 0x1258, fore_aft);
        put(ee, 0x1250, side);
        let vertical = if calm { g.f(-5.0, 5.0) } else if g.one_in(4) { g.f(20.0, 100.0) } else { g.f(-20.0, 20.0) };
        put(ee, 0x1254, vertical);
        let on_ground = g.one_in(2);
        let airborne = !on_ground && !g.one_in(4);
        ee.set_u32(BIKE + 0x324, u32::from(on_ground));
        ee.set_u32(BIKE + 0xde0, u32::from(airborne));
        let was = if calm { airborne } else if airborne { g.one_in(3) } else { !g.one_in(3) };
        ee.set_u32(BIKE + 0xde4, u32::from(was));
        let hang = g.near(lim[14], 0.0, 2.0);
        put(ee, 0x1190, hang);
        ee.set_u32(BIKE + 0x1194, if calm { 0 } else { b(&mut g, 6) });
        ee.set_u32(BIKE + 0x11d0, if calm { 0 } else { b(&mut g, 6) });
        let angle = |g: &mut Gen, l: f32| if calm { g.f(-0.5, 0.5) } else if g.one_in(8) { l } else if g.one_in(3) { g.f(-2.0, 2.0) } else { g.f(-0.8, 0.8) };
        let pitch = if g.one_in(2) { angle(&mut g, lim[8]) } else { -angle(&mut g, lim[5]) };
        put(ee, 0x360, pitch);
        let roll = angle(&mut g, lim[9]);
        put(ee, 0x368, roll);
        let crossup = if g.one_in(3) { g.f(0.0, 1.0) } else { g.near(0.5, 0.0, 0.02) };
        put(ee, 0x141c, crossup);
        let balance = 4;
        let id = if calm { 255 } else { [balance, 41, 255, 40, g.bits() % 60, g.bits() % 60][(g.bits() % 6) as usize] };
        let wanted = if calm { 0xfe } else if g.one_in(2) { id } else if g.one_in(2) { 0xfe } else { g.bits() % 60 };
        ee.set_u32(BIKE + 0x1524, id);
        ee.set_u32(BIKE + 0x152c, balance);
        ee.set_u32(BIKE + 0x1530, wanted);
        let blend = g.near(0.95, 0.5, 1.0);
        put(ee, 0x14e4, blend);
        ee.set_u32(BIKE + 0x14cc, b(&mut g, 4));
        ee.set_u32(BIKE + 0x146c, u32::from(g.one_in(2)));
        let up = g.near(lim[11], -1.0, 1.0);
        put(ee, 0xbf4, up);
        wreck_quad(ee, &mut g, BIKE + 0x1260, 1.0);
        let body = rig.body;
        let v = [g.f(-40.0, 40.0), g.f(-20.0, 20.0), g.f(-40.0, 40.0), g.f(-1.0, 1.0)];
        for (i, x) in v.into_iter().enumerate() {
            ee.set_f32(body + 0x80 + 4 * i as u32, if g.one_in(10) { 0.0 } else { x });
        }
        let height = g.f(-5.0, 5.0);
        ee.set_f32(body + 0x74, height);
        // The record, its object, its contacts.
        ee.set_u32(BIKE + wreck::RECORD, W_RECORD);
        let touching = g.one_in(2);
        ee.set_u32(W_RECORD + 0x34, if touching { g.bits() | 1 } else { 0 });
        ee.set_u32(W_RECORD + 0x20c, if g.one_in(3) { 0 } else { W_OBJECT });
        ee.set_u32(W_OBJECT + 0x94, [0x20, 0x10, 0x21, 0][(g.bits() % 4) as usize]);
        ee.set_u32(wreck::STRUCK_CLASS, 0x20);
        let small = g.one_in(2);
        ee.set_u32(BIKE + 0xbc0, if small { 0 } else { 2 });
        let n = if g.one_in(30) { u32::MAX } else { g.bits() % 5 };
        ee.set_u32(W_RECORD + 0xf0, n);
        ee.set_u32(W_RECORD + 0xf8, W_CONTACTS);
        for i in 0..5 {
            let e = W_CONTACTS + 0x90 * i;
            ee.set_u32(e, b(&mut g, 4));
            let reach = if g.one_in(5) { if small { f32::from_bits(0x4039_999a) } else { 4.5 } } else { g.f(0.0, 7.0) };
            ee.set_f32(e + 0x24, reach);
            let actor = W_ACTORS + 0x200 * i;
            ee.set_u32(e + 0x84, if g.one_in(2) { 0 } else { actor });
            ee.set_u32(actor + 0xe4, actor + 0x100);
            let theirs = if g.one_in(5) { height + lim[12] } else { height + g.f(-4.0, 6.0) };
            ee.set_f32(actor + 0x174, theirs);
            wreck_quad(ee, &mut g, actor + 0x180, 30.0);
        }
        // The striker, the clock, the run.
        ee.set_u32(BIKE + 0x1440, if g.one_in(4) { W_STRIKER } else { 0 });
        ee.set_u32(W_STRIKER + 0x94, g.bits() & 0x0f);
        ee.set_u32(BIKE + 0x1110, g.bits() & 0x0f);
        let dt = if g.one_in(2) { f32::from_bits(DT) } else { g.f(0.002, 0.05) };
        let window = if reason == 13 { 2.5 } else { 0.5 };
        let max = g.near(3.0, 0.0, 6.0);
        put(ee, wreck::MAX_WRECK_TIME, max);
        let clock = match g.bits() % 4 {
            0 => vu_sub(window, dt),
            1 => vu_sub(max, dt),
            _ => g.near(0.0, 0.0, 6.0),
        };
        put(ee, wreck::CLOCK, clock);
        ee.set_u32(BIKE + wreck::COUNTING, u32::from(g.one_in(2)));
        ee.set_u32(BIKE + wreck::SETTLES, 1 - b(&mut g, 3));
        ee.set_u32(BIKE + wreck::RUN, if g.one_in(4) { 0 } else { W_RUN });
        wreck_words(ee, &mut g, W_RUN + 8, 2);
        wreck_words(ee, &mut g, BIKE + 0x6c4, 1);
        let seed = u64::from(g.bits());
        *rig.says.borrow_mut() = Says { oob: b(&mut g, 8), bounds: b(&mut g, 3), seed, ..Says::default() };

        let calls = rig.check(k, WRECK_TEST, &[u64::from(BIKE), u64::from(at)], dt, |m| wreck::wreck_test(m, at, F(dt)));

        let enters: Vec<u32> = calls.iter().filter_map(|c| if let wreck::Out::Enter(r) = c { Some(*r) } else { None }).collect();
        if enters.is_empty() {
            tally(&mut t, "no entry");
        }
        for r in 1..16 {
            if enters.contains(&r) {
                tally(&mut t, ["", "1 front", "2 left", "3 right", "4 back", "5 flat or struck", "6 scenery", "7 joust", "8 nose dive", "9", "10", "11 tail", "12 no-hander", "13 out of bounds", "14 no control", "15 struck"][r as usize]);
            }
        }
        assert!(enters.iter().all(|r| ![0, 9, 10].contains(r) && *r < 16), "case {k}: entered {enters:?}");
        if enters.len() == 2 {
            tally(&mut t, "two entries in a frame");
        }
        if enters.len() > 2 {
            tally(&mut t, "three entries in a frame");
        }
        if calls.contains(&wreck::Out::Post { id: 12, arg: 0 }) {
            tally(&mut t, "the count armed");
        }
        if calls.contains(&wreck::Out::Unwreck(1)) {
            tally(&mut t, "unwrecked");
        }
        if calls.iter().any(|c| matches!(c, wreck::Out::Bounds { .. })) {
            tally(&mut t, "a position's bounds asked");
        }
    }
    eprintln!("{t:#?}");
    for (path, n) in &t {
        assert!(*n >= 20, "path {path} taken only {n} times");
    }
    assert!(t.len() >= 19, "every path taken: {t:#?}");
}

#[allow(clippy::float_arithmetic)]
fn vu_sub(a: f32, b: f32) -> f32 {
    a - b
}

/// **The wreck's virtuals are the original's**: 12 000 calls, each one of the entry (reasons 13, 0
/// and others), the unwreck (kind 0 or 1, down for 13 or not), the riding part on or off, the record's
/// part to the wreck one or back, and the put-back (snap and kind each 0 or 1), on state 4's bike with
/// every word it reads drawn at random — the state, its kind, rumbler and the rumble's answer; the
/// size word (0, 1, 2, 3); the damping quadwords and the post's word; the throw's target and scale;
/// the 0x1778 chain (each link there or not); the part bits, parts and their `+0x20`; the flags byte;
/// the put-back's spots, the run's two spots (each word there or not), the terrain and its normal, the
/// trail, the bone list (counts −1 to 4, kinds 0, 1 and 2) — through SLUS-21104's and the port's. Every
/// word of the bike, its body and the fake world comes out the same, and the calls are the same. The
/// callees are hooked to log and do nothing (`rand` and the tests answer).
#[test]
fn the_wrecks_virtuals_are_the_originals() {
    let Some(mut rig) = wreck_rig() else { return };
    let image_words = snap_regions(&rig.ee, &rig.regions);
    let mut g = Gen(0x0031_b4b1);
    let mut t = BTreeMap::new();
    for k in 0..12_000 {
        let ee = &mut rig.ee;
        restore_regions(ee, &rig.regions, &image_words);
        let b = |g: &mut Gen, n: u32| u32::from(g.one_in(n));
        let bike = u64::from(BIKE);
        // The words they all may read.
        ee.set_u32(BIKE + wreck::STATE, if g.one_in(4) { 0 } else { W_STATE });
        ee.set_u32(W_STATE + 0x28, [1, 1, 0, 3][(g.bits() % 4) as usize]);
        ee.set_u32(W_STATE + 0x5c, if g.one_in(4) { 0 } else { W_RUMBLER });
        wreck_words(ee, &mut g, W_STATE + 0x6ac, 1);
        ee.set_u32(BIKE + 0xbbc, g.bits() % 4);
        for o in [0xca0, 0xcb0, 0xcc0, 0xd40, 0xd50, 0xd60, 0x50] {
            wreck_quad(ee, &mut g, BIKE + o, 10.0);
        }
        for o in [0xd20, 0x6e4, 0x142c, 0xb4, 0xc0, 0x17b0, 0x17b4, 0x1480, 0x1284, 0x1104, 0x1108, 0xdec, 0x173c, 0x1740, 0xd78] {
            wreck_words(ee, &mut g, BIKE + o, 1);
        }
        let reason = if g.one_in(2) { 13 } else { g.bits() % 16 };
        ee.set_u32(BIKE + wreck::REASON, reason);
        ee.set_u32(BIKE + 0xe8, W_THROW);
        let throw = g.f(-5.0, 5.0);
        ee.set_f32(W_THROW + 4, throw);
        let scale = if g.one_in(10) { 0.0 } else { g.f(0.5, 3.0) };
        ee.set_f32(wreck::THROW_SCALE, scale);
        ee.set_u32(BIKE + 0x1778, if g.one_in(3) { 0 } else { W_G });
        ee.set_u32(W_G + 0x34, if g.one_in(3) { 0 } else { W_H });
        ee.set_u32(BIKE + wreck::RECORD, W_RECORD);
        ee.set_u32(BIKE + wreck::RUN, if g.one_in(4) { 0 } else { W_RUN });
        wreck_words(ee, &mut g, W_RUN + 8, 2);
        ee.set_u32(BIKE + 0xd70, if g.one_in(3) { 0 } else { g.bits() | 1 });
        ee.set_u32(BIKE + 0xd74, if g.one_in(3) { 0 } else { g.bits() | 1 });
        ee.set_u32(BIKE + 0x1738, if g.one_in(3) { 0 } else { g.bits() | 1 });
        // The parts.
        let flags = g.bits();
        ee.set_u32(BIKE + 0x44, flags);
        ee.set_u32(BIKE + 0xd0, if g.one_in(4) { 0 } else { W_PARTS[(g.bits() % 2) as usize] });
        let [wp, rp] = if g.one_in(2) { W_PARTS } else { [W_PARTS[1], W_PARTS[0]] };
        ee.set_u32(BIKE + wreck::WRECK_PART, wp);
        ee.set_u32(BIKE + wreck::RIDE_PART, rp);
        for p in W_PARTS {
            ee.set_u32(p + 0x20, b(&mut g, 3));
        }
        // The put-back's terrain, trail and bones.
        ee.set_u32(BIKE + 0x3b4, if g.one_in(3) { 0 } else { g.bits() | 1 });
        ee.set_u32(BIKE + 0x10dc, if g.one_in(3) { 0 } else { g.bits() | 1 });
        let n = if g.one_in(20) { u32::MAX } else { g.bits() % 5 };
        ee.set_u32(BIKE + 0x1470, n);
        ee.set_u32(BIKE + 0x1478, W_KINDS);
        ee.set_u32(BIKE + 0x1474, W_PLACED);
        for i in 0..4 {
            ee.set_u32(W_KINDS + 4 * i, g.bits() % 3);
            wreck_words(ee, &mut g, W_PLACED + 4 * i, 1);
        }
        let mut normal = [0u32; 4];
        for x in &mut normal {
            *x = g.f(-1.0, 1.0).to_bits();
        }
        let seed = u64::from(g.bits());
        *rig.says.borrow_mut() = Says { ready: b(&mut g, 2), seed, normal, ..Says::default() };
        let (snap, kind) = (u32::from(g.one_in(2)), u32::from(g.one_in(2)));
        let x = g.f(0.0, 0.05);

        let (path, calls) = match g.bits() % 6 {
            0 => {
                let r = [13, 13, 0, 1, 5, 7, 14, 15][(g.bits() % 8) as usize];
                let c = rig.check(k, WRECK_ENTER, &[bike, u64::from(r)], x, |m| wreck::enter(m, r));
                (if r == 13 { "entry, 13" } else if r == 0 { "entry, 0" } else { "entry" }, c)
            }
            1 => {
                let c = rig.check(k, UNWRECK, &[bike, u64::from(kind)], x, |m| wreck::unwreck(m, kind));
                (if reason == 13 { "unwreck, 13" } else { "unwreck" }, c)
            }
            2 => {
                let on = g.one_in(2);
                let c = rig.check(k, PART_TOGGLE, &[bike, u64::from(on)], x, |m| wreck::part(m, on));
                (if on { "part on" } else { "part off" }, c)
            }
            3 | 4 => {
                let to = g.one_in(2);
                let c = rig.check(k, if to { TO_WRECK } else { TO_RIDE }, &[bike], x, |m| wreck::swap(m, to));
                (if to { "to the wreck part" } else { "to the riding part" }, c)
            }
            _ => {
                let c = rig.check(k, PUT_BACK, &[bike, u64::from(snap), u64::from(kind)], x, |m| wreck::put_back(m, snap, kind));
                (if snap != 0 && calls_normal(&c) { "put back, snapped" } else { "put back" }, c)
            }
        };
        tally(&mut t, path);
        use wreck::Out;
        for (c, name) in [
            (Out::Rand, "rand drawn"),
            (Out::Ground, "ground reset"),
        ] {
            if calls.contains(&c) {
                tally(&mut t, name);
            }
        }
        if calls.iter().any(|c| matches!(c, Out::Rumble(_))) {
            tally(&mut t, "rumbled");
        }
        if calls.iter().any(|c| matches!(c, Out::BodyQuad { .. })) {
            tally(&mut t, "the body's wreck damping");
        }
        if calls.iter().any(|c| matches!(c, Out::Attach { .. })) {
            tally(&mut t, "a part attached");
        }
        if calls.iter().any(|c| matches!(c, Out::Place { .. })) {
            tally(&mut t, "a bone placed");
        }
        if calls.iter().any(|c| matches!(c, Out::Trail(_))) {
            tally(&mut t, "the trail reset");
        }
    }
    eprintln!("{t:#?}");
    for (path, n) in &t {
        assert!(*n >= 20, "path {path} taken only {n} times");
    }
    assert!(t.len() >= 18, "every path taken: {t:#?}");
}

fn calls_normal(c: &[wreck::Out]) -> bool {
    c.iter().any(|c| matches!(c, wreck::Out::Normal { .. }))
}

// --- F39: the vehicle update's callees -----------------------------------------------------------------

use game_sim::air::AcosTable;
use game_sim::handling::{anim, attach, node};
use game_sim::toys::Turns;
use game_sim::handling::update::{self, Updates};

/// The vehicle update's callees in SLUS-21104 (research/oracle.txt `# F39`): the bike's slots (the
/// prepare's first call is a thunk to its held states' half), then the deeper calls they make.
const FADE_FN: u32 = 0x004e_1a20;
const LEAN_FN: u32 = 0x004e_5d20;
const PREPARE_FN: u32 = 0x004d_d8f0;
const PREPARE_THUNK: u32 = 0x004e_7bb0;
const PREPARE_HELD: u32 = 0x004a_8350;
const RIDE_FN: u32 = 0x004a_7b70;
const SYNC_FN: u32 = 0x004e_2f50;
const SNAPSHOT_FN: u32 = 0x004d_9f30;
const ANIM_STOP: u32 = 0x0013_e650;
const ANIM_PLAY: u32 = 0x0013_eec0;
const ANIM_BLEND: u32 = 0x0013_f5e0;
const ANIM_BLEND4: u32 = 0x0014_0720;
/// F43a's: the two channel kinds' samplers (a rotation's, a position's: the vtable's `+0x10` and
/// `+0x14`), and the heap the stop makes its two saved-pose buffers from (with the source file's name
/// and the two lines it passes).
const ANIM_TURNS: u32 = 0x0012_c550;
const ANIM_MOVES: u32 = 0x0012_db50;
const HEAP_ALLOC: u32 = 0x001b_7340;
const HEAP_FILE: u32 = 0x0039_7180;
const NODE_SPIN: u32 = 0x0019_5210;
const NODE_AIM: u32 = 0x0019_41b0;
const NODE_AXES: u32 = 0x0019_4960;
const NODE_LOCAL: u32 = 0x0019_5320;
const ATTACH_MOVED: u32 = 0x0026_6d30;
/// F43b's: the attachment move's database (research/oracle.txt `# F43b`): the list's and the tree's
/// vtables, the tree node's class and the one it passes through, the link pool's and the current
/// tree's globals, and the heap's calls with the file names and lines each site passes.
const LIST_VT: u32 = 0x003b_5510;
const TREE_VT: u32 = 0x003b_6700;
const TREE_NODE_VT: u32 = 0x003b_66e0;
const TREE_NODE_BASE: u32 = 0x003b_52b0;
const LINK_POOL: u32 = 0x0040_c460;
const CURRENT_TREE: u32 = 0x0040_c198;
const HEAP_ALLOC_IN: u32 = 0x001b_73d0;
const HEAP_REALLOC: u32 = 0x001b_7470;
const HEAP_NEW: u32 = 0x001b_7620;
const HEAP_FREE: u32 = 0x001b_76d0;
const LIST_FILE: u32 = 0x0039_99f8;
const POOL_FILE: u32 = 0x0039_66e0;
const TREE_FILE: u32 = 0x0039_98a8;
const NODE_FILE: u32 = 0x0039_a520;
const ATTACH_GLOBALS: attach::Globals = attach::Globals { links: LINK_POOL, current: CURRENT_TREE, node_class: TREE_NODE_VT, node_base: TREE_NODE_BASE };
/// F42's: the bike's gear, trick step, net check and net unpack, and the player's stunt mark and the
/// vehicle's end of the unpack they call (research/oracle.txt `# F42`).
const GEAR_FN: u32 = 0x004d_aab0;
const TRICK_FN: u32 = 0x004e_49c0;
const NET_CHECK_FN: u32 = 0x004a_86f0;
const NET_UNPACK_FN: u32 = 0x004d_d490;
const MARK_FN: u32 = 0x0046_b110;
/// F31b5f's: the clip held at its end (vt `+0x4ec`).
const TRICK_HOLD_FN: u32 = 0x004e_4810;
thread_local! {
    /// F45 — the calls [`UpdateInRam`] served with the ports since the last take, by name.
    static F45_SERVED: RefCell<BTreeMap<&'static str, u32>> = RefCell::default();
}

/// The bike's remote stunt clocks (vt `+0x4e8`) and trick wanted (vt `+0x4b4`), F45's.
const STUNT_CLOCKS_FN: u32 = 0x004e_4900;
const TRICK_WANTED_FN: u32 = 0x004e_47a0;
const NET_POST: u32 = 0x004a_8c40;
/// F44's: the scene node's on, off, shown and hidden (the trail's vt `+0x18`, `+0x20`, `+0x28`,
/// `+0x2c`), the net sender's send (a thunk to the bike's), and the record's and the second part's
/// steps (research/oracle.txt `# F44`).
const NODE_ON: u32 = 0x0012_8040;
const NODE_OFF: u32 = 0x0012_8080;
const NODE_SHOWN: u32 = 0x0012_7f60;
const NODE_HIDDEN: u32 = 0x0012_7fd0;
const SEND_THUNK: u32 = 0x004b_fa30;
const PART_STEPS: [u32; 2] = [0x001b_cfa0, 0x001b_acd0];
/// F44a's: the bike's three packets (vt `+0x544`, `+0x400`, `+0x404`), the sender's own send (its vt
/// `+0x28`) and the connection's queue it ends in, the quantiser table's lookup, keep and quantise,
/// state 4's hash and key test, the interrupts off and on (research/oracle.txt `# F44a`).
const STUNT_FN: u32 = 0x004e_7e70;
const FULL_FN: u32 = 0x004a_8e30;
const DELTA_FN: u32 = 0x004a_90c0;
const SENDER_SEND: u32 = 0x0025_e9d0;
const NET_QUEUE: u32 = 0x0028_2190;
/// F31b5k's: the socket send a full queue goes out by.
const NET_FLUSH: u32 = 0x0028_45f0;
const Q_LOOKUP: u32 = 0x0026_7ba0;
const Q_KEEP: u32 = 0x0026_7910;
const Q_QUANTISE: u32 = 0x0026_7590;
const KEY_HASH: u32 = 0x001c_3820;
const KEY_SAME: u32 = 0x001c_38d0;
const IRQ_OFF: u32 = 0x0034_8fa0;
const IRQ_ON: u32 = 0x0034_8ff0;
/// The stunt packet's two instructions that read RCNT0, and the scratch slots of the counter's
/// stand-in word, the queue's header and its packet.
const COUNTER_LUI: u32 = 0x004e_7f44;
const COUNTER_LW: u32 = 0x004e_7f50;
const COUNTER_K: u32 = 250;
const HEAD_K: u32 = 251;
const PACKET_K: u32 = 252;
/// The bike's slots F39 ports, each with its function.
const UPDATE_OWN: [(u32, u32); 6] = [
    (0x3a4, FADE_FN),
    (0x4f0, LEAN_FN),
    (0x3e4, PREPARE_FN),
    (0x35c, RIDE_FN),
    (0x378, SYNC_FN),
    (0x37c, SNAPSHOT_FN),
];
/// The scratch slots the port's calls hand out: the two bone buffers.
const U_POSES: u32 = 73;

/// The leaves the vehicle update's part walk ([`update::Out::Walk`]: the bike's own listeners, the
/// collision record's and the second part's) reaches on state 4, by their step, as (step, whether the
/// step ends with the walk of its own listeners, name): F31b5j. None is the bike's code, and none
/// writes a word of the bike or its physics blocks ([`PartWalk`]); but the effects and the emitters
/// draw on `rand`, and through it reach the bike ([`flip_needs_the_part_walks_effects`]). The walks
/// reach fourteen: one each of the first two, eleven of the third and one of the fourth.
///
/// - The models: flips its `+0x58` between 0 and 1 every frame, then for each entry of its list at
///   `+0x228` (count `+0x234`) that is set and not yet attached, attaches the models it names: a
///   scene object's frame, no simulation.
/// - The effects: sets a flags byte on each of its table's entries (`+0x24`, 0x2c bytes each) from
///   the words they point at, tells its items (`+0x28`) of a changed key, turns each item on or off by
///   those flags (vt `+0x18`, `+0x1c`) and steps the ones on (vt `+0x14`, `dt`), which draws on
///   `rand` now and then.
/// - The emitters, two kinds (a listener at its object's `+0xc`, its step a thunk to the object's):
///   from the emitter's two spots' distance over `dt` each spawns particles, drawing on the C
///   library's `rand` (the one the wreck's throw and the rider's reset draw on) for their counts and
///   spreads.
const PART_LEAVES: [(u32, bool, &str); 4] =
    [(0x0014_1d30, true, "models"), (0x004f_a4a0, true, "effects"), (0x0051_1c10, false, "emitter"), (0x004f_fd20, false, "second emitter")];

/// The vehicle update's part walk, the port's [`tail::walk`] on RAM: a group's step (the walk itself)
/// is the walk one level down, and each leaf ([`PART_LEAVES`]) is the original's, its closing walk
/// stood in for by the port's; F31b5n: but the effects' own work is [`effects::step`]'s, only its
/// items' virtuals the original's. Every leaf step (and effects item call) is watched: it may not change a word of the bike or
/// its physics blocks; the steps that move `rand`'s state, and any that builds the smooth noise, are
/// counted. With `"part leaves dropped"` in [`IN_RUST`] the leaves do not step at all; with `"part
/// leaf dropped: <name>"` that kind's own work is dropped and its walk kept.
struct PartWalk<'a> {
    ee: &'a mut Ee,
}

impl Listeners for PartWalk<'_> {
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn step(&mut self, e: u32, dt: F) {
        let f = self.ee.u32_at(self.ee.u32_at(e + 4) + 0x40);
        if f == TAIL {
            return tail::walk(self, e, dt);
        }
        let Some(&(_, closes, name)) = PART_LEAVES.iter().find(|l| l.0 == f) else {
            panic!("a part leaf other than the known: {e:#x}, step {f:#x}")
        };
        if in_rust("part leaves dropped") {
            return;
        }
        if in_rust(&format!("part leaf dropped: {name}")) {
            if closes {
                tail::walk(self, e, dt);
            }
            return;
        }
        if name == "effects" {
            effects::step(self, e, dt);
        } else {
            self.original(f, &[u64::from(e)], &[dt.0], name, if closes { vec![(e, dt.to_bits())] } else { vec![] });
        }
        if closes {
            tail::walk(self, e, dt);
        }
    }
}

impl PartWalk<'_> {
    /// The original's `f` run on `ints` and `floats`, booked under `name`, its closing walk (`TAIL`)
    /// hooked and answered as `walks` ((the group, `dt`'s bits) each), and watched: it may not change a
    /// word of the bike or its blocks; a step that moved `rand`'s state or built the smooth noise is
    /// counted.
    fn original(&mut self, f: u32, ints: &[u64], floats: &[f32], name: &str, walks: Vec<(u32, u32)>) {
        let (before, rand, noise_unbuilt) = (bike_and_blocks(self.ee), rand_state(self.ee), self.ee.u32_at(NOISE_UNBUILT) != 0);
        let walked: Rc<RefCell<Vec<(u32, u32)>>> = Rc::default();
        let w = walked.clone();
        let old = self.ee.hooks.insert(TAIL, Box::new(move |ee: &mut Ee| w.borrow_mut().push((ee.r(4) as u32, ee.f(12).to_bits()))));
        self.ee.set(28, GP);
        let r0 = self.ee.ran;
        self.ee.call(f, ints, floats, BUDGET);
        book(format!("the tail: the update's {name}"), self.ee.ran - r0);
        match old {
            Some(h) => self.ee.hooks.insert(TAIL, h),
            None => self.ee.hooks.remove(&TAIL),
        };
        if rand_state(self.ee) != rand {
            book(format!("the tail: the update's {name} steps that drew rand"), 1);
        }
        if noise_unbuilt && self.ee.u32_at(NOISE_UNBUILT) == 0 {
            book(format!("the tail: the update's {name} steps that built the noise"), 1);
        }
        assert_eq!(*walked.borrow(), walks, "{name} {ints:#x?}'s closing walk");
        let parted: Vec<String> =
            before.iter().zip(&bike_and_blocks(self.ee)).filter(|(b, a)| b != a).map(|((at, b), (_, a))| format!("{at:#x}: {b:#010x} -> {a:#010x}")).collect();
        assert!(parted.is_empty(), "{name} {ints:#x?} (fn {f:#x}) wrote the bike's words:\n  {}", parted.join("\n  "));
    }
}

/// F31b5n: the effects' own work is the port's ([`effects::step`]); its items' virtuals are the
/// original's, each booked by kind (`the update's effects item <vt slot> <fn>`) and watched as a
/// leaf is.
impl Effects for PartWalk<'_> {
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn set_byte(&mut self, at: u32, b: u8) {
        self.ee.write(at, 1, u128::from(b));
    }
    fn out(&mut self, c: effects::Out, dt: F) {
        use effects::Out;
        let (item, slot, ints, floats): (u32, u32, Vec<u64>, Vec<f32>) = match c {
            Out::Key { item, key } => (item, 8, vec![u64::from(item), u64::from(key)], vec![]),
            Out::On(item) => (item, 0x18, vec![u64::from(item)], vec![]),
            Out::Off(item) => (item, 0x1c, vec![u64::from(item)], vec![]),
            Out::Step(item) => (item, 0x14, vec![u64::from(item)], vec![dt.0]),
        };
        let f = self.ee.u32_at(self.ee.u32_at(item + 0x10) + slot);
        self.original(f, &ints, &floats, &format!("effects item +{slot:#x} {f:#x}"), vec![]);
        book("the tail: the update's effects item calls", 1);
    }
}

/// The bike in RAM as the update's callees' port sees it: every word read and written in place, each
/// call out the original's.
struct UpdateInRam<'a> {
    ee: &'a mut Ee,
    /// What `$f12` is handed with the calls that take no float of their own (`dt`).
    x: f32,
}

impl UpdateInRam<'_> {
    fn call(&mut self, f: u32, ints: &[u64], floats: &[f32]) -> u32 {
        self.ee.set(28, GP);
        census(self.ee, |ee| ee.call(f, ints, floats, BUDGET));
        self.ee.r(2) as u32
    }
    fn own(&self, slot: u32) {
        let f = UPDATE_OWN.iter().find(|s| s.0 == slot).unwrap().1;
        assert_eq!(vt(self.ee, slot), f, "the bike's vt +{slot:#x}");
    }
}

impl Updates for UpdateInRam<'_> {
    fn bike(&self) -> u32 {
        BIKE
    }
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn set(&mut self, at: u32, w: u32) {
        self.ee.set_u32(at, w);
    }
    fn out(&mut self, c: update::Out) -> u32 {
        use update::Out;
        label(&format!("update {c:?}"));
        // F42's four, the port's too.
        for (k, slot, f) in [(Out::Gear, 0x358, GEAR_FN), (Out::Trick, 0x4e4, TRICK_FN), (Out::NetCheck, 0x3e8, NET_CHECK_FN), (Out::NetUnpack, 0x3f0, NET_UNPACK_FN)] {
            if c == k {
                assert_eq!(vt(self.ee, slot), f, "the bike's vt +{slot:#x}");
            }
        }
        let dt = F(self.x);
        // F31b5j: the part walk, the port's ([`PartWalk`]); with "the walk handed on", the
        // original's.
        if let Out::Walk(g) = c
            && !in_rust("the walk handed on")
        {
            tail::walk(&mut PartWalk { ee: &mut *self.ee }, g, dt);
            return 0;
        }
        // F31b5l: the wreck fade's world search, the port's ([`fade_search_in_ram`]); what it hands
        // on (a plant part stood, a hearer) booked by kind.
        if let Out::Search(rec) = c
            && in_rust("fade search")
        {
            let (r0, mut kinds) = (self.ee.ran, BTreeMap::new());
            fade_search_in_ram(self.ee, rec, &mut kinds);
            let handed: Vec<&str> = kinds.keys().filter_map(|k| k.strip_prefix("> ")).collect();
            book(format!("the tail: the fade search hands on {}", handed.join(", ")), self.ee.ran - r0);
            return 0;
        }
        // F31b5l: the kind clear and the bike shown, the port's.
        match c {
            Out::KindClear if in_rust("kind clear") => {
                assert_eq!(vt(self.ee, 0x39c), KIND_CLEAR_FN, "the bike's vt +0x39c");
                update::kind_clear(self);
                return 0;
            }
            Out::Show => {
                assert_eq!(vt(self.ee, 0x398), SHOW_FN, "the bike's vt +0x398");
                update::show(self);
                return 0;
            }
            _ => {}
        }
        match c {
            Out::Gear => return update::gear(self),
            Out::Trick => return update::trick(self, dt),
            Out::NetCheck => return update::net_check(self),
            Out::NetUnpack => {
                update::net_unpack(self, dt);
                return 0;
            }
            // F31b5f's.
            Out::TrickHold(entry) => {
                assert_eq!(vt(self.ee, 0x4ec), TRICK_HOLD_FN, "the bike's vt +0x4ec");
                update::trick_hold(self, entry, dt);
                return 0;
            }
            _ => {}
        }
        // F43a's animation controller, the port's.
        match c {
            Out::AnimStop(ctrl) => {
                anim::stop(&mut NodeRam(self.ee), ctrl);
                return 0;
            }
            Out::AnimPlay { clip, .. } => kinds(self.ee, &[clip]),
            Out::AnimBlend { clips, .. } => kinds(self.ee, &clips),
            _ => {}
        }
        let t = anim::Tables { turns: Turns::get(), acos: acos() };
        match c {
            Out::AnimPlay { ctrl, clip, weight, fade } => {
                anim::play(&mut NodeRam(self.ee), &t, ctrl, clip, F::from_bits(weight), F::from_bits(fade));
                return 0;
            }
            Out::AnimBlend { ctrl, clips, weight, mix, fade } => {
                anim::blend(&mut NodeRam(self.ee), &t, ctrl, clips, F::from_bits(weight), F::from_bits(mix), F::from_bits(fade));
                return 0;
            }
            _ => {}
        }
        // F43b's: the attachment moved in its database, the port's.
        if let Out::Moved(a) = c {
            attach::moved(&mut NodeRam(self.ee), &ATTACH_GLOBALS, a);
            return 0;
        }
        // F43's scene-node calls, the port's.
        let pose = match c {
            Out::Pose { buf, .. } | Out::Place { buf, .. } => self.ee.scratch(U_POSES + buf),
            _ => 0,
        };
        let n = &mut NodeRam(self.ee);
        match c {
            Out::NodeSet { node, at } => {
                node::set(n, node, at);
                return 0;
            }
            Out::Spin { node, axis, angle } => {
                node::spin(n, node, axis, F::from_bits(angle), curve());
                return 0;
            }
            Out::Aim { node, dir, up } => {
                let up = [0, 4, 8, 12].map(|k| n.0.u32_at(up + k));
                node::aim(n, node, dir, up);
                return 0;
            }
            Out::Pose { node, .. } => {
                node::frame(n, node, pose);
                return 0;
            }
            Out::Place { part, .. } => {
                node::set_frame(n, part, pose);
                return 0;
            }
            Out::Frame { node, at } => {
                node::frame(n, node, at);
                return 0;
            }
            Out::Position { node, at } => {
                node::position(n, node, at);
                return 0;
            }
            Out::Axes { node, z, y } => {
                node::axes(n, node, z, y);
                return 0;
            }
            _ => {}
        }
        // F44's: the port where the object's slot holds the original it stands for.
        let ee = &*self.ee;
        let slot = |o: u32, at: u32, slot: u32| ee.u32_at(ee.u32_at(o + at) + slot);
        match c {
            Out::TrailOn(t) if slot(t, 4, 0x18) == NODE_ON => return void(update::trail_on(self, t)),
            Out::TrailOff(t) if slot(t, 4, 0x20) == NODE_OFF => return void(update::trail_off(self, t)),
            Out::Shown(n) if slot(n, 4, 0x28) == NODE_SHOWN => return void(update::shown(self, n)),
            Out::Hidden(n) if slot(n, 4, 0x2c) == NODE_HIDDEN => return void(update::hidden(self, n)),
            Out::Send { net, arg } if net == BIKE + update::NET && slot(net, 0x38, 0x20) == SEND_THUNK => return update::send(self, arg),
            Out::Part(p) if PART_STEPS.contains(&slot(p, 0xc, 0xbc)) => return void(update::part_step(self, p)),
            _ => {}
        }
        // F44a's: the same, where no oracle hooks the original.
        let ported = |f: u32| !ee.hooks.contains_key(&f);
        match c {
            Out::NetStunt(a) if vt(ee, 0x544) == STUNT_FN && ported(STUNT_FN) => return void(update::net_stunt(self, a)),
            Out::NetFull(a) if vt(ee, 0x400) == FULL_FN && ported(FULL_FN) => return void(update::net_full(self, a)),
            Out::NetDelta(a) if vt(ee, 0x404) == DELTA_FN && ported(DELTA_FN) => return void(update::net_delta(self, a)),
            Out::SenderSend { sender, arg, packet } if slot(sender, 0x38, 0x28) == SENDER_SEND && ported(SENDER_SEND) => {
                return void(update::sender_send(self, sender, arg, packet, 0, 0));
            }
            Out::Hash { f, key } if f == KEY_HASH && ported(KEY_HASH) => return update::decimal_hash(self, key),
            Out::SameKey { f, a, b } if f == KEY_SAME && ported(KEY_SAME) => return update::same_key(a, b),
            // F31b5k's: the connection's queue, its socket send the original's.
            Out::NetQueue { conn, head, packet, flags } if ported(NET_QUEUE) => return update::net_queue(self, conn, head, &packet, flags),
            // RCNT0 is hardware: its stand-in word, as the original's patched read ([`hooked`]).
            Out::Counter => return ee.u32_at(ee.scratch(COUNTER_K)),
            _ => {}
        }
        // F45's: the trick step's calls and the unpack's node turn, the ports where the bike's slot holds
        // the original and no oracle hooks it; the run's calls the stunt's start and end make [`run`]'s,
        // their event posts the original's ([`RunHanded`]).
        let served = |what: &'static str| F45_SERVED.with(|s| *s.borrow_mut().entry(what).or_default() += 1);
        match c {
            Out::TrickRemote if vt(ee, 0x4e8) == STUNT_CLOCKS_FN && ported(STUNT_CLOCKS_FN) => {
                served("TrickRemote");
                return update::stunt_clocks(self, dt);
            }
            Out::TrickWanted if vt(ee, 0x4b4) == TRICK_WANTED_FN && ported(TRICK_WANTED_FN) => {
                served("TrickWanted");
                return update::trick_wanted(self);
            }
            Out::TrickEnd(id) if vt(ee, 0x4ac) == TRICK_END_FN && ported(TRICK_END_FN) => {
                served("TrickEnd");
                return update::trick_end(self, id);
            }
            Out::TrickStart(id) if vt(ee, 0x4b8) == STUNT_START_FN && ported(STUNT_START_FN) => {
                served("TrickStart");
                return update::stunt_start(self, id, 0);
            }
            Out::Mark { .. } if ported(MARK_FN) => {
                served("Mark");
                return serve_run(&mut RunHanded { ee: &mut *self.ee }, c);
            }
            Out::RunOpen(_) | Out::RunScore { .. } | Out::RunClose { .. } | Out::RunTally { .. } | Out::RunEnd { .. } => {
                served("Run");
                return serve_run(&mut RunHanded { ee: &mut *self.ee }, c);
            }
            Out::Orient { node, at, pos } if ported(NODE_PLACE) => {
                served("Orient");
                let (q, p) = ([0, 4, 8, 12].map(|k| ee.u32_at(at + k)), [0, 4, 8, 12].map(|k| ee.u32_at(pos + k)));
                node::orient(&mut NodeRam(self.ee), node, q, p);
                return 0;
            }
            // F45a's: the net interpolate, its put-back the bike's own in the interpreter.
            Out::NetInterpolate { older, newer, t } if vt(ee, 0x3ec) == NET_INTERPOLATE_FN && ported(NET_INTERPOLATE_FN) => {
                served("NetInterpolate");
                net_interpolate_in_ram(self.ee, older, newer, F(f32::from_bits(t)), dt);
                return 0;
            }
            // F45a's: the wheel clear, suspension and tire forces, quad::Steps' on Nat05's ground.
            Out::WheelClear(_) | Out::Suspension(_) | Out::TireForces(_) if unpack_map().is_some() && [WHEEL_CLEAR, SUSPENSION, TIRE_FORCES].iter().all(|&f| ported(f)) => {
                served(match c {
                    Out::WheelClear(_) => "WheelClear",
                    Out::Suspension(_) => "Suspension",
                    _ => "TireForces",
                });
                unpack_wheel_in_ram(self.ee, c, dt, unpack_map().unwrap());
                return 0;
            }
            // F45a's: the net post, [`bike::net_post`] with its calls [`PostsInRam`]'s.
            Out::NetPost if [NET_POST, NODE_BOUNDS, vt(ee, 0xe4), vt(ee, 0x124)].iter().all(|&f| ported(f)) => {
                served("NetPost");
                net_post_in_ram(self.ee, dt);
                return 0;
            }
            _ => {}
        }
        let packet_at = |ee: &mut Ee, p: &update::Packet| {
            let at = ee.scratch(PACKET_K);
            for (k, &b) in p.data().iter().enumerate() {
                ee.write(at + k as u32, 1, u128::from(b));
            }
            at
        };
        let (buf, head) = match c {
            Out::SenderSend { packet, .. } => (packet_at(self.ee, &packet), 0),
            Out::NetQueue { head, packet, .. } => {
                let buf = packet_at(self.ee, &packet);
                let at = self.ee.scratch(HEAD_K);
                self.ee.write(at, 2, u128::from(u16::from_le_bytes(head)));
                self.ee.set_u32(at + 4, buf);
                (buf, at)
            }
            _ => (0, 0),
        };
        let (bike, x) = (u64::from(BIKE), self.x);
        let ee = &*self.ee;
        let obj_fn = |o: u32, at: u32, slot: u32| ee.u32_at(ee.u32_at(o + at) + slot);
        let u = u64::from;
        let (f, ints, floats) = match c {
            Out::Search(r) => (SEARCH_ALL, vec![u(r), 0], vec![x]),
            Out::KindClear => (vt(ee, 0x39c), vec![bike], vec![x]),
            Out::Show => unreachable!("served by the port"),
            Out::ObjectOn(o) => (obj_fn(o, 0x28, 0x20), vec![u(o + 0x24)], vec![x]),
            Out::Insert { obj, space, at } => (SPACE_INSERT, vec![u(obj), u(space), u(at), 1], vec![x]),
            Out::Gear | Out::Trick | Out::NetCheck | Out::NetUnpack | Out::TrickHold(_) => unreachable!("served by the port"),
            Out::TrickEnd(id) => (vt(ee, 0x4ac), vec![bike, id as i32 as i64 as u64], vec![x]),
            Out::TrickRemote => (vt(ee, 0x4e8), vec![bike], vec![x]),
            Out::TrickWanted => (vt(ee, 0x4b4), vec![bike], vec![x]),
            Out::TrickStart(id) => (vt(ee, 0x4b8), vec![bike, u(id), 0], vec![x]),
            Out::Mark { run, player, on, id } => (MARK_FN, vec![u(run), u(player), u(on), u(id), 0], vec![x]),
            Out::NetInterpolate { older, newer, t } => (vt(ee, 0x3ec), vec![bike, u(older), u(newer)], vec![x, f32::from_bits(t)]),
            Out::Orient { node, at, pos } => (NODE_PLACE, vec![u(node), 0, u(at), u(pos)], vec![x]),
            Out::WheelClear(w) => (WHEEL_CLEAR, vec![u(w)], vec![x]),
            Out::Contacts => (CONTACTS, vec![bike], vec![x]),
            Out::Suspension(w) => (SUSPENSION, vec![u(w)], vec![x]),
            Out::TireForces(w) => (TIRE_FORCES, vec![u(w)], vec![x]),
            Out::NetPost => (NET_POST, vec![bike], vec![x]),
            Out::AnimStop(_) | Out::AnimPlay { .. } | Out::AnimBlend { .. } => unreachable!("served by the port"),
            Out::NodeSet { .. } | Out::Spin { .. } | Out::Aim { .. } | Out::Pose { .. } | Out::Place { .. } | Out::Frame { .. } | Out::Position { .. } | Out::Axes { .. } => {
                unreachable!("served by the port")
            }
            Out::Moved(_) => unreachable!("served by the port"),
            Out::TrailOn(t) => (obj_fn(t, 4, 0x18), vec![u(t)], vec![x]),
            Out::TrailOff(t) => (obj_fn(t, 4, 0x20), vec![u(t)], vec![x]),
            Out::Send { net, arg } => (obj_fn(net, 0x38, 0x20), vec![u(net), u(arg)], vec![x]),
            Out::Part(p) => (obj_fn(p, 0xc, 0xbc), vec![u(p)], vec![x]),
            Out::Shown(n) => (obj_fn(n, 4, 0x28), vec![u(n)], vec![x]),
            Out::Hidden(n) => (obj_fn(n, 4, 0x2c), vec![u(n)], vec![x]),
            Out::NetStunt(a) => (vt(ee, 0x544), vec![bike, u(a)], vec![x]),
            Out::NetFull(a) => (vt(ee, 0x400), vec![bike, u(a)], vec![x]),
            Out::NetDelta(a) => (vt(ee, 0x404), vec![bike, u(a)], vec![x]),
            Out::Walk(g) => (TAIL, vec![u(g)], vec![x]),
            Out::SenderSend { sender, arg, packet } => (obj_fn(sender, 0x38, 0x28), vec![u(sender), u(arg), u(buf), u(packet.len), 0, 0], vec![x]),
            Out::NetQueue { conn, flags, .. } => (NET_QUEUE, vec![u(conn), u(head), u(flags)], vec![x]),
            Out::NetFlush { socket, len, reliable } => {
                let r = u64::from(reliable);
                (NET_FLUSH, vec![u(socket), u64::MAX, u(update::FLUSH_BUFFER), u(len), r, r * 1000], vec![x])
            }
            Out::Hash { f, key } => (f, vec![u(key)], vec![x]),
            Out::SameKey { f, a, b } => (f, vec![u(a), u(b)], vec![x]),
            Out::IrqOff => (IRQ_OFF, vec![], vec![x]),
            Out::IrqOn => (IRQ_ON, vec![], vec![x]),
            Out::Counter => unreachable!("the stand-in word"),
            Out::NodePlace { .. } | Out::Local { .. } | Out::Rotate { .. } => unreachable!("asked through vector()"),
            Out::RunOpen(_) | Out::RunScore { .. } | Out::RunClose { .. } | Out::RunTally { .. } | Out::RunEnd { .. } => {
                unreachable!("served by the port")
            }
        };
        let r0 = self.ee.ran;
        let w = self.call(f, &ints, &floats);
        let kind = format!("{c:?}");
        book(format!("the tail: the update's {}", &kind[..kind.find([' ', '(']).unwrap_or(kind.len())]), self.ee.ran - r0);
        w
    }
    fn vector(&mut self, c: update::Out) -> update::Quad {
        use update::Out;
        label(&format!("update {c:?}"));
        // F43: the port's.
        let n = &mut NodeRam(self.ee);
        match c {
            Out::NodePlace { node, at } => node::place(n, node, at),
            Out::Local { node, at } => {
                let at = [0, 4, 8, 12].map(|k| n.0.u32_at(at + k));
                node::local(n, node, at)
            }
            Out::Rotate { node, v } => node::rotate(n, node, v),
            c => unreachable!("{c:?} writes no vector"),
        }
    }
    fn noise(&mut self, t: F) -> F {
        noise_at(self.ee, &mut None, t)
    }
}

/// The RAM the scene-node ports ([`node`]) read and write: the interpreter's.
struct NodeRam<'a>(&'a mut Ee);

impl node::Ram for NodeRam<'_> {
    fn word(&mut self, at: u32) -> u32 {
        self.0.u32_at(at)
    }
    fn set(&mut self, at: u32, w: u32) {
        self.0.set_u32(at, w);
    }
}

impl anim::Anim for NodeRam<'_> {
    fn alloc(&mut self, bytes: u32, buffer: anim::Saved) -> u32 {
        let line = match buffer {
            anim::Saved::Rotations => 0x52a,
            anim::Saved::Positions => 0x52d,
        };
        self.0.set(28, GP);
        census(self.0, |ee| ee.call(HEAP_ALLOC, &[u64::from(bytes), u64::from(HEAP_FILE), line], &[], BUDGET));
        self.0.r(2) as u32
    }
}

impl NodeRam<'_> {
    /// An interpreter call the attachment move makes (the heap's, a pool's constructor), its census
    /// under `heap Moved`.
    fn heap(&mut self, f: u32, ints: &[u32]) -> u32 {
        let ints: Vec<u64> = ints.iter().map(|&a| u64::from(a)).collect();
        label("heap Moved");
        self.0.set(28, GP);
        census(self.0, |ee| ee.call(f, &ints, &[], BUDGET));
        label("update Moved");
        self.0.r(2) as u32
    }
}

impl attach::Space for NodeRam<'_> {
    fn class(&mut self, sub: u32) -> attach::Class {
        match self.0.u32_at(sub + 4) {
            LIST_VT => attach::Class::List,
            TREE_VT => attach::Class::Tree,
            _ => attach::Class::Other,
        }
    }
    fn alloc(&mut self, bytes: u32, site: attach::Site) -> u32 {
        match site {
            attach::Site::Block => self.heap(HEAP_ALLOC, &[bytes, LIST_FILE, 94]),
            attach::Site::Chunk => self.heap(HEAP_ALLOC, &[bytes, POOL_FILE, 0xad]),
            attach::Site::ChunkIn(h) => self.heap(HEAP_ALLOC_IN, &[h, bytes, POOL_FILE, 0xaa]),
            attach::Site::Node => self.heap(HEAP_NEW, &[bytes, NODE_FILE, 0x5b]),
            s => unreachable!("{s:?} reallocates"),
        }
    }
    fn realloc(&mut self, at: u32, bytes: u32, site: attach::Site) -> u32 {
        match site {
            attach::Site::Blocks => self.heap(HEAP_REALLOC, &[at, bytes, LIST_FILE, 91]),
            attach::Site::Grow => self.heap(HEAP_REALLOC, &[at, bytes, TREE_FILE, 0x6d8]),
            s => unreachable!("{s:?} allocates"),
        }
    }
    fn free(&mut self, at: u32) {
        self.heap(HEAP_FREE, &[at]);
    }
    fn construct(&mut self, ctor: u32, link: u32) {
        self.heap(ctor, &[link]);
    }
}

/// Every channel of the clips one of the two kinds the port samples.
fn kinds(ee: &Ee, clips: &[u32]) {
    for &clip in clips {
        let data = ee.u32_at(clip + 4);
        for (list, count, slot, f) in [(0x1c, 0x28, 0x10, ANIM_TURNS), (0x18, 0x24, 0x14, ANIM_MOVES)] {
            for k in 0..ee.u32_at(data + count) {
                let ch = ee.u32_at(ee.u32_at(data + list) + 4 * k);
                assert_eq!(ee.u32_at(ee.u32_at(ch + 0x10) + slot), f, "clip {clip:#x}'s channel {ch:#x}");
            }
        }
    }
}

/// The arc cosine table the slerps read, built ([`AcosTable::build`] is the image's).
fn acos() -> &'static AcosTable {
    static A: std::sync::OnceLock<AcosTable> = std::sync::OnceLock::new();
    A.get_or_init(AcosTable::build)
}

/// The sine table the spin reads, built ([`CurveTable::build`] is the image's, S104).
fn curve() -> &'static game_sim::camera::CurveTable {
    static C: std::sync::OnceLock<game_sim::camera::CurveTable> = std::sync::OnceLock::new();
    C.get_or_init(game_sim::camera::CurveTable::build)
}

/// A port that answers nothing, as a call's `v0` of 0.
fn void(_: ()) -> u32 {
    0
}

/// One of the vehicle update's F39 callees as the port's, on the bike in RAM.
fn port_update(ee: &mut Ee, c: tail::Out, dt: F) {
    let mut m = UpdateInRam { ee, x: dt.0 };
    match c {
        tail::Out::Fade => {
            m.own(0x3a4);
            update::fade(&mut m, dt);
        }
        tail::Out::Lean => {
            m.own(0x4f0);
            update::lean(&mut m, dt);
        }
        tail::Out::Prepare => {
            m.own(0x3e4);
            update::prepare(&mut m, dt);
        }
        tail::Out::Ride => {
            m.own(0x35c);
            update::ride(&mut m, dt);
        }
        tail::Out::Sync => {
            m.own(0x378);
            update::sync(&mut m);
        }
        tail::Out::Snapshot => {
            m.own(0x37c);
            update::snapshot(&mut m);
        }
        tail::Out::Network => {
            update::network(&mut m, BIKE + update::NET, dt);
        }
        tail::Out::Parts => update::parts(&mut m),
        c => unreachable!("{c:?}"),
    }
}

/// The F39 oracle's fake world, at `U_AREA` (past the wreck's): the bike's vtable, the functions the
/// hooks stand for, the parts', trail's and sender's vtables, and the objects the bike's words point
/// at.
const U_AREA: u32 = 0x01c0_6000;
const U_VTABLE: u32 = U_AREA;
const U_FNS: u32 = U_AREA + 0x600;
const U_PART_VT: u32 = U_AREA + 0x700;
const U_TRAIL_VT: u32 = U_AREA + 0x7c0;
const U_NET_VT: u32 = U_AREA + 0x800;
const U_STATE: u32 = U_AREA + 0x900;
const U_RECORD: u32 = U_AREA + 0x940;
const U_PART2: u32 = U_AREA + 0xa80;
const U_CTRL: u32 = U_AREA + 0xb00;
const U_MODEL: u32 = U_AREA + 0xb40;
const U_HUNG: u32 = U_AREA + 0xd00;
const U_KINDS: u32 = U_AREA + 0xd20;
const U_HUNGS: u32 = U_AREA + 0xd40;
const U_TABLE: u32 = U_AREA + 0x1400;
const U_ANIMS: u32 = U_AREA + 0x1700;
const U_CLIPS: u32 = U_AREA + 0x1b00;
const U_CLIPDATA: u32 = U_AREA + 0x1b80;
const U_TRAIL: u32 = U_AREA + 0x1e00;
const U_RUN: u32 = U_AREA + 0x1e40;
const U_TIMER_LIST: u32 = U_AREA + 0x2200;
const U_TIMERS: u32 = U_AREA + 0x2240;
const U_LEN: u32 = 0x2300;

/// What the hooked calls answer, and the `dt` they are to be handed.
#[derive(Clone, Copy, Debug, Default)]
struct UpSays {
    gear: u32,
    trick: u32,
    net: u32,
    dt: f32,
    /// F42's: the remote stunt clocks' and the trick wanted's.
    remote: u32,
    wanted: u32,
}

/// A float's bits off a hash: sign, a power of two from 1/32 to 64, any mantissa.
fn fake_float(h: u64) -> u32 {
    let h = h as u32;
    (h & 0x8000_0000) | ((0x7a + (h >> 23) % 12) << 23) | (h & 0x7f_ffff)
}

/// The vector a hooked node call writes: a hash of what it is (`tag`), the node and the vector in.
fn fake_quad(tag: u64, node: u32, v: update::Quad) -> update::Quad {
    let s = v.iter().fold(mix(tag << 32 ^ u64::from(node)), |s, &w| mix(s ^ u64::from(w)));
    [0u64, 1, 2, 3].map(|k| fake_float(mix(s.wrapping_add(k))))
}

fn put_fake(ee: &mut Ee, at: u32, q: update::Quad) {
    for (k, w) in q.into_iter().enumerate() {
        ee.set_u32(at + 4 * k as u32, w);
    }
}

/// The node writes the hooks make: a frame (64 bytes), a position, two axes.
fn fake_frame(ee: &mut Ee, node: u32, at: u32) {
    for k in 0..4 {
        put_fake(ee, at + 16 * k, fake_quad(4, node, [k, 0, 0, 0]));
    }
}

fn fake_position(ee: &mut Ee, node: u32, at: u32) {
    put_fake(ee, at, fake_quad(5, node, [0; 4]));
}

fn fake_axes(ee: &mut Ee, node: u32, z: u32, y: u32) {
    put_fake(ee, z, fake_quad(6, node, [0; 4]));
    put_fake(ee, y, fake_quad(7, node, [0; 4]));
}

/// The port's side of the F39 oracle: words in RAM, the calls logged, the answers [`UpSays`]', the
/// node writes the hooks' fakes.
struct UpdateLogged<'a> {
    ee: &'a mut Ee,
    log: Vec<update::Out>,
    says: UpSays,
}

impl Updates for UpdateLogged<'_> {
    fn bike(&self) -> u32 {
        BIKE
    }
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn set(&mut self, at: u32, w: u32) {
        self.ee.set_u32(at, w);
    }
    fn out(&mut self, c: update::Out) -> u32 {
        use update::Out;
        self.log.push(c);
        match c {
            Out::Gear => self.says.gear,
            Out::Trick => self.says.trick,
            Out::NetCheck => self.says.net,
            Out::TrickRemote => self.says.remote,
            Out::TrickWanted => self.says.wanted,
            Out::Frame { node, at } => {
                fake_frame(self.ee, node, at);
                0
            }
            Out::Position { node, at } => {
                fake_position(self.ee, node, at);
                0
            }
            Out::Axes { node, z, y } => {
                fake_axes(self.ee, node, z, y);
                0
            }
            _ => 0,
        }
    }
    fn vector(&mut self, c: update::Out) -> update::Quad {
        use update::Out;
        self.log.push(c);
        match c {
            Out::NodePlace { node, at } => fake_quad(1, node, at),
            Out::Local { node, at } => fake_quad(2, node, [0, 4, 8, 12].map(|k| self.ee.u32_at(at + k))),
            Out::Rotate { node, v } => fake_quad(3, node, v),
            c => unreachable!("{c:?} writes no vector"),
        }
    }
    fn noise(&mut self, _: F) -> F {
        unreachable!("the trick hold is logged, not run, on the F39 and F42 rigs")
    }
}

/// The F39 oracle rig: the image with every call the callees make hooked to log (answer as
/// [`UpSays`] says, and write the fakes), the bike's vtable the fake one at [`U_AREA`].
struct UpdateRig {
    ee: Ee,
    log: Rc<RefCell<Vec<update::Out>>>,
    says: Rc<RefCell<UpSays>>,
    regions: Vec<(u32, u32)>,
    body: u32,
    wheels: [u32; 2],
}

fn update_rig() -> Option<UpdateRig> {
    use update::Out;
    let mut ee = hooked(image()?);
    for (slot, f) in UPDATE_OWN {
        assert_eq!(vt(&ee, slot), f, "the bike's vt +{slot:#x}");
    }
    assert_eq!(ee.u32_at(PREPARE_FN + 0x38), 0x0c00_0000 | PREPARE_THUNK >> 2, "the prepare's first call");
    assert_eq!(ee.u32_at(PREPARE_THUNK), 0x0800_0000 | PREPARE_HELD >> 2, "the prepare's thunk's target");
    let body = ee.u32_at(BIKE + update::BODY);
    let wheels = [ee.u32_at(BIKE + 0x17b0), ee.u32_at(BIKE + 0x17b4)];
    for o in (0..U_LEN).step_by(4) {
        ee.set_u32(U_AREA + o, 0);
    }
    let log: Rc<RefCell<Vec<Out>>> = Rc::default();
    let says: Rc<RefCell<UpSays>> = Rc::default();
    let poses: Rc<RefCell<Vec<u32>>> = Rc::default();
    let stack = ee.stack;
    let mut next = U_FNS;
    let mut hook = |ee: &mut Ee, vtable: Option<(u32, u32)>, at: u32, h: Box<dyn Fn(&mut Ee) -> Out>| {
        let at = match vtable {
            Some((vt, slot)) => {
                let f = next;
                next += 4;
                ee.set_u32(vt + slot, f);
                f
            }
            None => at,
        };
        let l = log.clone();
        ee.hooks.insert(at, Box::new(move |ee: &mut Ee| {
            let c = h(ee);
            l.borrow_mut().push(c);
        }));
    };
    let quad_at = |ee: &Ee, at: u64| -> update::Quad { [0, 4, 8, 12].map(|k| ee.u32_at(at as u32 + k)) };
    let dt_is = |ee: &Ee, s: &Rc<RefCell<UpSays>>, what: &str| {
        assert_eq!(ee.f(12).to_bits(), s.borrow().dt.to_bits(), "{what} is handed dt");
    };
    // The bike's own: each logs, three answer.
    for (slot, c) in [(0x39c, Out::KindClear), (0x358, Out::Gear), (0x4e4, Out::Trick), (0x4ac, Out::TrickEnd(u32::MAX)), (0x3e8, Out::NetCheck), (0x3f0, Out::NetUnpack)] {
        let s = says.clone();
        hook(&mut ee, Some((U_VTABLE, slot)), 0, Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, BIKE, "a bike virtual handed something other than the bike");
            let says = *s.borrow();
            match c {
                Out::Gear => ee.set(2, u64::from(says.gear)),
                Out::Trick => {
                    dt_is(ee, &s, "the trick step");
                    ee.set(2, u64::from(says.trick));
                }
                Out::TrickEnd(_) => assert_eq!(ee.r(5) as u32, u32::MAX, "the trick end's id"),
                Out::NetCheck => ee.set(2, u64::from(says.net)),
                Out::NetUnpack => dt_is(ee, &s, "the net unpack"),
                _ => {}
            }
            c
        }));
    }
    // The parts', the trail's, the sender's.
    let s = says.clone();
    hook(&mut ee, Some((U_PART_VT, 0xbc)), 0, Box::new(move |ee| {
        dt_is(ee, &s, "a part's step");
        Out::Part(ee.r(4) as u32)
    }));
    hook(&mut ee, Some((U_TRAIL_VT, 0x18)), 0, Box::new(|ee| Out::TrailOn(ee.r(4) as u32)));
    hook(&mut ee, Some((U_TRAIL_VT, 0x20)), 0, Box::new(|ee| Out::TrailOff(ee.r(4) as u32)));
    let s = says.clone();
    hook(&mut ee, Some((U_NET_VT, 0x20)), 0, Box::new(move |ee| {
        dt_is(ee, &s, "the send");
        Out::Send { net: ee.r(4) as u32, arg: ee.r(5) as u32 }
    }));
    // The functions.
    hook(&mut ee, None, SEARCH_ALL, Box::new(|ee| {
        assert_eq!(ee.r(5), 0, "the search's second argument");
        Out::Search(ee.r(4) as u32)
    }));
    hook(&mut ee, None, ANIM_STOP, Box::new(|ee| {
        assert_eq!([5, 6, 7, 8].map(|r| ee.r(r)), [0; 4], "the stop's arguments");
        Out::AnimStop(ee.r(4) as u32)
    }));
    hook(&mut ee, None, ANIM_PLAY, Box::new(|ee| {
        assert_eq!([6, 7, 8, 9, 10, 11].map(|r| ee.r(r)), [0; 6], "the play's arguments");
        assert_eq!(ee.f(14).to_bits(), 0, "the play's third float");
        Out::AnimPlay { ctrl: ee.r(4) as u32, clip: ee.r(5) as u32, weight: ee.f(12).to_bits(), fade: ee.f(13).to_bits() }
    }));
    hook(&mut ee, None, ANIM_BLEND, Box::new(|ee| {
        assert_eq!([7, 8, 9, 10, 11].map(|r| ee.r(r)), [0; 5], "the blend's arguments");
        Out::AnimBlend {
            ctrl: ee.r(4) as u32,
            clips: [ee.r(5) as u32, ee.r(6) as u32],
            weight: ee.f(12).to_bits(),
            mix: ee.f(13).to_bits(),
            fade: ee.f(14).to_bits(),
        }
    }));
    hook(&mut ee, None, ANIM_BLEND4, Box::new(|_| panic!("the four-clip blend is called")));
    hook(&mut ee, None, NODE_TIP, Box::new(move |ee| {
        let (node, at) = (ee.r(5) as u32, quad_at(ee, ee.r(6)));
        let out = ee.r(4) as u32;
        put_fake(ee, out, fake_quad(1, node, at));
        Out::NodePlace { node, at }
    }));
    hook(&mut ee, None, NODE_SET, Box::new(move |ee| {
        assert_eq!(ee.r(5), 0, "the set's second argument");
        Out::NodeSet { node: ee.r(4) as u32, at: quad_at(ee, ee.r(6)) }
    }));
    hook(&mut ee, None, NODE_SPIN, Box::new(|ee| Out::Spin { node: ee.r(4) as u32, axis: ee.r(5) as u32, angle: ee.f(12).to_bits() }));
    hook(&mut ee, None, NODE_AIM, Box::new(move |ee| {
        assert_eq!([7, 8, 9].map(|r| ee.r(r)), [1; 3], "the aim's flags");
        Out::Aim { node: ee.r(4) as u32, dir: quad_at(ee, ee.r(5)), up: ee.r(6) as u32 }
    }));
    let p = poses.clone();
    hook(&mut ee, None, BONE_POSE, Box::new(move |ee| {
        assert_eq!(ee.r(5), 0, "the pose's second argument");
        let (node, at) = (ee.r(4) as u32, ee.r(6) as u32);
        if (stack - 0x1_0000..stack).contains(&at) {
            let mut p = p.borrow_mut();
            p.push(at);
            Out::Pose { node, buf: p.len() as u32 - 1 }
        } else {
            fake_frame(ee, node, at);
            Out::Frame { node, at }
        }
    }));
    let p = poses.clone();
    hook(&mut ee, None, BONE_PLACE, Box::new(move |ee| {
        let buf = p.borrow().iter().position(|&b| b == ee.r(5) as u32).map_or(99, |i| i as u32);
        Out::Place { part: ee.r(4) as u32, buf }
    }));
    hook(&mut ee, None, NODE_POSITION, Box::new(|ee| {
        assert_eq!(ee.r(5), 0, "the position's second argument");
        let (node, at) = (ee.r(4) as u32, ee.r(6) as u32);
        fake_position(ee, node, at);
        Out::Position { node, at }
    }));
    hook(&mut ee, None, ATTACH_MOVED, Box::new(|ee| Out::Moved(ee.r(4) as u32)));
    hook(&mut ee, None, NODE_AXES, Box::new(|ee| {
        assert_eq!(ee.r(5), 0, "the axes' second argument");
        let (node, z, y) = (ee.r(4) as u32, ee.r(6) as u32, ee.r(7) as u32);
        fake_axes(ee, node, z, y);
        Out::Axes { node, z, y }
    }));
    hook(&mut ee, None, NODE_LOCAL, Box::new(move |ee| {
        let (out, node, at) = (ee.r(4) as u32, ee.r(5) as u32, ee.r(6) as u32);
        let q = fake_quad(2, node, quad_at(ee, u64::from(at)));
        put_fake(ee, out, q);
        Out::Local { node, at }
    }));
    hook(&mut ee, None, NODE_ROTATE, Box::new(move |ee| {
        let (out, node, v) = (ee.r(4) as u32, ee.r(5) as u32, quad_at(ee, ee.r(6)));
        put_fake(ee, out, fake_quad(3, node, v));
        Out::Rotate { node, v }
    }));
    let s = says.clone();
    hook(&mut ee, None, TAIL, Box::new(move |ee| {
        dt_is(ee, &s, "the walk");
        Out::Walk(ee.r(4) as u32)
    }));
    // The fakes in place, and the poses' list cleared with every log.
    ee.set_u32(BIKE + 4, U_VTABLE);
    ee.set_u32(U_RECORD + 0xc, U_PART_VT);
    ee.set_u32(U_PART2 + 0xc, U_PART_VT);
    ee.set_u32(U_TRAIL + 4, U_TRAIL_VT);
    let p = poses;
    let l = log.clone();
    ee.hooks.insert(U_AREA + 0x6fc, Box::new(move |_: &mut Ee| {
        p.borrow_mut().clear();
        l.borrow_mut().clear();
    }));
    let regions = vec![(BIKE, 0x1820), (body, 0x230), (wheels[0], 0x920), (wheels[1], 0x920), (U_AREA, U_LEN), (update::POSE_SPEED, 4)];
    Some(UpdateRig { ee, log, says, regions, body, wheels })
}

impl UpdateRig {
    /// Runs the original's `f` and then the port's `port` from the same words and answers; asserts
    /// the words, the calls and the answer (`v0`) the same.
    fn check(&mut self, k: usize, f: u32, ints: &[u64], x: f32, port: impl FnOnce(&mut UpdateLogged) -> u32) -> Vec<update::Out> {
        let before = snap_regions(&self.ee, &self.regions);
        let says = *self.says.borrow();
        self.ee.call(U_AREA + 0x6fc, &[], &[], 10);
        self.ee.set(28, GP);
        self.ee.call(f, ints, &[x], BUDGET);
        let want_v0 = self.ee.r(2) as u32;
        let want = std::mem::take(&mut *self.log.borrow_mut());
        let want_words = snap_regions(&self.ee, &self.regions);
        restore_regions(&mut self.ee, &self.regions, &before);
        let mut m = UpdateLogged { ee: &mut self.ee, log: Vec::new(), says };
        let v0 = port(&mut m);
        let got = m.log;
        assert_eq!(format!("{got:?}"), format!("{want:?}"), "case {k} ({f:#x}): the calls");
        let got_words = snap_regions(&self.ee, &self.regions);
        if got_words != want_words {
            let names: Vec<(u32, u32)> = self.regions.iter().flat_map(|&(at, len)| (0..len).step_by(4).map(move |o| (at, o))).collect();
            let d: Vec<String> = (0..got_words.len())
                .filter(|&i| got_words[i] != want_words[i])
                .map(|i| format!("{:#x}+{:#x}: port {:#010x}, original {:#010x}", names[i].0, names[i].1, got_words[i], want_words[i]))
                .collect();
            panic!("case {k} ({f:#x}): {} words differ\n  {}", d.len(), d.iter().take(20).cloned().collect::<Vec<_>>().join("\n  "));
        }
        if let Some(v0) = Some(v0).filter(|_| [RIDE_FN, NETWORK, GEAR_FN, TRICK_FN, NET_CHECK_FN].contains(&f)) {
            assert_eq!(v0, want_v0, "case {k} ({f:#x}): the answer");
        }
        want
    }
}

#[allow(clippy::float_arithmetic)]
fn vu_add(a: f32, b: f32) -> f32 {
    a + b
}

/// **The vehicle update's callees are the original's**: 10 000 bikes, each state 4's with every word
/// the callees read drawn at random — the fade's clock and start (running out this frame among them),
/// the record and its held bit, the hung parts (−1 to 4, kinds 0-2); the auto gears, the gear's
/// entries (no wait, down or up), speeds about 5 and 10 ft/s, the clock about the wait; the stunt ids
/// (in the table, the riding pose, none, −1), the clip's length and time, loads, the pose speed, the
/// hold about `dt`, down, the trick step's answer; the vehicle state's kind (0-5) and the net check's
/// answer, the fade's rate, the new stunt, the clips (zero length among them), the fore-aft and cross
/// blends about their halves, the suspension's and head's words, the seat, look and lean quadwords;
/// the flags and the camera's, replay's, trail's and body's masks, take-off, trail on and off, the
/// wreck, the kept stunts, the run or none; the sender's flags, clock, timers (−1 to 4, due on the
/// clock among them); the parts or none — and run through SLUS-21104's fade, ride, lean, prepare,
/// sync, snapshot, network and parts in that order, each against the port's from the same words.
/// Every word of the bike, its body, its wheels and the fake world comes out the same, the calls are
/// the same, in order, with the same arguments, and the ride's and network's answers are. The deeper
/// callees are hooked to log; the node calls write hashed vectors and frames, the same in the port's
/// mirror.
#[test]
fn the_vehicle_updates_callees_are_the_originals() {
    use update::Out;
    let Some(mut rig) = update_rig() else { return };
    let image_words = snap_regions(&rig.ee, &rig.regions);
    let mut g = Gen(0x0000_f39f);
    let mut t = BTreeMap::new();
    let bike = u64::from(BIKE);
    for k in 0..10_000 {
        let ee = &mut rig.ee;
        restore_regions(ee, &rig.regions, &image_words);
        let b = |g: &mut Gen, n: u32| u32::from(g.one_in(n));
        let put = |ee: &mut Ee, o: u32, v: f32| ee.set_f32(BIKE + o, v);
        let word = |ee: &mut Ee, o: u32, w: u32| ee.set_u32(BIKE + o, w);
        let dt = if g.one_in(2) { f32::from_bits(DT) } else { g.f(0.002, 0.05) };
        // The fade.
        let fading = !g.one_in(4);
        let fade = if !fading {
            [0.0, -0.0, -0.5][(g.bits() % 3) as usize]
        } else {
            match g.bits() % 4 {
                0 => dt,
                1 => g.f(0.0, 2.0 * dt),
                _ => g.f(0.0, 3.0),
            }
        };
        put(ee, update::FADE, fade);
        let start = if g.one_in(5) { 0.0 } else { g.f(0.5, 3.0) };
        put(ee, update::FADE_START, start);
        word(ee, update::RECORD, if !fading && g.one_in(3) { 0 } else { U_RECORD });
        wreck_words(ee, &mut g, U_RECORD + 0x124, 1);
        wreck_words(ee, &mut g, U_RECORD + 0xf8, 1);
        word(ee, 0x1428, U_CTRL);
        ee.set_u32(U_CTRL + 0x24, U_MODEL);
        word(ee, 0x1470, if g.one_in(15) { u32::MAX } else { g.bits() % 5 });
        word(ee, 0x1474, U_HUNG);
        word(ee, 0x1478, U_KINDS);
        for i in 0..4 {
            ee.set_u32(U_HUNG + 4 * i, U_HUNGS + 0x180 * i);
            ee.set_u32(U_KINDS + 4 * i, g.bits() % 3);
        }
        // The ride.
        word(ee, 0x10d4, 1 - b(&mut g, 4));
        let gear = g.bits() % 4;
        let mut waits = [0.0f32; 4];
        for (i, w) in waits.iter_mut().enumerate() {
            let e = BIKE + 0xec0 + 32 * i as u32;
            *w = if g.one_in(4) { [0.0, -0.0][(g.bits() % 2) as usize] } else { g.f(0.0, 1.0) };
            ee.set_f32(e, *w);
            let step = g.f(0.5, 2.0);
            ee.set_f32(e + 4, step);
            ee.set_u32(e + 8, b(&mut g, 2));
        }
        let speed = match g.bits() % 4 {
            0 => 5.0,
            1 => 10.0,
            2 => g.f(-5.0, 15.0),
            _ => g.f(0.0, 60.0),
        };
        put(ee, 0x32c, speed);
        let clock = if g.one_in(2) { vu_sub(waits[gear as usize], dt) } else { g.f(0.0, 1.0) };
        put(ee, 0x10d0, clock);
        let (now, top) = (g.f(0.0, 5.0), g.f(1.0, 5.0));
        put(ee, 0x10cc, now);
        put(ee, 0x10d8, top);
        // The lean: the stunt table, the anims, the clips.
        word(ee, 0x14f0, U_TABLE);
        for id in -1i32..46 {
            let e = (U_TABLE as i32 + 0x18 + 12 * id) as u32;
            ee.set_u32(e, U_ANIMS + 0x80 * (g.bits() % 8));
        }
        for i in 0..8 {
            let a = U_ANIMS + 0x80 * i;
            let len = g.f(0.0, 3.0);
            ee.set_f32(a + 0x40, len);
            ee.set_u32(a + 0x58, if g.one_in(2) { 0 } else { g.bits() });
            ee.set_u32(U_CLIPS + 0x10 * i + 4, U_CLIPDATA + 0x50 * i);
            let len = if g.one_in(8) { 0.0 } else { g.f(0.1, 5.0) };
            ee.set_f32(U_CLIPDATA + 0x50 * i + 0x44, len);
        }
        for o in (0x1530..0x15f8).step_by(4) {
            word(ee, o, U_CLIPS + 0x10 * (g.bits() % 8));
        }
        let pose = 46;
        let id = [4, pose, 0xff, u32::MAX, g.bits() % 46, g.bits() % 46][(g.bits() % 6) as usize];
        word(ee, update::STUNT, id);
        word(ee, 0x152c, if g.one_in(2) { id } else { [pose, 4][(g.bits() % 2) as usize] });
        let entry = update::stunt_entry(U_TABLE, id);
        let clip_len = if entry == 0 { 1.0 } else { ee.f32_at(ee.u32_at(entry) + 0x40) };
        let time = if g.one_in(4) { clip_len } else { g.f(0.0, 3.0) };
        put(ee, 0x14d8, time);
        for o in [0x704, 0x708] {
            let x = if g.one_in(2) { g.near(0.0, -1.0, 0.0) } else { g.f(0.0, 1.0) };
            put(ee, o, x);
        }
        word(ee, 0xbbc, g.bits() % 3);
        let pose_speed = g.f(2.0, 8.0);
        ee.set_f32(update::POSE_SPEED, pose_speed);
        if g.one_in(4) {
            put(ee, 0x32c, pose_speed);
        }
        word(ee, 0x324, 1 - b(&mut g, 2));
        let hold = match g.bits() % 4 {
            0 => dt,
            1 => -0.0,
            2 => g.f(0.0, 2.0 * dt),
            _ => g.f(0.0, 1.0),
        };
        put(ee, 0x14f4, hold);
        word(ee, 0xc80, [0, 0, 0, 7, 1, 3][(g.bits() % 6) as usize]);
        word(ee, 0x31c, b(&mut g, 3));
        // The prepare.
        word(ee, update::STATE, U_STATE);
        ee.set_u32(U_STATE + 0x28, g.bits() % 6);
        let rate = g.f(-0.2, 1.2);
        put(ee, 0x14bc, rate);
        let rate = g.f(0.0, 10.0);
        put(ee, 0x14c0, rate);
        let next = if g.one_in(2) { id } else { [4, pose, 0xff, u32::MAX, g.bits() % 46, g.bits() % 60][(g.bits() % 6) as usize] };
        word(ee, update::NEXT_STUNT, next);
        word(ee, 0x1650, b(&mut g, 4));
        for (o, at, lo, hi) in [(0x12ec, 0.0, -1.2, 1.2), (0x14b0, 0.0, 0.0, 2.0), (0x330, 0.0, -3.0, 3.0), (0x14b4, 0.5, -0.2, 1.2), (0x14b8, 0.5, -0.2, 1.2), (0x141c, 0.5, -0.2, 1.2), (0x12e8, 0.0, -1.2, 1.2), (0x1810, 0.0, -1.0, 1.0)] {
            let x = g.near(at, lo, hi);
            put(ee, o, x);
        }
        for o in [0x1800, 0x17e0, 0x17f0] {
            wreck_quad(ee, &mut g, BIKE + o, 5.0);
        }
        for w in rig.wheels {
            wreck_quad(ee, &mut g, w + 0x90, 5.0);
            wreck_quad(ee, &mut g, w + 0xc0, 1.0);
            for o in [0xf0, 0x1d0, 0x8ac] {
                let x = g.f(-1.0, 1.0);
                ee.set_f32(w + o, x);
            }
        }
        // The sync and the snapshot.
        word(ee, 0x444, g.bits());
        for o in [0x9c4, 0xa94, 0x734, 0xb54] {
            word(ee, o, if g.one_in(8) { 0 } else { 1 << (g.bits() % 32) });
        }
        word(ee, 0xa70, b(&mut g, 2));
        for o in [0x70, 0x80, 0x190] {
            wreck_quad(ee, &mut g, rig.body + o, 30.0);
        }
        for o in [0xbd0, 0x340, 0xda0, 0xb20, 0x1260] {
            wreck_quad(ee, &mut g, BIKE + o, 30.0);
        }
        for o in [0x1194, 0x11d0, 0xb34, 0x944] {
            word(ee, o, b(&mut g, 2));
        }
        for o in [0x368, 0x12c8, 0xd08, 0x318, 0x12cc, 0x12d4, 0x12c4, 0x14e4, 0xd0, 0x16d4] {
            wreck_words(ee, &mut g, BIKE + o, 1);
        }
        word(ee, 0x10dc, U_TRAIL);
        word(ee, 0x868, if g.one_in(3) { g.bits() % 8 } else { 0 });
        for o in [0x13b8, 0x13bc, 0x13c0] {
            word(ee, o, if g.one_in(4) { next } else { 41 + g.bits() % 20 });
        }
        word(ee, 0x684, if g.one_in(4) { 0 } else { U_RUN });
        wreck_words(ee, &mut g, U_RUN + 0x50, 1);
        wreck_words(ee, &mut g, U_RUN + 0x2c0, 1);
        // The sender and the parts.
        let net = BIKE + update::NET;
        wreck_words(ee, &mut g, net + 0x14, 1);
        let net_clock = g.f(0.0, 10.0);
        ee.set_f32(net + 0x18, net_clock);
        ee.set_u32(net + 0x1c, if g.one_in(15) { u32::MAX } else { g.bits() % 5 });
        ee.set_u32(net + 0x24, U_TIMER_LIST);
        wreck_words(ee, &mut g, net + 0x34, 1);
        ee.set_u32(net + 0x38, U_NET_VT);
        for i in 0..4 {
            let e = U_TIMERS + 0x10 * i;
            ee.set_u32(U_TIMER_LIST + 4 * i, e);
            ee.set_u32(e, g.bits() & 0xff);
            let period = g.f(0.0, 2.0);
            ee.set_f32(e + 4, period);
            let due = if g.one_in(3) { vu_add(net_clock, dt) } else { g.f(0.0, 12.0) };
            ee.set_f32(e + 8, due);
        }
        word(ee, 0xd4, if g.one_in(3) { 0 } else { U_PART2 });
        *rig.says.borrow_mut() = UpSays { gear, trick: if g.one_in(2) { 0 } else { 1 + g.bits() % 46 }, net: b(&mut g, 2), dt, ..Default::default() };

        // Fade, ride, lean, prepare, sync, snapshot, network, parts: each from the last one's words.
        let (x, f) = (dt, F(dt));
        let c = rig.check(k, FADE_FN, &[bike], x, |m| {
            update::fade(m, f);
            0
        });
        tally(&mut t, match () {
            _ if !fading => "fade off",
            _ if c.contains(&Out::KindClear) => "fade over",
            _ if start == 0.0 => "fade, no start",
            _ => "fade, alpha",
        });
        if fading && rig.ee.u8_at(U_RECORD + 0x126) & 0x20 != 0 {
            tally(&mut t, "fade held");
        }
        let c = rig.check(k, RIDE_FN, &[bike], x, |m| update::ride(m, f));
        let moved = rig.ee.u32_at(BIKE + 0x10d0) != clock.to_bits() || rig.ee.f32_at(BIKE + 0x10cc).to_bits() != now.to_bits();
        tally(&mut t, match () {
            _ if !c.contains(&Out::Gear) => "ride, manual",
            _ if moved && rig.ee.u32_at(BIKE + 0x10d0) == 0 => "ride, shifted or dropped",
            _ if moved => "ride, counting",
            _ => "ride, slow",
        });
        let c = rig.check(k, LEAN_FN, &[bike], x, |m| {
            update::lean(m, f);
            0
        });
        if rig.ee.u32_at(BIKE + 0x14cc) != 0 {
            tally(&mut t, "lean, the clip ran out");
        }
        if c.contains(&Out::TrickEnd(u32::MAX)) {
            tally(&mut t, "lean, the hold ran out");
        }
        tally(&mut t, match rig.ee.u32_at(BIKE + update::NEXT_STUNT) {
            46 => "lean, the riding pose",
            0xff => "lean, no stunt",
            _ => "lean, a trick",
        });
        let c = rig.check(k, PREPARE_FN, &[bike], x, |m| {
            update::prepare(m, f);
            0
        });
        for (has, name) in [
            (c.contains(&Out::NetUnpack), "prepare, net unpack"),
            (c.iter().any(|c| matches!(c, Out::AnimStop(_))), "prepare, stopped"),
            (c.iter().any(|c| matches!(c, Out::AnimBlend { .. })), "prepare, the pose's blend"),
            (c.iter().any(|c| matches!(c, Out::AnimPlay { clip, .. } if *clip != rig.ee.u32_at(BIKE + 0x15f0))), "prepare, a stunt's clip"),
            (c.iter().filter(|c| matches!(c, Out::Spin { .. })).count() == 3, "prepare, the bars turned"),
            (c.iter().any(|c| matches!(c, Out::Place { .. })), "prepare, a part placed"),
        ] {
            if has {
                tally(&mut t, name);
            }
        }
        let c = rig.check(k, SYNC_FN, &[bike], x, |m| {
            update::sync(m);
            0
        });
        for (has, name) in [
            (c.iter().any(|c| matches!(c, Out::Local { .. })), "sync, the camera's"),
            (c.iter().any(|c| matches!(c, Out::TrailOn(_))), "sync, trail on"),
            (c.iter().any(|c| matches!(c, Out::TrailOff(_))), "sync, trail off"),
            (c.iter().any(|c| matches!(c, Out::Rotate { .. })), "sync, the trail's spot"),
            (rig.ee.u32_at(BIKE + 0xb30) == 1, "sync, the wreck's flag"),
        ] {
            if has {
                tally(&mut t, name);
            }
        }
        let c = rig.check(k, SNAPSHOT_FN, &[bike], x, |m| {
            update::snapshot(m);
            0
        });
        if !c.is_empty() {
            tally(&mut t, if rig.ee.u32_at(BIKE + 0x868) == 7 { "snapshot, thrown" } else { "snapshot" });
            tally(&mut t, if rig.ee.u8_at(BIKE + 0x904) == 0xff { "snapshot, no stunt kept" } else { "snapshot, a stunt kept" });
        }
        let c = rig.check(k, NETWORK, &[u64::from(net)], x, |m| update::network(m, net, f));
        let fired = rig.ee.u32_at(net + 0x14) & 1 != 0;
        tally(&mut t, match () {
            _ if !c.is_empty() => "network, sent",
            _ if fired => "network, due but held",
            _ => "network, nothing due",
        });
        let c = rig.check(k, PARTS, &[bike], x, |m| {
            update::parts(m);
            0
        });
        tally(&mut t, if c.len() == 3 { "parts, both" } else { "parts, fewer" });
    }
    eprintln!("{t:#?}");
    for (path, n) in &t {
        assert!(*n >= 20, "path {path} taken only {n} times");
    }
    assert!(t.len() >= 28, "every path taken: {t:#?}");
}

/// **F41: of the rider's writes into the bike, the bike's frame reads one word.** The tail's rider
/// ([`OTHER_LEAVES`]' third) writes two windows of the bike's collision record, the bike's
/// `+0x870..+0x8a0` and `+0x920..+0x980`, which the port does not write. Both C15 replays run whole in
/// the interpreter with every byte the rider's step writes into those windows marked; a read of a
/// marked byte outside the rider's own step, by the rest of that frame or any later one, before
/// something else writes it again, is the bike's frame depending on the rider. Every read in the frame
/// counts: the original's code and the interpreter's hooks. The rider's own reads are counted apart.
///
/// The ride: none. The flip: only `+0x920` (the record's `+0x1f0`), and only from the wreck row
/// (125) until the put-back clock starts counting (row 140): it is the rider's "a ragdoll part is
/// touching" flag, which the wreck clock ([`wreck::clock`]'s `SETTLES`) waits on. S278 ports the flag
/// and its hand-over ([`rider`]); the parts' touched words it is made from are still the original
/// ragdoll world's (S278a; research/oracle.txt `# F41`, `# S278`). Nothing else writes any marked byte
/// again.
#[test]
fn the_riders_writes_into_the_bike_are_read_by_the_wreck_clock_alone() {
    for (name, n) in [("ride-2", 459), ("flip-1", 465)] {
        let Some(t) = riders_writes_read(name) else { return };
        eprintln!("{name}: {} frames, {} steps, {} bytes marked, {} read back by the rider, {} reads by the frame", t.frames, t.steps, t.marked, t.own_reads, t.read.len());
        assert_eq!(t.frames, n);
        assert_eq!(t.steps, n, "the rider steps once a frame");
        assert!(t.marked > 0, "{name}: the rider wrote the windows");
        assert_eq!(t.overwritten, 0, "{name}: nothing but the rider writes the windows");
        let words: std::collections::BTreeSet<u32> = t.read.iter().map(|&(_, a)| (a - BIKE) & !3).collect();
        let frames: std::collections::BTreeSet<usize> = t.read.iter().map(|&(r, _)| r).collect();
        // F44a: the stunt packet (no longer stubbed) sends the rider's +0x940, and nothing else of it.
        let sent: std::collections::BTreeSet<u32> = t.sent.iter().map(|&(_, a)| (a - BIKE) & !3).collect();
        eprintln!("{name}: {} bytes of the rider's sent by the stunt packet, on {} frames", t.sent.len(), t.sent.iter().map(|&(r, _)| r).collect::<std::collections::BTreeSet<_>>().len());
        assert_eq!(sent.into_iter().collect::<Vec<_>>(), [0x940], "{name}: the rider's words the stunt packet sends");
        if name == "ride-2" {
            assert!(t.read.is_empty(), "{name}: the rider's writes read by the bike's frame: {words:#x?}");
        } else {
            assert_eq!(words.into_iter().collect::<Vec<_>>(), [wreck::SETTLES], "{name}: the words of the rider's read by the bike's frame");
            assert_eq!(frames.into_iter().collect::<Vec<_>>(), (125..=140).collect::<Vec<_>>(), "{name}: the rows the wreck clock reads it");
        }
    }
}

/// What [`riders_writes_read`] saw over a replay: the frames, the rider's steps, the bytes of the
/// windows its steps wrote (summed over them), how many of those something else wrote again before
/// the next step, how many the rider itself read back, and every read of a marked byte by anything
/// else (frame, address).
#[derive(Debug, Default)]
struct RiderReads {
    frames: usize,
    steps: usize,
    marked: usize,
    overwritten: usize,
    own_reads: usize,
    read: Vec<(usize, u32)>,
    /// F44a: the reads of marked bytes by the stunt packet, which sends them (frame, address).
    sent: Vec<(usize, u32)>,
}

/// [`replay`] with the rider's step hooked to run as itself in between two read watches
/// ([`RiderReads`]).
fn riders_writes_read(name: &str) -> Option<RiderReads> {
    let (rider, step, windows) = OTHER_LEAVES[2];
    let windows: Vec<std::ops::Range<u32>> = windows.iter().map(|r| BIKE + r.start..BIKE + r.end).collect();
    let mut ee = hooked(image()?);
    let rows = rows(name)?;
    let t: Rc<RefCell<(RiderReads, std::collections::BTreeSet<u32>)>> = Rc::default();
    // The reads and writes since the last mark, checked against the marked bytes and then clearing
    // them; the frame's index for the report.
    let settle = |t: &mut (RiderReads, std::collections::BTreeSet<u32>), ee: &mut Ee, frame: usize| {
        let reads = ee.reads.replace(Some(Default::default())).unwrap_or_default();
        let (r, marked) = t;
        r.read.extend(reads.intersection(marked).map(|&a| (frame, a)));
        let gone: Vec<u32> = ee.written.intersection(marked).copied().collect();
        r.overwritten += gone.len();
        for a in gone {
            marked.remove(&a);
        }
        ee.written.clear();
    };
    let frame: Rc<RefCell<usize>> = Rc::default();
    let (h, at) = (t.clone(), frame.clone());
    ee.hooks.insert(
        step,
        Box::new(move |ee: &mut Ee| {
            assert_eq!(ee.r(4) as u32, rider, "the rider step's listener");
            let mut t = h.borrow_mut();
            settle(&mut t, ee, *at.borrow());
            let (args, x, ra, sp, stack) = ([ee.r(4), ee.r(5), ee.r(6), ee.r(7)], ee.f(12), ee.r(31), ee.r(29), ee.stack);
            ee.stack = (sp as u32 - 0x40) & !0xf;
            ee.call(step, &args, &[x], BUDGET);
            // The interpreter files the hook back under the `pc` it leaves, so leave it here.
            (ee.stack, ee.pc) = (stack, step);
            ee.set(29, sp);
            ee.set(31, ra);
            let reads = ee.reads.replace(Some(Default::default())).unwrap_or_default();
            let (r, marked) = &mut *t;
            r.steps += 1;
            r.own_reads += reads.intersection(marked).count();
            let wrote: Vec<u32> = ee.written.iter().copied().filter(|a| windows.iter().any(|w| w.contains(a))).collect();
            r.marked += wrote.len();
            marked.extend(wrote);
            ee.written.clear();
        }),
    );
    // F44a: the stunt packet, run as itself between two read watches the same, its reads apart.
    let (h, at) = (t.clone(), frame.clone());
    ee.hooks.insert(
        STUNT_FN,
        Box::new(move |ee: &mut Ee| {
            let mut t = h.borrow_mut();
            settle(&mut t, ee, *at.borrow());
            let (args, ra, sp, stack) = ([ee.r(4), ee.r(5)], ee.r(31), ee.r(29), ee.stack);
            ee.stack = (sp as u32 - 0x40) & !0xf;
            ee.call(STUNT_FN, &args, &[], BUDGET);
            (ee.stack, ee.pc) = (stack, STUNT_FN);
            ee.set(29, sp);
            ee.set(31, ra);
            let reads = ee.reads.replace(Some(Default::default())).unwrap_or_default();
            let (r, marked) = &mut *t;
            let frame = *at.borrow();
            r.sent.extend(reads.intersection(marked).map(|&a| (frame, a)));
            let gone: Vec<u32> = ee.written.intersection(marked).copied().collect();
            r.overwritten += gone.len();
            ee.written.clear();
        }),
    );
    for (r, row) in rows.iter().enumerate() {
        poke(&mut ee, row, &READER);
        *frame.borrow_mut() = r;
        ee.written.clear();
        *ee.reads.borrow_mut() = Some(Default::default());
        ee.set(28, GP);
        ee.call(FRAME, &[u64::from(SIM)], &[f32::from_bits(DT)], 400_000_000);
        settle(&mut t.borrow_mut(), &mut ee, r);
        *ee.reads.borrow_mut() = None;
        let d = diff(&ee, row);
        assert!(d.is_empty(), "{name} row {r} (vsync {}): {} words differ\n  {}", row["vsync"], d.len(), d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  "));
        t.borrow_mut().0.frames += 1;
        poke(&mut ee, row, &SPOT);
    }
    ee.hooks.remove(&step);
    ee.hooks.remove(&STUNT_FN);
    let (r, _) = Rc::try_unwrap(t).ok().unwrap().into_inner();
    Some(r)
}

/// F42's fake world, in the F39 rig's area (its own words written each case): the hooked calls'
/// functions, the network clock, the steer's filter, the snapshot and its three quadwords, the
/// snapshots' times, the stunt table and its clips.
const N_FNS: u32 = U_AREA + 0x680;
const N_CLOCK: u32 = U_AREA + 0xd00;
const N_FILTER: u32 = U_AREA + 0x1200;
const N_SNAP: u32 = U_AREA + 0x1210;
const N_TIMES: u32 = U_AREA + 0x1280;
const N_TABLE: u32 = U_TABLE;
const N_ANIMS: u32 = U_ANIMS;

fn set_byte(ee: &mut Ee, at: u32, v: u32) {
    let a = at & !3;
    let s = 8 * (at & 3);
    let w = ee.u32_at(a) & !(0xff << s) | (v & 0xff) << s;
    ee.set_u32(a, w);
}

/// **The trick step, the gear and the net check and unpack are the original's**: 10 000 bikes, each
/// the image's with every word the four read drawn at random — the gear byte; remote or not, the next
/// stunt (in the table, none, −1, past it), the kept five (it among them or not), driven or not, the
/// entries' kind bytes (high nibbles among them) and clips, the asked kind, the clips' lengths (zero
/// and negative among them), their `+0x48`s and ids, the time on each of the step's edges (the length,
/// 0.3 s, the length plus the slack, the `+0x48`, less `dt`), the slack, the two answers; the snapshot
/// count (≥ 5 or not, unsigned), the clock, the lags, the snapshots' times about the wanted one (each
/// of the walk's five ends, `t` past both clamps); the snapshot's position, quaternion and velocity,
/// the body's offset, the wheels' rates, spins (past ±π), steer limits and down words, the filter's
/// steer, time (zero among them) — and run through SLUS-21104's four against the port's from the same
/// words. Every word of the bike, its body, its wheels, the fake world and the three globals comes out
/// the same, the calls are the same with the same arguments, and the three answers are. The deeper
/// calls are hooked to log (the node ones through the F39 rig's); the stunt-entry and stunt-of-kind
/// lookups run as the originals.
#[test]
fn the_trick_step_gear_and_net_virtuals_are_the_originals() {
    use update::Out;
    let Some(mut rig) = update_rig() else { return };
    let says = rig.says.clone();
    let dt_is = move |ee: &Ee, what: &str| {
        assert_eq!(ee.f(12).to_bits(), says.borrow().dt.to_bits(), "{what} is handed dt");
    };
    let mut next = N_FNS;
    type Hook = Box<dyn Fn(&mut Ee) -> Out>;
    let mut hook = |ee: &mut Ee, slot: Option<u32>, at: u32, h: Hook, log: Rc<RefCell<Vec<Out>>>| {
        let at = match slot {
            Some(slot) => {
                ee.set_u32(U_VTABLE + slot, next);
                next += 4;
                next - 4
            }
            None => at,
        };
        ee.hooks.insert(at, Box::new(move |ee: &mut Ee| {
            let c = h(ee);
            log.borrow_mut().push(c);
        }));
    };
    let bike_is = |ee: &Ee| assert_eq!(ee.r(4) as u32, BIKE, "a bike virtual handed something other than the bike");
    let (d, s) = (dt_is.clone(), rig.says.clone());
    hook(&mut rig.ee, Some(0x4e8), 0, Box::new(move |ee| {
        bike_is(ee);
        d(ee, "the remote stunt clocks");
        ee.set(2, u64::from(s.borrow().remote));
        Out::TrickRemote
    }), rig.log.clone());
    let d = dt_is.clone();
    hook(&mut rig.ee, Some(0x4ec), 0, Box::new(move |ee| {
        bike_is(ee);
        d(ee, "the clip hold");
        Out::TrickHold(ee.r(5) as u32)
    }), rig.log.clone());
    let (d, s) = (dt_is.clone(), rig.says.clone());
    hook(&mut rig.ee, Some(0x4b4), 0, Box::new(move |ee| {
        bike_is(ee);
        d(ee, "the trick wanted");
        ee.set(2, u64::from(s.borrow().wanted));
        Out::TrickWanted
    }), rig.log.clone());
    let d = dt_is.clone();
    hook(&mut rig.ee, Some(0x4b8), 0, Box::new(move |ee| {
        bike_is(ee);
        d(ee, "the stunt start");
        assert_eq!(ee.r(6), 0, "the stunt start's third argument");
        Out::TrickStart(ee.r(5) as u32)
    }), rig.log.clone());
    let d = dt_is.clone();
    hook(&mut rig.ee, Some(0x4ac), 0, Box::new(move |ee| {
        bike_is(ee);
        d(ee, "the trick end");
        Out::TrickEnd(ee.r(5) as u32)
    }), rig.log.clone());
    hook(&mut rig.ee, Some(0x3ec), 0, Box::new(move |ee| {
        bike_is(ee);
        Out::NetInterpolate { older: ee.r(5) as u32, newer: ee.r(6) as u32, t: ee.f(13).to_bits() }
    }), rig.log.clone());
    let d = dt_is.clone();
    hook(&mut rig.ee, None, MARK_FN, Box::new(move |ee| {
        d(ee, "the stunt mark");
        assert_eq!(ee.r(8), 0, "the stunt mark's fifth argument");
        Out::Mark { run: ee.r(4) as u32, player: ee.r(5) as u32, on: ee.r(6) as u32, id: ee.r(7) as u32 }
    }), rig.log.clone());
    let d = dt_is.clone();
    hook(&mut rig.ee, None, NODE_PLACE, Box::new(move |ee| {
        d(ee, "the node's turn");
        assert_eq!(ee.r(5), 0, "the node turn's second argument");
        Out::Orient { node: ee.r(4) as u32, at: ee.r(6) as u32, pos: ee.r(7) as u32 }
    }), rig.log.clone());
    for (at, what) in [(WHEEL_CLEAR, "the wheel clear"), (SUSPENSION, "the suspension"), (TIRE_FORCES, "the tire forces")] {
        let d = dt_is.clone();
        hook(&mut rig.ee, None, at, Box::new(move |ee| {
            d(ee, what);
            let w = ee.r(4) as u32;
            match at {
                WHEEL_CLEAR => Out::WheelClear(w),
                SUSPENSION => Out::Suspension(w),
                _ => Out::TireForces(w),
            }
        }), rig.log.clone());
    }
    for (at, c) in [(CONTACTS, Out::Contacts), (NET_POST, Out::NetPost)] {
        let d = dt_is.clone();
        hook(&mut rig.ee, None, at, Box::new(move |ee| {
            bike_is(ee);
            d(ee, &format!("{c:?}"));
            c
        }), rig.log.clone());
    }
    rig.regions.extend([(update::TRICK_SLACK, 4), (update::NET_CLOCK, 4), (update::NET_LAG, 4)]);
    let image_words = snap_regions(&rig.ee, &rig.regions);
    let (body, [front, rear]) = (rig.body, rig.wheels);
    let mut g = Gen(0x0000_f42f);
    let mut t = BTreeMap::new();
    let bike = u64::from(BIKE);
    for k in 0..10_000 {
        let ee = &mut rig.ee;
        restore_regions(ee, &rig.regions, &image_words);
        let word = |ee: &mut Ee, o: u32, w: u32| ee.set_u32(BIKE + o, w);
        let put = |ee: &mut Ee, o: u32, v: f32| ee.set_f32(BIKE + o, v);
        let dt = if g.one_in(2) { f32::from_bits(DT) } else { g.f(0.002, 0.05) };
        let (remote, wanted) = (g.bits() % 3, g.bits() % 3);
        *rig.says.borrow_mut() = UpSays { dt, remote, wanted, ..Default::default() };
        // The gear.
        set_byte(ee, rear + 0x110, if g.one_in(4) { g.bits() } else { g.bits() % 6 });
        // The trick step: the table, its clips, the stunt and the clock.
        word(ee, 0x48c, u32::from(g.one_in(8)));
        word(ee, 0x14f0, N_TABLE);
        let kinds = [0, 1, 2, 3, 4, 5, 0x13, 0x24, 0xf4];
        let mut anim = |g: &mut Gen, j: u32| {
            let a = N_ANIMS + 0x60 * j;
            let len = match g.bits() % 6 {
                0 => 0.0,
                1 => -0.5,
                2 => g.f(0.05, 0.3),
                _ => g.f(0.3, 2.0),
            };
            ee.set_f32(a + 0x44, len);
            ee.set_f32(a + 0x48, if g.one_in(3) { len } else { g.f(0.0, 2.0) });
            set_byte(ee, a + 0x58, g.bits());
            (a, len)
        };
        let anims: Vec<(u32, f32)> = (0..8).map(|j| anim(&mut g, j)).collect();
        for i in 0..47 {
            // The header's +0xc word is the −1 stunt's clip.
            let e = if i == 46 { N_TABLE + 0xc } else { N_TABLE + 0x18 + 12 * i };
            ee.set_u32(e, anims[(g.bits() % 8) as usize].0);
            set_byte(ee, e + 8, if g.one_in(3) { kinds[(g.bits() % kinds.len() as u32) as usize] } else { 0x40 });
        }
        let id = match g.bits() % 8 {
            0 => 0xff,
            1 => u32::MAX,
            2 => 46 + g.bits() % 100,
            _ => g.bits() % 46,
        };
        word(ee, 0x1528, id);
        for o in [0x13b8, 0x13bc, 0x13c0, 0x13c4, 0x13c8] {
            word(ee, o, if g.one_in(8) { id } else { g.bits() % 46 });
        }
        word(ee, 0x1194, u32::from(g.one_in(2)));
        word(ee, 0x11d0, u32::from(g.one_in(2)));
        let entry = if id == u32::MAX { N_TABLE + 0xc } else { N_TABLE + 0x18 + 12 * id.min(46) };
        let (clip, len) = if id < 46 || id == u32::MAX { anims.iter().copied().find(|a| a.0 == ee.u32_at(entry)).unwrap() } else { (0, 0.0) };
        let kind = if g.one_in(2) && clip != 0 { ee.u8_at(entry + 8).into() } else { kinds[(g.bits() % kinds.len() as u32) as usize] };
        set_byte(ee, BIKE + 0x14ec, kind | if g.one_in(3) { 0x30 } else { 0 });
        let slack = if g.one_in(4) { 0.0 } else { g.f(0.0, 0.5) };
        ee.set_f32(update::TRICK_SLACK, slack);
        let plus = if clip != 0 { ee.f32_at(clip + 0x48) } else { 1.0 };
        let time = match g.bits() % 10 {
            0 => len,
            1 => len - dt,
            2 => len.min(0.3),
            3 => len.min(0.3) - dt / 4.0,
            4 => len + slack,
            5 => len + slack + dt / 4.0,
            6 => plus - dt,
            7 => plus,
            _ => g.f(0.0, 2.5),
        };
        put(ee, 0x14d8, time);
        put(ee, 0x14e4, g.f(0.0, 1.0));
        word(ee, 0x1788, g.bits() % 3);
        word(ee, 0x14d4, g.bits());
        word(ee, 0x684, g.bits());
        word(ee, 0x680, g.bits());
        // The net check: the count, the clock and lags, the snapshots' times newest first.
        word(ee, 0xd04, match g.bits() % 4 {
            0 => g.bits() % 5,
            1 => 0x8000_0000 | g.bits(),
            _ => 5 + g.bits() % 20,
        });
        ee.set_u32(update::NET_CLOCK, N_CLOCK);
        let now = g.f(0.0, 500.0);
        ee.set_f32(N_CLOCK + 0x4cc, now);
        let (lag, mine) = (if g.one_in(3) { 0.1 } else { g.f(0.0, 0.3) }, g.f(-0.1, 0.3));
        ee.set_f32(update::NET_LAG, lag);
        put(ee, 0xd10, mine);
        let want = now - mine - lag;
        let step = g.f(0.01, 0.2);
        let ends = g.bits() % 7;
        for i in 0..6u32 {
            let at = N_TIMES + 16 * i;
            // Snapshot `ends` the first older than the wanted time (its five slots, two past).
            let t = want + step * (ends as f32 - i as f32 - 0.5) + if g.one_in(3) { g.f(-step, step) } else { 0.0 };
            ee.set_f32(at, if g.one_in(20) { want } else { t });
            word(ee, 0xcd4 + 8 * i, at);
        }
        // The unpack: the snapshot, the body, the wheels, the filter.
        word(ee, 0xbcc, N_SNAP);
        for (k, at) in [N_SNAP + 0x10, N_SNAP + 0x20, N_SNAP + 0x30].into_iter().enumerate() {
            ee.set_u32(N_SNAP + 4 * k as u32, at);
            for c in 0..4 {
                ee.set_f32(at + 4 * c, g.f(-2.0, 2.0) * if k == 0 { 300.0 } else if k == 2 { 40.0 } else { 1.0 });
            }
        }
        for c in 0..4 {
            ee.set_f32(body + 0x1a0 + 4 * c, g.f(-2.0, 2.0));
            ee.set_f32(body + 0xb0 + 4 * c, g.f(-20.0, 20.0));
        }
        for w in [front, rear] {
            ee.set_f32(w + 0x838, g.f(0.1, 1.5));
            ee.set_f32(w + 0x1d0, g.near(0.0, -4.0, 4.0));
            let lim = g.f(0.0, 0.7);
            ee.set_f32(w + 0x70, lim);
            ee.set_f32(w + 0x74, if g.one_in(5) { lim } else { -g.f(0.0, 0.7) });
            ee.set_u32(w + 0xdc, if g.one_in(2) { 0 } else { g.bits() | 1 });
        }
        word(ee, 0xd1c, N_FILTER);
        ee.set_f32(N_FILTER, g.f(-0.7, 0.7));
        ee.set_f32(N_FILTER + 4, if g.one_in(20) { 0.0 } else if g.one_in(3) { dt } else { g.f(0.0, 0.5) });
        word(ee, 0xde0, g.bits() % 3);
        word(ee, 0x324, g.bits() % 3);
        let x = dt;
        let f = F(dt);
        let c = rig.check(k, GEAR_FN, &[bike], x, |m| update::gear(m));
        assert!(c.is_empty(), "case {k}: the gear calls {c:?}");
        tally(&mut t, if rig.ee.u8_at(rear + 0x110) < 6 { "gear, a gear" } else { "gear, past them" });
        let c = rig.check(k, TRICK_FN, &[bike], x, |m| update::trick(m, f));
        let has = |p: &dyn Fn(&Out) -> bool| c.iter().any(p);
        tally(&mut t, match () {
            _ if has(&|c| *c == Out::TrickRemote) => "trick, remote",
            _ if has(&|c| matches!(c, Out::TrickHold(_))) => "trick, held",
            _ if has(&|c| matches!(c, Out::TrickStart(_))) => "trick, started",
            _ if has(&|c| matches!(c, Out::TrickEnd(_))) => "trick, ended",
            _ if has(&|c| matches!(c, Out::Mark { on: 0, .. })) => "trick, runs on",
            _ if has(&|c| *c == Out::TrickWanted) => "trick, none wanted or of the kind",
            _ => "trick, a kept stunt runs on",
        });
        if has(&|c| matches!(c, Out::Mark { on: 1, .. })) {
            tally(&mut t, "trick, it fits");
        }
        if id >= 46 && id != u32::MAX {
            tally(&mut t, "trick, no stunt");
        }
        let c = rig.check(k, NET_CHECK_FN, &[bike], x, |m| update::net_check(m));
        tally(&mut t, match c.first() {
            None => "net check, too few",
            Some(&Out::NetInterpolate { t, .. }) => match f32::from_bits(t) {
                t if t == f32::from_bits(0xbf66_6666) => "net check, clamped low",
                t if t == f32::from_bits(0x3ff3_3333) => "net check, clamped high",
                _ => "net check, between",
            },
            Some(c) => unreachable!("{c:?}"),
        });
        if let Some(&Out::NetInterpolate { older, .. }) = c.first() {
            tally(&mut t, ["net check, older 1", "net check, older 2", "net check, older 3", "net check, older 4", "net check, older 5", "net check, older 6"]
                [((older - N_TIMES) / 16) as usize]);
        }
        let c = rig.check(k, NET_UNPACK_FN, &[bike], x, |m| {
            update::net_unpack(m, f);
            0
        });
        assert_eq!(c.len(), 10, "case {k}: the unpack's calls {c:?}");
        let down = [front, rear].map(|w| rig.ee.u32_at(w + 0xdc) != 0);
        tally(&mut t, match down {
            [true, true] => "unpack, both down",
            [false, false] => "unpack, both up",
            _ => "unpack, one down",
        });
    }
    eprintln!("{t:#?}");
    for (path, n) in &t {
        assert!(*n >= 20, "path {path} taken only {n} times");
    }
    assert!(t.len() >= 20, "every path taken: {t:#?}");
}

/// F44's fake world: up to [`V_NODES`] scene nodes hung under the trail, 0x30 apart, and a node
/// class whose shown and hidden are hooks.
const V_AREA: u32 = 0x01c1_0000;
const V_NODES: u32 = 14;
const V_VT: u32 = V_AREA + 0x400;
const V_FNS: u32 = V_AREA + 0x480;
const V_LEN: u32 = 0x500;

/// **The trail's, the net sender's and the parts' virtuals are the original's** (F44): 10 000
/// cases on state 4's objects, each run through SLUS-21104's and then the port's from the same words.
/// The trail on or off, with a random tree of nodes under it (state 4's node class, or one whose
/// shown and hidden are hooks; the kept-apart bit 3 and the rest of `+0x20` at random); the bike's
/// send with the sender on or off and its beat −2 to 12 or anything, the packets hooked to log and to
/// change the sender's bit and the beat now and then (so the re-reads count); the record's and the
/// second part's steps, `+0x124` at random, the walk hooked. Every word of the bike, the trail, the
/// tree and both parts comes out the same, the calls are the same, in order, with the same arguments
/// (each hook also asserts f12 = `dt`), and so is the send's answer.
#[test]
fn the_trail_sender_and_part_virtuals_are_the_originals() {
    use update::Out;
    let Some(mut ee) = image().map(hooked) else { return };
    let net = BIKE + update::NET;
    let trail = ee.u32_at(BIKE + 0x10dc);
    let parts = [ee.u32_at(BIKE + update::RECORD), ee.u32_at(BIKE + 0xd4)];
    let class = ee.u32_at(ee.u32_at(trail + 0x10) + 4);
    let slot = |ee: &Ee, o: u32, at: u32, s: u32| ee.u32_at(ee.u32_at(o + at) + s);
    for (o, f) in [(trail, [NODE_ON, NODE_OFF]), (ee.u32_at(trail + 0x10), [NODE_ON, NODE_OFF])] {
        assert_eq!([0x18, 0x20].map(|s| slot(&ee, o, 4, s)), f, "state 4's node on and off");
        assert_eq!([0x28, 0x2c].map(|s| slot(&ee, o, 4, s)), [NODE_SHOWN, NODE_HIDDEN], "state 4's node shown and hidden");
    }
    assert_eq!(slot(&ee, net, 0x38, 0x20), SEND_THUNK, "the sender's send");
    assert_eq!(parts.map(|p| slot(&ee, p, 0xc, 0xbc)), PART_STEPS, "the parts' steps");
    for o in (0..V_LEN).step_by(4) {
        ee.set_u32(V_AREA + o, 0);
    }
    let log: Rc<RefCell<Vec<Out>>> = Rc::default();
    let dt = Rc::new(RefCell::new(0f32));
    type Says = Box<dyn Fn(&mut Ee) -> Out>;
    let mut hook = |ee: &mut Ee, at: u32, h: Says| {
        let (l, d) = (log.clone(), dt.clone());
        ee.hooks.insert(at, Box::new(move |ee: &mut Ee| {
            assert_eq!(ee.f(12).to_bits(), d.borrow().to_bits(), "a hooked call is handed dt");
            let c = h(ee);
            l.borrow_mut().push(c);
        }));
    };
    ee.set_u32(V_VT + 0x28, V_FNS);
    ee.set_u32(V_VT + 0x2c, V_FNS + 4);
    hook(&mut ee, V_FNS, Box::new(|ee| Out::Shown(ee.r(4) as u32)));
    hook(&mut ee, V_FNS + 4, Box::new(|ee| Out::Hidden(ee.r(4) as u32)));
    // The packets: each may turn the sender off or on and move the beat, from the words it finds.
    let packet = |ee: &mut Ee, c: fn(u32) -> Out| {
        assert_eq!(ee.r(4) as u32, BIKE, "a packet of something other than the bike");
        let h = mix(u64::from(ee.u32_at(BIKE + 0x414)) << 32 ^ u64::from(ee.u32_at(BIKE + 0xcfc)) ^ u64::from(ee.r(5) as u32));
        if h % 5 == 0 {
            ee.set_u32(BIKE + 0x414, ee.u32_at(BIKE + 0x414) ^ 0x100);
        }
        if h / 5 % 4 == 0 {
            ee.set_u32(BIKE + 0xcfc, ((h >> 8) as u32 % 14).wrapping_sub(2));
        }
        c(ee.r(5) as u32)
    };
    for (s, c) in [(0x544, Out::NetStunt as fn(u32) -> Out), (0x400, Out::NetFull), (0x404, Out::NetDelta)] {
        let at = vt(&ee, s);
        hook(&mut ee, at, Box::new(move |ee| packet(ee, c)));
    }
    hook(&mut ee, TAIL, Box::new(|ee| Out::Walk(ee.r(4) as u32)));
    let regions = vec![(BIKE, 0x1820), (trail, 0x40), (V_AREA, V_LEN), (parts[0], 0x130), (parts[1], 0x130)];
    let image_words = snap_regions(&ee, &regions);
    let mut g = Gen(0x0000_f44f);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        restore_regions(&mut ee, &regions, &image_words);
        let x = g.f(0.0, 0.05);
        *dt.borrow_mut() = x;
        let (f, ints, c) = match g.bits() % 4 {
            kind @ (0 | 1) => {
                ee.set_u32(trail + 0x20, g.bits());
                ee.set_u32(trail + 0x10, 0);
                let n = g.bits() % (V_NODES + 1);
                let mut last = [0u32; V_NODES as usize + 1];
                for i in 0..n {
                    let at = V_AREA + 0x30 * i;
                    ee.set_u32(at + 4, if g.one_in(4) { V_VT } else { class });
                    ee.set_u32(at + 0x20, g.bits());
                    let p = g.bits() % (i + 1);
                    let parent = if p == 0 { trail } else { V_AREA + 0x30 * (p - 1) };
                    match last[p as usize] {
                        0 => ee.set_u32(parent + 0x10, at),
                        prev => ee.set_u32(prev + 0xc, at),
                    }
                    last[p as usize] = at;
                }
                tally(&mut t, if n == 0 { "trail, no nodes" } else { "trail, a tree" });
                if kind == 0 {
                    (NODE_ON, vec![u64::from(trail)], Out::TrailOn(trail))
                } else {
                    (NODE_OFF, vec![u64::from(trail)], Out::TrailOff(trail))
                }
            }
            2 => {
                let on = if g.one_in(2) { 0x100 } else { 0 };
                ee.set_u32(BIKE + 0x414, g.bits() & !0x100 | on);
                let beat = if g.one_in(8) { g.bits() } else if g.one_in(4) { 0 } else { (g.bits() % 15).wrapping_sub(2) };
                ee.set_u32(BIKE + 0xcfc, beat);
                let arg = g.bits();
                (SEND_THUNK, vec![u64::from(net), u64::from(arg)], Out::Send { net, arg })
            }
            _ => {
                let p = parts[g.bits() as usize % 2];
                ee.set_u32(p + 0x124, g.bits());
                tally(&mut t, if p == parts[0] { "part, the record's" } else { "part, the second" });
                (slot(&ee, p, 0xc, 0xbc), vec![u64::from(p)], Out::Part(p))
            }
        };
        let before = snap_regions(&ee, &regions);
        log.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(f, &ints, &[x], BUDGET);
        let want_v0 = ee.r(2) as u32;
        let want = std::mem::take(&mut *log.borrow_mut());
        let want_words = snap_regions(&ee, &regions);
        restore_regions(&mut ee, &regions, &before);
        // The walk handed on to the original, which the rig's hook stands in for.
        IN_RUST.with(|r| *r.borrow_mut() = vec!["the walk handed on"]);
        let v0 = UpdateInRam { ee: &mut ee, x }.out(c);
        IN_RUST.with(|r| r.borrow_mut().clear());
        let got = std::mem::take(&mut *log.borrow_mut());
        assert_eq!(format!("{got:?}"), format!("{want:?}"), "case {k} ({c:?}): the calls");
        let got_words = snap_regions(&ee, &regions);
        if got_words != want_words {
            let names: Vec<(u32, u32)> = regions.iter().flat_map(|&(at, len)| (0..len).step_by(4).map(move |o| (at, o))).collect();
            let d: Vec<String> = (0..got_words.len())
                .filter(|&i| got_words[i] != want_words[i])
                .map(|i| format!("{:#x}+{:#x}: port {:#010x}, original {:#010x}", names[i].0, names[i].1, got_words[i], want_words[i]))
                .collect();
            panic!("case {k} ({c:?}): {} words differ\n  {}", d.len(), d.iter().take(20).cloned().collect::<Vec<_>>().join("\n  "));
        }
        let shown = want.iter().filter(|c| matches!(c, Out::Shown(_) | Out::Hidden(_))).count();
        if shown > 0 {
            tally(&mut t, "trail, hooked nodes reached");
        }
        if matches!(c, Out::TrailOn(_) | Out::TrailOff(_)) {
            let skipped = (0..V_NODES).any(|i| ee.u32_at(V_AREA + 0x30 * i + 4) != 0 && ee.u32_at(V_AREA + 0x30 * i + 0x20) & 0x800 != 0);
            if skipped {
                tally(&mut t, "trail, a node kept apart");
            }
            if want_words == before {
                tally(&mut t, "trail, no change");
            }
        }
        if let Out::Send { .. } = c {
            assert_eq!(v0, want_v0, "case {k}: the send's answer");
            for (p, what) in [(Out::NetStunt(0), "send, the stunt's"), (Out::NetFull(0), "send, the whole"), (Out::NetDelta(0), "send, the change")] {
                if want.iter().any(|c| std::mem::discriminant(c) == std::mem::discriminant(&p)) {
                    tally(&mut t, what);
                }
            }
            let beat = before[0xcfc / 4];
            if before[0x414 / 4] & 0x100 == 0 {
                tally(&mut t, "send, off");
            } else if beat >= 10 {
                tally(&mut t, "send, beat out of range");
            }
            if want.len() > 1 {
                tally(&mut t, "send, stunt then another");
            }
        }
    }
    eprintln!("{t:#?}");
    for (path, n) in &t {
        assert!(*n >= 100, "path {path:?} taken only {n} times");
    }
    assert_eq!(t.len(), 13, "every path taken");
}

/// **F44a — the bike's three net packets, their quantisers and the sender's own send, the
/// originals**: on the state04 image, 10 000 cases each run the original from the same words and then
/// the port through [`UpdateInRam`] (so its slot checks run too), the connection's queue hooked under
/// both. The stunt's, the whole state's and the change's packet off random words, counter and clock;
/// the sender's send, state 4's hash, the table's lookup, keep and quantise called alone. Every word of
/// the bike, the table and its entries, the net record and its vectors matches, and so do the queued
/// packets byte for byte and every answer.
#[test]
fn the_bikes_net_packets_are_the_originals() {
    use update::{Out, Packet};
    let Some(mut ee) = image().map(hooked) else { return };
    let net = BIKE + update::NET;
    assert_eq!([0x544, 0x400, 0x404].map(|s| vt(&ee, s)), [STUNT_FN, FULL_FN, DELTA_FN], "the bike's three packets");
    assert_eq!(ee.u32_at(ee.u32_at(net + 0x38) + 0x28), SENDER_SEND, "the sender's own send");
    let table = ee.u32_at(BIKE + update::QUANTISERS);
    assert_eq!([ee.u32_at(table + 0x10), ee.u32_at(table + 0x14)], [KEY_HASH, KEY_SAME], "state 4's hash and key test");
    let entries = ee.u32_at(table + 0x30);
    let rec = ee.u32_at(BIKE + update::NET_FIELDS);
    let vecs = [0, 4, 8].map(|o| ee.u32_at(rec + o));
    let keys: Vec<u32> = [vecs[0], vecs[2], vecs[1]].iter().flat_map(|&v| [v, v + 4, v + 8]).collect();
    let clock = ee.u32_at(update::NET_CLOCK) + 0x4cc;
    let (counter, alt, out) = (ee.scratch(COUNTER_K), ee.scratch(248), ee.scratch(249));
    let log: Rc<RefCell<Vec<Out>>> = Rc::default();
    let l = log.clone();
    ee.hooks.insert(NET_QUEUE, Box::new(move |ee: &mut Ee| {
        let h = ee.r(5) as u32;
        let head = [ee.u8_at(h), ee.u8_at(h + 1)];
        let buf = ee.u32_at(h + 4);
        let mut packet = Packet { len: u32::from(head[1]), ..Packet::EMPTY };
        for k in 0..usize::from(head[1]) {
            packet.bytes[k] = ee.u8_at(buf + k as u32);
        }
        l.borrow_mut().push(Out::NetQueue { conn: ee.r(4) as u32, head, packet, flags: ee.r(6) as u32 });
        ee.set(2, 1);
    }));
    let regions = vec![
        (BIKE, 0x1820),
        (table, 0x38),
        (entries, 9 * 0x18),
        (rec, 0x28),
        (vecs[1], 0x10),
        (vecs[2], 0x10),
        (update::TIMER_COUNTS, 4),
        (clock, 4),
        (counter, 4),
        (alt, 0x10),
        (out, 0x10),
    ];
    let image_words = snap_regions(&ee, &regions);
    let mut g = Gen(0x0000_f44a);
    let val = |g: &mut Gen| if g.one_in(16) { g.bits() } else { g.near(0.0, -300.0, 300.0).to_bits() };
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        restore_regions(&mut ee, &regions, &image_words);
        let kind = g.bits() % 8;
        // The words the packets read: the nine, the second vector's w, the stunt's five, the levers.
        if kind < 3 {
            for &key in &keys {
                ee.set_u32(key, val(&mut g));
            }
            ee.set_u32(vecs[1] + 0xc, val(&mut g));
            for o in [0x10, 0x14, 0x18, 0x1c, 0x20] {
                ee.set_u32(ee.u32_at(rec + o), g.bits());
            }
            ee.set_f32(BIKE + 0x14e8, g.near(0.0, -2.0, 2.0));
            ee.set_f32(BIKE + 0x318, g.near(0.1, 0.0, 0.2));
            ee.set_u32(clock, g.bits());
            ee.set_u32(update::TIMER_COUNTS, if g.one_in(2) { g.bits() % 100_000 } else { g.bits() });
            ee.set_u32(counter, g.bits());
            ee.set_u32(net, g.bits());
            ee.set_u32(net + 0x10, g.bits());
            for i in 0..4 {
                ee.set_u32(alt + 4 * i, val(&mut g));
            }
            if g.one_in(6) {
                ee.set_u32(rec + 4 * (g.bits() % 3), alt);
            }
        }
        // The quantisers: their last, scale, step and dead band.
        if kind == 2 || kind >= 5 {
            for i in 0..9 {
                let e = entries + 0x18 * i;
                let key = keys[ee.u32_at(e) as usize % 9];
                let x = f32::from_bits(ee.u32_at(key));
                let last = if g.one_in(3) { x + g.f(-0.01, 0.01) } else { g.near(0.0, -300.0, 300.0) };
                ee.set_f32(e + 4, last);
                let scale = (1u32 << (g.bits() % 26)) as f32 * g.f(0.5, 1.0);
                ee.set_f32(e + 0xc, scale);
                ee.set_f32(e + 0x10, g.f(0.5, 1.5) / scale);
                ee.set_f32(e + 0x14, if g.one_in(3) { 0.0 } else { g.f(0.0, 0.02) });
            }
        }
        let arg = g.bits();
        let key = match g.bits() % 4 {
            0 | 1 => keys[(g.bits() % 9) as usize],
            2 => keys[(g.bits() % 9) as usize] ^ 4 << (g.bits() % 4),
            _ => g.bits(),
        };
        let x = f32::from_bits(val(&mut g));
        let max = (g.bits() % 6).wrapping_sub(1);
        if kind == 7 && g.one_in(2) {
            for i in 0..9 {
                ee.set_f32(entries + 0x18 * i + 4, x + g.f(-0.01, 0.01));
            }
        }
        // Exactly on and beside the byte-size thresholds: scale 1 from a last of 0.
        let x = if kind == 7 && g.one_in(4) {
            for i in 0..9 {
                let e = entries + 0x18 * i;
                ee.set_f32(e + 4, 0.0);
                ee.set_f32(e + 0xc, 1.0);
                ee.set_f32(e + 0x10, 1.0);
                ee.set_f32(e + 0x14, 0.0);
            }
            let edge = [0x80, 0x8000, 0x80_0000][(g.bits() % 3) as usize] as f32 - (g.bits() % 2) as f32;
            if g.one_in(2) { -edge } else { edge }
        } else {
            x
        };
        let (a4, a5) = (if g.one_in(2) { 0 } else { g.bits() }, g.bits());
        let len = g.bits() % 65;
        if kind == 3 {
            for i in 0..len {
                ee.write(ee.scratch(PACKET_K) + i, 1, u128::from(g.bits() as u8));
            }
        }
        let u = u64::from;
        let (f, ints, floats) = match kind {
            0 => (STUNT_FN, vec![u(BIKE), u(arg)], vec![]),
            1 => (FULL_FN, vec![u(BIKE), u(arg)], vec![]),
            2 => (DELTA_FN, vec![u(BIKE), u(arg)], vec![]),
            3 => (SENDER_SEND, vec![u(net), u(arg), u(ee.scratch(PACKET_K)), u(len), u(a4), u(a5)], vec![]),
            4 => (KEY_HASH, vec![key as i32 as i64 as u64], vec![]),
            5 => (Q_LOOKUP, vec![u(table), u(key), u(out)], vec![]),
            6 => (Q_KEEP, vec![u(table), u(key)], vec![x]),
            _ => (Q_QUANTISE, vec![u(table), u(key), u(out), max as i32 as i64 as u64], vec![x]),
        };
        let before = snap_regions(&ee, &regions);
        log.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(f, &ints, &floats, BUDGET);
        let want_v0 = ee.r(2) as u32;
        let want = std::mem::take(&mut *log.borrow_mut());
        let want_words = snap_regions(&ee, &regions);
        restore_regions(&mut ee, &regions, &before);
        let mut m = UpdateInRam { ee: &mut ee, x: 0.0 };
        let v0 = match kind {
            0 => m.out(Out::NetStunt(arg)),
            1 => m.out(Out::NetFull(arg)),
            2 => m.out(Out::NetDelta(arg)),
            3 => {
                let mut packet = Packet { len, ..Packet::EMPTY };
                for i in 0..len {
                    packet.bytes[i as usize] = m.ee.u8_at(m.ee.scratch(PACKET_K) + i);
                }
                update::sender_send(&mut m, net, arg, packet, a4, a5);
                want_v0
            }
            4 => m.out(Out::Hash { f: KEY_HASH, key }),
            5 => match update::lookup(&mut m, table, key) {
                Some(i) => {
                    m.ee.set_u32(out, i);
                    1
                }
                None => 0,
            },
            6 => update::keep(&mut m, x.to_bits(), table, key),
            _ => {
                let mut b: [u8; 4] = std::array::from_fn(|i| m.ee.u8_at(out + i as u32));
                let n = update::quantise(&mut m, x.to_bits(), table, key, &mut b, max);
                for (i, &b) in b.iter().enumerate() {
                    m.ee.write(out + i as u32, 1, u128::from(b));
                }
                n
            }
        };
        let got = std::mem::take(&mut *log.borrow_mut());
        assert_eq!(format!("{got:?}"), format!("{want:?}"), "case {k} (kind {kind}): the queued packets");
        if kind >= 4 {
            assert_eq!(v0, want_v0, "case {k} (kind {kind}): the answer");
        }
        let got_words = snap_regions(&ee, &regions);
        if got_words != want_words {
            let names: Vec<(u32, u32)> = regions.iter().flat_map(|&(at, len)| (0..len).step_by(4).map(move |o| (at, o))).collect();
            let d: Vec<String> = (0..got_words.len())
                .filter(|&i| got_words[i] != want_words[i])
                .map(|i| format!("{:#x}+{:#x}: port {:#010x}, original {:#010x}", names[i].0, names[i].1, got_words[i], want_words[i]))
                .collect();
            panic!("case {k} (kind {kind}): {} words differ\n  {}", d.len(), d.iter().take(20).cloned().collect::<Vec<_>>().join("\n  "));
        }
        let missing = (0..3).any(|i| before[regions[..3].iter().map(|r| r.1 / 4).sum::<u32>() as usize + i] == alt);
        let sent = |n: usize| match &want[..] {
            [Out::NetQueue { packet, .. }] => packet.bytes[n],
            _ => panic!("case {k}: one packet queued, not {want:?}"),
        };
        match kind {
            0 => {
                tally(&mut t, if sent(0) & 4 != 0 { "stunt, up" } else { "stunt, down" });
                if sent(4) & 0x80 != 0 {
                    tally(&mut t, "stunt, stamp past 2^31");
                }
            }
            1 | 2 => {
                let what = if kind == 1 { ["whole, every key", "whole, a key missing", "whole, turned"] } else { ["change, every key", "change, a key missing", "change, turned"] };
                tally(&mut t, if missing { what[1] } else { what[0] });
                let turned = if kind == 1 { sent(0) & 4 != 0 } else { sent(2) & 0x10 != 0 };
                if turned {
                    tally(&mut t, what[2]);
                }
                if kind == 2 {
                    let mask = u32::from(sent(0)) | u32::from(sent(1)) << 8 | u32::from(sent(2)) << 16;
                    if mask & 0x40_0000 != 0 {
                        tally(&mut t, "change, fast");
                    }
                    if !missing {
                        for i in 0..9 {
                            tally(&mut t, ["change, 1 byte", "change, 2 bytes", "change, 3 bytes", "change, 4 bytes"][(mask >> (2 + 2 * i) & 3) as usize]);
                        }
                    }
                }
            }
            3 => tally(&mut t, if a4 == 0 { "sender, its own flags" } else { "sender, flags handed" }),
            4 => tally(&mut t, if (key as i32) < 0 { "hash, negative" } else { "hash, positive" }),
            5 | 6 => tally(&mut t, match (kind, want_v0) {
                (5, 0) => "lookup, missed",
                (5, _) => "lookup, found",
                (_, 0) => "keep, missed",
                _ => "keep, found",
            }),
            _ => {
                let e_changed = want_words != before;
                tally(&mut t, match want_v0 {
                    0 if (max as i32) < 4 && keys.contains(&key) => "quantise, no room",
                    0 => "quantise, missed",
                    1 if !e_changed => "quantise, dead band",
                    _ => "quantise, sent",
                });
            }
        }
    }
    eprintln!("{t:#?}");
    for (path, n) in &t {
        assert!(*n >= 100, "path {path:?} taken only {n} times");
    }
    assert_eq!(t.len(), 26, "every path taken");
}

/// **F31b5k — the connection's queue, the original's**: on the state04 image, 10 000 cases each fill
/// the bike's connection's five queues with random messages up to a random room (state 4's 1300
/// bytes one time in four, the rest small, so full queues are common), its connected word and socket
/// drawn so the flush is and is not taken, and queue one message (a random header and up to 64
/// bytes) under random flags. SLUS-21104's queue and [`update::net_queue`] (through [`UpdateInRam`],
/// so the rig's serving is what runs) start from the same words; the socket's send is hooked under
/// both, to log its arguments and answer a drawn success or failure. Every word of the connection,
/// the queues' buffers and the flush buffer matches, the sends and the answer match, and the original
/// writes nothing else.
#[test]
fn the_connections_queue_is_the_originals() {
    use std::cell::Cell;
    let Some(mut ee) = image() else { return };
    let conn = ee.u32_at(BIKE + update::NET + 0x34);
    assert_eq!(ee.u32_at(ee.u32_at(BIKE + update::NET + 0x38) + 0x28), SENDER_SEND, "the sender's own send");
    assert!(ee.u32_at(conn + update::CONN_UP) == 0 && (ee.u32_at(conn + update::CONN_SOCKET) as i32) < 0, "state 4 is offline");
    let starts = update::CONN_QUEUES.map(|q| ee.u32_at(conn + q + 4));
    for (k, &q) in update::CONN_QUEUES.iter().enumerate() {
        assert_eq!(ee.u32_at(conn + q), starts[k], "queue {q:#x}: the count byte leads its buffer");
        assert_eq!(ee.u32_at(conn + q + 12), 0x514, "queue {q:#x}: state 4's room");
    }
    const SPAN: u32 = 0x530;
    let mut regions = vec![(conn, 0x130), (update::FLUSH_BUFFER, SPAN)];
    regions.extend(starts.iter().map(|&s| (s, SPAN)));
    let (head, buf) = (ee.scratch(HEAD_K), ee.scratch(PACKET_K));
    let sends: Rc<RefCell<Vec<[u64; 6]>>> = Rc::default();
    let answer = Rc::new(Cell::new(0u32));
    let (l, a) = (sends.clone(), answer.clone());
    ee.hooks.insert(NET_FLUSH, Box::new(move |ee: &mut Ee| {
        l.borrow_mut().push(std::array::from_fn(|i| ee.r(4 + i)));
        ee.set(2, a.get() as i32 as i64 as u64);
    }));
    let image_words = snap_regions(&ee, &regions);
    let mut g = Gen(0x0031_b5cb);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        restore_regions(&mut ee, &regions, &image_words);
        for &(at, len) in &regions[1..] {
            for o in (0..len).step_by(4) {
                ee.set_u32(at + o, g.bits());
            }
        }
        let up = if g.one_in(3) { 0 } else { g.bits() | 1 };
        let socket = if g.one_in(3) { g.bits() | 0x8000_0000 } else { g.bits() % 64 };
        ee.set_u32(conn + update::CONN_UP, up);
        ee.set_u32(conn + update::CONN_SOCKET, socket);
        let len = if g.one_in(4) { 64 } else { g.bits() % 65 };
        for (q, &start) in update::CONN_QUEUES.iter().zip(&starts) {
            let room = match g.bits() % 8 {
                0 | 1 => 0x514,
                2 => g.bits() % 8,
                _ => 2 + g.bits() % 200,
            };
            // Full more often than not: filled to within a message of the room.
            // Small messages packed to the brim, one time in three, so no one drop makes room.
            let small = g.one_in(3);
            let fill = if small { room } else if g.one_in(3) { g.bits() % (room + 1) } else { room.saturating_sub(g.bits() % 70) };
            let (mut end, mut count) = (start + 1, 0u32);
            while count < 255 {
                let n = if small { g.bits() % 4 } else if g.one_in(3) { g.bits() % 8 } else { g.bits() % 65 };
                if end - start + 2 + n > fill {
                    break;
                }
                ee.write(end + 1, 1, u128::from(n));
                end += 2 + n;
                count += 1;
            }
            ee.write(start, 1, u128::from(count));
            for (o, w) in [(0, start), (4, start), (8, end), (12, room)] {
                ee.set_u32(conn + q + o, w);
            }
        }
        let flags = match g.bits() % 6 {
            0 | 1 => 0,
            2 => 2,
            3 => 0x10,
            4 => 0x12,
            _ => g.bits(),
        };
        let id = g.bits() as u8;
        let mut packet = update::Packet { len, ..update::Packet::EMPTY };
        for b in &mut packet.bytes[..len as usize] {
            *b = g.bits() as u8;
        }
        ee.write(head, 2, u128::from(u16::from_le_bytes([id, len as u8])));
        ee.set_u32(head + 4, buf);
        for (i, &b) in packet.data().iter().enumerate() {
            ee.write(buf + i as u32, 1, u128::from(b));
        }
        answer.set(if g.one_in(2) { g.bits() % 2000 } else { g.bits() | 0x8000_0000 });
        let before = snap_regions(&ee, &regions);
        let queues_before: Vec<(u32, u32)> = starts.iter().zip(update::CONN_QUEUES).map(|(&s, q)| (ee.u8_at(s).into(), ee.u32_at(conn + q + 8))).collect();

        ee.set(28, GP);
        ee.call(NET_QUEUE, &[u64::from(conn), u64::from(head), u64::from(flags)], &[], BUDGET);
        let want_v0 = ee.r(2) as u32;
        let outside = written_outside(&ee, &regions);
        assert_eq!(outside, Vec::<String>::new(), "case {k}: the original writes only the connection and its buffers");
        let want = snap_regions(&ee, &regions);
        let want_sends = std::mem::take(&mut *sends.borrow_mut());
        let queues_after: Vec<(u32, u32)> = starts.iter().zip(update::CONN_QUEUES).map(|(&s, q)| (ee.u8_at(s).into(), ee.u32_at(conn + q + 8))).collect();

        restore_regions(&mut ee, &regions, &before);
        let v0 = update::net_queue(&mut UpdateInRam { ee: &mut ee, x: 0.0 }, conn, [id, len as u8], &packet, flags);
        let got = snap_regions(&ee, &regions);
        let got_sends = std::mem::take(&mut *sends.borrow_mut());
        assert_eq!(got_sends, want_sends, "case {k}: the socket's sends");
        assert_eq!(v0, want_v0, "case {k}: the answer");
        if got != want {
            let names: Vec<(u32, u32)> = regions.iter().flat_map(|&(at, len)| (0..len).step_by(4).map(move |o| (at, o))).collect();
            let d: Vec<String> = (0..got.len())
                .filter(|&i| got[i] != want[i])
                .map(|i| format!("{:#x}+{:#x}: port {:#010x}, original {:#010x}", names[i].0, names[i].1, got[i], want[i]))
                .collect();
            panic!("case {k} (flags {flags:#x}): {} words differ\n  {}", d.len(), d.iter().take(20).cloned().collect::<Vec<_>>().join("\n  "));
        }

        tally(&mut t, match flags & 0x12 {
            0 => "plain",
            0x10 => "bit 4",
            _ => "bit 1",
        });
        if want_v0 == 0 {
            tally(&mut t, "refused");
        }
        for s in &want_sends {
            tally(&mut t, match (s[4], (answer.get() as i32) < 0) {
                (_, true) => "flushed, failed",
                (0, _) => "flushed, sent",
                _ => "flushed reliably, sent",
            });
        }
        let need = len + 2;
        for (i, (&(cb, eb), &(ca, ea))) in queues_before.iter().zip(&queues_after).enumerate() {
            let start = starts[i];
            tally(&mut t, match () {
                _ if (cb, eb) == (ca, ea) => continue,
                _ if ca == (cb + 1) & 0xff && ea == eb + need => "pushed into room",
                _ if (ca, ea) == (0, start + 1) => "emptied by the flush",
                _ if (ca, ea) == (1, start + 1 + need) => "pushed, every message dropped",
                _ => "pushed, some messages dropped",
            });
        }
    }
    eprintln!("{t:#?}");
    for (path, n) in &t {
        assert!(*n >= 100, "path {path:?} taken only {n} times");
    }
    assert_eq!(t.len(), 11, "every path taken");
}

// --- F31b4c: the bike's mass properties, the port's -------------------------------------------------

use game_sim::handling::mass;

/// State 4's bike's file: `mx_race250p.phy` off the disc (`MXVSATV.PSR`, `Veh\mx_race250p.phy`).
/// Every 420-lb bike's `[Mass Properties]` is the same, byte for byte.
fn race250p() -> Option<Vec<u8>> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2/phy").join("mx_race250p.phy");
    if !path.exists() {
        skip(&format!(
            "{} is not there: Veh\\mx_race250p.phy out of MX vs. ATV's MXVSATV.PSR (F31b4c's journal)",
            path.display()
        ));
        return None;
    }
    Some(std::fs::read(path).unwrap())
}

/// The model box the part reader was handed at load, as `(x, y, z)` sides' halves: the one whose
/// block inertia ([`mass::block_inertia`], then [`mass::record_inertia`]) is the record's, bit for
/// bit. The record keeps the load's inertia (the bike's loader puts its own over the body's only);
/// the box the game's box call gives for the bike's node in state 4 is not it any more (the wheels'
/// nodes have moved), so it is worked back from the record: the squares solved for, then the floats
/// around them walked until every lane is the record's.
fn load_box(part: &mass::Part, want: [u32; 3]) -> Option<Vf> {
    let (m, (cm, _)) = (part.body.mass, part.record.mass?);
    let (s, k) = (part.body.inertia_scale, f64::from(cm.0) * f64::from(part.body.inv_mass.0));
    let m12 = f64::from(m.0) / 12.0;
    let w = want.map(|b| f64::from(f32::from_bits(b)) / k);
    let sums = [w[0] / (f64::from(s.x()) * m12), w[1] / (f64::from(s.y()) * m12), w[2] / (f64::from(s.z()) * m12)];
    // y² + z², x² + z², x² + y²: each square the half of the other two sums less its own.
    let sq = [(sums[1] + sums[2] - sums[0]) / 2.0, (sums[0] + sums[2] - sums[1]) / 2.0, (sums[0] + sums[1] - sums[2]) / 2.0];
    let guess = sq.map(|v| (v.sqrt() / 2.0) as f32);
    let near = |g: f32, d: i32| f32::from_bits((g.to_bits() as i32 + d) as u32);
    const R: i32 = 48;
    for dx in -R..=R {
        for dy in -R..=R {
            for dz in -R..=R {
                let half = Vf::new(near(guess[0], dx), near(guess[1], dy), near(guess[2], dz), 1.0);
                let (i, _) = mass::record_inertia(mass::block_inertia(m, s, half), cm, part.body.inv_mass);
                if [i.x(), i.y(), i.z()].map(f32::to_bits) == want {
                    return Some(half);
                }
            }
        }
    }
    None
}

/// The port's `[Mass Properties]` words for state 4's bike, as `(address, mask, value)`, each
/// address once (its last write): the part reader's ([`mass::Part`]) on the load's box
/// ([`load_box`]), then the bike loader's ([`mass::Bike`]) over them.
fn mass_words(phy: &[u8]) -> Option<Vec<(u32, u32, u32)>> {
    let ee = image()?;
    let ini = game_sim::phy::Ini::parse_bytes(phy);
    let s = ini.section("Mass Properties");
    let at = mass::Layout {
        object: BIKE,
        body: ee.u32_at(BIKE + seed::BODY),
        record: ee.u32_at(BIKE + seed::RECORD),
        gravity: ee.u32_at(BIKE + 0xe8),
    };
    let curve = game_sim::camera::CurveTable::build();
    let probe = mass::Part::read(&s, Vf::new(1.0, 1.0, 1.0, 1.0), &curve);
    let want = std::array::from_fn(|k| ee.u32_at(at.record + 0x60 + 4 * k as u32));
    let half = load_box(&probe, want).expect("no box near the solved one gives the record's inertia");
    eprintln!("the load's half box: {:?}", half.to_bits().map(f32::from_bits));
    let mut last = BTreeMap::<u32, (u32, u32)>::new();
    let mut w = mass::Part::read(&s, half, &curve).words(at);
    w.extend(mass::Bike::read(&s).words(BIKE, at.body));
    for (a, mask, v) in w {
        let e = last.entry(a).or_insert((0, 0));
        *e = (e.0 | mask, e.1 & !mask | v & mask);
    }
    Some(last.into_iter().map(|(a, (mask, v))| (a, mask, v)).collect())
}

/// **F31b4c — state 4's mass words, the port's**: every word the part reader and the bike's loader
/// write from `mx_race250p.phy`'s `[Mass Properties]` — the body's mass, inertia and centre of
/// gravity, the record's mass, inertia, springs, sweep and frictions, the gravity block, the flag
/// bits, the bike's inertia, centres of gravity and travels — is state 4's, word for word.
#[test]
fn the_mass_words_are_state4s_word_for_word() {
    let Some(phy) = race250p() else { return };
    let Some(words) = mass_words(&phy) else { return };
    let ee = image().unwrap();
    let off: Vec<String> = words
        .iter()
        .filter(|&&(a, mask, v)| ee.u32_at(a) & mask != v & mask)
        .map(|&(a, mask, v)| {
            let was = ee.u32_at(a) & mask;
            format!("{a:#x} mask {mask:#x}: state 4 {was:#010x} ({}), port {:#010x} ({})", f32::from_bits(was), v & mask,
                    f32::from_bits(v & mask))
        })
        .collect();
    assert!(off.is_empty(), "{} of {} mass words differ:\n  {}", off.len(), words.len(), off.join("\n  "));
    assert_eq!(words.len(), 134, "the words the reader and the loader write");
    // The record keeps the box's inertia, the body the file's.
    let (body, record) = (ee.u32_at(BIKE + seed::BODY), ee.u32_at(BIKE + seed::RECORD));
    assert_ne!(ee.u32_at(record + 0x60), ee.u32_at(body + 0x130));
    eprintln!("{} mass words, every one state 4's", words.len());
}

/// [`replay_port_record`] with the port's mass words written over the image first.
fn replay_port_record_massed(name: &str) -> Option<(Vec<bool>, u32)> {
    let words = mass_words(&race250p()?)?;
    let trn = nat05_trn()?;
    let map = track::Terrain::parse(&trn).unwrap();
    let mut ee = hooked(image()?);
    for (a, mask, v) in words {
        ee.set_u32(a, ee.u32_at(a) & !mask | v & mask);
    }
    let rows = rows(name)?;
    let (mut out, mut queries) = (Vec::new(), 0);
    for (r, row) in rows.iter().enumerate() {
        poke(&mut ee, row, &READER);
        let (steps, posts, ported, q) = port_record_frame(&mut ee, F(f32::from_bits(DT)), &map);
        assert_eq!((steps, posts, ported), (4, 1, 2), "the Force body every substep, the PostProcess once, the record's two virtuals");
        queries += q;
        let d = diff(&ee, row);
        assert!(
            d.is_empty(),
            "{name} row {r} (vsync {}), the port's mass words and record pass: {} words differ\n  {}",
            row["vsync"],
            d.len(),
            d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  ")
        );
        let w = wrecked(|o| ee.u32_at(BIKE + o));
        assert_eq!(w, row_wrecked(row), "{name} row {r}: the wreck state");
        out.push(w);
        poke(&mut ee, row, &SPOT);
    }
    Some((out, queries))
}

/// **C15's ride on the port's mass words**: every row of ride-2 word for word.
#[test]
fn ride_replays_bit_exact_on_the_ports_mass_words() {
    let Some((w, _)) = replay_port_record_massed("ride-2") else { return };
    assert_eq!(w.len(), 459);
    assert!(w.iter().all(|w| !w), "the ride never wrecks");
}

/// **C15's flip on the port's mass words**: wrecked on row 125 and put back on row 193, every row
/// word for word.
#[test]
fn flip_wrecks_and_is_put_back_bit_exact_on_the_ports_mass_words() {
    let Some((w, _)) = replay_port_record_massed("flip-1") else { return };
    assert_eq!(w.len(), 465);
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [125, 193], "wrecked on row 125, put back on row 193");
}

// ---------------------------------------------------------------------------------------------
// F31b5a: the head's and Force body's steps quad::Steps', on the bike read by handling::steps.
// ---------------------------------------------------------------------------------------------

use game_sim::handling::steps;
use game_sim::noise::SmoothNoise;

/// The smooth noise's tables (its permutation, its gradients), the flag that says they are not built
/// yet, and the word it leaves its knot in.
const NOISE_PERM: u32 = 0x003f_3540;
const NOISE_G1: u32 = 0x003f_6580;
const NOISE_UNBUILT: u32 = 0x0038_f708;
const NOISE_KNOT: u32 = 0x003f_6d88;
/// What builds the noise's tables, off the C library's `rand()`, whose state is the doubleword
/// `+0xa8` of the block the word at [`RAND_BLOCK`] points to.
const NOISE_BUILD: u32 = 0x0013_4680;
const RAND_BLOCK: u32 = 0x0039_3a90;
/// The word the struck thing's class (its `+0x94`) is tested against.
const STRUCK_MASK: u32 = 0x0054_2db0;
/// The preload's cue (`a0` the word at [`CUE_ON`]).
const PRELOAD_CUE: u32 = 0x0027_2300;
const CUE_ON: u32 = 0x0044_2eb0;

fn rand_state(ee: &Ee) -> u64 {
    ee.read(ee.u32_at(RAND_BLOCK) + 0xa8, 8) as u64
}

/// One sample of the smooth noise as the original takes it: the tables built first if they are not
/// (the original's builder, run where the original runs it), then [`SmoothNoise::at_accumulated`]
/// on them, its knot left where the original leaves it.
fn noise_at(ee: &mut Ee, noise: &mut Option<SmoothNoise>, t: F) -> F {
    if ee.u32_at(NOISE_UNBUILT) != 0 {
        ee.set_u32(NOISE_UNBUILT, 0);
        ee.set(28, GP);
        ee.call(NOISE_BUILD, &[], &[], BUDGET);
        *noise = None;
    }
    let n = noise.get_or_insert_with(|| noise_tables(ee));
    ee.set_u32(NOISE_KNOT, (F(4096.0) + t).to_i32() as u32);
    n.at_accumulated(t)
}

fn noise_tables(ee: &Ee) -> SmoothNoise {
    assert_eq!(ee.u32_at(NOISE_UNBUILT), 0, "the noise's tables are built");
    let perm = std::array::from_fn(|k| ee.u32_at(NOISE_PERM + 4 * k as u32));
    let g1 = std::array::from_fn(|k| F(ee.f32_at(NOISE_G1 + 4 * k as u32)));
    SmoothNoise::from_tables(&perm, &g1)
}

/// The struck thing's class as the steep test reads it: the record touching, and the thing it
/// touches of the class the mask names.
fn struck_class(ee: &Ee) -> bool {
    let record = ee.u32_at(BIKE + seed::RECORD);
    if record == 0 || ee.u32_at(record + 0x34) == 0 {
        return false;
    }
    let thing = ee.u32_at(record + 0x20c);
    thing != 0 && ee.u32_at(thing + 0x94) & ee.u32_at(STRUCK_MASK) != 0
}

fn call_name(c: &Call) -> String {
    let s = format!("{c:?}");
    s.split(['(', ' ']).next().unwrap().to_string()
}

/// The bike in RAM as [`StepInRam`] keeps it, its steps [`quad::Steps`]' on the bike
/// [`steps::read`] reads before each: what the steps hand on (the gravity's record call, the
/// preload's cue, a stunt's start and end, a wreck) the original's, run after the step's words are
/// back, and the contacts the original's too. The noise is [`SmoothNoise::at_accumulated`] on the
/// original's tables. F31b5g: the gravity's record call is answered by the record's resolution
/// ([`record::held`]); with `pure`, the rest are Rust's ([`StepsInRam::in_rust`]).
struct StepsInRam<'a, 'm> {
    inner: StepInRam<'a>,
    map: &'m track::Terrain<'m>,
    noise: &'m mut Option<SmoothNoise>,
    answered: &'m mut BTreeMap<String, u32>,
    pure: bool,
}

impl StepsInRam<'_, '_> {
    fn run(&mut self, f: u32, ints: &[u32], x: F) -> u32 {
        let ee = &mut *self.inner.ram.ee;
        ee.set(28, GP);
        ee.call(f, &ints.iter().map(|&i| u64::from(i)).collect::<Vec<_>>(), &[x.0], BUDGET);
        ee.r(2) as u32
    }

    /// What the steps handed on, the original's; `from` the call it came from (for the stunt that
    /// ends: the backflip's, or the air spin's side's), `k` the bike after it.
    fn hand_on(&mut self, c: Call, from: Call, k: &quad::Bike) {
        let dt = self.inner.dt;
        let ee = &*self.inner.ram.ee;
        match c {
            Call::GravityRecord => {
                let record = ee.u32_at(BIKE + seed::RECORD);
                let f = ee.u32_at(ee.u32_at(record + 4) + 0x88);
                self.run(f, &[record, 1], dt);
            }
            Call::PreloadCue => {
                let ints = [ee.u32_at(CUE_ON), BIKE + 0x674, 8, 14, ee.u32_at(BIKE + 0xd20), 0];
                self.run(PRELOAD_CUE, &ints, dt);
            }
            Call::StartStunt(id) => {
                let f = vt(ee, 0x4b8);
                assert_ne!(self.run(f, &[BIKE, id, 1], dt), 0, "the stunt starts");
            }
            Call::EndStunt => {
                let id = match from {
                    Call::Backflip(_) => k.flip_stunt,
                    Call::SpinAir(_) => {
                        let s = &k.parts.spin;
                        if s.dir > 0 { s.stunt_left } else { s.stunt_right }
                    }
                    c => panic!("{c:?} ends no stunt"),
                };
                let f = vt(ee, 0x4ac);
                self.run(f, &[BIKE, id], dt);
            }
            Call::Wreck(r) => {
                let f = vt(ee, 0x368);
                self.run(f, &[BIKE, u32::from(r)], dt);
            }
            c => panic!("the bike's steps hand on no {c:?}"),
        }
    }

    /// F31b5g — what the steps hand on, with nothing handed to the interpreter: the preload's cue
    /// dropped (a post no listener of the bike's reads: F31b5f's, and
    /// [`replay_pure`]); the stunt's start and end
    /// [`update::stunt_start`]'s and [`update::trick_end`]'s, the player's run's calls [`run`]'s
    /// (F31b5h: [`RunServed`], their event posts dropped as the cue); the wreck [`wreck::enter`]'s. Answers whether it answered `c`.
    fn in_rust(&mut self, c: Call, from: Call, k: &quad::Bike) -> bool {
        let dt = self.inner.dt;
        let ee = &mut *self.inner.ram.ee;
        match c {
            Call::PreloadCue => {}
            Call::StartStunt(id) => {
                assert_eq!(vt(ee, 0x4b8), STUNT_START_FN, "the bike's vt +0x4b8");
                assert_eq!(update::stunt_start(&mut RunServed { ee }, id, 1), 1);
            }
            Call::EndStunt => {
                let id = match from {
                    Call::Backflip(_) => k.flip_stunt,
                    Call::SpinAir(_) => {
                        let s = &k.parts.spin;
                        if s.dir > 0 { s.stunt_left } else { s.stunt_right }
                    }
                    c => panic!("{c:?} ends no stunt"),
                };
                assert_eq!(vt(ee, 0x4ac), TRICK_END_FN, "the bike's vt +0x4ac");
                update::trick_end(&mut RunServed { ee }, id);
            }
            Call::Wreck(r) => {
                assert_eq!(vt(ee, 0x368), WRECK_ENTER, "the bike's vt +0x368");
                wreck::enter(&mut WreckInRam { ee, x: dt.0 }, u32::from(r));
            }
            _ => return false,
        }
        true
    }
}

/// The bike's stunt start and trick end (vt `+0x4b8`, `+0x4ac`), and the player's run's calls they
/// make (F31b5g).
const STUNT_START_FN: u32 = 0x004e_6370;
const TRICK_END_FN: u32 = 0x004e_4db0;
const RUN_OPEN: u32 = 0x0046_a5b0;
const RUN_SCORE: u32 = 0x0046_aae0;
const RUN_CLOSE: u32 = 0x0046_b4c0;
const RUN_TALLY: u32 = 0x0046_a760;
const RUN_END: u32 = 0x0046_acf0;

/// [`update::stunt_start`] and [`update::trick_end`] on the bike in RAM, the start's trick ends
/// [`update::trick_end`]'s, the player's run's calls [`run`]'s ([`serve_run`], F31b5h).
struct RunServed<'a> {
    ee: &'a mut Ee,
}

impl Updates for RunServed<'_> {
    fn bike(&self) -> u32 {
        BIKE
    }
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn set(&mut self, at: u32, w: u32) {
        self.ee.set_u32(at, w);
    }
    fn out(&mut self, c: update::Out) -> u32 {
        use update::Out;
        match c {
            Out::TrickEnd(id) => update::trick_end(self, id),
            c => serve_run(&mut RunInRam { ee: &mut *self.ee, posts: 0 }, c),
        }
    }
    fn vector(&mut self, c: update::Out) -> update::Quad {
        panic!("the stunt's start and end ask no vector of {c:?}")
    }
    fn noise(&mut self, _: F) -> F {
        panic!("the stunt's start and end ask no noise")
    }
}

/// F45 — the run's words in RAM, its listeners [`told`], its event posts handed to the original
/// (the post on the queue word at [`CUE_ON`]), for the stunt's start and end [`UpdateInRam`] serves.
struct RunHanded<'a> {
    ee: &'a mut Ee,
}

impl run::Runs for RunHanded<'_> {
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn set(&mut self, at: u32, w: u32) {
        self.ee.set_u32(at, w);
    }
    fn out(&mut self, c: run::Out) {
        match c {
            run::Out::Tell { .. } => told(self, c),
            run::Out::Post { on, kind, id, data, x } => {
                let q = self.ee.u32_at(CUE_ON);
                self.ee.set(28, GP);
                census(self.ee, |ee| ee.call(POST_FN, &[q, on, kind, id, data, x].map(u64::from), &[], BUDGET));
            }
        }
    }
}

/// F31b5h — one of the run's calls the stunt's start and end hand out, made by [`run`]'s port with
/// the original's arguments (the open's player the bike's, as the start leaves it in `a1`).
fn serve_run(m: &mut impl run::Runs, c: update::Out) -> u32 {
    use update::Out;
    let one = F::ONE;
    match c {
        Out::RunOpen(r) => {
            let player = m.word(BIKE + 0x680);
            run::open(m, r, player)
        }
        Out::RunScore { run: r, player, id, scores: [a, b] } => run::score(m, r, player, id, 12, F(f32::from_bits(a)), F::ZERO, F(f32::from_bits(b)), one, one),
        Out::Mark { run: r, player, on, id } => run::mark(m, r, player, on, id, 0),
        Out::RunClose { run: r, player } => run::close(m, one, r, player, 1),
        Out::RunTally { run: r, player, count } => run::tally(m, F(f32::from_bits(count)), r, player, 1),
        Out::RunEnd { run: r, player, id } => run::end(m, one, r, player, 1, id, 0),
        c => panic!("the stunt's start and end call no {c:?}"),
    }
}

/// F31b5h — the run's listeners state 4 has: the group's open and end (answering 0, nothing else),
/// its tally and score, and the bike's watcher's four, the last six behind thunks (`j` with `a0`
/// moved back to the listener's owner).
const GROUP_OPEN_FN: u32 = 0x0046_a6f0;
const GROUP_END_FN: u32 = 0x0046_afe0;
const GROUP_TALLY_FN: u32 = 0x0051_9210;
const GROUP_SCORE_FN: u32 = 0x0051_9ab0;
const WATCH_OPEN_FN: u32 = 0x004e_7240;
const WATCH_SCORE_FN: u32 = 0x004e_72b0;
const WATCH_END_FN: u32 = 0x004e_7340;
const WATCH_TALLY_FN: u32 = 0x004e_73f0;
/// The event post (the preload's cue's).
const POST_FN: u32 = PRELOAD_CUE;

/// A listener thunk at `f` told `to`: its target and the owner it hands on (`to` plus its `addiu`).
fn thunk(m: &mut impl run::Runs, f: u32, to: u32) -> (u32, u32) {
    let (j, a) = (m.word(f), m.word(f.wrapping_add(4)));
    assert!(j >> 26 == 2 && a >> 16 == 0x2484, "{f:#x} is no listener thunk: {j:#010x} {a:#010x}");
    (f & 0xf000_0000 | (j & 0x03ff_ffff) << 2, to.wrapping_add(a as u16 as i16 as i32 as u32))
}

/// A listener told, by its vtable's function: the ports of the ones state 4 has.
fn told(m: &mut impl run::Runs, c: run::Out) {
    use run::Heard;
    let run::Out::Tell { to, vt, player, heard } = c else { panic!("{c:?} tells no listener") };
    let table = m.word(to.wrapping_add(vt));
    let f = m.word(table.wrapping_add(heard.slot(vt)));
    if matches!(f, GROUP_OPEN_FN | GROUP_END_FN) {
        return;
    }
    let (target, owner) = thunk(m, f, to);
    match (target, heard) {
        (GROUP_SCORE_FN, Heard::Score(n)) => run::group_score(m, owner, player, n),
        (GROUP_TALLY_FN, Heard::Tally { flag, .. }) => run::group_tally(m, owner, player, flag),
        (WATCH_OPEN_FN, Heard::Open) => run::watch_open(m, owner, player),
        (WATCH_SCORE_FN, Heard::Score(n)) => run::watch_score(m, owner, player, n),
        (WATCH_END_FN, Heard::End { flag, node, .. }) => run::watch_end(m, owner, player, flag, node),
        (WATCH_TALLY_FN, Heard::Tally { x, flag }) => run::watch_tally(m, owner, F(f32::from_bits(x)), player, flag),
        _ => panic!("{to:#x} told {heard:?} through {f:#x} -> {target:#x}: no port"),
    };
}

/// The run's words in RAM, its listeners [`told`], its event posts dropped and counted (as the
/// preload's cue: no listener of theirs on state 4 writes the bike).
struct RunInRam<'a> {
    ee: &'a mut Ee,
    posts: u32,
}

impl run::Runs for RunInRam<'_> {
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn set(&mut self, at: u32, w: u32) {
        self.ee.set_u32(at, w);
    }
    fn out(&mut self, c: run::Out) {
        match c {
            run::Out::Tell { .. } => told(self, c),
            run::Out::Post { .. } => self.posts += 1,
        }
    }
}

impl Moto for StepsInRam<'_, '_> {
    fn call(&mut self, c: Call, q: &mut Quad, body: &mut quad::Body) {
        if matches!(c, Call::Contacts) {
            return self.inner.call(c, q, body);
        }
        *self.answered.entry(call_name(&c)).or_default() += 1;
        self.inner.write(q, body);
        let ee = &*self.inner.ram.ee;
        let m = |a| ee.u32_at(a);
        let (mut k, at) = steps::read(&m, BIKE).unwrap_or_else(|e| panic!("{c:?}: the bike's words: {e}"));
        k.struck_class = struck_class(ee);
        let last = steps::words(&k, &at, &m);
        let terrain = {
            let t = at.wheels.map(|w| ee.u32_at(w + 0x1e0));
            assert!(t[0] == t[1] || t[0] == 0 || t[1] == 0, "the wheels stand on one terrain: {t:#x?}");
            let t = if t[0] != 0 { t[0] } else { t[1] };
            (t != 0).then(|| ground::read_terrain(&m, t, Some(self.map)))
        };
        // F31b5g: the gravity's record call answered by the record's resolution.
        let record_at = ee.u32_at(BIKE + seed::RECORD);
        if matches!(c, Call::Gravity) {
            k.contacts = record::held(&m, record_at);
        }
        let mut s = quad::Steps {
            bike: k,
            rest: Vec::<Call>::new(),
            ground: |p: Vf, rec: &mut u8| terrain.as_ref().expect("a wheel with ground has a terrain").ground(p, Vf::ZERO, rec),
            sweep: (),
        };
        s.call(c, q, body);
        let quad::Steps { bike: k, rest, .. } = s;
        let ee = &*self.inner.ram.ee;
        let now = steps::words(&k, &at, &|a| ee.u32_at(a));
        let mut set = Vec::new();
        for (&(a, w), &(la, lw)) in now.iter().zip(&last) {
            assert_eq!(a, la);
            if w != lw {
                set.push((a, w));
            }
        }
        // The record's words and the whole body as the resolution left them (as read, when it did not
        // run), then the listener's view of the body over them.
        if let Some(h) = &k.contacts {
            set.extend(record::held_words(h, record_at, &|a| ee.u32_at(a)));
        }
        for (a, w) in set {
            self.inner.ram.ee.set_u32(a, w);
        }
        self.inner.write(q, body);
        for r in rest {
            if self.pure && self.in_rust(r, c, &k) {
                *self.answered.entry(format!("= {}", call_name(&r))).or_default() += 1;
                continue;
            }
            *self.answered.entry(format!("> {}", call_name(&r))).or_default() += 1;
            self.hand_on(r, c, &k);
        }
        (*q, *body) = self.inner.read();
    }
}

impl bike::Rides for StepsInRam<'_, '_> {
    fn rider(&mut self) -> &mut bike::Rider {
        &mut self.inner.rider
    }
    fn noise(&mut self, t: F) -> F {
        noise_at(self.inner.ram.ee, self.noise, t)
    }
}

/// [`port_head`] with its steps [`StepsInRam`]'s.
fn steps_head(ee: &mut Ee, dt: F, map: &track::Terrain<'_>, noise: &mut Option<SmoothNoise>, answered: &mut BTreeMap<String, u32>, pure: bool) {
    let body = ee.u32_at(BIKE + seed::BODY);
    let (wheels, record) = (the_wheels(ee), ee.u32_at(BIKE + seed::RECORD));
    let words = |ee: &Ee| (bike::read_params(&|a| ee.u32_at(a), BIKE), bike::read_held(&|a| ee.u32_at(a), BIKE));
    let (p, held) = words(ee);
    let mut axes = seed::body(&|a| ee.u32_at(a), body).angular_axes;
    let was = axes;
    let ram = InRam { ee, wheels, record, body, last: Vec::new() };
    let inner = StepInRam { ram, rider: bike::Rider::default(), rider_last: Vec::new(), dt };
    let mut m = StepsInRam { inner, map, noise, answered, pure };
    let (mut q, mut b) = m.inner.read();
    bike::ground_head(&p, &mut q, &mut b, held, &mut axes, dt, &mut m);
    m.inner.write(&q, &b);
    for k in 0..3 {
        if axes[k] != was[k] {
            m.inner.ram.ee.set_u32(body + 0x120 + 4 * k as u32, u32::from(axes[k]));
        }
    }
}

/// [`port_step`] with its steps [`StepsInRam`]'s.
fn steps_step(ee: &mut Ee, dt: F, map: &track::Terrain<'_>, noise: &mut Option<SmoothNoise>, answered: &mut BTreeMap<String, u32>, pure: bool) {
    let body = ee.u32_at(BIKE + seed::BODY);
    let (wheels, record) = (the_wheels(ee), ee.u32_at(BIKE + seed::RECORD));
    let p = bike::read_params(&|a| ee.u32_at(a), BIKE);
    let ram = InRam { ee, wheels, record, body, last: Vec::new() };
    let inner = StepInRam { ram, rider: bike::Rider::default(), rider_last: Vec::new(), dt };
    let mut m = StepsInRam { inner, map, noise, answered, pure };
    let (mut q, mut b) = m.inner.read();
    bike::ground_step(&p, &mut q, &mut b, dt, &mut m);
    m.inner.write(&q, &b);
}

/// A frame as [`PortPost`]'s, the bike's PreProcess [`steps_head`] and its Force body
/// [`steps_step`].
struct PortSteps<'a, 'm> {
    post: PortPost<'a>,
    map: &'m track::Terrain<'m>,
    noise: &'m mut Option<SmoothNoise>,
    answered: BTreeMap<String, u32>,
    heads: u32,
    steps: u32,
    /// F31b5g: the steps hand nothing on ([`StepsInRam::in_rust`]).
    pure: bool,
}

impl Dynamics for PortSteps<'_, '_> {
    fn count(&self, l: List) -> i32 {
        self.post.count(l)
    }
    fn preforce(&mut self, i: i32, dt: F) {
        if self.post.0.0.0.elt(List::Preforce, i) != BIKE_PREFORCE {
            if dropped_others(self.pure) {
                *self.answered.entry("dropped PreProcess".into()).or_default() += 1;
                return;
            }
            return self.post.preforce(i, dt);
        }
        let (map, noise) = (self.map, &mut *self.noise);
        let mut answered = std::mem::take(&mut self.answered);
        let h = &mut self.post.0.0.0;
        for (a, w) in seed::body_words(&h.body, h.at) {
            h.ee.set_u32(a, w);
        }
        steps_head(h.ee, dt, map, noise, &mut answered, self.pure);
        h.body = seed::body(&|a| h.ee.u32_at(a), h.at);
        self.answered = answered;
        self.heads += 1;
    }
    fn force(&mut self, i: i32, h: F) {
        let x = &mut self.post.0.0.0;
        let e = x.elt(List::Force, i);
        if x.ee.u32_at(e + 0xc) != BIKE_FORCE {
            return self.post.force(i, h);
        }
        let (map, noise) = (self.map, &mut *self.noise);
        let mut answered = std::mem::take(&mut self.answered);
        for (a, w) in seed::body_words(&x.body, x.at) {
            x.ee.set_u32(a, w);
        }
        steps_step(x.ee, h, map, noise, &mut answered, self.pure);
        x.body = seed::body(&|a| x.ee.u32_at(a), x.at);
        self.answered = answered;
        self.steps += 1;
    }
    fn enabled(&self, b: i32) -> bool {
        self.post.enabled(b)
    }
    fn integrate(&mut self, b: i32, h: F) {
        self.post.integrate(b, h)
    }
    fn end_substep(&mut self, b: i32) {
        self.post.end_substep(b)
    }
    fn constrain(&mut self, b: i32, last: bool) {
        self.post.constrain(b, last)
    }
    fn post(&mut self, i: i32, dt: F) {
        self.post.post(i, dt)
    }
}

/// [`port_record_frame`] with the world step [`PortSteps`]'. Answers the heads and Force bodies the
/// steps ran, the PostProcesses, and the calls they answered (`> ` those they handed on).
fn steps_frame(ee: &mut Ee, dt: F, map: &track::Terrain<'_>, noise: &mut Option<SmoothNoise>) -> (u32, u32, u32, BTreeMap<String, u32>) {
    let call = |ee: &mut Ee, f: u32, ints: &[u64], x: F| {
        ee.set(28, GP);
        ee.call(f, ints, &[x.0], BUDGET);
        ee.r(2)
    };
    assert_eq!(ee.u8_at(ee.u32_at(SKIP_FLAG) + 0x225) & 8, 0, "the frame is not skipped");
    let mine = ee.u32_at(BIKE + seed::RECORD);
    for (slot, original) in [(0x7c, RECORD_CLEAR), (0x84, RECORD_PASS)] {
        for i in 0..ee.u32_at(SIM + 0x24) {
            let e = call(ee, LIST24_GET, &[u64::from(SIM + 0x24), u64::from(i)], F::ZERO) as u32;
            let f = ee.u32_at(ee.u32_at(e + 4) + slot);
            if e == mine {
                assert_eq!(f, original, "the bike's record's vt +{slot:#x}");
                let t = ee.u32_at(e + 0xec);
                let terrain = (t != 0).then(|| ground::read_terrain(&|a| ee.u32_at(a), t, Some(map)));
                port_record(ee, e, slot, dt, terrain.as_ref());
            } else {
                call(ee, f, &[u64::from(e)], dt);
            }
        }
    }
    let world = World { iterations: ee.u32_at(WORLD) as i32, rate: F(ee.f32_at(WORLD + 4)) };
    let at = ee.u32_at(BIKE + seed::BODY);
    let body = seed::body(&|a| ee.u32_at(a), at);
    let post = PortPost(PortStep(PortHead(Hybrid { ee, body, at, integrated: 0 }), 0), 0);
    let mut h = PortSteps { post, map, noise, answered: BTreeMap::new(), heads: 0, steps: 0, pure: false };
    world.step(dt, &mut h);
    let (body, heads, steps, posts, answered) = (h.post.0.0.0.body, h.heads, h.steps, h.post.1, h.answered);
    for (a, w) in seed::body_words(&body, at) {
        ee.set_u32(a, w);
    }
    let (bikes, others) = port_tail(ee, dt);
    assert_eq!((bikes, others.len()), (1, 3), "the tail steps the bike once, its two other leaves and the second's rider: {others:#x?}");
    let mut i = 0;
    while i < ee.u32_at(SIM + 0x3c) {
        let e = call(ee, LIST3C_GET, &[u64::from(SIM + 0x3c), u64::from(i)], F::ZERO) as u32;
        if call(ee, ee.u32_at(ee.u32_at(e) + 8), &[u64::from(e)], dt) != 0 {
            call(ee, LIST3C_REMOVE, &[u64::from(SIM + 0x3c), u64::from(i)], F::ZERO);
        } else {
            i += 1;
        }
    }
    (heads, steps, posts, answered)
}

/// [`replay_port_record_massed`] with every frame [`steps_frame`]. Answers the wreck state per row
/// and what the steps answered over the replay.
fn replay_steps(name: &str) -> Option<(Vec<bool>, BTreeMap<String, u32>)> {
    let words = mass_words(&race250p()?)?;
    let trn = nat05_trn()?;
    let map = track::Terrain::parse(&trn).unwrap();
    let mut ee = hooked(image()?);
    for (a, mask, v) in words {
        ee.set_u32(a, ee.u32_at(a) & !mask | v & mask);
    }
    let mut noise = None;
    let rows = rows(name)?;
    let (mut out, mut all) = (Vec::new(), BTreeMap::<String, u32>::new());
    for (r, row) in rows.iter().enumerate() {
        poke(&mut ee, row, &READER);
        let (heads, steps, posts, answered) = steps_frame(&mut ee, F(f32::from_bits(DT)), &map, &mut noise);
        assert_eq!((heads, steps, posts), (1, 4, 1), "the head once, the Force body every substep, the PostProcess once");
        for (k, n) in answered {
            *all.entry(k).or_default() += n;
        }
        let d = diff(&ee, row);
        assert!(
            d.is_empty(),
            "{name} row {r} (vsync {}), the steps quad::Steps': {} words differ\n  {}",
            row["vsync"],
            d.len(),
            d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  ")
        );
        let w = wrecked(|o| ee.u32_at(BIKE + o));
        assert_eq!(w, row_wrecked(row), "{name} row {r}: the wreck state");
        out.push(w);
        poke(&mut ee, row, &SPOT);
    }
    eprintln!("{name}: the steps answered {all:?}");
    Some((out, all))
}

/// **F31b5a — state 4's bike, read and written back**: [`steps::read`]'s bike, put back by
/// [`steps::words`], is state 4's at every word it names, and a changed field moves its word only.
#[test]
fn state4s_bike_reads_back_word_for_word() {
    let Some(ee) = image() else { return };
    let m = |a| ee.u32_at(a);
    let (k, at) = steps::read(&m, BIKE).unwrap();
    let w = steps::words(&k, &at, &m);
    let off: Vec<String> =
        w.iter().filter(|&&(a, v)| ee.u32_at(a) != v).map(|&(a, v)| format!("{a:#x}: state 4 {:#010x}, words {v:#010x}", ee.u32_at(a))).collect();
    assert!(off.is_empty(), "{} of {} words differ:\n  {}", off.len(), w.len(), off.join("\n  "));
    let mut seen = std::collections::BTreeSet::new();
    let twice: Vec<u32> = w.iter().map(|&(a, _)| a).filter(|&a| !seen.insert(a)).collect();
    eprintln!("{} words, {} addresses, named twice: {twice:#x?}", w.len(), seen.len());
    // Read back from the words, the bike is the same bike.
    let mut img: HashMap<u32, u32> = HashMap::new();
    let mut k2 = k.clone();
    k2.engine.rpm = k2.engine.rpm + F::ONE;
    for (a, v) in steps::words(&k2, &at, &m) {
        img.insert(a, v);
    }
    let m2 = |a| img.get(&a).copied().unwrap_or_else(|| ee.u32_at(a));
    let moved: Vec<u32> = steps::words(&k2, &at, &m2).iter().filter(|&&(a, v)| ee.u32_at(a) != v).map(|&(a, _)| a).collect();
    assert_eq!(moved.len(), 1, "one field moves one word: {moved:#x?}");
}

/// **F31b5a — the original's noise**: state 4's tables are not built yet; [`SmoothNoise::build`] off
/// state 4's `rand()` builds the original's, and leaves the generator where the original leaves it;
/// [`SmoothNoise::at_accumulated`] on them is the original's sample, bit for bit, over the knots and
/// between them.
#[test]
fn the_accumulated_noise_is_the_originals() {
    let Some(mut ee) = image() else { return };
    assert_ne!(ee.u32_at(NOISE_UNBUILT), 0, "state 4's noise is not built yet");
    let mut rng = game_sim::rng::Rng::from_state(rand_state(&ee));
    let built = SmoothNoise::build(&mut rng);
    let mut noise = None;
    noise_at(&mut ee, &mut noise, F::ZERO);
    let noise = noise.unwrap();
    assert!(built == noise, "the port's tables are the original's");
    assert_eq!(rng.state(), rand_state(&ee), "the build takes the original's draws");
    let mut off = Vec::new();
    for k in 0..4000 {
        let t = F(-300.0 + k as f32 * 0.173);
        ee.set(28, GP);
        ee.call(SMOOTH_NOISE, &[], &[t.0], BUDGET);
        let (want, got) = (ee.f(0).to_bits(), noise.at_accumulated(t).to_bits());
        if want != got || ee.u32_at(NOISE_KNOT) != (F(4096.0) + t).to_i32() as u32 {
            off.push(format!("t {}: original {want:#010x}, port {got:#010x}", t.0));
        }
    }
    assert!(off.is_empty(), "{} of 4000 differ:\n  {}", off.len(), off.iter().take(20).cloned().collect::<Vec<_>>().join("\n  "));
}

/// **F31b5a — C15's ride on the steps quad::Steps'**: every row of ride-2 word for word, the head's
/// and the Force body's steps answered by [`quad::Steps`].
#[test]
fn ride_replays_bit_exact_on_quad_steps() {
    let Some((w, all)) = replay_steps("ride-2") else { return };
    assert_eq!(w.len(), 459);
    assert!(w.iter().all(|w| !w), "the ride never wrecks");
    let handed: Vec<(&str, u32)> = all.iter().filter(|(k, _)| k.starts_with("> ")).map(|(k, &n)| (&k[2..], n)).collect();
    // F31b5g: the gravity's record call is the record's resolution's ([`record::held`]).
    assert_eq!(handed, [], "nothing handed on");
    for c in ["InputLimiter", "Clutch", "AutoShift", "HeadTail", "Gravity", "EngineUpdate", "Suspension", "TireForces", "WheelIntegrate"] {
        assert!(all.get(c).copied().unwrap_or(0) > 0, "the steps answered no {c}: {all:?}");
    }
}

/// **F45 — C15's rides with the trick step's calls in Rust**: ride-2 and flip-1 in lockstep, the
/// vehicle update's whole RAM the original's after every frame ([`port_tail`]), with the remote stunt
/// clocks, trick wanted, trick end, stunt start, stunt mark, the run's calls and the node turn
/// [`UpdateInRam`]'s ports wherever the update reaches them. Answers what was served, by name.
fn replay_trick_calls(name: &str) -> Option<BTreeMap<&'static str, u32>> {
    F45_SERVED.with(|s| s.borrow_mut().clear());
    replay_steps(name)?;
    let served = F45_SERVED.with(|s| std::mem::take(&mut *s.borrow_mut()));
    eprintln!("{name}: F45 served {served:?}");
    Some(served)
}

#[test]
fn ride_replays_with_the_trick_calls_in_rust() {
    let Some(served) = replay_trick_calls("ride-2") else { return };
    // The trick wanted every frame (the step asks it, the bike on the ground, so it answers 0).
    assert_eq!(served, BTreeMap::from([("TrickWanted", 459)]));
}

#[test]
fn flip_replays_with_the_trick_calls_in_rust() {
    let Some(served) = replay_trick_calls("flip-1") else { return };
    // The backflip's stunt marked through its clip and ended once; a local bike reaches no stunt
    // clocks and no node turn, and the step starts no trick of its own.
    assert_eq!(served, BTreeMap::from([("Mark", 21), ("TrickEnd", 1), ("TrickWanted", 367)]));
}

/// **F31b5a — C15's flip on the steps quad::Steps'**: wrecked on row 125 and put back on row 193,
/// every row word for word, the backflip's stunt handed on.
#[test]
fn flip_replays_bit_exact_on_quad_steps() {
    let Some((w, all)) = replay_steps("flip-1") else { return };
    assert_eq!(w.len(), 465);
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [125, 193], "wrecked on row 125, put back on row 193");
    assert!(all.get("Backflip").copied().unwrap_or(0) > 0, "{all:?}");
    // Handed on: the preload's cue and the backflip's stunt once each; no stunt ends and no wreck
    // the steps ask for (the wreck on row 125 is the wreck test's).
    let handed: Vec<(&str, u32)> = all.iter().filter(|(k, _)| k.starts_with("> ")).map(|(k, &n)| (&k[2..], n)).collect();
    assert_eq!(handed, [("PreloadCue", 1), ("StartStunt", 1)]);
}

// ---------------------------------------------------------------------------------------------
// F31b5b: the PostProcess's and the collision record's callees Rust's, on the bike read by
// handling::steps.
// ---------------------------------------------------------------------------------------------

/// The bike's collidables' classes: a ray probe's vtable and a mesh's.
const RAYS_VTABLE: u32 = 0x003b_6c20;
const MESH_VTABLE: u32 = 0x003b_6d00;
const KINDS: steps::Kinds = steps::Kinds { rays: RAYS_VTABLE, mesh: MESH_VTABLE };
/// What the collision sync hands on: a collidable's box into the spatial database (its entry
/// `+0x18`, the centre and half extent, the reach in `f12`), and the gather of the nearby list
/// (`+0xc4`) round the active probe (kind 3).
const SPATIAL_MOVE: u32 = 0x0018_d690;
const GATHER: u32 = 0x001d_6a60;

/// The words `now` names that differ from `last` into RAM, each word as the last of its writes has
/// it (`now` may name words `last` does not: the node's children, marked).
fn set_changed(ee: &mut Ee, now: &[(u32, u32)], last: &[(u32, u32)]) {
    let last: HashMap<u32, u32> = last.iter().copied().collect();
    let now: BTreeMap<u32, u32> = now.iter().copied().collect();
    for (a, w) in now {
        if last.get(&a) != Some(&w) {
            ee.set_u32(a, w);
        }
    }
}

/// The node's right and up rows' heights, which the bike keeps as the backflip's where the node
/// keeps them, off its world frame (as [`quad::Steps`] keeps them).
fn rows_heights(k: &mut quad::Bike) {
    let w = k.parts.scene.nodes[0].world;
    (k.right_y, k.up_y) = (F(w[1].y()), F(w[2].y()));
}

/// `f` on the bike's scene as [`steps::read`] reads it out of RAM, the scene's words put back where
/// they changed.
fn on_scene<R>(ee: &mut Ee, f: impl FnOnce(&mut game_sim::atv_moto::Scene) -> R) -> R {
    let m = |a| ee.u32_at(a);
    let (mut k, at) = steps::read(&m, BIKE).unwrap_or_else(|e| panic!("the bike's words: {e}"));
    let last = steps::words(&k, &at, &m);
    let r = f(&mut k.parts.scene);
    rows_heights(&mut k);
    let now = steps::words(&k, &at, &|a| ee.u32_at(a));
    set_changed(ee, &now, &last);
    r
}

/// The bike in RAM as [`PostInRam`] keeps it, the PostProcess's callees Rust's: the spatial update,
/// the node's bounds, the collision sync and the node's position [`quad::Steps`]' on the bike
/// [`steps::read_sync`] reads, the node's placement [`game_sim::atv_moto::Scene::place`]. What the
/// sync hands on (a collidable's database move, the gather) is the original's. F47: the body reset
/// and the respawn [`reset`]'s, the out-of-world message [`reset::post`] (its listeners the
/// original's), the put-back [`wreck::put_back`], and the spatial update and the node's bounds of a
/// bike or node in the database whose node gathers other nodes [`node::owner_box`] and
/// [`node::bounds`] on RAM (the database move the original's).
struct PostsInRam<'a, 'm> {
    inner: PostInRam<'a>,
    answered: &'m mut BTreeMap<String, u32>,
}

impl PostsInRam<'_, '_> {
    fn count(&mut self, what: String) {
        *self.answered.entry(what).or_default() += 1;
    }

    /// `c` the steps', on the bike read out of RAM and its words put back where they changed; what
    /// they hand on the original's.
    fn steps(&mut self, c: Call, q: &mut Quad, body: &mut quad::Body) {
        self.count(call_name(&c));
        self.inner.write(q, body);
        let ee = &*self.inner.step.ram.ee;
        let m = |a| ee.u32_at(a);
        let (k, at, parts) = steps::read_sync(&m, BIKE, KINDS).unwrap_or_else(|e| panic!("{c:?}: the bike's words: {e}"));
        let last = steps::post_words(&k, &at, &parts, &m);
        let mut s = quad::Steps {
            bike: k,
            rest: Vec::<Call>::new(),
            ground: |_: Vf, _: &mut u8| -> (Vf, Vf) { panic!("the bike's PostProcess asks for no ground") },
            sweep: (),
        };
        s.call(c, q, body);
        let quad::Steps { bike: k, rest, .. } = s;
        let ee = &mut *self.inner.step.ram.ee;
        let now = steps::post_words(&k, &at, &parts, &|a| ee.u32_at(a));
        set_changed(ee, &now, &last);
        self.inner.write(q, body);
        let dt = self.inner.step.dt;
        for r in rest {
            let name = call_name(&r);
            let rust = in_rust(&name);
            self.count(if rust { name } else { format!("> {name}") });
            let ee = &mut *self.inner.step.ram.ee;
            ee.set(28, GP);
            match r {
                Call::PartMove { part, centre, half, reach } if rust => {
                    let entry = parts[game_sim::atv_sync::slot(part)] + 0x18;
                    spatial_move_in_ram(ee, entry, centre, half, reach);
                }
                Call::PartMove { part, centre, half, reach } => {
                    let (c, h) = (ee.scratch(0), ee.scratch(1));
                    set_vf(ee, c, centre);
                    set_vf(ee, h, half);
                    let entry = parts[game_sim::atv_sync::slot(part)] + 0x18;
                    ee.call(SPATIAL_MOVE, &[u64::from(entry), u64::from(c), u64::from(h)], &[reach.0], BUDGET);
                }
                Call::Gather(p) if rust => {
                    gather_in_ram(ee, ee.u32_at(BIKE + 0xc4), parts[game_sim::atv_sync::slot(p)], 3);
                }
                Call::Gather(p) => {
                    let ints = [ee.u32_at(BIKE + 0xc4), parts[game_sim::atv_sync::slot(p)], 3].map(u64::from);
                    ee.call(GATHER, &ints, &[dt.0], BUDGET);
                }
                r => panic!("the PostProcess's steps hand on no {r:?}"),
            }
        }
        (*q, *body) = self.inner.read();
    }
}

/// F47: whether the bike or its node is in the database with a node that gathers other nodes'
/// bounds (the scene [`steps::read_sync`] reads holds the node alone).
fn gathers_in_database(ee: &Ee) -> bool {
    let node = ee.u32_at(BIKE + 0xe4);
    ee.u32_at(node + 0x10c) != 0 && (ee.u32_at(BIKE + 8) != 0 || ee.u32_at(node + 8) != 0)
}

/// F47: with [`gathers_in_database`], the spatial update (the bike's entry moved to its node's box,
/// reach 0) or the node's bounds (its entry moved to them) on RAM, the move the original's; whether
/// `c` was one of them, so served.
fn node_box_on_ram(ee: &mut Ee, c: Call) -> bool {
    if !matches!(c, Call::SpatialUpdate | Call::NodeBounds) || !gathers_in_database(ee) {
        return false;
    }
    let node = ee.u32_at(BIKE + 0xe4);
    let m = &mut NodeRam(ee);
    let moved = match c {
        Call::SpatialUpdate => node::owner_box(m, BIKE, node).map(|(c, h)| (BIKE, c, h, F::ZERO)),
        _ => node::bounds(m, node).map(|(c, h, r)| (node, c, h, r)),
    };
    if let Some((entry, centre, half, reach)) = moved {
        let (c, h) = (ee.scratch(0), ee.scratch(1));
        set_vf(ee, c, centre);
        set_vf(ee, h, half);
        ee.set(28, GP);
        ee.call(SPATIAL_MOVE, &[u64::from(entry), u64::from(c), u64::from(h)], &[reach.0], BUDGET);
    }
    true
}

impl PostsInRam<'_, '_> {
    fn node_box(&mut self, c: Call, q: &mut Quad, body: &mut quad::Body) {
        self.count(format!("{} gathered", call_name(&c)));
        self.inner.write(q, body);
        assert!(node_box_on_ram(self.inner.step.ram.ee, c), "{c:?} gathered");
        (*q, *body) = self.inner.read();
    }
}

impl Moto for PostsInRam<'_, '_> {
    fn call(&mut self, c: Call, q: &mut Quad, body: &mut quad::Body) {
        match c {
            Call::SpatialUpdate | Call::NodeBounds if gathers_in_database(self.inner.step.ram.ee) => self.node_box(c, q, body),
            Call::SpatialUpdate | Call::NodeBounds | Call::CollisionSync | Call::NodePosition => self.steps(c, q, body),
            Call::BodyReset | Call::Respawn => {
                self.count(call_name(&c));
                self.inner.write(q, body);
                let m = &mut ResetInRam(self.inner.step.ram.ee);
                if c == Call::BodyReset {
                    reset::body_reset(m, BIKE);
                } else {
                    assert_eq!(vt(m.0, RESPAWN.0), RESPAWN.1, "the bike's vt +0xe8");
                    reset::respawn(m, BIKE);
                }
                (*q, *body) = self.inner.read();
            }
            c => {
                self.count(format!("> {}", call_name(&c)));
                self.inner.call(c, q, body)
            }
        }
    }
}

impl bike::Rides for PostsInRam<'_, '_> {
    fn rider(&mut self) -> &mut bike::Rider {
        self.inner.rider()
    }
    fn noise(&mut self, t: F) -> F {
        self.inner.noise(t)
    }
}

impl bike::Posts for PostsInRam<'_, '_> {
    fn post(&mut self) -> &mut bike::Post {
        self.inner.post()
    }
    fn out(&mut self, c: bike::Out, q: &mut Quad, body: &mut quad::Body) {
        match c {
            bike::Out::Place { orientation, at } => {
                self.count("Place".into());
                assert_eq!(orientation.to_bits(), body.orientation.to_bits(), "the node is placed with the body's quaternion");
                self.inner.write(q, body);
                on_scene(self.inner.step.ram.ee, |s| s.place(0, orientation, at));
                (*q, *body) = self.inner.read();
            }
            bike::Out::OutOfWorld(id) => {
                self.count("OutOfWorld".into());
                self.inner.write(q, body);
                let ee = &mut *self.inner.step.ram.ee;
                let (kind, msg) = bike::OUT_OF_WORLD;
                let queue = ee.u32_at(POST_QUEUE);
                reset::post(&mut ResetInRam(ee), queue, BIKE + 0x674, [kind, msg, 0, id]);
                (*q, *body) = self.inner.read();
            }
            bike::Out::PutBack { place, keep } => {
                self.count("PutBack".into());
                self.inner.write(q, body);
                let x = self.inner.step.dt.0;
                let m = &mut WreckInRam { ee: self.inner.step.ram.ee, x };
                assert_eq!(vt(m.ee, PUT_BACK_SLOT), PUT_BACK, "the bike's vt +0x3a8");
                wreck::put_back(m, u32::from(place), u32::from(keep));
                (*q, *body) = self.inner.read();
            }
        }
    }
}

/// [`port_post`] with its callees [`PostsInRam`]'s.
fn posts_post(ee: &mut Ee, dt: F, answered: &mut BTreeMap<String, u32>) {
    let body = ee.u32_at(BIKE + seed::BODY);
    let (wheels, record) = (the_wheels(ee), ee.u32_at(BIKE + seed::RECORD));
    let ram = InRam { ee, wheels, record, body, last: Vec::new() };
    let step = StepInRam { ram, rider: bike::Rider::default(), rider_last: Vec::new(), dt };
    let inner = PostInRam { step, post: bike::Post::default(), post_last: Vec::new() };
    let mut m = PostsInRam { inner, answered };
    let (mut q, mut b) = m.inner.read();
    bike::air_state(&mut q, &mut b, dt, &mut m);
    m.inner.write(&q, &b);
}

/// The bike's collision record as [`RecordInRam`] sees its outside, the bike's scene node's
/// transforms Rust's (on the scene [`steps::read`] reads) and the body's damping too; the probes'
/// searches and the terrain query as [`RecordInRam`]'s.
struct RecordsInRam<'a, 't, 'm> {
    inner: RecordInRam<'a, 't>,
    answered: &'m mut BTreeMap<String, u32>,
}

impl RecordsInRam<'_, '_, '_> {
    fn scene<R>(&mut self, what: &str, f: impl FnOnce(&mut game_sim::atv_moto::Scene) -> R) -> R {
        *self.answered.entry(format!("record {what}")).or_default() += 1;
        let node = self.inner.word(0xe8);
        assert_eq!(node, self.inner.ee.u32_at(BIKE + 0xe4), "the record's node is the bike's");
        on_scene(self.inner.ee, f)
    }
}

impl RecordsInRam<'_, '_, '_> {
    /// F47: the record's node and its RAM, if the node has a parent (which the scene
    /// [`steps::read`] reads does not hold): the node transforms then [`node`]'s on RAM.
    fn parented(&mut self, what: &str) -> Option<(u32, NodeRam<'_>)> {
        let n = self.inner.word(0xe8);
        if self.inner.ee.u32_at(n + NODE_PARENT) == 0 {
            return None;
        }
        *self.answered.entry(format!("record {what}, parented")).or_default() += 1;
        Some((n, NodeRam(self.inner.ee)))
    }
}

impl Collides for RecordsInRam<'_, '_, '_> {
    fn damp(&mut self, linear: Vf, angular: Vf) {
        *self.answered.entry("record damp".into()).or_default() += 1;
        let at = self.inner.word(0xe4);
        let ee = &mut *self.inner.ee;
        let mut b = seed::body(&|a| ee.u32_at(a), at);
        (b.damping, b.ang_damping) = (linear, angular);
        for (a, w) in seed::body_words(&b, at) {
            ee.set_u32(a, w);
        }
    }
    fn node_rotate(&mut self, v: Vf) -> Vf {
        if let Some((n, mut m)) = self.parented("node_rotate") {
            return Vf::from_bits(node::rotate(&mut m, n, v.to_bits()));
        }
        self.scene("node_rotate", |s| s.turn(0, v))
    }
    fn node_position(&mut self) -> Vf {
        let out = self.inner.at + 0xd0;
        if let Some((n, mut m)) = self.parented("node_position") {
            node::position(&mut m, n, out);
            return vf_at(self.inner.ee, out);
        }
        let p = self.scene("node_position", |s| s.position(0));
        set_vf(self.inner.ee, out, p);
        p
    }
    fn node_place(&mut self, v: Vf) -> Vf {
        if let Some((n, mut m)) = self.parented("node_place") {
            return Vf::from_bits(node::place(&mut m, n, v.to_bits()));
        }
        self.scene("node_place", |s| s.tip(0, v))
    }
    fn node_move(&mut self, at: Vf) {
        if let Some((n, mut m)) = self.parented("node_move") {
            node::set(&mut m, n, at.to_bits());
            return;
        }
        self.scene("node_move", |s| s.set_position(0, at)).expect("the bike's node has no parent");
    }
    fn node_up(&self) -> [F; 3] {
        self.inner.node_up()
    }
    fn chassis_velocity(&self) -> Vf {
        self.inner.chassis_velocity()
    }
    fn sweep(&mut self, from: Vf, to: Vf, nearby: bool) -> Option<Swept> {
        // F46c: the nearby sweep Rust's; F51: its pairs and the global one (never on C15) too.
        if !in_rust("record sweep") {
            *self.answered.entry("> record sweep".into()).or_default() += 1;
            return self.inner.sweep(from, to, nearby);
        }
        *self.answered.entry("record sweep".into()).or_default() += 1;
        let (at, probe, list) = (self.inner.at, self.inner.word(0x204), self.inner.word(0xfc));
        let hit = if nearby {
            sweep_in_ram(self.inner.ee, at, probe, list, from, to, self.answered)
        } else {
            sweep_set_in_ram(self.inner.ee, at, probe, from, to);
            sweep_all_in_ram(self.inner.ee, probe, 0, self.answered).expect("a listener searched the database inside the record's sweep: not followed");
            self.inner.ee.u8_at(probe + 0x126) & ws::flags::HIT != 0
        };
        if !hit {
            return None;
        }
        let ee = &*self.inner.ee;
        let hits = ee.u32_at(probe + 0x7c);
        let obj = ee.u32_at(ee.u32_at(hits + 0x9c));
        let (normal, point) = (vf_at(ee, ee.u32_at(hits + 0x94)), vf_at(ee, ee.u32_at(hits + 0x98)));
        let struck = if obj == 0 { Struck::default() } else { self.inner.struck(obj) };
        Some(Swept { normal, point, struck })
    }
    fn place_part(&mut self) {
        *self.answered.entry("> record place_part".into()).or_default() += 1;
        self.inner.place_part()
    }
    fn part_rays(&self) -> Vec<[Vf; 2]> {
        self.inner.part_rays()
    }
    fn search(&mut self, nearby: bool) -> Vec<Hit> {
        // F46d: the nearby search Rust's; the global one (never on C15) still the original's.
        if !(nearby && in_rust("record search")) {
            *self.answered.entry("> record search".into()).or_default() += 1;
            return self.inner.search(nearby);
        }
        *self.answered.entry("record search".into()).or_default() += 1;
        let (part, list) = (self.inner.word(0x200), self.inner.word(0xfc));
        let contacts = part_search_in_ram(self.inner.ee, part, list, self.answered);
        contacts
            .iter()
            .map(|c| Hit { from: c.from, to: c.to, normal: c.normal, point: c.point, struck: self.inner.struck(c.object) })
            .collect()
    }
    fn terrain_box(&self) -> [Vf; 2] {
        self.inner.terrain_box()
    }
    fn under(&mut self, g: &mut Ground) {
        self.inner.under(g)
    }
    fn velocity(&mut self, s: &Struck) -> Vf {
        *self.answered.entry("> record velocity".into()).or_default() += 1;
        self.inner.velocity(s)
    }
    fn other(&mut self, id: u32) -> &mut Record {
        self.inner.other(id)
    }
}

/// [`port_record`] with its outside [`RecordsInRam`]'s.
fn posts_record(ee: &mut Ee, at: u32, slot: u32, dt: F, terrain: Option<&game_sim::atv_ground::Terrain<'_>>, answered: &mut BTreeMap<String, u32>) {
    let mut rec = record::read(&|a| ee.u32_at(a), at);
    let read = rec.clone();
    let body_at = ee.u32_at(at + 0xe4);
    let mut body = (body_at != 0).then(|| quad_body(ee, body_at));
    let inner = RecordInRam { ee, at, terrain, others: BTreeMap::new(), queries: 0 };
    let mut w = RecordsInRam { inner, answered };
    if slot == 0x7c {
        record::clear(&mut rec, &mut w);
    } else {
        record::pass(&mut rec, at, body.as_mut(), dt, &mut w);
    }
    w.inner.finish(&read, &rec, body.as_ref().map(|b| (b, body_at)));
}

/// A frame as [`PortSteps`]', the bike's PostProcess [`posts_post`]'s.
struct PortPosts<'a, 'm>(PortSteps<'a, 'm>);

impl Dynamics for PortPosts<'_, '_> {
    fn count(&self, l: List) -> i32 {
        self.0.count(l)
    }
    fn preforce(&mut self, i: i32, dt: F) {
        self.0.preforce(i, dt)
    }
    fn force(&mut self, i: i32, h: F) {
        self.0.force(i, h)
    }
    fn enabled(&self, b: i32) -> bool {
        self.0.enabled(b)
    }
    fn integrate(&mut self, b: i32, h: F) {
        self.0.integrate(b, h)
    }
    fn end_substep(&mut self, b: i32) {
        self.0.end_substep(b)
    }
    fn constrain(&mut self, b: i32, last: bool) {
        self.0.constrain(b, last)
    }
    fn post(&mut self, i: i32, dt: F) {
        let s = &mut self.0;
        if s.post.0.0.0.elt(List::Post, i) != BIKE_POST {
            return s.post(i, dt);
        }
        let x = &mut s.post.0.0.0;
        for (a, w) in seed::body_words(&x.body, x.at) {
            x.ee.set_u32(a, w);
        }
        posts_post(x.ee, dt, &mut s.answered);
        x.body = seed::body(&|a| x.ee.u32_at(a), x.at);
        s.post.1 += 1;
    }
}

/// [`steps_frame`] with the bike's record's outside [`RecordsInRam`]'s and its PostProcess's
/// callees [`PostsInRam`]'s. Answers as [`steps_frame`] does, the record's and the PostProcess's
/// calls counted in with the steps'.
fn posts_frame(ee: &mut Ee, dt: F, map: &track::Terrain<'_>, noise: &mut Option<SmoothNoise>, pure: bool) -> (u32, u32, u32, BTreeMap<String, u32>) {
    let mut mark = (ee.ran, "the records' clear and pass");
    let call = |ee: &mut Ee, f: u32, ints: &[u64], x: F| {
        ee.set(28, GP);
        ee.call(f, ints, &[x.0], BUDGET);
        ee.r(2)
    };
    // F31b5m: in the pure run the sim lists' accessors are [`tail::list_get`] and
    // [`tail::list_remove`], and with `"other listeners dropped"` the frame's listeners other than the
    // bike's and the rider's are not run ([`dropped_others`]).
    let get = |ee: &mut Ee, list: u32, i: u32| {
        if pure { tail::list_get(&mut NodeRam(ee), list, i as i32) } else { call(ee, if list == SIM + 0x24 { LIST24_GET } else { LIST3C_GET }, &[u64::from(list), u64::from(i)], F::ZERO) as u32 }
    };
    assert_eq!(ee.u8_at(ee.u32_at(SKIP_FLAG) + 0x225) & 8, 0, "the frame is not skipped");
    let mine = ee.u32_at(BIKE + seed::RECORD);
    let mut answered = BTreeMap::new();
    for (slot, original) in [(0x7c, RECORD_CLEAR), (0x84, RECORD_PASS)] {
        for i in 0..ee.u32_at(SIM + 0x24) {
            let e = get(ee, SIM + 0x24, i);
            let f = ee.u32_at(ee.u32_at(e + 4) + slot);
            if e == mine {
                assert_eq!(f, original, "the bike's record's vt +{slot:#x}");
                let t = ee.u32_at(e + 0xec);
                let terrain = (t != 0).then(|| ground::read_terrain(&|a| ee.u32_at(a), t, Some(map)));
                posts_record(ee, e, slot, dt, terrain.as_ref(), &mut answered);
            } else if dropped_others(pure) {
                *answered.entry(format!("dropped record +{slot:#x}")).or_default() += 1;
            } else {
                call(ee, f, &[u64::from(e)], dt);
            }
        }
    }
    interpreted(ee, &mut mark, "the bike's world step");
    let world = World { iterations: ee.u32_at(WORLD) as i32, rate: F(ee.f32_at(WORLD + 4)) };
    let at = ee.u32_at(BIKE + seed::BODY);
    let body = seed::body(&|a| ee.u32_at(a), at);
    let post = PortPost(PortStep(PortHead(Hybrid { ee, body, at, integrated: 0 }), 0), 0);
    let mut h = PortPosts(PortSteps { post, map, noise, answered, heads: 0, steps: 0, pure });
    world.step(dt, &mut h);
    let PortPosts(h) = h;
    let (body, heads, steps, posts, answered) = (h.post.0.0.0.body, h.heads, h.steps, h.post.1, h.answered);
    for (a, w) in seed::body_words(&body, at) {
        ee.set_u32(a, w);
    }
    interpreted(ee, &mut mark, "the tail");
    let (bikes, others) = port_tail(ee, dt);
    assert_eq!((bikes, others.len()), (1, 3), "the tail steps the bike once, its two other leaves and the second's rider: {others:#x?}");
    interpreted(ee, &mut mark, "the +0x3c list");
    let mut i = 0;
    while i < ee.u32_at(SIM + 0x3c) {
        let e = get(ee, SIM + 0x3c, i);
        if call(ee, ee.u32_at(ee.u32_at(e) + 8), &[u64::from(e)], dt) != 0 {
            if pure {
                tail::list_remove(&mut NodeRam(ee), SIM + 0x3c, i as i32);
            } else {
                call(ee, LIST3C_REMOVE, &[u64::from(SIM + 0x3c), u64::from(i)], F::ZERO);
            }
        } else {
            i += 1;
        }
    }
    interpreted(ee, &mut mark, "");
    (heads, steps, posts, answered)
}

/// F31b5m: whether [`posts_frame`]'s pure run leaves out the frame's other listeners (F31b5d, F39:
/// none of their writes reaches the bike): the sim's records but the bike's (their clear and pass),
/// the world's PreProcesses but the bike's (the other riders'), the tail's first other leaf and the
/// manager's own work. The manager's closing walk, which steps the rider, still runs.
fn dropped_others(pure: bool) -> bool {
    pure && in_rust("other listeners dropped")
}

thread_local! {
    /// F31b5: the instructions the interpreter ran in each of [`posts_frame`]'s parts, summed until
    /// taken.
    static INTERPRETED: RefCell<BTreeMap<String, u64>> = const { RefCell::new(BTreeMap::new()) };
}

/// Books `n` instructions to `part` (a part nested in one of [`posts_frame`]'s, counted in both).
fn book(part: impl Into<String>, n: u64) {
    if n > 0 {
        INTERPRETED.with(|m| *m.borrow_mut().entry(part.into()).or_default() += n);
    }
}

/// Books the instructions run since `mark` to its part, and starts `next`'s.
fn interpreted(ee: &Ee, mark: &mut (u64, &'static str), next: &'static str) {
    book(mark.1, ee.ran - mark.0);
    *mark = (ee.ran, next);
}

/// Where two RAMs differ outside the interpreter's stack and scratch (the 256 KiB under `stack`
/// and the 1 KiB over it), as `start..end` ranges.
fn bytes_differ(a: &[u8], b: &[u8], stack: u32) -> Vec<String> {
    let stack = (stack.saturating_sub(0x4_0000) as usize)..(stack as usize + 0x400);
    let mut out = Vec::new();
    let mut run: Option<(usize, usize)> = None;
    for i in (0..a.len()).step_by(4) {
        let off = !stack.contains(&i) && a[i..i + 4] != b[i..i + 4];
        match (&mut run, off) {
            (Some((_, e)), true) if *e == i => *e = i + 4,
            (_, true) => {
                out.extend(run.take().map(|(s, e)| format!("{s:#x}..{e:#x}")));
                run = Some((i, i + 4));
            }
            _ => {}
        }
    }
    out.extend(run.map(|(s, e)| format!("{s:#x}..{e:#x}")));
    out
}

/// [`bytes_differ`] on two interpreters' RAMs, and their scratchpads.
fn ram_differs(a: &Ee, b: &Ee) -> Vec<String> {
    let mut out = bytes_differ(&a.ram, &b.ram, a.stack);
    if a.spr != b.spr {
        out.push("the scratchpad".into());
    }
    out
}

/// **F31b5b — the callees Rust's, in lockstep with F31b5a's**: C15's `name` replayed twice, once by
/// [`steps_frame`] (its record's outside and its PostProcess's callees the original's) and once by
/// [`posts_frame`]; every row word for word against C15, and the two RAMs alike after every frame
/// outside the stack. Answers the wreck state per row and what the second run's callees answered.
fn replay_posts(name: &str) -> Option<(Vec<bool>, BTreeMap<String, u32>)> {
    let words = mass_words(&race250p()?)?;
    let trn = nat05_trn()?;
    let map = track::Terrain::parse(&trn).unwrap();
    let mut a = hooked(image()?);
    for (at, mask, v) in words {
        a.set_u32(at, a.u32_at(at) & !mask | v & mask);
    }
    let mut b = hooked(image()?);
    b.ram.clone_from(&a.ram);
    let (mut noise_a, mut noise_b) = (None, None);
    let rows = rows(name)?;
    let dt = F(f32::from_bits(DT));
    let (mut out, mut all) = (Vec::new(), BTreeMap::<String, u32>::new());
    for (r, row) in rows.iter().enumerate() {
        poke(&mut a, row, &READER);
        poke(&mut b, row, &READER);
        steps_frame(&mut a, dt, &map, &mut noise_a);
        let (heads, steps, posts, answered) = posts_frame(&mut b, dt, &map, &mut noise_b, false);
        assert_eq!((heads, steps, posts), (1, 4, 1), "the head once, the Force body every substep, the PostProcess once");
        for (k, n) in answered {
            *all.entry(k).or_default() += n;
        }
        let d = diff(&b, row);
        assert!(
            d.is_empty(),
            "{name} row {r} (vsync {}), the callees Rust's: {} words differ\n  {}",
            row["vsync"],
            d.len(),
            d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  ")
        );
        let off = ram_differs(&a, &b);
        assert!(off.is_empty(), "{name} row {r}: RAM differs from F31b5a's run at {} ranges\n  {}", off.len(), off.iter().take(40).cloned().collect::<Vec<_>>().join("\n  "));
        let w = wrecked(|o| b.u32_at(BIKE + o));
        assert_eq!(w, row_wrecked(row), "{name} row {r}: the wreck state");
        out.push(w);
        poke(&mut a, row, &SPOT);
        poke(&mut b, row, &SPOT);
    }
    eprintln!("{name}: the callees answered {all:?}");
    Some((out, all))
}

/// **F31b5b — state 4's bike for its PostProcess, read and written back**: [`steps::read_post`]'s
/// bike, put back by [`steps::post_words`], is state 4's at every word it names; the riding probe is
/// the active one, both a ray probe, the frame a mesh; neither the bike nor its node is in the
/// database.
#[test]
fn state4s_bike_reads_back_for_its_postprocess() {
    let Some(ee) = image() else { return };
    let m = |a| ee.u32_at(a);
    let (k, at, parts) = steps::read_post(&m, BIKE, KINDS).unwrap();
    let w = steps::post_words(&k, &at, &parts, &m);
    let off: Vec<String> =
        w.iter().filter(|&&(a, v)| ee.u32_at(a) != v).map(|&(a, v)| format!("{a:#x}: state 4 {:#010x}, words {v:#010x}", ee.u32_at(a))).collect();
    assert!(off.is_empty(), "{} of {} words differ:\n  {}", off.len(), w.len(), off.join("\n  "));
    let c = &k.parts.collidables;
    assert!(c.sync, "the collision sync is on");
    assert_eq!((c.riding, c.frame, c.active), (Some(quad::Part::Riding), Some(quad::Part::Frame), Some(quad::Part::Riding)));
    use game_sim::atv_sync::{Kind, slot};
    assert!(matches!(c.states[slot(quad::Part::Riding)].kind, Kind::Rays { .. }));
    assert!(matches!(c.states[slot(quad::Part::Frame)].kind, Kind::Mesh { .. }));
    assert!(!k.parts.spatial && !k.parts.scene.nodes[0].spatial, "neither the bike nor its node is in the database");
    assert_ne!(ee.u32_at(at.node + 0xf4), 0, "the bike's node has children (the rider's)");
}

/// A node with no parent and no children, its frame and flags off `pick`, for the transforms'
/// oracle tests: written into RAM at `node` and read back as the scene's node 0.
fn random_node(ee: &mut Ee, node: u32, pick: &mut impl FnMut() -> u32) -> game_sim::atv_moto::Scene {
    let mut f = || F(((pick() % 4001) as f32 - 2000.0) / 1000.0);
    for o in (0x30..0xb0).step_by(4) {
        let v = f();
        ee.set_u32(node + o, v.0.to_bits());
    }
    // Clean or not, its bounds stale or not.
    let flags = ee.u32_at(node + 0x100) & !0x81 | [0x00, 0x01, 0x80, 0x81][(pick() % 4) as usize];
    ee.set_u32(node + 0x100, flags);
    let m = |a| ee.u32_at(a);
    steps::read(&m, BIKE).unwrap().0.parts.scene
}

/// A run of numbers off a seed, for the oracle tests.
fn picker(mut s: u32) -> impl FnMut() -> u32 {
    move || {
        s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
        s >> 8
    }
}

/// **F31b5b — the node's transforms are the original's**: on state 4's bike's node, its frame and
/// flags drawn at random 10,000 times, [`game_sim::atv_moto::Scene::turn`], `tip`, `position` and
/// `set_position` answer the original's rotate, point, position and set (no parent), bit for bit,
/// and leave the node's words as the original leaves them.
#[test]
fn the_node_transforms_are_the_originals() {
    let Some(mut ee) = image() else { return };
    let node = ee.u32_at(BIKE + 0xe4);
    let mut pick = picker(0x5eed);
    let (mut off, mut marked) = (Vec::new(), 0);
    let base = ee.ram.clone();
    for k in 0..10_000 {
        let mut scene = random_node(&mut ee, node, &mut pick);
        let mut f = || f32::from_bits(((pick() % 4001) as f32 / 100.0 - 20.0).to_bits());
        let v = Vf::new(f(), f(), f(), f());
        let (out, p) = (ee.scratch(1), ee.scratch(0));
        set_vf(&mut ee, p, v);
        let snapshot = ee.ram.clone();
        let (want, got, name) = match k % 4 {
            0 => {
                ee.set(28, GP);
                ee.call(NODE_ROTATE, &[u64::from(out), u64::from(node), u64::from(p)], &[], BUDGET);
                (vf_at(&ee, out), scene.turn(0, v), "rotate")
            }
            1 => {
                ee.set(28, GP);
                ee.call(NODE_TIP, &[u64::from(out), u64::from(node), u64::from(p)], &[], BUDGET);
                (vf_at(&ee, out), scene.tip(0, v), "point")
            }
            2 => {
                ee.set(28, GP);
                ee.call(NODE_POSITION, &[u64::from(node), 0, u64::from(out)], &[], BUDGET);
                (vf_at(&ee, out), scene.position(0), "position")
            }
            _ => {
                ee.set(28, GP);
                ee.call(NODE_SET, &[u64::from(node), 0, u64::from(p)], &[], BUDGET);
                scene.set_position(0, v).unwrap();
                (Vf::ZERO, Vf::ZERO, "set")
            }
        };
        if want.to_bits() != got.to_bits() {
            off.push(format!("{k} {name}: original {:x?}, port {:x?}", want.to_bits(), got.to_bits()));
        }
        // RAM: the original's after its call, the port's the scene's words put back.
        let after = std::mem::replace(&mut ee.ram, snapshot);
        let m = |a| ee.u32_at(a);
        let (mut b, at) = steps::read(&m, BIKE).unwrap();
        let last = steps::words(&b, &at, &m);
        b.parts.scene = scene;
        rows_heights(&mut b);
        let now = steps::words(&b, &at, &m);
        set_changed(&mut ee, &now, &last);
        if name == "set" {
            marked += usize::from(ee.u8_at(ee.u32_at(node + 0xf4) + 0x100) & 1 == 0);
        }
        off.extend(bytes_differ(&after, &ee.ram, ee.stack).into_iter().map(|r| format!("{k} {name}: RAM differs at {r}")));
        ee.ram.clone_from(&base);
    }
    assert!(marked > 500, "the set marked the node's children out of date {marked} times");
    assert!(off.is_empty(), "{} differ:\n  {}", off.len(), off.iter().take(30).cloned().collect::<Vec<_>>().join("\n  "));
}

/// **F31b5b — the record's damping setters are its port's**: the body's virtuals `+0x20` and `+0x68`
/// on random `(d, d, d, 1)` write the body's damping and its angular damping, and nothing else.
#[test]
fn the_damping_setters_are_the_ports() {
    let Some(mut ee) = image() else { return };
    let body = ee.u32_at(ee.u32_at(BIKE + seed::RECORD) + 0xe4);
    assert_eq!(body, ee.u32_at(BIKE + seed::BODY), "the record's body is the bike's");
    let mut pick = picker(0xda4f);
    let mut off = Vec::new();
    for k in 0..2000 {
        let d = ((pick() % 20001) as f32 - 10000.0) / 997.0;
        let (lin, ang) = (Vf::new(d, d, d, 1.0), Vf::new(d * 0.5, d * 0.5, d * 0.5, 1.0));
        let before = ee.ram.clone();
        for (slot, v) in BODY_DAMP.into_iter().zip([lin, ang]) {
            let p = ee.scratch(0);
            set_vf(&mut ee, p, v);
            let f = ee.u32_at(ee.u32_at(body + 0x110) + slot);
            ee.set(28, GP);
            ee.call(f, &[u64::from(body), u64::from(p)], &[], BUDGET);
        }
        let after = ee.ram.clone();
        ee.ram.clone_from(&before);
        let mut b = seed::body(&|a| ee.u32_at(a), body);
        (b.damping, b.ang_damping) = (lin, ang);
        for (a, w) in seed::body_words(&b, body) {
            ee.set_u32(a, w);
        }
        let stack = (ee.stack - 0x4_0000) as usize..(ee.stack + 0x400) as usize;
        for i in (0..after.len()).step_by(4) {
            if !stack.contains(&i) && after[i..i + 4] != ee.ram[i..i + 4] {
                off.push(format!("{k}: {i:#x} original {:02x?}, port {:02x?}", &after[i..i + 4], &ee.ram[i..i + 4]));
            }
        }
        ee.ram = after;
    }
    assert!(off.is_empty(), "{} differ:\n  {}", off.len(), off.iter().take(30).cloned().collect::<Vec<_>>().join("\n  "));
}

/// **F31b5b — C15's ride, the callees Rust's**: ride-2 in lockstep with F31b5a's run. The spatial
/// update, the collision sync, the node's position and placement and the record's node transforms
/// answered every frame; what is handed on is the sync's gather and the record's probe searches.
#[test]
fn ride_replays_bit_exact_with_rust_callees() {
    let Some((w, all)) = replay_posts("ride-2") else { return };
    assert_eq!(w.len(), 459);
    assert!(w.iter().all(|w| !w), "the ride never wrecks");
    for c in ["SpatialUpdate", "CollisionSync", "NodePosition", "Place"] {
        assert_eq!(all.get(c).copied(), Some(459), "{c} answered every frame: {all:?}");
    }
    // Handed on: the riding probe's and the frame mesh's database moves and the gather every frame,
    // and the record's two probe searches (the gravity's record call the resolution's, F31b5g).
    let handed: Vec<(&str, u32)> = all.iter().filter(|(k, _)| k.starts_with("> ")).map(|(k, &n)| (&k[2..], n)).collect();
    assert_eq!(handed, [("Gather", 459), ("PartMove", 918), ("record search", 459), ("record sweep", 459)]);
}

/// **F31b5b — C15's flip, the callees Rust's**: flip-1 in lockstep with F31b5a's run, wrecked on row
/// 125 and put back on row 193; the record's node point answered too.
#[test]
fn flip_replays_bit_exact_with_rust_callees() {
    let Some((w, all)) = replay_posts("flip-1") else { return };
    assert_eq!(w.len(), 465);
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [125, 193], "wrecked on row 125, put back on row 193");
    assert!(all.get("record node_place").copied().unwrap_or(0) > 0, "{all:?}");
    let handed: Vec<(&str, u32)> = all.iter().filter(|(k, _)| k.starts_with("> ")).map(|(k, &n)| (&k[2..], n)).collect();
    let answered: Vec<(&str, u32)> = all.iter().filter(|(k, _)| k.starts_with("record ") || ["SpatialUpdate", "CollisionSync", "NodePosition", "Place"].contains(&k.as_str())).map(|(k, &n)| (k.as_str(), n)).collect();
    // The record's node point 470 times; the database moves the riding probe's every frame and the
    // frame mesh's on 306 (out of the database on the other 159).
    assert_eq!(
        answered,
        [("CollisionSync", 465), ("NodePosition", 465), ("Place", 465), ("SpatialUpdate", 465), ("record node_place", 470)]
    );
    assert_eq!(
        handed,
        [
            ("Gather", 465),
            ("PartMove", 771),
            ("PreloadCue", 1),
            ("StartStunt", 1),
            ("record search", 465),
            ("record sweep", 465)
        ]
    );
}

/// **F31b5g — the steps hand nothing on**: C15's `name` replayed twice in lockstep by
/// [`posts_frame`], once handing the steps' calls on to the original and once with them Rust's
/// ([`StepsInRam::in_rust`]: the preload's cue and the run's event posts dropped, F31b5h: the run's calls ported); every row of
/// the second word for word against C15, and the bike and its blocks the first's after every frame.
/// Answers the wreck state per row, what the second run answered, and the RAM ranges where the two
/// came to differ (the dropped posts' writes, none the bike's).
fn replay_pure(name: &str, rust: &[&'static str]) -> Option<(Vec<bool>, BTreeMap<String, u32>, Vec<String>)> {
    let words = mass_words(&race250p()?)?;
    let trn = nat05_trn()?;
    let map = track::Terrain::parse(&trn).unwrap();
    let mut a = hooked(image()?);
    for (at, mask, v) in words {
        a.set_u32(at, a.u32_at(at) & !mask | v & mask);
    }
    let mut b = hooked(image()?);
    b.ram.clone_from(&a.ram);
    let (mut noise_a, mut noise_b) = (None, None);
    let rows = rows(name)?;
    let dt = F(f32::from_bits(DT));
    let (mut out, mut all, mut off) = (Vec::new(), BTreeMap::<String, u32>::new(), Vec::<String>::new());
    for (r, row) in rows.iter().enumerate() {
        poke(&mut a, row, &READER);
        poke(&mut b, row, &READER);
        // F31b5m: with the other listeners dropped, the second run's frame reads no byte where the
        // two RAMs differ going into it.
        let watch = rust.contains(&"other listeners dropped");
        let parted = if watch { differing_bytes(&a, &b) } else { Vec::new() };
        let counted = INTERPRETED.with(|m| std::mem::take(&mut *m.borrow_mut()));
        posts_frame(&mut a, dt, &map, &mut noise_a, false);
        INTERPRETED.with(|m| *m.borrow_mut() = counted);
        IN_RUST.with(|x| *x.borrow_mut() = rust.to_vec());
        if watch {
            b.reads.replace(Some(Default::default()));
        }
        let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| posts_frame(&mut b, dt, &map, &mut noise_b, true)));
        IN_RUST.with(|x| x.borrow_mut().clear());
        let (heads, steps, posts, answered) = ran.unwrap_or_else(|e| std::panic::resume_unwind(e));
        if watch {
            let reads = b.reads.replace(None).unwrap_or_default();
            let bad = read_where_parted(&reads, &parted);
            assert!(bad.is_empty(), "{name} row {r}: the frame read {} bytes where the RAM parts from the handing run's\n  {}", bad.len(), bad.iter().take(40).map(|a| format!("{a:#x}")).collect::<Vec<_>>().join("\n  "));
        }
        assert_eq!((heads, steps, posts), (1, 4, 1), "the head once, the Force body every substep, the PostProcess once");
        for (k, n) in answered {
            *all.entry(k).or_default() += n;
        }
        let d = diff(&b, row);
        assert!(d.is_empty(), "{name} row {r} (vsync {}), nothing handed on: {} words differ\n  {}", row["vsync"], d.len(), d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  "));
        let (wa, wb) = (bike_and_blocks(&a), bike_and_blocks(&b));
        let words: Vec<String> = wa.iter().zip(&wb).filter(|(x, y)| x != y).map(|(x, y)| format!("{:#x}: {:#010x} / {:#010x}", x.0, x.1, y.1)).collect();
        assert!(words.is_empty(), "{name} row {r}: the bike and its blocks differ from the handing run's at {} words\n  {}", words.len(), words.iter().take(40).cloned().collect::<Vec<_>>().join("\n  "));
        for x in ram_differs(&a, &b) {
            if !off.contains(&x) {
                eprintln!("{name} row {r}: RAM first differs at {x}");
                off.push(x);
            }
        }
        let w = wrecked(|o| b.u32_at(BIKE + o));
        assert_eq!(w, row_wrecked(row), "{name} row {r}: the wreck state");
        out.push(w);
        poke(&mut a, row, &SPOT);
        poke(&mut b, row, &SPOT);
    }
    eprintln!("{name}: nothing handed on answered {all:?}");
    Some((out, all, off))
}

/// Where two interpreters' RAMs (outside the stack and its scratch, as [`bytes_differ`]) and
/// scratchpads differ, as sorted `start..end` byte ranges at word grain; the scratchpad's at its
/// `0x7000_0000` addresses.
fn differing_bytes(a: &Ee, b: &Ee) -> Vec<std::ops::Range<u32>> {
    let stack = (a.stack.saturating_sub(0x4_0000) as usize)..(a.stack as usize + 0x400);
    let mut out: Vec<std::ops::Range<u32>> = Vec::new();
    let mut add = |at: u32| match out.last_mut() {
        Some(r) if r.end == at => r.end = at + 4,
        _ => out.push(at..at + 4),
    };
    for i in (0..a.ram.len()).step_by(4) {
        if !stack.contains(&i) && a.ram[i..i + 4] != b.ram[i..i + 4] {
            add(i as u32);
        }
    }
    for i in (0..a.spr.len()).step_by(4) {
        if a.spr[i..i + 4] != b.spr[i..i + 4] {
            add(0x7000_0000 + i as u32);
        }
    }
    out
}

/// The bytes of `reads` (as the interpreter logs them, any mirror of RAM) inside `parted`
/// ([`differing_bytes`]).
fn read_where_parted(reads: &std::collections::BTreeSet<u32>, parted: &[std::ops::Range<u32>]) -> Vec<u32> {
    reads
        .iter()
        .map(|&a| if a >> 28 == 7 { 0x7000_0000 | (a & 0x3fff) } else { a & 0x0fff_ffff })
        .filter(|a| {
            let i = parted.partition_point(|r| r.end <= *a);
            parted.get(i).is_some_and(|r| r.contains(a))
        })
        .collect()
}

/// The steps' calls [`replay_pure`]'s second run answered in Rust (`= `) and handed on (`> `, none
/// of the steps' own: F46's PostProcess and record calls stay).
fn steps_handed(all: &BTreeMap<String, u32>) -> (Vec<(&str, u32)>, Vec<(&str, u32)>) {
    let steps = ["GravityRecord", "PreloadCue", "StartStunt", "EndStunt", "Wreck"];
    let pick = |p: &str| all.iter().filter(|(k, _)| k.starts_with(p) && steps.contains(&&k[2..])).map(|(k, &n)| (&k[2..], n)).collect();
    (pick("= "), pick("> "))
}

/// **F31b5g — C15's ride with nothing handed on**: ride-2 bit-exact, the steps handing nothing to
/// the interpreter (the gravity's record call the resolution's every substep); F31b5h: the whole
/// RAM the handing run's after every frame.
#[test]
fn ride_replays_bit_exact_with_nothing_handed_on() {
    let Some((w, all, off)) = replay_pure("ride-2", &[]) else { return };
    assert_eq!(off, Vec::<String>::new(), "the RAM is the handing run's");
    assert_eq!(w.len(), 459);
    assert!(w.iter().all(|w| !w), "the ride never wrecks");
    let (rust, handed) = steps_handed(&all);
    assert_eq!((rust, handed), (vec![], vec![]));
}

/// **F31b5g — C15's flip with nothing handed on**: flip-1 bit-exact, wrecked on row 125 and put back
/// on row 193; the preload's cue dropped and the backflip's stunt started in Rust, once each.
/// F31b5h: the run's calls the stunt's start and end make [`run`]'s, so the whole RAM is the handing
/// run's after every frame but for the one word the dropped cue writes (the run's dropped event
/// posts write no other).
#[test]
fn flip_replays_bit_exact_with_nothing_handed_on() {
    let Some((w, all, off)) = replay_pure("flip-1", &[]) else { return };
    assert_eq!(off, ["0x851fec..0x851ff0"], "the RAM differs only at the cue's write");
    assert_eq!(w.len(), 465);
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [125, 193], "wrecked on row 125, put back on row 193");
    let (rust, handed) = steps_handed(&all);
    assert_eq!((rust, handed), (vec![("PreloadCue", 1), ("StartStunt", 1)], vec![]));
}

/// **F31b5 — every port at once**: [`replay_pure`] (the steps hand nothing on) with F46's four
/// names Rust's too ([`IN_RUST`]), the tail on [`port_tail`]'s ports. Answers the wreck state per
/// row, what was answered and handed on, the RAM ranges where it came to differ from the handing
/// run, and the instructions the interpreter still ran in each part of the frame (the second run's
/// alone).
fn replay_whole(name: &str) -> Option<(Vec<bool>, BTreeMap<String, u32>, Vec<String>, BTreeMap<String, u64>)> {
    INTERPRETED.with(|m| m.borrow_mut().clear());
    let ran = replay_pure(name, &["PartMove", "Gather", "record sweep", "record search", "frame post", "fade search", "kind clear"]);
    let all = INTERPRETED.with(|m| std::mem::take(&mut *m.borrow_mut()));
    let (w, answered, off) = ran?;
    eprintln!("{name}: every port at once, the interpreter ran {all:?}");
    Some((w, answered, off, all))
}

/// **F31b5 — C15's ride, every port at once**: ride-2 bit-exact and the RAM the handing run's.
#[test]
fn ride_replays_with_every_port_at_once() {
    let Some((w, all, off, ran)) = replay_whole("ride-2") else { return };
    assert_eq!(off, Vec::<String>::new(), "the RAM is the handing run's");
    assert_eq!(w.len(), 459);
    assert!(w.iter().all(|w| !w), "the ride never wrecks");
    let handed: Vec<(&str, u32)> = all.iter().filter(|(k, _)| k.starts_with("> ")).map(|(k, &n)| (&k[2..], n)).collect();
    // F54's two are the only calls still handed on by name; the rest of what the interpreter runs is
    // in `ran` (F31b5i..m).
    assert_eq!(handed, [("record search heard_hit", 282), ("record search part stand", 432)], "{ran:?}");
    walk_served(&ran);
    // F31b5k: the connection's queue is the port's; the interpreter runs none of it.
    assert!(!ran.keys().any(|k| k.contains("NetQueue")), "{ran:?}");
    three_served(&ran);
}

/// **F31b5 — C15's flip, every port at once**: flip-1 bit-exact, wrecked on row 125 and put back on
/// row 193, the RAM the handing run's but for the dropped cue's word.
#[test]
fn flip_replays_with_every_port_at_once() {
    let Some((w, all, off, ran)) = replay_whole("flip-1") else { return };
    assert_eq!(off, ["0x851fec..0x851ff0"], "the RAM differs only at the cue's write");
    assert_eq!(w.len(), 465);
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [125, 193], "wrecked on row 125, put back on row 193");
    let handed: Vec<(&str, u32)> = all.iter().filter(|(k, _)| k.starts_with("> ")).map(|(k, &n)| (&k[2..], n)).collect();
    assert_eq!(handed, [("record search heard_hit", 110), ("record search part stand", 161)], "{ran:?}");
    walk_served(&ran);
    assert!(!ran.keys().any(|k| k.contains("NetQueue")), "{ran:?}");
    three_served(&ran);
}

/// F31b5l: the tail's frame post, the wreck fade's search and the kind clear are the ports', never
/// the interpreter's.
fn three_served(ran: &BTreeMap<String, u64>) {
    for k in ["the tail: out Message", "the tail: the update's Search", "the tail: the update's KindClear"] {
        assert!(!ran.contains_key(k), "{k} handed on: {ran:?}");
    }
}

/// F31b5j: the update's part walk is the port's ([`PartWalk`]), never the interpreter's, and it
/// stepped every kind of [`PART_LEAVES`].
fn walk_served(ran: &BTreeMap<String, u64>) {
    assert!(!ran.contains_key("the tail: the update's Walk"), "the part walk handed on: {ran:?}");
    for (_, _, name) in PART_LEAVES {
        let name = if name == "effects" { "effects item calls" } else { name };
        assert!(ran.contains_key(&format!("the tail: the update's {name}")), "the part walk's {name} never stepped: {ran:?}");
    }
    // F31b5n: the effects' own work is the port's.
    assert!(!ran.contains_key("the tail: the update's effects"), "the effects handed on: {ran:?}");
}

/// **F31b5j — what the part walk's leaves give the bike**: [`replay_whole`]'s ports with the
/// update's part leaves ([`PART_LEAVES`]) not stepped at all (`"part leaves dropped"`), or with one
/// kind's own work dropped (`"part leaf dropped: models"`). Each row is checked against C15 word for
/// word and the bike and its physics blocks against the handing run's after every frame
/// ([`replay_pure`]'s checks); the first that fails is answered as the panic's message. On success,
/// answers the wreck state per row and where the RAM parts from the handing run's.
fn replay_without_part_leaves(name: &str, mode: &'static str) -> Result<(Vec<bool>, Vec<String>), String> {
    let ran = std::panic::catch_unwind(|| replay_pure(name, &["PartMove", "Gather", "record sweep", "record search", mode]));
    match ran {
        Ok(Some((w, _, off))) => Ok((w, off)),
        Ok(None) => Err("skipped".into()),
        Err(e) => Err(e.downcast_ref::<String>().cloned().unwrap_or_default()),
    }
}

/// Ride-2 needs nothing of the part leaves: dropped, it is C15's on all 459 rows and the bike the
/// handing run's.
#[test]
fn ride_replays_without_the_part_leaves() {
    let (w, off) = match replay_without_part_leaves("ride-2", "part leaves dropped") {
        Err(e) if e == "skipped" => return,
        r => r.unwrap(),
    };
    assert_eq!(w.len(), 459);
    assert!(w.iter().all(|w| !w), "the ride never wrecks");
    assert!(!off.is_empty(), "the leaves dropped, the RAM parts from the handing run's");
}

/// The words flip-1's bike parts at, on row 114, with the part walk's effects or emitters not as
/// the original's: the trick hold's clip fraction (`+0x14e4`) and its copy (`+0x908`).
fn parts_at_the_hold(e: &str) {
    assert!(e.starts_with("flip-1 row 114: the bike and its blocks differ from the handing run's at 2 words"), "{e}");
    let at = |o: u32| format!("{:#x}: ", BIKE + o);
    assert!(e.contains(&at(0x908)) && e.contains(&at(0x14e4)), "{e}");
}

/// Flip-1 needs the part walk's effects and emitters. With every leaf dropped, or the effects' own
/// work alone (which turns the emitters on and off), the bike parts from the handing run's on row
/// 114 at the trick hold ([`parts_at_the_hold`]): the hold's shake samples the smooth noise, whose
/// tables state 4 has not built yet and the hold builds off the C library's `rand`, which the
/// emitters draw on every frame. The models' own work dropped, flip-1 is C15's on all 465 rows,
/// wreck and put-back, and the bike the handing run's.
#[test]
fn flip_needs_the_part_walks_effects() {
    match replay_without_part_leaves("flip-1", "part leaves dropped") {
        Err(e) if e == "skipped" => return,
        Err(e) => parts_at_the_hold(&e),
        Ok(_) => panic!("flip-1 rode the same with the part leaves dropped"),
    }
    match replay_without_part_leaves("flip-1", "part leaf dropped: effects") {
        Err(e) => parts_at_the_hold(&e),
        Ok(_) => panic!("flip-1 rode the same with the effects dropped"),
    }
    let (w, off) = replay_without_part_leaves("flip-1", "part leaf dropped: models").unwrap();
    assert_eq!(w.len(), 465);
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [125, 193], "wrecked on row 125, put back on row 193");
    assert!(!off.is_empty(), "the models dropped, the RAM parts from the handing run's");
}

/// **F31b5m — the frame's other listeners left out**: [`replay_whole`]'s ports with
/// `"other listeners dropped"` ([`dropped_others`]): the sim's records but the bike's, the other
/// riders' PreProcesses, the tail's first other leaf and the manager's own work are not run, and the
/// sim lists' accessors are Rust's. Every row is checked against C15 word for word and the bike and
/// its physics blocks against the handing run's after every frame ([`replay_pure`]), and the frame
/// reads no byte of RAM or scratchpad where the two runs differ going into it. Answers the wreck
/// state per row, what was answered and dropped, where the RAM parts and the interpreter's census.
fn replay_without_others(name: &str) -> Option<(Vec<bool>, BTreeMap<String, u32>, Vec<String>, BTreeMap<String, u64>)> {
    INTERPRETED.with(|m| m.borrow_mut().clear());
    let ran = replay_pure(name, &["PartMove", "Gather", "record sweep", "record search", "other listeners dropped"]);
    let all = INTERPRETED.with(|m| std::mem::take(&mut *m.borrow_mut()));
    let (w, answered, off) = ran?;
    eprintln!("{name}: the other listeners dropped, the interpreter ran {all:?}\n  the RAM parts at {} ranges", off.len());
    Some((w, answered, off, all))
}

/// What [`replay_without_others`] dropped, and that none of it ran in the interpreter.
fn others_dropped(name: &str, rows: u32, answered: &BTreeMap<String, u32>, ran: &BTreeMap<String, u64>, off: &[String]) {
    let dropped: Vec<(&str, u32)> = answered.iter().filter(|(k, _)| k.starts_with("dropped")).map(|(k, &n)| (k.as_str(), n)).collect();
    // State 4's records list holds only the bike's record (F31b5d), so no record is dropped; the
    // five other riders' PreProcesses are, once a frame each.
    assert_eq!(dropped, [("dropped PreProcess", 5 * rows)], "{name}");
    for part in ["the tail: the first other leaf", "the tail: the manager"] {
        assert!(!ran.contains_key(part), "{name}: {part} still ran: {ran:?}");
    }
    assert!(ran.contains_key("the tail: the rider"), "{name}: the manager's walk no longer steps the rider: {ran:?}");
    assert!(!off.is_empty(), "{name}: the others dropped, the RAM parts from the handing run's");
}

/// Ride-2 with the other listeners left out: C15's on all 459 rows, the bike and its blocks the
/// handing run's, no byte read where the RAMs part.
#[test]
fn ride_replays_with_the_other_listeners_dropped() {
    let Some((w, answered, off, ran)) = replay_without_others("ride-2") else { return };
    assert_eq!(w.len(), 459);
    assert!(w.iter().all(|w| !w), "the ride never wrecks");
    others_dropped("ride-2", 459, &answered, &ran, &off);
}

/// Flip-1 with the other listeners left out: C15's on all 465 rows, wrecked on row 125 and put back on
/// row 193, the bike and its blocks the handing run's, no byte read where the RAMs part.
#[test]
fn flip_replays_with_the_other_listeners_dropped() {
    let Some((w, answered, off, ran)) = replay_without_others("flip-1") else { return };
    assert_eq!(w.len(), 465);
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [125, 193], "wrecked on row 125, put back on row 193");
    others_dropped("flip-1", 465, &answered, &ran, &off);
}

/// **F31b5m — the sim lists' accessors are the original's**: 10 000 lists (a count from -3 to 40, an
/// array or none, sometimes the list's own words inside its array so the move writes the count), each
/// asked for an element from 4 before the array to 3 past the count. [`tail::list_get`] answers what
/// both of SLUS-21104's getters (the `+0x24` list's and the `+0x3c` list's) answer, and
/// [`tail::list_remove`] answers what the `+0x3c` list's remove answers and leaves the same words; the
/// original writes nothing outside the list and its array.
#[test]
fn the_sim_lists_accessors_are_the_originals() {
    let Some(mut ee) = image() else { return };
    let base = ee.scratch(0);
    let size = 0x200;
    let mut g = Gen(0x0031_b5d0);
    let (mut removed, mut refused, mut aliased) = (0, 0, 0);
    for case in 0..10_000 {
        let array = base + 0x40;
        for o in (0..size).step_by(4) {
            ee.set_u32(base + o, g.bits());
        }
        let n = (g.bits() % 44) as i32 - 3;
        let alias = n > 2 && g.one_in(8);
        let list = if alias {
            // The list's count and array words are elements of its own array.
            let j = g.bits() % (n as u32 - 2);
            ee.set_u32(array + 4 * j + 8, array);
            array + 4 * j
        } else {
            base
        };
        ee.set_u32(list, n as u32);
        if !alias && g.one_in(10) {
            ee.set_u32(list + 8, 0);
        } else {
            ee.set_u32(list + 8, array);
        }
        let i = if alias && g.one_in(2) { ((list - array) / 4) as i32 } else { (g.bits() % (n.max(0) as u32 + 7)) as i32 - 4 };
        aliased += i32::from(alias && list == array.wrapping_add((i as u32).wrapping_mul(4)));
        let before: Vec<u32> = (0..size).step_by(4).map(|o| ee.u32_at(base + o)).collect();
        let array_set = ee.u32_at(list + 8) != 0;
        if array_set {
            for f in [LIST24_GET, LIST3C_GET] {
                ee.set(28, GP);
                ee.call(f, &[u64::from(list), i as i64 as u64], &[], BUDGET);
                let want = ee.r(2) as u32;
                assert_eq!(tail::list_get(&mut NodeRam(&mut ee), list, i), want, "case {case}: the getter {f:#x}, n {n}, i {i}");
            }
        }
        ee.written.clear();
        ee.set(28, GP);
        ee.call(LIST3C_REMOVE, &[u64::from(list), i as i64 as u64], &[], BUDGET);
        let want = ee.r(2) as u32;
        let stack = ee.stack.saturating_sub(0x4_0000)..ee.stack;
        let outside: Vec<String> = ee.written_ranges().into_iter().filter(|&(s, e)| (s..e).any(|a| !stack.contains(&a) && !(base..base + size).contains(&a))).map(|(s, e)| format!("{s:#x}..{e:#x}")).collect();
        assert!(outside.is_empty(), "case {case}: the remove wrote outside the list: {outside:?}");
        let after: Vec<u32> = (0..size).step_by(4).map(|o| ee.u32_at(base + o)).collect();
        for (k, w) in before.iter().enumerate() {
            ee.set_u32(base + 4 * k as u32, *w);
        }
        let got = tail::list_remove(&mut NodeRam(&mut ee), list, i);
        assert_eq!(u32::from(got), want, "case {case}: the remove's answer, n {n}, i {i}");
        let mine: Vec<u32> = (0..size).step_by(4).map(|o| ee.u32_at(base + o)).collect();
        assert_eq!(mine, after, "case {case}: the remove's words, n {n}, i {i}");
        if got { removed += 1 } else { refused += 1 }
    }
    assert!(removed > 5000 && refused > 2000 && aliased > 50, "removed {removed}, refused {refused}, the count moved {aliased}");
}

/// F31b5n's oracle: the item virtuals' stand-ins, at fake addresses (`FX_FAKE + slot`, two vtables
/// `0x100` apart), and what each does to RAM so the port's re-reads are put to the test: an item
/// turned on reads 1 (3 with its mask's 0x40); off, 0 (2 with 0x80); a step with 0x100 takes one
/// off the items' count; a key with 0x200 one off the table's count, and the key is kept at the
/// item's `+0x1c`.
const FX_FAKE: u32 = 0x0000_2000;

fn fx_react(ee: &mut Ee, e: u32, slot: u32, item: u32, key: u32) {
    let mask = ee.u32_at(item + effects::ITEM_MASK);
    match slot {
        0x18 => ee.write(item + effects::ITEM_STATE, 1, if mask & 0x40 != 0 { 3 } else { 1 }),
        0x1c => ee.write(item + effects::ITEM_STATE, 1, if mask & 0x80 != 0 { 2 } else { 0 }),
        0x14 if mask & 0x100 != 0 => ee.set_u32(e + effects::ITEMS, ee.u32_at(e + effects::ITEMS).wrapping_sub(1)),
        8 => {
            ee.set_u32(item + 0x1c, key);
            if mask & 0x200 != 0 {
                let t = ee.u32_at(e + effects::TABLE);
                ee.set_u32(t, ee.u32_at(t).wrapping_sub(1));
            }
        }
        _ => {}
    }
}

/// The port's side of the oracle: RAM in place, each item call logged as (slot, item, key, `dt`'s
/// bits) and answered by [`fx_react`].
struct FxOracle<'a> {
    ee: &'a mut Ee,
    e: u32,
    log: Vec<(u32, u32, u32, u32)>,
}

impl Effects for FxOracle<'_> {
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn set_byte(&mut self, at: u32, b: u8) {
        self.ee.write(at, 1, u128::from(b));
    }
    fn out(&mut self, c: effects::Out, dt: F) {
        use effects::Out;
        let (slot, item, key, f) = match c {
            Out::Key { item, key } => (8, item, u32::from(key), 0),
            Out::On(item) => (0x18, item, 0, 0),
            Out::Off(item) => (0x1c, item, 0, 0),
            Out::Step(item) => (0x14, item, 0, dt.to_bits()),
        };
        self.log.push((slot, item, key, f));
        fx_react(self.ee, self.e, slot, item, key);
    }
}

/// **F31b5n — the effects' own work is the original's**: 10 000 effects listeners in scratch RAM,
/// each with up to six table entries (their tested words zero or not, keys kept or changed, the
/// table's word zero or not, counts down to −1) and up to eight items (unbound, bound to an entry or
/// to a stray flags byte; masks over all five flags; state bytes 0 to 3 and junk), run through
/// SLUS-21104's effects step with the items' virtuals hooked ([`fx_react`]) and through
/// [`effects::step`]. The items' calls come in the same order with the same arguments, the original
/// ends with its closing walk (the listener, `dt`), and the scratch RAM comes out the same.
#[test]
fn the_effects_step_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let fx = PART_LEAVES.iter().find(|l| l.2 == "effects").unwrap().0;
    let (e, table, entries, words, items, array, vts) =
        (ee.scratch(100), ee.scratch(101), ee.scratch(102), ee.scratch(108), ee.scratch(110), ee.scratch(115), ee.scratch(116));
    let region = (e, ee.scratch(118) - e);
    let log: Rc<RefCell<Vec<(u32, u32, u32, u32)>>> = Rc::default();
    for vt in 0..2 {
        for slot in [8, 0x14, 0x18, 0x1c] {
            let l = log.clone();
            ee.hooks.insert(
                FX_FAKE + 0x100 * vt + slot,
                Box::new(move |ee: &mut Ee| {
                    let (item, key) = (ee.r(4) as u32, ee.r(5) as u32);
                    let f = if slot == 0x14 { ee.f(12).to_bits() } else { 0 };
                    l.borrow_mut().push((slot, item, if slot == 8 { key } else { 0 }, f));
                    fx_react(ee, e, slot, item, key);
                }),
            );
        }
    }
    let l = log.clone();
    ee.hooks.insert(TAIL, Box::new(move |ee: &mut Ee| l.borrow_mut().push((0, ee.r(4) as u32, 0, ee.f(12).to_bits()))));
    let mut g = Gen(0x0031_b5e0);
    let mut seen = BTreeMap::<&str, u32>::new();
    for case in 0..10_000 {
        let mut setup = Vec::<(u32, u32)>::new();
        for o in (0..region.1).step_by(4) {
            setup.push((e + o, g.bits()));
        }
        for k in 0..16 {
            setup.push((words + 4 * k, if g.one_in(2) { 0 } else { g.bits() }));
        }
        let word_at = |g: &mut Gen| words + 4 * (g.bits() % 16);
        let n_entries = g.bits() % 8;
        setup.push((e + effects::TABLE, table));
        setup.push((table, n_entries.wrapping_sub(1)));
        setup.push((table + 4, entries));
        setup.push((table + effects::TABLE, word_at(&mut g)));
        for k in 0..6 {
            let at = entries + effects::ENTRY * k;
            setup.push((at + effects::FIRST, word_at(&mut g)));
            setup.push((at + effects::SECOND, word_at(&mut g)));
            setup.push((at + effects::KEY, words + g.bits() % 64));
        }
        let n_items = g.bits() % 10;
        setup.push((e + effects::ITEMS, n_items.wrapping_sub(1)));
        setup.push((e + effects::ITEM_ARRAY, array));
        for k in 0..8 {
            let item = items + 0x20 * k;
            setup.push((array + 4 * k, item));
            let link = match g.bits() % 4 {
                0 => 0,
                1 => words,
                _ => entries + effects::ENTRY * (g.bits() % 6),
            };
            setup.push((item + effects::ITEM_ENTRY, link));
            setup.push((item + effects::ITEM_STATE, if g.one_in(4) { g.bits() } else { g.bits() % 4 }));
            setup.push((item + effects::ITEM_MASK, g.bits() & 0x3df));
            setup.push((item + 0x10, vts + 0x20 * (g.bits() % 2)));
        }
        for vt in 0..2 {
            for slot in [8, 0x14, 0x18, 0x1c] {
                setup.push((vts + 0x20 * vt + slot, FX_FAKE + 0x100 * vt + slot));
            }
        }
        // Keys kept as they are, sometimes.
        let lay = |ee: &mut Ee, setup: &[(u32, u32)]| {
            for &(at, w) in setup {
                ee.set_u32(at, w);
            }
        };
        lay(&mut ee, &setup);
        for k in 0..6 {
            let at = entries + effects::ENTRY * k;
            if g.one_in(2) {
                let key = ee.u8_at(ee.u32_at(at + effects::KEY));
                setup.push((at + effects::FLAGS, (ee.u32_at(at + effects::FLAGS) & !0xff00) | u32::from(key) << 8));
            }
        }
        let dt = F(g.near(0.0333, -1.0, 1.0));
        lay(&mut ee, &setup);
        log.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(fx, &[u64::from(e)], &[dt.0], 1_000_000);
        let mut want = std::mem::take(&mut *log.borrow_mut());
        assert_eq!(want.pop(), Some((0, e, 0, dt.to_bits())), "case {case}: the original's closing walk");
        let want_ram = snap_regions(&ee, &[region]);
        lay(&mut ee, &setup);
        let mut port = FxOracle { ee: &mut ee, e, log: vec![] };
        effects::step(&mut port, e, dt);
        let got = std::mem::take(&mut port.log);
        assert_eq!(got, want, "case {case}: the items' calls");
        assert_eq!(snap_regions(&ee, &[region]), want_ram, "case {case}: the scratch RAM");
        for &(slot, ..) in &want {
            *seen.entry(match slot { 8 => "key", 0x14 => "step", 0x18 => "on", _ => "off" }).or_default() += 1;
        }
        if want.is_empty() {
            *seen.entry("no call").or_default() += 1;
        }
    }
    eprintln!("effects oracle: {seen:?}");
    assert_eq!(seen.len(), 5, "every kind of call, and none: {seen:?}");
}

/// **F31b5l — what the frame post's listeners give the bike**: [`replay_whole`]'s ports with the
/// tail's frame post telling no listener (`"frame post tells dropped"`): every row checked against
/// C15 word for word and the bike and its physics blocks against the handing run's after every frame
/// ([`replay_pure`]'s checks). Answers the wreck state per row and where the RAM parts from the
/// handing run's (the HUD's and the race progress's own words), or the first failure.
fn replay_without_frame_tells(name: &str) -> Result<(Vec<bool>, Vec<String>), String> {
    let names = ["PartMove", "Gather", "record sweep", "record search", "frame post", "fade search", "kind clear", "frame post tells dropped"];
    match std::panic::catch_unwind(|| replay_pure(name, &names)) {
        Ok(Some((w, _, off))) => Ok((w, off)),
        Ok(None) => Err("skipped".into()),
        Err(e) => Err(e.downcast_ref::<String>().cloned().unwrap_or_default()),
    }
}

/// Ride-2 needs nothing of the frame post's listeners: told nothing, it is C15's on all 459 rows
/// and the bike the handing run's.
#[test]
fn ride_replays_without_the_frame_posts_tells() {
    let (w, off) = match replay_without_frame_tells("ride-2") {
        Err(e) if e == "skipped" => return,
        r => r.unwrap(),
    };
    assert_eq!(w.len(), 459);
    assert!(w.iter().all(|w| !w), "the ride never wrecks");
    assert!(!off.is_empty(), "the tells dropped, the RAM parts from the handing run's");
}

/// Flip-1 needs nothing of them either: C15's on all 465 rows, wreck and put-back, and the bike the
/// handing run's.
#[test]
fn flip_replays_without_the_frame_posts_tells() {
    let (w, off) = match replay_without_frame_tells("flip-1") {
        Err(e) if e == "skipped" => return,
        r => r.unwrap(),
    };
    assert_eq!(w.len(), 465);
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [125, 193], "wrecked on row 125, put back on row 193");
    assert!(!off.is_empty(), "the tells dropped, the RAM parts from the handing run's");
}

/// F31b5l: the part probe's box, its vt `+0xc4`.
const PART_BOUNDS_FN: u32 = 0x001b_d1d0;

/// **F31b5l — the part's box is the original's**: 20 000 part probes in scratch, each with a frame
/// and either a tree (its first node's box) or none (its first ray), every float drawn at random
/// (now and then 0, −0, 1, a denormal, the largest or any bits), run through SLUS-21104's vt `+0xc4`
/// and through [`atv_part::bounds`]: the centre and the half extents come out bit for bit.
#[test]
fn the_parts_box_is_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    assert_eq!(ee.u32_at(ee.u32_at(ee.u32_at(BIKE + 0xcc) + 0xc) + 0xc4), PART_BOUNDS_FN, "the riding part's vt +0xc4");
    let (part, node, rays, out) = (WRECK_AREA, WRECK_AREA + 0x400, WRECK_AREA + 0x500, WRECK_AREA + 0x600);
    let mut g = Gen(0x0031_b5c4);
    let mut t = BTreeMap::new();
    let draw = |g: &mut Gen| if g.one_in(6) { reset_float(g) } else { g.f(-400.0, 400.0).to_bits() };
    for k in 0..20_000 {
        let tree = g.one_in(2);
        tally(&mut t, if tree { "a tree" } else { "a ray" });
        for i in 0..16 {
            ee.set_u32(part + 0x130 + 4 * i, draw(&mut g));
        }
        for i in 0..9 {
            ee.set_u32(node + 4 * i, draw(&mut g));
        }
        for i in 0..8 {
            ee.set_u32(rays + 4 * i, draw(&mut g));
        }
        ee.set_u32(part + 0x17c, if tree { node } else { 0 });
        ee.set_u32(part + 0x180, rays);
        ee.set(28, GP);
        ee.call(PART_BOUNDS_FN, &[u64::from(part), u64::from(out), u64::from(out + 0x10)], &[], BUDGET);
        let want = [vf_at(&ee, out).to_bits(), vf_at(&ee, out + 0x10).to_bits()];
        let (nodes, _) = if tree { tree_in_ram(&ee, node, 0) } else { (vec![], 0) };
        let segments = [ray_probe::Segment { from: vf_at(&ee, rays), to: vf_at(&ee, rays + 0x10) }];
        let probe = ray_probe::RayProbe { transform: matrix_at(&ee, part + 0x130), root: 0, nodes: &nodes, segments: &segments };
        let (c, h) = atv_part::bounds(&probe);
        let got = [c.map(|x| x.0.to_bits()), h.map(|x| x.0.to_bits())];
        assert_eq!(got, want, "case {k} ({}): centre, half", if tree { "tree" } else { "ray" });
    }
    eprintln!("{t:?}");
}

/// **F31b5l — the kind clear is the original's**: 10 000 cases on state 4's bike, with its state
/// there or not and of kind 3 or not, 0 to 4 hung parts, the shown object there or not (in a space
/// or not, the bike's `+0x44` bit up or not), each of the two parts there or not with a category
/// list 0 to 6 long drawn from the cleared categories' bits, the bike's own, the loaded one's and
/// others, the bike's own and the loaded category drawn, and now and then a cleared name missing
/// from the table (so it is added). Run through SLUS-21104's vt `+0x39c` and through
/// [`update::kind_clear`] (the show [`update::show`]); the object's switch-on and the spatial insert
/// are logged on both sides, the same calls in the same order, and every word of the bike, the
/// parts, the table and the loaded category comes out the same.
#[test]
fn the_kind_clear_is_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    assert_eq!((vt(&ee, 0x39c), vt(&ee, 0x398)), (KIND_CLEAR_FN, SHOW_FN), "the bike's vt +0x39c and +0x398");
    let area = WRECK_AREA;
    let (state, hung, obj, obj_vt, on_fn, model) = (area, area + 0x100, area + 0x200, area + 0x300, area + 0x380, area + 0x400);
    let parts = [area + 0x800, area + 0xc00];
    let table = ee.u32_at(update::CATEGORY_NAMES);
    let names = ee.u32_at(table);
    assert!(names >= 3, "the categories' table holds {names} names");
    let regions = [(BIKE, 0x1800), (area, 0x1000), (table, 4 + 32 * (names + 4)), (update::LOADED_CATEGORY, 4)];
    let image_words = snap_regions(&ee, &regions);
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    let l = log.clone();
    ee.hooks.insert(on_fn, Box::new(move |ee: &mut Ee| l.borrow_mut().push(format!("on {:#x}", ee.r(4)))));
    let l = log.clone();
    ee.hooks.insert(SPACE_INSERT, Box::new(move |ee: &mut Ee| l.borrow_mut().push(format!("insert {:#x} {:#x} {:#x} {}", ee.r(4), ee.r(5), ee.r(6), ee.r(7)))));
    let mut g = Gen(0x0031_b5c1);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        restore_regions(&mut ee, &regions, &image_words);
        let kind = if g.one_in(4) { 3 } else { g.bits() % 5 };
        ee.set_u32(BIKE + 0xbc4, if g.one_in(5) { 0 } else { state });
        ee.set_u32(state + 0x28, kind);
        let n = g.bits() % 5;
        ee.set_u32(BIKE + 0x1470, n);
        ee.set_u32(BIKE + 0x1474, hung);
        for i in 0..n {
            ee.set_u32(hung + 4 * i, model + 0x10 * (i + 1));
        }
        ee.set_u32(BIKE + 0xe4, model);
        let flags = ee.u32_at(BIKE + 0x44);
        ee.set_u32(BIKE + 0x44, flags & !1 | u32::from(!g.one_in(4)));
        ee.set_u32(BIKE + 0xd0, if g.one_in(5) { 0 } else { obj });
        ee.set_u32(obj + 0x28, obj_vt);
        ee.set_u32(obj_vt + 0x20, on_fn);
        ee.set_u32(obj + 0x20, u32::from(g.one_in(2)));
        if g.one_in(8) {
            // A cleared name missing: its table entry renamed.
            let i = g.bits() % names;
            ee.set_u32(table + 4 + 32 * i, 0x5a5a_5a5a);
            tally(&mut t, "a name renamed");
        }
        let own = 1 << (g.bits() % 32);
        ee.set_u32(BIKE + 0x30c, own);
        let loaded = if g.one_in(3) { 0 } else { 1 << (g.bits() % 32) };
        ee.set_u32(update::LOADED_CATEGORY, loaded);
        for (j, p) in parts.into_iter().enumerate() {
            ee.set_u32(BIKE + [0xcc, 0xd4][j], if g.one_in(5) { 0 } else { p });
            let len = g.bits() % 7;
            ee.set_u32(p + 0x100, len);
            ee.set_u32(p + 0xfc, p + 0x200);
            for i in 0..len {
                let bit = match g.bits() % 5 {
                    0 => 1 << (g.bits() % names.min(32)),
                    1 => own,
                    2 => loaded,
                    3 => 0,
                    _ => 1 << (g.bits() % 32),
                };
                ee.set_u32(p + 0x200 + 4 * i, bit);
            }
        }
        let ran = ee.u32_at(BIKE + 0xbc4) != 0 && kind == 3;
        tally(&mut t, if ran { "kind 3" } else { "cleared" });
        let before = snap_regions(&ee, &regions);
        log.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(KIND_CLEAR_FN, &[u64::from(BIKE)], &[], BUDGET);
        let (want, want_log) = (snap_regions(&ee, &regions), std::mem::take(&mut *log.borrow_mut()));
        restore_regions(&mut ee, &regions, &before);
        update::kind_clear(&mut UpdateInRam { ee: &mut ee, x: 0.0 });
        let (got, got_log) = (snap_regions(&ee, &regions), std::mem::take(&mut *log.borrow_mut()));
        if want_log.iter().any(|c| c.starts_with("insert")) {
            tally(&mut t, "inserted");
        }
        assert_eq!(got_log, want_log, "case {k}: the calls");
        let off: Vec<String> = (0..want.len()).filter(|&i| want[i] != got[i]).map(|i| format!("word {i}: original {:#010x}, port {:#010x}", want[i], got[i])).collect();
        assert!(off.is_empty(), "case {k}: {}", off.join("\n  "));
    }
    eprintln!("{t:?}");
    assert!(t.values().all(|&n| n >= 500), "{t:?}");
}

/// The record's resolution (its vt `+0x88`), which the gravity calls with 1.
const RECORD_RESOLVE: u32 = 0x0020_5730;

/// What the last call wrote outside the interpreter's stack and scratch and outside `regions`
/// (`(start, len)`), as `start..end` ranges.
fn written_outside(ee: &Ee, regions: &[(u32, u32)]) -> Vec<String> {
    let stack = ee.stack.saturating_sub(0x4_0000)..ee.stack + 0x400;
    let inside = |a: u32| stack.contains(&a) || regions.iter().any(|&(at, n)| (at..at + n).contains(&a));
    ee.written_ranges().into_iter().filter(|&(s, e)| (s..e).any(|a| !inside(a))).map(|(s, e)| format!("{s:#x}..{e:#x}")).collect()
}

/// **F31b5g — the gravity's record call is the record's resolution**: 10 000 records, each state 4's
/// bike's with every word the resolution reads drawn at random (the record's damping switch and
/// levels, masses, inertia, centres, responses, masks and frictions, its junk normal speed and
/// relative velocity; the body's masses, damping, position, velocities, forces, corrections, inertia,
/// orientation, centre, moment, rates and twist; 0 to 14 contacts on a surface or
/// none, of kind 0: a kind-1 contact is always with another record, and none is held), run through SLUS-21104's `+0x88` handed 1 and through
/// [`record::held`], [`game_sim::atv_contact::Held::resolve`] on the body's centre of mass and
/// [`record::held_words`]. Every word of the record, its list and the body comes out the same, and
/// the original writes nothing else.
#[test]
#[allow(clippy::float_arithmetic)]
fn the_gravitys_record_call_is_the_records_resolution() {
    let Some(mut ee) = image() else { return };
    let rec = ee.u32_at(BIKE + seed::RECORD);
    let body = ee.u32_at(rec + 0xe4);
    assert_eq!(body, ee.u32_at(BIKE + seed::BODY), "the record's body is the bike's");
    assert_eq!(ee.u32_at(ee.u32_at(rec + 4) + 0x88), RECORD_RESOLVE, "the record's vt +0x88");
    assert_eq!(ee.u32_at(body + 0xe8), 0, "the body has no listeners");
    let list = ee.u32_at(rec + 0xf8);
    let collidable = ee.u32_at(rec + 0x208);
    assert_ne!(collidable, 0);
    let regions = [(rec, 0x210), (list, 14 * record::CONTACT), (body, 0x230), (collidable + 0x94, 4)];
    let image_words = snap_regions(&ee, &regions);
    let mut g = Gen(0x0031_b5a7);
    let mut t = BTreeMap::new();
    let quad = |ee: &mut Ee, g: &mut Gen, a: u32, s: f32, w: Option<f32>| {
        for i in 0..3 {
            let x = g.near(0.0, -s, s);
            ee.set_f32(a + 4 * i, x);
        }
        let w = w.unwrap_or_else(|| g.near(0.0, -2.0, 2.0));
        ee.set_f32(a + 12, w);
    };
    let around = |g: &mut Gen, x: f32| match g.bits() % 16 {
        0 => 0.0,
        1 => -x,
        2 => x,
        _ => x * g.f(0.5, 1.5),
    };
    for k in 0..10_000 {
        restore_regions(&mut ee, &regions, &image_words);
        // The record.
        ee.set_u32(rec + 0x44, u32::from(!g.one_in(4)));
        for at in [0x4c, 0x54] {
            let x = match g.bits() % 4 {
                0 => 1.0,
                1 => g.f(0.0, 1.0),
                2 => g.f(1.0, 3.0),
                _ => g.f(-0.5, 1.5),
            };
            ee.set_f32(rec + at, x);
        }
        let x = around(&mut g, 13.0435);
        ee.set_f32(rec + 0x58, x);
        let x = if g.one_in(8) { 0.0 } else { around(&mut g, 0.0767) };
        ee.set_f32(rec + 0x5c, x);
        quad(&mut ee, &mut g, rec + 0x70, 0.1, None);
        quad(&mut ee, &mut g, rec + 0x80, 4.0, None);
        quad(&mut ee, &mut g, rec + 0x90, 50.0, None);
        for at in [0x100, 0x104] {
            let x = if g.one_in(4) { 0xffff_ffff } else { g.bits() % 16 };
            ee.set_u32(rec + at, x);
        }
        let x = g.bits() % 16;
        ee.set_u32(collidable + 0x94, x);
        let x = around(&mut g, 0.5);
        ee.set_f32(rec + 0x10c, x);
        for at in [0x118, 0x130] {
            let (s, d) = (around(&mut g, 1500.0), around(&mut g, 200.0));
            let r = if g.one_in(3) { 0.0 } else { g.f(0.0, 1.0) };
            ee.set_f32(rec + at, s);
            ee.set_f32(rec + at + 4, d);
            ee.set_f32(rec + at + 8, r);
        }
        for at in [0x13c, 0x140] {
            ee.set_u32(rec + at, u32::from(!g.one_in(4)));
        }
        for at in [0x150, 0x160, 0x170, 0x180, 0x190] {
            match g.bits() % 4 {
                0 | 1 => quad(&mut ee, &mut g, rec + at, 0.0, Some(1.0)),
                2 => {
                    for i in 0..4 {
                        let x = if g.one_in(2) { 0.0 } else { 1.0 };
                        ee.set_f32(rec + at + 4 * i, x);
                    }
                }
                _ => quad(&mut ee, &mut g, rec + at, 2.0, None),
            }
            if ee.f32_at(rec + at) == 0.0 && ee.f32_at(rec + at + 4) == 0.0 && g.one_in(2) {
                for i in 0..3 {
                    ee.set_f32(rec + at + 4 * i, 1.0);
                }
            }
        }
        for i in 0..18 {
            let x = around(&mut g, 0.5);
            ee.set_f32(rec + 0x1a0 + 4 * i, x);
        }
        let x = g.f(-50.0, 50.0);
        ee.set_f32(rec + 0x1e8, x);
        quad(&mut ee, &mut g, rec + 0x1f0, 50.0, None);
        // The body.
        let x = around(&mut g, 13.0435);
        ee.set_f32(body + 0x18, x);
        let x = if g.one_in(8) { 0.0 } else { around(&mut g, 0.0767) };
        ee.set_f32(body + 0x1c, x);
        for (at, s) in [(0x70, 20.0), (0x80, 30.0), (0xa0, 1000.0), (0xd0, 10.0), (0x1b0, 1000.0), (0x1d0, 3.0), (0x200, 10.0)] {
            quad(&mut ee, &mut g, body + at, s, None);
        }
        quad(&mut ee, &mut g, body + 0x140, 0.1, None);
        for at in [0x30, 0x150] {
            if g.one_in(3) {
                quad(&mut ee, &mut g, body + at, 0.0, Some(1.0));
                for i in 0..3 {
                    ee.set_f32(body + at + 4 * i, 1.0);
                }
            } else {
                for i in 0..4 {
                    let x = g.f(0.0, 2.0);
                    ee.set_f32(body + at + 4 * i, x);
                }
            }
        }
        let q = match g.bits() % 12 {
            0 => [0.0, 0.0, 0.0, 1.0],
            1 => [0.0, 1.0, 0.0, 0.0],
            _ => {
                let q = [0; 4].map(|_| g.f(-1.0, 1.0));
                let n = q.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3) * g.f(0.999, 1.001);
                q.map(|x| x / n)
            }
        };
        for (i, x) in q.into_iter().enumerate() {
            ee.set_f32(body + 0x190 + 4 * i as u32, x);
        }
        if g.one_in(4) {
            set_vf(&mut ee, body + 0x1a0, Vf::new(0.0, 0.5, -0.5, 1.0));
        } else {
            quad(&mut ee, &mut g, body + 0x1a0, 4.0, None);
        }
        // The contacts.
        let n = if g.one_in(6) { 0 } else if g.one_in(6) { 14 } else { 1 + g.bits() % 13 };
        ee.set_u32(rec + 0xf0, n);
        for i in 0..n {
            let e = list + record::CONTACT * i;
            ee.set_u32(e, 0);
            quad(&mut ee, &mut g, e + 0x10, 3.0, None);
            quad(&mut ee, &mut g, e + 0x20, 3.0, None);
            let w = if g.one_in(3) { 0.0 } else { g.f(-2.0, 2.0) };
            match g.bits() % 8 {
                0 => set_vf(&mut ee, e + 0x30, Vf::new(0.0, 1.0, 0.0, w)),
                1 => set_vf(&mut ee, e + 0x30, Vf::new(-1.0, 0.0, -0.0, w)),
                2 => quad(&mut ee, &mut g, e + 0x30, 1.5, None),
                _ => {
                    let v = [0; 3].map(|_| g.f(-1.0, 1.0));
                    let l = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3);
                    set_vf(&mut ee, e + 0x30, Vf::new(v[0] / l, v[1] / l, v[2] / l, w));
                }
            }
            for at in [0x40, 0x50, 0x60, 0x70] {
                quad(&mut ee, &mut g, e + at, 20.0, None);
            }
            let s = if g.one_in(4) { u32::MAX } else { g.bits() % 16 };
            ee.set_u32(e + 0x80, s);
            ee.set_u32(e + 0x84, 0);
            ee.set_u32(e + 0x88, 0);
        }
        let m = |a| ee.u32_at(a);
        let mut h = record::held(&m, rec).unwrap_or_else(|| panic!("record {k}: not held"));
        let cofg = vf_at(&ee, body + 0x1a0);
        let before = snap_regions(&ee, &regions);
        ee.set(28, GP);
        ee.call(RECORD_RESOLVE, &[u64::from(rec), 1], &[], BUDGET);
        let stray = written_outside(&ee, &regions);
        assert!(stray.is_empty(), "record {k}: the original wrote outside the record, its list and the body: {stray:?}");
        let want = snap_regions(&ee, &regions);
        assert!(h.resolve(cofg), "record {k}: the port resolved nothing");
        restore_regions(&mut ee, &regions, &before);
        let m = |a| ee.u32_at(a);
        let words = record::held_words(&h, rec, &m);
        for (a, w) in words {
            ee.set_u32(a, w);
        }
        let got = snap_regions(&ee, &regions);
        if got != want {
            let mut off = Vec::new();
            let mut i = 0;
            for &(at, len) in &regions {
                for o in (0..len).step_by(4) {
                    if got[i] != want[i] {
                        off.push(format!("{:#x}: port {:#010x}, original {:#010x}", at + o, got[i], want[i]));
                    }
                    i += 1;
                }
            }
            panic!("record {k} ({n} contacts): {} words differ\n  {}", off.len(), off.iter().take(40).cloned().collect::<Vec<_>>().join("\n  "));
        }
        tally(&mut t, if n == 0 { "no contacts" } else if n == 14 { "the list full" } else { "some contacts" });
        if ee.u32_at(rec + 0x44) != 0 && n > 0 {
            tally(&mut t, "damped");
        }
        if (0..n).any(|i| ee.u32_at(list + record::CONTACT * i + 0x80) == u32::MAX) {
            tally(&mut t, "off any surface");
        }
        if before[..0x210 / 4] != want[..0x210 / 4] {
            tally(&mut t, "the record written");
        }
        let b0 = 0x210 / 4 + 14 * record::CONTACT as usize / 4;
        if before[b0 + 0x80 / 4..b0 + 0x90 / 4] != want[b0 + 0x80 / 4..b0 + 0x90 / 4] {
            tally(&mut t, "the velocity changed");
        }
        if before[b0 + 0x1d0 / 4..b0 + 0x1e0 / 4] != want[b0 + 0x1d0 / 4..b0 + 0x1e0 / 4] {
            tally(&mut t, "the rates changed");
        }
    }
    eprintln!("the record's resolution: {t:?}");
    for want in ["no contacts", "the list full", "some contacts", "damped", "off any surface", "the record written", "the velocity changed", "the rates changed"] {
        assert!(t.get(want).copied().unwrap_or(0) >= 50, "path {want:?} taken {} times: {t:?}", t.get(want).copied().unwrap_or(0));
    }
}

/// The run's calls a hooked original makes, as [`update::Out`]s, their fixed arguments checked;
/// with `ends`, the bike's trick end too (answering 1).
fn hook_run_calls(ee: &mut Ee, log: &Rc<RefCell<Vec<update::Out>>>, ends: bool) {
    use update::Out;
    let one = 1.0f32.to_bits();
    let l = log.clone();
    ee.hooks.insert(RUN_OPEN, Box::new(move |ee: &mut Ee| {
        l.borrow_mut().push(Out::RunOpen(ee.r(4) as u32));
        ee.set(2, 0);
    }));
    let l = log.clone();
    ee.hooks.insert(RUN_SCORE, Box::new(move |ee: &mut Ee| {
        assert_eq!(ee.r(7), 12, "the score's fourth argument");
        assert_eq!([13, 15, 16].map(|i| ee.f(i).to_bits()), [0, one, one], "the score's f13, f15, f16");
        let (run, player, id) = (ee.r(4) as u32, ee.r(5) as u32, ee.r(6) as u32);
        l.borrow_mut().push(Out::RunScore { run, player, id, scores: [ee.f(12).to_bits(), ee.f(14).to_bits()] });
        ee.set(2, 0);
    }));
    let l = log.clone();
    ee.hooks.insert(MARK_FN, Box::new(move |ee: &mut Ee| {
        assert_eq!(ee.r(8), 0, "the mark's fifth argument");
        l.borrow_mut().push(Out::Mark { run: ee.r(4) as u32, player: ee.r(5) as u32, on: ee.r(6) as u32, id: ee.r(7) as u32 });
        ee.set(2, 0);
    }));
    let l = log.clone();
    ee.hooks.insert(RUN_CLOSE, Box::new(move |ee: &mut Ee| {
        assert_eq!((ee.r(6), ee.f(12).to_bits()), (1, one), "the close's third argument and f12");
        l.borrow_mut().push(Out::RunClose { run: ee.r(4) as u32, player: ee.r(5) as u32 });
        ee.set(2, 0);
    }));
    let l = log.clone();
    ee.hooks.insert(RUN_TALLY, Box::new(move |ee: &mut Ee| {
        assert_eq!(ee.r(6), 1, "the tally's third argument");
        l.borrow_mut().push(Out::RunTally { run: ee.r(4) as u32, player: ee.r(5) as u32, count: ee.f(12).to_bits() });
        ee.set(2, 0);
    }));
    let l = log.clone();
    ee.hooks.insert(RUN_END, Box::new(move |ee: &mut Ee| {
        assert_eq!((ee.r(6), ee.r(8), ee.f(12).to_bits()), (1, 0, one), "the end's third and fifth arguments and f12");
        l.borrow_mut().push(Out::RunEnd { run: ee.r(4) as u32, player: ee.r(5) as u32, id: ee.r(7) as u32 });
        ee.set(2, 0);
    }));
    if ends {
        let l = log.clone();
        ee.hooks.insert(TRICK_END_FN, Box::new(move |ee: &mut Ee| {
            assert_eq!(ee.r(4) as u32, BIKE, "a trick end on another bike");
            l.borrow_mut().push(Out::TrickEnd(ee.r(5) as u32));
            ee.set(2, 1);
        }));
    }
}

/// [`update::stunt_start`] and [`update::trick_end`] on the bike in RAM, every call logged (a trick
/// end answering 1, the rest 0); a word set outside the bike fails.
struct RunLogged<'a> {
    ee: &'a mut Ee,
    log: Vec<update::Out>,
}

impl Updates for RunLogged<'_> {
    fn bike(&self) -> u32 {
        BIKE
    }
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn set(&mut self, at: u32, w: u32) {
        assert!((BIKE..BIKE + 0x1800).contains(&at), "a word set outside the bike: {at:#x}");
        self.ee.set_u32(at, w);
    }
    fn out(&mut self, c: update::Out) -> u32 {
        self.log.push(c);
        u32::from(matches!(c, update::Out::TrickEnd(_)))
    }
    fn vector(&mut self, c: update::Out) -> update::Quad {
        panic!("the stunt's start and end ask no vector of {c:?}")
    }
    fn noise(&mut self, _: F) -> F {
        panic!("the stunt's start and end ask no noise")
    }
}

/// **F31b5g — the stunt start is the original's**: 10 000 bikes, each state 4's with the start's
/// words drawn at random (off the ground or held or neither, the five kept stunts, the next stunt
/// round 41, the clip's clocks, the run and the player or none, the player's run flag, its list of
/// running stunts in scratch, the stunt table's scores and scale for the id), started on an id among
/// the kept five or the table's 46 (the game's are; past it the original reads through address 0)
/// with the third argument 0 or 1, the id handed sign-extended as a `lw` leaves it, through
/// SLUS-21104's vt `+0x4b8` and [`update::stunt_start`]. The answer, every word of the bike, the
/// player and the table comes out the same, nothing else is written, and the trick ends and the run's
/// calls (hooked to log) are made in the same order with the same arguments.
#[test]
#[allow(clippy::float_arithmetic)]
fn the_stunt_start_is_the_originals() {
    let Some(mut ee) = image() else { return };
    assert_eq!(vt(&ee, 0x4b8), STUNT_START_FN, "the bike's vt +0x4b8");
    assert_eq!(vt(&ee, 0x4ac), TRICK_END_FN, "the bike's vt +0x4ac");
    let (run0, player0, table) = (ee.u32_at(BIKE + 0x684), ee.u32_at(BIKE + 0x680), ee.u32_at(BIKE + 0x14f0));
    assert!(run0 != 0 && player0 != 0 && table != 0);
    let log: Rc<RefCell<Vec<update::Out>>> = Rc::default();
    hook_run_calls(&mut ee, &log, true);
    let nodes: Vec<u32> = (0..5).map(|k| ee.scratch(16 + k)).collect();
    let mut regions = vec![(BIKE, 0x1800), (player0, 0xd0), (table, 0x18 + 46 * 12)];
    let scored: Vec<u32> = (0..46).map(|i| ee.u32_at(table + 0x18 + 12 * i)).filter(|&e| e != 0).collect();
    regions.extend(scored.iter().map(|&e| (e + 0x50, 8)));
    regions.extend(nodes.iter().map(|&n| (n, 0x28)));
    let image_words = snap_regions(&ee, &regions);
    let kept0 = [0x13b8, 0x13bc, 0x13c0, 0x13c4, 0x13c8].map(|o| ee.u32_at(BIKE + o));
    let mut g = Gen(0x0031_b5a8);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        restore_regions(&mut ee, &regions, &image_words);
        let either = |g: &mut Gen| if g.one_in(2) { 0 } else if g.one_in(2) { 1 } else { g.bits() };
        let (air, held) = (either(&mut g), either(&mut g));
        ee.set_u32(BIKE + 0x324, air);
        ee.set_u32(BIKE + 0xc80, held);
        let mut kept = kept0;
        if g.one_in(4) {
            kept = kept.map(|x| if g.one_in(3) { g.bits() % 46 } else { x });
        }
        for (i, x) in kept.into_iter().enumerate() {
            ee.set_u32(BIKE + 0x13b8 + 4 * i as u32, x);
        }
        let id = if g.one_in(2) { kept[(g.bits() % 5) as usize] } else { g.bits() % 46 };
        let next = match g.bits() % 5 {
            0 => 40,
            1 => 41,
            2 => 0xff,
            3 => g.bits(),
            _ => g.bits() % 60,
        };
        ee.set_u32(BIKE + 0x1528, next);
        for o in [0x14d8, 0x14e8, 0x14dc, 0x1650] {
            let x = g.bits();
            ee.set_u32(BIKE + o, x);
        }
        let run = if g.one_in(6) { 0 } else { run0 };
        let player = if g.one_in(6) { 0 } else { player0 };
        ee.set_u32(BIKE + 0x684, run);
        ee.set_u32(BIKE + 0x680, player);
        let flags = g.bits();
        ee.set_u32(player0 + 8, flags);
        let n = g.bits() % 6;
        for i in 0..n as usize {
            let f = if g.one_in(5) { g.bits() & !0x10 } else { g.bits() | 0x10 };
            let s = if g.one_in(6) { kept[(g.bits() % 5) as usize] } else { g.bits() % 50 };
            let to = if i + 1 < n as usize { nodes[i + 1] } else { 0 };
            ee.set_u32(nodes[i], f);
            ee.set_u32(nodes[i] + 4, s);
            ee.set_u32(nodes[i] + 0x24, to);
        }
        ee.set_u32(player0 + 0xc8, if n > 0 { nodes[0] } else { 0 });
        if id < 46 {
            let e = table + 0x18 + 12 * id;
            let scale = match g.bits() % 4 {
                0 => 1.0,
                1 => -0.0,
                _ => g.f(-2.0, 3.0),
            };
            ee.set_f32(e + 4, scale);
            let def = ee.u32_at(e);
            if def != 0 {
                for o in [0x50, 0x54] {
                    let x = match g.bits() % 5 {
                        0 => 10.0,
                        1 => 0.0,
                        _ => g.f(-20.0, 60.0),
                    };
                    ee.set_f32(def + o, x);
                }
            }
        }
        let arg = g.bits() % 2;
        let before = snap_regions(&ee, &regions);
        log.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(STUNT_START_FN, &[u64::from(BIKE), id as i32 as i64 as u64, u64::from(arg)], &[], BUDGET);
        let answer = ee.r(2) as u32;
        let stray = written_outside(&ee, &regions);
        assert!(stray.is_empty(), "bike {k}: the original wrote outside the bike: {stray:?}");
        let want = snap_regions(&ee, &regions);
        let want_log = log.borrow().clone();
        restore_regions(&mut ee, &regions, &before);
        let mut m = RunLogged { ee: &mut ee, log: Vec::new() };
        let got_answer = update::stunt_start(&mut m, id, arg);
        let got_log = std::mem::take(&mut m.log);
        assert_eq!(got_answer, answer, "bike {k}: the answer");
        assert_eq!(got_log, want_log, "bike {k}: the calls (id {id:#x})");
        let got = snap_regions(&ee, &regions);
        if got != want {
            let mut off = Vec::new();
            let mut i = 0;
            for &(at, len) in &regions {
                for o in (0..len).step_by(4) {
                    if got[i] != want[i] {
                        off.push(format!("{:#x}: port {:#010x}, original {:#010x}", at + o, got[i], want[i]));
                    }
                    i += 1;
                }
            }
            panic!("bike {k} (id {id:#x}): {} words differ\n  {}", off.len(), off.join("\n  "));
        }
        // The paths.
        let is_kept = kept.contains(&id);
        if (air != 0 || held != 0) && !is_kept {
            tally(&mut t, "not started");
            continue;
        }
        tally(&mut t, if is_kept { "a kept stunt" } else { "a stunt" });
        let written = ee.u32_at(BIKE + 0x1528) == id && next != id;
        if is_kept {
            tally(&mut t, if written { "kept, the next written" } else { "kept, the next kept" });
        }
        if run == 0 || player == 0 {
            tally(&mut t, "no run or player");
            continue;
        }
        let ends = want_log.iter().filter(|c| matches!(c, update::Out::TrickEnd(_))).count();
        if ends > 0 {
            tally(&mut t, "running stunts ended");
        }
        if !is_kept && ends < n as usize {
            tally(&mut t, "the list stopped short");
        }
        tally(&mut t, if flags & 0x20 == 0 { "the run opened" } else { "the run open" });
        if let Some(update::Out::RunScore { scores, .. }) = want_log.iter().find(|c| matches!(c, update::Out::RunScore { .. })) {
            if scores.contains(&10.0f32.to_bits()) {
                tally(&mut t, "a score at the least");
            }
        }
    }
    eprintln!("the stunt start: {t:?}");
    for want in [
        "not started",
        "a kept stunt",
        "a stunt",
        "kept, the next written",
        "kept, the next kept",
        "no run or player",
        "running stunts ended",
        "the list stopped short",
        "the run opened",
        "the run open",
        "a score at the least",
    ] {
        assert!(t.get(want).copied().unwrap_or(0) >= 20, "path {want:?} taken {} times: {t:?}", t.get(want).copied().unwrap_or(0));
    }
}

/// **F31b5g — the trick end is the original's**: 10 000 bikes, each state 4's with the run and the
/// player or none, the player's flags and stunt count drawn at random, ending −1 or one stunt (the
/// id sign-extended: the original's −1 is a 64-bit compare), through SLUS-21104's vt `+0x4ac` and [`update::trick_end`]. The answer and the run's calls
/// (hooked to log) are the same, and neither writes a word.
#[test]
fn the_trick_end_is_the_originals() {
    let Some(mut ee) = image() else { return };
    assert_eq!(vt(&ee, 0x4ac), TRICK_END_FN, "the bike's vt +0x4ac");
    let (run0, player0) = (ee.u32_at(BIKE + 0x684), ee.u32_at(BIKE + 0x680));
    let log: Rc<RefCell<Vec<update::Out>>> = Rc::default();
    hook_run_calls(&mut ee, &log, false);
    let regions = [(BIKE, 0x1800), (player0, 0xd0)];
    let image_words = snap_regions(&ee, &regions);
    let mut g = Gen(0x0031_b5a9);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        restore_regions(&mut ee, &regions, &image_words);
        let run = if g.one_in(6) { 0 } else { run0 };
        let player = if g.one_in(6) { 0 } else { player0 };
        ee.set_u32(BIKE + 0x684, run);
        ee.set_u32(BIKE + 0x680, player);
        let flags = g.bits();
        ee.set_u32(player0 + 8, flags);
        let count = match g.bits() % 4 {
            0 => 0,
            1 => g.bits() % 10,
            2 => g.bits() | 0x8000_0000,
            _ => g.bits(),
        };
        ee.set_u32(player0 + 0xcc, count);
        let id = if g.one_in(2) { u32::MAX } else { g.bits() % 60 };
        log.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(TRICK_END_FN, &[u64::from(BIKE), id as i32 as i64 as u64], &[], BUDGET);
        let answer = ee.r(2) as u32;
        let stray = written_outside(&ee, &[]);
        assert!(stray.is_empty(), "bike {k}: the original wrote {stray:?}");
        let want_log = log.borrow().clone();
        let mut m = RunLogged { ee: &mut ee, log: Vec::new() };
        let got_answer = update::trick_end(&mut m, id);
        let got_log = std::mem::take(&mut m.log);
        assert_eq!((got_answer, got_log), (answer, want_log.clone()), "bike {k} (id {id:#x})");
        tally(&mut t, match want_log.first() {
            None if run == 0 || player == 0 => "no run or player",
            None => "the run not open",
            Some(update::Out::RunClose { .. }) => "closed and tallied",
            Some(update::Out::RunEnd { .. }) => "one ended",
            Some(c) => panic!("{c:?}"),
        });
    }
    eprintln!("the trick end: {t:?}");
    for want in ["no run or player", "the run not open", "closed and tallied", "one ended"] {
        assert!(t.get(want).copied().unwrap_or(0) >= 100, "path {want:?} taken {} times: {t:?}", t.get(want).copied().unwrap_or(0));
    }
}

// F31b5c — the tail's, wreck's and update's callees left in the interpreter, watched.

/// One interpreter callee of [`TailInRam`], [`WreckInRam`] or [`UpdateInRam`] over a replay: how
/// often it ran, on how many rows, and each word of the bike's physics blocks ([`physics`]) and of
/// the bike object it changed, with the rows it changed them on.
#[derive(Default, Debug)]
struct Callee {
    calls: u32,
    rows: std::collections::BTreeSet<usize>,
    blocks: BTreeMap<String, std::collections::BTreeSet<usize>>,
    bike: BTreeMap<u32, std::collections::BTreeSet<usize>>,
}

/// The census of one replay, while it is taken: the row, the label of the call being made, and the
/// callees by label.
#[derive(Default)]
struct Census {
    row: usize,
    label: String,
    callees: BTreeMap<String, Callee>,
}

thread_local! {
    static CENSUS: RefCell<Option<Census>> = const { RefCell::new(None) };
}

/// Names the interpreter calls that follow, by their `Out`'s variant (its arguments dropped).
fn label(l: &str) {
    CENSUS.with(|c| {
        if let Some(c) = c.borrow_mut().as_mut() {
            c.label = l.split(['(', ' ', '{']).take(2).collect::<Vec<_>>().join(" ");
        }
    });
}

/// Runs an interpreter call; while a census is taken, logs what it changed of the bike's physics
/// blocks and the bike object under the current label.
fn census(ee: &mut Ee, call: impl FnOnce(&mut Ee)) {
    if CENSUS.with(|c| c.borrow().is_none()) {
        return call(ee);
    }
    let blocks = physics(ee);
    let snap = |ee: &Ee| -> Vec<u32> {
        let mut w: Vec<u32> = (0..0x1800).step_by(4).map(|o| ee.u32_at(BIKE + o)).collect();
        for &(_, at, n) in &blocks {
            w.extend((0..n).step_by(4).map(|o| ee.u32_at(at + o)));
        }
        w
    };
    let before = snap(ee);
    call(ee);
    let after = snap(ee);
    CENSUS.with(|c| {
        let mut c = c.borrow_mut();
        let c = c.as_mut().unwrap();
        let row = c.row;
        let e = c.callees.entry(c.label.clone()).or_default();
        e.calls += 1;
        e.rows.insert(row);
        for (k, (b, a)) in before.iter().zip(&after).enumerate() {
            if b == a {
                continue;
            }
            if k < 0x600 {
                e.bike.entry(4 * k as u32).or_default().insert(row);
            } else {
                let mut i = k - 0x600;
                for &(name, _, n) in &blocks {
                    if i < n as usize / 4 {
                        e.blocks.entry(format!("{name}+{:#x}", 4 * i)).or_default().insert(row);
                        break;
                    }
                    i -= n as usize / 4;
                }
            }
        }
    });
}

/// [`replay_steps`] with the census taken: answers its callees by label.
fn census_of(name: &str) -> Option<BTreeMap<String, Callee>> {
    let words = mass_words(&race250p()?)?;
    let trn = nat05_trn()?;
    let map = track::Terrain::parse(&trn).unwrap();
    let mut ee = hooked(image()?);
    for (a, mask, v) in words {
        ee.set_u32(a, ee.u32_at(a) & !mask | v & mask);
    }
    let mut noise = None;
    let rows = rows(name)?;
    CENSUS.with(|c| *c.borrow_mut() = Some(Census::default()));
    for (r, row) in rows.iter().enumerate() {
        CENSUS.with(|c| c.borrow_mut().as_mut().unwrap().row = r);
        poke(&mut ee, row, &READER);
        steps_frame(&mut ee, F(f32::from_bits(DT)), &map, &mut noise);
        let d = diff(&ee, row);
        assert!(d.is_empty(), "{name} row {r}: {} words differ\n  {}", d.len(), d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  "));
        poke(&mut ee, row, &SPOT);
    }
    let c = CENSUS.with(|c| c.borrow_mut().take()).unwrap();
    Some(c.callees)
}

fn show(name: &str, callees: &BTreeMap<String, Callee>) {
    for (l, c) in callees {
        let rows = |s: &std::collections::BTreeSet<usize>| format!("{} rows {:?}..{:?}", s.len(), s.first(), s.last());
        eprintln!("{name} {l}: {} calls, {}", c.calls, rows(&c.rows));
        for (w, s) in &c.blocks {
            eprintln!("    block {w}: {}", rows(s));
        }
        for (w, s) in &c.bike {
            eprintln!("    bike +{w:#x}: {}", rows(s));
        }
    }
}

/// A random float for the resets' oracle: zeros of either sign, ones, tiny, huge and the PS2's
/// largest among the plain ones.
fn reset_float(g: &mut Gen) -> u32 {
    match g.bits() % 8 {
        0 => 0,
        1 => 0x8000_0000,
        2 => 0x3f80_0000,
        3 => g.bits() & 0x807f_ffff,
        4 => 0x7f7f_ffff | (g.bits() & 0x8000_0000),
        5 => g.bits(),
        _ => g.f(-50.0, 50.0).to_bits(),
    }
}

/// **F31b5c — the engine's and a wheel's resets are the originals**: 10 000 of each on state 4's
/// engine and wheels, every word they read drawn at random (the idle revs, the gear count about 1 and
/// signed, the final drive and the gear table's ratios with zeros, tiny and huge ones; every word of
/// the wheel), run through SLUS-21104's resets and through [`wreck::engine_reset`] and
/// [`wreck::wheel_reset`] from the same words. The engine, its table and the wheel come out word for
/// word.
#[test]
fn the_engine_and_wheel_resets_are_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    let engine = ee.u32_at(BIKE + 0x142c);
    let wheels = the_wheels(&ee);
    let table = WRECK_AREA;
    let regions = [(engine, 0x100), (table, 0x40), (wheels[0], 0x920), (wheels[1], 0x920)];
    let image_words = snap_regions(&ee, &regions);
    let mut g = Gen(0x0031_b5c0);
    let mut t = BTreeMap::new();
    for k in 0..20_000 {
        restore_regions(&mut ee, &regions, &image_words);
        let (f, at, path) = if k % 2 == 0 {
            for o in [0x54, 0xa0, 0xa4, 0xa8, 0x5c, 0x74, 0x78] {
                ee.set_u32(engine + o, g.bits());
            }
            ee.set_u32(engine + 0x34, reset_float(&mut g));
            let gears = if g.one_in(5) { [0, -1, i32::MIN, 1][(g.bits() % 4) as usize] } else { (g.bits() % 7) as i32 };
            ee.set_u32(engine + 0x6c, gears as u32);
            ee.set_u32(engine + 0x70, table);
            ee.set_u32(table, reset_float(&mut g));
            (ENGINE_RESET, engine, if gears >= 1 { "engine, in first" } else { "engine, no gears" })
        } else {
            let wheel = wheels[(g.bits() % 2) as usize];
            for o in (0..0x920).step_by(4) {
                ee.set_u32(wheel + o, g.bits());
            }
            (WHEEL_RESET, wheel, "wheel")
        };
        tally(&mut t, path);
        let before = snap_regions(&ee, &regions);
        ee.set(28, GP);
        ee.call(f, &[u64::from(at)], &[0.0], BUDGET);
        let want = snap_regions(&ee, &regions);
        restore_regions(&mut ee, &regions, &before);
        let mut m = WreckInRam { ee: &mut ee, x: 0.0 };
        if f == ENGINE_RESET {
            wreck::engine_reset(&mut m, at);
        } else {
            wreck::wheel_reset(&mut m, at);
        }
        let got = snap_regions(&ee, &regions);
        let off: Vec<String> = (0..want.len()).filter(|&i| want[i] != got[i]).map(|i| format!("word {i}: original {:#010x}, port {:#010x}", want[i], got[i])).collect();
        assert!(off.is_empty(), "case {k} ({path}): {}", off.join("\n  "));
    }
    eprintln!("{t:?}");
    assert!(t.values().all(|&n| n >= 1000), "{t:?}");
}

/// The interpreter callees the census allows to change the bike's physics blocks: none.
fn census_writers(c: &BTreeMap<String, Callee>) -> Vec<(&str, usize, Vec<&String>)> {
    c.iter().filter(|(_, e)| !e.blocks.is_empty()).map(|(l, e)| (l.as_str(), e.blocks.len(), e.blocks.keys().take(4).collect())).collect()
}

/// **F31b5c — C15's ride, the tail's, wreck's and update's callees watched**: on every row of
/// ride-2, each callee the port's vehicle update, wreck and update callees still hand the
/// interpreter (the pause lookup, the rumble, the frame message, the out-of-bounds test, the
/// rider's animation and nodes, the bike's net packets, the walk; F44 made the parts' and sender's
/// virtuals Rust) leaves every word of the bike's physics blocks as it found it.
#[test]
fn the_rides_tail_callees_leave_the_physics_alone() {
    let Some(c) = census_of("ride-2") else { return };
    show("ride-2", &c);
    assert_eq!(census_writers(&c), [], "callees that wrote the physics blocks");
    assert_eq!(c.keys().map(String::as_str).collect::<Vec<_>>(), RIDE_CALLEES, "the callees left in the interpreter");
}

/// **F31b5c — C15's flip, watched the same**: on every row of flip-1, wreck and put-back among
/// them, the callees left in the interpreter leave the physics blocks alone. F31b5e: with the
/// put-back's ground reset Rust ([`wreck::ground`]: its orient, the body's and the node's calls, the
/// spatial update and the part syncs), none of them is left a writer; what the reset still hands on
/// (the unhooks, the parts on and attached) changes no word of them.
#[test]
fn the_flips_tail_callees_leave_the_physics_alone() {
    let Some(c) = census_of("flip-1") else { return };
    show("flip-1", &c);
    assert_eq!(census_writers(&c), [], "callees that wrote the physics blocks");
    for l in ["wreck Unhook", "wreck PartOn", "wreck Attach"] {
        assert!(c[l].rows.contains(&193), "{l} runs on the put-back's row");
    }
    assert_eq!(c.keys().map(String::as_str).collect::<Vec<_>>(), FLIP_CALLEES, "the callees left in the interpreter");
}

/// The callees ride-2 leaves in the interpreter, by label ([`label`]). F43b: the attachment move is
/// the port's, the heap it allocates from the original's (`heap Moved`). F45: the trick wanted is
/// the port's. F31b5j: the update's walk is the port's, its leaves watched by [`PartWalk`] itself.
/// F31b5k: the connection's queue is the port's.
const RIDE_CALLEES: [&str; 8] = [
    "heap Moved",
    "tail Message",
    "tail Paused",
    "tail Rumble",
    "tail RumbleReady",
    "update IrqOff",
    "update IrqOn",
    "wreck OutOfBounds",
];
/// The callees flip-1 leaves in the interpreter: ride-2's, the trick's, the wreck's and the put-back's.
/// F45: the stunt mark, trick end and trick wanted are the ports'.
const FLIP_CALLEES: [&str; 24] = [
    "heap Moved",
    "tail Message",
    "tail Paused",
    "tail Rumble",
    "tail RumbleReady",
    "update IrqOff",
    "update IrqOn",
    "update KindClear",
    "update Search",
    "wreck Attach",
    "wreck Camera",
    "wreck Kind",
    "wreck OutOfBounds",
    "wreck PartOff",
    "wreck PartOn",
    "wreck Place",
    "wreck Pose",
    "wreck Post",
    "wreck Rider",
    "wreck Rumble",
    "wreck RumbleReady",
    "wreck Stance",
    "wreck Trail",
    "wreck Unhook",
];

// ---------------------------------------------------------------------------------------------
// F31b5d: every other listener on state 4's sim shown inert on both replays
// ---------------------------------------------------------------------------------------------

/// The preforce list's thunk (its elements' vt `+8`), the bike's and the five riders' alike: it hands
/// its element's owner to the owner's PreProcess.
const PREFORCE_THUNK: u32 = 0x004d_e0c0;
/// The force list's code (its element's `+8`'s `+0xc`) and the post list's (its element's vt `+8`),
/// the bike's on state 4.
const FORCE_THUNK: u32 = 0x0049_bd50;
const POST_THUNK: u32 = 0x004d_e0d0;

/// A listener of state 4's sim that is not the bike's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Other {
    /// One of the five other riders' PreProcess, by its place among them on the preforce list.
    PreProcess(u32),
    /// The tail's first other leaf.
    Leaf,
    /// The tail's second, the manager: its own work and walk but its rider's step.
    Manager,
    /// The manager's rider, whose target is the bike's record.
    Rider,
}

/// How [`listeners_flow`] runs the other listeners.
#[derive(Clone, Copy, PartialEq)]
enum Others {
    /// As themselves, watched: their writes marked, and every read of a marked byte by the rest of the
    /// frame noted ([`Flow`]).
    Watched,
    /// As themselves, unhooked.
    Run,
    /// Not at all: their hooks return at once. The rider's one word the frame reads ([`wreck::SETTLES`])
    /// is fed from the capture instead, row r's into frame r + 1, as the put-back spot is.
    Dropped,
}

/// What [`listeners_flow`] saw over a replay.
#[derive(Debug, Default)]
struct Flow {
    /// Each other listener's runs.
    ran: BTreeMap<Other, usize>,
    /// Every byte an other listener wrote that nothing else has written since, and who wrote it.
    marked: BTreeMap<u32, Other>,
    /// The bytes marked during the current outermost other listener by one nested in it.
    nested: std::collections::BTreeSet<u32>,
    /// How deep in other listeners the interpreter is.
    depth: u32,
    /// Every read of a marked byte by anything but an other listener: (row, byte, its writer, the
    /// stretch of the frame that read it).
    read: Vec<(usize, u32, Other, &'static str)>,
    /// The stretch of the frame running: one of the bike's listeners, or the rest of the frame.
    stage: &'static str,
    /// Every byte of the bike object or its physics blocks an other listener wrote outside the windows
    /// it is known to write ([`OTHER_LEAVES`]): (row, byte, the listener).
    wrote_bike: Vec<(usize, u32, Other)>,
    /// The bike object's and its blocks' ranges, this row's.
    bike: Vec<std::ops::Range<u32>>,
    /// The elements the `+0x24` and `+0x3c` lists' getters answered, this frame.
    got24: Vec<u32>,
    got3c: Vec<u32>,
    /// The row being run.
    row: usize,
    /// The bike object and its blocks after every frame ([`bike_and_blocks`]).
    after: Vec<Vec<(u32, u32)>>,
}

impl Flow {
    /// The reads and writes since the last settle: reads of marked bytes are the frame depending on
    /// an other listener; marked bytes written are the frame's again. A byte written and then read
    /// in one stretch counts as read too: the reads are a set, so this errs towards reporting.
    fn settle(&mut self, ee: &mut Ee) {
        let reads = ee.reads.replace(Some(Default::default())).unwrap_or_default();
        for a in reads {
            if let Some(&w) = self.marked.get(&a) {
                self.read.push((self.row, a, w, self.stage));
            }
        }
        for a in std::mem::take(&mut ee.written) {
            self.marked.remove(&a);
        }
    }
}

/// Runs the hooked function `f` as itself, from inside its hook, with the arguments the hook was
/// reached with.
fn run_as_itself(ee: &mut Ee, f: u32) {
    let (args, x, ra, sp, stack) = ([ee.r(4), ee.r(5), ee.r(6), ee.r(7)], ee.f(12), ee.r(31), ee.r(29), ee.stack);
    ee.stack = (sp as u32 - 0x40) & !0xf;
    ee.call(f, &args, &[x], BUDGET);
    // The interpreter files the hook back under the `pc` it leaves, so leave it here.
    (ee.stack, ee.pc) = (stack, f);
    ee.set(29, sp);
    ee.set(31, ra);
}

/// A part of the frame's own code hooked to look at: it runs as itself, its writes kept in the
/// frame's.
fn run_as_frame(ee: &mut Ee, f: u32) {
    let before = std::mem::take(&mut ee.written);
    run_as_itself(ee, f);
    ee.written.extend(before);
}

/// One of the bike's listeners, its reads told apart as `stage`'s.
fn run_as_stage(ee: &mut Ee, f: u32, stage: &'static str, flow: &RefCell<Flow>) {
    let outer = {
        let mut t = flow.borrow_mut();
        t.settle(ee);
        std::mem::replace(&mut t.stage, stage)
    };
    run_as_itself(ee, f);
    let mut t = flow.borrow_mut();
    t.settle(ee);
    t.stage = outer;
}

/// An other listener, watched: the frame's reads and writes so far settled, then it runs as itself,
/// its reads its own, its writes marked (those into the bike outside its windows noted). The
/// interpreter's stack is left unmarked: a frame on it is written before it is read.
fn run_as_other(ee: &mut Ee, f: u32, who: Other, flow: &RefCell<Flow>) {
    if flow.borrow().depth == 0 {
        flow.borrow_mut().settle(ee);
    }
    let before = std::mem::take(&mut ee.written);
    flow.borrow_mut().depth += 1;
    run_as_itself(ee, f);
    let t = &mut *flow.borrow_mut();
    t.depth -= 1;
    let wrote = std::mem::replace(&mut ee.written, before);
    let windows: &[std::ops::Range<u32>] = if who == Other::Rider { OTHER_LEAVES[2].2 } else { &[] };
    let stack = (ee.stack - STACK_USE as u32)..(ee.stack + 0x400);
    let wrote: Vec<u32> = wrote.into_iter().filter(|a| !stack.contains(a)).collect();
    for &a in &wrote {
        if t.bike.iter().any(|r| r.contains(&a)) && !t.nested.contains(&a) && !windows.iter().any(|r| r.contains(&a.wrapping_sub(BIKE))) {
            t.wrote_bike.push((t.row, a, who));
        }
    }
    if t.depth == 0 {
        // Its reads are its own; the bytes a nested listener wrote stay that one's.
        ee.reads.replace(Some(Default::default()));
        for a in wrote {
            if !t.nested.contains(&a) {
                t.marked.insert(a, who);
            }
        }
        t.nested.clear();
    } else {
        for a in wrote {
            t.marked.insert(a, who);
            t.nested.insert(a);
            ee.written.insert(a);
        }
    }
}

/// C15's `name` replayed whole in the interpreter as [`replay`] does, every row's blocks bit-exact
/// against the capture, the other listeners of state 4's sim run as `others` says: the five other
/// riders' PreProcess (the preforce thunk on any element but the bike's) and the tail's other leaves
/// ([`OTHER_LEAVES`]). On every frame the world's lists are the bike's body, its PreProcess and the
/// riders', its Force and its PostProcess alone, before and after; the `+0x24` list's getter hands
/// the frame the bike's record alone, once a pass, and the `+0x3c` list's nothing.
fn listeners_flow(name: &str, others: Others) -> Option<Flow> {
    let mut ee = hooked(image()?);
    let rows = rows(name)?;
    let mine = ee.u32_at(BIKE + seed::RECORD);
    let lists = |ee: &Ee| -> Vec<Vec<u32>> {
        [List::Bodies, List::Preforce, List::Force, List::Constraints, List::Post]
            .into_iter()
            .map(|l| {
                let (n, arr) = list(ee, l);
                (0..n).map(|i| ee.u32_at(arr + 4 * i)).collect()
            })
            .collect()
    };
    let want = lists(&ee);
    let riders: Vec<u32> = want[1].iter().copied().filter(|&e| e != BIKE_PREFORCE).collect();
    assert_eq!(want[0], [ee.u32_at(BIKE + 0xb8)], "{name}: one body, the bike's");
    assert_eq!((want[1].len(), riders.len()), (6, 5), "{name}: the bike's PreProcess and five riders': {:#x?}", want[1]);
    assert!(want[1].iter().all(|&e| ee.u32_at(ee.u32_at(e) + 8) == PREFORCE_THUNK), "{name}: the preforce list's code is the one thunk");
    assert_eq!(want[2].len(), 1, "{name}: one Force");
    assert_eq!((ee.u32_at(want[2][0] + 0xc), ee.u32_at(ee.u32_at(want[2][0] + 8) + 0xc)), (BIKE_FORCE, FORCE_THUNK), "{name}: the Force is the bike's");
    assert!(want[3].is_empty(), "{name}: no constraints");
    assert_eq!(want[4], [BIKE_POST], "{name}: one PostProcess, the bike's");
    assert_eq!(ee.u32_at(ee.u32_at(BIKE_POST) + 8), POST_THUNK);
    let t: Rc<RefCell<Flow>> = Rc::default();
    t.borrow_mut().stage = "frame";
    if others != Others::Run {
        let h = t.clone();
        ee.hooks.insert(
            PREFORCE_THUNK,
            Box::new(move |ee: &mut Ee| {
                let e = ee.r(4) as u32;
                if e == BIKE_PREFORCE {
                    return run_as_stage(ee, PREFORCE_THUNK, "PreProcess", &h);
                }
                let k = riders.iter().position(|&r| r == e).unwrap_or_else(|| panic!("a preforce element not on the list: {e:#x}"));
                let who = Other::PreProcess(k as u32);
                *h.borrow_mut().ran.entry(who).or_default() += 1;
                if others == Others::Watched {
                    run_as_other(ee, PREFORCE_THUNK, who, &h);
                }
            }),
        );
        for ((e, step, _), who) in OTHER_LEAVES.into_iter().zip([Other::Leaf, Other::Manager, Other::Rider]) {
            let h = t.clone();
            ee.hooks.insert(
                step,
                Box::new(move |ee: &mut Ee| {
                    assert_eq!(ee.r(4) as u32, e, "{who:?}'s step's listener");
                    *h.borrow_mut().ran.entry(who).or_default() += 1;
                    if others == Others::Watched {
                        run_as_other(ee, step, who, &h);
                    }
                }),
            );
        }
        for (f, stage) in [(RECORD_CLEAR, "record clear"), (RECORD_PASS, "record pass"), (FORCE_THUNK, "Force"), (POST_THUNK, "PostProcess"), (VEHICLE_THUNK, "vehicle update")] {
            let h = t.clone();
            ee.hooks.insert(f, Box::new(move |ee: &mut Ee| run_as_stage(ee, f, stage, &h)));
        }
    }
    for (get, three_c) in [(LIST24_GET, false), (LIST3C_GET, true)] {
        let h = t.clone();
        ee.hooks.insert(
            get,
            Box::new(move |ee: &mut Ee| {
                run_as_frame(ee, get);
                let mut t = h.borrow_mut();
                let e = ee.r(2) as u32;
                if three_c { t.got3c.push(e) } else { t.got24.push(e) }
            }),
        );
    }
    for (r, row) in rows.iter().enumerate() {
        poke(&mut ee, row, &READER);
        {
            let mut t = t.borrow_mut();
            t.row = r;
            t.bike = std::iter::once(BIKE..BIKE + 0x1800).chain(physics(&ee).into_iter().map(|(_, at, n)| at..at + n)).collect();
            t.got24.clear();
            t.got3c.clear();
        }
        assert_eq!(lists(&ee), want, "{name} row {r}: the world's lists before the frame");
        ee.written.clear();
        if others == Others::Watched {
            *ee.reads.borrow_mut() = Some(Default::default());
        }
        ee.set(28, GP);
        ee.call(FRAME, &[u64::from(SIM)], &[f32::from_bits(DT)], BUDGET);
        if others == Others::Watched {
            let mut t = t.borrow_mut();
            t.settle(&mut ee);
            assert_eq!(t.stage, "frame");
            *ee.reads.borrow_mut() = None;
        }
        assert_eq!(lists(&ee), want, "{name} row {r}: the world's lists after the frame");
        {
            let mut t = t.borrow_mut();
            assert_eq!(t.got24, [mine, mine], "{name} row {r}: the +0x24 list is the bike's record alone, for each of its two passes");
            assert!(t.got3c.is_empty(), "{name} row {r}: the +0x3c list hands the frame nothing: {:#x?}", t.got3c);
            assert_eq!(ee.u32_at(SIM + 0x3c), 0, "{name} row {r}: the +0x3c list is empty");
            t.after.push(bike_and_blocks(&ee));
        }
        let d = diff(&ee, row);
        assert!(d.is_empty(), "{name} row {r} (vsync {}): {} words differ\n  {}", row["vsync"], d.len(), d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  "));
        assert_eq!(wrecked(|o| ee.u32_at(BIKE + o)), row_wrecked(row), "{name} row {r}: the wreck state");
        poke(&mut ee, row, &SPOT);
        if others == Others::Dropped {
            poke(&mut ee, row, &[(wreck::SETTLES, 4)]);
        }
    }
    drop(ee);
    Some(Rc::try_unwrap(t).ok().unwrap().into_inner())
}

/// **F31b5d — of the other listeners, only the tail's rider writes anything the bike's frame reads**:
/// C15 ride-2 and flip-1 replayed whole in the interpreter, every row bit-exact, with each listener of
/// state 4's sim that is not the bike's watched ([`listeners_flow`]): the five other riders'
/// PreProcess, the tail's first other leaf, its manager and the manager's rider, each once a frame;
/// the `+0x24` list the bike's record alone and the `+0x3c` list empty on every frame, the world's
/// force and post lists the bike's alone. None of them writes a byte of the bike object or its physics
/// blocks outside the rider's two windows ([`OTHER_LEAVES`]); and of every byte they write, the only
/// ones the rest of the frame (or a later one) reads before writing them itself are the rider's. The
/// reads are a set per stretch of the frame, so a byte the frame writes and then reads in one stretch
/// counts as read: the rider's list errs long. That the frame needs none of them but the rider's
/// `+0x920` is [`the_bikes_frame_without_the_other_listeners_is_bit_exact`]'s.
#[test]
fn only_the_riders_writes_are_read_by_the_bikes_frame() {
    for (name, n) in [("ride-2", 459), ("flip-1", 465)] {
        let Some(t) = listeners_flow(name, Others::Watched) else { return };
        assert_eq!(t.after.len(), n);
        let every: Vec<Other> = (0..5).map(Other::PreProcess).chain([Other::Leaf, Other::Manager, Other::Rider]).collect();
        assert_eq!(t.ran.keys().copied().collect::<Vec<_>>(), every, "{name}: the other listeners");
        assert!(t.ran.values().all(|&k| k == n), "{name}: each once a frame: {:?}", t.ran);
        assert!(t.wrote_bike.is_empty(), "{name}: {} bytes of the bike written by the others, the first {:x?}", t.wrote_bike.len(), &t.wrote_bike[..t.wrote_bike.len().min(20)]);
        let mut by: BTreeMap<(Other, &str), (std::collections::BTreeSet<u32>, usize, usize)> = BTreeMap::new();
        for &(r, a, w, st) in &t.read {
            let e = by.entry((w, st)).or_insert((Default::default(), r, r));
            e.0.insert(a & !3);
            (e.1, e.2) = (e.1.min(r), e.2.max(r));
        }
        for ((w, st), (words, f, l)) in &by {
            eprintln!("{name}: {w:?}'s writes read by {st}: {} words, {:#x}..={:#x}, rows {f}..={l}", words.len(), words.first().unwrap(), words.last().unwrap());
        }
        let writers: std::collections::BTreeSet<Other> = t.read.iter().map(|&(_, _, w, _)| w).collect();
        assert_eq!(writers.into_iter().collect::<Vec<_>>(), [Other::Rider], "{name}: whose writes the bike's frame reads");
    }
}

/// **F31b5d — the bike's frame without the other listeners is bit-exact**: C15 ride-2 and flip-1
/// replayed whole in the interpreter twice, once with every other listener of state 4's sim run and
/// once with none of them run at all ([`Others::Dropped`]: the five other riders' PreProcess and the
/// tail's first two other leaves return at once, the second's walk and so the third, the rider, with
/// it; the `+0x24` and `+0x3c` lists and the world's force and
/// post lists hold nothing else on any frame, [`listeners_flow`]'s checks). The one word of the rider's
/// the frame needs, its "a ragdoll part is touching" flag at the bike's [`wreck::SETTLES`] (F41), is
/// fed from the capture as the put-back spot is. Every row's body, node, record, wheels and engine are
/// C15's word for word, the wreck at row 125 and the put-back at 193; and after every frame the whole
/// bike object and its physics blocks are the full frame's, but for the rider's own windows of the
/// record, exactly the words of them the rider writes ([`OTHER_LEAVES`]; `+0x920`, fed, a frame
/// late). So the pure frame steps the bike alone.
#[test]
fn the_bikes_frame_without_the_other_listeners_is_bit_exact() {
    for (name, n) in [("ride-2", 459), ("flip-1", 465)] {
        let (Some(run), Some(dropped)) = (listeners_flow(name, Others::Run), listeners_flow(name, Others::Dropped)) else { return };
        assert_eq!((run.after.len(), dropped.after.len()), (n, n));
        // The rider is stepped by the manager's own walk, so it goes with it.
        assert!(dropped.ran.values().all(|&k| k == n) && dropped.ran.len() == 7 && !dropped.ran.contains_key(&Other::Rider), "{name}: each other listener dropped once a frame: {:?}", dropped.ran);
        let windows = OTHER_LEAVES[2].2;
        let mut parted = std::collections::BTreeSet::new();
        for (r, (a, b)) in run.after.iter().zip(&dropped.after).enumerate() {
            assert_eq!(a.len(), b.len(), "{name} row {r}: the same blocks");
            for (&(at, x), &(at2, y)) in a.iter().zip(b) {
                assert_eq!(at, at2, "{name} row {r}: the same blocks");
                if x != y {
                    let o = at.wrapping_sub(BIKE);
                    assert!(o < 0x1800 && windows.iter().any(|w| w.contains(&o)), "{name} row {r}: {at:#x} (bike+{o:#x}) {x:#010x} with the others, {y:#010x} without");
                    parted.insert(o);
                }
            }
        }
        eprintln!("{name}: without the others, the bike's words that part from the full frame's: {parted:#x?}");
        let mut want: Vec<u32> = vec![0x930, 0x934, 0x938, 0x940];
        if name == "flip-1" {
            want = (0x870..0x88c).step_by(4).filter(|&o| o != 0x87c).chain([0x890, 0x894, 0x898, 0x920]).chain(want).chain((0x950..0x980).step_by(4).filter(|&o| o != 0x95c)).collect();
        }
        assert_eq!(parted.into_iter().collect::<Vec<_>>(), want, "{name}: the words that part are the rider's");
    }
}

// ---------------------------------------------------------------------------------------------
// F31b5f: the bike words the tail's, wreck's and update's interpreter callees write
// ---------------------------------------------------------------------------------------------

/// **F31b5f — the clip held at its end is the original's**: 10 000 cases on state 4's bike, the hold's
/// four clocks (`+0x14d8`, `+0x14dc`, `+0x14e0`, `+0x14e8`), the clip's run and `dt` drawn at random
/// (the hold clock mostly about its half-second ramp, either side of it and inside, the rest zeros,
/// tiny, huge and any bits), run through SLUS-21104's vt `+0x4ec` and through [`update::trick_hold`]
/// from the same words. The bike's words, the clip and the noise's knot come out word for word, the
/// noise on the original's tables.
#[test]
fn the_trick_hold_is_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    assert_eq!(vt(&ee, 0x4ec), TRICK_HOLD_FN, "the bike's vt +0x4ec");
    let (entry, clip) = (WRECK_AREA, WRECK_AREA + 0x40);
    let regions = [(BIKE + 0x14d0, 0x20), (entry, 0x100), (NOISE_KNOT, 4)];
    let image_words = snap_regions(&ee, &regions);
    let mut g = Gen(0x0031_b5f0);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        restore_regions(&mut ee, &regions, &image_words);
        ee.set_u32(entry, clip);
        let hold = if g.one_in(4) { reset_float(&mut g) } else { g.f(-0.3, 0.8).to_bits() };
        ee.set_u32(BIKE + 0x14dc, hold);
        for o in [0x14d8, 0x14e0, 0x14e4, 0x14e8] {
            ee.set_u32(BIKE + o, if g.one_in(3) { reset_float(&mut g) } else { g.f(-5.0, 5.0).to_bits() });
        }
        ee.set_u32(clip + 0x48, if g.one_in(4) { reset_float(&mut g) } else { g.f(0.05, 3.0).to_bits() });
        let dt = if g.one_in(5) { f32::from_bits(reset_float(&mut g)) } else { f32::from_bits(DT) };
        let h = F(f32::from_bits(hold));
        tally(&mut t, if !(h > F::ZERO) { "hold at or under 0" } else if h < F(0.5) { "hold on the ramp" } else { "hold past the ramp" });
        let before = snap_regions(&ee, &regions);
        ee.set(28, GP);
        ee.call(TRICK_HOLD_FN, &[u64::from(BIKE), u64::from(entry)], &[dt], BUDGET);
        let want = snap_regions(&ee, &regions);
        restore_regions(&mut ee, &regions, &before);
        let mut m = UpdateInRam { ee: &mut ee, x: dt };
        update::trick_hold(&mut m, entry, F(dt));
        let got = snap_regions(&ee, &regions);
        let off: Vec<String> = (0..want.len()).filter(|&i| want[i] != got[i]).map(|i| format!("word {i}: original {:#010x}, port {:#010x}", want[i], got[i])).collect();
        assert!(off.is_empty(), "case {k} (hold {hold:#010x}, dt {dt}): {}", off.join("\n  "));
    }
    eprintln!("{t:?}");
    assert!(t.values().all(|&n| n >= 1000), "{t:?}");
}

/// **F31b5f — the settle is the original's**: 10 000 cases on state 4's bike, the run and the player
/// (`+0x684`, `+0x680`) each there or not, the player's listener list 0 to 4 long, the six clocks'
/// words and the flip's two drawn at random, run through SLUS-21104's vt `+0x4a8` and through
/// [`wreck::settle`] from the same words. The stance's call (vt `+0x4b0`) and each listener's
/// (its vt `+0xa0`: the listener, 5, a zero word, 0, 0) are logged on both sides, the same calls in
/// the same order; the bike's words come out word for word.
#[test]
fn the_settle_is_the_originals() {
    use wreck::Out;
    let Some(mut ee) = image().map(hooked) else { return };
    assert_eq!((vt(&ee, 0x4a8), vt(&ee, 0x53c), vt(&ee, 0x540)), (SETTLE_FN, FLIP_CLIP_SET, FLIP_NEXT_SET), "the bike's vt +0x4a8, +0x53c and +0x540");
    let (player, listeners, fake_vt, fake_fn) = (WRECK_AREA + 0x400, WRECK_AREA + 0x600, WRECK_AREA + 0x800, WRECK_AREA + 0x900);
    let log: Rc<RefCell<Vec<Out>>> = Rc::default();
    let l = log.clone();
    ee.hooks.insert(vt(&ee, 0x4b0), Box::new(move |ee: &mut Ee| {
        assert_eq!(ee.r(4) as u32, BIKE, "the stance's bike");
        l.borrow_mut().push(Out::Stance(ee.r(5) as u32 as i32));
    }));
    let l = log.clone();
    ee.hooks.insert(fake_fn, Box::new(move |ee: &mut Ee| {
        assert_eq!((ee.r(5), ee.u32_at(ee.r(6) as u32), ee.r(7), ee.r(8)), (5, 0, 0, 0), "a listener's word");
        l.borrow_mut().push(Out::Told(ee.r(4) as u32));
    }));
    let regions = [(BIKE + 0x680, 0x10), (BIKE + 0x700, 0x20), (BIKE + 0x1740, 0x20), (WRECK_AREA, 0xa00)];
    let image_words = snap_regions(&ee, &regions);
    let mut g = Gen(0x0031_b5f1);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        restore_regions(&mut ee, &regions, &image_words);
        let on = !g.one_in(4);
        ee.set_u32(BIKE + 0x684, if on || g.one_in(2) { g.bits() | 1 } else { 0 });
        ee.set_u32(BIKE + 0x680, if on || g.one_in(2) { player } else { 0 });
        for o in [0x1744, 0x1748, 0x704, 0x708, 0x70c, 0x710, 0x1754, 0x1758, 0x174c, 0x1750, 0x700, 0x714] {
            ee.set_u32(BIKE + o, g.bits());
        }
        let n = g.bits() % 5;
        ee.set_u32(player + 0x1d4, if n == 0 { 0 } else { listeners });
        for i in 0..n {
            let at = listeners + 0x40 * i;
            ee.set_u32(at + 0x1c, if i + 1 == n { 0 } else { at + 0x40 });
            ee.set_u32(at + 0x20, fake_vt);
        }
        ee.set_u32(fake_vt + 0xa0, fake_fn);
        let settles = ee.u32_at(BIKE + 0x684) != 0 && ee.u32_at(BIKE + 0x680) != 0;
        tally(&mut t, if !settles { "no run or player" } else if n == 0 { "settles, no listeners" } else { "settles, listeners" });
        let before = snap_regions(&ee, &regions);
        log.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(SETTLE_FN, &[u64::from(BIKE)], &[0.0], BUDGET);
        let (want, want_log) = (snap_regions(&ee, &regions), log.take());
        restore_regions(&mut ee, &regions, &before);
        let mut m = WreckLogged { ee: &mut ee, log: Vec::new(), says: Says::default() };
        wreck::settle(&mut m);
        let got_log = std::mem::take(&mut m.log);
        let got = snap_regions(&ee, &regions);
        assert_eq!(got_log, want_log, "case {k}: the calls");
        let off: Vec<String> = (0..want.len()).filter(|&i| want[i] != got[i]).map(|i| format!("word {i}: original {:#010x}, port {:#010x}", want[i], got[i])).collect();
        assert!(off.is_empty(), "case {k}: {}", off.join("\n  "));
    }
    eprintln!("{t:?}");
    assert!(t.values().all(|&n| n >= 1000), "{t:?}");
}

/// One of the callees whose bike words F31b5f watches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Writer {
    /// The clip held at its end (vt `+0x4ec`), ported as [`update::trick_hold`].
    TrickHold,
    /// The kind set (vt `+0x3a0`), the put-back's with a kind.
    KindSet,
    /// The event post (every one: the wreck's, the put-back's and the preload's cue).
    Post,
    /// The settle (vt `+0x4a8`), first in the wreck's entry and the unwreck.
    Settle,
}

/// The kind set and the settle (the bike's vt `+0x3a0` and `+0x4a8` on state 4).
const KIND_SET_FN: u32 = 0x004e_19a0;
const SETTLE_FN: u32 = 0x004e_4cd0;
/// The bike's vt `+0x53c` and `+0x540`, the settle's last two calls: a store of `a1` into `+0x1754`,
/// one into `+0x1758`.
const FLIP_CLIP_SET: u32 = 0x004e_7a30;
const FLIP_NEXT_SET: u32 = 0x004e_7a40;
const WRITERS: [(u32, Writer); 4] = [(TRICK_HOLD_FN, Writer::TrickHold), (KIND_SET_FN, Writer::KindSet), (POST_MESSAGE, Writer::Post), (SETTLE_FN, Writer::Settle)];

/// What [`writers_flow`] saw over a replay.
#[derive(Debug, Default)]
struct Writes {
    /// Each writer's runs, and the rows it ran on.
    ran: BTreeMap<Writer, std::collections::BTreeSet<usize>>,
    /// Each writer's words of the bike object it wrote, and the rows.
    wrote: BTreeMap<(Writer, u32), std::collections::BTreeSet<usize>>,
    /// Every byte of the bike object a writer wrote that nothing else has written since, and who.
    marked: BTreeMap<u32, Writer>,
    /// Every read of a marked word: (the word, its writer, who read it: a writer or the rest of the
    /// frame) and the rows.
    read: BTreeMap<(u32, Writer, Option<Writer>), std::collections::BTreeSet<usize>>,
    depth: u32,
    row: usize,
    /// The bike object and its blocks after every frame ([`bike_and_blocks`]).
    after: Vec<Vec<(u32, u32)>>,
}

impl Writes {
    /// The reads and writes since the last settle, `by` whom (none: the rest of the frame).
    fn settle(&mut self, ee: &mut Ee, by: Option<Writer>) {
        let reads = ee.reads.replace(Some(Default::default())).unwrap_or_default();
        for a in reads {
            if let Some(&w) = self.marked.get(&a) {
                self.read.entry((a.wrapping_sub(BIKE) & !3, w, by)).or_default().insert(self.row);
            }
        }
        if by.is_none() {
            for a in std::mem::take(&mut ee.written) {
                self.marked.remove(&a);
            }
        }
    }
}

/// C15's `name` replayed whole in the interpreter as [`replay`] does, every row's blocks and wreck
/// state bit-exact against the capture, with the [`WRITERS`] hooked: watched (each runs as itself, its
/// writes into the bike object marked and every later read of a marked byte noted, as
/// [`listeners_flow`]'s are) or, with `skip`, the kind set and the posts returning at once. A writer
/// reached inside another one is the outer one's.
fn writers_flow(name: &str, skip: bool) -> Option<Writes> {
    let mut ee = hooked(image()?);
    let rows = rows(name)?;
    assert_eq!((vt(&ee, 0x3a0), vt(&ee, 0x4a8), vt(&ee, 0x4ec)), (KIND_SET_FN, SETTLE_FN, TRICK_HOLD_FN), "the bike's vt +0x3a0, +0x4a8 and +0x4ec");
    let t: Rc<RefCell<Writes>> = Rc::default();
    for (f, who) in WRITERS {
        let h = t.clone();
        ee.hooks.insert(
            f,
            Box::new(move |ee: &mut Ee| {
                let row = h.borrow().row;
                h.borrow_mut().ran.entry(who).or_default().insert(row);
                if skip && matches!(who, Writer::KindSet | Writer::Post) {
                    ee.set(2, 0);
                    return;
                }
                if skip || h.borrow().depth > 0 {
                    return run_as_itself(ee, f);
                }
                h.borrow_mut().settle(ee, None);
                let before = std::mem::take(&mut ee.written);
                h.borrow_mut().depth += 1;
                run_as_itself(ee, f);
                let w = &mut *h.borrow_mut();
                w.depth -= 1;
                w.settle(ee, Some(who));
                let wrote = std::mem::replace(&mut ee.written, before);
                for a in wrote {
                    let o = a.wrapping_sub(BIKE);
                    if o < 0x1800 {
                        w.marked.insert(a, who);
                        w.wrote.entry((who, o & !3)).or_default().insert(row);
                    } else {
                        ee.written.insert(a);
                    }
                }
            }),
        );
    }
    for (r, row) in rows.iter().enumerate() {
        poke(&mut ee, row, &READER);
        t.borrow_mut().row = r;
        ee.written.clear();
        if !skip {
            *ee.reads.borrow_mut() = Some(Default::default());
        }
        ee.set(28, GP);
        ee.call(FRAME, &[u64::from(SIM)], &[f32::from_bits(DT)], BUDGET);
        if !skip {
            t.borrow_mut().settle(&mut ee, None);
            *ee.reads.borrow_mut() = None;
        }
        t.borrow_mut().after.push(bike_and_blocks(&ee));
        let d = diff(&ee, row);
        assert!(d.is_empty(), "{name} row {r} (vsync {}): {} words differ\n  {}", row["vsync"], d.len(), d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  "));
        assert_eq!(wrecked(|o| ee.u32_at(BIKE + o)), row_wrecked(row), "{name} row {r}: the wreck state");
        poke(&mut ee, row, &SPOT);
    }
    drop(ee);
    Some(Rc::try_unwrap(t).ok().unwrap().into_inner())
}

fn rows_of(s: &std::collections::BTreeSet<usize>) -> String {
    format!("{} rows {:?}..={:?}", s.len(), s.first().unwrap(), s.last().unwrap())
}

/// **F31b5f — the kind set's and the posts' bike words are unread; the trick hold's and the settle's
/// are read, and both are ported**: C15 ride-2 and flip-1 replayed whole in the interpreter, every row
/// bit-exact, with the trick hold, the kind set, every event post and the settle watched
/// ([`writers_flow`]). Every word of the bike object each one stores (changed or not):
/// - the trick hold ([`update::trick_hold`]): its clocks and the clip's fraction, `+0x14dc..+0x14e8`
///   (rows 113..=124); the fraction is read by the rest of the frame, the clocks by the hold itself;
/// - the settle ([`wreck::settle`]): `+0x704..+0x710`, `+0x1744`, `+0x1748`, `+0x1754`, `+0x1758`
///   (rows 125 and 193); `+0x704` and `+0x708` are read by the frame;
/// - the kind set: `+0x9a8` (row 193), never read before the frame stores it again;
/// - the posts (their listener the camera's controller): `+0x460`, `+0x468` (row 125), read by
///   nothing but a later post; and `+0x444` (rows 125 and 193), the bike's flags, stored back as they
///   were (the F31b5c census sees no change) and read by the frame.
#[test]
fn the_kind_sets_and_posts_bike_words_are_unread() {
    for name in ["ride-2", "flip-1"] {
        let Some(w) = writers_flow(name, false) else { return };
        for (who, rows) in &w.ran {
            eprintln!("{name}: {who:?} ran on {}", rows_of(rows));
        }
        for ((who, o), rows) in &w.wrote {
            eprintln!("{name}: {who:?} wrote bike+{o:#x} on {}", rows_of(rows));
        }
        for ((o, who, by), rows) in &w.read {
            eprintln!("{name}: bike+{o:#x}, {who:?}'s, read by {} on {}", by.map_or("the frame".into(), |b| format!("{b:?}")), rows_of(rows));
        }
        let wrote: Vec<(Writer, u32)> = w.wrote.keys().copied().collect();
        let mut want: Vec<(Writer, u32)> = Vec::new();
        if name == "flip-1" {
            want.extend([0x14dc, 0x14e0, 0x14e4, 0x14e8].map(|o| (Writer::TrickHold, o)));
            want.push((Writer::KindSet, 0x9a8));
            want.extend([0x444, 0x460, 0x468].map(|o| (Writer::Post, o)));
            want.extend([0x704, 0x708, 0x70c, 0x710, 0x1744, 0x1748, 0x1754, 0x1758].map(|o| (Writer::Settle, o)));
        }
        assert_eq!(wrote, want, "{name}: the bike words the callees store");
        let unported: Vec<_> = w.read.keys().filter(|&&(o, who, by)| match who {
            Writer::KindSet => true,
            Writer::Post => !(by == Some(Writer::Post) || by.is_none() && o == 0x444),
            Writer::TrickHold | Writer::Settle => false,
        }).collect();
        assert!(unported.is_empty(), "{name}: reads of the kind set's or the posts' words: {unported:x?}");
    }
}

/// **F31b5f — the frame without the kind set and the posts is bit-exact**: C15 ride-2 and flip-1
/// replayed whole in the interpreter with every event post and the kind set returning at once
/// ([`writers_flow`] with `skip`): every row's blocks and wreck state are C15's, and after every
/// frame the whole bike object and its physics blocks are the full frame's ([`listeners_flow`],
/// [`Others::Run`]) but for `+0x460` and `+0x468` (the posts', from row 125). The kind set's `+0x9a8`
/// is stored again within its frame, and the flags at `+0x444` the posts store back stay as they were.
#[test]
fn the_frame_without_the_kind_set_and_posts_is_bit_exact() {
    for (name, n) in [("ride-2", 459), ("flip-1", 465)] {
        let (Some(run), Some(dropped)) = (listeners_flow(name, Others::Run), writers_flow(name, true)) else { return };
        assert_eq!((run.after.len(), dropped.after.len()), (n, n));
        let mut parted = std::collections::BTreeSet::new();
        for (r, (a, b)) in run.after.iter().zip(&dropped.after).enumerate() {
            assert_eq!(a.len(), b.len(), "{name} row {r}: the same blocks");
            for (&(at, x), &(at2, y)) in a.iter().zip(b) {
                assert_eq!(at, at2, "{name} row {r}: the same blocks");
                if x != y {
                    let o = at.wrapping_sub(BIKE);
                    assert!([0x9a8, 0x460, 0x468].contains(&o), "{name} row {r}: {at:#x} (bike+{o:#x}) {x:#010x} with them, {y:#010x} without");
                    parted.insert(o);
                }
            }
        }
        eprintln!("{name}: without them, the bike's words that part: {parted:#x?}");
        let want: &[u32] = if name == "flip-1" { &[0x460, 0x468] } else { &[] };
        assert_eq!(parted.into_iter().collect::<Vec<_>>(), want, "{name}: the words that part");
        let ran: Vec<(Writer, usize)> = dropped.ran.iter().map(|(&k, v)| (k, v.len())).collect();
        let want: Vec<(Writer, usize)> = if name == "flip-1" { vec![(Writer::TrickHold, 12), (Writer::KindSet, 1), (Writer::Post, 465), (Writer::Settle, 2)] } else { vec![(Writer::Post, 459)] };
        assert_eq!(ran, want, "{name}: the rows each callee is reached on");
    }
}

// --- F43: the scene-node calls the bike's update makes, ported ----------------------------------

/// The sine and cosine of an angle off the curve table (`a0` the sine, `a1` the cosine, `$f12` the
/// angle), and where the table's pointer is.
const SIN_COS: u32 = 0x0011_0180;
const CURVE_AT: u32 = 0x003e_ab20;
/// Where the oracle lays its node forests and the calls' in and out quads: clear of the image's data
/// and of the stack the calls run on (the writes check holds it).
const FOREST: u32 = 0x01e0_0000;
const FOREST_NODES: u32 = 6;
const NODE_SIZE: u32 = 0x120;
const FOREST_IN: u32 = FOREST + 0x1000;
const FOREST_OUT: u32 = FOREST + 0x1100;
const FOREST_END: u32 = FOREST + 0x1200;

/// A float word for a frame, a point or an angle: mostly small, sometimes exact, sometimes any
/// exponent, now and then any bits at all.
fn node_float(x: u64) -> u32 {
    let unit = (x >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0;
    match x % 20 {
        0 => 0,
        1 => 0x8000_0000,
        2 => 1f32.to_bits(),
        3 => (-1f32).to_bits(),
        4..=6 => (x >> 8) as u32 & 0x807f_ffff | (((x >> 33) % 60 + 100) as u32) << 23,
        7 => (x >> 13) as u32,
        _ => (unit * 4.0).to_bits(),
    }
}

/// One random forest of [`FOREST_NODES`] nodes laid into RAM: every word of each random, then the
/// parent, child and sibling links of a forest, and flag bytes either kept in order (a node out of
/// date has its subtree out of date) or any.
fn lay_forest(ee: &mut Ee, seed: u64) {
    let mut k = 0u64;
    let mut r = || {
        k += 1;
        mix(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ k)
    };
    for o in (FOREST..FOREST_END).step_by(4) {
        let x = r();
        ee.set_u32(o, if x % 3 == 0 { x as u32 } else { node_float(x) });
    }
    let node = |i: u32| FOREST + i * NODE_SIZE;
    let mut parent = [None; FOREST_NODES as usize];
    for i in 1..FOREST_NODES {
        if r() % 10 < 7 {
            parent[i as usize] = Some((r() % u64::from(i)) as u32);
        }
    }
    let ordered = r() % 10 < 7;
    let mut current = [false; FOREST_NODES as usize];
    for i in 0..FOREST_NODES {
        let n = node(i);
        let p = parent[i as usize];
        ee.set_u32(n + 0xf0, p.map_or(0, node));
        let kids: Vec<u32> = (0..FOREST_NODES).filter(|&c| parent[c as usize] == Some(i)).collect();
        ee.set_u32(n + 0xf4, kids.first().map_or(0, |&c| node(c)));
        // A node's siblings: the next of its parent's children (roots have none).
        let sib = p.and_then(|p| (i + 1..FOREST_NODES).find(|&c| parent[c as usize] == Some(p)));
        ee.set_u32(n + 0xf8, sib.map_or(0, node));
        let on = r() % 2 == 0 && (!ordered || p.is_none_or(|p| current[p as usize]));
        current[i as usize] = on;
        let w = ee.u32_at(n + 0x100);
        let byte = (r() as u32 & 0x7e) | u32::from(on) | if r() % 2 == 0 { 0x80 } else { 0 };
        ee.set_u32(n + 0x100, w & !0xff | byte);
    }
}

/// The ten calls, in the shapes the bike's update makes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum NodeCall {
    Place,
    Rotate,
    Local,
    Position,
    Axes,
    Frame,
    PartFromFrame,
    Set,
    Aim,
    Spin,
}

const NODE_CALLS: [NodeCall; 10] =
    [NodeCall::Place, NodeCall::Rotate, NodeCall::Local, NodeCall::Position, NodeCall::Axes, NodeCall::Frame, NodeCall::PartFromFrame, NodeCall::Set, NodeCall::Aim, NodeCall::Spin];

/// Runs one call on node `n`, the original in the interpreter or the port on its RAM; the quads a
/// call hands back land at [`FOREST_OUT`].
fn node_call(ee: &mut Ee, c: NodeCall, n: u32, angle: f32, original: bool) {
    let q = |ee: &Ee, at: u32| [0, 4, 8, 12].map(|k| ee.u32_at(at + k));
    let (inp, up, out) = (FOREST_IN, FOREST_IN + 0x10, FOREST_OUT);
    let u = u64::from;
    if original {
        let (f, ints, floats): (u32, Vec<u64>, Vec<f32>) = match c {
            NodeCall::Place => (NODE_TIP, vec![u(out), u(n), u(inp)], vec![]),
            NodeCall::Rotate => (NODE_ROTATE, vec![u(out), u(n), u(inp)], vec![]),
            NodeCall::Local => (NODE_LOCAL, vec![u(out), u(n), u(inp)], vec![]),
            NodeCall::Position => (NODE_POSITION, vec![u(n), 0, u(out)], vec![]),
            NodeCall::Axes => (NODE_AXES, vec![u(n), 0, u(out), u(out + 0x10)], vec![]),
            NodeCall::Frame => (BONE_POSE, vec![u(n), 0, u(out)], vec![]),
            NodeCall::PartFromFrame => (BONE_PLACE, vec![u(n), u(inp)], vec![]),
            NodeCall::Set => (NODE_SET, vec![u(n), 0, u(inp)], vec![]),
            NodeCall::Aim => (NODE_AIM, vec![u(n), u(inp), u(up), 1, 1, 1], vec![]),
            NodeCall::Spin => (NODE_SPIN, vec![u(n), u(inp)], vec![angle]),
        };
        ee.set(28, GP);
        ee.call(f, &ints, &floats, BUDGET);
        return;
    }
    let (d, upq) = (q(ee, inp), q(ee, up));
    let m = &mut NodeRam(ee);
    let back = match c {
        NodeCall::Place => Some(node::place(m, n, d)),
        NodeCall::Rotate => Some(node::rotate(m, n, d)),
        NodeCall::Local => Some(node::local(m, n, d)),
        NodeCall::Position => {
            node::position(m, n, out);
            None
        }
        NodeCall::Axes => {
            node::axes(m, n, out, out + 0x10);
            None
        }
        NodeCall::Frame => {
            node::frame(m, n, out);
            None
        }
        NodeCall::PartFromFrame => {
            node::set_frame(m, n, inp);
            None
        }
        NodeCall::Set => {
            node::set(m, n, d);
            None
        }
        NodeCall::Aim => {
            node::aim(m, n, d, upq);
            None
        }
        NodeCall::Spin => {
            node::spin(m, n, inp, F(angle), curve());
            None
        }
    };
    if let Some(back) = back {
        for (k, w) in back.into_iter().enumerate() {
            ee.set_u32(out + 4 * k as u32, w);
        }
    }
}

/// **F43 — the scene-node calls are the original's**: 10 000 random forests (six nodes, every word
/// of each random, the links a forest's, the flags in order or not), each with one of the ten calls
/// on a random node of it — the original in the interpreter, then the port on the same RAM; every
/// word of the forest and of the quads handed in and out the same, and the original writes nothing
/// else outside its stack.
#[test]
fn the_scene_node_calls_are_the_originals() {
    let Some(mut ee) = image() else { return };
    let mut t: BTreeMap<String, u32> = BTreeMap::new();
    let snap = |ee: &Ee| -> Vec<u32> { (FOREST..FOREST_END).step_by(4).map(|o| ee.u32_at(o)).collect() };
    let stack = ee.stack;
    for k in 0..10_000u64 {
        lay_forest(&mut ee, k);
        let x = mix(k ^ 0x00f4_3000);
        let c = NODE_CALLS[(x % 10) as usize];
        let i = ((x >> 8) % u64::from(FOREST_NODES)) as u32;
        let n = FOREST + i * NODE_SIZE;
        let a = mix(x);
        let angle = match a % 8 {
            0 => (a >> 11) as u32 as i32 as f32 / 2e5,
            1 => [0.0, -0.0, std::f32::consts::PI, -std::f32::consts::PI, std::f32::consts::TAU][(a >> 9) as usize % 5],
            _ => (a >> 40) as f32 / (1u64 << 24) as f32 * 16.0 - 8.0,
        };
        let before = snap(&ee);
        let parent = ee.u32_at(n + 0xf0);
        let current = ee.u32_at(n + 0x100) & 1 == 1;
        let up_stale = std::iter::successors(Some(parent), |&p| (p != 0).then(|| ee.u32_at(p + 0xf0))).take_while(|&p| p != 0).any(|p| ee.u32_at(p + 0x100) & 1 == 0);
        let kids = ee.u32_at(n + 0xf4) != 0;
        let mut path = format!("{c:?}{}{}", if parent != 0 { " parent" } else { " root" }, if current { " current" } else { " stale" });
        if up_stale {
            *t.entry(format!("{c:?} a stale parent")).or_default() += 1;
        }
        if kids && current && matches!(c, NodeCall::PartFromFrame | NodeCall::Set | NodeCall::Aim | NodeCall::Spin) {
            *t.entry(format!("{c:?} children put out of date")).or_default() += 1;
        }
        if c == NodeCall::Spin && ee.u32_at(FOREST_IN + 8) >> 31 == 1 {
            path += " z<0";
        }
        *t.entry(path.clone()).or_default() += 1;
        node_call(&mut ee, c, n, angle, true);
        let off: Vec<u32> = ee.written.iter().copied().filter(|&w| !(FOREST..FOREST_END).contains(&w) && !(stack - 0x1_0000..stack + 0x4000).contains(&w)).collect();
        assert!(off.is_empty(), "case {k} ({path}): the original wrote outside the forest: {:#x?}", &off[..off.len().min(8)]);
        let want = snap(&ee);
        for (o, w) in (FOREST..FOREST_END).step_by(4).zip(&before) {
            ee.set_u32(o, *w);
        }
        node_call(&mut ee, c, n, angle, false);
        let got = snap(&ee);
        let diff: Vec<String> =
            (0..want.len()).filter(|&j| want[j] != got[j]).map(|j| format!("+{:#x}: original {:#010x}, port {:#010x}", 4 * j, want[j], got[j])).take(12).collect();
        assert!(diff.is_empty(), "case {k} ({path}, node {i}, angle {angle}): {}", diff.join("\n  "));
    }
    eprintln!("{t:#?}");
    for c in NODE_CALLS {
        for p in [" root current", " root stale", " parent current", " parent stale"] {
            assert!(t.get(&format!("{c:?}{p}")).is_some_and(|&n| n >= 50), "{c:?}{p}: {t:?}");
        }
        assert!(t.get(&format!("{c:?} a stale parent")).is_some_and(|&n| n >= 50), "{c:?} a stale parent: {t:?}");
    }
    for c in [NodeCall::PartFromFrame, NodeCall::Set, NodeCall::Aim, NodeCall::Spin] {
        assert!(t.get(&format!("{c:?} children put out of date")).is_some_and(|&n| n >= 50), "{c:?}: {t:?}");
    }
}

/// **F43 — the sine table is the image's and the sine and cosine the original's**: the built table
/// ([`CurveTable::build`]) word for word the one the image's pointer names, and 100 000 angles' sine
/// and cosine (small, round, out to ±10 000) the same words.
#[test]
fn the_spins_sine_and_cosine_are_the_originals() {
    let Some(mut ee) = image() else { return };
    let table = ee.u32_at(CURVE_AT);
    for (i, e) in curve().entries().iter().enumerate() {
        let at = table + 8 * i as u32;
        assert_eq!([ee.u32_at(at), ee.u32_at(at + 4)], e.map(f32::to_bits), "entry {i}");
    }
    let (s, c) = (ee.scratch(0), ee.scratch(1));
    for k in 0..100_000u64 {
        let a = mix(k ^ 0x5143);
        let angle = match a % 4 {
            0 => (a >> 11) as u32 as i32 as f32 / 2e5,
            1 => (k as f32 - 50_000.0) * std::f32::consts::FRAC_PI_4,
            _ => (a >> 40) as f32 / (1u64 << 24) as f32 * 16.0 - 8.0,
        };
        ee.set(28, GP);
        ee.call(SIN_COS, &[u64::from(s), u64::from(c)], &[angle], BUDGET);
        let (sin, cos) = node::sin_cos(F(angle), curve());
        assert_eq!([ee.u32_at(s), ee.u32_at(c)], [sin.to_bits(), cos.to_bits()], "angle {angle} ({:#010x})", angle.to_bits());
    }
}

// --- F45: the trick step's last calls and the unpack's node turn, ported ---------------------------

/// **F45 — the remote bike's stunt clocks are the original's**: 10 000 cases on state 4's bike, a stunt
/// table laid in the scratch area (every entry naming one clip), the next stunt's id under 46, past it
/// or a small negative, the hold flag (`+0x1788`), the clocks, the clip's run and `dt` drawn at random,
/// run through SLUS-21104's vt `+0x4e8` and through [`update::stunt_clocks`] from the same words (the
/// hold it hands to the original's on one side, [`update::trick_hold`] on the other). The answer, the
/// bike's words, the table, the clip and the noise's knot come out word for word, and the original
/// writes nothing else outside its stack but the noise's tables, built on first use.
#[test]
fn the_stunt_clocks_are_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    assert_eq!((vt(&ee, 0x4e8), vt(&ee, 0x4ec)), (STUNT_CLOCKS_FN, TRICK_HOLD_FN), "the bike's vt +0x4e8 and +0x4ec");
    let (table, clip) = (WRECK_AREA + 0x100, WRECK_AREA + 0x800);
    let regions = [(BIKE + 0x14d0, 0x30), (BIKE + 0x1528, 4), (BIKE + 0x1788, 4), (WRECK_AREA, 0x1000), (NOISE_KNOT, 4)];
    let image_words = snap_regions(&ee, &regions);
    let stack = ee.stack;
    let mut g = Gen(0x0000_f450);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        restore_regions(&mut ee, &regions, &image_words);
        ee.set_u32(BIKE + 0x14f0, table);
        for id in -4i32..46 {
            ee.set_u32(update::stunt_entry(table, id as u32), clip);
        }
        let id = match g.bits() % 8 {
            0 => 46 + g.bits() % 4,
            // Any id past the table (a big negative one names a word outside RAM in the original too).
            1 => (g.bits() >> 1).max(46),
            2 => (-1 - (g.bits() % 4) as i32) as u32,
            _ => g.bits() % 46,
        };
        ee.set_u32(BIKE + update::NEXT_STUNT, id);
        let held = g.one_in(4);
        ee.set_u32(BIKE + 0x1788, if held { g.bits() | 1 } else { 0 });
        for o in [0x14d8, 0x14dc, 0x14e0, 0x14e4, 0x14e8] {
            ee.set_u32(BIKE + o, if g.one_in(4) { reset_float(&mut g) } else { g.f(-1.0, 3.0).to_bits() });
        }
        ee.set_u32(clip + 0x48, if g.one_in(4) { reset_float(&mut g) } else { g.f(0.05, 3.0).to_bits() });
        let dt = if g.one_in(5) { f32::from_bits(reset_float(&mut g)) } else { f32::from_bits(DT) };
        let entry = update::stunt_entry(table, id);
        let path = if entry == 0 {
            "no entry"
        } else if held {
            "held"
        } else {
            let frac = (F(ee.f32_at(BIKE + 0x14d8)) + F(dt)) / F(ee.f32_at(clip + 0x48));
            if frac.min(F::ONE) < F::ONE { "running" } else { "run out" }
        };
        tally(&mut t, path);
        let before = snap_regions(&ee, &regions);
        ee.set(28, GP);
        ee.call(STUNT_CLOCKS_FN, &[u64::from(BIKE)], &[dt], BUDGET);
        let back = ee.v0() as u32;
        // The hold's noise builds its tables on first use: its flag, its tables and `rand()`'s state.
        let rand = ee.u32_at(RAND_BLOCK) + 0xa8;
        let noise = |w: &u32| (NOISE_PERM..NOISE_KNOT).contains(w) || (NOISE_UNBUILT..NOISE_UNBUILT + 4).contains(w) || (rand..rand + 8).contains(w);
        let inside = |w: &u32| regions.iter().any(|&(at, len)| (at..at + len).contains(w)) || (stack - 0x1_0000..stack + 0x4000).contains(w) || noise(w);
        let off: Vec<u32> = ee.written.iter().copied().filter(|w| !inside(w)).collect();
        assert!(off.is_empty(), "case {k} ({path}): the original wrote elsewhere: {:#x?}", &off[..off.len().min(8)]);
        let want = snap_regions(&ee, &regions);
        restore_regions(&mut ee, &regions, &before);
        let mut m = UpdateInRam { ee: &mut ee, x: dt };
        let got_back = update::stunt_clocks(&mut m, F(dt));
        let got = snap_regions(&ee, &regions);
        let diff: Vec<String> = (0..want.len()).filter(|&i| want[i] != got[i]).map(|i| format!("word {i}: original {:#010x}, port {:#010x}", want[i], got[i])).collect();
        assert!(diff.is_empty(), "case {k} ({path}, id {id:#x}, dt {dt}): {}", diff.join("\n  "));
        assert_eq!(got_back, back, "case {k} ({path}): the answer");
    }
    eprintln!("{t:?}");
    assert_eq!(t.len(), 4, "{t:?}");
    assert!(t.values().all(|&n| n >= 1000), "{t:?}");
}

/// **F45 — whether a trick is wanted is the original's**: 10 000 cases on state 4's bike, each word the
/// answer reads (`+0x324`, the object at `+0xc8` and its `+0x34`, `+0x14d4`, the kind byte `+0x14ec`'s
/// two nibbles, `+0x13e4`) zero or not at random, run through SLUS-21104's vt `+0x4b4` and through
/// [`update::trick_wanted`]: the same answer, and the original writes nothing outside its stack.
#[test]
fn the_trick_wanted_is_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    assert_eq!(vt(&ee, 0x4b4), TRICK_WANTED_FN, "the bike's vt +0x4b4");
    let object = WRECK_AREA;
    let regions = [(BIKE + 0x324, 4), (BIKE + 0xc8, 4), (BIKE + 0x14d4, 4), (BIKE + 0x14ec, 4), (BIKE + 0x13e4, 4), (WRECK_AREA, 0x100)];
    let image_words = snap_regions(&ee, &regions);
    let stack = ee.stack;
    let mut g = Gen(0x0000_f451);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        restore_regions(&mut ee, &regions, &image_words);
        let mut word = |g: &mut Gen| if g.one_in(2) { 0 } else { g.bits() | 1 << (g.bits() % 32) };
        let (a, b, c, d) = (word(&mut g), word(&mut g), word(&mut g), word(&mut g));
        ee.set_u32(BIKE + 0x324, a);
        ee.set_u32(BIKE + 0xc8, object);
        ee.set_u32(object + 0x34, b);
        ee.set_u32(BIKE + 0x14d4, c);
        ee.set_u32(BIKE + 0x13e4, d);
        let lo = if g.one_in(4) { 0 } else { 1 + g.bits() % 15 };
        let hi = if g.one_in(4) { 0 } else { 1 + g.bits() % 15 };
        let kind = g.bits() & !0xff | hi << 4 | lo;
        ee.set_u32(BIKE + 0x14ec, kind);
        let path = if a != 0 {
            "+0x324 set"
        } else if b != 0 {
            "the object's +0x34 set"
        } else if c == 0 {
            "+0x14d4 clear"
        } else if lo == 0 {
            "no low nibble"
        } else if hi == 0 {
            "no high nibble"
        } else if d != 0 {
            "+0x13e4 set"
        } else {
            "wanted"
        };
        tally(&mut t, path);
        ee.set(28, GP);
        ee.call(TRICK_WANTED_FN, &[u64::from(BIKE)], &[], BUDGET);
        let want = ee.v0() as u32;
        let off: Vec<u32> = ee.written.iter().copied().filter(|w| !(stack - 0x1_0000..stack + 0x4000).contains(w)).collect();
        assert!(off.is_empty(), "case {k} ({path}): the original wrote {:#x?}", &off[..off.len().min(8)]);
        let got = update::trick_wanted(&mut UpdateInRam { ee: &mut ee, x: 0.0 });
        assert_eq!(got, want, "case {k} ({path})");
    }
    eprintln!("{t:?}");
    assert_eq!(t.len(), 7, "{t:?}");
    assert!(t.values().all(|&n| n >= 200), "{t:?}");
}

/// **F45 — the node turned to a quaternion at a point is the original's**: 10 000 random forests (as
/// F43's), a random node of each turned by SLUS-21104's node placement with no frame of reference
/// (the unpack's call: the node, 0, the quaternion, the position) and by [`node::orient`], the
/// quaternion a unit one, near one or any words, the position any. Every word of the forest and of
/// the quads handed in the same, and the original writes nothing else outside its stack.
#[test]
fn the_node_turn_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let mut t: BTreeMap<String, u32> = BTreeMap::new();
    let snap = |ee: &Ee| -> Vec<u32> { (FOREST..FOREST_END).step_by(4).map(|o| ee.u32_at(o)).collect() };
    let stack = ee.stack;
    let (at, pos) = (FOREST_IN, FOREST_IN + 0x10);
    let mut g = Gen(0x0000_f452);
    for k in 0..10_000u64 {
        lay_forest(&mut ee, k ^ 0x00f4_5000);
        let i = g.bits() % FOREST_NODES;
        let n = FOREST + i * NODE_SIZE;
        let q = match g.bits() % 4 {
            0 | 1 => {
                let v = [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0)];
                let l = F(v[0]) * F(v[0]) + F(v[1]) * F(v[1]) + F(v[2]) * F(v[2]) + F(v[3]) * F(v[3]);
                let l = l.sqrt();
                if l > F(1e-3) { v.map(|x| (F(x) / l).to_bits()) } else { [0, 0, 0, 1f32.to_bits()] }
            }
            2 => [0, 0, 0, 0].map(|_: u32| g.f(-1.1, 1.1).to_bits()),
            _ => [0, 0, 0, 0].map(|_: u32| reset_float(&mut g)),
        };
        for (j, w) in q.into_iter().enumerate() {
            ee.set_u32(at + 4 * j as u32, w);
        }
        for j in 0..4 {
            ee.set_u32(pos + 4 * j, if g.one_in(4) { reset_float(&mut g) } else { g.f(-500.0, 500.0).to_bits() });
        }
        let parent = ee.u32_at(n + 0xf0);
        let up_stale = std::iter::successors(Some(parent), |&p| (p != 0).then(|| ee.u32_at(p + 0xf0))).take_while(|&p| p != 0).any(|p| ee.u32_at(p + 0x100) & 1 == 0);
        let path = format!("{}{}", if parent != 0 { "parent" } else { "root" }, if up_stale { ", a stale parent" } else { "" });
        *t.entry(path.clone()).or_default() += 1;
        if ee.u32_at(n + 0xf4) != 0 {
            *t.entry("children".into()).or_default() += 1;
        }
        let before = snap(&ee);
        ee.set(28, GP);
        ee.call(NODE_PLACE, &[u64::from(n), 0, u64::from(at), u64::from(pos)], &[], BUDGET);
        let off: Vec<u32> = ee.written.iter().copied().filter(|&w| !(FOREST..FOREST_END).contains(&w) && !(stack - 0x1_0000..stack + 0x4000).contains(&w)).collect();
        assert!(off.is_empty(), "case {k} ({path}): the original wrote outside the forest: {:#x?}", &off[..off.len().min(8)]);
        let want = snap(&ee);
        for (o, w) in (FOREST..FOREST_END).step_by(4).zip(&before) {
            ee.set_u32(o, *w);
        }
        let (qw, pw) = ([0, 4, 8, 12].map(|j| ee.u32_at(at + j)), [0, 4, 8, 12].map(|j| ee.u32_at(pos + j)));
        node::orient(&mut NodeRam(&mut ee), n, qw, pw);
        let got = snap(&ee);
        let diff: Vec<String> =
            (0..want.len()).filter(|&j| want[j] != got[j]).map(|j| format!("+{:#x}: original {:#010x}, port {:#010x}", 4 * j, want[j], got[j])).take(12).collect();
        assert!(diff.is_empty(), "case {k} ({path}, node {i}): {}", diff.join("\n  "));
    }
    eprintln!("{t:#?}");
    for p in ["root", "parent", "parent, a stale parent", "children"] {
        assert!(t.get(p).is_some_and(|&n| n >= 500), "{p}: {t:?}");
    }
}

// --- F43a: the rider's animation controller the bike's update calls, ported ----------------------

/// Where the image keeps the turn table the rotation keys are read through (4352 entries of two
/// words), and the arc cosine table's pointer and scale.
const TURNS_AT: u32 = 0x003d_3de0;
const ACOS_AT: u32 = 0x003e_a9c0;
const ACOS_SCALE_AT: u32 = 0x003e_a9d8;

/// **F43a — the key tables are the image's**: the turn table ([`Turns`], wide then narrow) and the arc
/// cosine table ([`AcosTable`]) the slerps read, word for word the ones the image built at startup.
#[test]
fn the_anim_key_tables_are_the_images() {
    let Some(ee) = image() else { return };
    let turns = Turns::get();
    for (i, e) in turns.wide.iter().chain(&turns.narrow).enumerate() {
        let at = TURNS_AT + 8 * i as u32;
        assert_eq!([ee.u32_at(at), ee.u32_at(at + 4)], e.map(F::to_bits), "turn {i}");
    }
    assert_eq!(turns.wide.len() + turns.narrow.len(), 4352);
    let table = ee.u32_at(ACOS_AT);
    for (i, e) in acos().entries().iter().enumerate() {
        assert_eq!(ee.u32_at(table + 4 * i as u32), e.to_bits(), "arc cosine {i}");
    }
    assert_eq!(ee.f32_at(ACOS_SCALE_AT), 14999.5);
}

/// Where the oracle lays its controller, bones, clips, channels and keys: past the node forest, clear
/// of the image's data and of the stack (the writes check holds it).
const ANIM: u32 = 0x01e0_8000;
const ANIM_END: u32 = ANIM + 0x4400;
const A_BONES: u32 = ANIM + 0x300;
const A_SAVED: [u32; 2] = [ANIM + 0x500, ANIM + 0x580];
const A_MASK: u32 = ANIM + 0x600;
const A_VT: u32 = ANIM + 0x620;
const A_OUT: u32 = ANIM + 0x680;
const A_CLIPS: [u32; 2] = [ANIM + 0x700, ANIM + 0x708];
const A_CHANNELS: u32 = ANIM + 0x1000;
const A_KEYS: u32 = ANIM + 0x2000;
/// The two buffers the oracle's heap hands a stop that has none (the heap itself waits on a semaphore
/// the interpreter has not got).
const A_HEAP: [u32; 2] = [ANIM_END, ANIM_END + 0x100];

/// The calls, in the update's shapes, and the two channel samplers on their own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AnimCall {
    Stop,
    Play,
    Blend,
    Rotation,
    Position,
}

/// The oracle's controller RAM: the interpreter's, with the saved buffers the original made handed
/// back to the port.
struct GivenRam<'a>(&'a mut Ee, [u32; 2]);

impl node::Ram for GivenRam<'_> {
    fn word(&mut self, at: u32) -> u32 {
        self.0.u32_at(at)
    }
    fn set(&mut self, at: u32, w: u32) {
        self.0.set_u32(at, w);
    }
}

impl anim::Anim for GivenRam<'_> {
    fn alloc(&mut self, _bytes: u32, buffer: anim::Saved) -> u32 {
        self.1[(buffer == anim::Saved::Positions) as usize]
    }
}

/// A rotation key's quaternion words: mostly a unit axis's (so slerps mostly take the arc), now and
/// then any halves; the turn always in the table.
fn rotation_key(r: &mut impl FnMut() -> u64) -> [u32; 2] {
    let x = r();
    let turn = (x % 4352) as u32;
    let axis: [i32; 3] = if x % 4 == 0 {
        [r() as i16, r() as i16, r() as i16].map(i32::from)
    } else {
        let v = [0, 1, 2].map(|_| (r() >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0);
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-3);
        v.map(|c| (c / l * 32767.0) as i32)
    };
    [turn | (axis[0] as u32) << 16, axis[1] as u32 & 0xffff | (axis[2] as u32) << 16]
}

/// One random controller laid into RAM: every word random first, then the bones (on the forest's
/// nodes), the saved buffers (or none), the fade list (or none), two clips of up to four rotation and
/// three position channels, each on or off with -1 (a rotation's only) to 20 keys at rising times (now and then two the
/// same) and a cursor from -1 to the last key (the game stores 1 to the last; past it the original
/// reads a key off the end).
fn lay_controller(ee: &mut Ee, seed: u64) -> (u32, u32) {
    let mut k = 0u64;
    let mut r = || {
        k += 1;
        mix(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ k ^ 0xa43a)
    };
    for o in (ANIM..ANIM_END).step_by(4) {
        let x = r();
        ee.set_u32(o, if x % 3 == 0 { x as u32 } else { node_float(x) });
    }
    let ctrl = ANIM;
    let bones = (r() % 7) as u32;
    ee.set_u32(ctrl + 0x1f8, bones);
    ee.set_u32(ctrl + 0x1fc, A_BONES);
    for b in 0..6 {
        ee.set_u32(A_BONES + 0x40 * b, FOREST + (r() % u64::from(FOREST_NODES)) as u32 * NODE_SIZE);
    }
    for (i, at) in [0x208, 0x20c].into_iter().enumerate() {
        ee.set_u32(ctrl + at, if r() % 6 == 0 { 0 } else { A_SAVED[i] });
    }
    for b in 0..6 {
        if r() % 4 != 0 {
            // A unit quaternion, as a stop leaves.
            let v = [0, 1, 2, 3].map(|_| (r() >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0);
            let l = v.iter().map(|c| c * c).sum::<f32>().sqrt().max(1e-3);
            for (c, x) in v.iter().enumerate() {
                ee.set_f32(A_SAVED[0] + 16 * b + 4 * c as u32, x / l);
            }
        }
    }
    ee.set_u32(ctrl + 0x210, if r() % 2 == 0 { 0 } else { A_MASK });
    for b in 0..6 {
        ee.set_u32(A_MASK + 4 * b, if r() % 2 == 0 { 0 } else { r() as u32 | 1 });
    }
    ee.set_u32(A_VT + 0x10, ANIM_TURNS);
    ee.set_u32(A_VT + 0x14, ANIM_MOVES);
    let length = match r() % 6 {
        0 => 1.0,
        1 => 0.0,
        _ => 0.25 + (r() >> 40) as f32 / (1u64 << 24) as f32 * 2.0,
    };
    let mut ch = A_CHANNELS;
    let mut keys = A_KEYS;
    for c in 0..2u32 {
        let clip = A_CLIPS[c as usize];
        let map = ANIM + 0x800 + 0x100 * c;
        let data = ANIM + 0xa00 + 0x100 * c;
        let lists = [ANIM + 0xc00 + 0x40 * c, ANIM + 0xc20 + 0x40 * c];
        ee.set_u32(clip, map);
        ee.set_u32(clip + 4, data);
        for i in 0..0x10 {
            let b = if r() % 5 == 0 { 0xff } else { (r() % 6) as u32 };
            let w = ee.u32_at(map + (i & !3));
            ee.set_u32(map + (i & !3), w & !(0xff << (8 * (i & 3))) | b << (8 * (i & 3)));
        }
        ee.set_u32(data + 0x1c, lists[0]);
        ee.set_u32(data + 0x18, lists[1]);
        ee.set_u32(data + 0x28, (r() % 5) as u32);
        ee.set_u32(data + 0x24, (r() % 4) as u32);
        ee.set_f32(data + 0x44, if c == 0 || r() % 2 == 0 { length } else { length * 0.5 });
        for (kind, list, count) in [(0u32, lists[0], 4u32), (1, lists[1], 3)] {
            for j in 0..count {
                ee.set_u32(list + 4 * j, ch);
                // A position channel's search never ends on a count below zero: the original's too.
                let n: i32 = match r() % 12 {
                    0 if kind == 0 => -1,
                    0 => 0,
                    1 => 0,
                    2 => 1,
                    3 => 2,
                    4 => 3 + (r() % 18) as i32,
                    _ => 3 + (r() % 6) as i32,
                };
                ee.set_u32(ch, (r() % 0x10) as u32);
                ee.set_u32(ch + 4, if r() % 6 == 0 { 0 } else { r() as u32 | 1 });
                ee.set_u32(ch + 8, n as u32);
                ee.set_u32(ch + 0x10, A_VT);
                ee.set_u32(ch + 0x14, keys + 0x10);
                ee.set_u32(ch + 0x18, ((r() % (n.max(0) as u64 + 1)) as u32).wrapping_sub(1));
                let size = if kind == 0 { 12 } else { 16 };
                let mut t = length * ((r() >> 40) as f32 / (1u64 << 24) as f32 * 0.6 - 0.3);
                for i in 0..n.max(0) as u32 {
                    let at = keys + 0x10 + size * i;
                    ee.set_f32(at, t);
                    if kind == 0 {
                        let q = rotation_key(&mut r);
                        ee.set_u32(at + 4, q[0]);
                        ee.set_u32(at + 8, q[1]);
                    }
                    if r() % 16 != 0 {
                        t += length * (r() >> 40) as f32 / (1u64 << 24) as f32 * 0.3;
                    }
                }
                ch += 0x20;
                keys += 0x180;
            }
        }
    }
    let x = r();
    let flags = ee.u32_at(ctrl + 0x2c);
    ee.set_u32(ctrl + 0x2c, flags & !0xffff | (x as u32 & 0xffff));
    (ctrl, (r() % 7) as u32)
}

/// A weight, mix or fade: mostly in 0..1, sometimes past either end, sometimes exactly 0 or 1.
fn anim_float(x: u64) -> f32 {
    match x % 10 {
        0 => 0.0,
        1 => 1.0,
        2 => -((x >> 40) as f32 / (1u64 << 24) as f32) * 0.3,
        3 => 1.0 + (x >> 40) as f32 / (1u64 << 24) as f32 * 0.3,
        _ => (x >> 40) as f32 / (1u64 << 24) as f32,
    }
}

/// **F43a — the anim controller's calls are the original's**: 10 000 random controllers
/// ([`lay_controller`]: bones on a random node forest, two clips, their channels and keys), each with
/// a stop, a play, a blend or one channel sampled — the original in the interpreter (its stack zeroed
/// first), then the port on the same RAM; every word of the controller, its clips and the forest the
/// same, the saved buffers the original made as well (from a heap that hands out two fixed ones), and
/// the original writes nothing else outside its stack.
#[test]
fn the_anim_controller_calls_are_the_originals() {
    let Some(mut ee) = image() else { return };
    let mut tally: BTreeMap<String, u32> = BTreeMap::new();
    let mut t = |s: String| *tally.entry(s).or_default() += 1;
    let snap = |ee: &Ee| -> Vec<u32> { (FOREST..FOREST_END).chain(ANIM..ANIM_END).step_by(4).map(|o| ee.u32_at(o)).collect() };
    let stack = ee.stack;
    let tables = anim::Tables { turns: Turns::get(), acos: acos() };
    ee.hooks.insert(
        HEAP_ALLOC,
        Box::new(|ee: &mut Ee| {
            assert_eq!(ee.r(5) as u32, HEAP_FILE);
            assert_eq!(ee.r(4) as u32, ee.u32_at(ANIM + 0x1f8) << 4);
            let line = ee.r(6);
            assert!(line == 0x52a || line == 0x52d, "line {line:#x}");
            ee.set(2, u64::from(A_HEAP[(line == 0x52d) as usize]));
        }),
    );
    let calls = [AnimCall::Stop, AnimCall::Play, AnimCall::Play, AnimCall::Blend, AnimCall::Blend, AnimCall::Rotation, AnimCall::Position];
    for k in 0..10_000u64 {
        lay_forest(&mut ee, k ^ 0xa43);
        let (ctrl, pick) = lay_controller(&mut ee, k);
        let x = mix(k ^ 0x00f4_3a00);
        let c = calls[(x % 7) as usize];
        let (weight, mix_, fade) = (anim_float(mix(x ^ 1)), anim_float(mix(x ^ 2)), if mix(x ^ 3) % 4 == 0 { 0.0 } else { anim_float(mix(x ^ 4)) });
        let ch = A_CHANNELS + 0x20 * (if c == AnimCall::Position { 4 + pick % 3 } else { pick % 4 } + 7 * (pick & 1));
        let time = {
            let data = ee.u32_at(A_CLIPS[0] + 4);
            ee.f32_at(data + 0x44) * (anim_float(mix(x ^ 5)) * 1.4 - 0.2)
        };
        // The path, by what the call is handed.
        let flags = ee.u32_at(ctrl + 0x2c);
        let mut path = format!("{c:?}");
        match c {
            AnimCall::Stop => {
                if ee.u32_at(ctrl + 0x208) == 0 || ee.u32_at(ctrl + 0x20c) == 0 {
                    path += " made buffers";
                }
            }
            AnimCall::Play | AnimCall::Blend => {
                path += if fade > 0.0 { " faded" } else { " not faded" };
                t(format!("{c:?}{}", if flags & 0x10 != 0 && ee.u32_at(ctrl + 0x210) != 0 { " a fade list" } else { " no fade list" }));
                t(format!("{c:?} nodes {}", (flags >> 12) & 3));
            }
            AnimCall::Rotation | AnimCall::Position => {
                let n = ee.u32_at(ch + 8) as i32;
                let keys = ee.u32_at(ch + 0x14);
                let size = if c == AnimCall::Rotation { 12 } else { 16 };
                path += if ee.u32_at(ch + 4) == 0 { " off" } else { " on" };
                path += &match n {
                    ..=0 => " no keys".to_string(),
                    1 | 2 => format!(" {n} keys"),
                    _ if time < ee.f32_at(keys) => " before the first".to_string(),
                    _ if time > ee.f32_at(keys + size * (n as u32 - 1)) => " past the last".to_string(),
                    _ => " inside".to_string(),
                };
            }
        }
        t(path.clone());
        for o in (stack - 0x2000..stack).step_by(4) {
            ee.set_u32(o, 0);
        }
        let before = snap(&ee);
        let u = u64::from;
        let (f, ints, floats): (u32, Vec<u64>, Vec<f32>) = match c {
            AnimCall::Stop => (ANIM_STOP, vec![u(ctrl), 0, 0, 0, 0], vec![]),
            AnimCall::Play => (ANIM_PLAY, vec![u(ctrl), u(A_CLIPS[0]), 0, 0, 0, 0, 0, 0], vec![weight, fade, 0.0]),
            AnimCall::Blend => (ANIM_BLEND, vec![u(ctrl), u(A_CLIPS[0]), u(A_CLIPS[1]), 0, 0, 0, 0, 0], vec![weight, mix_, fade]),
            AnimCall::Rotation => (ANIM_TURNS, vec![u(ch), u(A_OUT)], vec![time]),
            AnimCall::Position => (ANIM_MOVES, vec![u(ch), u(A_OUT)], vec![time]),
        };
        ee.set(28, GP);
        ee.call(f, &ints, &floats, BUDGET);
        let given = [ee.u32_at(ctrl + 0x208), ee.u32_at(ctrl + 0x20c)];
        let made = path.ends_with("made buffers");
        let off: Vec<u32> = ee.written.iter().copied().filter(|&w| !(FOREST..FOREST_END).contains(&w) && !(ANIM..A_HEAP[1] + 0x100).contains(&w) && !(stack - 0x1_0000..stack + 0x4000).contains(&w)).collect();
        assert!(off.is_empty(), "case {k} ({path}): the original wrote outside the controller: {:#x?}", &off[..off.len().min(8)]);
        let buffers = |ee: &Ee| -> Vec<u32> {
            let n = if made { ee.u32_at(ctrl + 0x1f8) } else { 0 };
            given.iter().flat_map(|&b| (0..4 * n).map(move |j| b + 4 * j)).map(|a| ee.u32_at(a)).collect()
        };
        let (want, want_buf) = (snap(&ee), buffers(&ee));
        for (o, w) in (FOREST..FOREST_END).chain(ANIM..ANIM_END).step_by(4).zip(&before) {
            ee.set_u32(o, *w);
        }
        let m = &mut GivenRam(&mut ee, given);
        match c {
            AnimCall::Stop => anim::stop(m, ctrl),
            AnimCall::Play => anim::play(m, &tables, ctrl, A_CLIPS[0], F(weight), F(fade)),
            AnimCall::Blend => anim::blend(m, &tables, ctrl, A_CLIPS, F(weight), F(mix_), F(fade)),
            AnimCall::Rotation => {
                if let Some(q) = anim::rotation(m, &tables, ch, F(time)) {
                    for (j, w) in q.into_iter().enumerate() {
                        m.0.set_u32(A_OUT + 4 * j as u32, w);
                    }
                }
            }
            AnimCall::Position => {
                let mut q = [0, 4, 8, 12].map(|j| m.0.u32_at(A_OUT + j));
                anim::position(m, ch, F(time), &mut q);
                for (j, w) in q.into_iter().enumerate() {
                    m.0.set_u32(A_OUT + 4 * j as u32, w);
                }
            }
        }
        let (got, got_buf) = (snap(&ee), buffers(&ee));
        let names = |j: usize| if j < ((FOREST_END - FOREST) / 4) as usize { format!("forest +{:#x}", 4 * j) } else { format!("controller +{:#x}", 4 * j - (FOREST_END - FOREST) as usize) };
        let diff: Vec<String> = (0..want.len()).filter(|&j| want[j] != got[j]).map(|j| format!("{}: original {:#010x}, port {:#010x}", names(j), want[j], got[j])).take(12).collect();
        assert!(diff.is_empty(), "case {k} ({path}, weight {weight}, mix {mix_}, fade {fade}, time {time}): {}", diff.join("\n  "));
        assert_eq!(want_buf, got_buf, "case {k} ({path}): the saved buffers");
    }
    eprintln!("{tally:#?}");
    let want = [
        "Stop", "Stop made buffers", "Play faded", "Play not faded", "Play a fade list", "Play no fade list", "Play nodes 0", "Play nodes 1", "Play nodes 2", "Play nodes 3",
        "Blend faded", "Blend not faded", "Blend a fade list", "Blend no fade list", "Blend nodes 0", "Blend nodes 3",
        "Rotation off no keys", "Rotation on no keys", "Rotation on 1 keys", "Rotation on 2 keys", "Rotation on before the first", "Rotation on past the last", "Rotation on inside",
        "Position off inside", "Position on no keys", "Position on 1 keys", "Position on 2 keys", "Position on before the first", "Position on past the last", "Position on inside",
    ];
    for p in want {
        assert!(tally.get(p).is_some_and(|&n| n >= 20), "{p}: {tally:#?}");
    }
}

// F31b5h — the player's run's calls, in Rust.

/// One of the run's six calls an oracle case makes.
#[derive(Clone, Copy, Debug, PartialEq)]
enum RunCall {
    Open,
    Score,
    Mark,
    End,
    Close,
    Tally,
}

/// Where a fake listener's told calls land: the hooked stubs (`+ slot`, never run) for a group's
/// vtable and a watcher's.
const STUB_GROUP: u32 = 0x100;
const STUB_WATCH: u32 = 0x200;

/// What a listener with its vtable at `vt` hears through `slot`, its arguments in the registers.
fn heard_at(ee: &Ee, vt: u32, slot: u32) -> run::Heard {
    use run::Heard;
    let (x, flag, a3) = (ee.f(12).to_bits(), ee.r(6) as u32, ee.r(7) as u32);
    let all = [Heard::Open, Heard::Tally { x, flag }, Heard::Score(flag), Heard::End { x, flag, node: a3 }];
    all.into_iter().find(|h| h.slot(vt) == slot).unwrap_or_else(|| panic!("no call through slot {slot:#x}"))
}

/// The run's ports on RAM, with the fake listeners' (their vtable one of `fakes`) calls and every
/// event post logged, the rest of the listeners [`told`].
struct RunOracle<'a> {
    ee: &'a mut Ee,
    fakes: [u32; 2],
    log: Vec<run::Out>,
}

impl run::Runs for RunOracle<'_> {
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn set(&mut self, at: u32, w: u32) {
        self.ee.set_u32(at, w);
    }
    fn out(&mut self, c: run::Out) {
        match c {
            run::Out::Tell { to, vt, .. } if self.fakes.contains(&self.ee.u32_at(to.wrapping_add(vt))) => self.log.push(c),
            run::Out::Tell { .. } => told(self, c),
            run::Out::Post { .. } => self.log.push(c),
        }
    }
}

/// A chain of `n` of `nodes` (next at `+0x24`), each with random words and a flag word from
/// `flags`, its ids among `ids`; answers its head.
fn chain(ee: &mut Ee, g: &mut Gen, nodes: &[u32], n: usize, ids: &[u32]) -> u32 {
    for i in 0..n {
        let at = nodes[i];
        ee.set_u32(at, g.bits() & 0x3f | if g.one_in(8) { g.bits() } else { 0 });
        ee.set_u32(at + 4, ids[(g.bits() as usize) % ids.len()]);
        for o in [0xc, 0x10, 0x14, 0x18] {
            let x = g.f(-50.0, 200.0);
            ee.set_f32(at + o, x);
        }
        let s = match g.bits() % 4 {
            0 => 0.0,
            1 => g.f(0.0, 1000.0).round(),
            _ => g.near(25.0, -100.0, 1000.0),
        };
        ee.set_f32(at + 8, s);
        let step = match g.bits() % 5 {
            0 => 0.0,
            1 => -0.0,
            2 => -5.0,
            3 => 10.0,
            _ => g.f(0.1, 50.0),
        };
        ee.set_f32(at + 0x20, step);
        ee.set_u32(at + 0x24, if i + 1 < n { nodes[i + 1] } else { 0 });
    }
    if n > 0 { nodes[0] } else { 0 }
}

/// **F31b5h — one of the run's calls is the original's**: 10 000 cases on state 4's run and player,
/// their words drawn at random (the run's flags, step and history length, its free list the image's
/// or a short one or none, the player's flags, its stunts and history as chains of 0 to 5 nodes in
/// scratch, its counts and scores), told to the image's group and the bike's watcher or to fakes in
/// scratch (a group with 0 to 2 members, a watcher, 0 to 2 more on the player's list) or to none,
/// the group's owner sometimes holding 0 to 3 gauges, the bike's watcher's words (its best combo,
/// time, kept stunts and the stunt table's repeat scales) drawn too; made with random arguments (ids
/// sign-extended) through SLUS-21104's call and [`run`]'s port. The answer, every word of the run,
/// the player, the nodes, the bike, its record, the table and the gauges comes out the same, nothing
/// else is written, and the fakes' calls (hooked stubs) and the event posts (hooked) are made in the
/// same order with the same arguments. Answers the paths taken.
#[allow(clippy::float_arithmetic)]
fn run_oracle(which: RunCall, seed: u64) -> Option<BTreeMap<&'static str, u32>> {
    let mut ee = image()?;
    let (run0, player0) = (ee.u32_at(BIKE + 0x684), ee.u32_at(BIKE + 0x680));
    let (group0, watch0) = (ee.u32_at(run0 + 0x24), ee.u32_at(player0 + 4));
    assert_eq!(watch0, BIKE + 0x680, "the player's watcher is the bike's");
    let owner = group0 - 0x364;
    let settings = ee.u32_at(owner + 0x3d4);
    let (record, table) = (ee.u32_at(BIKE + 0xd24), ee.u32_at(BIKE + 0x14f0));
    let mut free0 = Vec::new();
    let mut n = ee.u32_at(run0 + 0x3c);
    while n != 0 && free0.len() < 4 {
        free0.push(n);
        n = ee.u32_at(n + 0x24);
    }
    let block = |k: u32| ee.scratch(k);
    let nodes: Vec<u32> = (16..22).map(block).collect();
    let history: Vec<u32> = (22..26).map(block).collect();
    let spare: Vec<u32> = (26..29).map(block).collect();
    let (gvt, wvt) = (block(29), block(31));
    let (fake_group, members, fake_watch, listed) = (block(33), [block(34), block(35)], block(36), [block(37), block(38)]);
    let gauges = block(39);
    let gauge: Vec<u32> = [40, 42, 44].map(block).to_vec();
    for slot in (0x3c..0x50).step_by(4) {
        ee.set_u32(gvt + slot, STUB_GROUP + slot);
        ee.set_u32(wvt + slot, STUB_WATCH + slot);
    }
    for &l in [fake_group].iter().chain(&members) {
        ee.set_u32(l + 0x1c, gvt);
    }
    for &l in [fake_watch].iter().chain(&listed) {
        ee.set_u32(l + 0x20, wvt);
    }
    let log: Rc<RefCell<Vec<run::Out>>> = Rc::default();
    for (base, vt) in [(STUB_GROUP, run::GROUP_VT), (STUB_WATCH, run::WATCH_VT)] {
        for slot in (0x3c..0x50).step_by(4) {
            if vt == run::GROUP_VT && slot == 0x4c || vt == run::WATCH_VT && slot == 0x3c {
                continue;
            }
            let l = log.clone();
            ee.hooks.insert(base + slot, Box::new(move |ee: &mut Ee| {
                let heard = heard_at(ee, vt, slot);
                l.borrow_mut().push(run::Out::Tell { to: ee.r(4) as u32, vt, player: ee.r(5) as u32, heard });
                ee.set(2, 0);
            }));
        }
    }
    let l = log.clone();
    ee.hooks.insert(POST_FN, Box::new(move |ee: &mut Ee| {
        assert_eq!(ee.r(4) as u32, ee.u32_at(CUE_ON), "the post's queue");
        let [on, kind, id, data, x] = [5, 6, 7, 8, 9].map(|i| ee.r(i) as u32);
        l.borrow_mut().push(run::Out::Post { on, kind, id, data, x });
        ee.set(2, 0);
    }));
    let mut regions = vec![(run0, 0x310), (player0, 0x1e0), (group0, 0x20), (owner + 0x3d0, 0x14), (BIKE, 0x1800), (record, 0x24), (table, 0x1c + 46 * 12)];
    regions.extend(free0.iter().map(|&n| (n, 0x28)));
    regions.extend(nodes.iter().chain(&history).chain(&spare).map(|&n| (n, 0x28)));
    regions.push((gauges, 0x10));
    regions.extend(gauge.iter().map(|&g| (g, 0x64)));
    let image_words = snap_regions(&ee, &regions);
    let mut g = Gen(seed);
    let mut t = BTreeMap::new();
    let ids: Vec<u32> = (0..6).map(|_| g.bits() % 46).collect();
    for k in 0..10_000 {
        restore_regions(&mut ee, &regions, &image_words);
        // The run.
        let rf = g.bits() & 0xffff | if g.one_in(2) { 1 } else { 0 } & !if g.one_in(3) { 8 } else { 0 };
        let rf = if g.one_in(3) { rf & !8 | 1 } else { rf };
        ee.set_u32(run0 + 0x50, rf);
        let step = match g.bits() % 4 {
            0 => 1.0,
            1 => 10.0,
            2 => 0.5,
            _ => g.f(0.1, 100.0),
        };
        ee.set_f32(run0 + 0x2a8, step);
        ee.set_u32(run0 + 0x300, match g.bits() % 4 {
            0 => u32::MAX,
            1 => g.bits(),
            _ => g.bits() % 7,
        });
        let free = match g.bits() % 4 {
            0 => 0,
            1 => {
                let n = 1 + (g.bits() % 3) as usize;
                chain(&mut ee, &mut g, &spare, n, &ids)
            }
            _ => free0[0],
        };
        ee.set_u32(run0 + 0x3c, free);
        let group = match g.bits() % 4 {
            0 => 0,
            1 => fake_group,
            _ => group0,
        };
        ee.set_u32(run0 + 0x24, group);
        let m = (g.bits() % 3) as usize;
        let to = if m > 0 { members[0] } else { 0 };
        ee.set_u32(group + 4, to);
        ee.set_u32(members[0] + 0x18, if m > 1 { members[1] } else { 0 });
        ee.set_u32(members[1] + 0x18, 0);
        // The gauges.
        if group == group0 && g.one_in(2) {
            let count = match g.bits() % 5 {
                0 => g.bits() | 0x8000_0000,
                n => n - 1,
            };
            ee.set_u32(owner + 0x3dc, gauges);
            ee.set_u32(owner + 0x3e0, count);
            for (i, &at) in gauge.iter().enumerate() {
                ee.set_u32(gauges + 4 * i as u32, at);
                ee.set_u32(at + 0x20, g.bits());
                for o in [0x50, 0x58, 0x5c, 0x60] {
                    let x = g.near(0.0, -3.0, 3.0);
                    ee.set_f32(at + o, x);
                }
            }
            for o in [0x58c, 0x594, 0x59c] {
                let x = g.near(1.0, -3.0, 3.0);
                ee.set_f32(settings + o, x);
            }
        }
        // The player.
        let player = if g.one_in(10) { 0 } else { player0 };
        let pf = g.bits() & !0x63 | g.bits() & 0x63;
        let pf = if g.one_in(2) { pf | 0x20 } else { pf };
        let pf = if which == RunCall::Open && g.one_in(2) { pf & !1 | 2 } else { pf };
        ee.set_u32(player0 + 8, pf);
        let watch = match g.bits() % 4 {
            0 => 0,
            1 => fake_watch,
            _ => watch0,
        };
        ee.set_u32(player0 + 4, watch);
        let l = (g.bits() % 3) as usize;
        ee.set_u32(player0 + 0x1d4, if l > 0 { listed[0] } else { 0 });
        ee.set_u32(listed[0] + 0x1c, if l > 1 { listed[1] } else { 0 });
        ee.set_u32(listed[1] + 0x1c, 0);
        let n = (g.bits() % 6) as usize;
        let head = chain(&mut ee, &mut g, &nodes, n, &ids);
        ee.set_u32(player0 + 0xc8, head);
        // The tally walks its history by the counts: they are the chains' there, as the game keeps them.
        let off = which != RunCall::Tally && g.one_in(4);
        ee.set_u32(player0 + 0xcc, if off { g.bits() % 10 } else { n as u32 });
        let h = (g.bits() % 5) as usize;
        let past = chain(&mut ee, &mut g, &history, h, &ids);
        ee.set_u32(player0 + 0xd0, past);
        ee.set_u32(player0 + 0xd4, if off { g.bits() % 8 } else { h as u32 });
        let score = match g.bits() % 4 {
            0 => 1_000_000.0,
            1 => g.f(999_000.0, 1_001_000.0),
            _ => g.f(0.0, 50_000.0),
        };
        ee.set_f32(player0 + 0xdc, score);
        for o in [0xd8, 0xe0] {
            let x = g.f(0.0, 1000.0);
            ee.set_f32(player0 + o, x);
        }
        // The bike's watcher's words.
        let combo = g.near(0.0, 0.0, 2000.0);
        ee.set_f32(BIKE + 0x6f0, combo);
        let best = match g.bits() % 3 {
            0 => -1.0,
            _ => g.f(0.0, 60.0),
        };
        ee.set_f32(BIKE + 0x6f4, best);
        let now = g.f(0.0, 60.0);
        ee.set_f32(BIKE + 0x1750, now);
        for o in [0x13b8, 0x13bc, 0x13c0] {
            let x = g.bits() % 46;
            ee.set_u32(BIKE + o, x);
        }
        ee.set_u32(table, if g.one_in(4) { 0 } else { 1 });
        for o in [4, 8, 0xc] {
            let x = g.near(1.0, 0.1, 1.5);
            ee.set_f32(table + o, x);
        }
        for i in 0..46 {
            let x = if g.one_in(3) { 1.0 } else { g.f(0.0, 1.2) };
            ee.set_f32(table + 0x1c + 12 * i, x);
        }
        // The call.
        let x = match g.bits() % 3 {
            0 => 1.0,
            _ => g.near(1.0, -2.0, 30.0),
        };
        let flag = match g.bits() % 3 {
            0 => 0,
            1 => 1,
            _ => g.bits(),
        };
        let id = if g.one_in(3) { u32::MAX } else if g.one_in(4) { g.bits() % 46 } else { ids[(g.bits() as usize) % ids.len()] };
        let node = if g.one_in(2) || n == 0 { 0 } else { nodes[(g.bits() as usize) % n] };
        let kind = if g.one_in(2) { 12 } else { g.bits() };
        let fs: [f32; 5] = std::array::from_fn(|_| g.near(1.0, -20.0, 60.0));
        let sx = |v: u32| v as i32 as i64 as u64;
        let (r, p) = (u64::from(run0), u64::from(player));
        let (f, ints, floats): (u32, Vec<u64>, Vec<f32>) = match which {
            RunCall::Open => (RUN_OPEN, vec![r, p], vec![]),
            RunCall::Score => (RUN_SCORE, vec![r, p, sx(id), sx(kind)], fs.to_vec()),
            RunCall::Mark => (MARK_FN, vec![r, p, u64::from(flag), sx(id), u64::from(node)], vec![]),
            RunCall::End => (RUN_END, vec![r, p, u64::from(flag), sx(id), u64::from(node)], vec![x]),
            RunCall::Close => (RUN_CLOSE, vec![r, p, u64::from(flag)], vec![x]),
            RunCall::Tally => (RUN_TALLY, vec![r, p, u64::from(flag)], vec![x]),
        };
        let before = snap_regions(&ee, &regions);
        log.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(f, &ints, &floats, BUDGET);
        let answer = ee.r(2) as u32;
        let stray = written_outside(&ee, &regions);
        assert!(stray.is_empty(), "case {k}: the original wrote outside the run's words: {stray:?}");
        let want = snap_regions(&ee, &regions);
        let want_log = log.borrow().clone();
        restore_regions(&mut ee, &regions, &before);
        let mut m = RunOracle { ee: &mut ee, fakes: [gvt, wvt], log: Vec::new() };
        let [f12, f13, f14, f15, f16] = fs.map(F);
        let got_answer = match which {
            RunCall::Open => run::open(&mut m, run0, player),
            RunCall::Score => run::score(&mut m, run0, player, id, kind, f12, f13, f14, f15, f16),
            RunCall::Mark => run::mark(&mut m, run0, player, flag, id, node),
            RunCall::End => run::end(&mut m, F(x), run0, player, flag, id, node),
            RunCall::Close => run::close(&mut m, F(x), run0, player, flag),
            RunCall::Tally => run::tally(&mut m, F(x), run0, player, flag),
        };
        let got_log = std::mem::take(&mut m.log);
        assert_eq!(got_answer, answer, "case {k}: the answer");
        assert_eq!(got_log, want_log, "case {k}: the calls");
        let got = snap_regions(&ee, &regions);
        if got != want {
            let mut off = Vec::new();
            let mut i = 0;
            for &(at, len) in &regions {
                for o in (0..len).step_by(4) {
                    if got[i] != want[i] {
                        off.push(format!("{:#x}: port {:#010x}, original {:#010x}", at + o, got[i], want[i]));
                    }
                    i += 1;
                }
            }
            panic!("case {k} ({which:?}): {} words differ\n  {}", off.len(), off.join("\n  "));
        }
        // The paths.
        let word = |a: u32| {
            let i = regions.iter().scan(0usize, |s, &(at, len)| {
                let i = *s;
                *s += len as usize / 4;
                Some((at, len, i))
            });
            i.into_iter().find(|&(at, len, _)| (at..at + len).contains(&a)).map(|(at, _, i)| (before[i + ((a - at) / 4) as usize], want[i + ((a - at) / 4) as usize])).unwrap()
        };
        let changed = |a: u32| {
            let (b, w) = word(a);
            b != w
        };
        if player == 0 {
            tally(&mut t, "no player");
            continue;
        }
        let posts = want_log.iter().filter(|c| matches!(c, run::Out::Post { .. })).count();
        let fakes = want_log.len() - posts;
        if posts > 0 {
            tally(&mut t, "posted");
        }
        if fakes > 0 {
            tally(&mut t, "fakes told");
        }
        if gauge.iter().any(|&a| changed(a + 0x58) || changed(a + 0x5c) || changed(a + 0x60)) {
            tally(&mut t, "a gauge kicked");
        }
        let node_flags_changed = nodes[..n].iter().filter(|&&a| changed(a)).count();
        match which {
            RunCall::Open => {
                tally(&mut t, if changed(player0 + 8) { "opened" } else { "left" });
                if changed(player0 + 8) && head != 0 {
                    tally(&mut t, "stunts freed");
                }
            }
            RunCall::Score => {
                tally(&mut t, if answer == 0 { if pf & 0x20 == 0 { "no run open" } else { "no free node" } } else { "scored" });
                if answer != 0 && free == free0[0] {
                    tally(&mut t, "off the image's free list");
                }
            }
            RunCall::Mark => tally(&mut t, match node_flags_changed {
                0 => "nothing marked",
                _ if flag != 0 => "marked",
                _ => "unmarked",
            }),
            RunCall::End => {
                if answer == 0 {
                    tally(&mut t, "nothing running");
                    continue;
                }
                tally(&mut t, if node_flags_changed == 0 { "no node" } else if flag == 0 { "unscored" } else { "scored" });
                if nodes[..n].iter().any(|&a| changed(a + 8)) {
                    tally(&mut t, "the score scaled");
                }
                if word(player0 + 8).1 & 0x40 != 0 {
                    tally(&mut t, "still running");
                }
            }
            RunCall::Close => tally(&mut t, match node_flags_changed {
                0 => "none ended",
                1 => "one ended",
                _ => "some ended",
            }),
            RunCall::Tally => {
                if answer == 0 {
                    tally(&mut t, "no run open");
                    continue;
                }
                tally(&mut t, if rf & 0x8000 == 0 { "the stunts freed" } else if (ee.u32_at(run0 + 0x300) as i32) < 0 { "the history dropped" } else { "the history kept" });
                if changed(player0 + 0xd4) && word(player0 + 0xd4).1 != word(player0 + 0xd4).0 && rf & 0x8000 != 0 && word(player0 + 0xd4).1 == ee.u32_at(run0 + 0x300) {
                    tally(&mut t, "the history cut");
                }
                if changed(BIKE + 0x6f0) {
                    tally(&mut t, "a best combo");
                }
                if changed(BIKE + 0x6f4) {
                    tally(&mut t, "a best time");
                }
                if (0..46).any(|i| changed(table + 0x1c + 12 * i)) {
                    tally(&mut t, "repeats scaled");
                }
                if changed(player0 + 0xdc) {
                    tally(&mut t, "the score moved");
                }
            }
        }
    }
    eprintln!("{which:?}: {t:?}");
    Some(t)
}

/// [`run_oracle`]'s `paths` each taken at least `least` times.
fn run_paths(which: RunCall, seed: u64, paths: &[&str], least: u32) {
    let Some(t) = run_oracle(which, seed) else { return };
    for want in paths {
        assert!(t.get(want).copied().unwrap_or(0) >= least, "{which:?}: path {want:?} taken {} times: {t:?}", t.get(want).copied().unwrap_or(0));
    }
}

/// **F31b5h — the run's open is the original's** ([`run_oracle`]).
#[test]
fn the_run_open_is_the_originals() {
    run_paths(RunCall::Open, 0x31b5_0001, &["no player", "opened", "left", "stunts freed", "posted", "fakes told"], 50);
}

/// **F31b5h — a stunt's score is the original's** ([`run_oracle`]).
#[test]
fn the_run_score_is_the_originals() {
    run_paths(RunCall::Score, 0x31b5_0002, &["no player", "no run open", "no free node", "scored", "off the image's free list", "posted", "fakes told", "a gauge kicked"], 50);
}

/// **F31b5h — a stunt's mark is the original's** ([`run_oracle`]).
#[test]
fn the_run_mark_is_the_originals() {
    run_paths(RunCall::Mark, 0x31b5_0003, &["no player", "nothing marked", "marked", "unmarked"], 50);
}

/// **F31b5h — a stunt's end is the original's** ([`run_oracle`]).
#[test]
fn the_run_end_is_the_originals() {
    run_paths(RunCall::End, 0x31b5_0004, &["no player", "nothing running", "no node", "unscored", "scored", "the score scaled", "still running", "posted", "fakes told"], 50);
}

/// **F31b5h — the run's close is the original's** ([`run_oracle`]).
#[test]
fn the_run_close_is_the_originals() {
    run_paths(RunCall::Close, 0x31b5_0005, &["no player", "none ended", "one ended", "some ended", "posted", "fakes told"], 50);
}

/// **F31b5h — the run's tally is the original's** ([`run_oracle`]).
#[test]
fn the_run_tally_is_the_originals() {
    run_paths(
        RunCall::Tally,
        0x31b5_0006,
        &["no player", "no run open", "the stunts freed", "the history dropped", "the history kept", "the history cut", "a best combo", "a best time", "repeats scaled", "the score moved", "posted", "fakes told"],
        50,
    );
}

// --- F46: the record's searches and the sync's database half in Rust ----------------------------------

use game_sim::atv_spatial::{self, Cell as DbCell, Database, Extra, Item, Link, LinkAt, List as DbList, Place, Pool, Slot, SlotAt, Sub};

/// The global holding the database's link pool, and the list sub-database's vtable (MD75c1's).
const DB_POOL_AT: u32 = 0x0040_c460;
const DB_LIST_VT: u32 = 0x003b_5510;

thread_local! {
    /// The handed-on calls [`posts_frame`] answers in Rust, by [`call_name`] (`PartMove`): counted
    /// without the `> `.
    static IN_RUST: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

fn in_rust(name: &str) -> bool {
    IN_RUST.with(|r| r.borrow().contains(&name))
}

/// Where the database [`DbInRam::read`] read is in RAM: its cells', portals' and sub-databases'
/// addresses (in the model's order), each list's blocks, the entries by item, the link pool's
/// chunks and stride.
struct DbInRam {
    cells: Vec<u32>,
    portals: Vec<u32>,
    subs: Vec<u32>,
    blocks: Vec<Vec<u32>>,
    items: Vec<u32>,
    chunks: Vec<u32>,
    stride: u32,
    capacity: u32,
}

impl DbInRam {
    fn index(list: &[u32], a: u32, what: &str) -> usize {
        list.iter().position(|&x| x == a).unwrap_or_else(|| panic!("{what} {a:#x} is none of the database's"))
    }

    fn item(&mut self, a: u32) -> Item {
        self.items.iter().position(|&x| x == a).unwrap_or_else(|| {
            self.items.push(a);
            self.items.len() - 1
        })
    }

    fn slot_at(&self, sub: usize, a: u32, size: u32) -> SlotAt {
        for (b, &base) in self.blocks[sub].iter().enumerate() {
            if (base..base + 4 * size).contains(&a) {
                assert_eq!((a - base) % 4, 0, "a slot {a:#x} off its block's words");
                return SlotAt { block: b, index: ((a - base) / 4) as usize };
            }
        }
        panic!("slot {a:#x} is in none of sub-database {sub}'s blocks");
    }

    fn slot_addr(&self, sub: usize, s: SlotAt) -> u32 {
        self.blocks[sub][s.block] + 4 * s.index as u32
    }

    fn link_at(&self, a: u32) -> LinkAt {
        for (c, &base) in self.chunks.iter().enumerate() {
            let first = base + 4;
            if (first..first + self.capacity * self.stride).contains(&a) {
                assert_eq!((a - first) % self.stride, 0, "a link {a:#x} off its chunk's stride");
                return LinkAt { chunk: c, index: ((a - first) / self.stride) as usize };
            }
        }
        panic!("link {a:#x} is in none of the pool's chunks");
    }

    fn place(&self, ee: &Ee, sub: usize, a: u32) -> Place {
        assert_eq!(ee.u32_at(self.subs[sub] + 4), DB_LIST_VT, "a place {a:#x} in sub-database {:#x}, not a list", self.subs[sub]);
        Place::Slot(self.slot_at(sub, a, ee.u32_at(self.subs[sub] + 0x28)))
    }

    /// The link pool at `p`, as the move takes and gives its links.
    fn pool(&mut self, ee: &Ee, p: u32) -> Pool {
        let mut c = ee.u32_at(p + 0xc);
        while c != 0 {
            self.chunks.push(c);
            assert!(self.chunks.len() < 64, "the pool's chunk chain does not end");
            c = ee.u32_at(c);
        }
        (self.capacity, self.stride) = (ee.u32_at(p + 8), ee.u32_at(p + 0x1c));
        let mut free = vec![];
        let mut f = ee.u32_at(p + 0x24);
        while f != 0 {
            free.push(self.link_at(f));
            assert!(free.len() < 4096, "the pool's free chain does not end");
            f = ee.u32_at(f);
        }
        free.reverse();
        let mut spare = 0;
        let mut s = ee.u32_at(p + 0x14);
        while s != 0 {
            spare += 1;
            assert!(spare < 4096, "the pool's set-aside chain does not end");
            s = ee.u32_at(s);
        }
        Pool { capacity: ee.i32_at(p + 8), stride: self.stride, used: ee.i32_at(p + 0x18), chunks: self.chunks.len(), spare, free }
    }

    /// The database `entry` (a collidable's `+0x18`) is in, as [`atv_spatial`] models it, and the
    /// entry's record (its `+8`); the entry is item 0. The cells are the record's and those its links
    /// reach; their lists are read slot by slot, any other sub-database (the quadtree the gather
    /// reads) by its kind alone, which a move into it would refuse.
    fn read(ee: &Ee, entry: u32) -> (DbInRam, Database, atv_spatial::Record) {
        let mut m = DbInRam { cells: vec![], portals: vec![], subs: vec![], blocks: vec![], items: vec![entry], chunks: vec![], stride: 0, capacity: 0 };
        let pool = m.pool(ee, ee.u32_at(DB_POOL_AT));
        let rec = ee.u32_at(entry + 8);
        let home = ee.u32_at(rec + 4);
        let mut db = Database { pool, ..Default::default() };
        if home == 0 {
            return (m, db, atv_spatial::Record { last: vf_at(ee, rec + 0x10), ..Default::default() });
        }
        m.cells.push(home);
        let mut k = 0;
        while k < m.cells.len() {
            let c = m.cells[k];
            let mut l = ee.u32_at(c + 0x24);
            while l != 0 {
                let (to, portal) = (ee.u32_at(l + 4), ee.u32_at(l + 8));
                if !m.cells.contains(&to) {
                    m.cells.push(to);
                }
                if !m.portals.contains(&portal) {
                    m.portals.push(portal);
                }
                l = ee.u32_at(l);
            }
            for o in [0x28, 0x2c] {
                let s = ee.u32_at(c + o);
                if s != 0 && !m.subs.contains(&s) {
                    m.subs.push(s);
                }
            }
            k += 1;
        }
        for &c in &m.cells {
            let mut links = vec![];
            let mut l = ee.u32_at(c + 0x24);
            while l != 0 {
                links.push(Link { cell: DbInRam::index(&m.cells, ee.u32_at(l + 4), "a link's cell"), portal: DbInRam::index(&m.portals, ee.u32_at(l + 8), "a link's portal") });
                l = ee.u32_at(l);
            }
            let sub = |o: u32| match ee.u32_at(c + o) {
                0 => None,
                s => Some(DbInRam::index(&m.subs, s, "a cell's sub-database")),
            };
            db.cells.push(DbCell { links, subs: [sub(0x28), sub(0x2c)] });
        }
        db.portals = m
            .portals
            .iter()
            .map(|&a| atv_spatial::Portal {
                corners: std::array::from_fn(|i| vf_at(ee, a + 0x30 + 0x10 * i as u32)),
                normal: vf_at(ee, a + 0x70),
                plane: F(ee.f32_at(a + 0xc4)),
                edges: std::array::from_fn(|i| vf_at(ee, a + 0x80 + 0x10 * i as u32)),
            })
            .collect();
        for k in 0..m.subs.len() {
            let s = m.subs[k];
            let kind = ee.u32_at(s + 8);
            if ee.u32_at(s + 4) != DB_LIST_VT {
                m.blocks.push(vec![]);
                db.subs.push(Sub::Other { kind });
                continue;
            }
            let (size, last, arr) = (ee.i32_at(s + 0x28), ee.i32_at(s + 0x30), ee.u32_at(s + 0x34));
            m.blocks.push((0..ee.u32_at(s + 0x2c)).map(|b| ee.u32_at(arr + 4 * b)).collect());
            let n = m.blocks[k].len();
            let mut blocks = vec![];
            for b in 0..n {
                let base = m.blocks[k][b];
                let handed = if b + 1 == n { last } else { size } as usize;
                let mut slots = vec![None; size as usize];
                for (i, slot) in slots.iter_mut().enumerate().take(handed) {
                    let v = ee.u32_at(base + 4 * i as u32);
                    *slot = Some(if v & 2 != 0 {
                        let next = v & !3;
                        Slot::Free((next != 0).then(|| m.slot_at(k, next, size as u32)))
                    } else {
                        Slot::Item(m.item(v))
                    });
                }
                blocks.push(slots);
            }
            let free = match ee.u32_at(s + 0x38) {
                0 => None,
                f => Some(m.slot_at(k, f, size as u32)),
            };
            db.subs.push(Sub::List(DbList { kind, size, last, blocks, free }));
        }
        let sub = DbInRam::index(&m.subs, ee.u32_at(rec + 8), "the record's sub-database");
        let mut record = atv_spatial::Record { cell: Some(0), sub, slot: m.place(ee, sub, ee.u32_at(rec + 0xc)), last: vf_at(ee, rec + 0x10), extras: vec![] };
        let mut l = ee.u32_at(rec);
        while l != 0 {
            let sub = DbInRam::index(&m.subs, ee.u32_at(l + 8), "an extra's sub-database");
            record.extras.push(Extra { at: m.link_at(l), cell: DbInRam::index(&m.cells, ee.u32_at(l + 4), "an extra's cell"), sub, slot: m.place(ee, sub, ee.u32_at(l + 0xc)) });
            assert!(record.extras.len() < 64, "the record's extras do not end");
            l = ee.u32_at(l);
        }
        (m, db, record)
    }

    fn place_word(&self, sub: usize, p: Place) -> u32 {
        match p {
            Place::Slot(s) => self.slot_addr(sub, s),
            Place::Key(k) => k,
        }
    }

    /// The move's outcome (`now`, read as `was`) written back where it changed: the record's words
    /// and each list's slots, count and free chain. What a move here never changes — the cells, a
    /// list's blocks, the link pool, the extras — must not have changed.
    fn write(&self, ee: &mut Ee, entry: u32, was: (&Database, &atv_spatial::Record), now: (&Database, &atv_spatial::Record)) {
        assert_eq!(was.0.cells, now.0.cells, "the cells changed");
        assert_eq!(was.0.pool, now.0.pool, "the link pool changed: not written back");
        assert_eq!(was.1.extras, now.1.extras, "the record's extras changed: not written back");
        for (k, (a, b)) in was.0.subs.iter().zip(&now.0.subs).enumerate() {
            let (Sub::List(a), Sub::List(b)) = (a, b) else {
                assert_eq!(a, b, "sub-database {:#x} is no list and changed", self.subs[k]);
                continue;
            };
            assert_eq!(a.blocks.len(), b.blocks.len(), "list {:#x} grew a block: not written back", self.subs[k]);
            for (bl, (x, y)) in a.blocks.iter().zip(&b.blocks).enumerate() {
                for (i, (x, y)) in x.iter().zip(y).enumerate() {
                    if x == y {
                        continue;
                    }
                    let w = match y.expect("a slot handed out is handed out still") {
                        Slot::Item(it) => self.items[it],
                        Slot::Free(n) => n.map_or(0, |n| self.slot_addr(k, n)) | 2,
                    };
                    ee.set_u32(self.slot_addr(k, SlotAt { block: bl, index: i }), w);
                }
            }
            let s = self.subs[k];
            ee.set_u32(s + 0x30, b.last as u32);
            ee.set_u32(s + 0x38, b.free.map_or(0, |f| self.slot_addr(k, f)));
        }
        let rec = ee.u32_at(entry + 8);
        let r = now.1;
        if r.cell.is_some() {
            ee.set_u32(rec + 4, self.cells[r.cell.unwrap()]);
            ee.set_u32(rec + 8, self.subs[r.sub]);
            ee.set_u32(rec + 0xc, self.place_word(r.sub, r.slot));
        }
        set_vf(ee, rec + 0x10, r.last);
    }
}

/// **F46a — a collidable's spatial-database move in Rust**: `entry`'s database read out of RAM,
/// moved by [`atv_spatial::spatial_move`], and written back.
fn spatial_move_in_ram(ee: &mut Ee, entry: u32, centre: Vf, half: Vf, reach: F) {
    let (m, mut db, mut rec) = DbInRam::read(ee, entry);
    let (was_db, was_rec) = (db.clone(), rec.clone());
    atv_spatial::spatial_move(&mut db, &mut rec, 0, centre, half, reach);
    m.write(ee, entry, (&was_db, &was_rec), (&db, &rec));
}

use game_sim::atv_world;
use game_sim::broad_phase as bp;
use game_sim::world_search as ws;
use game_sim::{atv_part, ray_probe};

/// F46b: the gather's globals — the stamp queries hand entries out under (a halfword), the
/// quadtree last queried, world units per tree cell, the table every global query ends with, the
/// ecosystem (placed groups' cell and models). The quadtree's vtable is [`TREE_VT`].
const STAMP_AT: u32 = 0x003d_3b88;
const CURRENT_AT: u32 = 0x0040_c198;
const CELL_SIZE_AT: u32 = 0x0040_c1a0;
const GLOBAL_AT: u32 = 0x0040_c440;
const ECO_AT: u32 = 0x0052_cd88;

/// A float's word as it is, bits and all (the gather copies the centre by integer loads).
fn word_f(ee: &Ee, a: u32) -> F {
    F(f32::from_bits(ee.u32_at(a)))
}

fn set_u8(ee: &mut Ee, a: u32, v: u8) {
    ee.write(a, 1, u128::from(v));
}

fn set_u16(ee: &mut Ee, a: u32, v: u16) {
    ee.write(a, 2, u128::from(v));
}

/// The quadtree query's node pool (tree `+0x94`), as its take and its reset leave it in RAM: chunks
/// of `+8` nodes of `+0x1c` bytes after a `+0x38`-byte header, the chunks in use from `+0xc` to
/// `+0x10`, the spare ones from `+0x14`, the next node of the last chunk at `+0x28` with `+0x18` of
/// it taken, the free chain at `+0x24`.
mod query_pool {
    use ps2_oracle::Ee;

    /// A node: the next of the last chunk, else off the free chain, else the first spare chunk (a
    /// new chunk off the heap the gather never needs here: it panics).
    pub fn take(ee: &mut Ee, p: u32) -> u32 {
        let (cap, used, stride) = (ee.i32_at(p + 8), ee.i32_at(p + 0x18), ee.u32_at(p + 0x1c));
        if used < cap {
            let n = ee.u32_at(p + 0x28);
            ee.set_u32(p + 0x28, n + stride);
            ee.set_u32(p + 0x18, (used + 1) as u32);
            return n;
        }
        let free = ee.u32_at(p + 0x24);
        if free != 0 {
            ee.set_u32(p + 0x24, ee.u32_at(free));
            return free;
        }
        let spare = ee.u32_at(p + 0x14);
        assert_ne!(spare, 0, "the query's pool {p:#x} wants a chunk off the heap: not modelled");
        if ee.u32_at(p + 0xc) == 0 {
            ee.set_u32(p + 0x10, spare);
            ee.set_u32(p + 0xc, spare);
        } else {
            let last = ee.u32_at(p + 0x10);
            ee.set_u32(last, spare);
            ee.set_u32(p + 0x10, spare);
        }
        ee.set_u32(p + 0x14, ee.u32_at(spare));
        ee.set_u32(spare, 0);
        let n = spare + ee.u32_at(p + 0x38);
        ee.set_u32(p + 0x28, n + stride);
        ee.set_u32(p + 0x18, 1);
        n
    }

    /// Every node given back: the chunks in use put before the spare ones.
    pub fn reset(ee: &mut Ee, p: u32) {
        let first = ee.u32_at(p + 0xc);
        if first != 0 {
            let (spare, last) = (ee.u32_at(p + 0x14), ee.u32_at(p + 0x10));
            if spare != 0 && last != 0 {
                ee.set_u32(last, spare);
            }
            ee.set_u32(p + 0x14, first);
        }
        ee.set_u32(p + 0xc, 0);
        ee.set_u32(p + 0x10, 0);
        ee.set_u32(p + 0x18, ee.u32_at(p + 8));
        ee.set_u32(p + 0x24, 0);
    }
}

/// Where the world [`GatherInRam::read`] read is in RAM: the entries by item (a collidable's is its
/// `+0x18`), the space links, tables and structures in the model's order, and per structure its
/// tree nodes, their chunks and the open query's list nodes.
#[derive(Default)]
struct GatherInRam {
    items: Vec<u32>,
    links: Vec<u32>,
    tables: Vec<u32>,
    structures: Vec<u32>,
    nodes: Vec<Vec<u32>>,
    chunks: Vec<Vec<Vec<u32>>>,
    listed: Vec<Vec<u32>>,
    current: u32,
}

impl GatherInRam {
    fn item(&mut self, e: u32) -> usize {
        assert_ne!(e, 0, "an entry at 0");
        self.items.iter().position(|&x| x == e).unwrap_or_else(|| {
            self.items.push(e);
            self.items.len() - 1
        })
    }

    fn table(&mut self, t: u32) -> usize {
        self.tables.iter().position(|&x| x == t).unwrap_or_else(|| {
            self.tables.push(t);
            self.tables.len() - 1
        })
    }

    fn structure(&mut self, s: u32) -> Option<usize> {
        (s != 0).then(|| {
            self.structures.iter().position(|&x| x == s).unwrap_or_else(|| {
                self.structures.push(s);
                self.structures.len() - 1
            })
        })
    }

    fn pair(&mut self, ee: &Ee, t: u32) -> bp::Pair {
        bp::Pair { fixed: self.structure(ee.u32_at(t + 0x28)), moving: self.structure(ee.u32_at(t + 0x2c)) }
    }

    fn objects(&mut self, ee: &Ee, l: u32) -> ws::Objects {
        let (mode, at, n) = (ee.u32_at(l), ee.u32_at(l + 4), ee.u32_at(l + 8));
        ws::Objects { mode, list: (0..n).map(|i| self.item(ee.u32_at(at + 4 * i) + 0x18)).collect() }
    }

    /// Tree node `n` of structure `s` and every node under it, numbered as met.
    fn tree_node(&mut self, ee: &Ee, s: usize, n: u32, t: &mut bp::Quadtree) -> usize {
        let k = t.nodes.len();
        t.nodes.push(bp::Node::default());
        self.nodes[s].push(n);
        self.chunks[s].push(vec![]);
        let mut c = ee.u32_at(n + 4);
        while c != 0 {
            let slots = std::array::from_fn(|j| match ee.u32_at(c + 4 + 4 * j as u32) {
                0 => None,
                e => Some(self.item(e)),
            });
            t.nodes[k].chunks.push(slots);
            self.chunks[s][k].push(c);
            assert!(self.chunks[s][k].len() < 4096, "node {n:#x}'s chunks do not end");
            c = ee.u32_at(c);
        }
        let children = ee.u32_at(n);
        if children != 0 {
            for i in 0..4 {
                let ch = ee.u32_at(children + 4 * i as u32);
                if ch != 0 {
                    t.nodes[k].children[i] = Some(self.tree_node(ee, s, ch, t));
                }
            }
        }
        k
    }

    fn tree(&mut self, ee: &Ee, s: usize) -> bp::Quadtree {
        let a = self.structures[s];
        let mut t = bp::Quadtree {
            flags: ee.u32_at(a + 0xc),
            origin: [word_f(ee, a + 0x28), word_f(ee, a + 0x30)],
            extent: [word_f(ee, a + 0x40), word_f(ee, a + 0x44)],
            cell: ee.i32_at(a + 0x50),
            scale: word_f(ee, a + 0x54),
            busy: ee.u32_at(a + 0x890),
            cells: [0x8a0, 0x8a4, 0x8a8, 0x8ac].map(|o| ee.i32_at(a + o)),
            slot: ee.u32_at(a + 0xa4) as usize,
            ..Default::default()
        };
        let root = ee.u32_at(a + 0x8c);
        if root != 0 {
            t.root = Some(self.tree_node(ee, s, root, &mut t));
        }
        let mut l = ee.u32_at(a + 0x9c);
        while l != 0 {
            let n = ee.u32_at(l + 4);
            t.list.push(self.nodes[s].iter().position(|&x| x == n).unwrap_or_else(|| panic!("listed node {n:#x} is none of the tree's")));
            self.listed[s].push(l);
            assert!(t.list.len() < 4096, "the open query's list does not end");
            l = ee.u32_at(l);
        }
        t.head = (!t.list.is_empty()).then_some(0);
        t.at = match ee.u32_at(a + 0xa0) {
            0 => None,
            at => Some(self.listed[s].iter().position(|&x| x == at).unwrap_or_else(|| panic!("the query is at {at:#x}, off its list"))),
        };
        t.chunk = match ee.u32_at(a + 0xa8) {
            0 => None,
            c => Some(
                self.chunks[s]
                    .iter()
                    .enumerate()
                    .find_map(|(n, cs)| cs.iter().position(|&x| x == c).map(|k| (n, k)))
                    .unwrap_or_else(|| panic!("the query's chunk {c:#x} is none of the tree's")),
            ),
        };
        t
    }

    fn list(&mut self, ee: &Ee, s: usize) -> bp::Blocks {
        let a = self.structures[s];
        let (size, last, arr) = (ee.i32_at(a + 0x28), ee.i32_at(a + 0x30), ee.u32_at(a + 0x34));
        let blocks = (arr != 0).then(|| {
            let n = ee.u32_at(a + 0x2c);
            (0..n)
                .map(|b| {
                    let base = ee.u32_at(arr + 4 * b);
                    let used = if b + 1 == n { last } else { size };
                    (0..size)
                        .map(|i| match ee.u32_at(base + 4 * i as u32) {
                            _ if i >= used => None,
                            w if w & 2 != 0 => None,
                            w => Some(self.item(w)),
                        })
                        .collect()
                })
                .collect()
        });
        bp::Blocks { flags: ee.u32_at(a + 0xc), size, last, blocks, block: ee.i32_at(a + 0x3c), entry: ee.i32_at(a + 0x40) }
    }

    /// The world the gather round `probe` (a collidable) walks, as [`bp::Broad`] and
    /// [`ws::Collidable`]s model it: the probe's chain of spaces, their tables and the global one,
    /// each table's structures (the quadtree node by node, the list of moving entries slot by
    /// slot, and nothing else), and every entry they hold, with the collidables their object lists
    /// name. The probe is item 0.
    fn read(ee: &Ee, probe: u32) -> (GatherInRam, Vec<ws::Collidable>, bp::Broad) {
        let mut m = GatherInRam::default();
        m.item(probe + 0x18);
        let mut broad = bp::Broad { stamp: ee.u16_at(STAMP_AT), cell_size: word_f(ee, CELL_SIZE_AT), ..Default::default() };
        let mut l = ee.u32_at(probe + 0x20);
        while l != 0 {
            m.links.push(l);
            assert!(m.links.len() < 64, "the probe's spaces do not end");
            l = ee.u32_at(l);
        }
        for k in 0..m.links.len() {
            let tables = m.table(ee.u32_at(m.links[k] + 4));
            broad.spaces.push(bp::Space { next: (k + 1 < m.links.len()).then_some(k + 1), tables });
        }
        let global = ee.u32_at(GLOBAL_AT);
        broad.global = (global != 0).then(|| m.table(global));
        let mut k = 0;
        while k < m.tables.len() {
            let t = m.tables[k];
            let own = m.pair(ee, t);
            let (subs, n) = (ee.u32_at(t + 0x38), ee.u32_at(t + 0x3c));
            let subs = (0..n).map(|i| m.pair(ee, ee.u32_at(subs + 4 * i))).collect();
            broad.tables.push(bp::Tables { own, subs });
            k += 1;
        }
        m.nodes = vec![vec![]; m.structures.len()];
        m.chunks = vec![vec![]; m.structures.len()];
        m.listed = vec![vec![]; m.structures.len()];
        for s in 0..m.structures.len() {
            let a = m.structures[s];
            broad.structures.push(match ee.u32_at(a + 4) {
                TREE_VT => bp::Structure::Tree(m.tree(ee, s)),
                DB_LIST_VT => bp::Structure::List(m.list(ee, s)),
                vt => panic!("structure {a:#x} (vtable {vt:#x}) is neither the quadtree nor the list"),
            });
        }
        let trees = broad.structures.iter().filter(|s| matches!(s, bp::Structure::Tree(_))).count();
        assert!(trees <= 1, "the probe's tables hold {trees} quadtrees: the write-back follows one");
        m.current = ee.u32_at(CURRENT_AT);
        broad.current = m.structures.iter().position(|&x| x == m.current);
        let mut objects = vec![];
        let mut k = 0;
        while k < m.items.len() {
            let e = m.items[k];
            let class = ws::Class::from_byte(ee.u8_at(e + 2));
            let mut place = bp::Placement { stamp: ee.u16_at(e), ..Default::default() };
            let mut o = ws::Collidable { class, ..Default::default() };
            match class {
                ws::Class::Object => {
                    let c = e - 0x18;
                    place.centre = [0, 4, 8, 12].map(|i| word_f(ee, c + 0x50 + i));
                    o.flags = ee.u8_at(c + 0x126);
                    o.nearby = ee.u8_at(c + 0x45) & 1 != 0;
                    o.radius = word_f(ee, c + 0x70);
                    o.category = ee.u32_at(c + 0x94);
                    o.collides_with = ee.u32_at(c + 0xdc);
                    o.objects = m.objects(ee, c + 0xe0);
                    o.more_objects = m.objects(ee, c + 0xec);
                    let cs = c + 0xf8;
                    o.categories = ws::Categories { mode: ee.u32_at(cs), list: (0..ee.u32_at(cs + 8)).map(|i| ee.u32_at(ee.u32_at(cs + 4) + 4 * i)).collect() };
                }
                ws::Class::Group => {
                    place.cell = [ee.u16_at(e + 0xc), ee.u16_at(e + 0x10)];
                    place.model = ee.u8_at(e + 0x12);
                    place.blend = ee.u8_at(e + 0x15) as i8;
                    let eco = ee.u32_at(ECO_AT);
                    broad.group_cell = word_f(ee, eco + 0xc4);
                    let mi = usize::from(place.model);
                    if broad.models.len() <= mi {
                        broad.models.resize(mi + 1, bp::Model::default());
                    }
                    let model = ee.u32_at(ee.u32_at(eco + 0x84) + 4 * mi as u32);
                    broad.models[mi] = bp::Model { near: word_f(ee, model + 0x10c), far: word_f(ee, model + 0x114) };
                }
                ws::Class::Other(_) => {}
            }
            objects.push(o);
            broad.places.push(place);
            k += 1;
        }
        broad.places[0].space = (!broad.spaces.is_empty()).then_some(0);
        (m, objects, broad)
    }

    /// The gather's outcome (`now`, read as `was`; `begun` the tree just after the query began, to
    /// tell a fresh list from a restarted one) written back where it changed: each entry's stamp and
    /// each collidable's flag byte, the global stamp and the tree last queried, every structure's
    /// walk state, the tree query's nodes taken from its pool and given back as the original takes
    /// and gives them, and the nearby list. What the gather never changes — the spaces, the tables,
    /// the trees' nodes, the lists' slots, every other collidable field — must not have changed.
    /// Answers how the tree's query went: `fresh`, `restarted`, or `off` the tree.
    fn write(&self, ee: &mut Ee, list: u32, was: (&[ws::Collidable], &bp::Broad), now: (&[ws::Collidable], &bp::Broad, &atv_world::Nearby), begun: &bp::Broad) -> &'static str {
        let (no, nb, nl) = now;
        let how = self.write_query(ee, was, (no, nb), begun);
        let arr = ee.u32_at(list);
        for (i, c) in nl.found.iter().enumerate() {
            ee.set_u32(arr + 8 * i as u32, self.items[c.object]);
            ee.set_u32(arr + 8 * i as u32 + 4, c.distance.0.to_bits());
        }
        ee.set_u32(list + 4, nl.found.len() as u32);
        for (i, w) in nl.centre.iter().enumerate() {
            ee.set_u32(list + 0x10 + 4 * i as u32, w.0.to_bits());
        }
        ee.set_u32(list + 0x20, nl.bound.0.to_bits());
        ee.set_u32(list + 0x30, u32::from(nl.built));
        how
    }

    /// [`GatherInRam::write`] without the nearby list: a query's outcome written back (F51's global
    /// search runs two queries and fills no list).
    fn write_query(&self, ee: &mut Ee, was: (&[ws::Collidable], &bp::Broad), now: (&[ws::Collidable], &bp::Broad), begun: &bp::Broad) -> &'static str {
        let mut how = "off";
        let (wo, wb) = was;
        let (no, nb) = now;
        assert_eq!((&wb.spaces, &wb.tables, wb.global), (&nb.spaces, &nb.tables, nb.global), "the spaces changed");
        for (k, (a, b)) in wo.iter().zip(no).enumerate() {
            let e = self.items[k];
            assert_eq!(ws::Collidable { flags: b.flags, ..a.clone() }, *b, "collidable {:#x} changed beyond its flags", e - 0x18);
            if a.flags != b.flags {
                set_u8(ee, e - 0x18 + 0x126, b.flags);
            }
            let (pa, pb) = (&wb.places[k], &nb.places[k]);
            assert_eq!(bp::Placement { stamp: pb.stamp, ..pa.clone() }, *pb, "entry {e:#x}'s place changed beyond its stamp");
            if pa.stamp != pb.stamp {
                set_u16(ee, e, pb.stamp);
            }
        }
        if wb.stamp != nb.stamp {
            set_u16(ee, STAMP_AT, nb.stamp);
        }
        if wb.current != nb.current {
            ee.set_u32(CURRENT_AT, nb.current.map_or(self.current, |s| self.structures[s]));
        }
        for (s, (sa, sb)) in wb.structures.iter().zip(&nb.structures).enumerate() {
            let a = self.structures[s];
            match (sa, sb, &begun.structures[s]) {
                (bp::Structure::Tree(x), bp::Structure::Tree(y), bp::Structure::Tree(z)) => {
                    assert_eq!((&x.nodes, x.root), (&y.nodes, y.root), "tree {a:#x}'s nodes changed");
                    let was_open = x.flags & 1 != 0;
                    let stamped = begun.stamp != wb.stamp;
                    let restart = was_open && z.flags & 1 != 0 && z.cells == x.cells && stamped;
                    let fresh = z.flags & 1 != 0 && stamped && !restart;
                    let pool = ee.u32_at(a + 0x94);
                    how = if restart { "restarted" } else if fresh { "fresh" } else { "off" };
                    if was_open && fresh {
                        query_pool::reset(ee, pool);
                    }
                    let mut taken = vec![];
                    if fresh {
                        let mut prev = 0;
                        for &n in z.list.iter().rev() {
                            let at = query_pool::take(ee, pool);
                            ee.set_u32(at, prev);
                            ee.set_u32(at + 4, self.nodes[s][n]);
                            prev = at;
                            taken.insert(0, at);
                        }
                    }
                    // A list the walk left open (F51's search ends so) keeps the nodes just taken.
                    let left_open = (fresh || restart || !stamped) && !y.list.is_empty() && y.list == z.list;
                    if (was_open || fresh) && !left_open {
                        query_pool::reset(ee, pool);
                    }
                    assert!(y.list.is_empty() || y.list == x.list || left_open, "tree {a:#x}'s query left a list open");
                    let open = if left_open && fresh { &taken } else { &self.listed[s] };
                    let listed = |i: Option<usize>| i.map_or(0, |i| open[i]);
                    ee.set_u32(a + 0xc, y.flags);
                    ee.set_u32(a + 0x890, y.busy);
                    for (i, c) in y.cells.iter().enumerate() {
                        ee.set_u32(a + 0x8a0 + 4 * i as u32, *c as u32);
                    }
                    ee.set_u32(a + 0x9c, listed(y.head));
                    ee.set_u32(a + 0xa0, listed(y.at));
                    ee.set_u32(a + 0xa4, y.slot as u32);
                    ee.set_u32(a + 0xa8, y.chunk.map_or(0, |(n, c)| self.chunks[s][n][c]));
                    if was_open || stamped {
                        ee.set_u32(a + 0xac, 0);
                    }
                }
                (bp::Structure::List(x), bp::Structure::List(y), _) => {
                    assert_eq!((x.size, x.last, &x.blocks), (y.size, y.last, &y.blocks), "list {a:#x}'s slots changed");
                    ee.set_u32(a + 0xc, y.flags);
                    ee.set_u32(a + 0x3c, y.block as u32);
                    ee.set_u32(a + 0x40, y.entry as u32);
                }
                _ => panic!("structure {a:#x} changed its kind"),
            }
        }
        how
    }
}

/// The nearby list at `l` (the bike's `+0xc4`) as [`atv_world::Nearby`] holds it, its candidates
/// left out (the gather starts it afresh).
fn nearby_in_ram(ee: &Ee, l: u32) -> atv_world::Nearby {
    atv_world::Nearby {
        found: vec![],
        centre: [0, 4, 8, 12].map(|i| word_f(ee, l + 0x10 + i)),
        bound: word_f(ee, l + 0x20),
        least: word_f(ee, l + 0x24),
        extra: word_f(ee, l + 0x28),
        no_groups: ee.u32_at(l + 0x2c) != 0,
        built: ee.u32_at(l + 0x30) != 0,
        capacity: ee.i32_at(l + 0x34),
        filter: ee.u32_at(l + 0x38) != 0,
        flat: ee.u32_at(l + 0x3c) != 0,
    }
}

/// **F46b — the collision sync's gather in Rust**: the world round `probe` (a collidable) and the
/// nearby list at `list` read out of RAM, [`atv_world::gather`] run with `kind`, and its marks,
/// stamps, walk state and list written back.
fn gather_in_ram(ee: &mut Ee, list: u32, probe: u32, kind: u32) -> &'static str {
    let (m, mut objects, mut broad) = GatherInRam::read(ee, probe);
    let mut near = nearby_in_ram(ee, list);
    let (was_o, was_b) = (objects.clone(), broad.clone());
    // The query's first structure begun as the gather begins it, on a copy: whether the tree's list
    // is fresh, and which.
    let mut begun = broad.clone();
    let space = begun.places[0].space.expect("a probe in a space");
    let (centre, l) = (begun.places[0].centre, near.least);
    let (mut lo, mut hi) = ([F::ZERO; 4], [F::ZERO; 4]);
    bp::Query::begin(&mut begun, space, kind, centre, [l, l, l, F::ONE], Some((&mut lo, &mut hi)), true);
    atv_world::gather(&mut objects, &mut broad, &mut near, 0, kind);
    m.write(ee, list, (&was_o, &was_b), (&objects, &broad, &near), &begun)
}

/// F46c: the sweep probe's clear's globals — the two words it zeroes, and where it notes the
/// probe's record.
const SWEEP_ZEROED: [u32; 2] = [0x003d_ce28, 0x003d_ce20];
const SWEEP_RECORD_AT: u32 = 0x003d_c618;

/// The sweep's narrow phase past its clear, none of it Rust's: on C15 no pair is tested and no group
/// is searched (the probe's `NO_GROUPS`), so none of it is reached.
struct Unported;

impl ws::Narrow for Unported {
    fn clear(&mut self, _: &mut [ws::Collidable], _: usize) {
        unreachable!("the sweep's own clear")
    }
    fn prepare(&mut self, _: &mut [ws::Collidable], _: usize) {
        panic!("the sweep prepared a grid")
    }
    fn pair(&mut self, _: &mut [ws::Collidable], _: usize, other: usize) -> bool {
        panic!("the sweep tested a pair (item {other}): its pair test is not Rust's")
    }
    fn parts(&mut self, _: &mut [ws::Collidable], group: usize) -> usize {
        panic!("the sweep searched a group (item {group})")
    }
    fn part(&mut self, _: &mut [ws::Collidable], group: usize, _: usize) -> Option<usize> {
        panic!("the sweep searched a group (item {group})")
    }
    fn heard_hit(&mut self, _: &mut [ws::Collidable], _: usize, _: usize, _: usize) {
        panic!("a listener heard the sweep hit")
    }
    fn heard_struck(&mut self, _: &mut [ws::Collidable], _: usize, _: usize, _: usize) {
        panic!("a listener heard the sweep strike")
    }
    fn on_hit(&mut self, _: &mut [ws::Collidable], _: usize, _: usize, _: usize) {
        panic!("the sweep called on a hit")
    }
    fn on_struck(&mut self, _: &mut [ws::Collidable], _: usize, _: usize, _: usize) {
        panic!("the sweep called on a strike")
    }
}

impl game_sim::atv_sweep::Outside for Unported {
    fn target(&mut self, other: usize, _: &mut dyn FnMut(Option<&game_sim::sweep_probe::Target>) -> bool) -> bool {
        panic!("the sweep paired item {other}")
    }
}

/// The nearby search's world round `probe` (a collidable): the probe as item 0, the nearby list at
/// `list`'s candidates, and the collidables their object lists name, with the filter's fields; and
/// the candidates by item. Answers the entries by item too.
fn sweep_world_in_ram(ee: &Ee, probe: u32, list: u32) -> (Vec<u32>, Vec<ws::Collidable>, Vec<ws::Candidate>) {
    let mut m = GatherInRam::default();
    m.item(probe + 0x18);
    let (arr, n) = (ee.u32_at(list), ee.u32_at(list + 4));
    let found = (0..n).map(|i| ws::Candidate { object: m.item(ee.u32_at(arr + 8 * i)), distance: word_f(ee, arr + 8 * i + 4) }).collect();
    let mut objects = vec![];
    let mut k = 0;
    while k < m.items.len() {
        let e = m.items[k];
        let mut o = ws::Collidable { class: ws::Class::from_byte(ee.u8_at(e + 2)), ..Default::default() };
        if o.class == ws::Class::Object {
            let c = e - 0x18;
            o.flags = ee.u8_at(c + 0x126);
            o.nearby = ee.u8_at(c + 0x45) & 1 != 0;
            o.radius = word_f(ee, c + 0x70);
            o.category = ee.u32_at(c + 0x94);
            o.collides_with = ee.u32_at(c + 0xdc);
            o.objects = m.objects(ee, c + 0xe0);
            o.more_objects = m.objects(ee, c + 0xec);
            let cs = c + 0xf8;
            o.categories = ws::Categories { mode: ee.u32_at(cs), list: (0..ee.u32_at(cs + 8)).map(|i| ee.u32_at(ee.u32_at(cs + 4) + 4 * i)).collect() };
        }
        objects.push(o);
        k += 1;
    }
    (m.items, objects, found)
}

/// **F46c — the record's sweep in Rust**: the segment from `from` to `to` set on the record `rec`'s
/// sweep probe `probe` ([`sweep_set_in_ram`]) and searched over the nearby list `list`
/// ([`atv_search::search`] with [`atv_sweep::Sweep`]: since F51 its pairs too, over
/// [`SweepWorldInRam`]); everything written back. Answers the hit bit.
fn sweep_in_ram(ee: &mut Ee, rec: u32, probe: u32, list: u32, from: Vf, to: Vf, kinds: &mut BTreeMap<String, u32>) -> bool {
    use game_sim::atv_search;
    use game_sim::atv_sweep;
    let segments = sweep_set_in_ram(ee, rec, probe, from, to);
    // The search.
    assert_eq!(ee.u32_at(list + 0x40), 0, "the nearby list has a grid: the sweep would prepare it");
    let (mut items, mut objects, found) = sweep_world_in_ram(ee, probe, list);
    // The plant groups' parts, as collidables of their own.
    for k in 0..objects.len() {
        if objects[k].class != ws::Class::Group {
            continue;
        }
        for p in PartWorldInRam::species_parts(ee, items[k]) {
            if p != 0 && !items.contains(&(p + 0x18)) {
                items.push(p + 0x18);
                objects.push(ws::Collidable { class: ws::Class::from_byte(ee.u8_at(p + 0x18 + 2)), flags: ee.u8_at(p + 0x126), ..Default::default() });
            }
        }
    }
    let retired: Vec<bool> = items.iter().map(|&e| ee.u32_at(e - 0x18) == 1).collect();
    let count = segments.len();
    let r = ee.u32_at(probe + 0x7c);
    let (answer, more) = sweep_answer_in_ram(ee, r, count, &mut items);
    let mut port = SweepProbe { segments: segments.clone(), answer, ..SweepProbe::default() };
    sweep_local_in_ram(ee, probe, count, &mut port);
    let mut stood = port.clone();
    sweep_probe_clear(&mut stood);
    let mut world = PartWorldInRam { ee: &mut *ee, items, mesh: None, kinds: BTreeMap::new(), listeners: vec![], callbacks: vec![] };
    for (k, o) in objects.iter_mut().enumerate() {
        if o.class == ws::Class::Object {
            let e = world.items[k];
            world.hear(o, e - 0x18);
        }
    }
    let heard: Vec<ws::Collidable> = objects.clone();
    let mut outside = SweepWorldInRam { world: &mut world, centre: stood.world.centre, radius: stood.world.radius, pairs: 0, descriptor: None };
    let mut sweep = atv_sweep::Sweep::new(segments, more, &mut outside);
    sweep.probe = port;
    let hit = atv_search::search(&mut objects, 0, &found, &|o: usize| retired.get(o).copied().unwrap_or(false), &mut sweep);
    let (port, more) = (sweep.probe, sweep.more);
    let (pairs, descriptor) = (outside.pairs, outside.descriptor);
    for (k, v) in std::mem::take(&mut world.kinds) {
        *kinds.entry(k).or_default() += v;
    }
    let items = world.items;
    for (k, (a, b)) in heard.iter().zip(&objects).enumerate() {
        assert_eq!(ws::Collidable { flags: b.flags, radius: b.radius, group: b.group, ..a.clone() }, *b, "item {k} changed beyond its flags and reach");
        if a.flags != b.flags {
            assert_eq!(a.class, ws::Class::Object, "item {k}'s flags");
            set_u8(ee, items[k] - 0x18 + 0x126, b.flags);
        }
        if let Some(g) = b.group {
            ee.set_u32(items[k] - 0x18 + 0x88, items[g]);
        }
    }
    // The clear's world form and the record's reset.
    let w = &port.world;
    let [lengths, starts, dirs] = [0x144, 0x148, 0x14c].map(|o| ee.u32_at(probe + o));
    for (i, p) in w.probes[..w.count].iter().enumerate() {
        let i = i as u32;
        ee.set_u32(lengths + 4 * i, p.length.0.to_bits());
        set_vf(ee, starts + 0x10 * i, p.origin);
        set_vf(ee, dirs + 0x10 * i, p.dir);
    }
    for o in [0x160, 0x50] {
        set_vf(ee, probe + o, w.centre);
    }
    assert_eq!(objects[0].radius, w.radius);
    for o in [0x170, 0x70] {
        ee.set_u32(probe + o, w.radius.0.to_bits());
    }
    for a in SWEEP_ZEROED {
        ee.set_u32(a, 0);
    }
    ee.set_u32(SWEEP_RECORD_AT, r);
    sweep_pairs_write(ee, probe, count, &port, pairs, descriptor);
    write_sweep_answer(ee, r, count, &items, &port.answer, &more);
    hit
}

/// The probe's local forms (each segment's origin and direction in the last paired object's
/// frame, and the centre) into `port`, as RAM has them.
fn sweep_local_in_ram(ee: &Ee, probe: u32, count: usize, port: &mut SweepProbe) {
    use f51_probe as pa;
    assert_eq!(ee.u32_at(probe + pa::SMOOTH), 0, "the probe asks for smoothed normals: not ported");
    let (locals, ldirs) = (ee.u32_at(probe + pa::LOCAL_ORIGINS), ee.u32_at(probe + pa::LOCAL_DIRS));
    for i in 0..count {
        port.local.probes[i].origin = vf_at(ee, locals + 0x10 * i as u32);
        port.local.probes[i].dir = vf_at(ee, ldirs + 0x10 * i as u32);
    }
    port.local.centre = vf_at(ee, probe + pa::LOCAL_CENTRE);
}

/// What the pairs write beyond the answer, where any ran: the smoothing word noted in its global,
/// the local forms, and the last swept mesh's descriptor in its global.
fn sweep_pairs_write(ee: &mut Ee, probe: u32, count: usize, port: &SweepProbe, pairs: u32, descriptor: Option<u32>) {
    use f51_probe as pa;
    if pairs > 0 {
        let (locals, ldirs) = (ee.u32_at(probe + pa::LOCAL_ORIGINS), ee.u32_at(probe + pa::LOCAL_DIRS));
        ee.set_u32(SWEEP_SMOOTH_AT, ee.u32_at(probe + pa::SMOOTH));
        for i in 0..count as u32 {
            set_vf(ee, locals + 0x10 * i, port.local.probes[i as usize].origin);
            set_vf(ee, ldirs + 0x10 * i, port.local.probes[i as usize].dir);
        }
        set_vf(ee, probe + pa::LOCAL_CENTRE, port.local.centre);
    }
    if let Some(d) = descriptor {
        ee.set_u32(SWEEP_DESCRIPTOR_AT, d);
    }
}

/// F46c's set and place in Rust: the segment from `from` to `to` (the record `rec`'s own, written at
/// `+0xc0`) set on its sweep probe `probe` ([`atv_sweep::set`]), and the probe placed (its box, and
/// its database entry moved, [`spatial_move_in_ram`]). Answers the segments.
fn sweep_set_in_ram(ee: &mut Ee, rec: u32, probe: u32, from: Vf, to: Vf) -> Vec<game_sim::sweep_probe::Segment> {
    use game_sim::atv_sweep;
    set_vf(ee, rec + 0xc0, from);
    set_vf(ee, rec + 0xd0, to);
    // The set: the record's segment the probe's, no nodes.
    let n = ee.u32_at(probe + 0x130);
    let points: Vec<[Vf; 2]> = (0..n).map(|i| [vf_at(ee, rec + 0xc0 + 0x20 * i), vf_at(ee, rec + 0xd0 + 0x20 * i)]).collect();
    let (segments, b) = atv_sweep::set(&points, None);
    let (nodes, quads) = (ee.u32_at(probe + 0x134), ee.u32_at(probe + 0x138));
    for (i, s) in segments.iter().enumerate() {
        let i = i as u32;
        for k in 0..2 {
            ee.set_u32(nodes + 4 * (2 * i + k), 0);
            set_vf(ee, quads + 0x10 * (2 * i + k), s.ends[k as usize]);
        }
    }
    set_vf(ee, probe + 0x50, b.centre);
    for o in [0x170, 0x70] {
        ee.set_u32(probe + o, b.reach.0.to_bits());
    }
    // The place: its box again, and its entry moved.
    if ee.u32_at(probe + 0x20) != 0 {
        let b = atv_sweep::bounds(&segments);
        set_vf(ee, probe + 0x50, b.centre);
        ee.set_u32(probe + 0x70, b.reach.0.to_bits());
        spatial_move_in_ram(ee, probe + 0x18, b.centre, b.half, b.reach);
    }
    segments
}

/// A ray probe's or a mesh's tree as it sits in RAM, 36 bytes a node (the key, the two child words
/// at `+8`, the centre at `+0xc`, the half extents at `+0x18`), read from `root` down; and one past
/// its highest leaf.
fn tree_in_ram(ee: &Ee, nodes: u32, root: u16) -> (Vec<track::Node>, usize) {
    let blank = track::Node { volume: F::ZERO, centre: [F::ZERO; 3], half_extent: [F::ZERO; 3], children: [0, 0] };
    let (mut out, mut leaves, mut todo) = (Vec::new(), 0, vec![root]);
    while let Some(w) = todo.pop() {
        if w & 0x8000 != 0 {
            leaves = leaves.max(usize::from(w & 0x7fff) + 1);
            continue;
        }
        let n = nodes + 36 * u32::from(w);
        if out.len() <= usize::from(w) {
            out.resize(usize::from(w) + 1, blank);
        }
        let three = |a: u32| [0, 4, 8].map(|k| word_f(ee, a + k));
        let children = [ee.u16_at(n + 8), ee.u16_at(n + 10)];
        out[usize::from(w)] = track::Node { volume: word_f(ee, n), centre: three(n + 12), half_extent: three(n + 24), children };
        todo.extend(children);
    }
    (out, leaves)
}

fn matrix_at(ee: &Ee, a: u32) -> ray_probe::Matrix {
    [0, 1, 2, 3].map(|k| vf_at(ee, a + 0x10 * k))
}

/// A mesh collidable (kind 0) as the part's pair reads it: its frame (`+0x130`), its tree's root
/// (`+0x23c`), nodes (`+0x244`) and triangles (`+0x248`, 0x14 bytes each: three vertex shorts, the
/// packed normal at `+0xc`), and its vertices through the descriptor at `+0x230` (the array, and at
/// `+8` whether they are packed shorts).
struct MeshInRam {
    id: u32,
    transform: ray_probe::Matrix,
    root: u16,
    nodes: Vec<track::Node>,
    facets: Vec<ray_probe::Facet>,
    vertices: track::Points,
}

impl MeshInRam {
    fn read(ee: &Ee, c: u32) -> MeshInRam {
        assert_eq!(ee.u32_at(c + 0x78), 0, "{c:#x} is not a mesh");
        let root = ee.u16_at(c + 0x23c);
        let (nodes, count) = tree_in_ram(ee, ee.u32_at(c + 0x244), root);
        let at = ee.u32_at(c + 0x248);
        let facets: Vec<ray_probe::Facet> = (0..count as u32)
            .map(|i| {
                let f = at + 0x14 * i;
                ray_probe::Facet { vertices: [0, 2, 4].map(|k| ee.u16_at(f + k)), normal: [0, 2, 4].map(|k| ee.u16_at(f + 0xc + k) as i16) }
            })
            .collect();
        let corners = facets.iter().flat_map(|f| f.vertices).max().map_or(0, |m| u32::from(m) + 1);
        let d = c + 0x230;
        let array = ee.u32_at(d);
        let vertices = if ee.u32_at(d + 8) != 0 {
            track::Points::Packed((0..corners).map(|i| [0, 2, 4].map(|k| ee.u16_at(array + 6 * i + k) as i16)).collect())
        } else {
            track::Points::Wide((0..corners).map(|i| [0, 4, 8].map(|k| word_f(ee, array + 12 * i + k))).collect())
        };
        MeshInRam { id: c, transform: matrix_at(ee, c + 0x130), root, nodes, facets, vertices }
    }

    fn mesh(&self) -> ray_probe::Mesh<'_> {
        ray_probe::Mesh { id: self.id, transform: self.transform, root: self.root, nodes: &self.nodes, facets: &self.facets, vertices: &self.vertices }
    }
}

/// A plant group's part `i` stood at the plant (`(group entry, i)`, answers the part or 0).
const ECO_PART: u32 = 0x0046_d3c0;

/// The world of a part search in RAM: what each item is to the pair, read from RAM at each pair (a
/// mesh, kind 0; a hierarchy, kind 1, not read: C15 never pairs one; anything else nothing struck),
/// and a plant group's parts, each stood at the plant by the original ([`ECO_PART`], handed on).
/// Counts the pairs by kind, and the stands.
struct PartWorldInRam<'e> {
    ee: &'e mut Ee,
    items: Vec<u32>,
    mesh: Option<MeshInRam>,
    kinds: BTreeMap<String, u32>,
    /// The listeners and the callbacks the collidables name ([`ws::Collidable::listeners`], `on_hit`,
    /// `on_struck` index these), each called in the original (handed on).
    listeners: Vec<u32>,
    callbacks: Vec<u32>,
}

impl PartWorldInRam<'_> {
    /// Calls the original's `f` with `args` (collidables by item, as their bases), counted as `what`,
    /// the probe's and the other's flags written first (the hit bit is up when they run). A listener
    /// must leave both listener lists as they were: the search's are read once.
    fn hand_on(&mut self, objects: &[ws::Collidable], what: &str, f: u32, this: u32, a: usize, b: usize) {
        *self.kinds.entry(format!("> record search {what}")).or_default() += 1;
        let [ca, cb] = [a, b].map(|k| self.items[k] - 0x18);
        for (k, c) in [(a, ca), (b, cb)] {
            set_u8(self.ee, c + 0x126, objects[k].flags);
        }
        let counts = [ca, cb].map(|c| self.ee.u32_at(c + 0x104));
        self.ee.set(28, GP);
        let args: Vec<u64> = if this == 0 { vec![u64::from(ca), u64::from(cb)] } else { vec![u64::from(this), u64::from(ca), u64::from(cb)] };
        self.ee.call(f, &args, &[], BUDGET);
        assert_eq!([ca, cb].map(|c| self.ee.u32_at(c + 0x104)), counts, "{what} changed a listener list");
    }

    /// The listeners (`+0x104` the count, `+0x10c` the array) and the callbacks (`+0x11c` on a hit,
    /// `+0x120` on being struck) of the collidable at `c`, into `o`.
    fn hear(&mut self, o: &mut ws::Collidable, c: u32) {
        let (n, arr) = (self.ee.u32_at(c + 0x104), self.ee.u32_at(c + 0x10c));
        for j in 0..n {
            o.listeners.push(self.listeners.len());
            self.listeners.push(self.ee.u32_at(arr + 4 * j));
        }
        for (slot, at) in [(&mut o.on_hit, 0x11c), (&mut o.on_struck, 0x120)] {
            let f = self.ee.u32_at(c + at);
            if f != 0 {
                *slot = Some(self.callbacks.len());
                self.callbacks.push(f);
            }
        }
    }

    /// The species' parts of the plant group at entry `g` (0 where it has none).
    fn species_parts(ee: &Ee, g: u32) -> Vec<u32> {
        let sp = ee.u32_at(ee.u32_at(ee.u32_at(ECO_AT) + 0x84) + 4 * u32::from(ee.u8_at(g + 0x12)));
        (0..ee.u32_at(sp + 0x194)).map(|i| ee.u32_at(ee.u32_at(sp + 0x19c) + 4 * i)).collect()
    }
}

impl atv_part::Outside for PartWorldInRam<'_> {
    fn shape(&mut self, other: usize) -> ray_probe::Shape<'_> {
        let c = self.items[other] - 0x18;
        let kind = self.ee.u32_at(c + 0x78);
        *self.kinds.entry(format!("record search pair, kind {kind}")).or_default() += 1;
        match kind {
            0 => ray_probe::Shape::Mesh(self.mesh.insert(MeshInRam::read(self.ee, c)).mesh()),
            1 => panic!("the part search paired a hierarchy ({c:#x}): its parts are not read"),
            5 => unreachable!("a sphere is the part's own test"),
            6 => panic!("the part search paired a kind 6 ({c:#x}): its test is not Rust's"),
            _ => ray_probe::Shape::Never,
        }
    }
    fn sphere(&mut self, other: usize) -> Option<atv_part::Sphere> {
        let c = self.items[other] - 0x18;
        (self.ee.u32_at(c + 0x78) == 5).then(|| {
            *self.kinds.entry("record search pair, kind 5".into()).or_default() += 1;
            atv_part::Sphere { centre: vf_at(self.ee, c + 0x50), radius: word_f(self.ee, c + 0x180), radius2: word_f(self.ee, c + 0x184) }
        })
    }
}

impl ws::Narrow for PartWorldInRam<'_> {
    fn clear(&mut self, _: &mut [ws::Collidable], _: usize) {
        unreachable!("the part's own clear")
    }
    fn prepare(&mut self, _: &mut [ws::Collidable], _: usize) {
        panic!("the part search prepared a grid")
    }
    fn pair(&mut self, _: &mut [ws::Collidable], _: usize, _: usize) -> bool {
        unreachable!("the part's own pair")
    }
    fn parts(&mut self, _: &mut [ws::Collidable], group: usize) -> usize {
        Self::species_parts(self.ee, self.items[group]).len()
    }
    fn part(&mut self, _: &mut [ws::Collidable], group: usize, i: usize) -> Option<usize> {
        *self.kinds.entry("> record search part stand".into()).or_default() += 1;
        self.ee.set(28, GP);
        self.ee.call(ECO_PART, &[u64::from(self.items[group]), i as u64], &[], BUDGET);
        let p = self.ee.v0() as u32;
        (p != 0).then(|| self.items.iter().position(|&e| e == p + 0x18).expect("a stood part the search did not list"))
    }
    fn heard_hit(&mut self, objects: &mut [ws::Collidable], l: usize, probe: usize, other: usize) {
        let l = self.listeners[l];
        let f = self.ee.u32_at(self.ee.u32_at(l) + 8);
        self.hand_on(objects, "heard_hit", f, l, probe, other);
    }
    fn heard_struck(&mut self, objects: &mut [ws::Collidable], l: usize, struck: usize, probe: usize) {
        let l = self.listeners[l];
        let f = self.ee.u32_at(self.ee.u32_at(l) + 0xc);
        self.hand_on(objects, "heard_struck", f, l, struck, probe);
    }
    fn on_hit(&mut self, objects: &mut [ws::Collidable], c: usize, probe: usize, other: usize) {
        let f = self.callbacks[c];
        self.hand_on(objects, "on_hit", f, 0, probe, other);
    }
    fn on_struck(&mut self, objects: &mut [ws::Collidable], c: usize, struck: usize, probe: usize) {
        let f = self.callbacks[c];
        self.hand_on(objects, "on_struck", f, 0, struck, probe);
    }
}

/// The part's record (`+0x7c`) in RAM: what the clear leaves (the nearest normal and point, the
/// pair's first point, the struck part, the flags) and the list's shape (capacity `+0x88`, the rays
/// `+0x90` or none, one contact a ray `+0xbc`).
fn part_record_in_ram(ee: &Ee, r: u32) -> ray_probe::Record {
    ray_probe::Record {
        normal: vf_at(ee, r + 0x20),
        point: vf_at(ee, r + 0x30),
        first_point: vf_at(ee, r + 0x70),
        part: ee.u32_at(r + 0x84),
        listed: ee.u32_at(r + 0x90) != 0,
        capacity: ee.u32_at(r + 0x88) as usize,
        one_per_ray: ee.u8_at(r + 0xbc) != 0,
        flags: ee.u8_at(r + 0xb8),
        nearest: F::ONE,
        ..ray_probe::Record::default()
    }
}

/// The smooth-flag global the part's pair sets from the probe's `+0x184`.
const PART_SMOOTH_AT: u32 = 0x003d_c610;

/// **F46d — the record's part search in Rust**: the ray probe `part` (the riding or wreck rays) searched
/// over the nearby list `list` ([`atv_search::search`] with [`atv_part::PartRays`]: the clear and the
/// pair [`ray_probe`]'s, the meshes read from RAM), and everything written back: the flags, the
/// globals, the record. Answers the record's contacts; counts the pairs by kind into `kinds`.
fn part_search_in_ram(ee: &mut Ee, part: u32, list: u32, kinds: &mut BTreeMap<String, u32>) -> Vec<ray_probe::Contact> {
    use game_sim::atv_search;
    assert_eq!(ee.u32_at(part + 0x188), 0, "the part has a ray filter: its reset and test are not Rust's");
    assert_eq!(ee.u32_at(list + 0x40), 0, "the nearby list has a grid: the search would prepare it");
    let (mut items, mut objects, found) = sweep_world_in_ram(ee, part, list);
    // The plant groups' parts, as collidables of their own (the search tests them unfiltered).
    for k in 0..objects.len() {
        if objects[k].class != ws::Class::Group {
            continue;
        }
        for p in PartWorldInRam::species_parts(ee, items[k]) {
            if p != 0 && !items.contains(&(p + 0x18)) {
                items.push(p + 0x18);
                objects.push(ws::Collidable { class: ws::Class::from_byte(ee.u8_at(p + 0x18 + 2)), flags: ee.u8_at(p + 0x126), ..Default::default() });
            }
        }
    }
    let root = ee.u16_at(part + 0x170);
    let (nodes, _) = tree_in_ram(ee, ee.u32_at(part + 0x17c), root);
    let segs = ee.u32_at(part + 0x180);
    let segments: Vec<ray_probe::Segment> =
        (0..ee.u32_at(part + 0x178)).map(|i| ray_probe::Segment { from: vf_at(ee, segs + 0x20 * i), to: vf_at(ee, segs + 0x20 * i + 0x10) }).collect();
    let probe = ray_probe::RayProbe { transform: matrix_at(ee, part + 0x130), root, nodes: &nodes, segments: &segments };
    let r = ee.u32_at(part + 0x7c);
    let record = part_record_in_ram(ee, r);
    let retired: Vec<bool> = items.iter().map(|&e| ee.u32_at(e - 0x18) == 1).collect();
    let mut world = PartWorldInRam { ee: &mut *ee, items: items.clone(), mesh: None, kinds: BTreeMap::new(), listeners: vec![], callbacks: vec![] };
    for (o, &e) in objects.iter_mut().zip(&items) {
        if o.class == ws::Class::Object {
            world.hear(o, e - 0x18);
        }
    }
    let was = objects.clone();
    let mut n = atv_part::PartRays { probe, record, world: &mut world, pairs: 0 };
    let hit = atv_search::search(&mut objects, 0, &found, &|o| retired[o], &mut n);
    let (record, pairs) = (n.record, n.pairs);
    for (k, v) in std::mem::take(&mut world.kinds) {
        *kinds.entry(k).or_default() += v;
    }
    for (k, (a, b)) in was.iter().zip(&objects).enumerate() {
        assert_eq!(ws::Collidable { flags: b.flags, group: b.group, ..a.clone() }, *b, "item {k} changed beyond its flags and group");
        if let Some(g) = b.group {
            ee.set_u32(items[k] - 0x18 + 0x88, items[g]);
        }
        if a.flags != b.flags {
            assert_eq!(a.class, ws::Class::Object, "item {k}'s flags");
            set_u8(ee, items[k] - 0x18 + 0x126, b.flags);
        }
    }
    write_part_record(ee, part, r, &record, pairs);
    // A sphere struck leaves no contact: the hit bit alone says it.
    assert!(hit || record.contacts.is_empty(), "contacts without the hit bit");
    record.contacts
}

/// **F31b5l — the wreck fade's world search in Rust**: the ray probe `part` (the riding rays, the
/// collidable the fade names) against the world directly ([`atv_search::collide_world`] with
/// [`atv_part::PartRays`], not lazy), the world [`GatherInRam::read`]'s with the plant groups' parts
/// added as collidables of their own, and everything written back: the flags, the stamps and the
/// structures' walk state ([`GatherInRam::write_query`]), the parts' group, the probe's last centre
/// and budget, the clear's globals and the record ([`write_part_record`]). Counts the pairs and the
/// calls handed on (a plant part stood, a hearer) into `kinds`.
fn fade_search_in_ram(ee: &mut Ee, part: u32, kinds: &mut BTreeMap<String, u32>) {
    use game_sim::atv_search;
    assert_eq!(ee.u32_at(part + 0x188), 0, "the part has a ray filter: its reset and test are not Rust's");
    let (mut m, mut objects, mut broad) = GatherInRam::read(ee, part);
    for k in 0..objects.len() {
        if objects[k].class != ws::Class::Group {
            continue;
        }
        for p in PartWorldInRam::species_parts(ee, m.items[k]) {
            if p != 0 && !m.items.contains(&(p + 0x18)) {
                m.items.push(p + 0x18);
                objects.push(ws::Collidable { class: ws::Class::from_byte(ee.u8_at(p + 0x18 + 2)), flags: ee.u8_at(p + 0x126), ..Default::default() });
                broad.places.push(bp::Placement::default());
            }
        }
    }
    broad.places[0].last = [0, 4, 8, 12].map(|i| word_f(ee, part + 0x60 + i));
    broad.places[0].budget = word_f(ee, part + 0x74);
    let root = ee.u16_at(part + 0x170);
    let (nodes, _) = tree_in_ram(ee, ee.u32_at(part + 0x17c), root);
    let segs = ee.u32_at(part + 0x180);
    let segments: Vec<ray_probe::Segment> =
        (0..ee.u32_at(part + 0x178)).map(|i| ray_probe::Segment { from: vf_at(ee, segs + 0x20 * i), to: vf_at(ee, segs + 0x20 * i + 0x10) }).collect();
    let probe = ray_probe::RayProbe { transform: matrix_at(ee, part + 0x130), root, nodes: &nodes, segments: &segments };
    let r = ee.u32_at(part + 0x7c);
    let record = part_record_in_ram(ee, r);
    let retired: Vec<bool> = m.items.iter().map(|&e| ee.u32_at(e - 0x18) == 1).collect();
    // The static query begun as the search begins it, on a copy: whether the tree's list is fresh.
    let mut begun = broad.clone();
    let active = objects[0].flags & bp::more_flags::ACTIVE != 0;
    if let (true, Some(space)) = (active, begun.places[0].space) {
        let (c, h) = atv_part::bounds(&probe);
        let (mut lo, mut hi) = ([F::ZERO; 4], [F::ZERO; 4]);
        bp::Query::begin(&mut begun, space, 1, c, h, Some((&mut lo, &mut hi)), true);
    }
    let mut world = PartWorldInRam { ee: &mut *ee, items: m.items.clone(), mesh: None, kinds: BTreeMap::new(), listeners: vec![], callbacks: vec![] };
    for (o, &e) in objects.iter_mut().zip(&m.items) {
        if o.class == ws::Class::Object {
            world.hear(o, e - 0x18);
        }
    }
    let (was_o, was_b) = (objects.clone(), broad.clone());
    let mut n = atv_part::PartRays { probe, record, world: &mut world, pairs: 0 };
    atv_search::collide_world(&mut objects, &mut broad, 0, 0, &|o| retired[o], &mut n);
    let (record, pairs) = (n.record, n.pairs);
    for (k, v) in std::mem::take(&mut world.kinds) {
        *kinds.entry(k.replace("record search", "fade search")).or_default() += v;
    }
    for (k, o) in objects.iter_mut().enumerate() {
        if let Some(g) = o.group.take() {
            ee.set_u32(m.items[k] - 0x18 + 0x88, m.items[g]);
        }
    }
    let (last, budget) = (broad.places[0].last, broad.places[0].budget);
    broad.places[0].last = was_b.places[0].last;
    broad.places[0].budget = was_b.places[0].budget;
    let mut was_o = was_o;
    for o in &mut was_o {
        o.group = None;
    }
    m.write_query(ee, (&was_o, &was_b), (&objects, &broad), &begun);
    for (i, w) in last.iter().enumerate() {
        ee.set_u32(part + 0x60 + 4 * i as u32, w.0.to_bits());
    }
    ee.set_u32(part + 0x74, budget.0.to_bits());
    if active && broad.places[0].space.is_some() {
        write_part_record(ee, part, r, &record, pairs);
    }
}

/// The part search's outcome past the flags, written back: the clear's globals, the pairs' smooth
/// flag and per-contact words, and the part's record at `r`.
fn write_part_record(ee: &mut Ee, part: u32, r: u32, record: &ray_probe::Record, pairs: u32) {
    // The clear's globals, and the pairs' smooth flag and per-contact words.
    for a in SWEEP_ZEROED {
        ee.set_u32(a, 0);
    }
    ee.set_u32(SWEEP_RECORD_AT, r);
    if pairs > 0 {
        ee.set_u32(PART_SMOOTH_AT, ee.u32_at(part + 0x184));
        let zeroed = ee.u32_at(r + 0xac);
        for i in 0..record.capacity as u32 {
            ee.set_u32(zeroed + 4 * i, 0);
        }
    }
    // The record.
    let quads = [record.from, record.to, record.normal, record.point, record.push, record.low, record.high, record.first_point];
    for (i, q) in quads.into_iter().enumerate() {
        set_vf(ee, r + 0x10 * i as u32, q);
    }
    ee.set_u32(r + 0x80, record.first.0.to_bits());
    ee.set_u32(r + 0x84, record.part);
    ee.set_u32(r + 0x8c, record.contacts.len() as u32);
    ee.set_u32(r + 0xb4, record.nearest.0.to_bits());
    set_u8(ee, r + 0xb8, record.flags);
    let [rays, normals, points, objs, placed, fractions, words] = [0x90, 0x94, 0x98, 0x9c, 0xa0, 0xa8, 0xb0].map(|o| ee.u32_at(r + o));
    for (i, c) in record.contacts.iter().enumerate() {
        let i = i as u32;
        set_vf(ee, rays + 0x20 * i, c.from);
        set_vf(ee, rays + 0x20 * i + 0x10, c.to);
        set_vf(ee, normals + 0x10 * i, c.normal);
        set_vf(ee, points + 0x10 * i, c.point);
        ee.set_u32(objs + 4 * i, c.object);
        ee.set_u32(placed + 4 * i, u32::from(c.placed));
        ee.set_u32(fractions + 4 * i, c.fraction.0.to_bits());
        let w = words + 2 * i;
        let word = ee.u32_at(w & !3);
        let shift = (w & 2) * 8;
        ee.set_u32(w & !3, word & !(0xffff << shift) | u32::from(c.segment) << shift);
    }
}

/// **F46 — the handed-on calls in Rust, in lockstep with [`posts_frame`]**: C15's `name` replayed
/// twice by [`posts_frame`], once handing its calls on to the original and once with those `rust`
/// names Rust's ([`IN_RUST`]); every row of the second word for word against C15, and the two RAMs
/// alike after every frame outside the stack. Answers the wreck state per row and what the second
/// run answered.
fn replay_in_rust(name: &str, rust: &[&'static str]) -> Option<(Vec<bool>, BTreeMap<String, u32>)> {
    let words = mass_words(&race250p()?)?;
    let trn = nat05_trn()?;
    let map = track::Terrain::parse(&trn).unwrap();
    let mut a = hooked(image()?);
    for (at, mask, v) in words {
        a.set_u32(at, a.u32_at(at) & !mask | v & mask);
    }
    let mut b = hooked(image()?);
    b.ram.clone_from(&a.ram);
    let (mut noise_a, mut noise_b) = (None, None);
    let rows = rows(name)?;
    let dt = F(f32::from_bits(DT));
    let (mut out, mut all) = (Vec::new(), BTreeMap::<String, u32>::new());
    for (r, row) in rows.iter().enumerate() {
        poke(&mut a, row, &READER);
        poke(&mut b, row, &READER);
        posts_frame(&mut a, dt, &map, &mut noise_a, false);
        IN_RUST.with(|x| *x.borrow_mut() = rust.to_vec());
        let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| posts_frame(&mut b, dt, &map, &mut noise_b, false)));
        IN_RUST.with(|x| x.borrow_mut().clear());
        let (_, _, _, answered) = ran.unwrap_or_else(|e| std::panic::resume_unwind(e));
        for (k, n) in answered {
            *all.entry(k).or_default() += n;
        }
        let d = diff(&b, row);
        assert!(d.is_empty(), "{name} row {r} (vsync {}), {rust:?} Rust's: {} words differ\n  {}", row["vsync"], d.len(), d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  "));
        let off = ram_differs(&a, &b);
        assert!(off.is_empty(), "{name} row {r}: {rust:?} Rust's, RAM differs from the handing run's at {} ranges\n  {}", off.len(), off.iter().take(40).cloned().collect::<Vec<_>>().join("\n  "));
        let w = wrecked(|o| b.u32_at(BIKE + o));
        assert_eq!(w, row_wrecked(row), "{name} row {r}: the wreck state");
        out.push(w);
        poke(&mut a, row, &SPOT);
        poke(&mut b, row, &SPOT);
    }
    eprintln!("{name}: {rust:?} Rust's, answered {all:?}");
    Some((out, all))
}

/// The handed-on calls of a [`replay_in_rust`] answer (`> `), and how often `names` were answered.
fn handed_and(all: &BTreeMap<String, u32>, names: &[&str]) -> (Vec<(String, u32)>, Vec<(String, u32)>) {
    let handed = all.iter().filter(|(k, _)| k.starts_with("> ")).map(|(k, &n)| (k[2..].to_string(), n)).collect();
    let rust = names.iter().map(|&k| (k.to_string(), all.get(k).copied().unwrap_or(0))).collect();
    (handed, rust)
}

/// **F46a — state 4's database as the move sees it**: the riding probe's and the frame mesh's
/// records in one cell with no links, in its list of moving entries (one block of 1024, 141 handed
/// out), with no extra cells; read and written back unchanged, RAM is state 4's.
#[test]
fn state4s_database_reads_back_word_for_word() {
    let Some(ee) = image() else { return };
    for entry in [0x00ea_63b8, 0x00ea_5988] {
        let (m, db, rec) = DbInRam::read(&ee, entry);
        assert_eq!((m.cells.len(), m.portals.len(), m.subs.len()), (1, 0, 2), "entry {entry:#x}: one cell, no portal, two sub-databases");
        let Sub::List(l) = &db.subs[rec.sub] else { panic!("entry {entry:#x} is in no list") };
        assert_eq!((l.size, l.last, l.blocks.len(), rec.extras.len()), (0x400, 0x8d, 1, 0), "entry {entry:#x}'s list");
        assert!(matches!(db.subs[1 - rec.sub], Sub::Other { .. }), "the cell's other sub-database is the quadtree, read by its kind");
        let mut b = Ee::with_ram(ee.ram.clone());
        m.write(&mut b, entry, (&db, &rec), (&db, &rec));
        let off = bytes_differ(&ee.ram, &b.ram, ee.stack);
        assert!(off.is_empty(), "entry {entry:#x}: written back, RAM differs at {off:?}");
    }
}

/// **F46a — C15's ride, the database moves Rust's**: ride-2 in lockstep with [`posts_frame`]'s
/// handing run, the riding probe's and the frame mesh's moves [`atv_spatial::spatial_move`]'s every
/// frame, RAM alike.
#[test]
fn ride_replays_with_the_database_moves_in_rust() {
    let Some((w, all)) = replay_in_rust("ride-2", &["PartMove"]) else { return };
    assert_eq!(w.len(), 459);
    assert!(w.iter().all(|w| !w), "the ride never wrecks");
    let (handed, rust) = handed_and(&all, &["PartMove"]);
    assert_eq!(rust, [("PartMove".to_string(), 918)]);
    assert!(handed.iter().all(|(k, _)| k != "PartMove"), "{handed:?}");
}

/// **F46a — C15's flip, the database moves Rust's**: flip-1 in lockstep, wrecked on row 125 and put
/// back on row 193; the riding probe moved every frame and the frame mesh on 306.
#[test]
fn flip_replays_with_the_database_moves_in_rust() {
    let Some((w, all)) = replay_in_rust("flip-1", &["PartMove"]) else { return };
    assert_eq!(w.len(), 465);
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [125, 193], "wrecked on row 125, put back on row 193");
    let (handed, rust) = handed_and(&all, &["PartMove"]);
    assert_eq!(rust, [("PartMove".to_string(), 771)]);
    assert!(handed.iter().all(|(k, _)| k != "PartMove"), "{handed:?}");
}

/// **F46b — state 4's gather, the original's and Rust's**: the riding probe's and the frame mesh's
/// gathers into the bike's nearby list on state 4, then the riding probe stood at points round the
/// bike (out to 400 ft, so the query's cells, the tree nodes it lists and what it meets change) and
/// gathered again, one RAM by the original (`atv_gather_nearby`) and one by [`gather_in_ram`]:
/// the two RAMs alike after every gather outside the stack.
#[test]
fn state4s_gather_in_rust_leaves_ram_as_the_original() {
    let Some(ee) = image() else { return };
    let mut a = hooked(ee);
    let mut b = Ee::with_ram(a.ram.clone());
    let list = a.u32_at(BIKE + 0xc4);
    let (riding, frame) = (0x00ea_63a0u32, 0x00ea_5970u32);
    let home: [u32; 3] = std::array::from_fn(|i| a.u32_at(riding + 0x50 + 4 * i as u32));
    let mut g = 0x4634_3662_u64;
    let mut unit = move || {
        g = g.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (g >> 40) as f32 / (1u64 << 24) as f32
    };
    let (mut hows, mut sizes) = (BTreeMap::<&str, u32>::new(), BTreeMap::<u32, u32>::new());
    for n in 0..300 {
        let probe = if n == 1 { frame } else { riding };
        if n >= 2 {
            let reach = if n < 150 { 60.0 } else { 400.0 };
            for (i, h) in home.iter().enumerate() {
                let d = if i == 1 { (unit() * 2.0 - 1.0) * 20.0 } else { (unit() * 2.0 - 1.0) * reach };
                let v = (f32::from_bits(*h) + d).to_bits();
                a.set_u32(riding + 0x50 + 4 * i as u32, v);
                b.set_u32(riding + 0x50 + 4 * i as u32, v);
            }
        }
        a.set(28, GP);
        a.call(GATHER, &[u64::from(list), u64::from(probe), 3], &[0.0], BUDGET);
        *hows.entry(gather_in_ram(&mut b, list, probe, 3)).or_default() += 1;
        *sizes.entry(a.u32_at(list + 4)).or_default() += 1;
        let off = bytes_differ(&a.ram, &b.ram, a.stack);
        assert!(off.is_empty(), "gather {n} (probe {probe:#x}): RAM differs at {} ranges\n  {}", off.len(), off.iter().take(40).cloned().collect::<Vec<_>>().join("\n  "));
    }
    eprintln!("state 4's gathers: {hows:?}, list sizes {sizes:?}");
    assert!(hows.get("fresh").copied().unwrap_or(0) >= 250, "the queries were fresh: {hows:?}");
    assert!(sizes.len() > 5, "the lists differ in size: {sizes:?}");
}

/// **F46b — C15's ride, the gather Rust's**: ride-2 in lockstep with [`posts_frame`]'s handing
/// run, the database moves and the riding probe's gather Rust's every frame, RAM alike.
#[test]
fn ride_replays_with_the_gather_in_rust() {
    let Some((w, all)) = replay_in_rust("ride-2", &["PartMove", "Gather"]) else { return };
    assert_eq!(w.len(), 459);
    let (handed, rust) = handed_and(&all, &["PartMove", "Gather"]);
    assert_eq!(rust, [("PartMove".to_string(), 918), ("Gather".to_string(), 459)]);
    assert!(handed.iter().all(|(k, _)| k != "PartMove" && k != "Gather"), "{handed:?}");
}

/// **F46b — C15's flip, the gather Rust's**: flip-1 in lockstep, through the wreck and the put
/// back, the database moves and every gather Rust's, RAM alike.
#[test]
fn flip_replays_with_the_gather_in_rust() {
    let Some((w, all)) = replay_in_rust("flip-1", &["PartMove", "Gather"]) else { return };
    assert_eq!(w.len(), 465);
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [125, 193], "wrecked on row 125, put back on row 193");
    let (handed, rust) = handed_and(&all, &["PartMove", "Gather"]);
    assert_eq!(rust, [("PartMove".to_string(), 771), ("Gather".to_string(), 465)]);
    assert!(handed.iter().all(|(k, _)| k != "PartMove" && k != "Gather"), "{handed:?}");
}

/// **F46c — C15's ride, the record's sweep Rust's**: ride-2 in lockstep with [`posts_frame`]'s
/// handing run, every sweep of the bike's collision record [`sweep_in_ram`]'s (all of them nearby,
/// none hitting), RAM alike.
#[test]
fn ride_replays_with_the_sweep_in_rust() {
    let Some((w, all)) = replay_in_rust("ride-2", &["record sweep"]) else { return };
    assert_eq!(w.len(), 459);
    let (handed, rust) = handed_and(&all, &["record sweep"]);
    assert_eq!(rust, [("record sweep".to_string(), 459)]);
    assert!(handed.iter().all(|(k, _)| k != "record sweep"), "{handed:?}");
}

/// **F46c — C15's flip, the sweep, the gather and the moves Rust's**: flip-1 in lockstep, wrecked on
/// row 125 and put back on row 193, the record swept on every row.
#[test]
fn flip_replays_with_the_sweep_in_rust() {
    let names = ["PartMove", "Gather", "record sweep"];
    let Some((w, all)) = replay_in_rust("flip-1", &names) else { return };
    assert_eq!(w.len(), 465);
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [125, 193], "wrecked on row 125, put back on row 193");
    let (handed, rust) = handed_and(&all, &names);
    assert_eq!(rust, [("PartMove".to_string(), 771), ("Gather".to_string(), 465), ("record sweep".to_string(), 465)]);
    assert!(handed.iter().all(|(k, _)| !names.contains(&k.as_str())), "{handed:?}");
}

/// **F46d — C15's ride, the part search Rust's**: ride-2 in lockstep with [`posts_frame`]'s handing
/// run, every search of the riding part [`part_search_in_ram`]'s (all of them nearby, none hitting),
/// RAM alike.
#[test]
fn ride_replays_with_the_part_search_in_rust() {
    let Some((w, all)) = replay_in_rust("ride-2", &["record search"]) else { return };
    assert_eq!(w.len(), 459);
    let (handed, rust) = handed_and(&all, &["record search"]);
    assert_eq!(rust, [("record search".to_string(), 459)]);
    assert!(handed.iter().all(|(k, _)| k != "record search"), "{handed:?}");
    let pairs: Vec<(&String, &u32)> = all.iter().filter(|(k, _)| k.starts_with("record search pair")).collect();
    assert_eq!(pairs, [(&"record search pair, kind 5".to_string(), &432)], "ride-2's pairs: plant spheres alone");
    // Two listeners hear each of the 141 spheres struck (heard by RAM alike: a miss logs nothing).
    assert_eq!(all.get("> record search heard_hit"), Some(&282));
}

/// **F46 — C15's ride, all four of F46 Rust's**: ride-2 in lockstep with [`posts_frame`]'s handing
/// run, the database moves, the gather, the record's sweep and the part search all Rust's every
/// frame, none of them handed on, RAM alike.
#[test]
fn ride_replays_with_the_searches_and_the_database_in_rust() {
    let names = ["PartMove", "Gather", "record sweep", "record search"];
    let Some((w, all)) = replay_in_rust("ride-2", &names) else { return };
    assert_eq!(w.len(), 459);
    assert!(w.iter().all(|w| !w), "the ride never wrecks");
    let (handed, rust) = handed_and(&all, &names);
    assert_eq!(rust.iter().map(|(_, n)| *n).collect::<Vec<_>>(), [918, 459, 459, 459], "{rust:?}");
    assert!(handed.iter().all(|(k, _)| !names.contains(&k.as_str())), "{handed:?}");
    let pairs: Vec<(&String, &u32)> = all.iter().filter(|(k, _)| k.starts_with("record search pair")).collect();
    assert_eq!(pairs, [(&"record search pair, kind 5".to_string(), &432)], "ride-2's pairs: plant spheres alone");
    assert_eq!(all.get("> record search heard_hit"), Some(&282));
}

/// **F46d — C15's flip, all four of F46 Rust's**: flip-1 in lockstep, the riding part searched
/// until row 125, the wreck part from there to row 193, the riding part again after; the moves, the
/// gather, the sweep and the part search all Rust's.
#[test]
fn flip_replays_with_the_part_search_in_rust() {
    let names = ["PartMove", "Gather", "record sweep", "record search"];
    let Some((w, all)) = replay_in_rust("flip-1", &names) else { return };
    assert_eq!(w.len(), 465);
    let (handed, rust) = handed_and(&all, &names);
    assert_eq!(rust.iter().map(|(_, n)| *n).collect::<Vec<_>>(), [771, 465, 465, 465], "{rust:?}");
    assert!(handed.iter().all(|(k, _)| !names.contains(&k.as_str())), "{handed:?}");
    let pairs: Vec<(&String, &u32)> = all.iter().filter(|(k, _)| k.starts_with("record search pair")).collect();
    assert_eq!(pairs, [(&"record search pair, kind 5".to_string(), &161)], "flip-1's pairs: plant spheres alone");
    assert_eq!(all.get("> record search heard_hit"), Some(&110));
}

/// SLUS-21104's ray probe against a sphere, its two tests (research/oracle.txt `# F46d`): a world
/// segment, and a node's box in a frame.
const SPHERE_SEGMENT: u32 = 0x0010_6180;
const SPHERE_NODE: u32 = 0x0010_6e30;

/// **F46d — the sphere tests are the original's**: 40 000 segments and 40 000 node boxes against
/// spheres, drawn round each other (eighths, with now and then a low bit flipped, a zero signed, a
/// zero-length segment, a radius squared that is an end's own distance squared), through
/// [`atv_part::segment_reaches_sphere`] and [`atv_part::node_reaches_sphere`] and the original's;
/// the same answer every time, and both answers seen.
#[test]
fn sphere_tests_are_the_originals() {
    let Some(mut ee) = image() else { return };
    let mut n = 0u64;
    let mut draw = |span: i64| -> f32 {
        n += 1;
        let r = mix(0xf46d ^ n.wrapping_mul(0x9e37_79b9));
        let x = ((r % (2 * span as u64 * 8 + 1)) as i64 - span * 8) as f32 / 8.0;
        match r >> 60 {
            0 => f32::from_bits(x.to_bits() ^ (r >> 32 & 3) as u32),
            1 => -0.0,
            _ => x,
        }
    };
    let quad = |ee: &mut Ee, a: u32, v: Vf| set_vf(ee, a, v);
    let mut seen = [0u32; 2];
    for i in 0..40_000 {
        let centre = Vf([draw(4), draw(4), draw(4), draw(1)]);
        let from = Vf([draw(8), draw(8), draw(8), 1.0]);
        let to = if i % 50 == 0 { from } else { Vf([draw(8), draw(8), draw(8), 1.0]) };
        let mut r2 = F(draw(16).abs());
        if i % 7 == 0 {
            let d: Vec<f32> = (0..3).map(|k| centre.0[k] - from.0[k]).collect();
            r2 = F(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]);
        }
        let [a, b, c, out] = [0, 1, 2, 3].map(|k| ee.scratch(k));
        quad(&mut ee, a, from);
        quad(&mut ee, b, to);
        quad(&mut ee, c, centre);
        ee.set(28, GP);
        ee.call(SPHERE_SEGMENT, &[u64::from(a), u64::from(b), u64::from(c), u64::from(out)], &[r2.0], BUDGET);
        let want = ee.v0() != 0;
        let got = atv_part::segment_reaches_sphere(from, to, centre, r2);
        assert_eq!(got, want, "segment {i}: {from:?} to {to:?}, centre {centre:?}, r2 {r2:?}");
        seen[usize::from(want)] += 1;
    }
    assert!(seen.iter().all(|&k| k > 1000), "segments struck and missed: {seen:?}");
    let mut seen = [0u32; 2];
    for i in 0..40_000 {
        let c = [F(draw(4)), F(draw(4)), F(draw(4))];
        let h = [F(draw(3).abs()), F(draw(3).abs()), F(draw(3).abs())];
        let m: ray_probe::Matrix = core::array::from_fn(|k| Vf([draw(2), draw(2), draw(2), if k == 3 { 1.0 } else { 0.0 }]));
        let radius = F(draw(4).abs());
        let radius2 = if i % 3 == 0 { F(draw(16).abs()) } else { radius * radius };
        let s = atv_part::Sphere { centre: Vf([draw(6), draw(6), draw(6), 1.0]), radius, radius2 };
        let [a, b, sc, frame] = [0, 1, 2, 3].map(|k| ee.scratch(k));
        quad(&mut ee, a, Vf([c[0].0, c[1].0, c[2].0, draw(1)]));
        quad(&mut ee, b, Vf([h[0].0, h[1].0, h[2].0, draw(1)]));
        quad(&mut ee, sc, s.centre);
        for (k, row) in m.iter().enumerate() {
            quad(&mut ee, frame + 0x10 * k as u32, *row);
        }
        ee.set(28, GP);
        ee.call(SPHERE_NODE, &[u64::from(a), u64::from(b), u64::from(sc), u64::from(frame)], &[radius.0, radius2.0], BUDGET);
        let want = ee.v0() != 0;
        let got = atv_part::node_reaches_sphere(c, h, &s, &m);
        assert_eq!(got, want, "node {i}: centre {c:?}, half {h:?}, frame {m:?}, sphere {s:?}");
        seen[usize::from(want)] += 1;
    }
    assert!(seen.iter().all(|&k| k > 1000), "nodes reached and not: {seen:?}");
}

// ---------------------------------------------------------------------------------------------
// F47: the PostProcess's paths C15 never takes — the body reset, the respawn, the out-of-world
// message's post and the node's box for a bike or node in the spatial database — each against the
// original over random bikes (research/oracle.txt `# F47`).
// ---------------------------------------------------------------------------------------------

use game_sim::handling::reset::{self, Resets};

/// The bike's vtable slots and their functions on state 4: the body reset and the respawn; the
/// body's on word (its second vtable's `+0xc`); the collision record handed back to its owner; the
/// node's bounds gather (its vt `+0x94`); the file and line the riding part's list grows with.
const BODY_RESET: (u32, u32) = (0x13c, 0x0049_4ae0);
const RESPAWN: (u32, u32) = (0xe8, 0x004a_4c60);
const BODY_ON: u32 = 0x0049_2b20;
const RELEASE: u32 = 0x0020_3f40;
const NODE_REBOUND: u32 = 0x0018_ce40;
const GROW_FILE: (u64, u64) = (0x0039_683e, 0x54a);
/// State 4's spatial database (the bike's parts' entries', its `+0xb4`) and the word its parts were
/// added with (`+0xc0`); the bike's add (its vt `+0xe0`) and the node's (as the ground reset hangs it
/// back).
const DATABASE: (u32, u32) = (0x0067_2cd0, 2);
const BIKE_ADD: (u32, u32) = (0xe0, 0x0049_2bb0);
const NODE_ADD: u32 = 0x0019_6c10;
/// A scene node's parent (`+0xf0`) and flag byte (`+0x100`).
const NODE_PARENT: u32 = 0xf0;
/// A run of RAM state 4 leaves zero, for the oracles' fake lists, queues and listeners.
const F47_AREA: u32 = 0x0009_0000;
/// The scratch slot the port lays the event's message in.
const MESSAGE_SCRATCH: u32 = 5;

/// The bike's RAM as [`reset`]'s port sees it: each call out the original's but the body's on word
/// (the port's, its slot checked), and the ground reset ([`wreck::ground`], served as the replays
/// serve it).
struct ResetInRam<'a>(&'a mut Ee);

impl Resets for ResetInRam<'_> {
    fn word(&mut self, at: u32) -> u32 {
        self.0.u32_at(at)
    }
    fn set(&mut self, at: u32, w: u32) {
        self.0.set_u32(at, w);
    }
    fn out(&mut self, c: reset::Out) -> u32 {
        let ee = &mut *self.0;
        let u = u64::from;
        let (f, ints) = match c {
            reset::Out::BodyOn { body, on } => {
                assert_eq!(ee.u32_at(ee.u32_at(body + 0x110) + 0xc), BODY_ON, "the body's on word's virtual");
                reset::body_on(self, body, on);
                return 0;
            }
            reset::Out::Ground => {
                wreck::ground(&mut WreckInRam { ee, x: 0.0 });
                return 0;
            }
            reset::Out::Message(words) => {
                let at = ee.scratch(MESSAGE_SCRATCH);
                for (k, w) in words.into_iter().enumerate() {
                    ee.set_u32(at + 4 * k as u32, w);
                }
                return at;
            }
            reset::Out::Release { owner, record } => (RELEASE, vec![u(owner), u(record)]),
            reset::Out::Grow { at, bytes } => (HEAP_REALLOC, vec![u(at), u(bytes), GROW_FILE.0, GROW_FILE.1]),
            reset::Out::Tell { to, from, message } => (ee.u32_at(ee.u32_at(to + 8) + 0xc), vec![u(to), u(from), u(message)]),
        };
        ee.set(28, GP);
        ee.call(f, &ints, &[0.0], BUDGET);
        ee.r(2) as u32
    }
}

/// What a hooked call was handed: its address, then its arguments' words.
type Calls = Rc<RefCell<Vec<(u32, Vec<u32>)>>>;

/// `at` hooked to log its first `ints` integer arguments, the quads at the arguments `quads` point
/// to, and `$f12` with `float`; it answers `v0`.
fn log_hook(ee: &mut Ee, at: u32, ints: usize, quads: &'static [usize], float: bool, v0: u32, log: &Calls) {
    let log = log.clone();
    ee.hooks.insert(at, Box::new(move |ee: &mut Ee| {
        let mut w: Vec<u32> = (0..ints).map(|k| ee.r(4 + k) as u32).collect();
        for &q in quads {
            let p = ee.r(4 + q) as u32;
            w.extend([0, 4, 8, 12].map(|k| ee.u32_at(p + k)));
        }
        if float {
            w.push(ee.f(12).to_bits());
        }
        log.borrow_mut().push((at, w));
        ee.set(2, u64::from(v0));
    }));
}

/// The original's run of `f` and the port's from the same RAM: where RAM differs, and whether
/// the hooked calls differ.
fn f47_compare(ee: &mut Ee, log: &Calls, original: impl FnOnce(&mut Ee), port: impl FnOnce(&mut Ee)) -> Vec<String> {
    let (before, spr) = (ee.ram.clone(), ee.spr.clone());
    log.borrow_mut().clear();
    original(ee);
    let want_log = std::mem::take(&mut *log.borrow_mut());
    let (want, want_spr) = (std::mem::replace(&mut ee.ram, before), std::mem::replace(&mut ee.spr, spr));
    port(ee);
    let got_log = std::mem::take(&mut *log.borrow_mut());
    // The glue's scratch slots over the stack (the out-parameters it hands the original's callees,
    // [`WRECK_SCRATCH`]'s the highest) are not the game's.
    let mut want = want;
    let slots = ee.stack as usize..ee.scratch(WRECK_SCRATCH[4] + 1) as usize;
    want[slots.clone()].copy_from_slice(&ee.ram[slots]);
    let mut d: Vec<String> = bytes_differ(&want, &ee.ram, ee.stack).into_iter().map(|r| format!("RAM at {r}")).collect();
    if want_spr != ee.spr {
        let at: Vec<usize> = (0..want_spr.len()).filter(|&i| want_spr[i] != ee.spr[i]).collect();
        d.push(format!("the scratchpad at {:x?} ({} bytes)", &at[..at.len().min(8)], at.len()));
    }
    if want_log != got_log {
        d.push(format!("calls: original {want_log:x?}, port {got_log:x?}"));
    }
    d
}

fn f47_report(t: &BTreeMap<&'static str, u32>, off: &[String], floor: u32) {
    eprintln!("{t:#?}");
    assert!(off.is_empty(), "{} differ:\n  {}", off.len(), off.iter().take(30).cloned().collect::<Vec<_>>().join("\n  "));
    assert!(t.values().all(|n| *n >= floor), "every path taken {floor} times: {t:#?}");
}

/// **F47 — the body reset is the original's**: 10,000 times on state 4's bike, its body on or off,
/// the physics world's three lists (count, where the bike's entry is, if anywhere) drawn, the
/// record's owner, the record and the riding part there or not, the flag byte's bits 7 and 6, and
/// the riding part's list (count, the world's word in it, an empty slot or neither, so that it
/// grows) — SLUS-21104's body reset (the bike's vt `+0x13c`) in the interpreter, and
/// [`reset::body_reset`] with the record's release and the list's growth the original's. The
/// allocator is hooked (it logs and answers a fixed block). Every word of RAM and the calls come
/// out the same.
#[test]
fn the_body_reset_is_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    assert_eq!(vt(&ee, BODY_RESET.0), BODY_RESET.1, "the bike's vt +0x13c");
    let log: Calls = Rc::default();
    log_hook(&mut ee, HEAP_REALLOC, 4, &[], false, F47_AREA + 0x800, &log);
    let base = ee.ram.clone();
    let mut g = Gen(0x0f47_0001);
    let (mut t, mut off) = (BTreeMap::new(), Vec::new());
    let (body, world, owner, record, riding, bounds) = [reset::BODY, reset::WORLD, reset::OWNER, reset::RECORD, reset::RIDING, reset::BOUNDS].map(|o| ee.u32_at(BIKE + o)).into();
    let second = ee.u32_at(BIKE + reset::SECOND);
    for k in 0..10_000 {
        ee.ram.clone_from(&base);
        let on = !g.one_in(8);
        ee.set_u32(body, if on { if g.one_in(2) { 1 } else { g.bits() | 1 } } else { 0 });
        tally(&mut t, if on { "body on" } else { "body off" });
        for (i, (&list, x)) in reset::LISTS.iter().zip([body, second, BIKE + reset::POST]).enumerate() {
            let items = F47_AREA + 0x100 * i as u32;
            let count = (g.bits() % 8) as i32 - 1;
            ee.set_u32(world + list, count as u32);
            ee.set_u32(world + list + 8, items);
            for j in 0..8 {
                ee.set_u32(items + 4 * j, if g.one_in(3) { 0 } else { g.bits() });
            }
            if count > 0 && !g.one_in(4) {
                ee.set_u32(items + 4 * (g.bits() % count as u32), x);
                if i == 0 {
                    tally(&mut t, "the body listed");
                }
            }
        }
        let (has_owner, has_record, has_riding) = (!g.one_in(4), !g.one_in(4), !g.one_in(4));
        ee.set_u32(BIKE + reset::OWNER, if has_owner { owner } else { 0 });
        ee.set_u32(BIKE + reset::RECORD, if has_record { record } else { 0 });
        ee.set_u32(BIKE + reset::RIDING, if has_riding { riding } else { 0 });
        let bits = g.bits() & 0xc0;
        let b = u32::from(ee.u8_at(BIKE + reset::FLAG_BYTE)) & !0xc0 | bits;
        set_byte(&mut ee, BIKE + reset::FLAG_BYTE, b);
        ee.set_u32(BIKE + reset::FLAG_WORD, g.bits());
        let x = if g.one_in(2) { 0x800 } else { g.bits() };
        ee.set_u32(bounds + 0xa4, x);
        let (count, items) = ((g.bits() % 6) as i32 - 1, F47_AREA + 0x400);
        ee.set_u32(riding + 0x100, count as u32);
        ee.set_u32(riding + 0xfc, items);
        ee.set_u32(riding + 0xf8, g.bits());
        for j in 0..6 {
            ee.set_u32(items + 4 * j, g.bits() | 1);
        }
        if count > 0 {
            match g.bits() % 3 {
                0 => ee.set_u32(items + 4 * (g.bits() % count as u32), x),
                1 => ee.set_u32(items + 4 * (g.bits() % count as u32), 0),
                _ => {}
            }
        }
        if on && has_owner && has_record {
            tally(&mut t, match (bits & 0x80 != 0, has_riding) {
                (true, true) => "the riding part's list",
                (true, false) => "no riding part",
                _ => "the record released",
            });
        }
        tally(&mut t, if bits & 0x40 != 0 { "bit 6" } else { "no bit 6" });
        let d = f47_compare(
            &mut ee,
            &log,
            |ee| {
                ee.set(28, GP);
                ee.call(BODY_RESET.1, &[u64::from(BIKE)], &[0.0], BUDGET);
            },
            |ee| reset::body_reset(&mut ResetInRam(ee), BIKE),
        );
        off.extend(d.into_iter().map(|d| format!("{k}: {d}")));
    }
    f47_report(&t, &off, 300);
}

/// **F47 — the respawn is the original's**: 10,000 times on state 4's bike, drawn as
/// [`the_ground_reset_is_the_originals`] draws it (its spot, forward and up, the body's offset, the
/// parts' flag and the riding part there or not) and its respawned word — SLUS-21104's respawn (the
/// bike's vt `+0xe8`) and [`reset::respawn`], the ground reset [`wreck::ground`].
#[test]
#[allow(clippy::float_arithmetic)]
fn the_respawn_is_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    assert_eq!(vt(&ee, RESPAWN.0), RESPAWN.1, "the bike's vt +0xe8");
    let log: Calls = Rc::default();
    let base = ee.ram.clone();
    let mut g = Gen(0x0f47_0002);
    let (mut t, mut off) = (BTreeMap::new(), Vec::new());
    let body = ee.u32_at(BIKE + wreck::BODY);
    let riding = ee.u32_at(BIKE + 0xcc);
    for k in 0..10_000 {
        ee.ram.clone_from(&base);
        let r = |g: &mut Gen, s: f32| [g.f(-s, s), g.f(-s, s), g.f(-s, s), g.near(1.0, -2.0, 2.0)];
        let quad = |ee: &mut Ee, at: u32, v: [f32; 4]| {
            for (i, x) in v.into_iter().enumerate() {
                ee.set_f32(at + 4 * i as u32, x);
            }
        };
        let at = r(&mut g, 500.0);
        let (fwd, up) = loop {
            let (a, b) = (r(&mut g, 3.0), r(&mut g, 3.0));
            let c = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
            let n = |v: &[f32]| v[..3].iter().map(|x| x * x).sum::<f32>();
            if n(&c) > 0.01 * n(&a) * n(&b) && n(&a) > 0.01 && n(&b) > 0.01 {
                break (a, b);
            }
        };
        quad(&mut ee, BIKE + 0x50, at);
        quad(&mut ee, BIKE + 0x60, fwd);
        quad(&mut ee, BIKE + 0x70, up);
        let off_by = r(&mut g, 1.5);
        quad(&mut ee, body + 0x1a0, off_by);
        let (on, ride) = (g.one_in(2), !g.one_in(4));
        let b44 = ee.u8_at(BIKE + 0x44) & !1 | u8::from(on);
        set_byte(&mut ee, BIKE + 0x44, u32::from(b44));
        ee.set_u32(BIKE + 0xcc, if ride { riding } else { 0 });
        ee.set_u32(BIKE + reset::RESPAWNED, g.bits());
        tally(&mut t, if on { "parts on" } else { "parts left" });
        tally(&mut t, if ride { "riding part" } else { "no riding part" });
        let d = f47_compare(
            &mut ee,
            &log,
            |ee| {
                ee.set(28, GP);
                ee.call(RESPAWN.1, &[u64::from(BIKE)], &[0.0], BUDGET);
            },
            |ee| reset::respawn(&mut ResetInRam(ee), BIKE),
        );
        off.extend(d.into_iter().map(|d| format!("{k}: {d}")));
    }
    f47_report(&t, &off, 1000);
}

/// **F47 — the event post is the original's**: 10,000 posts, nine in ten on a queue drawn in
/// [`F47_AREA`] (up to six registrations, live or not, the poster's or another's, its tag or
/// another; up to six listeners each, live or not, some of which change their registration's count
/// when told), one in ten the out-of-world message on state 4's own queue from the bike's post
/// element to its 11 listeners — SLUS-21104's post in the interpreter and [`reset::post`], the
/// listeners the original's (the drawn ones hooked to log what they are told). Every word of RAM
/// outside the stack and the calls come out the same.
#[test]
fn the_event_post_is_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    let log: Calls = Rc::default();
    // The drawn listeners' class: a vtable whose `+0xc` logs (the listener, the poster's word, the
    // message) and moves the count its listener names by the step it holds.
    let (vtable, tell) = (F47_AREA + 0xf00, F47_AREA + 0xf80);
    ee.set_u32(vtable + 0xc, tell);
    let l = log.clone();
    ee.hooks.insert(tell, Box::new(move |ee: &mut Ee| {
        let (to, from, msg) = (ee.r(4) as u32, ee.r(5) as u32, ee.r(6) as u32);
        let mut w = vec![to, from];
        w.extend([0, 4, 8, 12].map(|k| ee.u32_at(msg + k)));
        l.borrow_mut().push((tell, w));
        let (step, count) = (ee.u32_at(to + 0x20), ee.u32_at(to + 0x24));
        if step != 0 {
            let c = ee.u32_at(count);
            ee.set_u32(count, c.wrapping_add(step));
        }
    }));
    let base = ee.ram.clone();
    let mut g = Gen(0x0f47_0003);
    let (mut t, mut off) = (BTreeMap::new(), Vec::new());
    let poster = F47_AREA + 0x1000;
    for k in 0..10_000 {
        ee.ram.clone_from(&base);
        let (queue, from, message) = if g.one_in(10) {
            tally(&mut t, "state 4's queue");
            let id = ee.u32_at(BIKE + 0xd20);
            let (kind, msg) = bike::OUT_OF_WORLD;
            (ee.u32_at(POST_QUEUE), BIKE + 0x674, [kind, msg, 0, id])
        } else {
            let q = F47_AREA;
            let tag = if g.one_in(3) { 0 } else { g.bits() };
            ee.set_u32(poster, tag);
            let regs = (g.bits() % 8) as i32 - 1;
            ee.set_u32(q + 0x24, regs as u32);
            ee.set_u32(q + 0x2c, q + 0x40);
            let mut mine = false;
            for j in 0..6u32 {
                let e = q + 0x100 + 0x40 * j;
                ee.set_u32(q + 0x40 + 4 * j, e);
                let live = !g.one_in(3);
                let me = !g.one_in(3);
                let tagged = !g.one_in(4);
                ee.set_u32(e, if live { 0 } else { 1 + g.bits() % 3 });
                ee.set_u32(e + 0x10, if me { poster } else { g.bits() });
                ee.set_u32(e + 4, if tagged { tag } else { tag ^ (1 + g.bits() % 0xff) });
                mine |= (j as i32) < regs && live && me && tagged;
                let n = (g.bits() % 8) as i32 - 1;
                ee.set_u32(e + 0x30, n as u32);
                let items = F47_AREA + 0x400 + 0x40 * j;
                ee.set_u32(e + 0x38, items);
                for i in 0..8u32 {
                    let rec = F47_AREA + 0x800 + 0x100 * j + 0x20 * i;
                    ee.set_u32(items + 4 * i, rec);
                    ee.set_u32(rec, if g.one_in(4) { 1 } else { 0 });
                    let obj = F47_AREA + 0x2000 + 0x400 * j + 0x40 * i;
                    ee.set_u32(rec + 0x10, obj);
                    ee.set_u32(obj + 8, vtable);
                    let step = match g.bits() % 8 {
                        0 => 1,
                        1 => u32::MAX,
                        _ => 0,
                    };
                    ee.set_u32(obj + 0x20, step);
                    ee.set_u32(obj + 0x24, e + 0x30);
                }
            }
            tally(&mut t, if mine { "a registration" } else { "none registered" });
            (q, poster, [g.bits() % 16, g.bits() % 32, g.bits(), g.bits()])
        };
        let [kind, id, last, data] = message;
        let d = f47_compare(
            &mut ee,
            &log,
            |ee| {
                ee.set(28, GP);
                let ints = [queue, from, kind, id, data, last].map(u64::from);
                ee.call(POST_MESSAGE, &ints, &[0.0], BUDGET);
            },
            |ee| reset::post(&mut ResetInRam(ee), queue, from, message),
        );
        off.extend(d.into_iter().map(|d| format!("{k}: {d}")));
    }
    f47_report(&t, &off, 900);
}

/// **F47 — the node's box for a bike or node in the spatial database is the original's**: 10,000
/// times on state 4's bike, its and its node's database entries set, the node's flag bits (current,
/// own bounds, a box of its own, a sphere, bounded, stale) and second byte's bit 2 drawn, its own
/// centre, half extent and radius and its position drawn, and how many of the 22 skeleton nodes its
/// gather takes in, each current or not and with bounds of its own or not, theirs drawn too —
/// SLUS-21104's node bounds and spatial update (the bike's vt `+0xe4`) in the interpreter, and
/// [`node::bounds`] and [`node::world_box`] on RAM, the database move hooked (it logs its entry,
/// centre, half extent and reach). Every word of RAM and the moves come out the same.
#[test]
#[allow(clippy::float_arithmetic)]
fn the_node_box_in_the_database_is_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    let node = ee.u32_at(BIKE + 0xe4);
    assert_eq!(ee.u32_at(ee.u32_at(node + 4) + 0x94), NODE_REBOUND, "the node's vt +0x94");
    assert_eq!(vt(&ee, 0xe4), SPATIAL_UPDATE, "the bike's vt +0xe4");
    let log: Calls = Rc::default();
    log_hook(&mut ee, SPATIAL_MOVE, 1, &[1, 2], true, 0, &log);
    let listed = ee.u32_at(node + 0x10c);
    let list = ee.u32_at(node + 0x144);
    assert_eq!(listed, 22, "state 4's node gathers its 22 skeleton nodes");
    let skeleton: Vec<u32> = (0..listed).map(|k| ee.u32_at(list + 4 * k)).collect();
    let base = ee.ram.clone();
    let mut g = Gen(0x0f47_0004);
    let (mut t, mut off) = (BTreeMap::new(), Vec::new());
    let quad = |ee: &mut Ee, at: u32, v: [f32; 4]| {
        for (i, x) in v.into_iter().enumerate() {
            ee.set_f32(at + 4 * i as u32, x);
        }
    };
    for k in 0..10_000 {
        ee.ram.clone_from(&base);
        let r = |g: &mut Gen, s: f32| [g.near(0.0, -s, s), g.near(0.0, -s, s), g.near(0.0, -s, s), g.near(1.0, -2.0, 2.0)];
        ee.set_u32(BIKE + 8, F47_AREA | 0x10);
        ee.set_u32(node + 8, if g.one_in(8) { 0 } else { F47_AREA | 0x20 });
        let n = g.bits() % (listed + 1);
        ee.set_u32(node + 0x10c, n);
        let mut own_listed = false;
        for (i, &s) in skeleton.iter().enumerate() {
            let fl = u32::from(ee.u8_at(s + 0x100)) & !0x05 | g.bits() & 0x05;
            set_byte(&mut ee, s + 0x100, fl);
            quad(&mut ee, s + 0xb0, r(&mut g, 2.0));
            quad(&mut ee, s + 0xc0, r(&mut g, 1.5));
            ee.set_f32(s + 0x104, g.near(0.0, -0.5, 2.0));
            own_listed |= (i as u32) < n && fl & 0x04 != 0;
        }
        let mut fl = u32::from(ee.u8_at(node + 0x100)) & !0xdd | g.bits() & 0xdd;
        let stale = fl & 0x80 != 0 && (fl & 0x08 != 0 || fl & 0x40 == 0);
        // A sphere with nothing to gather has no centre (the original's stack word): not drawn.
        if stale && fl & 0x10 != 0 && fl & 0x04 == 0 && !own_listed {
            fl |= 0x04;
        }
        set_byte(&mut ee, node + 0x100, fl);
        let more = u32::from(ee.u8_at(node + 0x101)) & !0x04 | g.bits() & 0x04;
        set_byte(&mut ee, node + 0x101, more);
        quad(&mut ee, node + 0xb0, r(&mut g, 2.0));
        quad(&mut ee, node + 0xc0, r(&mut g, 1.5));
        ee.set_f32(node + 0x104, g.near(0.0, -0.5, 2.0));
        ee.set_f32(node + 0x108, g.near(0.0, -1.0, 4.0));
        if g.one_in(2) {
            let p = r(&mut g, 300.0);
            quad(&mut ee, node + 0x60, [p[0], p[1], p[2], 1.0]);
        }
        tally(&mut t, if stale { if fl & 0x10 != 0 { "gathered as a sphere" } else { "gathered as a box" } } else { "not gathered" });
        tally(&mut t, if more & 0x04 != 0 { "round the origin" } else { "the box's reach" });
        let what = k % 3;
        tally(&mut t, ["the node's bounds", "the spatial update", "both"][what as usize]);
        let d = f47_compare(
            &mut ee,
            &log,
            |ee| {
                ee.set(28, GP);
                if what != 1 {
                    ee.call(NODE_BOUNDS, &[u64::from(node)], &[0.0], BUDGET);
                }
                if what != 0 {
                    ee.call(SPATIAL_UPDATE, &[u64::from(BIKE)], &[0.0], BUDGET);
                }
            },
            |ee| {
                let mut moved = Vec::new();
                let m = &mut NodeRam(ee);
                if what != 1 {
                    if let Some((c, h, reach)) = node::bounds(m, node) {
                        moved.push((node, c, h, reach));
                    }
                }
                if what != 0 {
                    if let Some((c, h)) = node::owner_box(m, BIKE, node) {
                        moved.push((BIKE, c, h, F::ZERO));
                    }
                }
                for (a, c, h, reach) in moved {
                    let mut w = vec![a];
                    w.extend(c.to_bits());
                    w.extend(h.to_bits());
                    w.push(reach.to_bits());
                    log.borrow_mut().push((SPATIAL_MOVE, w));
                }
            },
        );
        off.extend(d.into_iter().map(|d| format!("{k}: {d}")));
    }
    f47_report(&t, &off, 1000);
}

/// **F47 — the PostProcess off C15's paths is the original's**: 10,000 times on state 4's bike, its
/// body's position, the world's floor and box and the flag byte drawn (as
/// [`the_bikes_post_process_is_the_originals`] draws them, so that about half the bikes are out of
/// the world), the body on or off, the flag byte `+0x44`'s bits 7 and 6, the bike and its node put in
/// state 4's spatial database or not (by the game's own adds; bit 1 of `+0x45` with the node's), and
/// the node's stale bits — SLUS-21104's PostProcess in the interpreter, and [`bike::air_state`] with
/// its callees [`PostsInRam`]'s: the body reset, the respawn, the message's post, the put-back and the
/// node's box in the database the F47 ports, the database's own calls the original's in both. Every
/// word of RAM outside the stack comes out the same.
#[test]
#[allow(clippy::float_arithmetic)]
fn the_post_process_out_of_the_world_is_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    assert_eq!(vt(&ee, BIKE_ADD.0), BIKE_ADD.1, "the bike's vt +0xe0");
    assert_eq!([0xb4, 0xc0].map(|o| ee.u32_at(BIKE + o)), [DATABASE.0, DATABASE.1], "state 4's database");
    let log: Calls = Rc::default();
    let body = ee.u32_at(BIKE + seed::BODY);
    let node = ee.u32_at(BIKE + 0xe4);
    let world = ee.u32_at(BIKE + 0xe8);
    let area = F47_AREA + 0x3000;
    let base = ee.ram.clone();
    let mut g = Gen(0x0f47_0005);
    let (mut t, mut off) = (BTreeMap::new(), Vec::new());
    let mut answered = BTreeMap::new();
    for k in 0..10_000 {
        ee.ram.clone_from(&base);
        let v = |ee: &mut Ee, g: &mut Gen, a: u32, r: f32| {
            for i in 0..3 {
                let x = g.near(0.0, -r, r);
                ee.set_f32(a + 4 * i, x);
            }
        };
        v(&mut ee, &mut g, body + 0x70, 150.0);
        ee.set_u32(body, u32::from(!g.one_in(8)));
        let flags = [0x00, 0x01, 0x40, 0x41][(g.bits() % 4) as usize] | (g.bits() & 0x06) as u8 | (g.bits() & 0xb8) as u8;
        let (in_bike, in_node) = (g.one_in(2), g.one_in(2));
        // Bit 1: the node is in the database.
        set_byte(&mut ee, BIKE + 0x45, u32::from(flags & !0x02) | if in_node { 0x02 } else { 0 });
        let low = u32::from(ee.u8_at(BIKE + 0x44)) & !0xc0 | g.bits() & 0xc0;
        set_byte(&mut ee, BIKE + 0x44, low);
        ee.set_f32(world + 0x98, g.near(0.0, -150.0, 150.0));
        if g.one_in(5) {
            ee.set_u32(world + 0xa8, 0);
        } else {
            ee.set_u32(world + 0xa8, area);
            for (o, sign) in [(0x30, -1.0), (0x40, 1.0)] {
                for i in 0..4 {
                    let x = sign * g.f(20.0, 140.0);
                    ee.set_f32(area + o + 4 * i, x);
                }
            }
        }
        let nf = u32::from(ee.u8_at(node + 0x100)) & !0xc8 | g.bits() & 0xc8;
        set_byte(&mut ee, node + 0x100, nf);
        // Into the database as the game puts them there (the node's world box gathered on the way).
        ee.set(28, GP);
        if in_bike {
            ee.call(BIKE_ADD.1, &[BIKE, DATABASE.0, DATABASE.1].map(u64::from), &[0.0], BUDGET);
        }
        if in_node {
            ee.set_u32(BIKE + 0xb0, DATABASE.0);
            ee.call(NODE_ADD, &[node, DATABASE.0, 1, 1].map(u64::from), &[0.0], BUDGET);
            let nf = u32::from(ee.u8_at(node + 0x100)) & !0xc8 | g.bits() & 0xc8;
            set_byte(&mut ee, node + 0x100, nf);
        }
        assert_eq!((ee.u32_at(BIKE + 8) != 0, ee.u32_at(node + 8) != 0), (in_bike, in_node), "{k}: the database entries");
        tally(&mut t, match (in_bike, in_node) {
            (false, false) => "out of the database",
            (true, false) => "the bike in the database",
            (false, true) => "the node in the database",
            (true, true) => "both in the database",
        });
        let before = answered.clone();
        let d = f47_compare(
            &mut ee,
            &log,
            |ee| {
                ee.set(28, GP);
                ee.call(AIR_STATE, &[u64::from(BIKE)], &[f32::from_bits(DT)], BUDGET);
            },
            |ee| posts_post(ee, F(f32::from_bits(DT)), &mut answered),
        );
        let ran = |w: &str| answered.get(w).copied().unwrap_or(0) > before.get(w).copied().unwrap_or(0);
        tally(&mut t, if ran("OutOfWorld") { "out of the world" } else { "in the world" });
        if ran("BodyReset") {
            tally(&mut t, "body reset");
        }
        if ran("NodeBounds gathered") {
            tally(&mut t, "node bounds gathered");
        }
        off.extend(d.into_iter().map(|d| format!("{k}: {d}")));
    }
    eprintln!("{answered:#?}");
    f47_report(&t, &off, 500);
}

/// **F47 — the record's node with a parent is the original's**: 10,000 times on state 4's bike, its
/// node hung under a parent drawn in [`F47_AREA`] (state 4's node's words, its own frame, its world
/// frame and whether that is current drawn, no parent of its own), the node's own frame and whether
/// it is current drawn, and one of the four node calls the record's pass makes (the turn of the
/// sweep's offset, the position, the placement, the move to the swept point) with a drawn argument —
/// through [`RecordInRam`] (SLUS-21104's node functions) and through [`RecordsInRam`] (F43's
/// [`node`] ports on RAM, as F47 serves a parented node). Every word of RAM and the answers come out
/// the same.
#[test]
#[allow(clippy::float_arithmetic)]
fn the_records_parented_node_is_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    let rec = ee.u32_at(BIKE + seed::RECORD);
    let node = ee.u32_at(rec + 0xe8);
    assert_eq!(node, ee.u32_at(BIKE + 0xe4), "the record's node is the bike's");
    assert_eq!(ee.u32_at(node + NODE_PARENT), 0, "state 4's node has no parent");
    let parent = F47_AREA + 0x4000;
    let log: Calls = Rc::default();
    let base = ee.ram.clone();
    let mut g = Gen(0x0f47_0006);
    let (mut t, mut off) = (BTreeMap::new(), Vec::new());
    let mut answered = BTreeMap::new();
    let frame = |ee: &mut Ee, g: &mut Gen, at: u32| {
        for r in 0..4u32 {
            for c in 0..4u32 {
                let x = match (r, c) {
                    (3, 3) => 1.0,
                    (3, _) => g.near(0.0, -200.0, 200.0),
                    (_, 3) => 0.0,
                    _ if r == c => g.near(1.0, -1.2, 1.2),
                    _ => g.near(0.0, -1.0, 1.0),
                };
                ee.set_f32(at + 0x10 * r + 4 * c, x);
            }
        }
    };
    for k in 0..10_000 {
        ee.ram.clone_from(&base);
        for o in (0..0x150).step_by(4) {
            let w = ee.u32_at(node + o);
            ee.set_u32(parent + o, w);
        }
        for o in [NODE_PARENT, 0xf4, 0xf8, 0x10c] {
            ee.set_u32(parent + o, 0);
        }
        ee.set_u32(node + NODE_PARENT, parent);
        frame(&mut ee, &mut g, parent + 0x30);
        frame(&mut ee, &mut g, parent + 0x70);
        frame(&mut ee, &mut g, node + 0x30);
        let (pc, nc) = (g.one_in(2), g.one_in(2));
        for (n, current) in [(parent, pc), (node, nc)] {
            let f = u32::from(ee.u8_at(n + 0x100)) & !1 | u32::from(current);
            set_byte(&mut ee, n + 0x100, f);
        }
        tally(&mut t, match (pc, nc) {
            (true, true) => "both current",
            (true, false) => "the node out of date",
            (false, true) => "the parent out of date",
            (false, false) => "neither current",
        });
        let v = Vf::new(g.near(0.0, -50.0, 50.0), g.near(0.0, -50.0, 50.0), g.near(0.0, -50.0, 50.0), g.near(1.0, -2.0, 2.0));
        let what = k % 4;
        tally(&mut t, ["the turn", "the position", "the placement", "the move"][what as usize]);
        let run = |c: &mut dyn Collides| match what {
            0 => Some(c.node_rotate(v)),
            1 => Some(c.node_position()),
            2 => Some(c.node_place(v)),
            _ => {
                c.node_move(v);
                None
            }
        };
        let said = |log: &Calls, r: Option<Vf>| {
            if let Some(r) = r {
                log.borrow_mut().push((what, r.to_bits().to_vec()));
            }
        };
        let d = f47_compare(
            &mut ee,
            &log,
            |ee| {
                let mut c = RecordInRam { ee, at: rec, terrain: None, others: BTreeMap::new(), queries: 0 };
                said(&log, run(&mut c));
            },
            |ee| {
                let inner = RecordInRam { ee, at: rec, terrain: None, others: BTreeMap::new(), queries: 0 };
                let mut c = RecordsInRam { inner, answered: &mut answered };
                said(&log, run(&mut c));
            },
        );
        off.extend(d.into_iter().map(|d| format!("{k}: {d}")));
    }
    eprintln!("{answered:#?}");
    assert_eq!(answered.values().sum::<u32>(), 10_000, "every call served parented: {answered:#?}");
    f47_report(&t, &off, 2000);
}

// S278: the rider's "a ragdoll part is touching" flag (handling::rider), the one word of the rider's the
// bike's frame reads (F41).
use game_sim::handling::rider;

/// The tail's rider ([`OTHER_LEAVES`]' third) and its mode machine's ground ask, the call just before the
/// wreck arm's walk of the part list; its parts' clear; its hand-over into the record.
const RIDER: u32 = 0x010a_f350;
const RIDER_GROUND_ASK: u32 = 0x001d_6a60;
const PART_CLEAR: u32 = 0x0049_ae10;
const HAND_OVER: u32 = 0x004f_1210;
/// The hand-over's fake part block (its holder, then the nine words) and the main body's node holder
/// moved off its node by 4 (the quadword load drops it).
const S_HOLDER: u32 = U_AREA + 0x3000;
const S_BLOCK: u32 = U_AREA + 0x3040;

/// The rider's words through the interpreter's RAM, writes kept apart in an overlay when it has one.
struct RiderRam<'a> {
    ee: &'a mut Ee,
    overlay: Option<HashMap<u32, u32>>,
}

impl node::Ram for RiderRam<'_> {
    fn word(&mut self, at: u32) -> u32 {
        self.overlay.as_ref().and_then(|o| o.get(&at).copied()).unwrap_or_else(|| self.ee.u32_at(at))
    }
    fn set(&mut self, at: u32, w: u32) {
        match self.overlay.as_mut() {
            Some(o) => {
                o.insert(at, w);
            }
            None => self.ee.set_u32(at, w),
        }
    }
}

/// The regions the hand-over and the clear may touch: the rider, the bike's record, the rider's parts
/// (the list's and the named), the main body's node, the fake block.
fn rider_regions(ee: &Ee) -> Vec<(u32, u32)> {
    let mut out = vec![(RIDER, 0x700), (ee.u32_at(RIDER + rider::RECORD), 0x260), (S_HOLDER, 0x80)];
    let n = ee.u32_at(RIDER + rider::PARTS_COUNT);
    let list = ee.u32_at(RIDER + rider::PARTS);
    let mut parts: Vec<u32> = (0..n).map(|i| ee.u32_at(list + 4 * i)).collect();
    parts.extend(rider::NAMED.iter().map(|&o| ee.u32_at(RIDER + o)));
    parts.sort();
    parts.dedup();
    out.extend(parts.into_iter().map(|p| (p, 0x80)));
    let main = ee.u32_at(ee.u32_at(RIDER + rider::MAIN) + 0xa4);
    out.push((main & !0xf, 0x90));
    out
}

/// **The rider's hand-over into the bike's record and a part's clear are the original's**: 10 000
/// calls of each on state 4's rider, with every word they read drawn at random — the flag (0, 1, any
/// other bits), each of the nine named parts any of the rider's parts, each part's touched word (0 or
/// not) and velocity change (signed zeros among them), the impact left from before, the main body's
/// node position (its holder's pointer off by 4 one time in four), the part block's holder (none or a
/// fake one) — through SLUS-21104's and the port's. Every word of the rider, the record, the parts, the
/// node and the block comes out the same, and neither writes anywhere else.
#[test]
fn the_riders_hand_over_and_part_clear_are_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    let regions = rider_regions(&ee);
    let image_words = snap_regions(&ee, &regions);
    let list = ee.u32_at(RIDER + rider::PARTS);
    let parts: Vec<u32> = (0..ee.u32_at(RIDER + rider::PARTS_COUNT)).map(|i| ee.u32_at(list + 4 * i)).collect();
    let main_holder = ee.u32_at(RIDER + rider::MAIN);
    let main = ee.u32_at(main_holder + 0xa4);
    assert_eq!(main & 0xf, 0, "the main body's node is aligned");
    let record = ee.u32_at(RIDER + rider::RECORD);
    assert_eq!(record, BIKE + 0x730, "the rider's record is the bike's");
    let mut g = Gen(0x0027_8001);
    let mut t = BTreeMap::new();
    for k in 0..20_000 {
        restore_regions(&mut ee, &regions, &image_words);
        let f = |g: &mut Gen| {
            let x = g.near(0.0, -40.0, 40.0);
            if g.one_in(9) { -x } else { x }
        };
        let flag = match g.bits() % 3 {
            0 => 0,
            1 => 1,
            _ => g.bits() | 2,
        };
        ee.set_u32(RIDER + rider::TOUCHING, flag);
        for o in rider::NAMED {
            ee.set_u32(RIDER + o, parts[(g.bits() as usize) % parts.len()]);
        }
        for &p in &parts {
            ee.set_u32(p + rider::TOUCHED, if g.one_in(2) { 0 } else { g.bits() | 1 });
            for i in 0..4 {
                let v = f(&mut g);
                ee.set_f32(p + rider::DV + 4 * i, v);
                ee.set_u32(p + 0x18 + 4 * i, g.bits());
            }
        }
        for i in 0..4 {
            let (a, b) = (f(&mut g), f(&mut g));
            ee.set_f32(RIDER + rider::IMPACT + 4 * i, a);
            ee.set_f32(main + 0x70 + 4 * i, b);
            ee.set_u32(record + rider::R_IMPACT + 4 * i, g.bits());
        }
        ee.set_u32(main_holder + 0xa4, if g.one_in(4) { main + 4 } else { main });
        let block = !g.one_in(3);
        ee.set_u32(record + rider::R_BLOCK, if block { S_HOLDER } else { 0 });
        ee.set_u32(S_HOLDER + 0x14, S_BLOCK);
        let clear = g.one_in(2);
        let target = parts[(g.bits() as usize) % parts.len()];
        let given = snap_regions(&ee, &regions);

        ee.set(28, GP);
        ee.written.clear();
        if clear {
            ee.call(PART_CLEAR, &[u64::from(target)], &[], BUDGET);
        } else {
            ee.call(HAND_OVER, &[u64::from(RIDER)], &[], BUDGET);
        }
        let stack = ee.stack;
        let outside: Vec<u32> = ee.written.iter().copied().filter(|&a| !(stack - 0x4_0000..stack + 0x1000).contains(&a) && !regions.iter().any(|&(at, n)| (at..at + n).contains(&a))).collect();
        assert!(outside.is_empty(), "case {k}: the original wrote outside the regions: {:#x?}", &outside[..outside.len().min(8)]);
        let want = snap_regions(&ee, &regions);

        restore_regions(&mut ee, &regions, &given);
        ee.written.clear();
        let mut m = RiderRam { ee: &mut ee, overlay: None };
        if clear {
            rider::clear(&mut m, target);
        } else {
            rider::hand_over(&mut m, RIDER);
        }
        let outside: Vec<u32> = ee.written.iter().copied().filter(|&a| !regions.iter().any(|&(at, n)| (at..at + n).contains(&a))).collect();
        assert!(outside.is_empty(), "case {k}: the port wrote outside the regions");
        let got = snap_regions(&ee, &regions);
        let addrs: Vec<u32> = regions.iter().flat_map(|&(at, n)| (0..n).step_by(4).map(move |o| at + o)).collect();
        for ((w, gw), at) in want.iter().zip(&got).zip(&addrs) {
            assert_eq!(gw, w, "case {k} ({}): the word at {at:#x}", if clear { "clear" } else { "hand-over" });
        }
        if clear {
            tally(&mut t, "a part cleared");
        } else {
            tally(&mut t, if flag == 0 { "flag off: the rest quadword" } else { "flag on: the impact" });
            tally(&mut t, if block { "the part block written" } else { "no part block" });
            if flag != 0 {
                let touched = rider::NAMED.iter().filter(|&&o| ee.u32_at(ee.u32_at(RIDER + o) + rider::TOUCHED) != 0).count();
                tally(&mut t, ["no named part touched", "one touched", "some touched", "some touched", "some touched", "some touched", "some touched", "some touched", "some touched", "all nine touched"][touched]);
            }
            if ee.u32_at(main_holder + 0xa4) != main {
                tally(&mut t, "the node's pointer unaligned");
            }
        }
    }
    eprintln!("{t:#?}");
    for (path, n) in &t {
        assert!(*n >= 20, "path {path} taken only {n} times");
    }
    assert!(t.len() >= 10, "every path taken: {t:#?}");
}

/// What [`rider_flag_replay`] saw: frames, the wreck arm's walks (and of them those that set the flag),
/// clears, hand-overs, and every row's `SETTLES` as the port handed it over.
#[derive(Debug, Default)]
struct RiderFlag {
    walks: usize,
    set: usize,
    clears: usize,
    hand_overs: usize,
    settles: Vec<u32>,
}

/// [`replay`] with the rider's part clear and hand-over served by the port ([`rider::clear`],
/// [`rider::hand_over`]), and the wreck arm's walk checked against [`rider::touching`]: the port's flag
/// and clock, worked out from the parts' touched words where the walk reads them (just after the
/// mode machine's ground ask, which only the wreck arm makes) and the frame's `dt`, are what the
/// original's walk leaves at the first clear after it. Every row is still checked bit-exact, and the
/// bike's `SETTLES` the port handed over against the row's.
fn rider_flag_replay(name: &str) -> Option<RiderFlag> {
    let mut ee = hooked(image()?);
    let rows = rows(name)?;
    let t: Rc<RefCell<(RiderFlag, Option<HashMap<u32, u32>>)>> = Rc::default();
    let h = t.clone();
    ee.hooks.insert(
        RIDER_GROUND_ASK,
        Box::new(move |ee: &mut Ee| {
            let (args, x, ra, sp, stack) = ([ee.r(4), ee.r(5), ee.r(6), ee.r(7)], ee.f(12), ee.r(31), ee.r(29), ee.stack);
            let ours = ee.r(4) as u32 == ee.u32_at(RIDER + 0x5b8);
            ee.stack = (sp as u32 - 0x40) & !0xf;
            ee.call(RIDER_GROUND_ASK, &args, &[x], BUDGET);
            (ee.stack, ee.pc) = (stack, RIDER_GROUND_ASK);
            ee.set(29, sp);
            ee.set(31, ra);
            if ours {
                // The walk is next, its dt the mode machine's (f20, kept across the call).
                let dt = F(ee.f(20));
                let mut m = RiderRam { ee, overlay: Some(HashMap::new()) };
                rider::touching(&mut m, RIDER, dt);
                h.borrow_mut().1 = m.overlay;
            }
        }),
    );
    let h = t.clone();
    ee.hooks.insert(
        PART_CLEAR,
        Box::new(move |ee: &mut Ee| {
            let mut t = h.borrow_mut();
            if let Some(o) = t.1.take() {
                for at in [rider::TOUCHING, rider::TOUCHING_TIME] {
                    assert_eq!(ee.u32_at(RIDER + at), o[&(RIDER + at)], "the wreck arm's walk: rider +{at:#x}");
                }
                t.0.walks += 1;
                t.0.set += usize::from(ee.u32_at(RIDER + rider::TOUCHING) != 0);
            }
            t.0.clears += 1;
            let part = ee.r(4) as u32;
            rider::clear(&mut RiderRam { ee, overlay: None }, part);
        }),
    );
    let h = t.clone();
    ee.hooks.insert(
        HAND_OVER,
        Box::new(move |ee: &mut Ee| {
            assert_eq!(ee.r(4) as u32, RIDER, "the hand-over's rider");
            h.borrow_mut().0.hand_overs += 1;
            rider::hand_over(&mut RiderRam { ee, overlay: None }, RIDER);
        }),
    );
    for (r, row) in rows.iter().enumerate() {
        poke(&mut ee, row, &READER);
        ee.set(28, GP);
        ee.call(FRAME, &[u64::from(SIM)], &[f32::from_bits(DT)], 400_000_000);
        let d = diff(&ee, row);
        assert!(d.is_empty(), "{name} row {r} (vsync {}): {} words differ\n  {}", row["vsync"], d.len(), d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  "));
        let o = obj(row);
        let settles = ee.u32_at(BIKE + wreck::SETTLES);
        assert_eq!(settles, o[wreck::SETTLES as usize / 4], "{name} row {r}: the bike's SETTLES");
        let w = wrecked(|o| ee.u32_at(BIKE + o));
        assert_eq!(w, row_wrecked(row), "{name} row {r}: the wreck state");
        t.borrow_mut().0.settles.push(settles);
        poke(&mut ee, row, &SPOT);
    }
    assert!(t.borrow().1.is_none(), "a walk left unchecked");
    for a in [RIDER_GROUND_ASK, PART_CLEAR, HAND_OVER] {
        ee.hooks.remove(&a);
    }
    Some(Rc::try_unwrap(t).ok().unwrap().into_inner().0)
}

/// **The rider's flag in the port's hands, both C15 replays bit-exact**: the rider's part clear and
/// hand-over are the port's, and the wreck arm's walk is checked against the port's on every frame it
/// runs ([`rider_flag_replay`]). The ride: no walk, `SETTLES` 0 throughout. The flip: the walk from the
/// wreck on, the flag set from row 128 (three rows after the wreck) on every row after, and the
/// clock's put-back on row 193 as before. What sets the parts' touched words is still the original's
/// ragdoll world (S278a).
#[test]
fn the_riders_flag_reaches_the_wreck_clock_through_the_port() {
    for (name, n) in [("ride-2", 459usize), ("flip-1", 465)] {
        let Some(t) = rider_flag_replay(name) else { return };
        eprintln!("{name}: {} walks ({} set), {} clears, {} hand-overs", t.walks, t.set, t.clears, t.hand_overs);
        assert_eq!(t.settles.len(), n);
        assert_eq!(t.hand_overs, n, "{name}: one hand-over a frame");
        let first = t.settles.iter().position(|&s| s != 0);
        if name == "ride-2" {
            assert_eq!(t.walks, 0, "{name}: the ride never reaches the wreck arm");
            assert_eq!(first, None);
        } else {
            assert!(t.walks > 0 && t.set > 0 && t.set < t.walks, "{name}: walks both ways");
            assert_eq!(first, Some(128), "{name}: the first row SETTLES is set");
            assert!(t.settles[128..].iter().all(|&s| s == 1), "{name}: SETTLES stays set");
        }
    }
}

/// The wreck arm's walk and the mode machine's clear loop, as runs of the mode machine's own
/// instructions (the rider in `s2`, `dt` in `f20`): each from its first instruction to the one after
/// it, where a hook ends the call.
const WALK: (u32, u32) = (0x004e_ffb4, 0x004f_0020);
const CLEAR_LOOP: (u32, u32) = (0x004f_0274, 0x004f_02b0);
/// A fake part list of up to twelve and its parts, in the F39 rig's area.
const S_LIST: u32 = U_AREA + 0x3100;
const S_PARTS: u32 = U_AREA + 0x3200;

/// **The wreck arm's walk and the clear loop are the original's**: 10 000 runs of each stretch of the
/// mode machine on state 4's rider, its list a fake one — the count −2 to 12, each part's four flags
/// (touched 0 or not, at any place in the list or none), the flag and its clock left from before
/// (signed zeros among them), `dt` the game's or any — against [`rider::touching`] and
/// [`rider::clear_all`]. The rider's, the list's and the parts' words come out the same, and neither
/// writes anywhere else.
#[test]
fn the_riders_walk_and_clear_loop_are_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    for (_, end) in [WALK, CLEAR_LOOP] {
        // The loop's own calls leave `ra` theirs: the end goes back to the call's.
        ee.hooks.insert(end, Box::new(|ee: &mut Ee| ee.set(31, u64::from(ps2_oracle::DONE) | 0xffff_ffff_0000_0000)));
    }
    let regions = vec![(RIDER, 0x700), (S_LIST, 0x40), (S_PARTS, 0x80 * 12)];
    let image_words = snap_regions(&ee, &regions);
    let mut g = Gen(0x0027_8002);
    let mut t = BTreeMap::new();
    for k in 0..20_000 {
        restore_regions(&mut ee, &regions, &image_words);
        let walk = k % 2 == 0;
        let n = (g.bits() % 15) as i32 - 2;
        ee.set_u32(RIDER + rider::PARTS_COUNT, n as u32);
        ee.set_u32(RIDER + rider::PARTS, S_LIST);
        let first = if g.one_in(3) { 99 } else { g.bits() % 12 };
        for i in 0..12 {
            let p = S_PARTS + 0x80 * i;
            ee.set_u32(S_LIST + 4 * i, p);
            let touched = if i == first { g.bits() | 1 } else if i > first && g.one_in(2) { g.bits() } else { 0 };
            ee.set_u32(p + rider::TOUCHED, touched);
            for o in [0x18, 0x1c, 0x20] {
                ee.set_u32(p + o, g.bits());
            }
        }
        ee.set_u32(RIDER + rider::TOUCHING, if g.one_in(2) { 0 } else { g.bits() });
        let clock = g.near(0.0, -1.0, 9.0);
        ee.set_f32(RIDER + rider::TOUCHING_TIME, clock);
        let dt = if g.one_in(2) { f32::from_bits(DT) } else { g.near(0.0, -0.05, 0.05) };
        let given = snap_regions(&ee, &regions);

        let (start, _) = if walk { WALK } else { CLEAR_LOOP };
        ee.set(28, GP);
        ee.set(18, u64::from(RIDER));
        ee.set_f(20, dt);
        ee.written.clear();
        ee.call(start, &[], &[], BUDGET);
        let stack = ee.stack;
        let outside: Vec<u32> = ee.written.iter().copied().filter(|&a| !(stack - 0x4_0000..stack + 0x1000).contains(&a) && !regions.iter().any(|&(at, n)| (at..at + n).contains(&a))).collect();
        assert!(outside.is_empty(), "case {k}: the original wrote outside the regions: {:#x?}", &outside[..outside.len().min(8)]);
        let want = snap_regions(&ee, &regions);

        restore_regions(&mut ee, &regions, &given);
        ee.written.clear();
        let mut m = RiderRam { ee: &mut ee, overlay: None };
        if walk {
            rider::touching(&mut m, RIDER, F(dt));
        } else {
            rider::clear_all(&mut m, RIDER);
        }
        let outside = ee.written.iter().filter(|&&a| !regions.iter().any(|&(at, n)| (at..at + n).contains(&a))).count();
        assert_eq!(outside, 0, "case {k}: the port wrote outside the regions");
        let got = snap_regions(&ee, &regions);
        let addrs: Vec<u32> = regions.iter().flat_map(|&(at, n)| (0..n).step_by(4).map(move |o| at + o)).collect();
        for ((w, gw), at) in want.iter().zip(&got).zip(&addrs) {
            assert_eq!(gw, w, "case {k} ({}, count {n}): the word at {at:#x}", if walk { "walk" } else { "clear loop" });
        }
        let set = ee.u32_at(RIDER + rider::TOUCHING) != 0;
        tally(&mut t, match (walk, n) {
            (true, n) if n < 0 => "walk, a negative count",
            (true, 0) => "walk, no parts",
            (true, _) if set => "walk, a part touched",
            (true, _) => "walk, none touched",
            (false, n) if n <= 0 => "clear loop, nothing to clear",
            (false, _) => "clear loop, parts cleared",
        });
    }
    eprintln!("{t:#?}");
    for (path, n) in &t {
        assert!(*n >= 20, "path {path} taken only {n} times");
    }
    assert_eq!(t.len(), 6, "every path taken: {t:#?}");
}

// --- S278a: the rider ragdoll world's bone collision ----------------------------------------------

use game_sim::camera::CurveTable;
use game_sim::handling::bone;

/// The bone collision's solve and push-out, and the scene's calls they make: the sphere moved, a
/// shape's virtual (the sphere's `vt +200`), the probe's segment set and the probe brought up to date,
/// the search in a pool and in the whole scene.
const BONE_SOLVE: u32 = 0x0049_bb00;
const BONE_PUSH_OUT: u32 = 0x0049_b220;
const SHAPE_PLACE: u32 = 0x001c_1e90;
const SHAPE_VT_REFRESH: u32 = 200;
const SHAPE_SEGMENT: u32 = 0x001b_9290;
const SHAPE_REFRESH: u32 = 0x0011_4360;
const SHAPE_SEARCH: u32 = 0x0011_51c0;
const SHAPE_SEARCH_ALL: u32 = 0x0011_48e0;
/// A mover's two virtuals, spin and velocity (each writing a quadword to `a0`).
/// The ground query's cache of read cells, ten of 0x70 bytes (with the terrain's `+0x9e0` set-up flag
/// and `+0x9e4` index): keyed by the sample and filled from the map, so the port, reading the map,
/// keeps none.
const GROUND_RING: u32 = 0x0054_1550;
const MOVER_SPIN_VT: u32 = 0xc4;
const MOVER_VELOCITY_VT: u32 = 0xc0;
/// The fake objects of the bone collision oracle, in the F39 rig's area past S278's: a body to turn,
/// a collidable, its collision object, the object's dynamics body and chassis, a node and its parent,
/// a mover, its vtable and two functions in it (hooked: never run), what they answer, two hit records
/// (the sphere's, the probe's) and the probe's swept point and normal: in the 12 KiB at `0x01cb_b000`,
/// the only free words of state 4 near the others.
const B_AREA: u32 = 0x01cb_b000;
const B_TURNED: u32 = B_AREA;
const B_COLL: u32 = B_AREA + 0x200;
const B_OBJECT: u32 = B_AREA + 0x300;
const B_BODY2: u32 = B_AREA + 0x400;
const B_CHASSIS: u32 = B_AREA + 0x600;
const B_NODE: u32 = B_AREA + 0x900;
const B_MOVER: u32 = B_AREA + 0xb00;
const B_VT: u32 = B_AREA + 0xe00;
const B_SPIN_FN: u32 = B_AREA + 0xf00;
const B_VEL_FN: u32 = B_AREA + 0xf10;
const B_SPIN: u32 = B_AREA + 0xf20;
const B_VEL: u32 = B_AREA + 0xf30;
const B_HITS: u32 = B_AREA + 0x1000;
const B_SWEPT_HITS: u32 = B_AREA + 0x1100;
const B_SWEPT_AT: u32 = B_AREA + 0x1200;
const B_PARENT: u32 = B_AREA + 0x1300;
const B_END: u32 = B_AREA + 0x1500;

/// Where a hook's nested calls take and leave their quadwords: the 64 bytes under the hooked call's
/// `sp`, above the nested calls' stack ([`call_from_hook`]).
fn hook_scratch(ee: &Ee) -> u32 {
    (ee.r(29) as u32 - 0x40) & !0xf
}

/// Calls `f` from inside a hook, on the stack below the hooked call's and [`hook_scratch`], and puts
/// the hook's `ra`, `sp` and `pc` back after.
fn call_from_hook(ee: &mut Ee, f: u32, ints: &[u64]) {
    let (pc, ra, sp, stack) = (ee.pc, ee.r(31), ee.r(29), ee.stack);
    ee.stack = (sp as u32 - 0x80) & !0xf;
    ee.set(28, GP);
    ee.call(f, ints, &[], BUDGET);
    (ee.stack, ee.pc) = (stack, pc);
    ee.set(29, sp);
    ee.set(31, ra);
}

/// What the oracle's fake scene was asked, and what it answers: per shape, a hit or not and the hit
/// record it leaves.
#[derive(Default)]
struct Asked {
    log: Vec<String>,
    hit: HashMap<u32, (bool, u32)>,
}

/// A search's answer: the shape's hit bit and its hit record.
fn answer(m: &mut impl node::Ram, asked: &Asked, shape: u32) {
    let (hit, rec) = asked.hit.get(&shape).copied().unwrap_or((false, 0));
    let (at, s) = ((shape + bone::S_HIT_BYTE) & !3, 8 * ((shape + bone::S_HIT_BYTE) & 3));
    let w = m.word(at);
    m.set(at, w & !(bone::S_HIT_BIT << s) | u32::from(hit) * bone::S_HIT_BIT << s);
    m.set(shape + bone::S_RECORD, rec);
}

fn quad_bits(m: &mut impl node::Ram, at: u32) -> [u32; 4] {
    [0, 4, 8, 12].map(|k| m.word(at + k))
}

/// The fake scene on the port's side: the same log and answers as the hooks on the original's.
struct FakeScene(Rc<RefCell<Asked>>);

impl<'a> bone::Scene<RiderRam<'a>> for FakeScene {
    fn place(&mut self, _: &mut RiderRam<'a>, sphere: u32, at: u32) {
        let mut a = self.0.borrow_mut();
        a.log.push(format!("place {sphere:#x} at {at:#x}"));
        a.log.push(format!("refresh {sphere:#x}"));
    }
    fn segment(&mut self, _: &mut RiderRam<'a>, probe: u32, from: Vf, to: Vf) {
        let mut a = self.0.borrow_mut();
        a.log.push(format!("segment {probe:#x} {:08x?} {:08x?}", from.to_bits(), to.to_bits()));
        a.log.push(format!("refresh {probe:#x}"));
    }
    fn search(&mut self, m: &mut RiderRam<'a>, shape: u32, pool: u32) {
        let a = &mut *self.0.borrow_mut();
        a.log.push(format!("search {shape:#x} in {pool:#x}"));
        answer(m, a, shape);
    }
    fn search_all(&mut self, m: &mut RiderRam<'a>, shape: u32) {
        let a = &mut *self.0.borrow_mut();
        a.log.push(format!("search {shape:#x}"));
        answer(m, a, shape);
    }
    fn spin(&mut self, m: &mut RiderRam<'a>, mover: u32) -> Vf {
        self.0.borrow_mut().log.push(format!("spin {mover:#x}"));
        Vf::from_bits(quad_bits(m, B_SPIN))
    }
    fn velocity(&mut self, m: &mut RiderRam<'a>, mover: u32) -> Vf {
        self.0.borrow_mut().log.push(format!("velocity {mover:#x}"));
        Vf::from_bits(quad_bits(m, B_VEL))
    }
}

/// The scene on the interpreter's: each call the original's own, from inside the solve's hook.
struct OriginalScene;

impl<'a> bone::Scene<RiderRam<'a>> for OriginalScene {
    fn place(&mut self, m: &mut RiderRam<'a>, sphere: u32, at: u32) {
        call_from_hook(m.ee, SHAPE_PLACE, &[u64::from(sphere), u64::from(at), 0]);
        let f = m.ee.u32_at(m.ee.u32_at(sphere + 0xc) + SHAPE_VT_REFRESH);
        call_from_hook(m.ee, f, &[u64::from(sphere)]);
    }
    fn segment(&mut self, m: &mut RiderRam<'a>, probe: u32, from: Vf, to: Vf) {
        let out = hook_scratch(m.ee);
        for (k, w) in from.to_bits().into_iter().chain(to.to_bits()).enumerate() {
            m.ee.set_u32(out + 4 * k as u32, w);
        }
        call_from_hook(m.ee, SHAPE_SEGMENT, &[u64::from(probe), u64::from(out), 0]);
        call_from_hook(m.ee, SHAPE_REFRESH, &[u64::from(probe)]);
    }
    fn search(&mut self, m: &mut RiderRam<'a>, shape: u32, pool: u32) {
        call_from_hook(m.ee, SHAPE_SEARCH, &[u64::from(shape), u64::from(pool), 1]);
    }
    fn search_all(&mut self, m: &mut RiderRam<'a>, shape: u32) {
        call_from_hook(m.ee, SHAPE_SEARCH_ALL, &[u64::from(shape), 0]);
    }
    fn spin(&mut self, m: &mut RiderRam<'a>, mover: u32) -> Vf {
        let (f, out) = (m.ee.u32_at(m.ee.u32_at(mover + 4) + MOVER_SPIN_VT), hook_scratch(m.ee));
        call_from_hook(m.ee, f, &[u64::from(out), u64::from(mover)]);
        Vf::from_bits(quad_bits(m, out))
    }
    fn velocity(&mut self, m: &mut RiderRam<'a>, mover: u32) -> Vf {
        let (f, out) = (m.ee.u32_at(m.ee.u32_at(mover + 4) + MOVER_VELOCITY_VT), hook_scratch(m.ee));
        call_from_hook(m.ee, f, &[u64::from(out), u64::from(mover)]);
        Vf::from_bits(quad_bits(m, out))
    }
}

/// The rider's ragdoll parts (the rider's list), in state 4.
fn rider_parts(ee: &Ee) -> Vec<u32> {
    let list = ee.u32_at(RIDER + rider::PARTS);
    (0..ee.u32_at(RIDER + rider::PARTS_COUNT)).map(|i| ee.u32_at(list + 4 * i)).collect()
}

/// **The bone collision's push-out and solve are the original's**: 24 000 calls on state 4's rider
/// parts, half of each, through SLUS-21104's and the port's, with the scene's calls answered by a fake
/// scene on both sides (hooks on the original's, [`FakeScene`] on the port's: each call logged, each
/// search's hit and hit record drawn per case) and every word they read drawn at random: the part's
/// flags, factors, scratch and the body it turns (none, its own, another), the body's position near
/// its own, velocity, radius, mass and frame; the point and normal; the other side (none, a
/// collidable with nothing, with a dynamics body, with a vehicle's chassis and node — the node current
/// or not, under a parent or not —, with a mover of class 2 or not, both); and for the solve, the part
/// on or not, its body there and enabled or not, the terrain there or not and the point in its box or
/// at or past its edge, the body above or below the ground, the scene's hit with −1 to 3 contacts, the
/// sweep on or not, its probe there and its hit, the pool there or not, `dt` the game's, half of it or
/// any. Every word of the parts, their bodies, shapes and the fakes comes out the same, the scene is
/// asked the same in the same order, and neither writes anywhere else.
#[test]
fn the_bone_collision_is_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    let Some(trn) = nat05_trn() else { return };
    let map = track::Terrain::parse(&trn).unwrap();
    let curve = CurveTable::build();
    let parts = rider_parts(&ee);
    assert_eq!(parts.len(), 9, "the rider's parts");
    for a in (B_AREA..B_END).step_by(4) {
        assert_eq!(ee.u32_at(a), 0, "the fakes' area is free at {a:#x}");
    }
    let asked: Rc<RefCell<Asked>> = Rc::default();
    let mut regions = vec![(B_AREA, B_END - B_AREA)];
    let mut shapes = Vec::new();
    for &p in &parts {
        let b = ee.u32_at(p + bone::BODY);
        assert_eq!((p & 0xf, b & 0xf), (0, 0), "the part and its body are aligned");
        regions.push((p, 0xa0));
        regions.push((b, 0x1e0));
        for o in [bone::SPHERE, bone::PROBE] {
            let s = ee.u32_at(p + o);
            assert_ne!(s, 0);
            shapes.push(s);
            regions.push((s & !0xf, 0x130));
        }
    }
    regions.sort();
    regions.dedup();
    // The original's side of the fake scene.
    let log = |name: &'static str, quads: bool| {
        let a = asked.clone();
        Box::new(move |ee: &mut Ee| {
            let (a0, a1) = (ee.r(4) as u32, ee.r(5) as u32);
            let line = match name {
                "place" => format!("place {a0:#x} at {a1:#x}"),
                "segment" if quads => format!("segment {a0:#x} {:08x?} {:08x?}", quad_bits(&mut RiderRam { ee, overlay: None }, a1), quad_bits(&mut RiderRam { ee, overlay: None }, a1 + 16)),
                "search" => format!("search {a0:#x} in {a1:#x}"),
                "search all" => format!("search {a0:#x}"),
                "spin" | "velocity" => {
                    let from = if name == "spin" { B_SPIN } else { B_VEL };
                    for k in 0..4 {
                        let w = ee.u32_at(from + 4 * k);
                        ee.set_u32(a0 + 4 * k, w);
                    }
                    format!("{name} {a1:#x}")
                }
                _ => format!("refresh {a0:#x}"),
            };
            let mut a = a.borrow_mut();
            a.log.push(line);
            if name.starts_with("search") {
                assert!(name != "search" || ee.r(6) == 1, "the pool search's third argument");
                answer(&mut RiderRam { ee, overlay: None }, &a, a0);
            }
        }) as Box<dyn FnMut(&mut Ee)>
    };
    ee.hooks.insert(SHAPE_PLACE, log("place", false));
    ee.hooks.insert(SHAPE_SEGMENT, log("segment", true));
    ee.hooks.insert(SHAPE_REFRESH, log("refresh", false));
    ee.hooks.insert(SHAPE_SEARCH, log("search", false));
    ee.hooks.insert(SHAPE_SEARCH_ALL, log("search all", false));
    ee.hooks.insert(B_SPIN_FN, log("spin", false));
    ee.hooks.insert(B_VEL_FN, log("velocity", false));
    for &s in &shapes {
        let f = ee.u32_at(ee.u32_at(s + 0xc) + SHAPE_VT_REFRESH);
        ee.hooks.entry(f).or_insert_with(|| log("refresh", false));
    }
    let image_words = snap_regions(&ee, &regions);
    let addrs: Vec<u32> = regions.iter().flat_map(|&(at, n)| (0..n).step_by(4).map(move |o| at + o)).collect();
    let mut g = Gen(0x0278_a001);
    let mut t = BTreeMap::new();
    for k in 0..24_000 {
        restore_regions(&mut ee, &regions, &image_words);
        let solve = k % 2 == 1;
        let p = parts[(g.bits() as usize) % parts.len()];
        let b = ee.u32_at(p + bone::BODY);
        let sv = |g: &mut Gen, lo: f32, hi: f32| g.near(0.0, lo, hi);
        let put = |ee: &mut Ee, at: u32, v: [f32; 4]| {
            for (i, x) in v.into_iter().enumerate() {
                ee.set_f32(at + 4 * i as u32, x);
            }
        };
        let quat = |g: &mut Gen| [sv(g, -1.0, 1.0), sv(g, -1.0, 1.0), sv(g, -1.0, 1.0), sv(g, -1.0, 1.0)];
        let vec = |g: &mut Gen, r: f32| [sv(g, -r, r), sv(g, -r, r), sv(g, -r, r), if g.one_in(4) { sv(g, -r, r) } else { 1.0 }];
        // The body: near where it is, any velocity, frame and mass.
        let at = [0, 4, 8].map(|o| ee.f32_at(b + bone::B_POSITION + o));
        let body = |ee: &mut Ee, g: &mut Gen, b: u32, at: [f32; 3]| {
            let w = if g.one_in(4) { g.f(-2.0, 2.0) } else { 1.0 };
            put(ee, b + bone::B_POSITION, [at[0] + g.f(-1.5, 1.5), at[1] + g.f(-3.0, 3.0), at[2] + g.f(-1.5, 1.5), w]);
            put(ee, b + bone::B_VELOCITY, vec(g, 15.0));
            put(ee, b + bone::B_QUAT, quat(g));
            put(ee, b + bone::B_RATES, vec(g, 8.0));
            put(ee, b + bone::B_INV_INERTIA, [g.f(0.0, 6.0), g.f(0.0, 6.0), g.f(0.0, 6.0), sv(g, -1.0, 1.0)]);
            ee.set_f32(b + bone::B_INV_MASS, g.f(0.05, 4.0));
            ee.set_f32(b + bone::B_RADIUS, g.f(0.02, 1.0));
        };
        body(&mut ee, &mut g, b, at);
        body(&mut ee, &mut g, B_TURNED, at);
        body(&mut ee, &mut g, B_BODY2, [at[0] + 1.0, at[1], at[2]]);
        let turned = match g.bits() % 6 {
            0 => 0,
            1 | 2 => B_TURNED,
            _ => b,
        };
        ee.set_u32(p + bone::TURNED, turned);
        ee.set_u32(p + bone::SWEPT, u32::from(g.one_in(4)));
        for (o, lo, hi) in [(bone::UNPENETRATION, 0.0, 1.5), (bone::ROTATION, 0.0, 2.0), (bone::RESTITUTION, 0.0, 1.0), (bone::FRICTION, 0.0, 1.0)] {
            ee.set_f32(p + o, g.near(0.0, lo, hi));
        }
        put(&mut ee, p + bone::DV, vec(&mut g, 5.0));
        put(&mut ee, p + bone::SCRATCH, [g.f(0.0, 4.0), 0.0, 0.0, if g.one_in(4) { sv(&mut g, -0.5, 0.5) } else { 0.0 }]);
        put(&mut ee, p + bone::POINT, [at[0] + g.f(-1.5, 1.5), at[1] + g.f(-1.5, 1.5), at[2] + g.f(-1.5, 1.5), 1.0]);
        put(&mut ee, p + bone::NORMAL, vec(&mut g, 1.0));
        // The other side.
        let other = |ee: &mut Ee, g: &mut Gen| -> (u32, &'static str) {
            if g.one_in(4) {
                return (0, "against nothing");
            }
            let mover = !g.one_in(3);
            let object = g.bits() % 3;
            ee.set_u32(B_COLL + bone::C_MOVER, if mover { B_MOVER } else { 0 });
            ee.set_u32(B_COLL + bone::C_OBJECT, if object == 0 { 0 } else { B_OBJECT });
            ee.set_u32(B_OBJECT + bone::O_BODY, if object == 1 { B_BODY2 } else { 0 });
            let class = ee.u32_at(B_MOVER + bone::MOVER_CLASS);
            (
                B_COLL,
                match (object, mover, class == bone::MOVER_IS) {
                    (1, ..) => "against a dynamics body",
                    (2, ..) => "against a vehicle",
                    (_, true, true) => "against a mover",
                    (_, true, false) => "against a mover not of class 2",
                    (_, false, _) => "against a collidable with neither",
                },
            )
        };
        ee.set_u32(B_MOVER + bone::MOVER_CLASS, if g.one_in(4) { 1 } else { bone::MOVER_IS });
        ee.set_u32(B_MOVER + 4, B_VT);
        ee.set_u32(B_VT + MOVER_SPIN_VT, B_SPIN_FN);
        ee.set_u32(B_VT + MOVER_VELOCITY_VT, B_VEL_FN);
        ee.set_u32(B_MOVER + bone::MOVER_NODE, B_NODE);
        put(&mut ee, B_MOVER + bone::MOVER_ORIGIN, vec(&mut g, 2.0));
        put(&mut ee, B_SPIN, vec(&mut g, 5.0));
        put(&mut ee, B_VEL, vec(&mut g, 15.0));
        ee.set_u32(B_OBJECT + bone::O_NODE, B_NODE);
        ee.set_u32(B_OBJECT + bone::O_CHASSIS, B_CHASSIS);
        put(&mut ee, B_OBJECT + bone::O_COFG, vec(&mut g, 1.0));
        put(&mut ee, B_OBJECT + bone::O_COFG_WORLD, vec(&mut g, 1.0));
        put(&mut ee, B_CHASSIS + bone::CH_COFG, vec(&mut g, 1.0));
        put(&mut ee, B_CHASSIS + bone::CH_VELOCITY, vec(&mut g, 15.0));
        put(&mut ee, B_CHASSIS + bone::CH_RATES, vec(&mut g, 5.0));
        for n in [B_NODE, B_PARENT] {
            for r in 0..8 {
                let mut v = vec(&mut g, 1.0);
                if r == 3 || r == 7 {
                    v = [at[0] + g.f(-3.0, 3.0), at[1] + g.f(-3.0, 3.0), at[2] + g.f(-3.0, 3.0), 1.0];
                }
                put(&mut ee, n + 0x30 + 0x10 * r, v);
            }
        }
        let parented = g.one_in(2);
        ee.set_u32(B_NODE + 0xf0, if parented { B_PARENT } else { 0 });
        ee.set_u32(B_NODE + 0x100, if g.one_in(2) { 1 } else { 0 });
        ee.set_u32(B_PARENT + 0x100, 1);
        let (coll, against) = other(&mut ee, &mut g);
        ee.set_u32(p + bone::OTHER, coll);
        tally(&mut t, against);
        // The solve's words and the fake scene's answers.
        let dt = match g.bits() % 3 {
            0 => f32::from_bits(DT),
            1 => f32::from_bits(0x3c88_850a),
            _ => g.f(0.0, 0.1),
        };
        if solve {
            ee.set_u32(p + bone::ON, u32::from(!g.one_in(12)));
            if g.one_in(16) {
                ee.set_u32(b + bone::B_ENABLED, 0);
            }
            if g.one_in(24) {
                ee.set_u32(p + bone::BODY, 0);
            }
            let terrain = ee.u32_at(p + bone::TERRAIN);
            match g.bits() % 10 {
                0 => {
                    ee.set_u32(p + bone::TERRAIN, 0);
                    tally(&mut t, "solve: no terrain");
                }
                1 => {
                    tally(&mut t, "solve: at or past the terrain's edge");
                    let edge = [0x30, 0x40, 0x38, 0x48][(g.bits() % 4) as usize];
                    let x = ee.f32_at(terrain + edge) + if g.one_in(2) { 0.0 } else if edge < 0x40 { -1.0 } else { 1.0 };
                    ee.set_f32(b + bone::B_POSITION + (edge & 8), x);
                }
                _ => {}
            }
            ee.set_u32(p + bone::SWEEP, u32::from(!g.one_in(4)));
            if g.one_in(6) {
                ee.set_u32(p + bone::PROBE, 0);
            }
            if g.one_in(3) {
                ee.set_u32(p + bone::POOL, 0);
            }
            ee.set_u32(B_HITS + bone::R_COUNT, ((g.bits() % 5) as i32 - 1) as u32);
            for i in 0..3 {
                put(&mut ee, B_HITS + bone::R_POINTS + 0x10 * i, [at[0] + g.f(-1.0, 1.0), at[1] + g.f(-1.0, 1.0), at[2] + g.f(-1.0, 1.0), 1.0]);
                put(&mut ee, B_HITS + bone::R_NORMALS + 0x10 * i, vec(&mut g, 1.0));
                let (c, _) = other(&mut ee, &mut g);
                ee.set_u32(B_HITS + bone::R_OTHERS + 4 * i, c);
            }
            ee.set_u32(B_SWEPT_HITS + bone::R_SWEPT_POINT, B_SWEPT_AT);
            ee.set_u32(B_SWEPT_HITS + bone::R_SWEPT_NORMAL, B_SWEPT_AT + 0x10);
            put(&mut ee, B_SWEPT_AT, [at[0] + g.f(-1.0, 1.0), at[1] + g.f(-1.0, 1.0), at[2] + g.f(-1.0, 1.0), 1.0]);
            put(&mut ee, B_SWEPT_AT + 0x10, vec(&mut g, 1.0));
            let mut a = asked.borrow_mut();
            a.hit.clear();
            a.hit.insert(ee.u32_at(p + bone::SPHERE), (g.one_in(3), B_HITS));
            if ee.u32_at(p + bone::PROBE) != 0 {
                a.hit.insert(ee.u32_at(p + bone::PROBE), (g.one_in(2), B_SWEPT_HITS));
            }
        }
        let given = snap_regions(&ee, &regions);

        asked.borrow_mut().log.clear();
        ee.set(28, GP);
        ee.written.clear();
        if solve {
            ee.call(BONE_SOLVE, &[u64::from(p)], &[dt], BUDGET);
        } else {
            ee.call(BONE_PUSH_OUT, &[u64::from(p), u64::from(p + bone::POINT), u64::from(p + bone::NORMAL)], &[dt], BUDGET);
        }
        let stack = ee.stack;
        let terrain = ee.u32_at(p + bone::TERRAIN);
        let cache = |a: u32| (GROUND_RING..GROUND_RING + 0x70 * 10).contains(&a) || (terrain + 0x9e0..terrain + 0x9e8).contains(&a);
        let outside: Vec<u32> = ee.written.iter().copied().filter(|&a| !(stack - 0x4_0000..stack + 0x1000).contains(&a) && !cache(a) && !regions.iter().any(|&(at, n)| (at..at + n).contains(&a))).collect();
        let mut quads: Vec<u32> = outside.iter().map(|a| a & !0xf).collect();
        quads.dedup();
        assert!(outside.is_empty(), "case {k}: the original wrote outside the regions: {quads:#x?} (log {:?})", asked.borrow().log);
        let want = snap_regions(&ee, &regions);
        let want_log = std::mem::take(&mut asked.borrow_mut().log);

        restore_regions(&mut ee, &regions, &given);
        ee.written.clear();
        let mut scene = FakeScene(asked.clone());
        let mut m = RiderRam { ee: &mut ee, overlay: None };
        if solve {
            bone::solve(&mut m, &mut scene, p, F(dt), Some(&map), &curve);
        } else {
            bone::push_out(&mut m, &mut scene, p, p + bone::POINT, p + bone::NORMAL, &curve);
        }
        let outside: Vec<u32> = ee.written.iter().copied().filter(|&a| !regions.iter().any(|&(at, n)| (at..at + n).contains(&a))).collect();
        assert!(outside.is_empty(), "case {k}: the port wrote outside the regions: {:#x?}", &outside[..outside.len().min(8)]);
        let got = snap_regions(&ee, &regions);
        let got_log = std::mem::take(&mut asked.borrow_mut().log);
        assert_eq!(got_log, want_log, "case {k} ({}): the scene's calls", if solve { "solve" } else { "push-out" });
        for ((w, gw), at) in want.iter().zip(&got).zip(&addrs) {
            assert_eq!(gw, w, "case {k} ({}, part {p:#x}): the word at {at:#x}", if solve { "solve" } else { "push-out" });
        }
        let flag = |o: u32| ee.u32_at(p + o) != 0;
        if solve {
            let on = given[addrs.iter().position(|&a| a == p + bone::ON).unwrap()] != 0;
            let has_body = ee.u32_at(p + bone::BODY) != 0 && ee.u32_at(ee.u32_at(p + bone::BODY)) != 0;
            if !on || !has_body {
                tally(&mut t, if on { "solve: no body or disabled" } else { "solve: off" });
                continue;
            }
            tally(&mut t, if flag(bone::GROUND) { "solve: ground pushed" } else { "solve: no ground push" });
            let n = ee.u32_at(B_HITS + bone::R_COUNT) as i32;
            tally(&mut t, match (flag(bone::SCENE), flag(bone::SWEPT), n) {
                (true, true, _) => "solve: swept hit",
                (true, false, n) if n <= 0 => "solve: scene hit, no contacts",
                (true, false, _) => "solve: scene contacts",
                (false, _, _) => "solve: no scene, no sweep hit",
            });
            if want_log.iter().any(|l| l.starts_with("segment")) {
                tally(&mut t, "solve: swept");
            }
        } else {
            tally(&mut t, if turned == 0 || flag(bone::SWEPT) { "push-out: straight" } else { "push-out: impulse" });
            if parented {
                tally(&mut t, "push-out: node under a parent");
            }
        }
    }
    eprintln!("{t:#?}");
    for (path, n) in &t {
        assert!(*n >= 20, "path {path} taken only {n} times");
    }
    assert!(t.len() >= 16, "every path taken: {t:#?}");
}

/// What [`bone_replay`] saw: every hand-over's parts' four flags (touched, ground, scene, swept, in the
/// rider's list order), and with the port's solve, its calls and what they did.
#[derive(Debug, Default, PartialEq)]
struct BoneRun {
    flags: Vec<Vec<[u32; 4]>>,
    solves: usize,
    ground: usize,
    scene: usize,
    swept: usize,
}

/// [`replay`] with every hand-over's parts' flags kept, and with `port`, every bone collision solve
/// (the rider ragdoll world's part vtable's) [`bone::solve`]'s on the map, the scene's calls the
/// original's ([`OriginalScene`]). Every row is still checked bit-exact.
fn bone_replay(name: &str, port: bool) -> Option<BoneRun> {
    let mut ee = hooked(image()?);
    let rows = rows(name)?;
    let trn: &'static [u8] = Box::leak(nat05_trn()?.into_boxed_slice());
    let map: &'static track::Terrain = Box::leak(Box::new(track::Terrain::parse(trn).unwrap()));
    let curve: &'static CurveTable = Box::leak(Box::new(CurveTable::build()));
    let t: Rc<RefCell<BoneRun>> = Rc::default();
    let h = t.clone();
    ee.hooks.insert(
        HAND_OVER,
        Box::new(move |ee: &mut Ee| {
            let flags = rider_parts(ee).into_iter().map(|p| [0, 4, 8, 12].map(|k| ee.u32_at(p + bone::TOUCHED + k))).collect();
            h.borrow_mut().flags.push(flags);
            run_as_itself(ee, HAND_OVER);
        }),
    );
    if port {
        let h = t.clone();
        ee.hooks.insert(
            BONE_SOLVE,
            Box::new(move |ee: &mut Ee| {
                let (part, dt) = (ee.r(4) as u32, F(ee.f(12)));
                bone::solve(&mut RiderRam { ee, overlay: None }, &mut OriginalScene, part, dt, Some(map), curve);
                ee.pc = BONE_SOLVE;
                let mut t = h.borrow_mut();
                t.solves += 1;
                t.ground += usize::from(ee.u32_at(part + bone::GROUND) != 0);
                t.scene += usize::from(ee.u32_at(part + bone::SCENE) != 0);
                t.swept += usize::from(ee.u32_at(part + bone::SWEPT) != 0);
            }),
        );
    }
    for (r, row) in rows.iter().enumerate() {
        poke(&mut ee, row, &READER);
        ee.set(28, GP);
        ee.call(FRAME, &[u64::from(SIM)], &[f32::from_bits(DT)], 400_000_000);
        let d = diff(&ee, row);
        assert!(d.is_empty(), "{name} row {r} (vsync {}, port {port}): {} words differ\n  {}", row["vsync"], d.len(), d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  "));
        poke(&mut ee, row, &SPOT);
    }
    for a in [HAND_OVER, BONE_SOLVE] {
        ee.hooks.remove(&a);
    }
    Some(Rc::try_unwrap(t).ok().unwrap().into_inner())
}

/// **The rider's bone collisions in the port's hands, both C15 replays bit-exact** (S278a): every
/// solve of the rider ragdoll world's parts is [`bone::solve`]'s (its ground, push-outs and impulses
/// the port's, the scene's search the original's), every row of ride-2 and flip-1 comes out word for
/// word, and every part's touched, ground, scene and swept words at every frame's hand-over are the
/// original-only run's. On C15 every push-out is against the ground: no scene or swept hit, no other
/// side. The ride's parts first touch on row 216 (26 rows of 459, 12 393 solves), the flip's on row
/// 127, the row before its `SETTLES` (76 rows of 465, 289 touches, 1 118 ground arms of 12 303 solves).
#[test]
fn the_bone_collisions_touch_as_the_originals() {
    for (name, n) in [("ride-2", 459usize), ("flip-1", 465)] {
        let Some(want) = bone_replay(name, false) else { return };
        let Some(got) = bone_replay(name, true) else { return };
        eprintln!("{name}: {} solves, {} ground, {} scene, {} swept", got.solves, got.ground, got.scene, got.swept);
        assert_eq!(want.flags.len(), n, "{name}: one hand-over a frame");
        assert_eq!(want.solves, 0);
        assert!(got.solves >= 9 * n, "{name}: every part solved every frame");
        for (r, (w, g)) in want.flags.iter().zip(&got.flags).enumerate() {
            assert_eq!(g, w, "{name} row {r}: the parts' flags at the hand-over");
        }
        assert_eq!(got.flags.len(), n);
        let touched = |b: &BoneRun| b.flags.iter().flatten().filter(|f| f[0] != 0).count();
        assert_eq!(touched(&got), touched(&want));
        if name == "flip-1" {
            assert!(got.ground > 0 && touched(&got) > 0, "{name}: the ragdoll touches the ground");
        }
        assert_eq!((got.scene, got.swept), (0, 0), "{name}: no scene or swept hit on C15");
        let first = got.flags.iter().position(|r| r.iter().any(|f| f[0] != 0));
        assert_eq!(first, Some(if name == "flip-1" { 127 } else { 216 }), "{name}: the first row a part touches");
    }
}

use game_sim::quad::{Anim, Lag, NetSmooth, Remote, Snapshot};

/// The bike's net interpolate (vt `+0x3ec`): the quad's own, which ends in the biplane's.
const NET_INTERPOLATE_FN: u32 = 0x004e_7bc0;

/// Where the net interpolate reads and writes, off the bike's pointers (the quad's map,
/// tests/atv_quad/interpolate.rs): the snapshot's nine places (`*(+0xbcc)`), the eight lags (position
/// x, y, z `*(+0xd14)`; velocity x, y, z `*(+0xd18)`; crossup `+0x1790`; fore and aft `+0x178c`) and the
/// world's area (`*(+0x3b0) +0x3b4`, its `+0x550`).
#[derive(Clone, Copy, Debug)]
struct NetPlaces {
    snap: [u32; 9],
    lags: [u32; 8],
    area: u32,
}

impl NetPlaces {
    fn of(ee: &Ee, bike: u32) -> NetPlaces {
        let table = ee.u32_at(bike + 0xbcc);
        let (pos, vel) = (ee.u32_at(bike + 0xd14), ee.u32_at(bike + 0xd18));
        let mut lags = [0; 8];
        for k in 0..3 {
            lags[k] = ee.u32_at(pos + 4 * k as u32);
            lags[3 + k] = ee.u32_at(vel + 4 * k as u32);
        }
        lags[6] = ee.u32_at(bike + 0x1790);
        lags[7] = ee.u32_at(bike + 0x178c);
        NetPlaces { snap: std::array::from_fn(|k| ee.u32_at(table + 4 * k as u32)), lags, area: ee.u32_at(ee.u32_at(bike + 0x3b0) + 0x3b4) + 0x550 }
    }
}

fn net_snapshot(ee: &Ee, a: u32) -> Snapshot {
    Snapshot {
        time: F(ee.f32_at(a)),
        position: vf_at(ee, a + 0x10),
        velocity: vf_at(ee, a + 0x20),
        orientation: vf_at(ee, a + 0x30),
        crossup_blend: F(ee.f32_at(a + 0x40)),
        fore_aft_blend: F(ee.f32_at(a + 0x44)),
        wreck: ee.u32_at(a + 0x48),
        stunt: ee.u32_at(a + 0x4c) as i32,
        word: ee.u32_at(a + 0x50),
    }
}

/// What the net interpolate keeps, read off the bike.
struct NetWords {
    s: NetSmooth,
    r: Remote,
    a: Anim,
    q: Quad,
}

fn net_words(ee: &Ee, bike: u32, p: &NetPlaces) -> NetWords {
    let lag = |k: usize| {
        let a = p.lags[k];
        Lag { value: F(ee.f32_at(a)), time: F(ee.f32_at(a + 4)), fraction: F(ee.f32_at(a + 8)) }
    };
    let s = NetSmooth {
        position: [lag(0), lag(1), lag(2)],
        velocity: [lag(3), lag(4), lag(5)],
        crossup: lag(6),
        fore_aft: lag(7),
        put_back_at: vf_at(ee, bike + 0xd40),
        velocity_change: vf_at(ee, p.snap[3]),
        wreck: ee.u32_at(p.snap[6]),
        word: ee.u32_at(p.snap[8]),
    };
    let r = Remote { position: vf_at(ee, p.snap[0]), orientation: vf_at(ee, p.snap[1]), velocity: vf_at(ee, p.snap[2]), ..Remote::default() };
    let a = Anim {
        crossup_blend: F(ee.f32_at(p.snap[4])),
        fore_aft_blend: F(ee.f32_at(p.snap[5])),
        next_stunt: ee.u32_at(p.snap[7]) as i32,
        ..Anim::default()
    };
    NetWords { s, r, a, q: Quad { speed_len: F(ee.f32_at(bike + 0x32c)), ..Quad::default() } }
}

/// Every word the net interpolate may write, put back on the bike.
fn put_net_words(ee: &mut Ee, bike: u32, p: &NetPlaces, w: &NetWords) {
    let lags = [w.s.position[0], w.s.position[1], w.s.position[2], w.s.velocity[0], w.s.velocity[1], w.s.velocity[2], w.s.crossup, w.s.fore_aft];
    for (k, l) in lags.iter().enumerate() {
        let a = p.lags[k];
        for (o, x) in [(0, l.value), (4, l.time), (8, l.fraction)] {
            ee.set_u32(a + o, x.to_bits());
        }
    }
    ee.set_u32(p.snap[4], w.a.crossup_blend.to_bits());
    ee.set_u32(p.snap[5], w.a.fore_aft_blend.to_bits());
    ee.set_u32(p.snap[7], w.a.next_stunt as u32);
    ee.set_u32(p.snap[8], w.s.word);
    ee.set_u32(p.snap[6], w.s.wreck);
    set_vf(ee, bike + 0xd40, w.s.put_back_at);
    set_vf(ee, p.snap[0], w.r.position);
    set_vf(ee, p.snap[3], w.s.velocity_change);
    set_vf(ee, p.snap[2], w.r.velocity);
    ee.set_u32(bike + 0x32c, w.q.speed_len.to_bits());
    set_vf(ee, p.snap[1], w.r.orientation);
}

/// **F45a — the bike's net interpolate (vt `+0x3ec`) on its RAM**, through
/// [`quad::net_interpolate_head`] and [`quad::net_interpolate_tail`]: the head's words are written
/// first; when it asks for a put-back, the bike's own (vt `+0x3a8`, the bike, 0, 0; `dt` in `$f12`)
/// runs in the interpreter on them, and the tail starts from the words the put-back left.
fn net_interpolate_in_ram(ee: &mut Ee, older: u32, newer: u32, t: F, dt: F) {
    let p = NetPlaces::of(ee, BIKE);
    let (o, n) = (net_snapshot(ee, older), net_snapshot(ee, newer));
    let area = [0, 4, 8, 12].map(|k| F(ee.f32_at(p.area + k)));
    let mut w = net_words(ee, BIKE, &p);
    let l = quad::net_interpolate_head(&o, &n, t, dt, area, &mut w.s, &w.r, &mut w.a, acos());
    let (p, mut w) = if l.put_back {
        put_net_words(ee, BIKE, &p, &w);
        let f = vt(ee, PUT_BACK_SLOT);
        ee.set(28, GP);
        census(ee, |ee| ee.call(f, &[u64::from(BIKE), 0, 0], &[dt.0], BUDGET));
        let p = NetPlaces::of(ee, BIKE);
        let w = net_words(ee, BIKE, &p);
        (p, w)
    } else {
        (p, w)
    };
    quad::net_interpolate_tail(&l, dt, &mut w.s, &mut w.r, &mut w.q);
    put_net_words(ee, BIKE, &p, &w);
}

/// The generated cases' two snapshots.
const NET_OLDER: u32 = WRECK_AREA + 0x400;
const NET_NEWER: u32 = WRECK_AREA + 0x480;

#[allow(clippy::float_arithmetic)]
impl Gen {
    fn unit_quaternion(&mut self) -> Vf {
        let v: [f32; 4] = std::array::from_fn(|_| self.f(-1.0, 1.0));
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2] + v[3] * v[3]).sqrt().max(1e-3);
        Vf::new(v[0] / n, v[1] / n, v[2] / n, v[3] / n)
    }

    fn net_snapshot(&mut self, position: Vf, orientation: Vf) -> Snapshot {
        let mut velocity = Vf::new(self.f(-60.0, 60.0), self.f(-60.0, 60.0), self.f(-60.0, 60.0), self.f(-1.0, 1.0));
        if self.one_in(30) {
            velocity.0[(self.bits() % 3) as usize] = f32::MAX;
        }
        Snapshot {
            time: F(self.f(0.0, 500.0)),
            position,
            velocity,
            orientation,
            crossup_blend: F(self.f(-0.5, 1.5)),
            fore_aft_blend: F(self.near(0.5, -0.5, 1.5)),
            wreck: if self.one_in(2) { 0 } else { self.bits() % 16 },
            stunt: if self.one_in(2) { 46 } else { self.bits() as i32 },
            word: self.bits(),
        }
    }

    fn net_t(&mut self) -> F {
        F(match self.bits() % 9 {
            0 => 0.0,
            1 => 1.0,
            2 => f32::from_bits(0xbf66_6666),
            3 => f32::from_bits(0x3ff3_3333),
            4 => self.f(-0.9, 0.0),
            5 => self.f(1.0, 1.9),
            _ => self.f(0.0, 1.0),
        })
    }
}

fn put_net_snapshot(ee: &mut Ee, a: u32, s: &Snapshot) {
    ee.set_u32(a, s.time.to_bits());
    set_vf(ee, a + 0x10, s.position);
    set_vf(ee, a + 0x20, s.velocity);
    set_vf(ee, a + 0x30, s.orientation);
    ee.set_u32(a + 0x40, s.crossup_blend.to_bits());
    ee.set_u32(a + 0x44, s.fore_aft_blend.to_bits());
    ee.set_u32(a + 0x48, s.wreck);
    ee.set_u32(a + 0x4c, s.stunt as u32);
    ee.set_u32(a + 0x50, s.word);
}

/// RAM's bytes at `at` put back to `base`'s.
fn reset_bytes(ee: &mut Ee, base: &[u8], at: impl IntoIterator<Item = u32>) {
    for a in at {
        ee.ram[a as usize] = base[a as usize];
    }
}

/// **F45a — the bike's net interpolate is the original's, put-back and all**: 10 000 snapshot pairs
/// on state 4's bike (its own pointers, lags and world area), each run through SLUS-21104's vt
/// `+0x3ec` and through [`net_interpolate_in_ram`] from the same RAM. The pairs sit about where the
/// snapshot last put the bike, a little way off, or far (the put-back, which runs the bike's whole
/// vt `+0x3a8` in the interpreter on both sides), now and then past the world's edge, with `FLT_MAX`
/// lanes, and `t` at and between its clamps; every byte either side writes outside the stack comes
/// out the same.
#[test]
#[allow(clippy::float_arithmetic)]
fn the_bikes_net_interpolate_is_the_originals() {
    let Some(mut ee) = image().map(hooked) else { return };
    assert_eq!(vt(&ee, 0x3ec), NET_INTERPOLATE_FN, "the bike's vt +0x3ec is the quad's net interpolate");
    let p = NetPlaces::of(&ee, BIKE);
    eprintln!("the bike's net places: snapshot {:x?}, lags {:x?}, area at {:#x}", p.snap, p.lags, p.area);
    let area = [0, 4, 8, 12].map(|k| ee.f32_at(p.area + k));
    let home = vf_at(&ee, p.snap[0]);
    let base = ee.ram.clone();
    let mut dirty = std::collections::BTreeSet::new();
    let mut g = Gen(0x0000_f45a);
    let mut t = BTreeMap::new();
    for k in 0..10_000 {
        reset_bytes(&mut ee, &base, std::mem::take(&mut dirty));
        ee.written.clear();
        // Where the snapshot last put the bike: the image's, or anywhere in the world.
        let last = if g.one_in(4) { Vf::new(g.f(area[0], area[1]), g.f(-100.0, 300.0), g.f(area[2], area[3]), 1.0) } else { home };
        set_vf(&mut ee, p.snap[0], last);
        let (reach, path) = match g.bits() % 3 {
            0 => (3.0, "near"),
            1 => (12.0, "lags set"),
            _ => (100.0, "put back"),
        };
        let pos = Vf::new(last.x() + g.f(-reach, reach), last.y() + g.f(-reach, reach), last.z() + g.f(-reach, reach), g.f(-1.0, 1.0));
        let pos = if g.one_in(10) { Vf::new(area[0] + g.f(-50.0, 50.0), pos.y(), area[3] + g.f(-50.0, 50.0), pos.w()) } else { pos };
        let next = Vf::new(pos.x() + g.f(-5.0, 5.0), pos.y() + g.f(-5.0, 5.0), pos.z() + g.f(-5.0, 5.0), g.f(-1.0, 1.0));
        let o0 = if g.one_in(3) { vf_at(&ee, p.snap[1]) } else { g.unit_quaternion() };
        let o1 = match g.bits() % 4 {
            0 => o0,
            1 => Vf::new(-o0.x(), -o0.y(), -o0.z(), -o0.w()),
            _ => g.unit_quaternion(),
        };
        let (older, newer) = (g.net_snapshot(pos, o0), g.net_snapshot(next, o1));
        put_net_snapshot(&mut ee, NET_OLDER, &older);
        put_net_snapshot(&mut ee, NET_NEWER, &newer);
        for (j, &a) in p.lags.iter().enumerate() {
            let around = if j < 3 { 3000.0 } else if j < 6 { 60.0 } else { 1.5 };
            ee.set_u32(a, g.f(-around, around).to_bits());
            ee.set_u32(a + 4, if g.one_in(30) { 0 } else { g.near(0.05, 0.005, 0.5).to_bits() });
            ee.set_u32(a + 8, g.f(0.0, 1.0).to_bits());
        }
        let (tt, dt) = (g.net_t(), F(if g.one_in(4) { f32::from_bits(DT) } else { g.f(0.0, 0.2) }));
        let off = both_ways(
            &mut ee,
            &base,
            &mut dirty,
            |_| false,
            |ee| ee.call(NET_INTERPOLATE_FN, &[u64::from(BIKE), u64::from(NET_OLDER), u64::from(NET_NEWER)], &[dt.0, tt.0], BUDGET),
            |ee| net_interpolate_in_ram(ee, NET_OLDER, NET_NEWER, tt, dt),
        );
        assert!(off.is_empty(), "case {k} ({path}, t {tt:?}, dt {dt:?}): {} bytes differ\n  {}", off.len(), off.iter().take(24).cloned().collect::<Vec<_>>().join("\n  "));
        let far = {
            let l = quad::net_interpolate_head(&older, &newer, tt, dt, area.map(F), &mut net_words(&ee, BIKE, &p).s, &Remote { position: last, ..Remote::default() }, &mut Anim::default(), acos());
            l.put_back
        };
        tally(&mut t, if far { "put back" } else { path });
        tally(&mut t, if tt.0 <= 0.0 { "t at or before" } else if tt.0 < 1.0 { "t between" } else { "t at or past" });
    }
    reset_bytes(&mut ee, &base, dirty);
    eprintln!("{t:?}");
    assert!(t.values().all(|&n| n >= 500), "{t:?}");
}

/// The original (`original`) and the port (`port`) run from the same RAM: what the case laid since
/// `ee.written` was last cleared, on `base`. Answers each byte outside the stack (and outside what
/// `skip` names) the two leave differently; `dirty` gains every byte either wrote, for the next case
/// to put back.
fn both_ways(
    ee: &mut Ee,
    base: &[u8],
    dirty: &mut std::collections::BTreeSet<u32>,
    skip: impl Fn(u32) -> bool,
    original: impl FnOnce(&mut Ee),
    port: impl FnOnce(&mut Ee),
) -> Vec<String> {
    let stack = ee.stack;
    let in_stack = |a: &u32| (stack - 0x1_0000..stack + 0x4000).contains(a) || skip(*a);
    let laid: Vec<(u32, u8)> = ee.written.iter().map(|&a| (a, ee.u8_at(a))).collect();
    ee.written.clear();
    ee.set(28, GP);
    original(ee);
    let by_original = std::mem::take(&mut ee.written);
    let want: Vec<(u32, u8)> = by_original.iter().map(|&a| (a, ee.u8_at(a))).collect();
    reset_bytes(ee, base, by_original.iter().copied());
    for &(a, b) in &laid {
        ee.ram[a as usize] = b;
    }
    ee.written.clear();
    port(ee);
    let by_port = std::mem::take(&mut ee.written);
    let mut off = Vec::new();
    for &(a, b) in want.iter().filter(|(a, _)| !in_stack(a)) {
        if ee.u8_at(a) != b {
            off.push(format!("{a:#010x}: original {b:#04x}, port {:#04x}", ee.u8_at(a)));
        }
    }
    for &a in by_port.iter().filter(|a| !in_stack(a) && !by_original.contains(a)) {
        if ee.u8_at(a) != base_or_laid(base, &laid, a) {
            off.push(format!("{a:#010x}: only the port wrote it ({:#04x})", ee.u8_at(a)));
        }
    }
    dirty.extend(by_original);
    dirty.extend(by_port);
    dirty.extend(laid.iter().map(|&(a, _)| a));
    off
}

fn base_or_laid(base: &[u8], laid: &[(u32, u8)], a: u32) -> u8 {
    laid.iter().find(|&&(x, _)| x == a).map_or(base[a as usize], |&(_, b)| b)
}

/// **F45a — the bike's whole net check is the original's, its interpolate served**: 3 000 cases on
/// state 4's bike with six snapshots in its slots (`+0xcd4`, `+0xcdc`, … 8 apart, newest first, their
/// times round the lagged network clock so the walk stops at each step or runs out), run through
/// SLUS-21104's vt `+0x3e8` and through [`update::net_check`] on [`UpdateInRam`], which serves the vt
/// `+0x3ec` it hands on with [`net_interpolate_in_ram`] (F45a; the put-back the bike's own in both).
/// The answer and every byte outside the stack come out the same. Now and then the bike has fewer
/// than five snapshots come (`+0xd04`), and nothing is interpolated.
#[test]
#[allow(clippy::float_arithmetic)]
fn the_bikes_net_check_is_the_originals_with_the_interpolate_in_rust() {
    let Some(mut ee) = image().map(hooked) else { return };
    assert_eq!((vt(&ee, 0x3e8), vt(&ee, 0x3ec)), (NET_CHECK_FN, NET_INTERPOLATE_FN), "the bike's vt +0x3e8 and +0x3ec");
    let p = NetPlaces::of(&ee, BIKE);
    let home = vf_at(&ee, p.snap[0]);
    let want_at = F(ee.f32_at(ee.u32_at(update::NET_CLOCK) + 0x4cc)) - F(ee.f32_at(BIKE + 0xd10)) - F(ee.f32_at(update::NET_LAG));
    eprintln!("the lagged clock {want_at:?}");
    let base = ee.ram.clone();
    let mut dirty = std::collections::BTreeSet::new();
    let mut g = Gen(0x0000_f45b);
    let mut t = BTreeMap::new();
    let snaps: [u32; 6] = std::array::from_fn(|k| WRECK_AREA + 0x400 + 0x80 * k as u32);
    for k in 0..3_000 {
        reset_bytes(&mut ee, &base, std::mem::take(&mut dirty));
        ee.written.clear();
        let come = if g.one_in(10) { g.bits() % 5 } else { 5 + g.bits() % 40 };
        ee.set_u32(BIKE + 0xd04, come);
        // The walk stops after `stop` steps: the newest `stop + 1` snapshots after the lagged clock.
        let stop = g.bits() % 6;
        let step = g.f(0.02, 0.2);
        let mut at = Vf::new(home.x() + g.f(-30.0, 30.0), home.y() + g.f(-30.0, 30.0), home.z() + g.f(-30.0, 30.0), 1.0);
        let mut o = if g.one_in(2) { vf_at(&ee, p.snap[1]) } else { g.unit_quaternion() };
        for (j, &a) in snaps.iter().enumerate() {
            let mut s = g.net_snapshot(at, o);
            let ahead = stop as f32 - j as f32;
            s.time = F(want_at.0 + step * ahead - if ahead == 0.0 && g.one_in(3) { 0.0 } else { g.f(0.0, step) });
            if g.one_in(20) {
                s.time = F(want_at.0);
            }
            put_net_snapshot(&mut ee, a, &s);
            ee.set_u32(BIKE + 0xcd4 + 8 * j as u32, a);
            at = Vf::new(at.x() - g.f(-3.0, 3.0), at.y() - g.f(-3.0, 3.0), at.z() - g.f(-3.0, 3.0), 1.0);
            o = if g.one_in(3) { g.unit_quaternion() } else { o };
        }
        for (j, &a) in p.lags.iter().enumerate() {
            let around = if j < 3 { 3000.0 } else if j < 6 { 60.0 } else { 1.5 };
            ee.set_u32(a, g.f(-around, around).to_bits());
            ee.set_u32(a + 4, if g.one_in(30) { 0 } else { g.near(0.05, 0.005, 0.5).to_bits() });
            ee.set_u32(a + 8, g.f(0.0, 1.0).to_bits());
        }
        let dt = if g.one_in(4) { f32::from_bits(DT) } else { g.f(0.0, 0.2) };
        let (mut back, mut got) = (0, 0);
        F45_SERVED.with(|s| s.borrow_mut().clear());
        let off = both_ways(
            &mut ee,
            &base,
            &mut dirty,
            |_| false,
            |ee| {
                ee.call(NET_CHECK_FN, &[u64::from(BIKE)], &[dt], BUDGET);
                back = ee.v0() as u32;
            },
            |ee| got = update::net_check(&mut UpdateInRam { ee, x: dt }),
        );
        assert!(off.is_empty(), "case {k} ({come} come, stop {stop}): {} bytes differ\n  {}", off.len(), off.iter().take(24).cloned().collect::<Vec<_>>().join("\n  "));
        assert_eq!(got, back, "case {k}: the answer");
        let served = F45_SERVED.with(|s| s.borrow().get("NetInterpolate").copied().unwrap_or(0));
        assert_eq!(served, u32::from(come >= 5), "case {k}: the interpolate served");
        tally(&mut t, if come < 5 { "under five" } else { ["stop 0", "stop 1", "stop 2", "stop 3", "stop 4", "stop 5"][stop as usize] });
    }
    reset_bytes(&mut ee, &base, dirty);
    eprintln!("{t:?}");
    assert!(t.values().all(|&n| n >= 150), "{t:?}");
}

thread_local! {
    /// F45a: Nat05's ground, which the state 4 bike stands on, for the net unpack's wheel steps
    /// ([`unpack_wheel_in_ram`]); `None` without the disc's file.
    static UNPACK_MAP: std::cell::OnceCell<Option<&'static track::Terrain<'static>>> = const { std::cell::OnceCell::new() };
}

fn unpack_map() -> Option<&'static track::Terrain<'static>> {
    UNPACK_MAP.with(|m| {
        *m.get_or_init(|| {
            let trn: &'static [u8] = Box::leak(nat05_trn()?.into_boxed_slice());
            Some(&*Box::leak(Box::new(track::Terrain::parse(trn).unwrap())))
        })
    })
}

/// **F45a — one of the net unpack's wheel calls** (a wheel's clear, suspension or tire forces: `c`
/// on the wheel at `wheel`) on the bike in RAM, through [`quad::Steps`] on the ground of `map` as
/// [`StepsInRam`] serves the Force body's.
fn unpack_wheel_in_ram(ee: &mut Ee, c: update::Out, dt: F, map: &track::Terrain<'_>) {
    use update::Out;
    let body = ee.u32_at(BIKE + seed::BODY);
    let (wheels, record) = (the_wheels(ee), ee.u32_at(BIKE + seed::RECORD));
    let i = |w: u32| match wheels.iter().position(|&x| x == w) {
        Some(0) => bike::FRONT,
        Some(_) => bike::REAR,
        None => panic!("{w:#x} is none of the bike's wheels {wheels:#x?}"),
    };
    let c = match c {
        Out::WheelClear(w) => Call::WheelClear(i(w)),
        Out::Suspension(w) => Call::Suspension(i(w), dt),
        Out::TireForces(w) => Call::TireForces(i(w), dt),
        c => panic!("the unpack's wheel calls are no {c:?}"),
    };
    let ram = InRam { ee, wheels, record, body, last: Vec::new() };
    let inner = StepInRam { ram, rider: bike::Rider::default(), rider_last: Vec::new(), dt };
    let (mut noise, mut answered) = (None, BTreeMap::new());
    let mut m = StepsInRam { inner, map, noise: &mut noise, answered: &mut answered, pure: true };
    let (mut q, mut b) = m.inner.read();
    m.call(c, &mut q, &mut b);
    m.inner.write(&q, &b);
    assert!(answered.keys().all(|k| !k.starts_with('>') && !k.starts_with('=')), "{c:?} handed on {answered:?}");
}

/// **F45a — the bike's whole net unpack is the original's, its wheel steps in Rust**: 2 000 cases on
/// state 4's bike on Nat05, the snapshot's position (about the image's, a foot or two up or down, so
/// the wheels are down, half down or up), quaternion (the image's or tipped) and velocity drawn, the
/// steer filter's time and `dt` too, run through SLUS-21104's vt `+0x3f0` and through
/// [`update::net_unpack`] on [`UpdateInRam`]: the node turn [`node::orient`]'s, the wheel clears,
/// suspensions and tire forces [`unpack_wheel_in_ram`]'s (F45a); the contacts and the net post
/// still the original's. Every byte outside the stack comes out the same.
#[test]
#[allow(clippy::float_arithmetic)]
fn the_bikes_net_unpack_is_the_originals_with_the_wheels_in_rust() {
    let Some(mut ee) = image().map(hooked) else { return };
    if unpack_map().is_none() {
        return;
    }
    assert_eq!(vt(&ee, 0x3f0), NET_UNPACK_FN, "the bike's vt +0x3f0");
    let p = NetPlaces::of(&ee, BIKE);
    let (home, turned, filter) = (vf_at(&ee, p.snap[0]), vf_at(&ee, p.snap[1]), ee.u32_at(BIKE + 0xd1c));
    let wheels = the_wheels(&ee);
    // The original's ground query keeps a cache of the cells it read; the port reads the map.
    let terrain = ee.u32_at(wheels[0] + 0x1e0);
    assert_ne!(terrain, 0, "the wheels' terrain");
    let cache = |a: u32| (GROUND_RING..GROUND_RING + 0x70 * 10).contains(&a) || (terrain + 0x9e0..terrain + 0x9e8).contains(&a);
    let base = ee.ram.clone();
    let mut dirty = std::collections::BTreeSet::new();
    let mut g = Gen(0x0000_f45c);
    let (mut t, mut posts) = (BTreeMap::new(), BTreeMap::new());
    for k in 0..2_000 {
        reset_bytes(&mut ee, &base, std::mem::take(&mut dirty));
        ee.written.clear();
        let up = g.f(-1.0, 2.5);
        set_vf(&mut ee, p.snap[0], Vf::new(home.x() + g.f(-3.0, 3.0), home.y() + up, home.z() + g.f(-3.0, 3.0), home.w()));
        let q = if g.one_in(3) {
            turned
        } else {
            let v: [f32; 4] = std::array::from_fn(|j| turned.0[j] + g.f(-0.15, 0.15));
            let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2] + v[3] * v[3]).sqrt();
            Vf::new(v[0] / n, v[1] / n, v[2] / n, v[3] / n)
        };
        set_vf(&mut ee, p.snap[1], q);
        set_vf(&mut ee, p.snap[2], Vf::new(g.f(-30.0, 30.0), g.f(-10.0, 10.0), g.f(-30.0, 30.0), g.f(-1.0, 1.0)));
        ee.set_u32(filter, g.f(-0.5, 0.5).to_bits());
        ee.set_u32(filter + 4, if g.one_in(20) { 0 } else { g.near(0.1, 0.01, 0.5).to_bits() });
        let dt = if g.one_in(4) { f32::from_bits(DT) } else { g.f(0.001, 0.05) };
        F45_SERVED.with(|s| s.borrow_mut().clear());
        let off = both_ways(
            &mut ee,
            &base,
            &mut dirty,
            cache,
            |ee| ee.call(NET_UNPACK_FN, &[u64::from(BIKE)], &[dt], BUDGET),
            |ee| update::net_unpack(&mut UpdateInRam { ee, x: dt }, F(dt)),
        );
        assert!(off.is_empty(), "case {k} (up {up}, dt {dt}): {} bytes differ\n  {}", off.len(), off.iter().take(24).cloned().collect::<Vec<_>>().join("\n  "));
        let served = F45_SERVED.with(|s| std::mem::take(&mut *s.borrow_mut()));
        for c in ["WheelClear", "Suspension", "TireForces"] {
            assert_eq!(served.get(c), Some(&2), "case {k}: {c} served twice ({served:?})");
        }
        assert_eq!(served.get("NetPost"), Some(&1), "case {k}: the net post served ({served:?})");
        if k == 0 {
            eprintln!("served: {served:?}");
        }
        for (c, n) in served.iter().filter(|(c, _)| c.starts_with("post ")) {
            *posts.entry(*c).or_insert(0) += n;
        }
        let down = wheels.map(|w| ee.u32_at(w + 0xdc) != 0);
        tally(&mut t, match down {
            [true, true] => "both down",
            [false, false] => "both up",
            [true, false] => "front down",
            [false, true] => "rear down",
        });
    }
    reset_bytes(&mut ee, &base, dirty);
    eprintln!("{t:?}\nthe net post's calls: {posts:?}");
    assert!(t.values().all(|&n| n >= 100) && t.len() >= 3, "{t:?}");
}

/// **F45a — the net unpack's end** ([`bike::net_post`]) on the bike in RAM, its spatial update, node
/// bounds and collision sync [`PostsInRam`]'s as the PostProcess's are (what the sync hands on, the
/// collidables' database moves and the gather, the original's unless [`in_rust`] names them). Every
/// call it answered is counted in [`F45_SERVED`] as `post <name>`.
fn net_post_in_ram(ee: &mut Ee, dt: F) {
    let body = ee.u32_at(BIKE + seed::BODY);
    let (wheels, record) = (the_wheels(ee), ee.u32_at(BIKE + seed::RECORD));
    let ram = InRam { ee, wheels, record, body, last: Vec::new() };
    let step = StepInRam { ram, rider: bike::Rider::default(), rider_last: Vec::new(), dt };
    let inner = PostInRam { step, post: bike::Post::default(), post_last: Vec::new() };
    let mut answered = BTreeMap::new();
    let mut m = PostsInRam { inner, answered: &mut answered };
    let (mut q, mut b) = m.inner.read();
    bike::net_post(&mut q, &mut b, &mut m);
    m.inner.write(&q, &b);
    F45_SERVED.with(|s| {
        let mut s = s.borrow_mut();
        for (k, n) in answered {
            *s.entry(Box::leak(format!("post {k}").into_boxed_str())).or_default() += n;
        }
    });
}

// F31b5i: the rider leaf of the frame's tail on the ports: its mode machine (rider::modes, S278b and
// S285), its bone collision solves (bone::solve, S278a), its part clear and hand-over (S278), and the
// leaf's own wreck spots into the bike's record (rider::wreck_spots).

/// The rider's listener step's three calls: the rider's frame (the mode machine, then the rider's
/// world step and its vt `+0x148`), the copy after it, and the step run while the rider's `+0xd34` is
/// set. The frame's mode machine; the machine's outside calls (S278b's oracle's addresses).
const RIDER_FRAME: u32 = 0x004f_1790;
const RIDER_AFTER: u32 = 0x004f_2970;
const RIDER_EXTRA: u32 = 0x004f_2d50;
const RIDER_MODES: u32 = 0x004e_f320;
const RIDER_RESET: u32 = 0x004e_ef40;
const RIDER_WRECK_ENTRY: u32 = 0x004e_d700;
const RIDER_POSE: u32 = 0x0050_5720;
const RIDER_SETTLE_VT: u32 = 0x0050_5710;
const RIDER_BODY_TARGET: u32 = 0x0049_3b00;
/// The riding reset's (S287) and the wreck entry's (S288): the bodies' vt `+0x18`, `+0x60`, `+0x20`,
/// the heap's resize, the rider's `+0x8f4` object's cue.
const RIDER_BODY_MASS: u32 = 0x0049_3cc0;
const RIDER_BODY_INERTIA: u32 = 0x0049_3bf0;
const RIDER_BODY_AT: u32 = 0x0049_3b20;
const HEAP_RESIZE: u32 = 0x001b_7470;
const RIDER_WRECK_CUE: u32 = 0x004f_7ca0;

thread_local! {
    /// What [`rider_leaf`] served in Rust, by name, over a replay.
    static LEAF_SERVED: RefCell<BTreeMap<&'static str, u32>> = const { RefCell::new(BTreeMap::new()) };
}

fn leaf_served(what: &'static str) {
    LEAF_SERVED.with(|s| *s.borrow_mut().entry(what).or_default() += 1);
}

/// Nat05's map, for the rider's ground queries (the tail's walk has no map of its own).
fn nat05_map() -> &'static track::Terrain<'static> {
    thread_local! {
        static M: &'static track::Terrain<'static> = {
            let trn: &'static [u8] = Box::leak(nat05_trn().expect("Nat05's map").into_boxed_slice());
            Box::leak(Box::new(track::Terrain::parse(trn).unwrap()))
        };
    }
    M.with(|m| *m)
}

/// `f` called from inside a hook with `ints` and `dt`, the hooked call's `pc`, `sp`, `ra` and stack
/// put back after (the interpreter files the hook under the `pc` it leaves); answers `v0`.
fn call_from_hook_dt(ee: &mut Ee, f: u32, ints: &[u64], dt: F) -> u64 {
    let (pc, ra, sp, stack) = (ee.pc, ee.r(31), ee.r(29), ee.stack);
    ee.stack = (sp as u32 - 0x80) & !0xf;
    ee.set(28, GP);
    ee.call(f, ints, &[dt.0], BUDGET);
    let v = ee.r(2);
    (ee.stack, ee.pc) = (stack, pc);
    ee.set(29, sp);
    ee.set(31, ra);
    v
}

/// The mode machine's outside calls from inside its hook: the riding reset the port's
/// ([`rider::riding_reset`], S287), the wreck entry the port's ([`rider::wreck_entry`], S288), and
/// the bodies' virtuals ([`rider::body_target`], [`rider::body_mass`], [`rider::body_inertia`],
/// [`rider::body_at`]), every other the original's (S286-S289 port them).
struct LeafRider<'a> {
    ee: &'a mut Ee,
}

impl node::Ram for LeafRider<'_> {
    fn word(&mut self, at: u32) -> u32 {
        self.ee.u32_at(at)
    }
    fn set(&mut self, at: u32, w: u32) {
        self.ee.set_u32(at, w)
    }
}

impl rider::Rider for LeafRider<'_> {
    fn call(&mut self, c: rider::Call, dt: F) -> u32 {
        use rider::Call as C;
        let r = u64::from(RIDER);
        let vt = |ee: &Ee, holder: u32, slot: u32| ee.u32_at(ee.u32_at(holder) + slot);
        let v = match c {
            C::Rand => call_from_hook_dt(self.ee, RAND, &[], dt),
            C::RidingReset => {
                rider::riding_reset(self, RIDER, dt);
                0
            }
            C::WreckEntry(kind) => {
                rider::wreck_entry(self, RIDER, kind, dt);
                0
            }
            C::WreckCue(cue) => call_from_hook_dt(self.ee, RIDER_WRECK_CUE, &[u64::from(cue), 0], dt),
            C::Pose => {
                assert_eq!(vt(self.ee, RIDER + 4, 0xd0), RIDER_POSE, "the rider's vt +0xd0");
                call_from_hook_dt(self.ee, RIDER_POSE, &[r, 0], dt)
            }
            C::Settle => {
                assert_eq!(vt(self.ee, RIDER + 4, 0xcc), RIDER_SETTLE_VT, "the rider's vt +0xcc");
                call_from_hook_dt(self.ee, RIDER_SETTLE_VT, &[r], dt)
            }
            C::Switch { part, on } => {
                // The aim force keeps its vtable at +8, the parts at +0x10.
                let at = if part == self.ee.u32_at(RIDER + rider::AIM_FORCE) { 8 } else { 0x10 };
                let f = vt(self.ee, part + at, 0x10);
                call_from_hook_dt(self.ee, f, &[u64::from(part), u64::from(on)], dt)
            }
            C::Gather { list, probe, kind } => call_from_hook_dt(self.ee, GATHER, &[u64::from(list), u64::from(probe), u64::from(kind)], dt),
            C::BodyTarget { body, from } => {
                assert_eq!(vt(self.ee, body + 0x110, 0x68), RIDER_BODY_TARGET, "the body's vt +0x68");
                rider::body_target(self, body, from);
                0
            }
            C::BodyMass { body, mass } => {
                assert_eq!(vt(self.ee, body + 0x110, 0x18), RIDER_BODY_MASS, "the body's vt +0x18");
                rider::body_mass(self, body, mass);
                0
            }
            C::BodyInertia { body, from } => {
                assert_eq!(vt(self.ee, body + 0x110, 0x60), RIDER_BODY_INERTIA, "the body's vt +0x60");
                rider::body_inertia(self, body, from);
                0
            }
            C::BodyAt { body, from } => {
                assert_eq!(vt(self.ee, body + 0x110, 0x20), RIDER_BODY_AT, "the body's vt +0x20");
                rider::body_at(self, body, from);
                0
            }
            C::Grow { list, bytes } => call_from_hook_dt(self.ee, HEAP_RESIZE, &[u64::from(list), u64::from(bytes), 0x0039_683e, 0x4cf], dt),
        };
        let name = match c {
            C::Rand => "> rand",
            C::RidingReset => "= riding reset",
            C::WreckEntry(_) => "= wreck entry",
            C::WreckCue(_) => "> wreck cue",
            C::Pose => "> pose",
            C::Settle => "> settle",
            C::Switch { .. } => "> switch",
            C::Gather { .. } => "> gather",
            C::BodyTarget { .. } => "= body target",
            C::BodyMass { .. } => "= body mass",
            C::BodyInertia { .. } => "= body inertia",
            C::BodyAt { .. } => "= body at",
            C::Grow { .. } => "> heap resize",
        };
        leaf_served(name);
        v as u32
    }
}

/// **The rider's listener step** on the ports ([`TreeInRam`] when [`in_rust`] names `rider leaf`):
/// the rider's frame runs in the interpreter with its mode machine [`rider::modes`]'s, its bone
/// collision solves [`bone::solve`]'s (the scene's calls the original's), its part clears and
/// hand-over [`rider::clear`]'s and [`rider::hand_over`]'s; then the wreck spots
/// ([`rider::wreck_spots`]); then the copy after and, with `+0xd34` set, the extra step, both the
/// original's. The bike's record gets every word the rider writes into it from the port.
fn rider_leaf(ee: &mut Ee, dt: F) {
    let map = nat05_map();
    let curve = curve();
    ee.hooks.insert(
        RIDER_MODES,
        Box::new(move |ee: &mut Ee| {
            assert_eq!(ee.r(4) as u32, RIDER, "the mode machine's rider");
            let dt = F(ee.f(12));
            let (pc, ra, sp) = (ee.pc, ee.r(31), ee.r(29));
            rider::modes(&mut LeafRider { ee }, RIDER, dt, Some(map));
            ee.pc = pc;
            ee.set(29, sp);
            ee.set(31, ra);
            leaf_served("= modes");
        }),
    );
    ee.hooks.insert(
        BONE_SOLVE,
        Box::new(move |ee: &mut Ee| {
            let (part, dt) = (ee.r(4) as u32, F(ee.f(12)));
            bone::solve(&mut RiderRam { ee, overlay: None }, &mut OriginalScene, part, dt, Some(map), curve);
            ee.pc = BONE_SOLVE;
            leaf_served("= bone solve");
        }),
    );
    ee.hooks.insert(
        PART_CLEAR,
        Box::new(|ee: &mut Ee| {
            let part = ee.r(4) as u32;
            rider::clear(&mut RiderRam { ee, overlay: None }, part);
            leaf_served("= part clear");
        }),
    );
    ee.hooks.insert(
        HAND_OVER,
        Box::new(|ee: &mut Ee| {
            assert_eq!(ee.r(4) as u32, RIDER, "the hand-over's rider");
            rider::hand_over(&mut RiderRam { ee, overlay: None }, RIDER);
            leaf_served("= hand-over");
        }),
    );
    ee.set(28, GP);
    ee.call(RIDER_FRAME, &[u64::from(RIDER)], &[dt.0], BUDGET);
    for a in [RIDER_MODES, BONE_SOLVE, PART_CLEAR, HAND_OVER] {
        ee.hooks.remove(&a);
    }
    rider::wreck_spots(&mut RiderRam { ee, overlay: None }, RIDER);
    leaf_served("= wreck spots");
    ee.set(28, GP);
    ee.call(RIDER_AFTER, &[u64::from(RIDER)], &[], BUDGET);
    if ee.u32_at(RIDER + 0xd34) != 0 {
        ee.set(28, GP);
        ee.call(RIDER_EXTRA, &[u64::from(RIDER)], &[dt.0], BUDGET);
        leaf_served("> extra step");
    }
}

/// [`replay_whole`]'s ports with the rider leaf's too ([`rider_leaf`]). Answers what
/// [`replay_pure`] answers, what the leaf served, and the interpreter's census.
#[allow(clippy::type_complexity)]
fn replay_with_rider_leaf(name: &str) -> Option<(Vec<bool>, Vec<String>, BTreeMap<&'static str, u32>, BTreeMap<String, u64>)> {
    INTERPRETED.with(|m| m.borrow_mut().clear());
    LEAF_SERVED.with(|s| s.borrow_mut().clear());
    let ran = replay_pure(name, &["PartMove", "Gather", "record sweep", "record search", "rider leaf"]);
    let census = INTERPRETED.with(|m| std::mem::take(&mut *m.borrow_mut()));
    let served = LEAF_SERVED.with(|s| std::mem::take(&mut *s.borrow_mut()));
    let (w, _, off) = ran?;
    eprintln!("{name}: the rider leaf served {served:?}\n  the interpreter ran {census:?}");
    Some((w, off, served, census))
}

/// **F31b5i — C15's ride with the rider leaf on the ports**: ride-2 bit-exact, every port at once
/// and the rider's mode machine, bone solves, clears, hand-over and wreck spots Rust's.
#[test]
fn ride_replays_with_the_rider_leaf_on_the_ports() {
    let Some((w, off, served, census)) = replay_with_rider_leaf("ride-2") else { return };
    assert_eq!(w.len(), 459);
    assert!(w.iter().all(|w| !w), "the ride never wrecks");
    assert_eq!(leaf_off(&off), Vec::<String>::new(), "the RAM is the handing run's but the ground query's ring");
    assert_eq!(
        served.into_iter().collect::<Vec<_>>(),
        [("= bone solve", 12393), ("= hand-over", 459), ("= modes", 459), ("= wreck spots", 459), ("> extra step", 459), ("> switch", 1836)],
        "{census:?}"
    );
}

/// **F31b5i — C15's flip with the rider leaf on the ports**: flip-1 bit-exact, wrecked on row 125
/// and put back on row 193, the rider's writes into the bike's record the port's; the put back's
/// riding reset the port's too (S287), and the wreck entry the port's (S288).
#[test]
fn flip_replays_with_the_rider_leaf_on_the_ports() {
    let Some((w, off, served, census)) = replay_with_rider_leaf("flip-1") else { return };
    assert_eq!(w.len(), 465);
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [125, 193], "wrecked on row 125, put back on row 193");
    assert_eq!(leaf_off(&off), ["0x851fec..0x851ff0"], "the RAM differs only at the cue's write and the ground query's ring");
    assert_eq!(
        served.into_iter().collect::<Vec<_>>(),
        [
            ("= body at", 18),
            ("= body inertia", 36),
            ("= body mass", 36),
            ("= body target", 55),
            ("= bone solve", 12303),
            ("= hand-over", 465),
            ("= modes", 465),
            ("= riding reset", 1),
            ("= wreck entry", 1),
            ("= wreck spots", 465),
            ("> extra step", 465),
            ("> gather", 68),
            ("> pose", 4),
            ("> rand", 5),
            ("> settle", 4),
            ("> switch", 1501),
            ("> wreck cue", 1)
        ],
        "{census:?}"
    );
}

/// The rider's terrain, whose two words (`+0x9e0`, `+0x9e4`) keep the ground query's ring.
const RIDER_TERRAIN: u32 = 0x00bb_d720;

/// `off`'s ranges but those inside the ground query's ring ([`GROUND_RING`], 32 cells: S278b's
/// oracle's) and its terrain's two words (S278a, S285: the port's query reads the map and keeps no
/// ring).
fn leaf_off(off: &[String]) -> Vec<String> {
    let kept = |a: u32| (GROUND_RING..GROUND_RING + 0x70 * 32).contains(&a) || (RIDER_TERRAIN + 0x9e0..RIDER_TERRAIN + 0x9e8).contains(&a);
    off.iter()
        .filter(|x| {
            let (s, e) = x.split_once("..").unwrap();
            let p = |t: &str| u32::from_str_radix(t.trim_start_matches("0x"), 16).unwrap();
            !(p(s)..p(e)).all(kept)
        })
        .cloned()
        .collect()
}


// ---------------------------------------------------------------------------------------------
// F51: the sweep probe's paths C15 never takes — its pair test (meshes, hierarchies, the kinds it
// strikes nothing of) with segments on nodes, and the global search — each against the original
// (research/oracle.txt `# F51`).
// ---------------------------------------------------------------------------------------------

use game_sim::atv_sweep;
use game_sim::collide::{MAX_PROBES, MeshRef};
use game_sim::sweep_probe::{Placed, Shape, SweepProbe, Target, Transform};

/// The sweep probe's set (its points and nodes), clear (vt `+0xb0`) and pair (vt `+0xb4`).
const SWEEP_SET: u32 = PROBE_SET;
const SWEEP_CLEAR: u32 = 0x001b_95f0;
const SWEEP_PAIR: u32 = 0x001b_9a60;
/// The run of RAM the oracles' arrays, mesh copies and hierarchies go in ([`F47_AREA`]'s run).
const F51_AREA: u32 = F47_AREA;
const F51_AREA_LEN: u32 = 0x10000;
const F51_CASES: usize = 10_000;

/// A collidable's fields as the pair reads them.
mod f51_at {
    pub const NEARBY: u32 = 0x45;
    pub const CENTRE: u32 = 0x50;
    pub const RADIUS: u32 = 0x70;
    pub const KIND: u32 = 0x78;
    pub const FLAGS: u32 = 0x126;
    pub const TRANSFORM: u32 = 0x130;
    pub const PARTS: u32 = 0x210;
    pub const PART_LIST: u32 = 0x224;
    pub const VERTICES: u32 = 0x230;
    pub const PACKED: u32 = 0x238;
    pub const NODES: u32 = 0x244;
    pub const TRIANGLES: u32 = 0x248;
    /// How much of a mesh collidable a copy takes.
    pub const MESH_LEN: u32 = 0x250;
}

/// The sweep probe's own fields (MX Unleashed's moved `+0x10`).
mod f51_probe {
    pub const RECORD: u32 = 0x7c;
    pub const COUNT: u32 = 0x130;
    pub const NODES: u32 = 0x134;
    pub const ENDS: u32 = 0x138;
    pub const LOCAL_ORIGINS: u32 = 0x13c;
    pub const LOCAL_DIRS: u32 = 0x140;
    pub const LENGTHS: u32 = 0x144;
    pub const ORIGINS: u32 = 0x148;
    pub const DIRS: u32 = 0x14c;
    pub const LOCAL_CENTRE: u32 = 0x150;
    pub const CENTRE: u32 = 0x160;
    pub const RADIUS: u32 = 0x170;
    pub const SMOOTH: u32 = 0x174;
}

/// The record's fields as the sweep probe uses them.
mod f51_rec {
    pub const NORMAL: u32 = 0x20;
    pub const POINT: u32 = 0x30;
    pub const PUSH: u32 = 0x40;
    pub const PART: u32 = 0x84;
    pub const HITS: u32 = 0x8c;
    pub const NORMALS: u32 = 0x94;
    pub const POINTS: u32 = 0x98;
    pub const STRUCK: u32 = 0x9c;
    pub const OWNERS: u32 = 0xa0;
    pub const FLAGS: u32 = 0xa4;
    pub const FRACTIONS: u32 = 0xa8;
    pub const WORDS: u32 = 0xac;
    pub const NEAREST: u32 = 0xb4;
}

/// A mesh collidable's mesh as the sweep reads it: everything reachable from node 0, the triangles'
/// three vertex shorts, and the vertices wide or packed.
struct SweptMesh {
    nodes: Vec<track::Node>,
    triangles: Vec<track::Triangle>,
    vertices: track::Points,
}

impl SweptMesh {
    fn read(ee: &Ee, c: u32) -> SweptMesh {
        use f51_at as at;
        let (nodes_at, tris_at) = (ee.u32_at(c + at::NODES), ee.u32_at(c + at::TRIANGLES));
        let node = |i: u32| {
            let a = nodes_at + 0x24 * i;
            let v3 = |o: u32| [0, 4, 8].map(|k| word_f(ee, a + o + k));
            track::Node { volume: word_f(ee, a + 4), centre: v3(0xc), half_extent: v3(0x18), children: [ee.u16_at(a + 8), ee.u16_at(a + 0xa)] }
        };
        let (mut n_nodes, mut n_tris) = (1u32, 0u32);
        let mut todo = vec![0u32];
        let mut seen = std::collections::BTreeSet::new();
        while let Some(i) = todo.pop() {
            if !seen.insert(i) {
                continue;
            }
            let n = node(i);
            for c in 0..2 {
                match n.child(c) {
                    track::Child::Node(j) => {
                        n_nodes = n_nodes.max(u32::from(j) + 1);
                        todo.push(u32::from(j));
                    }
                    track::Child::Leaf(t) => n_tris = n_tris.max(u32::from(t) + 1),
                }
            }
        }
        let nodes = (0..n_nodes).map(node).collect();
        let triangles: Vec<track::Triangle> = (0..n_tris)
            .map(|i| track::Triangle { distance: F::ZERO, normal: [F::ZERO; 3], vertices: [0, 2, 4].map(|k| ee.u16_at(tris_at + 0x14 * i + k)) })
            .collect();
        let corners = triangles.iter().flat_map(|t| t.vertices).max().map_or(0, |m| u32::from(m) + 1);
        let va = ee.u32_at(c + at::VERTICES);
        let vertices = if ee.u32_at(c + at::PACKED) != 0 {
            track::Points::Packed((0..corners).map(|i| [0, 2, 4].map(|k| ee.u16_at(va + 6 * i + k) as i16)).collect())
        } else {
            track::Points::Wide((0..corners).map(|i| [0, 4, 8].map(|k| word_f(ee, va + 12 * i + k))).collect())
        };
        SweptMesh { nodes, triangles, vertices }
    }

    fn mesh(&self) -> MeshRef<'_> {
        MeshRef { nodes: &self.nodes, triangles: &self.triangles, vertices: &self.vertices }
    }
}

fn f51_transform(ee: &Ee, c: u32) -> Transform {
    [0, 1, 2, 3].map(|k| vf_at(ee, c + f51_at::TRANSFORM + 0x10 * k))
}

/// A world point through a transform in host arithmetic: only for aiming segments and placing copies.
#[allow(clippy::float_arithmetic)]
fn f51_place(m: &Transform, p: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|k| m[3].0[k] + m[0].0[k] * p[0] + m[1].0[k] * p[1] + m[2].0[k] * p[2])
}

/// The world the pair oracle pairs against, in RAM: every object of the probe's database world
/// (nat05's 259 meshes, its plants and probes), copies of some meshes turned and moved, and
/// hierarchies of two or three such copies each (nat05 has none of its own). Each hierarchy's parts
/// stand just before it in [`SweepScene::objects`].
struct SweepScene {
    probe: u32,
    rec: u32,
    objects: Vec<u32>,
    meshes: Vec<Option<SweptMesh>>,
    /// Per object the meshes it is swept through: a mesh its own, a hierarchy its parts.
    parts: Vec<Vec<usize>>,
    /// The objects with triangles to aim at.
    aims: Vec<usize>,
    arrays: SweepArrays,
}

/// Where the probe's and the record's arrays are moved to (the probe's own hold one segment),
/// sixteen segments each; and the set's two arguments.
struct SweepArrays {
    points: u32,
    set_nodes: u32,
    node_mats: u32,
    node_ptrs: u32,
    ends: u32,
    local_origins: u32,
    local_dirs: u32,
    lengths: u32,
    origins: u32,
    dirs: u32,
    normals: u32,
    rpoints: u32,
    struck: u32,
    owners: u32,
    flags: u32,
    fractions: u32,
    words: u32,
}

/// A copy of mesh collidable `from` at `c`, sharing its tree, turned by `q` round its own
/// translation and moved by `by`; its sphere moved with it and grown a little.
#[allow(clippy::float_arithmetic)]
fn f51_turned_copy(ee: &mut Ee, from: u32, c: u32, q: Vf, by: [f32; 3], w_junk: [f32; 3]) {
    use f51_at as at;
    for o in (0..at::MESH_LEN).step_by(4) {
        ee.set_u32(c + o, ee.u32_at(from + o));
    }
    let old = f51_transform(ee, from);
    let centre = vf_at(ee, from + at::CENTRE);
    // The sphere's centre in the mesh's space (the rows are a rotation).
    let rel: [f32; 3] = std::array::from_fn(|k| centre.0[k] - old[3].0[k]);
    let local: [f32; 3] = std::array::from_fn(|r| (0..3).map(|k| rel[k] * old[r].0[k]).sum());
    let [x, y, z, w] = q.0;
    let rows = [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + z * w), 2.0 * (x * z - y * w)],
        [2.0 * (x * y - z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + x * w)],
        [2.0 * (x * z + y * w), 2.0 * (y * z - x * w), 1.0 - 2.0 * (x * x + y * y)],
    ];
    let mut m = old;
    for r in 0..3 {
        m[r] = Vf::new(rows[r][0], rows[r][1], rows[r][2], w_junk[r]);
    }
    m[3] = Vf::new(old[3].0[0] + by[0], old[3].0[1] + by[1], old[3].0[2] + by[2], old[3].0[3]);
    for r in 0..4 {
        set_vf(ee, c + at::TRANSFORM + 0x10 * r as u32, m[r]);
    }
    let moved = f51_place(&m, local);
    set_vf(ee, c + at::CENTRE, Vf::new(moved[0], moved[1], moved[2], centre.w()));
    let r = word_f(ee, from + at::RADIUS).0;
    ee.set_u32(c + at::RADIUS, (r * 1.01 + 0.05).to_bits());
}

#[allow(clippy::float_arithmetic)]
fn sweep_scene(ee: &mut Ee, g: &mut Gen) -> SweepScene {
    use f51_at as at;
    for a in (F51_AREA..F51_AREA + F51_AREA_LEN).step_by(4) {
        assert_eq!(ee.u32_at(a), 0, "{a:#x} of the oracles' run is not free");
    }
    let probe = ee.u32_at(ee.u32_at(BIKE + seed::RECORD) + 0x204);
    assert_eq!(ee.u32_at(probe + at::KIND), 3, "the record's sweep is not a sweep probe");
    let rec = ee.u32_at(probe + f51_probe::RECORD);
    let (m, world, _) = GatherInRam::read(ee, probe);
    let mut objects: Vec<u32> =
        (1..world.len()).filter(|&k| world[k].class == ws::Class::Object).map(|k| m.items[k] - 0x18).filter(|&c| c != probe).collect();
    let mut next = F51_AREA;
    let mut take = |n: u32| {
        let a = next;
        next += (n + 0xf) & !0xf;
        assert!(next <= F51_AREA + F51_AREA_LEN, "the oracles' run is full");
        a
    };
    let p = MAX_PROBES as u32;
    let arrays = SweepArrays {
        points: take(0x20 * p),
        set_nodes: take(8 * p),
        node_mats: take(0x80 * p),
        node_ptrs: take(8 * p),
        ends: take(0x20 * p),
        local_origins: take(0x10 * p),
        local_dirs: take(0x10 * p),
        lengths: take(4 * p),
        origins: take(0x10 * p),
        dirs: take(0x10 * p),
        normals: take(0x10 * p),
        rpoints: take(0x10 * p),
        struck: take(4 * p),
        owners: take(4 * p),
        flags: take(4 * p),
        fractions: take(4 * p),
        words: take(4 * p),
    };
    let solid: Vec<u32> = objects.iter().copied().filter(|&c| ee.u32_at(c + at::KIND) == 0 && ee.u32_at(c + at::TRIANGLES) != 0).collect();
    let mut parts: Vec<Vec<usize>> = Vec::new();
    for k in 0..objects.len() {
        parts.push(if ee.u32_at(objects[k] + at::KIND) == 0 { vec![k] } else { vec![] });
    }
    let copy = |ee: &mut Ee, g: &mut Gen, take: &mut dyn FnMut(u32) -> u32, from: u32| {
        let c = take(at::MESH_LEN);
        let w = [0; 3].map(|_| if g.one_in(4) { g.f(-1.0, 1.0) } else { 0.0 });
        let by = [0; 3].map(|_| g.f(-3.0, 3.0));
        f51_turned_copy(ee, from, c, g.unit_quaternion(), by, w);
        c
    };
    // Turned and moved copies on their own.
    for _ in 0..12 {
        let from = solid[(g.bits() as usize) % solid.len()];
        let c = copy(ee, g, &mut take, from);
        parts.push(vec![objects.len()]);
        objects.push(c);
    }
    // Hierarchies: a mesh and its one or two nearest, each copied (turned and moved a little, or
    // standing where it stood), the parts listed; the sphere round all of them.
    for h in 0..10 {
        let first = solid[(g.bits() as usize) % solid.len()];
        let c0 = vf_at(ee, first + at::CENTRE);
        let mut by: Vec<(f32, u32)> = solid
            .iter()
            .filter(|&&c| c != first)
            .map(|&c| {
                let d = vf_at(ee, c + at::CENTRE);
                ((0..3).map(|i| (d.0[i] - c0.0[i]).powi(2)).sum::<f32>(), c)
            })
            .collect();
        by.sort_by(|a, b| a.0.total_cmp(&b.0));
        let n = 2 + (g.bits() % 2) as usize;
        let froms: Vec<u32> = std::iter::once(first).chain(by.iter().take(n - 1).map(|&(_, c)| c)).collect();
        let list_at = take(4 * n as u32);
        let mut mine = Vec::new();
        let mut reach = 0.0f32;
        for (j, &from) in froms.iter().enumerate() {
            let c = if h % 3 == 0 {
                let c = take(at::MESH_LEN);
                f51_turned_copy(ee, from, c, Vf::new(0.0, 0.0, 0.0, 1.0), [0.0; 3], [0.0; 3]);
                for r in 0..3 {
                    set_vf(ee, c + at::TRANSFORM + 0x10 * r, vf_at(ee, from + at::TRANSFORM + 0x10 * r));
                }
                c
            } else {
                copy(ee, g, &mut take, from)
            };
            ee.set_u32(list_at + 4 * j as u32, c);
            let d = vf_at(ee, c + at::CENTRE);
            let far = (0..3).map(|i| (d.0[i] - c0.0[i]).powi(2)).sum::<f32>().sqrt() + word_f(ee, c + at::RADIUS).0;
            reach = reach.max(far);
            mine.push(objects.len());
            parts.push(vec![objects.len()]);
            objects.push(c);
        }
        let hc = take(0x230);
        set_u8(ee, hc + at::NEARBY, 1);
        set_vf(ee, hc + at::CENTRE, c0);
        ee.set_u32(hc + at::RADIUS, (reach * 1.01 + 0.05).to_bits());
        ee.set_u32(hc + at::KIND, 1);
        ee.set_u32(hc + at::PARTS, n as u32);
        ee.set_u32(hc + at::PART_LIST, list_at);
        parts.push(mine);
        objects.push(hc);
    }
    let meshes: Vec<Option<SweptMesh>> = objects.iter().map(|&c| (ee.u32_at(c + at::KIND) == 0).then(|| SweptMesh::read(ee, c))).collect();
    for &c in &objects {
        assert_ne!(ee.u32_at(c + at::KIND), 8, "{c:#x} is a finite plane: its pair is not ported");
    }
    let aims = (0..objects.len()).filter(|&k| parts[k].iter().any(|&j| meshes[j].as_ref().is_some_and(|m| !m.triangles.is_empty()))).collect();
    SweepScene { probe, rec, objects, meshes, parts, aims, arrays }
}

impl SweepScene {
    fn index_of(&self, a: u32) -> Option<usize> {
        (a != 0).then(|| self.objects.iter().position(|&o| o == a).unwrap_or(1_000_000 + a as usize))
    }

    /// Object `c` as the pair reads it.
    fn target<'a>(&self, ee: &Ee, placed: &'a [Placed<'a>], c: usize) -> Target<'a> {
        use f51_at as at;
        let o = self.objects[c];
        let shape = match ee.u32_at(o + at::KIND) {
            0 => Shape::Mesh(placed[c]),
            1 => {
                let first = self.parts[c][0];
                Shape::Hierarchy(&placed[first..first + self.parts[c].len()])
            }
            _ => Shape::Other,
        };
        Target { nearby: ee.u8_at(o + at::NEARBY) & 1 != 0, centre: vf_at(ee, o + at::CENTRE), radius: word_f(ee, o + at::RADIUS), shape }
    }
}

fn f51_unit(g: &mut Gen) -> [f32; 3] {
    #[allow(clippy::float_arithmetic)]
    loop {
        let v = [g.f(-1.0, 1.0), g.f(-1.0, 1.0), g.f(-1.0, 1.0)];
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if l > 0.1 && l <= 1.0 {
            return v.map(|x| x / l);
        }
    }
}

fn f51_junk(g: &mut Gen) -> Vf {
    Vf([0; 4].map(|_| g.f(-50.0, 50.0)))
}

/// One generated case for the pair oracle: its points and nodes (`None` when the set is given no
/// node array), and the objects to pair against.
struct SweepCase {
    points: Vec<[Vf; 2]>,
    nodes: Option<Vec<[Option<Transform>; 2]>>,
    pairs: Vec<usize>,
}

#[allow(clippy::float_arithmetic)]
fn sweep_case(ee: &Ee, s: &SweepScene, g: &mut Gen) -> SweepCase {
    let pick = |g: &mut Gen, n: usize| (g.bits() as usize) % n;
    // A hierarchy one case in four (there are few of them).
    let hierarchies: Vec<usize> = s.aims.iter().copied().filter(|&k| s.meshes[k].is_none()).collect();
    let aim = if g.one_in(4) { hierarchies[pick(g, hierarchies.len())] } else { s.aims[pick(g, s.aims.len())] };
    let mut pairs = vec![aim];
    for _ in 0..pick(g, 3) {
        pairs.push(if g.one_in(2) { s.aims[pick(g, s.aims.len())] } else { pick(g, s.objects.len()) });
    }
    let count = match pick(g, 10) {
        0..=4 => 1,
        5..=7 => 2 + pick(g, 3),
        _ => 5 + pick(g, 12),
    };
    let away = if g.one_in(10) { [g.f(-200.0, 200.0), g.f(50.0, 200.0), g.f(-200.0, 200.0)] } else { [0.0; 3] };
    let with_nodes = !g.one_in(8);
    let (mut points, mut nodes) = (Vec::new(), Vec::new());
    let mut last = [0.0f32; 3];
    for i in 0..count {
        let solid: Vec<usize> = s.parts[aim].iter().copied().filter(|&j| s.meshes[j].as_ref().is_some_and(|m| !m.triangles.is_empty())).collect();
        let part = solid[pick(g, solid.len())];
        let mesh = s.meshes[part].as_ref().unwrap();
        let m = f51_transform(ee, s.objects[part]);
        let target = if i > 0 && g.one_in(2) {
            last.map(|x| x + g.f(-0.3, 0.3))
        } else {
            let t = &mesh.triangles[pick(g, mesh.triangles.len())];
            let c = mesh.mesh().corners(t).map(|v| f51_place(&m, [v.0[0], v.0[1], v.0[2]]));
            let (mut u, mut v) = (g.f(-0.15, 1.0), g.f(-0.15, 1.0));
            if u + v > 1.1 {
                (u, v) = (1.0 - u, 1.0 - v);
            }
            std::array::from_fn(|k| c[0][k] + (c[1][k] - c[0][k]) * u + (c[2][k] - c[0][k]) * v)
        };
        last = target;
        let dir = f51_unit(g);
        let length = if g.one_in(30) { 0.0 } else { g.f(0.005, 3.0) };
        let before = length * g.f(-0.2, 1.2);
        let a: [f32; 3] = std::array::from_fn(|k| target[k] - dir[k] * before + away[k]);
        let b: [f32; 3] = std::array::from_fn(|k| a[k] + dir[k] * length);
        let mut seg = [Vf::ZERO; 2];
        let mut on = [None; 2];
        for (k, e) in [a, b].into_iter().enumerate() {
            let w = if g.one_in(4) { g.f(-2.0, 2.0) } else { 1.0 };
            if with_nodes && g.one_in(5) {
                // On a node: nearly the identity, moved.
                let mut node: Transform = [Vf([1.0, 0.0, 0.0, 0.0]), Vf([0.0, 1.0, 0.0, 0.0]), Vf([0.0, 0.0, 1.0, 0.0]), Vf::ZERO];
                for row in node.iter_mut().take(3) {
                    for c in 0..4 {
                        row.0[c] += g.f(-0.02, 0.02);
                    }
                }
                let t = [g.f(-2.0, 2.0), g.f(-2.0, 2.0), g.f(-2.0, 2.0)];
                node[3] = Vf([t[0], t[1], t[2], g.f(-1.0, 1.0)]);
                on[k] = Some(node);
                seg[k] = Vf([e[0] - t[0], e[1] - t[1], e[2] - t[2], w]);
            } else {
                seg[k] = Vf([e[0], e[1], e[2], w]);
            }
        }
        if g.one_in(40) {
            seg[1] = seg[0];
            on[1] = on[0];
        }
        points.push(seg);
        nodes.push(on);
    }
    SweepCase { points, nodes: with_nodes.then_some(nodes), pairs }
}

/// The case into RAM: the probe's and the record's arrays moved to the scene's, the points and
/// nodes where the set takes them, leftovers of an earlier search in the forms and the answer (the
/// same in `port` and `more`), and each candidate's nearby bit mostly on.
fn sweep_case_in_ram(ee: &mut Ee, s: &SweepScene, g: &mut Gen, case: &SweepCase, port: &mut SweepProbe, more: &mut atv_sweep::More) {
    use f51_probe as pa;
    use f51_rec as ra;
    let (pr, a) = (s.probe, &s.arrays);
    ee.set_u32(pr + pa::COUNT, case.points.len() as u32);
    for (key, at) in [
        (pa::NODES, a.node_ptrs),
        (pa::ENDS, a.ends),
        (pa::LOCAL_ORIGINS, a.local_origins),
        (pa::LOCAL_DIRS, a.local_dirs),
        (pa::LENGTHS, a.lengths),
        (pa::ORIGINS, a.origins),
        (pa::DIRS, a.dirs),
    ] {
        ee.set_u32(pr + key, at);
    }
    assert_eq!(ee.u32_at(pr + pa::RECORD), s.rec);
    assert_eq!(ee.u32_at(pr + pa::SMOOTH), 0, "the probe asks for smoothed normals");
    for (i, seg) in case.points.iter().enumerate() {
        let i = i as u32;
        for k in 0..2u32 {
            set_vf(ee, a.points + 0x20 * i + 0x10 * k, seg[k as usize]);
            // What the set overwrites: junk.
            set_vf(ee, a.ends + 0x20 * i + 0x10 * k, f51_junk(g));
            ee.set_u32(a.node_ptrs + 8 * i + 4 * k, g.bits());
            let slot = a.node_mats + 0x80 * i + 0x40 * k;
            let node = case.nodes.as_ref().and_then(|n| n[i as usize][k as usize]);
            match node {
                Some(m) => {
                    for r in 0..4u32 {
                        set_vf(ee, slot + 0x10 * r, m[r as usize]);
                    }
                    // A node's world transform is its +0x70.
                    ee.set_u32(a.set_nodes + 8 * i + 4 * k, slot - 0x70);
                }
                None => ee.set_u32(a.set_nodes + 8 * i + 4 * k, 0),
            }
        }
    }
    for i in 0..MAX_PROBES {
        let (o, d, lo, ld) = (f51_junk(g), f51_junk(g), f51_junk(g), f51_junk(g));
        let iu = i as u32;
        set_vf(ee, a.origins + 0x10 * iu, o);
        set_vf(ee, a.dirs + 0x10 * iu, d);
        set_vf(ee, a.local_origins + 0x10 * iu, lo);
        set_vf(ee, a.local_dirs + 0x10 * iu, ld);
        port.world.probes[i].origin = o;
        port.world.probes[i].dir = d;
        port.local.probes[i].origin = lo;
        port.local.probes[i].dir = ld;
    }
    let lc = f51_junk(g);
    set_vf(ee, pr + pa::LOCAL_CENTRE, lc);
    port.local.centre = lc;

    let an = s.rec;
    for (key, at) in [
        (ra::NORMALS, a.normals),
        (ra::POINTS, a.rpoints),
        (ra::STRUCK, a.struck),
        (ra::OWNERS, a.owners),
        (ra::FLAGS, a.flags),
        (ra::FRACTIONS, a.fractions),
        (ra::WORDS, a.words),
    ] {
        ee.set_u32(an + key, at);
    }
    let ans = &mut port.answer;
    ans.head = [f51_junk(g), f51_junk(g)];
    set_vf(ee, an, ans.head[0]);
    set_vf(ee, an + 0x10, ans.head[1]);
    let r = &mut ans.record;
    r.nearest_normal = f51_junk(g);
    r.nearest_point = f51_junk(g);
    r.push = f51_junk(g);
    r.hits = (g.bits() % 5) as i32;
    r.nearest = F(g.f(0.0, 1.0));
    set_vf(ee, an + ra::NORMAL, r.nearest_normal);
    set_vf(ee, an + ra::POINT, r.nearest_point);
    set_vf(ee, an + ra::PUSH, r.push);
    ee.set_u32(an + ra::HITS, r.hits as u32);
    ee.set_u32(an + ra::NEAREST, r.nearest.0.to_bits());
    let part = if g.one_in(3) { 0 } else { s.objects[(g.bits() as usize) % s.objects.len()] };
    ee.set_u32(an + ra::PART, part);
    more.part = s.index_of(part);
    for i in 0..MAX_PROBES {
        let iu = i as u32;
        r.fraction[i] = F(if g.one_in(2) { -1.0 } else { g.f(0.0, 1.0) });
        r.point[i] = f51_junk(g);
        r.normal[i] = f51_junk(g);
        r.hit[i] = g.one_in(2);
        r.owner[i] = (g.bits() % 2) as i32;
        more.words[i] = g.bits();
        let struck = if g.one_in(3) { 0 } else { s.objects[(g.bits() as usize) % s.objects.len()] };
        ans.struck[i] = s.index_of(struck);
        ee.set_u32(a.fractions + 4 * iu, r.fraction[i].0.to_bits());
        set_vf(ee, a.rpoints + 0x10 * iu, r.point[i]);
        set_vf(ee, a.normals + 0x10 * iu, r.normal[i]);
        ee.set_u32(a.flags + 4 * iu, u32::from(r.hit[i]));
        ee.set_u32(a.owners + 4 * iu, r.owner[i] as u32);
        ee.set_u32(a.words + 4 * iu, more.words[i]);
        ee.set_u32(a.struck + 4 * iu, struck);
    }
    for &c in &case.pairs {
        let o = s.objects[c] + f51_at::NEARBY;
        let byte = ee.u8_at(o);
        let on = !g.one_in(12);
        set_u8(ee, o, byte & !1 | u8::from(on));
    }
}

#[derive(Default, Debug)]
struct SweepReached {
    pairs: usize,
    struck: usize,
    refused: usize,
    hierarchy_struck: usize,
    parts_kept: usize,
    others: usize,
    segments_hit: usize,
    on_nodes: usize,
    no_nodes: usize,
    no_length: usize,
    sixteen: usize,
}

/// The pair against the original's (F51): over 10,000 generated probes of 1 to 16 segments (some on
/// nodes, some of no length, some far from everything), aimed through the triangles of nat05's
/// meshes, of turned copies of them and of hierarchies made of copies, the original's set, clear and
/// one to three pairs against [`atv_sweep::set`], [`sweep_probe::clear`] and [`atv_sweep::pair`] on
/// the same numbers; every return, both forms of the segments, the sphere, and the whole record
/// compared bit for bit — its part (`+0x84`) and third word array (`+0xac`) too.
#[test]
fn the_sweeps_pair_is_the_originals() {
    use f51_probe as pa;
    use f51_rec as ra;
    let Some(mut ee) = image() else { return };
    ee.set(28, GP);
    let mut g = Gen(0x0f51_5eed_0000_0001);
    let s = sweep_scene(&mut ee, &mut g);
    let empty = track::Points::Wide(Vec::new());
    let placed: Vec<Placed> = (0..s.objects.len())
        .map(|i| {
            let mesh = match &s.meshes[i] {
                Some(m) => m.mesh(),
                None => MeshRef { nodes: &[], triangles: &[], vertices: &empty },
            };
            Placed { transform: f51_transform(&ee, s.objects[i]), mesh, object: i }
        })
        .collect();
    let (pr, a) = (s.probe, &s.arrays);
    let mut seen = SweepReached::default();
    for case in 0..F51_CASES {
        let c = sweep_case(&ee, &s, &mut g);
        let mut port = SweepProbe::default();
        let mut more = atv_sweep::More::default();
        sweep_case_in_ram(&mut ee, &s, &mut g, &c, &mut port, &mut more);
        let count = c.points.len();
        let set_nodes = if c.nodes.is_some() { a.set_nodes } else { 0 };
        seen.on_nodes += c.nodes.iter().flatten().filter(|n| n.iter().any(Option::is_some)).count();
        seen.no_nodes += usize::from(c.nodes.is_none());
        seen.no_length += c.points.iter().filter(|p| p[0] == p[1]).count();
        seen.sixteen += usize::from(count == MAX_PROBES);

        // The set.
        ee.call(SWEEP_SET, &[u64::from(pr), u64::from(a.points), u64::from(set_nodes)], &[], BUDGET);
        let (segments, b) = atv_sweep::set(&c.points, c.nodes.as_deref());
        let ctx = format!("case {case}: {count} segments");
        assert_eq!(vf_at(&ee, pr + f51_at::CENTRE).to_bits(), b.centre.to_bits(), "{ctx}: the set's centre");
        for o in [pa::RADIUS, f51_at::RADIUS] {
            assert_eq!(ee.u32_at(pr + o), b.reach.0.to_bits(), "{ctx}: the set's reach at {o:#x}");
        }
        for (i, seg) in segments.iter().enumerate() {
            let i = i as u32;
            for k in 0..2u32 {
                assert_eq!(vf_at(&ee, a.ends + 0x20 * i + 0x10 * k).to_bits(), seg.ends[k as usize].to_bits(), "{ctx}: segment {i}'s point {k}");
                let want = if seg.nodes[k as usize].is_some() { a.node_mats + 0x80 * i + 0x40 * k - 0x70 } else { 0 };
                assert_eq!(ee.u32_at(a.node_ptrs + 8 * i + 4 * k), want, "{ctx}: segment {i}'s node {k}");
            }
        }
        port.segments = segments;

        // The clear.
        let flags = ee.u8_at(pr + f51_at::FLAGS);
        ee.call(SWEEP_CLEAR, &[u64::from(pr)], &[], BUDGET);
        sweep_probe_clear(&mut port);
        assert_eq!(ee.u8_at(pr + f51_at::FLAGS), flags & !0x20, "{ctx}: the clear's hit bit");

        // The pairs.
        let mut returns = Vec::new();
        for &o in &c.pairs {
            ee.call(SWEEP_PAIR, &[u64::from(pr), u64::from(s.objects[o])], &[], BUDGET);
            let theirs = ee.v0() != 0;
            let t = s.target(&ee, &placed, o);
            let part = more.part;
            let ours = atv_sweep::pair(&mut port, &mut more, Some(&t));
            seen.parts_kept += usize::from(more.part != part);
            returns.push((o, theirs, ours));
            seen.pairs += 1;
            seen.struck += usize::from(theirs);
            seen.refused += usize::from(!t.nearby);
            seen.others += usize::from(matches!(t.shape, Shape::Other));
            seen.hierarchy_struck += usize::from(theirs && matches!(t.shape, Shape::Hierarchy(_)));
        }
        if case % 97 == 0 {
            ee.call(SWEEP_PAIR, &[u64::from(pr), 0], &[], BUDGET);
            assert_eq!(ee.v0(), 0, "{ctx}: a null pair");
            assert!(!atv_sweep::pair(&mut port, &mut more, None));
        }
        let ctx = format!("{ctx}, pairs {returns:?}");
        for (_, theirs, ours) in &returns {
            assert_eq!(theirs, ours, "{ctx}: the return");
        }

        // The probe.
        assert_eq!(vf_at(&ee, pr + pa::CENTRE).to_bits(), port.world.centre.to_bits(), "{ctx}: the centre");
        assert_eq!(vf_at(&ee, pr + f51_at::CENTRE).to_bits(), port.world.centre.to_bits(), "{ctx}: the collidable's centre");
        assert_eq!(ee.u32_at(pr + pa::RADIUS), port.world.radius.0.to_bits(), "{ctx}: the radius");
        assert_eq!(ee.u32_at(pr + f51_at::RADIUS), port.world.radius.0.to_bits(), "{ctx}: the reach");
        assert_eq!(vf_at(&ee, pr + pa::LOCAL_CENTRE).to_bits(), port.local.centre.to_bits(), "{ctx}: the local centre");
        for i in 0..count {
            let iu = i as u32;
            let (w, l) = (port.world.probes[i], port.local.probes[i]);
            assert_eq!(vf_at(&ee, a.origins + 0x10 * iu).to_bits(), w.origin.to_bits(), "{ctx}: segment {i}'s start");
            assert_eq!(vf_at(&ee, a.dirs + 0x10 * iu).to_bits(), w.dir.to_bits(), "{ctx}: segment {i}'s direction");
            assert_eq!(ee.u32_at(a.lengths + 4 * iu), w.length.0.to_bits(), "{ctx}: segment {i}'s length");
            assert_eq!(vf_at(&ee, a.local_origins + 0x10 * iu).to_bits(), l.origin.to_bits(), "{ctx}: segment {i}'s local start");
            assert_eq!(vf_at(&ee, a.local_dirs + 0x10 * iu).to_bits(), l.dir.to_bits(), "{ctx}: segment {i}'s local direction");
        }

        // The record.
        let (an, ans) = (s.rec, &port.answer);
        let r = &ans.record;
        assert_eq!(vf_at(&ee, an).to_bits(), ans.head[0].to_bits(), "{ctx}: +0");
        assert_eq!(vf_at(&ee, an + 0x10).to_bits(), ans.head[1].to_bits(), "{ctx}: +0x10");
        assert_eq!(vf_at(&ee, an + ra::NORMAL).to_bits(), r.nearest_normal.to_bits(), "{ctx}: the nearest normal");
        assert_eq!(vf_at(&ee, an + ra::POINT).to_bits(), r.nearest_point.to_bits(), "{ctx}: the nearest point");
        assert_eq!(vf_at(&ee, an + ra::PUSH).to_bits(), r.push.to_bits(), "{ctx}: the push");
        assert_eq!(ee.u32_at(an + ra::HITS) as i32, r.hits, "{ctx}: the tally");
        assert_eq!(ee.u32_at(an + ra::NEAREST), r.nearest.0.to_bits(), "{ctx}: the nearest fraction");
        assert_eq!(s.index_of(ee.u32_at(an + ra::PART)), more.part, "{ctx}: the part");
        for i in 0..MAX_PROBES {
            let iu = i as u32;
            assert_eq!(ee.u32_at(a.fractions + 4 * iu), r.fraction[i].0.to_bits(), "{ctx}: segment {i}'s fraction");
            assert_eq!(vf_at(&ee, a.rpoints + 0x10 * iu).to_bits(), r.point[i].to_bits(), "{ctx}: segment {i}'s point");
            assert_eq!(vf_at(&ee, a.normals + 0x10 * iu).to_bits(), r.normal[i].to_bits(), "{ctx}: segment {i}'s normal");
            assert_eq!(ee.u32_at(a.flags + 4 * iu), u32::from(r.hit[i]), "{ctx}: segment {i}'s hit word");
            assert_eq!(ee.u32_at(a.owners + 4 * iu) as i32, r.owner[i], "{ctx}: segment {i}'s owner word");
            assert_eq!(ee.u32_at(a.words + 4 * iu), more.words[i], "{ctx}: segment {i}'s +0xac word");
            assert_eq!(s.index_of(ee.u32_at(a.struck + 4 * iu)), ans.struck[i], "{ctx}: segment {i}'s struck");
            seen.segments_hit += usize::from(i < count && r.hit[i]);
        }
    }
    eprintln!("F51 pair: {seen:?}");
    assert!(seen.struck > 2000 && seen.hierarchy_struck > 1000 && seen.parts_kept > 1000 && seen.refused > 500 && seen.others > 500, "the cases do not reach enough");
    assert!(seen.on_nodes > 1000 && seen.no_nodes > 500 && seen.no_length > 100 && seen.sixteen > 100, "the segments do not vary enough");
}

fn sweep_probe_clear(probe: &mut SweepProbe) {
    game_sim::sweep_probe::clear(probe);
}

/// The global search's sweep descriptor global: the last mesh the pair swept (its `+0x230`).
const SWEEP_DESCRIPTOR_AT: u32 = 0x003d_ce38;
/// The pair's smooth-normals global (the probe's `+0x174`).
const SWEEP_SMOOTH_AT: u32 = 0x003d_c610;

/// The sweep's world in RAM: what each item is to the pair, read from RAM at each pair (a mesh, a
/// hierarchy and its parts, or nothing struck), and everything else as F46d's part search has it
/// ([`PartWorldInRam`]: a group's parts stood by the original, the listeners and callbacks handed
/// on). Keeps what the pairs leave in the two globals only the pair writes.
struct SweepWorldInRam<'w, 'e> {
    world: &'w mut PartWorldInRam<'e>,
    /// The probe's sphere as the clear stands it, for telling which pairs sweep.
    centre: Vf,
    radius: F,
    pairs: u32,
    descriptor: Option<u32>,
}

impl atv_sweep::Outside for SweepWorldInRam<'_, '_> {
    fn target(&mut self, other: usize, pair: &mut dyn FnMut(Option<&Target>) -> bool) -> bool {
        use f51_at as at;
        let ee = &*self.world.ee;
        let c = self.world.items[other] - 0x18;
        let kind = ee.u32_at(c + at::KIND);
        *self.world.kinds.entry(format!("sweep pair, kind {kind}")).or_default() += 1;
        self.pairs += 1;
        let (nearby, centre, radius) = (ee.u8_at(c + at::NEARBY) & 1 != 0, vf_at(ee, c + at::CENTRE), word_f(ee, c + at::RADIUS));
        let parts: Vec<u32> = match kind {
            0 => vec![c],
            1 => (0..ee.u32_at(c + at::PARTS)).map(|j| ee.u32_at(ee.u32_at(c + at::PART_LIST) + 4 * j)).collect(),
            8 => panic!("the sweep paired a finite plane ({c:#x}): its pair is not ported"),
            _ => vec![],
        };
        let meshes: Vec<SweptMesh> = parts.iter().map(|&p| SweptMesh::read(ee, p)).collect();
        let ids: Vec<usize> = parts
            .iter()
            .map(|&p| match self.world.items.iter().position(|&e| e == p + 0x18) {
                Some(k) => k,
                None => {
                    self.world.items.push(p + 0x18);
                    self.world.items.len() - 1
                }
            })
            .collect();
        let ee = &*self.world.ee;
        let placed: Vec<Placed> = meshes.iter().zip(&parts).zip(&ids).map(|((m, &p), &id)| Placed { transform: f51_transform(ee, p), mesh: m.mesh(), object: id }).collect();
        let shape = match kind {
            0 => Shape::Mesh(placed[0]),
            1 => Shape::Hierarchy(&placed),
            _ => Shape::Other,
        };
        // Whether the pair gets to its sweep: the spheres as the pair tests them.
        let d = self.centre.sub(centre);
        let reach = radius + self.radius;
        if nearby && F(ps2_float::fpu::dot([d.x(), d.y(), d.z()], [d.x(), d.y(), d.z()])) < reach * reach {
            if let Some(&last) = parts.last() {
                self.descriptor = Some(last + 0x230);
            }
        }
        pair(Some(&Target { nearby, centre, radius, shape }))
    }
}

impl ws::Narrow for SweepWorldInRam<'_, '_> {
    fn clear(&mut self, _: &mut [ws::Collidable], _: usize) {
        unreachable!("the sweep's own clear")
    }
    fn prepare(&mut self, _: &mut [ws::Collidable], _: usize) {
        panic!("the sweep prepared a grid")
    }
    fn pair(&mut self, _: &mut [ws::Collidable], _: usize, _: usize) -> bool {
        unreachable!("the sweep's own pair")
    }
    fn parts(&mut self, objects: &mut [ws::Collidable], group: usize) -> usize {
        self.world.parts(objects, group)
    }
    fn part(&mut self, objects: &mut [ws::Collidable], group: usize, i: usize) -> Option<usize> {
        self.world.part(objects, group, i)
    }
    fn heard_hit(&mut self, objects: &mut [ws::Collidable], l: usize, probe: usize, other: usize) {
        self.world.heard_hit(objects, l, probe, other)
    }
    fn heard_struck(&mut self, objects: &mut [ws::Collidable], l: usize, struck: usize, probe: usize) {
        self.world.heard_struck(objects, l, struck, probe)
    }
    fn on_hit(&mut self, objects: &mut [ws::Collidable], c: usize, probe: usize, other: usize) {
        self.world.on_hit(objects, c, probe, other)
    }
    fn on_struck(&mut self, objects: &mut [ws::Collidable], c: usize, struck: usize, probe: usize) {
        self.world.on_struck(objects, c, struck, probe)
    }
}

/// The sweep probe's segments in RAM: its count, its two node pointers (a node's world transform at
/// its `+0x70`) and its two points each.
fn sweep_segments_in_ram(ee: &Ee, probe: u32) -> Vec<game_sim::sweep_probe::Segment> {
    use f51_probe as pa;
    let (nodes, ends) = (ee.u32_at(probe + pa::NODES), ee.u32_at(probe + pa::ENDS));
    (0..ee.u32_at(probe + pa::COUNT))
        .map(|i| {
            let mut s = game_sim::sweep_probe::Segment::default();
            for k in 0..2 {
                s.ends[k as usize] = vf_at(ee, ends + 0x20 * i + 0x10 * k);
                let n = ee.u32_at(nodes + 8 * i + 4 * k);
                s.nodes[k as usize] = (n != 0).then(|| [0, 1, 2, 3].map(|r| vf_at(ee, n + 0x70 + 0x10 * r)));
            }
            s
        })
        .collect()
}

/// The sweep probe's answer in RAM (`+0x7c`), its first `count` segments' entries, and the record's
/// [`atv_sweep::More`]; collidables by `items` (one not listed is pushed).
fn sweep_answer_in_ram(ee: &Ee, rec: u32, count: usize, items: &mut Vec<u32>) -> (game_sim::sweep_probe::Answer, atv_sweep::More) {
    use f51_rec as ra;
    let mut item = |c: u32| -> Option<usize> {
        (c != 0).then(|| match items.iter().position(|&e| e == c + 0x18) {
            Some(k) => k,
            None => {
                items.push(c + 0x18);
                items.len() - 1
            }
        })
    };
    let mut a = game_sim::sweep_probe::Answer { head: [vf_at(ee, rec), vf_at(ee, rec + 0x10)], ..Default::default() };
    let mut more = atv_sweep::More { part: item(ee.u32_at(rec + ra::PART)), ..Default::default() };
    let r = &mut a.record;
    r.nearest_normal = vf_at(ee, rec + ra::NORMAL);
    r.nearest_point = vf_at(ee, rec + ra::POINT);
    r.push = vf_at(ee, rec + ra::PUSH);
    r.hits = ee.u32_at(rec + ra::HITS) as i32;
    r.nearest = word_f(ee, rec + ra::NEAREST);
    let [normals, points, struck, owners, hits, fractions, words] = [ra::NORMALS, ra::POINTS, ra::STRUCK, ra::OWNERS, ra::FLAGS, ra::FRACTIONS, ra::WORDS].map(|o| ee.u32_at(rec + o));
    for i in 0..count {
        let iu = i as u32;
        r.normal[i] = vf_at(ee, normals + 0x10 * iu);
        r.point[i] = vf_at(ee, points + 0x10 * iu);
        r.owner[i] = ee.u32_at(owners + 4 * iu) as i32;
        let h = ee.u32_at(hits + 4 * iu);
        assert!(h <= 1, "segment {i}'s hit word is {h:#x}");
        r.hit[i] = h != 0;
        r.fraction[i] = word_f(ee, fractions + 4 * iu);
        more.words[i] = ee.u32_at(words + 4 * iu);
        a.struck[i] = item(ee.u32_at(struck + 4 * iu));
    }
    (a, more)
}

/// [`sweep_answer_in_ram`]'s answer written back.
fn write_sweep_answer(ee: &mut Ee, rec: u32, count: usize, items: &[u32], a: &game_sim::sweep_probe::Answer, more: &atv_sweep::More) {
    use f51_rec as ra;
    let base = |k: Option<usize>| k.map_or(0, |k| items[k] - 0x18);
    set_vf(ee, rec, a.head[0]);
    set_vf(ee, rec + 0x10, a.head[1]);
    let r = &a.record;
    set_vf(ee, rec + ra::NORMAL, r.nearest_normal);
    set_vf(ee, rec + ra::POINT, r.nearest_point);
    set_vf(ee, rec + ra::PUSH, r.push);
    ee.set_u32(rec + ra::HITS, r.hits as u32);
    ee.set_u32(rec + ra::NEAREST, r.nearest.0.to_bits());
    ee.set_u32(rec + ra::PART, base(more.part));
    let [normals, points, struck, owners, hits, fractions, words] = [ra::NORMALS, ra::POINTS, ra::STRUCK, ra::OWNERS, ra::FLAGS, ra::FRACTIONS, ra::WORDS].map(|o| ee.u32_at(rec + o));
    for i in 0..count {
        let iu = i as u32;
        set_vf(ee, normals + 0x10 * iu, r.normal[i]);
        set_vf(ee, points + 0x10 * iu, r.point[i]);
        ee.set_u32(owners + 4 * iu, r.owner[i] as u32);
        ee.set_u32(hits + 4 * iu, u32::from(r.hit[i]));
        ee.set_u32(fractions + 4 * iu, r.fraction[i].0.to_bits());
        ee.set_u32(words + 4 * iu, more.words[i]);
        ee.set_u32(struck + 4 * iu, base(a.struck[i]));
    }
}

/// **F51 — the global search in Rust**: the sweep probe `probe` against the database world
/// ([`atv_sweep::search_world`], the narrow phase [`atv_sweep::Sweep`] over [`SweepWorldInRam`]),
/// called `lazy`, and everything written back: the query's walk (F47's [`GatherInRam::write_query`]),
/// the flags and groups, the budget and last centre, the clear's world form and globals, the pairs'
/// globals, local forms and answer. Answers the nearest distance; counts the pairs by kind into
/// `kinds`. `None` (nothing written back) when a listener handed on searched the database itself:
/// the original's own walk goes on from what that search left, which the port's cannot follow.
fn sweep_all_in_ram(ee: &mut Ee, probe: u32, lazy: u32, kinds: &mut BTreeMap<String, u32>) -> Option<F> {
    use f51_probe as pa;
    let (m, mut objects, mut broad) = GatherInRam::read(ee, probe);
    broad.places[0].budget = word_f(ee, probe + 0x74);
    broad.places[0].last = [0, 4, 8, 12].map(|i| word_f(ee, probe + 0x60 + i));
    let mut items = m.items.clone();
    // The plant groups' parts, as collidables of their own.
    for k in 0..objects.len() {
        if objects[k].class != ws::Class::Group {
            continue;
        }
        for p in PartWorldInRam::species_parts(ee, items[k]) {
            if p != 0 && !items.contains(&(p + 0x18)) {
                items.push(p + 0x18);
                objects.push(ws::Collidable { class: ws::Class::from_byte(ee.u8_at(p + 0x18 + 2)), flags: ee.u8_at(p + 0x126), ..Default::default() });
                broad.places.push(bp::Placement::default());
            }
        }
    }
    let segments = sweep_segments_in_ram(ee, probe);
    let count = segments.len();
    let rec = ee.u32_at(probe + pa::RECORD);
    let (answer, more) = sweep_answer_in_ram(ee, rec, count, &mut items);
    let mut port = SweepProbe { segments: segments.clone(), answer, ..SweepProbe::default() };
    sweep_local_in_ram(ee, probe, count, &mut port);
    let mut stood = port.clone();
    sweep_probe_clear(&mut stood);
    let retired: Vec<bool> = items.iter().map(|&e| ee.u32_at(e - 0x18) == 1).collect();
    let b = atv_sweep::bounds(&segments);
    let mut world = PartWorldInRam { ee: &mut *ee, items, mesh: None, kinds: BTreeMap::new(), listeners: vec![], callbacks: vec![] };
    for (k, o) in objects.iter_mut().enumerate() {
        if o.class == ws::Class::Object {
            let e = world.items[k];
            world.hear(o, e - 0x18);
        }
    }
    let was_o = objects.clone();
    let was_b = broad.clone();
    let stamp = world.ee.u16_at(STAMP_AT);
    let mut outside = SweepWorldInRam { world: &mut world, centre: stood.world.centre, radius: stood.world.radius, pairs: 0, descriptor: None };
    let mut sweep = atv_sweep::Sweep::new(segments, more, &mut outside);
    sweep.probe = port;
    let nearest = atv_sweep::search_world(&mut objects, &mut broad, 0, lazy, &|o| retired.get(o).copied().unwrap_or(false), &mut sweep);
    let (port, more) = (sweep.probe, sweep.more);
    let (pairs, descriptor) = (outside.pairs, outside.descriptor);
    for (k, v) in std::mem::take(&mut world.kinds) {
        *kinds.entry(k).or_default() += v;
    }
    let items = world.items;
    if ee.u16_at(STAMP_AT) != stamp {
        return None;
    }
    // The clear ran (the search got past its budget): the world form is the one it stands.
    let cleared = port.world == stood.world;
    // What the walk may change beyond flags and stamps: the probe's reach and place, the parts' groups.
    let mut wo = was_o.clone();
    let mut wb = was_b.clone();
    wo[0].radius = objects[0].radius;
    let p = &broad.places[0];
    (wb.places[0].centre, wb.places[0].budget, wb.places[0].last) = (p.centre, p.budget, p.last);
    for (k, o) in objects.iter().enumerate() {
        wo[k].group = o.group;
        if let Some(g) = o.group {
            ee.set_u32(items[k] - 0x18 + 0x88, items[g]);
        }
    }
    // The first query begun as the search begins it, on a copy.
    let mut begun = was_b.clone();
    if cleared {
        let space = begun.places[0].space.expect("a searching probe in a space");
        let (mut lo, mut hi) = ([F::ZERO; 4], [F::ZERO; 4]);
        bp::Query::begin(&mut begun, space, 1, b.centre.0.map(F), b.half.0.map(F), Some((&mut lo, &mut hi)), true);
    }
    let m = GatherInRam { items: items.clone(), ..m };
    m.write_query(ee, (&wo, &wb), (&objects, &broad), &begun);
    ee.set_u32(probe + 0x74, broad.places[0].budget.0.to_bits());
    for (i, w) in broad.places[0].last.iter().enumerate() {
        ee.set_u32(probe + 0x60 + 4 * i as u32, w.0.to_bits());
    }
    if cleared {
        // Its world form, sphere and globals, then the box's centre and reach.
        let w = &port.world;
        let [lengths, starts, dirs] = [pa::LENGTHS, pa::ORIGINS, pa::DIRS].map(|o| ee.u32_at(probe + o));
        for (i, s) in w.probes[..w.count].iter().enumerate() {
            let i = i as u32;
            ee.set_u32(lengths + 4 * i, s.length.0.to_bits());
            set_vf(ee, starts + 0x10 * i, s.origin);
            set_vf(ee, dirs + 0x10 * i, s.dir);
        }
        set_vf(ee, probe + pa::CENTRE, w.centre);
        ee.set_u32(probe + pa::RADIUS, w.radius.0.to_bits());
        set_vf(ee, probe + f51_at::CENTRE, b.centre);
        ee.set_u32(probe + f51_at::RADIUS, b.reach.0.to_bits());
        for a in SWEEP_ZEROED {
            ee.set_u32(a, 0);
        }
        ee.set_u32(SWEEP_RECORD_AT, rec);
    }
    sweep_pairs_write(ee, probe, count, &port, pairs, descriptor);
    write_sweep_answer(ee, rec, count, &items, &port.answer, &more);
    Some(nearest)
}

/// A segment for F51's searches: one in three anywhere round `home` (60 or 400 ft out, up to 4 ft
/// long), else through a triangle of one of `meshes` (starting before it, or past it); one in four
/// with a stray `w`.
fn f51_segment(g: &mut Gen, ee: &Ee, meshes: &[u32], home: Vf) -> [Vf; 2] {
    let (from, to) = if g.one_in(3) {
        let reach = if g.one_in(2) { 60.0 } else { 400.0 };
        let p = [g.f(-reach, reach), g.f(-20.0, 20.0), g.f(-reach, reach)];
        let d = f51_unit(g).map(|x| x * g.f(0.0, 4.0));
        ([0, 1, 2].map(|i| home.0[i] + p[i]), d)
    } else {
        let c = meshes[(g.bits() as usize) % meshes.len()];
        let mesh = SweptMesh::read(ee, c);
        let t = &mesh.triangles[(g.bits() as usize) % mesh.triangles.len()];
        let m = f51_transform(ee, c);
        let k = mesh.mesh().corners(t).map(|v| f51_place(&m, [v.0[0], v.0[1], v.0[2]]));
        let (u, v) = (g.f(0.0, 0.5), g.f(0.0, 0.5));
        let at: [f32; 3] = std::array::from_fn(|i| k[0][i] + (k[1][i] - k[0][i]) * u + (k[2][i] - k[0][i]) * v);
        let dir = f51_unit(g);
        let length = g.f(0.01, 4.0);
        let before = length * g.f(-0.2, 1.2);
        (std::array::from_fn(|i| at[i] - dir[i] * before), dir.map(|x| x * length))
    };
    let w = if g.one_in(4) { g.f(-2.0, 2.0) } else { 1.0 };
    [Vf::new(from[0], from[1], from[2], w), Vf::new(from[0] + to[0], from[1] + to[1], from[2] + to[2], w)]
}

/// The global search against the original's (F51): the record's sweep probe set (the original's set
/// and place) on a segment through a triangle of one of nat05's meshes, or anywhere round the bike out
/// to 400 ft, and searched, one RAM by the original and one by [`sweep_all_in_ram`]; the answers alike
/// and the two RAMs alike after every search outside the stack. Now and then the probe is called lazy
/// (its budget left from the search before), made inactive, or let search the plants (`NO_GROUPS`
/// down: their parts stood by the original, their spheres paired).
#[test]
fn the_sweeps_global_search_is_the_originals() {
    let Some(ee) = image() else { return };
    let mut a = hooked(ee);
    let mut b = hooked(image().unwrap());
    b.ram.clone_from(&a.ram);
    let probe = a.u32_at(a.u32_at(BIKE + seed::RECORD) + 0x204);
    assert_eq!(a.u32_at(probe + f51_probe::COUNT), 1, "the record's sweep has one segment");
    let (m, world, _) = GatherInRam::read(&a, probe);
    let meshes: Vec<u32> = (1..world.len())
        .filter(|&k| world[k].class == ws::Class::Object)
        .map(|k| m.items[k] - 0x18)
        .filter(|&c| a.u32_at(c + f51_at::KIND) == 0 && a.u32_at(c + f51_at::TRIANGLES) != 0)
        .collect();
    let home = vf_at(&a, probe + f51_at::CENTRE);
    let flags = a.u8_at(probe + f51_at::FLAGS);
    let points = F51_AREA;
    let mut g = Gen(0x0f51_0a11_5eed_0002);
    let mut kinds = BTreeMap::<String, u32>::new();
    let mut seen = BTreeMap::<&str, u32>::new();
    for n in 0..1500 {
        let ends = f51_segment(&mut g, &a, &meshes, home);
        let lazy = u32::from(g.one_in(4));
        let mut byte = flags;
        if g.one_in(15) {
            byte &= !bp::more_flags::ACTIVE;
        }
        if g.one_in(4) {
            byte &= !ws::flags::NO_GROUPS;
        }
        for ee in [&mut a, &mut b] {
            set_vf(ee, points, ends[0]);
            set_vf(ee, points + 0x10, ends[1]);
            set_u8(ee, probe + f51_at::FLAGS, byte & !ws::flags::HIT | flags & ws::flags::HIT);
            ee.set(28, GP);
            ee.call(SWEEP_SET, &[u64::from(probe), u64::from(points), 0], &[], BUDGET);
            ee.set(28, GP);
            ee.call(PROBE_PLACE, &[u64::from(probe)], &[], BUDGET);
        }
        let off = bytes_differ(&a.ram, &b.ram, a.stack);
        assert!(off.is_empty(), "search {n}: the set and place left the RAMs apart at {off:?}");
        a.set(28, GP);
        a.call(SEARCH_ALL, &[u64::from(probe), u64::from(lazy)], &[], BUDGET);
        let theirs = a.f0();
        let ctx = format!("search {n} (lazy {lazy}, flags {byte:#x})");
        let Some(ours) = sweep_all_in_ram(&mut b, probe, lazy, &mut kinds) else {
            // A struck plant's listener searched the database mid-walk: not followed (F51's journal).
            *seen.entry("reentered").or_default() += 1;
            b.ram.clone_from(&a.ram);
            continue;
        };
        assert_eq!(theirs.to_bits(), ours.0.to_bits(), "{ctx}: the answer ({theirs} theirs, {} ours)", ours.0);
        let off = bytes_differ(&a.ram, &b.ram, a.stack);
        assert!(off.is_empty(), "{ctx}: RAM differs at {} ranges\n  {}", off.len(), off.iter().take(40).cloned().collect::<Vec<_>>().join("\n  "));
        let hit = a.u8_at(probe + f51_at::FLAGS) & ws::flags::HIT != 0;
        *seen.entry(if byte & bp::more_flags::ACTIVE == 0 { "inactive" } else if theirs == f32::MAX { "max" } else if hit { "hit" } else if lazy != 0 { "lazy" } else { "clear" }).or_default() += 1;
    }
    eprintln!("F51 global: {seen:?}, pairs {kinds:?}");
    assert!(seen.get("hit").copied().unwrap_or(0) >= 300, "the searches hit: {seen:?}");
    assert!(seen.get("inactive").copied().unwrap_or(0) >= 50, "{seen:?}");
    assert!(kinds.keys().any(|k| k.contains("part stand")), "no plant was searched: {kinds:?}");
}

/// The nearby sweep with its pairs against the original's (F51; C15's sweeps pair nothing): the
/// record's segment set and placed and the record's nearby list gathered round the sweep probe (the
/// original's, on both RAMs), then searched over the list, one RAM by the original and one by
/// [`sweep_in_ram`] (its set and place again, then the search); the hit bits alike and the RAMs alike
/// outside the stack.
#[test]
fn the_nearby_sweep_with_its_pairs_is_the_originals() {
    let Some(ee) = image() else { return };
    let mut a = hooked(ee);
    let mut b = hooked(image().unwrap());
    b.ram.clone_from(&a.ram);
    let rec = a.u32_at(BIKE + seed::RECORD);
    let (probe, list) = (a.u32_at(rec + 0x204), a.u32_at(rec + 0xfc));
    let (m, world, _) = GatherInRam::read(&a, probe);
    let meshes: Vec<u32> = (1..world.len())
        .filter(|&k| world[k].class == ws::Class::Object)
        .map(|k| m.items[k] - 0x18)
        .filter(|&c| a.u32_at(c + f51_at::KIND) == 0 && a.u32_at(c + f51_at::TRIANGLES) != 0)
        .collect();
    let home = vf_at(&a, probe + f51_at::CENTRE);
    let flags = a.u8_at(probe + f51_at::FLAGS);
    let mut g = Gen(0x0f51_0a11_5eed_0003);
    let mut kinds = BTreeMap::<String, u32>::new();
    let mut seen = BTreeMap::<&str, u32>::new();
    for n in 0..600 {
        let ends = f51_segment(&mut g, &a, &meshes, home);
        let mut byte = flags;
        if g.one_in(4) {
            byte &= !ws::flags::NO_GROUPS;
        }
        for ee in [&mut a, &mut b] {
            set_vf(ee, rec + 0xc0, ends[0]);
            set_vf(ee, rec + 0xd0, ends[1]);
            set_u8(ee, probe + f51_at::FLAGS, byte);
            ee.set(28, GP);
            ee.call(SWEEP_SET, &[u64::from(probe), u64::from(rec + 0xc0), 0], &[], BUDGET);
            ee.set(28, GP);
            ee.call(PROBE_PLACE, &[u64::from(probe)], &[], BUDGET);
            ee.set(28, GP);
            ee.call(GATHER, &[u64::from(list), u64::from(probe), 3], &[0.0], BUDGET);
        }
        let ctx = format!("sweep {n} (flags {byte:#x}, {} nearby)", a.u32_at(list + 4));
        let off = bytes_differ(&a.ram, &b.ram, a.stack);
        assert!(off.is_empty(), "{ctx}: the set, place and gather left the RAMs apart at {off:?}");
        a.set(28, GP);
        a.call(SEARCH_NEAR, &[u64::from(probe), u64::from(list), 1], &[], BUDGET);
        let stamp = b.u16_at(STAMP_AT);
        let ours = sweep_in_ram(&mut b, rec, probe, list, ends[0], ends[1], &mut kinds);
        let theirs = a.u8_at(probe + f51_at::FLAGS) & ws::flags::HIT != 0;
        if b.u16_at(STAMP_AT) != stamp {
            // A struck plant's listener searched the database (as in the global search's test).
            *seen.entry("reentered").or_default() += 1;
            b.ram.clone_from(&a.ram);
            continue;
        }
        assert_eq!(theirs, ours, "{ctx}: the hit");
        let off = bytes_differ(&a.ram, &b.ram, a.stack);
        assert!(off.is_empty(), "{ctx}: RAM differs at {} ranges\n  {}", off.len(), off.iter().take(40).cloned().collect::<Vec<_>>().join("\n  "));
        *seen.entry(if theirs { "hit" } else if a.u32_at(list + 4) > 0 { "missed" } else { "empty" }).or_default() += 1;
    }
    eprintln!("F51 nearby: {seen:?}, pairs {kinds:?}");
    assert!(seen.get("hit").copied().unwrap_or(0) >= 150, "the sweeps hit: {seen:?}");
    assert!(seen.get("missed").copied().unwrap_or(0) >= 100, "{seen:?}");
}
