//! **The rider body actor's tick** (S61): everything the original's update does round the ragdoll's
//! step, in its order.
//!
//! [`crate::ragdoll`] has the ragdoll's own pieces: the two entries, the throw's clock, the damping
//! switch and the step. [`crate::ragdoll_ride::Down`] ran them with a fixed drive and a settle gate
//! read off the bones. This is what the original wraps them in, once a tick:
//!
//! 1. **the model's effects**: the request's glow and its fade-out;
//! 2. **the look clocks**: two angles eased towards their targets at one of two rates;
//! 3. **the mode machine** ([`Actor::mode`]), while the request is live: which of five things poses
//!    the body, the entry of the one it has just changed into, the feet's holds and collisions
//!    switched for the look-around, the riding pose's damper scaled by speed, and in the wreck the
//!    throw's clock, the roll, the bones coming off the bike, the ground under the main bone and the
//!    settle test ([`wreck_arm`]). Then every hold's target is copied off the request;
//! 4. **the physics update**: the world stepped while the body is placed and active, every bone's
//!    orientation put back on the unit sphere, the repose;
//! 5. **the tail**: what the request is handed back ([`Tail`]).
//!
//! The wreck's path (the mode machine in the wreck, the step and the tail) is held against the
//! original tick for tick on the S56 captures (`tests/ragdoll_wrecks.rs`). The rest is ported from
//! the same functions and not proved: no capture has a rider who is not down.
//!
//! What the actor asks of the rest of the game and the port does not have is [`Outside`]: the
//! skeleton the bones are posed at, the look-around's reset and the wreck sound.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::chassis::{normalize, turn};
use crate::math::{self, Mat4};
use crate::phy::Wrecks;
use crate::ragdoll::{Among, Bike, Carrier, Drive, MAIN, PARTS, Ragdoll, Tables, product};
use crate::ragdoll_contact::{BikeBox, Contact, Scene, Sphere};
use crate::rider_anim::Mode;
use crate::rng::Rng;
use crate::world::World;
use ps2_float::{Dest, F, Vf, fpu};

/// `(0, 0, 0, 1)`.
const UNIT_W: Vf = Vf([0.0, 0.0, 0.0, 1.0]);

/// What the hold velocity is of the request's velocity: `0.8`, as its bit pattern.
const CARRIED: F = F(f32::from_bits(0x3f4c_cccd));

/// The clip the feet's look-around switch looks for.
const LOOK_CLIP: i32 = 30;

/// The feet's holds and the bone collisions the look-around switches with them: the first foot's
/// hold goes with the second collision, the second's with the first.
const FOOT_HOLDS: [usize; 2] = [2, 3];
const FOOT_COLLISIONS: [usize; 2] = [1, 0];

/// How long a foot's hold takes to fade back in once the look-around lets it go, in seconds.
const FOOT_FADE: F = F(2.0);

/// The order the tail adds the touching collisions' velocity changes in: the actor's named
/// collision fields' order, as indices into the collisions.
const IMPACT_ORDER: [usize; 9] = [2, 3, 4, 6, 5, 8, 7, 1, 0];

/// **What the actor reads off the riding request**, each tick. The request is the bike's: the rider's
/// owner fills it in before the actor's update.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Request {
    /// The request is live: the mode machine runs. Otherwise the body is not placed.
    pub live: bool,
    /// Whoever owns the request is placing the body ([`Mode::Held`]).
    pub held: bool,
    /// The bike's orientation as three rows (what the riding pose is carried round with).
    pub rows: [Vf; 3],
    /// The bike's drawn transform: the bike box's frame.
    pub frame: Mat4,
    pub velocity: Vf,
    /// The bike's rates. `y` is what the look-around reads, `z` the spin the riding pose is carried
    /// round by.
    pub rates: Vf,
    /// Where the two hands' holds hold: the bike's hand dummies.
    pub hands: [Vf; 2],
    /// Where the two feet's holds hold, before the feet's own offsets.
    pub feet: [Vf; 2],
    /// The bike box's centre and half-size, when the request points at both.
    pub bike_box: Option<(Vf, Vf)>,
    /// The look-around is wanted whatever the speed.
    pub looking: bool,
    /// The step's pose off the clip table is in charge. Latched: the actor keeps it.
    pub posed: bool,
    /// The wreck's reason, zero while riding.
    pub reason: i32,
    /// The start line's cut is on ([`Mode::Gate`]).
    pub cut: bool,
    /// What the wreck entry hands the bones for some reasons.
    pub hit_velocity: Vf,
    /// The look-around is off for this one.
    pub tucked: bool,
    /// The two numbers the look-around's feet switch reads, and the speed the pose damper is scaled
    /// by.
    pub pitch: F,
    pub speed: F,
    /// The point the riding pose's spin is about, and the bike's orientation.
    pub centre: Vf,
    pub quat: Vf,
    /// The bike's collider: whether there is one, and its centre and radius. The bones are off the
    /// bike once the main bone is a foot past it.
    pub bike: Option<(Vf, F)>,
    /// The model's effect: 2 glows, 1 stops glowing, anything else leaves it.
    pub effect: i32,
    /// The model's fade-out, while positive: the alpha runs from one down to a tenth as it goes from
    /// `fade_from` to zero.
    pub fade: F,
    pub fade_from: F,
}

impl Default for Request {
    fn default() -> Request {
        Request {
            live: true,
            held: false,
            rows: [Vf([1.0, 0.0, 0.0, 0.0]), Vf([0.0, 1.0, 0.0, 0.0]), Vf([0.0, 0.0, 1.0, 0.0])],
            frame: Mat4([Vf([1.0, 0.0, 0.0, 0.0]), Vf([0.0, 1.0, 0.0, 0.0]), Vf([0.0, 0.0, 1.0, 0.0]), UNIT_W]),
            velocity: UNIT_W,
            rates: UNIT_W,
            hands: [UNIT_W; 2],
            feet: [UNIT_W; 2],
            bike_box: None,
            looking: false,
            posed: false,
            reason: 0,
            cut: false,
            hit_velocity: UNIT_W,
            tucked: false,
            pitch: F::ZERO,
            speed: F::ZERO,
            centre: UNIT_W,
            quat: UNIT_W,
            bike: None,
            effect: 0,
            fade: F::ZERO,
            fade_from: F::ZERO,
        }
    }
}

/// What the skeleton hands the step: each bone's point in the riding pose and its rest orientation.
/// The mode machine reads the main bone's rest before the skeleton's own update this tick, the step
/// after it: `main_rest` is the first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Skeleton {
    pub points: [Vf; PARTS],
    pub rests: [Vf; PARTS],
    pub main_rest: Vf,
}

impl Default for Skeleton {
    fn default() -> Skeleton {
        Skeleton { points: [UNIT_W; PARTS], rests: [UNIT_W; PARTS], main_rest: UNIT_W }
    }
}

/// **What the actor asks of the rest of the game** that this crate does not have.
pub trait Outside {
    /// Every bone posed where the animated skeleton has it, then reset there. `carried` is the
    /// original's `a1`, which the repose sets: the bones are posed still. Clear (the entries), each
    /// takes the velocity and spin of its node's move since the node was last drawn (S110).
    fn pose(&mut self, ragdoll: &mut Ragdoll, dt: F, carried: bool) {
        let _ = (ragdoll, dt, carried);
    }
    /// The look-around and the placement reset.
    fn look_reset(&mut self) {}
    /// The wreck entry's last call, which draws.
    fn wreck_sound(&mut self, reason: i32, rng: &mut Rng) {
        let _ = (reason, rng);
    }
}

/// An outside with none of it.
pub struct Nothing;

impl Outside for Nothing {}

/// The model's effects.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Model {
    pub glow: bool,
    pub glow_level: F,
    pub alpha: F,
}

/// **The look clocks**: two angles, each eased towards its target at the fast rate while it is on and
/// the slow one while it is off, no further a tick than the rate times the tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Look {
    pub on: [bool; 2],
    pub fast: [F; 2],
    pub slow: [F; 2],
    pub target: [F; 2],
    pub at: [F; 2],
}

impl Look {
    fn tick(&mut self, dt: F) {
        let rate: [F; 2] = std::array::from_fn(|k| (if self.on[k] { self.fast[k] } else { self.slow[k] }) * dt);
        for (k, r) in rate.into_iter().enumerate() {
            let d = fpu::min(fpu::max(fpu::sub(self.target[k].0, self.at[k].0), fpu::neg(r.0)), r.0);
            self.at[k] = self.at[k] + d;
        }
    }
}

/// The torques' fade: its length, and what is left of it. A stiffening restarts it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fade {
    pub length: F,
    pub left: F,
}

/// The start line's own switch, on an actor whose owner has one: the cut that is on now and the one
/// that was on the last tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cut {
    pub now: bool,
    pub was: bool,
}

/// **What the tail hands back to the request.**
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tail {
    /// Something of the ragdoll was touching when the mode machine looked.
    pub touching: bool,
    /// `(0, 0, 0, 1)` plus the velocity changes of the step's touching collisions, when touching.
    pub impact: Vf,
    /// The main bone's position.
    pub main: Vf,
    /// The pair of flags the physics update sets: the body is driven by the skeleton this tick.
    pub driven: bool,
}

/// **The rider body actor**, as much of it as its update reads and writes.
#[derive(Clone, Debug, PartialEq)]
pub struct Actor {
    pub ragdoll: Ragdoll,
    /// One per bone collision.
    pub contacts: Vec<Contact>,
    /// What poses the body, and what posed it the last time the mode changed.
    pub mode: Mode,
    pub prev: Mode,
    /// The posing flag the request latches, and the idle that keeps the body out of riding.
    pub posed: bool,
    pub latched: bool,
    pub idle: bool,
    /// The look-around's pose playing, `-1` for none: the feet switch runs only then.
    pub look_pose: i32,
    /// The clip the body plays.
    pub clip: i32,
    /// Something of the ragdoll is touching.
    pub touching: bool,
    pub fade: Fade,
    /// The feet's fades back in, and the weight each foot's hold is at.
    pub foot_fade: [F; 2],
    pub foot_weight: [F; 2],
    pub look: Look,
    /// The riding pose's spring and damper; the damper is its base scaled by speed.
    pub pose_spring: Vf,
    pub pose_damping: Vf,
    pub pose_damping_base: Vf,
    /// The speed the damper is scaled up to, and the one under which the look-around may lift a foot.
    pub stand_speed: F,
    pub slow_speed: F,
    /// The start line's step and its breathe clock, which entering the gate restarts.
    pub gate_step: i32,
    pub gate_clock: F,
    /// The feet's offsets from the request's points, turned by the bike.
    pub foot_points: [Vf; 2],
    /// Where the four holds hold, and the velocity they follow.
    pub hold_targets: [Vf; 4],
    pub hold_velocity: Vf,
    /// The bike box's centre and half-size, and its frame.
    pub bike_box: (Vf, Vf),
    pub bike_frame: Mat4,
    /// The physics update's three flags.
    pub placed: bool,
    pub active: bool,
    pub repose: bool,
    pub cut: Option<Cut>,
    pub impact: Vf,
    pub model: Model,
    pub gravity_scale: F,
    pub torques: bool,
}

impl Actor {
    /// An actor over a ragdoll, with the constructor's numbers and the live ones the S56 states
    /// hold.
    pub fn new(ragdoll: Ragdoll, collisions: usize) -> Actor {
        Actor {
            ragdoll,
            contacts: vec![Contact::default(); collisions],
            mode: Mode::Held,
            prev: Mode::Held,
            posed: false,
            latched: false,
            idle: false,
            look_pose: -1,
            clip: 0,
            touching: false,
            fade: Fade { length: F::ONE, left: F::ZERO },
            foot_fade: [F::ZERO; 2],
            foot_weight: [F::ONE; 2],
            look: Look::default(),
            pose_spring: UNIT_W,
            pose_damping: UNIT_W,
            pose_damping_base: UNIT_W,
            stand_speed: F::ZERO,
            slow_speed: F::ZERO,
            gate_step: 0,
            gate_clock: F::ZERO,
            foot_points: [UNIT_W; 2],
            hold_targets: [UNIT_W; 4],
            hold_velocity: UNIT_W,
            bike_box: (UNIT_W, UNIT_W),
            bike_frame: Mat4([UNIT_W; 4]),
            placed: false,
            active: true,
            repose: false,
            cut: None,
            impact: UNIT_W,
            model: Model::default(),
            gravity_scale: F::ONE,
            torques: true,
        }
    }

    /// **One tick of the actor's update**, in the original's order: the model's effects, the look
    /// clocks, the mode machine while the request is live, the physics update and the tail. The
    /// skeleton's own update and the animation's selection, which the physics update calls in front
    /// of the step, are the caller's.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        dt: F,
        r: &Request,
        skeleton: &Skeleton,
        tables: &Tables,
        wrecks: &Wrecks,
        scenery: Scenery<'_, '_>,
        rng: &mut Rng,
        outside: &mut dyn Outside,
    ) -> Tail {
        self.effects(r);
        self.look.tick(dt);
        let Scenery { spheres, world, terrain, scene } = scenery;
        if r.live {
            self.mode(dt, r, skeleton, wrecks, &mut *world, rng, outside);
        } else {
            self.placed = false;
        }
        let driven = self.physics(dt, r, skeleton, tables, Scenery { spheres, world, terrain, scene }, outside);
        self.tail(driven)
    }

    /// The request's effects on the model.
    fn effects(&mut self, r: &Request) {
        match r.effect {
            2 => {
                self.model.glow = true;
                self.model.glow_level = F(fpu::div(160.0, 100.0));
                self.model.alpha = F(0.5);
            }
            1 => {
                self.model.glow = false;
                self.model.glow_level = F::ZERO;
            }
            _ => {}
        }
        if F::ZERO < r.fade {
            self.model.alpha = ramp(r.fade, r.fade_from, F::ZERO, F(0.1), F::ONE);
        }
    }

    /// **The mode machine**: the mode, its entry, the feet's look-around switch, the pose damper,
    /// the riding body's main bone, the placement, the wreck's arm and the holds' targets.
    #[allow(clippy::too_many_arguments)]
    pub fn mode(
        &mut self,
        dt: F,
        r: &Request,
        skeleton: &Skeleton,
        wrecks: &Wrecks,
        world: &mut dyn World,
        rng: &mut Rng,
        outside: &mut dyn Outside,
    ) {
        if r.posed {
            self.latched = true;
        }
        let mode = if r.held {
            Mode::Held
        } else if r.reason != 0 {
            Mode::Wrecked
        } else if self.posed || self.latched {
            Mode::Posed
        } else if !r.cut && !self.idle {
            Mode::Riding
        } else {
            Mode::Gate
        };
        self.mode = mode;
        let prev = self.prev;
        let doll = &mut self.ragdoll;
        if mode == Mode::Held && prev != Mode::Held {
            doll.entered = false;
            self.prev = mode;
        } else if mode == Mode::Gate && prev != Mode::Gate {
            doll.entered = false;
            self.gate_step = 0;
            outside.look_reset();
            self.gate_clock = rng.unit() * F(0.5);
            self.prev = mode;
        } else if mode == Mode::Wrecked && prev != Mode::Wrecked {
            outside.pose(doll, dt, false);
            let bike = Bike { velocity: r.velocity, rates: r.rates, hit_velocity: r.hit_velocity };
            doll.wreck_enter(r.reason, &bike, wrecks, rng);
            outside.wreck_sound(r.reason, rng);
            self.prev = mode;
        } else if mode == Mode::Wrecked || prev != Mode::Wrecked {
            if mode == Mode::Posed && prev != Mode::Posed {
                doll.entered = false;
                outside.look_reset();
                self.prev = mode;
            } else if mode == Mode::Riding && prev != Mode::Riding {
                if !self.placed {
                    outside.pose(doll, dt, false);
                }
                doll.ride_enter(r.bike.is_some());
                self.prev = mode;
            }
        } else {
            // Up again for riding or posing: the wreck's flags go, and that mode's own entry does not
            // run (the original's quirk).
            doll.flung = false;
            doll.settled = false;
            doll.roll = F::ZERO;
            doll.launch.force_time = F(-1.0);
            self.prev = mode;
        }

        if self.look_pose == -1 && mode != Mode::Wrecked && !self.posed {
            self.feet(dt, r);
        }
        let s = ramp(r.speed, F::ZERO, self.stand_speed, F(2.0), F::ONE);
        let base = self.pose_damping_base;
        self.pose_damping = base.merge(Dest::XYZ, base.mul(Vf::splat(s.0)));
        if mode != Mode::Wrecked {
            self.ragdoll.bones[MAIN].body.quat = product(r.quat, skeleton.main_rest);
        }
        if !self.repose {
            self.repose = r.held;
        }
        self.placed = self.ragdoll.entered;
        if let Some(cut) = self.cut.as_mut() {
            if self.placed && mode != Mode::Wrecked {
                if !cut.now {
                    self.placed = false;
                } else if !cut.was {
                    outside.look_reset();
                    outside.pose(&mut self.ragdoll, dt, false);
                }
            }
            cut.was = cut.now;
        }
        if mode == Mode::Wrecked {
            wreck_arm(
                &mut self.ragdoll,
                &self.contacts,
                &mut self.touching,
                &mut self.fade,
                dt,
                r.quat,
                r.bike,
                r.reason,
                wrecks,
                world,
            );
        }
        for c in &mut self.contacts {
            clear(c);
        }
        for c in &mut self.ragdoll.collisions {
            c.touching = false;
        }
        let v = r.velocity;
        self.hold_velocity = v.merge(Dest::XYZ, v.mul(Vf::splat(CARRIED.0)));
        self.hold_targets[1] = r.hands[1];
        self.hold_targets[0] = r.hands[0];
        let foot = |k: usize| {
            let t = turn(r.quat, self.foot_points[k]);
            r.feet[k].merge(Dest::XYZ, r.feet[k].sub(t))
        };
        self.hold_targets[3] = foot(1);
        self.hold_targets[2] = foot(0);
        if let Some(b) = r.bike_box {
            self.bike_box = b;
        }
        self.bike_frame = r.frame;
    }

    /// **The feet's look-around switch**: a slow bike tipped one way lifts a foot off its hold (and
    /// that foot's collision on), and a foot let go of fades back in over two seconds.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn feet(&mut self, dt: F, r: &Request) {
        let doll = &mut self.ragdoll;
        let (_, up) = math::quat_axes(r.quat);
        let up_y = up[1];
        let free = r.looking
            || (!r.tucked
                && !(r.speed < F::ZERO)
                && (!(F(fpu::abs(r.rates.y())) < F(1.5)) || !(r.speed < self.stand_speed)));
        if !free {
            doll.holds[FOOT_HOLDS[1]] = false;
            doll.collisions[FOOT_COLLISIONS[1]].enabled = true;
            doll.holds[FOOT_HOLDS[0]] = false;
            doll.collisions[FOOT_COLLISIONS[0]].enabled = true;
            self.foot_fade[1] = F(0.5);
            self.foot_fade[0] = F(0.5);
            return;
        }
        let slow = self.clip == LOOK_CLIP && r.speed < self.slow_speed;
        if slow && !(r.pitch <= F(0.7)) && up_y < F(0.9) {
            self.lift(1, 0);
        } else if slow && r.pitch < F(0.3) && up_y < F(0.9) {
            self.lift(0, 1);
        } else {
            let doll = &mut self.ragdoll;
            doll.holds[FOOT_HOLDS[1]] = true;
            doll.collisions[FOOT_COLLISIONS[1]].enabled = false;
            doll.holds[FOOT_HOLDS[0]] = true;
            doll.collisions[FOOT_COLLISIONS[0]].enabled = false;
            for k in 0..2 {
                if !(self.foot_fade[k] <= F::ZERO) {
                    self.foot_fade[k] = self.foot_fade[k] - dt;
                    let w = ramp(self.foot_fade[k], F::ZERO, FOOT_FADE, F::ONE, F::ZERO);
                    self.foot_weight[k] = w;
                    self.ragdoll.pins[FOOT_HOLDS[k]].gains[2] = w.0;
                }
            }
        }
    }

    /// Foot `off` lifted, its collision on and its fade restarted; foot `on` held at full weight.
    fn lift(&mut self, off: usize, on: usize) {
        let doll = &mut self.ragdoll;
        doll.holds[FOOT_HOLDS[off]] = false;
        doll.collisions[FOOT_COLLISIONS[off]].enabled = true;
        self.foot_fade[off] = FOOT_FADE;
        doll.holds[FOOT_HOLDS[on]] = true;
        self.foot_weight[on] = F::ONE;
        doll.pins[FOOT_HOLDS[on]].gains[2] = 1.0;
        doll.collisions[FOOT_COLLISIONS[on]].enabled = false;
    }

    /// **The physics update**: the world stepped while the body is placed and active and not to be
    /// reposed, every bone's orientation normalised after it, then the repose. Says whether the body
    /// is driven by the skeleton.
    fn physics(
        &mut self,
        dt: F,
        r: &Request,
        skeleton: &Skeleton,
        tables: &Tables,
        scenery: Scenery<'_, '_>,
        outside: &mut dyn Outside,
    ) -> bool {
        let driven = !(self.placed && self.active && !self.ragdoll.riding && !self.repose);
        if self.placed && self.active && !self.repose {
            let drive = Drive {
                bike: Carrier { rows: r.rows, spin: F(r.rates.z()), velocity: r.velocity, centre: r.centre, quat: r.quat },
                points: skeleton.points,
                rests: skeleton.rests,
                hold_targets: self.hold_targets,
                hold_velocity: self.hold_velocity,
                gravity_scale: self.gravity_scale,
                pose_spring: self.pose_spring,
                pose_damping: self.pose_damping,
                torques: self.torques,
                mode: self.mode as i32,
                fade_length: self.fade.length,
                fade_left: self.fade.left,
            };
            let Scenery { spheres, world, terrain, scene } = scenery;
            let bike_box =
                BikeBox { frame: self.bike_frame, centre: self.bike_box.0, half: self.bike_box.1, carried: Some(self.hold_velocity) };
            self.ragdoll.step_among(
                dt,
                &drive,
                tables,
                Among { spheres, contacts: &mut self.contacts, terrain: Some((world, terrain.0, terrain.1)), scene, bike_box: Some(bike_box) },
            );
            for b in &mut self.ragdoll.bones {
                b.body.quat = normalize(b.body.quat);
            }
        }
        if self.repose {
            outside.pose(&mut self.ragdoll, dt, true);
            self.repose = false;
        }
        driven
    }

    /// **The tail**: the touching flag and the step's impact, and the main bone's position.
    fn tail(&mut self, driven: bool) -> Tail {
        self.impact = if self.touching { impact(&self.contacts) } else { UNIT_W };
        Tail { touching: self.touching, impact: self.impact, main: self.ragdoll.bones[MAIN].body.position, driven }
    }
}

/// What the step's bone collisions solve against: the spheres, the ground and its box, the scene.
pub struct Scenery<'a, 'w> {
    pub spheres: &'a [Sphere],
    pub world: &'a mut (dyn World + 'w),
    /// The terrain's box, `(min, max)`: the bone collisions ask the ground only inside it.
    pub terrain: (Vf, Vf),
    pub scene: &'a mut dyn Scene,
}

/// A collision's four flags cleared, as each tick's mode machine leaves them for the step.
fn clear(c: &mut Contact) {
    c.touched = false;
    c.ground = false;
    c.scene = false;
    c.swept = false;
}

/// `(0, 0, 0, 1)` plus each touching collision's velocity change, `x`, `y` and `z`, in the actor's
/// order.
pub fn impact(contacts: &[Contact]) -> Vf {
    IMPACT_ORDER
        .iter()
        .filter_map(|&k| contacts.get(k))
        .filter(|c| c.touched)
        .fold(UNIT_W, |sum, c| sum.merge(Dest::XYZ, sum.add(c.dv)))
}

/// **The original's clamped ramp**: `y0` at `x0` to `y1` at `x1`, held between the two, and `y0`
/// when the ramp has no length. Its instructions: the fraction, then `y0 + fraction * (y1 - y0)`
/// through the accumulator, then a max and a min.
pub fn ramp(x: F, x0: F, x1: F, y0: F, y1: F) -> F {
    if x0 == x1 {
        return y0;
    }
    let t = fpu::div(fpu::sub(x.0, x0.0), fpu::sub(x1.0, x0.0));
    let span = fpu::sub(y1.0, y0.0);
    let (lo, hi) = (fpu::min(y0.0, y1.0), fpu::max(y0.0, y1.0));
    let y = fpu::madd(fpu::adda(0.0, y0.0), t, span);
    F(fpu::min(fpu::max(y, lo), hi))
}

/// The damping switch with the fade: a stiffening restarts the torques' fade.
fn damp(doll: &mut Ragdoll, fade: &mut Fade, stiff: bool, reason: i32) {
    doll.damping(stiff, reason);
    if stiff {
        fade.left = fade.length;
    }
}

/// `|v|` the way the wreck's arm takes it: `x² + y² + z²` through the FPU's accumulator, then the
/// square root.
fn length(v: Vf) -> F {
    let acc = fpu::mula(v.x(), v.x());
    let acc = fpu::madda(acc, v.y(), v.y());
    F(fpu::sqrt(fpu::madd(acc, v.z(), v.z())))
}

/// **The wreck's arm of the mode machine**, once a tick while wrecked: the throw's clock, the roll
/// (how long something of the ragdoll has been touching), the bones off the bike once the main bone
/// is a foot past the bike's collider (`bike`, its centre and radius; `None` never), the ground
/// under the main bone, and the settle test:
///
/// * still being thrown, rolling, or within `RiderFlailHeight` of the ground: a body not flung
///   comes to rest (stiff, and the main bone at `GroundAngularRateDamping`) once it has rolled for
///   `RiderRollTime` or is slower than `RiderRollSpeed`; a flung body landing goes limp;
/// * otherwise it is flung: stiff, and no longer at rest.
///
/// Then the torques' fade runs down. `touching` is the actor's flag; the contacts are the last
/// step's.
#[allow(clippy::too_many_arguments, clippy::neg_cmp_op_on_partial_ord)]
pub fn wreck_arm(
    doll: &mut Ragdoll,
    contacts: &[Contact],
    touching: &mut bool,
    fade: &mut Fade,
    dt: F,
    quat: Vf,
    bike: Option<(Vf, F)>,
    reason: i32,
    wrecks: &Wrecks,
    world: &mut dyn World,
) {
    doll.throw_clock(dt, quat);
    if contacts.iter().any(|c| c.touched) {
        *touching = true;
        doll.roll = doll.roll + dt;
    } else {
        *touching = false;
        doll.roll = F::ZERO;
    }
    if let (false, Some((centre, radius))) = (doll.off_bike, bike) {
        let p = doll.bones[MAIN].body.position;
        let d = p.merge(Dest::XYZ, p.sub(centre));
        if !(length(d) <= F::ONE + radius) {
            doll.off_bike = true;
            // Every bone collision's sphere takes the bike off its ignore list (S105).
            doll.ignores_bike = false;
        }
    }
    let main = doll.bones[MAIN].body;
    let (ground, _) = world.ground(main.position);
    let roll = doll.roll;
    if roll != F::ZERO
        || F::ZERO < doll.launch.hand_hold
        || F::ZERO < doll.launch.force_time
        || doll.flung
        || F(main.position.y()) - F(ground.y()) <= wrecks.rider_flail_height
    {
        if roll <= F::ZERO || !doll.flung {
            if (wrecks.rider_roll_time < roll || length(main.velocity) < wrecks.rider_roll_speed) && !doll.settled {
                doll.settled = true;
                damp(doll, fade, true, reason);
                doll.bones[MAIN].body.ang_damping = wrecks.ground_angular_rate_damping;
            }
        } else {
            damp(doll, fade, false, reason);
            doll.flung = false;
        }
    } else {
        damp(doll, fade, true, reason);
        doll.flung = true;
        doll.settled = false;
    }
    if F::ZERO < fade.left {
        fade.left = fade.left - dt;
    }
}
