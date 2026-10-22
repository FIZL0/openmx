//! The biplane's world search: what its collision object's two probes strike among the track's
//! collision meshes (S48).
//!
//! The plane's collision object ([`crate::contact_search::Record`]) has a ray probe of 36 segments out
//! of `Biplane\col\biplanem.col` and a sweep probe of one segment, and a nearby search the broad
//! phase fills with the collidables around it. [`crate::contact_search::collide`] asks the world two
//! things through [`Search`]: the ray probe's hits where the plane stands, and the sweep probe's hit
//! along the segment the plane flew since a moment ago. [`Scene`] answers both over the track's props
//! ([`crate::world::Props`]) the way the original's search does:
//!
//! 1. **the candidates** — every prop as a collidable, with its distance from the ray probe's centre
//!    less its reach, nearest first ([`Scene::new`]);
//! 2. **the search** — S44's [`world_search::search_nearby`] over that list, once with each probe:
//!    its clear, the reach test against the probe's radius, the pair filter, then the probe's own
//!    narrow phase — S46's [`ray_probe::pair`] or S47's [`sweep_probe::pair`] — and the listeners;
//! 3. **the answers** — the ray probe's contacts, in its order, become [`Hit`]s; the sweep probe's
//!    nearest normal and point become the [`Swept`] push-out.
//!
//! A prop is a mesh in the ray probe's form: the triangles' face normals packed into shorts as the
//! collision loader packs them ([`pack_normal`]), the mesh placed where the scene puts it.
//!
//! The ray probe has two listeners. **The hit listener** asks the plane's body to wake when a struck
//! mesh's category is in the game's global mask ([`WAKE_MASK`], which every scenery mesh's category
//! has), and the waking does nothing to a body that is awake — and the plane's always is. **The
//! sound listener** is presentation only. Neither changes a frame, so [`Scene`] hears them and does
//! nothing.

#![deny(clippy::float_arithmetic)]

use crate::collide::MeshRef;
use crate::contact::Node as Frame;
use crate::contact_search::{Hit, Search, Struck, Swept};
use crate::ray_probe::{self, Facet, Matrix, RayProbe};
use crate::sweep_probe::{self, SweepProbe};
use crate::track::{self, Node};
use crate::world::{Prop, Props};
use crate::world_search::{self, Candidate, Categories, Class, Collidable, Narrow, Nearby, flags};
use ps2_float::{Dest, F, Vf, vu};

/// The plane's ray collision file, in the machines' archive.
pub const BIPLANE_RAYS: &str = "Biplane\\col\\biplanem.col";

/// A scenery mesh's category (`+0x84`): what every collision mesh a track places is in, read off
/// biplane.p2m2's save state (the 23 meshes around the plane).
pub const SCENERY: u32 = 0x48;

/// The categories of the probes' own collidables, and what they collide with (`+0x84`, `+0xcc`).
pub const PROBE: u32 = 0x8;
pub const PROBE_COLLIDES_WITH: u32 = 0xffff_ffff;

/// The global mask the hit listener tests a struck category against (0x40 in every state).
pub const WAKE_MASK: u32 = 0x40;

/// The plane's mask of categories its crash test spares from the speed rule: the "VEHICLE" surface
/// flag. A scenery mesh is not one.
pub const VEHICLE: u32 = 0x2000;

/// The ray probe's flag byte (`+0x116`) and the sweep probe's, as the plane's are built: neither is
/// struck by anything; the sweep never searches placed groups.
const RAY_FLAGS: u8 = 0x12;
const SWEEP_FLAGS: u8 = 0x13;

/// The ray probe as the plane's loader builds it: the segments and their tree, model space.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rays {
    pub root: u16,
    pub nodes: Vec<Node>,
    pub segments: Vec<ray_probe::Segment>,
}

impl Rays {
    /// Off the ray collision file (`biplanem.col`): its segments with `w` 1, its tree from node 0.
    pub fn of(file: &track::Rays) -> Rays {
        let point = |v: track::V3| Vf::new(v[0].0, v[1].0, v[2].0, 1.0);
        Rays {
            root: 0,
            nodes: file.nodes.clone(),
            segments: file.segments.iter().map(|s| ray_probe::Segment { from: point(s.from), to: point(s.to) }).collect(),
        }
    }

    /// The segments as the collision object lists them, `[start, end]`.
    pub fn pairs(&self) -> Vec<[Vf; 2]> {
        self.segments.iter().map(|s| [s.from, s.to]).collect()
    }
}

/// A face normal as the collision mesh loader stores it: each lane plus 768.0 on the vector unit,
/// and the low half of the sum's bits. 768 puts the binary point fourteen bits up the mantissa, so
/// this is the lane times 16384, cut toward zero (an exact 0 a hair below comes out as −1).
pub fn pack_normal(n: track::V3) -> [i16; 3] {
    n.map(|c| (vu::add(c.0, 768.0).to_bits() & 0xffff) as u16 as i16)
}

/// A mesh's triangles in the ray probe's form.
pub fn facets(mesh: &track::Mesh) -> Vec<Facet> {
    mesh.triangles.iter().map(|t| Facet { vertices: t.vertices, normal: pack_normal(t.normal) }).collect()
}

/// A collision object's transform as a frame.
fn matrix(node: &Frame) -> Matrix {
    [node.rows[0], node.rows[1], node.rows[2], node.origin]
}

/// The ray probe's reach where `t` stands it: its tree's root box through the frame, and the box's
/// half diagonal (the probe collidable's `+0x40` and `+0x60`).
pub fn reach(rays: &Rays, t: &Matrix) -> (Vf, F) {
    let Some(root) = rays.nodes.get(usize::from(rays.root)) else {
        return (t[3], F::ZERO);
    };
    let c = Vf::new(root.centre[0].0, root.centre[1].0, root.centre[2].0, 1.0);
    let h = Vf::new(root.half_extent[0].0, root.half_extent[1].0, root.half_extent[2].0, 0.0);
    // The translation first, then the three rows (`vaddw` off zero, `vmadd` x, y, z).
    let acc = Vf::madd(t[3].add(Vf::ZERO), t[0], Vf([c.x(); 4]));
    let acc = Vf::madd(acc, t[1], Vf([c.y(); 4]));
    let centre = Vf::madd(acc, t[2], Vf([c.z(); 4]));
    (centre.merge(Dest::W, c), length(h))
}

fn length(v: Vf) -> F {
    F(vu::dot3(v.0, v.0)).sqrt()
}

/// Every category allowed: a category list of mode 1 (the ones it never collides with) and empty.
fn any_category() -> Categories {
    Categories { mode: 1, list: Vec::new() }
}

/// The two probes' slots in [`Scene`]'s collidables; the props follow, prop `i` at `FIRST + i`.
const RAY: usize = 0;
const SWEEP: usize = 1;
const FIRST: usize = 2;

/// The world the plane searches, for one frame: the track's props as collidables, the candidates
/// gathered around the ray probe, and the two probes' state.
pub struct Scene<'a> {
    props: &'a [Prop],
    rays: &'a Rays,
    transform: Matrix,
    objects: Vec<Collidable>,
    nearby: Nearby,
    facets: Vec<Option<Vec<Facet>>>,
    ray: ray_probe::Record,
    sweep: SweepProbe,
}

impl<'a> Scene<'a> {
    /// The props as collidables and the candidates around the ray probe placed at `node`.
    pub fn new(props: Option<&'a Props>, rays: &'a Rays, node: &Frame) -> Scene<'a> {
        let props = props.map_or(&[][..], |p| &p.props[..]);
        let transform = matrix(node);
        let (centre, radius) = reach(rays, &transform);
        let probe = |flags, radius| Collidable {
            class: Class::Object,
            flags,
            radius,
            category: PROBE,
            collides_with: PROBE_COLLIDES_WITH,
            categories: any_category(),
            ..Collidable::default()
        };
        let mut objects = vec![probe(RAY_FLAGS, radius), probe(SWEEP_FLAGS, F::ZERO)];
        let mut found = Vec::new();
        for (i, p) in props.iter().enumerate() {
            objects.push(Collidable {
                class: Class::Object,
                flags: flags::ENABLED,
                nearby: true,
                radius: p.radius,
                category: SCENERY,
                collides_with: PROBE_COLLIDES_WITH,
                categories: any_category(),
                ..Collidable::default()
            });
            let d = p.centre.sub(centre);
            let gap = length(Vf::new(d.x(), d.y(), d.z(), 0.0)) - p.radius;
            found.push(Candidate { object: FIRST + i, distance: F::ZERO.max(gap) });
        }
        found.sort_by(|a, b| a.distance.0.total_cmp(&b.distance.0));
        let ray = ray_probe::Record {
            listed: true,
            capacity: rays.segments.len(),
            one_per_ray: true,
            ..ray_probe::Record::default()
        };
        let sweep = SweepProbe::default();
        Scene { props, rays, transform, objects, nearby: Nearby { found, prepare: false }, facets: vec![None; props.len()], ray, sweep }
    }

    /// Prop `i`'s triangles in the ray probe's form, packed once a frame.
    fn facets_of(&mut self, i: usize) -> &[Facet] {
        let props = self.props;
        self.facets[i].get_or_insert_with(|| facets(&props[i].mesh))
    }
}

/// A prop's mesh where the scene stands it.
fn placed(p: &Prop) -> Matrix {
    p.to_world.0
}

impl Narrow for Scene<'_> {
    fn clear(&mut self, objects: &mut [Collidable], probe: usize) {
        if probe == RAY {
            ray_probe::clear(&mut objects[probe].flags, &mut self.ray);
        } else {
            sweep_probe::clear(&mut self.sweep);
            objects[probe].flags &= !flags::HIT;
            objects[probe].radius = self.sweep.world.radius;
        }
    }

    fn prepare(&mut self, _objects: &mut [Collidable], _probe: usize) {}

    fn pair(&mut self, _objects: &mut [Collidable], probe: usize, other: usize) -> bool {
        let i = other - FIRST;
        let p = &self.props[i];
        if probe == RAY {
            let rays = self.rays;
            let transform = self.transform;
            let facets = self.facets_of(i).to_vec();
            let p = &self.props[i];
            let mesh = ray_probe::Mesh {
                id: other as u32,
                transform: placed(p),
                root: 0,
                nodes: &p.mesh.nodes,
                facets: &facets,
                vertices: &p.mesh.vertices,
            };
            let probe = RayProbe { transform, root: rays.root, nodes: &rays.nodes, segments: &rays.segments };
            ray_probe::pair(&probe, &ray_probe::Shape::Mesh(mesh), &mut self.ray)
        } else {
            let target = sweep_probe::Target {
                nearby: true,
                centre: p.centre,
                radius: p.radius,
                shape: sweep_probe::Shape::Mesh(sweep_probe::Placed { transform: placed(p), mesh: MeshRef::of(&p.mesh), object: other }),
            };
            sweep_probe::pair(&mut self.sweep, Some(&target))
        }
    }

    fn parts(&mut self, _objects: &mut [Collidable], _group: usize) -> usize {
        0
    }

    fn part(&mut self, _objects: &mut [Collidable], _group: usize, _i: usize) -> Option<usize> {
        None
    }

    /// The ray probe's two listeners: the hit listener's wake is a no-op on the plane's awake body,
    /// and the sound listener is not simulation.
    fn heard_hit(&mut self, _objects: &mut [Collidable], _listener: usize, _probe: usize, _other: usize) {}

    fn heard_struck(&mut self, _objects: &mut [Collidable], _listener: usize, _struck: usize, _probe: usize) {}

    fn on_hit(&mut self, _objects: &mut [Collidable], _callback: usize, _probe: usize, _other: usize) {}

    fn on_struck(&mut self, _objects: &mut [Collidable], _callback: usize, _struck: usize, _probe: usize) {}
}

/// What a prop is to the contact search: itself, with no collision object and no mover.
fn struck(object: usize) -> Struck {
    Struck { object, other: None, mover: None }
}

impl Search for Scene<'_> {
    /// The ray probe's search. The plane's collision object always has a nearby search; without one
    /// the original asks the broad phase directly, which here is the same list.
    fn rays(&mut self, _nearby: bool) -> Vec<Hit> {
        let mut objects = std::mem::take(&mut self.objects);
        let nearby = std::mem::take(&mut self.nearby);
        world_search::search_nearby(&mut objects, RAY, &nearby, self);
        (self.objects, self.nearby) = (objects, nearby);
        self.ray
            .contacts
            .iter()
            .map(|c| Hit { from: c.from, to: c.to, point: c.point, normal: c.normal, struck: struck(c.object as usize) })
            .collect()
    }

    fn sweep(&mut self, from: Vf, to: Vf, _nearby: bool) -> Option<Swept> {
        self.sweep.segments = vec![sweep_probe::Segment { ends: [from, to], nodes: [None, None] }];
        let mut objects = std::mem::take(&mut self.objects);
        let nearby = std::mem::take(&mut self.nearby);
        let hit = world_search::search_nearby(&mut objects, SWEEP, &nearby, self);
        (self.objects, self.nearby) = (objects, nearby);
        let a = &self.sweep.answer;
        hit.then(|| Swept { push: a.record.nearest_normal, point: a.record.nearest_point, struck: struck(a.struck[0].unwrap_or(SWEEP)) })
    }

    fn moved(&mut self, node: &Frame) {
        self.transform = matrix(node);
    }

    fn velocity(&self, _struck: &Struck) -> Vf {
        Vf::ZERO
    }
}

/// The category of what [`Scene`] reports as struck: every prop is scenery.
pub fn category(_object: usize) -> u32 {
    SCENERY
}
