//! **The quad's collision record's contacts** (MD75e1), ported from MX vs. ATV Unleashed's PS2 ELF
//! (SLUS-21104): what the record does with the contacts it holds, once a substep, when the vehicle's
//! gravity tells it to (the record's vt `+0x88`; [`crate::quad::Call::GravityRecord`]).
//!
//! It is MX Unleashed's resolution ([`crate::contact`]): the same five steps over the list (the
//! relative velocity; past [`crate::contact::HARD`], the impulse for the record's own contacts, else
//! the spring; the friction; the damping), instruction for instruction, on the ATV's record layout. Two
//! things differ in this build:
//!
//! - the own body's turn from an impulse is multiplied lane by lane by a quadword of the record's
//!   ([`Record::impulse_mask`]) before its inverse inertia; a chassis' (the record on a vehicle) is
//!   not, nor is the other side's;
//! - each contact keeps the relative velocity it was resolved with ([`Entry::relative`]), as well as
//!   the record's last one.
//!
//! The body is the quad's [`atv_body::Body`], whose virtuals the resolution calls: add velocity (into
//! the correction too), add rates (into the twist too), the damping and angular damping set whole, the
//! moment and the force added. Each of those also tells the body's listeners; the quad's body has none.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::atv_body;
use crate::contact::{
    Collider, Contact, HARD, Mover, Other, Own, Rigid, damp, friction_on, impulse_masked, mutual, plus, relative_velocity_on,
    spring_on,
};
use ps2_float::{F, Vf};

/// The ATV's rigid body with the two fields of it the resolution reads that the body's step does not
/// keep: the mass (`+0x18`) and the centre of mass in the body's frame (`+0x1a0`).
#[derive(Debug)]
pub struct Weighed<'a> {
    pub body: &'a mut atv_body::Body,
    pub mass: F,
    pub cofg: Vf,
}

impl Rigid for Weighed<'_> {
    fn mass(&self) -> F {
        self.mass
    }
    fn inv_mass(&self) -> F {
        self.body.inv_mass
    }
    fn damping(&self) -> Vf {
        self.body.damping
    }
    fn ang_damping(&self) -> Vf {
        self.body.ang_damping
    }
    fn position(&self) -> Vf {
        self.body.position
    }
    fn velocity(&self) -> Vf {
        self.body.velocity
    }
    fn inv_inertia(&self) -> Vf {
        self.body.inv_inertia
    }
    fn quat(&self) -> Vf {
        self.body.quat
    }
    fn cofg(&self) -> Vf {
        self.cofg
    }
    fn rates(&self) -> Vf {
        self.body.rates
    }
    fn set_damping(&mut self, d: Vf) {
        self.body.damping = d;
    }
    fn set_ang_damping(&mut self, d: Vf) {
        self.body.ang_damping = d;
    }
    fn add_velocity(&mut self, dv: Vf) {
        self.body.velocity = plus(self.body.velocity, dv);
        self.body.correction = plus(self.body.correction, dv);
    }
    fn add_rates(&mut self, dw: Vf) {
        self.body.rates = plus(self.body.rates, dw);
        self.body.twist = plus(self.body.twist, dw);
    }
    fn add_moment_local(&mut self, m: Vf) {
        self.body.moment = plus(self.body.moment, m);
    }
    fn push(&mut self, f: Vf) {
        self.body.force = plus(self.body.force, f);
    }
}

/// One contact in the record's list (`0x90` bytes each): MX Unleashed's, and at `+0x70` the relative
/// velocity it was resolved with. The ATV's entry keeps its surface at `+0x80` and the other record
/// and the mover at `+0x84` and `+0x88`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Entry {
    pub contact: Contact,
    pub relative: Vf,
}

/// The record: MX Unleashed's collision object's tuning and the three fields the resolution writes
/// ([`Collider`]), and the ATV's mask on the own body's impulse turn (`+0x150`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Record {
    pub collider: Collider,
    pub impulse_mask: Vf,
}

/// **The record's resolution** (vt `+0x88`): every contact in the list, in order, as
/// [`crate::contact::resolve`] does it, with this build's two differences (the module's doc).
/// `impulses` is the call's second argument (the gravity passes 1). The entries' `other`s and
/// `mover`s index `others` and `movers`. The damping runs only with the record on a body.
pub fn resolve(
    own: &mut Record,
    side: &mut Own<'_>,
    entries: &mut [Entry],
    others: &mut [Other<'_>],
    movers: &[Mover],
    impulses: bool,
) {
    let mask = own.impulse_mask;
    let own = &mut own.collider;
    for entry in entries.iter_mut() {
        let e = &mut entry.contact;
        let mut other = e.other.map(|k| &mut others[k]);
        let mover = e.mover.map(|k| &movers[k]);
        relative_velocity_on(own, side, e, other.as_deref_mut(), mover);
        entry.relative = own.relative;
        let shared = mutual(own, other.as_deref().map(|o: &Other<'_>| &*o.collider));
        if own.normal_speed < HARD {
            if impulses && e.kind == 0 {
                impulse_masked(own, side, e, other.as_deref_mut(), shared, Some(mask));
            }
        } else {
            spring_on(own, side, e, other.as_deref_mut(), mover.is_some(), shared);
        }
        friction_on(own, side, e, other.as_deref().map(|o: &Other<'_>| &*o.collider));
        if own.damp
            && let Own::Body(body) = side
        {
            damp(own, &mut **body, e);
        }
    }
}

/// **What the quad keeps of its record** for [`crate::quad::Steps`] to answer the gravity's call
/// ([`crate::quad::Bike::contacts`]): the record, its list, the movers the list points at, and the
/// body's mass. While a listener runs, `body` is the frame's whole rigid body (the listener's view of
/// it lacks what the resolution reads: the damping, the inverse inertia and mass, the twist).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Held {
    pub record: Record,
    pub entries: Vec<Entry>,
    pub movers: Vec<Mover>,
    pub mass: F,
    pub body: Option<atv_body::Body>,
}

impl Held {
    /// **The list refilled from the record's pass** (MD98, MD105b): the contacts
    /// [`crate::handling::record::pass`] filed this frame become the entries, each as the pass leaves it in
    /// the list (its kind, ray, normal, point and surface word; both arms [`crate::handling::record::BLANK`]).
    /// An entry's relative velocity is left as the list's slot held it (the resolution writes it before it
    /// reads it). `false`, and the list left empty, if a contact is with another record or a mover, which
    /// the held record cannot answer ([`Held::resolve`]).
    pub fn refill(&mut self, filed: &[crate::handling::record::Contact]) -> bool {
        let was = std::mem::take(&mut self.entries);
        if filed.iter().any(|c| c.other != 0 || c.mover != 0) {
            return false;
        }
        let blank = crate::handling::record::BLANK;
        self.entries = filed
            .iter()
            .enumerate()
            .map(|(i, c)| Entry {
                contact: Contact {
                    kind: c.kind,
                    from: c.from,
                    on_body: c.to,
                    normal: c.normal,
                    point: c.point,
                    arm: blank,
                    other_arm: blank,
                    surface: c.surface as i32,
                    other: None,
                    mover: None,
                },
                relative: was.get(i).map_or(Vf::ZERO, |e| e.relative),
            })
            .collect();
        true
    }

    /// The record's resolution on the quad's body, handed 1 as the gravity hands it, with `cofg` the
    /// body's centre of mass in its frame. `false`, and nothing done, if a contact is with another
    /// record (none is held) or the whole body is not in.
    pub fn resolve(&mut self, cofg: Vf) -> bool {
        if self.entries.iter().any(|e| e.contact.other.is_some()) {
            return false;
        }
        let Some(body) = self.body.as_mut() else { return false };
        let mut me = Weighed { body, mass: self.mass, cofg };
        resolve(&mut self.record, &mut Own::Body(&mut me), &mut self.entries, &mut [], &self.movers, true);
        true
    }
}
