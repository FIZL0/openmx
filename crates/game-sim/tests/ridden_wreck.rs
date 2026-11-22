//! S502 — a downed rider's wreck bike meets the riding bikes in a ride of several riders.
//!
//! S134 proved [`game_sim::wreck_bike::WreckBike::tick_ridden`] against the original (S93's short
//! split-screen ride); these drive the port's 2p ride with it: P2 down beside P1, its wreck bike
//! tumbling into P1's bike.
//!
//! - `p2s_wreck_bike_puts_mirrors_on_p1s_list`: P2's wreck bike, ticked among the riders' bikes, puts
//!   kind-1 mirrors on P1's record and none on its own; filed on P1's list they lead it and name the
//!   wreck bike's body among the other sides after the players'.
//! - `p2s_wreck_bike_pushes_p1_in_the_ride`: [`Sim::step_all`] with the strike rides P1 off where it
//!   stood, where the same ride with P1 out of the bikes' world (no collision files) leaves it.
//! - `p1s_wreck_bike_pushes_p2_in_the_ride`: the same with the riders swapped. P2 is framed first in
//!   the walk, so this holds only with the wreck bikes ticked ahead of every rider's frame.

use game_render::disc::Disc;
use game_sim::bike_col::{self, BikeCol};
use game_sim::phy::{Part, Phy, Wrecks};
use game_sim::ride::{Controls, Crash, Tables, WRECK_INERTIA};
use game_sim::rider::Rider;
use game_sim::{ridden_wreck, rider_contact};
use game_sim::trick::{Set, Wreck};
use game_sim::world::Flat;
use game_sim::Sim;
use ps2_float::{F, Vf};
use ps2_oracle::skip;
use std::sync::Arc;

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);

/// race125p.phy, the bike's collision files and the wreck bike's part, off the disc.
fn disc(tables: &Tables) -> Option<(Phy, Arc<BikeCol>, Part)> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let res = disc.res("Tracks\\free09_ss_2.res").expect("the split track's archive");
    let read = |name: String| res.read_suffix(&name).unwrap_or_else(|e| panic!("{e}"));
    let col = BikeCol::load(&read(bike_col::rays_file(bike_col::MODEL)), &read(bike_col::shape_file(bike_col::MODEL)))
        .expect("the bike's collision files");
    let part = Part::read_bytes(&read("wreckbike.phy".into()), "PhysicsPart1");
    let bikes: Vec<String> = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let n = n.to_ascii_lowercase();
            n.starts_with("bike\\") && n.ends_with(".res")
        })
        .collect();
    let phy = bikes
        .iter()
        .find_map(|b| disc.res(b).ok()?.read_suffix("race125p.phy").ok())
        .expect("race125p.phy in a bike archive");
    Some((Phy::read_bytes(&phy, &tables.curve), Arc::new(col), part))
}

/// P1 riding at the spawn and P2 down `apart` feet along x, its wreck bike entered from its bike
/// sliding at `towards` feet a second at P1; wrecks off. Both carry the bike's collision files when
/// `p1_col`, else P2 alone.
fn two(apart: f32, towards: f32, phy: &Phy, col: &Arc<BikeCol>, part: &Part, p1_col: bool) -> Sim {
    let mut sim = Sim::new(0x5502);
    Arc::make_mut(&mut sim.phy).collisions = phy.collisions;
    sim.rider.sense.wrecks_enabled = false;
    let mounts = game_sim::stock::mounts(&sim.phy);
    let s = game_sim::stock::SPAWN;
    let at = Vf::new(s.x() + apart, s.y(), s.z(), 1.0);
    let mut r = Rider::new(&sim.phy, mounts, at, game_sim::stock::FACING, Set::default());
    r.sense.wrecks_enabled = false;
    r.carries_wreck(*part, Wrecks::default(), WRECK_INERTIA);
    r.carries_bike_col(Arc::clone(col));
    r.ride.chassis.vel = Vf::new(-towards, 0.0, 0.0, 0.0);
    r.state.wreck = Some(Wreck(5));
    r.crash = Some(Crash::begin(&r.ride, Wreck(5), part, WRECK_INERTIA, &Wrecks::default(), None));
    let p = (*sim.phy).clone();
    sim.add_player(p, r);
    if p1_col {
        sim.rider.carries_bike_col(Arc::clone(col));
    }
    sim
}

/// Beside P1, sliding into it.
const APART: f32 = 1.5;
const TOWARDS: f32 = 6.0;

#[test]
fn p2s_wreck_bike_puts_mirrors_on_p1s_list() {
    let tables = Tables::build();
    let Some((phy, col, part)) = disc(&tables) else { return };
    let mut sim = two(APART, TOWARDS, &phy, &col, &part, true);
    let mut ticks = 0;
    let mut filed = 0;
    for _ in 0..10 {
        let mut probe = sim.clone();
        let struck = probe.wreck_meets_riders(1, DT, &mut Flat::default()).expect("P1's bike is in reach");
        assert!(struck.mirrors[1].is_empty(), "P2's wreck bike struck its own bike");
        assert!(probe.rider_of(1).crash.as_ref().unwrap().bike.flown, "the rider's frame would tick it again");
        let mirrors = &struck.mirrors[0];
        assert!(mirrors.iter().all(|c| c.kind == 1), "a mirror on P1's record is not of kind 1");
        if !mirrors.is_empty() {
            ticks += 1;
            let mut bump = sim.bump(0, Vec::new());
            let own = bump.contacts.len();
            ridden_wreck::file_struck(&mut bump, 0, std::slice::from_ref(&struck), Vf::ZERO, &Flat::default());
            let named = bump.vehicles.len() + bump.planes.len() + bump.props.len();
            assert_eq!(bump.cars.len(), 1, "the wreck bike's body is not among P1's other sides");
            assert_eq!(bump.contacts.len(), (own + mirrors.len()).min(rider_contact::CAPACITY));
            for c in &bump.contacts[..mirrors.len()] {
                assert_eq!((c.kind, c.other), (1, Some(named)), "a mirror on P1's list does not name the wreck bike");
            }
            filed += mirrors.len();
        }
        sim.step_all(DT, &[Controls::default(); 2], &mut Flat::default());
    }
    eprintln!("ticks P2's wreck bike struck P1's bike: {ticks}, mirrors filed: {filed}");
    assert!(ticks > 0, "P2's wreck bike never struck P1's bike");
}

/// How far the riding player `struck` moves along x in ten ticks of [`Sim::step_all`] beside the
/// downed one, with it in the bikes' world and out of it; with `swap` P1 is the one down.
fn pushed(struck: usize, swap: bool) -> Option<(f32, f32)> {
    let tables = Tables::build();
    let (phy, col, part) = disc(&tables)?;
    let mut sims = [true, false].map(|p1_col| two(APART, TOWARDS, &phy, &col, &part, p1_col));
    if swap {
        for s in &mut sims {
            let s = &mut *s;
            std::mem::swap(&mut s.rider, &mut s.others[0].rider);
        }
    }
    let start = sims[0].rider_of(struck).ride.position();
    for s in &mut sims {
        for _ in 0..10 {
            s.step_all(DT, &[Controls::default(); 2], &mut Flat::default());
        }
    }
    let [a, b] = sims.map(|s| (s.rider_of(struck).ride.position().x() - start.x()).abs());
    eprintln!("player {} moved {a} ft along x struck, {b} ft out of the bikes' world", struck + 1);
    assert!(b < 0.01, "out of the bikes' world it moved {b} ft");
    Some((a, b))
}

#[test]
fn p2s_wreck_bike_pushes_p1_in_the_ride() {
    let Some((a, b)) = pushed(0, false) else { return };
    assert!(a > b + 0.01, "P2's wreck bike did not push P1: {a} ft");
}

/// The other way round: P1's wreck bike ticks ahead of P2's frame too, though P2 is framed first.
#[test]
fn p1s_wreck_bike_pushes_p2_in_the_ride() {
    let Some((a, b)) = pushed(1, true) else { return };
    assert!(a > b + 0.01, "P1's wreck bike did not push P2: {a} ft");
}
