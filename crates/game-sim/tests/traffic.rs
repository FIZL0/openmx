//! The path cars against the original (S81): s89-along's save state of free06, the train and the
//! earth movers on their clips, and the disc's terrain under them.
//!
//! 1. **the measure** — every car's path, measured by the port from the state's keys, is the state's
//!    arc lengths bit for bit;
//! 2. **the drive** — every car, started from the state's, stepped by the port and by the original's
//!    own update in the interpreter for [`TICKS`] ticks, is the original's on every tick: the clock,
//!    the curve's cursor, the point, the heading, the up and the model's world matrix, bit for bit;
//! 3. **the mesh** — the car's collision mesh, stood on the port's pose, has the original's sphere
//!    (the collidable's centre and radius) and its world matrix, bit for bit;
//! 4. **the motion** (S136) — on the same drive, the car's velocity and spin as the mesh carries them
//!    ([`Moving`]) are what the original's car answers its two vehicle virtuals, bit for bit, and
//!    the frame's `1/dt` is the port's divide;
//! 5. **the wheel on a car** (S136) — the wheel's contact-velocity step, on a wheel whose contact is
//!    on one of the state's cars and on one whose contact is on the world, is
//!    [`ride::wheel_contact_velocity`] on [`Moving::at`], bit for bit;
//! 6. **the ride** (S136) — a bike parked on the moving train's roof moves with it and stays on it.
//! 7. **the plane's world** (S141) — the plane's world holds the cars as the original's does: toys
//!    in its moving list ahead of its probes, in the order biplane.p2m2's state has them;
//! 8. **the plane on the train** (S141) — a plane set down on the moving train's roof stays on it
//!    and moves with it, rollback or not; one flown into its side meets it.
//! 9. **the wreck bike's world** (S171) — the original's wreck bike searches the plane's world, the
//!    cars toys in its moving list; the port's thrown wreck bike rides the wagon's roof, and the
//!    put-back's query gathers the cars from the moving list.

use game_sim::archive::{Psr, Res};
use game_sim::math::Mat4;
use game_sim::track::{Mesh, Scene, Terrain};
use game_sim::chassis::{self, Mat3};
use game_sim::ride;
use game_sim::traffic::{Car, Moving, ORIGIN, Spec, Traffic};
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
    /// S136: the vtable, the car's previous world matrix (copied at the update's top), its origin and
    /// its vehicle kind; the vehicle virtuals' slots; the application's `dt` and `1/dt`.
    pub const VTABLE: u32 = 0x8;
    pub const BEFORE: u32 = 0x2e0;
    pub const ORIGIN: u32 = 0x190;
    pub const KIND: u32 = 0x1b0;
    pub const VELOCITY_SLOT: u32 = 0xb8;
    pub const SPIN_SLOT: u32 = 0xbc;
    pub const VELOCITY: u32 = 0x260;
    pub const SPIN: u32 = 0x290;
    pub const DT: u32 = 0x344;
    pub const INV_DT: u32 = 0x348;
}

/// The application object, which keeps the frame's `dt` and `1/dt`.
fn app(ee: &Ee) -> u32 {
    ee.u32_at(addr("app_pointer").unwrap())
}

/// The car's answer to one of its vehicle virtuals (`a0` out, `a1` the car), written over the
/// field the virtual keeps its own copy in.
fn virtual_of(ee: &mut Ee, car: u32, slot: u32, out: u32) -> Vf {
    let f = ee.u32_at(ee.u32_at(car + at::VTABLE) + slot);
    ee.call(f, &[u64::from(car + out), u64::from(car)], &[], BUDGET);
    vf(ee, car + out)
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
        // Caught mid-ride: the cursor has long left where the measuring put it.
        stale: false,
        level: Default::default(),
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
    // The original's `1/dt` is the application's, made from the frame's own step (see
    // `the_frames_one_over_dt_is_the_ports_divide`): this drive's step is not the state's, so the
    // state's `1/dt` is put to this one's.
    let inv = app(&ee) + at::INV_DT;
    ee.set_f32(inv, (F::ONE / DT).0);
    let (mut moved, mut turned) = (0, 0);

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
        // S136: what the mesh says its car is doing, against what the car answers.
        for (k, &car) in CARS.iter().enumerate() {
            let what = format!("tick {tick}: {} ({car:#x})", name(&ee, car));
            let moving = world.props.props[first + k].moving.unwrap_or_else(|| panic!("{what}: no motion on the mesh"));
            assert_eq!(mat_bits(&moving.pose), mat_bits(&traffic.cars[k].pose), "{what}: the motion's pose");
            let velocity = virtual_of(&mut ee, car, at::VELOCITY_SLOT, at::VELOCITY);
            assert_eq!(bits(moving.velocity), bits(velocity), "{what}: velocity");
            let spin = virtual_of(&mut ee, car, at::SPIN_SLOT, at::SPIN);
            assert_eq!(bits(moving.spin), bits(spin), "{what}: spin");
            moved += usize::from(moving.velocity.x() != 0.0 || moving.velocity.z() != 0.0);
            turned += usize::from(moving.spin.y() != 0.0);
        }
    }
    // Not a drive of cars standing still: most move every tick, and the curves turn them.
    assert!(moved > TICKS * CARS.len() / 2 && turned > TICKS, "{moved} moving and {turned} turning");
}

/// The state's own `1/dt` is one PS2 divide of its own `dt`, which is what [`Traffic::frame`] makes;
/// and every car in it is a vehicle of kind 2, the one kind a wheel's contact keeps, about the
/// origin [`ORIGIN`] — which nothing in the port reads off a car, so here is where it is checked.
#[test]
fn the_frames_one_over_dt_is_the_ports_divide() {
    let Some(ee) = state() else { return };
    let app = app(&ee);
    let dt = F(ee.f32_at(app + at::DT));
    assert!(dt > F::ZERO, "the state's dt is {}", dt.0);
    assert_eq!((F::ONE / dt).to_bits(), ee.u32_at(app + at::INV_DT), "1/{} against {}", dt.0, ee.f32_at(app + at::INV_DT));
    for &car in &CARS {
        assert_eq!(ee.u32_at(car + at::KIND), 2, "{} ({car:#x}) kind", name(&ee, car));
        assert_eq!(bits(vf(&ee, car + at::ORIGIN)), bits(ORIGIN), "{} ({car:#x}) origin", name(&ee, car));
    }
}

/// A small generator, for the cases below.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }
    /// A float in `[-r, r)`.
    fn f(&mut self, r: f32) -> f32 {
        (self.next() as f32 / 4_294_967_296.0 * 2.0 - 1.0) * r
    }
    /// A vector `r` around `c`, with `w` given.
    fn around(&mut self, c: Vf, r: f32, w: f32) -> Vf {
        Vf::new(c.x() + self.f(r), c.y() + self.f(r), c.z() + self.f(r), w)
    }
}

/// How many wheels [`the_wheel_on_a_car_moves_as_the_originals_does`] tries.
const WHEELS: usize = 20_000;

/// The wheel's contact-velocity step against the port's (S136), on a wheel and a chassis laid out in
/// RAM s89-along leaves empty: the wheel's contact on one of the state's 21 cars, moving as the
/// state left it (its matrix before and after its last update, at the state's `1/dt`), or on the
/// world. The chassis hangs on another car's node, which only its velocity's turn into the chassis'
/// own frame reads. Everything the step writes is compared: `+0x170`, `+0x180`, `+0x190`, and what
/// it measured the car at, `+0x1b0` and `+0x1c0`.
#[test]
fn the_wheel_on_a_car_moves_as_the_originals_does() {
    const WHEEL: u32 = 0x01f8_0000;
    const CHASSIS: u32 = 0x01f9_0000;
    let Some(mut ee) = state() else { return };
    let step = addr("wheel_contact_velocity").unwrap();
    let inv_dt = F(ee.f32_at(app(&ee) + at::INV_DT));
    let put = |ee: &mut Ee, a: u32, v: Vf| {
        for (k, x) in v.0.into_iter().enumerate() {
            ee.set_f32(a + 4 * k as u32, x);
        }
    };
    let mut rng = Rng(0x5136_5136_5136_5136);
    let mut on_cars = 0;
    for case in 0..WHEELS {
        let car = CARS[rng.next() as usize % CARS.len()];
        let hung = CARS[rng.next() as usize % CARS.len()];
        let on = rng.next() % 4 != 0;
        let node = ee.u32_at(car + at::NODE);
        let now = mat(&ee, node + at::WORLD);
        let centre = now.0[3];
        let (vel, spin) = (rng.around(Vf::ZERO, 60.0, 0.0), rng.around(Vf::ZERO, 6.0, 0.0));
        let (cofg, load, contact) = (rng.around(centre, 8.0, 1.0), rng.around(centre, 8.0, 1.0), rng.around(centre, 8.0, 1.0));
        for o in (0..0x220).step_by(4) {
            ee.set_u32(WHEEL + o, 0);
        }
        for o in (0..0x2a0).step_by(4) {
            ee.set_u32(CHASSIS + o, 0);
        }
        ee.set_u32(WHEEL + 0x1e4, CHASSIS);
        ee.set_u32(WHEEL + 0x1f4, if on { car } else { 0 });
        put(&mut ee, WHEEL + 0x120, contact);
        put(&mut ee, WHEEL + 0x140, load);
        ee.set_u32(CHASSIS + at::NODE, ee.u32_at(hung + at::NODE));
        put(&mut ee, CHASSIS + 0x60, cofg);
        put(&mut ee, CHASSIS + 0x260, vel);
        put(&mut ee, CHASSIS + 0x280, spin);
        ee.call(step, &[u64::from(WHEEL)], &[], BUDGET);

        let moving = Moving::of(&mat(&ee, car + at::BEFORE), &mat(&ee, node + at::WORLD), inv_dt);
        let under = on.then(|| moving.at(contact));
        let own = ride::contact_velocity(vel, spin, load, cofg);
        let (contact_vel, chassis_vel) = ride::wheel_contact_velocity(own, vel, under);
        let hung_m = mat(&ee, ee.u32_at(hung + at::NODE) + at::WORLD);
        let local = chassis::unrotate(&Mat3::new(hung_m.0[0], hung_m.0[1], hung_m.0[2]), chassis_vel);
        let still = Vf::new(0.0, 0.0, 0.0, 1.0);
        let (theirs_v, theirs_p) = under.unwrap_or((still, still));
        let what = |field: &str| format!("case {case}: {} ({car:#x}) {}: {field}", name(&ee, car), if on { "on it" } else { "on the world" });
        assert_eq!(bits(vf(&ee, WHEEL + 0x1b0)), bits(theirs_v), "{}", what("+0x1b0 the car's velocity"));
        assert_eq!(bits(vf(&ee, WHEEL + 0x1c0)), bits(theirs_p), "{}", what("+0x1c0 the car's velocity at the contact"));
        assert_eq!(bits(vf(&ee, WHEEL + 0x170)), bits(contact_vel), "{}", what("+0x170 the contact's velocity"));
        assert_eq!(bits(vf(&ee, WHEEL + 0x180)), bits(chassis_vel), "{}", what("+0x180 the chassis' velocity"));
        assert_eq!(bits(vf(&ee, WHEEL + 0x190)), bits(local), "{}", what("+0x190 the chassis' velocity, its own way"));
        assert_eq!(ee.u32_at(WHEEL + 0x1a0), 1, "{}", what("+0x1a0"));
        on_cars += usize::from(on);
    }
    assert!(on_cars > WHEELS / 2, "only {on_cars} wheels on a car");
}

/// The wheel's contact-velocity step's owner branch (S145) against [`contact::other_at`]: the wheel's
/// contact on another collision object (`+0x1f0`), laid out in RAM s89-along leaves empty, half the
/// cases on a dynamics body (`+0xf4`: its centre `+0x60`, velocity `+0x70`, unit quaternion
/// `+0x170`, rates `+0x1b0`) and half on a vehicle's chassis (`+0xf0`: its centre of mass `+0x190`,
/// velocity `+0x260`, rates `+0x290`, on one of the cars' transform nodes at `+0xf8`), the object's
/// own centre of mass at `+0x90`. The owner branch is taken over a vehicle the wheel also names
/// (`+0x1f4`, a car in a third of the cases). Compared: the object's centre in the world (`+0xa0`),
/// `+0x1b0`, `+0x1c0`, and the step's `+0x170`, `+0x180`, `+0x190` after it. Every `w` lane is random.
#[test]
fn the_wheel_on_another_object_moves_as_the_originals_does() {
    use game_sim::biplane::Body;
    use game_sim::contact::{self, Collider, Node, Other, Side, Vehicle, Weighed};
    const WHEEL: u32 = 0x01f8_0000;
    const CHASSIS: u32 = 0x01f9_0000;
    const OWNER: u32 = 0x01fa_0000;
    const OTHER: u32 = 0x01fa_1000;
    let Some(mut ee) = state() else { return };
    let step = addr("wheel_contact_velocity").unwrap();
    let put = |ee: &mut Ee, a: u32, v: Vf| {
        for (k, x) in v.0.into_iter().enumerate() {
            ee.set_f32(a + 4 * k as u32, x);
        }
    };
    let mut rng = Rng(0x5145_5145_5145_5145);
    let mut bodies = 0;
    for case in 0..WHEELS {
        let car = CARS[rng.next() as usize % CARS.len()];
        let hung = CARS[rng.next() as usize % CARS.len()];
        let on_body = rng.next() % 2 == 0;
        let named = rng.next() % 3 == 0;
        let node_at = ee.u32_at(car + at::NODE);
        let centre = mat(&ee, node_at + at::WORLD).0[3];
        let (vel, spin) = (rng.around(Vf::ZERO, 60.0, 0.0), rng.around(Vf::ZERO, 6.0, 0.0));
        let (cofg, load, contact_at) = (rng.around(centre, 8.0, 1.0), rng.around(centre, 8.0, 1.0), rng.around(centre, 8.0, 1.0));
        let contact_at = Vf::new(contact_at.x(), contact_at.y(), contact_at.z(), rng.f(2.0));
        let theirs_v = Vf::new(rng.f(60.0), rng.f(60.0), rng.f(60.0), rng.f(2.0));
        let rates = Vf::new(rng.f(6.0), rng.f(6.0), rng.f(6.0), rng.f(2.0));
        let own_cofg = Vf::new(rng.f(3.0), rng.f(3.0), rng.f(3.0), rng.f(2.0));
        let other_centre = Vf::new(centre.x() + rng.f(8.0), centre.y() + rng.f(8.0), centre.z() + rng.f(8.0), rng.f(2.0));
        let chassis_cofg = Vf::new(rng.f(3.0), rng.f(3.0), rng.f(3.0), rng.f(2.0));
        let q = {
            let v = [rng.f(1.0), rng.f(1.0), rng.f(1.0), rng.f(1.0)];
            let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3);
            Vf::new(v[0] / n, v[1] / n, v[2] / n, v[3] / n)
        };
        for o in (0..0x220).step_by(4) {
            ee.set_u32(WHEEL + o, 0);
        }
        for o in (0..0x2a0).step_by(4) {
            ee.set_u32(CHASSIS + o, 0);
        }
        for o in (0..0x100).step_by(4) {
            ee.set_u32(OWNER + o, 0);
        }
        for o in (0..0x2a0).step_by(4) {
            ee.set_u32(OTHER + o, 0);
        }
        ee.set_u32(WHEEL + 0x1e4, CHASSIS);
        ee.set_u32(WHEEL + 0x1f0, OWNER);
        ee.set_u32(WHEEL + 0x1f4, if named { car } else { 0 });
        put(&mut ee, WHEEL + 0x120, contact_at);
        put(&mut ee, WHEEL + 0x140, load);
        ee.set_u32(CHASSIS + at::NODE, ee.u32_at(hung + at::NODE));
        put(&mut ee, CHASSIS + 0x60, cofg);
        put(&mut ee, CHASSIS + 0x260, vel);
        put(&mut ee, CHASSIS + 0x280, spin);
        put(&mut ee, OWNER + 0x90, own_cofg);
        if on_body {
            ee.set_u32(OWNER + 0xf4, OTHER);
            put(&mut ee, OTHER + 0x60, other_centre);
            put(&mut ee, OTHER + 0x70, theirs_v);
            put(&mut ee, OTHER + 0x170, q);
            put(&mut ee, OTHER + 0x1b0, rates);
        } else {
            ee.set_u32(OWNER + 0xf0, OTHER);
            ee.set_u32(OWNER + 0xf8, node_at);
            put(&mut ee, OTHER + 0x190, chassis_cofg);
            put(&mut ee, OTHER + 0x260, theirs_v);
            put(&mut ee, OTHER + 0x290, rates);
        }
        ee.call(step, &[u64::from(WHEEL)], &[], BUDGET);

        let mut collider = Collider { cofg: own_cofg, ..Collider::default() };
        let under = if on_body {
            let mut body = Body { position: other_centre, velocity: theirs_v, quat: q, rates, ..Body::default() };
            let mut rigid = Weighed { body: &mut body, mass: F::ONE };
            contact::other_at(&mut Other { collider: &mut collider, side: Side::Body(&mut rigid) }, contact_at)
        } else {
            let m = mat(&ee, node_at + at::WORLD);
            let mut v = Vehicle {
                node: Node { rows: [m.0[0], m.0[1], m.0[2]], origin: m.0[3] },
                cofg: chassis_cofg,
                velocity: theirs_v,
                rates,
                ..Vehicle::default()
            };
            contact::other_at(&mut Other { collider: &mut collider, side: Side::Vehicle(&mut v) }, contact_at)
        };
        let own = ride::contact_velocity(vel, spin, load, cofg);
        let (contact_vel, chassis_vel) = ride::wheel_contact_velocity(own, vel, Some(under));
        let hung_m = mat(&ee, ee.u32_at(hung + at::NODE) + at::WORLD);
        let local = chassis::unrotate(&Mat3::new(hung_m.0[0], hung_m.0[1], hung_m.0[2]), chassis_vel);
        let what = |field: &str| format!("case {case}: on a {}: {field}", if on_body { "body" } else { "chassis" });
        assert_eq!(bits(vf(&ee, OWNER + 0xa0)), bits(collider.cofg_world), "{}", what("+0xa0 the object's centre in the world"));
        assert_eq!(bits(vf(&ee, WHEEL + 0x1b0)), bits(under.0), "{}", what("+0x1b0 its velocity"));
        assert_eq!(bits(vf(&ee, WHEEL + 0x1c0)), bits(under.1), "{}", what("+0x1c0 its velocity at the contact"));
        assert_eq!(bits(vf(&ee, WHEEL + 0x170)), bits(contact_vel), "{}", what("+0x170 the contact's velocity"));
        assert_eq!(bits(vf(&ee, WHEEL + 0x180)), bits(chassis_vel), "{}", what("+0x180 the chassis' velocity"));
        assert_eq!(bits(vf(&ee, WHEEL + 0x190)), bits(local), "{}", what("+0x190 the chassis' velocity, its own way"));
        assert_eq!(ee.u32_at(WHEEL + 0x1a0), 1, "{}", what("+0x1a0"));
        bodies += usize::from(on_body);
    }
    assert!(bodies > WHEELS / 3 && bodies < 2 * WHEELS / 3, "{bodies} of {WHEELS} on a body");
}

/// The state's cars, stepped by the port, their meshes in `props`: what the shell hands every seat.
fn traffic_of(ee: &Ee, parts: &[(Vec<u8>, Res)], props: &mut Props) -> Traffic {
    let specs: Vec<Spec> = CARS
        .iter()
        .map(|&car| {
            let col = name(ee, car).to_ascii_lowercase().replace(".psg", ".col");
            spec_of(ee, car, member(parts, &col).map(|b| Mesh::parse(&b).unwrap_or_else(|e| panic!("{col}: {e}"))))
        })
        .collect();
    let mut traffic = Traffic::new(specs);
    traffic.cars = CARS.iter().map(|&car| car_of(ee, car)).collect();
    traffic.attach(props);
    traffic
}

/// The wagon the bike is parked on: the train's sixth car, a boxcar whose roof is flat at `y` 151.6
/// for 24 feet either way of its middle, running `+x` at 55 ft/s and turning a little. (The fourth,
/// whose deck the first probe found, is a hopper: a bowl a bike rolls to the bottom of.)
const WAGON: usize = 5;
const ROOF: f32 = 151.6;
/// How long the bike rides it: five seconds, 275 feet of track.
const PARKED: usize = 150;
/// The ticks the bike's suspension takes to settle from the spawn's height (it sinks 1.2 ft) before
/// its place on the deck is taken.
const SETTLE: usize = 30;
/// Off the deck: further from the wagon's middle along it than the roof is long, or below it.
const DECK: f32 = 24.0;

/// What [`parked`] measured.
#[derive(Debug)]
struct Parked {
    /// The most the bike moved along and across the deck, in the wagon's own frame, after
    /// [`SETTLE`].
    along: f32,
    /// And up or down off it.
    up: f32,
    /// The ticks before it left the deck — [`PARKED`] if it never did.
    on: usize,
    /// Its speed at the end, and the wagon's.
    speed: f32,
    car: f32,
}

/// A stock bike parked on [`WAGON`]'s deck, both brakes held, for [`PARKED`] ticks. `still` takes
/// the wagon's motion away from the wheels — the port before S136, where a braked tire held on to
/// the world. `drag` keeps the bike's air drag, which reads the bike's speed over the world, wagon
/// or no wagon.
fn parked(still: bool, drag: bool) -> Option<Parked> {
    let ee = state()?;
    let parts = archives()?;
    let scn = member(&parts, "free06.scn").expect("free06.scn");
    let scene = Scene::parse(&scn).expect("free06's scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let terrain_bytes = member(&parts, &leaf).expect("free06's terrain");
    let terrain = Terrain::parse(&terrain_bytes).expect("free06's terrain");
    let mut world = Place { ground: Ground::new(&terrain, &scene.terrain), props: Props::default() };
    let mut traffic = traffic_of(&ee, &parts, &mut world.props);
    let first = traffic.first.expect("attached");
    traffic.frame(DT, &mut world);
    let moving = world.props.props[first + WAGON].moving.expect("the wagon moves");

    // On the roof's middle, facing the way the wagon goes, at its speed: the bike's origin is the
    // stock spawn's height over the ground it stands on.
    let mut sim = game_sim::Sim::new(0x5136);
    if !drag {
        std::sync::Arc::make_mut(&mut sim.phy).mv.drag = F::ZERO;
    }
    let deck = Vf::new(moving.pose.0[3].x(), ROOF + game_sim::stock::SPAWN.y(), moving.pose.0[3].z(), 1.0);
    sim.rider.ride.place(deck, moving.pose.0[2]);
    sim.rider.ride.chassis.vel = Vf::new(moving.velocity.x(), moving.velocity.y(), moving.velocity.z(), 0.0);
    // Where the bike is in the wagon's own frame.
    let off = |sim: &game_sim::Sim, car: &Mat4| {
        let at = sim.rider.ride.position();
        let d = Vf::new(at.x() - car.0[3].x(), at.y() - car.0[3].y(), at.z() - car.0[3].z(), 0.0);
        chassis::unrotate(&Mat3::new(car.0[0], car.0[1], car.0[2]), d)
    };
    let held = ride::Controls { brake: F::ONE, front_brake: F::ONE, rear_brake: F::ONE, ..Default::default() };
    let (mut start, mut along, mut up, mut on) = (None, 0.0f32, 0.0f32, PARKED);
    for tick in 0..PARKED {
        // The production order: the cars first, then the rider (`Sim::step`). Here the test steps the
        // cars itself, so that the control can take their motion away in between.
        traffic.frame(DT, &mut world);
        if still {
            world.props.props.iter_mut().for_each(|p| p.moving = None);
        }
        sim.step(DT, &held, &mut world);
        let now = off(&sim, &traffic.cars[WAGON].pose);
        if on == PARKED && (now.x().abs().max(now.z().abs()) > DECK || now.y() < 0.0) {
            on = tick;
        }
        if tick == SETTLE {
            start = Some(now);
        }
        if let Some(start) = start {
            along = along.max((now.x() - start.x()).abs()).max((now.z() - start.z()).abs());
            up = up.max((now.y() - start.y()).abs());
        }
    }
    let v = sim.rider.ride.chassis.vel;
    let car = moving.velocity;
    Some(Parked {
        along,
        up,
        on,
        speed: (v.x() * v.x() + v.z() * v.z()).sqrt(),
        car: (car.x() * car.x() + car.z() * car.z()).sqrt(),
    })
}

/// S136's test: a bike parked on the moving train's deck moves with it and stays on it — its tires
/// hold on to the wagon, not to the world under it. The brief's comparison is against a capture of
/// the original doing it (S129's), which does not exist yet: this proves the port's bike rides the
/// wagon, and the controls prove it is the wagon's motion in the wheels' contact that does it.
///
/// With no air drag the bike is carried exactly: 0.08 ft along the deck in five seconds. With the
/// stock bike's drag — the original's too, against the bike's speed over the *world* — 55 ft/s of
/// head wind out-pulls both braked tires, the bike slips back at 8 ft/s and goes off the wagon's back
/// in three seconds rather than one.
///
/// S146 compared against the original: its 250 braked on a wagon's roof creeps back about 0.2 ft/s and
/// stays on, drag and all, and the port's replay of it agrees bit for bit (tests/replay.rs
/// `a_bike_parked_on_the_trains_roof_rides_it_as_the_original_does`). The 8 ft/s slip here is this
/// stock bike's and these controls', not a carrying fault.
#[test]
fn a_bike_parked_on_the_train_rides_it() {
    let Some(carried) = parked(false, false) else { return };
    assert!(carried.along < 0.5, "the bike moved {} ft along the wagon's deck", carried.along);
    assert!(carried.up < 0.1, "the bike rose or sank {} ft off the deck", carried.up);
    assert_eq!(carried.on, PARKED, "the bike left the deck: {carried:?}");
    assert!((carried.speed - carried.car).abs() < 0.5, "the bike is at {} ft/s on a wagon at {}", carried.speed, carried.car);

    // The control: the same, with the wagon's motion kept from the wheels. The braked tires hold on
    // to the world and the wagon runs out from under the bike.
    let left = parked(true, false).expect("the same state");
    assert!(left.on < 45, "without the wagon's motion the bike still stayed on for {} ticks", left.on);
    assert!(left.speed < carried.car - 10.0, "without the wagon's motion the bike still ran at {} ft/s", left.speed);

    // The stock bike, drag and all: it slips, but it is the wagon its tires brake against.
    let dragged = parked(false, true).expect("the same state");
    let control = parked(true, true).expect("the same state");
    assert!(dragged.on > 75 && dragged.on > 2 * control.on, "with its drag the bike stayed on {} ticks, {} without the wagon", dragged.on, control.on);
}

/// What [`thrown`] measured: the wreck bike's slip along [`WAGON`]'s deck and its height over the
/// roof, both in the wagon's own frame, and how many of its contacts named the wagon as their mover.
#[derive(Debug)]
struct Thrown {
    along: f32,
    over: f32,
    on_mover: usize,
}

/// The wreck bike (`bike\wreckbike.phy`, the thrown machine) let go a foot over [`WAGON`]'s roof at
/// the wagon's own velocity, and ticked among the world's props for [`PARKED`] ticks, the cars first
/// as `Sim::step` has them. `still` takes the cars' motion away from the search, the port before
/// S145, where a body met a car as it meets a wall.
fn thrown(still: bool) -> Option<Thrown> {
    use game_sim::contact_search::Search;
    let ee = state()?;
    let parts = archives()?;
    let part = game_sim::phy::Part::read_bytes(&member(&parts, "bike\\wreckbike.phy").expect("the wreck bike's file"), "PhysicsPart1");
    let scn = member(&parts, "free06.scn").expect("free06.scn");
    let scene = Scene::parse(&scn).expect("free06's scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let terrain_bytes = member(&parts, &leaf).expect("free06's terrain");
    let terrain = Terrain::parse(&terrain_bytes).expect("free06's terrain");
    let mut world = Place { ground: Ground::new(&terrain, &scene.terrain), props: Props::default() };
    let mut traffic = traffic_of(&ee, &parts, &mut world.props);
    let first = traffic.first.expect("attached");
    traffic.frame(DT, &mut world);
    let car = world.props.props[first + WAGON].moving.expect("the wagon moves");
    let at = Vf::new(car.pose.0[3].x(), ROOF + 1.0, car.pose.0[3].z(), 1.0);
    let mut bike = game_sim::wreck_bike::WreckBike::new(&part, ride::WRECK_INERTIA, at, Vf::new(0.0, 0.0, 0.0, 1.0));
    bike.body.velocity = Vf::new(car.velocity.x(), car.velocity.y(), car.velocity.z(), 0.0);
    let (mut along, mut over, mut on_mover) = (0.0f32, f32::MIN, 0);
    for tick in 0..PARKED {
        traffic.frame(DT, &mut world);
        if still {
            world.props.props.iter_mut().for_each(|p| p.moving = None);
        }
        let rays = game_sim::wreck_bike::ray_tree();
        let mut nearby = game_sim::flight_search::search();
        let mut scene = game_sim::flight_search::Scene::wooded(Some(&world.props), None, None, None, &rays, &bike.record.node, &mut nearby);
        if !still {
            // The scene's movers are every prop's, by index, and a car's is its own motion.
            let movers = scene.movers();
            let moving = world.props.props[first + WAGON].moving.expect("the wagon moves");
            assert_eq!(movers.len(), world.props.props.len(), "one mover a prop");
            assert_eq!(movers[first + WAGON], moving.mover(), "the wagon's mover");
            let struck = game_sim::contact_search::Struck { object: game_sim::flight_search::FIRST + first + WAGON, other: None, mover: Some((first + WAGON, 2)) };
            assert_eq!(scene.velocity(&struck).0.map(f32::to_bits), moving.velocity.0.map(f32::to_bits), "the wagon's velocity");
        }
        bike.tick(DT, None, &mut scene);
        on_mover += bike.record.contacts.iter().filter(|c| c.mover == Some(first + WAGON)).count();
        let pose = traffic.cars[WAGON].pose;
        let p = bike.body.position;
        let d = Vf::new(p.x() - pose.0[3].x(), p.y() - pose.0[3].y(), p.z() - pose.0[3].z(), 0.0);
        let local = chassis::unrotate(&Mat3::new(pose.0[0], pose.0[1], pose.0[2]), d);
        if tick >= SETTLE {
            along = along.max(local.z().abs()).max(local.x().abs());
            over = over.max(p.y() - ROOF);
        }
    }
    Some(Thrown { along, over, on_mover })
}

/// S145's test: a body's contacts ride their platform. The wreck bike dropped on the moving
/// wagon's roof lands on it, its contacts name the wagon as their mover (`+0x78`), and the response
/// meets the wagon's motion, so it is carried; the control, the same with the wagon's motion kept
/// from the search, has the roof braking the bike against the world and the wagon runs out from
/// under it.
#[test]
fn a_wreck_bike_dropped_on_the_train_rides_it() {
    let Some(carried) = thrown(false) else { return };
    eprintln!("carried {carried:?}");
    assert!(carried.on_mover > 0, "no contact named the wagon: {carried:?}");
    let left = thrown(true).expect("the same state");
    eprintln!("still {left:?}");
    assert_eq!(left.on_mover, 0, "a still car is no mover");
    // Measured: 1.18 ft of slip and 1.36 ft at most over the roof while carried; 212 ft (off the
    // wagon's back) without its motion.
    assert!(carried.along < 3.0, "the carried bike slid {} ft along the deck", carried.along);
    assert!(0.0 < carried.over && carried.over < 3.0, "the carried bike is {} ft over the roof", carried.over);
    assert!(left.along > DECK, "without the wagon's motion the bike still stayed on: {left:?}");
}

// --- S141: the plane and the train ------------------------------------------------------------------

/// biplane.p2m2's state: free06, the plane flown, the train and the earth movers on their clips.
const PLANE_STATE: &str = "biplane.p2m2_SaveState.eeMemory.bin";
/// The plane, and its collision record's ray probe (S48's offsets).
const PLANE: u32 = 0x00c4_4a40;
const PLANE_COLLISION: u32 = 0x170;
const RAY_PROBE: u32 = 0x1f0;

/// A collidable's fields, and the broad phase's (S44, S45, S96).
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

/// What a search leaves on a collidable's flags (its two marks) and the two bits it never reads.
const MARKS: u8 = 0xf0;

/// **The plane's world holds the cars as the original's does** (S141). biplane.p2m2's state is the
/// plane's world on free06: the first 21 slots of its moving list are the path cars' mesh
/// collidables in free06.toy's order (the order a ride's load attaches the cars in), each of category
/// 0x48 with its car (path car vtable, vehicle kind 2) as its mover and no collision object, then the
/// two driving toys, then the plane's own. The port's [`flight_search::world_flown`] lists its cars
/// in its props' order, all ahead of its two probes, each with the state's category, flags and (by
/// its model) radius; its scenery world ([`flight_search::world_wooded`]) still files them in the
/// tree.
#[test]
fn the_planes_world_holds_the_cars_as_the_originals_does() {
    use game_sim::flight_search::{self, FIRST, RAY, SWEEP, TOY};
    use game_sim::broad_phase::Structure;
    let Some(along) = state() else { return };
    let Some(kit) = Kit::load().ok() else { return };
    let path = kit.ctx("states").join(PLANE_STATE);
    if !path.exists() {
        skip(&format!("{} is not there: `./kit state ram` of biplane.p2m2_SaveState.p2s", path.display()));
        return;
    }
    let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));

    // The state's moving list, and its path cars.
    let ray = ee.u32_at(ee.u32_at(PLANE + PLANE_COLLISION) + RAY_PROBE);
    let list = ee.u32_at(ee.u32_at(ee.u32_at(ray + col_at::SPACE) + col_at::TABLE) + col_at::MOVING);
    let block = ee.u32_at(ee.u32_at(list + col_at::LIST_BLOCKS));
    let last = ee.i32_at(list + col_at::LIST_LAST) as u32;
    let slots: Vec<u32> = (0..last).map(|k| ee.u32_at(block + 4 * k)).collect();
    let path_car = ee.u32_at(CARS[0] + at::VTABLE).max(along.u32_at(CARS[0] + at::VTABLE));
    let cars: Vec<u32> = slots.iter().copied().take_while(|&o| o & 2 == 0 && {
        let m = ee.u32_at(o + col_at::MOVER);
        m != 0 && ee.u32_at(m + at::VTABLE) == path_car
    }).collect();
    assert_eq!(cars.len(), CARS.len(), "the moving list's first slots are the path cars");

    let Some((world, traffic)) = free06_with_cars() else { return };
    let first = traffic.first.expect("attached");
    let props = &world.props.props;
    let (flown, objects) = flight_search::world_flown(props, None, None, None, Vf::new(0.0, 0.0, 0.0, 1.0), F::ZERO);
    let Structure::List(moving) = &flown.structures[1] else { panic!("the moving list") };
    let listed: Vec<usize> = moving.blocks.as_ref().expect("a block")[0].iter().take(moving.last as usize).map(|o| o.expect("a slot")).collect();
    let toys: Vec<usize> = (0..CARS.len()).map(|k| FIRST + first + k).collect();
    assert_eq!(listed, [&toys[..], &[RAY, SWEEP]].concat(), "the port's moving list: the cars, then the probes");

    // The original's: the `.toy`'s cars in its order, which is the order a ride's load attaches them in.
    let models = toy_models(&archives().expect("free06's archives"));
    for (k, &o) in cars.iter().enumerate() {
        let theirs = ee.u32_at(o + col_at::MOVER);
        assert_eq!(name(&ee, theirs), models[k], "slot {k}: the .toy's Car{}", k + 1);
        assert_eq!(ee.u32_at(theirs + at::KIND), 2, "slot {k}: the car's vehicle kind");
        assert_eq!(ee.u32_at(o + col_at::OWNER), 0, "slot {k}: no collision object");
        assert_eq!(ee.u32_at(o + col_at::CATEGORY), TOY, "slot {k}: the category");
    }
    // s89-along's cars are in their addresses' order, not the `.toy`'s: each matches the state's by its
    // model.
    for (k, &car) in CARS.iter().enumerate() {
        let model = name(&along, car);
        let o = cars[models.iter().position(|m| *m == model).unwrap_or_else(|| panic!("car {k}: {model} in the .toy"))];
        let ours = &objects[FIRST + first + k];
        assert_eq!(ours.category, TOY, "car {k}: the category");
        assert_eq!(ee.u8_at(o + col_at::FLAGS) & !MARKS, ours.flags, "car {k}: the flags");
        assert_eq!(ee.f32_at(o + col_at::RADIUS).to_bits(), ours.radius.0.to_bits(), "car {k} ({model}): the radius");
    }

    // The worlds the cars are not toys in yet: the tree's, scenery.
    let (_, scenery) = flight_search::world_wooded(props, None, None, None, Vf::new(0.0, 0.0, 0.0, 1.0), F::ZERO);
    assert_eq!(scenery[FIRST + first].category, flight_search::SCENERY, "a car in the scenery world");
}

/// free06.toy's cars' models in its order, `Car1` on, leaving out the `HasPhysics` vehicles.
fn toy_models(parts: &[(Vec<u8>, Res)]) -> Vec<String> {
    let text = String::from_utf8_lossy(&member(parts, "free06.toy").expect("free06.toy")).into_owned();
    let mut cars: Vec<(usize, String, bool)> = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(n) = line.strip_prefix("[Car").and_then(|l| l.strip_suffix(']')).and_then(|n| n.parse().ok()) {
            cars.push((n, String::new(), false));
        } else if let (Some(car), Some((key, value))) = (cars.last_mut(), line.split_once('=')) {
            match key.trim() {
                "FileName" => car.1 = value.trim().to_string(),
                "HasPhysics" => car.2 = value.trim().eq_ignore_ascii_case("true"),
                _ => {}
            }
        }
    }
    cars.sort_by_key(|c| c.0);
    cars.into_iter().filter(|c| !c.2).map(|c| c.1).collect()
}

/// The machines' archive's member `name`, out of the disc's `MX.PSR`.
fn machine(name: &str) -> Option<Vec<u8>> {
    let kit = Kit::load().ok()?;
    let mut file = std::fs::File::open(kit.ctx("iso").join("MX.PSR")).ok()?;
    let mut front = vec![0u8; 64 << 10];
    std::io::Read::read_exact(&mut file, &mut front).expect("the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find("machine.res").expect("machine.res is in MX.PSR");
    std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    std::io::Read::read_exact(&mut file, &mut bytes).expect("read");
    let res = Res::parse(&bytes).expect("machine.res");
    let e = res.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name))?;
    res.read(&bytes, e).ok()
}

/// The biplane off the disc, with its ray probe: the plane a ride flies.
fn biplane() -> Option<game_sim::flight::Flight> {
    use game_sim::flight_search::{BIPLANE_RAYS, Rays};
    let plane = game_sim::phy::Plane::read_bytes(&machine(game_sim::phy::BIPLANE_PHY)?);
    let rays = game_sim::track::Rays::parse(&machine(BIPLANE_RAYS)?).expect("biplanem.col");
    let mut f = game_sim::flight::Flight::new(&plane).with_rays(Rays::of(&rays));
    f.attitude.flown = true;
    Some(f)
}

/// free06 with s89-along's cars on it, stepped once so that every car has its motion.
fn free06_with_cars() -> Option<(Place<'static>, Traffic)> {
    let ee = state()?;
    let parts = archives()?;
    let scn = member(&parts, "free06.scn").expect("free06.scn");
    let scene = Box::leak(Box::new(Scene::parse(&scn).expect("free06's scene")));
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let bytes: &'static [u8] = Box::leak(member(&parts, &leaf).expect("free06's terrain").into_boxed_slice());
    let terrain = Box::leak(Box::new(Terrain::parse(bytes).expect("free06's terrain")));
    let mut world = Place { ground: Ground::new(terrain, &scene.terrain), props: Props::default() };
    let mut traffic = traffic_of(&ee, &parts, &mut world.props);
    traffic.frame(DT, &mut world);
    Some((world, traffic))
}

/// How long the plane rides the wagon: five seconds.
const FLOWN: usize = 150;

/// What [`set_down`] measured.
#[derive(Debug)]
struct SetDown {
    /// The most the plane moved along and across the deck, and up or down off it, in the wagon's
    /// own frame, after [`SETTLE`].
    along: f32,
    up: f32,
    /// The ticks before it left the deck ([`FLOWN`] if it never did), and before it first wrecked.
    on: usize,
    wrecked: Option<usize>,
    /// Whether its rays were on a car with the car as the contacts' mover, on every tick after
    /// [`SETTLE`] it was on the deck.
    on_the_car: bool,
    /// Its speed at the end, and the wagon's.
    speed: f32,
    car: f32,
    /// Every tick's body position and velocity, bit for bit.
    trace: Vec<[u32; 8]>,
}

/// The plane set down, throttle shut, a foot over [`WAGON`]'s roof, facing the way the wagon goes and
/// at its speed, flown for [`FLOWN`] ticks — cars first, then the plane, as `Sim::step` has them.
/// `still` takes the cars' motion away from the plane's contacts, which then hold on to the world.
/// `rollback` keeps the plane and the cars at that tick and, at the end, puts them back the way an
/// online rollback's restore does (the plane through the snapshot's wire, which leaves the movers
/// out; the cars' meshes stood again, their motions left as the later tick wrote them) and flies the
/// ticks after it again.
fn set_down(still: bool, drag: bool, rollback: Option<usize>) -> Option<SetDown> {
    let (mut world, mut traffic) = free06_with_cars()?;
    let first = traffic.first.expect("attached");
    let tables = game_sim::ride::Tables::build();
    let mut f = biplane()?;
    if !drag {
        (f.aero.drag0, f.aero.drag1, f.aero.drag_de) = (F::ZERO, F::ZERO, F::ZERO);
    }
    let moving = world.props.props[first + WAGON].moving.expect("the wagon moves");
    let lowest = f.tires.iter().map(|t| F(t.center.y()) - t.radius).fold(F(f32::MAX), |a, b| a.min(b));
    let deck = Vf::new(moving.pose.0[3].x(), ROOF - lowest.to_f32() + 1.0, moving.pose.0[3].z(), 1.0);
    f.launch(deck, moving.pose.0[2], moving.velocity, &tables.acos);
    let off = |f: &game_sim::flight::Flight, car: &Mat4| {
        let at = f.body.position;
        let d = Vf::new(at.x() - car.0[3].x(), at.y() - car.0[3].y(), at.z() - car.0[3].z(), 0.0);
        chassis::unrotate(&Mat3::new(car.0[0], car.0[1], car.0[2]), d)
    };
    let mut out = SetDown { along: 0.0, up: 0.0, on: FLOWN, wrecked: None, on_the_car: true, speed: 0.0, car: 0.0, trace: Vec::new() };
    let mut start = None;
    let mut kept = None;
    let mut rolled = false;
    let mut tick = 0;
    while tick < FLOWN {
        if rollback == Some(tick) && !rolled && kept.is_none() {
            kept = Some((f.clone(), traffic.cars.clone()));
        }
        traffic.frame(DT, &mut world);
        if still {
            for p in world.props.props.iter_mut() {
                if let Some(m) = &mut p.moving {
                    (m.velocity, m.spin) = (Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 0.0, 1.0));
                }
            }
        }
        f.frame(DT, Default::default(), &mut world, &tables);
        let b = &f.body;
        let row = [b.position.to_bits(), b.velocity.to_bits()].concat();
        if out.trace.len() > tick {
            out.trace[tick] = row.try_into().unwrap();
        } else {
            out.trace.push(row.try_into().unwrap());
        }
        let now = off(&f, &traffic.cars[WAGON].pose);
        if out.on == FLOWN && (now.x().abs().max(now.z().abs()) > DECK || now.y() < -2.0) {
            out.on = tick;
        }
        if out.wrecked.is_none() && f.crash.reason != 0 {
            out.wrecked = Some(tick);
        }
        if tick == SETTLE {
            start = Some(now);
        }
        if let Some(start) = start.filter(|_| out.on == FLOWN) {
            out.along = out.along.max((now.x() - start.x()).abs()).max((now.z() - start.z()).abs());
            out.up = out.up.max((now.y() - start.y()).abs());
            out.on_the_car &= f.record.contacts.iter().any(|c| c.mover.is_some());
        }
        tick += 1;
        if tick == FLOWN
            && let Some((plane, cars)) = kept.take()
        {
            let wire = game_sim::snapshot::encode_flights(&[Some(Box::new(plane))]);
            let mut back = game_sim::snapshot::decode_flights(&wire).expect("the plane's wire");
            let mut restored = *back.pop().flatten().expect("the plane");
            // As `snapshot::decode` does: the probe is this machine's, not the wire's.
            (restored.rays, restored.bike) = (f.rays.clone(), f.bike.clone());
            f = restored;
            traffic.cars = cars;
            traffic.place(&mut world);
            tick = rollback.expect("kept at the rollback's tick");
            rolled = true;
            start = None;
        }
    }
    let v = f.body.velocity;
    let car = world.props.props[first + WAGON].moving.expect("the wagon moves").velocity;
    out.speed = (v.x() * v.x() + v.z() * v.z()).sqrt();
    out.car = (car.x() * car.x() + car.z() * car.z()).sqrt();
    Some(out)
}

/// **A plane set down on the moving train's roof stays on it and moves with it** (S141). The plane's
/// wheels ask the terrain alone (the original's tire probe is the world's ground query), so on a
/// wagon it is the collision object's rays that hold it up: their hits on the car's mesh carry the
/// car as their mover, and the resolution's spring and friction work against the car's motion at
/// the contact ([`game_sim::contact::resolve`], S42's mover branch). At the wagon's 55 ft/s, under
/// MinWreckSpeed's 73, the contact does not wreck it.
///
/// The control takes the cars' motion from the contacts: the plane's rays then hold on to the
/// world, and the wagon runs out from under it. And a rollback in the middle of the ride, which
/// restores the plane without its movers, flies the rest of it bit for bit as it was flown.
#[test]
fn a_plane_set_down_on_the_train_rides_it() {
    let quiet = |s: &SetDown| format!("{:?}", SetDown { trace: Vec::new(), ..*s });
    let Some(carried) = set_down(false, false, None) else { return };
    eprintln!("carried, drag off: {}", quiet(&carried));
    assert_eq!(carried.wrecked, None, "the plane wrecked on the wagon: {}", quiet(&carried));
    assert_eq!(carried.on, FLOWN, "the plane left the deck: {}", quiet(&carried));
    assert!(carried.on_the_car, "the plane's contacts were not on the car");
    assert!(carried.along < 0.25, "the plane moved {} ft along the wagon's deck", carried.along);
    assert!(carried.up < 0.25, "the plane rose or sank {} ft off the deck", carried.up);
    assert!((carried.speed - carried.car).abs() < 0.05, "the plane is at {} ft/s on a wagon at {}", carried.speed, carried.car);

    let left = set_down(true, false, None).expect("the same state");
    eprintln!("left, drag off: {}", quiet(&left));
    assert!(left.on < FLOWN / 2, "without the wagon's motion the plane still stayed on for {} ticks", left.on);

    // The plane's own drag reads its speed over the air, as the original's does: it slips back a
    // little, and still rides.
    let dragged = set_down(false, true, None).expect("the same state");
    eprintln!("carried, drag on: {}", quiet(&dragged));
    assert_eq!((dragged.wrecked, dragged.on), (None, FLOWN), "the plane with its drag: {}", quiet(&dragged));
    assert!(dragged.along < 4.0 && (dragged.speed - dragged.car).abs() < 1.0, "the plane with its drag: {}", quiet(&dragged));

    let again = set_down(false, true, Some(FLOWN / 2)).expect("the same state");
    assert_eq!(again.trace, dragged.trace, "the ride after the rollback is not the ride");
}

/// The plane flown level at 120 ft/s, gas on, from 60 ft abeam of [`WAGON`] straight at its side, a
/// few feet under its roof, for a second and a half: the tick it wrecked and why, the mesh its record struck,
/// and whether any of its contacts on the way had a car as its mover. With `cars` off it is the
/// same flight over free06 with no cars on it.
fn flown_into(cars: bool) -> Option<(Option<(usize, u32)>, Option<usize>, bool, usize)> {
    let (mut world, traffic) = free06_with_cars()?;
    let first = traffic.first.expect("attached");
    let moving = world.props.props[first + WAGON].moving.expect("the wagon moves");
    let mut traffic = cars.then_some(traffic);
    if !cars {
        world.props.props.clear();
    }
    let tables = game_sim::ride::Tables::build();
    let mut f = biplane()?;
    let (side, centre) = (moving.pose.0[0], moving.pose.0[3]);
    let start = Vf::new(centre.x() + 60.0 * side.x(), ROOF - 6.0, centre.z() + 60.0 * side.z(), 1.0);
    let facing = Vf::new(-side.x(), 0.0, -side.z(), 0.0);
    f.launch(start, facing, Vf::new(-120.0 * side.x(), 0.0, -120.0 * side.z(), 1.0), &tables.acos);
    let gas = game_sim::biplane::Pad { forward: F::ONE, ..Default::default() };
    let (mut wrecked, mut met) = (None, false);
    for tick in 0..45 {
        if let Some(t) = &mut traffic {
            t.frame(DT, &mut world);
        }
        f.frame(DT, gas, &mut world, &tables);
        met |= f.record.contacts.iter().any(|c| c.mover.is_some());
        if f.crash.reason != 0 {
            wrecked = Some((tick, f.crash.reason));
            break;
        }
    }
    Some((wrecked, f.record.struck, met, first))
}

/// **A plane flown into the train's side meets it** (S141). The cars are in the plane's world as
/// toys, so its rays strike a wagon's mesh, the contacts carry the car, and the crash test wrecks
/// it: a toy's category is not the vehicle bit that spares a plane striking a bike. Over the same
/// ground with no cars on it the same flight flies on.
#[test]
fn a_plane_flown_into_the_trains_side_meets_it() {
    let Some((wrecked, struck, met, first)) = flown_into(true) else { return };
    eprintln!("into the train: wrecked {wrecked:?}, struck {struck:?}, on a car {met}");
    let cars = game_sim::flight_search::FIRST + first..game_sim::flight_search::FIRST + first + CARS.len();
    assert_eq!(wrecked.map(|w| w.1), Some(game_sim::biplane::CRASHED), "the plane flew through the train");
    assert!(struck.is_some_and(|s| cars.contains(&s)), "the plane struck {struck:?}, not a car ({cars:?})");
    assert!(met, "no contact had a car as its mover");

    let (open, ..) = flown_into(false).expect("the same state");
    assert_eq!(open, None, "with no cars the plane still wrecked");
}

// --- S146: the cars from a ride's save state ---------------------------------------------------------

/// The curve's cached tangents (`+0x510`, `+0x520`) and its "tangents dirty" word (`+0x4fc`, research/
/// oracle.txt S128): the curve makes the tangents again only when the word is set, which a cursor move
/// does, and the next evaluation clears it.
const TANGENTS: [u32; 2] = [0x510, 0x520];
const TANGENTS_DIRTY: u32 = 0x4fc;

/// Whether a car caught in a RAM image still rides [`Spec::measured`] — the port's [`Car::stale`]: the
/// tangents it has cached are the measuring's, bit for bit. The dirty word cannot say so: it is clear
/// both on a car that has not moved since its load and on one whose cursor has moved since and whose
/// tangents were made again for its own segment.
fn stale_of(ee: &Ee, car: u32, spec: &Spec) -> bool {
    TANGENTS.iter().zip(&spec.measured).all(|(&o, m)| bits(vf(ee, car + o))[..3] == bits(*m)[..3])
}

/// A save state of the ride, out of the slot's `context/states`.
fn image(file: &str) -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("states").join(file);
    if !path.exists() {
        skip(&format!("{} is not there", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

/// How many ticks [`the_cars_drive_on_from_a_rides_state`] runs each image.
const RIDE_TICKS: usize = 90;

/// **S146: the cars seeded from a ride's save state drive on as the original's.** S129's and S146's
/// images, each with its 21 cars seeded as the replays seed them ([`stale_of`]), stepped by the port
/// and by the original's `path_car_update` on the same RAM for [`RIDE_TICKS`] ticks: every car's world
/// matrix, bit for bit. Seeding [`Car::stale`] from the dirty word being clear instead (S129's first
/// replay) put every moving car on the measuring's tangents: S146's wagon swung 60° off its rails on
/// the first tick and threw the bike parked on its roof.
#[test]
fn the_cars_drive_on_from_a_rides_state() {
    let Some(parts) = archives() else { return };
    let scn = member(&parts, "free06.scn").expect("free06.scn");
    let scene = Scene::parse(&scn).expect("free06's scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let terrain_bytes = member(&parts, &leaf).expect("free06's terrain");
    let terrain = Terrain::parse(&terrain_bytes).expect("free06's terrain");
    let update = addr("path_car_update").unwrap();
    let mut ran = 0;
    for file in ["s146-roof.eeMemory.bin", "s129-across.eeMemory.bin", "s129-braked.eeMemory.bin", STATE] {
        let Some(mut ee) = image(file) else { continue };
        let specs: Vec<Spec> = CARS
            .iter()
            .map(|&car| {
                let col = name(&ee, car).to_ascii_lowercase().replace(".psg", ".col");
                spec_of(&ee, car, member(&parts, &col).map(|b| Mesh::parse(&b).unwrap()))
            })
            .collect();
        // Whether the dirty word's rule would have seeded the same: it does not on any car that has
        // left its first segment.
        let dirty_rule: Vec<bool> = CARS.iter().map(|&car| ee.u32_at(car + TANGENTS_DIRTY) == 0).collect();
        let stale: Vec<bool> = CARS.iter().zip(&specs).map(|(&car, spec)| stale_of(&ee, car, spec)).collect();
        println!("{file}: stale {stale:?}, the dirty word's rule {dirty_rule:?}");
        let mut world = Place { ground: Ground::new(&terrain, &scene.terrain), props: Props::default() };
        let mut traffic = Traffic::new(specs);
        traffic.cars = CARS.iter().zip(&stale).map(|(&car, &stale)| Car { stale, ..car_of(&ee, car) }).collect();
        // The control: the dirty word's rule, one tick on, has cars off the original's place.
        let mut control = traffic.clone();
        control.cars.iter_mut().zip(&dirty_rule).for_each(|(c, &d)| c.stale = d);
        let mut other = Place { ground: Ground::new(&terrain, &scene.terrain), props: Props::default() };
        control.frame(DT, &mut other);
        let mut ours = traffic.clone();
        ours.frame(DT, &mut other);
        let off = (0..CARS.len()).filter(|&k| mat_bits(&control.cars[k].pose) != mat_bits(&ours.cars[k].pose)).count();
        println!("{file}: {off} cars off their place a tick on with the dirty word's rule");
        assert!(off > 0, "{file}: the dirty word's rule seeds the same cars");
        traffic.attach(&mut world.props);
        ee.set_f32(app(&ee) + at::INV_DT, (F::ONE / DT).0);
        for tick in 0..RIDE_TICKS {
            for &car in &CARS {
                ee.call(update, &[u64::from(car)], &[DT.0], BUDGET);
            }
            traffic.frame(DT, &mut world);
            for (k, &car) in CARS.iter().enumerate() {
                let node = ee.u32_at(car + at::NODE);
                let what = format!("{file} tick {tick}: {} ({car:#x})", name(&ee, car));
                assert_eq!(mat_bits(&traffic.cars[k].pose), mat_bits(&mat(&ee, node + at::WORLD)), "{what}: world matrix");
                assert_eq!(traffic.cars[k].seg, ee.u32_at(car + at::CURSOR), "{what}: cursor");
            }
        }
        ran += 1;
    }
    println!("{ran} images driven");
}

// --- S151: the riding bike's body rays name the car as their mover ----------------------------------

/// The riding bike's body rays (`BikeCol::rays`, the disc's `Bike\Col\bike` files) against free06's
/// cars from beside [`WAGON`], facing its side, `off` feet out from its middle along its side row:
/// the car search's list ([`game_sim::mover_contact::struck`]). `still` takes the cars' motion away.
fn beside(off: f32, still: bool) -> Option<(Vec<game_sim::contact::Contact>, usize, Vec<game_sim::contact::Mover>, Moving)> {
    let ee = state()?;
    let parts = archives()?;
    let file = |name: String| member(&parts, &name).unwrap_or_else(|| panic!("{name} is not in free06's archives"));
    let col = game_sim::bike_col::BikeCol::load(&file(game_sim::bike_col::rays_file(game_sim::bike_col::MODEL)), &file(game_sim::bike_col::shape_file(game_sim::bike_col::MODEL)))
        .expect("the bike's collision files");
    let scn = member(&parts, "free06.scn").expect("free06.scn");
    let scene = Scene::parse(&scn).expect("free06's scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let terrain_bytes = member(&parts, &leaf).expect("free06's terrain");
    let terrain = Terrain::parse(&terrain_bytes).expect("free06's terrain");
    let mut world = Place { ground: Ground::new(&terrain, &scene.terrain), props: Props::default() };
    let mut traffic = traffic_of(&ee, &parts, &mut world.props);
    let first = traffic.first.expect("attached");
    traffic.frame(DT, &mut world);
    traffic.frame(DT, &mut world);
    let moving = world.props.props[first + WAGON].moving.expect("the wagon moves");
    if still {
        world.props.props.iter_mut().for_each(|p| p.moving = None);
    }
    let pose = moving.pose.0;
    let side = pose[0];
    let at = Vf::new(pose[3].x() + side.x() * off, pose[3].y() + game_sim::stock::SPAWN.y(), pose[3].z() + side.z() * off, 1.0);
    let mut sim = game_sim::Sim::new(0x5151);
    sim.rider.ride.place(at, Vf::new(-side.x(), 0.0, -side.z(), 0.0));
    let cars = &world.props.props[first..];
    let list = game_sim::mover_contact::struck(&col, &sim.rider.ride.transform(), cars, first, DT);
    Some((list, first, game_sim::flight_search::movers(&world.props.props), moving))
}

/// S151: the original's contact search (`collision_record_collide`, its actor loop) writes each own
/// entry's mover (`+0x78`) from the struck collidable's owner (`+0x7c`) when that is a vehicle of
/// kind 2 (`+0x1b0`), else 0 — an argument the decompile drops, read off the disassembly
/// (`research/oracle.txt` S151). Every path car's mesh collidable has the car as that owner, kind 2,
/// and no collision object (S141's test, and the S151 journal on four ride images), so the original's
/// bike struck by a car carries the car's motion in its response, and the wreck test (which reads
/// `+0x74` alone) still calls it scenery. The port's car search names the same: every entry beside
/// the moving wagon names a car's prop as its mover, and that prop's mover is the car's own motion;
/// with the cars still, none does.
#[test]
fn the_car_search_names_the_car_as_its_mover() {
    let mut found = None;
    for off in [4.0f32, 5.0, 6.0, 7.0, 8.0, 9.0] {
        let Some((list, first, movers, moving)) = beside(off, false) else { return };
        if !list.is_empty() {
            found = Some((off, list, first, movers, moving));
            break;
        }
    }
    let (off, list, first, movers, moving) = found.expect("no body ray met the wagon's side");
    eprintln!("{off} ft out: {} entries", list.len());
    for c in &list {
        let m = c.mover.unwrap_or_else(|| panic!("an entry on a moving car named no mover: {c:?}"));
        assert!(m >= first, "the mover {m} is not a car");
        assert!(c.other.is_none(), "a car has no collision object");
    }
    assert!(list.iter().any(|c| c.mover == Some(first + WAGON)), "no entry named the wagon: {list:?}");
    assert_eq!(movers[first + WAGON], moving.mover(), "the wagon's mover is its motion");
    assert!(game_sim::mover_contact::wreck_contacts(&list).len() == list.len(), "the wreck test reads a car as scenery");

    let (still, ..) = beside(off, true).expect("the same state");
    assert!(!still.is_empty(), "the still wagon was not struck");
    assert!(still.iter().all(|c| c.mover.is_none()), "a still car named a mover");
}

// --- S171: the wreck bike's world and the put-back's ---------------------------------------------

/// S171's state of free06, the train running: s89-along's ride with the bike teleported into the
/// train's front car, so that a wreck bike is in the world (`./kit state ram` of the slot, saved a few
/// seconds on).
const WRECK_STATE: &str = "s171-thrown.eeMemory.bin";
/// The wreck bike's collision object in it, the one whose probes are placed in the world: its
/// collidable (`+0x1f8`) of the wreck bike's category, its ray probe and sweep probe (`+0x1f0`,
/// `+0x1f4`, S76).
const WRECK_RECORD: u32 = 0x0197_0060;
const WRECK_RAY: u32 = 0x1f0;
const WRECK_SWEEP: u32 = 0x1f4;
const WRECK_COLLIDABLE: u32 = 0x1f8;
/// A probe's leave-out list (mode, array, count) and its category list.
const LEAVE_OUT: u32 = 0xd0;
const CATEGORY_LIST: u32 = 0xe8;

/// A probe's moving list: its placement's table's (`+0x04`, `+0x04`, `+0x38`), as its slots.
fn moving_of(ee: &Ee, probe: u32) -> (u32, Vec<u32>) {
    let list = ee.u32_at(ee.u32_at(ee.u32_at(probe + col_at::SPACE) + col_at::TABLE) + col_at::MOVING);
    let block = ee.u32_at(ee.u32_at(list + col_at::LIST_BLOCKS));
    let last = ee.i32_at(list + col_at::LIST_LAST) as u32;
    (list, (0..last).map(|k| ee.u32_at(block + 4 * k)).collect())
}

/// The models of a moving list's toys at its head (category 0x48, a kind-2 vehicle as mover, no
/// collision object), with each one's flags, marks masked.
fn toys_of(ee: &Ee, slots: &[u32]) -> Vec<(String, u8)> {
    slots
        .iter()
        .take_while(|&&o| o & 2 == 0 && ee.u32_at(o + col_at::CATEGORY) == game_sim::flight_search::TOY)
        .map(|&o| {
            let m = ee.u32_at(o + col_at::MOVER);
            assert_eq!(ee.u32_at(m + at::KIND), 2, "a toy's mover is a vehicle of kind 2");
            assert_eq!(ee.u32_at(o + col_at::OWNER), 0, "a toy has no collision object");
            (name(ee, m), ee.u8_at(o + col_at::FLAGS) & !MARKS)
        })
        .collect()
}

/// **The wreck bike's world holds the cars as the plane's does** (S171). In the original's free06
/// with a wreck bike in the world, both of the wreck bike's probes are placed in the one table whose
/// moving list biplane.p2m2's plane is in: the path cars at its head in the `.toy`'s order, then the
/// two driving toys, each a toy of category 0x48 with its car as mover — the same models, in the same
/// order, with the same flags as biplane.p2m2's. Neither probe leaves the toys out (the ray probe's
/// category list is empty, the sweep's masks 0x8 and 0x800 only), so the wreck bike's search meets
/// the cars as the plane's does: the world [`game_sim::flight_search::world_flown`] builds, which
/// [`game_sim::wreck_bike::WreckBike::tick_among`] now searches.
#[test]
fn the_wreck_bikes_world_holds_the_cars_as_the_planes_does() {
    use game_sim::flight_search::{TOY, WAKE_MASK};
    let Some(kit) = Kit::load().map_err(|w| skip(&w)).ok() else { return };
    let (wreck, plane) = (kit.ctx("states").join(WRECK_STATE), kit.ctx("states").join(PLANE_STATE));
    for p in [&wreck, &plane] {
        if !p.exists() {
            skip(&format!("{} is not there (S171's and S141's free06 states)", p.display()));
            return;
        }
    }
    let ee = Ee::from_ram(&wreck).unwrap_or_else(|e| panic!("{e}"));
    let flown = Ee::from_ram(&plane).unwrap_or_else(|e| panic!("{e}"));

    let collidable = ee.u32_at(WRECK_RECORD + WRECK_COLLIDABLE);
    assert_eq!(ee.u32_at(collidable + col_at::CATEGORY), game_sim::wreck_bike::COLLIDABLE, "the wreck bike's collidable");
    let (ray, sweep) = (ee.u32_at(WRECK_RECORD + WRECK_RAY), ee.u32_at(WRECK_RECORD + WRECK_SWEEP));
    let (list, slots) = moving_of(&ee, ray);
    assert_eq!(moving_of(&ee, sweep).0, list, "both probes are in one table");

    let plane_ray = flown.u32_at(flown.u32_at(PLANE + PLANE_COLLISION) + RAY_PROBE);
    let (plane_list, plane_slots) = moving_of(&flown, plane_ray);
    assert_eq!(list, plane_list, "the wreck bike's moving list is where the plane's is");

    let theirs = toys_of(&ee, &slots);
    assert_eq!(theirs, toys_of(&flown, &plane_slots), "the wreck bike's toys are the plane's, in its order");
    assert_eq!(theirs.len(), CARS.len() + 2, "the path cars and the two driving toys");
    let models = toy_models(&archives().expect("free06's archives"));
    let cars: Vec<&str> = theirs[..CARS.len()].iter().map(|t| t.0.as_str()).collect();
    assert_eq!(cars, models.iter().map(String::as_str).collect::<Vec<_>>(), "the cars in the .toy's order");
    assert!(TOY & WAKE_MASK != 0, "a toy wakes the bike's body");

    // What the probes leave out: none of it a toy's.
    for (probe, cats) in [(ray, &[][..]), (sweep, &[0x8u32, 0x800][..])] {
        let (mode, array, count) = (ee.u32_at(probe + LEAVE_OUT), ee.u32_at(probe + LEAVE_OUT + 4), ee.u32_at(probe + LEAVE_OUT + 8));
        assert_eq!(mode, 1, "the leave-out list is on");
        let left: Vec<u32> = (0..count).map(|k| ee.u32_at(ee.u32_at(array + 4 * k) + col_at::CATEGORY)).collect();
        assert!(!left.contains(&TOY), "a probe leaves a toy out: {left:x?}");
        let (array, count) = (ee.u32_at(probe + CATEGORY_LIST + 4), ee.u32_at(probe + CATEGORY_LIST + 8));
        let listed: Vec<u32> = (0..count).map(|k| ee.u32_at(array + 4 * k)).collect();
        assert_eq!(listed, cats, "the probe's category list");
    }
}

/// How fast the wrecked rider is thrown back along the wagon's roof, over the wagon's own speed.
const SLIDE: f32 = 12.0;

/// What [`slid`] measured.
#[derive(Debug)]
struct Slid {
    /// The ticks before the bike left the deck ([`PARKED`] if it never did).
    on: usize,
    /// How far it slid back along the deck, at most, and its speed over the wagon's at the end.
    back: f32,
    relative: f32,
    /// How many of its contacts named the wagon as their mover.
    on_mover: usize,
}

/// The wreck bike thrown onto [`WAGON`]'s moving roof the way a rider's wreck hands it to the ride:
/// [`game_sim::wreck_bike::WreckBike::tick_among`] among the world's props, the cars stepped first,
/// let go two feet over the roof at the wagon's velocity less [`SLIDE`] ft/s along it and falling at
/// 10 ft/s. `still` takes the cars' motion away: they are then scenery in the tree, as every car was
/// to the wreck bike before S171.
fn slid(still: bool) -> Option<Slid> {
    let (mut world, mut traffic) = free06_with_cars()?;
    let parts = archives()?;
    let part = game_sim::phy::Part::read_bytes(&member(&parts, "bike\\wreckbike.phy").expect("the wreck bike's file"), "PhysicsPart1");
    let first = traffic.first.expect("attached");
    let car = world.props.props[first + WAGON].moving.expect("the wagon moves");
    let v = car.velocity;
    let speed = (v.x() * v.x() + v.z() * v.z()).sqrt();
    let ahead = Vf::new(v.x() / speed, 0.0, v.z() / speed, 0.0);
    let at = Vf::new(car.pose.0[3].x(), ROOF + 2.0, car.pose.0[3].z(), 1.0);
    let mut bike = game_sim::wreck_bike::WreckBike::new(&part, ride::WRECK_INERTIA, at, Vf::new(0.0, 0.0, 0.0, 1.0));
    bike.body.velocity = Vf::new(car.velocity.x() - SLIDE * ahead.x(), -10.0, car.velocity.z() - SLIDE * ahead.z(), 0.0);
    let (mut on, mut back, mut on_mover) = (PARKED, 0.0f32, 0);
    for tick in 0..PARKED {
        traffic.frame(DT, &mut world);
        if still {
            world.props.props.iter_mut().for_each(|p| p.moving = None);
        }
        bike.tick_among(DT, None, Some(&world.props), None);
        on_mover += bike.record.contacts.iter().filter(|c| c.mover == Some(first + WAGON)).count();
        let pose = traffic.cars[WAGON].pose;
        let p = bike.body.position;
        let d = Vf::new(p.x() - pose.0[3].x(), p.y() - pose.0[3].y(), p.z() - pose.0[3].z(), 0.0);
        let local = chassis::unrotate(&Mat3::new(pose.0[0], pose.0[1], pose.0[2]), d);
        back = back.max(local.z().abs()).max(local.x().abs());
        if on == PARKED && (local.z().abs() > DECK || local.x().abs() > DECK || p.y() < ROOF - 3.0) {
            on = tick;
        }
    }
    let car = world.props.props[first + WAGON].moving.map_or(Vf::ZERO, |m| m.velocity);
    let v = bike.body.velocity;
    let relative = ((v.x() - car.x()).powi(2) + (v.y() - car.y()).powi(2) + (v.z() - car.z()).powi(2)).sqrt();
    Some(Slid { on, back, relative, on_mover })
}

/// **A wrecked rider thrown onto the moving train's roof slides with it** (S171): the wreck bike,
/// thrown back along the wagon's roof, is braked by the roof against the wagon's motion — its
/// contacts name the wagon as their mover — and comes to ride with it, still on the deck. With the
/// cars' motion taken away the roof brakes it against the world and the wagon runs out from under it.
#[test]
fn a_wrecked_rider_thrown_onto_the_moving_train_slides_with_it() {
    let Some(carried) = slid(false) else { return };
    eprintln!("carried {carried:?}");
    assert!(carried.on_mover > 0, "no contact named the wagon: {carried:?}");
    assert_eq!(carried.on, PARKED, "the thrown bike left the wagon: {carried:?}");
    assert!(carried.back < DECK, "the thrown bike slid {} ft along the deck", carried.back);
    assert!(carried.relative < 2.0, "the thrown bike ends {} ft/s off the wagon's speed", carried.relative);
    let left = slid(true).expect("the same state");
    eprintln!("still {left:?}");
    assert_eq!(left.on_mover, 0, "a still car is no mover");
    assert!(left.on < PARKED, "without the wagon's motion the thrown bike still stayed on: {left:?}");
}

/// **The put-back's query gathers the cars from the moving list** (S171): its world is the plane's,
/// so a bundle over the wagon's roof gathers every car (the moving list hands them all out wherever
/// they are), ahead of the scenery, and meets the roof; a race's static-only gather lists none of
/// them and meets nothing there.
#[test]
fn the_put_back_gathers_the_cars_from_the_moving_list() {
    use game_sim::back_search::{self, Cache, Race};
    use game_sim::collide::{Bundle, Probe, Record};
    use game_sim::flight_search::FIRST;
    let Some((world, traffic)) = free06_with_cars() else { return };
    let first = traffic.first.expect("attached");
    let pose = traffic.cars[WAGON].pose.0[3];
    let mut bundle = Bundle { count: 1, centre: Vf::new(pose.x(), ROOF + 5.0, pose.z(), 1.0), radius: F(10.0), ..Bundle::default() };
    bundle.probes[0] = Probe { origin: Vf::new(pose.x(), ROOF + 10.0, pose.z(), 1.0), dir: Vf::new(0.0, -1.0, 0.0, 0.0), length: F(10.0) };
    let scene = back_search::Scene { props: &world.props.props, trees: None, field: None };
    let ask = |race: Race<'_>| {
        let (mut cache, mut found) = (Cache::default(), Record::new());
        back_search::query(scene, &bundle, &mut cache, race, &mut found);
        let listed: Vec<usize> = cache.list.nearby.found.iter().map(|c| c.object).collect();
        (listed, found)
    };
    let cars: Vec<usize> = (0..CARS.len()).map(|k| FIRST + first + k).collect();
    let (listed, found) = ask(Race::default());
    assert_eq!(&listed[..cars.len()], &cars[..], "the cars head the list");
    assert!(found.hit[0], "the bundle did not meet the roof");
    assert!((found.point[0].y() - ROOF).abs() < 0.5, "met at y {}, not the roof", found.point[0].y());
    let (listed, found) = ask(Race { static_only: true, checkpoints: None });
    assert!(listed.iter().all(|o| !cars.contains(o)), "a static gather listed a car: {listed:?}");
    assert!(!found.hit[0], "a static gather met the roof");
}
