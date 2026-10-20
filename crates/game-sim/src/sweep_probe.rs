//! The sweep probe's narrow phase: a bundle of short segments, swept through a mesh or through each
//! mesh of a hierarchy, with the answers brought back into the world.
//!
//! A collision object that sweeps (the biplane's, to find what it flew into since the last frame) has
//! a *sweep probe*: a collidable whose shape is up to [`MAX_PROBES`] segments, each between two points
//! that may ride on a transform node. The nearby search ([`crate::world_search`]) asks it two things:
//!
//! 1. [`clear`], once per search: stand the segments where their nodes are now, derive each one's
//!    direction and length, and wrap them all in one sphere — the reach the search tests candidates
//!    against, and the one the pair test rejects on. The result's nearest hit is reset here, but not
//!    the per-segment answers: [`pair`] resets those.
//! 2. [`pair`], once per candidate: reject on the two spheres, then take the segments into the
//!    candidate's own space (through the inverse of its transform), sweep them with
//!    [`collide::sweep`], and take what was hit back out (through the transform itself).
//!
//! Three things in it a reader will want to "fix", and must not:
//!
//! * **Each candidate starts over.** The pair test resets the nearest fraction and every segment's
//!   fraction before it sweeps, but not the nearest hit's normal, point and push-out. So after a search
//!   the result is the *last* candidate that latched a nearest hit, not the nearest over all of them;
//!   and a candidate that hits nothing nearer than a whole segment leaves the previous one's normal
//!   and point in place, untransformed again.
//! * **A hierarchy's nearest is taken out through every part that hit.** Within a hierarchy each part
//!   is swept into the same result, and whenever a part hits *anything*, the result's nearest normal,
//!   point and push-out are transformed out through that part's transform — whether or not that part's
//!   hit is the nearest one. A segment's own answer is taken out only once (its owner word marks it).
//! * **The lengths are not rescaled.** The segments go into the mesh's space through the inverse
//!   transform built as a transpose, which is only an inverse for a rotation; a scaled mesh would sweep
//!   segments of the wrong length. No shipped transform carries a scale.
//!
//! What is not here: the probe's third slot, the pass against a cell grid the nearby search can hand
//! it, and the finite-plane case of the pair test — see the module's journal entry.

#![deny(clippy::float_arithmetic)]

use crate::collide::{self, Bundle, MAX_PROBES, MeshRef, Probe, Record};
use ps2_float::{F, Vf, fpu, vu};

/// A transform as the collision code stores it: three rows, then the translation. A point goes
/// through it as `t + r0·x + r1·y + r2·z`.
pub type Transform = [Vf; 4];

/// One of the probe's segments as the collision object sets it (`+0x124` the two node pointers,
/// `+0x128` the two points): from `ends[0]` to `ends[1]`, each through its node's world transform
/// (the node's `+0x80`..`+0xb0`) if it has one, or as it stands if not.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Segment {
    pub ends: [Vf; 2],
    pub nodes: [Option<Transform>; 2],
}

/// What the sweep probe answers in (`+0x6c`): [`collide::Record`], where the sweep writes, plus what
/// the pair test adds — which collidable each segment struck — and the two quads the clear resets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Answer {
    /// `+0` and `+0x10`: reset to `(0, 0, 0, 1)` by [`clear`]; nothing in the sweep reads or writes them.
    pub head: [Vf; 2],
    /// `+0x20`..`+0xb0`, as [`collide::sweep`] fills it. [`Record::owner`] is the pair test's "already
    /// taken out of a hierarchy part's space" mark.
    pub record: Record,
    /// `+0x9c`: per segment, the collidable its answer belongs to. Written for every segment that hit,
    /// never cleared.
    pub struck: [Option<usize>; MAX_PROBES],
}

impl Default for Answer {
    fn default() -> Answer {
        Answer { head: [Vf::ZERO; 2], record: Record::new(), struck: [None; MAX_PROBES] }
    }
}

/// The sweep probe: its segments, their two forms, and the answer.
#[derive(Clone, Debug, PartialEq)]
pub struct SweepProbe {
    /// `+0x120`: the segments, at most [`MAX_PROBES`].
    pub segments: Vec<Segment>,
    /// The segments in the world, as [`clear`] derives them: `+0x138` the starts, `+0x13c` the unit
    /// directions, `+0x134` the lengths, `+0x150` the centre of their box and `+0x160` the radius
    /// around it. The centre and the radius are also the collidable's `+0x40` and `+0x60`.
    pub world: Bundle,
    /// The same segments in the space of the mesh [`pair`] last swept: `+0x12c` the starts, `+0x130`
    /// the directions, `+0x140` the centre. The lengths and the radius are the world's (one field each
    /// in the original).
    pub local: Bundle,
    /// `+0x6c`.
    pub answer: Answer,
}

impl Default for SweepProbe {
    fn default() -> SweepProbe {
        SweepProbe { segments: Vec::new(), world: Bundle::default(), local: Bundle::default(), answer: Answer::default() }
    }
}

/// `(0, 0, 0, 1)`, which the clear writes wherever it means "nothing".
const ORIGIN: Vf = Vf([0.0, 0.0, 0.0, 1.0]);

/// The sweep probe's clear, once per search (class slot `+0x90`): the segments stood in the world,
/// their sphere, and the answer's nearest hit reset. The caller lowers the collidable's hit bit
/// ([`crate::world_search::flags::HIT`]) and copies [`SweepProbe::world`]'s centre and radius into the
/// collidable's centre and reach, as the original's clear does.
pub fn clear(probe: &mut SweepProbe) {
    let mut lo = Vf([fpu::MAX, fpu::MAX, fpu::MAX, 1.0]);
    let mut hi = Vf([fpu::NEG_MAX, fpu::NEG_MAX, fpu::NEG_MAX, 1.0]);
    let count = probe.segments.len().min(MAX_PROBES);
    probe.world.count = count;
    for i in 0..count {
        let s = probe.segments[i];
        let end = |k: usize| match s.nodes[k] {
            Some(m) => to_world(&m, s.ends[k]),
            None => s.ends[k],
        };
        let (a, b) = (end(0), end(1));
        let d = xyz(b.sub(a), b);
        let length = F(fpu::sqrt(fpu::dot([d.x(), d.y(), d.z()], [d.x(), d.y(), d.z()])));
        let dir = if !(length <= F::ZERO) { xyz(d.mul_q(vu::div(1.0, length.0)), d) } else { ORIGIN };
        probe.world.probes[i] = Probe { origin: a, dir, length };
        lo = lo.mini(b).mini(a);
        hi = hi.max(b).max(a);
    }
    let half = |v: Vf| xyz(v.mul_q(0.5), v);
    let centre = half(xyz(hi.add(lo), hi));
    let extent = half(xyz(hi.sub(lo), hi));
    probe.world.centre = centre;
    probe.world.radius = F(fpu::sqrt(fpu::dot([extent.x(), extent.y(), extent.z()], [extent.x(), extent.y(), extent.z()])));

    let answer = &mut probe.answer;
    answer.head = [ORIGIN; 2];
    answer.record.nearest = F::ONE;
    answer.record.hits = 0;
}

/// A mesh the probe can be swept through, where it stands: its transform (`+0x120`..`+0x150`), its
/// tree, triangles and vertices (`+0x234`, `+0x238`, `+0x220`), and the collidable it is.
#[derive(Clone, Copy, Debug)]
pub struct Placed<'a> {
    pub transform: Transform,
    pub mesh: MeshRef<'a>,
    pub object: usize,
}

/// What a candidate is to the pair test (`+0x68`).
#[derive(Clone, Copy, Debug)]
pub enum Shape<'a> {
    /// A mesh (0).
    Mesh(Placed<'a>),
    /// A hierarchy (1): its parts (`+0x200` and `+0x214`), each a mesh where it stands.
    Hierarchy(&'a [Placed<'a>]),
    /// Anything this test never strikes: another probe (2 the ray probe, 3 a sweep probe), or a kind
    /// it has no case for. Not the finite plane (8), which it does strike and which is not ported.
    Other,
}

/// A candidate as the pair test reads it.
#[derive(Clone, Copy, Debug)]
pub struct Target<'a> {
    /// `+0x31` bit 0: it may be struck by a nearby search at all.
    pub nearby: bool,
    /// `+0x40` and `+0x60`: its sphere.
    pub centre: Vf,
    pub radius: F,
    pub shape: Shape<'a>,
}

/// The sweep probe against one candidate (class slot `+0x94`): whether any segment struck it, with the
/// answer filled in. `None` is a null candidate, which the original refuses like an unreachable one.
///
/// The probe must have been [`clear`]ed this search: the test reads the world form the clear derived.
pub fn pair(probe: &mut SweepProbe, other: Option<&Target>) -> bool {
    let Some(other) = other else { return false };
    if !other.nearby {
        return false;
    }
    let count = probe.world.count.min(MAX_PROBES);
    let rec = &mut probe.answer.record;
    for i in 0..count {
        rec.hit[i] = false;
        rec.owner[i] = 0;
    }

    // The two spheres, on the FPU: centre to centre squared against the summed radii squared.
    let d = probe.world.centre.sub(other.centre);
    let reach = other.radius + probe.world.radius;
    if !(F(fpu::dot([d.x(), d.y(), d.z()], [d.x(), d.y(), d.z()])) < reach * reach) {
        return false;
    }
    rec.nearest = F::ONE;
    for i in 0..count {
        rec.fraction[i] = F(-1.0);
    }
    rec.hits = 0;

    match other.shape {
        Shape::Other => false,
        Shape::Mesh(placed) => {
            sweep_into(probe, &placed);
            let rec = &mut probe.answer.record;
            let mut struck = false;
            for i in 0..count {
                if rec.hit[i] {
                    struck = true;
                    rec.point[i] = to_world(&placed.transform, rec.point[i]);
                    rec.normal[i] = turn(&placed.transform, rec.normal[i]);
                    probe.answer.struck[i] = Some(placed.object);
                }
            }
            if struck {
                take_out_nearest(rec, &placed.transform);
            }
            struck
        }
        Shape::Hierarchy(parts) => {
            let mut struck = false;
            for placed in parts {
                sweep_into(probe, placed);
                let rec = &mut probe.answer.record;
                let mut this = false;
                for i in 0..count {
                    if rec.hit[i] && rec.owner[i] == 0 {
                        struck = true;
                        this = true;
                        rec.point[i] = to_world(&placed.transform, rec.point[i]);
                        rec.normal[i] = turn(&placed.transform, rec.normal[i]);
                        probe.answer.struck[i] = Some(placed.object);
                        rec.owner[i] = 1;
                    }
                }
                if this {
                    take_out_nearest(rec, &placed.transform);
                }
            }
            struck
        }
    }
}

/// The segments into the mesh's space, and the sweep. The local form keeps what the original's arrays
/// keep: the lengths and the radius are the world's own.
fn sweep_into(probe: &mut SweepProbe, placed: &Placed) {
    let inverse = invert(&placed.transform);
    let count = probe.world.count.min(MAX_PROBES);
    probe.local.count = count;
    for i in 0..count {
        let w = probe.world.probes[i];
        let l = &mut probe.local.probes[i];
        l.dir = turn(&inverse, w.dir);
        l.origin = to_world(&inverse, w.origin);
        l.length = w.length;
    }
    probe.local.centre = to_world(&inverse, probe.world.centre);
    probe.local.radius = probe.world.radius;
    collide::sweep(&probe.local, &placed.mesh, &mut probe.answer.record);
}

/// The answer's nearest normal, point and push-out, out of a mesh's space.
fn take_out_nearest(rec: &mut Record, m: &Transform) {
    rec.nearest_point = to_world(m, rec.nearest_point);
    rec.nearest_normal = turn(m, rec.nearest_normal);
    rec.push = turn(m, rec.push);
}

// --- the transforms, in the original's VU0 instruction order ------------------------------------------

/// A point through a transform: `vaddax` of the translation and zero into the accumulator, then
/// `vmaddax`/`vmadday` and `vmaddz.xyz` over the rows. `w` is the point's own.
fn to_world(m: &Transform, p: Vf) -> Vf {
    let acc = Vf::madd(m[3].add(Vf::ZERO), m[0], Vf([p.x(); 4]));
    let acc = Vf::madd(acc, m[1], Vf([p.y(); 4]));
    xyz(Vf::madd(acc, m[2], Vf([p.z(); 4])), p)
}

/// A direction through a transform's rows only: `vmulax`, `vmadday`, `vmaddz.xyz`. `w` is its own.
fn turn(m: &Transform, v: Vf) -> Vf {
    let acc = m[0].mul(Vf([v.x(); 4]));
    let acc = Vf::madd(acc, m[1], Vf([v.y(); 4]));
    xyz(Vf::madd(acc, m[2], Vf([v.z(); 4])), v)
}

/// The inverse the pair test builds: the rows transposed lane by lane (each moved lane added to
/// `vf0`'s zero, a VU add), and the translation `−(r0·t.x + r1·t.y + r2·t.z)` from an accumulator of
/// zero, `vmsubax`/`vmsubay`/`vmsubz.xyz`. Each row's `w` stays the original row's.
fn invert(m: &Transform) -> Transform {
    let moved = |v: f32| vu::add(0.0, v);
    let [r0, r1, r2, t] = *m;
    let c0 = Vf([r0.x(), moved(r1.x()), moved(r2.x()), r0.w()]);
    let c1 = Vf([moved(r0.y()), r1.y(), moved(r2.y()), r1.w()]);
    let c2 = Vf([moved(r0.z()), moved(r1.z()), r2.z(), r2.w()]);
    let acc = Vf::msub(Vf::ZERO.add(Vf::ZERO), c0, Vf([t.x(); 4]));
    let acc = Vf::msub(acc, c1, Vf([t.y(); 4]));
    let t = xyz(Vf::msub(acc, c2, Vf([t.z(); 4])), t);
    [c0, c1, c2, t]
}

/// Three lanes written, `w` from `w_from` — every `.xyz` instruction here stores a register loaded
/// from its first source.
fn xyz(v: Vf, w_from: Vf) -> Vf {
    Vf([v.x(), v.y(), v.z(), w_from.w()])
}
