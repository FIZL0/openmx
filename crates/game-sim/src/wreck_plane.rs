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
//! each wreck bike with the plane's body on its resolve. Since F32 the wreck bike is stepped inside the
//! plane's frame instead, as the original's is (research/f30): one dynamics world holds both bodies,
//! and each substep the wreck bike's force listener runs after the plane's, then both integrate
//! ([`InWorld`], [`Met::downed`]); [`step_downed`] hands the stepped bike back. The Sim's plane lane ([`crate::Sim::step`])
//! does it for the other players (who are not stepped otherwise while a plane flies), the shell's
//! split screen and online seats for a lone flown plane, where the downed rider's own step later in
//! the walk skips the wreck bike's tick ([`WreckBike::flown`]). Planes flown together (S276a,
//! [`crate::flight::frames_together`]) meet it through [`search_downed_by`] and [`step_downed_by`]:
//! one search with every hull, the mirrors split onto each plane's [`Met`], one body the joint
//! resolve pushes, and one step with every plane's body.
//!
//! From 137910's image (P2 down), the plane after the first of the tick's two substeps, with the
//! mirror, is the original's row 137911 to its printed digits (`tests/c16_fly.rs`
//! `row_137911_is_the_wreck_bikes_first_substep`). The row is read in the middle of the frame, so the
//! tick's end is past it (research/s274).

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

/// **A downed wreck bike's hull in a flown plane's world** (F35): the wreck part's mesh collidable
/// (`wreckbikef.col`, category [`crate::wreck_bike::COLLIDABLE`]) where the wreck bike's record's node
/// stands it, at the copy nearest the plane, its body's velocity (what the plane's sweep reads of it),
/// and its record's room for contacts. The original files it on the plane's node, so the plane's rays
/// strike it (C16's tick 137863, research/r31): [`search_downed_by`] puts one in [`Met::hulls`] beside
/// each body of [`Met::wrecks`], and the plane's search has it in its world
/// ([`flight_search::Scene::wrecking`]).
#[derive(Clone, Debug)]
pub struct Hull {
    pub mesh: std::sync::Arc<crate::track::Mesh>,
    pub at: crate::math::Mat4,
    pub velocity: ps2_float::Vf,
    pub capacity: usize,
}

impl Hull {
    /// The hull as the plane's search places it: a knock's mesh, frame and sphere.
    pub fn knock(&self) -> flight_search::Knock<'_> {
        let (to_world, centre, radius) = crate::world::Prop::posed(&self.mesh, &self.at);
        flight_search::Knock { mesh: &self.mesh, to_world, centre, radius, velocity: self.velocity, capacity: self.capacity }
    }
}

/// The mesh `r`'s wreck bike's hull is, in `plane`'s world: its ragdoll kit's (where the props'
/// worlds take it, [`crate::Sim::wreck_hull`]), else the one the plane carries ([`Flight::wreck`]).
fn hull_mesh(r: &Rider, plane: &Flight) -> Option<std::sync::Arc<crate::track::Mesh>> {
    match r.ragdoll_kit.as_ref().and_then(|k| k.bike_mesh.as_ref()) {
        Some(b) => Some(std::sync::Arc::new(b.mesh.clone())),
        None => plane.wreck.clone(),
    }
}

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

/// **A downed wreck bike in a flown plane's dynamics world** (F32): the original's wreck bike is a
/// body of the plane's world (research/f30), so each substep its force listener runs after the
/// planes' (gravity, and its list resolved against the props it struck, then the planes' bodies),
/// and it is integrated beside them; its preforce clears its shock peak and its post puts its
/// transform where its body is, as its own world's do ([`WreckBike::tick`]).
///
/// [`search_downed_by`] files one in the first of its planes' [`Met::downed`]; the plane's frame
/// ([`crate::flight::Flight::frame_met`], [`crate::flight::frames_together`]) steps it, its body the
/// one the planes' resolve pushes ([`Met::wrecks`]); [`step_downed_by`] hands it back to its rider.
#[derive(Clone, Debug, Default)]
pub struct InWorld {
    /// The rider's wreck bike after its search, its entries on plane `j` renamed to the props it
    /// struck plus `j`, the props' bodies in [`WreckBike::struck`].
    pub bike: WreckBike,
    pub movers: Vec<contact::Mover>,
    /// Where its body is: in the met its place in [`Met::wrecks`]; within the frame its place among
    /// the frame's pushed bodies.
    pub place: usize,
    /// The planes on its resolve, after the props, by their place among the planes its search had
    /// (the fleet's planes with a met, in order; a lone plane's frame has only `0`).
    pub planes: Vec<usize>,
    /// Its frame ran in the plane's world. A plane whose frame never ran leaves it to
    /// [`step_downed_by`], which steps it after as before F32 ([`WreckBike::step_flown`]).
    pub stepped: bool,
}

impl InWorld {
    /// Its preforce listener, `body` its body in the frame.
    pub fn preforce(&mut self, body: &mut Pushed) {
        body.body.clear_shock_peak();
    }

    /// Its force listener, after the planes': the part's gravity and its list resolved against the
    /// props it struck then `planes` (each its copy nearest the bike, [`crate::flight::Flight::flown`]),
    /// which the caller writes back. `body` is its body in the frame, which the planes' resolve
    /// pushed this substep.
    pub fn force(&mut self, body: &mut Pushed, planes: &mut [Flown]) {
        (self.bike.collider, self.bike.body) = (body.collider, body.body);
        let WreckBike { body: b, mass, gravity, collider, record, struck, .. } = &mut self.bike;
        let mut props: Vec<_> = struck.struck.iter_mut().map(Pushed::weighed).collect();
        let mut flown: Vec<_> = planes.iter_mut().map(|p| (&mut p.collider, contact::Weighed { body: &mut p.body, mass: p.mass })).collect();
        let mut others: Vec<contact::Other<'_>> = props
            .iter_mut()
            .chain(flown.iter_mut())
            .map(|(collider, body)| contact::Other { collider: &mut **collider, side: contact::Side::Body(body) })
            .collect();
        let movers = &self.movers;
        crate::biplane::collision_force(b, *gravity, |body| {
            let mut rigid = contact::Weighed { body, mass: *mass };
            contact::resolve(collider, &mut rigid, &mut record.contacts, &mut others, movers, true);
        });
        drop(others);
        (body.collider, body.body) = (self.bike.collider, self.bike.body);
    }

    /// Its post listener: the transform where `body` (its body in the frame) is.
    pub fn post(&mut self, body: &Pushed) {
        self.bike.record.node = crate::wreck_bike::node_of(&body.body);
        self.stepped = true;
    }
}

/// **The downed riders' wreck bikes searched among the flown planes**, before their frame (S276,
/// S276a): what [`step_downed`] needs after it. Each searched rider, by its place in the riders
/// handed over, its search's movers, and where its body went in each plane's [`Met::wrecks`] (the
/// plane's place among those handed over, the body's in its met); and the planes whose hulls were in
/// the searches, in their order there.
#[derive(Debug, Default)]
pub struct Downed {
    searched: Vec<(usize, Vec<contact::Mover>, Vec<(usize, usize)>, Option<(usize, usize)>)>,
    planes: Vec<usize>,
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
    search_downed_by(riders, &[plane], dt, world, &mut [met])
}

/// [`search_downed`] among several flown planes (S276a, the planes [`crate::flight::frames_together`]
/// flies): each wreck bike searched once, every hull in its world in `planes`' order (a plane with no
/// hull left out), its mirrors on plane `k`'s hull after `mets[k]`'s, and its body in each of their
/// [`Met::wrecks`] under one `object` (its place among the searched), which the joint frame pushes as
/// one body. `riders` leaves out the flyers' own.
pub fn search_downed_by(riders: &mut [&mut Rider], planes: &[&Flight], dt: F, world: &mut dyn World, mets: &mut [&mut Met]) -> Downed {
    let mut out = Downed::default();
    let hulls: Vec<(usize, crate::rider_contact::Hull<'_>)> = planes.iter().enumerate().filter_map(|(k, p)| Some((k, p.hull(0)?))).collect();
    if hulls.is_empty() {
        return out;
    }
    out.planes = hulls.iter().map(|&(k, _)| k).collect();
    let capacity = hulls.iter().map(|(_, h)| h.capacity).max().unwrap_or(0);
    for (i, r) in riders.iter_mut().enumerate().filter(|(_, r)| down(r)) {
        let meshes: Vec<_> = hulls.iter().map(|&(k, _)| hull_mesh(r, planes[k])).collect();
        let Some(c) = r.crash.as_mut() else { continue };
        let from = c.bike.origin();
        let them: Vec<Plane<'_>> = hulls
            .iter()
            .map(|&(k, ref h)| {
                let mut at = h.at;
                at.0[3] = crate::nearest_copy(&*world, from, at.0[3]);
                Plane { mesh: h.mesh, at, velocity: planes[k].body.velocity }
            })
            .collect();
        let (ground, props, trees) = world.wooded_near(from);
        let (movers, mirrors) = c.bike.search_flown(dt, ground.as_deref(), props, trees, &them, capacity);
        drop(ground);
        let w = out.searched.len();
        let b = &c.bike;
        let mut placed = Vec::with_capacity(hulls.len());
        for ((&(k, _), mirrors), mesh) in hulls.iter().zip(&mirrors).zip(meshes) {
            let Some(met) = mets.get_mut(k) else { continue };
            let j = met.wrecks.len();
            let near = planes[k].body.position;
            met.mirrors.extend(mirrors.iter().map(|e| Contact { other: Some(WRECK_FROM + j), point: crate::nearest_copy(&*world, near, e.point), ..*e }));
            met.wrecks.push(Pushed { collection: 0, object: w, collider: b.collider, body: b.body, mass: b.mass });
            // Its hull in the plane's world (F35), the copy nearest the plane.
            met.hulls.resize(j, None);
            met.hulls.push(mesh.map(|mesh| {
                let n = &b.record.node;
                let mut at = crate::math::Mat4([n.rows[0], n.rows[1], n.rows[2], n.origin]);
                at.0[3] = crate::nearest_copy(&*world, near, at.0[3]);
                Hull { mesh, at, velocity: b.body.velocity, capacity: b.record.capacity }
            }));
            placed.push((k, j));
        }
        // Its step in the first of its planes' worlds (F32): a copy of the bike, its entries on the
        // hulls renamed to their places on its resolve, after the props it struck.
        let held = placed.first().and_then(|&(k, j)| {
            let met = mets.get_mut(k)?;
            let mut bike = b.clone();
            let struck = std::mem::take(&mut bike.struck);
            let first = struck.struck.len();
            for e in &mut bike.record.contacts {
                if let Some(o) = e.other.filter(|&o| o >= PLANES) {
                    e.other = Some(first + o - PLANES);
                }
            }
            bike.struck.struck = struck.struck;
            met.downed.push(InWorld { bike, movers: movers.clone(), place: j, planes: out.planes.clone(), stepped: false });
            Some((k, met.downed.len() - 1))
        });
        out.searched.push((i, movers, placed, held));
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
    step_downed_by(riders, &mut [plane], downed, &mut [met], dt, world, own_step);
}

/// [`step_downed`] after the planes' joint frame (S276a): each wreck bike [`search_downed_by`]
/// searched takes back one body, the joint resolve's (the first of its planes' [`Met::wrecks`], which
/// all hold the same). Since F32 the frame stepped it in the planes' world ([`InWorld`]), and it takes
/// that bike back whole; one whose frame did not run steps its world after, as before, with every
/// searched plane's body on its resolve, in the search's order, each written back to its plane.
/// `planes` and `mets` as the search's.
pub fn step_downed_by(
    riders: &mut [&mut Rider],
    planes: &mut [&mut Flight],
    downed: Downed,
    mets: &mut [&mut Met],
    dt: F,
    world: &dyn World,
    own_step: bool,
) {
    let wrecks: Vec<Vec<Pushed>> = mets.iter_mut().map(|m| std::mem::take(&mut m.wrecks)).collect();
    let mut held: Vec<Vec<InWorld>> = mets.iter_mut().map(|m| std::mem::take(&mut m.downed)).collect();
    for (i, movers, placed, at) in downed.searched {
        let r = &mut *riders[i];
        let Some(c) = r.crash.as_mut() else { continue };
        let stepped = at.and_then(|(k, d)| held.get_mut(k)?.get_mut(d)).filter(|d| d.stepped).map(std::mem::take);
        let in_world = stepped.is_some();
        if let Some(d) = stepped {
            c.bike = d.bike;
        }
        if let Some(w) = placed.first().and_then(|&(k, j)| wrecks.get(k)?.get(j)) {
            (c.bike.collider, c.bike.body) = (w.collider, w.body);
        }
        if !in_world {
            let from = c.bike.origin();
            let mut flown: Vec<Flown> = downed.planes.iter().map(|&k| planes[k].flown(k, from, world)).collect();
            c.bike.step_flown(dt, &movers, &mut flown);
            for (&k, f) in downed.planes.iter().zip(&flown) {
                planes[k].pushed(f);
            }
        }
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
