//! S198 — a knocked free09 prop's own collision object, the port against the original's run in the
//! interpreter.
//!
//! The first recording's save state is free09 with every prop asleep where it was made, each with its
//! collision object built (the wreck bike's kind: the part's tuning, a ray probe off the model's
//! `m.col`, a sweep probe). First the loader: every free09 prop's collision object as
//! [`PropSet::load`] makes it, against the state's — the tuning, the record's switches, the ray count
//! and every ray.
//!
//! Then a loose prop lying on its own (a beach ball, the egg) is woken in the state the way a strike
//! wakes it — its collection activated, its object enabled, which files the collision object on the
//! world node — and tossed up and sideways. Every 1/30 s the original's own code runs as the world
//! node's update runs it for that prop: the collision object's clear and collide (the sweep back, the
//! rays against free09's terrain and the scene's meshes), then the collection's update (its world step:
//! gravity and the contacts' resolve in the force listener, the integrate, the post), then the
//! listeners the wake put on the node. The port's [`PropSet::frame_among`] runs beside it over the disc's
//! terrain and scene, and every frame the body, the node, the record's flags and the swept segment are
//! compared bit for bit: up, down onto the ground, the bounce and the roll, asleep, and on asleep
//! with its rays still searching (`DetectCollisionsWhenDisabled`), until the object's countdown puts
//! it back and its unfiling clears the record.
//!
//! The tosses are aimed clear of what the port's prop world does not have yet: the other props'
//! `f.col` (a ball tossed into its neighbour touches it — S197) and the toys' meshes (free09's
//! trailers, which a ride's world has but this test does not load).
//!
//! Skipped with a line where the disc, the kit or the state is not there.

use std::path::PathBuf;

use game_render::disc::{Disc, Res};
use game_sim::biplane::{Area, Body};
use game_sim::contact::{Collider, Node, Response};
use game_sim::phy::Ini;
use game_sim::prop_set::PropSet;
use game_sim::track::{Scene, Terrain};
use game_sim::world::{Ground, Props};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

const STATE: &str = "first_recording.p2m2_SaveState.eeMemory.bin";
const BUDGET: u64 = 50_000_000;

/// The `.toy` actor's collection list (see tests/props.rs).
const LIST: u32 = 0x011b_43f4;

/// free09's terrain: three root cells by three, nine feet a sample (as tests/prop_set.rs has it).
const FREE09: (f32, f32, f32, f32) = (3.0, 3.0, 9.0, 9.0);

/// The collection's fields.
mod coll {
    pub const FLAGS: u32 = 0x20;
    pub const ACTIVE: u32 = 2;
    pub const NAME: u32 = 0x144;
    pub const LISTENER: u32 = 0x34;
    pub const NODE: u32 = 0x248;
    pub const BODIES: u32 = 0x320;
}

/// The physics object's.
mod obj {
    pub const LISTENER: u32 = 0x50;
    pub const DETECT: u32 = 0x6c;
    pub const BODY: u32 = 0x110;
    pub const COLLISION: u32 = 0x120;
    pub const RAY_PROBE: u32 = 0x124;
    pub const SWEEP_PROBE: u32 = 0x12c;
    pub const NODE: u32 = 0x13c;
    pub const FRESH: u32 = 0x1b0;
    pub const COUNTDOWN: u32 = 0x1bc;
}

/// The collision object's (tests/wreck_bike.rs has the same kind's).
mod rec {
    pub const VTABLE: u32 = 0x08;
    pub const SEARCH_GROUND: u32 = 0x30;
    pub const SEARCH: u32 = 0x34;
    pub const SWEEP_BACK: u32 = 0x3c;
    pub const TOUCHING: u32 = 0x40;
    pub const TOUCHING_OTHER: u32 = 0x44;
    pub const ON_GROUND: u32 = 0x48;
    pub const ON_OBJECT: u32 = 0x4c;
    pub const DAMP: u32 = 0x50;
    pub const REST_LINEAR: u32 = 0x54;
    pub const DAMP_LINEAR: u32 = 0x58;
    pub const REST_ANGULAR: u32 = 0x5c;
    pub const DAMP_ANGULAR: u32 = 0x60;
    pub const MASS: u32 = 0x64;
    pub const INV_MASS: u32 = 0x68;
    pub const INV_INERTIA: u32 = 0x80;
    pub const COFG: u32 = 0x90;
    pub const COFG_WORLD: u32 = 0xa0;
    pub const OFFSET: u32 = 0xb0;
    pub const LEAD: u32 = 0xc0;
    pub const PUSH_OUT: u32 = 0xc4;
    pub const FACING: u32 = 0xc8;
    pub const SWEPT_FROM: u32 = 0xd0;
    pub const SWEPT_TO: u32 = 0xe0;
    pub const NODE: u32 = 0xf8;
    pub const CAPACITY: u32 = 0x104;
    pub const NEARBY: u32 = 0x10c;
    pub const ACTORS: u32 = 0x110;
    pub const COLLIDES: u32 = 0x114;
    pub const MAX_DEPTH: u32 = 0x118;
    pub const SCENERY: u32 = 0x124;
    pub const ACTOR: u32 = 0x13c;
    pub const ANGULAR: u32 = 0x148;
    pub const ACTOR_ANGULAR: u32 = 0x14c;
    pub const SPRING_MASK: u32 = 0x150;
    pub const FRICTION_MASK: u32 = 0x160;
    pub const ACTOR_SPRING_MASK: u32 = 0x170;
    pub const ACTOR_FRICTION_MASK: u32 = 0x180;
    pub const FRICTION: u32 = 0x190;
    pub const ACTOR_FRICTION: u32 = 0x194;
    pub const SURFACES: u32 = 0x198;
    pub const NORMAL_SPEED: u32 = 0x1d8;
    pub const RELATIVE: u32 = 0x1e0;
    pub const RAY_PROBE: u32 = 0x1f0;
    pub const SWEEP_PROBE: u32 = 0x1f4;
    pub const COLLIDABLE: u32 = 0x1f8;
    /// The collidable's flags; the ray probe's count and rays.
    pub const FLAGS: u32 = 0x84;
    pub const RAY_COUNT: u32 = 0x168;
    pub const RAYS: u32 = 0x170;
    /// The collision object's clear and collide, by vtable slot.
    pub const CLEAR: u32 = 0x74;
    pub const COLLIDE: u32 = 0x7c;
}

/// The body's.
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

/// The original's functions.
mod f {
    pub const UPDATE: u32 = 0x003b_28a0;
    pub const SETTLE: u32 = 0x003b_2440;
    pub const ACTIVATE: u32 = 0x003b_27a0;
    pub const ENABLE: u32 = 0x0015_b790;
    pub const LISTEN: u32 = 0x0015_b2a0;
}

fn image() -> Option<Ee> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/states").join(STATE);
    let path = if here.exists() { here } else { kit.ctx("states").join(STATE) };
    match Ee::from_ram(&path) {
        Ok(mut ee) => {
            ee.set(28, u64::from(addr("boot_gp").unwrap()));
            Some(ee)
        }
        Err(why) => {
            skip(&format!("{why} — `./kit state ram context/recordings/first_recording.p2m2_SaveState.p2s` writes it"));
            None
        }
    }
}

fn text(ee: &Ee, at: u32) -> String {
    (0..256).map(|k| ee.u8_at(at + k)).take_while(|&b| b != 0).map(|b| b as char).collect()
}

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf::from_bits([0, 4, 8, 12].map(|k| ee.u32_at(a + k)))
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

fn bodies_differ(got: &Body, want: &Body) -> Vec<String> {
    let mut out = Vec::new();
    if got.enabled != want.enabled {
        out.push(format!("enabled: the game {}, the port {}", want.enabled, got.enabled));
    }
    for (name, g, w) in [
        ("last_position", got.last_position, want.last_position),
        ("position", got.position, want.position),
        ("velocity", got.velocity, want.velocity),
        ("last_force", got.last_force, want.last_force),
        ("force", got.force, want.force),
        ("shock_peak", got.shock_peak, want.shock_peak),
        ("shock", got.shock, want.shock),
        ("step", got.step, want.step),
        ("last_quat", got.last_quat, want.last_quat),
        ("quat", got.quat, want.quat),
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
    // The linear acceleration's `w` is whatever a vector register held (see `Body::integrate`).
    if got.accel.to_bits()[..3] != want.accel.to_bits()[..3] {
        out.push(format!("accel: the game {:?}, the port {:?}", want.accel.0, got.accel.0));
    }
    out
}

fn node_rows(n: &Node) -> [[u32; 4]; 4] {
    [n.rows[0].to_bits(), n.rows[1].to_bits(), n.rows[2].to_bits(), n.origin.to_bits()]
}

fn ee_rows(ee: &Ee, node: u32) -> [[u32; 4]; 4] {
    [0, 1, 2, 3].map(|k| quad(ee, node + 0x80 + 16 * k).to_bits())
}

fn response(ee: &Ee, r: u32, at: u32, angular: u32, spring: u32, friction_mask: u32, friction: u32) -> Response {
    Response {
        stiffness: F(ee.f32_at(r + at)),
        damping: F(ee.f32_at(r + at + 4)),
        restitution: F(ee.f32_at(r + at + 8)),
        angular: ee.u32_at(r + angular) != 0,
        friction: F(ee.f32_at(r + friction)),
        spring_mask: quad(ee, r + spring),
        friction_mask: quad(ee, r + friction_mask),
    }
}

fn collider_of(ee: &Ee, r: u32) -> Collider {
    use rec::*;
    let c = ee.u32_at(r + COLLIDABLE);
    Collider {
        damp: ee.u32_at(r + DAMP) != 0,
        damp_linear: F(ee.f32_at(r + DAMP_LINEAR)),
        damp_angular: F(ee.f32_at(r + DAMP_ANGULAR)),
        mass: F(ee.f32_at(r + MASS)),
        inv_mass: F(ee.f32_at(r + INV_MASS)),
        inv_inertia: quad(ee, r + INV_INERTIA),
        cofg: quad(ee, r + COFG),
        cofg_world: quad(ee, r + COFG_WORLD),
        actors: ee.u32_at(r + ACTORS),
        collides_with: ee.u32_at(r + COLLIDES),
        collidable: (c != 0).then(|| ee.u32_at(c + FLAGS)),
        max_depth: F(ee.f32_at(r + MAX_DEPTH)),
        scenery: response(ee, r, SCENERY, ANGULAR, SPRING_MASK, FRICTION_MASK, FRICTION),
        actor: response(ee, r, ACTOR, ACTOR_ANGULAR, ACTOR_SPRING_MASK, ACTOR_FRICTION_MASK, ACTOR_FRICTION),
        surfaces: core::array::from_fn(|k| F(ee.f32_at(r + SURFACES + 4 * k as u32))),
        normal_speed: F(ee.f32_at(r + NORMAL_SPEED)),
        relative: quad(ee, r + RELATIVE),
    }
}

/// The game's collections, in the order they were made.
fn collections(ee: &Ee) -> Vec<u32> {
    let mut out = Vec::new();
    let mut cell = ee.u32_at(LIST + 4);
    while cell != 0 {
        out.push(ee.u32_at(cell));
        cell = ee.u32_at(cell + 8);
    }
    out
}

/// free09 off the disc: the props as the ride loads them, the terrain's bytes and the scene's
/// collision meshes.
struct Free09 {
    set: PropSet,
    terrain: Vec<u8>,
    scene: Scene,
    props: Props,
}

fn free09() -> Option<Free09> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let names: Vec<String> = ["Tracks\\free09.res", "Tracks\\free09_2.res"]
        .iter()
        .filter_map(|want| disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case(want)).map(|e| e.name.clone()))
        .collect();
    let archives: Vec<Res> = names.iter().map(|n| disc.res(n).expect("a free09 archive")).collect();
    let res = &archives[0];
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
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let terrain = res.read_suffix(&leaf).expect("free09's terrain");
    let (props, missing) =
        Props::load(&scene, |want| archives.iter().find_map(|r| r.read_suffix(want).ok()).ok_or_else(|| "no archive of this track has it".to_string()));
    assert_eq!(missing, ["UTILITYSHACK1.col: this is a Hierarchy collision file, read as Mesh"], "free09's props the port lacks");
    Some(Free09 { set, terrain, scene, props })
}

fn same(what: &str, got: Vf, want: Vf) {
    assert_eq!(got.to_bits(), want.to_bits(), "{what}: the game {:?}, the port {:?}", want.0, got.0);
}

/// Every free09 prop's collision object as the loader makes it, against the state's.
#[test]
fn every_props_collision_object_is_the_originals() {
    let Some(ee) = image() else { return };
    let Some(free) = free09() else { return };
    let game = collections(&ee);
    assert_eq!(free.set.collections.len(), game.len(), "the collection count");
    let (mut rayed, mut bare) = (0, 0);
    for (port, &c) in free.set.collections.iter().zip(&game) {
        let name = text(&ee, c + coll::NAME);
        assert_eq!(port.objects.len() as u32, ee.u32_at(c + coll::BODIES), "{name}: the bodies");
        for (k, o) in port.objects.iter().enumerate() {
            let at = |what: &str| format!("{name} object {k}: {what}");
            let go = ee.u32_at(ee.u32_at(c + coll::BODIES + 8) + 4 * k as u32);
            let r = ee.u32_at(go + obj::COLLISION);
            let col = o.collision.as_ref().unwrap_or_else(|| panic!("{}", at("a collision object")));
            assert_ne!(r, 0, "{}", at("the game has one"));
            // The tuning, less what a contact last wrote (stale in an asleep object).
            let mut want = collider_of(&ee, r);
            want.cofg_world = col.collider.cofg_world;
            want.normal_speed = col.collider.normal_speed;
            want.relative = col.collider.relative;
            assert_eq!(col.collider, want, "{}", at("the collider"));
            let rc = &col.record;
            let flag = |a: u32| ee.u32_at(r + a) != 0;
            assert_eq!(rc.search_ground, flag(rec::SEARCH_GROUND), "{}", at("search ground"));
            assert_eq!(rc.search, flag(rec::SEARCH), "{}", at("search"));
            assert_eq!(rc.sweep_back, flag(rec::SWEEP_BACK), "{}", at("sweep back"));
            assert_eq!(rc.nearby, flag(rec::NEARBY), "{}", at("nearby"));
            assert_eq!(rc.capacity as u32, ee.u32_at(r + rec::CAPACITY), "{}", at("the contact capacity"));
            assert_eq!(rc.rest_linear.to_bits(), ee.u32_at(r + rec::REST_LINEAR), "{}", at("rest linear"));
            assert_eq!(rc.rest_angular.to_bits(), ee.u32_at(r + rec::REST_ANGULAR), "{}", at("rest angular"));
            same(&at("offset"), rc.offset, quad(&ee, r + rec::OFFSET));
            assert_eq!(rc.lead.to_bits(), ee.u32_at(r + rec::LEAD), "{}", at("lead"));
            assert_eq!(rc.push_out.to_bits(), ee.u32_at(r + rec::PUSH_OUT), "{}", at("push out"));
            assert_eq!(rc.facing.to_bits(), ee.u32_at(r + rec::FACING), "{}", at("facing"));

            assert_eq!(ee.u32_at(r + rec::NODE), ee.u32_at(go + obj::NODE), "{}", at("the record's node is the model's"));
            // The probes: the object's own, the record's the same.
            let (probe, sweep) = (ee.u32_at(go + obj::RAY_PROBE), ee.u32_at(go + obj::SWEEP_PROBE));
            assert_eq!(ee.u32_at(r + rec::RAY_PROBE), probe, "{}", at("the record's ray probe"));
            assert_eq!(ee.u32_at(r + rec::SWEEP_PROBE), sweep, "{}", at("the record's sweep probe"));
            assert_eq!(rc.sweeps, sweep != 0, "{}", at("a sweep probe"));
            match &rc.rays {
                None => {
                    assert_eq!(probe, 0, "{}", at("no ray probe"));
                    bare += 1;
                }
                Some(rays) => {
                    assert_ne!(probe, 0, "{}", at("a ray probe"));
                    let (n, list) = (ee.u32_at(probe + rec::RAY_COUNT), ee.u32_at(probe + rec::RAYS));
                    assert_eq!(rays.len() as u32, n, "{}", at("the ray count"));
                    for (i, ray) in rays.iter().enumerate() {
                        same(&at(&format!("ray {i}'s start")), ray[0], quad(&ee, list + 0x20 * i as u32));
                        same(&at(&format!("ray {i}'s end")), ray[1], quad(&ee, list + 0x20 * i as u32 + 0x10));
                    }
                    rayed += 1;
                }
            }
        }
    }
    // The loose props have rays; the signs have none.
    assert!(rayed >= 30 && bare >= 1, "{rayed} objects with rays, {bare} without");
}

/// What a knock did, by frame.
struct Knocked {
    /// The first frame on the ground after leaving it.
    landed: u32,
    /// The body put to sleep.
    slept: u32,
    /// The object's listener done: put back.
    put_back: u32,
    /// Frames the rays met one of the scene's meshes.
    on_object: u32,
}

/// One knock: the collection whose body lies nearest `near`, woken and given `velocity` and `rates`,
/// frame by frame against the original until the object's listener lets go.
fn knock(near: [f32; 3], velocity: [f32; 3], rates: [f32; 3]) -> Option<Knocked> {
    let mut ee = image()?;
    let mut free = free09()?;
    let terrain = Terrain::parse(&free.terrain).expect("free09's terrain");
    let ground = Ground::new(&terrain, &free.scene.terrain);
    let field = ground.field();
    let game = collections(&ee);
    // The one nearest.
    let pos = |c: u32| {
        let o = ee.u32_at(ee.u32_at(c + coll::BODIES + 8));
        quad(&ee, ee.u32_at(o + obj::BODY) + body_at::POSITION)
    };
    let dist = |c: u32| (0..3).map(|k| (pos(c)[k] - near[k]).powi(2)).sum::<f32>();
    let (ci, &c) = game.iter().enumerate().min_by(|a, b| dist(*a.1).total_cmp(&dist(*b.1))).unwrap();
    let name = text(&ee, c + coll::NAME);
    assert!(dist(c) < 1.0, "{name} is {} from {near:?}", dist(c).sqrt());
    assert_eq!(ee.u32_at(c + coll::BODIES), 1, "{name}: one body");
    let o = ee.u32_at(ee.u32_at(c + coll::BODIES + 8));
    let (b, node, r) = (ee.u32_at(o + obj::BODY), ee.u32_at(o + obj::NODE), ee.u32_at(o + obj::COLLISION));
    assert_eq!(ee.u32_at(c + coll::FLAGS) & coll::ACTIVE, 0, "{name} asleep in the state");
    assert_ne!(ee.u32_at(o + obj::DETECT), 0, "{name}: DetectCollisionsWhenDisabled");
    let world_node = ee.u32_at(c + coll::NODE);

    // The port's object from the state: what moves, as the state left it.
    {
        let port = &mut free.set.collections[ci];
        let body = ee_body(&ee, b);
        let d = bodies_differ(&port.bones[0].body, &body);
        assert!(d.is_empty(), "{name}: the loaded body: {d:#?}");
        assert_eq!(node_rows(&port.objects[0].node), ee_rows(&ee, node), "{name}: the loaded node");
        let col = port.objects[0].collision.as_mut().unwrap();
        col.collider.cofg_world = quad(&ee, r + rec::COFG_WORLD);
        col.collider.normal_speed = F(ee.f32_at(r + rec::NORMAL_SPEED));
        col.collider.relative = quad(&ee, r + rec::RELATIVE);
        let rc = &mut col.record;
        rc.touching = ee.u32_at(r + rec::TOUCHING) != 0;
        rc.touching_other = ee.u32_at(r + rec::TOUCHING_OTHER) != 0;
        rc.on_ground = ee.u32_at(r + rec::ON_GROUND) != 0;
        rc.on_object = ee.u32_at(r + rec::ON_OBJECT) != 0;
        rc.swept_from = quad(&ee, r + rec::SWEPT_FROM);
        rc.swept_to = quad(&ee, r + rec::SWEPT_TO);
    }

    // The wake and the toss.
    ee.call(f::ACTIVATE, &[u64::from(c)], &[], BUDGET);
    ee.call(f::ENABLE, &[u64::from(o)], &[], BUDGET);
    free.set.wake(ci, 0);
    for (at, v) in [(body_at::VELOCITY, velocity), (body_at::RATES, rates)] {
        for (k, x) in v.iter().enumerate() {
            ee.set_f32(b + at + 4 * k as u32, *x);
        }
    }
    {
        let body = &mut free.set.collections[ci].bones[0].body;
        body.velocity = Vf::new(velocity[0], velocity[1], velocity[2], body.velocity[3]);
        body.rates = Vf::new(rates[0], rates[1], rates[2], body.rates[3]);
        let d = bodies_differ(body, &ee_body(&ee, b));
        assert!(d.is_empty(), "{name}: the tossed body: {d:#?}");
    }
    assert!(free.set.collections[ci].objects[0].filed, "{name}: the wake files the collision object");
    let tail = {
        let n = ee.u32_at(world_node + 0x48);
        let at = ee.u32_at(world_node + 0x48 + 8);
        [ee.u32_at(at + 4 * (n - 2)), ee.u32_at(at + 4 * (n - 1))]
    };
    assert_eq!(tail, [c + coll::LISTENER, o + obj::LISTENER], "{name}: the wake's two listeners");

    // Frame by frame: the collision object's clear and collide, the collection's update, the listeners.
    let vt = ee.u32_at(r + rec::VTABLE);
    let (clear, collide) = (ee.u32_at(vt + rec::CLEAR), ee.u32_at(vt + rec::COLLIDE));
    let dt = 1.0f32 / 30.0;
    let mut listeners = vec![(f::SETTLE, c), (f::LISTEN, o)];
    let (mut airborne, mut touched, mut slept, mut on_object) = (false, None, None, 0);
    let mut frame = 0;
    while frame < 6000 && !listeners.is_empty() {
        frame += 1;
        ee.call(clear, &[u64::from(r)], &[], BUDGET);
        ee.call(collide, &[u64::from(r)], &[dt], BUDGET);
        ee.call(f::UPDATE, &[u64::from(c)], &[dt], BUDGET);
        let mut i = 0;
        while i < listeners.len() {
            let (fun, this) = listeners[i];
            ee.call(fun, &[u64::from(this)], &[dt], BUDGET);
            if ee.v0() as u32 != 0 {
                listeners.remove(i);
            } else {
                i += 1;
            }
        }
        free.set.frame_among(F(dt), Some(&field), Some(&free.props), None);

        let port = &free.set.collections[ci];
        let ob = &port.objects[0];
        let col = ob.collision.as_ref().unwrap();
        let at = |what: &str| format!("{name}, frame {frame}: {what}");
        let body = ee_body(&ee, b);
        let d = bodies_differ(&port.bones[0].body, &body);
        assert!(d.is_empty(), "{}: {d:#?}", at("the body"));
        assert_eq!(node_rows(&ob.node), ee_rows(&ee, node), "{}", at("the node"));
        let flag = |a: u32| ee.u32_at(r + a) != 0;
        let rc = &col.record;
        assert_eq!(
            [rc.touching, rc.touching_other, rc.on_ground, rc.on_object],
            [flag(rec::TOUCHING), flag(rec::TOUCHING_OTHER), flag(rec::ON_GROUND), flag(rec::ON_OBJECT)],
            "{}",
            at("touching, touching another, on the ground, on an object")
        );
        same(&at("swept from"), rc.swept_from, quad(&ee, r + rec::SWEPT_FROM));
        same(&at("swept to"), rc.swept_to, quad(&ee, r + rec::SWEPT_TO));
        same(&at("the contact's relative velocity"), col.collider.relative, quad(&ee, r + rec::RELATIVE));
        same(&at("the centre of mass"), col.collider.cofg_world, quad(&ee, r + rec::COFG_WORLD));
        assert_eq!(col.collider.normal_speed.to_bits(), ee.u32_at(r + rec::NORMAL_SPEED), "{}", at("the normal speed"));
        assert_eq!(ob.countdown.to_bits(), ee.u32_at(o + obj::COUNTDOWN), "{}", at("the countdown"));
        assert_eq!(ob.fresh, ee.u32_at(o + obj::FRESH) != 0, "{}", at("fresh"));
        assert_eq!(port.active, ee.u32_at(c + coll::FLAGS) & coll::ACTIVE != 0, "{}", at("active"));
        let left: Vec<u32> = listeners.iter().map(|&(fun, _)| fun).collect();
        let ported: Vec<u32> = port
            .listeners
            .iter()
            .map(|l| if *l == game_sim::prop_world::Listener::Settle { f::SETTLE } else { f::LISTEN })
            .collect();
        assert_eq!(ported, left, "{}", at("the listeners left"));
        if !rc.touching {
            airborne = true;
        }
        if rc.on_object {
            on_object += 1;
        }
        if rc.on_ground && airborne && touched.is_none() {
            touched = Some(frame);
        }
        if !body.enabled && slept.is_none() {
            slept = Some(frame);
            assert!(ob.filed, "{}", at("asleep, it stays filed (DetectCollisionsWhenDisabled)"));
        }
    }
    assert!(listeners.is_empty(), "{name}: still held after {frame} frames");
    let k = Knocked { landed: touched.expect("it lands"), slept: slept.expect("it goes to sleep"), put_back: frame, on_object };
    eprintln!("{name}: on the ground at frame {}, asleep at {}, put back at {}, {} frames on a scene mesh", k.landed, k.slept, k.put_back, k.on_object);
    Some(k)
}

/// A beach ball by the track, tossed up and away from the other balls: down onto the ground (frame
/// 35), bouncing and rolling, asleep (575) and searching on, put back (2376).
#[test]
fn a_tossed_beach_ball_lands_bounces_and_rolls_as_the_originals() {
    let Some(k) = knock([1761.45, 127.52, 5192.78], [-4.0, 18.0, 6.0], [1.5, -0.5, 2.0]) else { return };
    assert!(k.landed > 1 && k.slept > k.landed + 100 && k.put_back > k.slept, "{} {} {}", k.landed, k.slept, k.put_back);
}

/// The egg on its sculpture (`LargeEgg.col`), tossed off it south, away from the trailers (the toys'
/// meshes, which this test does not load): 49 frames its rays meet a scene mesh, then down onto the
/// ground (93), asleep (1321), put back (3122).
#[test]
fn a_tossed_egg_slides_off_its_sculpture_and_lands_as_the_originals() {
    let Some(k) = knock([3244.70, 143.67, 3977.15], [2.0, 10.0, -12.0], [0.0, 2.0, 3.0]) else { return };
    assert!(k.on_object > 0, "the egg meets its sculpture's mesh");
    assert!(k.landed > 1 && k.slept > k.landed && k.put_back > k.slept, "{} {} {}", k.landed, k.slept, k.put_back);
}
