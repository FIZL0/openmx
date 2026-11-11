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
//! The pair (vt `+0xb4`: a box, a sphere, a hierarchy's parts or a mesh, each writing the record's
//! contacts) is the caller's ([`Search::rest`]); it is not ported yet (S279a).

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::world_search::{Collidable, Narrow, flags};
use ps2_float::{F, Vf};

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
