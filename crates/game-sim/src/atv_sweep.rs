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
//! The pair test (vt `+0xb4`, a contact-list sweep through a mesh or a hierarchy) is **not** here:
//! [`Sweep`] hands it, and a group's parts, to the caller. On C15's rides it never runs: nothing of
//! nat05's world but its plants comes within the probe's reach, and the probe never searches placed
//! groups (`+0x126` bit 0), so no search hits.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::collide::{Bundle, MAX_PROBES};
use crate::sweep_probe::{self, Segment, SweepProbe};
use crate::world_search::{Collidable, Narrow, flags};
use ps2_float::{F, Vf, fpu};

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

/// The sweep probe's narrow phase for [`crate::atv_search::search`]: its own clear, and everything else
/// (the pair test, a group's parts, the listeners and callbacks) handed to `rest`.
pub struct Sweep<'a> {
    pub probe: SweepProbe,
    pub rest: &'a mut dyn Narrow,
}

impl Sweep<'_> {
    /// The probe on `segments`, at most [`MAX_PROBES`] (the original has no cap; MX Unleashed's clear
    /// stops at its sixteenth).
    pub fn new(segments: Vec<Segment>, rest: &mut dyn Narrow) -> Sweep<'_> {
        assert!(segments.len() <= MAX_PROBES, "a sweep probe of {} segments", segments.len());
        Sweep { probe: SweepProbe { segments, ..SweepProbe::default() }, rest }
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
