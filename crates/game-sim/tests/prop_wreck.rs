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
//! S244 — the other way round and the impulse. research/s244/strike.py records the hit3 and hit5
//! runs: in hit3 (2829 to 2835) the wreck bike's own rays strike a ball, its entries naming the ball
//! and their mirrors at the head of the ball's list ([`prop_wreck::wreck_rays_on_props`]; 2832 fails
//! without them); in hit5 (2851 to 2857, research/s244/throw_ball.py throwing ball 4 at the wreck bike
//! from behind) the ball's impulse in the props' step throws the wreck bike at 2855. Then the same
//! ticks through a [`Sim`]: [`Sim::wreck_strike`], the wreck bike's frame, [`Sim::wreck_pushed_back`],
//! [`Sim::props_step`].
//!
//! Skipped with a line where the disc, the kit or the states are not there (they are in s2's context
//! for hit2 and s3's for hit3/hit5; research/s197b/slots.py ram hit2-NNNN and research/s244/ram.py
//! hit3-NNNN / hit5-NNNN write the RAM images).

use game_render::disc::{Disc, Res};
use game_sim::Sim;
use game_sim::biplane::Body;
use game_sim::contact::{Contact, Node, Other, Side, Weighed};
use game_sim::phy::{Ini, Part, Wrecks};
use game_sim::prop_set::{PropMovers, PropSet};
use game_sim::flight_search::{self, Plane};
use game_sim::math::Mat4;
use game_sim::prop_world;
use game_sim::prop_wreck;
use game_sim::ragdoll_scene::Bike;
use game_sim::ride::{Crash, WRECK_INERTIA};
use game_sim::trick::Wreck;
use game_sim::track::{Scene, Terrain};
use game_sim::world::{Ground, Props};
use game_sim::wreck_bike::WreckBike;
use game_sim::biplane::Area;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::path::PathBuf;
use std::sync::Arc;

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

/// A run's states: hit2's (S197b) or hit3's (S244), and the script that writes their RAM images.
#[derive(Clone, Copy)]
struct Run {
    name: &'static str,
    ram: &'static str,
    /// The ball that meets the wreck bike: its collection.
    ball: usize,
}

const HIT2: Run = Run { name: "s197b-hit2", ram: "research/s197b/slots.py ram hit2", ball: BALL };
const HIT3: Run = Run { name: "s244-hit3", ram: "research/s244/ram.py hit3", ball: BALL };
const HIT5: Run = Run { name: "s244-hit5", ram: "research/s244/ram.py hit5", ball: 4 };

fn load(kit: &Kit, run: Run, frame: u32) -> Option<Ee> {
    let state = context(kit, &format!("states/{}-{frame}.eeMemory.bin", run.name));
    let mut ee = Ee::from_ram(&state).map_err(|why| skip(&format!("{why} — {}-{frame} writes it", run.ram))).ok()?;
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    assert_eq!(ee.u32_at(addr("frame_counter").unwrap()), frame, "the state is not frame {frame}");
    Some(ee)
}

/// What one tick found: what differs from the state after it, whether the ball's rays struck the
/// wreck bike, whether the wreck bike's struck a prop, whether the wreck bike's frame pushed a prop's body (an impulse on an entry of its own
/// naming the prop, which goes into both bodies), and whether the props' step pushed the wreck bike's
/// (the same on a prop's entry naming the wreck bike).
struct Ticked {
    bad: Vec<String>,
    struck: bool,
    entries: bool,
    pushed: bool,
    thrown: bool,
}

/// How a tick finds the strikes between the props and the wreck bike.
#[derive(Clone, Copy, PartialEq)]
enum Way {
    /// The props' rays on the wreck bike alone, searched after the pass (S197b's stand-in,
    /// [`prop_wreck::struck_by_props`]).
    Props,
    /// Those and the wreck bike's rays on the props (S244, [`prop_wreck::strikes`]).
    Rays,
    /// The wreck bike's hull in the props' own world for their search (S243, [`PropSet::search_among`]).
    Node,
}

/// One tick of `run` from `frame` the port's way (`way`), against `frame + 1`.
fn tick(kit: &Kit, run: Run, frame: u32, way: Way) -> Option<Ticked> {
    let (before, after) = (load(kit, run, frame)?, load(kit, run, frame + 1)?);
    let mut free = free09()?;
    let terrain = Terrain::parse(&free.terrain).expect("free09's terrain");
    let ground = Ground::new(&terrain, &free.scene.terrain);
    let field = ground.field();
    let woken = seed(&before, &mut free.set);
    assert!(woken.contains(&run.ball), "ball {} woken in {}-{frame}: {woken:?}", run.ball, run.name);
    let filed: Vec<u32> = (0..before.u32_at(PROPS_NODE + 0x30)).map(|k| before.u32_at(before.u32_at(PROPS_NODE + 0x38) + 4 * k)).collect();
    assert!(filed.contains(&WRECK_OBJECT), "the wreck bike's object on the props node: {filed:x?}");
    assert_eq!(before.u32_at(WRECK_OBJECT + 0xf4), WRECK_BODY, "the object's body");
    let mut bike = wreck_at(&before, &free.part);
    let dt = F(after.f32_at(addr("frame_dt").unwrap()));

    let (movers, mut strike) = if way == Way::Node {
        let n = &bike.record.node;
        let at = Mat4([n.rows[0], n.rows[1], n.rows[2], n.origin]);
        let hull = prop_world::Wreck { hull: Plane { mesh: &free.mesh.mesh, at, velocity: bike.body.velocity }, side: 0 };
        let (movers, mut strikes) = free.set.search_among(dt, Some(&field), Some(&free.props), None, &[hull]);
        // The ball's kept list holds the hull (after the props' collidables), as the state's holds
        // the wreck bike's mesh collidable (its owner at +0x80 the wreck bike's object), at the same
        // distance less its reach.
        let search = before.u32_at(before.u32_at(object(&before, run.ball) + obj::COLLISION) + 0x10c);
        let (list, n) = (before.u32_at(search), before.u32_at(search + 4));
        let want = (0..n).map(|i| list + 8 * i).find(|&e| before.u32_at(before.u32_at(e) + 0x80) == WRECK_OBJECT).map(|e| before.f32_at(e + 4));
        let kept = &free.set.collections[run.ball].objects[0].collision.as_ref().unwrap().nearby.nearby.found;
        let got = kept.iter().find(|c| c.object == flight_search::FIRST + free.props.props.len()).map(|c| c.distance.0);
        eprintln!("{}-{frame}: the wreck bike in the ball's list at {got:?}, state {want:?}", run.name);
        assert!(want.is_some(), "the state's ball list holds the wreck bike");
        assert_eq!(got.map(f32::to_bits), want.map(f32::to_bits), "the wreck bike's distance in the ball's kept list");
        (movers, strikes.remove(0))
    } else {
        let movers = free.set.search(dt, Some(&field), Some(&free.props), None);
        let strike = if way == Way::Rays {
            prop_wreck::strikes(&mut free.set, &free.mesh.mesh, &bike, 0)
        } else {
            prop_wreck::struck_by_props(&mut free.set, &free.mesh.mesh, &bike, 0)
        };
        (movers, strike)
    };
    let mut bad = Vec::new();
    let struck = !strike.mirrors.is_empty();
    let entries = !strike.entries.is_empty();
    eprintln!(
        "{}-{frame}: {} entries, {} mirrors, struck {:?}",
        run.name,
        strike.entries.len(),
        strike.mirrors.len(),
        strike.struck.iter().map(|p| (p.collection, p.object)).collect::<Vec<_>>()
    );

    // The ball's list: its own entries, then those naming the wreck bike.
    let ball_rec = after.u32_at(object(&after, run.ball) + obj::COLLISION);
    let want: Vec<(Contact, u32)> = list_at(&after, ball_rec);
    let got = &free.set.collections[run.ball].objects[0].collision.as_ref().unwrap().record.contacts;
    eprintln!("{}-{frame}: the ball's list, port {} state {}", run.name, got.len(), want.len());
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

    // The wreck bike's list after its frame: its own entries, the entries its rays struck on the
    // props, then the mirrors. Its world's step pushes what its entries name, written back before
    // the props' step (Sim::wreck_pushed_back).
    let found: Vec<Body> = strike.struck.iter().map(|p| p.body).collect();
    bike.tick_among_struck(dt, Some(&field), Some(&free.props), None, &mut strike);
    let pushed = strike.struck.iter().zip(&found).any(|(p, b)| !bodies_differ("", &p.body, b).is_empty());
    free.set.pushed(&strike.struck);
    let want: Vec<(Contact, u32)> = list_at(&after, WRECK_OBJECT);
    let got = &bike.record.contacts;
    eprintln!("{}-{frame}: the wreck bike's list, port {} state {}", run.name, got.len(), want.len());
    if got.len() != want.len() {
        bad.push(format!("the wreck bike's list: port {} entries, state {}", got.len(), want.len()));
    }
    for (k, (p, (o, _))) in got.iter().zip(&want).enumerate() {
        let d = searched_differ(p, o);
        if !d.is_empty() {
            bad.push(format!("the wreck bike's entry {k}: {d:?}"));
        }
    }
    // The state's body is the props' step's: the wreck bike as its frame left it is the state's only
    // where the props' step leaves it.
    let frame_bad = bodies_differ("the wreck bike (its frame)", &bike.body, &ee_body(&after, WRECK_BODY));

    // The props' step, the wreck bike the entries' other side.
    let framed = bike.body;
    {
        let WreckBike { body, mass, collider, .. } = &mut bike;
        let mut rigid = Weighed { body, mass: *mass };
        free.set.step(dt, &movers, &mut [Other { collider, side: Side::Body(&mut rigid) }]);
    }
    let ball = &free.set.collections[run.ball].bones[0].body;
    bad.extend(bodies_differ("the ball", ball, &ee_body(&after, after.u32_at(object(&after, run.ball) + obj::BODY))));
    bad.extend(bodies_differ("the wreck bike (after the props)", &bike.body, &ee_body(&after, WRECK_BODY)));
    let thrown = !bodies_differ("", &bike.body, &framed).is_empty();
    if !thrown {
        bad.extend(frame_bad);
    }
    eprintln!(
        "{}-{frame}: the wreck bike's frame {} a prop, the props' step {} the wreck bike",
        run.name,
        if pushed { "pushed" } else { "left" },
        if thrown { "pushed" } else { "left" }
    );
    Some(Ticked { bad, struck, entries, pushed, thrown })
}

/// hit2-2828 to 2831, each tick from its state: the ball's rays on the wreck bike, the lists and
/// both bodies bit for bit.
#[test]
fn the_ball_strikes_the_wreck_bike() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    for frame in 2828..2831 {
        let Some(Ticked { bad, struck, .. }) = tick(&kit, HIT2, frame, Way::Rays) else { return };
        assert!(struck, "hit2-{frame}: the ball's rays struck nothing");
        assert!(bad.is_empty(), "hit2-{frame} -> {}: {bad:#?}", frame + 1);
    }
}

/// S243: the same ticks with the wreck bike's hull in the ball's own world, gathered into its kept
/// nearby list with the scene and searched with one ray record over both.
#[test]
fn the_wreck_bike_is_in_the_balls_world() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    for frame in 2828..2831 {
        let Some(Ticked { bad, struck, .. }) = tick(&kit, HIT2, frame, Way::Node) else { return };
        assert!(struck, "hit2-{frame}: the ball's rays struck nothing");
        assert!(bad.is_empty(), "hit2-{frame} -> {}: {bad:#?}", frame + 1);
    }
}

/// S244: hit3 (research/s244/strike.py, hit2's run again from its put_ball.py state with a state on
/// every tick) carries on past hit2's states to the tick the wreck bike's own rays strike the ball's
/// `f.col` (2832 -> 2833: four entries on the wreck bike's list naming the ball, their mirrors at the
/// head of the ball's, [`prop_wreck::wreck_rays_on_props`]). The two are parting, so no impulse: the
/// wreck bike's spring and friction on its entries read the ball's velocity, the ball's on its mirrors
/// the wreck bike's — the wreck bike slowed by 2.1 ft/s, the ball thrown on by 10 ft/s up. Every tick
/// from 2829 to 2835 is the original's bit for bit; without the wreck bike's rays (the props' rays
/// alone, S197b) 2832 is not.
#[test]
fn the_wreck_bikes_rays_strike_the_ball() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    let mut struck = Vec::new();
    for frame in 2829..2836 {
        let Some(t) = tick(&kit, HIT3, frame, Way::Rays) else { return };
        assert!(t.bad.is_empty(), "hit3-{frame} -> {}: {:#?}", frame + 1, t.bad);
        if t.entries {
            struck.push(frame);
        }
    }
    assert!(struck.starts_with(&[2832]), "the wreck bike's rays struck the ball in ticks {struck:?}, not first from 2832");
    let Some(t) = tick(&kit, HIT3, 2832, Way::Props) else { return };
    assert!(!t.bad.is_empty(), "hit3-2832 matches without the wreck bike's rays on the ball");
    eprintln!("hit3-2832 without the wreck bike's rays: {:#?}", t.bad);
}

/// S244: hit5 (research/s244/throw_ball.py from hit3's put state, run on to frame 2848: ball 4 put
/// 14 ft behind the wreck bike, 2.5 ft up, thrown along its way at 55 ft/s; then research/s244/strike.py)
/// — ball 4 catches the wreck bike up at some 35 ft/s, far past a hard contact's 10, on 2855 -> 2856:
/// its own entries on the wreck bike take the impulse in the props' step and it goes into the wreck
/// bike's body too ([`game_sim::Sim::props_step`]'s other side), the wreck bike thrown on and the ball
/// slowed. Every tick from 2851 to 2857 bit for bit — on 2855 the state's wreck bike is the props'
/// step's, not its frame's alone.
#[test]
fn the_balls_impulse_moves_the_wreck_bike() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    let mut thrown = Vec::new();
    for frame in 2851..2858 {
        let Some(t) = tick(&kit, HIT5, frame, Way::Rays) else { return };
        assert!(t.bad.is_empty(), "hit5-{frame} -> {}: {:#?}", frame + 1, t.bad);
        assert!(!t.pushed, "hit5-{frame}: the wreck bike's frame pushed a prop");
        if t.thrown {
            thrown.push(frame);
        }
    }
    assert!(thrown.contains(&2855), "the props' step pushed the wreck bike in ticks {thrown:?}, not from 2855");
}

// --- the Sim's wiring --------------------------------------------------------------------------------

/// The ragdoll's kit as the shell builds it (tests/ragdoll_props.rs's `ride_kit`), its wreck bike
/// mesh free09's (`mesh`): [`game_sim::Sim::wreck_strike`] reads the mesh off the kit.
fn ride_kit(mesh: &Bike) -> Option<game_sim::ragdoll_ride::Kit> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
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
    let mut kit = game_sim::ragdoll_ride::Kit::new(&rider_file, doll.rig);
    kit.bike_mesh = Some(Arc::new(mesh.clone()));
    Some(kit)
}

/// One tick of `run` from `frame` through the Sim (S244): a one-player [`Sim`] with free09's props
/// as the state has them in its traffic and its rider down, the wreck bike the state's, then the
/// tick's prop half in the step paths' order — the props' collide pass, [`Sim::wreck_strike`], the
/// wreck bike's frame ([`WreckBike::tick_among`], as [`game_sim::ride::Crash::step`] runs it),
/// [`Sim::wreck_pushed_back`], [`Sim::props_step`] — against `frame + 1`: the ball's list, both
/// bodies, the bike's strike spent. The rider's own bike stands where [`Sim::new`] puts it, far off.
fn sim_tick(kit: &Kit, run: Run, frame: u32) -> Option<Vec<String>> {
    let (before, after) = (load(kit, run, frame)?, load(kit, run, frame + 1)?);
    let mut free = free09()?;
    let doll = ride_kit(&free.mesh)?;
    let terrain = Terrain::parse(&free.terrain).expect("free09's terrain");
    let ground = Ground::new(&terrain, &free.scene.terrain);
    let field = ground.field();
    seed(&before, &mut free.set);
    let dt = F(after.f32_at(addr("frame_dt").unwrap()));

    let mut sim = Sim::new(0x5244);
    sim.traffic.props = free.set.clone();
    sim.rider.carries_wreck(free.part, Wrecks::default(), WRECK_INERTIA);
    sim.rider.carries_ragdoll(Arc::new(doll));
    let mut crash = Crash::begin(&sim.rider.ride, Wreck(5), &free.part, WRECK_INERTIA, &Wrecks::default(), None);
    crash.bike = wreck_at(&before, &free.part);
    sim.rider.crash = Some(crash);
    let me = sim.players();

    let movers = sim.traffic.props.search(dt, Some(&field), Some(&free.props), None);
    let mut want_set = sim.traffic.props.clone();
    let want = prop_wreck::strikes(&mut want_set, &free.mesh.mesh, &sim.rider.crash.as_ref().unwrap().bike, me);
    sim.wreck_strike(0);
    let mut bad = Vec::new();
    let bike = &mut sim.rider.crash.as_mut().unwrap().bike;
    if bike.struck != want {
        bad.push(format!("Sim::wreck_strike filed {:?}, prop_wreck::strikes {want:?}", bike.struck));
    }

    // The ball's list as the state has it, the wreck bike named as the Sim's other side `me`.
    let got = &sim.traffic.props.collections[run.ball].objects[0].collision.as_ref().unwrap().record.contacts;
    let state: Vec<(Contact, u32)> = list_at(&after, after.u32_at(object(&after, run.ball) + obj::COLLISION));
    if got.len() != state.len() {
        bad.push(format!("the ball's list: Sim {} entries, state {}", got.len(), state.len()));
    }
    for (k, (p, (o, other))) in got.iter().zip(&state).enumerate() {
        let d = searched_differ(p, o);
        if !d.is_empty() || (p.other == Some(me)) != (*other == WRECK_OBJECT) {
            bad.push(format!("the ball's entry {k}: {d:?}, names {:?}, state's other {other:#x}", p.other));
        }
    }

    bike.tick_among(dt, Some(&field), Some(&free.props), None);
    sim.wreck_pushed_back(0);
    if sim.rider.crash.as_ref().unwrap().bike.struck != prop_wreck::WreckStrike::default() {
        bad.push("the wreck bike's strike not spent".to_string());
    }
    let n = sim.traffic.props.collections.len();
    sim.props_step(dt, &PropMovers { sets: vec![movers], of: vec![Some(0); n], near: Vec::new() });
    let ball = &sim.traffic.props.collections[run.ball].bones[0].body;
    bad.extend(bodies_differ("the ball", ball, &ee_body(&after, after.u32_at(object(&after, run.ball) + obj::BODY))));
    bad.extend(bodies_differ("the wreck bike", &sim.rider.crash.as_ref().unwrap().bike.body, &ee_body(&after, WRECK_BODY)));
    Some(bad)
}

/// **The Sim's wiring** (S197b's follow-up, S244): [`Sim::wreck_strike`] files both ways' strikes as
/// [`prop_wreck::strikes`] does, the wreck bike named the Sim's other side `players`, its frame spends
/// them, [`Sim::wreck_pushed_back`] and [`Sim::props_step`] carry the pushes across — the ball and the
/// wreck bike the original's bit for bit on hit2-2828 (the ball's rays on the wreck bike), hit3-2832
/// (the wreck bike's on the ball) and hit5-2855 (the ball's impulse into the wreck bike).
#[test]
fn the_sim_wires_the_wreck_bikes_strikes() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    for (run, frame) in [(HIT2, 2828), (HIT3, 2832), (HIT5, 2855)] {
        let Some(bad) = sim_tick(&kit, run, frame) else { return };
        assert!(bad.is_empty(), "{}-{frame} -> {} through the Sim: {bad:#?}", run.name, frame + 1);
    }
}
