//! S128: the path cars' ground switches against the original, on recording2's save state of free09
//! and its six trailers (the cabs and their trailers A, C and D), all `PresampleTerrain`.
//!
//! - [`the_trailers_keys_are_presampled_as_the_originals`]: the shell's specs, built from free09's
//!   `.toy` and clips, are the state's cars; their arc lengths, measured on the clip's own keys, are
//!   the state's bit for bit, and presampled on the port's ground every key's position and normal
//!   is the state's;
//! - [`the_trailers_drive_from_their_load_as_the_originals_do`]: every car put back as its load
//!   leaves it (the clip's keys, measured by the original's own measure, clock zero, not yet
//!   presampled), then stepped by the original's update in the interpreter and by the port for
//!   [`TICKS`] ticks: the first frame's presample, the lag's own walk, the curve on the measure's
//!   stale tangents (one cab lagged onto the segment the measuring leaves its cursor on), and the
//!   ride on the key normals, bit for bit on every tick;
//! - [`a_level_car_drives_as_the_originals_do`]: the same cars switched to `PathIsLevel` (no car on
//!   the disc sets it) and stepped from the state, one of them stood still so that its height
//!   holds: the path's height, the one before, the hold and the pose, bit for bit.
//!
//! The state and the disc are game data, git-ignored: absent, the tests skip.

use game_render::disc::{Disc, Res};
use game_sim::math::Mat4;
use game_sim::track::{Scene, Terrain};
use game_sim::traffic::{Car, Key, Level, Spec};
use game_sim::world::Ground;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

const STATE: &str = "recording2.p2m2_SaveState.eeMemory.bin";
const BUDGET: u64 = 50_000_000;
/// Long enough for every car to cross keys, and for the trailers' lag to leave the segment the
/// measuring left their cursors on.
const TICKS: usize = 600;

/// The path car's fields (`research/oracle.txt` S81, S128).
mod at {
    pub const NODE: u32 = 0x254;
    pub const PATH: u32 = 0x430;
    pub const LAST: u32 = 0x450;
    pub const POINT: u32 = 0x460;
    pub const POSITION: u32 = 0x470;
    pub const HEAD: u32 = 0x480;
    pub const UP: u32 = 0x4a0;
    pub const LEVEL: u32 = 0x4d0;
    pub const HELD: u32 = 0x4d4;
    pub const SOULTREE: u32 = 0x4e8;
    pub const SAMPLED: u32 = 0x4dc;
    pub const FIRST: u32 = 0x4e4;
    pub const PRESAMPLE: u32 = 0x4ec;
    pub const CURSOR: u32 = 0x4f4;
    pub const DIRTY: u32 = 0x4fc;
    /// The cursor's keys, as pointers: the curve reads them, and moves them only with the cursor.
    pub const KEY_I: u32 = 0x500;
    pub const KEY_J: u32 = 0x504;
    /// The curve's tangents, kept from when the cursor last moved.
    pub const TANGENTS: u32 = 0x510;
    pub const TIME_SCALE: u32 = 0x534;
    pub const LAG: u32 = 0x558;
    pub const HEADING_FILTER: u32 = 0x55c;
    pub const NORMAL_FILTER: u32 = 0x560;
    pub const LEVEL_LAST: u32 = 0x568;
    /// The path: its key count, its keys (`0x30` each: time, length, position, normal), its clock,
    /// its level height.
    pub const KEYS: u32 = 0x8;
    pub const CLOCK: u32 = 0xc;
    pub const LEVEL_Y: u32 = 0x10;
    pub const KEY: u32 = 0x30;
    pub const WORLD: u32 = 0x80;
    pub const VTABLE: u32 = 0x8;
    /// The vtable's measure (`path_car_measure`).
    pub const MEASURE: u32 = 0xd0;
    pub const DT: u32 = 0x344;
}

fn state() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: `./kit state ram` of recording2.p2m2_SaveState.p2s", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

fn vf(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

fn set_vf(ee: &mut Ee, a: u32, v: Vf) {
    for (k, &x) in v.0.iter().enumerate() {
        ee.set_f32(a + 4 * k as u32, x);
    }
}

fn mat(ee: &Ee, a: u32) -> Mat4 {
    Mat4([vf(ee, a), vf(ee, a + 16), vf(ee, a + 32), vf(ee, a + 48)])
}

fn bits(v: Vf) -> [u32; 4] {
    v.to_bits()
}

fn mat_bits(m: &Mat4) -> [[u32; 4]; 4] {
    m.0.map(bits)
}

/// The state's path cars, by the vtable eight bytes into each.
fn cars(ee: &Ee) -> Vec<u32> {
    let vtable = addr("path_car_vtable").unwrap();
    (0..32u32 << 20).step_by(16).filter(|&a| ee.u32_at(a + at::VTABLE) == vtable).collect()
}

fn keys_of(ee: &Ee, car: u32) -> Vec<Key> {
    let path = ee.u32_at(car + at::PATH);
    let (n, table) = (ee.u32_at(path), ee.u32_at(path + at::KEYS));
    (0..n)
        .map(|k| table + k * at::KEY)
        .map(|k| Key { t: F(ee.f32_at(k)), len: F(ee.f32_at(k + 4)), pos: vf(ee, k + 0x10), normal: vf(ee, k + 0x20) })
        .collect()
}

/// A car as the state holds it: its keys (already presampled, their lengths the original's) and
/// numbers, and its state.
fn spec_of(ee: &Ee, car: u32) -> Spec {
    let keys = keys_of(ee, car);
    let points: Vec<(F, Vf)> = keys.iter().map(|k| (k.t, k.pos)).collect();
    let f = |o| F(ee.f32_at(car + o));
    let mut spec = Spec::new(&points, f(at::TIME_SCALE), f(at::HEADING_FILTER), f(at::NORMAL_FILTER), f(at::LAG), None).expect("a path");
    // The lengths are the clip's, measured before the presample: the state's, not these keys'.
    for (ours, theirs) in spec.keys.iter_mut().zip(&keys) {
        ours.len = theirs.len;
    }
    // The curve's tangents are the ones it kept, until its cursor moves.
    spec.measured = [vf(ee, car + at::TANGENTS), vf(ee, car + at::TANGENTS + 16)];
    spec.presampled(keys)
}

fn car_of(ee: &Ee, car: u32) -> Car {
    let path = ee.u32_at(car + at::PATH);
    Car {
        clock: F(ee.f32_at(path + at::CLOCK)),
        seg: ee.u32_at(car + at::CURSOR),
        point: vf(ee, car + at::LAST),
        head: vf(ee, car + at::HEAD),
        normal: vf(ee, car + at::UP),
        first: ee.u32_at(car + at::FIRST) != 0,
        pose: mat(ee, ee.u32_at(car + at::NODE) + at::WORLD),
        stale: ee.u32_at(car + at::DIRTY) == 0,
        level: Level { y: F(ee.f32_at(path + at::LEVEL_Y)), last: F(ee.f32_at(car + at::LEVEL_LAST)), held: ee.u32_at(car + at::HELD) != 0 },
    }
}

/// free09's archives, its ground, and the shell's specs of its cars (built from the `.toy` and the
/// clips, not yet presampled), for `then`.
fn on_free09<R>(then: impl FnOnce(&mut Ground<'_>, Vec<Spec>) -> R) -> Option<R> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let parts: Vec<Res> = ["Tracks\\free09.res", "Tracks\\free09_2.res"].iter().filter_map(|n| disc.res(n).ok()).collect();
    let read = |want: &str| parts.iter().find_map(|r| r.read_suffix(want).ok()).unwrap_or_else(|| panic!("no archive of free09 has {want}"));
    let scene = Scene::parse(&read(".scn")).expect("parse the scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let bytes = read(leaf.rsplit('\\').next().unwrap());
    let terrain = Terrain::parse(&bytes).expect("parse the terrain");
    let mut ground = Ground::new(&terrain, &scene.terrain);
    // The render side's own presample is not what is under test: it gets the flat ground.
    let mut flat = |p: game_render::mat::V3| (p, game_render::mat::v3(0.0, 1.0, 0.0));
    let traffic = game_render::toy::Traffic::load(&parts, "free09", &mut flat);
    let specs = traffic.sim(&parts).specs.as_ref().clone();
    Some(then(&mut ground, specs))
}

/// Which of the shell's specs is this car of the state: the same lag and time scale, the same key
/// times and the same `x` and `z` on every key (the presample moves only the height).
fn spec_for<'a>(ee: &Ee, car: u32, specs: &'a [Spec]) -> &'a Spec {
    let keys = keys_of(ee, car);
    let f = |o| ee.u32_at(car + o);
    let same = |s: &&Spec| {
        (s.lag.to_bits(), s.time_scale.to_bits()) == (f(at::LAG), f(at::TIME_SCALE))
            && s.keys.len() == keys.len()
            && s.keys.iter().zip(&keys).all(|(a, b)| a.t.to_bits() == b.t.to_bits() && a.pos.x().to_bits() == b.pos.x().to_bits() && a.pos.z().to_bits() == b.pos.z().to_bits())
    };
    specs.iter().find(same).unwrap_or_else(|| panic!("{car:#x}: no car of free09's .toy drives this path"))
}

#[test]
fn the_trailers_keys_are_presampled_as_the_originals() {
    let Some(ee) = state() else { return };
    let cars = cars(&ee);
    assert_eq!(cars.len(), 6, "the state's path cars");
    for &car in &cars {
        assert_eq!(ee.u32_at(car + at::PRESAMPLE), 1, "{car:#x} does not presample");
        assert_eq!((ee.u32_at(car + at::LEVEL), ee.u32_at(car + at::SOULTREE)), (0, 0), "{car:#x}: PathIsLevel or DriveOnSoultree");
    }
    let mut checked = 0;
    let Some(()) = on_free09(|ground, specs| {
        assert_eq!(specs.len(), 6, "free09's .toy cars on clips");
        for &car in &cars {
            let spec = spec_for(&ee, car, &specs);
            assert!(spec.presample && !spec.level, "{car:#x}: the shell's spec has the wrong switches");
            let theirs = keys_of(&ee, car);
            for (k, (ours, theirs)) in spec.keys.iter().zip(&theirs).enumerate() {
                assert_eq!(ours.len.to_bits(), theirs.len.to_bits(), "{car:#x} key {k}: length {} against {}", ours.len.0, theirs.len.0);
            }
            spec.presample(ground);
            for (k, (ours, theirs)) in spec.keys().iter().zip(&theirs).enumerate() {
                assert_eq!(bits(ours.pos), bits(theirs.pos), "{car:#x} key {k}: position {:?} against {:?}", ours.pos, theirs.pos);
                assert_eq!(bits(ours.normal), bits(theirs.normal), "{car:#x} key {k}: normal {:?} against {:?}", ours.normal, theirs.normal);
                checked += 1;
            }
        }
    }) else {
        return;
    };
    assert!(checked > 100, "only {checked} keys");
}

#[test]
fn the_trailers_drive_from_their_load_as_the_originals_do() {
    let Some(mut ee) = state() else { return };
    let cars = cars(&ee);
    let update = addr("path_car_update").unwrap();
    let dt = F(ee.f32_at(ee.u32_at(addr("app_pointer").unwrap()) + at::DT));
    assert!(dt > F::ZERO, "the state's dt is {}", dt.0);
    let mut stale_ticks = 0;
    let Some(()) = on_free09(|ground, specs| {
        let mut specs: Vec<Spec> = cars.iter().map(|&car| spec_for(&ee, car, &specs).clone()).collect();
        // The first cab lagged half its path's second-to-last segment, in both: its first point is
        // on the segment the measuring left the cursor on, so it rides the measure's tangents.
        let n = specs[0].keys.len();
        let lag = specs[0].keys[n - 2].len * F(0.5);
        specs[0].lag = lag;
        ee.set_f32(cars[0] + at::LAG, lag.0);
        let mut ours: Vec<Car> = specs.iter().map(Car::new).collect();
        // Every car back to its load: the clip's keys, measured by the original, clock zero, not yet
        // presampled, and the constructor's point, heading and up.
        for (spec, &car) in specs.iter().zip(&cars) {
            let path = ee.u32_at(car + at::PATH);
            let table = ee.u32_at(path + at::KEYS);
            for (k, key) in spec.keys.iter().enumerate() {
                let a = table + k as u32 * at::KEY;
                ee.set_f32(a + 4, 0.0);
                set_vf(&mut ee, a + 0x10, key.pos);
                set_vf(&mut ee, a + 0x20, Vf::new(0.0, 1.0, 0.0, 1.0));
            }
            ee.set_u32(car + at::CURSOR, 0);
            ee.set_u32(car + at::KEY_I, table);
            ee.set_u32(car + at::KEY_J, table + at::KEY);
            ee.set_u32(car + at::DIRTY, 1);
            let measure = ee.u32_at(ee.u32_at(car + at::VTABLE) + at::MEASURE);
            ee.call(measure, &[u64::from(car)], &[], BUDGET);
            assert_eq!(ee.u32_at(car + at::CURSOR), spec.cursor, "{car:#x}: where the measuring leaves the cursor");
            for (k, key) in spec.keys.iter().enumerate() {
                assert_eq!(key.len.to_bits(), ee.u32_at(table + k as u32 * at::KEY + 4), "{car:#x} key {k}: the original's measure");
            }
            ee.set_f32(path + at::CLOCK, 0.0);
            ee.set_u32(car + at::SAMPLED, 0);
            ee.set_u32(car + at::FIRST, 0);
            set_vf(&mut ee, car + at::LAST, Vf::new(0.0, 0.0, 0.0, 1.0));
            set_vf(&mut ee, car + at::HEAD, Vf::new(0.0, 0.0, 0.0, 1.0));
            set_vf(&mut ee, car + at::UP, Vf::new(0.0, 1.0, 0.0, 1.0));
        }
        for tick in 0..TICKS {
            for (k, &car) in cars.iter().enumerate() {
                let stale = ours[k].stale;
                ee.call(update, &[u64::from(car)], &[dt.0], BUDGET);
                specs[k].step(&mut ours[k], dt, ground);
                let (ours, theirs) = (&ours[k], car_of(&ee, car));
                stale_ticks += usize::from(stale && ours.stale);
                let what = |field: &str| format!("tick {tick}: car {k} ({car:#x}) {field}");
                assert_eq!(ours.clock.to_bits(), theirs.clock.to_bits(), "{}", what("clock"));
                assert_eq!(ours.seg, theirs.seg, "{}", what("cursor"));
                assert_eq!(bits(ours.point), bits(vf(&ee, car + at::POINT)), "{}", what("point"));
                assert_eq!(bits(ours.head), bits(theirs.head), "{}", what("heading"));
                assert_eq!(bits(ours.normal), bits(theirs.normal), "{}", what("up"));
                assert_eq!(bits(ours.pose.0[3]), bits(vf(&ee, car + at::POSITION)), "{}", what("position"));
                assert_eq!(mat_bits(&ours.pose), mat_bits(&theirs.pose), "{}", what("world matrix"));
                if tick == 0 {
                    assert_eq!(ee.u32_at(car + at::SAMPLED), 1, "{}", what("the first frame's flag"));
                    for (j, (a, b)) in specs[k].keys().iter().zip(keys_of(&ee, car)).enumerate() {
                        assert_eq!((bits(a.pos), bits(a.normal)), (bits(b.pos), bits(b.normal)), "{}", what(&format!("key {j} presampled")));
                    }
                }
            }
        }
    }) else {
        return;
    };
    assert!(stale_ticks > 0, "no car rode the measure's tangents: the stale branch is not exercised");
}

#[test]
fn a_level_car_drives_as_the_originals_do() {
    let Some(mut ee) = state() else { return };
    let cars = cars(&ee);
    let update = addr("path_car_update").unwrap();
    let dt = F(ee.f32_at(ee.u32_at(addr("app_pointer").unwrap()) + at::DT));
    let (mut held, mut riding) = (0, 0);
    let Some(()) = on_free09(|ground, _| {
        let mut specs = Vec::new();
        let mut ours = Vec::new();
        for (k, &car) in cars.iter().enumerate() {
            // `PathIsLevel` on, `PresampleTerrain` off, the level as the constructor leaves it; the
            // first car stood still, so its height holds on its second frame.
            let path = ee.u32_at(car + at::PATH);
            ee.set_u32(car + at::LEVEL, 1);
            ee.set_u32(car + at::PRESAMPLE, 0);
            ee.set_u32(car + at::HELD, 0);
            ee.set_u32(car + at::LEVEL_LAST, 0xff7f_ffff);
            ee.set_f32(path + at::LEVEL_Y, 0.0);
            if k == 0 {
                ee.set_f32(car + at::TIME_SCALE, 0.0);
            }
            let mut spec = spec_of(&ee, car);
            spec.presample = false;
            spec.level = true;
            specs.push(spec);
            ours.push(car_of(&ee, car));
        }
        for tick in 0..TICKS {
            for (k, &car) in cars.iter().enumerate() {
                ee.call(update, &[u64::from(car)], &[dt.0], BUDGET);
                specs[k].step(&mut ours[k], dt, ground);
                let (ours, theirs) = (&ours[k], car_of(&ee, car));
                let what = |field: &str| format!("tick {tick}: car {k} ({car:#x}) {field}");
                assert_eq!(ours.clock.to_bits(), theirs.clock.to_bits(), "{}", what("clock"));
                assert_eq!(ours.seg, theirs.seg, "{}", what("cursor"));
                assert_eq!(bits(ours.head), bits(theirs.head), "{}", what("heading"));
                assert_eq!(bits(ours.normal), bits(theirs.normal), "{}", what("up (left alone)"));
                assert_eq!(ours.level.y.to_bits(), theirs.level.y.to_bits(), "{}", what("the path's height"));
                assert_eq!(ours.level.last.to_bits(), theirs.level.last.to_bits(), "{}", what("the height before"));
                assert_eq!(ours.level.held, theirs.level.held, "{}", what("the hold"));
                assert_eq!(mat_bits(&ours.pose), mat_bits(&theirs.pose), "{}", what("world matrix"));
                held += usize::from(ours.level.held);
                riding += usize::from(!ours.level.held);
            }
        }
    }) else {
        return;
    };
    assert!(held > 0 && riding > 0, "{held} held and {riding} riding car-ticks: both branches are not exercised");
}
