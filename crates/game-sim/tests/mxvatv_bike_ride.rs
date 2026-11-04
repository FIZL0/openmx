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
use ps2_float::F;

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
