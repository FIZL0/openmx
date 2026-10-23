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
    const EDGE: f32 = 16_777_216.0;
    if let Some((lo, hi)) = field
        && lo.iter().chain(&hi).all(|&v| -EDGE < v && v < EDGE)
    {
        return (lo, hi);
    }
    let (mut lo, mut hi) = ([F::ZERO; 2], [F::ONE; 2]);
    for p in props {
        let (c, h) = prop_box(p);
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
    for (i, p) in props.iter().enumerate() {
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
        let key = broad_build::key(&tree, hi, c, h);
        broad_build::insert(&mut tree, FIRST + i, key);
    }
    let mut block = vec![None; MOVING_BLOCK as usize];
    (block[RAY], block[SWEEP]) = (Some(RAY), Some(SWEEP));
    let mut last = 2;
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
    let list = Blocks { size: MOVING_BLOCK, last, blocks: Some(vec![block]), ..Blocks::default() };
    let broad = Broad {
        spaces: vec![Space { next: None, tables: 0 }],
        tables: vec![Tables { own: Pair { fixed: Some(0), moving: Some(1) }, subs: Vec::new() }],
        structures: vec![Structure::Tree(tree), Structure::List(list)],
        cell_size,
        places,
        ..Broad::default()
    };
    (broad, objects)
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
    let Some(trees) = trees else { return world_among(props, bikes, field, centre, radius) };
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
    let (mut broad, mut objects) = world_among(props, bikes, field, centre, radius);
    let (_, hi) = tree_box(props, field);
    let Structure::Tree(tree) = &mut broad.structures[0] else { unreachable!("the table's first structure is its tree") };
    for g in trees.groups() {
        let (c, h) = g.bounds(trees.cell());
        let o = objects.len();
        objects.push(Collidable { class: Class::Group, nearby: true, ..Collidable::default() });
        broad.places.push(Placement { centre: c, cell: [g.cell[0], g.cell[2]], model: g.model, blend: g.blend, ..Placement::default() });
        let key = broad_build::key(tree, hi, c, h);
        broad_build::insert(tree, o, key);
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
    (broad, objects)
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
    /// What the bike's struck listener ([`bike_struck`]) kept on each of the ride's bikes this frame.
    pub struck: Vec<Option<u32>>,
    rays: &'a Rays,
    transform: Matrix,
    objects: Vec<Collidable>,
    nearby: Nearby,
    facets: Vec<Option<Vec<Facet>>>,
    ray: ray_probe::Record,
    sweep: SweepProbe,
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
    let props = props.map_or(&[][..], |p| &p.props[..]);
    let (centre, radius) = reach(rays, &matrix(node));
    let (mut broad, mut objects) = world_wooded(props, trees, bikes, field, centre, radius);
    refill(&mut objects, &mut broad, search);
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
        let props = props.map_or(&[][..], |p| &p.props[..]);
        let transform = matrix(node);
        let (centre, radius) = reach(rays, &transform);
        let (mut broad, mut objects) = world_wooded(props, trees, bikes.as_ref(), field, centre, radius);
        if !search.built || search.nearby.found.iter().any(|c| objects.len() <= c.object) {
            refill(&mut objects, &mut broad, search);
        }
        let ray = ray_probe::Record {
            listed: true,
            capacity: rays.segments.len(),
            one_per_ray: true,
            ..ray_probe::Record::default()
        };
        let sweep = SweepProbe::default();
        let nearby = search.nearby.clone();
        let struck = vec![None; bikes.map_or(0, |b| b.bikes.len())];
        let groups = FIRST + props.len() + bikes.map_or(0, |b| b.bikes.len());
        let trunks = groups + trees.map_or(0, |t| t.groups().len());
        Scene {
            props,
            trees,
            groups,
            trunks,
            trunk_facets: std::collections::HashMap::new(),
            bikes,
            bike_facets: None,
            struck,
            rays,
            transform,
            objects,
            nearby,
            facets: vec![None; props.len()],
            ray,
            sweep,
        }
    }

    /// The world's trunk `j`, when `other` is one of [`world_wooded`]'s trunks.
    fn trunk(&self, other: usize) -> Option<usize> {
        let j = other.checked_sub(self.trunks)?;
        self.trees.is_some_and(|t| j < t.props.props.len()).then_some(j)
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
        self.bike(other).map_or(other, |k| BIKES + k)
    }

    /// Prop `i`'s triangles in the ray probe's form, packed once a frame.
    fn facets_of(&mut self, i: usize) -> &[Facet] {
        let props = self.props;
        self.facets[i].get_or_insert_with(|| facets(&props[i].mesh))
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
            let facets = self.bike_facets.get_or_insert_with(|| facets(mesh)).clone();
            let target = ray_probe::Mesh {
                id: other as u32,
                transform: t.0,
                root: 0,
                nodes: &mesh.nodes,
                facets: &facets,
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

    /// The ray probe's two listeners: the hit listener's wake is a no-op on the plane's awake body,
    /// and the sound listener is not simulation.
    fn heard_hit(&mut self, _objects: &mut [Collidable], _listener: usize, _probe: usize, _other: usize) {}

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
            .map(|c| Hit { from: c.from, to: c.to, point: c.point, normal: c.normal, struck: struck(self.id(c.object as usize)) })
            .collect()
    }

    fn sweep(&mut self, from: Vf, to: Vf, _nearby: bool) -> Option<Swept> {
        self.sweep.segments = vec![sweep_probe::Segment { ends: [from, to], nodes: [None, None] }];
        let mut objects = std::mem::take(&mut self.objects);
        let nearby = std::mem::take(&mut self.nearby);
        let hit = world_search::search_nearby(&mut objects, SWEEP, &nearby, self);
        (self.objects, self.nearby) = (objects, nearby);
        let a = &self.sweep.answer;
        hit.then(|| Swept { push: a.record.nearest_normal, point: a.record.nearest_point, struck: struck(self.id(a.struck[0].unwrap_or(SWEEP))) })
    }

    fn moved(&mut self, node: &Frame) {
        self.transform = matrix(node);
    }

    fn velocity(&self, _struck: &Struck) -> Vf {
        Vf::ZERO
    }
}

/// The category of what [`Scene`] reports as struck: a trunk's ([`TREES`]) is [`TREE`], a bike's
/// ([`BIKES`]) is [`BIKE`], every prop's is scenery.
pub fn category(object: usize) -> u32 {
    if object >= TREES {
        TREE
    } else if object >= BIKES {
        BIKE
    } else {
        SCENERY
    }
}
