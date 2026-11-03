//! **Props by props** (S242): a knockable prop's own ray probe against another prop's collidable mesh
//! (its model's `<node>f.col`, [`PropSet::shapes`]) where that prop's node stands it — free09's
//! beach ball on another — as entries on the striker's list naming the struck prop, and the mirrors on
//! the struck prop's.
//!
//! The original files every woken prop's collision object on free09's props node, and the node's
//! update empties every filed list, then searches each object in list order, then steps (research/s242,
//! the drop2 states: ball 1 dropped onto ball 0 at rest). An awake ball's ray probe searches with
//! every category, and a ball's mesh collidable is [`prop_world::COLLIDABLE`] wherever it stands —
//! the moving tree awake, the static tree at rest — so in a tick where one ball meets another:
//!
//! 1. the striker's rays, after the terrain's, meet the other ball's mesh: an entry (kind 0) on the
//!    striker's list naming the struck ball's object, its mirror (kind 1, the normal turned round) on
//!    the struck ball's — at the head of its list when the struck ball is later in the node's list
//!    (its own search has not run yet: the node emptied its list), after its own entries when it is
//!    earlier;
//! 2. the struck listeners wake the struck ball, its body enabled then its collection activated
//!    ([`PropSet::struck`]); a ball at rest is filed by it at the end of the node's list and searched
//!    in the same pass — its own rays strike the striker back;
//! 3. the collections step: each kind-0 entry's first-substep impulse goes into both bodies, a
//!    mirror pushes its own body alone ([`crate::contact::resolve`]).
//!
//! In the port the node's list order is [`Object::filed_at`] ([`PropSet::stamp_filed`]); every
//! collection's own search runs first ([`PropSet::search`]), then [`strike_props`] walks the filed
//! objects in the node's order; an entry naming another prop carries [`PROP_OTHER`] plus the prop's
//! place among every prop ([`flat`]), and the step hands each collection the props its list names
//! as copies after its strikers ([`step_named`]). The drop2 states are the proof:
//! `tests/prop_props.rs`.
//!
//! What is not here: a body of the striker's own collection (each free09 ball is a collection of its
//! own); the original's one narrow pass ordering a scene mesh and a prop by the walk's list and keeping
//! one contact per ray between them — a prop's entries go after its scene's; the broad phase's gather,
//! which is every prop with a mesh by its distance from the rays' centre.

#![deny(clippy::float_arithmetic)]

use crate::contact::{Contact, Node, Other, Side};
use crate::contact_search::{self, NO_SURFACE};
use crate::flight_search::{self, Rays};
use crate::prop_contact::Pushed;
use crate::prop_set::PropSet;
use crate::prop_world::{self, Collection, Object};
use crate::ray_probe::{self, Facet, RayProbe};
use crate::ragdoll::Tables;
use crate::track::Mesh;
use crate::world::Prop;
use crate::world_search::{self, Candidate, Categories, Class, Collidable, Narrow, Nearby, flags};
use ps2_float::{F, Vf, fpu};

/// A contact naming prop `f` (its [`flat`] place) on its other side carries `PROP_OTHER + f`, far past
/// any striker a step is handed; the step names it again by where it hands the prop ([`step_named`]).
pub const PROP_OTHER: usize = 1 << 24;

/// Prop `(collection, object)`'s place among every prop, collection by collection in object order.
pub fn flat(set: &PropSet, collection: usize, object: usize) -> usize {
    set.collections[..collection].iter().map(|c| c.objects.len()).sum::<usize>() + object
}

/// The prop at a [`flat`] place.
pub fn unflat(set: &PropSet, mut f: usize) -> Option<(usize, usize)> {
    for (c, coll) in set.collections.iter().enumerate() {
        if f < coll.objects.len() {
            return Some((c, f));
        }
        f -= coll.objects.len();
    }
    None
}

impl PropSet {
    /// Every filed object not stamped yet gets its place at the end of the node's list
    /// ([`Object::filed_at`]), in collection and object order: a stand-in for the order the wakes
    /// filed them in, which the node's list keeps and the port does not.
    pub fn stamp_filed(&mut self) {
        let mut last = self.collections.iter().flat_map(|c| c.objects.iter()).map(|o| o.filed_at).max().unwrap_or(0);
        for c in &mut self.collections {
            for o in c.objects.iter_mut().filter(|o| o.filed && o.filed_at == 0) {
                last += 1;
                o.filed_at = last;
            }
        }
    }

    /// The filed objects in the node's list order.
    pub fn node_order(&self) -> Vec<(usize, usize)> {
        let mut order: Vec<(u32, usize, usize)> = Vec::new();
        for (c, coll) in self.collections.iter().enumerate() {
            for (k, o) in coll.objects.iter().enumerate().filter(|(_, o)| o.filed) {
                order.push((o.filed_at, c, k));
            }
        }
        order.sort_unstable();
        order.into_iter().map(|(_, c, k)| (c, k)).collect()
    }
}

/// One prop's collidable mesh where its node stands it.
struct Posed<'a> {
    collection: usize,
    object: usize,
    flat: usize,
    mesh: &'a Mesh,
    to_world: [Vf; 4],
    centre: Vf,
    radius: F,
}

/// The probe's place in the search's list; the props follow it.
const PROBE: usize = 0;
const PROPS: usize = 1;

/// A prop's ray probe against the other props' meshes: prop `k` of `posed` is collidable `PROPS + k`.
struct OnTheProps<'a> {
    rays: &'a Rays,
    at: [Vf; 4],
    posed: &'a [Posed<'a>],
    facets: &'a mut [Option<Vec<Facet>>],
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
            transform: p.to_world,
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

/// What one striker's rays met: each struck prop's `(collection, object, flat)`, and the ray contacts
/// on it, in the order the rays first met it.
type Met = Vec<((usize, usize, usize), Vec<ray_probe::Contact>)>;

/// Object `k` of collection `c`'s ray probe against every other collection's props with a mesh.
fn strikes_of(set: &PropSet, c: usize, k: usize, facets: &mut [Option<Vec<Facet>>]) -> Met {
    let coll = &set.collections[c];
    let o = &coll.objects[k];
    let (Some(Some(tree)), Some(col)) = (set.rays[c].get(k), o.collision.as_ref()) else { return Vec::new() };
    let mut posed = Vec::new();
    let mut f = 0;
    for (pc, (other, shapes)) in set.collections.iter().zip(set.shapes.iter()).enumerate() {
        for (po, (object, mesh)) in other.objects.iter().zip(shapes).enumerate() {
            let here = f;
            f += 1;
            if pc == c || !object.collision_enabled || object.collision.is_none() {
                continue;
            }
            let Some(mesh) = mesh.as_ref() else { continue };
            let (to_world, centre, radius) = Prop::posed(mesh, &PropSet::matrix(&object.node));
            posed.push(Posed { collection: pc, object: po, flat: here, mesh, to_world: to_world.0, centre, radius });
        }
    }
    if posed.is_empty() {
        return Vec::new();
    }
    let at = frame(&o.node);
    let (centre, radius) = flight_search::reach(tree, &at);
    let mut objects = vec![Collidable {
        class: Class::Object,
        flags: flags::ENABLED,
        nearby: true,
        radius,
        category: flight_search::PROBE,
        collides_with: flight_search::PROBE_COLLIDES_WITH,
        categories: col.categories.clone(),
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
    let found = posed
        .iter()
        .enumerate()
        .map(|(i, p)| Candidate { object: PROPS + i, distance: distance(centre, p.centre) - p.radius })
        .collect();
    let mut cache: Vec<Option<Vec<Facet>>> = posed.iter().map(|p| facets[p.flat].take()).collect();
    let ray = ray_probe::Record { listed: true, capacity: tree.segments.len(), one_per_ray: true, ..ray_probe::Record::default() };
    let mut narrow = OnTheProps { rays: tree, at, posed: &posed, facets: &mut cache, ray };
    world_search::search_nearby(&mut objects, PROBE, &Nearby { found, prepare: false }, &mut narrow);
    let contacts = std::mem::take(&mut narrow.ray.contacts);
    for (p, kept) in posed.iter().zip(cache) {
        facets[p.flat] = kept;
    }
    let mut met: Met = Vec::new();
    for h in contacts {
        let p = &posed[h.object as usize - PROPS];
        let key = (p.collection, p.object, p.flat);
        match met.iter_mut().find(|(m, _)| *m == key) {
            Some((_, list)) => list.push(h),
            None => met.push((key, vec![h])),
        }
    }
    met
}

/// **The props' rays on the other props**, after every collection's own search: every filed object
/// in the node's list order ([`PropSet::node_order`]), its probe where its node stands, its category
/// list as its search left it, against every other collection's props. Each struck ray puts an entry
/// on the striker's list after its own (at most its capacity), naming the struck prop
/// (`PROP_OTHER + flat`), the striker's flags set as a hit sets them and its listeners told as its
/// search's are; the struck prop is woken ([`PropSet::struck`]) and gets the mirror, at the head of
/// its list if it is later in the node's list, at the end if earlier. A struck prop that was not filed
/// is filed at the end, searched at once by `own` (its collection, its object: the terrain and the
/// scene, as [`Collection::search_object`]) and walked in its turn.
pub fn strike_props(set: &mut PropSet, own: &mut dyn FnMut(&mut PropSet, usize, usize)) {
    if set.tables.is_none() || set.collections.len() < 2 {
        return;
    }
    let total: usize = set.collections.iter().map(|c| c.objects.len()).sum();
    let mut facets: Vec<Option<Vec<Facet>>> = (0..total).map(|_| None).collect();
    // How many mirrors this pass put at the head of each list.
    let mut heads = vec![0usize; total];
    let mut order = set.node_order();
    let mut i = 0;
    while i < order.len() {
        let (c, k) = order[i];
        i += 1;
        let me = flat(set, c, k);
        for ((sc, so, sf), hits) in strikes_of(set, c, k, &mut facets) {
            let coll = &mut set.collections[c];
            let asleep = !coll.bones[k].body.enabled;
            let r = &mut coll.objects[k].collision.as_mut().expect("a striker has a collision object").record;
            for h in &hits {
                r.touching = true;
                r.on_object = true;
                r.touching_other = true;
                contact_search::add(r, 0, h.from, h.to, h.point, h.normal, NO_SURFACE, Some(PROP_OTHER + sf), None);
            }
            // The probe's two listeners, the body's then the collection's.
            if asleep && prop_world::COLLIDABLE & flight_search::WAKE_MASK != 0 {
                coll.enable(k);
            }
            coll.activate();
            set.file_on(c);
            let mirrors: Vec<Contact> = hits
                .iter()
                .map(|h| Contact {
                    kind: 1,
                    from: h.from,
                    on_body: h.to,
                    normal: crate::contact::negated(h.normal),
                    point: h.point,
                    arm: Vf::VF0,
                    other_arm: Vf::VF0,
                    surface: NO_SURFACE,
                    other: Some(PROP_OTHER + me),
                    mover: None,
                })
                .collect();
            let was_filed = set.collections[sc].objects[so].filed;
            set.struck(sc, so);
            let mine = set.collections[c].objects[k].filed_at;
            let head = if was_filed {
                set.collections[sc].objects[so].filed_at > mine
            } else {
                set.stamp_filed();
                order.push((sc, so));
                own(set, sc, so);
                heads[sf] = 0;
                true
            };
            let Some(col) = set.collections[sc].objects[so].collision.as_mut() else { continue };
            let r = &mut col.record;
            if head {
                let at = heads[sf].min(r.contacts.len());
                let n = mirrors.len();
                r.contacts.splice(at..at, mirrors);
                r.contacts.truncate(r.capacity);
                heads[sf] = at + n;
            } else {
                for m in mirrors {
                    if r.contacts.len() < r.capacity {
                        r.contacts.push(m);
                    }
                }
            }
            r.touching = true;
            r.on_object = true;
            r.touching_other = true;
        }
    }
}

/// Whether a list names another prop.
fn names_a_prop(c: &Collection) -> bool {
    c.objects.iter().filter_map(|o: &Object| o.collision.as_ref()).any(|col| col.record.contacts.iter().any(|e| e.other.is_some_and(|o| o >= PROP_OTHER)))
}

/// **Collection `c`'s step** with the props its lists name ([`strike_props`]) handed after
/// `strikers`, as the step finds them, each written back after: what its impulse did to the body and
/// the centre its object's resolve wrote. A list naming no prop steps as before. The listeners on the
/// node are not run: the set walks them after every collection's step ([`PropSet::walk_listeners`]).
pub fn step_named(set: &mut PropSet, c: usize, dt: F, tables: &Tables, movers: &[crate::contact::Mover], strikers: &mut [Other<'_>]) {
    if !names_a_prop(&set.collections[c]) {
        set.collections[c].step_world_among(dt, tables, movers, strikers);
        return;
    }
    // The named props, in the order the lists first name them, and each named entry's place.
    let mut named: Vec<usize> = Vec::new();
    for o in &set.collections[c].objects {
        for e in o.collision.iter().flat_map(|col| &col.record.contacts) {
            if let Some(f) = e.other.and_then(|o| o.checked_sub(PROP_OTHER)) {
                if !named.contains(&f) {
                    named.push(f);
                }
            }
        }
    }
    let mut pushed: Vec<Pushed> = named
        .iter()
        .map(|&f| {
            let (pc, po) = unflat(set, f).expect("a named prop is in the set");
            let coll = &set.collections[pc];
            let bone = &coll.bones[po];
            let collider = coll.objects[po].collision.as_ref().map(|col| col.collider).unwrap_or_default();
            Pushed { collection: pc, object: po, collider, body: bone.body, mass: bone.mass }
        })
        .collect();
    let first = strikers.len();
    // Which entries named a prop, to name them back after the step.
    let marked: Vec<Vec<bool>> = set.collections[c]
        .objects
        .iter()
        .map(|o| o.collision.iter().flat_map(|col| &col.record.contacts).map(|e| e.other.is_some_and(|x| x >= PROP_OTHER)).collect())
        .collect();
    for o in &mut set.collections[c].objects {
        for e in o.collision.iter_mut().flat_map(|col| col.record.contacts.iter_mut()) {
            if let Some(f) = e.other.and_then(|x| x.checked_sub(PROP_OTHER)) {
                e.other = Some(first + named.iter().position(|&n| n == f).expect("named above"));
            }
        }
    }
    {
        let mut bodies: Vec<_> = pushed.iter_mut().map(|p| p.weighed()).collect();
        let mut others: Vec<Other<'_>> = strikers
            .iter_mut()
            .map(|s| Other {
                collider: &mut *s.collider,
                side: match &mut s.side {
                    Side::Body(b) => Side::Body(&mut **b),
                    Side::Vehicle(v) => Side::Vehicle(&mut **v),
                },
            })
            .chain(bodies.iter_mut().map(|(collider, body)| Other { collider: &mut **collider, side: Side::Body(body) }))
            .collect();
        set.collections[c].step_world_among(dt, tables, movers, &mut others);
    }
    // The step may have emptied a list (a body put back); name back only what is still there.
    for (o, marks) in set.collections[c].objects.iter_mut().zip(&marked) {
        for (e, &m) in o.collision.iter_mut().flat_map(|col| col.record.contacts.iter_mut()).zip(marks) {
            if m {
                e.other = e.other.map(|x| PROP_OTHER + named[x - first]);
            }
        }
    }
    set.pushed(&pushed);
}
