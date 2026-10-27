//! S56 — the rider's ragdoll through whole wrecks of the original, tick by tick.
//!
//! Four wrecks were recorded off the human's ride on Free09 (recording2's save state, the pad driven
//! by `research/s56/*.pad`): over the bars on a landing (`front`, reason 1), a side crash at speed
//! (`side`, reason 2), a loop-out backwards (`back`, reason 11) and a tip-over (`tip`, reason 3). Each
//! capture (`watch/s56-ragdoll.toml`, one row a tick) carries every bone's body, the actor's wreck
//! fields, the holds, the skeleton's rests and the whole riding request; each has save states from the
//! tick the wreck starts, which `research/s56_state_check.py` proved equal to their rows word for word.
//!
//! The RAM images (`./kit state ram context/capture/s56/<name>-<frame>.p2s --out
//! context/capture/s56/<name>-<frame>.ee.bin`) and scratchpads (the state's `Scratchpad.bin`, as
//! `<name>-<frame>.spr.bin`) are what a replay starts from.

use game_sim::rng::Rng;
use ps2_float::Vf;
use ps2_oracle::{Ee, Kit, addr, skip};
use std::collections::HashMap;
use std::path::PathBuf;

/// The rider body actor in every s56 state (the human's vehicle less 0x400).
const ACTOR_S56: u32 = 0x017a_7700;

std::thread_local! {
    /// The rider body actor the helpers read: S56's, or a 2p rider's ([`with_actor`]).
    static ACTOR_AT: std::cell::Cell<u32> = const { std::cell::Cell::new(ACTOR_S56) };
}

/// The rider body actor the helpers below read and the original's update is called on.
fn body_actor() -> u32 {
    ACTOR_AT.with(|a| a.get())
}

/// Points the helpers at another rider body actor (a 2p rider's) on this test's thread.
fn with_actor(a: u32) {
    ACTOR_AT.with(|c| c.set(a));
}
const PARTS: u32 = 0x2f4;

/// One wreck: its capture's name, and the frame of its state.
struct Wreck {
    name: &'static str,
    frame: u32,
}

/// A state of each wreck from its first ticks: side's is the tick before the entry (the reason set,
/// `riding` still set); the others' saves landed a tick or two into the wreck.
const WRECKS: [Wreck; 4] = [
    Wreck { name: "front", frame: 2600 },
    Wreck { name: "side", frame: 2604 },
    Wreck { name: "back", frame: 2475 },
    Wreck { name: "tip", frame: 2475 },
];

/// Where an exploration writes what it found (a passing check keeps no log).
fn note(text: &str) {
    use std::io::Write;
    let Ok(kit) = Kit::load() else { return };
    let path = kit.root.join("context/capture/s56/notes.txt");
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path).expect("the notes");
    writeln!(f, "{text}").expect("write the notes");
}

fn ctx(rel: &str) -> Option<PathBuf> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let local = kit.root.join("context").join(rel);
    let path = if local.exists() { local } else { kit.ctx(rel) };
    if !path.exists() {
        skip(&format!("{} is not there (S56's captures: research/s56_wreck.py)", path.display()));
        return None;
    }
    Some(path)
}

/// A capture: its header's columns and its rows by frame counter.
struct Capture {
    cols: HashMap<String, usize>,
    rows: HashMap<u32, Vec<String>>,
}

impl Capture {
    fn load(name: &str) -> Option<Capture> {
        Capture::load_in("s56", name)
    }

    /// A capture of `context/capture/<dir>`.
    fn load_in(dir: &str, name: &str) -> Option<Capture> {
        let text = std::fs::read_to_string(ctx(&format!("capture/{dir}/{name}.csv"))?).expect("read the capture");
        let mut lines = text.lines();
        let cols: HashMap<String, usize> =
            lines.next().expect("a header").split(',').enumerate().map(|(i, c)| (c.to_string(), i)).collect();
        let fc = cols["frame_counter"];
        let rows = lines
            .map(|l| l.split(',').map(str::to_string).collect::<Vec<_>>())
            .map(|r| (r[fc].parse().expect("a frame counter"), r))
            .collect();
        Some(Capture { cols, rows })
    }

    fn text(&self, frame: u32, col: &str) -> &str {
        &self.rows[&frame][*self.cols.get(col).unwrap_or_else(|| panic!("no column {col}"))]
    }

    fn get(&self, frame: u32, col: &str) -> f32 {
        let s = self.text(frame, col);
        s.parse::<f64>().unwrap_or_else(|_| panic!("{col} at {frame}: {s:?}")) as f32
    }

    fn int(&self, frame: u32, col: &str) -> u64 {
        let s = self.text(frame, col);
        s.parse().unwrap_or_else(|_| panic!("{col} at {frame}: {s:?}"))
    }

    fn quad(&self, frame: u32, col: &str) -> Vf {
        Vf::new(
            self.get(frame, &format!("{col}.x")),
            self.get(frame, &format!("{col}.y")),
            self.get(frame, &format!("{col}.z")),
            self.get(frame, &format!("{col}.w")),
        )
    }

    fn bytes(&self, frame: u32, col: &str) -> Vec<u8> {
        let s = self.text(frame, col);
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex")).collect()
    }

    fn has(&self, frame: u32) -> bool {
        self.rows.contains_key(&frame)
    }
}

fn state(w: &Wreck) -> Option<Ee> {
    state_in("s56", w)
}

fn state_in(dir: &str, w: &Wreck) -> Option<Ee> {
    let path = ctx(&format!("capture/{dir}/{}-{}.ee.bin", w.name, w.frame))?;
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    if let Some(spr) = ctx(&format!("capture/{dir}/{}-{}.spr.bin", w.name, w.frame)) {
        ee.spr = std::fs::read(spr).expect("the scratchpad");
    }
    assert_eq!(ee.u32_at(body_actor() + 0x2e0), 18, "no rider body actor at {:#x}", body_actor());
    assert_eq!(ee.u32_at(addr("frame_counter").unwrap()), w.frame, "the state is not frame {}", w.frame);
    Some(ee)
}

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| ee.f32_at(a + k)))
}

fn bone_body(ee: &Ee, i: usize) -> u32 {
    ee.u32_at(ee.u32_at(body_actor() + PARTS) + 0xb0 * i as u32 + 0xa4)
}

/// What the capture holds of each bone's body, by column and offset.
const BODY: [(&str, u32); 6] =
    [("last_pos", 0x50), ("pos", 0x60), ("vel", 0x70), ("last_quat", 0x150), ("quat", 0x170), ("rates", 0x1b0)];

/// Each bone's captured lanes that differ from `bones` (per bone, its six quads), as names.
fn differ(cap: &Capture, frame: u32, bones: &[[Vf; 6]]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, b) in bones.iter().enumerate() {
        for (k, (name, _)) in BODY.iter().enumerate() {
            let want = cap.quad(frame, &format!("bone{i}.{name}"));
            for l in 0..4 {
                if want[l].to_bits() != b[k][l].to_bits() {
                    out.push(format!("bone{i}.{name}[{l}] {:e} want {:e}", b[k][l], want[l]));
                }
            }
        }
    }
    out
}

fn ee_bones(ee: &Ee) -> Vec<[Vf; 6]> {
    (0..18).map(|i| BODY.map(|(_, o)| quad(ee, bone_body(ee, i) + o))).collect()
}

fn set_bytes(ee: &mut Ee, a: u32, bytes: &[u8]) {
    let at = (a & 0x01ff_ffff) as usize;
    ee.ram[at..at + bytes.len()].copy_from_slice(bytes);
}

/// What the body's update reads off the rest of the game in tick `frame`, laid into `ee` from the
/// capture: the riding request (and the hand dummies 0x200 before it) and the bike centre its +0x1ac
/// points at. An s62 capture carries the wreck bike's whole object round that pointer too (S62:
/// `bike_a` from 0x980 before it, `bike_b` from 0x680 after it), and the broad phase's pools.
fn lay_in(ee: &mut Ee, cap: &Capture, frame: u32) {
    lay_in_as(ee, cap, frame, true);
}

/// [`lay_in`], with the wreck bike put back in the tree ([`place_bike`]) or, `placed` false, left
/// where the first state had it (S62's replay before S113).
fn lay_in_as(ee: &mut Ee, cap: &Capture, frame: u32, placed: bool) {
    let req = ee.u32_at(body_actor() + 0x488);
    set_bytes(ee, req - 0x200, &cap.bytes(frame, "req_area"));
    let far = ee.u32_at(req + 0x1ac);
    if cap.cols.contains_key("bike_a") {
        set_bytes(ee, far - 0x980, &cap.bytes(frame, "bike_a"));
        set_bytes(ee, far + 0x680, &cap.bytes(frame, "bike_b"));
    }
    set_bytes(ee, far, &cap.bytes(frame, "req_far"));
    for (col, _) in cap.cols.iter().filter(|(c, _)| c.starts_with('w') && c.len() == 8) {
        let at = u32::from_str_radix(&col[1..], 16).expect("a world block's address");
        if (0x0179_0000..0x0181_0000).contains(&at) {
            continue;
        }
        set_bytes(ee, at, &cap.bytes(frame, col));
    }
    if placed && cap.cols.contains_key("bike_a") {
        place_bike(ee);
    }
}

/// The wreck bike put in the track's tree where the game's own frame keeps it (S113). Laying the
/// capture in moves the bike but not its place in the tree: its placement record (`B+4`, not laid
/// in) keeps the first state's key, and the pools laid in take it out of its old node once the game
/// has moved it (front, from 2619) without the node the game moved it into. The bones' search is
/// then built off a tree that does not hold the bike, and from the first rebuild after that (front's
/// 2628) the Ee pairs no bone with it while the game's bones hit it. So, every row with the bike
/// placed: its box by its own bounds (`mesh_collidable_bounds`), its key by the tree's (vt `+0x20`,
/// as the world's add asks it), the record's key and centre set to them, and the bike inserted
/// under that key (vt `+0x24`) unless the tree holds it there already. Once the put-back has taken
/// it out of the broad phase (`B+4` 0, S108), out of every node of the track's tree too, as the
/// game's unwreck, which the replay does not run, leaves it.
fn place_bike(ee: &mut Ee) {
    let b = ee.u32_at(ee.u32_at(body_actor() + 0x488) + 0x1ac);
    let place = ee.u32_at(b + 4);
    if place == 0 {
        let sphere = ee.u32_at(collision(ee, 0) + coll_at::SPHERE);
        let tree = ee.u32_at(ee.u32_at(ee.u32_at(sphere + 4) + 4) + 0x34);
        unplace(ee, ee.u32_at(tree + 0x94), b);
        return;
    }
    let tree = ee.u32_at(place + 8);
    let (centre, half) = (ee.scratch(0), ee.scratch(1));
    ee.call(addr("mesh_collidable_bounds").unwrap(), &[u64::from(b), u64::from(centre), u64::from(half)], &[], 1_000_000);
    let vt = ee.u32_at(tree + 8);
    ee.call(ee.u32_at(vt + 0x20), &[u64::from(tree), u64::from(centre), u64::from(half), 0], &[], 1_000_000);
    let key = ee.r(2) as u32;
    if !tree_holds_at(ee, tree, b, key) {
        ee.call(ee.u32_at(vt + 0x24), &[u64::from(tree), u64::from(b), u64::from(key), u64::from(centre), u64::from(half)], &[], 1_000_000);
    }
    ee.set_u32(place + 0xc, key);
    for k in 0..4 {
        let w = ee.u32_at(centre + 4 * k);
        ee.set_u32(place + 0x10 + 4 * k, w);
    }
}

/// Every slot holding `o` in the chunks of the tree under `node` emptied.
fn unplace(ee: &mut Ee, node: u32, o: u32) {
    let ram = |a: u32| (0x10_0000..0x200_0000).contains(&a) && a % 4 == 0;
    if !ram(node) {
        return;
    }
    let mut c = ee.u32_at(node + 4);
    while c != 0 && ram(c) {
        for k in 1..5 {
            if ee.u32_at(c + 4 * k) == o {
                ee.set_u32(c + 4 * k, 0);
            }
        }
        c = ee.u32_at(c);
    }
    let children = ee.u32_at(node);
    if ram(children) {
        for i in 0..4 {
            unplace(ee, ee.u32_at(children + 4 * i), o);
        }
    }
}

/// Whether every node `key` names (a corner bit each, `broad_tree_insert`'s walk from bit `0x4000`
/// down `level + 1` steps) is in the tree and holds `o` in its chunks.
fn tree_holds_at(ee: &Ee, tree: u32, o: u32, key: u32) -> bool {
    let level = key & 0xf;
    let (x, z) = ((key >> 17) & 0x7ff8, (key >> 5) & 0x7ff8);
    let size = 0x4000 >> level;
    (0..4).filter(|c| key & 0x10 << c != 0).all(|c| {
        let (cx, cz) = (x + size * (c >> 1), z + size * (c & 1));
        let mut node = ee.u32_at(tree + 0x94);
        for l in 0..=level {
            let bit = 0x4000 >> l;
            let children = ee.u32_at(node);
            if node == 0 || children == 0 {
                return false;
            }
            node = ee.u32_at(children + 4 * (2 * u32::from(cx & bit != 0) + u32::from(cz & bit != 0)));
            if node == 0 {
                return false;
            }
        }
        let mut chunk = ee.u32_at(node + 4);
        while chunk != 0 {
            if (1..5).any(|k| ee.u32_at(chunk + 4 * k) == o) {
                return true;
            }
            chunk = ee.u32_at(chunk);
        }
        false
    })
}

/// Whether the recorder saw tick `frame - 1` again under the next frame counter (a save state's pause
/// can land between the counter's step and the simulation's): no tick ran between the two rows.
fn repeated(cap: &Capture, frame: u32) -> bool {
    cap.int(frame, "rng_state") == cap.int(frame - 1, "rng_state")
        && cap.quad(frame, "bone0.pos") == cap.quad(frame - 1, "bone0.pos")
}

fn rng_at(ee: &Ee) -> u64 {
    let at = addr("rand_state").unwrap();
    u64::from(ee.u32_at(at)) | u64::from(ee.u32_at(at + 4)) << 32
}

fn set_rng(ee: &mut Ee, state: u64) {
    set_bytes(ee, addr("rand_state").unwrap(), &state.to_le_bytes());
}

/// The draws between two states of the generator, if `to` is `from` advanced fewer than `most` times.
fn draws(from: u64, to: u64, most: u32) -> Option<u32> {
    let mut r = Rng::from_state(from);
    (0..most).find(|_| {
        let hit = r.state() == to;
        r.rand();
        hit
    })
}

/// Everything a call can change, to run a tick again from the same start.
struct Snapshot {
    ram: Vec<u8>,
    spr: Vec<u8>,
    gpr: [u128; 32],
    fpr: [u32; 32],
    vf: [[f32; 4]; 32],
    vacc: [f32; 4],
    q: f32,
    acc: u32,
}

impl Snapshot {
    fn of(ee: &Ee) -> Snapshot {
        Snapshot {
            ram: ee.ram.clone(),
            spr: ee.spr.clone(),
            gpr: ee.gpr,
            fpr: ee.fpr,
            vf: ee.vf,
            vacc: ee.vacc,
            q: ee.q,
            acc: ee.acc,
        }
    }

    fn back(&self, ee: &mut Ee) {
        ee.ram.copy_from_slice(&self.ram);
        ee.spr.copy_from_slice(&self.spr);
        (ee.gpr, ee.fpr, ee.vf, ee.vacc, ee.q, ee.acc) = (self.gpr, self.fpr, self.vf, self.vacc, self.q, self.acc);
    }
}

/// One tick of the original's own update in the oracle, with the bike's side laid in from the capture,
/// checked against every captured bone. The rest of the game draws random numbers in the same tick,
/// before the body and after it; where the body draws, the draws before it are found by trying each
/// count in turn from the previous row's state. `Ok` holds that count and the body's own draws.
fn originals_tick(ee: &mut Ee, cap: &Capture, frame: u32) -> Result<(u32, u32), Vec<String>> {
    originals_tick_as(ee, cap, frame, true)
}

/// [`originals_tick`] with the capture laid in by [`lay_in_as`].
fn originals_tick_as(ee: &mut Ee, cap: &Capture, frame: u32, placed: bool) -> Result<(u32, u32), Vec<String>> {
    let update = addr("rider_body_update").unwrap();
    lay_in_as(ee, cap, frame, placed);
    let before = cap.int(frame - 1, "rng_state");
    let snap = Snapshot::of(ee);
    let mut first = None;
    for n in 0..64 {
        snap.back(ee);
        let mut r = Rng::from_state(before);
        (0..n).for_each(|_| {
            r.rand();
        });
        set_rng(ee, r.state());
        ee.call(update, &[u64::from(body_actor())], &[cap.get(frame, "dt")], 2_000_000_000);
        let drew = draws(r.state(), rng_at(ee), 1000).expect("the body's draws");
        let d = differ(cap, frame, &ee_bones(ee));
        if d.is_empty() {
            return Ok((n, drew));
        }
        first.get_or_insert(d);
        if drew == 0 {
            break; // the body drew nothing: the generator is not the difference
        }
    }
    snap.back(ee);
    Err(first.unwrap())
}

/// How far each wreck's state replays exactly through the original's own update with what the capture
/// lays in: up to the tick the rider and the bike (or another moving collidable the capture does not
/// carry) first touch. The tick after each is the first the replay misses.
const EXACT_TO: [(&str, u32); 4] = [("front", 2607), ("side", 2612), ("back", 2490), ("tip", 2481)];

/// The oracle reproduces the original's ragdoll tick for tick, bit for bit, from each wreck's state:
/// the entry with its six draws (side's state is the tick before it), the steps after, the duplicate
/// rows a save state leaves skipped. What the port is to be held against.
#[test]
fn the_originals_update_replays_each_wreck() {
    for (w, (name, to)) in WRECKS.iter().zip(EXACT_TO) {
        assert_eq!(w.name, name);
        let Some(cap) = Capture::load(w.name) else { return };
        let Some(mut ee) = state(w) else { return };
        let mut entry = None;
        for frame in w.frame + 1..=to {
            if repeated(&cap, frame) {
                continue;
            }
            match originals_tick(&mut ee, &cap, frame) {
                Ok((_, 6)) => entry = Some(frame),
                Ok(_) => {}
                Err(d) => panic!("{name}: tick {frame} parts in {} lanes: {:?}", d.len(), &d[..d.len().min(4)]),
            }
        }
        if name == "side" {
            assert_eq!(entry, Some(w.frame + 1), "side: the wreck entry's six draws");
        }
    }
}

/// The original's own update through each whole capture, from the wreck's first tick and from the
/// one after.
#[test]
#[ignore = "exploration"]
fn the_originals_update_replays_the_capture() {
    for w in WRECKS.iter().flat_map(|w| [Wreck { name: w.name, frame: w.frame }, Wreck { name: w.name, frame: w.frame + 1 }]) {
        let Some(cap) = Capture::load(w.name) else { return };
        let Some(mut ee) = state(&w) else { return };
        let mut frame = w.frame + 1;
        let mut log = Vec::new();
        let mut parted = None;
        while cap.has(frame) {
            if repeated(&cap, frame) {
                log.push(format!("{frame}: the row repeats {}", frame - 1));
                frame += 1;
                continue;
            }
            match originals_tick(&mut ee, &cap, frame) {
                Ok((n, drew)) if drew > 0 => log.push(format!("{frame}: {n} draws before, {drew} by the body")),
                Ok(_) => {}
                Err(d) => {
                    parted = Some(d);
                    break;
                }
            }
            frame += 1;
        }
        for l in &log {
            note(&format!("  {} {l}", w.name));
        }
        match parted {
            Some(d) => note(&format!(
                "{} from {}: exact to {}, {frame} parts in {} lanes {:?}",
                w.name,
                w.frame,
                frame - 1,
                d.len(),
                d.iter().take(4).collect::<Vec<_>>()
            )),
            None => note(&format!("{} from {}: exact through {}", w.name, w.frame, frame - 1)),
        }
    }
}

/// Exploration: what the original's update reads, over every tick of every capture, that the game
/// changes from tick to tick (the bytes consecutive states differ in) and that is not the actor's own
/// or laid in already: the ranges a capture has to carry for a replay to see the moving world.
#[test]
#[ignore = "exploration"]
fn what_the_wrecks_read() {
    let mut changing = std::collections::BTreeSet::new();
    for (name, a, b) in [("front", 2600, 2601), ("side", 2604, 2605), ("back", 2475, 2476), ("tip", 2475, 2476)] {
        let read = |f: u32| std::fs::read(ctx(&format!("capture/s56/{name}-{f}.ee.bin")).unwrap()).unwrap();
        let (ra, rb) = (read(a), read(b));
        changing.extend((0..ra.len()).filter(|&i| ra[i] != rb[i]).map(|i| i as u32));
    }
    let mut wanted = std::collections::BTreeSet::new();
    for w in &WRECKS {
        let Some(cap) = Capture::load(w.name) else { return };
        let Some(mut ee) = state(w) else { return };
        let req = ee.u32_at(body_actor() + 0x488);
        let far = ee.u32_at(req + 0x1ac);
        let mut frame = w.frame + 1;
        let mut parted = None;
        while cap.has(frame) {
            *ee.reads.borrow_mut() = Some(Default::default());
            if originals_tick(&mut ee, &cap, frame).is_err() {
                parted.get_or_insert(frame);
                set_rng(&mut ee, cap.int(frame - 1, "rng_state"));
                ee.call(addr("rider_body_update").unwrap(), &[u64::from(body_actor())], &[cap.get(frame, "dt")], 2_000_000_000);
            }
            let reads = ee.reads.borrow_mut().take().unwrap();
            wanted.extend(reads.into_iter().filter(|&x| {
                x >> 28 != 7
                    && !ee.written.contains(&x)
                    && !(req - 0x200..req + 0x300).contains(&x)
                    && !(far..far + 0x100).contains(&x)
                    && !(body_actor()..body_actor() + 0x1000).contains(&x)
                    && changing.contains(&(x & 0x01ff_ffff))
            }));
            frame += 1;
        }
        note(&format!("{} from {}: parted at {parted:?}", w.name, w.frame));
    }
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    for &x in &wanted {
        let x = x & 0x01ff_ffff;
        match ranges.last_mut() {
            Some(last) if last.1 + 32 >= x => last.1 = x + 1,
            _ => ranges.push((x, x + 1)),
        }
    }
    note(&format!("{} ranges, {} bytes:", ranges.len(), ranges.iter().map(|r| r.1 - r.0).sum::<u32>()));
    for (a, b) in ranges {
        note(&format!("  {a:#x} {}", b - a));
    }
}

// ---------------------------------------------------------------------------------------------------
// S61: the port's tick against the original's
// ---------------------------------------------------------------------------------------------------

use game_sim::biplane::{Body, World as Stepped};
use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::collide::{Bundle, Record};
use game_sim::math::Mat4;
use game_sim::phy::{Throw, Wrecks};
use game_sim::ragdoll::{self, Bone, Collision, Joint, Launch, Pin, Ragdoll, Records, Tables};
use game_sim::ragdoll_body::{Actor, Cut, Fade, Look, Outside, Request, Scenery, Skeleton};
use game_sim::contact::{Collider, Mover, Node, Other, Side, Vehicle, Weighed};
use game_sim::ragdoll_contact::{Against, Contact, Scene, Sphere, moving};
use game_sim::rider_anim::Mode;
use ps2_float::F;
use game_sim::ragdoll_scene::{BikeSearch, Cells};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

/// The actor's fields (research/oracle.txt, S50, S51 and S61).
mod at {
    pub const VTABLE: u32 = 0x08;
    pub const RIDING: u32 = 0x2a4;
    pub const REPOSE: u32 = 0x2a8;
    pub const PLACED: u32 = 0x2ac;
    pub const ACTIVE: u32 = 0x2b0;
    pub const STIFF: u32 = 0x2b4;
    pub const TORQUES: u32 = 0x2b8;
    pub const WORLD: u32 = 0x2cc;
    pub const GRAVITY_SCALE: u32 = 0x2d0;
    pub const SKELETON: u32 = 0x234;
    pub const PARTS: u32 = 0x2f4;
    pub const CUT: u32 = 0x420;
    pub const LOOK_ON: u32 = 0x44c;
    pub const LOOK_TARGET: u32 = 0x454;
    pub const LOOK_FAST: u32 = 0x464;
    pub const LOOK_SLOW: u32 = 0x46c;
    pub const LOOK_AT: u32 = 0x474;
    pub const MODE: u32 = 0x47c;
    pub const PREV: u32 = 0x480;
    pub const BIKE: u32 = 0x488;
    pub const TOUCHING: u32 = 0x48c;
    pub const ROLL: u32 = 0x490;
    pub const CUT_NOW: u32 = 0x494;
    pub const CUT_WAS: u32 = 0x498;
    pub const ENTERED: u32 = 0x49c;
    pub const PART_RECORDS: u32 = 0x4b8;
    pub const JOINT_RECORDS: u32 = 0x4c0;
    pub const HOLD_TARGETS: u32 = 0x4d0;
    pub const FOOT_POINTS: u32 = 0x510;
    pub const HOLD_VELOCITY: u32 = 0x530;
    pub const BOX_CENTRE: u32 = 0x540;
    pub const BOX_HALF: u32 = 0x550;
    pub const BIKE_FRAME: u32 = 0x580;
    pub const HOLDS: [u32; 4] = [0x5d0, 0x5d4, 0x5d8, 0x5dc];
    pub const BOX: u32 = 0x5f8;
    pub const COLLISIONS: u32 = 0x624;
    pub const FORCE: u32 = 0x62c;
    pub const FOOT_FADE: u32 = 0x638;
    pub const FADE_LENGTH: u32 = 0x644;
    pub const FADE_LEFT: u32 = 0x648;
    pub const POSE_SPRING: u32 = 0x650;
    pub const POSE_DAMPING: u32 = 0x660;
    pub const POSE_DAMPING_BASE: u32 = 0x670;
    pub const POSED: u32 = 0x680;
    pub const LATCHED: u32 = 0x684;
    pub const OTHER_FRAME: u32 = 0x6a8;
    pub const WORLD_FRAME: u32 = 0x6ac;
    pub const OFF_BIKE: u32 = 0x6b0;
    pub const SETTLED: u32 = 0x6b4;
    pub const FLUNG: u32 = 0x6b8;
    pub const FORCE_TIME: u32 = 0x6bc;
    pub const HAND_HOLD: u32 = 0x6c0;
    pub const LAUNCH: u32 = 0x6d0;
    pub const PULL: u32 = 0x6e0;
    pub const IMPACT: u32 = 0x6f0;
    pub const WRECKS: u32 = 0x700;
    pub const GROUND_DAMPING: u32 = 0x8c0;
    pub const FLAIL_HEIGHT: u32 = 0x8d0;
    pub const ROLL_TIME: u32 = 0x8d4;
    pub const ROLL_SPEED: u32 = 0x8d8;
    pub const CLIP: u32 = 0x8f4;
    pub const LOOK_POSE: u32 = 0x9f0;
    pub const GATE_STEP: u32 = 0xa0c;
    pub const STAND_SPEED: u32 = 0xa14;
    pub const SLOW_SPEED: u32 = 0xa20;
    pub const GATE_CLOCK: u32 = 0xa28;
    pub const IDLE: u32 = 0xa2c;
}

/// The riding request's fields the update reads and the three its tail writes.
mod req_at {
    pub const LIVE: u32 = 0x08;
    pub const HELD: u32 = 0x0c;
    pub const ROWS: u32 = 0x10;
    pub const FRAME: u32 = 0x50;
    pub const VELOCITY: u32 = 0x90;
    pub const RATES: u32 = 0xa0;
    pub const HANDS: u32 = 0xb0;
    pub const FEET: u32 = 0xd0;
    pub const BOX: [u32; 2] = [0xf0, 0xf4];
    pub const LOOKING: u32 = 0xf8;
    pub const POSED: u32 = 0xfc;
    pub const REASON: u32 = 0x104;
    pub const IMPACT: u32 = 0x130;
    pub const CUT: u32 = 0x140;
    pub const HIT_VELOCITY: u32 = 0x150;
    pub const TUCKED: u32 = 0x16c;
    pub const PITCH: u32 = 0x170;
    pub const SPEED: u32 = 0x178;
    pub const CENTRE: u32 = 0x180;
    pub const QUAT: u32 = 0x190;
    pub const BIKE: u32 = 0x1ac;
    pub const TOUCHING: u32 = 0x1bc;
    pub const MAIN: u32 = 0x1c0;
    pub const EFFECT: u32 = 0x238;
    pub const FADE: u32 = 0x23c;
    pub const FADE_FROM: u32 = 0x240;
}

/// Offsets inside a pin, a bone collision and a bone's body.
mod pin_at {
    pub const ON: u32 = 0x04;
    pub const ANCHORS: [u32; 2] = [0x20, 0x30];
    pub const TARGET: u32 = 0x40;
    pub const TARGET_VELOCITY: u32 = 0x50;
    pub const FIRST: u32 = 0x68;
    pub const SECOND: u32 = 0x6c;
    pub const LOWER: u32 = 0x80;
    pub const UPPER: u32 = 0x90;
    pub const HIT: u32 = 0xa0;
    pub const STICKY: u32 = 0xb0;
    pub const GAINS: u32 = 0x100;
}

mod coll_at {
    pub const ON: u32 = 0x04;
    pub const TOUCHED: u32 = 0x14;
    pub const GROUND: u32 = 0x18;
    pub const SCENE: u32 = 0x1c;
    pub const SWEPT: u32 = 0x20;
    pub const SWEEP: u32 = 0x24;
    pub const SWEEP_BACK: u32 = 0x28;
    pub const UNPENETRATION: u32 = 0x2c;
    pub const ROTATION: u32 = 0x30;
    pub const RESTITUTION: u32 = 0x34;
    pub const FRICTION: u32 = 0x38;
    pub const POINT: u32 = 0x40;
    pub const NORMAL: u32 = 0x50;
    pub const DV: u32 = 0x60;
    pub const TERRAIN: u32 = 0x78;
    pub const BODY: u32 = 0x7c;
    pub const SPHERE: u32 = 0x88;
    pub const SEGMENT: u32 = 0x8c;
    /// The terrain's box.
    pub const BOX_MIN: u32 = 0x30;
    pub const BOX_MAX: u32 = 0x40;
    /// A collider's hit flag (bit 5 of the byte) and its record.
    pub const FLAGS: u32 = 0x116;
    pub const RECORD: u32 = 0x6c;
}

mod body_at {
    pub const MASS: u32 = 0x0c;
    pub const INV_MASS: u32 = 0x10;
    pub const RADIUS: u32 = 0x14;
    pub const DAMPING: u32 = 0x20;
    pub const LAST_POSITION: u32 = 0x50;
    pub const POSITION: u32 = 0x60;
    pub const VELOCITY: u32 = 0x70;
    pub const LAST_FORCE: u32 = 0x80;
    pub const FORCE: u32 = 0x90;
    pub const ACCEL: u32 = 0xa0;
    pub const SHOCK_PEAK: u32 = 0xb0;
    pub const SHOCK: u32 = 0xc0;
    pub const STEP: u32 = 0xf0;
    pub const INERTIA: u32 = 0x110;
    pub const INV_INERTIA: u32 = 0x120;
    pub const ANG_DAMPING: u32 = 0x130;
    pub const LAST_QUAT: u32 = 0x150;
    pub const REFERENCE: u32 = 0x160;
    pub const QUAT: u32 = 0x170;
    pub const COFG: u32 = 0x180;
    pub const MOMENT: u32 = 0x190;
    pub const LAST_MOMENT: u32 = 0x1a0;
    pub const RATES: u32 = 0x1b0;
    pub const ANG_ACCEL: u32 = 0x1c0;
    pub const TWIST_PEAK: u32 = 0x1d0;
    pub const TWIST: u32 = 0x1e0;
}

fn f(ee: &Ee, a: u32) -> F {
    F(ee.f32_at(a))
}

fn bits(v: Vf) -> [u32; 4] {
    [v[0].to_bits(), v[1].to_bits(), v[2].to_bits(), v[3].to_bits()]
}

fn bone_of(ee: &Ee, body: u32) -> usize {
    (0..ragdoll::PARTS).find(|&i| bone_body(ee, i) == body).unwrap_or(usize::MAX)
}

fn joint_pin(ee: &Ee, i: usize) -> u32 {
    ee.u32_at(ee.u32_at(body_actor() + at::JOINT_RECORDS) + 0x50 * i as u32 + 8)
}

fn collision(ee: &Ee, i: usize) -> u32 {
    ee.u32_at(ee.u32_at(body_actor() + at::COLLISIONS) + 4 * i as u32)
}

fn pins(ee: &Ee) -> Vec<u32> {
    at::HOLDS.iter().map(|&h| ee.u32_at(body_actor() + h)).chain((0..17).map(|i| joint_pin(ee, i))).collect()
}

fn pin_of(ee: &Ee, p: u32) -> Pin {
    use pin_at::*;
    let second = ee.u32_at(p + SECOND);
    Pin {
        first: bone_of(ee, ee.u32_at(p + FIRST)),
        second: (second != 0).then(|| bone_of(ee, second)),
        anchors: ANCHORS.map(|a| quad(ee, p + a)),
        target: quad(ee, p + TARGET),
        target_velocity: quad(ee, p + TARGET_VELOCITY),
        lower: quad(ee, p + LOWER),
        upper: quad(ee, p + UPPER),
        hit: quad(ee, p + HIT),
        sticky: quad(ee, p + STICKY),
        gains: quad(ee, p + GAINS),
    }
}

fn bone_at(ee: &Ee, b: u32) -> Bone {
    use body_at::*;
    Bone {
        body: Body {
            enabled: ee.u32_at(b) != 0,
            inv_mass: f(ee, b + INV_MASS),
            damping: quad(ee, b + DAMPING),
            position: quad(ee, b + POSITION),
            velocity: quad(ee, b + VELOCITY),
            inv_inertia: quad(ee, b + INV_INERTIA),
            ang_damping: quad(ee, b + ANG_DAMPING),
            quat: quad(ee, b + QUAT),
            rates: quad(ee, b + RATES),
            last_position: quad(ee, b + LAST_POSITION),
            last_force: quad(ee, b + LAST_FORCE),
            force: quad(ee, b + FORCE),
            accel: quad(ee, b + ACCEL),
            shock_peak: quad(ee, b + SHOCK_PEAK),
            shock: quad(ee, b + SHOCK),
            step: quad(ee, b + STEP),
            last_quat: quad(ee, b + LAST_QUAT),
            cofg: quad(ee, b + COFG),
            moment: quad(ee, b + MOMENT),
            last_moment: quad(ee, b + LAST_MOMENT),
            ang_accel: quad(ee, b + ANG_ACCEL),
            twist_peak: quad(ee, b + TWIST_PEAK),
            twist: quad(ee, b + TWIST),
        },
        mass: f(ee, b + MASS),
        inertia: quad(ee, b + INERTIA),
        reference: quad(ee, b + REFERENCE),
    }
}

fn bones_of(ee: &Ee) -> Vec<Bone> {
    (0..ragdoll::PARTS).map(|i| bone_at(ee, bone_body(ee, i))).collect()
}

fn throw_at(ee: &Ee, a: u32) -> Throw {
    Throw {
        force_time: f(ee, a),
        hand_hold_time: f(ee, a + 4),
        force: quad(ee, a + 0x10),
        force_time_delta: f(ee, a + 0x20),
        hand_hold_time_delta: f(ee, a + 0x24),
        force_delta: quad(ee, a + 0x30),
    }
}

/// The actor's `[Wrecks]`: the seven throws and the settle test's four numbers.
fn wrecks_of(ee: &Ee) -> Wrecks {
    let t = |k: u32| throw_at(ee, body_actor() + at::WRECKS + 0x40 * k);
    Wrecks {
        hit_front: t(0),
        hit_side: t(1),
        hit_back: t(2),
        hit_bottom: t(3),
        fall_off_front: t(4),
        fall_off_back: t(5),
        stunt: t(6),
        ground_angular_rate_damping: quad(ee, body_actor() + at::GROUND_DAMPING),
        rider_flail_height: f(ee, body_actor() + at::FLAIL_HEIGHT),
        rider_roll_time: f(ee, body_actor() + at::ROLL_TIME),
        rider_roll_speed: f(ee, body_actor() + at::ROLL_SPEED),
        ..Wrecks::default()
    }
}

fn ragdoll_of(ee: &Ee) -> Ragdoll {
    let recs = ee.u32_at(body_actor() + at::PART_RECORDS);
    let parts = (0..ragdoll::PARTS as u32)
        .map(|i| {
            let r = recs + 0x60 * i;
            ragdoll::PartRecord {
                ride_mass: f(ee, r),
                wreck_mass: f(ee, r + 4),
                ride_inertia: quad(ee, r + 0x10),
                wreck_inertia: quad(ee, r + 0x20),
                ride_angular_damping: quad(ee, r + 0x30),
                settled_angular_damping: quad(ee, r + 0x40),
                limp_angular_damping: quad(ee, r + 0x50),
            }
        })
        .collect();
    let jrecs = ee.u32_at(body_actor() + at::JOINT_RECORDS);
    let joints: Vec<_> = (0..17)
        .map(|i| {
            let r = jrecs + 0x50 * i as u32;
            let pin = joint_pin(ee, i);
            ragdoll::JointRecord {
                part1: bone_of(ee, ee.u32_at(pin + 0x68)),
                part2: bone_of(ee, ee.u32_at(pin + 0x6c)),
                max_angle: f(ee, r + 0xc),
                ride_spring: quad(ee, r + 0x10),
                ride_damping: quad(ee, r + 0x20),
                wreck_spring: quad(ee, r + 0x30),
                wreck_damping: quad(ee, r + 0x40),
            }
        })
        .collect();
    let collisions: Vec<usize> = (0..9).map(|i| bone_of(ee, ee.u32_at(collision(ee, i) + coll_at::BODY))).collect();
    let mut r = Ragdoll::new(Records { parts, joints, collisions });
    r.bones = bones_of(ee);
    for (i, j) in r.joints.iter_mut().enumerate() {
        let pin = joint_pin(ee, i);
        *j = Joint {
            wrecked: ee.u32_at(pin + 0x70) != 0,
            max_angle: f(ee, pin + 0xc4),
            spring: quad(ee, pin + 0xd0),
            damping: quad(ee, pin + 0xe0),
            critical: quad(ee, pin + 0xf0),
        };
    }
    for (i, c) in r.collisions.iter_mut().enumerate() {
        let k = collision(ee, i);
        *c = Collision { bone: c.bone, enabled: ee.u32_at(k + coll_at::ON) != 0, touching: ee.u32_at(k + coll_at::TOUCHED) != 0 };
    }
    let w = ee.u32_at(body_actor() + at::WORLD);
    r.world = Stepped { iterations: ee.i32_at(w), rate: f(ee, w + 4) };
    r.holds = at::HOLDS.map(|h| ee.u32_at(ee.u32_at(body_actor() + h) + pin_at::ON) != 0);
    r.bike_box = ee.u32_at(ee.u32_at(body_actor() + at::BOX) + 4) != 0;
    let force = ee.u32_at(body_actor() + at::FORCE);
    r.throwing = ee.u32_at(force) != 0;
    r.launch = Launch {
        force: quad(ee, body_actor() + at::LAUNCH),
        force_time: f(ee, body_actor() + at::FORCE_TIME),
        hand_hold: f(ee, body_actor() + at::HAND_HOLD),
        target: bone_of(ee, ee.u32_at(force + 0x24)),
        world_frame: ee.u32_at(body_actor() + at::WORLD_FRAME) != 0,
        other_frame: ee.u32_at(body_actor() + at::OTHER_FRAME) != 0,
    };
    r.entered = ee.u32_at(body_actor() + at::ENTERED) != 0;
    r.stiff = ee.u32_at(body_actor() + at::STIFF) != 0;
    r.riding = ee.u32_at(body_actor() + at::RIDING) != 0;
    r.off_bike = ee.u32_at(body_actor() + at::OFF_BIKE) != 0;
    r.flung = ee.u32_at(body_actor() + at::FLUNG) != 0;
    r.settled = ee.u32_at(body_actor() + at::SETTLED) != 0;
    r.roll = f(ee, body_actor() + at::ROLL);
    r.pins = pins(ee).iter().map(|&p| pin_of(ee, p)).collect();
    r.pull = quad(ee, body_actor() + at::PULL);
    r
}

fn contact_of(ee: &Ee, c: u32) -> Contact {
    use coll_at::*;
    Contact {
        touched: ee.u32_at(c + TOUCHED) != 0,
        ground: ee.u32_at(c + GROUND) != 0,
        scene: ee.u32_at(c + SCENE) != 0,
        swept: ee.u32_at(c + SWEPT) != 0,
        point: quad(ee, c + POINT),
        normal: quad(ee, c + NORMAL),
        dv: quad(ee, c + DV),
    }
}

fn sphere_of(ee: &Ee, c: u32, bone: usize) -> Sphere {
    use coll_at::*;
    Sphere {
        bone,
        radius: f(ee, ee.u32_at(c + BODY) + body_at::RADIUS),
        unpenetration: f(ee, c + UNPENETRATION),
        rotation: f(ee, c + ROTATION),
        restitution: f(ee, c + RESTITUTION),
        friction: f(ee, c + FRICTION),
        sweep: ee.u32_at(c + SWEEP) != 0,
        sweep_back: f(ee, c + SWEEP_BACK),
    }
}

fn mode_of(m: i32) -> Mode {
    match m {
        0 => Mode::Held,
        1 => Mode::Riding,
        2 => Mode::Wrecked,
        3 => Mode::Posed,
        4 => Mode::Gate,
        _ => panic!("mode {m}"),
    }
}

fn mat_at(ee: &Ee, a: u32) -> Mat4 {
    Mat4([0, 0x10, 0x20, 0x30].map(|k| quad(ee, a + k)))
}

/// The actor as the port's, off the RAM.
fn actor_of(ee: &Ee) -> Actor {
    let a = |o: u32| body_actor() + o;
    let pair = |o: u32| [f(ee, a(o)), f(ee, a(o + 4))];
    let mut actor = Actor::new(ragdoll_of(ee), 9);
    actor.contacts = (0..9).map(|i| contact_of(ee, collision(ee, i))).collect();
    actor.mode = mode_of(ee.i32_at(a(at::MODE)));
    actor.prev = mode_of(ee.i32_at(a(at::PREV)));
    actor.posed = ee.u32_at(a(at::POSED)) != 0;
    actor.latched = ee.u32_at(a(at::LATCHED)) != 0;
    actor.idle = ee.u32_at(a(at::IDLE)) != 0;
    actor.look_pose = ee.i32_at(a(at::LOOK_POSE));
    actor.clip = ee.i32_at(a(at::CLIP));
    actor.touching = ee.u32_at(a(at::TOUCHING)) != 0;
    actor.fade = Fade { length: f(ee, a(at::FADE_LENGTH)), left: f(ee, a(at::FADE_LEFT)) };
    actor.foot_fade = pair(at::FOOT_FADE);
    actor.look = Look {
        on: [0, 4].map(|k| ee.u32_at(a(at::LOOK_ON + k)) != 0),
        fast: pair(at::LOOK_FAST),
        slow: pair(at::LOOK_SLOW),
        target: pair(at::LOOK_TARGET),
        at: pair(at::LOOK_AT),
    };
    actor.pose_spring = quad(ee, a(at::POSE_SPRING));
    actor.pose_damping = quad(ee, a(at::POSE_DAMPING));
    actor.pose_damping_base = quad(ee, a(at::POSE_DAMPING_BASE));
    actor.stand_speed = f(ee, a(at::STAND_SPEED));
    actor.slow_speed = f(ee, a(at::SLOW_SPEED));
    actor.gate_step = ee.i32_at(a(at::GATE_STEP));
    actor.gate_clock = f(ee, a(at::GATE_CLOCK));
    actor.foot_points = [0, 0x10].map(|k| quad(ee, a(at::FOOT_POINTS + k)));
    actor.hold_targets = std::array::from_fn(|k| quad(ee, a(at::HOLD_TARGETS + 0x10 * k as u32)));
    actor.hold_velocity = quad(ee, a(at::HOLD_VELOCITY));
    actor.bike_box = (quad(ee, a(at::BOX_CENTRE)), quad(ee, a(at::BOX_HALF)));
    actor.bike_frame = mat_at(ee, a(at::BIKE_FRAME));
    actor.placed = ee.u32_at(a(at::PLACED)) != 0;
    actor.active = ee.u32_at(a(at::ACTIVE)) != 0;
    actor.repose = ee.u32_at(a(at::REPOSE)) != 0;
    actor.torques = ee.u32_at(a(at::TORQUES)) != 0;
    actor.gravity_scale = f(ee, a(at::GRAVITY_SCALE));
    let cut = ee.u32_at(a(at::CUT));
    actor.cut = (ee.u32_at(cut + 0x34) == 2)
        .then(|| Cut { now: ee.u32_at(a(at::CUT_NOW)) != 0, was: ee.u32_at(a(at::CUT_WAS)) != 0 });
    actor.impact = quad(ee, a(at::IMPACT));
    actor
}

/// The riding request as the port's, off the RAM.
fn request_of(ee: &Ee) -> Request {
    use req_at::*;
    let r = ee.u32_at(body_actor() + at::BIKE);
    let on = |o: u32| ee.u32_at(r + o) != 0;
    let q = |o: u32| quad(ee, r + o);
    let (bx, half) = (ee.u32_at(r + BOX[0]), ee.u32_at(r + BOX[1]));
    let bike = ee.u32_at(r + BIKE);
    Request {
        live: on(LIVE),
        held: on(HELD),
        rows: [0, 0x10, 0x20].map(|k| q(ROWS + k)),
        frame: mat_at(ee, r + FRAME),
        velocity: q(VELOCITY),
        rates: q(RATES),
        hands: [q(HANDS), q(HANDS + 0x10)],
        feet: [q(FEET), q(FEET + 0x10)],
        bike_box: (bx != 0 && half != 0).then(|| (quad(ee, bx), quad(ee, half))),
        looking: on(LOOKING),
        posed: on(POSED),
        reason: ee.i32_at(r + REASON),
        cut: on(CUT),
        hit_velocity: q(HIT_VELOCITY),
        tucked: on(TUCKED),
        pitch: f(ee, r + PITCH),
        speed: f(ee, r + SPEED),
        centre: q(CENTRE),
        quat: q(QUAT),
        bike: (bike != 0).then(|| (quad(ee, bike + 0x40), f(ee, bike + 0x60))),
        effect: ee.i32_at(r + EFFECT),
        fade: f(ee, r + FADE),
        fade_from: f(ee, r + FADE_FROM),
    }
}

fn rest_at(ee: &Ee, bone: usize) -> u32 {
    let idx = ee.u32_at(bone_body(ee, bone) + 8);
    let part = ee.u32_at(body_actor() + at::PARTS) + 0xb0 * idx;
    ee.u32_at(body_actor() + at::SKELETON) + 0x60 * ee.u32_at(part + 0x40) + 0x50
}

fn tables_of(ee: &Ee) -> Tables {
    let base = ee.u32_at(addr("curve_table_ptr").unwrap());
    let curve: Vec<[f32; 2]> =
        (0..CURVE_ENTRIES as u32).map(|k| [ee.f32_at(base + 8 * k), ee.f32_at(base + 8 * k + 4)]).collect();
    Tables { curve: CurveTable::from_entries(&curve), ..Tables::build() }
}

/// Run the hooked function itself from inside its hook, on a stack below the caller's, and come
/// back to the hook's caller as if nothing had been hooked.
fn through(ee: &mut Ee) {
    let (pc, ra, sp, steps, stack) = (ee.pc, ee.r(31), ee.r(29), ee.steps, ee.stack);
    let args: Vec<u64> = (4..12).map(|k| ee.r(k)).collect();
    ee.stack = (sp as u32 - 0x800) & !0xf;
    ee.call(pc, &args, &[], 2_000_000_000);
    ee.set(31, ra);
    ee.set(29, sp);
    (ee.steps, ee.stack, ee.pc) = (steps, stack, pc);
}

/// What the world answered the original in one tick, in the order it asked.
#[derive(Default)]
struct Heard {
    /// The ground queries: the point asked about, and the point and normal answered.
    ground: VecDeque<([u32; 4], Vf, Vf)>,
    /// Each bone collision's sphere against the scene: its centre, the hits, and what each hit is
    /// with when that is another collidable (the record names it).
    spheres: VecDeque<([u32; 4], Option<Vec<(Vf, Vf)>>, Vec<Option<Moving>>)>,
    /// The others of the last sphere test the port made.
    last: Vec<Option<Moving>>,
    /// Each sweep: the segment, and the hit.
    segments: VecDeque<(([u32; 4], [u32; 4]), Option<(Vf, Vf)>)>,
    centre: [u32; 4],
    segment: ([u32; 4], [u32; 4]),
    /// The bones as each pose-and-reset left them.
    poses: VecDeque<Vec<Bone>>,
    /// The riding pose's point for each bone, as the skeleton handed it, and the skeleton's rests
    /// as the step read them (the update moves the skeleton before the step).
    points: [Option<Vf>; ragdoll::PARTS],
    rests: Option<[Vf; ragdoll::PARTS]>,
    /// What the port asked that the original did not.
    odd: Vec<String>,
    /// The sphere hits against another collider (the record names it).
    moving: usize,
}

/// The other side of a sphere's hit, read off the original's RAM as the hit was made: what the
/// push-out measures the bone's velocity against (S62, research/oracle.txt).
#[derive(Clone)]
enum Moving {
    Still,
    Mover(Mover),
    Body(Collider, Body),
    Vehicle(Collider, Vehicle),
}

impl Moving {
    /// The collidable at `c`: its collision object (`+0x80`) on a dynamics body (`+0xf4`) or on a
    /// vehicle's chassis (`+0xf0`, its transform `+0xf8`), else its owner (`+0x7c`) when that is a
    /// mover (class `+0x1b0` is 2).
    fn of(ee: &mut Ee, c: u32) -> Moving {
        let (owner, object) = (ee.u32_at(c + 0x7c), ee.u32_at(c + 0x80));
        let node_at = |ee: &Ee, n: u32| {
            assert_ne!(ee.u32_at(n + 0x110), 0, "a transform not up to date at {n:#x}");
            Node { rows: [0, 1, 2].map(|k| quad(ee, n + 0x80 + 0x10 * k)), origin: quad(ee, n + 0xb0) }
        };
        if object != 0 {
            let collider = Collider { cofg: quad(ee, object + 0x90), cofg_world: quad(ee, object + 0xa0), ..Collider::default() };
            let body = ee.u32_at(object + 0xf4);
            if body != 0 {
                use body_at::*;
                let b = Body {
                    position: quad(ee, body + POSITION),
                    velocity: quad(ee, body + VELOCITY),
                    quat: quad(ee, body + QUAT),
                    rates: quad(ee, body + RATES),
                    ..Body::default()
                };
                return Moving::Body(collider, b);
            }
            let v = ee.u32_at(object + 0xf0);
            let vehicle = Vehicle {
                node: node_at(ee, ee.u32_at(object + 0xf8)),
                cofg: quad(ee, v + 0x190),
                velocity: quad(ee, v + 0x260),
                rates: quad(ee, v + 0x290),
                ..Vehicle::default()
            };
            return Moving::Vehicle(collider, vehicle);
        }
        if owner == 0 || ee.u32_at(owner + 0x1b0) != 2 {
            return Moving::Still;
        }
        // The mover's spin and velocity are its two virtuals' answers.
        let vtable = ee.u32_at(owner + 8);
        let out = ee.scratch(0);
        let mut ask = |ee: &mut Ee, slot: u32| {
            let (pc, ra, sp, steps, stack) = (ee.pc, ee.r(31), ee.r(29), ee.steps, ee.stack);
            ee.stack = (sp as u32 - 0x800) & !0xf;
            ee.call(ee.u32_at(vtable + slot), &[u64::from(out), u64::from(owner)], &[], 2_000_000_000);
            ee.set(31, ra);
            ee.set(29, sp);
            (ee.steps, ee.stack, ee.pc) = (steps, stack, pc);
            quad(ee, out)
        };
        let spin = ask(ee, 0xbc);
        let velocity = ask(ee, 0xb8);
        Moving::Mover(Mover { spin, velocity, node: node_at(ee, ee.u32_at(owner + 0x254)), origin: quad(ee, owner + 0x190) })
    }

    /// Its velocity at `at`, as the port's push-out has it.
    fn at(&self, at: Vf) -> Vf {
        match self.clone() {
            Moving::Still => moving(at, Against::Still),
            Moving::Mover(m) => moving(at, Against::Mover(&m)),
            Moving::Body(mut collider, mut body) => {
                let mut w = Weighed { body: &mut body, mass: F::ONE };
                moving(at, Against::Other(&mut Other { collider: &mut collider, side: Side::Body(&mut w) }))
            }
            Moving::Vehicle(mut collider, mut vehicle) => {
                moving(at, Against::Other(&mut Other { collider: &mut collider, side: Side::Vehicle(&mut vehicle) }))
            }
        }
    }
}

/// Every call of the update's that reaches the world, hooked to run as it is and to write down
/// what it answered.
fn listen(ee: &mut Ee) -> Rc<RefCell<Heard>> {
    let heard = Rc::new(RefCell::new(Heard::default()));
    let spheres: Vec<u32> = (0..9).map(|i| ee.u32_at(collision(ee, i) + coll_at::SPHERE)).collect();
    let segments: Vec<u32> = (0..9).map(|i| ee.u32_at(collision(ee, i) + coll_at::SEGMENT)).collect();
    let h = Rc::clone(&heard);
    ee.hooks.insert(
        addr("ground_query").unwrap(),
        Box::new(move |ee: &mut Ee| {
            let (point, normal) = (ee.r(5) as u32, ee.r(6) as u32);
            let asked = bits(quad(ee, point));
            through(ee);
            let n = if normal == 0 { Vf::default() } else { quad(ee, normal) };
            h.borrow_mut().ground.push_back((asked, quad(ee, point), n));
        }),
    );
    let h = Rc::clone(&heard);
    ee.hooks.insert(
        addr("collider_set_position").unwrap(),
        Box::new(move |ee: &mut Ee| {
            h.borrow_mut().centre = bits(quad(ee, ee.r(5) as u32));
            through(ee);
        }),
    );
    let h = Rc::clone(&heard);
    ee.hooks.insert(
        addr("probe_bundle_set_points").unwrap(),
        Box::new(move |ee: &mut Ee| {
            let p = ee.r(5) as u32;
            if p != 0 {
                h.borrow_mut().segment = (bits(quad(ee, p)), bits(quad(ee, p + 0x10)));
            }
            through(ee);
        }),
    );
    let collide = |h: Rc<RefCell<Heard>>, spheres: Vec<u32>, segments: Vec<u32>| {
        Box::new(move |ee: &mut Ee| {
            let c = ee.r(4) as u32;
            through(ee);
            let hit = ee.u8_at(c + coll_at::FLAGS) & 0x20 != 0;
            let rec = ee.u32_at(c + coll_at::RECORD);
            let mut h = h.borrow_mut();
            if segments.contains(&c) {
                let answer = hit.then(|| (quad(ee, rec + 0x30), quad(ee, rec + 0x20)));
                let s = h.segment;
                h.segments.push_back((s, answer));
            } else if spheres.contains(&c) {
                let n = if hit { ee.u32_at(rec + 4) } else { 0 };
                let answer = hit.then(|| (0..n).map(|i| (quad(ee, rec + 0x60 + 0x10 * i), quad(ee, rec + 0x30 + 0x10 * i))).collect());
                let others: Vec<u32> = (0..n).map(|i| ee.u32_at(rec + 0x90 + 4 * i)).collect();
                h.moving += others.iter().filter(|&&o| o != 0).count();
                let others = others.into_iter().map(|o| (o != 0).then(|| Moving::of(ee, o))).collect();
                let c = h.centre;
                h.spheres.push_back((c, answer, others));
            }
        }) as Box<dyn FnMut(&mut Ee)>
    };
    ee.hooks.insert(addr("collide_against_search").unwrap(), collide(Rc::clone(&heard), spheres.clone(), segments.clone()));
    ee.hooks.insert(addr("probe_collide_world").unwrap(), collide(Rc::clone(&heard), spheres, segments));
    let h = Rc::clone(&heard);
    ee.hooks.insert(
        addr("orientation_transform_point").unwrap(),
        Box::new(move |ee: &mut Ee| {
            let (out, body) = (ee.r(4) as u32, (ee.r(6) as u32).wrapping_sub(body_at::COFG));
            through(ee);
            let i = bone_of(ee, body);
            if i < ragdoll::PARTS {
                h.borrow_mut().points[i] = Some(quad(ee, out));
            }
        }),
    );
    // The normalisation right after the step: the skeleton's rests as the step read them.
    let normalise = ee.u32_at(ee.u32_at(body_actor() + at::VTABLE) + 0xac);
    let h = Rc::clone(&heard);
    ee.hooks.insert(
        normalise,
        Box::new(move |ee: &mut Ee| {
            h.borrow_mut().rests = Some(std::array::from_fn(|i| quad(ee, rest_at(ee, i))));
            through(ee);
        }),
    );
    let reset = ee.u32_at(ee.u32_at(body_actor() + at::VTABLE) + 0xa0);
    let h = Rc::clone(&heard);
    ee.hooks.insert(
        reset,
        Box::new(move |ee: &mut Ee| {
            through(ee);
            h.borrow_mut().poses.push_back(bones_of(ee));
        }),
    );
    heard
}

/// The world, the scene and the skeleton as the original heard them, for the port.
struct Replay<'a>(&'a RefCell<Heard>);

impl game_sim::world::World for Replay<'_> {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        let mut h = self.0.borrow_mut();
        match h.ground.pop_front() {
            Some((asked, p, n)) => {
                if asked != bits(at) {
                    h.odd.push(format!("the ground at {at:?}, the original's at {asked:x?}"));
                }
                (p, n)
            }
            None => {
                h.odd.push(format!("the ground at {at:?}, which the original did not ask"));
                (at, Vf::new(0.0, 1.0, 0.0, 0.0))
            }
        }
    }
    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

impl Scene for Replay<'_> {
    fn touching(&mut self, centre: Vf, _radius: F) -> Option<Vec<(Vf, Vf)>> {
        let mut h = self.0.borrow_mut();
        match h.spheres.pop_front() {
            Some((asked, hits, others)) => {
                if asked != bits(centre) {
                    h.odd.push(format!("a sphere at {centre:?}, the original's at {asked:x?}"));
                }
                h.last = others;
                hits
            }
            None => {
                h.odd.push(format!("a sphere at {centre:?}, which the original did not test"));
                h.last.clear();
                None
            }
        }
    }
    fn other(&mut self, i: usize) -> Option<usize> {
        self.0.borrow().last.get(i).and_then(Option::as_ref).map(|_| i)
    }
    fn velocity(&mut self, other: usize, at: Vf) -> Vf {
        let h = self.0.borrow();
        h.last[other].as_ref().expect("an other the port named").at(at)
    }
    fn sweep(&mut self, from: Vf, to: Vf) -> Option<(Vf, Vf)> {
        let mut h = self.0.borrow_mut();
        match h.segments.pop_front() {
            Some((asked, hit)) => {
                if asked != (bits(from), bits(to)) {
                    h.odd.push(format!("a sweep {from:?} to {to:?}, the original's {asked:x?}"));
                }
                hit
            }
            None => {
                h.odd.push(format!("a sweep {from:?} to {to:?}, which the original did not make"));
                None
            }
        }
    }
}

struct Posed<'a>(&'a RefCell<Heard>);

impl Outside for Posed<'_> {
    fn pose(&mut self, doll: &mut Ragdoll, _dt: F, _carried: bool) {
        let mut h = self.0.borrow_mut();
        match h.poses.pop_front() {
            Some(bones) => doll.bones = bones,
            None => h.odd.push("a pose the original did not make".into()),
        }
    }
    fn wreck_sound(&mut self, _reason: i32, rng: &mut Rng) {
        rng.rand();
    }
}

/// Every number of the actor the update writes, by name, as bits.
fn written(a: &Actor) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    let mut v = |name: String, x: Vf| out.extend((0..4).map(|k| (format!("{name}[{k}]"), x[k].to_bits())));
    let r = &a.ragdoll;
    for (i, b) in r.bones.iter().enumerate() {
        let d = &b.body;
        // The linear acceleration's `w` is a register the integrator stores and nothing loads, and so
        // is the angular one's on a pinned body (as in tests/ragdoll.rs): neither is compared.
        let stale = |x: Vf, is: bool| if is { Vf::new(x[0], x[1], x[2], 0.0) } else { x };
        for (n, x) in [
            ("position", d.position),
            ("velocity", d.velocity),
            ("quat", d.quat),
            ("rates", d.rates),
            ("last_position", d.last_position),
            ("last_quat", d.last_quat),
            ("damping", d.damping),
            ("ang_damping", d.ang_damping),
            ("inv_inertia", d.inv_inertia),
            ("force", d.force),
            ("last_force", d.last_force),
            ("accel", stale(d.accel, true)),
            ("shock_peak", d.shock_peak),
            ("shock", d.shock),
            ("step", d.step),
            ("moment", d.moment),
            ("last_moment", d.last_moment),
            ("ang_accel", stale(d.ang_accel, d.inv_mass == F::ZERO)),
            ("twist_peak", d.twist_peak),
            ("twist", d.twist),
            ("cofg", d.cofg),
            ("inertia", b.inertia),
            ("reference", b.reference),
            ("mass/inv_mass/enabled", Vf::new(b.mass.0, d.inv_mass.0, f32::from(u8::from(d.enabled)), 0.0)),
        ] {
            v(format!("bone{i}.{n}"), x);
        }
    }
    for (i, p) in r.pins.iter().enumerate() {
        for (n, x) in [("target", p.target), ("target_velocity", p.target_velocity), ("hit", p.hit), ("sticky", p.sticky), ("gains", p.gains)] {
            v(format!("pin{i}.{n}"), x);
        }
    }
    for (i, j) in r.joints.iter().enumerate() {
        for (n, x) in [("spring", j.spring), ("damping", j.damping), ("critical", j.critical)] {
            v(format!("joint{i}.{n}"), x);
        }
        v(format!("joint{i}.wrecked/max"), Vf::new(f32::from(u8::from(j.wrecked)), j.max_angle.0, 0.0, 0.0));
    }
    for (i, c) in a.contacts.iter().enumerate() {
        v(format!("contact{i}.point"), c.point);
        v(format!("contact{i}.normal"), c.normal);
        v(format!("contact{i}.dv"), c.dv);
        let flags = [c.touched, c.ground, c.scene, c.swept, r.collisions[i].enabled].map(|b| f32::from(u8::from(b)));
        v(format!("contact{i}.touched/ground/scene/swept"), Vf(flags[..4].try_into().unwrap()));
        v(format!("collision{i}.on"), Vf::new(flags[4], 0.0, 0.0, 0.0));
    }
    let b = |x: bool| f32::from(u8::from(x));
    v("holds".into(), Vf(r.holds.map(b)));
    v("throwing/entered/stiff/riding".into(), Vf::new(b(r.throwing), b(r.entered), b(r.stiff), b(r.riding)));
    v("off_bike/flung/settled/roll".into(), Vf::new(b(r.off_bike), b(r.flung), b(r.settled), r.roll.0));
    v("launch.force".into(), r.launch.force);
    v(
        "launch.force_time/hand_hold/target/frames".into(),
        Vf::new(r.launch.force_time.0, r.launch.hand_hold.0, r.launch.target as f32, b(r.launch.world_frame) + 2.0 * b(r.launch.other_frame)),
    );
    v("pull".into(), r.pull);
    v("mode/prev/touching/placed".into(), Vf::new(a.mode as i32 as f32, a.prev as i32 as f32, b(a.touching), b(a.placed)));
    v("repose/fade".into(), Vf::new(b(a.repose), a.fade.length.0, a.fade.left.0, 0.0));
    for (k, t) in a.hold_targets.iter().enumerate() {
        v(format!("hold_target{k}"), *t);
    }
    v("hold_velocity".into(), a.hold_velocity);
    v("box_centre".into(), a.bike_box.0);
    v("box_half".into(), a.bike_box.1);
    for (k, row) in a.bike_frame.0.iter().enumerate() {
        v(format!("bike_frame{k}"), *row);
    }
    v("pose_damping".into(), a.pose_damping);
    v("impact".into(), a.impact);
    out
}

/// The tick the S56 replay ran, rerun with the world's calls listened to; then the port's tick off
/// the RAM the original started from, with the original's answers. Every number the update writes
/// is compared, and what the tail hands the request. A sphere's hit against another collidable is
/// answered with that collidable as the original's RAM had it (S62), and the port's push-out measures
/// the bone against its velocity.
///
/// With `carried`, the port's actor is its own from the tick before (the first tick takes the
/// original's): the port runs the wreck on its own state, and only the world's answers are the
/// original's. The second number is how many of the original's sphere hits were with another
/// collidable.
fn port_tick(ee: &mut Ee, cap: &Capture, frame: u32, draws_before: u32, drew: Option<u32>, carried: Option<&mut Option<Actor>>) -> (Vec<String>, usize) {
    let update = addr("rider_body_update").unwrap();
    let dt = cap.get(frame, "dt");
    let mut r = Rng::from_state(cap.int(frame - 1, "rng_state"));
    (0..draws_before).for_each(|_| {
        r.rand();
    });
    set_rng(ee, r.state());
    let actor = actor_of(ee);
    let request = request_of(ee);
    let wrecks = wrecks_of(ee);
    let tables = tables_of(ee);
    let rests: [Vf; ragdoll::PARTS] = std::array::from_fn(|i| quad(ee, rest_at(ee, i)));
    let spheres: Vec<Sphere> = (0..9).map(|i| sphere_of(ee, collision(ee, i), actor.ragdoll.records.collisions[i])).collect();
    let terrains: Vec<u32> = (0..9).map(|i| ee.u32_at(collision(ee, i) + coll_at::TERRAIN)).collect();
    assert!(terrains.iter().all(|&t| t == terrains[0] && t != 0), "{frame}: the collisions' terrains {terrains:x?}");
    let terrain = (quad(ee, terrains[0] + coll_at::BOX_MIN), quad(ee, terrains[0] + coll_at::BOX_MAX));

    let saved = std::mem::take(&mut ee.hooks);
    let heard = listen(ee);
    ee.call(update, &[u64::from(body_actor())], &[dt], 2_000_000_000);
    ee.hooks = saved;
    let mut out = Vec::new();
    let d = if drew.is_some() { differ(cap, frame, &ee_bones(ee)) } else { Vec::new() };
    if !d.is_empty() {
        out.push(format!("the listened tick is not the replayed one: {:?}", &d[..d.len().min(4)]));
    }
    let drew = drew.unwrap_or_else(|| draws(r.state(), rng_at(ee), 1000).expect("the body's draws"));
    let after = Snapshot::of(ee);
    let game = actor_of(ee);
    let req = ee.u32_at(body_actor() + at::BIKE);
    let game_tail = (ee.u32_at(req + req_at::TOUCHING) != 0, bits(quad(ee, req + req_at::IMPACT)), bits(quad(ee, req + req_at::MAIN)));

    let mut carried = carried;
    let mut port = carried.as_mut().and_then(|c| c.take()).unwrap_or(actor);
    let (points, at_step) = (heard.borrow().points, heard.borrow().rests);
    let skeleton = Skeleton { points: points.map(|p| p.unwrap_or_default()), rests: at_step.unwrap_or(rests), main_rest: rests[0] };
    let mut rng = Rng::from_state(r.state());
    let tail = {
        let mut world = Replay(&heard);
        let mut scene = Replay(&heard);
        let scenery = Scenery { spheres: &spheres, world: &mut world, terrain, scene: &mut scene };
        port.tick(F(dt), &request, &skeleton, &tables, &wrecks, scenery, &mut rng, &mut Posed(&heard))
    };
    let h = heard.borrow();
    for (what, left) in [("ground queries", h.ground.len()), ("sphere tests", h.spheres.len()), ("sweeps", h.segments.len()), ("poses", h.poses.len())] {
        if left > 0 {
            out.push(format!("{left} of the original's {what} the port did not make"));
        }
    }
    let port_drew = draws(r.state(), rng.state(), 1000).unwrap_or(u32::MAX);
    if !(port_drew == drew || (port_drew == 0 && drew != 6)) {
        out.push(format!("the port drew {port_drew}, the original {drew}"));
    }
    let (want, got) = (written(&game), written(&port));
    for ((name, w), (_, g)) in want.iter().zip(&got) {
        if w != g {
            out.push(format!("{name}: {:e} want {:e}", f32::from_bits(*g), f32::from_bits(*w)));
        }
    }
    let port_tail = (tail.touching, bits(tail.impact), bits(tail.main));
    if port_tail != game_tail {
        out.push(format!("the tail {port_tail:x?} want {game_tail:x?}"));
    }
    out.extend(h.odd.iter().cloned());
    let moving = h.moving;
    drop(h);
    if let Some(c) = carried {
        *c = Some(port);
    }
    // The original's state goes on to the next tick, whatever the port made of this one.
    after.back(ee);
    (out, moving)
}

/// What the ticks compared went through, so that a pass on fewer fails.
#[derive(Debug, Default)]
struct Went {
    ticks: usize,
    entries: usize,
    touching: usize,
    off_bike: usize,
    flung: usize,
    landed: usize,
    settles: usize,
    settled: usize,
}

impl Went {
    fn add(&mut self, before: &Actor, after: &Ee) {
        let a = actor_of(after);
        let r = &a.ragdoll;
        self.ticks += 1;
        self.entries += usize::from(before.mode != Mode::Wrecked && a.mode == Mode::Wrecked);
        self.touching += usize::from(a.touching);
        self.off_bike += usize::from(r.off_bike && !before.ragdoll.off_bike);
        self.flung += usize::from(r.flung);
        self.landed += usize::from(before.ragdoll.flung && !r.flung);
        self.settles += usize::from(r.settled && !before.ragdoll.settled);
        self.settled += usize::from(r.settled);
    }
}

/// **The port's tick equals the original's on every tick of the s56 captures that the oracle
/// replays** (S61): from the same RAM, the port's actor update (the mode machine, the wreck's arm,
/// the step, the normalisation and the tail) and the original's, every number of every bone, pin,
/// joint and collision and every actor field the update writes compared bit for bit. The world's
/// answers (the ground, the scene's hits, the sweeps) and the skeleton's (its pose at the wreck
/// entry, the riding pose's points) are the original's own in the same tick, listened to.
#[test]
fn the_ports_tick_is_the_originals() {
    let mut went = Went::default();
    for (w, (name, to)) in WRECKS.iter().zip(EXACT_TO) {
        let Some(cap) = Capture::load(w.name) else { return };
        let Some(mut ee) = state(w) else { return };
        for frame in w.frame + 1..=to {
            if repeated(&cap, frame) {
                continue;
            }
            let before = Snapshot::of(&ee);
            let (n, drew) = originals_tick(&mut ee, &cap, frame).unwrap_or_else(|d| panic!("{name}: {frame}: {d:?}"));
            before.back(&mut ee);
            lay_in(&mut ee, &cap, frame);
            let was = actor_of(&ee);
            let (d, _) = port_tick(&mut ee, &cap, frame, n, Some(drew), None);
            assert!(d.is_empty(), "{name}: tick {frame} parts in {} numbers: {:#?}", d.len(), &d[..d.len().min(12)]);
            went.add(&was, &ee);
        }
    }
    eprintln!("{went:?}");
    assert!(went.ticks >= 35 && went.entries == 1 && went.touching > 0, "{went:?}");
}

/// **The port's tick equals the original's past the replay**, to the end of each capture: the
/// original runs on from the last tick the capture replays with the bike's side still laid in from
/// the capture (its bones no longer the capture's, since the bike's collider the capture does not
/// carry has touched them), and from each tick's RAM the port's tick is compared with it as above.
/// This is where the bodies come off the bike, land and come to rest.
#[test]
fn the_ports_tick_is_the_originals_to_rest() {
    let mut went = Went::default();
    let mut hits = 0;
    for (w, (name, to)) in WRECKS.iter().zip(EXACT_TO) {
        let Some(cap) = Capture::load(w.name) else { return };
        let Some(mut ee) = state(w) else { return };
        let mut frame = w.frame + 1;
        while cap.has(frame) {
            if repeated(&cap, frame) {
                frame += 1;
                continue;
            }
            if frame <= to {
                originals_tick(&mut ee, &cap, frame).unwrap_or_else(|d| panic!("{name}: {frame}: {d:?}"));
            } else {
                lay_in(&mut ee, &cap, frame);
                let was = actor_of(&ee);
                let (d, moving) = port_tick(&mut ee, &cap, frame, 0, None, None);
                hits += usize::from(moving > 0);
                if !d.is_empty() {
                    note(&format!("{name} {frame}:\n  {}", d.join("\n  ")));
                }
                assert!(d.is_empty(), "{name}: tick {frame} parts in {} numbers: {:#?}", d.len(), &d[..d.len().min(12)]);
                went.add(&was, &ee);
            }
            frame += 1;
        }
    }
    eprintln!("{went:?}, {hits} ticks with a moving hit");
    // Every capture comes to rest: the settle test throws in all four, and the flung, landed and
    // settled arms each run.
    assert!(went.ticks >= 850 && went.settles == 4, "{went:?}");
    assert!(went.flung > 0 && went.landed > 0 && went.settled > 0 && went.off_bike > 0, "{went:?}");
    // The ticks S61 left out, a bone's hit against another collidable (S62): compared too.
    assert!(hits >= 12, "{hits} ticks with a moving hit");
}

/// S62's captures (`research/s62_wreck.py`, context/capture/s62): each wreck's first state, and the
/// last tick the original's own update holds every captured bone through with the wreck bike's
/// object and the broad phase's pools laid in (tests/ragdoll_bike.rs) and the bike kept in the
/// track's tree where the game keeps it (S113: front stopped at 2627 without it).
const S62: [(&str, u32, u32); 5] =
    [("front", 2602, 2888), ("side", 2607, 3146), ("back", 2477, 3179), ("tip", 2472, 3179), ("front3", 2630, 2687)];

/// **The port replays each s62 wreck through the bike to the rider at rest** (S62): from each
/// wreck's first state the port's actor runs on its own state, tick after tick, handed only the
/// world's answers the original heard (the ground, the scene's hits and what each is with, the
/// sweeps, the skeleton), and every number the update writes is compared with the original's tick,
/// bit for bit. Up to the last tick the original holds the capture through, that is the game's own
/// wreck; past it (front's last 292 ticks) the original runs on in the oracle from its own state.
#[test]
fn the_port_replays_the_wrecks_through_the_bike() {
    let mut went = Went::default();
    let (mut hits, mut exact) = (0, 0);
    for (name, start, to) in S62 {
        let w = Wreck { name, frame: start };
        let Some(cap) = Capture::load_in("s62", name) else { return };
        let Some(mut ee) = state_in("s62", &w) else { return };
        let mut port = None;
        let settles = went.settles;
        let mut frame = start + 1;
        while cap.has(frame) {
            if repeated(&cap, frame) {
                frame += 1;
                continue;
            }
            let (n, drew) = if frame <= to {
                let before = Snapshot::of(&ee);
                let (n, drew) = originals_tick(&mut ee, &cap, frame).unwrap_or_else(|d| panic!("{name}: {frame}: {:?}", &d[..d.len().min(4)]));
                before.back(&mut ee);
                exact += 1;
                (n, Some(drew))
            } else {
                (0, None)
            };
            lay_in(&mut ee, &cap, frame);
            let was = actor_of(&ee);
            let (d, moving) = port_tick(&mut ee, &cap, frame, n, drew, Some(&mut port));
            hits += usize::from(moving > 0);
            if !d.is_empty() {
                note(&format!("s62 {name} {frame}:\n  {}", d.join("\n  ")));
            }
            assert!(d.is_empty(), "{name}: tick {frame} parts in {} numbers: {:#?}", d.len(), &d[..d.len().min(12)]);
            went.add(&was, &ee);
            frame += 1;
        }
        eprintln!("{name}: {start}..{}, the capture's own to {to}, settles {}", frame - 1, went.settles - settles);
    }
    eprintln!("{went:?}, {exact} ticks the capture's own, {hits} with a moving hit");
    // Every wreck through to the bones at rest, each settling once (front3's state is from inside its
    // wreck, so four entries), and the hits against the wreck bike's body: told they
    // were with the still scenery instead, the port parts at front's 2611.
    assert!(went.ticks >= 2600 && exact >= 2000 && went.entries == 4 && went.settles == 5 && went.settled > 2500, "{went:?}, {exact} exact");
    assert!(hits >= 11, "{hits} ticks with a moving hit");
}

// --- S105: the bones' spheres against the wreck bike's mesh ----------------------------------------

/// `wreckbikef.col` off the disc, as the shell loads it, or `None` (skipped) without a disc.
fn wreck_bike_mesh() -> Option<game_sim::ragdoll_scene::Bike> {
    use game_render::disc::Disc;
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let names: Vec<String> =
        disc.entries().iter().filter(|e| e.name.to_ascii_lowercase().ends_with(".res")).map(|e| e.name.clone()).collect();
    let col = names.iter().find_map(|n| disc.res(n).ok().and_then(|r| r.read_suffix("wreckbikef.col").ok()));
    Some(game_sim::ragdoll_scene::Bike::parse(&col.expect("an archive carries wreckbikef.col")).expect("the file reads as a mesh"))
}

/// A quad as it lies in RAM, nothing flushed.
fn raw_quad(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| f32::from_bits(ee.u32_at(a + k))))
}

/// The wreck bike's collidable (the request's `+0x1ac`) and its frame (`+0x120`, rows then origin).
fn bike_collidable(ee: &Ee) -> (u32, game_sim::contact::Node) {
    let b = ee.u32_at(ee.u32_at(body_actor() + 0x488) + 0x1ac);
    let frame = game_sim::contact::Node { rows: [0, 1, 2].map(|k| raw_quad(ee, b + 0x120 + 0x10 * k)), origin: raw_quad(ee, b + 0x150) };
    (b, frame)
}

/// **The port's mesh is the one the original tests against** (S105): `wreckbikef.col` as the port
/// reads it, every node (children, centre, half extent), every triangle (corners, distance, the
/// normal in the loader's 16-bit form) and every vertex, equal to the wreck bike collidable's
/// arrays in RAM word for word.
#[test]
fn the_wreck_bike_mesh_is_the_ones_in_ram() {
    let Some(bike) = wreck_bike_mesh() else { return };
    let Some(ee) = state_in("s62", &Wreck { name: "front", frame: 2603 }) else { return };
    let (b, _) = bike_collidable(&ee);
    assert_eq!(ee.u32_at(b + 0x68), 0, "the wreck bike's collidable is a mesh");
    let (nodes, triangles, vertices) = (ee.u32_at(b + 0x234), ee.u32_at(b + 0x238), ee.u32_at(b + 0x220));
    assert_eq!(ee.u32_at(b + 0x228), 0, "wide vertices");
    let m = &bike.mesh;
    for (i, n) in m.nodes.iter().enumerate() {
        let at = nodes + 0x24 * i as u32;
        assert_eq!([ee.u16_at(at + 8), ee.u16_at(at + 10)], n.children, "node {i}'s children");
        for k in 0..3 {
            assert_eq!(ee.u32_at(at + 0xc + 4 * k as u32), n.centre[k].0.to_bits(), "node {i}'s centre");
            assert_eq!(ee.u32_at(at + 0x18 + 4 * k as u32), n.half_extent[k].0.to_bits(), "node {i}'s half extent");
        }
    }
    for (i, (t, packed)) in m.triangles.iter().zip(&bike.normals).enumerate() {
        let at = triangles + 0x14 * i as u32;
        assert_eq!([0, 2, 4].map(|k| ee.u16_at(at + k)), t.vertices, "triangle {i}'s corners");
        assert_eq!(ee.u32_at(at + 8), t.distance.0.to_bits(), "triangle {i}'s distance");
        assert_eq!([0xc, 0xe, 0x10].map(|k| ee.u16_at(at + k) as i16), *packed, "triangle {i}'s normal");
    }
    let game_sim::track::Points::Wide(v) = &m.vertices else { panic!("the file's vertices are packed") };
    for (i, p) in v.iter().enumerate() {
        for k in 0..3 {
            assert_eq!(ee.u32_at(vertices + 12 * i as u32 + 4 * k as u32), p[k].0.to_bits(), "vertex {i}");
        }
    }
    eprintln!("{} nodes, {} triangles, {} vertices", m.nodes.len(), m.triangles.len(), v.len());
}

/// The port's answer for a sphere at `centre` of `radius` against the bike in `frame`, as the
/// record's world contact, and the original's after its pair ran: `(point, normal)` bits.
type Answer = Option<([u32; 4], [u32; 4])>;

fn ports_answer(bike: &game_sim::ragdoll_scene::Bike, frame: &game_sim::contact::Node, centre: Vf, radius: f32) -> Answer {
    bike.touch(frame, centre, ps2_float::F(radius)).map(|t| (bits(t.world.0), bits(t.world.1)))
}

/// **The sphere test answers as the original's, over generated spheres** (S105): in front's 2603
/// state, 20,000 spheres stood near the wreck bike (round a random vertex, a random way off, radii
/// from a tenth to a metre and the bones' own three), each through the original's pair on a bone's
/// sphere collider with an emptied record, and through the port: hit or not, the plane distance,
/// and the contact's point and normal in the world, bit for bit.
#[test]
fn the_sphere_test_answers_as_the_originals() {
    let Some(bike) = wreck_bike_mesh() else { return };
    let Some(mut ee) = state_in("s62", &Wreck { name: "front", frame: 2603 }) else { return };
    let (b, frame) = bike_collidable(&ee);
    let sphere = ee.u32_at(collision(&ee, 0) + coll_at::SPHERE);
    let (clear, pair) = (addr("sphere_collider_clear").unwrap(), addr("sphere_collider_pair").unwrap());
    let game_sim::track::Points::Wide(v) = &bike.mesh.vertices else { panic!("packed") };
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut next = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 40) as f32) / (1u64 << 24) as f32
    };
    let mut hits = 0;
    for i in 0..20_000 {
        let p = frame.place(Vf::new(v[i % v.len()][0].0, v[i % v.len()][1].0, v[i % v.len()][2].0, 1.0));
        let radius = match i % 5 {
            0 => 0.4f32,
            1 => 0.75,
            2 => 0.5,
            _ => 0.1 + 0.9 * next(),
        };
        let off = Vf::new(next() - 0.5, next() - 0.5, next() - 0.5, 0.0);
        let reach = 4.0 * radius * next();
        let centre = Vf::new(p.x() + off.x() * reach, p.y() + off.y() * reach, p.z() + off.z() * reach, 1.0);
        for at in [0x40, 0x120] {
            ee.write(sphere + at, 16, u128::from_le_bytes(bytemuck_quad(centre)));
        }
        ee.set_f32(sphere + 0x170, radius);
        ee.set_f32(sphere + 0x174, ps2_float::fpu::mul(radius, radius));
        ee.call(clear, &[u64::from(sphere)], &[], 1_000_000);
        ee.call(pair, &[u64::from(sphere), u64::from(b)], &[], 50_000_000);
        let rec = ee.u32_at(sphere + coll_at::RECORD);
        let hit = ee.v0() != 0;
        let original: Answer = hit.then(|| {
            assert_eq!(ee.u32_at(rec + 4), 1, "{i}: one contact");
            assert_eq!(ee.u32_at(rec + 0x90), b, "{i}: with the bike");
            (bits(raw_quad(&ee, rec + 0x60)), bits(raw_quad(&ee, rec + 0x30)))
        });
        let port = ports_answer(&bike, &frame, centre, radius);
        assert_eq!(port, original, "{i}: sphere at {centre:?} r {radius}");
        if hit {
            let t = bike.touch(&frame, centre, ps2_float::F(radius)).unwrap();
            assert_eq!(t.distance.0.to_bits(), ee.u32_at(rec + 8), "{i}: the plane distance");
            hits += 1;
        }
    }
    eprintln!("{hits} of 20000 hit");
    assert!(hits > 4000 && hits < 16000, "{hits} hits");
}

fn bytemuck_quad(v: Vf) -> [u8; 16] {
    let mut out = [0; 16];
    for k in 0..4 {
        out[4 * k..4 * k + 4].copy_from_slice(&v[k].to_bits().to_le_bytes());
    }
    out
}

/// The bones' search as the original holds it (the one the actor's `+0x628` points at): where the
/// first collision's sphere stands, the list's centre, bound and built flag, and the bike's place in the list.
fn search_in_ram(ee: &Ee, b: u32, sphere: u32) -> BikeSearch {
    let s = ee.u32_at(body_actor() + 0x628);
    let n = ee.u32_at(s + 4);
    let found = (0..n).map(|i| ee.u32_at(s) + 8 * i).find(|&at| ee.u32_at(at) == b);
    BikeSearch {
        stood: Some(raw_quad(ee, sphere + 0x40)),
        centre: raw_quad(ee, s + 0x10),
        bound: F(ee.f32_at(s + 0x20)),
        built: ee.u32_at(s + 0x30) != 0,
        listed: found.is_some(),
        distance: found.map_or(F::ZERO, |at| F(ee.f32_at(at + 4))),
    }
}

/// The track's tree as the port lays it over the one in RAM's box (the bones' first sphere's
/// space's table's static structure), its origin, extent, cell and scale held equal to RAM's.
fn cells_of_ram(ee: &Ee, sphere: u32) -> (Cells, u32) {
    let tree = ee.u32_at(ee.u32_at(ee.u32_at(sphere + 4) + 4) + 0x34);
    let origin = [F(ee.f32_at(tree + 0x30)), F(ee.f32_at(tree + 0x38))];
    let extent = [F(ee.f32_at(tree + 0x48)), F(ee.f32_at(tree + 0x4c))];
    let cells = Cells::over(origin, [origin[0] + extent[0], origin[1] + extent[1]]);
    let t = &cells.tree;
    assert_eq!((t.origin, t.extent, t.cell, t.scale), (origin, extent, ee.i32_at(tree + 0x58), F(ee.f32_at(tree + 0x5c))), "the tree");
    (cells, tree)
}

/// Whether the tree under `node` holds `o` in one of its nodes' chunks of four.
fn tree_holds(ee: &Ee, node: u32, o: u32) -> bool {
    // Laying the capture in can leave a node pointing at what is no node in the Ee.
    let ram = |a: u32| (0x10_0000..0x200_0000).contains(&a) && a % 4 == 0;
    if !ram(node) {
        return false;
    }
    let mut c = ee.u32_at(node + 4);
    while c != 0 && ram(c) {
        if (1..5).any(|k| ee.u32_at(c + 4 * k) == o) {
            return true;
        }
        c = ee.u32_at(c);
    }
    let children = ee.u32_at(node);
    ram(children) && (0..4).any(|i| ee.u32_at(children + 4 * i) != 0 && tree_holds(ee, ee.u32_at(children + 4 * i), o))
}

/// The paths (child indices from the root) of the nodes under `node` that hold `o`.
fn paths_holding(ee: &Ee, node: u32, o: u32, path: &mut Vec<u32>, out: &mut Vec<Vec<u32>>) {
    let ram = |a: u32| (0x10_0000..0x200_0000).contains(&a) && a % 4 == 0;
    if !ram(node) {
        return;
    }
    let mut c = ee.u32_at(node + 4);
    while c != 0 && ram(c) {
        if (1..5).any(|k| ee.u32_at(c + 4 * k) == o) {
            out.push(path.clone());
            break;
        }
        c = ee.u32_at(c);
    }
    let children = ee.u32_at(node);
    if ram(children) {
        for i in 0..4 {
            let n = ee.u32_at(children + 4 * i);
            if n != 0 {
                path.push(i);
                paths_holding(ee, n, o, path, out);
                path.pop();
            }
        }
    }
}

/// The key of the nodes the Ee's tree holds the bike in, read back from their paths (child `2x + z`
/// of the bit each level halves, `0x4000` first), as `broad_build::insert` reads a key: the least
/// cell, a corner bit for each node, the level. `None` when no node holds it.
fn bike_key_in_tree(ee: &Ee, b: u32, tree: u32) -> Option<u32> {
    let mut paths = Vec::new();
    paths_holding(ee, ee.u32_at(tree + 0x94), b, &mut Vec::new(), &mut paths);
    let level = paths.first()?.len() as u32 - 1;
    assert!(paths.iter().all(|p| p.len() as u32 == level + 1), "the bike's nodes are on one level: {paths:?}");
    let cell = |p: &Vec<u32>| p.iter().enumerate().fold((0, 0), |(x, z), (l, &i)| (x | (i >> 1) * (0x4000 >> l), z | (i & 1) * (0x4000 >> l)));
    let cells: Vec<(u32, u32)> = paths.iter().map(cell).collect();
    let (x, z) = (cells.iter().map(|c| c.0).min()?, cells.iter().map(|c| c.1).min()?);
    let size = 0x4000 >> level;
    let corners = cells.iter().fold(0, |k, &(cx, cz)| {
        assert!(cx - x <= size && cz - z <= size, "the bike's nodes are a block of four: {paths:?}");
        k | 0x10 << (2 * u32::from(cx != x) + u32::from(cz != z))
    });
    Some((x << 17) & 0xfff0_0000 | (z << 5) & 0xf_ff00 | corners | level)
}

/// The bike's key in the tree as RAM holds it (its placement's `+0xc`), or `None` when it is in
/// none of the tree's nodes.
fn bike_key_in_ram(ee: &Ee, b: u32, tree: u32) -> Option<u32> {
    let place = ee.u32_at(b + 4);
    (place != 0 && tree_holds(ee, ee.u32_at(tree + 0x94), b)).then(|| ee.u32_at(place + 0xc))
}

/// **The game keeps the wreck bike in the track's tree where its sphere stands** (S105): in every
/// one of the twenty s62 states where the bike is in the broad phase, its placement is in the
/// track's tree, at its sphere's centre (its mesh's root box's, through its frame), under the key the
/// port's [`Cells::key`] gives that box, and the tree holds it there. In the four late states (after the put-back, S108) the bike
/// is in no structure at all.
#[test]
fn the_bike_is_in_the_tree_where_its_sphere_stands() {
    let Some(bike) = wreck_bike_mesh() else { return };
    let states = [
        ("back", [2477, 2478, 2479, 2877]),
        ("front", [2602, 2603, 2604, 3002]),
        ("front3", [2630, 2631, 2632, 2668]),
        ("side", [2607, 2608, 2609, 3007]),
        ("tip", [2472, 2473, 2474, 2872]),
    ];
    let (mut placed, mut out) = (0, 0);
    for (name, frames) in states {
        for frame in frames {
            let Some(ee) = state_in("s62", &Wreck { name, frame }) else { return };
            let (b, _) = bike_collidable(&ee);
            let sphere = ee.u32_at(collision(&ee, 0) + coll_at::SPHERE);
            let (cells, tree) = cells_of_ram(&ee, sphere);
            let place = ee.u32_at(b + 4);
            if place == 0 {
                out += 1;
                continue;
            }
            assert_eq!(ee.u32_at(place + 8), tree, "{name} {frame}: in the track's tree");
            assert_eq!(bits(raw_quad(&ee, place + 0x10)), bits(raw_quad(&ee, b + 0x40)), "{name} {frame}: at its sphere");
            let (_, at) = bike_collidable(&ee);
            let m = Mat4([at.rows[0], at.rows[1], at.rows[2], at.origin]);
            let (c, _) = game_sim::flight_search::bike_box(&bike.mesh, &m);
            assert_eq!(c.map(|v| v.0.to_bits()), bits(raw_quad(&ee, b + 0x40)), "{name} {frame}: its box's centre is its sphere's");
            let key = cells.key(&bike, &m);
            assert_eq!(ee.u32_at(place + 0xc), key, "{name} {frame}: the key");
            assert_eq!(bike_key_in_ram(&ee, b, tree), Some(key), "{name} {frame}: the tree holds it");
            assert_eq!(bike_key_in_tree(&ee, b, tree), Some(key), "{name} {frame}: its nodes read back to its key");
            placed += 1;
        }
    }
    eprintln!("{placed} states with the bike in the tree, {out} with it in none");
    assert_eq!((placed, out), (16, 4));
}

/// The game manager singleton, whose human player's vehicle is `+0x30 -> +0x34 -> +0 -> +0x48 -> +0`.
const GAME_MANAGER: u32 = 0x004e_1e98;

/// **The bike's unwreck takes the wreck bike out of the broad phase** (S108): in each s62 wreck's
/// last entry state the bike is in the track's tree, placed (`B+4`) and gated in for the pair
/// (`B+0x31` bit 0, which the sphere pair asks first). The vehicle's wreck part (`+0x7a8`) holds it
/// at `+0x128`. The original's unwreck, run on that state as every put-back runs it, stops the wreck
/// part, and after it the bike is in no node of the tree, its placement is 0 and its gate is down:
/// what the s62 captures show from each put-back's row on (the put-back's own tick's gather is
/// `every_sphere_test_of_the_wrecks_is_the_bike_meshs`'s).
#[test]
fn the_unwreck_takes_the_wreck_bike_out_of_the_broad_phase() {
    let Some(unwreck) = addr("bike_unwreck").ok() else { return };
    let Some(stop) = addr("wreck_part_stop").ok() else { return };
    for (name, frame) in [("back", 2479), ("front", 2604), ("front3", 2632), ("side", 2609), ("tip", 2474)] {
        let Some(mut ee) = state_in("s62", &Wreck { name, frame }) else { return };
        let (b, _) = bike_collidable(&ee);
        let sphere = ee.u32_at(collision(&ee, 0) + coll_at::SPHERE);
        let (_, tree) = cells_of_ram(&ee, sphere);
        let player = ee.u32_at(ee.u32_at(ee.u32_at(ee.u32_at(GAME_MANAGER) + 0x30) + 0x34));
        let vehicle = ee.u32_at(ee.u32_at(player + 0x48));
        let part = ee.u32_at(vehicle + 0x7a8);
        assert_eq!(ee.u32_at(part + 0x128), b, "{name} {frame}: the wreck part's collidable is the bike");
        assert_eq!(ee.u32_at(ee.u32_at(part + 0x38) + 0x120), stop, "{name} {frame}: the unwreck's call is the stop");
        assert!(ee.u32_at(b + 4) != 0 && ee.u8_at(b + 0x31) & 1 != 0, "{name} {frame}: placed and gated in");
        assert!(tree_holds(&ee, ee.u32_at(tree + 0x94), b), "{name} {frame}: in the tree");
        ee.call(unwreck, &[u64::from(vehicle), 1], &[], 200_000_000);
        assert_eq!((ee.u32_at(b + 4), ee.u8_at(b + 0x31) & 1), (0, 0), "{name} {frame}: unplaced and gated out");
        assert!(!tree_holds(&ee, ee.u32_at(tree + 0x94), b), "{name} {frame}: in no node of the tree");
    }
}

/// **Every sphere test of the five s62 wrecks, answered by the port** (S105): the original's update
/// runs each wreck as S62's replay does, and the port's search for the bike ([`BikeSearch`]) is
/// carried beside it from the wreck's first state: gathered each frame where the first sphere stood
/// (offered the bike once the bones are off it), and after the update equal to the original's search
/// (centre, bound, built, the bike listed and its distance), bit for bit. At each of the bones' sphere
/// tests the port pairs the sphere with the bike exactly when the original's search does, and tests
/// it against the mesh in the bike collidable's frame of that moment: every test the original
/// answered with a hit is the port's hit with the same world point and normal, and every miss a miss.
#[test]
fn every_sphere_test_of_the_wrecks_is_the_bike_meshs() {
    let Some(bike) = wreck_bike_mesh() else { return };
    let bike = Rc::new(bike);
    #[derive(Default)]
    struct Count {
        tests: usize,
        hits: usize,
        paired: usize,
        gathers: usize,
        rebuilt: usize,
        listed: usize,
        /// The pair calls with the bike in the sphere test running now.
        now: Vec<u32>,
        /// The gathers with the bike out of the broad phase (S108).
        gone: Vec<String>,
        wrong: Vec<String>,
    }
    let count = Rc::new(RefCell::new(Count::default()));
    let update = addr("rider_body_update").unwrap();
    for (name, start, to) in S62 {
        let w = Wreck { name, frame: start };
        let Some(cap) = Capture::load_in("s62", name) else { return };
        let Some(mut ee) = state_in("s62", &w) else { return };
        let spheres: Vec<u32> = (0..9).map(|i| ee.u32_at(collision(&ee, i) + coll_at::SPHERE)).collect();
        let (b, _) = bike_collidable(&ee);
        let (cells, tree) = cells_of_ram(&ee, spheres[0]);
        let model = Rc::new(RefCell::new(search_in_ram(&ee, b, spheres[0])));
        let mut frame = start + 1;
        while cap.has(frame) {
            if repeated(&cap, frame) {
                frame += 1;
                continue;
            }
            let n = if frame <= to {
                let before = Snapshot::of(&ee);
                let (n, _) = originals_tick(&mut ee, &cap, frame).unwrap_or_else(|d| panic!("{name}: {frame}: {:?}", &d[..d.len().min(4)]));
                before.back(&mut ee);
                n
            } else {
                0
            };
            lay_in(&mut ee, &cap, frame);
            let mut r = Rng::from_state(cap.int(frame - 1, "rng_state"));
            (0..n).for_each(|_| {
                r.rand();
            });
            set_rng(&mut ee, r.state());
            let saved = std::mem::take(&mut ee.hooks);
            let centre = Rc::new(RefCell::new(Vf::default()));
            let c = Rc::clone(&centre);
            ee.hooks.insert(
                addr("collider_set_position").unwrap(),
                Box::new(move |ee: &mut Ee| {
                    *c.borrow_mut() = raw_quad(ee, ee.r(5) as u32);
                    through(ee);
                }),
            );
            let (k, c) = (Rc::clone(&count), Rc::clone(&centre));
            let spheres_at = spheres.clone();
            ee.hooks.insert(
                addr("sphere_collider_pair").unwrap(),
                Box::new(move |ee: &mut Ee| {
                    let (s, o) = (ee.r(4) as u32, ee.r(5) as u32);
                    if spheres_at.contains(&s) && o == b {
                        let mut k = k.borrow_mut();
                        k.paired += 1;
                        k.now.push(s);
                        let rec = ee.u32_at(s + coll_at::RECORD);
                        if bits(raw_quad(ee, s + 0x40)) != bits(*c.borrow()) || ee.u32_at(rec + 4) != 0 {
                            k.wrong.push(format!("pair: the sphere's centre or a record not empty"));
                        }
                    }
                    through(ee);
                }),
            );
            let collide = |k: Rc<RefCell<Count>>, c: Rc<RefCell<Vf>>, spheres: Vec<u32>, bike: Rc<game_sim::ragdoll_scene::Bike>, model: Rc<RefCell<BikeSearch>>| {
                Box::new(move |ee: &mut Ee| {
                    let s = ee.r(4) as u32;
                    if !spheres.contains(&s) {
                        return through(ee);
                    }
                    let radius = f32::from_bits(ee.u32_at(s + 0x170));
                    let reaches = model.borrow().reaches(F(ee.f32_at(s + 0x60)));
                    let (_, at) = bike_collidable(ee);
                    let port = if reaches { ports_answer(&bike, &at, *c.borrow(), radius) } else { None };
                    k.borrow_mut().now.clear();
                    through(ee);
                    let hit = ee.u8_at(s + coll_at::FLAGS) & 0x20 != 0;
                    let rec = ee.u32_at(s + coll_at::RECORD);
                    let mut k = k.borrow_mut();
                    k.tests += 1;
                    let original: Answer = hit.then(|| {
                        if ee.u32_at(rec + 4) != 1 || ee.u32_at(rec + 0x90) != b {
                            k.wrong.push(format!("{} contacts, with {:#x}", ee.u32_at(rec + 4), ee.u32_at(rec + 0x90)));
                        }
                        (bits(raw_quad(ee, rec + 0x60)), bits(raw_quad(ee, rec + 0x30)))
                    });
                    k.hits += usize::from(hit);
                    let paired = k.now.contains(&s);
                    if paired != reaches || port != original {
                        k.wrong.push(format!(
                            "sphere {s:#x} at {:?} r {radius}: paired {paired}, the port's search {:?}; port {port:x?}, original {original:x?}",
                            *c.borrow(),
                            *model.borrow(),
                        ));
                    }
                }) as Box<dyn FnMut(&mut Ee)>
            };
            for hook in ["collide_against_search", "probe_collide_world"] {
                let f = collide(Rc::clone(&count), Rc::clone(&centre), spheres.clone(), Rc::clone(&bike), Rc::clone(&model));
                ee.hooks.insert(addr(hook).unwrap(), f);
            }
            // The port's gather where the original's runs: once a frame in the wreck mode only (a
            // body put back on the bike stops), round the first sphere where it stands, the bike
            // offered once the bones are off it, and the bike where the Ee's tree holds it: where
            // the game's own frame puts it (place_bike, S113), under the key the port's cells give
            // its laid-in box. Before S113 the tree lost the bike from front's 2619 on, and the
            // rebuild at 2628 left it out.
            let (m, k, cells, s0, bk) = (Rc::clone(&model), Rc::clone(&count), cells.clone(), spheres[0], Rc::clone(&bike));
            ee.hooks.insert(
                addr("collision_search_gather").unwrap(),
                Box::new(move |ee: &mut Ee| {
                    let place = ee.u32_at(b + 4);
                    if place != 0 {
                        let (_, at) = bike_collidable(ee);
                        let frame = Mat4([at.rows[0], at.rows[1], at.rows[2], at.origin]);
                        if cells.key(&bk, &frame) != ee.u32_at(place + 0xc) {
                            k.borrow_mut().wrong.push(format!("the bike placed under another key than the port's"));
                        }
                    }
                    let key = (place != 0).then(|| ee.u32_at(place + 0xc)).filter(|&k| tree_holds_at(ee, tree, b, k));
                    let mut m = m.borrow_mut();
                    let before = *m;
                    m.stood = Some(raw_quad(ee, s0 + 0x40));
                    let reach = F(ee.f32_at(s0 + 0x60));
                    let bike_sphere = (raw_quad(ee, b + 0x40), F(ee.f32_at(b + 0x60)));
                    let offered = ee.u32_at(body_actor() + 0x6b0) != 0 && key.is_some();
                    // The put-back's tick (S108): the bike's unwreck has taken it out of the tree and
                    // lowered the pair's gate on it before the body's update gathers.
                    let gone = ee.u32_at(b + 4) == 0;
                    if gone {
                        k.borrow_mut().gone.push(format!("{name} {frame}"));
                        if tree_holds(ee, ee.u32_at(tree + 0x94), b) || ee.u8_at(b + 0x31) & 1 != 0 {
                            k.borrow_mut().wrong.push(format!("a bike out of the broad phase still in the tree or gated in"));
                        }
                    }
                    m.gather(Some(&cells), reach, bike_sphere, key.unwrap_or(0), offered);
                    let mut k = k.borrow_mut();
                    k.gathers += 1;
                    k.rebuilt += usize::from(bits(m.centre) != bits(before.centre) || m.bound.0.to_bits() != before.bound.0.to_bits());
                    k.listed += usize::from(m.listed);
                    through(ee);
                }),
            );
            ee.call(update, &[u64::from(body_actor())], &[cap.get(frame, "dt")], 2_000_000_000);
            ee.hooks = saved;
            let mut theirs = search_in_ram(&ee, b, spheres[0]);
            let ours = *model.borrow();
            theirs.stood = ours.stood;
            if !theirs.listed {
                // A distance means nothing for a bike not in the list.
                theirs.distance = ours.distance;
            }
            let k = count.borrow();
            assert_eq!(format!("{ours:?}"), format!("{theirs:?}"), "{name} {frame}: the search");
            assert!(k.wrong.is_empty(), "{name} {frame}: {:#?}", &k.wrong[..k.wrong.len().min(6)]);
            drop(k);
            frame += 1;
        }
    }
    let k = count.borrow();
    eprintln!(
        "{} sphere tests, {} paired with the bike, {} hits; {} gathers, {} rebuilt, {} with the bike listed",
        k.tests, k.paired, k.hits, k.gathers, k.rebuilt, k.listed
    );
    let counts = (k.tests, k.paired, k.hits, k.gathers, k.rebuilt, k.listed);
    assert_eq!(counts, (24822, 852, 84, 367, 12, 230), "tests, paired, hits, gathers, rebuilt, listed");
    // One gather each wreck after the bike has left the broad phase: the put-back's tick (S108).
    assert_eq!(k.gone, ["front 2681", "side 2686", "back 2556", "tip 2551", "front3 2681"], "the gathers with the bike gone");
}

// --- S56 (reopened): a wreck at walking pace ------------------------------------------------------

/// The roll-out: the standing bike on recording2's state, thrown up and rolled about its own forward
/// axis until the airborne roll limit wrecks it (reason 8) at 2.3 ft/s, the pad idle. On the ground
/// the ridden bike's roll freedom is off (the ground step's chassis flags), so a standing or crawling
/// bike cannot tip over at all; this is the slowest wreck the original has. Its first state, the last
/// tick the original's update holds the capture through, and the capture's last row.
const ROLL: (&str, u32, u32) = ("roll", 2481, 3180);

/// **The port replays the slow roll-out to the rider at rest** (S56): as
/// `the_port_replays_the_wrecks_through_the_bike`, from the roll's first state, the port's actor on
/// its own state against the original's update every tick, and the original against the capture.
#[test]
fn the_port_replays_the_slow_roll_out() {
    let (name, start, to) = ROLL;
    let Some(cap) = Capture::load_in("s62", name) else { return };
    let Some(mut ee) = state_in("s62", &Wreck { name, frame: start }) else { return };
    let (mut went, mut port, mut exact, mut hits) = (Went::default(), None, 0, 0);
    let mut frame = start + 1;
    while cap.has(frame) {
        if repeated(&cap, frame) {
            frame += 1;
            continue;
        }
        let (n, drew) = if frame <= to {
            let before = Snapshot::of(&ee);
            let (n, drew) = originals_tick(&mut ee, &cap, frame).unwrap_or_else(|d| panic!("{name}: {frame}: {:?}", &d[..d.len().min(4)]));
            before.back(&mut ee);
            exact += 1;
            (n, Some(drew))
        } else {
            (0, None)
        };
        lay_in(&mut ee, &cap, frame);
        let was = actor_of(&ee);
        let (d, moving) = port_tick(&mut ee, &cap, frame, n, drew, Some(&mut port));
        hits += usize::from(moving > 0);
        assert!(d.is_empty(), "{name}: tick {frame} parts in {} numbers: {:#?}", d.len(), &d[..d.len().min(12)]);
        went.add(&was, &ee);
        frame += 1;
    }
    eprintln!("{name}: {start}..{}, the capture's own to {to}: {went:?}, {exact} exact, {hits} with a moving hit", frame - 1);
    // The state is from the entry's own tick (the reason is first read there), so no entry is compared;
    // off the bike once, settled 22 ticks in and still through the put-back to the capture's end.
    assert!(went.ticks == 699 && exact == 699 && went.off_bike == 1 && went.settles == 1 && went.settled == 677, "{went:?}, {exact} exact");
}


// --- S113: front's 2628, the bike the tree lost ---------------------------------------------------

/// What the original's update did with the wreck bike in one tick: its pairs with a bone's sphere,
/// and each sphere test answered with a hit on it (the sphere, the record's world normal).
#[derive(Default)]
struct BikeTick {
    pairs: usize,
    hits: Vec<(u32, Vf)>,
    rebuilt: bool,
}

/// One tick of the original's update with the draws before it `n`, the bike's pairs and hits heard.
fn bike_tick(ee: &mut Ee, cap: &Capture, frame: u32, n: u32, placed: bool) -> BikeTick {
    lay_in_as(ee, cap, frame, placed);
    let mut r = Rng::from_state(cap.int(frame - 1, "rng_state"));
    (0..n).for_each(|_| {
        r.rand();
    });
    set_rng(ee, r.state());
    let (b, _) = bike_collidable(ee);
    let spheres: Vec<u32> = (0..9).map(|i| ee.u32_at(collision(ee, i) + coll_at::SPHERE)).collect();
    let search = ee.u32_at(body_actor() + 0x628);
    let centre = raw_quad(ee, search + 0x10);
    let heard = Rc::new(RefCell::new(BikeTick::default()));
    let h = Rc::clone(&heard);
    ee.hooks.insert(
        addr("sphere_collider_pair").unwrap(),
        Box::new(move |ee: &mut Ee| {
            h.borrow_mut().pairs += usize::from(ee.r(5) as u32 == b);
            through(ee);
        }),
    );
    let h = Rc::clone(&heard);
    ee.hooks.insert(
        addr("collide_against_search").unwrap(),
        Box::new(move |ee: &mut Ee| {
            let s = ee.r(4) as u32;
            through(ee);
            let rec = ee.u32_at(s + coll_at::RECORD);
            if spheres.contains(&s) && ee.u8_at(s + coll_at::FLAGS) & 0x20 != 0 && ee.u32_at(rec + 0x90) == b {
                h.borrow_mut().hits.push((s, raw_quad(ee, rec + 0x30)));
            }
        }),
    );
    ee.call(addr("rider_body_update").unwrap(), &[u64::from(body_actor())], &[cap.get(frame, "dt")], 2_000_000_000);
    ee.hooks.clear();
    let mut t = std::mem::take(&mut *heard.borrow_mut());
    t.rebuilt = bits(raw_quad(ee, search + 0x10)) != bits(centre);
    t
}

/// **Front parts at 2628 because the replay's tree lost the wreck bike** (S113): two replays of
/// front from its first state through the original's update, one laying the capture in as S62 did
/// (the bike's place in the track's tree left as the first state had it), one with the bike put
/// where the game's frame puts it ([`place_bike`]). Both hold the capture to 2627. At 2628 the
/// bones' search is rebuilt round the first sphere; without the bike in the tree it is not listed,
/// no bone's sphere is paired with it and every bone lands 0.4-0.7 off the capture; with it, the
/// bones' spheres are paired with the bike, hit it, and the tick is the capture's bit for bit.
#[test]
fn front_parts_at_2628_without_the_bike_in_the_tree() {
    let Some(cap) = Capture::load_in("s62", "front") else { return };
    let Some(mut lost) = state_in("s62", &Wreck { name: "front", frame: 2602 }) else { return };
    let mut placed = state_in("s62", &Wreck { name: "front", frame: 2602 }).unwrap();
    let mut draws_before = 0;
    for frame in 2603..=2628 {
        if repeated(&cap, frame) {
            continue;
        }
        let snap = Snapshot::of(&placed);
        let (n, _) = originals_tick(&mut placed, &cap, frame).unwrap_or_else(|d| panic!("placed: {frame}: {:?}", &d[..d.len().min(4)]));
        draws_before = n;
        if frame == 2628 {
            snap.back(&mut placed);
            break;
        }
        originals_tick_as(&mut lost, &cap, frame, false).unwrap_or_else(|d| panic!("lost: {frame}: {:?}", &d[..d.len().min(4)]));
    }
    let lost_tick = bike_tick(&mut lost, &cap, 2628, draws_before, false);
    let placed_tick = bike_tick(&mut placed, &cap, 2628, draws_before, true);
    let (lost_d, placed_d) = (differ(&cap, 2628, &ee_bones(&lost)), differ(&cap, 2628, &ee_bones(&placed)));
    eprintln!("lost: {} pairs, {} hits, {} lanes off; placed: {} pairs, {} hits", lost_tick.pairs, lost_tick.hits.len(), lost_d.len(), placed_tick.pairs, placed_tick.hits.len());
    // The search is rebuilt that tick either way; only the placed tree lists the bike.
    assert!(lost_tick.rebuilt && placed_tick.rebuilt, "the search rebuilt at 2628");
    assert_eq!((lost_tick.pairs, lost_tick.hits.len()), (0, 0), "the lost tree pairs no sphere with the bike");
    assert_eq!(lost_d.len(), 360, "every bone off the capture: {:?}", &lost_d[..lost_d.len().min(4)]);
    assert!(placed_d.is_empty(), "the placed tree's tick is the capture's: {:?}", &placed_d[..placed_d.len().min(4)]);
    assert_eq!((placed_tick.pairs, placed_tick.hits.len()), (36, 11), "pairs and hits with the bike");
    // The bike's laid-in frame against the bones' displacement: the hits on bones 0, 16 and 17's
    // spheres are on a face whose world normal is the frame's first row, word for word, and those
    // bones and 15 (pulled by its joints) moved 0.45-0.95 ft along it, bone 0 back against its own
    // velocity (y -0.57 with vy +5.6).
    let (_, at) = bike_collidable(&placed);
    let row = at.rows[0];
    let sphere_of_bone = |ee: &Ee, bone: usize| {
        (0..9).map(|i| collision(ee, i)).find(|&c| bone_of(ee, ee.u32_at(c + coll_at::BODY)) == bone).map(|c| ee.u32_at(c + coll_at::SPHERE))
    };
    for bone in [0, 16, 17] {
        let s = sphere_of_bone(&placed, bone).expect("a sphere on the bone");
        let normals: Vec<[u32; 4]> = placed_tick.hits.iter().filter(|h| h.0 == s).map(|h| bits(h.1)).collect();
        assert!(!normals.is_empty(), "bone {bone}'s sphere hits the bike");
        assert!(normals.iter().all(|n| n[..3] == bits(row)[..3]), "bone {bone}: hit normals {normals:x?}, the bike's first row {:?}", row);
    }
    let (lb, pb) = (ee_bones(&lost), ee_bones(&placed));
    for bone in [0, 15, 16, 17] {
        let d: [f32; 3] = [0, 1, 2].map(|l| pb[bone][1][l] - lb[bone][1][l]);
        let len = d.iter().map(|x| x * x).sum::<f32>().sqrt();
        let along = (0..3).map(|l| d[l] * row[l]).sum::<f32>() / len;
        eprintln!("bone {bone}: moved {len:.3} ft, {along:.4} along the bike's first row");
        assert!((0.45..0.95).contains(&len) && along > 0.95, "bone {bone}: moved {d:?}");
    }
    let (d0, v0) = ([0, 1, 2].map(|l| pb[0][1][l] - lb[0][1][l]), lb[0][2]);
    assert!(d0[1] < -0.5 && v0[1] > 5.0 && (0..3).map(|l| d0[l] * v0[l]).sum::<f32>() < 0.0, "bone 0 pushed back: {d0:?}, velocity {v0:?}");
}


// --- S155: every wreck of the 2p recording, from the RAM image before it -------------------------

use game_sim::air;

/// The 2p capture's few columns these tests read, rows by frame counter (C10's
/// `context/capture/2p/2p.csv`, 12,917 rows of ~17 KB: only these columns are kept).
struct TwoP {
    rows: HashMap<u32, Vec<String>>,
    cols: HashMap<&'static str, usize>,
}

/// The columns [`TwoP`] keeps.
const TWO_P_COLS: [&str; 12] = [
    "frame_counter", "dt", "p1_wreck_reason", "p2_wreck_reason", "p1_mode", "p2_mode",
    "p1_ch_centre.x", "p1_ch_centre.y", "p1_ch_centre.z", "p2_ch_centre.x", "p2_ch_centre.y", "p2_ch_centre.z",
];

impl TwoP {
    fn load() -> Option<TwoP> {
        use std::io::BufRead;
        let file = std::fs::File::open(ctx("capture/2p/2p.csv")?).expect("open the 2p capture");
        let mut lines = std::io::BufReader::new(file).lines().map(|l| l.expect("read the 2p capture"));
        let head = lines.next().expect("a header");
        let header: Vec<&str> = head.split(',').collect();
        let at: Vec<usize> = TWO_P_COLS.iter().map(|c| header.iter().position(|h| h == c).unwrap_or_else(|| panic!("no column {c}"))).collect();
        let cols = TWO_P_COLS.iter().enumerate().map(|(k, &c)| (c, k)).collect();
        let rows = lines
            .map(|l| {
                let all: Vec<&str> = l.split(',').collect();
                let kept: Vec<String> = at.iter().map(|&i| all[i].to_string()).collect();
                (kept[0].parse().expect("a frame counter"), kept)
            })
            .collect();
        Some(TwoP { rows, cols })
    }

    fn text(&self, frame: u32, col: &str) -> &str {
        &self.rows.get(&frame).unwrap_or_else(|| panic!("no row {frame}"))[self.cols[col]]
    }

    fn dt(&self, frame: u32) -> f32 {
        self.text(frame, "dt").parse::<f64>().expect("a dt") as f32
    }

    fn int(&self, frame: u32, col: &str) -> u32 {
        self.text(frame, col).parse().unwrap_or_else(|_| panic!("{col} at {frame}"))
    }
}

/// A rider's ridden vehicle in a 2p RAM image: the game manager's list, P1 its fourth entry, P2 its
/// third (watch/2p.toml).
fn two_p_bike(ee: &Ee, p: u32) -> u32 {
    let hops: &[u32] = if p == 1 { &[0x34, 0x34, 0x8, 0x8, 0x8, 0x0] } else { &[0x34, 0x34, 0x8, 0x8, 0x0] };
    let mut a = ee.u32_at(GAME_MANAGER);
    for &h in hops {
        a = ee.u32_at(a + h);
    }
    a
}

/// One of C10's RAM images (`context/states/c10/c10-<tick>.ram`), holding frame `tick`.
fn c10_state(tick: u32) -> Option<Ee> {
    let path = ctx(&format!("states/c10/c10-{tick}.ram")).or_else(|| ctx(&format!("states/s155/c10-{tick}.ram")))?;
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    assert_eq!(ee.u32_at(addr("frame_counter").unwrap()), tick, "c10-{tick}.ram is not frame {tick}");
    Some(ee)
}


impl TwoP {
    /// Rider `p`'s chassis centre on the row of `frame`, as bits.
    fn centre(&self, frame: u32, p: u32) -> [u32; 3] {
        ["x", "y", "z"].map(|c| (self.text(frame, &format!("p{p}_ch_centre.{c}")).parse::<f64>().expect("a centre") as f32).to_bits())
    }

    fn reason(&self, frame: u32, p: u32) -> u32 {
        self.int(frame, &format!("p{p}_wreck_reason"))
    }

    /// The row whose chassis centre is the image's, bit for bit, and its wreck reason too (a wrecked
/// bike's centre stands still), within a few ticks of its own
    /// counter: an image holds its counter's row or the one after (C10's states sit a tick off).
    fn row_of(&self, ee: &Ee, p: u32) -> Option<u32> {
        let bike = two_p_bike(ee, p);
        let ch = ee.u32_at(bike + 0x7a4);
        let at = [0, 4, 8].map(|k| ee.u32_at(ch + 0x60 + k));
        let fc = ee.u32_at(addr("frame_counter").unwrap());
        let reason = ee.u32_at(bike + 0x748);
        (fc.saturating_sub(2)..=fc + 2).find(|&f| self.rows.contains_key(&f) && self.centre(f, p) == at && self.reason(f, p) == reason)
    }
}

/// The wreck test's inputs off a real bike: `tests/air.rs`' `poke`, read back the other way. Its
/// heading and up are left for the caller: the test takes them from the transform as it runs.
fn wreck_inputs(ee: &Ee, bike: u32, dt: f32) -> (air::Params, air::Air, air::Chassis, air::Step, air::Loads) {
    let f = |a: u32| F(ee.f32_at(a));
    let b = |a: u32| ee.u32_at(a) != 0;
    let v = |a: u32| Vf([0, 4, 8, 12].map(|k| ee.f32_at(a + k)));
    let (o, ch, fw, rw, coll) = (bike, ee.u32_at(bike + 0x7a4), ee.u32_at(bike + 0x7b0), ee.u32_at(bike + 0x7b4), ee.u32_at(bike + 0x7d0));
    let p = air::Params {
        airborne_control_moment: v(o + 0x4d0),
        airborne_damping: f(o + 0x4e0),
        airborne_control_force: f(o + 0x4e4),
        brake_gyro_torque: f(o + 0x6ac),
        unroll_gain: f(o + 0x6b0),
        unroll_torque: f(o + 0x6b4),
        unyaw_gain: f(o + 0x6b8),
        unyaw_torque: f(o + 0x6bc),
        control_authority_time: f(o + 0x4ec),
        preload_time: f(o + 0x4ac),
        max_preload: f(o + 0x4c4),
        min_preload_time: f(o + 0x4b4),
        max_preload_time: f(o + 0x4b8),
        min_dpad_preload_time: f(o + 0x4bc),
        max_dpad_preload_time: f(o + 0x4c0),
        min_backflip_time: f(o + 0x504),
        max_backflip_time: f(o + 0x508),
        min_backflip_torque: f(o + 0x510),
        max_backflip_torque: f(o + 0x514),
        backflip_torque_rate_increase: f(o + 0x518),
        backflip_torque_rate_decrease: f(o + 0x51c),
        front_wreck_speed_limit: f(o + 0x754),
        back_wreck_speed_limit: f(o + 0x758),
        side_wreck_speed_limit: f(o + 0x75c),
        racing_down_wreck_speed_limit: f(o + 0x760),
        freestyle_down_wreck_speed_limit: f(o + 0x764),
        ground_pitch_down_wreck_angle: f(o + 0x768),
        ground_pitch_up_wreck_angle: f(o + 0x76c),
        airborne_pitch_down_wreck_angle: f(o + 0x770),
        airborne_pitch_up_wreck_angle: f(o + 0x774),
        airborne_roll_wreck_angle: f(o + 0x778),
        no_hander_wreck_speed_limit: f(o + 0x77c),
        no_hander_max_roll_angle: f(o + 0x780),
        joust_wreck_height: f(o + 0x784),
        crossup_wreck: f(o + 0x78c),
        crossup_wreck_time: f(o + 0x790),
        rotation_speed: None,
    };
    let a = air::Air {
        on_ground: b(o + 0x574),
        both_wheels: b(o + 0x578),
        airborne: b(o + 0x57c),
        was_airborne: b(o + 0x580),
        hang_time: f(o + 0x4f0),
        contact_vel: v(o + 0x430),
        crossup: f(o + 0x600),
        authority: f(o + 0x4e8),
        preload_ready: b(o + 0x4a0),
        preload_firing: b(o + 0x4a4),
        preload_armed: b(o + 0x4a8),
        preload_timer: f(o + 0x4b0),
        preload_force: v(o + 0x490),
        flip_torque_applied: b(o + 0x4f4),
        flipping: b(o + 0x4f8),
        flip_active: b(o + 0x4fc),
        flip_armed: b(o + 0x500),
        flip_grace: f(o + 0x50c),
        flip_torque: v(o + 0x520),
        wreck_impact: v(o + 0x560),
        ..air::Air::default()
    };
    let c = air::Chassis {
        speed: f(ch + 0x34),
        pitch: f(ch + 0x90),
        roll: f(ch + 0x98),
        force: v(ch + 0x130),
        moment: v(ch + 0x140),
        vel: v(ch + 0x260),
        local_vel: v(ch + 0x270),
        ang_vel: v(ch + 0x290),
    };
    let st = air::Step {
        dt: F(dt),
        stick_hold: f(o + 0x630),
        front_contact: b(fw + 0xdc),
        rear_contact: b(rw + 0xdc),
        front_vel: v(fw + 0x190),
        rear_vel: v(rw + 0x190),
        stick: (f(o + 0x628), f(o + 0x62c)),
        dpad_y: b(o + 0x624),
        dpad_dir: ee.i32_at(o + 0x9fc),
        brake: f(o + 0x640),
        front_brake: f(o + 0x650),
        rear_brake: f(o + 0x654),
        lean: f(o + 0x670),
        stunt: b(o + 0xa0c),
        touching: b(coll + 0x40),
        ..air::Step::default()
    };
    let lo = air::Loads {
        side: f(o + 0x550),
        vertical: f(o + 0x554),
        fore_aft: f(o + 0x558),
        landing_crossup: f(o + 0x788),
        no_hander: b(o + 0xa44),
        racing: b(o + 0x810),
        pose: ee.i32_at(o + 0xa78),
        pose_wanted: ee.i32_at(o + 0xa84),
        pose_blend: f(o + 0xa2c),
        trick_wrecked: b(o + 0xa10),
    };
    (p, a, c, st, lo)
}

/// The wreck test's last clause's inputs off a real bike: its collision record's contact list (S37's
/// layout, `tests/air.rs`' `lay` read back), the 50cc flag, and the chassis' mass and height.
fn hit_inputs(ee: &Ee, bike: u32) -> (bool, F, F, Vec<air::Contact>) {
    let (ch, coll) = (ee.u32_at(bike + 0x7a4), ee.u32_at(bike + 0x7d0));
    let v = |a: u32| Vf([0, 4, 8, 12].map(|k| ee.f32_at(a + k)));
    let (n, list) = (ee.u32_at(coll + 0x100), ee.u32_at(coll + 0x108));
    let list = (0..n)
        .map(|i| {
            let e = list + 0x80 * i;
            let other = match ee.u32_at(e + 0x74) {
                0 => air::Other::Scenery,
                o if ee.u32_at(o + 0xf0) != 0 => {
                    let body = ee.u32_at(o + 0xf0);
                    air::Other::Rider { height: F(ee.f32_at(body + 0x64)), vel: v(body + 0x260) }
                }
                o => {
                    let body = ee.u32_at(o + 0xf4);
                    air::Other::Object { mass: F(ee.f32_at(body + 0xc)), vel: v(body + 0x70) }
                }
            };
            air::Contact { mirrored: ee.u32_at(e) != 0, reach: F(ee.f32_at(e + 0x24)), other }
        })
        .collect();
    (ee.u32_at(bike + 0x588) != 0, F(ee.f32_at(ch + 0x210)), F(ee.f32_at(ch + 0x64)), list)
}

/// One call of the original's wreck test on the bike, and the port's answer to the same inputs.
#[derive(Clone, Debug, PartialEq)]
struct Judged {
    /// The capture row the tick makes (the image's row plus the ticks run).
    row: u32,
    /// The reason the bike held going in (0 riding).
    held: u32,
    /// The reasons the original entered, in order.
    original: Vec<u32>,
    /// The port's: [`air::wreck_test`] then [`air::hit_wreck`], as `rider.rs` composes them (the
    /// hit's reason is the one that stays; both are entered).
    port: Vec<u32>,
    /// What the clauses judged: pitch, roll (degrees), the g loads, hang time, the crossup shown.
    seen: (f32, f32, [f32; 3], f32, f32),
}

/// The original's own vehicle update on rider `p`'s bike from a 2p RAM image, `ticks` times (each
/// with the capture's `dt` for the row it makes), the wreck test heard at every call: its inputs
/// read off the bike as it is entered, the port asked the same, and the original's calls of the
/// bike's "came off" slot written down. The image's row is found by its chassis centre. Returns that
/// row, the last row the run's chassis centre still held to the capture's bit for bit (only this
/// bike is updated, so the rest of the frame — the pad, the rider's body — is the image's), and
/// every call.
fn judge_from(cap: &TwoP, ee: &mut Ee, p: u32, ticks: u32) -> (u32, u32, Vec<Judged>) {
    let bike = two_p_bike(ee, p);
    let row0 = cap.row_of(ee, p).unwrap_or_else(|| panic!("P{p}: no row of the capture is the image's chassis centre"));
    let calls = Rc::new(RefCell::new(Vec::new()));
    let off = ee.u32_at(ee.u32_at(bike + 0x48) + 0xfc);
    let c = Rc::clone(&calls);
    ee.hooks.insert(
        off,
        Box::new(move |ee: &mut Ee| {
            if ee.r(4) as u32 == bike {
                c.borrow_mut().push(ee.r(5) as u32);
            }
            through(ee);
        }),
    );
    let judged = Rc::new(RefCell::new(Vec::new()));
    let row = Rc::new(RefCell::new(row0));
    let (j, r, c) = (Rc::clone(&judged), Rc::clone(&row), Rc::clone(&calls));
    ee.hooks.insert(
        addr("bike_wreck_test").unwrap(),
        Box::new(move |ee: &mut Ee| {
            if ee.r(4) as u32 != bike {
                return through(ee);
            }
            let dt = ee.f(12);
            let held = ee.u32_at(bike + 0x748);
            let (prm, mut a, ch, mut st, lo) = wreck_inputs(ee, bike, dt);
            let (fifty, mass, height, list) = hit_inputs(ee, bike);
            c.borrow_mut().clear();
            through(ee);
            // The two axes the test takes off the transform as it runs (`orientation_axes`).
            st.heading = Vf([0, 4, 8, 12].map(|k| ee.f32_at(bike + 0x410 + k)));
            st.up = Vf([0, 4, 8, 12].map(|k| ee.f32_at(bike + 0x420 + k)));
            let landed = if held == 0 { air::wreck_test(&prm, &mut a, &ch, &st, &lo) } else { None };
            let hit = air::hit_wreck(&prm, &mut a, st.touching, fifty, mass, height, &list);
            let port = landed.into_iter().chain(hit).map(|w| w as u32).collect();
            let seen = (ch.pitch.0.to_degrees(), ch.roll.0.to_degrees(), [lo.side.0, lo.vertical.0, lo.fore_aft.0], a.hang_time.0, lo.landing_crossup.0);
            j.borrow_mut().push(Judged { row: *r.borrow() + 1, held, original: c.borrow().clone(), port, seen });
        }),
    );
    let mut held_to = row0;
    for _ in 0..ticks {
        let next = *row.borrow() + 1;
        // The tick that writes row `next` steps on the dt its row before holds: a row's dt is
        // the one the frame after it will take (with `next`'s own, most runs part at once).
        ee.call(addr("bike_vehicle_update").unwrap(), &[u64::from(bike)], &[cap.dt(next - 1)], 2_000_000_000);
        *row.borrow_mut() = next;
        let ch = ee.u32_at(bike + 0x7a4);
        if held_to + 1 == next && cap.rows.contains_key(&next) && cap.centre(next, p) == [0, 4, 8].map(|k| ee.u32_at(ch + 0x60 + k)) {
            held_to = next;
        }
    }
    ee.hooks.clear();
    let out = judged.borrow().clone();
    (row0, held_to, out)
}


/// C10's index of the 2p capture's wrecks (research/c10/events.json): the first row holding the
/// reason, the rider and the reason.
const WRECKS_2P: [(u32, u32, u32); 17] = [
    (128995, 1, 8), // NoseDive
    (129945, 1, 8), // NoseDive
    (130966, 1, 11), // Tail
    (132997, 1, 3), // Right
    (133313, 1, 12), // NoHander
    (133953, 1, 12), // NoHander
    (134381, 1, 12), // NoHander
    (135196, 1, 12), // NoHander
    (135855, 1, 12), // NoHander
    (136559, 1, 12), // NoHander
    (139659, 1, 2), // Left
    (130418, 2, 6), // Scenery
    (130897, 2, 6), // Scenery
    (131595, 2, 1), // Front
    (133800, 2, 8), // NoseDive
    (134315, 2, 12), // NoHander
    (140363, 2, 2), // Left
];

/// Which of the wreck test's limits a near miss came close to (C10's near-miss index).
#[derive(Clone, Copy, Debug)]
enum Clause {
    VerticalG,
    GroundPitchDown,
    GroundPitchUp,
    TouchPitchDown,
    TouchPitchUp,
    TouchRoll,
}

impl Clause {
    /// The quantity the clause judges, out of what the wreck test was handed.
    fn of(self, j: &Judged) -> f32 {
        let (pitch, roll, g, _, _) = j.seen;
        match self {
            Clause::VerticalG => g[1],
            Clause::GroundPitchDown | Clause::TouchPitchDown => -pitch,
            Clause::GroundPitchUp | Clause::TouchPitchUp => pitch,
            Clause::TouchRoll => roll.abs(),
        }
    }
}

/// C10's near misses: the row, the rider, the clause and the value the index read off the row
/// (rounded to three places), each ridden out by the original.
const NEAR_2P: [(u32, u32, Clause, f32); 29] = [
    (128520, 1, Clause::GroundPitchUp, 43.275),
    (128892, 1, Clause::GroundPitchUp, 43.028),
    (129123, 1, Clause::TouchPitchDown, 40.386),
    (129124, 1, Clause::GroundPitchDown, 40.542),
    (129485, 1, Clause::GroundPitchDown, 60.252),
    (129485, 1, Clause::TouchPitchDown, 60.252),
    (129486, 1, Clause::VerticalG, 55.874),
    (130420, 1, Clause::GroundPitchDown, 67.43),
    (130420, 1, Clause::TouchPitchDown, 67.43),
    (131484, 1, Clause::TouchRoll, 85.949),
    (131628, 1, Clause::GroundPitchUp, 48.143),
    (131872, 1, Clause::VerticalG, 53.274),
    (132463, 1, Clause::VerticalG, 34.328),
    (132608, 1, Clause::TouchRoll, 69.417),
    (135076, 1, Clause::TouchRoll, 50.019),
    (128467, 2, Clause::VerticalG, 44.553),
    (128849, 2, Clause::GroundPitchDown, 50.332),
    (128849, 2, Clause::TouchPitchDown, 50.332),
    (128851, 2, Clause::VerticalG, 48.348),
    (131576, 2, Clause::GroundPitchDown, 41.341),
    (131576, 2, Clause::TouchPitchDown, 41.341),
    (133253, 2, Clause::GroundPitchDown, 45.492),
    (133538, 2, Clause::VerticalG, 64.094),
    (133744, 2, Clause::GroundPitchUp, 52.783),
    (135556, 2, Clause::TouchRoll, 49.217),
    (136062, 2, Clause::GroundPitchUp, 56.896),
    (136062, 2, Clause::TouchPitchUp, 56.896),
    (136142, 2, Clause::GroundPitchUp, 50.572),
    (137982, 2, Clause::TouchRoll, 43.005),
];

/// The latest 2p RAM image from before row `frame` for rider `p`: C10's (`states/c10/`) and
/// S155's earlier ones (`states/s155/`, the same playback recipe a few ticks sooner), up to eight
/// ticks back, whose chassis centre is a row before `frame`'s. With the row it holds.
fn image_before(cap: &TwoP, frame: u32, p: u32) -> Option<(String, Ee, u32)> {
    let kit = Kit::load().ok()?;
    let mut found: Vec<(u32, PathBuf)> = Vec::new();
    for dir in ["states/c10", "states/s155"] {
        let local = kit.root.join("context").join(dir);
        let at = if local.exists() { local } else { kit.ctx(dir) };
        for t in frame.saturating_sub(8)..frame {
            let path = at.join(format!("c10-{t}.ram"));
            if path.exists() {
                found.push((t, path));
            }
        }
    }
    found.sort();
    for (t, path) in found.into_iter().rev() {
        let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
        ee.set(28, u64::from(addr("boot_gp").unwrap()));
        assert_eq!(ee.u32_at(addr("frame_counter").unwrap()), t, "{} is not frame {t}", path.display());
        if let Some(row) = cap.row_of(&ee, p).filter(|&r| r < frame) {
            return Some((path.display().to_string(), ee, row));
        }
    }
    None
}

/// How many of the 46 indexed events the original's run from its image reproduces on the
/// capture's own line (see the test).
const TWO_P_ON_THE_CAPTURE: usize = 22;

/// **Every wreck of the 2p recording, and every near miss, judged by the port as the original
/// judged it** (S155). From the RAM image before each indexed event the original's own vehicle
/// update runs the rider's bike to a tick past it, each tick on the dt of the capture row before
/// (a row's dt is the next frame's). At every call of the original's wreck test the port's
/// [`air::wreck_test`] and [`air::hit_wreck`] are handed the same inputs, read off the bike as the
/// test is entered, and must enter what the original entered, on every call of every run.
/// Where the run is still the capture's (the chassis centre bit for bit from the image to the row of the
/// event, and on it: the test runs after the tick has moved the chassis): each wreck is the original's first, on the indexed row, with the indexed reason; each
/// near miss is ridden out through the row after, with the clause's quantity the index's value.
/// Runs that leave the capture first (the image holds the bike, not the rest of the frame's work
/// on it) are listed; [`TWO_P_ON_THE_CAPTURE`] is the floor on the others. A skip with a line
/// where the capture or the images are not in this worktree.
#[test]
fn the_2p_wrecks_are_the_originals() {
    let Some(cap) = TwoP::load() else { return };
    let (mut exact, mut missing, mut drifted) = (Vec::new(), Vec::new(), Vec::new());
    for (frame, p, reason) in WRECKS_2P {
        assert_eq!((cap.reason(frame - 1, p), cap.reason(frame, p)), (0, reason), "P{p} {frame}: the index's wreck is not in the capture");
        let Some((image, mut ee, row0)) = image_before(&cap, frame, p) else {
            missing.push(format!("wreck P{p} {frame}"));
            continue;
        };
        let (_, to, js) = judge_from(&cap, &mut ee, p, frame + 1 - row0);
        eprintln!("P{p} wreck {reason} at {frame}: from {image} (row {row0}), the capture's centre held to {to}");
        for j in &js {
            eprintln!("    {j:?}");
            assert_eq!(j.port, j.original, "P{p} {frame}: row {}: the port's wreck test answers {:?}, the original's {:?}", j.row, j.port, j.original);
        }
        let first = js.iter().find(|j| j.held == 0 && !j.original.is_empty());
        // While the original's run is still the capture's (its chassis centre bit for bit up to the event
        // row), its first wreck is the indexed one on the indexed row. A run that has left
        // the capture is the original riding a different line; it is listed, not judged.
        if to >= frame {
            let first = first.unwrap_or_else(|| panic!("P{p} {frame}: the original never wrecked though its run held the capture to {to}"));
            assert_eq!((first.row, first.original.last().copied()), (frame, Some(reason)), "P{p} {frame}: the original's first wreck: {first:?}");
            exact.push(format!("wreck P{p} {frame}"));
        } else {
            drifted.push(format!("wreck P{p} {frame} (left at {to}; the original's wreck: {:?})", first.map(|j| (j.row, j.original.clone()))));
        }
    }
    for (frame, p, clause, value) in NEAR_2P {
        let Some((image, mut ee, row0)) = image_before(&cap, frame, p) else {
            missing.push(format!("near miss P{p} {frame} {clause:?}"));
            continue;
        };
        let (_, to, js) = judge_from(&cap, &mut ee, p, frame + 1 - row0 + 1);
        let at = js.iter().find(|j| j.row == frame).unwrap_or_else(|| panic!("P{p} {frame}: no wreck test on the row"));
        eprintln!("P{p} near miss {clause:?} {value} at {frame}: from {image} (row {row0}), held to {to}, judged {}", clause.of(at));
        for j in &js {
            assert_eq!(j.port, j.original, "P{p} {frame} {clause:?}: row {}: the port's wreck test answers {:?}, the original's {:?}", j.row, j.port, j.original);
        }
        if to >= frame {
            for j in &js {
                assert!(j.held == 0 && j.original.is_empty(), "P{p} {frame} {clause:?}: the original wrecked on row {}: {j:?}", j.row);
            }
            assert!((clause.of(at) - value).abs() < 0.0015, "P{p} {frame} {clause:?}: the wreck test judged {} on the row, the index {value}", clause.of(at));
            exact.push(format!("near miss P{p} {frame} {clause:?}"));
        } else {
            drifted.push(format!("near miss P{p} {frame} {clause:?} (left at {to}; judged {})", clause.of(at)));
        }
    }
    eprintln!("{} of {} events on the capture's own run: {exact:?}", exact.len(), WRECKS_2P.len() + NEAR_2P.len());
    eprintln!("left the capture first: {drifted:?}");
    if !missing.is_empty() {
        skip(&format!("S155: no RAM image before {} of the 2p events: {missing:?} (research/s155/rams.py says how they are saved)", missing.len()));
        return;
    }
    assert_eq!(exact.len() + drifted.len(), WRECKS_2P.len() + NEAR_2P.len());
    assert!(exact.len() >= TWO_P_ON_THE_CAPTURE, "only {} of the 2p events ran on the capture's own line (was {TWO_P_ON_THE_CAPTURE})", exact.len());
}

#[test]
#[ignore = "exploration"]
fn s155_how_long_an_image_holds() {
    let Some(cap) = TwoP::load() else { return };
    let ticks: Vec<u32> = std::env::var("S155_TICKS").unwrap_or_default().split(',').filter_map(|t| t.parse().ok()).collect();
    for t in ticks {
        for p in [1, 2] {
            let Some(mut ee) = c10_state(t) else { continue };
            let Some(row0) = cap.row_of(&ee, p) else {
                eprintln!("{t} P{p}: no row");
                continue;
            };
            let Some(mut one) = c10_state(t) else { continue };
            judge_from(&cap, &mut one, p, 1);
            let ch = one.u32_at(two_p_bike(&one, p) + 0x7a4);
            let off: Vec<Vec<f32>> = (0..3).map(|d| (0..3).map(|k| one.f32_at(ch + 0x60 + 4 * k as u32) - f32::from_bits(cap.centre(row0 + d, p)[k])).collect()).collect();
            let (_, to, _) = judge_from(&cap, &mut ee, p, 8);
            eprintln!("{t} P{p}: row {row0}, held to {to} ({} ticks); the first tick off by {off:?}", to - row0);
        }
    }
}

// --- S160: the ragdoll in the 2p recording ---------------------------------------------------------

/// The 2p capture's bones, modes and the rest S160 reads, for one rider, rows by frame counter.
struct TwoPBody {
    rows: HashMap<u32, Vec<f64>>,
    cols: HashMap<String, usize>,
}

/// The lanes of a bone the 2p capture carries (watch/2p.toml's `p<N>_bone`).
const BONE_LANES: [(&str, usize); 3] = [("pos", 3), ("vel", 3), ("quat", 4)];

impl TwoPBody {
    fn load(p: u32) -> Option<TwoPBody> {
        use std::io::BufRead;
        let mut want: Vec<String> = ["frame_counter", "dt", "rng_state"].map(String::from).to_vec();
        for c in ["mode", "mode_prev", "touching", "off_bike", "settled", "flung", "wreck_reason"] {
            want.push(format!("p{p}_{c}"));
        }
        for i in 0..18 {
            for (n, k) in BONE_LANES {
                for l in &["x", "y", "z", "w"][..k] {
                    want.push(format!("p{p}_bone{i}.{n}.{l}"));
                }
            }
        }
        let file = std::fs::File::open(ctx("capture/2p/2p.csv")?).expect("open the 2p capture");
        let mut lines = std::io::BufReader::new(file).lines().map(|l| l.expect("read the 2p capture"));
        let head = lines.next().expect("a header");
        let header: Vec<&str> = head.split(',').collect();
        let at: Vec<usize> = want.iter().map(|c| header.iter().position(|h| h == c).unwrap_or_else(|| panic!("no column {c}"))).collect();
        let cols = want.iter().enumerate().map(|(k, c)| (c.clone(), k)).collect();
        let rows = lines
            .map(|l| {
                let all: Vec<&str> = l.split(',').collect();
                let kept: Vec<f64> = at.iter().map(|&i| all[i].parse().unwrap_or_else(|_| panic!("{:?}", all[i]))).collect();
                (kept[0] as u32, kept)
            })
            .collect();
        Some(TwoPBody { rows, cols })
    }

    fn get(&self, frame: u32, col: &str) -> f64 {
        self.rows.get(&frame).unwrap_or_else(|| panic!("no row {frame}"))[*self.cols.get(col).unwrap_or_else(|| panic!("no column {col}"))]
    }

    fn rng(&self, frame: u32) -> u64 {
        self.get(frame, "rng_state") as u64
    }

    /// Rider `p`'s bone lanes on `frame` that differ from the image's, bit for bit, as names.
    fn differ(&self, ee: &Ee, frame: u32, p: u32) -> Vec<String> {
        let mut out = Vec::new();
        for i in 0..18 {
            let body = bone_body(ee, i);
            for (n, k) in BONE_LANES {
                let o = match n {
                    "pos" => 0x60,
                    "vel" => 0x70,
                    _ => 0x170,
                };
                for (l, lane) in ["x", "y", "z", "w"][..k].iter().enumerate() {
                    let want = self.get(frame, &format!("p{p}_bone{i}.{n}.{lane}")) as f32;
                    let got = ee.f32_at(body + o + 4 * l as u32);
                    if want.to_bits() != got.to_bits() {
                        out.push(format!("bone{i}.{n}.{lane} {got:e} want {want:e}"));
                    }
                }
            }
        }
        out
    }
}

/// Rider `p`'s body actor in a 2p image: the game manager's list after the two bikes (P2's bike,
/// P1's bike, P1's body, P2's body: watch/2p.toml), its record less 0x400.
fn two_p_actor(ee: &Ee, p: u32) -> u32 {
    let hops: &[u32] = if p == 1 { &[0x34, 0x34, 0x8, 0x8, 0x8, 0x8, 0x0] } else { &[0x34, 0x34, 0x8, 0x8, 0x8, 0x8, 0x8, 0x0] };
    let mut a = ee.u32_at(GAME_MANAGER);
    for &h in hops {
        a = ee.u32_at(a + h);
    }
    let a = a - 0x400;
    assert_eq!(ee.u32_at(a + 0x2e0), 18, "P{p}: no rider body actor at {a:#x}");
    a
}

/// The ridden player's update (`./kit fn 0x0027c890`): the bike's vehicle update and the work round
/// it that hands the rider body its request (the bike's vehicle update alone leaves the body a tick
/// behind its bike in every riding row).
const PLAYER_UPDATE: u32 = 0x0027_c890;

/// `ride_reset_request` (bike, kind, page) and `bike_page_listener` (the bike's +0x1340): S5, S72.
const RESET_REQUEST: u32 = 0x0027_b9f0;
const PAGE_LISTENER: u32 = 0x0028_1c10;

/// The 2p wrecks the human ended with the L1+R1 chord before the ride machine's own put-back
/// (P1's pad byte 3 goes `0xb3`/`0xf3` on the row the capture's body mode goes 0).
const CHORDS: [u32; 3] = [128995, 129945, 130966];

/// A two-row stand-in for an s56 [`Capture`], so [`port_tick`] runs on a 2p tick: `frame`'s dt and
/// the generator before it.
fn tick_rows(frame: u32, dt: f32, rng: u64) -> Capture {
    let cols = [("frame_counter", 0), ("dt", 1), ("rng_state", 2)].map(|(c, k)| (c.to_string(), k)).into_iter().collect();
    let row = |f: u32| vec![f.to_string(), format!("{dt:e}"), rng.to_string()];
    Capture { cols, rows: [(frame - 1, row(frame - 1)), (frame, row(frame))].into_iter().collect() }
}

/// One 2p wreck run by [`ragdoll_2p`].
#[derive(Debug, Default)]
struct Ran2p {
    /// The image's row (the bike's and the body's), and the last row the original's run held every
    /// captured bone of the rider to, bit for bit.
    row0: u32,
    held: u32,
    /// The ticks the port's tick was compared on, and the ones it parted on (row, first numbers).
    ticks: usize,
    parted: Vec<(u32, Vec<String>)>,
    /// The original's body modes as the run went, with the row each began on, and the capture's.
    modes: Vec<(u32, i32)>,
    captured: Vec<(u32, i32)>,
    went: Went,
    hits: usize,
    /// The ride machine's put-back requests (row, kind, page), and the row whose end the page's
    /// listener respawned the bike on.
    asked: Vec<(u32, u32, u32)>,
    respawned: Option<u32>,
    /// The first row after the put-back whose captured bones the run's are again, bit for bit.
    rejoined: Option<u32>,
}

/// Rider `p`'s wreck `frame` of the 2p recording, from the RAM image before it to `rows` past the
/// put-back the capture shows: the original's player update and body update each tick (each on the
/// dt of the row before: S155), the body's generator draws found from the capture's while the run
/// still holds its bones; and on every tick, from the original's RAM before the body's update, the
/// port's actor tick ([`port_tick`]: every number the update writes, the world's answers the
/// original's own) on its own state, carried from tick to tick (from the original's after a tick
/// that parts).
fn ragdoll_2p(cap: &TwoP, body: &TwoPBody, frame: u32, p: u32, rows: u32, chord: bool) -> Option<(String, Ran2p)> {
    let (image, mut ee, row0) = image_before(cap, frame, p)?;
    let bike = two_p_bike(&ee, p);
    with_actor(two_p_actor(&ee, p));
    let mut row0 = row0;
    if !body.differ(&ee, row0, p).is_empty() && body.differ(&ee, row0 + 1, p).is_empty() {
        // Saved between the body's update and its bike's: the bike's tick first.
        ee.call(PLAYER_UPDATE, &[u64::from(bike)], &[body.get(row0, "dt") as f32], 2_000_000_000);
        row0 += 1;
    }
    let mode = |f: u32| body.get(f, &format!("p{p}_mode")) as i32;
    let back = (frame..).take_while(|&f| body.rows.contains_key(&f)).find(|&f| mode(f) == 0 && mode(f - 1) != 0).unwrap_or(frame);
    let last = (back + rows).min(*body.rows.keys().max().unwrap());
    let mut ran = Ran2p { row0, held: if body.differ(&ee, row0, p).is_empty() { row0 } else { 0 }, ..Ran2p::default() };
    let mut port = None;
    // The original's actor as its update left it, to tell what the rest of the frame wrote after.
    let mut left: Option<Actor> = None;
    let asked = Rc::new(RefCell::new(Vec::new()));
    for row in row0 + 1..=last {
        let dt = body.get(row - 1, "dt") as f32;
        let a = Rc::clone(&asked);
        ee.hooks.insert(
            RESET_REQUEST,
            Box::new(move |ee: &mut Ee| {
                if ee.r(4) as u32 == bike {
                    a.borrow_mut().push((row, ee.r(5) as u32, ee.r(6) as u32));
                }
                through(ee);
            }),
        );
        if row == back && chord {
            // The human's L1+R1 on the put-back row (pad0/pad1 byte 3 `0xb3`/`0xf3`): the chord's
            // handler, an input event ahead of the updates, takes its latch and asks for kind 1
            // with no page, which respawns at once (`player_hand_reset`, S84).
            ee.set_u32(bike + 0x14d0, 1);
            ee.call(RESET_REQUEST, &[u64::from(bike), 1, 0], &[], 2_000_000_000);
            ran.respawned = Some(row);
        }
        ee.call(PLAYER_UPDATE, &[u64::from(bike)], &[dt], 2_000_000_000);
        ee.hooks.clear();
        // While the run holds the capture: the draws the rest of the frame made before the body.
        let mut n = 0;
        if ran.held + 1 == row {
            let snap = Snapshot::of(&ee);
            for k in 0..40u32 {
                snap.back(&mut ee);
                let mut r = Rng::from_state(body.rng(row - 1));
                (0..k).for_each(|_| {
                    r.rand();
                });
                set_rng(&mut ee, r.state());
                ee.call(addr("rider_body_update").unwrap(), &[u64::from(body_actor())], &[dt], 2_000_000_000);
                if body.differ(&ee, row, p).is_empty() {
                    (n, ran.held) = (k, row);
                    break;
                }
                if rng_at(&ee) == r.state() {
                    break; // the body drew nothing: the generator is not the difference
                }
            }
            snap.back(&mut ee);
        }
        let rng = if ran.held == row { body.rng(row - 1) } else { rng_at(&ee) };
        let was = actor_of(&ee);
        if let Some(left) = left.as_ref().filter(|&l| *l != was) {
            // Carrying the port's actor is the original's only if nothing but the body's update
            // writes it: not the player update, the respawn or the chord (none did, on any run).
            ran.parted.push((row, vec![format!("the frame wrote the actor between its updates (mode {:?} -> {:?})", left.mode, was.mode)]));
        }
        let (d, moving) = port_tick(&mut ee, &tick_rows(row, dt, rng), row, n, None, Some(&mut port));
        left = Some(actor_of(&ee));
        if let (Some(mine), Some(game)) = (&mut port, &left) {
            // The vehicle animation's two outputs the body's mode reads, which the original's update
            // runs (through the actor's vtable) and the port's actor tick does not: vehicle_anim.rs.
            mine.posed = game.posed;
            mine.clip = game.clip;
        }
        ran.ticks += 1;
        ran.hits += usize::from(moving > 0);
        if !d.is_empty() {
            ran.parted.push((row, d[..d.len().min(6)].to_vec()));
            port = None; // the next tick starts the port from the original's RAM again
        }
        ran.went.add(&was, &ee);
        let m = ee.i32_at(body_actor() + at::MODE);
        if ran.modes.last().map(|&(_, l)| l) != Some(m) {
            ran.modes.push((row, m));
        }
        if ran.captured.last().map(|&(_, l)| l) != Some(mode(row)) {
            ran.captured.push((row, mode(row)));
        }
        if ran.respawned.is_some_and(|r| r < row) && body.differ(&ee, row, p).is_empty() {
            ran.rejoined.get_or_insert(row);
        }
        if row + 1 == back && !chord && !asked.borrow().is_empty() {
            // The fade's black draw at the end of the tick before the capture's put-back: the
            // page's listener on the bike respawns it (S72, S84). The fader is not run here.
            ee.call(PAGE_LISTENER, &[u64::from(bike + 0x1340)], &[], 2_000_000_000);
            ran.respawned = Some(row);
        }
    }
    ran.asked = asked.borrow().clone();
    Some((image, ran))
}

/// How many of the 2p wrecks the original's run from its image enters, puts back and rides on
/// again (all 17 but P2's two Scenery wrecks, which S155's runs never enter: S176).
const TWO_P_RAGDOLLS: usize = 15;

/// How many of the runs hold every captured bone of the rider bit for bit from the image to the
/// wreck's row, over at least one tick (three of P2's images are on the wreck's row already).
const TWO_P_BONES_TO_THE_WRECK: usize = 5;

/// How many of the runs' wreck entries (mode 2) and riding re-entries (mode 1) begin on the
/// capture's own row.
const TWO_P_SWITCHES_ON_THE_ROW: usize = 21;

/// **Every 2p wreck's bones: the port's ragdoll is the original's from the wreck to the put-back
/// and back on the bike** (S160). For each of C10's 17 wrecks of the 2p recording, both riders,
/// [`ragdoll_2p`] runs the original from the RAM image before it, to ten rows past the capture's
/// put-back. The port's actor tick runs beside it on its own state, carried from tick to tick, with
/// only the world's answers and the vehicle animation's two outputs taken from the original. On every
/// tick of every run (the entry pose, the flight, the ground contacts, the rest, the put-back and the
/// riding entry), every bone, pin, joint, collision and actor number the update writes must be the
/// original's bit for bit, and nothing but the body's update may write the actor.
/// Each run that enters must go 2 → 0 → 1 as the capture does. Floors: [`TWO_P_RAGDOLLS`] runs
/// enter, [`TWO_P_BONES_TO_THE_WRECK`] hold the captured bones to the wreck row, and
/// [`TWO_P_SWITCHES_ON_THE_ROW`] mode switches land on the capture's row. Skips with a line when the
/// capture or the images are not in this worktree.
#[test]
fn every_2p_wrecks_bones_are_the_originals() {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let Some(cap) = TwoP::load() else { return };
    let (Some(one), Some(two)) = (TwoPBody::load(1), TwoPBody::load(2)) else { return };
    let bodies = [one, two];
    let next = AtomicUsize::new(0);
    let runs = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..6 {
            s.spawn(|| {
                while let Some(&(frame, p, reason)) = WRECKS_2P.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let ran = ragdoll_2p(&cap, &bodies[p as usize - 1], frame, p, 10, CHORDS.contains(&frame));
                    runs.lock().unwrap().push((frame, p, reason, ran));
                }
            });
        }
    });
    let mut runs = runs.into_inner().unwrap();
    runs.sort_by_key(|r| (r.1, r.0));
    let (mut missing, mut entered, mut not_entered, mut held, mut on_row, mut off_row) = (Vec::new(), 0, Vec::new(), 0, 0, Vec::new());
    let mut went = Went::default();
    for (frame, p, reason, ran) in runs {
        let Some((image, ran)) = ran else {
            missing.push(format!("P{p} {frame}"));
            continue;
        };
        eprintln!(
            "P{p} {frame} ({reason}): {image} row {}, the bones held to {}; {} ticks; modes {:?}, the capture's {:?}; put back on {:?} (asked {:?}); {:?}",
            ran.row0, ran.held, ran.ticks, ran.modes, ran.captured, ran.respawned, ran.asked, ran.went
        );
        if let Some((row, d)) = ran.parted.first() {
            panic!("P{p} {frame}: the port's ragdoll parts from the original's on row {row} ({} ticks part): {d:?}", ran.parted.len());
        }
        held += usize::from(ran.held >= frame && ran.held > ran.row0);
        let w = &ran.went;
        (went.ticks, went.entries, went.touching, went.off_bike) = (went.ticks + w.ticks, went.entries + w.entries, went.touching + w.touching, went.off_bike + w.off_bike);
        (went.flung, went.landed, went.settles, went.settled) = (went.flung + w.flung, went.landed + w.landed, went.settles + w.settles, went.settled + w.settled);
        if ran.went.entries == 0 {
            not_entered.push(format!("P{p} {frame} ({reason})"));
            continue;
        }
        entered += 1;
        assert_eq!((ran.went.entries, ran.went.off_bike), (1, 1), "P{p} {frame}: one wreck, the rider off the bike once");
        assert!(ran.respawned.is_some(), "P{p} {frame}: never put back");
        let from_wreck = |m: &[(u32, i32)]| m.iter().skip_while(|s| s.1 != 2).copied().collect::<Vec<_>>();
        let (run, captured) = (from_wreck(&ran.modes), from_wreck(&ran.captured));
        assert_eq!(run.iter().map(|s| s.1).collect::<Vec<_>>(), [2, 0, 1], "P{p} {frame}: the run's modes from the wreck {run:?}");
        assert_eq!(captured.iter().map(|s| s.1).collect::<Vec<_>>(), [2, 0, 1], "P{p} {frame}: the capture's modes from the wreck {captured:?}");
        for k in [0, 2] {
            if run[k].0 == captured[k].0 {
                on_row += 1;
            } else {
                off_row.push(format!("P{p} {frame} mode {} on {} (the capture's {})", run[k].1, run[k].0, captured[k].0));
            }
        }
    }
    eprintln!("{entered} wrecks entered, put back and ridden on again, {held} with the bones held to the wreck row; {went:?}");
    eprintln!("not entered by the original's run: {not_entered:?}");
    eprintln!("{on_row} mode switches on the capture's row; off it: {off_row:?}");
    if !missing.is_empty() {
        skip(&format!("S160: no RAM image before {} of the 2p wrecks: {missing:?} (research/s155/rams.py says how they are saved)", missing.len()));
        return;
    }
    assert!(entered >= TWO_P_RAGDOLLS, "only {entered} of the 2p wrecks were entered and put back (was {TWO_P_RAGDOLLS})");
    assert!(held >= TWO_P_BONES_TO_THE_WRECK, "only {held} runs held the captured bones to the wreck (was {TWO_P_BONES_TO_THE_WRECK})");
    assert!(on_row >= TWO_P_SWITCHES_ON_THE_ROW, "only {on_row} mode switches on the capture's row (was {TWO_P_SWITCHES_ON_THE_ROW})");
    assert!(went.touching > 0 && went.landed > 0 && went.settles >= entered - 1, "the runs never touched, landed or settled: {went:?}");
}
