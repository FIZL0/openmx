//! A fresh car (S149e): what the car's constructor builds off its `.phy` and its model, against the
//! cars the original parks, and that the car it builds drives; and a parked car's settle and sleep
//! (S484) against the original's own, stepped in the interpreter.

use std::sync::Arc;

use game_render::disc::Disc;
use game_sim::air::AcosTable;
use game_sim::bindings::Command;
use game_sim::camera::CurveTable;
use game_sim::archive::Res;
use game_sim::car::{Car, CarModel, Kind, Machine, SETTLE_FRAMES, UNDRIVEN_TOP_WHEEL_SPEED, collision_of_model, parked_spawn, parked_up};
use game_sim::eco::{Trees, Wooded};
use game_sim::flight_search::Rays;
use game_sim::phy::Ini;
use game_sim::track::{self, Scene, Terrain};
use game_sim::world::{Ground, Place, Props};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

/// A car the original parks: its RAM image (every one before anyone drives), where the car object
/// is, and the model the factory gave it.
struct Parked {
    kind: Kind,
    state: &'static str,
    at: u32,
    model: &'static str,
}

/// free09's truck (the human recording's image), free07's baja and free11's buggy (S149a's).
const PARKED: [Parked; 3] = [
    Parked { kind: Kind::MonsterTruck, state: "first_recording.p2m2_SaveState.eeMemory.bin", at: 0x00e1e210, model: "monstertruck\\anim\\monstertruck.psg" },
    Parked { kind: Kind::Baja, state: "s149a-free07.ee.bin", at: 0x00605270, model: "baja\\anim\\baja.psg" },
    Parked { kind: Kind::Buggy, state: "s149a-free11.ee.bin", at: 0x00611ba0, model: "buggy\\anim\\buggy.psg" },
];

/// The car's words: its weight, body, spawn (position, nose, up), launch, preload, yaw limit and
/// its four tires.
mod car_at {
    pub const GRAVITY: u32 = 0x140;
    pub const BODY: u32 = 0x160;
    pub const LAST_COFG: u32 = 0x250;
    pub const SPAWN: u32 = 0x270;
    pub const FACING: u32 = 0x280;
    pub const UP: u32 = 0x290;
    pub const LAUNCH: u32 = 0x42c;
    pub const PRELOAD: u32 = 0x440;
    pub const YAW_LIMIT: u32 = 0x540;
    pub const TIRES: [u32; 4] = [0x548, 0x54c, 0x550, 0x554];
}

/// The car's physics object wrapper (S484): where it is in the car, its vtable and world node, and
/// the vtable's store (spot, nose, up), put-back and enable.
mod wrapper_at {
    pub const OF_CAR: u32 = 0x50;
    pub const VTABLE: u32 = 0x38;
    pub const NODE: u32 = 0x134;
    pub const SPOT: u32 = 0x90;
    pub const NOSE: u32 = 0xa0;
    pub const UP: u32 = 0xb0;
    pub const STORE: u32 = 0xe0;
    pub const PUT_BACK: u32 = 0xdc;
    pub const ENABLE: u32 = 0x124;
}

fn quad(ee: &Ee, at: u32) -> Vf {
    Vf::from_bits([0, 4, 8, 12].map(|i| ee.u32_at(at + i)))
}

fn bits3(v: Vf) -> [u32; 3] {
    [v.x().to_bits(), v.y().to_bits(), v.z().to_bits()]
}

fn near(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol * b.abs().max(1.0)
}

/// Every word the constructor and loader set that a parked car has not moved since: bit for bit
/// where the port builds them the original's way, and inside the model round trip's rounding where
/// it does not ([`Machine::new`]).
#[test]
fn a_fresh_car_is_built_as_the_original_parks_it() {
    let Some(kit) = Kit::load().map_err(|why| skip(&why)).ok() else { return };
    let Some(mut disc) = Disc::open_configured().map_err(|why| skip(&why)).ok() else { return };
    let res = disc.res("machine.res").unwrap();
    let mut seen = 0;
    for p in &PARKED {
        let Some(ee) = Ee::from_ram(&kit.ctx("states").join(p.state)).map_err(|why| skip(&why)).ok() else { continue };
        let car = Car::read_bytes(&res.read(p.kind.phy()).unwrap(), p.kind);
        let model = CarModel::read(&res.read(p.model).unwrap()).unwrap();
        let c = p.at;
        let m = Machine::new(&car, &model, quad(&ee, c + car_at::SPAWN), quad(&ee, c + car_at::FACING), quad(&ee, c + car_at::UP));
        let b = ee.u32_at(c + car_at::BODY);
        let kind = p.kind;
        assert_eq!(m.body.inv_mass.0.to_bits(), ee.u32_at(b + 0x10), "{kind:?} inv_mass");
        assert_eq!(bits3(m.body.damping), bits3(quad(&ee, b + 0x20)), "{kind:?} damping");
        assert_eq!(bits3(m.body.ang_damping), bits3(quad(&ee, b + 0x130)), "{kind:?} ang_damping");
        assert_eq!(bits3(m.body.cofg), bits3(quad(&ee, b + 0x180)), "{kind:?} cofg");
        assert_eq!(bits3(m.last_cofg), bits3(quad(&ee, c + car_at::LAST_COFG)), "{kind:?} last cofg");
        assert_eq!(bits3(m.gravity), bits3(quad(&ee, c + car_at::GRAVITY)), "{kind:?} gravity");
        assert_eq!(m.controls.yaw_rate_limit.0, ee.f32_at(c + car_at::YAW_LIMIT), "{kind:?} yaw limit");
        let want = quad(&ee, b + 0x120);
        for i in 0..3 {
            assert!(near(m.body.inv_inertia.0[i], want.0[i], 1e-4), "{kind:?} inv_inertia {:?} want {want:?}", m.body.inv_inertia);
        }
        // The launch's timer has run down since the spawn; the rest is the file's.
        let l = |i: u32| ee.f32_at(c + car_at::LAUNCH + 4 * i);
        assert_eq!([m.launch.start.0, m.launch.time.0, m.launch.max_scale.0, m.launch.max_scale_speed.0], [l(1), l(2), l(3), l(4)], "{kind:?} launch");
        assert!(l(0) <= m.launch.timer.0, "{kind:?} launch timer");
        let pr = |i: u32| ee.f32_at(c + car_at::PRELOAD + 4 * i);
        assert_eq!([m.preload.time.0, m.preload.timer.0, m.preload.min_time.0, m.preload.max_time.0, m.preload.max_preload.0], [pr(3), pr(4), pr(5), pr(6), pr(7)], "{kind:?} preload");
        for k in 0..4 {
            let t = ee.u32_at(c + car_at::TIRES[k]);
            assert_eq!(m.wheels[k].top_wheel_speed.0, ee.f32_at(t + 0xa0), "{kind:?} tire {k} top wheel speed");
            assert_eq!(m.wheels[k].s.friction_scale.0, ee.f32_at(t + 0x9b4), "{kind:?} tire {k} friction scale");
            let want = quad(&ee, t + 0x80);
            for i in 0..3 {
                assert!((m.wheels[k].mount.0[i] - want.0[i]).abs() < 2e-3, "{kind:?} tire {k} mount {:?} want {want:?}", m.wheels[k].mount);
            }
        }
        if car.top_wheel_speed(0).is_none() {
            assert_eq!(m.wheels[0].top_wheel_speed, UNDRIVEN_TOP_WHEEL_SPEED);
        }
        // The parked body has settled a little; its turn is still the spawn's.
        let q = quad(&ee, b + 0x170);
        let dot: f32 = (0..4).map(|i| q.0[i] * m.body.quat.0[i]).sum();
        assert!(dot.abs() > 0.999, "{kind:?} quat {:?} want {q:?}", m.body.quat);
        seen += 1;
    }
    eprintln!("{seen} parked cars compared");
}

/// Built at free07's baja spot and handed the gas, each car settles on its tires and drives off
/// along its nose, upright.
#[test]
fn a_fresh_car_drives_off_on_the_gas() {
    let Some(mut disc) = Disc::open_configured().map_err(|why| skip(&why)).ok() else { return };
    let res = disc.res("machine.res").unwrap();
    let track = disc.res("Tracks\\free07.res").unwrap();
    let scene = Scene::parse(&track.read_suffix(".scn").unwrap()).unwrap();
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let bytes = track.read_suffix(leaf.rsplit('\\').next().unwrap()).unwrap();
    let terrain = Terrain::parse(&bytes).unwrap();
    let mut ground = Ground::new(&terrain, &scene.terrain);
    let (acos, curve) = (AcosTable::build(), CurveTable::build());
    for p in &PARKED {
        let car = Car::read_bytes(&res.read(p.kind.phy()).unwrap(), p.kind);
        let model = CarModel::read(&res.read(p.model).unwrap()).unwrap();
        let start = Vf::new(4020.0, 138.71805, 2357.0, 1.0);
        let mut m = Machine::new(&car, &model, start, Vf::new(0.0, 0.0, -1.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0));
        m.drive(true);
        let dt = F(1.0 / 30.0);
        for frame in 0..240 {
            let gas = frame >= 60;
            m.controls.input(&car, (F::ZERO, F::ZERO), F::ZERO, m.ground_speed, m.side_slip, |c| if gas && c == Command::GAS { F::ONE } else { F::ZERO });
            m.step(&car, &mut ground, dt, &acos, &curve, &mut |_| F::ZERO);
            assert!(m.body.position.0.iter().all(|v| v.is_finite()), "{:?} blew up on frame {frame}", p.kind);
        }
        let moved = start.z() - m.body.position.z();
        eprintln!("{:?}: {moved} ft along the nose, at {:?}, up {:?}, grounded {}", p.kind, m.body.position, m.up, m.grounded);
        assert!(moved > 30.0, "{:?} went {moved} ft", p.kind);
        assert!(m.up.y() > 0.7, "{:?} is not upright: {:?}", p.kind, m.up);
        assert!(m.grounded, "{:?} is off the ground", p.kind);
    }
}

/// `f` on free09 as a parked car meets it: the terrain, the props and the trees, out of the disc's
/// two track archives.
fn on_free09<R>(disc: &mut Disc, f: impl FnOnce(&mut Wooded<Place<'_>>) -> R) -> R {
    on_track(disc, &["Tracks\\free09.res", "Tracks\\free09_2.res"], f)
}

/// The parked truck as the port builds it: its `.phy`, model, rays and hull out of `machine.res`.
fn truck(disc: &mut Disc) -> (Car, CarModel, track::Rays, track::Mesh) {
    let res = disc.res("machine.res").unwrap();
    let kind = Kind::MonsterTruck;
    let car = Car::read_bytes(&res.read(kind.phy()).unwrap(), kind);
    let model = CarModel::read(&res.read(PARKED[0].model).unwrap()).unwrap();
    let rays = track::Rays::parse(&res.read(kind.rays()).unwrap()).unwrap();
    let hull = track::Mesh::parse(&res.read(kind.hull()).unwrap()).unwrap();
    (car, model, rays, hull)
}

/// The original's frame time while free09 loads: the asleep truck's body steps a quarter of it.
const LOAD_DT: f32 = 0.03333;

/// **The parked truck's settle and sleep, against the original's** (S484). recording2's truck is put
/// back at its spawn with its own calls (its store and put-back, then its enable) and stepped by
/// free09's node update at the load's frame time; the port's car, built at the same spawn, steps
/// beside it. The body matches bit for bit every frame, and both fall asleep on frame 56.
///
/// The put-back leaves a few of each tire's words from the image's own settle (its slip tangent,
/// last velocity, force and torque, last compression, side ratio, the low-speed grip latch and the
/// wheel's angle); a freshly built tire has them zero, as the port's are, so the test zeroes them.
/// The tire mounts and the inertia are the original's: the port's are off by its model round trip
/// ([`Machine::new`], under 0.002 ft and 1e-5 apart), which moves the bodies apart from touchdown.
#[test]
fn a_parked_truck_settles_and_sleeps_as_the_original() {
    let Some(kit) = Kit::load().map_err(|why| skip(&why)).ok() else { return };
    let Some(mut disc) = Disc::open_configured().map_err(|why| skip(&why)).ok() else { return };
    let path = kit.ctx("states").join("recording2.p2m2_SaveState.eeMemory.bin");
    let Some(mut ee) = Ee::from_ram(&path).map_err(|why| skip(&why)).ok() else { return };
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let (car, model, rays, hull) = truck(&mut disc);
    on_free09(&mut disc, |world| {
    let (acos, curve) = (AcosTable::build(), CurveTable::build());

    let c = PARKED[0].at;
    let w = c + wrapper_at::OF_CAR;
    let vt = ee.u32_at(w + wrapper_at::VTABLE);
    let node = ee.u32_at(w + wrapper_at::NODE);
    let spot = ee.scratch(0);
    for (i, o) in [car_at::SPAWN, car_at::FACING, car_at::UP].into_iter().enumerate() {
        let v = ee.read(c + o, 16);
        ee.write(spot + 16 * i as u32, 16, v);
    }
    let (pos, dir, up) = (u64::from(spot), u64::from(spot + 16), u64::from(spot + 32));
    ee.call(ee.u32_at(vt + wrapper_at::STORE), &[u64::from(w), pos, dir, up], &[], 50_000_000);
    ee.call(ee.u32_at(vt + wrapper_at::PUT_BACK), &[u64::from(w)], &[], 50_000_000);
    ee.call(ee.u32_at(vt + wrapper_at::ENABLE), &[u64::from(w)], &[], 50_000_000);
    for t in car_at::TIRES.map(|o| ee.u32_at(c + o)) {
        for o in [0xe0, 0x1a0, 0x1e0, 0x1f4, 0x214, 0x230, 0x234, 0x238, 0x250, 0x254, 0x258, 0x2b0] {
            ee.set_u32(t + o, 0);
        }
    }
    let app = ee.u32_at(addr("app_pointer").unwrap());
    ee.set_f32(app + 0x344, LOAD_DT);
    ee.set_f32(app + 0x348, 1.0 / LOAD_DT);
    let b = ee.u32_at(c + car_at::BODY);
    // The world node's update steps every awake body filed on it and runs their listeners.
    let node_update = addr("collision_node_update").unwrap();

    // The spawn: the ground under machine.txt's spot plus the model's middle.
    let spawn = parked_spawn(world, &model, Vf::new(4594.0, 135.0, 3838.0, 1.0));
    assert_eq!(bits3(spawn), bits3(quad(&ee, c + car_at::SPAWN)), "spawn {spawn:?}");
    let mut m = Machine::new(&car, &model, spawn, quad(&ee, c + car_at::FACING), quad(&ee, c + car_at::UP));
    for (k, o) in car_at::TIRES.into_iter().enumerate() {
        m.wheels[k].mount = quad(&ee, ee.u32_at(c + o) + 0x80);
    }
    m.body.inv_inertia = quad(&ee, b + 0x120);
    let mut col = collision_of_model(&car, &model, &m.body, Rays::of(&rays));
    let fresh = (m.clone(), col.clone());

    let words = |m: &Machine| {
        let b = &m.body;
        [("position", 0x60, b.position), ("velocity", 0x70, b.velocity), ("accel", 0xa0, b.accel), ("last position", 0x50, b.last_position),
         ("last quat", 0x150, b.last_quat), ("quat", 0x170, b.quat), ("rates", 0x1b0, b.rates), ("ang accel", 0x1c0, b.ang_accel)]
    };
    let mut slept = None;
    for frame in 0..200 {
        ee.call(node_update, &[u64::from(node)], &[LOAD_DT], 2_000_000_000);
        m.step_struck(&car, world, &mut col, F(LOAD_DT), &acos, &curve, &mut |_| F::ZERO);
        for (name, o, v) in words(&m) {
            let want = quad(&ee, b + o);
            assert_eq!(bits3(v), bits3(want), "frame {frame} {name} {v:?} want {want:?}");
        }
        let asleep = ee.u32_at(b) == 0;
        assert_eq!(m.at_rest(&car), asleep, "frame {frame}: at rest");
        if asleep {
            slept = Some(frame);
            break;
        }
    }
    eprintln!("the original's truck slept on frame {slept:?} at {:?}", quad(&ee, b + 0x60));
    assert_eq!(slept, Some(56));

    // The settle itself, from the same start, sleeps there too and leaves the body disabled.
    let (mut m2, mut col2) = fresh;
    assert_eq!(m2.settle(&car, world, &mut col2, F(LOAD_DT), &acos, &curve, 400), slept);
    assert!(!m2.body.enabled);
    assert_eq!(bits3(m2.body.position), bits3(quad(&ee, b + 0x60)));

    // And the Sim's parked list stands its hull there.
    let mut sim = game_sim::Sim::new(1);
    sim.park([(hull, &m2)]);
    assert_eq!(sim.parked.len(), 1);
    });
}

/// The port's own parked truck, as the shell would build it (its model's mounts and inertia),
/// settles near where both of the original's runs left it: those differ from each other by more
/// than the port does from either, as each run's load frames stepped it at their own frame times.
#[test]
fn a_parked_truck_comes_to_rest_where_the_original_parks_it() {
    let Some(kit) = Kit::load().map_err(|why| skip(&why)).ok() else { return };
    let Some(mut disc) = Disc::open_configured().map_err(|why| skip(&why)).ok() else { return };
    let (car, model, rays, _) = truck(&mut disc);
    on_free09(&mut disc, |world| {
    let (acos, curve) = (AcosTable::build(), CurveTable::build());
    let spawn = parked_spawn(world, &model, Vf::new(4594.0, 135.0, 3838.0, 1.0));
    let mut m = Machine::new(&car, &model, spawn, Vf::new(0.0, 0.0, -1.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0));
    let mut col = collision_of_model(&car, &model, &m.body, Rays::of(&rays));
    let slept = m.settle(&car, world, &mut col, F(LOAD_DT), &acos, &curve, 400);
    eprintln!("the port's truck slept on frame {slept:?} at {:?}, turned {:?}", m.body.position, m.body.quat);
    assert!(slept.is_some_and(|f| (40..80).contains(&f)), "slept on {slept:?}");
    let mut seen = 0;
    for state in ["first_recording.p2m2_SaveState.eeMemory.bin", "recording2.p2m2_SaveState.eeMemory.bin"] {
        let Some(ee) = Ee::from_ram(&kit.ctx("states").join(state)).map_err(|why| skip(&why)).ok() else { continue };
        let b = ee.u32_at(PARKED[0].at + car_at::BODY);
        assert_eq!(ee.u32_at(b), 0, "{state}: the truck is asleep");
        let (at, q) = (quad(&ee, b + 0x60), quad(&ee, b + 0x170));
        let off = (0..3).map(|i| (m.body.position.0[i] - at.0[i]).abs()).fold(0.0, f32::max);
        let dot: f32 = (0..4).map(|i| q.0[i] * m.body.quat.0[i]).sum();
        eprintln!("{state}: rests at {at:?} turned {q:?}: {off} ft off");
        assert!(off < 0.15, "{state}: {off} ft from {at:?}");
        assert!(dot.abs() > 0.99999, "{state}: turned {q:?}");
        seen += 1;
    }
    eprintln!("{seen} images compared");
    });
}

/// R52's free07 images (copy 3, slot (5, 1)): the game frames since "in game" each was saved on,
/// and its baja body's `x`.
const BAJA_IMAGES: [(&str, usize, f32); 2] = [("r52-free07-baja.ee.bin", 303, 3916.3367), ("r52-free07-baja-later.ee.bin", 1711, 3941.2859)];

/// free07's `[Machine1]`: its `Position` in `machine.txt`.
const BAJA_AT: Vf = Vf([3899.637, 151.1854, 2438.25, 1.0]);

/// free07's `[Machine1]` `Direction` line in `machine.txt`, read as the game reads it: through the
/// ini reader, which truncates where an `f32` literal rounds to nearest (the y's last bit).
fn baja_facing() -> Vf {
    Ini::parse("[Machine1]\nDirection = 0.9989964, -0.03599517, 0.02666235\n").section("Machine1").vector("Direction", Vf::ZERO)
}

/// **free07's parked baja rolls on as the original's, and play starts where the original's load
/// leaves it** (B47). R52's image's baja is put back at its spawn with its own calls (its stored
/// spot, then its enable), as [`a_parked_truck_settles_and_sleeps_as_the_original`]'s truck, and
/// stepped by free07's node update at the load's frame time beside the port's car. Its spawn's up
/// (`+0x290`) is the ground's normal under the spot, a 30-degree face ([`parked_up`]). On this slope
/// neither sleeps: the port's body keeps within 0.05 ft of the original's over 1810 frames (its
/// velocity word is 1e-5 off from the first frame, from the turn's last bit, and the two stick-slip
/// out of phase later, but both creep downhill alike).
///
/// The original's body passes R52's two images' places 88 and 98 frames after the frames they were
/// saved on, so its load steps the car about 90 frames before play: [`SETTLE_FRAMES`]. The port
/// once settled it 400 frames, and upright, so it left the slope's near-stall 30 frames early and
/// stood 21 ft further down at the start of play (R52 read that as a rest at 3919.8, but that is the
/// hull's node: the body was still awake and rolling).
#[test]
fn free07s_parked_baja_rolls_on_as_the_original() {
    let Some(kit) = Kit::load().map_err(|why| skip(&why)).ok() else { return };
    let Some(mut disc) = Disc::open_configured().map_err(|why| skip(&why)).ok() else { return };
    let path = kit.ctx("states").join(BAJA_IMAGES[0].0);
    let Some(mut ee) = Ee::from_ram(&path).map_err(|why| skip(&why)).ok() else { return };
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let res = disc.res("machine.res").unwrap();
    let kind = Kind::Baja;
    let car = Car::read_bytes(&res.read(kind.phy()).unwrap(), kind);
    let model = CarModel::read(&res.read(PARKED[1].model).unwrap()).unwrap();
    let rays = track::Rays::parse(&res.read(kind.rays()).unwrap()).unwrap();
    let hull = track::Mesh::parse(&res.read(kind.hull()).unwrap()).unwrap();
    on_track(&mut disc, &["Tracks\\free07.res", "Tracks\\free07_2.res"], |world| {
    let (acos, curve) = (AcosTable::build(), CurveTable::build());
    let c = PARKED[1].at;
    let w = c + wrapper_at::OF_CAR;
    let vt = ee.u32_at(w + wrapper_at::VTABLE);
    let node = ee.u32_at(w + wrapper_at::NODE);
    // The put-back squares the store's nose and up in place, so the image's store holds the load's
    // put-back's squared pair and a second put-back squares it again (the up's last bits move).
    // Give it back the pair the load stored, the car's own copies, so this put-back is the load's.
    let (copy_nose, copy_up) = (quad(&ee, c + car_at::FACING), quad(&ee, c + car_at::UP));
    let (once_nose, once_up) = game_sim::flight::spawn_axes(copy_nose, copy_up);
    let stored = (quad(&ee, w + wrapper_at::NOSE), quad(&ee, w + wrapper_at::UP));
    assert_eq!((once_nose.to_bits(), once_up.to_bits()), (stored.0.to_bits(), stored.1.to_bits()), "the load squared the car's copies once");
    for (to, from) in [(wrapper_at::NOSE, car_at::FACING), (wrapper_at::UP, car_at::UP)] {
        for i in [0, 4, 8, 12] {
            ee.set_u32(w + to + i, ee.u32_at(c + from + i));
        }
    }
    ee.call(ee.u32_at(vt + wrapper_at::PUT_BACK), &[u64::from(w)], &[], 50_000_000);
    ee.call(ee.u32_at(vt + wrapper_at::ENABLE), &[u64::from(w)], &[], 50_000_000);
    for t in car_at::TIRES.map(|o| ee.u32_at(c + o)) {
        for o in [0xe0, 0x1a0, 0x1e0, 0x1f4, 0x214, 0x230, 0x234, 0x238, 0x250, 0x254, 0x258, 0x2b0] {
            ee.set_u32(t + o, 0);
        }
    }
    let app = ee.u32_at(addr("app_pointer").unwrap());
    ee.set_f32(app + 0x344, LOAD_DT);
    ee.set_f32(app + 0x348, 1.0 / LOAD_DT);
    let b = ee.u32_at(c + car_at::BODY);
    let node_update = addr("collision_node_update").unwrap();

    // machine.txt's spot gives the original's spawn but for the ground's last bits (5e-5 ft), its
    // nose the file's, its up the ground's normal; the put-back turns the body by them.
    let (at, facing) = (BAJA_AT, baja_facing());
    let spawn = parked_spawn(world, &model, at);
    assert!((spawn.y() - quad(&ee, c + car_at::SPAWN).y()).abs() < 1e-4, "spawn {spawn:?}");
    let nose = quad(&ee, c + car_at::FACING);
    assert_eq!(bits3(facing), bits3(nose), "nose {nose:?}");
    let up = parked_up(world, at);
    assert_eq!(bits3(up), bits3(quad(&ee, c + car_at::UP)), "up {up:?}");
    let mut m = Machine::new(&car, &model, quad(&ee, c + car_at::SPAWN), nose, up);
    // Turned bit for bit: squared once, from the file's nose and the ground's up, as the load's put-back.
    let turn = quad(&ee, b + 0x170);
    assert_eq!(m.body.quat.to_bits(), turn.to_bits(), "turned {:?} want {turn:?}", m.body.quat);
    for (k, o) in car_at::TIRES.into_iter().enumerate() {
        m.wheels[k].mount = quad(&ee, ee.u32_at(c + o) + 0x80);
    }
    m.body.inv_inertia = quad(&ee, b + 0x120);
    let mut col = collision_of_model(&car, &model, &m.body, Rays::of(&rays));

    let mut passed = [None; 2];
    let (mut last_x, mut worst) = (0.0f32, 0.0f32);
    let mut at_play = Vf::default();
    for frame in 0..1810 {
        ee.call(node_update, &[u64::from(node)], &[LOAD_DT], 2_000_000_000);
        m.step_struck(&car, world, &mut col, F(LOAD_DT), &acos, &curve, &mut |_| F::ZERO);
        let want = quad(&ee, b + 0x60);
        assert_ne!(ee.u32_at(b), 0, "frame {frame}: the original's is awake");
        assert!(!m.at_rest(&car), "frame {frame}: the port's at rest at {:?}", m.body.position);
        let off = (0..3).map(|i| (m.body.position.0[i] - want.0[i]).abs()).fold(0.0, f32::max);
        worst = worst.max(off);
        assert!(off < 0.05, "frame {frame}: {:?} want {want:?}", m.body.position);
        for (k, &(_, _, x)) in BAJA_IMAGES.iter().enumerate() {
            if last_x < x && want.x() >= x {
                passed[k] = Some(frame);
            }
        }
        last_x = want.x();
        if frame + 1 == SETTLE_FRAMES {
            at_play = want;
        }
    }
    eprintln!("the port's baja within {worst} ft of the original's; passes the images' places on {passed:?}");
    for (k, &(state, saved, _)) in BAJA_IMAGES.iter().enumerate() {
        let load = passed[k].expect(state) - saved;
        assert!((85..=100).contains(&load), "{state}: the load stepped {load} frames");
        assert!(load.abs_diff(SETTLE_FRAMES) <= 10, "{state}: the load stepped {load} frames, the port's {SETTLE_FRAMES}");
    }

    // The port's own settle, as the shell parks it (its model's mounts and inertia), leaves it awake
    // where the original's stands at that frame.
    let p = game_sim::car::Parked::settled(Arc::new(car.clone()), &model, Rays::of(&rays), Arc::new(hull), world, at, facing, &acos, &curve);
    let got = p.machine.body.position;
    let off = (0..3).map(|i| (got.0[i] - at_play.0[i]).abs()).fold(0.0, f32::max);
    eprintln!("the port's settle leaves it awake {} at {got:?}, {off} ft from the original's", p.awake());
    assert!(p.awake(), "the settle put it to sleep at {got:?}");
    assert!(off < 0.05, "settled at {got:?}, the original's at {at_play:?}");
    });
}

/// `f` on the track whose archives are `names` (the first holding the scene), as a parked car meets
/// it: the terrain, the props and the trees.
fn on_track<R>(disc: &mut Disc, names: &[&str], f: impl FnOnce(&mut Wooded<Place<'_>>) -> R) -> R {
    let parts: Vec<(Vec<u8>, Res)> = names.iter().filter_map(|n| disc.member(n).ok()).filter_map(|b| Res::parse(&b).ok().map(|r| (b, r))).collect();
    let by_end = |end: &str| {
        let want = end.to_ascii_lowercase();
        parts.iter().find_map(|(b, r)| {
            let e = r.entries.iter().find(|e| {
                let n = e.name.to_ascii_lowercase();
                n == want || n.ends_with(&format!("\\{want}"))
            })?;
            r.read(b, e).ok()
        })
    };
    let (bytes, res) = &parts[0];
    let member = |end: &str| res.read(bytes, res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(end)).unwrap()).unwrap();
    let scene = Scene::parse(&member(".scn")).unwrap();
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let terrain_file = by_end(leaf.rsplit('\\').next().unwrap()).unwrap();
    let terrain = Terrain::parse(&terrain_file).unwrap();
    let (props, _) = Props::load(&scene, |want| by_end(want).ok_or_else(|| format!("no {want}")));
    let ground = Ground::new(&terrain, &scene.terrain);
    let (trees, _) = Trees::on_ground(&member(".esb"), &ground, &CurveTable::build()).unwrap();
    f(&mut Wooded { world: Place { ground, props }, trees })
}

/// **A Restart puts free07's parked baja back as the original's does** (S500). R67: each world reset
/// (the load's, and each Restart's from the pause menu) runs every actor's reset, and the car's puts
/// it back from its physics object's store, squaring the store's nose and up in place again. R52's
/// image holds the load's once-squared store; the original's world reset is run on it twice, and
/// the port's put-back twice beside it: the body's place and turn, and the store, bit for bit
/// after each, and the body's speeds and turn rates zero.
#[test]
fn a_restart_puts_the_parked_baja_back_as_the_original() {
    let Some(kit) = Kit::load().map_err(|why| skip(&why)).ok() else { return };
    let Some(mut disc) = Disc::open_configured().map_err(|why| skip(&why)).ok() else { return };
    let path = kit.ctx("states").join(BAJA_IMAGES[0].0);
    let Some(mut ee) = Ee::from_ram(&path).map_err(|why| skip(&why)).ok() else { return };
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let res = disc.res("machine.res").unwrap();
    let kind = Kind::Baja;
    let car = Car::read_bytes(&res.read(kind.phy()).unwrap(), kind);
    let model = CarModel::read(&res.read(PARKED[1].model).unwrap()).unwrap();
    let rays = track::Rays::parse(&res.read(kind.rays()).unwrap()).unwrap();
    let hull = track::Mesh::parse(&res.read(kind.hull()).unwrap()).unwrap();
    on_track(&mut disc, &["Tracks\\free07.res", "Tracks\\free07_2.res"], |world| {
    let (acos, curve) = (AcosTable::build(), CurveTable::build());
    let c = PARKED[1].at;
    let w = c + wrapper_at::OF_CAR;
    let b = ee.u32_at(c + car_at::BODY);
    let store = |ee: &Ee| (quad(ee, w + wrapper_at::SPOT), quad(ee, w + wrapper_at::NOSE), quad(ee, w + wrapper_at::UP));

    // The port's load leaves the store as the original's: its pair squared once.
    let mut p = game_sim::car::Parked::settled(Arc::new(car.clone()), &model, Rays::of(&rays), Arc::new(hull), world, BAJA_AT, baja_facing(), &acos, &curve);
    let (spot, nose, up) = store(&ee);
    // The x, y and z: the original's w lanes hold 1.0 (its factory's copies), the port's the file
    // nose's 0, and the turn reads no w (S498 proved it bit for bit from the port's).
    assert_eq!((bits3(p.store.nose), bits3(p.store.up)), (bits3(nose), bits3(up)), "the load's store");
    // Its spot but for the ground's last bits (5e-5 ft, see the test above).
    assert!((0..3).all(|i| (p.store.at.0[i] - spot.0[i]).abs() < 1e-4), "spot {:?} want {spot:?}", p.store.at);
    p.store.at = spot;

    let app = ee.u32_at(addr("app_pointer").unwrap());
    // The world reset's part that puts the cars back: its game manager's (world +0x30) actor manager
    // (+0x34) running every actor's reset. R67 found the whole world reset's bits the same; here the
    // rest of it calls through a pointer the interpreter has no hook for.
    let actors = ee.u32_at(ee.u32_at(ee.u32_at(app + 0x3cc) + 0x30) + 0x34);
    assert_ne!(actors, 0, "the actor manager");
    let each = addr("actor_manager_each_early").unwrap();
    for restart in 1..=2 {
        ee.call(each, &[u64::from(actors)], &[], 2_000_000_000);
        p.put_back();
        let (spot, nose, up) = store(&ee);
        assert_eq!(bits3(spot), bits3(p.store.at), "restart {restart}: the spot is kept");
        assert_eq!((bits3(p.store.nose), bits3(p.store.up)), (bits3(nose), bits3(up)), "restart {restart}: the store squared again");
        let m = &p.machine.body;
        for (name, o, v) in [("position", 0x60, m.position), ("quat", 0x170, m.quat), ("last quat", 0x150, m.last_quat), ("velocity", 0x70, m.velocity), ("rates", 0x1b0, m.rates)] {
            let want = quad(&ee, b + o);
            eprintln!("restart {restart}: {name} {v:?}, the original's {want:?}");
            assert_eq!(bits3(v), bits3(want), "restart {restart}: {name} {v:?} want {want:?}");
        }
        assert_eq!(m.quat.to_bits()[3], quad(&ee, b + 0x170).to_bits()[3], "restart {restart}: quat w");
    }
    // R67's bits after one restart, which the second turns on from.
    eprintln!("the store's up after two restarts {:?}", p.store.up);
    });
}
