//! **A downed rider's wreck bike meets the parked cars** (S486): after a strike on a parked car the
//! rider's wreck bike tumbles on beside it, and its rays strike the car's hull as they strike the
//! scenery.
//!
//! The parked car's collision object is on the riders' node (S485, [`crate::car::Parked`]), and the
//! wreck bike's ray probe (an empty category list) strikes every collidable there as it strikes a
//! flown plane's hull ([`crate::wreck_plane`]): an entry (kind 0) on the wreck bike's list after its
//! terrain entries, naming the car's object, and its mirror (kind 1) on the car's record, which tells
//! the car's struck listener and wakes its body. Its resolve pushes the car's body as it pushes a
//! plane's ([`contact::Side::Body`]).
//!
//! So each downed rider's wreck bike near a parked car is searched with the cars' hulls in its world
//! ([`WreckBike::search_flown`], the hulls where their collision nodes stand, the copy nearest the
//! bike on a world that wraps), stepped with the cars' bodies on its resolve
//! ([`WreckBike::step_flown`]), the bodies written back ([`crate::car::Parked::pushed`]) and every car
//! with a mirror woken ([`crate::car::Parked::struck`]), just before the rider's own frame, whose
//! wreck step then skips the wreck bike's tick ([`WreckBike::flown`]). A wreck bike far from every
//! parked car (further than [`REACH`] past a hull's sphere) is ticked by its rider's frame as before,
//! so a ride whose cars are elsewhere is unchanged. One a flown plane's lane ticked already this tick
//! ([`crate::wreck_plane::search_downed`]) is left as that lane left it.
//!
//! S484's capture (`context/capture/s484/ride1.csv`) has the original's truck woken at rest for single
//! frames after the strike (rows 68 and 70, a 0.001 ft step at 70), with the wrecked rider's bike
//! sliding back along the truck's nose. That is the wreck bike: with its search among the parked
//! cars the port's truck is woken at rest on row 68's frame, steps 0.0012 ft and sleeps as on row
//! 69 (the sleep after the riders' frames, [`crate::car::Parked::sleep`]); the original's second
//! wake (row 70) is not reproduced, the port's wreck bike having slid clear (tests/replay.rs
//! `a_bike_ridden_into_the_parked_truck_pushes_it_as_the_original`).
//!
//! S486c: that row-68 match is the ride's phase. From the script's spot the port's bike reaches the
//! nose 0.68 ft behind the original's frame phase and stops 0.6 ft short of theirs, so its wreck bike
//! coasts on at the bike's 3.6 ft/s into the nose late. With the spot moved to line the approach up,
//! the wreck bike stops where theirs does, slides back as theirs (within 0.01 ft for 4 frames) and
//! leaves the nose 8 frames after the strike: neither at-rest wake is the wreck bike's (tests/replay.rs
//! `the_lined_up_wreck_bike_slides_back_off_the_truck_as_the_original`).
//!
//! F73: they are the thrown rider's. The downed rider's bones are lent the parked cars for the frame
//! ([`crate::Sim::lend_parked`], [`crate::ragdoll_scene::ParkedCar`]), meet their hulls, and each car
//! a bone struck is woken after the frame ([`crate::Sim::parked_back`]): lined up, the truck wakes at
//! rest on rows 68 and 70's frames.

use crate::rider::Rider;
use crate::rider_contact::Flown;
use crate::flight_search::Plane;
use crate::world::World;
use ps2_float::F;

/// How far past a parked car's hull sphere a wreck bike's search may reach it: its rays reach a few
/// feet past its own sphere, well inside this.
pub const REACH: F = F(20.0);

/// Whether `r`'s wreck bike is in the riders' world: the rider down, its part colliding, not a ghost
/// (whose bodies meet nobody), and not ticked already this tick by a flown plane's lane.
fn down(r: &Rider) -> bool {
    r.crash.as_ref().is_some_and(|c| !c.bike.flown)
        && !r.ghost.on
        && r.wreckage.as_ref().is_some_and(|w| w.part.collision_enabled)
}

impl crate::Sim {
    /// **Player `i`'s wreck bike among the parked cars**, ahead of the rider's frame (S486, the
    /// module's comment): nothing for a rider who is up, or a wreck bike out of every car's reach.
    pub(crate) fn wreck_meets_parked(&mut self, i: usize, dt: F, world: &mut dyn World) {
        if self.parked_cars.is_empty() {
            return;
        }
        let n = self.players();
        let crate::Sim { rider, others, parked_cars, .. } = self;
        let r = if i == 0 { rider } else { &mut others[i - 1].rider };
        wreck_meets_cars(r, parked_cars, n, dt, world);
    }
}

/// **`r`'s wreck bike among `parked_cars`**, ahead of the rider's frame ([`crate::Sim::wreck_meets_parked`]),
/// car `k`'s hull named `n + k`: split screen's seats share one copy of the cars (F72), which each
/// seat's downed rider meets so (F75).
pub fn wreck_meets_cars(r: &mut Rider, parked_cars: &mut [crate::car::Parked], n: usize, dt: F, world: &mut dyn World) {
    if parked_cars.is_empty() || !down(r) {
        return;
    }
    let Some(c) = r.crash.as_mut() else { return };
    let from = c.bike.origin();
    let hulls: Vec<_> = parked_cars.iter().enumerate().map(|(k, p)| p.hull(n + k)).collect();
    let them: Vec<Plane<'_>> = hulls
        .iter()
        .zip(parked_cars.iter())
        .map(|(h, p)| {
            let mut at = h.at;
            at.0[3] = crate::nearest_copy(&*world, from, at.0[3]);
            Plane { mesh: h.mesh, at, velocity: p.machine.body.velocity }
        })
        .collect();
    let near = them.iter().any(|p| {
        let (_, centre, radius) = crate::world::Prop::posed(p.mesh, &p.at);
        !(crate::actor::distance(centre, from) > radius + REACH)
    });
    if !near {
        return;
    }
    let capacity = hulls.iter().map(|h| h.capacity).max().unwrap_or(0);
    let (ground, props, trees) = world.wooded_near(from);
    let (movers, mirrors) = c.bike.search_flown(dt, ground.as_deref(), props, trees, &them, capacity);
    drop(ground);
    drop((them, hulls));
    let mut flown: Vec<Flown> = parked_cars.iter().enumerate().map(|(k, p)| p.flown(n + k, from, &*world)).collect();
    c.bike.step_flown(dt, &movers, &mut flown);
    c.bike.flown = true;
    for ((p, f), m) in parked_cars.iter_mut().zip(&flown).zip(&mirrors) {
        p.pushed(f);
        if !m.is_empty() {
            p.struck();
        }
    }
}

impl crate::Sim {
    /// **The parked cars lent to player `i`'s downed rider's bones for the frame** (F73,
    /// [`crate::ragdoll_scene::ParkedCar`]): each one's hull where its collision node stands and its
    /// body, the copies nearest the wreck bike on a world that wraps. Nothing for a rider who is up
    /// or a ghost (whose bodies meet nobody).
    pub(crate) fn lend_parked(&mut self, i: usize, world: &dyn World) {
        if self.parked_cars.is_empty() {
            return;
        }
        let n = self.players();
        let crate::Sim { rider, others, parked_cars, .. } = self;
        let r = if i == 0 { rider } else { &mut others[i - 1].rider };
        lend_cars(r, parked_cars, n, world);
    }

    /// The parked cars player `i`'s bones were lent ([`crate::Sim::lend_parked`]) taken back, and
    /// each one a bone struck woken by its struck listener (F73). Holding none, nothing: split
    /// screen's shared copy was lent from outside the Sim and goes back there ([`cars_back`], F75c).
    pub(crate) fn parked_back(&mut self, i: usize) {
        if self.parked_cars.is_empty() {
            return;
        }
        let crate::Sim { rider, others, parked_cars, .. } = self;
        let r = if i == 0 { rider } else { &mut others[i - 1].rider };
        cars_back(r, parked_cars);
    }
}

/// **`parked_cars` lent to `r`'s downed bones for the frame** ([`crate::Sim::lend_parked`]), car
/// `k`'s hull named `n + k`: split screen's seats share one copy of the cars (F72), which each seat's
/// thrown rider meets so (F75c). Nothing for a rider who is up or a ghost.
pub fn lend_cars(r: &mut Rider, parked_cars: &[crate::car::Parked], n: usize, world: &dyn World) {
    let Some(c) = r.crash.as_ref().filter(|_| !r.ghost.on) else { return };
    let from = c.bike.origin();
    r.ride.parked = parked_cars
        .iter()
        .enumerate()
        .map(|(k, p)| {
            let mut at = p.hull(n + k).at;
            at.0[3] = crate::nearest_copy(world, from, at.0[3]);
            crate::ragdoll_scene::ParkedCar::lent(std::sync::Arc::clone(&p.hull), &at, p.flown(n + k, from, world), k)
        })
        .collect();
}

/// **The cars [`lend_cars`] lent `r`'s bones taken back**, each one of `parked_cars` a bone struck
/// woken by its struck listener ([`crate::Sim::parked_back`], F73; split screen's shared copy, F75c).
pub fn cars_back(r: &mut Rider, parked_cars: &mut [crate::car::Parked]) {
    for c in std::mem::take(&mut r.ride.parked) {
        if let (true, Some(p)) = (c.struck, parked_cars.get_mut(c.car)) {
            p.struck();
        }
    }
}

impl crate::Sim {

    /// **The parked cars as a flown plane's lane meets them** (S486a,
    /// [`crate::wreck_plane::search_downed_parked`]): each one's hull where its body's node stands and
    /// its body, the copies nearest the plane at `flying`, named after the players as their hulls are
    /// in the bikes' lists.
    pub(crate) fn parked_in_plane(&self, flying: ps2_float::Vf, world: &dyn World) -> Vec<crate::wreck_plane::ParkedIn> {
        cars_in_plane(&self.parked_cars, self.players(), flying, world)
    }

    /// **What the flown plane's lane did to the parked cars**, after its wreck bikes' step (S486a,
    /// [`cars_from_plane`]).
    pub(crate) fn parked_from_plane(&mut self, met: &crate::flight::Met, struck: &[bool]) {
        cars_from_plane(&mut self.parked_cars, met, struck);
    }
}

/// **`parked_cars` as a flown plane's lane meets them** ([`crate::Sim::parked_in_plane`]), car `k`
/// named `n + k`: split screen's seats share one copy of the cars (F72), which a seat's lone flown
/// plane meets so (F75a).
pub fn cars_in_plane(parked_cars: &[crate::car::Parked], n: usize, flying: ps2_float::Vf, world: &dyn World) -> Vec<crate::wreck_plane::ParkedIn> {
    parked_cars
        .iter()
        .enumerate()
        .map(|(k, p)| {
            let h = p.hull(n + k);
            let mut at = h.at;
            at.0[3] = crate::nearest_copy(world, flying, at.0[3]);
            let f = p.flown(n + k, flying, world);
            crate::wreck_plane::ParkedIn {
                hull: crate::wreck_plane::Hull { mesh: std::sync::Arc::clone(&p.hull), at, velocity: f.body.velocity, capacity: h.capacity },
                body: crate::prop_contact::Pushed { collection: 0, object: n + k, collider: f.collider, body: f.body, mass: f.mass },
            }
        })
        .collect()
}

/// **What the flown plane's lane did to `parked_cars`**, after its wreck bikes' step (S486a): each
/// body as the plane's resolve and the wreck bikes' left it ([`crate::flight::Met::wrecks`]), written
/// back ([`crate::car::Parked::pushed`]), and every car the plane's rays struck
/// ([`crate::flight::Met::hit`]) or a wreck bike's (`struck`) woken. Split screen's shared copy (F75a)
/// as the Sim's own ([`crate::Sim::parked_from_plane`]).
pub fn cars_from_plane(parked_cars: &mut [crate::car::Parked], met: &crate::flight::Met, struck: &[bool]) {
    for (k, p) in parked_cars.iter_mut().enumerate() {
        if let Some(w) = met.wrecks.get(k) {
            p.pushed(&crate::wreck_plane::flown_of(w));
        }
        if met.hit.get(k).is_some_and(|h| !h.is_empty()) || struck.get(k).copied().unwrap_or(false) {
            p.struck();
        }
    }
}
