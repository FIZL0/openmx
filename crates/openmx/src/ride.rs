//! **The shell's own stand-in for the bike, and the one part of this binary that is not a port.**
//!
//! The bike's ground physics is S3: the throttle, the brake, the lean and the steering out of the
//! track's `bike\*.phy` set, with the suspension and the wheel contact that feed S10's air code. It is
//! not ported yet, and neither is the `.phy` reader that would fill in [`game_sim::air::Params`] — so
//! there is nothing in `game-sim` that can move a bike along the ground, and the air code cannot be
//! handed its parameters either.
//!
//! Rather than invent either of them inside the simulation, where a guess would be indistinguishable
//! from a proved port six months from now, the shell carries its own rider here:
//!
//! * Plain host `f32`. No [`ps2_float`] types, no claim of bit-exactness, and deliberately outside
//!   `game-sim` so that no test, hash or replay can ever come to depend on it.
//! * No parameter pretends to be the game's. The numbers below are chosen to feel like a 125 on dirt
//!   (a bike's model is seven and a half feet long and the disc's unit is a foot, so they are in feet
//!   and seconds), and they are *not* `Race125P.phy`'s.
//! * Everything it does read is real: the ground it rides on is D6's proved terrain query, the input
//!   it reads is I1's and I2's pad, and the frame it runs in is S1's fixed update.
//!
//! When S3 lands, this module goes: the ride becomes a `game-sim` state, the shell keeps the window,
//! the pad and the camera, and this file's only trace is a shorter `main`.

use game_render::mat::{V3, v3};

use crate::ground::Surface;

/// Gravity, in feet per second squared.
const GRAVITY: f32 = 32.17;
/// What the throttle is worth, wide open, in feet per second squared.
const ACCELERATION: f32 = 34.0;
/// And the brake.
const BRAKING: f32 = 48.0;
/// Rolling resistance, which is what stops a bike that is not being ridden.
const ROLLING: f32 = 2.5;
/// Drag, as the coefficient of `v^2` that makes the top speed come out at about 65 mph.
const DRAG: f32 = 0.0038;
/// How fast the bike can turn on the ground at a standstill, in radians a second, and how much of
/// that it loses at full speed — a bike at speed turns in a wider arc.
const YAW_RATE: f32 = 1.9;
const YAW_AT_SPEED: f32 = 0.55;
/// How much clearance counts as the wheels having left the ground, in feet, and how long the bike has
/// to be up there before coming down costs it anything. Without both of these a bumpy field launches
/// the bike every few frames and charges it a landing each time, which caps a wide-open 125 at walking
/// pace. The game's own answer to this is the suspension, which is S3's.
const LIFT_OFF: f32 = 0.4;
const SETTLED: f32 = 0.1;
/// How much speed a landing costs, as a fraction, plus what a bad angle adds to it.
const LANDING_LOSS: f32 = 0.06;
const LANDING_SLAM: f32 = 0.5;
/// The wheelbase the bike is pitched over when it sits on a slope: the model's own length.
const WHEELBASE: f32 = 7.5;

/// What the rider is asking for this frame, read off the port's pad.
#[derive(Clone, Copy, Debug, Default)]
pub struct Controls {
    /// Throttle and brake, 0..1 — a control's strength, which is its pressure byte for an analogue
    /// control and 1.0 for a digital one.
    pub throttle: f32,
    pub brake: f32,
    /// The left stick: steering, and the rider's weight fore and aft.
    pub steer: f32,
    pub lean: f32,
    /// Put the bike back on the start line.
    pub respawn: bool,
}

/// Where the bike is and what it is doing.
#[derive(Clone, Copy, Debug)]
pub struct Ride {
    pub pos: V3,
    /// Which way it points, in radians about the up axis, zero being +Z — the direction the scene's
    /// own start direction faces on this track.
    pub heading: f32,
    /// Along the heading, in feet per second.
    pub speed: f32,
    /// Vertical speed, which is the ground's climb rate while the wheels are down.
    pub vertical: f32,
    pub airborne: bool,
    /// Seconds since the wheels left the ground.
    pub hang_time: f32,
    /// The ground's normal under the wheels, which is what the bike is pitched and rolled onto.
    pub up: V3,
    /// The surface id under the wheels, out of the terrain's own samples.
    pub surface: u8,
    /// Where a respawn puts it back.
    spawn: V3,
    spawn_heading: f32,
}

impl Ride {
    /// On the start line, facing the way the scene says, with the wheels on the ground.
    pub fn new(spawn: V3, heading: f32, ground: &impl Surface) -> Ride {
        let mut ride = Ride {
            pos: spawn,
            heading,
            speed: 0.0,
            vertical: 0.0,
            airborne: false,
            hang_time: 0.0,
            up: v3(0.0, 1.0, 0.0),
            surface: 0,
            spawn,
            spawn_heading: heading,
        };
        ride.settle(ground);
        ride
    }

    /// Put the bike back on the start line.
    pub fn respawn(&mut self, ground: &impl Surface) {
        self.pos = self.spawn;
        self.heading = self.spawn_heading;
        self.speed = 0.0;
        self.vertical = 0.0;
        self.airborne = false;
        self.hang_time = 0.0;
        self.settle(ground);
    }

    /// Drop the bike onto the ground wherever it is.
    fn settle(&mut self, ground: &impl Surface) {
        if let Some(contact) = ground.contact(self.pos.x, self.pos.z) {
            self.pos.y = contact.y;
            self.up = contact.normal;
            self.surface = contact.surface;
        }
    }

    /// Which way the bike points, as a direction.
    pub fn facing(&self) -> V3 {
        v3(self.heading.sin(), 0.0, self.heading.cos())
    }

    /// Where the camera and the model should put it: the chassis' own frame, pitched onto the slope
    /// it is standing on.
    ///
    /// The pitch is geometric — the ground a wheelbase ahead against the ground a wheelbase behind —
    /// because the suspension that would decide it is S3's. It keeps the bike's wheels in the dirt
    /// going up a face, which is the difference between riding a track and sliding over it.
    pub fn frame(&self, ground: &impl Surface) -> (V3, V3) {
        let facing = self.facing();
        if self.airborne {
            return (facing, v3(0.0, 1.0, 0.0));
        }
        let half = facing.scale(WHEELBASE * 0.5);
        let front = ground.contact(self.pos.x + half.x, self.pos.z + half.z);
        let rear = ground.contact(self.pos.x - half.x, self.pos.z - half.z);
        let look = match (front, rear) {
            (Some(f), Some(r)) => v3(facing.x * WHEELBASE, f.y - r.y, facing.z * WHEELBASE).norm(),
            _ => facing,
        };
        (look, self.up)
    }

    /// One frame of riding.
    ///
    /// The order is the order a ground vehicle has to be stepped in to stay on a height field:
    /// speed, then heading, then the move, then the ground under where it ended up — which decides
    /// whether the wheels are still down.
    pub fn step(&mut self, dt: f32, controls: &Controls, ground: &impl Surface) {
        if dt <= 0.0 {
            return;
        }
        if controls.respawn {
            self.respawn(ground);
            return;
        }

        // Speed. The throttle only pulls while the wheels are down; drag and rolling resistance are
        // what the bike coasts down against.
        let drive = if self.airborne {
            0.0
        } else {
            ACCELERATION * controls.throttle - BRAKING * controls.brake - ROLLING
        };
        let resist = DRAG * self.speed * self.speed;
        self.speed = (self.speed + (drive - resist) * dt).max(0.0);

        // Heading. Airborne steering is the stick's moment about the chassis, which is the air
        // code's and needs the bike's inertia, so up there the bike holds the line it left on.
        if !self.airborne {
            let fast = (self.speed / 95.0).clamp(0.0, 1.0);
            self.heading += controls.steer * YAW_RATE * (1.0 - YAW_AT_SPEED * fast) * dt;
        }

        // The move, along the heading. The lean is the rider's weight, which in a real port shifts
        // the contact patch; here it is worth a few per cent of drive and nothing else.
        let facing = self.facing();
        let was_at = self.pos.y;
        let travel = self.speed * dt * (1.0 - 0.05 * controls.lean.clamp(-1.0, 1.0));
        let (x, z) = (
            self.pos.x + facing.x * travel,
            self.pos.z + facing.z * travel,
        );

        // The ground where it ended up. Off the field there is nothing to ride on, so the bike keeps
        // its old place and loses its speed — the edge of the map is a wall.
        let Some(contact) = ground.contact(x, z) else {
            self.speed = 0.0;
            return;
        };
        self.pos.x = x;
        self.pos.z = z;
        self.surface = contact.surface;

        // Where free flight would put it this step, against where the ground is: higher than the
        // ground means the wheels have left it, which is how a lip launches the bike.
        let free = self.pos.y + self.vertical * dt - 0.5 * GRAVITY * dt * dt;
        if free > contact.y + LIFT_OFF {
            self.airborne = true;
            self.hang_time += dt;
            self.pos.y = free;
            self.vertical -= GRAVITY * dt;
        } else {
            if self.airborne {
                // A landing. How much it costs depends on how hard it was and how far the bike is
                // from flat — which is where the air code's own landing test would come in, if it
                // had its parameters. A hop off a bump is not a landing and costs nothing.
                if self.hang_time > SETTLED {
                    let slam = (-self.vertical / 40.0).clamp(0.0, 1.0);
                    self.speed *= 1.0 - LANDING_LOSS - LANDING_SLAM * slam;
                }
                self.airborne = false;
                self.hang_time = 0.0;
            }
            // On the ground the vertical speed is whatever the ground is doing: that is what makes
            // the top of a jump throw the bike rather than drop it off a cliff, and it is measured
            // before the bike is put down on the new height.
            self.vertical = ((contact.y - was_at) / dt).clamp(-GRAVITY, GRAVITY);
            self.pos.y = contact.y;
            self.up = contact.normal;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ground::Contact;

    /// A flat plane with no edges, which is enough ground to prove the shape of the ride on without a
    /// disc. The one test that wants an edge makes its own.
    fn plain(height: f32) -> impl Surface {
        move |_x: f32, _z: f32| {
            Some(Contact {
                y: height,
                normal: v3(0.0, 1.0, 0.0),
                surface: 0,
            })
        }
    }

    /// Flat for the first hundred feet, then the ground drops away — a lip to launch off.
    fn lip() -> impl Surface {
        |x: f32, _z: f32| {
            Some(Contact {
                y: if x < 100.0 { 20.0 } else { 0.0 },
                normal: v3(0.0, 1.0, 0.0),
                surface: 0,
            })
        }
    }

    const FRAME: f32 = 1.0 / 60.0;

    /// Nothing here is proved against the original — it is not a port — but the ride still has to do
    /// what the pad asks: open the throttle and the bike leaves the start line in the direction the
    /// scene pointed it, and reaches a top speed rather than running away.
    #[test]
    fn a_held_throttle_pulls_away_down_the_heading_and_tops_out() {
        let ground = plain(12.0);
        let mut ride = Ride::new(v3(20.0, 0.0, 20.0), 0.0, &ground);
        assert_eq!(ride.pos.y, 12.0, "the wheels start on the ground");

        let wide_open = Controls {
            throttle: 1.0,
            ..Controls::default()
        };
        for _ in 0..60 {
            ride.step(FRAME, &wide_open, &ground);
        }
        // A second of throttle: moving, still on the ground, and along +Z, which is heading zero.
        assert!(ride.speed > 20.0, "a second of throttle: {}", ride.speed);
        assert!(!ride.airborne);
        assert!(ride.pos.z > 20.0 && (ride.pos.x - 20.0).abs() < 0.001);
        assert_eq!(ride.pos.y, 12.0);

        for _ in 0..60 * 60 {
            ride.step(FRAME, &wide_open, &ground);
        }
        // A minute of it: drag has settled it somewhere sane for a 125, and it is still a number.
        assert!(
            ride.speed.is_finite() && (60.0..110.0).contains(&ride.speed),
            "{}",
            ride.speed
        );
    }

    /// The brake stops it, and nothing drags it backwards once it has.
    #[test]
    fn the_brake_brings_it_to_a_stop_and_no_further() {
        let ground = plain(0.0);
        let mut ride = Ride::new(v3(50.0, 0.0, 50.0), 0.0, &ground);
        ride.speed = 60.0;
        let on_the_brake = Controls {
            brake: 1.0,
            ..Controls::default()
        };
        for _ in 0..180 {
            ride.step(FRAME, &on_the_brake, &ground);
        }
        assert_eq!(ride.speed, 0.0);
        assert!(
            ride.pos.z > 50.0,
            "it stopped going forwards, not backwards"
        );
    }

    /// Ride off a lip at speed and the wheels leave the ground; come down and they are back on it,
    /// a good deal slower than they went up.
    #[test]
    fn a_lip_at_speed_puts_it_in_the_air_and_it_lands_again() {
        let ground = lip();
        let mut ride = Ride::new(v3(60.0, 0.0, 60.0), std::f32::consts::FRAC_PI_2, &ground);
        ride.speed = 70.0;
        let coasting = Controls {
            throttle: 0.5,
            ..Controls::default()
        };

        let mut flew = false;
        let mut top = ride.pos.y;
        for _ in 0..240 {
            ride.step(FRAME, &coasting, &ground);
            flew |= ride.airborne;
            top = top.max(ride.pos.y);
            assert!(ride.pos.y.is_finite() && ride.speed.is_finite());
        }
        assert!(flew, "it never left the lip");
        assert!(top >= 20.0, "it never got above the lip: {top}");
        assert!(!ride.airborne, "it never came down");
        assert_eq!(ride.pos.y, 0.0, "it landed on the lower ground");
        assert!(ride.speed < 70.0, "a landing costs speed: {}", ride.speed);
    }

    /// Off the edge of the field there is nothing to ride on, and the ride stops dead rather than
    /// walking off into the dark with a NaN height.
    #[test]
    fn the_edge_of_the_field_is_a_wall() {
        // A field a thousand feet across, with nothing at all past it.
        let ground = |x: f32, z: f32| {
            ((0.0..1000.0).contains(&x) && (0.0..1000.0).contains(&z)).then_some(Contact {
                y: 5.0,
                normal: v3(0.0, 1.0, 0.0),
                surface: 0,
            })
        };
        let mut ride = Ride::new(v3(990.0, 0.0, 500.0), std::f32::consts::FRAC_PI_2, &ground);
        ride.speed = 80.0;
        for _ in 0..600 {
            ride.step(FRAME, &Controls::default(), &ground);
            assert!(ride.pos.x < 1000.0 && ride.pos.x.is_finite());
            assert!(ride.pos.y.is_finite() && ride.speed.is_finite());
        }
        assert_eq!(ride.speed, 0.0);
    }

    /// The pad's respawn puts it back where the scene started it, whatever it was doing.
    #[test]
    fn respawn_puts_it_back_on_the_start_line() {
        let ground = plain(3.0);
        let spawn = v3(100.0, 0.0, 200.0);
        let mut ride = Ride::new(spawn, 1.0, &ground);
        for _ in 0..120 {
            ride.step(
                FRAME,
                &Controls {
                    throttle: 1.0,
                    steer: 1.0,
                    ..Controls::default()
                },
                &ground,
            );
        }
        assert!(ride.pos.z != spawn.z);
        ride.step(
            FRAME,
            &Controls {
                respawn: true,
                ..Controls::default()
            },
            &ground,
        );
        assert_eq!((ride.pos.x, ride.pos.z), (spawn.x, spawn.z));
        assert_eq!(ride.pos.y, 3.0);
        assert_eq!(ride.heading, 1.0);
        assert_eq!(ride.speed, 0.0);
    }
}
