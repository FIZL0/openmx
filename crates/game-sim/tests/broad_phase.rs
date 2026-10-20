//! S45 — the broad phase against the original.
//!
//! On the save state beside C7's recording, on the whole world as it stands there: the quadtree of the
//! static collidables (7107 nodes, 6606 collidables) and the block list of the moving ones (120), with
//! the plane's two probes and its own nearby search. The world is read into a [`Broad`] once and both
//! sides then run on, case after case, from the state the last case left: the stamps, the tree's open
//! query, the lists' cursors, the flags.
//!
//! Each case shakes what a query walks before it runs: the probe's table's moving structure and its
//! sub-tables, a second space on the probe's chain, the global table — built out of the tree, the
//! moving list and a third structure the test makes, a small block list of the moving collidables with
//! full blocks and free slots (the save state's own list has one block, so the path for full blocks would
//! go unwalked). Now and then the tree is left busy, the stamp is put by its wrap, flags 0x04 and 0x08
//! are flipped on some collidables. Then:
//!
//! - `the_gather_matches_the_original`: `collision_search_gather` on the plane's search with its ray
//!   probe (the call the plane's refresh makes) — the probe moved a little, not at all, about the plane
//!   or anywhere in the world; the search's fields rewritten or left. Compared: the list (collidables and
//!   distances, bit-exact), the centre, the bound, the built flag; the tree's open query (flags, busy,
//!   cells, the nodes gathered, where it is), both lists' cursors, the stamp, the tree last queried; every
//!   collidable's stamp and flags.
//! - `probe_collide_world_matches_the_original`: `probe_collide_world` with one of the two probes, whose
//!   class vtable is swapped for one whose clear, narrow phase and box are hooks; a group's two part
//!   functions, the listeners and the callbacks are hooks too, as in S44. Compared: the return, every call
//!   out in order, the probe's budget and last centre, every collidable's flags, group and stamp, and the
//!   structures as above.

use game_sim::broad_phase::{
    Blocks, Broad, Model, Node, Pair, Placement, Probe, Quadtree, Search, Space, Structure, Tables,
    collidable_distance, gather, more_flags, probe_collide_world,
};
use game_sim::world_search::{
    Candidate, Categories, Class, Collidable, Narrow, Nearby, Objects, accepts, flags,
};
use ps2_float::F;
use ps2_oracle::{Ee, Kit, addr, skip};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

const STATE: &str = "biplane.p2m2_SaveState.eeMemory.bin";
const PLANE: u32 = 0x00c4_4a40;
const PLANE_COLLISION: u32 = 0x170;
const CASES: usize = 10_000;
const BUDGET: u64 = 50_000_000;

/// The collision object's fields: its nearby search and its two probes.
const NEARBY: u32 = 0x10c;
const RAY_PROBE: u32 = 0x1f0;
const SWEEP_PROBE: u32 = 0x1f4;

/// A collidable's fields.
mod at {
    pub const STAMP: u32 = 0x00;
    pub const CLASS: u32 = 0x02;
    pub const SPACE: u32 = 0x04;
    pub const CELL_X: u32 = 0x08;
    pub const CELL_Z: u32 = 0x0c;
    pub const MODEL: u32 = 0x0e;
    pub const BLEND: u32 = 0x11;
    pub const NEARBY: u32 = 0x31;
    pub const CLASS_VTABLE: u32 = 0x38;
    pub const CENTRE: u32 = 0x40;
    pub const LAST: u32 = 0x50;
    pub const RADIUS: u32 = 0x60;
    pub const BUDGET: u32 = 0x64;
    pub const GROUP: u32 = 0x78;
    pub const CATEGORY: u32 = 0x84;
    pub const COLLIDES_WITH: u32 = 0xcc;
    pub const OBJECTS: u32 = 0xd0;
    pub const MORE_OBJECTS: u32 = 0xdc;
    pub const CATEGORIES: u32 = 0xe8;
    pub const LISTENERS: u32 = 0xf4;
    pub const ON_HIT: u32 = 0x10c;
    pub const ON_STRUCK: u32 = 0x110;
    pub const FLAGS: u32 = 0x116;
}

/// A quadtree's fields.
mod tree_at {
    pub const FLAGS: u32 = 0x14;
    pub const ORIGIN_X: u32 = 0x30;
    pub const ORIGIN_Z: u32 = 0x38;
    pub const EXTENT: u32 = 0x48;
    pub const CELL: u32 = 0x58;
    pub const SCALE: u32 = 0x5c;
    pub const ROOT: u32 = 0x94;
    /// The pool the links of its list come from.
    pub const LINK_POOL: u32 = 0x9c;
    pub const HEAD: u32 = 0xa4;
    pub const AT: u32 = 0xa8;
    pub const SLOT: u32 = 0xac;
    pub const CHUNK: u32 = 0xb0;
    pub const BUSY: u32 = 0x898;
    pub const CELLS: u32 = 0x8a8;
}

/// A block list's fields.
mod list_at {
    pub const FLAGS: u32 = 0x14;
    pub const SIZE: u32 = 0x30;
    pub const COUNT: u32 = 0x34;
    pub const LAST: u32 = 0x38;
    pub const ARRAY: u32 = 0x3c;
    pub const BLOCK: u32 = 0x44;
    pub const ENTRY: u32 = 0x48;
}

/// A table's fields, and a space link's.
mod table_at {
    pub const FIXED: u32 = 0x34;
    pub const MOVING: u32 = 0x38;
    pub const SUBS: u32 = 0x44;
    pub const SUB_COUNT: u32 = 0x48;
    pub const NEXT: u32 = 0x00;
    pub const TABLE: u32 = 0x04;
}

/// The search's fields.
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

/// The probe class's vtable slots the direct probe calls.
mod slot {
    pub const CLEAR: u32 = 0x90;
    pub const PAIR: u32 = 0x94;
    pub const BOUNDS: u32 = 0xa4;
}

/// Where the hooks live: addresses no code reaches, each one a function the test stands in.
mod hook {
    pub const CLEAR: u32 = 0x2000;
    pub const BOUNDS: u32 = 0x2010;
    pub const PAIR: u32 = 0x2020;
    pub const HEARD_HIT: u32 = 0x2030;
    pub const HEARD_STRUCK: u32 = 0x2040;
    pub const ON_HIT: [u32; 2] = [0x2050, 0x2060];
    pub const ON_STRUCK: [u32; 2] = [0x2070, 0x2080];
}

/// Listener objects, shared out among the collidables the direct probe can strike.
const LISTENERS: usize = 6;
/// How many of the plane's gathered collidables get listeners and callbacks, and may be a group's part.
const POOL: usize = 12;
/// The test's own block list: blocks of this many slots, at most this many blocks.
const L2_SIZE: u32 = 4;
const L2_BLOCKS: u32 = 3;
/// A quadtree's header, up to the end of the open query's cells.
const TREE_HEADER: u32 = 0x8c0;
/// The size of one of a link pool's blocks.
const POOL_BLOCK: u32 = 0x1000;

fn plane() -> Option<Ee> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: C7's save state, `./kit state ram` of biplane.p2m2_SaveState.p2s", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

/// A deterministic generator for the cases.
struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    fn one_in(&mut self, n: u64) -> bool {
        self.next() % n == 0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Where everything is, on the original's side, and which index each address has on the port's.
struct World {
    /// Collidables: the two probes first, then the tree's, the moving list's, and what the plane's search
    /// holds.
    addrs: Vec<u32>,
    index: HashMap<u32, usize>,
    /// The tree, the save state's moving list, the test's own list, the test's second tree.
    structures: [u32; 4],
    nodes: Vec<u32>,
    node_index: HashMap<u32, usize>,
    chunks: HashMap<u32, (usize, usize)>,
    /// Each probe's first space (both name one table), and that table.
    spaces: [u32; 2],
    table: u32,
    /// The test's tables: one on a second space, two sub-tables, one to be global; the second space's
    /// link, the sub-table array, the test list's block array.
    tables: [u32; 4],
    link: u32,
    subs: u32,
    blocks: u32,
    search: u32,
    /// Class-2 collidables, by index; the moving ones' addresses.
    class2: Vec<usize>,
    movers: Vec<u32>,
    /// For the direct probe: the listener objects and their vtable, the probe vtable, each pool
    /// collidable's listener array.
    listeners: Vec<u32>,
    listener_vtable: u32,
    probe_vtable: u32,
    pool: Vec<usize>,
    pool_lists: Vec<u32>,
}

impl World {
    /// The port's view of an address: a collidable's index, or something no collidable is.
    fn index_of(&self, a: u32) -> usize {
        self.index.get(&a).copied().unwrap_or(1_000_000 + a as usize)
    }
    fn addr_of(&self, i: usize) -> u32 {
        if i >= 1_000_000 { (i - 1_000_000) as u32 } else { self.addrs[i] }
    }
    fn structure(&self, a: u32) -> Option<usize> {
        if a == 0 {
            return None;
        }
        Some(self.structures.iter().position(|&s| s == a).unwrap_or_else(|| panic!("structure {a:#x} unknown")))
    }
}

fn copy(ee: &mut Ee, from: u32, to: u32, n: u32) {
    for k in (0..n).step_by(4) {
        let v = ee.u32_at(from + k);
        ee.set_u32(to + k, v);
    }
}

fn collidable_of(ee: &Ee, w: &World, o: u32) -> Collidable {
    let class = Class::from_byte(ee.u8_at(o + at::CLASS));
    if class != Class::Object {
        // Only the class of a placed group or a non-collidable is read; the rest is not this layout.
        return Collidable { class, ..Collidable::default() };
    }
    let list = |base: u32| -> (u32, Vec<u32>) {
        let n = ee.i32_at(base + 8).max(0) as u32;
        let a = ee.u32_at(base + 4);
        (ee.u32_at(base), (0..n).map(|i| ee.u32_at(a + 4 * i)).collect())
    };
    let (mode, l) = list(o + at::OBJECTS);
    let objects = Objects { mode, list: l.iter().map(|&a| w.index_of(a)).collect() };
    let (mode, l) = list(o + at::MORE_OBJECTS);
    let more_objects = Objects { mode, list: l.iter().map(|&a| w.index_of(a)).collect() };
    let (mode, list) = list(o + at::CATEGORIES);
    let n = ee.u32_at(o + at::LISTENERS);
    let la = ee.u32_at(o + at::LISTENERS + 8);
    let some = |a: u32| (a != 0).then_some(a as usize);
    let group = ee.u32_at(o + at::GROUP);
    Collidable {
        class,
        flags: ee.u8_at(o + at::FLAGS),
        nearby: ee.u8_at(o + at::NEARBY) & 1 != 0,
        radius: F(ee.f32_at(o + at::RADIUS)),
        group: (group != 0).then(|| w.index_of(group)),
        category: ee.u32_at(o + at::CATEGORY),
        collides_with: ee.u32_at(o + at::COLLIDES_WITH),
        objects,
        more_objects,
        categories: Categories { mode, list },
        listeners: (0..n).map(|k| ee.u32_at(la + 4 * k) as usize).collect(),
        on_hit: some(ee.u32_at(o + at::ON_HIT)),
        on_struck: some(ee.u32_at(o + at::ON_STRUCK)),
    }
}

fn vec4(ee: &Ee, a: u32) -> [F; 4] {
    [0, 4, 8, 12].map(|k| F(ee.f32_at(a + k)))
}

fn set_vec4(ee: &mut Ee, a: u32, v: [F; 4]) {
    for (k, x) in v.iter().enumerate() {
        ee.set_u32(a + 4 * k as u32, x.to_bits());
    }
}

fn placement_of(ee: &Ee, o: u32, space: Option<usize>) -> Placement {
    Placement {
        stamp: ee.u16_at(o + at::STAMP),
        space,
        centre: vec4(ee, o + at::CENTRE),
        last: vec4(ee, o + at::LAST),
        budget: F(ee.f32_at(o + at::BUDGET)),
        cell: [ee.u16_at(o + at::CELL_X), ee.u16_at(o + at::CELL_Z)],
        model: ee.u8_at(o + at::MODEL),
        blend: ee.u8_at(o + at::BLEND) as i8,
    }
}

/// The tree's open query, as the original keeps it: the links from `+0xa4` (each the node it names),
/// which of them `+0xa8` is, the chunk and the slot.
#[derive(Debug, PartialEq)]
struct TreeQuery {
    flags: u32,
    busy: u32,
    cells: [i32; 4],
    list: Vec<usize>,
    at: Option<usize>,
    chunk: Option<(usize, usize)>,
    slot: usize,
}

fn tree_query_of(ee: &Ee, w: &World, t: u32) -> TreeQuery {
    let mut links = Vec::new();
    let mut l = ee.u32_at(t + tree_at::HEAD);
    while l != 0 {
        links.push((l, w.node_index[&ee.u32_at(l + 4)]));
        l = ee.u32_at(l);
        assert!(links.len() <= w.nodes.len(), "the tree's list does not end");
    }
    let at = ee.u32_at(t + tree_at::AT);
    let chunk = ee.u32_at(t + tree_at::CHUNK);
    TreeQuery {
        flags: ee.u32_at(t + tree_at::FLAGS),
        busy: ee.u32_at(t + tree_at::BUSY),
        cells: [0, 4, 8, 12].map(|k| ee.i32_at(t + tree_at::CELLS + k)),
        list: links.iter().map(|&(_, n)| n).collect(),
        at: (at != 0).then(|| links.iter().position(|&(l, _)| l == at).expect("+0xa8 on the list")),
        chunk: (chunk != 0).then(|| w.chunks[&chunk]),
        slot: ee.i32_at(t + tree_at::SLOT) as usize,
    }
}

fn port_tree_query(t: &Quadtree) -> TreeQuery {
    TreeQuery {
        flags: t.flags,
        busy: t.busy,
        cells: t.cells,
        list: if t.head.is_some() { t.list.clone() } else { Vec::new() },
        at: t.at,
        chunk: t.chunk,
        slot: t.slot,
    }
}

fn tree_of(ee: &Ee, w: &World, t: u32) -> Quadtree {
    let nodes = w
        .nodes
        .iter()
        .map(|&n| {
            let arr = ee.u32_at(n);
            let children = [0, 1, 2, 3].map(|i| {
                let c = if arr == 0 { 0 } else { ee.u32_at(arr + 4 * i) };
                (c != 0).then(|| w.node_index[&c])
            });
            let mut chunks = Vec::new();
            let mut c = ee.u32_at(n + 4);
            while c != 0 {
                chunks.push([1, 2, 3, 4].map(|k| {
                    let o = ee.u32_at(c + 4 * k);
                    (o != 0).then(|| w.index[&o])
                }));
                c = ee.u32_at(c);
            }
            Node { children, chunks }
        })
        .collect();
    let q = tree_query_of(ee, w, t);
    Quadtree {
        flags: q.flags,
        origin: [F(ee.f32_at(t + tree_at::ORIGIN_X)), F(ee.f32_at(t + tree_at::ORIGIN_Z))],
        extent: [F(ee.f32_at(t + tree_at::EXTENT)), F(ee.f32_at(t + tree_at::EXTENT + 4))],
        cell: ee.i32_at(t + tree_at::CELL),
        scale: F(ee.f32_at(t + tree_at::SCALE)),
        root: Some(0),
        nodes,
        busy: q.busy,
        cells: q.cells,
        head: (!q.list.is_empty()).then_some(0),
        list: q.list,
        at: q.at,
        chunk: q.chunk,
        slot: q.slot,
    }
}

fn blocks_of(ee: &Ee, w: &World, l: u32) -> Blocks {
    let size = ee.i32_at(l + list_at::SIZE);
    let last = ee.i32_at(l + list_at::LAST);
    let n = ee.i32_at(l + list_at::COUNT);
    let arr = ee.u32_at(l + list_at::ARRAY);
    let blocks = (arr != 0).then(|| {
        (0..n)
            .map(|b| {
                let block = ee.u32_at(arr + 4 * b as u32);
                let used = if b == n - 1 { last } else { size };
                (0..size)
                    .map(|k| {
                        let o = ee.u32_at(block + 4 * k as u32);
                        (k < used && o & 2 == 0).then(|| w.index[&o])
                    })
                    .collect()
            })
            .collect()
    });
    Blocks {
        flags: ee.u32_at(l + list_at::FLAGS),
        size,
        last,
        blocks,
        block: ee.i32_at(l + list_at::BLOCK),
        entry: ee.i32_at(l + list_at::ENTRY),
    }
}

/// The spaces, the tables and the global table, as the original's RAM has them now.
fn topology(ee: &Ee, w: &World, broad: &mut Broad) {
    let mut tables: Vec<u32> = Vec::new();
    let mut table = |t: u32| match tables.iter().position(|&x| x == t) {
        Some(i) => i,
        None => {
            tables.push(t);
            tables.len() - 1
        }
    };
    let mut links: Vec<u32> = Vec::new();
    for &head in &w.spaces {
        let mut s = head;
        while s != 0 && !links.contains(&s) {
            links.push(s);
            s = ee.u32_at(s + table_at::NEXT);
        }
    }
    let link = |s: u32| (s != 0).then(|| links.iter().position(|&x| x == s).unwrap());
    broad.spaces = links
        .iter()
        .map(|&s| Space { next: link(ee.u32_at(s + table_at::NEXT)), tables: table(ee.u32_at(s + table_at::TABLE)) })
        .collect();
    for (i, &head) in w.spaces.iter().enumerate() {
        broad.places[i].space = link(head);
    }
    let global = ee.u32_at(addr("world_tree_global_table").unwrap());
    let global = (global != 0).then(|| table(global));
    let pair = |t: u32| Pair {
        fixed: w.structure(ee.u32_at(t + table_at::FIXED)),
        moving: w.structure(ee.u32_at(t + table_at::MOVING)),
    };
    broad.tables = tables
        .iter()
        .map(|&t| {
            let subs = ee.u32_at(t + table_at::SUBS);
            Tables {
                own: pair(t),
                subs: (0..ee.i32_at(t + table_at::SUB_COUNT).max(0) as u32)
                    .map(|i| pair(ee.u32_at(subs + 4 * i)))
                    .collect(),
            }
        })
        .collect();
    broad.global = global;
}

fn world(ee: &mut Ee) -> (World, Broad, Vec<Collidable>) {
    let rec = ee.u32_at(PLANE + PLANE_COLLISION);
    let probes = [ee.u32_at(rec + RAY_PROBE), ee.u32_at(rec + SWEEP_PROBE)];
    let search = ee.u32_at(rec + NEARBY);
    let spaces = probes.map(|o| ee.u32_at(o + at::SPACE));
    let table = ee.u32_at(spaces[0] + table_at::TABLE);
    assert_eq!(ee.u32_at(spaces[1] + table_at::TABLE), table, "the two probes' spaces name one table");
    let tree = ee.u32_at(table + table_at::FIXED);
    let list = ee.u32_at(table + table_at::MOVING);

    let mut k = 0;
    let mut next = |n: u32| {
        let a = ee.scratch(k + 1);
        k += n;
        a
    };
    let own_list = next(2);
    let tree2 = next(TREE_HEADER / 0x40 + 1);
    let pool2 = next(2);
    let pool_blocks = [next(POOL_BLOCK / 0x40), next(POOL_BLOCK / 0x40)];
    let blocks = next(1);
    let tables = [next(2), next(2), next(2), next(2)];
    let link = next(1);
    let subs = next(1);
    let listeners: Vec<u32> = (0..LISTENERS).map(|_| next(1)).collect();
    let listener_vtable = next(1);
    let probe_vtable = next(3);
    let pool_lists: Vec<u32> = (0..POOL + 2).map(|_| next(1)).collect();
    let mut w = World {
        addrs: probes.to_vec(),
        index: HashMap::new(),
        structures: [tree, list, own_list, tree2],
        nodes: Vec::new(),
        node_index: HashMap::new(),
        chunks: HashMap::new(),
        spaces,
        table,
        tables,
        link,
        subs,
        blocks,
        search,
        class2: Vec::new(),
        movers: Vec::new(),
        listeners,
        listener_vtable,
        probe_vtable,
        pool: Vec::new(),
        pool_lists,
    };

    // The test's list: the save state's header, no blocks yet.
    copy(ee, list, own_list, 0x50);
    ee.set_u32(own_list + list_at::SIZE, L2_SIZE);
    ee.set_u32(own_list + list_at::COUNT, 0);
    ee.set_u32(own_list + list_at::LAST, 0);
    ee.set_u32(own_list + list_at::ARRAY, blocks);
    ee.set_u32(own_list + list_at::BLOCK, 0);
    ee.set_u32(own_list + list_at::ENTRY, 0);

    // The second tree: the first's header (its nodes, its pool, its query as the save state left it)
    // moved off the first's origin, so that its cells, and the box they cover, are not the first's.
    copy(ee, tree, tree2, TREE_HEADER);
    ee.set_f32(tree2 + tree_at::ORIGIN_X, -40.0);
    ee.set_f32(tree2 + tree_at::ORIGIN_Z, -25.0);
    // Its own pool for the links of its list: ending a tree empties its whole pool, so a pool shared
    // with the first would hand the first's links out again while its list still holds them. Two free
    // blocks, nothing taken.
    let pool = ee.u32_at(tree + tree_at::LINK_POOL);
    copy(ee, pool, pool2, 0x50);
    for at in [0x14, 0x18, 0x2c, 0x30] {
        ee.set_u32(pool2 + at, 0);
    }
    let full = ee.u32_at(pool + 0x10);
    ee.set_u32(pool2 + 0x20, full);
    ee.set_u32(pool2 + 0x1c, pool_blocks[0]);
    ee.set_u32(pool_blocks[0], pool_blocks[1]);
    ee.set_u32(pool_blocks[1], 0);
    ee.set_u32(tree2 + tree_at::LINK_POOL, pool2);

    // The tree's nodes, then every collidable in a chunk, then every one in the moving list.
    let mut stack = vec![ee.u32_at(tree + tree_at::ROOT)];
    while let Some(n) = stack.pop() {
        w.node_index.insert(n, w.nodes.len());
        w.nodes.push(n);
        let arr = ee.u32_at(n);
        if arr != 0 {
            stack.extend((0..4).map(|i| ee.u32_at(arr + 4 * i)).filter(|&c| c != 0));
        }
    }
    let add = |w: &mut World, o: u32| {
        if !w.addrs.contains(&o) && !w.index.contains_key(&o) {
            w.index.insert(o, w.addrs.len());
            w.addrs.push(o);
        }
    };
    w.index.insert(probes[0], 0);
    w.index.insert(probes[1], 1);
    for i in 0..w.nodes.len() {
        let mut c = ee.u32_at(w.nodes[i] + 4);
        let mut j = 0;
        while c != 0 {
            w.chunks.insert(c, (i, j));
            for k in 1..5 {
                let o = ee.u32_at(c + 4 * k);
                if o != 0 {
                    add(&mut w, o);
                }
            }
            c = ee.u32_at(c);
            j += 1;
        }
    }
    let (size, n, last) = (ee.i32_at(list + list_at::SIZE), ee.i32_at(list + list_at::COUNT), ee.i32_at(list + list_at::LAST));
    let arr = ee.u32_at(list + list_at::ARRAY);
    for b in 0..n {
        let block = ee.u32_at(arr + 4 * b as u32);
        for k in 0..if b == n - 1 { last } else { size } {
            let o = ee.u32_at(block + 4 * k as u32);
            if o & 2 == 0 {
                add(&mut w, o);
                w.movers.push(o);
            }
        }
    }
    // What the plane's search holds, one of which has left the world since it was gathered.
    let found = ee.u32_at(search + search_at::FOUND);
    for k in 0..ee.i32_at(search + search_at::COUNT).max(0) as u32 {
        add(&mut w, ee.u32_at(found + 8 * k));
    }
    w.class2 = (0..w.addrs.len()).filter(|&i| ee.u8_at(w.addrs[i] + at::CLASS) == 2).collect();

    let wm = ee.u32_at(addr("world_manager").unwrap());
    let models = ee.u32_at(wm + 0x94);
    let top = w.addrs.iter().filter(|&&o| ee.u8_at(o + at::CLASS) == 3).map(|&o| ee.u8_at(o + at::MODEL)).max().unwrap_or(0);
    let models = (0..=u32::from(top))
        .map(|m| {
            let p = ee.u32_at(models + 4 * m);
            if p == 0 { Model::default() } else { Model { near: F(ee.f32_at(p + 0x10c)), far: F(ee.f32_at(p + 0x114)) } }
        })
        .collect();
    let current = ee.u32_at(addr("world_tree_current").unwrap());
    let mut broad = Broad {
        spaces: Vec::new(),
        tables: Vec::new(),
        structures: vec![
            Structure::Tree(tree_of(ee, &w, tree)),
            Structure::List(blocks_of(ee, &w, list)),
            Structure::List(blocks_of(ee, &w, own_list)),
            Structure::Tree(tree_of(ee, &w, tree2)),
        ],
        global: None,
        stamp: ee.u16_at(addr("world_tree_stamp").unwrap()),
        current: w.structure(current),
        cell_size: F(ee.f32_at(addr("world_tree_cell_size").unwrap())),
        group_cell: F(ee.f32_at(wm + 0xd4)),
        models,
        places: w.addrs.iter().map(|&o| placement_of(ee, o, None)).collect(),
    };
    topology(ee, &w, &mut broad);
    let objects = w.addrs.iter().map(|&o| collidable_of(ee, &w, o)).collect();
    eprintln!(
        "world: {} nodes, {} chunks, {} collidables ({} class 2, {} moving), {} models",
        w.nodes.len(),
        w.chunks.len(),
        w.addrs.len(),
        w.class2.len(),
        w.movers.len(),
        broad.models.len()
    );
    (w, broad, objects)
}

/// A new shape for what the queries walk: the probes' table's moving structure and sub-tables, a second
/// space, the global table; written into the original, read back into the port.
fn shake_topology(ee: &mut Ee, w: &World, broad: &mut Broad, p: &mut Pick) {
    let [tree, list, own, tree2] = w.structures;
    let pick = |p: &mut Pick| [0, tree, list, own, tree2][p.below(5) as usize];
    ee.set_u32(w.table + table_at::FIXED, [tree, tree, tree2][p.below(3) as usize]);
    ee.set_u32(w.table + table_at::MOVING, [list, list, list, own, tree][p.below(5) as usize]);
    ee.set_u32(w.subs, w.tables[1]);
    ee.set_u32(w.subs + 4, w.tables[2]);
    ee.set_u32(w.table + table_at::SUBS, w.subs);
    ee.set_u32(w.table + table_at::SUB_COUNT, [0, 0, 1, 2][p.below(4) as usize]);
    for &t in &w.tables {
        let (f, m) = (pick(p), pick(p));
        ee.set_u32(t + table_at::FIXED, f);
        ee.set_u32(t + table_at::MOVING, m);
        ee.set_u32(t + table_at::SUB_COUNT, 0);
    }
    for s in w.spaces {
        ee.set_u32(s + table_at::NEXT, if p.one_in(3) { w.link } else { 0 });
    }
    ee.set_u32(w.link + table_at::NEXT, 0);
    ee.set_u32(w.link + table_at::TABLE, w.tables[0]);
    let g = [0, 0, w.tables[3], w.table][p.below(4) as usize];
    ee.set_u32(addr("world_tree_global_table").unwrap(), g);
    topology(ee, w, broad);
}

/// New contents for the test's block list (its walk rewound on both sides).
fn shake_own_list(ee: &mut Ee, w: &World, broad: &mut Broad, p: &mut Pick) {
    let l = w.structures[2];
    let n = p.below(u64::from(L2_BLOCKS) + 1) as u32;
    for b in 0..n {
        let block = w.blocks + 0x10 + 0x10 * b;
        ee.set_u32(w.blocks + 4 * b, block);
        for k in 0..L2_SIZE {
            let word = if p.one_in(4) {
                2 | (p.next() as u32 & 0xfffc)
            } else if p.one_in(16) {
                w.addrs[p.below(2) as usize]
            } else {
                w.movers[p.below(w.movers.len() as u64) as usize]
            };
            ee.set_u32(block + 4 * k, word);
        }
    }
    ee.set_u32(l + list_at::COUNT, n);
    ee.set_u32(l + list_at::LAST, p.below(u64::from(L2_SIZE) + 1) as u32);
    ee.set_u32(l + list_at::ARRAY, if p.one_in(10) { 0 } else { w.blocks });
    ee.set_u32(l + list_at::BLOCK, 0);
    ee.set_u32(l + list_at::ENTRY, 0);
    broad.structures[2] = Structure::List(blocks_of(ee, w, l));
}

/// What a query left on the original's side and on the port's, checked: the structures, the stamp, the
/// tree last queried, every collidable's stamp and flags.
fn compare_world(ee: &Ee, w: &World, broad: &Broad, objects: &[Collidable], case: usize) {
    for s in [0, 3] {
        let Structure::Tree(t) = &broad.structures[s] else { unreachable!() };
        assert_eq!(port_tree_query(t), tree_query_of(ee, w, w.structures[s]), "case {case}: tree {s}'s query");
    }
    for s in 1..3 {
        let Structure::List(l) = &broad.structures[s] else { unreachable!() };
        let a = w.structures[s];
        let want = (ee.u32_at(a + list_at::FLAGS), ee.i32_at(a + list_at::BLOCK), ee.i32_at(a + list_at::ENTRY));
        assert_eq!((l.flags, l.block, l.entry), want, "case {case}: list {s}'s flags and walk");
    }
    assert_eq!(broad.stamp, ee.u16_at(addr("world_tree_stamp").unwrap()), "case {case}: the stamp");
    let current = ee.u32_at(addr("world_tree_current").unwrap());
    assert_eq!(broad.current, w.structure(current), "case {case}: the tree last queried");
    for (i, &o) in w.addrs.iter().enumerate() {
        assert_eq!(broad.places[i].stamp, ee.u16_at(o + at::STAMP), "case {case}: collidable {i} ({o:#x})'s stamp");
    }
    for &i in &w.class2 {
        let o = w.addrs[i];
        assert_eq!(objects[i].flags, ee.u8_at(o + at::FLAGS), "case {case}: collidable {i} ({o:#x})'s flags");
    }
}

/// Things both sides share and the cases stir: the tree left busy, the stamp by its wrap, flags 0x04 and
/// 0x08 flipped on a few collidables.
fn stir(ee: &mut Ee, w: &World, broad: &mut Broad, objects: &mut [Collidable], p: &mut Pick) {
    if p.one_in(10) {
        let busy = 1 + p.below(0x1000) as u32;
        let s = [0, 3][p.below(2) as usize];
        ee.set_u32(w.structures[s] + tree_at::BUSY, busy);
        let Structure::Tree(t) = &mut broad.structures[s] else { unreachable!() };
        t.busy = busy;
    }
    if p.one_in(50) {
        let stamp = 0xfff0 + p.below(16) as u16;
        ee.write(addr("world_tree_stamp").unwrap(), 2, u128::from(stamp));
        broad.stamp = stamp;
    }
    for _ in 0..p.below(8) {
        let i = w.class2[p.below(w.class2.len() as u64) as usize];
        let f = objects[i].flags ^ [0x04, 0x08, 0x0c][p.below(3) as usize];
        ee.write(w.addrs[i] + at::FLAGS, 1, u128::from(f));
        objects[i].flags = f;
    }
}

/// A point about the plane: on a collidable it gathered, or near one, or near the plane.
fn about_the_plane(broad: &Broad, near: &[usize], p: &mut Pick) -> [F; 4] {
    let o = near[p.below(near.len() as u64) as usize];
    let c = broad.places[o].centre;
    let j = if p.one_in(4) { 0.0 } else { p.range(1.0, 300.0) };
    [
        F(c[0].0 + p.range(-j, j)),
        F(c[1].0 + p.range(-j, j)),
        F(c[2].0 + p.range(-j, j)),
        F::ONE,
    ]
}

fn anywhere(p: &mut Pick) -> [F; 4] {
    [F(p.range(30.0, 6850.0)), F(p.range(-50.0, 400.0)), F(p.range(30.0, 6850.0)), F::ONE]
}

/// The search's list in the original's RAM: (collidable, distance bits).
fn found_of(ee: &Ee, w: &World) -> Vec<(u32, u32)> {
    let n = ee.i32_at(w.search + search_at::COUNT);
    let a = ee.u32_at(w.search + search_at::FOUND);
    (0..n.max(0) as u32).map(|k| (ee.u32_at(a + 8 * k), ee.u32_at(a + 8 * k + 4))).collect()
}

fn search_of(ee: &Ee, w: &World) -> Search {
    let s = w.search;
    Search {
        nearby: Nearby {
            found: found_of(ee, w)
                .into_iter()
                .map(|(o, d)| Candidate { object: w.index[&o], distance: F::from_bits(d) })
                .collect(),
            prepare: ee.u32_at(s + 0x38) != 0,
        },
        centre: vec4(ee, s + search_at::CENTRE),
        bound: F(ee.f32_at(s + search_at::BOUND)),
        least: F(ee.f32_at(s + search_at::LEAST)),
        extra: F(ee.f32_at(s + search_at::EXTRA)),
        no_groups: ee.u32_at(s + search_at::NO_GROUPS) != 0,
        built: ee.u32_at(s + search_at::BUILT) != 0,
        capacity: ee.i32_at(s + search_at::CAPACITY),
    }
}

#[test]
fn the_gather_matches_the_original() {
    let Some(mut ee) = plane() else { return };
    let (w, mut broad, mut objects) = world(&mut ee);
    let gather_at = addr("collision_search_gather").unwrap();
    let probe = 0;
    let mut search = search_of(&ee, &w);
    let near: Vec<usize> = search.nearby.found.iter().map(|c| c.object).collect();
    eprintln!("the plane's search: {} gathered, capacity {}", near.len(), search.capacity);

    let mut p = Pick(0x5ea4_c4fa_0000_0045);
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for case in 0..CASES {
        if !p.one_in(3) {
            shake_topology(&mut ee, &w, &mut broad, &mut p);
        }
        if p.one_in(8) {
            shake_own_list(&mut ee, &w, &mut broad, &mut p);
        }
        stir(&mut ee, &w, &mut broad, &mut objects, &mut p);

        // The probe: moved a little from where the list was gathered, not at all, about the plane or
        // anywhere; now and then another reach.
        let mode = p.below(5);
        let centre = match mode {
            0 => {
                let r = search.bound.0;
                let c = search.centre;
                [F(c[0].0 + p.range(-r, r) * 0.5), F(c[1].0 + p.range(-r, r) * 0.5), F(c[2].0 + p.range(-r, r) * 0.5), F::ONE]
            }
            1 => broad.places[probe].centre,
            2 | 3 => about_the_plane(&broad, &near, &mut p),
            _ => anywhere(&mut p),
        };
        set_vec4(&mut ee, w.addrs[probe] + at::CENTRE, centre);
        broad.places[probe].centre = centre;
        if p.one_in(3) {
            let r = if p.one_in(6) { 0.0 } else { p.range(0.1, 60.0) };
            ee.set_f32(w.addrs[probe] + at::RADIUS, r);
            objects[probe].radius = F(r);
        }

        // The search: left as the last case left it, or rewritten.
        if p.one_in(2) {
            let s = w.search;
            let least = if p.one_in(10) { p.range(120.0, 600.0) } else { p.range(1.0, 120.0) };
            ee.set_f32(s + search_at::LEAST, least);
            ee.set_f32(s + search_at::EXTRA, if p.one_in(4) { 0.0 } else { p.range(0.0, 80.0) });
            ee.set_u32(s + search_at::NO_GROUPS, u32::from(p.one_in(4)));
            ee.set_u32(s + search_at::BUILT, u32::from(!p.one_in(4)));
            let capacity = if p.one_in(3) { p.below(12) as u32 } else { 128 };
            ee.set_u32(s + search_at::CAPACITY, capacity);
            if p.one_in(3) {
                ee.set_f32(s + search_at::BOUND, p.range(0.0, 100.0));
            }
            if p.one_in(4) {
                set_vec4(&mut ee, s + search_at::CENTRE, anywhere(&mut p));
            }
            search = search_of(&ee, &w);
        }
        // Now and then a collidable gathered only within the bound, exactly on it: the bound is the box's
        // half side when the reach is shorter, so the half side is set to that collidable's distance.
        let mut on_the_bound = false;
        if p.one_in(5) {
            let o = near[p.below(near.len() as u64) as usize];
            let c = broad.places[o].centre;
            let centre = [0, 1, 2].map(|k| F(c[k].0 + p.range(-200.0, 200.0)));
            let centre = [centre[0], centre[1], centre[2], F::ONE];
            set_vec4(&mut ee, w.addrs[probe] + at::CENTRE, centre);
            broad.places[probe].centre = centre;
            let d = collidable_distance(F::ZERO, &objects, &broad, o, centre);
            if d.0 > 1.0 && d.0 < 600.0 && accepts(&objects, probe, o, false) {
                let s = w.search;
                ee.set_u32(s + search_at::LEAST, d.to_bits());
                ee.set_f32(s + search_at::EXTRA, 0.0);
                ee.set_u32(s + search_at::BUILT, 0);
                ee.set_u32(s + search_at::CAPACITY, 128);
                search = search_of(&ee, &w);
                let r = objects[probe].radius.0.min(d.0 * 0.5);
                ee.set_f32(w.addrs[probe] + at::RADIUS, r);
                objects[probe].radius = F(r);
                let f = objects[o].flags | more_flags::WITHIN_BOUND;
                ee.write(w.addrs[o] + at::FLAGS, 1, u128::from(f));
                objects[o].flags = f;
                on_the_bound = true;
            }
        }
        // The same centre gathered again (the same cells, so a tree still open restarts).
        if mode == 1 && p.one_in(2) {
            ee.set_u32(w.search + search_at::BUILT, 0);
            search.built = false;
        }
        let kind = if p.one_in(2) { 1 } else { 3 };

        let before = (search.built, search.centre);
        let mid_walk = [0, 3].iter().any(|&s| matches!(&broad.structures[s], Structure::Tree(t) if t.flags & 1 != 0 && t.chunk.is_some()));
        gather(&mut objects, &mut broad, &mut search, probe, kind);
        ee.call(gather_at, &[u64::from(w.search), u64::from(w.addrs[probe]), u64::from(kind)], &[], BUDGET);

        let found: Vec<(u32, u32)> = search.nearby.found.iter().map(|c| (w.addrs[c.object], c.distance.to_bits())).collect();
        assert_eq!(found, found_of(&ee, &w), "case {case}: the list");
        let after = search_of(&ee, &w);
        assert_eq!(search.centre.map(F::to_bits), after.centre.map(F::to_bits), "case {case}: the centre");
        assert_eq!(search.bound.to_bits(), after.bound.to_bits(), "case {case}: the bound");
        assert_eq!(search.built, after.built, "case {case}: built");
        compare_world(&ee, &w, &broad, &objects, case);

        let mut count = |k: &'static str, n: usize| *seen.entry(k).or_default() += n;
        let resorted = before.0 && before.1 == search.centre && broad.places[probe].centre != search.centre;
        count("re-sorted", usize::from(resorted));
        count("gathered", usize::from(!resorted));
        count("full", usize::from(!resorted && search.nearby.found.len() as i32 == search.capacity));
        count("empty", usize::from(search.nearby.found.is_empty()));
        count("objects kept", search.nearby.found.iter().filter(|c| objects[c.object].class == Class::Object).count());
        count("groups kept", search.nearby.found.iter().filter(|c| objects[c.object].class == Class::Group).count());
        count("busy", usize::from([0, 3].iter().any(|&s| matches!(&broad.structures[s], Structure::Tree(t) if t.busy != 0))));
        let uses = |s: usize| {
            let tables = broad.tables.iter();
            tables.flat_map(|t| std::iter::once(&t.own).chain(&t.subs)).any(|q| q.fixed == Some(s) || q.moving == Some(s))
        };
        count("two trees", usize::from(uses(0) && uses(3)));
        count("on the bound", usize::from(on_the_bound));
        count("a tree left mid-walk", usize::from(mid_walk));
        count("a second space", usize::from(broad.spaces[broad.places[probe].space.unwrap()].next.is_some()));
        count("sub-tables", usize::from(!broad.tables[0].subs.is_empty()));
        count("global", usize::from(broad.global.is_some()));
    }
    let mut seen: Vec<_> = seen.into_iter().collect();
    seen.sort();
    eprintln!("gather over {CASES} cases: {seen:?}");
    for (k, n) in &seen {
        // A tree is left mid-walk only when the list fills inside a tree its query does not end (one
        // reached through a sub-table, a second space or the global table): about one case in a hundred.
        let least = if *k == "a tree left mid-walk" { 50 } else { 300 };
        assert!(*n > least, "{k} reached only {n} times");
    }
}

/// One call the direct probe made outside itself, in addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Call {
    Clear(u32),
    Bounds(u32),
    Pair(u32, u32),
    Parts(u32),
    Part(u32, u32),
    HeardHit(u32, u32, u32),
    HeardStruck(u32, u32, u32),
    OnHit(u32, u32, u32),
    OnStruck(u32, u32, u32),
}

/// What the hooks answer, in call order: the narrow phase's hits, the part counts and the parts; the
/// probe's box.
#[derive(Default)]
struct Script {
    hits: Vec<bool>,
    counts: Vec<i32>,
    parts: Vec<u32>,
    at: [usize; 3],
    centre: [F; 4],
    half: [F; 4],
}

impl Script {
    fn hit(&mut self) -> bool {
        self.at[0] += 1;
        self.hits[(self.at[0] - 1) % self.hits.len()]
    }
    fn count(&mut self) -> i32 {
        self.at[1] += 1;
        self.counts[(self.at[1] - 1) % self.counts.len()]
    }
    fn part(&mut self) -> u32 {
        self.at[2] += 1;
        self.parts[(self.at[2] - 1) % self.parts.len()]
    }
}

/// The port's side: the same script, the same log, in addresses.
struct Port<'a> {
    w: &'a World,
    script: Script,
    log: Vec<Call>,
}

impl Narrow for Port<'_> {
    fn clear(&mut self, objects: &mut [Collidable], probe: usize) {
        objects[probe].flags &= !flags::HIT;
        self.log.push(Call::Clear(self.w.addr_of(probe)));
    }
    fn prepare(&mut self, _: &mut [Collidable], _: usize) {
        unreachable!("the direct probe prepares nothing");
    }
    fn pair(&mut self, _: &mut [Collidable], probe: usize, other: usize) -> bool {
        self.log.push(Call::Pair(self.w.addr_of(probe), self.w.addr_of(other)));
        self.script.hit()
    }
    fn parts(&mut self, _: &mut [Collidable], group: usize) -> usize {
        self.log.push(Call::Parts(self.w.addr_of(group)));
        self.script.count().max(0) as usize
    }
    fn part(&mut self, _: &mut [Collidable], group: usize, i: usize) -> Option<usize> {
        self.log.push(Call::Part(self.w.addr_of(group), i as u32));
        let p = self.script.part();
        (p != 0).then(|| self.w.index_of(p))
    }
    fn heard_hit(&mut self, _: &mut [Collidable], l: usize, probe: usize, other: usize) {
        self.log.push(Call::HeardHit(l as u32, self.w.addr_of(probe), self.w.addr_of(other)));
    }
    fn heard_struck(&mut self, _: &mut [Collidable], l: usize, struck: usize, probe: usize) {
        self.log.push(Call::HeardStruck(l as u32, self.w.addr_of(struck), self.w.addr_of(probe)));
    }
    fn on_hit(&mut self, _: &mut [Collidable], c: usize, probe: usize, other: usize) {
        self.log.push(Call::OnHit(c as u32, self.w.addr_of(probe), self.w.addr_of(other)));
    }
    fn on_struck(&mut self, _: &mut [Collidable], c: usize, struck: usize, probe: usize) {
        self.log.push(Call::OnStruck(c as u32, self.w.addr_of(struck), self.w.addr_of(probe)));
    }
}

impl Probe for Port<'_> {
    fn bounds(&mut self, _: &mut [Collidable], probe: usize) -> ([F; 4], [F; 4]) {
        self.log.push(Call::Bounds(self.w.addr_of(probe)));
        (self.script.centre, self.script.half)
    }
}

#[test]
fn probe_collide_world_matches_the_original() {
    let Some(mut ee) = plane() else { return };
    let (mut w, mut broad, _) = world(&mut ee);
    let collide_at = addr("probe_collide_world").unwrap();
    let near: Vec<usize> = search_of(&ee, &w).nearby.found.iter().map(|c| c.object).collect();
    let groups: Vec<usize> = (0..w.addrs.len()).filter(|&i| ee.u8_at(w.addrs[i] + at::CLASS) == 3).collect();

    // Nobody hears a hit or is called back but the probes and a pool of what the plane gathered, whose
    // listeners and callbacks are the test's: the real ones are not this test's to run.
    for &i in &w.class2 {
        let o = w.addrs[i];
        ee.set_u32(o + at::LISTENERS, 0);
        ee.set_u32(o + at::ON_HIT, 0);
        ee.set_u32(o + at::ON_STRUCK, 0);
    }
    w.pool = [0, 1].into_iter().chain(near.iter().copied().filter(|&i| w.class2.contains(&i)).take(POOL)).collect();
    let mut objects: Vec<Collidable> = w.addrs.iter().map(|&o| collidable_of(&ee, &w, o)).collect();

    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    let script: Rc<RefCell<Script>> = Rc::default();
    let a = |ee: &Ee, i: usize| ee.r(4 + i) as u32;
    {
        let log = log.clone();
        ee.hooks.insert(hook::CLEAR, Box::new(move |ee| {
            let probe = a(ee, 0);
            let f = ee.u8_at(probe + at::FLAGS) & !flags::HIT;
            ee.write(probe + at::FLAGS, 1, u128::from(f));
            log.borrow_mut().push(Call::Clear(probe));
        }));
    }
    {
        let (log, script) = (log.clone(), script.clone());
        ee.hooks.insert(hook::BOUNDS, Box::new(move |ee| {
            log.borrow_mut().push(Call::Bounds(a(ee, 0)));
            let s = script.borrow();
            set_vec4(ee, a(ee, 1), s.centre);
            set_vec4(ee, a(ee, 2), s.half);
        }));
    }
    {
        let (log, script) = (log.clone(), script.clone());
        ee.hooks.insert(hook::PAIR, Box::new(move |ee| {
            log.borrow_mut().push(Call::Pair(a(ee, 0), a(ee, 1)));
            let hit = script.borrow_mut().hit();
            ee.set(2, u64::from(hit));
        }));
    }
    {
        let (log, script) = (log.clone(), script.clone());
        ee.hooks.insert(addr("group_part_count").unwrap(), Box::new(move |ee| {
            log.borrow_mut().push(Call::Parts(a(ee, 0)));
            let n = script.borrow_mut().count();
            ee.set(2, n as i64 as u64);
        }));
    }
    {
        let (log, script) = (log.clone(), script.clone());
        ee.hooks.insert(addr("group_part").unwrap(), Box::new(move |ee| {
            log.borrow_mut().push(Call::Part(a(ee, 0), a(ee, 1)));
            let p = script.borrow_mut().part();
            ee.set(2, u64::from(p));
        }));
    }
    {
        let log = log.clone();
        ee.hooks.insert(hook::HEARD_HIT, Box::new(move |ee| {
            log.borrow_mut().push(Call::HeardHit(a(ee, 0), a(ee, 1), a(ee, 2)));
        }));
    }
    {
        let log = log.clone();
        ee.hooks.insert(hook::HEARD_STRUCK, Box::new(move |ee| {
            log.borrow_mut().push(Call::HeardStruck(a(ee, 0), a(ee, 1), a(ee, 2)));
        }));
    }
    for h in hook::ON_HIT {
        let log = log.clone();
        ee.hooks.insert(h, Box::new(move |ee| log.borrow_mut().push(Call::OnHit(h, a(ee, 0), a(ee, 1)))));
    }
    for h in hook::ON_STRUCK {
        let log = log.clone();
        ee.hooks.insert(h, Box::new(move |ee| log.borrow_mut().push(Call::OnStruck(h, a(ee, 0), a(ee, 1)))));
    }
    for &l in &w.listeners {
        ee.set_u32(l, w.listener_vtable);
    }
    ee.set_u32(w.listener_vtable + 8, hook::HEARD_HIT);
    ee.set_u32(w.listener_vtable + 0xc, hook::HEARD_STRUCK);
    ee.set_u32(w.probe_vtable + slot::CLEAR, hook::CLEAR);
    ee.set_u32(w.probe_vtable + slot::PAIR, hook::PAIR);
    ee.set_u32(w.probe_vtable + slot::BOUNDS, hook::BOUNDS);
    for &o in &w.addrs[..2] {
        ee.set_u32(o + at::CLASS_VTABLE, w.probe_vtable);
    }

    let mut p = Pick(0x5ea4_c4fa_0000_4504);
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for case in 0..CASES {
        if !p.one_in(3) {
            shake_topology(&mut ee, &w, &mut broad, &mut p);
        }
        if p.one_in(8) {
            shake_own_list(&mut ee, &w, &mut broad, &mut p);
        }
        stir(&mut ee, &w, &mut broad, &mut objects, &mut p);

        // The pool's listeners and callbacks.
        for (k, &i) in w.pool.iter().enumerate() {
            let o = w.addrs[i];
            let n = match p.below(4) {
                0 | 1 => 0,
                2 => 1,
                _ => 2 + p.below(2) as usize,
            };
            let ls: Vec<u32> = (0..n).map(|_| w.listeners[p.below(LISTENERS as u64) as usize]).collect();
            ee.set_u32(o + at::LISTENERS, ls.len() as u32);
            ee.set_u32(o + at::LISTENERS + 8, w.pool_lists[k]);
            for (j, &l) in ls.iter().enumerate() {
                ee.set_u32(w.pool_lists[k] + 4 * j as u32, l);
            }
            objects[i].listeners = ls.iter().map(|&l| l as usize).collect();
            let cb = |p: &mut Pick, which: [u32; 2]| if p.one_in(2) { 0 } else { which[p.below(2) as usize] };
            let (h, s) = (cb(&mut p, hook::ON_HIT), cb(&mut p, hook::ON_STRUCK));
            ee.set_u32(o + at::ON_HIT, h);
            ee.set_u32(o + at::ON_STRUCK, s);
            objects[i].on_hit = (h != 0).then_some(h as usize);
            objects[i].on_struck = (s != 0).then_some(s as usize);
        }

        // The probe: its flags, where it is and was, its budget, its reach.
        let probe = p.below(2) as usize;
        let o = w.addrs[probe];
        let mut f = objects[probe].flags & !(0x02 | flags::NO_GROUPS | flags::HIT);
        f |= if p.one_in(10) { 0 } else { 0x02 };
        f |= u8::from(p.one_in(3)) * flags::NO_GROUPS;
        f |= u8::from(p.one_in(2)) * flags::HIT;
        ee.write(o + at::FLAGS, 1, u128::from(f));
        objects[probe].flags = f;
        let centre = match p.below(6) {
            0 | 1 | 2 => about_the_plane(&broad, &near, &mut p),
            3 => {
                let g = groups[p.below(groups.len() as u64) as usize];
                let c = broad.places[g].cell;
                let x = c[0] as f32 * broad.group_cell.0 + p.range(-20.0, 20.0);
                let z = c[1] as f32 * broad.group_cell.0 + p.range(-20.0, 20.0);
                [F(x), F(p.range(0.0, 300.0)), F(z), F::ONE]
            }
            4 => broad.places[probe].centre,
            _ => anywhere(&mut p),
        };
        let last = match p.below(3) {
            0 => centre,
            1 => [0, 1, 2, 3].map(|k| F(centre[k].0 + if k < 3 { p.range(-10.0, 10.0) } else { 0.0 })),
            _ => broad.places[probe].last,
        };
        let budget = if p.one_in(3) { 0.0 } else { p.range(-5.0, 30.0) };
        let radius = if p.one_in(4) { p.range(0.0, 2.0) } else { p.range(0.5, 40.0) };
        set_vec4(&mut ee, o + at::CENTRE, centre);
        set_vec4(&mut ee, o + at::LAST, last);
        ee.set_f32(o + at::BUDGET, budget);
        ee.set_f32(o + at::RADIUS, radius);
        let place = &mut broad.places[probe];
        place.centre = centre;
        place.last = last;
        place.budget = F(budget);
        objects[probe].radius = F(radius);
        let lazy = match p.below(4) {
            0 | 1 => 0,
            2 => 1,
            _ => p.next() as u32 | 1,
        };

        // The box, the narrow phase's answers, the parts.
        let half = [p.range(0.0, 40.0), p.range(0.0, 40.0), p.range(0.0, 40.0), 0.0];
        let off = [p.range(-5.0, 5.0), p.range(-5.0, 5.0), p.range(-5.0, 5.0), 0.0];
        let hits: Vec<bool> = (0..1 + p.below(6)).map(|_| !p.one_in(3)).collect();
        let counts: Vec<i32> = (0..1 + p.below(3)).map(|_| [-1, 0, 1, 2, 3][p.below(5) as usize]).collect();
        let parts: Vec<u32> = (0..1 + p.below(4))
            .map(|_| if p.one_in(4) { 0 } else { w.addrs[w.pool[p.below(w.pool.len() as u64) as usize]] })
            .collect();
        let script_of = || Script {
            hits: hits.clone(),
            counts: counts.clone(),
            parts: parts.clone(),
            at: [0; 3],
            centre: [0, 1, 2, 3].map(|k| F(centre[k].0 + off[k])),
            half: half.map(F),
        };
        *script.borrow_mut() = script_of();
        log.borrow_mut().clear();

        let mut port = Port { w: &w, script: script_of(), log: Vec::new() };
        let got = probe_collide_world(&mut objects, &mut broad, probe, lazy, &mut port);
        ee.call(collide_at, &[u64::from(o), u64::from(lazy)], &[], BUDGET);
        let want = ee.f0();

        assert_eq!(port.log, *log.borrow(), "case {case}: the calls out");
        assert_eq!(got.to_bits(), want.to_bits(), "case {case}: the return ({} against {want})", got.0);
        let place = &broad.places[probe];
        assert_eq!(place.budget.to_bits(), ee.u32_at(o + at::BUDGET), "case {case}: the budget");
        assert_eq!(place.last.map(F::to_bits), vec4(&ee, o + at::LAST).map(F::to_bits), "case {case}: the last centre");
        compare_world(&ee, &w, &broad, &objects, case);
        for &i in &w.class2 {
            let g = ee.u32_at(w.addrs[i] + at::GROUP);
            assert_eq!(objects[i].group.map(|g| w.addr_of(g)).unwrap_or(0), g, "case {case}: collidable {i}'s group");
        }

        let calls = log.borrow();
        let mut count = |k: &'static str, n: usize| *seen.entry(k).or_default() += n;
        count("inactive", usize::from(f & 0x02 == 0));
        count("lazy, budget left", usize::from(f & 0x02 != 0 && !calls.contains(&Call::Clear(o))));
        count("hit", usize::from(f & 0x02 != 0 && objects[probe].flags & flags::HIT != 0));
        count("pairs", calls.iter().filter(|c| matches!(c, Call::Pair(..))).count());
        count("parts", calls.iter().filter(|c| matches!(c, Call::Part(..))).count());
        count("heard", calls.iter().filter(|c| matches!(c, Call::HeardHit(..) | Call::HeardStruck(..))).count());
        count("callbacks", calls.iter().filter(|c| matches!(c, Call::OnHit(..) | Call::OnStruck(..))).count());
        count("missed, nearest > 0", usize::from(calls.contains(&Call::Clear(o)) && got.0 > 0.0));
        count("moving pass", usize::from({
            let mut s = broad.places[probe].space;
            let mut m = false;
            while let Some(i) = s {
                let own = broad.tables[broad.spaces[i].tables].own;
                m |= own.fixed != own.moving;
                s = broad.spaces[i].next;
            }
            m && calls.contains(&Call::Clear(o))
        }));
    }
    let mut seen: Vec<_> = seen.into_iter().collect();
    seen.sort();
    eprintln!("probe_collide_world over {CASES} cases: {seen:?}");
    for (k, n) in &seen {
        assert!(*n > 300, "{k} reached only {n} times");
    }
}
