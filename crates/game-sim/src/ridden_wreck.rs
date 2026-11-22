//! **A downed rider's wreck bike meets the riding bikes** (S502): in a ride of several riders the
//! wreck bike tumbles on into another rider's bike, and its rays strike that bike's frame.
//!
//! S134 proved the wreck bike's tick with the riders' bikes in its world
//! ([`WreckBike::tick_ridden`], S93's short split-screen ride, where P2's wreck bike strikes P1's
//! bike): its entries name the struck rider's record, that record takes the kind-1 mirrors, and the
//! wreck bike's world steps before the riders' do, every rider's chassis as the frame before left it.
//!
//! So each downed rider's wreck bike with a riding bike in reach (within [`REACH`] past the bike's
//! frame) is ticked among the riders' bikes just before its rider's frame, where the parked cars'
//! lane ticks it ([`crate::parked_wreck`]), and its rider's frame then skips its tick
//! ([`WreckBike::flown`]). One the parked cars' or a flown plane's lane ticked already this tick is
//! left as that lane left it. A wreck bike out of every bike's reach is ticked by its rider's frame
//! as before.
//!
//! The mirrors go on the struck rider's list for its frame ([`file_struck`]), ahead of its own
//! entries, as the flown planes' do ([`crate::rider_contact::struck_first`]): the original's struck
//! record holds them after its own frame. Each names the wreck bike's body, a copy among the other
//! sides after the woken cars' ([`crate::rider_contact::Bump::cars`]); a kind-1 mirror pushes no
//! other side, so nothing is written back.

use crate::contact::Contact;
use crate::flight_search::{Bike, Bikes};
use crate::prop_contact::Pushed;
use crate::rider::Rider;
use crate::rider_contact::{Bump, CAPACITY};
use crate::wreck_bike::{Riders, WreckBike};
use crate::world::World;
use ps2_float::{F, Vf};

/// How far past a riding bike's frame a wreck bike's search may reach it: its rays reach a few feet
/// past its own sphere, well inside this.
pub const REACH: F = F(20.0);

/// What one wreck bike's rays put on the riders' records: its body as the mirrors' other side, and
/// the mirrors each rider's record took, by player.
#[derive(Clone, Debug)]
pub struct Struck {
    pub by: Pushed,
    pub mirrors: Vec<Vec<Contact>>,
}

/// Whether `r`'s wreck bike is in the riders' world: the rider down, its part colliding, not a ghost
/// (whose bodies meet nobody), and not ticked already this tick by another lane.
fn down(r: &Rider) -> bool {
    r.crash.as_ref().is_some_and(|c| !c.bike.flown)
        && !r.ghost.on
        && r.wreckage.as_ref().is_some_and(|w| w.part.collision_enabled)
}

impl crate::Sim {
    /// **Player `i`'s wreck bike among the riding bikes**, ahead of the rider's frame (the module's
    /// comment): every other player's bike where it stands nearest the wreck bike, in the world while
    /// its rider is up, no ghost and carries the bike's collision files. Nothing for a rider who is
    /// up, or a wreck bike out of every bike's reach.
    pub fn wreck_meets_riders(&mut self, i: usize, dt: F, world: &mut dyn World) -> Option<Struck> {
        let n = self.players();
        if n < 2 || !down(self.rider_of(i)) {
            return None;
        }
        let from = self.rider_of(i).crash.as_ref()?.bike.origin();
        let bikes: Vec<Bike> = (0..n)
            .map(|j| {
                let r = self.rider_of(j);
                let mut transform = r.ride.transform();
                transform.0[3] = crate::nearest_copy(&*world, from, transform.0[3]);
                Bike { transform, in_world: j != i && r.state.wreck.is_none() && !r.ghost.on && r.bike_col.is_some() }
            })
            .collect();
        let near = bikes.iter().enumerate().filter(|(_, b)| b.in_world).find_map(|(j, b)| {
            let col = self.rider_of(j).bike_col.as_ref()?;
            (!(crate::actor::distance(b.transform.0[3], from) > col.frame_radius + REACH)).then(|| col.clone())
        });
        let col = near?;
        // Every chassis and record object as the frame before left them, the chassis nearest the
        // wreck bike.
        let mut bump = self.bump(i, Vec::new());
        for (_, v) in bump.vehicles.iter_mut().enumerate().filter(|&(j, _)| j != i) {
            v.node.origin = crate::nearest_copy(&*world, from, v.node.origin);
        }
        let (ground, props, trees) = world.wooded_near(from);
        let rider = if i == 0 { &mut self.rider } else { &mut self.others[i - 1].rider };
        let bike = &mut rider.crash.as_mut()?.bike;
        let riders = Riders { bikes: Bikes { mesh: &col.frame, bikes: &bikes }, chassis: &mut bump.vehicles, colliders: &mut bump.colliders };
        let mirrors = bike.tick_ridden(dt, ground.as_deref(), props, trees, riders);
        bike.flown = true;
        Some(Struck { by: pushed(bike, i), mirrors })
    }
}

/// The wreck bike of player `i` as the other side of its mirrors: its collision object and body,
/// a copy. Its collection is no car's ([`crate::Sim::cars_probed`] looks the cars up by theirs).
fn pushed(bike: &WreckBike, i: usize) -> Pushed {
    Pushed { collection: usize::MAX, object: i, collider: bike.collider, body: bike.body, mass: bike.mass }
}

/// **The wreck bikes' mirrors on player `me`'s list** (the module's comment): each wreck bike's body
/// among the other sides after the woken cars', where the bike stands nearest `at` on a world that
/// wraps, and its mirrors ahead of the list's own entries, naming it and moved to the copy nearest
/// `at`, up to the record's [`CAPACITY`]. Called once the list's other sides are all in.
pub fn file_struck(bump: &mut Bump, me: usize, struck: &[Struck], at: Vf, world: &dyn World) {
    let mut list = Vec::new();
    for s in struck {
        let Some(mirrors) = s.mirrors.get(me).filter(|m| !m.is_empty()) else { continue };
        let k = bump.vehicles.len() + bump.planes.len() + bump.props.len() + bump.cars.len();
        let mut by = s.by.clone();
        let near = crate::nearest_copy(world, at, by.body.position);
        if near != by.body.position {
            let shift = near.sub(by.body.position);
            by.body.position = near;
            by.body.last_position = by.body.last_position.add(shift);
        }
        bump.cars.push(by);
        list.extend(mirrors.iter().map(|&c| Contact { point: crate::nearest_copy(world, at, c.point), other: Some(k), ..c }));
    }
    if list.is_empty() {
        return;
    }
    list.append(&mut bump.contacts);
    list.truncate(CAPACITY);
    bump.contacts = list;
}

/// Whether any of `struck` put a mirror on player `me`'s record.
pub fn struck_on(struck: &[Struck], me: usize) -> bool {
    struck.iter().any(|s| s.mirrors.get(me).is_some_and(|m| !m.is_empty()))
}
