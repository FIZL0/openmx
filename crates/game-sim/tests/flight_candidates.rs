//! The plane's candidates in the original's order (S96), against biplane.p2m2's save state and the disc.
//!
//! S48 sorted the track's props by distance; the original hands them out in the order its broad
//! phase's walk meets them, and that order is made when the world is built. Four proofs:
//!
//! 1. **the tree** — free06's scene props, put in the port's tree ([`flight_search::world`]), sit in
//!    the save state's tree node for node and, within each node, in the same order; the tree is laid
//!    over the same box with the same cells;
//! 2. **the gather** — the original's `collision_search_gather` run on the save state around the props
//!    and the port's over its own world list the same props in the same order with the same distances,
//!    gathered fresh and then kept and measured again;
//! 3. **the 34** — the save state's own list: the port's world with the moving objects it does not
//!    have (the toys, the bikes, the race gates) put in as the state has them, gathered where the state
//!    gathered and measured where its probe is now, is the state's 34 candidates, in order, bit for bit.
//! 4. **when** (S99) — a second state, saved with the moving objects' centres copied at the moment the
//!    plane's post listener gathered: with those centres every distance is the state's, and with the
//!    ones it holds only the objects that moved after the gather are apart.

use std::collections::HashMap;

use game_sim::archive::{Psr, Res};
use game_sim::broad_phase::{Blocks, Broad, Quadtree, Search, Structure, gather};
use game_sim::flight_search::{self, FIRST, RAY, SWEEP};
use game_sim::track::{Scene, Terrain};
use game_sim::world::{Ground, Props};
use game_sim::world_search::{Candidate, Categories, Class, Collidable, Objects};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

const STATE: &str = "biplane.p2m2_SaveState.eeMemory.bin";
const TRACK: &str = "Tracks\\free06.res";
const PLANE: u32 = 0x00c4_4a40;
const PLANE_COLLISION: u32 = 0x170;
const NEARBY: u32 = 0x10c;
const RAY_PROBE: u32 = 0x1f0;
const SWEEP_PROBE: u32 = 0x1f4;
const BUDGET: u64 = 50_000_000;
const CASES: usize = 3_000;

/// A collidable's fields (S44/S45's offsets).
mod at {
    pub const CLASS: u32 = 0x02;
    pub const SPACE: u32 = 0x04;
    pub const NEARBY: u32 = 0x31;
    pub const CENTRE: u32 = 0x40;
    pub const RADIUS: u32 = 0x60;
    pub const CATEGORY: u32 = 0x84;
    pub const COLLIDES_WITH: u32 = 0xcc;
    pub const OBJECTS: u32 = 0xd0;
    pub const MORE_OBJECTS: u32 = 0xdc;
    pub const CATEGORIES: u32 = 0xe8;
    pub const FLAGS: u32 = 0x116;
}

/// A space link's table; a table's two structures; the tree's and the moving list's fields.
mod tree_at {
    pub const TABLE: u32 = 0x04;
    pub const FIXED: u32 = 0x34;
    pub const MOVING: u32 = 0x38;
    pub const FLAGS: u32 = 0x14;
    pub const LO_X: u32 = 0x30;
    pub const HI_X: u32 = 0x34;
    pub const LO_Z: u32 = 0x38;
    pub const HI_Z: u32 = 0x3c;
    pub const EXTENT: u32 = 0x48;
    pub const CELL: u32 = 0x58;
    pub const SCALE: u32 = 0x5c;
    pub const ROOT: u32 = 0x94;
    pub const LIST_SIZE: u32 = 0x30;
    pub const LIST_COUNT: u32 = 0x34;
    pub const LIST_LAST: u32 = 0x38;
    pub const LIST_BLOCKS: u32 = 0x3c;
}

/// The nearby search's fields.
mod search_at {
    pub const FOUND: u32 = 0x00;
    pub const COUNT: u32 = 0x04;
    pub const CENTRE: u32 = 0x10;
    pub const BOUND: u32 = 0x20;
    pub const LEAST: u32 = 0x24;
    pub const EXTRA: u32 = 0x28;
    pub const NO_GROUPS: u32 = 0x2c;
    pub const BUILT: u32 = 0x30;
    pub const CAPACITY: u32 = 0x34;
}

fn plane() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: `./kit state ram` of biplane.p2m2_SaveState.p2s", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

/// A member of a track archive by the leaf its scene spells.
fn by_suffix<'a>(res: &'a Res, want: &str) -> Option<&'a game_sim::archive::Entry> {
    let want = want.to_ascii_lowercase();
    res.entries.iter().find(|e| {
        let name = e.name.to_ascii_lowercase();
        name == want || name.ends_with(&format!("\\{want}"))
    })
}

/// free06's scene props, read the way the shell reads them, and its field's box on the ground plane.
fn free06() -> Option<(Props, ([F; 2], [F; 2]))> {
    let kit = Kit::load().ok()?;
    let at = kit.ctx("iso").join("MX.PSR");
    let Ok(mut file) = std::fs::File::open(&at) else {
        skip(&format!("no {} — `./kit extract MX.PSR`", at.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    std::io::Read::read_exact(&mut file, &mut front).expect("the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let mut member = |name: &str| -> Option<Vec<u8>> {
        let e = psr.find(name)?;
        std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(e.at)).expect("seek");
        let mut bytes = vec![0u8; e.size as usize];
        std::io::Read::read_exact(&mut file, &mut bytes).expect("read");
        Some(bytes)
    };
    let bytes = member(TRACK)?;
    let res = Res::parse(&bytes).ok()?;
    let scn = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(".scn"))?;
    let scene = Scene::parse(&res.read(&bytes, scn).ok()?).ok()?;
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let te = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(&leaf))?;
    let terrain_bytes = res.read(&bytes, te).ok()?;
    let terrain = Terrain::parse(&terrain_bytes).ok()?;
    let ground = Ground::new(&terrain, &scene.terrain);
    let field = ground.field();
    let bounds = ([F(field.min.x()), F(field.min.z())], [F(field.max.x()), F(field.max.z())]);
    let second = member(&TRACK.replace(".res", "_2.res"));
    let mut parts = vec![(bytes.clone(), res)];
    if let Some(b) = second
        && let Ok(r) = Res::parse(&b)
    {
        parts.push((b, r));
    }
    let (props, missing) = Props::load(&scene, |want| {
        parts
            .iter()
            .find_map(|(bytes, res)| by_suffix(res, want).and_then(|e| res.read(bytes, e).ok()))
            .ok_or_else(|| "no archive of this track has it".to_string())
    });
    assert!(missing.is_empty(), "free06's props the archives lack: {missing:?}");
    Some((props, bounds))
}

fn vec4(ee: &Ee, a: u32) -> [F; 4] {
    [0, 4, 8, 12].map(|k| F(ee.f32_at(a + k)))
}

fn set_vec4(ee: &mut Ee, a: u32, v: [F; 4]) {
    for (k, c) in v.iter().enumerate() {
        ee.set_u32(a + 4 * k as u32, c.to_bits());
    }
}

fn bits(v: [F; 4]) -> [u32; 4] {
    v.map(F::to_bits)
}

/// The plane's two probes, its search, and the table they are in.
struct Plane {
    ray: u32,
    sweep: u32,
    search: u32,
    tree: u32,
    list: u32,
}

fn the_plane(ee: &Ee) -> Plane {
    let rec = ee.u32_at(PLANE + PLANE_COLLISION);
    let ray = ee.u32_at(rec + RAY_PROBE);
    let table = ee.u32_at(ee.u32_at(ray + at::SPACE) + tree_at::TABLE);
    Plane {
        ray,
        sweep: ee.u32_at(rec + SWEEP_PROBE),
        search: ee.u32_at(rec + NEARBY),
        tree: ee.u32_at(table + tree_at::FIXED),
        list: ee.u32_at(table + tree_at::MOVING),
    }
}

/// The original's tree: per node (by its path of child indices from the root), its slots in order.
fn tree_slots(ee: &Ee, root: u32) -> Vec<(Vec<u8>, Vec<u32>)> {
    fn walk(ee: &Ee, node: u32, path: &mut Vec<u8>, out: &mut Vec<(Vec<u8>, Vec<u32>)>) {
        let mut slots = Vec::new();
        let mut c = ee.u32_at(node + 4);
        while c != 0 {
            slots.extend((1..=4).map(|k| ee.u32_at(c + 4 * k)));
            c = ee.u32_at(c);
        }
        out.push((path.clone(), slots));
        let children = ee.u32_at(node);
        if children != 0 {
            for i in 0..4u8 {
                let k = ee.u32_at(children + 4 * u32::from(i));
                if k != 0 {
                    path.push(i);
                    walk(ee, k, path, out);
                    path.pop();
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(ee, root, &mut Vec::new(), &mut out);
    out
}

/// The port's tree the same way.
fn port_slots(tree: &Quadtree) -> Vec<(Vec<u8>, Vec<usize>)> {
    fn walk(t: &Quadtree, node: usize, path: &mut Vec<u8>, out: &mut Vec<(Vec<u8>, Vec<usize>)>) {
        let slots = t.nodes[node].chunks.iter().flatten().flatten().copied().collect();
        out.push((path.clone(), slots));
        for (i, c) in t.nodes[node].children.iter().enumerate() {
            if let Some(c) = *c {
                path.push(i as u8);
                walk(t, c, path, out);
                path.pop();
            }
        }
    }
    let mut out = Vec::new();
    walk(tree, tree.root.expect("a root"), &mut Vec::new(), &mut out);
    out
}

fn tree_of(broad: &Broad) -> &Quadtree {
    match &broad.structures[0] {
        Structure::Tree(t) => t,
        Structure::List(_) => panic!("the plane's table's first structure is its tree"),
    }
}

/// Each of the save state's scene meshes (category [`flight_search::SCENERY`]) by its port index: the
/// prop whose centre and reach have its bits.
fn scenery_index(ee: &Ee, broad: &Broad, objects: &[Collidable], tree: u32) -> HashMap<u32, usize> {
    let mut by_place: HashMap<([u32; 4], u32), usize> = HashMap::new();
    for i in FIRST..objects.len() {
        let key = (bits(broad.places[i].centre), objects[i].radius.to_bits());
        assert!(by_place.insert(key, i).is_none(), "two of free06's props stand at {key:?}");
    }
    let mut index = HashMap::new();
    for (_, slots) in tree_slots(ee, ee.u32_at(tree + tree_at::ROOT)) {
        for o in slots {
            if o == 0 || ee.u32_at(o + at::CATEGORY) != flight_search::SCENERY || index.contains_key(&o) {
                continue;
            }
            let key = (bits(vec4(ee, o + at::CENTRE)), ee.u32_at(o + at::RADIUS));
            let i = *by_place.get(&key).unwrap_or_else(|| panic!("the state's mesh {o:#x} at {key:?} is none of the port's props"));
            index.insert(o, i);
        }
    }
    index
}

/// **The tree**: laid over the same box with the same cells, and free06's props in the same nodes in
/// the same order — and what each prop is to the search (flags, category, mask, lists) the state's.
#[test]
fn the_props_tree_is_the_originals() {
    let Some(ee) = plane() else { return };
    let Some((props, field)) = free06() else { return };
    let p = the_plane(&ee);
    let (broad, objects) = flight_search::world(&props.props, Some(field), Vf::new(0.0, 0.0, 0.0, 1.0), F::ZERO);
    let tree = tree_of(&broad);

    // The box: the state's corner is -0.0, the field's +0.0, the same point to every comparison the
    // tree makes.
    let t = p.tree;
    assert_eq!(F(ee.f32_at(t + tree_at::LO_X)), tree.origin[0], "the tree's x origin");
    assert_eq!(F(ee.f32_at(t + tree_at::LO_Z)), tree.origin[1], "the tree's z origin");
    assert_eq!(ee.u32_at(t + tree_at::HI_X), field.1[0].to_bits(), "the tree's far x");
    assert_eq!(ee.u32_at(t + tree_at::HI_Z), field.1[1].to_bits(), "the tree's far z");
    assert_eq!(ee.u32_at(t + tree_at::EXTENT), tree.extent[0].to_bits(), "the tree's x extent");
    assert_eq!(ee.u32_at(t + tree_at::EXTENT + 4), tree.extent[1].to_bits(), "the tree's z extent");
    assert_eq!(ee.i32_at(t + tree_at::CELL), tree.cell, "the smallest node's side");
    assert_eq!(ee.u32_at(t + tree_at::SCALE), tree.scale.to_bits(), "cells per world unit");
    assert_eq!(ee.u32_at(t + tree_at::FLAGS) & 2, tree.flags & 2, "the tree reports its box");
    let global = ee.u32_at(addr("world_tree_global_table").unwrap());
    let table = ee.u32_at(ee.u32_at(p.ray + at::SPACE) + tree_at::TABLE);
    assert!(global == 0 || global == table, "a global table ({global:#x}) other than the plane's");
    assert_eq!(ee.u32_at(ee.u32_at(p.ray + at::SPACE)), 0, "a second space on the ray probe's chain");

    let index = scenery_index(&ee, &broad, &objects, t);
    assert_eq!(index.len(), props.props.len(), "the state's scene meshes and free06's props");
    for (&o, &i) in &index {
        let c = &objects[i];
        assert_eq!(u32::from(ee.u8_at(o + at::CLASS)), 2, "{o:#x}'s class");
        // 0x10 and 0x20 are a search's marks (searched, struck), 0x40 and 0x80 bits the broad phase
        // and the search never read; some meshes have them.
        assert_eq!(ee.u8_at(o + at::FLAGS) & 0x0f, c.flags, "{o:#x}'s flags");
        assert_eq!(ee.u8_at(o + at::NEARBY) & 1 != 0, c.nearby, "{o:#x}'s nearby bit");
        assert_eq!(ee.u32_at(o + at::COLLIDES_WITH), c.collides_with, "{o:#x}'s mask");
        for list in [at::OBJECTS, at::MORE_OBJECTS] {
            assert_eq!(ee.u32_at(o + list + 8), 0, "{o:#x}'s object list at +{list:#x}");
        }
        assert_eq!(ee.u32_at(o + at::CATEGORIES), c.categories.mode, "{o:#x}'s category list's mode");
        assert_eq!(ee.u32_at(o + at::CATEGORIES + 8), 0, "{o:#x}'s category list");
    }

    // Node for node: the props in each, in slot order (the state's other collidables between them).
    let theirs: HashMap<Vec<u8>, Vec<usize>> = tree_slots(&ee, ee.u32_at(t + tree_at::ROOT))
        .into_iter()
        .map(|(path, slots)| (path, slots.into_iter().filter_map(|o| index.get(&o).copied()).collect::<Vec<_>>()))
        .filter(|(_, s)| !s.is_empty())
        .collect();
    let ours: HashMap<Vec<u8>, Vec<usize>> = port_slots(tree).into_iter().filter(|(_, s)| !s.is_empty()).collect();
    let mut paths: Vec<_> = theirs.keys().chain(ours.keys()).cloned().collect();
    paths.sort();
    paths.dedup();
    let mut entries = 0;
    for path in &paths {
        assert_eq!(ours.get(path), theirs.get(path), "the props in node {path:?}");
        entries += ours[path].len();
    }
    eprintln!("{} props in {entries} node entries over {} nodes, all where the original put them", index.len(), paths.len());
}

/// The original's list, as (port index, distance bits), keeping only what `index` names.
fn found_of(ee: &Ee, search: u32, index: &HashMap<u32, usize>) -> (Vec<(usize, u32)>, usize) {
    let n = ee.i32_at(search + search_at::COUNT).max(0) as u32;
    let a = ee.u32_at(search + search_at::FOUND);
    let all: Vec<(u32, u32)> = (0..n).map(|k| (ee.u32_at(a + 8 * k), ee.u32_at(a + 8 * k + 4))).collect();
    (all.iter().filter_map(|&(o, d)| index.get(&o).map(|&i| (i, d))).collect(), all.len())
}

fn ours(search: &Search) -> Vec<(usize, u32)> {
    search.nearby.found.iter().map(|c| (c.object, c.distance.to_bits())).collect()
}

struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * ((self.next() >> 40) as f32 / (1u64 << 24) as f32)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// **The gather**: the original's broad phase on the save state and the port's on its own world, both
/// around the same point near a prop, list the same props in the same order with the same distances —
/// gathered fresh, and then, the probe moved within the bound, kept and measured again.
#[test]
fn the_gather_meets_the_props_in_the_originals_order() {
    let Some(mut ee) = plane() else { return };
    let Some((props, field)) = free06() else { return };
    let p = the_plane(&ee);
    let gather_at = addr("collision_search_gather").unwrap();
    let (mut broad, mut objects) = flight_search::world(&props.props, Some(field), Vf::new(0.0, 0.0, 0.0, 1.0), F::ZERO);
    let index = scenery_index(&ee, &broad, &objects, p.tree);
    let capacity = ee.i32_at(p.search + search_at::CAPACITY);
    assert_eq!(capacity, flight_search::SEARCH_CAPACITY, "the plane's search's capacity");
    assert_eq!(F(ee.f32_at(p.search + search_at::LEAST)), F(flight_search::SEARCH_LEAST), "its least");
    assert_eq!(F(ee.f32_at(p.search + search_at::EXTRA)), F(flight_search::SEARCH_EXTRA), "its extra");
    assert_eq!(ee.u32_at(p.search + search_at::NO_GROUPS), 0, "it gathers groups");

    let mut rng = Pick(0x5960_0000_c4d1_da7e);
    let (mut compared, mut kept, mut full, mut most) = (0, 0, 0, 0);
    for case in 0..CASES {
        let near = FIRST + rng.below(props.props.len() as u64) as usize;
        let c = broad.places[near].centre;
        let reach = objects[near].radius.0 + 40.0;
        let centre = [F(c[0].0 + rng.range(-reach, reach)), F(c[1].0 + rng.range(-30.0, 30.0)), F(c[2].0 + rng.range(-reach, reach)), F::ONE];
        let radius = if rng.below(4) == 0 { rng.range(0.0, 60.0) } else { 14.478_082 };
        let (least, extra) = if rng.below(2) == 0 { (10.0, 30.0) } else { (rng.range(1.0, 60.0), rng.range(0.0, 80.0)) };
        set_vec4(&mut ee, p.ray + at::CENTRE, centre);
        ee.set_f32(p.ray + at::RADIUS, radius);
        ee.set_f32(p.search + search_at::LEAST, least);
        ee.set_f32(p.search + search_at::EXTRA, extra);
        ee.set_u32(p.search + search_at::BUILT, 0);
        let mut search = Search { least: F(least), extra: F(extra), capacity, ..Search::default() };
        broad.places[RAY].centre = centre;
        objects[RAY].radius = F(radius);
        for step in 0..2 {
            if step == 1 {
                // Moved within the bound, which keeps the list.
                let b = search.bound.0 - radius;
                if !(b > 0.5) {
                    break;
                }
                let d = b * 0.5;
                let moved = [F(centre[0].0 + rng.range(-d, d) * 0.57), centre[1], F(centre[2].0 + rng.range(-d, d) * 0.57), F::ONE];
                set_vec4(&mut ee, p.ray + at::CENTRE, moved);
                broad.places[RAY].centre = moved;
            }
            gather(&mut objects, &mut broad, &mut search, RAY, 3);
            ee.call(gather_at, &[u64::from(p.search), u64::from(p.ray), 3], &[], BUDGET);
            let (theirs, all) = found_of(&ee, p.search, &index);
            if step == 0 && all as i32 == capacity {
                full += 1;
                break;
            }
            assert_eq!(ours(&search), theirs, "case {case} step {step}: the props, in order, with their distances");
            assert_eq!(bits(search.centre), bits(vec4(&ee, p.search + search_at::CENTRE)), "case {case}: the centre");
            assert_eq!(search.bound.to_bits(), ee.u32_at(p.search + search_at::BOUND), "case {case}: the bound");
            compared += 1;
            kept += usize::from(step == 1);
            most = most.max(theirs.len());
        }
    }
    eprintln!("{compared} lists compared ({kept} kept and measured again), {full} cases the original's filled, at most {most} props in one");
    assert!(compared > CASES && kept > CASES / 3 && most > 4, "the cases did not reach far enough");
}

/// One of the state's collidables as the search sees it, its object lists in port indices.
fn collidable_of(ee: &Ee, o: u32, index: &HashMap<u32, usize>) -> Collidable {
    let objects = |a: u32| {
        let (arr, n) = (ee.u32_at(o + a + 4), ee.u32_at(o + a + 8));
        Objects { mode: ee.u32_at(o + a), list: (0..n).map(|k| ee.u32_at(arr + 4 * k)).map(|x| index.get(&x).copied().unwrap_or(usize::MAX)).collect() }
    };
    let (arr, n) = (ee.u32_at(o + at::CATEGORIES + 4), ee.u32_at(o + at::CATEGORIES + 8));
    Collidable {
        class: Class::from_byte(ee.u8_at(o + at::CLASS)),
        flags: ee.u8_at(o + at::FLAGS),
        nearby: ee.u8_at(o + at::NEARBY) & 1 != 0,
        radius: F(ee.f32_at(o + at::RADIUS)),
        category: ee.u32_at(o + at::CATEGORY),
        collides_with: ee.u32_at(o + at::COLLIDES_WITH),
        objects: objects(at::OBJECTS),
        more_objects: objects(at::MORE_OBJECTS),
        categories: Categories { mode: ee.u32_at(o + at::CATEGORIES), list: (0..n).map(|k| ee.u32_at(arr + 4 * k)).collect() },
        ..Collidable::default()
    }
}

/// **The save state's 34 candidates**, out of the port's world: its tree of free06's props, and its
/// moving list as the state has it — the port's two probes where the plane's are, and the moving
/// objects the port has no simulation of (the toys, the plane's own body, the pilot, the other bikes,
/// the race gates) put in as stand-ins, with the one bike that has since left the world back in the
/// free slot it was gathered from. Gathered where the state's list was gathered, and measured again
/// where the ray probe is now, it is the state's list: the same 34, in order, and each distance bit for
/// bit but the five whose objects have driven on since.
#[test]
fn the_states_34_candidates_out_of_the_ports_world() {
    let Some(ee) = plane() else { return };
    let Some((props, field)) = free06() else { return };
    let p = the_plane(&ee);
    let n = ee.i32_at(p.search + search_at::COUNT) as u32;
    let found = ee.u32_at(p.search + search_at::FOUND);
    let theirs: Vec<(u32, u32)> = (0..n).map(|k| (ee.u32_at(found + 8 * k), ee.u32_at(found + 8 * k + 4))).collect();
    assert_eq!(theirs.len(), 34, "the state's list");

    let ray_centre = vec4(&ee, p.ray + at::CENTRE);
    let (mut broad, mut objects) =
        flight_search::world(&props.props, Some(field), Vf::new(0.0, 0.0, 0.0, 1.0), F(ee.f32_at(p.ray + at::RADIUS)));
    let mut index = scenery_index(&ee, &broad, &objects, p.tree);
    index.insert(p.ray, RAY);
    index.insert(p.sweep, SWEEP);

    // The moving list's slots, and the candidates no list holds now (in the free slot they left).
    let size = ee.i32_at(p.list + tree_at::LIST_SIZE);
    let last = ee.i32_at(p.list + tree_at::LIST_LAST);
    assert_eq!(size, flight_search::MOVING_BLOCK, "the moving list's block size");
    assert_eq!(ee.i32_at(p.list + tree_at::LIST_COUNT), 1, "the moving list's blocks");
    let block = ee.u32_at(ee.u32_at(p.list + tree_at::LIST_BLOCKS));
    let mut slots: Vec<u32> = (0..last as u32).map(|k| ee.u32_at(block + 4 * k)).collect();
    let gone: Vec<u32> = theirs.iter().map(|c| c.0).filter(|o| !slots.contains(o)).collect();
    assert_eq!(gone.len(), 1, "the candidates no list holds: {gone:x?}");
    assert_eq!(ee.u32_at(gone[0] + at::SPACE), 0, "the one gone has left the world");
    // A free slot holds a link with bit 1 set; the bike was gathered from the first of the two.
    let free: Vec<usize> = (0..slots.len()).filter(|&k| slots[k] & 2 != 0).collect();
    assert_eq!(free.len(), 2, "the moving list's free slots");
    slots[free[0]] = gone[0];

    // Stand-ins: every moving object but the probes, then anything their lists name.
    let mut stand_ins = Vec::new();
    for &o in &slots {
        if o & 2 == 0 && !index.contains_key(&o) {
            index.insert(o, objects.len() + stand_ins.len());
            stand_ins.push(o);
        }
    }
    for &o in &stand_ins {
        objects.push(collidable_of(&ee, o, &index));
        broad.places.push(game_sim::broad_phase::Placement { centre: vec4(&ee, o + at::CENTRE), ..Default::default() });
    }
    // The ray probe's lists name the plane's body and the pilot, which are in the list now.
    let ray = collidable_of(&ee, p.ray, &index);
    assert_eq!((ray.flags, ray.category, ray.collides_with), (objects[RAY].flags, objects[RAY].category, objects[RAY].collides_with), "the ray probe");
    objects[RAY].objects = ray.objects;
    let mut block: Vec<Option<usize>> = slots.iter().map(|o| if o & 2 == 0 { index.get(o).copied() } else { None }).collect();
    block.resize(size as usize, None);
    broad.structures[1] = Structure::List(Blocks { size, last, blocks: Some(vec![block]), ..Blocks::default() });

    let mut search = flight_search::search();
    broad.places[RAY].centre = vec4(&ee, p.search + search_at::CENTRE);
    gather(&mut objects, &mut broad, &mut search, RAY, 3);
    assert!(search.nearby.found.iter().all(|c| c.object >= FIRST + props.props.len()), "a scene mesh within the bound");
    assert_eq!(search.bound.to_bits(), ee.u32_at(p.search + search_at::BOUND), "the bound");
    broad.places[RAY].centre = ray_centre;
    gather(&mut objects, &mut broad, &mut search, RAY, 3);
    let addr_of: HashMap<usize, u32> = index.iter().map(|(&a, &i)| (i, a)).collect();
    let got: Vec<(u32, u32)> = search.nearby.found.iter().map(|c: &Candidate| (addr_of[&c.object], c.distance.to_bits())).collect();
    let order = |l: &[(u32, u32)]| l.iter().map(|c| c.0).collect::<Vec<_>>();
    assert_eq!(order(&got), order(&theirs), "the state's 34 candidates, in order");
    // The distances: bit for bit, but for the five that drive (two of the toys and the three bikes),
    // which moved after the plane last measured them — the state holds where they are now.
    let apart: Vec<usize> = (0..got.len()).filter(|&k| got[k].1 != theirs[k].1).collect();
    assert_eq!(apart, [21, 22, 23, 24, 25], "the candidates whose distance is not the state's");
    for &k in &apart {
        let (a, b) = (f32::from_bits(got[k].1), f32::from_bits(theirs[k].1));
        assert!((a - b).abs() < 0.01 * b, "candidate {k}: {a} here, {b} in the state");
    }
}


/// The second biplane state (S99): biplane.p2m2's, six ticks on, saved while a hook at the plane's
/// search refresh copied every moving-list object's centre into spare RAM just before the gather.
const LATER: &str = "biplane_later.eeMemory.bin";
/// That copy: a count, then eight records of a tick, the list's length, each slot's centre (16 bytes
/// a slot) and each slot's word.
const COPIES: u32 = 0x01fc_0000;
const TICK: u32 = 0x0047_9d04;

fn later() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("states").join(LATER);
    if !path.exists() {
        skip(&format!("{} is not there: `./kit state ram` of context/recordings/biplane_later.p2s (S99)", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

/// The state's list, gathered by the port out of its own world plus the state's moving objects (as
/// in the 34), each moving object at `centre_of(its slot, it)` — no slot for a candidate that has left
/// the list: (object, distance bits) for the port's list and the state's.
fn the_list_with(ee: &Ee, props: &Props, field: ([F; 2], [F; 2]), centre_of: impl Fn(Option<usize>, u32) -> [F; 4]) -> (Vec<(u32, u32)>, Vec<(u32, u32)>) {
    let p = the_plane(ee);
    let n = ee.i32_at(p.search + search_at::COUNT) as u32;
    let found = ee.u32_at(p.search + search_at::FOUND);
    let theirs: Vec<(u32, u32)> = (0..n).map(|k| (ee.u32_at(found + 8 * k), ee.u32_at(found + 8 * k + 4))).collect();

    let (mut broad, mut objects) =
        flight_search::world(&props.props, Some(field), Vf::new(0.0, 0.0, 0.0, 1.0), F(ee.f32_at(p.ray + at::RADIUS)));
    let mut index = scenery_index(ee, &broad, &objects, p.tree);
    index.insert(p.ray, RAY);
    index.insert(p.sweep, SWEEP);

    let size = ee.i32_at(p.list + tree_at::LIST_SIZE);
    let last = ee.i32_at(p.list + tree_at::LIST_LAST);
    assert_eq!(ee.i32_at(p.list + tree_at::LIST_COUNT), 1, "the moving list's blocks");
    let block = ee.u32_at(ee.u32_at(p.list + tree_at::LIST_BLOCKS));
    let mut slots: Vec<u32> = (0..last as u32).map(|k| ee.u32_at(block + 4 * k)).collect();
    // A candidate no list holds has left the world; it was gathered from the first free slot.
    let gone: Vec<u32> = theirs.iter().map(|c| c.0).filter(|o| !slots.contains(o)).collect();
    assert!(gone.len() <= 1, "the candidates no list holds: {gone:x?}");
    let mut gone_slot = None;
    if let Some(&g) = gone.first() {
        assert_eq!(ee.u32_at(g + at::SPACE), 0, "the one gone has left the world");
        let k = (0..slots.len()).find(|&k| slots[k] & 2 != 0).expect("a free slot");
        slots[k] = g;
        gone_slot = Some(k);
    }

    let mut stand_ins = Vec::new();
    for (k, &o) in slots.iter().enumerate() {
        if o & 2 == 0 && !index.contains_key(&o) {
            index.insert(o, objects.len() + stand_ins.len());
            stand_ins.push((k, o));
        }
    }
    for &(k, o) in &stand_ins {
        objects.push(collidable_of(ee, o, &index));
        let centre = centre_of(if Some(k) == gone_slot { None } else { Some(k) }, o);
        broad.places.push(game_sim::broad_phase::Placement { centre, ..Default::default() });
    }
    let ray = collidable_of(ee, p.ray, &index);
    objects[RAY].objects = ray.objects;
    let mut block: Vec<Option<usize>> = slots.iter().map(|o| if o & 2 == 0 { index.get(o).copied() } else { None }).collect();
    block.resize(size as usize, None);
    broad.structures[1] = Structure::List(Blocks { size, last, blocks: Some(vec![block]), ..Blocks::default() });

    // Built where the state's list was built, then kept and measured again where the ray probe is.
    let mut search = flight_search::search();
    broad.places[RAY].centre = vec4(ee, p.search + search_at::CENTRE);
    gather(&mut objects, &mut broad, &mut search, RAY, 3);
    assert_eq!(search.bound.to_bits(), ee.u32_at(p.search + search_at::BOUND), "the bound");
    broad.places[RAY].centre = vec4(ee, p.ray + at::CENTRE);
    gather(&mut objects, &mut broad, &mut search, RAY, 3);
    let addr_of: HashMap<usize, u32> = index.iter().map(|(&a, &i)| (i, a)).collect();
    let got = search.nearby.found.iter().map(|c: &Candidate| (addr_of[&c.object], c.distance.to_bits())).collect();
    (got, theirs)
}

/// **When the list is gathered** (S99): once a tick, in the plane's dynamics-world post listener —
/// after every contact search and the substeps, before the other bodies' posts — around the ray
/// probe's reach at the stepped pose. A second state, six ticks on from the first, holds the centres
/// the moving objects (and the plane's candidates) had at that moment: with them, the port's gather is the state's list, all 34,
/// in order, every distance bit for bit. With the centres the state holds (the toys and bikes have
/// posted and moved since), exactly the four that moved are apart — S96's five stale distances.
#[test]
fn the_plane_gathers_in_its_post_before_the_others_move() {
    let Some(ee) = later() else { return };
    let Some((props, field)) = free06() else { return };
    let copies = ee.u32_at(COPIES);
    assert!(copies > 0, "the state's copies of the moving centres");
    let copy = COPIES + 0x100 + ((copies - 1) & 7) * 0x1000;
    // The save fell between this tick's gather and the next: the last copy is of this tick.
    assert_eq!(ee.u32_at(copy), ee.u32_at(TICK), "the last copy's tick");
    let p = the_plane(&ee);
    let last = ee.u32_at(p.list + tree_at::LIST_LAST);
    assert_eq!(ee.u32_at(copy + 4), last, "the copied list's length");
    let block = ee.u32_at(ee.u32_at(p.list + tree_at::LIST_BLOCKS));
    for k in 0..last {
        assert_eq!(ee.u32_at(copy + 0x810 + 4 * k), ee.u32_at(block + 4 * k), "the copied list's slot {k}");
    }
    // The gather is built where the search's centre is and is kept; it has not moved since the first state.
    let first = plane();
    if let Some(first) = &first {
        let q = the_plane(first);
        assert_eq!(bits(vec4(first, q.search + search_at::CENTRE)), bits(vec4(&ee, p.search + search_at::CENTRE)), "the build centre");
    }

    // A candidate out of the list (a bike that has left the world and still drives) by its place in
    // the plane's list, copied too.
    let theirs_at = |o: u32| (0..ee.u32_at(copy + 0xa00)).find(|&i| ee.u32_at(ee.u32_at(p.search + search_at::FOUND) + 8 * i) == o);
    let (got, theirs) = the_list_with(&ee, &props, field, |k, o| match k {
        Some(k) => vec4(&ee, copy + 0x10 + 16 * k as u32),
        None => vec4(&ee, copy + 0xa10 + 16 * theirs_at(o).expect("the gone one in the plane's list")),
    });
    assert_eq!(theirs.len(), 34, "the state's list");
    assert_eq!(ee.u32_at(copy + 0xa00), 34, "the copied list's candidates");
    let order = |l: &[(u32, u32)]| l.iter().map(|c| c.0).collect::<Vec<_>>();
    assert_eq!(order(&got), order(&theirs), "the state's 34, in order");
    let off: Vec<(usize, u32, f32, f32)> = (0..got.len())
        .filter(|&k| got[k].1 != theirs[k].1)
        .map(|k| (k, got[k].0, f32::from_bits(got[k].1), f32::from_bits(theirs[k].1)))
        .collect();
    assert_eq!(off, [], "every distance, bit for bit, at the centres of the gather");

    // The centres the state holds: apart are exactly the objects that moved after the gather.
    let moved: Vec<usize> =
        (0..theirs.len()).filter(|&i| bits(vec4(&ee, copy + 0xa10 + 16 * i as u32)) != bits(vec4(&ee, theirs[i].0 + at::CENTRE))).collect();
    let (now, _) = the_list_with(&ee, &props, field, |_, o| vec4(&ee, o + at::CENTRE));
    assert_eq!(order(&now), order(&theirs), "the order does not hang on the centres");
    let apart: Vec<usize> = (0..now.len()).filter(|&k| now[k].1 != theirs[k].1).collect();
    assert_eq!(apart, moved, "the candidates apart at the state's centres are the ones that moved");
    assert_eq!(apart.len(), 5, "S96's five: they moved after the plane's gather");
}

// --- the trees (S102) --------------------------------------------------------------------------------

/// A placed group's fields: its cell's three shorts, its species slot, its two size bytes.
mod group_at {
    pub const CELL: u32 = 0x08;
    pub const MODEL: u32 = 0x0e;
    pub const HEIGHT: u32 = 0x10;
    pub const WIDTH: u32 = 0x11;
}

/// free06's ecosystem from its archive, stood on its ground as the loader stands it, and its trunks
/// built on the state's own curve table.
fn free06_trees(ee: &Ee) -> Option<game_sim::eco::Trees> {
    let kit = Kit::load().ok()?;
    let mut file = std::fs::File::open(kit.ctx("iso").join("MX.PSR")).ok()?;
    let mut front = vec![0u8; 64 << 10];
    std::io::Read::read_exact(&mut file, &mut front).expect("the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find(TRACK)?;
    std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    std::io::Read::read_exact(&mut file, &mut bytes).expect("read");
    let res = Res::parse(&bytes).ok()?;
    let read = |suffix: &str| {
        let e = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(suffix)).unwrap_or_else(|| panic!("free06 has no {suffix}"));
        res.read(&bytes, e).expect("a free06 member")
    };
    let scene = Scene::parse(&read(".scn")).expect("free06's scene");
    let terrain_bytes = read(&scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn"));
    let terrain = Terrain::parse(&terrain_bytes).expect("free06's terrain");
    let mut eco = game_sim::eco::Ecosystem::parse(&read(".esb")).expect("free06.esb parses");
    let layout = &scene.terrain;
    let extent = F::from(layout.width.max(layout.breadth) as i32 * game_sim::world::SAMPLES_PER_WIDTH as i32) * terrain.scale;
    let ground = Ground::new(&terrain, layout);
    let field = ground.field();
    eco.stand(extent, |p| F(game_sim::contact_search::ground(&field, p).point.y()));
    let table = ee.u32_at(addr("curve_table_ptr").unwrap());
    let entries: Vec<[f32; 2]> = (0..game_sim::camera::CURVE_ENTRIES as u32)
        .map(|i| [f32::from_bits(ee.u32_at(table + 8 * i)), f32::from_bits(ee.u32_at(table + 8 * i + 4))])
        .collect();
    let curve = game_sim::camera::CurveTable::from_entries(&entries);
    let (trees, missed) = game_sim::eco::Trees::new(&eco, game_sim::eco::cell_size(extent), &curve);
    assert!(missed.is_empty(), "free06's solids the port does not build: {missed:?}");
    Some(trees)
}

/// **The trees in the tree** (S102): free06's plants, put in the port's tree as placed groups over
/// its props ([`flight_search::world_wooded`]), sit in the save state's tree node for node and, within
/// each node, in the same order among the props; each is at the state's cell (its height the one the
/// loader stood it at) with the state's size bytes.
#[test]
fn the_plants_are_in_the_props_tree_where_the_original_put_them() {
    let Some(ee) = plane() else { return };
    let Some((props, field)) = free06() else { return };
    let Some(trees) = free06_trees(&ee) else { return };
    let p = the_plane(&ee);
    let (broad, objects) = flight_search::world_wooded(&props.props, Some(&trees), None, Some(field), Vf::new(0.0, 0.0, 0.0, 1.0), F::ZERO);
    let tree = tree_of(&broad);
    let first_group = objects.iter().position(|c| c.class == Class::Group).expect("free06 has plants");
    assert_eq!(first_group, FIRST + props.props.len(), "the groups follow the props");

    // The props as before (their own collidables sit in front of the groups).
    let mut index = scenery_index(&ee, &broad, &objects[..first_group], p.tree);
    assert_eq!(index.len(), props.props.len(), "the state's scene meshes and free06's props");

    // The state's groups by their cell on the ground plane and their size bytes.
    let mut by_place: HashMap<(u16, u16, i8, i8), Vec<usize>> = HashMap::new();
    for (i, g) in trees.groups().iter().enumerate() {
        by_place.entry((g.cell[0], g.cell[2], g.height, g.blend)).or_default().push(i);
    }
    let (mut groups, mut stood, mut off) = (0, 0, Vec::new());
    for (_, slots) in tree_slots(&ee, ee.u32_at(p.tree + tree_at::ROOT)) {
        for o in slots {
            if o == 0 || u32::from(ee.u8_at(o + at::CLASS)) != 3 || index.contains_key(&o) {
                continue;
            }
            let cell = [0, 2, 4].map(|k| ee.u16_at(o + group_at::CELL + k));
            let key = (cell[0], cell[2], ee.u8_at(o + group_at::HEIGHT) as i8, ee.u8_at(o + group_at::WIDTH) as i8);
            let found = by_place.get(&key).unwrap_or_else(|| panic!("the state's group {o:#x} at {key:?} is none of the port's plants"));
            // Two plants on one spot of one size: take the first not yet named.
            let taken: std::collections::HashSet<usize> = index.values().copied().collect();
            let g = *found.iter().find(|&&g| !taken.contains(&(first_group + g))).unwrap_or_else(|| panic!("{o:#x}: every plant at {key:?} is taken"));
            let ours = &trees.groups()[g];
            if ours.cell[1] != cell[1] {
                off.push((o, cell[1], ours.cell[1]));
            }
            assert_eq!(u32::from(ee.u8_at(o + group_at::MODEL)) + 1, u32::from(ours.model), "{o:#x}'s species (game slot n - 1)");
            stood += usize::from(ours.cell[1] == cell[1]);
            index.insert(o, first_group + g);
            groups += 1;
        }
    }
    assert!(off.is_empty(), "{} of {groups} plants at another height than the state's, e.g. {:?}", off.len(), &off[..off.len().min(8)]);
    assert_eq!(groups, trees.groups().len(), "the state's groups and free06's plants");

    let theirs: HashMap<Vec<u8>, Vec<usize>> = tree_slots(&ee, ee.u32_at(p.tree + tree_at::ROOT))
        .into_iter()
        .map(|(path, slots)| (path, slots.into_iter().filter_map(|o| index.get(&o).copied()).collect::<Vec<_>>()))
        .filter(|(_, s)| !s.is_empty())
        .collect();
    let ours: HashMap<Vec<u8>, Vec<usize>> = port_slots(tree).into_iter().filter(|(_, s)| !s.is_empty()).collect();
    let mut paths: Vec<_> = theirs.keys().chain(ours.keys()).cloned().collect();
    paths.sort();
    paths.dedup();
    let mut entries = 0;
    for path in &paths {
        assert_eq!(ours.get(path), theirs.get(path), "the props and plants in node {path:?}");
        entries += ours[path].len();
    }
    eprintln!("{groups} plants ({stood} at the state's height) and {} props in {entries} node entries over {} nodes", props.props.len(), paths.len());
}
