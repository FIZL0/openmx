//! F34 — Nat05's prop world for SLUS-21104's bike, proved against state 4.
//!
//! State 4 (`context/mxvatv-ps2/state04.ee.bin`) is a Free Ride on Nat05 with the bike at its spawn.
//! It holds the loaded world's quadtree, every collidable in it, the bike's nearby list (bike `+0xc4`)
//! and its gather probe (bike `+0xd8`), and the bike's three ray probes: the two trigger rays (bike
//! `+0x1430`), the wreck part and the riding part. The port builds the world from the disc's files
//! (`Env\nat05\Nat05.scn`, its `Col\*.col` and `Env\nat05\Nat05.toy` out of `nat05.res`, and
//! `Env\Nat05\nat05.esb` out of `MXVSATV.PSR`, both copied into `context/mxvatv-ps2/nat05/`) with
//! [`game_sim::atv_world`], as MD76a1 does Desert Oasis's.
//!
//! Without the image or the files the tests skip (and say so).

//! The tests:
//!
//! - the world's tree is the image's node for node, each collidable's fields too, the zones' shapes
//!   as the probes' pair reads them;
//! - the bike's probes are the image's (their rays, trees and records);
//! - each of the bike's three ray probes, stood at frames that put a ray across one of Nat05's meshes
//!   (six a mesh; Nat05's plants have no trunks to aim at), gathers the list the original gathers and
//!   searches it to the record the original's search leaves, word for word, and to its hit bit (a
//!   plant's sphere or a zone struck alone sets it with no contact);
//! - each species' sphere part, built from the esb, is the image's (its point, frame, radius and
//!   radius squared), on the eco's unit, and no plant in the image is hidden or turned.
//!
//! What the ride itself filed (the vehicles' spheres, the riders' and machines' parts in the moving
//! list) is copied from the image and retired on both sides, so the searches see the world alone; a
//! zone's callback (which names what struck it in its lists) is the ride's, so before each search the
//! port's lists are made the image's again.

use game_sim::atv_props::{self, PropWorld, Rays};
use game_sim::atv_search::{Plants, Wheels};
use game_sim::atv_world::{Nearby, PROBE, World};
use game_sim::broad_build;
use game_sim::broad_phase::Structure;
use game_sim::flight_search;
use game_sim::ray_probe::{self, Matrix, Record};
use game_sim::world::Prop;
use game_sim::world_search::{Categories, Class, Collidable, Objects};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

/// SLUS-21104's `$gp`, which the calls need (crt0 sets it at `0x001001ac`).
const GP: u64 = 0x003d_baf0;
/// The gather, the search, a ray probe's "stand at this frame" (its vtable's `+0xcc`).
const GATHER: u32 = 0x001d_6a60;
const SEARCH_FN: u32 = 0x0011_51c0;
const STAND: u32 = 0x001b_cfe0;
/// The world's quadtree (the global at `0x0040c198` points at it), and the ecosystem object.
const TREE_PTR: u32 = 0x0040_c198;
const ECO_AT: u32 = 0x0052_cd88;
/// The bike, its nearby list and gather probe.
const BIKE: u32 = 0x00e7_14e0;
const LIST_OF: u32 = 0xc4;
const PROBE_OF: u32 = 0xd8;
/// The bike's ray probes (bike-relative pointers): the two trigger rays, the wreck part, the riding
/// part (the gather probe itself); and how many contacts each one's record keeps a ray.
const PROBES: [(u32, &str, usize); 3] = [(0x1430, "trigger", 3), (0x1434, "wreck", 1), (0x1438, "riding", 1)];
/// The scene mesh class's vtable, the vehicle sphere's, the ray probe's.
const MESH_VTABLE: u32 = 0x003b_6d00;
const SPHERE_VTABLE: u32 = 0x003b_6b40;
const RAY_VTABLE: u32 = 0x003b_6c20;
/// The six vehicles whose spheres (`+0xd80`) the tree holds, the bike first.
const VEHICLES: [u32; 6] = [BIKE, 0x00ec_24c0, 0x00f0_4b90, 0x00f4_6560, 0x00f8_8de0, 0x00fc_b720];
const SPHERE: u32 = 0xd80;
/// What the ride filed in the tree besides: two riders' spheres (at the origin), and two parked
/// machines' frame mesh and probe.
const RIDE_PARTS: [u32; 6] = [0x01da_1be0, 0x01da_4a70, 0x01da_0e40, 0x01d9_8520, 0x01da_3cf0, 0x01d9_9290];

/// A collidable's fields (the entry the tree and the list hold is at `+0x18`).
mod at {
    pub const ENTRY: u32 = 0x18;
    pub const VTABLE: u32 = 0xc;
    pub const NEARBY: u32 = 0x45;
    pub const CENTRE: u32 = 0x50;
    pub const RADIUS: u32 = 0x70;
    pub const KIND: u32 = 0x78;
    pub const RECORD: u32 = 0x7c;
    pub const CATEGORY: u32 = 0x94;
    pub const COLLIDES_WITH: u32 = 0xdc;
    pub const OBJECTS: u32 = 0xe0;
    pub const MORE_OBJECTS: u32 = 0xec;
    pub const CATEGORIES: u32 = 0xf8;
    pub const FLAGS: u32 = 0x126;
    pub const TRANSFORM: u32 = 0x130;
    pub const ROOT: u32 = 0x170;
    pub const RAYS: u32 = 0x178;
    pub const NODES: u32 = 0x17c;
    pub const SEGMENTS: u32 = 0x180;
    pub const SMOOTH: u32 = 0x184;
    pub const FILTER: u32 = 0x188;
}

/// A ray probe's record.
mod rec {
    pub const FROM: u32 = 0x0;
    pub const TO: u32 = 0x10;
    pub const NORMAL: u32 = 0x20;
    pub const POINT: u32 = 0x30;
    pub const PUSH: u32 = 0x40;
    pub const LOW: u32 = 0x50;
    pub const HIGH: u32 = 0x60;
    pub const FIRST_POINT: u32 = 0x70;
    pub const FIRST: u32 = 0x80;
    pub const CAPACITY: u32 = 0x88;
    pub const COUNT: u32 = 0x8c;
    pub const RAYS: u32 = 0x90;
    pub const NORMALS: u32 = 0x94;
    pub const POINTS: u32 = 0x98;
    pub const OBJECTS: u32 = 0x9c;
    pub const PLACED: u32 = 0xa0;
    pub const FRACTIONS: u32 = 0xa8;
    pub const ZEROED: u32 = 0xac;
    pub const SEGMENTS: u32 = 0xb0;
    pub const NEAREST: u32 = 0xb4;
    pub const FLAGS: u32 = 0xb8;
    pub const ONE_PER_RAY: u32 = 0xbc;
}

/// What a tree entry or a candidate is, the same on both sides: a collidable by its centre's and
/// reach's bits, a plant by its cell, model and blend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Id {
    Object([u32; 4]),
    Plant(u16, u16, u8, i8),
}

/// What a record's contact struck, the same on both sides: a mesh by its centre's and reach's bits,
/// a species' part by its model (loaded order) and index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Struck {
    Mesh([u32; 4]),
    Part(usize, usize),
    Other(u32),
}

/// One of the bike's ray probes on both sides.
struct Probe {
    at: u32,
    name: &'static str,
    object: usize,
    rays: flight_search::Rays,
    per_ray: usize,
}

struct Data {
    ee: Ee,
    tree: u32,
    list: u32,
    gather: u32,
    pw: PropWorld,
    /// The image's collidables and the port's they are.
    port: BTreeMap<u32, usize>,
    probes: Vec<Probe>,
    /// Each species' first part collidable, and the image's part collidables.
    slots: Vec<usize>,
    parts: Vec<Vec<u32>>,
    /// The collidables taken from the image whose first word is 1.
    retired: BTreeSet<usize>,
    /// The syscalls the calls made: number and caller.
    syscalls: Rc<RefCell<BTreeSet<(u32, u32)>>>,
}

fn data() -> Option<Data> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let image = kit.ctx("mxvatv-ps2").join(format!("state04.ee{}", ".bin"));
    let dir = kit.ctx("mxvatv-ps2/nat05");
    let res = dir.join("nat05.res");
    let esb = dir.join("nat05.esb");
    for (p, what) in [
        (&image, "state 4's RAM image (F31's recipe: research/journal)"),
        (&res, "Nat05's nat05.res off MX vs. ATV's disc (Env/Nat05)"),
        (&esb, "Env\\Nat05\\nat05.esb out of MXVSATV.PSR on MX vs. ATV's disc"),
    ] {
        if !p.exists() {
            skip(&format!("{} is not there: {what}", p.display()));
            return None;
        }
    }
    let mut ee = Ee::from_ram(&image).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, GP);
    let syscalls = Rc::new(RefCell::new(BTreeSet::new()));
    // Every syscall stub answered (the search's listeners lock and unlock): each one's number and
    // its caller logged.
    for a in (0x0030_0000..0x0036_0000u32).step_by(4) {
        if ee.u32_at(a) >> 16 == 0x2403 && ee.u32_at(a + 4) == 0x0000_000c && ee.u32_at(a + 8) == 0x03e0_0008 {
            let n = ee.u32_at(a) & 0xffff;
            let log = syscalls.clone();
            ee.hooks.insert(a, Box::new(move |ee: &mut Ee| {
                log.borrow_mut().insert((n, ee.r(31) as u32));
                let v = if n == 0x2f { 1 } else { ee.r(4) };
                ee.set(2, v);
            }));
        }
    }
    let tree = ee.u32_at(TREE_PTR);
    let (list, gather) = (ee.u32_at(BIKE + LIST_OF), ee.u32_at(BIKE + PROBE_OF));
    let mut pw = atv_props::build(&std::fs::read(&res).unwrap(), &std::fs::read(&esb).unwrap(), "nat05", collidable(&ee, gather))
        .unwrap_or_else(|e| panic!("{e}"));
    let hi = atv_props_hi(&pw);
    let w = &mut pw.world;
    let mut port = BTreeMap::from([(gather, PROBE)]);
    // Two riders' spheres at the origin, then two parked machines' frame mesh and probe: the ride's,
    // not the world's, so taken from the image.
    for c in RIDE_PARTS {
        let o = w.add(collidable(&ee, c), centre(&ee, c));
        let r = F(ee.f32_at(c + at::RADIUS));
        let Structure::Tree(t) = &mut w.broad.structures[0] else { unreachable!() };
        let key = broad_build::key(t, hi, centre(&ee, c), [r, r, r, F::ONE]);
        broad_build::insert(t, o, key);
        port.insert(c, o);
    }
    // The vehicles' spheres: the bike's first (bike `+0xd80`), then five more no machine rides.
    let vehicles = w.add_vehicles(VEHICLES.len(), hi);
    port.extend(VEHICLES.iter().map(|v| ee.u32_at(v + SPHERE)).zip(vehicles.clone()));
    let player = vehicles.start;
    w.move_vehicle(player, [0, 4, 8].map(|i| F(ee.f32_at(BIKE + 0xd90 + i))), F(game_sim::atv_world::VEHICLE_REACH));
    w.broad.places[player].centre = centre(&ee, ee.u32_at(BIKE + SPHERE));
    w.broad.places[PROBE].centre = centre(&ee, gather);
    // The space's moving list, slot for slot: the riders' and machines' parts, the ride's (from the
    // image too).
    let moving = moving_list(&ee, gather);
    let block = ee.u32_at(ee.u32_at(moving + 0x34));
    for slot in 0..ee.u32_at(moving + 0x30) {
        let e = ee.u32_at(block + 4 * slot);
        if e & 2 != 0 {
            continue;
        }
        let c = e - at::ENTRY;
        let o = match port.get(&c) {
            Some(&o) => o,
            None => w.add(collidable(&ee, c), centre(&ee, c)),
        };
        port.insert(c, o);
        w.put_moving(o, slot as usize);
    }
    // The bike's probes: the riding part is the gather probe; the other two are in no structure.
    let mut probes = Vec::new();
    for (off, name, per_ray) in PROBES {
        let c = ee.u32_at(BIKE + off);
        let object = match port.get(&c) {
            Some(&o) => o,
            None => {
                let o = w.add(collidable(&ee, c), centre(&ee, c));
                port.insert(c, o);
                o
            }
        };
        probes.push(Probe { at: c, name, object, rays: image_rays(&ee, c), per_ray });
    }
    // Every other collidable in the tree is the port's at its place: the port's map of the image's.
    let taken: BTreeSet<u32> = port.keys().copied().collect();
    let mut by_id: BTreeMap<Id, Vec<usize>> = BTreeMap::new();
    for o in 0..w.broad.places.len() {
        by_id.entry(port_id(w, o)).or_default().push(o);
    }
    walk_entries(&ee, ee.u32_at(tree + 0x8c), &mut |e| {
        if ee.u8_at(e + 2) == 2 && !port.contains_key(&(e - at::ENTRY)) {
            let os = &by_id[&image_id(&ee, e).unwrap()];
            assert_eq!(os.len(), 1, "one port collidable at {:?}", image_id(&ee, e));
            port.insert(e - at::ENTRY, os[0]);
        }
    });
    // Every collidable taken from the image names in its lists what the image's does; the world's own
    // name none (the world test asserts the image's agree), until a zone's callback fills them.
    for &c in &taken {
        let o = port[&c];
        let (objects, more) = (lists(&ee, c + at::OBJECTS, &port), lists(&ee, c + at::MORE_OBJECTS, &port));
        w.objects[o].objects = objects;
        w.objects[o].more_objects = more;
    }
    let first_part = w.objects.len();
    w.objects.extend(game_sim::atv_search::part_collidables(&pw.eco));
    let slots = Plants::new(&pw.trees, &pw.eco, &pw.world, first_part).slots;
    let parts = image_parts(&ee, pw.eco.species.iter().flatten().count() as u32);
    // The ride's own collidables (the machines' meshes and the riders' spheres, the vehicles'
    // spheres) have no shape in the port's world: retired on both sides (first word 1), so the
    // searches see the world alone. The bike's probes stay.
    let mine: BTreeSet<u32> = probes.iter().map(|p| p.at).collect();
    for &c in taken.iter().filter(|c| !mine.contains(c)) {
        ee.set_u32(c, 1);
    }
    let retired = port.iter().filter(|&(&c, _)| ee.u32_at(c) == 1).map(|(_, &o)| o).collect();
    Some(Data { ee, tree, list, gather, pw, port, probes, slots, parts, retired, syscalls })
}

/// The space's moving list: the gather probe's space's tables (`+0x20`, `+4`), their `+0x2c` (the
/// tree is their `+0x28`).
fn moving_list(ee: &Ee, gather: u32) -> u32 {
    let tables = ee.u32_at(ee.u32_at(gather + 0x20) + 4);
    assert_eq!(ee.u32_at(tables + 0x28), ee.u32_at(TREE_PTR), "the space's fixed structure is the world's tree");
    ee.u32_at(tables + 0x2c)
}

/// The terrain box's high corner, which the ride files its collidables against.
fn atv_props_hi(pw: &PropWorld) -> [F; 2] {
    game_sim::atv_world::terrain_box(&pw.scene.terrain).1
}

/// A collidable's centre (`+0x50`).
fn centre(ee: &Ee, c: u32) -> [F; 4] {
    [0, 4, 8, 12].map(|i| F(ee.f32_at(c + at::CENTRE + i)))
}

fn quad_at(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

fn bits(v: Vf) -> [u32; 4] {
    v.0.map(f32::to_bits)
}

fn words(ee: &Ee, a: u32) -> [u32; 4] {
    [0, 4, 8, 12].map(|o| ee.u32_at(a + o))
}

/// One of the image's lists of collidables (mode, array, count), as the port's: a collidable the
/// port has not built (one of the bike's own) stands for a number past the world's end.
fn lists(ee: &Ee, l: u32, port: &BTreeMap<u32, usize>) -> Objects {
    let (mode, at, n) = (ee.u32_at(l), ee.u32_at(l + 4), ee.u32_at(l + 8));
    let list = (0..n)
        .map(|i| {
            let p = ee.u32_at(at + 4 * i);
            port.get(&p).copied().unwrap_or(usize::MAX - p as usize)
        })
        .collect();
    Objects { mode, list }
}

fn categories(ee: &Ee, c: u32) -> Categories {
    let c = c + at::CATEGORIES;
    Categories { mode: ee.u32_at(c), list: (0..ee.u32_at(c + 8)).map(|i| ee.u32_at(ee.u32_at(c + 4) + 4 * i)).collect() }
}

/// The image's collidable `c` as the port's: its reach, flags and filter, its object lists empty
/// until every one they name is built.
fn collidable(ee: &Ee, c: u32) -> Collidable {
    Collidable {
        class: Class::Object,
        flags: ee.u8_at(c + at::FLAGS) & 0x0f,
        nearby: ee.u8_at(c + at::NEARBY) & 1 != 0,
        radius: F(ee.f32_at(c + at::RADIUS)),
        category: ee.u32_at(c + at::CATEGORY),
        collides_with: ee.u32_at(c + at::COLLIDES_WITH),
        categories: categories(ee, c),
        ..Collidable::default()
    }
}

/// A ray probe's rays and its tree as the image holds them (`n` rays, `n - 1` nodes).
fn image_rays(ee: &Ee, p: u32) -> flight_search::Rays {
    let (n, segs, nodes) = (ee.u32_at(p + at::RAYS), ee.u32_at(p + at::SEGMENTS), ee.u32_at(p + at::NODES));
    let f = |a: u32| F(ee.f32_at(a));
    flight_search::Rays {
        root: ee.u16_at(p + at::ROOT),
        nodes: (0..n.saturating_sub(1))
            .map(|k| {
                let a = nodes + 0x24 * k;
                game_sim::track::Node {
                    volume: f(a),
                    centre: [f(a + 12), f(a + 16), f(a + 20)],
                    half_extent: [f(a + 24), f(a + 28), f(a + 32)],
                    children: [ee.u16_at(a + 8), ee.u16_at(a + 10)],
                }
            })
            .collect(),
        segments: (0..n).map(|k| ray_probe::Segment { from: quad_at(ee, segs + 0x20 * k), to: quad_at(ee, segs + 0x20 * k + 0x10) }).collect(),
    }
}

/// Each species' part collidables, in loaded order.
fn image_parts(ee: &Ee, species: u32) -> Vec<Vec<u32>> {
    let eco = ee.u32_at(ECO_AT);
    let models = ee.u32_at(eco + 0x84);
    (0..species)
        .map(|m| {
            let a = ee.u32_at(models + 4 * m);
            (0..ee.u32_at(a + 0x194)).map(|k| ee.u32_at(ee.u32_at(a + 0x19c) + 4 * k)).collect()
        })
        .collect()
}

/// The image's tree entry `e` as an [`Id`], or `None` for a scene node (class 7).
fn image_id(ee: &Ee, e: u32) -> Option<Id> {
    match ee.u8_at(e + 2) {
        2 => {
            let w = |o: u32| ee.u32_at(e - at::ENTRY + o);
            Some(Id::Object([w(at::CENTRE), w(at::CENTRE + 4), w(at::CENTRE + 8), w(at::RADIUS)]))
        }
        3 => Some(Id::Plant(ee.u16_at(e + 0xc), ee.u16_at(e + 0x10), ee.u8_at(e + 0x12), ee.u8_at(e + 0x15) as i8)),
        _ => None,
    }
}

fn port_id(w: &World, o: usize) -> Id {
    let p = &w.broad.places[o];
    match w.objects[o].class {
        Class::Object => Id::Object([p.centre[0].0.to_bits(), p.centre[1].0.to_bits(), p.centre[2].0.to_bits(), w.objects[o].radius.0.to_bits()]),
        _ => Id::Plant(p.cell[0], p.cell[1], p.model, p.blend),
    }
}
/// Every node's entries in order, by the node's path from the root (a child's index per step);
/// an empty slot is `None`. `again` counts the second filings, `skipped` the scene nodes left out.
fn image_nodes(ee: &Ee, node: u32, path: String, out: &mut BTreeMap<String, Vec<Option<Id>>>, skipped: &mut usize, again: &mut usize) {
    let (mut ids, mut seen, mut nodes) = (Vec::new(), Vec::new(), 0);
    let mut chunk = ee.u32_at(node + 4);
    while chunk != 0 {
        for s in 0..4 {
            let e = ee.u32_at(chunk + 4 + 4 * s);
            if e == 0 {
                ids.push(None);
                continue;
            }
            if seen.contains(&e) {
                *again += 1;
            }
            seen.push(e);
            match image_id(ee, e) {
                Some(id) => ids.push(Some(id)),
                None => {
                    assert_eq!(ee.u8_at(e + 2), 7, "only scene nodes are left out");
                    *skipped += 1;
                    nodes += 1;
                }
            }
        }
        chunk = ee.u32_at(chunk);
    }
    if nodes != 0 {
        ids.retain(Option::is_some);
    }
    while ids.last() == Some(&None) {
        ids.pop();
    }
    if !ids.is_empty() {
        out.insert(path.clone(), ids);
    }
    let children = ee.u32_at(node);
    if children != 0 {
        for i in 0..4 {
            let c = ee.u32_at(children + 4 * i);
            if c != 0 {
                image_nodes(ee, c, format!("{path}{i}"), out, skipped, again);
            }
        }
    }
}

fn port_nodes(w: &World, node: usize, path: String, out: &mut BTreeMap<String, Vec<Option<Id>>>) {
    let Structure::Tree(t) = &w.broad.structures[0] else { unreachable!() };
    let n = &t.nodes[node];
    let mut ids: Vec<Option<Id>> = n.chunks.iter().flatten().map(|o| o.map(|o| port_id(w, o))).collect();
    while ids.last() == Some(&None) {
        ids.pop();
    }
    if !ids.is_empty() {
        out.insert(path.clone(), ids);
    }
    for (i, c) in n.children.iter().enumerate() {
        if let Some(c) = c {
            port_nodes(w, *c, format!("{path}{i}"), out);
        }
    }
}

fn walk_entries(ee: &Ee, node: u32, f: &mut dyn FnMut(u32)) {
    let mut chunk = ee.u32_at(node + 4);
    while chunk != 0 {
        for s in 0..4 {
            let e = ee.u32_at(chunk + 4 + 4 * s);
            if e != 0 {
                f(e);
            }
        }
        chunk = ee.u32_at(chunk);
    }
    let children = ee.u32_at(node);
    if children != 0 {
        for i in 0..4 {
            let c = ee.u32_at(children + 4 * i);
            if c != 0 {
                walk_entries(ee, c, f);
            }
        }
    }
}


#[test]
fn nat05s_world_is_state_4s() {
    let Some(d) = data() else { return };
    let (ee, w) = (&d.ee, &d.pw.world);
    let Structure::Tree(t) = &w.broad.structures[0] else { unreachable!() };
    let tree = |a: u32| ee.u32_at(d.tree + a);
    assert_eq!(
        [t.origin[0], t.origin[1], t.extent[0], t.extent[1], t.scale].map(|f| f.0.to_bits()),
        [tree(0x28), tree(0x30), tree(0x40), tree(0x44), tree(0x54)],
        "the tree's box and scale"
    );
    // Node for node, entry for entry. A node holding scene nodes (class 7, the renderer's) is
    // compared without them and without its empty slots.
    let (mut want, mut got, mut skipped, mut again) = (BTreeMap::new(), BTreeMap::new(), 0, 0);
    image_nodes(ee, tree(0x8c), String::new(), &mut want, &mut skipped, &mut again);
    port_nodes(w, t.root.unwrap(), String::new(), &mut got);
    assert_eq!(want.len(), got.len(), "the nodes holding entries");
    let mut entries = 0;
    for (p, ids) in &want {
        let mine = &got[p];
        let mine: Vec<Option<Id>> = if ids.contains(&None) { mine.clone() } else { mine.iter().copied().filter(Option::is_some).collect() };
        assert_eq!(&mine, ids, "node {p:?}");
        entries += ids.iter().flatten().count();
    }
    assert_eq!(again, 5, "the start zones' meshes filed a second time");
    // Every collidable in the tree is one of the port's, its fields the port's.
    let image = d.port.clone();
    let mut kinds = BTreeMap::new();
    walk_entries(ee, tree(0x8c), &mut |e| {
        if ee.u8_at(e + 2) == 2 {
            assert!(image.contains_key(&(e - at::ENTRY)), "{:#x} is one of the port's", e - at::ENTRY);
        }
    });
    let back: BTreeMap<usize, u32> = image.iter().map(|(&c, &o)| (o, c)).collect();
    let mut taken_out = 0;
    for (&c, &o) in &image {
        let what = format!("collidable {c:#x} (port {o})");
        let p = &w.objects[o];
        assert_eq!(p.class, Class::Object, "{what}");
        assert_eq!(ee.u8_at(c + at::FLAGS) & 0x0f, p.flags & 0x0f, "{what}: flags");
        assert_eq!(ee.u8_at(c + at::NEARBY) & 1 != 0, p.nearby, "{what}: nearby");
        assert_eq!(ee.u32_at(c + at::RADIUS), p.radius.0.to_bits(), "{what}: reach");
        assert_eq!(ee.u32_at(c + at::CATEGORY), p.category, "{what}: category");
        assert_eq!(ee.u32_at(c + at::COLLIDES_WITH), p.collides_with, "{what}: collides with");
        assert_eq!(categories(ee, c), p.categories, "{what}: categories");
        for (l, mine) in [(at::OBJECTS, &p.objects), (at::MORE_OBJECTS, &p.more_objects)] {
            let n = ee.u32_at(c + l + 8);
            let named: Vec<u32> = (0..n).map(|i| ee.u32_at(ee.u32_at(c + l + 4) + 4 * i)).collect();
            let mine_named: Vec<u32> = mine.list.iter().map(|o| back.get(o).copied().unwrap_or((usize::MAX - o) as u32)).collect();
            assert_eq!(named, mine_named, "{what}: object list +{l:#x}");
            if n != 0 {
                assert_eq!(ee.u32_at(c + l), mine.mode, "{what}: object list +{l:#x}'s mode");
            }
        }
        assert_eq!(centre(ee, c).map(|f| f.0.to_bits()), w.broad.places[o].centre.map(|f| f.0.to_bits()), "{what}: centre");
        if w.meshes.contains(&o) {
            assert_eq!(ee.u32_at(c + at::VTABLE), MESH_VTABLE, "{what} is a scene mesh");
            taken_out += usize::from(!p.nearby);
        }
        *kinds.entry((ee.u32_at(c + at::VTABLE), p.category)).or_insert(0) += 1;
    }
    // The moving list, slot for slot.
    let Structure::List(l) = &w.broad.structures[1] else { unreachable!() };
    let moving = moving_list(ee, d.gather);
    assert_eq!(l.last as u32, ee.u32_at(moving + 0x30), "the moving list's slots used");
    let block = ee.u32_at(ee.u32_at(moving + 0x34));
    let blocks = l.blocks.as_ref().expect("a block");
    let mut used = 0;
    for slot in 0..l.last as u32 {
        let e = ee.u32_at(block + 4 * slot);
        match blocks[0][slot as usize] {
            None => assert!(e & 2 != 0, "moving slot {slot} is free"),
            Some(o) => {
                assert_eq!(back.get(&o), Some(&(e - at::ENTRY)), "moving slot {slot}");
                used += 1;
            }
        }
    }
    let meshes = (MESH_VTABLE, game_sim::atv_world::MESH);
    assert_eq!(kinds[&meshes], w.meshes.len(), "every scene mesh of the port's is in the image's tree");
    for v in VEHICLES {
        let c = ee.u32_at(v + SPHERE);
        assert!(image.contains_key(&c) && ee.u32_at(c + at::VTABLE) == SPHERE_VTABLE, "vehicle {v:#x}'s sphere");
    }
    assert_eq!(taken_out, 24, "Nat05's AirRace shapes, filed with no nearby bit");
    let zones = d.pw.zones.clone().filter(|o| back.contains_key(o)).count();
    assert_eq!(zones, d.pw.zones.len(), "the toy's zones");
    // Each zone's shape as the probes' pair reads it.
    for (o, z) in d.pw.zones.clone().zip(&d.pw.zone_shapes) {
        let c = back[&o];
        match ee.u32_at(c + at::KIND) {
            5 => {
                let s = atv_props::zone_sphere(z).expect("a sphere");
                let want = (words(&ee, c + at::CENTRE), ee.u32_at(c + 0x180), ee.u32_at(c + 0x184));
                assert_eq!((bits(s.centre), s.radius.0.to_bits(), s.radius2.0.to_bits()), want, "zone {}'s sphere", z.name);
            }
            6 => {
                let b = atv_props::zone_box(z).expect("a box");
                let want = ([0, 1, 2, 3].map(|i| words(&ee, c + at::TRANSFORM + 0x10 * i)), words(&ee, c + 0x170), words(&ee, c + 0x180));
                assert_eq!((b.frame.map(bits), bits(b.centre), bits(b.half)), want, "zone {}'s box", z.name);
            }
            k => panic!("zone {} is kind {k}", z.name),
        }
    }
    println!(
        "Nat05's tree: {} nodes, {entries} entries ({skipped} scene nodes left out, {again} filed again); {} collidables' fields; \
         {taken_out} taken out; {zones} zones; {used} in the moving list; kinds {kinds:x?}",
        want.len(),
        image.len()
    );
}

/// A deterministic generator: inputs only.
struct Gen(u64);

impl Gen {
    fn unit(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    fn signed(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
}

fn rows_of(q: [f32; 4]) -> [[f32; 3]; 3] {
    let [x, y, z, w] = q;
    [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + w * z), 2.0 * (x * z - w * y)],
        [2.0 * (x * y - w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + w * x)],
        [2.0 * (x * z + w * y), 2.0 * (y * z - w * x), 1.0 - 2.0 * (x * x + y * y)],
    ]
}

fn frame_of(r: [[f32; 3]; 3], t: [f32; 3]) -> Matrix {
    [
        Vf::new(r[0][0], r[0][1], r[0][2], 0.0),
        Vf::new(r[1][0], r[1][1], r[1][2], 0.0),
        Vf::new(r[2][0], r[2][1], r[2][2], 0.0),
        Vf::new(t[0], t[1], t[2], 1.0),
    ]
}

fn euler_quat(yaw: f32, pitch: f32, roll: f32) -> [f32; 4] {
    let (sy, cy) = (yaw * 0.5).sin_cos();
    let (sp, cp) = (pitch * 0.5).sin_cos();
    let (sr, cr) = (roll * 0.5).sin_cos();
    [cy * sp * cr + sy * cp * sr, sy * cp * cr - cy * sp * sr, cy * cp * sr - sy * sp * cr, cy * cp * cr + sy * sp * sr]
}

/// A frame that puts one of `rays`' rays across a triangle of `p`: the triangle's centre give or take
/// 0.6 ft, the ray's middle there, turned any way about up and tipped by up to `tilt`.
fn aim(g: &mut Gen, p: &Prop, rays: &flight_search::Rays, tilt: f32) -> Matrix {
    let verts = game_sim::collide::MeshRef::of(&p.mesh);
    let tris = &p.mesh.triangles;
    let tri = &tris[(g.unit() * tris.len() as f32) as usize % tris.len()];
    let m = p.to_world.0;
    let world: Vec<[f32; 3]> = tri
        .vertices
        .map(|v| verts.vertex(v))
        .iter()
        .map(|v| [0, 1, 2].map(|k| v.x() * m[0].0[k] + v.y() * m[1].0[k] + v.z() * m[2].0[k] + m[3].0[k]))
        .collect();
    let target = [0, 1, 2].map(|k| (world[0][k] + world[1][k] + world[2][k]) / 3.0 + g.signed() * 0.6);
    let (yaw, pitch, roll) = (g.unit() * std::f32::consts::TAU, g.signed() * tilt, g.signed() * tilt);
    let r = rows_of(euler_quat(yaw, pitch, roll));
    let n = rays.segments.len();
    let s = rays.segments[(g.unit() * n as f32) as usize % n];
    let mid = [0, 1, 2].map(|k| (s.from.0[k] + s.to.0[k]) * 0.5);
    frame_of(r, [0, 1, 2].map(|k| target[k] - (mid[0] * r[0][k] + mid[1] * r[1][k] + mid[2] * r[2][k])))
}

/// A record, comparable: the nearest, the low/high/push, the first, the part, the flags, and each
/// contact (its ray, normal, point, what it struck, placed, fraction, ray word).
#[derive(Debug, PartialEq)]
struct Seen {
    nearest: [[u32; 4]; 4],
    spread: [[u32; 4]; 3],
    first: ([u32; 4], u32),
    near: (u32, u8),
    contacts: Vec<([u32; 4], [u32; 4], [u32; 4], [u32; 4], Struck, u32, u32, u16)>,
}

fn image_struck(ee: &Ee, parts: &[Vec<u32>], c: u32) -> Struck {
    for (m, p) in parts.iter().enumerate() {
        if let Some(k) = p.iter().position(|&p| p == c) {
            return Struck::Part(m, k);
        }
    }
    if ee.u32_at(c + at::VTABLE) == MESH_VTABLE {
        let w = |o: u32| ee.u32_at(c + o);
        Struck::Mesh([w(at::CENTRE), w(at::CENTRE + 4), w(at::CENTRE + 8), w(at::RADIUS)])
    } else {
        Struck::Other(c)
    }
}

fn port_struck(w: &World, slots: &[usize], o: u32) -> Struck {
    let o = o as usize;
    if let Some(m) = slots.iter().rposition(|&s| s <= o) {
        return Struck::Part(m, o - slots[m]);
    }
    if w.meshes.contains(&o) {
        let p = &w.broad.places[o];
        Struck::Mesh([p.centre[0].0.to_bits(), p.centre[1].0.to_bits(), p.centre[2].0.to_bits(), w.objects[o].radius.0.to_bits()])
    } else {
        Struck::Other(o as u32)
    }
}

fn image_record(ee: &Ee, parts: &[Vec<u32>], r: u32) -> Seen {
    let n = ee.u32_at(r + rec::COUNT);
    let arr = |o: u32| ee.u32_at(r + o);
    for i in 0..ee.u32_at(r + rec::CAPACITY) {
        assert_eq!(ee.u32_at(arr(rec::ZEROED) + 4 * i), 0, "the per-contact words the pair zeroes stay zero");
    }
    Seen {
        nearest: [rec::FROM, rec::TO, rec::NORMAL, rec::POINT].map(|o| words(ee, r + o)),
        spread: [rec::PUSH, rec::LOW, rec::HIGH].map(|o| words(ee, r + o)),
        first: (words(ee, r + rec::FIRST_POINT), ee.u32_at(r + rec::FIRST)),
        near: (ee.u32_at(r + rec::NEAREST), ee.u8_at(r + rec::FLAGS)),
        contacts: (0..n)
            .map(|i| {
                (
                    words(ee, arr(rec::RAYS) + 0x20 * i),
                    words(ee, arr(rec::RAYS) + 0x20 * i + 0x10),
                    words(ee, arr(rec::NORMALS) + 0x10 * i),
                    words(ee, arr(rec::POINTS) + 0x10 * i),
                    image_struck(ee, parts, ee.u32_at(arr(rec::OBJECTS) + 4 * i)),
                    ee.u32_at(arr(rec::PLACED) + 4 * i),
                    ee.u32_at(arr(rec::FRACTIONS) + 4 * i),
                    ee.u16_at(arr(rec::SEGMENTS) + 2 * i),
                )
            })
            .collect(),
    }
}

fn port_record(w: &World, slots: &[usize], r: &Record) -> Seen {
    Seen {
        nearest: [r.from, r.to, r.normal, r.point].map(bits),
        spread: [r.push, r.low, r.high].map(bits),
        first: (bits(r.first_point), r.first.0.to_bits()),
        near: (r.nearest.0.to_bits(), r.flags),
        contacts: r
            .contacts
            .iter()
            .map(|c| (bits(c.from), bits(c.to), bits(c.normal), bits(c.point), port_struck(w, slots, c.object), u32::from(c.placed), c.fraction.0.to_bits(), c.segment))
            .collect(),
    }
}

/// The port's record as the image holds it: the fields the search's clear leaves as they were.
fn record_from_image(ee: &Ee, r: u32, rays: usize, per_ray: usize) -> Record {
    let mut out = atv_props::record(rays, per_ray);
    out.normal = quad_at(ee, r + rec::NORMAL);
    out.point = quad_at(ee, r + rec::POINT);
    out.first_point = quad_at(ee, r + rec::FIRST_POINT);
    out.flags = ee.u8_at(r + rec::FLAGS);
    out
}

/// What one search came to on both sides.
struct Outcome {
    want: Seen,
    got: Seen,
    hit: (bool, bool),
    /// The lists, entry by entry (what and how far), and whether the original's held a placed group.
    lists: (Vec<(Id, u32)>, Vec<(Id, u32)>),
    groups: bool,
    /// How many collidables' lists the callbacks had changed since the last search.
    synced: usize,
}

/// Every collidable's object lists the image's, and how many had changed.
fn sync_lists(d: &mut Data) -> usize {
    let mut changed = 0;
    for (&c, &o) in &d.port {
        let (objects, more) = (lists(&d.ee, c + at::OBJECTS, &d.port), lists(&d.ee, c + at::MORE_OBJECTS, &d.port));
        let p = &mut d.pw.world.objects[o];
        if (&p.objects, &p.more_objects) != (&objects, &more) {
            changed += 1;
            (p.objects, p.more_objects) = (objects, more);
        }
    }
    changed
}

/// The original and the port: probe `k` stood at `t`, the bike's list gathered round `t`'s position
/// and searched.
fn run(d: &mut Data, k: usize, record: &mut Record, t: &Matrix) -> Outcome {
    // What the callbacks did to the lists (a trigger zone struck excludes what struck it) is the
    // ride's: the port's lists are the image's again.
    let synced = sync_lists(d);
    let (ee, p) = (&mut d.ee, &d.probes[k]);
    for (i, row) in t.iter().enumerate() {
        for j in 0..4 {
            ee.set_f32(p.at + at::TRANSFORM + 0x10 * i as u32 + 4 * j, row.0[j as usize]);
        }
    }
    ee.call(STAND, &[u64::from(p.at), u64::from(p.at + at::TRANSFORM)], &[], 1_000_000);
    for j in 0..3 {
        ee.set_f32(d.gather + at::CENTRE + 4 * j, t[3].0[j as usize]);
    }
    ee.call(GATHER, &[u64::from(d.list), u64::from(d.gather), 3], &[], 50_000_000);
    let entries: Vec<(u32, u32)> = (0..ee.u32_at(d.list + 4)).map(|i| (ee.u32_at(ee.u32_at(d.list) + 8 * i), ee.u32_at(ee.u32_at(d.list) + 8 * i + 4))).collect();
    let groups = entries.iter().any(|&(e, _)| ee.u8_at(e + 2) == 3);
    let want_list = entries.iter().map(|&(e, dist)| (image_id(ee, e).expect("a collidable or a plant"), dist)).collect();
    ee.call(SEARCH_FN, &[u64::from(p.at), u64::from(d.list), 1], &[], 200_000_000);
    let want_hit = ee.v0() != 0;
    assert_eq!(want_hit, ee.u8_at(p.at + at::FLAGS) & 0x20 != 0);
    let want = image_record(ee, &d.parts, ee.u32_at(p.at + at::RECORD));
    let reach = ee.u32_at(p.at + at::RADIUS);

    let pw = &mut d.pw;
    let w = &mut pw.world;
    let mut list = Nearby::quad();
    w.place_probe([t[3].x(), t[3].y(), t[3].z()].map(F));
    game_sim::atv_world::gather(&mut w.objects, &mut w.broad, &mut list, PROBE, 3);
    let got_list = list.found.iter().map(|c| (port_id(w, c.object), c.distance.0.to_bits())).collect();
    let plants = Plants::new(&pw.trees, &pw.eco, w, d.slots[0]);
    let mut wheels = Wheels::new(&p.rays, *t, &pw.props, w.meshes.start, plants);
    wheels.record = std::mem::take(record);
    let parts = d.slots[0]..w.objects.len();
    let mut rays = Rays { wheels, parts, zones: pw.zones.clone(), zone_shapes: &pw.zone_shapes, spheres: pw.spheres.clone(), stood: None };
    let retired = |o: usize| d.retired.contains(&o);
    let hit = atv_props::search(&mut w.objects, p.object, &list.found, &retired, &mut rays);
    assert_eq!(w.objects[p.object].radius.0.to_bits(), reach, "the {} probe's reach", p.name);
    *record = rays.wheels.record;
    let got = port_record(w, &d.slots, record);
    Outcome { want, got, hit: (want_hit, hit), lists: (want_list, got_list), groups, synced }
}

#[test]
fn the_bikes_probes_are_the_images() {
    let Some(d) = data() else { return };
    let ee = &d.ee;
    for p in &d.probes {
        let c = p.at;
        assert_eq!(ee.u32_at(c + at::VTABLE), RAY_VTABLE, "the {} probe is a ray probe", p.name);
        assert_eq!([ee.u32_at(c + at::SMOOTH), ee.u32_at(c + at::FILTER), ee.u32_at(c + at::KIND)], [0, 0, 2], "the {} probe: no smooth normals, no per-ray filter", p.name);
        // The trigger rays' tree is the port's tree of them; the parts' (every ray from one point)
        // are the bike's file's, read as the image holds them.
        if p.name == "trigger" {
            let built = game_sim::kiowa::ray_tree(&p.rays.segments);
            assert_eq!((built.root, &built.nodes), (p.rays.root, &p.rays.nodes), "the trigger probe's tree");
        }
        let r = ee.u32_at(c + at::RECORD);
        let want = atv_props::record(p.rays.segments.len(), p.per_ray);
        assert_eq!(ee.u32_at(r + rec::CAPACITY) as usize, want.capacity, "the {} probe's record", p.name);
        assert_eq!(ee.u32_at(r + rec::ONE_PER_RAY) != 0, want.one_per_ray, "the {} probe's record", p.name);
        let t: Matrix = [0, 1, 2, 3].map(|i| quad_at(ee, c + at::TRANSFORM + 0x10 * i));
        let (centre, radius) = game_sim::atv_search::reach(&p.rays, &t);
        assert_eq!(bits(centre)[..3], words(ee, c + at::CENTRE)[..3], "the {} probe's centre", p.name);
        assert_eq!(radius.0.to_bits(), ee.u32_at(c + at::RADIUS), "the {} probe's reach", p.name);
    }
    let parts: usize = d.parts.iter().map(Vec::len).sum();
    let solids: usize = d.pw.eco.species.iter().flatten().map(|s| s.solids.len()).sum();
    assert_eq!(parts, solids, "the species' parts");
    println!("the bike's probes: {:?} rays", d.probes.iter().map(|p| (p.name, p.rays.segments.len())).collect::<Vec<_>>());
}

#[test]
fn the_bikes_probes_search_nat05_as_the_original() {
    let Some(mut d) = data() else { return };
    let per_mesh: usize = std::env::var("F34_FRAMES").ok().and_then(|s| s.parse().ok()).unwrap_or(6);
    let mut g = Gen(0x4633_345f_6e61_7405);
    let mut out = String::new();
    for k in 0..d.probes.len() {
        let p = &d.probes[k];
        let (name, rays) = (p.name, p.rays.clone());
        let r = d.ee.u32_at(p.at + at::RECORD);
        let mut record = record_from_image(&d.ee, r, rays.segments.len(), p.per_ray);
        // Frames that put one ray across a mesh (one in eight tipped anywhere), and across trunks
        // spread over the file (none on Nat05), tipped anywhere; and the probe's own frame in the image.
        let mut frames: Vec<(String, Matrix)> = Vec::new();
        for (o, prop) in d.pw.props.iter().enumerate() {
            for i in 0..per_mesh {
                frames.push((format!("mesh {o} #{i}"), aim(&mut g, prop, &rays, if i % 8 == 7 { 3.0 } else { 0.35 })));
            }
        }
        let trunks = &d.pw.trees.props.props;
        let n = 200.min(trunks.len());
        for j in 0..n {
            let j = j * trunks.len() / n;
            for i in 0..2 {
                frames.push((format!("trunk {j} #{i}"), aim(&mut g, &trunks[j], &rays, 3.0)));
            }
        }
        frames.push(("the image's".into(), [0, 1, 2, 3].map(|i| quad_at(&d.ee, d.probes[k].at + at::TRANSFORM + 0x10 * i))));
        let (mut hits, mut contacts, mut groups, mut hit_only, mut synced) = (0, 0, 0, 0, 0);
        let mut struck: BTreeMap<Struck, usize> = BTreeMap::new();
        for (what, t) in &frames {
            let o = run(&mut d, k, &mut record, t);
            assert_eq!(o.lists.1, o.lists.0, "the {name} probe, {what}: the list");
            assert_eq!(o.got, o.want, "the {name} probe, {what}: the record");
            assert_eq!(o.hit.1, o.hit.0, "the {name} probe, {what}: the hit bit; {} contacts, groups {}, list {:x?}", o.want.contacts.len(), o.groups, o.lists.0);
            // Struck with no contact: a plant's sphere or a zone alone.
            hit_only += usize::from(o.hit.0 && o.want.contacts.is_empty());
            hits += usize::from(o.hit.0);
            synced += o.synced;
            contacts += o.want.contacts.len();
            groups += usize::from(o.groups);
            for c in &o.want.contacts {
                *struck.entry(c.4).or_insert(0) += 1;
            }
        }
        let meshes = struck.keys().filter(|s| matches!(s, Struck::Mesh(_))).count();
        let on_parts: usize = struck.iter().filter(|(s, _)| matches!(s, Struck::Part(..))).map(|(_, n)| n).sum();
        let others: Vec<_> = struck.keys().filter(|s| matches!(s, Struck::Other(_))).collect();
        out += &format!(
            "\nthe {name} probe: {} frames, {hits} hit, {contacts} contacts on {meshes} meshes and {on_parts} on parts, others {others:x?}; \
             {groups} lists held plants; {hit_only} hit with no contact; {synced} lists changed by callbacks",
            frames.len()
        );
        assert!(contacts > 100, "the {name} probe struck something");
    }
    println!("{out}\nsyscalls (number, caller): {:x?}", d.syscalls.borrow());
}


/// A plant group entry's flags (`+0x16`): hidden, turned.
const PLANT_HIDDEN_OR_TURNED: u8 = 0x0c;

#[test]
fn the_plants_spheres_are_the_images() {
    let Some(d) = data() else { return };
    let ee = &d.ee;
    let eco = ee.u32_at(ECO_AT);
    assert_eq!(d.pw.spheres.unit.0.to_bits(), ee.u32_at(eco + 0xc4), "the eco's unit");
    assert_eq!(d.pw.spheres.species.len(), d.parts.len(), "the species");
    let mut spheres = 0;
    for (m, ((scales, built), parts)) in d.pw.spheres.species.iter().zip(&d.parts).enumerate() {
        let a = ee.u32_at(ee.u32_at(eco + 0x84) + 4 * m as u32);
        let want = [0x100, 0x104, 0x10c, 0x114].map(|o| ee.u32_at(a + o));
        assert_eq!([scales.lean[0], scales.lean[1], scales.size[0], scales.size[1]].map(|f| f.0.to_bits()), want, "species {m}'s scale words");
        assert_eq!(built.len(), parts.len(), "species {m}'s parts");
        for (k, (b, &p)) in built.iter().zip(parts).enumerate() {
            assert_eq!(b.is_some(), ee.u32_at(p + at::KIND) == 5, "species {m}'s part {k} is a sphere");
            let Some(b) = b else { continue };
            assert_eq!(bits(b.local), words(ee, p + 0x130), "species {m}'s part {k}'s point");
            for r in 0..3 {
                assert_eq!(bits(b.frame[r]), words(ee, p + 0x140 + 0x10 * r as u32), "species {m}'s part {k}'s frame row {r}");
            }
            assert_eq!((b.radius.0.to_bits(), b.radius2.0.to_bits()), (ee.u32_at(p + 0x180), ee.u32_at(p + 0x184)), "species {m}'s part {k}'s radius");
            spheres += 1;
        }
    }
    let mut plants = 0;
    walk_entries(ee, ee.u32_at(d.tree + 0x8c), &mut |e| {
        if ee.u8_at(e + 2) == 3 {
            assert_eq!(ee.u8_at(e + 0x16) & PLANT_HIDDEN_OR_TURNED, 0, "plant {e:#x}'s flags");
            plants += 1;
        }
    });
    assert!(spheres > 0 && plants > 0, "{spheres} spheres, {plants} plants");
    println!("{spheres} sphere parts, {plants} plant entries");
}
