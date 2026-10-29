//! **The bike's body rays against the moving cars** (S137): the Dallas train's cars (S81,
//! [`crate::traffic`]) struck by a riding bike's eleven body rays, as scenery contacts on its
//! collision record.
//!
//! The ride's own probes ([`crate::ride`]'s wheel probe, its one ray from the centre of mass along the
//! velocity, and the head ray the wreck test sweeps) stand in for the original's body rays against
//! the world. Against a train car they miss: its collision mesh is open underneath, the deck's
//! underside about three feet above the rails, so the centre-of-mass ray rides under it between the
//! bogies, and the head ray starts inside the car, where every face it meets is a back face. The
//! original's rays fan out of the hub at the rider's middle ([`crate::air::HEAD`]`.0`) and strike the
//! car's side, so this runs them, the files' own ([`BikeCol::rays`]), against every car where the
//! traffic last stood it:
//!
//! * once a tick, before the bike is stepped, as the rider-against-rider contacts are gathered
//!   ([`crate::rider_contact::gather`]): the same record, the same capacity, a car being what a prop
//!   is to the search, with no collision object ([`contact_search::collide`]) — but a mover, the car
//!   (S145: its mesh collidable's owner is the car, a vehicle of kind 2, as S142 read), named by its
//!   prop's index ([`crate::rider_contact::Bump::movers`]), so the response meets the car's motion;
//! * the list is resolved by the ground step at the top of every substep with the bike's scenery
//!   response ([`crate::rider_contact::Bump::resolve`]), the impulse the first substep's;
//! * and the wreck test reads it: the first entry whose ray reaches above the hit reach is a
//!   [`crate::air::Wreck::Scenery`] ([`wreck_contacts`]).
//!
//! The static props are not searched here: the body rays meet every prop, cars among them, in the
//! tick's scenery search ([`crate::rider_contact::scenery`], S138), which keeps this search's entry
//! for a ray both list; the wreck test reads both lists and no longer sweeps the head ray through
//! the props where that search ran (S152).

#![deny(clippy::float_arithmetic)]

use crate::air;
use crate::bike_col::{self, BikeCol};
use crate::contact::{Contact, Node};
use crate::contact_search::{self, Hit, Record, Search, Struck, Swept};
use crate::flight_search;
use crate::math::{self, Mat4};
use crate::ray_probe::{self, Facet};
use crate::rider_contact::{self, CAPACITY};
use crate::world::{Prop, World};
use crate::world_search::{self, Candidate, Categories, Class, Collidable, Narrow, Nearby, flags};
use ps2_float::{F, Vf, fpu};

/// The body rays' narrow phase: car `k` is collidable `CARS + k`, after the bike's own four.
struct Strikes<'a> {
    col: &'a BikeCol,
    at: Mat4,
    cars: &'a [Prop],
    facets: Vec<Option<Vec<Facet>>>,
    ray: ray_probe::Record,
}

/// Where the cars start in the search's list: after the bike's four collidables.
const CARS: usize = 4;

impl Narrow for Strikes<'_> {
    fn clear(&mut self, objects: &mut [Collidable], probe: usize) {
        ray_probe::clear(&mut objects[probe].flags, &mut self.ray);
    }

    fn prepare(&mut self, _objects: &mut [Collidable], _probe: usize) {}

    fn pair(&mut self, _objects: &mut [Collidable], _probe: usize, other: usize) -> bool {
        let Some(k) = other.checked_sub(CARS) else { return false };
        let p = &self.cars[k];
        let facets = self.facets[k].get_or_insert_with(|| flight_search::facets(&p.mesh));
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

/// The bike's search: its body rays against the cars gathered round them.
struct Probe<'a> {
    strikes: Strikes<'a>,
    objects: Vec<Collidable>,
    /// The first car's index among the world's props: what its mover is named by (S145).
    first: usize,
}

impl Probe<'_> {
    /// The cars in the traffic's order, each with its distance from the rays' centre less its reach:
    /// the broad phase's gather, which a moving list hands out in the order it was filed.
    fn gathered(&self) -> Nearby {
        let (centre, _) = flight_search::reach(&self.strikes.col.rays, &self.strikes.at.0);
        let found = self
            .strikes
            .cars
            .iter()
            .enumerate()
            .map(|(k, p)| Candidate { object: CARS + k, distance: distance(centre, p.centre) - p.radius })
            .collect();
        Nearby { found, prepare: false }
    }
}

impl Search for Probe<'_> {
    fn rays(&mut self, _nearby: bool) -> Vec<Hit> {
        let nearby = self.gathered();
        world_search::search_nearby(&mut self.objects, bike_col::BODY_RAYS, &nearby, &mut self.strikes);
        self.strikes
            .ray
            .contacts
            .iter()
            .map(|c| {
                let struck = Struck { object: c.object as usize, other: None, mover: self.mover(c.object as usize) };
                Hit { from: c.from, to: c.to, point: c.point, normal: c.normal, struck }
            })
            .collect()
    }

    fn sweep(&mut self, _from: Vf, _to: Vf, _nearby: bool) -> Option<Swept> {
        None
    }

    fn moved(&mut self, _node: &Node) {}

    /// A car's velocity, its mover's `vt+0xb8` (S145). The sweep is not run here, so nothing asks.
    fn velocity(&self, struck: &Struck) -> Vf {
        match struck.mover.and_then(|(m, _)| self.strikes.cars.get(m.checked_sub(self.first)?)?.moving) {
            Some(m) => m.velocity,
            None => Vf::ZERO,
        }
    }
}

impl Probe<'_> {
    /// Collidable `object`'s mover (S145): a car's collision mesh has no collision object (`+0x80` is
    /// 0) and its owner (`+0x7c`) is the car, a vehicle of kind 2 (S142), named by its prop's index
    /// among the world's ([`crate::flight_search::movers`]). A car the traffic has not yet stepped
    /// has no motion to give, and none.
    fn mover(&self, object: usize) -> Option<(usize, u32)> {
        let k = object.checked_sub(CARS)?;
        self.strikes.cars.get(k)?.moving?;
        Some((self.first + k, contact_search::MOVER_KIND))
    }
}

/// **A riding bike's contacts with the cars**: its body rays (`col`) on the chassis at `at`, against
/// `cars` where they stand. One entry per ray that struck, at most [`CAPACITY`]. `first` is the first
/// car's index among the world's props, which a contact names its car's mover by (S145).
pub fn struck(col: &BikeCol, at: &Mat4, cars: &[Prop], first: usize, dt: F) -> Vec<Contact> {
    if cars.is_empty() {
        return Vec::new();
    }
    let mut objects = bike_col::collidables(col, 0).to_vec();
    objects.extend(cars.iter().map(|p| Collidable {
        class: Class::Object,
        flags: flags::ENABLED,
        nearby: true,
        radius: p.radius,
        category: flight_search::SCENERY,
        collides_with: u32::MAX,
        categories: Categories { mode: 1, list: Vec::new() },
        ..Collidable::default()
    }));
    let ray = ray_probe::Record { listed: true, capacity: CAPACITY, one_per_ray: true, ..ray_probe::Record::default() };
    let strikes = Strikes { col, at: *at, cars, facets: vec![None; cars.len()], ray };
    let mut probe = Probe { strikes, objects, first };
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
    contact_search::collide(&mut records, 0, None, None, &mut probe, dt);
    let mut list = records.swap_remove(0).contacts;
    if let Some(e) = inside(col, at, cars, first).filter(|_| list.len() < CAPACITY) {
        if !list.iter().any(|c| c.from == e.from && c.on_body == e.on_body) {
            list.push(e);
        }
    }
    list
}

/// **A car that came over the head ray** (the port's own): the ray reaching highest
/// ([`air::HEAD`]'s, ray 8 of the files') with its end inside a car's root box, as a contact on the
/// box's nearest face (a side, an end or the roof; never the floor) at the end's place.
///
/// A ray that starts inside a mesh strikes nothing, its every face a back face, so a car whose end
/// sweeps into a bike standing beside the line, or a bike squeezed between two cars where they
/// meet, has nothing on its list while the train runs through it. The original's chassis sweep back
/// (not ported) answers the bike's own travel, not the car's. As the head ray's entry, the wreck test
/// reads it as scenery struck ([`wreck_contacts`]): a rider the train ran into comes off.
fn inside(col: &BikeCol, at: &Mat4, cars: &[Prop], first: usize) -> Option<Contact> {
    let head = col.rays.segments.iter().max_by(|a, b| a.to.y().total_cmp(&b.to.y()))?;
    let end = math::transform_point(at, head.to);
    for (k, p) in cars.iter().enumerate() {
        let Some(root) = p.mesh.nodes.first() else { continue };
        let local = math::transform_point(&p.from_world, end);
        let (c, h) = (root.centre, root.half_extent);
        let lo = [c[0] - h[0], c[1] - h[1], c[2] - h[2]];
        let hi = [c[0] + h[0], c[1] + h[1], c[2] + h[2]];
        let q = [F(local.x()), F(local.y()), F(local.z())];
        if (0..3).any(|i| !(lo[i] < q[i] && q[i] < hi[i])) {
            continue;
        }
        // The nearest face out, `(axis, outward sign, how far)`: x first, then z, then up.
        let faces = [(0, F::ONE, hi[0] - q[0]), (0, F(-1.0), q[0] - lo[0]), (2, F::ONE, hi[2] - q[2]),
                     (2, F(-1.0), q[2] - lo[2]), (1, F::ONE, hi[1] - q[1])];
        let (axis, sign, _) = faces.into_iter().fold(faces[0], |best, f| if f.2 < best.2 { f } else { best });
        let mut on = q;
        on[axis] = if sign > F::ZERO { hi[axis] } else { lo[axis] };
        let point = math::transform_point(&p.to_world, Vf::new(on[0].0, on[1].0, on[2].0, 1.0));
        let row = p.to_world.0[axis];
        let normal = Vf::new((F(row.x()) * sign).0, (F(row.y()) * sign).0, (F(row.z()) * sign).0, 0.0);
        return Some(Contact {
            kind: 0,
            from: head.from,
            on_body: head.to,
            normal,
            point,
            arm: Vf::VF0,
            other_arm: Vf::VF0,
            surface: contact_search::NO_SURFACE,
            other: None,
            mover: p.moving.map(|_| first + k),
        });
    }
    None
}

/// What of a bike's list the wreck test reads as scenery: its own entries (kind 0) with nothing on
/// the other side, each reaching as high as its ray's end in the bike's model space. The test reads
/// the other side's record (`+0x74`) alone, so an entry on a car, which names only its mover
/// (`+0x78`), is scenery (S145).
pub fn wreck_contacts(list: &[Contact]) -> Vec<air::Contact> {
    list.iter()
        .filter(|c| c.kind == 0 && c.other.is_none())
        .map(|c| air::Contact { mirrored: false, reach: F(c.on_body.y()), other: air::Other::Scenery })
        .collect()
}

/// `cars` added to the end of `list`, which keeps at most [`CAPACITY`] entries, as the record does.
pub fn merged(mut list: Vec<Contact>, cars: Vec<Contact>) -> Vec<Contact> {
    let room = CAPACITY.saturating_sub(list.len());
    list.extend(cars.into_iter().take(room));
    list
}

impl crate::Sim {
    /// Tell every rider where the cars' meshes lie in the world's props ([`crate::rider::Rider::cars`]),
    /// for a ghost's frame to pass through them (S215).
    pub fn lend_cars(&mut self) {
        let span = self.traffic.span();
        self.rider.cars = span;
        for p in &mut self.others {
            p.rider.cars = span;
        }
    }

    /// Player `i`'s contacts with the traffic's cars this tick ([`struck`]): none for a rider who is
    /// down (the wreck bike's object is not the riding one), a bike the shell gave no collision files,
    /// or a world with no cars in it.
    pub fn car_contacts(&self, i: usize, world: &mut dyn World, dt: F) -> Vec<Contact> {
        let Some(first) = self.traffic.first else { return Vec::new() };
        let r = self.rider_of(i);
        let Some(col) = r.bike_col.as_deref() else { return Vec::new() };
        // A ghost's body rays ignore the cars' category (S215, [`crate::traffic::Hidden`]).
        if r.state.wreck.is_some() || r.ghost.on {
            return Vec::new();
        }
        let Some(props) = world.moving() else { return Vec::new() };
        let cars = props.props.get(first..).unwrap_or(&[]);
        struck(col, &r.ride.transform(), cars, first, dt)
    }
}
