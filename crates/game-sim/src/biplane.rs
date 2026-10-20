//! The biplane: its crash test, its wreck, and the way back to its spawn.
//!
//! The flight itself (the controls into the surfaces, the aerodynamics, the prop, the wheels) is S34's
//! and is not in this module yet. What is here is the part of the plane's frame that decides it has
//! crashed and puts it back, ported from `Biplane.cpp` and `MXBiplane` (research/oracle.txt, R8 and
//! S36) and proved against a playback of the human's biplane recording
//! (`tests/biplane.rs::the_biplane_crashes_and_respawns`). It runs on a [`Plane`] that carries only
//! the fields those functions read and write; S34's flight is to grow that struct rather than
//! replace it, and to call the functions below from the places the original calls them:
//!
//! * [`Plane::reset_chord`] from the input slot, before the forces. A respawn there is integrated on
//!   the same tick, which is why the capture's chord respawn shows a body already falling.
//! * [`Plane::wreck_check`] from the measure slot, after the surfaces are animated and **before**
//!   `airspeed` and `up_y` are recomputed: the test reads the previous tick's two.
//! * [`Plane::frame`] from the frame slot, which clears the chord's latch and runs the respawn clock.
//!
//! # What a crash is
//!
//! Two reasons, and only two ([`Wreck`]):
//!
//! * **Contact (1).** The body itself — not a tire — is touching something, and either the plane was
//!   faster than `MinWreckSpeed` (the file's 50 mph) on a surface without the `VEHICLE` material flag,
//!   or it is upside down. Ground, a wall and a building are all the same test: there is no reason of
//!   its own for water, and no wall test apart from the body's contact.
//! * **The map edge (13).** Within 150 ft of the terrain's edge the plane is thrown back toward the
//!   middle of the map on a ballistic arc with a random tumble, and wrecked.
//!
//! The wreck is the plane's own and not `Wreckbike.phy`'s: the body keeps flying under `[Wrecks]`'
//! damping, inertia scale and heavier gravity, the throttle and the engine are cut, and the prop is
//! drawn bent ([`Prop`]). After `WreckTime` seconds (reason 13: after it has been down for 2.5 s, then
//! `WreckTime`) the plane is unwrecked, which resets it: back to the spawn transform the track's
//! `machine.txt` placed it at, standing still, with the throttle and every surface at zero.
//!
//! The reset chord (L1 and R1, [`crate::ride::Controls::respawn`]) does the same at once, but only
//! for a plane that is on its wheels or wrecked.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::hash::{State, Walk};
use crate::phy::{G, Ini, MPH};
use crate::rng::Rng;
use ps2_float::{Acc, Dest, F, Vf};

/// The quadword the game's rodata holds for "nothing": a point at the origin.
const NOTHING: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);
/// The rodata quadword an unwreck restores both dampings to.
const UNDAMPED: Vf = Vf::new(1.0, 1.0, 1.0, 1.0);

/// How close to the terrain's edge the map-edge test fires, in feet.
const EDGE_MARGIN: f32 = 150.0;
/// How much the terrain query multiplies its root-cell count by: sixteen cells a side, two levels.
const CELLS_PER_ROOT: f32 = 256.0;
/// The map-edge throw's horizontal speed is capped at this distance, flown in one arc.
const THROW_REACH: f32 = 2073.599_9;
/// The throw's arc: the time constant under the square root, and the two terms of the lift.
const THROW_G: f32 = 14.906_832;
const THROW_LIFT_RATE: f32 = 16.1;
const THROW_LIFT_REACH: f32 = 240.0;
/// The tumble the throw puts in, per axis, as a share of one random number in `[0, 1)`.
const THROW_SPIN: [f32; 3] = [0.5, 0.4, 0.7];
/// A wreck spins the body up by this much.
const WRECK_SPIN: f32 = 4.0;
/// How long a reason-13 plane must have been down before its `WreckTime` starts.
const EDGE_SETTLE: f32 = 2.5;
/// A reason-13 plane keeps this share of its velocity through the unwreck.
const EDGE_KEEP: f32 = 0.3;
/// Pounds to slugs as the plane's body is given its mass: 1/32.2 rounded the way the parts reader and
/// the collision record round it, through a multiply. `Weight / 32.2` would be one ulp heavier
/// (`div.s` rounds to nearest), and the captured gravity force shows the multiply.
const SLUGS_PER_POUND: f32 = 0.031_055_9;
/// The respawn clock's length, when something arms it ([`Plane::arm_respawn`]).
const RESPAWN_TIME: f32 = 10.0;

/// `[Wrecks]` off `Biplane\biplane.phy`: the five keys the plane's reader takes out of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wrecks {
    /// `WreckTime`: seconds from the wreck to the unwreck.
    pub wreck_time: F,
    /// `MinWreckSpeed`, written in miles per hour and held in feet a second (the reader scales it by
    /// [`MPH`] after everything else is read).
    pub min_wreck_speed: F,
    /// `WreckGravity`: the gravity scale while wrecked.
    pub wreck_gravity: F,
    /// `WreckDamping`, a quadword, into both of the body's dampings.
    pub wreck_damping: Vf,
    /// `WreckInertia`, a quadword: what the body's inertia is scaled by while wrecked.
    pub wreck_inertia: Vf,
}

impl Wrecks {
    /// Read the block out of the file's bytes.
    pub fn read_bytes(bytes: &[u8]) -> Wrecks {
        Wrecks::read(&Ini::parse_bytes(bytes))
    }

    /// Load `[Wrecks]` in the reader's order. A missing float leaves the destination alone, and the
    /// object's are zero before the read, so a missing one reads as zero; a missing quadword takes
    /// the rodata's origin point.
    pub fn read(ini: &Ini) -> Wrecks {
        let s = ini.section("Wrecks");
        Wrecks {
            wreck_time: s.float_or("WreckTime", F::ZERO),
            min_wreck_speed: s.float_or("MinWreckSpeed", F::ZERO) * F(MPH),
            wreck_gravity: s.float_or("WreckGravity", F::ZERO),
            wreck_damping: s.vector("WreckDamping", NOTHING),
            wreck_inertia: s.vector("WreckInertia", NOTHING),
        }
    }
}

/// Why the plane is wrecked: the value the original keeps in the plane's wreck word.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Wreck {
    /// The body touched something too fast, or upside down.
    Contact = 1,
    /// The plane came within [`EDGE_MARGIN`] of the terrain's edge and was thrown back.
    MapEdge = 13,
}

/// Which prop model the plane shows: the blurred cone, or the bent one once it is wrecked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prop {
    /// `propconeblur`, textured `biplaneprop`.
    Blur,
    /// `propconebent`, textured `biplanepropbroken`.
    Bent,
}

/// A transform as the actor keeps it: a position and two axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub at: Vf,
    pub forward: Vf,
    pub up: Vf,
}

/// The rigid body, as far as the crash code touches it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    /// The mass in slugs (`Weight` × [`SLUGS_PER_POUND`]).
    pub mass: F,
    pub at: Vf,
    pub forward: Vf,
    pub up: Vf,
    pub vel: Vf,
    /// The angular velocity.
    pub spin: Vf,
    /// The diagonal of the inertia tensor.
    pub inertia: Vf,
    /// The two dampings a wreck writes: the body's setters at `+0x1c` and `+0x54`, both given the
    /// same quadword. Which is linear and which angular is not established.
    pub damping: Vf,
    pub spin_damping: Vf,
}

/// What the body is touching this tick, as the crash test reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Contact {
    /// The body (not a tire) is in contact.
    pub touching: bool,
    /// The surface it touches carries the `VEHICLE` material flag, which the speed rule spares.
    pub vehicle: bool,
}

/// The biplane, as far as S36 ports it.
#[derive(Clone, Debug, PartialEq)]
pub struct Plane {
    pub body: Body,
    /// The gravity force the body is given: `(0, −scale·m·g, 0, 1)`.
    pub gravity: Vf,
    /// The gravity scale an unwreck restores the force from.
    pub gravity_scale: F,
    /// A second scale the map-edge throw halves and its unwreck doubles back. Zero in the shipped
    /// state, and nothing in this module reads it.
    pub edge_scale: F,
    /// The throttle, and the engine's power, which a wreck cuts.
    pub throttle: F,
    pub engine_power: F,
    /// The surfaces a reset zeroes: elevator, elevator command, aileron, rudder.
    pub elevator: F,
    pub elevator_command: F,
    pub aileron: F,
    pub rudder: F,
    /// Any tire on the ground.
    pub grounded: bool,
    /// The speed and the up vector's `y`, both as the last measure left them.
    pub airspeed: F,
    pub up_y: F,
    /// The wreck word, the reason-13 "has been down" latch, and the seconds wrecked.
    pub wreck: Option<Wreck>,
    pub down: bool,
    pub clock: F,
    /// The inertia the body had when the file was read, which a wreck scales and an unwreck restores.
    pub inertia: Vf,
    /// Where `machine.txt` put the plane, and the actor's transform, which a reset copies it into.
    pub spawn: Placement,
    pub placed: Placement,
    /// A velocity the reset clears with the transform.
    pub kept_vel: Vf,
    /// The reset chord has fired this frame.
    pub latch: bool,
    /// The respawn clock, in seconds; idle at zero.
    pub respawn_clock: F,
    /// The base reset's flag on the body, set by every reset.
    pub fresh: bool,
}

impl Plane {
    /// A plane standing at its spawn: `weight` in pounds, `inertia` the body's diagonal, `gravity_scale`
    /// the scale its gravity force is built with.
    pub fn new(spawn: Placement, weight: F, inertia: Vf, gravity_scale: F) -> Plane {
        let mass = weight * F(SLUGS_PER_POUND);
        let body = Body {
            mass,
            at: spawn.at,
            forward: spawn.forward,
            up: spawn.up,
            vel: NOTHING,
            spin: NOTHING,
            inertia,
            damping: UNDAMPED,
            spin_damping: UNDAMPED,
        };
        Plane {
            body,
            gravity: gravity_force(gravity_scale, mass),
            gravity_scale,
            edge_scale: F::ZERO,
            throttle: F::ZERO,
            engine_power: F::ZERO,
            elevator: F::ZERO,
            elevator_command: F::ZERO,
            aileron: F::ZERO,
            rudder: F::ZERO,
            grounded: false,
            airspeed: F::ZERO,
            up_y: F::ONE,
            wreck: None,
            down: false,
            clock: F::ZERO,
            inertia,
            spawn,
            placed: spawn,
            kept_vel: NOTHING,
            latch: false,
            respawn_clock: F::ZERO,
            fresh: false,
        }
    }

    /// **The crash test**, once a tick from the measure slot.
    ///
    /// `extent` is the terrain's size ([`map_extent`]), `None` for a world with no terrain. The
    /// map-edge throw draws three numbers from `rng`, the game's own `rand`.
    pub fn wreck_check(&mut self, dt: F, w: &Wrecks, contact: Contact, extent: Option<(F, F)>, rng: &mut Rng) {
        if self.wreck != Some(Wreck::MapEdge) {
            if let Some(size) = extent.filter(|&s| near_edge(self.body.at, s)) {
                self.throw_back(size, rng);
                self.wreck(Wreck::MapEdge, w);
            }
        }
        match self.wreck {
            None => {
                let fast = contact.touching && !contact.vehicle && !(self.airspeed <= w.min_wreck_speed);
                if fast || (contact.touching && self.up_y < F::ZERO) {
                    self.wreck(Wreck::Contact, w);
                }
            }
            Some(reason) => {
                self.clock = self.clock + dt;
                match reason {
                    Wreck::MapEdge => {
                        if !self.down && F(EDGE_SETTLE) < self.clock && (self.grounded || contact.touching) {
                            self.down = true;
                            self.clock = F::ZERO;
                        }
                        if self.down && w.wreck_time < self.clock {
                            let v = self.body.vel;
                            self.body.vel = v.merge(Dest::XYZ, v.mul(Vf::splat(EDGE_KEEP)));
                            self.unwreck();
                        }
                    }
                    Wreck::Contact => {
                        if w.wreck_time < self.clock {
                            self.unwreck();
                        }
                    }
                }
            }
        }
    }

    /// The map-edge throw: toward the middle of the map, level with the plane, as fast as an arc of
    /// half the map (at most [`THROW_REACH`]) needs, plus the arc's lift, and a random tumble.
    fn throw_back(&mut self, (width, depth): (F, F), rng: &mut Rng) {
        let half = F(0.5);
        let mid_x = half * width;
        let mid_z = half * depth;
        let at = self.body.at;
        let target = Vf::new(mid_x.0, at.y(), mid_z.0, 1.0);
        let d = target.merge(Dest::XYZ, target.sub(at));
        let (x, y, z) = (F(d.x()), F(d.y()), F(d.z()));
        let len2 = Acc::mula(x, x).madda(y, y).madd(z, z);
        let dir = if F::ZERO == len2 {
            NOTHING
        } else {
            let k = F::ONE.rsqrt(len2);
            Vf::new((x * k).0, (y * k).0, (z * k).0, d.w())
        };
        let root = F(2.0) * F(THROW_G).sqrt();
        let speed = F(THROW_REACH).min(mid_x) / root;
        let lift = F(THROW_LIFT_RATE) * root + F(THROW_LIFT_REACH) / root;
        let vel = dir.merge(Dest::XYZ, dir.mul(Vf::splat(speed.0)));
        self.body.vel = Vf::new(vel.x(), (F(vel.y()) + lift).0, vel.z(), vel.w());
        self.edge_scale = self.edge_scale * half;
        let sx = F(THROW_SPIN[0]) * rng.unit();
        let sy = F(THROW_SPIN[1]) * rng.unit();
        let sz = F(THROW_SPIN[2]) * rng.unit();
        self.body.spin = Vf::new(sx.0, sy.0, sz.0, 1.0);
    }

    /// **The wreck**: the reason kept, the body spun up and given `[Wrecks]`' damping, inertia and
    /// gravity, the throttle and the engine cut. (The player's pad rumble and the engine sound are
    /// the original's too, and are not simulation.)
    pub fn wreck(&mut self, reason: Wreck, w: &Wrecks) {
        let inertia = w.wreck_inertia.merge(Dest::XYZ, w.wreck_inertia.mul(self.inertia));
        self.wreck = Some(reason);
        self.down = false;
        let s = self.body.spin;
        self.body.spin = s.merge(Dest::XYZ, s.mul(Vf::splat(WRECK_SPIN)));
        self.body.damping = w.wreck_damping;
        self.body.spin_damping = w.wreck_damping;
        self.body.inertia = inertia;
        self.gravity = gravity_force(w.wreck_gravity, self.body.mass);
        self.throttle = F::ZERO;
        self.engine_power = F::ZERO;
    }

    /// **The unwreck**: everything [`Plane::wreck`] changed put back, then [`Plane::reset`]. The reset
    /// chord calls it on a plane that is not wrecked too, and it resets that one the same way.
    pub fn unwreck(&mut self) {
        if self.wreck == Some(Wreck::MapEdge) {
            self.edge_scale = self.edge_scale / F(0.5);
        }
        self.wreck = None;
        self.clock = F::ZERO;
        self.body.damping = UNDAMPED;
        self.body.spin_damping = UNDAMPED;
        self.body.inertia = self.inertia;
        self.gravity = gravity_force(self.gravity_scale, self.body.mass);
        self.reset();
    }

    /// **The reset**: the plane back at its spawn transform, standing still, with the throttle and the
    /// four surfaces at zero and the respawn clock idle.
    pub fn reset(&mut self) {
        self.placed = self.spawn;
        self.kept_vel = NOTHING;
        self.throttle = F::ZERO;
        self.elevator = F::ZERO;
        self.elevator_command = F::ZERO;
        self.aileron = F::ZERO;
        self.rudder = F::ZERO;
        self.body.at = self.placed.at;
        self.body.forward = self.placed.forward;
        self.body.up = self.placed.up;
        self.body.vel = NOTHING;
        self.body.spin = NOTHING;
        self.fresh = true;
        self.respawn_clock = F::ZERO;
    }

    /// **The reset chord**, from the input slot: `respawn` is [`crate::ride::Controls::respawn`], the
    /// same two commands the bike's chord is. Once a frame, and only on the wheels or wrecked.
    pub fn reset_chord(&mut self, respawn: bool) {
        if respawn && !self.latch && (self.grounded || self.wreck.is_some()) {
            self.latch = true;
            self.unwreck();
        }
    }

    /// Arm the respawn clock: the hook the original's machine-racer interface calls on the plane.
    /// `immediate` is that interface's mode 1, which resets at once instead.
    pub fn arm_respawn(&mut self, immediate: bool) {
        if immediate {
            self.respawn();
        } else {
            self.respawn_clock = F(RESPAWN_TIME);
        }
    }

    /// **The frame slot's share**: the chord's latch cleared, and the respawn clock run down.
    pub fn frame(&mut self, dt: F) {
        self.latch = false;
        if F::ZERO < self.respawn_clock {
            self.respawn_clock = self.respawn_clock - dt;
            if self.respawn_clock <= F::ZERO {
                self.respawn();
            }
        }
    }

    /// The respawn clock's end: the spawn transform put back and the plane reset, then the controls
    /// zeroed again.
    fn respawn(&mut self) {
        self.placed = self.spawn;
        self.kept_vel = NOTHING;
        self.reset();
        self.throttle = F::ZERO;
        self.elevator = F::ZERO;
        self.elevator_command = F::ZERO;
        self.aileron = F::ZERO;
        self.rudder = F::ZERO;
    }

    /// Which prop the plane shows.
    pub fn prop(&self) -> Prop {
        if self.wreck.is_some() { Prop::Bent } else { Prop::Blur }
    }

    /// The wreck word as the original holds it: 0, 1 or 13.
    pub fn wreck_word(&self) -> u32 {
        self.wreck.map_or(0, |r| r as u32)
    }
}

/// The gravity force for a scale and a mass: `(0, (−scale·m)·g, 0, 1)`, in the original's order.
fn gravity_force(scale: F, mass: F) -> Vf {
    let y = F(G) * (-scale * mass);
    Vf::new(0.0, y.0, 0.0, 1.0)
}

/// The terrain's size as the map-edge test computes it: root cells times 256 times the sample spacing,
/// across and along.
pub fn map_extent(cells_x: F, scale_x: F, cells_z: F, scale_z: F) -> (F, F) {
    (cells_x * F(CELLS_PER_ROOT) * scale_x, cells_z * F(CELLS_PER_ROOT) * scale_z)
}

/// The map-edge test: the point is within [`EDGE_MARGIN`] of the terrain's edge, or off it.
pub fn near_edge(at: Vf, (width, depth): (F, F)) -> bool {
    let m = F(EDGE_MARGIN);
    let (x, z) = (F(at.x()), F(at.z()));
    let inside = m <= x && x <= width - m && m <= z && z <= depth - m;
    !inside
}

impl State for Plane {
    fn walk(&self, w: &mut Walk<'_>) {
        w.group("body", |w| {
            w.f("mass", self.body.mass);
            w.vf("at", self.body.at);
            w.vf("forward", self.body.forward);
            w.vf("up", self.body.up);
            w.vf("vel", self.body.vel);
            w.vf("spin", self.body.spin);
            w.vf("inertia", self.body.inertia);
            w.vf("damping", self.body.damping);
            w.vf("spin_damping", self.body.spin_damping);
        });
        w.vf("gravity", self.gravity);
        w.f("gravity_scale", self.gravity_scale);
        w.f("edge_scale", self.edge_scale);
        w.f("throttle", self.throttle);
        w.f("engine_power", self.engine_power);
        w.f("elevator", self.elevator);
        w.f("elevator_command", self.elevator_command);
        w.f("aileron", self.aileron);
        w.f("rudder", self.rudder);
        w.bool("grounded", self.grounded);
        w.f("airspeed", self.airspeed);
        w.f("up_y", self.up_y);
        w.u32("wreck", self.wreck_word());
        w.bool("down", self.down);
        w.f("clock", self.clock);
        w.vf("inertia", self.inertia);
        w.vf("spawn_at", self.spawn.at);
        w.vf("spawn_forward", self.spawn.forward);
        w.vf("spawn_up", self.spawn.up);
        w.vf("placed_at", self.placed.at);
        w.vf("kept_vel", self.kept_vel);
        w.bool("latch", self.latch);
        w.f("respawn_clock", self.respawn_clock);
        w.bool("fresh", self.fresh);
    }
}
