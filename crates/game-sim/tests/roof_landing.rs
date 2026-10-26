//! P147 — the rider's head meeting the ground while riding.
//!
//! The original's riding bike searches the terrain with its body rays only while the bike is tipped
//! or upside down: the ground step clears the search when the bike's up is above a quarter and sets
//! it otherwise ([`rider_contact::searches_ground`]). A ray that ends under the ground is a contact
//! against the scenery, and the wreck test's hit clause throws the rider off (reason 6) when that ray
//! is the head's. So a bike that lands on its roof wrecks there, whatever the Air Wrecks switch says:
//! the switch only skips the airborne attitude clause (P138), and a flip still turning skips that
//! clause anyway.
//!
//! These ride race125p (its own `[Wrecks]` limits) with the disc's body rays over a flat floor. The
//! captured roof landing on free09, against the original's tick and reason, is `tests/replay.rs`'
//! `roof_landings_wreck_as_the_original_does_air_wrecks_on_and_off`.

use std::sync::Arc;

use game_render::disc::Disc;
use game_sim::air;
use game_sim::bike_col::{self, BikeCol};
use game_sim::phy::Phy;
use game_sim::ride::Controls;
use game_sim::rider::Tables;
use game_sim::rider_contact;
use game_sim::trick::{Set, Wreck};
use game_sim::world::Flat;
use game_sim::{Sim, stock};
use ps2_float::{F, Vf};
use ps2_oracle::skip;

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);
const SCENERY: Wreck = Wreck(air::Wreck::Scenery as u8);

/// race125p.phy and the bike's two collision files, off the disc.
fn disc(tables: &Tables) -> Option<(Phy, Arc<BikeCol>)> {
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
    Some((Phy::read_bytes(&phy, &tables.ride.curve), Arc::new(col)))
}

/// race125p with the body rays, wrecks on, Air Wrecks as `air_wrecks`, placed `height` feet over the
/// floor with `up` its up axis and going forward at 20.
fn thrown(air_wrecks: bool, height: f32, up: Vf) -> Option<Sim> {
    let tables = Arc::new(Tables::build());
    let (phy, col) = disc(&tables)?;
    let mut sim = Sim::ride(0x147, tables, phy, stock::SPAWN, stock::FACING, Set::default());
    sim.rider.carries_bike_col(col);
    sim.rider.sense.wrecks_enabled = true;
    sim.rider.sense.air_wrecks = air_wrecks;
    let at = Vf::new(stock::SPAWN.x(), stock::SPAWN.y() + height, stock::SPAWN.z(), 1.0);
    sim.rider.ride.place_on(at, stock::FACING, up);
    sim.rider.ride.chassis.vel = Vf::new(0.0, 0.0, 20.0, 0.0);
    Some(sim)
}

/// Each tick until the first wreck or `most` ticks: whether the bike was airborne after it, whether its
/// body rays found the floor ([`rider_contact::ground`] on the tick's end transform), and the wreck.
fn ride(sim: &mut Sim, most: usize) -> Vec<(bool, bool, Option<Wreck>)> {
    let mut out = Vec::new();
    for _ in 0..most {
        let f = sim.step(DT, &Controls::default(), &mut Flat::default());
        let ride = &sim.rider.ride;
        let col = sim.rider.bike_col.as_deref().expect("the body rays");
        let found = !rider_contact::ground(col, &ride.transform(), &mut Flat::default(), DT).is_empty();
        out.push((ride.air.airborne, found, f.wreck));
        if f.wreck.is_some() {
            break;
        }
    }
    out
}

/// The search turns on at a quarter: up exactly 0.25 searches, a hair above it does not.
#[test]
fn the_ground_search_turns_on_below_a_quarter_up() {
    let at = |up_y: f32| {
        let side = (1.0 - up_y * up_y).sqrt();
        game_sim::math::basis(Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Vf::new(side, up_y, 0.0, 0.0), true, false)
    };
    assert!(!rider_contact::searches_ground(&at(1.0)));
    assert!(!rider_contact::searches_ground(&at(0.26)));
    assert!(rider_contact::searches_ground(&at(0.25)));
    assert!(rider_contact::searches_ground(&at(0.0)));
    assert!(rider_contact::searches_ground(&at(-1.0)));
}

/// Off: upside down in the air rides on (P138), and the landing on the roof throws the rider off on
/// the head (reason 6) on the tick after the body rays reach the floor: the rays search from where
/// the last tick left the bike, as the original's collision object does before the bike moves. The drop is short and fast: a bike that
/// is not flipping is levelled by the air control, and dropped from 15 ft race125p comes down on its
/// wheels.
#[test]
fn off_wrecks_a_roof_landing_on_the_head() {
    let Some(mut sim) = thrown(false, 9.0, Vf::new(0.0, -1.0, 0.0, 0.0)) else { return };
    sim.rider.ride.chassis.vel = Vf::new(0.0, -30.0, 20.0, 0.0);
    let seen = ride(&mut sim, 200);
    let n = seen.len();
    assert!(n > 1, "wrecked on the first tick, before the floor: {seen:?}");
    assert!(seen[..n - 1].iter().all(|(_, _, w)| w.is_none()), "{seen:?}");
    assert_eq!(seen[n - 1].2, Some(SCENERY), "landed on the roof: {seen:?}");
    assert!(seen[n - 2].1, "the head wreck came without the body rays in the floor the tick before: {seen:?}");
    assert!(seen[..n - 2].iter().all(|(_, found, _)| !found), "the rays were in the floor a tick early: {seen:?}");
}

/// On, a backflip still turning skips the airborne clause, as the original's does: upside down and
/// flipping, the bike lands on its roof and wrecks on the head on the tick it does with the switch off.
#[test]
fn a_flip_onto_the_roof_wrecks_the_same_on_and_off() {
    let landed = |air_wrecks: bool| {
        let mut sim = thrown(air_wrecks, 15.0, Vf::new(0.0, -0.93, 0.37, 0.0))?;
        sim.rider.ride.air.flipping = true;
        Some(ride(&mut sim, 200))
    };
    let (Some(on), Some(off)) = (landed(true), landed(false)) else { return };
    assert_eq!(on, off, "the switch changed a landing the airborne clause never judged");
    assert_eq!(off.last().unwrap().2, Some(SCENERY), "{off:?}");
    assert!(off.len() > 5, "wrecked in the air: {off:?}");
}

/// Smooth landings stay ridden out: upright off a small drop, under either setting, no wreck and the
/// body rays never search the floor.
#[test]
fn an_upright_landing_rides_out() {
    for air_wrecks in [true, false] {
        let Some(mut sim) = thrown(air_wrecks, 6.0, Vf::new(0.0, 1.0, 0.0, 0.0)) else { return };
        let seen = ride(&mut sim, 120);
        assert_eq!(seen.len(), 120, "air wrecks {air_wrecks}: wrecked {:?}", seen.last());
        assert!(seen.iter().any(|(a, _, _)| !a), "air wrecks {air_wrecks}: never came down");
        assert!(seen.iter().all(|(_, found, _)| !found), "air wrecks {air_wrecks}: the body rays searched the floor");
    }
}
