//! MD76a1 and MD83 — Desert Oasis's collidables and the quad's nearby list, proved against
//! SLUS-21104.
//!
//! The oracle is MD37a's RAM image of Desert Oasis Free Ride with the atv_ quad at its spawn
//! (`context/mxvatv-ps2/md37a/atv-r-spawn.ee.bin`): it holds the loaded world's quadtree, every
//! collidable in it, and the quad's nearby list (quad `+0xc4`) with its probe (quad `+0xd8`). The
//! port builds the same world from the disc's files — `Env\mc02\MC02.scn` and its `Col\*.col` out of
//! `mc02.res`, and `Env\MC02\MC02.esb` out of `MXVSATV.PSR` (both copied into the same folder) — and
//! must hold:
//!
//! - **the tree** the image holds: the same box, cells and levels, and in every node the same
//!   collidables in the same order — meshes, plants, the `.toy`'s zones (`Env\mc02\MC02.toy`), the
//!   start zone's mesh again, the six vehicles' spheres — each with the image's fields. Only the
//!   239 scene nodes of class 7 are left out, which are not collidables;
//! - **the moving list** the image holds, slot for slot: the quad's frame mesh, riding probe and
//!   sweep, the six riders' parts and the two category-0 sweeps, their object lists naming the
//!   same collidables, the rider spheres' radii read from the rider file (`Rider\Anim\mrider.phy`
//!   out of `MXVSATV.PSR`, copied into the same folder). What the image says of the parts' places
//!   and the player's vehicle sphere's position is taken as input: it is the ride's, not the world's;
//! - **the list** the original's gather (`atv_gather_nearby`) fills, run on the image with the probe moved:
//!   every candidate, in order, with its distance, and the bound, bit for bit. The points are every
//!   row of MD37a's fifteen captures (the quad's position), among them the rows where the quad
//!   touches the palms ahead of the spawn (race-jump, vsync 35805 and 35851-35863), points scattered
//!   round every mesh, and points over the whole map.
//!
//! Without the image, the files or the captures the tests skip (and say so).

use game_sim::archive::Res;
use game_sim::atv_world::{self, Nearby, PROBE, World};
use game_sim::eco::Ecosystem;
use game_sim::phy::Ini;
use game_sim::track::Scene;
use game_sim::broad_phase::Structure;
use game_sim::world::Props;
use game_sim::world_search::{Categories, Class, Collidable, Objects};
use ps2_float::F;
use ps2_oracle::{Ee, Kit, skip};
use std::collections::BTreeMap;
use std::path::Path;

/// SLUS-21104's `$gp`, which the call needs (crt0 sets it at `0x001001ac`).
const GP: u64 = 0x003d_baf0;
/// The gather, the quad's list and its probe, the world's quadtree and the ecosystem object.
const GATHER: u32 = 0x001d_6a60;
const QUAD: u32 = 0x015e_0690;
const LIST: u32 = 0x0162_7a40;
const PROBE_AT: u32 = 0x0159_c500;
const TREE: u32 = 0x0065_47c0;
const ECO_AT: u32 = 0x0052_cd88;
/// The scene mesh class's vtable.
const MESH_VTABLE: u32 = 0x003b_6d00;
/// The six vehicles (the player's quad first, another quad, four bikes), whose spheres (`+0xd80`) the
/// tree holds in this order.
const VEHICLES: [u32; 6] = [QUAD, 0x0165_7090, 0x019b_87f0, 0x01a0_5cf0, 0x01a4_85b0, 0x01a8_aed0];
const SPHERE: u32 = 0xd80;
/// The quad's own collidables: the frame mesh, the sweep, the trigger probe and the wreck rays
/// (the riding probe is [`PROBE_AT`]).
const BODY_AT: u32 = 0x0162_7370;
const SWEEP_AT: u32 = 0x0162_82b0;
const TRIGGER_AT: u32 = 0x0162_cd30;
const WRECK_AT: u32 = 0x0162_cef0;
/// The space's moving list, and the zone collidables in the `.toy`'s order.
const MOVING: u32 = 0x0065_52b0;
const ZONES_AT: [u32; 6] = [0x013e_ded0, 0x013e_a4e0, 0x013e_acb0, 0x013e_b480, 0x013e_bc50, 0x013e_fe70];

/// A collidable's fields (the entry the tree and the list hold is at `+0x18`).
mod at {
    pub const ENTRY: u32 = 0x18;
    pub const VTABLE: u32 = 0xc;
    pub const NEARBY: u32 = 0x45;
    pub const CENTRE: u32 = 0x50;
    pub const RADIUS: u32 = 0x70;
    pub const CATEGORY: u32 = 0x94;
    pub const COLLIDES_WITH: u32 = 0xdc;
    pub const OBJECTS: u32 = 0xe0;
    pub const MORE_OBJECTS: u32 = 0xec;
    pub const CATEGORIES: u32 = 0xf8;
    pub const FLAGS: u32 = 0x126;
}

/// What a tree entry or a candidate is, the same on both sides: a collidable by its centre's and
/// reach's bits, a plant by its cell, model and blend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Id {
    Object([u32; 4]),
    Plant(u16, u16, u8, i8),
}

struct Data {
    ee: Ee,
    world: World,
    points: Vec<(String, [f32; 3])>,
    /// The image's collidables the port built one by one, and where.
    port: BTreeMap<u32, usize>,
    zones: Vec<atv_world::Zone>,
}

fn data() -> Option<Data> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let dir = kit.ctx("mxvatv-ps2/md37a");
    let image = dir.join(format!("atv-r-spawn.ee{}", ".bin"));
    let res = dir.join("mc02.res");
    let esb = dir.join("MC02.esb");
    let rider = dir.join("mrider.phy");
    for (p, what) in [
        (&image, "MD37a's eeMemory.bin of atv-r-spawn.p2s (memory: md37a ATV states live in s2)"),
        (&res, "Desert Oasis's mc02.res off MX vs. ATV's disc (Env/MC02)"),
        (&esb, "Env\\MC02\\MC02.esb out of MXVSATV.PSR on MX vs. ATV's disc"),
        (&rider, "Rider\\Anim\\mrider.phy out of MXVSATV.PSR on MX vs. ATV's disc"),
    ] {
        if !p.exists() {
            skip(&format!("{} is not there: {what}", p.display()));
            return None;
        }
    }
    let mut ee = Ee::from_ram(&image).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, GP);
    if ee.u32_at(QUAD + 0xc4) != LIST || ee.u32_at(QUAD + 0xd8) != PROBE_AT {
        panic!("{} is not the MX vs. ATV quad image these addresses are from", image.display());
    }
    let file = std::fs::read(&res).unwrap();
    let archive = Res::parse(&file).unwrap();
    let read = |name: &str| -> Result<Vec<u8>, String> {
        let e = archive.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name)).ok_or(format!("no {name}"))?;
        archive.read(&file, e)
    };
    let scene = Scene::parse(&read("Env\\mc02\\MC02.scn").unwrap()).unwrap();
    let (props, missed) = Props::load(&scene, |n| read(&format!("Col\\{n}")));
    assert!(missed.is_empty(), "MC02's meshes not read: {missed:?}");
    let eco = Ecosystem::parse(&std::fs::read(&esb).unwrap()).unwrap();
    let toy = Ini::parse_bytes(&read("Env\\mc02\\MC02.toy").unwrap());
    let (groups, models) = atv_world::groups(&eco);
    let field = atv_world::terrain_box(&scene.terrain);
    let hi = field.1;
    let mut world = World::new(collidable(&ee, PROBE_AT), &props.props, &groups, models, atv_world::plant_cell(field), field);
    let mut port = BTreeMap::from([(PROBE_AT, PROBE)]);
    // After the plants: the zones, the start zone's mesh again, the vehicles' spheres.
    let zones = atv_world::zones(&toy);
    let filed = world.file_zones(&zones, hi);
    port.extend(ZONES_AT.iter().copied().zip(filed));
    for i in atv_world::start_zones(&scene) {
        world.file_again(world.meshes.start + i, &props.props[i], hi);
    }
    let vehicles = world.add_vehicles(VEHICLES.len(), hi);
    port.extend(VEHICLES.iter().map(|v| ee.u32_at(v + SPHERE)).zip(vehicles.clone()));
    // The player's was filed where the quad last gathered round it (quad `+0xd90`), and has moved on
    // since by less than the 10 ft that files it again.
    let player = vehicles.start;
    world.move_vehicle(player, [0, 4, 8].map(|i| F(ee.f32_at(QUAD + 0xd90 + i))), F(atv_world::VEHICLE_REACH), hi);
    world.broad.places[player].centre = centre(&ee, ee.u32_at(QUAD + SPHERE));
    // The moving list: the quad's three, then (after slots freed) the riders' parts in pairs and the
    // two category-0 sweeps. The quad's other two are in no structure.
    let block = ee.u32_at(ee.u32_at(MOVING + 0x34));
    let image_at = |slot: u32| ee.u32_at(block + 4 * slot) - at::ENTRY;
    for (c, slot) in [(BODY_AT, Some(0)), (PROBE_AT, Some(1)), (SWEEP_AT, Some(2)), (TRIGGER_AT, None), (WRECK_AT, None)] {
        let o = match port.get(&c) {
            Some(&o) => o,
            None => world.add(collidable(&ee, c), centre(&ee, c)),
        };
        port.insert(c, o);
        if let Some(slot) = slot {
            world.put_moving(o, slot);
        }
    }
    // Six riders of nine pairs, each sphere's radius the rider file's.
    let radii = atv_world::rider_radii(&Ini::parse_bytes(&std::fs::read(&rider).unwrap()));
    assert_eq!(radii.len(), 9, "the rider file's collision spheres");
    for slot in 18..126u32 {
        let c = image_at(slot);
        let part = if slot % 2 == 0 { atv_world::rider_sphere(radii[(slot as usize - 18) / 2 % 9]) } else { atv_world::rider_sweep() };
        let o = world.add(part, centre(&ee, c));
        world.put_moving(o, slot as usize);
        port.insert(c, o);
    }
    for slot in [126, 127u32] {
        let c = image_at(slot);
        let o = world.add(atv_world::owned_sweep(), centre(&ee, c));
        world.put_moving(o, slot as usize);
        port.insert(c, o);
    }
    // The quad's parts' lists, naming the port's collidables.
    for c in [BODY_AT, PROBE_AT, SWEEP_AT, TRIGGER_AT, WRECK_AT] {
        let o = port[&c];
        world.objects[o].objects = objects(&ee, c + at::OBJECTS, &port);
        world.objects[o].more_objects = objects(&ee, c + at::MORE_OBJECTS, &port);
    }
    world.broad.places[PROBE].centre = centre(&ee, PROBE_AT);
    let points = capture_points(&kit.ctx("capture/md37a"))?;
    Some(Data { ee, world, points, port, zones })
}

/// A collidable's centre (`+0x50`).
fn centre(ee: &Ee, c: u32) -> [F; 4] {
    [0, 4, 8, 12].map(|i| F(ee.f32_at(c + at::CENTRE + i)))
}

/// One of the image's lists of collidables (mode, array, count), as the port's.
fn objects(ee: &Ee, l: u32, port: &BTreeMap<u32, usize>) -> Objects {
    let (mode, at, n) = (ee.u32_at(l), ee.u32_at(l + 4), ee.u32_at(l + 8));
    let list = (0..n).map(|i| *port.get(&ee.u32_at(at + 4 * i)).expect("a collidable the port built")).collect();
    Objects { mode, list }
}

fn categories(ee: &Ee, c: u32) -> Categories {
    let c = c + at::CATEGORIES;
    Categories { mode: ee.u32_at(c), list: (0..ee.u32_at(c + 8)).map(|i| ee.u32_at(ee.u32_at(c + 4) + 4 * i)).collect() }
}

/// The image's collidable `c` as the port's (the quad's own parts, whose fields the quad's code
/// sets): its reach, flags and filter, its object lists empty until every one they name is built.
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

/// `c`'s fields the gather and the pair filter read, the image's against the port's `o`: class,
/// flags (the low four bits), nearby, reach, category, mask, category list and object lists (by
/// what they name). `port` says which image collidable each port one is.
fn same_fields(ee: &Ee, c: u32, w: &World, o: usize, image: &BTreeMap<usize, u32>, what: &str) {
    let p = &w.objects[o];
    assert_eq!(p.class, Class::Object, "{what}");
    assert_eq!(ee.u8_at(c + at::FLAGS) & 0x0f, p.flags, "{what}: flags");
    assert_eq!(ee.u8_at(c + at::NEARBY) & 1 != 0, p.nearby, "{what}: nearby");
    assert_eq!(ee.u32_at(c + at::RADIUS), p.radius.0.to_bits(), "{what}: reach");
    assert_eq!(ee.u32_at(c + at::CATEGORY), p.category, "{what}: category");
    assert_eq!(ee.u32_at(c + at::COLLIDES_WITH), p.collides_with, "{what}: collides with");
    assert_eq!(categories(ee, c), p.categories, "{what}: categories");
    assert_eq!(centre(ee, c).map(|f| f.0.to_bits()), w.broad.places[o].centre.map(|f| f.0.to_bits()), "{what}: centre");
    for (l, mine) in [(at::OBJECTS, &p.objects), (at::MORE_OBJECTS, &p.more_objects)] {
        let (mode, list, n) = (ee.u32_at(c + l), ee.u32_at(c + l + 4), ee.u32_at(c + l + 8));
        let named: Vec<u32> = (0..n).map(|i| ee.u32_at(list + 4 * i)).collect();
        let mine_named: Vec<u32> = mine.list.iter().map(|o| *image.get(o).expect("a port collidable the image has")).collect();
        assert_eq!(named, mine_named, "{what}: object list +{l:#x}");
        if n != 0 {
            assert_eq!(mode, mine.mode, "{what}: object list +{l:#x}'s mode");
        }
    }
}

/// The quad's position in every row of the captures, named by capture and vsync.
fn capture_points(dir: &Path) -> Option<Vec<(String, [f32; 3])>> {
    let mut names: Vec<_> = std::fs::read_dir(dir).ok().into_iter().flatten().flatten().map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "csv")).collect();
    names.sort();
    if names.len() < 15 {
        skip(&format!("{} does not hold MD37a's fifteen captures", dir.display()));
        return None;
    }
    let mut out = Vec::new();
    for name in names {
        let text = std::fs::read_to_string(&name).unwrap();
        let mut lines = text.lines();
        let head: Vec<&str> = lines.next().unwrap().split(',').collect();
        let col = |h: &str| head.iter().position(|c| *c == h).unwrap();
        let (frame, x, y, z) = (col("frame"), col("pos.x"), col("pos.y"), col("pos.z"));
        let stem = name.file_stem().unwrap().to_string_lossy().into_owned();
        for line in lines {
            let f: Vec<&str> = line.split(',').collect();
            let p = [x, y, z].map(|c| f[c].parse::<f32>().unwrap());
            out.push((format!("{stem}@{}", f[frame]), p));
        }
    }
    Some(out)
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

/// Every node's entries in order, by the node's path from the root (a child's index per step),
/// each slot as it is: a collidable a node holds twice (SLUS-21104 files Zone_Platform_Machine's mesh
/// a second time, after the zones) is there twice, and an empty slot is `None`. `again` counts the
/// second filings, `skipped` the scene nodes left out.
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
    // Where a scene node took a slot the port has no entry for, only the order is the port's.
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

#[test]
fn mc02s_tree_holds_every_collidable_of_the_originals_in_its_order() {
    let Some(d) = data() else { return };
    let (ee, w) = (&d.ee, &d.world);
    let Structure::Tree(t) = &w.broad.structures[0] else { unreachable!() };
    let bits = |a: u32| ee.u32_at(TREE + a);
    assert_eq!([t.origin[0], t.origin[1], t.extent[0], t.extent[1], t.scale].map(|f| f.0.to_bits()),
        [bits(0x28), bits(0x30), bits(0x40), bits(0x44), bits(0x54)], "the tree's box and scale");
    assert_eq!(t.cell as u32, bits(0x50), "the tree's least cell");
    assert_eq!(w.broad.cell_size.0, f32::from_bits(ee.u32_at(0x0040_c1a0)), "world units a cell");
    // The ecosystem's cell and the species' reaches the group distance reads.
    let eco = ee.u32_at(ECO_AT);
    assert_eq!(w.broad.group_cell.0.to_bits(), ee.u32_at(eco + 0xc4), "the plants' cell");
    let models = ee.u32_at(eco + 0x84);
    let mut used: Vec<u8> = w.groups.clone().map(|g| w.broad.places[g].model).collect();
    used.sort();
    used.dedup();
    for m in &used {
        let at = ee.u32_at(models + 4 * u32::from(*m));
        let model = w.broad.models[usize::from(*m)];
        assert_eq!([model.near.0.to_bits(), model.far.0.to_bits()], [ee.u32_at(at + 0x10c), ee.u32_at(at + 0x114)], "species {m}'s reach");
    }

    let (mut want, mut got, mut skipped, mut again) = (BTreeMap::new(), BTreeMap::new(), 0, 0);
    image_nodes(ee, bits(0x8c), String::new(), &mut want, &mut skipped, &mut again);
    port_nodes(w, t.root.unwrap(), String::new(), &mut got);
    let count = |m: &BTreeMap<String, Vec<Option<Id>>>| m.values().flatten().flatten().count();
    let holes = |m: &BTreeMap<String, Vec<Option<Id>>>| m.values().flatten().filter(|i| i.is_none()).count();
    println!("tree: {} entries in {} nodes ({} empty slots), {again} filed again in a node, {skipped} scene nodes; port {} in {} nodes ({} empty)",
        count(&want), want.len(), holes(&want), count(&got), got.len(), holes(&got));
    let differ: Vec<_> = want
        .iter()
        .filter(|(p, ids)| {
            let mine = got.get(*p).map(|g| if ids.contains(&None) { g.clone() } else { g.iter().copied().filter(Option::is_some).collect() });
            mine.as_ref() != Some(*ids)
        })
        .map(|(p, ids)| (p, ids, got.get(p)))
        .collect();
    for (p, w, g) in &differ {
        println!("node {p:?}: image {w:?}\n  port {g:?}");
    }
    assert!(differ.is_empty(), "{} nodes differ", differ.len());
    assert_eq!(want.len(), got.len(), "nodes holding something");
    assert_eq!(count(&want), count(&got), "entries");
    assert_eq!(skipped, 239, "MC02's scene nodes");
    assert_eq!(again, 2, "the start zone's mesh, a second time in each of its two nodes");
    assert_eq!(holes(&want), 1, "the slot the player's sphere left");

    // Each collidable is the image's: every field the gather and the pair filter read. A mesh,
    // a zone or a sphere is found by its centre and reach.
    let image: BTreeMap<usize, u32> = d.port.iter().map(|(&c, &o)| (o, c)).collect();
    let (mut checked, mut kinds) = (std::collections::BTreeSet::new(), BTreeMap::new());
    walk_entries(ee, bits(0x8c), &mut |e| {
        if ee.u8_at(e + 2) != 2 || !checked.insert(e) {
            return;
        }
        let c = e - at::ENTRY;
        let id = image_id(ee, e);
        let o = (0..w.objects.len()).find(|&o| w.objects[o].class == Class::Object && Some(port_id(w, o)) == id).expect("every image collidable is a port one");
        same_fields(ee, c, w, o, &image, &format!("{c:#x}"));
        *kinds.entry((ee.u32_at(c + at::VTABLE), ee.u32_at(c + at::CATEGORY))).or_insert(0) += 1;
    });
    println!("collidables by (vtable, category): {kinds:x?}");
    let filed = w.objects.iter().filter(|o| o.class == Class::Object).count();
    let moving = 3 + 108 + 2 + 2; // the moving list's, and the quad's two in no structure
    assert_eq!(checked.len(), filed - moving, "every port collidable in the tree is one of the image's");
    assert_eq!(kinds.get(&(MESH_VTABLE, atv_world::MESH)), Some(&w.meshes.len()), "the scene's meshes");
    assert_eq!(kinds.get(&(0x003b_6b40, atv_world::VEHICLE)), Some(&6), "the vehicles' spheres");
}

/// The zones are the `.toy`'s: each one's collidable, its frame and its reach, bit for bit.
#[test]
fn mc02s_zones_are_the_toys_triggers() {
    let Some(d) = data() else { return };
    let ee = &d.ee;
    assert_eq!(d.zones.len(), ZONES_AT.len());
    for (z, &c) in d.zones.iter().zip(&ZONES_AT) {
        let w = |o: u32| ee.u32_at(c + o);
        assert_eq!(z.centre().map(|f| f.0.to_bits()), [0, 4, 8, 12].map(|i| w(at::CENTRE + i)), "{}: centre", z.name);
        assert_eq!(z.reach().0.to_bits(), w(at::RADIUS), "{}: reach", z.name);
        let f = z.frame().to_bits();
        match z.shape {
            atv_world::Shape::Sphere { radius } => {
                assert_eq!(w(0xc), 0x003b_6ec0, "{}: a sphere", z.name);
                assert_eq!(z.centre().map(|f| f.0.to_bits()), [0, 4, 8, 12].map(|i| w(0x130 + i)), "{}: local centre", z.name);
                assert_eq!(f, std::array::from_fn(|i| w(0x140 + 4 * i as u32)), "{}: frame", z.name);
                assert_eq!([radius.0.to_bits(), (radius * radius).0.to_bits()], [w(0x180), w(0x184)], "{}: radius", z.name);
            }
            atv_world::Shape::Box { half, .. } => {
                assert_eq!(w(0xc), 0x003b_6a60, "{}: a box", z.name);
                assert_eq!(f, std::array::from_fn(|i| w(0x130 + 4 * i as u32)), "{}: frame", z.name);
                assert_eq!(half.map(|h| h.0.to_bits()), [0, 4, 8].map(|i| w(0x180 + i)), "{}: half sides", z.name);
            }
        }
    }
}

/// The moving list is the image's, slot for slot, and each collidable in it the image's.
#[test]
fn mc02s_moving_list_is_the_originals() {
    let Some(d) = data() else { return };
    let (ee, w) = (&d.ee, &d.world);
    let Structure::List(l) = &w.broad.structures[1] else { unreachable!() };
    assert_eq!([l.flags, l.size as u32, l.last as u32], [0x0c, 0x28, 0x30].map(|o| ee.u32_at(MOVING + o)), "flags, block size, slots used");
    let blocks = l.blocks.as_ref().expect("a block");
    assert_eq!(blocks.len() as u32, ee.u32_at(MOVING + 0x2c), "blocks");
    let block = ee.u32_at(ee.u32_at(MOVING + 0x34));
    let image: BTreeMap<usize, u32> = d.port.iter().map(|(&c, &o)| (o, c)).collect();
    let mut used = 0;
    for slot in 0..l.last as u32 {
        let e = ee.u32_at(block + 4 * slot);
        match blocks[0][slot as usize] {
            None => assert!(e & 2 != 0, "slot {slot} is free"),
            Some(o) => {
                assert!(e & 2 == 0, "slot {slot} is used");
                assert_eq!(image.get(&o), Some(&(e - at::ENTRY)), "slot {slot}");
                same_fields(ee, e - at::ENTRY, w, o, &image, &format!("slot {slot}"));
                used += 1;
            }
        }
    }
    assert_eq!(used, 3 + 108 + 2);
    // The space's tables: the tree fixed, the list moving.
    let tables = ee.u32_at(ee.u32_at(PROBE_AT + 0x20) + 4);
    assert_eq!([ee.u32_at(tables + 0x28), ee.u32_at(tables + 0x2c), ee.u32_at(tables + 0x3c)], [TREE, MOVING, 0]);
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

/// A deterministic generator: inputs only.
struct Gen(u64);

impl Gen {
    fn unit(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

#[test]
fn the_quads_nearby_list_is_the_originals() {
    let Some(mut d) = data() else { return };
    let mut points = d.points.clone();
    let mut g = Gen(0x4d44_3736_6131);
    // Round every mesh, a few feet to its reach and more away, and over the whole map.
    let centres: Vec<[f32; 4]> = d.world.meshes.clone().map(|o| {
        let p = &d.world.broad.places[o];
        [p.centre[0].0, p.centre[1].0, p.centre[2].0, d.world.objects[o].radius.0]
    }).collect();
    for (k, c) in centres.iter().enumerate() {
        for i in 0..24 {
            let r = c[3] + 30.0;
            let p = [c[0] + (g.unit() * 2.0 - 1.0) * r, c[1] + (g.unit() * 2.0 - 1.0) * 20.0, c[2] + (g.unit() * 2.0 - 1.0) * r];
            points.push((format!("mesh {k} #{i}"), p));
        }
    }
    for i in 0..4000 {
        let p = [g.unit() * 5800.0 - 20.0, 100.0 + g.unit() * 60.0, g.unit() * 5800.0 - 20.0];
        points.push((format!("map #{i}"), p));
    }
    // A probe with no filter and flat distances, and one that skips the plants, a few hundred times
    // each: the list's other modes.
    let modes = [(1u32, 0u32, 0u32), (0, 1, 0), (1, 0, 1), (0, 0, 0)];

    let (mut lists, mut entries, mut palms, mut plants, mut meshes) = (0, 0, 0, 0, 0);
    let mut kept: BTreeMap<(bool, u32), usize> = BTreeMap::new();
    let mut longest = 0;
    let palm_rows = ["race-jump@35805", "race-jump@35851", "race-jump@35855", "race-jump@35859", "race-jump@35861", "race-jump@35863"];
    for (n, (name, p)) in points.iter().enumerate() {
        let mode = if n < d.points.len() || n % 8 != 0 { modes[0] } else { modes[1 + (n / 8) % 3] };
        let ee = &mut d.ee;
        ee.set_u32(LIST + 0x38, mode.0);
        ee.set_u32(LIST + 0x3c, mode.1);
        ee.set_u32(LIST + 0x2c, mode.2);
        for (i, v) in p.iter().enumerate() {
            ee.set_f32(PROBE_AT + at::CENTRE + 4 * i as u32, *v);
        }
        ee.call(GATHER, &[u64::from(LIST), u64::from(PROBE_AT), 3], &[], 50_000_000);
        let count = ee.u32_at(LIST + 4);
        let at = ee.u32_at(LIST);
        let mut want = Vec::new();
        for i in 0..count {
            let e = ee.u32_at(at + 8 * i);
            let id = image_id(ee, e).unwrap_or_else(|| panic!("{name}: a scene node gathered"));
            want.push((id, ee.u32_at(at + 8 * i + 4)));
            if ee.u8_at(e + 2) == 2 {
                *kept.entry((mode.0 != 0, ee.u32_at(e - at::ENTRY + at::CATEGORY))).or_insert(0) += 1;
            }
        }

        let w = &mut d.world;
        let mut list = Nearby { filter: mode.0 != 0, flat: mode.1 != 0, no_groups: mode.2 != 0, ..Nearby::quad() };
        w.place_probe(p.map(F));
        atv_world::gather(&mut w.objects, &mut w.broad, &mut list, PROBE, 3);
        let got: Vec<(Id, u32)> = list.found.iter().map(|c| (port_id(w, c.object), c.distance.0.to_bits())).collect();
        assert_eq!(list.bound.0.to_bits(), d.ee.u32_at(LIST + 0x20), "{name} {p:?}: the bound");
        assert_eq!(got, want, "{name} {p:?} (mode {mode:?}): the list");

        lists += 1;
        entries += got.len();
        longest = longest.max(got.len());
        for (id, _) in &got {
            match id {
                Id::Object(_) => meshes += 1,
                Id::Plant(..) => plants += 1,
            }
        }
        if palm_rows.contains(&name.as_str()) {
            let palm = |reach: f32| got.iter().any(|(id, _)| matches!(id, Id::Object(m) if (f32::from_bits(m[3]) - reach).abs() < 0.05));
            assert!(palm(23.49) && palm(28.6), "{name}: the two palms are in the list: {got:?}");
            palms += 1;
        }
    }
    println!(
        "{lists} lists ({} from the captures), {entries} candidates ({meshes} collidables, {plants} plants), longest {longest}; \
         collidables by (filter, category): {kept:x?}; {palms} palm rows",
        d.points.len()
    );
    assert_eq!(palms, palm_rows.len(), "every palm row was asked");
    // Every list held zones, and without the filter the quad's own frame mesh, each as the
    // original's: what MD76a1's world lacked.
    for k in [(true, atv_world::ZONE), (false, atv_world::ZONE), (false, 0x3012)] {
        assert!(kept.contains_key(&k), "no list held {k:x?}: {kept:x?}");
    }
    assert!(meshes > 1000 && plants > 1000, "the lists hold both meshes and plants");
}
