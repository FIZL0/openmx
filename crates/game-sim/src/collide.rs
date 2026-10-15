//! Collision against the world: a bundle of probe segments swept through a collision mesh's
//! bounding-volume tree.
//!
//! Everything in the game that has to stay out of the scenery — a wheel, a rider's shoulder, a bale —
//! asks the same question: *along this short segment, where is the first triangle, and which way does
//! it face?* The original answers it for up to [`MAX_PROBES`] segments at once, because the expensive
//! part is walking the tree and the segments of one wheel all point the same way. One such group is a
//! [`Bundle`]; the answers land in a [`Record`].
//!
//! The sweep is deliberately the original's and not a better one. Three of its choices change the
//! game if you "fix" them:
//!
//! * **One-sided triangles.** A triangle is only solid from the side its winding faces: the test needs
//!   `normal · direction < 0` and takes a back-facing hit as no hit at all. Every piece of track
//!   geometry is modelled with that in mind, and making triangles solid from both sides turns the
//!   underside of every jump into a ceiling.
//! * **One probe decides the descent.** At a tree node the sweep looks for the *first* probe of the
//!   bundle that still reaches the node's box, and descends on its behalf with the probes before it
//!   switched off for the rest of that subtree (see [`Bundle::count`]). It is not a per-probe
//!   traversal, and the saving is what makes it affordable.
//! * **A hit resolves to blocked travel, not to a penetration depth.** What comes back is the part of
//!   the segment the probe did *not* get to travel, projected onto the contact normal — see
//!   [`Record::push`]. It points the way the probe was going, it is measured in fractions of one unit
//!   rather than of the probe's own length, and it is not the vector a depth-based resolver would hand
//!   back.
//!
//! What is *not* here: the broad phase that decides which meshes are close enough to be worth
//! sweeping, the dispatch that picks a shape class, and the ray-tree-against-mesh-tree variant the
//! bike's own wheels use (two trees descended against each other, with a different leaf test). This
//! module is the shape test and the sweep over one mesh; the module docs of the callers say when each
//! one runs.

#![deny(clippy::float_arithmetic)]
// Comparisons are written the way the original branches: `!(a < b)` where its instruction is a
// `c.lt.s` whose false arm continues. The EE has no NaN to be incomparable — an operand that would be
// one is read as a saturated maximum — so the negation is exactly `a >= b`, and keeping the original's
// sense is what lets this be read against the listing.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use crate::track::{Child, Node, Points, Triangle};
use ps2_float::{F, Vf, fpu, vu};

/// How many probes one bundle may carry.
///
/// The original's limit, not a chosen one: the sweep keeps its "which probes are still live" mask in a
/// fixed sixteen-word block of the scratchpad per level of the tree, and the loops that fill it are
/// bounded by nothing but the bundle's own count. A seventeenth probe would be tested against a mask
/// word belonging to the next level down.
pub const MAX_PROBES: usize = 16;

/// How deep the descent may go.
///
/// Also the original's, and also a consequence of where the mask lives: each level takes the next
/// sixteen words of the scratchpad, which is 16 KiB, so the 257th level would write outside it. No
/// shipped tree comes close — a 256-level binary tree would need more nodes than the disc holds — so
/// the limit is here as the original's bound, and the port stops descending instead of writing where
/// the original would have.
pub const MAX_LEVELS: usize = 256;

/// One probe: where it starts, which way it points (unit length), and how far it goes.
///
/// The direction and the length are separate because the sweep needs both separately, and because the
/// caller derives them once per frame and then tests them against many meshes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Probe {
    pub origin: Vf,
    /// Unit direction. The sweep divides by its components, and a zero component is handled, but a
    /// direction that is not unit length silently rescales every fraction the sweep reports.
    pub dir: Vf,
    pub length: F,
}

impl Probe {
    /// A probe from one point to another, derived the way the original derives one: the length is the
    /// distance, and the direction is the difference divided by it.
    ///
    /// A zero-length segment comes back with the length zero and whatever the VU's divide makes of a
    /// division by zero (its largest magnitude, not an infinity); the sweep rejects a probe whose
    /// length is not positive before it ever looks at the direction.
    pub fn between(from: Vf, to: Vf) -> Probe {
        let delta = to.sub(from);
        let length = F(vec3_length(delta));
        Probe { origin: from, dir: div_scalar(delta, length), length }
    }
}

/// A group of probes tested together, with the sphere that encloses all of them.
///
/// The enclosing sphere is the first thing a tree node is tested against, so a bundle whose probes are
/// nowhere near a node costs one comparison per axis rather than one descent per probe. The caller
/// that builds the bundle out of a vehicle's attachment points is what fills [`Bundle::centre`] and
/// [`Bundle::radius`] in; a bundle with a radius too small quietly stops colliding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bundle {
    /// How many of [`Bundle::probes`] are real. Must not exceed [`MAX_PROBES`].
    pub count: usize,
    pub probes: [Probe; MAX_PROBES],
    /// The centre of the bundle's own bounding volume.
    pub centre: Vf,
    /// Its radius, added to each tree node's half extents when the whole bundle is tested against one.
    pub radius: F,
}

impl Default for Bundle {
    fn default() -> Bundle {
        Bundle { count: 0, probes: [Probe::default(); MAX_PROBES], centre: Vf::ZERO, radius: F::ZERO }
    }
}

/// Where the answers go: one set of fields per probe, and one set for the nearest hit of the bundle.
///
/// The original keeps all of this in the collision record the caller hands to the sweep, as separate
/// arrays it holds pointers to — the sweep is handed a record that the caller has already cleared, and
/// several meshes are swept into the same one before anybody reads it. [`Record::new`] is that cleared
/// state: no hits, every fraction negative (the sweep's "nothing yet" marker), and the nearest
/// fraction at 1, which is the whole length of a probe and therefore further than any hit can be.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Record {
    /// Per probe: how far along it the nearest hit so far is, as a fraction of its length. Negative
    /// means no hit.
    pub fraction: [F; MAX_PROBES],
    /// Per probe: where that hit is, in the mesh's space.
    pub point: [Vf; MAX_PROBES],
    /// Per probe: the triangle's outward normal there.
    pub normal: [Vf; MAX_PROBES],
    /// Per probe: whether anything was hit at all. Set for every hit, not only for a nearer one.
    pub hit: [bool; MAX_PROBES],
    /// Per probe: the caller's slot for *what* was hit. The sweep clears the probe's slot when that
    /// probe becomes the bundle's nearest hit, which is how a caller that sweeps several meshes into
    /// one record can tell which mesh owns the answer.
    pub owner: [i32; MAX_PROBES],
    /// How many (probe, triangle) tests hit — a tally, not a count of probes: a probe that hits four
    /// triangles adds four.
    pub hits: i32,
    /// The nearest fraction over the whole bundle, and the probe it belongs to.
    pub nearest: F,
    pub nearest_probe: Option<usize>,
    /// The nearest hit's normal and point, copied from that probe's entry.
    pub nearest_normal: Vf,
    pub nearest_point: Vf,
    /// How the sweep resolves the nearest hit: the travel the probe did *not* get to make,
    /// `direction · (1 − fraction)`, projected onto the contact normal and laid back along it.
    ///
    /// Two things about it that a reader expecting a push-out will get wrong. It points *into* the
    /// surface, along the way the probe was travelling, because that is the travel the triangle took
    /// away — a caller removes it from the motion rather than adding it. And the direction is unit
    /// length, so it is a fraction of one unit of travel and not of the probe's length: a short probe
    /// and a long one stopped at the same fraction resolve to the same vector. That is the original's
    /// arithmetic, and every caller is tuned around it.
    pub push: Vf,
}

impl Default for Record {
    fn default() -> Record {
        Record::new()
    }
}

impl Record {
    /// A cleared record, as a caller hands one to the sweep.
    pub fn new() -> Record {
        Record {
            fraction: [F(-1.0); MAX_PROBES],
            point: [Vf::ZERO; MAX_PROBES],
            normal: [Vf::ZERO; MAX_PROBES],
            hit: [false; MAX_PROBES],
            owner: [0; MAX_PROBES],
            hits: 0,
            nearest: F::ONE,
            nearest_probe: None,
            nearest_normal: Vf::ZERO,
            nearest_point: Vf::ZERO,
            push: Vf::ZERO,
        }
    }
}

/// The parts of a parsed collision mesh the sweep reads: the tree, the triangles, and the vertices the
/// triangles index.
///
/// Borrowed rather than owned because one mesh is swept against many bundles, and because the parsed
/// form ([`crate::track::Mesh`]) is shared with everything else that reads a track.
#[derive(Clone, Copy, Debug)]
pub struct MeshRef<'a> {
    pub nodes: &'a [Node],
    pub triangles: &'a [Triangle],
    pub vertices: &'a Points,
}

impl<'a> MeshRef<'a> {
    pub fn of(mesh: &'a crate::track::Mesh) -> MeshRef<'a> {
        MeshRef { nodes: &mesh.nodes, triangles: &mesh.triangles, vertices: &mesh.vertices }
    }

    /// One vertex, in whichever of the two widths the mesh was stored in.
    ///
    /// The packed form is three shorts, and the scale the original applies to them is a pair of
    /// constants that all but cancel: the short is multiplied by one 32767th and then by 32767, in that
    /// order, so the result is the short's own value give or take the two roundings. The order is
    /// kept because the roundings are real — a packed mesh and a wide mesh of "the same" vertex are
    /// not always the same float.
    pub fn vertex(&self, index: u16) -> Vf {
        match self.vertices {
            Points::Wide(v) => {
                let v = v[index as usize];
                Vf([v[0].0, v[1].0, v[2].0, 1.0])
            }
            Points::Packed(v) => {
                let v = v[index as usize];
                let one = |s: i16| (PACKED_RECIPROCAL * F(fpu::from_i32(s as i32)) * PACKED_SCALE).0;
                Vf([one(v[0]), one(v[1]), one(v[2]), 1.0])
            }
        }
    }

    /// A triangle's three vertices.
    pub fn corners(&self, tri: &Triangle) -> [Vf; 3] {
        tri.vertices.map(|i| self.vertex(i))
    }
}

/// The two constants a packed mesh's shorts go through, in the order the original multiplies them:
/// a rounded 1/32767 first, then 32767.
const PACKED_RECIPROCAL: F = F(3.051_851e-5);
const PACKED_SCALE: F = F(32767.0);

/// Sweep a bundle of probes through a mesh, accumulating into `record`.
///
/// `record` may already hold the results of earlier meshes; a hit only replaces what is in there if it
/// is nearer. The return value says whether the sweep reached the tree at all, which is what the
/// original returns and is not the same as "something was hit" — read [`Record::hits`] for that.
pub fn sweep(bundle: &Bundle, mesh: &MeshRef, record: &mut Record) -> bool {
    if mesh.nodes.is_empty() || bundle.count == 0 {
        return false;
    }
    // The top call starts with every probe live; each level hands its children a copy.
    let mut live = [true; MAX_PROBES];
    descend(Child::Node(0), bundle, mesh, record, &mut live, 0);
    true
}

/// One level of the descent: either a node's box against the bundle, or a leaf triangle against every
/// probe still live.
fn descend(what: Child, bundle: &Bundle, mesh: &MeshRef, record: &mut Record, live: &mut [bool; MAX_PROBES], level: usize) {
    if level >= MAX_LEVELS {
        return;
    }
    match what {
        Child::Leaf(index) => {
            if let Some(tri) = mesh.triangles.get(index as usize) {
                leaf(tri, bundle, mesh, record, live);
            }
        }
        Child::Node(index) => {
            let Some(node) = mesh.nodes.get(index as usize) else { return };
            let centre = Vf([node.centre[0].0, node.centre[1].0, node.centre[2].0, 0.0]);
            let half = node.half_extent;

            // The whole bundle first: its sphere against the node's box, grown by the radius. One
            // comparison per axis, and it throws away most of the tree.
            let d = centre.sub(bundle.centre);
            let reach = |extent: F, from: f32| F(fpu::abs(from)) <= extent + bundle.radius;
            if !(reach(half[0], d.x()) && reach(half[1], d.y()) && reach(half[2], d.z())) {
                return;
            }

            // Then probe by probe, until one of them reaches the box. That one's descent carries the
            // whole bundle down, minus the probes before it, which cannot reach anything inside this
            // subtree either and are switched off for it.
            for i in 0..bundle.count.min(MAX_PROBES) {
                if live[i] && probe_reaches(&bundle.probes[i], centre, half) {
                    let mut below = *live;
                    descend(node.child(0), bundle, mesh, record, &mut below, level + 1);
                    let mut below = *live;
                    descend(node.child(1), bundle, mesh, record, &mut below, level + 1);
                    return;
                }
                live[i] = false;
            }
        }
    }
}

/// Does this probe's segment reach the box at `centre` with half extents `half`?
///
/// Not a slab test: the original tries the three axes in the order x, z, y, and for each one solves for
/// the parameter at which the segment crosses the *near* face on that axis and checks the other two
/// axes there. A segment with an endpoint inside the box is caught by the first test, before any
/// division happens.
fn probe_reaches(probe: &Probe, centre: Vf, half: [F; 3]) -> bool {
    let (o, dir, length) = (probe.origin, probe.dir, probe.length);
    let d = centre.sub(o);
    let within = |extent: F, v: f32| !(extent < F(fpu::abs(v)));
    if within(half[0], d.x()) && within(half[1], d.y()) && within(half[2], d.z()) {
        return true;
    }

    // The face to aim at on this axis is the one the origin is on the outside of.
    let face = |extent: F, towards: f32, from: F| if F::ZERO < F(towards) { -from - extent } else { extent - from };
    // Where the segment crosses that face, or before its start if it never does.
    let crossing = |c: F, offset: F, slope: f32| {
        if F::ZERO == F(slope) { F(-1.0) } else { (c + offset) / F(slope) }
    };
    let on = |t: F, axis: usize| F(fpu::abs((F(o.0[axis]).madd(t, dir.0[axis]) - F(centre.0[axis])).0));
    let along = |t: F| !(t < F::ZERO) && !(length < t);

    let (cx, cy, cz) = (F(centre.x()), F(centre.y()), F(centre.z()));
    let t = crossing(cx, face(half[0], d.x(), F(o.x())), dir.x());
    if along(t) && !(half[1] < on(t, 1)) && !(half[2] < on(t, 2)) {
        return true;
    }
    let t = crossing(cz, face(half[2], d.z(), F(o.z())), dir.z());
    if along(t) && !(half[1] < on(t, 1)) && !(half[0] < on(t, 0)) {
        return true;
    }
    let t = crossing(cy, face(half[1], d.y(), F(o.y())), dir.y());
    along(t) && !(half[2] < on(t, 2)) && !(half[0] < on(t, 0))
}

/// One triangle against every live probe: the shape test, and the bookkeeping a hit does.
fn leaf(tri: &Triangle, bundle: &Bundle, mesh: &MeshRef, record: &mut Record, live: &[bool; MAX_PROBES]) {
    let [v0, v1, v2] = mesh.corners(tri);
    // The index is a probe's slot in the bundle, the live mask and the record alike; it is not a walk
    // over any one of them.
    #[allow(clippy::needless_range_loop)]
    for i in 0..bundle.count.min(MAX_PROBES) {
        if !live[i] {
            continue;
        }
        let probe = &bundle.probes[i];
        let Some((fraction, point, normal)) = test(probe, [v0, v1, v2]) else { continue };

        // Nearer than what this probe already had — or its first hit at all.
        if fraction < record.fraction[i] || record.fraction[i] < F::ZERO {
            record.fraction[i] = fraction;
            record.point[i] = point;
            record.normal[i] = normal;
        }
        record.hit[i] = true;
        record.hits += 1;

        // The bundle's own nearest is compared against what the probe holds *now*, which is not
        // necessarily this triangle: a farther hit on a probe that already had a nearer one can still
        // be the moment the bundle's nearest is latched.
        if record.fraction[i] < record.nearest {
            record.nearest = record.fraction[i];
            record.nearest_probe = Some(i);
            record.nearest_normal = record.normal[i];
            record.nearest_point = record.point[i];
            // The hit resolves to the travel left over, along the normal. `dir` is unit length and the
            // leftover is of one unit, not of the probe's length.
            let left = probe.dir.mul_q((F::ONE - record.fraction[i]).0);
            let n = record.normal[i];
            let depth = fpu::dot([left.x(), left.y(), left.z()], [n.x(), n.y(), n.z()]);
            record.push = xyz(n.mul_q(depth), n);
            record.owner[i] = 0;
        }
    }
}

/// The shape test itself: one segment against one triangle.
///
/// Returns the fraction of the probe's length at which it hits, the point, and the triangle's normal —
/// or nothing, which covers all four ways the original rejects a triangle:
///
/// 1. the segment runs along or away from the facing side (`normal · direction >= 0`): back faces are
///    not solid,
/// 2. the probe has no length to travel,
/// 3. the plane crossing is behind the segment's start or past its end,
/// 4. the crossing point is outside the triangle.
///
/// The containment test in (4) is worth reading twice, because it is not the barycentric one. The
/// triangle is flattened onto the two axes the normal is *weakest* on, and then the test asks where the
/// ray from `v0` through the point crosses the opposite edge `v1 → v2`: at a parameter `u` along that
/// edge, having travelled `s` times the distance to the point. Inside means `s >= 1` — the edge is at
/// or beyond the point — and `u` within the edge. Degenerate triangles fall out of it through the two
/// divisions, which is why a zero denominator rejects rather than clamping.
pub fn test(probe: &Probe, corners: [Vf; 3]) -> Option<(F, Vf, Vf)> {
    let [v0, v1, v2] = corners;
    let normal = normalize(cross(xyz(v1.sub(v0), v1), xyz(v2.sub(v0), v2)));
    let facing = F(fpu::dot([normal.x(), normal.y(), normal.z()], [probe.dir.x(), probe.dir.y(), probe.dir.z()]));
    if !(facing < F::ZERO) || !(F::ZERO < probe.length) {
        return None;
    }
    let to_plane = v0.sub(probe.origin);
    let t = F(fpu::dot([normal.x(), normal.y(), normal.z()], [to_plane.x(), to_plane.y(), to_plane.z()])) / facing;
    if t < F::ZERO || probe.length < t {
        return None;
    }
    let point = xyz(probe.dir.mul_q(t.0).add(probe.origin), probe.dir);

    // Which pair of axes to flatten onto: the two the normal is smallest on, compared squared.
    let sq = normal.mul(normal);
    let (a, b) = if F(sq.x()) < F(sq.y()) || F(sq.x()) < F(sq.z()) {
        if F(sq.z()) <= F(sq.y()) { (0, 2) } else { (0, 1) }
    } else {
        (2, 1)
    };
    let (pa, pb) = (F(point.0[a]) - F(v0.0[a]), F(point.0[b]) - F(v0.0[b]));
    let (ea, eb) = (F(v1.0[a]) - F(v0.0[a]), F(v1.0[b]) - F(v0.0[b]));
    let (fa, fb) = (F(v2.0[a]) - F(v1.0[a]), F(v2.0[b]) - F(v1.0[b]));

    let den = pb * fa - fb * pa;
    let mut u = F(-1.0);
    if den != F::ZERO {
        let s = (fa * eb - fb * ea) / den;
        if F::ONE <= s {
            u = if fb * fb < fa * fa { (pa * s - ea) / fa } else { (pb * s - eb) / fb };
        }
    }
    if u < F::ZERO || F::ONE < u {
        return None;
    }
    Some((t / probe.length, point, normal))
}

// --- the maths helpers, in the original's instruction order -----------------------------------------

/// `v0 × v1` as the VU0 pair that computes one: `vopmula` then `vopmsub`, over `xyz`.
fn cross(a: Vf, b: Vf) -> Vf {
    xyz(Vf(vu::cross(a.0, b.0)), a)
}

/// Three lanes written, `w` left as it was.
///
/// Every one of the game's vector helpers is a `.xyz` instruction on a register it loaded from its
/// *first* source and then stores whole, so the `w` lane of the answer is the `w` lane of that source —
/// it is not computed, and it is not zero. That is visible from outside: a contact point's `w` is its
/// probe's direction's, and a normal's is its triangle's second vertex's. Threading it through is not
/// cosmetic; the record is copied around whole, and anything reading four lanes sees it.
fn xyz(result: Vf, w_from: Vf) -> Vf {
    Vf([result.x(), result.y(), result.z(), w_from.w()])
}

/// Normalise `xyz`, the way the game's own helper does: square the three lanes, add them up as
/// `(x + y) + z` with the accumulator, take the reciprocal square root — the VU's, which is a divide by
/// a chopped root and not a correctly rounded operation — and scale. The `w` lane comes back untouched.
fn normalize(v: Vf) -> Vf {
    let sq = v.mul(v);
    let len2 = vu::madd(vu::add(sq.x(), sq.y()), 1.0, sq.z());
    let q = vu::rsqrt(1.0, len2);
    Vf([vu::mul(v.x(), q), vu::mul(v.y(), q), vu::mul(v.z(), q), v.w()])
}

/// `|v|` over `xyz`, on the FPU: the same `mula`/`madda`/`madd` chain as a dot product with itself, then
/// `sqrt.s`.
fn vec3_length(v: Vf) -> f32 {
    fpu::sqrt(fpu::dot([v.x(), v.y(), v.z()], [v.x(), v.y(), v.z()]))
}

/// `v / s` over `xyz` as the game's helper does it: one VU divide for the reciprocal, then `vmulq`.
/// The `w` lane comes back untouched.
fn div_scalar(v: Vf, s: F) -> Vf {
    let q = vu::div(1.0, s.0);
    Vf([vu::mul(v.x(), q), vu::mul(v.y(), q), vu::mul(v.z(), q), v.w()])
}
