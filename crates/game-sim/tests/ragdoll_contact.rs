//! S52 — the ragdoll's bone collisions against the original.
//!
//! Like tests/ragdoll.rs, every proof runs on the live rider body actor in the first recording's save
//! state, on its nine bone collisions as the game wired them:
//!
//! 1. **the records**: `Sphere::read` of the disc's `rider\anim\mrider.phy` against what the loader
//!    and the constructor left on each collision and its bone's body;
//! 2. **the solve**, over generated bones, collision numbers, terrain boxes, ground answers, scene
//!    contacts and sweep hits. The ground query, the scene's two collide calls and the sweep's
//!    segment setters are hooked: the hooks answer what the case generated and log what they were
//!    asked, and the port's [`Terrain`] and [`Scene`] answer the same and log the same. The push-out
//!    and the impulse run for real on both sides;
//! 3. **the bike box**, over generated frames, boxes, carried velocities and bones in and out of the
//!    box, on both of its frames: the actor's moving one and the constraint's own;
//! 4. **the reset**: the actor's own reset over every body of the ragdoll's world, with every field
//!    of every body generated, and the body's two start setters.

use game_sim::archive::{Psr, Res};
use game_sim::biplane::Body;
use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::collide::{Bundle, Record};
use game_sim::math::Mat4;
use game_sim::ragdoll_pose::{self, Start};
use game_sim::ragdoll_contact::{BikeBox, Contact, SWEEP_BACK, Scene, Sphere, Terrain};
use game_sim::world::World;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::cell::RefCell;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::rc::Rc;

const STATE: &str = "first_recording.p2m2_SaveState.eeMemory.bin";
/// The rider's body actor in that state.
const ACTOR: u32 = 0x0159_1c10;
const COLLISIONS: u32 = 0x624;
const PARTS: u32 = 0x2f4;
const CASES: usize = 10_000;
const BUDGET: u64 = 2_000_000;

/// A bone collision's fields (research/oracle.txt, S52).
mod at {
    pub const ON: u32 = 0x04;
    pub const TOUCHED: u32 = 0x14;
    pub const GROUND: u32 = 0x18;
    pub const SCENE: u32 = 0x1c;
    pub const SWEPT: u32 = 0x20;
    pub const SWEEP: u32 = 0x24;
    pub const SWEEP_BACK: u32 = 0x28;
    pub const UNPENETRATION: u32 = 0x2c;
    pub const ROTATION: u32 = 0x30;
    pub const RESTITUTION: u32 = 0x34;
    pub const FRICTION: u32 = 0x38;
    pub const POINT: u32 = 0x40;
    pub const NORMAL: u32 = 0x50;
    pub const DV: u32 = 0x60;
    pub const TERRAIN: u32 = 0x78;
    pub const BODY: u32 = 0x7c;
    pub const BODY2: u32 = 0x80;
    pub const SPHERE: u32 = 0x88;
    pub const SEGMENT: u32 = 0x8c;
}

/// The body's fields the solve reads and writes.
mod body_at {
    pub const ENABLED: u32 = 0x00;
    pub const INV_MASS: u32 = 0x10;
    pub const RADIUS: u32 = 0x14;
    pub const POSITION: u32 = 0x60;
    pub const VELOCITY: u32 = 0x70;
    pub const INV_INERTIA: u32 = 0x120;
    pub const QUAT: u32 = 0x170;
    pub const RATES: u32 = 0x1b0;
}

/// A collider's hit flag (bit 5 of the byte at `+0x116`) and its record.
const COLLIDER_FLAGS: u32 = 0x116;
const COLLIDER_RECORD: u32 = 0x6c;
/// The terrain's box.
const BOX_MIN: u32 = 0x30;
const BOX_MAX: u32 = 0x40;

fn state() -> Option<Ee> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: `./kit state ram` of first_recording.p2m2_SaveState.p2s", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    assert_eq!(ee.u32_at(ACTOR + 0x620), 9, "no rider body actor at {ACTOR:#x}");
    Some(ee)
}

/// `rider\anim\mrider.phy`, out of the first rider's archive.
fn rider_file() -> Option<Vec<u8>> {
    let kit = Kit::load().ok()?;
    let path = kit.ctx("iso").join("MX.PSR");
    let Ok(mut f) = File::open(&path) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", path.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find("Rider\\thq1.res").expect("thq1.res is in MX.PSR");
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    let res = Res::parse(&bytes).expect("thq1.res");
    let e = res
        .entries
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case("rider\\anim\\mrider.phy"))
        .expect("the rider's archive carries mrider.phy");
    Some(res.read(&bytes, e).expect("its bytes"))
}

fn collision(ee: &Ee, i: usize) -> u32 {
    ee.u32_at(ee.u32_at(ACTOR + COLLISIONS) + 4 * i as u32)
}

fn bone_body(ee: &Ee, i: usize) -> u32 {
    ee.u32_at(ee.u32_at(ACTOR + PARTS) + 0xb0 * i as u32 + 0xa4)
}

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| ee.f32_at(a + k)))
}

fn set_quad(ee: &mut Ee, a: u32, v: Vf) {
    for k in 0..4 {
        ee.set_f32(a + 4 * k as u32, v[k]);
    }
}

fn f(ee: &Ee, a: u32) -> F {
    F(ee.f32_at(a))
}

fn curve(ee: &Ee) -> CurveTable {
    let base = ee.u32_at(addr("curve_table_ptr").unwrap());
    let entries: Vec<[f32; 2]> =
        (0..CURVE_ENTRIES as u32).map(|k| [ee.f32_at(base + 8 * k), ee.f32_at(base + 8 * k + 4)]).collect();
    CurveTable::from_entries(&entries)
}

fn bits(v: Vf) -> [u32; 4] {
    v.to_bits()
}

#[test]
fn the_spheres_are_the_live_collisions() {
    let (Some(ee), Some(bytes)) = (state(), rider_file()) else { return };
    let spheres = Sphere::read_bytes(&bytes);
    assert_eq!(spheres.len(), 9);
    for (i, s) in spheres.iter().enumerate() {
        let c = collision(&ee, i);
        let b = ee.u32_at(c + at::BODY);
        assert_eq!(b, bone_body(&ee, s.bone), "collision {i}'s bone");
        assert_eq!(ee.u32_at(c + at::BODY2), b, "collision {i} pushes its own bone");
        let same = |what: &str, got: F, k: u32| {
            assert_eq!(got.to_bits(), ee.u32_at(k), "collision {i}'s {what}");
        };
        same("radius", s.radius, b + body_at::RADIUS);
        same("unpenetration", s.unpenetration, c + at::UNPENETRATION);
        same("rotation", s.rotation, c + at::ROTATION);
        same("restitution", s.restitution, c + at::RESTITUTION);
        same("friction", s.friction, c + at::FRICTION);
        same("sweep back", s.sweep_back, c + at::SWEEP_BACK);
        assert_eq!(s.sweep, ee.u32_at(c + at::SWEEP) != 0, "collision {i}'s sweep");
        assert_ne!(ee.u32_at(c + at::SPHERE), 0);
        assert_ne!(ee.u32_at(c + at::SEGMENT), 0);
        // The impulse's scratch: its `w` is the base every sum starts from, which the port takes as 0.
        assert_eq!(ee.u32_at(c + 0x9c), 0, "collision {i}'s scratch w");
    }
}

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
    /// Mostly in `±x`, now and then exactly zero, a negative zero or `-x`.
    fn float(&mut self, x: f32) -> f32 {
        match self.upto(16) {
            0 => 0.0,
            1 => -0.0,
            2 => -x,
            _ => self.range(-x, x),
        }
    }
    fn positive(&mut self, x: f32) -> f32 {
        match self.upto(40) {
            0 => 0.0,
            1 => -self.range(0.0, x),
            _ => self.range(0.01, 2.0 * x),
        }
    }
    fn vector(&mut self, x: f32) -> Vf {
        Vf::new(self.float(x), self.float(x), self.float(x), self.float(2.0))
    }
    fn unit_vector(&mut self) -> Vf {
        let v = [self.float(1.0), self.float(1.0), self.float(1.0)];
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3);
        Vf::new(v[0] / n, v[1] / n, v[2] / n, self.float(2.0))
    }
    fn quat(&mut self) -> Vf {
        let v = [self.float(1.0), self.float(1.0), self.float(1.0), self.float(1.0)];
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3);
        Vf::new(v[0] / n, v[1] / n, v[2] / n, v[3] / n)
    }
    /// A contact near the bone: a point about a radius off its centre, and a normal.
    fn contact(&mut self, centre: Vf, radius: f32) -> (Vf, Vf) {
        let d = self.unit_vector();
        let r = radius * self.range(-0.5, 1.5);
        let p = Vf::new(centre.x() + d.x() * r, centre.y() + d.y() * r, centre.z() + d.z() * r, self.float(2.0));
        let n = if self.chance(4) { self.unit_vector() } else { Vf::new(-d.x(), -d.y(), -d.z(), self.float(2.0)) };
        (p, n)
    }
}

/// One case's answers, and what each side was asked.
#[derive(Clone, Debug, Default, PartialEq)]
struct Answers {
    ground: (Vf, Vf),
    touching: Option<Vec<(Vf, Vf)>>,
    sweep: Option<(Vf, Vf)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Asked {
    ground: Vec<[u32; 4]>,
    centre: Vec<[u32; 4]>,
    segment: Vec<([u32; 4], [u32; 4])>,
}

struct Fake {
    answers: Answers,
    asked: Asked,
}

impl World for Fake {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.asked.ground.push(bits(at));
        self.answers.ground
    }
    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

struct FakeScene<'a>(&'a RefCell<Fake>);

impl Scene for FakeScene<'_> {
    fn touching(&mut self, centre: Vf, _radius: F) -> Option<Vec<(Vf, Vf)>> {
        let mut f = self.0.borrow_mut();
        f.asked.centre.push(bits(centre));
        f.answers.touching.clone()
    }
    fn sweep(&mut self, from: Vf, to: Vf) -> Option<(Vf, Vf)> {
        let mut f = self.0.borrow_mut();
        f.asked.segment.push((bits(from), bits(to)));
        f.answers.sweep
    }
}

struct WorldRef<'a>(&'a RefCell<Fake>);

impl World for WorldRef<'_> {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.0.borrow_mut().ground(at)
    }
    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

fn set_hit(ee: &mut Ee, collider: u32, hit: bool) {
    let b = ee.u8_at(collider + COLLIDER_FLAGS);
    let b = if hit { b | 0x20 } else { b & !0x20 };
    ee.write(collider + COLLIDER_FLAGS, 1, u128::from(b));
}

/// Hook the solve's calls out: they answer `answers` and log into `asked`.
fn hook(ee: &mut Ee, shared: &Rc<RefCell<(Answers, Asked)>>, segments: Vec<u32>) {
    let s = Rc::clone(shared);
    ee.hooks.insert(
        addr("ground_query").unwrap(),
        Box::new(move |ee: &mut Ee| {
            let (point, normal) = (ee.r(5) as u32, ee.r(6) as u32);
            let mut s = s.borrow_mut();
            s.1.ground.push(bits(quad(ee, point)));
            set_quad(ee, point, s.0.ground.0);
            set_quad(ee, normal, s.0.ground.1);
        }),
    );
    let s = Rc::clone(shared);
    ee.hooks.insert(
        addr("collider_set_position").unwrap(),
        Box::new(move |ee: &mut Ee| {
            let at = quad(ee, ee.r(5) as u32);
            s.borrow_mut().1.centre.push(bits(at));
        }),
    );
    let s = Rc::clone(shared);
    ee.hooks.insert(
        addr("probe_bundle_set_points").unwrap(),
        Box::new(move |ee: &mut Ee| {
            let p = ee.r(5) as u32;
            let (from, to) = (quad(ee, p), quad(ee, p + 0x10));
            s.borrow_mut().1.segment.push((bits(from), bits(to)));
        }),
    );
    ee.hooks.insert(addr("probe_object_moved").unwrap(), Box::new(|_| {}));
    let collide = |s: Rc<RefCell<(Answers, Asked)>>, segments: Vec<u32>| {
        Box::new(move |ee: &mut Ee| {
            let c = ee.r(4) as u32;
            let rec = ee.u32_at(c + COLLIDER_RECORD);
            let s = s.borrow();
            // The segment's record holds one hit, the sphere's up to three: whichever this collider
            // is, the case's answers go where the solve reads them.
            if segments.contains(&c) {
                let hit = s.0.sweep;
                set_hit(ee, c, hit.is_some());
                if let Some((p, n)) = hit {
                    set_quad(ee, rec + 0x30, p);
                    set_quad(ee, rec + 0x20, n);
                }
            } else {
                let hits = s.0.touching.clone();
                set_hit(ee, c, hits.is_some());
                let hits = hits.unwrap_or_default();
                ee.set_u32(rec + 4, hits.len() as u32);
                for (i, (p, n)) in hits.iter().enumerate() {
                    set_quad(ee, rec + 0x60 + 0x10 * i as u32, *p);
                    set_quad(ee, rec + 0x30 + 0x10 * i as u32, *n);
                    ee.set_u32(rec + 0x90 + 4 * i as u32, 0);
                }
            }
        }) as Box<dyn FnMut(&mut Ee)>
    };
    ee.hooks.insert(addr("collide_against_search").unwrap(), collide(Rc::clone(shared), segments.clone()));
    ee.hooks.insert(addr("probe_collide_world").unwrap(), collide(Rc::clone(shared), segments));
}

fn body_of(ee: &Ee, b: u32) -> Body {
    Body {
        enabled: ee.u32_at(b + body_at::ENABLED) != 0,
        inv_mass: f(ee, b + body_at::INV_MASS),
        position: quad(ee, b + body_at::POSITION),
        velocity: quad(ee, b + body_at::VELOCITY),
        inv_inertia: quad(ee, b + body_at::INV_INERTIA),
        quat: quad(ee, b + body_at::QUAT),
        rates: quad(ee, b + body_at::RATES),
        ..Body::default()
    }
}

fn contact_of(ee: &Ee, c: u32) -> Contact {
    Contact {
        touched: ee.u32_at(c + at::TOUCHED) != 0,
        ground: ee.u32_at(c + at::GROUND) != 0,
        scene: ee.u32_at(c + at::SCENE) != 0,
        swept: ee.u32_at(c + at::SWEPT) != 0,
        point: quad(ee, c + at::POINT),
        normal: quad(ee, c + at::NORMAL),
        dv: quad(ee, c + at::DV),
    }
}

fn sphere_of(ee: &Ee, c: u32, bone: usize) -> Sphere {
    Sphere {
        bone,
        radius: f(ee, ee.u32_at(c + at::BODY) + body_at::RADIUS),
        unpenetration: f(ee, c + at::UNPENETRATION),
        rotation: f(ee, c + at::ROTATION),
        restitution: f(ee, c + at::RESTITUTION),
        friction: f(ee, c + at::FRICTION),
        sweep: ee.u32_at(c + at::SWEEP) != 0,
        sweep_back: f(ee, c + at::SWEEP_BACK),
    }
}

/// What the cases covered, so a test that passes on nothing fails instead.
#[derive(Default, Debug)]
struct Seen {
    ground: usize,
    scene: usize,
    swept: usize,
    off: usize,
}

#[test]
fn the_solve_matches_the_original() {
    let Some(mut ee) = state() else { return };
    let table = curve(&ee);
    let solve = addr("bone_collision_solve").unwrap();
    // The solve reaches its collision class through the vtable: make sure that is the one ported.
    let c0 = collision(&ee, 0);
    assert_eq!(ee.u32_at(ee.u32_at(c0 + 0x10) + 0x18), solve);
    // The sphere collider's refresh, through its own vtable, is hooked out with the rest.
    let refresh = ee.u32_at(ee.u32_at(ee.u32_at(c0 + at::SPHERE) + 0x38) + 0xa8);
    ee.hooks.insert(refresh, Box::new(|_| {}));
    // The collide hook tells the two colliders apart by address.
    let mut segments = Vec::new();
    for i in 0..9 {
        let c = collision(&ee, i);
        let seg = ee.u32_at(c + at::SEGMENT);
        let sph = ee.u32_at(c + at::SPHERE);
        segments.push(seg);
        assert_ne!(ee.u32_at(c + 0x84), 0, "collision {i} collides against its search pool");
        assert_ne!(ee.u32_at(sph + COLLIDER_RECORD), 0);
        assert_ne!(ee.u32_at(seg + COLLIDER_RECORD), 0);
    }
    let shared = Rc::new(RefCell::new((Answers::default(), Asked::default())));
    hook(&mut ee, &shared, segments);
    let terrain = ee.u32_at(c0 + at::TERRAIN);
    assert_ne!(terrain, 0);

    let mut pick = Pick(0x52_c0_11_15_10_4e);
    let mut seen = Seen::default();
    for case in 0..CASES {
        let i = pick.upto(9) as usize;
        let c = collision(&ee, i);
        let b = ee.u32_at(c + at::BODY);
        let bone = (0..18).find(|&k| bone_body(&ee, k) == b).unwrap();

        // The bone.
        let pos = Vf::new(pick.range(10.0, 6000.0), pick.range(-20.0, 260.0), pick.range(10.0, 6000.0), pick.float(2.0));
        let radius = pick.positive(0.6);
        ee.set_u32(b + body_at::ENABLED, u32::from(!pick.chance(30)));
        // The masses stay physical: a negative inverse mass or inertia can make the lever's
        // denominator vanish, and the original then reads the curve table at a wild index.
        ee.set_f32(b + body_at::INV_MASS, pick.range(0.05, 6.0));
        ee.set_f32(b + body_at::RADIUS, radius);
        set_quad(&mut ee, b + body_at::POSITION, pos);
        set_quad(&mut ee, b + body_at::VELOCITY, pick.vector(40.0));
        let lane = |pick: &mut Pick| if pick.chance(10) { 0.0 } else { pick.range(0.01, 6.0) };
        let ii = Vf::new(lane(&mut pick), lane(&mut pick), lane(&mut pick), pick.float(2.0));
        set_quad(&mut ee, b + body_at::INV_INERTIA, ii);
        set_quad(&mut ee, b + body_at::QUAT, pick.quat());
        set_quad(&mut ee, b + body_at::RATES, pick.vector(10.0));

        // The collision's numbers and flags.
        ee.set_u32(c + at::ON, u32::from(!pick.chance(30)));
        for k in [at::TOUCHED, at::GROUND, at::SCENE, at::SWEPT] {
            ee.set_u32(c + k, pick.upto(2) as u32);
        }
        ee.set_u32(c + at::SWEEP, u32::from(!pick.chance(4)));
        ee.set_f32(c + at::SWEEP_BACK, if pick.chance(4) { pick.float(0.2) } else { SWEEP_BACK });
        ee.set_f32(c + at::UNPENETRATION, if pick.chance(3) { 1.0 } else { pick.float(1.5) });
        ee.set_f32(c + at::ROTATION, if pick.chance(3) { 1.0 } else { pick.float(1.5) });
        ee.set_f32(c + at::RESTITUTION, if pick.chance(2) { 0.0 } else { pick.float(1.0) });
        ee.set_f32(c + at::FRICTION, match pick.upto(3) {
            0 => 1.0,
            1 => 0.8,
            _ => pick.float(1.2),
        });
        set_quad(&mut ee, c + at::POINT, pick.vector(9.0));
        set_quad(&mut ee, c + at::NORMAL, pick.vector(1.0));
        set_quad(&mut ee, c + at::DV, pick.vector(9.0));

        // The terrain: none, over it, off it or on its edge.
        let on_terrain = !pick.chance(6);
        ee.set_u32(c + at::TERRAIN, if on_terrain { terrain } else { 0 });
        let mut lo = Vf::new(pos.x() - pick.range(0.0, 50.0), 0.0, pos.z() - pick.range(0.0, 50.0), 0.0);
        let mut hi = Vf::new(pos.x() + pick.range(0.0, 50.0), 300.0, pos.z() + pick.range(0.0, 50.0), 0.0);
        match pick.upto(10) {
            0 => lo = Vf::new(pos.x(), 0.0, lo.z(), 0.0),
            1 => hi = Vf::new(hi.x(), 300.0, pos.z(), 0.0),
            2 => lo = Vf::new(lo.x(), 0.0, pos.z() + 1.0, 0.0),
            3 => hi = Vf::new(pos.x() - 1.0, 300.0, hi.z(), 0.0),
            _ => {}
        }
        set_quad(&mut ee, terrain + BOX_MIN, lo);
        set_quad(&mut ee, terrain + BOX_MAX, hi);

        // What the world answers.
        let ground_y = pos.y() - radius * pick.range(-1.0, 2.0);
        let ground_n = if pick.chance(3) { Vf::new(0.0, 1.0, 0.0, 1.0) } else { pick.unit_vector() };
        let ground = (Vf::new(pos.x() + pick.float(0.01), ground_y, pos.z() + pick.float(0.01), pick.float(2.0)), ground_n);
        let touching = if pick.chance(3) {
            let n = pick.upto(4) as usize;
            Some((0..n).map(|_| pick.contact(pos, radius)).collect())
        } else {
            None
        };
        let sweep = if pick.chance(3) { Some(pick.contact(pos, radius)) } else { None };
        let contacts = touching.as_ref().map_or(0, Vec::len);
        let answers = Answers { ground, touching, sweep };

        // The port first, off the case as written.
        let sphere = sphere_of(&ee, c, bone);
        let mut body = body_of(&ee, b);
        let mut contact = contact_of(&ee, c);
        let on = ee.u32_at(c + at::ON) != 0;
        let fake = RefCell::new(Fake { answers: answers.clone(), asked: Asked::default() });
        {
            let mut w = WorldRef(&fake);
            let t = on_terrain.then(|| Terrain { world: &mut w as &mut dyn World, min: lo, max: hi });
            contact.solve(on, &sphere, &mut body, t, &mut FakeScene(&fake), &table);
        }
        let port_asked = fake.into_inner().asked;

        // Then the original.
        *shared.borrow_mut() = (answers, Asked::default());
        ee.call(solve, &[u64::from(c), 1], &[], BUDGET);
        let game_asked = shared.borrow().1.clone();
        let game = (body_of(&ee, b), contact_of(&ee, c));

        let ctx = || format!("case {case}: collision {i} (bone {bone})");
        assert_eq!(port_asked.ground, game_asked.ground, "{}: the ground query's point", ctx());
        assert_eq!(port_asked.centre, game_asked.centre, "{}: the sphere's centre", ctx());
        assert_eq!(port_asked.segment, game_asked.segment, "{}: the sweep's segment", ctx());
        for (what, got, want) in [
            ("position", body.position, game.0.position),
            ("velocity", body.velocity, game.0.velocity),
            ("quat", body.quat, game.0.quat),
            ("rates", body.rates, game.0.rates),
            ("point", contact.point, game.1.point),
            ("normal", contact.normal, game.1.normal),
            ("dv", contact.dv, game.1.dv),
        ] {
            assert_eq!(bits(got), bits(want), "{}: {what} is {want:?} in the game, {got:?} in the port", ctx());
        }
        let flags = |k: &Contact| (k.touched, k.ground, k.scene, k.swept);
        assert_eq!(flags(&contact), flags(&game.1), "{}: touched, ground, scene, swept", ctx());

        if !on || !body.enabled {
            seen.off += 1;
        } else {
            seen.ground += usize::from(!port_asked.ground.is_empty() && pos.y() < ground_y + radius);
            seen.scene += contacts;
            seen.swept += usize::from(contact.swept);
        }
    }
    eprintln!("{seen:?}");
    assert!(seen.ground > 2000 && seen.scene > 1000 && seen.swept > 1000 && seen.off > 300, "{seen:?}");
}

/// The bike box's fields (research/oracle.txt, S52).
mod box_at {
    pub const ON: u32 = 0x04;
    pub const OTHER: u32 = 0x14;
    pub const FRAME: u32 = 0x20;
    pub const CENTRE: u32 = 0x70;
    pub const HALF: u32 = 0x80;
    pub const FRAME_AT: u32 = 0x60;
    pub const CENTRE_AT: u32 = 0x90;
    pub const HALF_AT: u32 = 0x94;
    pub const CARRIED_AT: u32 = 0x98;
    pub const BODY: u32 = 0x9c;
}

/// A rotation's three rows out of a unit quaternion, and an origin.
fn frame(pick: &mut Pick, origin: Vf) -> Mat4 {
    let q = pick.quat();
    let (x, y, z, w) = (q.x(), q.y(), q.z(), q.w());
    let mut junk = || if pick.chance(2) { 0.0 } else { pick.float(2.0) };
    Mat4([
        Vf::new(1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + z * w), 2.0 * (x * z - y * w), junk()),
        Vf::new(2.0 * (x * y - z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + x * w), junk()),
        Vf::new(2.0 * (x * z + y * w), 2.0 * (y * z - x * w), 1.0 - 2.0 * (x * x + y * y), junk()),
        origin,
    ])
}

#[test]
fn the_bike_box_matches_the_original() {
    let Some(mut ee) = state() else { return };
    let solve = addr("bike_box_solve").unwrap();
    let r = ee.u32_at(ACTOR + 0x5f8);
    assert_eq!(ee.u32_at(r), 4, "the bike box is a type-4 constraint");
    assert_eq!(ee.u32_at(ee.u32_at(r + 0x10) + 0x18), solve);
    assert_eq!(ee.u32_at(r + box_at::OTHER), 0);
    let b = ee.u32_at(r + box_at::BODY);
    assert_eq!(b, bone_body(&ee, 0), "the bike box holds the main bone");
    // The actor's moving frame, as the game wired it.
    let live = [box_at::FRAME_AT, box_at::CENTRE_AT, box_at::HALF_AT, box_at::CARRIED_AT].map(|k| ee.u32_at(r + k));
    assert_eq!(live, [0x580, 0x540, 0x550, 0x530].map(|k| ACTOR + k));

    let mut pick = Pick(0xb1_6e_b0_c5_52);
    let (mut held, mut free, mut moving, mut still, mut off) = (0, 0, 0, 0, 0);
    for case in 0..CASES {
        let on = !pick.chance(30);
        let enabled = !pick.chance(30);
        let carried = !pick.chance(3);
        ee.set_u32(r + box_at::ON, u32::from(on));
        ee.set_u32(b + body_at::ENABLED, u32::from(enabled));

        // Now and then a square frame on whole numbers and a box in sixteenths, with the bone exactly
        // on one face: the frame then rounds nothing, so the two compares meet their equal case.
        let face = pick.chance(6);
        let sixteenths = |pick: &mut Pick, n: u64| (pick.upto(2 * n + 1) as f32 - n as f32) / 16.0;
        let origin = if face {
            Vf::new(pick.upto(6000) as f32, pick.upto(260) as f32, pick.upto(6000) as f32, 1.0)
        } else {
            Vf::new(pick.range(10.0, 6000.0), pick.range(-20.0, 260.0), pick.range(10.0, 6000.0), 1.0)
        };
        let m = if face {
            Mat4([Vf::new(1.0, 0.0, 0.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0), Vf::new(0.0, 0.0, 1.0, 0.0), origin])
        } else {
            frame(&mut pick, origin)
        };
        let (centre, half) = if face {
            let c = Vf::new(sixteenths(&mut pick, 48), sixteenths(&mut pick, 48), sixteenths(&mut pick, 48), 1.0);
            let h = Vf::new(sixteenths(&mut pick, 32).abs(), sixteenths(&mut pick, 32).abs(), sixteenths(&mut pick, 32).abs(), 1.0);
            (c, h)
        } else if pick.chance(4) {
            (Vf::new(0.0, 4.3, -1.5, 1.0), Vf::new(0.45, 0.6, 1.75, 1.0))
        } else {
            (pick.vector(3.0), Vf::new(pick.positive(1.0), pick.positive(1.0), pick.positive(1.5), pick.float(2.0)))
        };
        // The bone: somewhere about the box, often inside it and often a little way out of it.
        let local = |pick: &mut Pick, c: f32, h: f32| c + h.abs() * pick.range(-1.6, 1.6) + pick.float(0.05);
        let mut l = [local(&mut pick, centre.x(), half.x()), local(&mut pick, centre.y(), half.y()), local(&mut pick, centre.z(), half.z())];
        if face {
            let k = pick.upto(3) as usize;
            l[k] = if pick.chance(2) { centre[k] + half[k] } else { centre[k] - half[k] };
            for k in 0..3 {
                if pick.chance(2) {
                    l[k] = centre[k] + sixteenths(&mut pick, 40);
                }
            }
        }
        let row = |k: usize| m.0[k];
        let pos = Vf::new(
            origin.x() + row(0).x() * l[0] + row(1).x() * l[1] + row(2).x() * l[2],
            origin.y() + row(0).y() * l[0] + row(1).y() * l[1] + row(2).y() * l[2],
            origin.z() + row(0).z() * l[0] + row(1).z() * l[1] + row(2).z() * l[2],
            pick.float(2.0),
        );
        let vel = pick.vector(60.0);
        let bike_vel = pick.vector(60.0);
        set_quad(&mut ee, b + body_at::POSITION, pos);
        set_quad(&mut ee, b + body_at::VELOCITY, vel);

        // Either the actor's frame through the four pointers, or the constraint's own.
        if carried {
            for (k, v) in m.0.iter().enumerate() {
                set_quad(&mut ee, ACTOR + 0x580 + 0x10 * k as u32, *v);
            }
            set_quad(&mut ee, ACTOR + 0x540, centre);
            set_quad(&mut ee, ACTOR + 0x550, half);
            set_quad(&mut ee, ACTOR + 0x530, bike_vel);
            for (k, a) in [box_at::FRAME_AT, box_at::CENTRE_AT, box_at::HALF_AT, box_at::CARRIED_AT].iter().zip(live) {
                ee.set_u32(r + k, a);
            }
        } else {
            for (k, v) in m.0.iter().enumerate() {
                set_quad(&mut ee, r + box_at::FRAME + 0x10 * k as u32, *v);
            }
            set_quad(&mut ee, r + box_at::CENTRE, centre);
            set_quad(&mut ee, r + box_at::HALF, half);
            // Any of the first three pointers missing sends it to its own frame.
            let gone = [box_at::FRAME_AT, box_at::CENTRE_AT, box_at::HALF_AT][pick.upto(3) as usize];
            for (k, a) in [box_at::FRAME_AT, box_at::CENTRE_AT, box_at::HALF_AT, box_at::CARRIED_AT].iter().zip(live) {
                ee.set_u32(r + k, if *k == gone || (pick.chance(2) && *k != box_at::CARRIED_AT) { 0 } else { a });
            }
            ee.set_u32(r + gone, 0);
        }

        let bike_box = BikeBox { frame: m, centre, half, carried: carried.then_some(bike_vel) };
        let mut body = Body { enabled, position: pos, velocity: vel, ..Body::default() };
        bike_box.hold(on, &mut body);

        ee.call(solve, &[u64::from(r)], &[], BUDGET);
        let ctx = || format!("case {case}: on {on}, enabled {enabled}, carried {carried}, local {l:?}, half {half:?}");
        assert_eq!(bits(body.position), bits(quad(&ee, b + body_at::POSITION)), "{}: position", ctx());
        assert_eq!(bits(body.velocity), bits(quad(&ee, b + body_at::VELOCITY)), "{}: velocity", ctx());

        if !on || !enabled {
            off += 1;
        } else {
            if carried {
                moving += 1;
            } else {
                still += 1;
            }
            // Out of the box on some axis, as the case placed it (a bone right on a face may go
            // either way through the frame's rounding, and is counted as the case meant it).
            let out = (0..3).any(|k| {
                let d = l[k] - centre[k];
                !(d <= half[k]) || d < -half[k]
            });
            if out {
                held += 1;
            } else {
                free += 1;
            }
        }
    }
    eprintln!("bike box: {held} held, {free} free, {moving} moving, {still} still, {off} off");
    assert!(held > 3000 && free > 300 && moving > 3000 && still > 1500 && off > 300, "{held} {free} {moving} {still} {off}");
}

/// Every quadword of a body the reset reads or writes, and its start.
mod reset_at {
    pub const START_POSITION: u32 = 0x30;
    pub const START_VELOCITY: u32 = 0x40;
    pub const START_RATES: u32 = 0x140;
    pub const START_QUAT: u32 = 0x160;
    pub const LAST_POSITION: u32 = 0x50;
    pub const LAST_FORCE: u32 = 0x80;
    pub const FORCE: u32 = 0x90;
    pub const ACCEL: u32 = 0xa0;
    pub const SHOCK_PEAK: u32 = 0xb0;
    pub const SHOCK: u32 = 0xc0;
    pub const LAST_QUAT: u32 = 0x150;
    pub const MOMENT: u32 = 0x190;
    pub const LAST_MOMENT: u32 = 0x1a0;
    pub const ANG_ACCEL: u32 = 0x1c0;
    pub const TWIST_PEAK: u32 = 0x1d0;
    pub const TWIST: u32 = 0x1e0;
}

/// A body's state as the reset sees it, every quadword by name.
fn whole_body(ee: &Ee, b: u32) -> Body {
    use reset_at::*;
    Body {
        last_position: quad(ee, b + LAST_POSITION),
        last_force: quad(ee, b + LAST_FORCE),
        force: quad(ee, b + FORCE),
        accel: quad(ee, b + ACCEL),
        shock_peak: quad(ee, b + SHOCK_PEAK),
        shock: quad(ee, b + SHOCK),
        last_quat: quad(ee, b + LAST_QUAT),
        moment: quad(ee, b + MOMENT),
        last_moment: quad(ee, b + LAST_MOMENT),
        ang_accel: quad(ee, b + ANG_ACCEL),
        twist_peak: quad(ee, b + TWIST_PEAK),
        twist: quad(ee, b + TWIST),
        ..body_of(ee, b)
    }
}

fn start_of(ee: &Ee, b: u32) -> Start {
    use reset_at::*;
    Start {
        position: quad(ee, b + START_POSITION),
        velocity: quad(ee, b + START_VELOCITY),
        quat: quad(ee, b + START_QUAT),
        rates: quad(ee, b + START_RATES),
    }
}

#[test]
fn the_reset_matches_the_original() {
    let Some(mut ee) = state() else { return };
    let reset = addr("rider_ragdoll_reset").unwrap();
    let world = ee.u32_at(ACTOR + 0x2cc);
    let count = ee.u32_at(world + 8) as usize;
    assert_eq!(count, 18, "the ragdoll's world holds the eighteen bones");
    let list = ee.u32_at(world + 0x10);
    let bodies: Vec<u32> = (0..count).map(|k| ee.u32_at(list + 4 * k as u32)).collect();
    for (k, &b) in bodies.iter().enumerate() {
        assert_eq!(b, bone_body(&ee, k), "the world's body {k} is bone {k}'s");
        assert_eq!(ee.u32_at(ee.u32_at(b + 0x100) + 0x2c), addr("body_reset").unwrap());
        assert_eq!(ee.u32_at(ee.u32_at(b + 0x100) + 0x10), addr("body_set_start").unwrap());
        assert_eq!(ee.u32_at(ee.u32_at(b + 0x100) + 0x48), addr("body_set_start_turn").unwrap());
    }
    // Every quadword of the body from its inverse mass to the twist, generated: what the reset leaves
    // alone must come out as it went in, on both sides.
    let quads: Vec<u32> = (1..0x1f).map(|k| 0x10 * k).filter(|&o| o != 0xd0 && o != 0xe0 && o != 0x100).collect();

    let mut pick = Pick(0x7e_5e_70_52);
    for case in 0..CASES / 10 {
        let mut ours = Vec::new();
        for &b in &bodies {
            for &o in &quads {
                set_quad(&mut ee, b + o, pick.vector(500.0));
            }
            ours.push((whole_body(&ee, b), start_of(&ee, b)));
        }
        ragdoll_pose::reset(ours.iter_mut().map(|(b, s)| (b, &*s)));
        ee.call(reset, &[u64::from(ACTOR)], &[], BUDGET);
        for (k, &b) in bodies.iter().enumerate() {
            let (port, start) = &ours[k];
            assert_eq!(*start, start_of(&ee, b), "case {case}: body {k}'s start moved");
            let game = whole_body(&ee, b);
            assert_eq!(format!("{port:?}"), format!("{game:?}"), "case {case}: body {k}");
            for (what, got, want) in [
                ("position", port.position, game.position),
                ("last position", port.last_position, game.last_position),
                ("velocity", port.velocity, game.velocity),
                ("quat", port.quat, game.quat),
                ("last quat", port.last_quat, game.last_quat),
                ("rates", port.rates, game.rates),
                ("force", port.force, game.force),
                ("twist", port.twist, game.twist),
            ] {
                assert_eq!(bits(got), bits(want), "case {case}: body {k}'s {what}");
            }
        }
    }

    // The two setters, each handed two of the body's own quadwords as its arguments (never the start
    // itself, which the first copy would overwrite before the second reads it).
    let (place, turn) = (addr("body_set_start").unwrap(), addr("body_set_start_turn").unwrap());
    let sources: Vec<u32> = quads.iter().copied().filter(|o| ![0x30, 0x40, 0x140, 0x160].contains(o)).collect();
    for case in 0..CASES {
        let b = bodies[pick.upto(18) as usize];
        for &o in &quads {
            set_quad(&mut ee, b + o, pick.vector(500.0));
        }
        let n = sources.len() as u64;
        let (p, v) = (sources[pick.upto(n) as usize], sources[pick.upto(n) as usize]);
        let mut start = start_of(&ee, b);
        let (pv, vv) = (quad(&ee, b + p), quad(&ee, b + v));
        if pick.chance(2) {
            start.place(pv, vv);
            ee.call(place, &[u64::from(b), u64::from(b + p), u64::from(b + v)], &[], BUDGET);
        } else {
            start.turn(pv, vv);
            ee.call(turn, &[u64::from(b), u64::from(b + p), u64::from(b + v)], &[], BUDGET);
        }
        assert_eq!(format!("{start:?}"), format!("{:?}", start_of(&ee, b)), "case {case}: offsets {p:#x}, {v:#x}");
    }
}
