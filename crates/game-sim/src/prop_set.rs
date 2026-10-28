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
//! Once a frame ([`PropSet::frame`]) every collection runs its node's update ([`Collection::tick`]),
//! in the order they were made. A collection nothing has woken does nothing, so a track full of props
//! at rest costs a walk of the list.
//!
//! The state is [`PropSet::collections`]: hashed and sent with a rollback snapshot, on a track that
//! has props (an empty set is not walked or sent, so every state without props hashes and encodes as
//! before). Which scene model each body stands on ([`PropSet::models`]) and the ragdoll's tables the
//! pins solve with are constants of the track, shared by every clone.
//!
//! What is not here yet (filed as their own tasks): each prop's own collision object (its `m.col` rays
//! and sweep against the terrain and the scene, resolved in its force listener), its `f.col` as a
//! collidable the bike, the rider, the wreck bike and the plane strike — whose struck listener wakes
//! it — and the contacts' impulse on its body. Until then nothing in a ride wakes a prop.

#![deny(clippy::float_arithmetic)]

use std::sync::Arc;

use crate::biplane::Area;
use crate::hash::{State, Walk};
use crate::contact::Node;
use crate::math::Mat4;
use crate::phy::{Ini, Part};
use crate::prop_world::{Collection, Edge, Joint, Listener, Object, Unmade};
use crate::props::{self, Member, NodeBox};
use crate::ragdoll::{Bone, Tables};
use crate::track::Placement;
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
        let mut bodies = bodies.into_iter().peekable();
        for (c, coll) in kept.iter().enumerate() {
            let mut parts: Vec<(&Part, &str)> = Vec::new();
            let mut bones = Vec::new();
            let mut frames: Vec<Mat4> = Vec::new();
            while let Some(made) = bodies.next_if(|p| p.collection == c) {
                let m = coll.members[made.member];
                let entry = &coll.pco.parts[m.part];
                parts.push((&entry.part, &entry.name));
                bones.push(Bone { body: made.body, mass: entry.part.mass, inertia: Vf::ZERO, reference: Vf::VF0 });
                frames.push(props::frame(&models[m.model]));
            }
            let (mut world, unmade) = Collection::new(&coll.pco, &parts, bones, &frames, &tables.curve);
            for u in unmade {
                said.push(match u {
                    Unmade::TorsionSpring(name) => format!("{}: {name}'s BodyTorsionSpring is not ported", coll.name),
                    Unmade::NoPart(name) => format!("{}: a constraint on {name}, which it has no part of", coll.name),
                });
            }
            world.edge = edge;
            on.push(coll.members.iter().map(|m| m.model).collect());
            collections.push(world);
        }
        let set = PropSet { collections, models: Arc::new(on), tables: Some(tables) };
        (set, said)
    }

    /// No props on this track.
    pub fn is_empty(&self) -> bool {
        self.collections.is_empty()
    }

    /// **One frame**: every collection's node update, in the order they were made.
    pub fn frame(&mut self, dt: F) {
        let Some(tables) = &self.tables else { return };
        for c in &mut self.collections {
            c.tick(dt, tables);
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
