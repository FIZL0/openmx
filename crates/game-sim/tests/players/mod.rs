//! A scripted ride of several riders, for the snapshot tests and the platform vectors (N1).
//!
//! **A stand-in for S54's multi-player ride, which does not exist yet.** Each player here is a whole
//! [`Sim`] stepped one after the other, with no step order of the original's behind it and no
//! rider-to-rider contact: what it proves is that every part of one rider's state saves, restores,
//! encodes and hashes the same, several times over, with the riders in different states at once. When
//! S54 puts the players inside the simulation, this is the file that should step that instead.
//!
//! Built from integers and the crate's stand-in bike, so it is the same on every machine without a
//! file travelling with it, and it proves nothing about the game.

#![allow(dead_code)] // each test binary that includes this uses part of it

use std::sync::Arc;

use game_sim::collide::{Bundle, Record};
use game_sim::rider::Tables;
use game_sim::trick::{SLOTS, Set, Stunt};
use game_sim::world::World;
use game_sim::{Input, Sim, state_hash, stock};
use ps2_float::{F, Vf};

/// How many riders the scripted ride has: enough for the riders to be in four different states.
pub const PLAYERS: usize = 4;

/// The ground the script rides: level, with a kicker across the whole width every [`PERIOD`] feet of
/// `z`, a slope of one in two for [`RAMP`] feet and then a drop back to the floor — so the riders who
/// hold the gas leave the ground, and the ones who hold a trick do it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Kickers;

pub const PERIOD: f32 = 64.0;
pub const START: f32 = 24.0;
pub const RAMP: f32 = 6.0;
/// The face's normal, `(0, 2, -1) / √5`, written out so no square root is taken here.
const FACE: Vf = Vf([0.0, 0.894_427_2, -0.447_213_6, 0.0]);

impl World for Kickers {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        // Host arithmetic, which is exact here: `rem_euclid` and the multiply are IEEE operations
        // with one rounding each, the same on every machine this runs on.
        let t = at.z().rem_euclid(PERIOD) - START;
        if (0.0..RAMP).contains(&t) {
            (Vf::new(at.x(), t * 0.5, at.z(), 1.0), FACE)
        } else {
            (Vf::new(at.x(), 0.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0))
        }
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// 24 made-up stunts, a second long and safe after half of it, so a held trick button does a trick.
pub fn stunts() -> Set {
    Set::new(
        (0..SLOTS)
            .map(|i| Stunt {
                name: format!("stunt{i}"),
                duration: F(1.0),
                safe_time: F(0.5),
                hold_time: F(0.25),
                for_bike: true,
                index: i as i32,
                ..Stunt::default()
            })
            .collect(),
    )
}

/// The riders at tick 0, side by side across `x`, sharing one set of tables. Rider 1 and rider 2
/// wreck; 0 and 3 cannot.
pub fn start() -> Vec<Sim> {
    let tables = Arc::new(Tables::build());
    (0..PLAYERS)
        .map(|i| {
            let phy = stock::phy(&tables.ride.curve);
            let at = Vf::new(12.0 * i as f32, 1.0, 0.0, 1.0);
            let mut sim = Sim::ride(0x4e31_0000 + i as u32, Arc::clone(&tables), phy, at, stock::FACING, stunts());
            sim.rider.sense.wrecks_enabled = i == 1 || i == 2;
            sim
        })
        .collect()
}

const CROSS: u16 = 0x0040;
const SQUARE: u16 = 0x0080;
const CIRCLE: u16 = 0x0020;
const TRIANGLE: u16 = 0x0010;
const L1: u16 = 0x0004;
const R1: u16 = 0x0008;
const L2: u16 = 0x0001;
const UP: u16 = 0x1000;
const LEFT: u16 = 0x8000;

/// Player `p`'s pad on frame `f`: a function of the two integers and nothing else.
pub fn input(p: usize, f: usize) -> Input {
    match p {
        // On the gas, weaving, and a trick held over every kicker.
        0 => {
            let weave = if (f / 45) % 2 == 0 { 40 } else { -40 };
            let trick = if f % 50 >= 30 { CIRCLE | UP } else { 0 };
            Input { buttons: CROSS | trick, stick: (weave, 0) }
        }
        // On the gas with the wreck test on, and the reset chord (R1 held, L1 pressed) every 90.
        1 => {
            let chord = match f % 90 {
                80..=86 => R1,
                87..=88 => R1 | L1,
                _ => 0,
            };
            Input { buttons: CROSS | chord, stick: (0, 64) }
        }
        // Anything at all, from the C library's LCG, as the canonical fixture's pad is.
        2 => {
            let mut x: u32 = 0x5eed_0000 ^ f as u32;
            for _ in 0..3 {
                x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            }
            Input { buttons: (x >> 16) as u16, stick: ((x >> 8) as i8, x as i8) }
        }
        // Gas, brake and clutch in turn, and a second trick button with the stick pulled back.
        _ => {
            let hands = match (f / 20) % 4 {
                0 => CROSS,
                1 => CROSS | L2,
                2 => SQUARE,
                _ => CROSS | TRIANGLE | LEFT,
            };
            Input { buttons: hands, stick: (-20, -100) }
        }
    }
}

/// Every player's pad on frame `f`.
pub fn inputs(f: usize) -> [Input; PLAYERS] {
    std::array::from_fn(|p| input(p, f))
}

/// What happened to the riders over a run: the script is only worth anything if it reaches these.
#[derive(Clone, Copy, Debug, Default)]
pub struct Seen {
    pub airborne: bool,
    pub tricks: bool,
    pub landed: bool,
    pub wrecks: bool,
    pub respawns: bool,
}

/// One tick of every rider, in player order, over the kickers.
pub fn step(players: &mut [Sim], inputs: &[Input], seen: &mut Seen) {
    for (sim, input) in players.iter_mut().zip(inputs) {
        let said = sim.tick_in(*input, &mut Kickers);
        seen.airborne |= sim.rider.ride.air.airborne;
        seen.tricks |= said.started.is_some();
        seen.landed |= said.landed.is_some();
        seen.wrecks |= said.wreck.is_some();
        seen.respawns |= said.respawned;
    }
}

/// The riders' hashes folded together in player order: the one number a session compares.
pub fn hash(players: &[Sim]) -> u64 {
    players.iter().fold(0, |h, sim| state_hash(&sim.hash().to_le_bytes(), h))
}

/// The whole of a rider's state as text, the parts the hash leaves out included (the animation, the
/// rider's own body and idles): a restore that missed one of those would still hash the same.
pub fn whole(sim: &Sim) -> String {
    format!("{} {} {:?} {:?} {:?}", sim.ticks, sim.rng, sim.input, sim.rider, sim.plane)
}
