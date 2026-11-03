//! **The bike strikes a woken `HasPhysics` truck's body** (S251): a riding bike's eleven body rays
//! against each physical truck's `f.col` ([`PhysicsCar::shape`]) where its body stands it
//! ([`PhysicsCar::pose`]), as entries on the bike's collision record naming the truck's body on their
//! other side, and the mirrors on the truck's own record — what the knockable props' `f.col` is to the
//! same rays ([`crate::prop_contact`]).
//!
//! What the original does (S186, S208, the interpreter on s89-along): the wake (`physics_car_wake`)
//! stands the car's path mesh (`+0x428`) down, its `+0x31` bits 0 and 1 cleared, and files the physics
//! object's `f.col` (category `0x18`), its bits set; the put-back stands the path mesh up again. The
//! `f.col`'s collision object names the physics object's body (`+0xf4`) and no chassis, so to the bike's
//! search it is a body like a knocked prop's: the entry's other side is the truck's collision object,
//! and the bike's resolve pushes both bodies on the frame's first substep, the truck weighed by its
//! mass ([`crate::rider_contact::Bump::trucks`]). The `f.col` has no struck listener of the car's: a hit
//! on it wakes nothing ([`crate::truck_wheel`]).
//!
//! The truck is filed on the same world node as the bike and the props (free06's), after the bike, so
//! the bike's collide puts its mirrors (kind 1, the normal turned round, the bike on the other side) at
//! the head of the truck's list, which the truck's step resolves against the bike's chassis as its
//! frame left it ([`crate::Sim::trucks_late`]).
//!
//! In the strike's own tick the rays meet the path mesh first (its entry kept, its mover the car,
//! [`crate::mover_contact`]), the car wakes, and then this search meets the woken `f.col`: the same
//! search in the original, with one entry a ray, the nearer kept. Here the path mesh's entry for a
//! ray this search also struck gives way to the `f.col`'s ([`merged`]) when it is no nearer.

#![deny(clippy::float_arithmetic)]

use crate::bike_col::{self, BikeCol};
use crate::contact::{Contact, Node};
use crate::contact_search::{self, Hit, Record, Search, Struck, Swept};
use crate::flight_search;
use crate::math::Mat4;
use crate::physics_car::PhysicsCar;
use crate::prop_contact::Pushed;
use crate::prop_world;
use crate::ray_probe::{self, Facet};
use crate::rider_contact::{self, CAPACITY};
use crate::track::Mesh;
use crate::traffic::Traffic;
use crate::world::{Prop, World};
use crate::world_search::{self, Candidate, Categories, Class, Collidable, Narrow, Nearby, flags};
use ps2_float::{F, Vf, fpu};

/// What the bike's body rays struck of the trucks this tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Strike {
    /// The bike's entries (kind 0), each naming its truck's body by `first + k`, `k` its place in
    /// [`Strike::struck`].
    pub contacts: Vec<Contact>,
    /// Each struck truck, in the order the rays first met it: its car, and the mirrored entries (kind
    /// 1) that go on its list, naming the striker by its player.
    pub struck: Vec<(usize, Vec<Contact>)>,
}

/// One truck's `f.col`, where its body stands it.
struct Posed<'a> {
    car: usize,
    mesh: &'a Mesh,
    to_world: Mat4,
    centre: Vf,
    radius: F,
}

/// Where the trucks start in the search's list: after the bike's four collidables.
const TRUCKS: usize = 4;

/// The body rays' narrow phase: truck `k` of the posed is collidable `TRUCKS + k`.
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
        let Some(k) = other.checked_sub(TRUCKS) else { return false };
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

/// **A riding bike's strikes on the woken trucks**: its body rays (`col`) on the chassis at `at`,
/// against the `f.col` of every physical truck of `trucks` (the traffic's physics objects, by car)
/// where its body stands it. `me` is the striker's player, the mirrors' other side; `first` where the
/// struck bodies start among the bike's other sides. At most [`CAPACITY`] entries, one per ray.
pub fn strike(col: &BikeCol, at: &Mat4, trucks: &[Option<PhysicsCar>], me: usize, first: usize, dt: F) -> Strike {
    let posed: Vec<Posed<'_>> = trucks
        .iter()
        .enumerate()
        .filter_map(|(car, p)| {
            let p = p.as_ref().filter(|p| p.physical && p.object().filed)?;
            let mesh = (*p.shape).as_ref()?;
            let (to_world, centre, radius) = Prop::posed(mesh, &p.pose());
            Some(Posed { car, mesh, to_world, centre, radius })
        })
        .collect();
    if posed.is_empty() {
        return Strike::default();
    }
    let mut objects = bike_col::collidables(col, 0).to_vec();
    // The woken truck's `f.col`: category `0x18` with its nearby bit set by the wake (S208).
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
        .map(|(k, p)| Candidate { object: TRUCKS + k, distance: distance(centre, p.centre) - p.radius })
        .collect();
    world_search::search_nearby(&mut objects, bike_col::BODY_RAYS, &Nearby { found, prepare: false }, &mut strikes);
    // Each struck truck's collision object is a record of its own after the bike's, in the order the
    // rays first met it.
    let mut hit: Vec<usize> = Vec::new();
    let hits: Vec<Hit> = strikes
        .ray
        .contacts
        .iter()
        .map(|c| {
            let k = c.object as usize - TRUCKS;
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
        let capacity = trucks[posed[k].car].as_ref().and_then(|p| p.object().collision.as_ref()).map_or(0, |c| c.record.capacity);
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
        .map(|(&k, r)| (posed[k].car, r.contacts.into_iter().map(|e| Contact { other: Some(me), ..e }).collect()))
        .collect();
    Strike { contacts, struck }
}

/// `list` with the trucks' entries of `strike` added: an entry of `list` on a car's path mesh
/// (a mover's, nothing on its other side) for a ray that struck a truck's `f.col` gives way to it,
/// in its place, as the one-a-ray record keeps the later of two hits that is no farther; the rest
/// go on the end, which keeps at most [`CAPACITY`].
pub fn merged(mut list: Vec<Contact>, strike: Vec<Contact>) -> Vec<Contact> {
    let mut rest = Vec::new();
    for t in strike {
        match list.iter().position(|c| c.kind == 0 && c.other.is_none() && c.mover.is_some() && c.from == t.from) {
            Some(i) => list[i] = t,
            None => rest.push(t),
        }
    }
    crate::mover_contact::merged(list, rest)
}

impl Traffic {
    /// What [`strike`] did to the trucks: each struck one's mirrors put at the head of its list, which
    /// keeps at most its capacity, its flags set as the striker's search sets them. Nothing wakes: the
    /// `f.col` has no struck listener. Answers each struck body as the bike's resolve pushes it, in the
    /// strike's order ([`Pushed`], its collection the car and its object 0).
    pub fn truck_strike(&mut self, strike: &Strike) -> Vec<Pushed> {
        let mut pushed = Vec::with_capacity(strike.struck.len());
        for (k, mirrors) in &strike.struck {
            let Some(Some(p)) = self.physics.get_mut(*k) else { continue };
            let set = &mut p.set;
            let bone = &set.bones[0];
            let Some(col) = set.objects[0].collision.as_mut() else { continue };
            let r = &mut col.record;
            let own = std::mem::take(&mut r.contacts);
            r.contacts = mirrors.iter().copied().chain(own).take(r.capacity).collect();
            r.touching = true;
            r.on_object = true;
            r.touching_other = true;
            pushed.push(Pushed { collection: *k, object: 0, collider: col.collider, body: bone.body, mass: bone.mass });
        }
        pushed
    }

    /// The struck trucks' bodies as the bike's frame left them, written back: the body and the centre
    /// its collision object's resolve wrote.
    pub fn trucks_pushed(&mut self, pushed: &[Pushed]) {
        for t in pushed {
            let Some(Some(p)) = self.physics.get_mut(t.collection) else { continue };
            p.set.bones[0].body = t.body;
            if let Some(col) = p.set.objects[0].collision.as_mut() {
                col.collider = t.collider;
            }
        }
    }
}

impl crate::Sim {
    /// Player `i`'s strikes on the woken trucks this tick ([`strike`]), its mirrors naming striker
    /// `me`: none for a rider who is down (the wreck bike's object is not the riding one), a ghost, a
    /// bike with no collision files, or no truck awake. Its entries name the bodies from 0 until
    /// [`crate::Sim::truck_strike_into`] puts them after the bump's.
    pub fn truck_strike(&self, i: usize, me: usize, dt: F) -> Strike {
        let r = self.rider_of(i);
        let Some(col) = r.bike_col.as_deref() else { return Strike::default() };
        if r.state.wreck.is_some() || r.ghost.on || !crate::truck_wheel::Trucks::any(&self.traffic) {
            return Strike::default();
        }
        strike(col, &r.ride.transform(), &self.traffic.physics, me, 0, dt)
    }

    /// `strike` put on the trucks ([`Traffic::truck_strike`]) and in `bump`: its entries in the bump's
    /// list ([`merged`]), naming the struck bodies after every vehicle, plane and prop of the bump, and
    /// its struck bodies the bump's to push.
    pub fn truck_strike_into(&mut self, bump: &mut rider_contact::Bump, mut strike: Strike) {
        let first = bump.vehicles.len() + bump.planes.len() + bump.props.len();
        for e in &mut strike.contacts {
            e.other = e.other.map(|o| first + o);
        }
        bump.trucks = self.traffic.truck_strike(&strike);
        bump.contacts = merged(std::mem::take(&mut bump.contacts), strike.contacts);
    }
}

/// **The woken trucks' path meshes, stood down for the bikes' frames** (S251): what the wake does to the
/// car's own mesh (`+0x428`'s `+0x31` bits 0 and 1 cleared) until the put-back stands it up, so a bike's
/// body rays, sweep, nearby gather and wheels meet the truck's `f.col` alone. The meshes are taken out
/// of the world's props ([`traffic::hide`]) for the riders' part of the tick and put back before the
/// cars' late update stands them again ([`StoodDown::up`]).
///
/// [`traffic::hide`]: crate::traffic::hide
#[derive(Default)]
pub struct StoodDown {
    /// The cars stood down, and what of their mesh was taken, in the order they were.
    hidden: Vec<(usize, crate::traffic::Hidden)>,
}

impl StoodDown {
    /// Stand down every physical car's path mesh in `world` not stood down already: once before the
    /// bikes' searches, and again after the cars' strikes for the ones they woke.
    pub fn down(&mut self, traffic: &Traffic, world: &mut dyn World) {
        let Some([first, end]) = traffic.span() else { return };
        for prop in first..end {
            let Some(k) = traffic.car_at(prop) else { continue };
            let physical = traffic.physics.get(k).and_then(Option::as_ref).is_some_and(|p| p.physical);
            if physical && !self.hidden.iter().any(|(c, _)| *c == k) {
                self.hidden.push((k, crate::traffic::hide(world, [prop, prop + 1])));
            }
        }
    }

    /// Every mesh stood down put back, as it was.
    pub fn up(self, world: &mut dyn World) {
        for (_, h) in self.hidden.into_iter().rev() {
            crate::traffic::show(world, h);
        }
    }
}
