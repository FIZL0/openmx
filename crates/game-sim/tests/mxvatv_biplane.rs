//! MD80b — MX vs. ATV's (SLUS-21104's) biplane step against the original.
//!
//! The same proofs as tests/biplane.rs, on SLUS-21104's plane: MD80a's Desert Oasis image of the
//! biplane (`context/mxvatv-ps2/md80a/biplane.ee.bin`, model one) is the interpreter's RAM, each case's
//! inputs are written into the plane's own fields (and the engine's and the aero model's, through the
//! pointers they hold), the original runs, and every field it writes is compared bit for bit with the
//! port's. Ten thousand generated cases each:
//!
//! 1. **the controls** (`biplane::controls_mxvatv`);
//! 2. **the autopilot** (`biplane::autopilot_mxvatv`), the curve table taken from the same image;
//! 3. **the engine** (`Engine::step_mxvatv`), the smooth noise hooked as tests/biplane.rs hooks it;
//! 4. **the aerodynamics** (`Aero::forces_mxvatv`), the force and moment recorded at the body's entries;
//! 5. **MXVehicle's wreck check**: the terrain's edge test (`Area::outside_mxvatv`) run on the image's
//!    own terrain, and the throw (`biplane::throw_mxvatv`) with the edge test hooked to answer extents
//!    and `rand` hooked to answer the port's generator's numbers.
//!
//! And **the reader** (`Plane::read_mxvatv`): MX vs. ATV's `biplane.phy` read as model one and model two
//! against what the game loaded into the two images, `biplane` (model one) and `biplane3` (model two).
//!
//! The images and the file are game data, so every test skips without them. What is not proved here,
//! and why, is in the journal's last section (research/journal/2026-11-04-md80b-mxvatv-biplane-step).

use game_sim::biplane::{
    Aero, Air, Area, Attitude, Autopilot, Engine, Joystick, Pad, Surfaces, autopilot_mxvatv, controls_mxvatv,
    throw_mxvatv,
};
use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::flight::Flight;
use game_sim::phy::{Plane, mxvatv_model_one};
use game_sim::rng::Rng;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::cell::RefCell;
use std::rc::Rc;

const CASES: usize = 10_000;
const BUDGET: u64 = 200_000;

/// The planes in MD80a's two images (research/oracle.txt # MD80a, # MD80b).
const PLANE: u32 = 0x014d_e810;
const PLANE_TWO: u32 = 0x01b1_3790;
/// SLUS-21104's `$gp` (crt0 sets it at `0x001001ac`), and the biplane's vtable.
const GP: u64 = 0x003d_baf0;
const BIPLANE_VTABLE: u32 = 0x003c_e7b0;

/// A function or global by its research/oracle.txt name (# MD80, # MD80b).
fn f(name: &str) -> u32 {
    addr(name).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The plane's fields, by offset.
mod at {
    pub const FLOWN: u32 = 0x10f0;
    pub const BACK: u32 = 0x310;
    pub const FORWARD: u32 = 0x314;
    pub const THROTTLE: u32 = 0x318;
    pub const GROUNDED: u32 = 0x324;
    pub const STICKS: u32 = 0x10f4; // elevator, aileron, rudder
    pub const BRAKE: u32 = 0x1100;
    pub const SURFACES: u32 = 0x1104; // elevator, command, aileron, rudder, steer
    pub const JOYSTICK: u32 = 0x1118; // twelve floats to MinElevator
    pub const AUTOPILOT: u32 = 0x1148; // six floats
    pub const INTEGRAL: u32 = 0x12e0;
    pub const BETA: u32 = 0x330;
    pub const ALPHA: u32 = 0x334;
    pub const SPEED: u32 = 0x358;
    pub const ROLL: u32 = 0x368;
    pub const NOSE_Y: u32 = 0xbe4;
    pub const BODY: u32 = 0xb8;
    pub const TERRAIN: u32 = 0x3b0;
    pub const GATE: u32 = 0x308;
    pub const GRAVITY: u32 = 0xe8;
    pub const ENGINE: u32 = 0x1164;
    pub const AERO: u32 = 0x1168;
    pub const MODEL: u32 = 0x1320;
}
/// The rigid body's fields, and its vtable's force and moment entries.
const BODY_POS: u32 = 0x70;
const BODY_VEL: u32 = 0x80;
const BODY_RATES: u32 = 0x1d0;
const BODY_VTABLE: u32 = 0x110;
const FORCE_SLOT: u32 = 0x74;
const MOMENT_SLOT: u32 = 0x78;

/// The interpreter on one of MD80a's images, with `$gp` set, or why not.
fn image(name: &str, plane: u32) -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2/md80a").join(format!("{name}.ee{}", ".bin"));
    if !path.exists() {
        skip(&format!(
            "{} is not there: MD80a's (eeMemory.bin of {name}.p2s; backups in kept-context/md80a)",
            path.display()
        ));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, GP);
    assert_eq!(ee.u32_at(plane + 4), BIPLANE_VTABLE, "no biplane at {plane:#x} in {}", path.display());
    Some(ee)
}

fn plane() -> Option<Ee> {
    let ee = image("biplane", PLANE)?;
    // the parts are wired the way # MD80b reads them
    let (e, a) = (ee.u32_at(PLANE + at::ENGINE), ee.u32_at(PLANE + at::AERO));
    assert_eq!(ee.u32_at(e + 0x24), PLANE + at::THROTTLE, "the engine is not wired to the plane");
    assert_eq!(ee.u32_at(a + 0xbc), PLANE + at::SPEED, "the aero model is not wired to the plane");
    Some(ee)
}

/// A deterministic generator for the cases (tests/biplane.rs's).
struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    fn one_in(&mut self, n: u64) -> bool {
        self.next() % n == 0
    }
    fn around(&mut self, x: f32) -> f32 {
        match self.next() % 16 {
            0 => 0.0,
            1 => -x,
            2 => x,
            _ => x * self.range(0.5, 1.5),
        }
    }
    fn stick(&mut self, lo: f32) -> f32 {
        match self.next() % 6 {
            0 => 0.0,
            1 => 1.0,
            2 => lo,
            _ => self.range(lo, 1.0),
        }
    }
}

fn angle(pick: &mut Pick) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    match pick.next() % 10 {
        0 => 0.0,
        1 => PI,
        2 => -PI,
        3 => FRAC_PI_2,
        4 => -FRAC_PI_2,
        _ => pick.range(-PI, PI),
    }
}

fn floats(ee: &Ee, at: u32, n: usize) -> Vec<F> {
    (0..n).map(|k| F(ee.f32_at(at + 4 * k as u32))).collect()
}

fn vector(ee: &Ee, at: u32) -> [f32; 4] {
    [0, 4, 8, 12].map(|k| ee.f32_at(at + k))
}

fn same(what: &str, case: usize, got: F, want: f32) {
    assert_eq!(got.to_bits(), want.to_bits(), "case {case}: {what} is {want:e} in the game, {:e} in the port", got.to_f32());
}

fn same_vector(what: &str, case: usize, got: Vf, want: [f32; 4]) {
    for k in 0..4 {
        assert_eq!(got[k].to_bits(), want[k].to_bits(),
                   "case {case}: {what}[{k}] is {:e} in the game, {:e} in the port ({want:?} / {got:?})", want[k], got[k]);
    }
}

fn pad_of(ee: &Ee) -> Pad {
    let p = floats(ee, PLANE + at::STICKS, 3);
    Pad {
        forward: F(ee.f32_at(PLANE + at::FORWARD)),
        back: F(ee.f32_at(PLANE + at::BACK)),
        elevator: p[0],
        aileron: p[1],
        rudder: p[2],
    }
}

fn surfaces_of(ee: &Ee) -> Surfaces {
    let s = floats(ee, PLANE + at::SURFACES, 5);
    Surfaces {
        throttle: F(ee.f32_at(PLANE + at::THROTTLE)),
        brake: F(ee.f32_at(PLANE + at::BRAKE)),
        elevator: s[0],
        elevator_command: s[1],
        aileron: s[2],
        rudder: s[3],
        steer: s[4],
        pitch_integral: F(ee.f32_at(PLANE + at::INTEGRAL)),
    }
}

fn joystick_of(ee: &Ee) -> Joystick {
    let j = floats(ee, PLANE + at::JOYSTICK, 12);
    Joystick {
        throttle_increment: j[0],
        reverse_throttle: j[1],
        aileron_rate: j[2],
        elevator_rate: j[3],
        rudder_rate: j[4],
        max_elevator: j[5],
        max_aileron: j[6],
        max_rudder: j[7],
        // +0x1138 is not the controls'
        ramp_off_roll: j[9],
        off_roll: j[10],
        min_elevator: j[11],
    }
}

fn autopilot_of(ee: &Ee) -> Autopilot {
    let a = floats(ee, PLANE + at::AUTOPILOT, 6);
    Autopilot { aileron_dead_zone: a[0], max_pitch_rate: a[1], max_roll: a[2], feed_forward: a[3], gain: a[4], integrator_gain: a[5] }
}

fn attitude_of(ee: &Ee) -> Attitude {
    let body = ee.u32_at(PLANE + at::BODY);
    Attitude {
        flown: ee.u32_at(PLANE + at::FLOWN) != 0,
        grounded: ee.u32_at(PLANE + at::GROUNDED) != 0,
        nose_y: F(ee.f32_at(PLANE + at::NOSE_Y)),
        roll: F(ee.f32_at(PLANE + at::ROLL)),
        speed: F(ee.f32_at(PLANE + at::SPEED)),
        pitch_rate: F(ee.f32_at(body + BODY_RATES)),
    }
}

fn check_surfaces(case: usize, got: &Surfaces, ee: &Ee) {
    let want = surfaces_of(ee);
    for (name, got, want) in [
        ("the throttle", got.throttle, want.throttle),
        ("the brake", got.brake, want.brake),
        ("the elevator", got.elevator, want.elevator),
        ("the elevator command", got.elevator_command, want.elevator_command),
        ("the aileron", got.aileron, want.aileron),
        ("the rudder", got.rudder, want.rudder),
        ("the tail wheel's steer", got.steer, want.steer),
        ("the pitch integral", got.pitch_integral, want.pitch_integral),
    ] {
        same(name, case, got, want.to_f32());
    }
}

// ---------------------------------------------------------------------------------------------------
// 1 — the controls
// ---------------------------------------------------------------------------------------------------

#[test]
fn the_mxvatv_controls_match_the_original() {
    let Some(mut ee) = plane() else { return };
    let shipped: Vec<f32> = (0..12).map(|k| ee.f32_at(PLANE + at::JOYSTICK + 4 * k)).collect();
    let mut pick = Pick(0x9e37_79b9_7f4a_7c15);
    for case in 0..CASES {
        ee.set_u32(PLANE + at::FLOWN, u32::from(!pick.one_in(20)));
        ee.set_u32(PLANE + at::GROUNDED, u32::from(pick.one_in(2)));
        let (forward, back) = (pick.stick(0.0), pick.stick(0.0));
        ee.set_f32(PLANE + at::FORWARD, forward);
        ee.set_f32(PLANE + at::BACK, back);
        for k in 0..3 {
            let v = pick.stick(-1.0);
            ee.set_f32(PLANE + at::STICKS + 4 * k, v);
        }
        // a signed zero now and then, where the fused throttle step and a plain one differ
        let throttle = if pick.one_in(16) { -0.0 } else { pick.range(-0.5, 1.2) };
        ee.set_f32(PLANE + at::THROTTLE, throttle);
        let brake = pick.range(-1.0, 1.0);
        ee.set_f32(PLANE + at::BRAKE, brake);
        for k in 0..5 {
            let v = pick.range(-1.0, 1.0);
            ee.set_f32(PLANE + at::SURFACES + 4 * k, v);
        }
        for (k, &x) in shipped.iter().enumerate() {
            let v = pick.around(x);
            ee.set_f32(PLANE + at::JOYSTICK + 4 * k as u32, v);
        }
        if pick.one_in(10) {
            // the fade's degenerate range
            let r = ee.f32_at(PLANE + at::JOYSTICK + 4 * 9);
            ee.set_f32(PLANE + at::JOYSTICK + 4 * 10, r);
        }
        let nose = if pick.one_in(3) { pick.range(-1.0, 1.0) } else { pick.range(-0.8, 0.8) };
        ee.set_f32(PLANE + at::NOSE_Y, nose);
        let roll = angle(&mut pick);
        ee.set_f32(PLANE + at::ROLL, roll);
        let dt = match pick.next() % 8 {
            0 => 0.0,
            1..=4 => 1.0 / 30.0,
            _ => pick.range(0.0, 0.1),
        };

        let (mut pad, mut s, j, a) = (pad_of(&ee), surfaces_of(&ee), joystick_of(&ee), attitude_of(&ee));
        ee.call(f("atv_biplane_controls"), &[u64::from(PLANE)], &[dt], BUDGET);
        controls_mxvatv(&mut pad, &mut s, &j, &a, F(dt));

        let want = pad_of(&ee);
        for (name, got, want) in [
            ("forward", pad.forward, want.forward),
            ("back", pad.back, want.back),
            ("the elevator stick", pad.elevator, want.elevator),
            ("the aileron stick", pad.aileron, want.aileron),
            ("the rudder buttons", pad.rudder, want.rudder),
        ] {
            same(name, case, got, want.to_f32());
        }
        check_surfaces(case, &s, &ee);
    }
    println!("the controls: {CASES} cases, bit-exact");
}

// ---------------------------------------------------------------------------------------------------
// 2 — the autopilot
// ---------------------------------------------------------------------------------------------------

#[test]
fn the_mxvatv_autopilot_matches_the_original() {
    let Some(mut ee) = plane() else { return };
    let base = ee.u32_at(f("atv_curve_table_ptr"));
    let entries: Vec<[f32; 2]> =
        (0..CURVE_ENTRIES as u32).map(|k| [ee.f32_at(base + 8 * k), ee.f32_at(base + 8 * k + 4)]).collect();
    let table = CurveTable::from_entries(&entries);
    let body = ee.u32_at(PLANE + at::BODY);
    let shipped: Vec<f32> = (0..6).map(|k| ee.f32_at(PLANE + at::AUTOPILOT + 4 * k)).collect();
    let max_elevator = ee.f32_at(PLANE + at::JOYSTICK + 4 * 5);
    let mut pick = Pick(0x2545_f491_4f6c_dd1d);
    for case in 0..CASES {
        let aileron = if pick.one_in(2) { pick.range(-0.1, 0.1) } else { pick.stick(-1.0) };
        ee.set_f32(PLANE + at::STICKS + 4, aileron);
        let elevator = pick.range(-1.0, 1.0);
        ee.set_f32(PLANE + at::SURFACES, elevator);
        let command = pick.range(-1.0, 1.0);
        ee.set_f32(PLANE + at::SURFACES + 4, command);
        let max = pick.around(max_elevator).abs();
        ee.set_f32(PLANE + at::JOYSTICK + 4 * 5, max);
        for (k, &x) in shipped.iter().enumerate() {
            let v = if k == 2 { pick.range(0.0, 1.6) } else { pick.around(x) };
            ee.set_f32(PLANE + at::AUTOPILOT + 4 * k as u32, v);
        }
        let integral = pick.range(-0.6, 0.6);
        ee.set_f32(PLANE + at::INTEGRAL, integral);
        let nose = pick.range(-1.0, 1.0);
        ee.set_f32(PLANE + at::NOSE_Y, nose);
        let roll = angle(&mut pick);
        ee.set_f32(PLANE + at::ROLL, roll);
        let speed = match pick.next() % 8 {
            0 => pick.range(-5.0, 5.0),
            _ => pick.range(20.0, 250.0),
        };
        ee.set_f32(PLANE + at::SPEED, speed);
        let rate = pick.range(-2.0, 2.0);
        ee.set_f32(body + BODY_RATES, rate);
        let dt = if pick.one_in(2) { 1.0 / 30.0 / 4.0 } else { pick.range(0.0, 0.05) };

        let (pad, mut s, j, ap, a) = (pad_of(&ee), surfaces_of(&ee), joystick_of(&ee), autopilot_of(&ee), attitude_of(&ee));
        ee.call(f("atv_biplane_autopilot"), &[u64::from(PLANE)], &[dt], BUDGET);
        autopilot_mxvatv(&pad, &mut s, &j, &ap, &a, &table, F(dt));
        check_surfaces(case, &s, &ee);
    }
    println!("the autopilot: {CASES} cases, bit-exact");
}

// ---------------------------------------------------------------------------------------------------
// 3 — the engine
// ---------------------------------------------------------------------------------------------------

/// tests/biplane.rs's stand-in for the smooth noise.
fn noise(t: f32) -> f32 {
    let h = t.to_bits().wrapping_mul(0x9e37_79b9) >> 8;
    h as f32 / (1u32 << 23) as f32 - 1.0
}

/// Hooks the body's force (and, with `moment`, moment) entries to record each vector they are handed.
fn record_body(ee: &mut Ee, body: u32, moment: bool) -> Rc<RefCell<Vec<[f32; 4]>>> {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let vtable = ee.u32_at(body + BODY_VTABLE);
    let slots: &[u32] = if moment { &[FORCE_SLOT, MOMENT_SLOT] } else { &[FORCE_SLOT] };
    for &slot in slots {
        let s = seen.clone();
        ee.hooks.insert(
            ee.u32_at(vtable + slot),
            Box::new(move |ee| {
                let v = ee.r(5) as u32;
                s.borrow_mut().push(vector(ee, v));
            }),
        );
    }
    seen
}

#[test]
fn the_mxvatv_engine_matches_the_original() {
    let Some(mut ee) = plane() else { return };
    let e = ee.u32_at(PLANE + at::ENGINE);
    let body = ee.u32_at(e + 0x4c);
    assert_eq!(ee.u32_at(e + 0x2c), PLANE + at::NOSE_Y, "the engine's nose pointer");
    ee.hooks.insert(f("atv_smooth_noise"), Box::new(|ee| {
        let t = ee.f(12);
        ee.set_f(0, noise(t));
    }));
    let seen = record_body(&mut ee, body, false);
    // +0x30 MinPropEfficiency .. +0x68 the clock, MX Unleashed's fifteen words 0xc lower
    let shipped: Vec<f32> = (0..15).map(|k| ee.f32_at(e + 0x30 + 4 * k)).collect();
    let mut pick = Pick(0x6a09_e667_f3bc_c908);
    for case in 0..CASES {
        for (k, &x) in shipped.iter().enumerate() {
            let v = match k {
                7 => continue,                    // the body pointer
                3 => angle(&mut pick),            // the prop's angle
                2 => pick.range(0.0, 3000.0),     // its rpm
                0xd => pick.range(-0.5, 2500.0),  // the note
                0xe => pick.range(0.0, 100.0),    // the clock
                _ => pick.around(x),
            };
            ee.set_f32(e + 0x30 + 4 * k as u32, v);
        }
        if pick.one_in(16) {
            // a signed zero turning backwards: the fused advance and a plain one differ
            ee.set_f32(e + 0x3c, -0.0);
            ee.set_f32(e + 0x38, -pick.range(0.0, 3000.0));
        }
        if pick.one_in(10) {
            let lo = ee.f32_at(e + 0x58);
            ee.set_f32(e + 0x5c, lo);
        }
        let throttle = pick.stick(-0.2);
        ee.set_f32(PLANE + at::THROTTLE, throttle);
        let nose = pick.range(-1.0, 1.0);
        ee.set_f32(PLANE + at::NOSE_Y, nose);
        let vel = [0; 3].map(|_| if pick.one_in(8) { 0.0 } else { pick.range(-150.0, 150.0) });
        for (k, v) in vel.iter().enumerate() {
            ee.set_f32(body + BODY_VEL + 4 * k as u32, *v);
        }
        let altitude = pick.range(-100.0, 900.0);
        ee.set_f32(body + BODY_POS + 4, altitude);
        let dt = match pick.next() % 8 {
            0 => 0.0,
            1..=4 => 1.0 / 30.0 / 4.0,
            _ => pick.range(0.0, 0.05),
        };

        let fs = floats(&ee, e + 0x30, 15);
        let mut port = Engine {
            min_efficiency: fs[0],
            full_efficiency_speed: fs[1],
            prop_rpm: fs[2],
            prop_angle: fs[3],
            max_power: fs[4],
            fade_altitude: fs[5],
            max_altitude: fs[6],
            power: fs[8],
            rpm: fs[9],
            idle_rpm: fs[10],
            max_rpm: fs[11],
            thrust: fs[12],
            note: fs[13],
            clock: fs[14],
        };
        seen.borrow_mut().clear();
        ee.call(f("atv_biplane_engine_step"), &[u64::from(e)], &[dt], BUDGET);
        let force = port.step_mxvatv(F(throttle), Vf::new(vel[0], vel[1], vel[2], 1.0), F(altitude), F(nose), F(dt),
                                     &mut |t: F| F(noise(t.to_f32())));

        let want = floats(&ee, e + 0x30, 15);
        for (name, got, want) in [
            ("the prop's angle", port.prop_angle, want[3]),
            ("the power", port.power, want[8]),
            ("the rpm", port.rpm, want[9]),
            ("the thrust", port.thrust, want[12]),
            ("the note", port.note, want[13]),
            ("the clock", port.clock, want[14]),
        ] {
            same(name, case, got, want.to_f32());
        }
        let forces = seen.borrow();
        assert_eq!(forces.len(), 1, "case {case}: the engine pushed the body {} times", forces.len());
        same_vector("the thrust", case, force, forces[0]);
    }
    println!("the engine: {CASES} cases, bit-exact");
}

// ---------------------------------------------------------------------------------------------------
// 4 — the aerodynamics
// ---------------------------------------------------------------------------------------------------

#[test]
fn the_mxvatv_aerodynamics_match_the_original() {
    let Some(mut ee) = plane() else { return };
    let a = ee.u32_at(PLANE + at::AERO);
    let body = ee.u32_at(a + 0xd8);
    // the six inputs' pointers: alpha, beta, the three surfaces, the body's rates
    for (off, want) in [
        (0xc0, PLANE + at::ALPHA),
        (0xc4, PLANE + at::BETA),
        (0xc8, PLANE + at::SURFACES + 4),
        (0xcc, PLANE + at::SURFACES + 8),
        (0xd0, PLANE + at::SURFACES + 12),
        (0xd4, body + BODY_RATES),
    ] {
        assert_eq!(ee.u32_at(a + off), want, "the aero model's pointer at +{off:#x}");
    }
    let seen = record_body(&mut ee, body, true);
    // +0x28 density to +0xb8 Cnda, 37 words; +0x38 (the aspect ratio) is not read
    let shipped: Vec<f32> = (0..37).map(|k| ee.f32_at(a + 0x28 + 4 * k)).collect();
    let mut pick = Pick(0xbb67_ae85_84ca_a73b);
    for case in 0..CASES {
        ee.set_u32(a + 0x24, u32::from(pick.one_in(2)));
        for (k, &x) in shipped.iter().enumerate() {
            let v = match k {
                9 => pick.range(1.0, 10.0), // CLDropOff, a power
                _ => pick.around(x),
            };
            ee.set_f32(a + 0x28 + 4 * k as u32, v);
        }
        if pick.one_in(3) {
            ee.set_f32(a + 0x44, pick.range(-1.0, 6.0));
            ee.set_f32(a + 0x3c, pick.range(-0.1, 0.1));
        }
        let alpha = if pick.one_in(4) { pick.range(-3.0, 3.0) } else { pick.range(-0.5, 0.5) };
        let beta = pick.range(-0.6, 0.6);
        let speed = match pick.next() % 8 {
            0 => pick.range(-2.0, 2.0),
            1 => pick.range(-60.0, 0.0),
            _ => pick.range(0.0, 250.0),
        };
        let surfaces = [0; 3].map(|_| pick.range(-0.6, 0.6));
        let rates = [0; 3].map(|_| pick.range(-3.0, 3.0));
        ee.set_f32(PLANE + at::ALPHA, alpha);
        ee.set_f32(PLANE + at::BETA, beta);
        ee.set_f32(PLANE + at::SPEED, speed);
        for (k, v) in surfaces.iter().enumerate() {
            ee.set_f32(PLANE + at::SURFACES + 4 + 4 * k as u32, *v);
        }
        for (k, v) in rates.iter().enumerate() {
            ee.set_f32(body + BODY_RATES + 4 * k as u32, *v);
        }

        let c = floats(&ee, a + 0x28, 37);
        let mut port = Aero {
            true_speed: ee.u32_at(a + 0x24) != 0,
            density: c[0],
            chord: c[1],
            span: c[2],
            area: c[3],
            default_alpha: c[5],
            cm_break_alpha: c[6],
            lift_alpha: c[7],
            lift_round_off: c[8],
            lift_drop_off: c[9],
            lift_scale: c[10],
            lift_angle_scale: c[11],
            lift_q: c[12],
            lift_de: c[13],
            drag0: c[14],
            drag1: c[15],
            drag_de: c[16],
            side_beta: c[17],
            side_da: c[18],
            side_dr: c[19],
            side_p: c[20],
            side_r: c[21],
            cl_beta: c[22],
            cl_p: c[23],
            cl_r: c[24],
            cl_da: c[25],
            cl_dr: c[26],
            cm0: c[27],
            cm_alpha: c[28],
            cm_q: c[29],
            cm_de: c[30],
            cm_stall: c[31],
            cn_beta: c[32],
            cn_p: c[33],
            cn_r: c[34],
            cn_dr: c[35],
            cn_da: c[36],
            ..Aero::default()
        };
        let air = Air {
            alpha: F(alpha),
            beta: F(beta),
            speed: F(speed),
            elevator: F(surfaces[0]),
            aileron: F(surfaces[1]),
            rudder: F(surfaces[2]),
            rates: Vf::new(rates[0], rates[1], rates[2], 1.0),
        };
        seen.borrow_mut().clear();
        ee.call(f("atv_biplane_aero_step"), &[u64::from(a)], &[], BUDGET);
        let (force, moment) = port.forces_mxvatv(&air);

        let want = floats(&ee, a + 0xdc, 5);
        for (name, got, want) in [
            ("CL", port.lift, want[0]),
            ("CD", port.drag, want[1]),
            ("CY", port.side, want[2]),
            ("Cl", port.roll, want[3]),
            ("CM", port.pitch, want[4]),
        ] {
            same(name, case, got, want.to_f32());
        }
        let pushed = seen.borrow();
        assert_eq!(pushed.len(), 2, "case {case}: the aero model pushed the body {} times", pushed.len());
        same_vector("the force", case, force, pushed[0]);
        same_vector("the moment", case, moment, pushed[1]);
    }
    println!("the aerodynamics: {CASES} cases, bit-exact");
}

// ---------------------------------------------------------------------------------------------------
// 5 — MXVehicle's wreck check
// ---------------------------------------------------------------------------------------------------

/// The terrain's edge test on the image's own terrain: whether a point is within the margin of an edge,
/// and the terrain's extents, which it hands back only when it says yes.
#[test]
fn the_mxvatv_edge_test_matches_the_original() {
    let Some(mut ee) = plane() else { return };
    let terrain = ee.u32_at(PLANE + at::TERRAIN);
    let (x_out, z_out, point) = (0x01f8_0000u32, 0x01f8_0010u32, 0x01f8_0020u32);
    // the extents, from a point that is out on every side
    ee.set_f32(point, -1.0);
    ee.set_f32(point + 8, -1.0);
    ee.set_u32(point + 12, 0x3f80_0000);
    ee.call(f("atv_terrain_edge"), &[u64::from(terrain), u64::from(point), u64::from(x_out), u64::from(z_out), 0], &[150.0], BUDGET);
    assert_eq!(ee.r(2), 1, "a point at (-1, -1) is not by the edge");
    let area = Area { width: F(ee.f32_at(x_out)), depth: F(ee.f32_at(z_out)) };
    assert!(area.width > F(300.0) && area.depth > F(300.0), "the image's terrain is {area:?}");
    let mut pick = Pick(0x3c6e_f372_fe94_f82b);
    let mut outs = 0;
    for case in 0..CASES {
        let margin = if pick.one_in(2) { 150.0 } else { pick.range(0.0, 400.0) };
        let side = |pick: &mut Pick, extent: f32| match pick.next() % 6 {
            0 => margin,
            1 => extent - margin,
            2 => pick.range(-500.0, margin),
            3 => pick.range(extent - margin, extent + 500.0),
            _ => pick.range(-500.0, extent + 500.0),
        };
        let (x, z) = (side(&mut pick, area.width.to_f32()), side(&mut pick, area.depth.to_f32()));
        let y = pick.range(-100.0, 900.0);
        for (k, v) in [x, y, z, 1.0].into_iter().enumerate() {
            ee.set_f32(point + 4 * k as u32, v);
        }
        ee.call(f("atv_terrain_edge"), &[u64::from(terrain), u64::from(point), u64::from(x_out), u64::from(z_out), 0], &[margin], BUDGET);
        let got = area.outside_mxvatv(Vf::new(x, y, z, 1.0), F(margin));
        assert_eq!(ee.r(2) != 0, got, "case {case}: ({x}, {z}) with margin {margin} in {area:?}");
        outs += usize::from(got);
    }
    println!("the edge test: {CASES} cases ({outs} by the edge) on a {:?} terrain", area);
}

/// The throw back from the edge: the wreck check with the edge test hooked to say yes and hand back
/// generated extents, and `rand` hooked to hand out the port's generator's numbers.
#[test]
fn the_mxvatv_throw_matches_the_original() {
    let Some(mut ee) = plane() else { return };
    let body = ee.u32_at(PLANE + at::BODY);
    let gravity = ee.u32_at(PLANE + at::GRAVITY) + 4;
    let extents = Rc::new(RefCell::new((0.0f32, 0.0f32)));
    let x = extents.clone();
    ee.hooks.insert(f("atv_terrain_edge"), Box::new(move |ee| {
        let (w, d) = *x.borrow();
        let (a2, a3) = (ee.r(6) as u32, ee.r(7) as u32);
        ee.set_f32(a2, w);
        ee.set_f32(a3, d);
        ee.set(2, 1);
    }));
    let rng = Rc::new(RefCell::new(Rng::from_seed(0x0d80_b0b0)));
    let r = rng.clone();
    ee.hooks.insert(f("atv_rand"), Box::new(move |ee| {
        let v = r.borrow_mut().rand();
        ee.set(2, v as u32 as u64);
    }));
    let mut pick = Pick(0xa54f_f53a_5f1d_36f1);
    for case in 0..CASES {
        let (w, d) = match pick.next() % 6 {
            0 => (0.0, 0.0),
            1 => (pick.range(0.0, 400.0), pick.range(0.0, 400.0)),
            _ => (pick.range(300.0, 9000.0), pick.range(300.0, 9000.0)),
        };
        *extents.borrow_mut() = (w, d);
        let mut position = [0; 3].map(|_| pick.range(-500.0, 9500.0));
        if pick.one_in(8) {
            position = [0.5 * w, position[1], 0.5 * d];
        }
        let w_lane = if pick.one_in(2) { 1.0 } else { pick.range(-2.0, 2.0) };
        let position = Vf::new(position[0], position[1], position[2], w_lane);
        let velocity = [0; 4].map(|_| pick.range(-200.0, 200.0));
        let rates = [0; 4].map(|_| pick.range(-3.0, 3.0));
        let scale = pick.range(0.0, 3.0);
        for k in 0..4 {
            ee.set_f32(body + BODY_POS + 4 * k, position[k as usize]);
            ee.set_f32(body + BODY_VEL + 4 * k, velocity[k as usize]);
            ee.set_f32(body + BODY_RATES + 4 * k, rates[k as usize]);
        }
        ee.set_f32(gravity, scale);
        ee.set_u32(PLANE + at::GATE, 1);

        let mut port_rng = rng.borrow().clone();
        ee.call(f("atv_vehicle_wreck_check"), &[u64::from(PLANE)], &[], BUDGET);
        assert_eq!(ee.r(2), 1, "case {case}: the wreck check did not throw");
        let mut v = Vf::new(velocity[0], velocity[1], velocity[2], velocity[3]);
        let mut spin = Vf::new(rates[0], rates[1], rates[2], rates[3]);
        let mut g = F(scale);
        let area = Area { width: F(w), depth: F(d) };
        throw_mxvatv(&area, position, &mut v, &mut spin, &mut g, &mut port_rng);
        same_vector("the velocity", case, v, vector(&ee, body + BODY_VEL));
        same_vector("the spin", case, spin, vector(&ee, body + BODY_RATES));
        same("the gravity scale", case, g, ee.f32_at(gravity));
        assert_eq!(port_rng.state(), rng.borrow().state(), "case {case}: the port drew a different count of numbers");
    }
    // and the gate: a vehicle whose +0x308 is clear is never thrown
    ee.set_u32(PLANE + at::GATE, 0);
    ee.call(f("atv_vehicle_wreck_check"), &[u64::from(PLANE)], &[], BUDGET);
    assert_eq!(ee.r(2), 0, "the wreck check ran with its gate clear");
    println!("the throw: {CASES} cases, bit-exact");
}

// ---------------------------------------------------------------------------------------------------
// 6 — the reader
// ---------------------------------------------------------------------------------------------------

fn phy() -> Option<Vec<u8>> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2/md80b").join("biplane.phy");
    match std::fs::read(&path) {
        Ok(b) => Some(b),
        Err(_) => {
            skip(&format!("{} is not there: the disc's Veh\\biplane.phy (as mods/machines/mxvatv_fliers/Veh/ has it)", path.display()));
            None
        }
    }
}

fn same_word(what: &str, got: F, ee: &Ee, at: u32) {
    let want = ee.u32_at(at);
    assert_eq!(got.to_bits(), want, "{what} is {:e} in the game, {:e} in the port", f32::from_bits(want), got.to_f32());
}

fn same_quad(what: &str, got: Vf, ee: &Ee, at: u32) {
    for k in 0..4 {
        same_word(&format!("{what}[{k}]"), F(got[k]), ee, at + 4 * k as u32);
    }
}

#[test]
fn the_mxvatv_biplane_file_reads_as_the_original_loads_it() {
    let Some(bytes) = phy() else { return };
    // the garage's skins: biplane (form 1) model one, biplane3 (form 3) model two
    assert!(mxvatv_model_one(1) && mxvatv_model_one(2) && mxvatv_model_one(6) && mxvatv_model_one(7));
    assert!((3..=5).chain(8..=10).all(|f| !mxvatv_model_one(f)));
    for (name, plane, model_one) in [("biplane", PLANE, true), ("biplane3", PLANE_TWO, false)] {
        let Some(ee) = image(name, plane) else { return };
        assert_eq!(ee.u32_at(plane + at::MODEL), u32::from(model_one), "{name}'s model word");
        let p = Plane::read_mxvatv_bytes(&bytes, model_one);
        let a = ee.u32_at(plane + at::AERO);
        let aero = &p.plane.aero;
        same_word("Chord", aero.chord, &ee, a + 0x2c);
        same_word("Span", aero.span, &ee, a + 0x30);
        same_word("SurfaceArea", aero.surface_area, &ee, a + 0x34);
        same_word("AspectRatio", aero.aspect_ratio, &ee, a + 0x38);
        same_word("DefaultAngleOfAttack", aero.default_angle_of_attack, &ee, a + 0x3c);
        let s = &p.plane.surfaces;
        same_word("MaxElevator", s.max_elevator, &ee, plane + at::JOYSTICK + 4 * 5);
        same_word("MaxAileron", s.max_aileron, &ee, plane + at::JOYSTICK + 4 * 6);
        same_word("MaxRudder", s.max_rudder, &ee, plane + at::JOYSTICK + 4 * 7);
        let w = &p.plane.wrecks;
        same_word("MaxWreckTime", w.wreck_time, &ee, plane + 0xc88);
        same_word("WreckGravity", w.wreck_gravity, &ee, plane + 0xc90);
        same_quad("WreckDamping", w.wreck_damping, &ee, plane + 0xca0);
        same_quad("WreckInertia", w.wreck_inertia, &ee, plane + 0xcb0);
        same_quad("PelvisBoxPosition", p.plane.pelvis_box.position, &ee, plane + 0x1330);
        same_quad("PelvisBoxSize", p.plane.pelvis_box.size, &ee, plane + 0x1340);
        same_quad("Left1stPerShoulderOffset", p.left_shoulder_offset, &ee, plane + 0x980);
        same_quad("Right1stPerShoulderOffset", p.right_shoulder_offset, &ee, plane + 0x990);
        same_quad("FPCameraOffset", p.fp_camera_offset, &ee, plane + 0xda0);
        // the flight built off it flies SLUS-21104's steps with the model's surfaces
        let flight = Flight::new_mxvatv(&p);
        assert!(flight.mxvatv && !Flight::new(&p.plane).mxvatv, "the MX vs. ATV switch");
        same_word("the flight's MaxElevator", flight.joystick.max_elevator, &ee, plane + at::JOYSTICK + 4 * 5);
        println!("{name}: model {}, read as loaded", if model_one { "one" } else { "two" });
    }
}
