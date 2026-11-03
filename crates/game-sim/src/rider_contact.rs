//! **Rider against rider** (S78): one riding bike's body rays striking another's frame, and the two
//! pushed apart.
//!
//! The original keeps every riding bike's collision object (the bike's `+0x7d0`) in one list on the
//! level's collision node, and that node's update is the whole of it, once a tick:
//!
//! 1. every object's contact list is emptied, then every object searches, in list order — so all of
//!    the tick's contacts are found from where the last tick left every bike ([`gather`]). A bike's
//!    body rays striking another's frame put a contact on the striker's list and its mirror (kind 1,
//!    the normal turned round) on the struck one's ([`contact_search::collide`]);
//! 2. then the bikes are updated, and each one's ground step resolves its own list at the top of
//!    every substep, the chassis being the own side and the other bike's chassis the other side
//!    ([`Bump::resolve`], [`contact::resolve_ridden`]). The impulse is the first substep's only, and
//!    only a striker's; a contact slower than the hard speed is a spring on both bikes.
//!
//! The tuning is the bike's `[Collisions]` block, through the five setters the bike's builder calls
//! ([`collider`]): `tests/rider_contact.rs` proves it against both split-screen riders' objects.
//!
//! A ghost (a put-back's, or a paused rider's) collides with no one: the ghost switch takes its frame
//! out of the broad phase and puts the vehicle category in its rays' ignore list, so it neither strikes
//! nor is struck ([`bike_col::ghost`]). A rider who is down rides the wreck bike, whose object is not
//! in this list.
//!
//! What is not here: the objects' sweep back (the riding bike's is on its chassis, which
//! [`contact_search::sweep_back`] does not port), and the broad phase's gather, which is replaced by
//! every other rider's frame in player order with its distance from the rays' centre. The riding
//! bike's rays against the world's props are searched by the rider's own update, after this list
//! ([`scenery`], S138).

#![deny(clippy::float_arithmetic)]

use crate::bike_col::{self, BikeCol};
use crate::contact::{self, Collider, Contact, Node, Other, Response, Ridden, Side, Vehicle};
use crate::contact_search::{self, Hit, Record, Search, Struck, Swept};
use crate::math::Mat4;
use crate::Sim;
use crate::ride::Controls;
use crate::phy::{Collisions, Phy};
use crate::rider::{Frame, Rider};
use crate::world::World;
use crate::ray_probe::{self, Matrix};
use crate::world_search::{self, Candidate, Collidable, Narrow, Nearby, flags};
use ps2_float::{F, Vf, fpu};

/// A pound's weight in its own units, which the springs' stiffness is scaled by.
const POUND: F = F(f32::from_bits(0x4200_cccd));

/// The setters' "copy the other one" value, in every lane.
const SENTINEL: Vf = Vf::new(-1.0, -1.0, -1.0, 1.0);

/// The result record's capacity, the body rays' eleven.
pub const CAPACITY: usize = 11;

/// The riding bike's collision object's tuning, as the builder's setters leave it for a chassis of
/// `chassis_mass` (`+0x210`):
///
/// * the mass is `CollisionWeight`'s, `CollisionCofGOffset` the centre; the inverse inertia is
///   `(0, 0, 0, 1)`, because the chassis' tensor is still zero when the mass is set;
/// * the springs: `|k · m · 32.2|` and `d · 2 · √(m·k)` on the chassis' mass, the body ones copying
///   the plain ones where they are -1;
/// * the moment masks, the body ones copying the plain ones where they are the -1 vector;
/// * `Friction`, `BodyFriction` (-1 copies), and the sixteen surface frictions
///   ([`Collisions::surface_friction`]: `TerrainFriction` in each on MX Unleashed's files).
pub fn collider(c: &Collisions, chassis_mass: F) -> Collider {
    let mass = c.collision_mass;
    let inv_mass = if mass > F::ZERO { F::ONE / mass } else { F::ZERO };
    let stiffness = |k: F| (k * chassis_mass * POUND).abs();
    let damping = |d: F, k: F| {
        if !(chassis_mass > F::ZERO) || !(k > F::ZERO) {
            F::ZERO
        } else {
            d * F(2.0) * (chassis_mass * k).sqrt()
        }
    };
    let scenery_stiffness = stiffness(c.spring);
    let scenery = Response {
        stiffness: scenery_stiffness,
        damping: damping(c.damping, scenery_stiffness),
        restitution: c.restitution,
        angular: c.angular_impulse_enabled,
        friction: c.friction,
        spring_mask: c.spring_moment_scale,
        friction_mask: c.friction_moment_scale,
    };
    let copied = |v: F, plain: F| if v == F(-1.0) { plain } else { v };
    let actor_stiffness = if c.body_spring == F(-1.0) { scenery.stiffness } else { stiffness(c.body_spring) };
    let actor_damping =
        if c.body_damping == F(-1.0) { scenery.damping } else { damping(c.body_damping, actor_stiffness) };
    let masked = |v: Vf, plain: Vf| if v == SENTINEL { plain } else { v };
    let actor = Response {
        stiffness: actor_stiffness,
        damping: actor_damping,
        restitution: copied(c.body_restitution, scenery.restitution),
        angular: c.body_angular_impulse_enabled,
        friction: copied(c.body_friction, scenery.friction),
        spring_mask: masked(c.body_spring_moment_scale, scenery.spring_mask),
        friction_mask: masked(c.body_friction_moment_scale, scenery.friction_mask),
    };
    let frame = bike_col::SHAPE | bike_col::ACTOR | bike_col::RIDER | bike_col::VEHICLE;
    Collider {
        damp: false,
        damp_linear: F::ONE,
        damp_angular: F::ONE,
        mass,
        inv_mass,
        inv_inertia: Vf::VF0,
        cofg: c.collision_cofg_offset,
        cofg_world: Vf::ZERO,
        actors: bike_col::RECORD_ACTORS,
        collides_with: u32::MAX,
        collidable: Some(frame | bike_col::RIDE_START[0] | bike_col::RIDE_START[1]),
        max_depth: c.max_compression,
        scenery,
        actor,
        surfaces: c.surface_friction,
        normal_speed: F::ZERO,
        relative: Vf::ZERO,
    }
}

/// One player's bike as the gather sees it.
#[derive(Clone, Copy)]
pub struct Seat<'a> {
    /// The bike's two collision files; `None` is a bike the shell gave none, which is in no list.
    pub col: Option<&'a BikeCol>,
    /// The chassis' transform, as the last tick left it.
    pub at: Mat4,
    /// A ghost: out of the broad phase, its rays ignoring the vehicle category.
    pub ghost: bool,
    /// On the bike: a rider who is down is on the wreck bike, whose object is not in the list.
    pub riding: bool,
}

impl Seat<'_> {
    fn listed(&self) -> bool {
        self.col.is_some() && self.riding
    }
}

/// The node a collision object's transform is.
pub fn node(m: &Mat4) -> Node {
    Node { rows: [m.0[0], m.0[1], m.0[2]], origin: m.0[3] }
}

/// A model-space point through the transform, the translation first (the collidables' own sum).
fn placed(m: &Matrix, c: [F; 3]) -> Vf {
    let acc = m[3].mul(Vf::splat(1.0));
    let acc = Vf::madd(acc, m[0], Vf::splat(c[0].0));
    let acc = Vf::madd(acc, m[1], Vf::splat(c[1].0));
    let p = Vf::madd(acc, m[2], Vf::splat(c[2].0));
    Vf::new(p.x(), p.y(), p.z(), 1.0)
}

/// The length between two points, through the FPU.
fn distance(a: Vf, b: Vf) -> F {
    let d = a.sub(b);
    let acc = fpu::mula(d.x(), d.x());
    let acc = fpu::madda(acc, d.y(), d.y());
    F(fpu::sqrt(fpu::madd(acc, d.z(), d.z())))
}

/// The narrow phase of one bike's body rays: every other bike's frame mesh, struck as its player, and
/// every flown plane's hull (N22a), collidable `4 · players + k`.
struct Strikes<'a> {
    seats: &'a [Seat<'a>],
    hulls: &'a [Posed<'a>],
    facets: Vec<Option<Vec<ray_probe::Facet>>>,
    ray: ray_probe::Record,
}

impl Narrow for Strikes<'_> {
    fn clear(&mut self, objects: &mut [Collidable], probe: usize) {
        ray_probe::clear(&mut objects[probe].flags, &mut self.ray);
    }

    fn prepare(&mut self, _objects: &mut [Collidable], _probe: usize) {}

    fn pair(&mut self, _objects: &mut [Collidable], probe: usize, other: usize) -> bool {
        let (i, j) = (probe / 4, other / 4);
        if let Some(k) = other.checked_sub(4 * self.seats.len()) {
            let (Some(mine), Some(h)) = (self.seats[i].col, self.hulls.get(k)) else { return false };
            let facets = self.facets[k].get_or_insert_with(|| crate::flight_search::facets(h.mesh));
            let mesh = ray_probe::Mesh {
                id: other as u32,
                transform: h.to_world.0,
                root: 0,
                nodes: &h.mesh.nodes,
                facets,
                vertices: &h.mesh.vertices,
            };
            return ray_probe::pair(&mine.probe(self.seats[i].at.0), &ray_probe::Shape::Mesh(mesh), &mut self.ray);
        }
        if other % 4 != bike_col::FRAME {
            return false;
        }
        let (Some(mine), Some(theirs)) = (self.seats[i].col, self.seats[j].col) else { return false };
        mine.strike(self.seats[i].at.0, theirs, j as u32, self.seats[j].at.0, &mut self.ray)
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

/// One bike's search: its body rays against what was gathered round them.
struct Probe<'a> {
    strikes: Strikes<'a>,
    objects: Vec<Collidable>,
    me: usize,
}

impl Probe<'_> {
    /// The flown planes' hulls (N22a), then the other bikes' frames, in player order, with their
    /// distance from the rays' centre less the frame's reach: a ghost's is not in the broad phase,
    /// nor is one off the list. The planes come first, as the original's search lists them.
    fn gathered(&self) -> Nearby {
        let seats = self.strikes.seats;
        let Some(mine) = seats[self.me].col else { return Nearby::default() };
        let (centre, _) = crate::flight_search::reach(&mine.rays, &seats[self.me].at.0);
        let planes = self
            .strikes
            .hulls
            .iter()
            .enumerate()
            .map(|(k, h)| Candidate { object: 4 * seats.len() + k, distance: distance(centre, h.centre) - h.radius });
        let found = planes
            .chain(seats.iter()
            .enumerate()
            .filter(|&(j, s)| j != self.me && s.listed() && !s.ghost)
            .filter_map(|(j, s)| {
                let col = s.col?;
                let root = col.frame.nodes.first()?;
                let theirs = placed(&s.at.0, root.centre);
                Some(Candidate { object: 4 * j + bike_col::FRAME, distance: distance(centre, theirs) - col.frame_radius })
            }))
            .collect();
        Nearby { found, prepare: false }
    }
}

impl Search for Probe<'_> {
    fn rays(&mut self, _nearby: bool) -> Vec<Hit> {
        let nearby = self.gathered();
        let probe = 4 * self.me + bike_col::BODY_RAYS;
        world_search::search_nearby(&mut self.objects, probe, &nearby, &mut self.strikes);
        self.strikes
            .ray
            .contacts
            .iter()
            .map(|c| {
                let j = c.object as usize;
                let n = self.strikes.seats.len();
                // A plane's hull is struck as its record, after the players' (N22a).
                let struck = match j.checked_sub(4 * n) {
                    Some(k) => Struck { object: j, other: Some(n + k), mover: None },
                    None => Struck { object: 4 * j + bike_col::FRAME, other: Some(j), mover: None },
                };
                Hit { from: c.from, to: c.to, point: c.point, normal: c.normal, struck }
            })
            .collect()
    }

    fn sweep(&mut self, _from: Vf, _to: Vf, _nearby: bool) -> Option<Swept> {
        None
    }

    fn moved(&mut self, _node: &Node) {}

    fn velocity(&self, _struck: &Struck) -> Vf {
        Vf::ZERO
    }
}

/// The collidables of every seat, four a player, as the ride and the ghost switch leave them.
fn collidables(seats: &[Seat<'_>]) -> Vec<Collidable> {
    let mut objects = Vec::with_capacity(4 * seats.len());
    for (i, s) in seats.iter().enumerate() {
        match s.col {
            Some(col) => {
                let mut four = bike_col::collidables(col, 4 * i);
                if s.ghost {
                    bike_col::ghost(&mut four[bike_col::BODY_RAYS].categories, bike_col::VEHICLE, true);
                    bike_col::ghost(&mut four[bike_col::SWEEP].categories, bike_col::VEHICLE, true);
                }
                if !s.riding {
                    for c in &mut four {
                        c.flags &= !flags::ENABLED;
                    }
                }
                objects.extend(four);
            }
            None => objects.extend(std::iter::repeat_with(Collidable::default).take(4)),
        }
    }
    objects
}

/// **The tick's contacts**: every listed bike's list emptied, then every one searched in player order,
/// from where `seats` stand. One list a player (empty for one off the list).
pub fn gather(seats: &[Seat<'_>], dt: F) -> Vec<Vec<Contact>> {
    gather_near(seats, dt, &|_, at| at)
}

/// The copy of a point `at` nearest another, `from` (MD44): [`crate::nearest_copy`] on a world that
/// wraps, `at` itself on any other.
pub type Near<'a> = &'a dyn Fn(Vf, Vf) -> Vf;

/// [`gather`] on a world that wraps (MD44, the port's own): each bike searched with every other where
/// it is nearest it (`near`, from its transform's origin), so two bikes a seam apart strike as two a
/// few feet apart do. The struck bike's half of a contact is put back by its own copy: its world
/// point is the copy nearest where it really stands. On a world that does not wrap `near` answers
/// the point it is given, and this is [`gather`] to the bit.
pub fn gather_near(seats: &[Seat<'_>], dt: F, near: Near<'_>) -> Vec<Vec<Contact>> {
    gather_met(seats, &[], dt, near).0
}

/// **A flown plane's hull as the riders' bikes meet it** (N22a): its collision mesh
/// (`biplanef.col`, [`crate::flight::Flight::hull`]) where the plane's collision node stands it
/// (the model's origin, as the plane's own search last placed it), and the room its record has.
/// Its seat is the plane's player, which the bikes' resolve writes its body back to.
#[derive(Clone, Copy)]
pub struct Hull<'a> {
    pub seat: usize,
    pub mesh: &'a crate::track::Mesh,
    pub at: Mat4,
    pub capacity: usize,
}

/// A hull where it stands, with the sphere its collidable is gathered by.
struct Posed<'a> {
    mesh: &'a crate::track::Mesh,
    to_world: Mat4,
    centre: Vf,
    radius: F,
}

/// [`gather_near`] with the flown planes' hulls in the riders' world (N22a). The original keeps both
/// planes' collision objects in the riders' list, ahead of the bikes, and a bike's body rays strike
/// a plane's hull (category [`crate::flight::COLLIDABLE`], which they do not ignore) as they strike
/// another bike's frame: the entry on the bike's list names plane `k` as `seats.len() + k`, and the
/// mirror (kind 1) goes on the plane's list naming the bike. Answers each seat's list, then each
/// hull's mirrors, in `hulls`' order. The planes' own searches are their frames' ([`crate::flight`]),
/// so their lists here hold only the mirrors, which the plane's frame puts after its own entries
/// ([`crate::flight::Met`]), as the original's list order puts them.
pub fn gather_met(seats: &[Seat<'_>], hulls: &[Hull<'_>], dt: F, near: Near<'_>) -> (Vec<Vec<Contact>>, Vec<Vec<Contact>>) {
    let n = seats.len();
    let mut records: Vec<Record> = seats
        .iter()
        .map(|s| Record {
            search_ground: false,
            search: s.listed(),
            sweep_back: true,
            rest_linear: F::ONE,
            rest_angular: F::ONE,
            node: node(&s.at),
            capacity: CAPACITY,
            nearby: true,
            // The heightfield pass is the only reader of the rays here, and it is off.
            rays: s.listed().then(Vec::new),
            ..Record::default()
        })
        .collect();
    records.extend(hulls.iter().map(|h| Record { capacity: h.capacity, ..Record::default() }));
    let mut objects = collidables(seats);
    objects.extend(hulls.iter().map(|h| Collidable {
        class: world_search::Class::Object,
        flags: flags::ENABLED,
        nearby: true,
        radius: crate::world::Prop::posed(h.mesh, &h.at).2,
        category: crate::flight::COLLIDABLE,
        collides_with: crate::flight::COLLIDES_WITH,
        categories: world_search::Categories { mode: 1, list: Vec::new() },
        ..Collidable::default()
    }));
    for me in 0..seats.len() {
        if !seats[me].listed() {
            continue;
        }
        let from = seats[me].at.0[3];
        let placed: Vec<Seat<'_>> = seats
            .iter()
            .enumerate()
            .map(|(j, s)| {
                let mut s = *s;
                if j != me {
                    s.at.0[3] = near(from, s.at.0[3]);
                }
                s
            })
            .collect();
        let posed: Vec<Posed<'_>> = hulls
            .iter()
            .map(|h| {
                let mut at = h.at;
                at.0[3] = near(from, at.0[3]);
                let (to_world, centre, radius) = crate::world::Prop::posed(h.mesh, &at);
                Posed { mesh: h.mesh, to_world, centre, radius }
            })
            .collect();
        let before: Vec<usize> = records.iter().map(|r| r.contacts.len()).collect();
        let ray = ray_probe::Record { listed: true, capacity: CAPACITY, one_per_ray: true, ..ray_probe::Record::default() };
        let strikes = Strikes { seats: &placed, hulls: &posed, facets: posed.iter().map(|_| None).collect(), ray };
        let mut probe = Probe { strikes, objects: objects.clone(), me };
        contact_search::collide(&mut records, me, None, None, &mut probe, dt);
        for (o, r) in records.iter_mut().enumerate().filter(|&(o, _)| o != me) {
            let stands = if o < n { seats[o].at.0[3] } else { hulls[o - n].at.0[3] };
            for c in &mut r.contacts[before[o]..] {
                c.point = near(stands, c.point);
            }
        }
    }
    let mut lists = records.into_iter().map(|r| r.contacts);
    let bikes = lists.by_ref().take(n).collect();
    (bikes, lists.collect())
}

/// **One bike's contacts while it is stepped**: its list, every player's object, and every other
/// player's chassis as it stands — which the resolution pushes, and the caller writes back.
#[derive(Clone, Debug, Default)]
pub struct Bump {
    pub me: usize,
    pub contacts: Vec<Contact>,
    pub colliders: Vec<Collider>,
    pub vehicles: Vec<Vehicle>,
    /// The movers the list's scenery contacts name: the props that moved this tick ([`scenery`],
    /// S145). Empty while nothing the rays struck moves.
    pub movers: Vec<crate::contact::Mover>,
    /// The knockable props' bodies the list's entries strike (S197, [`crate::prop_contact`]), the
    /// other sides after the players' (an entry's other `players + k` is `props[k]`): pushed by the
    /// resolve, and written back to the props after the frame.
    pub props: Vec<crate::prop_contact::Pushed>,
    /// The flown planes' bodies the list's entries strike (N22a, [`gather_met`]), the other sides
    /// after the players' and before the props (an entry's other `players + k` is `planes[k]`):
    /// pushed by the resolve, and written back to the planes after the frame
    /// ([`crate::flight::Flight::pushed`]).
    pub planes: Vec<Flown>,
}

/// **A flown plane's body as a bike's resolve pushes it** (N22a): a copy of its collision object's
/// tuning and its body, carried through the bike's frame in its bump and written back after. On a
/// world that wraps the body is the copy nearest the bike ([`crate::flight::Flight::flown`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Flown {
    pub seat: usize,
    pub collider: Collider,
    pub body: crate::biplane::Body,
    pub mass: F,
}

impl Bump {
    /// The list resolved against the bike's chassis, as its ground step does at the top of a substep.
    /// `first` is the frame's first substep, the only one with impulses.
    pub fn resolve(&mut self, own: &mut Ridden, first: bool) {
        if self.contacts.is_empty() {
            return;
        }
        let mut mine = self.colliders[self.me];
        let mut planes: Vec<_> = self
            .planes
            .iter_mut()
            .map(|p| (&mut p.collider, contact::Weighed { body: &mut p.body, mass: p.mass }))
            .collect();
        let mut bodies: Vec<_> = self.props.iter_mut().map(crate::prop_contact::Pushed::weighed).collect();
        let mut others: Vec<Other<'_>> = self
            .colliders
            .iter_mut()
            .zip(self.vehicles.iter_mut())
            .map(|(collider, v)| Other { collider, side: Side::Vehicle(v) })
            .chain(planes.iter_mut().map(|(collider, body)| Other { collider: &mut **collider, side: Side::Body(body) }))
            .chain(bodies.iter_mut().map(|(collider, body)| Other { collider: &mut **collider, side: Side::Body(body) }))
            .collect();
        contact::resolve_ridden(&mut mine, own, &mut self.contacts, &mut others, &self.movers, first);
        self.colliders[self.me] = mine;
    }
}

/// A player's bike as the gather sees it; `seated` false is a seat nobody rides.
fn seat_of(r: &Rider, seated: bool) -> Seat<'_> {
    Seat { col: r.bike_col.as_deref(), at: r.ride.transform(), ghost: r.ghost.on, riding: seated && r.state.wreck.is_none() }
}

/// A player's chassis as the other side of a contact.
fn vehicle_of(r: &Rider, p: &Phy) -> Vehicle {
    vehicle_on(&r.ride, p)
}

fn vehicle_on(ride: &crate::ride::Ride, p: &Phy) -> Vehicle {
    let c = &ride.chassis;
    let rows = &c.inv_inertia.rows;
    Vehicle {
        node: node(&ride.transform()),
        cofg: c.cofg,
        mass: p.mass.mass,
        inv_mass: c.inv_mass,
        inv_inertia: [F(rows[0].x()), F(rows[1].y()), F(rows[2].z())],
        velocity: c.vel,
        rates: c.ang_vel,
        shock: Vf::ZERO,
        twist: Vf::ZERO,
    }
}

/// **The body rays against the world** (S138): the riding bike's eleven rays, from where the last tick
/// left it (`at`), searched through the world's props and trees as its collision object searches them
/// once a tick, and the contacts the search puts on its list. The original's object makes no
/// difference between a prop and another rider's frame: one search, one list, and the ground step
/// resolves it, a prop being a contact with no other side (the world does not move). Resolving it is
/// what loads the chassis' shock (`+0x170`) on a wall, and so the g peak the wreck test reads.
/// `None` when the world hands over no props or trees, where the ride's own probe stands in.
///
/// A prop that moves this tick (a path car's mesh) is struck with its mover (S145,
/// [`crate::flight_search::Scene`]): the sweep back meets it at its velocity, and the contacts on it
/// name it in the movers that come back with them, which [`Bump::movers`] carries to the resolve.
///
/// This one gathers a list of its own around the rays where they stand; the ride's tick searches the
/// bike's kept one ([`scenery_in`]).
pub fn scenery(col: &BikeCol, at: &Mat4, world: &mut dyn World, dt: F) -> Option<(Vec<Contact>, Vec<crate::contact::Mover>)> {
    scenery_in(col, at, &mut None, world, dt)
}

/// [`scenery`] through the bike's nearby search as last gathered (S189, [`crate::ride::Ride::nearby`]):
/// the body rays collide only against its list, a candidate only within their reach of where it was
/// measured from, so a bike whose list was gathered as a ghost meets no path car.
pub fn scenery_in(
    col: &BikeCol,
    at: &Mat4,
    nearby: &mut Option<Box<crate::broad_phase::Search>>,
    world: &mut dyn World,
    dt: F,
) -> Option<(Vec<Contact>, Vec<crate::contact::Mover>)> {
    let at = node(at);
    crate::ride::nearby_scene(nearby, world, &col.rays, &at, |scene| scenery_search(at, scene, dt))
}

fn scenery_search(at: Node, scene: &mut crate::flight_search::Scene<'_>, dt: F) -> (Vec<Contact>, Vec<crate::contact::Mover>) {
    let mut records = [Record {
        search_ground: false,
        search: true,
        sweep_back: true,
        rest_linear: F::ONE,
        rest_angular: F::ONE,
        node: at,
        capacity: CAPACITY,
        nearby: true,
        rays: Some(Vec::new()),
        ..Record::default()
    }];
    contact_search::collide(&mut records, 0, None, None, scene, dt);
    let movers = contact_search::Search::movers(scene);
    let [record] = records;
    (record.contacts, movers)
}

/// The up the ground step's end turns the record's ground search on at or under (P147): the bike's
/// transform's up `y`. Past it — on its side, upside down — its body rays look for the terrain.
pub const GROUND_SEARCH_UP: F = F(0.25);

/// Whether the riding bike's collision object searches the terrain this tick: its `+0x30`, which the
/// ground step's end sets from the transform it leaves, and the next tick's search reads. Off while
/// `0.25 < up.y`, so a NaN up searches.
pub fn searches_ground(at: &Mat4) -> bool {
    !(GROUND_SEARCH_UP < F(at.0[1].y()))
}

/// **The body rays in the ground** (P147): the riding bike's eleven rays, from where the last tick
/// left it (`at`), against the terrain, when [`searches_ground`] says the object looks. Each ray that
/// points down (against the transform's up) and ends inside the terrain's rectangle with its end below
/// the ground is a contact with no other side — the scenery, its surface the ground's
/// ([`contact_search::collide`]'s heightfield pass, S43). These are the first entries of the object's
/// list, ahead of what the actor search finds; the head ray among them is the rider's head in the
/// ground, which the wreck test's last clause wrecks at reason 6, whatever the airborne clause says.
/// Empty upright, and on a world with no terrain.
pub fn ground(col: &BikeCol, at: &Mat4, world: &mut dyn World, dt: F) -> Vec<Contact> {
    if !searches_ground(at) {
        return Vec::new();
    }
    let (field, _, _) = world.wooded();
    let Some(field) = field else { return Vec::new() };
    let rays = col.rays.segments.iter().map(|s| [s.from, s.to]).collect();
    let mut records = [Record {
        search_ground: true,
        search: true,
        sweep_back: true,
        rest_linear: F::ONE,
        rest_angular: F::ONE,
        node: node(at),
        capacity: CAPACITY,
        nearby: true,
        rays: Some(rays),
        ..Record::default()
    }];
    contact_search::collide(&mut records, 0, None, Some(&*field), &mut crate::wreck_bike::NoObjects, dt);
    let [record] = records;
    record.contacts
}

/// The wreck test's view of [`ground`]'s contacts: own entries, each by its ray's reach, on scenery.
pub fn ground_hits(contacts: &[Contact]) -> Vec<crate::air::Contact> {
    contacts
        .iter()
        .map(|c| crate::air::Contact { mirrored: false, reach: F(c.on_body.y()), other: crate::air::Other::Scenery })
        .collect()
}

/// **What of a bike's list the wreck test reads** (S272): [`crate::mover_contact::wreck_contacts`]'s
/// scenery entries, and with them, in the list's order, the own entries on a flown plane's hull
/// ([`Bump::planes`], other `players + k`) as the actor they are — a rigid body with no chassis, by
/// the plane's mass and its body's velocity ([`crate::air::Other::Object`]), which a bike lighter
/// than the plane wrecks on ([`crate::air::Wreck::Joust`]) when the entry's ray reaches the hit
/// clause. C16's joust: plane 0 taxied into P2's parked Race500P (the 50cc reach, 4 ft), whose ray
/// reaching 4.58 struck the hull, and P2 wrecked 7 on that tick. Entries on another rider's bike
/// and on the knockable props are left out, as before (the head ray stands in for the riders,
/// [`crate::ride::hits_among`]). The plane's velocity is its body's as the bump carries it into the
/// frame.
pub fn wreck_contacts(b: &Bump) -> Vec<crate::air::Contact> {
    let players = b.vehicles.len();
    b.contacts
        .iter()
        .filter(|c| c.kind == 0)
        .filter_map(|c| {
            let other = match c.other {
                None => crate::air::Other::Scenery,
                Some(o) => {
                    let f = o.checked_sub(players).and_then(|k| b.planes.get(k))?;
                    crate::air::Other::Object { mass: f.mass, vel: f.body.velocity }
                }
            };
            Some(crate::air::Contact { mirrored: false, reach: F(c.on_body.y()), other })
        })
        .collect()
}

/// A bike alone in its list: `contacts` against its own object and chassis, the one seat there is.
pub fn alone(ride: &crate::ride::Ride, p: &Phy, contacts: Vec<Contact>) -> Bump {
    Bump { me: 0, contacts, colliders: vec![collider(&p.collisions, p.mass.mass)], vehicles: vec![vehicle_on(ride, p)], movers: Vec::new(), props: Vec::new(), planes: Vec::new() }
}

/// `riders`' contacts, or `None` when none of them carries its collision files.
fn contacts_of(riders: &[(&Rider, bool)], dt: F, near: Near<'_>) -> Option<Vec<Vec<Contact>>> {
    if riders.iter().all(|(r, _)| r.bike_col.is_none()) {
        return None;
    }
    let seats: Vec<Seat<'_>> = riders.iter().map(|&(r, seated)| seat_of(r, seated)).collect();
    Some(gather_near(&seats, dt, near))
}

/// The bump of player `me`, every other player's chassis where it is nearest `me`'s (`near`, MD44):
/// where [`gather_near`] struck it, so the levers the resolve measures from its centre are the
/// few feet they are, not a width.
fn bump_of(riders: &[(&Rider, &Phy)], me: usize, contacts: Vec<Contact>, near: Near<'_>) -> Bump {
    let from = riders[me].0.ride.transform().0[3];
    let vehicles = riders
        .iter()
        .enumerate()
        .map(|(j, &(r, p))| {
            let mut v = vehicle_of(r, p);
            if j != me {
                v.node.origin = near(from, v.node.origin);
            }
            v
        })
        .collect();
    Bump { me, contacts, colliders: riders.iter().map(|(_, p)| collider(&p.collisions, p.mass.mass)).collect(), vehicles, movers: Vec::new(), props: Vec::new(), planes: Vec::new() }
}

/// `at` as it is: the copy of a point on a world that does not wrap.
fn here(_: Vf, at: Vf) -> Vf {
    at
}

/// What `bump`'s resolution did to another player's chassis: its velocity and its rates. The two
/// impact accumulators it also adds into are emptied at the top of their own next step, and the two
/// rotated copies of the velocities are left as the original leaves them.
fn bumped_one(r: &mut Rider, v: &Vehicle) {
    r.ride.chassis.vel = v.velocity;
    r.ride.chassis.ang_vel = v.rates;
}

impl crate::Sim {
    fn phy_of(&self, i: usize) -> &Phy {
        if i == 0 { &self.phy } else { &self.others[i - 1].phy }
    }

    fn rider_of_mut(&mut self, i: usize) -> &mut Rider {
        if i == 0 { &mut self.rider } else { &mut self.others[i - 1].rider }
    }

    /// **The tick's rider-against-rider contacts**, one list a player, gathered before anyone is
    /// stepped; `None` when no bike carries its collision files, which is every session before S78.
    pub fn bike_contacts(&self, dt: F) -> Option<Vec<Vec<Contact>>> {
        let riders: Vec<(&Rider, bool)> = (0..self.players()).map(|i| (self.rider_of(i), true)).collect();
        contacts_of(&riders, dt, &here)
    }

    /// [`crate::Sim::bike_contacts`] on `world`: across its seams when it wraps (MD44,
    /// [`gather_near`]).
    pub fn bike_contacts_in(&self, dt: F, world: &dyn World) -> Option<Vec<Vec<Contact>>> {
        let riders: Vec<(&Rider, bool)> = (0..self.players()).map(|i| (self.rider_of(i), true)).collect();
        contacts_of(&riders, dt, &|from, at| crate::nearest_copy(world, from, at))
    }

    /// Player `me`'s contacts, with every player's object and chassis as they stand now.
    pub fn bump(&self, me: usize, contacts: Vec<Contact>) -> Bump {
        let riders: Vec<(&Rider, &Phy)> = (0..self.players()).map(|i| (self.rider_of(i), self.phy_of(i))).collect();
        bump_of(&riders, me, contacts, &here)
    }

    /// [`crate::Sim::bump`] on `world`: every other chassis where it is nearest `me`'s (MD44).
    pub fn bump_in(&self, me: usize, contacts: Vec<Contact>, world: &dyn World) -> Bump {
        let riders: Vec<(&Rider, &Phy)> = (0..self.players()).map(|i| (self.rider_of(i), self.phy_of(i))).collect();
        bump_of(&riders, me, contacts, &|from, at| crate::nearest_copy(world, from, at))
    }

    /// What `bump` did to the other players' chassis, written back ([`bumped_one`]).
    pub fn bumped(&mut self, bump: &Bump) {
        for (i, v) in bump.vehicles.iter().enumerate() {
            if i != bump.me {
                bumped_one(self.rider_of_mut(i), v);
            }
        }
    }

    /// [`crate::Sim::step`] for a one-player Sim on its bike, with its rider-against-rider contacts:
    /// what an online seat, which is a Sim of its own, steps with ([`among`]).
    pub fn step_bumped(&mut self, dt: F, controls: &Controls, world: &mut dyn World, bump: Option<&mut Bump>) -> Frame {
        if bump.is_none() || !self.others.is_empty() || self.plane.is_some() {
            return self.step(dt, controls, world);
        }
        // The props' step waits for the bike's strike and push (S197a, [`crate::prop_contact`]), the
        // wreck bike on their node (S243).
        let prop_movers = self.frame_held_wrecked(dt, world, &[(0, self.players())]);
        self.lend_cars();
        let mut bump = bump;
        let cars = self.car_contacts(0, world, dt);
        if let Some(b) = bump.as_deref_mut() {
            b.contacts = crate::mover_contact::merged(std::mem::take(&mut b.contacts), cars);
            let strike = self.prop_strike_seated(b.vehicles.len() + b.planes.len(), dt);
            self.strike_into(b, strike);
        }
        // The wheels stand on the props for the frame (S210, [`crate::prop_wheel`]).
        self.lend_props(0);
        let said = self.rider.frame_bumped(&self.phy, &self.tables, controls, dt, world, &[], bump.as_deref_mut());
        self.props_back(0);
        if let Some(b) = bump.as_deref() {
            self.traffic.props.pushed(&b.props);
        }
        self.wreck_pushed_back(0);
        self.props_step(dt, &prop_movers);
        self.traffic.frame_late(dt, world);
        self.ticks += 1;
        said
    }
}

/// **Online, one Sim a seat** (N1): every seat's player 1 against every other's, `seated` saying who
/// rides (a seat with nobody on it is in no list, nor is one flying the plane). Each seat's list, or
/// `None` when no bike carries its collision files. A pure function of the seats' states, so every
/// machine, and every rollback's re-run, finds the same.
pub fn among(sims: &[Sim], seated: &[bool], dt: F) -> Option<Vec<Vec<Contact>>> {
    let riders: Vec<(&Rider, bool)> = sims
        .iter()
        .enumerate()
        .map(|(i, s)| (&s.rider, seated.get(i).copied().unwrap_or(false) && s.plane.is_none()))
        .collect();
    contacts_of(&riders, dt, &here)
}

/// [`among`] on `world`: across its seams when it wraps (MD44, [`gather_near`]).
pub fn among_in(sims: &[Sim], seated: &[bool], dt: F, world: &dyn World) -> Option<Vec<Vec<Contact>>> {
    let riders: Vec<(&Rider, bool)> = sims
        .iter()
        .enumerate()
        .map(|(i, s)| (&s.rider, seated.get(i).copied().unwrap_or(false) && s.plane.is_none()))
        .collect();
    contacts_of(&riders, dt, &|from, at| crate::nearest_copy(world, from, at))
}

/// `riders`' bodies as they stand, `me`'s contacts: what the seats' shared props step strikes
/// (S197c, [`crate::prop_seats`]), as [`crate::Sim::bump`] is for one Sim's.
pub(crate) fn bump_here(riders: &[(&Rider, &Phy)], me: usize, contacts: Vec<Contact>) -> Bump {
    bump_of(riders, me, contacts, &here)
}

/// Seat `me`'s contacts, for [`Sim::step_bumped`].
pub fn bump_among(sims: &[Sim], me: usize, contacts: Vec<Contact>) -> Bump {
    let riders: Vec<(&Rider, &Phy)> = sims.iter().map(|s| (&s.rider, &*s.phy)).collect();
    bump_of(&riders, me, contacts, &here)
}

/// [`bump_among`] on `world`: every other seat's chassis where it is nearest `me`'s (MD44).
pub fn bump_among_in(sims: &[Sim], me: usize, contacts: Vec<Contact>, world: &dyn World) -> Bump {
    let riders: Vec<(&Rider, &Phy)> = sims.iter().map(|s| (&s.rider, &*s.phy)).collect();
    bump_of(&riders, me, contacts, &|from, at| crate::nearest_copy(world, from, at))
}

/// What seat `bump.me`'s step did to the other seats' chassis, written back.
pub fn bumped_among(sims: &mut [Sim], bump: &Bump) {
    for (i, (s, v)) in sims.iter_mut().zip(&bump.vehicles).enumerate() {
        if i != bump.me {
            bumped_one(&mut s.rider, v);
        }
    }
}

/// **Riders as the shell holds them** (N22): split screen's seats are one-player Sims of their own,
/// as online's are, but each inside its seat, so these take the riders themselves rather than a
/// slice of Sims — each with whether it is in the others' world this tick (on its bike, its pad in,
/// not frozen). [`among_in`] on them.
pub fn contacts_among(riders: &[(&Rider, bool)], dt: F, world: &dyn World) -> Option<Vec<Vec<Contact>>> {
    contacts_of(riders, dt, &|from, at| crate::nearest_copy(world, from, at))
}

/// [`contacts_among`] with the flown planes' `hulls` in the riders' world (N22a, [`gather_met`]):
/// each rider's list, then each hull's mirrors in `hulls`' order.
pub fn contacts_met(riders: &[(&Rider, bool)], hulls: &[Hull<'_>], dt: F, world: &dyn World) -> Option<(Vec<Vec<Contact>>, Vec<Vec<Contact>>)> {
    if riders.iter().all(|(r, _)| r.bike_col.is_none()) {
        return None;
    }
    let seats: Vec<Seat<'_>> = riders.iter().map(|&(r, seated)| seat_of(r, seated)).collect();
    Some(gather_met(&seats, hulls, dt, &|from, at| crate::nearest_copy(world, from, at)))
}

/// [`among_in`] with the seats' flown planes in the riders' world (N22a, [`contacts_met`]): every
/// seat whose [`Sim::plane`] has its hull, in seat order, while `seated`.
pub fn among_met(sims: &[Sim], seated: &[bool], dt: F, world: &dyn World) -> Option<(Vec<Vec<Contact>>, Vec<Vec<Contact>>)> {
    let riders: Vec<(&Rider, bool)> = sims
        .iter()
        .enumerate()
        .map(|(i, s)| (&s.rider, seated.get(i).copied().unwrap_or(false) && s.plane.is_none()))
        .collect();
    let hulls: Vec<Hull<'_>> = sims
        .iter()
        .enumerate()
        .filter(|&(i, _)| seated.get(i).copied().unwrap_or(false))
        .filter_map(|(i, s)| s.plane.as_ref()?.hull(i))
        .collect();
    contacts_met(&riders, &hulls, dt, world)
}

/// The riders' bikes as a flown plane's mirrors meet them (N22a, [`crate::flight::Met`]): the plane's
/// share of [`gather_met`], every rider's object and chassis by player, where it is nearest the
/// plane (`flying`, the plane's body) on a world that wraps.
pub fn met_riders(riders: &[(&Rider, &Phy)], mirrors: Vec<Contact>, flying: Vf, world: &dyn World) -> crate::flight::Met {
    let vehicles = riders
        .iter()
        .map(|&(r, p)| {
            let mut v = vehicle_of(r, p);
            v.node.origin = crate::nearest_copy(world, flying, v.node.origin);
            v
        })
        .collect();
    let colliders = riders.iter().map(|(_, p)| collider(&p.collisions, p.mass.mass)).collect();
    crate::flight::Met { mirrors, colliders, vehicles, ..crate::flight::Met::default() }
}

/// [`met_riders`] on online's seats.
pub fn met_among(sims: &[Sim], mirrors: Vec<Contact>, flying: Vf, world: &dyn World) -> crate::flight::Met {
    let riders: Vec<(&Rider, &Phy)> = sims.iter().map(|s| (&s.rider, &*s.phy)).collect();
    met_riders(&riders, mirrors, flying, world)
}

/// **Seat `me`'s flown plane among the riders' bikes** (N22b): [`met_riders`], with the bikes its
/// rays strike named ([`crate::flight::Met::bikes`]) — every other player, in [`plane_bikes`]'
/// order — and the plane as the bikes' lists call it, `named` (the riders' count and its place among
/// the flown planes, [`Bump::planes`]).
pub fn met_plane(riders: &[(&Rider, &Phy)], me: usize, named: usize, mirrors: Vec<Contact>, flying: Vf, world: &dyn World) -> crate::flight::Met {
    let mut met = met_riders(riders, mirrors, flying, world);
    met.bikes = (0..riders.len()).filter(|&j| j != me).collect();
    met.named = Some(named);
    met
}

/// [`met_plane`] on online's seats.
pub fn met_plane_among(sims: &[Sim], me: usize, named: usize, mirrors: Vec<Contact>, flying: Vf, world: &dyn World) -> crate::flight::Met {
    let riders: Vec<(&Rider, &Phy)> = sims.iter().map(|s| (&s.rider, &*s.phy)).collect();
    met_plane(&riders, me, named, mirrors, flying, world)
}

/// **A bike's list with the planes' strikes on it** (N22b): the mirrors the flown planes' own
/// searches put on bike `me`'s record (each plane's [`crate::flight::Met::struck`], in the planes'
/// order), then the bike's own gather's list, up to the record's [`CAPACITY`] — the original's node
/// empties every list, then searches the planes before the riders, and a full record takes no more.
/// Each mirror's point is moved to the copy nearest the bike (`at`) on a world that wraps, as
/// [`gather_met`]'s are.
pub fn struck_first(struck: &[Vec<Contact>], own: Vec<Contact>, at: Vf, world: &dyn World) -> Vec<Contact> {
    let mut list: Vec<Contact> = struck
        .iter()
        .flatten()
        .map(|&c| Contact { point: crate::nearest_copy(world, at, c.point), ..c })
        .collect();
    list.extend(own);
    list.truncate(CAPACITY);
    list
}

/// **The bikes a plane pushed** (N22b): each rider an entry of the plane's own named takes its
/// chassis' speeds back from the plane's resolve ([`crate::flight::Met::pushed`]), as
/// [`bumped_riders`] takes them from a bike's.
pub fn pushed_riders(riders: &mut [&mut Rider], met: &crate::flight::Met) {
    for ((r, v), _) in riders.iter_mut().zip(&met.vehicles).zip(&met.pushed).filter(|(_, p)| **p) {
        bumped_one(r, v);
    }
}

/// [`pushed_riders`] on online's seats.
pub fn pushed_among(sims: &mut [Sim], met: &crate::flight::Met) {
    for ((s, v), _) in sims.iter_mut().zip(&met.vehicles).zip(&met.pushed).filter(|(_, p)| **p) {
        bumped_one(&mut s.rider, v);
    }
}

/// [`bump_among_in`] on riders as the shell holds them (N22).
pub fn bump_riders(riders: &[(&Rider, &Phy)], me: usize, contacts: Vec<Contact>, world: &dyn World) -> Bump {
    bump_of(riders, me, contacts, &|from, at| crate::nearest_copy(world, from, at))
}

/// [`bumped_among`] on riders as the shell holds them (N22).
pub fn bumped_riders(riders: &mut [&mut Rider], bump: &Bump) {
    for (i, (r, v)) in riders.iter_mut().zip(&bump.vehicles).enumerate() {
        if i != bump.me {
            bumped_one(r, v);
        }
    }
}

/// **The riders `me` can joust** (S224): every other rider in the world, unwrecked and no ghost,
/// where they stand now — what [`crate::Sim::step_all`] hands each rider's frame, built for seats
/// that are Sims of their own (online, split screen). On a world that wraps, each where it is
/// nearest `me`. A pure function of the riders' states, so every machine and every rollback's
/// re-run builds the same.
pub fn rivals_among(riders: &[(&Rider, bool)], me: usize, world: &dyn World) -> Vec<crate::ride::Rival> {
    let from = riders[me].0.ride.chassis.cofg_world;
    riders
        .iter()
        .enumerate()
        .filter(|&(j, &(r, on))| j != me && on && r.state.wreck.is_none() && !r.ghost.on)
        .map(|(_, &(r, _))| {
            let mut rival = r.ride.rival();
            rival.centre = crate::nearest_copy(world, from, rival.centre);
            rival
        })
        .collect()
}

/// **The other riders' bikes in seat `me`'s plane's world** (S225), as [`crate::Sim::step`] builds
/// them for player 1's plane (S95): every other seat's chassis, in the world while it is on its bike
/// (`on`), unwrecked and no ghost, and where it is nearest the plane (`flying`) on a world that
/// wraps. `me`'s own bike is out: the original takes it out of the world when its rider switches.
pub fn plane_bikes(riders: &[(&Rider, bool)], me: usize, flying: Vf, world: &dyn World) -> Vec<crate::flight_search::Bike> {
    riders
        .iter()
        .enumerate()
        .filter(|&(j, _)| j != me)
        .map(|(_, &(r, on))| {
            let mut transform = r.ride.transform();
            transform.0[3] = crate::nearest_copy(world, flying, transform.0[3]);
            crate::flight_search::Bike { transform, in_world: on && r.state.wreck.is_none() && !r.ghost.on }
        })
        .collect()
}

impl Sim {
    /// [`Sim::step_bumped`] with the riders `me` can joust as well (S224, [`rivals_among`]): a
    /// one-player Sim's bike step as [`Sim::step`]'s — its cars, the event, the seam — with the
    /// other seats' contacts and head-on rivals in its frame, as [`Sim::step_all`] gives each of its
    /// riders. With neither, and on the plane, it is [`Sim::step`].
    pub fn step_among(
        &mut self,
        dt: F,
        controls: &Controls,
        world: &mut dyn World,
        bump: Option<&mut Bump>,
        rivals: &[crate::ride::Rival],
    ) -> Frame {
        self.step_seat(dt, controls, world, bump, rivals, None)
    }

    /// [`Sim::step_among`] for seat `seat` of seats that share one prop set (S197c,
    /// [`crate::prop_seats`]): the set is lent to this Sim for its bike's strike and frame, the strike's
    /// mirrors naming it by its seat, and the props' collide pass (the first seat stepped runs it) and
    /// step are the set's, once a tick ([`crate::prop_seats::Shared::finish`]). Alone it still strikes
    /// the shared set. On the plane it is [`Sim::step`], the set untouched.
    #[allow(clippy::too_many_arguments)]
    pub fn step_shared(
        &mut self,
        dt: F,
        controls: &Controls,
        world: &mut dyn World,
        bump: Option<&mut Bump>,
        rivals: &[crate::ride::Rival],
        shared: &mut crate::prop_seats::Shared,
        seat: usize,
    ) -> Frame {
        self.step_seat(dt, controls, world, bump, rivals, Some((shared, seat)))
    }

    fn step_seat(
        &mut self,
        dt: F,
        controls: &Controls,
        world: &mut dyn World,
        bump: Option<&mut Bump>,
        rivals: &[crate::ride::Rival],
        mut shared: Option<(&mut crate::prop_seats::Shared, usize)>,
    ) -> Frame {
        let alone = bump.is_none() && rivals.is_empty() && shared.is_none();
        if alone || !self.others.is_empty() || self.plane.is_some() {
            return self.step(dt, controls, world);
        }
        // The props' step waits for the bike's strike and push (S197a, [`crate::prop_contact`]).
        // Shared, the first seat stepped this tick runs their collide pass, in its traffic's place.
        let prop_movers = match shared.as_mut() {
            Some((s, _)) => s.held(self, dt, world),
            None => self.frame_held_wrecked(dt, world, &[(0, self.players())]),
        };
        self.lend_cars();
        self.event_frame(dt);
        let cars = self.car_contacts(0, world, dt);
        let players = bump.as_deref().map_or(1, |b| b.vehicles.len() + b.planes.len());
        let strike = match shared.as_ref() {
            Some((_, seat)) => self.prop_strike_shared(*seat, players, dt),
            None => self.prop_strike_seated(players, dt),
        };
        let mut own;
        let bump = match bump {
            Some(b) => {
                b.contacts = crate::mover_contact::merged(std::mem::take(&mut b.contacts), cars);
                Some(b)
            }
            None if cars.is_empty() && strike.contacts.is_empty() => None,
            None => {
                own = self.bump(0, cars);
                Some(&mut own)
            }
        };
        let mut bump = bump;
        if let Some(b) = bump.as_deref_mut() {
            self.strike_into(b, strike);
        }
        // Shared, the seat's wreck bike was on the props' node with every seat's (S260): what their
        // rays struck on it goes to its frame; alone, it was on the node too (S243).
        if let Some((s, seat)) = shared.as_mut() {
            s.wreck_struck(self, *seat);
        }
        // The wheels stand on the props for the frame (S210, [`crate::prop_wheel`]).
        self.lend_props(0);
        let mut said = self.rider.frame_bumped(&self.phy, &self.tables, controls, dt, world, rivals, bump.as_deref_mut());
        self.props_back(0);
        if let Some(b) = bump.as_deref() {
            self.traffic.props.pushed(&b.props);
        }
        self.wreck_pushed_back(0);
        // Shared, the set goes back out before the props' step, which is the set's.
        if let Some((s, _)) = shared.as_mut() {
            s.back(self);
        }
        self.props_step(dt, &prop_movers);
        self.traffic.frame_late(dt, world);
        said.seam = self.rider.wrap_round(&*world);
        self.ticks += 1;
        said
    }
}
