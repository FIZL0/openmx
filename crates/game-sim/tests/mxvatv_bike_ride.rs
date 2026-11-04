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
    ee.hooks.insert(0x004e_7e70, Box::new(|_: &mut Ee| {}));
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
/// port's ([`port_record`]). Answers the Force body and PostProcess counts, how many records the port
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
