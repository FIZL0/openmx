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
//! The only sub-database ported is the one every cell of MD37a's image holds for moving entries (its
//! second), a **list** of slots in blocks with a free chain. A cell's first, the static quadtree
//! (`broad_phase`'s, which the gather reads), never takes a moving entry there and is [`Sub::Other`].

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use ps2_float::vu::Dest;
use ps2_float::{F, Vf, fpu, fpu::Acc};

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

/// A cell's sub-database.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sub {
    List(List),
    /// One of another class (the static quadtree): only its kind is read here.
    Other { kind: u32 },
}

/// Where a pool link is: its chunk (in the chain's order) and its place in it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LinkAt {
    pub chunk: usize,
    pub index: usize,
}

/// An extra cell an entry's box reaches into: the pool link (`+0` the next) and the cell (`+4`), the
/// sub-database (`+8`) and the slot (`+0xc`) it is in there.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Extra {
    pub at: LinkAt,
    pub cell: usize,
    pub sub: usize,
    pub slot: SlotAt,
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
    /// sub-database and the slot it is in there.
    pub cell: Option<usize>,
    pub sub: usize,
    pub slot: SlotAt,
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
            Sub::Other { kind } => *kind,
        }
    }

    fn list(&mut self) -> &mut List {
        match self {
            Sub::List(l) => l,
            Sub::Other { .. } => panic!("a moving entry in a cell's static sub-database: only lists are ported (MD75c1)"),
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

    fn pick(&mut self, sub: usize, current: Option<SlotAt>) -> SlotAt {
        self.subs[sub].list().pick(current)
    }

    /// vt `+0x20`, then vt `+0x28` (a list's move does nothing) if the slot is the same, else vt
    /// `+0x2c` and `+0x24`. The slot it is in after.
    fn shift(&mut self, sub: usize, item: Item, slot: SlotAt) -> SlotAt {
        let to = self.pick(sub, Some(slot));
        if to != slot {
            let l = self.subs[sub].list();
            l.take_out(slot);
            l.put(item, to);
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
    fn overlaps(&mut self, rec: &mut Record, item: Item, centre: Vf, r: F) {
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
                rec.extras[i].slot = self.shift(e.sub, item, e.slot);
                cells[k] = None;
                i += 1;
            } else {
                self.subs[e.sub].list().take_out(e.slot);
                rec.extras.remove(i);
                self.pool.give(e.at);
            }
        }
        for c in cells.into_iter().flatten() {
            let sub = self.sub_of(c, self.subs[kind_of].kind());
            kind_of = sub;
            let slot = self.pick(sub, None);
            self.subs[sub].list().put(item, slot);
            let at = self.pool.take();
            rec.extras.insert(0, Extra { at, cell: c, sub, slot });
        }
    }
}

/// **`atv_spatial_move`**: `item`'s record moved to the box at `centre`, `half` either way, with the
/// sphere reaching `reach` for the overlaps.
pub fn spatial_move(db: &mut Database, rec: &mut Record, item: Item, centre: Vf, half: Vf, reach: F) {
    let _ = half; // handed to every sub-database call; a list reads none of it
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
        rec.slot = db.shift(rec.sub, item, rec.slot);
    } else {
        db.subs[rec.sub].list().take_out(rec.slot);
        for e in std::mem::take(&mut rec.extras) {
            db.subs[e.sub].list().take_out(e.slot);
            db.pool.give(e.at);
        }
        rec.cell = Some(cell);
        rec.sub = db.sub_of(cell, db.subs[rec.sub].kind());
        rec.slot = db.pick(rec.sub, None);
        db.subs[rec.sub].list().put(item, rec.slot);
    }
    db.overlaps(rec, item, centre, reach);
    rec.last = centre;
}

/// The quad's entry and its node's, as a [`Space`]'s sub-databases hold them.
pub const QUAD: Item = 0;
pub const NODE: Item = 1;

/// The database and the two records [`crate::quad::Call::SpatialMove`] moves: the quad's own entry
/// (`+8` of the quad) and its node's.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Space {
    pub db: Database,
    pub quad: Record,
    pub node: Record,
}

/// Whoever runs the quad's calls with [`crate::quad::Call::SpatialMove`] answered here, in `space`
/// (the quad as [`QUAD`], its node as [`NODE`]); every other call goes on to `rest`.
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
            c => self.rest.call(c, q, body),
        }
    }
}
