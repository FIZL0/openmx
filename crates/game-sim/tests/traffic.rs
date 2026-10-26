//! The path cars against the original (S81): s89-along's save state of free06, the train and the
//! earth movers on their clips, and the disc's terrain under them.
//!
//! 1. **the measure** — every car's path, measured by the port from the state's keys, is the state's
//!    arc lengths bit for bit;
//! 2. **the drive** — every car, started from the state's, stepped by the port and by the original's
//!    own update in the interpreter for [`TICKS`] ticks, is the original's on every tick: the clock,
//!    the curve's cursor, the point, the heading, the up and the model's world matrix, bit for bit;
//! 3. **the mesh** — the car's collision mesh, stood on the port's pose, has the original's sphere
//!    (the collidable's centre and radius) and its world matrix, bit for bit.

use game_sim::archive::{Psr, Res};
use game_sim::math::Mat4;
use game_sim::track::{Mesh, Scene, Terrain};
use game_sim::traffic::{Car, Spec, Traffic};
use game_sim::world::{Ground, Place, Props};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

const STATE: &str = "s89-along.eeMemory.bin";
const TRACK: &str = "Tracks\\free06.res";
const BUDGET: u64 = 50_000_000;
/// How long the drive is: ten seconds, the train well past a key or two of its 251.
const TICKS: usize = 300;
/// The scene's step, a thirtieth of a second.
const DT: F = F(1.0 / 30.0);

/// The state's 21 path cars: the train's 13 (engine first, each wagon a `LagDistance` further back),
/// four dump trucks, two scrapers' fronts and their backs.
const CARS: [u32; 21] = [
    0x00d5_25f0, 0x00d5_cb60, 0x00d6_3660, 0x00d6_c990, 0x00d7_6440, 0x00d7_fd50, 0x00d8_69e0, 0x00d8_7930,
    0x00d8_bab0, 0x00d8_f670, 0x00d9_3c40, 0x00d9_4820, 0x00d9_5660, 0x01ca_7f60, 0x01cb_bf20, 0x01cc_9300,
    0x01cc_a8d0, 0x01cc_bfe0, 0x01cf_8b00, 0x01cf_99d0, 0x01cf_b180,
];

/// The car's fields (`research/oracle.txt` S81).
mod at {
    pub const NODE: u32 = 0x254;
    pub const NAME: u32 = 0x320;
    pub const MESH: u32 = 0x428;
    pub const PATH: u32 = 0x430;
    pub const LAST: u32 = 0x450;
    pub const POINT: u32 = 0x460;
    pub const POSITION: u32 = 0x470;
    pub const HEAD: u32 = 0x480;
    pub const UP: u32 = 0x4a0;
    pub const FIRST: u32 = 0x4e4;
    pub const CURSOR: u32 = 0x4f4;
    pub const TIME_SCALE: u32 = 0x534;
    pub const LAG: u32 = 0x558;
    pub const HEADING_FILTER: u32 = 0x55c;
    pub const NORMAL_FILTER: u32 = 0x560;
    /// The path: its key count, its keys (`0x30` each), its clock.
    pub const KEYS: u32 = 0x8;
    pub const CLOCK: u32 = 0xc;
    pub const KEY: u32 = 0x30;
    /// The model node's world matrix; the mesh collidable's sphere and world matrix.
    pub const WORLD: u32 = 0x80;
    pub const CENTRE: u32 = 0x40;
    pub const RADIUS: u32 = 0x60;
    pub const PLACED: u32 = 0x120;
}

fn state() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: `./kit state ram` of S89's s89-along save state", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

/// free06's archives, whole: the track's and its second.
fn archives() -> Option<Vec<(Vec<u8>, Res)>> {
    let kit = Kit::load().ok()?;
    let at = kit.ctx("iso").join("MX.PSR");
    let Ok(mut file) = std::fs::File::open(&at) else {
        skip(&format!("no {} — `./kit extract MX.PSR`", at.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    std::io::Read::read_exact(&mut file, &mut front).expect("the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let mut parts = Vec::new();
    for name in [TRACK.to_string(), TRACK.replace(".res", "_2.res")] {
        let Some(e) = psr.find(&name) else { continue };
        std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(e.at)).expect("seek");
        let mut bytes = vec![0u8; e.size as usize];
        std::io::Read::read_exact(&mut file, &mut bytes).expect("read");
        if let Ok(res) = Res::parse(&bytes) {
            parts.push((bytes, res));
        }
    }
    (!parts.is_empty()).then_some(parts)
}

fn member(parts: &[(Vec<u8>, Res)], want: &str) -> Option<Vec<u8>> {
    let want = want.to_ascii_lowercase();
    parts.iter().find_map(|(bytes, res)| {
        let e = res.entries.iter().find(|e| {
            let name = e.name.to_ascii_lowercase();
            name == want || name.ends_with(&format!("\\{want}"))
        })?;
        res.read(bytes, e).ok()
    })
}

fn vf(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

fn mat(ee: &Ee, a: u32) -> Mat4 {
    Mat4([vf(ee, a), vf(ee, a + 16), vf(ee, a + 32), vf(ee, a + 48)])
}

fn name(ee: &Ee, car: u32) -> String {
    let bytes: Vec<u8> = (0..64).map(|k| ee.u8_at(car + at::NAME + k)).take_while(|&b| b != 0).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// A car's path and numbers, out of the state; its mesh off the disc when there is a disc.
fn spec_of(ee: &Ee, car: u32, mesh: Option<Mesh>) -> Spec {
    let path = ee.u32_at(car + at::PATH);
    let (n, keys) = (ee.u32_at(path), ee.u32_at(path + at::KEYS));
    let points: Vec<(F, Vf)> = (0..n).map(|k| keys + k * at::KEY).map(|k| (F(ee.f32_at(k)), vf(ee, k + 0x10))).collect();
    let f = |o| F(ee.f32_at(car + o));
    Spec::new(&points, f(at::TIME_SCALE), f(at::HEADING_FILTER), f(at::NORMAL_FILTER), f(at::LAG), mesh).expect("a path")
}

/// A car's state, out of the state.
fn car_of(ee: &Ee, car: u32) -> Car {
    Car {
        clock: F(ee.f32_at(ee.u32_at(car + at::PATH) + at::CLOCK)),
        seg: ee.u32_at(car + at::CURSOR),
        point: vf(ee, car + at::LAST),
        head: vf(ee, car + at::HEAD),
        normal: vf(ee, car + at::UP),
        first: ee.u32_at(car + at::FIRST) != 0,
        pose: mat(ee, ee.u32_at(car + at::NODE) + at::WORLD),
    }
}

fn bits(v: Vf) -> [u32; 4] {
    v.to_bits()
}

fn mat_bits(m: &Mat4) -> [[u32; 4]; 4] {
    m.0.map(bits)
}

#[test]
fn the_ports_arc_lengths_are_the_originals() {
    let Some(ee) = state() else { return };
    let mut keys = 0;
    for &car in &CARS {
        let spec = spec_of(&ee, car, None);
        let table = ee.u32_at(ee.u32_at(car + at::PATH) + at::KEYS);
        for (k, key) in spec.keys.iter().enumerate() {
            let theirs = ee.u32_at(table + k as u32 * at::KEY + 4);
            assert_eq!(key.len.to_bits(), theirs, "{} ({car:#x}) key {k}: {} against {}", name(&ee, car), key.len.0, f32::from_bits(theirs));
            keys += 1;
        }
    }
    assert!(keys > 10_000, "only {keys} keys measured");
}

#[test]
fn the_cars_drive_as_the_originals_do() {
    let Some(mut ee) = state() else { return };
    let Some(parts) = archives() else { return };
    let scn = member(&parts, "free06.scn").expect("free06.scn");
    let scene = Scene::parse(&scn).expect("free06's scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let terrain_bytes = member(&parts, &leaf).expect("free06's terrain");
    let terrain = Terrain::parse(&terrain_bytes).expect("free06's terrain");
    let mut world = Place { ground: Ground::new(&terrain, &scene.terrain), props: Props::default() };
    let update = addr("path_car_update").unwrap();

    let specs: Vec<Spec> = CARS
        .iter()
        .map(|&car| {
            let col = name(&ee, car).to_ascii_lowercase().replace(".psg", ".col");
            let mesh = member(&parts, &col).map(|b| Mesh::parse(&b).unwrap_or_else(|e| panic!("{col}: {e}")));
            assert!(mesh.is_some(), "no {col} in free06's archives");
            spec_of(&ee, car, mesh)
        })
        .collect();
    let mut traffic = Traffic::new(specs);
    traffic.cars = CARS.iter().map(|&car| car_of(&ee, car)).collect();
    traffic.attach(&mut world.props);
    let first = traffic.first.expect("attached");

    for tick in 0..TICKS {
        for &car in &CARS {
            ee.call(update, &[u64::from(car)], &[DT.0], BUDGET);
        }
        traffic.frame(DT, &mut world);
        for (k, &car) in CARS.iter().enumerate() {
            let ours = &traffic.cars[k];
            let what = |field: &str| format!("tick {tick}: {} ({car:#x}) {field}", name(&ee, car));
            let path = ee.u32_at(car + at::PATH);
            assert_eq!(ours.clock.to_bits(), ee.u32_at(path + at::CLOCK), "{}", what("clock"));
            assert_eq!(ours.seg, ee.u32_at(car + at::CURSOR), "{}", what("cursor"));
            assert_eq!(bits(ours.point), bits(vf(&ee, car + at::POINT)), "{}", what("point"));
            assert_eq!(bits(ours.point), bits(vf(&ee, car + at::LAST)), "{}", what("last point"));
            assert_eq!(bits(ours.head), bits(vf(&ee, car + at::HEAD)), "{}", what("heading"));
            assert_eq!(bits(ours.normal), bits(vf(&ee, car + at::UP)), "{}", what("up"));
            assert_eq!(bits(ours.pose.0[3]), bits(vf(&ee, car + at::POSITION)), "{}", what("position"));
            let node = ee.u32_at(car + at::NODE);
            assert_eq!(mat_bits(&ours.pose), mat_bits(&mat(&ee, node + at::WORLD)), "{}", what("world matrix"));
            // The mesh, where the port stood it, against the collidable the original moved.
            let mesh = ee.u32_at(car + at::MESH);
            let prop = &world.props.props[first + k];
            assert_eq!(mat_bits(&prop.to_world), mat_bits(&mat(&ee, mesh + at::PLACED)), "{}", what("mesh matrix"));
            assert_eq!(bits(prop.centre), bits(vf(&ee, mesh + at::CENTRE)), "{}", what("mesh centre"));
            assert_eq!(prop.radius.to_bits(), ee.u32_at(mesh + at::RADIUS), "{}", what("mesh radius"));
        }
    }
}
