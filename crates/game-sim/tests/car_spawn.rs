//! A fresh car (S149e): what the car's constructor builds off its `.phy` and its model, against the
//! cars the original parks, and that the car it builds drives.

use game_render::disc::Disc;
use game_sim::air::AcosTable;
use game_sim::bindings::Command;
use game_sim::camera::CurveTable;
use game_sim::car::{Car, CarModel, Kind, Machine, UNDRIVEN_TOP_WHEEL_SPEED};
use game_sim::track::{Scene, Terrain};
use game_sim::world::Ground;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};

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
