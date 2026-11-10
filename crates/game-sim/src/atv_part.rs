//! **The bike's part search** (F46d): SLUS-21104's ray probe searched as the bike's collision record
//! asks in its pass ([`crate::handling::record`]'s `Collides::search`): the active part (the riding or
//! the wreck rays) over the record's nearby list, its record read back as the pass's hits.
//!
//! The probe is MX Unleashed's ray probe ([`crate::ray_probe`]) with its fields moved up `0x10`, as the
//! quad's trigger probe is ([`crate::atv_search`]): `+0x130` its frame, `+0x170` its tree's root,
//! `+0x178` the ray count, `+0x17c` the tree's nodes, `+0x180` the rays (two quads each), `+0x184` the
//! smooth flag, `+0x188` a per-ray filter, `+0x7c` its record. The search is
//! [`crate::atv_search::search`] with [`PartRays`]:
//!
//! - **the clear** (vt `+0xb0`): MX Unleashed's ([`ray_probe::clear`]): the hit bit lowered, the
//!   record's nearest segment, push, low and high to `(0, 0, 0, 1)`, its nearest and its pair's first
//!   to 1, no contacts. Besides, two globals zeroed and the record made the current one (the caller's
//!   to write, [`PartRays`] has no memory), and with a filter its reset (the filter is not here);
//! - **the pair** (vt `+0xb4`): the smooth flag made the global the segment test reads (the caller's
//!   to write), the per-contact words at the record's `+0xac` zeroed, the pair's first set to 1, then
//!   by the other's kind (`+0x78`): 0 a mesh and 1 a placed hierarchy ([`ray_probe::pair`]), 5 a
//!   sphere ([`sphere_reaches`]: the hit bit alone, no contact), anything else nothing struck (kind 6
//!   has a test of its own, not here: the bike never meets one).
//!
//! The riding part searches the placed groups (it does not carry `NO_GROUPS`): a plant's parts are
//! its spheres, stood at the plant by the caller ([`Narrow::part`]). What hears a hit (the probe's
//! listeners and the struck's, and the two callbacks) is the caller's too ([`Outside`]).

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::ray_probe::{self, Matrix, RayProbe, Record, Shape};
use crate::world_search::{Collidable, Narrow};
use ps2_float::vu::{Dest, Field};
use ps2_float::{F, Vf, fpu, vu};

/// The world the part searches: what the probe is paired with, by collidable (a mesh, a hierarchy,
/// or [`Shape::Never`]), and as a [`Narrow`] a group's parts, the listeners and the callbacks.
pub trait Outside: Narrow {
    fn shape(&mut self, other: usize) -> Shape<'_>;
    /// The other as a sphere (kind 5), which the pair tests itself: `None` for the rest.
    fn sphere(&mut self, _other: usize) -> Option<Sphere> {
        None
    }
}

/// A sphere collidable as the probe's pair reads it: its centre, radius and the radius squared (both
/// stored; the test reads each where it needs it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    pub centre: Vf,
    pub radius: F,
    pub radius2: F,
}

/// The probe's tree against a sphere: a node whose box (in the probe's frame) reaches the sphere goes
/// on to its first child, and to its second only if the first missed; a segment (taken to the world)
/// reaches it by [`segment_reaches_sphere`]. Struck or not is all it says: no contact is written.
pub fn sphere_reaches(probe: &RayProbe, sphere: &Sphere) -> bool {
    reach(probe, sphere, probe.root)
}

fn reach(probe: &RayProbe, s: &Sphere, word: u16) -> bool {
    if word & 0x8000 != 0 {
        let seg = probe.segments[(word & 0x7fff) as usize];
        let (from, to) = (place(&probe.transform, seg.from), place(&probe.transform, seg.to));
        return segment_reaches_sphere(from, to, s.centre, s.radius2);
    }
    let n = &probe.nodes[word as usize];
    if !node_reaches_sphere(n.centre, n.half_extent, s, &probe.transform) {
        return false;
    }
    reach(probe, s, n.children[0]) || reach(probe, s, n.children[1])
}

/// A node's box (centre `c`, half extents `h`, in `m`'s frame) against the sphere: the centre taken
/// into the frame (`m` transposed), its distance from the box per axis; out if any axis is more than
/// the radius past the box, else in if the corner distance squared is under the radius squared.
pub fn node_reaches_sphere(c: [F; 3], h: [F; 3], s: &Sphere, m: &Matrix) -> bool {
    let v: [f32; 3] = core::array::from_fn(|i| vu::sub(s.centre.0[i], m[3].0[i]));
    let local: [f32; 3] = core::array::from_fn(|i| {
        let acc = vu::mul(m[i].x(), v[0]);
        let acc = vu::madd(acc, m[i].y(), v[1]);
        vu::madd(acc, m[i].z(), v[2])
    });
    let d: [f32; 3] = core::array::from_fn(|i| f32::from_bits(vu::sub(local[i], c[i].0).to_bits() & 0x7fff_ffff));
    for i in 0..3 {
        let e = vu::sub(vu::sub(d[i], h[i].0), s.radius.0);
        if !(F(e) <= F::ZERO) {
            return false;
        }
    }
    let q: [f32; 3] = core::array::from_fn(|i| vu::sub(d[i], vu::mini(d[i], h[i].0)));
    let dist2 = fpu::madd(fpu::madda(fpu::mula(q[0], q[0]), q[1], q[1]), q[2], q[2]);
    !(s.radius2 <= F(dist2))
}

/// A world segment against the sphere: in if either end is inside, else if the centre projects onto
/// the segment (ahead of `from`, not past `to`) within the radius of the line.
pub fn segment_reaches_sphere(from: Vf, to: Vf, centre: Vf, radius2: F) -> bool {
    let d0: [f32; 3] = core::array::from_fn(|i| vu::sub(centre.0[i], from.0[i]));
    let d0_2 = fpu::madd(fpu::madda(fpu::mula(d0[0], d0[0]), d0[1], d0[1]), d0[2], d0[2]);
    let d1: [f32; 3] = core::array::from_fn(|i| vu::sub(centre.0[i], to.0[i]));
    let d1_2 = fpu::madd(fpu::madda(fpu::mula(d1[0], d1[0]), d1[1], d1[1]), d1[2], d1[2]);
    if F(d0_2) <= radius2 || F(d1_2) <= radius2 {
        return true;
    }
    let seg: [f32; 3] = core::array::from_fn(|i| vu::sub(to.0[i], from.0[i]));
    let sq: [f32; 3] = seg.map(|x| vu::mul(x, x));
    let len2 = vu::madd(vu::add(sq[0], sq[1]), 1.0, sq[2]);
    let q = vu::rsqrt(1.0, len2);
    let u: [f32; 3] = seg.map(|x| vu::mul(x, q));
    let off = fpu::sub(d0_2, radius2.0);
    let t = fpu::madd(fpu::madda(fpu::mula(d0[0], u[0]), d0[1], u[1]), d0[2], u[2]);
    let t2 = fpu::mul(t, t);
    if F(t) < F::ZERO || !(F(off) <= F(t2)) {
        return false;
    }
    let seg2 = fpu::madd(fpu::madda(fpu::mula(seg[0], seg[0]), seg[1], seg[1]), seg[2], seg[2]);
    !(F(seg2) <= F(t2))
}

/// `v` through `m`, `xyz` written and `w` kept: `vaddax` of the position, then `vmaddax` / `vmadday`
/// / `vmaddz`.
fn place(m: &Matrix, v: Vf) -> Vf {
    let lane = |i: usize| {
        let acc = vu::add(m[3].0[i], 0.0);
        let acc = vu::madd(acc, m[0].0[i], v.x());
        let acc = vu::madd(acc, m[1].0[i], v.y());
        vu::madd(acc, m[2].0[i], v.z())
    };
    Vf([lane(0), lane(1), lane(2), v.w()])
}

/// The part probe's narrow phase for [`crate::atv_search::search`]: its own clear and pair, and
/// everything else handed to `world`.
pub struct PartRays<'a, 'p> {
    pub probe: RayProbe<'p>,
    pub record: Record,
    pub world: &'a mut dyn Outside,
    /// How many pairs ran: each one rewrote the smooth-flag global and zeroed the per-contact words.
    pub pairs: u32,
}

impl Narrow for PartRays<'_, '_> {
    fn clear(&mut self, objects: &mut [Collidable], probe: usize) {
        ray_probe::clear(&mut objects[probe].flags, &mut self.record);
    }
    /// Never asked: [`crate::atv_search::search`] prepares nothing.
    fn prepare(&mut self, objects: &mut [Collidable], probe: usize) {
        self.world.prepare(objects, probe)
    }
    fn pair(&mut self, _objects: &mut [Collidable], _probe: usize, other: usize) -> bool {
        self.pairs += 1;
        if let Some(sphere) = self.world.sphere(other) {
            self.record.first = F::ONE;
            return sphere_reaches(&self.probe, &sphere);
        }
        let shape = self.world.shape(other);
        ray_probe::pair(&self.probe, &shape, &mut self.record)
    }
    fn parts(&mut self, objects: &mut [Collidable], group: usize) -> usize {
        self.world.parts(objects, group)
    }
    fn part(&mut self, objects: &mut [Collidable], group: usize, i: usize) -> Option<usize> {
        self.world.part(objects, group, i)
    }
    fn heard_hit(&mut self, objects: &mut [Collidable], listener: usize, probe: usize, other: usize) {
        self.world.heard_hit(objects, listener, probe, other)
    }
    fn heard_struck(&mut self, objects: &mut [Collidable], listener: usize, struck: usize, probe: usize) {
        self.world.heard_struck(objects, listener, struck, probe)
    }
    fn on_hit(&mut self, objects: &mut [Collidable], callback: usize, probe: usize, other: usize) {
        self.world.on_hit(objects, callback, probe, other)
    }
    fn on_struck(&mut self, objects: &mut [Collidable], callback: usize, struck: usize, probe: usize) {
        self.world.on_struck(objects, callback, struck, probe)
    }
}

/// The part probe's box (vt `+0xc4`), as the world probe ([`crate::atv_search::collide_world`]) asks
/// for it: its tree's first node's box, or without a tree its first ray's (the midpoint, and half the
/// span, each `vadd`/`vsub` then `× 0.5`), stood through the frame. The centre is the translation
/// then the three rows (`vadda`, `vmadda` x, y, `vmadd` z); the half extents are the three rows each
/// scaled by its half, made positive, and summed (`vadda` the second and first, `vmadd` the third by
/// 1). Both `w` 1.
pub fn bounds(probe: &RayProbe) -> ([F; 4], [F; 4]) {
    let half = Vf::splat(0.5);
    let (c, h) = match probe.nodes.first() {
        Some(n) => (
            Vf::new(n.centre[0].0, n.centre[1].0, n.centre[2].0, 1.0),
            Vf::new(n.half_extent[0].0, n.half_extent[1].0, n.half_extent[2].0, 1.0),
        ),
        None => {
            let s = probe.segments[0];
            (s.to.add(s.from).mul(half).merge(Dest::W, Vf::VF0), s.to.sub(s.from).mul(half).merge(Dest::W, Vf::VF0))
        }
    };
    let t = &probe.transform;
    let acc = Vf::madd(t[3].add(Vf::ZERO), t[0], c.bc(Field::X));
    let acc = Vf::madd(acc, t[1], c.bc(Field::Y));
    let centre = Vf::madd(acc, t[2], c.bc(Field::Z)).merge(Dest::W, Vf::VF0);
    let [r0, r1, r2] = [0, 1, 2].map(|i| t[i].mul(h.bc([Field::X, Field::Y, Field::Z][i])).abs());
    let extent = Vf::madd(r1.add(r0), r2, Vf::splat(1.0)).merge(Dest::W, Vf::VF0);
    (centre.0.map(F), extent.0.map(F))
}

impl crate::broad_phase::Probe for PartRays<'_, '_> {
    fn bounds(&mut self, _objects: &mut [Collidable], _probe: usize) -> ([F; 4], [F; 4]) {
        bounds(&self.probe)
    }
}
