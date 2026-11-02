//! The struck `HasPhysics` cars against the original (S186): s89-along's two free06 company trucks,
//! each woken by the car's own wake and then run by the world node's update in the interpreter,
//! against the port's [`PhysicsCar`] loaded off the disc and woken from the same car.
//!
//! 1. **the load** — the port's object off `ctruck.pco`, `ctruck.psg`'s box and `ctruckm.col` is the
//!    state's: the body's mass, inertia, damping and centre of mass, the collision object's tuning
//!    and its 47-contact list, the thresholds, `ResetTime`, the world's bounds, and put away (the body
//!    off, the countdown `-1`);
//! 2. **the struck mode** — the wake (`car+0x588`, the placement written back into the car's heading
//!    and up, the body's velocity and rates the car's two vehicle virtuals) and then every frame of
//!    the node's update until the car is back on its path, about eleven seconds: the body, the
//!    model node's world rows, the contact count, the countdown, the body's switch and the car's
//!    physical flag, bit for bit; the truck slides to rest, lies `ResetTime` with its collision object
//!    still searching, and is put back on the same frame;
//! 3. a mutation: a `ResetTime` off by a frame puts the truck back a frame off.

use game_sim::archive::{Psr, Res};
use game_sim::biplane::Body;
use game_sim::math::Mat4;
use game_sim::physics_car::{self, PhysicsCar};
use game_sim::prop_world::Edge;
use game_sim::props::model_file;
use game_sim::track::{Scene, Terrain};
use game_sim::traffic::{Car, Moving};
use game_sim::contact_search::Heightfield;
use game_sim::world::{Ground, World};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

const STATE: &str = "s89-along.eeMemory.bin";
const TRACK: &str = "Tracks\\free06.res";
const BUDGET: u64 = 50_000_000;
const NODE_BUDGET: u64 = 2_000_000_000;
const DT: F = F(1.0 / 30.0);
/// Past `ResetTime` (10 s) and the settling before it.
const TICKS: usize = 420;

/// The car's fields (`research/oracle.txt` S81, S127, S186).
mod at {
    pub const VTABLE: u32 = 0x8;
    pub const NODE: u32 = 0x254;
    pub const WORLD_NOW: u32 = 0x2a0;
    pub const WORLD_LAST: u32 = 0x2e0;
    pub const NAME: u32 = 0x320;
    pub const PATH: u32 = 0x430;
    pub const LAST: u32 = 0x450;
    pub const HEAD: u32 = 0x480;
    pub const UP: u32 = 0x4a0;
    pub const FIRST: u32 = 0x4e4;
    pub const CURSOR: u32 = 0x4f4;
    pub const CLOCK: u32 = 0xc;
    pub const PHYSICAL: u32 = 0x588;
    pub const OBJECT: u32 = 0x698;
    pub const VELOCITY_SLOT: u32 = 0xb8;
    pub const SPIN_SLOT: u32 = 0xbc;
    pub const VELOCITY: u32 = 0x260;
    pub const SPIN: u32 = 0x290;
    pub const DT: u32 = 0x344;
    pub const INV_DT: u32 = 0x348;
    pub const WAKE: u32 = 0x003e_3b00;
}

/// The physics object's (the props' class): the edge switch and object, the body, the collision
/// object, the world node, the countdown and `ResetTime`, the six thresholds.
mod obj {
    pub const OFF_EDGE: u32 = 0x70;
    pub const EDGE: u32 = 0x100;
    pub const BODY: u32 = 0x110;
    pub const COLLISION: u32 = 0x120;
    pub const WORLD_NODE: u32 = 0x134;
    pub const COUNTDOWN: u32 = 0x1bc;
    pub const RESET_TIME: u32 = 0x1c0;
    pub const THRESHOLDS: u32 = 0x1c4;
    /// The collision object's contact count and capacity.
    pub const CONTACTS: u32 = 0x100;
    pub const CAPACITY: u32 = 0x104;
    /// Its mass and inverse mass (the collider's).
    pub const MASS: u32 = 0x64;
}

const EDGE_MIN: u32 = 0x30;
const EDGE_MAX: u32 = 0x40;
const NODE_UPDATE: u32 = 0x0026_f800;
const WORLD_ROWS: u32 = 0x80;

/// The body's fields.
mod body_at {
    pub const ENABLED: u32 = 0x00;
    pub const INV_MASS: u32 = 0x10;
    pub const DAMPING: u32 = 0x20;
    pub const LAST_POSITION: u32 = 0x50;
    pub const POSITION: u32 = 0x60;
    pub const VELOCITY: u32 = 0x70;
    pub const LAST_FORCE: u32 = 0x80;
    pub const FORCE: u32 = 0x90;
    pub const ACCEL: u32 = 0xa0;
    pub const SHOCK_PEAK: u32 = 0xb0;
    pub const SHOCK: u32 = 0xc0;
    pub const STEP: u32 = 0xf0;
    pub const INV_INERTIA: u32 = 0x120;
    pub const ANG_DAMPING: u32 = 0x130;
    pub const LAST_QUAT: u32 = 0x150;
    pub const QUAT: u32 = 0x170;
    pub const COFG: u32 = 0x180;
    pub const MOMENT: u32 = 0x190;
    pub const LAST_MOMENT: u32 = 0x1a0;
    pub const RATES: u32 = 0x1b0;
    pub const ANG_ACCEL: u32 = 0x1c0;
    pub const TWIST_PEAK: u32 = 0x1d0;
    pub const TWIST: u32 = 0x1e0;
}

fn ram() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: `./kit state ram` of its save state", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
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

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf::from_bits([0, 4, 8, 12].map(|k| ee.u32_at(a + k)))
}

fn mat(ee: &Ee, a: u32) -> Mat4 {
    Mat4([0, 1, 2, 3].map(|k| quad(ee, a + 16 * k)))
}

fn mat_bits(m: &Mat4) -> [[u32; 4]; 4] {
    m.0.map(Vf::to_bits)
}

fn text(ee: &Ee, a: u32) -> String {
    (0..260).map(|k| ee.u8_at(a + k)).take_while(|&b| b != 0).map(|b| b as char).collect()
}

fn ee_body(ee: &Ee, b: u32) -> Body {
    use body_at::*;
    let q = |a: u32| quad(ee, b + a);
    Body {
        enabled: ee.u32_at(b + ENABLED) != 0,
        inv_mass: F(ee.f32_at(b + INV_MASS)),
        damping: q(DAMPING),
        last_position: q(LAST_POSITION),
        position: q(POSITION),
        velocity: q(VELOCITY),
        last_force: q(LAST_FORCE),
        force: q(FORCE),
        accel: q(ACCEL),
        shock_peak: q(SHOCK_PEAK),
        shock: q(SHOCK),
        step: q(STEP),
        inv_inertia: q(INV_INERTIA),
        ang_damping: q(ANG_DAMPING),
        last_quat: q(LAST_QUAT),
        quat: q(QUAT),
        cofg: q(COFG),
        moment: q(MOMENT),
        last_moment: q(LAST_MOMENT),
        rates: q(RATES),
        ang_accel: q(ANG_ACCEL),
        twist_peak: q(TWIST_PEAK),
        twist: q(TWIST),
    }
}

/// Every lane of every field that differs, but the linear acceleration's `w` (whatever a vector
/// register held, `Body::integrate`).
fn bodies_differ(got: &Body, want: &Body) -> Vec<String> {
    let mut out = Vec::new();
    if got.enabled != want.enabled {
        out.push(format!("enabled: the game {}, the port {}", want.enabled, got.enabled));
    }
    if got.inv_mass.0.to_bits() != want.inv_mass.0.to_bits() {
        out.push(format!("inv_mass: the game {:?}, the port {:?}", want.inv_mass, got.inv_mass));
    }
    for (name, g, w) in [
        ("damping", got.damping, want.damping),
        ("last_position", got.last_position, want.last_position),
        ("position", got.position, want.position),
        ("velocity", got.velocity, want.velocity),
        ("last_force", got.last_force, want.last_force),
        ("force", got.force, want.force),
        ("shock_peak", got.shock_peak, want.shock_peak),
        ("shock", got.shock, want.shock),
        ("step", got.step, want.step),
        ("inv_inertia", got.inv_inertia, want.inv_inertia),
        ("ang_damping", got.ang_damping, want.ang_damping),
        ("last_quat", got.last_quat, want.last_quat),
        ("quat", got.quat, want.quat),
        ("cofg", got.cofg, want.cofg),
        ("moment", got.moment, want.moment),
        ("last_moment", got.last_moment, want.last_moment),
        ("rates", got.rates, want.rates),
        ("ang_accel", got.ang_accel, want.ang_accel),
        ("twist_peak", got.twist_peak, want.twist_peak),
        ("twist", got.twist, want.twist),
    ] {
        if g.to_bits() != w.to_bits() {
            out.push(format!("{name}: the game {:?}, the port {:?}", w.0, g.0));
        }
    }
    if got.accel.to_bits()[..3] != want.accel.to_bits()[..3] {
        out.push(format!("accel: the game {:?}, the port {:?}", want.accel.0, got.accel.0));
    }
    out
}

fn car_of(ee: &Ee, car: u32) -> Car {
    Car {
        clock: F(ee.f32_at(ee.u32_at(car + at::PATH) + at::CLOCK)),
        seg: ee.u32_at(car + at::CURSOR),
        point: quad(ee, car + at::LAST),
        head: quad(ee, car + at::HEAD),
        normal: quad(ee, car + at::UP),
        first: ee.u32_at(car + at::FIRST) != 0,
        pose: mat(ee, ee.u32_at(car + at::NODE) + WORLD_ROWS),
        stale: false,
        level: Default::default(),
    }
}

fn virtual_of(ee: &mut Ee, car: u32, slot: u32, out: u32) -> Vf {
    let f = ee.u32_at(ee.u32_at(car + at::VTABLE) + slot);
    ee.call(f, &[u64::from(car + out), u64::from(car)], &[], BUDGET);
    quad(ee, car + out)
}

/// The state's `HasPhysics` cars, in their addresses' order.
fn trucks(ee: &Ee) -> Vec<u32> {
    let vtable = addr("physics_car_vtable").unwrap();
    (0x0010_0000u32..0x0200_0000).step_by(16).filter(|&o| ee.u32_at(o + at::VTABLE) == vtable).collect()
}

/// free06's terrain, and the truck's physics object off the disc, its bounds the terrain's extent.
struct Loaded {
    terrain_bytes: Vec<u8>,
    scene: Scene,
    car: PhysicsCar,
}

fn load(parts: &[(Vec<u8>, Res)], model: &str, edge: Option<Edge>) -> Loaded {
    let scene = Scene::parse(&member(parts, "free06.scn").expect("free06.scn")).expect("free06's scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let terrain_bytes = member(parts, &leaf).expect("free06's terrain");
    let model = model.to_ascii_lowercase();
    let pco = member(parts, &physics_car::pco_file(&model)).expect("the .pco");
    let stem = model.trim_end_matches(".psg");
    let psg = member(parts, &model_file(stem)).expect("the .psg");
    let rays = member(parts, &game_sim::props::rays_file(&model)).expect("the m.col");
    let shape = member(parts, &game_sim::props::shape_file(&model)).expect("the f.col");
    let car = PhysicsCar::load(&pco, &psg, Some(&rays), Some(&shape), edge).unwrap_or_else(|e| panic!("{model}: {e}"));
    Loaded { terrain_bytes, scene, car }
}

/// The edge object's bounds, `x` and `z`.
fn ee_edge(ee: &Ee, object: u32) -> Edge {
    let e = ee.u32_at(object + obj::EDGE);
    let lane = |at: u32, k: u32| F(ee.f32_at(e + at + 4 * k));
    Edge { min: [lane(EDGE_MIN, 0), lane(EDGE_MIN, 2)], max: [lane(EDGE_MAX, 0), lane(EDGE_MAX, 2)] }
}

#[test]
fn the_port_loads_the_trucks_physics_object() {
    let Some(ee) = ram() else { return };
    let Some(parts) = archives() else { return };
    let cars = trucks(&ee);
    assert_eq!(cars.len(), 2, "s89-along's physics cars");
    for &car in &cars {
        let model = text(&ee, car + at::NAME);
        let object = ee.u32_at(car + at::OBJECT);
        let loaded = load(&parts, &model, Some(ee_edge(&ee, object)));
        let port = &loaded.car;
        let terrain = Terrain::parse(&loaded.terrain_bytes).expect("free06's terrain");
        let ground = Ground::new(&terrain, &loaded.scene.terrain);
        let top = ground.field().max();
        let edge = ee_edge(&ee, object);
        assert_eq!(edge, Edge { min: [F::ZERO, F::ZERO], max: [F(top.x()), F(top.z())] }, "{model}: the bounds are the terrain's");
        assert!(ee.u32_at(object + obj::OFF_EDGE) != 0 && port.object().reset_off_edge, "{model}: ResetOffEdgeOfWorld");
        let body = ee_body(&ee, ee.u32_at(object + obj::BODY));
        let ours = port.body();
        for (name, g, w) in [
            ("inv_mass", Vf::splat(ours.inv_mass.0), Vf::splat(body.inv_mass.0)),
            ("inv_inertia", ours.inv_inertia, body.inv_inertia),
            ("damping", ours.damping, body.damping),
            ("ang_damping", ours.ang_damping, body.ang_damping),
            ("cofg", ours.cofg, body.cofg),
        ] {
            assert_eq!(g.to_bits(), w.to_bits(), "{model}: the body's {name}: the game {:?}, the port {:?}", w.0, g.0);
        }
        assert!(!body.enabled && !ours.enabled, "{model}: the body is off");
        let o = port.object();
        assert_eq!(o.countdown.0, ee.f32_at(object + obj::COUNTDOWN), "{model}: the countdown");
        assert_eq!(o.reset_time.0, ee.f32_at(object + obj::RESET_TIME), "{model}: ResetTime");
        for (k, t) in o.thresholds.iter().enumerate() {
            assert_eq!(t.0.to_bits(), ee.u32_at(object + obj::THRESHOLDS + 4 * k as u32), "{model}: threshold {k}");
        }
        let rec = ee.u32_at(object + obj::COLLISION);
        let c = o.collision.as_ref().expect("a collision object");
        assert_eq!(c.record.capacity as u32, ee.u32_at(rec + obj::CAPACITY), "{model}: the contact list's capacity, one a ray");
        assert_eq!(ee.u32_at(rec + obj::CONTACTS), 0, "{model}: the state's contacts");
        assert!(!o.filed && c.record.contacts.is_empty(), "{model}: put away");
        let tuning = [c.collider.mass.0, c.collider.inv_mass.0].map(f32::to_bits);
        assert_eq!(tuning, [ee.u32_at(rec + obj::MASS), ee.u32_at(rec + obj::MASS + 4)], "{model}: the collision object's mass");
    }
}

/// One truck's struck mode: woken, then the node's update, both sides, every frame until it is back.
/// `reset_time` overrides the port's `ResetTime` (the mutation). Answers the frame the port put the
/// car back on, after asserting everything before it agreed.
fn struck(truck: usize, reset_time: Option<F>) -> Option<(usize, usize)> {
    let mut ee = ram()?;
    let parts = archives()?;
    let cars = trucks(&ee);
    let car = cars[truck];
    let model = text(&ee, car + at::NAME);
    let object = ee.u32_at(car + at::OBJECT);
    let Loaded { terrain_bytes, scene, car: mut port } = load(&parts, &model, Some(ee_edge(&ee, object)));
    if let Some(t) = reset_time {
        port.set.objects[0].reset_time = t;
    }
    let terrain = Terrain::parse(&terrain_bytes).expect("free06's terrain");
    let mut ground = Ground::new(&terrain, &scene.terrain);
    let app = ee.u32_at(addr("app_pointer").unwrap());
    ee.set_f32(app + at::INV_DT, (F::ONE / DT).0);
    ee.set_f32(app + at::DT, DT.0);

    // The car as the port's traffic has it, and its pose a frame ago.
    let mut ours = car_of(&ee, car);
    let last = mat(&ee, car + at::WORLD_LAST);

    ee.call(at::WAKE, &[u64::from(car)], &[], BUDGET);
    assert!(port.wake(&mut ours, &last, F::ONE / DT), "{model}: the port's wake");
    assert!(!port.wake(&mut ours, &last, F::ONE / DT), "{model}: a second wake does nothing");
    // The motion the wake gave the body is the car's two virtuals' after the placement moved the model.
    assert_eq!(mat_bits(&ours.pose), mat_bits(&mat(&ee, ee.u32_at(car + at::NODE) + WORLD_ROWS)), "{model}: the placed model");
    let moving = Moving::of(&last, &ours.pose, F::ONE / DT);
    assert_eq!(moving.velocity.to_bits(), virtual_of(&mut ee, car, at::VELOCITY_SLOT, at::VELOCITY).to_bits(), "{model}: the car's velocity");
    assert_eq!(moving.spin.to_bits(), virtual_of(&mut ee, car, at::SPIN_SLOT, at::SPIN).to_bits(), "{model}: the car's spin");
    assert_eq!(ee.u32_at(car + at::PHYSICAL), 1, "{model}: the original's car is physical");
    let body = ee.u32_at(object + obj::BODY);
    let rec = ee.u32_at(object + obj::COLLISION);
    let node = ee.u32_at(object + obj::WORLD_NODE);
    let model_node = ee.u32_at(car + at::NODE);
    assert_eq!(ours.head.to_bits(), quad(&ee, car + at::HEAD).to_bits(), "{model}: the heading the placement wrote back");
    assert_eq!(ours.normal.to_bits(), quad(&ee, car + at::UP).to_bits(), "{model}: the up the placement wrote back");
    let woken = bodies_differ(port.body(), &ee_body(&ee, body));
    assert!(woken.is_empty(), "{model}: the woken body:\n{}", woken.join("\n"));

    let (mut rested, mut back) = (None, None);
    for tick in 0..TICKS {
        ee.call(NODE_UPDATE, &[u64::from(node)], &[DT.0], NODE_BUDGET);
        let (field, props, trees) = ground.wooded();
        let put_back = port.tick(DT, field.as_deref(), props, trees);
        let what = |field: &str| format!("{model} tick {tick}: {field}");
        let want = ee_body(&ee, body);
        let differ = bodies_differ(port.body(), &want);
        assert!(differ.is_empty(), "{}:\n{}", what("the body"), differ.join("\n"));
        if port.body().enabled {
            assert_eq!(mat_bits(&port.pose()), mat_bits(&mat(&ee, model_node + WORLD_ROWS)), "{}", what("the model node"));
        }
        let o = port.object();
        let contacts = o.collision.as_ref().map_or(0, |c| c.record.contacts.len()) as u32;
        assert_eq!(contacts, ee.u32_at(rec + obj::CONTACTS), "{}", what("the contact count"));
        assert_eq!(o.countdown.0.to_bits(), ee.u32_at(object + obj::COUNTDOWN), "{}", what("the countdown"));
        assert_eq!(port.physical, ee.u32_at(car + at::PHYSICAL) != 0, "{}", what("physical"));
        if rested.is_none() && !want.enabled {
            rested = Some(tick);
        }
        if put_back {
            back = Some(tick);
            break;
        }
    }
    let rested = rested.unwrap_or_else(|| panic!("{model}: never at rest in {TICKS} frames"));
    let back = back.unwrap_or_else(|| panic!("{model}: never put back in {TICKS} frames"));
    eprintln!("{model} ({car:#x}): at rest on frame {rested}, back on its path on frame {back}");
    Some((rested, back))
}

#[test]
fn the_first_truck_struck_settles_and_goes_back_as_the_originals_does() {
    let Some((rested, back)) = struck(0, None) else { return };
    assert!(back > rested + 290, "ResetTime's ten seconds after the rest");
}

#[test]
fn the_second_truck_struck_settles_and_goes_back_as_the_originals_does() {
    let Some((rested, back)) = struck(1, None) else { return };
    assert!(back > rested + 290, "ResetTime's ten seconds after the rest");
}

/// The mutation: a `ResetTime` a frame shorter must not agree with the original's countdown.
#[test]
fn a_shorter_reset_time_parts_from_the_original() {
    let parted = std::panic::catch_unwind(|| struck(0, Some(F(10.0) - DT)));
    match parted {
        Ok(None) => {}
        Ok(Some(_)) => panic!("a ResetTime a frame short agreed with the original"),
        Err(e) => {
            let why = e.downcast_ref::<String>().cloned().unwrap_or_default();
            assert!(why.contains("the countdown"), "parted somewhere else: {why}");
        }
    }
}

/// The node's update does nothing to the port's object while it is put away.
#[test]
fn a_put_away_truck_ticks_to_nothing() {
    let Some(parts) = archives() else { return };
    let Loaded { car: mut port, .. } = load(&parts, "ctruck.psg", None);
    let before = port.clone();
    for _ in 0..30 {
        assert!(!port.tick(DT, None, None, None));
    }
    assert!(port == before, "a put-away truck moved");
}

/// A truck on a straight path over flat ground, in the ride (`Sim`), struck on its tenth tick: it
/// stands where its body is with its clock running on, settles, and is back on its path a
/// `ResetTime` later; a snapshot from its slide, copied or sent as bytes, ticks on the same; a
/// machine without the physics cars refuses the bytes.
#[test]
fn a_struck_truck_rides_in_the_sim_and_rolls_back() {
    use game_sim::traffic::{Spec, Traffic};
    use game_sim::world::Flat;
    use game_sim::{Sim, hash, snapshot};
    let Some(parts) = archives() else { return };
    let points = [(F(0.0), Vf::new(0.0, 0.0, 0.0, 1.0)), (F(10.0), Vf::new(0.0, 0.0, 300.0, 1.0)), (F(20.0), Vf::new(0.0, 0.0, 600.0, 1.0))];
    let ride = || {
        let spec = Spec::new(&points, F(0.5), F(0.5), F(0.5), F::ZERO, None).expect("a path");
        let truck = load(&parts, "ctruck.psg", None).car;
        let mut sim = Sim::new(7);
        sim.traffic = Traffic::new(vec![spec]).with_physics(vec![Some(truck)]);
        sim
    };
    let mut world = Flat::default();
    let mut sim = ride();
    for _ in 0..10 {
        sim.step(DT, &Default::default(), &mut world);
    }
    let p = |sim: &Sim| sim.traffic.physics[0].clone().expect("the truck's physics");
    assert!(!p(&sim).physical && !p(&sim).body().enabled, "unstruck, the truck is a path car");
    assert!(sim.traffic.strike(0, DT, &mut world), "the strike wakes it");
    assert!(!sim.traffic.strike(0, DT, &mut world), "a second strike does nothing");
    assert!(p(&sim).body().velocity.z() > 10.0, "woken with the car's motion: {:?}", p(&sim).body().velocity.0);
    let clock = sim.traffic.cars[0].clock;
    let (mut saved, mut bytes, mut hashes) = (None, Vec::new(), Vec::new());
    let mut back = None;
    for t in 0..TICKS {
        if t == 5 {
            saved = Some(snapshot::save(&sim));
            bytes = snapshot::encode(&sim);
        }
        sim.step(DT, &Default::default(), &mut world);
        hashes.push(hash::of(&sim));
        let phys = p(&sim);
        if phys.physical {
            assert_eq!(mat_bits(&sim.traffic.cars[0].pose), mat_bits(&phys.pose()), "tick {t}: the car stands at its body");
        } else if back.is_none() {
            back = Some(t);
        }
    }
    let back = back.expect("put back on its path");
    assert!(back > 300, "a ResetTime after it settled: {back}");
    let car = &sim.traffic.cars[0];
    assert!(car.clock != clock, "the clock ran on");
    assert_eq!(car.pose.0[3].x(), 0.0, "back on the path's line");

    let mut again = ride();
    snapshot::restore(&mut again, saved.as_ref().expect("saved"));
    let mut sent = ride();
    snapshot::decode(&mut sent, &bytes).expect("the bytes decode");
    assert_eq!(hash::of(&sent), hash::of(&again), "sent and copied agree");
    for (t, &h) in hashes.iter().enumerate().skip(5) {
        again.step(DT, &Default::default(), &mut world);
        sent.step(DT, &Default::default(), &mut world);
        assert_eq!(hash::of(&again), h, "tick {t}: the copied snapshot");
        assert_eq!(hash::of(&sent), h, "tick {t}: the sent one");
    }
    let mut bare = Sim::new(7);
    bare.traffic = Traffic::new(vec![Spec::new(&points, F(0.5), F(0.5), F(0.5), F::ZERO, None).unwrap()]);
    assert!(snapshot::decode(&mut bare, &bytes).is_err(), "physics cars in the state, none here");
}
