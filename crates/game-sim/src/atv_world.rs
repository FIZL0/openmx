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
//! MD83 adds the rest of what SLUS-21104 files, each after the plants and in this order:
//!
//! - **the zones** ([`zones`], [`World::file_zones`]): the `.toy`'s `[TriggerN]` shapes, a sphere of
//!   `Radius` or a box of `Box` half sides turned by `LookVector`/`UpVector`, each stood at its
//!   `Position`, category [`ZONE`], boxed the way a mesh is ([`broad_build::mesh_box`]);
//! - **the start zone's mesh again** ([`start_zones`], [`World::file_again`]): a scene `COL=` of
//!   `Type=StartZone` (MC02's `Zone_Platform_Machine.col`) is filed a second time, the same box;
//! - **the vehicles' spheres** ([`World::add_vehicles`], [`World::move_vehicle`]): one per vehicle
//!   (category 0, the vehicle's own gather probe), filed at the origin and moved by
//!   `atv_spatial_move` with a box [`VEHICLE_HALF`] each way: kept in place under the same key,
//!   else taken out ([`take_out`]: its old slot left empty, a chunk or node it empties freed) and
//!   put in again (MD90);
//! - **the moving list** ([`World::put_moving`]): one block of [`MOVING_SLOTS`] slots the space's
//!   moving table walks after the tree — the quad's frame mesh, its riding probe and its sweep, the
//!   riders' parts ([`rider_sphere`], [`rider_sweep`]) and two category-0 sweeps ([`owned_sweep`]).
//!   A rider's nine spheres' radii are the rider file's ([`rider_radii`], MD83a).
//!
//! MD83b adds the vehicle's own gather round its sphere ([`World::place_vehicle`],
//! [`World::gather_vehicle`], [`VehicleSphere`]): each time the vehicle puts its sphere it keeps the
//! point, and it files the sphere there again ([`moving_key`]: nowhere if the box leaves the tree) only
//! when the last gather found it more than 10 ft from the one before. Its list ([`Nearby::vehicle`])
//! is gathered afresh, flat and over the static structures, only past those 10 ft.
//!
//! The tree's 239 entries of class 7 are scene nodes, not collidables: the gather passes over them.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::broad_build;
use crate::broad_phase::{Blocks, Broad, Model, Pair, Placement, Query, Space, Structure, Tables, more_flags};
use crate::eco::{self, Ecosystem, Group};
use crate::broad_phase::more_flags::ACTIVE;
use crate::flight_search;
use crate::math::{self, Mat4};
use crate::phy::Ini;
use crate::ragdoll_contact::Sphere;
use crate::track::{Scene, TerrainLayout};
use crate::world::Prop;
use crate::world_search::{Candidate, Categories, Class, Collidable, accepts, flags};
use ps2_float::{Acc, F, Vf, fpu, vu};

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
/// A zone's category, and its flag byte: enabled and active (a box zone's byte adds `0x40`, which
/// the gather does not read).
pub const ZONE: u32 = 0x8000;
pub const ZONE_FLAGS: u8 = flags::ENABLED | ACTIVE;
/// A vehicle's sphere (vehicle `+0xd80`): category 0, enabled and active; the half side of the box
/// `atv_spatial_move` files it with (`0x00525270`), and its reach while riding (`+0x170`; 5 when the
/// rider is off).
pub const VEHICLE: u32 = 0;
pub const VEHICLE_FLAGS: u8 = flags::ENABLED | ACTIVE;
pub const VEHICLE_HALF: f32 = 40.0;
pub const VEHICLE_REACH: f32 = 40.0;
pub const VEHICLE_OFF_REACH: f32 = 5.0;
/// How far, squared, a vehicle goes from where it last gathered before it gathers again (`+0xd3c`).
pub const VEHICLE_REGATHER: f32 = 100.0;
/// The moving list's one block (`+0x28`).
pub const MOVING_SLOTS: i32 = 0x400;
/// A rider part's category: every sphere and sweep the riders' bodies are made of.
pub const RIDER: u32 = 0x1000;
/// The structures [`World::new`] makes: the tree, then the moving list.
const TREE: usize = 0;
const MOVING: usize = 1;

/// The world, as the quad's gather reads it: the broad phase, and its collidables (the probe, then
/// the meshes, then the plants).
pub struct World {
    pub broad: Broad,
    pub objects: Vec<Collidable>,
    /// Where the scene's meshes are, and the plants' groups.
    pub meshes: std::ops::Range<usize>,
    pub groups: std::ops::Range<usize>,
    /// The key each vehicle's sphere is filed under in the tree (its record's `+0xc`; 0 in none).
    pub filed: std::collections::BTreeMap<usize, u32>,
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
        World { broad, objects, meshes: 1..first_group, groups: first_group..first_group + groups.len(), filed: Default::default() }
    }

    /// Put the probe at `centre` (`w` 1).
    pub fn place_probe(&mut self, centre: [F; 3]) {
        self.broad.places[PROBE].centre = [centre[0], centre[1], centre[2], F::ONE];
    }

    /// `c` added at `centre`, in no structure yet. Its number.
    pub fn add(&mut self, c: Collidable, centre: [F; 4]) -> usize {
        self.objects.push(c);
        self.broad.places.push(Placement { centre, ..Placement::default() });
        self.objects.len() - 1
    }

    fn tree(&mut self) -> &mut crate::broad_phase::Quadtree {
        let Structure::Tree(t) = &mut self.broad.structures[TREE] else { unreachable!("the tree is first") };
        t
    }

    /// `o` filed in the tree with the box `centre` ± `half`, the far corner the tree was laid to
    /// being `hi`.
    fn file(&mut self, o: usize, centre: [F; 4], half: [F; 4], hi: [F; 2]) {
        let tree = self.tree();
        let key = broad_build::key(tree, hi, centre, half);
        broad_build::insert(tree, o, key);
    }

    /// The `.toy`'s zones added and filed, in its order, after everything filed so far; where they
    /// are. `hi` is the tree's far corner ([`terrain_box`]'s).
    pub fn file_zones(&mut self, zones: &[Zone], hi: [F; 2]) -> std::ops::Range<usize> {
        let first = self.objects.len();
        for z in zones {
            let (c, h) = z.bounds();
            let o = self.add(z.collidable(), z.centre());
            self.file(o, c, h, hi);
        }
        first..self.objects.len()
    }

    /// The scene mesh `o` filed a second time, with the box it was filed with.
    pub fn file_again(&mut self, o: usize, prop: &Prop, hi: [F; 2]) {
        let (c, h) = flight_search::prop_box(prop);
        self.file(o, c, h, hi);
    }

    /// `n` vehicles' spheres, each at the origin with no reach and filed there, in order. Each is a
    /// probe in the one space: its vehicle gathers round it ([`World::gather_vehicle`]).
    pub fn add_vehicles(&mut self, n: usize, hi: [F; 2]) -> std::ops::Range<usize> {
        let first = self.objects.len();
        for _ in 0..n {
            let origin = [F::ZERO, F::ZERO, F::ZERO, F::ONE];
            let o = self.add(vehicle_sphere(), origin);
            self.broad.places[o].space = Some(0);
            let tree = self.tree();
            let key = broad_build::key(tree, hi, origin, vehicle_half());
            broad_build::insert(tree, o, key);
            self.filed.insert(o, key);
        }
        first..self.objects.len()
    }

    /// A vehicle's sphere `o` moved to `centre` with `reach` (`atv_spatial_move`): filed again with
    /// the box `centre` ± [`VEHICLE_HALF`] ([`World::refile`]). The vehicle files it only when it is
    /// put ([`World::place_vehicle`]) after a gather that found it 10 ft from the last one; between,
    /// its centre moves on with the vehicle and the tree is left alone.
    pub fn move_vehicle(&mut self, o: usize, centre: [F; 3], reach: F) {
        let centre = [centre[0], centre[1], centre[2], F::ONE];
        self.broad.places[o].centre = centre;
        self.objects[o].radius = reach;
        self.refile(o, centre);
    }

    /// `o` filed again with the box `centre` ± [`VEHICLE_HALF`], as `atv_spatial_move` does it in
    /// the cell's tree (MD89's [`atv_spatial::Tree`] pick, move, out and in): the new key is its
    /// [`moving_key`] (0, nowhere, unless the box is wholly in the tree). The same key as the one it
    /// is filed under: moved in place, which only widens the nodes' heights, and this tree keeps
    /// none. Another: taken out from under the old key ([`take_out`]), which frees a chunk or a node
    /// it leaves empty, and put in under the new one. Its centre and reach are left as they are.
    ///
    /// [`atv_spatial::Tree`]: crate::atv_spatial::Tree
    fn refile(&mut self, o: usize, centre: [F; 4]) {
        let old = self.filed.get(&o).copied().unwrap_or(0);
        let tree = self.tree();
        let key = moving_key(tree, centre, vehicle_half());
        if key == old {
            return;
        }
        take_out(tree, o, old);
        broad_build::insert(tree, o, key);
        self.filed.insert(o, key);
    }

    /// **`atv_vehicle_sphere_place`**: the vehicle's sphere put at `at`. It keeps `at` (all four
    /// lanes) and its sweep's top 10 ft above it, and reaches [`VEHICLE_REACH`], or
    /// [`VEHICLE_OFF_REACH`] with the rider `off`. If the last gather found it more than
    /// `sqrt(limit)` from where the one before it gathered, it is filed again at `at`
    /// (`atv_spatial_move`, reach 40 whatever its own). Then, if asked, the vehicle gathers round it
    /// ([`World::gather_vehicle`]).
    ///
    /// The original goes on to test what the list holds against the vehicle's sweep and plants, which
    /// can lift the caller's point and its answer says whether the place is clear; that is not ported.
    pub fn place_vehicle(&mut self, v: &mut VehicleSphere, at: [F; 4], off: bool, gather: bool) {
        v.at = at;
        v.top = [at[0], F(10.0) + at[1], at[2]];
        v.reach = F(if off { VEHICLE_OFF_REACH } else { VEHICLE_REACH });
        if v.moved > v.limit {
            self.refile(v.object, at);
        }
        if gather {
            self.gather_vehicle(v);
        }
    }

    /// **`atv_vehicle_sphere_gather`**'s gather: the sphere's centre and reach become where it was put
    /// and its reach; `moved` is its centre's squared distance from where it last gathered (a vector
    /// unit subtract, then `mula`/`madda`/`madd`), and past `limit` the list is gathered afresh round
    /// it on the ground plane (static structures only, kind 1) and the place kept.
    pub fn gather_vehicle(&mut self, v: &mut VehicleSphere) {
        let o = v.object;
        self.broad.places[o].centre = v.at;
        self.objects[o].radius = v.reach;
        let d: [F; 3] = std::array::from_fn(|i| F(vu::sub(v.at[i].0, v.gathered[i].0)));
        v.moved = Acc::mula(d[0], d[0]).madda(d[1], d[1]).madd(d[2], d[2]);
        if v.moved > v.limit {
            v.list.flat = true;
            gather(&mut self.objects, &mut self.broad, &mut v.list, o, 1);
            v.list.flat = false;
            v.gathered = v.at;
        }
    }

    /// `o` in the moving list's slot `slot`: the list's one block made at its first use, and the
    /// slots in use run to the last one taken.
    pub fn put_moving(&mut self, o: usize, slot: usize) {
        let Structure::List(l) = &mut self.broad.structures[MOVING] else { unreachable!("the moving list is second") };
        l.size = MOVING_SLOTS;
        let blocks = l.blocks.get_or_insert_with(|| vec![vec![None; MOVING_SLOTS as usize]]);
        blocks[0][slot] = Some(o);
        l.last = l.last.max(slot as i32 + 1);
    }
}

/// **The tree's out** (quadtree vt `+0x2c`, [`atv_spatial::Tree::take_out`]): `o` taken out from
/// under `key` (nothing for 0). Under each corner the key names, if the nodes down to the key's
/// level are all there, `o` is cleared from the first of the last one's chunks that holds it (every
/// slot of that chunk holding it), and the chunk dropped if that left it empty; a node left with no
/// entries and no children is unhooked from its parent. An unhooked node stays in `nodes`, empty and
/// out of every walk from the root.
///
/// [`atv_spatial::Tree::take_out`]: crate::atv_spatial::Tree::take_out
pub fn take_out(tree: &mut crate::broad_phase::Quadtree, o: usize, key: u32) {
    let Some(root) = tree.root else { return };
    if key == 0 {
        return;
    }
    let level = key & 0xf;
    let size = 0x4000 >> level;
    let (x, z) = (((key & 0xfff0_0000) >> 17) as i32, ((key & 0xfff00) >> 5) as i32);
    for corner in 0..4 {
        if key & (0x10 << corner) == 0 {
            continue;
        }
        let x = x.wrapping_add(if corner & 2 != 0 { size } else { 0 });
        let z = z.wrapping_add(if corner & 1 != 0 { size } else { 0 });
        let (mut node, mut parent, mut i, mut bit) = (Some(root), root, 0, 0x4000);
        for _ in 0..=level {
            let Some(n) = node else { break };
            i = 2 * usize::from(x & bit != 0) + usize::from(z & bit != 0);
            parent = n;
            node = tree.nodes[n].children[i];
            bit >>= 1;
        }
        let Some(n) = node else { continue };
        let leaf = &mut tree.nodes[n];
        let mut k = 0;
        while k < leaf.chunks.len() {
            let chunk = &mut leaf.chunks[k];
            let found = chunk.contains(&Some(o));
            chunk.iter_mut().filter(|s| **s == Some(o)).for_each(|s| *s = None);
            if chunk.iter().all(Option::is_none) {
                leaf.chunks.remove(k);
            } else {
                k += 1;
            }
            if found {
                break;
            }
        }
        if leaf.chunks.is_empty() && leaf.children.iter().all(Option::is_none) {
            tree.nodes[parent].children[i] = None;
        }
    }
}

/// The key a moving collidable's box is filed under when it moves (the tree's own pick): 0 unless
/// the box, less the tree's origin, lies wholly within its extent both ways. A static one's box is
/// clamped to the tree instead ([`broad_build::key`]), which within it comes to the same.
pub fn moving_key(tree: &crate::broad_phase::Quadtree, centre: [F; 4], half: [F; 4]) -> u32 {
    let [ox, oz] = tree.origin;
    let (x0, x1) = (centre[0] - half[0] - ox, centre[0] + half[0] - ox);
    let (z0, z1) = (centre[2] - half[2] - oz, centre[2] + half[2] - oz);
    let within = |lo: F, hi: F, extent: F| !(hi < 0.0) && lo <= extent && hi <= extent && !(lo < 0.0);
    if !within(x0, x1, tree.extent[0]) || !within(z0, z1, tree.extent[1]) {
        return 0;
    }
    broad_build::key(tree, [F(f32::MAX); 2], centre, half)
}

fn vehicle_half() -> [F; 4] {
    [F(VEHICLE_HALF), F(VEHICLE_HALF), F(VEHICLE_HALF), F::ONE]
}

/// A vehicle's own sphere (vehicle `+0xd80`, the world's collidable `object`) and what the vehicle
/// keeps round it.
#[derive(Clone, Debug, PartialEq)]
pub struct VehicleSphere {
    pub object: usize,
    /// `+0x160`: where it was last put; `+0x148`'s first point, its sweep's top, 10 ft above it.
    pub at: [F; 4],
    pub top: [F; 3],
    /// `+0x170`: how far it reaches when it next gathers.
    pub reach: F,
    /// Vehicle `+0xd84`: its list ([`Nearby::vehicle`]).
    pub list: Nearby,
    /// Vehicle `+0xd90`: where it last gathered; `+0xd38` how far, squared, the last gather found it
    /// from there; `+0xd3c` how far that may be before it gathers again.
    pub gathered: [F; 4],
    pub moved: F,
    pub limit: F,
}

impl VehicleSphere {
    /// The sphere `object` as a vehicle starts with it: at the origin, never gathered.
    pub fn new(object: usize) -> VehicleSphere {
        let origin = [F::ZERO, F::ZERO, F::ZERO, F::ONE];
        VehicleSphere {
            object,
            at: origin,
            top: [F::ZERO; 3],
            reach: F::ZERO,
            list: Nearby::vehicle(),
            gathered: origin,
            moved: F::ZERO,
            limit: F(VEHICLE_REGATHER),
        }
    }
}

/// A vehicle's sphere before it is placed: category [`VEHICLE`], collides with everything, an empty
/// list of categories it never collides with.
pub fn vehicle_sphere() -> Collidable {
    Collidable {
        flags: VEHICLE_FLAGS,
        nearby: true,
        category: VEHICLE,
        collides_with: u32::MAX,
        categories: Categories { mode: 1, list: Vec::new() },
        ..Collidable::default()
    }
}

/// One of a rider's parts, in the moving list by pairs: a sphere of `radius` (not enabled, active)
/// and a sweep (not enabled, active, with no-groups set) that collides only with category `0x200`.
pub fn rider_sphere(radius: F) -> Collidable {
    Collidable {
        flags: ACTIVE,
        nearby: true,
        radius,
        category: RIDER,
        collides_with: u32::MAX,
        categories: Categories { mode: 1, list: Vec::new() },
        ..Collidable::default()
    }
}

/// A rider's sphere radii, in the moving list's order, out of the rider file (`MXVSATV.PSR`'s
/// `Rider\Anim\mrider.phy`): one per `Type = Collision` constraint in file order, the `Radius` of
/// the `[PartN]` its `Part` names. MX Unleashed's rider spheres ([`Sphere::read`]), read the same way:
/// MC02's nine are parts 15, 18 (the feet), 1, 3, 4, 7, 11, 14 and 17.
pub fn rider_radii(rider: &Ini) -> Vec<F> {
    Sphere::read(rider).iter().map(|s| s.radius).collect()
}

pub fn rider_sweep() -> Collidable {
    Collidable {
        flags: ACTIVE | flags::NO_GROUPS,
        nearby: true,
        category: RIDER,
        collides_with: u32::MAX,
        categories: Categories { mode: 0, list: vec![0x200] },
        ..Collidable::default()
    }
}

/// The moving list's two category-0 sweeps (MC02's last two slots, each with an owner of its own):
/// not enabled, not struck by a nearby search, colliding only with categories `0x20` and `0x400`.
pub fn owned_sweep() -> Collidable {
    Collidable {
        flags: ACTIVE | flags::NO_GROUPS,
        collides_with: u32::MAX,
        categories: Categories { mode: 0, list: vec![0x20, 0x400] },
        ..Collidable::default()
    }
}

/// A trigger's shape: a sphere, or a box of half sides turned to face `look` with `up` up.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Sphere { radius: F },
    Box { half: [F; 3], look: Vf, up: Vf },
}

/// One of the `.toy`'s zones (`[TriggerN]`).
#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    pub name: String,
    pub position: [F; 3],
    pub shape: Shape,
}

/// The `.toy`'s zones in its order: `[Triggers]`' `NumberOfTriggers`, then each `[TriggerN]` that
/// says `Zone` (MC02's six all do), a box where it gives `Box`, else a sphere of `Radius`.
pub fn zones(toy: &Ini) -> Vec<Zone> {
    let n = toy.section("Triggers").int("NumberOfTriggers").unwrap_or(0);
    (1..=n)
        .filter_map(|i| {
            let s = toy.section(&format!("Trigger{i}"));
            if s.int("Zone").unwrap_or(0) == 0 {
                return None;
            }
            let p = s.vector("Position", Vf::new(0.0, 0.0, 0.0, 1.0));
            let shape = match s.text("Box") {
                Some(_) => {
                    let b = s.vector("Box", Vf::new(0.0, 0.0, 0.0, 1.0));
                    Shape::Box {
                        half: [F(b.x()), F(b.y()), F(b.z())],
                        look: s.vector("LookVector", Vf::new(0.0, 0.0, 1.0, 0.0)),
                        up: s.vector("UpVector", Vf::new(0.0, 1.0, 0.0, 0.0)),
                    }
                }
                None => Shape::Sphere { radius: s.float("Radius").unwrap_or(F::ZERO) },
            };
            Some(Zone { name: s.text("Name").unwrap_or("").to_string(), position: [F(p.x()), F(p.y()), F(p.z())], shape })
        })
        .collect()
}

/// Where in the scene's collision list a zone's mesh is (`Type=StartZone`): each is filed twice.
pub fn start_zones(scene: &Scene) -> Vec<usize> {
    scene.collision.iter().enumerate().filter(|(_, c)| c.kind.as_deref() == Some("StartZone")).map(|(i, _)| i).collect()
}

impl Zone {
    /// Its centre (`+0x50`), `w` 1.
    pub fn centre(&self) -> [F; 4] {
        [self.position[0], self.position[1], self.position[2], F::ONE]
    }

    /// A box's frame: its rows the side, `up` and `look`, normalised (no orthogonalising), and its
    /// position.
    pub fn frame(&self) -> Mat4 {
        let [x, y, z] = self.position;
        match &self.shape {
            Shape::Box { look, up, .. } => math::basis(Vf::new(x.0, y.0, z.0, 1.0), *look, *up, false, false),
            Shape::Sphere { .. } => Mat4::IDENTITY,
        }
    }

    /// How far it reaches (`+0x70`): a sphere's radius, a box's corner (`mula`/`madda`/`madd`, then
    /// the square root).
    pub fn reach(&self) -> F {
        match &self.shape {
            Shape::Sphere { radius } => *radius,
            Shape::Box { half: h, .. } => Acc::mula(h[0], h[0]).madda(h[1], h[1]).madd(h[2], h[2]).sqrt(),
        }
    }

    /// The box it is filed with, as the class gives it ([`broad_build::mesh_box`]): a sphere's local
    /// centre (its position) through the identity with the radius each way, a box's origin through
    /// its frame with its half sides.
    pub fn bounds(&self) -> ([F; 4], [F; 4]) {
        match &self.shape {
            Shape::Sphere { radius } => broad_build::mesh_box(&Mat4::IDENTITY, self.position, [*radius; 3]),
            Shape::Box { half, .. } => broad_build::mesh_box(&self.frame(), [F::ZERO; 3], *half),
        }
    }

    /// Its collidable: category [`ZONE`], enabled, struck by a nearby search, colliding with
    /// everything.
    pub fn collidable(&self) -> Collidable {
        Collidable {
            flags: ZONE_FLAGS,
            nearby: true,
            radius: self.reach(),
            category: ZONE,
            collides_with: u32::MAX,
            categories: Categories { mode: 1, list: Vec::new() },
            ..Collidable::default()
        }
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

    /// A vehicle's list round its own sphere (vehicle `+0xd84`): 10 ft each way, a reach of 30, room
    /// for 196, the pair filter, distances in space (its gather turns them flat while it runs).
    pub fn vehicle() -> Nearby {
        Nearby { least: F(10.0), extra: F(30.0), capacity: 0xc4, filter: true, ..Nearby::default() }
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
