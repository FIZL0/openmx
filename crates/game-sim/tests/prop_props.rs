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
//! Skipped with a line where the disc, the kit or the states are not there (they are in s3's context
//! only; research/s242/slots.py ram drop2-NNNN writes the RAM images).

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
