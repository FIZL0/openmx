//! S240 — a thrown rider's bones striking free09's beach ball in PCSX2, and the port's bones' scene
//! against it.
//!
//! research/s240/put_ball.py drops an awake ball six feet above the human's lying rider, and
//! research/s240/strike.py records watch/s240-ragdoll.toml and saves drop3-2852 to drop3-2857 (F16:
//! S240's drop2 states were lost; journal/F16 has the recipe). Bones' collisions' spheres touch the
//! ball's mesh collidable (its `f.col`) from 2853, the contact's other the ball's collidable.
//!
//! 1. **the list**: the original's gather on the human's bones' search lists the awake balls near the probe (category
//!    `0x18`, `+0x116` `0x14`), each at the distance [`KnockedProp::near`] measures from the same probe;
//! 2. **the sphere test**: a bone's sphere collider paired with the ball's collidable, once or twice on
//!    one record, is [`KnockedProp::touch`] on the port's ball where its node stands it, bit for bit;
//! 3. **the hit**: a bone's sphere where it stood in drop3-2853's tick, against the ball as drop3-2852
//!    left it, takes the record with the state's point and normal;
//! 4. **the whole tick** (F16): the original's rider body update from drop3-2852 is drop3-2853's
//!    bones; the port's [`Actor::tick`](game_sim::ragdoll_body::Actor::tick) off the same RAM, the
//!    world's answers the original's but the ball's (the port's ball's sphere test and push-out), writes
//!    the same numbers; and the props' step after it is drop3-2853's ball.
//!
//! The push-out against the ball is [`Against::Other`] on its body (`tests/ragdoll_contact.rs` proves
//! it on generated bodies), and the struck listener [`PropSet::struck`] (`tests/prop_knock.rs`).
//!
//! Skipped with a line where the disc, the kit or the states are not there (context/states holds
//! them; research/s240/slots.py ram drop3-NNNN writes the RAM images).

#[path = "ragdoll_ball/tick.rs"]
mod tick;

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

/// The human's rider body actor in the drop states (research/s240/bones.py).
const HUMAN: u32 = 0x0179_7980;

/// The run the states are from, its first state, and its last.
const RUN: &str = "drop3";
const FIRST: u32 = 2852;
const LAST: u32 = 2857;

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
    pub const FRESH: u32 = 0x1b0;
    pub const COUNTDOWN: u32 = 0x1bc;
}

/// A prop's collision object (tests/prop_wreck.rs).
mod rec {
    pub const TOUCHING: u32 = 0x40;
    pub const TOUCHING_OTHER: u32 = 0x44;
    pub const ON_GROUND: u32 = 0x48;
    pub const ON_OBJECT: u32 = 0x4c;
    pub const COFG_WORLD: u32 = 0xa0;
    pub const SWEPT_FROM: u32 = 0xd0;
    pub const SWEPT_TO: u32 = 0xe0;
    pub const NORMAL_SPEED: u32 = 0x1d8;
    pub const RELATIVE: u32 = 0x1e0;
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
        last_force: q(0x80),
        force: q(0x90),
        accel: q(0xa0),
        shock_peak: q(0xb0),
        shock: q(0xc0),
        step: q(0xf0),
        inv_inertia: q(0x120),
        ang_damping: q(0x130),
        last_quat: q(0x150),
        quat: q(0x170),
        cofg: q(0x180),
        moment: q(0x190),
        last_moment: q(0x1a0),
        rates: q(0x1b0),
        ang_accel: q(0x1c0),
        twist_peak: q(0x1d0),
        twist: q(0x1e0),
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
        let r = ee.u32_at(o + obj::COLLISION);
        let ob = &mut port.objects[0];
        ob.node = node_at(ee, ee.u32_at(o + obj::NODE));
        ob.countdown = F(ee.f32_at(o + obj::COUNTDOWN));
        ob.fresh = ee.u32_at(o + obj::FRESH) != 0;
        let col = ob.collision.as_mut().expect("a ball has a collision object");
        col.collider.cofg_world = quad(ee, r + rec::COFG_WORLD);
        col.collider.normal_speed = F(ee.f32_at(r + rec::NORMAL_SPEED));
        col.collider.relative = quad(ee, r + rec::RELATIVE);
        let rc = &mut col.record;
        rc.touching = ee.u32_at(r + rec::TOUCHING) != 0;
        rc.touching_other = ee.u32_at(r + rec::TOUCHING_OTHER) != 0;
        rc.on_ground = ee.u32_at(r + rec::ON_GROUND) != 0;
        rc.on_object = ee.u32_at(r + rec::ON_OBJECT) != 0;
        rc.swept_from = quad(ee, r + rec::SWEPT_FROM);
        rc.swept_to = quad(ee, r + rec::SWEPT_TO);
        woken.push((ci, r));
    }
    woken
}

fn load(kit: &Kit, frame: u32) -> Option<Ee> {
    let state = context(kit, &format!("states/s240-{RUN}-{frame}.eeMemory.bin"));
    let mut ee = Ee::from_ram(&state).map_err(|why| skip(&format!("{why} — research/s240/slots.py ram {RUN}-{frame} writes it"))).ok()?;
    if let Ok(spr) = std::fs::read(state.with_file_name(format!("s240-{RUN}-{frame}.spr.bin"))) {
        ee.spr = spr;
    }
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

/// How near the probe an awake ball always makes the original's list, in feet: five times the
/// largest bone sphere.
const LISTED: f32 = 5.0;

/// **The bones' search lists the awake balls** (S240): in drop3-2852 every collidable of category
/// `0x18` the human's bones' search lists is flagged awake, and the original's gather, its first
/// sphere stood at random probes round the ball, measures each listed ball as the port's
/// [`KnockedProp::near`] does, bit for bit; and, built at the probe, lists every awake ball within
/// [`LISTED`] of it.
#[test]
fn the_bones_search_lists_the_awake_balls_as_the_originals() {
    let Some(kit) = Kit::load().map_err(|w| skip(&w)).ok() else { return };
    let Some(mut ee) = load(&kit, FIRST) else { return };
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
    let (mut compared, mut near) = (0, 0);
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
        let said: Vec<_> = ours.iter().map(|(p, o)| (*o, p.distance.0)).collect();
        for &(o, d) in &theirs {
            let Some((p, _)) = ours.iter().find(|(_, t)| *t == o) else { panic!("{what}: the port has no awake ball {o:#x}: {said:x?}") };
            assert_eq!(p.distance.to_bits(), d, "{what}: {o:#x}'s distance: port {} game {}", p.distance.0, f32::from_bits(d));
        }
        // A search built at this probe lists every ball within LISTED. One further off may be left
        // out (F16: built searches in drop3-2852 leave out ball 2 at 17.1, 20.9 and 42.6 ft and list
        // ball 4 at 44.3; why is not known). A kept search holds what was near where it was built.
        let built = case % 2 == 1 || case == 0;
        for (p, o) in ours.iter().filter(|(p, _)| built && p.distance.0 <= LISTED) {
            assert!(theirs.iter().any(|(t, _)| t == o), "{what}: the original does not list {o:#x} at {}", p.distance.0);
            near += 1;
        }
        compared += theirs.len();
    }
    eprintln!("{compared} listed balls compared, {near} within {LISTED} ft of a built search");
    assert!(near > 0, "no ball within {LISTED} ft of a built search");
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
/// of an awake ball's mesh in drop3-2852 (the bones' radii and random ones from a tenth to a metre),
/// each through the original's pair on bone 2's sphere collider with an emptied record, with the
/// ball's collidable once or twice; and through the port's ball, offered the record's hit so far.
/// The record's count, what it is with, the plane distance it holds, and the contact's point and
/// normal in the world, bit for bit.
#[test]
fn the_sphere_test_on_a_ball_answers_as_the_originals() {
    let Some(kit) = Kit::load().map_err(|w| skip(&w)).ok() else { return };
    let Some(mut ee) = load(&kit, FIRST) else { return };
    let Some(mut set) = free09() else { return };
    let (_, sphere) = bone_sphere(&ee, 2);
    let balls = knocked(&ee, &mut set, quad(&ee, sphere + at::CENTRE));
    assert!(!balls.is_empty(), "no awake ball in {RUN}-{FIRST}");
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

/// **A bone strikes the ball as in the capture** (S240): in drop3-2853 the first bone's collision on
/// the ball's collidable; its sphere, where the tick stood it, paired with the port's ball as
/// drop3-2852 left it, takes the record with the collision's point and normal, bit for bit; and the push-out
/// asks the ball's body how it moves ([`Against::Other`]).
#[test]
fn a_bone_strikes_the_ball_as_in_the_capture() {
    let Some(kit) = Kit::load().map_err(|w| skip(&w)).ok() else { return };
    let (Some(before), Some(after)) = (load(&kit, FIRST), load(&kit, FIRST + 1)) else { return };
    let Some(mut set) = free09() else { return };
    let on: Vec<u32> = knocked(&before, &mut set, Vf::new(0.0, 0.0, 0.0, 1.0)).into_iter().map(|(_, o)| o).collect();
    let k = (0..9).find(|&k| on.contains(&after.u32_at(bone_sphere(&after, k).0 + bone::OTHER))).expect("a bone on a ball");
    let (c, sphere) = bone_sphere(&after, k);
    let other = after.u32_at(c + bone::OTHER);
    let (centre, radius) = (quad(&after, sphere + at::CENTRE), F(after.f32_at(sphere + at::RADIUS)));
    let balls = knocked(&before, &mut set, centre);
    let (ball, o) = balls.iter().find(|(_, o)| *o == other).unwrap_or_else(|| panic!("bone {k}'s other {other:#x} is none of the awake balls"));
    eprintln!(
        "bone {k}: sphere at {centre:?} r {}; ball {:?} reach {} at distance {}",
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

/// free09's terrain and static props off the disc, for the props' step.
struct Ground {
    terrain: Vec<u8>,
    scene: Scene,
    props: game_sim::world::Props,
}

fn ground() -> Option<Ground> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let names: Vec<String> = ["Tracks\\free09.res", "Tracks\\free09_2.res"]
        .iter()
        .filter_map(|want| disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case(want)).map(|e| e.name.clone()))
        .collect();
    let archives: Vec<game_render::disc::Res> = names.iter().map(|n| disc.res(n).expect("a free09 archive")).collect();
    let scene = Scene::parse(&archives[0].read_suffix("free09.scn").expect("free09.scn")).expect("free09.scn parses");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let terrain = archives[0].read_suffix(&leaf).expect("free09's terrain");
    let (props, _) = game_sim::world::Props::load(&scene, |want| {
        archives.iter().find_map(|r| r.read_suffix(want).ok()).ok_or_else(|| "no archive of this track has it".to_string())
    });
    Some(Ground { terrain, scene, props })
}

/// `n` bytes at `a` of `from` into `ee`.
fn copy(ee: &mut Ee, from: &Ee, a: u32, n: usize) {
    let at = (a & 0x01ff_ffff) as usize;
    ee.ram[at..at + n].copy_from_slice(&from.ram[at..at + n]);
}

/// What the rider body's update reads off the rest of the game's tick, from the state after it (as
/// tests/ragdoll_wrecks.rs lays it in from a capture): the riding request with the hand dummies
/// 0x200 before it, and the bike's object round the request's `+0x1ac`.
fn lay_in(ee: &mut Ee, after: &Ee) {
    let req = ee.u32_at(HUMAN + tick::at::BIKE);
    copy(ee, after, req - 0x200, 1280);
    let far = ee.u32_at(req + 0x1ac);
    copy(ee, after, far - 0x980, 0x980 + 0x680 + 1280);
}

/// The original's rider body update from `ee` (laid in), its draws before the body found as
/// tests/ragdoll_wrecks.rs finds them, until the bones are `after`'s: the draws before it and the dt,
/// `ee` left where the update starts. Else the lanes the first try missed.
fn originals_tick(ee: &mut Ee, after: &Ee, dts: &[f32]) -> Result<(u32, f32), Vec<String>> {
    let update = addr("rider_body_update").unwrap();
    let want = tick::ee_bones(after);
    let start = tick::rng_at(ee);
    let snap = tick::Snapshot::of(ee);
    let mut first = None;
    for &dt in dts {
        for n in 0..64 {
            snap.back(ee);
            let mut r = game_sim::rng::Rng::from_state(start);
            (0..n).for_each(|_| {
                r.rand();
            });
            tick::set_rng(ee, r.state());
            ee.call(update, &[u64::from(HUMAN)], &[dt], 2_000_000_000);
            let drew = tick::draws(r.state(), tick::rng_at(ee), 1000).expect("the body's draws");
            let got = tick::ee_bones(ee);
            let d: Vec<String> = (0..18)
                .flat_map(|i| (0..6).map(move |k| (i, k)))
                .filter(|&(i, k)| got[i][k].to_bits() != want[i][k].to_bits())
                .map(|(i, k)| format!("bone {i} {}: {:?} want {:?}", tick::BODY[k].0, got[i][k], want[i][k]))
                .collect();
            if d.is_empty() {
                snap.back(ee);
                tick::set_rng(ee, r.state());
                return Ok((n, dt));
            }
            first.get_or_insert(d);
            if drew == 0 {
                break;
            }
        }
    }
    snap.back(ee);
    Err(first.unwrap())
}

/// **A whole ragdoll tick against the ball** (F16): from each drop3 state to the next, the original's
/// rider body update (the request and the bike laid in from the next state, the draws before it
/// found) is the next state's bones; the port's [`Actor::tick`](game_sim::ragdoll_body::Actor::tick)
/// off the same RAM, with the original's world answers but a sphere test on the ball answered by the
/// port's ball as the state left it ([`KnockedProp::touch`]) and its push-out against the port's
/// ball's body ([`Against::Other`]), writes every number the original's does; and the props' step
/// after it (search and step, from the balls as the state left them) is the next state's balls.
#[test]
fn a_whole_ragdoll_tick_against_the_ball_is_the_originals() {
    let Some(kit) = Kit::load().map_err(|w| skip(&w)).ok() else { return };
    let Some(world) = ground() else { return };
    let terrain = game_sim::track::Terrain::parse(&world.terrain).expect("free09's terrain");
    let g = game_sim::world::Ground::new(&terrain, &world.scene.terrain);
    let field = g.field();
    let mut bad = Vec::new();
    let mut struck = 0;
    for frame in FIRST..LAST {
        let (Some(mut ee), Some(after)) = (load(&kit, frame), load(&kit, frame + 1)) else { return };
        let Some(mut set) = free09() else { return };
        let what = format!("{RUN}-{frame}");
        lay_in(&mut ee, &after);
        let frame_dt = addr("frame_dt").unwrap();
        let dts = [after.f32_at(frame_dt), ee.f32_at(frame_dt)];
        let (n, dt) = match originals_tick(&mut ee, &after, &dts) {
            Ok(found) => found,
            Err(d) => {
                bad.push(format!("{what}: the original's update parts from the next state in {} lanes: {:?}", d.len(), &d[..d.len().min(4)]));
                continue;
            }
        };
        // The probe the bones' search measures from in the tick: the first sphere where it stood.
        let probe = quad(&after, bone_sphere(&after, 0).1 + at::CENTRE);
        let balls = knocked(&ee, &mut set, probe);
        let ball_ids: Vec<u32> = balls.iter().map(|(_, o)| *o).collect();
        // The ball the original's spheres meet, else the nearest.
        let on = (0..9).find_map(|k| {
            let o = after.u32_at(bone_sphere(&after, k).0 + bone::OTHER);
            ball_ids.contains(&o).then_some(o)
        });
        let (ball, o) = match on {
            Some(o) => balls.iter().find(|(_, b)| *b == o).unwrap(),
            None => balls.iter().min_by(|a, b| a.0.distance.0.total_cmp(&b.0.distance.0)).expect("an awake ball"),
        };
        let t = tick::port_tick(&mut ee, dt, ball, *o);
        eprintln!(
            "{what}: {n} draws before the body, dt {dt}; ball {:?} ({o:#x}) at {}; the original's spheres on it {}, the port's {}; bones on it after: {:?}; {} differences",
            (ball.pushed.collection, ball.pushed.object),
            ball.distance.0,
            t.on_ball,
            t.port_on_ball,
            t.touching_ball,
            t.bad.len()
        );
        struck += t.port_on_ball;
        bad.extend(t.bad.iter().map(|b| format!("{what}: {b}")));

        // The props' step, from the balls as the state left them: the bones' push-out reads the ball's
        // body and writes none of it.
        let mut set = free09().unwrap();
        let woken = seed(&load(&kit, frame).unwrap(), &mut set);
        let movers = set.search(F(dt), Some(&field), Some(&world.props), None);
        set.step(F(dt), &movers, &mut []);
        for (c, _) in woken {
            let o = object(&after, c);
            let want = ee_body(&after, after.u32_at(o + obj::BODY));
            let got = &set.collections[c].bones[0].body;
            for (lane, p, w) in [
                ("last position", got.last_position, want.last_position),
                ("position", got.position, want.position),
                ("velocity", got.velocity, want.velocity),
                ("last quat", got.last_quat, want.last_quat),
                ("quat", got.quat, want.quat),
                ("rates", got.rates, want.rates),
            ] {
                if p.to_bits() != w.to_bits() {
                    bad.push(format!("{what}: ball {c} {lane} after the props' step: {p:?} want {w:?}"));
                }
            }
        }
    }
    for b in &bad {
        eprintln!("{b}");
    }
    assert!(bad.is_empty(), "{} differences", bad.len());
    assert!(struck > 0, "no tick's sphere met the ball");
}
