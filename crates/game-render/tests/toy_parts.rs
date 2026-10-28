//! The hierarchy toys' collision parts against the original (S133): free10's shovel and nat11's
//! eight gondolas, as the shell builds them from the disc, against a save state of each track
//! (R7's scripted boot: `Free10` freestyle, `Nat11` practice).
//!
//! 1. **the binding** — every toy with collision has the original's parts, in its order: the
//!    `.col` hierarchy's part `p` rides the joint whose node the original's part `p` is on (its
//!    `+0x230`), and the 800-byte kinematic body bound to the part follows that same node (`+0x254`).
//!    A mesh `.col` (free10's windsock) is one part, on the toy's root node;
//! 2. **the motion** — every toy, started from the state's clock and hints, stepped by
//!    [`Toys::frame`] beside the original's `motion_advance`, `transform_rebuild` and the kinematic
//!    body's update in the interpreter for a whole loop of its clip and over its wrap: each part's
//!    pose is the body's matrix, and its [`game_sim::traffic::Moving`] velocity and spin are what the
//!    body's two virtuals (the path car's) answer, bit for bit, on every tick.
//!
//! Needs the user's own disc and the two states, so it skips with the reason when one is not there.

use game_render::animated_toy::Animated;
use game_render::disc::{Disc, Res};
use game_sim::math::Mat4;
use game_sim::toys::{Clock, Hint, Motion, RotKeys, Toys};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

const BUDGET: u64 = 50_000_000;
/// Twelve seconds of free10: past the end of the shovel's ten-second loop, wherever its clock starts.
const FREE10_TICKS: usize = 360;
/// 130 seconds of nat11: past the end of the gondolas' 125-second loop.
const NAT11_TICKS: usize = 3900;

/// The fields (`research/oracle.txt` V43, S133).
mod at {
    pub const MODEL: u32 = 0xb0;
    pub const FINISHED: u32 = 0x3c;
    pub const HANDLE: u32 = 0x48;
    pub const PREV: u32 = 0x54;
    pub const FADING: u32 = 0x80;
    pub const TIME: u32 = 0x84;
    pub const ROOT: u32 = 0x30;
    pub const JOINT_COUNT: u32 = 0x230;
    pub const JOINTS: u32 = 0x234;
    pub const MIRROR_MAP: u32 = 0x238;
    pub const JOINT: u32 = 0x60;
    pub const NODE: u32 = 0x34;
    pub const LOCAL: u32 = 0x40;
    /// The motion object's collision parts and their count (`animated_toy_collision`).
    pub const PARTS: u32 = 0x300;
    pub const PART_COUNT: u32 = 0x304;
    /// A part (a mesh collidable): its node and its kinematic body.
    pub const PART_NODE: u32 = 0x230;
    pub const PART_BODY: u32 = 0x7c;
    /// The kinematic body: its vtable slot, its node, its matrix now.
    pub const VTABLE: u32 = 0x8;
    pub const BODY_NODE: u32 = 0x254;
    pub const BODY_NOW: u32 = 0x2a0;
    /// The clip.
    pub const POSITIONS: u32 = 0x18;
    pub const ROTATIONS: u32 = 0x1c;
    pub const POSITION_COUNT: u32 = 0x20;
    pub const ROTATION_COUNT: u32 = 0x24;
    pub const COUNT: u32 = 0x8;
    pub const KEYS: u32 = 0x14;
    pub const HINT: u32 = 0x18;
}

fn mat(ee: &Ee, a: u32) -> [[u32; 4]; 4] {
    [0, 16, 32, 48].map(|r| [0, 4, 8, 12].map(|c| ee.u32_at(a + r + c)))
}

fn bits(m: &Mat4) -> [[u32; 4]; 4] {
    m.0.map(Vf::to_bits)
}

fn xyz(v: Vf) -> [u32; 3] {
    let b = v.to_bits();
    [b[0], b[1], b[2]]
}

/// The track's archives: `<track>.res` and `<track>_2.res`, as the shell opens them.
fn archives(track: &str) -> Option<Vec<Res>> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            println!("SKIPPED: {why}");
            return None;
        }
    };
    let names: Vec<String> = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.ends_with(&format!("\\{track}.res")) || n.ends_with(&format!("\\{track}_2.res"))
        })
        .collect();
    assert!(!names.is_empty(), "no archive of {track} on the disc");
    Some(names.iter().map(|n| disc.res(n).unwrap_or_else(|e| panic!("{n}: {e:?}"))).collect())
}

/// A clip's channels as raw words: each rotation channel's name and keys, then each position
/// channel's — the state's, read through the clip's lists.
fn clip_words(ee: &Ee, clip: u32, motion: &Motion) -> Option<Vec<u32>> {
    let list = |at_list, at_count| -> Vec<u32> {
        let (l, n) = (ee.u32_at(clip + at_list), ee.u32_at(clip + at_count));
        (0..n).map(|c| ee.u32_at(l + 4 * c)).collect()
    };
    let (rot, pos) = (list(at::ROTATIONS, at::ROTATION_COUNT), list(at::POSITIONS, at::POSITION_COUNT));
    if rot.len() != motion.rotation.len() || pos.len() != motion.position.len() {
        return None;
    }
    let mut out = Vec::new();
    for (ours, &ch) in motion.rotation.iter().zip(&rot) {
        let (n, keys) = (ee.u32_at(ch + at::COUNT), ee.u32_at(ch + at::KEYS));
        let words = match &ours.keys {
            RotKeys::Wide(_) => n * 3,
            RotKeys::Narrow(_) => n * 2,
        };
        out.push(ee.u32_at(ch));
        out.extend((0..words).map(|w| ee.u32_at(keys + 4 * w)));
    }
    for &ch in &pos {
        let (n, keys) = (ee.u32_at(ch + at::COUNT), ee.u32_at(ch + at::KEYS));
        out.push(ee.u32_at(ch));
        out.extend((0..n * 4).map(|w| ee.u32_at(keys + 4 * w)));
    }
    Some(out)
}

/// The same words out of the port's motion.
fn motion_words(motion: &Motion) -> Vec<u32> {
    let mut out = Vec::new();
    for c in &motion.rotation {
        out.push(c.name);
        match &c.keys {
            RotKeys::Wide(k) => out.extend(k.iter().flat_map(|k| {
                let a = k.axis.map(|a| a as u16 as u32);
                [k.t.to_bits(), u32::from(k.turn) | a[0] << 16, a[1] | a[2] << 16]
            })),
            RotKeys::Narrow(k) => out.extend(k.iter().flat_map(|k| {
                let a = k.axis.map(|a| a as u8 as u32);
                [k.t.to_bits(), u32::from(k.turn) | a[0] << 8 | a[1] << 16 | a[2] << 24]
            })),
        }
    }
    for c in &motion.position {
        out.push(c.name);
        out.extend(c.keys.iter().flat_map(|k| [k.t.to_bits(), k.p[0].to_bits(), k.p[1].to_bits(), k.p[2].to_bits()]));
    }
    out
}

/// A world of nothing but props, which the toys move.
struct Shelf(game_sim::world::Props);

impl game_sim::world::World for Shelf {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        (Vf::new(at.x(), -1.0e6, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 1.0))
    }
    fn solid(&mut self, _bundle: &game_sim::collide::Bundle, _record: &mut game_sim::collide::Record) -> bool {
        false
    }
    fn moving(&mut self) -> Option<&mut game_sim::world::Props> {
        Some(&mut self.0)
    }
}

/// One state toy the port's toy `k` is: its motion object, its joints' nodes, its parts' bodies.
struct Matched {
    model: u32,
    nodes: Vec<u32>,
    bodies: Vec<u32>,
}

/// The track's toys, built from the disc, against the state: the binding checked, then `ticks`
/// ticks of the motion. Returns how many of the parts' (tick, part) steps moved.
fn parts_ride_their_nodes(track: &str, state: &str, toys_want: usize, parts_want: usize, ticks: usize) -> Option<usize> {
    let mut archives = archives(track)?;
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("states").join(state);
    if !path.exists() {
        skip(&format!("{} is not there: S133's save state of {track} (R7's scripted boot, then `./kit state ram`)", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let animated = Animated::load(&mut archives, track);
    assert!(animated.skipped.is_empty(), "{track}: {:?}", animated.skipped);
    let specs = &animated.sim.specs;
    assert_eq!(specs.len(), toys_want, "{track}'s toys");

    let vtable = addr("animated_toy_vtable").unwrap();
    let body_vtable = addr("kinematic_body_vtable").unwrap();
    let toys: Vec<u32> = (0..32u32 << 20).step_by(4).filter(|&a| ee.u32_at(a) == vtable).map(|a| a - 8).collect();
    assert_eq!(toys.len(), toys_want, "{track}: the state's animated toys");

    // 1. Each of the port's toys is one of the state's — the same place and the same clip — and
    //    its parts are the state's parts on the same nodes.
    let mut matched: Vec<Matched> = Vec::new();
    let mut taken = vec![false; toys.len()];
    let mut parts = 0;
    for (k, spec) in specs.iter().enumerate() {
        let what = |field: &str| format!("{track}: toy {k} ({}) {field}", animated.names[k]);
        let ours = motion_words(&animated.sim.motions[spec.motion]);
        let found = (0..toys.len()).find(|&i| {
            let model = ee.u32_at(toys[i] + at::MODEL);
            let clip = ee.u32_at(ee.u32_at(model + at::HANDLE) + 4);
            !taken[i]
                && mat(&ee, ee.u32_at(model + at::ROOT) + at::LOCAL) == bits(&spec.place)
                && clip_words(&ee, clip, &animated.sim.motions[spec.motion]).is_some_and(|w| w == ours)
        });
        let Some(i) = found else { panic!("{}", what("is no state toy: no place and clip are its")) };
        taken[i] = true;
        let model = ee.u32_at(toys[i] + at::MODEL);
        assert_eq!(ee.u32_at(model + at::FADING), 0, "{}", what("is fading between clips"));
        assert_eq!(ee.u32_at(model + at::MIRROR_MAP), 0, "{}", what("is mirrored"));
        let root = ee.u32_at(model + at::ROOT);
        let joints = ee.u32_at(model + at::JOINTS);
        let nodes: Vec<u32> = (0..ee.u32_at(model + at::JOINT_COUNT)).map(|j| ee.u32_at(joints + j * at::JOINT + at::NODE)).collect();
        assert_eq!(nodes.len(), spec.rig.parents.len(), "{}", what("joints"));
        let count = ee.u32_at(model + at::PART_COUNT) as usize;
        let list = ee.u32_at(model + at::PARTS);
        let theirs: Vec<u32> = (0..count as u32).map(|p| ee.u32_at(list + 4 * p)).collect();
        let mut bodies = Vec::new();
        if spec.mesh.is_some() {
            // A mesh `.col` stands at the placement: one part, on the root, never split.
            assert!(spec.parts.is_empty(), "{}", what("has a mesh and parts"));
            assert_eq!(count, 1, "{}", what("mesh's part count"));
            assert_eq!(ee.u32_at(theirs[0] + at::PART_NODE), root, "{}", what("mesh's node"));
        } else {
            assert_eq!(count, spec.parts.len(), "{}", what("part count"));
            for (p, (part, &their)) in spec.parts.iter().zip(&theirs).enumerate() {
                let node = part.joint.map_or(root, |j| nodes[j]);
                let theirs_node = ee.u32_at(their + at::PART_NODE);
                let on = nodes.iter().position(|&n| n == theirs_node);
                assert_eq!(node, theirs_node, "{}", what(&format!("part {p}'s node: ours joint {:?}, theirs {on:?}", part.joint)));
                let body = ee.u32_at(their + at::PART_BODY);
                assert_eq!(ee.u32_at(body + at::VTABLE), body_vtable, "{}", what(&format!("part {p}'s body's class")));
                assert_eq!(ee.u32_at(body + at::BODY_NODE), node, "{}", what(&format!("part {p}'s body's node")));
                bodies.push(body);
            }
            parts += spec.parts.len();
        }
        matched.push(Matched { model, nodes, bodies });
    }
    assert_eq!(parts, parts_want, "{track}: the hierarchy parts");

    // 2. The clocks and hints the state stands at.
    let mut sim: Toys = animated.sim.clone();
    for (k, m) in matched.iter().enumerate() {
        let spec = &sim.specs[k];
        let clip = ee.u32_at(ee.u32_at(m.model + at::HANDLE) + 4);
        let (l, n) = (ee.u32_at(clip + at::ROTATIONS), ee.u32_at(clip + at::ROTATION_COUNT));
        let base = sim.base[spec.motion];
        for c in 0..n {
            sim.hints[base + c as usize] = Hint(ee.u32_at(ee.u32_at(l + 4 * c) + at::HINT));
        }
        sim.clocks[k] = Clock {
            time: F(ee.f32_at(m.model + at::TIME)),
            prev: F(ee.f32_at(m.model + at::PREV)),
            finished: ee.u32_at(m.model + at::FINISHED) != 0,
        };
    }
    // The scene's step and its reciprocal: the body's virtuals read the original's, the port makes
    // its own with one division.
    let frame_dt = addr("frame_dt").unwrap();
    let dt = F(ee.f32_at(frame_dt));
    assert_eq!((F::ONE / dt).to_bits(), ee.u32_at(frame_dt + 4), "{track}: 1/dt is the app's");
    let mut world = Shelf(game_sim::world::Props::default());
    sim.attach(&mut world.0);
    let first = sim.first.expect("attached");

    let advance = addr("motion_advance").unwrap();
    let rebuild = addr("transform_rebuild").unwrap();
    let update = addr("kinematic_body_update").unwrap();
    let velocity = addr("path_car_velocity").unwrap();
    let spin = addr("path_car_spin").unwrap();
    let mut moved = 0;
    for tick in 0..ticks {
        // The original: each toy's clip and nodes, then its parts' bodies.
        for m in &matched {
            ee.call(advance, &[u64::from(m.model), 0, 0, 0], &[dt.0], BUDGET);
            for &n in &m.nodes {
                ee.call(rebuild, &[u64::from(n)], &[], BUDGET);
            }
            for &b in &m.bodies {
                ee.call(update, &[u64::from(b)], &[], BUDGET);
            }
        }
        sim.frame(dt, &mut world);
        let mut at_prop = first;
        for (k, m) in matched.iter().enumerate() {
            let spec = &sim.specs[k];
            if spec.mesh.is_some() {
                at_prop += 1;
            }
            for (p, &b) in m.bodies.iter().enumerate() {
                let what = |field: &str| format!("{track}: tick {tick}: toy {k} ({}) part {p} {field}", animated.names[k]);
                let ours = world.0.props[at_prop + p].moving.unwrap_or_else(|| panic!("{}", what("is not told")));
                assert_eq!(bits(&ours.pose), mat(&ee, b + at::BODY_NOW), "{}", what("pose"));
                let out = ee.scratch(0);
                ee.call(velocity, &[u64::from(out), u64::from(b)], &[], BUDGET);
                let v = [0, 4, 8].map(|o| ee.u32_at(out + o));
                assert_eq!(xyz(ours.velocity), v, "{}", what("velocity"));
                ee.call(spin, &[u64::from(out), u64::from(b)], &[], BUDGET);
                let s = [0, 4, 8].map(|o| ee.u32_at(out + o));
                assert_eq!(xyz(ours.spin), s, "{}", what("spin"));
                if ours.velocity.0[..3].iter().chain(&ours.spin.0[..3]).any(|v| *v != 0.0) {
                    moved += 1;
                }
            }
            at_prop += spec.parts.len();
        }
    }
    Some(moved)
}

#[test]
fn free10s_shovel_parts_ride_their_nodes() {
    // The windsock (a mesh) and the shovel (ten parts).
    let Some(moved) = parts_ride_their_nodes("free10", "s133-free10.ee.bin", 2, 10, FREE10_TICKS) else { return };
    assert!(moved > FREE10_TICKS, "the shovel's arm moves: {moved} part-ticks did");
}

#[test]
fn nat11s_gondola_parts_ride_their_nodes() {
    // Eight gondolas, two parts each, on their lift's clips.
    let Some(moved) = parts_ride_their_nodes("nat11", "s133-nat11.ee.bin", 8, 16, NAT11_TICKS) else { return };
    assert_eq!(moved, NAT11_TICKS * 16, "every gondola part moves on every tick");
}
