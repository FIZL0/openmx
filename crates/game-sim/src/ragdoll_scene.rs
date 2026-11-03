//! **The bones against the wreck bike** (S105): the scene test a bone's sphere makes against the wreck
//! bike's collision mesh (`wreckbikef.col`), and the [`Scene`] that answers a ride's ragdoll with it.
//!
//! The original stands each bone's sphere collider at the bone and pairs it with what its search
//! gathered; against a mesh collidable the pair walks the mesh's box tree in the collidable's frame,
//! takes the first triangle under the sphere whose plane it reaches, keeps the nearest of them in the
//! sphere's record, and hands the hit back through the frame. Every sphere test of S62's five wrecks
//! that hit anything hit this one mesh (the bike's own body object), so a ride's scene is the bike.
//!
//! The bike is gathered for the bones as the original gathers it: one search for all of them, once
//! a frame in the wreck, round the first collision's sphere ([`BikeSearch`]), in the track's tree
//! where the bike stands ([`Cells`]), and only once the bones are off the bike (until then every
//! sphere ignores it). A sphere is tested against the mesh only when the search reaches it.
//!
//! The same search lists the path cars (S142), every one of them every frame, each re-measured; a
//! sphere is tested against a car's mesh too when the car is within its reach, on the same record,
//! and the push-out measures the bone against the car's motion ([`PathCar`]).
//!
//! The search lists the track's static props too (S165): the scene's collision meshes stand in the
//! same tree as the bike ([`crate::flight_search::prop_box`]'s keys), and a regather lists each one
//! whose cells the query reaches and whose reach is inside the bound, in the walk's order with the
//! bike ([`Statics`]); a sphere is tested against a listed prop's mesh, on the same record, when its
//! distance is within the sphere's radius, and a prop does not move ([`StillProp`]).
//!
//! And the ecosystem's plants (S206): the search lists placed groups too, filed in the tree after
//! the props and measured on the ground plane ([`group_distance`]), and a sphere is handed a listed
//! plant's trunks at its place in the list, unfiltered ([`StillProp::near_wooded`]). The rest of what
//! the track's tree holds meets no bone, though the original lists it: the trigger boxes (category
//! `0x100`, a shape the sphere's pair has no test for), the other riders' wreck bikes, and the
//! hierarchy meshes the port's world lacks.
//!
//! And the knockable props while they are awake (S240): the moving structure holds their meshes, the
//! search lists every one, a sphere is paired with each one within its reach, the push-out measures
//! the bone against the prop's body, and a prop a sphere's record took is woken ([`KnockedProp`]).
//! At rest (F7) they stand in the track's tree, filed after the plants, and a regather lists each
//! one the query's cells reach whatever its distance; met as an awake one is, the strike waking it
//! ([`Statics::knock_keys`], [`KnockedProp::resting`]).
//!
//! **The sweep** (S166): each bone's collision also has a segment probe, one segment from where the
//! bone was `1/15` s ago to where it is, run through the same search list the spheres are
//! ([`sweep_through`]). But the probe's pair filter *includes* only the track's scenery
//! ([`class::SCENERY`]), and neither the wreck bike nor a path car is of it: the original's bones
//! never sweep against either, and a bone fast enough to cross a wagon's wall between two frames goes
//! through it there too. The scene still offers both, with their classes, so that the filter is
//! what turns them away; S165's props are in the bones' scene but not yet listed to the sweep (S209).
//!
//! What is not here: the hierarchy class of collision file, which the wreck bike is not; and the bike
//! leaving the broad phase, which the put-back does (S108) and the caller says
//! ([`crate::ragdoll_ride::Down::step_posed_with`]).

#![deny(clippy::float_arithmetic)]

use crate::biplane::Body;
use crate::broad_build;
use crate::broad_phase::{Broad, Pair, Placement, Quadtree, Query, Space, Structure, Tables};
use crate::collide::MeshRef;
use crate::contact::{Collider, Node, Other, Side, Weighed};
use crate::math::Mat4;
use crate::ragdoll_contact::{Against, Scene, moving};
use crate::sweep_probe::{self, Placed, Segment, Shape, SweepProbe, Target};
use crate::track::{self, Child};
use ps2_float::{Acc, Dest, F, Vf, fpu, vu};

/// **The wreck bike's collision mesh**: its box tree, its triangles' normals as the loader packs
/// them, and its vertices.
#[derive(Clone, Debug)]
pub struct Bike {
    pub mesh: track::Mesh,
    /// Each triangle's normal in the loader's 16-bit form, which the test unpacks again.
    pub normals: Vec<[i16; 3]>,
}

/// One sphere's hit on the mesh, in the bike's frame and in the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Touch {
    /// The sphere's centre's distance from the triangle's plane.
    pub distance: F,
    /// The point on the plane under the centre, and the plane's normal (unit, `w` one), in the
    /// mesh's frame: what the sphere's record holds.
    pub local: (Vf, Vf),
    /// The same two through the frame: what the push-out is handed.
    pub world: (Vf, Vf),
}

impl Bike {
    /// The mesh out of its collision file. `None` when the file does not read as a mesh or has no
    /// nodes.
    pub fn parse(col: &[u8]) -> Option<Bike> {
        let mesh = track::Mesh::parse(col).ok()?;
        mesh.nodes.first()?;
        let normals = mesh.triangles.iter().map(|t| crate::flight_search::pack_normal(t.normal)).collect();
        Some(Bike { mesh, normals })
    }

    /// **A sphere at `centre` (world) of `radius` against the mesh in `frame`**: the original's
    /// sphere-on-mesh pair with a fresh record (one contact at most). The squared radius is the
    /// collider's own, `radius * radius` through the FPU.
    pub fn touch(&self, frame: &Node, centre: Vf, radius: F) -> Option<Touch> {
        touch_mesh(&self.mesh, Normals::Packed(&self.normals), frame, centre, radius, None)
    }
}

/// A mesh's triangle normals as the sphere test reads them: packed once with the mesh ([`Bike`]),
/// or packed as each leaf is reached ([`PathCar`], whose mesh is the world's). Both are
/// [`crate::flight_search::pack_normal`] of the triangle's normal.
#[derive(Clone, Copy)]
enum Normals<'a> {
    Packed(&'a [[i16; 3]]),
    Unpacked,
}

/// **A sphere against a mesh in `frame`, on a record that may already hold a hit** at `held` (the
/// distance of the hit an earlier candidate of the same search left in it, S142). The record keeps
/// one contact: a leaf takes it only when the record is empty or its hit is strictly nearer than the
/// one held. `Some` when this mesh took it, with this mesh's hit; `None` when it missed, or touched
/// no nearer than what was held, which then stands.
fn touch_mesh(mesh: &track::Mesh, normals: Normals<'_>, frame: &Node, centre: Vf, radius: F, held: Option<F>) -> Option<Touch> {
    let r2 = F(fpu::mul(radius.0, radius.0));
    let none = Vf::splat(0.0);
    let mut best = held.map(|d| (d, none, none));
    let mut took = false;
    descend(mesh, normals, 0, frame, centre, radius, r2, &mut best, &mut took);
    let (distance, normal, point) = best.filter(|_| took)?;
    Some(Touch { distance, local: (point, normal), world: (frame.place(point), frame.rotate(normal)) })
}

/// The tree walk from `word`: a node's box is tested, then its first child, and its second only
/// when the first answers no. A leaf answers yes when the sphere touches its triangle and the
/// record took it, or was full; no when the hit was no nearer than the one the record holds.
#[allow(clippy::too_many_arguments)]
fn descend(
    mesh: &track::Mesh,
    normals: Normals<'_>,
    word: u16,
    frame: &Node,
    centre: Vf,
    r: F,
    r2: F,
    best: &mut Option<(F, Vf, Vf)>,
    took: &mut bool,
) -> bool {
    let tris = MeshRef::of(mesh);
    match child(word) {
        Child::Node(i) => {
            let Some(node) = mesh.nodes.get(usize::from(i)) else { return false };
            if !box_reached(node, frame, centre, r, r2) {
                return false;
            }
            descend(mesh, normals, node.children[0], frame, centre, r, r2, best, took)
                || descend(mesh, normals, node.children[1], frame, centre, r, r2, best, took)
        }
        Child::Leaf(i) => {
            let i = usize::from(i);
            let Some(t) = tris.triangles.get(i) else { return false };
            let packed = match normals {
                Normals::Packed(n) => match n.get(i) {
                    Some(&p) => p,
                    None => return false,
                },
                Normals::Unpacked => crate::flight_search::pack_normal(mesh.triangles[i].normal),
            };
            let c = frame.unplace(centre);
            let n = unpack(packed);
            let [a, b, d] = t.vertices.map(|v| tris.vertex(v));
            let dist = dot(n, minus(c, a));
            if dist.0 < 0.0 || !(dist.0 <= r.0) {
                return false;
            }
            let p = minus(c, n.merge(Dest::XYZ, n.mul(Vf::splat(dist.0))));
            let hit = inside(p, n, [a, b, d])
                || edge(c, a, b).abs().0 < r.0
                || edge(c, b, d).abs().0 < r.0
                || edge(c, d, a).abs().0 < r.0;
            if !hit {
                return false;
            }
            match best {
                Some((held, ..)) if !(dist.0 < held.0) => false,
                _ => {
                    *best = Some((dist, unit(n), p));
                    *took = true;
                    true
                }
            }
        }
    }
}

/// What a child word points at: the top bit marks a triangle.
fn child(word: u16) -> Child {
    if word & 0x8000 == 0 { Child::Node(word) } else { Child::Leaf(word & 0x7fff) }
}

/// **A tree node's box against the sphere**: the centre into the mesh's frame and off the box's
/// centre, every lane's magnitude, and out when any axis is past the half extent by more than the
/// radius; else the part past the box, squared, inside the squared radius.
fn box_reached(node: &track::Node, frame: &Node, centre: Vf, r: F, r2: F) -> bool {
    let [cx, cy, cz] = node.centre.map(|c| c.0);
    let [hx, hy, hz] = node.half_extent.map(|h| h.0);
    let local = frame.unplace(centre);
    let a = bits_abs(minus(local, Vf::new(cx, cy, cz, 0.0)));
    let half = Vf::new(hx, hy, hz, 0.0);
    let past = minus(a, half);
    let past = past.merge(Dest::XYZ, past.sub(Vf::splat(r.0)));
    if !(past.x() <= 0.0) || !(past.y() <= 0.0) || !(past.z() <= 0.0) {
        return false;
    }
    let out = minus(a, a.mini(half));
    let s = Acc::mula(out.x(), out.x()).madda(out.y(), out.y()).madd(out.z(), out.z());
    !(r2.0 <= s.0)
}

/// **Whether `p`, on the plane, is inside the triangle**: the corner tests round `b` and then round
/// `c`, each `n · (e × q)` and `n · (q × e')` not negative.
fn inside(p: Vf, n: Vf, [a, b, c]: [Vf; 3]) -> bool {
    let n = Vf::new(n.x(), n.y(), n.z(), 0.0);
    let round = |at: Vf, e1: Vf, e2: Vf| {
        let (e1, e2, q) = (minus(e1, at), minus(e2, at), minus(p, at));
        !(dot(n, cross(e1, q)).0 < 0.0) && !(dot(n, cross(q, e2)).0 < 0.0)
    };
    round(b, c, a) && round(c, a, b)
}

/// **The distance from `p` to the edge from `b` to `a`**, through the FPU: the far end's square is
/// the original's `|w|² + 2 w·e + |e|²`, with the sign of the middle term as it is written.
fn edge(p: Vf, a: Vf, b: Vf) -> F {
    let e = minus(a, b);
    let w = minus(p, b);
    let along = dot(w, e).0;
    let ww = dot(w, w).0;
    if along <= 0.0 {
        return F(fpu::sqrt(ww));
    }
    let ee = dot(e, e).0;
    if !(along < ee) {
        return F(fpu::sqrt(fpu::add(ee, fpu::add(ww, fpu::mul(2.0, along)))));
    }
    let t = fpu::div(along, ee);
    let tt = fpu::mul(t, t);
    let k = fpu::mul(fpu::mul(2.0, t), along);
    F(fpu::sqrt(fpu::madd(fpu::suba(ww, k), ee, tt)))
}

/// A triangle's packed normal as the loader's helper unpacks it: each short onto `768.0`'s bits,
/// less `768.0` through the FPU, and `w` one.
fn unpack(n: [i16; 3]) -> Vf {
    let one = |s: i16| fpu::sub(f32::from_bits(0x4440_0000u32.wrapping_add(s as i32 as u32)), 768.0);
    Vf::new(one(n[0]), one(n[1]), one(n[2]), 1.0)
}

/// `vmul.xyz`, `vadday.x`, `vmaddz.x` (by one), `vrsqrt`, `vmulq.xyz`: unit length, `w` kept.
fn unit(v: Vf) -> Vf {
    let sq = v.mul(v);
    let len2 = vu::madd(vu::add(sq.x(), sq.y()), 1.0, sq.z());
    let q = vu::rsqrt(1.0, len2);
    v.merge(Dest::XYZ, v.mul(Vf::splat(q)))
}

/// `a.xyz − b.xyz`, `w` from `a`.
fn minus(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

/// `vopmula a, b` / `vopmsub a ← b, a`, `.xyz`: `a × b`, `w` from `a`.
fn cross(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, Vf::opmsub(Vf::opmula(a, b), b, a))
}

/// The FPU's three-term dot product, `mula.s` / `madda.s` / `madd.s`, in this operand order.
fn dot(a: Vf, b: Vf) -> F {
    Acc::mula(a.x(), b.x()).madda(a.y(), b.y()).madd(a.z(), b.z())
}

/// The sign bits cleared, every lane (`pand` with `0x7fffffff`: nothing flushed).
fn bits_abs(v: Vf) -> Vf {
    Vf(v.0.map(|x| f32::from_bits(x.to_bits() & 0x7fff_ffff)))
}

/// The bones' search's box half side, which is also the least its bound is (`+0x24`), and the reach
/// the bound may grow to (`+0x28`).
const SEARCH_LEAST: f32 = 10.0;
const SEARCH_EXTRA: f32 = 30.0;

/// **The track's tree**, which the bones' search queries for the bike and measures its bound in: the
/// tree the broad phase lays over the terrain's box on the ground plane, as
/// [`broad_build::tree_over`] lays it, empty but for the bike.
#[derive(Clone, Debug, PartialEq)]
pub struct Cells {
    pub tree: Quadtree,
    /// The far corner the tree was laid to, and the world size of a cell.
    pub hi: [F; 2],
    pub size: F,
}

impl Cells {
    /// The tree over the box `lo`..`hi` (`x`, `z`).
    pub fn over(lo: [F; 2], hi: [F; 2]) -> Cells {
        let (mut tree, size) = broad_build::tree_over(lo, hi, F(crate::flight_search::TREE_LEAST));
        // It reports the box its cells cover, which is what the gather's bound is measured in.
        tree.flags = 2;
        Cells { tree, hi, size }
    }

    /// The tree over a terrain's box; `None` for a field with no edge (one a test samples off a
    /// world), which a track's tree is not laid over.
    pub fn of(field: &dyn crate::contact_search::Heightfield) -> Option<Cells> {
        const EDGE: f32 = 16_777_216.0;
        let (lo, hi) = (field.min(), field.max());
        let (lo, hi) = ([F(lo.x()), F(lo.z())], [F(hi.x()), F(hi.z())]);
        lo.iter().chain(&hi).all(|&v| -EDGE < v && v < EDGE).then(|| Cells::over(lo, hi))
    }

    /// Where the bike goes in the tree, standing at `frame`: [`broad_build::key`] of its mesh's root
    /// box through the frame ([`crate::flight_search::bike_box`]).
    pub fn key(&self, bike: &Bike, frame: &Mat4) -> u32 {
        let (c, h) = crate::flight_search::bike_box(&bike.mesh, frame);
        broad_build::key(&self.tree, self.hi, c, h)
    }

    /// **The search's query round `probe`**: the box of side `2 × least` round it through the tree
    /// with the static props in it, prop `i` under its key in `props` `(i, key)` (S165), and the bike
    /// after them under `key`; where the walk met the bike (how many props before it), the props it
    /// met in its order, and the box the cells cover (`x`, `z`; in cells times the cell size, the
    /// tree's origin not added back; zero for a box off the tree).
    fn query_among(&self, probe: Vf, key: u32, props: &[(usize, u32)]) -> (Option<usize>, Vec<usize>, [F; 2], [F; 2]) {
        let mut tree = self.tree.clone();
        let most = props.iter().map(|&(i, _)| i + 1).max().unwrap_or(0);
        for &(i, k) in props {
            broad_build::insert(&mut tree, i + 1, k);
        }
        broad_build::insert(&mut tree, 0, key);
        let mut broad = Broad {
            spaces: vec![Space { next: None, tables: 0 }],
            tables: vec![Tables { own: Pair { fixed: Some(0), moving: None }, subs: Vec::new() }],
            structures: vec![Structure::Tree(tree)],
            cell_size: self.size,
            places: vec![Placement::default(); most + 1],
            ..Broad::default()
        };
        let centre = [F(probe.x()), F(probe.y()), F(probe.z()), F(probe.w())];
        let half = [F(SEARCH_LEAST), F(SEARCH_LEAST), F(SEARCH_LEAST), F::ONE];
        let (mut lo, mut hi) = ([F::ZERO; 4], [F::ZERO; 4]);
        let mut q = Query::begin(&mut broad, 0, 1, centre, half, Some((&mut lo, &mut hi)), true);
        let (mut bike, mut met) = (None, Vec::new());
        while let Some(o) = q.next(&mut broad, Some((&mut lo, &mut hi))) {
            match o {
                0 => bike = bike.or(Some(met.len())),
                i => met.push(i - 1),
            }
        }
        q.end(&mut broad);
        (bike, met, [lo[0], lo[2]], [hi[0], hi[2]])
    }
}

/// The bones' search's capacity (`+0x34`): it stops listing at this many.
const SEARCH_CAPACITY: usize = 128;

/// **The track's static props the bones' search lists** (S165): the scene's collision meshes in the
/// track's tree, each listed at a regather when the query's cells reach it and its reach from the
/// probe is under the bound (a scenery mesh's flags ask for the bound, the bike's do not), and kept
/// as listed while the list is kept. The pair filter passes every bone sphere to every scenery mesh
/// (s89-along, `tests/ragdoll_props.rs`).
///
/// And the ecosystem's plants (S206): the bones' search does not keep placed groups off its list, so
/// every plant whose reach on the ground plane is under the bound is listed with the props, in the
/// walk's order ([`group_distance`]); a plant is filed in the tree after the props.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Statics {
    /// The listed props, by their index in the world's set ([`crate::world::World::collision`]); on
    /// a world of tiles the near set at the search's centre (V112, [`crate::world::World::wooded_near`]), in
    /// the order the tree's walk met them. An index at or past the set's length is a plant: the
    /// group that many past it in the world's trees ([`crate::world::World::wooded`]); and past the
    /// plants a knockable prop at rest ([`Statics::knock_from`], F7).
    pub props: Vec<usize>,
    /// How many of them the walk met before the bike: where the bike stands in the list.
    pub bike_at: usize,
}

impl Statics {
    /// Each scene prop's key in `cells`' tree, by its index in `props`: the ones that stand still.
    pub fn keys(cells: &Cells, props: &[crate::world::Prop]) -> Vec<(usize, u32)> {
        props
            .iter()
            .enumerate()
            .filter(|(_, p)| p.moving.is_none())
            .map(|(i, p)| {
                let (c, h) = crate::flight_search::prop_box(p);
                (i, broad_build::key(&cells.tree, cells.hi, c, h))
            })
            .collect()
    }

    /// The keys in `cells`' tree of the plants of `trees` that a gather round `probe` of `reach` can
    /// list (S206), each by its index `first` (the world's prop count) past the props. A plant whose
    /// reach is no nearer than the most the bound can be is never listed, and leaving it out of the
    /// tree moves no other entry in the walk's order, so only the plants near the probe are filed.
    pub fn group_keys(cells: &Cells, trees: &crate::eco::Trees, first: usize, probe: Vf, reach: F) -> Vec<(usize, u32)> {
        let most = F(SEARCH_EXTRA).max(reach).max(F(SEARCH_LEAST));
        trees
            .groups()
            .iter()
            .enumerate()
            .filter(|(_, g)| group_distance(g, trees.cell(), probe) < most)
            .map(|(j, g)| {
                let (c, h) = g.bounds(trees.cell());
                (first + j, broad_build::key(&cells.tree, cells.hi, c, h))
            })
            .collect()
    }
}

impl Statics {
    /// Where the knockable props at rest are filed in [`Statics::props`] (F7): past the world's props
    /// and its plants, prop `k` of [`knock_index`] at this plus `k`.
    pub fn knock_from(props: &[crate::world::Prop], trees: Option<&crate::eco::Trees>) -> usize {
        props.len() + trees.map_or(0, |t| t.groups().len())
    }

    /// The keys in `cells`' tree of `set`'s knockable props at rest (F7), each by its index `first`
    /// ([`Statics::knock_from`]) past [`knock_index`]: every asleep body with a collision object and
    /// a collidable mesh, under the key of its mesh's root box where its node stands it. The original
    /// files a sleeping prop's `f.col` in the track's static tree (category `0x18`, `+0x116` `0x04`);
    /// a woken one leaves it for the moving structure ([`KnockedProp::near`]).
    pub fn knock_keys(cells: &Cells, set: &crate::prop_set::PropSet, first: usize) -> Vec<(usize, u32)> {
        let mut out = Vec::new();
        for (k, (c, o)) in knock_order(set).enumerate() {
            let Some((mesh, matrix)) = resting(set, c, o) else { continue };
            let (to_world, _, _) = crate::world::Prop::posed(mesh, &matrix);
            let (centre, half) = crate::flight_search::bike_box(mesh, &to_world);
            out.push((first + k, broad_build::key(&cells.tree, cells.hi, centre, half)));
        }
        out
    }
}

/// Every body of `set`, collection by collection in object order: what a knockable prop's index in
/// [`Statics::props`] counts (F7).
fn knock_order(set: &crate::prop_set::PropSet) -> impl Iterator<Item = (usize, usize)> + '_ {
    set.collections.iter().enumerate().flat_map(|(c, coll)| (0..coll.objects.len()).map(move |o| (c, o)))
}

/// Body `k` of [`knock_order`]: its collection and object.
pub fn knock_index(set: &crate::prop_set::PropSet, k: usize) -> Option<(usize, usize)> {
    knock_order(set).nth(k)
}

/// Body `o` of collection `c` when it is a knockable prop at rest: asleep, with a collision object
/// and a collidable mesh; its mesh and the matrix its node stands it at.
fn resting(set: &crate::prop_set::PropSet, c: usize, o: usize) -> Option<(&track::Mesh, Mat4)> {
    let coll = set.collections.get(c)?;
    let object = coll.objects.get(o)?;
    object.collision.as_ref()?;
    if coll.bones.get(o)?.body.enabled {
        return None;
    }
    let mesh = set.shapes.get(c)?.get(o)?.as_ref()?;
    Some((mesh, crate::prop_set::PropSet::matrix(&object.node)))
}

/// **How far a plant's reach is from `p`**, as the gather measures a placed group (S206): on the ground
/// plane, from the plant's cell (times the cell size) less its reach (`0 +` it, as it is written), and
/// 0 when inside.
pub fn group_distance(g: &crate::eco::Group, cell: F, p: Vf) -> F {
    let at = |c: u16| F::from(i32::from(c)) * cell;
    let (dx, dz) = (F(p.x()) - at(g.cell[0]), F(p.z()) - at(g.cell[2]));
    let d = Acc::mula(dx, dx).madd(dz, dz).sqrt() - (F::ZERO + g.reach);
    if d <= 0.0 { F::ZERO } else { d }
}

/// How far listed entry `i` of [`Statics::props`] is from `p`: a prop's reach, or past the props a
/// plant's.
fn listed_distance(props: &[crate::world::Prop], trees: Option<&crate::eco::Trees>, i: usize, p: Vf) -> Option<F> {
    match props.get(i) {
        Some(prop) => Some(reach_distance(prop_reach(prop), p)),
        None => trees.and_then(|t| t.groups().get(i - props.len()).map(|g| group_distance(g, t.cell(), p))),
    }
}

/// Where a static prop stands to the search, its reach measured from: its box's centre and its
/// sphere's radius.
fn prop_reach(p: &crate::world::Prop) -> (Vf, F) {
    let (c, _) = crate::flight_search::prop_box(p);
    (Vf::new(c[0].0, c[1].0, c[2].0, c[3].0), p.radius)
}

/// **What the bones' search holds of the wreck bike** (S105). The original gathers one search for
/// all the bone colliders, once a frame before the off-bike test, around the first collision's
/// sphere where it last stood. While that sphere's reach stays inside the bound of where the list was
/// gathered, the list is only measured again; otherwise it is gathered anew, and the bike is in it
/// only if the bones no longer ignore it then and the query's cells reach where it is in the track's
/// tree. A bone's sphere is paired with the bike only when the
/// bike is listed and its distance in the list is within the sphere's radius.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BikeSearch {
    /// Where the first collision's sphere last stood (`+0x40`); `None` until it has.
    pub stood: Option<Vf>,
    /// `+0x10`: the probe's centre when the list was gathered; `+0x20` the bound; `+0x30` built.
    pub centre: Vf,
    pub bound: F,
    pub built: bool,
    /// The bike is in the list, at `distance` (how far its sphere's reach is from the probe).
    pub listed: bool,
    pub distance: F,
}

impl BikeSearch {
    /// **The frame's gather**, the probe of `reach` where the first sphere last stood, the bike's
    /// sphere at `bike` and in the tree under `key` ([`Cells::key`]); `offered` is whether the pair
    /// filter lets the bike through (the bones no longer ignore it). Nothing before the sphere has
    /// stood anywhere.
    pub fn gather(&mut self, cells: Option<&Cells>, reach: F, bike: (Vf, F), key: u32, offered: bool) {
        self.gather_among(cells, reach, bike, key, offered, &[], &[], &mut Statics::default());
    }

    /// [`BikeSearch::gather`] with the track's static props in the tree (S165): `props` the world's
    /// set and `keys` [`Statics::keys`] of it. A regather lists the props into `statics`; a kept list
    /// keeps them.
    #[allow(clippy::too_many_arguments)]
    pub fn gather_among(
        &mut self,
        cells: Option<&Cells>,
        reach: F,
        bike: (Vf, F),
        key: u32,
        offered: bool,
        props: &[crate::world::Prop],
        keys: &[(usize, u32)],
        statics: &mut Statics,
    ) {
        self.gather_wooded(cells, reach, bike, key, offered, props, None, keys, statics);
    }

    /// [`BikeSearch::gather_among`] with the ecosystem's plants in the tree too (S206): `keys` the
    /// props' [`Statics::keys`] and then the plants' [`Statics::group_keys`], and a plant is listed
    /// when its reach is under the bound, as a prop is.
    #[allow(clippy::too_many_arguments)]
    pub fn gather_wooded(
        &mut self,
        cells: Option<&Cells>,
        reach: F,
        bike: (Vf, F),
        key: u32,
        offered: bool,
        props: &[crate::world::Prop],
        trees: Option<&crate::eco::Trees>,
        keys: &[(usize, u32)],
        statics: &mut Statics,
    ) {
        self.gather_knocking(cells, reach, bike, key, offered, props, trees, keys, usize::MAX, statics);
    }

    /// [`BikeSearch::gather_wooded`] with the knockable props at rest in the tree too (F7): `keys`
    /// ends with their [`Statics::knock_keys`], each index at or past `knock_from`, and such an entry
    /// is listed wherever the query's cells reach it, its distance not asked (its `+0x116` is `0x04`:
    /// bit `0x08` clear, as the bike's).
    #[allow(clippy::too_many_arguments)]
    pub fn gather_knocking(
        &mut self,
        cells: Option<&Cells>,
        reach: F,
        bike: (Vf, F),
        key: u32,
        offered: bool,
        props: &[crate::world::Prop],
        trees: Option<&crate::eco::Trees>,
        keys: &[(usize, u32)],
        knock_from: usize,
        statics: &mut Statics,
    ) {
        let Some(probe) = self.stood else { return };
        if !self.regathers(reach) {
            self.distance = reach_distance(bike, probe);
            return;
        }
        // Without a tree the bike is anywhere the query looks, and the bound is its least.
        let (at, met, [lo_x, lo_z], [hi_x, hi_z]) = cells.map_or((Some(0), Vec::new(), [F::ZERO; 2], [F::ZERO; 2]), |c| c.query_among(probe, key, keys));
        let (x, z) = (F(probe.x()), F(probe.z()));
        self.centre = probe;
        self.bound = (x - lo_x).min(z - lo_z).min(hi_x - x).min(hi_z - z).min(F(SEARCH_EXTRA).max(reach)).max(F(SEARCH_LEAST));
        // The list in the walk's order, the bike where the walk met it, up to the capacity.
        let bike_at = at.filter(|_| offered);
        statics.props.clear();
        statics.bike_at = 0;
        let mut listed = false;
        for (n, &i) in met.iter().enumerate() {
            if bike_at == Some(n) && !listed {
                listed = statics.props.len() < SEARCH_CAPACITY;
                statics.bike_at = statics.props.len();
            }
            if statics.props.len() + usize::from(listed) >= SEARCH_CAPACITY {
                break;
            }
            if i >= knock_from || listed_distance(props, trees, i, probe).is_some_and(|d| d < self.bound) {
                statics.props.push(i);
            }
        }
        if bike_at.is_some_and(|n| n >= met.len()) && !listed {
            listed = statics.props.len() < SEARCH_CAPACITY;
            statics.bike_at = statics.props.len();
        }
        self.listed = listed;
        self.distance = reach_distance(bike, probe);
        self.built = true;
    }

    /// Whether this frame's gather, the probe of `reach` where the first sphere last stood, queries
    /// the tree anew: the sphere has stood, and the list is not built or the probe's reach has left
    /// its bound.
    pub fn regathers(&self, reach: F) -> bool {
        self.stood.is_some_and(|p| !(self.built && length(minus(p, self.centre)) + reach < self.bound))
    }

    /// Whether a sphere of `radius` is paired with the bike.
    pub fn reaches(&self, radius: F) -> bool {
        self.listed && self.distance <= radius
    }
}

/// How far the reach of the sphere `(centre, radius)` is from `p`; never below 0.
fn reach_distance((centre, radius): (Vf, F), p: Vf) -> F {
    F::ZERO.max(length(minus(centre, p)) - radius)
}

/// The broad phase's length, `mula`/`madda`/`madd` then the root.
fn length(d: Vf) -> F {
    Acc::mula(d.x(), d.x()).madda(d.y(), d.y()).madd(d.z(), d.z()).sqrt()
}

/// **A path car as the bones' search lists it** (S142). The original's search hands out every
/// moving collidable of the space whatever its position, a car's mesh among them, and re-measures
/// each one's distance every frame; a sphere is paired with the car when that distance is within
/// its radius. The test walks the car's mesh in the collidable's frame ([`crate::world::Prop`]'s
/// `to_world`), and the push-out takes the car as a mover ([`crate::traffic::Moving::mover`]).
#[derive(Clone, Debug)]
pub struct PathCar {
    pub mesh: track::Mesh,
    pub frame: Node,
    pub mover: crate::contact::Mover,
    /// How far the car's sphere's reach is from where the first collision's sphere last stood.
    pub distance: F,
    /// The collidable's sphere (`+0x40`, `+0x60`), which the segment probe's pair test rejects on.
    pub centre: Vf,
    pub radius: F,
}

impl PathCar {
    /// The car under `prop`, measured from `probe`; `None` for a prop that does not move.
    pub fn of(prop: &crate::world::Prop, probe: Vf) -> Option<PathCar> {
        let moving = prop.moving.as_ref()?;
        let [x, y, z, at] = prop.to_world.0;
        Some(PathCar {
            mesh: prop.mesh.clone(),
            frame: Node { rows: [x, y, z], origin: at },
            mover: moving.mover(),
            distance: reach_distance((prop.centre, prop.radius), probe),
            centre: prop.centre,
            radius: prop.radius,
        })
    }

    /// **The cars a frame's spheres can be paired with**: every moving prop whose distance from
    /// `probe` is within `reach` (the largest sphere's radius), in the props' order. The rest no
    /// sphere reaches, and no bone's segment strikes a car at all ([`class::BONE_SWEEP`]), so their
    /// meshes are not copied.
    pub fn near(props: &[crate::world::Prop], probe: Vf, reach: F) -> Vec<PathCar> {
        props
            .iter()
            .filter(|p| p.moving.is_some() && reach_distance((p.centre, p.radius), probe) <= reach)
            .filter_map(|p| PathCar::of(p, probe))
            .collect()
    }

    /// The sphere test against the car's mesh on a record holding a hit at `held` (the record's
    /// hit from the candidates before it); `Some` when this car took the record.
    pub fn touch(&self, centre: Vf, radius: F, held: Option<F>) -> Option<Touch> {
        touch_mesh(&self.mesh, Normals::Unpacked, &self.frame, centre, radius, held)
    }
}

/// **A static prop as the bones' search lists it** (S165): a scene collision mesh, standing where
/// the scene put it ([`crate::world::Prop`]'s `to_world`, the collidable's frame), measured from
/// where the first collision's sphere last stood. It does not move: a contact on it is with the
/// scenery ([`crate::ragdoll_contact::Scene::other`] `None`).
#[derive(Clone, Debug)]
pub struct StillProp {
    pub mesh: track::Mesh,
    pub frame: Node,
    /// How far the prop's reach is from the probe, measured again each frame.
    pub distance: F,
}

impl StillProp {
    /// The prop as the bones see it, measured from `probe`.
    pub fn of(prop: &crate::world::Prop, probe: Vf) -> StillProp {
        let [x, y, z, at] = prop.to_world.0;
        StillProp { mesh: prop.mesh.clone(), frame: Node { rows: [x, y, z], origin: at }, distance: reach_distance(prop_reach(prop), probe) }
    }

    /// **The listed props a frame's spheres can be paired with**: those of `statics` whose distance
    /// from `probe` is within `reach` (the largest sphere's radius), in the list's order, each with
    /// its place in the list. The rest no sphere reaches, so their meshes are not copied.
    pub fn near(props: &[crate::world::Prop], statics: &Statics, probe: Vf, reach: F) -> Vec<(usize, StillProp)> {
        StillProp::near_wooded(props, None, statics, probe, reach)
    }

    /// [`StillProp::near`] with the listed plants (S206). The original's search hands a sphere a
    /// placed group's parts one by one, at the group's place in the list, each made ready under the
    /// plant and paired with no filter, when the group's own distance is within the sphere's radius.
    /// A part is a trunk ([`crate::eco::Group::parts`]): a mesh like a prop's, its gate open, no body
    /// or owner behind it, so a contact on it is with the still scenery. So each trunk of a listed
    /// plant is a [`StillProp`] at the plant's place, measured as the plant is. A part the port does
    /// not build (a sphere, MX vs. ATV's) is one the sphere's pair has no test for.
    pub fn near_wooded(
        props: &[crate::world::Prop],
        trees: Option<&crate::eco::Trees>,
        statics: &Statics,
        probe: Vf,
        reach: F,
    ) -> Vec<(usize, StillProp)> {
        let mut out = Vec::new();
        for (n, &i) in statics.props.iter().enumerate() {
            if let Some(p) = props.get(i) {
                if reach_distance(prop_reach(p), probe) <= reach {
                    out.push((n, StillProp::of(p, probe)));
                }
                continue;
            }
            let Some((t, g)) = trees.and_then(|t| t.groups().get(i - props.len()).map(|g| (t, g))) else { continue };
            let distance = group_distance(g, t.cell(), probe);
            if !(distance <= reach) {
                continue;
            }
            for &part in g.parts.iter().flatten() {
                let Some(trunk) = t.props.props.get(part) else { continue };
                out.push((n, StillProp { distance, ..StillProp::of(trunk, probe) }));
            }
        }
        out
    }

    /// The sphere test against the prop's mesh on a record holding a hit at `held`; `Some` when this
    /// prop took the record.
    pub fn touch(&self, centre: Vf, radius: F, held: Option<F>) -> Option<Touch> {
        touch_mesh(&self.mesh, Normals::Unpacked, &self.frame, centre, radius, held)
    }
}

/// **A knockable prop as the bones' search lists it** (S240): an awake prop's collidable mesh (its
/// model's `f.col`, [`crate::prop_set::PropSet::shapes`]) where its body node stands it, measured from
/// where the first collision's sphere last stood. The original files a woken prop's collidable in the
/// space's moving structure (category `0x18`, `+0x116` `0x14`: listed wherever the walk meets it, as a
/// path car is), and a sphere within its reach is paired with its mesh on the same record. A contact on
/// it is with the prop's collision object (`+0x80`), so the push-out measures the bone against the
/// prop's body ([`Against::Other`], a copy: the bone is pushed, the prop is not). A sphere whose record
/// the prop took fires the prop's listeners, the struck one waking the prop as a strike does
/// ([`crate::prop_set::PropSet::struck`]); [`KnockedProp::struck`] says the caller should.
///
/// A prop at rest is in the track's static tree instead (F7, `+0x116` `0x04`): listed at a regather
/// wherever the query's cells reach it, at its place in the walk among the scene props, and kept with
/// the list ([`KnockedProp::resting`]). It is met as an awake one is: the same mesh test on the same
/// record, the push-out against its body (asleep, so still), and its struck listener wakes it. The
/// prop's own rays never meet a bone: the bones' collisions are not filed on the props' world node, so
/// the prop's search does not find them.
#[derive(Clone, Debug)]
pub struct KnockedProp {
    pub shapes: std::sync::Arc<Vec<Vec<Option<track::Mesh>>>>,
    pub frame: Node,
    pub distance: F,
    /// Its collection, its object, and its body and collision object, copied.
    pub pushed: crate::prop_contact::Pushed,
    /// A sphere's record took it this frame: its struck listener runs.
    pub struck: bool,
    /// Its place in [`Statics::props`] when it is at rest (F7), the static tree's walk offering it
    /// there; `None` for an awake one, offered after the static tree's entries.
    pub place: Option<usize>,
}

impl KnockedProp {
    /// **The knockable props a frame's spheres can be paired with**: every awake body with a collision
    /// object and a mesh whose distance from `probe` is within `reach` (the largest sphere's radius),
    /// in collection and object order.
    pub fn near(set: &crate::prop_set::PropSet, probe: Vf, reach: F) -> Vec<KnockedProp> {
        let mut out = Vec::new();
        for (c, (coll, shapes)) in set.collections.iter().zip(set.shapes.iter()).enumerate() {
            for (o, (object, mesh)) in coll.objects.iter().zip(shapes).enumerate() {
                let (Some(mesh), Some(col), Some(bone)) = (mesh, object.collision.as_ref(), coll.bones.get(o)) else { continue };
                if !bone.body.enabled {
                    continue;
                }
                let (to_world, centre, radius) = crate::world::Prop::posed(mesh, &crate::prop_set::PropSet::matrix(&object.node));
                let distance = reach_distance((centre, radius), probe);
                if !(distance <= reach) {
                    continue;
                }
                let [x, y, z, at] = to_world.0;
                out.push(KnockedProp {
                    shapes: set.shapes.clone(),
                    frame: Node { rows: [x, y, z], origin: at },
                    distance,
                    pushed: crate::prop_contact::Pushed { collection: c, object: o, collider: col.collider, body: bone.body, mass: bone.mass },
                    struck: false,
                    place: None,
                });
            }
        }
        out
    }

    /// **The knockable props at rest a frame's spheres can be paired with** (F7): the entries of
    /// `statics` at or past `first` ([`Statics::knock_from`]) that are still asleep, whose distance
    /// from `probe` is within `reach`, in the list's order, each with its place in it. One woken since
    /// the list was gathered is the moving structure's ([`KnockedProp::near`]).
    pub fn resting(set: &crate::prop_set::PropSet, statics: &Statics, first: usize, probe: Vf, reach: F) -> Vec<KnockedProp> {
        let mut out = Vec::new();
        for (n, &i) in statics.props.iter().enumerate() {
            let Some((c, o)) = i.checked_sub(first).and_then(|k| knock_index(set, k)) else { continue };
            let Some((mesh, matrix)) = resting(set, c, o) else { continue };
            let (to_world, centre, radius) = crate::world::Prop::posed(mesh, &matrix);
            let distance = reach_distance((centre, radius), probe);
            if !(distance <= reach) {
                continue;
            }
            let (Some(col), Some(bone)) = (set.collections[c].objects[o].collision.as_ref(), set.collections[c].bones.get(o)) else { continue };
            let [x, y, z, at] = to_world.0;
            out.push(KnockedProp {
                shapes: set.shapes.clone(),
                frame: Node { rows: [x, y, z], origin: at },
                distance,
                pushed: crate::prop_contact::Pushed { collection: c, object: o, collider: col.collider, body: bone.body, mass: bone.mass },
                struck: false,
                place: Some(n),
            });
        }
        out
    }

    /// The prop's collidable mesh.
    pub fn mesh(&self) -> Option<&track::Mesh> {
        self.shapes.get(self.pushed.collection)?.get(self.pushed.object)?.as_ref()
    }

    /// The sphere test against the prop's mesh on a record holding a hit at `held`; `Some` when this
    /// prop took the record.
    pub fn touch(&self, centre: Vf, radius: F, held: Option<F>) -> Option<Touch> {
        touch_mesh(self.mesh()?, Normals::Unpacked, &self.frame, centre, radius, held)
    }
}

/// **The collidables' class words** (`+0x84`), which the pair filter matches against a probe's
/// include or exclude list (S166).
pub mod class {
    /// The track's scenery, its static props' meshes (theirs is `0x220`): the one class a bone's
    /// segment probe includes, set by the bone collision's constructor.
    pub const SCENERY: u32 = 0x20;
    /// A path car's mesh collidable.
    pub const PATH_CAR: u32 = 0x48;
    /// The wreck bike's collidable (the wreck part's), the bike's own class words and its rider's.
    pub const WRECK_BIKE: u32 = 0x9_a058;
    /// What a bone's segment probe lets through: the filter's list is in include mode and holds
    /// [`SCENERY`] only.
    pub const BONE_SWEEP: u32 = SCENERY;
}

/// **One candidate of the bones' search as a segment probe meets it** (S166): its distance in the
/// list, its class, its sphere, and its mesh where it stands.
#[derive(Clone, Copy)]
pub struct Candidate<'a> {
    pub distance: F,
    pub class: u32,
    pub centre: Vf,
    pub radius: F,
    pub mesh: &'a track::Mesh,
    pub frame: &'a Node,
}

/// **A bone's segment from `from` to `to` through the search's list** (S166): the bone collision's
/// sweep, its probe's include list `includes` ([`class::BONE_SWEEP`] for a bone). The segment probe
/// is stood on the two points and cleared, which wraps the segment in a sphere of half its length;
/// then each candidate in the list's order whose distance is within that radius and whose class the
/// include list holds is paired with it ([`sweep_probe::pair`], a mesh), and the probe is hit when
/// any pair says so. The hit is the probe's nearest point and normal as the last pair left them, in
/// the world.
///
/// Each pair starts the nearest over, so a later candidate that is struck at all wins over a nearer
/// hit on an earlier one; and one struck no nearer than the segment's end leaves the nearest as the
/// one before it left it. `None` also when the probe was struck but no pair of this sweep ever
/// latched a nearest (the segment's end exactly on a face), where the original reads whatever its
/// probe's record kept from an earlier frame.
pub fn sweep_through<'a>(from: Vf, to: Vf, includes: u32, list: impl IntoIterator<Item = Candidate<'a>>) -> Option<(Vf, Vf)> {
    let mut probe = SweepProbe { segments: vec![Segment { ends: [from, to], nodes: [None; 2] }], ..SweepProbe::default() };
    sweep_probe::clear(&mut probe);
    let reach = probe.world.radius;
    let (mut struck, mut latched) = (false, false);
    for (object, c) in list.into_iter().enumerate() {
        if !(c.distance <= reach) || c.class & includes == 0 {
            continue;
        }
        let transform = [c.frame.rows[0], c.frame.rows[1], c.frame.rows[2], c.frame.origin];
        let placed = Placed { transform, mesh: MeshRef::of(c.mesh), object };
        let target = Target { nearby: true, centre: c.centre, radius: c.radius, shape: Shape::Mesh(placed) };
        struck |= sweep_probe::pair(&mut probe, Some(&target));
        latched |= probe.answer.record.nearest < F::ONE;
    }
    let rec = &probe.answer.record;
    (struck && latched).then_some((rec.nearest_point, rec.nearest_normal))
}

/// **A ride's ragdoll scene: the wreck bike**, standing in its frame. A bone that touches the mesh
/// is with the bike's own body object, and the push-out measures the bone against the bike's body
/// there.
pub struct WreckBikeScene<'a> {
    pub mesh: &'a Bike,
    pub frame: Node,
    /// The bike's body and its collision object, copied: the velocity query writes the object's
    /// centre of mass in the world, which the bike's own resolution rewrites before it reads it.
    pub body: Body,
    pub collider: Collider,
    pub mass: F,
    /// What the bones' search holds of the bike this frame.
    pub search: BikeSearch,
    /// Every centre a sphere was stood at this frame, in the order the step stood them.
    pub stood: Vec<Vf>,
    /// The path cars the search lists near the bones this frame (S142), after the bike; empty
    /// before the search is built.
    pub cars: Vec<PathCar>,
    /// The static props the search lists near the bones this frame (S165), each with its place in
    /// the list, and where the bike stands among them ([`Statics::bike_at`]).
    pub props: Vec<(usize, StillProp)>,
    pub bike_at: usize,
    /// What the last sphere's contact is with: 0 the bike, `k + 1` the car `k` (and past the cars the
    /// knockable props, S240); `None` for nothing
    /// or a static prop, which is the scenery.
    pub with: Option<usize>,
    /// The bike collidable's sphere (`B+0x40`, `B+0x60`), which a segment probe's pair test rejects
    /// on (S166); `None` leaves the bike out of the sweep.
    pub sphere: Option<(Vf, F)>,
    /// The knockable props the search lists near the bones this frame: first those at rest (F7), in
    /// the list's order, each offered at its place among the static props; then the awake ones
    /// (S240), after the static tree's entries and before the cars. Empty before the search is built.
    /// Contact `k + 1 + cars` is with prop `k`.
    pub knocked: Vec<KnockedProp>,
}

impl<'a> WreckBikeScene<'a> {
    /// The scene of `bike` (its mesh) standing at `frame` (the ride's transform of the wreck bike), a
    /// sphere paired with it as `search` says.
    pub fn new(mesh: &'a Bike, frame: &Mat4, bike: &crate::wreck_bike::WreckBike, search: BikeSearch) -> WreckBikeScene<'a> {
        WreckBikeScene {
            mesh,
            frame: Node { rows: [frame.0[0], frame.0[1], frame.0[2]], origin: frame.0[3] },
            body: bike.body.clone(),
            collider: bike.collider.clone(),
            mass: bike.mass,
            search,
            stood: Vec::new(),
            cars: Vec::new(),
            props: Vec::new(),
            bike_at: 0,
            with: None,
            sphere: None,
            knocked: Vec::new(),
        }
    }
}

impl Scene for WreckBikeScene<'_> {
    /// The sphere's one record, offered the static tree's candidates in the list's order (the props,
    /// the knockable props at rest, the bike where the walk met it) and then each listed awake prop
    /// and car it reaches: a later candidate's hit replaces the held one only when strictly nearer
    /// (the record's single slot).
    fn touching(&mut self, centre: Vf, radius: F) -> Option<Vec<(Vf, Vf)>> {
        self.stood.push(centre);
        self.with = None;
        let mut best: Option<Touch> = None;
        let mut bike_done = false;
        let mut props = self.props.iter().peekable();
        let first = 1 + self.cars.len();
        // The props at rest lead `knocked`, in the list's order (F7).
        let mut rest = 0;
        loop {
            let prop_next = props.peek().map(|(n, _)| *n);
            let rest_next = self.knocked.get(rest).and_then(|p| p.place);
            let next = match (prop_next, rest_next) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
            if !bike_done && next.is_none_or(|n| self.bike_at <= n) {
                bike_done = true;
                if self.search.reaches(radius)
                    && let Some(t) = touch_mesh(&self.mesh.mesh, Normals::Packed(&self.mesh.normals), &self.frame, centre, radius, best.map(|b| b.distance))
                {
                    best = Some(t);
                    self.with = Some(0);
                }
                continue;
            }
            if let Some(n) = rest_next
                && prop_next.is_none_or(|p| n < p)
            {
                let prop = &mut self.knocked[rest];
                if prop.distance <= radius
                    && let Some(t) = prop.touch(centre, radius, best.map(|b| b.distance))
                {
                    best = Some(t);
                    self.with = Some(first + rest);
                    prop.struck = true;
                }
                rest += 1;
                continue;
            }
            let Some((_, prop)) = props.next() else { break };
            if !(prop.distance <= radius) {
                continue;
            }
            if let Some(t) = prop.touch(centre, radius, best.map(|b| b.distance)) {
                best = Some(t);
                self.with = None;
            }
        }
        if self.search.built {
            for (k, prop) in self.knocked.iter_mut().enumerate().skip(rest) {
                if !(prop.distance <= radius) {
                    continue;
                }
                if let Some(t) = prop.touch(centre, radius, best.map(|b| b.distance)) {
                    best = Some(t);
                    self.with = Some(first + k);
                    prop.struck = true;
                }
            }
            for (k, car) in self.cars.iter().enumerate() {
                if !(car.distance <= radius) {
                    continue;
                }
                if let Some(t) = car.touch(centre, radius, best.map(|b| b.distance)) {
                    best = Some(t);
                    self.with = Some(k + 1);
                }
            }
        }
        best.map(|t| vec![t.world])
    }

    /// The bone's segment through the same list: the bike when the search lists it, then the cars
    /// (S166). The probe includes neither class, so it never strikes them.
    fn sweep(&mut self, from: Vf, to: Vf) -> Option<(Vf, Vf)> {
        let search = self.search;
        let bike = self.sphere.filter(|_| search.listed).map(|(centre, radius)| Candidate {
            distance: search.distance,
            class: class::WRECK_BIKE,
            centre,
            radius,
            mesh: &self.mesh.mesh,
            frame: &self.frame,
        });
        let cars = self.cars.iter().map(|car| Candidate {
            distance: car.distance,
            class: class::PATH_CAR,
            centre: car.centre,
            radius: car.radius,
            mesh: &car.mesh,
            frame: &car.frame,
        });
        sweep_through(from, to, class::BONE_SWEEP, bike.into_iter().chain(cars))
    }

    fn other(&mut self, i: usize) -> Option<usize> {
        if i == 0 { self.with } else { None }
    }

    fn velocity(&mut self, other: usize, at: Vf) -> Vf {
        if let Some(p) = other.checked_sub(1 + self.cars.len()).and_then(|k| self.knocked.get_mut(k)) {
            let (collider, mut w) = p.pushed.weighed();
            return moving(at, Against::Other(&mut Other { collider, side: Side::Body(&mut w) }));
        }
        if let Some(car) = other.checked_sub(1).and_then(|k| self.cars.get(k)) {
            return moving(at, Against::Mover(&car.mover));
        }
        let mut w = Weighed { body: &mut self.body, mass: self.mass };
        moving(at, Against::Other(&mut Other { collider: &mut self.collider, side: Side::Body(&mut w) }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_packed_normal_unpacks_to_itself() {
        for c in [0.0f32, 1.0, -1.0, 0.5, -0.7071, 0.3333] {
            let n = crate::flight_search::pack_normal([F(c), F(0.0), F(c)]);
            let u = unpack(n);
            assert_eq!(crate::flight_search::pack_normal([F(u.x()), F(u.y()), F(u.z())]), n, "{c}");
            assert_eq!(u.w(), 1.0);
        }
    }

    #[test]
    fn the_far_end_keeps_the_originals_sign() {
        // Past the far end the original adds the middle term where the distance would subtract it.
        let d = edge(Vf::new(3.0, 1.0, 0.0, 1.0), Vf::new(1.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 0.0, 1.0));
        assert_eq!(d.0, fpu::sqrt(17.0));
        let mid = edge(Vf::new(0.5, 2.0, 0.0, 1.0), Vf::new(1.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 0.0, 1.0));
        assert_eq!(mid.0, 2.0);
    }
}
