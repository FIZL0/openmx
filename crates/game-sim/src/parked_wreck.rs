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
        if !down(r) {
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
}

impl crate::Sim {
    /// **The parked cars as a flown plane's lane meets them** (S486a,
    /// [`crate::wreck_plane::search_downed_parked`]): each one's hull where its body's node stands and
    /// its body, the copies nearest the plane at `flying`, named after the players as their hulls are
    /// in the bikes' lists.
    pub(crate) fn parked_in_plane(&self, flying: ps2_float::Vf, world: &dyn World) -> Vec<crate::wreck_plane::ParkedIn> {
        let n = self.players();
        self.parked_cars
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

    /// **What the flown plane's lane did to the parked cars**, after its wreck bikes' step (S486a): each
    /// body as the plane's resolve and the wreck bikes' left it ([`crate::flight::Met::wrecks`]),
    /// written back ([`crate::car::Parked::pushed`]), and every car the plane's rays struck
    /// ([`crate::flight::Met::hit`]) or a wreck bike's (`struck`) woken.
    pub(crate) fn parked_from_plane(&mut self, met: &crate::flight::Met, struck: &[bool]) {
        for (k, p) in self.parked_cars.iter_mut().enumerate() {
            if let Some(w) = met.wrecks.get(k) {
                p.pushed(&crate::wreck_plane::flown_of(w));
            }
            if met.hit.get(k).is_some_and(|h| !h.is_empty()) || struck.get(k).copied().unwrap_or(false) {
                p.struck();
            }
        }
    }
}
