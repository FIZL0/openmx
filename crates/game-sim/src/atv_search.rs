//! **The quad's wheel-ray search** (MD76a2): what [`crate::chassis::ContactCall::Collide`] does in
//! [`crate::quad::wheel_contacts`] — SLUS-21104's nearby search, run with the quad's trigger probe over
//! the list the gather filled ([`crate::atv_world::gather`]), and the probe's record read out as the
//! distributor's contacts. Proved against the original's own search on MD37a's image
//! (`tests/atv_search.rs`).
//!
//! The probe is MX Unleashed's ray probe ([`crate::ray_probe`]) with its fields moved: four segments,
//! one per wheel ([`crate::quad::trigger_segments`]), in a tree of their own ([`crate::kiowa::ray_tree`]),
//! stood where the quad's scene node is. Before the search the probe is told where the node is: the
//! frame is kept, and the probe's centre and reach are its tree's root box through it ([`reach`]).
//!
//! The search ([`search`]) is MX Unleashed's ([`crate::world_search::search_nearby`]) with two
//! differences:
//!
//! - a collidable whose first word is 1 is passed over before it is marked searched (the
//!   constructor sets it to 0, and nothing in Desert Oasis's static world sets it; the caller says
//!   which those are);
//! - the probe is made ready only when the list carries the terrain grid *and* the caller asks for
//!   it. The quad asks, but its list never carries one, so the preparation is not here.
//!
//! The pair is MX Unleashed's too ([`ray_probe::pair`]): the record's fields after `+0xac` moved up a
//! word, and every pair first zeroes a per-contact word array at `+0xac` that nothing the quad reads
//! (or anything in the search) writes. A record holds three contacts a ray (12) and keeps one per ray.
//!
//! The plants' parts ([`Plants`]): the species with a sphere stand a sphere, whose ray test writes no
//! contact (so the port does not test it: only the hit bit could tell); the two with a cylinder
//! (MC02's models 7 and 8, prickly pear and weeds) stand MX Unleashed's trunk mesh
//! ([`eco::Trees`]). MC02's two have no height, so nothing strikes them.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::atv_world::World;
use crate::eco::{self, Ecosystem, Trees};
use crate::flight_search::{self, Rays};
use crate::ray_probe::{self, Facet, Matrix, RayProbe, Record};
use crate::world::Prop;
use crate::world_search::{Candidate, Class, Collidable, Narrow, accepts, flags, test};
use ps2_float::{F, Vf, fpu};

/// How many contacts the record keeps a ray (the probe's builder sizes it `3 × rays`).
pub const CONTACTS_PER_RAY: usize = 3;

/// The quad's probe's record, for `rays` rays: a contact list three a ray long, one contact per ray.
pub fn record(rays: usize) -> Record {
    Record { listed: true, capacity: CONTACTS_PER_RAY * rays, one_per_ray: true, nearest: F::ONE, ..Record::default() }
}

/// The probe stood at `t` (the scene node's frame): its centre, its tree's root box's centre through
/// the frame (`w` the root's 1), and its reach, the box's half diagonal as an FPU chain
/// (`mula`, `madda`, `madd`, `sqrt`).
pub fn reach(rays: &Rays, t: &Matrix) -> (Vf, F) {
    let (centre, _) = flight_search::reach(rays, t);
    let Some(root) = rays.nodes.get(usize::from(rays.root)) else {
        return (centre, F::ZERO);
    };
    let [x, y, z] = root.half_extent.map(|h| h.0);
    (centre, F(fpu::sqrt(fpu::madd(fpu::madda(fpu::mula(x, x), y, y), z, z))))
}

/// **The search**: `objects[probe]` against everything the gather `found`. Returns the probe's hit
/// bit. `retired` says which collidables' first word is 1.
pub fn search(
    objects: &mut [Collidable],
    probe: usize,
    found: &[Candidate],
    retired: &dyn Fn(usize) -> bool,
    narrow: &mut dyn Narrow,
) -> bool {
    objects[probe].flags |= flags::SEARCHED;
    narrow.clear(objects, probe);
    for c in found {
        if !(c.distance <= objects[probe].radius) {
            continue;
        }
        let o = c.object;
        match objects[o].class {
            Class::Object => {
                if retired(o) {
                    continue;
                }
                objects[o].flags |= flags::SEARCHED;
                if accepts(objects, probe, o, true) {
                    test(objects, probe, o, narrow);
                }
            }
            Class::Group if objects[probe].flags & flags::NO_GROUPS == 0 => {
                let mut i = 0;
                while i < narrow.parts(objects, o) {
                    if let Some(p) = narrow.part(objects, o, i) {
                        objects[p].group = Some(o);
                        objects[p].flags |= flags::SEARCHED;
                        test(objects, probe, p, narrow);
                    }
                    i += 1;
                }
            }
            _ => {}
        }
    }
    objects[probe].flags & flags::HIT != 0
}

/// A plant's parts as the probe tests them: each species' parts are collidables of their own, one
/// for all the plants of the species, stood where the plant is each time the search asks for it.
/// A sphere's ray test writes no contact; a cylinder is the trunk mesh [`Trees`] builds, at the
/// plant's cell with the part's own rows (MC02's are all the identity).
pub struct Plants<'a> {
    pub trees: &'a Trees,
    /// Each placed group's species (in loaded order), the first group at collidable `first_group`.
    pub models: Vec<u8>,
    pub first_group: usize,
    /// Each species' first part collidable ([`part_collidables`]).
    pub slots: Vec<usize>,
}

impl<'a> Plants<'a> {
    /// `world`'s plants, with `eco`'s parts as collidables from `first_part` on.
    pub fn new(trees: &'a Trees, eco: &Ecosystem, world: &World, first_part: usize) -> Plants<'a> {
        let mut slots = Vec::new();
        let mut at = first_part;
        for s in eco.species.iter().flatten() {
            slots.push(at);
            at += s.solids.len();
        }
        let models = world.groups.clone().map(|g| world.broad.places[g].model).collect();
        Plants { trees, models, first_group: world.groups.start, slots }
    }
}

/// One collidable per part of each species, in loaded order: what [`Plants`] hands the search.
pub fn part_collidables(eco: &Ecosystem) -> Vec<Collidable> {
    let n = eco.species.iter().flatten().map(|s| s.solids.len()).sum();
    vec![Collidable { class: Class::Object, flags: flags::ENABLED, ..Collidable::default() }; n]
}

/// The quad's trigger probe against Desert Oasis's world: the narrow phase, the plants' parts and
/// the record. Nobody listens to the probe, the meshes or the parts, and none has a callback.
pub struct Wheels<'a> {
    pub rays: &'a Rays,
    /// Where the scene node is (`+0x130`).
    pub transform: Matrix,
    /// The scene's meshes, the first at collidable `first_mesh`.
    pub props: &'a [Prop],
    pub first_mesh: usize,
    pub plants: Plants<'a>,
    pub record: Record,
    facets: Vec<Vec<Facet>>,
    trunks: Vec<Option<Vec<Facet>>>,
    /// The trunk the part last handed out is standing as.
    trunk: Option<usize>,
}

impl<'a> Wheels<'a> {
    pub fn new(rays: &'a Rays, transform: Matrix, props: &'a [Prop], first_mesh: usize, plants: Plants<'a>) -> Wheels<'a> {
        let facets = props.iter().map(|p| flight_search::facets(&p.mesh)).collect();
        let trunks = vec![None; plants.trees.props.props.len()];
        let record = record(rays.segments.len());
        Wheels { rays, transform, props, first_mesh, plants, record, facets, trunks, trunk: None }
    }

    fn group(&self, group: usize) -> &eco::Group {
        &self.plants.trees.groups()[group - self.plants.first_group]
    }
}

fn mesh<'m>(id: usize, p: &'m Prop, facets: &'m [Facet]) -> ray_probe::Mesh<'m> {
    ray_probe::Mesh { id: id as u32, transform: p.to_world.0, root: 0, nodes: &p.mesh.nodes, facets, vertices: &p.mesh.vertices }
}

impl Narrow for Wheels<'_> {
    fn clear(&mut self, objects: &mut [Collidable], probe: usize) {
        ray_probe::clear(&mut objects[probe].flags, &mut self.record);
    }

    /// Never asked: the quad's list carries no terrain grid.
    fn prepare(&mut self, _objects: &mut [Collidable], _probe: usize) {}

    fn pair(&mut self, _objects: &mut [Collidable], _probe: usize, other: usize) -> bool {
        let probe = RayProbe { transform: self.transform, root: self.rays.root, nodes: &self.rays.nodes, segments: &self.rays.segments };
        if other < self.first_mesh + self.props.len() {
            let i = other - self.first_mesh;
            let shape = ray_probe::Shape::Mesh(mesh(other, &self.props[i], &self.facets[i]));
            return ray_probe::pair(&probe, &shape, &mut self.record);
        }
        // A plant's part: a sphere strikes nothing a contact is made of, but the pair still starts
        // the record's nearest-of-this-pair afresh.
        let Some(j) = self.trunk else {
            return ray_probe::pair(&probe, &ray_probe::Shape::Never, &mut self.record);
        };
        let p = &self.plants.trees.props.props[j];
        let facets = self.trunks[j].get_or_insert_with(|| flight_search::facets(&p.mesh));
        ray_probe::pair(&probe, &ray_probe::Shape::Mesh(mesh(other, p, facets)), &mut self.record)
    }

    fn parts(&mut self, _objects: &mut [Collidable], group: usize) -> usize {
        self.group(group).parts.len()
    }

    fn part(&mut self, _objects: &mut [Collidable], group: usize, i: usize) -> Option<usize> {
        self.trunk = self.group(group).parts[i];
        let model = self.plants.models[group - self.plants.first_group];
        Some(self.plants.slots[usize::from(model)] + i)
    }

    fn heard_hit(&mut self, _objects: &mut [Collidable], _listener: usize, _probe: usize, _other: usize) {}

    fn heard_struck(&mut self, _objects: &mut [Collidable], _listener: usize, _struck: usize, _probe: usize) {}

    fn on_hit(&mut self, _objects: &mut [Collidable], _callback: usize, _probe: usize, _other: usize) {}

    fn on_struck(&mut self, _objects: &mut [Collidable], _callback: usize, _struck: usize, _probe: usize) {}
}

/// The record as the distributor reads it, contact by contact: where along and across the quad (the
/// ray's first end), the normal, the point, and the collidable struck. Desert Oasis's meshes belong to
/// no body and no vehicle.
pub fn contacts(rec: &Record) -> Vec<crate::quad::Contact> {
    rec.contacts
        .iter()
        .map(|c| crate::quad::Contact { at: c.from, normal: c.normal, point: c.point, node: c.object, ..Default::default() })
        .collect()
}

/// **`ContactCall::Collide`** for the quad: the probe stood at `transform` (its centre and reach into
/// `objects[probe]`), then searched over `found`. Returns the hit bit; the contacts are
/// [`contacts`] of `wheels.record`.
pub fn collide(objects: &mut [Collidable], probe: usize, found: &[Candidate], wheels: &mut Wheels) -> bool {
    let (_, radius) = reach(wheels.rays, &wheels.transform);
    objects[probe].radius = radius;
    search(objects, probe, found, &|_| false, wheels)
}

/// **Desert Oasis's sweep** (MD86): what [`crate::quad::Steps`] answers [`crate::quad::Call::Contacts`]
/// with. `world` holds the trigger probe (`probe`) and, from `first_part`, the plants' parts
/// ([`part_collidables`]) after its own; `rays` are the probe's.
///
/// Stood at the node's frame, the gather probe ([`crate::atv_world::PROBE`]) is put at its position
/// and the quad's list (`list`) gathered round it ([`crate::atv_world::gather`]), then [`collide`]
/// over the list into `record`. The original gathers in the PostProcess's collision sync, round the
/// node as that PostProcess left it, and nothing moves the node or the world between that and the
/// next frame's sweeps. MD75c2: that gather is answered here ([`crate::quad::Sweep::gather`], round the
/// active probe as its sync stood it); once it has been, the sweep searches the list it left, and
/// until then (no PostProcess has run) the sweep makes it.
pub struct Oasis<'a> {
    pub world: World,
    pub props: &'a [Prop],
    pub eco: &'a Ecosystem,
    pub trees: &'a Trees,
    pub rays: Rays,
    pub probe: usize,
    pub first_part: usize,
    pub list: crate::atv_world::Nearby,
    pub record: Record,
    /// Whether the collision sync has gathered the list.
    pub gathered: bool,
}

impl<'a> Oasis<'a> {
    pub fn new(world: World, props: &'a [Prop], eco: &'a Ecosystem, trees: &'a Trees, rays: Rays, probe: usize, first_part: usize) -> Oasis<'a> {
        let record = record(rays.segments.len());
        Oasis { world, props, eco, trees, rays, probe, first_part, list: crate::atv_world::Nearby::quad(), record, gathered: false }
    }
}

impl crate::quad::Sweep for Oasis<'_> {
    fn sweep(&mut self, frame: &Matrix) -> Option<(bool, Vec<crate::quad::Contact>)> {
        if !self.gathered {
            let t = frame[3];
            self.world.place_probe([t.x(), t.y(), t.z()].map(F));
            crate::atv_world::gather(&mut self.world.objects, &mut self.world.broad, &mut self.list, crate::atv_world::PROBE, 3);
        }
        let plants = Plants::new(self.trees, self.eco, &self.world, self.first_part);
        let mut wheels = Wheels::new(&self.rays, *frame, self.props, self.world.meshes.start, plants);
        wheels.record = std::mem::take(&mut self.record);
        let hit = collide(&mut self.world.objects, self.probe, &self.list.found, &mut wheels);
        self.record = wheels.record;
        Some((hit, contacts(&self.record)))
    }

    /// The collision sync's gather: the gather probe ([`crate::atv_world::PROBE`]) at the active
    /// probe's centre (every lane) with its reach, and the list refilled round it.
    fn gather(&mut self, centre: Vf, reach: F) -> bool {
        let w = &mut self.world;
        w.broad.places[crate::atv_world::PROBE].centre = centre.0.map(F);
        w.objects[crate::atv_world::PROBE].radius = reach;
        crate::atv_world::gather(&mut w.objects, &mut w.broad, &mut self.list, crate::atv_world::PROBE, 3);
        self.gathered = true;
        true
    }
}

/// **The world probe** (the wreck fade's search, F31b5l): `objects[probe]` against the world directly,
/// as [`crate::broad_phase::probe_collide_world`] with SLUS-21104's two differences: the probe must
/// also be in a space to collide at all (else only its hit bit is lowered), and in the static
/// structures a collidable `retired` names is passed over before it is marked searched. Returns how
/// far the probe is from the nearest thing it did not touch, also its new budget.
pub fn collide_world<N: crate::broad_phase::Probe>(
    objects: &mut [Collidable],
    broad: &mut crate::broad_phase::Broad,
    probe: usize,
    lazy: u32,
    retired: &dyn Fn(usize) -> bool,
    narrow: &mut N,
) -> F {
    if broad.places[probe].space.is_none() {
        objects[probe].flags &= !flags::HIT;
        return F::MAX;
    }
    crate::broad_phase::probe_collide_world_passing(objects, broad, probe, lazy, retired, narrow)
}
