//! MD120 — cars and the Kiowa on a world that wraps (world_mxu, MD29): neither is thrown at a seam
//! it crosses, and each comes in at the far side driving or flying on, as the rider and the plane do
//! (MD29, MD44).
//!
//! The world is a flat floor 1,134 feet square, wrapped on `x` only, with the same floor not wrapped
//! as the control, where the original's edge throw still throws both. Each machine starts 84 feet
//! short of the far `x` seam (inside the edge throw's 150 feet), heading east: a car on the gas
//! through the shell's own [`vehicle::CarRide`], the helicopter climbing and asking for ground speed
//! forward through [`vehicle::KiowaRide`]. On the wrapped floor it is never thrown and its `x` comes
//! back inside the world past the seam, its `z` untouched; on the other, it is thrown. Off the disc's
//! `machine.res`: skips without a disc, saying so.

#[allow(dead_code)]
#[path = "../src/tire_tracks.rs"]
mod tire_tracks;
#[allow(dead_code)]
#[path = "../src/particles.rs"]
mod particles;
#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;

use game_sim::biplane::{Area, THROWN};
use game_sim::car::CarEvent;
use game_sim::collide::{Bundle, Record};
use game_sim::input::{Control, EventQueue, Pad, Report};
use game_sim::markers::Spot;
use game_sim::world::World;
use ps2_float::{F, Vf};

const DT: f32 = 1.0 / 30.0;
/// The floor's side, feet: world_mxu's are bigger, the seam's test the same.
const SIDE: f32 = 1134.0;
/// Where each machine starts: inside the edge throw's 150 feet of the far `x` seam.
const START: (f32, f32) = (1050.0, 567.0);

/// A flat floor at 0, [`SIDE`] square, wrapped on `x` or not at all, nothing solid.
struct Ring {
    wrap: bool,
}

impl World for Ring {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        (Vf::new(at.x(), 0.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0))
    }
    fn surface(&mut self, _at: Vf) -> u8 {
        1
    }
    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
    fn area(&self) -> Option<Area> {
        Some(Area { width: F(SIDE), depth: F(SIDE) })
    }
    fn wrap(&self) -> [bool; 2] {
        [self.wrap, false]
    }
}

fn machines() -> Option<game_render::Res> {
    let Ok(mut disc) = game_render::Disc::open_configured() else {
        println!("skipped: no disc");
        return None;
    };
    Some(disc.res("machine.res").expect("the machines' archive"))
}

fn east() -> Spot {
    Spot::still(Vf::new(START.0, 0.0, START.1, 1.0), Vf::new(1.0, 0.0, 0.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0))
}

/// Each car on the gas from [`START`] east, for up to `ticks`: the tick it was thrown, if it was,
/// and whether its `x` came back inside past the seam.
fn drive(res: &game_render::Res, machine: vehicle::Machine, wrap: bool, ticks: usize) -> (Option<usize>, bool) {
    let tables = game_sim::rider::Tables::build();
    let table = game_sim::bindings::Table::SHIPPED;
    let mut world = Ring { wrap };
    let mut car = vehicle::CarRide::new(machine, 0, None, &mut |n| res.read_suffix(n).ok()).unwrap();
    car.take(&east(), &mut world).unwrap();
    assert!(car.struck.is_some(), "{machine:?}: no collision object off the disc");
    let mut pad = Pad::new(0);
    pad.connected = true;
    let mut events = EventQueue::new();
    let mut crossed = false;
    let mut last_x = START.0;
    for tick in 0..ticks {
        pad.poll(F(DT), &Report::from_buttons(Control::Cross.mask()), &mut events);
        car.step(&pad, &table, &tables.ride, F(DT), &mut world);
        let m = car.driven.as_ref().unwrap();
        if m.event == Some(CarEvent::Thrown) {
            return (Some(tick), crossed);
        }
        let at = m.body.position;
        assert!(at.0.iter().all(|v| v.is_finite()), "{machine:?}: blew up on tick {tick}");
        assert!((at.z() - START.1).abs() < 5.0, "{machine:?}: wandered off its line to z {}", at.z());
        if at.x() < last_x - SIDE / 2.0 {
            assert!((0.0..SIDE).contains(&at.x()), "{machine:?}: tick {tick}: not brought inside: {at:?}");
            assert!(m.ground_speed > F(10.0), "{machine:?}: tick {tick}: not driving on: {} ft/s", m.ground_speed.0);
            crossed = true;
        }
        last_x = at.x();
        if crossed && at.x() > 60.0 {
            break;
        }
    }
    (None, crossed)
}

#[test]
fn a_car_driven_over_the_seam_is_not_thrown() {
    let Some(res) = machines() else { return };
    for machine in [vehicle::Machine::Buggy, vehicle::Machine::Monster, vehicle::Machine::Baja] {
        let (thrown, crossed) = drive(&res, machine, true, 600);
        println!("{machine:?}: thrown {thrown:?}, crossed {crossed}");
        assert_eq!(thrown, None, "{machine:?}: thrown on the wrapped world");
        assert!(crossed, "{machine:?}: never reached the seam");
        // The control: the same floor not wrapped throws it at its edge, as the original's does.
        let (thrown, _) = drive(&res, machine, false, 600);
        assert!(thrown.is_some(), "{machine:?}: the unwrapped floor's edge did not throw it");
    }
}

/// The helicopter from [`START`], 40 feet up, climbing and asking for ground speed forward (east):
/// the tick it was thrown, if it was, and whether its `x` came back inside past the seam.
fn fly(res: &game_render::Res, wrap: bool, ticks: usize) -> (Option<usize>, bool) {
    let tables = game_sim::ride::Tables::build();
    let table = game_sim::bindings::Table::SHIPPED;
    let mut world = Ring { wrap };
    let phy = res.read_suffix(game_sim::kiowa::KIOWA_PHY).expect("kiowa.phy");
    let rays = res.read_suffix(game_sim::kiowa::KIOWA_RAYS).expect("kiowam.col");
    let mut heli = vehicle::KiowaRide::new(&phy, 0, None).with_rays(&rays);
    let at = Vf::new(START.0, 40.0, START.1, 1.0);
    heli.flown = Some(heli.built(at, Vf::new(1.0, 0.0, 0.0, 0.0), &mut world));
    let mut pad = Pad::new(0);
    pad.connected = true;
    let mut events = EventQueue::new();
    let mut crossed = false;
    let mut last_x = START.0;
    for tick in 0..ticks {
        let climb = if tick < 30 { Control::Cross.mask() } else { 0 };
        pad.poll(F(DT), &Report::from_buttons(climb | Control::Up.mask()), &mut events);
        heli.step(&pad, &table, &tables, F(DT), &mut world);
        let h = heli.flown.as_ref().unwrap();
        if h.ride.crash.reason == THROWN {
            return (Some(tick), crossed);
        }
        assert_eq!(h.ride.crash.reason, 0, "tick {tick}: the helicopter crashed");
        let at = h.ride.body.position;
        assert!(at.0.iter().all(|v| v.is_finite()), "blew up on tick {tick}");
        if at.x() < last_x - SIDE / 2.0 {
            assert!((0.0..SIDE).contains(&at.x()), "tick {tick}: not brought inside: {at:?}");
            // Its record came over with it: the matrix is taken from the moved body.
            let m = h.ride.measure.matrix.0[3];
            assert!((m.x() - at.x()).abs() < 20.0, "tick {tick}: the matrix stayed behind at {m:?}");
            let r = h.collision.record.node.origin;
            assert!((r.x() - at.x()).abs() < 20.0, "tick {tick}: the record stayed behind at {r:?}");
            crossed = true;
        }
        last_x = at.x();
        if crossed && at.x() > 60.0 {
            break;
        }
    }
    (None, crossed)
}

#[test]
fn the_kiowa_flown_over_the_seam_is_not_thrown() {
    let Some(res) = machines() else { return };
    let (thrown, crossed) = fly(&res, true, 900);
    println!("kiowa: thrown {thrown:?}, crossed {crossed}");
    assert_eq!(thrown, None, "thrown on the wrapped world");
    assert!(crossed, "never reached the seam");
    let (thrown, _) = fly(&res, false, 900);
    assert!(thrown.is_some(), "the unwrapped floor's edge did not throw it");
}
