//! S239 — a ridden bike's frame under an asleep prop wakes it, the port against the original's run
//! in the interpreter.
//!
//! The original files every bike's frame collidable (category 0x9a048, the ridden bike's and the AI
//! riders' alike) on the props' node, so a prop's ray probe meets them like any other collidable, and
//! its two listeners hear the hit: the hit listener wakes an asleep body (0x9a048 has the wake mask's
//! bit, 0x40), the collection's listener activates the collection. The port tests a prop's rays on
//! the bikes after its scene search ([`PropSet::hear_ridden`]); this is that hit's listeners.
//!
//! The egg on its sculpture (`LargeEgg.col`) is woken and put straight back to sleep where it lies,
//! as in S200's test, so it searches with the wake mask's categories only. One of free09's AI bikes
//! (first in the egg's list) is far away and frame after frame the egg sleeps on. Then the bike is
//! moved beside the egg — its frame's node and its collidable's centre in RAM, its chassis' node in
//! the port's ridden bikes — and on the next frame the egg's rays strike the bike's frame: the egg is
//! woken, its collection activated, and its list holds its rays' entries on the bike. From there the
//! egg falls off the sculpture against the bike's chassis, the original's resolving its entries
//! against the chassis read from RAM each frame. Every frame the body, the node, the record's flags,
//! the probe's category list, the countdown, the collection's active bit and the node's listeners are
//! compared bit for bit, and on the strike frame the egg's list, entry by entry.
//!
//! Skipped with a line where the disc, the kit or the state is not there.


use std::path::PathBuf;

use game_render::disc::{Disc, Res};
use game_sim::biplane::{Area, Body};
use game_sim::camera::CurveTable;
use game_sim::contact::{Collider, Contact, Node, Other, Response, Side, Vehicle};
use game_sim::eco::{self, Ecosystem, Trees};
use game_sim::flight_search;
use game_sim::math::Mat4;
use game_sim::phy::Ini;
use game_sim::prop_set::PropSet;
use game_sim::prop_world::Listener;
use game_sim::track::{Scene, Terrain};
use game_sim::world::{Ground, Props};
use game_sim::world_search::Categories;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

const STATE: &str = "first_recording.p2m2_SaveState.eeMemory.bin";
const BUDGET: u64 = 50_000_000;
/// The `.toy` actor's collection list {count, head}; cells {data, +4, +8 next}.
const LIST: u32 = 0x011b_43f4;
/// The terrain's area, as `free09.scn` lays it.
const FREE09: (f32, f32, f32, f32) = (3.0, 3.0, 9.0, 9.0);
/// Where the egg lies on its sculpture.
const EGG: [f32; 3] = [3244.70, 143.67, 3977.15];

mod coll {
    pub const FLAGS: u32 = 0x20;
    pub const ACTIVE: u32 = 2;
    pub const NAME: u32 = 0x144;
    pub const LISTENER: u32 = 0x34;
    pub const NODE: u32 = 0x248;
    pub const BODIES: u32 = 0x320;
}

mod obj {
    pub const LISTENER: u32 = 0x50;
    pub const DETECT: u32 = 0x6c;
    pub const BODY: u32 = 0x110;
    pub const COLLISION: u32 = 0x120;
    pub const RAY_PROBE: u32 = 0x124;
    pub const FRESH: u32 = 0x1b0;
    pub const COUNTDOWN: u32 = 0x1bc;
}

/// The ray probe's category list.
mod probe {
    pub const MODE: u32 = 0xe8;
    pub const LIST: u32 = 0xec;
    pub const COUNT: u32 = 0xf0;
}

/// A collidable's category, and where it is.
const CATEGORY: u32 = 0x84;
const CENTRE: u32 = 0x40;
/// A bike's frame collidable's copy of its chassis' world transform: the origin.
const BIKE_ORIGIN: u32 = 0x150;

mod rec {
    pub const VTABLE: u32 = 0x08;
    pub const TOUCHING: u32 = 0x40;
    pub const TOUCHING_OTHER: u32 = 0x44;
    pub const ON_GROUND: u32 = 0x48;
    pub const ON_OBJECT: u32 = 0x4c;
    pub const CLEAR: u32 = 0x74;
    pub const COLLIDE: u32 = 0x7c;
}

/// The world node's listener list {count, +8 items}.
const NODE_LISTENERS: u32 = 0x48;

mod body_at {
    pub const ENABLED: u32 = 0x00;
    pub const POSITION: u32 = 0x60;
    pub const VELOCITY: u32 = 0x70;
    pub const QUAT: u32 = 0x170;
    pub const RATES: u32 = 0x1b0;
}

/// The original's functions.
mod f {
    pub const UPDATE: u32 = 0x003b_28a0;
    pub const SETTLE: u32 = 0x003b_2440;
    pub const ENABLE: u32 = 0x0015_b790;
    pub const DISABLE: u32 = 0x0015_b8c0;
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

/// A ray probe's category list as the state has it.
fn ee_categories(ee: &Ee, p: u32) -> Categories {
    let (list, n) = (ee.u32_at(p + probe::LIST), ee.u32_at(p + probe::COUNT));
    Categories { mode: ee.u32_at(p + probe::MODE), list: (0..n).map(|i| ee.u32_at(list + 4 * i)).collect() }
}

fn port_categories(set: &PropSet, c: usize, k: usize) -> Categories {
    set.collections[c].objects[k].collision.as_ref().expect("a collision object").categories.clone()
}

/// free09 off the disc: the props as the ride loads them, the terrain's bytes, the scene's
/// collision meshes and the ecosystem's plants (a woken egg flies into a tree's trunk).
struct Free09 {
    set: PropSet,
    terrain: Vec<u8>,
    scene: Scene,
    props: Props,
    trees: Trees,
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
    let (props, _) = Props::load(&scene, |want| archives.iter().find_map(|r| r.read_suffix(want).ok()).ok_or_else(|| "no archive of this track has it".to_string()));
    let esb = Ecosystem::parse(&res.read_suffix("free09.esb").expect("free09.esb")).expect("free09.esb parses");
    let (trees, missed) = Trees::new(&esb, eco::cell_size(F(6912.0)), &CurveTable::build());
    assert!(missed.is_empty(), "{missed:?}");
    Some(Free09 { set, terrain, scene, props, trees })
}


/// The node's listeners that are this collection's (`c`) and its object's (`o`), in the node's order.
fn ee_listeners(ee: &Ee, node: u32, c: u32, o: u32) -> Vec<Listener> {
    let n = ee.u32_at(node + NODE_LISTENERS);
    let items = ee.u32_at(node + NODE_LISTENERS + 8);
    (0..n)
        .map(|i| ee.u32_at(items + 4 * i))
        .filter_map(|l| {
            if l == c + coll::LISTENER {
                Some(Listener::Settle)
            } else if l == o + obj::LISTENER {
                Some(Listener::Object(0))
            } else {
                None
            }
        })
        .collect()
}

/// The node's listeners run, as its update runs them: each in order, taken off when it says so.
fn run_listeners(ee: &mut Ee, node: u32, c: u32, o: u32, dt: f32) {
    let mut i = 0;
    while i < ee.u32_at(node + NODE_LISTENERS) {
        let items = ee.u32_at(node + NODE_LISTENERS + 8);
        let l = ee.u32_at(items + 4 * i);
        let call = if l == c + coll::LISTENER {
            Some((f::SETTLE, c))
        } else if l == o + obj::LISTENER {
            Some((f::LISTEN, o))
        } else {
            None
        };
        let done = call.is_some_and(|(fun, this)| {
            ee.call(fun, &[u64::from(this)], &[dt], BUDGET);
            ee.v0() as u32 != 0
        });
        if done {
            let n = ee.u32_at(node + NODE_LISTENERS);
            for j in i..n - 1 {
                let next = ee.u32_at(items + 4 * (j + 1));
                ee.set_u32(items + 4 * j, next);
            }
            ee.set_u32(node + NODE_LISTENERS, n - 1);
        } else {
            i += 1;
        }
    }
}

fn node_rows(n: &Node) -> [[u32; 4]; 4] {
    [n.rows[0].to_bits(), n.rows[1].to_bits(), n.rows[2].to_bits(), n.origin.to_bits()]
}

fn ee_rows(ee: &Ee, node: u32) -> [[u32; 4]; 4] {
    [0, 1, 2, 3].map(|k| quad(ee, node + 0x80 + 16 * k).to_bits())
}

fn same_body(at: &str, got: &Body, ee: &Ee, b: u32) {
    assert_eq!(got.enabled, ee.u32_at(b + body_at::ENABLED) != 0, "{at}: enabled");
    for (name, g, a) in [
        ("position", got.position, body_at::POSITION),
        ("velocity", got.velocity, body_at::VELOCITY),
        ("quat", got.quat, body_at::QUAT),
        ("rates", got.rates, body_at::RATES),
    ] {
        let w = quad(ee, b + a);
        assert_eq!(g.to_bits(), w.to_bits(), "{at}: {name}: the game {:?}, the port {:?}", w.0, g.0);
    }
}



/// A collision record's list (`+0x100` count, `+0x108` entries, `0x80` bytes each), the other side
/// only as there or not (99).
fn list_at(ee: &Ee, r: u32) -> Vec<Contact> {
    let (n, at) = (ee.u32_at(r + 0x100), ee.u32_at(r + 0x108));
    (0..n)
        .map(|k| {
            let e = at + 0x80 * k;
            Contact {
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
            }
        })
        .collect()
}

fn node_at(ee: &Ee, n: u32) -> Node {
    Node { rows: [0, 1, 2].map(|k| quad(ee, n + 0x80 + 16 * k)), origin: quad(ee, n + 0xb0) }
}

/// A bike's frame record's chassis as the other side of a contact (`+0xf0` the chassis, `+0xf8` its
/// node).
fn vehicle_at(ee: &Ee, rec: u32) -> Vehicle {
    let v = ee.u32_at(rec + 0xf0);
    Vehicle {
        node: node_at(ee, ee.u32_at(rec + 0xf8)),
        cofg: quad(ee, v + 0x190),
        mass: F(ee.f32_at(v + 0x210)),
        inv_mass: F(ee.f32_at(v + 0x214)),
        inv_inertia: [0xf0, 0x104, 0x118].map(|at| F(ee.f32_at(v + at))),
        velocity: quad(ee, v + 0x260),
        rates: quad(ee, v + 0x290),
        shock: quad(ee, v + 0x170),
        twist: quad(ee, v + 0x180),
    }
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

/// A collision record's tuning, as its resolve reads it.
fn collider_at(ee: &Ee, r: u32) -> Collider {
    let c = ee.u32_at(r + 0x1f8);
    Collider {
        damp: ee.u32_at(r + 0x50) != 0,
        damp_linear: F(ee.f32_at(r + 0x58)),
        damp_angular: F(ee.f32_at(r + 0x60)),
        mass: F(ee.f32_at(r + 0x64)),
        inv_mass: F(ee.f32_at(r + 0x68)),
        inv_inertia: quad(ee, r + 0x80),
        cofg: quad(ee, r + 0x90),
        cofg_world: quad(ee, r + 0xa0),
        actors: ee.u32_at(r + 0x110),
        collides_with: ee.u32_at(r + 0x114),
        collidable: (c != 0).then(|| ee.u32_at(c + CATEGORY)),
        max_depth: F(ee.f32_at(r + 0x118)),
        scenery: response(ee, r, 0x124, 0x148, 0x150, 0x160, 0x190),
        actor: response(ee, r, 0x13c, 0x14c, 0x170, 0x180, 0x194),
        surfaces: core::array::from_fn(|k| F(ee.f32_at(r + 0x198 + 4 * k as u32))),
        normal_speed: F(ee.f32_at(r + 0x1d8)),
        relative: quad(ee, r + 0x1e0),
    }
}

/// `bikef.col`, the bikes' frame collidable, out of the first archive on the disc that has it.
fn bike_frame() -> Option<game_sim::track::Mesh> {
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let names: Vec<String> = disc.entries().iter().filter(|e| e.name.to_ascii_lowercase().ends_with(".res")).map(|e| e.name.clone()).collect();
    let bytes = names.iter().find_map(|n| disc.res(n).ok()?.read(flight_search::BIKE_COLLISION).ok())?;
    Some(game_sim::track::Mesh::parse(&bytes).expect("bikef.col parses"))
}

/// The search fields of two entries that differ, by name.
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

/// The egg asleep on its sculpture, an AI bike beside it when it was put to sleep (its list gathered
/// with the bike near): its rays strike the bike's frame, whose category has the wake mask's bit, and
/// the hit listener wakes it.
#[test]
fn an_ai_bike_beside_an_asleep_egg_wakes_it_as_the_originals() {
    ride(true);
}

/// The egg asleep on its sculpture, an AI bike moved beside it at frame 31, after its list was
/// gathered: the list keeps the bike at the distance it was gathered at, beyond the probe's reach, so
/// that frame's search does not test it; the asleep check then gathers the list again with the bike
/// near, and the next frame's search strikes it — in the original as in the port.
#[test]
fn an_ai_bike_moved_beside_an_asleep_egg_wakes_it_a_gather_later() {
    ride(false);
}

/// Rides the egg asleep for 120 frames beside an AI bike, moved beside it before its asleep list's
/// gather (`at_gather`) or after it, at frame 31, and compares it with the original's each frame.
fn ride(at_gather: bool) {
    let Some(mut ee) = image() else { return };
    let Some(mut free) = free09() else { return };
    let Some(frame) = bike_frame() else { return };
    let terrain = Terrain::parse(&free.terrain).expect("free09's terrain");
    let ground = Ground::new(&terrain, &free.scene.terrain);
    let field = ground.field();
    let game = collections(&ee);
    let pos = |ee: &Ee, c: u32| {
        let o = ee.u32_at(ee.u32_at(c + coll::BODIES + 8));
        quad(ee, ee.u32_at(o + obj::BODY) + body_at::POSITION)
    };
    let dist = |v: Vf, to: [f32; 3]| (0..3).map(|k| (v[k] - to[k]).powi(2)).sum::<f32>().sqrt();
    let (ci, &c) = game.iter().enumerate().min_by(|a, b| dist(pos(&ee, *a.1), EGG).total_cmp(&dist(pos(&ee, *b.1), EGG))).unwrap();
    let name = text(&ee, c + coll::NAME);
    assert!(dist(pos(&ee, c), EGG) < 1.0, "{name} is not the egg");
    let o = ee.u32_at(ee.u32_at(c + coll::BODIES + 8));
    let (b, r, p) = (ee.u32_at(o + obj::BODY), ee.u32_at(o + obj::COLLISION), ee.u32_at(o + obj::RAY_PROBE));
    let node = ee.u32_at(c + coll::NODE);
    let model = ee.u32_at(o + 0x13c);
    assert_ne!(ee.u32_at(o + obj::DETECT), 0, "{name}: DetectCollisionsWhenDisabled");

    // Woken and put to sleep where it lies, as in S200's test.
    ee.call(f::ENABLE, &[u64::from(o)], &[], BUDGET);
    ee.call(f::DISABLE, &[u64::from(o)], &[], BUDGET);
    free.set.collections[ci].enable(0);
    free.set.collections[ci].disable(0);
    assert_eq!(port_categories(&free.set, ci, 0), ee_categories(&ee, p), "{name}: asleep");

    // The first AI bike in the egg's list (the woken egg gathered it): its frame collidable, its
    // record and the record's node.
    let search = ee.u32_at(r + 0x10c);
    let (list, n) = (ee.u32_at(search), ee.u32_at(search + 4));
    let bike = (0..n)
        .map(|i| ee.u32_at(list + 8 * i))
        .find(|&col| ee.u32_at(col + CATEGORY) == flight_search::BIKE && ee.u8_at(col + 0x116) == 0x14)
        .expect("an AI bike in the egg's list");
    let bike_rec = ee.u32_at(bike + 0x80);
    let bike_node = ee.u32_at(bike_rec + 0xf8);
    assert_eq!(ee.u32_at(bike_rec + 0x1f8), bike, "the bike's frame record holds its collidable");
    let far = node_at(&ee, bike_node);
    assert_eq!(quad(&ee, bike + BIKE_ORIGIN).to_bits(), far.origin.to_bits(), "the bike's collidable's transform is its frame's node's");
    let bike_at = |n: &Node| Mat4([n.rows[0], n.rows[1], n.rows[2], n.origin]);
    let ridden = |n: &Node| [flight_search::Bike { transform: bike_at(n), in_world: true }];
    free.set.bike_frame = Some(std::sync::Arc::new(frame));
    free.set.file_ridden(true);

    // Where beside the egg its rays strike the bike's frame: the first of a ring of spots round the
    // egg's centre, the bike's frame's centre moved there.
    let centre = quad(&ee, bike + CENTRE);
    let egg_node = free.set.collections[ci].objects[0].node.clone();
    let rays = free.set.rays[ci][0].clone().expect("the egg's rays");
    let categories = port_categories(&free.set, ci, 0);
    let moved = |to: Vf| {
        let d = Vf::new(to[0] - centre[0], to[1] - centre[1], to[2] - centre[2], 0.0);
        let mut n = far.clone();
        n.origin = Vf::new(far.origin[0] + d[0], far.origin[1] + d[1], far.origin[2] + d[2], 1.0);
        (n, d)
    };
    let mesh = free.set.bike_frame.clone().unwrap();
    let (near, d) = (0..64)
        .find_map(|i| {
            let (ring, a) = (1.0 + (i / 16) as f32, (i % 16) as f32 * std::f32::consts::TAU / 16.0);
            let e = egg_node.origin;
            let to = Vf::new(e[0] + ring * a.cos(), e[1] + 1.0, e[2] + ring * a.sin(), 1.0);
            let (n, d) = moved(to);
            let hit = !flight_search::bikes_probed(&rays, &egg_node, &categories, flight_search::Bikes { mesh: &mesh, bikes: &ridden(&n) }).is_empty();
            hit.then_some((n, d))
        })
        .expect("a spot beside the egg where its rays meet the bike's frame");

    let vt = ee.u32_at(r + rec::VTABLE);
    let (clear, collide) = (ee.u32_at(vt + rec::CLEAR), ee.u32_at(vt + rec::COLLIDE));
    let dt = 1.0f32 / 30.0;
    let mut bike_place = far.clone();
    if at_gather {
        // The bike beside the egg, and the egg woken and put to sleep again: the wake gathers its
        // list with the bike near.
        for (k, w) in [0u32, 4, 8].into_iter().enumerate() {
            ee.set_u32(bike_node + 0xb0 + w, near.origin[k].to_bits());
            ee.set_u32(bike + CENTRE + w, (centre[k] + d[k]).to_bits());
            ee.set_u32(bike + BIKE_ORIGIN + w, near.origin[k].to_bits());
        }
        bike_place = near.clone();
        ee.call(f::ENABLE, &[u64::from(o)], &[], BUDGET);
        ee.call(f::DISABLE, &[u64::from(o)], &[], BUDGET);
        free.set.hear_ridden(&ridden(&bike_place));
        free.set.collections[ci].enable(0);
        free.set.collections[ci].disable(0);
    }
    let mut woken_at = None;
    for frame in 1..=120 {
        if frame == 31 && !at_gather {
            // The bike beside the egg: its frame's node and its collidable's centre.
            for (k, w) in [0u32, 4, 8].into_iter().enumerate() {
                ee.set_u32(bike_node + 0xb0 + w, near.origin[k].to_bits());
                ee.set_u32(bike + CENTRE + w, (centre[k] + d[k]).to_bits());
                ee.set_u32(bike + BIKE_ORIGIN + w, near.origin[k].to_bits());
            }
            bike_place = near.clone();
        }
        let at = format!("{name}, frame {frame}");
        free.set.hear_ridden(&ridden(&bike_place));
        ee.call(clear, &[u64::from(r)], &[], BUDGET);
        ee.call(collide, &[u64::from(r)], &[dt], BUDGET);
        let movers = free.set.search(F(dt), Some(&field), Some(&free.props), Some(&free.trees));
        {
            // The egg's search: what its rays struck, entry by entry.
            let want = list_at(&ee, r);
            let got = &free.set.collections[ci].objects[0].collision.as_ref().unwrap().record.contacts;
            assert_eq!(got.len(), want.len(), "{at}: the egg's list's length");
            for (k, (g, w)) in got.iter().zip(&want).enumerate() {
                assert!(searched_differ(g, w).is_empty(), "{at}: entry {k}: {:?}\nthe port {g:?}\nthe game {w:?}", searched_differ(g, w));
            }
        }
        ee.call(f::UPDATE, &[u64::from(c)], &[dt], BUDGET);
        run_listeners(&mut ee, node, c, o, dt);
        let mut collider = collider_at(&ee, bike_rec);
        let mut vehicle = vehicle_at(&ee, bike_rec);
        let mut strikers = [Other { collider: &mut collider, side: Side::Vehicle(&mut vehicle) }];
        free.set.step(F(dt), &movers, &mut strikers);

        let port = &free.set.collections[ci];
        let ob = &port.objects[0];
        let col = ob.collision.as_ref().unwrap();
        same_body(&at, &port.bones[0].body, &ee, b);
        assert_eq!(node_rows(&ob.node), ee_rows(&ee, model), "{at}: the node");
        let flag = |a: u32| ee.u32_at(r + a) != 0;
        let rc = &col.record;
        assert_eq!(
            [rc.touching, rc.touching_other, rc.on_ground, rc.on_object],
            [flag(rec::TOUCHING), flag(rec::TOUCHING_OTHER), flag(rec::ON_GROUND), flag(rec::ON_OBJECT)],
            "{at}: touching, touching another, on the ground, on an object"
        );
        assert_eq!(col.categories, ee_categories(&ee, p), "{at}: the probe's category list");
        assert_eq!(ob.countdown.to_bits(), ee.u32_at(o + obj::COUNTDOWN), "{at}: the countdown");
        assert_eq!(ob.fresh, ee.u32_at(o + obj::FRESH) != 0, "{at}: fresh");
        assert_eq!(port.active, ee.u32_at(c + coll::FLAGS) & coll::ACTIVE != 0, "{at}: active");
        assert_eq!(port.listeners, ee_listeners(&ee, node, c, o), "{at}: the node's listeners");
        let awake = port.bones[0].body.enabled;
        if frame <= 30 && !at_gather {
            assert!(!awake && !port.active, "{at}: the bike far off, it sleeps on");
        }
        if awake && woken_at.is_none() {
            woken_at = Some(frame);
        }
    }
    let ob = &free.set.collections[ci].objects[0];
    eprintln!("{name}: woken by the bike at frame {woken_at:?}; at the end awake {}, at {:?}", free.set.collections[ci].bones[0].body.enabled, free.set.collections[ci].bones[0].body.position.0);
    if at_gather {
        assert_eq!(woken_at, Some(1), "the bike's frame wakes the egg on its first search");
    } else {
        assert_eq!(woken_at, Some(32), "the bike wakes the egg the frame after its list gathers it near");
    }
    assert!(ob.filed, "{name}: still filed");
}
