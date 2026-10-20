//! The broad phase: which collidables are near enough to a probe to be worth a narrow phase.
//!
//! The world keeps its collidables in two structures per *table* ([`Tables`]): a quadtree of the static
//! ones ([`Quadtree`], placed groups and fixed meshes, each in the smallest cell that holds it) and a
//! block list of the moving ones ([`Blocks`]). A collidable belongs to a chain of *spaces* ([`Space`]),
//! each naming a table; a [`Query`] walks the structures of every table on the chain, then the global
//! table, handing out each collidable once (a 16-bit stamp, bumped per query, marks the ones already
//! handed out).
//!
//! Two users, both ported here:
//!
//! - [`gather`]: a collision object's nearby search ([`Search`], the [`Nearby`] list S44 searches)
//!   filled around a probe — everything the pair filter lets through within a *bound*, the distance the
//!   query's cells are known to cover. While the probe stays inside that bound the list is kept and only
//!   its distances are refreshed ([`resort`]).
//! - [`probe_collide_world`]: a probe tested straight against the world, for a collision object without
//!   a nearby search: the static structures with a box, then every moving object.
//!
//! What the classes do — a probe's box, its narrow phase, a group's parts, listeners — comes in through
//! [`Probe`] (the bounds) and S44's [`Narrow`].

#![deny(clippy::float_arithmetic)]

use ps2_float::{Acc, F, fpu, vu};

use crate::world_search::{self, Candidate, Class, Collidable, Narrow, Nearby, accepts, flags};

/// `+0x116` bits the broad phase reads beyond [`flags`].
pub mod more_flags {
    /// On a probe: it collides at all. Without it [`super::probe_collide_world`] only lowers the hit bit.
    pub const ACTIVE: u8 = 0x02;
    /// On a candidate: it is gathered only within the search's bound, not anywhere its cells were.
    pub const WITHIN_BOUND: u8 = 0x08;
}

/// What the broad phase reads of a collidable beyond S44's [`Collidable`]. For a placed group only the
/// cell, the model and the blend mean anything.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Placement {
    /// `+0`: the stamp of the last query that handed it out.
    pub stamp: u16,
    /// `+4`: the first of the spaces it is in.
    pub space: Option<usize>,
    /// `+0x40`: where it is; `+0x50`: where it was when it last collided against the world.
    pub centre: [F; 4],
    pub last: [F; 4],
    /// `+0x64`: how far it may still move before it must collide against the world again.
    pub budget: F,
    /// `+8`, `+0xc`: a placed group's cell; `+0xe` its model; `+0x11` how far its reach lies between the
    /// model's two (in 128ths).
    pub cell: [u16; 2],
    pub model: u8,
    pub blend: i8,
}

/// A model the world places, as the broad phase sees it: its reach at blend 0 (`+0x10c`) and at 128
/// (`+0x114`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Model {
    pub near: F,
    pub far: F,
}

/// One link of a collidable's chain of spaces: the table it names (`+4`) and the next link (`+0`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Space {
    pub next: Option<usize>,
    pub tables: usize,
}

/// A pair of structures: the static one (`+0x34`) and the moving one (`+0x38`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pair {
    pub fixed: Option<usize>,
    pub moving: Option<usize>,
}

/// A table of the world: its two structures and its sub-tables (`+0x44`, `+0x48`), whose static
/// structures a query walks after its own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tables {
    pub own: Pair,
    pub subs: Vec<Pair>,
}

/// A quadtree node: its four children and its chunks of four collidables (an empty slot is `None`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Node {
    pub children: [Option<usize>; 4],
    pub chunks: Vec<[Option<usize>; 4]>,
}

/// The static structure: a quadtree over the world's ground plane, in integer cells.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Quadtree {
    /// `+0x14`: bit 0 a query is open, bit 1 it reports the box its cells cover.
    pub flags: u32,
    /// `+0x30`, `+0x38`: the world point at cell 0; `+0x48`, `+0x4c`: how far the tree reaches from it.
    pub origin: [F; 2],
    pub extent: [F; 2],
    /// `+0x58`: the smallest node's size in cells (a power of two); `+0x5c` cells per world unit.
    pub cell: i32,
    pub scale: F,
    pub root: Option<usize>,
    pub nodes: Vec<Node>,
    /// `+0x898`: while set, the open query hands out nothing. Only a new query clears it.
    pub busy: u32,
    /// `+0x8a8`..: the open query's cells, `x0, z0, x1, z1`.
    pub cells: [i32; 4],
    /// The nodes the open query gathered, in the order it walks them, and where it is: the node
    /// (`+0xa4` the first, `+0xa8` the current), the chunk (`+0xb0`) and the slot in it (`+0xac`).
    pub list: Vec<usize>,
    pub head: Option<usize>,
    pub at: Option<usize>,
    pub chunk: Option<(usize, usize)>,
    pub slot: usize,
}

/// The moving structure: blocks of collidable slots, every block full but the last.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Blocks {
    /// `+0x14`, as [`Quadtree::flags`].
    pub flags: u32,
    /// `+0x30`: slots in a block; `+0x38`: slots used in the last one.
    pub size: i32,
    pub last: i32,
    /// `+0x3c`/`+0x34`: the blocks, if there is an array at all. A free slot is `None`.
    pub blocks: Option<Vec<Vec<Option<usize>>>>,
    /// `+0x44`, `+0x48`: where the walk is.
    pub block: i32,
    pub entry: i32,
}

/// One of the world's structures.
#[derive(Clone, Debug, PartialEq)]
pub enum Structure {
    Tree(Quadtree),
    List(Blocks),
}

/// Everything the broad phase reads of the world.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Broad {
    pub spaces: Vec<Space>,
    pub tables: Vec<Tables>,
    pub structures: Vec<Structure>,
    /// The table every global query ends with, if any.
    pub global: Option<usize>,
    /// The stamp queries hand collidables out under.
    pub stamp: u16,
    /// The quadtree last queried.
    pub current: Option<usize>,
    /// World units per tree cell, for the box a quadtree reports.
    pub cell_size: F,
    /// World units per placed-group cell, and the placed models.
    pub group_cell: F,
    pub models: Vec<Model>,
    /// Per collidable, beside the [`Collidable`] slice.
    pub places: Vec<Placement>,
}

/// The `+0xa4` slot a probe's class adds to [`Narrow`] for [`probe_collide_world`].
pub trait Probe: Narrow {
    /// The probe's box: its centre and its half extents.
    fn bounds(&mut self, objects: &mut [Collidable], probe: usize) -> ([F; 4], [F; 4]);
}

/// A box a structure reports, `lo` and `hi`; `w` is left alone.
type Bounds<'a> = Option<(&'a mut [F; 4], &'a mut [F; 4])>;

fn sub3(a: [F; 4], b: [F; 4]) -> [F; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `vsub.xyz`.
fn vsub3(a: [F; 4], b: [F; 4]) -> [F; 3] {
    [0, 1, 2].map(|i| F(vu::sub(a[i].0, b[i].0)))
}

fn length(d: [F; 3]) -> F {
    Acc::mula(d[0], d[0]).madda(d[1], d[1]).madd(d[2], d[2]).sqrt()
}

fn flat_length(dx: F, dz: F) -> F {
    Acc::mula(dx, dx).madd(dz, dz).sqrt()
}

/// How far `objects[o]`'s reach, grown by `extra`, is from `p`; never below 0.
pub fn collidable_distance(extra: F, objects: &[Collidable], broad: &Broad, o: usize, p: [F; 4]) -> F {
    let reach = objects[o].radius + extra;
    let d = length(sub3(broad.places[o].centre, p)) - reach;
    F::ZERO.max(d)
}

/// How far apart the reaches of `a` and `b` are; never below 0.
pub fn pair_distance(objects: &[Collidable], broad: &Broad, a: usize, b: usize) -> F {
    let reach = objects[a].radius + objects[b].radius;
    let d = length(sub3(broad.places[a].centre, broad.places[b].centre)) - reach;
    F::ZERO.max(d)
}

/// How far `probe`'s reach, grown by `r`, is from `p` on the ground plane; never below 0.
pub fn point_distance(r: F, objects: &[Collidable], broad: &Broad, probe: usize, p: [F; 4]) -> F {
    let c = broad.places[probe].centre;
    let s = flat_length(c[0] - p[0], c[2] - p[2]);
    F::ZERO.max(s - (objects[probe].radius + r))
}

impl Broad {
    /// A placed group's reach: its model's, blended.
    fn group_reach(&self, g: usize) -> F {
        let p = &self.places[g];
        let m = self.models[usize::from(p.model)];
        m.near + F::from(i32::from(p.blend)) * (m.far - m.near) / 128.0
    }

    /// A placed group's point on the ground plane.
    fn group_point(&self, g: usize) -> [F; 2] {
        let c = self.places[g].cell;
        [F::from(i32::from(c[0])) * self.group_cell, F::from(i32::from(c[1])) * self.group_cell]
    }

    /// How far a placed group's reach is from `p`, as the gather measures it: 0 when inside.
    fn group_distance(&self, g: usize, p: [F; 4]) -> F {
        let [x, z] = self.group_point(g);
        let s = flat_length(p[0] - x, p[2] - z);
        let d = s - (F::ZERO + self.group_reach(g));
        if d <= 0.0 { F::ZERO } else { d }
    }
}

impl Quadtree {
    /// Begin a query of the box `centre` ± `half`: the cells it covers, the box they cover reported in
    /// `bounds`, and the nodes they reach gathered. The same cells again only restart the open query.
    /// False, with nothing reported, for a box off the tree.
    fn begin(&mut self, me: usize, g: &mut Globals, centre: [F; 4], half: [F; 4], bounds: Bounds) -> bool {
        let x0 = (centre[0] - half[0]) - self.origin[0];
        let x1 = (centre[0] + half[0]) - self.origin[0];
        let z0 = (centre[2] - half[2]) - self.origin[1];
        let z1 = (centre[2] + half[2]) - self.origin[1];
        let (lo_x, hi_x) = if x1 < x0 { (x1, x0) } else { (x0, x1) };
        let (lo_z, hi_z) = if z1 < z0 { (z1, z0) } else { (z0, z1) };
        if hi_x < 0.0 || !(lo_x <= self.extent[0]) || hi_z < 0.0 || !(lo_z <= self.extent[1]) {
            return false;
        }
        let mask = -self.cell;
        let cx0 = (lo_x * self.scale).to_i32() & mask;
        let cz0 = (lo_z * self.scale).to_i32() & mask;
        let cx1 = (self.cell - 1).wrapping_add((hi_x * self.scale).to_i32() & mask);
        let cz1 = (self.cell - 1).wrapping_add((hi_z * self.scale).to_i32() & mask);
        if let Some((lo, hi)) = bounds {
            lo[0] = F::from(cx0) * g.cell_size;
            lo[1] = F(fpu::NEG_MAX);
            lo[2] = F::from(cz0) * g.cell_size;
            hi[0] = F::from(cx1) * g.cell_size;
            hi[1] = F::MAX;
            hi[2] = F::from(cz1) * g.cell_size;
        }
        if self.flags & 1 != 0 {
            if self.cells == [cx0, cz0, cx1, cz1] {
                self.restart(g);
                return true;
            }
            self.end();
        }
        self.busy = 0;
        self.flags |= 1;
        *g.stamp = g.stamp.wrapping_add(1);
        self.cells = [cx0, cz0, cx1, cz1];
        *g.current = Some(me);
        self.at = None;
        self.chunk = None;
        self.slot = 0;
        if let Some(root) = self.root {
            let mut list = Vec::new();
            self.collect(root, [cx0, cz0, cx1, cz1], [0, 0], 0x4000, &mut list);
            list.reverse();
            self.list = list;
            self.at = (!self.list.is_empty()).then_some(0);
        }
        self.head = self.at;
        true
    }

    /// The nodes under `node` whose square meets the cells, each pushed after its own (the original
    /// prepends, so the list is this reversed). `size` is a child's side; below the smallest node's
    /// the children are not looked at.
    fn collect(&self, node: usize, cells: [i32; 4], origin: [i32; 2], size: i32, list: &mut Vec<usize>) {
        let [x0, z0, x1, z1] = cells;
        if !(size < self.cell) {
            for (i, child) in self.nodes[node].children.iter().enumerate() {
                let Some(child) = *child else { continue };
                let cx = origin[0].wrapping_add(if i & 2 != 0 { size } else { 0 });
                let cz = origin[1].wrapping_add(if i & 1 != 0 { size } else { 0 });
                if cx.wrapping_add(size) < x0 || x1 < cx || cz.wrapping_add(size) < z0 || z1 < cz {
                    continue;
                }
                if cx < x0 || x1 < cx.wrapping_add(size) || cz < z0 || z1 < cz.wrapping_add(size) {
                    self.collect(child, cells, [cx, cz], size >> 1, list);
                } else {
                    self.collect_all(child, list);
                }
            }
        }
        if !self.nodes[node].chunks.is_empty() {
            list.push(node);
        }
    }

    /// Every node under `node`, children first.
    fn collect_all(&self, node: usize, list: &mut Vec<usize>) {
        for child in self.nodes[node].children.into_iter().flatten() {
            self.collect_all(child, list);
        }
        if !self.nodes[node].chunks.is_empty() {
            list.push(node);
        }
    }

    fn next(&mut self, me: usize, g: &mut Globals) -> Option<usize> {
        if self.flags & 1 == 0 || self.busy != 0 {
            return None;
        }
        *g.current = Some(me);
        let mut at = self.at?;
        let stamp = *g.stamp;
        loop {
            if self.chunk.is_none() {
                let n = self.list[at];
                self.chunk = (!self.nodes[n].chunks.is_empty()).then_some((n, 0));
            }
            while let Some((n, c)) = self.chunk {
                while self.slot < 4 {
                    let j = self.slot;
                    self.slot += 1;
                    if let Some(o) = self.nodes[n].chunks[c][j]
                        && g.places[o].stamp != stamp
                    {
                        g.places[o].stamp = stamp;
                        return Some(o);
                    }
                }
                self.slot = 0;
                self.chunk = (c + 1 < self.nodes[n].chunks.len()).then_some((n, c + 1));
            }
            at += 1;
            self.at = (at < self.list.len()).then_some(at);
            self.at?;
        }
    }

    /// Walk the open query's nodes again, under a new stamp. The chunk it was in is kept.
    fn restart(&mut self, g: &mut Globals) {
        self.at = self.head;
        self.slot = 0;
        *g.stamp = g.stamp.wrapping_add(1);
    }

    fn end(&mut self) {
        if self.flags & 1 == 0 {
            return;
        }
        self.list.clear();
        self.head = None;
        self.at = None;
        self.slot = 0;
        self.cells = [i32::MIN; 4];
        self.flags &= !1;
    }
}

impl Blocks {
    fn begin(&mut self) -> bool {
        self.block = 0;
        self.entry = 0;
        self.flags |= 1;
        true
    }

    /// The next collidable in a slot. One in a full block is handed out once per stamp; one in the last
    /// block every time.
    fn next(&mut self, g: &mut Globals) -> Option<usize> {
        let blocks = self.blocks.as_ref()?;
        let n = blocks.len() as i32;
        let stamp = *g.stamp;
        loop {
            if self.block < n - 1 {
                let b = &blocks[self.block as usize];
                while self.entry < self.size && b[self.entry as usize].is_none() {
                    self.entry += 1;
                }
                let j = self.entry;
                self.entry += 1;
                if self.size < self.entry {
                    self.block += 1;
                    self.entry = 0;
                    continue;
                }
                let o = b[j as usize].expect("a used slot");
                if g.places[o].stamp == stamp {
                    continue;
                }
                g.places[o].stamp = stamp;
                return Some(o);
            }
            if !(self.block < n) {
                return None;
            }
            let b = &blocks[self.block as usize];
            while self.entry < self.last && b[self.entry as usize].is_none() {
                self.entry += 1;
            }
            if !(self.entry < self.last) {
                self.block += 1;
                return None;
            }
            self.entry += 1;
            return b[self.entry as usize - 1];
        }
    }

    fn end(&mut self) {
        self.block = 0;
        self.entry = 0;
        self.flags &= !1;
    }
}

/// What a structure's query reads and writes outside the structure.
struct Globals<'a> {
    stamp: &'a mut u16,
    current: &'a mut Option<usize>,
    cell_size: F,
    places: &'a mut [Placement],
}

impl Broad {
    fn split(&mut self, s: usize) -> (&mut Structure, Globals<'_>) {
        let g = Globals {
            stamp: &mut self.stamp,
            current: &mut self.current,
            cell_size: self.cell_size,
            places: &mut self.places,
        };
        (&mut self.structures[s], g)
    }

    fn begin_structure(&mut self, s: usize, centre: [F; 4], half: [F; 4], bounds: Bounds) -> bool {
        let (st, mut g) = self.split(s);
        match st {
            Structure::Tree(t) => t.begin(s, &mut g, centre, half, bounds),
            Structure::List(l) => l.begin(),
        }
    }

    fn next_in(&mut self, s: usize) -> Option<usize> {
        let (st, mut g) = self.split(s);
        match st {
            Structure::Tree(t) => t.next(s, &mut g),
            Structure::List(l) => l.next(&mut g),
        }
    }

    fn end_structure(&mut self, s: usize) {
        match &mut self.structures[s] {
            Structure::Tree(t) => t.end(),
            Structure::List(l) => l.end(),
        }
    }

    fn reports_bounds(&self, s: usize) -> bool {
        let flags = match &self.structures[s] {
            Structure::Tree(t) => t.flags,
            Structure::List(l) => l.flags,
        };
        flags & 2 != 0
    }
}

/// A walk over the structures of a chain of spaces. `kind` bit 0 walks the static structures, bit 1
/// the moving ones.
#[derive(Clone, Debug, PartialEq)]
pub struct Query {
    pub centre: [F; 4],
    pub half: [F; 4],
    pub kind: u32,
    /// `+0x30`: the space being walked; `+0x24` its table; `+0x28` the structure; `+0x38` the sub-table
    /// (-1 for the table's own pair).
    pub space: Option<usize>,
    pub tables: usize,
    pub current: Option<usize>,
    pub sub: i32,
    /// `+0x34`: end with the global table.
    pub global: bool,
}

impl Query {
    /// The first table's structure begun with the box; `bounds` gets what it reports.
    pub fn begin(
        broad: &mut Broad,
        space: usize,
        kind: u32,
        centre: [F; 4],
        half: [F; 4],
        bounds: Bounds,
        global: bool,
    ) -> Query {
        let tables = broad.spaces[space].tables;
        let mut q = Query { centre, half, kind, space: Some(space), tables, current: None, sub: -1, global };
        q.pick(broad);
        let s = q.current.expect("a space with a structure of the kind asked for");
        broad.begin_structure(s, centre, half, bounds);
        q
    }

    /// The current table's static structure if asked for, else its moving one.
    fn pick(&mut self, broad: &Broad) {
        let own = broad.tables[self.tables].own;
        if self.kind & 1 != 0 {
            self.current = own.fixed;
        }
        if self.current.is_none() && self.kind & 2 != 0 {
            self.current = own.moving;
        }
    }

    /// The next collidable: the current structure's, else the next structure's, begun with the box and
    /// what it reports added to `bounds` (if it reports any).
    pub fn next(&mut self, broad: &mut Broad, mut bounds: Bounds) -> Option<usize> {
        let mut r = broad.next_in(self.current.expect("a begun query"));
        loop {
            let Some(cur) = self.current else { return r };
            if r.is_some() {
                return r;
            }
            let t = &broad.tables[self.tables];
            let pair = if self.sub == -1 { t.own } else { t.subs[self.sub as usize] };
            self.current = None;
            if Some(cur) == pair.fixed && self.kind & 2 != 0 && pair.moving != pair.fixed {
                self.current = pair.moving;
            }
            if self.current.is_none() {
                self.sub += 1;
                if let Some(p) = t.subs.get(self.sub as usize) {
                    self.current = p.fixed;
                }
            }
            if self.current.is_none() {
                self.sub = -1;
                if let Some(s) = self.space {
                    self.space = broad.spaces[s].next;
                    if let Some(s) = self.space {
                        self.tables = broad.spaces[s].tables;
                        self.pick(broad);
                    }
                }
                if self.current.is_none()
                    && self.global
                    && let Some(g) = broad.global
                    && self.tables != g
                {
                    self.tables = g;
                    self.pick(broad);
                }
            }
            let cur = self.current?;
            match &mut bounds {
                Some((lo, hi)) if broad.reports_bounds(cur) => {
                    // What a structure that turns the box down leaves here is whatever the original's
                    // stack held: see the journal.
                    let (mut tlo, mut thi) = ([F::ZERO; 4], [F::ZERO; 4]);
                    broad.begin_structure(cur, self.centre, self.half, Some((&mut tlo, &mut thi)));
                    for i in 0..3 {
                        if tlo[i] < lo[i] {
                            lo[i] = tlo[i];
                        }
                        if !(thi[i] <= hi[i]) {
                            hi[i] = thi[i];
                        }
                    }
                }
                _ => {
                    broad.begin_structure(cur, self.centre, self.half, None);
                }
            }
            r = broad.next_in(cur);
        }
    }

    /// End the current table's two structures.
    pub fn end(&self, broad: &mut Broad) {
        let own = broad.tables[self.tables].own;
        if let Some(s) = own.fixed {
            broad.end_structure(s);
        }
        if let Some(s) = own.moving
            && own.moving != own.fixed
        {
            broad.end_structure(s);
        }
    }
}

/// A collision object's nearby search (`+0x10c`), with what the gather keeps beside the list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Search {
    /// `+0`, `+4`, `+0x38`: the list and its prepare flag.
    pub nearby: Nearby,
    /// `+0x10`: the probe's centre when the list was gathered.
    pub centre: [F; 4],
    /// `+0x20`: how far the probe's reach may move from there before the list is gathered again.
    pub bound: F,
    /// `+0x24`: the half side of the box queried, and the least the bound is.
    pub least: F,
    /// `+0x28`: a reach the bound may grow to beyond the probe's.
    pub extra: F,
    /// `+0x2c`: placed groups are never gathered.
    pub no_groups: bool,
    /// `+0x30`: there is a list to keep.
    pub built: bool,
    /// `+0x34`: the most candidates gathered.
    pub capacity: i32,
}

/// The list's distances measured again from `centre`, nothing added or dropped.
pub fn resort(objects: &[Collidable], broad: &Broad, search: &mut Search, centre: [F; 4]) {
    for c in &mut search.nearby.found {
        c.distance = match objects[c.object].class {
            Class::Object => collidable_distance(F::ZERO, objects, broad, c.object, centre),
            _ => broad.group_distance(c.object, centre),
        };
    }
}

/// Fill `search` around `probe`: unless the probe's reach is still within the bound of where the list
/// was gathered (then only [`resort`]), query the box of side `2 × least` around it and keep, up to the
/// capacity, every collidable the pair filter lets through (those marked [`more_flags::WITHIN_BOUND`]
/// only within the bound) and every placed group within the bound. The bound is how far the queried
/// cells reach past the centre, at most the larger of the probe's reach and `extra`, at least `least`.
pub fn gather(objects: &mut [Collidable], broad: &mut Broad, search: &mut Search, probe: usize, kind: u32) {
    let centre = broad.places[probe].centre;
    let moved = length(vsub3(centre, search.centre));
    if search.built && moved + objects[probe].radius < search.bound {
        resort(objects, broad, search, centre);
        return;
    }
    let half = [search.least, search.least, search.least, F::ONE];
    search.nearby.found.clear();
    let (mut lo, mut hi) = ([F::ZERO; 4], [F::ZERO; 4]);
    let space = broad.places[probe].space.expect("a probe in a space");
    let mut q = Query::begin(broad, space, kind, centre, half, Some((&mut lo, &mut hi)), true);
    search.centre = centre;
    let reach = search.extra.max(objects[probe].radius);
    search.bound = (centre[0] - lo[0])
        .min(centre[2] - lo[2])
        .min(hi[0] - centre[0])
        .min(hi[2] - centre[2])
        .min(reach)
        .max(search.least);
    while let Some(o) = q.next(broad, Some((&mut lo, &mut hi))) {
        if !((search.nearby.found.len() as i32) < search.capacity) {
            break;
        }
        match objects[o].class {
            Class::Object => {
                objects[o].flags |= flags::SEARCHED;
                let distance = collidable_distance(F::ZERO, objects, broad, o, centre);
                if accepts(objects, probe, o, false)
                    && (objects[o].flags & more_flags::WITHIN_BOUND == 0 || distance < search.bound)
                {
                    search.nearby.found.push(Candidate { object: o, distance });
                }
            }
            Class::Group if !search.no_groups => {
                let distance = broad.group_distance(o, centre);
                if distance < search.bound {
                    search.nearby.found.push(Candidate { object: o, distance });
                }
            }
            _ => {}
        }
    }
    q.end(broad);
    search.built = true;
}

/// `probe` against the world directly, for a collision object without a nearby search; returns how far
/// the probe is from the nearest thing it did not touch (0 after a hit), which is also its new budget.
///
/// An inactive probe only lowers its hit bit. Otherwise the distance moved since the last time comes
/// off the budget, and with `lazy` set a probe with budget left stops there. Else: the probe cleared,
/// its box queried in the static structures (a collidable within reach through the pair filter and the
/// narrow phase; a placed group within reach part by part, unfiltered), then, if any table on its chain
/// has moving objects apart from its static ones, every moving object the same way.
pub fn probe_collide_world<N: Probe>(
    objects: &mut [Collidable],
    broad: &mut Broad,
    probe: usize,
    lazy: u32,
    narrow: &mut N,
) -> F {
    if objects[probe].flags & more_flags::ACTIVE == 0 {
        objects[probe].flags &= !flags::HIT;
        return F::MAX;
    }
    let mut lazy = lazy;
    {
        let p = &mut broad.places[probe];
        p.budget = p.budget - length(vsub3(p.centre, p.last));
        p.last = p.centre;
        if lazy != 0 && !(p.budget <= 0.0) {
            return p.budget;
        }
        p.budget = F::ZERO;
    }
    narrow.clear(objects, probe);
    objects[probe].flags |= flags::SEARCHED;
    let (c, h) = narrow.bounds(objects, probe);
    let (mut lo, mut hi) = ([F::ZERO; 4], [F::ZERO; 4]);
    let space = broad.places[probe].space.expect("a probe in a space");
    let mut q = Query::begin(broad, space, 1, c, h, Some((&mut lo, &mut hi)), true);
    let mut nearest = ((c[0] - h[0]) - lo[0])
        .min((c[2] - h[2]) - lo[2])
        .min(hi[0] - (c[0] + h[0]))
        .min(hi[2] - (c[2] + h[2]));
    while let Some(o) = q.next(broad, Some((&mut lo, &mut hi))) {
        match objects[o].class {
            Class::Object => {
                objects[o].flags |= flags::SEARCHED;
                let d = pair_distance(objects, broad, probe, o);
                if d == 0.0 {
                    nearest = F::ZERO;
                    if accepts(objects, probe, o, true) {
                        world_search::test(objects, probe, o, narrow);
                    }
                } else if d < nearest {
                    nearest = d;
                    if lazy != 0 && d < broad.places[probe].budget {
                        lazy <<= 1;
                    }
                }
            }
            Class::Group if objects[probe].flags & flags::NO_GROUPS == 0 => {
                let [x, z] = broad.group_point(o);
                let d = point_distance(broad.group_reach(o), objects, broad, probe, [x, F::ZERO, z, F::ZERO]);
                if d == 0.0 {
                    let mut i = 0;
                    while i < narrow.parts(objects, o) {
                        if let Some(p) = narrow.part(objects, o, i) {
                            objects[p].group = Some(o);
                            nearest = F::ZERO;
                            objects[p].flags |= flags::SEARCHED;
                            world_search::test(objects, probe, p, narrow);
                        }
                        i += 1;
                    }
                } else if lazy != 0 && d < nearest {
                    nearest = d;
                    if d < broad.places[probe].budget {
                        lazy <<= 1;
                    }
                }
            }
            _ => {}
        }
    }
    let mut moving = false;
    let mut s = Some(space);
    while let Some(i) = s {
        let own = broad.tables[broad.spaces[i].tables].own;
        moving |= own.fixed != own.moving;
        s = broad.spaces[i].next;
    }
    if moving {
        let mut q = Query::begin(broad, space, 2, c, h, None, false);
        while let Some(o) = q.next(broad, None) {
            if objects[o].class != Class::Object {
                continue;
            }
            objects[o].flags |= flags::SEARCHED;
            if pair_distance(objects, broad, probe, o) == 0.0 && accepts(objects, probe, o, true) {
                world_search::test(objects, probe, o, narrow);
            }
        }
    }
    if objects[probe].flags & flags::HIT != 0 {
        broad.places[probe].last = [F::ZERO, F::from_bits(0xc61c_4000), F::ZERO, F::ONE];
        nearest = F::ZERO;
    }
    broad.places[probe].budget = nearest;
    nearest
}
