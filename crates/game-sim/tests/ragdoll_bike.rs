//! S62 — the rider's ragdoll against the wreck bike and the other moving collidables, tick by tick.
//!
//! S56's oracle replay (tests/ragdoll_wrecks.rs) runs the original's own body update on a wreck's save
//! state with each tick's riding request laid in from the capture, and holds every bone bit for bit,
//! up to the first tick a bone touches something that moves: the capture did not carry it. The s62
//! captures (`research/s62_wreck.py`, watch/s56-ragdoll.toml) carry the wreck bike's whole object by
//! chain off the request's +0x1ac, and the broad phase's list pools whole; this replays the four
//! wrecks from their first state on, through the bike contact.
//!
//! The captures and their states (`<name>-<frame>.p2s`, with `.ee.bin`/`.spr.bin` RAM and scratchpad
//! images written next to them) are in context/capture/s62, git-ignored.

use game_sim::rng::Rng;
use ps2_float::Vf;
use ps2_oracle::{Ee, Kit, addr, skip};
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

/// The rider body actor in every s62 state (the human's vehicle less 0x400), as in S56's.
const ACTOR: u32 = 0x017a_7700;
const PARTS: u32 = 0x2f4;

/// One wreck: its capture's name, the frame of its first state and of its late one.
struct Wreck {
    name: &'static str,
    frame: u32,
    late: u32,
}

const WRECKS: [Wreck; 5] = [
    Wreck { name: "front", frame: 2602, late: 3002 },
    Wreck { name: "side", frame: 2607, late: 3007 },
    Wreck { name: "back", frame: 2477, late: 2877 },
    Wreck { name: "tip", frame: 2472, late: 2872 },
    Wreck { name: "front3", frame: 2630, late: 2668 },
];

/// Where an exploration writes what it found (a passing check keeps no log).
fn note(text: &str) {
    use std::io::Write;
    let Ok(kit) = Kit::load() else { return };
    let path = kit.root.join("context/capture/s62/notes.txt");
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path).expect("the notes");
    writeln!(f, "{text}").expect("write the notes");
}

fn ctx(rel: &str) -> Option<PathBuf> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let local = kit.root.join("context").join(rel);
    let path = if local.exists() { local } else { kit.ctx(rel) };
    if !path.exists() {
        skip(&format!("{} is not there (S62's captures: research/s62_wreck.py)", path.display()));
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
        let text = std::fs::read_to_string(ctx(&format!("capture/s62/{name}.csv"))?).expect("read the capture");
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

fn state(name: &str, frame: u32) -> Option<Ee> {
    let path = ctx(&format!("capture/s62/{name}-{frame}.ee.bin"))?;
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    if let Some(spr) = ctx(&format!("capture/s62/{name}-{frame}.spr.bin")) {
        ee.spr = std::fs::read(spr).expect("the scratchpad");
    }
    assert_eq!(ee.u32_at(ACTOR + 0x2e0), 18, "no rider body actor at {ACTOR:#x}");
    assert_eq!(ee.u32_at(addr("frame_counter").unwrap()), frame, "the state is not frame {frame}");
    Some(ee)
}

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| ee.f32_at(a + k)))
}

fn bone_body(ee: &Ee, i: usize) -> u32 {
    ee.u32_at(ee.u32_at(ACTOR + PARTS) + 0xb0 * i as u32 + 0xa4)
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

/// Where the wreck bike's object starts, relative to the pointer at the request's +0x1ac, and the two
/// fields it is captured in (watch/s56-ragdoll.toml).
const BIKE_A: u32 = 0x980;
const BIKE_B: u32 = 0x680;

/// What the body's update reads off the rest of the game in tick `frame`, laid into `ee` from the
/// capture: the riding request (and the hand dummies 0x200 before it), the wreck bike's object by its
/// pointer, and the absolute world blocks. Returns the ranges laid in.
fn lay_in(ee: &mut Ee, cap: &Capture, frame: u32) -> Vec<(u32, u32)> {
    let mut laid = Vec::new();
    let mut put = |ee: &mut Ee, a: u32, bytes: &[u8]| {
        set_bytes(ee, a, bytes);
        laid.push((a & 0x01ff_ffff, (a & 0x01ff_ffff) + bytes.len() as u32));
    };
    let req = ee.u32_at(ACTOR + 0x488);
    put(ee, req - 0x200, &cap.bytes(frame, "req_area"));
    let far = ee.u32_at(req + 0x1ac);
    put(ee, far - BIKE_A, &cap.bytes(frame, "bike_a"));
    put(ee, far + BIKE_B, &cap.bytes(frame, "bike_b"));
    put(ee, far, &cap.bytes(frame, "req_far"));
    for (col, _) in cap.cols.iter().filter(|(c, _)| c.starts_with('w') && c.len() == 8) {
        let at = u32::from_str_radix(&col[1..], 16).expect("a world block's address");
        if (0x0179_0000..0x0181_0000).contains(&at) {
            continue; // the rider's own records: the update writes them, the Ee carries them
        }
        put(ee, at, &cap.bytes(frame, col));
    }
    laid
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

/// One tick of the original's own update in the oracle, with the moving world laid in from the
/// capture, checked against every captured bone; the draws the rest of the game makes first are found
/// by trying each count (S56). `Ok` holds that count and the body's own draws.
fn originals_tick(ee: &mut Ee, cap: &Capture, frame: u32) -> Result<(u32, u32), Vec<String>> {
    let update = addr("rider_body_update").unwrap();
    lay_in(ee, cap, frame);
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
        ee.call(update, &[u64::from(ACTOR)], &[cap.get(frame, "dt")], 2_000_000_000);
        let drew = draws(r.state(), rng_at(ee), 1000).expect("the body's draws");
        let d = differ(cap, frame, &ee_bones(ee));
        if d.is_empty() {
            return Ok((n, drew));
        }
        first.get_or_insert(d);
        if drew == 0 {
            break;
        }
    }
    snap.back(ee);
    Err(first.unwrap())
}

/// A tick whose row is torn (the recorder read it mid-tick, see frame-counter captures): run it
/// unchecked with each count of draws before it, and keep the one under which the next tick replays
/// exact. `Ok` holds the two counts.
fn through_torn(ee: &mut Ee, cap: &Capture, frame: u32) -> Option<(u32, u32)> {
    let update = addr("rider_body_update").unwrap();
    lay_in(ee, cap, frame);
    let before = cap.int(frame - 1, "rng_state");
    let snap = Snapshot::of(ee);
    for n in 0..64 {
        snap.back(ee);
        let mut r = Rng::from_state(before);
        (0..n).for_each(|_| {
            r.rand();
        });
        set_rng(ee, r.state());
        ee.call(update, &[u64::from(ACTOR)], &[cap.get(frame, "dt")], 2_000_000_000);
        if let Ok((m, _)) = originals_tick(ee, cap, frame + 1) {
            return Some((n, m));
        }
    }
    snap.back(ee);
    None
}

/// The bones' distance from the bike's body in a tick, the nearest, as a check that a wreck's replay
/// really reaches the bike: the bike body's position is at -0x780+0x60 of the request's +0x1ac.
fn nearest_to_bike(ee: &Ee) -> f32 {
    let req = ee.u32_at(ACTOR + 0x488);
    let bike = quad(ee, ee.u32_at(req + 0x1ac) - 0x780 + 0x60);
    (0..18)
        .map(|i| {
            let p = quad(ee, bone_body(ee, i) + 0x60);
            let d = [0, 1, 2].map(|k| p[k] - bike[k]);
            (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
        })
        .fold(f32::INFINITY, f32::min)
}

/// Exploration: each wreck replayed from its first state to the end of its capture; where it parts,
/// what the tick read that the capture did not lay in, that the tick did not write, and that the game
/// changed between the wreck's first and late states: the moving world still missing.
#[test]
#[ignore = "exploration"]
fn what_the_bike_contact_reads() {
    for w in &WRECKS {
        let Some(cap) = Capture::load(w.name) else { return };
        let Some(mut ee) = state(w.name, w.frame) else { return };
        let late = std::fs::read(ctx(&format!("capture/s62/{}-{}.ee.bin", w.name, w.late)).unwrap()).unwrap();
        let _changing: BTreeSet<u32> = (0..late.len()).filter(|&i| late[i] != ee.ram[i]).map(|i| i as u32).collect();
        let mut frame = w.frame + 1;
        let mut closest = f32::INFINITY;
        let mut log = Vec::new();
        let mut seen: BTreeSet<u32> = BTreeSet::new();
        let mut touching = Vec::new();
        let others: std::rc::Rc<std::cell::RefCell<std::collections::BTreeMap<u32, u32>>> = Default::default();
        let into = std::rc::Rc::clone(&others);
        let push_out = addr("bone_collision_push_out").unwrap();
        ee.hooks.insert(
            push_out,
            Box::new(move |ee: &mut Ee| {
                let (gpr, fpr, steps, stack) = (ee.gpr, ee.fpr, ee.steps, ee.stack);
                *into.borrow_mut().entry(ee.u32_at(ee.r(4) as u32 + 0x70)).or_default() += 1;
                ee.stack = (ee.r(29) as u32 - 0x400) & !0xf;
                let (a, b, c) = (ee.r(4), ee.r(5), ee.r(6));
                ee.call(push_out, &[a, b, c], &[], 2_000_000_000);
                (ee.gpr, ee.fpr, ee.steps, ee.stack) = (gpr, fpr, steps, stack);
            }),
        );
        while cap.has(frame) {
            if repeated(&cap, frame) {
                frame += 1;
                continue;
            }
            *ee.reads.borrow_mut() = Some(Default::default());
            let tick = originals_tick(&mut ee, &cap, frame);
            let read = ee.reads.borrow_mut().take().unwrap();
            let far = ee.u32_at(ee.u32_at(ACTOR + 0x488) + 0x1ac) & 0x01ff_ffff;
            let rates = far - 0x780 + 0x1b0;
            if read.iter().any(|&x| x & 0x01ff_ffff == rates) {
                touching.push(frame);
            }
            match tick {
                Ok((n, drew)) => {
                    seen.extend(read.into_iter().map(|x| x & 0x01ff_ffff));
                    if drew > 0 {
                        log.push(format!("{frame}: {n} draws before, {drew} by the body"));
                    }
                    closest = closest.min(nearest_to_bike(&ee));
                }
                Err(d) if cap.has(frame + 1) && through_torn(&mut ee, &cap, frame).is_some() => {
                    log.push(format!("{frame}: torn ({} lanes), {} exact after it", d.len(), frame + 1));
                    frame += 1;
                }
                Err(d) => {
                    // Run the tick once more with the reads tracked.
                    let laid = lay_in(&mut ee, &cap, frame);
                    set_rng(&mut ee, cap.int(frame - 1, "rng_state"));
                    *ee.reads.borrow_mut() = Some(Default::default());
                    ee.call(addr("rider_body_update").unwrap(), &[u64::from(ACTOR)], &[cap.get(frame, "dt")], 2_000_000_000);
                    let reads = ee.reads.borrow_mut().take().unwrap();
                    let missing: Vec<u32> = reads
                        .into_iter()
                        .map(|x| x & 0x01ff_ffff)
                        .filter(|&x| {
                            !ee.written.contains(&x)
                                && !ee.written.contains(&(x | 0x2000_0000))
                                && !laid.iter().any(|&(a, b)| (a..b).contains(&x))
                                && !seen.contains(&x)
                        })
                        .collect();
                    let mut ranges: Vec<(u32, u32)> = Vec::new();
                    for x in missing {
                        match ranges.last_mut() {
                            Some(last) if last.1 + 32 >= x => last.1 = x + 1,
                            _ => ranges.push((x, x + 1)),
                        }
                    }
                    note(&format!(
                        "{} from {}: exact to {}, {frame} parts in {} lanes {:?}; nearest bone to the bike {closest}",
                        w.name,
                        w.frame,
                        frame - 1,
                        d.len(),
                        &d[..d.len().min(3)]
                    ));
                    note(&format!(
                        "  missing: {}",
                        ranges.iter().map(|(a, b)| format!("{a:#x}+{}", b - a)).collect::<Vec<_>>().join(" ")
                    ));
                    break;
                }
            }
            frame += 1;
        }
        if !cap.has(frame) {
            note(&format!("{} from {}: exact through {}; nearest bone to the bike {closest}", w.name, w.frame, frame - 1));
        }
        for l in &log {
            note(&format!("  {} {l}", w.name));
        }
        note(&format!("  {} push-outs by other side (counted over every try): {:x?}", w.name, others.borrow()));
        note(&format!("  {} reads the bike body's rates in {} ticks: {:?}", w.name, touching.len(), &touching[..touching.len().min(40)]));
    }
}

/// Exploration: the front wreck's parting tick laid in with parts of the previous row: which ones
/// replay it exact.
#[test]
#[ignore = "exploration"]
fn which_row_the_front_reads() {
    let Some(cap) = Capture::load("front") else { return };
    let Some(mut ee) = state("front", 2602) else { return };
    for frame in 2603..2628 {
        originals_tick(&mut ee, &cap, frame).expect("exact to 2627");
    }
    let snap = Snapshot::of(&ee);
    let req = ee.u32_at(ACTOR + 0x488);
    let far = ee.u32_at(req + 0x1ac);
    for (what, cols) in [
        ("bike", &["bike_a", "bike_b", "req_far"][..]),
        ("request", &["req_area"][..]),
        ("pools", &["w0601800", "w0e37000", "w11b1000", "w1672800", "w1673800"][..]),
        ("all", &["bike_a", "bike_b", "req_far", "req_area", "w0601800", "w0e37000", "w11b1000", "w1672800", "w1673800"][..]),
    ] {
        let mut best = usize::MAX;
        for n in 0..32 {
            snap.back(&mut ee);
            lay_in(&mut ee, &cap, 2628);
            for &c in cols {
                let at = match c {
                    "bike_a" => far - BIKE_A,
                    "bike_b" => far + BIKE_B,
                    "req_far" => far,
                    "req_area" => req - 0x200,
                    w => u32::from_str_radix(&w[1..], 16).unwrap(),
                };
                if cap.cols.contains_key(c) {
                    set_bytes(&mut ee, at, &cap.bytes(2627, c));
                }
            }
            let mut r = Rng::from_state(cap.int(2627, "rng_state"));
            (0..n).for_each(|_| {
                r.rand();
            });
            set_rng(&mut ee, r.state());
            ee.call(addr("rider_body_update").unwrap(), &[u64::from(ACTOR)], &[cap.get(2628, "dt")], 2_000_000_000);
            best = best.min(differ(&cap, 2628, &ee_bones(&ee)).len());
        }
        note(&format!("front 2628 with the {what} from 2627: {best} lanes off"));
    }
    for f in 2620..2632 {
        note(&format!("front {f}: {:?} draws in the frame", draws(cap.int(f - 1, "rng_state"), cap.int(f, "rng_state"), 5000)));
    }
    let mut best = usize::MAX;
    for n in 0..16 {
        for m in 0..8 {
            snap.back(&mut ee);
            lay_in(&mut ee, &cap, 2628);
            let mut r = Rng::from_state(cap.int(2627, "rng_state"));
            (0..n).for_each(|_| {
                r.rand();
            });
            set_rng(&mut ee, r.state());
            ee.call(addr("rider_body_update").unwrap(), &[u64::from(ACTOR)], &[cap.get(2628, "dt")], 2_000_000_000);
            let mut r = Rng::from_state(rng_at(&ee));
            (0..m).for_each(|_| {
                r.rand();
            });
            set_rng(&mut ee, r.state());
            ee.call(addr("rider_body_update").unwrap(), &[u64::from(ACTOR)], &[cap.get(2628, "dt")], 2_000_000_000);
            best = best.min(differ(&cap, 2628, &ee_bones(&ee)).len());
        }
    }
    note(&format!("front 2628 as two updates: {best} lanes off"));
}

/// Exploration: the front wreck's parting tick, bone by bone: how far each captured field is off.
#[test]
#[ignore = "exploration"]
fn where_the_front_parts() {
    let Some(cap) = Capture::load("front") else { return };
    let Some(mut ee) = state("front", 2602) else { return };
    for frame in 2603..2628 {
        originals_tick(&mut ee, &cap, frame).expect("exact to 2627");
    }
    let start = ee_bones(&ee);
    let snap = Snapshot::of(&ee);
    let mut best: Option<(usize, u32)> = None;
    for n in 0..24 {
        snap.back(&mut ee);
        lay_in(&mut ee, &cap, 2628);
        let mut r = Rng::from_state(cap.int(2627, "rng_state"));
        (0..n).for_each(|_| {
            r.rand();
        });
        set_rng(&mut ee, r.state());
        ee.call(addr("rider_body_update").unwrap(), &[u64::from(ACTOR)], &[cap.get(2628, "dt")], 2_000_000_000);
        let d = differ(&cap, 2628, &ee_bones(&ee)).len();
        if best.is_none_or(|(b, _)| d < b) {
            best = Some((d, n));
        }
    }
    note(&format!("front 2628: fewest lanes off {best:?}"));
    let n = best.unwrap().1;
    snap.back(&mut ee);
    lay_in(&mut ee, &cap, 2628);
    let mut r = Rng::from_state(cap.int(2627, "rng_state"));
    (0..n).for_each(|_| {
        r.rand();
    });
    set_rng(&mut ee, r.state());
    ee.call(addr("rider_body_update").unwrap(), &[u64::from(ACTOR)], &[cap.get(2628, "dt")], 2_000_000_000);
    let got = ee_bones(&ee);
    for i in 0..18 {
        let line: Vec<String> = BODY
            .iter()
            .enumerate()
            .map(|(k, (name, _))| {
                let want = cap.quad(2628, &format!("bone{i}.{name}"));
                let off = (0..4).map(|l| (got[i][k][l] - want[l]).abs()).fold(0.0f32, f32::max);
                let was = (0..4).map(|l| (start[i][k][l] - want[l]).abs()).fold(0.0f32, f32::max);
                let d = [0, 1, 2].map(|l| want[l] - got[i][k][l]);
                if k == 1 || k == 2 {
                    format!("{name} {d:.3?}")
                } else {
                    format!("{name} {off:.3e} (from 2627's {was:.3e})")
                }
            })
            .collect();
        note(&format!("  bone{i}: {}", line.join(", ")));
    }
}

/// Exploration: every push-out of the front's last exact ticks and its parting one — the collision,
/// its bone body, the other side (+0x70) and how far the bone moved — run through the original's
/// own push-out from a hook that logs around it.
#[test]
#[ignore = "exploration"]
fn what_the_front_touches() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let Some(cap) = Capture::load("front") else { return };
    let Some(mut ee) = state("front", 2602) else { return };
    let push_out = addr("bone_collision_push_out").unwrap();
    let log: Rc<RefCell<Vec<String>>> = Rc::default();
    let into = Rc::clone(&log);
    ee.hooks.insert(
        push_out,
        Box::new(move |ee: &mut Ee| {
            let (gpr, fpr, steps, stack) = (ee.gpr, ee.fpr, ee.steps, ee.stack);
            let this = ee.r(4) as u32;
            let body = ee.u32_at(this + 0x7c);
            let other = ee.u32_at(this + 0x70);
            let normal = quad(ee, ee.r(6) as u32);
            let before = quad(ee, body + 0x60);
            ee.stack = (ee.r(29) as u32 - 0x400) & !0xf;
            let (a, b, c) = (ee.r(4), ee.r(5), ee.r(6));
            ee.call(push_out, &[a, b, c], &[], 2_000_000_000);
            let after = quad(ee, body + 0x60);
            let moved = [0, 1, 2].map(|k| after[k] - before[k]);
            into.borrow_mut().push(format!(
                "this {this:#x} body {body:#x} other {other:#x} normal {:?} moved {moved:?}",
                [normal[0], normal[1], normal[2]]
            ));
            (ee.gpr, ee.fpr, ee.steps, ee.stack) = (gpr, fpr, steps, stack);
        }),
    );
    for frame in 2603..=2628 {
        log.borrow_mut().clear();
        let ok = originals_tick(&mut ee, &cap, frame).is_ok();
        if frame >= 2618 {
            note(&format!("front {frame} {}: {} push-outs", if ok { "exact" } else { "parts" }, log.borrow().len()));
            for l in log.borrow().iter() {
                note(&format!("  {l}"));
            }
        }
        if !ok {
            break;
        }
    }
}

/// Exploration: every byte the replay's ticks read, compared at each later state of a wreck with the
/// original's own RAM there: the ones that differ and are not laid in are inputs the capture lacks.
#[test]
#[ignore = "exploration"]
fn what_the_states_say() {
    for w in &WRECKS {
        let Some(cap) = Capture::load(w.name) else { return };
        let Some(mut ee) = state(w.name, w.frame) else { return };
        let mut seen: BTreeSet<u32> = BTreeSet::new();
        let mut wrote: BTreeSet<u32> = BTreeSet::new();
        for frame in w.frame + 1..=w.frame + 2 {
            *ee.reads.borrow_mut() = Some(Default::default());
            originals_tick(&mut ee, &cap, frame).expect("exact");
            seen.extend(ee.reads.borrow_mut().take().unwrap().into_iter().map(|x| x & 0x01ff_ffff));
            wrote.extend(ee.written.iter().map(|x| x & 0x01ff_ffff));
            let truth = std::fs::read(ctx(&format!("capture/s62/{}-{frame}.ee.bin", w.name)).unwrap()).unwrap();
            let laid = lay_in(&mut ee, &cap, frame);
            let off: Vec<u32> = seen
                .iter()
                .copied()
                .filter(|&x| (x as usize) < truth.len() && truth[x as usize] != ee.ram[x as usize])
                .filter(|&x| !laid.iter().any(|&(a, b)| (a..b).contains(&x)))
                .collect();
            let mut ranges: Vec<(u32, u32, bool)> = Vec::new();
            for x in off {
                match ranges.last_mut() {
                    Some(last) if last.1 + 16 >= x => last.1 = x + 1,
                    _ => ranges.push((x, x + 1, wrote.contains(&x))),
                }
            }
            note(&format!(
                "{} {frame}: {} read bytes off the state ({} ranges): {}",
                w.name,
                ranges.iter().map(|r| r.1 - r.0).sum::<u32>(),
                ranges.len(),
                ranges
                    .iter()
                    .take(60)
                    .map(|(a, b, wr)| format!("{a:#x}+{}{}", b - a, if *wr { "w" } else { "" }))
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
    }
}

/// Replays `w` from its first state; the last tick it holds every bone of every row through, and
/// the ticks in which the update read the wreck bike's body (its rates: the bones against the bike).
fn replay(w: &Wreck, to: u32) -> Option<(u32, Vec<u32>)> {
    let cap = Capture::load(w.name)?;
    let mut ee = state(w.name, w.frame)?;
    let mut touching = Vec::new();
    let mut frame = w.frame + 1;
    while frame <= to && cap.has(frame) {
        if !repeated(&cap, frame) {
            *ee.reads.borrow_mut() = Some(Default::default());
            let tick = originals_tick(&mut ee, &cap, frame);
            let read = ee.reads.borrow_mut().take().unwrap();
            if tick.is_err() {
                break;
            }
            let rates = (ee.u32_at(ee.u32_at(ACTOR + 0x488) + 0x1ac) & 0x01ff_ffff) - 0x780 + 0x1b0;
            if read.iter().any(|&x| x & 0x01ff_ffff == rates) {
                touching.push(frame);
            }
        }
        frame += 1;
    }
    Some((frame - 1, touching))
}

/// The original's own update, the s62 wrecks laid in tick by tick with the wreck bike's object and
/// the broad phase's pools: side, back and the tip-over replay bit-exactly through the rider's whole
/// wreck, the put-back onto the bike (~79 ticks in) and on; the front one 25 ticks into it, through
/// eight ticks against the bike; a second front one (front3) from 22 ticks in through the put-back.
#[test]
fn the_wrecks_replay_through_the_bike() {
    let want = [
        ("front", 2627, 8),
        ("side", 3146, 1),
        ("back", 3179, 0),
        ("tip", 3179, 0),
        ("front3", 2687, 1),
    ];
    for (w, (name, to, touches)) in WRECKS.iter().zip(want) {
        assert_eq!(w.name, name);
        let Some((last, touching)) = replay(w, to + 1) else { return };
        assert_eq!(last, to, "{name}: exact to {last}");
        assert!(touching.len() >= touches, "{name}: the bike's body read in {touching:?}");
    }
}
