//! **The seats share one prop set** (S197c): online and split screen ride one-player Sims, a seat
//! each, and each Sim carries a copy of the track's knockable props ([`crate::Sim::traffic`]). Struck
//! in their own seats (S197a), a ball one seat knocked stood still in every other seat's world, and
//! the shell draws seat 0's copy only, so a ball player 2 knocked did not move on screen.
//!
//! So for the tick the seats hold one set, as [`crate::Sim::step_all`]'s one Sim of several riders
//! does: [`Shared::take`] lifts it out of seat 0 (the other seats' copies are dropped), every seat's
//! bike step is lent it for its strike and frame ([`crate::Sim::step_shared`], its mirrors naming
//! the striker by its seat), the first of them runs its collide pass in its traffic's place, and
//! [`Shared::finish`] steps it once with every seat's chassis and wreck bike as the strikers — by
//! seat, then each wreck bike `seats + seat`, as `step_all`'s props step reads its players — and puts
//! it back in every seat. A seat flying the plane (S259) is lent the set as a bike's seat is
//! ([`Shared::lend`], [`Shared::back`]): its plane's rays strike the set and the set's its hull
//! ([`crate::flight::Flight::frame_props`] naming the plane by its seat), and in the set's step the
//! plane, not the bike it left, is that seat's striker, as [`crate::Sim::step`]'s plane lane has it.
//! Every seat's wreck bike is on the props' node (S260), as a Sim's riders' are (S243,
//! [`crate::Sim::frame_held_wrecked`]): [`Shared::take`] keeps each downed seat's hull where it stands
//! before any seat steps, the set's collide pass has them all in every prop's world, and each seat's
//! step hands its wreck bike what the props' rays struck on it ([`Shared::wreck_struck`]).
//! Between ticks every seat holds the same set, so the hash, the snapshot and
//! a rollback's copy carry it as they did, and seat 0's is the one the shell draws.

use crate::Sim;
use crate::biplane::Body;
use crate::flight::Flight;
use crate::contact::{Collider, Other, Side, Weighed};
use crate::phy::Phy;
use crate::math::Mat4;
use crate::prop_set::{PropMovers, PropSet};
use crate::prop_world::Wreck;
use crate::prop_wreck::WreckStrike;
use crate::ragdoll_scene::Bike;
use crate::rider::Rider;
use crate::world::World;
use ps2_float::{F, Vf};
use std::sync::Arc;

/// The seats' one prop set, for a tick: [`Shared::take`], each seat's [`crate::Sim::step_shared`],
/// then [`Shared::finish`].
pub struct Shared {
    set: PropSet,
    /// The collide pass's movers, once the first seat stepped has run it.
    movers: Option<PropMovers>,
    seats: usize,
    /// Each downed seat's wreck bike as a hull on the props' node, as the tick found it
    /// ([`crate::Sim::wreck_hull`]): its seat, mesh, pose and velocity.
    wrecks: Vec<(usize, Arc<Bike>, Mat4, Vf)>,
    /// What the collide pass's props struck on each seat's wreck bike, by seat, until its step takes it.
    struck: Vec<Option<WreckStrike>>,
}

impl Shared {
    /// The props out of the seats for the tick: seat 0's set, the others' copies dropped. `None` with
    /// fewer than two seats (one steps as it always has) or no props on the track.
    pub fn take(sims: &mut [&mut Sim]) -> Option<Shared> {
        if sims.len() < 2 || sims[0].traffic.props.is_empty() {
            return None;
        }
        let mut set = std::mem::take(&mut sims[0].traffic.props);
        // Every seat's bike, by seat, as the props' probes hear it (F27a2, `Sim::hear_ridden`).
        let ridden: Vec<crate::flight_search::Bike> = sims
            .iter()
            .map(|s| crate::flight_search::Bike { transform: s.rider.ride.transform(), in_world: s.plane.is_none() && s.rider.state.wreck.is_none() && !s.rider.ghost.on })
            .collect();
        set.hear_ridden(&ridden);
        set.file_ridden(true);
        for s in sims.iter_mut().skip(1) {
            s.traffic.props = PropSet::default();
        }
        let wrecks = sims
            .iter()
            .enumerate()
            .filter(|(_, s)| s.plane.is_none())
            .filter_map(|(seat, s)| s.wreck_hull(0).map(|(mesh, at, velocity)| (seat, mesh, at, velocity)))
            .collect();
        Some(Shared { set, movers: None, seats: sims.len(), wrecks, struck: vec![None; sims.len()] })
    }



    /// How many seats share the set.
    pub fn seats(&self) -> usize {
        self.seats
    }

    /// A seat's held traffic frame ([`crate::traffic::Traffic::frame_held`]) with the set lent to it:
    /// the first seat's runs the set's collide pass with every seat's wreck bike on the node
    /// ([`crate::traffic::Traffic::frame_held_among`]), its movers kept for [`Shared::finish`] and
    /// its strikes for each seat's wreck bike ([`Shared::wreck_struck`]); a later seat's runs on its
    /// own empty set and is lent the set after. Answers the movers of the seat's own props, which have
    /// none.
    pub(crate) fn held(&mut self, sim: &mut Sim, dt: F, world: &mut dyn World) -> PropMovers {
        if self.movers.is_none() {
            std::mem::swap(&mut sim.traffic.props, &mut self.set);
            let wrecks = on_node(&self.wrecks, self.seats);
            let (movers, strikes) = sim.traffic.frame_held_among(dt, world, &wrecks);
            drop(wrecks);
            for ((seat, ..), strike) in self.wrecks.iter().zip(strikes) {
                self.struck[*seat] = Some(strike);
            }
            self.movers = Some(movers);
            return PropMovers::default();
        }
        let movers = sim.traffic.frame_held(dt, world);
        std::mem::swap(&mut sim.traffic.props, &mut self.set);
        movers
    }

    /// The set lent to a seat that flies the plane (S259), in its traffic's props for the plane's
    /// frame: [`Shared::held`], the seat's own movers dropped, as its own set is empty.
    pub fn lend(&mut self, sim: &mut Sim, dt: F, world: &mut dyn World) {
        let _ = self.held(sim, dt, world);
    }

    /// The set itself, for planes flown together to strike (S259a,
    /// [`crate::flight::frames_together_props`]): no seat's traffic holds it then.
    pub fn set_mut(&mut self) -> &mut PropSet {
        &mut self.set
    }

    /// A flying seat's traffic before its plane flies with the others (S259a,
    /// [`crate::flight::frames_together_props`]): [`crate::traffic::Traffic::frame`] on its own
    /// empty set, the set's collide pass if no seat ran it yet ([`Shared::held`]), and the set
    /// kept here for the planes' frame.
    pub fn traffic(&mut self, sim: &mut Sim, dt: F, world: &mut dyn World) {
        self.lend(sim, dt, world);
        self.back(sim);
        sim.traffic.frame_late(dt, world);
    }

    /// **What the props' rays struck on seat `seat`'s wreck bike** in the collide pass, kept on the
    /// bike for its frame ([`crate::wreck_bike::WreckBike::struck`]), as
    /// [`crate::Sim::frame_held_wrecked`] keeps it. Nothing for a seat whose rider is up.
    pub fn wreck_struck(&mut self, sim: &mut Sim, seat: usize) {
        let Some(strike) = self.struck.get_mut(seat).and_then(Option::take) else { return };
        if let Some(crash) = sim.rider.crash.as_mut() {
            crash.bike.struck = strike;
        }
    }

    /// The set back from the seat [`Shared::held`] or [`Shared::lend`] lent it to.
    pub fn back(&mut self, sim: &mut Sim) {
        std::mem::swap(&mut sim.traffic.props, &mut self.set);
    }

    /// **The set's step**, after every seat's: the collide pass if no seat's bike ran it, then the
    /// step with every seat's chassis as it stands, by seat, and every seat's wreck bike after them,
    /// which a prop's impulse pushes, written back ([`crate::Sim::props_step`] over the seats). Then
    /// the set in every seat. A seat whose [`Sim::plane`] flies strikes it with its plane
    /// ([`Shared::finish_flying`]).
    pub fn finish(self, sims: &mut [&mut Sim], dt: F, world: &mut dyn World) {
        let mut lanes: Vec<Option<Flight>> = sims.iter_mut().map(|s| s.plane.take()).collect();
        self.finish_flying(sims, lanes.iter_mut().map(Option::as_mut).collect(), dt, world);
        for (s, f) in sims.iter_mut().zip(lanes) {
            s.plane = f;
        }
    }

    /// [`Shared::finish`] with each seat's flown plane, by seat (`None` on its bike): in the set's
    /// step a flying seat's striker is its plane's hull and body (S259), pushed as a wreck bike's
    /// body is, in its seat's place, as [`crate::Sim::step`]'s plane lane has the plane striker 0.
    /// The split-screen shell holds its planes beside its Sims, so it hands them here.
    pub fn finish_flying(mut self, sims: &mut [&mut Sim], mut flights: Vec<Option<&mut Flight>>, dt: F, world: &mut dyn World) {
        let movers = match self.movers.take() {
            Some(m) => m,
            None => {
                // No seat's frame is left to take the wreck bikes' strikes; the props' entries on
                // them are still the pass's.
                let wrecks = on_node(&self.wrecks, self.seats);
                let (movers, _) = self.set.search_in_among(dt, world, &wrecks);
                drop(wrecks);
                movers
            }
        };
        let riders: Vec<(&Rider, &Phy)> = sims.iter().map(|s| (&s.rider, &*s.phy)).collect();
        let mut bump = crate::rider_contact::bump_here(&riders, 0, Vec::new());
        drop(riders);
        let sides: Vec<Option<(Collider, Body, F)>> = sims.iter().map(|s| s.wreck_sides()[0]).collect();
        let mut wrecks: Vec<(Collider, Body, F)> = sides.iter().map(|s| s.unwrap_or_default()).collect();
        let mut bodies: Vec<(&mut Collider, Weighed<'_>)> = wrecks.iter_mut().map(|(c, body, mass)| (c, Weighed { body, mass: *mass })).collect();
        flights.resize_with(sims.len(), || None);
        let mut planes: Vec<Option<(&mut Collider, Weighed<'_>)>> = flights
            .iter_mut()
            .map(|f| f.as_deref_mut().map(|f| (&mut f.collider, Weighed { body: &mut f.body, mass: f.mass })))
            .collect();
        let mut others: Vec<Other<'_>> = bump
            .colliders
            .iter_mut()
            .zip(bump.vehicles.iter_mut())
            .zip(planes.iter_mut())
            .map(|((collider, v), plane)| match plane {
                Some((collider, body)) => Other { collider: &mut **collider, side: Side::Body(body) },
                None => Other { collider, side: Side::Vehicle(v) },
            })
            .chain(bodies.iter_mut().map(|(collider, body)| Other { collider: &mut **collider, side: Side::Body(body) }))
            .collect();
        self.set.step_with(dt, &movers, &mut others);
        drop(others);
        drop(planes);
        drop(bodies);
        for ((s, side), w) in sims.iter_mut().zip(&sides).zip(wrecks) {
            s.wrecks_pushed(&[side.map(|_| w)]);
        }
        for s in sims.iter_mut().skip(1) {
            s.traffic.props.clone_from(&self.set);
        }
        sims[0].traffic.props = self.set;
    }
}

/// The kept wreck bikes ([`Shared::take`]) as the props' world holds them, each named `seats + seat`
/// on the props' entries, as [`Shared::finish`]'s step reads it.
fn on_node(wrecks: &[(usize, Arc<Bike>, Mat4, Vf)], seats: usize) -> Vec<Wreck<'_>> {
    wrecks
        .iter()
        .map(|(seat, mesh, at, velocity)| Wreck {
            hull: crate::flight_search::Plane { mesh: &mesh.mesh, at: *at, velocity: *velocity },
            side: seats + seat,
        })
        .collect()
}
