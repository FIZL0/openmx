//! S197b — free09's beach ball striking the wreck bike in PCSX2, and the port's tick against it.
//!
//! research/s197b/drop.py wrecks the human's bike near a ball (Flat), research/s197b/put_ball.py
//! puts an awake ball just ahead of the tumbling wreck bike, and research/s197b/wreck.py records
//! watch/s197b-wreck.toml and saves the first ticks the two are close (hit2-2828 to hit2-2831). In
//! them the ball's own rays strike the wreck bike's `f.col`: entries on the ball's list naming the
//! wreck bike, their mirrors on the wreck bike's after its own ([`prop_wreck`]).
//!
//! The tick from hit2-2828 to hit2-2829 the port's way: the props' collide pass, the props' rays on
//! the wreck bike ([`prop_wreck::struck_by_props`]), the wreck bike's frame with the mirrors
//! ([`WreckBike::tick_among_struck`]), then the props' step with the wreck bike the entries' other
//! side — against the state: both lists, then both bodies.
//!
//! Skipped with a line where the disc, the kit or the states are not there (they are in s2's context
//! only; research/s197b/slots.py ram hit2-NNNN writes the RAM images).

use game_render::disc::{Disc, Res};
use game_sim::biplane::Body;
use game_sim::contact::{Contact, Node, Other, Side, Weighed};
use game_sim::phy::{Ini, Part};
use game_sim::prop_set::PropSet;
use game_sim::prop_wreck;
use game_sim::ragdoll_scene::Bike;
use game_sim::ride::WRECK_INERTIA;
use game_sim::track::{Scene, Terrain};
use game_sim::world::{Ground, Props};
use game_sim::wreck_bike::WreckBike;
use game_sim::biplane::Area;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::path::PathBuf;

/// The `.toy` actor's collection list (tests/prop_collision.rs).
const LIST: u32 = 0x011b_43f4;

/// free09's terrain: three root cells by three, nine feet a sample.
const FREE09: (f32, f32, f32, f32) = (3.0, 3.0, 9.0, 9.0);

/// The wreck bike's body and its collision object in the hit2 states (research/s197b, oracle.txt
/// S197b): the object is filed on the props node, the body alone in the node's world.
const WRECK_BODY: u32 = 0x00dc_8980;
const WRECK_OBJECT: u32 = 0x00dc_98f0;

/// The props' world node (oracle.txt S197).
const PROPS_NODE: u32 = 0x00e1_7ef0;

/// The ball that meets the wreck bike: free09's collection 2.
const BALL: usize = 2;

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

mod rec {
    pub const TOUCHING: u32 = 0x40;
    pub const TOUCHING_OTHER: u32 = 0x44;
    pub const ON_GROUND: u32 = 0x48;
    pub const ON_OBJECT: u32 = 0x4c;
    pub const COFG_WORLD: u32 = 0xa0;
    pub const SWEPT_FROM: u32 = 0xd0;
    pub const SWEPT_TO: u32 = 0xe0;
    pub const NODE: u32 = 0xf8;
    pub const NORMAL_SPEED: u32 = 0x1d8;
    pub const RELATIVE: u32 = 0x1e0;
}

fn context(kit: &Kit, rel: &str) -> PathBuf {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context").join(rel);
    if here.exists() { here } else { kit.ctx(rel) }
}

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf::from_bits([0, 4, 8, 12].map(|k| ee.u32_at(a + k)))
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

/// A list entry; the other side only as there or not (99), and as the wreck bike or not.
fn entry_at(ee: &Ee, e: u32) -> (Contact, u32) {
    let c = Contact {
        kind: ee.u32_at(e),
        from: quad(ee, e + 0x10),
        on_body: quad(ee, e + 0x20),
        normal: quad(ee, e + 0x30),
        point: quad(ee, e + 0x40),
        arm: quad(ee, e + 0x50),
        other_arm: quad(ee, e + 0x60),
        surface: ee.u32_at(e + 0x70) as i32,
        other: (ee.u32_at(e + 0x74) != 0).then_some(99),
        mover: (ee.u32_at(e + 0x78) != 0).then_some(99),
    };
    (c, ee.u32_at(e + 0x74))
}

fn list_at(ee: &Ee, r: u32) -> Vec<(Contact, u32)> {
    let (n, at) = (ee.u32_at(r + 0x100), ee.u32_at(r + 0x108));
    (0..n).map(|k| entry_at(ee, at + 0x80 * k)).collect()
}

/// The contact fields a search writes, by name, that differ.
fn searched_differ(port: &Contact, orig: &Contact) -> Vec<&'static str> {
    let mut out = Vec::new();
    for (what, p, o) in [("from", port.from, orig.from), ("on_body", port.on_body, orig.on_body), ("normal", port.normal, orig.normal), ("point", port.point, orig.point)] {
        if p.to_bits() != o.to_bits() {
            out.push(what);
        }
    }
    if port.kind != orig.kind {
        out.push("kind");
    }
    if port.surface != orig.surface {
        out.push("surface");
    }
    if port.other.is_some() != orig.other.is_some() {
        out.push("other");
    }
    out
}

fn bodies_differ(what: &str, got: &Body, want: &Body) -> Vec<String> {
    let mut bad = Vec::new();
    for (lane, p, o) in [
        ("last position", got.last_position, want.last_position),
        ("position", got.position, want.position),
        ("velocity", got.velocity, want.velocity),
        ("last quat", got.last_quat, want.last_quat),
        ("quat", got.quat, want.quat),
        ("rates", got.rates, want.rates),
        ("accel", got.accel, want.accel),
        ("ang accel", got.ang_accel, want.ang_accel),
        ("shock", got.shock, want.shock),
        ("twist", got.twist, want.twist),
    ] {
        if p.to_bits() != o.to_bits() {
            bad.push(format!("{what} {lane}: port {:?} state {:?}", p.0, o.0));
        }
    }
    bad
}

/// free09 off the disc, and the wreck bike's part and mesh.
struct Free09 {
    set: PropSet,
    terrain: Vec<u8>,
    scene: Scene,
    props: Props,
    part: Part,
    mesh: Bike,
}

fn free09() -> Option<Free09> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
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
    let (props, _) =
        Props::load(&scene, |want| archives.iter().find_map(|r| r.read_suffix(want).ok()).ok_or_else(|| "no archive of this track has it".to_string()));
    let ss = disc.res("Tracks\\free09_ss_2.res").expect("the split track's archive");
    let part = Part::read_bytes(&ss.read_suffix("bike\\wreckbike.phy").expect("wreckbike.phy"), "PhysicsPart1");
    let mesh = Bike::parse(&ss.read_suffix("wreckbikef.col").expect("wreckbikef.col")).expect("the wreck bike's mesh");
    Some(Free09 { set, terrain, scene, props, part, mesh })
}

/// The woken collections as the state left them, read into the port's (tests/prop_knock.rs's seed,
/// without its listener checks): the body, the node, the countdown, the fresh flag and the record's
/// moving parts. Answers the woken collections' indices.
fn seed(ee: &Ee, set: &mut PropSet) -> Vec<usize> {
    let game = collections(ee);
    assert_eq!(game.len(), set.collections.len(), "the state's collections");
    let mut woken = Vec::new();
    for (ci, &c) in game.iter().enumerate() {
        if ee.u32_at(c + coll::FLAGS) & coll::ACTIVE == 0 {
            continue;
        }
        let o = object(ee, ci);
        let (b, node, r) = (ee.u32_at(o + obj::BODY), ee.u32_at(o + obj::NODE), ee.u32_at(o + obj::COLLISION));
        set.wake(ci, 0);
        let port = &mut set.collections[ci];
        let body = ee_body(ee, b);
        if !body.enabled {
            port.disable(0);
        }
        port.bones[0].body = body;
        let ob = &mut port.objects[0];
        ob.node = node_at(ee, node);
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
        woken.push(ci);
    }
    woken
}

/// The port's wreck bike as the state holds it.
fn wreck_at(ee: &Ee, part: &Part) -> WreckBike {
    let (b, c) = (WRECK_BODY, WRECK_OBJECT);
    let body = ee_body(ee, b);
    let mut bike = WreckBike::new(part, WRECK_INERTIA, body.position, body.quat);
    bike.body = body;
    bike.collider.cofg_world = quad(ee, c + rec::COFG_WORLD);
    bike.collider.normal_speed = F(ee.f32_at(c + rec::NORMAL_SPEED));
    bike.collider.relative = quad(ee, c + rec::RELATIVE);
    let r = &mut bike.record;
    r.touching = ee.u32_at(c + rec::TOUCHING) != 0;
    r.touching_other = ee.u32_at(c + rec::TOUCHING_OTHER) != 0;
    r.on_ground = ee.u32_at(c + rec::ON_GROUND) != 0;
    r.on_object = ee.u32_at(c + rec::ON_OBJECT) != 0;
    r.swept_from = quad(ee, c + rec::SWEPT_FROM);
    r.swept_to = quad(ee, c + rec::SWEPT_TO);
    r.node = node_at(ee, ee.u32_at(c + rec::NODE));
    bike
}

fn load(kit: &Kit, frame: u32) -> Option<Ee> {
    let state = context(kit, &format!("states/s197b-hit2-{frame}.eeMemory.bin"));
    let mut ee = Ee::from_ram(&state).map_err(|why| skip(&format!("{why} — research/s197b/slots.py ram hit2-{frame} writes it"))).ok()?;
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    assert_eq!(ee.u32_at(addr("frame_counter").unwrap()), frame, "the state is not frame {frame}");
    Some(ee)
}

/// One tick from `frame` the port's way, against `frame + 1`: answers what differs.
fn tick(kit: &Kit, frame: u32) -> Option<Vec<String>> {
    let (before, after) = (load(kit, frame)?, load(kit, frame + 1)?);
    let mut free = free09()?;
    let terrain = Terrain::parse(&free.terrain).expect("free09's terrain");
    let ground = Ground::new(&terrain, &free.scene.terrain);
    let field = ground.field();
    let woken = seed(&before, &mut free.set);
    assert!(woken.contains(&BALL), "ball {BALL} woken in hit2-{frame}: {woken:?}");
    let filed: Vec<u32> = (0..before.u32_at(PROPS_NODE + 0x30)).map(|k| before.u32_at(before.u32_at(PROPS_NODE + 0x38) + 4 * k)).collect();
    assert!(filed.contains(&WRECK_OBJECT), "the wreck bike's object on the props node: {filed:x?}");
    assert_eq!(before.u32_at(WRECK_OBJECT + 0xf4), WRECK_BODY, "the object's body");
    let mut bike = wreck_at(&before, &free.part);
    let dt = F(after.f32_at(addr("frame_dt").unwrap()));

    let movers = free.set.search(dt, Some(&field), Some(&free.props), None);
    let mut strike = prop_wreck::struck_by_props(&mut free.set, &free.mesh.mesh, &bike, 0);
    let mut bad = Vec::new();
    eprintln!("hit2-{frame}: {} mirrors, struck {:?}", strike.mirrors.len(), strike.struck.iter().map(|p| (p.collection, p.object)).collect::<Vec<_>>());
    if strike.is_empty() {
        bad.push("the ball's rays struck nothing".to_string());
    }

    // The ball's list: its own entries, then those naming the wreck bike.
    let ball_rec = after.u32_at(object(&after, BALL) + obj::COLLISION);
    let want: Vec<(Contact, u32)> = list_at(&after, ball_rec);
    let got = &free.set.collections[BALL].objects[0].collision.as_ref().unwrap().record.contacts;
    eprintln!("hit2-{frame}: the ball's list, port {} state {}", got.len(), want.len());
    if got.len() != want.len() {
        bad.push(format!("the ball's list: port {} entries, state {}", got.len(), want.len()));
    }
    for (k, (p, (o, other))) in got.iter().zip(&want).enumerate() {
        let d = searched_differ(p, o);
        let names_bike = p.other == Some(0);
        if !d.is_empty() || names_bike != (*other == WRECK_OBJECT) {
            bad.push(format!("the ball's entry {k}: {d:?}, port names the bike {names_bike}, state's other {other:#x}"));
        }
    }

    // The wreck bike's list after its frame: its own entries, then the mirrors.
    bike.tick_among_struck(dt, Some(&field), Some(&free.props), None, &mut strike);
    let want: Vec<(Contact, u32)> = list_at(&after, WRECK_OBJECT);
    let got = &bike.record.contacts;
    eprintln!("hit2-{frame}: the wreck bike's list, port {} state {}", got.len(), want.len());
    if got.len() != want.len() {
        bad.push(format!("the wreck bike's list: port {} entries, state {}", got.len(), want.len()));
    }
    for (k, (p, (o, _))) in got.iter().zip(&want).enumerate() {
        let d = searched_differ(p, o);
        if !d.is_empty() {
            bad.push(format!("the wreck bike's entry {k}: {d:?}"));
        }
    }
    bad.extend(bodies_differ("the wreck bike (its frame)", &bike.body, &ee_body(&after, WRECK_BODY)));

    // The props' step, the wreck bike the entries' other side.
    {
        let WreckBike { body, mass, collider, .. } = &mut bike;
        let mut rigid = Weighed { body, mass: *mass };
        free.set.step(dt, &movers, &mut [Other { collider, side: Side::Body(&mut rigid) }]);
    }
    let ball = &free.set.collections[BALL].bones[0].body;
    bad.extend(bodies_differ("the ball", ball, &ee_body(&after, after.u32_at(object(&after, BALL) + obj::BODY))));
    bad.extend(bodies_differ("the wreck bike (after the props)", &bike.body, &ee_body(&after, WRECK_BODY)));
    Some(bad)
}

/// hit2-2828 to 2831, each tick from its state: the ball's rays on the wreck bike, the lists and
/// both bodies bit for bit.
#[test]
fn the_ball_strikes_the_wreck_bike() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    for frame in 2828..2831 {
        let Some(bad) = tick(&kit, frame) else { return };
        assert!(bad.is_empty(), "hit2-{frame} -> {}: {bad:#?}", frame + 1);
    }
}
