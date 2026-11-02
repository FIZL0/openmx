//! **Wheels stand on a woken `HasPhysics` truck** (S208): the riding bike's wheel probe (its two
//! segments, [`crate::ride::wheel_rays`]) against each physical truck's `f.col`
//! ([`PhysicsCar::shape`]) where the truck's body stands it ([`PhysicsCar::pose`]), in the same record
//! as the scene's and the knockable props' hits ([`crate::prop_wheel`]), with the truck's path mesh
//! out of the wheel's search while it is physical.
//!
//! What the original does (S168, S208, `tests/bike_col.rs`, run in the interpreter on s89-along's two
//! free06 trucks): the physics object's `f.col` (`car+0x698` → `+0x128`) is category `0x18` and its
//! nearby bit is clear while the truck sleeps, so the wheel's search (which asks the pair filter with
//! the nearby bit) refuses it. The wake (`physics_car_wake`) sets the bit and stands the car's own
//! path mesh (`+0x428`) down, and then the filter takes every riding bike's wheel probe against the
//! `f.col`. Its collision object (`+0x80`) names the physics object's body (`+0xf4`) and no chassis,
//! so a wheel on it is:
//!
//! 1. a contact of the wheel probe's record like any other, one a segment, the nearer kept: here the
//!    trucks are tried after the scene and the props, on the same record;
//! 2. the distributor's owner word (`+0x1f0`) set to the truck's collision object and no vehicle, so
//!    the contact-velocity step takes its owner branch on a dynamics body ([`crate::contact::other_at`]'s body
//!    side, [`crate::prop_world::Collection::under`], S169's proof). The port's owner word is
//!    [`handle`], the car's place with [`OWNER`] set, which no prop's word ([`crate::prop_wheel::handle`])
//!    reaches.
//!
//! The `f.col` has no struck listener of the car's (the init puts that on the path mesh and the ray
//! probe, S127) and its object's own listener is off, so a wheel on a woken truck wakes nothing.
//!
//! **Not 1:1.** The original's `f.col` is filed from the wake to the put-away, and the port takes it
//! in reach for the same span ([`PhysicsCar::physical`]); whether the bit drops while the body lies
//! off before the put-away was not read. The body rays and the sweep still meet the path mesh, not
//! the `f.col`, and do not push the truck (S251).
//!
//! The trucks are the [`crate::Sim`]'s ([`crate::traffic::Traffic::physics`]), lent to the riding bike
//! for its frame with the props ([`crate::prop_wheel::Lent`]) and taken back after it.

#![deny(clippy::float_arithmetic)]

use crate::bike_col;
use crate::flight_search::{self, Rays};
use crate::math::Mat4;
use crate::physics_car::PhysicsCar;
use crate::ray_probe::{self, Facet, RayProbe};
use crate::track::Mesh;
use crate::traffic::{self, Hidden, Traffic};
use crate::world::{Prop, World};
use crate::world_search::{self, Candidate, Categories, Class, Collidable, Narrow, Nearby, Objects, flags};
use ps2_float::{F, Vf, fpu};

/// Where a truck's contact names its `f.col` in the wheel probe's record (`ray_probe::Contact::object`):
/// past the scene's collidables and the props' ([`crate::prop_wheel::FIRST`]).
pub const FIRST: u32 = 0x0200_0000;

/// The bit a truck's owner word carries: a prop's is its small place in the set.
pub const OWNER: u32 = 0x4000_0000;

/// The wheel's owner word for car `k`'s physics object.
pub fn handle(k: usize) -> u32 {
    OWNER | k as u32
}

/// The car an owner word names, `None` for a word that is not a truck's.
pub fn named(owner: u32) -> Option<usize> {
    (owner & OWNER != 0).then_some((owner & !OWNER) as usize)
}

/// The trucks as a ride holds them for its frame: every car's physics object (`None` for a car
/// without), and each car's path mesh's place in the world's props (`None` for a car without one).
#[derive(Clone, Default)]
pub struct Trucks {
    pub cars: Vec<Option<PhysicsCar>>,
    pub meshes: Vec<Option<usize>>,
}

impl std::fmt::Debug for Trucks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Trucks({} physical)", self.physical().count())
    }
}

impl Trucks {
    /// The traffic's physics objects, taken for the frame, and where its cars' meshes lie.
    pub fn take(traffic: &mut Traffic) -> Trucks {
        let cars = std::mem::take(&mut traffic.physics);
        let mut meshes = vec![None; cars.len()];
        if let Some([first, end]) = traffic.span() {
            for prop in first..end {
                if let Some(slot) = traffic.car_at(prop).and_then(|k| meshes.get_mut(k)) {
                    *slot = Some(prop);
                }
            }
        }
        Trucks { cars, meshes }
    }

    /// Whether any car is physical now: nothing to lend otherwise.
    pub fn any(traffic: &Traffic) -> bool {
        traffic.physics.iter().flatten().any(|p| p.physical)
    }

    /// The physical cars, by their place.
    fn physical(&self) -> impl Iterator<Item = (usize, &PhysicsCar)> + '_ {
        self.cars.iter().enumerate().filter_map(|(k, p)| Some((k, p.as_ref().filter(|p| p.physical)?)))
    }

    /// Stand the physical cars' path meshes down in `world` for the wheel's search, as the wake does
    /// ([`traffic::hide`]); [`traffic::show`] each back after it.
    pub fn stand_down(&self, world: &mut dyn World) -> Vec<Hidden> {
        self.physical().filter_map(|(k, _)| self.meshes.get(k).copied().flatten()).map(|m| traffic::hide(world, [m, m + 1])).collect()
    }
}

/// One truck's `f.col`, where its body stands it.
struct Posed<'a> {
    car: usize,
    mesh: &'a Mesh,
    to_world: Mat4,
    centre: Vf,
    radius: F,
}

/// The wheel probe's narrow phase: collidable `1 + k` is truck `k` of the posed, the probe is 0.
struct Wheels<'a> {
    rays: &'a Rays,
    at: Mat4,
    posed: &'a [Posed<'a>],
    facets: Vec<Option<Vec<Facet>>>,
    ray: ray_probe::Record,
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

/// **The wheel probe `rays` on the chassis at `at` against the physical trucks of `trucks`**, on
/// `ray`, the wheel probe's record as the scene's and the props' searches left it: each truck's `f.col`
/// within the probe's reach of its sphere, in the cars' order, its contacts kept by the record's
/// one-a-segment rule. Answers which of the record's contacts are on a truck, with its owner word and
/// its mesh's material ([`crate::prop_wheel::Found`]'s form).
pub fn search(rays: &Rays, at: &Mat4, trucks: &Trucks, ray: &mut ray_probe::Record) -> Vec<(usize, u32, u8)> {
    let posed: Vec<Posed<'_>> = trucks
        .physical()
        .filter_map(|(car, p)| {
            let mesh = (*p.shape).as_ref()?;
            let (to_world, centre, radius) = Prop::posed(mesh, &p.pose());
            Some(Posed { car, mesh, to_world, centre, radius })
        })
        .collect();
    if posed.is_empty() {
        return Vec::new();
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
    // The woken truck's `f.col`: category `0x18` with its nearby bit set by the wake (S208).
    objects.extend(posed.iter().map(|p| Collidable {
        class: Class::Object,
        flags: flags::ENABLED,
        nearby: true,
        radius: p.radius,
        category: crate::prop_world::COLLIDABLE,
        collides_with: crate::prop_world::COLLIDES_WITH,
        categories: Categories { mode: 1, list: Vec::new() },
        ..Collidable::default()
    }));
    let found = posed
        .iter()
        .enumerate()
        .map(|(k, p)| Candidate { object: 1 + k, distance: distance(centre, p.centre) - p.radius })
        .collect();
    let mut wheels = Wheels { rays, at: *at, posed: &posed, facets: (0..posed.len()).map(|_| None).collect(), ray: std::mem::take(ray) };
    world_search::search_nearby(&mut objects, 0, &Nearby { found, prepare: false }, &mut wheels);
    *ray = std::mem::take(&mut wheels.ray);
    ray.contacts
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let p = posed.get(c.object.checked_sub(FIRST)? as usize)?;
            Some((i, handle(p.car), p.mesh.surface.map_or(0, |s| s as u8)))
        })
        .collect()
}

/// **What a wheel whose owner word is `owner` stands on** (S208): the truck's body's velocity and its
/// velocity at the world `point` ([`crate::prop_world::Collection::under`], [`contact::other_at`]'s
/// body side). `None` for a word that is not a truck's, or a truck with no physics object.
///
/// [`contact::other_at`]: crate::contact::other_at
pub fn under(trucks: &mut Trucks, owner: u32, point: Vf) -> Option<(Vf, Vf)> {
    let k = named(owner)?;
    trucks.cars.get_mut(k)?.as_mut()?.set.under(0, point)
}
