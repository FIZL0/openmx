// ---------------------------------------------------------------------------------------------------
// S364: the habits' keeper word in the Sim — the score keeper's mode (rider +0xfa0 → +0x64) that
// ai_habits' cross-up and the race start read, off the Sim's ride: its race or none.
// ---------------------------------------------------------------------------------------------------

use super::path_nets::{S282C_RIDERS, S282C_STATE};
use super::race4::{C33_RIDERS, C33_STATE};
use super::*;
use game_sim::Sim;
use game_sim::ai::{AiRider, Entry, Roster, keeper_mode, keeper_mode_of};
use game_sim::race::{Gate, Race};
use game_sim::world::Flat;
use ps2_oracle::{Ee, Kit};

/// A race as the Sim holds one: the gate closed, no riders seen, no laps.
fn a_race() -> Race {
    Race::new(Gate::new(), &[], F(3.0), Rng::from_state(1))
}

/// Each AI rider's score keeper mode in `image`, by brain (`brains`, the rider its interface's).
fn modes_of(ee: &Ee, brains: impl IntoIterator<Item = u32>) -> Vec<i32> {
    brains
        .into_iter()
        .map(|b| {
            let r = ee.u32_at(b + 0x40) - 0x10d0;
            let keeper = ee.u32_at(r + 0xfa0);
            assert_ne!(keeper, 0, "brain {b:#x}'s rider {r:#x} has a score keeper");
            ee.u32_at(keeper + 0x64) as i32
        })
        .collect()
}

/// The mapping against the original's own RAM: on sx01's race image (S282c) every AI rider's score
/// keeper is in the race mode, which is the Sim's mode with a race; on free09's free ride with four
/// riders (C33) every AI rider's keeper is in the free-ride mode, the Sim's with none. A missing image
/// skips its half.
#[test]
fn s364_the_keeper_mode_is_the_images_on_a_race_and_a_free_ride() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => return skip(&why),
    };
    let mut proved = 0;
    let race = kit.ctx("states").join(S282C_STATE);
    if race.exists() {
        let ee = Ee::from_ram(&race).unwrap_or_else(|e| panic!("{e}"));
        let want = keeper_mode_of(Some(&a_race()));
        assert_eq!(want, keeper_mode::RACE);
        assert_eq!(modes_of(&ee, S282C_RIDERS), [want; 3], "sx01's race image: the AI riders' keeper +0x64");
        proved += 1;
    } else {
        skip(&format!("{} is not there (research/c31's race start, S282c)", race.display()));
    }
    let free = kit.ctx(C33_STATE);
    if free.exists() {
        let ee = Ee::from_ram(&free).unwrap_or_else(|e| panic!("{e}"));
        let brains = C33_RIDERS.iter().map(|r| r.2).filter(|&b| b != 0);
        let want = keeper_mode_of(None);
        assert_eq!(want, keeper_mode::FREE_RIDE);
        assert_eq!(modes_of(&ee, brains), [want; 3], "free09's free ride (C33): the AI riders' keeper +0x64");
        proved += 1;
    } else {
        skip(&format!("{} is not there (C33's free09 ride)", free.display()));
    }
    if proved == 0 {
        skip("neither image is there");
    }
}

/// The Sim feeds it: an AI rider's habits hold the free-ride mode after a think on a ride with no
/// race, and the race mode after one on a ride with a race, whatever they held before.
#[test]
fn s364_the_sim_puts_the_keeper_mode_in_the_habits() {
    let mut sim = Sim::new(0x5364);
    let mounts = game_sim::stock::mounts(&sim.phy);
    let at = game_sim::stock::SPAWN;
    let r = game_sim::rider::Rider::new(&sim.phy, mounts, at, game_sim::stock::FACING, Set::default());
    let phy = (*sim.phy).clone();
    sim.add_player(phy, r);
    let roster = Roster { humans: 1, ghost: false, ai: vec![AiRider { identity: 0, skill: 0 }], all_ai: None };
    sim.spawn_ai(&roster, None, 90, 0, &Entry::default(), Rng::from_state(1));
    let keeper = |sim: &Sim| sim.ai.as_deref().expect("a lane").pilots[0].1.racer.habits.keeper_state;
    let mut world = Flat::default();
    let dt = F(1.0 / 30.0);
    for (race, want) in [(None, keeper_mode::FREE_RIDE), (Some(a_race()), keeper_mode::RACE), (None, keeper_mode::FREE_RIDE)] {
        sim.ai.as_deref_mut().unwrap().pilots[0].1.racer.habits.keeper_state = -1;
        sim.race = race.map(Box::new);
        sim.think(1, dt, &mut world);
        assert_eq!(keeper(&sim), want, "race {}", sim.race.is_some());
    }
}
