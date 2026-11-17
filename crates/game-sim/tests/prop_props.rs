//! S242 — free09's beach ball dropped on another in PCSX2, and the port's ticks against it.
//!
//! research/s242/put_ball.py puts ball 1, awake, sixteen feet over ball 0 at rest (off s197b's
//! hit2-2828), and research/s242/strike.py records watch/s197b-wreck.toml and saves the ticks the two
//! meet (drop2-2868 to drop2-2873). In them ball 1's rays strike ball 0's `f.col` (ball 0 woken,
//! filed at the end of the node's list and searched in the same pass, its rays striking back), and
//! then each other's in node order ([`prop_props`]).
//!
//! Each tick from its state the port's way — the props' collide pass with the props' rays on the
//! props ([`PropSet::search`]), then the props' step — against the next state: both balls' lists,
//! then both bodies, bit for bit.
//!
//! F21 — the same within one collection: free11's bowling pin 1, woken and tossed onto pin 0 by
//! research/f21/toss.py, its rays on pin 0's `f.col` (lean-209 to lean-214), against the same checks
//! with every body of a collection seeded.
//!
//! Skipped with a line where the disc, the kit or the states are not there (they are in s3's and s5's
//! context only; research/s242/slots.py ram drop2-NNNN and research/f21/slots.py ram lean-NNNN write
//! the RAM images).

use game_render::disc::{Disc, Res};
use game_sim::biplane::Area;
use game_sim::biplane::Body;
use game_sim::contact::{Contact, Node};
use game_sim::phy::Ini;
use game_sim::prop_props::{self, PROP_OTHER};
use game_sim::prop_set::PropSet;
use game_sim::track::{Scene, Terrain};
use game_sim::world::{Ground, Props};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::path::PathBuf;

/// The `.toy` actor's collection list (tests/prop_collision.rs).
const LIST: u32 = 0x011b_43f4;

/// free09's terrain: three root cells by three, nine feet a sample.
const FREE09: (f32, f32, f32, f32) = (3.0, 3.0, 9.0, 9.0);

/// The props' world node (oracle.txt S197): its filed list's count and array.
const PROPS_NODE: u32 = 0x00e1_7ef0;

/// The two balls that meet: free09's collections 0 (at rest under) and 1 (dropped).
const BALLS: [usize; 2] = [0, 1];

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

/// A list entry; the other side only as there or not (99), and the collision object it names.
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

/// free09 off the disc.
struct Free09 {
    set: PropSet,
    terrain: Vec<u8>,
    scene: Scene,
    props: Props,
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
    Some(Free09 { set, terrain, scene, props })
}

/// The node's filed collision objects, in its list's order.
fn filed(ee: &Ee) -> Vec<u32> {
    (0..ee.u32_at(PROPS_NODE + 0x30)).map(|k| ee.u32_at(ee.u32_at(PROPS_NODE + 0x38) + 4 * k)).collect()
}

/// Every collection as the state left it, read into the port's (tests/prop_wreck.rs's seed, the
/// resting ones too: a ball at rest is struck where its node stands): the body, the node, the
/// countdown, the fresh flag, the record's moving parts, and its place in the node's list.
fn seed(ee: &Ee, set: &mut PropSet) {
    let game = collections(ee);
    assert_eq!(game.len(), set.collections.len(), "the state's collections");
    let node = filed(ee);
    for (ci, &c) in game.iter().enumerate() {
        let o = object(ee, ci);
        let (b, n, r) = (ee.u32_at(o + obj::BODY), ee.u32_at(o + obj::NODE), ee.u32_at(o + obj::COLLISION));
        let body = ee_body(ee, b);
        if ee.u32_at(c + coll::FLAGS) & coll::ACTIVE != 0 {
            set.wake(ci, 0);
            if !body.enabled {
                set.collections[ci].disable(0);
            }
        }
        let port = &mut set.collections[ci];
        port.bones[0].body = body;
        let ob = &mut port.objects[0];
        ob.node = node_at(ee, n);
        ob.countdown = F(ee.f32_at(o + obj::COUNTDOWN));
        ob.fresh = ee.u32_at(o + obj::FRESH) != 0;
        let at = node.iter().position(|&f| f == r);
        assert_eq!(ob.filed, at.is_some(), "collection {ci}'s object filed as the node has it");
        ob.filed_at = at.map_or(0, |k| k as u32 + 1);
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
    }
}

fn load(kit: &Kit, frame: u32) -> Option<Ee> {
    let state = context(kit, &format!("states/s242-drop2-{frame}.eeMemory.bin"));
    let mut ee = Ee::from_ram(&state).map_err(|why| skip(&format!("{why} — research/s242/slots.py ram drop2-{frame} writes it"))).ok()?;
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
    seed(&before, &mut free.set);
    let dt = F(after.f32_at(addr("frame_dt").unwrap()));
    // Each collection's collision object, by the port's flat place.
    let objects: Vec<u32> = (0..free.set.collections.len()).map(|c| after.u32_at(object(&after, c) + obj::COLLISION)).collect();
    let mut bad = Vec::new();

    let movers = free.set.search(dt, Some(&field), Some(&free.props), None);
    let order: Vec<u32> = free.set.node_order().iter().map(|&(c, _)| objects[c]).collect();
    let want: Vec<u32> = filed(&after).into_iter().filter(|r| objects.contains(r)).collect();
    if order != want {
        bad.push(format!("the node's balls: port {order:x?} state {want:x?}"));
    }
    for (c, &r) in objects.iter().enumerate() {
        let want: Vec<(Contact, u32)> = list_at(&after, r);
        let col = free.set.collections[c].objects[0].collision.as_ref().unwrap();
        let got = &col.record.contacts;
        let names = |e: &Contact| e.other.and_then(|o| o.checked_sub(PROP_OTHER)).map(|f| objects[f]);
        if BALLS.contains(&c) {
            eprintln!(
                "drop2-{frame}: ball {c}'s list, port {:?} state {:?}",
                got.iter().map(|e| (e.kind, names(e).map(|r| format!("{r:#x}")))).collect::<Vec<_>>(),
                want.iter().map(|(e, o)| (e.kind, format!("{o:#x}"))).collect::<Vec<_>>()
            );
            if got.len() != want.len() {
                bad.push(format!("ball {c}'s list: port {} entries, state {}", got.len(), want.len()));
            }
            for (k, (p, (o, other))) in got.iter().zip(&want).enumerate() {
                let d = searched_differ(p, o);
                let named = names(p);
                if !d.is_empty() || named.is_some() != objects.contains(other) || named.is_some_and(|n| n != *other) {
                    bad.push(format!("ball {c}'s entry {k}: {d:?}, port names {named:x?}, state's other {other:#x}"));
                }
            }
            let r0 = &col.record;
            let flags = [r0.touching, r0.touching_other, r0.on_ground, r0.on_object];
            let want = [rec::TOUCHING, rec::TOUCHING_OTHER, rec::ON_GROUND, rec::ON_OBJECT].map(|o| after.u32_at(r + o) != 0);
            if flags != want {
                bad.push(format!("ball {c}'s flags: port {flags:?} state {want:?}"));
            }
        } else {
            let port = got.iter().any(|e| names(e).is_some());
            let state = want.iter().any(|(_, o)| objects.contains(o));
            if port || state {
                bad.push(format!("collection {c} names a prop: port {port} state {state}"));
            }
        }
    }

    free.set.step(dt, &movers, &mut []);
    for c in BALLS {
        let ball = &free.set.collections[c].bones[0].body;
        bad.extend(bodies_differ(&format!("ball {c}"), ball, &ee_body(&after, after.u32_at(object(&after, c) + obj::BODY))));
    }
    Some(bad)
}

/// drop2-2868 to 2872, each tick from its state: the balls' rays on each other, the node's order, the
/// lists and both bodies bit for bit.
#[test]
fn a_ball_strikes_a_ball() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    let mut bad = Vec::new();
    for frame in 2868..2873 {
        let Some(b) = tick(&kit, frame) else { return };
        bad.extend(b.into_iter().map(|b| format!("drop2-{frame}: {b}")));
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// drop2-2868 run on its own to 2873, the port's props never put back to a state: both balls' bodies
/// each tick.
#[test]
fn a_ball_on_a_ball_runs_on() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    let Some(first) = load(&kit, 2868) else { return };
    let Some(mut free) = free09() else { return };
    let terrain = Terrain::parse(&free.terrain).expect("free09's terrain");
    let ground = Ground::new(&terrain, &free.scene.terrain);
    let field = ground.field();
    seed(&first, &mut free.set);
    let mut bad = Vec::new();
    for frame in 2869..2874 {
        let Some(after) = load(&kit, frame) else { return };
        let dt = F(after.f32_at(addr("frame_dt").unwrap()));
        let movers = free.set.search(dt, Some(&field), Some(&free.props), None);
        free.set.step(dt, &movers, &mut []);
        for c in BALLS {
            let ball = &free.set.collections[c].bones[0].body;
            bad.extend(bodies_differ(&format!("drop2-{frame} ball {c}"), ball, &ee_body(&after, after.u32_at(object(&after, c) + obj::BODY))));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// The flat places name every prop once, collection by collection.
#[test]
fn flat_places_round_trip() {
    let Some(free) = free09() else { return };
    let set = &free.set;
    let mut f = 0;
    for (c, coll) in set.collections.iter().enumerate() {
        for k in 0..coll.objects.len() {
            assert_eq!(prop_props::flat(set, c, k), f);
            assert_eq!(prop_props::unflat(set, f), Some((c, k)));
            f += 1;
        }
    }
    assert_eq!(prop_props::unflat(set, f), None);
}

/// F21: free11's `.toy` collection list and its props' world node (research/f21/lists.py).
const FREE11_LIST: u32 = 0x00a9_d7d4;
const FREE11_NODE: u32 = 0x007a_3d40;

/// The two of free11's ten bowling pins (its one collection of ten bodies) that meet: pin 1 tossed
/// onto pin 0, research/f21/toss.py.
const MET: [usize; 2] = [0, 1];

/// free11 off the disc, its terrain's area from its scene (c16_fly.rs's).
fn free11() -> Option<Free09> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let names: Vec<String> = ["Tracks\\free11.res", "Tracks\\free11_2.res"]
        .iter()
        .filter_map(|want| disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case(want)).map(|e| e.name.clone()))
        .collect();
    let archives: Vec<Res> = names.iter().map(|n| disc.res(n).expect("a free11 archive")).collect();
    let res = &archives[0];
    let toy = Ini::parse_bytes(&res.read_suffix("free11.toy").expect("free11.toy"));
    let scene = Scene::parse(&res.read_suffix("free11.scn").expect("free11.scn")).expect("free11.scn parses");
    let t = &scene.terrain;
    let area = Area::new(F(t.width as f32), F(t.breadth as f32), t.width_scale, t.breadth_scale);
    let (set, said) = PropSet::load(&toy, "Tracks\\Free11\\", &scene.models, &mut |p| res.read(p).ok(), &mut |p| res.read_suffix(p).ok(), Some(area));
    assert!(said.is_empty(), "free11 loads whole: {said:?}");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let terrain = res.read_suffix(&leaf).expect("free11's terrain");
    let (props, _) =
        Props::load(&scene, |want| archives.iter().find_map(|r| r.read_suffix(want).ok()).ok_or_else(|| "no archive of this track has it".to_string()));
    Some(Free09 { set, terrain, scene, props })
}

fn free11_load(kit: &Kit, frame: u32) -> Option<Ee> {
    let state = context(kit, &format!("states/f21-lean-{frame}.eeMemory.bin"));
    let mut ee = Ee::from_ram(&state).map_err(|why| skip(&format!("{why} — research/f21/slots.py ram lean-{frame} writes it"))).ok()?;
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    assert_eq!(ee.u32_at(addr("frame_counter").unwrap()), frame, "the state is not frame {frame}");
    Some(ee)
}

/// Every collection's objects in a free11 state, in the list's order: the port's flat order.
fn free11_objects(ee: &Ee) -> Vec<Vec<u32>> {
    let mut out = Vec::new();
    let mut cell = ee.u32_at(FREE11_LIST + 4);
    while cell != 0 {
        let c = ee.u32_at(cell);
        let (n, at) = (ee.u32_at(c + coll::BODIES), ee.u32_at(c + coll::BODIES + 8));
        out.push((0..n).map(|k| ee.u32_at(at + 4 * k)).collect());
        cell = ee.u32_at(cell + 8);
    }
    out
}

fn free11_filed(ee: &Ee) -> Vec<u32> {
    (0..ee.u32_at(FREE11_NODE + 0x30)).map(|k| ee.u32_at(ee.u32_at(FREE11_NODE + 0x38) + 4 * k)).collect()
}

/// [`seed`] for collections of many bodies: every object of every collection as the state left it.
fn free11_seed(ee: &Ee, set: &mut PropSet) {
    let game = free11_objects(ee);
    assert_eq!(game.len(), set.collections.len(), "the state's collections");
    let mut cell = ee.u32_at(FREE11_LIST + 4);
    let node = free11_filed(ee);
    for (ci, objects) in game.iter().enumerate() {
        let c = ee.u32_at(cell);
        cell = ee.u32_at(cell + 8);
        assert_eq!(objects.len(), set.collections[ci].objects.len(), "collection {ci}'s bodies");
        if ee.u32_at(c + coll::FLAGS) & coll::ACTIVE != 0 {
            set.collections[ci].activate();
            for (k, &o) in objects.iter().enumerate() {
                if ee.u32_at(ee.u32_at(o + obj::BODY)) != 0 {
                    set.collections[ci].enable(k);
                }
            }
            set.file_on(ci);
        }
        for (k, &o) in objects.iter().enumerate() {
            let (b, n, r) = (ee.u32_at(o + obj::BODY), ee.u32_at(o + obj::NODE), ee.u32_at(o + obj::COLLISION));
            let port = &mut set.collections[ci];
            port.bones[k].body = ee_body(ee, b);
            let ob = &mut port.objects[k];
            ob.node = node_at(ee, n);
            ob.countdown = F(ee.f32_at(o + obj::COUNTDOWN));
            ob.fresh = ee.u32_at(o + obj::FRESH) != 0;
            let at = node.iter().position(|&f| f == r);
            assert_eq!(ob.filed, at.is_some(), "collection {ci}'s object {k} filed as the node has it");
            ob.filed_at = at.map_or(0, |k| k as u32 + 1);
            let Some(col) = ob.collision.as_mut() else { continue };
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
        }
    }
}

/// One free11 tick from `frame` the port's way, against `frame + 1`: answers what differs.
fn free11_tick(kit: &Kit, frame: u32) -> Option<Vec<String>> {
    let (before, after) = (free11_load(kit, frame)?, free11_load(kit, frame + 1)?);
    let mut free = free11()?;
    let terrain = Terrain::parse(&free.terrain).expect("free11's terrain");
    let ground = Ground::new(&terrain, &free.scene.terrain);
    let field = ground.field();
    free11_seed(&before, &mut free.set);
    let dt = F(after.f32_at(addr("frame_dt").unwrap()));
    let game = free11_objects(&after);
    let pins: Vec<usize> = (0..game.len()).filter(|&c| game[c].len() == 10).collect();
    assert_eq!(pins.len(), 1, "one collection of ten bodies");
    let pins = pins[0];
    // Every object's collision object, by the port's flat place.
    let flat: Vec<u32> = game.iter().flatten().map(|&o| after.u32_at(o + obj::COLLISION)).collect();
    let mut bad = Vec::new();

    let movers = free.set.search(dt, Some(&field), Some(&free.props), None);
    let order: Vec<u32> = free.set.node_order().iter().map(|&(c, k)| flat[prop_props::flat(&free.set, c, k)]).collect();
    let want: Vec<u32> = free11_filed(&after).into_iter().filter(|r| flat.contains(r)).collect();
    if order != want {
        bad.push(format!("the node's props: port {order:x?} state {want:x?}"));
    }
    let names = |e: &Contact| e.other.and_then(|o| o.checked_sub(PROP_OTHER)).map(|f| flat[f]);
    for (c, objects) in game.iter().enumerate() {
        for (k, &o) in objects.iter().enumerate() {
            let r = after.u32_at(o + obj::COLLISION);
            let Some(col) = free.set.collections[c].objects[k].collision.as_ref() else { continue };
            let want = list_at(&after, r);
            let got = &col.record.contacts;
            if c == pins && MET.contains(&k) {
                eprintln!(
                    "lean-{frame}: pin {k}'s list, port {:?} state {:?}",
                    got.iter().map(|e| (e.kind, names(e).map(|r| format!("{r:#x}")))).collect::<Vec<_>>(),
                    want.iter().map(|(e, o)| (e.kind, format!("{o:#x}"))).collect::<Vec<_>>()
                );
                if got.len() != want.len() {
                    bad.push(format!("pin {k}'s list: port {} entries, state {}", got.len(), want.len()));
                }
                for (j, (p, (w, other))) in got.iter().zip(&want).enumerate() {
                    let d = searched_differ(p, w);
                    let named = names(p);
                    if !d.is_empty() || named.is_some() != flat.contains(other) || named.is_some_and(|n| n != *other) {
                        bad.push(format!("pin {k}'s entry {j}: {d:?}, port names {named:x?}, state's other {other:#x}"));
                    }
                }
                let r0 = &col.record;
                let flags = [r0.touching, r0.touching_other, r0.on_ground, r0.on_object];
                let want = [rec::TOUCHING, rec::TOUCHING_OTHER, rec::ON_GROUND, rec::ON_OBJECT].map(|f| after.u32_at(r + f) != 0);
                if flags != want {
                    bad.push(format!("pin {k}'s flags: port {flags:?} state {want:?}"));
                }
            } else {
                let port = got.iter().any(|e| names(e).is_some());
                let state = want.iter().any(|(_, o)| flat.contains(o));
                if port || state {
                    bad.push(format!("collection {c}'s object {k} names a prop: port {port} state {state}"));
                }
            }
        }
    }

    free.set.step(dt, &movers, &mut []);
    for k in MET {
        let pin = &free.set.collections[pins].bones[k].body;
        bad.extend(bodies_differ(&format!("pin {k}"), pin, &ee_body(&after, after.u32_at(game[pins][k] + obj::BODY))));
    }
    Some(bad)
}

/// F21: lean-209 to 214, each tick from its state: free11's pin 1 strikes pin 0 of its own
/// collection (212), pin 0 woken and filed after it with pin 1's mirror at its head, then each on
/// the other in node order (213) — the node's order, both lists and both bodies bit for bit.
#[test]
fn a_pin_strikes_a_pin_of_its_own_collection() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    let mut bad = Vec::new();
    for frame in 209..214 {
        let Some(b) = free11_tick(&kit, frame) else { return };
        bad.extend(b.into_iter().map(|b| format!("lean-{frame}: {b}")));
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// lean-209 run on its own to 214, never put back to a state: both pins' bodies each tick.
#[test]
fn a_pin_on_a_pin_runs_on() {
    let Ok(kit) = Kit::load().map_err(|why| skip(&why)) else { return };
    let Some(first) = free11_load(&kit, 209) else { return };
    let Some(mut free) = free11() else { return };
    let terrain = Terrain::parse(&free.terrain).expect("free11's terrain");
    let ground = Ground::new(&terrain, &free.scene.terrain);
    let field = ground.field();
    free11_seed(&first, &mut free.set);
    let game = free11_objects(&first);
    let pins = (0..game.len()).find(|&c| game[c].len() == 10).expect("the ten pins");
    let mut bad = Vec::new();
    for frame in 210..215 {
        let Some(after) = free11_load(&kit, frame) else { return };
        let dt = F(after.f32_at(addr("frame_dt").unwrap()));
        let movers = free.set.search(dt, Some(&field), Some(&free.props), None);
        free.set.step(dt, &movers, &mut []);
        for k in MET {
            let pin = &free.set.collections[pins].bones[k].body;
            bad.extend(bodies_differ(&format!("lean-{frame} pin {k}"), pin, &ee_body(&after, after.u32_at(game[pins][k] + obj::BODY))));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}
