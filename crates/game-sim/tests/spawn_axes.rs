//! S205 — how the original squares a spawn's nose and up, proved against it.
//!
//! A plane is put on its spawn through the physics object's place (`physics_object_place` in
//! `research/oracle.txt`), handed the spawn's position, its nose and its up — a track bay's up being
//! the ground query's normal under it. Before it touches the body it squares the two in place (the
//! side is the nose crossed with the up, the up the side crossed with the nose, both normalised by
//! VU0's reciprocal square root) and turns them into the quaternion it places the body with
//! (`axes_to_quat`). That prologue is a pure function of the three quadwords, so it is proved the
//! cheap way: the original's own instructions in `ps2-oracle`'s interpreter, over 10,000 generated
//! inputs, against [`flight::spawn_axes`] and [`flight::spawn_turn`] bit for bit. The run is stopped
//! just after the quaternion call returns, before the place reaches the body (an object the
//! interpreter does not have).
//!
//! The inputs lean on what a spawn is handed: the disc's `Direction`s (level and not), the ground's
//! normals (near up, leaning up to 60 degrees, and the flat ground's `0.99999994`), and a share of
//! arbitrary vectors, zero and huge ones included, so every lane of the arithmetic is reached.

use game_sim::flight;
use ps2_float::Vf;
use ps2_oracle::{DONE, Ee, addr, game_or_skip};

/// How many inputs the proof runs: the project's floor for an oracle proof.
const N: usize = 10_000;

/// Where the inputs and the quaternion go: past the executable and well away from the stack.
const SCRATCH: u32 = 0x0120_0000;

/// The place's instruction after its `axes_to_quat` call and that call's delay slot: the quaternion
/// is on its stack, and the next thing it does is reach for the body.
const AFTER_TURN: u32 = 0xb4;

/// A small generator, so the run is the same every time.
struct Gen(u64);

impl Gen {
    fn bits(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.bits() as f32 / (1u64 << 31) as f32)
    }
    /// A nose: a `machine.txt` `Direction` (written to three places, not normalised), a level one of
    /// any length, or any vector at all.
    fn facing(&mut self) -> [f32; 4] {
        const FILE: [[f32; 3]; 6] =
            [[0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [-0.296, -0.042, 0.954], [-1.0, 0.0, -0.5], [-0.991, -0.08, -0.11]];
        match self.bits() % 10 {
            0..=2 => {
                let f = FILE[self.bits() as usize % FILE.len()];
                [f[0], f[1], f[2], 0.0]
            }
            3..=6 => [self.range(-30.0, 30.0), 0.0, self.range(-30.0, 30.0), 0.0],
            7 => [0.0, 0.0, 0.0, 0.0],
            8 => [f32::MAX, f32::MAX, -f32::MAX, 0.0],
            _ => [self.range(-30.0, 30.0), self.range(-30.0, 30.0), self.range(-30.0, 30.0), self.range(-1.0, 1.0)],
        }
    }
    /// An up: the ground query's normal (unit, `w` 1 or a hair under, as the terrain answers), the
    /// flat ground's, the world's, or any vector.
    fn up(&mut self) -> [f32; 4] {
        match self.bits() % 10 {
            0 => [0.0, 0.99999994, 0.0, 1.0],
            1 => [0.0, 1.0, 0.0, 0.0],
            2..=7 => {
                let (x, z) = (self.range(-0.6, 0.6), self.range(-0.6, 0.6));
                let y = self.range(0.5, 1.0);
                let l = (x * x + y * y + z * z).sqrt();
                [x / l, y / l, z / l, if self.bits() % 2 == 0 { 1.0 } else { 0.9999999 }]
            }
            8 => [0.0, 0.0, 0.0, 0.0],
            _ => [self.range(-30.0, 30.0), self.range(-30.0, 30.0), self.range(-30.0, 30.0), self.range(-1.0, 1.0)],
        }
    }
}

fn put(ee: &mut Ee, at: u32, v: [f32; 4]) {
    for (i, x) in v.iter().enumerate() {
        ee.set_u32(at + 4 * i as u32, x.to_bits());
    }
}

fn get(ee: &Ee, at: u32) -> [u32; 4] {
    core::array::from_fn(|i| ee.u32_at(at + 4 * i as u32))
}

/// The nose and the up squared as the original's place squares them, and the quaternion it places
/// the body with, bit for bit — the lanes it writes (`xyz`) and the `w`s it leaves.
#[test]
fn a_spawn_is_squared_as_the_originals_place_squares_it() {
    let Some(mut ee) = game_or_skip() else { return };
    let place = addr("physics_object_place").unwrap();
    let (pos, look, up, quat) = (SCRATCH, SCRATCH + 0x10, SCRATCH + 0x20, SCRATCH + 0x30);
    // Copy the quaternion off the place's stack and end the call there.
    ee.hooks.insert(
        place + AFTER_TURN,
        Box::new(move |ee: &mut Ee| {
            let sp = ee.r(29) as u32;
            let q = ee.read(sp + 128, 16);
            ee.write(quat, 16, q);
            ee.set(31, DONE as i32 as i64 as u64);
        }),
    );
    let mut g = Gen(0x5205_0001);
    let mut wrong = 0;
    for i in 0..N {
        let (f, u) = (g.facing(), g.up());
        put(&mut ee, pos, [1.0, 2.0, 3.0, 0.0]);
        put(&mut ee, look, f);
        put(&mut ee, up, u);
        ee.call(place, &[0, pos as u64, look as u64, up as u64], &[], 10_000);
        let (ours_f, ours_u) = flight::spawn_axes(Vf(f), Vf(u));
        let ours_q = flight::spawn_turn(Vf(f), Vf(u));
        let theirs = (get(&ee, look), get(&ee, up), get(&ee, quat));
        let ours = (ours_f.0.map(f32::to_bits), ours_u.0.map(f32::to_bits), ours_q.0.map(f32::to_bits));
        if theirs != ours {
            wrong += 1;
            eprintln!("{i}: nose {f:?} up {u:?}\n  the game {theirs:x?}\n  the port {ours:x?}");
            assert!(wrong < 6, "stopping after six");
        }
    }
    assert_eq!(wrong, 0, "{wrong} of {N} spawns are squared otherwise");
}

/// What the squaring does to a bay on a slope: the nose stays the file's `Direction`, the wings take
/// the slope across it, and a slope along the nose is taken off — the plane never pitches with the
/// ground. On flat ground (the disc's every biplane bay, normal `0.99999994`) it is the level spawn.
#[test]
fn a_sloped_bay_rolls_the_wings_and_keeps_the_nose() {
    let level = flight::spawn_axes(Vf::new(0.0, 0.0, -1.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0));
    let flat = flight::spawn_axes(Vf::new(0.0, 0.0, -1.0, 0.0), Vf::new(0.0, 0.99999994, 0.0, 1.0));
    assert_eq!(flat.0.xyz_bits(), level.0.xyz_bits());
    assert_eq!(flat.1.xyz_bits(), level.1.xyz_bits());

    // 30 degrees across the nose (the ground falls away to +x): the up leans with it.
    let (s, c) = (0.5f32, 0.8660254f32);
    let (nose, up) = flight::spawn_axes(Vf::new(0.0, 0.0, -1.0, 0.0), Vf::new(s, c, 0.0, 1.0));
    assert_eq!((nose.x(), nose.y(), nose.z()), (0.0, 0.0, -1.0), "the nose moved");
    assert!((up.x() - s).abs() < 1e-6 && (up.y() - c).abs() < 1e-6 && up.z().abs() < 1e-6, "across: {up:?}");

    // 30 degrees along it (uphill ahead): the up stands straight.
    let (nose, up) = flight::spawn_axes(Vf::new(0.0, 0.0, -1.0, 0.0), Vf::new(0.0, c, s, 1.0));
    assert_eq!((nose.x(), nose.y(), nose.z()), (0.0, 0.0, -1.0), "the nose moved");
    assert!(up.x().abs() < 1e-6 && (up.y() - 1.0).abs() < 1e-6 && up.z().abs() < 1e-6, "along: {up:?}");
}

trait Xyz {
    fn xyz_bits(&self) -> [u32; 3];
}

impl Xyz for Vf {
    fn xyz_bits(&self) -> [u32; 3] {
        [self.x().to_bits(), self.y().to_bits(), self.z().to_bits()]
    }
}
