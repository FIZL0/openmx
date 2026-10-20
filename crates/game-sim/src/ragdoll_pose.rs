//! **Where the ragdoll's bones start from**: each bone body's start state, and the reset that puts
//! the body back at it.
//!
//! A bone body keeps a start beside its state: a position and a velocity, set together, and an
//! orientation and its rates, set together. The pose (the body actor's own virtual, which reads the
//! skeleton's world pose; not ported, see the S52 journal) writes the start; the reset, the other of
//! the two virtuals both ragdoll entries begin with, copies it into every body of the ragdoll's world
//! and empties everything the last steps accumulated.

use crate::biplane::Body;
use ps2_float::Vf;

/// What a reset leaves in every accumulator: the original copies its constant `(0, 0, 0, 1)`.
const EMPTY: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);

/// **A bone body's start**, as its two setters write it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Start {
    pub position: Vf,
    pub velocity: Vf,
    pub quat: Vf,
    pub rates: Vf,
}

impl Start {
    /// The first setter: the position and the velocity, whole quadwords.
    pub fn place(&mut self, position: Vf, velocity: Vf) {
        self.position = position;
        self.velocity = velocity;
    }

    /// The second: the orientation and the rates.
    pub fn turn(&mut self, quat: Vf, rates: Vf) {
        self.quat = quat;
        self.rates = rates;
    }

    /// **The reset of one body**: at its start, both this substep's and the last one's position and
    /// orientation, and every force, moment and acceleration emptied.
    pub fn reset(&self, body: &mut Body) {
        body.last_position = self.position;
        body.position = self.position;
        body.velocity = self.velocity;
        body.force = EMPTY;
        body.last_force = EMPTY;
        body.shock_peak = EMPTY;
        body.shock = EMPTY;
        body.accel = EMPTY;
        body.last_quat = self.quat;
        body.quat = self.quat;
        body.rates = self.rates;
        body.moment = EMPTY;
        body.last_moment = EMPTY;
        body.ang_accel = EMPTY;
        body.twist_peak = EMPTY;
        body.twist = EMPTY;
    }
}

/// **The ragdoll's reset**: every body of its world, in the world's order, back at its start.
pub fn reset<'a>(bodies: impl IntoIterator<Item = (&'a mut Body, &'a Start)>) {
    for (body, start) in bodies {
        start.reset(body);
    }
}
