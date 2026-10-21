//! S70 — the hand-over from the riding skeleton to the wreck's bones, against the original.
//!
//! The original's wreck entry poses every bone where the skeleton's node is and takes the bone's
//! velocity and spin from how far the node moved and turned since its matrix was last drawn, over
//! the tick's frame time; it hands no bone the bike's velocity. Its riding skeleton is the riding
//! ragdoll's bodies: after each step every node is set to its body's position less the body's
//! centre of mass turned by its orientation, so the nodes the entry reads on tick T are the bodies
//! at the end of T−1, and the drawn matrices are the nodes at the end of T−2.
//!
//! 1. [`the_hand_over_is_the_originals`]: S56's side wreck, from the state the tick before it, run
//!    through the original's own update; the port's pose from what the original's pose read is its
//!    bones' starts bit for bit, and the starts survive the tick in RAM.
//! 2. [`the_riding_bodies_hand_over`]: all four S56 wrecks, the port's hand-over
//!    ([`ragdoll_ride::node_of`], [`ragdoll_ride::drawn`], the pose with the tick's frame time)
//!    from the captured bodies of the two ticks before, against the starts the original left in
//!    RAM. Close, not bit-exact: the skeleton's node evaluation is not ported.

use game_sim::air::AcosTable;
use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::math::{Mat4, quat_to_mat4};
use game_sim::ragdoll_pose::{self, Bone, Start};
use game_sim::ragdoll_ride;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

/// The rider body actor in every s56 state (the human's vehicle less 0x400).
const ACTOR: u32 = 0x017a_7700;
const PARTS: u32 = 0x2f4;

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

    fn bytes(&self, frame: u32, col: &str) -> Vec<u8> {
        let s = self.text(frame, col);
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex")).collect()
    }
}

fn state(name: &str, frame: u32) -> Option<Ee> {
    let path = ctx(&format!("capture/s56/{name}-{frame}.ee.bin"))?;
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    if let Some(spr) = ctx(&format!("capture/s56/{name}-{frame}.spr.bin")) {
        ee.spr = std::fs::read(spr).expect("the scratchpad");
    }
    assert_eq!(ee.u32_at(ACTOR + 0x2e0), 18, "no rider body actor at {ACTOR:#x}");
    assert_eq!(ee.u32_at(addr("frame_counter").unwrap()), frame, "the state is not frame {frame}");
    Some(ee)
}

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| ee.f32_at(a + k)))
}

fn set_bytes(ee: &mut Ee, a: u32, bytes: &[u8]) {
    let at = (a & 0x01ff_ffff) as usize;
    ee.ram[at..at + bytes.len()].copy_from_slice(bytes);
}

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

fn through(ee: &mut Ee) {
    let (pc, ra, sp, steps, stack) = (ee.pc, ee.r(31), ee.r(29), ee.steps, ee.stack);
    let args: Vec<u64> = (4..12).map(|k| ee.r(k)).collect();
    let floats: Vec<f32> = (12..20).map(|k| f32::from_bits(ee.fpr[k])).collect();
    ee.stack = (sp as u32 - 0x800) & !0xf;
    ee.call(pc, &args, &floats, 2_000_000_000);
    ee.set(31, ra);
    ee.set(29, sp);
    (ee.steps, ee.stack, ee.pc) = (steps, stack, pc);
}

fn part(ee: &Ee, k: usize) -> u32 {
    ee.u32_at(ACTOR + PARTS) + 0xb0 * k as u32
}

fn body(ee: &Ee, k: usize) -> u32 {
    ee.u32_at(part(ee, k) + 0xa4)
}

fn curve(ee: &Ee) -> CurveTable {
    let base = ee.u32_at(addr("curve_table_ptr").unwrap());
    let entries: Vec<[f32; 2]> =
        (0..CURVE_ENTRIES as u32).map(|k| [ee.f32_at(base + 8 * k), ee.f32_at(base + 8 * k + 4)]).collect();
    CurveTable::from_entries(&entries)
}

fn bits(v: Vf) -> [u32; 3] {
    [v.0[0].to_bits(), v.0[1].to_bits(), v.0[2].to_bits()]
}

/// A bone body's starts as the pose leaves them: position, velocity, orientation, rates.
fn starts_of(ee: &Ee) -> Vec<[Vf; 4]> {
    (0..18).map(|k| [0x30, 0x40, 0x160, 0x140].map(|o| quad(ee, body(ee, k) + o))).collect()
}

#[derive(Default)]
struct Seen {
    dt: Vec<f32>,
    still: Vec<u64>,
    drawn: Vec<Vec<Mat4>>,
    starts: Vec<Vec<[Vf; 4]>>,
    nodes: Vec<Vf>,
    axes: Vec<(Vf, Vf)>,
}

#[test]
fn the_hand_over_is_the_originals() {
    let Some(cap) = Capture::load("side") else { return };
    let Some(mut ee) = state("side", 2604) else { return };
    let Some(after) = state("side", 2605) else { return };
    let seen = Rc::new(RefCell::new(Seen::default()));
    let s = Rc::clone(&seen);
    ee.hooks.insert(
        addr("rider_ragdoll_pose").unwrap(),
        Box::new(move |ee: &mut Ee| {
            let drawn = (0..18).map(|k| Mat4([0, 0x10, 0x20, 0x30].map(|o| quad(ee, part(ee, k) + 0x60 + o)))).collect();
            {
                let mut s = s.borrow_mut();
                s.dt.push(f32::from_bits(ee.fpr[12]));
                s.still.push(ee.r(5));
                s.drawn.push(drawn);
            }
            through(ee);
            let starts = starts_of(ee);
            s.borrow_mut().starts.push(starts);
        }),
    );
    let s = Rc::clone(&seen);
    ee.hooks.insert(
        addr("node_world_position").unwrap(),
        Box::new(move |ee: &mut Ee| {
            let out = ee.r(6) as u32;
            through(ee);
            s.borrow_mut().nodes.push(quad(ee, out));
        }),
    );
    let s = Rc::clone(&seen);
    ee.hooks.insert(
        addr("orientation_axes").unwrap(),
        Box::new(move |ee: &mut Ee| {
            let (f, u) = (ee.r(6) as u32, ee.r(7) as u32);
            through(ee);
            s.borrow_mut().axes.push((quad(ee, f), quad(ee, u)));
        }),
    );
    let cofgs: Vec<Vf> = (0..18).map(|k| quad(&ee, body(&ee, k) + 0x180)).collect();
    let (acos, curve) = (AcosTable::build(), curve(&ee));
    lay_in(&mut ee, &cap, 2605);
    let dt = cap.get(2605, "dt");
    let bike = Vf::new(cap.get(2605, "bike_vel.x"), cap.get(2605, "bike_vel.y"), cap.get(2605, "bike_vel.z"), 0.0);
    ee.call(addr("rider_body_update").unwrap(), &[u64::from(ACTOR)], &[dt], 2_000_000_000);
    let s = seen.borrow();

    // One pose, moving, over the tick's frame time, reading every node once.
    assert_eq!(s.starts.len(), 1, "the entry poses once");
    assert_eq!(s.still, [0], "the entry's pose is not the still one");
    assert_eq!(s.dt[0].to_bits(), dt.to_bits(), "the pose runs over the tick's frame time");
    assert_eq!((s.nodes.len(), s.axes.len()), (18, 18), "the pose reads every node");

    let bones: Vec<Bone> = (0..18).map(|k| Bone { at: s.nodes[k], forward: s.axes[k].0, up: s.axes[k].1 }).collect();
    let mut drawn = s.drawn[0].clone();
    let mut ours = [Start::default(); 18];
    ragdoll_pose::pose_all(
        bones.iter().zip(&cofgs).zip(drawn.iter_mut().zip(ours.iter_mut())).map(|((b, c), (d, st))| (b, *c, d, st)),
        false,
        F(dt),
        &acos,
        &curve,
    );
    let theirs = &s.starts[0];
    let kept = starts_of(&after);
    let mut off_bike = 0;
    for k in 0..18 {
        let o = &ours[k];
        let t = theirs[k];
        assert_eq!(bits(o.position), bits(t[0]), "bone {k}'s start position");
        assert_eq!(bits(o.velocity), bits(t[1]), "bone {k}'s start velocity");
        assert_eq!(o.quat.to_bits(), t[2].to_bits(), "bone {k}'s start orientation");
        assert_eq!(bits(o.rates), bits(t[3]), "bone {k}'s start rates");
        for (j, what) in ["position", "velocity", "orientation", "rates"].iter().enumerate() {
            let w = if j == 2 { 4 } else { 3 };
            assert_eq!(kept[k][j].to_bits()[..w], t[j].to_bits()[..w], "bone {k}'s start {what} is still in RAM after the tick");
        }
        if bits(o.velocity) != bits(bike) {
            off_bike += 1;
        }
    }
    assert_eq!(off_bike, 18, "no bone starts with the bike's velocity");
}

/// Each S56 wreck's entry tick, and the state saved at the end of it.
const WRECKS: [(&str, u32); 4] = [("front", 2600), ("side", 2605), ("back", 2475), ("tip", 2474)];

/// How far the port's hand-over from the captured bodies may be from the original's: the node's
/// position is a few ulps off at four thousand units, which the frame time multiplies by thirty,
/// and its axes tens of ulps, which the rates see the same way (0.03 and 0.1 at most on S56's).
const NEAR: [f32; 4] = [0.002, 0.1, 1e-4, 0.15];

#[test]
fn the_riding_bodies_hand_over() {
    let (mut compared, mut torn, mut report) = (0, 0, Vec::new());
    for (name, entry) in WRECKS {
        let Some(cap) = Capture::load(name) else { return };
        let Some(after) = state(name, entry) else { return };
        assert_eq!(cap.get(entry, "a_mode"), 2.0, "{name}: {entry} is in the wreck");
        assert_eq!(cap.get(entry - 1, "a_mode"), 1.0, "{name}: {} is riding", entry - 1);
        let (acos, curve) = (AcosTable::build(), curve(&after));
        let cofgs: Vec<Vf> = (0..18).map(|k| quad(&after, body(&after, k) + 0x180)).collect();
        let row = |f: u32, k: usize, c: &str| {
            let get = |a: &str| cap.get(f, &format!("bone{k}.{c}.{a}"));
            Vf::new(get("x"), get("y"), get("z"), get("w"))
        };
        let nodes_at = |f: u32| -> Vec<Bone> {
            (0..18)
                .map(|k| {
                    let (at, q) = ragdoll_ride::node_of(row(f, k, "pos"), row(f, k, "quat"), cofgs[k]);
                    let m = quat_to_mat4(q, at);
                    Bone { at, forward: m.0[2], up: m.0[1] }
                })
                .collect()
        };
        // A tick whose row repeats the one before left the bodies where they were, so the drawn
        // matrices are the nodes at the last row that differs.
        let same = |a: u32, b: u32| (0..18).all(|k| bits(row(a, k, "pos")) == bits(row(b, k, "pos")));
        let mut last = entry - 2;
        while same(last, entry - 1) {
            last -= 1;
        }
        let now = nodes_at(entry - 1);
        let mut drawn = ragdoll_ride::drawn(&nodes_at(last));
        let mut ours = [Start::default(); 18];
        ragdoll_pose::pose_all(
            now.iter().zip(&cofgs).zip(drawn.iter_mut().zip(ours.iter_mut())).map(|((b, c), (d, st))| (b, *c, d, st)),
            false,
            F(cap.get(entry, "dt")),
            &acos,
            &curve,
        );
        let theirs = starts_of(&after);
        let mut worst = [0f32; 4];
        for k in 0..18 {
            let o = [ours[k].position, ours[k].velocity, ours[k].quat, ours[k].rates];
            for j in 0..4 {
                let w = if j == 2 { 4 } else { 3 };
                for i in 0..w {
                    worst[j] = worst[j].max((o[j].0[i] - theirs[k][j].0[i]).abs());
                }
            }
        }
        eprintln!("{name}: worst position {} velocity {} orientation {} rates {}", worst[0], worst[1], worst[2], worst[3]);
        // A torn row: the capture's tick before the entry holds the tick before that, so the
        // nodes it gives are not the ones the original posed from.
        if last != entry - 2 && worst[0] > NEAR[0] {
            eprintln!("{name}: row {} repeats row {last} but the original's nodes moved: torn, not compared", entry - 1);
            torn += 1;
            continue;
        }
        report.push((name, worst));
        compared += 1;
    }
    assert!(torn <= 1, "{torn} wrecks torn");
    for (name, worst) in report {
        for (j, what) in ["position", "velocity", "orientation", "rates"].iter().enumerate() {
            assert!(worst[j] <= NEAR[j], "{name}: the starts' {what} is {} off the original's", worst[j]);
        }
    }
    assert_eq!(compared + torn, 4, "every wreck compared");
    assert!(compared >= 3, "only {compared} wrecks compared");
}
