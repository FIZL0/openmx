//! S22 — a bike left standing on the flat does not shake itself over.
//!
//! The ground solver has no retail address of its own: it is the wiring of `wheel.rs` and `move.rs`,
//! and what it gets wrong shows up as a *trajectory* and not as a wrong word in an object. So what is
//! checked here is the one property a suspension has to have before any of it can be believed — a
//! bike with nobody on the pad, on level ground, comes to rest and *stays* at rest:
//!
//! 1. **It settles.** The two wheels' loads add up to the bike's own weight, which is the static
//!    equilibrium of two springs under one mass and the only place the solver is allowed to stop.
//! 2. **It stays settled.** Over the last four seconds nothing moves: each wheel's compression holds
//!    to a thousandth of a foot and the body's measured g load stays small. The failure this is here
//!    for (`research/journal/.../S22`) was a sideways g load alternating sign and *growing* frame over
//!    frame until the wreck test fired, so a bound on the g load over a long run is exactly the thing
//!    a regression would break.
//!
//! The bike is `tests/freeride-bike.phy.txt`, the hand-written one the rest of this crate's tests use
//! — no number in it is the disc's (`AGENT.md`). The two mounts are this file's: the original takes
//! each one from the bike model's own "TireFront"/"TireRear" node, and a test with no model has to
//! put a wheel hub somewhere, so it puts each one a tire radius up and a wheelbase apart.

use game_sim::phy::Phy;
use game_sim::ride::{Controls, Ride, Tables};
use game_sim::world::Flat;
use ps2_float::{F, Vf};

const BIKE: &str = include_str!("freeride-bike.phy.txt");

/// The frame the original runs a level at: its own 30 fps clamp, which is two ground substeps.
const DT: F = F(game_sim::tick::DeltaTime::CAP_30);

/// Eight seconds of standing, which is three times the two and a half the ringing took to wreck the
/// bike it was found on.
const FRAMES: usize = 240;

/// One wheel's hub, in the chassis' own frame: a tire radius off the floor, half a wheelbase out.
fn mounts(p: &Phy) -> (Vf, Vf) {
    (
        Vf::new(0.0, p.front.tire_radius.0, 2.1, 1.0),
        Vf::new(0.0, p.rear.tire_radius.0, -2.1, 1.0),
    )
}

#[test]
fn a_bike_left_standing_on_the_flat_settles_and_stays_settled() {
    let tables = Tables::build();
    let p = Phy::read_bytes(BIKE.as_bytes(), &tables.curve);
    let (front, rear) = mounts(&p);
    // High enough that both wheels start clear of the floor, so the solver has to catch the bike
    // rather than being handed a pose that is already its answer.
    let at = Vf::new(0.0, 1.0, 0.0, 1.0);
    let mut ride = Ride::new(&p, (front, rear), at, Vf::new(0.0, 0.0, 1.0, 0.0));
    let mut floor = Flat::default();
    let hands = Controls::default();

    let weight = p.mass.weight;
    let mut last = (F::ZERO, F::ZERO);
    let (mut worst_step, mut worst_g) = (F::ZERO, F::ZERO);
    for k in 0..FRAMES {
        ride.frame(&p, &tables, &hands, DT, &mut floor);
        let (f, r) = (ride.front.w.compression, ride.rear.w.compression);
        // The last four seconds: by then a bike that is going to settle has.
        if k >= FRAMES / 2 {
            worst_step = worst_step.max((f - last.0).abs()).max((r - last.1).abs());
            let g = ride.mv.g_peak;
            worst_g = worst_g.max(F(g.x()).abs()).max(F(g.y()).abs()).max(F(g.z()).abs());
        }
        if std::env::var("STAND_TRACE").is_ok() && k % 10 == 0 {
            let g = ride.mv.g_peak;
            println!("{k:>4} g {:>9.3} {:>9.3} {:>9.3}  comp {:>7.4} {:>7.4}  load {:>7.1} {:>7.1}  y {:>7.3} pitch {:>8.4}",
                g.x(), g.y(), g.z(), f.0, r.0,
                ride.front.w.spring_force.0, ride.rear.w.spring_force.0,
                ride.position().y(), ride.chassis.euler.x());
        }
        last = (f, r);
    }

    let load = ride.front.w.spring_force + ride.rear.w.spring_force;
    assert!(
        ride.front.mv.contact && ride.rear.mv.contact,
        "both wheels should still be on the floor: front {} rear {}",
        ride.front.mv.contact, ride.rear.mv.contact,
    );
    assert!(
        (load - weight).abs() < weight * F(0.05),
        "the two wheels should carry the bike's weight: {} of {weight:?}", load.0,
    );
    assert!(
        worst_step < F(0.001),
        "a settled suspension should not move: worst compression step {}", worst_step.0,
    );
    assert!(worst_g < F(1.0), "a settled bike should read no g load: worst {}", worst_g.0);
}
