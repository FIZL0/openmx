//! **The material table's collision sounds** (F27): which hits of a frame play, at what level.
//!
//! The track's material table (`Col\template.cmt`) puts a listener on the ray probe of every collision
//! object the world node files: the four bikes' probes from the start, a knockable prop's while it is
//! awake (its wake files it, its sleep takes it off). Each hit that probe makes on another collidable is
//! queued — the probe, the struck collidable — and once a frame the table's update plays the queue and
//! empties it ([`play`]):
//!
//! 1. each entry's speed is the length of its probe's owner's velocity (the owner's body's, or with no
//!    body its chassis'), taken before anything is dropped;
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
//! one still sounds plays over it.
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
    /// The owner's velocity (its body's, else its chassis'), feet a second.
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

/// The owner's speed: its velocity's length, the `mula`/`madda`/`madd` sum.
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
