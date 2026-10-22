//! The riding bike's collidables: what its body strikes other riders with, and what they strike.
//!
//! Each bike loads two collision files out of `Bike\Col\` when it is built, both named from the
//! bike's model name (`bike` for every rider, the split-screen second player's too):
//!
//! * **`bikem.col`** — eleven segments from the seat out, in a tree of their own: a ray probe
//!   ([`ray_probe::RayProbe`]), the bike's body rays;
//! * **`bikef.col`** — a triangle mesh around the frame: an ordinary collision mesh
//!   ([`ray_probe::Mesh`]), which is what another rider's body rays strike.
//!
//! So one bike's rays against another's frame is the ray probe against a mesh, which
//! [`ray_probe::pair`] already is: [`BikeCol::strike`] puts the two in place and calls it.
//!
//! The builder makes four collidables (the frame mesh, the rays, a second ray probe with no file, and
//! a sweep), and [`collidables`] is them as the ride leaves them: the classes and categories they are
//! given, the masks the ride's start ORs into three of them, and the lists of who ignores whom. A
//! riding bike's rays and sweep have empty category lists; a ghost's put-back adds the vehicle
//! category to them and takes it out again by zeroing its slot ([`ghost`]), so a bike that has been a
//! ghost keeps a list holding 0, which ignores nothing either.
//!
//! What is not here: the second ray probe's and the sweep's segments and reaches, which the ride sets
//! as it goes (they have no file), and the collidables' listeners and callbacks. The journal of S86
//! has the addresses.

#![deny(clippy::float_arithmetic)]

use crate::flight_search::{self, Rays};
use crate::ray_probe::{self, Facet, Matrix, Record};
use crate::track;
use crate::world_search::{Categories, Class, Collidable, Objects, flags};
use ps2_float::{F, Vf};

/// The directory the bike's collision files load from (`"%sCol"` of `Bike\`).
pub const DIR: &str = "Bike\\Col\\";
/// The model name every riding bike formats its file names with.
pub const MODEL: &str = "bike";

/// The body rays' file for a model (`"%sm.col"`).
pub fn rays_file(model: &str) -> String {
    format!("{DIR}{model}m.col")
}

/// The frame mesh's file for a model (`"%sf.col"`).
pub fn shape_file(model: &str) -> String {
    format!("{DIR}{model}f.col")
}

/// The category every rider's collidables are in (the world's rider word).
pub const RIDER: u32 = 0x0000_0008;
/// The vehicle category: what the frame mesh is in, and what a ghost's rays and sweep stop ignoring.
pub const VEHICLE: u32 = 0x0000_0040;
/// The two words the ride's start ORs into the frame mesh, both ray probes and the wreck part.
pub const RIDE_START: [u32; 2] = [0x0000_8000, 0x0001_0000];
/// The category the second ray probe never collides with, besides the bike's own actor word.
pub const SECOND_IGNORES: u32 = 0x0000_0100;
/// The bike's own words (its `+0x390` and `+0x394` as every bike is built): the actor, which the
/// frame mesh and the result record carry, and the frame mesh's own category.
pub const ACTOR: u32 = 0x0000_2000;
pub const SHAPE: u32 = 0x0008_0000;

/// The order [`collidables`] lists them in.
pub const FRAME: usize = 0;
pub const BODY_RAYS: usize = 1;
pub const SECOND_RAYS: usize = 2;
pub const SWEEP: usize = 3;

/// The result record's actors word (`+0x110`): the bike's actor and the rider category.
pub const RECORD_ACTORS: u32 = ACTOR | RIDER;

/// One bike's two collision files, as the loader builds them.
#[derive(Clone, Debug, PartialEq)]
pub struct BikeCol {
    /// The body rays, from node 0, model space.
    pub rays: Rays,
    /// The frame mesh, and its triangles in the probe's form.
    pub frame: track::Mesh,
    pub facets: Vec<Facet>,
    /// The frame mesh's reach (`+0x60`): its root box's half diagonal, through the FPU.
    pub frame_radius: F,
}

impl BikeCol {
    /// Off the two files' bytes.
    pub fn load(rays_col: &[u8], shape_col: &[u8]) -> Result<BikeCol, String> {
        let rays = Rays::of(&track::Rays::parse(rays_col)?);
        let frame = track::Mesh::parse(shape_col)?;
        let (_, frame_radius) =
            crate::ragdoll_ride::bike_sphere(shape_col).ok_or_else(|| "the frame mesh has no tree".to_string())?;
        let facets = flight_search::facets(&frame);
        Ok(BikeCol { rays, frame, facets, frame_radius })
    }

    /// The body rays' reach (`+0x60`): their root box's half diagonal (the frame does not change it).
    pub fn rays_radius(&self) -> F {
        flight_search::reach(&self.rays, &IDENTITY).1
    }

    /// The body rays standing at `transform`.
    pub fn probe(&self, transform: Matrix) -> ray_probe::RayProbe<'_> {
        ray_probe::RayProbe { transform, root: self.rays.root, nodes: &self.rays.nodes, segments: &self.rays.segments }
    }

    /// The frame mesh standing at `transform`, struck as `id`.
    pub fn mesh(&self, id: u32, transform: Matrix) -> ray_probe::Mesh<'_> {
        ray_probe::Mesh {
            id,
            transform,
            root: 0,
            nodes: &self.frame.nodes,
            facets: &self.facets,
            vertices: &self.frame.vertices,
        }
    }

    /// This bike's body rays at `at` against `other`'s frame mesh at `other_at` (struck as `id`): the
    /// probe's pair, with `rec` updated.
    pub fn strike(&self, at: Matrix, other: &BikeCol, id: u32, other_at: Matrix, rec: &mut Record) -> bool {
        ray_probe::pair(&self.probe(at), &ray_probe::Shape::Mesh(other.mesh(id, other_at)), rec)
    }
}

const IDENTITY: Matrix = [
    Vf::new(1.0, 0.0, 0.0, 0.0),
    Vf::new(0.0, 1.0, 0.0, 0.0),
    Vf::new(0.0, 0.0, 1.0, 0.0),
    Vf::new(0.0, 0.0, 0.0, 1.0),
];

/// The four collidables of one riding bike, in the order [`FRAME`], [`BODY_RAYS`], [`SECOND_RAYS`],
/// [`SWEEP`], as the ride leaves them; `first` is where the first stands in the caller's list (the
/// object lists name the others by it). The radii are the files' for the frame and the body rays and
/// 0 for the two the ride sizes as it goes.
pub fn collidables(col: &BikeCol, first: usize) -> [Collidable; 4] {
    let start = RIDE_START[0] | RIDE_START[1];
    let at = |i: usize| first + i;
    let never = |list: Vec<usize>| Objects { mode: 1, list };
    let object = |flags: u8, radius: F, category: u32, objects: Objects, categories: Vec<u32>| Collidable {
        class: Class::Object,
        flags,
        nearby: true,
        radius,
        category,
        collides_with: u32::MAX,
        objects,
        more_objects: never(Vec::new()),
        categories: Categories { mode: 1, list: categories },
        ..Collidable::default()
    };
    [
        object(flags::ENABLED, col.frame_radius, SHAPE | ACTOR | RIDER | VEHICLE | start, never(Vec::new()), Vec::new()),
        object(RAY_FLAGS, col.rays_radius(), RIDER | start, never(vec![at(FRAME), at(SECOND_RAYS), at(SWEEP)]), Vec::new()),
        object(RAY_FLAGS, F::ZERO, start, never(vec![at(BODY_RAYS), at(FRAME), at(SWEEP)]), vec![SECOND_IGNORES, ACTOR]),
        object(RAY_FLAGS | flags::NO_GROUPS, F::ZERO, RIDER, never(vec![at(BODY_RAYS), at(FRAME), at(SECOND_RAYS)]), Vec::new()),
    ]
}

/// A ray probe's flag byte as the builder leaves it (bit 1, never struck).
const RAY_FLAGS: u8 = 0x02;

/// The ghost put-back's change to a category list: `ignore` puts `category` in the first 0 slot (or
/// at the end) unless it is listed already, otherwise its first slot is zeroed and the list keeps its
/// length.
pub fn ghost(categories: &mut Categories, category: u32, ignore: bool) {
    if ignore {
        if categories.list.contains(&category) {
            return;
        }
        match categories.list.iter_mut().find(|c| **c == 0) {
            Some(slot) => *slot = category,
            None => categories.list.push(category),
        }
    } else if let Some(slot) = categories.list.iter_mut().find(|c| **c == category) {
        *slot = 0;
    }
}
