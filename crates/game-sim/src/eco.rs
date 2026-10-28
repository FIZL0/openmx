//! The ecosystem's trunks (S97): the solid shape each tree species of an `.esb` stands with, built
//! into a collision mesh the way the original builds it, and placed under every plant of the species.
//!
//! The drawing half of an `.esb` is the renderer's (`game-render`'s `eco`, V48); this is the half the
//! ride needs, read again here because the sim cannot depend on the renderer:
//!
//! * **The shapes.** A species lists solid records — `u32 kind, vec3 at, 12 bytes, f32 radius,
//!   f32 height` — and the ecosystem's loader turns each into a collidable when the scene asks it to
//!   (a free ride does). Every tree of the disc's free tracks has one, kind 1, at the origin, radius
//!   1.5 and 30 to 50 high: an upright sixteen-sided prism, the trunk. Bushes have none. Kind 2 is a
//!   sphere (MX vs. ATV's tracks have them, S102): a part the ray probe's narrow phase only answers
//!   yes or no to, writing no contact, so it is kept as a part that is never tested. Kinds 0 (a mesh
//!   file by the species' name) and 3 (a shape class of its own) stand on no track of either game's
//!   discs and are not built ([`Trees::new`] skips them, and says so).
//! * **The mesh.** [`cylinder`] lays out the prism exactly — its sixteen corners come off the engine's
//!   `{cos, sin}` table without the table's correction — and [`build`] is the engine's collision-tree
//!   builder, the one every mesh made at run time goes through: each triangle's plane, the identity
//!   transform the builder applies in place, the centroids, and the volume-driven split, node by node,
//!   to the bit.
//! * **The plants.** One mesh per species, shared: the original moves it under each plant as it is
//!   asked about, to the plant's cell times the ecosystem's cell size. [`Trees`] keeps one
//!   [`world::Prop`] per plant instead, at the same place.
//! * **No sway** (S101). The ecosystem has room for a pool of per-plant bend blocks (a bend
//!   quaternion, an angle, a state byte: below 2 bending, 4 fading) that a plant points at with a
//!   flag bit and an index, and its frame and draw passes read one when the bit is set. The pool is
//!   only made when the scene's `[Environment]` has `UseEcoPhysics` (default off), no scene on the
//!   disc has it, and nothing in the game ever writes a block or sets the bit. A hit tree stands
//!   exactly as the file put it, so [`Trees`] never moves: there is nothing to port.
//!
//! Which searches see the trunks is the part that is the original's own and not this module's: the
//! bike's eleven body rays search placed groups, its swept-sphere probe does not, and the put-back
//! never sweeps a group but refuses any spot within twenty feet of one ([`Trees::crowded`], S75's
//! rule). A bike carrying its collision files searches them with its body rays through
//! [`World::wooded`] (S138, `rider_contact::scenery`), and the resolve of what they find loads the
//! impact a head-on trunk wrecks the rider on, as the original's does (S126); one without them falls
//! back on [`World::groups`] for its own frame probes and the head ray (the S97 journal). The biplane's and the wreck bike's ray probes search them
//! through their nearby lists as the original's do, the plants placed groups in the world's tree
//! ([`World::wooded`], [`crate::flight_search::Scene::wooded`], S102); the camera never asks the
//! world for anything but the terrain.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::broad_phase::Model;
use crate::camera::CurveTable;
use crate::collide::{Bundle, Record};
use crate::contact_search::Heightfield;
use crate::flight_search;
use crate::math::Mat4;
use crate::track::{Mesh, Node, Points, Triangle};
use crate::world::{self, Prop, Props, World};
use ps2_float::{Acc, F, Vf, fpu, vu};

/// The solid record's kind for an upright cylinder.
pub const CYLINDER: u32 = 1;

/// The solid record's kind for a sphere (S102): a collidable of the sphere class, `radius` round at
/// `at`. MX vs. ATV's tracks stand some; no MXU species has one.
pub const SPHERE: u32 = 2;

/// How many sides the original gives a cylinder: sixteen corners a sixteenth of a turn apart.
pub const SIDES: usize = 16;

/// The angle between two of a cylinder's corners, as the loader steps it: a rounded `pi / 8`.
const SIDE_STEP: F = F(0.392_699_1);

/// How near (feet, on the ground plane) a placed group's cell may be to a put-back's spot before the
/// put-back refuses it: S75's rule, `dx² + dz² <= 400`.
const CROWDED: F = F(400.0);

// --- the file ----------------------------------------------------------------------------------------

/// A solid shape a species stands with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Solid {
    pub kind: u32,
    pub at: [F; 3],
    pub radius: F,
    pub height: F,
}

/// One species slot of the file: what the trunks need of it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Species {
    /// The model's name, which a kind-0 solid would load its mesh by.
    pub model: String,
    /// Feet: the usual, the least and the most — the original's `+0x100`, `+0x104`, `+0x108`.
    pub height: [F; 3],
    /// Feet: the usual, the least and the most — the original's `+0x10c`, `+0x110`, `+0x114`.
    pub width: [F; 3],
    pub solids: Vec<Solid>,
}

/// One plant: its species' slot, its cell, and its two size bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plant {
    /// The slot (`0..256`), without the stand-on-the-ground bit.
    pub species: u8,
    /// The bit that asks the loader to stand the plant on the ground ([`Ecosystem::stand`]).
    pub snap: bool,
    /// In 65536ths of the map's larger side.
    pub cell: [u16; 3],
    pub height: i8,
    /// How far the plant's reach lies between its species' usual width and its widest, in 128ths.
    pub width: i8,
}

/// A parsed `.esb`, as far as the trunks need it.
#[derive(Clone, Debug, PartialEq)]
pub struct Ecosystem {
    pub version: u32,
    /// By slot: `None` where the file's slot is empty.
    pub species: Vec<Option<Species>>,
    pub plants: Vec<Plant>,
}

struct Reader<'a> {
    d: &'a [u8],
    p: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self.p.checked_add(n).filter(|&e| e <= self.d.len());
        let end = end.ok_or_else(|| format!("the .esb ends at {} of {} bytes it needs", self.d.len(), self.p + n))?;
        let b = &self.d[self.p..end];
        self.p = end;
        Ok(b)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn f(&mut self) -> Result<F, String> {
        Ok(F(f32::from_bits(self.u32()?)))
    }
    fn string(&mut self) -> Result<String, String> {
        let n = self.u8()? as usize;
        let b = self.take(n)?;
        let b = b.split(|&c| c == 0).next().unwrap_or(b);
        Ok(String::from_utf8_lossy(b).into_owned())
    }
    /// A flag byte and, if it is set, a string.
    fn maybe_string(&mut self) -> Result<Option<String>, String> {
        if self.u8()? != 0 { self.string().map(Some) } else { Ok(None) }
    }
}

impl Ecosystem {
    /// Read an `.esb`. The layout is the renderer's reader's (V48), kept to the fields the trunks use.
    pub fn parse(d: &[u8]) -> Result<Ecosystem, String> {
        let mut r = Reader { d, p: 0 };
        let version = r.u32()?;
        let mut species = Vec::with_capacity(256);
        for _ in 0..256 {
            if r.u8()? == 0 {
                species.push(None);
                continue;
            }
            let model = r.string()?;
            r.string()?; // the billboard
            let height = [r.f()?, r.f()?, r.f()?];
            let width = [r.f()?, r.f()?, r.f()?];
            r.take(5 * 4 + 12)?; // the billboard's rectangle and trunk, and three words
            if version == 2 {
                r.take(36)?;
                r.maybe_string()?;
            }
            let count = r.u32()? as usize;
            let mut solids = Vec::with_capacity(count.min(256));
            for _ in 0..count {
                let kind = r.u32()?;
                let at = [r.f()?, r.f()?, r.f()?];
                r.take(12)?;
                solids.push(Solid { kind, at, radius: r.f()?, height: r.f()? });
            }
            species.push(Some(Species { model, height, width, solids }));
        }
        r.maybe_string()?; // the shrubbery
        if version == 2 {
            r.u32()?;
            r.maybe_string()?; // the probability map
        }
        let count = r.u32()? as usize;
        let mut plants = Vec::with_capacity(count.min(1 << 20));
        for _ in 0..count {
            let kind = r.u8()?;
            let cell = [r.u16()?, r.u16()?, r.u16()?];
            let (height, width) = (r.u8()? as i8, r.u8()? as i8);
            let slot = kind & 0x7f;
            if species[usize::from(slot)].is_none() {
                return Err(format!("a plant of species slot {slot}, which is empty"));
            }
            plants.push(Plant { species: slot, snap: kind & 0x80 != 0, cell, height, width });
        }
        if r.p != d.len() {
            return Err(format!("{} bytes after the last plant", d.len() - r.p));
        }
        Ok(Ecosystem { version, species, plants })
    }

    /// Stand every plant that asks for it on the ground, as the renderer does (V48): `ground` answers
    /// the height under a world point, kept as a whole number of cells cut towards zero. The original's
    /// own plant reader does this, with the map's larger side over 65536 and its inverse; the arithmetic
    /// is the renderer's reading of it, not proved here.
    pub fn stand(&mut self, extent: F, mut ground: impl FnMut(Vf) -> F) {
        let (cell, per) = (cell_size(extent), F(65536.0) / extent);
        for p in self.plants.iter_mut().filter(|p| p.snap) {
            let [x, y, z] = place(p.cell, cell);
            let h = ground(Vf::new(x.0, y.0, z.0, 1.0));
            p.cell[1] = (h * per).to_i32() as u16;
        }
    }
}

/// The ecosystem's cell size: the map's larger side over 65536, the way the original's ecosystem
/// object keeps it (`extent` as the terrain states it; on free09 6912 feet, so 0.10546875).
pub fn cell_size(extent: F) -> F {
    extent / 65536.0
}

/// A plant's cell in feet: each of the three shorts times the cell size.
pub fn place(cell: [u16; 3], size: F) -> [F; 3] {
    cell.map(|c| F::from(i32::from(c)) * size)
}

// --- the cylinder ------------------------------------------------------------------------------------

/// A kind-1 solid as the loader lays it out: sixteen corners round the top (`0..16`, at `height`) and
/// the same sixteen round the bottom (`16..32`, at 0), each moved by `at`, and two triangles a side.
///
/// Corner `i` is `i` sixteenths of a turn round, wrapped into `-pi..pi` and looked up in the `{cos,
/// sin}` table with no correction; `x` is the cosine times the radius and `z` the sine times it. The
/// corners keep a `w` the table lookup never reads: 1, plus `at`'s, which the loader sets to 1.
pub fn cylinder(solid: &Solid, curve: &CurveTable) -> (Vec<Vf>, Vec<[i32; 3]>) {
    let mut corners = vec![Vf::ZERO; 2 * SIDES];
    for i in 0..SIDES {
        let (cos, sin) = curve.coarse(F::from(i as i32) * SIDE_STEP);
        let (x, z) = (cos * solid.radius, sin * solid.radius);
        corners[i] = Vf::new(x.0, solid.height.0, z.0, 1.0);
        corners[SIDES + i] = Vf::new(x.0, 0.0, z.0, 1.0);
    }
    let at = Vf::new(solid.at[0].0, solid.at[1].0, solid.at[2].0, 1.0);
    let corners = corners.into_iter().map(|c| c.add(at)).collect();
    let mut tris = Vec::with_capacity(2 * SIDES);
    for i in 0..SIDES as i32 {
        let j = if i + 1 > SIDES as i32 - 1 { 0 } else { i + 1 };
        let s = SIDES as i32;
        tris.push([i, j, j + s]);
        tris.push([i, j + s, i + s]);
    }
    (corners, tris)
}

// --- the tree builder --------------------------------------------------------------------------------

/// A node of a built tree, the 36 bytes the builder writes: the volume and radius of its box, its two
/// children, the box's centre and half extents.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BuiltNode {
    pub volume: F,
    pub radius: F,
    /// As [`crate::track::Node::children`]: the top bit marks a triangle.
    pub children: [u16; 2],
    pub centre: [F; 3],
    pub half: [F; 3],
}

/// A triangle of a built tree, in the tree's order: its corners, its plane's distance and normal.
/// The builder stores the normal packed ([`Facet::packed`]); the float is what it packs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Facet {
    pub vertices: [u16; 3],
    pub distance: F,
    pub normal: [F; 3],
}

impl Facet {
    /// The normal as the builder stores it: each lane plus 768, the low half of its bits — the form
    /// the ray probe reads ([`flight_search::pack_normal`]).
    pub fn packed(&self) -> [i16; 3] {
        flight_search::pack_normal(self.normal)
    }
}

/// What the builder makes of a vertex list and a triangle list.
#[derive(Clone, Debug, PartialEq)]
pub struct Built {
    /// `triangles - 1` nodes, the root first and each node's subtrees after it, the left one first.
    pub nodes: Vec<BuiltNode>,
    pub facets: Vec<Facet>,
    /// The vertices as the builder leaves them: through its identity transform, in place.
    pub vertices: Vec<Vf>,
}

/// One triangle while the tree is built: its corners, its plane, and its centroid.
#[derive(Clone, Copy, Debug)]
struct Work {
    corners: [i32; 3],
    normal: Vf,
    centroid: Vf,
    distance: F,
}

/// The plane through three corners: the unit normal of `(v1 - v0) × (v2 - v0)` and the distance that
/// puts `v0` on it. A degenerate triangle's normal is zero.
fn plane(v0: Vf, v1: Vf, v2: Vf) -> (Vf, F) {
    let b = v2.sub(v0);
    let a = v1.sub(v0);
    let n = Vf::opmsub(Vf::opmula(a, b), b, a);
    let n = a.merge(ps2_float::Dest::XYZ, n);
    let len2 = Acc::mula(n.x(), n.x()).madda(n.y(), n.y()).madd(n.z(), n.z());
    let n = if len2 == 0.0 {
        Vf::new(0.0, 0.0, 0.0, 1.0)
    } else {
        let q = F::ONE.rsqrt(len2);
        Vf::new((F(n.x()) * q).0, (F(n.y()) * q).0, (F(n.z()) * q).0, n.w())
    };
    let d = Acc::mula(n.x(), v0.x()).madda(n.y(), v0.y()).madd(n.z(), v0.z());
    (n, F(fpu::neg(d.0)))
}

/// The builder's transform, which is always the identity: its three rows and its translation, as it
/// leaves them in the mesh.
const ROWS: [Vf; 3] = [Vf::new(1.0, 0.0, 0.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0), Vf::new(0.0, 0.0, 1.0, 0.0)];

/// A point through the identity, as the builder does it in place: the translation (which comes out
/// `+0`) plus each row times its lane, on the accumulator.
fn through(v: Vf) -> Vf {
    let acc = Vf::ZERO.add(Vf::VF0.bc(vu::Field::X));
    let acc = Vf::madd(acc, ROWS[0], v.bc(vu::Field::X));
    let acc = Vf::madd(acc, ROWS[1], v.bc(vu::Field::Y));
    let out = Vf::madd(acc, ROWS[2], v.bc(vu::Field::Z));
    v.merge(ps2_float::Dest::XYZ, out)
}

/// A direction through the identity: the rows alone.
fn turned(v: Vf) -> Vf {
    let acc = ROWS[0].mul(v.bc(vu::Field::X));
    let acc = Vf::madd(acc, ROWS[1], v.bc(vu::Field::Y));
    let out = Vf::madd(acc, ROWS[2], v.bc(vu::Field::Z));
    v.merge(ps2_float::Dest::XYZ, out)
}

/// The box round every corner of `tris`: its centre, half extents, volume and radius.
///
/// The volume is the box's, each side held to at least a hundredth of a foot so that a flat box
/// still weighs something; the radius is the half extents' length.
fn bounds(tris: &[Work], verts: &[Vf]) -> ([F; 3], [F; 3], F, F) {
    let first = verts[tris[0].corners[0] as usize];
    let (mut lo, mut hi) = (first, first);
    for t in tris {
        for &k in &t.corners {
            let v = verts[k as usize];
            for l in 0..3 {
                if !(v.0[l] <= hi.0[l]) {
                    hi.0[l] = v.0[l];
                }
                if v.0[l] < lo.0[l] {
                    lo.0[l] = v.0[l];
                }
            }
        }
    }
    let centre = lo.add(hi).mul(Vf::splat(0.5));
    let half = hi.sub(lo).mul(Vf::splat(0.5));
    let (hx, hy, hz) = (F(half.x()), F(half.y()), F(half.z()));
    let radius = Acc::mula(hx, hx).madda(hy, hy).madd(hz, hz).sqrt();
    let least = F(0.01);
    let (sz, sx, sy) = ((F(2.0) * hz).abs().max(least), (F(2.0) * hx).abs().max(least), (F(2.0) * hy).abs().max(least));
    let volume = sz * (sx * sy);
    ([F(centre.x()), F(centre.y()), F(centre.z())], [hx, hy, hz], volume, radius)
}

/// `tris` split at `split` on `axis`: the centroids past it first, the rest second. Where either side
/// would be empty the list is cut in half instead, in its own order.
fn partition(tris: &[Work], axis: usize, split: F) -> (Vec<Work>, Vec<Work>) {
    let (past, rest): (Vec<Work>, Vec<Work>) = tris.iter().partition(|t| !(t.centroid.0[axis] <= split.0));
    if past.is_empty() || rest.is_empty() {
        let half = tris.len() / 2;
        return (tris[..half].to_vec(), tris[half..].to_vec());
    }
    (past, rest)
}

/// Where to split `tris`: the box round their centroids cut into twentieths, and of the cuts one to
/// eighteen twentieths along each axis the one whose two halves' boxes weigh least — the first such,
/// trying the longest axis's fourth cut before any.
fn choose(tris: &[Work], verts: &[Vf]) -> (usize, F) {
    let (mut lo, mut hi) = (tris[0].centroid, tris[0].centroid);
    for t in tris {
        for l in 0..3 {
            let c = t.centroid.0[l];
            if !(c <= hi.0[l]) {
                hi.0[l] = c;
            }
            if !(lo.0[l] <= c) {
                lo.0[l] = c;
            }
        }
    }
    let ext = |l: usize| F(hi.0[l]) - F(lo.0[l]);
    let step = |l: usize| ext(l) / 20.0;
    let mut longest = ext(0);
    if longest <= 0.0 {
        longest = F::ZERO;
    }
    let mut axis = usize::from(longest < ext(1));
    if axis == 1 {
        longest = ext(1);
    }
    if longest < ext(2) {
        axis = 2;
    }
    let weigh = |axis: usize, split: F| {
        let (past, rest) = partition(tris, axis, split);
        let (_, _, rest_volume, _) = bounds(&rest, verts);
        let (_, _, past_volume, _) = bounds(&past, verts);
        past_volume + rest_volume
    };
    let mut best = weigh(axis, F(lo.0[axis]) + step(axis) * 4.0);
    let mut best_cut = 3;
    for cut in 0..18 {
        for l in 0..3 {
            let split = F(lo.0[l]) + F::from(cut + 1) * step(l);
            let cost = weigh(l, split);
            if !(best <= cost) {
                best = cost;
                best_cut = cut;
                axis = l;
            }
        }
    }
    (axis, F(lo.0[axis]) + F::from(best_cut + 1) * step(axis))
}

/// One node and everything under it: its box round `tris`, its split, and its two halves — each a
/// triangle if it is one, else the next node in order.
fn node(nodes: &mut Vec<BuiltNode>, tris: &mut [Work], verts: &[Vf], first: usize) {
    let me = nodes.len() - 1;
    let (centre, half, volume, radius) = bounds(tris, verts);
    let (axis, split) = choose(tris, verts);
    let (past, rest) = partition(tris, axis, split);
    let left = past.len();
    for (slot, t) in tris.iter_mut().zip(past.into_iter().chain(rest)) {
        *slot = t;
    }
    nodes[me] = BuiltNode { volume, radius, children: [0; 2], centre, half };
    let n = tris.len();
    let (lo, hi) = tris.split_at_mut(left);
    if left == 1 {
        nodes[me].children[0] = first as u16 | 0x8000;
    } else {
        nodes[me].children[0] = nodes.len() as u16;
        nodes.push(nodes[me]);
        node(nodes, lo, verts, first);
    }
    if n - left == 1 {
        nodes[me].children[1] = (first + left) as u16 | 0x8000;
    } else {
        nodes[me].children[1] = nodes.len() as u16;
        nodes.push(nodes[me]);
        node(nodes, hi, verts, first + left);
    }
}

/// The engine's collision-tree builder over `vertices` (quadwords) and `triangles` (three corner
/// indices each). Two triangles at least: the builder's own root never handles one.
pub fn build(vertices: &[Vf], triangles: &[[i32; 3]]) -> Built {
    assert!(triangles.len() >= 2, "the builder needs two triangles, got {}", triangles.len());
    let v = |k: i32| vertices[k as usize];
    let mut work: Vec<Work> = triangles
        .iter()
        .map(|&corners| {
            let (normal, distance) = plane(v(corners[0]), v(corners[1]), v(corners[2]));
            Work { corners, normal, centroid: Vf::ZERO, distance }
        })
        .collect();
    let vertices: Vec<Vf> = vertices.iter().map(|&p| through(p)).collect();
    let third = vu::div(1.0, 3.0);
    for t in &mut work {
        t.normal = turned(t.normal);
        let [a, b, c] = t.corners.map(|k| vertices[k as usize]);
        t.centroid = a.add(b).add(c).mul_q(third);
    }
    let mut nodes = vec![BuiltNode { volume: F::ZERO, radius: F::ZERO, children: [0; 2], centre: [F::ZERO; 3], half: [F::ZERO; 3] }];
    node(&mut nodes, &mut work, &vertices, 0);
    let facets = work
        .iter()
        .map(|t| Facet {
            vertices: t.corners.map(|k| k as u16),
            distance: t.distance,
            normal: [F(t.normal.x()), F(t.normal.y()), F(t.normal.z())],
        })
        .collect();
    Built { nodes, facets, vertices }
}

impl Built {
    /// The tree as the port's sweep reads a mesh ([`crate::collide::sweep`]). The vertices are the
    /// ones the mesh keeps, copied before the builder's transform; the normals are the floats the
    /// builder packs.
    pub fn mesh(&self, before: &[Vf]) -> Mesh {
        let v3 = |v: Vf| [F(v.x()), F(v.y()), F(v.z())];
        Mesh {
            version: 0,
            surface: None,
            transform: [[F::ONE, F::ZERO, F::ZERO], [F::ZERO, F::ONE, F::ZERO], [F::ZERO, F::ZERO, F::ONE], [F::ZERO; 3]],
            vertices: Points::Wide(before.iter().map(|&v| v3(v)).collect()),
            normals: None,
            nodes: self
                .nodes
                .iter()
                .map(|n| Node { volume: n.volume, centre: n.centre, half_extent: n.half, children: n.children })
                .collect(),
            triangles: self
                .facets
                .iter()
                .map(|f| Triangle { distance: f.distance, normal: f.normal, vertices: f.vertices })
                .collect(),
        }
    }
}

// --- the plants --------------------------------------------------------------------------------------

/// A placed group: one plant as the broad phase holds it (S102) and the put-back's rule reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// The plant's cell, its species' slot, and its two size bytes.
    pub cell: [u16; 3],
    pub model: u8,
    pub blend: i8,
    pub height: i8,
    /// Its reach on the ground plane: the species' usual width, `blend` 128ths of the way to its widest.
    pub reach: F,
    /// Half its height: the species' usual, `height` 128ths of the usual less the least further, halved.
    pub half_height: F,
    /// Its species has solids (the count the original's part loop and the put-back read).
    pub solid: bool,
    /// Its parts, one per solid in the species' order: a trunk by its index in [`Trees::props`], or
    /// `None` for one no search the port runs takes a contact from (a sphere, or a kind not built).
    pub parts: Vec<Option<usize>>,
}

impl Group {
    /// The box the loader puts the plant in its world tree with: on the plant's cell (`cell` feet a
    /// step), from its foot up its height, `reach` either way across.
    pub fn bounds(&self, cell: F) -> ([F; 4], [F; 4]) {
        let at = |c: u16| F::from(i32::from(c)) * cell;
        let centre = [at(self.cell[0]), self.half_height + at(self.cell[1]), at(self.cell[2]), F::ZERO];
        (centre, [self.reach, self.half_height, self.reach, F::ZERO])
    }
}

/// Every trunk of an ecosystem, stood under its plants.
#[derive(Default)]
pub struct Trees {
    /// One prop per plant per solid: the species' mesh, moved to the plant's cell.
    pub props: Props,
    groups: Vec<Group>,
    cell: F,
    models: Vec<Model>,
}

impl Trees {
    /// The trunks of `eco` with plants on a grid of `cell` feet, their cylinders laid out off `curve`.
    /// The second half is a line per solid this module does not build (kinds other than 1 and 2).
    pub fn new(eco: &Ecosystem, cell: F, curve: &CurveTable) -> (Trees, Vec<String>) {
        let mut missed = Vec::new();
        let meshes: Vec<Vec<Option<(Mesh, BuiltNode)>>> = eco
            .species
            .iter()
            .enumerate()
            .map(|(slot, sp)| {
                let Some(sp) = sp else { return Vec::new() };
                sp.solids
                    .iter()
                    .map(|s| {
                        if s.kind == SPHERE {
                            // A part only the hit bit sees: see [`Trees`].
                            return None;
                        }
                        if s.kind != CYLINDER {
                            missed.push(format!("{} (slot {slot}): a solid of kind {}, not built", sp.model, s.kind));
                            return None;
                        }
                        let (corners, tris) = cylinder(s, curve);
                        let built = build(&corners, &tris);
                        Some((built.mesh(&corners), built.nodes[0]))
                    })
                    .collect()
            })
            .collect();
        let models = eco
            .species
            .iter()
            .map(|s| s.as_ref().map_or(Model::default(), |s| Model { near: s.width[0], far: s.width[2] }))
            .collect();
        let mut trees = Trees { props: Props::default(), groups: Vec::with_capacity(eco.plants.len()), cell, models };
        for p in &eco.plants {
            let slot = usize::from(p.species);
            let sp = eco.species[slot].as_ref();
            let width = sp.map_or([F::ZERO; 3], |s| s.width);
            let reach = width[0] + F::from(i32::from(p.width)) * (width[2] - width[0]) / 128.0;
            let height = sp.map_or([F::ZERO; 3], |s| s.height);
            let half_height = (height[0] + F::from(i32::from(p.height)) * (height[0] - height[1]) / 128.0) * 0.5;
            let mut parts = Vec::with_capacity(meshes[slot].len());
            let [x, y, z] = place(p.cell, cell);
            for m in &meshes[slot] {
                let Some((mesh, root)) = m else {
                    parts.push(None);
                    continue;
                };
                parts.push(Some(trees.props.props.len()));
                let to_world = Mat4([ROWS[0], ROWS[1], ROWS[2], Vf::new(x.0, y.0, z.0, 1.0)]);
                let from_world = Mat4([ROWS[0], ROWS[1], ROWS[2], Vf::new(fpu::neg(x.0), fpu::neg(y.0), fpu::neg(z.0), 1.0)]);
                let centre = Vf::new((root.centre[0] + x).0, (root.centre[1] + y).0, (root.centre[2] + z).0, 1.0);
                trees.props.props.push(Prop { mesh: mesh.clone(), to_world, from_world, centre, radius: root.radius, moving: None });
            }
            let solid = !parts.is_empty();
            trees.groups.push(Group { cell: p.cell, model: p.species, blend: p.width, height: p.height, reach, half_height, solid, parts });
        }
        (trees, missed)
    }

    /// A track's trees as its ride loads them (V50): `esb` read, every plant that asks stood on
    /// `ground` by the terrain object's own point query, and the trunks built on a cell of the map's
    /// larger side over 65536 — the field's samples times their spacing, as the renderer's grove
    /// measures it (V48). The second half is [`Trees::new`]'s.
    pub fn on_ground(esb: &[u8], ground: &world::Ground, curve: &CurveTable) -> Result<(Trees, Vec<String>), String> {
        let mut eco = Ecosystem::parse(esb)?;
        let extent = F::from(ground.samples_x.max(ground.samples_z) as i32) * ground.spacing;
        let field = ground.field();
        eco.stand(extent, |p| F(crate::contact_search::ground(&field, p).point.y()));
        Ok(Trees::new(&eco, cell_size(extent), curve))
    }

    /// The plants as placed groups, in the file's order.
    pub fn groups(&self) -> &[Group] {
        &self.groups
    }

    /// Feet a plant's cell steps.
    pub fn cell(&self) -> F {
        self.cell
    }

    /// Each species slot's reach at blend 0 and 128, as the broad phase measures a group by.
    pub fn models(&self) -> &[Model] {
        &self.models
    }

    /// Sweep a world-space bundle against every trunk it could reach ([`Props::sweep`]).
    pub fn sweep(&self, bundle: &Bundle, record: &mut Record) -> bool {
        self.props.sweep(bundle, record)
    }

    /// The put-back's rule for placed groups (S75): a group whose reach comes within `radius` of
    /// `centre` on the ground plane, with parts, and whose cell is within twenty feet of `centre`,
    /// refuses the spot.
    ///
    /// The original measures the reach once per gather and gathers again only when the spot has moved
    /// far enough; this measures it at `centre` every time.
    pub fn crowded(&self, centre: Vf, radius: F) -> bool {
        self.groups.iter().any(|g| {
            let [x, _, z] = place(g.cell, self.cell);
            let (dx, dz) = (x - F(centre.x()), z - F(centre.z()));
            let gap = Acc::mula(dx, dx).madd(dz, dz).sqrt() - (F::ZERO + g.reach);
            let gap = if gap <= 0.0 { F::ZERO } else { gap };
            gap < radius && g.solid && Acc::mula(dx, dx).madd(dz, dz) <= CROWDED
        })
    }
}

/// A world with an ecosystem's trunks standing in it: everything asked of `world` is asked of it,
/// and the trunks answer [`World::groups`] and add their refusals to [`World::crowded`].
pub struct Wooded<W: World> {
    pub world: W,
    pub trees: Trees,
}

impl<W: World> World for Wooded<W> {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.world.ground(at)
    }

    fn surface(&mut self, at: Vf) -> u8 {
        self.world.surface(at)
    }

    fn wheel_surface(&mut self, at: Vf) -> Option<u8> {
        self.world.wheel_surface(at)
    }

    fn solid(&mut self, bundle: &Bundle, record: &mut Record) -> bool {
        self.world.solid(bundle, record)
    }

    fn groups(&mut self, bundle: &Bundle, record: &mut Record) -> bool {
        let inner = self.world.groups(bundle, record);
        self.trees.sweep(bundle, record) | inner
    }

    fn crowded(&mut self, centre: Vf, radius: F) -> bool {
        self.world.crowded(centre, radius) || self.trees.crowded(centre, radius)
    }

    fn area(&self) -> Option<crate::biplane::Area> {
        self.world.area()
    }

    fn off_world(&self, at: Vf, margin: F) -> bool {
        self.world.off_world(at, margin)
    }

    fn wrap(&self) -> [bool; 2] {
        self.world.wrap()
    }

    fn keeps(&mut self, at: Vf) -> bool {
        self.world.keeps(at)
    }

    fn heightfield(&mut self) -> Option<Box<dyn Heightfield + '_>> {
        self.world.heightfield()
    }

    fn collision(&mut self) -> (Option<Box<dyn Heightfield + '_>>, Option<&world::Props>) {
        self.world.collision()
    }

    fn wooded(&mut self) -> (Option<Box<dyn Heightfield + '_>>, Option<&world::Props>, Option<&Trees>) {
        let (field, props) = self.world.collision();
        (field, props, Some(&self.trees))
    }

    fn moving(&mut self) -> Option<&mut world::Props> {
        self.world.moving()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trunk() -> (Vec<Vf>, Vec<[i32; 3]>) {
        let solid = Solid { kind: CYLINDER, at: [F::ZERO; 3], radius: F(1.5), height: F(40.0) };
        cylinder(&solid, &CurveTable::build())
    }

    #[test]
    fn a_trunk_is_thirty_one_nodes_over_thirty_two_triangles() {
        let (corners, tris) = trunk();
        let built = build(&corners, &tris);
        assert_eq!((built.nodes.len(), built.facets.len()), (31, 32));
        let mut seen = [false; 32];
        for n in &built.nodes {
            for c in n.children {
                if c & 0x8000 != 0 {
                    assert!(!seen[usize::from(c & 0x7fff)], "a triangle twice");
                    seen[usize::from(c & 0x7fff)] = true;
                }
            }
        }
        assert!(seen.iter().all(|&s| s), "every triangle is a leaf once");
        assert_eq!(built.nodes[0].half[1], F(20.0));
    }
}
