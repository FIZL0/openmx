//! **The props' rays strike the wreck bike, and its rays the props** (S197b, S244): a knockable
//! prop's own ray probe against the wreck bike's collision mesh (`bike\col\wreckbikef.col`) where
//! the bike's record stands it, as entries on the prop's collision record — the wreck bike named on
//! the other side — and the mirrors on the wreck bike's; and the wreck bike's rays against the props'
//! meshes the same way round.
//!
//! The original files the wreck bike's collision object on free09's props node with the balls' (the
//! hit2 states, research/s197b: the node's list holds the bikes' objects, the wreck bike's among them,
//! then the woken balls'). The node's update empties every list and searches every object in list
//! order, then steps the node's world, which holds the wreck bike's body alone; the balls' collection
//! worlds step after. So in a tick where a ball meets the wreck bike:
//!
//! 1. the wreck bike searches first: the terrain, the scene, and the props — its ray probe's category
//!    list is empty, so its rays strike a ball's mesh (0x18) too (S244's hit3 states, research/s244;
//!    S197b read the leave-out list 0x8 as the rays', where it is the sweep probe's): an entry (kind
//!    0) on the wreck bike's list naming the ball, its mirror (kind 1) at the head of the ball's;
//! 2. the ball searches: its terrain rays, then its ray probe meets the wreck bike's mesh (category
//!    [`wreck_bike::COLLIDABLE`], in any of the prop's category lists, the wake mask's alone too): an
//!    entry (kind 0) on the ball's list naming the wreck bike, its mirror (kind 1, the normal turned
//!    round, the ball on its other side) on the wreck bike's, after its own entries;
//! 3. the wreck bike's world steps: its force listener resolves its list, its entries against the
//!    ball's body as the frame found it — the impulse, the spring, the friction going into both — and
//!    the mirrors, which take no impulse, the spring or the friction on the bike alone;
//! 4. the ball's collection steps: its force listener resolves its list, the first substep's impulse
//!    on an entry naming the wreck bike going into both bodies (hit5: a ball's impulse throws the
//!    wreck bike there, in the props' step, not its frame).
//!
//! In the port the wreck bike is on the props' node too (S243, [`crate::Sim::frame_held_wrecked`]):
//! its hull is a collidable in every prop's world ([`crate::prop_world::Wreck`],
//! [`crate::flight_search::Scene::probed_among`]), gathered into the prop's kept nearby list with the
//! rest and met by the prop's one ray record, so a ray keeps one contact, the nearer, between the scene
//! and the bike. The pass hands the mirrors and the struck bodies to the wreck bike's frame
//! ([`WreckBike::tick_struck`], through [`crate::ride::Crash::struck`]), and what its frame pushed comes
//! back to the props ([`crate::Sim::wreck_pushed_back`], S244) before the props' step, which reads the
//! wreck bike as its other side ([`crate::Sim::props_step`]). The hit2 states are the proof:
//! `tests/prop_wreck.rs`.
//!
//! The other way round (S244), [`strikes`] ([`crate::Sim::wreck_strike`]): [`wreck_rays_on_props`]
//! and the mirrors it files on the props, then [`struck_by_props`]; both ways' entries, mirrors and
//! struck bodies go to the wreck bike's frame. The hit3 and hit5 states are the proof.
//!
//! [`struck_by_props`] is S197b's stand-in, a search of the wreck bike after the pass with the entries
//! put after everything the prop's own search found; the seats' shared props took it until S260, which
//! puts every seat's wreck bike on the node ([`crate::prop_seats`]).
//!
//! The sweep probes meet neither way: the wreck bike's and a prop's sweep both exclude 0x8 (and
//! 0x800), which both meshes carry ([`crate::prop_world::sweep_categories`]).
//!
//! On the node the wreck bike's own rays run too (S260a, [`wreck_rays_first`] and [`wreck_rays_filed`]
//! in [`PropSet::search_in_among`]): first, as its object is listed ahead of the balls', each struck
//! prop woken before its search, the mirrors at the head of its list after it; so one Sim
//! ([`crate::Sim::frame_held_wrecked`]) and the seats' shared props ([`crate::prop_seats`]) both have
//! them. The hit3 states are the proof.
//!
//! On a world that wraps (MD167a) the wreck bike's rays on the node meet a prop by a seam's far side
//! one width over ([`wreck_rays_wrapped`]), as the riding bike's do (MD167): the body goes to its frame
//! a width back and to the props' step with the bike a width over ([`crate::prop_contact::across_moved`]).
//!
//! What is not here: the wreck bike's entries put after its scene's, where the
//! original's one narrow pass keeps one contact per ray between them; the rider's ragdoll and the plane
//! against a prop; a prop's rays on a riding bike's mesh (on another prop's: S242,
//! [`crate::prop_props`]).

#![deny(clippy::float_arithmetic)]

use crate::contact::{Contact, Node};
use crate::contact_search::{self, NO_SURFACE, Record};
use crate::flight_search::{self, Rays};
use crate::math::Mat4;
use crate::prop_contact::{Pushed, Strike};
use crate::prop_set::PropSet;
use crate::prop_world::{self, Wreck};
use crate::world::Prop;
use crate::ray_probe::{self, Facet, RayProbe};
use crate::track::Mesh;
use crate::world_search::{self, Candidate, Categories, Class, Collidable, Narrow, Nearby, flags};
use crate::wreck_bike::{self, WreckBike};
use ps2_float::{F, Vf, fpu};

/// What the props' rays struck on the wreck bike this tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WreckStrike {
    /// The mirrors (kind 1) for the wreck bike's list, in the order the props searched, each naming
    /// its prop by its place in [`WreckStrike::struck`].
    pub mirrors: Vec<Contact>,
    /// Each prop whose rays struck the bike, in the order they searched, as the frame found its body.
    pub struck: Vec<Pushed>,
    /// The wreck bike's own rays on the props' meshes (S244, [`wreck_rays_on_props`]): its entries
    /// (kind 0), each naming its prop by its place in [`WreckStrike::struck`], for its list after
    /// what its own search finds and before the mirrors.
    pub entries: Vec<Contact>,
}

impl WreckStrike {
    pub fn is_empty(&self) -> bool {
        self.mirrors.is_empty() && self.entries.is_empty()
    }
}

/// The probe's and the wreck bike's places in the search's list.
const PROBE: usize = 0;
const BIKE: usize = 1;

/// The prop's ray probe against the wreck bike's mesh.
struct OnTheBike<'a> {
    rays: &'a Rays,
    at: [Vf; 4],
    mesh: &'a Mesh,
    bike_at: [Vf; 4],
    facets: &'a [Facet],
    ray: ray_probe::Record,
}

impl Narrow for OnTheBike<'_> {
    fn clear(&mut self, objects: &mut [Collidable], probe: usize) {
        ray_probe::clear(&mut objects[probe].flags, &mut self.ray);
    }

    fn prepare(&mut self, _objects: &mut [Collidable], _probe: usize) {}

    fn pair(&mut self, _objects: &mut [Collidable], _probe: usize, other: usize) -> bool {
        let target = ray_probe::Mesh {
            id: other as u32,
            transform: self.bike_at,
            root: 0,
            nodes: &self.mesh.nodes,
            facets: self.facets,
            vertices: &self.mesh.vertices,
        };
        let probe = RayProbe { transform: self.at, root: self.rays.root, nodes: &self.rays.nodes, segments: &self.rays.segments };
        ray_probe::pair(&probe, &ray_probe::Shape::Mesh(target), &mut self.ray)
    }

    fn parts(&mut self, _objects: &mut [Collidable], _group: usize) -> usize {
        0
    }

    fn part(&mut self, _objects: &mut [Collidable], _group: usize, _i: usize) -> Option<usize> {
        None
    }

    fn heard_hit(&mut self, _objects: &mut [Collidable], _listener: usize, _probe: usize, _other: usize) {}

    fn heard_struck(&mut self, _objects: &mut [Collidable], _listener: usize, _struck: usize, _probe: usize) {}

    fn on_hit(&mut self, _objects: &mut [Collidable], _callback: usize, _probe: usize, _other: usize) {}

    fn on_struck(&mut self, _objects: &mut [Collidable], _callback: usize, _struck: usize, _probe: usize) {}
}

/// The length between two points, through the FPU.
fn distance(a: Vf, b: Vf) -> F {
    let d = a.sub(b);
    let acc = fpu::mula(d.x(), d.x());
    let acc = fpu::madda(acc, d.y(), d.y());
    F(fpu::sqrt(fpu::madd(acc, d.z(), d.z())))
}

fn frame(node: &Node) -> [Vf; 4] {
    [node.rows[0], node.rows[1], node.rows[2], node.origin]
}

/// The mesh's sphere where `t` stands it: its root box's centre through the frame, and the box's half
/// diagonal (the collidable's `+0x40`, `+0x60`).
fn sphere(mesh: &Mesh, t: &[Vf; 4]) -> (Vf, F) {
    let Some(n) = mesh.nodes.first() else { return (t[3], F::ZERO) };
    let rays = Rays { root: 0, nodes: vec![*n], segments: Vec::new() };
    flight_search::reach(&rays, t)
}

/// **The props' rays on the wreck bike** standing as its record's node has it (`mesh` its
/// `wreckbikef.col`): every filed prop object with a ray probe, in the node's list order, its
/// probe where its node stands, its category list as its search left it. A prop that struck gets an
/// entry for each struck ray after its own entries and ahead of the mirrors props filed after it put
/// on its list ([`own_end`]; at most its capacity), naming the wreck bike as its
/// other side `me` (the index its step's other sides give the bike), and its flags set as a hit sets
/// them; its listeners hear the hit as its search's do (an asleep body woken by a category of the wake
/// mask, the collection activated). Answers the mirrors and the struck bodies for the bike's frame.
pub fn struck_by_props(set: &mut PropSet, mesh: &Mesh, bike: &WreckBike, me: usize) -> WreckStrike {
    struck_on(set, mesh, frame(&bike.record.node), wreck_bike::COLLIDABLE, bike.collider.collides_with, me)
}

/// [`struck_by_props`] on any collision mesh standing at `bike_at` (its node's rows and origin), its
/// collidable's `category` and `collides_with` mask: the wreck bike's, and the plane's hull (S241,
/// [`crate::flight::Flight::frame_props`]).
pub fn struck_on(set: &mut PropSet, mesh: &Mesh, bike_at: [Vf; 4], category: u32, collides_with: u32, me: usize) -> WreckStrike {
    struck_on_by(set, mesh, bike_at, category, collides_with, me, None).0
}

/// **[`struck_on`] on a world that wraps** (MD254a, MD29: the port's own): a prop by the far side of
/// a seam from the mesh, whose rays reach the mesh's copy one width over, strikes it there — the mesh
/// moved by the seam's shift for that prop's search, as the riding bike's strike has the props
/// ([`crate::prop_contact::strike_wrapped`]): its entries at its own place, the mirrors moved back to
/// the mesh's. A prop the mesh's own place gives a hit is struck there alone. Also answers each
/// struck body met at a copy, by its place in [`WreckStrike::struck`], and the shift that brings
/// the mesh to it (as [`Strike::across`] has them); on a world that does not wrap, none, and the
/// strike is [`struck_on`]'s.
#[allow(clippy::too_many_arguments)]
pub fn struck_on_in(
    set: &mut PropSet,
    mesh: &Mesh,
    bike_at: [Vf; 4],
    category: u32,
    collides_with: u32,
    me: usize,
    world: &dyn crate::world::World,
) -> (WreckStrike, Vec<(usize, [F; 2])>) {
    struck_on_by(set, mesh, bike_at, category, collides_with, me, Some(world))
}

#[allow(clippy::too_many_arguments)]
fn struck_on_by(
    set: &mut PropSet,
    mesh: &Mesh,
    bike_at: [Vf; 4],
    category: u32,
    collides_with: u32,
    me: usize,
    world: Option<&dyn crate::world::World>,
) -> (WreckStrike, Vec<(usize, [F; 2])>) {
    let mut out = WreckStrike::default();
    let mut across = Vec::new();
    if set.tables.is_none() || mesh.nodes.is_empty() {
        return (out, across);
    }
    let (bike_centre, bike_radius) = sphere(mesh, &bike_at);
    let facets = flight_search::facets(mesh);
    let rays = std::sync::Arc::clone(&set.rays);
    // In the node's list order, as its search pass walks them (S258).
    set.stamp_filed();
    for (c, k) in set.node_order() {
        let Some(Some(tree)) = rays[c].get(k) else { continue };
        let own_end = own_end(set, c, k);
        let coll = &mut set.collections[c];
        let o = &coll.objects[k];
        let (true, Some(col)) = (o.filed, o.collision.as_ref()) else { continue };
        let at = frame(&o.node);
        let (centre, radius) = flight_search::reach(tree, &at);
        // The mesh's own place, then on a world that wraps its copies by the seams the prop's
        // sphere reaches over, where the prop's rays can reach them (MD254a).
        let mut places = vec![[F::ZERO; 2]];
        if let Some(world) = world {
            for s in crate::tiles::seam_shifts(world, centre, radius + bike_radius) {
                let by = [F::ZERO - s[0], F::ZERO - s[1]];
                if distance(centre, crate::seam::moved(bike_centre, by)) - bike_radius <= radius {
                    places.push(by);
                }
            }
        }
        let mut met = None;
        for by in places {
            let (bike_at, bike_centre) = if by == [F::ZERO; 2] {
                (bike_at, bike_centre)
            } else {
                ([bike_at[0], bike_at[1], bike_at[2], crate::seam::moved(bike_at[3], by)], crate::seam::moved(bike_centre, by))
            };
            let mut objects = vec![
                Collidable {
                    class: Class::Object,
                    flags: flags::ENABLED,
                    nearby: true,
                    radius,
                    category: flight_search::PROBE,
                    collides_with: flight_search::PROBE_COLLIDES_WITH,
                    categories: col.categories.clone(),
                    ..Collidable::default()
                },
                Collidable {
                    class: Class::Object,
                    flags: flags::ENABLED,
                    nearby: true,
                    radius: bike_radius,
                    category,
                    collides_with,
                    categories: Categories { mode: 1, list: Vec::new() },
                    ..Collidable::default()
                },
            ];
            let ray = ray_probe::Record { listed: true, capacity: tree.segments.len(), one_per_ray: true, ..ray_probe::Record::default() };
            let mut narrow = OnTheBike { rays: tree, at, mesh, bike_at, facets: &facets, ray };
            let found = vec![Candidate { object: BIKE, distance: distance(centre, bike_centre) - bike_radius }];
            world_search::search_nearby(&mut objects, PROBE, &Nearby { found, prepare: false }, &mut narrow);
            if !narrow.ray.contacts.is_empty() {
                met = Some((narrow.ray.contacts, by));
                break;
            }
        }
        let Some((contacts, by)) = met else { continue };
        let back = [F::ZERO - by[0], F::ZERO - by[1]];
        let asleep = !coll.bones[k].body.enabled;
        let index = out.struck.len();
        let col = coll.objects[k].collision.as_mut().expect("checked above");
        let r = &mut col.record;
        let mut at = own_end;
        for h in &contacts {
            r.touching = true;
            r.on_object = true;
            r.touching_other = true;
            // The prop's own search's entries: ahead of the mirrors the props filed after it put
            // on its list (S258, [`own_end`]).
            if r.contacts.len() < r.capacity {
                let tail = r.contacts.split_off(at.min(r.contacts.len()));
                contact_search::add(r, 0, h.from, h.to, h.point, h.normal, NO_SURFACE, Some(me), None);
                r.contacts.extend(tail);
                at += 1;
            }
            out.mirrors.push(Contact {
                kind: 1,
                from: h.from,
                on_body: h.to,
                normal: crate::contact::negated(h.normal),
                point: if by == [F::ZERO; 2] { h.point } else { crate::seam::moved(h.point, back) },
                arm: Vf::VF0,
                other_arm: Vf::VF0,
                surface: NO_SURFACE,
                other: Some(index),
                mover: None,
            });
        }
        let collider = col.collider;
        // The probe's two listeners, the body's then the collection's.
        if asleep && category & flight_search::WAKE_MASK != 0 {
            coll.enable(k);
        }
        coll.activate();
        let bone = &coll.bones[k];
        if by != [F::ZERO; 2] {
            across.push((index, by));
        }
        out.struck.push(Pushed { collection: c, object: k, collider, body: bone.body, mass: bone.mass });
    }
    (out, across)
}

/// Where prop `(c, k)`'s own search's entries end on its list: before the mirrors at its end that
/// props filed after it put there, their rays striking it in their own searches, later in the node's
/// pass ([`crate::prop_props::strike_props`]). In hit0-11061 ball 2 (filed after ball 0) strikes
/// ball 0 and the plane's hull is struck by ball 0: ball 0's hull entry comes before ball 2's mirror.
fn own_end(set: &PropSet, c: usize, k: usize) -> usize {
    let mine = set.collections[c].objects[k].filed_at;
    let Some(r) = set.collections[c].objects[k].collision.as_ref().map(|col| &col.record) else { return 0 };
    let later = |e: &Contact| {
        let named = e.other.and_then(|o| o.checked_sub(crate::prop_props::PROP_OTHER)).and_then(|f| crate::prop_props::unflat(set, f));
        e.kind == 1 && named.is_some_and(|(sc, so)| set.collections[sc].objects[so].filed_at > mine)
    };
    r.contacts.len() - r.contacts.iter().rev().take_while(|e| later(e)).count()
}

/// One prop's collidable mesh, where its node stands it.
struct Posed<'a> {
    collection: usize,
    object: usize,
    mesh: &'a Mesh,
    to_world: Mat4,
    centre: Vf,
    radius: F,
    /// The shift that brings the wreck bike to this place: zero at the prop's own, a seam's at a copy
    /// (MD167a).
    by: [F; 2],
}

/// Where the props start in [`wreck_rays_on_props`]'s list: after the wreck bike's ray probe.
const PROPS: usize = 1;

/// The wreck bike's ray probe against the props' meshes: prop `k` is collidable `PROPS + k`.
struct OnTheProps<'a> {
    rays: &'a Rays,
    at: [Vf; 4],
    posed: &'a [Posed<'a>],
    facets: Vec<Option<Vec<Facet>>>,
    ray: ray_probe::Record,
}

impl Narrow for OnTheProps<'_> {
    fn clear(&mut self, objects: &mut [Collidable], probe: usize) {
        ray_probe::clear(&mut objects[probe].flags, &mut self.ray);
    }

    fn prepare(&mut self, _objects: &mut [Collidable], _probe: usize) {}

    fn pair(&mut self, _objects: &mut [Collidable], _probe: usize, other: usize) -> bool {
        let Some(k) = other.checked_sub(PROPS) else { return false };
        let p = &self.posed[k];
        let facets = self.facets[k].get_or_insert_with(|| flight_search::facets(p.mesh));
        let target = ray_probe::Mesh {
            id: other as u32,
            transform: p.to_world.0,
            root: 0,
            nodes: &p.mesh.nodes,
            facets,
            vertices: &p.mesh.vertices,
        };
        let probe = RayProbe { transform: self.at, root: self.rays.root, nodes: &self.rays.nodes, segments: &self.rays.segments };
        ray_probe::pair(&probe, &ray_probe::Shape::Mesh(target), &mut self.ray)
    }

    fn parts(&mut self, _objects: &mut [Collidable], _group: usize) -> usize {
        0
    }

    fn part(&mut self, _objects: &mut [Collidable], _group: usize, _i: usize) -> Option<usize> {
        None
    }

    fn heard_hit(&mut self, _objects: &mut [Collidable], _listener: usize, _probe: usize, _other: usize) {}

    fn heard_struck(&mut self, _objects: &mut [Collidable], _listener: usize, _struck: usize, _probe: usize) {}

    fn on_hit(&mut self, _objects: &mut [Collidable], _callback: usize, _probe: usize, _other: usize) {}

    fn on_struck(&mut self, _objects: &mut [Collidable], _callback: usize, _struck: usize, _probe: usize) {}
}

/// **The wreck bike's rays on the props** (S244): its ray probe ([`wreck_bike::ray_tree`]) where its
/// record's node stands it, against every prop of `set` with a collidable mesh (its `f.col`, category
/// [`prop_world::COLLIDABLE`]) where its node stands. The original's probe takes a prop's mesh: its
/// category list is empty, every category met (the hit3 states, research/s244: four of the wreck
/// bike's rays on ball 2's `f.col`). `me` is the wreck bike as the props' mirrors name it. Answers its
/// entries (kind 0, at most its record's capacity, one per ray), each naming its prop by its place in
/// [`Strike::struck`] (`first` 0), and each struck prop's mirrors, for [`PropSet::strike`].
pub fn wreck_rays_on_props(set: &PropSet, bike: &WreckBike, me: usize) -> Strike {
    wreck_rays_at(set, frame(&bike.record.node), bike.record.capacity, me, &[])
}

/// **The wreck bike's rays on the props of a world that wraps** (MD167a, MD29): [`wreck_rays_on_props`]
/// with its probe standing at `at` and a record of one a ray, where a prop by the far side of a seam
/// the rays reach across stands again one width over, by the bike, as the riding bike's strike has it
/// ([`crate::prop_contact::strike_wrapped`]): a copy's hit is its prop's, its mirrors go back to the
/// prop's own place, and [`Strike::across`] names it. On a world that does not wrap, the plain strike.
pub fn wreck_rays_wrapped(set: &PropSet, at: &Mat4, me: usize, world: &dyn crate::world::World) -> Strike {
    let rays = wreck_bike::ray_tree();
    let (centre, reach) = flight_search::reach(&rays, &at.0);
    let seams = crate::tiles::seam_shifts(world, centre, reach);
    wreck_rays_at(set, at.0, wreck_bike::rays().len(), me, &seams)
}

/// [`wreck_rays_on_props`] with the wreck bike's probe standing at `at` (its record's node's rows and
/// origin) and its record holding at most `capacity`; the props by a seam a copy `seams` over (each a
/// shift that brings the bike to the world's copy on a seam's far side), after every prop's own place.
fn wreck_rays_at(set: &PropSet, at: [Vf; 4], capacity: usize, me: usize, seams: &[[F; 2]]) -> Strike {
    let mut posed: Vec<Posed<'_>> = set
        .collections
        .iter()
        .zip(set.shapes.iter())
        .enumerate()
        .flat_map(|(c, (coll, shapes))| {
            coll.objects.iter().zip(shapes).enumerate().filter_map(move |(o, (object, mesh))| {
                let mesh = mesh.as_ref().filter(|_| object.collision.is_some())?;
                let (to_world, centre, radius) = Prop::posed(mesh, &PropSet::matrix(&object.node));
                Some(Posed { collection: c, object: o, mesh, to_world, centre, radius, by: [F::ZERO; 2] })
            })
        })
        .collect();
    let rays = wreck_bike::ray_tree();
    let (centre, reach) = flight_search::reach(&rays, &at);
    let mut copies = Vec::new();
    for &by in seams {
        let back = [F::ZERO - by[0], F::ZERO - by[1]];
        for p in &posed {
            let moved = crate::seam::moved(p.centre, back);
            if distance(centre, moved) - p.radius <= reach {
                let mut to_world = p.to_world;
                crate::seam::matrix(&mut to_world, back);
                copies.push(Posed { centre: moved, to_world, by, ..*p });
            }
        }
    }
    posed.extend(copies);
    if posed.is_empty() {
        return Strike::default();
    }
    let mut objects = vec![Collidable {
        class: Class::Object,
        flags: flags::ENABLED,
        nearby: true,
        radius: reach,
        category: flight_search::PROBE,
        collides_with: flight_search::PROBE_COLLIDES_WITH,
        categories: Categories { mode: 1, list: Vec::new() },
        ..Collidable::default()
    }];
    objects.extend(posed.iter().map(|p| Collidable {
        class: Class::Object,
        flags: flags::ENABLED,
        nearby: true,
        radius: p.radius,
        category: prop_world::COLLIDABLE,
        collides_with: prop_world::COLLIDES_WITH,
        categories: Categories { mode: 1, list: Vec::new() },
        ..Collidable::default()
    }));
    let ray = ray_probe::Record { listed: true, capacity, one_per_ray: true, ..ray_probe::Record::default() };
    let mut narrow = OnTheProps { rays: &rays, at, posed: &posed, facets: (0..posed.len()).map(|_| None).collect(), ray };
    let found = posed
        .iter()
        .enumerate()
        .map(|(k, p)| Candidate { object: PROPS + k, distance: distance(centre, p.centre) - p.radius })
        .collect();
    world_search::search_nearby(&mut objects, PROBE, &Nearby { found, prepare: false }, &mut narrow);
    let mut out = Strike::default();
    let mut hit: Vec<usize> = Vec::new();
    // A copy one width over is its prop's (MD167a).
    let body = |k: usize| (posed[k].collection, posed[k].object);
    for h in &narrow.ray.contacts {
        let k = h.object as usize - PROPS;
        let index = hit.iter().position(|&s| body(s) == body(k)).unwrap_or_else(|| {
            hit.push(k);
            let p = &posed[k];
            out.struck.push((p.collection, p.object, Vec::new()));
            hit.len() - 1
        });
        let mut entries = Record { capacity, contacts: std::mem::take(&mut out.contacts), ..Record::default() };
        contact_search::add(&mut entries, 0, h.from, h.to, h.point, h.normal, NO_SURFACE, Some(index), None);
        out.contacts = entries.contacts;
        let p = &posed[k];
        let capacity = set.collections[p.collection].objects[p.object].collision.as_ref().map_or(0, |c| c.record.capacity);
        let mut mirrors = Record { capacity, contacts: std::mem::take(&mut out.struck[index].2), ..Record::default() };
        contact_search::add(&mut mirrors, 1, h.from, h.to, h.point, crate::contact::negated(h.normal), NO_SURFACE, Some(me), None);
        out.struck[index].2 = mirrors.contacts;
    }
    // A copy's mirrors go back to its prop's place, where the prop's step resolves them.
    for (index, &k) in hit.iter().enumerate() {
        let by = posed[k].by;
        if by != [F::ZERO; 2] {
            for e in &mut out.struck[index].2 {
                e.point = crate::seam::moved(e.point, by);
            }
            out.across.push((index, by));
        }
    }
    out
}

/// **Both ways at once** (S244), in the original's order on the props node: the wreck bike's rays on
/// the props ([`wreck_rays_on_props`]: each struck prop woken, the mirrors at the head of its list,
/// [`PropSet::strike`]), then the props' rays on the wreck bike ([`struck_by_props`]). Answers the
/// wreck bike's entries and mirrors and every struck body for its frame; `me` as in
/// [`struck_by_props`].
pub fn strikes(set: &mut PropSet, mesh: &Mesh, bike: &WreckBike, me: usize) -> WreckStrike {
    let strike = wreck_rays_on_props(set, bike, me);
    let pushed = set.strike(&strike);
    joined(strike.contacts, pushed, struck_by_props(set, mesh, bike, me))
}

/// **The wreck bikes' own rays on the props' node** (S260a), the first half: the original's node lists
/// the wreck bikes' objects ahead of the props', so each wreck bike searches first. Its rays on the
/// props' meshes ([`wreck_rays_on_props`], where its hull stands, its record's capacity one a ray),
/// each struck prop woken as a strike wakes it ([`PropSet::struck`]) before the props search, so a
/// woken one searches in the same pass. Called by [`PropSet::search_among`] and
/// [`PropSet::search_in_among`] before their searches; [`wreck_rays_filed`] after.
pub(crate) fn wreck_rays_first(set: &mut PropSet, wrecks: &[Wreck<'_>]) -> Vec<(usize, Strike)> {
    let capacity = wreck_bike::rays().len();
    let rayed = wrecks.iter().map(|w| (w.side, wreck_rays_at(set, w.hull.at.0, capacity, w.side, &[]))).collect();
    woken(set, rayed)
}

/// [`wreck_rays_first`] in `world`: on one that wraps, each wreck bike's rays meet the props by a
/// seam's far side one width over too ([`wreck_rays_wrapped`], MD167a).
pub(crate) fn wreck_rays_first_in(set: &mut PropSet, wrecks: &[Wreck<'_>], world: &dyn crate::world::World) -> Vec<(usize, Strike)> {
    let rayed = wrecks.iter().map(|w| (w.side, wreck_rays_wrapped(set, &w.hull.at, w.side, world))).collect();
    woken(set, rayed)
}

/// Each prop the wreck bikes' rays struck woken as a strike wakes it, in their order.
fn woken(set: &mut PropSet, rayed: Vec<(usize, Strike)>) -> Vec<(usize, Strike)> {
    for (c, o, _) in rayed.iter().flat_map(|(_, s)| &s.struck) {
        set.struck(*c, *o);
    }
    rayed
}

/// The second half of [`wreck_rays_first`], after the props' searches: each wreck bike's mirrors at
/// the head of the props' lists, ahead of their own searches' entries (the first wreck bike's first,
/// so put on last), and each `by_props` (what the props' rays struck on it) joined with its own
/// entries and the bodies they struck, as [`strikes`] joins them.
/// A body struck across a seam goes to the wreck bike's frame a width back, by the bike, noted as its
/// side's ([`PropSet::strike_as`], MD167a).
/// The hulls after the wreck bikes' (the parked cars', S486b) cast no rays: what the props' rays
/// struck on them follows as it is.
pub(crate) fn wreck_rays_filed(set: &mut PropSet, rayed: Vec<(usize, Strike)>, mut by_props: Vec<WreckStrike>) -> Vec<WreckStrike> {
    let mut pushed: Vec<Vec<Pushed>> = rayed.iter().rev().map(|(side, s)| set.strike_as(s, *side)).collect();
    pushed.reverse();
    let unrayed = by_props.split_off(rayed.len().min(by_props.len()));
    let mut out: Vec<WreckStrike> = rayed.into_iter().zip(pushed).zip(by_props).map(|(((_, s), pushed), by)| joined(s.contacts, pushed, by)).collect();
    out.extend(unrayed);
    out
}

/// The wreck bike's strike on the props ([`wreck_rays_on_props`], put on the props by
/// [`PropSet::strike`]) and the props' on it ([`struck_on`]) as one [`WreckStrike`]: the bodies the
/// bike's rays struck first, in their order, then the others the props' rays name; each mirror's
/// index moved to its body's place.
fn joined(entries: Vec<Contact>, mut struck: Vec<Pushed>, by_props: WreckStrike) -> WreckStrike {
    let WreckStrike { mut mirrors, struck: theirs, .. } = by_props;
    let places: Vec<usize> = theirs
        .into_iter()
        .map(|p| {
            struck.iter().position(|q| (q.collection, q.object) == (p.collection, p.object)).unwrap_or_else(|| {
                struck.push(p);
                struck.len() - 1
            })
        })
        .collect();
    for m in &mut mirrors {
        m.other = m.other.map(|o| places[o]);
    }
    WreckStrike { mirrors, struck, entries }
}

/// A wreck bike on the props' node ([`crate::Sim::wreck_hulls`]): its player, its side to the props'
/// step, its mesh, where its record stands it, and its body's velocity.
pub(crate) type Hull = (usize, usize, std::sync::Arc<crate::ragdoll_scene::Bike>, Mat4, Vf);

/// `hulls` as the props' searches take them.
pub(crate) fn hulls_on_node(hulls: &[Hull]) -> Vec<Wreck<'_>> {
    hulls.iter().map(|(_, side, mesh, at, velocity)| Wreck { hull: flight_search::Plane { mesh: &mesh.mesh, at: *at, velocity: *velocity }, side: *side }).collect()
}

/// **The parked cars' hulls on the props' node** (S486b): each car's hull where its body's node
/// stands it ([`crate::car::Parked::hull`]) and its body's velocity, after the wreck bikes' hulls, so
/// a woken prop's rays strike it as they strike a wreck bike's. Car `k` is the other side `first + k`
/// of the props' step: after its vehicles and the wreck bikes' bodies ([`crate::Sim::props_step`]).
/// They cast no rays of their own there ([`PropSet::parked`]): a car's collision object is on the
/// riders' node.
pub(crate) fn parked_on_node(cars: &[crate::car::Parked], first: usize) -> Vec<Wreck<'_>> {
    cars.iter()
        .enumerate()
        .map(|(k, p)| {
            let h = p.hull(first + k);
            Wreck { hull: flight_search::Plane { mesh: h.mesh, at: h.at, velocity: p.machine.body.velocity }, side: first + k }
        })
        .collect()
}

/// Each parked car the props' rays struck (`strikes`, in [`crate::Sim::parked_cars`]' order) woken:
/// the struck listener of its collision object, which the props' mirrors name (S486b).
pub(crate) fn wake_parked(cars: &mut [crate::car::Parked], strikes: &[WreckStrike]) {
    for (p, s) in cars.iter_mut().zip(strikes) {
        if !s.mirrors.is_empty() {
            p.struck();
        }
    }
}

/// **A late prop's strike on a wreck bike after the pass's** (S384): the prop searched at the walk's
/// end, so its mirrors go on the bike's list after every mirror the pass put there, each naming its
/// body by its place in `into`'s struck bodies (one already there kept, a new one put at the end).
pub fn late_struck(into: &mut WreckStrike, late: WreckStrike) {
    let WreckStrike { mirrors, struck, entries } = late;
    let places: Vec<usize> = struck
        .into_iter()
        .map(|p| {
            into.struck.iter().position(|q| (q.collection, q.object) == (p.collection, p.object)).unwrap_or_else(|| {
                into.struck.push(p);
                into.struck.len() - 1
            })
        })
        .collect();
    into.mirrors.extend(mirrors.into_iter().map(|m| Contact { other: m.other.map(|o| places[o]), ..m }));
    into.entries.extend(entries);
}

impl WreckBike {
    /// [`WreckBike::tick_struck`] in a world with objects in it, as [`WreckBike::tick_among`] searches
    /// it.
    pub fn tick_among_struck(&mut self, dt: F, ground: Option<&dyn contact_search::Heightfield>, props: Option<&crate::world::Props>, trees: Option<&crate::eco::Trees>, struck: &mut WreckStrike) {
        let rays = wreck_bike::ray_tree();
        let bounds = ground.map(|f| {
            let (lo, hi) = (f.min(), f.max());
            ([F(lo.x()), F(lo.z())], [F(hi.x()), F(hi.z())])
        });
        let mut nearby = flight_search::search();
        let mut scene = flight_search::Scene::flown(props, trees, None, bounds, &rays, &self.record.node, &mut nearby);
        self.tick_struck(dt, ground, &mut scene, struck);
    }
}

impl crate::Sim {
    /// **Player `i`'s wreck bike struck by the props' rays** this tick ([`struck_by_props`]), kept on
    /// the bike for its frame ([`WreckBike::struck`]). The props' entries name it as the other side
    /// `players + i` of their step ([`crate::Sim::props_step`], [`crate::Sim::wreck_sides`]). Nothing
    /// for a rider who is up, a part whose collision is off, a ride with no ragdoll kit (the mesh is
    /// the kit's) or no props.
    pub fn wreck_strike(&mut self, i: usize) {
        self.wreck_strike_as(i, self.players() + i);
    }

    /// [`crate::Sim::wreck_strike`] with the wreck bike named `side` on the props' entries: a seat's
    /// on the seats' shared props is `seats + seat` (S197c, [`crate::prop_seats`]).
    pub(crate) fn wreck_strike_as(&mut self, i: usize, side: usize) {
        let props = &mut self.traffic.props;
        if props.is_empty() {
            return;
        }
        let rider = if i == 0 { &mut self.rider } else { &mut self.others[i - 1].rider };
        if !rider.wreckage.as_ref().is_some_and(|w| w.part.collision_enabled) {
            return;
        }
        let (Some(crash), Some(kit)) = (rider.crash.as_mut(), rider.ragdoll_kit.as_ref()) else { return };
        let Some(mesh) = kit.bike_mesh.as_deref() else { return };
        crash.bike.struck = strikes(props, &mesh.mesh, &crash.bike, side);
    }

    /// **What player `i`'s wreck bike pushed in its frame**, written back (S244): the props its rays
    /// struck ([`wreck_rays_on_props`]) took its impulse in its world's step, which runs before the
    /// props' (the original's props node steps the wreck bike's world, then the balls' collection
    /// worlds). Then the bike's strike is spent. Called after the rider's frame, before
    /// [`crate::Sim::props_step`].
    pub fn wreck_pushed_back(&mut self, i: usize) {
        let rider = if i == 0 { &mut self.rider } else { &mut self.others[i - 1].rider };
        let Some(crash) = rider.crash.as_mut() else { return };
        let struck = std::mem::take(&mut crash.bike.struck);
        self.traffic.props.pushed(&struck.struck);
    }

    /// **The props' collide pass with the wreck bikes on the node** (S243,
    /// [`crate::traffic::Traffic::frame_held_among`]): each of `players` — a player and the other side
    /// its wreck bike is to the props' step — who is down with a ragdoll kit's mesh and a part whose
    /// collision is on has its wreck bike's hull in every prop's world where its record stands it,
    /// and what the props' rays struck on it kept on the bike for its frame ([`WreckBike::struck`]),
    /// in place of [`crate::Sim::wreck_strike`]'s search after the pass.
    ///
    /// The parked cars' hulls follow the wreck bikes' (S486b, [`parked_on_node`]): one a prop's rays
    /// strike is woken, and the props' step pushes its body.
    pub(crate) fn frame_held_wrecked(&mut self, dt: F, world: &mut dyn crate::world::World, players: &[(usize, usize)]) -> crate::prop_set::PropMovers {
        self.hear_ridden();
        let kept = self.wreck_hulls(players);
        let first = 2 * self.players();
        let crate::Sim { traffic, parked_cars, .. } = self;
        let mut wrecks = hulls_on_node(&kept);
        if !traffic.props.is_empty() {
            wrecks.extend(parked_on_node(parked_cars, first));
        }
        traffic.props.parked = wrecks.len() - kept.len();
        let (movers, mut strikes) = traffic.frame_held_among(dt, world, &wrecks);
        traffic.props.parked = 0;
        drop(wrecks);
        let cars = strikes.split_off(kept.len().min(strikes.len()));
        wake_parked(parked_cars, &cars);
        for ((i, ..), strike) in kept.iter().zip(strikes) {
            let rider = if *i == 0 { &mut self.rider } else { &mut self.others[*i - 1].rider };
            if let Some(crash) = rider.crash.as_mut() {
                crash.bike.struck = strike;
            }
        }
        movers
    }

    /// **The ridden bikes the props' probes hear this tick** (F27a2, [`PropSet::hear_ridden`]): each
    /// player's chassis where the last tick left it, in the world while its rider is up and no ghost,
    /// and player 1's not while the plane flies (its bike is out of the world then, S95).
    pub(crate) fn hear_ridden(&mut self) {
        let flying = self.plane.is_some();
        let bikes: Vec<flight_search::Bike> = std::iter::once(&self.rider)
            .chain(self.others.iter().map(|p| &p.rider))
            .enumerate()
            .map(|(i, r)| flight_search::Bike { transform: r.ride.transform(), in_world: !(i == 0 && flying) && r.state.wreck.is_none() && !r.ghost.on })
            .collect();
        self.traffic.props.hear_ridden(&bikes);
        // The plane lane's props' step has the plane for its one striker.
        self.traffic.props.file_ridden(!flying);
    }

    /// Each of `players` (a player and its side, as [`crate::Sim::frame_held_wrecked`] takes them)
    /// whose wreck bike is a hull on the props' node ([`crate::Sim::wreck_hull`]), in their order;
    /// none on a track with no props.
    pub(crate) fn wreck_hulls(&self, players: &[(usize, usize)]) -> Vec<Hull> {
        if self.traffic.props.is_empty() {
            return Vec::new();
        }
        players.iter().filter_map(|&(i, side)| self.wreck_hull(i).map(|(mesh, at, velocity)| (i, side, mesh, at, velocity))).collect()
    }

    /// **What a late prop's rays struck on the wreck bikes** (S384, [`PropSet::strike_in_among`]):
    /// each of `hulls`' strike put after what the pass struck on its bike ([`late_struck`]), for the
    /// bike's frame, unless that frame already ran this tick (`framed` says, by player): it took
    /// what the pass struck, and these mirrors go nowhere.
    pub(crate) fn late_struck(&mut self, hulls: &[Hull], strikes: Vec<WreckStrike>, framed: &dyn Fn(usize) -> bool) {
        for ((i, ..), late) in hulls.iter().zip(strikes) {
            if late.is_empty() || framed(*i) {
                continue;
            }
            let rider = if *i == 0 { &mut self.rider } else { &mut self.others[*i - 1].rider };
            if let Some(crash) = rider.crash.as_mut() {
                late_struck(&mut crash.bike.struck, late);
            }
        }
    }

    /// **Player `i`'s wreck bike as a hull on the props' node** (S243): its mesh, where its record's
    /// node stands it, and its body's velocity. Nothing for a rider who is up, a part whose collision
    /// is off, or a ride with no ragdoll kit's mesh.
    pub(crate) fn wreck_hull(&self, i: usize) -> Option<(std::sync::Arc<crate::ragdoll_scene::Bike>, Mat4, Vf)> {
        let rider = self.rider_of(i);
        if !rider.wreckage.as_ref().is_some_and(|w| w.part.collision_enabled) {
            return None;
        }
        let (crash, kit) = (rider.crash.as_ref()?, rider.ragdoll_kit.as_ref()?);
        let mesh = kit.bike_mesh.clone()?;
        Some((mesh, Mat4(frame(&crash.bike.record.node)), crash.bike.body.velocity))
    }

    /// Every player's wreck bike as the props' step reads it, in player order (`None` for a rider
    /// who is up): its collision object and body, copied, and its mass.
    pub fn wreck_sides(&self) -> Vec<Option<(crate::contact::Collider, crate::biplane::Body, F)>> {
        (0..self.players())
            .map(|i| self.rider_of(i).crash.as_ref().map(|c| (c.bike.collider, c.bike.body, c.bike.mass)))
            .collect()
    }

    /// [`crate::Sim::wreck_sides`] after the props' step, written back: what a prop's impulse did to
    /// a wreck bike's body, and the centre its object's resolve wrote.
    pub fn wrecks_pushed(&mut self, sides: &[Option<(crate::contact::Collider, crate::biplane::Body, F)>]) {
        for (i, side) in sides.iter().enumerate() {
            let rider = if i == 0 { &mut self.rider } else { &mut self.others[i - 1].rider };
            if let (Some((collider, body, _)), Some(crash)) = (side, rider.crash.as_mut()) {
                crash.bike.collider = *collider;
                crash.bike.body = *body;
            }
        }
    }
}
