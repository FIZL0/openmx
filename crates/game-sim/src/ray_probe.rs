//! A ray probe's narrow phase: a tree of short segments descended against a mesh's tree of triangles.
//!
//! The biplane asks the world where its wings and wheels are about to touch with one of these: a fixed
//! set of segments in the plane's own frame (36 of them), kept in a bounding-volume tree of their own,
//! and a result [`Record`] that collects one contact per segment that struck something. The nearby
//! search ([`crate::world_search`]) hands the probe each candidate it gathered; [`pair`] is what the
//! probe does with one, and [`clear`] is what it does before the first.
//!
//! The descent is two trees against each other ([`crate::collide`] is one tree against a bundle):
//!
//! * **node against node** — two oriented boxes, the full fifteen-axis separating test, each axis
//!   skipped early when the first box's own extent already covers the distance; then the side whose
//!   node has the larger key opens (the mesh's on a tie), and both its children are always tried;
//! * **segment against node** — the segment into the mesh's frame, its midpoint and half-extent
//!   against the box on the three axes and the three crosses;
//! * **node against triangle** — the triangle into the probe's frame, thirteen axes (its normal, the
//!   three box axes, and the box axes crossed with its three edges — the third edge normalised, the
//!   first two not);
//! * **segment against triangle** — [`crate::collide::test`], which is the same test the original
//!   runs here, on the segment taken into the mesh's frame.
//!
//! One of the original's choices is kept because changing it changes what the plane hits:
//!
//! * **A contact is per segment, not per triangle.** With [`Record::one_per_ray`] set (the plane's is),
//!   a segment that strikes a second triangle replaces its first contact only when nearer, and a
//!   segment that struck nothing never appears. Contacts are left in the struck mesh's frame until the
//!   pair is over, then taken to the world through it and marked [`Contact::placed`] — a contact that a
//!   later mesh updates is taken through *that* mesh's transform.
//!
//! What is not here: the probe's preparation (the search's `prepare` answer, which only runs when the
//! search carries an object to prepare against), the smooth-normal variant of the segment test (the
//! probe's flag for it is clear), a probe with a per-ray filter attached, and the other shapes the
//! original dispatches to (heightfields and the like) — none of them is in the plane's search. The
//! journal of S46 has their addresses.

#![deny(clippy::float_arithmetic)]
// Comparisons are written the way the original branches (see `collide`): `!(a <= b)` where the
// instruction is a `c.ole.s` whose true arm continues.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use crate::collide::{self, MeshRef};
use crate::track::{Node, Points};
use ps2_float::{F, Vf, fpu, vu};

/// A frame: three rotation rows then the position, row-vector convention (`p·R + t`).
pub type Matrix = [Vf; 4];

/// One of the probe's rays, in the probe's frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Segment {
    pub from: Vf,
    pub to: Vf,
}

/// A triangle as a mesh keeps it in memory: three vertex indices, and its face normal packed as three
/// shorts in units of 1/16384.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Facet {
    pub vertices: [u16; 3],
    pub normal: [i16; 3],
}

/// The probe: its frame, and its segments with the tree over them. With no tree (`nodes` empty) the
/// group gate boxes the first segment instead.
#[derive(Clone, Copy, Debug)]
pub struct RayProbe<'a> {
    pub transform: Matrix,
    pub root: u16,
    pub nodes: &'a [Node],
    pub segments: &'a [Segment],
}

/// A collision mesh: its frame, its tree, its triangles and vertices, and who it is — the identity a
/// contact records as what it struck.
#[derive(Clone, Copy, Debug)]
pub struct Mesh<'a> {
    pub id: u32,
    pub transform: Matrix,
    pub root: u16,
    pub nodes: &'a [Node],
    pub facets: &'a [Facet],
    pub vertices: &'a Points,
}

/// One part of a [`Hierarchy`], and whether it is switched on.
#[derive(Clone, Copy, Debug)]
pub struct Part<'a> {
    pub enabled: bool,
    pub mesh: Mesh<'a>,
}

/// Meshes placed together (a building, a hangar): one box around all of them in the group's own
/// frame, which gates the lot, then each part on its own.
#[derive(Clone, Copy, Debug)]
pub struct Hierarchy<'a> {
    pub centre: Vf,
    pub half: Vf,
    pub frame: Matrix,
    pub parts: &'a [Part<'a>],
}

/// What the probe can be paired with.
#[derive(Clone, Copy, Debug)]
pub enum Shape<'a> {
    Mesh(Mesh<'a>),
    Hierarchy(Hierarchy<'a>),
    /// Spheres, rays and the other probe-like classes: the ray probe never strikes them.
    Never,
}

/// One segment's contact.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Contact {
    /// The segment, as the probe keeps it (its own frame, never transformed).
    pub from: Vf,
    pub to: Vf,
    pub normal: Vf,
    pub point: Vf,
    /// The [`Mesh::id`] that placed it.
    pub object: u32,
    /// Taken to the world. A contact the descent writes or updates is not, until its pair ends.
    pub placed: bool,
    /// How far along its segment, as a fraction of it.
    pub fraction: F,
    /// The segment's child word in the probe's tree (leaf bit included).
    pub segment: u16,
}

/// The probe's result.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Record {
    /// The nearest contact's segment, as the probe keeps it.
    pub from: Vf,
    pub to: Vf,
    /// The nearest contact's normal and point.
    pub normal: Vf,
    pub point: Vf,
    /// The push out of what was struck: [`Record::low`] plus [`Record::high`] after a pair.
    pub push: Vf,
    /// The least and greatest of the per-contact blocked travel along the normal, per axis.
    pub low: Vf,
    pub high: Vf,
    /// The nearest hit of the last pair, and its fraction. Reset to 1 at the start of each pair.
    pub first_point: Vf,
    pub first: F,
    /// The last part of a group that was struck.
    pub part: u32,
    /// Whether the probe keeps a contact list at all.
    pub listed: bool,
    pub capacity: usize,
    pub contacts: Vec<Contact>,
    /// One contact per segment: a second hit of the same segment updates its contact.
    pub one_per_ray: bool,
    /// The nearest fraction of all.
    pub nearest: F,
    /// Bit 0: the nearest fields are in the world already. Bit 1: the nearest normal is a smooth one.
    pub flags: u8,
}

/// [`Record::flags`]: the nearest fields have been taken to the world.
pub const PLACED: u8 = 0x01;
/// [`Record::flags`]: the nearest normal was interpolated from the vertices' (never set here).
pub const SMOOTH: u8 = 0x02;

/// `(0, 0, 0, 1)`, which the original keeps as a constant and copies whole.
const ORIGIN: Vf = Vf::VF0;

/// The probe's clear, before a search: its hit bit lowered (`flags` is the probe's
/// [`crate::world_search::Collidable::flags`]) and the result emptied. The nearest normal and point,
/// the nearest pair's point and [`Record::flags`] are left as they were.
pub fn clear(flags: &mut u8, rec: &mut Record) {
    *flags &= !crate::world_search::flags::HIT;
    rec.from = ORIGIN;
    rec.to = ORIGIN;
    rec.nearest = F::ONE;
    rec.contacts.clear();
    rec.push = ORIGIN;
    rec.low = ORIGIN;
    rec.high = ORIGIN;
    rec.first = F::ONE;
}

/// The probe against one shape: whether any segment struck it, with the record updated and every new
/// contact taken to the world.
pub fn pair(probe: &RayProbe, other: &Shape, rec: &mut Record) -> bool {
    rec.first = F::ONE;
    match other {
        Shape::Mesh(mesh) => {
            if !tree(probe, mesh, rec) {
                return false;
            }
            let t = &mesh.transform;
            if rec.flags & PLACED == 0 {
                rec.normal = rotate(t, rec.normal);
                rec.low = rotate(t, rec.low);
                rec.high = rotate(t, rec.high);
                rec.point = place(t, rec.point);
                rec.flags |= PLACED;
            }
            rec.first_point = place(t, rec.first_point);
            rec.push = keep_w(rec.low.add(rec.high), rec.low);
            settle(rec, mesh);
            true
        }
        Shape::Hierarchy(group) => {
            let (mut low, mut high) = (ORIGIN, ORIGIN);
            let mut struck = false;
            if !group_reaches(probe, group) {
                return false;
            }
            for part in group.parts {
                if !part.enabled || !tree(probe, &part.mesh, rec) {
                    continue;
                }
                struck = true;
                for i in 0..3 {
                    low.0[i] = fpu::min(low.0[i], rec.low.0[i]);
                }
                for i in 0..3 {
                    high.0[i] = fpu::max(high.0[i], rec.high.0[i]);
                }
                rec.push = keep_w(low.add(high), low);
                let t = &part.mesh.transform;
                if rec.flags & PLACED == 0 {
                    rec.normal = rotate(t, rec.normal);
                    rec.low = rotate(t, rec.low);
                    rec.high = rotate(t, rec.high);
                    rec.push = rotate(t, rec.push);
                    rec.point = place(t, rec.point);
                    rec.first_point = place(t, rec.first_point);
                    rec.flags |= PLACED;
                }
                rec.part = part.mesh.id;
                rec.low = ORIGIN;
                rec.high = ORIGIN;
                settle(rec, &part.mesh);
            }
            struck
        }
        Shape::Never => false,
    }
}

/// Every contact the descent wrote, taken to the world through the mesh that holds it.
fn settle(rec: &mut Record, mesh: &Mesh) {
    for c in rec.contacts.iter_mut().filter(|c| !c.placed) {
        c.normal = rotate(&mesh.transform, c.normal);
        c.point = place(&mesh.transform, c.point);
        c.object = mesh.id;
        c.placed = true;
    }
}

/// The group's gate: the probe's box (its tree's first node, or its first segment) against the
/// group's, in the group's frame.
pub fn group_reaches(probe: &RayProbe, group: &Hierarchy) -> bool {
    let (centre, half) = match probe.nodes.first() {
        Some(n) => (point_of(n.centre, 1.0), point_of(n.half_extent, 1.0)),
        None => {
            let s = probe.segments[0];
            let mid = keep_w(s.to.add(s.from), s.to);
            let span = keep_w(s.to.sub(s.from), s.to);
            (keep_w(mid.mul(Vf::splat(0.5)), mid), keep_w(span.mul(Vf::splat(0.5)), span))
        }
    };
    let into_group = compose(&invert(&group.frame), &probe.transform);
    boxes(group.centre, group.half, centre, half, &into_group, &abs_rows(&into_group))
}

/// The tree against the tree: the two frames related, then the descent from both roots.
fn tree(probe: &RayProbe, mesh: &Mesh, rec: &mut Record) -> bool {
    Descent::new(probe, mesh, rec).run(probe.root, mesh.root)
}

/// One of the descent's mixed steps on its own: segment `a` (a leaf word of the probe's tree) against
/// node `b` of the mesh's, or node `a` of the probe's against triangle `b` (a leaf word of the mesh's).
pub fn leaf_reaches(probe: &RayProbe, mesh: &Mesh, a: u16, b: u16) -> bool {
    Descent::new(probe, mesh, &mut Record::default()).node_leaf(a, b)
}

struct Descent<'a, 'r> {
    probe: &'a RayProbe<'a>,
    mesh: &'a Mesh<'a>,
    vertices: MeshRef<'a>,
    to_probe: Matrix,
    to_mesh: Matrix,
    to_mesh_abs: [Vf; 3],
    rec: &'r mut Record,
}

const LEAF: u16 = 0x8000;

impl<'a, 'r> Descent<'a, 'r> {
    fn new(probe: &'a RayProbe<'a>, mesh: &'a Mesh<'a>, rec: &'r mut Record) -> Self {
        let to_probe = compose(&invert(&probe.transform), &mesh.transform);
        let to_mesh = invert(&to_probe);
        Descent {
            probe,
            mesh,
            vertices: MeshRef { nodes: mesh.nodes, triangles: &[], vertices: mesh.vertices },
            to_probe,
            to_mesh,
            to_mesh_abs: abs_rows(&to_mesh),
            rec,
        }
    }

    /// One step: `a` a child word of the probe's tree, `b` of the mesh's. Both children of whichever
    /// side descends are always tried; the answer is whether anything below struck.
    fn run(&mut self, a: u16, b: u16) -> bool {
        let (a_leaf, b_leaf) = (a & LEAF != 0, b & LEAF != 0);
        if a_leaf && b_leaf {
            return self.leaf(a, b);
        }
        if a_leaf || b_leaf {
            if !self.node_leaf(a, b) {
                return false;
            }
            return if a_leaf { self.mesh_children(a, b) } else { self.probe_children(a, b) };
        }
        let (pn, mn) = (&self.probe.nodes[a as usize], &self.mesh.nodes[b as usize]);
        // The probe's box goes into the mesh's frame, beside the mesh's.
        if !boxes(point_of(mn.centre, 0.0), point_of(mn.half_extent, 0.0), point_of(pn.centre, 0.0),
                  point_of(pn.half_extent, 0.0), &self.to_mesh, &self.to_mesh_abs) {
            return false;
        }
        if pn.volume <= mn.volume { self.mesh_children(a, b) } else { self.probe_children(a, b) }
    }

    fn mesh_children(&mut self, a: u16, b: u16) -> bool {
        let [c0, c1] = self.mesh.nodes[b as usize].children;
        let first = self.run(a, c0);
        let second = self.run(a, c1);
        first | second
    }

    fn probe_children(&mut self, a: u16, b: u16) -> bool {
        let [c0, c1] = self.probe.nodes[a as usize].children;
        let first = self.run(c0, b);
        let second = self.run(c1, b);
        first | second
    }

    /// A segment against a mesh node, or a probe node against a triangle.
    fn node_leaf(&self, a: u16, b: u16) -> bool {
        if a & LEAF != 0 {
            let s = self.probe.segments[(a & !LEAF) as usize];
            let n = &self.mesh.nodes[b as usize];
            return segment_reaches(point_of(n.centre, 0.0), n.half_extent, s.from, s.to, &self.to_mesh);
        }
        let n = &self.probe.nodes[a as usize];
        let facet = &self.mesh.facets[(b & !LEAF) as usize];
        let normal = unpack(facet.normal);
        let corners = facet.vertices.map(|i| self.vertices.vertex(i));
        let [v0, v1, v2] = corners.map(|v| place4(&self.to_probe, v));
        let normal = rotate4(&self.to_probe, normal);
        facet_reaches(point_of(n.centre, 0.0), n.half_extent, [v0, v1, v2], normal)
    }

    /// A segment against a triangle, and the record updated with what it struck.
    fn leaf(&mut self, a: u16, b: u16) -> bool {
        let seg = self.probe.segments[(a & !LEAF) as usize];
        let facet = &self.mesh.facets[(b & !LEAF) as usize];
        let face = unpack(facet.normal);
        let from = place(&self.to_mesh, seg.from);
        let to = place(&self.to_mesh, seg.to);
        let delta = keep_w(to.sub(from), to);
        let length = F(fpu::sqrt(fpu::dot(xyz(delta), xyz(delta))));
        let q = vu::div(1.0, length.0);
        let dir = keep_w(delta.mul_q(q), delta);
        let probe = collide::Probe { origin: from, dir, length };
        let Some((t, point, normal)) = collide::test(&probe, facet.vertices.map(|i| self.vertices.vertex(i)))
        else {
            return false;
        };
        let rec = &mut *self.rec;
        if !rec.listed {
            if t < rec.nearest {
                rec.nearest = t;
                rec.normal = normal;
                rec.flags &= !SMOOTH;
            }
            return true;
        }
        if t < rec.first {
            rec.first_point = point;
            rec.first = t;
        }
        let mut added = false;
        let found = if rec.one_per_ray { rec.contacts.iter().position(|c| c.segment == a) } else { None };
        match found {
            Some(i) => {
                let c = &mut rec.contacts[i];
                if t < c.fraction {
                    c.placed = false;
                    c.fraction = t;
                    c.point = point;
                    c.normal = normal;
                    added = true;
                }
            }
            None if rec.contacts.len() < rec.capacity => {
                rec.contacts.push(Contact {
                    from: seg.from,
                    to: seg.to,
                    normal,
                    point,
                    object: 0,
                    placed: false,
                    fraction: t,
                    segment: a,
                });
                added = true;
            }
            None => {}
        }
        if added {
            let travel = keep_w(dir.mul(Vf::splat(length.0)), dir);
            let blocked = keep_w(travel.mul(Vf::splat((F::ONE - t).0)), travel);
            let k = fpu::dot(xyz(blocked), xyz(face));
            let along = face.mul(Vf::splat(k));
            for i in 0..3 {
                rec.low.0[i] = fpu::min(along.0[i], rec.low.0[i]);
            }
            for i in 0..3 {
                rec.high.0[i] = fpu::max(along.0[i], rec.high.0[i]);
            }
        }
        if t <= rec.nearest {
            rec.flags &= !PLACED;
            rec.nearest = t;
            rec.from = seg.from;
            rec.to = seg.to;
            rec.point = point;
            rec.normal = normal;
            rec.flags &= !SMOOTH;
        }
        true
    }
}

// --- the four shape tests --------------------------------------------------------------------------

/// Box `b` against box `a`, both oriented: `m` takes `b`'s frame into `a`'s and `m_abs` is its rows'
/// magnitudes. The fifteen axes in the original's order — `a`'s three (x, z, y, each skipped when
/// `a`'s own extent covers the distance on it), `b`'s three, then the nine crosses, each passed early
/// when `a`'s share of the radius already covers it.
pub fn boxes(ac: Vf, ah: Vf, bc: Vf, bh: Vf, m: &Matrix, m_abs: &[Vf; 3]) -> bool {
    let d = keep_w(place(m, bc).sub(ac), bc);
    let ad = abs(d);
    let [ahx, ahy, ahz] = [F(ah.x()), F(ah.y()), F(ah.z())];
    let [bhx, bhy, bhz] = [F(bh.x()), F(bh.y()), F(bh.z())];
    let am = |r: usize, l: usize| F(m_abs[r].0[l]);
    let r = |row: usize, l: usize| F(m[row].0[l]);
    let [dx, dy, dz] = [F(d.x()), F(d.y()), F(d.z())];

    for (k, ahk) in [(0, ahx), (2, ahz), (1, ahy)] {
        let dk = F(ad.0[k]);
        if !(dk <= ahk) {
            let reach = ahk + ((bhx * am(0, k) + bhy * am(1, k)) + bhz * am(2, k));
            if !(dk <= reach) {
                return false;
            }
        }
    }
    for (j, bhj) in [(0, bhx), (1, bhy), (2, bhz)] {
        let p = (dx * r(j, 0)).madd(dy, r(j, 1)).madd(dz, r(j, 2)).abs();
        let reach = ((ahx * am(j, 0) + ahy * am(j, 1)) + ahz * am(j, 2)) + bhj;
        if !(p <= reach) {
            return false;
        }
    }
    // The crosses: (distance projection, a's share, b's share), as the listing pairs them.
    let crosses: [(F, F, F); 9] = [
        ((dy * -r(0, 2)).madd(dz, r(0, 1)), ahy * am(0, 2) + ahz * am(0, 1), bhy * am(2, 0) + bhz * am(1, 0)),
        ((dy * -r(1, 2)).madd(dz, r(1, 1)), ahy * am(1, 2) + ahz * am(1, 1), bhx * am(2, 0) + bhz * am(0, 0)),
        ((dy * -r(2, 2)).madd(dz, r(2, 1)), ahy * am(2, 2) + ahz * am(2, 1), bhx * am(1, 0) + bhy * am(0, 0)),
        ((dx * r(0, 2)).madd(dz, -r(0, 0)), ahx * am(0, 2) + ahz * am(0, 0), bhy * am(2, 1) + bhz * am(1, 1)),
        ((dx * r(1, 2)).madd(dz, -r(1, 0)), ahx * am(1, 2) + ahz * am(1, 0), bhx * am(2, 1) + bhz * am(0, 1)),
        ((dx * r(2, 2)).madd(dz, -r(2, 0)), ahx * am(2, 2) + ahz * am(2, 0), bhx * am(1, 1) + bhy * am(0, 1)),
        ((dx * -r(0, 1)).madd(dy, r(0, 0)), ahx * am(0, 1) + ahy * am(0, 0), bhy * am(2, 2) + bhz * am(1, 2)),
        ((dx * -r(1, 1)).madd(dy, r(1, 0)), ahx * am(1, 1) + ahy * am(1, 0), bhx * am(2, 2) + bhz * am(0, 2)),
        ((dx * -r(2, 1)).madd(dy, r(2, 0)), ahx * am(2, 1) + ahy * am(2, 0), bhx * am(1, 2) + bhy * am(0, 2)),
    ];
    for (p, a_share, b_share) in crosses {
        let p = p.abs();
        if !(p <= a_share) && !(p <= a_share + b_share) {
            return false;
        }
    }
    true
}

/// A segment (in the probe's frame; `m` takes it to the box's) against an axis-aligned box: its
/// midpoint and half-extent on the box's three axes, then on the three crosses.
pub fn segment_reaches(c: Vf, h: [F; 3], from: Vf, to: Vf, m: &Matrix) -> bool {
    let mut p = place(m, from);
    let mut q = place(m, to);
    for i in 0..3 {
        q.0[i] = (F(q.0[i]) - F(c.0[i])).0;
    }
    for i in 0..3 {
        p.0[i] = (F(p.0[i]) - F(c.0[i])).0;
    }
    let sum = keep_w(p.add(q), p);
    let mid = keep_w(sum.mul(Vf::splat(0.5)), sum);
    let half = keep_w(mid.sub(p), mid);
    let (am, ah) = (abs(mid), abs(half));
    let mut out = am;
    for i in 0..3 {
        out.0[i] = (F(out.0[i]) - h[i]).0;
    }
    let out = keep_w(out.sub(ah), out);
    if !(F(out.x()) <= F::ZERO && F(out.y()) <= F::ZERO && F(out.z()) <= F::ZERO) {
        return false;
    }
    let [mx, my, mz] = [F(mid.x()), F(mid.y()), F(mid.z())];
    let [hx, hy, hz] = [F(half.x()), F(half.y()), F(half.z())];
    let [ax, ay, az] = [F(ah.x()), F(ah.y()), F(ah.z())];
    let [ex, ey, ez] = h;
    (my * hz - mz * hy).abs() <= ey * az + ez * ay
        && (mx * hz - mz * hx).abs() <= ex * az + ez * ax
        && (mx * hy - my * hx).abs() <= ex * ay + ey * ax
}

/// A box (centre and half extents, axis-aligned) against a triangle already in its frame: thirteen
/// axes — the normal, the box's three, and the box's three crossed with each edge. The first two edges
/// (from the first corner) are used as they are; the third is normalised first.
pub fn facet_reaches(c: Vf, h: [F; 3], corners: [Vf; 3], normal: Vf) -> bool {
    let [v0, v1, v2] = corners;
    let half = Vf([h[0].0, h[1].0, h[2].0, 0.0]);
    let d = keep_w(v0.sub(c), v0);
    let (mut e0, mut e1, mut e2) = (Vf::ZERO, Vf::ZERO, Vf::ZERO);
    let zero = 0.0;
    for axis in 0..13 {
        let a = match axis {
            0 => normal,
            1 => Vf([1.0, 0.0, 0.0, 1.0]),
            2 => Vf([0.0, 1.0, 0.0, 1.0]),
            3 => Vf([0.0, 0.0, 1.0, 1.0]),
            4 => Vf([zero, fpu::neg(e0.z()), e0.y(), 1.0]),
            5 => Vf([e0.z(), zero, fpu::neg(e0.x()), 1.0]),
            6 => Vf([fpu::neg(e0.y()), e0.x(), zero, 1.0]),
            7 => Vf([zero, fpu::neg(e1.z()), e1.y(), 1.0]),
            8 => Vf([e1.z(), zero, fpu::neg(e1.x()), 1.0]),
            9 => Vf([fpu::neg(e1.y()), e1.x(), zero, 1.0]),
            10 => {
                e2 = normalize(keep_w(v2.sub(v1), v2));
                Vf([zero, fpu::neg(e2.z()), e2.y(), 1.0])
            }
            11 => Vf([e2.z(), zero, fpu::neg(e2.x()), 1.0]),
            _ => Vf([fpu::neg(e2.y()), e2.x(), zero, 1.0]),
        };
        let spread = abs(a.mul(half));
        let reach = F(spread.z()) + (F(spread.x()) + F(spread.y()));
        let p = F(fpu::dot(xyz(d), xyz(a)));
        if axis == 0 {
            if !(p.abs() <= reach) {
                return false;
            }
            e0 = keep_w(v0.sub(v1), v0);
            e1 = keep_w(v0.sub(v2), v0);
            continue;
        }
        let q0 = F(fpu::dot(xyz(e0), xyz(a)));
        let q1 = F(fpu::dot(xyz(e1), xyz(a)));
        let (p1, p2) = (p - q0, p - q1);
        if reach < p.min(p1.min(p2)) {
            return false;
        }
        if !(-reach <= p.max(p1.max(p2))) {
            return false;
        }
    }
    true
}

// --- frames ------------------------------------------------------------------------------------------

/// The inverse of a rigid frame: the rotation transposed (each moved lane added to zero, as the VU
/// moves it) and the position taken back through it, the `w` lanes left as they were.
pub fn invert(m: &Matrix) -> Matrix {
    let [r0, r1, r2, t] = *m;
    let z = |x: f32| vu::add(0.0, x);
    let n0 = Vf([r0.x(), z(r1.x()), z(r2.x()), r0.w()]);
    let n1 = Vf([z(r0.y()), r1.y(), z(r2.y()), r1.w()]);
    let n2 = Vf([z(r0.z()), z(r1.z()), r2.z(), r2.w()]);
    let mut p = t;
    for i in 0..3 {
        let acc = vu::add(0.0, 0.0);
        let acc = vu::msub(acc, n0.0[i], t.x());
        let acc = vu::msub(acc, n1.0[i], t.y());
        p.0[i] = vu::msub(acc, n2.0[i], t.z());
    }
    [n0, n1, n2, p]
}

/// `outer` applied to `inner`'s rows: a frame that goes through `inner` and then `outer`. All four
/// lanes of each row, as the original computes them.
pub fn compose(outer: &Matrix, inner: &Matrix) -> Matrix {
    [rotate4(outer, inner[0]), rotate4(outer, inner[1]), rotate4(outer, inner[2]), place4(outer, inner[3])]
}

/// The three rotation rows' magnitudes, every lane.
pub fn abs_rows(m: &Matrix) -> [Vf; 3] {
    [abs(m[0]), abs(m[1]), abs(m[2])]
}

/// `v` through `m`'s rotation and position, `xyz` written and `w` kept: `vaddax` of the position, then
/// `vmaddax` / `vmadday` / `vmaddz`.
fn place(m: &Matrix, v: Vf) -> Vf {
    keep_w(place4(m, v), v)
}

/// [`place`] with the `w` lane computed too (the `.xyzw` form).
fn place4(m: &Matrix, v: Vf) -> Vf {
    let acc = Vf(core::array::from_fn(|i| vu::add(m[3].0[i], 0.0)));
    Vf(core::array::from_fn(|i| {
        let acc = vu::madd(acc.0[i], m[0].0[i], v.x());
        let acc = vu::madd(acc, m[1].0[i], v.y());
        vu::madd(acc, m[2].0[i], v.z())
    }))
}

/// `v` through `m`'s rotation alone, `xyz` written and `w` kept: `vmulax` / `vmadday` / `vmaddz`.
fn rotate(m: &Matrix, v: Vf) -> Vf {
    keep_w(rotate4(m, v), v)
}

/// [`rotate`] with the `w` lane computed too.
fn rotate4(m: &Matrix, v: Vf) -> Vf {
    Vf(core::array::from_fn(|i| {
        let acc = vu::mul(m[0].0[i], v.x());
        let acc = vu::madd(acc, m[1].0[i], v.y());
        vu::madd(acc, m[2].0[i], v.z())
    }))
}

// --- small helpers -----------------------------------------------------------------------------------

/// A packed face normal: each short over 16384, which the original gets exactly by adding it to the
/// bits of 768.0 and subtracting 768.0. Its `w` is whatever was on the stack; nothing reads it.
fn unpack(n: [i16; 3]) -> Vf {
    let one = |s: i16| (F(f32::from_bits(0x4440_0000u32.wrapping_add(s as i32 as u32))) - F(768.0)).0;
    Vf([one(n[0]), one(n[1]), one(n[2]), 0.0])
}

/// Normalise `xyz` the way the game's helper does (see `collide`): the squares summed `(x + y) + z`,
/// the VU's reciprocal square root, `w` untouched.
fn normalize(v: Vf) -> Vf {
    let sq = v.mul(v);
    let len2 = vu::madd(vu::add(sq.x(), sq.y()), 1.0, sq.z());
    let q = vu::rsqrt(1.0, len2);
    Vf([vu::mul(v.x(), q), vu::mul(v.y(), q), vu::mul(v.z(), q), v.w()])
}

/// The sign bits cleared, every lane (`pand` with `0x7fffffff`: nothing flushed).
fn abs(v: Vf) -> Vf {
    Vf(v.0.map(|x| f32::from_bits(x.to_bits() & 0x7fff_ffff)))
}

fn keep_w(result: Vf, w_from: Vf) -> Vf {
    Vf([result.x(), result.y(), result.z(), w_from.w()])
}

fn xyz(v: Vf) -> [f32; 3] {
    [v.x(), v.y(), v.z()]
}

fn point_of(v: [F; 3], w: f32) -> Vf {
    Vf([v[0].0, v[1].0, v[2].0, w])
}
