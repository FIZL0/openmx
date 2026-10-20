//! **The wreck sound's draw**: the one piece of the rider's wreck sound that is simulation.
//!
//! The rider body actor's wreck entry ([`crate::ragdoll::Ragdoll::wreck_enter`]) ends by asking the
//! actor's sound emitter for a wreck sound. Playing it is audio and is not this crate's. But picking
//! *which* sound it plays draws from the game's one generator, [`Rng`]. That moves every draw after
//! it, so a port that skips the pick is a draw out of step after every wreck.
//!
//! The emitter keeps two lists of sounds, chosen by the bike's wreck reason:
//!
//! | reason | list |
//! |---|---|
//! | 1–7, 12 | [`Emitter::crash`] |
//! | 8–11, 14 | [`Emitter::fall`] |
//! | 0, 13, 15+ and below 0 | none, and no draw |
//!
//! Nothing is picked either while the emitter is still playing something. A list of one sound, or of
//! none, picks its first entry without drawing. Two or more draw a uniform index, and an index equal
//! to the list's last pick moves on to the next sound, so the same sound never plays twice running.
//!
//! In retail every rider's two lists hold five sounds each (first_recording's save state), so every
//! wreck the switch knows draws exactly once, unless the emitter is busy.

use crate::rng::Rng;

/// One list of sounds as the pick sees it: how many there are, and the last one picked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct List {
    pub count: i32,
    /// `-1` before the first pick, which no index can equal.
    pub last: i32,
}

impl List {
    /// A list of `count` sounds, nothing picked yet.
    pub fn new(count: i32) -> List {
        List { count, last: -1 }
    }

    /// **The engine's no-repeat pick**: a uniform index into the list, moved on by one (and wrapped)
    /// when it is the last pick. A list shorter than two answers its first entry and draws nothing.
    pub fn pick(&mut self, rng: &mut Rng) -> i32 {
        if self.count < 2 {
            self.last = 0;
            return 0;
        }
        let mut i = rng.index(self.count);
        if i == self.last {
            i = (i + 1) % self.count;
        }
        self.last = i;
        i
    }
}

/// Which of the emitter's two lists a wreck picked from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Which {
    /// Thrown, hit or landed hard.
    Crash,
    /// Fell off the front or the back.
    Fall,
}

/// The rider's sound emitter, as much of it as the pick reads and writes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Emitter {
    pub crash: List,
    pub fall: List,
}

/// The size of each retail rider's two lists.
pub const RETAIL_SOUNDS: i32 = 5;

impl Emitter {
    /// The emitter as every retail rider has it: five sounds in each list, nothing picked yet.
    pub fn retail() -> Emitter {
        Emitter { crash: List::new(RETAIL_SOUNDS), fall: List::new(RETAIL_SOUNDS) }
    }

    /// **The wreck entry's last call**: the sound for `reason`, picked and not played.
    ///
    /// `busy` is the emitter's "still playing" status, which is audio state. A caller with no sound
    /// system has no way to know it, and hands in `false`. Answers which list and which entry, or
    /// `None` when nothing was picked.
    pub fn wreck(&mut self, reason: i32, busy: bool, rng: &mut Rng) -> Option<(Which, i32)> {
        if busy {
            return None;
        }
        match reason {
            1..=7 | 12 => Some((Which::Crash, self.crash.pick(rng))),
            8..=11 | 14 => Some((Which::Fall, self.fall.pick(rng))),
            _ => None,
        }
    }
}
