//! **A map's prop world for SLUS-21104's bike** (F34): the world a Free Ride loads, built from the
//! map's files ([`build`]), and the bike's ray probes searched over it ([`Rays`], [`search`]).
//!
//! The world is [`crate::atv_world`]'s, as MD76a1 builds Desert Oasis's, filed in the original's
//! order: the scene's meshes (`Env\<map>\<map>.scn` and its `Col\*.col`), the plant groups of the
//! map's `.esb`, the `.toy`'s trigger zones, then each start zone's mesh a second time. One thing the
//! Free Ride does differently from what [`Scene::parse`] leaves: a shape its activity takes out (MD13)
//! is filed all the same, its "nearby" bit off, so the gather lists it and no nearby search strikes
//! it ([`Scene::parse_with_taken_out`]). On Nat05 those are the AirRace gates.
//!
//! What the ride files besides (the vehicles' spheres, the riders' spheres, the parked machines) is
//! the caller's, after [`build`].
//!
//! The bike's three ray probes (its two trigger rays, the collision record's wreck and riding parts)
//! are the quad's trigger probe ([`crate::atv_search`]) with their own rays and records: the trigger
//! probe keeps three contacts a ray, the two parts one. Each is stood at its frame, its reach set, and
//! searched over the bike's list with [`crate::atv_search::search`]. The pair is the quad's
//! ([`Wheels`]) on a scene mesh or a plant's part; a trigger zone is struck or not
//! ([`crate::atv_part`]'s sphere and box tests) with no contact; on anything else the ride filed (a
//! vehicle's or a rider's sphere) the caller's to say, here nothing struck.
//!
//! Proved against the original's stand, gather and search on state 4 (`tests/mxvatv_bike_props.rs`).

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::archive::Res;
use crate::atv_part::{self, OrientedBox, Sphere};
use crate::atv_search::{self, Wheels};
use crate::atv_world::{self, World, Zone};
use crate::camera::CurveTable;
use crate::eco::{Ecosystem, Trees};
use crate::phy::Ini;
use crate::ray_probe::{self, RayProbe, Record};
use crate::track::Scene;
use crate::world::{Prop, Props};
use crate::world_search::{Candidate, Collidable, Narrow};
use ps2_float::{F, Vf, fpu};

/// A map's prop world: the world, what its meshes and plants are, and the parsed files.
pub struct PropWorld {
    pub world: World,
    pub props: Vec<Prop>,
    pub eco: Ecosystem,
    pub trees: Trees,
    pub scene: Scene,
    /// The `.toy`'s trigger zones' collidables, and the zones in that order.
    pub zones: std::ops::Range<usize>,
    pub zone_shapes: Vec<Zone>,
}

/// A sphere zone as the probe's pair reads it: its centre, its radius and the radius squared (`mul.s`).
pub fn zone_sphere(z: &Zone) -> Option<Sphere> {
    let atv_world::Shape::Sphere { radius } = z.shape else { return None };
    let [x, y, z] = z.position;
    Some(Sphere { centre: Vf::new(x.0, y.0, z.0, 1.0), radius, radius2: F(fpu::mul(radius.0, radius.0)) })
}

/// A box zone as the probe's pair reads it: its frame ([`Zone::frame`]), its centre in that frame the
/// origin, its half sides.
pub fn zone_box(z: &Zone) -> Option<OrientedBox> {
    let atv_world::Shape::Box { half, .. } = z.shape else { return None };
    Some(OrientedBox { centre: Vf::new(0.0, 0.0, 0.0, 1.0), half: Vf::new(half[0].0, half[1].0, half[2].0, 1.0), frame: z.frame().0 })
}

/// The world over `map` (`Nat05`), its files out of the map's `.res` and its `.esb`, the gather
/// probe `probe` at [`atv_world::PROBE`].
pub fn build(res: &[u8], esb: &[u8], map: &str, probe: Collidable) -> Result<PropWorld, String> {
    let archive = Res::parse(res)?;
    let read = |name: &str| -> Result<Vec<u8>, String> {
        let e = archive.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name)).ok_or(format!("no {name}"))?;
        archive.read(res, e)
    };
    let (scene, taken_out) = Scene::parse_with_taken_out(&read(&format!("Env\\{map}\\{map}.scn"))?)?;
    let (props, missed) = Props::load(&scene, |n| read(&format!("Col\\{n}")));
    if !missed.is_empty() {
        return Err(format!("{map}'s meshes not read: {missed:?}"));
    }
    let eco = Ecosystem::parse(esb)?;
    let toy = Ini::parse_bytes(&read(&format!("Env\\{map}\\{map}.toy"))?);
    let (groups, models) = atv_world::groups(&eco);
    let field = atv_world::terrain_box(&scene.terrain);
    let cell = atv_world::plant_cell(field);
    let (trees, missed) = Trees::new(&eco, cell, &CurveTable::build());
    if !missed.is_empty() {
        return Err(format!("{map}'s plant solids not built: {missed:?}"));
    }
    let mut world = World::new(probe, &props.props, &groups, models, cell, field);
    for (i, &out) in taken_out.iter().enumerate() {
        world.objects[world.meshes.start + i].nearby = !out;
    }
    let zone_shapes = atv_world::zones(&toy);
    let zones = world.file_zones(&zone_shapes, field.1);
    for i in atv_world::start_zones(&scene) {
        world.file_again(world.meshes.start + i, &props.props[i], field.1);
    }
    Ok(PropWorld { world, props: props.props, eco, trees, scene, zones, zone_shapes })
}

/// A ray probe's record: a contact list `per_ray` contacts a ray long, one contact per ray.
pub fn record(rays: usize, per_ray: usize) -> Record {
    Record { listed: true, capacity: per_ray * rays, one_per_ray: true, nearest: F::ONE, ..Record::default() }
}

/// One of the bike's ray probes against the prop world: the quad's narrow phase ([`Wheels`]) on the
/// scene's meshes and the plants' parts (collidables `parts`), the zones (collidables `zones`, shapes
/// `zone_shapes`) struck or not, nothing struck on anything else.
pub struct Rays<'a> {
    pub wheels: Wheels<'a>,
    pub parts: std::ops::Range<usize>,
    pub zones: std::ops::Range<usize>,
    pub zone_shapes: &'a [Zone],
}

impl Narrow for Rays<'_> {
    fn clear(&mut self, objects: &mut [Collidable], probe: usize) {
        self.wheels.clear(objects, probe);
    }

    fn prepare(&mut self, objects: &mut [Collidable], probe: usize) {
        self.wheels.prepare(objects, probe);
    }

    fn pair(&mut self, objects: &mut [Collidable], probe: usize, other: usize) -> bool {
        let w = &self.wheels;
        let mesh = (w.first_mesh..w.first_mesh + w.props.len()).contains(&other);
        if mesh || self.parts.contains(&other) {
            return self.wheels.pair(objects, probe, other);
        }
        let p = RayProbe { transform: w.transform, root: w.rays.root, nodes: &w.rays.nodes, segments: &w.rays.segments };
        if self.zones.contains(&other) {
            let z = &self.zone_shapes[other - self.zones.start];
            self.wheels.record.first = F::ONE;
            return match (zone_sphere(z), zone_box(z)) {
                (Some(s), _) => atv_part::sphere_reaches(&p, &s),
                (_, Some(b)) => atv_part::box_reaches(&p, &b),
                _ => false,
            };
        }
        ray_probe::pair(&p, &ray_probe::Shape::Never, &mut self.wheels.record)
    }

    fn parts(&mut self, objects: &mut [Collidable], group: usize) -> usize {
        self.wheels.parts(objects, group)
    }

    fn part(&mut self, objects: &mut [Collidable], group: usize, i: usize) -> Option<usize> {
        self.wheels.part(objects, group, i)
    }

    fn heard_hit(&mut self, _objects: &mut [Collidable], _listener: usize, _probe: usize, _other: usize) {}

    fn heard_struck(&mut self, _objects: &mut [Collidable], _listener: usize, _struck: usize, _probe: usize) {}

    fn on_hit(&mut self, _objects: &mut [Collidable], _callback: usize, _probe: usize, _other: usize) {}

    fn on_struck(&mut self, _objects: &mut [Collidable], _callback: usize, _struck: usize, _probe: usize) {}
}

/// The probe stood at `rays.wheels.transform` (its reach into `objects[probe]`), then searched over
/// `found`, passing over the collidables `retired` names (first word 1). Returns the hit bit; the
/// contacts are in `rays.wheels.record`.
pub fn search(objects: &mut [Collidable], probe: usize, found: &[Candidate], retired: &dyn Fn(usize) -> bool, rays: &mut Rays) -> bool {
    let (_, radius) = atv_search::reach(rays.wheels.rays, &rays.wheels.transform);
    objects[probe].radius = radius;
    atv_search::search(objects, probe, found, retired, rays)
}
