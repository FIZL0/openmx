//! S240 — a thrown rider's bones striking free09's beach ball in PCSX2, and the port's bones' scene
//! against it.
//!
//! research/s240/put_ball.py drops an awake ball three feet above the human's lying rider (off S197b's
//! hit2-2828), and research/s240/strike.py records watch/s240-ragdoll.toml and saves drop2-2840 to
//! drop2-2845. Bone 2's collision's sphere touches the ball's mesh collidable (its `f.col`) at 2840
//! and 2841, the contact's other the ball's collidable, and the ground from 2842 on.
//!
//! 1. **the list**: the original's gather on the human's bones' search lists every awake ball (category
//!    `0x18`, `+0x116` `0x14`), each at the distance [`KnockedProp::near`] measures from the same probe;
//! 2. **the sphere test**: a bone's sphere collider paired with the ball's collidable, once or twice on
//!    one record, is [`KnockedProp::touch`] on the port's ball where its node stands it, bit for bit;
//! 3. **the hit**: bone 2's sphere where it stood in drop2-2841's tick, against the ball as drop2-2840
//!    left it, takes the record with the state's point and normal.
//!
//! The push-out against the ball is [`Against::Other`] on its body (`tests/ragdoll_contact.rs` proves
//! it on generated bodies), and the struck listener [`PropSet::struck`] (`tests/prop_knock.rs`).
//!
//! Skipped with a line where the disc, the kit or the states are not there (they are in s3's context
//! only; research/s240/slots.py ram drop2-NNNN writes the RAM images).

use game_render::disc::Disc;
use game_sim::biplane::{Area, Body};
use game_sim::contact::Node;
use game_sim::phy::Ini;
use game_sim::prop_set::PropSet;
use game_sim::ragdoll_contact::Against;
use game_sim::ragdoll_scene::{KnockedProp, Touch};
use game_sim::track::Scene;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::path::PathBuf;

/// The `.toy` actor's collection list (tests/prop_collision.rs).
const LIST: u32 = 0x011b_43f4;

/// free09's terrain: three root cells by three, nine feet a sample.
const FREE09: (f32, f32, f32, f32) = (3.0, 3.0, 9.0, 9.0);

/// The human's rider body actor in the drop2 states (research/s240/bones.py).
const HUMAN: u32 = 0x0179_7980;

/// The bone whose sphere meets the ball.
const BONE: u32 = 2;

const BUDGET: u64 = 50_000_000;

mod coll {
    pub const FLAGS: u32 = 0x20;
    pub const ACTIVE: u32 = 2;
    pub const BODIES: u32 = 0x320;
}

mod obj {
    pub const BODY: u32 = 0x110;
    pub const COLLISION: u32 = 0x120;
    pub const NODE: u32 = 0x13c;
}

/// A collidable's fields, a sphere collider's and its record's (tests/ragdoll_props.rs).
mod at {
    pub const CENTRE: u32 = 0x40;
    pub const REACH: u32 = 0x60;
    pub const OBJECT: u32 = 0x80;
    pub const CATEGORY: u32 = 0x84;
    pub const FLAGS: u32 = 0x116;
    pub const CENTRE2: u32 = 0x120;
    pub const RADIUS: u32 = 0x170;
    pub const RADIUS2: u32 = 0x174;
    pub const RECORD: u32 = 0x6c;
    pub const COUNT: u32 = 0x4;
    pub const HELD: u32 = 0x8;
    pub const NORMAL: u32 = 0x30;
    pub const POINT: u32 = 0x60;
    pub const WITH: u32 = 0x90;
}

/// The bones' search, a bone collision, and the actor's words that hold them.
mod bone {
    pub const ACTOR_COLLISIONS: u32 = 0x624;
    pub const ACTOR_SEARCH: u32 = 0x628;
    pub const FOUND: u32 = 0x00;
    pub const COUNT: u32 = 0x04;
    pub const BUILT: u32 = 0x30;
    pub const POINT: u32 = 0x40;
    pub const NORMAL: u32 = 0x50;
    pub const OTHER: u32 = 0x70;
    pub const SPHERE: u32 = 0x88;
}

/// The category of a prop's mesh collidable, and the flags of an awake one.
const PROP: u32 = 0x18;
const AWAKE: u8 = 0x14;

fn context(kit: &Kit, rel: &str) -> PathBuf {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context").join(rel);
    if here.exists() { here } else { kit.ctx(rel) }
}

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf::from_bits([0, 4, 8, 12].map(|k| ee.u32_at(a + k)))
}

fn set_vf(ee: &mut Ee, a: u32, v: Vf) {
    for (k, b) in v.to_bits().into_iter().enumerate() {
        ee.set_u32(a + 4 * k as u32, b);
    }
}

fn ee_body(ee: &Ee, b: u32) -> Body {
    let q = |a: u32| quad(ee, b + a);
    Body {
        enabled: ee.u32_at(b) != 0,
        inv_mass: F(ee.f32_at(b + 0x10)),
        damping: q(0x20),
        last_position: q(0x50),
        position: q(0x60),
        velocity: q(0x70),
        inv_inertia: q(0x120),
        last_quat: q(0x150),
        quat: q(0x170),
        rates: q(0x1b0),
        ..Body::default()
    }
}

fn node_at(ee: &Ee, n: u32) -> Node {
    Node { rows: [0, 1, 2].map(|k| quad(ee, n + 0x80 + 16 * k)), origin: quad(ee, n + 0xb0) }
}

fn collections(ee: &Ee) -> Vec<u32> {
    let mut out = Vec::new();
    let mut cell = ee.u32_at(LIST + 4);
    while cell != 0 {
        out.push(ee.u32_at(cell));
        cell = ee.u32_at(cell + 8);
    }
    out
}

/// Collection `c`'s one object.
fn object(ee: &Ee, c: usize) -> u32 {
    ee.u32_at(ee.u32_at(collections(ee)[c] + coll::BODIES + 8))
}

/// free09's knockable props off the disc.
fn free09() -> Option<PropSet> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let name = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("Tracks\\free09.res")).map(|e| e.name.clone())?;
    let res = disc.res(&name).expect("free09's archive");
    let toy = Ini::parse_bytes(&res.read_suffix("free09.toy").expect("free09.toy"));
    let scene = Scene::parse(&res.read_suffix("free09.scn").expect("free09.scn")).expect("free09.scn parses");
    let (x, z, sx, sz) = FREE09;
    let (set, said) = PropSet::load(
        &toy,
        "Tracks\\Free09\\",
        &scene.models,
        &mut |p| res.read(p).ok(),
        &mut |p| res.read_suffix(p).ok(),
        Some(Area::new(F(x), F(z), F(sx), F(sz))),
    );
    assert!(said.is_empty(), "free09 loads whole: {said:?}");
    Some(set)
}

/// The woken collections as the state left them: the body and the node. Answers each woken
/// collection's index with its object's collision object.
fn seed(ee: &Ee, set: &mut PropSet) -> Vec<(usize, u32)> {
    let game = collections(ee);
    assert_eq!(game.len(), set.collections.len(), "the state's collections");
    let mut woken = Vec::new();
    for (ci, &c) in game.iter().enumerate() {
        if ee.u32_at(c + coll::FLAGS) & coll::ACTIVE == 0 {
            continue;
        }
        let o = object(ee, ci);
        set.wake(ci, 0);
        let port = &mut set.collections[ci];
        let body = ee_body(ee, ee.u32_at(o + obj::BODY));
        if !body.enabled {
            port.disable(0);
        }
        port.bones[0].body = body;
        port.objects[0].node = node_at(ee, ee.u32_at(o + obj::NODE));
        woken.push((ci, ee.u32_at(o + obj::COLLISION)));
    }
    woken
}

fn load(kit: &Kit, frame: u32) -> Option<Ee> {
    let state = context(kit, &format!("states/s240-drop2-{frame}.eeMemory.bin"));
    let mut ee = Ee::from_ram(&state).map_err(|why| skip(&format!("{why} — research/s240/slots.py ram drop2-{frame} writes it"))).ok()?;
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    assert_eq!(ee.u32_at(addr("frame_counter").unwrap()), frame, "the state is not frame {frame}");
    Some(ee)
}

/// The human's bone collision `k`, and its sphere collider.
fn bone_sphere(ee: &Ee, k: u32) -> (u32, u32) {
    let c = ee.u32_at(ee.u32_at(HUMAN + bone::ACTOR_COLLISIONS) + 4 * k);
    (c, ee.u32_at(c + bone::SPHERE))
}

/// The prop mesh collidable on collision object `object`: the one word of category [`PROP`] whose
/// `+0x80` names it.
fn collidable_of(ee: &Ee, object: u32) -> u32 {
    let found: Vec<u32> = (0x10_0000u32..0x1ff_fe00)
        .step_by(16)
        .filter(|&a| ee.u32_at(a + at::OBJECT) == object && ee.u32_at(a + at::CATEGORY) == PROP)
        .collect();
    assert_eq!(found.len(), 1, "the collidables on {object:#x}: {found:x?}");
    found[0]
}

/// The port's awake props in free09 as `ee` holds them, each with its collidable in the state.
fn knocked(ee: &Ee, set: &mut PropSet, probe: Vf) -> Vec<(KnockedProp, u32)> {
    let woken = seed(ee, set);
    let props = KnockedProp::near(set, probe, F(1.0e9));
    props
        .into_iter()
        .map(|p| {
            let (_, object) = woken.iter().find(|(c, _)| *c == p.pushed.collection).expect("a listed prop is woken");
            let o = collidable_of(ee, *object);
            (p, o)
        })
        .collect()
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
    fn unit_vector(&mut self) -> Vf {
        let v = [self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0)];
        let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3);
        Vf::new(v[0] / n, v[1] / n, v[2] / n, 0.0)
    }
}

/// **The bones' search lists the awake balls** (S240): in drop2-2840 every collidable of category
/// `0x18` the human's bones' search lists is flagged awake, and the original's gather, its first
/// sphere stood at random probes round the ball, measures each listed ball as the port's
/// [`KnockedProp::near`] does, bit for bit; and lists every awake ball the port does.
#[test]
fn the_bones_search_lists_the_awake_balls_as_the_originals() {
    let Some(kit) = Kit::load().map_err(|w| skip(&w)).ok() else { return };
    let Some(mut ee) = load(&kit, 2840) else { return };
    let Some(mut set) = free09() else { return };
    let search = ee.u32_at(HUMAN + bone::ACTOR_SEARCH);
    let (_, sphere) = bone_sphere(&ee, 0);
    let listed = |ee: &Ee| -> Vec<(u32, u32)> {
        let (a, n) = (ee.u32_at(search + bone::FOUND), ee.u32_at(search + bone::COUNT));
        (0..n).map(|k| (ee.u32_at(a + 8 * k), ee.u32_at(a + 8 * k + 4))).filter(|&(o, _)| ee.u32_at(o + at::CATEGORY) == PROP).collect()
    };
    let before = listed(&ee);
    assert!(!before.is_empty(), "no prop in the bones' list");
    for &(o, _) in &before {
        assert_eq!(ee.u8_at(o + at::FLAGS), AWAKE, "the listed prop {o:#x}");
    }
    let ball = quad(&ee, sphere + at::CENTRE);
    let gather = addr("collision_search_gather").unwrap();
    let mut pick = Pick(0x5240_0000_ba11_0001);
    let mut compared = 0;
    for case in 0..500 {
        let probe = if case == 0 {
            ball
        } else {
            Vf::new(ball.x() + pick.range(-40.0, 40.0), ball.y() + pick.range(-10.0, 10.0), ball.z() + pick.range(-40.0, 40.0), 1.0)
        };
        set_vf(&mut ee, sphere + at::CENTRE, probe);
        if case % 2 == 1 {
            ee.set_u32(search + bone::BUILT, 0);
        }
        ee.call(gather, &[u64::from(search), u64::from(sphere), 3], &[], BUDGET);
        let theirs = listed(&ee);
        let ours = knocked(&ee, &mut set, probe);
        let what = format!("case {case}: probe {probe:?}");
        assert_eq!(theirs.len(), ours.len(), "{what}: the original lists {theirs:x?}");
        for (p, o) in &ours {
            let Some(&(_, d)) = theirs.iter().find(|(t, _)| t == o) else { panic!("{what}: the original does not list {o:#x}") };
            assert_eq!(p.distance.to_bits(), d, "{what}: {o:#x}'s distance: port {} game {}", p.distance.0, f32::from_bits(d));
        }
        compared += ours.len();
    }
    eprintln!("{compared} listed balls compared");
}

/// A point near the prop's mesh: one of its vertices through its frame.
fn near_vertex(prop: &KnockedProp, pick: &mut Pick) -> Vf {
    let mesh = prop.mesh().expect("a ball has a mesh");
    let n = match &mesh.vertices {
        game_sim::track::Points::Wide(v) => v.len(),
        game_sim::track::Points::Packed(v) => v.len(),
    };
    let v = game_sim::collide::MeshRef::of(mesh).vertex(pick.upto(n as u64) as u16);
    prop.frame.place(Vf::new(v.x(), v.y(), v.z(), 1.0))
}

const SPHERES: usize = 20_000;

/// **The sphere test on a ball answers as the original's** (S240): 20,000 spheres stood near a vertex
/// of an awake ball's mesh in drop2-2840 (the bones' radii and random ones from a tenth to a metre),
/// each through the original's pair on bone 2's sphere collider with an emptied record, with the
/// ball's collidable once or twice; and through the port's ball, offered the record's hit so far.
/// The record's count, what it is with, the plane distance it holds, and the contact's point and
/// normal in the world, bit for bit.
#[test]
fn the_sphere_test_on_a_ball_answers_as_the_originals() {
    let Some(kit) = Kit::load().map_err(|w| skip(&w)).ok() else { return };
    let Some(mut ee) = load(&kit, 2840) else { return };
    let Some(mut set) = free09() else { return };
    let (_, sphere) = bone_sphere(&ee, BONE);
    let balls = knocked(&ee, &mut set, quad(&ee, sphere + at::CENTRE));
    assert!(!balls.is_empty(), "no awake ball in drop2-2840");
    let (clear, pair) = (addr("sphere_collider_clear").unwrap(), addr("sphere_collider_pair").unwrap());
    let mut pick = Pick(0x5240_5240_0000_0002);
    let mut hits = 0;
    for case in 0..SPHERES {
        let (ball, o) = &balls[pick.upto(balls.len() as u64) as usize];
        let twice = pick.upto(2) == 0;
        let p = near_vertex(ball, &mut pick);
        let radius = match case % 5 {
            0 => 0.4f32,
            1 => 0.75,
            2 => 0.5,
            _ => pick.range(0.1, 1.0),
        };
        let off = pick.unit_vector();
        let reach = 2.0 * radius * pick.unit();
        let centre = Vf::new(p.x() + off.x() * reach, p.y() + off.y() * reach, p.z() + off.z() * reach, 1.0);
        set_vf(&mut ee, sphere + at::CENTRE, centre);
        set_vf(&mut ee, sphere + at::CENTRE2, centre);
        ee.set_f32(sphere + at::RADIUS, radius);
        ee.set_f32(sphere + at::RADIUS2, ps2_float::fpu::mul(radius, radius));
        ee.call(clear, &[u64::from(sphere)], &[], 1_000_000);
        for _ in 0..1 + usize::from(twice) {
            ee.call(pair, &[u64::from(sphere), u64::from(*o)], &[], BUDGET);
        }
        let rec = ee.u32_at(sphere + at::RECORD);
        let count = ee.u32_at(rec + at::COUNT);
        let original = (count > 0).then(|| (ee.u32_at(rec + at::WITH), ee.u32_at(rec + at::HELD), quad(&ee, rec + at::POINT).to_bits(), quad(&ee, rec + at::NORMAL).to_bits()));

        let mut best: Option<Touch> = None;
        for _ in 0..1 + usize::from(twice) {
            if let Some(t) = ball.touch(centre, F(radius), best.map(|b| b.distance)) {
                best = Some(t);
            }
        }
        let port = best.map(|t| (*o, t.distance.to_bits(), t.world.0.to_bits(), t.world.1.to_bits()));
        let what = format!("case {case}: ball {:?} ({o:#x}), twice {twice}, sphere at {centre:?} r {radius}", (ball.pushed.collection, ball.pushed.object));
        assert!(count <= 1, "{what}: {count} contacts");
        assert_eq!(port, original, "{what}");
        hits += usize::from(best.is_some());
    }
    eprintln!("{hits} of {SPHERES} hit");
    assert!(hits > SPHERES / 5 && hits < SPHERES * 9 / 10, "{hits} hits");
}

/// **Bone 2 strikes the ball as in the capture** (S240): in drop2-2841 bone 2's collision is on the
/// ball's collidable; its sphere, where the tick stood it, paired with the port's ball as drop2-2840
/// left it, takes the record with the collision's point and normal, bit for bit; and the push-out
/// asks the ball's body how it moves ([`Against::Other`]).
#[test]
fn bone_2_strikes_the_ball_as_in_the_capture() {
    let Some(kit) = Kit::load().map_err(|w| skip(&w)).ok() else { return };
    let (Some(before), Some(after)) = (load(&kit, 2840), load(&kit, 2841)) else { return };
    let Some(mut set) = free09() else { return };
    let (c, sphere) = bone_sphere(&after, BONE);
    let other = after.u32_at(c + bone::OTHER);
    let (centre, radius) = (quad(&after, sphere + at::CENTRE), F(after.f32_at(sphere + at::RADIUS)));
    let balls = knocked(&before, &mut set, centre);
    let (ball, o) = balls.iter().find(|(_, o)| *o == other).unwrap_or_else(|| panic!("bone {BONE}'s other {other:#x} is none of the awake balls"));
    eprintln!(
        "bone {BONE}: sphere at {centre:?} r {}; ball {:?} reach {} at distance {}",
        radius.0,
        (ball.pushed.collection, ball.pushed.object),
        after.f32_at(o + at::REACH),
        ball.distance.0
    );
    assert!(ball.distance <= radius, "the ball is out of the sphere's reach");
    let t = ball.touch(centre, radius, None).expect("the port's sphere misses the ball");
    let (point, normal) = (quad(&after, c + bone::POINT), quad(&after, c + bone::NORMAL));
    assert_eq!((t.world.0.to_bits(), t.world.1.to_bits()), (point.to_bits(), normal.to_bits()), "port {:?} {:?}, game {point:?} {normal:?}", t.world.0, t.world.1);
    let mut pushed = ball.pushed.clone();
    let (collider, mut w) = pushed.weighed();
    let v = game_sim::ragdoll_contact::moving(point, Against::Other(&mut game_sim::contact::Other { collider, side: game_sim::contact::Side::Body(&mut w) }));
    eprintln!("the ball moves at {v:?} under the bone");
}
