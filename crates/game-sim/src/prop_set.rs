//! **A track's knockable props in a ride** (S131): every collection the `.toy` makes
//! ([`crate::props`]'s loader), as [`crate::prop_world`]'s worlds, held in the simulation's state.
//!
//! The loader is the original's walk, in its order: the `.toy`'s `[PhysicsCollections]` claim the
//! scene's models ([`props::claim`]), each body's inertia comes off its model's box, read from the
//! model's `.psg` ([`NodeBox::from_model`]), and each collection's world is made with every body
//! asleep where its model stands ([`Collection::new`]). The world's bounds a body is put back from
//! when it leaves them (`ResetOffEdgeOfWorld`) are the terrain's extent: `0` to the map's width and
//! depth, which is the edge object every free09 prop names in the first_recording state
//! (`0..6912` on both axes, free09's three root cells of 256 samples nine feet apart).
//!
//! Each body has its collision object (S198, [`Collision`]): the part's tuning, and the model's
//! `<node>m.col` read off the disc as its ray probe (a model with no ray file — the signs — has no ray
//! probe and no sweep probe).
//!
//! Once a frame ([`PropSet::frame_among`]) the props' world node updates: every filed collision
//! object searches the terrain and the scene ([`Collection::search`]), then every collection steps its
//! world and runs its listeners ([`Collection::step`]), in the order they were made. A collection
//! nothing has woken does nothing, so a track full of props at rest costs a walk of the list.
//!
//! The state is [`PropSet::collections`]: hashed and sent with a rollback snapshot, on a track that
//! has props (an empty set is not walked or sent, so every state without props hashes and encodes as
//! before). Which scene model each body stands on ([`PropSet::models`]) and the ragdoll's tables the
//! pins solve with are constants of the track, shared by every clone.
//!
//! Each body's model's `<node>f.col` is its collidable mesh ([`PropSet::shapes`]): the riding bike's
//! body rays strike it where the body's node stands it, between the node's collide pass and its step
//! ([`PropSet::search`], [`PropSet::step`]), which wakes the body ([`PropSet::struck`]) and pushes it
//! (S197, [`crate::prop_contact`]); the other props' rays strike it too (S242, [`crate::prop_props`]).

#![deny(clippy::float_arithmetic)]

use std::sync::Arc;

use crate::biplane::Area;
use crate::contact::{Mover, Node, Other};
use crate::contact_search::Heightfield;
use crate::eco::Trees;
use crate::flight_search::{self, Rays};
use crate::hash::{State, Walk};
use crate::math::Mat4;
use crate::phy::{Ini, Part};
use crate::prop_world::{Collection, Collision, Edge, Joint, Listener, Object, Unmade, Wreck};
use crate::prop_wreck::WreckStrike;
use crate::props::{self, Member, NodeBox};
use crate::ragdoll::{Bone, Tables};
use crate::track::{self, Mesh, Placement};
use crate::world::Props;
use ps2_float::{F, Vf};

/// The track's knockable props: their worlds, and the constants they were made from.
#[derive(Clone, Default)]
pub struct PropSet {
    /// Every collection, in the order the loader made them, which is the order they tick.
    pub collections: Vec<Collection>,
    /// Each collection's bodies' scene models ([`crate::track::Scene::models`] indices), in the
    /// collection's object order. A constant of the track.
    pub models: Arc<Vec<Vec<usize>>>,
    /// The tables the pins' solve reads: built once, shared. `None` only for an empty set.
    pub tables: Option<Arc<Tables>>,
    /// Each body's ray probe tree (its model's `m.col`, `None` for a model with none), collection by
    /// collection in object order. A constant of the track.
    pub rays: Arc<Vec<Vec<Option<Rays>>>>,
    /// Each body's collidable mesh (its model's `f.col`, `None` for a model with none or one that does
    /// not parse), collection by collection in object order: what the bike's body rays strike (S197,
    /// [`crate::prop_contact`]). A constant of the track.
    pub shapes: Arc<Vec<Vec<Option<Mesh>>>>,
    /// The world node's listeners (F20): one list for every collection's, each by its collection, in
    /// the node's order — put on at the end ([`PropSet::file_listeners`]), and a finished one's slot
    /// taking the list's last ([`PropSet::walk_listeners`]). Each collection's own
    /// [`Collection::listeners`] says which of its are on.
    pub listeners: Vec<(usize, Listener)>,
    /// The ridden bikes' frame collidable mesh ([`flight_search::BIKE_COLLISION`]), which the props'
    /// probes are heard striking (F27a2, [`PropSet::hear_ridden`]); `None`, and they hear no bike. A
    /// constant of the ride, for the sound alone.
    pub bike_frame: Option<Arc<Mesh>>,
}

impl PropSet {
    /// **The ridden bikes the props' probes hear in the next collide pass** (F27a2): each player's
    /// chassis, by player, where the pass finds it ([`crate::material_hit::Ridden`]). Sound only.
    pub fn hear_ridden(&mut self, bikes: &[flight_search::Bike]) {
        for c in &mut self.collections {
            let r = &mut c.heard.1;
            r.mesh.clone_from(&self.bike_frame);
            r.bikes.clear();
            r.bikes.extend_from_slice(bikes);
        }
    }

    /// **Whether the props' probes file what they strike on the ridden bikes** (B29): their entries
    /// name the bikes by player, so this is on where the props' step has the players' chassis for its
    /// strikers, by player ([`crate::Sim::props_step`], the seats' shared step), and the bikes' strikes
    /// file the mirrors ([`crate::prop_contact::strike`]). Off, the bikes are heard only (F27a2).
    pub fn file_ridden(&mut self, on: bool) {
        for c in &mut self.collections {
            c.heard.1.file = on;
        }
    }

    /// The props of a track: `toy` its `.toy`, `track_dir` its directory as the `.pco` paths are made
    /// (`Tracks\Free09\`), `models` the scene's models. `pco` hands back a file by that path, `psg` a
    /// model's `.psg` by its file name (`sign_speedlmt.psg`); `area` is the terrain's extent, the
    /// bounds a prop that leaves them is put back from.
    ///
    /// What could not be made is said, one line each: an entry the loader does not port, a body whose
    /// model's box cannot be read (that whole collection is left out), a constraint not made.
    pub fn load(
        toy: &Ini,
        track_dir: &str,
        models: &[Placement],
        pco: &mut dyn FnMut(&str) -> Option<Vec<u8>>,
        psg: &mut dyn FnMut(&str) -> Option<Vec<u8>>,
        area: Option<Area>,
    ) -> (PropSet, Vec<String>) {
        let (claimed, unported) = props::claim(&props::entries(toy), track_dir, models, pco);
        let mut said: Vec<String> = unported.iter().map(|u| format!("not ported: {u:?}")).collect();
        // Every member's box, read once; a collection with a box it cannot read is left out whole.
        let mut kept = Vec::new();
        let mut boxes = std::collections::BTreeMap::new();
        for coll in claimed {
            let mut ok = true;
            for m in &coll.members {
                let file = props::model_file(&models[m.model].file);
                let read = psg(&file).ok_or_else(|| format!("no {file}")).and_then(|b| NodeBox::from_model(&b));
                match read {
                    Ok(b) => {
                        boxes.insert((m.model, m.part), b);
                    }
                    Err(why) => {
                        said.push(format!("{}: {file}: {why}", coll.name));
                        ok = false;
                    }
                }
            }
            if ok {
                kept.push(coll);
            }
        }
        let bodies = props::bodies(&kept, models, &mut |m: Member| boxes.get(&(m.model, m.part)).copied());
        let tables = Arc::new(Tables::build());
        let edge = area.map(|a| Edge { min: [F::ZERO, F::ZERO], max: [a.width, a.depth] });
        let mut collections = Vec::with_capacity(kept.len());
        let mut on = Vec::with_capacity(kept.len());
        let mut trees = Vec::with_capacity(kept.len());
        // Each model's ray file, read once: `None` where it has none, or it does not parse (said).
        let mut ray_files: std::collections::BTreeMap<String, Option<Rays>> = std::collections::BTreeMap::new();
        // And its collidable mesh, the same way (S197).
        let mut shape_files: std::collections::BTreeMap<String, Option<Mesh>> = std::collections::BTreeMap::new();
        let mut shapes = Vec::with_capacity(kept.len());
        let mut bodies = bodies.into_iter().peekable();
        for (c, coll) in kept.iter().enumerate() {
            let mut parts: Vec<(&Part, &str)> = Vec::new();
            let mut bones = Vec::new();
            let mut frames: Vec<Mat4> = Vec::new();
            let mut colliders = Vec::new();
            let mut probes = Vec::new();
            let mut meshes = Vec::new();
            while let Some(made) = bodies.next_if(|p| p.collection == c) {
                let m = coll.members[made.member];
                let entry = &coll.pco.parts[m.part];
                parts.push((&entry.part, &entry.name));
                bones.push(Bone { body: made.body, mass: entry.part.mass, inertia: made.inertia, reference: Vf::VF0 });
                frames.push(props::frame(&models[m.model]));
                colliders.push(made.collider);
                let file = props::rays_file(&models[m.model].file);
                let probe = ray_files.entry(file.clone()).or_insert_with(|| {
                    let bytes = psg(&file)?;
                    match track::Rays::parse(&bytes) {
                        Ok(r) => Some(flight_search::Rays::of(&r)),
                        Err(why) => {
                            said.push(format!("{}: {file}: {why}", coll.name));
                            None
                        }
                    }
                });
                probes.push(probe.clone());
                let file = props::shape_file(&models[m.model].file);
                let mesh = shape_files.entry(file.clone()).or_insert_with(|| {
                    let bytes = psg(&file)?;
                    match Mesh::parse(&bytes) {
                        Ok(m) => Some(m),
                        Err(why) => {
                            said.push(format!("{}: {file}: {why}", coll.name));
                            None
                        }
                    }
                });
                meshes.push(mesh.clone());
            }
            let (mut world, unmade) = Collection::new(&coll.pco, &parts, bones, &frames, &tables.curve);
            for u in unmade {
                said.push(match u {
                    Unmade::NoPart(name) => format!("{}: a constraint on {name}, which it has no part of", coll.name),
                });
            }
            world.edge = edge;
            for (((o, collider), probe), (part, _)) in world.objects.iter_mut().zip(colliders).zip(&probes).zip(&parts) {
                o.collision = Some(Collision::new(part, collider, probe.as_ref().map(Rays::pairs)));
            }
            on.push(coll.members.iter().map(|m| m.model).collect());
            trees.push(probes);
            shapes.push(meshes);
            collections.push(world);
        }
        let set = PropSet {
            collections,
            models: Arc::new(on),
            tables: Some(tables),
            rays: Arc::new(trees),
            shapes: Arc::new(shapes),
            listeners: Vec::new(),
            bike_frame: None,
        };
        (set, said)
    }

    /// Every collection's material listeners hear the frame's searches afresh (F27a).
    fn heard_afresh(&mut self) {
        for c in &mut self.collections {
            c.heard.0.clear();
            c.heard.1.probed.clear();
        }
    }

    /// No props on this track.
    pub fn is_empty(&self) -> bool {
        self.collections.is_empty()
    }

    /// **One frame** with nothing to strike: [`PropSet::frame_among`] with no terrain and no scene.
    pub fn frame(&mut self, dt: F) {
        self.frame_among(dt, None, None, None);
    }

    /// **One frame**: the props' world node's update — every filed collision object's search against
    /// the terrain `field`, the scene's collision meshes `props` and the ecosystem's `trees`
    /// ([`Collection::search`]), then every collection's step and listeners ([`Collection::step`]), in
    /// the order they were made.
    pub fn frame_among(&mut self, dt: F, field: Option<&dyn Heightfield>, props: Option<&Props>, trees: Option<&Trees>) {
        let movers = self.search(dt, field, props, trees);
        self.step(dt, &movers, &mut []);
    }

    /// The first half of [`PropSet::frame_among`], the node's collide pass: every filed collision
    /// object's search. Answers the movers the hits name, for [`PropSet::step`]: what a riding bike
    /// strikes goes in between (S197, [`crate::prop_contact`]).
    pub fn search(&mut self, dt: F, field: Option<&dyn Heightfield>, props: Option<&Props>, trees: Option<&Trees>) -> Vec<Mover> {
        if self.tables.is_none() {
            return Vec::new();
        }
        self.stamp_filed();
        self.heard_afresh();
        for c in 0..self.collections.len() {
            self.collections[c].search(dt, &self.rays[c], field, props, trees);
            self.file_listeners();
        }
        crate::prop_props::strike_props(self, &mut |set, c, k| {
            let rays = Arc::clone(&set.rays);
            set.collections[c].search_object(k, dt, &rays[c], field, props, trees);
        });
        props.map(|p| flight_search::movers(&p.props)).unwrap_or_default()
    }

    /// [`PropSet::search`] with the wreck bikes on the node (S243, [`Collection::search_among`]):
    /// also what the props' rays struck on each of `wrecks`, in its order.
    pub fn search_among(
        &mut self,
        dt: F,
        field: Option<&dyn Heightfield>,
        props: Option<&Props>,
        trees: Option<&Trees>,
        wrecks: &[Wreck<'_>],
    ) -> (Vec<Mover>, Vec<WreckStrike>) {
        let mut strikes = vec![WreckStrike::default(); wrecks.len()];
        if self.tables.is_none() {
            return (Vec::new(), strikes);
        }
        let rayed = crate::prop_wreck::wreck_rays_first(self, wrecks);
        self.stamp_filed();
        self.heard_afresh();
        for k in 0..self.collections.len() {
            self.collections[k].search_wrecked(k, dt, &self.rays[k], field, props, trees, wrecks, &mut strikes);
            self.file_listeners();
        }
        crate::prop_props::strike_props(self, &mut |set, c, k| {
            let rays = Arc::clone(&set.rays);
            set.collections[c].search_object_wrecked(c, k, dt, &rays[c], field, props, trees, wrecks, &mut strikes);
        });
        let strikes = crate::prop_wreck::wreck_rays_filed(self, rayed, strikes);
        (props.map(|p| flight_search::movers(&p.props)).unwrap_or_default(), strikes)
    }

    /// [`PropSet::search`] in a `world`, each collection against the shapes near its bodies (MD58):
    /// [`World::wooded_near`] around the middle of its filed bodies, as the plane's and the bike's
    /// searches take a world of many tiles (V96, MD28), so a woken prop's tick does not grow with the
    /// world. A collection whose filed bodies lie further apart than a near set reaches for all of
    /// them searches the whole set ([`World::wooded`]), and one with nothing filed searches nothing
    /// and asks nothing. A world of one map hands its whole set everywhere, so there it is
    /// [`PropSet::search`] over [`World::wooded`].
    ///
    /// A contact's mover is named by its place in the set its search was handed, so the movers go
    /// by set ([`PropMovers`]), and [`PropSet::step_with`] hands each collection its own.
    ///
    /// [`World::wooded_near`]: crate::world::World::wooded_near
    /// [`World::wooded`]: crate::world::World::wooded
    pub fn search_in(&mut self, dt: F, world: &mut dyn crate::world::World) -> PropMovers {
        self.search_in_among(dt, world, &[]).0
    }

    /// [`PropSet::search_in`] with the wreck bikes on the node ([`PropSet::search_among`]). A kept
    /// nearby list ([`Collision::nearby`]) names collidables by their place in the set it was gathered
    /// in, so one gathered in another near set is forgotten first, as the plane's is
    /// ([`flight_search::near_world`]).
    pub fn search_in_among(&mut self, dt: F, world: &mut dyn crate::world::World, wrecks: &[Wreck<'_>]) -> (PropMovers, Vec<WreckStrike>) {
        let mut out = PropMovers::default();
        let mut strikes = vec![WreckStrike::default(); wrecks.len()];
        if self.tables.is_none() {
            return (out, strikes);
        }
        let rayed = crate::prop_wreck::wreck_rays_first(self, wrecks);
        self.stamp_filed();
        self.heard_afresh();
        let mut keys: Vec<Option<[i32; 2]>> = Vec::new();
        // The collections that searched, in order: each puts its listeners on after its search.
        let mut filed = Vec::new();
        for (c, rays) in self.collections.iter_mut().zip(self.rays.iter()) {
            let Some(at) = filed_middle(c) else {
                out.of.push(None);
                continue;
            };
            let key = world.near_set(at).filter(|_| filed_within(c, at));
            for o in &mut c.objects {
                let Some(n) = o.collision.as_mut().map(|c| &mut c.nearby).filter(|n| n.built) else { continue };
                let [x, y, z, _] = n.centre;
                if world.near_set(Vf::new(x.0, y.0, z.0, 1.0)) != key {
                    n.built = false;
                    n.nearby.found.clear();
                }
            }
            let (field, props, trees) = if key.is_some() { world.wooded_near(at) } else { world.wooded() };
            c.search_wrecked(out.of.len(), dt, rays, field.as_deref(), props, trees, wrecks, &mut strikes);
            filed.push(out.of.len());
            let set = match keys.iter().position(|k| *k == key) {
                Some(i) => i,
                None => {
                    keys.push(key);
                    out.sets.push(props.map(|p| flight_search::movers(&p.props)).unwrap_or_default());
                    out.near.push(at);
                    out.sets.len() - 1
                }
            };
            out.of.push(Some(set));
        }
        for c in filed {
            self.file_on(c);
        }
        // A prop the props' rays file searches against its collection's set, or one near it (S242).
        crate::prop_props::strike_props(self, &mut |set, c, k| {
            let at = set.collections[c].objects[k].node.origin;
            let s = match out.of[c] {
                Some(s) => s,
                None => {
                    let key = world.near_set(at);
                    let s = match keys.iter().position(|k| *k == key) {
                        Some(i) => i,
                        None => {
                            let (_, props, _) = if key.is_some() { world.wooded_near(at) } else { world.wooded() };
                            keys.push(key);
                            out.sets.push(props.map(|p| flight_search::movers(&p.props)).unwrap_or_default());
                            out.near.push(at);
                            out.sets.len() - 1
                        }
                    };
                    out.of[c] = Some(s);
                    s
                }
            };
            let (field, props, trees) = if keys[s].is_some() { world.wooded_near(out.near[s]) } else { world.wooded() };
            let rays = Arc::clone(&set.rays);
            set.collections[c].search_object_wrecked(c, k, dt, &rays[c], field.as_deref(), props, trees, wrecks, &mut strikes);
        });
        let strikes = crate::prop_wreck::wreck_rays_filed(self, rayed, strikes);
        (out, strikes)
    }

    /// [`PropSet::step`] with each collection's movers its own search's ([`PropSet::search_in`]).
    pub fn step_with(&mut self, dt: F, movers: &PropMovers, strikers: &mut [Other<'_>]) {
        let Some(tables) = &self.tables else { return };
        let tables = Arc::clone(tables);
        for k in 0..self.collections.len() {
            crate::prop_props::step_named(self, k, dt, &tables, movers.of(k), strikers);
        }
        self.walk_listeners(dt);
    }

    /// The second half of [`PropSet::frame_among`]: every collection's step and listeners, the
    /// contacts' `movers` the search's, and `strikers` the riding bikes a struck body's mirrored
    /// contacts name (S197: by player, [`crate::prop_contact`]).
    pub fn step(&mut self, dt: F, movers: &[Mover], strikers: &mut [Other<'_>]) {
        let Some(tables) = &self.tables else { return };
        let tables = Arc::clone(tables);
        for k in 0..self.collections.len() {
            crate::prop_props::step_named(self, k, dt, &tables, movers, strikers);
        }
        self.walk_listeners(dt);
    }

    /// Wake one body: its collection activated and the body enabled.
    pub fn wake(&mut self, collection: usize, object: usize) {
        let c = &mut self.collections[collection];
        c.activate();
        c.enable(object);
        self.file_on(collection);
    }

    /// Wake one body as a strike does (S195, S197): the body enabled first, then its collection
    /// activated, so the object's listener goes on the world node ahead of the collection's settle —
    /// the reverse of [`PropSet::wake`], and the order the sleep tick reads.
    pub fn struck(&mut self, collection: usize, object: usize) {
        let c = &mut self.collections[collection];
        c.enable(object);
        c.activate();
        self.file_on(collection);
    }

    /// Every collection's listeners not yet on the world node put on at its end, collection by
    /// collection, each in the order it put them on: called after anything that may wake one, before
    /// another collection wakes.
    pub fn file_listeners(&mut self) {
        for c in 0..self.collections.len() {
            self.file_on(c);
        }
    }

    /// [`PropSet::file_listeners`] for collection `c` alone.
    pub fn file_on(&mut self, c: usize) {
        for &l in &self.collections[c].listeners {
            if !self.listeners.contains(&(c, l)) {
                self.listeners.push((c, l));
            }
        }
    }

    /// **The world node's listener walk** (F20), after every collection's step: each listener on the
    /// one list in turn, a finished one taken off by moving the list's last into its slot and the
    /// slot read again (the node's remove), so one collection's listener coming off reorders
    /// another's: in s195 knock ball 1's settle coming off at 3010 put ball 2's settle ahead of its
    /// object's listener, and ball 2's collection sleeps a tick after its body.
    pub fn walk_listeners(&mut self, dt: F) {
        self.file_listeners();
        let mut i = 0;
        while i < self.listeners.len() {
            let (c, l) = self.listeners[i];
            if self.collections[c].hear(l, dt) {
                self.listeners.swap_remove(i);
                let own = &mut self.collections[c].listeners;
                if let Some(j) = own.iter().position(|&x| x == l) {
                    own.remove(j);
                }
            } else {
                i += 1;
            }
        }
    }

    /// What a wheel standing on a prop's `f.col` reads (S169, [`Collection::under`]): the body's
    /// velocity and its velocity at the world `point`. `None` for a body with no collision object.
    pub fn under(&mut self, collection: usize, object: usize, point: Vf) -> Option<(Vf, Vf)> {
        self.collections[collection].under(object, point)
    }

    /// Every body's scene model and the node it is drawn by (the body's pose since it was last
    /// woken, else where it was made or put back), collection by collection.
    pub fn nodes(&self) -> impl Iterator<Item = (usize, &Node)> + '_ {
        self.collections
            .iter()
            .zip(self.models.iter())
            .flat_map(|(c, models)| models.iter().copied().zip(c.objects.iter().map(|o| &o.node)))
    }

    /// The scene model's world matrix a node stands it at: its three rows and its origin.
    pub fn matrix(node: &Node) -> Mat4 {
        Mat4([node.rows[0], node.rows[1], node.rows[2], node.origin])
    }
}

/// The movers [`PropSet::search_in`]'s searches named (MD58): one list for each set of shapes a
/// search was handed, and which list each collection's contacts index.
#[derive(Clone, Debug, Default)]
pub struct PropMovers {
    /// Each set's movers ([`flight_search::movers`]), in the order the sets were first handed.
    pub sets: Vec<Vec<Mover>>,
    /// Each collection's set, in collection order: `None` for one that did not search.
    pub of: Vec<Option<usize>>,
    /// Where each set was asked for (S242: a prop the props' rays file searches its collection's set).
    pub near: Vec<Vf>,
}

impl PropMovers {
    /// The movers collection `k`'s contacts index: none where it did not search.
    pub fn of(&self, k: usize) -> &[Mover] {
        self.of.get(k).copied().flatten().map_or(&[][..], |s| &self.sets[s][..])
    }
}

/// The middle of a collection's filed bodies on the ground plane, at the first one's height: where
/// [`PropSet::search_in`] asks for its near set. `None` with nothing filed.
fn filed_middle(c: &Collection) -> Option<Vf> {
    let mut filed = c.objects.iter().filter(|o| o.filed).map(|o| o.node.origin);
    let first = filed.next()?;
    let (mut lo, mut hi) = ([F(first.x()), F(first.z())], [F(first.x()), F(first.z())]);
    for at in filed {
        let p = [F(at.x()), F(at.z())];
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let half = F(0.5);
    Some(Vf::new(((lo[0] + hi[0]) * half).0, first.y(), ((lo[1] + hi[1]) * half).0, 1.0))
}

/// How far a filed body's search may reach past its own origin, feet, for a near set to serve it
/// ([`filed_within`]): half of [`crate::world::NEAR_MARGIN`], far more than a prop's rays and its
/// sweep back over a tick.
const PROP_REACH: f32 = 128.0;

/// Whether every filed body of `c` lies within [`PROP_REACH`] of the margin of `at`'s near
/// square, so the near set holds every shape its search can meet. `at` is in its square, so a body
/// within `NEAR_MARGIN − PROP_REACH` of `at` on each axis is.
fn filed_within(c: &Collection, at: Vf) -> bool {
    let room = F(crate::world::NEAR_MARGIN) - F(PROP_REACH);
    c.objects
        .iter()
        .filter(|o| o.filed)
        .all(|o| (F(o.node.origin.x()) - F(at.x())).abs() <= room && (F(o.node.origin.z()) - F(at.z())).abs() <= room)
}

impl std::fmt::Debug for PropSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PropSet").field("collections", &self.collections.len()).finish()
    }
}

impl State for Collection {
    /// What a tick writes: the bodies, the objects' countdowns, flags and nodes, the pins' running
    /// state, the world's lists and the node's listeners, the switch and the collection's countdown.
    /// The numbers read off the files (the world's rate, the thresholds, gravity, the bounds) are the
    /// track's and not walked.
    fn walk(&self, w: &mut Walk<'_>) {
        w.bool("active", self.active);
        w.f("countdown", self.countdown);
        for b in &self.bones {
            w.field("body", &crate::flight::BodyWalk(&b.body));
            w.vf("reference", b.reference);
        }
        w.list("objects", &self.objects);
        w.list("joints", &self.joints);
        for (name, list) in [("bodies", &self.bodies), ("forces", &self.forces), ("preforce", &self.preforce), ("posts", &self.posts)] {
            w.u32(name, list.len() as u32);
            list.iter().for_each(|&k| w.u32("k", k as u32));
        }
        w.u32("listeners", self.listeners.len() as u32);
        for l in &self.listeners {
            w.u32(
                "listener",
                match *l {
                    Listener::Settle => u32::MAX,
                    Listener::Object(k) => k as u32,
                },
            );
        }
    }
}

impl State for Object {
    fn walk(&self, w: &mut Walk<'_>) {
        w.vf("place", self.place);
        w.vf("look", self.look);
        w.vf("up", self.up);
        w.f("countdown", self.countdown);
        w.bool("fresh", self.fresh);
        w.bool("listens", self.listens);
        w.bool("filed", self.filed);
        w.u32("filed_at", self.filed_at);
        for (k, row) in self.node.rows.iter().enumerate() {
            w.vf(["row0", "row1", "row2"][k], *row);
        }
        w.vf("origin", self.node.origin);
        if let Some(c) = &self.collision {
            w.field("collision", c);
        }
    }
}

impl State for Collision {
    /// What a search and a resolve write; the node is the object's, walked there.
    fn walk(&self, w: &mut Walk<'_>) {
        let c = &self.collider;
        w.f("normal_speed", c.normal_speed);
        w.vf("relative", c.relative);
        w.vf("cofg_world", c.cofg_world);
        let r = &self.record;
        w.bool("touching", r.touching);
        w.bool("on_ground", r.on_ground);
        w.bool("on_object", r.on_object);
        w.bool("touching_other", r.touching_other);
        w.vf("swept_from", r.swept_from);
        w.vf("swept_to", r.swept_to);
        w.u32("contacts", r.contacts.len() as u32);
        w.u32("struck", r.struck.map_or(u32::MAX, |s| s as u32));
        w.u32("category_mode", self.categories.mode);
        w.u32("categories", self.categories.list.len() as u32);
        self.categories.list.iter().for_each(|&m| w.u32("category", m));
    }
}

impl State for Joint {
    fn walk(&self, w: &mut Walk<'_>) {
        w.bool("on", self.on);
        w.vf("hit", self.pin.hit);
        w.vf("sticky", self.pin.sticky);
        w.vf("gains", self.pin.gains);
        w.vf("target", self.pin.target);
        w.vf("target_velocity", self.pin.target_velocity);
    }
}
