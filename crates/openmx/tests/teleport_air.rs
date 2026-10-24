//! P141 — a teleport onto another player puts you where they are, in the air too. Each case is
//! ridden the way the shell and the online session ride it: the selection stepped to the other
//! player, Up held until it fires, with every seat's spot taken before the tick (`vehicle::lane_spot`,
//! `Garage::spot`), and the plane's markers through `vehicle::lane_markers` online and
//! `Garage::markers` offline. The disc's biplane over a flat floor, at the 30 Hz clamp; skipped
//! without a disc.

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

use game_sim::Sim;
use game_sim::flight::Flight;
use game_sim::input::{Control, Report};
use game_sim::markers::{self, Did, Pick, SLOTS, Spot};
use game_sim::ride::Controls;
use game_sim::world::Flat;
use ps2_float::{F, Vf};
use vehicle::{Garage, Kind, Plane, Spawn, Wanted};

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);
const GAS: u16 = 0x0040; // Cross, the shipped preset's gas, the plane's too

/// The disc's biplane, or `None` (said) without a disc.
fn the_discs_plane() -> Option<Flight> {
    use game_render::disc::Disc;
    let mut disc = match Disc::open_configured() {
        Ok(disc) => disc,
        Err(e) => {
            eprintln!("skipped: {e}");
            return None;
        }
    };
    let name = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("machine.res"))?.name.clone();
    let res = disc.res(&name).expect("machine.res");
    let bytes = res.read_suffix(game_sim::phy::BIPLANE_PHY).expect("machine.res carries biplane.phy");
    Some(Flight::new(&game_sim::phy::Plane::read_bytes(&bytes)))
}

/// The plane's bay on the flat floor, its wheels on it.
fn bay(flight: &Flight, x: f32) -> Spawn {
    let lowest = flight.tires.iter().map(|t| t.center.y() - t.radius.0).fold(f32::MAX, f32::min);
    Spawn { at: Vf::new(x, -lowest, -300.0, 1.0), facing: Vf::new(0.0, 0.0, -1.0, 0.0) }
}

/// An online seat flying the plane: started in a bay, then put 300 ft up at 130 ft/s heading -z
/// with the throttle open, and flown on gas for two seconds.
fn flying_seat(template: &Flight) -> Sim {
    let mut sim = Sim::new(0x141);
    vehicle::start_lane(template, bay(template, 400.0), &mut sim);
    let mut f = sim.plane.take().unwrap();
    f.surfaces.throttle = F::ONE;
    f.launch(Vf::new(400.0, 300.0, -300.0, 1.0), Vf::new(0.0, 0.0, -1.0, 0.0), Vf::new(0.0, 0.0, -130.0, 0.0), &sim.tables.ride.acos);
    sim.plane = Some(f);
    for _ in 0..60 {
        fly(&mut sim, Report::from_buttons(GAS));
    }
    let f = sim.plane.as_ref().unwrap();
    assert!(f.crash.reason == 0 && !f.measure.grounded && f.body.position.y() > 200.0, "the target is not flying: {:?}", f.body.position);
    sim
}

/// One tick of an online seat's plane on `report`, no markers: what `net::rides` runs for it.
fn fly(sim: &mut Sim, report: Report) {
    sim.poll(DT, &[report, Report::missing()]);
    vehicle::fly_lane(sim, DT, Wanted::default(), &mut Flat::default());
}

/// One online tick of seat 0 in the plane with seat 1 at `them`: the spots before anybody moves,
/// then each seat's step, then seat 0's markers. `them` steps on gas (in the plane) or rides on
/// nothing.
fn online_tick(me: &mut Sim, them: &mut Sim, report: Report) -> Option<Did> {
    let mut world = Flat::default();
    let spots = [vehicle::lane_spot(me, &mut world), vehicle::lane_spot(them, &mut world)];
    me.poll(DT, &[report, Report::missing()]);
    vehicle::fly_lane(me, DT, Wanted::default(), &mut world);
    step_other(them);
    vehicle::lane_markers(me, 0, &spots, &[true, true], &mut world)
}

fn step_other(them: &mut Sim) {
    if them.plane.is_some() {
        fly(them, Report::from_buttons(GAS));
    } else {
        let c = them.poll(DT, &[Report::missing(), Report::missing()]);
        them.step(DT, &c, &mut Flat::default());
    }
}

/// Right pressed until the selection is on the other player, then Up held until it fires; `tick`
/// is one tick of seat 0 on a report. What the Up did.
fn up_onto_the_other(mut tick: impl FnMut(Report) -> Option<Did>, with: u16) -> Option<Did> {
    for _ in 0..SLOTS {
        tick(Report::from_buttons(with | Control::Right.mask()));
        tick(Report::from_buttons(with));
    }
    (0..30).find_map(|_| tick(Report::from_buttons(with | Control::Up.mask())))
}

fn length(v: Vf) -> f32 {
    v.x().hypot(v.y()).hypot(v.z())
}

fn apart(a: Vf, b: Vf) -> f32 {
    length(Vf::new(a.x() - b.x(), a.y() - b.y(), a.z() - b.z(), 0.0))
}

/// The plane's model origin.
fn origin(f: &Flight) -> Vf {
    f.pose().1
}

/// A bike teleported to a plane at 300 ft appears [`markers::CLEARANCE`] under it at its velocity
/// and falls on: the next tick it is in the air, still within a clearance (and a tick's fall) of
/// the plane, moving on its heading at its speed less a tick of the bike's air drag, and not wrecked — online, the
/// plane's spot coming from `vehicle::lane_spot`, and the same machine twice is the same tick.
#[test]
fn a_bike_teleported_to_a_flying_plane_falls_from_under_it_at_its_velocity() {
    let Some(template) = the_discs_plane() else { return };
    let mut plane = flying_seat(&template);
    let mut bike = Sim::new(0x88);
    let tick = |bike: &mut Sim, plane: &mut Sim, report: Report| {
        let mut world = Flat::default();
        let spots = [vehicle::lane_spot(bike, &mut world), vehicle::lane_spot(plane, &mut world)];
        assert!(spots[1].flight.is_some(), "a flying plane's spot carries no flight");
        let c = bike.poll(DT, &[report, Report::missing()]).stick_alone();
        bike.step(DT, &c, &mut world);
        step_other(plane);
        markers::frame_among(bike, 0, &spots, &[true, true], &Controls::default())
    };
    let mut before = None;
    let did = up_onto_the_other(
        |r| {
            before = Some(vehicle::lane_spot(&plane, &mut Flat::default()));
            tick(&mut bike, &mut plane, r)
        },
        0,
    );
    assert_eq!(did, Some(Did::Teleported(Pick::Player(1))));
    let spot = before.unwrap();
    let theirs = spot.flight.as_deref().unwrap();
    let at = bike.rider.ride.position();
    let want = origin(theirs);
    assert_eq!([at.x(), at.y(), at.z()], [want.x(), want.y() - markers::CLEARANCE, want.z()], "not a clearance under the plane");
    assert_eq!(bike.rider.ride.chassis.vel.to_bits()[..3], theirs.body.velocity.to_bits()[..3], "not at the plane's velocity");
    assert!(bike.rider.ghost.on, "the teleport's grace");

    // Every peer runs the same tick: two copies stay one.
    let (mut b2, mut p2) = (bike.clone(), plane.clone());
    tick(&mut bike, &mut plane, Report::missing());
    tick(&mut b2, &mut p2, Report::missing());
    assert_eq!(bike.hash(), b2.hash());

    let (at, them) = (bike.rider.ride.position(), origin(plane.plane.as_ref().unwrap()));
    let vel = bike.rider.ride.chassis.vel;
    let pv = plane.plane.as_ref().unwrap().body.velocity;
    println!("a tick on: bike {at:?} at {vel:?}, plane {them:?} at {pv:?}");
    assert!(bike.rider.ride.air.airborne, "the bike is not in the air");
    assert!(bike.rider.state.wreck.is_none() && bike.rider.crash.is_none(), "the teleport wrecked the bike");
    assert!(apart(at, them) < markers::CLEARANCE + 3.0, "{} ft from the plane", apart(at, them));
    assert!(at.y() > 250.0, "the bike is not up with the plane: {at:?}");
    // A tick of the bike's own air drag, which at the plane's 128 ft/s takes 14 ft/s off it (11%
    // of the speed; 1.3 ft/s at 40), and of gravity: on the plane's heading, a little slower.
    let (sv, sp) = (length(vel), length(pv));
    let along = (vel.x() * pv.x() + vel.y() * pv.y() + vel.z() * pv.z()) / (sv * sp);
    assert!(sv > 0.85 * sp && sv <= sp, "the bike is at {sv} ft/s, the plane at {sp}");
    assert!(along > 0.999, "the bike is off the plane's heading: {vel:?} for {pv:?}");
}

/// A plane teleported to a flying plane flies on beside it: [`markers::BESIDE`] feet off its right
/// wing, at its velocity and attitude, its own throttle kept; and two seconds on, flown on the same
/// gas with the throttle opened to theirs, still beside it at its speed and heading, neither wrecked. Online through `vehicle::lane_markers` (the
/// teleporter standing in its bay with the throttle shut, which it keeps) and offline through
/// `Garage::markers` alike.
#[test]
fn a_plane_teleported_to_a_flying_plane_flies_on_beside_it() {
    let Some(template) = the_discs_plane() else { return };

    // Online.
    let mut them = flying_seat(&template);
    let mut me = Sim::new(0x107);
    vehicle::start_lane(&template, bay(&template, 0.0), &mut me);
    let mut before = None;
    let did = up_onto_the_other(
        |r| {
            before = Some(vehicle::lane_spot(&them, &mut Flat::default()));
            online_tick(&mut me, &mut them, r)
        },
        0,
    );
    assert_eq!(did, Some(Did::Teleported(Pick::Player(1))));
    check_beside(me.plane.as_ref().unwrap(), before.unwrap().flight.as_deref().unwrap(), F::ZERO);
    // The throttle stayed this plane's own, shut in its bay: opened to theirs, it flies as they do.
    me.plane.as_mut().unwrap().surfaces.throttle = them.plane.as_ref().unwrap().surfaces.throttle;
    let mut peer = (me.clone(), them.clone());
    for _ in 0..60 {
        online_tick(&mut me, &mut them, Report::from_buttons(GAS));
        online_tick(&mut peer.0, &mut peer.1, Report::from_buttons(GAS));
    }
    assert_eq!(me.hash(), peer.0.hash(), "two peers parted");
    flies_beside(me.plane.as_ref().unwrap(), them.plane.as_ref().unwrap());

    // Offline: the shell's garage, its plane flown and its markers run by `Garage::pass`.
    let mut them = flying_seat(&template);
    let mut sim = Sim::new(0x107);
    let mut garage = Garage::new(Kind::Biplane, Some(Plane::flying(bay(&template, 0.0), template.clone()))).unwrap();
    let mut tick = |sim: &mut Sim, garage: &mut Garage, them: &mut Sim, report: Report| {
        let mut world = Flat::default();
        let spots = [garage.spot(sim, &mut world), vehicle::lane_spot(them, &mut world)];
        let controls = sim.poll(DT, &[report, Report::missing()]).stick_alone();
        garage.step(sim, DT, &controls, &mut world);
        step_other(them);
        garage.markers(sim, 0, &spots, &[true, true], &mut world)
    };
    let mut before = None;
    let did = up_onto_the_other(
        |r| {
            before = Some(vehicle::lane_spot(&them, &mut Flat::default()));
            tick(&mut sim, &mut garage, &mut them, r)
        },
        0,
    );
    assert_eq!(did, Some(Did::Teleported(Pick::Player(1))));
    let mine = garage.plane.as_ref().unwrap();
    check_beside(mine.flight.as_ref().unwrap(), before.unwrap().flight.as_deref().unwrap(), F::ZERO);
    assert_eq!(mine.at.to_bits()[..3], origin(mine.flight.as_ref().unwrap()).to_bits()[..3], "the drawn plane is not where it flies");
    garage.plane.as_mut().unwrap().flight.as_mut().unwrap().surfaces.throttle = them.plane.as_ref().unwrap().surfaces.throttle;
    for _ in 0..60 {
        tick(&mut sim, &mut garage, &mut them, Report::from_buttons(GAS));
    }
    flies_beside(garage.plane.as_ref().unwrap().flight.as_ref().unwrap(), them.plane.as_ref().unwrap());
}

/// `mine` is `theirs` moved [`markers::BESIDE`] along its right wing, at its velocity and attitude,
/// unwrecked, with `throttle` its own.
fn check_beside(mine: &Flight, theirs: &Flight, throttle: F) {
    let side = theirs.measure.matrix.0[0];
    let (a, b) = (origin(mine), origin(theirs));
    for k in 0..3 {
        assert!((a[k] - b[k] - side[k] * markers::BESIDE).abs() < 1e-2, "not beside: {a:?} {b:?} along {side:?}");
    }
    assert_eq!(mine.body.velocity.to_bits(), theirs.body.velocity.to_bits());
    assert_eq!(mine.body.quat.to_bits(), theirs.body.quat.to_bits());
    assert_eq!(mine.crash.reason, 0);
    assert_eq!(mine.surfaces.throttle, throttle, "the throttle is not the teleporter's own");
}

/// Two planes flown on the same pad and throttle: within 1% of each other's speed and a degree of
/// each other's heading, still [`markers::BESIDE`] apart within 2 ft, neither wrecked.
fn flies_beside(mine: &Flight, theirs: &Flight) {
    let (a, b) = (mine.body.velocity, theirs.body.velocity);
    let speed = (length(a), length(b));
    let dot = (a.x() * b.x() + a.y() * b.y() + a.z() * b.z()) / (speed.0 * speed.1);
    let gap = apart(origin(mine), origin(theirs));
    println!("two seconds on: {speed:?} ft/s, {:.2} deg apart, {gap:.1} ft between", dot.min(1.0).acos().to_degrees());
    assert!(mine.crash.reason == 0 && theirs.crash.reason == 0, "a plane wrecked");
    assert!(!mine.measure.grounded && origin(mine).y() > 100.0, "the teleported plane is not flying");
    assert!((speed.0 - speed.1).abs() < 0.01 * speed.1, "speeds {speed:?}");
    assert!(dot > 1f32.to_radians().cos(), "headings part by {} deg", dot.acos().to_degrees());
    assert!((gap - markers::BESIDE).abs() < 2.0, "{gap} ft apart");
}

/// The rider's seat for a plane to come to: a stand-in bike thrown up off the floor (`launch`, the
/// same throw a bike teleported to a plane gets), one tick on so it is mid-jump.
fn jumping() -> Sim {
    let mut sim = Sim::new(0x88);
    let at = sim.rider.ride.position();
    let (forward, up) = sim.rider.ride.axes();
    sim.rider.ride.launch(Vf::new(at.x(), at.y() + 12.0, at.z(), 1.0), forward, up, Vf::new(0.0, 15.0, 40.0, 0.0));
    let c = sim.poll(DT, &[Report::missing(), Report::missing()]);
    sim.step(DT, &c, &mut Flat::default());
    sim
}

/// A plane teleported to a rider mid-jump is in the air at their spot: a clearance over the bike,
/// level along its heading, at its velocity and out of any wreck, the throttle its own; it falls on
/// from there, in the air the tick after.
#[test]
fn a_plane_teleported_to_a_rider_mid_jump_is_in_the_air_over_them() {
    let Some(template) = the_discs_plane() else { return };
    let mut them = jumping();
    let mut me = Sim::new(0x107);
    vehicle::start_lane(&template, bay(&template, 0.0), &mut me);
    let mut before: Option<Spot> = None;
    let did = up_onto_the_other(
        |r| {
            before = Some(vehicle::lane_spot(&them, &mut Flat::default()));
            online_tick(&mut me, &mut them, r)
        },
        0,
    );
    assert_eq!(did, Some(Did::Teleported(Pick::Player(1))));
    let spot = before.unwrap();
    assert!(!spot.at_rest(), "the rider mid-jump reads as at rest");
    let ride = &spot.motion.as_ref().expect("a riding player's motion").0;
    assert!(ride.air.airborne, "the rider is not mid-jump");
    let f = me.plane.as_ref().unwrap();
    let o = origin(f);
    for (k, want) in [spot.at.x(), spot.at.y() + markers::CLEARANCE, spot.at.z()].into_iter().enumerate() {
        assert!((o[k] - want).abs() < 1e-3, "not a clearance over the rider: {o:?} for {:?}", spot.at);
    }
    assert_eq!(f.body.velocity.to_bits()[..3], ride.chassis.vel.to_bits()[..3], "not at the rider's velocity");
    assert!(f.measure.forward.y().abs() < 1e-5 && f.measure.up.y() > 0.999, "not level");
    assert_eq!((f.crash.reason, f.surfaces.throttle), (0, F::ZERO));
    online_tick(&mut me, &mut them, Report::missing());
    let f = me.plane.as_ref().unwrap();
    assert!(!f.measure.grounded && origin(f).y() > 5.0, "the plane is not in the air: {:?}", origin(f));
}

/// A plane teleported to a rider at rest lands as it did before P141: on its wheels on the floor
/// where the rider stands, stopped. Online and offline.
#[test]
fn a_plane_teleported_to_a_rider_at_rest_lands_by_them() {
    let Some(template) = the_discs_plane() else { return };
    let mut them = Sim::new(0x88);
    for _ in 0..30 {
        step_other(&mut them);
    }
    let spot = Spot::of(&them.rider);
    assert!(spot.at_rest(), "a rider standing still is not at rest: {:?}", them.rider.ride.chassis.vel);

    let mut me = Sim::new(0x107);
    vehicle::start_lane(&template, bay(&template, 0.0), &mut me);
    let mut before: Option<Spot> = None;
    let did = up_onto_the_other(
        |r| {
            before = Some(vehicle::lane_spot(&them, &mut Flat::default()));
            online_tick(&mut me, &mut them, r)
        },
        0,
    );
    assert_eq!(did, Some(Did::Teleported(Pick::Player(1))));
    let landed = bay(&template, 0.0).at.y();
    let check = |f: &Flight, spot: &Spot| {
        let o = origin(f);
        assert!((o.x() - spot.at.x()).abs() < 1e-3 && (o.z() - spot.at.z()).abs() < 1e-3, "not where the rider stands: {o:?}");
        assert!((o.y() - landed).abs() < 1e-3, "not on its wheels on the floor: {o:?}");
        assert_eq!(f.body.velocity.to_bits()[..3], [0; 3], "not at rest");
    };
    check(me.plane.as_ref().unwrap(), &before.clone().unwrap());

    let mut sim = Sim::new(0x107);
    let mut garage = Garage::new(Kind::Biplane, Some(Plane::flying(bay(&template, 0.0), template.clone()))).unwrap();
    let did = up_onto_the_other(
        |r| {
            let mut world = Flat::default();
            let spots = [garage.spot(&sim, &mut world), vehicle::lane_spot(&them, &mut world)];
            before = Some(spots[1].clone());
            let controls = sim.poll(DT, &[r, Report::missing()]).stick_alone();
            garage.step(&mut sim, DT, &controls, &mut world);
            garage.markers(&mut sim, 0, &spots, &[true, true], &mut world)
        },
        0,
    );
    assert_eq!(did, Some(Did::Teleported(Pick::Player(1))));
    check(garage.plane.as_ref().unwrap().flight.as_ref().unwrap(), &before.unwrap());
}

