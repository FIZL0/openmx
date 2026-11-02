//! **Wheels stand on the knockable props** (S210): the riding bike's wheel probe (its two segments,
//! [`crate::ride::wheel_rays`]) against every prop's collidable mesh (its model's `<node>f.col`,
//! [`PropSet::shapes`]) where the prop's body node stands it, in the same record as the scene's hits.
//!
//! What the original does, read on S195's knock states (free09, `tests/prop_wheel.rs`): every
//! prop's `f.col` is category `0x18`, its nearby bit set asleep and awake, no mover (`+0x7c`) and its
//! owner (`+0x80`) the prop's collision object, so the wheel probe's pair filter (which leaves out
//! only `[0x100, 0x2000]`) accepts it both ways, and the balls round the bike are in its nearby
//! search. So a wheel on a prop is:
//!
//! 1. a contact of the wheel probe's record like any other, one a segment, the nearer kept
//!    ([`ray_probe`]'s rule): here the props are tried after the scene, on the scene's record;
//! 2. a hit the `f.col`'s one listener hears — the prop's struck listener, which enables the body and
//!    activates its collection ([`PropSet::struck`]), so a wheel rolling onto a bale wakes it as a body
//!    ray striking it does (S197);
//! 3. the distributor's owner word (`+0x1f0`) set to the prop's collision object and no vehicle
//!    (`+0x1f4` 0), so the contact-velocity step takes its owner branch, which it asks before the
//!    vehicle: the body's velocity and its velocity at the contact point ([`PropSet::under`], S169,
//!    proved against the original's step). The port's owner word is [`handle`], the prop's place in
//!    the set, never zero.
//!
//! The props are the [`crate::Sim`]'s ([`crate::traffic::Traffic::props`]): lent to the riding bike
//! for its frame ([`crate::ride::Ride::props`], [`crate::Sim::lend_props`]) and taken back after it,
//! so between frames the ride holds none and its hash and snapshot are as before. A body the bike's
//! body rays struck this tick is pushed on a copy in the frame's bump
//! ([`crate::prop_contact::Pushed`]), and a wheel on it reads that copy, as the original reads the
//! one body the resolve pushes.

#![deny(clippy::float_arithmetic)]

use crate::bike_col;
use crate::contact::{self, Other, Side};
use crate::flight_search::{self, Rays};
use crate::math::Mat4;
use crate::prop_contact::Pushed;
use crate::prop_set::PropSet;
use crate::ray_probe::{self, Facet, RayProbe};
use crate::track::Mesh;
use crate::world::Prop;
use crate::world_search::{self, Candidate, Categories, Class, Collidable, Narrow, Nearby, Objects, flags};
use ps2_float::{F, Vf, fpu};

/// Where a prop's contact names its mesh in the wheel probe's record (`ray_probe::Contact::object`):
/// past any of the scene's collidables.
pub const FIRST: u32 = 0x0100_0000;

/// The wheel's owner word for a prop: 1 + its place among every collection's objects, in order.
pub fn handle(set: &PropSet, collection: usize, object: usize) -> u32 {
    let before: usize = set.collections[..collection].iter().map(|c| c.objects.len()).sum();
    (1 + before + object) as u32
}

/// The prop an owner word names: its collection and its object. `None` for 0 or a word past the set.
pub fn named(set: &PropSet, owner: u32) -> Option<(usize, usize)> {
    let mut k = (owner as usize).checked_sub(1)?;
    for (c, coll) in set.collections.iter().enumerate() {
        if k < coll.objects.len() {
            return Some((c, k));
        }
        k -= coll.objects.len();
    }
    None
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

/// The wheel probe's narrow phase: collidable `1 + k` is prop `k`, the probe is collidable 0.
struct Wheels<'a> {
    rays: &'a Rays,
    at: Mat4,
    posed: &'a [Posed<'a>],
    facets: Vec<Option<Vec<Facet>>>,
    ray: ray_probe::Record,
    /// The props whose struck listener heard a hit, in the order they heard it.
    heard: Vec<usize>,
}

impl Narrow for Wheels<'_> {
    // The record goes on from the scene's search, which cleared it: nothing is emptied here.
    fn clear(&mut self, _objects: &mut [Collidable], _probe: usize) {}

    fn prepare(&mut self, _objects: &mut [Collidable], _probe: usize) {}

    fn pair(&mut self, _objects: &mut [Collidable], _probe: usize, other: usize) -> bool {
        let k = other - 1;
        let p = &self.posed[k];
        let facets = self.facets[k].get_or_insert_with(|| flight_search::facets(p.mesh));
        let mesh = ray_probe::Mesh {
            id: FIRST + k as u32,
            transform: p.to_world.0,
            root: 0,
            nodes: &p.mesh.nodes,
            facets,
            vertices: &p.mesh.vertices,
        };
        let probe = RayProbe { transform: self.at.0, root: self.rays.root, nodes: &self.rays.nodes, segments: &self.rays.segments };
        ray_probe::pair(&probe, &ray_probe::Shape::Mesh(mesh), &mut self.ray)
    }

    fn parts(&mut self, _objects: &mut [Collidable], _group: usize) -> usize {
        0
    }

    fn part(&mut self, _objects: &mut [Collidable], _group: usize, _i: usize) -> Option<usize> {
        None
    }

    fn heard_hit(&mut self, _objects: &mut [Collidable], _listener: usize, _probe: usize, _other: usize) {}

    /// The `f.col`'s one listener, the prop's struck listener: the body is woken after the search.
    fn heard_struck(&mut self, _objects: &mut [Collidable], _listener: usize, struck: usize, _probe: usize) {
        if !self.heard.contains(&(struck - 1)) {
            self.heard.push(struck - 1);
        }
    }

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

/// What [`search`] found on the props: each contact it left in the record that is a prop's, by the
/// record's place, with the prop's owner word and its mesh's material.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Found {
    pub on: Vec<(usize, u32, u8)>,
}

/// **The wheel probe `rays` on the chassis at `at` against the props of `set`**, on `ray`, the
/// wheel probe's record as the scene's search left it: each prop with a collidable mesh within the
/// probe's reach of its sphere, in the set's order, its contacts kept by the record's one-a-segment
/// rule. Every prop struck is woken ([`PropSet::struck`]), in the order it was first struck. Answers
/// which of the record's contacts are on a prop.
pub fn search(rays: &Rays, at: &Mat4, set: &mut PropSet, ray: &mut ray_probe::Record) -> Found {
    if set.is_empty() {
        return Found::default();
    }
    let shapes = set.shapes.clone();
    let posed: Vec<Posed<'_>> = set
        .collections
        .iter()
        .zip(shapes.iter())
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
        return Found::default();
    }
    let (centre, reach) = flight_search::reach(rays, &at.0);
    // The wheel probe as the bike builds it (the second ray probe, [`bike_col::collidables`]): its
    // category list leaves out the props' own `0x100` and the bikes' actor word, never `0x18`.
    let start = bike_col::RIDE_START[0] | bike_col::RIDE_START[1];
    let mut objects = vec![Collidable {
        class: Class::Object,
        flags: 0x02,
        nearby: true,
        radius: reach,
        category: start,
        collides_with: u32::MAX,
        objects: Objects { mode: 1, list: Vec::new() },
        more_objects: Objects { mode: 1, list: Vec::new() },
        categories: Categories { mode: 1, list: vec![bike_col::SECOND_IGNORES, bike_col::ACTOR] },
        ..Collidable::default()
    }];
    objects.extend(posed.iter().map(|p| Collidable {
        class: Class::Object,
        flags: flags::ENABLED,
        nearby: true,
        radius: p.radius,
        category: crate::prop_world::COLLIDABLE,
        collides_with: crate::prop_world::COLLIDES_WITH,
        categories: Categories { mode: 1, list: Vec::new() },
        listeners: vec![0],
        ..Collidable::default()
    }));
    let found = posed
        .iter()
        .enumerate()
        .map(|(k, p)| Candidate { object: 1 + k, distance: distance(centre, p.centre) - p.radius })
        .collect();
    let mut wheels = Wheels {
        rays,
        at: *at,
        posed: &posed,
        facets: (0..posed.len()).map(|_| None).collect(),
        ray: std::mem::take(ray),
        heard: Vec::new(),
    };
    world_search::search_nearby(&mut objects, 0, &Nearby { found, prepare: false }, &mut wheels);
    *ray = std::mem::take(&mut wheels.ray);
    let on = ray
        .contacts
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let k = c.object.checked_sub(FIRST)? as usize;
            let p = &posed[k];
            let material = p.mesh.surface.map_or(0, |s| s as u8);
            Some((i, handle(set, p.collection, p.object), material))
        })
        .collect();
    let heard: Vec<(usize, usize)> = wheels.heard.iter().map(|&k| (posed[k].collection, posed[k].object)).collect();
    drop(posed);
    for (c, o) in heard {
        set.struck(c, o);
    }
    Found { on }
}

/// **What a wheel whose owner word is `owner` stands on** (S210): the prop's velocity and its
/// velocity at the world `point` ([`PropSet::under`], [`contact::other_at`]'s body side). A body the
/// frame's bump is pushing (`pushed`, the body rays struck it this tick) is read off that copy, the
/// one the resolve moves. `None` for an owner that is not a prop of `set`, or one with no collision
/// object.
pub fn under(set: &mut PropSet, pushed: &mut [Pushed], owner: u32, point: Vf) -> Option<(Vf, Vf)> {
    let (c, o) = named(set, owner)?;
    if let Some(p) = pushed.iter_mut().find(|p| p.collection == c && p.object == o) {
        let (collider, mut body) = p.weighed();
        return Some(contact::other_at(&mut Other { collider, side: Side::Body(&mut body) }, point));
    }
    set.under(c, o, point)
}

/// The props as a ride holds them for its frame: the set, and its pushed copies' place is the frame's
/// bump; and the traffic's physics objects, whose woken trucks the wheels stand on too (S208,
/// [`crate::truck_wheel`]). A newtype so the ride can print itself without printing every prop.
#[derive(Clone, Default)]
pub struct Lent(pub PropSet, pub crate::truck_wheel::Trucks);

impl std::fmt::Debug for Lent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Lent({} collections)", self.0.collections.len())
    }
}

impl crate::Sim {
    /// Lend the props to player `i`'s bike for its frame (S210, [`crate::ride::Ride::props`]): its
    /// wheel probe searches them, wakes what it strikes and reads what it stands on. The traffic's
    /// physics objects go with them while a truck is woken (S208, [`crate::truck_wheel`]). Nothing is
    /// lent on a track with no props and no woken truck. [`crate::Sim::props_back`] takes them back.
    pub fn lend_props(&mut self, i: usize) {
        let trucks = crate::truck_wheel::Trucks::any(&self.traffic);
        if self.traffic.props.is_empty() && !trucks {
            return;
        }
        let set = std::mem::take(&mut self.traffic.props);
        let trucks = if trucks { crate::truck_wheel::Trucks::take(&mut self.traffic) } else { Default::default() };
        let rider = if i == 0 { &mut self.rider } else { &mut self.others[i - 1].rider };
        rider.ride.props = Some(Box::new(Lent(set, trucks)));
    }

    /// The props player `i`'s bike was lent ([`crate::Sim::lend_props`]), back where they are kept.
    pub fn props_back(&mut self, i: usize) {
        let rider = if i == 0 { &mut self.rider } else { &mut self.others[i - 1].rider };
        if let Some(set) = rider.ride.props.take() {
            let Lent(props, trucks) = *set;
            self.traffic.props = props;
            if !trucks.cars.is_empty() {
                self.traffic.physics = trucks.cars;
            }
        }
    }
}
