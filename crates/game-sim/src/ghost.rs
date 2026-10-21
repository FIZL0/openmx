//! **The bike's ghost after a put-back** (P57, S63): the bike, its parts and the rider drawn
//! see-through and faded back in to solid over three seconds — and, for as long as it lasts, a bike
//! that the other riders pass straight through.
//!
//! Every put-back of the "after a wreck" kind starts it — the pad's L1+R1 and the wreck's own respawn
//! alike, which are the only two the port has: the respawn's model setup turns the see-through flag
//! on, sets each model's alpha to a half and the clock to its length, 3 s, the rider's constructor's
//! value and never read from a file. Then, once a tick at the head of the bike's update and so before
//! anything in it can put the bike back again: the clock runs down by the tick's `dt` and the alpha is
//! the share of the clock gone, mapped onto `0.1..1` — 0.11 on the next tick, a hundredth more each
//! tick after at 30 Hz, 0.9999 on the ninetieth — until the clock is spent, when the flag goes off and
//! the bike is solid. A put-back while it runs starts it again. The plane has none of it (S57).
//!
//! **The collision half** (S63). The same put-back adds the vehicle class to the ignore lists of both
//! of the rider's collision bodies, and the clock's end takes it off them again. The original's pair
//! filter rejects a pair when *either* body's list (in its exclude mode) holds the other's class, so
//! while the ghost runs it neither strikes another rider nor is struck by one: in the port, its head
//! ray sweeps the world alone, and it is no [`Rival`] to anybody else's ([`crate::Sim::step_all`]).
//!
//! And the clock stands still while the ghost is inside another machine. Each tick, before counting,
//! the ghost's step flips its body's list to *include* mode for one overlap query and back, so the
//! query sees vehicles and nothing else, and the clock counts only when it found none. In include
//! mode the other body's own list still has its say: another ghost's list rejects this one, so two
//! ghosts inside each other do not hold each other's clocks. What the query finds the port asks of
//! [`Rival`]'s bodies — see [`overlaps`] for the shape.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::hash::{State, Walk};
use crate::ride::Rival;
use ps2_float::{Acc, F};

/// The ghost of one rider's bike.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ghost {
    /// Seconds left, and below zero once it has run out; the original never puts it back to zero.
    pub clock: F,
    /// The alpha the models are drawn at while [`Ghost::on`].
    pub alpha: F,
    /// The see-through flag, and with it the vehicle class on the bodies' ignore lists: the two go
    /// on together at the put-back and off together on the clock's last tick.
    pub on: bool,
}

impl Ghost {
    /// How long the fade takes, seconds.
    pub const LENGTH: F = F(3.0);
    /// The alpha the put-back sets, drawn on its own tick.
    pub const START: F = F(0.5);
    /// The alpha the fade starts from and never goes under.
    pub const FLOOR: F = F(0.1);

    /// A put-back: see-through, at half, with the whole clock to run, and passing through the other
    /// machines.
    pub fn start(&mut self) {
        *self = Ghost { clock: Self::LENGTH, alpha: Self::START, on: true };
    }

    /// One tick of the bike's update, before anything in it. `inside` is the overlap query: asked
    /// only while the clock runs, as the original queries only then, and the clock does not count
    /// on a tick it answers yes.
    pub fn step(&mut self, dt: F, inside: impl FnOnce() -> bool) {
        if !(self.clock > F::ZERO) {
            return;
        }
        if !inside() {
            self.clock -= dt;
        }
        if self.clock > F::ZERO {
            let len = Self::LENGTH;
            // A length of zero holds the floor; the original tests it, though its length is fixed.
            self.alpha = if len == F::ZERO {
                Self::FLOOR
            } else {
                let gone = (self.clock - len) / (F::ZERO - len);
                let span = F(1.0) - Self::FLOOR;
                Acc::adda(F::ZERO, Self::FLOOR)
                    .madd(gone, span)
                    .max(Self::FLOOR.min(F(1.0)))
                    .min(Self::FLOOR.max(F(1.0)))
            };
        } else {
            self.on = false;
        }
    }

    /// What to draw the bike and the rider at: below 1 while the ghost is on, 1 otherwise.
    pub fn shown(&self) -> f32 {
        if self.on { self.alpha.0 } else { 1.0 }
    }
}

/// Whether the body `me` is inside any of `others`: the ghost's overlap query.
///
/// **The shape is the port's own**, [`Rival`]'s sphere (S54): the original asks the two collidables,
/// whose shapes come out of the bikes' models, which the port does not load. Two spheres overlap when
/// their centres are strictly closer than the sum of their radii — squared both sides, through the
/// FPU's accumulator, so no square root decides it. Spheres that only touch do not hold the clock.
pub fn overlaps(me: &Rival, others: &[Rival]) -> bool {
    others.iter().any(|o| {
        let d = me.centre.sub(o.centre);
        let apart = Acc::mula(d.x(), d.x()).madda(d.y(), d.y()).madd(d.z(), d.z());
        let reach = me.radius + o.radius;
        apart < reach * reach
    })
}

impl State for Ghost {
    fn walk(&self, w: &mut Walk<'_>) {
        w.f("clock", self.clock);
        w.f("alpha", self.alpha);
        w.bool("on", self.on);
    }
}
