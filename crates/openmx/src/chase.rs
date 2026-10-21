//! What the chase camera is handed each frame: the machine with the controller, seen the way the
//! original's per-frame target writer sees it (P34).
//!
//! The race camera does not read the vehicle. It reads a small target block the vehicle fills once a
//! frame (`research/oracle.txt`, `vehicle_camera_target`): the seat position, the chassis' forward
//! and up rows, the velocity, the lean, a flag and the speed. The flag is the one that is easy to get
//! backwards. It is stored as `on_ground == 0` — the rider is in the *air* — and it is what tells the
//! camera to head along the velocity rather than along the bike's forward row. Handed the on-ground
//! flag instead, the camera took its heading off the forward row through every jump, and a backflip
//! turns that row over: the view swung round to the front of the bike at the top of the flip and
//! back again on the way down.
//!
//! This file is std and `game_sim` only, for the same reason `pad.rs` is: the ride test compiles it
//! in by path, because a binary crate has nothing a test can `use`.

use game_sim::camera::{Ridden, Target};
use game_sim::ride::{Crash, Ride};
use ps2_float::{F, Vf};

use crate::vehicle::{Garage, Kind};

/// The camera's target for whichever machine is up: the bike's ride, or the parked plane.
pub fn ridden(garage: &Garage, ride: &Ride) -> Ridden {
    match (garage.up, &garage.plane) {
        // The plane, as the camera's target: where it is, its nose, its up, and no lean — a plane
        // has no rider's stick to lean on. Which of the plane's fields the original's plane camera
        // reads was not traced; these are its body's (P31). Its flag is the same one: in the recording2 biplane save state the plane stands on its wheels and the block's
        // flag reads 0.
        (Kind::Biplane, Some(plane)) => Ridden {
            target: Target {
                body_point: plane.at,
                forward: plane.facing,
                up: plane.up,
                vel: plane.vel,
                lean: F::ZERO,
            },
            airborne: !plane.on_ground(),
            speed: plane.speed(),
        },
        _ => bike(ride),
    }
}

/// The bike's target block. The rows are the chassis quaternion's, which is the transform the
/// original's writer reads them from (the vehicle's `+0x18` node, P32's `*(bike+0x18) ==
/// *(chassis+0x254)`); the speed is the chassis' own `+0x34`, floored at zero as the writer floors
/// it (a length never goes below).
pub fn bike(ride: &Ride) -> Ridden {
    let (forward, up) = ride.axes();
    Ridden {
        target: Target {
            body_point: ride.position(),
            forward,
            up,
            vel: ride.chassis.vel,
            lean: ride.held.lean.0,
        },
        airborne: !ride.air.on_ground,
        speed: ride.chassis.speed,
    }
}

/// The target block while the rider is down (S53): the wreck body's place and speed, so the camera
/// follows the bike and the thrown rider rather than the spot the wreck began. The rows are the
/// wreck's flat heading and the world's up, not the tumbling body's own: which rows the original's
/// writer reads off a wrecked bike was not traced, and a camera turned by every tumble is unwatchable.
pub fn wrecked(crash: &Crash) -> Ridden {
    let v = crash.body.vel;
    let speed = F((v.x() * v.x() + v.y() * v.y() + v.z() * v.z()).sqrt());
    Ridden {
        target: Target {
            body_point: crash.origin,
            forward: crash.facing,
            up: ps2_float::Vf::new(0.0, 1.0, 0.0, 0.0),
            vel: v,
            lean: F::ZERO,
        },
        airborne: false,
        speed,
    }
}

// --- the wreck camera (V31) ------------------------------------------------------------------------

/// What the wreck camera follows while the rider is down: the rider's body first, and the wrecked bike.
///
/// The original's wreck camera holds a list of targets, and a wreck leaves two on it: the rider body
/// actor's camera interface (the primary, the one a target with its "primary" word set is swapped to
/// the front of the list for) and the bike's own camera block. The body's point is a point on the
/// ragdoll's upper body. Measured in PCSX2 it sits within 0.05 to 0.15 ft of `[Part4]`'s node once
/// the ragdoll slows down, and within about 0.7 ft while it flies. The writer of that point was
/// not found. A rider with no ragdoll is down as [`Crash`]'s one rider point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WreckTargets {
    pub rider: Vf,
    pub bike: Vf,
    /// The bike's velocity, which the camera freezes at the wreck's entry: the bike block's flag word
    /// has the bit that says "read my velocity, don't difference my position", and nothing updates
    /// the velocity it carries once the rider is off.
    pub bike_vel: Vf,
}

/// The wreck camera's two targets, off the crash and the ragdoll if the rider has one.
pub fn wreck_targets(crash: &Crash, down: Option<(&game_sim::ragdoll_ride::Down, &game_sim::ragdoll_ride::Kit)>) -> WreckTargets {
    let rider = match down {
        Some((down, kit)) => down
            .nodes(kit)
            .get(game_sim::ragdoll::BACK)
            .map_or(crash.rider, |node| node.0[3]),
        None => crash.rider,
    };
    WreckTargets { rider, bike: crash.origin, bike_vel: crash.body.vel }
}

/// The original's camera frame is the game's 30 Hz frame: its lags are handed 1/30 s, and the aim's
/// eases are fractions per frame, which this scales to the shell's `dt`.
const WRECK_FRAME: f32 = 1.0 / 30.0;

/// The wreck camera's tuning, as its setup and constructor write it (`research/oracle.txt`, V31).
const WRECK_BASE_FOV: f32 = 80.0;
const WRECK_MIN_FOV: f32 = 20.0;
const WRECK_MAX_FOV: f32 = 82.0;
const WRECK_MIN_DISTANCE: f32 = 13.0;
const WRECK_MAX_DISTANCE: f32 = 100.0;
/// Where the aim settles: a quarter turn round from the way the targets are moving, so the wreck is
/// seen side-on, and eight degrees up.
const WRECK_YAW: f32 = 1.5707964;
const WRECK_PITCH: f32 = 0.1396263;
/// The aim's ease per 30 Hz frame.
const WRECK_AIM_EASE: f32 = 0.05;
/// How far the direction overshoots its new aim, as a share of the turn since last frame.
const WRECK_DIRECTION_OVERSHOOT: f32 = 0.1;
/// The look-at leads the targets by this many seconds of their velocity.
const WRECK_LEAD: f32 = 0.2;
/// The look-at holds still while the new one is within a foot of it.
const WRECK_DEADBAND: f32 = 1.0;
/// The lowest the eye may sit above the ground: this, or three near planes if that is more.
const WRECK_FLOOR: f32 = 5.0;
const WRECK_FOV_TAU: f32 = 0.77;

/// **The wreck camera** (V31): the original's `MXCameraWreck`, the camera each screen switches to the
/// moment its rider wrecks and away from when the rider is back on. It is not the follow camera with
/// another target. It is a second camera with its own frame:
///
/// * it looks at the targets' point: the rider's body, pulled towards the bike by how close the two
///   are (all the way to the body at 45 ft apart, 25 at 16:9), led by 0.2 s of their velocity;
/// * its direction is the way they are moving turned by its aim, which eases towards a quarter turn
///   round it and eight degrees up, so the rider is filmed from the side; the eye's lags do the
///   smoothing, the direction itself follows at once;
/// * its distance is 13 ft, eased out as far as framing the rider and the bike in its field of view
///   takes, and never past 100;
/// * its eye stays at least 5 ft above the ground, and the point it frames is never under it.
///
/// It starts where the follow camera was: its eye from the view's eye, its look-at from the follow
/// camera's look-at lags, each with the wreck's own lag. A cannonball (reason 13) gets shorter ones.
/// Getting back on hands the screen back to the follow camera at once, with no blend: the follow camera
/// snaps.
#[derive(Clone, Debug)]
pub struct WreckCamera {
    /// It has the screen.
    pub on: bool,
    /// The 16:9 tuning: a nearer radius for the second target, a lower look-at lag ratio.
    pub wide: bool,
    pub eye: [game_sim::camera::Lag; 3],
    pub look: [game_sim::camera::Lag; 3],
    pub fov: game_sim::camera::Lag,
    /// The look-at before the lags, which the deadband holds.
    pub look_at: [f32; 3],
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
    /// The farthest the targets are apart, or one and a half minimum distances with one target.
    pub spread: f32,
    /// What the distance has eased out past its minimum.
    pub distance_offset: f32,
    /// What the field of view has moved off its base.
    pub fov_offset: f32,
    /// The field of view it moves off: the screen's as the wreck began, the follow camera's.
    pub base_fov: f32,
    /// The direction from the targets to the eye.
    pub direction: [f32; 3],
    /// The last velocity fast enough to steer by.
    pub heading: Option<[f32; 3]>,
    /// Each target's point last frame, for the velocity of the ones that are differenced.
    pub last: [Option<[f32; 3]>; 2],
    pub bike_vel: [f32; 3],
}

fn v3(v: Vf) -> [f32; 3] {
    [v.x(), v.y(), v.z()]
}

fn vf(v: [f32; 3]) -> Vf {
    Vf::new(v[0], v[1], v[2], 1.0)
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn mul(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn length(a: [f32; 3]) -> f32 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

fn unit(a: [f32; 3]) -> [f32; 3] {
    let l = length(a);
    if l > 0.0 { mul(a, 1.0 / l) } else { a }
}

/// A per-frame ease of `k` at 30 Hz, as a fraction of the gap for a step of `dt`.
fn ease(k: f32, dt: f32) -> f32 {
    1.0 - (1.0 - k).powf(dt / WRECK_FRAME)
}

impl WreckCamera {
    /// The camera as its constructor and setup leave it: off, aimed straight along the way the
    /// targets go, at its base field of view.
    pub fn new(wide: bool) -> WreckCamera {
        let lag = |tau: f32| game_sim::camera::Lag::new(F(tau));
        let mut fov = lag(WRECK_FOV_TAU);
        fov.value = F(WRECK_BASE_FOV);
        WreckCamera {
            on: false,
            wide,
            eye: [lag(0.33); 3],
            look: [lag(0.33), lag(0.33 * Self::look_y_ratio(wide)), lag(0.33)],
            fov,
            look_at: [0.0; 3],
            distance: WRECK_MIN_DISTANCE,
            yaw: 0.0,
            pitch: 0.0,
            spread: 0.0,
            distance_offset: 0.0,
            fov_offset: 0.0,
            base_fov: WRECK_BASE_FOV,
            direction: [0.0; 3],
            heading: None,
            last: [None; 2],
            bike_vel: [0.0; 3],
        }
    }

    /// The look-at's vertical lag, as a share of the other two.
    fn look_y_ratio(wide: bool) -> f32 {
        if wide { 0.25 } else { 0.75 }
    }

    /// How close the bike must be to the rider to pull the camera's point towards it.
    pub fn radius(&self) -> f32 {
        if self.wide { 25.0 } else { 45.0 }
    }

    /// **The switch**: the screen's wreck notice. The eye starts at `eye` (the view's), the look-at at
    /// `look_at` (the follow camera's look-at lags), and the distance and direction from where that
    /// eye is, and its field of view moves off `fov`, the screen's. `reason` 13, the cannonball, lags
    /// less.
    pub fn enter(&mut self, eye: Vf, look_at: Vf, fov: f32, reason: u8, targets: &WreckTargets) {
        let cannonball = reason == 13;
        let (eye_tau, look_tau) = if cannonball { (0.2, 0.23) } else { (0.28, 0.3) };
        let ratio = Self::look_y_ratio(self.wide);
        let look_y_tau = if cannonball { look_tau * ratio * 0.9 } else { look_tau * ratio };
        for (lag, (value, tau)) in self.eye.iter_mut().zip(v3(eye).into_iter().zip([eye_tau; 3])) {
            lag.set_value(F(value));
            lag.set_tau(F(tau));
        }
        for (lag, (value, tau)) in self.look.iter_mut().zip(v3(look_at).into_iter().zip([look_tau, look_y_tau, look_tau])) {
            lag.set_value(F(value));
            lag.set_tau(F(tau));
        }
        // The activation: the distance and direction are the eye's from the targets' centre, the
        // look-at is where the lags hold it, the spread is measured afresh and the field of view's
        // offset is dropped.
        let (rider, bike) = (v3(targets.rider), v3(targets.bike));
        let centre = mul(add(rider, bike), 0.5);
        let away = sub(v3(eye), centre);
        self.distance = length(away);
        self.direction = unit(away);
        self.look_at = v3(look_at);
        self.distance_offset = (self.distance - WRECK_MIN_DISTANCE).clamp(0.0, WRECK_MAX_DISTANCE - WRECK_MIN_DISTANCE);
        self.distance = WRECK_MIN_DISTANCE + self.distance_offset;
        self.spread = 0.0;
        self.fov_offset = 0.0;
        self.base_fov = fov;
        self.last = [Some(rider), Some(bike)];
        self.bike_vel = v3(targets.bike_vel);
        self.on = true;
    }

    /// **Back on**: the screen goes back to the follow camera, and the wreck camera's list is
    /// emptied. The aim and the field of view are kept, as the original's are, for the next wreck.
    pub fn leave(&mut self) {
        self.on = false;
        self.last = [None; 2];
    }

    /// **Which camera has the screen**, once a frame before either runs: `targets` while the rider is
    /// down (`reason` the wreck's), `None` while they ride. The first frame down switches to this
    /// camera from where `follow` had the view — its eye lags and its look-at lags, as they were left
    /// last frame, and its field of view as the base — and the first frame back on hands the screen back to `follow` and makes it snap,
    /// as the original's does: its eye jumped to the bike the frame the rider was back on, with no
    /// blend. True while this camera has the screen.
    pub fn switch(&mut self, targets: Option<&WreckTargets>, reason: u8, follow: &mut game_sim::camera::Follow) -> bool {
        match targets {
            Some(targets) if !self.on => {
                let lags = |l: &[game_sim::camera::Lag; 3]| Vf::new(l[0].value.0, l[1].value.0, l[2].value.0, 1.0);
                self.enter(lags(&follow.pos_filter), lags(&follow.look_filter), follow.fov_filter.value.0, reason, targets);
            }
            None if self.on => {
                self.leave();
                follow.snap = true;
            }
            _ => {}
        }
        self.on
    }

    /// The point the camera frames and the velocity it leads by: the rider, pulled towards the bike
    /// while they are within [`WreckCamera::radius`] of each other, and the two velocities averaged.
    /// The rider's velocity is its point differenced; the bike's is the frozen one.
    pub fn target(&mut self, dt: f32, targets: &WreckTargets) -> ([f32; 3], [f32; 3], usize) {
        let (rider, bike) = (v3(targets.rider), v3(targets.bike));
        let apart = length(sub(bike, rider));
        // The second target's share: one less its place over the count plus one, times how far
        // inside the radius it is.
        let near = (1.0 - apart / self.radius()).max(0.0);
        let mut point = rider;
        let mut active = 1;
        if near > 0.0 {
            point = add(point, add(rider, mul(sub(bike, rider), (1.0 - 1.0 / 3.0) * near)));
            active = 2;
        }
        point = mul(point, 1.0 / active as f32);
        let rider_vel = match self.last[0] {
            Some(was) if dt > 0.0 => mul(sub(rider, was), 1.0 / dt),
            _ => [0.0; 3],
        };
        self.last = [Some(rider), Some(bike)];
        // Under 20 ft/s, a target's climb counts for seven tenths.
        let slow = |v: [f32; 3]| if length(v) < 20.0 { [v[0], v[1] * 0.7, v[2]] } else { v };
        let mut vel = slow(rider_vel);
        if active == 2 {
            vel = mul(add(vel, slow(self.bike_vel)), 0.5);
        }
        (point, vel, active)
    }

    /// One frame. `short_view` is the follow camera's short-viewport test; `near` the near plane.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        dt: f32,
        targets: &WreckTargets,
        short_view: bool,
        near: f32,
        curve: &game_sim::camera::CurveTable,
        ground: &mut impl game_sim::camera::Surface,
    ) -> game_sim::camera::View {
        // The spread first: how far apart the targets are (with one, one and a half minimum
        // distances), and how much that moved since last frame.
        let apart = length(sub(v3(targets.bike), v3(targets.rider)));
        let counted = apart < self.radius();
        let was = if self.spread == 0.0 { apart } else { self.spread };
        let (spread, grew) = if counted { (apart, apart - was) } else { (WRECK_MIN_DISTANCE * 1.5, 0.0) };
        self.spread = spread;
        let fov = self.fov_step(grew);
        self.distance_step(grew, dt);

        let (mut point, vel, _) = self.target(dt, targets);
        // The derived class's target: never under the ground.
        let floor = ground.on_surface(vf(point)).y();
        if point[1] < floor {
            point[1] = floor;
        }

        // The aim: pitch eases to eight degrees (less a third in a short view), and the yaw to a
        // quarter turn once the targets are spread out.
        let pitch_to = if short_view { WRECK_PITCH - 0.33 * WRECK_PITCH } else { WRECK_PITCH };
        let k = ease(WRECK_AIM_EASE, dt);
        self.pitch += (pitch_to - self.pitch) * k;
        if self.spread > 3.0 {
            self.yaw += (WRECK_YAW - self.yaw) * k;
        }

        // The direction: the way the targets move, flat, or the last way they moved fast enough.
        let moving = length(vel) * length(vel) > 9.0;
        if moving {
            self.heading = Some(vel);
        }
        let flat = [vel[0], 0.0, vel[2]];
        let from = if flat[0] * flat[0] + flat[2] * flat[2] > 9.0 {
            flat
        } else {
            let h = self.heading.unwrap_or([0.0, 0.0, 1.0]);
            [h[0], 0.0, h[2]]
        };
        let along = vf(unit(from));
        let up = Vf::new(0.0, 1.0, 0.0, 0.0);
        // `along × up`: the axis the pitch lifts the direction about.
        let side = Vf::new(-along.z(), 0.0, along.x(), 0.0);
        let turned = game_sim::camera::rotate(F(self.yaw), up, game_sim::camera::rotate(F(self.pitch), side, along, curve), curve);
        // Not an ease: the new aim, a tenth of the turn past it.
        let turned = v3(turned);
        self.direction = unit(add(turned, mul(sub(turned, self.direction), WRECK_DIRECTION_OVERSHOOT)));

        // The look-at: the point led by its velocity, held while it moves less than a foot.
        let lead = if moving { add(point, mul(vel, WRECK_LEAD)) } else { point };
        if length(sub(lead, self.look_at)) >= WRECK_DEADBAND {
            self.look_at = lead;
        }

        // The eye: the point, out along the direction by the distance.
        self.distance = (WRECK_MIN_DISTANCE + self.distance_offset).clamp(WRECK_MIN_DISTANCE, WRECK_MAX_DISTANCE);
        self.distance_offset = self.distance - WRECK_MIN_DISTANCE;
        let eye = add(point, mul(self.direction, self.distance));

        let dt = F(dt);
        let look = [0, 1, 2].map(|i| self.look[i].step(F(self.look_at[i]), dt).0);
        let mut eye = [0, 1, 2].map(|i| self.eye[i].step(F(eye[i]), dt).0);
        let fov = self.fov.step(F(fov), dt);
        // The eye's floor: never nearer the ground than the floor height.
        let height = WRECK_FLOOR.max(near * 3.0);
        let ground_y = ground.on_surface(vf(eye)).y();
        if eye[1] - ground_y < height {
            eye[1] = ground_y + height;
            self.eye[1].set_value(F(eye[1]));
        }
        game_sim::camera::View { position: vf(eye), look_at: vf(look), fov }
    }

    /// The field of view this frame, and its offset's walk: a spread that moved by a tenth of a foot
    /// or more moves the offset by a tenth of that; otherwise it settles where the distance puts it,
    /// widest at the nearest.
    fn fov_step(&mut self, grew: f32) -> f32 {
        let step = grew / 10.0;
        if step.abs() >= 0.1 || self.fov_offset <= 0.0 {
            self.fov_offset += step;
        } else {
            let span = WRECK_MAX_DISTANCE - WRECK_MIN_DISTANCE;
            let out = (WRECK_MAX_DISTANCE - (WRECK_MIN_DISTANCE + self.distance_offset)) / span;
            self.fov_offset = WRECK_MIN_FOV + (WRECK_MAX_FOV - WRECK_MIN_FOV) * out - self.base_fov;
        }
        // The clamp, with the original's slip: past the widest the offset is set to the clamp's span,
        // not to what would reach the widest, and the field of view holds at the widest until the
        // offset walks back.
        let fov = self.base_fov + self.fov_offset;
        if fov > WRECK_MAX_FOV {
            self.fov_offset = WRECK_MAX_FOV - WRECK_MIN_FOV;
            WRECK_MAX_FOV
        } else if fov < WRECK_MIN_FOV {
            self.fov_offset = 0.0;
            WRECK_MIN_FOV
        } else {
            fov
        }
    }

    /// The distance's walk: a tenth of the spread's growth, and, while the spread is not running
    /// away, a twentieth of the gap to the distance that fits the spread in the field of view.
    fn distance_step(&mut self, grew: f32, dt: f32) {
        let mut step = grew / 10.0;
        if grew <= 0.9 {
            let wide = (self.base_fov + self.fov_offset - 20.0).max(WRECK_MIN_FOV);
            let fits = WRECK_MIN_DISTANCE.max(self.spread / ((wide * 0.5).to_radians().tan() * 2.0));
            let gap = fits - (WRECK_MIN_DISTANCE + self.distance_offset);
            if gap > 0.0 || -gap < self.distance_offset {
                step += gap * ease(WRECK_AIM_EASE, dt);
            }
        }
        self.distance_offset += step;
    }

    /// The distance [`WreckCamera::update`] eases towards for a spread at a base field of view `fov`:
    /// far enough to fit the spread across the field of view less twenty degrees.
    #[allow(dead_code)] // the tests' number
    pub fn fitting_distance(spread: f32, fov: f32) -> f32 {
        let wide = (fov - 20.0).max(WRECK_MIN_FOV);
        WRECK_MIN_DISTANCE.max(spread / ((wide * 0.5).to_radians().tan() * 2.0))
    }
}
