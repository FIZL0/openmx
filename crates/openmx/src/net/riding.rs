//! **The riding ragdoll online** (P133): the riding rider drawn from the riding ragdoll, as offline
//! (S90), off a **drawn riding ragdoll** the shell steps beside the session, for the picture only.
//!
//! Offline the rider's own tick steps its riding ragdoll ([`game_sim::rider::Rider::riding`]), held
//! to the grips of the bike the shell last drew ([`game_sim::rider::Rider::holds`]) and pulled to the
//! skeleton the shell posed. Online every machine must step the same state, and those two are the
//! renderer's: the chassis' clip blend and the body's pose are not in the session's state, so a seat's
//! drawn bike can differ between machines and over a rollback. The session's riders get neither and
//! run no riding ragdoll, as before, so its state and its hash are untouched.
//!
//! The drawn riding ragdoll is the riding ragdoll of each shell seat, stepped once a tick the session moved on,
//! on the seat as shown (its last tick run, a prediction where pads are still on their way), with
//! the drawn holds and this tick's skeleton, and handed to the shown rider for the picture. It is
//! never rolled back: a re-run moves the bike, and a move past the riding ragdoll's
//! [`game_sim::ragdoll_riding::STRAY`] starts it afresh, as a teleport does offline.

use std::sync::Arc;

use game_sim::math::Mat4;
use game_sim::ragdoll_ride::Posed;
use game_sim::ragdoll_riding::{Holds, Riding};
use game_sim::rider::Rider;
use game_sim::world::World;
use ps2_float::F;

/// Every shell seat's [`Seat`], by shell seat.
#[derive(Default)]
pub struct DrawnRiding {
    seats: Vec<Seat>,
}

/// One shell seat's riding ragdoll beside the session.
#[derive(Default)]
struct Seat {
    riding: Option<Box<Riding>>,
    /// The session tick the seat was last shown at.
    tick: Option<u64>,
    /// What the next step reads off the top of its tick, as the rider's own riding tick does: the
    /// bike's transform, the vehicle animation's clip-table words and its clip, as last shown.
    from: Option<(Mat4, (bool, bool), i32)>,
    /// The grips of the bike as last drawn.
    holds: Option<Arc<Holds>>,
}

impl DrawnRiding {
    /// Shell seat `d` shown at session tick `tick`, its rider `rider` (a copy of its session seat):
    /// its riding ragdoll stepped once if the tick moved on, `dt` long, held to the grips last drawn and
    /// pulled to `skeleton` (the body as posed this tick), then handed to `rider` to draw. Only
    /// while the rider is up and has a kit, as the rider's own riding tick runs.
    pub fn show(&mut self, d: usize, tick: u64, rider: &mut Rider, skeleton: Option<Arc<Posed>>, dt: F, world: &mut dyn World) {
        if self.seats.len() <= d {
            self.seats.resize_with(d + 1, Seat::default);
        }
        let s = &mut self.seats[d];
        let moved = s.tick != Some(tick);
        let up = rider.state.wreck.is_none() && rider.crash.is_none() && rider.ragdoll.is_none();
        match (rider.ragdoll_kit.clone(), up) {
            (Some(kit), true) if moved => {
                if let (Some(holds), Some((before, table, clip))) = (s.holds.clone(), s.from) {
                    let mut riding = s.riding.take().filter(|r| !r.strayed(before.0[3])).unwrap_or_else(|| Box::new(Riding::new(&kit)));
                    rider.skeleton = skeleton;
                    riding.ride(&kit, rider, &holds, &before, table, clip, dt, world);
                    s.riding = Some(riding);
                }
            }
            (Some(_), true) => {}
            _ => s.riding = None,
        }
        if moved {
            let veh = &rider.body.veh;
            s.from = Some((rider.ride.transform(), (veh.from_table, veh.holding), veh.state));
            s.tick = Some(tick);
        }
        rider.riding = s.riding.clone();
    }

    /// Shell seat `d`'s bike as just drawn: its grips, which its next step is held to.
    pub fn hold(&mut self, d: usize, holds: Option<Arc<Holds>>) {
        if let Some(s) = self.seats.get_mut(d) {
            s.holds = holds;
        }
    }
}
