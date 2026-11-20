//! **The parked plane in the riders' world** (S494): free10's biplane, parked with nobody aboard
//! (F80), as the bikes meet it.
//!
//! S494's captures (`research/s494/index.md`): the original's bike ridden into free10's parked plane
//! stops dead on its hull, and the plane's body, awake all along, takes the push (v 0.02 to 1.35 ft/s
//! on the strike, then slows on the ground); free06's helicopter the same. Both fliers' sleep
//! thresholds are never met (R52), so the original steps them every frame.
//!
//! So a parked plane is a [`Flight`] nobody flies ([`crate::biplane::Attitude::flown`] off), held in
//! [`crate::Sim::parked_planes`]: stepped every frame ahead of the bikes, after the parked cars
//! ([`Flight::frame`] on a pad that holds nothing), its hull in the bikes' list after the cars'
//! ([`Flight::hull`]), and its body on each bike's resolve after theirs ([`Flight::flown`]), written
//! back after the bike's frame ([`Flight::pushed`]). A bike's strike wakes nothing: the plane never
//! sleeps.

use crate::flight::Flight;
use crate::world::World;
use ps2_float::F;

impl crate::Sim {
    /// **The map's parked planes as the riders' bikes meet them** (S494, the module's comment): held
    /// in [`crate::Sim::parked_planes`], but for one without a hull or a ray probe, which no bike
    /// could meet. [`crate::Sim::parked`], the AI riders' hulls, is the caller's
    /// ([`crate::Sim::park_cars_and_fliers`]).
    pub fn park_planes(&mut self, planes: Vec<Flight>) {
        self.parked_planes = planes.into_iter().filter(|p| p.hull(0).is_some()).collect();
    }

    /// The parked planes' frame, ahead of the bikes', after the parked cars' (S494): each one stepped
    /// on a pad that holds nothing, in its own world, as the settle that parked it.
    pub(crate) fn parked_planes_frame(&mut self, dt: F, world: &mut dyn World) {
        let tables = &self.tables.ride;
        for p in &mut self.parked_planes {
            p.frame(dt, crate::biplane::Pad::default(), world, tables);
        }
    }
}
