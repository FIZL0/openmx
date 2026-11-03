//! **Desert Oasis's collidables** (MD76a1): MX vs. ATV Unleashed's static collision world for MC02 as
//! SLUS-21104 loads it, and the gather that fills the quad's nearby list from it. Proved against the
//! original's own gather on MD37a's image (`tests/atv_gather.rs`).
//!
//! The world is MX Unleashed's (S96, S102) with the same three steps ([`crate::broad_build`]):
//!
//! - **the tree** is laid over the terrain's box on the ground plane (MC02: `-0`..5760 both ways,
//!   the terrain's 5 × 256 samples of 4.5 ft), its least node 96 ft across;
//! - **the scene's meshes** go in first, in the scene's order: every `[ObjectN]` `COL=` of the
//!   `.scn`, stood where it says ([`Prop::place`]), boxed by its root node through its frame
//!   ([`flight_search::prop_box`]), category [`MESH`] and gathered only within the bound;
//! - **the ecosystem's plants** go in after them, in the `.esb`'s order, each a placed group boxed
//!   by its cell, its height and its reach ([`eco::Group::bounds`]).
//!
//! The gather ([`gather`]) is MX Unleashed's (`broad_phase::gather`) with the quad's list in its own
//! layout and three differences: it always gathers afresh (no resort, no "still within the bound"),
//! the pair filter is optional (without it a collidable is kept if it is enabled), and the distance
//! can be measured on the ground plane. A group's distance adds `0` to the probe's point first.
//!
//! What SLUS-21104's world holds that this one does not: the scene's zones (five start/trigger
//! shapes of category `0x8000` and one more), the vehicles' and riders' own collidables in the moving
//! list, and 239 tree entries of class 7. The image's every gather near the props leaves those out
//! (the test strips the zones and counts them; it finds none of class 7 or the moving list kept).

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::broad_build;
use crate::broad_phase::{Blocks, Broad, Model, Pair, Placement, Query, Space, Structure, Tables, more_flags};
use crate::eco::{self, Ecosystem, Group};
use crate::flight_search;
use crate::track::TerrainLayout;
use crate::world::Prop;
use crate::world_search::{Candidate, Categories, Class, Collidable, accepts, flags};
use ps2_float::{Acc, F, fpu};

/// A scene mesh's category (`+0x94`), and its flag byte (`+0x126`): enabled, and gathered only
/// within the bound.
pub const MESH: u32 = 0x600;
pub const MESH_FLAGS: u8 = flags::ENABLED | more_flags::WITHIN_BOUND;
/// The least side of the tree's smallest node, in feet (`0x0031a750`'s `96.0`).
pub const TREE_LEAST: f32 = 96.0;
/// Samples a terrain width unit holds.
const SAMPLES_PER_WIDTH: i32 = 256;
/// The probe's collidable, first in every world.
pub const PROBE: usize = 0;

/// The world, as the quad's gather reads it: the broad phase, and its collidables (the probe, then
/// the meshes, then the plants).
pub struct World {
    pub broad: Broad,
    pub objects: Vec<Collidable>,
    /// Where the scene's meshes are, and the plants' groups.
    pub meshes: std::ops::Range<usize>,
    pub groups: std::ops::Range<usize>,
}

/// The ground-plane box the tree is laid over: from minus the border to the terrain's samples times
/// their spacing, `x` then `z`. MC02 has no border, so its low corner is `-0`.
pub fn terrain_box(layout: &TerrainLayout) -> ([F; 2], [F; 2]) {
    let lo = F(fpu::neg(layout.border_width.0));
    let across = F::from(i32::try_from(layout.width).unwrap_or(0) * SAMPLES_PER_WIDTH) * layout.width_scale;
    let along = F::from(i32::try_from(layout.breadth).unwrap_or(0) * SAMPLES_PER_WIDTH) * layout.breadth_scale;
    ([lo, lo], [across, along])
}

/// The plants as placed groups (in the file's order), and each species' reach at blend 0 and 128.
/// SLUS-21104 numbers the species in the order it loads them, the file's empty slots left out (MC02's
/// slot 0 is empty, so its slot 1 is model 0), and a group's model is that number. A group's reach is
/// its species' usual width, `blend` 128ths of the way to its widest; its height is the file's, not
/// stood on the ground (nothing the gather reads depends on it).
pub fn groups(eco: &Ecosystem) -> (Vec<Group>, Vec<Model>) {
    let models = eco.species.iter().flatten().map(|s| Model { near: s.width[0], far: s.width[2] }).collect();
    let loaded = |slot: u8| eco.species[..usize::from(slot)].iter().flatten().count() as u8;
    let groups = eco
        .plants
        .iter()
        .map(|p| {
            let sp = eco.species[usize::from(p.species)].as_ref();
            let width = sp.map_or([F::ZERO; 3], |s| s.width);
            let height = sp.map_or([F::ZERO; 3], |s| s.height);
            let reach = width[0] + F::from(i32::from(p.width)) * (width[2] - width[0]) / 128.0;
            let half_height = (height[0] + F::from(i32::from(p.height)) * (height[0] - height[1]) / 128.0) * 0.5;
            let solid = sp.is_some_and(|s| !s.solids.is_empty());
            Group { cell: p.cell, model: loaded(p.species), blend: p.width, height: p.height, reach, half_height, solid, parts: Vec::new() }
        })
        .collect();
    (groups, models)
}

impl World {
    /// The world: `probe` at [`PROBE`] in the one table's space, the tree over `lo`..`hi` filed with
    /// `meshes` (the scene's, in its order) and then the plants' `groups` on a grid of `cell` feet.
    /// The moving list is empty.
    pub fn new(
        probe: Collidable,
        meshes: &[Prop],
        groups: &[Group],
        models: Vec<Model>,
        cell: F,
        (lo, hi): ([F; 2], [F; 2]),
    ) -> World {
        let (mut tree, cell_size) = broad_build::tree_over(lo, hi, F(TREE_LEAST));
        // It reports the box its cells cover, which the gather's bound is measured in.
        tree.flags = 2;
        let mut objects = vec![probe];
        let mut places = vec![Placement { space: Some(0), ..Placement::default() }];
        let mut items = Vec::with_capacity(meshes.len() + groups.len());
        for p in meshes {
            let (c, h) = flight_search::prop_box(p);
            items.push((objects.len(), c, h));
            objects.push(Collidable {
                class: Class::Object,
                flags: MESH_FLAGS,
                nearby: true,
                radius: p.radius,
                category: MESH,
                collides_with: u32::MAX,
                // An empty list of categories it never collides with.
                categories: Categories { mode: 1, list: Vec::new() },
                ..Collidable::default()
            });
            places.push(Placement { centre: c, ..Placement::default() });
        }
        let first_group = objects.len();
        for g in groups {
            let (c, h) = g.bounds(cell);
            items.push((objects.len(), c, h));
            objects.push(Collidable { class: Class::Group, nearby: true, ..Collidable::default() });
            places.push(Placement { centre: c, cell: [g.cell[0], g.cell[2]], model: g.model, blend: g.blend, ..Placement::default() });
        }
        for (o, c, h) in items {
            let key = broad_build::key(&tree, hi, c, h);
            broad_build::insert(&mut tree, o, key);
        }
        let broad = Broad {
            spaces: vec![Space { next: None, tables: 0 }],
            tables: vec![Tables { own: Pair { fixed: Some(0), moving: Some(1) }, subs: Vec::new() }],
            structures: vec![Structure::Tree(tree), Structure::List(Blocks::default())],
            cell_size,
            group_cell: cell,
            models,
            places,
            ..Broad::default()
        };
        World { broad, objects, meshes: 1..first_group, groups: first_group..first_group + groups.len() }
    }

    /// Put the probe at `centre` (`w` 1).
    pub fn place_probe(&mut self, centre: [F; 3]) {
        self.broad.places[PROBE].centre = [centre[0], centre[1], centre[2], F::ONE];
    }
}

/// The quad's nearby list (quad `+0xc4`), as the gather fills it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Nearby {
    /// `+0`, `+4`: what was gathered, each with its distance.
    pub found: Vec<Candidate>,
    /// `+0x10`: the probe's centre when it was gathered; `+0x20` how far the queried cells reach past
    /// it, between `least` and the larger of the probe's reach and `extra`.
    pub centre: [F; 4],
    pub bound: F,
    /// `+0x24`: the half side of the box queried, and the least the bound is; `+0x28` a reach the
    /// bound may grow to beyond the probe's.
    pub least: F,
    pub extra: F,
    /// `+0x2c`: placed groups are never gathered.
    pub no_groups: bool,
    /// `+0x30`: it has been gathered.
    pub built: bool,
    /// `+0x34`: the most it holds.
    pub capacity: i32,
    /// `+0x38`: a collidable must pass the pair filter; without, only be enabled.
    pub filter: bool,
    /// `+0x3c`: distances on the ground plane (`x`, `z`) rather than in space.
    pub flat: bool,
}

impl Nearby {
    /// The quad's list as MD37a's images hold it: 10 ft each way, a reach of 30, room for 256, the
    /// pair filter, distances in space.
    pub fn quad() -> Nearby {
        Nearby { least: F(10.0), extra: F(30.0), capacity: 0x100, filter: true, ..Nearby::default() }
    }
}

/// `if a < b { a } else { b }`, the original's compare-and-pick, and the larger the same way.
fn least(a: F, b: F) -> F {
    if a < b { a } else { b }
}

fn most(a: F, b: F) -> F {
    if a < b { b } else { a }
}

/// How far `o`'s reach is from `p` (extra 0): `mula`/`madda`/`madd`, never below 0.
fn distance(objects: &[Collidable], broad: &Broad, o: usize, p: [F; 4]) -> F {
    let reach = objects[o].radius + F::ZERO;
    let c = broad.places[o].centre;
    let d = [c[0] - p[0], c[1] - p[1], c[2] - p[2]];
    let s = Acc::mula(d[0], d[0]).madda(d[1], d[1]).madd(d[2], d[2]).sqrt();
    F::ZERO.max(s - reach)
}

/// The same on the ground plane: `z` into the accumulator first.
fn flat_distance(objects: &[Collidable], broad: &Broad, o: usize, p: [F; 4]) -> F {
    let reach = objects[o].radius + F::ZERO;
    let c = broad.places[o].centre;
    let (dz, dx) = (c[2] - p[2], c[0] - p[0]);
    let s = Acc::mula(dz, dz).madd(dx, dx).sqrt();
    F::ZERO.max(s - reach)
}

/// How far a placed group's reach is from `p` on the ground plane: each axis `0 + p` less the cell
/// in feet (`adda`, `msub`), `z` into the accumulator first, less the model's reach blended (and `0`
/// added); 0 when inside.
fn group_distance(broad: &Broad, g: usize, p: [F; 4]) -> F {
    let place = &broad.places[g];
    let m = broad.models[usize::from(place.model)];
    let cell = |c: u16| F::from(i32::from(c));
    let dx = Acc::adda(F::ZERO, p[0]).msub(cell(place.cell[0]), broad.group_cell);
    let dz = Acc::adda(F::ZERO, p[2]).msub(cell(place.cell[1]), broad.group_cell);
    let s = Acc::mula(dz, dz).madd(dx, dx).sqrt();
    let reach = m.near + F::from(i32::from(place.blend)) * (m.far - m.near) / 128.0;
    let d = s - (F::ZERO + reach);
    if d <= 0.0 { F::ZERO } else { d }
}

/// Fill `list` around `probe` (the quad's with `kind` 3): query the box of side
/// `2 × least` around the probe's centre in the probe's chain of tables, and keep, up to the
/// capacity, every collidable the filter lets through (one gathered only within the bound, only
/// there) and every placed group within the bound. The bound is how far the queried cells reach past
/// the centre, at most the larger of the probe's reach and `extra`, at least `least`.
pub fn gather(objects: &mut [Collidable], broad: &mut Broad, list: &mut Nearby, probe: usize, kind: u32) {
    let centre = broad.places[probe].centre;
    let half = [list.least, list.least, list.least, F::ONE];
    list.found.clear();
    let (mut lo, mut hi) = ([F::ZERO; 4], [F::ZERO; 4]);
    let space = broad.places[probe].space.expect("a probe in a space");
    let mut q = Query::begin(broad, space, kind, centre, half, Some((&mut lo, &mut hi)), true);
    list.centre = centre;
    let reach = most(objects[probe].radius, list.extra);
    let cells = least(least(least(centre[0] - lo[0], centre[2] - lo[2]), hi[0] - centre[0]), hi[2] - centre[2]);
    list.bound = most(least(cells, reach), list.least);
    while let Some(o) = q.next(broad, Some((&mut lo, &mut hi))) {
        if !((list.found.len() as i32) < list.capacity) {
            break;
        }
        match objects[o].class {
            Class::Object => {
                objects[o].flags |= flags::SEARCHED;
                let d = if list.flat {
                    flat_distance(objects, broad, o, centre)
                } else {
                    distance(objects, broad, o, centre)
                };
                let kept = if list.filter {
                    accepts(objects, probe, o, false)
                } else {
                    objects[o].flags & flags::ENABLED != 0
                };
                if kept && (objects[o].flags & more_flags::WITHIN_BOUND == 0 || d < list.bound) {
                    list.found.push(Candidate { object: o, distance: d });
                }
            }
            Class::Group if !list.no_groups => {
                let d = group_distance(broad, o, centre);
                if d < list.bound {
                    list.found.push(Candidate { object: o, distance: d });
                }
            }
            _ => {}
        }
    }
    q.end(broad);
    list.built = true;
}

/// The ecosystem's cell for a terrain box: its larger side over 65536 ([`eco::cell_size`]).
pub fn plant_cell((lo, hi): ([F; 2], [F; 2])) -> F {
    let extent = most(hi[0] - lo[0], hi[1] - lo[1]);
    eco::cell_size(extent)
}
