//! V88 — **the rider animates from the first tick after a switch from the plane back to the bike.**
//!
//! What the body played in those seconds was the start line: `Rider::new` puts the body at a race's
//! gate (its request's cut flag up), and only the throttle opened during a get-ready idle or a start
//! pose lowers it. A ride that has opened the throttle once has lowered it for good, so a
//! bike → plane → bike round trip always rode on; a ride *started in the plane* handed the bike
//! over with the flag still up, and the body played the breathe, a get-ready idle (hands off the
//! bars) and, past the idle, another three-and-a-half-second breathe the throttle cannot cut, while
//! the bike rolled away under it. The shell now builds its riders with no start line
//! (`Rider::no_start_line`).
//!
//! Each case is the shell's two calls a tick for a seat — its garage's step, then the body's
//! `Body::ride` while the bike is up — on the disc's biplane, the stock bike and `thq1`'s body, over
//! a flat floor at the 30 Hz clamp. Skipped without a disc.

#[path = "../src/menu.rs"]
#[allow(dead_code)]
mod menu;
#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;
#[path = "../src/roster.rs"]
#[allow(dead_code)]
mod roster;
#[path = "../src/chase.rs"]
#[allow(dead_code)]
mod chase;
#[path = "../src/join.rs"]
#[allow(dead_code)]
mod join;
#[path = "../src/split.rs"]
#[allow(dead_code)]
mod split;
#[path = "../src/overlay.rs"]
#[allow(dead_code)]
mod overlay;

use game_render::disc::{Disc, Res};
use game_render::rider::{Body, Sex};
use game_sim::Sim;
use game_sim::flight::Flight;
use game_sim::ride::Controls;
use game_sim::rider::Gate;
use game_sim::rider_anim::Mode;
use game_sim::trick::{Set as Stunts24, Stunts};
use game_sim::vehicle_anim::Blend;
use game_sim::world::Flat;
use ps2_float::{F, Vf};
use vehicle::{Garage, Kind, Plane, Spawn};

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);
/// Five seconds of a ride, at the clamp.
const FIVE_SECONDS: usize = 150;

fn res(disc: &mut Disc, ends: &str) -> Res {
    let want = ends.to_ascii_lowercase();
    let name = disc
        .entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with(&want))
        .min_by_key(|e| e.name.len())
        .map(|e| e.name.clone())
        .unwrap_or_else(|| panic!("the disc has no {ends}"));
    disc.res(&name).unwrap_or_else(|why| panic!("{name} does not open: {why}"))
}

/// What a case is built from: the disc's plane and a way to load a fresh body.
struct Kit {
    flight: Flight,
    load: Box<dyn Fn() -> Body>,
}

fn kit() -> Option<Kit> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            println!("SKIPPED: {why}");
            return None;
        }
    };
    let app = res(&mut disc, "appstart.res");
    let table = app.read_suffix("rider\\anim\\stunts.ini").expect("the app archive has no stunt table");
    let set = Stunts24::new(Stunts::parse(&table).expect("the stunt table parses").selection(0));
    let machine = res(&mut disc, "machine.res");
    let bytes = machine.read_suffix(game_sim::phy::BIPLANE_PHY).expect("machine.res carries biplane.phy");
    let flight = Flight::new(&game_sim::phy::Plane::read_bytes(&bytes));
    let motion = Sim::new(0).phy.motion();
    let disc = std::cell::RefCell::new(disc);
    let load = Box::new(move || {
        let rider = res(&mut disc.borrow_mut(), "thq1.res");
        Body::load(vec![rider], "thq1", Sex::Male, "", &set.stunts, motion).expect("the body loads")
    });
    Some(Kit { flight, load })
}

/// A seat as the shell builds one: the body's step rates, and no start line.
fn seat(seed: u32) -> Sim {
    let mut sim = Sim::new(seed);
    sim.rider.body_rates = game_render::vehicle::params();
    sim.rider.no_start_line();
    sim
}

/// The plane's bay on the flat floor beside the bike, its wheels on it.
fn bay(flight: &Flight) -> Spawn {
    let lowest = flight.tires.iter().map(|t| t.center.y() - t.radius.0).fold(f32::MAX, f32::min);
    Spawn { at: Vf::new(40.0, -lowest, -300.0, 1.0), facing: Vf::new(0.0, 0.0, -1.0, 0.0) }
}

/// The cross-fade the blend is drawn with: 1 all of the pose it came out of, 0 all of the clips.
fn ramp(blend: Blend) -> Option<f32> {
    match blend {
        Blend::Free => None,
        Blend::One { ramp, .. } | Blend::Two { ramp, .. } | Blend::Four { ramp, .. } | Blend::Pair { ramp, .. } => Some(ramp.0),
    }
}

/// One tick of what a case watches.
#[derive(Debug)]
struct Tick {
    mode: Mode,
    cut: bool,
    blend: Blend,
    drawn: bool,
    /// The joints' positions, in the bike's own space (a posed body is drawn at the bike's transform).
    joints: Vec<[f32; 3]>,
}

/// One tick of a seat: the garage's step, then the body's frame while the bike is up.
fn tick(garage: &mut Garage, sim: &mut Sim, body: &mut Body, world: &mut Flat, c: &Controls) -> Option<Tick> {
    garage.step(sim, DT, c, world);
    if garage.up != Kind::Bike {
        return None;
    }
    let frame = body.ride(DT, &mut sim.rider, c, Gate::default());
    let (palette, _) = body.palette(None);
    Some(Tick {
        mode: sim.rider.body.machine.mode,
        cut: sim.rider.body_request.cut,
        blend: frame.blend,
        drawn: body.posed,
        joints: palette.iter().map(|m| [m.0[3][0], m.0[3][1], m.0[3][2]]).collect(),
    })
}

fn bike_tick(garage: &mut Garage, sim: &mut Sim, body: &mut Body, world: &mut Flat, c: &Controls) -> Tick {
    tick(garage, sim, body, world, c).expect("the bike is up")
}

/// The body rides on every tick: the riding mode, a pose off the riding grid, the flag down, and the
/// cross-fade running down and never back up, to all of the clips.
fn rides_from_the_first_tick(what: &str, ticks: &[Tick]) {
    let mut last = f32::MAX;
    for (t, k) in ticks.iter().enumerate() {
        assert!(!k.cut, "{what}, tick {t} after the swap: the body is at a start line");
        assert_eq!(k.mode, Mode::Riding, "{what}, tick {t} after the swap: {k:?}");
        assert!(k.drawn, "{what}, tick {t} after the swap: nothing has posed the body");
        let r = ramp(k.blend).unwrap_or_else(|| panic!("{what}, tick {t} after the swap: no riding blend, {:?}", k.blend));
        assert!(r <= last, "{what}, tick {t} after the swap: the fade went back up, {last} -> {r}");
        last = r;
    }
    assert_eq!(last, 0.0, "{what}: the fade never finished");
}

/// A ride started in the plane, switched to the bike after two seconds of flying nowhere, then
/// ridden for five seconds on `c`.
fn plane_first(kit: &Kit, c: Controls) -> Vec<Tick> {
    let (mut sim, mut world, mut body) = (seat(7), Flat::default(), (kit.load)());
    let mut garage = Garage::new(Kind::Biplane, Some(Plane::flying(bay(&kit.flight), kit.flight.clone()))).unwrap();
    for _ in 0..60 {
        assert!(tick(&mut garage, &mut sim, &mut body, &mut world, &Controls::default()).is_none());
    }
    garage.switch(Kind::Bike, &mut sim, &mut world).expect("the bike stands under the plane");
    (0..FIVE_SECONDS).map(|_| bike_tick(&mut garage, &mut sim, &mut body, &mut world, &c)).collect()
}

#[test]
fn a_ride_started_in_the_plane_hands_over_a_riding_body() {
    let Some(kit) = kit() else { return };
    let idle = plane_first(&kit, Controls::default());
    rides_from_the_first_tick("idle", &idle);
    // Nothing posed this body before the swap, and its first pose is all of the riding grid's clips:
    // no cross-fade out of the bind pose it was loaded in. (The fresh body's two lean axes then ease
    // onto the standing bike's for most of a second, 2.4 in at the most: that is the grid following
    // the ride.)
    assert_eq!(ramp(idle[0].blend), Some(0.0), "the first pose after the swap fades: {:?}", idle[0].blend);
    let rolling = plane_first(&kit, Controls { gas: F::ONE, ..Controls::default() });
    rides_from_the_first_tick("rolling", &rolling);

    // The rolling body moves with the ride.
    let moved = rolling
        .windows(2)
        .filter(|w| w[0].joints.iter().zip(&w[1].joints).any(|(a, b)| (0..3).any(|i| (a[i] - b[i]).abs() > 1e-4)))
        .count();
    assert!(moved > FIVE_SECONDS / 2, "the rolling body hardly moved: {moved} ticks of {FIVE_SECONDS}");

    // What it was before (V88): a rider fresh off `Rider::new` is still at the gate, so a shell that
    // built one without `no_start_line` would show the get-ready sequence again.
    let mut sim = Sim::new(7);
    sim.rider.body_rates = game_render::vehicle::params();
    let mut body = (kit.load)();
    let frame = body.ride(DT, &mut sim.rider, &Controls::default(), Gate::default());
    assert_eq!(sim.rider.body.machine.mode, Mode::Gate, "{frame:?}");
}

#[test]
fn a_round_trip_through_the_plane_draws_the_bike_that_never_left() {
    let Some(kit) = kit() else { return };
    let plane = || Some(Plane::flying(bay(&kit.flight), kit.flight.clone()));
    let idle = Controls::default();
    // Two seats on the same bike: one takes the plane and comes back, the other never leaves.
    let (mut went, mut stayed) = (seat(7), seat(7));
    let (mut went_body, mut stayed_body) = ((kit.load)(), (kit.load)());
    let (mut went_garage, mut stayed_garage) = (Garage::new(Kind::Bike, plane()).unwrap(), Garage::new(Kind::Bike, plane()).unwrap());
    let (mut w1, mut w2) = (Flat::default(), Flat::default());
    // Set down and stood still, both.
    for _ in 0..90 {
        bike_tick(&mut went_garage, &mut went, &mut went_body, &mut w1, &idle);
        bike_tick(&mut stayed_garage, &mut stayed, &mut stayed_body, &mut w2, &idle);
    }
    went_garage.switch(Kind::Biplane, &mut went, &mut w1).expect("the plane lands by the bike");
    for _ in 0..60 {
        assert!(tick(&mut went_garage, &mut went, &mut went_body, &mut w1, &idle).is_none());
        bike_tick(&mut stayed_garage, &mut stayed, &mut stayed_body, &mut w2, &idle);
    }
    went_garage.switch(Kind::Bike, &mut went, &mut w1).expect("the bike stands under the plane");
    let mut worst = 0.0f32;
    for t in 0..FIVE_SECONDS {
        let a = bike_tick(&mut went_garage, &mut went, &mut went_body, &mut w1, &idle);
        let b = bike_tick(&mut stayed_garage, &mut stayed, &mut stayed_body, &mut w2, &idle);
        assert_eq!((a.mode, a.cut), (b.mode, b.cut), "tick {t} after the swap");
        assert_eq!(
            std::mem::discriminant(&a.blend),
            std::mem::discriminant(&b.blend),
            "tick {t} after the swap: {:?} vs {:?}",
            a.blend,
            b.blend
        );
        let off = a.joints.iter().zip(&b.joints).map(|(p, q)| (0..3).map(|i| (p[i] - q[i]).abs()).fold(0.0, f32::max)).fold(0.0, f32::max);
        worst = worst.max(off);
        // A tenth of an inch: the two bikes are stood by different hands (the switch's teleport and
        // the ride's own settle), so their suspension, and the rider's lean off it, can differ in the
        // last bits.
        assert!(off < 0.1 / 12.0, "tick {t} after the swap: a joint is drawn {:.4} in from the bike that never left", off * 12.0);
    }
    println!("worst joint apart over five seconds: {:.5} in", worst * 12.0);
}
