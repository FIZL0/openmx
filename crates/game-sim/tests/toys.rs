//! The animated toys against the original (V43): first_recording's save state of free09, its four
//! seti windmills and its rustic windmill, each on its clip.
//!
//! 1. **the tables** — the port's two turn tables are the ones the original built at startup, bit for
//!    bit;
//! 2. **the clips** — every toy, started from the state's (its clock, its clip's hints), advanced by
//!    the port and by the original's own `motion_advance` in the interpreter for [`TICKS`] ticks, is
//!    the original's on every tick: the clock, every rotation channel's hint, every joint's local
//!    matrix and, composed by the original's `transform_rebuild`, every joint's world matrix;
//! 3. **the visibility gate** (V76) — the original's own toy update, run whole in the interpreter on
//!    each toy with `AnimateWhenNotVisible` on and off, its root node's search stamp moved or not and
//!    its in-view bit set or not, steps exactly the toys [`Toys::frame`] steps when the shell says a
//!    toy was drawn (stamp moved and in view), each clock bit for bit;
//! 4. **the windmill's body** (S167) — the rustic windmill's collision body, never updated in the
//!    state, answers its motion through its own two virtuals as the port's mesh is told it, bit for
//!    bit, before and after the toy's first and second steps.

use std::collections::HashMap;

use game_sim::math::Mat4;
use game_sim::motion::Clip;
use game_sim::toys::{
    Bodies, Clock, Hint, Motion, NarrowKey, PosChannel, PosKey, Rig, RotChannel, RotKey, RotKeys, Spec, Toys, Turns,
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
    Spec { motion, rig, place: mat(ee, root + at::LOCAL), start: F::ZERO, mesh: None, parts: Vec::new(), unseen: true, search: true }
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

/// S145: a toy's collision parts carry their joints' motion. Every toy of first_recording's state
/// on its clip, with two empty parts each — one on the root (the placement), one on the last joint —
/// put among a world's props and stepped by [`Toys::frame`]: each part's mesh is told the
/// [`game_sim::traffic::Moving`] of its joint's world matrix from the tick before to this one, the
/// path car's arithmetic (the kinematic body's two virtuals are the car's), and a part on the root
/// stands still. The windmills turn, so the last joint's spins.
#[test]
fn a_toys_collision_parts_carry_their_joints_motion() {
    use game_sim::toys::Part;
    use game_sim::traffic::Moving;
    let Some(ee) = state() else { return };
    let mut clips: HashMap<u32, usize> = HashMap::new();
    let mut motions = Vec::new();
    let mut channels: Vec<Vec<u32>> = Vec::new();
    let mut specs = Vec::new();
    let empty = game_sim::track::Mesh {
        version: 1,
        surface: None,
        transform: [[F::ONE, F::ZERO, F::ZERO], [F::ZERO, F::ONE, F::ZERO], [F::ZERO, F::ZERO, F::ONE], [F::ZERO; 3]],
        vertices: game_sim::track::Points::Wide(Vec::new()),
        normals: None,
        nodes: Vec::new(),
        triangles: Vec::new(),
    };
    for &toy in &TOYS {
        let model = ee.u32_at(toy + at::MODEL);
        let clip = ee.u32_at(ee.u32_at(model + at::HANDLE) + 4);
        let k = *clips.entry(clip).or_insert_with(|| {
            let (m, rot) = motion_of(&ee, clip);
            motions.push(m);
            channels.push(rot);
            motions.len() - 1
        });
        let m = &motions[k];
        let names = m.rotation.iter().map(|c| c.name).chain(m.position.iter().map(|c| c.name)).max().map_or(0, |n| n + 1);
        let mut spec = spec_of(&ee, model, k, names);
        let last = spec.rig.parents.len() - 1;
        spec.parts = vec![Part { joint: None, mesh: empty.clone() }, Part { joint: Some(last), mesh: empty.clone() }];
        specs.push(spec);
    }
    let mut toys = Toys::new(motions, specs);
    toys.hints = channels.iter().flatten().map(|&ch| Hint(ee.u32_at(ch + at::HINT))).collect();
    toys.clocks = TOYS
        .iter()
        .map(|&toy| {
            let model = ee.u32_at(toy + at::MODEL);
            Clock { time: f(&ee, model + at::TIME), prev: f(&ee, model + at::PREV), finished: ee.u32_at(model + at::FINISHED) != 0 }
        })
        .collect();
    // Bodies a toy has stepped since the load (S167): the matrix before is the joint's, not the
    // constructor's identity.
    toys.bodies = vec![Bodies(2); TOYS.len()];
    let mut world = Shelf(game_sim::world::Props::default());
    toys.attach(&mut world.0);
    assert_eq!(world.0.props.len(), 2 * TOYS.len(), "two parts a toy");
    let inv_dt = F::ONE / DT;
    let mut spun = 0;
    for tick in 0..30 {
        // What the step will make of each toy, on a copy: the joint's world matrix either side.
        let mut copy = toys.clone();
        let before: Vec<Mat4> = (0..TOYS.len()).map(|k| {
            let mut hints = toys.hints.clone();
            let base = toys.base[toys.specs[k].motion];
            let locals = game_sim::toys::pose(&toys.motions[toys.specs[k].motion], &toys.specs[k].rig, toys.clocks[k].time, &mut hints[base..]);
            *toys.worlds(k, &locals).last().expect("joints")
        }).collect();
        let after: Vec<Mat4> = (0..TOYS.len()).map(|k| {
            let locals = copy.advance(k, DT).expect("a looping clip");
            *copy.worlds(k, &locals).last().expect("joints")
        }).collect();
        toys.frame(DT, &mut world);
        for k in 0..TOYS.len() {
            let what = |field: &str| format!("tick {tick}: toy {k} {field}");
            let root = world.0.props[2 * k].moving.expect("the root part is told");
            let place = toys.specs[k].place;
            let still = Moving::of(&place, &place, inv_dt);
            assert_eq!((root.velocity.0.map(f32::to_bits), root.spin.0.map(f32::to_bits)), (still.velocity.0.map(f32::to_bits), still.spin.0.map(f32::to_bits)), "{}", what("root part's motion"));
            assert!(root.velocity.0[..3].iter().chain(&root.spin.0[..3]).all(|v| *v == 0.0), "{}", what("root part moves"));
            let tip = world.0.props[2 * k + 1].moving.expect("the joint's part is told");
            let want = Moving::of(&before[k], &after[k], inv_dt);
            assert_eq!(mat_bits(&tip.pose), mat_bits(&want.pose), "{}", what("joint part's pose"));
            assert_eq!(tip.velocity.0.map(f32::to_bits), want.velocity.0.map(f32::to_bits), "{}", what("joint part's velocity"));
            assert_eq!(tip.spin.0.map(f32::to_bits), want.spin.0.map(f32::to_bits), "{}", what("joint part's spin"));
            if tip.spin.0[..3].iter().any(|v| *v != 0.0) {
                spun += 1;
            }
        }
    }
    assert_eq!(spun, 30 * TOYS.len(), "every toy's last joint turns on every tick");
}

/// The fields the toy update's visibility gate reads (`research/oracle.txt` V76).
mod gate {
    /// The toy's `AnimateWhenNotVisible`.
    pub const UNSEEN: u32 = 0x10c;
    /// The toy's copy of its root node's search stamp, taken on each step.
    pub const STAMP: u32 = 0x144;
    /// The root node's flags byte whose bit 0 is the draw's last frustum test.
    pub const IN_VIEW: u32 = 0x111;
}

/// V76: the update's visibility gate. For each toy of the state in turn and each of the eight ways
/// of its three inputs — `AnimateWhenNotVisible` on or off, its root node's search stamp moved since
/// the toy's last step or not, the node's in-view bit set or not — the original's whole update
/// (`animated_toy_update`, dt in `f12`) is run on that toy alone, and the port's [`Toys::frame`]
/// with that toy's [`Spec::unseen`](game_sim::toys::Spec) set the same and the others' off and
/// unseen, its [`Toys::seen`] the stamp having moved *and* the bit set. Every toy's clock is the
/// original's after each: the gated toy's untouched, the stepped one's advanced, bit for bit. A
/// stepped toy takes its node's stamp, so the frame after one that did not reach it is gated again.
#[test]
fn the_visibility_gate_is_the_originals() {
    let Some(mut ee) = state() else { return };
    let update = addr("animated_toy_update").unwrap();
    let mut clips: HashMap<u32, usize> = HashMap::new();
    let mut motions = Vec::new();
    let mut channels: Vec<Vec<u32>> = Vec::new();
    let mut specs = Vec::new();
    for &toy in &TOYS {
        let model = ee.u32_at(toy + at::MODEL);
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
        // free09.toy writes it off for all five, and nothing has set it since the load.
        assert_eq!(ee.u32_at(toy + gate::UNSEEN), 0, "{toy:#x}'s AnimateWhenNotVisible");
    }
    let mut toys = Toys::new(motions, specs);
    toys.hints = channels.iter().flatten().map(|&ch| Hint(ee.u32_at(ch + at::HINT))).collect();
    let clocks = |ee: &Ee| -> Vec<Clock> {
        TOYS.iter()
            .map(|&toy| {
                let model = ee.u32_at(toy + at::MODEL);
                Clock { time: f(ee, model + at::TIME), prev: f(ee, model + at::PREV), finished: ee.u32_at(model + at::FINISHED) != 0 }
            })
            .collect()
    };
    toys.clocks = clocks(&ee);
    let mut world = Shelf(game_sim::world::Props::default());
    let (mut stepped, mut gated) = (0, 0);
    for (k, &toy) in TOYS.iter().enumerate() {
        let root = ee.u32_at(ee.u32_at(toy + at::MODEL) + at::ROOT);
        for case in 0..8u32 {
            let (unseen, moved, in_view) = (case & 1 != 0, case & 2 != 0, case & 4 != 0);
            let what = format!("toy {k} ({toy:#x}), AnimateWhenNotVisible {unseen}, stamp moved {moved}, in view {in_view}");
            ee.set_u32(toy + gate::UNSEEN, u32::from(unseen));
            let stamp = ee.u32_at(root) & 0xffff;
            ee.write(toy + gate::STAMP, 2, u128::from(if moved { stamp ^ 1 } else { stamp }));
            let flags = ee.u8_at(root + gate::IN_VIEW) & !1;
            ee.write(root + gate::IN_VIEW, 1, u128::from(flags | u8::from(in_view)));
            let was = clocks(&ee);
            ee.call(update, &[u64::from(toy)], &[DT.0], BUDGET);
            let now = clocks(&ee);

            let specs = std::sync::Arc::make_mut(&mut toys.specs);
            for (j, spec) in specs.iter_mut().enumerate() {
                spec.unseen = j == k && unseen;
            }
            toys.seen = (0..TOYS.len()).map(|j| j == k && moved && in_view).collect();
            toys.frame(DT, &mut world);
            for (j, (ours, theirs)) in toys.clocks.iter().zip(&now).enumerate() {
                assert_eq!(
                    (ours.time.to_bits(), ours.prev.to_bits(), ours.finished),
                    (theirs.time.to_bits(), theirs.prev.to_bits(), theirs.finished),
                    "{what}: toy {j}'s clock"
                );
            }
            if now[k] == was[k] {
                gated += 1;
                assert!(!unseen && !(moved && in_view), "{what}: the original passed it over");
            } else {
                stepped += 1;
                assert_eq!(ee.u32_at(toy + gate::STAMP) & 0xffff, stamp, "{what}: the step takes the node's stamp");
            }
        }
        // Put the toy back as the state had it.
        ee.set_u32(toy + gate::UNSEEN, 0);
    }
    assert_eq!((stepped, gated), (5 * TOYS.len(), 3 * TOYS.len()), "five of the eight ways step a toy");
}

/// The kinematic body's fields (`research/oracle.txt` S145).
mod body {
    /// The body's node: the rustic windmill's static root.
    pub const NODE: u32 = 0x254;
    /// Its update's two matrices: the node's world at its last update, and at the one before.
    pub const NOW: u32 = 0x2a0;
    pub const BEFORE: u32 = 0x2e0;
    /// The vehicle virtuals a contact asks: velocity and spin (`a0` out, `a1` the body).
    pub const VELOCITY_SLOT: u32 = 0xb8;
    pub const SPIN_SLOT: u32 = 0xbc;
    /// The application object's `1/dt`, which the velocity virtual scales by.
    pub const INV_DT: u32 = 0x348;
}

/// S167: **the rustic windmill's collision answers the original's motion, bogus or not.** In
/// first_recording's state free09's one kinematic body is the rustic windmill's collision part, on
/// the toy's static root, and its two matrices are still the constructor's identity: the toy has
/// not been in view since the load. Over a run of ticks the windmill is drawn or not; the original's
/// own toy update runs in the interpreter with its gate set to match, then the body's velocity and
/// spin virtuals are asked, as a contact on the part asks them. The port's [`Toys::frame`] with the
/// toy's [`Toys::seen`] the same tells the windmill's mesh the same [`game_sim::traffic::Moving`],
/// bit for bit: the node's position times `1/dt` and the turn from the identity to the placement
/// until the toy has stepped twice, then nothing. A live capture (`research/s167_windmill.py`) shows
/// the original flinging a bike that lands on it.
#[test]
fn the_windmills_body_answers_the_originals_motion() {
    let Some(mut ee) = state() else { return };
    let update = addr("animated_toy_update").unwrap();
    let vtable = addr("kinematic_body_vtable").unwrap();
    let bodies: Vec<u32> = (0..32u32 << 20).step_by(4).filter(|&a| ee.u32_at(a) == vtable).map(|a| a - 8).collect();
    assert_eq!(bodies.len(), 1, "free09's kinematic bodies");
    let windmill = TOYS[4];
    let model = ee.u32_at(windmill + at::MODEL);
    let root = ee.u32_at(model + at::ROOT);
    let b = bodies[0];
    assert_eq!(ee.u32_at(b + body::NODE), root, "the body is on the rustic windmill's root");
    for m in [body::NOW, body::BEFORE] {
        assert_eq!(mat_bits(&mat(&ee, b + m)), mat_bits(&Mat4::IDENTITY), "the state's body +{m:#x}: never updated");
    }
    let mut clips: HashMap<u32, usize> = HashMap::new();
    let mut motions = Vec::new();
    let mut channels: Vec<Vec<u32>> = Vec::new();
    let mut specs = Vec::new();
    for &toy in &TOYS {
        let model = ee.u32_at(toy + at::MODEL);
        let clip = ee.u32_at(ee.u32_at(model + at::HANDLE) + 4);
        let k = *clips.entry(clip).or_insert_with(|| {
            let (m, rot) = motion_of(&ee, clip);
            motions.push(m);
            channels.push(rot);
            motions.len() - 1
        });
        let m = &motions[k];
        let names = m.rotation.iter().map(|c| c.name).chain(m.position.iter().map(|c| c.name)).max().map_or(0, |n| n + 1);
        let mut spec = spec_of(&ee, model, k, names);
        spec.unseen = false;
        if toy == windmill {
            assert_eq!(mat_bits(&mat(&ee, root + at::WORLD)), mat_bits(&spec.place), "the root's world is its placement");
            spec.mesh = Some(game_sim::track::Mesh {
                version: 1,
                surface: None,
                transform: [[F::ONE, F::ZERO, F::ZERO], [F::ZERO, F::ONE, F::ZERO], [F::ZERO, F::ZERO, F::ONE], [F::ZERO; 3]],
                vertices: game_sim::track::Points::Wide(Vec::new()),
                normals: None,
                nodes: Vec::new(),
                triangles: Vec::new(),
            });
        }
        specs.push(spec);
    }
    let mut toys = Toys::new(motions, specs);
    toys.hints = channels.iter().flatten().map(|&ch| Hint(ee.u32_at(ch + at::HINT))).collect();
    toys.clocks = TOYS
        .iter()
        .map(|&toy| {
            let model = ee.u32_at(toy + at::MODEL);
            Clock { time: f(&ee, model + at::TIME), prev: f(&ee, model + at::PREV), finished: ee.u32_at(model + at::FINISHED) != 0 }
        })
        .collect();
    let mut world = Shelf(game_sim::world::Props::default());
    toys.attach(&mut world.0);
    assert_eq!(world.0.props.len(), 1, "the rustic windmill's mesh alone");
    let inv_dt = F::ONE / DT;
    let app = ee.u32_at(addr("app_pointer").unwrap());
    ee.set_f32(app + body::INV_DT, inv_dt.0);
    let out = ee.scratch(0);
    let ask = |ee: &mut Ee, slot: u32| -> Vf {
        let f = ee.u32_at(ee.u32_at(b + 8) + slot);
        ee.call(f, &[u64::from(out), u64::from(b)], &[], BUDGET);
        vf(ee, out)
    };
    let lanes = |v: Vf| [v.0[0].to_bits(), v.0[1].to_bits(), v.0[2].to_bits()];
    // Drawn or not, tick by tick: twice unseen, the first step, unseen, the second, and on.
    let drawn = [false, false, true, false, false, true, false, true, true, false];
    let (mut flung, mut still) = (0, 0);
    for (tick, &seen) in drawn.iter().enumerate() {
        let what = format!("tick {tick} (drawn {seen})");
        let stamp = ee.u32_at(root) & 0xffff;
        ee.write(windmill + gate::STAMP, 2, u128::from(if seen { stamp ^ 1 } else { stamp }));
        let flags = ee.u8_at(root + gate::IN_VIEW) & !1;
        ee.write(root + gate::IN_VIEW, 1, u128::from(flags | u8::from(seen)));
        ee.call(update, &[u64::from(windmill)], &[DT.0], BUDGET);
        let (velocity, spin) = (ask(&mut ee, body::VELOCITY_SLOT), ask(&mut ee, body::SPIN_SLOT));

        toys.seen = TOYS.iter().map(|&t| t == windmill && seen).collect();
        toys.frame(DT, &mut world);
        let ours = world.0.props[0].moving.expect("the windmill's mesh is a mover");
        println!("{what}: velocity {:?} / {:?}, spin {:?} / {:?}", velocity.0, ours.velocity.0, spin.0, ours.spin.0);
        assert_eq!(lanes(ours.velocity), lanes(velocity), "{what}: the velocity");
        assert_eq!(lanes(ours.spin), lanes(spin), "{what}: the spin");
        if velocity.0[..3].iter().all(|v| *v == 0.0) && spin.0[..3].iter().all(|v| *v == 0.0) {
            still += 1;
        } else {
            assert!(velocity.0[2] > 100_000.0, "{what}: the bogus velocity is the root's position x 30");
            flung += 1;
        }
    }
    assert_eq!((flung, still), (5, 5), "bogus until the second step, still after");
}
