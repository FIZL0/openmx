//! S224 — the riders a seat can joust, when every seat is a one-player Sim of its own (online, and
//! split screen since N22): [`rider_contact::rivals_among`] and [`Sim::step_among`] ride exactly
//! what [`Sim::step_all`]'s one Sim of N riders rides — the same head-on wrecks on the same ticks,
//! every chassis bit for bit — so a seat meets the others as S54's port of the original's walk does.

use std::sync::Arc;

use game_sim::ride::Controls;
use game_sim::rider::Rider;
use game_sim::trick::Set;
use game_sim::rider_contact;
use game_sim::world::Flat;
use game_sim::Sim;
use ps2_float::{F, Vf};

const SPAWN: Vf = game_sim::stock::SPAWN;

/// `tests/ride.rs`'s stack: `n` riders `rise` feet over one another, player 1 on top, each lower one
/// moving, wrecks on.
fn stacked(n: usize, rise: f32) -> Sim {
    let mut sim = Sim::new(0x5354);
    sim.rider.sense.wrecks_enabled = true;
    let top = Vf::new(SPAWN.x(), SPAWN.y() + rise * (n - 1) as f32, SPAWN.z(), 1.0);
    sim.rider.ride.place(top, game_sim::stock::FACING);
    let mounts = game_sim::stock::mounts(&sim.phy);
    for i in 1..n {
        let at = Vf::new(SPAWN.x(), SPAWN.y() + rise * (n - 1 - i) as f32, SPAWN.z(), 1.0);
        let mut r = Rider::new(&sim.phy, mounts, at, game_sim::stock::FACING, Set::default());
        r.ride.chassis.vel = Vf::new(i as f32, 0.0, 2.0 * i as f32, 0.0);
        let phy = (*sim.phy).clone();
        sim.add_player(phy, r);
    }
    sim
}

/// The same riders, one Sim a seat, as the online ride and split screen hold them.
fn seats(all: &Sim) -> Vec<Sim> {
    (0..all.players())
        .map(|i| {
            let mut s = Sim::new(0x5354);
            s.phy = Arc::new((*all.phy).clone());
            s.rider = all.rider_of(i).clone();
            s
        })
        .collect()
}

fn held(n: usize, k: usize) -> Vec<Controls> {
    (0..n).map(|i| Controls { gas: F(if (k + i) % 3 == 0 { 1.0 } else { 0.0 }), ..Controls::default() }).collect()
}

/// One tick of the seats, the last first (S88), each with the others where they stand now.
fn step_seats(sims: &mut [Sim], controls: &[Controls], dt: F) -> Vec<Option<game_sim::trick::Wreck>> {
    let mut wrecks = vec![None; sims.len()];
    for me in (0..sims.len()).rev() {
        let on: Vec<(&Rider, bool)> = sims.iter().map(|s| (&s.rider, true)).collect();
        let rivals = rider_contact::rivals_among(&on, me, &Flat::default());
        drop(on);
        wrecks[me] = sims[me].step_among(dt, &controls[me], &mut Flat::default(), None, &rivals).wreck;
    }
    wrecks
}

/// The stack jousts as one Sim's does: each rider under another wrecks with reason 7 on the same
/// tick, and every chassis stays the multi-player Sim's for two seconds of held gas after.
#[test]
fn seats_joust_as_one_sim_does() {
    let dt = F(game_sim::tick::DeltaTime::CAP_30);
    let joust = Some(game_sim::trick::Wreck::from(game_sim::air::Wreck::Joust));
    for n in [2, 4] {
        let mut all = stacked(n, 3.0);
        let mut sims = seats(&all);
        for k in 0..60 {
            let controls = held(n, k);
            let want: Vec<_> = all.step_all(dt, &controls, &mut Flat::default()).into_iter().map(|f| f.wreck).collect();
            let got = step_seats(&mut sims, &controls, dt);
            assert_eq!(got, want, "{n} riders, tick {k}: the wrecks");
            if k == 0 {
                assert!(got[1..].iter().all(|w| *w == joust), "{n} riders: every rider under another jousts");
            }
            for (i, s) in sims.iter().enumerate() {
                assert_eq!(s.rider.ride.chassis, all.rider_of(i).ride.chassis, "{n} riders, tick {k}: player {}", i + 1);
            }
        }
    }
}

/// Nobody to joust: a seat out of the others' world (its pad out, frozen, on the plane), a wrecked
/// rider and a ghost are nobody's rival, and the seat steps as [`Sim::step`] alone.
#[test]
fn seats_out_of_the_world_are_nobodys_rivals() {
    let all = stacked(2, 3.0);
    let mut sims = seats(&all);
    let rivals = |sims: &[Sim], on: [bool; 2]| {
        let riders: Vec<(&Rider, bool)> = sims.iter().zip(on).map(|(s, o)| (&s.rider, o)).collect();
        rider_contact::rivals_among(&riders, 1, &Flat::default()).len()
    };
    assert_eq!(rivals(&sims, [true, true]), 1);
    assert_eq!(rivals(&sims, [false, true]), 0, "a seat out of the world");
    sims[0].rider.ghost.on = true;
    assert_eq!(rivals(&sims, [true, true]), 0, "a ghost");
    let dt = F(game_sim::tick::DeltaTime::CAP_30);
    let mut alone = sims[1].clone();
    let said = sims[1].step_among(dt, &Controls::default(), &mut Flat::default(), None, &[]);
    let want = alone.step(dt, &Controls::default(), &mut Flat::default());
    assert_eq!(said.wreck, want.wreck);
    assert_eq!(sims[1].hash(), alone.hash(), "with no rivals the step is Sim::step");
}
