//! **The bike's sweep probe** (F46c): SLUS-21104's sweep probe, what the bike's
//! collision record sweeps back with ([`crate::handling::record`]'s `Collides::sweep`): the segment from
//! where the body was to where it is set on the probe, the probe placed, and searched over the
//! record's nearby list.
//!
//! The probe is MX Unleashed's sweep probe ([`crate::sweep_probe`]) with its fields moved up `0x10`
//! and no cap on the segment count: segments `+0x130` (count), `+0x134` (two node pointers each),
//! `+0x138` (two points each); the world form `+0x144` lengths, `+0x148` starts, `+0x14c` directions,
//! `+0x160` the centre and `+0x170` the radius; `+0x50` and `+0x70` the collidable's centre and reach.
//!
//! The three calls:
//!
//! 1. [`set`]: the segments' points and nodes taken, and the box of their ends ([`bounds`]) put in the
//!    centre and both radii.
//! 2. The place (`atv_part_sync`): the probe's vt `+0xc8` does nothing; then, with an entry in the
//!    world's spatial database (`+0x20`), its vt `+0xc4` gives [`bounds`] again (the centre and reach
//!    written too) and the entry is moved to it ([`crate::atv_spatial::spatial_move`]).
//! 3. The search, `collide_against_search` ([`crate::atv_search::search`]) with the sweep's own clear
//!    ([`Sweep`]): the segments stood in the world as MX Unleashed's clear stands them
//!    ([`crate::sweep_probe::clear`]), the hit bit lowered, and the record (`+0x7c`) reset — its first two
//!    quads to `(0, 0, 0, 1)`, its nearest (`+0xb4`) to 1, its count (`+0x8c`) to 0.
//!
//! 4. The pair (vt `+0xb4`, [`pair`]): MX Unleashed's sweep probe pair ([`crate::sweep_probe::pair`]) on
//!    the record's own arrays, with two more writes — a third per-segment word array (`+0xac`) zeroed
//!    with the hit and owner words, and the hierarchy part that last took a segment kept (`+0x84`).
//!    [`Sweep`] runs it on what its caller's [`Outside`] says each collidable is.
//!
//! On C15's rides the pair never runs: nothing of nat05's world but its plants comes within the
//! probe's reach, and the probe never searches placed groups (`+0x126` bit 0), so no search hits. It
//! is proved against the original's own pair over generated probes and nat05's meshes instead.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::collide::{Bundle, MAX_PROBES};
use crate::sweep_probe::{self, Segment, Shape, SweepProbe, Target};
use crate::broad_phase::{Broad, Query, more_flags, pair_distance};
use crate::world_search::{Class, Collidable, Narrow, accepts, flags, test};
use ps2_float::{Acc, F, Vf, fpu, vu};

/// The box round the segments' ends, as the set and the box slot (vt `+0xc4`) build it: the lanes'
/// least and greatest over each segment's second end then its first (`vmini`/`vmax` on all four
/// lanes, from `(±max, ±max, ±max, 1)`), the centre half their sum and the half extent half their
/// difference (`xyz`; `w` the greatest's), and the reach the half extent's length (an FPU chain).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub centre: Vf,
    pub half: Vf,
    pub reach: F,
}

/// [`Bounds`] of `segments`, each end through its node if it has one.
pub fn bounds(segments: &[Segment]) -> Bounds {
    let mut lo = Vf([fpu::MAX, fpu::MAX, fpu::MAX, 1.0]);
    let mut hi = Vf([fpu::NEG_MAX, fpu::NEG_MAX, fpu::NEG_MAX, 1.0]);
    for s in segments {
        let [a, b] = ends(s);
        lo = lo.mini(b).mini(a);
        hi = hi.max(b).max(a);
    }
    let half = |v: Vf| xyz(v.mul_q(0.5), v);
    let centre = half(xyz(hi.add(lo), hi));
    let extent = half(xyz(hi.sub(lo), hi));
    let reach = F(fpu::sqrt(fpu::dot([extent.x(), extent.y(), extent.z()], [extent.x(), extent.y(), extent.z()])));
    Bounds { centre, half: extent, reach }
}

/// A segment's two ends in the world: through the node's frame (`t + 0` then `r0·x`, `r1·y`, `r2·z`
/// accumulated, `xyz`; `w` the point's) or as it stands.
pub fn ends(s: &Segment) -> [Vf; 2] {
    std::array::from_fn(|k| match s.nodes[k] {
        Some(m) => {
            let p = s.ends[k];
            let acc = Vf::madd(m[3].add(Vf::ZERO), m[0], Vf([p.x(); 4]));
            let acc = Vf::madd(acc, m[1], Vf([p.y(); 4]));
            xyz(Vf::madd(acc, m[2], Vf([p.z(); 4])), p)
        }
        None => s.ends[k],
    })
}

fn xyz(v: Vf, w_from: Vf) -> Vf {
    Vf([v.x(), v.y(), v.z(), w_from.w()])
}

/// **The set** (the probe, its points, and its nodes or none): the
/// segments' points and nodes as given (a node left off is none), and their [`bounds`]: its centre
/// goes in `+0x50`, its reach in `+0x170` and `+0x70`.
pub fn set(points: &[[Vf; 2]], nodes: Option<&[[Option<sweep_probe::Transform>; 2]]>) -> (Vec<Segment>, Bounds) {
    let segments: Vec<Segment> =
        points.iter().enumerate().map(|(i, &ends)| Segment { ends, nodes: nodes.map_or([None; 2], |n| n[i]) }).collect();
    let b = bounds(&segments);
    (segments, b)
}

/// What the ATV's record holds past [`sweep_probe::Answer`]: the third per-segment word array the
/// pair zeroes (`+0xac`; nothing in the sweep writes it otherwise) and the hierarchy part that last
/// took a segment (`+0x84`; a mesh does not write it).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct More {
    pub words: [u32; MAX_PROBES],
    pub part: Option<usize>,
}

/// **The pair** (vt `+0xb4`): the probe against one collidable, `None` a null one. MX Unleashed's
/// pair ([`sweep_probe::pair`]) with the record's arrays where the ATV keeps them: a collidable that
/// is not nearby is refused; then each segment's hit, owner and [`More::words`] are zeroed, the
/// two spheres tested (the FPU's `mula`/`madda`/`madd` against the summed radii squared), the
/// nearest reset to 1, the fractions to -1 and the tally to 0; a mesh or each part of a hierarchy
/// swept in its own space, and what was struck taken back out. A hierarchy part that takes a
/// segment (its owner word still 0) is kept in [`More::part`].
///
/// Kinds 2 and 3 (the other probes) and every kind but 0, 1 and 8 strike nothing
/// ([`Shape::Other`]); the finite plane (8) is not ported (the caller must not hand one).
pub fn pair(probe: &mut SweepProbe, more: &mut More, other: Option<&Target>) -> bool {
    let Some(other) = other else { return false };
    if !other.nearby {
        return false;
    }
    let count = probe.segments.len().min(MAX_PROBES);
    let rec = &mut probe.answer.record;
    for i in 0..count {
        rec.hit[i] = false;
        rec.owner[i] = 0;
        more.words[i] = 0;
    }
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
    let parts: &[sweep_probe::Placed] = match &other.shape {
        Shape::Other => return false,
        Shape::Mesh(placed) => core::slice::from_ref(placed),
        Shape::Hierarchy(parts) => parts,
    };
    let hierarchy = matches!(other.shape, Shape::Hierarchy(_));
    let mut struck = false;
    for placed in parts {
        sweep_probe::sweep_into(probe, placed);
        let rec = &mut probe.answer.record;
        let mut this = false;
        for i in 0..count {
            if rec.hit[i] && !(hierarchy && rec.owner[i] != 0) {
                rec.point[i] = sweep_probe::to_world(&placed.transform, rec.point[i]);
                rec.normal[i] = sweep_probe::turn(&placed.transform, rec.normal[i]);
                probe.answer.struck[i] = Some(placed.object);
                if hierarchy {
                    more.part = Some(placed.object);
                    rec.owner[i] = 1;
                }
                this = true;
                struck = true;
            }
        }
        if this {
            sweep_probe::take_out_nearest(rec, &placed.transform);
        }
    }
    struck
}

/// The world the sweep searches: what each collidable is to the pair, and as a [`Narrow`] a group's
/// parts, the listeners and the callbacks.
pub trait Outside: Narrow {
    /// `other` as a [`Target`] (`None` a null one) handed to `pair`, and what it answers.
    fn target(&mut self, other: usize, pair: &mut dyn FnMut(Option<&Target>) -> bool) -> bool;
}

/// The sweep probe's narrow phase for [`crate::atv_search::search`] and [`search_world`]: its own
/// clear and pair, and everything else (a group's parts, the listeners and callbacks) handed to
/// `rest`.
pub struct Sweep<'a> {
    pub probe: SweepProbe,
    pub more: More,
    pub rest: &'a mut dyn Outside,
}

impl Sweep<'_> {
    /// The probe on `segments`, at most [`MAX_PROBES`] (the original has no cap; MX Unleashed's clear
    /// stops at its sixteenth), with the record's [`More`] as it stands.
    pub fn new(segments: Vec<Segment>, more: More, rest: &mut dyn Outside) -> Sweep<'_> {
        assert!(segments.len() <= MAX_PROBES, "a sweep probe of {} segments", segments.len());
        Sweep { probe: SweepProbe { segments, ..SweepProbe::default() }, more, rest }
    }

    /// The world form the last clear stood the segments in.
    pub fn world(&self) -> &Bundle {
        &self.probe.world
    }
}

impl Narrow for Sweep<'_> {
    /// The clear (vt `+0xb0`): hit bit down, the segments stood in the world, and the collidable's
    /// reach the bundle's radius. The record's reset is the caller's to write.
    fn clear(&mut self, objects: &mut [Collidable], probe: usize) {
        objects[probe].flags &= !flags::HIT;
        sweep_probe::clear(&mut self.probe);
        objects[probe].radius = self.probe.world.radius;
    }
    fn prepare(&mut self, objects: &mut [Collidable], probe: usize) {
        self.rest.prepare(objects, probe)
    }
    fn pair(&mut self, _objects: &mut [Collidable], _probe: usize, other: usize) -> bool {
        let (probe, more) = (&mut self.probe, &mut self.more);
        self.rest.target(other, &mut |t| pair(probe, more, t))
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

/// **The global search** (the record's sweep with nothing nearby, called lazy 0): the probe against
/// every collidable its chain of spaces holds round its box. Answers how far the nearest one was.
///
/// MX Unleashed's (`broad_phase::probe_collide_world`) with the sweep's own slots and these
/// differences:
///
/// - the probe searches only if it is active (`+0x126` bit 1) *and* has an entry in the database;
///   otherwise the hit bit is lowered and the answer is `f32::MAX`;
/// - a collidable whose first word is 1 (`retired`) is passed over in the first walk before it is
///   marked searched (the walk of the moving tables does not ask);
/// - a placed group's distance to the probe is taken on the ground plane with `z` into the
///   accumulator first;
/// - the probe's box (vt `+0xc4`) is [`bounds`] of its segments, and puts its centre and reach in the
///   collidable (`+0x50`, `+0x70`) as it answers.
///
/// The budget and last centre are the probe's place's (`+0x74`, `+0x60`).
pub fn search_world(objects: &mut [Collidable], broad: &mut Broad, probe: usize, lazy: u32, retired: &dyn Fn(usize) -> bool, sweep: &mut Sweep) -> F {
    let space = broad.places[probe].space;
    let Some(space) = space.filter(|_| objects[probe].flags & more_flags::ACTIVE != 0) else {
        objects[probe].flags &= !flags::HIT;
        return F(fpu::MAX);
    };
    let mut lazy = lazy;
    {
        let p = &mut broad.places[probe];
        let d: [F; 3] = std::array::from_fn(|i| F(vu::sub(p.centre[i].0, p.last[i].0)));
        p.budget = p.budget - Acc::mula(d[0], d[0]).madda(d[1], d[1]).madd(d[2], d[2]).sqrt();
        p.last = p.centre;
        if lazy != 0 && !(p.budget <= 0.0) {
            return p.budget;
        }
        p.budget = F::ZERO;
    }
    sweep.clear(objects, probe);
    objects[probe].flags |= flags::SEARCHED;
    let b = bounds(&sweep.probe.segments);
    broad.places[probe].centre = b.centre.0.map(F);
    objects[probe].radius = b.reach;
    let (c, h) = (b.centre.0.map(F), b.half.0.map(F));
    let (mut lo, mut hi) = ([F::ZERO; 4], [F::ZERO; 4]);
    let mut q = Query::begin(broad, space, 1, c, h, Some((&mut lo, &mut hi)), true);
    let mut nearest = ((c[0] - h[0]) - lo[0]).min((c[2] - h[2]) - lo[2]).min(hi[0] - (c[0] + h[0])).min(hi[2] - (c[2] + h[2]));
    while let Some(o) = q.next(broad, Some((&mut lo, &mut hi))) {
        match objects[o].class {
            Class::Object => {
                if retired(o) {
                    continue;
                }
                objects[o].flags |= flags::SEARCHED;
                let d = pair_distance(objects, broad, probe, o);
                if d == 0.0 {
                    nearest = F::ZERO;
                    if accepts(objects, probe, o, true) {
                        test(objects, probe, o, sweep);
                    }
                } else if d < nearest {
                    nearest = d;
                    if lazy != 0 && d < broad.places[probe].budget {
                        lazy <<= 1;
                    }
                }
            }
            Class::Group if objects[probe].flags & flags::NO_GROUPS == 0 => {
                let d = group_distance(objects, broad, probe, o);
                if d == 0.0 {
                    let mut i = 0;
                    while i < sweep.parts(objects, o) {
                        if let Some(p) = sweep.part(objects, o, i) {
                            objects[p].group = Some(o);
                            nearest = F::ZERO;
                            objects[p].flags |= flags::SEARCHED;
                            test(objects, probe, p, sweep);
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
                test(objects, probe, o, sweep);
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

/// How far the probe's reach is from placed group `g`'s on the ground plane: the group's point its
/// cell in world units, `z` then `x` into the accumulator, less the probe's reach and the group's
/// model's blended; never below 0.
fn group_distance(objects: &[Collidable], broad: &Broad, probe: usize, g: usize) -> F {
    let place = &broad.places[g];
    let m = broad.models[usize::from(place.model)];
    let cell = |c: u16| F::from(i32::from(c)) * broad.group_cell;
    let reach = m.near + F::from(i32::from(place.blend)) * (m.far - m.near) / 128.0;
    let c = broad.places[probe].centre;
    let (dz, dx) = (c[2] - cell(place.cell[1]), c[0] - cell(place.cell[0]));
    let s = Acc::mula(dz, dz).madd(dx, dx).sqrt();
    F::ZERO.max(s - (objects[probe].radius + reach))
}
