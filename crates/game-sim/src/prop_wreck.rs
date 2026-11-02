//! **The props' rays strike the wreck bike** (S197b): a knockable prop's own ray probe against the
//! wreck bike's collision mesh (`bike\col\wreckbikef.col`) where the bike's record stands it, as
//! entries on the prop's collision record — the wreck bike named on the other side — and the mirrors
//! on the wreck bike's.
//!
//! The original files the wreck bike's collision object on free09's props node with the balls' (the
//! hit2 states, research/s197b: the node's list holds the bikes' objects, the wreck bike's among them,
//! then the woken balls'). The node's update empties every list and searches every object in list
//! order, then steps the node's world, which holds the wreck bike's body alone; the balls' collection
//! worlds step after. So in a tick where a ball meets the wreck bike:
//!
//! 1. the wreck bike searches first: the terrain, the scene. Its ray probe's category list leaves the
//!    props out (0x8 is in it, and a prop's mesh is 0x18), so its rays never strike a ball;
//! 2. the ball searches: its terrain rays, then its ray probe meets the wreck bike's mesh (category
//!    [`wreck_bike::COLLIDABLE`], in any of the prop's category lists, the wake mask's alone too): an
//!    entry (kind 0) on the ball's list naming the wreck bike, its mirror (kind 1, the normal turned
//!    round, the ball on its other side) on the wreck bike's, after its own entries;
//! 3. the wreck bike's world steps: its force listener resolves its list, the mirrors against the
//!    ball's body as the frame found it — a mirror takes no impulse, so the spring or the friction on
//!    the bike alone;
//! 4. the ball's collection steps: its force listener resolves its list, the first substep's impulse
//!    on an entry naming the wreck bike going into both bodies.
//!
//! In the port the wreck bike is on the props' node too (S243, [`crate::Sim::frame_held_wrecked`]):
//! its hull is a collidable in every prop's world ([`crate::prop_world::Wreck`],
//! [`crate::flight_search::Scene::probed_among`]), gathered into the prop's kept nearby list with the
//! rest and met by the prop's one ray record, so a ray keeps one contact, the nearer, between the scene
//! and the bike. The pass hands the mirrors and the struck bodies to the wreck bike's frame
//! ([`WreckBike::tick_struck`], through [`crate::ride::Crash::struck`]); the props' step reads the
//! wreck bike as its other side ([`crate::Sim::props_step`]). The hit2 states are the proof:
//! `tests/prop_wreck.rs`.
//!
//! [`struck_by_props`] is S197b's stand-in, a search of the wreck bike after the pass with the entries
//! put after everything the prop's own search found; the seats' shared props still take it
//! ([`crate::prop_seats`]).
//!
//! The sweep probes meet neither way: the wreck bike's and a prop's sweep both exclude 0x8 (and
//! 0x800), which both meshes carry ([`crate::prop_world::sweep_categories`]).
//!
//! What is not here: the rider's ragdoll and the plane against a prop; a prop's rays on another prop's
//! mesh or on a riding bike's.

#![deny(clippy::float_arithmetic)]

use crate::contact::{Contact, Node};
use crate::contact_search::{self, NO_SURFACE};
use crate::flight_search::{self, Rays};
use crate::prop_contact::Pushed;
use crate::prop_set::PropSet;
use crate::prop_world::Wreck;
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
}

impl WreckStrike {
    pub fn is_empty(&self) -> bool {
        self.mirrors.is_empty()
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
/// `wreckbikef.col`): every filed prop object with a ray probe, in collection and object order, its
/// probe where its node stands, its category list as its search left it. A prop that struck gets an
/// entry for each struck ray after its own list (at most its capacity), naming the wreck bike as its
/// other side `me` (the index its step's other sides give the bike), and its flags set as a hit sets
/// them; its listeners hear the hit as its search's do (an asleep body woken by a category of the wake
/// mask, the collection activated). Answers the mirrors and the struck bodies for the bike's frame.
pub fn struck_by_props(set: &mut PropSet, mesh: &Mesh, bike: &WreckBike, me: usize) -> WreckStrike {
    let mut out = WreckStrike::default();
    if set.tables.is_none() || mesh.nodes.is_empty() {
        return out;
    }
    let bike_at = frame(&bike.record.node);
    let (bike_centre, bike_radius) = sphere(mesh, &bike_at);
    let facets = flight_search::facets(mesh);
    let rays = std::sync::Arc::clone(&set.rays);
    for (c, coll_rays) in rays.iter().enumerate() {
        for (k, tree) in coll_rays.iter().enumerate() {
            let Some(tree) = tree else { continue };
            let coll = &mut set.collections[c];
            let o = &coll.objects[k];
            let (true, Some(col)) = (o.filed, o.collision.as_ref()) else { continue };
            let at = frame(&o.node);
            let (centre, radius) = flight_search::reach(tree, &at);
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
                    category: wreck_bike::COLLIDABLE,
                    collides_with: bike.collider.collides_with,
                    categories: Categories { mode: 1, list: Vec::new() },
                    ..Collidable::default()
                },
            ];
            let ray = ray_probe::Record { listed: true, capacity: tree.segments.len(), one_per_ray: true, ..ray_probe::Record::default() };
            let mut narrow = OnTheBike { rays: tree, at, mesh, bike_at, facets: &facets, ray };
            let found = vec![Candidate { object: BIKE, distance: distance(centre, bike_centre) - bike_radius }];
            world_search::search_nearby(&mut objects, PROBE, &Nearby { found, prepare: false }, &mut narrow);
            if narrow.ray.contacts.is_empty() {
                continue;
            }
            let asleep = !coll.bones[k].body.enabled;
            let index = out.struck.len();
            let col = coll.objects[k].collision.as_mut().expect("checked above");
            let r = &mut col.record;
            for h in &narrow.ray.contacts {
                r.touching = true;
                r.on_object = true;
                r.touching_other = true;
                contact_search::add(r, 0, h.from, h.to, h.point, h.normal, NO_SURFACE, Some(me), None);
                out.mirrors.push(Contact {
                    kind: 1,
                    from: h.from,
                    on_body: h.to,
                    normal: crate::contact::negated(h.normal),
                    point: h.point,
                    arm: Vf::VF0,
                    other_arm: Vf::VF0,
                    surface: NO_SURFACE,
                    other: Some(index),
                    mover: None,
                });
            }
            let collider = col.collider;
            // The probe's two listeners, the body's then the collection's.
            if asleep && wreck_bike::COLLIDABLE & flight_search::WAKE_MASK != 0 {
                coll.enable(k);
            }
            coll.activate();
            let bone = &coll.bones[k];
            out.struck.push(Pushed { collection: c, object: k, collider, body: bone.body, mass: bone.mass });
        }
    }
    out
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
        crash.bike.struck = struck_by_props(props, &mesh.mesh, &crash.bike, side);
    }

    /// **The props' collide pass with the wreck bikes on the node** (S243,
    /// [`crate::traffic::Traffic::frame_held_among`]): each of `players` — a player and the other side
    /// its wreck bike is to the props' step — who is down with a ragdoll kit's mesh and a part whose
    /// collision is on has its wreck bike's hull in every prop's world where its record stands it,
    /// and what the props' rays struck on it kept on the bike for its frame ([`WreckBike::struck`]),
    /// in place of [`crate::Sim::wreck_strike`]'s search after the pass.
    pub(crate) fn frame_held_wrecked(&mut self, dt: F, world: &mut dyn crate::world::World, players: &[(usize, usize)]) -> crate::prop_set::PropMovers {
        let mut kept = Vec::new();
        if !self.traffic.props.is_empty() {
            for &(i, side) in players {
                let rider = self.rider_of(i);
                if !rider.wreckage.as_ref().is_some_and(|w| w.part.collision_enabled) {
                    continue;
                }
                let (Some(crash), Some(kit)) = (rider.crash.as_ref(), rider.ragdoll_kit.as_ref()) else { continue };
                let Some(mesh) = kit.bike_mesh.clone() else { continue };
                let at = crate::math::Mat4(frame(&crash.bike.record.node));
                kept.push((i, side, mesh, at, crash.bike.body.velocity));
            }
        }
        let wrecks: Vec<Wreck<'_>> = kept
            .iter()
            .map(|(_, side, mesh, at, velocity)| Wreck { hull: flight_search::Plane { mesh: &mesh.mesh, at: *at, velocity: *velocity }, side: *side })
            .collect();
        let (movers, strikes) = self.traffic.frame_held_among(dt, world, &wrecks);
        drop(wrecks);
        for ((i, ..), strike) in kept.iter().zip(strikes) {
            let rider = if *i == 0 { &mut self.rider } else { &mut self.others[*i - 1].rider };
            if let Some(crash) = rider.crash.as_mut() {
                crash.bike.struck = strike;
            }
        }
        movers
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
