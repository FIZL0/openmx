//! P111 — a marker's Up onto another player carries their motion: the teleporter rides off as they
//! ride, at their speed and spin, and not from a stop on top of them. Ridden as the shell and the
//! online session ride the markers (`crates/openmx/tests/markers.rs`'s tick): every rider's spot
//! taken before the tick, the pads polled with the d-pad out of the ride, every rider stepped, then
//! player 1's markers. The stand-in bike on a flat floor at the 30 Hz clamp.
//!
//! A player who is down has no bike motion to take: their spot is the put-back record and the
//! teleport lands there at rest, as before P111. (A player in the plane is the shell's spot, under
//! the plane and motionless: `crates/openmx/src/vehicle.rs`'s own test.)

use game_sim::hash::Dump;
use game_sim::input::{Control, PADS, Report};
use game_sim::markers::{self, Did, HOLD_MS, Pick, SLOTS, Spot};
use game_sim::ride::{Controls, Ride};
use game_sim::rider::Rider;
use game_sim::trick::Set;
use game_sim::world::Flat;
use game_sim::{Sim, stock};
use ps2_float::{F, Vf};

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);

const GAS: u16 = 0x0040; // Cross, the shipped preset's gas

fn pads(buttons: u16) -> [Report; PADS] {
    let mut r = [Report::missing(); PADS];
    r[0] = Report::from_buttons(buttons);
    r
}

/// One tick: player 1 on `buttons`, player 2 on the same controls (so the two are given the same
/// pads), then player 1's markers.
fn tick(sim: &mut Sim, buttons: u16) -> Option<Did> {
    let spots: Vec<Spot> = (0..sim.players()).map(|i| Spot::of(sim.rider_of(i))).collect();
    let mut hands: Vec<Controls> = sim.poll_all(DT, &pads(buttons)).into_iter().map(Controls::stick_alone).collect();
    let same = hands[0].clone();
    hands.truncate(1);
    hands.resize(sim.players(), same);
    sim.step_all(DT, &hands, &mut Flat::default());
    markers::frame(sim, 0, &spots, &hands[0])
}

/// Player 1 at the spawn and player 2 12 ft beside it, `wrecks` on player 2, and player 1's
/// selection on player 2.
fn two_riders(wrecks: bool) -> Sim {
    let mut sim = Sim::new(0x111);
    let mounts = stock::mounts(&sim.phy);
    let at = Vf::new(stock::SPAWN.x() + 12.0, stock::SPAWN.y(), stock::SPAWN.z(), 1.0);
    let mut r = Rider::new(&sim.phy, mounts, at, stock::FACING, Set::default());
    r.sense.wrecks_enabled = wrecks;
    let phy = (*sim.phy).clone();
    sim.add_player(phy, r);
    for _ in 0..SLOTS {
        tick(&mut sim, Control::Right.mask());
        tick(&mut sim, 0);
    }
    assert_eq!(sim.markers.pick(0, 2), Pick::Player(1));
    sim
}

/// Up held on top of `with` until it fires: the spot player 2 had before that tick, which is what
/// the teleport took.
fn teleport(sim: &mut Sim, with: u16) -> Spot {
    for _ in 0..30 {
        let before = Spot::of(sim.rider_of(1));
        match tick(sim, with | Control::Up.mask()) {
            None => continue,
            Some(did) => {
                assert_eq!(did, Did::Teleported(Pick::Player(1)));
                assert!(sim.input.pad(0).control(Control::Up).held_ms >= HOLD_MS);
                return before;
            }
        }
    }
    panic!("Up never fired");
}

/// Every field of a ride but what is the rider's own and not the bike's motion: the spawn and the
/// put-back record.
fn motion(ride: &Ride) -> Vec<String> {
    Dump::of(ride)
        .fields
        .iter()
        .filter(|f| !f.path.starts_with("spawn") && !f.path.starts_with("back"))
        .map(|f| f.to_string())
        .collect()
}

fn at_rest(ride: &Ride) -> bool {
    let zero = |v: Vf| v.x() == 0.0 && v.y() == 0.0 && v.z() == 0.0;
    zero(ride.chassis.vel) && zero(ride.chassis.ang_vel) && ride.front.mv.spin == F::ZERO && ride.rear.mv.spin == F::ZERO
}

/// Onto a player riding at speed: the teleporter has their body velocity, angular velocity and both
/// wheels' spin, bit for bit, and given the same pads rides their path — every field of the bike the
/// same, one tick behind them (the spot is theirs before the tick, as every seat's is), for as long
/// as the ghost lasts and past it.
#[test]
fn a_teleport_onto_a_riding_player_rides_on_with_their_motion() {
    let mut sim = two_riders(false);
    // Player 2 gets going and turns a little, so there is spin as well as speed; player 1 stands.
    for k in 0..60 {
        let spots: Vec<Spot> = (0..sim.players()).map(|i| Spot::of(sim.rider_of(i))).collect();
        let mut hands: Vec<Controls> = sim.poll_all(DT, &pads(0)).into_iter().map(Controls::stick_alone).collect();
        hands.resize(sim.players(), Controls::default());
        hands[1] = Controls { gas: F::ONE, stick: (F(if k < 40 { 0.0 } else { 0.4 }), F::ZERO), ..Controls::default() };
        sim.step_all(DT, &hands, &mut Flat::default());
        assert_eq!(markers::frame(&mut sim, 0, &spots, &hands[0]), None);
    }
    let them = &sim.rider_of(1).ride;
    assert!(F(them.chassis.vel.z()).abs() > F(1.0), "player 2 is not moving: {:?}", them.chassis.vel);
    assert!(them.front.mv.spin != F::ZERO && them.rear.mv.spin != F::ZERO, "no wheel spin");
    assert!(at_rest(&sim.rider.ride) || F(sim.rider.ride.chassis.vel.z()).abs() < F(1.0));

    let spot = teleport(&mut sim, GAS);
    let taken = &spot.motion.as_ref().expect("a riding player's spot carries their motion").0;
    let me = &sim.rider.ride;
    assert_eq!(me.chassis.vel.to_bits(), taken.chassis.vel.to_bits(), "the body's velocity");
    assert_eq!(me.chassis.ang_vel.to_bits(), taken.chassis.ang_vel.to_bits(), "the body's angular velocity");
    assert_eq!(me.chassis.world_ang_vel.to_bits(), taken.chassis.world_ang_vel.to_bits());
    assert_eq!((me.front.mv.spin.0.to_bits(), me.rear.mv.spin.0.to_bits()), (taken.front.mv.spin.0.to_bits(), taken.rear.mv.spin.0.to_bits()), "the wheels' spin");
    assert_eq!(me.position().to_bits(), spot.at.to_bits(), "on their spot");
    assert!(sim.rider.ghost.on, "the ghost covers the landing");

    // The same pads from here: player 1 is player 2 a tick late, field for field.
    let mut path = vec![motion(&sim.rider_of(1).ride)];
    let ticks = (game_sim::ghost::Ghost::LENGTH.0 / DT.0) as usize + 15;
    for k in 0..ticks {
        tick(&mut sim, GAS);
        path.push(motion(&sim.rider_of(1).ride));
        assert_eq!(motion(&sim.rider.ride), path[k], "tick {k} after the teleport: the paths part");
        assert!(sim.rider.state.wreck.is_none() && sim.rider_of(1).state.wreck.is_none(), "tick {k}: a wreck");
    }
    let (a, b) = (sim.rider.ride.position(), sim.rider_of(1).ride.position());
    assert!(F(a.z() - b.z()).abs() < F(3.0) && F(a.x() - b.x()).abs() < F(3.0), "not alongside: {a:?} {b:?}");
}

/// Onto a player who is down: no motion in their spot, and the bike lands on their put-back record
/// at rest, as before.
#[test]
fn a_teleport_onto_a_downed_player_lands_at_rest() {
    let mut sim = two_riders(true);
    // The stand-in wrecks on its first tick with wrecks on (`Sim::new`): player 2 is down.
    assert!(sim.rider_of(1).state.wreck.is_some() || sim.rider_of(1).crash.is_some(), "player 2 is not down");
    // Player 1 on the move, so landing at rest is the teleport's doing.
    for _ in 0..45 {
        tick(&mut sim, GAS);
    }
    assert!(F(sim.rider.ride.chassis.vel.z()).abs() > F(1.0));
    let down = sim.rider_of(1).state.wreck.is_some() || sim.rider_of(1).crash.is_some();
    assert!(down, "player 2 got up");
    let spot = teleport(&mut sim, 0);
    assert_eq!(spot.motion, None, "a downed player's spot carries no motion");
    let back = sim.rider_of(1).ride.back;
    assert_eq!(spot.at.to_bits(), back.at.to_bits(), "their put-back record");
    assert!(at_rest(&sim.rider.ride), "{:?}", sim.rider.ride.chassis.vel);
    assert!(sim.rider.ghost.on);
}

/// A spot with no motion (a player in the plane: the floor under it) sets the bike down at rest.
#[test]
fn a_still_spot_lands_at_rest() {
    let mut sim = Sim::new(0x111);
    for _ in 0..45 {
        tick(&mut sim, GAS);
    }
    assert!(!at_rest(&sim.rider.ride));
    let at = Vf::new(40.0, 1.0, 40.0, 1.0);
    sim.rider.teleport_to(Spot::still(at, stock::FACING, Vf::new(0.0, 1.0, 0.0, 0.0)));
    assert!(at_rest(&sim.rider.ride));
    assert_eq!(sim.rider.ride.position().to_bits(), at.to_bits());
}

/// Onto a player on another build of bike (a heavier one here): the motion is theirs, the bike is
/// still the teleporter's own — its mass and its engine's tuning — and so is its spawn.
#[test]
fn a_teleport_onto_another_build_keeps_the_teleporters_bike() {
    let mut sim = Sim::new(0x111);
    let mounts = stock::mounts(&sim.phy);
    let mut heavy = (*sim.phy).clone();
    heavy.mass.mass = heavy.mass.mass * F(2.0);
    heavy.engine.rev_limit = heavy.engine.rev_limit * F(0.5);
    let at = Vf::new(stock::SPAWN.x() + 12.0, stock::SPAWN.y(), stock::SPAWN.z(), 1.0);
    let mut r = Rider::new(&heavy, mounts, at, stock::FACING, Set::default());
    r.sense.wrecks_enabled = false;
    sim.add_player(heavy, r);
    for _ in 0..SLOTS {
        tick(&mut sim, Control::Right.mask());
        tick(&mut sim, 0);
    }
    for _ in 0..30 {
        tick(&mut sim, GAS);
    }
    let own = sim.rider.ride.clone();
    assert_ne!(own.chassis.inv_mass, sim.rider_of(1).ride.chassis.inv_mass, "the control: one build");
    let spot = teleport(&mut sim, GAS);
    let taken = &spot.motion.as_ref().unwrap().0;
    let me = &sim.rider.ride;
    assert_eq!(me.chassis.vel.to_bits(), taken.chassis.vel.to_bits());
    assert_eq!(me.chassis.ang_vel.to_bits(), taken.chassis.ang_vel.to_bits());
    assert_eq!(me.chassis.inv_mass, own.chassis.inv_mass, "the bike's own mass");
    assert_eq!(me.engine.rev_limit, own.engine.rev_limit, "the bike's own engine");
    assert_eq!(me.spawn.to_bits(), own.spawn.to_bits(), "the rider's own spawn");
    for _ in 0..30 {
        tick(&mut sim, GAS);
    }
    assert!(sim.rider.state.wreck.is_none());
}
