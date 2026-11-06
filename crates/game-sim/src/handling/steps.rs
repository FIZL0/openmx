//! **SLUS-21104's bike as [`crate::quad::Steps`] sees it** (F31b5a): the bike's words read into a
//! [`crate::quad::Bike`], and back.
//!
//! The quad's steps (the input limiter, clutch, gearbox, head and tail, gravity, preload, backflip,
//! air spin, engine, airborne control, each wheel's clear, suspension, tire forces and integrate, the
//! rev limiter) are the bike's: the two share the functions, and the bike's words sit where the
//! quad's do (MD75d's `tests/atv_quad/image.rs` reader, the one this follows). The bike's front
//! wheel goes in the quad's left front ([`super::bike::FRONT`]) and its rear in the left rear
//! ([`super::bike::REAR`]); the right-hand two stay at their defaults, and nothing the bike's frame
//! calls reaches them.
//!
//! [`read`] and [`words`] walk one list of fields ([`fields`]) the two ways, so each word read is
//! the word written. What the steps read but do not keep is read and not written: the start gate's
//! penalty, the wheels' ground and brake pointers, the engine's driven wheels, the scene's links.
//! The struck thing's class is the caller's ([`crate::quad::Bike::struck_class`]: the mask it is
//! tested against is a word of the game's, not of the bike's), as is the collision record's
//! resolution ([`crate::quad::Bike::contacts`] is `None`: the gravity's record call is handed on
//! unless the caller holds the record, [`super::record::held`]).
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use super::seed::{BODY, ENGINE, NODE, RECORD, WHEELS, Words};
use crate::quad::Bike;
use ps2_float::{F, Vf};

/// The physics world (`+0xe8`), the start gate (`+0x680`).
const WORLD: u32 = 0xe8;
const GATE: u32 = 0x680;

/// Where the bike at [`At::bike`] keeps what [`read`] read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct At {
    pub bike: u32,
    pub body: u32,
    pub world: u32,
    pub record: u32,
    pub engine: u32,
    /// The engine's gear ratios, torque curve and its revs (`+0x70`, `+0x68`, `+0x8c`).
    pub ratios: u32,
    pub curve_torque: u32,
    pub curve_rpm: u32,
    /// The front and rear wheels, and the scene node.
    pub wheels: [u32; 2],
    pub node: u32,
}

/// The quad's slot each of the bike's wheels takes.
const SLOTS: [usize; 2] = [super::bike::FRONT, super::bike::REAR];

/// One word of a field, read or written.
trait Visit {
    fn word(&mut self, at: u32, x: &mut u32);
    /// The `mask` bits of the word at `at`, in place: the rest of the word is not the bike's.
    fn masked(&mut self, at: u32, mask: u32, x: &mut u32);

    fn f(&mut self, at: u32, x: &mut F) {
        let mut w = x.0.to_bits();
        self.word(at, &mut w);
        *x = F(f32::from_bits(w));
    }
    fn v(&mut self, at: u32, x: &mut Vf) {
        for k in 0..4 {
            let mut w = x.0[k].to_bits();
            self.word(at + 4 * k as u32, &mut w);
            x.0[k] = f32::from_bits(w);
        }
    }
    fn b(&mut self, at: u32, x: &mut bool) {
        let mut w = u32::from(*x);
        self.word(at, &mut w);
        *x = w != 0;
    }
    /// A flag the bike keeps the other way round.
    fn not(&mut self, at: u32, x: &mut bool) {
        let mut y = !*x;
        self.b(at, &mut y);
        *x = !y;
    }
    fn i(&mut self, at: u32, x: &mut i32) {
        let mut w = *x as u32;
        self.word(at, &mut w);
        *x = w as i32;
    }
    /// The byte at `at` (any alignment).
    fn byte(&mut self, at: u32, x: &mut u8) {
        let shift = 8 * (at & 3);
        let mut w = u32::from(*x) << shift;
        self.masked(at & !3, 0xff << shift, &mut w);
        *x = (w >> shift) as u8;
    }
}

struct Read<'a, M>(&'a M);

impl<M: Words> Visit for Read<'_, M> {
    fn word(&mut self, at: u32, x: &mut u32) {
        *x = self.0.word(at);
    }
    fn masked(&mut self, at: u32, mask: u32, x: &mut u32) {
        *x = self.0.word(at) & mask;
    }
}

struct Write<'a, M> {
    m: &'a M,
    out: Vec<(u32, u32)>,
}

impl<M: Words> Visit for Write<'_, M> {
    fn word(&mut self, at: u32, x: &mut u32) {
        self.out.push((at, *x));
    }
    fn masked(&mut self, at: u32, mask: u32, x: &mut u32) {
        self.out.push((at, (self.m.word(at) & !mask) | (*x & mask)));
    }
}

/// Where the bike at `bike` keeps its parts; an error for a part it has none of.
pub fn at(m: &impl Words, bike: u32) -> Result<At, String> {
    let need = |o: u32, what: &str| match m.word(bike + o) {
        0 => Err(format!("the bike at {bike:#010x} has no {what} (+{o:#x})")),
        p => Ok(p),
    };
    let engine = need(ENGINE, "engine")?;
    Ok(At {
        bike,
        body: need(BODY, "rigid body")?,
        world: need(WORLD, "physics world")?,
        record: need(RECORD, "collision record")?,
        engine,
        ratios: m.word(engine + 0x70),
        curve_torque: m.word(engine + 0x68),
        curve_rpm: m.word(engine + 0x8c),
        wheels: [need(WHEELS[0], "front wheel")?, need(WHEELS[1], "rear wheel")?],
        node: need(NODE, "scene node")?,
    })
}

/// The quad's air keys, all zero: [`fields`] reads those the steps read.
fn air_keys() -> crate::air::Params {
    crate::air::Params {
        airborne_control_moment: Vf::ZERO,
        airborne_damping: F::ZERO,
        airborne_control_force: F::ZERO,
        brake_gyro_torque: F::ZERO,
        unroll_gain: F::ZERO,
        unroll_torque: F::ZERO,
        unyaw_gain: F::ZERO,
        unyaw_torque: F::ZERO,
        control_authority_time: F::ZERO,
        preload_time: F::ZERO,
        max_preload: F::ZERO,
        min_preload_time: F::ZERO,
        max_preload_time: F::ZERO,
        min_dpad_preload_time: F::ZERO,
        max_dpad_preload_time: F::ZERO,
        min_backflip_time: F::ZERO,
        max_backflip_time: F::ZERO,
        min_backflip_torque: F::ZERO,
        max_backflip_torque: F::ZERO,
        backflip_torque_rate_increase: F::ZERO,
        backflip_torque_rate_decrease: F::ZERO,
        front_wreck_speed_limit: F::ZERO,
        back_wreck_speed_limit: F::ZERO,
        side_wreck_speed_limit: F::ZERO,
        racing_down_wreck_speed_limit: F::ZERO,
        freestyle_down_wreck_speed_limit: F::ZERO,
        ground_pitch_down_wreck_angle: F::ZERO,
        ground_pitch_up_wreck_angle: F::ZERO,
        airborne_pitch_down_wreck_angle: F::ZERO,
        airborne_pitch_up_wreck_angle: F::ZERO,
        airborne_roll_wreck_angle: F::ZERO,
        no_hander_wreck_speed_limit: F::ZERO,
        no_hander_max_roll_angle: F::ZERO,
        joust_wreck_height: F::ZERO,
        crossup_wreck: F::ZERO,
        crossup_wreck_time: F::ZERO,
        rotation_speed: None,
    }
}

fn blank() -> Bike {
    Bike {
        held: Default::default(),
        gas_cut: Default::default(),
        cut_time: F::ZERO,
        joystick: Default::default(),
        stick: Default::default(),
        clutch: F::ZERO,
        gate_cut: false,
        clutch_keys: Default::default(),
        ride: Default::default(),
        engine: Default::default(),
        air_keys: air_keys(),
        air: Default::default(),
        stick_hold: F::ZERO,
        preload_cue: false,
        spin_applied: false,
        spin_active: false,
        touching: false,
        struck_class: false,
        buttons: 0,
        right_y: F::ZERO,
        up_y: F::ZERO,
        stunt_id: 0,
        rider_stunt: 0,
        flip_clip: Default::default(),
        flip_stunt: 0,
        gas_gyro_torque: F::ZERO,
        gyro: Default::default(),
        wheels: Default::default(),
        more: Default::default(),
        ground: Default::default(),
        cofg: Vf::ZERO,
        spins: Default::default(),
        parts: Default::default(),
        contacts: None,
    }
}

/// Every word of the bike's [`Bike`] fields keep, `bike`-relative or off the part [`At`] says.
fn fields(k: &mut Bike, at: &At, v: &mut impl Visit) {
    let (o, bo, wo, eo, ro) = (at.bike, at.body, at.world, at.engine, at.record);

    // The input limiter's.
    let l = &mut k.held.lock;
    v.not(o + 0xdd0, &mut l.steer);
    v.not(o + 0xdd4, &mut l.throttle);
    v.not(o + 0x1298, &mut l.steer_was);
    v.b(o + 0x129c, &mut l.latch);
    let j = &mut k.joystick;
    for (a, x) in [
        (0x12d8, &mut j.x_axis_rate_limit),
        (0x12dc, &mut j.y_axis_rate_limit),
        (0x12e0, &mut j.x_dpad_rate_limit),
        (0x12e4, &mut j.y_dpad_rate_limit),
        (0x1310, &mut j.gas_rate_limit),
        (0x1314, &mut j.brake_rate_limit),
        (0x1318, &mut j.clutch_in_rate_limit),
        (0x131c, &mut j.clutch_out_rate_limit),
    ] {
        v.f(o + a, x);
    }
    v.b(o + 0x1764, &mut k.gas_cut.latched);
    v.f(o + 0x1768, &mut k.gas_cut.timer);
    v.f(o + 0x176c, &mut k.cut_time);
    v.f(o + 0x12a8, &mut k.stick.x);
    v.f(o + 0x12ac, &mut k.stick.y);
    v.b(o + 0x12a0, &mut k.stick.digital[0]);
    v.b(o + 0x12a4, &mut k.stick.digital[1]);
    v.f(o + 0x12c0, &mut k.clutch);

    // The clutch ramp's keys and clock.
    let c = &mut k.clutch_keys;
    for (a, x) in [
        (0x1350, &mut c.clutch_torque_scale_time),
        (0x1358, &mut c.max_clutch_torque_scale_speed),
        (0x135c, &mut c.max_clutch_torque_scale),
        (0x13dc, &mut c.up_shift_rpm),
        (0x13e0, &mut c.down_shift_rpm),
        (0x1320, &mut c.cofg_travel_forward),
        (0x1324, &mut c.cofg_travel_back),
        (0x1328, &mut c.clutch_cofg_travel_back),
        (0x132c, &mut c.brake_cofg_travel_forward),
    ] {
        v.f(o + a, x);
    }
    v.v(o + 0x1130, &mut c.cofg);
    v.v(o + 0x1150, &mut c.balance_cofg);
    v.f(wo + 4, &mut c.drag);
    v.f(o + 0x1348, &mut k.ride.clutch_timer);
    v.f(o + 0x134c, &mut k.ride.clutch_scale_start);
    v.f(o + 0x1354, &mut k.ride.clutch_cofg_weight);

    // The engine, and its tables (read whole, past the gear and point counts).
    let e = &mut k.engine;
    v.i(eo + 0x5c, &mut e.gear);
    v.i(eo + 0x6c, &mut e.gears);
    v.b(eo + 0x58, &mut e.clutch_locked);
    for (a, x) in [
        (0x34, &mut e.final_drive),
        (0x74, &mut e.ratio),
        (0x78, &mut e.inv_ratio),
        (0x7c, &mut e.clutch_torque_scale),
        (0xac, &mut e.rev_limit),
        (0x2c, &mut e.idle_throttle),
        (0x30, &mut e.engine_braking),
        (0x3c, &mut e.inv_inertia),
        (0x44, &mut e.clutch_static),
        (0x48, &mut e.clutch_kinetic),
        (0x54, &mut e.min_rpm),
        (0x98, &mut e.torque),
        (0x9c, &mut e.wheel_torque),
        (0xa0, &mut e.rpm),
        (0xa4, &mut e.road_rpm),
        (0xb4, &mut e.torque_scale),
    ] {
        v.f(eo + a, x);
    }
    let (mut points, mut index, mut frac) = (e.curve.points, e.curve.index, e.curve.frac());
    v.word(eo + 0x80, &mut points);
    v.word(eo + 0x84, &mut index);
    v.f(eo + 0x88, &mut frac);
    e.curve = crate::r#move::Curve::at(points, index, frac);
    for (n, r) in e.gear_ratios.iter_mut().enumerate() {
        v.f(at.ratios + 4 * n as u32, r);
    }
    for n in 0..e.curve_rpm.len() {
        v.f(at.curve_rpm + 4 * n as u32, &mut e.curve_rpm[n]);
        v.f(at.curve_torque + 4 * n as u32, &mut e.curve_torque[n]);
    }

    // The air steps' keys and state.
    let a = &mut k.air_keys;
    for (off, x) in [
        (0xc5c, &mut a.preload_time),
        (0xc64, &mut a.min_preload_time),
        (0xc68, &mut a.max_preload_time),
        (0xc6c, &mut a.max_preload),
        (0x1160, &mut a.min_dpad_preload_time),
        (0x1164, &mut a.max_dpad_preload_time),
        (0x11a4, &mut a.min_backflip_time),
        (0x11a8, &mut a.max_backflip_time),
        (0x11b0, &mut a.min_backflip_torque),
        (0x11b4, &mut a.max_backflip_torque),
        (0x11b8, &mut a.backflip_torque_rate_increase),
        (0x11bc, &mut a.backflip_torque_rate_decrease),
        (0x1404, &mut a.airborne_pitch_down_wreck_angle),
        (0x1408, &mut a.airborne_pitch_up_wreck_angle),
        (0x13e8, &mut a.front_wreck_speed_limit),
        (0x13ec, &mut a.back_wreck_speed_limit),
        (0x13f0, &mut a.side_wreck_speed_limit),
        (0x1180, &mut a.airborne_damping),
        (0x1184, &mut a.airborne_control_force),
        (0x1330, &mut a.brake_gyro_torque),
        (0x1338, &mut a.unroll_gain),
        (0x133c, &mut a.unroll_torque),
        (0x1340, &mut a.unyaw_gain),
        (0x1344, &mut a.unyaw_torque),
    ] {
        v.f(o + off, x);
    }
    v.v(o + 0x1170, &mut a.airborne_control_moment);
    let s = &mut k.air;
    v.b(o + 0xc50, &mut s.preload_ready);
    v.b(o + 0xc54, &mut s.preload_firing);
    v.b(o + 0xc58, &mut s.preload_armed);
    v.f(o + 0xc60, &mut s.preload_timer);
    v.v(o + 0xc70, &mut s.preload_force);
    v.b(o + 0x1194, &mut s.flip_torque_applied);
    v.b(o + 0x119c, &mut s.flip_active);
    v.b(o + 0x11a0, &mut s.flip_armed);
    v.f(o + 0x11ac, &mut s.flip_grace);
    v.v(o + 0x11c0, &mut s.flip_torque);

    v.f(o + 0x12b0, &mut k.stick_hold);
    v.b(o + 0x700, &mut k.preload_cue);
    v.b(o + 0x11d0, &mut k.spin_applied);
    v.b(o + 0x11d8, &mut k.spin_active);
    v.b(ro + 0x34, &mut k.touching);
    let mut buttons = u32::from(k.buttons);
    v.masked(o + 0x14ec, 0xf, &mut buttons);
    k.buttons = buttons as u8;
    v.i(o + 0x1528, &mut k.stunt_id);
    v.i(o + 0x1524, &mut k.rider_stunt);
    v.i(o + 0x1754, &mut k.flip_clip.0);
    v.word(o + 0x13b8, &mut k.flip_stunt);
    v.f(o + 0x1334, &mut k.gas_gyro_torque);
    v.b(o + 0x1290, &mut k.gyro.brake);
    v.b(o + 0x1294, &mut k.gyro.gas);
    v.v(bo + 0x1a0, &mut k.cofg);

    // Each wheel, in its quad slot.
    for (&slot, &w) in SLOTS.iter().zip(&at.wheels) {
        wheel(k, slot, w, v);
    }

    // The head and tail's, the gravity's and the 360's.
    let p = &mut k.parts;
    v.v(o + 0x1140, &mut p.cofg_off);
    v.v(bo + 0x60, &mut p.last_position);
    v.b(bo, &mut p.body_on);
    v.byte(o + 0x45, &mut p.flags);
    let sp = &mut p.spin;
    v.b(o + 0x11dc, &mut sp.armed);
    for (off, x) in [
        (0x11e0, &mut sp.min_time),
        (0x11e4, &mut sp.max_time),
        (0x11e8, &mut sp.grace),
        (0x11ec, &mut sp.min_pitch),
        (0x11f0, &mut sp.max_pitch),
        (0x11f4, &mut sp.min_yaw),
        (0x11f8, &mut sp.max_yaw),
        (0x1200, &mut sp.rate),
        (0x1220, &mut sp.heading),
        (0x12b4, &mut sp.flick),
    ] {
        v.f(o + off, x);
    }
    v.v(o + 0x1210, &mut sp.moment);
    v.i(o + 0x1224, &mut sp.dir);
    v.word(o + 0x13bc, &mut sp.stunt_right);
    v.word(o + 0x13c0, &mut sp.stunt_left);
    v.i(o + 0x1758, &mut sp.clip);
    v.word(o + 0x143c, &mut p.rear_node);

    // The scene node's frames and flag byte (its right and up rows' heights are the backflip's).
    let n = &mut p.scene.nodes[0];
    for r in 0..4 {
        v.v(at.node + 0x30 + 0x10 * r as u32, &mut n.local[r]);
        v.v(at.node + 0x70 + 0x10 * r as u32, &mut n.world[r]);
    }
    v.byte(at.node + 0x100, &mut n.flags);
    v.f(at.node + 0x84, &mut k.right_y);
    v.f(at.node + 0x94, &mut k.up_y);
}

/// The bike's wheel at `a`, into the quad's slot `i`: [`crate::wheel::Wheel`]'s words,
/// [`crate::atv_wheel::More`]'s, the drive's and [`crate::atv_wheel::Spin`]'s (its spin, torque,
/// spinning flag, rear-pair flag, donut and `1/r` are the drive's and the quad wheel's, which the
/// steps copy in: they are read once, as the drive's and the wheel's).
fn wheel(k: &mut Bike, i: usize, a: u32, v: &mut impl Visit) {
    let w = &mut k.wheels[i];
    v.b(a + 0x2c, &mut w.steered);
    v.b(a + 0x30, &mut w.balancing);
    v.b(a + 0xdc, &mut w.contact);
    v.v(a + 0x130, &mut w.contact_normal);
    v.f(a + 0x70, &mut w.max_steer_angle);
    v.f(a + 0x8ac, &mut w.steer_angle);
    v.f(a + 0x48, &mut w.spring_rate);
    for (off, x) in [
        (0x3c, &mut w.max_compression),
        (0x40, &mut w.max_damper_speed),
        (0x44, &mut w.extension_rate),
        (0x4c, &mut w.damping_out),
        (0x50, &mut w.damping_in),
        (0xf0, &mut w.compression),
        (0xf4, &mut w.last_compression),
        (0xf8, &mut w.spring_force),
        (0x214, &mut w.probe_length),
        (0x60, &mut w.steer_torque),
        (0x64, &mut w.steer_damping),
        (0x6c, &mut w.steer_inv_inertia),
        (0x74, &mut w.min_steer_angle),
        (0x8b0, &mut w.steer_rate),
    ] {
        v.f(a + off, x);
    }
    for (off, x) in [
        (0x80, &mut w.probe),
        (0x90, &mut w.mount),
        (0x100, &mut w.probe_end),
        (0x120, &mut w.contact_point),
        (0x140, &mut w.load_point),
        (0x150, &mut w.mount_world),
        (0x160, &mut w.probe_end_world),
        (0x170, &mut w.contact_velocity),
        (0x180, &mut w.chassis_velocity),
        (0x200, &mut w.ground_normal),
    ] {
        v.v(a + off, x);
    }
    v.i(a + 0xd0, &mut w.ray_hit);
    v.i(a + 0xd4, &mut w.swept_contact);
    v.i(a + 0xd8, &mut w.terrain_wins);
    v.i(a + 0xec, &mut w.ray_steps);

    let m = &mut k.more[i];
    v.byte(a + super::ground::WHEEL_RECORD, &mut m.record);
    v.v(a + 0xc0, &mut m.kept);
    v.v(a + 0x190, &mut m.chassis_velocity_local);
    v.i(a + 0x1a0, &mut m.ground_known);
    v.v(a + 0x1b0, &mut m.ground_velocity);
    v.v(a + 0x1c0, &mut m.ground_point_velocity);
    v.v(a + 0x8c0, &mut m.steer_axis);

    let d = &mut k.parts.drive[i];
    v.f(a + 0x1d4, &mut d.spin);
    v.f(a + 0x210, &mut d.torque);
    v.b(a + 0xe0, &mut d.from_torque);
    v.f(a + 0x858, &mut d.forward);

    let s = &mut k.spins[i];
    v.i(a + 0x34, &mut s.parametric);
    v.f(a + 0x58, &mut s.inv_inertia);
    v.f(a + 0x5c, &mut s.tire_inertia);
    v.v(a + 0xa0, &mut s.arm);
    v.i(a + 0xb0, &mut s.no_moment);
    v.f(a + 0xb8, &mut s.other_load_scale);
    v.b(a + 0xe4, &mut s.slip.locked);
    v.b(a + 0xe8, &mut s.slip.sliding);
    v.f(a + 0x1d0, &mut s.angle);
    v.f(a + 0x224, &mut s.cleared[0]);
    v.f(a + 0x228, &mut s.cleared[1]);
    v.f(a + 0x234, &mut s.side_per_load);
    for (j, t) in s.surfaces.iter_mut().enumerate() {
        let at = |n: usize| a + 0x238 + 0x40 * n as u32 + 4 * j as u32;
        let tables = t.drive.iter_mut().chain(t.side.iter_mut()).chain(t.drag.iter_mut());
        for (n, x) in tables.enumerate() {
            v.f(at(n), x);
        }
    }
    v.v(a + 0x840, &mut s.local_velocity);
    v.v(a + 0x850, &mut s.local_force);
    for r in 0..4 {
        v.v(a + 0x8d0 + 0x10 * r as u32, &mut s.frame[r]);
    }
}

/// **The bike at `bike` as a [`Bike`]**, and where its parts are.
///
/// Besides [`fields`]' words, what the steps read and do not keep: the start gate's penalty (its
/// `+0x1d0` above 0), each wheel's ground (on still ground only: a wheel's collision object
/// `+0x1f0` or mover `+0x1f4` is an error), its brake (the steps take the lever off the quad),
/// [`crate::wheel::Wheel::has_ground`] (`+0x1e0` set), the engine's driven wheels (`+0x90`, `+0x94`)
/// and the spin's stunt copies the steps take off the drive and the quad wheel. The scene is the
/// bike's node alone, which is to have no parent (its world frame is its own).
pub fn read(m: &impl Words, bike: u32) -> Result<(Bike, At), String> {
    let at = at(m, bike)?;
    let mut k = blank();
    fields(&mut k, &at, &mut Read(m));
    let gate = m.word(bike + GATE);
    k.gate_cut = gate != 0 && F::ZERO < m.f(gate + 0x1d0);
    for (&slot, &w) in SLOTS.iter().zip(&at.wheels) {
        if m.word(w + 0x1f0) != 0 || m.word(w + 0x1f4) != 0 {
            return Err(format!("the bike's wheel {w:#010x} stands on a collision object or a mover"));
        }
        k.wheels[slot].has_ground = m.word(w + super::ground::WHEEL_TERRAIN) != 0;
        k.ground[slot] = crate::atv_wheel::Ground::Still;
        let s = &mut k.spins[slot];
        s.brake = match m.word(w + 0x24) {
            0 => None,
            p => Some(m.f(p)),
        };
    }
    let slot = |p: u32| at.wheels.iter().position(|&w| w == p).map(|k| SLOTS[k]);
    let first = slot(m.word(at.engine + 0x90)).ok_or("the engine's first driven wheel is neither of the bike's")?;
    let second = match m.word(at.engine + 0x94) {
        0 => None,
        p => Some(slot(p).ok_or("the engine's second driven wheel is neither of the bike's")?),
    };
    k.parts.driven = (first, second);
    k.parts.record = true;
    if m.word(at.node + 0xf0) != 0 {
        return Err(format!("the bike's node {:#010x} has a parent, which the scene does not hold", at.node));
    }
    Ok((k, at))
}

/// [`read`]'s inverse: every word [`fields`] read, as `k` has it, `m` the memory the masked words'
/// other bits come from.
///
/// F31b5b: the node's children are not the scene's, but its move marks them: a node current in `m`
/// and out of date in `k` has had them marked out of date too ([`children_out_of_date`]), and their
/// flag words are put back after.
pub fn words(k: &Bike, at: &At, m: &impl Words) -> Vec<(u32, u32)> {
    let mut k = k.clone();
    let mut w = Write { m, out: Vec::new() };
    fields(&mut k, at, &mut w);
    let mut out = w.out;
    if m.word(at.node + FLAGS) & CLEAN != 0 && k.parts.scene.nodes[0].flags & CLEAN as u8 == 0 {
        children_out_of_date(m, m.word(at.node + CHILD), &mut out);
    }
    out
}

/// A scene node's flag word (its first byte's bit 0: its world frame current), its first child and
/// its next sibling.
const FLAGS: u32 = 0x100;
const CLEAN: u32 = 0x01;
const CHILD: u32 = 0xf4;
const SIBLING: u32 = 0xf8;

/// **The nodes under a moved one marked out of date** (`atv_node_out_of_date`, as
/// [`crate::atv_moto::Scene`] marks its own): down the sibling list from `first`, each current node
/// marked, and its children after it; under a node already out of date nothing is marked. The flag
/// words, into `out`.
fn children_out_of_date(m: &impl Words, first: u32, out: &mut Vec<(u32, u32)>) {
    let mut at = first;
    while at != 0 {
        let w = m.word(at + FLAGS);
        if w & CLEAN != 0 {
            out.push((at + FLAGS, w & !CLEAN));
            children_out_of_date(m, m.word(at + CHILD), out);
        }
        at = m.word(at + SIBLING);
    }
}

// --- F31b5b: the PostProcess's parts -------------------------------------------------------------

/// The vtables a collidable's kind is told by: a ray probe's and a mesh's. They are the game's, so
/// whoever knows the game hands them in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kinds {
    pub rays: u32,
    pub mesh: u32,
}

/// Where the bike's collidables are, by [`crate::atv_sync::slot`] (0 for none).
pub type PartsAt = [u32; 5];

/// The bike's riding probe, frame mesh and active probe (bike-relative), and the byte holding the
/// collision sync's bit (bit 0).
const RIDING: u32 = 0xcc;
const FRAME: u32 = 0xd0;
const ACTIVE: u32 = 0xd8;
const SYNC: u32 = 0x44;
/// The bike's and its node's spatial database entries (`+8`).
const ENTRY: u32 = 0x8;

/// **The bike at `bike` as [`read`] reads it, with what its PostProcess's sync and spatial update
/// read besides**: the bike's and its node's database entries (there or not), the node's box and
/// own bounds, the collision sync's bit, and the collidables, each a ray probe or a mesh by `kinds`.
/// The bike's riding probe (`+0xcc`) is [`crate::quad::Part::Riding`], its frame mesh (`+0xd0`)
/// [`crate::quad::Part::Frame`]; its active probe (`+0xd8`) is the riding one, or else
/// [`crate::quad::Part::Wreck`].
///
/// Refused: a collidable of another kind, and a bike or node in the database whose node's bounds
/// gather takes in other nodes (its `+0x10c` list), which the scene does not hold; out of the
/// database the node's box is never asked for.
pub fn read_post(m: &impl Words, bike: u32, kinds: Kinds) -> Result<(Bike, At, PartsAt), String> {
    let (k, at, parts) = read_sync(m, bike, kinds)?;
    if m.word(at.node + 0x10c) != 0 && (m.b(bike + ENTRY) || m.b(at.node + ENTRY)) {
        return Err(format!("the bike's node {:#010x} is in the database and gathers its bounds off other nodes", at.node));
    }
    Ok((k, at, parts))
}

/// **[`read_post`] without its refusal of a node in the database that gathers other nodes** (F47):
/// for the collision sync and the node's position, which read the node's own frame only; its box
/// is then [`super::node::bounds`]' and [`super::node::owner_box`]'s, on RAM.
pub fn read_sync(m: &impl Words, bike: u32, kinds: Kinds) -> Result<(Bike, At, PartsAt), String> {
    use crate::atv_sync::slot;
    use crate::quad::Part;
    let (mut k, at) = read(m, bike)?;
    let mut parts = [0; 5];
    let (riding, frame, active) = (m.word(bike + RIDING), m.word(bike + FRAME), m.word(bike + ACTIVE));
    let c = &mut k.parts.collidables;
    if riding != 0 {
        parts[slot(Part::Riding)] = riding;
        c.riding = Some(Part::Riding);
    }
    if frame != 0 {
        parts[slot(Part::Frame)] = frame;
        c.frame = Some(Part::Frame);
    }
    c.active = match active {
        0 => None,
        a if a == riding => Some(Part::Riding),
        a => {
            parts[slot(Part::Wreck)] = a;
            Some(Part::Wreck)
        }
    };
    for (s, &a) in parts.iter().enumerate() {
        if a == 0 {
            continue;
        }
        let xyz = |p: u32| Vf::new(m.f(p + 0xc).0, m.f(p + 0x10).0, m.f(p + 0x14).0, 0.0);
        let half = |p: u32| Vf::new(m.f(p + 0x18).0, m.f(p + 0x1c).0, m.f(p + 0x20).0, 0.0);
        let rows = |p: u32| std::array::from_fn(|r| m.v(p + 0x10 * r as u32));
        let vt = m.word(a + 0xc);
        k.parts.collidables.states[s].kind = if vt == kinds.rays {
            let (tree, bound) = (m.word(a + 0x17c), m.word(a + 0x180));
            crate::atv_sync::Kind::Rays { tree: (tree != 0).then(|| (xyz(tree), half(tree))), bound: (m.v(bound), m.v(bound + 0x10)) }
        } else if vt == kinds.mesh {
            let mesh = m.word(a + 0x244);
            crate::atv_sync::Kind::Mesh { local: rows(a + 0x170), centre: xyz(mesh), half: half(mesh), stood: [Vf::ZERO; 4], before: [Vf::ZERO; 4] }
        } else {
            return Err(format!("the bike's collidable {a:#010x} is neither a ray probe nor a mesh (vtable {vt:#010x})"));
        };
    }
    // The database entries are pointers the sync reads only: there or not.
    k.parts.spatial = m.b(bike + ENTRY);
    k.parts.scene.nodes[0].spatial = m.b(at.node + ENTRY);
    for (c, &a) in k.parts.collidables.states.iter_mut().zip(&parts) {
        c.spatial = a != 0 && m.b(a + 0x20);
    }
    post_fields(&mut k, &at, &parts, &mut Read(m));
    Ok((k, at, parts))
}

/// [`read_post`]'s inverse: [`words`], then the words [`read_post`] read besides (the database
/// entries, a kind's box and a mesh's frame on the node only read).
pub fn post_words(k: &Bike, at: &At, parts: &PartsAt, m: &impl Words) -> Vec<(u32, u32)> {
    let mut out = words(k, at, m);
    let mut k = k.clone();
    // The sync's bit shares a word with what [`words`] wrote: masked over that, not over RAM.
    let over = Over { m, out: &out };
    let mut w = Write { m: &over, out: Vec::new() };
    post_fields(&mut k, at, parts, &mut w);
    out.extend(w.out);
    out
}

/// RAM with words written over it, the last write of a word the one read.
struct Over<'a, M> {
    m: &'a M,
    out: &'a [(u32, u32)],
}

impl<M: Words> Words for Over<'_, M> {
    fn word(&self, at: u32) -> u32 {
        self.out.iter().rev().find(|w| w.0 == at).map_or_else(|| self.m.word(at), |w| w.1)
    }
}

fn post_fields(k: &mut Bike, at: &At, parts: &PartsAt, v: &mut impl Visit) {
    let p = &mut k.parts;
    let mut sync = u32::from(p.collidables.sync);
    v.masked(at.bike + SYNC, 1, &mut sync);
    p.collidables.sync = sync != 0;
    let n = &mut p.scene.nodes[0];
    v.v(at.node + 0xb0, &mut n.own_centre);
    v.v(at.node + 0xc0, &mut n.own_half);
    v.v(at.node + 0xd0, &mut n.centre);
    v.v(at.node + 0xe0, &mut n.half);
    v.f(at.node + 0x104, &mut n.radius);
    v.byte(at.node + 0x101, &mut n.more);
    v.f(at.node + 0x108, &mut n.reach);
    for (c, &a) in p.collidables.states.iter_mut().zip(parts) {
        if a == 0 {
            continue;
        }
        v.v(a + 0x50, &mut c.centre);
        v.f(a + 0x70, &mut c.reach);
        for r in 0..4 {
            v.v(a + 0x130 + 0x10 * r as u32, &mut c.frame[r]);
        }
        if let crate::atv_sync::Kind::Mesh { stood, before, .. } = &mut c.kind {
            for r in 0..4 {
                v.v(a + 0x1b0 + 0x10 * r as u32, &mut stood[r]);
                v.v(a + 0x1f0 + 0x10 * r as u32, &mut before[r]);
            }
        }
    }
}
