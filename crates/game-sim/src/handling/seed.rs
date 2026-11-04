//! **SLUS-21104's bike, read out of a RAM image** (F31a): the state a [`Handling::MxVsAtv`] ride
//! starts from when it starts where the original's bike is, as C15's ride and flip do (state 4,
//! `context/mxvatv-ps2/state04.ee.bin`; where its bike is: research/oracle.txt `# F31`).
//!
//! The bike's rigid body is SLUS-21104's ([`crate::atv_body::Body`], MD75's map), not the MX Unleashed
//! chassis [`crate::ride::Ride`] carries: MX vs. ATV's bike rides its physics world as the quad does,
//! with two wheels. So the seed is in the types SLUS-21104's ports already use: the body, each wheel
//! as [`crate::atv_wheel`] reads it ([`crate::wheel::Wheel`] at the ATV's offsets), and the engine
//! ([`crate::r#move::Engine`] at MD75b's).
//!
//! Each reader has a `*_words` inverse: the words, address and value, that the read value stands for.
//! A reader is right when every word it gives back is the image's (`tests/mxvatv_bike_ride.rs`).
//!
//! The bike's tuning (the `.phy` keys: the wheels' springs, the engine's gears and curve, the body's
//! limits) is read from the image too, where the game put it when it read the bike's files: the files
//! themselves are not on this disk.
//!
//! [`Handling::MxVsAtv`]: super::Handling::MxVsAtv
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::atv_body::Body;
use crate::r#move::{Curve, Engine, MAX_RPM_POINTS};
use crate::wheel::Wheel;
use ps2_float::{F, Vf};

/// The bike object's pointers, bike-relative: the rigid body, the scene node, the collision record,
/// the two wheels (front, rear) and the engine.
pub const BODY: u32 = 0xb8;
pub const NODE: u32 = 0xe4;
pub const RECORD: u32 = 0xc8;
pub const WHEELS: [u32; 2] = [0x17b0, 0x17b4];
pub const ENGINE: u32 = 0x142c;

/// A word reader over RAM: the image's, or the interpreter's.
pub trait Words {
    fn word(&self, at: u32) -> u32;

    /// A float as it sits in RAM, bit for bit: a denormal stays one (`F::from_bits` reads it as the
    /// FPU would, as zero).
    fn f(&self, at: u32) -> F {
        F(f32::from_bits(self.word(at)))
    }
    fn v(&self, at: u32) -> Vf {
        Vf([0, 4, 8, 12].map(|k| f32::from_bits(self.word(at + k))))
    }
    fn b(&self, at: u32) -> bool {
        self.word(at) != 0
    }
    fn i(&self, at: u32) -> i32 {
        self.word(at) as i32
    }
}

impl<T: Fn(u32) -> u32> Words for T {
    fn word(&self, at: u32) -> u32 {
        self(at)
    }
}

/// What [`read`] read, and where.
#[derive(Clone, Debug)]
pub struct Seed {
    pub body: Body,
    pub wheels: [Wheel; 2],
    pub engine: Engine,
    /// The addresses the three were read from: the body, the front and rear wheel, the engine, and
    /// the engine's ratio, torque and revs tables.
    pub at: At,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct At {
    pub body: u32,
    pub wheels: [u32; 2],
    pub engine: u32,
    pub ratios: u32,
    pub curve_torque: u32,
    pub curve_rpm: u32,
}

/// The bike at `bike` in `m`.
pub fn read(m: &impl Words, bike: u32) -> Seed {
    let engine = m.word(bike + ENGINE);
    let at = At {
        body: m.word(bike + BODY),
        wheels: WHEELS.map(|o| m.word(bike + o)),
        engine,
        ratios: m.word(engine + 0x70),
        curve_torque: m.word(engine + 0x68),
        curve_rpm: m.word(engine + 0x8c),
    };
    Seed { body: body(m, at.body), wheels: at.wheels.map(|w| wheel(m, w)), engine: read_engine(m, &at), at }
}

impl Seed {
    /// Every word the seed stands for, address and value.
    pub fn words(&self) -> Vec<(u32, u32)> {
        let mut out = body_words(&self.body, self.at.body);
        for (w, &a) in self.wheels.iter().zip(&self.at.wheels) {
            out.extend(wheel_words(w, a));
        }
        out.extend(engine_words(&self.engine, &self.at));
        out
    }
}

fn vf_words(out: &mut Vec<(u32, u32)>, at: u32, v: Vf) {
    for (k, w) in v.to_bits().into_iter().enumerate() {
        out.push((at + 4 * k as u32, w));
    }
}

/// The rigid body at `at` (research/oracle.txt `# MD75`). Its mass (`+0x18`), vtable (`+0x110`) and
/// list links are not the integrate's and are not read.
pub fn body(m: &impl Words, at: u32) -> Body {
    Body {
        enabled: m.b(at),
        linear_axes: [0, 1, 2].map(|k| m.b(at + 4 + 4 * k)),
        inv_mass: m.f(at + 0x1c),
        damping: m.v(at + 0x30),
        last_position: m.v(at + 0x60),
        position: m.v(at + 0x70),
        velocity: m.v(at + 0x80),
        last_force: m.v(at + 0x90),
        force: m.v(at + 0xa0),
        accel: m.v(at + 0xb0),
        correction_peak: m.v(at + 0xc0),
        correction: m.v(at + 0xd0),
        max_speed: m.f(at + 0xe0),
        max_accel: m.f(at + 0xe4),
        angular_axes: [0, 1, 2].map(|k| m.b(at + 0x120 + 4 * k)),
        inv_inertia: m.v(at + 0x140),
        ang_damping: m.v(at + 0x150),
        last_quat: m.v(at + 0x170),
        quat: m.v(at + 0x190),
        moment: m.v(at + 0x1b0),
        last_moment: m.v(at + 0x1c0),
        rates: m.v(at + 0x1d0),
        ang_accel: m.v(at + 0x1e0),
        twist_peak: m.v(at + 0x1f0),
        twist: m.v(at + 0x200),
        max_rates: [0, 1, 2].map(|k| m.f(at + 0x210 + 4 * k)),
        max_ang_accel: m.f(at + 0x220),
    }
}

/// [`body`]'s words: the flags as 1 or 0, as the original stores them.
pub fn body_words(b: &Body, at: u32) -> Vec<(u32, u32)> {
    let mut out = vec![(at, u32::from(b.enabled))];
    for k in 0..3u32 {
        out.push((at + 4 + 4 * k, u32::from(b.linear_axes[k as usize])));
        out.push((at + 0x120 + 4 * k, u32::from(b.angular_axes[k as usize])));
        out.push((at + 0x210 + 4 * k, b.max_rates[k as usize].to_bits()));
    }
    out.push((at + 0x1c, b.inv_mass.to_bits()));
    out.push((at + 0xe0, b.max_speed.to_bits()));
    out.push((at + 0xe4, b.max_accel.to_bits()));
    out.push((at + 0x220, b.max_ang_accel.to_bits()));
    for (o, v) in [
        (0x30, b.damping),
        (0x60, b.last_position),
        (0x70, b.position),
        (0x80, b.velocity),
        (0x90, b.last_force),
        (0xa0, b.force),
        (0xb0, b.accel),
        (0xc0, b.correction_peak),
        (0xd0, b.correction),
        (0x140, b.inv_inertia),
        (0x150, b.ang_damping),
        (0x170, b.last_quat),
        (0x190, b.quat),
        (0x1b0, b.moment),
        (0x1c0, b.last_moment),
        (0x1d0, b.rates),
        (0x1e0, b.ang_accel),
        (0x1f0, b.twist_peak),
        (0x200, b.twist),
    ] {
        vf_words(&mut out, at + o, v);
    }
    out
}

/// The wheel's floats, flags and vectors at the ATV's offsets ([`crate::atv_wheel`]'s table; the
/// rest where MX Unleashed has them).
const WHEEL_F: [u32; 17] =
    [0x3c, 0x40, 0x44, 0x48, 0x4c, 0x50, 0x60, 0x64, 0x6c, 0x70, 0x74, 0x8ac, 0x8b0, 0xf0, 0xf4, 0xf8, 0x214];
const WHEEL_I: [u32; 4] = [0xd0, 0xd4, 0xd8, 0xec];
const TOUCHING: u32 = 0xdc;
const WHEEL_B: [u32; 2] = [0x2c, 0x30];
const WHEEL_V: [u32; 11] = [0x80, 0x90, 0x100, 0x120, 0x130, 0x140, 0x150, 0x160, 0x170, 0x180, 0x200];

/// The wheel's fields by offset, for both directions.
fn wheel_fields(w: &mut Wheel) -> ([&mut F; 17], [&mut i32; 4], [&mut bool; 3], [&mut Vf; 11]) {
    (
        [
            &mut w.max_compression,
            &mut w.max_damper_speed,
            &mut w.extension_rate,
            &mut w.spring_rate,
            &mut w.damping_out,
            &mut w.damping_in,
            &mut w.steer_torque,
            &mut w.steer_damping,
            &mut w.steer_inv_inertia,
            &mut w.max_steer_angle,
            &mut w.min_steer_angle,
            &mut w.steer_angle,
            &mut w.steer_rate,
            &mut w.compression,
            &mut w.last_compression,
            &mut w.spring_force,
            &mut w.probe_length,
        ],
        [&mut w.ray_hit, &mut w.swept_contact, &mut w.terrain_wins, &mut w.ray_steps],
        [&mut w.steered, &mut w.balancing, &mut w.contact],
        [
            &mut w.probe,
            &mut w.mount,
            &mut w.probe_end,
            &mut w.contact_point,
            &mut w.contact_normal,
            &mut w.load_point,
            &mut w.mount_world,
            &mut w.probe_end_world,
            &mut w.contact_velocity,
            &mut w.chassis_velocity,
            &mut w.ground_normal,
        ],
    )
}

/// A wheel at `at`: its suspension and steering, as SLUS-21104's wheel steps read and keep them.
/// `+0xdc` (touching) is the contact flag; `+0x1e0` is the ground the ray asks, and only whether it is
/// set is kept ([`Wheel::has_ground`]).
pub fn wheel(m: &impl Words, at: u32) -> Wheel {
    let mut w = Wheel { has_ground: m.word(at + 0x1e0) != 0, ..Wheel::default() };
    let (fs, is, bs, vs) = wheel_fields(&mut w);
    for (x, o) in fs.into_iter().zip(WHEEL_F) {
        *x = m.f(at + o);
    }
    for (x, o) in is.into_iter().zip(WHEEL_I) {
        *x = m.i(at + o);
    }
    *bs[0] = m.b(at + WHEEL_B[0]);
    *bs[1] = m.b(at + WHEEL_B[1]);
    *bs[2] = m.b(at + TOUCHING);
    for (x, o) in vs.into_iter().zip(WHEEL_V) {
        *x = m.v(at + o);
    }
    w
}

/// [`wheel`]'s words (the ground pointer left out: only whether it is set is the wheel's).
pub fn wheel_words(w: &Wheel, at: u32) -> Vec<(u32, u32)> {
    let mut w = *w;
    let (fs, is, bs, vs) = wheel_fields(&mut w);
    let mut out: Vec<(u32, u32)> = fs.into_iter().zip(WHEEL_F).map(|(x, o)| (at + o, x.to_bits())).collect();
    out.extend(is.into_iter().zip(WHEEL_I).map(|(x, o)| (at + o, *x as u32)));
    out.push((at + WHEEL_B[0], u32::from(*bs[0])));
    out.push((at + WHEEL_B[1], u32::from(*bs[1])));
    out.push((at + TOUCHING, u32::from(*bs[2])));
    for (x, o) in vs.into_iter().zip(WHEEL_V) {
        vf_words(&mut out, at + o, *x);
    }
    out
}

/// The engine (research/oracle.txt `# MD75b`): its gearbox, clutch, torque curve and what its update
/// last worked out. The tables are read whole, past the gear and point counts.
fn read_engine(m: &impl Words, at: &At) -> Engine {
    let e = at.engine;
    let mut engine = Engine {
        gear: m.i(e + 0x5c),
        gears: m.i(e + 0x6c),
        final_drive: m.f(e + 0x34),
        ratio: m.f(e + 0x74),
        inv_ratio: m.f(e + 0x78),
        clutch_torque_scale: m.f(e + 0x7c),
        clutch_locked: m.b(e + 0x58),
        rev_limit: m.f(e + 0xac),
        idle_throttle: m.f(e + 0x2c),
        engine_braking: m.f(e + 0x30),
        inv_inertia: m.f(e + 0x3c),
        clutch_static: m.f(e + 0x44),
        clutch_kinetic: m.f(e + 0x48),
        min_rpm: m.f(e + 0x54),
        curve: Curve::at(m.word(e + 0x80), m.word(e + 0x84), m.f(e + 0x88)),
        torque: m.f(e + 0x98),
        wheel_torque: m.f(e + 0x9c),
        rpm: m.f(e + 0xa0),
        road_rpm: m.f(e + 0xa4),
        torque_scale: m.f(e + 0xb4),
        ..Engine::default()
    };
    for (k, r) in engine.gear_ratios.iter_mut().enumerate() {
        *r = m.f(at.ratios + 4 * k as u32);
    }
    for k in 0..MAX_RPM_POINTS {
        engine.curve_rpm[k] = m.f(at.curve_rpm + 4 * k as u32);
        engine.curve_torque[k] = m.f(at.curve_torque + 4 * k as u32);
    }
    engine
}

/// [`read_engine`]'s words.
fn engine_words(g: &Engine, at: &At) -> Vec<(u32, u32)> {
    let e = at.engine;
    let mut out = vec![
        (e + 0x5c, g.gear as u32),
        (e + 0x6c, g.gears as u32),
        (e + 0x58, u32::from(g.clutch_locked)),
        (e + 0x80, g.curve.points),
        (e + 0x84, g.curve.index),
        (e + 0x88, g.curve.frac().to_bits()),
    ];
    for (o, x) in [
        (0x34, g.final_drive),
        (0x74, g.ratio),
        (0x78, g.inv_ratio),
        (0x7c, g.clutch_torque_scale),
        (0xac, g.rev_limit),
        (0x2c, g.idle_throttle),
        (0x30, g.engine_braking),
        (0x3c, g.inv_inertia),
        (0x44, g.clutch_static),
        (0x48, g.clutch_kinetic),
        (0x54, g.min_rpm),
        (0x98, g.torque),
        (0x9c, g.wheel_torque),
        (0xa0, g.rpm),
        (0xa4, g.road_rpm),
        (0xb4, g.torque_scale),
    ] {
        out.push((e + o, x.to_bits()));
    }
    for (k, r) in g.gear_ratios.iter().enumerate() {
        out.push((at.ratios + 4 * k as u32, r.to_bits()));
    }
    for k in 0..MAX_RPM_POINTS {
        out.push((at.curve_rpm + 4 * k as u32, g.curve_rpm[k].to_bits()));
        out.push((at.curve_torque + 4 * k as u32, g.curve_torque[k].to_bits()));
    }
    out
}
