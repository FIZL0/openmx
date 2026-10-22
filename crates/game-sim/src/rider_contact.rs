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
//! [`contact_search::sweep_back`] does not port), the riding bike's rays against the world's props
//! (the ride's own bundle stands in for them), and the broad phase's gather, which is replaced by
//! every other rider's frame in player order with its distance from the rays' centre.

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
/// * `Friction`, `BodyFriction` (-1 copies), and `TerrainFriction` for all sixteen surfaces.
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
        surfaces: [c.terrain_friction; 16],
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

/// The narrow phase of one bike's body rays: every other bike's frame mesh, struck as its player.
struct Strikes<'a> {
    seats: &'a [Seat<'a>],
    ray: ray_probe::Record,
}

impl Narrow for Strikes<'_> {
    fn clear(&mut self, objects: &mut [Collidable], probe: usize) {
        ray_probe::clear(&mut objects[probe].flags, &mut self.ray);
    }

    fn prepare(&mut self, _objects: &mut [Collidable], _probe: usize) {}

    fn pair(&mut self, _objects: &mut [Collidable], probe: usize, other: usize) -> bool {
        let (i, j) = (probe / 4, other / 4);
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
    /// The other bikes' frames, in player order, with their distance from the rays' centre less the
    /// frame's reach: a ghost's is not in the broad phase, nor is one off the list.
    fn gathered(&self) -> Nearby {
        let seats = self.strikes.seats;
        let Some(mine) = seats[self.me].col else { return Nearby::default() };
        let (centre, _) = crate::flight_search::reach(&mine.rays, &seats[self.me].at.0);
        let found = seats
            .iter()
            .enumerate()
            .filter(|&(j, s)| j != self.me && s.listed() && !s.ghost)
            .filter_map(|(j, s)| {
                let col = s.col?;
                let root = col.frame.nodes.first()?;
                let theirs = placed(&s.at.0, root.centre);
                Some(Candidate { object: 4 * j + bike_col::FRAME, distance: distance(centre, theirs) - col.frame_radius })
            })
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
                let struck = Struck { object: 4 * j + bike_col::FRAME, other: Some(j), mover: None };
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
    let objects = collidables(seats);
    for me in 0..seats.len() {
        if !seats[me].listed() {
            continue;
        }
        let ray = ray_probe::Record { listed: true, capacity: CAPACITY, one_per_ray: true, ..ray_probe::Record::default() };
        let mut probe = Probe { strikes: Strikes { seats, ray }, objects: objects.clone(), me };
        contact_search::collide(&mut records, me, None, None, &mut probe, dt);
    }
    records.into_iter().map(|r| r.contacts).collect()
}

/// **One bike's contacts while it is stepped**: its list, every player's object, and every other
/// player's chassis as it stands — which the resolution pushes, and the caller writes back.
#[derive(Clone, Debug, Default)]
pub struct Bump {
    pub me: usize,
    pub contacts: Vec<Contact>,
    pub colliders: Vec<Collider>,
    pub vehicles: Vec<Vehicle>,
}

impl Bump {
    /// The list resolved against the bike's chassis, as its ground step does at the top of a substep.
    /// `first` is the frame's first substep, the only one with impulses.
    pub fn resolve(&mut self, own: &mut Ridden, first: bool) {
        if self.contacts.is_empty() {
            return;
        }
        let mut mine = self.colliders[self.me];
        let mut others: Vec<Other<'_>> = self
            .colliders
            .iter_mut()
            .zip(self.vehicles.iter_mut())
            .map(|(collider, v)| Other { collider, side: Side::Vehicle(v) })
            .collect();
        contact::resolve_ridden(&mut mine, own, &mut self.contacts, &mut others, &[], first);
        self.colliders[self.me] = mine;
    }
}

/// A player's bike as the gather sees it; `seated` false is a seat nobody rides.
fn seat_of(r: &Rider, seated: bool) -> Seat<'_> {
    Seat { col: r.bike_col.as_deref(), at: r.ride.transform(), ghost: r.ghost.on, riding: seated && r.state.wreck.is_none() }
}

/// A player's chassis as the other side of a contact.
fn vehicle_of(r: &Rider, p: &Phy) -> Vehicle {
    let c = &r.ride.chassis;
    let rows = &c.inv_inertia.rows;
    Vehicle {
        node: node(&r.ride.transform()),
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

/// `riders`' contacts, or `None` when none of them carries its collision files.
fn contacts_of(riders: &[(&Rider, bool)], dt: F) -> Option<Vec<Vec<Contact>>> {
    if riders.iter().all(|(r, _)| r.bike_col.is_none()) {
        return None;
    }
    let seats: Vec<Seat<'_>> = riders.iter().map(|&(r, seated)| seat_of(r, seated)).collect();
    Some(gather(&seats, dt))
}

fn bump_of(riders: &[(&Rider, &Phy)], me: usize, contacts: Vec<Contact>) -> Bump {
    Bump {
        me,
        contacts,
        colliders: riders.iter().map(|(_, p)| collider(&p.collisions, p.mass.mass)).collect(),
        vehicles: riders.iter().map(|&(r, p)| vehicle_of(r, p)).collect(),
    }
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
        contacts_of(&riders, dt)
    }

    /// Player `me`'s contacts, with every player's object and chassis as they stand now.
    pub fn bump(&self, me: usize, contacts: Vec<Contact>) -> Bump {
        let riders: Vec<(&Rider, &Phy)> = (0..self.players()).map(|i| (self.rider_of(i), self.phy_of(i))).collect();
        bump_of(&riders, me, contacts)
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
        let said = self.rider.frame_bumped(&self.phy, &self.tables, controls, dt, world, &[], bump);
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
    contacts_of(&riders, dt)
}

/// Seat `me`'s contacts, for [`Sim::step_bumped`].
pub fn bump_among(sims: &[Sim], me: usize, contacts: Vec<Contact>) -> Bump {
    let riders: Vec<(&Rider, &Phy)> = sims.iter().map(|s| (&s.rider, &*s.phy)).collect();
    bump_of(&riders, me, contacts)
}

/// What seat `bump.me`'s step did to the other seats' chassis, written back.
pub fn bumped_among(sims: &mut [Sim], bump: &Bump) {
    for (i, (s, v)) in sims.iter_mut().zip(&bump.vehicles).enumerate() {
        if i != bump.me {
            bumped_one(&mut s.rider, v);
        }
    }
}
