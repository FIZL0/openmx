//! **The material table's collision sounds** (F27): which hits of a frame play, at what level.
//!
//! The track's material table (`Col\template.cmt`) puts a listener on the ray probe of every collision
//! object the world node files: the four bikes' probes from the start, a knockable prop's while it is
//! awake (its wake files it, its sleep takes it off). Each hit that probe makes on another collidable is
//! queued — the probe, the struck collidable — and once a frame the table's update plays the queue and
//! empties it ([`play`]):
//!
//! 1. each entry's speed is the length of its probe's owner's *impulse*, not its velocity (B27): a
//!    body's shock peak (`+0xb0`, the largest change of velocity the collision response gave it per
//!    axis since the frame's preforce cleared it), or with no body the chassis' impact accumulator
//!    (`+0x170`, which the ground step empties at the top of every frame); taken before anything is
//!    dropped. Both are the frame's own, so a strike sounds on the frame of its shove and a contact
//!    that goes on without one (a ball the bike leans on the next tick) is under `MinImpulse`;
//! 2. a pair queued both ways round — A's probe struck B, and B's probe struck A — keeps the faster of
//!    the two; the slower is swapped out of the queue for its last entry;
//! 3. the sound is the struck collidable's material; a struck bike ([`BIKE`]) gives the striker's own
//!    material instead (a bike on a bike: the bike's, `bikehit1`), and a struck plant
//!    (its category in the foliage mask) the table's `SolidFoliageID`;
//! 4. a material the table has plays when the speed is at least its `CollisionMinImpulse`, at level
//!    `(int)(speed / CollisionMaxImpulse × 100)` clamped to `0..=100`, placed where the owner is,
//!    heard 3D between `CollisionMinDistance` and `CollisionMaxDistance`.
//!
//! The sound object is started with the flag that does not stop it first, so a hit while the last
//! one still sounds plays over it. What keeps a knock to one voice is the impulse of rule 1, not the
//! sound object: free09's knock (f27a1's RAM images) starts its `bbhit` on the strike frame, off the
//! ball's shock peak (16.81, level 67), and the two queued frames after it read 0 for the ball and
//! the bike alike, and play nothing.
//!
//! Sound only: nothing in the simulation reads a hit's sound, so no state hash or snapshot holds it.

#![deny(clippy::float_arithmetic)]

use ps2_float::{F, Vf};

/// A bike's collidable's material: struck, it plays the striker's own.
pub const BIKE: u8 = 16;

/// One queued hit: a probe's strike on a collidable, as the table's listener heard it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// The probe's owner's own collidable (its collision record's), named as [`Hit::struck`] names
    /// collidables; `None` for a record with none.
    pub owner: Option<u32>,
    /// That collidable's material byte, what a struck bike plays.
    pub owner_material: u8,
    /// The struck collidable.
    pub struck: u32,
    /// Its material byte.
    pub struck_material: u8,
    /// Its category is in the foliage mask (a plant's trunk).
    pub struck_foliage: bool,
    /// What the table reads as the owner's speed, feet a second: its body's shock peak, else its
    /// chassis' impact accumulator (rule 1 of the module's list). Not the owner's velocity.
    pub velocity: Vf,
    /// Where the owner is (that body's or chassis' origin): where the sound plays.
    pub at: Vf,
}

/// One material's collision numbers, as the table's update reads them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    pub id: u8,
    /// `CollisionMinImpulse` (0 when the file leaves it out) and `CollisionMaxImpulse` (1).
    pub min_impulse: F,
    pub max_impulse: F,
}

/// A hit to play: the material's place in the table, its level `0..=100`, where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sound {
    pub material: usize,
    pub level: i32,
    pub at: Vf,
}

/// **The table's update**: the frame's queue, played in its order after the pairs are dropped. The
/// queue is emptied. `materials` is the table in its order (the first of an ID is the one found);
/// `foliage` its `SolidFoliageID`.
pub fn play(queue: &mut Vec<Hit>, materials: &[Material], foliage: u8) -> Vec<Sound> {
    let mut speeds: Vec<F> = queue.iter().map(|h| speed(h.velocity)).collect();
    drop_pairs(queue, &mut speeds);
    // A struck bike's striker without a collidable leaves the material the entry before had: the
    // update keeps it in a register across the loop.
    let mut carried = 0u8;
    let mut out = Vec::new();
    for (h, &speed) in queue.iter().zip(&speeds) {
        let material = if h.struck_material == BIKE {
            if h.owner.is_some() { h.owner_material } else { carried }
        } else if h.struck_foliage {
            foliage
        } else {
            h.struck_material
        };
        carried = material;
        let Some(k) = (material != 0).then(|| materials.iter().position(|m| m.id == material)).flatten() else { continue };
        let m = &materials[k];
        if speed < m.min_impulse {
            continue;
        }
        out.push(Sound { material: k, level: level(speed, m.max_impulse), at: h.at });
    }
    queue.clear();
    out
}

/// The owner's speed: its impulse's length ([`Hit::velocity`]), the `mula`/`madda`/`madd` sum.
pub fn speed(v: Vf) -> F {
    let v = [v.x(), v.y(), v.z()].map(F);
    F::dot(v, v).sqrt()
}

/// **The level**: `(int)(100 × (speed / max))`, cut toward zero, held to `0..=100`.
pub fn level(speed: F, max_impulse: F) -> i32 {
    (F(100.0) * (speed / max_impulse)).to_i32().clamp(0, 100)
}

/// The pairs struck both ways round: for each entry in turn, every later entry whose struck one is
/// this entry's owner and whose owner is this entry's struck one. The slower goes (the later on a
/// tie), swapped out for the queue's last; a dropped earlier entry is looked at again in its new
/// form.
fn drop_pairs(queue: &mut Vec<Hit>, speeds: &mut Vec<F>) {
    if queue.len() < 2 {
        return;
    }
    let mut i = 0;
    while i < queue.len() {
        if let Some(a) = queue[i].owner {
            let mut j = i + 1;
            while let Some(found) = (j..queue.len()).find(|&j| queue[j].struck == a) {
                j = found;
                if queue[j].owner == Some(queue[i].struck) {
                    if speeds[i] < speeds[j] {
                        queue.swap_remove(i);
                        speeds.swap_remove(i);
                        // Looked at again, the entry now in slot `i`.
                        i = i.wrapping_sub(1);
                        break;
                    }
                    queue.swap_remove(j);
                    speeds.swap_remove(j);
                    // The entry swapped into `j` is the next one looked at.
                    continue;
                }
                j += 1;
            }
        }
        i = i.wrapping_add(1);
    }
}

/// The global mask a struck collidable's category is tested against for the foliage swap: a plant's
/// trunk ([`crate::flight_search::TREE`]). 4 in `s195-knock-2881`.
pub const FOLIAGE_MASK: u32 = 4;

/// A mesh collidable's material byte: its surface word's low byte (0 for a file too old to have one).
pub fn material_of(mesh: &crate::track::Mesh) -> u8 {
    mesh.surface.map_or(0, |s| s as u8)
}

/// **What a listened probe struck** (F27a), as the search that heard it names it: one of the ride's
/// bikes, a plane's hull, a knockable prop's mesh, or a collidable that is never a probe's owner (a
/// scene mesh, a toy, a trunk), whose material and foliage test are read where it was struck.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Struck {
    /// The ride's bike, by player.
    Bike(usize),
    /// A plane's hull, by its seat.
    Plane(usize),
    /// A knockable prop: its collection and its object.
    Prop { collection: usize, object: usize },
    /// Anything else: its place in the search that struck it, its material, its category in
    /// [`FOLIAGE_MASK`].
    Mesh { id: u32, material: u8, foliage: bool },
}

/// Who carries a listened probe: the queue entry's owner, whose velocity and place the table's update
/// reads when it plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    Bike(usize),
    Plane(usize),
    Prop { collection: usize, object: usize },
}

impl Struck {
    /// The collidable's name in the queue: one space for every kind, so the pair test meets an owner
    /// only as the struck one it is.
    pub fn id(&self) -> u32 {
        match *self {
            Struck::Bike(j) => 1 << 28 | j as u32,
            Struck::Plane(j) => 2 << 28 | j as u32,
            Struck::Prop { collection, object } => 3 << 28 | (collection as u32) << 12 | object as u32,
            Struck::Mesh { id, .. } => 4 << 28 | (id & 0x0fff_ffff),
        }
    }
}

impl Owner {
    pub fn struck(&self) -> Struck {
        match *self {
            Owner::Bike(j) => Struck::Bike(j),
            Owner::Plane(j) => Struck::Plane(j),
            Owner::Prop { collection, object } => Struck::Prop { collection, object },
        }
    }
}

/// **The hits a probe's material listener heard** since its owner's last search, in the order heard:
/// what the queue is filled from. Sound only: nothing in the simulation reads it, so it is equal to
/// any other (a state compared, hashed or sent leaves it out) and starts empty on a restored one.
#[derive(Clone, Debug, Default)]
pub struct Heard(pub Vec<Struck>);

impl PartialEq for Heard {
    fn eq(&self, _: &Heard) -> bool {
        true
    }
}

/// **An owner's impulse as the material table reads it** (B27): a bike's chassis impact accumulator
/// after its frame. Sound only, like [`Heard`]: nothing in the simulation reads it and the next
/// frame sets it afresh, so it is equal to any other, prints as nothing (a state compared, hashed or
/// sent leaves it out) and starts at zero on a restored one.
#[derive(Clone, Copy, Default)]
pub struct Impact(pub Vf);

impl PartialEq for Impact {
    fn eq(&self, _: &Impact) -> bool {
        true
    }
}

impl std::fmt::Debug for Impact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Impact")
    }
}

/// [`Heard`] for a set of owners (a collection's objects), each entry by its owner's index; and the
/// ridden bikes its probes hear this frame ([`Ridden`], F27a2).
#[derive(Clone, Debug, Default)]
pub struct HeardBy(pub Vec<(usize, Struck)>, pub Ridden);

/// **The ridden bikes as the props' probes hear them** (F27a2): the bikes' frame collidable mesh
/// ([`crate::flight_search::BIKE_COLLISION`]) and each player's chassis where the props' collide pass
/// finds it, by player. A prop's probe that strikes one is heard striking [`Struck::Bike`]
/// ([`crate::flight_search::bikes_struck`]); where the bikes `file`, it also puts its entry on the
/// prop's list and keeps the mirror for the bike's ([`Ridden::probed`], B29).
#[derive(Clone, Default)]
pub struct Ridden {
    pub mesh: Option<std::sync::Arc<crate::track::Mesh>>,
    pub bikes: Vec<crate::flight_search::Bike>,
    /// The bikes are the props' step's strikers, by player, so a probe's entry on one can name it:
    /// off where the props' step has other strikers (the plane lane's plane) or none.
    pub file: bool,
    /// What this collide pass's probes struck on the bikes, for the bikes' strikes
    /// ([`crate::prop_contact::strike`]): the object, the bike's player and the mirror (kind 1, its
    /// other side not yet named) that goes on the bike's list. Emptied at every pass.
    pub probed: Vec<(usize, usize, crate::contact::Contact)>,
}

impl std::fmt::Debug for Ridden {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ridden").field("mesh", &self.mesh.is_some()).field("bikes", &self.bikes).field("file", &self.file).field("probed", &self.probed).finish()
    }
}

impl PartialEq for HeardBy {
    fn eq(&self, _: &HeardBy) -> bool {
        true
    }
}

/// What the queue reads of an owner or a struck one, from the frame's end state.
pub trait Ride {
    /// An owner's collidable's material, its impulse (its body's shock peak, else its chassis'
    /// impact accumulator: [`Hit::velocity`]) and where it is; `None` for one no longer there, whose
    /// entry is not queued.
    fn owner(&self, o: Owner) -> Option<(u8, Vf, Vf)>;
    /// A struck one's material byte and whether its category is in [`FOLIAGE_MASK`].
    fn struck(&self, s: Struck) -> (u8, bool);
}

/// **The frame's queue** out of what the listeners heard, in the order given: each entry as
/// [`play`] reads it, its owner's numbers off `ride`.
pub fn queue(heard: &[(Owner, Struck)], ride: &dyn Ride) -> Vec<Hit> {
    heard
        .iter()
        .filter_map(|&(o, s)| {
            let (owner_material, velocity, at) = ride.owner(o)?;
            let (struck_material, struck_foliage) = match s {
                Struck::Mesh { material, foliage, .. } => (material, foliage),
                Struck::Bike(_) => (BIKE, false),
                _ => ride.struck(s),
            };
            Some(Hit { owner: Some(o.struck().id()), owner_material, struck: s.id(), struck_material, struck_foliage, velocity, at })
        })
        .collect()
}
