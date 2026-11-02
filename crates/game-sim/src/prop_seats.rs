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
//! it back in every seat. Between ticks every seat holds the same set, so the hash, the snapshot and
//! a rollback's copy carry it as they did, and seat 0's is the one the shell draws.

use crate::Sim;
use crate::biplane::Body;
use crate::contact::{Collider, Other, Side, Weighed};
use crate::phy::Phy;
use crate::prop_set::{PropMovers, PropSet};
use crate::rider::Rider;
use crate::world::World;
use ps2_float::F;

/// The seats' one prop set, for a tick: [`Shared::take`], each seat's [`crate::Sim::step_shared`],
/// then [`Shared::finish`].
pub struct Shared {
    set: PropSet,
    /// The collide pass's movers, once the first seat stepped has run it.
    movers: Option<PropMovers>,
    seats: usize,
}

impl Shared {
    /// The props out of the seats for the tick: seat 0's set, the others' copies dropped. `None` with
    /// fewer than two seats (one steps as it always has) or no props on the track.
    pub fn take(sims: &mut [&mut Sim]) -> Option<Shared> {
        if sims.len() < 2 || sims[0].traffic.props.is_empty() {
            return None;
        }
        let set = std::mem::take(&mut sims[0].traffic.props);
        for s in sims.iter_mut().skip(1) {
            s.traffic.props = PropSet::default();
        }
        Some(Shared { set, movers: None, seats: sims.len() })
    }

    /// How many seats share the set.
    pub fn seats(&self) -> usize {
        self.seats
    }

    /// A seat's held traffic frame ([`crate::traffic::Traffic::frame_held`]) with the set lent to it:
    /// the first seat's runs the set's collide pass, kept for [`Shared::finish`]; a later seat's runs
    /// on its own empty set and is lent the set after. Answers the movers of the seat's own props,
    /// which have none.
    pub(crate) fn held(&mut self, sim: &mut Sim, dt: F, world: &mut dyn World) -> PropMovers {
        if self.movers.is_none() {
            std::mem::swap(&mut sim.traffic.props, &mut self.set);
            self.movers = Some(sim.traffic.frame_held(dt, world));
            return PropMovers::default();
        }
        let movers = sim.traffic.frame_held(dt, world);
        std::mem::swap(&mut sim.traffic.props, &mut self.set);
        movers
    }

    /// The set back from the seat [`Shared::held`] lent it to.
    pub(crate) fn back(&mut self, sim: &mut Sim) {
        std::mem::swap(&mut sim.traffic.props, &mut self.set);
    }

    /// **The set's step**, after every seat's: the collide pass if no seat's bike ran it, then the
    /// step with every seat's chassis as it stands, by seat, and every seat's wreck bike after them,
    /// which a prop's impulse pushes, written back ([`crate::Sim::props_step`] over the seats). Then
    /// the set in every seat.
    pub fn finish(mut self, sims: &mut [&mut Sim], dt: F, world: &mut dyn World) {
        let movers = match self.movers.take() {
            Some(m) => m,
            None => self.set.search_in(dt, world),
        };
        let riders: Vec<(&Rider, &Phy)> = sims.iter().map(|s| (&s.rider, &*s.phy)).collect();
        let mut bump = crate::rider_contact::bump_here(&riders, 0, Vec::new());
        drop(riders);
        let sides: Vec<Option<(Collider, Body, F)>> = sims.iter().map(|s| s.wreck_sides()[0]).collect();
        let mut wrecks: Vec<(Collider, Body, F)> = sides.iter().map(|s| s.unwrap_or_default()).collect();
        let mut bodies: Vec<(&mut Collider, Weighed<'_>)> = wrecks.iter_mut().map(|(c, body, mass)| (c, Weighed { body, mass: *mass })).collect();
        let mut others: Vec<Other<'_>> = bump
            .colliders
            .iter_mut()
            .zip(bump.vehicles.iter_mut())
            .map(|(collider, v)| Other { collider, side: Side::Vehicle(v) })
            .chain(bodies.iter_mut().map(|(collider, body)| Other { collider: &mut **collider, side: Side::Body(body) }))
            .collect();
        self.set.step_with(dt, &movers, &mut others);
        drop(others);
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
