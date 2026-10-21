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
//!    of every body generated, and the body's two start setters;
//! 5. **the pose**: the actor's own pose over all eighteen bones, the skeleton's two reads hooked to
//!    answer generated bone positions and axes, everything after them run for real.

use game_sim::archive::{Psr, Res};
use game_sim::air::AcosTable;
use game_sim::biplane::Body;
use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::collide::{Bundle, Record};
use game_sim::math::Mat4;
use game_sim::ragdoll_pose::{self, Bone, Start};
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
    /// The collidable each touching contact is with (S62), 0 for none.
    others: Vec<u32>,
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
                    ee.set_u32(rec + 0x90 + 4 * i as u32, s.0.others.get(i).copied().unwrap_or(0));
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
        let answers = Answers { ground, touching, sweep, others: Vec::new() };

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

/// A part's drawn matrix, at `+0x60` of its record.
fn drawn_of(ee: &Ee, part: u32) -> Mat4 {
    Mat4([0, 0x10, 0x20, 0x30].map(|k| quad(ee, part + 0x60 + k)))
}

/// The bone's two axes out of an orientation, as the skeleton's read hands them: the `z` row forward,
/// the `y` row up, both with `w = 0`.
fn axes(m: &Mat4) -> (Vf, Vf) {
    let w0 = |v: Vf| Vf::new(v.x(), v.y(), v.z(), 0.0);
    (w0(m.0[2]), w0(m.0[1]))
}

/// `q` nudged by up to `by` in each lane and renormalised: a bone that turned a little since the last
/// frame.
fn nudge(pick: &mut Pick, q: Vf, by: f32) -> Vf {
    let v = [q.x(), q.y(), q.z(), q.w()].map(|x| x + pick.range(-by, by));
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3);
    Vf::new(v[0] / n, v[1] / n, v[2] / n, v[3] / n)
}

fn rotation(q: Vf, origin: Vf) -> Mat4 {
    let (x, y, z, w) = (q.x(), q.y(), q.z(), q.w());
    Mat4([
        Vf::new(1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + z * w), 2.0 * (x * z - y * w), 0.0),
        Vf::new(2.0 * (x * y - z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + x * w), 0.0),
        Vf::new(2.0 * (x * z + y * w), 2.0 * (y * z - x * w), 1.0 - 2.0 * (x * x + y * y), 0.0),
        origin,
    ])
}

/// The ragdoll's pose against the actor's own (its vt `+0xa4`), over every bone at once. The two reads
/// of the skeleton are hooked: each answers the generated bone for the node it is handed, and checks
/// that the nodes come in the parts' order. The quaternion, its axis and angle, the drawn matrix and
/// the two setters run for real.
///
/// The cases mix bones that turned a little since the drawn matrix (the frame-to-frame case the
/// velocity and rates are for), bones that did not turn at all (no angle, the helper's constant axis),
/// and bones with no relation to their drawn matrix, on both sides of `w`; and frame times that are
/// a frame, anything, zero and below zero, with and without the still flag.
#[test]
fn the_pose_matches_the_original() {
    let Some(mut ee) = state() else { return };
    let pose = addr("rider_ragdoll_pose").unwrap();
    let (place_at, axes_at) = (addr("node_world_position").unwrap(), addr("orientation_axes").unwrap());
    let (acos, curve) = (AcosTable::build(), curve(&ee));
    let parts = ee.u32_at(ACTOR + PARTS);
    let count = ee.u32_at(ACTOR + 0x2e0) as usize;
    assert_eq!(count, 18, "eighteen parts");
    let part = |k: usize| parts + 0xb0 * k as u32;
    let nodes: Vec<u32> = (0..count).map(|k| ee.u32_at(part(k) + 0xa0)).collect();
    assert!(nodes.iter().all(|&n| n != 0), "every part has its node");

    let bones = Rc::new(RefCell::new(vec![Bone::default(); count]));
    let calls = Rc::new(RefCell::new((0usize, 0usize)));
    {
        let (bones, calls, nodes) = (Rc::clone(&bones), Rc::clone(&calls), nodes.clone());
        ee.hooks.insert(
            place_at,
            Box::new(move |ee: &mut Ee| {
                let k = calls.borrow().0;
                calls.borrow_mut().0 += 1;
                assert_eq!((ee.r(4) as u32, ee.r(5)), (nodes[k], 0), "the pose reads bone {k}'s node in world space");
                set_quad(ee, ee.r(6) as u32, bones.borrow()[k].at);
            }),
        );
    }
    {
        let (bones, calls, nodes) = (Rc::clone(&bones), Rc::clone(&calls), nodes.clone());
        ee.hooks.insert(
            axes_at,
            Box::new(move |ee: &mut Ee| {
                let k = calls.borrow().1;
                calls.borrow_mut().1 += 1;
                assert_eq!((ee.r(4) as u32, ee.r(5)), (nodes[k], 0), "the pose reads bone {k}'s axes in world space");
                let b = bones.borrow()[k];
                set_quad(ee, ee.r(6) as u32, b.forward);
                set_quad(ee, ee.r(7) as u32, b.up);
            }),
        );
    }

    let mut pick = Pick(0x90_5e_52_a1);
    let (mut still, mut moving, mut no_turn, mut flipped, mut flipped_drawn) = (0, 0, 0, 0, 0);
    for case in 0..CASES / 10 {
        let dt = match pick.upto(8) {
            0 => 0.0,
            1 => -pick.range(0.0, 0.1),
            2 => pick.range(1e-4, 1.0),
            3 => 1.0 / 30.0,
            _ => 1.0 / 60.0,
        };
        let flag = pick.chance(6);
        let mut ours = Vec::new();
        for k in 0..count {
            let b = bone_body(&ee, k);
            let q = pick.quat();
            let at = pick.vector(400.0);
            let (last_q, last_at) = match pick.upto(4) {
                0 => (q, at),
                1 => (pick.quat(), pick.vector(400.0)),
                _ => (nudge(&mut pick, q, 0.05), Vf::new(at.x() + pick.float(2.0), at.y() + pick.float(2.0), at.z() + pick.float(2.0), pick.float(2.0))),
            };
            let (forward, up) = axes(&rotation(q, at));
            let bone = Bone { at, forward, up };
            let mut drawn = rotation(last_q, last_at);
            for row in 0..3 {
                if pick.chance(2) {
                    drawn.0[row] = Vf::new(drawn.0[row].x(), drawn.0[row].y(), drawn.0[row].z(), pick.float(2.0));
                }
            }
            for (r, v) in drawn.0.iter().enumerate() {
                set_quad(&mut ee, part(k) + 0x60 + 0x10 * r as u32, *v);
            }
            let cofg = if pick.chance(4) { Vf::new(0.0, 0.0, 0.0, 1.0) } else { pick.vector(1.5) };
            set_quad(&mut ee, b + 0x180, cofg);
            for o in [0x30, 0x40, 0x140, 0x160] {
                set_quad(&mut ee, b + o, pick.vector(500.0));
            }
            bones.borrow_mut()[k] = bone;
            ours.push((bone, cofg, drawn, start_of(&ee, b)));
            flipped += usize::from(q.w() < 0.0);
            flipped_drawn += usize::from(last_q.w() < 0.0);
        }
        *calls.borrow_mut() = (0, 0);
        ragdoll_pose::pose_all(
            ours.iter_mut().map(|(bone, cofg, drawn, start)| (&*bone, *cofg, drawn, start)),
            flag,
            F(dt),
            &acos,
            &curve,
        );
        ee.call(pose, &[u64::from(ACTOR), u64::from(flag)], &[dt], BUDGET);
        assert_eq!(*calls.borrow(), (count, count), "case {case}: one read of each per bone");
        for (k, (_, _, drawn, start)) in ours.iter().enumerate() {
            let game = start_of(&ee, bone_body(&ee, k));
            for (what, got, want) in [
                ("position", start.position, game.position),
                ("velocity", start.velocity, game.velocity),
                ("quat", start.quat, game.quat),
                ("rates", start.rates, game.rates),
            ] {
                assert_eq!(bits(got), bits(want), "case {case} (dt {dt}, still {flag}): bone {k}'s {what}");
            }
            let game = drawn_of(&ee, part(k));
            for r in 0..4 {
                assert_eq!(bits(drawn.0[r]), bits(game.0[r]), "case {case}: bone {k}'s drawn row {r}");
            }
            if flag || dt <= 0.0 {
                still += 1;
            } else if start.rates == Vf::new(0.0, 0.0, 0.0, 1.0) || bits(start.rates)[1..3] == [0, 0] {
                no_turn += 1;
            } else {
                moving += 1;
            }
        }
    }
    assert!(
        still > 3000 && moving > 6000 && no_turn > 200 && flipped > 4000 && flipped_drawn > 4000,
        "coverage: {still} still, {moving} moving, {no_turn} not turning, {flipped} flipped, {flipped_drawn} drawn flipped"
    );
}

// --- S62: contacts with things that move -----------------------------------------------------------
//
// The push-out measures the bone's velocity against whatever the contact is with. The contact's other
// (the collision's +0x70, a collidable) is generated in scratch memory in each of the forms the
// original tells apart: none; a collidable with nothing behind it; one whose owner is not a mover
// (class at +0x1b0 not 2); a mover (its two virtuals hooked to answer generated spins and velocities);
// a collision object (+0x80) on a dynamics body (+0xf4); and one on a vehicle's chassis (+0xf0, its
// transform +0xf8). Research/oracle.txt, S62.

use game_sim::contact::{Collider, Mover, Node, Other, Rigid, Side, Vehicle, Weighed};
use game_sim::ragdoll_contact::{Against, NodeBox, moving};

/// The other side's fields.
mod other_at {
    /// The collidable's owner (a mover when its class is 2) and its collision object.
    pub const OWNER: u32 = 0x7c;
    pub const OBJECT: u32 = 0x80;
    /// The collision object's centre of mass, in its body's frame and in the world, and its body,
    /// chassis and transform.
    pub const COFG: u32 = 0x90;
    pub const COFG_WORLD: u32 = 0xa0;
    pub const VEHICLE: u32 = 0xf0;
    pub const BODY: u32 = 0xf4;
    pub const NODE: u32 = 0xf8;
    /// A mover: its vtable, its class, its origin and its transform; the two slots the push-out
    /// calls; and where the test stashes what they answer (nothing reads `+0x200`/`+0x210`).
    pub const VTABLE: u32 = 0x08;
    pub const CLASS: u32 = 0x1b0;
    pub const ORIGIN: u32 = 0x190;
    pub const MOVER_NODE: u32 = 0x254;
    pub const VELOCITY_SLOT: u32 = 0xb8;
    pub const SPIN_SLOT: u32 = 0xbc;
    pub const SPIN: u32 = 0x200;
    pub const VELOCITY: u32 = 0x210;
    /// A chassis: centre of mass, velocity and angular velocity.
    pub const V_COFG: u32 = 0x190;
    pub const V_VELOCITY: u32 = 0x260;
    pub const V_RATES: u32 = 0x290;
    /// A transform: rows, origin, and the flag that says it is up to date.
    pub const ROWS: u32 = 0x80;
    pub const T_ORIGIN: u32 = 0xb0;
    pub const CLEAN: u32 = 0x110;
}

/// Where the cases put the other side, in the interpreter's scratch slots of 0x40.
struct Others {
    point: u32,
    normal: u32,
    /// Three collidables: on the mover, on the body's object, on the chassis' object.
    collidables: [u32; 3],
    objects: [u32; 2],
    body: u32,
    vehicle: u32,
    node: u32,
    mover: u32,
    mover_node: u32,
    vtable: u32,
    spin_hook: u32,
    velocity_hook: u32,
}

fn others(ee: &Ee) -> Others {
    Others {
        point: ee.scratch(0),
        normal: ee.scratch(1),
        collidables: [ee.scratch(2), ee.scratch(5), ee.scratch(8)],
        objects: [ee.scratch(11), ee.scratch(19)],
        body: ee.scratch(27),
        vehicle: ee.scratch(35),
        node: ee.scratch(46),
        mover: ee.scratch(51),
        mover_node: ee.scratch(61),
        vtable: ee.scratch(66),
        spin_hook: ee.scratch(70),
        velocity_hook: ee.scratch(71),
    }
}

fn zero(ee: &mut Ee, a: u32, n: u32) {
    for k in (0..n).step_by(4) {
        ee.set_u32(a + k, 0);
    }
}

fn write_node(ee: &mut Ee, n: u32, node: &Node) {
    use other_at::*;
    zero(ee, n, 0x120);
    for (k, r) in node.rows.iter().enumerate() {
        set_quad(ee, n + ROWS + 16 * k as u32, *r);
    }
    set_quad(ee, n + T_ORIGIN, node.origin);
    // Up to date: the transform does not rebuild itself from parents.
    ee.set_u32(n + CLEAN, 1);
}

impl Pick {
    /// A vector with any `w`.
    fn loose(&mut self, x: f32) -> Vf {
        Vf::new(self.range(-x, x), self.range(-x, x), self.range(-x, x), self.range(-2.0, 2.0))
    }
    /// A transform: a rotation, now and then scaled, the rows' `w`s anything (the transpose reads
    /// the third row's), somewhere about the bone.
    fn node(&mut self, near: Vf) -> Node {
        let q = self.quat();
        let (x, y, z, w) = (q.x(), q.y(), q.z(), q.w());
        let s = if self.chance(4) { self.range(0.5, 2.0) } else { 1.0 };
        let mut w0 = || if self.chance(2) { 0.0 } else { self.range(-1.0, 1.0) };
        let rows = [
            Vf::new(s * (1.0 - 2.0 * (y * y + z * z)), s * 2.0 * (x * y + z * w), s * 2.0 * (x * z - y * w), w0()),
            Vf::new(s * 2.0 * (x * y - z * w), s * (1.0 - 2.0 * (x * x + z * z)), s * 2.0 * (y * z + x * w), w0()),
            Vf::new(s * 2.0 * (x * z + y * w), s * 2.0 * (y * z - x * w), s * (1.0 - 2.0 * (x * x + y * y)), w0()),
        ];
        let o = self.loose(4.0);
        Node { rows, origin: Vf::new(near.x() + o.x(), near.y() + o.y(), near.z() + o.z(), 1.0) }
    }
}

/// The mover's two virtuals: each answers what the test stashed in the mover.
fn hook_mover(ee: &mut Ee, o: &Others) {
    for (hook, from) in [(o.spin_hook, other_at::SPIN), (o.velocity_hook, other_at::VELOCITY)] {
        ee.hooks.insert(hook, Box::new(move |ee: &mut Ee| {
            let (out, m) = (ee.r(4) as u32, ee.r(5) as u32);
            let v = quad(ee, m + from);
            set_quad(ee, out, v);
        }));
    }
}

/// The port's copy of the other side, read back off the generated RAM.
struct PortOthers {
    mover: Mover,
    /// The mover's class is 2 (it is a mover at all).
    is_mover: bool,
    body: Body,
    vehicle: Vehicle,
    objects: [Collider; 2],
    /// Which collidables carry what: the mover's has its owner, the objects' theirs.
    owner_on_object: bool,
}

impl PortOthers {
    /// The velocity at `at` of the `k`-th collidable, as [`moving`] has it.
    fn velocity(&mut self, k: usize, at: Vf) -> Vf {
        let still = Vf::new(0.0, 0.0, 0.0, 1.0);
        match k {
            0 if self.is_mover => moving(at, Against::Mover(&self.mover)),
            0 => still,
            1 => {
                let mut w = Weighed { body: &mut self.body, mass: F::ONE };
                let mut o = Other { collider: &mut self.objects[0], side: Side::Body(&mut w as &mut dyn Rigid) };
                moving(at, Against::Other(&mut o))
            }
            _ => {
                let mut o = Other { collider: &mut self.objects[1], side: Side::Vehicle(&mut self.vehicle) };
                moving(at, Against::Other(&mut o))
            }
        }
    }
}

/// Generate the other side: the mover, the body, the chassis and their objects, about `near`.
fn write_others(ee: &mut Ee, pick: &mut Pick, o: &Others, near: Vf) -> PortOthers {
    use other_at::*;
    // The mover.
    zero(ee, o.mover, 0x280);
    ee.set_u32(o.mover + VTABLE, o.vtable);
    ee.set_u32(o.vtable + SPIN_SLOT, o.spin_hook);
    ee.set_u32(o.vtable + VELOCITY_SLOT, o.velocity_hook);
    let is_mover = !pick.chance(5);
    ee.set_u32(o.mover + CLASS, if is_mover { 2 } else { pick.upto(2) as u32 });
    let mover_node = pick.node(near);
    write_node(ee, o.mover_node, &mover_node);
    ee.set_u32(o.mover + MOVER_NODE, o.mover_node);
    let mover = Mover { spin: pick.loose(4.0), velocity: pick.loose(40.0), node: mover_node, origin: pick.loose(2.0) };
    set_quad(ee, o.mover + SPIN, mover.spin);
    set_quad(ee, o.mover + VELOCITY, mover.velocity);
    set_quad(ee, o.mover + ORIGIN, mover.origin);

    // The body, as the bone bodies are laid out.
    zero(ee, o.body, 0x200);
    let off = pick.loose(3.0);
    let body = Body {
        position: Vf::new(near.x() + off.x(), near.y() + off.y(), near.z() + off.z(), off.w()),
        velocity: pick.loose(40.0),
        quat: pick.quat(),
        rates: pick.loose(8.0),
        ..Body::default()
    };
    set_quad(ee, o.body + body_at::POSITION, body.position);
    set_quad(ee, o.body + body_at::VELOCITY, body.velocity);
    set_quad(ee, o.body + body_at::QUAT, body.quat);
    set_quad(ee, o.body + body_at::RATES, body.rates);

    // The chassis and its transform.
    zero(ee, o.vehicle, 0x2c0);
    let node = pick.node(near);
    write_node(ee, o.node, &node);
    let vehicle = Vehicle { node, cofg: pick.loose(2.0), velocity: pick.loose(40.0), rates: pick.loose(4.0), ..Vehicle::default() };
    set_quad(ee, o.vehicle + V_COFG, vehicle.cofg);
    set_quad(ee, o.vehicle + V_VELOCITY, vehicle.velocity);
    set_quad(ee, o.vehicle + V_RATES, vehicle.rates);

    // The two collision objects: one on the body, one on the chassis.
    let mut objects = [Collider::default(); 2];
    for (k, &r) in o.objects.iter().enumerate() {
        zero(ee, r, 0x200);
        objects[k].cofg = pick.loose(1.5);
        objects[k].cofg_world = pick.loose(1.5);
        set_quad(ee, r + COFG, objects[k].cofg);
        set_quad(ee, r + COFG_WORLD, objects[k].cofg_world);
    }
    ee.set_u32(o.objects[0] + BODY, o.body);
    ee.set_u32(o.objects[1] + VEHICLE, o.vehicle);
    ee.set_u32(o.objects[1] + NODE, o.node);

    // The collidables: the first is the mover's; the other two are the objects', and now and then
    // carry the mover as their owner too (the object wins).
    let owner_on_object = pick.chance(3);
    for (k, &c) in o.collidables.iter().enumerate() {
        zero(ee, c, 0xc0);
        if k == 0 || owner_on_object {
            ee.set_u32(c + OWNER, o.mover);
        }
        if k > 0 {
            ee.set_u32(c + OBJECT, o.objects[k - 1]);
        }
    }
    PortOthers { mover, is_mover, body, vehicle, objects, owner_on_object }
}

/// Every lane the push-out writes, the port's against the game's.
fn same_push(ctx: &str, ee: &Ee, b: u32, c: u32, body: &Body, contact: &Contact) {
    for (what, got, want) in [
        ("position", body.position, quad(ee, b + body_at::POSITION)),
        ("velocity", body.velocity, quad(ee, b + body_at::VELOCITY)),
        ("quat", body.quat, quad(ee, b + body_at::QUAT)),
        ("rates", body.rates, quad(ee, b + body_at::RATES)),
        ("dv", contact.dv, quad(ee, c + at::DV)),
        ("point", contact.point, quad(ee, c + at::POINT)),
        ("normal", contact.normal, quad(ee, c + at::NORMAL)),
    ] {
        assert_eq!(bits(got), bits(want), "{ctx}: {what} is {want:?} in the game, {got:?} in the port");
    }
    assert_eq!(contact.touched, ee.u32_at(c + at::TOUCHED) != 0, "{ctx}: touched");
}

/// Generate one bone and its collision's numbers, and read them back as the port's.
fn write_bone(ee: &mut Ee, pick: &mut Pick, c: u32, b: u32) -> Vf {
    let pos = Vf::new(pick.range(10.0, 6000.0), pick.range(-20.0, 260.0), pick.range(10.0, 6000.0), pick.float(2.0));
    ee.set_u32(b + body_at::ENABLED, 1);
    ee.set_f32(b + body_at::INV_MASS, pick.range(0.05, 6.0));
    ee.set_f32(b + body_at::RADIUS, pick.range(0.05, 1.2));
    set_quad(ee, b + body_at::POSITION, pos);
    set_quad(ee, b + body_at::VELOCITY, pick.vector(40.0));
    let lane = |pick: &mut Pick| if pick.chance(10) { 0.0 } else { pick.range(0.01, 6.0) };
    let ii = Vf::new(lane(pick), lane(pick), lane(pick), pick.float(2.0));
    set_quad(ee, b + body_at::INV_INERTIA, ii);
    set_quad(ee, b + body_at::QUAT, pick.quat());
    set_quad(ee, b + body_at::RATES, pick.vector(10.0));
    ee.set_u32(c + at::ON, 1);
    ee.set_f32(c + at::UNPENETRATION, if pick.chance(3) { 1.0 } else { pick.float(1.5) });
    ee.set_f32(c + at::ROTATION, if pick.chance(3) { 1.0 } else { pick.float(1.5) });
    ee.set_f32(c + at::RESTITUTION, if pick.chance(2) { 0.0 } else { pick.float(1.0) });
    ee.set_f32(c + at::FRICTION, if pick.chance(2) { 1.0 } else { pick.float(1.2) });
    set_quad(ee, c + at::DV, pick.vector(9.0));
    pos
}

/// The push-out against each kind of other matches the original's, every lane it writes, and the
/// other's centre of mass in the world it writes on the way.
#[test]
fn the_push_out_against_moving_things_matches_the_original() {
    let Some(mut ee) = state() else { return };
    let table = curve(&ee);
    let push_out = addr("bone_collision_push_out").unwrap();
    let o = others(&ee);
    hook_mover(&mut ee, &o);
    let mut pick = Pick(0x62_b0_e5_0f_f1);
    // none, a bare collidable, a non-mover owner, the mover, the body, the chassis
    let mut seen = [0usize; 6];
    let mut swept = 0;
    for case in 0..CASES {
        let i = pick.upto(9) as usize;
        let c = collision(&ee, i);
        let b = ee.u32_at(c + at::BODY);
        let bone = (0..18).find(|&k| bone_body(&ee, k) == b).unwrap();
        let pos = write_bone(&mut ee, &mut pick, c, b);
        let is_swept = pick.chance(4);
        ee.set_u32(c + at::SWEPT, u32::from(is_swept));
        let radius = ee.f32_at(b + body_at::RADIUS);
        let (point, normal) = pick.contact(pos, radius);
        set_quad(&mut ee, o.point, point);
        set_quad(&mut ee, o.normal, normal);
        let mut port = write_others(&mut ee, &mut pick, &o, pos);
        // What the contact is with.
        let kind = pick.upto(6) as usize;
        let other = match kind {
            0 => 0,
            1 => {
                ee.set_u32(o.collidables[0] + other_at::OWNER, 0);
                o.collidables[0]
            }
            2 => {
                ee.set_u32(o.mover + other_at::CLASS, 3);
                port.is_mover = false;
                o.collidables[0]
            }
            3 => o.collidables[0],
            4 => o.collidables[1],
            _ => o.collidables[2],
        };
        ee.set_u32(c + 0x70, other);
        let kind = if kind == 3 && !port.is_mover { 2 } else { kind };
        seen[kind] += 1;
        swept += usize::from(is_swept);

        let sphere = sphere_of(&ee, c, bone);
        let mut body = body_of(&ee, b);
        let mut contact = contact_of(&ee, c);
        let against = match kind {
            0 | 1 => Vf::new(0.0, 0.0, 0.0, 1.0),
            2 | 3 => port.velocity(0, point),
            4 => port.velocity(1, point),
            _ => port.velocity(2, point),
        };
        contact.point = point;
        contact.normal = normal;
        contact.push_out_against(&sphere, &mut body, point, normal, against, &table);
        set_quad(&mut ee, c + at::POINT, point);
        set_quad(&mut ee, c + at::NORMAL, normal);
        ee.call(push_out, &[u64::from(c), u64::from(o.point), u64::from(o.normal)], &[], BUDGET);

        let ctx = format!("case {case}: collision {i}, other kind {kind}, swept {is_swept}, owner on object {}", port.owner_on_object);
        same_push(&ctx, &ee, b, c, &body, &contact);
        for k in 0..2 {
            let want = quad(&ee, o.objects[k] + other_at::COFG_WORLD);
            assert_eq!(bits(port.objects[k].cofg_world), bits(want), "{ctx}: object {k}'s centre in the world");
        }
    }
    eprintln!("push-out against: {seen:?}, {swept} swept");
    assert!(seen.iter().all(|&n| n > 1000) && swept > 2000, "{seen:?} {swept}");
}

/// A scene whose contacts are with the generated others: what the hooked colliders answer, and the
/// port's copy of the others for their velocities.
struct MovingScene<'a> {
    fake: &'a RefCell<Fake>,
    with: Vec<Option<usize>>,
    port: &'a mut PortOthers,
}

impl Scene for MovingScene<'_> {
    fn touching(&mut self, centre: Vf, radius: F) -> Option<Vec<(Vf, Vf)>> {
        FakeScene(self.fake).touching(centre, radius)
    }
    fn sweep(&mut self, from: Vf, to: Vf) -> Option<(Vf, Vf)> {
        FakeScene(self.fake).sweep(from, to)
    }
    fn other(&mut self, i: usize) -> Option<usize> {
        self.with[i]
    }
    fn velocity(&mut self, other: usize, at: Vf) -> Vf {
        self.port.velocity(other, at)
    }
}

/// The whole solve with scene contacts against moving things: each contact's push-out against its
/// own other, the ground's against nothing, and the sweep's against the last scene contact's other.
#[test]
fn the_solve_against_moving_things_matches_the_original() {
    let Some(mut ee) = state() else { return };
    let table = curve(&ee);
    let solve = addr("bone_collision_solve").unwrap();
    let c0 = collision(&ee, 0);
    let refresh = ee.u32_at(ee.u32_at(ee.u32_at(c0 + at::SPHERE) + 0x38) + 0xa8);
    ee.hooks.insert(refresh, Box::new(|_| {}));
    let segments: Vec<u32> = (0..9).map(|i| ee.u32_at(collision(&ee, i) + at::SEGMENT)).collect();
    let o = others(&ee);
    hook_mover(&mut ee, &o);
    let shared = Rc::new(RefCell::new((Answers::default(), Asked::default())));
    hook(&mut ee, &shared, segments);
    let terrain = ee.u32_at(c0 + at::TERRAIN);

    let mut pick = Pick(0x62_5c_e0_e5);
    let (mut moving_contacts, mut swept_moving) = (0, 0);
    for case in 0..CASES {
        let i = pick.upto(9) as usize;
        let c = collision(&ee, i);
        let b = ee.u32_at(c + at::BODY);
        let bone = (0..18).find(|&k| bone_body(&ee, k) == b).unwrap();
        let pos = write_bone(&mut ee, &mut pick, c, b);
        for k in [at::TOUCHED, at::GROUND, at::SCENE, at::SWEPT] {
            ee.set_u32(c + k, pick.upto(2) as u32);
        }
        ee.set_u32(c + at::SWEEP, u32::from(!pick.chance(4)));
        ee.set_f32(c + at::SWEEP_BACK, SWEEP_BACK);
        // A stale other from an earlier solve, which the solve must clear.
        ee.set_u32(c + 0x70, o.collidables[pick.upto(3) as usize]);
        let radius = ee.f32_at(b + body_at::RADIUS);
        let mut port = write_others(&mut ee, &mut pick, &o, pos);

        let on_terrain = pick.chance(2);
        ee.set_u32(c + at::TERRAIN, if on_terrain { terrain } else { 0 });
        let lo = Vf::new(pos.x() - 10.0, 0.0, pos.z() - 10.0, 0.0);
        let hi = Vf::new(pos.x() + 10.0, 300.0, pos.z() + 10.0, 0.0);
        set_quad(&mut ee, terrain + BOX_MIN, lo);
        set_quad(&mut ee, terrain + BOX_MAX, hi);
        let ground_y = pos.y() - radius * pick.range(-1.0, 2.0);
        let ground = (Vf::new(pos.x(), ground_y, pos.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 1.0));
        let n = if pick.chance(4) { 0 } else { 1 + pick.upto(3) as usize };
        let touching = (n > 0).then(|| (0..n).map(|_| pick.contact(pos, radius)).collect::<Vec<_>>());
        let with: Vec<Option<usize>> = (0..n).map(|_| if pick.chance(4) { None } else { Some(pick.upto(3) as usize) }).collect();
        let sweep = if pick.chance(2) { Some(pick.contact(pos, radius)) } else { None };
        moving_contacts += with.iter().flatten().count();
        swept_moving += usize::from(sweep.is_some() && with.last().copied().flatten().is_some());
        let answers = Answers { ground, touching, sweep, others: with.iter().map(|w| w.map_or(0, |k| o.collidables[k])).collect() };

        let sphere = sphere_of(&ee, c, bone);
        let mut body = body_of(&ee, b);
        let mut contact = contact_of(&ee, c);
        let fake = RefCell::new(Fake { answers: answers.clone(), asked: Asked::default() });
        {
            let mut w = WorldRef(&fake);
            let t = on_terrain.then(|| Terrain { world: &mut w as &mut dyn World, min: lo, max: hi });
            let mut scene = MovingScene { fake: &fake, with: with.clone(), port: &mut port };
            contact.solve(true, &sphere, &mut body, t, &mut scene, &table);
        }
        *shared.borrow_mut() = (answers, Asked::default());
        ee.call(solve, &[u64::from(c), 1], &[], BUDGET);

        let ctx = format!("case {case}: collision {i}, contacts with {with:?}, sweep {}", sweep.is_some());
        same_push(&ctx, &ee, b, c, &body, &contact);
        for k in 0..2 {
            let want = quad(&ee, o.objects[k] + other_at::COFG_WORLD);
            assert_eq!(bits(port.objects[k].cofg_world), bits(want), "{ctx}: object {k}'s centre in the world");
        }
    }
    eprintln!("solve against moving things: {moving_contacts} moving contacts, {swept_moving} sweeps after one");
    assert!(moving_contacts > 8000 && swept_moving > 1500, "{moving_contacts} {swept_moving}");
}

/// The bike box pointed at another actor's transform (+0x14) matches the original's, including the
/// velocity it writes (the clamped centre, turned out of the node's frame).
#[test]
fn the_bike_box_in_another_frame_matches_the_original() {
    let Some(mut ee) = state() else { return };
    let solve = addr("bike_box_solve").unwrap();
    let r = ee.u32_at(ACTOR + 0x5f8);
    let b = ee.u32_at(r + box_at::BODY);
    let o = others(&ee);
    let mut pick = Pick(0x62_b0_c5_14);
    let (mut held, mut free, mut off) = (0, 0, 0);
    for case in 0..CASES {
        let on = !pick.chance(30);
        let enabled = !pick.chance(30);
        ee.set_u32(r + box_at::ON, u32::from(on));
        ee.set_u32(b + body_at::ENABLED, u32::from(enabled));
        let origin = Vf::new(pick.range(10.0, 6000.0), pick.range(-20.0, 260.0), pick.range(10.0, 6000.0), 1.0);
        let node = pick.node(origin);
        write_node(&mut ee, o.node, &node);
        ee.set_u32(r + box_at::OTHER, o.node);
        let (centre, half) = if pick.chance(4) {
            (Vf::new(0.0, 1.9, -0.25, 1.0), Vf::new(0.5, 1.75, 2.5, 1.0))
        } else {
            (pick.vector(3.0), Vf::new(pick.positive(1.0), pick.positive(1.0), pick.positive(1.5), pick.float(2.0)))
        };
        set_quad(&mut ee, r + box_at::CENTRE, centre);
        set_quad(&mut ee, r + box_at::HALF, half);
        // The four pointers of the moving frame do not matter on this path: any of them, or none.
        for k in [box_at::FRAME_AT, box_at::CENTRE_AT, box_at::HALF_AT, box_at::CARRIED_AT] {
            ee.set_u32(r + k, if pick.chance(2) { 0 } else { ACTOR + 0x530 });
        }
        let l = [0, 1, 2].map(|k| centre[k] + half[k].abs() * pick.range(-1.6, 1.6));
        let pos = node.place(Vf::new(l[0], l[1], l[2], pick.float(2.0)));
        let vel = pick.vector(60.0);
        set_quad(&mut ee, b + body_at::POSITION, pos);
        set_quad(&mut ee, b + body_at::VELOCITY, vel);

        let mut body = Body { enabled, position: pos, velocity: vel, ..Body::default() };
        NodeBox { node, centre, half }.hold(on, &mut body);
        ee.call(solve, &[u64::from(r)], &[], BUDGET);
        let ctx = format!("case {case}: on {on}, enabled {enabled}, local {l:?}, half {half:?}");
        assert_eq!(bits(body.position), bits(quad(&ee, b + body_at::POSITION)), "{ctx}: position");
        assert_eq!(bits(body.velocity), bits(quad(&ee, b + body_at::VELOCITY)), "{ctx}: velocity");
        if !on || !enabled {
            off += 1;
        } else if (0..3).any(|k| !(l[k] - centre[k] <= half[k]) || l[k] - centre[k] < -half[k]) {
            held += 1;
        } else {
            free += 1;
        }
    }
    ee.set_u32(r + box_at::OTHER, 0);
    eprintln!("bike box in another frame: {held} held, {free} free, {off} off");
    assert!(held > 3000 && free > 300 && off > 300, "{held} {free} {off}");
}
