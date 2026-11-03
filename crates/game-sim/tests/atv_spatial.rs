//! MD75c1 — the world's spatial database move (`atv_spatial_move`), proved against SLUS-21104.
//!
//! The oracle is MD37a's RAM image of Desert Oasis Free Ride (`context/mxvatv-ps2/md37a/atv-r-spawn.ee.bin`,
//! as in `tests/atv_quad.rs`). Its database has three cells and no portal between them, so the walk,
//! the overlaps and the link pool never run on its own data. The tests:
//!
//! - **generated databases**: 12,000 of them laid in the image's free RAM — a tree of cells joined by
//!   portals placed on and off the way the entry moves, each cell's lists (blocks of slots, a free
//!   chain) and its first sub-database (a list, a quadtree — MD89 — or the static one), the link
//!   pool (chunks, a free chain, chunks set aside), the entry's record and the extra cells it is in —
//!   and the original's move run on them (its heap calls served from that RAM). Everything read back
//!   (the cells' links, every list's blocks and slots, every tree's nodes from its root — their
//!   heights bit for bit, their chunks and children — its pools, query, counts, the entries whose
//!   mark a put cleared; the link pool, the record) must be the port's, the centre bit for bit;
//! - **the image's own database**: the trigger probe's and the frame mesh's records moved in it as
//!   they are, then with one portal laid between the probe's cell and another of the image's cells,
//!   so that the entry walks into it out of the image's own lists and overlaps back;
//! - **the image's own tree** (MD89): the three vehicles' spheres moved in it, the same key and
//!   other ones.
//!
//! Without the image the tests skip (and say so).

use game_sim::atv_spatial::{self, Cell, Chunk, Database, Extra, Item, Link, LinkAt, List, Node, Place, Pool, Portal, Record, Slot, SlotAt, Sub, Tree};
use game_sim::quad::{Body, Call, Moto, Quad};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

/// SLUS-21104's `$gp` (crt0 sets it at `0x001001ac`).
const GP: u64 = 0x003d_baf0;
/// `atv_spatial_move`; the heap's allocate and reallocate; the link pool's global; the list
/// sub-database's vtable, the static quadtree's and the cell's.
const MOVE: u32 = 0x0018_d690;
const ALLOC: u32 = 0x001b_7340;
const REALLOC: u32 = 0x001b_7470;
const POOL_AT: u32 = 0x0040_c460;
const LIST_VT: u32 = 0x003b_5510;
const TREE_VT: u32 = 0x003b_6700;
const CELL_VT: u32 = 0x003b_55b0;
/// A tree node's vtable, at its `+0x10`.
const NODE_VT: u32 = 0x003b_66e0;
/// The heap's allocation and free with a file and line (a tree's nodes).
const NEW: u32 = 0x001b_7620;
const FREE: u32 = 0x001b_76d0;
/// The tree a tree's put or take-out is working on (its new and freed nodes are counted in it).
const CURRENT: u32 = 0x0040_c198;
/// An entry's first halfword as laid: a tree's put clears it.
const MARK: u32 = 0xa5a5;
/// The generated entries, `0x20` apart from `ITEMS`.
const ENTRIES: u32 = 64;

/// The image's RAM nothing uses (all zero), where the generated databases go.
const AREA: u32 = 0x01f8_0000;
const AREA_END: u32 = 0x01fe_c000;
/// Entry `i` at `ITEMS + 0x20 i` (its `+8` the record when it is the one moved).
const ITEMS: u32 = AREA;
const REC: u32 = AREA + 0x800;
const POOL: u32 = AREA + 0x840;
const CENTRE: u32 = AREA + 0x900;
const HALF: u32 = AREA + 0x910;
const PORTAL_SIZE: u32 = 0xd0;
const CELLS: u32 = AREA + 0x1000;
const LINKS: u32 = AREA + 0x1800;
const PORTALS: u32 = AREA + 0x2000;
const SUBS: u32 = AREA + 0x4000;
const HEAP: u32 = AREA + 0x8000;
/// Where the image test lays its portal and its links (and its heap, after them).
const IMAGE_PORTAL: u32 = AREA;
const IMAGE_LINKS: u32 = AREA + 0x100;
const IMAGE_HEAP: u32 = AREA + 0x1000;

const BUDGET: u64 = 4_000_000;

fn image() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2/md37a").join(format!("atv-r-spawn.ee{}", ".bin"));
    if !path.exists() {
        skip(&format!(
            "{} is not there: it is MD37a's (eeMemory.bin of atv-r-spawn.p2s; memory: md37a ATV states live in s2)",
            path.display()
        ));
        return None;
    }
    let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    if ee.u32_at(0x0065_52b0 + 4) != LIST_VT || ee.u32_at(0x0065_4680 + 4) != CELL_VT {
        panic!("{} is not the MX vs. ATV image these addresses are from", path.display());
    }
    Some(ee)
}

// --- the heap the original's allocations are served from ------------------------------------------------

/// A bump heap: every allocation 16-aligned after the last, its size kept for a reallocation.
#[derive(Default)]
struct Heap {
    next: u32,
    end: u32,
    sizes: BTreeMap<u32, u32>,
}

impl Heap {
    fn alloc(&mut self, n: u32) -> u32 {
        let at = self.next;
        self.next = (at + n + 15) & !15;
        assert!(self.next <= self.end, "the test's heap is full");
        self.sizes.insert(at, n);
        at
    }
}

type Shared = Rc<RefCell<Heap>>;

fn hook_heap(ee: &mut Ee, heap: &Shared) {
    let h = heap.clone();
    ee.hooks.insert(
        ALLOC,
        Box::new(move |ee| {
            let at = h.borrow_mut().alloc(ee.r(4) as u32);
            ee.set(2, at as u64);
        }),
    );
    let h = heap.clone();
    ee.hooks.insert(
        NEW,
        Box::new(move |ee| {
            let at = h.borrow_mut().alloc(ee.r(4) as u32);
            ee.set(2, at as u64);
        }),
    );
    ee.hooks.insert(FREE, Box::new(|_| {}));
    let h = heap.clone();
    ee.hooks.insert(
        REALLOC,
        Box::new(move |ee| {
            let (old, n) = (ee.r(4) as u32, ee.r(5) as u32);
            let at = h.borrow_mut().alloc(n);
            if old != 0 {
                // The image's own blocks were not allocated here: copy as much as is asked.
                let had = h.borrow().sizes.get(&old).copied().unwrap_or(n);
                for k in (0..had.min(n)).step_by(4) {
                    let w = ee.u32_at(old + k);
                    ee.set_u32(at + k, w);
                }
            }
            ee.set(2, at as u64);
        }),
    );
}

// --- laying a database in RAM and reading one back ------------------------------------------------------

fn set_vf(ee: &mut Ee, a: u32, v: Vf) {
    for (k, b) in v.to_bits().into_iter().enumerate() {
        ee.set_u32(a + 4 * k as u32, b);
    }
}

fn vf_at(ee: &Ee, a: u32) -> Vf {
    Vf::from_bits(std::array::from_fn(|k| ee.u32_at(a + 4 * k as u32)))
}

fn lay_portal(ee: &mut Ee, a: u32, p: &Portal) {
    for (i, c) in p.corners.iter().enumerate() {
        set_vf(ee, a + 0x30 + 0x10 * i as u32, *c);
    }
    set_vf(ee, a + 0x70, p.normal);
    for (i, e) in p.edges.iter().enumerate() {
        set_vf(ee, a + 0x80 + 0x10 * i as u32, *e);
    }
    ee.set_u32(a + 0xc4, p.plane.to_bits());
}

fn read_portal(ee: &Ee, a: u32) -> Portal {
    Portal {
        corners: std::array::from_fn(|i| vf_at(ee, a + 0x30 + 0x10 * i as u32)),
        normal: vf_at(ee, a + 0x70),
        plane: F(ee.f32_at(a + 0xc4)),
        edges: std::array::from_fn(|i| vf_at(ee, a + 0x80 + 0x10 * i as u32)),
    }
}

// --- pools: the link pool, and each tree's chunk and query pools ------------------------------------------

/// A pool's chunks in RAM: where each link is.
struct Links {
    chunks: Vec<u32>,
    capacity: u32,
    stride: u32,
}

impl Links {
    fn addr(&self, at: LinkAt) -> u32 {
        self.chunks[at.chunk] + 4 + at.index as u32 * self.stride
    }

    fn at(&self, a: u32) -> LinkAt {
        for (c, &base) in self.chunks.iter().enumerate() {
            let first = base + 4;
            if (first..first + self.capacity * self.stride).contains(&a) {
                assert_eq!((a - first) % self.stride, 0, "a link {a:#x} off its chunk's stride");
                return LinkAt { chunk: c, index: ((a - first) / self.stride) as usize };
            }
        }
        panic!("link {a:#x} is in none of the pool's chunks");
    }
}

/// `p` laid at `at`, its chunks (those in use and those set aside) off the heap.
fn lay_pool(ee: &mut Ee, h: &mut Heap, at: u32, p: &Pool) -> Links {
    let bytes = p.capacity as u32 * p.stride;
    let chunks: Vec<u32> = (0..p.chunks).map(|_| h.alloc(bytes + 4)).collect();
    for w in chunks.windows(2) {
        ee.set_u32(w[0], w[1]);
    }
    let spares: Vec<u32> = (0..p.spare).map(|_| h.alloc(bytes + 4)).collect();
    for w in spares.windows(2) {
        ee.set_u32(w[0], w[1]);
    }
    for (o, v) in [
        (8, p.capacity as u32),
        (0xc, chunks.first().copied().unwrap_or(0)),
        (0x10, chunks.last().copied().unwrap_or(0)),
        (0x14, spares.first().copied().unwrap_or(0)),
        (0x18, p.used as u32),
        (0x1c, p.stride),
        (0x20, bytes),
        (0x28, chunks.last().map_or(0, |c| c + 4 + p.used as u32 * p.stride)),
        (0x38, 4),
    ] {
        ee.set_u32(at + o, v);
    }
    let links = Links { chunks, capacity: p.capacity as u32, stride: p.stride };
    let mut head = 0;
    for &f in &p.free {
        let a = links.addr(f);
        ee.set_u32(a, head);
        head = a;
    }
    ee.set_u32(at + 0x24, head);
    links
}

fn read_pool(ee: &Ee, p: u32) -> (Pool, Links) {
    let mut chunks = vec![];
    let mut c = ee.u32_at(p + 0xc);
    while c != 0 {
        chunks.push(c);
        assert!(chunks.len() < 64, "the pool's chunk chain does not end");
        c = ee.u32_at(c);
    }
    let links = Links { chunks, capacity: ee.u32_at(p + 8), stride: ee.u32_at(p + 0x1c) };
    let mut free = vec![];
    let mut f = ee.u32_at(p + 0x24);
    while f != 0 {
        free.push(links.at(f));
        assert!(free.len() < 4096, "the pool's free chain does not end");
        f = ee.u32_at(f);
    }
    free.reverse();
    let mut spare = 0;
    let mut s = ee.u32_at(p + 0x14);
    while s != 0 {
        spare += 1;
        assert!(spare < 4096, "the pool's set-aside chain does not end");
        s = ee.u32_at(s);
    }
    let pool = Pool {
        capacity: ee.i32_at(p + 8),
        stride: links.stride,
        used: ee.i32_at(p + 0x18),
        chunks: links.chunks.len(),
        spare,
        free,
    };
    if let Some(&last) = links.chunks.last()
        && pool.used < pool.capacity
    {
        assert_eq!(ee.u32_at(p + 0x28), last + 4 + pool.used as u32 * links.stride, "the pool's next link");
    }
    (pool, links)
}

// --- trees -----------------------------------------------------------------------------------------------

fn lay_tree(ee: &mut Ee, h: &mut Heap, a: u32, t: &Tree) {
    ee.set_u32(a + 4, TREE_VT);
    ee.set_u32(a + 8, t.kind);
    ee.set_u32(a + 0xc, t.flags);
    for (o, v) in [(0x28, t.origin[0]), (0x30, t.origin[1]), (0x40, t.extent[0]), (0x44, t.extent[1]), (0x54, t.scale)] {
        ee.set_u32(a + o, v.to_bits());
    }
    ee.set_u32(a + 0x4c, t.sides.len() as u32 - 1);
    for (l, &side) in t.sides.iter().enumerate() {
        ee.set_u32(a + 0x58 + 4 * l as u32, side as u32);
    }
    let (pool, query) = (h.alloc(0x40), h.alloc(0x40));
    let links = lay_pool(ee, h, pool, &t.pool);
    lay_pool(ee, h, query, &t.query);
    ee.set_u32(a + 0x90, pool);
    ee.set_u32(a + 0x94, query);
    for (o, w) in [0x9c, 0xa0, 0xa4, 0xac].into_iter().zip(t.walk) {
        ee.set_u32(a + o, w);
    }
    for (k, w) in t.cells.into_iter().enumerate() {
        ee.set_u32(a + 0x8a0 + 4 * k as u32, w);
    }
    ee.set_u32(a + 0x880, t.count as u32);
    ee.set_u32(a + 0x888, t.stamp as u32);
    let root = lay_node(ee, h, t, t.root, &links);
    ee.set_u32(a + 0x8c, root);
}

/// A node as the tree makes them: 20 bytes, 36 with its children after it.
fn lay_node(ee: &mut Ee, h: &mut Heap, t: &Tree, n: usize, links: &Links) -> u32 {
    let node = t.node(n);
    let at = h.alloc(if node.children.is_some() { 36 } else { 20 });
    if let Some(c) = node.children {
        ee.set_u32(at, at + 0x14);
        for (i, c) in c.into_iter().enumerate() {
            let child = c.map_or(0, |c| lay_node(ee, h, t, c, links));
            ee.set_u32(at + 0x14 + 4 * i as u32, child);
        }
    }
    let mut head = at + 4;
    for c in &node.chunks {
        let l = links.addr(c.at);
        ee.set_u32(head, l);
        for (i, it) in c.items.iter().enumerate() {
            ee.set_u32(l + 4 + 4 * i as u32, it.map_or(0, item_addr));
        }
        head = l;
    }
    ee.set_u32(head, 0);
    ee.set_u32(at + 8, node.mid.to_bits());
    ee.set_u32(at + 0xc, node.half.to_bits());
    ee.set_u32(at + 0x10, NODE_VT);
    at
}

fn read_tree(ee: &Ee, a: u32, item: &mut dyn FnMut(u32) -> Item) -> Tree {
    let f = |o: u32| F(ee.f32_at(a + o));
    let deepest = ee.i32_at(a + 0x4c);
    let (pool, links) = read_pool(ee, ee.u32_at(a + 0x90));
    let (query, _) = read_pool(ee, ee.u32_at(a + 0x94));
    let mut t = Tree {
        kind: ee.u32_at(a + 8),
        flags: ee.u32_at(a + 0xc),
        origin: [f(0x28), f(0x30)],
        extent: [f(0x40), f(0x44)],
        sides: (0..=deepest).map(|l| ee.i32_at(a + 0x58 + 4 * l as u32)).collect(),
        scale: f(0x54),
        root: 0,
        nodes: vec![],
        pool,
        query,
        walk: [0x9c, 0xa0, 0xa4, 0xac].map(|o| ee.u32_at(a + o)),
        cells: std::array::from_fn(|k| ee.u32_at(a + 0x8a0 + 4 * k as u32)),
        count: ee.i32_at(a + 0x880),
        stamp: ee.i32_at(a + 0x888),
    };
    t.root = read_node(ee, &mut t, ee.u32_at(a + 0x8c), &links, item, 0);
    t
}

fn read_node(ee: &Ee, t: &mut Tree, a: u32, links: &Links, item: &mut dyn FnMut(u32) -> Item, depth: usize) -> usize {
    assert_eq!(ee.u32_at(a + 0x10), NODE_VT, "node {a:#x}'s vtable");
    assert!(depth < 20, "the tree does not end");
    let n = t.nodes.len();
    t.nodes.push(None);
    let children = match ee.u32_at(a) {
        0 => None,
        c => {
            assert_eq!(c, a + 0x14, "node {a:#x}'s children are not after it");
            let mut kids = [None; 4];
            for (i, k) in kids.iter_mut().enumerate() {
                *k = match ee.u32_at(c + 4 * i as u32) {
                    0 => None,
                    k => Some(read_node(ee, t, k, links, item, depth + 1)),
                };
            }
            Some(kids)
        }
    };
    let mut chunks = vec![];
    let mut c = ee.u32_at(a + 4);
    while c != 0 {
        let items = std::array::from_fn(|i| match ee.u32_at(c + 4 + 4 * i as u32) {
            0 => None,
            v => Some(v),
        });
        chunks.push(Chunk { at: links.at(c), items: items.map(|v| v.map(&mut *item)) });
        assert!(chunks.len() < 100_000, "node {a:#x}'s chunks do not end");
        c = ee.u32_at(c);
    }
    t.nodes[n] = Some(Node { children, chunks, mid: F(ee.f32_at(a + 8)), half: F(ee.f32_at(a + 0xc)) });
    n
}

/// Where a database is in RAM: its cells', portals' and sub-databases' addresses (in the model's
/// order), each list's blocks, and the pool's.
#[derive(Default)]
struct Map {
    cells: Vec<u32>,
    portals: Vec<u32>,
    subs: Vec<u32>,
    pool: u32,
}

/// What an address in a list's slot or a record's is in the model.
struct Reader<'a> {
    item: &'a mut dyn FnMut(u32) -> Item,
    blocks: Vec<Vec<u32>>,
    size: Vec<u32>,
    trees: Vec<bool>,
    links: Links,
}

impl Reader<'_> {
    fn index(list: &[u32], a: u32, what: &str) -> usize {
        list.iter().position(|&x| x == a).unwrap_or_else(|| panic!("{what} {a:#x} is none of the database's"))
    }

    fn slot_at(&self, sub: usize, a: u32) -> SlotAt {
        let size = self.size[sub];
        for (b, &base) in self.blocks[sub].iter().enumerate() {
            if (base..base + 4 * size).contains(&a) {
                assert_eq!((a - base) % 4, 0, "a slot {a:#x} off its block's words");
                return SlotAt { block: b, index: ((a - base) / 4) as usize };
            }
        }
        panic!("slot {a:#x} is in none of sub-database {sub}'s blocks");
    }

    /// A record's or an extra's place word: a tree's key, a list's slot.
    fn place(&self, sub: usize, a: u32) -> Place {
        if self.trees[sub] { Place::Key(a) } else { Place::Slot(self.slot_at(sub, a)) }
    }

    fn read(ee: &Ee, map: &Map, item: &mut dyn FnMut(u32) -> Item, rec: u32) -> (Database, Record) {
        let (pool, links) = read_pool(ee, map.pool);
        let mut r = Reader { item, blocks: vec![], size: vec![], trees: vec![], links };
        for &s in &map.subs {
            let (n, arr) = (ee.u32_at(s + 0x2c), ee.u32_at(s + 0x34));
            let list = ee.u32_at(s + 4) == LIST_VT;
            r.blocks.push(if list { (0..n).map(|b| ee.u32_at(arr + 4 * b)).collect() } else { vec![] });
            r.size.push(ee.u32_at(s + 0x28));
            r.trees.push(ee.u32_at(s + 4) == TREE_VT);
        }
        let mut db = Database { pool, ..Default::default() };
        for &c in &map.cells {
            let mut links = vec![];
            let mut l = ee.u32_at(c + 0x24);
            while l != 0 {
                links.push(Link {
                    cell: Reader::index(&map.cells, ee.u32_at(l + 4), "a link's cell"),
                    portal: Reader::index(&map.portals, ee.u32_at(l + 8), "a link's portal"),
                });
                l = ee.u32_at(l);
            }
            let sub = |o: u32| match ee.u32_at(c + o) {
                0 => None,
                s => Some(Reader::index(&map.subs, s, "a cell's sub-database")),
            };
            db.cells.push(Cell { links, subs: [sub(0x28), sub(0x2c)] });
        }
        db.portals = map.portals.iter().map(|&a| read_portal(ee, a)).collect();
        for (k, &s) in map.subs.iter().enumerate() {
            let kind = ee.u32_at(s + 8);
            if r.trees[k] {
                db.subs.push(Sub::Tree(read_tree(ee, s, r.item)));
                continue;
            }
            if ee.u32_at(s + 4) != LIST_VT {
                db.subs.push(Sub::Other { kind });
                continue;
            }
            let (size, last) = (ee.i32_at(s + 0x28), ee.i32_at(s + 0x30));
            let n = r.blocks[k].len();
            let mut blocks = vec![];
            for b in 0..n {
                let base = r.blocks[k][b];
                let handed = if b + 1 == n { last } else { size } as usize;
                let mut slots = vec![None; size as usize];
                for (i, slot) in slots.iter_mut().enumerate().take(handed) {
                    let v = ee.u32_at(base + 4 * i as u32);
                    *slot = Some(if v & 2 != 0 {
                        let next = v & !3;
                        Slot::Free((next != 0).then(|| r.slot_at(k, next)))
                    } else {
                        Slot::Item((r.item)(v))
                    });
                }
                // The image's lists keep stale words there; the generated ones are zero.
                for i in (handed..size as usize).filter(|_| base >= AREA) {
                    assert_eq!(ee.u32_at(base + 4 * i as u32), 0, "a slot past the last handed out is written");
                }
                blocks.push(slots);
            }
            let free = match ee.u32_at(s + 0x38) {
                0 => None,
                f => Some(r.slot_at(k, f)),
            };
            db.subs.push(Sub::List(List { kind, size, last, blocks, free }));
        }
        let cell = match ee.u32_at(rec + 4) {
            0 => None,
            c => Some(Reader::index(&map.cells, c, "the record's cell")),
        };
        let mut record = Record { cell, last: vf_at(ee, rec + 0x10), ..Default::default() };
        if cell.is_some() {
            record.sub = Reader::index(&map.subs, ee.u32_at(rec + 8), "the record's sub-database");
            record.slot = r.place(record.sub, ee.u32_at(rec + 0xc));
        }
        let mut l = ee.u32_at(rec);
        while l != 0 {
            let sub = Reader::index(&map.subs, ee.u32_at(l + 8), "an extra's sub-database");
            record.extras.push(Extra {
                at: r.links.at(l),
                cell: Reader::index(&map.cells, ee.u32_at(l + 4), "an extra's cell"),
                sub,
                slot: r.place(sub, ee.u32_at(l + 0xc)),
            });
            assert!(record.extras.len() < 64, "the record's extras do not end");
            l = ee.u32_at(l);
        }
        (db, record)
    }
}

fn item_addr(i: Item) -> u32 {
    ITEMS + 0x20 * i as u32
}

fn item_of(a: u32) -> Item {
    assert!(a >= ITEMS && (a - ITEMS) % 0x20 == 0 && a - ITEMS < 0x20 * ENTRIES, "slot word {a:#x} is no entry");
    ((a - ITEMS) / 0x20) as Item
}

/// The generated database laid in the area: the model's addresses, and the heap after them.
fn lay(ee: &mut Ee, heap: &Shared, db: &Database, rec: &Record, mover: Item) -> Map {
    ee.ram[AREA as usize..AREA_END as usize].fill(0);
    *heap.borrow_mut() = Heap { next: HEAP, end: AREA_END, sizes: BTreeMap::new() };
    let mut h = heap.borrow_mut();
    let map = Map {
        cells: (0..db.cells.len() as u32).map(|i| CELLS + 0x40 * i).collect(),
        portals: (0..db.portals.len() as u32).map(|i| PORTALS + PORTAL_SIZE * i).collect(),
        subs: (0..db.subs.len() as u32)
            .map(|i| if let Sub::Tree(_) = db.subs[i as usize] { h.alloc(0x8c0) } else { SUBS + 0x40 * i })
            .collect(),
        pool: POOL,
    };
    for i in 0..ENTRIES {
        ee.set_u32(ITEMS + 0x20 * i, if db.unmarked.contains(&(i as Item)) { 0 } else { MARK });
    }
    // The lists' blocks first: a slot word needs every block's address.
    let mut blocks: Vec<Vec<u32>> = vec![];
    for (k, s) in db.subs.iter().enumerate() {
        let a = map.subs[k];
        match s {
            Sub::Other { kind } => {
                // Never called: a move that reaches one panics in the port.
                ee.set_u32(a + 8, *kind);
                blocks.push(vec![]);
            }
            Sub::Tree(t) => {
                lay_tree(ee, &mut h, a, t);
                blocks.push(vec![]);
            }
            Sub::List(l) => {
                ee.set_u32(a + 4, LIST_VT);
                ee.set_u32(a + 8, l.kind);
                ee.set_u32(a + 0x28, l.size as u32);
                ee.set_u32(a + 0x2c, l.blocks.len() as u32);
                ee.set_u32(a + 0x30, l.last as u32);
                let mut bs = vec![];
                if !l.blocks.is_empty() {
                    let arr = h.alloc(4 * l.blocks.len() as u32);
                    ee.set_u32(a + 0x34, arr);
                    for b in 0..l.blocks.len() as u32 {
                        let at = h.alloc(4 * l.size as u32);
                        ee.set_u32(arr + 4 * b, at);
                        bs.push(at);
                    }
                }
                blocks.push(bs);
            }
        }
    }
    let slot_addr = |sub: usize, s: SlotAt| blocks[sub][s.block] + 4 * s.index as u32;
    let place_addr = |sub: usize, p: Place| match p {
        Place::Slot(s) => slot_addr(sub, s),
        Place::Key(k) => k,
    };
    for (k, s) in db.subs.iter().enumerate() {
        let Sub::List(l) = s else { continue };
        for (b, block) in l.blocks.iter().enumerate() {
            for (i, slot) in block.iter().enumerate() {
                let v = match slot {
                    None => 0,
                    Some(Slot::Item(it)) => item_addr(*it),
                    Some(Slot::Free(n)) => n.map_or(0, |n| slot_addr(k, n)) | 2,
                };
                ee.set_u32(blocks[k][b] + 4 * i as u32, v);
            }
        }
        ee.set_u32(map.subs[k] + 0x38, l.free.map_or(0, |f| slot_addr(k, f)));
    }
    let mut link = LINKS;
    for (c, cell) in db.cells.iter().enumerate() {
        let a = map.cells[c];
        ee.set_u32(a + 4, CELL_VT);
        let mut head = a + 0x24;
        for l in &cell.links {
            ee.set_u32(head, link);
            ee.set_u32(link + 4, map.cells[l.cell]);
            ee.set_u32(link + 8, map.portals[l.portal]);
            head = link;
            link += 0x10;
        }
        for (o, s) in [(0x28, cell.subs[0]), (0x2c, cell.subs[1])] {
            ee.set_u32(a + o, s.map_or(0, |s| map.subs[s]));
        }
    }
    assert!(link <= PORTALS, "too many links for the area");
    for (i, p) in db.portals.iter().enumerate() {
        lay_portal(ee, map.portals[i], p);
    }
    let links = lay_pool(ee, &mut h, POOL, &db.pool);
    ee.set_u32(POOL_AT, POOL);
    let mut next = 0;
    for e in rec.extras.iter().rev() {
        let a = links.addr(e.at);
        ee.set_u32(a, next);
        ee.set_u32(a + 4, map.cells[e.cell]);
        ee.set_u32(a + 8, map.subs[e.sub]);
        ee.set_u32(a + 0xc, place_addr(e.sub, e.slot));
        next = a;
    }
    ee.set_u32(REC, next);
    if let Some(c) = rec.cell {
        ee.set_u32(REC + 4, map.cells[c]);
        ee.set_u32(REC + 8, map.subs[rec.sub]);
        ee.set_u32(REC + 0xc, place_addr(rec.sub, rec.slot));
    }
    set_vf(ee, REC + 0x10, rec.last);
    ee.set_u32(item_addr(mover) + 8, REC);
    map
}

// --- the generator ------------------------------------------------------------------------------------

/// A deterministic generator: inputs only.
struct Gen(u64);

impl Gen {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 7) as f64 / (1u64 << 24) as f64
    }
    fn signed(&mut self) -> f64 {
        self.unit() * 2.0 - 1.0
    }
    fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i + 1);
            v.swap(i, j);
        }
    }
}

type V3 = [f64; 3];

fn v3(v: Vf) -> V3 {
    [v.x() as f64, v.y() as f64, v.z() as f64]
}
fn vf(v: V3, w: f32) -> Vf {
    Vf::from_bits([(v[0] as f32).to_bits(), (v[1] as f32).to_bits(), (v[2] as f32).to_bits(), w.to_bits()])
}
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unit(a: V3) -> V3 {
    let n = dot(a, a).sqrt();
    if n < 1e-6 { [0.0, 1.0, 0.0] } else { scale(a, 1.0 / n) }
}

/// A point in the box the generated cases move in.
fn point(g: &mut Gen) -> V3 {
    [g.signed() * 8.0, g.signed() * 8.0, g.signed() * 8.0]
}

/// A portal: on the way from `a` to `b` most of the time, facing roughly along it half of the time.
fn portal(g: &mut Gen, a: V3, b: V3) -> Portal {
    let way = sub(b, a);
    let at = if g.chance(0.6) {
        add(add(a, scale(way, g.unit() * 1.4 - 0.2)), scale(point(g), 0.05))
    } else {
        point(g)
    };
    let mut n = if g.chance(0.5) { unit(add(way, scale(point(g), 0.1))) } else { unit(point(g)) };
    if g.chance(0.5) {
        n = scale(n, -1.0);
    }
    let u = unit(cross(n, if n[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] }));
    let v = cross(n, u);
    let (su, sv) = (0.2 + g.unit() * 6.0, 0.2 + g.unit() * 6.0);
    let mut corners = [
        add(at, add(scale(u, -su), scale(v, -sv))),
        add(at, add(scale(u, su), scale(v, -sv))),
        add(at, add(scale(u, su), scale(v, sv))),
        add(at, add(scale(u, -su), scale(v, sv))),
    ];
    if g.chance(0.5) {
        corners.reverse();
    }
    let edges: [V3; 4] = std::array::from_fn(|i| {
        let (c, d) = (corners[i], corners[(i + 1) % 4]);
        let mut e = unit(cross(sub(d, c), n));
        if dot(e, sub(scale(add(c, d), 0.5), at)) < 0.0 {
            e = scale(e, -1.0);
        }
        if g.chance(0.05) { unit(point(g)) } else { e }
    });
    let mut plane = dot(n, corners[0]);
    if g.chance(0.1) {
        plane += g.signed() * 0.5;
    }
    Portal {
        corners: corners.map(|c| vf(c, 1.0)),
        normal: vf(n, 0.0),
        plane: F(plane as f32),
        edges: edges.map(|e| vf(e, 0.0)),
    }
}

/// A list of `size`-slot blocks: none, or some with the last part handed out, the slots handed out
/// holding other entries (`2..`) or on a free chain.
fn list(g: &mut Gen, kind: u32) -> List {
    let size = [1, 2, 3, 4, 8][g.below(5)];
    let count = [0, 1, 1, 2, 3][g.below(5)];
    let last = if count == 0 { size } else { g.below(size as usize + 1) as i32 };
    let mut l = List { kind, size, last, ..Default::default() };
    let mut handed = vec![];
    for b in 0..count {
        let n = if b + 1 == count { last } else { size } as usize;
        let mut block = vec![None; size as usize];
        for (i, s) in block.iter_mut().enumerate().take(n) {
            *s = Some(Slot::Item(2 + g.below(40)));
            handed.push(SlotAt { block: b, index: i });
        }
        l.blocks.push(block);
    }
    g.shuffle(&mut handed);
    let frees = if handed.is_empty() { 0 } else { g.below(handed.len() + 1) };
    for &s in &handed[..frees] {
        l.blocks[s.block][s.index] = Some(Slot::Free(l.free));
        l.free = Some(s);
    }
    l
}

/// A slot in `l` holding `item`: one of its entries' (taken over), else one handed out new.
fn place(g: &mut Gen, l: &mut List, item: Item) -> SlotAt {
    let mut held = vec![];
    for (b, block) in l.blocks.iter().enumerate() {
        for (i, s) in block.iter().enumerate() {
            if let Some(Slot::Item(it)) = s
                && *it >= 2
            {
                held.push(SlotAt { block: b, index: i });
            }
        }
    }
    let at = if !held.is_empty() && g.chance(0.7) {
        held[g.below(held.len())]
    } else {
        if l.last == l.size || l.blocks.is_empty() {
            l.blocks.push(vec![None; l.size as usize]);
            l.last = 0;
        }
        l.last += 1;
        SlotAt { block: l.blocks.len() - 1, index: l.last as usize - 1 }
    };
    l.blocks[at.block][at.index] = Some(Slot::Item(item));
    at
}

/// A pool of `stride`-byte links, up to three chunks of `capacity`, the last partly handed out; the
/// links handed out, shuffled.
fn pool(g: &mut Gen, strides: &[u32], capacities: &[i32]) -> (Pool, Vec<LinkAt>) {
    let stride = strides[g.below(strides.len())];
    let capacity = capacities[g.below(capacities.len())];
    let chunks = g.below(4);
    let used = if chunks == 0 { capacity } else { 1 + g.below(capacity as usize) as i32 };
    let mut handed = vec![];
    for c in 0..chunks {
        let n = if c + 1 == chunks { used } else { capacity } as usize;
        handed.extend((0..n).map(|index| LinkAt { chunk: c, index }));
    }
    g.shuffle(&mut handed);
    (Pool { capacity, stride, used, chunks, spare: g.below(3), free: vec![] }, handed)
}

/// A box's half sides: now tiny, now a few feet, now much of a tree; now and then upside down (no
/// height for a tree to widen to).
fn half(g: &mut Gen) -> Vf {
    let s = [0.01, 3.0, 3.0, 8.0][g.below(4)];
    let mut h = [g.unit() * s, g.unit() * s, g.unit() * s];
    if g.chance(0.03) {
        h[1] = -h[1] - 0.01;
    }
    vf(h, 0.0)
}

/// A quadtree over about the box the cases move in (some of it outside), down to level 0..6, its
/// cells now and then finer than its keys hold; other entries (`2..`) put in it and some taken out
/// again; its query open or not.
fn tree(g: &mut Gen, kind: u32) -> Tree {
    let deepest = g.below(7);
    let extent = [12.0 + g.unit() * 8.0, 12.0 + g.unit() * 8.0];
    let origin = [-6.0 - g.unit() * 4.0, -6.0 - g.unit() * 4.0];
    let mut scale = 32768.0 / extent[0].max(extent[1]);
    if g.chance(0.1) {
        scale *= 1.5;
    }
    let sides = (0..=deepest).map(|l| 0x4000 >> l).collect();
    let (mut chunks, handed) = pool(g, &[0x14, 0x18, 0x20], &[4, 8, 16]);
    chunks.free = handed[..g.below(handed.len() + 1)].to_vec();
    let mut t = Tree::new(kind, origin.map(|v| F(v as f32)), extent.map(|v| F(v as f32)), sides, F(scale as f32), chunks, Pool::default());
    let mut filed = vec![];
    for _ in 0..g.below(16) {
        let item = 2 + g.below(40);
        let (centre, half) = (vf(point(g), 1.0), half(g));
        let key = t.pick(centre, half);
        t.put(item, key, centre, half);
        filed.push((item, key));
    }
    for (item, key) in filed {
        if g.chance(0.3) {
            t.take_out(item, key);
        }
    }
    t.count = t.count.wrapping_add(g.below(4000) as i32);
    t.stamp = g.below(4000) as i32;
    let (mut query, handed) = pool(g, &[0x10, 0x14, 0x20], &[1, 2, 4, 8]);
    query.free = handed[..g.below(handed.len() + 1)].to_vec();
    t.query = query;
    t.flags = u32::from(g.chance(0.5)) | if g.chance(0.3) { 2 } else { 0 };
    if g.chance(0.7) {
        t.walk = std::array::from_fn(|_| g.next() as u32);
        t.cells = std::array::from_fn(|_| g.next() as u32);
    }
    t
}

/// The mover put in `sub`, its box about `last`, half of the time `half` itself (so that a move
/// to `last` keeps its place): a list's slot, a tree's key (none if `extra` and the box has no key,
/// as the original never makes such an extra).
fn file(g: &mut Gen, db: &mut Database, sub: usize, mover: Item, last: V3, half: Vf, extra: bool) -> Option<Place> {
    match &mut db.subs[sub] {
        Sub::List(l) => Some(Place::Slot(place(g, l, mover))),
        Sub::Tree(t) => {
            let at = if g.chance(0.6) { last } else { add(last, scale(point(g), 0.05)) };
            let h = if g.chance(0.6) { half } else { self::half(g) };
            let key = t.pick(vf(at, 1.0), h);
            if extra && key == 0 {
                return None;
            }
            t.put(mover, key, vf(at, 1.0), h);
            Some(Place::Key(key))
        }
        Sub::Other { .. } => unreachable!(),
    }
}

struct Case {
    db: Database,
    rec: Record,
    mover: Item,
    centre: Vf,
    half: Vf,
    reach: F,
}

fn case(g: &mut Gen) -> Case {
    let mover = g.below(2);
    let n = 1 + g.below(8);
    let last = point(g);
    let centre = match g.below(20) {
        0..=2 => last,
        3..=5 => add(last, scale(point(g), 0.05)),
        _ => point(g),
    };
    let mut db = Database::default();
    db.cells = vec![Cell::default(); n];
    for c in 1..n {
        let parent = g.below(c);
        let p = db.portals.len();
        db.portals.push(portal(g, last, centre));
        db.cells[c].links.push(Link { cell: parent, portal: p });
        db.cells[parent].links.push(Link { cell: c, portal: p });
    }
    for c in &mut db.cells {
        let mut links = std::mem::take(&mut c.links);
        g.shuffle(&mut links);
        c.links = links;
    }
    // Every cell's second sub-database a list of kind 2; its first a list of kind 1 in all of
    // them, or a tree of kind 1 in all of them (now and then the last cell's, as the image's two
    // cells share theirs: the mover is then in one tree twice), or the static one (never moved in)
    // in some. With
    // firsts in all, now and then one of kind 3 (which picks a cell's first) in either place, for
    // the kind the overlaps carry from one newly reached cell to the next.
    let mode = g.below(3);
    let firsts = mode < 2;
    for c in 0..n {
        let kind = if g.chance(0.2) { 3 } else { 1 };
        if mode == 0 {
            db.cells[c].subs[0] = Some(db.subs.len());
            db.subs.push(Sub::List(list(g, kind)));
        } else if mode == 1 && c > 0 && g.chance(0.3) {
            db.cells[c].subs[0] = db.cells[c - 1].subs[0];
        } else if mode == 1 {
            db.cells[c].subs[0] = Some(db.subs.len());
            db.subs.push(Sub::Tree(tree(g, kind)));
        } else if g.chance(0.5) {
            db.cells[c].subs[0] = Some(db.subs.len());
            db.subs.push(Sub::Other { kind: 1 });
        }
        db.cells[c].subs[1] = Some(db.subs.len());
        let kind = if firsts && g.chance(0.2) { 3 } else { 2 };
        db.subs.push(Sub::List(list(g, kind)));
    }
    let pick_sub = |g: &mut Gen, db: &Database, c: usize| {
        let s = db.cells[c].subs;
        if firsts && g.chance(0.5) { s[0].unwrap() } else { s[1].unwrap() }
    };
    let half = half(g);
    let w = if g.chance(0.5) { 1.0 } else { g.signed() as f32 * 4.0 };
    let mut rec = Record { last: vf(last, w), ..Default::default() };
    let (mut pool, mut handed) = pool(g, &[0x10, 0x14, 0x20], &[1, 2, 3, 4, 8]);
    if g.chance(0.95) {
        let home = g.below(n);
        rec.cell = Some(home);
        rec.sub = pick_sub(g, &db, home);
        rec.slot = file(g, &mut db, rec.sub, mover, last, half, false).unwrap();
        let mut others: Vec<usize> = (0..n).filter(|&c| c != home).collect();
        g.shuffle(&mut others);
        let k = g.below(others.len().min(3) + 1).min(handed.len());
        for &c in &others[..k] {
            let sub = pick_sub(g, &db, c);
            if let Some(slot) = file(g, &mut db, sub, mover, last, half, true) {
                rec.extras.push(Extra { at: handed.pop().unwrap(), cell: c, sub, slot });
            }
        }
    }
    let frees = g.below(handed.len() + 1);
    pool.free = handed[..frees].to_vec();
    db.pool = pool;
    let reach = match g.below(20) {
        0..=4 => 0.0,
        5..=7 => 100.0,
        _ => g.unit() * 8.0,
    };
    let mut centre = vf(centre, if g.chance(0.5) { 1.0 } else { g.signed() as f32 });
    // Now and then the centre on a portal's corner or its plane, the edge cases of both tests.
    if !db.portals.is_empty() && g.chance(0.05) {
        let p = db.portals[g.below(db.portals.len())];
        let c = p.corners[g.below(4)];
        centre = Vf::from_bits([c.x().to_bits(), c.y().to_bits(), c.z().to_bits(), centre.w().to_bits()]);
    }
    Case { db, rec, mover, centre, half, reach: F(reach as f32) }
}

// --- the tests ----------------------------------------------------------------------------------------

/// The port's move, half of the time through the quad's call ([`atv_spatial::Answers`]).
fn port(c: &Case, by_call: bool) -> (Database, Record) {
    let (mut db, mut rec) = (c.db.clone(), c.rec.clone());
    if by_call {
        let mut a = atv_spatial::Answers { space: atv_spatial::Space { db, ..Default::default() }, rest: Vec::<Call>::new() };
        let node = c.mover == atv_spatial::NODE;
        *if node { &mut a.space.node } else { &mut a.space.quad } = rec;
        a.call(Call::SpatialMove { node, centre: c.centre, half: c.half, reach: c.reach }, &mut Quad::default(), &mut Body::default());
        assert!(a.rest.is_empty(), "the move went on to the rest");
        let rec = if node { a.space.node } else { a.space.quad };
        (a.space.db, rec)
    } else {
        atv_spatial::spatial_move(&mut db, &mut rec, c.mover, c.centre, c.half, c.reach);
        (db, rec)
    }
}

/// The entries whose first halfword a tree's put cleared.
fn marks(ee: &Ee, db: &mut Database, items: impl IntoIterator<Item = (Item, u32)>) {
    db.unmarked = items.into_iter().filter(|&(_, a)| ee.u16_at(a) == 0).map(|(i, _)| i).collect();
}

fn generated(ee: &Ee, db: &mut Database) {
    marks(ee, db, (0..ENTRIES).map(|i| (i as Item, ITEMS + 0x20 * i)));
}

fn trees(db: &Database) -> Vec<&Tree> {
    db.subs.iter().filter_map(|s| if let Sub::Tree(t) = s { Some(t) } else { None }).collect()
}

fn written_outside(ee: &Ee, allowed: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let stack = (ee.stack - 0x1_0000, ee.ram.len() as u32);
    ee.written_ranges()
        .into_iter()
        .filter(|&(a, b)| ![stack].iter().chain(allowed).any(|&(lo, hi)| (a & 0x01ff_ffff) >= lo && (b & 0x01ff_ffff) <= hi))
        .collect()
}

#[test]
fn the_spatial_move_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let heap: Shared = Rc::default();
    hook_heap(&mut ee, &heap);
    let mut g = Gen(0x75c1_0001);
    let mut tally: BTreeMap<&str, u32> = BTreeMap::new();
    const N: usize = 12_000;
    for k in 0..N {
        let c = case(&mut g);
        let map = lay(&mut ee, &heap, &c.db, &c.rec, c.mover);
        let (mut before, rec0) = Reader::read(&ee, &map, &mut item_of, REC);
        generated(&ee, &mut before);
        assert_eq!((&before, &rec0), (&c.db, &c.rec), "case {k}: the laid database does not read back as generated");
        ee.set(28, GP);
        set_vf(&mut ee, CENTRE, c.centre);
        set_vf(&mut ee, HALF, c.half);
        ee.call(MOVE, &[item_addr(c.mover) as u64, CENTRE as u64, HALF as u64], &[c.reach.0], BUDGET);
        let out = written_outside(&ee, &[(AREA, AREA_END), (CURRENT, CURRENT + 4)]);
        assert!(out.is_empty(), "case {k}: the original wrote outside the database: {out:x?}");
        let (mut want_db, want_rec) = Reader::read(&ee, &map, &mut item_of, REC);
        generated(&ee, &mut want_db);
        let (got_db, got_rec) = port(&c, k % 2 == 1);
        if (&got_db, &got_rec) != (&want_db, &want_rec) || got_rec.last.to_bits() != want_rec.last.to_bits() {
            panic!(
                "case {k}: the port's move is not the original's\n  centre {:?} reach {:?}\n  was {:#?}\n  {:#?}\n  want {want_db:#?}\n  {want_rec:#?}\n  got {got_db:#?}\n  {got_rec:#?}",
                c.centre, c.reach, c.db, c.rec
            );
        }
        // What the case went through, from the model's before and after.
        let mut seen = |s: &'static str| *tally.entry(s).or_default() += 1;
        match (c.rec.cell, want_rec.cell) {
            (None, _) => seen("no cell: nothing"),
            (a, b) if a == b => seen("stayed in its cell"),
            _ => seen("walked to another cell"),
        }
        if let (Some(a), Some(b)) = (c.rec.cell, want_rec.cell)
            && a != b
            && !c.db.cells[a].links.iter().any(|l| l.cell == b)
        {
            seen("walked through two portals or more");
        }
        if c.rec.cell.is_some() {
            let cells = |r: &Record| r.extras.iter().map(|e| e.cell).collect::<Vec<_>>();
            let (was, now) = (cells(&c.rec), cells(&want_rec));
            if was.iter().any(|x| now.contains(x)) {
                seen("an extra cell kept");
            }
            if was.iter().any(|x| !now.contains(x)) {
                seen("an extra cell left");
            }
            if now.iter().any(|x| !was.contains(x)) {
                seen("an extra cell entered");
            }
            if c.db.pool.chunks < want_db.pool.chunks {
                seen(if c.db.pool.spare > want_db.pool.spare { "a set-aside chunk taken" } else { "a chunk off the heap" });
            }
            if want_db.pool.free.len() < c.db.pool.free.len() {
                seen("a freed link taken");
            }
            if want_db.pool.used > c.db.pool.used {
                seen("a chunk's next link taken");
            }
            let grew = c.db.subs.iter().zip(&want_db.subs).any(|(a, b)| match (a, b) {
                (Sub::List(a), Sub::List(b)) => a.blocks.len() < b.blocks.len(),
                _ => false,
            });
            if grew {
                seen("a list grew a block");
            }
            let mixed = want_rec.extras.iter().any(|e| want_db.cells[e.cell].subs[0] == Some(e.sub))
                && want_rec.extras.iter().any(|e| want_db.cells[e.cell].subs[1] == Some(e.sub));
            if mixed {
                seen("extras in both kinds of list");
            }
            if let Sub::Tree(_) = c.db.subs[c.rec.sub] {
                match () {
                    _ if c.rec.cell != want_rec.cell => seen("tree: out of its cell's"),
                    _ if c.rec.slot == want_rec.slot => seen("tree: the same key (moved)"),
                    _ => seen("tree: another key (out, in)"),
                }
            }
            if let Sub::Tree(_) = want_db.subs[want_rec.sub] {
                if want_rec.slot == Place::Key(0) {
                    seen("tree: no key for the box");
                } else if c.rec.cell != want_rec.cell {
                    seen("tree: into another cell's");
                }
            }
            if want_rec.extras.iter().any(|e| matches!(want_db.subs[e.sub], Sub::Tree(_))) {
                seen("tree: an extra in one");
            }
            let subs = |r: &Record| std::iter::once(r.sub).chain(r.extras.iter().map(|e| e.sub)).collect::<Vec<_>>();
            let twice = |r: &Record, db: &Database| {
                let s = subs(r);
                s.iter().enumerate().any(|(i, a)| matches!(db.subs[*a], Sub::Tree(_)) && s[..i].contains(a))
            };
            if twice(&c.rec, &c.db) || twice(&want_rec, &want_db) {
                seen("tree: the mover in one twice");
            }
            for (a, b) in trees(&c.db).into_iter().zip(trees(&want_db)) {
                if a.count < b.count {
                    seen("tree: a node made");
                }
                if a.count > b.count {
                    seen("tree: a node freed");
                }
                if a.pool.free.len() < b.pool.free.len() {
                    seen("tree: a chunk freed");
                }
                if a.pool.chunks < b.pool.chunks {
                    seen("tree: a chunk off the heap");
                }
                if a.flags & 1 != 0 && b.flags & 1 == 0 {
                    seen("tree: its query closed");
                }
                if a != b && a.stamp == b.stamp && a.count == b.count {
                    seen("tree: only heights widened");
                }
            }
        }
    }
    println!("{N} generated moves, the original's and the port's the same:");
    for (s, n) in &tally {
        println!("  {n:6}  {s}");
    }
    for s in [
        "no cell: nothing",
        "stayed in its cell",
        "walked to another cell",
        "walked through two portals or more",
        "an extra cell kept",
        "an extra cell left",
        "an extra cell entered",
        "a set-aside chunk taken",
        "a chunk off the heap",
        "a freed link taken",
        "a chunk's next link taken",
        "a list grew a block",
        "extras in both kinds of list",
        "tree: out of its cell's",
        "tree: the same key (moved)",
        "tree: another key (out, in)",
        "tree: no key for the box",
        "tree: into another cell's",
        "tree: an extra in one",
        "tree: the mover in one twice",
        "tree: a node made",
        "tree: a node freed",
        "tree: a chunk freed",
        "tree: a chunk off the heap",
        "tree: its query closed",
        "tree: only heights widened",
    ] {
        assert!(tally.get(s).copied().unwrap_or(0) >= 10, "the cases hardly ever went through \"{s}\": {tally:?}");
    }
}

/// The image's three cells, and the records of the quad's trigger probe and of the frame mesh.
const IMAGE_CELLS: [u32; 3] = [0x0065_45c0, 0x0065_4680, 0x0084_d8b0];
const IMAGE_ENTRIES: [u32; 2] = [0x0159_c518, 0x0162_7388];

/// The image's entries, numbered as they are met.
#[derive(Default)]
struct Items {
    at: Vec<u32>,
    of: HashMap<u32, Item>,
}

impl Items {
    fn new(first: u32) -> RefCell<Items> {
        let items = RefCell::new(Items::default());
        items.borrow_mut().of(first);
        items
    }

    fn of(&mut self, a: u32) -> Item {
        *self.of.entry(a).or_insert_with(|| {
            self.at.push(a);
            self.at.len() - 1
        })
    }
}

/// The image's database as the model reads it: entries numbered as they are met, the moved one
/// first.
fn read_image(ee: &Ee, map: &Map, items: &RefCell<Items>, entry: u32) -> (Database, Record) {
    let mut item = |a: u32| items.borrow_mut().of(a);
    let (mut db, rec) = Reader::read(ee, map, &mut item, ee.u32_at(entry + 8));
    marks(ee, &mut db, items.borrow().at.iter().enumerate().map(|(i, &a)| (i, a)));
    (db, rec)
}

fn image_map(ee: &Ee, portals: Vec<u32>) -> Map {
    let mut subs = vec![];
    for &c in &IMAGE_CELLS {
        for o in [0x28, 0x2c] {
            let s = ee.u32_at(c + o);
            if s != 0 && !subs.contains(&s) {
                subs.push(s);
            }
        }
    }
    Map { cells: IMAGE_CELLS.to_vec(), portals, subs, pool: ee.u32_at(POOL_AT) }
}

#[test]
fn the_images_own_database_moves_as_the_original() {
    let Some(base) = image() else { return };
    let mut tally = vec![];
    for (e, &entry) in IMAGE_ENTRIES.iter().enumerate() {
        for laid in [false, true] {
            let mut ee = Ee::with_ram(base.ram.clone());
            let heap: Shared = Rc::new(RefCell::new(Heap { next: IMAGE_HEAP, end: AREA_END, sizes: BTreeMap::new() }));
            hook_heap(&mut ee, &heap);
            let rec_at = ee.u32_at(entry + 8);
            let home = ee.u32_at(rec_at + 4);
            assert!(IMAGE_CELLS.contains(&home), "entry {entry:#x}'s record is in cell {home:#x}");
            let last = vf_at(&ee, rec_at + 0x10);
            let (centre, reach);
            let mut portals = vec![];
            if laid {
                // A portal a foot along +x of the last centre, between the entry's cell and another of
                // the image's, and the centre moved through it, its sphere reaching back.
                let other = *IMAGE_CELLS.iter().find(|&&c| c != home && ee.u32_at(c + 0x2c) != 0).unwrap();
                let l = v3(last);
                let at = add(l, [1.0, 0.0, 0.0]);
                let corners = [[0.0, -4.0, -4.0], [0.0, 4.0, -4.0], [0.0, 4.0, 4.0], [0.0, -4.0, 4.0]].map(|c| vf(add(at, c), 1.0));
                let p = Portal {
                    corners,
                    normal: vf([1.0, 0.0, 0.0], 0.0),
                    plane: F(at[0] as f32),
                    edges: [[0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]].map(|e| vf(e, 0.0)),
                };
                lay_portal(&mut ee, IMAGE_PORTAL, &p);
                for (k, (from, to)) in [(home, other), (other, home)].into_iter().enumerate() {
                    let link = IMAGE_LINKS + 0x10 * k as u32;
                    ee.set_u32(link, ee.u32_at(from + 0x24));
                    ee.set_u32(link + 4, to);
                    ee.set_u32(link + 8, IMAGE_PORTAL);
                    ee.set_u32(from + 0x24, link);
                }
                portals.push(IMAGE_PORTAL);
                centre = vf(add(l, [1.5, 0.25, -0.5]), last.w());
                reach = F(2.0);
            } else {
                centre = vf(add(v3(last), [0.0, 0.125, 0.0]), last.w());
                reach = F::ZERO;
            }
            let map = image_map(&ee, portals);
            let items = Items::new(entry);
            let (mut db, mut rec) = read_image(&ee, &map, &items, entry);
            let was = rec.clone();
            ee.set(28, GP);
            set_vf(&mut ee, CENTRE, centre);
            set_vf(&mut ee, HALF, vf([1.0, 1.0, 1.0], 0.0));
            ee.call(MOVE, &[entry as u64, CENTRE as u64, HALF as u64], &[reach.0], BUDGET);
            let (want_db, want_rec) = read_image(&ee, &map, &items, entry);
            atv_spatial::spatial_move(&mut db, &mut rec, 0, centre, vf([1.0, 1.0, 1.0], 0.0), reach);
            assert_eq!((&db, &rec), (&want_db, &want_rec), "entry {entry:#x} (portal laid: {laid}): the port's move is not the original's");
            assert_eq!(rec.last.to_bits(), want_rec.last.to_bits());
            if laid {
                assert_ne!(was.cell, rec.cell, "entry {entry:#x} did not walk through the laid portal");
                assert_eq!(rec.extras.len(), 1, "entry {entry:#x} does not reach back into its old cell");
            }
            tally.push(format!(
                "entry {e} portal {laid}: cell {:?} -> {:?}, slot {:?} -> {:?}, extras {}, pool chunks {}",
                was.cell,
                rec.cell,
                was.slot,
                rec.slot,
                rec.extras.len(),
                db.pool.chunks
            ));
        }
    }
    for t in &tally {
        println!("{t}");
    }
}

/// The vehicles' bodies: the player's quad, then two parked at the origin.
const IMAGE_VEHICLES: [u32; 3] = [0x015e_0690, 0x0165_7090, 0x019b_87f0];

/// The vehicles' spheres moved in the image's own tree (shared by two of its cells; 4264 nodes):
/// the player's, its key `0x780900f5`, to its last centre a little higher (the same key: its nodes
/// widened), a long way off (out of four corners and into others, nodes made) and far up (the root
/// widened); the parked two, filed under key `0x15` at the origin, where the box has no key (taken
/// out, put nowhere) and to the player's place. The moves' half and reach are the vehicles' (40).
#[test]
fn the_images_tree_moves_as_the_original() {
    let Some(base) = image() else { return };
    let mut tally = vec![];
    for (v, &vehicle) in IMAGE_VEHICLES.iter().enumerate() {
        let entry = base.u32_at(vehicle + 0xd80) + 0x18;
        let rec_at = base.u32_at(entry + 8);
        assert_eq!(base.u32_at(rec_at + 8), 0x0065_47c0, "vehicle {vehicle:#x}'s sphere is not in the tree");
        let last = v3(vf_at(&base, rec_at + 0x10));
        let player = v3(vf_at(&base, base.u32_at(IMAGE_VEHICLES[0] + 0xd80) + 0x160));
        let tos: Vec<V3> = if v == 0 {
            vec![add(last, [0.0, 0.5, 0.0]), add(last, [300.0, 20.0, -150.0]), add(last, [0.0, 900.0, 0.0])]
        } else {
            vec![last, player]
        };
        for to in tos {
            let mut ee = Ee::with_ram(base.ram.clone());
            let heap: Shared = Rc::new(RefCell::new(Heap { next: IMAGE_HEAP, end: AREA_END, sizes: BTreeMap::new() }));
            hook_heap(&mut ee, &heap);
            let map = image_map(&ee, vec![]);
            let items = Items::new(entry);
            let (mut db, mut rec) = read_image(&ee, &map, &items, entry);
            let was = rec.clone();
            let (centre, half, reach) = (vf(to, 1.0), vf([40.0, 40.0, 40.0], 0.0), F(40.0));
            ee.set(28, GP);
            set_vf(&mut ee, CENTRE, centre);
            set_vf(&mut ee, HALF, half);
            ee.call(MOVE, &[entry as u64, CENTRE as u64, HALF as u64], &[reach.0], BUDGET);
            let (want_db, want_rec) = read_image(&ee, &map, &items, entry);
            atv_spatial::spatial_move(&mut db, &mut rec, 0, centre, half, reach);
            assert!(
                (&db, &rec) == (&want_db, &want_rec),
                "vehicle {vehicle:#x} to {to:?}: the port's move is not the original's (record {rec:?}, want {want_rec:?})"
            );
            assert_eq!(rec.last.to_bits(), want_rec.last.to_bits());
            let Sub::Tree(t) = &db.subs[rec.sub] else { panic!("the sphere left the tree") };
            tally.push(format!(
                "vehicle {v} to {to:?}: {:x?} -> {:x?}, tree nodes {} (count {}), stamp {}, root height {:?} ± {:?}",
                was.slot,
                rec.slot,
                t.nodes.iter().flatten().count(),
                t.count,
                t.stamp,
                t.node(t.root).mid,
                t.node(t.root).half,
            ));
        }
    }
    for t in &tally {
        println!("{t}");
    }
}
