//! **The world's spatial database** (MD75c1): SLUS-21104's move of an entry's box (`atv_spatial_move`),
//! the call the quad's spatial update, its node's bounds and each collidable's sync end in.
//!
//! The database is a set of **cells** joined by **portals**. Each cell has a link list (a link: the
//! cell beyond and the portal on the way) and up to two sub-databases, picked by a kind's bit 0
//! (the first) and bit 1 (the second). An entry's **record** says which cell it is in, which of that
//! cell's sub-databases holds it and in which slot, the box's centre when it last moved, and the
//! extra cells its box reaches into, each a link out of a shared **pool** naming the cell, the
//! sub-database and the slot.
//!
//! A move:
//!
//! - **walks the portals** from the record's cell: the first link (in list order, never straight
//!   back) whose portal the segment from the last centre to the new one crosses is taken, and the
//!   walk starts again from the cell beyond;
//! - **in the same cell**, asks the sub-database for a slot (the current one handed in) and moves the
//!   entry there, or takes it out and puts it in the new slot;
//! - **in a new cell**, takes it out of the old sub-database and out of every extra cell (the links
//!   back to the pool), then puts it in the new cell's sub-database of the old one's kind;
//! - **then the overlaps**: every cell its centre's sphere reaches through a portal, gathered
//!   cell by cell from the record's cell; an extra cell still reached is moved in place, one no
//!   longer reached is left, and each newly reached one is entered with a link from the pool;
//! - and keeps the centre (all four lanes) for the next walk.
//!
//! Two classes of sub-database are ported. Every cell of MD37a's image holds its moving entries (its
//! second) in a **list** of slots in blocks with a free chain. A cell's first is the **quadtree**
//! (`broad_phase`'s, which the gather reads; MD89): an entry of kind bit 0 is filed there under a
//! key, the node at the key's level under each corner it names, and the nodes on the way keep the
//! height their entries span. The vehicles' spheres move in it. [`Sub::Other`] is one not read.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use ps2_float::vu::Dest;
use ps2_float::{F, Vf, fpu, fpu::Acc};
use std::collections::BTreeSet;

/// Who an entry is, as its sub-databases' slots hold it (the original keeps the entry's address).
pub type Item = usize;

/// A portal between two cells: a quad in a plane.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Portal {
    /// `+0x30`..`+0x60`: the corners, in order round the edge.
    pub corners: [Vf; 4],
    /// `+0x70`: the plane's normal; `+0xc4`: the plane's distance along it.
    pub normal: Vf,
    pub plane: F,
    /// `+0x80`..`+0xb0`: each edge's plane's normal (edge `i` runs from corner `i`).
    pub edges: [Vf; 4],
}

/// A link in a cell's list (`+0x24`): the cell beyond (`+4`) and the portal on the way (`+8`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Link {
    pub cell: usize,
    pub portal: usize,
}

/// A cell: its links in list order, and its two sub-databases (`+0x28`, `+0x2c`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub links: Vec<Link>,
    pub subs: [Option<usize>; 2],
}

/// Where a slot is in a list: its block and its place in it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlotAt {
    pub block: usize,
    pub index: usize,
}

/// What a list's slot holds: an entry, or (tagged with bit 1) the next free slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Item(Item),
    Free(Option<SlotAt>),
}

/// The moving entries' sub-database: slots in blocks of `size`, every block full but the last, and a
/// chain of freed ones.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct List {
    /// `+8`: its kind, which a new cell's sub-database is picked by.
    pub kind: u32,
    /// `+0x28`: slots in a block; `+0x30`: slots handed out of the last one.
    pub size: i32,
    pub last: i32,
    /// `+0x34` (`+0x2c` of them): the blocks. A slot never handed out is `None`.
    pub blocks: Vec<Vec<Option<Slot>>>,
    /// `+0x38`: the first free slot.
    pub free: Option<SlotAt>,
}

/// Where an entry is in a sub-database (a record's `+0xc`, an extra's `+0xc`): a list's slot, or the
/// key a tree filed it under (0: in none of its nodes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Slot(SlotAt),
    Key(u32),
}

impl Default for Place {
    fn default() -> Place {
        Place::Key(0)
    }
}

impl Place {
    fn slot(self) -> SlotAt {
        match self {
            Place::Slot(s) => s,
            Place::Key(k) => panic!("a tree's key {k:#x} handed to a list"),
        }
    }

    fn key(self) -> u32 {
        match self {
            Place::Key(k) => k,
            Place::Slot(s) => panic!("a list's slot {s:?} handed to a tree"),
        }
    }
}

/// A tree node's chunk of entries: a link out of the tree's pool (`+0` the next chunk, `+4`..`+0x10`
/// the four entries, 0 an empty one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chunk {
    pub at: LinkAt,
    pub items: [Option<Item>; 4],
}

/// A tree's node (vtable `0x003b66e0`).
#[derive(Clone, Debug)]
pub struct Node {
    /// `+0`: its four children (by `x` bit, then `z` bit), once it has grown them: the original
    /// reallocates the node to 36 bytes and keeps them at its own `+0x14`.
    pub children: Option<[Option<usize>; 4]>,
    /// `+4`: its chunks, in chain order.
    pub chunks: Vec<Chunk>,
    /// `+8`, `+0xc`: the middle of the height its entries' boxes span, and half of it. A new node's
    /// are 0 and `-f32::MAX`.
    pub mid: F,
    pub half: F,
}

/// The quadtree sub-database (vtable `0x003b6700`): nodes over the ground plane in integer cells,
/// each holding the entries filed at its level and place.
#[derive(Clone, Debug)]
pub struct Tree {
    /// `+8`: its kind; `+0xc`: bit 0 a query is open.
    pub kind: u32,
    pub flags: u32,
    /// `+0x28`, `+0x30`: the world point at cell 0; `+0x40`, `+0x44`: how far it reaches from it.
    pub origin: [F; 2],
    pub extent: [F; 2],
    /// `+0x58`..: each level's node side in cells, down to the deepest (`+0x4c`); `+0x54`: cells per
    /// world unit.
    pub sides: Vec<i32>,
    pub scale: F,
    /// `+0x8c`: the root. The nodes are by index; a freed one is `None`.
    pub root: usize,
    pub nodes: Vec<Option<Node>>,
    /// `+0x90`: the pool the chunks come out of; `+0x94`: the open query's.
    pub pool: Pool,
    pub query: Pool,
    /// `+0x9c`, `+0xa0`, `+0xa4`, `+0xac`: where the open query's walk is, and `+0x8a0`..`+0x8ac` its
    /// cells. Only closing the query touches them here.
    pub walk: [u32; 4],
    pub cells: [u32; 4],
    /// `+0x880`: nodes made less nodes freed; `+0x888`: entries put in less entries taken out.
    pub count: i32,
    pub stamp: i32,
}

/// Trees are equal when their nodes are, met from the root: the nodes' numbers do not count.
impl PartialEq for Tree {
    fn eq(&self, o: &Tree) -> bool {
        let bits = |v: &[F]| v.iter().map(|f| f.to_bits()).collect::<Vec<_>>();
        (self.kind, self.flags, &self.sides, self.walk, self.cells, self.count, self.stamp)
            == (o.kind, o.flags, &o.sides, o.walk, o.cells, o.count, o.stamp)
            && bits(&[self.origin[0], self.origin[1], self.extent[0], self.extent[1], self.scale])
                == bits(&[o.origin[0], o.origin[1], o.extent[0], o.extent[1], o.scale])
            && (&self.pool, &self.query) == (&o.pool, &o.query)
            && self.same(self.root, o, o.root)
    }
}

/// A cell's sub-database.
#[derive(Clone, Debug, PartialEq)]
pub enum Sub {
    List(List),
    Tree(Tree),
    /// One of another class, or one not read: only its kind is.
    Other { kind: u32 },
}

/// Where a pool link is: its chunk (in the chain's order) and its place in it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LinkAt {
    pub chunk: usize,
    pub index: usize,
}

/// An extra cell an entry's box reaches into: the pool link (`+0` the next) and the cell (`+4`), the
/// sub-database (`+8`) and the place (`+0xc`) it is in there.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Extra {
    pub at: LinkAt,
    pub cell: usize,
    pub sub: usize,
    pub slot: Place,
}

/// The pool the extra links come out of (one for the whole database): chunks of `capacity` links
/// `stride` bytes apart, a chain of freed ones, and chunks set aside to take again.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pool {
    /// `+8`, `+0x1c`: links in a chunk and their size; `+0x18`: links handed out of the last chunk.
    pub capacity: i32,
    pub stride: u32,
    pub used: i32,
    /// `+0xc`..`+0x10`: the chunks in use; `+0x14`: those set aside.
    pub chunks: usize,
    pub spare: usize,
    /// `+0x24`: the freed links, the first to be taken last.
    pub free: Vec<LinkAt>,
}

/// An entry's record (the entry's `+8`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Record {
    /// `+0`: the extra cells, the newest first.
    pub extras: Vec<Extra>,
    /// `+4`: its cell (none: it is in no database, and a move does nothing); `+8`, `+0xc`: the
    /// sub-database and the place it is in there.
    pub cell: Option<usize>,
    pub sub: usize,
    pub slot: Place,
    /// `+0x10`: its centre when it last moved.
    pub last: Vf,
}

/// The database: its cells, their portals and sub-databases, and the link pool.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Database {
    pub cells: Vec<Cell>,
    pub portals: Vec<Portal>,
    pub subs: Vec<Sub>,
    pub pool: Pool,
    /// The entries a tree has put in: the original clears each one's query mark (the entry's
    /// first halfword), which the database does not keep.
    pub unmarked: BTreeSet<Item>,
}

fn sub_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

/// `n · v` on the FPU, `n` first in each product.
fn dot(n: Vf, v: Vf) -> F {
    Acc::mula(F(n.x()), F(v.x())).madda(F(n.y()), F(v.y())).madd(F(n.z()), F(v.z()))
}

/// Whether the segment from `a` to `b` passes through `p` (`atv_portal_crosses`): the two ends strictly on
/// either side of its plane (their distances' sign bits differ), the crossing between them, and the
/// crossing on the inner side of all four edges (the edge crossed with the way to the point, against
/// the normal, never sign-negative).
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn crosses(p: &Portal, a: Vf, b: Vf) -> bool {
    let na = dot(p.normal, a);
    let nb = dot(p.normal, b);
    let (sa, sb) = (na - p.plane, nb - p.plane);
    if (sa.to_bits() ^ sb.to_bits()) as i32 >= 0 {
        return false;
    }
    let t = (p.plane - na) / (nb - na);
    if t < F::ZERO || !(t <= F::ONE) {
        return false;
    }
    let bt = b.merge(Dest::XYZ, b.mul(Vf::splat(t.0)));
    let s = F::ONE - t;
    let a_s = a.merge(Dest::XYZ, a.mul(Vf::splat(s.0)));
    let at = a_s.merge(Dest::XYZ, a_s.add(bt));
    (0..4).all(|i| {
        let c = p.corners[i];
        let edge = sub_xyz(p.corners[(i + 1) % 4], c);
        let to = sub_xyz(at, c);
        let x = Vf::opmsub(Vf::opmula(edge, to), to, edge);
        (dot(p.normal, x).to_bits() as i32) >= 0
    })
}

/// Whether a sphere at `c` reaching `r` touches `p` (`atv_portal_touches`): within `r` of its plane on
/// either side, and no further than `r` outside any edge's plane.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn touches(p: &Portal, c: Vf, r: F) -> bool {
    let v = sub_xyz(c, p.corners[0]);
    let s = dot(v, p.normal);
    if !(s <= r) || s < F(fpu::neg(r.0)) {
        return false;
    }
    (0..4).all(|i| dot(sub_xyz(c, p.corners[i]), p.edges[i]) <= r)
}

impl Pool {
    /// A link: the last chunk's next one while it has any, else the last one freed, else the first
    /// of a new chunk (one set aside if there is one).
    fn take(&mut self) -> LinkAt {
        if self.used < self.capacity {
            let at = LinkAt { chunk: self.chunks - 1, index: self.used as usize };
            self.used += 1;
            return at;
        }
        if let Some(at) = self.free.pop() {
            return at;
        }
        self.spare = self.spare.saturating_sub(1);
        self.chunks += 1;
        self.used = 1;
        LinkAt { chunk: self.chunks - 1, index: 0 }
    }

    /// A link handed back; a pool of links no bigger than the chain's word keeps none.
    fn give(&mut self, at: LinkAt) {
        if 4 < self.stride {
            self.free.push(at);
        }
    }

    /// Every link handed back at once (`atv_pool_reset`): the chunks in use set aside, in front of those
    /// set aside already, and the free chain dropped.
    fn reset(&mut self) {
        self.spare += self.chunks;
        self.chunks = 0;
        self.used = self.capacity;
        self.free.clear();
    }
}

/// The node side the walk down a key starts from (the root's children's), in cells.
const FIRST: i32 = 0x4000;

impl Tree {
    /// A tree with only its root, of no height, and no query open.
    pub fn new(kind: u32, origin: [F; 2], extent: [F; 2], sides: Vec<i32>, scale: F, pool: Pool, query: Pool) -> Tree {
        let mut t = Tree {
            kind,
            flags: 0,
            origin,
            extent,
            sides,
            scale,
            root: 0,
            nodes: vec![],
            pool,
            query,
            walk: [0; 4],
            cells: [0x8000_0000; 4],
            count: 0,
            stamp: 0,
        };
        t.root = t.make();
        t
    }

    fn same(&self, a: usize, o: &Tree, b: usize) -> bool {
        let (x, y) = (self.node(a), o.node(b));
        let kids = match (&x.children, &y.children) {
            (None, None) => true,
            (Some(p), Some(q)) => p.iter().zip(q).all(|pair| match pair {
                (None, None) => true,
                (Some(c), Some(d)) => self.same(*c, o, *d),
                _ => false,
            }),
            _ => false,
        };
        kids && x.chunks == y.chunks && (x.mid.to_bits(), x.half.to_bits()) == (y.mid.to_bits(), y.half.to_bits())
    }

    pub fn node(&self, n: usize) -> &Node {
        self.nodes[n].as_ref().expect("a freed node in the tree")
    }

    fn node_mut(&mut self, n: usize) -> &mut Node {
        self.nodes[n].as_mut().expect("a freed node in the tree")
    }

    /// vt `+0x4c`: a new node, counted.
    fn make(&mut self) -> usize {
        self.count = self.count.wrapping_add(1);
        self.nodes.push(Some(Node { children: None, chunks: vec![], mid: F::ZERO, half: F(-f32::MAX) }));
        self.nodes.len() - 1
    }

    /// vt `+0x40`: an open query closed (its pool's links all handed back, its walk and cells
    /// cleared).
    fn close(&mut self) {
        if self.flags & 1 != 0 {
            self.query.reset();
            self.walk = [0; 4];
            self.cells = [0x8000_0000; 4];
            self.flags &= !1;
        }
    }

    /// Node `n`'s height widened to take in `lo`..`hi` (nothing when `hi` is below `lo`).
    #[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
    fn widen(&mut self, n: usize, lo: F, hi: F) {
        if !(lo <= hi) {
            return;
        }
        let node = self.node_mut(n);
        let mut a = node.mid - node.half;
        if lo < a {
            a = lo;
        }
        let mut b = node.mid + node.half;
        if !(hi <= b) {
            b = hi;
        }
        node.mid = (a + b) / F(2.0);
        node.half = (b - a) / F(2.0);
    }

    /// A key's level, its corner cell (`x`, `z`) and that level's node side.
    fn parts(&self, key: u32) -> (usize, i32, i32, i32) {
        let level = (key & 0xf) as usize;
        let x = ((key & 0xfff0_0000) >> 17) as i32;
        let z = ((key & 0xfff00) >> 5) as i32;
        (level, x, z, self.sides[level])
    }

    /// Which child the cell (`x`, `z`) is under at the level whose bit is `bit`.
    fn child(x: i32, z: i32, bit: i32) -> usize {
        2 * usize::from(x & bit != 0) + usize::from(z & bit != 0)
    }

    /// The corner cells a key names: 0x10 its own, 0x20 the next in `z`, 0x40 in `x`, 0x80 both.
    fn corners(key: u32, x: i32, z: i32, size: i32) -> impl Iterator<Item = (i32, i32)> {
        (0..4).filter(move |c| key & (0x10 << c) != 0).map(move |c| {
            let x = x.wrapping_add(if c & 2 != 0 { size } else { 0 });
            let z = z.wrapping_add(if c & 1 != 0 { size } else { 0 });
            (x, z)
        })
    }

    /// vt `+0x20`: the key the box `centre` ± `half` is filed under, 0 unless it lies wholly in the
    /// tree. The box's corners less the origin, in cells (`x1`, `z1` one past); the first level whose
    /// node side is more than the box both ways, or the deepest; the corners at that side, and a
    /// level up (corners at twice the side) if they span more than one node; then the level (bits
    /// 0..4), the corners it spills into (0x10 always, 0x20 in `z`, 0x40 in `x`, 0x80 both) and the
    /// corner cell, `z` from bit 5 and `x` from bit 17, each at most 0x7fff.
    #[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
    pub fn pick(&self, centre: Vf, half: Vf) -> u32 {
        let [ox, oz] = self.origin;
        let [ex, ez] = self.extent;
        let (cx, cz, hx, hz) = (F(centre.x()), F(centre.z()), F(half.x()), F(half.z()));
        let (x0, x1) = ((cx - hx) - ox, (cx + hx) - ox);
        let (z0, z1) = ((cz - hz) - oz, (cz + hz) - oz);
        if x1 < F::ZERO
            || !(x0 <= ex)
            || z1 < F::ZERO
            || !(z0 <= ez)
            || !(x1 <= ex)
            || x0 < F::ZERO
            || !(z1 <= ez)
            || z0 < F::ZERO
        {
            return 0;
        }
        let s = self.scale;
        let (mut x0, mut z0) = ((x0 * s).to_i32(), (z0 * s).to_i32());
        let (mut x1, mut z1) = ((x1 * s).to_i32().wrapping_add(1), (z1 * s).to_i32().wrapping_add(1));
        let (width, depth) = (x1.wrapping_sub(x0), z1.wrapping_sub(z0));
        let deepest = self.sides.len() as i32 - 1;
        let mut level = 0;
        if 0 <= deepest {
            while width < self.sides[level as usize] && depth < self.sides[level as usize] {
                level += 1;
                if deepest < level {
                    break;
                }
            }
        }
        let level = level.min(deepest) as usize;
        let size = self.sides[level];
        let m = !(size.wrapping_sub(1));
        (x0, x1, z0, z1) = (x0 & m, x1 & m, z0 & m, z1 & m);
        let mut level = level as u32;
        if (size < x1.wrapping_sub(x0) || size < z1.wrapping_sub(z0)) && level != 0 {
            level -= 1;
            let m = !(size.wrapping_add(size).wrapping_sub(1));
            (x0, x1, z0, z1) = (x0 & m, x1 & m, z0 & m, z1 & m);
        }
        let mut corners = 0x10;
        if z1 != z0 {
            corners |= 0x20;
        }
        if x1 != x0 {
            corners |= 0x40;
        }
        if corners & 0x60 == 0x60 {
            corners |= 0x80;
        }
        let x0 = if x0 < 0x8000 { x0 } else { 0x7fff };
        let z0 = if z0 < 0x8000 { z0 } else { 0x7fff };
        level | corners | (z0 as u32) << 5 | (x0 as u32) << 17
    }

    /// vt `+0x24`: `item` filed under `key` with the box `centre` ± `half` (nothing for a key of
    /// 0). The open query is closed first. Under each corner the key names: the root and every node
    /// down to the key's level widened to the box's height, a node with no children grown them and
    /// a missing child made on the way; then the item in the node's first empty slot, or a chunk
    /// from the pool at the end of its chain.
    pub fn put(&mut self, item: Item, key: u32, centre: Vf, half: Vf) {
        if key == 0 {
            return;
        }
        self.close();
        let (lo, hi) = (F(centre.y()) - F(half.y()), F(centre.y()) + F(half.y()));
        let (level, x, z, size) = self.parts(key);
        self.stamp = self.stamp.wrapping_add(1);
        for (x, z) in Tree::corners(key, x, z, size) {
            let mut node = self.root;
            self.widen(node, lo, hi);
            let mut bit = FIRST;
            for _ in 0..=level {
                let i = Tree::child(x, z, bit);
                let child = match self.node(node).children {
                    Some(c) => c[i],
                    None => {
                        self.node_mut(node).children = Some([None; 4]);
                        None
                    }
                };
                let child = child.unwrap_or_else(|| {
                    let c = self.make();
                    self.node_mut(node).children.as_mut().expect("grown")[i] = Some(c);
                    c
                });
                self.widen(child, lo, hi);
                node = child;
                bit >>= 1;
            }
            self.file(node, item);
        }
    }

    /// `atv_quadtree_file`: `item` in node `n`'s first empty slot, else a new chunk's first at the end.
    fn file(&mut self, n: usize, item: Item) {
        let node = self.nodes[n].as_mut().expect("a freed node in the tree");
        for c in &mut node.chunks {
            if let Some(s) = c.items.iter_mut().find(|s| s.is_none()) {
                *s = Some(item);
                return;
            }
        }
        let at = self.pool.take();
        node.chunks.push(Chunk { at, items: [Some(item), None, None, None] });
    }

    /// vt `+0x28`: `item`, still under `key`, its box now `centre` ± `half`: the nodes down to each
    /// corner widened to its height. Nothing else changes, and the query stays open.
    #[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
    pub fn moved(&mut self, key: u32, centre: Vf, half: Vf) {
        let (lo, hi) = (F(centre.y()) - F(half.y()), F(centre.y()) + F(half.y()));
        if key == 0 || !(lo <= hi) {
            return;
        }
        let (level, x, z, size) = self.parts(key);
        for (x, z) in Tree::corners(key, x, z, size) {
            let mut node = self.root;
            self.widen(node, lo, hi);
            let mut bit = FIRST;
            for _ in 0..=level {
                let Some(c) = self.node(node).children else { break };
                let child = c[Tree::child(x, z, bit)].expect("a moved entry's key leads to a missing node");
                self.widen(child, lo, hi);
                node = child;
                bit >>= 1;
            }
        }
    }

    /// vt `+0x2c`: `item` taken out from under `key` (nothing for a key of 0). The open query is
    /// closed first. Under each corner, if the nodes down to the key's level are all there, the item
    /// is taken out of the last; a node left with no entries and no children is freed and its
    /// parent's child cleared. The nodes' heights are left as they are.
    pub fn take_out(&mut self, item: Item, key: u32) {
        if key == 0 {
            return;
        }
        self.close();
        let (level, x, z, size) = self.parts(key);
        self.stamp = self.stamp.wrapping_sub(1);
        for (x, z) in Tree::corners(key, x, z, size) {
            let (mut node, mut parent, mut i) = (Some(self.root), self.root, 0);
            let mut bit = FIRST;
            for _ in 0..=level {
                let Some(n) = node else { break };
                i = Tree::child(x, z, bit);
                parent = n;
                node = self.node(n).children.and_then(|c| c[i]);
                bit >>= 1;
            }
            let Some(n) = node else { continue };
            if !self.unfile(n, item) {
                self.count = self.count.wrapping_sub(1);
                self.nodes[n] = None;
                self.node_mut(parent).children.as_mut().expect("a parent has children")[i] = None;
            }
        }
    }

    /// `atv_quadtree_unfile`: `item` cleared from the first of node `n`'s chunks that holds it (every slot of
    /// that chunk holding it), the chunk freed to the pool if that left it empty. Whether the node
    /// still holds anything: an entry, or a child.
    ///
    /// A chunk with no entries at all is freed as it is met; the tree never leaves one.
    fn unfile(&mut self, n: usize, item: Item) -> bool {
        let node = self.nodes[n].as_mut().expect("a freed node in the tree");
        let mut k = 0;
        while k < node.chunks.len() {
            let c = &mut node.chunks[k];
            let mut found = false;
            for s in &mut c.items {
                if *s == Some(item) {
                    *s = None;
                    found = true;
                }
            }
            if c.items.iter().all(Option::is_none) {
                let at = node.chunks.remove(k).at;
                self.pool.give(at);
            } else {
                k += 1;
            }
            if found {
                break;
            }
        }
        !node.chunks.is_empty() || node.children.is_some_and(|c| c.iter().any(Option::is_some))
    }
}

impl List {
    /// vt `+0x20`: the slot an entry goes in — the one it has, else the first free one, else the
    /// last block's next (a new block when it is full).
    fn pick(&mut self, current: Option<SlotAt>) -> SlotAt {
        if let Some(s) = current {
            return s;
        }
        if let Some(f) = self.free {
            return f;
        }
        if self.last == self.size {
            self.blocks.push(vec![None; usize::try_from(self.size).unwrap_or(0)]);
            self.last = 0;
        }
        SlotAt { block: self.blocks.len() - 1, index: self.last as usize }
    }

    fn slot(&mut self, s: SlotAt) -> &mut Option<Slot> {
        &mut self.blocks[s.block][s.index]
    }

    /// vt `+0x24`: `item` put in `s`, the first free slot or the last block's next.
    fn put(&mut self, item: Item, s: SlotAt) {
        if self.free.is_some() && self.free == Some(s) {
            self.free = match *self.slot(s) {
                Some(Slot::Free(next)) => next,
                _ => None,
            };
        } else {
            self.last += 1;
        }
        *self.slot(s) = Some(Slot::Item(item));
    }

    /// vt `+0x2c`: slot `s` freed, first on the chain.
    fn take_out(&mut self, s: SlotAt) {
        *self.slot(s) = Some(Slot::Free(self.free));
        self.free = Some(s);
    }
}

impl Sub {
    fn kind(&self) -> u32 {
        match self {
            Sub::List(l) => l.kind,
            Sub::Tree(t) => t.kind,
            Sub::Other { kind } => *kind,
        }
    }
}

impl Database {
    /// `atv_cell_sub_of`: a cell's sub-database for `kind` — bit 0 the first, else bit 1 the second.
    fn sub_of(&self, cell: usize, kind: u32) -> usize {
        let c = &self.cells[cell];
        let s = if kind & 1 != 0 {
            c.subs[0]
        } else if kind & 2 != 0 {
            c.subs[1]
        } else {
            None
        };
        s.expect("a cell without the sub-database of its entry's kind")
    }

    fn other(sub: usize) -> ! {
        panic!("a moving entry in sub-database {sub}, one not read (Sub::Other)")
    }

    /// vt `+0x20`: the place for the box — a list's: the slot the entry has (`current`) if any; a
    /// tree's: the box's key.
    fn pick(&mut self, sub: usize, current: Option<Place>, centre: Vf, half: Vf) -> Place {
        match &mut self.subs[sub] {
            Sub::List(l) => Place::Slot(l.pick(current.map(Place::slot))),
            Sub::Tree(t) => Place::Key(t.pick(centre, half)),
            Sub::Other { .. } => Database::other(sub),
        }
    }

    /// vt `+0x24`.
    fn put(&mut self, sub: usize, item: Item, at: Place, centre: Vf, half: Vf) {
        match &mut self.subs[sub] {
            Sub::List(l) => l.put(item, at.slot()),
            Sub::Tree(t) => {
                if at.key() != 0 {
                    self.unmarked.insert(item);
                }
                t.put(item, at.key(), centre, half);
            }
            Sub::Other { .. } => Database::other(sub),
        }
    }

    /// vt `+0x2c`.
    fn take_out(&mut self, sub: usize, item: Item, at: Place) {
        match &mut self.subs[sub] {
            Sub::List(l) => l.take_out(at.slot()),
            Sub::Tree(t) => t.take_out(item, at.key()),
            Sub::Other { .. } => Database::other(sub),
        }
    }

    /// vt `+0x20`, then vt `+0x28` (a list's move does nothing) if the place is the same, else vt
    /// `+0x2c` and `+0x24`. The place it is in after.
    fn shift(&mut self, sub: usize, item: Item, at: Place, centre: Vf, half: Vf) -> Place {
        let to = self.pick(sub, Some(at), centre, half);
        if to == at {
            if let Sub::Tree(t) = &mut self.subs[sub] {
                t.moved(at.key(), centre, half);
            }
        } else {
            self.take_out(sub, item, at);
            self.put(sub, item, to, centre, half);
        }
        to
    }

    /// `atv_spatial_reach`: the cells beyond `cell`'s links (never `home`, nor one gathered already) whose
    /// portal the sphere touches, each followed by those beyond it.
    fn reach(&self, home: usize, cell: usize, centre: Vf, r: F, out: &mut Vec<usize>) {
        for l in &self.cells[cell].links {
            if l.cell == home || out.contains(&l.cell) {
                continue;
            }
            if touches(&self.portals[l.portal], centre, r) {
                out.push(l.cell);
                self.reach(home, l.cell, centre, r, out);
            }
        }
    }

    /// `atv_spatial_overlaps`: the extra cells brought up to date with the cells the sphere reaches.
    /// A newly reached cell whose sub-database has no place for the box (a tree's key 0) is not
    /// entered, though its sub-database's kind is still carried.
    fn overlaps(&mut self, rec: &mut Record, item: Item, centre: Vf, half: Vf, r: F) {
        let Some(home) = rec.cell else { return };
        let mut found = Vec::new();
        self.reach(home, home, centre, r, &mut found);
        let mut cells: Vec<Option<usize>> = found.into_iter().map(Some).collect();
        // The kind a newly reached cell's sub-database is picked by is the last one touched's.
        let mut kind_of = rec.sub;
        let mut i = 0;
        while i < rec.extras.len() {
            let e = rec.extras[i];
            if let Some(k) = cells.iter().position(|&c| c == Some(e.cell)) {
                kind_of = e.sub;
                rec.extras[i].slot = self.shift(e.sub, item, e.slot, centre, half);
                cells[k] = None;
                i += 1;
            } else {
                self.take_out(e.sub, item, e.slot);
                rec.extras.remove(i);
                self.pool.give(e.at);
            }
        }
        for c in cells.into_iter().flatten() {
            let sub = self.sub_of(c, self.subs[kind_of].kind());
            kind_of = sub;
            let slot = self.pick(sub, None, centre, half);
            if slot == Place::Key(0) {
                continue;
            }
            self.put(sub, item, slot, centre, half);
            let at = self.pool.take();
            rec.extras.insert(0, Extra { at, cell: c, sub, slot });
        }
    }
}

/// **`atv_spatial_move`**: `item`'s record moved to the box at `centre`, `half` either way, with the
/// sphere reaching `reach` for the overlaps.
pub fn spatial_move(db: &mut Database, rec: &mut Record, item: Item, centre: Vf, half: Vf, reach: F) {
    let Some(home) = rec.cell else { return };
    let (mut cell, mut from) = (home, None);
    'walk: loop {
        for l in &db.cells[cell].links {
            if Some(l.cell) != from && crosses(&db.portals[l.portal], rec.last, centre) {
                (from, cell) = (Some(cell), l.cell);
                continue 'walk;
            }
        }
        break;
    }
    if cell == home {
        rec.slot = db.shift(rec.sub, item, rec.slot, centre, half);
    } else {
        db.take_out(rec.sub, item, rec.slot);
        for e in std::mem::take(&mut rec.extras) {
            db.take_out(e.sub, item, e.slot);
            db.pool.give(e.at);
        }
        rec.cell = Some(cell);
        rec.sub = db.sub_of(cell, db.subs[rec.sub].kind());
        rec.slot = db.pick(rec.sub, None, centre, half);
        db.put(rec.sub, item, rec.slot, centre, half);
    }
    db.overlaps(rec, item, centre, half, reach);
    rec.last = centre;
}

/// The quad's entry and its node's, as a [`Space`]'s sub-databases hold them.
pub const QUAD: Item = 0;
pub const NODE: Item = 1;
/// MD75c2: the first of the quad's collidables' entries, each at `PARTS +` its
/// [`crate::atv_sync::slot`].
pub const PARTS: Item = 2;

/// The database and the two records [`crate::quad::Call::SpatialMove`] moves: the quad's own entry
/// (`+8` of the quad) and its node's. MD75c2's: the records [`crate::quad::Call::PartMove`] moves, each
/// collidable's (its `+0x20`), by [`crate::atv_sync::slot`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Space {
    pub db: Database,
    pub quad: Record,
    pub node: Record,
    pub parts: [Record; 5],
}

/// Whoever runs the quad's calls with [`crate::quad::Call::SpatialMove`] answered here, in `space`
/// (the quad as [`QUAD`], its node as [`NODE`]), and [`crate::quad::Call::PartMove`] (MD75c2, from
/// [`PARTS`] on); every other call goes on to `rest`.
#[derive(Clone, Debug, Default)]
pub struct Answers<R> {
    pub space: Space,
    pub rest: R,
}

impl<R: crate::quad::Moto> crate::quad::Moto for Answers<R> {
    fn call(&mut self, c: crate::quad::Call, q: &mut crate::quad::Quad, body: &mut crate::quad::Body) {
        match c {
            crate::quad::Call::SpatialMove { node, centre, half, reach } => {
                let s = &mut self.space;
                let (rec, item) = if node { (&mut s.node, NODE) } else { (&mut s.quad, QUAD) };
                spatial_move(&mut s.db, rec, item, centre, half, reach);
            }
            crate::quad::Call::PartMove { part, centre, half, reach } => {
                let s = &mut self.space;
                let k = crate::atv_sync::slot(part);
                spatial_move(&mut s.db, &mut s.parts[k], PARTS + k, centre, half, reach);
            }
            c => self.rest.call(c, q, body),
        }
    }
}
