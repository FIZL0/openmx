//! The nearby search: a probe tested against the candidates its broad phase gathered around it.
//!
//! A collision object's probes (the plane's ray probe and its sweep probe) are *collidables*, and so
//! is everything they can strike: a mesh, a hierarchy of meshes, another probe. Each frame the broad
//! phase gathers the collidables around the object into a list, each with its distance ([`Nearby`]),
//! and every search the object makes that frame walks that list ([`search_nearby`]):
//!
//! 1. the probe's result is emptied (its narrow phase's own clear) and, if the list asks for it, the
//!    probe is made ready (its narrow phase's own prepare);
//! 2. a candidate further than the probe's radius is passed over;
//! 3. another collidable is marked searched, must pass the pair filter ([`accepts`], with the "nearby"
//!    flag), and is then handed to the probe's narrow phase;
//! 4. a *placed group* — a model the level stands in many places, whose collision is its parts — is
//!    searched part by part, if the probe searches groups at all. A part skips the filter;
//! 5. each hit raises the probe's hit bit and tells, in this order, the probe's listeners, the struck
//!    one's listeners, the probe's own callback and the struck one's.
//!
//! The narrow phases, a group's parts and the listeners and callbacks are behaviour of the classes
//! involved, not of the search: they come in through [`Narrow`]. The search only decides which pairs
//! are tested and in what order, and who hears about a hit.

#![deny(clippy::float_arithmetic)]

use ps2_float::F;

/// `+0x116`, the collidable's flag byte, as the search reads and writes it.
pub mod flags {
    /// On a probe: it never searches placed groups.
    pub const NO_GROUPS: u8 = 0x01;
    /// It can be struck at all.
    pub const ENABLED: u8 = 0x04;
    /// Set on the probe and on every candidate within its reach, each time a search runs.
    pub const SEARCHED: u8 = 0x10;
    /// On a probe: the last search struck something. Only the probe's own clear lowers it.
    pub const HIT: u8 = 0x20;
}

/// `+2`: what a collidable is to the search.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Class {
    /// Another collidable, tested as itself (2).
    #[default]
    Object,
    /// A placed group, tested through its parts (3).
    Group,
    /// Anything else, which the search passes over.
    Other(u8),
}

impl Class {
    pub fn from_byte(b: u8) -> Class {
        match b {
            2 => Class::Object,
            3 => Class::Group,
            b => Class::Other(b),
        }
    }
}

/// One of a collidable's two lists of other collidables (`+0xd0`/`+0xd4`/`+0xd8` and
/// `+0xdc`/`+0xe0`/`+0xe4`): with `mode` 1 the ones it never collides with, with `mode` 0 the only ones
/// it collides with. An empty list, or any other mode, says nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Objects {
    pub mode: u32,
    pub list: Vec<usize>,
}

impl Objects {
    fn allows(&self, other: usize) -> bool {
        if self.list.is_empty() {
            return true;
        }
        let listed = self.list.contains(&other);
        !(listed && self.mode == 1 || !listed && self.mode == 0)
    }
}

/// A collidable's list of categories (`+0xe8`/`+0xec`/`+0xf0`): with `mode` 1 the categories it never
/// collides with; with any other mode the ones it collides with — and then an empty list collides with
/// nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Categories {
    pub mode: u32,
    pub list: Vec<u32>,
}

impl Categories {
    fn allows(&self, category: u32) -> bool {
        let any = self.list.iter().any(|&m| category & m != 0);
        if self.mode == 1 { !any } else { any }
    }
}

/// A collidable, as the nearby search and the pair filter see it. Listeners and callbacks are handles
/// the [`Narrow`] behind the search knows the meaning of.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Collidable {
    /// `+2`.
    pub class: Class,
    /// `+0x116`: see [`flags`].
    pub flags: u8,
    /// `+0x31` bit 0: it may be struck by a nearby search.
    pub nearby: bool,
    /// `+0x60`: how far it reaches. For a probe, a candidate further than this is not tested.
    pub radius: F,
    /// `+0x78`: the group a part was last searched as part of.
    pub group: Option<usize>,
    /// `+0x84`: its categories, as a mask; `+0xcc` the categories it collides with.
    pub category: u32,
    pub collides_with: u32,
    /// `+0xd0`.., `+0xdc`..: the two lists of other collidables; `+0xe8`..: the list of categories.
    pub objects: Objects,
    pub more_objects: Objects,
    pub categories: Categories,
    /// `+0xf4`: its listeners, in order.
    pub listeners: Vec<usize>,
    /// `+0x10c`: what it calls when, as a probe, it strikes something; `+0x110` what it calls when it is
    /// struck.
    pub on_hit: Option<usize>,
    pub on_struck: Option<usize>,
}

/// The pair filter: whether `objects[me]` may collide with `objects[other]`. `nearby` is what the
/// nearby search asks (the struck one must allow it); the broad phase asks without.
///
/// Both ways round, in this order: not itself; the other enabled; the other's nearby flag; the other's
/// category in this one's mask (one way only); this one's object lists about the other, then the
/// other's about this one, each list in turn; this one's category list about the other's category,
/// then the other's about this one's.
pub fn accepts(objects: &[Collidable], me: usize, other: usize, nearby: bool) -> bool {
    if me == other {
        return false;
    }
    let (a, b) = (&objects[me], &objects[other]);
    if b.flags & flags::ENABLED == 0 || nearby && !b.nearby || a.collides_with & b.category == 0 {
        return false;
    }
    a.objects.allows(other)
        && b.objects.allows(me)
        && a.more_objects.allows(other)
        && b.more_objects.allows(me)
        && a.categories.allows(b.category)
        && b.categories.allows(a.category)
}

/// One gathered candidate: a collidable and its distance from the search's centre, less its reach.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Candidate {
    pub object: usize,
    pub distance: F,
}

/// The nearby search an object keeps (`+0x10c` of a collision object): what the broad phase gathered
/// (`+0`, `+4`), and whether a probe searching it must be made ready first (`+0x38`).
///
/// `+0x38` is really the level's terrain cell grid, and the "preparation" a pass of the probe over it.
/// A level only has one when its scene names a grid file, and no scene on the disc does, so it is
/// always false in this game (tests/sweep_probe_reach.rs) and the pass is not ported.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Nearby {
    pub found: Vec<Candidate>,
    pub prepare: bool,
}

/// What the search hands to the classes involved: the probe's narrow phase, a group's parts, and the
/// listeners and callbacks a hit is told to. Called in the original's order, which a listener can see.
pub trait Narrow {
    /// The probe's own clear: empty its result (and lower its hit bit, which only this does).
    fn clear(&mut self, objects: &mut [Collidable], probe: usize);
    /// The probe's own preparation, when the search asks for it.
    fn prepare(&mut self, objects: &mut [Collidable], probe: usize);
    /// The probe's narrow phase against one collidable: whether it struck it (its result filled in).
    fn pair(&mut self, objects: &mut [Collidable], probe: usize, other: usize) -> bool;
    /// How many parts the group has. Asked again after every part.
    fn parts(&mut self, objects: &mut [Collidable], group: usize) -> usize;
    /// The group's part `i`, made ready to be tested, if it has one there.
    fn part(&mut self, objects: &mut [Collidable], group: usize, i: usize) -> Option<usize>;
    /// One of the probe's listeners hears that it struck `other`.
    fn heard_hit(&mut self, objects: &mut [Collidable], listener: usize, probe: usize, other: usize);
    /// One of the struck one's listeners hears that `probe` struck it.
    fn heard_struck(&mut self, objects: &mut [Collidable], listener: usize, struck: usize, probe: usize);
    /// The probe's callback, then the struck one's.
    fn on_hit(&mut self, objects: &mut [Collidable], callback: usize, probe: usize, other: usize);
    fn on_struck(&mut self, objects: &mut [Collidable], callback: usize, struck: usize, probe: usize);
}

/// The probe tested against `other`; on a hit, the bit raised and everyone told.
pub(crate) fn test(objects: &mut [Collidable], probe: usize, other: usize, narrow: &mut dyn Narrow) {
    if !narrow.pair(objects, probe, other) {
        return;
    }
    objects[probe].flags |= flags::HIT;
    // Each list is re-read on every step: a listener may change it.
    let mut i = 0;
    while let Some(&l) = objects[probe].listeners.get(i) {
        narrow.heard_hit(objects, l, probe, other);
        i += 1;
    }
    let mut i = 0;
    while let Some(&l) = objects[other].listeners.get(i) {
        narrow.heard_struck(objects, l, other, probe);
        i += 1;
    }
    if let Some(c) = objects[probe].on_hit {
        narrow.on_hit(objects, c, probe, other);
    }
    if let Some(c) = objects[other].on_struck {
        narrow.on_struck(objects, c, other, probe);
    }
}

/// The nearby search: `objects[probe]` against everything `search` gathered. Returns the probe's hit
/// bit.
pub fn search_nearby(objects: &mut [Collidable], probe: usize, search: &Nearby, narrow: &mut dyn Narrow) -> bool {
    objects[probe].flags |= flags::SEARCHED;
    narrow.clear(objects, probe);
    if search.prepare {
        narrow.prepare(objects, probe);
    }
    for c in &search.found {
        if !(c.distance <= objects[probe].radius) {
            continue;
        }
        let o = c.object;
        match objects[o].class {
            Class::Object => {
                objects[o].flags |= flags::SEARCHED;
                if accepts(objects, probe, o, true) {
                    test(objects, probe, o, narrow);
                }
            }
            Class::Group if objects[probe].flags & flags::NO_GROUPS == 0 => {
                let mut i = 0;
                while i < narrow.parts(objects, o) {
                    if let Some(p) = narrow.part(objects, o, i) {
                        objects[p].group = Some(o);
                        objects[p].flags |= flags::SEARCHED;
                        test(objects, probe, p, narrow);
                    }
                    i += 1;
                }
            }
            _ => {}
        }
    }
    objects[probe].flags & flags::HIT != 0
}
