//! The wreck bike as the original runs it: `Wreckbike.phy`'s one part, a generic rigid body with a
//! collision object beside it, stepped by a dynamics world of its own.
//!
//! When the rider wrecks, the bike stops being a vehicle and becomes a physics object: the body
//! ([`crate::biplane::Body`], the same generic body the plane flies), the collision object
//! ([`crate::contact::Collider`] and [`crate::contact_search::Record`]), a ray probe of eleven rays
//! out of the bike model's collision file, a sweep probe, and the record's transform. Once a frame
//! the world node they hang off runs, and **that node's update is the wreck bike's whole tick** — run
//! alone on the game's own memory it reproduces the captured body, tick for tick, to the bit (see the
//! journal). In order:
//!
//! 1. every collision object in the node empties its contact list ([`contact_search::clear`]);
//! 2. every one searches ([`contact_search::collide`]): the sweep back from where the body was a
//!    little while ago, the heightfield rays against the terrain, the world rays against the world's
//!    objects;
//! 3. the node's dynamics world steps ([`biplane::World::step`], 115 substeps a second, so four at
//!    30 Hz). The part's preforce listener clears the body's shock peak; each substep its force
//!    listener adds the part's gravity and has the collision object resolve its contacts
//!    ([`contact::resolve`]), and the body is integrated; after the substeps its post listener puts
//!    the transform where the body is ([`Body::pose`]).
//!
//! The collision object's tuning is set by the part's loader through four setters, from the part's
//! keys ([`collider`], [`record`]): the object's mass and inertia off the body's, the contact spring
//! off the body's mass, the high-speed sweep, the moment masks and the frictions.
//!
//! What the search meets in the world is handed in: the terrain as a
//! [`contact_search::Heightfield`], the world's objects as a [`contact_search::Search`]. The ride has
//! no port of the world search over its scene's objects yet, so it hands in [`NoObjects`].

#![deny(clippy::float_arithmetic)]

use crate::biplane::{self, Body, Dynamics, List};
use crate::contact::{self, Collider, Node, Response, Weighed};
use crate::contact_search::{self, Heightfield, Hit, Record, Search, Struck, Swept};
use crate::hash::{State, Walk};
use crate::math::quat_to_mat4;
use crate::phy::Part;
use ps2_float::{F, Vf, vu};

/// The world's two numbers: one constraint pass (it has no constraints) and 115 substeps a second.
pub const WORLD: biplane::World = biplane::World { iterations: 1, rate: F(115.0) };

/// The weight the contact spring's stiffness is scaled by: 32.2, a pound's weight in its own units.
const POUND: F = F(f32::from_bits(0x4200_cccd));

/// The setters' "copy the other one" value, in every lane.
const SENTINEL: Vf = Vf::new(-1.0, -1.0, -1.0, 1.0);

/// The flags of the wreck bike's collidable (the collision object's `+0x1f8`, its `+0x84`) and the
/// object's actor and collides-with masks (`+0x110`, `+0x114`). None of them is a key of the part:
/// they are read off the running game. They decide only what the object does against another
/// collision object, which the ride's wreck never meets.
pub const COLLIDABLE: u32 = 0x0009_a058;
pub const ACTORS: u32 = 8;
pub const COLLIDES_WITH: u32 = 0x0004_0000;

/// The bike model's ray probe: one start and eleven ends, model space, out of its collision file.
/// The ride has no model library, so these are the ones the game loaded, read off its memory (as
/// [`crate::ride::WRECK_INERTIA`] is).
const RAY_START: Vf = Vf([f32::from_bits(0xb792_0000), f32::from_bits(0x4012_ec4c), f32::from_bits(0xbcff_7c80), 1.0]);
pub const RAY_ENDS: [[u32; 3]; 11] = [
    [0xbfa8_3fe0, 0x4092_8f97, 0x3ed6_6320],
    [0x3fa8_3ebc, 0x4092_8f97, 0x3ed6_6320],
    [0x36e6_0000, 0x4045_298b, 0x4040_bbb8],
    [0x388d_2000, 0x3fc6_3de6, 0x4070_6d93],
    [0x388d_2000, 0x3fc6_3de6, 0xc074_6b85],
    [0x3f2f_4c26, 0x3fba_00d2, 0xbf6a_04c0],
    [0x388d_2000, 0x3fba_00d2, 0x3e92_7ee8],
    [0x388d_2000, 0x4069_2557, 0xc06e_dacb],
    [0xbf2f_4c26, 0x3fba_00d2, 0xbf6a_04c0],
    [0x37bb_c000, 0x3ddc_6c70, 0xc02e_43dd],
    [0x380d_1000, 0x3c39_6a00, 0x4023_8b47],
];

/// The rays as the probe holds them, `[start, end]`.
pub fn rays() -> Vec<[Vf; 2]> {
    RAY_ENDS
        .iter()
        .map(|e| [RAY_START, Vf([f32::from_bits(e[0]), f32::from_bits(e[1]), f32::from_bits(e[2]), 1.0])])
        .collect()
}

// --- the loader's four setters -----------------------------------------------------------------------

/// `1/x` on the FPU (`div.s`).
fn recip(x: F) -> F {
    F::ONE / x
}

/// The collision object's tuning, as the part's loader sets it, for a body of `body_mass` with
/// `body_inertia` (the tensor, not its inverse) and `body_inv_mass`.
///
/// * **Mass**: `CollisionWeight` in slugs, and its inverse. The inertia is the body's scaled by the
///   ratio of the two masses (`mass * body_inv_mass`), `w` the body's; its inverse is `1/x` and
///   `1/y` on the FPU and `1/z` on the vector unit's divider, `w` the inertia's — and all of it
///   `(0, 0, 0, 1)` if any axis is not positive. The centre of mass is `CollisionCofGOffset`.
/// * **Spring**: `MaxPenetration` is the deepest a contact is read; the stiffness is
///   `|32.2 * SpringStiffness * body mass|` and the damping `2 * SpringDamping * sqrt(body mass *
///   stiffness)` (0 when either is not positive); no restitution. The actors' response is a copy.
/// * **Moments**: the scenery's spring moment mask is `ImpulseAngularRateScale`, its friction's
///   `SpringMomentScale`; the actors' spring mask `FrictionMomentScale` and friction
///   `SpringMomentScale`. That is by the offsets the setter writes and the resolution reads, not by
///   the keys' names.
/// * **Friction**: `FrictionZone1` against the scenery and on every surface, `BodyFriction`
///   against actors.
pub fn collider(part: &Part, body_mass: F, body_inv_mass: F, body_inertia: Vf) -> Collider {
    let mass = part.collision_mass;
    let inv_mass = if mass > F::ZERO { recip(mass) } else { F::ZERO };
    let k = mass * body_inv_mass;
    let inertia = body_inertia.merge(ps2_float::Dest::XYZ, body_inertia.mul(Vf::splat(k.0)));
    let (ix, iy, iz) = (F(inertia.x()), F(inertia.y()), F(inertia.z()));
    let inv_inertia = if !(ix > F::ZERO) || !(iy > F::ZERO) || !(iz > F::ZERO) {
        Vf::new(0.0, 0.0, 0.0, 1.0)
    } else {
        Vf::new(recip(ix).0, recip(iy).0, vu::div(1.0, iz.0), inertia.w())
    };
    let stiffness = (POUND * (part.spring_stiffness * body_mass)).abs();
    let damping = |c: F| {
        if !(body_mass > F::ZERO) || !(stiffness > F::ZERO) {
            F::ZERO
        } else {
            (F(2.0) * c) * (body_mass * stiffness).sqrt()
        }
    };
    let scenery_damping = damping(part.spring_damping);
    let friction_mask = part.spring_moment_scale;
    let actor_spring = if part.friction_moment_scale == SENTINEL {
        part.impulse_angular_rate_scale
    } else {
        part.friction_moment_scale
    };
    Collider {
        damp: false,
        damp_linear: F::ONE,
        damp_angular: F::ONE,
        mass,
        inv_mass,
        inv_inertia,
        cofg: part.collision_cofg_offset,
        cofg_world: Vf::ZERO,
        actors: ACTORS,
        collides_with: COLLIDES_WITH,
        collidable: Some(COLLIDABLE),
        max_depth: part.max_penetration,
        scenery: Response {
            stiffness,
            damping: scenery_damping,
            restitution: F::ZERO,
            angular: true,
            friction: part.friction_zone1,
            spring_mask: part.impulse_angular_rate_scale,
            friction_mask,
        },
        actor: Response {
            stiffness,
            damping: damping(part.spring_damping),
            restitution: F::ZERO,
            angular: true,
            friction: part.body_friction,
            spring_mask: actor_spring,
            friction_mask,
        },
        surfaces: [part.friction_zone1; 16],
        normal_speed: F::ZERO,
        relative: Vf::ZERO,
    }
}

/// The collision object's search side: it looks for the ground and the world, sweeps back by
/// `CheckForHighSpeedCollisions` from `HighSpeedRayOffset`, `HighSpeedRaySpeedScale` seconds of
/// velocity back and `HighSpeedUnpenetrationDistance` out, and holds one contact a ray.
pub fn record(part: &Part, node: Node) -> Record {
    let rays = rays();
    Record {
        search_ground: true,
        search: true,
        sweep_back: part.check_for_high_speed_collisions,
        rest_linear: F::ONE,
        rest_angular: F::ONE,
        offset: part.high_speed_ray_offset,
        lead: part.high_speed_ray_speed_scale,
        push_out: part.high_speed_unpenetration_distance,
        facing: F(-1.0),
        node,
        capacity: rays.len(),
        nearby: true,
        rays: Some(rays),
        sweeps: true,
        ..Record::default()
    }
}

/// The transform the post listener writes: the body's [`Body::pose`] as rows and an origin.
pub fn node_of(body: &Body) -> Node {
    let (q, origin) = body.pose();
    let m = quat_to_mat4(q, origin);
    Node { rows: [m.0[0], m.0[1], m.0[2]], origin: m.0[3] }
}

// --- the world's objects -----------------------------------------------------------------------------

/// A world with nothing in it but the terrain: the rays and the sweep strike nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoObjects;

impl Search for NoObjects {
    fn rays(&mut self, _nearby: bool) -> Vec<Hit> {
        Vec::new()
    }
    fn sweep(&mut self, _from: Vf, _to: Vf, _nearby: bool) -> Option<Swept> {
        None
    }
    fn moved(&mut self, _node: &crate::contact::Node) {}
    fn velocity(&self, _struck: &Struck) -> Vf {
        Vf::ZERO
    }
}

// --- the bike ----------------------------------------------------------------------------------------

/// The wreck bike: the body, its mass (the body keeps only the inverse), the part's gravity, and the
/// collision object.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WreckBike {
    pub body: Body,
    pub mass: F,
    pub gravity: Vf,
    pub collider: Collider,
    pub record: Record,
}

/// The node's dynamics world as the wreck bike sees it: one body, the part's force listener, its
/// preforce and post listeners.
struct Lists<'a>(&'a mut WreckBike);

impl Dynamics for Lists<'_> {
    fn count(&self, list: List) -> i32 {
        match list {
            List::Constraints => 0,
            _ => 1,
        }
    }
    fn preforce(&mut self, _i: i32, _dt: F) {
        self.0.body.clear_shock_peak();
    }
    fn force(&mut self, _i: i32, _h: F) {
        let WreckBike { body, mass, gravity, collider, record } = &mut *self.0;
        biplane::collision_force(body, *gravity, |body| {
            let mut rigid = Weighed { body, mass: *mass };
            contact::resolve(collider, &mut rigid, &mut record.contacts, &mut [], &[], true);
        });
    }
    fn enabled(&self, _body: i32) -> bool {
        self.0.body.enabled
    }
    fn integrate(&mut self, _body: i32, h: F) {
        self.0.body.integrate(h);
    }
    fn end_substep(&mut self, _body: i32) {
        self.0.body.end_substep();
    }
    fn constrain(&mut self, _i: i32, _first: bool) {}
    fn post(&mut self, _i: i32, _dt: F) {
        self.0.record.node = node_of(&self.0.body);
    }
}

impl WreckBike {
    /// The part's body with nothing moving yet: `inertia` is the tensor (not its inverse), `w` 1.
    /// The body's damping is the part's default damping; the transform is where the body is.
    pub fn new(part: &Part, inertia: Vf, position: Vf, quat: Vf) -> WreckBike {
        let inv = |x: f32| if x == 0.0 { F::ZERO } else { recip(F(x)) };
        let inv_mass = if part.mass == F::ZERO { F::ZERO } else { recip(part.mass) };
        let body = Body {
            enabled: true,
            inv_mass,
            damping: part.velocity_damping,
            last_position: position,
            position,
            velocity: Vf::new(0.0, 0.0, 0.0, 1.0),
            last_force: Vf::new(0.0, 0.0, 0.0, 1.0),
            force: Vf::new(0.0, 0.0, 0.0, 1.0),
            accel: Vf::new(0.0, 0.0, 0.0, 1.0),
            shock_peak: Vf::new(0.0, 0.0, 0.0, 1.0),
            shock: Vf::new(0.0, 0.0, 0.0, 1.0),
            step: Vf::new(0.0, 0.0, 0.0, 1.0),
            inv_inertia: Vf::new(inv(inertia.x()).0, inv(inertia.y()).0, inv(inertia.z()).0, 1.0),
            ang_damping: part.angular_rate_damping,
            last_quat: quat,
            quat,
            cofg: part.cofg,
            moment: Vf::new(0.0, 0.0, 0.0, 1.0),
            last_moment: Vf::new(0.0, 0.0, 0.0, 1.0),
            rates: Vf::new(0.0, 0.0, 0.0, 1.0),
            ang_accel: Vf::new(0.0, 0.0, 0.0, 1.0),
            twist_peak: Vf::new(0.0, 0.0, 0.0, 1.0),
            twist: Vf::new(0.0, 0.0, 0.0, 1.0),
        };
        let collider = collider(part, part.mass, inv_mass, inertia);
        let record = record(part, node_of(&body));
        WreckBike { body, mass: part.mass, gravity: part.gravity, collider, record }
    }

    /// **One frame**, as the original's world node runs it: the contact list emptied, the search,
    /// and the world stepped.
    pub fn tick(&mut self, dt: F, ground: Option<&dyn Heightfield>, objects: &mut dyn Search) {
        contact_search::clear(&mut self.record, Some(&mut self.body));
        contact_search::collide(std::slice::from_mut(&mut self.record), 0, Some(&mut self.body), ground, objects, dt);
        WORLD.step(dt, &mut Lists(self));
    }

    /// The model's origin: the transform's translation.
    pub fn origin(&self) -> Vf {
        self.record.node.origin
    }
}

impl State for WreckBike {
    fn walk(&self, w: &mut Walk<'_>) {
        let b = &self.body;
        w.bool("enabled", b.enabled);
        w.f("inv_mass", b.inv_mass);
        for (name, v) in [
            ("damping", b.damping),
            ("last_position", b.last_position),
            ("position", b.position),
            ("velocity", b.velocity),
            ("last_force", b.last_force),
            ("force", b.force),
            ("accel", b.accel),
            ("shock_peak", b.shock_peak),
            ("shock", b.shock),
            ("step", b.step),
            ("inv_inertia", b.inv_inertia),
            ("ang_damping", b.ang_damping),
            ("last_quat", b.last_quat),
            ("quat", b.quat),
            ("cofg", b.cofg),
            ("moment", b.moment),
            ("last_moment", b.last_moment),
            ("rates", b.rates),
            ("ang_accel", b.ang_accel),
            ("twist_peak", b.twist_peak),
            ("twist", b.twist),
        ] {
            w.vf(name, v);
        }
        w.f("mass", self.mass);
        w.vf("gravity", self.gravity);
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
        for (k, row) in r.node.rows.iter().enumerate() {
            w.vf(["row0", "row1", "row2"][k], *row);
        }
        w.vf("origin", r.node.origin);
        w.u32("contacts", r.contacts.len() as u32);
    }
}
