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
//! Wired (S276): before a flown plane's frame [`search_downed`] searches every downed rider's wreck
//! bike with the hull in its world and puts the mirrors and the wreck bikes' bodies in the plane's
//! [`Met`]; after it [`step_downed`] takes the bodies back as the plane's resolve left them and steps
//! each wreck bike with the plane's body on its resolve. The Sim's plane lane ([`crate::Sim::step`])
//! does it for the other players (who are not stepped otherwise while a plane flies), the shell's
//! split screen and online seats for a lone flown plane, where the downed rider's own step later in
//! the walk skips the wreck bike's tick ([`WreckBike::flown`]).
//!
//! Not yet 1:1: from 137910's image (P2 down) the plane's first tick with the mirror lands nearer the
//! original's than without it, but not on it (research/s274).

#![deny(clippy::float_arithmetic)]

use crate::contact::{self, Contact};
use crate::prop_contact::Pushed;
use crate::contact_search::{self, Heightfield, Record, Search};
use crate::eco::Trees;
use crate::flight_search::{self, Plane, Scene};
use crate::flight::{Flight, Met, WRECK_FROM};
use crate::rider::Rider;
use crate::rider_contact::Flown;
use crate::world::{Props, World};
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

/// **The downed riders' wreck bikes searched among a flown plane**, before its frame (S276): what
/// [`step_downed`] needs after it. Each searched rider, by its place in the riders handed over, and
/// its search's movers.
#[derive(Debug, Default)]
pub struct Downed {
    searched: Vec<(usize, Vec<contact::Mover>)>,
}

impl Downed {
    /// No rider's wreck bike met the plane's hull this tick.
    pub fn is_empty(&self) -> bool {
        self.searched.is_empty()
    }
}

/// Whether `r`'s wreck bike is in a flown plane's world: the rider down, its part colliding, and not
/// a ghost (whose bodies meet nobody).
fn down(r: &Rider) -> bool {
    r.crash.is_some() && !r.ghost.on && r.wreckage.as_ref().is_some_and(|w| w.part.collision_enabled)
}

/// **Before the plane's frame** (S276): every downed rider's wreck bike searched with `plane`'s hull in
/// its world ([`WreckBike::search_flown`]; the hull nearest the bike on a world that wraps), and the
/// mirrors of its strikes on the hull after `met`'s, each naming [`crate::flight::WRECK_FROM`]` + j`,
/// its point at the copy nearest the plane, with the wreck bike's body `j` in [`Met::wrecks`]. A plane
/// with no hull (or no ray probe) meets nothing, and the downed riders' own steps tick their wreck
/// bikes as before. `riders` leaves out the flyer's own.
pub fn search_downed(riders: &mut [&mut Rider], plane: &Flight, dt: F, world: &mut dyn World, met: &mut Met) -> Downed {
    let mut out = Downed::default();
    let Some(h) = plane.hull(0) else { return out };
    for (i, r) in riders.iter_mut().enumerate().filter(|(_, r)| down(r)) {
        let Some(c) = r.crash.as_mut() else { continue };
        let from = c.bike.origin();
        let mut at = h.at;
        at.0[3] = crate::nearest_copy(&*world, from, at.0[3]);
        let planes = [Plane { mesh: h.mesh, at, velocity: plane.body.velocity }];
        let (ground, props, trees) = world.wooded_near(from);
        let (movers, mirrors) = c.bike.search_flown(dt, ground.as_deref(), props, trees, &planes, h.capacity);
        drop(ground);
        let j = met.wrecks.len();
        let near = plane.body.position;
        met.mirrors.extend(mirrors.iter().flatten().map(|e| Contact { other: Some(WRECK_FROM + j), point: crate::nearest_copy(&*world, near, e.point), ..*e }));
        let b = &c.bike;
        met.wrecks.push(Pushed { collection: 0, object: j, collider: b.collider, body: b.body, mass: b.mass });
        out.searched.push((i, movers));
    }
    out
}

/// **After the plane's frame** (S276): each wreck bike [`search_downed`] searched takes its body back
/// as the plane's resolve left it ([`Met::wrecks`], emptied here), then steps its world with the
/// plane's body on its resolve ([`WreckBike::step_flown`], the copy nearest the bike), which is
/// written back to the plane ([`Flight::pushed`]); its crash's origin and resting follow. With
/// `own_step` the rider's own [`crate::ride::Crash::step`] runs later this tick and skips the body's
/// tick ([`WreckBike::flown`]); without, nothing else steps the rider (the Sim's plane lane).
pub fn step_downed(riders: &mut [&mut Rider], plane: &mut Flight, downed: Downed, met: &mut Met, dt: F, world: &dyn World, own_step: bool) {
    let wrecks = std::mem::take(&mut met.wrecks);
    for (j, (i, movers)) in downed.searched.into_iter().enumerate() {
        let r = &mut *riders[i];
        let Some(c) = r.crash.as_mut() else { continue };
        if let Some(w) = wrecks.get(j) {
            (c.bike.collider, c.bike.body) = (w.collider, w.body);
        }
        let mut flown = [plane.flown(0, c.bike.origin(), world)];
        c.bike.step_flown(dt, &movers, &mut flown);
        plane.pushed(&flown[0]);
        c.bike.flown = own_step;
        c.origin = c.bike.origin();
        if let Some(w) = r.wreckage.as_ref() {
            c.resting = length(c.bike.body.velocity) <= w.part.speed_threshold
                && length(c.bike.body.rates) <= w.part.angular_rate_threshold;
        }
    }
}

/// `v.xyz`'s length as [`crate::ride::Crash::step`]'s resting test takes it: the FPU's accumulated dot
/// product, `x`, `y`, `z` in order, and its square root.
fn length(v: ps2_float::Vf) -> F {
    ps2_float::Acc::mula(v.x(), v.x()).madda(v.y(), v.y()).madd(v.z(), v.z()).sqrt()
}

/// The riders in `sims` but `me`'s, of the seats `on` says are in the ride, and `me`'s plane.
fn others_of<'s>(sims: &'s mut [crate::Sim], me: usize, on: &[bool]) -> (Vec<&'s mut Rider>, Option<&'s mut Flight>) {
    let mut plane = None;
    let mut riders = Vec::new();
    for (j, s) in sims.iter_mut().enumerate() {
        if j == me {
            plane = s.plane.as_mut();
        } else if on.get(j).copied().unwrap_or(true) {
            riders.push(&mut s.rider);
        }
    }
    (riders, plane)
}

/// [`search_downed`] on online's seats (S276): seat `me`'s plane ([`crate::Sim::plane`]) among the
/// downed riders of the other seats `on` holds in the ride. Nothing without a plane.
pub fn search_downed_among(sims: &mut [crate::Sim], me: usize, on: &[bool], dt: F, world: &mut dyn World, met: &mut Met) -> Downed {
    match others_of(sims, me, on) {
        (mut riders, Some(plane)) => search_downed(&mut riders, plane, dt, world, met),
        _ => Downed::default(),
    }
}

/// [`step_downed`] on online's seats, after seat `me`'s plane's frame: the same `on` as
/// [`search_downed_among`]'s, the riders' own steps to come this tick.
pub fn step_downed_among(sims: &mut [crate::Sim], me: usize, on: &[bool], downed: Downed, met: &mut Met, dt: F, world: &dyn World) {
    if downed.is_empty() {
        return;
    }
    if let (mut riders, Some(plane)) = others_of(sims, me, on) {
        step_downed(&mut riders, plane, downed, met, dt, world, true);
    }
}
