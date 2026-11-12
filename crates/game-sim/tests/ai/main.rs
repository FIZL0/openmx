//! The AI's oracle proofs, one topic per module (K49): this file holds what several share, and a new
//! task adds its tests in a topic file of its own (a new `mod` below) rather than at the end of another,
//! so two tasks' tests do not collide in the merge. A helper a later topic borrows is `pub(crate)` in
//! its own topic, and globbed in below.
//!
//! The AI roster (S12a, `roster`) is proved against the original's own player spawn.
//!
//! The whole spawn runs in the interpreter: the allocator, the player constructor and the two virtual
//! calls it makes on each player (its set-up, and the player manager's add) are hooked, so what is left
//! is exactly the part the port is — the AllAI pair, the ghost-race branch, the skill range from the
//! difficulty byte, the identity draws with their re-draw on a repeat, and one skill draw per AI
//! rider. Ten thousand rosters over every difficulty byte, racer and human counts from nothing to a
//! full grid, database sizes down to just enough, and the generator anywhere in its chain; each one is
//! compared player by player (kind, number, identity, skill) and on the generator's state afterwards.

use game_sim::ai::{
    FIRST_AI_IDENTITY, Flight, JUMP, Jump, Rider, RosterIn, Senses, TrickPlan, ask_for, bump, feed, gate_holds,
    plan_trick, spawn_roster,
};
use game_sim::collide::{Bundle, Record};
use game_sim::world::World;
use game_sim::rng::Rng;
use game_sim::trick::{SLOTS, Set, Stunt, Trick, Tricks};
use ps2_float::F;
use ps2_oracle::{addr, game_or_skip, skip};
use std::cell::RefCell;
use std::rc::Rc;
use game_sim::ai::{self, AvoidTuning, Feeler, Me, Other, Steer, flag};
use game_sim::camera::CurveTable;
use ps2_float::Vf;
use game_sim::ai::path::{self, Net, Node};
use game_sim::ai::{Character, Pair, Profile, Spread, Triple, read_character};
use game_sim::phy::Ini;
use game_sim::ai::{Brain, Climb, Levers, Recovery, Steps, Weights, tick_flag};
use game_sim::ai::seek as sk;
use game_sim::ai::follow as fw;

const BUDGET: u64 = 5_000_000;

/// The objects the spawn reads, laid out in RAM no program segment uses.
const CALLER: u32 = 0x0180_0000; // the spawn's own object: +0x30 -> +0x30 the player manager
const LINK: u32 = 0x0180_1000;
const MANAGER: u32 = 0x0180_2000; // +0x08 its vtable
const APP: u32 = 0x0180_3000;
const SETTINGS: u32 = 0x0180_4000;
const EVENT: u32 = 0x0180_5000;
const HUMAN_IDS: u32 = 0x0180_6000;
const DATABASE: u32 = 0x0180_7000;
const ID_HOLDER: u32 = 0x0180_8000;
const ID_LIST: u32 = 0x0180_9000;
const ID_ENTRIES: u32 = 0x0180_a000;
const VTABLE: u32 = 0x0180_b000;
/// Not code: the two virtual calls land here and are hooked.
const HOOK_SET_UP: u32 = 0x0180_c000;
const HOOK_ADD: u32 = 0x0180_c008;
/// Where the hooked allocator hands out players.
const PLAYERS: u32 = 0x0181_0000;
const PLAYER_SIZE: u32 = 0x800;

/// The entry pointer the database holds for identity `i`.
fn entry(i: i32) -> u32 {
    0x5a00_0000 + i as u32
}

struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u32) -> i32 {
        ((self.next() >> 32) as u32 % n) as i32
    }
}

fn named(names: &[&str]) -> Option<Vec<u32>> {
    let mut out = Vec::new();
    for n in names {
        match addr(n) {
            Ok(a) => out.push(a),
            Err(why) => {
                skip(&why);
                return None;
            }
        }
    }
    Some(out)
}

/// A player as the spawn left it: (kind, +0x6b0 number, +0x6b8 identity entry, +0x6bc skill).
type Seen = (u32, i32, u32, i32);

mod roster; // S12a: the roster
mod controls; // S12b: the ground controller
mod tricks; // S12d: when an AI rider asks for a trick, and which
mod avoidance; // S12c: the avoidance pass
mod brain_copy; // S12e: the rider-to-brain copy
mod path_queries; // S282: the track path's queries and the keep-on-track push
mod character; // S283: the character file
mod tick; // S281: the AI tick
mod seek; // S282a: the free-ride target seeker
mod follow; // S282b: the tick's tail
mod path_nets; // S282c: a track's two path nets off the disc
mod path_branch; // S309: the tick's path branch on sx01's net
mod callouts; // S312: the AI tick's callouts
mod seek_world; // S313: a map's seeker world off the disc

use avoidance::*;
use path_nets::*;
use path_queries::*;
use seek::*;
use tick::*;
use tricks::*;
