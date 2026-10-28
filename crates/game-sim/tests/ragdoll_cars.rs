//! The ragdoll's bones against the path cars (S142), on S89's s89-along save state of free06 with the
//! train moving, and the disc's car meshes.
//!
//! 1. **the sphere test** — a bone's sphere collider paired with the state's car collidables, one car
//!    or two in a row on one record, is [`PathCar::touch`] on the port's car, bit for bit: hit or
//!    not, which car holds the record, the plane distance, and the contact's point and normal;
//! 2. **the push-out** — a bone's push-out against a contact with one of the state's cars, the car's
//!    own two virtuals and node answering, is the port's against the car's mover
//!    ([`game_sim::traffic::Moving::mover`]), bit for bit;
//! 3. **the side** — a ragdoll thrown at the moving train's side bounces off it, and goes through
//!    it without the cars in the scene;
//! 4. **the roof** — a ragdoll dropped on a wagon's roof rides it, and slides off the back when the
//!    wagon is in the scene but reported still.

use game_sim::archive::{Psr, Res};
use game_sim::biplane::Body;
use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::chassis::{self, Mat3};
use game_sim::contact::Node;
use game_sim::math::Mat4;
use game_sim::phy::Wrecks;
use game_sim::ragdoll_contact::{Against, Contact, Sphere, moving};
use game_sim::ragdoll_scene::{Candidate, PathCar, Touch, class, sweep_through};
use game_sim::rng::Rng;
use game_sim::track::{Mesh, Scene, Terrain};
use game_sim::traffic::{Car, Moving, Spec, Traffic};
use game_sim::world::{Ground, Place, Props};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

const STATE: &str = "s89-along.eeMemory.bin";
const TRACK: &str = "Tracks\\free06.res";
const BUDGET: u64 = 50_000_000;
const DT: F = F(1.0 / 30.0);

/// The state's 21 path cars (`tests/traffic.rs`): the train's 13, engine first, then the earth movers.
const CARS: [u32; 21] = [
    0x00d5_25f0, 0x00d5_cb60, 0x00d6_3660, 0x00d6_c990, 0x00d7_6440, 0x00d7_fd50, 0x00d8_69e0, 0x00d8_7930,
    0x00d8_bab0, 0x00d8_f670, 0x00d9_3c40, 0x00d9_4820, 0x00d9_5660, 0x01ca_7f60, 0x01cb_bf20, 0x01cc_9300,
    0x01cc_a8d0, 0x01cc_bfe0, 0x01cf_8b00, 0x01cf_99d0, 0x01cf_b180,
];
const TRAIN: usize = 13;

/// One of the rider's bone sphere colliders in the state (`research/oracle.txt` S142): any of them
/// does, the pair reads only its centre, radius and record.
const SPHERE: u32 = 0x0085_c9d0;

/// The car's fields and its mesh collidable's (`research/oracle.txt` S81, S136, S142).
mod at {
    pub const NODE: u32 = 0x254;
    pub const NAME: u32 = 0x320;
    pub const MESH: u32 = 0x428;
    pub const PATH: u32 = 0x430;
    pub const LAST: u32 = 0x450;
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
    pub const PLACED: u32 = 0x120;
    pub const BEFORE: u32 = 0x2e0;
    pub const INV_DT: u32 = 0x348;
    /// The sphere collider's centre (and its copy), radius and squared radius, and its record: the
    /// count, the held distance, the slot's normal and point, and what the slot is with.
    pub const CENTRE: u32 = 0x40;
    pub const CENTRE2: u32 = 0x120;
    pub const RADIUS: u32 = 0x170;
    /// Any collidable's reach round its centre.
    pub const REACH: u32 = 0x60;
    pub const RADIUS2: u32 = 0x174;
    pub const RECORD: u32 = 0x6c;
    pub const COUNT: u32 = 0x4;
    pub const HELD: u32 = 0x8;
    pub const NORMAL: u32 = 0x30;
    pub const POINT: u32 = 0x60;
    pub const WITH: u32 = 0x90;
}

/// A bone collision's fields and its body's (`tests/ragdoll_contact.rs`).
mod coll {
    pub const ON: u32 = 0x04;
    pub const TOUCHED: u32 = 0x14;
    pub const SWEPT: u32 = 0x20;
    pub const UNPENETRATION: u32 = 0x2c;
    pub const ROTATION: u32 = 0x30;
    pub const RESTITUTION: u32 = 0x34;
    pub const FRICTION: u32 = 0x38;
    pub const POINT: u32 = 0x40;
    pub const NORMAL: u32 = 0x50;
    pub const DV: u32 = 0x60;
    pub const OTHER: u32 = 0x70;
    pub const BODY: u32 = 0x7c;
    pub const SPHERE: u32 = 0x88;
    pub const ENABLED: u32 = 0x00;
    pub const INV_MASS: u32 = 0x10;
    pub const B_RADIUS: u32 = 0x14;
    pub const POSITION: u32 = 0x60;
    pub const VELOCITY: u32 = 0x70;
    pub const INV_INERTIA: u32 = 0x120;
    pub const QUAT: u32 = 0x170;
    pub const RATES: u32 = 0x1b0;
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
    for name in [TRACK.to_string(), TRACK.replace(".res", "_2.res"), "Rider\\thq1.res".to_string()] {
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

fn set_vf(ee: &mut Ee, a: u32, v: Vf) {
    for k in 0..4 {
        ee.set_f32(a + 4 * k as u32, v[k]);
    }
}

fn mat(ee: &Ee, a: u32) -> Mat4 {
    Mat4([vf(ee, a), vf(ee, a + 16), vf(ee, a + 32), vf(ee, a + 48)])
}

fn bits(v: Vf) -> [u32; 4] {
    v.to_bits()
}

fn name(ee: &Ee, car: u32) -> String {
    let bytes: Vec<u8> = (0..64).map(|k| ee.u8_at(car + at::NAME + k)).take_while(|&b| b != 0).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The car's collision mesh off the disc.
fn mesh_of(ee: &Ee, parts: &[(Vec<u8>, Res)], car: u32) -> Mesh {
    let col = name(ee, car).to_ascii_lowercase().replace(".psg", ".col");
    let bytes = member(parts, &col).unwrap_or_else(|| panic!("no {col} in free06's archives"));
    Mesh::parse(&bytes).unwrap_or_else(|e| panic!("{col}: {e}"))
}

/// The port's car as the state holds it: the disc's mesh in the collidable's frame, and the motion
/// of the car's last update.
fn path_car(ee: &Ee, mesh: &Mesh, car: u32) -> PathCar {
    let collidable = ee.u32_at(car + at::MESH);
    let [x, y, z, o] = mat(ee, collidable + at::PLACED).0;
    let inv_dt = F(ee.f32_at(ee.u32_at(addr("app_pointer").unwrap()) + at::INV_DT));
    let node = ee.u32_at(car + at::NODE);
    let motion = Moving::of(&mat(ee, car + at::BEFORE), &mat(ee, node + at::WORLD), inv_dt);
    PathCar {
        mesh: mesh.clone(),
        frame: Node { rows: [x, y, z], origin: o },
        mover: motion.mover(),
        distance: F::ZERO,
        centre: vf(ee, collidable + at::CENTRE),
        radius: F(ee.f32_at(collidable + at::REACH)),
    }
}

/// A small generator.
struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    fn upto(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, n: u64) -> bool {
        self.upto(n) == 0
    }
    fn float(&mut self, x: f32) -> f32 {
        match self.upto(16) {
            0 => 0.0,
            1 => -0.0,
            2 => -x,
            _ => self.range(-x, x),
        }
    }
    fn vector(&mut self, x: f32) -> Vf {
        Vf::new(self.float(x), self.float(x), self.float(x), self.float(2.0))
    }
    fn unit_vector(&mut self) -> Vf {
        let v = [self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0)];
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3);
        Vf::new(v[0] / n, v[1] / n, v[2] / n, 0.0)
    }
    fn quat(&mut self) -> Vf {
        let v = [self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0)];
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3);
        Vf::new(v[0] / n, v[1] / n, v[2] / n, v[3] / n)
    }
}

/// A point near the car's mesh: one of its vertices through its frame.
fn near_vertex(car: &PathCar, pick: &mut Pick) -> Vf {
    let n = match &car.mesh.vertices {
        game_sim::track::Points::Wide(v) => v.len(),
        game_sim::track::Points::Packed(v) => v.len(),
    };
    let i = pick.upto(n as u64) as u16;
    let v = game_sim::collide::MeshRef::of(&car.mesh).vertex(i);
    car.frame.place(Vf::new(v.x(), v.y(), v.z(), 1.0))
}

/// How many spheres [`the_sphere_test_on_a_car_answers_as_the_originals`] stands.
const SPHERES: usize = 20_000;

/// **The sphere test on the cars answers as the original's** (S142): 20,000 spheres stood near a
/// vertex of one of the state's cars (radii the bones' three and random ones from a tenth to a
/// metre), each through the original's pair on a bone's sphere collider with an emptied record, once
/// with the car alone, once with it and then the car behind it on the train, and once with the same
/// car twice; and through the port's cars, each offered the record's hit so far. The record's count,
/// what it is with, the plane distance it holds, and the contact's point and normal in the world,
/// bit for bit.
#[test]
fn the_sphere_test_on_a_car_answers_as_the_originals() {
    let Some(mut ee) = state() else { return };
    let Some(parts) = archives() else { return };
    let (clear, pair) = (addr("sphere_collider_clear").unwrap(), addr("sphere_collider_pair").unwrap());
    let meshes: Vec<Mesh> = CARS.iter().map(|&car| mesh_of(&ee, &parts, car)).collect();
    let cars: Vec<PathCar> = CARS.iter().zip(&meshes).map(|(&car, m)| path_car(&ee, m, car)).collect();
    let mut pick = Pick(0x5142_5142_0bad_cafe);
    let (mut hits, mut second, mut kept) = (0, 0, 0);
    for case in 0..SPHERES {
        let k = pick.upto(CARS.len() as u64) as usize;
        // One car, the car and the one behind it, or the same car twice.
        let order: Vec<usize> = match pick.upto(3) {
            0 => vec![k],
            1 if k + 1 < TRAIN => vec![k, k + 1],
            1 if k > 0 && k < TRAIN => vec![k, k - 1],
            _ => vec![k, k],
        };
        let p = near_vertex(&cars[k], &mut pick);
        let radius = match case % 5 {
            0 => 0.4f32,
            1 => 0.75,
            2 => 0.5,
            _ => pick.range(0.1, 1.0),
        };
        let off = pick.unit_vector();
        let reach = 2.0 * radius * pick.unit();
        let centre = Vf::new(p.x() + off.x() * reach, p.y() + off.y() * reach, p.z() + off.z() * reach, 1.0);
        set_vf(&mut ee, SPHERE + at::CENTRE, centre);
        set_vf(&mut ee, SPHERE + at::CENTRE2, centre);
        ee.set_f32(SPHERE + at::RADIUS, radius);
        ee.set_f32(SPHERE + at::RADIUS2, ps2_float::fpu::mul(radius, radius));
        ee.call(clear, &[u64::from(SPHERE)], &[], 1_000_000);
        for &c in &order {
            ee.call(pair, &[u64::from(SPHERE), u64::from(ee.u32_at(CARS[c] + at::MESH))], &[], BUDGET);
        }
        let rec = ee.u32_at(SPHERE + at::RECORD);
        let count = ee.u32_at(rec + at::COUNT);
        let original = (count > 0).then(|| {
            (ee.u32_at(rec + at::WITH), ee.u32_at(rec + at::HELD), bits(vf(&ee, rec + at::POINT)), bits(vf(&ee, rec + at::NORMAL)))
        });

        let mut best: Option<(usize, Touch)> = None;
        for &c in &order {
            if let Some(t) = cars[c].touch(centre, F(radius), best.map(|(_, b)| b.distance)) {
                best = Some((c, t));
            }
        }
        let port = best.map(|(c, t)| (ee.u32_at(CARS[c] + at::MESH), t.distance.to_bits(), bits(t.world.0), bits(t.world.1)));
        let what = format!("case {case}: cars {order:?} ({}), sphere at {centre:?} r {radius}", name(&ee, CARS[k]));
        assert!(count <= 1, "{what}: {count} contacts");
        assert_eq!(port, original, "{what}");
        if let Some((c, _)) = best {
            hits += 1;
            second += usize::from(order.len() == 2 && c == order[1] && order[0] != order[1]);
            kept += usize::from(order.len() == 2 && c == order[0]);
        }
    }
    eprintln!("{hits} of {SPHERES} hit; the second car took {second}, the first kept {kept}");
    assert!(hits > SPHERES / 5 && hits < SPHERES * 9 / 10, "{hits} hits");
    assert!(second > 20 && kept > 1000, "the second car took {second} records, the first kept {kept}");
}

/// The bone collisions in the state: each one whose sphere (`+0x88`) is a sphere collider of
/// [`SPHERE`]'s class and whose body (`+0x7c`) has the sphere's radius.
fn collisions(ee: &Ee) -> Vec<u32> {
    let class = ee.u32_at(SPHERE + 0x38);
    let ram = |a: u32| (0x10_0000..0x1ff_fe00).contains(&a) && a % 4 == 0;
    (0x10_0000u32..0x1ff_fe00)
        .step_by(4)
        .filter(|&a| {
            let (s, b) = (ee.u32_at(a + coll::SPHERE), ee.u32_at(a + coll::BODY));
            ram(s) && ram(b) && ee.u32_at(s + 0x38) == class && ee.u32_at(b + coll::B_RADIUS) == ee.u32_at(s + at::RADIUS)
        })
        .collect()
}

fn curve(ee: &Ee) -> CurveTable {
    let base = ee.u32_at(addr("curve_table_ptr").unwrap());
    let entries: Vec<[f32; 2]> =
        (0..CURVE_ENTRIES as u32).map(|k| [ee.f32_at(base + 8 * k), ee.f32_at(base + 8 * k + 4)]).collect();
    CurveTable::from_entries(&entries)
}

/// **The push-out against a car is the original's** (S142): 10,000 bones generated on the state's
/// bone collisions, each pushed out of a contact near one of the 21 cars with the contact's other
/// the car's mesh collidable. The original's push-out finds the car as a mover through the
/// collidable's owner and calls the car's own two virtuals and reads its node; the port pushes
/// against [`moving`] of the car's [`Moving::mover`]. The bone's position, velocity, orientation and
/// rates, and the contact's change of velocity, bit for bit.
#[test]
fn the_push_out_against_a_car_is_the_originals() {
    let Some(mut ee) = state() else { return };
    let table = curve(&ee);
    let push_out = addr("bone_collision_push_out").unwrap();
    let found = collisions(&ee);
    assert!(found.len() >= 9, "{} bone collisions in the state", found.len());
    let Some(parts) = archives() else { return };
    let meshes: Vec<Mesh> = CARS.iter().map(|&car| mesh_of(&ee, &parts, car)).collect();
    let cars: Vec<PathCar> = CARS.iter().zip(&meshes).map(|(&car, m)| path_car(&ee, m, car)).collect();
    let (point_at, normal_at) = (0x01f8_0000u32, 0x01f8_0010u32);
    let mut pick = Pick(0x5142_0f_0f_5142);
    let mut moved = 0;
    for case in 0..10_000 {
        let c = found[pick.upto(found.len() as u64) as usize];
        let b = ee.u32_at(c + coll::BODY);
        let k = pick.upto(CARS.len() as u64) as usize;
        let around = cars[k].mover.node.origin;
        let pos = Vf::new(around.x() + pick.range(-20.0, 20.0), around.y() + pick.range(-5.0, 10.0), around.z() + pick.range(-20.0, 20.0), pick.float(2.0));
        let radius = pick.range(0.2, 1.0);
        ee.set_u32(b + coll::ENABLED, 1);
        ee.set_f32(b + coll::INV_MASS, pick.range(0.05, 6.0));
        ee.set_f32(b + coll::B_RADIUS, radius);
        set_vf(&mut ee, b + coll::POSITION, pos);
        set_vf(&mut ee, b + coll::VELOCITY, pick.vector(60.0));
        set_vf(&mut ee, b + coll::INV_INERTIA, Vf::new(pick.range(0.01, 6.0), pick.range(0.01, 6.0), pick.range(0.01, 6.0), 0.0));
        set_vf(&mut ee, b + coll::QUAT, pick.quat());
        set_vf(&mut ee, b + coll::RATES, pick.vector(10.0));
        ee.set_u32(c + coll::ON, 1);
        ee.set_f32(c + coll::UNPENETRATION, if pick.chance(3) { 1.0 } else { pick.range(0.0, 1.5) });
        ee.set_f32(c + coll::ROTATION, if pick.chance(3) { 1.0 } else { pick.range(0.0, 1.5) });
        ee.set_f32(c + coll::RESTITUTION, if pick.chance(2) { 0.0 } else { pick.range(0.0, 1.0) });
        ee.set_f32(c + coll::FRICTION, if pick.chance(2) { 1.0 } else { pick.range(0.0, 1.2) });
        set_vf(&mut ee, c + coll::DV, pick.vector(9.0));
        let swept = pick.chance(4);
        ee.set_u32(c + coll::SWEPT, u32::from(swept));
        let d = pick.unit_vector();
        let r = radius * pick.range(-0.5, 1.5);
        let point = Vf::new(pos.x() + d.x() * r, pos.y() + d.y() * r, pos.z() + d.z() * r, 1.0);
        let normal = Vf::new(-d.x(), -d.y(), -d.z(), 1.0);
        set_vf(&mut ee, point_at, point);
        set_vf(&mut ee, normal_at, normal);
        set_vf(&mut ee, c + coll::POINT, point);
        set_vf(&mut ee, c + coll::NORMAL, normal);
        ee.set_u32(c + coll::OTHER, ee.u32_at(CARS[k] + at::MESH));

        let f = |o: u32| F(ee.f32_at(c + o));
        let sphere = Sphere {
            bone: 0,
            radius: F(radius),
            unpenetration: f(coll::UNPENETRATION),
            rotation: f(coll::ROTATION),
            restitution: f(coll::RESTITUTION),
            friction: f(coll::FRICTION),
            sweep: false,
            sweep_back: F::ZERO,
        };
        let mut body = Body {
            enabled: true,
            inv_mass: F(ee.f32_at(b + coll::INV_MASS)),
            position: vf(&ee, b + coll::POSITION),
            velocity: vf(&ee, b + coll::VELOCITY),
            inv_inertia: vf(&ee, b + coll::INV_INERTIA),
            quat: vf(&ee, b + coll::QUAT),
            rates: vf(&ee, b + coll::RATES),
            ..Body::default()
        };
        let mut contact = Contact {
            touched: ee.u32_at(c + coll::TOUCHED) != 0,
            ground: false,
            scene: false,
            swept,
            point,
            normal,
            dv: vf(&ee, c + coll::DV),
        };
        let against = moving(point, Against::Mover(&cars[k].mover));
        contact.push_out_against(&sphere, &mut body, point, normal, against, &table);
        ee.call(push_out, &[u64::from(c), u64::from(point_at), u64::from(normal_at)], &[], BUDGET);

        let what = format!("case {case}: {} ({:#x}), swept {swept}", name(&ee, CARS[k]), CARS[k]);
        for (field, got, want) in [
            ("position", body.position, vf(&ee, b + coll::POSITION)),
            ("velocity", body.velocity, vf(&ee, b + coll::VELOCITY)),
            ("quat", body.quat, vf(&ee, b + coll::QUAT)),
            ("rates", body.rates, vf(&ee, b + coll::RATES)),
            ("dv", contact.dv, vf(&ee, c + coll::DV)),
        ] {
            assert_eq!(bits(got), bits(want), "{what}: {field} is {want:?} in the game, {got:?} in the port");
        }
        moved += usize::from(against.x() != 0.0 || against.z() != 0.0);
    }
    assert!(moved > 5000, "only {moved} contacts on a moving car");
}

// --- the ride ---------------------------------------------------------------------------------------

/// A car's path and numbers, and its state, out of the state (`tests/traffic.rs`).
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
        // Caught mid-ride: the cursor has long left where the measuring put it.
        stale: false,
        level: Default::default(),
    }
}

/// The ragdoll's kit as the shell builds it (`tests/ragdoll.rs`'s `ride_kit`): the rider's body and
/// model off the disc, the rider file's parts and pins, and the wreck bike's sphere and mesh; with
/// them the wreck bike's part and the rider's wreck set.
fn ride_kit() -> Option<(game_sim::ragdoll_ride::Kit, game_sim::phy::Part, Wrecks)> {
    use game_render::disc::Disc;
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let names: Vec<String> =
        disc.entries().iter().filter(|e| e.name.to_ascii_lowercase().ends_with(".res")).map(|e| e.name.clone()).collect();
    let rider_name = names.iter().filter(|n| n.to_ascii_lowercase().ends_with("thq1.res")).min_by_key(|n| n.len())?.clone();
    let res = disc.res(&rider_name).expect("the rider archive opens");
    let rider_file = res.read_suffix("mrider.phy").expect("the rider's archive carries mrider.phy");
    let mut body = game_render::rider::Body::load(
        vec![disc.res(&rider_name).expect("the rider archive opens")],
        "thq1",
        game_render::rider::Sex::Male,
        "",
        &[],
        game_sim::motion::Params::default(),
    )
    .expect("the rider's body loads");
    let model = game_render::psg::Model::parse(&res.read_suffix(&format!("{}.psg", body.geometry())).expect("its model"))
        .expect("the model parses");
    let doll = body.ragdoll_rig(&model, &rider_file).expect("every part names a node");
    let wreck_bike = names
        .iter()
        .find_map(|n| disc.res(n).ok().and_then(|r| r.read_suffix("wreckbike.phy").ok()))
        .expect("an archive carries wreckbike.phy");
    let mut kit = game_sim::ragdoll_ride::Kit::new(&rider_file, doll.rig);
    let col = names.iter().find_map(|n| disc.res(n).ok().and_then(|r| r.read_suffix("wreckbikef.col").ok()));
    kit.bike = col.as_deref().and_then(game_sim::ragdoll_ride::bike_sphere);
    kit.bike_mesh = col.as_deref().and_then(game_sim::ragdoll_scene::Bike::parse).map(std::sync::Arc::new);
    assert!(kit.bike.is_some() && kit.bike_mesh.is_some(), "an archive carries wreckbikef.col");
    Some((kit, game_sim::phy::Part::read_bytes(&wreck_bike, "PhysicsPart1"), Wrecks::read_bytes(&rider_file)))
}

/// The wagon of `tests/traffic.rs`'s parked bike: the train's sixth car, a boxcar whose roof is flat
/// at `y` 151.6 for 24 feet either way of its middle, running `+x` at 55 ft/s.
const WAGON: usize = 5;
const ROOF: f32 = 151.6;
const DECK: f32 = 24.0;

/// How the cars are in the bones' scene.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Cars {
    /// As they are: moving.
    Moving,
    /// In the scene, but reported standing still (their motion's pose kept, velocity and spin 0).
    Still,
    /// Not in the scene at all: the port before S142.
    Gone,
}

/// Where the main bone is in the wagon's frame (`x` across, `z` along; `y` its height in the world),
/// and how fast it moves over the wagon in the wagon's frame.
#[derive(Clone, Copy, Debug)]
struct Seen {
    at: Vf,
    over: Vf,
}

/// **A ragdoll beside or on the wagon** for `ticks` ticks of the moving train: entered from the
/// riding pose, its hands let go, every bone then moved so the main bone is `offset` from the wagon's
/// middle (`x`, `z` in the wagon's frame, `y` the height in the world), with the wagon's velocity
/// plus `push` (in the wagon's frame). The wreck bike stays where
/// the wreck entered, out of the way and out of the broad phase. The production order every tick:
/// the cars, then the bones. Answers the main bone each tick.
fn ride(cars: Cars, offset: Vf, push: Vf, ticks: usize) -> Option<Vec<Seen>> {
    let ee = state()?;
    let parts = archives()?;
    let (kit, part, wrecks) = ride_kit()?;
    let scn = member(&parts, "free06.scn").expect("free06.scn");
    let scene = Scene::parse(&scn).expect("free06's scene");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let terrain_bytes = member(&parts, &leaf).expect("free06's terrain");
    let terrain = Terrain::parse(&terrain_bytes).expect("free06's terrain");
    let mut world = Place { ground: Ground::new(&terrain, &scene.terrain), props: Props::default() };
    let specs: Vec<Spec> = CARS.iter().map(|&car| spec_of(&ee, car, Some(mesh_of(&ee, &parts, car)))).collect();
    let mut traffic = Traffic::new(specs);
    traffic.cars = CARS.iter().map(|&car| car_of(&ee, car)).collect();
    traffic.attach(&mut world.props);
    let first = traffic.first.expect("attached");
    let frame_cars = |traffic: &mut Traffic, world: &mut Place| {
        traffic.frame(DT, world);
        for p in &mut world.props.props {
            match cars {
                Cars::Moving => {}
                Cars::Still => p.moving = p.moving.map(|m| Moving::of(&m.pose, &m.pose, F::ONE / DT)),
                Cars::Gone => p.moving = None,
            }
        }
    };
    let before = traffic.cars[WAGON].pose;
    frame_cars(&mut traffic, &mut world);
    let _ = first;
    let wagon = traffic.cars[WAGON].pose;
    let velocity = Moving::of(&before, &wagon, F::ONE / DT).velocity;

    // The wreck where the train has not got to: 200 feet ahead of the wagon and off the track.
    let mut bike = game_sim::ride::Ride::default();
    bike.chassis.quat = Vf::new(0.0, 0.0, 0.0, 1.0);
    bike.origin = Vf::new(wagon.0[3].x() + 200.0, ROOF + 40.0, wagon.0[3].z() + 200.0, 1.0);
    let crash = game_sim::ride::Crash::begin(&bike, game_sim::trick::Wreck(5), &part, game_sim::ride::WRECK_INERTIA, &wrecks, None);
    let mut doll = game_sim::ragdoll_ride::Down::enter(&kit, &bike.transform(), &crash, crash.throw, 5, &wrecks, &mut Rng::default(), &mut world);
    doll.ragdoll.holds = [false; 4];
    let rows = Mat3::new(wagon.0[0], wagon.0[1], wagon.0[2]);
    let main = doll.main().position;
    let to = chassis::rotate(&rows, Vf::new(offset.x(), 0.0, offset.z(), 0.0));
    let pushed = chassis::rotate(&rows, push);
    for b in &mut doll.ragdoll.bones {
        let p = b.body.position;
        b.body.position = Vf::new(
            p.x() - main.x() + wagon.0[3].x() + to.x(),
            p.y() - main.y() + offset.y(),
            p.z() - main.z() + wagon.0[3].z() + to.z(),
            p.w(),
        );
        b.body.velocity = Vf::new(velocity.x() + pushed.x(), velocity.y() + pushed.y(), velocity.z() + pushed.z(), 0.0);
        b.body.rates = Vf::ZERO;
    }

    let mut seen = Vec::new();
    let mut last = wagon;
    for _ in 0..ticks {
        frame_cars(&mut traffic, &mut world);
        doll.step_posed_with(&kit, None, DT, &crash, &mut world, false);
        let pose = traffic.cars[WAGON].pose;
        let rows = Mat3::new(pose.0[0], pose.0[1], pose.0[2]);
        let m = doll.main();
        let d = Vf::new(m.position.x() - pose.0[3].x(), m.position.y() - pose.0[3].y(), m.position.z() - pose.0[3].z(), 0.0);
        let now = Moving::of(&last, &pose, F::ONE / DT).velocity;
        last = pose;
        let v = Vf::new(m.velocity.x() - now.x(), m.velocity.y() - now.y(), m.velocity.z() - now.z(), 0.0);
        let at = chassis::unrotate(&rows, d);
        seen.push(Seen { at: Vf::new(at.x(), m.position.y(), at.z(), 0.0), over: chassis::unrotate(&rows, v) });
    }
    Some(seen)
}

/// The wagon's flank, in its own frame: its mesh's right wall stands at `x` 4.5 from the roof at
/// [`ROOF`] down to below the floor, solid for `z` from 12.5 to 24 feet either way of its middle
/// (between them is the door, whose opening the mesh leaves open from 142 to 150 feet up).
const FLANK: f32 = 4.5;
const FLANK_ALONG: f32 = -17.0;

/// **A ragdoll thrown into the moving train's side bounces off it** (S142's test): the bones put
/// beside the wagon's flank, the main bone 2.5 feet out of it and halfway up it, moving with the
/// wagon and 20 ft/s at it. With the cars in the bones' scene the wall pushes the main bone back out
/// to its own radius and takes its 20 ft/s at the wagon away (the bones' push-out keeps little of a
/// bounce), and the wall's friction carries it along with the wagon as it slides down. Without them
/// (the port before S142) it is through the wall within five ticks and out of the far side.
#[test]
fn a_ragdoll_thrown_into_the_trains_side_bounces_off() {
    let offset = Vf::new(FLANK + 2.5, ROOF - 5.5, FLANK_ALONG, 0.0);
    let push = Vf::new(-20.0, 0.0, 0.0, 0.0);
    let Some(hit) = ride(Cars::Moving, offset, push, 20) else { return };
    let gone = ride(Cars::Gone, offset, push, 20).expect("the same state");
    let deepest = |s: &[Seen]| s.iter().map(|s| s.at.x()).fold(f32::MAX, f32::min);
    for (t, s) in hit.iter().enumerate() {
        println!(
            "tick {t}: at ({:.2}, {:.2}, {:.2}) over ({:.2}, {:.2}, {:.2}); without the cars at x {:.2}",
            s.at.x(), s.at.y(), s.at.z(), s.over.x(), s.over.y(), s.over.z(), gone[t].at.x()
        );
    }
    let last = hit.last().unwrap();
    assert!(deepest(&hit) > FLANK, "the main bone got to {} through a wall at {FLANK}", deepest(&hit));
    assert!(last.over.x() > -1.0, "the main bone still moves at the wagon at {} ft/s", -last.over.x());
    assert!(last.over.z().abs() < 1.5, "the main bone slides along the wagon at {} ft/s", last.over.z());
    assert!(deepest(&gone) < FLANK - 9.0, "without the cars the bone only got to {} (the wall at {FLANK})", deepest(&gone));
}

/// **A ragdoll landed on a wagon's roof moves with it** (S142's test): the bones dropped a foot onto
/// the boxcar's roof at its middle, moving with it. With the cars moving it rides for two seconds:
/// still on the roof, within a few feet of where it landed, and at the wagon's speed. With the
/// wagon in the scene but reported still (the push-out measuring the bones against the world, as a
/// static prop would) the roof slides out from under it and it goes off the back; without the cars
/// it falls through.
#[test]
fn a_ragdoll_landed_on_a_wagons_roof_moves_with_it() {
    let offset = Vf::new(0.0, ROOF + 4.5, 0.0, 0.0);
    let Some(carried) = ride(Cars::Moving, offset, Vf::ZERO, 60) else { return };
    let still = ride(Cars::Still, offset, Vf::ZERO, 60).expect("the same state");
    let gone = ride(Cars::Gone, offset, Vf::ZERO, 60).expect("the same state");
    for (t, s) in carried.iter().enumerate() {
        println!(
            "tick {t}: at ({:.2}, {:.2}, {:.2}) over ({:.2}, {:.2}, {:.2}); still at z {:.2} y {:.2}; gone y {:.2}",
            s.at.x(), s.at.y(), s.at.z(), s.over.x(), s.over.y(), s.over.z(), still[t].at.z(), still[t].at.y(), gone[t].at.y()
        );
    }
    let on = |s: &Seen| s.at.z().abs() < DECK && s.at.y() > ROOF - 1.0;
    let last = carried.last().unwrap();
    assert!(carried.iter().all(on), "the bones left the roof");
    assert!(last.at.z().abs() < 6.0, "the bones slid {} ft along the roof", last.at.z());
    let speed = (last.over.x() * last.over.x() + last.over.z() * last.over.z()).sqrt();
    assert!(speed < 2.0, "the bones move at {speed} ft/s over the wagon");
    assert!(!still.iter().all(on), "a wagon reported still carried the bones too");
    assert!(!gone.iter().all(on), "the bones stayed on a roof that is not in the scene");
}



// --- the sweep (S166) -------------------------------------------------------------------------------

/// A bone collision's sweep fields, the collidables' filter fields, and a search's
/// (`research/oracle.txt` S52, S63, S105, S166).
mod sweep_at {
    pub const SCENE: u32 = 0x1c;
    pub const SWEEP_ON: u32 = 0x24;
    pub const SWEEP_BACK: u32 = 0x28;
    pub const OTHER2: u32 = 0x74;
    pub const SEARCH: u32 = 0x84;
    pub const PROBE: u32 = 0x8c;
    pub const CLASS: u32 = 0x84;
    pub const MASK_MODE: u32 = 0xe8;
    pub const MASKS: u32 = 0xec;
    pub const MASK_COUNT: u32 = 0xf0;
    pub const LISTENERS: u32 = 0xf4;
    pub const ON_HIT: u32 = 0x10c;
    pub const ON_STRUCK: u32 = 0x110;
    pub const VTABLE: u32 = 0x38;
    pub const LIST: u32 = 0x0;
    pub const COUNT: u32 = 0x4;
    pub const GRID: u32 = 0x38;
}

/// A scene that is only the search's list, as a bone's segment probe with the include list
/// `.0` meets it.
struct Listed<'a>(u32, Vec<Candidate<'a>>);

impl game_sim::ragdoll_contact::Scene for Listed<'_> {
    fn sweep(&mut self, from: Vf, to: Vf) -> Option<(Vf, Vf)> {
        sweep_through(from, to, self.0, self.1.iter().copied())
    }
}

/// How many segments [`the_bones_sweep_through_the_cars_as_the_originals`] sweeps.
const SEGMENTS: usize = 20_000;

/// **A bone's sweep through the cars is the original's** (S166): 20,000 bones on the state's bone
/// collisions, each moving so that its last `1/15` s crosses a point near one of the 21 cars (from
/// a standstill to 120 ft/s, a random way), with the collision's search a list of that car alone,
/// it and the car behind it on the train, or it twice, at distances that are mostly 0 and sometimes
/// past the segment's reach. The original's bone collision sweep (its segment probe stood, cleared,
/// filtered and paired through the list, and the push-out of a hit) against the port's solve with a
/// scene that is only that list: swept or not, the contact's point and normal, and the bone's
/// position, velocity and the contact's change of velocity, bit for bit.
///
/// Every bone's probe includes only the scenery's class, so half the segments run as the game has
/// them and are never swept by a car, in either; the other half have the probe's include word
/// widened to the cars' `0x8` bit in both, which runs the pair on the car meshes.
#[test]
fn the_bones_sweep_through_the_cars_as_the_originals() {
    let Some(mut ee) = state() else { return };
    let Some(parts) = archives() else { return };
    let table = curve(&ee);
    let sweep = addr("bone_collision_sweep").unwrap();
    let found = collisions(&ee);
    assert!(found.len() >= 9, "{} bone collisions in the state", found.len());
    let probe_class = addr("sweep_probe_class").unwrap();
    for &c in &found {
        let p = ee.u32_at(c + sweep_at::PROBE);
        assert_eq!(ee.u32_at(p + sweep_at::VTABLE), probe_class, "{c:#x}'s segment is a sweep probe");
        // The filter's list: include mode, the scenery's class alone.
        let list = (ee.u32_at(p + sweep_at::MASK_MODE), ee.u32_at(p + sweep_at::MASK_COUNT), ee.u32_at(ee.u32_at(p + sweep_at::MASKS)));
        assert_eq!(list, (0, 1, class::BONE_SWEEP), "{c:#x}'s probe's include list");
        // Nothing listens to a struck probe, so the sweep changes nothing but the bone.
        assert_eq!((ee.u32_at(p + sweep_at::LISTENERS), ee.u32_at(p + sweep_at::ON_HIT)), (0, 0), "{c:#x}'s probe's listeners");
    }
    let meshes: Vec<Mesh> = CARS.iter().map(|&car| mesh_of(&ee, &parts, car)).collect();
    let cars: Vec<PathCar> = CARS.iter().zip(&meshes).map(|(&car, m)| path_car(&ee, m, car)).collect();
    for &car in &CARS {
        let m = ee.u32_at(car + at::MESH);
        assert_eq!(ee.u32_at(m + sweep_at::CLASS), class::PATH_CAR, "{}'s class", name(&ee, car));
        assert_eq!((ee.u32_at(m + sweep_at::LISTENERS), ee.u32_at(m + sweep_at::ON_STRUCK)), (0, 0), "{}'s listeners", name(&ee, car));
    }
    let (search, list) = (0x01f8_0100u32, 0x01f8_0200u32);
    for k in 0..0x40 {
        ee.set_u32(search + 4 * k, 0);
    }
    ee.set_u32(search + sweep_at::LIST, list);
    assert_eq!(ee.u32_at(search + sweep_at::GRID), 0);
    let mut pick = Pick(0x5166_5166_feed_f00d);
    let (mut swept, mut second, mut far, mut as_game) = (0, 0, 0, 0);
    for case in 0..SEGMENTS {
        let c = found[pick.upto(found.len() as u64) as usize];
        let b = ee.u32_at(c + coll::BODY);
        let p = ee.u32_at(c + sweep_at::PROBE);
        let includes = if case % 2 == 0 { class::BONE_SWEEP } else { 0x8 };
        ee.set_u32(ee.u32_at(p + sweep_at::MASKS), includes);
        let k = pick.upto(CARS.len() as u64) as usize;
        let order: Vec<usize> = match pick.upto(3) {
            0 => vec![k],
            1 if k + 1 < TRAIN => vec![k, k + 1],
            1 if k > 0 && k < TRAIN => vec![k, k - 1],
            _ => vec![k, k],
        };
        // The segment crosses `q`, near the car's mesh, `u` of the way back from where the bone is.
        let q = near_vertex(&cars[k], &mut pick);
        let off = pick.unit_vector();
        let q = Vf::new(q.x() + off.x() * pick.range(0.0, 1.0), q.y() + off.y() * pick.range(0.0, 1.0), q.z() + off.z() * pick.range(0.0, 1.0), 1.0);
        let d = pick.unit_vector();
        let length = if pick.chance(20) { 0.0 } else { pick.range(0.0, 8.0) };
        let u = pick.unit();
        let pos = Vf::new(q.x() + d.x() * length * u, q.y() + d.y() * length * u, q.z() + d.z() * length * u, if pick.chance(4) { pick.float(2.0) } else { 1.0 });
        let vel = Vf::new(d.x() * length * 15.0, d.y() * length * 15.0, d.z() * length * 15.0, if pick.chance(4) { pick.float(2.0) } else { 0.0 });
        let distances: Vec<f32> = order.iter().map(|_| if pick.chance(4) { pick.range(0.0, 4.0) } else { 0.0 }).collect();
        for (i, (&o, &dist)) in order.iter().zip(&distances).enumerate() {
            ee.set_u32(list + 8 * i as u32, ee.u32_at(CARS[o] + at::MESH));
            ee.set_f32(list + 8 * i as u32 + 4, dist);
        }
        ee.set_u32(search + sweep_at::COUNT, order.len() as u32);

        ee.set_u32(b + coll::ENABLED, 1);
        set_vf(&mut ee, b + coll::POSITION, pos);
        set_vf(&mut ee, b + coll::VELOCITY, vel);
        ee.set_u32(c + coll::ON, 1);
        ee.set_u32(c + sweep_at::SWEEP_ON, 1);
        ee.set_u32(c + sweep_at::SEARCH, search);
        ee.set_u32(c + coll::OTHER, 0);
        ee.set_u32(c + sweep_at::OTHER2, 0);
        ee.set_u32(c + sweep_at::SCENE, 0);
        ee.set_u32(c + coll::SWEPT, 0);
        set_vf(&mut ee, c + coll::POINT, pos);
        set_vf(&mut ee, c + coll::NORMAL, Vf::new(0.0, 1.0, 0.0, 1.0));

        let f = |o: u32| F(ee.f32_at(c + o));
        let sphere = Sphere {
            bone: 0,
            radius: F(ee.f32_at(b + coll::B_RADIUS)),
            unpenetration: f(coll::UNPENETRATION),
            rotation: f(coll::ROTATION),
            restitution: f(coll::RESTITUTION),
            friction: f(coll::FRICTION),
            sweep: true,
            sweep_back: f(sweep_at::SWEEP_BACK),
        };
        assert_eq!(sphere.sweep_back.0.to_bits(), game_sim::ragdoll_contact::SWEEP_BACK.to_bits());
        let mut body = Body {
            enabled: true,
            inv_mass: F(ee.f32_at(b + coll::INV_MASS)),
            position: pos,
            velocity: vel,
            inv_inertia: vf(&ee, b + coll::INV_INERTIA),
            quat: vf(&ee, b + coll::QUAT),
            rates: vf(&ee, b + coll::RATES),
            ..Body::default()
        };
        let mut contact = Contact { touched: false, ground: false, scene: false, swept: false, point: pos, normal: Vf::new(0.0, 1.0, 0.0, 1.0), dv: vf(&ee, c + coll::DV) };
        let candidates = order
            .iter()
            .zip(&distances)
            .map(|(&o, &dist)| Candidate {
                distance: F(dist),
                class: class::PATH_CAR,
                centre: cars[o].centre,
                radius: cars[o].radius,
                mesh: &cars[o].mesh,
                frame: &cars[o].frame,
            })
            .collect();
        let mut scene = Listed(includes, candidates);
        contact.solve(true, &sphere, &mut body, None, &mut scene, &table);
        ee.call(sweep, &[u64::from(c)], &[], BUDGET);

        let hit = ee.u32_at(c + coll::SWEPT) != 0;
        let what = format!("case {case}: includes {includes:#x}, cars {order:?} at {distances:?} ({}), bone at {pos:?} moving {vel:?}", name(&ee, CARS[k]));
        assert_eq!(contact.swept, hit, "{what}: swept");
        assert_eq!(contact.scene, ee.u32_at(c + sweep_at::SCENE) != 0, "{what}: scene");
        let mut fields = vec![("position", body.position, vf(&ee, b + coll::POSITION)), ("velocity", body.velocity, vf(&ee, b + coll::VELOCITY))];
        if hit {
            fields.extend([
                ("point", contact.point, vf(&ee, c + coll::POINT)),
                ("normal", contact.normal, vf(&ee, c + coll::NORMAL)),
                ("dv", contact.dv, vf(&ee, c + coll::DV)),
            ]);
        }
        for (field, got, want) in fields {
            assert_eq!(bits(got), bits(want), "{what}: {field} is {want:?} in the game, {got:?} in the port");
        }
        if includes == class::BONE_SWEEP {
            assert!(!hit, "{what}: the game's probe swept a car");
            as_game += 1;
        }
        if hit {
            swept += 1;
            second += usize::from(order.len() == 2 && order[0] != order[1]);
        }
        far += usize::from(distances.iter().any(|&d| d > 0.0));
        ee.set_u32(ee.u32_at(p + sweep_at::MASKS), class::BONE_SWEEP);
    }
    eprintln!("{as_game} segments as the game has them, none swept; of the rest {swept} swept, {second} with two cars listed; {far} lists with a car off at a distance");
    assert!(swept > SEGMENTS / 20 && swept < SEGMENTS * 9 / 20, "{swept} swept");
    assert!(second > 300, "{second} two-car sweeps");
}
