//! The animated toys against the original (V43): first_recording's save state of free09, its four
//! seti windmills and its rustic windmill, each on its clip.
//!
//! 1. **the tables** — the port's two turn tables are the ones the original built at startup, bit for
//!    bit;
//! 2. **the clips** — every toy, started from the state's (its clock, its clip's hints), advanced by
//!    the port and by the original's own `motion_advance` in the interpreter for [`TICKS`] ticks, is
//!    the original's on every tick: the clock, every rotation channel's hint, every joint's local
//!    matrix and, composed by the original's `transform_rebuild`, every joint's world matrix.

use std::collections::HashMap;

use game_sim::math::Mat4;
use game_sim::motion::Clip;
use game_sim::toys::{
    Clock, Hint, Motion, NarrowKey, PosChannel, PosKey, Rig, RotChannel, RotKey, RotKeys, Spec, Toys, Turns,
};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

const STATE: &str = "first_recording.p2m2_SaveState.eeMemory.bin";
const BUDGET: u64 = 50_000_000;
/// Ten seconds: seven and a half turns of the seti windmills' clip, five of the rustic one's.
const TICKS: usize = 300;
/// The scene's step, a thirtieth of a second.
const DT: F = F(1.0 / 30.0);

/// The state's animated toys (the class's vtable at `+8`): four seti windmills, then the rustic one.
const TOYS: [u32; 5] = [0x0127_b300, 0x0128_96b0, 0x0128_9e50, 0x0128_ed60, 0x0128_fa80];

/// The fields (`research/oracle.txt` V43).
mod at {
    /// The toy's model: its motion object.
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
    /// The transform node's local and world matrices, and its parent.
    pub const LOCAL: u32 = 0x40;
    pub const WORLD: u32 = 0x80;
    pub const PARENT: u32 = 0x100;
    /// The clip: its channel lists and counts, flags, duration.
    pub const POSITIONS: u32 = 0x18;
    pub const ROTATIONS: u32 = 0x1c;
    pub const POSITION_COUNT: u32 = 0x20;
    pub const ROTATION_COUNT: u32 = 0x24;
    pub const FLAGS: u32 = 0x28;
    pub const DURATION: u32 = 0x3c;
    /// A channel: its name index, on flag, key count, class, keys and hint.
    pub const ON: u32 = 0x4;
    pub const COUNT: u32 = 0x8;
    pub const CLASS: u32 = 0x10;
    pub const KEYS: u32 = 0x14;
    pub const HINT: u32 = 0x18;
    /// The channel class's sampler slot.
    pub const SAMPLER: u32 = 0x10;
}

fn state() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: `./kit state ram` of the human's first recording", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

fn vf(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

fn mat(ee: &Ee, a: u32) -> Mat4 {
    Mat4([vf(ee, a), vf(ee, a + 16), vf(ee, a + 32), vf(ee, a + 48)])
}

fn mat_bits(m: &Mat4) -> [[u32; 4]; 4] {
    m.0.map(Vf::to_bits)
}

fn f(ee: &Ee, a: u32) -> F {
    F(ee.f32_at(a))
}

fn i16_at(ee: &Ee, a: u32) -> i16 {
    (u16::from(ee.u8_at(a)) | u16::from(ee.u8_at(a + 1)) << 8) as i16
}

/// A clip out of the state, and the address of each of its rotation channels.
fn motion_of(ee: &Ee, clip: u32) -> (Motion, Vec<u32>) {
    let narrow = addr("motion_rot_sample_narrow").unwrap();
    let list = |at_list, at_count| -> Vec<u32> {
        let (l, n) = (ee.u32_at(clip + at_list), ee.u32_at(clip + at_count));
        (0..n).map(|i| ee.u32_at(l + 4 * i)).collect()
    };
    let rot = list(at::ROTATIONS, at::ROTATION_COUNT);
    let rotation = rot
        .iter()
        .map(|&ch| {
            let (n, keys) = (ee.u32_at(ch + at::COUNT), ee.u32_at(ch + at::KEYS));
            let keys = if ee.u32_at(ee.u32_at(ch + at::CLASS) + at::SAMPLER) == narrow {
                RotKeys::Narrow(
                    (0..n)
                        .map(|i| keys + 8 * i)
                        .map(|k| NarrowKey {
                            t: f(ee, k),
                            turn: ee.u8_at(k + 4),
                            axis: [5, 6, 7].map(|o| ee.u8_at(k + o) as i8),
                        })
                        .collect(),
                )
            } else {
                RotKeys::Wide(
                    (0..n)
                        .map(|i| keys + 12 * i)
                        .map(|k| RotKey {
                            t: f(ee, k),
                            turn: i16_at(ee, k + 4) as u16,
                            axis: [6, 8, 10].map(|o| i16_at(ee, k + o)),
                        })
                        .collect(),
                )
            };
            RotChannel { name: ee.u32_at(ch), on: ee.u32_at(ch + at::ON) != 0, keys }
        })
        .collect();
    let position = list(at::POSITIONS, at::POSITION_COUNT)
        .iter()
        .map(|&ch| {
            let (n, keys) = (ee.u32_at(ch + at::COUNT), ee.u32_at(ch + at::KEYS));
            let keys = (0..n)
                .map(|i| keys + 16 * i)
                .map(|k| PosKey { t: f(ee, k), p: [4, 8, 12].map(|o| f(ee, k + o)) })
                .collect();
            PosChannel { name: ee.u32_at(ch), on: ee.u32_at(ch + at::ON) != 0, keys }
        })
        .collect();
    let timing = Clip::new(f(ee, clip + at::DURATION), ee.u32_at(clip + at::FLAGS) & 1 != 0);
    (Motion { timing, rotation, position }, rot)
}

/// A toy's joints' transform nodes.
fn nodes(ee: &Ee, model: u32) -> Vec<u32> {
    let joints = ee.u32_at(model + at::JOINTS);
    (0..ee.u32_at(model + at::JOINT_COUNT)).map(|j| ee.u32_at(joints + j * at::JOINT + at::NODE)).collect()
}

/// A toy's skeleton and placement out of the state.
fn spec_of(ee: &Ee, model: u32, motion: usize, names: u32) -> Spec {
    let nodes = nodes(ee, model);
    let root = ee.u32_at(model + at::ROOT);
    let parents = nodes
        .iter()
        .map(|&n| {
            let p = ee.u32_at(n + at::PARENT);
            if p == root {
                None
            } else {
                Some(nodes.iter().position(|&q| q == p).unwrap_or_else(|| panic!("node {n:#x}'s parent {p:#x}")))
            }
        })
        .collect();
    let map_at = ee.u32_at(ee.u32_at(model + at::HANDLE));
    let rig = Rig {
        parents,
        rest: nodes.iter().map(|&n| mat(ee, n + at::LOCAL)).collect(),
        map: (0..names).map(|i| ee.u8_at(map_at + i)).collect(),
    };
    assert_eq!(ee.u32_at(root + at::PARENT), 0, "the root node {root:#x} has a parent");
    Spec { motion, rig, place: mat(ee, root + at::LOCAL), start: F::ZERO, mesh: None, parts: Vec::new() }
}

#[test]
fn the_turn_tables_are_the_originals() {
    let Some(ee) = state() else { return };
    let turns = Turns::get();
    for (name, table) in [("motion_turn_table", &turns.wide), ("motion_turn_table_narrow", &turns.narrow)] {
        let at = addr(name).unwrap();
        for (n, [sin, cos]) in table.iter().enumerate() {
            let a = at + 8 * n as u32;
            assert_eq!(sin.to_bits(), ee.u32_at(a), "{name} {n} sine");
            assert_eq!(cos.to_bits(), ee.u32_at(a + 4), "{name} {n} cosine");
        }
    }
}

#[test]
fn the_toys_turn_as_the_originals_do() {
    let Some(mut ee) = state() else { return };
    let advance = addr("motion_advance").unwrap();
    let rebuild = addr("transform_rebuild").unwrap();

    // The clips, each once, in the order the toys first use them, and each toy's.
    let mut clips: HashMap<u32, usize> = HashMap::new();
    let mut motions = Vec::new();
    let mut channels: Vec<Vec<u32>> = Vec::new();
    let mut specs = Vec::new();
    for &toy in &TOYS {
        let model = ee.u32_at(toy + at::MODEL);
        assert_eq!(ee.u32_at(model + at::FADING), 0, "{toy:#x} is fading between clips");
        assert_eq!(ee.u32_at(model + at::MIRROR_MAP), 0, "{toy:#x} is mirrored");
        let clip = ee.u32_at(ee.u32_at(model + at::HANDLE) + 4);
        let k = *clips.entry(clip).or_insert_with(|| {
            let (m, rot) = motion_of(&ee, clip);
            motions.push(m);
            channels.push(rot);
            motions.len() - 1
        });
        let m = &motions[k];
        let names = m.rotation.iter().map(|c| c.name).chain(m.position.iter().map(|c| c.name)).max().map_or(0, |n| n + 1);
        specs.push(spec_of(&ee, model, k, names));
    }
    assert_eq!(motions.len(), 3, "the seti windmills' two clips and the rustic one's");
    let mut toys = Toys::new(motions, specs);
    let hints = |ee: &Ee| -> Vec<u32> { channels.iter().flatten().map(|&ch| ee.u32_at(ch + at::HINT)).collect() };
    toys.hints = hints(&ee).into_iter().map(Hint).collect();
    toys.clocks = TOYS
        .iter()
        .map(|&toy| {
            let model = ee.u32_at(toy + at::MODEL);
            Clock { time: f(&ee, model + at::TIME), prev: f(&ee, model + at::PREV), finished: ee.u32_at(model + at::FINISHED) != 0 }
        })
        .collect();

    for tick in 0..TICKS {
        for (k, &toy) in TOYS.iter().enumerate() {
            let model = ee.u32_at(toy + at::MODEL);
            ee.call(advance, &[u64::from(model), 0, 0, 0], &[DT.0], BUDGET);
            let nodes = nodes(&ee, model);
            for &n in &nodes {
                ee.call(rebuild, &[u64::from(n)], &[], BUDGET);
            }
            let what = |field: &str| format!("tick {tick}: toy {k} ({toy:#x}) {field}");
            let locals = toys.advance(k, DT).expect("a looping clip");
            let worlds = toys.worlds(k, &locals);
            let c = toys.clocks[k];
            assert_eq!(c.time.to_bits(), ee.u32_at(model + at::TIME), "{}", what("clock"));
            assert_eq!(c.prev.to_bits(), ee.u32_at(model + at::PREV), "{}", what("previous clock"));
            assert_eq!(c.finished, ee.u32_at(model + at::FINISHED) != 0, "{}", what("finished"));
            for (j, &n) in nodes.iter().enumerate() {
                assert_eq!(mat_bits(&locals[j]), mat_bits(&mat(&ee, n + at::LOCAL)), "{}", what(&format!("joint {j} local")));
                assert_eq!(mat_bits(&worlds[j]), mat_bits(&mat(&ee, n + at::WORLD)), "{}", what(&format!("joint {j} world")));
            }
        }
        let ours: Vec<u32> = toys.hints.iter().map(|h| h.0).collect();
        assert_eq!(ours, hints(&ee), "tick {tick}: the clips' hints");
    }
}
