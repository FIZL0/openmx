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
//! 1. **the candidates** — the world the plane is in, as the broad phase holds it ([`world`]): the
//!    props in the static tree laid over the track's field ([`crate::broad_build`]), the probes in the
//!    moving list; and the plane's nearby search ([`search`]) filled from it by S45's
//!    [`broad_phase::gather`] around the ray probe — gathered again only once the probe's reach leaves
//!    the bound, else kept and its distances measured again — in the order the tree's walk meets them
//!    (S96; [`Scene::new`]);
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
//!
//! **The other riders' bikes** (S95) are in the plane's world too: each bike's collidable is the one
//! `bike\col\bikef.col` box ([`BIKE_COLLISION`]) placed on its chassis' transform, category
//! [`BIKE`], in the moving list after the probes in player order ([`Bike`], [`Bikes`]). A bike the
//! plane strikes is a vehicle to its crash test, and its one listener, **the bike's struck listener**
//! ([`bike_struck`]), keeps what struck it for the bike's wreck test, which wrecks it only when that
//! category is in the bike's mask ([`struck_wreck`]) — never a plane's probe's.
//!
//! **The ecosystem's plants** (S102) are placed groups in the static tree after the props, each in
//! the box the ecosystem's loader files it with ([`crate::eco::Group::bounds`]), gathered within the
//! bound by its reach and searched part by part, unfiltered, by the ray probe (the sweep probe's flags
//! keep it off groups): a trunk is a mesh like a prop's, category [`TREE`] ([`Scene::wooded`]). The
//! wreck bike's ray probe searches them the same way ([`crate::ride::Crash::step`]).

#![deny(clippy::float_arithmetic)]

use crate::collide::MeshRef;
use crate::contact::Node as Frame;
use crate::contact_search::{Hit, Search, Struck, Swept};
use crate::ray_probe::{self, Facet, Matrix, RayProbe};
use crate::sweep_probe::{self, SweepProbe};
use crate::track::{self, Node};
use crate::world::{Prop, Props};
use crate::broad_build;
use crate::eco::Trees;
use crate::broad_phase::{self, Blocks, Broad, Pair, Placement, Space, Structure, Tables, more_flags};
use crate::world_search::{self, Categories, Class, Collidable, Narrow, Nearby, flags};
use crate::math::{self, Mat4};
use ps2_float::{Dest, F, Vf, vu};

/// The plane's ray collision file, in the machines' archive.
pub const BIPLANE_RAYS: &str = "Biplane\\col\\biplanem.col";

/// A scenery mesh's category (`+0x84`): what every collision mesh a track's scene places is in, read
/// off biplane.p2m2's save state (its 353 meshes in the tree). The 23 meshes nearest the plane there,
/// category 0x48, are the track's toys — the train and the machines — which the port does not have as
/// collidables (S96).
pub const SCENERY: u32 = 0x220;

/// A scenery mesh's flag byte: struck, and gathered only within the search's bound.
const SCENERY_FLAGS: u8 = flags::ENABLED | more_flags::WITHIN_BOUND;

/// A toy's category (`+0x84`) and flag byte (S141): the `.toy`'s cars' mesh collidables, which the
/// plane's world holds in its moving list ahead of the probes, gathered wherever their spheres are, in
/// the order the track built them (biplane.p2m2's state: its first 23 slots are free06's `Car1` to
/// `Car23`, each with its car as a mover of kind 2; flags 0x05 but for a search's marks). The
/// `HasPhysics` cars are toys like the path cars, in their `.toy` place among them (S172): the
/// subclass's init is the path car's first, which adds the mesh to the world as the reader reads it.
/// What they are once struck, a body of their own, is not here (S186).
pub const TOY: u32 = 0x48;
const TOY_FLAGS: u8 = flags::ENABLED | flags::NO_GROUPS;

/// The plane's nearby search as its collision object is built with it: the box queried is
/// `2 × SEARCH_LEAST` across, the bound may grow to `SEARCH_EXTRA`, and at most `SEARCH_CAPACITY`
/// candidates are kept (biplane.p2m2's save state).
pub const SEARCH_LEAST: f32 = 10.0;
pub const SEARCH_EXTRA: f32 = 30.0;
pub const SEARCH_CAPACITY: i32 = 128;

/// The world's static tree: its smallest node at least this wide; and the moving list's block size.
pub const TREE_LEAST: f32 = 96.0;
pub const MOVING_BLOCK: i32 = 1024;

/// The categories of the probes' own collidables, and what they collide with (`+0x84`, `+0xcc`).
pub const PROBE: u32 = 0x8;
pub const PROBE_COLLIDES_WITH: u32 = 0xffff_ffff;

/// The global mask the hit listener tests a struck category against (0x40 in every state). The toys'
/// category has it; the scene's meshes' does not.
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

/// The ride's bike collision file: a box round the bike and rider, the same in every archive that
/// carries it (the tracks' second archives and the bikes').
pub const BIKE_COLLISION: &str = "bike\\col\\bikef.col";

/// A bike collidable's category (`+0x84`) and flag byte: struck, gathered wherever its sphere is.
/// (biplane.p2m2's state: the three riders' bikes in the plane's list, flags 0x14 with a search's
/// mark 0x10.) [`VEHICLE`] is in the category, so a plane that strikes a bike does not wreck on it.
pub const BIKE: u32 = 0x9_a048;
const BIKE_FLAGS: u8 = flags::ENABLED;

/// The bike's mask of what wrecks it when it strikes it (`+0x398`, 0x100000 on every bike in the
/// state): its wreck test's last clause reads the struck listener's mark against it.
pub const BIKE_STRUCK_MASK: u32 = 0x10_0000;

/// What the plane's collision record keeps as struck ([`crate::contact_search::Record::struck`])
/// when it is the ride's bike `k`: `BIKES + k`. A prop is its slot in [`world`]'s collidables.
pub const BIKES: usize = 1 << 16;

/// A trunk's category (`+0x84`): the word the ecosystem's loader gives every cylinder (S97).
pub const TREE: u32 = 4;
const TREE_FLAGS: u8 = flags::ENABLED;

/// What the collision record keeps as struck when it is trunk `j` of the world's trees
/// ([`crate::eco::Trees::props`]): `TREES + j`.
pub const TREES: usize = 1 << 17;

/// What the plane's collision record keeps as struck when it is the ride's other plane `j`
/// ([`Scene::among_planes`]): `PLANES + j`.
pub const PLANES: usize = 1 << 18;

/// Another plane in the plane's world (N22a1): its hull (the plane's [`crate::flight::HULL`]) where
/// its collision node stands it, and its body's velocity, which the sweep reads of it when struck.
#[derive(Clone, Copy)]
pub struct Plane<'a> {
    pub mesh: &'a track::Mesh,
    pub at: Mat4,
    pub velocity: Vf,
}

/// A plane's hull's collidable as another plane's world holds it: the riders' world's
/// ([`crate::rider_contact::gather_met`]).
fn plane_collidable(radius: F) -> Collidable {
    Collidable {
        class: Class::Object,
        flags: flags::ENABLED,
        nearby: true,
        radius,
        category: crate::flight::COLLIDABLE,
        collides_with: crate::flight::COLLIDES_WITH,
        categories: any_category(),
        ..Collidable::default()
    }
}

/// The bike's struck listener (its collidable's one listener, `vt +0xc`): it keeps what struck the
/// bike — the category of the collidable that did — for the bike's next wreck test, over whatever
/// struck it before.
pub fn bike_struck(mark: &mut Option<u32>, by: u32) {
    *mark = Some(by);
}

/// The bike's wreck test's last clause, after every other: a bike that something struck since its
/// last test wrecks — reason 5, [`crate::air::Wreck::Flat`] — when the striker's category is in the
/// bike's `mask` ([`BIKE_STRUCK_MASK`]). The test clears the mark whatever it answers. The plane's
/// probes are [`PROBE`], which no bike's mask has.
pub fn struck_wreck(mark: Option<u32>, mask: u32) -> Option<crate::air::Wreck> {
    mark.filter(|by| by & mask != 0).map(|_| crate::air::Wreck::Flat)
}

/// One of the ride's bikes as the plane's world holds it: its chassis' transform, which its collidable
/// is placed on, and whether it is in the world. A downed rider's bike has left it (the state's
/// wrecked one is in no list) and so has a ghost's; one gathered before it left is still searched
/// until the plane gathers again, as the original's is (the wrecked one is among the state's 34).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bike {
    pub transform: Mat4,
    pub in_world: bool,
}

/// The ride's bikes, in player order, and the collision mesh they share.
#[derive(Clone, Copy)]
pub struct Bikes<'a> {
    pub mesh: &'a track::Mesh,
    pub bikes: &'a [Bike],
}

/// A bike's sphere where `t` stands it: its mesh's root box's centre through the frame, on the FPU,
/// and the box's half diagonal (the collidable's `+0x40` is the box's, [`bike_box`]).
fn bike_sphere(mesh: &track::Mesh, t: &Mat4) -> (Vf, F) {
    match mesh.nodes.first() {
        Some(n) => {
            let c = Vf::new(n.centre[0].0, n.centre[1].0, n.centre[2].0, 1.0);
            let h = Vf::new(n.half_extent[0].0, n.half_extent[1].0, n.half_extent[2].0, 0.0);
            (math::transform_point(t, c), length(h))
        }
        None => (Vf::new(0.0, 0.0, 0.0, 1.0), F::ZERO),
    }
}

/// A bike's box in the world: its mesh's root node's through the chassis' frame.
pub fn bike_box(mesh: &track::Mesh, t: &Mat4) -> ([F; 4], [F; 4]) {
    match mesh.nodes.first() {
        Some(n) => broad_build::mesh_box(t, n.centre, n.half_extent),
        None => (four(t.0[3]), [F::ZERO, F::ZERO, F::ZERO, F::ONE]),
    }
}

/// The two probes' slots in [`world`]'s collidables; the props follow, prop `i` at `FIRST + i`, and
/// the ride's bikes after them.
pub const RAY: usize = 0;
pub const SWEEP: usize = 1;
pub const FIRST: usize = 2;

/// The plane's nearby search, empty, as its collision object is built with it.
pub fn search() -> broad_phase::Search {
    broad_phase::Search {
        least: F(SEARCH_LEAST),
        extra: F(SEARCH_EXTRA),
        capacity: SEARCH_CAPACITY,
        ..broad_phase::Search::default()
    }
}

fn four(v: Vf) -> [F; 4] {
    [F(v.x()), F(v.y()), F(v.z()), F(v.w())]
}

/// A prop's box in the world: its root node's through its frame, or a point where it has no tree.
pub fn prop_box(p: &Prop) -> ([F; 4], [F; 4]) {
    match p.mesh.nodes.first() {
        Some(n) => broad_build::mesh_box(&p.to_world, n.centre, n.half_extent),
        None => (four(p.centre), [F::ZERO, F::ZERO, F::ZERO, F::ONE]),
    }
}

/// The ground-plane box the world's tree is laid over: the field's, or, in a world without one, the
/// props' boxes together (and the unit square). A field without edges — the port's own worlds' — is
/// no field here.
fn tree_box(props: &[Prop], field: Option<([F; 2], [F; 2])>) -> ([F; 2], [F; 2]) {
    edged(field).unwrap_or_else(|| boxes_box(props.iter().map(prop_box)))
}

/// The field, if it has edges.
fn edged(field: Option<([F; 2], [F; 2])>) -> Option<([F; 2], [F; 2])> {
    const EDGE: f32 = 16_777_216.0;
    field.filter(|(lo, hi)| lo.iter().chain(hi).all(|&v| -EDGE < v && v < EDGE))
}

/// The boxes together on the ground plane, and the unit square.
fn boxes_box(boxes: impl Iterator<Item = ([F; 4], [F; 4])>) -> ([F; 2], [F; 2]) {
    let (mut lo, mut hi) = ([F::ZERO; 2], [F::ONE; 2]);
    for (c, h) in boxes {
        for (k, i) in [(0, 0), (1, 2)] {
            lo[k] = lo[k].min(c[i] - h[i]);
            hi[k] = hi[k].max(c[i] + h[i]);
        }
    }
    (lo, hi)
}

/// The world as the plane's broad phase holds it, with the ray probe's reach at `centre`, `radius`:
/// one table, its static tree holding the props in scene order (category [`SCENERY`], gathered only
/// within the bound) and its moving list the two probes, which nothing strikes. `field` is the
/// terrain's box on the ground plane (`x`, `z`), which the tree is laid over.
pub fn world(props: &[Prop], field: Option<([F; 2], [F; 2])>, centre: Vf, radius: F) -> (Broad, Vec<Collidable>) {
    world_among(props, None, field, centre, radius)
}

/// [`world`] with the ride's bikes in it (S95): after the props, bike `k` at `FIRST + props.len() + k`,
/// category [`BIKE`] with its one listener; the ones in the world in the moving list after the probes,
/// in player order (where the state's are).
pub fn world_among(
    props: &[Prop],
    bikes: Option<&Bikes<'_>>,
    field: Option<([F; 2], [F; 2])>,
    centre: Vf,
    radius: F,
) -> (Broad, Vec<Collidable>) {
    world_of(props, bikes, field, centre, radius, false)
}

/// [`world_among`], and with `toys` the props that move ([`Prop::moving`], the `.toy`'s cars) are toys
/// ([`TOY`]): in the moving list ahead of the probes rather than in the tree, as the plane's world
/// holds them (S141), and the wreck bike's and the put-back query's with it (S171). Without, every
/// prop is scenery, as the worlds the port has not yet told about the cars (the riding bike's wheel
/// and body searches, the props' own world) still hold them.
fn world_of(
    props: &[Prop],
    bikes: Option<&Bikes<'_>>,
    field: Option<([F; 2], [F; 2])>,
    centre: Vf,
    radius: F,
    toys: bool,
) -> (Broad, Vec<Collidable>) {
    let (mut broad, objects, filing) = world_unfiled(props, bikes, &[], field, centre, radius, toys);
    filing.file(&mut broad);
    (broad, objects)
}

/// What a world's static tree is filed with, in order (the props', then the plants'), and the box it
/// is laid over: the tree [`Filing::file`] fills is a function of these alone (P205).
struct Filing {
    lo: [F; 2],
    hi: [F; 2],
    items: Vec<(usize, [F; 4], [F; 4])>,
}

/// How many filled trees each thread keeps ([`Filing::file`]): a tick's worlds are a few (the bike's
/// three searches share one; the plane's, the wreck's and the put-back's are the flown one), more
/// only where a world of many tiles hands over another square's set.
const FILED_KEPT: usize = 8;

thread_local! {
    /// The trees last filled, newest first, each under its [`Filing`] as bits.
    static FILED: std::cell::RefCell<Vec<(Vec<u32>, broad_phase::Quadtree)>> = const { std::cell::RefCell::new(Vec::new()) };
}

impl Filing {
    /// Every item into `broad`'s tree as [`broad_build::key`] places it. The ride builds its worlds
    /// afresh for every search, three or more a tick, and filing every prop and plant of the near set
    /// was half the browser's frame (P205); the tree is a pure function of the box and the items, so
    /// one filed from the same items, bit for bit, is the same tree and is copied rather than filed
    /// again.
    fn file(self, broad: &mut Broad) {
        let Structure::Tree(tree) = &mut broad.structures[0] else { unreachable!("the table's first structure is its tree") };
        let bits = |v: &[F]| v.iter().map(|f| f.0.to_bits()).collect::<Vec<u32>>();
        let mut stamp = Vec::with_capacity(4 + 9 * self.items.len());
        stamp.extend(bits(&self.lo));
        stamp.extend(bits(&self.hi));
        for (o, c, h) in &self.items {
            stamp.push(u32::try_from(*o).expect("a collidable's number fits a word"));
            stamp.extend(bits(c));
            stamp.extend(bits(h));
        }
        FILED.with(|filed| {
            let mut filed = filed.borrow_mut();
            if let Some(k) = filed.iter().position(|(s, _)| *s == stamp) {
                let kept = filed.remove(k);
                *tree = kept.1.clone();
                filed.insert(0, kept);
                return;
            }
            for (o, c, h) in &self.items {
                let key = broad_build::key(tree, self.hi, *c, *h);
                broad_build::insert(tree, *o, key);
            }
            filed.insert(0, (stamp, tree.clone()));
            filed.truncate(FILED_KEPT);
        });
    }
}

/// [`world_of`] with its tree laid but nothing filed in it yet, and what to file.
fn world_unfiled(
    props: &[Prop],
    bikes: Option<&Bikes<'_>>,
    planes: &[Plane<'_>],
    field: Option<([F; 2], [F; 2])>,
    centre: Vf,
    radius: F,
    toys: bool,
) -> (Broad, Vec<Collidable>, Filing) {
    let (b, u, g) = BUILDS.get();
    BUILDS.set((b + 1, u, g));
    let probe = |flags, radius| Collidable {
        class: Class::Object,
        flags,
        nearby: true,
        radius,
        category: PROBE,
        collides_with: PROBE_COLLIDES_WITH,
        categories: any_category(),
        ..Collidable::default()
    };
    let mut objects = vec![probe(RAY_FLAGS, radius), probe(SWEEP_FLAGS, F::ZERO)];
    let at = Placement { space: Some(0), centre: four(centre), ..Placement::default() };
    let mut places = vec![at.clone(), at];
    let (lo, hi) = tree_box(props, field);
    let (mut tree, cell_size) = broad_build::tree_over(lo, hi, F(TREE_LEAST));
    // It reports the box its cells cover, which is what the gather's bound is measured in.
    tree.flags = 2;
    let mut filing = Filing { lo, hi, items: Vec::new() };
    let mut moving = Vec::new();
    for (i, p) in props.iter().enumerate() {
        if toys && p.moving.is_some() {
            objects.push(Collidable {
                class: Class::Object,
                flags: TOY_FLAGS,
                nearby: true,
                radius: p.radius,
                category: TOY,
                collides_with: PROBE_COLLIDES_WITH,
                categories: any_category(),
                ..Collidable::default()
            });
            places.push(Placement { centre: prop_box(p).0, ..Placement::default() });
            moving.push(FIRST + i);
            continue;
        }
        objects.push(Collidable {
            class: Class::Object,
            flags: SCENERY_FLAGS,
            nearby: true,
            radius: p.radius,
            category: SCENERY,
            collides_with: PROBE_COLLIDES_WITH,
            categories: any_category(),
            ..Collidable::default()
        });
        // At its box's centre, which is what the gather measures from (the sum's order is the box's,
        // so not quite [`Prop::centre`]).
        let (c, h) = prop_box(p);
        places.push(Placement { centre: c, ..Placement::default() });
        filing.items.push((FIRST + i, c, h));
    }
    let mut block = vec![None; MOVING_BLOCK as usize];
    moving.extend([RAY, SWEEP]);
    for (slot, &o) in block.iter_mut().zip(&moving) {
        *slot = Some(o);
    }
    let mut last = moving.len().min(block.len()) as i32;
    let first_plane = FIRST + props.len() + bikes.map_or(0, |b| b.bikes.len());
    for j in 0..planes.len() {
        if (last as usize) < block.len() {
            block[last as usize] = Some(first_plane + j);
            last += 1;
        }
    }
    for (k, b) in bikes.map_or(&[][..], |b| b.bikes).iter().enumerate() {
        let mesh = bikes.map(|b| b.mesh).expect("bikes come with their mesh");
        let (_, radius) = bike_sphere(mesh, &b.transform);
        let object = objects.len();
        objects.push(Collidable {
            class: Class::Object,
            flags: BIKE_FLAGS,
            nearby: true,
            radius,
            category: BIKE,
            collides_with: PROBE_COLLIDES_WITH,
            categories: any_category(),
            listeners: vec![k],
            ..Collidable::default()
        });
        let (c, _) = bike_box(mesh, &b.transform);
        places.push(Placement { centre: c, ..Placement::default() });
        if b.in_world && (last as usize) < block.len() {
            block[last as usize] = Some(object);
            last += 1;
        }
    }
    for p in planes {
        let (_, centre, radius) = Prop::posed(p.mesh, &p.at);
        objects.push(plane_collidable(radius));
        places.push(Placement { centre: four(centre), ..Placement::default() });
    }
    let list = Blocks { size: MOVING_BLOCK, last, blocks: Some(vec![block]), ..Blocks::default() };
    let broad = Broad {
        spaces: vec![Space { next: None, tables: 0 }],
        tables: vec![Tables { own: Pair { fixed: Some(0), moving: Some(1) }, subs: Vec::new() }],
        structures: vec![Structure::Tree(tree), Structure::List(list)],
        cell_size,
        places,
        ..Broad::default()
    };
    (broad, objects, filing)
}

/// [`world_among`] with the ecosystem's plants standing in it (S102): after the bikes, plant `g` as a
/// placed group at `FIRST + props + bikes + g`, filed in the static tree after the props in the
/// file's order, and after the groups every trunk, `j` at the groups' end plus `j`, in no structure —
/// a group's parts are found through it.
pub fn world_wooded(
    props: &[Prop],
    trees: Option<&Trees>,
    bikes: Option<&Bikes<'_>>,
    field: Option<([F; 2], [F; 2])>,
    centre: Vf,
    radius: F,
) -> (Broad, Vec<Collidable>) {
    wooded_of(props, trees, bikes, field, centre, radius, false)
}

/// [`world_wooded`] as the plane's world holds it (S141): the path cars are toys ([`world_of`]). The
/// wreck bike's world and the put-back query's are this one (S171).
pub fn world_flown(
    props: &[Prop],
    trees: Option<&Trees>,
    bikes: Option<&Bikes<'_>>,
    field: Option<([F; 2], [F; 2])>,
    centre: Vf,
    radius: F,
) -> (Broad, Vec<Collidable>) {
    wooded_of(props, trees, bikes, field, centre, radius, true)
}

fn wooded_of(
    props: &[Prop],
    trees: Option<&Trees>,
    bikes: Option<&Bikes<'_>>,
    field: Option<([F; 2], [F; 2])>,
    centre: Vf,
    radius: F,
    toys: bool,
) -> (Broad, Vec<Collidable>) {
    let (mut broad, objects, filing) = wooded_unfiled(props, trees, bikes, &[], field, centre, radius, toys);
    filing.file(&mut broad);
    (broad, objects)
}

/// [`wooded_of`] with its tree laid but nothing filed in it yet, and what to file.
fn wooded_unfiled(
    props: &[Prop],
    trees: Option<&Trees>,
    bikes: Option<&Bikes<'_>>,
    planes: &[Plane<'_>],
    field: Option<([F; 2], [F; 2])>,
    centre: Vf,
    radius: F,
    toys: bool,
) -> (Broad, Vec<Collidable>, Filing) {
    let Some(trees) = trees else { return world_unfiled(props, bikes, planes, field, centre, radius, toys) };
    // With no terrain to lay the tree over (none, or one with no edge), it spans the trunks as well
    // as the props.
    let bare = tree_box(props, None);
    let field = match tree_box(props, field) {
        laid if field.is_some() && laid != bare => Some(laid),
        _ => {
            let ((a, b), (c, d)) = (bare, tree_box(&trees.props.props, None));
            Some(([a[0].min(c[0]), a[1].min(c[1])], [b[0].max(d[0]), b[1].max(d[1])]))
        }
    };
    // The plants are filed after the props, in the same tree over the same box.
    let (mut broad, mut objects, mut filing) = world_unfiled(props, bikes, planes, field, centre, radius, toys);
    for g in trees.groups() {
        let (c, h) = g.bounds(trees.cell());
        let o = objects.len();
        objects.push(Collidable { class: Class::Group, nearby: true, ..Collidable::default() });
        broad.places.push(Placement { centre: c, cell: [g.cell[0], g.cell[2]], model: g.model, blend: g.blend, ..Placement::default() });
        filing.items.push((o, c, h));
    }
    for p in &trees.props.props {
        objects.push(Collidable {
            class: Class::Object,
            flags: TREE_FLAGS,
            nearby: true,
            radius: p.radius,
            category: TREE,
            collides_with: PROBE_COLLIDES_WITH,
            categories: any_category(),
            ..Collidable::default()
        });
        broad.places.push(Placement { centre: prop_box(p).0, ..Placement::default() });
    }
    broad.group_cell = trees.cell();
    broad.models = trees.models().to_vec();
    (broad, objects, filing)
}

/// **A world kept between searches** (S236). A tick's searches — the bike's wheels every substep, its
/// sweep back, the props', the plane's two — each asked [`wooded_of`] for the world afresh, and on the
/// Steam Deck building it (a collidable, a placement and a box for every prop, plant and trunk, the
/// tree filed or copied) was most of a tick. The world is a function of the props, the plants, the
/// field and `toys`; of the ray probe's reach, which only the two probes carry; and of the bikes'
/// transforms, which only the bikes' own collidables and the moving list's tail carry. So a world is
/// built once under a stamp of every bit the build reads but those ([`stamp_world`]) and kept; each
/// search puts the probes and the bikes where they are now ([`Kept::place`]) and the broad phase's
/// query state back as built ([`Kept::rewind`]). When only some props have moved since — the path
/// cars, every tick — the kept world is brought up to them ([`Kept::update`]) rather than built again.
/// Either way it is the `Broad` and the collidables a fresh build gives, field for field
/// ([`kept_world`]).
struct Kept {
    stamp: Vec<u32>,
    /// Where the props' words are in the stamp ([`PROP_WORDS`] each).
    props_at: std::ops::Range<usize>,
    /// Which build or update of a world this is, for the collidable lists searched in it ([`lend`]).
    generation: u64,
    broad: Broad,
    objects: Vec<Collidable>,
    /// The moving list's slots ahead of the bikes' (the toys and the two probes), and the first bike's
    /// collidable.
    moving: i32,
    bikes: usize,
    /// The first other plane's collidable (N22a1), after the bikes'.
    planes: usize,
    /// Each prop's box ([`prop_box`]); the trunks' boxes together ([`boxes_box`]), with plants; and
    /// the box the tree is laid over.
    boxes: Vec<([F; 4], [F; 4])>,
    trunks: Option<([F; 2], [F; 2])>,
    lo: [F; 2],
    hi: [F; 2],
    /// What the tree is filed with, in order: each item's collidable, its key, and how many changes
    /// the tree's log had before it ([`broad_build::insert_logged`]).
    filed: Vec<(usize, u32, usize)>,
    log: broad_build::Log,
}

/// How many built worlds each thread keeps: a tick's are the riding one (the bike's searches and the
/// props'), the flown one (the plane's, the wreck bike's, the put-back's), and another square's set in
/// a world of many tiles.
const WORLDS_KEPT: usize = 8;

/// Words a prop puts in a world's stamp ([`stamp_props`]).
const PROP_WORDS: usize = 25;

thread_local! {
    /// The worlds kept, newest first, and the stamp being made for the next search.
    static KEPT: std::cell::RefCell<(Vec<Kept>, Vec<u32>)> = const { std::cell::RefCell::new((Vec::new(), Vec::new())) };
    /// How many worlds this thread has built and how many kept ones it has brought up to moved props
    /// ([`world_builds`]), and the last generation handed out ([`Kept::generation`]).
    static BUILDS: std::cell::Cell<(u64, u64, u64)> = const { std::cell::Cell::new((0, 0, 0)) };
    /// Collidable lists a search was done with, each under the generation it was copied from.
    static SPARE: std::cell::RefCell<Vec<(u64, Vec<Collidable>)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// How many worlds the searches on this thread have built since it started, and how many kept ones
/// they have brought up to props that moved ([`Kept::update`]). A world kept from an earlier search is
/// neither: a steady ride builds none a search, and updates one a tick while the path cars drive.
pub fn world_builds() -> (u64, u64) {
    let (built, updated, _) = BUILDS.get();
    (built, updated)
}

/// Drop the worlds this thread keeps, and the collidable lists lent with them: the next search builds
/// afresh.
pub fn forget_worlds() {
    KEPT.with(|k| k.borrow_mut().0.clear());
    SPARE.with(|s| s.borrow_mut().clear());
}

/// The world a search with these would be handed ([`world_wooded`], or with `toys` [`world_flown`]),
/// as the worlds this thread keeps give it: the one to hold up against a fresh build.
#[allow(clippy::too_many_arguments)]
pub fn kept(
    props: &[Prop],
    trees: Option<&Trees>,
    bikes: Option<&Bikes<'_>>,
    field: Option<([F; 2], [F; 2])>,
    centre: Vf,
    radius: F,
    toys: bool,
) -> (Broad, Vec<Collidable>) {
    kept_world(props, trees, bikes, &[], field, centre, radius, toys, |broad, built, _| (broad.clone(), built.to_vec()))
}

fn generation() -> u64 {
    let (b, u, g) = BUILDS.get();
    BUILDS.set((b, u, g + 1));
    g + 1
}

fn bits(v: &[F]) -> impl Iterator<Item = u32> + '_ {
    v.iter().map(|f| f.0.to_bits())
}

/// What each prop gives the build, [`PROP_WORDS`] apiece after their count: whether it moves, its
/// reach, and what its box comes from — its root node through its frame, or its centre without a tree.
/// Where the props' words are.
fn stamp_props(stamp: &mut Vec<u32>, props: &[Prop]) -> std::ops::Range<usize> {
    stamp.push(props.len() as u32);
    let start = stamp.len();
    for p in props {
        stamp.push(u32::from(p.moving.is_some()));
        stamp.push(p.radius.0.to_bits());
        match p.mesh.nodes.first() {
            Some(n) => {
                stamp.push(1);
                stamp.extend(bits(&n.centre).chain(bits(&n.half_extent)));
            }
            None => {
                stamp.push(0);
                stamp.extend(p.centre.0.iter().map(|f| f.to_bits()).chain([0, 0]));
            }
        }
        stamp.extend(p.to_world.0.iter().flat_map(|r| r.0.map(f32::to_bits)));
    }
    debug_assert_eq!(stamp.len() - start, PROP_WORDS * props.len());
    start..stamp.len()
}

/// Every bit [`wooded_of`] reads but the probes' reach and the bikes' transforms; where the props'
/// words are.
fn stamp_world(
    stamp: &mut Vec<u32>,
    props: &[Prop],
    trees: Option<&Trees>,
    bikes: usize,
    planes: usize,
    field: Option<([F; 2], [F; 2])>,
    toys: bool,
) -> std::ops::Range<usize> {
    stamp.clear();
    stamp.push(u32::from(toys));
    match field {
        Some((lo, hi)) => {
            stamp.push(1);
            stamp.extend(bits(&lo).chain(bits(&hi)));
        }
        None => stamp.push(0),
    }
    stamp.push(bikes as u32);
    stamp.push(planes as u32);
    let at = stamp_props(stamp, props);
    let Some(trees) = trees else {
        stamp.push(0);
        return at;
    };
    stamp.push(1);
    stamp.push(trees.cell().0.to_bits());
    stamp.push(trees.groups().len() as u32);
    for g in trees.groups() {
        stamp.extend(g.cell.map(u32::from));
        stamp.push(u32::from(g.model) | u32::from(g.blend as u8) << 8);
        stamp.extend([g.reach.0.to_bits(), g.half_height.0.to_bits()]);
    }
    stamp.push(trees.models().len() as u32);
    for m in trees.models() {
        stamp.extend([m.near.0.to_bits(), m.far.0.to_bits()]);
    }
    stamp_props(stamp, &trees.props.props);
    at
}

/// The box [`wooded_of`]'s tree is laid over, from the props' boxes together (`bare`) and, with
/// plants, the trunks': the field if it has edges, else the props'; with plants, the field only where
/// it is not the props' box, else the props' and the trunks' together, if that has edges.
fn laid_box(bare: ([F; 2], [F; 2]), trunks: Option<([F; 2], [F; 2])>, field: Option<([F; 2], [F; 2])>) -> ([F; 2], [F; 2]) {
    let field = match trunks {
        None => field,
        Some((c, d)) => match edged(field).unwrap_or(bare) {
            laid if field.is_some() && laid != bare => Some(laid),
            _ => {
                let (a, b) = bare;
                Some(([a[0].min(c[0]), a[1].min(c[1])], [b[0].max(d[0]), b[1].max(d[1])]))
            }
        },
    };
    edged(field).unwrap_or(bare)
}

impl Kept {
    /// [`wooded_of`]'s world, its tree filed item by item with each change logged.
    #[allow(clippy::too_many_arguments)]
    fn build(
        props: &[Prop],
        trees: Option<&Trees>,
        bikes: Option<&Bikes<'_>>,
        planes: &[Plane<'_>],
        field: Option<([F; 2], [F; 2])>,
        centre: Vf,
        radius: F,
        toys: bool,
        stamp: &[u32],
        props_at: std::ops::Range<usize>,
    ) -> Kept {
        let (mut broad, objects, filing) = wooded_unfiled(props, trees, bikes, planes, field, centre, radius, toys);
        let Structure::Tree(tree) = &mut broad.structures[0] else { unreachable!("the table's first structure is its tree") };
        let mut log = broad_build::Log::default();
        let mut filed = Vec::with_capacity(filing.items.len());
        for &(o, c, h) in &filing.items {
            let key = broad_build::key(tree, filing.hi, c, h);
            filed.push((o, key, log.changes.len()));
            broad_build::insert_logged(tree, o, key, &mut log);
        }
        let moving = if toys { props.iter().filter(|p| p.moving.is_some()).count() } else { 0 };
        Kept {
            stamp: stamp.to_vec(),
            props_at,
            generation: generation(),
            broad,
            objects,
            moving: (moving + 2).min(MOVING_BLOCK as usize) as i32,
            bikes: FIRST + props.len(),
            planes: FIRST + props.len() + bikes.map_or(0, |b| b.bikes.len()),
            boxes: props.iter().map(prop_box).collect(),
            trunks: trees.map(|t| boxes_box(t.props.props.iter().map(prop_box))),
            lo: filing.lo,
            hi: filing.hi,
            filed,
            log,
        }
    }

    /// Bring the world up to `props`, which differ from those it was built with only in some props'
    /// words (`stamp` is theirs): each moved prop's collidable reach, placement and box, and the tree
    /// taken back to before the first of them was filed and the rest filed again with their keys, the
    /// moved ones' made anew. Nothing, and `false`, when it cannot be: a prop that has become a toy or
    /// stopped being one, or a tree laid over another box.
    fn update(&mut self, props: &[Prop], field: Option<([F; 2], [F; 2])>, toys: bool, stamp: &[u32]) -> bool {
        let at = self.props_at.start;
        let words = |i: usize| -> std::ops::Range<usize> { at + i * PROP_WORDS..at + (i + 1) * PROP_WORDS };
        let mut moved = Vec::new();
        for i in 0..props.len() {
            let w = words(i);
            if self.stamp[w.clone()] != stamp[w.clone()] {
                if toys && self.stamp[w.start] != stamp[w.start] {
                    return false;
                }
                moved.push((i, prop_box(&props[i])));
            }
        }
        let mut next = moved.iter().peekable();
        let boxes = self.boxes.iter().enumerate().map(|(i, &b)| match next.next_if(|(j, _)| *j == i) {
            Some(&(_, b)) => b,
            None => b,
        });
        if laid_box(boxes_box(boxes), self.trunks, field) != (self.lo, self.hi) {
            return false;
        }
        for &(i, b) in &moved {
            self.boxes[i] = b;
            self.objects[FIRST + i].radius = props[i].radius;
            self.broad.places[FIRST + i].centre = b.0;
        }
        let Structure::Tree(tree) = &mut self.broad.structures[0] else { unreachable!("the table's first structure is its tree") };
        let mut first = None;
        for (k, (o, key, _)) in self.filed.iter_mut().enumerate() {
            let Some(i) = o.checked_sub(FIRST).filter(|&i| i < props.len()) else { continue };
            if let Ok(m) = moved.binary_search_by_key(&i, |&(j, _)| j) {
                let (c, h) = moved[m].1;
                *key = broad_build::key(tree, self.hi, c, h);
                first.get_or_insert(k);
            }
        }
        if let Some(first) = first {
            broad_build::undo(tree, &mut self.log, self.filed[first].2);
            for (o, key, start) in &mut self.filed[first..] {
                *start = self.log.changes.len();
                broad_build::insert_logged(tree, *o, *key, &mut self.log);
            }
        }
        self.stamp.copy_from_slice(stamp);
        self.generation = generation();
        true
    }

    /// The two probes at the ray probe's reach, and the bikes on their transforms and, those in the
    /// world, in the moving list after the probes: what [`world_unfiled`] gives them.
    fn place(&mut self, bikes: Option<&Bikes<'_>>, planes: &[Plane<'_>], centre: Vf, radius: F) {
        self.objects[RAY].radius = radius;
        for p in [RAY, SWEEP] {
            self.broad.places[p].centre = four(centre);
        }
        let Structure::List(list) = &mut self.broad.structures[1] else { unreachable!("the table's second structure is its list") };
        let block = &mut list.blocks.as_mut().expect("the moving list has its block")[0];
        for slot in &mut block[self.moving as usize..list.last as usize] {
            *slot = None;
        }
        let mut last = self.moving;
        for (j, p) in planes.iter().enumerate() {
            let o = self.planes + j;
            let (_, centre, radius) = Prop::posed(p.mesh, &p.at);
            self.objects[o].radius = radius;
            self.broad.places[o].centre = four(centre);
            if (last as usize) < block.len() {
                block[last as usize] = Some(o);
                last += 1;
            }
        }
        for (k, b) in bikes.map_or(&[][..], |b| b.bikes).iter().enumerate() {
            let mesh = bikes.map(|b| b.mesh).expect("bikes come with their mesh");
            let o = self.bikes + k;
            self.objects[o].radius = bike_sphere(mesh, &b.transform).1;
            self.broad.places[o].centre = bike_box(mesh, &b.transform).0;
            if b.in_world && (last as usize) < block.len() {
                block[last as usize] = Some(o);
                last += 1;
            }
        }
        list.last = last;
    }

    /// The broad phase's query state as a fresh world has it: what a query writes outside the
    /// structures (the stamp, the structure last queried, each placement's stamp) and inside them (the
    /// open query's cells, nodes and place in its walk), which a query's end does not all put back.
    fn rewind(&mut self) {
        self.broad.stamp = 0;
        self.broad.current = None;
        for p in &mut self.broad.places {
            p.stamp = 0;
        }
        for s in &mut self.broad.structures {
            match s {
                Structure::Tree(t) => {
                    t.flags = 2;
                    t.busy = 0;
                    t.cells = [0; 4];
                    t.list.clear();
                    (t.head, t.at, t.chunk, t.slot) = (None, None, None, 0);
                }
                Structure::List(l) => (l.flags, l.block, l.entry) = (0, 0, 0),
            }
        }
    }
}

/// [`wooded_of`]'s world, kept from an earlier search ([`Kept`]) — the same, or brought up to the
/// props that moved since — or built and kept now, with the probes at `centre`, `radius` and the bikes
/// where they are: `f` is handed its broad phase, which it may query, its collidables as built, and
/// its generation.
#[allow(clippy::too_many_arguments)]
fn kept_world<R>(
    props: &[Prop],
    trees: Option<&Trees>,
    bikes: Option<&Bikes<'_>>,
    planes: &[Plane<'_>],
    field: Option<([F; 2], [F; 2])>,
    centre: Vf,
    radius: F,
    toys: bool,
    f: impl FnOnce(&mut Broad, &[Collidable], u64) -> R,
) -> R {
    let count = bikes.map_or(0, |b| b.bikes.len());
    KEPT.with(|kept| {
        let (kept, stamp) = &mut *kept.borrow_mut();
        let at = stamp_world(stamp, props, trees, count, planes.len(), field, toys);
        let k = match kept.iter().position(|w| w.stamp == *stamp) {
            Some(k) => k,
            None => {
                let near = kept.iter().position(|w| {
                    w.props_at == at && w.stamp[..at.start] == stamp[..at.start] && w.stamp[at.end..] == stamp[at.end..]
                });
                match near {
                    Some(k) if kept[k].update(props, field, toys, stamp) => {
                        let (b, u, g) = BUILDS.get();
                        BUILDS.set((b, u + 1, g));
                        k
                    }
                    _ => {
                        kept.insert(0, Kept::build(props, trees, bikes, planes, field, centre, radius, toys, stamp, at));
                        kept.truncate(WORLDS_KEPT);
                        0
                    }
                }
            }
        };
        if k != 0 {
            let w = kept.remove(k);
            kept.insert(0, w);
        }
        let w = &mut kept[0];
        w.place(bikes, planes, centre, radius);
        w.rewind();
        f(&mut w.broad, &w.objects, w.generation)
    })
}

/// A collidable list to search a kept world's collidables in, `built` of generation `generation`:
/// one handed back by an earlier search ([`give_back`]), so nothing is allocated. One searched in
/// this very generation differs from `built` only where a search writes — the flags, the group a part
/// was last searched in, a probe's reach — so only those are put back; any other is copied whole.
fn lend(generation: u64, built: &[Collidable]) -> Vec<Collidable> {
    let (had, mut objects) = SPARE
        .with(|s| {
            let mut s = s.borrow_mut();
            let k = s.iter().position(|(g, _)| *g == generation).or(s.len().checked_sub(1));
            k.map(|k| s.swap_remove(k))
        })
        .unwrap_or_default();
    if had == generation && objects.len() == built.len() {
        for (o, b) in objects.iter_mut().zip(built) {
            (o.flags, o.group, o.radius) = (b.flags, b.group, b.radius);
        }
    } else {
        objects.truncate(built.len());
        let n = objects.len();
        objects.clone_from_slice(&built[..n]);
        objects.extend_from_slice(&built[n..]);
    }
    objects
}

fn give_back(generation: u64, objects: Vec<Collidable>) {
    SPARE.with(|s| {
        let mut s = s.borrow_mut();
        if s.len() < 4 {
            s.push((generation, objects));
        }
    });
}

/// The world the plane searches, for one frame: the track's props as collidables, the candidates
/// gathered around the ray probe, and the two probes' state.
pub struct Scene<'a> {
    props: &'a [Prop],
    trees: Option<&'a Trees>,
    /// The first plant's collidable and the first trunk's ([`world_wooded`]).
    groups: usize,
    trunks: usize,
    trunk_facets: std::collections::HashMap<usize, Vec<Facet>>,
    bikes: Option<Bikes<'a>>,
    bike_facets: Option<Vec<Facet>>,
    /// The ride's other planes (N22a1, [`Scene::among_planes`]): their first collidable, and the first
    /// of their collision records among the search's.
    planes: &'a [Plane<'a>],
    planes_at: usize,
    first_plane: usize,
    plane_facets: std::collections::HashMap<usize, Vec<Facet>>,
    /// What the bike's struck listener ([`bike_struck`]) kept on each of the ride's bikes this frame.
    pub struck: Vec<Option<u32>>,
    /// The bikes' chassis velocities when their collision records are in the search ([`Scene::owned`]).
    owned: Option<&'a [Vf]>,
    rays: &'a Rays,
    transform: Matrix,
    objects: Lent,
    nearby: Nearby,
    facets: std::collections::HashMap<usize, Vec<Facet>>,
    ray: ray_probe::Record,
    sweep: SweepProbe,
    /// A prop's ray probe's ([`Scene::probed`]): its body asleep, and what its listeners heard.
    asleep: bool,
    heard: Heard,
}

/// A prop's ray probe's listeners (S200), in the order the prop's init puts them on it: the prop's
/// **hit listener**, which wakes the body (its enable) when the struck one's category is in the game's
/// global mask ([`WAKE_MASK`]) — the body's object asks for it (`+0x58`) from its making on, and no key
/// turns it off — and the prop's collection's, which activates the collection whatever was struck.
pub const HIT_LISTENER: usize = 0;
pub const COLLECTION_LISTENER: usize = 1;

/// What a prop's ray probe's listeners heard ([`Scene::probed`]): `woke`, the hit listener woke the
/// asleep body (and so the probe's category list is every category from that hit on); `struck`, the
/// collection's listener heard a hit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Heard {
    pub woke: bool,
    pub struck: bool,
}

/// The plane's nearby `search` refilled ([`broad_phase::gather`], both kinds of structure) around
/// the ray probe placed at `node`, in the world [`world`] builds there: what the original's plane does
/// once a tick in its post listener, after the world step, from the pose the step left (S99). The
/// next frame's [`Scene`] searches with what it gathered.
pub fn refresh(props: Option<&Props>, field: Option<([F; 2], [F; 2])>, rays: &Rays, node: &Frame, search: &mut broad_phase::Search) {
    refresh_among(props, None, field, rays, node, search);
}

/// [`refresh`] in a world with the ride's bikes in it ([`world_among`]).
pub fn refresh_among(
    props: Option<&Props>,
    bikes: Option<&Bikes<'_>>,
    field: Option<([F; 2], [F; 2])>,
    rays: &Rays,
    node: &Frame,
    search: &mut broad_phase::Search,
) {
    refresh_wooded(props, None, bikes, field, rays, node, search);
}

/// [`refresh_among`] in a world with the ecosystem's plants in it ([`world_wooded`]).
pub fn refresh_wooded(
    props: Option<&Props>,
    trees: Option<&Trees>,
    bikes: Option<&Bikes<'_>>,
    field: Option<([F; 2], [F; 2])>,
    rays: &Rays,
    node: &Frame,
    search: &mut broad_phase::Search,
) {
    refresh_of(props, trees, bikes, &[], field, rays, node, search, false);
}

/// [`refresh_wooded`] in the plane's world, the path cars toys in it ([`world_flown`]).
pub fn refresh_flown(
    props: Option<&Props>,
    trees: Option<&Trees>,
    bikes: Option<&Bikes<'_>>,
    field: Option<([F; 2], [F; 2])>,
    rays: &Rays,
    node: &Frame,
    search: &mut broad_phase::Search,
) {
    refresh_of(props, trees, bikes, &[], field, rays, node, search, true);
}

/// [`refresh_flown`] with the ride's other planes in the world (N22a1, [`Plane`]).
#[allow(clippy::too_many_arguments)]
pub fn refresh_planes(
    props: Option<&Props>,
    trees: Option<&Trees>,
    bikes: Option<&Bikes<'_>>,
    planes: &[Plane<'_>],
    field: Option<([F; 2], [F; 2])>,
    rays: &Rays,
    node: &Frame,
    search: &mut broad_phase::Search,
) {
    refresh_of(props, trees, bikes, planes, field, rays, node, search, true);
}

/// The gather in the world [`wooded_of`] builds around the ray probe placed at `node`, kept between
/// searches ([`kept_world`]).
#[allow(clippy::too_many_arguments)]
fn refresh_of(
    props: Option<&Props>,
    trees: Option<&Trees>,
    bikes: Option<&Bikes<'_>>,
    planes: &[Plane<'_>],
    field: Option<([F; 2], [F; 2])>,
    rays: &Rays,
    node: &Frame,
    search: &mut broad_phase::Search,
    toys: bool,
) {
    let props = props.map_or(&[][..], |p| &p.props[..]);
    let (centre, radius) = reach(rays, &matrix(node));
    kept_world(props, trees, bikes, planes, field, centre, radius, toys, |broad, built, generation| {
        let mut objects = lend(generation, built);
        refill(&mut objects, broad, search);
        give_back(generation, objects);
    });
}

/// The ghost's category (S129, S189): what the put-back adds to the riding bike's body rays' category
/// list, and what the path cars' meshes ([`TOY`]) and the bikes ([`BIKE`]) have in theirs.
pub const GHOST: u32 = 0x40;

/// **The riding bike's nearby search refilled** (S189): [`refresh_flown`] around the body rays placed
/// at `node`, with no bikes in the world (the riders meet each other through their own lists), the
/// path cars toys in it, and the rays' category list `ignore` (exclude mode) in the gather's pair
/// filter: a ghost's ([`GHOST`]) drops every car and toy, and the list keeps none of them until the
/// bike next gathers it afresh.
pub fn refresh_ridden(
    props: Option<&Props>,
    trees: Option<&Trees>,
    field: Option<([F; 2], [F; 2])>,
    rays: &Rays,
    node: &Frame,
    search: &mut broad_phase::Search,
    ignore: &[u32],
) {
    let props = props.map_or(&[][..], |p| &p.props[..]);
    let (centre, radius) = reach(rays, &matrix(node));
    let (mut broad, mut objects) = world_flown(props, trees, None, field, centre, radius);
    objects[RAY].categories = Categories { mode: 1, list: ignore.to_vec() };
    refill(&mut objects, &mut broad, search);
}

/// **The world the plane's search takes at `node`** (V96): [`World::wooded_near`] around the ray
/// probe's reach, which a world of many tiles answers with the shapes near one square of it (MD28)
/// rather than every tile's, so a tick's two broad phases ([`Scene::flown`], [`refresh_flown`]) do
/// not grow with the world. The kept nearby list names collidables by their place in the set it was
/// gathered in, so a probe that has come to another square's set has it forgotten and gathered again
/// there. A world of one map hands its whole set at every square ([`World::near_set`] `None`), as
/// before, and nothing is forgotten.
///
/// [`World::wooded_near`]: crate::world::World::wooded_near
/// [`World::near_set`]: crate::world::World::near_set
pub fn near_world<'w>(
    world: &'w mut dyn crate::world::World,
    rays: &Rays,
    node: &Frame,
    search: &mut broad_phase::Search,
) -> (Option<Box<dyn crate::contact_search::Heightfield + 'w>>, Option<&'w Props>, Option<&'w Trees>) {
    let (at, _) = reach(rays, &matrix(node));
    let [x, y, z, _] = search.centre;
    if search.built && world.near_set(at) != world.near_set(Vf::new(x.0, y.0, z.0, 1.0)) {
        search.built = false;
        search.nearby.found.clear();
    }
    world.wooded_near(at)
}

/// The gather, after forgetting a list that names more collidables than this world has (it was
/// gathered in another one).
fn refill(objects: &mut [Collidable], broad: &mut Broad, search: &mut broad_phase::Search) {
    if search.nearby.found.iter().any(|c| objects.len() <= c.object) {
        search.built = false;
        search.nearby.found.clear();
    }
    broad_phase::gather(objects, broad, search, RAY, 3);
}

impl<'a> Scene<'a> {
    /// The world around the ray probe placed at `node` ([`world`]), searched with the plane's nearby
    /// `search` as last [`refresh`]ed. A list never gathered, or one that names more collidables than
    /// this world has, is gathered here first: the first frame's seed, which the original's spawn
    /// does somewhere not found.
    pub fn new(
        props: Option<&'a Props>,
        field: Option<([F; 2], [F; 2])>,
        rays: &'a Rays,
        node: &Frame,
        search: &mut broad_phase::Search,
    ) -> Scene<'a> {
        Scene::among(props, None, field, rays, node, search)
    }

    /// [`Scene::new`] in a world with the ride's bikes in it ([`world_among`]).
    pub fn among(
        props: Option<&'a Props>,
        bikes: Option<Bikes<'a>>,
        field: Option<([F; 2], [F; 2])>,
        rays: &'a Rays,
        node: &Frame,
        search: &mut broad_phase::Search,
    ) -> Scene<'a> {
        Scene::wooded(props, None, bikes, field, rays, node, search)
    }

    /// [`Scene::among`] in a world with the ecosystem's plants in it ([`world_wooded`]): the ray probe
    /// meets a plant's trunks as it meets a prop.
    pub fn wooded(
        props: Option<&'a Props>,
        trees: Option<&'a Trees>,
        bikes: Option<Bikes<'a>>,
        field: Option<([F; 2], [F; 2])>,
        rays: &'a Rays,
        node: &Frame,
        search: &mut broad_phase::Search,
    ) -> Scene<'a> {
        Scene::of(props, trees, bikes, &[], field, rays, node, search, false, None)
    }

    /// [`Scene::wooded`] in the plane's own world (S141, [`world_flown`]): the path cars are toys,
    /// and a probe's hit on one carries its car as the hit's mover of kind 2 — what the original's
    /// mesh collidable names at `+0x7c` — so the contact's relative velocity is against the car's
    /// ([`Search::movers`], [`crate::contact::resolve`]'s mover branch).
    pub fn flown(
        props: Option<&'a Props>,
        trees: Option<&'a Trees>,
        bikes: Option<Bikes<'a>>,
        field: Option<([F; 2], [F; 2])>,
        rays: &'a Rays,
        node: &Frame,
        search: &mut broad_phase::Search,
    ) -> Scene<'a> {
        Scene::of(props, trees, bikes, &[], field, rays, node, search, true, None)
    }

    /// [`Scene::flown`] with the ride's other planes in the world (N22a1, [`Plane`]): each one's hull
    /// a collidable after the bikes', category [`crate::flight::COLLIDABLE`], in the moving list after
    /// the probes and ahead of the bikes, as the original's node lists the planes' objects ahead of
    /// the riders'. A probe's hit on plane `j` names its collision record as the struck one's other,
    /// `first + j` among the records the search is handed ([`Scene::owned`]'s bikes' come first), so
    /// [`crate::contact_search::collide`] files the mirror on it.
    #[allow(clippy::too_many_arguments)]
    pub fn among_planes(
        props: Option<&'a Props>,
        trees: Option<&'a Trees>,
        bikes: Option<Bikes<'a>>,
        planes: &'a [Plane<'a>],
        first: usize,
        field: Option<([F; 2], [F; 2])>,
        rays: &'a Rays,
        node: &Frame,
        search: &mut broad_phase::Search,
    ) -> Scene<'a> {
        let mut scene = Scene::of(props, trees, bikes, planes, field, rays, node, search, true, None);
        scene.first_plane = first;
        scene
    }

    /// **A prop's search** (S200): [`Scene::flown`] for a prop's ray probe, which carries the prop's
    /// category list `categories` (the gather's and the search's pair filter read it) and the prop's two
    /// listeners ([`HIT_LISTENER`], [`COLLECTION_LISTENER`]). `asleep` is whether the prop's body is
    /// asleep: the hit listener's wake is the body's enable, which does nothing to an awake one. What
    /// they heard is [`Scene::heard`].
    #[allow(clippy::too_many_arguments)]
    pub fn probed(
        props: Option<&'a Props>,
        trees: Option<&'a Trees>,
        field: Option<([F; 2], [F; 2])>,
        rays: &'a Rays,
        node: &Frame,
        search: &mut broad_phase::Search,
        categories: &Categories,
        asleep: bool,
    ) -> Scene<'a> {
        let mut scene = Scene::of(props, trees, None, &[], field, rays, node, search, true, Some(categories));
        scene.asleep = asleep;
        scene
    }

    /// What a prop's ray probe's listeners heard in the searches so far ([`Scene::probed`]).
    pub fn heard(&self) -> Heard {
        self.heard
    }

    #[allow(clippy::too_many_arguments)]
    fn of(
        props: Option<&'a Props>,
        trees: Option<&'a Trees>,
        bikes: Option<Bikes<'a>>,
        planes: &'a [Plane<'a>],
        field: Option<([F; 2], [F; 2])>,
        rays: &'a Rays,
        node: &Frame,
        search: &mut broad_phase::Search,
        toys: bool,
        categories: Option<&Categories>,
    ) -> Scene<'a> {
        let props = props.map_or(&[][..], |p| &p.props[..]);
        let transform = matrix(node);
        let (centre, radius) = reach(rays, &transform);
        let objects = kept_world(props, trees, bikes.as_ref(), planes, field, centre, radius, toys, |broad, built, generation| {
            let mut objects = lend(generation, built);
            if let Some(categories) = categories {
                objects[RAY].categories = categories.clone();
                objects[RAY].listeners = vec![HIT_LISTENER, COLLECTION_LISTENER];
            }
            if !search.built || search.nearby.found.iter().any(|c| objects.len() <= c.object) {
                refill(&mut objects, broad, search);
            }
            Lent { generation, objects }
        });
        let ray = ray_probe::Record {
            listed: true,
            capacity: rays.segments.len(),
            one_per_ray: true,
            ..ray_probe::Record::default()
        };
        let sweep = SweepProbe::default();
        let nearby = search.nearby.clone();
        let struck = vec![None; bikes.map_or(0, |b| b.bikes.len())];
        let planes_at = FIRST + props.len() + bikes.map_or(0, |b| b.bikes.len());
        let groups = planes_at + planes.len();
        let trunks = groups + trees.map_or(0, |t| t.groups().len());
        Scene {
            props,
            trees,
            groups,
            trunks,
            trunk_facets: std::collections::HashMap::new(),
            bikes,
            bike_facets: None,
            planes,
            planes_at,
            first_plane: 1,
            plane_facets: std::collections::HashMap::new(),
            struck,
            owned: None,
            rays,
            transform,
            objects,
            nearby,
            facets: std::collections::HashMap::new(),
            ray,
            sweep,
            asleep: false,
            heard: Heard::default(),
        }
    }

    /// The ray probe's record as the last [`Search::rays`] left it.
    pub fn ray_record(&self) -> &ray_probe::Record {
        &self.ray
    }

    /// The world's trunk `j`, when `other` is one of [`world_wooded`]'s trunks.
    fn trunk(&self, other: usize) -> Option<usize> {
        let j = other.checked_sub(self.trunks)?;
        self.trees.is_some_and(|t| j < t.props.props.len()).then_some(j)
    }

    /// **The bikes' own collision objects** (N22b): a riding bike's frame collidable is its collision
    /// record's (its owner, `+0x80`, is the record), so a hit on bike `k` names that record as the
    /// struck one's other, `1 + k` among the records the search is handed after the plane's, and
    /// [`crate::contact_search::collide`] files the mirror on it. `velocities`, each bike's chassis
    /// velocity in [`Bike`]s' order, is what the sweep reads of a struck bike. Without, a struck bike
    /// is only itself (S95's world, where the port has no record for it).
    pub fn owned(mut self, velocities: &'a [Vf]) -> Scene<'a> {
        self.owned = Some(velocities);
        self
    }

    /// The ride's other plane `j`, when `other` is one of [`Scene::among_planes`]' hulls.
    fn plane(&self, other: usize) -> Option<usize> {
        let j = other.checked_sub(self.planes_at)?;
        (j < self.planes.len()).then_some(j)
    }

    /// The ride's bike `k`, when `other` is one of [`world_among`]'s bikes.
    fn bike(&self, other: usize) -> Option<usize> {
        let k = other.checked_sub(FIRST + self.props.len())?;
        self.bikes.is_some_and(|b| k < b.bikes.len()).then_some(k)
    }

    /// What the plane's record keeps as struck for collidable `other` ([`BIKES`]).
    fn id(&self, other: usize) -> usize {
        if let Some(j) = self.trunk(other) {
            return TREES + j;
        }
        if let Some(j) = self.plane(other) {
            return PLANES + j;
        }
        self.bike(other).map_or(other, |k| BIKES + k)
    }

}

/// A scene's collidables, handed back when it is done for the next search to copy its world into
/// ([`lend`]), with the generation of the world they were copied from.
struct Lent {
    generation: u64,
    objects: Vec<Collidable>,
}

impl Drop for Lent {
    fn drop(&mut self) {
        // A prop's ray probe ([`Scene::probed`]) back to the world's own, which [`lend`] does not put
        // back.
        if let Some(ray) = self.objects.get_mut(RAY) {
            ray.categories = any_category();
            ray.listeners.clear();
        }
        give_back(self.generation, std::mem::take(&mut self.objects));
    }
}

impl Scene<'_> {
    /// A probe's narrow phase against the ride's bike `k`, collidable `other`: its mesh on its
    /// chassis' transform, as a prop's on its placement (`bikef.col`'s own transform is the identity).
    fn pair_bike(&mut self, probe: usize, other: usize, k: usize) -> bool {
        let Some(bikes) = self.bikes else { return false };
        let mesh = bikes.mesh;
        let t = bikes.bikes[k].transform;
        if probe == RAY {
            let facets = self.bike_facets.get_or_insert_with(|| facets(mesh));
            let target = ray_probe::Mesh {
                id: other as u32,
                transform: t.0,
                root: 0,
                nodes: &mesh.nodes,
                facets,
                vertices: &mesh.vertices,
            };
            let rays = self.rays;
            let probe = RayProbe { transform: self.transform, root: rays.root, nodes: &rays.nodes, segments: &rays.segments };
            ray_probe::pair(&probe, &ray_probe::Shape::Mesh(target), &mut self.ray)
        } else {
            let (centre, radius) = bike_sphere(mesh, &t);
            let target = sweep_probe::Target {
                nearby: true,
                centre,
                radius,
                shape: sweep_probe::Shape::Mesh(sweep_probe::Placed { transform: t.0, mesh: MeshRef::of(mesh), object: other }),
            };
            sweep_probe::pair(&mut self.sweep, Some(&target))
        }
    }
}

impl Scene<'_> {
    /// A probe's narrow phase against the ride's other plane `j`, collidable `other`: its hull where
    /// its node stands it, as a knockable prop's mesh on its node ([`Prop::posed`]).
    fn pair_plane(&mut self, probe: usize, other: usize, j: usize) -> bool {
        let p = self.planes[j];
        let (to_world, centre, radius) = Prop::posed(p.mesh, &p.at);
        if probe == RAY {
            let facets = self.plane_facets.entry(j).or_insert_with(|| facets(p.mesh));
            let target = ray_probe::Mesh {
                id: other as u32,
                transform: to_world.0,
                root: 0,
                nodes: &p.mesh.nodes,
                facets,
                vertices: &p.mesh.vertices,
            };
            let rays = self.rays;
            let probe = RayProbe { transform: self.transform, root: rays.root, nodes: &rays.nodes, segments: &rays.segments };
            ray_probe::pair(&probe, &ray_probe::Shape::Mesh(target), &mut self.ray)
        } else {
            let target = sweep_probe::Target {
                nearby: true,
                centre,
                radius,
                shape: sweep_probe::Shape::Mesh(sweep_probe::Placed { transform: to_world.0, mesh: MeshRef::of(p.mesh), object: other }),
            };
            sweep_probe::pair(&mut self.sweep, Some(&target))
        }
    }
}

impl Scene<'_> {
    /// A probe's narrow phase against trunk `j` of the world's trees, collidable `other`: a prop's.
    fn pair_trunk(&mut self, probe: usize, other: usize, j: usize) -> bool {
        let Some(trees) = self.trees else { return false };
        let p = &trees.props.props[j];
        if probe == RAY {
            let facets = self.trunk_facets.entry(j).or_insert_with(|| facets(&p.mesh));
            let mesh = ray_probe::Mesh {
                id: other as u32,
                transform: placed(p),
                root: 0,
                nodes: &p.mesh.nodes,
                facets,
                vertices: &p.mesh.vertices,
            };
            let rays = self.rays;
            let probe = RayProbe { transform: self.transform, root: rays.root, nodes: &rays.nodes, segments: &rays.segments };
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
        if let Some(j) = self.trunk(other) {
            return self.pair_trunk(probe, other, j);
        }
        if let Some(k) = self.bike(other) {
            return self.pair_bike(probe, other, k);
        }
        if let Some(j) = self.plane(other) {
            return self.pair_plane(probe, other, j);
        }
        let i = other - FIRST;
        let props = self.props;
        let p = &props[i];
        if probe == RAY {
            let rays = self.rays;
            let transform = self.transform;
            // Prop `i`'s triangles in the ray probe's form, packed once a frame.
            let facets = self.facets.entry(i).or_insert_with(|| facets(&p.mesh));
            let mesh = ray_probe::Mesh {
                id: other as u32,
                transform: placed(p),
                root: 0,
                nodes: &p.mesh.nodes,
                facets,
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

    /// A plant's parts: one per solid of its species, as the original's part count reads them.
    fn parts(&mut self, _objects: &mut [Collidable], group: usize) -> usize {
        let trees = self.trees.expect("a placed group in a world without trees");
        trees.groups()[group - self.groups].parts.len()
    }

    /// Part `i` of a plant: its trunk, already where the plant stands. A sphere's ray test writes no
    /// contact in the original (its narrow phase returns only whether a ray passed through it), and
    /// nothing the port reads off a search is the hit bit alone, so a sphere is no part here.
    fn part(&mut self, _objects: &mut [Collidable], group: usize, i: usize) -> Option<usize> {
        let trees = self.trees.expect("a placed group in a world without trees");
        trees.groups()[group - self.groups].parts[i].map(|j| self.trunks + j)
    }

    /// The ray probe's listeners. The plane's two are not here: the hit listener's wake is a no-op on
    /// the plane's awake body, and the sound listener is not simulation. A prop's two
    /// ([`Scene::probed`]): the hit listener wakes an asleep body when `other`'s category is in the
    /// global mask, and the wake's enable puts the probe's category list back to every category,
    /// which the rest of this search's pair filter reads; the collection's listener hears every hit.
    fn heard_hit(&mut self, objects: &mut [Collidable], listener: usize, probe: usize, other: usize) {
        match listener {
            HIT_LISTENER if self.asleep && objects[other].category & WAKE_MASK != 0 => {
                self.asleep = false;
                self.heard.woke = true;
                objects[probe].categories = any_category();
            }
            COLLECTION_LISTENER => self.heard.struck = true,
            _ => {}
        }
    }

    /// A bike's one listener, the struck listener: what struck it is the probe's collidable.
    fn heard_struck(&mut self, objects: &mut [Collidable], listener: usize, struck: usize, probe: usize) {
        if self.bike(struck).is_some() {
            bike_struck(&mut self.struck[listener], objects[probe].category);
        }
    }

    fn on_hit(&mut self, _objects: &mut [Collidable], _callback: usize, _probe: usize, _other: usize) {}

    fn on_struck(&mut self, _objects: &mut [Collidable], _callback: usize, _struck: usize, _probe: usize) {}
}

/// What a prop is to the contact search: itself, with no collision object and no mover.
fn struck(object: usize) -> Struck {
    Struck { object, other: None, mover: None }
}

impl Scene<'_> {
    /// What `object` is to the contact search (S145): a prop that moves this frame (a path car,
    /// [`Prop::moving`]) is a mesh collidable with no collision object whose owner is the car, a
    /// mover of kind 2, so it is struck with its mover — by the prop's index, the order
    /// [`Search::movers`] lists them in. Anything else is only itself ([`struck`]).
    fn struck_of(&self, object: usize) -> Struck {
        if let Some(j) = object.checked_sub(PLANES).filter(|&j| j < self.planes.len()) {
            return Struck { other: Some(self.first_plane + j), ..struck(object) };
        }
        if let Some(k) = object.checked_sub(BIKES).filter(|&k| object < TREES && self.owned.is_some_and(|v| k < v.len())) {
            return Struck { other: Some(1 + k), ..struck(object) };
        }
        let mover = self.prop(object).filter(|&i| self.props[i].moving.is_some());
        Struck { mover: mover.map(|i| (i, crate::contact_search::MOVER_KIND)), ..struck(object) }
    }

    /// Prop `i`, when `object` is one of the props' collidables.
    fn prop(&self, object: usize) -> Option<usize> {
        object.checked_sub(FIRST).filter(|&i| i < self.props.len())
    }

    /// The prop `object` is, when it is one of the props' collidables (S189: what a riding bike's
    /// wheel probe struck, for its car and its material).
    pub fn prop_at(&self, object: usize) -> Option<&Prop> {
        self.prop(object).map(|i| &self.props[i])
    }
}

impl Search for Scene<'_> {
    /// The ray probe's search. The plane's collision object always has a nearby search; without one
    /// the original asks the broad phase directly, which here is the same list.
    fn rays(&mut self, _nearby: bool) -> Vec<Hit> {
        let mut objects = std::mem::take(&mut self.objects.objects);
        let nearby = std::mem::take(&mut self.nearby);
        world_search::search_nearby(&mut objects, RAY, &nearby, self);
        (self.objects.objects, self.nearby) = (objects, nearby);
        self.ray
            .contacts
            .iter()
            .map(|c| Hit { from: c.from, to: c.to, point: c.point, normal: c.normal, struck: self.struck_of(self.id(c.object as usize)) })
            .collect()
    }

    fn sweep(&mut self, from: Vf, to: Vf, _nearby: bool) -> Option<Swept> {
        self.sweep.segments = vec![sweep_probe::Segment { ends: [from, to], nodes: [None, None] }];
        let mut objects = std::mem::take(&mut self.objects.objects);
        let nearby = std::mem::take(&mut self.nearby);
        let hit = world_search::search_nearby(&mut objects, SWEEP, &nearby, self);
        (self.objects.objects, self.nearby) = (objects, nearby);
        let a = &self.sweep.answer;
        hit.then(|| Swept { push: a.record.nearest_normal, point: a.record.nearest_point, struck: self.struck_of(self.id(a.struck[0].unwrap_or(SWEEP))) })
    }

    fn moved(&mut self, node: &Frame) {
        self.transform = matrix(node);
    }

    /// A struck prop that moves answers its car's velocity, the mover's `vt+0xb8` (S145), and a struck
    /// bike with its record in the search its chassis' (N22b, [`Scene::owned`]); the rest stand still.
    fn velocity(&self, struck: &Struck) -> Vf {
        if let Some(j) = struck.object.checked_sub(PLANES).filter(|&j| j < self.planes.len()) {
            return self.planes[j].velocity;
        }
        if let (Some(k), Some(v)) = (struck.other, self.owned) {
            return v[k - 1];
        }
        match struck.mover.and_then(|(i, _)| self.props[i].moving) {
            Some(m) => m.velocity,
            None => Vf::ZERO,
        }
    }

    fn movers(&self) -> Vec<crate::contact::Mover> {
        movers(self.props)
    }
}

/// **The props as movers** (S145): one a prop, by its index, while any prop moves ([`Prop::moving`],
/// [`crate::traffic::Moving::mover`]); empty while none does. A still prop's is never named, so it is
/// left at the default.
pub fn movers(props: &[Prop]) -> Vec<crate::contact::Mover> {
    if props.iter().all(|p| p.moving.is_none()) {
        return Vec::new();
    }
    props.iter().map(|p| p.moving.map(|m| m.mover()).unwrap_or_default()).collect()
}

/// The category of what [`Scene`] reports as struck: another plane's ([`PLANES`]) is
/// [`crate::flight::COLLIDABLE`], a trunk's ([`TREES`]) is [`TREE`], a bike's ([`BIKES`]) is
/// [`BIKE`], every prop's is scenery.
pub fn category(object: usize) -> u32 {
    if object >= PLANES {
        crate::flight::COLLIDABLE
    } else if object >= TREES {
        TREE
    } else if object >= BIKES {
        BIKE
    } else {
        SCENERY
    }
}
