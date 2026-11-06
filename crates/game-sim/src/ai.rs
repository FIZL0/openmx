//! The opponent riders: who they are and how good they are (S12a).
//!
//! When a world starts, the original spawns its players in one pass: the humans first, then as many AI
//! riders as it takes to make up the event's racer count. Each AI rider is given an identity — one of
//! the rider database's entries, drawn with `rand()` and never the same one twice — and a skill level
//! drawn from a range the event's AI difficulty byte decides. That pass is [`spawn_roster`]; the bikes
//! and the riding are later tasks (S12b–S12e).
//!
//! The draws are all the library's `rand()`, in a fixed order — the AllAI pair, then every identity
//! (re-drawn on a repeat), then one skill per AI rider in spawn order — so a port that agrees on the
//! roster but draws in another order would still put the generator out of step for everything after.
//!
//! Two things the original does here that the port leaves to its caller:
//!
//! * it seeds `srand` from the wall clock if nothing has seeded it yet. The caller hands in the
//!   generator it already has.
//! * an event marked as a career round takes its riders from a saved roster or the championship's own
//!   table instead of drawing them (S12f).
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::rng::Rng;
use ps2_float::{F, fpu};

/// The first rider-database entry an AI rider can be. The entries before it are never drawn.
pub const FIRST_AI_IDENTITY: i32 = 22;

/// The most AI riders the original can draw identities for: it keeps them in a twenty-entry array on
/// its stack.
pub const MAX_AI: usize = 20;

/// What the spawn reads from the event and the settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RosterIn {
    /// The settings' racer count, humans included.
    pub racers: i32,
    /// The human players asked for.
    pub humans: i32,
    /// The event's AI difficulty byte (90 when no menu has written it).
    pub difficulty: u8,
    /// How many entries the rider database has.
    pub identities: i32,
    /// The AllAI knob (0 on retail).
    pub all_ai: bool,
    /// The event is a ghost race (activity type 3).
    pub ghost_race: bool,
}

/// One AI rider as the spawn leaves it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AiRider {
    /// Its rider-database entry.
    pub identity: i32,
    /// Its skill level, in the difficulty's [`skill_range`].
    pub skill: i32,
}

/// The two numbers the AllAI knob draws before anything else: a racer number in `1..racers`, and a
/// time between 8 and 13. What reads them later was not traced (see the S12 journal).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AllAiDraw {
    pub racer: i32,
    pub seconds: F,
}

/// Who is on the start line.
#[derive(Clone, Debug, PartialEq)]
pub struct Roster {
    /// Human players, numbered first (player 0 up). The AllAI knob makes this 1.
    pub humans: i32,
    /// A ghost race spawns its ghost (as player 1, after the one human) and no AI riders.
    pub ghost: bool,
    /// The AI riders, numbered after the humans in this order.
    pub ai: Vec<AiRider>,
    pub all_ai: Option<AllAiDraw>,
}

/// The AI skill range `(low, high)` for an event's difficulty byte, both ends included.
///
/// Two straight lines rounded to the nearest whole level and put in order: one is 4 from difficulty
/// 90 up (and rises as the difficulty falls below it), the other is 7 below 80 and falls from 7 to 1
/// between 80 and 100. 100 and up is level 1 only; the default of 90 gives `(4, 4)`.
pub fn skill_range(difficulty: u8) -> (i32, i32) {
    if difficulty >= 100 {
        return (1, 1);
    }
    let level = |from: f32, base: f32| {
        let t = (F::from(i32::from(difficulty)) - F(from)) / F(20.0);
        fpu::to_i32((F(0.5) + (F(base) + F(6.0) * (F(1.0) - t))).to_f32())
    };
    let a = if difficulty < 90 { level(70.0, 4.0) } else { 4 };
    let b = if difficulty < 80 { 7 } else { level(80.0, 1.0) };
    if a < b { (a, b) } else { (b, a) }
}

/// The original's player spawn, as far as the dice go: see the module docs.
///
/// Panics where the original would hang or run off its array: more AI riders than there are database
/// entries from [`FIRST_AI_IDENTITY`] up to tell apart, or more than [`MAX_AI`] of them.
pub fn spawn_roster(rng: &mut Rng, input: RosterIn) -> Roster {
    let mut humans = input.humans;
    let mut all_ai = None;
    if input.all_ai {
        humans = 1;
        let racer = rng.index(input.racers - 1) + 1;
        let seconds = F(8.0) + F(5.0) * rng.unit();
        all_ai = Some(AllAiDraw { racer, seconds });
    }
    if input.ghost_race {
        return Roster { humans: 1, ghost: true, ai: Vec::new(), all_ai };
    }

    let count = input.racers - humans;
    let mut identities: Vec<i32> = Vec::new();
    if count > 0 {
        // `index(n)` truncates toward zero, so it reaches |n| values (one when n is 0).
        let drawable = (input.identities - FIRST_AI_IDENTITY).abs().max(1);
        assert!(count as usize <= MAX_AI, "{count} AI riders: the original keeps room for {MAX_AI}");
        assert!(
            count <= drawable,
            "{count} AI riders from {drawable} drawable identities: the original re-draws for ever"
        );
        while identities.len() < count as usize {
            let id = rng.index(input.identities - FIRST_AI_IDENTITY) + FIRST_AI_IDENTITY;
            if !identities.contains(&id) {
                identities.push(id);
            }
        }
    }
    let (low, high) = skill_range(input.difficulty);
    let ai = identities
        .into_iter()
        .map(|identity| AiRider { identity, skill: low + rng.index(high - low + 1) })
        .collect();
    Roster { humans, ghost: false, ai, all_ai }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The MXEvent default of 90 is the [4, 4] S14 read off the decompilation.
    #[test]
    fn the_default_difficulty_is_level_four() {
        assert_eq!(skill_range(90), (4, 4));
        assert_eq!(skill_range(100), (1, 1));
        assert_eq!(skill_range(75), (7, 9)); // 0.5 + 4 + 6 × 0.75 = 9 against the flat 7
    }

    /// The identities are all different and none comes from below the AI range.
    #[test]
    fn identities_never_repeat() {
        let mut rng = Rng::from_seed(7);
        let input = RosterIn {
            racers: 8,
            humans: 1,
            difficulty: 75,
            identities: 29,
            all_ai: false,
            ghost_race: false,
        };
        let roster = spawn_roster(&mut rng, input);
        let mut ids: Vec<i32> = roster.ai.iter().map(|r| r.identity).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 7);
        assert!(ids.iter().all(|&i| (FIRST_AI_IDENTITY..29).contains(&i)));
    }
}
