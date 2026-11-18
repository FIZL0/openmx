//! S337 — the buggy, the monster truck and the baja at the map's edge, through the shell's own
//! [`vehicle::CarRide`]: the car's step finds it inside the edge throw's 150 feet and the shell throws
//! it back toward the middle, wrecked; once it has flown and come down it is whole again, keeping
//! 0.3 of its speed. The world is a flat floor 1,134 feet square with nothing solid on it; each car
//! starts 84 feet short of its east edge, heading east on the gas. The throw's arithmetic is proved
//! against the original in game-sim's tests/car_wreck.rs; this proves the shell acts on it. Off the
//! disc's `machine.res`: skips without a disc, saying so.

#[allow(dead_code)]
#[path = "../src/tire_tracks.rs"]
mod tire_tracks;
#[allow(dead_code)]
#[path = "../src/particles.rs"]
mod particles;
#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;

use game_sim::biplane::Area;
use game_sim::car::CarEvent;
use game_sim::collide::{Bundle, Record};
use game_sim::input::{Control, EventQueue, Pad, Report};
use game_sim::markers::Spot;
use game_sim::world::World;
use ps2_float::{F, Vf};

const DT: f32 = 1.0 / 30.0;
const SIDE: f32 = 1134.0;
/// Where each car starts: inside the edge throw's 150 feet of the east edge.
const START: (f32, f32) = (1050.0, 567.0);

/// A flat floor at 0, [`SIDE`] square, nothing solid.
struct Floor;

impl World for Floor {
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
}

#[test]
fn a_car_at_the_maps_edge_is_thrown_back_and_lands_whole() {
    let Ok(mut disc) = game_render::Disc::open_configured() else {
        println!("skipped: no disc");
        return;
    };
    let res = disc.res("machine.res").expect("the machines' archive");
    let tables = game_sim::rider::Tables::build();
    let table = game_sim::bindings::Table::SHIPPED;
    for machine in [vehicle::Machine::Buggy, vehicle::Machine::Monster, vehicle::Machine::Baja] {
        let mut world = Floor;
        let mut car = vehicle::CarRide::new(machine, 0, None, &mut |n| res.read_suffix(n).ok()).unwrap();
        let spot = Spot::still(Vf::new(START.0, 0.0, START.1, 1.0), Vf::new(1.0, 0.0, 0.0, 0.0), Vf::new(0.0, 1.0, 0.0, 0.0));
        car.take(&spot, &mut world).unwrap();
        let mut pad = Pad::new(0);
        pad.connected = true;
        let mut events = EventQueue::new();
        let (mut thrown, mut landed) = (None, None);
        for tick in 0..900 {
            pad.poll(F(DT), &Report::from_buttons(Control::Cross.mask()), &mut events);
            let before = car.driven.as_ref().unwrap().body.velocity;
            car.step(&pad, &table, &tables.ride, F(DT), &mut world);
            let m = car.driven.as_ref().unwrap();
            assert!(m.body.position.0.iter().all(|v| v.is_finite()), "{machine:?}: blew up on tick {tick}");
            match (m.event, thrown) {
                (Some(CarEvent::Thrown), None) => {
                    let v = m.body.velocity;
                    assert!(m.wrecked && !m.wreck_lifted, "{machine:?}: thrown but not wrecked");
                    assert!(v.x() < -10.0 && v.y() > 50.0, "{machine:?}: not thrown back toward the middle: {v:?}");
                    assert!(v.z().abs() < 1.0, "{machine:?}: thrown off its line: {v:?}");
                    assert_eq!(m.edge_scale, F(10.0), "{machine:?}: the edge scale is not halved");
                    thrown = Some(tick);
                }
                (Some(CarEvent::Thrown), Some(_)) => panic!("{machine:?}: thrown again on tick {tick} while wrecked"),
                (Some(CarEvent::Unwreck), Some(_)) => {
                    assert!(!m.wrecked && m.latch, "{machine:?}: still wrecked after landing");
                    assert_eq!(m.edge_scale, F(20.0), "{machine:?}: the edge scale is not doubled back");
                    let fast = (before.x() * before.x() + before.z() * before.z()).sqrt();
                    let now = (m.body.velocity.x() * m.body.velocity.x() + m.body.velocity.z() * m.body.velocity.z()).sqrt();
                    assert!(now < 0.5 * fast, "{machine:?}: kept {now} of {fast} ft/s on landing");
                    landed = Some(tick);
                    break;
                }
                _ => {}
            }
        }
        println!("{machine:?}: thrown on tick {thrown:?}, landed whole on {landed:?}");
        let thrown = thrown.unwrap_or_else(|| panic!("{machine:?}: the edge never threw it"));
        let landed = landed.unwrap_or_else(|| panic!("{machine:?}: never came down whole"));
        assert!(thrown < 30 && landed > thrown + 30, "{machine:?}: thrown on {thrown}, landed on {landed}");
        let x = car.driven.as_ref().unwrap().body.position.x();
        assert!(x < START.0 - 150.0, "{machine:?}: came down at x {x}, not back toward the middle");
    }
}
