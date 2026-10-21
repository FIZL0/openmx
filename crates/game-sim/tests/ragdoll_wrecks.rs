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
const ACTOR: u32 = 0x017a_7700;
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
        let text = std::fs::read_to_string(ctx(&format!("capture/s56/{name}.csv"))?).expect("read the capture");
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
    let path = ctx(&format!("capture/s56/{}-{}.ee.bin", w.name, w.frame))?;
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    if let Some(spr) = ctx(&format!("capture/s56/{}-{}.spr.bin", w.name, w.frame)) {
        ee.spr = std::fs::read(spr).expect("the scratchpad");
    }
    assert_eq!(ee.u32_at(ACTOR + 0x2e0), 18, "no rider body actor at {ACTOR:#x}");
    assert_eq!(ee.u32_at(addr("frame_counter").unwrap()), w.frame, "the state is not frame {}", w.frame);
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

/// What the body's update reads off the rest of the game in tick `frame`, laid into `ee` from the
/// capture: the riding request (and the hand dummies 0x200 before it) and the bike centre its +0x1ac
/// points at.
fn lay_in(ee: &mut Ee, cap: &Capture, frame: u32) {
    let req = ee.u32_at(ACTOR + 0x488);
    set_bytes(ee, req - 0x200, &cap.bytes(frame, "req_area"));
    let far = ee.u32_at(req + 0x1ac);
    set_bytes(ee, far, &cap.bytes(frame, "req_far"));
    for (col, _) in cap.cols.iter().filter(|(c, _)| c.starts_with('w') && c.len() == 8) {
        let at = u32::from_str_radix(&col[1..], 16).expect("a world block's address");
        if (0x0179_0000..0x0181_0000).contains(&at) {
            continue;
        }
        set_bytes(ee, at, &cap.bytes(frame, col));
    }
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
        let req = ee.u32_at(ACTOR + 0x488);
        let far = ee.u32_at(req + 0x1ac);
        let mut frame = w.frame + 1;
        let mut parted = None;
        while cap.has(frame) {
            *ee.reads.borrow_mut() = Some(Default::default());
            if originals_tick(&mut ee, &cap, frame).is_err() {
                parted.get_or_insert(frame);
                set_rng(&mut ee, cap.int(frame - 1, "rng_state"));
                ee.call(addr("rider_body_update").unwrap(), &[u64::from(ACTOR)], &[cap.get(frame, "dt")], 2_000_000_000);
            }
            let reads = ee.reads.borrow_mut().take().unwrap();
            wanted.extend(reads.into_iter().filter(|&x| {
                x >> 28 != 7
                    && !ee.written.contains(&x)
                    && !(req - 0x200..req + 0x300).contains(&x)
                    && !(far..far + 0x100).contains(&x)
                    && !(ACTOR..ACTOR + 0x1000).contains(&x)
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
