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
//! A plant's sphere (F69): each species' parts are one collidable apiece, shared by all its plants.
//! A sphere solid of the `.esb` ([`eco::SPHERE`]) is the sphere class with its point the solid's `at`
//! (`w` 1), the identity frame and the solid's radius ([`plant_spheres`]). Each time the search asks
//! a plant for it, it is stood at the plant ([`atv_plant::stand`]) and the probe's pair reads its
//! centre, radius and radius squared: struck or not, no contact, as a sphere zone.
//!
//! The bike's record sweep (F70) searches the same world with [`atv_sweep::Sweep`] over [`Sweeps`]:
//! a scene mesh swept in its own space, everything else tested and struck by nothing.
//!
//! Proved against the original's stand, gather and search on state 4 (`tests/mxvatv_bike_props.rs`).

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::archive::Res;
use crate::atv_part::{self, OrientedBox, Sphere};
use crate::atv_plant::{self, Part, Stood};
use crate::atv_sphere;
use crate::atv_sweep;
use crate::collide::MeshRef;
use crate::atv_search::{self, Wheels};
use crate::atv_world::{self, World, Zone};
use crate::camera::CurveTable;
use crate::eco::{self, Ecosystem, Solid, Trees};
use crate::phy::Ini;
use crate::ray_probe::{self, RayProbe, Record};
use crate::sweep_probe::{Placed, Shape as SweepShape, Target};
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
    /// The plants' sphere parts.
    pub spheres: PlantSpheres,
}

/// The plants' parts as the stand reads them: the eco's unit (`+0xc4`, the plants' cell) and, by
/// species in loaded order, its four scale words and its parts, a sphere where the solid is one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlantSpheres {
    pub unit: F,
    pub species: Vec<(atv_plant::Species, Vec<Option<atv_sphere::Sphere>>)>,
}

/// A sphere solid as its part is built: the point the solid's `at` (`w` 1), the identity frame, the
/// radius and its square ([`atv_sphere::place`]). Any other kind is not a sphere.
pub fn solid_sphere(s: &Solid) -> Option<atv_sphere::Sphere> {
    if s.kind != eco::SPHERE {
        return None;
    }
    let mut sphere = atv_sphere::Sphere { frame: atv_sphere::IDENTITY, ..atv_sphere::Sphere::default() };
    atv_sphere::place(&mut sphere, Some(Vf::new(s.at[0].0, s.at[1].0, s.at[2].0, 1.0)), Some(s.radius));
    Some(sphere)
}

/// `eco`'s species' sphere parts and scale words (lean: the usual and least height; size: the usual
/// and widest width), the plants' cell `unit`.
pub fn plant_spheres(eco: &Ecosystem, unit: F) -> PlantSpheres {
    let species = eco
        .species
        .iter()
        .flatten()
        .map(|s| {
            let scales = atv_plant::Species { lean: [s.height[0], s.height[1]], size: [s.width[0], s.width[2]] };
            (scales, s.solids.iter().map(solid_sphere).collect())
        })
        .collect();
    PlantSpheres { unit, species }
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
    let spheres = plant_spheres(&eco, cell);
    Ok(PropWorld { world, props: props.props, eco, trees, scene, zones, zone_shapes, spheres })
}

/// A ray probe's record: a contact list `per_ray` contacts a ray long, one contact per ray.
pub fn record(rays: usize, per_ray: usize) -> Record {
    Record { listed: true, capacity: per_ray * rays, one_per_ray: true, nearest: F::ONE, ..Record::default() }
}

/// One of the bike's ray probes against the prop world: the quad's narrow phase ([`Wheels`]) on the
/// scene's meshes and the plants' parts (collidables `parts`), the zones (collidables `zones`, shapes
/// `zone_shapes`) struck or not, a plant's sphere (stood at the plant, `spheres`) struck or not,
/// nothing struck on anything else.
pub struct Rays<'a> {
    pub wheels: Wheels<'a>,
    pub parts: std::ops::Range<usize>,
    pub zones: std::ops::Range<usize>,
    pub zone_shapes: &'a [Zone],
    /// The species' sphere parts, as last stood.
    pub spheres: PlantSpheres,
    /// The part collidable last handed out, when it is a sphere: as the pair reads it.
    pub stood: Option<(usize, Sphere)>,
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
        let p = RayProbe { transform: w.transform, root: w.rays.root, nodes: &w.rays.nodes, segments: &w.rays.segments };
        if let Some((_, s)) = self.stood.filter(|&(o, _)| o == other && self.parts.contains(&other)) {
            self.wheels.record.first = F::ONE;
            return atv_part::sphere_reaches(&p, &s);
        }
        if mesh || self.parts.contains(&other) {
            return self.wheels.pair(objects, probe, other);
        }
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

    /// The part, stood at the plant: a sphere wholly ([`atv_plant::stand`]); any other the quad's.
    fn part(&mut self, objects: &mut [Collidable], group: usize, i: usize) -> Option<usize> {
        self.stood = None;
        let slot = self.wheels.part(objects, group, i)?;
        let plants = &self.wheels.plants;
        let g = &plants.trees.groups()[group - plants.first_group];
        let model = usize::from(plants.models[group - plants.first_group]);
        let Some((species, parts)) = self.spheres.species.get_mut(model) else { return Some(slot) };
        let Some(Some(sphere)) = parts.get_mut(i) else { return Some(slot) };
        // No plant the port loads is hidden or turned (the eco's turns are none on Nat05).
        let plant = atv_plant::Plant { position: g.cell, lean: g.height, size: g.blend, flags: 0, turn: Vf::ZERO };
        let mut scales = [F::ZERO; 2];
        match atv_plant::stand(&plant, species, self.spheres.unit, Some(Part::Sphere { sphere, scales: &mut scales }), None) {
            Stood::Nothing => None,
            _ => {
                self.stood = Some((slot, Sphere { centre: sphere.centre, radius: sphere.radius, radius2: sphere.radius2 }));
                Some(slot)
            }
        }
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

/// **The record's sweep over the prop world** (F70): what each collidable is to the sweep's pair
/// ([`atv_sweep::pair`]) and a placed group's parts, for [`atv_sweep::Sweep`] over the bike's record
/// list. A scene mesh is swept in its own space ([`Shape::Mesh`], its frame the prop's); anything
/// else the ride or the world filed (a zone, a vehicle's or a rider's sphere) has its centre and
/// reach tested and strikes nothing ([`Shape::Other`]), as the pair does every kind but a mesh, a
/// hierarchy and a finite plane. A plant's sphere part is stood at the plant first ([`atv_plant::stand`])
/// and tested at the centre and reach that leaves.
///
/// The nearby bits, centres and reaches are the world's as [`Sweeps::new`] finds them: the sweep
/// changes none but the probe's. Nobody listens and no callback runs (on Nat05 no mesh, plant or
/// zone the sweep can strike has either: a zone or a sphere is never struck).
pub struct Sweeps<'a> {
    pub props: &'a [Prop],
    pub first_mesh: usize,
    pub trees: &'a Trees,
    /// Each placed group's species (loaded order), the first group at collidable `first_group`.
    pub models: Vec<u8>,
    pub first_group: usize,
    /// Each species' first part collidable, as [`atv_search::Plants`] has them.
    pub slots: Vec<usize>,
    pub spheres: PlantSpheres,
    /// Each collidable's nearby bit, centre and reach.
    pub places: Vec<(bool, Vf, F)>,
    /// How many pairs ran.
    pub pairs: u32,
    /// The part last handed out, when it is a stood sphere: its collidable, centre and reach.
    stood: Option<(usize, Vf, F)>,
}

impl<'a> Sweeps<'a> {
    /// The sweep's world over `world` as it stands, its meshes `props`, its plants `trees` with the
    /// species' parts (`spheres`) as collidables from `first_part` on.
    pub fn new(world: &World, props: &'a [Prop], trees: &'a Trees, eco: &Ecosystem, spheres: PlantSpheres, first_part: usize) -> Sweeps<'a> {
        let mut slots = Vec::new();
        let mut at = first_part;
        for s in eco.species.iter().flatten() {
            slots.push(at);
            at += s.solids.len();
        }
        let places = (0..world.objects.len())
            .map(|o| {
                let c = world.broad.places.get(o).map_or([F::ZERO; 4], |p| p.centre);
                (world.objects[o].nearby, Vf::new(c[0].0, c[1].0, c[2].0, c[3].0), world.objects[o].radius)
            })
            .collect();
        let models = world.groups.clone().map(|g| world.broad.places[g].model).collect();
        Sweeps { props, first_mesh: world.meshes.start, trees, models, first_group: world.groups.start, slots, spheres, places, pairs: 0, stood: None }
    }
}

impl atv_sweep::Outside for Sweeps<'_> {
    fn target(&mut self, other: usize, pair: &mut dyn FnMut(Option<&Target>) -> bool) -> bool {
        self.pairs += 1;
        if let Some((_, centre, radius)) = self.stood.filter(|&(o, ..)| o == other) {
            return pair(Some(&Target { nearby: true, centre, radius, shape: SweepShape::Other }));
        }
        let (nearby, centre, radius) = self.places[other];
        let shape = match other.checked_sub(self.first_mesh).filter(|&i| i < self.props.len()) {
            Some(i) => SweepShape::Mesh(Placed { transform: self.props[i].to_world.0, mesh: MeshRef::of(&self.props[i].mesh), object: other }),
            None => SweepShape::Other,
        };
        pair(Some(&Target { nearby, centre, radius, shape }))
    }
}

impl Narrow for Sweeps<'_> {
    fn clear(&mut self, _objects: &mut [Collidable], _probe: usize) {
        unreachable!("the sweep's own clear")
    }

    /// Never asked: the record's list carries no terrain grid.
    fn prepare(&mut self, _objects: &mut [Collidable], _probe: usize) {}

    fn pair(&mut self, _objects: &mut [Collidable], _probe: usize, _other: usize) -> bool {
        unreachable!("the sweep's own pair")
    }

    fn parts(&mut self, _objects: &mut [Collidable], group: usize) -> usize {
        self.trees.groups()[group - self.first_group].parts.len()
    }

    /// The part, stood at the plant: a sphere as [`Rays`] stands it. A trunk's place as a collidable
    /// is not ported (Nat05's plants have none).
    fn part(&mut self, _objects: &mut [Collidable], group: usize, i: usize) -> Option<usize> {
        self.stood = None;
        let g = &self.trees.groups()[group - self.first_group];
        let model = usize::from(self.models[group - self.first_group]);
        let slot = self.slots[model] + i;
        assert!(g.parts[i].is_none(), "the sweep searched a trunk part (group {group}, part {i}): its place is not ported");
        let Some((species, parts)) = self.spheres.species.get_mut(model) else { return Some(slot) };
        let Some(Some(sphere)) = parts.get_mut(i) else { return Some(slot) };
        let plant = atv_plant::Plant { position: g.cell, lean: g.height, size: g.blend, flags: 0, turn: Vf::ZERO };
        let mut scales = [F::ZERO; 2];
        match atv_plant::stand(&plant, species, self.spheres.unit, Some(Part::Sphere { sphere, scales: &mut scales }), None) {
            Stood::Nothing => None,
            _ => {
                self.stood = Some((slot, sphere.centre, sphere.reach));
                Some(slot)
            }
        }
    }

    fn heard_hit(&mut self, _objects: &mut [Collidable], _listener: usize, _probe: usize, _other: usize) {}

    fn heard_struck(&mut self, _objects: &mut [Collidable], _listener: usize, _struck: usize, _probe: usize) {}

    fn on_hit(&mut self, _objects: &mut [Collidable], _callback: usize, _probe: usize, _other: usize) {}

    fn on_struck(&mut self, _objects: &mut [Collidable], _callback: usize, _struck: usize, _probe: usize) {}
}
