//! **The bike strikes the knockable props** (S197): a riding bike's eleven body rays against every
//! prop's collidable mesh (its model's `<node>f.col`, [`PropSet::shapes`]) where the prop's body node
//! stands it, as entries on the bike's collision record — the prop's own collision object named on
//! the other side — and the mirrors on the prop's.
//!
//! The original files the bike's collision object and every woken prop's on one world node (free09's
//! list in S195's knock state: the human bike's object, then the beach balls'), and the node's update
//! is the whole of it: every list emptied, every object searched in list order, the world stepped. The
//! bike is filed ahead of the props, so in the tick a ball is struck:
//!
//! 1. the bike's body rays meet the ball's `f.col`: an entry (kind 0) on the bike's list with the ball's
//!    object on its other side, and its mirror (kind 1, the normal turned round, the bike on its other
//!    side) on the ball's ([`contact_search::collide`]), ahead of what the ball's own rays find;
//! 2. the struck listener wakes the ball: the body enabled first, its object's listener on the node,
//!    then the collection activated, its settle after ([`PropSet::struck`], the order S195 read);
//! 3. the bike's update resolves its list at the top of every substep, the ball's body the other side
//!    ([`crate::rider_contact::Bump::resolve`]): faster than the hard speed the first substep's impulse
//!    goes into both — the bike gets the ball's push back — and slower a spring on the bike alone;
//! 4. the props' world steps: the ball's force listener resolves its mirror against the bike's chassis
//!    as the bike's update left it, a spring on the ball alone when slow, nothing but the friction and
//!    the relative velocity when fast (a mirror takes no impulse, [`contact::resolve`]).
//!
//! In the port the props' collide pass runs first (the traffic's, [`crate::traffic::Traffic::frame_held`]),
//! then this search, which puts the mirrors at the head of the struck lists, then the bike's frame, and
//! the props' step last ([`crate::Sim::step`]). S195's knock is the proof: `tests/prop_knock.rs`
//! `the_bike_strikes_the_ball`.
//!
//! What is not here: the rider's ragdoll, the wreck bike and the plane striking a prop's `f.col` (the
//! other props' rays do: S242, [`crate::prop_props`]); the prop's own rays striking the bike's frame (an entry on the prop's list naming
//! the bike, and its mirror on the bike's); the broad phase's gather, which is every prop with a mesh
//! in collection and object order, each by its distance from the rays' centre.
//!
//! Every step path strikes (S197a): [`crate::Sim::step`]'s one bike; [`crate::Sim::step_all`]'s
//! riders, each just before its own frame in the walk's order (the last player first), the props'
//! step after them all with every player's chassis as a striker; and a seat's one-player Sim
//! ([`crate::Sim::step_bumped`], [`crate::Sim::step_among`]: online and split screen), whose props are
//! its own copy, struck by its own bike ([`crate::Sim::prop_strike_seated`]) — unless the seats share one
//! set (S197c, [`crate::prop_seats`]), as the shell's and online's seats do.

#![deny(clippy::float_arithmetic)]

use crate::biplane::Body;
use crate::bike_col::{self, BikeCol};
use crate::contact::{Collider, Contact, Node, Other, Side, Weighed};
use crate::contact_search::{self, Hit, Record, Search, Struck, Swept};
use crate::flight_search;
use crate::math::Mat4;
use crate::prop_set::PropSet;
use crate::prop_world;
use crate::ray_probe::{self, Facet};
use crate::rider_contact::{self, CAPACITY};
use crate::track::Mesh;
use crate::world::Prop;
use crate::world_search::{self, Candidate, Categories, Class, Collidable, Narrow, Nearby, flags};
use ps2_float::{F, Vf, fpu};

/// A struck prop's body as the bike's resolve pushes it: a copy carried through the bike's frame in
/// its bump ([`crate::rider_contact::Bump::props`]) and written back after ([`PropSet::pushed`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Pushed {
    /// Its collection and its object in it.
    pub collection: usize,
    pub object: usize,
    /// Its collision object's tuning, which the resolve writes its centre into.
    pub collider: Collider,
    pub body: Body,
    pub mass: F,
}

impl Pushed {
    /// The body as the other side of a contact: its collision object and its weighed body.
    pub fn weighed(&mut self) -> (&mut Collider, Weighed<'_>) {
        (&mut self.collider, Weighed { body: &mut self.body, mass: self.mass })
    }
}

/// What the bike's body rays struck this tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Strike {
    /// The bike's entries (kind 0), each naming its prop's body by `first + k`, `k` its place in
    /// [`Strike::struck`].
    pub contacts: Vec<Contact>,
    /// Each struck body, in the order the rays first met it: its collection, its object, and the
    /// mirrored entries (kind 1) that go on its list, naming the striker by its player.
    pub struck: Vec<(usize, usize, Vec<Contact>)>,
}

/// One prop's collidable mesh, where its node stands it.
struct Posed<'a> {
    collection: usize,
    object: usize,
    mesh: &'a Mesh,
    to_world: Mat4,
    centre: Vf,
    radius: F,
}

/// Where the props start in the search's list: after the bike's four collidables.
const PROPS: usize = 4;

/// The body rays' narrow phase: prop `k` is collidable `PROPS + k`.
struct Strikes<'a> {
    col: &'a BikeCol,
    at: Mat4,
    posed: &'a [Posed<'a>],
    facets: Vec<Option<Vec<Facet>>>,
    ray: ray_probe::Record,
}

impl Narrow for Strikes<'_> {
    fn clear(&mut self, objects: &mut [Collidable], probe: usize) {
        ray_probe::clear(&mut objects[probe].flags, &mut self.ray);
    }

    fn prepare(&mut self, _objects: &mut [Collidable], _probe: usize) {}

    fn pair(&mut self, _objects: &mut [Collidable], _probe: usize, other: usize) -> bool {
        let Some(k) = other.checked_sub(PROPS) else { return false };
        let p = &self.posed[k];
        let facets = self.facets[k].get_or_insert_with(|| flight_search::facets(p.mesh));
        let mesh = ray_probe::Mesh {
            id: other as u32,
            transform: p.to_world.0,
            root: 0,
            nodes: &p.mesh.nodes,
            facets,
            vertices: &p.mesh.vertices,
        };
        ray_probe::pair(&self.col.probe(self.at.0), &ray_probe::Shape::Mesh(mesh), &mut self.ray)
    }

    fn parts(&mut self, _objects: &mut [Collidable], _group: usize) -> usize {
        0
    }

    fn part(&mut self, _objects: &mut [Collidable], _group: usize, _i: usize) -> Option<usize> {
        None
    }

    fn heard_hit(&mut self, _objects: &mut [Collidable], _listener: usize, _probe: usize, _other: usize) {}

    fn heard_struck(&mut self, _objects: &mut [Collidable], _listener: usize, _struck: usize, _probe: usize) {}

    fn on_hit(&mut self, _objects: &mut [Collidable], _callback: usize, _probe: usize, _other: usize) {}

    fn on_struck(&mut self, _objects: &mut [Collidable], _callback: usize, _struck: usize, _probe: usize) {}
}

/// The length between two points, through the FPU.
fn distance(a: Vf, b: Vf) -> F {
    let d = a.sub(b);
    let acc = fpu::mula(d.x(), d.x());
    let acc = fpu::madda(acc, d.y(), d.y());
    F(fpu::sqrt(fpu::madd(acc, d.z(), d.z())))
}

/// The search as [`contact_search::collide`] asks it: the hits the narrow phase already found.
struct Found(Vec<Hit>);

impl Search for Found {
    fn rays(&mut self, _nearby: bool) -> Vec<Hit> {
        std::mem::take(&mut self.0)
    }

    fn sweep(&mut self, _from: Vf, _to: Vf, _nearby: bool) -> Option<Swept> {
        None
    }

    fn moved(&mut self, _node: &Node) {}

    fn velocity(&self, _struck: &Struck) -> Vf {
        Vf::ZERO
    }
}

/// **A riding bike's strikes on the props**: its body rays (`col`) on the chassis at `at`, against
/// every prop of `set` with a collidable mesh where its node stands. `me` is the striker's player, the
/// mirrors' other side; `first` where the struck bodies start among the bike's other sides (the
/// session's player count). At most [`CAPACITY`] entries, one per ray.
pub fn strike(col: &BikeCol, at: &Mat4, set: &PropSet, me: usize, first: usize, dt: F) -> Strike {
    let posed: Vec<Posed<'_>> = set
        .collections
        .iter()
        .zip(set.shapes.iter())
        .enumerate()
        .flat_map(|(c, (coll, shapes))| {
            coll.objects.iter().zip(shapes).enumerate().filter_map(move |(o, (object, mesh))| {
                let mesh = mesh.as_ref().filter(|_| object.collision.is_some())?;
                let (to_world, centre, radius) = Prop::posed(mesh, &PropSet::matrix(&object.node));
                Some(Posed { collection: c, object: o, mesh, to_world, centre, radius })
            })
        })
        .collect();
    if posed.is_empty() {
        return Strike::default();
    }
    let mut objects = bike_col::collidables(col, 0).to_vec();
    objects.extend(posed.iter().map(|p| Collidable {
        class: Class::Object,
        flags: flags::ENABLED,
        nearby: true,
        radius: p.radius,
        category: prop_world::COLLIDABLE,
        collides_with: prop_world::COLLIDES_WITH,
        categories: Categories { mode: 1, list: Vec::new() },
        ..Collidable::default()
    }));
    let ray = ray_probe::Record { listed: true, capacity: CAPACITY, one_per_ray: true, ..ray_probe::Record::default() };
    let mut strikes = Strikes { col, at: *at, posed: &posed, facets: (0..posed.len()).map(|_| None).collect(), ray };
    let (centre, _) = flight_search::reach(&col.rays, &at.0);
    let found = posed
        .iter()
        .enumerate()
        .map(|(k, p)| Candidate { object: PROPS + k, distance: distance(centre, p.centre) - p.radius })
        .collect();
    world_search::search_nearby(&mut objects, bike_col::BODY_RAYS, &Nearby { found, prepare: false }, &mut strikes);
    // Each struck prop's collision object is a record of its own after the bike's, in the order the
    // rays first met it; its list starts empty, as the node's clear left it before the bike searched.
    let mut hit: Vec<usize> = Vec::new();
    let hits: Vec<Hit> = strikes
        .ray
        .contacts
        .iter()
        .map(|c| {
            let k = c.object as usize - PROPS;
            let at = hit.iter().position(|&h| h == k).unwrap_or_else(|| {
                hit.push(k);
                hit.len() - 1
            });
            let struck = Struck { object: c.object as usize, other: Some(1 + at), mover: None };
            Hit { from: c.from, to: c.to, point: c.point, normal: c.normal, struck }
        })
        .collect();
    if hits.is_empty() {
        return Strike::default();
    }
    let mut records = vec![Record {
        search: true,
        sweep_back: true,
        rest_linear: F::ONE,
        rest_angular: F::ONE,
        node: rider_contact::node(at),
        capacity: CAPACITY,
        nearby: true,
        rays: Some(Vec::new()),
        ..Record::default()
    }];
    records.extend(hit.iter().map(|&k| {
        let p = &posed[k];
        let object = &set.collections[p.collection].objects[p.object];
        let capacity = object.collision.as_ref().map_or(0, |c| c.record.capacity);
        Record { capacity, ..Record::default() }
    }));
    contact_search::collide(&mut records, 0, None, None, &mut Found(hits), dt);
    let mut records = records.into_iter();
    let mut contacts = records.next().map(|r| r.contacts).unwrap_or_default();
    for e in &mut contacts {
        e.other = e.other.map(|o| first + o - 1);
    }
    let struck = hit
        .iter()
        .zip(records)
        .map(|(&k, r)| {
            let mirrors = r.contacts.into_iter().map(|e| Contact { other: Some(me), ..e }).collect();
            (posed[k].collection, posed[k].object, mirrors)
        })
        .collect();
    Strike { contacts, struck }
}

impl PropSet {
    /// What `strike` did to the props: each struck body woken as a strike wakes it ([`PropSet::struck`])
    /// and its mirrors put at the head of its list, which keeps at most its capacity, its flags set as
    /// the striker's search sets them. Answers each struck body as the bike's resolve pushes it, in
    /// the strike's order.
    pub fn strike(&mut self, strike: &Strike) -> Vec<Pushed> {
        let mut pushed = Vec::with_capacity(strike.struck.len());
        for (c, o, mirrors) in &strike.struck {
            let (c, o) = (*c, *o);
            let filed = self.collections[c].objects[o].filed;
            self.struck(c, o);
            if !filed {
                // Filed behind the bike, it searches in this pass too: heard only (B28).
                self.collections[c].hear_filed_by_strike(o, &self.rays[c]);
            }
            let coll = &mut self.collections[c];
            let bone = &coll.bones[o];
            let Some(col) = coll.objects[o].collision.as_mut() else { continue };
            let r = &mut col.record;
            let own = std::mem::take(&mut r.contacts);
            r.contacts = mirrors.iter().copied().chain(own).take(r.capacity).collect();
            r.touching = true;
            r.on_object = true;
            r.touching_other = true;
            pushed.push(Pushed { collection: c, object: o, collider: col.collider, body: bone.body, mass: bone.mass });
        }
        pushed
    }

    /// The struck bodies as the bike's frame left them, written back: the body and the centre its
    /// collision object's resolve wrote.
    pub fn pushed(&mut self, pushed: &[Pushed]) {
        for p in pushed {
            let coll = &mut self.collections[p.collection];
            coll.bones[p.object].body = p.body;
            if let Some(col) = coll.objects[p.object].collision.as_mut() {
                col.collider = p.collider;
            }
        }
    }
}

impl crate::Sim {
    /// Player `i`'s strikes on the knockable props this tick ([`strike`]): none for a rider who is down
    /// (the wreck bike's object is not the riding one), a ghost (its body rays are out of the world),
    /// a bike the shell gave no collision files, or a track with no props.
    pub fn prop_strike(&self, i: usize, dt: F) -> Strike {
        self.prop_strike_by(i, i, self.players(), dt)
    }

    /// [`Sim::prop_strike`] for a one-player Sim's bike among `players` seats (online and split
    /// screen, S197a: [`crate::Sim::step_bumped`], [`crate::Sim::step_among`]): its entries name the
    /// struck bodies after every seat's vehicle in its bump, and its mirrors name it as player 1, the
    /// one striker its own props' step reads ([`Sim::props_step`]). A seat's props are its own copy,
    /// struck by its own bike only, as its cars are.
    pub fn prop_strike_seated(&self, players: usize, dt: F) -> Strike {
        self.prop_strike_by(0, 0, players, dt)
    }

    /// A one-player Sim's bike as seat `me` of the seats' shared props (S197c, [`crate::prop_seats`]):
    /// its mirrors name it by its seat, the striker the shared step reads it as, and its entries'
    /// bodies come after the `first` vehicles of its bump.
    pub(crate) fn prop_strike_shared(&self, me: usize, first: usize, dt: F) -> Strike {
        self.prop_strike_by(0, me, first, dt)
    }

    /// Player `i`'s strike, its mirrors naming striker `me` and its entries' bodies from `first`.
    fn prop_strike_by(&self, i: usize, me: usize, first: usize, dt: F) -> Strike {
        let r = self.rider_of(i);
        let Some(col) = r.bike_col.as_deref() else { return Strike::default() };
        if r.state.wreck.is_some() || r.ghost.on || self.traffic.props.is_empty() {
            return Strike::default();
        }
        strike(col, &r.ride.transform(), &self.traffic.props, me, first, dt)
    }

    /// `strike` put on the props ([`PropSet::strike`]: the struck woken, the mirrors filed) and in
    /// `bump`: its entries after the bump's, its struck bodies the bump's to push. [`PropSet::pushed`]
    /// writes them back after the bike's frame.
    pub fn strike_into(&mut self, bump: &mut rider_contact::Bump, strike: Strike) {
        bump.props = self.traffic.props.strike(&strike);
        bump.contacts = crate::mover_contact::merged(std::mem::take(&mut bump.contacts), strike.contacts);
    }

    /// The props' step after the riders' frames (S197): the struck bodies' mirrors read every player's
    /// chassis as their frame left it, by player; and after them every player's wreck bike (S197b,
    /// [`crate::prop_wreck`]: a prop's entry on one names it `players + i`), which a prop's impulse
    /// pushes, written back after.
    pub fn props_step(&mut self, dt: F, movers: &crate::prop_set::PropMovers) {
        if self.traffic.props.is_empty() {
            return;
        }
        let mut bump = self.bump(0, Vec::new());
        let sides = self.wreck_sides();
        let mut wrecks: Vec<(Collider, Body, F)> = sides.iter().map(|s| s.unwrap_or_default()).collect();
        let mut bodies: Vec<(&mut Collider, Weighed<'_>)> = wrecks.iter_mut().map(|(c, body, mass)| (c, Weighed { body, mass: *mass })).collect();
        let mut others: Vec<Other<'_>> = bump
            .colliders
            .iter_mut()
            .zip(bump.vehicles.iter_mut())
            .map(|(collider, v)| Other { collider, side: Side::Vehicle(v) })
            .chain(bodies.iter_mut().map(|(collider, body)| Other { collider: &mut **collider, side: Side::Body(body) }))
            .collect();
        self.traffic.props.step_with(dt, movers, &mut others);
        drop(others);
        drop(bodies);
        let pushed: Vec<_> = sides.iter().zip(wrecks).map(|(s, w)| s.map(|_| w)).collect();
        self.wrecks_pushed(&pushed);
    }
}
