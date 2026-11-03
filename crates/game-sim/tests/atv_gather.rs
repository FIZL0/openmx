//! MD76a1 — Desert Oasis's collidables and the quad's nearby list, proved against SLUS-21104.
//!
//! The oracle is MD37a's RAM image of Desert Oasis Free Ride with the atv_ quad at its spawn
//! (`context/mxvatv-ps2/md37a/atv-r-spawn.ee.bin`): it holds the loaded world's quadtree, every
//! collidable in it, and the quad's nearby list (quad `+0xc4`) with its probe (quad `+0xd8`). The
//! port builds the same world from the disc's files — `Env\mc02\MC02.scn` and its `Col\*.col` out of
//! `mc02.res`, and `Env\MC02\MC02.esb` out of `MXVSATV.PSR` (both copied into the same folder) — and
//! must hold:
//!
//! - **the tree** the image holds: the same box, cells and levels, and in every node the same
//!   meshes and plants in the same order (the image's other entries — zones, class 7 — left out);
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
/// The scene mesh class's vtable: the class-2 collidables the port builds.
const MESH_VTABLE: u32 = 0x003b_6d00;

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

/// What a tree entry or a candidate is, the same on both sides: a mesh by its centre's and reach's
/// bits, a plant by its cell, model and blend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Id {
    Mesh([u32; 4]),
    Plant(u16, u16, u8, i8),
}

struct Data {
    ee: Ee,
    world: World,
    points: Vec<(String, [f32; 3])>,
}

fn data() -> Option<Data> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let dir = kit.ctx("mxvatv-ps2/md37a");
    let image = dir.join(format!("atv-r-spawn.ee{}", ".bin"));
    let res = dir.join("mc02.res");
    let esb = dir.join("MC02.esb");
    for (p, what) in [
        (&image, "MD37a's eeMemory.bin of atv-r-spawn.p2s (memory: md37a ATV states live in s2)"),
        (&res, "Desert Oasis's mc02.res off MX vs. ATV's disc (Env/MC02)"),
        (&esb, "Env\\MC02\\MC02.esb out of MXVSATV.PSR on MX vs. ATV's disc"),
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
    let (groups, models) = atv_world::groups(&eco);
    let field = atv_world::terrain_box(&scene.terrain);
    let world = World::new(probe(&ee), &props.props, &groups, models, atv_world::plant_cell(field), field);
    let points = capture_points(&kit.ctx("capture/md37a"))?;
    Some(Data { ee, world, points })
}

/// The image's probe as the port's collidable: its reach, flags and filter. The collidables its
/// lists name (the quad's own) are not in the port's world; each stands for a number past its end.
fn probe(ee: &Ee) -> Collidable {
    let mut named = BTreeMap::new();
    let mut objects = |off: u32| {
        let (mode, at, n) = (ee.u32_at(PROBE_AT + off), ee.u32_at(PROBE_AT + off + 4), ee.u32_at(PROBE_AT + off + 8));
        let list = (0..n)
            .map(|i| {
                let p = ee.u32_at(at + 4 * i);
                let next = usize::MAX - named.len();
                *named.entry(p).or_insert(next)
            })
            .collect();
        Objects { mode, list }
    };
    let (o, m) = (objects(at::OBJECTS), objects(at::MORE_OBJECTS));
    let c = PROBE_AT + at::CATEGORIES;
    let categories = Categories {
        mode: ee.u32_at(c),
        list: (0..ee.u32_at(c + 8)).map(|i| ee.u32_at(ee.u32_at(c + 4) + 4 * i)).collect(),
    };
    Collidable {
        class: Class::Object,
        flags: ee.u8_at(PROBE_AT + at::FLAGS),
        nearby: ee.u8_at(PROBE_AT + at::NEARBY) & 1 != 0,
        radius: F(ee.f32_at(PROBE_AT + at::RADIUS)),
        category: ee.u32_at(PROBE_AT + at::CATEGORY),
        collides_with: ee.u32_at(PROBE_AT + at::COLLIDES_WITH),
        objects: o,
        more_objects: m,
        categories,
        ..Collidable::default()
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

/// The image's tree entry `e` as an [`Id`], or `None` for one the port does not build.
fn image_id(ee: &Ee, e: u32) -> Option<Id> {
    match ee.u8_at(e + 2) {
        2 => {
            let c = e - at::ENTRY;
            (ee.u32_at(c + at::VTABLE) == MESH_VTABLE && ee.u32_at(c + at::CATEGORY) == atv_world::MESH).then(|| {
                let w = |o: u32| ee.u32_at(c + o);
                Id::Mesh([w(at::CENTRE), w(at::CENTRE + 4), w(at::CENTRE + 8), w(at::RADIUS)])
            })
        }
        3 => Some(Id::Plant(ee.u16_at(e + 0xc), ee.u16_at(e + 0x10), ee.u8_at(e + 0x12), ee.u8_at(e + 0x15) as i8)),
        _ => None,
    }
}

fn port_id(w: &World, o: usize) -> Id {
    let p = &w.broad.places[o];
    match w.objects[o].class {
        Class::Object => Id::Mesh([p.centre[0].0.to_bits(), p.centre[1].0.to_bits(), p.centre[2].0.to_bits(), w.objects[o].radius.0.to_bits()]),
        _ => Id::Plant(p.cell[0], p.cell[1], p.model, p.blend),
    }
}

/// Every node's entries in order, by the node's path from the root (a child's index per step).
///
/// A collidable a node holds twice is kept where it is first: a query hands each out once (its
/// stamp), so a second entry in the same node is never what the walk meets first. `again` counts
/// them (SLUS-21104 files Zone_Platform_Machine's mesh a second time, after the plants).
fn image_nodes(ee: &Ee, node: u32, path: String, out: &mut BTreeMap<String, Vec<Id>>, skipped: &mut usize, again: &mut usize) {
    let (mut ids, mut seen) = (Vec::new(), Vec::new());
    let mut chunk = ee.u32_at(node + 4);
    while chunk != 0 {
        for s in 0..4 {
            let e = ee.u32_at(chunk + 4 + 4 * s);
            if e != 0 {
                if seen.contains(&e) {
                    *again += 1;
                    continue;
                }
                seen.push(e);
                match image_id(ee, e) {
                    Some(id) => ids.push(id),
                    None => *skipped += 1,
                }
            }
        }
        chunk = ee.u32_at(chunk);
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

fn port_nodes(w: &World, node: usize, path: String, out: &mut BTreeMap<String, Vec<Id>>) {
    let Structure::Tree(t) = &w.broad.structures[0] else { unreachable!() };
    let n = &t.nodes[node];
    let ids: Vec<Id> = n.chunks.iter().flatten().flatten().map(|&o| port_id(w, o)).collect();
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
fn mc02s_tree_holds_the_originals_meshes_and_plants_in_its_order() {
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
    let count = |m: &BTreeMap<String, Vec<Id>>| m.values().map(Vec::len).sum::<usize>();
    println!("tree: {} entries the port builds in {} nodes, {skipped} it does not, {again} filed again in a node; port {} in {} nodes",
        count(&want), want.len(), count(&got), got.len());
    let differ: Vec<_> = want.iter().filter(|(p, ids)| got.get(*p) != Some(*ids)).map(|(p, ids)| (p, ids, got.get(p))).collect();
    for (p, w, g) in &differ {
        println!("node {p:?}: image {w:?}\n  port {g:?}");
    }
    assert!(differ.is_empty(), "{} nodes differ", differ.len());
    assert_eq!(want.len(), got.len(), "nodes holding something");

    // Each mesh is the image's: its category, its flags and its lists.
    let mut meshes = 0;
    for ids in want.values() {
        for id in ids {
            if let Id::Mesh(_) = id {
                meshes += 1;
            }
        }
    }
    let mut mine: Vec<Id> = w.meshes.clone().map(|o| port_id(w, o)).collect();
    mine.sort();
    mine.dedup();
    assert_eq!(mine.len(), w.meshes.len(), "the meshes are told apart by their centres and reaches");
    assert!(meshes >= mine.len());
    let mut checked = 0;
    let mut seen = std::collections::BTreeSet::new();
    walk_entries(ee, bits(0x8c), &mut |e| {
        if ee.u8_at(e + 2) != 2 || !seen.insert(e) {
            return;
        }
        let c = e - at::ENTRY;
        if ee.u32_at(c + at::VTABLE) != MESH_VTABLE {
            return;
        }
        let o = w.meshes.clone().find(|&o| Some(port_id(w, o)) == image_id(ee, e)).expect("every image mesh is a port mesh");
        let p = &w.objects[o];
        assert_eq!(ee.u32_at(c + at::CATEGORY), p.category);
        assert_eq!(ee.u8_at(c + at::FLAGS) & 0x0f, p.flags, "mesh {c:#x}'s flags");
        assert_eq!(ee.u8_at(c + at::NEARBY) & 1 != 0, p.nearby);
        assert_eq!(ee.u32_at(c + at::COLLIDES_WITH), p.collides_with);
        assert_eq!([ee.u32_at(c + at::CATEGORIES), ee.u32_at(c + at::CATEGORIES + 8)], [p.categories.mode, p.categories.list.len() as u32]);
        for l in [at::OBJECTS, at::MORE_OBJECTS] {
            assert_eq!(ee.u32_at(c + l + 8), 0, "mesh {c:#x}'s object lists are empty");
        }
        checked += 1;
    });
    assert_eq!(checked, w.meshes.len(), "every port mesh is one of the image's");
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
    let mut left: BTreeMap<(bool, u32), usize> = BTreeMap::new();
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
            match image_id(ee, e) {
                Some(id) => want.push((id, ee.u32_at(at + 8 * i + 4))),
                None => {
                    assert_eq!(ee.u8_at(e + 2), 2, "{name}: only collidables the port does not build are left out");
                    *left.entry((mode.0 != 0, ee.u32_at(e - at::ENTRY + at::CATEGORY))).or_insert(0) += 1;
                }
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
                Id::Mesh(_) => meshes += 1,
                Id::Plant(..) => plants += 1,
            }
        }
        if palm_rows.contains(&name.as_str()) {
            let palm = |reach: f32| got.iter().any(|(id, _)| matches!(id, Id::Mesh(m) if (f32::from_bits(m[3]) - reach).abs() < 0.05));
            assert!(palm(23.49) && palm(28.6), "{name}: the two palms are in the list: {got:?}");
            palms += 1;
        }
    }
    println!(
        "{lists} lists ({} from the captures), {entries} candidates ({meshes} meshes, {plants} plants), longest {longest}; \
         left out by (filter, category) (the original's, not built): {left:x?}; {palms} palm rows",
        d.points.len()
    );
    assert_eq!(palms, palm_rows.len(), "every palm row was asked");
    // With the pair filter (the quad's own list), only the scene's zones; without, the moving
    // list's vehicles and riders too, which the port's world does not hold.
    assert!(left.keys().all(|&(filter, c)| c == 0x8000 || !filter), "left out: {left:x?}");
    assert!(meshes > 1000 && plants > 1000, "the lists hold both meshes and plants");
}
