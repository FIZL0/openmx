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
//! What is not here yet (filed as its own task, S197): a prop's `f.col` as a collidable the bike, the
//! rider, the wreck bike, the plane and the other props strike — whose struck listener wakes it — and
//! the contacts' impulse on its body. Until then nothing in a ride wakes a prop.

#![deny(clippy::float_arithmetic)]

use std::sync::Arc;

use crate::biplane::Area;
use crate::contact::Node;
use crate::contact_search::Heightfield;
use crate::eco::Trees;
use crate::flight_search::{self, Rays};
use crate::hash::{State, Walk};
use crate::math::Mat4;
use crate::phy::{Ini, Part};
use crate::prop_world::{Collection, Collision, Edge, Joint, Listener, Object, Unmade};
use crate::props::{self, Member, NodeBox};
use crate::ragdoll::{Bone, Tables};
use crate::track::{self, Placement};
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
}

impl PropSet {
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
        let mut bodies = bodies.into_iter().peekable();
        for (c, coll) in kept.iter().enumerate() {
            let mut parts: Vec<(&Part, &str)> = Vec::new();
            let mut bones = Vec::new();
            let mut frames: Vec<Mat4> = Vec::new();
            let mut colliders = Vec::new();
            let mut probes = Vec::new();
            while let Some(made) = bodies.next_if(|p| p.collection == c) {
                let m = coll.members[made.member];
                let entry = &coll.pco.parts[m.part];
                parts.push((&entry.part, &entry.name));
                bones.push(Bone { body: made.body, mass: entry.part.mass, inertia: Vf::ZERO, reference: Vf::VF0 });
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
            }
            let (mut world, unmade) = Collection::new(&coll.pco, &parts, bones, &frames, &tables.curve);
            for u in unmade {
                said.push(match u {
                    Unmade::TorsionSpring(name) => format!("{}: {name}'s BodyTorsionSpring is not ported", coll.name),
                    Unmade::NoPart(name) => format!("{}: a constraint on {name}, which it has no part of", coll.name),
                });
            }
            world.edge = edge;
            for (((o, collider), probe), (part, _)) in world.objects.iter_mut().zip(colliders).zip(&probes).zip(&parts) {
                o.collision = Some(Collision::new(part, collider, probe.as_ref().map(Rays::pairs)));
            }
            on.push(coll.members.iter().map(|m| m.model).collect());
            trees.push(probes);
            collections.push(world);
        }
        let set = PropSet { collections, models: Arc::new(on), tables: Some(tables), rays: Arc::new(trees) };
        (set, said)
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
        let Some(tables) = &self.tables else { return };
        for (c, rays) in self.collections.iter_mut().zip(self.rays.iter()) {
            c.search(dt, rays, field, props, trees);
        }
        let movers = props.map(|p| flight_search::movers(&p.props)).unwrap_or_default();
        for c in &mut self.collections {
            c.step(dt, tables, &movers);
        }
    }

    /// Wake one body, as a strike does: its collection activated and the body enabled.
    pub fn wake(&mut self, collection: usize, object: usize) {
        let c = &mut self.collections[collection];
        c.activate();
        c.enable(object);
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
