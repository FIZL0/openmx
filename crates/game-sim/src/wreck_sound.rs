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
use ps2_float::{Acc, F, Vf};

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

// --- past the entry (S173) ---------------------------------------------------------------------

// The entry's pick is not the only one. While the rider is down, the same emitter cries out again on
// the thrown body's touches, a pick of `[IMPACT]` (the crash list) each time, and the bike's own
// `[Wreck]` emitter clanks at the entry and on the wreck body's hits. Every pick of a list of two or
// more draws, the clank's always, so a port that skips them is out of step from the wreck body's first
// touchdown on. Each emitter picks only while nothing of its own still plays, and "still plays" is
// the sound chip's voice state: the clip's length and a tail past its end, which a streamed sound has
// ([`TAIL`]). So the two emitters keep a clock of how long they stay busy.

/// How long an emitter still reads busy after its clip ended. P164 bounds the bike's to more than
/// 0.34 s and at most 0.78 s; the rider's voice is streamed the same way and its tail is taken to be
/// the same, unmeasured. The shell's mixer keeps the same 0.4 s.
pub const TAIL: F = F(0.4);

/// The retail 125's `[Wreck]` list: four clanks.
pub const RETAIL_CLANKS: i32 = 4;

/// What the two emitters were loaded with: each list's clip lengths in seconds, in list order, and
/// the two `MinImpulse`s. Configuration, which a shell that has the sound files hands in
/// ([`crate::rider::Rider::wreck_kit`]); a list it has no lengths for counts its clips as
/// lasting nothing, so its emitter is busy for the tail alone.
#[derive(Clone, Debug, PartialEq)]
pub struct Kit {
    /// The voice's `[Fall]` list.
    pub fall: Vec<F>,
    /// The voice's `[IMPACT]` list, [`Emitter::crash`].
    pub crash: Vec<F>,
    /// `rider.aud`'s `[Params]` `MinImpulse`: how fast the thrown body must be going.
    pub voice_min: F,
    /// The bike's `[Wreck]` list.
    pub clank: Vec<F>,
    /// Its `MinImpulse`: how hard the wreck body must hit.
    pub clank_min: F,
}

impl Default for Kit {
    /// The retail rider's and 125's thresholds, with no clip lengths.
    fn default() -> Kit {
        Kit { fall: Vec::new(), crash: Vec::new(), voice_min: F(12.8), clank: Vec::new(), clank_min: F(18.8) }
    }
}

impl Kit {
    /// How many clanks the bike's list holds: its lengths' count, or the retail 125's when the shell
    /// handed none.
    pub fn clank_count(&self) -> i32 {
        if self.clank.is_empty() { RETAIL_CLANKS } else { self.clank.len() as i32 }
    }
}

/// The clip at `i` of `lengths`, or nothing.
fn length(lengths: &[F], i: i32) -> F {
    usize::try_from(i).ok().and_then(|i| lengths.get(i)).copied().unwrap_or(F(0.0))
}

/// A vector's length as both emitters take it: `x²` into the accumulator, `y²` added there, `z²`
/// added out of it, and the root.
pub fn magnitude(v: Vf) -> F {
    Acc::mula(v.x(), v.x()).madda(v.y(), v.y()).madd(v.z(), v.z()).sqrt()
}

/// **The two emitters' state past their lists**: how long each stays busy, the bike's last clank, and
/// the body's speed the tick before.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clocks {
    /// Seconds the rider's voice still reads busy.
    pub voice: F,
    /// Seconds the bike's clank still reads busy.
    pub clank: F,
    /// The bike's last clank, `-1` before the first.
    pub clank_last: i32,
    /// The thrown body's speed the tick before, while down.
    pub was: F,
}

impl Default for Clocks {
    fn default() -> Clocks {
        Clocks { voice: F(0.0), clank: F(0.0), clank_last: -1, was: F(0.0) }
    }
}

impl Clocks {
    /// The top of a tick `dt` long: both clocks run down.
    pub fn age(&mut self, dt: F) {
        self.voice = (self.voice - dt).max(F(0.0));
        self.clank = (self.clank - dt).max(F(0.0));
    }

    /// Whether the rider's voice still plays.
    pub fn voice_busy(&self) -> bool {
        self.voice > F(0.0)
    }

    /// Whether the bike's clank still plays.
    pub fn clank_busy(&self) -> bool {
        self.clank > F(0.0)
    }

    /// The untouched state: nothing playing, nothing clanked. What a hash of a rider never down skips.
    pub fn idle(&self) -> bool {
        *self == Clocks::default()
    }

    /// **The wreck entry's voice**, [`Emitter::wreck`] on this clock: busy while the voice plays, and
    /// a pick starts its clip's clock.
    pub fn voice_entry(&mut self, e: &mut Emitter, kit: &Kit, reason: i32, rng: &mut Rng) -> Option<(Which, i32)> {
        let got = e.wreck(reason, self.voice_busy(), rng);
        if let Some((which, i)) = got {
            let lengths = match which {
                Which::Crash => &kit.crash,
                Which::Fall => &kit.fall,
            };
            self.voice = length(lengths, i) + TAIL;
        }
        got
    }

    /// **The voice's frame** on the thrown body's touches, every tick after the entry's: down for any
    /// reason but 0 and the shootoff's 13, touching, faster than `MinImpulse`, and the voice idle — an
    /// `[IMPACT]` pick. `speed` is this tick's; the one held against `MinImpulse` is the larger of it
    /// and the last tick's, the speed the body came down at, which the port's bodies can lose on the
    /// very tick they touch where the original's reads it the tick after (as the shell's voice did, P180).
    pub fn voice_frame(&mut self, e: &mut Emitter, kit: &Kit, down: bool, touching: bool, speed: F, rng: &mut Rng) -> Option<i32> {
        let held = speed.max(self.was);
        self.was = if down { speed } else { F(0.0) };
        if !down || !touching || !(kit.voice_min < held) || self.voice_busy() {
            return None;
        }
        let i = e.crash.pick(rng);
        self.voice = length(&kit.crash, i) + TAIL;
        Some(i)
    }

    /// **The player's wreck entry's clank**: the bike's emitter idle and the riding bike's impulse
    /// at least `MinImpulse`. A fall off the back, with none, is silent.
    pub fn clank_entry(&mut self, kit: &Kit, impulse: F, rng: &mut Rng) -> Option<i32> {
        if self.clank_busy() || !(impulse >= kit.clank_min) {
            return None;
        }
        Some(self.clank(kit, rng))
    }

    /// **The clank's frame**, every tick but the entry's: wrecked, the riding record touched (the
    /// wreck body's while down), the emitter idle and the wreck body's shock peak at least `MinImpulse`.
    pub fn clank_frame(&mut self, kit: &Kit, wrecked: bool, touching: bool, impulse: F, rng: &mut Rng) -> Option<i32> {
        if !(wrecked && touching) || self.clank_busy() || !(impulse >= kit.clank_min) {
            return None;
        }
        Some(self.clank(kit, rng))
    }

    /// The clank's pick: unlike [`List::pick`] it draws even for a list of one, and moves on from
    /// the last only for two or more.
    fn clank(&mut self, kit: &Kit, rng: &mut Rng) -> i32 {
        let n = kit.clank_count();
        let mut i = rng.index(n);
        if n > 1 && i == self.clank_last {
            i = (i + 1) % n;
        }
        self.clank_last = i;
        self.clank = length(&kit.clank, i) + TAIL;
        i
    }
}
