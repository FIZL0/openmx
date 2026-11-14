//! **A rider part's sphere** (S279): SLUS-21104's sphere collidable (the shape the rider ragdoll
//! world's bone collisions place and search, [`crate::handling::bone`]'s `Scene`), its place, its
//! refresh, its clear and its search over a nearby list.
//!
//! The sphere keeps a local point (`+0x130`), a frame (`+0x140`, four rows), a radius and its square
//! (`+0x180`, `+0x184`); as a collidable, its centre (`+0x50`) and reach (`+0x70`).
//!
//! - [`place`]: the point taken whole when there is one, the radius and its square
//!   when there is one (the bone collision hands no radius).
//! - [`refresh`] (vt `200`): the frame its node's world frame, or the identity when it has no node
//!   (`+0x190`; no rider sphere has one); the centre the frame times the point (`m3 + 0`, then
//!   `m0·x`, `m1·y`, `m2·z` accumulated, `xyz`; `w` the point's) and the reach the radius. The
//!   refresh does not move the sphere's entry in the world's spatial database.
//! - The search (`collide_against_search`, [`crate::atv_search::search`]) with [`Search`], whose clear
//!   (vt `+0xb0`) lowers the hit bit and empties the record (`+0x7c`: its count `+4` to 0, made the
//!   current record).
//!
//! - The pair (vt `+0xb4`, [`pair`], S279a): the sphere against one collidable, by its kind — a box
//!   (6) and another sphere (5) say only whether they meet; a mesh (0) and a hierarchy's parts (1)
//!   walk the mesh's box tree for the nearest triangle the sphere reaches and write it in the
//!   sphere's [`Record`]. [`Search`] hands it to its caller ([`Search::rest`]), which knows what each
//!   collidable is.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::atv_part::{self, node_reaches_sphere};
use crate::collide::MeshRef;
use crate::contact::Node;
use crate::ragdoll_scene::{dot, inside, minus, unit, unpack};
use crate::track::Child;
use crate::world_search::{Collidable, Narrow, flags};
use ps2_float::{Acc, Dest, F, Vf, fpu, vu};

/// Four rows: three axes, then the position.
pub type Matrix = [Vf; 4];

/// The frame a sphere with no node takes: built from `1.0` and zeros (`pcpyld`/`pextlw`), the last row
/// `vf0`.
pub const IDENTITY: Matrix = [Vf([1.0, 0.0, 0.0, 0.0]), Vf([0.0, 1.0, 0.0, 0.0]), Vf([0.0, 0.0, 1.0, 0.0]), Vf([0.0, 0.0, 0.0, 1.0])];

/// The sphere's own words.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sphere {
    /// `+0x130`: the point, in the frame.
    pub local: Vf,
    /// `+0x140`: the frame.
    pub frame: Matrix,
    /// `+0x180`, `+0x184`.
    pub radius: F,
    pub radius2: F,
    /// `+0x50`, `+0x70`: where the search finds it.
    pub centre: Vf,
    pub reach: F,
}

/// **The place**: the point `at` (all four lanes) and the radius `r` (and `r·r`), each when given.
pub fn place(s: &mut Sphere, at: Option<Vf>, r: Option<F>) {
    if let Some(p) = at {
        s.local = p;
    }
    if let Some(r) = r {
        s.radius = r;
        s.radius2 = r * r;
    }
}

/// **The refresh**: the frame (`node`'s world frame, or [`IDENTITY`]), then the centre and reach.
pub fn refresh(s: &mut Sphere, node: Option<Matrix>) {
    s.frame = node.unwrap_or(IDENTITY);
    let (m, p) = (s.frame, s.local);
    let acc = Vf::madd(m[3].add(Vf::ZERO), m[0], Vf([p.x(); 4]));
    let acc = Vf::madd(acc, m[1], Vf([p.y(); 4]));
    let c = Vf::madd(acc, m[2], Vf([p.z(); 4]));
    s.centre = Vf([c.x(), c.y(), c.z(), p.w()]);
    s.reach = s.radius;
}

/// The sphere's narrow phase in a search: its own clear, everything else (the pair, a group's parts,
/// the listeners and callbacks) `rest`'s.
pub struct Search<'a> {
    pub rest: &'a mut dyn Narrow,
    /// Whether the clear ran (the record emptied: the caller writes its count and the current
    /// record).
    pub cleared: bool,
}

impl<'a> Search<'a> {
    pub fn new(rest: &'a mut dyn Narrow) -> Search<'a> {
        Search { rest, cleared: false }
    }
}

impl Narrow for Search<'_> {
    /// The clear (vt `+0xb0`): the hit bit down and the record emptied.
    fn clear(&mut self, objects: &mut [Collidable], probe: usize) {
        objects[probe].flags &= !flags::HIT;
        self.cleared = true;
    }
    fn prepare(&mut self, objects: &mut [Collidable], probe: usize) {
        self.rest.prepare(objects, probe)
    }
    fn pair(&mut self, objects: &mut [Collidable], probe: usize, other: usize) -> bool {
        self.rest.pair(objects, probe, other)
    }
    fn parts(&mut self, objects: &mut [Collidable], group: usize) -> usize {
        self.rest.parts(objects, group)
    }
    fn part(&mut self, objects: &mut [Collidable], group: usize, i: usize) -> Option<usize> {
        self.rest.part(objects, group, i)
    }
    fn heard_hit(&mut self, objects: &mut [Collidable], listener: usize, probe: usize, other: usize) {
        self.rest.heard_hit(objects, listener, probe, other)
    }
    fn heard_struck(&mut self, objects: &mut [Collidable], listener: usize, struck: usize, probe: usize) {
        self.rest.heard_struck(objects, listener, struck, probe)
    }
    fn on_hit(&mut self, objects: &mut [Collidable], callback: usize, probe: usize, other: usize) {
        self.rest.on_hit(objects, callback, probe, other)
    }
    fn on_struck(&mut self, objects: &mut [Collidable], callback: usize, struck: usize, probe: usize) {
        self.rest.on_struck(objects, callback, struck, probe)
    }
}

/// How many contacts a sphere's record has room for (its normals, points, others, claimed and spare
/// words are three each).
pub const CONTACTS: usize = 3;

/// **The sphere's record** (`+0x7c`), as its pair writes it: room (`+0`), the count (`+4`), the
/// nearest hit's distance (`+8`), the sphere's centre at the hit (`+0x20`), and per contact its
/// normal (`+0x30`) and point (`+0x60`), the collidable struck (`+0x90`), whether it was already taken
/// back to the world (`+0x9c`), and a spare word the pair zeroes (`+0xa8`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Record {
    pub capacity: i32,
    pub count: i32,
    pub distance: F,
    pub centre: Vf,
    pub normals: [Vf; CONTACTS],
    pub points: [Vf; CONTACTS],
    pub others: [Option<usize>; CONTACTS],
    pub claimed: [u32; CONTACTS],
    pub words: [u32; CONTACTS],
}

/// A mesh as the pair walks it: its frame (`+0x130`), its tree, triangles and vertices (`+0x244`,
/// `+0x248`, `+0x230`), and each triangle's packed normal (the triangle's `+0xc`).
#[derive(Clone, Copy)]
pub struct Placed<'a> {
    pub frame: Matrix,
    pub mesh: MeshRef<'a>,
    pub normals: &'a [[i16; 3]],
}

/// What the other collidable is to the pair, by its kind (`+0x78`).
#[derive(Clone, Copy)]
pub enum Shape<'a> {
    /// Kind 0.
    Mesh(Placed<'a>),
    /// Kind 1: a box (`+0x130` centre, `+0x140` half extents) in a frame (`+0x1b0`) round its parts
    /// (`+0x224`, `+0x210` of them).
    Hierarchy { centre: [F; 3], half: [F; 3], frame: Matrix, parts: &'a [Placed<'a>] },
    /// Kind 5: a sphere's point, frame and radius.
    Sphere { local: Vf, frame: Matrix, radius: F },
    /// Kind 6: a box (`+0x170` centre, `+0x180` half extents) in a frame (`+0x130`).
    Box { centre: [F; 3], half: [F; 3], frame: Matrix },
    /// Every other kind: nothing met.
    Other,
}

/// The other collidable: whether it may be met (`+0x45` bit 0), what it is, and what the record
/// names it by.
#[derive(Clone, Copy)]
pub struct Target<'a> {
    pub nearby: bool,
    pub shape: Shape<'a>,
    pub object: usize,
}

/// **The pair** (vt `+0xb4`): the sphere against `other`, writing what a mesh's triangle took in
/// `record`. Not nearby: no. A box: the sphere's point in its own frame against the box, the box's
/// frame taken into the sphere's ([`in_frame_of`]). Another sphere: [`spheres_meet`]. A hierarchy:
/// its box round the sphere's centre, then each part's mesh until one is touched. A mesh: its tree
/// ([`touch`]). A mesh or part touched: the record's spare words zeroed, its centre the sphere's,
/// and each contact not yet claimed taken through the mesh's frame and given `other`. Also answers
/// the last mesh walked (a hierarchy's part by index, a mesh as 0): the original leaves its vertex
/// descriptor in a global.
pub fn pair(s: &Sphere, record: &mut Record, other: &Target) -> (bool, Option<usize>) {
    if !other.nearby {
        return (false, None);
    }
    // The FPU reads the radius and its square as operands (exponent 255 the largest, a denormal 0).
    let s = &Sphere { radius: operand(s.radius), radius2: operand(s.radius2), ..*s };
    let me = atv_part::Sphere { centre: s.centre, radius: s.radius, radius2: s.radius2 };
    let hit = match other.shape {
        Shape::Box { centre, half, frame } => {
            let local = atv_part::Sphere { centre: s.local, ..me };
            return (node_reaches_sphere(centre, half, &local, &in_frame_of(&s.frame, &frame)), None);
        }
        Shape::Sphere { local, frame, radius } => return (spheres_meet(s.radius, radius, s.local, &s.frame, local, &frame), None),
        Shape::Mesh(m) => (touch(&m, s, record).then_some(m.frame), Some(0)),
        Shape::Hierarchy { centre, half, frame, parts } => {
            if !node_reaches_sphere(centre, half, &me, &frame) {
                return (false, None);
            }
            let mut walked = None;
            let mut hit = None;
            for (i, m) in parts.iter().enumerate() {
                walked = Some(i);
                if touch(m, s, record) {
                    hit = Some(m.frame);
                    break;
                }
            }
            (hit, walked)
        }
        Shape::Other => return (false, None),
    };
    let (hit, walked) = hit;
    let Some(frame) = hit else { return (false, walked) };
    record.words = [0; CONTACTS];
    record.centre = s.centre;
    let node = node(&frame);
    let mut i = 0;
    while i < record.count as usize {
        if record.claimed[i] == 0 {
            record.normals[i] = node.rotate(record.normals[i]);
            record.points[i] = node.place(record.points[i]);
            record.others[i] = Some(other.object);
            record.claimed[i] = 1;
        }
        i += 1;
    }
    (true, walked)
}

fn node(m: &Matrix) -> Node {
    Node { rows: [m[0], m[1], m[2]], origin: m[3] }
}

/// `other`'s frame in the sphere's: the sphere's frame inverted as a rotation (the `3×3` transposed
/// lane by lane, each moved lane added to `0`; `w` the rows' own) and its position `0 − Rᵀ·m3`
/// (`vmsuba`, `w` kept), then each of `other`'s rows through it (the position onto `inv3 + 0`), all
/// four lanes.
pub fn in_frame_of(sphere: &Matrix, other: &Matrix) -> Matrix {
    let [m0, m1, m2, m3] = *sphere;
    let z = |v: f32| vu::add(0.0, v);
    let t0 = Vf([m0.x(), z(m1.x()), z(m2.x()), m0.w()]);
    let t1 = Vf([z(m0.y()), m1.y(), z(m2.y()), m1.w()]);
    let t2 = Vf([z(m0.z()), z(m1.z()), m2.z(), m2.w()]);
    let acc = Vf::msub(Vf::ZERO.add(Vf::ZERO), t0, Vf::splat(m3.x()));
    let acc = Vf::msub(acc, t1, Vf::splat(m3.y()));
    let t3 = m3.merge(Dest::XYZ, Vf::msub(acc, t2, Vf::splat(m3.z())));
    let row = |acc: Vf, o: Vf| {
        let acc = Vf::madd(acc, t0, Vf::splat(o.x()));
        let acc = Vf::madd(acc, t1, Vf::splat(o.y()));
        Vf::madd(acc, t2, Vf::splat(o.z()))
    };
    let first = |o: Vf| {
        let acc = t0.mul(Vf::splat(o.x()));
        let acc = Vf::madd(acc, t1, Vf::splat(o.y()));
        Vf::madd(acc, t2, Vf::splat(o.z()))
    };
    [first(other[0]), first(other[1]), first(other[2]), row(t3.add(Vf::ZERO), other[3])]
}

/// **Two spheres**: each point through its frame (`xyz`, `w` kept), and whether the squared distance
/// between them (`b − a`, the FPU's `mula`/`madda`/`madd`) is no more than the radii's sum squared.
pub fn spheres_meet(ra: F, rb: F, a: Vf, fa: &Matrix, b: Vf, fb: &Matrix) -> bool {
    let reach = fpu::add(ra.0, rb.0);
    let (a, b) = (node(fa).place(a), node(fb).place(b));
    let d = minus(b, a);
    let d2 = Acc::mula(d.x(), d.x()).madda(d.y(), d.y()).madd(d.z(), d.z());
    d2.0 <= fpu::mul(reach, reach)
}

/// **The sphere against a mesh**: its tree walked from word 0 with the sphere's centre, radius and
/// squared radius ([`descend`]).
fn touch(m: &Placed, s: &Sphere, record: &mut Record) -> bool {
    descend(m, &node(&m.frame), 0, s, record)
}

/// The tree walk from `word`: a node whose box reaches the sphere (in the mesh's frame,
/// [`node_reaches_sphere`]) goes on to its first child, and to its second only if the first said no.
/// A triangle: the centre into the frame, its distance above the triangle's plane `0..=r`, and the
/// foot on the plane inside the triangle or an edge nearer than `r` ([`edge`]); then, with room in
/// the record, taken as its one contact when the record is empty or the hit is nearer than the one
/// it holds (no otherwise), and yes with no room.
fn descend(m: &Placed, frame: &Node, word: u16, s: &Sphere, record: &mut Record) -> bool {
    let (r, r2) = (s.radius, s.radius2);
    match child(word) {
        Child::Node(i) => {
            let n = &m.mesh.nodes[usize::from(i)];
            let me = atv_part::Sphere { centre: s.centre, radius: r, radius2: r2 };
            if !node_reaches_sphere(n.centre, n.half_extent, &me, &m.frame) {
                return false;
            }
            descend(m, frame, n.children[0], s, record) || descend(m, frame, n.children[1], s, record)
        }
        Child::Leaf(i) => {
            let i = usize::from(i);
            let c = frame.unplace(s.centre);
            let n = unpack(m.normals[i]);
            let [a, b, d] = m.mesh.triangles[i].vertices.map(|v| m.mesh.vertex(v));
            let dist = dot(n, minus(c, a));
            if dist.0 < 0.0 || !(dist.0 <= r.0) {
                return false;
            }
            let p = minus(c, n.merge(Dest::XYZ, n.mul(Vf::splat(dist.0))));
            let hit = inside(p, n, [a, b, d])
                || fpu::abs(edge(c, a, b)) < r.0
                || fpu::abs(edge(c, b, d)) < r.0
                || fpu::abs(edge(c, d, a)) < r.0;
            if !hit {
                return false;
            }
            if record.count < record.capacity {
                if record.count != 0 && !(dist.0 < operand(record.distance).0) {
                    return false;
                }
                record.normals[0] = unit(n);
                record.points[0] = p;
                record.distance = dist;
                record.claimed[0] = 0;
                record.count = 1;
            }
            true
        }
    }
}

fn operand(x: F) -> F {
    F(fpu::operand(x.0.to_bits()))
}

fn child(word: u16) -> Child {
    if word & 0x8000 == 0 { Child::Node(word) } else { Child::Leaf(word & 0x7fff) }
}

/// **The distance from `p` to the edge from `b` to `a`**, through the FPU: `p` behind `b`, its
/// distance from `b`; past `a`, the root of `|e|² + (|w|² − 2 w·e)` (`adda` of zero, `msub`); else of
/// `t² |e|²` onto `|w|² − 2t (w·e)` (`t = w·e / |e|²`; `msub`, then `adda` of zero and `madd`). MX
/// Unleashed's test is the same but for these two sums.
fn edge(p: Vf, a: Vf, b: Vf) -> f32 {
    let e = minus(a, b);
    let w = minus(p, b);
    let along = dot(w, e).0;
    let ww = dot(w, w).0;
    if along <= 0.0 {
        return fpu::sqrt(ww);
    }
    let ee = dot(e, e).0;
    if !(along < ee) {
        return fpu::sqrt(fpu::add(ee, fpu::msub(fpu::adda(0.0, ww), 2.0, along)));
    }
    let t = fpu::div(along, ee);
    let k = fpu::msub(fpu::adda(0.0, ww), fpu::mul(2.0, t), along);
    fpu::sqrt(fpu::madd(fpu::adda(0.0, k), ee, fpu::mul(t, t)))
}
