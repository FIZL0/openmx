//! The collision object's contact search: what fills the list [`crate::contact::resolve`] works through.
//!
//! Once a frame, before the world steps, each collision object empties its list ([`clear`]) and looks
//! for what it touches ([`collide`]). The search has three parts, in this order:
//!
//! 1. the *sweep back* ([`sweep_back`]): a segment from where the object's reference point was a little
//!    while ago (its velocity times a lead time) to where it is now, searched against the world. When
//!    it hits something, the body is put back out of it — unless what it hit is another collision
//!    object or a mover and the two are flying apart;
//! 2. the *heightfield rays*: each of the ray probe's rays that points down (against the object's up)
//!    has its end looked up in the terrain ([`ground`]), and a contact is added where the end is below
//!    the ground. The terrain is a grid of cells of four corners, each split into two triangles; which
//!    way a cell is split depends on the terrain and on the cell's parity;
//! 3. the *world rays*: the ray probe searched against the world, and one contact per hit. A hit on
//!    another collision object also gives *that* object a contact back, its normal turned round.
//!
//! The record of the last thing the rays struck, and the flags saying whether the object touched
//! anything, the ground, an object or another collision object, are what the crash test reads.
//!
//! The two world searches (the near-field one and the global one the object chooses between) and the
//! probes' own narrow phases are not this module's: they come in through [`Search`]. Neither the
//! heightfield query nor the ground result it is read through knows about water — the depth the ground
//! result reports is always 0 — so if water is to enter a collision object's contacts at all, it is as
//! one of the world search's collidables (see the journal).
//!
//! The terrain comes in through [`Heightfield`]: the cell reader plus the few fields of the terrain
//! object the query reads. [`TerrainField`] is that over [`crate::track::Terrain`].

#![deny(clippy::float_arithmetic)]

use crate::biplane::Body;
use crate::chassis::turn;
use crate::contact::{Contact, Node, dot, minus, negated, plus, scaled, unit, unturn};
use crate::track::Terrain;
use ps2_float::{Dest, F, Field, Vf};

/// Up, as the original keeps it (`w` 1). Also the normal of a cell the terrain cannot read.
const UP: Vf = Vf::new(0.0, 1.0, 0.0, 1.0);

/// The surface a contact the world search found carries: none.
pub const NO_SURFACE: i32 = -1;

/// The kind of mover that counts as one (`+0x1b0`); anything else on a struck object is ignored.
pub const MOVER_KIND: u32 = 2;

// --- the heightfield -------------------------------------------------------------------------------

/// One terrain cell as the cell reader hands it back: the heights before the terrain's height scale,
/// the corners' normals (unnormalised, whole quadwords) and surfaces. Corner order: the sample itself,
/// `+x`, `+z`, both.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cell {
    pub heights: Vf,
    pub normals: [Vf; 4],
    pub surfaces: [u8; 4],
}

/// The terrain object, as the heightfield query and the rays read it.
pub trait Heightfield {
    /// `+0x30`, `+0x40`: the box the rays' ends must be inside (`x` and `z`, both ends open).
    fn min(&self) -> Vf;
    fn max(&self) -> Vf;
    /// `+0x778`: samples per world unit.
    fn per_sample(&self) -> F;
    /// `+0x80`: what the cell reader's heights are multiplied by.
    fn height_scale(&self) -> F;
    /// `+0x8a8`: read the terrain two samples to a cell instead of one.
    fn coarse(&self) -> bool;
    /// `+0x760 == 1`: every cell is split the same way; otherwise it alternates with the cell's parity.
    fn split_one_way(&self) -> bool;
    /// The cell reader: the cell at sample `(ix, iz)`, `step` samples wide, or `None` where the
    /// terrain has none.
    fn cell(&self, ix: i32, iz: i32, step: i32) -> Option<Cell>;

    /// A world of several terrains (MD8, [`crate::tiles`]): hand `query` the terrain under a world
    /// point, with the point moved into that terrain's own coordinates, and say whether there was
    /// one. `None` for a single terrain, which is queried as it is.
    fn tile(&self, at: Vf, query: &mut dyn FnMut(&dyn Heightfield, Vf)) -> Option<bool> {
        let _ = (at, query);
        None
    }
}

/// [`Terrain`] with the terrain object's own fields beside it.
pub struct TerrainField<'a> {
    pub terrain: &'a Terrain<'a>,
    pub min: Vf,
    pub max: Vf,
    pub per_sample: F,
    pub height_scale: F,
    pub coarse: bool,
    pub split_one_way: bool,
}

impl Heightfield for TerrainField<'_> {
    fn min(&self) -> Vf {
        self.min
    }
    fn max(&self) -> Vf {
        self.max
    }
    fn per_sample(&self) -> F {
        self.per_sample
    }
    fn height_scale(&self) -> F {
        self.height_scale
    }
    fn coarse(&self) -> bool {
        self.coarse
    }
    fn split_one_way(&self) -> bool {
        self.split_one_way
    }
    fn cell(&self, ix: i32, iz: i32, step: i32) -> Option<Cell> {
        let q = self.terrain.cell(u32::try_from(ix).ok()?, u32::try_from(iz).ok()?, step as u32)?;
        let [h0, h1, h2, h3] = q.height.map(F::to_f32);
        Some(Cell {
            heights: Vf::new(h0, h1, h2, h3),
            normals: q.normal.map(|n| Vf::new(n[0].0, n[1].0, n[2].0, n[3].0)),
            surfaces: q.surface,
        })
    }
}

/// The ground under a point, as the query leaves its buffer: the point with the ground's height in
/// `y`, the blended normal, the cell's four surfaces and each corner's weight.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ground {
    pub point: Vf,
    pub normal: Vf,
    pub surfaces: [u8; 4],
    pub weights: Vf,
}

/// The heightfield query: the ground at `at`'s `x` and `z`. The caller hands it `at` with every
/// corner weighted 1; a cell the terrain cannot read leaves both, with the normal straight up and no
/// surfaces. (The original also keeps the last ten cells it read; the cache changes nothing it
/// returns, so the port has none.)
pub fn ground(t: &dyn Heightfield, at: Vf) -> Ground {
    // A tile of a world answers in its own coordinates, and its answer is put back at `at`.
    let mut found = None;
    if t.tile(at, &mut |tile, local| found = Some(ground(tile, local))).is_some() {
        return match found {
            Some(g) => Ground { point: Vf::new(at.x(), g.point.y(), at.z(), at.w()), ..g },
            None => Ground { point: at, normal: UP, surfaces: [0; 4], weights: Vf::splat(1.0) },
        };
    }
    let (step, wide) = if t.coarse() { (2, F(2.0)) } else { (1, F::ONE) };
    let fx = (F(at.x()) * t.per_sample()).max(F::ZERO);
    let fz = (F(at.z()) * t.per_sample()).max(F::ZERO);
    let (mut ix, mut iz) = (fx.to_i32(), fz.to_i32());
    if step == 2 {
        ix &= !1;
        iz &= !1;
    }
    let weights = Vf::splat(1.0);
    let Some(cell) = t.cell(ix, iz, step) else {
        return Ground { point: at, normal: UP, surfaces: [0; 4], weights };
    };
    let h = cell.heights.mul(Vf::splat(t.height_scale().to_f32()));
    let [h0, h1, h2, h3] = h.0.map(F);

    let u = (fx - F::from(ix)) / wide;
    let v = (fz - F::from(iz)) / wide;
    let (uu, nu, nv, vv) = (u * u, F::ONE - u, F::ONE - v, v * v);
    let d00 = uu + vv;
    let nunu = nu * nu;
    let d10 = nunu + vv;
    let nvnv = nv * nv;
    let d11 = nunu + nvnv;
    let weights = Vf::new((nu * nv).0, (u * nv).0, (nu * v).0, (u * v).0);
    let d01 = uu + nvnv;

    let [n0, n1, n2, n3] = cell.normals;
    let acc = n0.mul(weights.bc(Field::X));
    let acc = Vf::madd(acc, n1, weights.bc(Field::Y));
    let acc = Vf::madd(acc, n2, weights.bc(Field::Z));
    let normal = unit(Vf::madd(acc, n3, weights.bc(Field::W)));

    let y = if t.split_one_way() || (ix & 1) != (iz & 1) {
        if d00 < d11 {
            h0 + u * (h1 - h0) + v * (h2 - h0)
        } else {
            h2 + u * (h3 - h2) + (F::ONE - v) * (h1 - h3)
        }
    } else if d10 < d01 {
        h0 + u * (h1 - h0) + v * (h3 - h1)
    } else {
        h2 + u * (h3 - h2) + (F::ONE - v) * (h0 - h2)
    };
    Ground { point: Vf::new(at.x(), y.0, at.z(), at.w()), normal, surfaces: cell.surfaces, weights }
}

/// The surface the ground result reports: the corner with the largest weight (the first, on a tie).
pub fn surface(g: &Ground) -> i32 {
    let (mut best, mut at) = (F::ZERO, 0);
    for (i, &w) in g.weights.0.iter().enumerate() {
        if !(F(w) <= best) {
            best = F(w);
            at = i;
        }
    }
    i32::from(g.surfaces[at])
}

/// The surface the heightfield query writes into the wheel's record (S89): the corner of the cell
/// nearest `at`, by squared distance in the cell's own units, the first on a tie. `None` where the
/// terrain has no cell — the query returns before it writes, so the wheel keeps the surface it had.
///
/// It is the terrain's surface whatever the wheel is riding: the query knows nothing of collision
/// meshes, and the swept contact that does never writes the byte. The corners come in
/// [`ground`]'s order — the sample, `+x`, `+z`, both.
pub fn nearest_surface(t: &dyn Heightfield, at: Vf) -> Option<u8> {
    let mut found = None;
    if t.tile(at, &mut |tile, local| found = nearest_surface(tile, local)).is_some() {
        return found;
    }
    let (step, wide) = if t.coarse() { (2, F(2.0)) } else { (1, F::ONE) };
    let fx = (F(at.x()) * t.per_sample()).max(F::ZERO);
    let fz = (F(at.z()) * t.per_sample()).max(F::ZERO);
    let (mut ix, mut iz) = (fx.to_i32(), fz.to_i32());
    if step == 2 {
        ix &= !1;
        iz &= !1;
    }
    let cell = t.cell(ix, iz, step)?;
    let u = (fx - F::from(ix)) / wide;
    let v = (fz - F::from(iz)) / wide;
    let (nu, nv) = (F::ONE - u, F::ONE - v);
    let d = [u * u + v * v, nu * nu + v * v, u * u + nv * nv, nu * nu + nv * nv];
    let (mut best, mut at) = (F(f32::MAX), 0);
    for (i, &di) in d.iter().enumerate() {
        if !(best <= di) {
            best = di;
            at = i;
        }
    }
    Some(cell.surfaces[at])
}

// --- the record ------------------------------------------------------------------------------------

/// A collision object, as the search reads and writes it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Record {
    /// `+0x30` look for the ground, `+0x34` look at all, `+0x3c` sweep back.
    pub search_ground: bool,
    pub search: bool,
    pub sweep_back: bool,
    /// `+0x40` touched anything, `+0x44` another collision object, `+0x48` the ground, `+0x4c` an
    /// object. Set by the search, emptied by [`clear`].
    pub touching: bool,
    pub touching_other: bool,
    pub on_ground: bool,
    pub on_object: bool,
    /// `+0x50`: give the body its damping back when the list is emptied; `+0x54`, `+0x5c` what to.
    pub damp: bool,
    pub rest_linear: F,
    pub rest_angular: F,
    /// `+0xb0`: the sweep's reference point, in the body's frame.
    pub offset: Vf,
    /// `+0xc0`: how far back the sweep starts, in seconds of the body's velocity; `+0xc4` how far
    /// along the hit's push-out direction the body is put; `+0xc8` the cosine past which two objects'
    /// directions count as flying apart.
    pub lead: F,
    pub push_out: F,
    pub facing: F,
    /// `+0xd0`, `+0xe0`: the last sweep's segment.
    pub swept_from: Vf,
    pub swept_to: Vf,
    /// `+0xf8`: the object's transform. It has no parent, so moving the body moves its translation.
    pub node: Node,
    /// `+0x100`..`+0x108`: the contacts, at most `capacity` of them.
    pub contacts: Vec<Contact>,
    pub capacity: usize,
    /// `+0x10c`: the object has a near-field search (and the world search goes through it).
    pub nearby: bool,
    /// `+0x1f0`: the ray probe's rays (`+0x168`, `+0x170`: start and end, in model space), or `None`
    /// for an object with no ray probe; `+0x1f4`: it has a sweep probe.
    pub rays: Option<Vec<[Vf; 2]>>,
    pub sweeps: bool,
    /// `+0x1fc`: the last object the world rays struck. Nothing empties it.
    pub struck: Option<usize>,
}

/// One contact onto the list, if there is room.
#[allow(clippy::too_many_arguments)]
pub fn add(
    rec: &mut Record,
    kind: u32,
    from: Vf,
    to: Vf,
    point: Vf,
    normal: Vf,
    surface: i32,
    other: Option<usize>,
    mover: Option<usize>,
) {
    if rec.contacts.len() < rec.capacity {
        rec.contacts.push(Contact {
            kind,
            from,
            on_body: to,
            normal,
            point,
            arm: Vf::VF0,
            other_arm: Vf::VF0,
            surface,
            other,
            mover,
        });
    }
}

/// Empty the list and the flags, and give the body its damping back if the object lowers it.
pub fn clear(rec: &mut Record, body: Option<&mut Body>) {
    rec.contacts.clear();
    rec.touching = false;
    rec.on_ground = false;
    rec.on_object = false;
    rec.touching_other = false;
    if let Some(body) = body {
        if rec.damp {
            let (l, a) = (rec.rest_linear.0, rec.rest_angular.0);
            body.damping = Vf::new(l, l, l, 1.0);
            body.ang_damping = Vf::new(a, a, a, 1.0);
        }
    }
}

// --- the world -------------------------------------------------------------------------------------

/// What a probe struck, as the hit list's object gives it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Struck {
    /// The object itself (what [`Record::struck`] keeps).
    pub object: usize,
    /// `+0x80`: its collision object, by index into the records [`collide`] is handed.
    pub other: Option<usize>,
    /// `+0x7c`: its mover, by index, with the mover's kind ([`MOVER_KIND`] is the only one that counts).
    pub mover: Option<(usize, u32)>,
}

impl Struck {
    fn mover(&self) -> Option<usize> {
        self.mover.and_then(|(m, kind)| (kind == MOVER_KIND).then_some(m))
    }
}

/// One of the ray probe's hits: the ray (`+0x90`, two quadwords a hit), the hit point (`+0x98`) and
/// the normal (`+0x94`), and what was struck (`+0x9c`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hit {
    pub from: Vf,
    pub to: Vf,
    pub point: Vf,
    pub normal: Vf,
    pub struck: Struck,
}

/// The sweep probe's hit: the direction to push out along (`+0x20`), the point to push out from
/// (`+0x30`), and what it struck first.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Swept {
    pub push: Vf,
    pub point: Vf,
    pub struck: Struck,
}

/// The world, as the search asks it. `nearby` says which of the two searches the original runs: the
/// object's near-field one or the global one.
pub trait Search {
    /// Search with the ray probe where it stands, and return its hits in its order.
    fn rays(&mut self, nearby: bool) -> Vec<Hit>;
    /// Set the sweep probe to the segment, place it, search with it, and return its hit if its "hit"
    /// flag is up.
    fn sweep(&mut self, from: Vf, to: Vf, nearby: bool) -> Option<Swept>;
    /// Place the ray probe again at the object's node, after the sweep moved the body.
    fn moved(&mut self, node: &Node);
    /// The velocity of what the sweep struck: its collision object's body (or vehicle chassis), or,
    /// with none, its mover's.
    fn velocity(&self, struck: &Struck) -> Vf;
    /// The movers a [`Struck::mover`] indexes, for [`crate::contact::resolve`] (S145). None by
    /// default: a world whose hits name no mover.
    fn movers(&self) -> Vec<crate::contact::Mover> {
        Vec::new()
    }
}

/// The sweep back, on an object with a body. (The original also sweeps an object on a vehicle's
/// chassis, through its transform; that is not ported.)
pub fn sweep_back(rec: &mut Record, body: &mut Body, search: &mut dyn Search, dt: F) {
    if !rec.sweep_back || !rec.sweeps {
        return;
    }
    let r = turn(body.quat, rec.offset);
    rec.swept_to = plus(body.position, r);
    let back = scaled(scaled(body.velocity, rec.lead), dt);
    rec.swept_from = minus(rec.swept_to, back);
    let Some(hit) = search.sweep(rec.swept_from, rec.swept_to, rec.nearby) else {
        return;
    };
    if hit.struck.other.is_some() || hit.struck.mover().is_some() {
        let ours = body.velocity;
        let theirs = search.velocity(&hit.struck);
        if !(dot(ours, ours).sqrt() <= F::ONE) && !(dot(theirs, theirs).sqrt() <= F::ONE) {
            let (ours, theirs) = (unit(ours), unit(theirs));
            if !(dot(ours, theirs) <= rec.facing) {
                return;
            }
        }
    }
    let at = minus(plus(hit.point, scaled(hit.push, rec.push_out)), r);
    body.position = at;
    rec.node.origin = rec.node.origin.merge(Dest::XYZ, at);
    if rec.rays.is_some() {
        search.moved(&rec.node);
    }
    rec.swept_to = at;
}

/// The sweep back, on an object with no body of its own whose transform is a vehicle's chassis' node
/// (`+0xf4` empty, `+0xf0` the chassis): the riding bike's (S125). The same sweep as [`sweep_back`]'s,
/// but the reference point is the offset turned by the node's rows onto its translation, the lead is
/// along the chassis' velocity (`+0x260`), and a hit moves the node and nothing else: the chassis
/// reads its centre of mass back off its node when it next integrates. `Some` with the node's new
/// translation when the sweep moved it.
pub fn sweep_back_chassis(rec: &mut Record, velocity: Vf, search: &mut dyn Search, dt: F) -> Option<Vf> {
    if !rec.sweep_back || !rec.sweeps {
        return None;
    }
    let r = rec.node.rotate(rec.offset);
    rec.swept_to = plus(rec.node.origin, r);
    let back = scaled(scaled(velocity, rec.lead), dt);
    rec.swept_from = minus(rec.swept_to, back);
    let hit = search.sweep(rec.swept_from, rec.swept_to, rec.nearby)?;
    if hit.struck.other.is_some() || hit.struck.mover().is_some() {
        let theirs = search.velocity(&hit.struck);
        if !(dot(velocity, velocity).sqrt() <= F::ONE) && !(dot(theirs, theirs).sqrt() <= F::ONE) {
            let (ours, theirs) = (unit(velocity), unit(theirs));
            if !(dot(ours, theirs) <= rec.facing) {
                return None;
            }
        }
    }
    let at = minus(plus(hit.point, scaled(hit.push, rec.push_out)), r);
    rec.node.origin = rec.node.origin.merge(Dest::XYZ, at);
    if rec.rays.is_some() {
        search.moved(&rec.node);
    }
    rec.swept_to = at;
    Some(rec.node.origin)
}

/// The whole search for `records[me]`: the sweep back, the heightfield rays, the world rays.
pub fn collide(
    records: &mut [Record],
    me: usize,
    mut body: Option<&mut Body>,
    field: Option<&dyn Heightfield>,
    search: &mut dyn Search,
    dt: F,
) {
    if let Some(body) = body.as_deref_mut() {
        sweep_back(&mut records[me], body, search, dt);
    }
    let rec = &mut records[me];
    if rec.rays.is_none() || !rec.search {
        return;
    }
    if let (true, Some(t)) = (rec.search_ground, field) {
        let up = match body.as_deref() {
            Some(b) => unturn(b.quat, UP),
            None => {
                let [r0, r1, r2] = rec.node.rows;
                Vf::new(r0.y(), r1.y(), r2.y(), 1.0)
            }
        };
        let (lo, hi) = (t.min(), t.max());
        let rays = rec.rays.clone().unwrap_or_default();
        for [start, end] in rays {
            if !(dot(minus(end, start), up) < F::ZERO) {
                continue;
            }
            let tip = rec.node.place(end);
            let inside = tip.x() < hi.x() && !(tip.x() <= lo.x()) && tip.z() < hi.z() && !(tip.z() <= lo.z());
            if !inside {
                continue;
            }
            let g = ground(t, tip);
            let point = minus(g.point, scaled(g.normal, F::ZERO));
            if !(point.y() <= tip.y()) {
                rec.touching = true;
                rec.on_ground = true;
                add(rec, 0, start, end, point, g.normal, surface(&g), None, None);
            }
        }
    }
    let nearby = records[me].nearby;
    for hit in search.rays(nearby) {
        let rec = &mut records[me];
        rec.touching = true;
        rec.on_object = true;
        let s = hit.struck;
        add(rec, 0, hit.from, hit.to, hit.point, hit.normal, NO_SURFACE, s.other, s.mover());
        rec.struck = Some(s.object);
        if let Some(o) = s.other {
            rec.touching_other = true;
            let other = &mut records[o];
            other.touching = true;
            other.on_object = true;
            other.touching_other = true;
            add(other, 1, hit.from, hit.to, hit.point, negated(hit.normal), NO_SURFACE, Some(me), None);
        }
    }
}
