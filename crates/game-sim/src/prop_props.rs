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
//! A body of the striker's own collection is struck the same way (F21): a prop's probe ignores its own
//! mesh and nothing else, so free11's pins strike each other as balls do (research/f21, the lean
//! states: pin 1 tossed onto pin 0, one collection). The step hands such a body in place, not as a
//! copy: its collection's world integrates it, and a sibling's entries name it by its place after
//! the step's other sides ([`step_named`], [`Collection::lists`]).
//!
//! A striker whose rays meet two props or more meets them in its kept nearby list's order (S385): the
//! original gathers that list in the static tree, where every prop's `f.col` stands asleep or awake,
//! keyed where it stands (F68), so the props come in the order the query's walk reaches them, as the
//! bike's gather lists them ([`flight_search::knocks_walked`], `tests/prop_wheel_gather.rs`). The
//! port meets them in flat order first, and only when two or more are met asks the walk (the
//! [`Pass`]'s world, round where the striker's list was gathered) and searches again in its order.
//!
//! What is not here: the original's one narrow pass ordering a scene mesh and a prop by the walk's list and keeping
//! one contact per ray between them — a prop's entries go after its scene's; the kept list's
//! contents — a prop the walk does not list is still met, after those it does, in flat order.

#![deny(clippy::float_arithmetic)]

use crate::contact::{Contact, Node, Other, Side};
use crate::contact_search::{self, NO_SURFACE};
use crate::flight_search::{self, Rays};
use crate::prop_contact::Pushed;
use crate::prop_set::PropSet;
use crate::prop_world::{self, Collection, Object};
use crate::ray_probe::{self, Facet, RayProbe};
use crate::contact_search::Heightfield;
use crate::eco::Trees;
use crate::world::{Props, World};
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

/// **What the props' pass asks of its caller** ([`strike_props`]): a struck prop's own search, and the
/// order a striker's gather walks the props in.
pub trait Pass {
    /// Search object `k` of collection `c` at once: the terrain and the scene, as
    /// [`Collection::search_object`].
    fn own(&mut self, set: &mut PropSet, c: usize, k: usize);

    /// The indices into `knocks` a nearby list gathered round `centre` lists, in its walk's order
    /// ([`flight_search::knocks_walked`]), in the world near `at`; `None` where there is no world to
    /// walk (no terrain, no scene, no plants), and the props are met in flat order. The tree is laid
    /// over the terrain's box, and the props are filed after the scene's meshes and the plants, so a
    /// world of terrain alone walks the props in the same order as the whole scene's.
    fn walked(&mut self, knocks: flight_search::Knocks<'_>, at: Vf, centre: Vf, radius: F) -> Option<Vec<usize>>;
}

/// A [`Pass`] whose world is the scene's meshes `props` and the plants `trees` over the terrain
/// `field`, the same wherever the striker stands.
pub struct Among<'a, S> {
    pub field: Option<&'a dyn Heightfield>,
    pub props: Option<&'a Props>,
    pub trees: Option<&'a Trees>,
    pub own: S,
}

impl<S: FnMut(&mut PropSet, usize, usize)> Pass for Among<'_, S> {
    fn own(&mut self, set: &mut PropSet, c: usize, k: usize) {
        (self.own)(set, c, k);
    }

    fn walked(&mut self, knocks: flight_search::Knocks<'_>, _at: Vf, centre: Vf, radius: F) -> Option<Vec<usize>> {
        (self.field.is_some() || self.props.is_some() || self.trees.is_some())
            .then(|| flight_search::knocks_walked(self.props, self.trees, knocks, self.field.map(field_box), centre, radius))
    }
}

/// A [`Pass`] in a `world`: the striker's gather walks the shapes near it ([`World::wooded_near`]), or
/// the whole set where the world has one ([`World::wooded`]), and `own` searches in the world too.
pub struct InWorld<'w, S> {
    pub world: &'w mut dyn World,
    pub own: S,
}

impl<S: FnMut(&mut PropSet, &mut dyn World, usize, usize)> Pass for InWorld<'_, S> {
    fn own(&mut self, set: &mut PropSet, c: usize, k: usize) {
        (self.own)(set, &mut *self.world, c, k);
    }

    fn walked(&mut self, knocks: flight_search::Knocks<'_>, at: Vf, centre: Vf, radius: F) -> Option<Vec<usize>> {
        let (field, props, trees) = if self.world.near_set(at).is_some() { self.world.wooded_near(at) } else { self.world.wooded() };
        (field.is_some() || props.is_some() || trees.is_some())
            .then(|| flight_search::knocks_walked(props, trees, knocks, field.as_deref().map(field_box), centre, radius))
    }
}

/// A heightfield's box on the ground plane, (x, z) low and high.
fn field_box(f: &(dyn Heightfield + '_)) -> ([F; 2], [F; 2]) {
    let (lo, hi) = (f.min(), f.max());
    ([F(lo.x()), F(lo.z())], [F(hi.x()), F(hi.z())])
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

/// Object `k` of collection `c`'s ray probe against every other prop with a mesh, its own
/// collection's other bodies too: in flat order, or, where it meets two or more, again in the order
/// its gather walks them ([`Pass::walked`]), those the walk does not list after, in flat order.
fn strikes_of(set: &PropSet, c: usize, k: usize, facets: &mut [Option<Vec<Facet>>], pass: &mut dyn Pass) -> Met {
    let coll = &set.collections[c];
    let o = &coll.objects[k];
    let (Some(Some(tree)), Some(col)) = (set.rays[c].get(k), o.collision.as_ref()) else { return Vec::new() };
    let mut posed = Vec::new();
    let mut f = 0;
    for (pc, (other, shapes)) in set.collections.iter().zip(set.shapes.iter()).enumerate() {
        for (po, (object, mesh)) in other.objects.iter().zip(shapes).enumerate() {
            let here = f;
            f += 1;
            if (pc, po) == (c, k) || !object.collision_enabled || object.collision.is_none() {
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
    let mut cache: Vec<Option<Vec<Facet>>> = posed.iter().map(|p| facets[p.flat].take()).collect();
    let flat_order: Vec<usize> = (0..posed.len()).collect();
    let mut met = met_in(&mut objects, &posed, &mut cache, tree, at, centre, &flat_order);
    if met.len() > 1 {
        // Where the striker's kept list was gathered, else where its rays reach now.
        let gathered = &col.nearby;
        let round = if gathered.built { Vf::new(gathered.centre[0].0, gathered.centre[1].0, gathered.centre[2].0, 1.0) } else { centre };
        let knocks: Vec<flight_search::Knock<'_>> = posed
            .iter()
            .map(|p| {
                let other = &set.collections[p.collection];
                flight_search::Knock {
                    mesh: p.mesh,
                    to_world: crate::math::Mat4(p.to_world),
                    centre: p.centre,
                    radius: p.radius,
                    velocity: other.bones[p.object].body.velocity,
                    capacity: other.objects[p.object].collision.as_ref().map_or(0, |c| c.record.capacity),
                }
            })
            .collect();
        let knocks = flight_search::Knocks { knocks: &knocks, asleep: &[] };
        if let Some(walked) = pass.walked(knocks, o.node.origin, round, radius) {
            let mut order: Vec<usize> = walked.into_iter().filter(|&i| i < posed.len()).collect();
            order.dedup();
            order.extend(flat_order.iter().filter(|i| !order.contains(i)).collect::<Vec<_>>());
            if order != flat_order {
                met = met_in(&mut objects, &posed, &mut cache, tree, at, centre, &order);
            }
        }
    }
    for (p, kept) in posed.iter().zip(cache) {
        facets[p.flat] = kept;
    }
    met
}

/// The striker's rays at `at` (reach centred on `centre`) against `posed`, its candidates in `order`.
fn met_in(objects: &mut [Collidable], posed: &[Posed<'_>], cache: &mut [Option<Vec<Facet>>], tree: &Rays, at: [Vf; 4], centre: Vf, order: &[usize]) -> Met {
    let found = order.iter().map(|&i| Candidate { object: PROPS + i, distance: distance(centre, posed[i].centre) - posed[i].radius }).collect();
    let ray = ray_probe::Record { listed: true, capacity: tree.segments.len(), one_per_ray: true, ..ray_probe::Record::default() };
    let mut narrow = OnTheProps { rays: tree, at, posed, facets: cache, ray };
    world_search::search_nearby(objects, PROBE, &Nearby { found, prepare: false }, &mut narrow);
    let contacts = std::mem::take(&mut narrow.ray.contacts);
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
/// list as its search left it, against every other prop. Each struck ray puts an entry
/// on the striker's list after its own (at most its capacity), naming the struck prop
/// (`PROP_OTHER + flat`), the striker's flags set as a hit sets them and its listeners told as its
/// search's are; the struck prop is woken ([`PropSet::struck`]) and gets the mirror, at the head of
/// its list if it is later in the node's list, at the end if earlier. A struck prop that was not filed
/// is filed at the end, searched at once by the pass ([`Pass::own`]) and walked in its turn.
pub fn strike_props(set: &mut PropSet, pass: &mut dyn Pass) {
    let order = set.node_order();
    strike_props_from(set, order, pass);
}

/// [`strike_props`]' walk from the objects `late` filed at the node list's end, in that order (B34b):
/// a vehicle's strike put on in the props' pass ([`PropSet::strike_in`]) files them after every object
/// the pass walked, so the walk reaches them, and what their rays file, last.
pub(crate) fn strike_props_late(set: &mut PropSet, late: Vec<(usize, usize)>, pass: &mut dyn Pass) {
    strike_props_from(set, late, pass);
}

fn strike_props_from(set: &mut PropSet, mut order: Vec<(usize, usize)>, pass: &mut dyn Pass) {
    let total: usize = set.collections.iter().map(|c| c.objects.len()).sum();
    if set.tables.is_none() || total < 2 {
        return;
    }
    let mut facets: Vec<Option<Vec<Facet>>> = (0..total).map(|_| None).collect();
    // How many mirrors this pass put at the head of each list.
    let mut heads = vec![0usize; total];
    let mut i = 0;
    while i < order.len() {
        let (c, k) = order[i];
        i += 1;
        let me = flat(set, c, k);
        for ((sc, so, sf), hits) in strikes_of(set, c, k, &mut facets, pass) {
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
                pass.own(set, sc, so);
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
/// the centre its object's resolve wrote. A body of `c` itself is not a copy: its entries name it
/// by its object's place after the copies, and the collection's force listener hands it in place
/// ([`Collection::lists`]). A list naming no prop steps as before. The listeners on the
/// node are not run: the set walks them after every collection's step ([`PropSet::walk_listeners`]).
pub fn step_named(set: &mut PropSet, c: usize, dt: F, tables: &Tables, movers: &[crate::contact::Mover], strikers: &mut [Other<'_>]) {
    if !names_a_prop(&set.collections[c]) {
        set.collections[c].step_world_among(dt, tables, movers, strikers);
        return;
    }
    // The named props of the other collections, in the order the lists first name them, and each
    // named entry's place; this collection's own bodies follow them, object by object.
    let own = flat(set, c, 0);
    let mine = own..own + set.collections[c].objects.len();
    let mut named: Vec<usize> = Vec::new();
    for o in &set.collections[c].objects {
        for e in o.collision.iter().flat_map(|col| &col.record.contacts) {
            if let Some(f) = e.other.and_then(|o| o.checked_sub(PROP_OTHER)) {
                if !mine.contains(&f) && !named.contains(&f) {
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
    let siblings = first + named.len();
    // Which entries named a prop, to name them back after the step.
    let marked: Vec<Vec<bool>> = set.collections[c]
        .objects
        .iter()
        .map(|o| o.collision.iter().flat_map(|col| &col.record.contacts).map(|e| e.other.is_some_and(|x| x >= PROP_OTHER)).collect())
        .collect();
    for o in &mut set.collections[c].objects {
        for e in o.collision.iter_mut().flat_map(|col| col.record.contacts.iter_mut()) {
            if let Some(f) = e.other.and_then(|x| x.checked_sub(PROP_OTHER)) {
                e.other = Some(if mine.contains(&f) { siblings + f - own } else { first + named.iter().position(|&n| n == f).expect("named above") });
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
                e.other = e.other.map(|x| PROP_OTHER + if x >= siblings { own + x - siblings } else { named[x - first] });
            }
        }
    }
    set.pushed(&pushed);
}
