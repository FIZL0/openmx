//! **The ragdoll's bone collisions**: what keeps a thrown rider's bones out of the ground.
//!
//! Each of the nine `Collision` constraints in `mrider.phy` is a sphere of the bone's own `Radius`
//! round the bone's centre of mass. The dynamics world solves every constraint once per iteration,
//! and a bone collision's solve does three things in a row:
//!
//! 1. **the ground**: if the bone is over the terrain's box, the ground query under it. A bone whose
//!    centre is less than its radius above the answer is pushed out along the ground's normal;
//! 2. **the scene**: the bone's sphere collider is stood at the centre and collided against the
//!    scene. Every contact it reports is pushed out the same way;
//! 3. **the sweep**: a segment from where the bone was `1/15` s ago (its velocity, backwards) to
//!    where it is now is swept against the scene, so that a fast bone does not pass through a thin
//!    wall between two steps. A hit is pushed out too, and marks the bone *swept*.
//!
//! The push-out ([`Contact::push_out`]) moves the bone's centre out to its radius and keeps
//! `Friction` of the velocity along the surface and none of the velocity into it (or `-Restitution`
//! of it, when it is coming in slower than 5 ft/s). It applies the two corrections as an impulse at
//! the contact, a radius from the centre, so a bone that hits off-centre is also turned; a swept bone
//! takes them straight on its centre instead.
//!
//! Against something that moves (the wreck bike's bodies, another rider, a vehicle) the push-out is
//! the same, but the velocity it takes apart is the bone's *relative to that thing* at the contact
//! ([`moving`]): the bounce and the friction are measured in the other's frame, so a bone lying on a
//! bike that rolls away is carried with it. The other side is not pushed back. The scene's broad
//! phase is not here: [`Scene`] hands the solve whatever the caller's scene found, and says which
//! collidable each contact is with.

use crate::biplane::{Body, unturn};
use crate::camera::CurveTable;
use crate::chassis::{self, Mat3, turn};
use crate::contact::{Mover, Node, Other, Side};
use crate::math::Mat4;
use crate::phy::Ini;
use crate::world::World;
use ps2_float::fpu::{self, Acc};
use ps2_float::{Dest, F, Field, Vf, vu};

/// What the velocity is measured against when the other side of a contact has no body: the
/// constructor's own `(0, 0, 0, 1)`.
const STILL: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);

/// Below this speed into the surface the bounce is `-Restitution` times it; at or above it, the
/// whole of it is kept.
const BOUNCE_BELOW: f32 = 5.0;

/// How far back the sweep starts: `1/15` s of the bone's velocity, a two-ulp-short single the
/// constructor writes as `0x3d888889`.
pub const SWEEP_BACK: f32 = f32::from_bits(0x3d88_8889);

/// **One bone collision's numbers**, out of its `Collision` section and its bone's `Part`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    /// The bone it is for.
    pub bone: usize,
    /// The bone's `Radius`, which the body carries. The sphere collider is built with it too.
    pub radius: F,
    /// `UnpenetrationScale`: how much of the overlap one push-out takes back, and how much of the
    /// new velocity is kept.
    pub unpenetration: F,
    /// `RotationScale`: how far from the centre the impulse is applied, in radii.
    pub rotation: F,
    pub restitution: F,
    pub friction: F,
    /// The sweep is on, and how far back it looks. The constructor sets both and nothing reads them
    /// from the file.
    pub sweep: bool,
    pub sweep_back: F,
}

impl Sphere {
    /// The collisions of a rider file, in file order. A missing key is the loader's default:
    /// `Restitution` 0, the three others 1.
    pub fn read(ini: &Ini) -> Vec<Sphere> {
        let constraints = ini.section("Physics").int("NumberOfConstraints").unwrap_or(0).max(0) as usize;
        (1..=constraints)
            .map(|i| ini.section(&format!("Constraint{i}")))
            .filter(|s| s.text("Type").is_some_and(|t| t.eq_ignore_ascii_case("Collision")))
            .map(|s| {
                let bone = (s.int("Part").unwrap_or(1) - 1).max(0) as usize;
                Sphere {
                    bone,
                    radius: ini.section(&format!("Part{}", bone + 1)).float_or("Radius", F::ZERO),
                    unpenetration: s.float_or("UnpenetrationScale", F::ONE),
                    rotation: s.float_or("RotationScale", F::ONE),
                    restitution: s.float_or("Restitution", F::ZERO),
                    friction: s.float_or("Friction", F::ONE),
                    sweep: true,
                    sweep_back: F(SWEEP_BACK),
                }
            })
            .collect()
    }

    /// A rider file's bytes.
    pub fn read_bytes(bytes: &[u8]) -> Vec<Sphere> {
        Sphere::read(&Ini::parse_bytes(bytes))
    }
}

/// **What one bone collision remembers** between solves. The flags are set and never cleared here:
/// the riding entry clears `touched` ([`crate::ragdoll::Collision::touching`]), and only `swept`
/// starts every solve clear.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Contact {
    /// A push-out has run.
    pub touched: bool,
    /// The ground query pushed the bone out.
    pub ground: bool,
    /// The scene's collider or the sweep reported a hit.
    pub scene: bool,
    /// This solve's sweep hit. A swept bone is pushed without the impulse.
    pub swept: bool,
    /// The last contact: the point the bone was pushed away from and the surface's normal there.
    pub point: Vf,
    pub normal: Vf,
    /// The last push-out's change of velocity.
    pub dv: Vf,
    /// The surface the ground query last wrote (P186): the terrain's nearest cell corner under the
    /// bone's centre ([`World::wheel_surface`]), written by every query, touching or not, and kept
    /// where the query writes nothing. The rider's dust and chunks read it as their sphere's zone.
    pub surface: u8,
}

/// The terrain the ground half asks, and its box. The query is only made for a bone strictly inside
/// the box in `x` and `z`.
pub struct Terrain<'a> {
    pub world: &'a mut dyn World,
    pub min: Vf,
    pub max: Vf,
}

/// **The scene's colliders**, as the bone collision asks them.
pub trait Scene {
    /// The bone's sphere stood at `centre`: `None` when the collider reports no hit, otherwise its
    /// contacts as `(point, normal)`, at most three (the collider's record holds three).
    fn touching(&mut self, centre: Vf, radius: F) -> Option<Vec<(Vf, Vf)>> {
        let _ = (centre, radius);
        None
    }

    /// The segment from `from` to `to`: the first hit's `(point, normal)`.
    fn sweep(&mut self, from: Vf, to: Vf) -> Option<(Vf, Vf)> {
        let _ = (from, to);
        None
    }

    /// Which collidable the `i`-th contact of the last [`Scene::touching`] is with: `None` for the
    /// scenery, which does not move.
    fn other(&mut self, i: usize) -> Option<usize> {
        let _ = i;
        None
    }

    /// How fast collidable `other` moves at the world point `at`: [`moving`] over it, which may also
    /// write the other's collision object (its centre of mass in the world).
    fn velocity(&mut self, other: usize, at: Vf) -> Vf {
        let _ = (other, at);
        STILL
    }
}

/// **What a bone's contact is with**, as the push-out tells the three kinds of other apart.
pub enum Against<'a, 'b> {
    /// The scenery, or a collidable that is neither of the two below: the velocity is `(0, 0, 0, 1)`.
    Still,
    /// A collidable whose owner is a moving object that is not a dynamics body (its class is 2).
    Mover(&'a Mover),
    /// A collidable on a collision object, with a dynamics body or on a vehicle's chassis. This one
    /// wins over a mover when a collidable has both.
    Other(&'a mut Other<'b>),
}

/// **The velocity of the other side of a contact at `at`**, which the push-out measures the bone's
/// against: the other's own velocity plus its spin crossed with the arm from its centre of mass to
/// the point. A collision object's centre of mass in the world ([`crate::contact::Collider::cofg_world`])
/// is written on the way, as the original does.
///
/// Each kind keeps its spin in a different frame, so the arm is taken into that frame and the spin's
/// part turned back out: a body's through its quaternion, a vehicle's and a mover's through their
/// transform node.
pub fn moving(at: Vf, against: Against<'_, '_>) -> Vf {
    match against {
        Against::Still => STILL,
        Against::Mover(m) => {
            let arm = minus(m.node.unplace(at), m.origin);
            plus(m.node.rotate(cross_xyz(m.spin, arm)), m.velocity)
        }
        Against::Other(o) => match &mut o.side {
            Side::Body(b) => {
                let q = b.quat();
                o.collider.cofg_world = turn(q, o.collider.cofg);
                let centre = plus(b.position(), o.collider.cofg_world);
                let arm = unturn(q, minus(at, centre));
                plus(turn(q, cross_xyz(b.rates(), arm)), b.velocity())
            }
            Side::Vehicle(v) => {
                o.collider.cofg_world = v.node.rotate(o.collider.cofg);
                let arm = minus(v.node.unplace(at), plus(v.cofg, o.collider.cofg_world));
                plus(v.node.rotate(cross_xyz(v.rates, arm)), v.velocity)
            }
        },
    }
}

/// `a.xyz + b.xyz`, `w` from `a`.
fn plus(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}

/// `a.xyz - b.xyz`, `w` from `a`.
fn minus(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

/// `a x b`, `.xyz`, `w` from `a`: `vopmula a, b` then `vopmsub b, a`.
fn cross_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, cross(a, b))
}

/// A scene with nothing in it but the terrain.
pub struct Open;

impl Scene for Open {}

impl Contact {
    /// **The solve**: the ground, then the scene, then the sweep, on a bone collision that is `on`.
    /// A disabled body is left alone.
    pub fn solve(
        &mut self,
        on: bool,
        s: &Sphere,
        body: &mut Body,
        terrain: Option<Terrain<'_>>,
        scene: &mut dyn Scene,
        curve: &CurveTable,
    ) {
        if !on || !body.enabled {
            return;
        }
        self.swept = false;
        self.point = body.position;
        if let Some(t) = terrain {
            let p = self.point;
            if p.x() < t.max.x() && !(p.x() <= t.min.x()) && p.z() < t.max.z() && !(p.z() <= t.min.z()) {
                let (point, normal) = t.world.ground(p);
                if let Some(surface) = t.world.wheel_surface(p) {
                    self.surface = surface;
                }
                self.point = point;
                self.normal = normal;
                if F(body.position.y()) < F(point.y()) + s.radius {
                    self.ground = true;
                    self.push_out(s, body, point, normal, curve);
                }
            }
        }
        // What the contact is with: the scenery's for the ground, each scene contact's own, and the
        // sweep's is the last scene contact's (the sweep does not say what it hit).
        let mut other = None;
        if let Some(hits) = scene.touching(body.position, s.radius) {
            self.scene = true;
            for (i, (point, normal)) in hits.into_iter().enumerate() {
                self.point = point;
                self.normal = normal;
                other = scene.other(i);
                let against = other.map_or(STILL, |o| scene.velocity(o, point));
                self.push_out_against(s, body, point, normal, against, curve);
            }
        }
        if s.sweep {
            let back = body.velocity.merge(Dest::XYZ, body.velocity.mul(Vf::splat(s.sweep_back.0)));
            let from = body.position.merge(Dest::XYZ, body.position.sub(back));
            if let Some((point, normal)) = scene.sweep(from, body.position) {
                self.scene = true;
                self.swept = true;
                self.point = point;
                self.normal = normal;
                let against = other.map_or(STILL, |o| scene.velocity(o, point));
                self.push_out_against(s, body, point, normal, against, curve);
            }
        }
    }

    /// **The push-out** against a surface with no body behind it.
    pub fn push_out(&mut self, s: &Sphere, body: &mut Body, point: Vf, normal: Vf, curve: &CurveTable) {
        self.push_out_against(s, body, point, normal, STILL, curve);
    }

    /// **The push-out** against a surface moving at `against` at the contact ([`moving`]): the
    /// bounce and the friction act on the bone's velocity relative to it.
    pub fn push_out_against(&mut self, s: &Sphere, body: &mut Body, point: Vf, normal: Vf, against: Vf, curve: &CurveTable) {
        self.touched = true;
        let pos = body.position;
        let d = pos.merge(Dest::XYZ, pos.sub(point));
        let depth = dot(d, normal);
        let corr = normal.merge(Dest::XYZ, normal.mul(Vf::splat((s.unpenetration * (s.radius - depth)).0)));

        let vel = body.velocity;
        let rel = vel.merge(Dest::XYZ, vel.sub(against));
        let into = dot(rel, normal);
        let vn = normal.merge(Dest::XYZ, normal.mul(Vf::splat(into.0)));
        let vt = rel.merge(Dest::XYZ, rel.sub(vn));
        let bounce = if into < F(BOUNCE_BELOW) { vn.merge(Dest::XYZ, vn.mul(Vf::splat(fpu::neg(s.restitution.0)))) } else { vn };
        let slide = vt.merge(Dest::XYZ, vt.mul(Vf::splat(s.friction.0)));
        let sum = bounce.merge(Dest::XYZ, bounce.add(slide));
        let kept = sum.merge(Dest::XYZ, sum.mul(Vf::splat(s.unpenetration.0)));
        self.dv = kept.merge(Dest::XYZ, kept.sub(rel));

        if self.swept {
            body.position = pos.merge(Dest::XYZ, pos.add(corr));
            body.velocity = vel.merge(Dest::XYZ, vel.add(self.dv));
            return;
        }
        // The arm: from the centre towards the contact, a radius long (times `RotationScale`), in the
        // bone's own frame.
        let a = point.merge(Dest::XYZ, point.sub(pos));
        let sq = a.mul(a);
        let len2 = vu::madd(vu::add(sq.x(), sq.y()), 1.0, sq.z());
        let a = a.merge(Dest::XYZ, a.mul_q(vu::rsqrt(1.0, len2)));
        let a = a.merge(Dest::XYZ, a.mul(Vf::splat((s.rotation * s.radius).0)));
        let arm = unturn(body.quat, a);
        let flip = |v: Vf| Vf::new(fpu::neg(v.x()), fpu::neg(v.y()), fpu::neg(v.z()), v.w());
        impulse(body, arm, flip(corr), flip(self.dv), curve);
    }
}

/// **The bike box**: what holds the main bone on the bike while the rider is riding it.
///
/// The constraint is a box in a frame: the bone's centre is taken into the frame, held inside
/// `centre ± half` axis by axis, and taken back out. An axis it was held on also loses its velocity
/// along that axis, in the frame. The rider body actor points the frame at the bike's drawn
/// transform and `carried` at the bike's velocity (the captured `bike_vel`), which the bone's
/// velocity is measured against so that a bone riding along with the bike is not stopped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BikeBox {
    /// The frame the box is in: rows 0-2 its axes, row 3 its origin.
    pub frame: Mat4,
    /// The box's centre in that frame, and its half-size on each axis.
    pub centre: Vf,
    pub half: Vf,
    /// The frame's own velocity, when it moves. `None` is the constraint's own still frame.
    pub carried: Option<Vf>,
}

impl BikeBox {
    /// **The solve**, on a bike box that is `on`. A disabled body is left alone.
    pub fn hold(&self, on: bool, body: &mut Body) {
        if !on || !body.enabled {
            return;
        }
        let axes = Mat3::new(self.frame.0[0], self.frame.0[1], self.frame.0[2]);
        let origin = self.frame.0[3];
        let vel = match self.carried {
            Some(c) => body.velocity.merge(Dest::XYZ, body.velocity.sub(c)),
            None => body.velocity,
        };
        let p = body.position;
        let p = chassis::unrotate(&axes, p.merge(Dest::XYZ, p.sub(origin)));
        let mut p = p.merge(Dest::XYZ, p.sub(self.centre));
        let mut v = chassis::unrotate(&axes, vel);
        for (i, f) in [Field::X, Field::Y, Field::Z].into_iter().enumerate() {
            let h = self.half.lane(f);
            let at = p.lane(f);
            let held = if !(F(at) <= F(h)) {
                Some(h)
            } else if F(at) < F(fpu::neg(h)) {
                Some(fpu::neg(h))
            } else {
                None
            };
            if let Some(h) = held {
                p[i] = h;
                v[i] = 0.0;
            }
        }
        let p = p.merge(Dest::XYZ, p.add(self.centre));
        // Back out: the origin (plus `vf0.x`, a zero) first, then the three axes.
        let acc = origin.add(Vf::VF0.bc(Field::X));
        let acc = Vf::madd(acc, axes.rows[0], p.bc(Field::X));
        let acc = Vf::madd(acc, axes.rows[1], p.bc(Field::Y));
        body.position = p.merge(Dest::XYZ, Vf::madd(acc, axes.rows[2], p.bc(Field::Z)));
        let v = chassis::rotate(&axes, v);
        body.velocity = match self.carried {
            Some(c) => v.merge(Dest::XYZ, v.add(c)),
            None => v,
        };
    }
}

/// **The bike box in another actor's frame**: the same constraint when it is pointed at another
/// actor's transform node instead of a frame of its own. The box is the constraint's own (`centre`,
/// `half`), and nothing carries the bone's velocity.
///
/// The original keeps one oddity here that the port keeps too: having clamped the bone's centre and
/// zeroed the held axes of its velocity in the node's frame, it writes back the *clamped centre*,
/// turned out of the node's frame, as the velocity. The zeroed velocity is never used.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeBox {
    pub node: Node,
    pub centre: Vf,
    pub half: Vf,
}

impl NodeBox {
    /// **The solve**, on a bike box that is `on`. A disabled body is left alone.
    pub fn hold(&self, on: bool, body: &mut Body) {
        if !on || !body.enabled {
            return;
        }
        let p = self.node.unplace(body.position);
        let mut p = p.merge(Dest::XYZ, p.sub(self.centre));
        for (i, f) in [Field::X, Field::Y, Field::Z].into_iter().enumerate() {
            let h = self.half.lane(f);
            let at = p.lane(f);
            if !(F(at) <= F(h)) {
                p[i] = h;
            } else if F(at) < F(fpu::neg(h)) {
                p[i] = fpu::neg(h);
            }
        }
        let p = p.merge(Dest::XYZ, p.add(self.centre));
        body.position = self.node.place(p);
        body.velocity = self.node.rotate(p);
    }
}

/// `a.xyz . b.xyz` the way the FPU takes it: one `mula`, one `madda`, one `madd`.
fn dot(a: Vf, b: Vf) -> F {
    Acc::mula(F(a.x()), F(b.x())).madda(F(a.y()), F(b.y())).madd(F(a.z()), F(b.z()))
}

/// The `w` lane's sum of `s`'s three lanes, started off `base`: `base + x`, then `+ 1 * y`, `+ 1 * z`.
fn sum3(base: f32, s: Vf) -> f32 {
    vu::madd(vu::madd(vu::add(base, s.x()), 1.0, s.y()), 1.0, s.z())
}

/// `a x b` as the impulse takes it: `vopmula a, b` then `vopmsub b, a`.
fn cross(a: Vf, b: Vf) -> Vf {
    Vf::opmsub(Vf::opmula(a, b), b, a)
}

/// **The impulse at the arm** that moves the body by `-shift` and changes its velocity by `-kick`
/// (the push-out hands both in negated), each split between the centre and the turn the way one
/// impulse at a lever is: `j = -v / (inv_mass + n.((I^-1 (arm x n)) x arm))` along `v`'s
/// direction `n` in the body's frame. The shift's turn is applied to the quaternion through the
/// curve table; the kick's goes into the rates.
///
/// The original keeps the body's inverse mass in the `x` of a scratch quadword whose other lanes
/// the constructor zeroes and nothing writes; its `w` starts each sum. Here that is `0`.
fn impulse(body: &mut Body, arm: Vf, shift: Vf, kick: Vf, curve: &CurveTable) {
    const BASE: f32 = 0.0;
    let m = body.inv_mass.0;
    let q = body.quat;
    let ii = body.inv_inertia;
    let minus_one = Vf::VF0.sub(Vf::VF0.bc(Field::W));
    // One impulse: its size along `v`'s direction, as `j`, and its turn, as `jl x arm` scaled.
    let along = |v: Vf| -> Vf {
        let n = v.mul_q(vu::rsqrt(1.0, sum3(BASE, v.mul(v))));
        let nl = unturn(q, n);
        let c = cross(arm, nl).mul(ii);
        let e = cross(c, arm);
        let k = vu::add(sum3(BASE, nl.mul(e)), m);
        let v = v.merge(Dest::XYZ, v.mul(minus_one));
        v.mul_q(vu::div(1.0, k))
    };
    let mv = Vf::splat(m);

    let j = along(shift);
    body.position = body.position.merge(Dest::XYZ, body.position.add(j.mul(mv)));
    let turned = turn(q, cross(arm, unturn(q, j)).mul(ii));
    let ang = vu::add(BASE, vu::sqrt(sum3(BASE, turned.mul(turned))));
    let axis = turned.mul_q(vu::div(1.0, ang));

    let j = along(kick);
    body.velocity = body.velocity.merge(Dest::XYZ, body.velocity.add(j.mul(mv)));
    let r = cross(arm, unturn(q, j)).mul(ii);
    body.rates = body.rates.merge(Dest::XYZ, body.rates.add(r));

    // The shift's turn: `ang` about `axis`, as the quaternion `(axis sin(ang/2), cos(ang/2))`
    // multiplied on the left.
    let (cos, sin) = curve.at(F(0.5) * F(ang));
    let r = axis.mul(Vf::splat(sin.0)).merge(Dest::W, Vf::VF0.mul(Vf::splat(cos.0)));
    let p = r.mul(q);
    let acc = Vf::opmula(r, q);
    let acc = Vf::madd(acc, r, q.bc(Field::W));
    let acc = Vf::madd(acc, q, r.bc(Field::W));
    let xyz = Vf::opmsub(acc, q, r);
    let w = vu::msub(vu::msub(vu::sub(p.w(), p.x()), 1.0, p.y()), 1.0, p.z());
    body.quat = xyz.merge(Dest::W, Vf::splat(w));
}
