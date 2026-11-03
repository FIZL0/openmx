//! **The wreck bike meets a flown plane** (S274): after a joust the wreck bike tumbles on under the
//! plane that threw it, and the two go on touching.
//!
//! What the original does (C16's fly tape, research/s274): the wreck bike's collision object is on the
//! same node as the plane's, and its ray probe (an empty category list) strikes the plane's hull
//! (`biplanef.col`, category [`crate::flight::COLLIDABLE`]) as it strikes the scenery. Each hit is an
//! entry (kind 0) on the wreck bike's list after its terrain entries, naming the plane's collision
//! object, and its mirror (kind 1, the normal turned round) on the plane's list after the plane's own
//! entries. The plane's own rays never strike the wreck bike's mesh (the wreck bike's sweep leaves out
//! 0x8, which the hull's category has, so only its rays meet). Both images after the joust, 137880 and
//! 137910, hold exactly that: the plane's list one mirror naming the wreck bike's object, the wreck
//! bike's two terrain entries then the entry naming the plane.
//!
//! In the port the plane's hull is in the wreck bike's world as another plane is in a plane's
//! ([`flight_search::Scene::among_planes`]), so one narrow pass keeps one contact per ray between the
//! scenery and the hull. Its resolve takes the plane's body as the entry's other side
//! ([`contact::Side::Body`]): the impulse, the spring and the friction go into both, as they go into a
//! ball's body (S197b). The mirrors go to the plane's list ([`crate::flight::Met::wrecks`]), whose
//! resolve springs only its own side against the wreck bike's body.
//!
//! Not yet 1:1: from 137910's image (P2 down) the plane's first tick with the mirror lands nearer the
//! original's than without it, but not on it (research/s274).

#![deny(clippy::float_arithmetic)]

use crate::contact::{self, Contact};
use crate::prop_contact::Pushed;
use crate::contact_search::{self, Heightfield, Record, Search};
use crate::eco::Trees;
use crate::flight_search::{self, Plane, Scene};
use crate::rider_contact::Flown;
use crate::world::Props;
use crate::wreck_bike::{WreckBike, ray_tree};
use ps2_float::F;

/// What the wreck bike's search filed on the flown planes: each plane's mirrors, in `planes`' order.
pub type Mirrors = Vec<Vec<Contact>>;

impl WreckBike {
    /// **The wreck bike's search with the flown planes' hulls in its world**: its list emptied and
    /// searched as [`WreckBike::tick_among`]'s, the hulls (`planes`, each where its collision node
    /// stands, nearest the bike on a world that wraps) after the scenery. Its entries on plane `j`
    /// name `PLANES + j`, which [`WreckBike::step_flown`] reads; the mirrors are handed back, each
    /// plane's up to `capacity`. What the props' rays struck on it this tick
    /// ([`WreckBike::struck`]) follows its own entries, as [`WreckBike::tick_struck`] files it.
    pub fn search_flown(
        &mut self,
        dt: F,
        ground: Option<&dyn Heightfield>,
        props: Option<&Props>,
        trees: Option<&Trees>,
        planes: &[Plane<'_>],
        capacity: usize,
    ) -> (Vec<contact::Mover>, Mirrors) {
        let rays = ray_tree();
        let bounds = ground.map(|f| {
            let (lo, hi) = (f.min(), f.max());
            ([F(lo.x()), F(lo.z())], [F(hi.x()), F(hi.z())])
        });
        let mut nearby = flight_search::search();
        let node = self.record.node;
        let mut scene = Scene::among_planes(props, trees, None, planes, 1, bounds, &rays, &node, &mut nearby);
        contact_search::clear(&mut self.record, Some(&mut self.body));
        let mut records = vec![std::mem::take(&mut self.record)];
        records.extend(planes.iter().map(|_| Record { capacity, ..Record::default() }));
        contact_search::collide(&mut records, 0, Some(&mut self.body), ground, &mut scene, dt);
        let movers = scene.movers();
        drop(scene);
        let theirs = records.split_off(1);
        self.record = records.pop().unwrap_or_default();
        for e in &mut self.record.contacts {
            if let Some(o) = e.other.filter(|&o| o >= 1) {
                e.other = Some(PLANES + o - 1);
            }
        }
        let struck = &self.struck;
        let r = &mut self.record;
        for e in struck.entries.iter().chain(&struck.mirrors) {
            (r.touching, r.on_object, r.touching_other) = (true, true, true);
            if r.contacts.len() < r.capacity {
                r.contacts.push(*e);
            }
        }
        (movers, theirs.into_iter().map(|r| r.contacts).collect())
    }

    /// **The wreck bike's world stepped with the flown planes' bodies** after [`WreckBike::search_flown`]:
    /// its entries' other sides the props the props' pass struck ([`WreckBike::struck`], spent here as
    /// [`WreckBike::tick`] spends it) then `planes`' bodies, which its resolve pushes. The caller writes
    /// them back ([`crate::flight::Flight::pushed`]).
    pub fn step_flown(&mut self, dt: F, movers: &[contact::Mover], planes: &mut [Flown]) {
        let mut struck = std::mem::take(&mut self.struck);
        let first = struck.struck.len();
        for e in &mut self.record.contacts {
            if let Some(o) = e.other.filter(|&o| o >= PLANES) {
                e.other = Some(first + o - PLANES);
            }
        }
        struck.struck.extend(planes.iter().map(|p| Pushed { collection: 0, object: p.seat, collider: p.collider, body: p.body, mass: p.mass }));
        self.step_among(dt, movers, &mut struck.struck);
        for (p, b) in planes.iter_mut().zip(struck.struck.drain(first..)) {
            (p.collider, p.body) = (b.collider, b.body);
        }
        self.struck.struck = struck.struck;
    }
}

/// What a wreck bike's entry on flown plane `j` names between its search and its step: `PLANES + j`.
pub const PLANES: usize = flight_search::PLANES;
