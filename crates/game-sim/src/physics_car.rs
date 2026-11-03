//! **A struck `HasPhysics` car** (S186): the path car's subclass ([`crate::traffic`], S127) turned into
//! a rigid body when something strikes it, until it has lain still for its `ResetTime`, when it goes
//! back to its path.
//!
//! The car carries a physics object of the knockable props' own class ([`crate::prop_world`]): one
//! body out of the model's `.pco` (`[Physics Properties]`, the `.phy` part's keys, [`Part`]), a
//! collision object tuned by the part's setters ([`crate::wreck_bike::collider`]) with the model's
//! `<model>m.col` rays as its ray probe, and the model's `<model>f.col` mesh as the collidable others
//! strike while it is awake. It is filed on the world node the wreck bike and the props live on, and
//! it is made put away, the body off. Here it is a [`Collection`] of that one object whose world is the
//! node's ([`crate::wreck_bike::WORLD`]: one pass, 115 substeps a second), always active — it has no
//! settle listener of a collection's, and its object puts no listener of its own on the node; the
//! car's own listener does that work ([`PhysicsCar::settle`]).
//!
//! **The wake** ([`PhysicsCar::wake`]): the car's struck listener, which every search hitting the
//! car's path mesh or the object's ray probe calls. Unless the car is physical already: the object is
//! put on (its collision object, ray and sweep probes and `f.col` stood up and filed, then the enable:
//! the body into the world's body, force, preforce and post lists, on, and fresh), the car turns
//! physical, its path mesh is stood down, and the object is placed at the car — the body's centre of
//! mass through the car's model node, the car's heading as the look and its up as the up, squared and
//! normalised by the placement and **written back into the car's heading and up**. The body then takes
//! the car's own velocity and spin (its two vehicle virtuals, [`crate::traffic::Moving`]), and the
//! car's listener goes on the world node.
//!
//! **The node's update** ([`PhysicsCar::tick`]) is the props': the filed object empties its contacts
//! and searches (the sweep back, the 47 rays against the terrain and the world's objects; a hit on a
//! waking category wakes a body that is off), the world steps (the preforce listener clears the
//! shock peak; each substep the force listener adds the part's gravity and resolves the contacts; the
//! post listener writes the body's pose to the car's model node, or puts a body past the world's
//! bounds to sleep and back), and then the car's listener runs:
//!
//! * while the body is on, once it is at rest ([`Collection::at_rest`]: not fresh, and the speed under
//!   `SpeedThreshold`, the rate under `AngularRateThreshold`) it is taken out of the world's body,
//!   force and post lists (its preforce listener stays) and turned off, and the countdown is set to
//!   `ResetTime`. Its collision object stays filed: the sleeping truck goes on searching, and a hit on
//!   a waking category turns its body back on;
//! * while it is off, a countdown above zero runs down by the frame's `dt`, and once it is at or below
//!   zero the object is put away (its collision object unfiled, its list emptied), the car stops being
//!   physical and its path mesh is stood back up, and the listener comes off the node.
//!
//! The car's update while it is physical ([`crate::traffic::Traffic`]) runs the path's clock on as the
//! path car's does and stands the car where the body's node is; the first update back on the path is
//! the path car's own, with the car's last pose made its current one, so it answers no velocity.

#![deny(clippy::float_arithmetic)]

use std::sync::Arc;

use crate::biplane::Body;
use crate::contact::{Mover, Node};
use crate::contact_search::{self, Heightfield};
use crate::eco::Trees;
use crate::flight_search::Rays;
use crate::hash::{State, Walk};
use crate::math::{Mat4, mul};
use crate::phy::Part;
use crate::prop_world::{self, BoundsNode, Collection, Collision, Edge, Object};
use crate::props::NodeBox;
use crate::ragdoll::{Bone, Tables};
use crate::track::{self, Mesh};
use crate::traffic::{Car, Moving};
use crate::world::Props;
use crate::wreck_bike::{self, WreckBike};
use ps2_float::{F, Vf};

/// The `.pco`'s one section: the part's keys.
pub const SECTION: &str = "Physics Properties";

/// A car's `.pco`, as its constructor names it: the model's name with `.pco` for `.psg`, in the track's
/// directory (`Tracks\Free06\ctruck.pco`).
pub fn pco_file(model: &str) -> String {
    let stem = model.rfind('.').map_or(model, |at| &model[..at]);
    format!("{stem}.pco")
}

/// **A `HasPhysics` car's physics object**: its one-object world, whether the car is physical now and
/// was at its last update, and the track's constants (the ray probe's tree, the `f.col`, the tables
/// the world's step takes).
#[derive(Clone)]
pub struct PhysicsCar {
    /// The object: its body, its collision object, its world's lists. Always active.
    pub set: Collection,
    /// The car is a rigid body now (`car+0x588`), and was at its last update (`+0x58c`).
    pub physical: bool,
    pub was_physical: bool,
    /// The car's pose at its last update's top (`car+0x2e0`): what the wake reads its motion from.
    pub last: Mat4,
    /// The ray probe's tree, the model's `m.col` (`None` where it has none): a constant.
    pub rays: Arc<Option<Rays>>,
    /// The model's `f.col`, the collidable stood at the body while it is physical: a constant.
    pub shape: Arc<Option<Mesh>>,
    /// The tables the world's step takes (it has no pins or springs to read them).
    pub tables: Option<Arc<Tables>>,
}

impl PartialEq for PhysicsCar {
    /// The state: the constants are the track's.
    fn eq(&self, other: &PhysicsCar) -> bool {
        self.set == other.set && self.physical == other.physical && self.was_physical == other.was_physical && self.last == other.last
    }
}

/// **The model's box, as the bounds query takes it** (`.psg` bytes): every node's own box (its
/// 224-byte record's centre and half extent) widened by its children's through their world matrices,
/// from the root ([`prop_world::node_bounds`]). A truck's root is its `Body` node, the node the car is
/// posed by, and its children are its four tires.
pub fn model_bounds(psg: &[u8]) -> Result<NodeBox, String> {
    Ok(prop_world::node_bounds(&model_tree(psg, None)?))
}

/// [`model_bounds`]' span, its low and high corners ([`prop_world::node_span`]).
pub fn model_span(psg: &[u8]) -> Result<(Vf, Vf), String> {
    Ok(prop_world::node_span(&model_tree(psg, None)?))
}

/// The model's node tree as the bounds query walks it, its root at `posed` (its world rows) or where
/// the file has it.
pub fn model_tree(psg: &[u8], posed: Option<&Mat4>) -> Result<BoundsNode, String> {
    const NODES: usize = 12;
    const RECORD: usize = 224;
    const MATRIX: usize = 128;
    const CENTRE: usize = 192;
    const HALF: usize = 204;
    const PARENT: usize = 220;
    let word = |at: usize| -> Result<u32, String> {
        psg.get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| format!(".psg ends at {} before byte {at}", psg.len()))
    };
    if psg.get(..4) != Some(b"psg\0".as_slice()) {
        return Err("not a .psg: no psg\\0 at the front".into());
    }
    let count = word(8)? as usize;
    let at = |i: usize| NODES + i * RECORD;
    let mut parents = Vec::with_capacity(count);
    let mut worlds: Vec<Mat4> = Vec::with_capacity(count);
    for i in 0..count {
        let parent = word(at(i) + PARENT)? as i32;
        let row = |r: usize| -> Result<Vf, String> {
            let b = at(i) + MATRIX + r * 16;
            Ok(Vf::from_bits([word(b)?, word(b + 4)?, word(b + 8)?, word(b + 12)?]))
        };
        let local = Mat4([row(0)?, row(1)?, row(2)?, row(3)?]);
        let world = match usize::try_from(parent) {
            Ok(p) if p < i => mul(&local, &worlds[p]),
            Ok(_) => return Err(format!("node {i}'s parent {parent} comes after it")),
            Err(_) => posed.map_or(local, |w| mul(&local, w)),
        };
        parents.push(parent);
        worlds.push(world);
    }
    if count == 0 || parents[0] != -1 {
        return Err("a .psg whose first node is not its root".into());
    }
    let three = |at: usize| -> Result<Vf, String> { Ok(Vf::from_bits([word(at)?, word(at + 4)?, word(at + 8)?, 1f32.to_bits()])) };
    let own = |i: usize| -> Result<NodeBox, String> { Ok(NodeBox { centre: three(at(i) + CENTRE)?, half_extent: three(at(i) + HALF)? }) };
    fn tree(i: usize, parents: &[i32], worlds: &[Mat4], own: &dyn Fn(usize) -> Result<NodeBox, String>) -> Result<BoundsNode, String> {
        let mut children = Vec::new();
        for (k, &p) in parents.iter().enumerate() {
            if p == i as i32 && k > i {
                children.push(tree(k, parents, worlds, own)?);
            }
        }
        Ok(BoundsNode { own: Some(own(i)?), world: worlds[i], children })
    }
    tree(0, &parents, &worlds, &own)
}

/// **The size a car's body takes its inertia off**: the model's span ([`model_span`]), high corner less
/// low, halved and doubled — the span itself, not [`crate::props::box_size`] of its centre and half extent,
/// which can land an ulp off when a child widens the box (`ctruck`'s length).
pub fn model_size(psg: &[u8]) -> Result<Vf, String> {
    let (lo, hi) = model_span(psg)?;
    Ok(hi.sub(lo).mul(Vf::splat(0.5)).mul(Vf::splat(2.0)))
}

impl PhysicsCar {
    /// **The car's physics object as its init leaves it**: the part out of the `.pco` (`pco`), its body's
    /// inertia off the model's span ([`model_size`], [`Part::inertia`]), the
    /// collision object with the `m.col`'s rays (`rays`, when the model has one), the `f.col`
    /// (`shape`), and the world's bounds a body past which is put back (`edge`, the terrain's). Put
    /// away: the body off and in no list, the collision object unfiled, the countdown `-1`.
    pub fn load(pco: &[u8], psg: &[u8], rays: Option<&[u8]>, shape: Option<&[u8]>, edge: Option<Edge>) -> Result<PhysicsCar, String> {
        let part = Part::read_bytes(pco, SECTION);
        let inertia = part.inertia(model_size(psg)?);
        let rays = rays.map(track::Rays::parse).transpose()?.map(|r| Rays::of(&r));
        let shape = shape.map(Mesh::parse).transpose()?;
        let made = WreckBike::new(&part, inertia, Vf::VF0, Vf::VF0);
        let mut body = made.body;
        body.enabled = false;
        let up = Vf::new(0.0, 1.0, 0.0, 0.0);
        let look = Vf::new(0.0, 0.0, 1.0, 0.0);
        let mut object = Object::new(&part, Vf::VF0, look, up);
        // The object listener is not put on (`+0x5c` 0): the car's own listener settles it.
        object.listens = false;
        object.collision = Some(Collision::new(&part, made.collider, rays.as_ref().map(Rays::pairs)));
        let set = Collection {
            world: wreck_bike::WORLD,
            bones: vec![Bone { body, mass: part.mass, inertia, reference: Vf::VF0 }],
            objects: vec![object],
            joints: Vec::new(),
            springs: Vec::new(),
            bodies: Vec::new(),
            forces: Vec::new(),
            preforce: Vec::new(),
            posts: Vec::new(),
            listeners: Vec::new(),
            // The node's world steps every frame; with no body in it the step is nothing.
            active: true,
            pin_to_body: false,
            reset_time: F(-1.0),
            countdown: F(-1.0),
            edge,
            heard: crate::material_hit::HeardBy::default(),
        };
        Ok(PhysicsCar { set, physical: false, was_physical: false, last: Mat4([Vf::VF0; 4]), rays: Arc::new(rays), shape: Arc::new(shape), tables: Some(Arc::new(Tables::build())) })
    }

    /// The body.
    pub fn body(&self) -> &Body {
        &self.set.bones[0].body
    }

    /// The object: its countdown, its node, its collision object.
    pub fn object(&self) -> &Object {
        &self.set.objects[0]
    }

    /// Where the car's model stands while it is physical: the object's node, which the post listener
    /// writes from the body.
    pub fn pose(&self) -> Mat4 {
        let n = &self.set.objects[0].node;
        Mat4([n.rows[0], n.rows[1], n.rows[2], n.origin])
    }

    /// **The wake** (the car's struck listener): unless the car is physical already, the object put on,
    /// the car physical, the object placed at the car (`car.pose` its model node, `car.head` and
    /// `car.normal` the look and up, squared and normalised and written back), the car stood where the
    /// placement put the model, and the body given the car's motion from `last` (the car's pose a
    /// frame ago) to there, as the car's two vehicle virtuals read it ([`Moving::of`]). Says whether
    /// it woke.
    pub fn wake(&mut self, car: &mut Car, last: &Mat4, inv_dt: F) -> bool {
        if self.physical {
            return false;
        }
        self.set.enable(0);
        self.physical = true;
        let node = Node { rows: [car.pose.0[0], car.pose.0[1], car.pose.0[2]], origin: car.pose.0[3] };
        let o = &mut self.set.objects[0];
        o.place = prop_world::node_point(&node, self.set.bones[0].body.cofg);
        o.look = car.head;
        o.up = car.normal;
        self.set.place(0);
        let o = &self.set.objects[0];
        car.head = o.look;
        car.normal = o.up;
        car.pose = self.pose();
        let moving = Moving::of(last, &car.pose, inv_dt);
        let b = &mut self.set.bones[0].body;
        b.velocity = moving.velocity;
        b.rates = moving.spin;
        true
    }

    /// **The world node's update, for this object**: the filed collision object's search against the
    /// terrain `field` and the world's `props` and `trees` ([`Collection::search`]), the world stepped
    /// with the contacts' movers, then the car's listener while the car is physical. Says whether the
    /// car was put back this frame.
    pub fn tick(&mut self, dt: F, field: Option<&dyn Heightfield>, props: Option<&Props>, trees: Option<&Trees>) -> bool {
        let rays = Arc::clone(&self.rays);
        self.set.search(dt, std::slice::from_ref(&*rays), field, props, trees);
        let movers: Vec<Mover> = props.map(|p| crate::flight_search::movers(&p.props)).unwrap_or_default();
        self.step(dt, &movers)
    }

    /// The second half of [`PhysicsCar::tick`]: the world stepped with `movers`, then the listener.
    pub fn step(&mut self, dt: F, movers: &[Mover]) -> bool {
        let tables = self.tables.get_or_insert_with(|| Arc::new(Tables::build())).clone();
        self.set.step(dt, &tables, movers);
        self.physical && self.settle(dt)
    }

    /// **The car's listener on the node**: says whether it is done (the car put back).
    pub fn settle(&mut self, dt: F) -> bool {
        if self.set.bones[0].body.enabled {
            if !self.set.at_rest(0) {
                return false;
            }
            // Out of the world's lists but for its preforce listener, and off; the collision object is
            // not unfiled.
            let set = &mut self.set;
            set.bodies.retain(|&k| k != 0);
            set.forces.retain(|&k| k != 0);
            set.posts.retain(|&k| k != 0);
            set.bones[0].body.enabled = false;
            let o = &mut set.objects[0];
            o.countdown = o.reset_time;
            return false;
        }
        let o = &mut self.set.objects[0];
        if o.countdown <= F::ZERO {
            return false;
        }
        o.countdown = o.countdown - dt;
        if !(o.countdown <= F::ZERO) {
            return false;
        }
        self.put_away();
        self.physical = false;
        true
    }

    /// The object put away: its collision object off the node, its contacts and flags emptied.
    fn put_away(&mut self) {
        let o = &mut self.set.objects[0];
        if !o.filed {
            return;
        }
        o.filed = false;
        if let Some(c) = o.collision.as_mut() {
            contact_search::clear(&mut c.record, Some(&mut self.set.bones[0].body));
        }
    }
}

impl State for PhysicsCar {
    fn walk(&self, w: &mut Walk<'_>) {
        w.bool("physical", self.physical);
        w.bool("was_physical", self.was_physical);
        for (name, row) in ["last0", "last1", "last2", "last3"].into_iter().zip(self.last.0) {
            w.vf(name, row);
        }
        w.field("object", &self.set);
    }
}
