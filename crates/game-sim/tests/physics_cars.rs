//! The `HasPhysics` cars against the original (S127): the path car's subclass that carries a rigid
//! body for when it is struck. s89-along's save state of free06 holds two of them, the company trucks
//! `.toy` `Car22` and `Car23`; biplane.p2m2's state holds the same two in the plane's world.
//!
//! 1. **the drive** — both trucks, started from the state's, stepped by the port as path cars and by
//!    the subclass's own update in the interpreter for [`TICKS`] ticks, are the original's on every
//!    tick: clock, cursor, point, heading, up, the model's world matrix, the mesh's matrix, centre and
//!    radius, and the velocity and spin the car answers, bit for bit; and the original's never turns
//!    physical on its own;
//! 2. **the load** — the state's trucks are the `.toy`'s `HasPhysics` cars in its order, each with
//!    its `.pco` path and a physics object whose body is off;
//! 3. **the plane's world** — the moving list's slots after the 21 path cars are these two trucks'
//!    meshes, as the port's world files them after its path cars.

use game_sim::archive::{Psr, Res};
use game_sim::math::Mat4;
use game_sim::track::{Mesh, Scene, Terrain};
use game_sim::traffic::{Car, Spec, Traffic};
use game_sim::world::{Ground, Place, Props};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

const STATE: &str = "s89-along.eeMemory.bin";
const PLANE_STATE: &str = "biplane.p2m2_SaveState.eeMemory.bin";
const TRACK: &str = "Tracks\\free06.res";
const BUDGET: u64 = 50_000_000;
/// Ten seconds of the trucks' clips.
const TICKS: usize = 300;
const DT: F = F(1.0 / 30.0);

/// The car's fields (`research/oracle.txt` S81, S136, S127).
mod at {
    pub const VTABLE: u32 = 0x8;
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
    pub const KEYS: u32 = 0x8;
    pub const CLOCK: u32 = 0xc;
    pub const KEY: u32 = 0x30;
    pub const WORLD: u32 = 0x80;
    pub const CENTRE: u32 = 0x40;
    pub const RADIUS: u32 = 0x60;
    pub const PLACED: u32 = 0x120;
    pub const UPDATE_SLOT: u32 = 0x38;
    pub const VELOCITY_SLOT: u32 = 0xb8;
    pub const SPIN_SLOT: u32 = 0xbc;
    pub const VELOCITY: u32 = 0x260;
    pub const SPIN: u32 = 0x290;
    pub const DT: u32 = 0x344;
    pub const INV_DT: u32 = 0x348;
    /// The subclass's own: physical now, physical last update, the clock's stop flag, the `.pco`
    /// path, the physics object; the object's body (its first word the body's enabled flag) and
    /// its reset countdown and `ResetTime`.
    pub const PHYSICAL: u32 = 0x588;
    pub const WAS_PHYSICAL: u32 = 0x58c;
    pub const STOP: u32 = 0x590;
    pub const PCO: u32 = 0x594;
    pub const OBJECT: u32 = 0x698;
    pub const BODY: u32 = 0x110;
    pub const COUNTDOWN: u32 = 0x1bc;
    pub const RESET_TIME: u32 = 0x1c0;
}

/// A collidable's fields and the broad phase's (as tests/traffic.rs's S141 test reads them).
mod col_at {
    pub const SPACE: u32 = 0x04;
    pub const RADIUS: u32 = 0x60;
    pub const MOVER: u32 = 0x7c;
    pub const OWNER: u32 = 0x80;
    pub const CATEGORY: u32 = 0x84;
    pub const FLAGS: u32 = 0x116;
    pub const TABLE: u32 = 0x04;
    pub const MOVING: u32 = 0x38;
    pub const LIST_LAST: u32 = 0x38;
    pub const LIST_BLOCKS: u32 = 0x3c;
}

/// The plane, and its collision record's ray probe (S48).
const PLANE: u32 = 0x00c4_4a40;
const PLANE_COLLISION: u32 = 0x170;
const RAY_PROBE: u32 = 0x1f0;
/// What a search leaves on a collidable's flags.
const MARKS: u8 = 0xf0;

fn ram(file: &str) -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("states").join(file);
    if !path.exists() {
        skip(&format!("{} is not there: `./kit state ram` of its save state", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

/// Every object in RAM whose vtable is `vtable`, by a scan of the words at `+8` of each quadword.
fn objects_of(ee: &Ee, vtable: u32) -> Vec<u32> {
    (0x0010_0000u32..0x0200_0000).step_by(16).filter(|&o| ee.u32_at(o + at::VTABLE) == vtable).collect()
}

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

fn text(ee: &Ee, a: u32) -> String {
    let bytes: Vec<u8> = (0..260).map(|k| ee.u8_at(a + k)).take_while(|&b| b != 0).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn name(ee: &Ee, car: u32) -> String {
    text(ee, car + at::NAME)
}

fn bits(v: Vf) -> [u32; 4] {
    v.to_bits()
}

fn mat_bits(m: &Mat4) -> [[u32; 4]; 4] {
    m.0.map(bits)
}

fn spec_of(ee: &Ee, car: u32, mesh: Option<Mesh>) -> Spec {
    let path = ee.u32_at(car + at::PATH);
    let (n, keys) = (ee.u32_at(path), ee.u32_at(path + at::KEYS));
    let points: Vec<(F, Vf)> = (0..n).map(|k| keys + k * at::KEY).map(|k| (F(ee.f32_at(k)), vf(ee, k + 0x10))).collect();
    let f = |o| F(ee.f32_at(car + o));
    Spec::new(&points, f(at::TIME_SCALE), f(at::HEADING_FILTER), f(at::NORMAL_FILTER), f(at::LAG), mesh).expect("a path")
}

fn car_of(ee: &Ee, car: u32) -> Car {
    Car {
        clock: F(ee.f32_at(ee.u32_at(car + at::PATH) + at::CLOCK)),
        seg: ee.u32_at(car + at::CURSOR),
        point: vf(ee, car + at::LAST),
        head: vf(ee, car + at::HEAD),
        normal: vf(ee, car + at::UP),
        first: ee.u32_at(car + at::FIRST) != 0,
        pose: mat(ee, ee.u32_at(car + at::NODE) + at::WORLD),
        stale: false,
        level: Default::default(),
    }
}

fn virtual_of(ee: &mut Ee, car: u32, slot: u32, out: u32) -> Vf {
    let f = ee.u32_at(ee.u32_at(car + at::VTABLE) + slot);
    ee.call(f, &[u64::from(car + out), u64::from(car)], &[], BUDGET);
    vf(ee, car + out)
}

/// free06.toy's cars in its reader's order, `Car1` to `Car<NumberOfCars>`: each one's model and
/// whether it has `HasPhysics`.
fn toy_cars(parts: &[(Vec<u8>, Res)]) -> Vec<(String, bool)> {
    let text = String::from_utf8_lossy(&member(parts, "free06.toy").expect("free06.toy")).into_owned();
    let mut sections: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for line in text.lines().map(|l| l.split(';').next().unwrap_or("").trim()) {
        if let Some(n) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            sections.push((n.to_ascii_lowercase(), Vec::new()));
        } else if let (Some(s), Some((k, v))) = (sections.last_mut(), line.split_once('=')) {
            s.1.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
    }
    let get = |s: &str, k: &str| sections.iter().find(|x| x.0 == s).and_then(|x| x.1.iter().find(|kv| kv.0 == k)).map(|kv| kv.1.clone());
    let count: usize = get("cars", "numberofcars").expect("NumberOfCars").parse().expect("a count");
    (1..=count)
        .map(|i| {
            let s = format!("car{i}");
            let model = get(&s, "filename").unwrap_or_default();
            (model, get(&s, "hasphysics").is_some_and(|v| v.eq_ignore_ascii_case("true")))
        })
        .collect()
}

/// The state's `HasPhysics` cars, in their addresses' order.
fn trucks(ee: &Ee) -> Vec<u32> {
    objects_of(ee, addr("physics_car_vtable").unwrap())
}

#[test]
fn the_state_holds_the_toys_physics_cars() {
    let Some(ee) = ram(STATE) else { return };
    let Some(parts) = archives() else { return };
    let toy = toy_cars(&parts);
    let wanted: Vec<&str> = toy.iter().filter(|c| c.1).map(|c| c.0.as_str()).collect();
    let found = objects_of(&ee, addr("physics_car_vtable").unwrap());
    assert_eq!(found.len(), wanted.len(), "the state's physics cars against the .toy's {wanted:?}");
    for &car in &found {
        let model = name(&ee, car);
        assert!(wanted.iter().any(|w| w.eq_ignore_ascii_case(&model)), "{model} is no HasPhysics car of the .toy");
        assert_eq!(ee.u32_at(car + at::PHYSICAL), 0, "{car:#x}: physical in the state");
        assert_eq!(ee.u32_at(car + at::WAS_PHYSICAL), 0, "{car:#x}: physical last update");
        assert_eq!(ee.u32_at(car + at::STOP), 0, "{car:#x}: the clock stops while physical");
        assert_eq!(text(&ee, car + at::PCO), "Tracks\\Free06\\ctruck.pco", "{car:#x}: the .pco");
        let object = ee.u32_at(car + at::OBJECT);
        assert_eq!(ee.u32_at(ee.u32_at(object + at::BODY)), 0, "{car:#x}: the body is on");
        assert_eq!(ee.f32_at(object + at::COUNTDOWN), -1.0, "{car:#x}: the reset countdown");
        assert_eq!(ee.f32_at(object + at::RESET_TIME), 10.0, "{car:#x}: ResetTime");
        // The mesh is the car's own, like a path car's: mover the car, no collision object.
        let mesh = ee.u32_at(car + at::MESH);
        assert_eq!(ee.u32_at(mesh + col_at::MOVER), car, "{car:#x}: the mesh's mover");
        assert_eq!(ee.u32_at(mesh + col_at::OWNER), 0, "{car:#x}: the mesh's owner");
    }
    let mut ours: Vec<usize> = toy.iter().enumerate().filter(|c| c.1.1).map(|c| c.0 + 1).collect();
    ours.sort();
    assert_eq!(ours, [22, 23], "free06's HasPhysics cars within NumberOfCars");
}

#[test]
fn the_physics_cars_drive_as_the_originals_do() {
    let Some(mut ee) = ram(STATE) else { return };
    let Some(parts) = archives() else { return };
    let scn = member(&parts, "free06.scn").expect("free06.scn");
    let scene = Scene::parse(&scn).expect("free06's scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let terrain_bytes = member(&parts, &leaf).expect("free06's terrain");
    let terrain = Terrain::parse(&terrain_bytes).expect("free06's terrain");
    let mut world = Place { ground: Ground::new(&terrain, &scene.terrain), props: Props::default() };
    let cars = trucks(&ee);
    assert_eq!(cars.len(), 2, "s89-along's physics cars");

    let specs: Vec<Spec> = cars
        .iter()
        .map(|&car| {
            let col = name(&ee, car).to_ascii_lowercase().replace(".psg", ".col");
            let mesh = member(&parts, &col).map(|b| Mesh::parse(&b).unwrap_or_else(|e| panic!("{col}: {e}")));
            assert!(mesh.is_some(), "no {col} in free06's archives");
            spec_of(&ee, car, mesh)
        })
        .collect();
    let mut traffic = Traffic::new(specs);
    traffic.cars = cars.iter().map(|&car| car_of(&ee, car)).collect();
    traffic.attach(&mut world.props);
    let first = traffic.first.expect("attached");
    let app = ee.u32_at(addr("app_pointer").unwrap());
    ee.set_f32(app + at::INV_DT, (F::ONE / DT).0);
    ee.set_f32(app + at::DT, DT.0);
    let (mut moved, mut turned) = (0, 0);

    for tick in 0..TICKS {
        // The subclass's own update, through its vtable, as the toy world's list calls it.
        for &car in &cars {
            let update = ee.u32_at(ee.u32_at(car + at::VTABLE) + at::UPDATE_SLOT);
            assert_eq!(update, addr("physics_car_update").unwrap(), "{car:#x}: the update slot");
            ee.call(update, &[u64::from(car)], &[DT.0], BUDGET);
        }
        traffic.frame(DT, &mut world);
        for (k, &car) in cars.iter().enumerate() {
            let ours = &traffic.cars[k];
            let what = |field: &str| format!("tick {tick}: {} ({car:#x}) {field}", name(&ee, car));
            assert_eq!(ee.u32_at(car + at::PHYSICAL), 0, "{}", what("turned physical"));
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
            let mesh = ee.u32_at(car + at::MESH);
            let prop = &world.props.props[first + k];
            assert_eq!(mat_bits(&prop.to_world), mat_bits(&mat(&ee, mesh + at::PLACED)), "{}", what("mesh matrix"));
            assert_eq!(bits(prop.centre), bits(vf(&ee, mesh + at::CENTRE)), "{}", what("mesh centre"));
            assert_eq!(prop.radius.to_bits(), ee.u32_at(mesh + at::RADIUS), "{}", what("mesh radius"));
        }
        for (k, &car) in cars.iter().enumerate() {
            let what = format!("tick {tick}: {} ({car:#x})", name(&ee, car));
            let moving = world.props.props[first + k].moving.unwrap_or_else(|| panic!("{what}: no motion on the mesh"));
            assert_eq!(bits(moving.velocity), bits(virtual_of(&mut ee, car, at::VELOCITY_SLOT, at::VELOCITY)), "{what}: velocity");
            assert_eq!(bits(moving.spin), bits(virtual_of(&mut ee, car, at::SPIN_SLOT, at::SPIN)), "{what}: spin");
            moved += usize::from(moving.velocity.x() != 0.0 || moving.velocity.z() != 0.0);
            turned += usize::from(moving.spin.y() != 0.0);
        }
    }
    assert!(moved > TICKS && turned > TICKS / 4, "{moved} moving and {turned} turning: the trucks did not drive");
}

/// **The plane's world lists the trucks after the path cars** (S141's "two driving toys"):
/// biplane.p2m2's moving list has free06's 21 path cars' meshes, then these two, in the `.toy`'s
/// order, each of category 0x48, flags as a path car's, mover the truck and no collision object, and
/// its radius the mesh's own. The port's [`game_sim::flight_search::world_flown`] lists a track's
/// cars in its props' order, so free06's 23, path and physics alike, built off the disc in the
/// `.toy`'s order, file the two trucks last.
#[test]
fn the_planes_world_lists_the_trucks_after_the_path_cars() {
    use game_sim::flight_search::{self, FIRST, TOY};
    let Some(ee) = ram(PLANE_STATE) else { return };
    let Some(parts) = archives() else { return };
    let ray = ee.u32_at(ee.u32_at(PLANE + PLANE_COLLISION) + RAY_PROBE);
    let list = ee.u32_at(ee.u32_at(ee.u32_at(ray + col_at::SPACE) + col_at::TABLE) + col_at::MOVING);
    let block = ee.u32_at(ee.u32_at(list + col_at::LIST_BLOCKS));
    let last = ee.i32_at(list + col_at::LIST_LAST) as u32;
    let slots: Vec<u32> = (0..last).map(|k| ee.u32_at(block + 4 * k)).collect();
    let physics = addr("physics_car_vtable").unwrap();
    let path = addr("path_car_vtable").unwrap();
    let kind = |o: u32| {
        let m = ee.u32_at(o + col_at::MOVER);
        if o & 2 != 0 || m == 0 { 0 } else { ee.u32_at(m + at::VTABLE) }
    };
    let toy = toy_cars(&parts);
    let cars: Vec<u32> = slots.iter().copied().take_while(|&o| kind(o) == path || kind(o) == physics).collect();
    assert_eq!(cars.len(), toy.len(), "the moving list's car slots against the .toy's {} cars", toy.len());
    // The original: path cars first in the .toy's order, then the physics cars in theirs.
    let order: Vec<(String, bool)> = toy.iter().filter(|c| !c.1).chain(toy.iter().filter(|c| c.1)).cloned().collect();
    for (k, &o) in cars.iter().enumerate() {
        let car = ee.u32_at(o + col_at::MOVER);
        assert_eq!(name(&ee, car), order[k].0, "slot {k}: the model");
        assert_eq!(kind(o) == physics, order[k].1, "slot {k}: the class");
        assert_eq!(ee.u32_at(o + col_at::CATEGORY), TOY, "slot {k}: the category");
        assert_eq!(ee.u32_at(o + col_at::OWNER), 0, "slot {k}: no collision object");
    }
    // free06's physics cars are its last two: the .toy's order and the original's are one here.
    assert!(toy.iter().rev().take(2).all(|c| c.1) && toy.iter().rev().skip(2).all(|c| !c.1), "free06's physics cars are Car22 and Car23");

    // The port's: the trucks' meshes off the disc, filed as toys with the original's flags and radius.
    let Some(along) = ram(STATE) else { return };
    let trucks = trucks(&along);
    let specs: Vec<Spec> = trucks
        .iter()
        .map(|&car| {
            let mesh = member(&parts, "ctruck.col").map(|b| Mesh::parse(&b).expect("ctruck.col"));
            spec_of(&along, car, mesh)
        })
        .collect();
    let mut traffic = Traffic::new(specs);
    traffic.cars = trucks.iter().map(|&car| car_of(&along, car)).collect();
    // Stepped once, so that each mesh carries its motion, which is what makes it a toy.
    let scene = Scene::parse(&member(&parts, "free06.scn").expect("free06.scn")).expect("free06's scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let terrain_bytes = member(&parts, &leaf).expect("free06's terrain");
    let terrain = Terrain::parse(&terrain_bytes).expect("free06's terrain");
    let mut world = Place { ground: Ground::new(&terrain, &scene.terrain), props: Props::default() };
    traffic.attach(&mut world.props);
    traffic.frame(DT, &mut world);
    let first = traffic.first.expect("attached");
    let (_, objects) = flight_search::world_flown(&world.props.props, None, None, None, Vf::new(0.0, 0.0, 0.0, 1.0), F::ZERO);
    for (k, &o) in cars.iter().rev().take(2).rev().enumerate() {
        let ours = &objects[FIRST + first + k];
        assert_eq!(ours.category, TOY, "truck {k}: the category");
        assert_eq!(ee.u8_at(o + col_at::FLAGS) & !MARKS, ours.flags, "truck {k}: the flags");
        assert_eq!(ee.f32_at(o + col_at::RADIUS).to_bits(), ours.radius.0.to_bits(), "truck {k}: the radius");
    }
}
