//! S34 — the biplane's own flight against the original.
//!
//! Four oracle proofs, one per function of `biplane.rs`, each over ten thousand generated cases and
//! each run on the live plane in the save state beside C7's recording: the state's RAM is the
//! interpreter's, the case's inputs are written into the plane's own fields (and the engine's, the
//! aero model's and the body's, through the pointers the plane holds), the original runs, and every
//! field it writes is compared bit for bit with the port's. Running on the live plane is what makes
//! the pointers honest: the engine reads the throttle through its own pointer to it, the aero model
//! its six inputs through six, and none of that wiring is typed in here.
//!
//! 1. **the controls**, with the gate, the buttons, the stick, the attitude and the tuning all
//!    generated, the fade's degenerate range included;
//! 2. **the autopilot**, both sides of the dead zone, every bank from level through inverted, and the
//!    game's own curve table taken from the same image;
//! 3. **the engine**, with the engine's shared smooth noise hooked to a function the port is handed
//!    too, and the body's force entry hooked to record the thrust it is given;
//! 4. **the aerodynamics**, the whole `[Aerodynamics]` section generated around the shipped values,
//!    the force and the moment recorded at the body's two entries.
//!
//! What is not proved here, and why, is in the journal entry's last section.

use game_sim::biplane::{
    Aero, Air, Attitude, Autopilot, Body, Engine, Joystick, Mount, Pad, Surfaces, Tire, autopilot, collision_force, controls,
};
use game_sim::collide::{Bundle, Record};
use game_sim::world::World;
use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::cell::RefCell;
use std::rc::Rc;

/// The save state beside C7's recording, and the plane in it (MXBiplane, the player's).
const STATE: &str = "biplane.p2m2_SaveState.eeMemory.bin";
const PLANE: u32 = 0x00c4_4a40;
const CASES: usize = 10_000;
const BUDGET: u64 = 200_000;

/// The plane's fields, by offset.
mod at {
    pub const FLOWN: u32 = 0x334;
    pub const PAD: u32 = 0x338; // five floats
    pub const SURFACES: u32 = 0x34c; // throttle, brake, elevator, command, aileron, rudder, steer
    pub const JOYSTICK: u32 = 0x368; // eleven floats to MinElevator
    pub const AUTOPILOT: u32 = 0x398; // six floats
    pub const INTEGRAL: u32 = 0x584;
    pub const GROUNDED: u32 = 0x590;
    pub const SPEED: u32 = 0x5c8;
    pub const NOSE_Y: u32 = 0x5d4;
    pub const ROLL: u32 = 0x5f8;
    pub const ALPHA: u32 = 0x59c;
    pub const BETA: u32 = 0x5a0;
    pub const BODY: u32 = 0x160;
    pub const ENGINE: u32 = 0x400;
    pub const AERO: u32 = 0x404;
}
/// The rigid body's fields.
const BODY_POS: u32 = 0x60;
const BODY_VEL: u32 = 0x70;
const BODY_VTABLE: u32 = 0x100;
const BODY_RATES: u32 = 0x1b0;
const FORCE_SLOT: u32 = 0x58;
const MOMENT_SLOT: u32 = 0x5c;

/// The interpreter on the state's RAM, with `$gp` as the boot code leaves it, or why not.
fn plane() -> Option<Ee> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: C7's save state, `./kit state ram` of biplane.p2m2_SaveState.p2s", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    // The plane is where this file says, and wired the way R8 read it: the engine's throttle pointer
    // and the aero model's speed pointer both land in this object.
    let engine = ee.u32_at(PLANE + at::ENGINE);
    let aero = ee.u32_at(PLANE + at::AERO);
    assert_eq!(ee.u32_at(engine + 0x30), PLANE + at::SURFACES, "no live biplane at {PLANE:#x} in {}", path.display());
    assert_eq!(ee.u32_at(aero + 0xc8), PLANE + at::SPEED, "the aero model is not wired to the plane");
    Some(ee)
}

/// A deterministic generator for the cases.
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
    /// Mostly `x` scaled by up to half either way, sometimes zero, sometimes negated.
    fn around(&mut self, x: f32) -> f32 {
        match self.next() % 16 {
            0 => 0.0,
            1 => -x,
            2 => x,
            _ => x * self.range(0.5, 1.5),
        }
    }
    /// A stick or button value: often exactly 0 or the ends.
    fn stick(&mut self, lo: f32) -> f32 {
        match self.next() % 6 {
            0 => 0.0,
            1 => 1.0,
            2 => lo,
            _ => self.range(lo, 1.0),
        }
    }
}

fn floats(ee: &Ee, at: u32, n: usize) -> Vec<F> {
    (0..n).map(|k| F(ee.f32_at(at + 4 * k as u32))).collect()
}

fn same(what: &str, case: usize, got: F, want: f32) {
    assert_eq!(got.to_bits(), want.to_bits(), "case {case}: {what} is {want:e} in the game, {:e} in the port", got.to_f32());
}

fn the_curve_table(ee: &Ee) -> CurveTable {
    let base = ee.u32_at(addr("curve_table_ptr").unwrap());
    let entries: Vec<[f32; 2]> =
        (0..CURVE_ENTRIES as u32).map(|k| [ee.f32_at(base + 8 * k), ee.f32_at(base + 8 * k + 4)]).collect();
    CurveTable::from_entries(&entries)
}

fn pad_of(ee: &Ee) -> Pad {
    let p = floats(ee, PLANE + at::PAD, 5);
    Pad { forward: p[0], back: p[1], elevator: p[2], aileron: p[3], rudder: p[4] }
}

fn surfaces_of(ee: &Ee) -> Surfaces {
    let s = floats(ee, PLANE + at::SURFACES, 7);
    Surfaces {
        throttle: s[0],
        brake: s[1],
        elevator: s[2],
        elevator_command: s[3],
        aileron: s[4],
        rudder: s[5],
        steer: s[6],
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
        // +0x388 is not the controls'
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
    let want = floats(ee, PLANE + at::SURFACES, 7);
    same("the throttle", case, got.throttle, want[0].0);
    same("the brake", case, got.brake, want[1].0);
    same("the elevator", case, got.elevator, want[2].0);
    same("the elevator command", case, got.elevator_command, want[3].0);
    same("the aileron", case, got.aileron, want[4].0);
    same("the rudder", case, got.rudder, want[5].0);
    same("the tail wheel's steer", case, got.steer, want[6].0);
    same("the pitch integral", case, got.pitch_integral, ee.f32_at(PLANE + at::INTEGRAL));
}

/// An angle anywhere in `-pi..pi`, with the ends and the quarter turns now and then.
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

// ---------------------------------------------------------------------------------------------------
// 1 — the controls
// ---------------------------------------------------------------------------------------------------

#[test]
fn the_controls_match_the_original() {
    let Some(mut ee) = plane() else { return };
    let f = addr("biplane_controls").unwrap();
    let shipped: Vec<f32> = (0..12).map(|k| ee.f32_at(PLANE + at::JOYSTICK + 4 * k)).collect();
    let mut pick = Pick(0x9e37_79b9_7f4a_7c15);
    for case in 0..CASES {
        ee.set_u32(PLANE + at::FLOWN, u32::from(!pick.one_in(20)));
        ee.set_u32(PLANE + at::GROUNDED, u32::from(pick.one_in(2)));
        for (k, lo) in [0.0, 0.0, -1.0, -1.0, -1.0].into_iter().enumerate() {
            let v = pick.stick(lo);
            ee.set_f32(PLANE + at::PAD + 4 * k as u32, v);
        }
        for k in 0..7 {
            let v = if k == 0 { pick.range(-0.5, 1.2) } else { pick.range(-1.0, 1.0) };
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
        let dt = if pick.one_in(2) { 1.0 / 30.0 } else { pick.range(0.0, 0.1) };

        let (mut pad, mut s, j, a) = (pad_of(&ee), surfaces_of(&ee), joystick_of(&ee), attitude_of(&ee));
        ee.call(f, &[u64::from(PLANE)], &[dt], BUDGET);
        controls(&mut pad, &mut s, &j, &a, F(dt));

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
fn the_autopilot_matches_the_original() {
    let Some(mut ee) = plane() else { return };
    let f = addr("biplane_autopilot").unwrap();
    let table = the_curve_table(&ee);
    let body = ee.u32_at(PLANE + at::BODY);
    let shipped: Vec<f32> = (0..6).map(|k| ee.f32_at(PLANE + at::AUTOPILOT + 4 * k)).collect();
    let max_elevator = ee.f32_at(PLANE + at::JOYSTICK + 4 * 5);
    let mut pick = Pick(0x2545_f491_4f6c_dd1d);
    for case in 0..CASES {
        let aileron = if pick.one_in(2) { pick.range(-0.1, 0.1) } else { pick.stick(-1.0) };
        ee.set_f32(PLANE + at::PAD + 12, aileron);
        let elevator = pick.range(-1.0, 1.0);
        ee.set_f32(PLANE + at::SURFACES + 8, elevator);
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
        ee.call(f, &[u64::from(PLANE)], &[dt], BUDGET);
        autopilot(&pad, &mut s, &j, &ap, &a, &table, F(dt));
        check_surfaces(case, &s, &ee);
    }
    println!("the autopilot: {CASES} cases, bit-exact");
}

// ---------------------------------------------------------------------------------------------------
// 3 — the engine
// ---------------------------------------------------------------------------------------------------

/// What the hooked smooth noise answers, on both sides: any function of `t` will do, as long as it is
/// the same one, and this one reaches the whole of `-1..1`.
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
                s.borrow_mut().push([0, 4, 8, 12].map(|k| ee.f32_at(v + k)));
            }),
        );
    }
    seen
}

fn same_vector(what: &str, case: usize, got: Vf, want: [f32; 4]) {
    for k in 0..4 {
        assert_eq!(got[k].to_bits(), want[k].to_bits(),
                   "case {case}: {what}[{k}] is {:e} in the game, {:e} in the port ({want:?} / {got:?})", want[k], got[k]);
    }
}

#[test]
fn the_engine_matches_the_original() {
    let Some(mut ee) = plane() else { return };
    let f = addr("biplane_engine_step").unwrap();
    let e = ee.u32_at(PLANE + at::ENGINE);
    let body = ee.u32_at(e + 0x58);
    ee.hooks.insert(addr("smooth_noise").unwrap(), Box::new(|ee| {
        let t = ee.f(12);
        ee.set_f(0, noise(t));
    }));
    let seen = record_body(&mut ee, body, false);
    let shipped: Vec<f32> = (0..15).map(|k| ee.f32_at(e + 0x3c + 4 * k)).collect();
    let mut pick = Pick(0x6a09_e667_f3bc_c908);
    for case in 0..CASES {
        for (k, &x) in shipped.iter().enumerate() {
            let v = match k {
                7 => continue,                         // the body pointer
                3 => angle(&mut pick),                 // the prop's angle
                2 => pick.range(0.0, 3000.0),          // its rpm
                0xd => pick.range(-0.5, 2500.0),       // the note
                0xe => pick.range(0.0, 100.0),         // the clock
                _ => pick.around(x),
            };
            ee.set_f32(e + 0x3c + 4 * k as u32, v);
        }
        if pick.one_in(10) {
            let lo = ee.f32_at(e + 0x64);
            ee.set_f32(e + 0x68, lo);
        }
        let throttle = pick.stick(-0.2);
        ee.set_f32(PLANE + at::SURFACES, throttle);
        let nose = pick.range(-1.0, 1.0);
        ee.set_f32(PLANE + at::NOSE_Y, nose);
        let vel = [0; 3].map(|_| if pick.one_in(8) { 0.0 } else { pick.range(-150.0, 150.0) });
        for (k, v) in vel.iter().enumerate() {
            ee.set_f32(body + BODY_VEL + 4 * k as u32, *v);
        }
        let altitude = pick.range(-100.0, 900.0);
        ee.set_f32(body + BODY_POS + 4, altitude);
        let dt = if pick.one_in(2) { 1.0 / 30.0 / 4.0 } else { pick.range(0.0, 0.05) };

        let fs = floats(&ee, e + 0x3c, 15);
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
        ee.call(f, &[u64::from(e)], &[dt], BUDGET);
        let force = port.step(F(throttle), Vf::new(vel[0], vel[1], vel[2], 1.0), F(altitude), F(nose), F(dt),
                              &mut |t: F| F(noise(t.to_f32())));

        let want = floats(&ee, e + 0x3c, 15);
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
fn the_aerodynamics_match_the_original() {
    let Some(mut ee) = plane() else { return };
    let f = addr("biplane_aero_forces").unwrap();
    let a = ee.u32_at(PLANE + at::AERO);
    let body = ee.u32_at(a + 0xe4);
    let seen = record_body(&mut ee, body, true);
    // +0x34 density to +0xc4 Cnda, 37 words; +0x44 (the aspect ratio) is not read
    let shipped: Vec<f32> = (0..37).map(|k| ee.f32_at(a + 0x34 + 4 * k)).collect();
    let mut pick = Pick(0xbb67_ae85_84ca_a73b);
    for case in 0..CASES {
        ee.set_u32(a + 0x30, u32::from(pick.one_in(2)));
        for (k, &x) in shipped.iter().enumerate() {
            let v = match k {
                9 => pick.range(1.0, 10.0), // CLDropOff (+0x58), a power: keep it one
                _ => pick.around(x),
            };
            ee.set_f32(a + 0x34 + 4 * k as u32, v);
        }
        if pick.one_in(3) {
            // the shipped file's zero slope and default angle, or something there
            ee.set_f32(a + 0x50, pick.range(-1.0, 6.0));
            ee.set_f32(a + 0x48, pick.range(-0.1, 0.1));
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
            ee.set_f32(PLANE + at::SURFACES + 12 + 4 * k as u32, *v);
        }
        for (k, v) in rates.iter().enumerate() {
            ee.set_f32(body + BODY_RATES + 4 * k as u32, *v);
        }

        let c = floats(&ee, a + 0x34, 37);
        let mut port = Aero {
            true_speed: ee.u32_at(a + 0x30) != 0,
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
        ee.call(f, &[u64::from(a)], &[], BUDGET);
        let (force, moment) = port.forces(&air);

        let want = floats(&ee, a + 0xe8, 5);
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

// --- S39: the wheels --------------------------------------------------------------------------------

/// The plane's three tires (front left, front right, tail), its drawn frame, and the fields the
/// wheels read off it.
const TIRES: [u32; 3] = [0x408, 0x40c, 0x410];
const FRAME: u32 = 0x600;
const RATES: u32 = 0x5b0;
const BRAKE: u32 = 0x350;
const STEER: u32 = 0x364;
/// The body's world force accumulator, and its other velocity copy the velocity entry also moves.
const BODY_FORCE: u32 = 0x90;
const BODY_VEL2: u32 = 0xc0;
const BODY_QUAT: u32 = 0x170;
const VELOCITY_SLOT: u32 = 0x3c;
const PUSH_SLOT: u32 = 0x64;
const FORCE_AT_SLOT: u32 = 0x68;

fn vector(ee: &Ee, at: u32) -> Vf {
    Vf::new(ee.f32_at(at), ee.f32_at(at + 4), ee.f32_at(at + 8), ee.f32_at(at + 12))
}

fn put(ee: &mut Ee, at: u32, v: Vf) {
    for k in 0..4 {
        ee.set_f32(at + 4 * k as u32, v[k]);
    }
}

/// A tire as the port sees it, read off the original's object.
fn tire_of(ee: &Ee, t: u32) -> Tire {
    let f = |o: u32| F(ee.f32_at(t + o));
    let v = |o: u32| vector(ee, t + o);
    let b = |o: u32| ee.u32_at(t + o) != 0;
    Tire {
        max_compression: f(0x3c),
        max_decompression_rate: f(0x40),
        spring: f(0x44),
        bounce: f(0x48),
        rebound: f(0x4c),
        center: v(0x60),
        radius: f(0x150),
        inv_radius: f(0x154),
        max_braking: f(0x158),
        side_force: f(0x15c),
        brake_friction: f(0x160),
        steering_moment: f(0x164),
        braked: b(0x30),
        steered: b(0x34),
        offset: v(0x50),
        below: b(0x80),
        held: b(0x84),
        touching: b(0x88),
        compression: f(0x8c),
        last_compression: f(0x90),
        spring_force: f(0x94),
        bottom: v(0xa0),
        low: v(0x100),
        surface: 0,
        ground: v(0xc0),
        normal: v(0xd0),
        contact: v(0xe0),
        hub: v(0xf0),
        contact_velocity: v(0x110),
        angle: f(0x120),
        spin: f(0x124),
        last_normal: v(0x140),
        slip: v(0x170),
        friction: v(0x180),
        frame: [v(0x190), v(0x1a0), v(0x1b0)],
    }
}

/// Every field the three listeners write, the original's against the port's.
fn same_tire(ee: &Ee, case: usize, t: u32, port: &Tire) {
    let flags = [("below", 0x80, port.below), ("held", 0x84, port.held), ("touching", 0x88, port.touching)];
    for (what, o, got) in flags {
        assert_eq!(ee.u32_at(t + o), u32::from(got), "case {case}: {what}");
    }
    for (what, o, got) in [
        ("compression", 0x8c, port.compression),
        ("last compression", 0x90, port.last_compression),
        ("spring force", 0x94, port.spring_force),
        ("angle", 0x120, port.angle),
        ("spin", 0x124, port.spin),
    ] {
        same(what, case, got, ee.f32_at(t + o));
    }
    for (what, o, got) in [
        ("offset", 0x50, port.offset),
        ("bottom", 0xa0, port.bottom),
        ("ground", 0xc0, port.ground),
        ("normal", 0xd0, port.normal),
        ("contact", 0xe0, port.contact),
        ("hub", 0xf0, port.hub),
        ("low", 0x100, port.low),
        ("contact velocity", 0x110, port.contact_velocity),
        ("last normal", 0x140, port.last_normal),
        ("slip", 0x170, port.slip),
        ("friction", 0x180, port.friction),
        ("side row", 0x190, port.frame[0]),
        ("up row", 0x1a0, port.frame[1]),
        ("forward row", 0x1b0, port.frame[2]),
    ] {
        same_vector(what, case, got, vector(ee, t + o).0);
    }
}

/// A sloped plane as the ground: `y = y0 + sx·x + sz·z`, one normal everywhere. The original's ground
/// query is hooked to this, and the port's world is it.
#[derive(Clone, Copy)]
struct Slope {
    y0: f32,
    sx: f32,
    sz: f32,
    normal: Vf,
}

impl Slope {
    fn y(&self, x: f32, z: f32) -> f32 {
        self.y0 + self.sx * x + self.sz * z
    }
}

impl World for Slope {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        (Vf::new(at.x(), self.y(at.x(), at.z()), at.z(), at.w()), self.normal)
    }
    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// Every push the body is given, in order: which entry, and the vectors.
#[derive(Clone, Debug, PartialEq)]
enum Push {
    Velocity(Vf),
    Force(Vf),
    At(Vf, Vf, Vf),
    Moment(Vf),
}

fn bits(p: &Push) -> Vec<u32> {
    let vs = match p {
        Push::Velocity(v) | Push::Force(v) | Push::Moment(v) => vec![*v],
        Push::At(f, p, m) => vec![*f, *p, *m],
    };
    vs.iter().flat_map(|v| v.to_bits()).collect()
}

/// The port's body: the fields the wheels read, and a log of the pushes, with the velocity and force
/// entries doing what the original's do to the fields read back.
struct Logged {
    enabled: bool,
    pos: Vf,
    vel: Vf,
    force: Vf,
    quat: Vf,
    log: Vec<Push>,
}

impl Body for Logged {
    fn enabled(&self) -> bool {
        self.enabled
    }
    fn position(&self) -> Vf {
        self.pos
    }
    fn velocity(&self) -> Vf {
        self.vel
    }
    fn force(&self) -> Vf {
        self.force
    }
    fn orientation(&self) -> Vf {
        self.quat
    }
    fn set_position(&mut self, at: Vf) {
        self.pos = at;
    }
    fn add_velocity(&mut self, v: Vf) {
        self.vel = self.vel.merge(ps2_float::Dest::XYZ, self.vel.add(v));
        self.log.push(Push::Velocity(v));
    }
    fn add_force(&mut self, f: Vf) {
        self.force = self.force.merge(ps2_float::Dest::XYZ, self.force.add(f));
        self.log.push(Push::Force(f));
    }
    fn add_force_at(&mut self, f: Vf, at: Vf, mask: Vf) {
        self.log.push(Push::At(f, at, mask));
    }
    fn add_moment_local(&mut self, m: Vf) {
        self.log.push(Push::Moment(m));
    }
}

/// Hooks the body's velocity, force, force-at-a-point and moment entries: each records what it is
/// handed, and the first two add it into the fields the way the original's do (the listeners they
/// would run are S38's).
fn log_body(ee: &mut Ee, body: u32) -> Rc<RefCell<Vec<Push>>> {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let vt = ee.u32_at(body + BODY_VTABLE);
    let s = seen.clone();
    ee.hooks.insert(ee.u32_at(vt + VELOCITY_SLOT), Box::new(move |ee| {
        let (b, v) = (ee.r(4) as u32, vector(ee, ee.r(5) as u32));
        for o in [BODY_VEL, BODY_VEL2] {
            let w = vector(ee, b + o);
            put(ee, b + o, w.merge(ps2_float::Dest::XYZ, w.add(v)));
        }
        s.borrow_mut().push(Push::Velocity(v));
    }));
    let s = seen.clone();
    ee.hooks.insert(ee.u32_at(vt + PUSH_SLOT), Box::new(move |ee| {
        let (b, v) = (ee.r(4) as u32, vector(ee, ee.r(5) as u32));
        let w = vector(ee, b + BODY_FORCE);
        put(ee, b + BODY_FORCE, w.merge(ps2_float::Dest::XYZ, w.add(v)));
        s.borrow_mut().push(Push::Force(v));
    }));
    let s = seen.clone();
    ee.hooks.insert(ee.u32_at(vt + FORCE_AT_SLOT), Box::new(move |ee| {
        let at = |r: usize| vector(ee, ee.r(r) as u32);
        s.borrow_mut().push(Push::At(at(5), at(6), at(7)));
    }));
    let s = seen.clone();
    ee.hooks.insert(ee.u32_at(vt + MOMENT_SLOT), Box::new(move |ee| {
        s.borrow_mut().push(Push::Moment(vector(ee, ee.r(5) as u32)));
    }));
    seen
}

fn same_pushes(case: usize, port: &[Push], game: &[Push]) {
    let names = |l: &[Push]| l.iter().map(|p| format!("{p:?}")).collect::<Vec<_>>();
    assert_eq!(port.len(), game.len(), "case {case}: pushes\n game {:?}\n port {:?}", names(game), names(port));
    for (k, (p, g)) in port.iter().zip(game).enumerate() {
        assert!(std::mem::discriminant(p) == std::mem::discriminant(g) && bits(p) == bits(g),
                "case {case}: push {k} is {g:?} in the game, {p:?} in the port");
    }
}

/// A rotation from yaw, pitch and roll: the frame's three rows and the quaternion `(x, y, z, w)`.
fn rotation(yaw: f64, pitch: f64, roll: f64) -> ([Vf; 3], Vf) {
    let (cy, sy) = ((yaw / 2.0).cos(), (yaw / 2.0).sin());
    let (cp, sp) = ((pitch / 2.0).cos(), (pitch / 2.0).sin());
    let (cr, sr) = ((roll / 2.0).cos(), (roll / 2.0).sin());
    // yaw about y, then pitch about x, then roll about z
    let mul = |a: [f64; 4], b: [f64; 4]| {
        [
            a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
            a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
            a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
            a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
        ]
    };
    let q = mul(mul([0.0, sy, 0.0, cy], [sp, 0.0, 0.0, cp]), [0.0, 0.0, sr, cr]);
    let [x, y, z, w] = q;
    let r = [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + z * w), 2.0 * (x * z - y * w)],
        [2.0 * (x * y - z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + x * w)],
        [2.0 * (x * z + y * w), 2.0 * (y * z - x * w), 1.0 - 2.0 * (x * x + y * y)],
    ];
    let row = |k: usize| Vf::new(r[k][0] as f32, r[k][1] as f32, r[k][2] as f32, 0.0);
    ([row(0), row(1), row(2)], Vf::new(x as f32, y as f32, z as f32, w as f32))
}

/// A unit-ish vector near `up`, now and then anywhere.
fn normal_near_up(pick: &mut Pick) -> Vf {
    let (x, y, z) = if pick.one_in(8) {
        (pick.range(-1.0, 1.0), pick.range(-1.0, 1.0), pick.range(-1.0, 1.0))
    } else {
        (pick.range(-0.4, 0.4), 1.0, pick.range(-0.4, 0.4))
    };
    let l = (x * x + y * y + z * z).sqrt().max(1e-3);
    Vf::new(x / l, y / l, z / l, if pick.one_in(4) { 0.0 } else { 1.0 })
}

/// One generated case for a tire: the plane's frame and wheel inputs, the tire's tuning and state, the
/// body's fields, all written into the original, and the port's copies of the same.
struct Wheel {
    tire: u32,
    mount: Mount,
    body: Logged,
    world: Slope,
    dt: F,
}

fn wheel_case(ee: &mut Ee, pick: &mut Pick, case: usize, shipped: &[Tire; 3]) -> Wheel {
    let k = case % 3;
    let t = ee.u32_at(PLANE + TIRES[k]);
    let s = shipped[k];
    let (rows, quat) = rotation(
        f64::from(pick.range(-3.2, 3.2)),
        f64::from(if pick.one_in(6) { pick.range(-3.2, 3.2) } else { pick.range(-0.4, 0.4) }),
        f64::from(if pick.one_in(6) { pick.range(-3.2, 3.2) } else { pick.range(-0.4, 0.4) }),
    );
    let pos = Vf::new(pick.range(0.0, 7000.0), pick.range(-50.0, 400.0), pick.range(0.0, 7000.0), 1.0);
    let frame = [rows[0], rows[1], rows[2], pos];
    for (r, v) in frame.iter().enumerate() {
        put(ee, PLANE + FRAME + 16 * r as u32, *v);
    }
    let rates = Vf::new(pick.range(-2.0, 2.0), pick.range(-2.0, 2.0), pick.range(-2.0, 2.0), 1.0);
    put(ee, PLANE + RATES, rates);
    let speed = if pick.one_in(8) { 0.0 } else { pick.range(-30.0, 250.0) };
    ee.set_f32(PLANE + at::SPEED, speed);
    let brake = pick.stick(0.0);
    let steer = pick.stick(-1.0);
    ee.set_f32(PLANE + BRAKE, brake);
    ee.set_f32(PLANE + STEER, steer);

    // the tuning, around what ships
    let scalars = [
        (0x3c, s.max_compression),
        (0x40, s.max_decompression_rate),
        (0x44, s.spring),
        (0x48, s.bounce),
        (0x4c, s.rebound),
        (0x158, s.max_braking),
        (0x15c, s.side_force),
        (0x160, s.brake_friction),
        (0x164, s.steering_moment),
    ];
    for (o, x) in scalars {
        let x = if pick.one_in(4) { x.to_f32() } else { x.to_f32() * pick.range(0.5, 1.5) };
        ee.set_f32(t + o, x);
    }
    let radius = s.radius.to_f32() * pick.range(0.7, 1.3);
    ee.set_f32(t + 0x150, radius);
    ee.set_f32(t + 0x154, 1.0 / radius);
    let c = s.center;
    put(ee, t + 0x60, Vf::new(c.x() + pick.range(-0.2, 0.2), c.y() + pick.range(-0.2, 0.2), c.z() + pick.range(-0.2, 0.2), c.w()));

    // the state the last substep left
    ee.set_u32(t + 0x80, u32::from(pick.one_in(3)));
    ee.set_u32(t + 0x84, u32::from(pick.one_in(12)));
    ee.set_u32(t + 0x88, u32::from(pick.one_in(2)));
    ee.set_f32(t + 0x90, if pick.one_in(4) { 0.0 } else { pick.range(0.0, 0.6) });
    ee.set_f32(t + 0x120, pick.range(-3.2, 3.2));
    let last = normal_near_up(pick);
    put(ee, t + 0x140, last);
    // the probe's lean, as a probe would have left it: one radius down the plane's up row, give or take
    let lean = Vf::new(-0.0, -radius * pick.range(0.8, 1.0), radius * pick.range(-0.6, 0.6), 1.0);
    put(ee, t + 0x50, lean);
    for o in [0xe0, 0x190, 0x1a0, 0x1b0] {
        put(ee, t + o, Vf::new(pick.range(-1.0, 1.0), pick.range(-1.0, 1.0), pick.range(-1.0, 1.0), 0.0));
    }
    put(ee, t + 0x180, Vf::new(pick.range(-9.0, 9.0), 0.0, pick.range(-9.0, 9.0), if pick.one_in(2) { 1.0 } else { 0.0 }));

    // the ground: through the bottom of the tire, give or take its radius
    let center = vector(ee, t + 0x60);
    let local = [center.x() + lean.x(), center.y() + lean.y(), center.z() + lean.z()];
    let low: Vec<f32> = (0..3).map(|i| pos[i] + rows[0][i] * local[0] + rows[1][i] * local[1] + rows[2][i] * local[2]).collect();
    let normal = normal_near_up(pick);
    let (sx, sz) = if normal.y().abs() > 0.05 { (-normal.x() / normal.y(), -normal.z() / normal.y()) } else { (0.0, 0.0) };
    let (sx, sz) = (sx.clamp(-3.0, 3.0), sz.clamp(-3.0, 3.0));
    let y_at_low = low[1] + radius * pick.range(-1.0, 2.5);
    let world = Slope { y0: y_at_low - sx * low[0] - sz * low[2], sx, sz, normal };
    put(ee, t + 0xd0, normal);
    let ground = Vf::new(low[0] + pick.range(-0.5, 0.5), 0.0, low[2] + pick.range(-0.5, 0.5), 1.0);
    put(ee, t + 0xc0, Vf::new(ground.x(), world.y(ground.x(), ground.z()), ground.z(), 1.0));

    // the body
    let b = ee.u32_at(t + 0x12c);
    assert_eq!(b, ee.u32_at(PLANE + at::BODY), "tire {k} is not on the plane's body");
    let body = Logged {
        enabled: true,
        pos: Vf::new(pos.x() + pick.range(-1.0, 1.0), pos.y() + pick.range(-1.0, 1.0), pos.z() + pick.range(-1.0, 1.0), 1.0),
        vel: Vf::new(pick.range(-60.0, 60.0), pick.range(-30.0, 30.0), pick.range(-60.0, 60.0), 1.0),
        force: Vf::new(pick.range(-3000.0, 3000.0), pick.range(-3000.0, 3000.0), pick.range(-3000.0, 3000.0), 1.0),
        quat: if pick.one_in(8) { normal_near_up(pick) } else { quat },
        log: Vec::new(),
    };
    put(ee, b + BODY_POS, body.pos);
    put(ee, b + BODY_VEL, body.vel);
    put(ee, b + BODY_FORCE, body.force);
    put(ee, b + BODY_QUAT, body.quat);

    let dt = if pick.one_in(8) { 0.0 } else { pick.range(0.001, 0.03) };
    Wheel { tire: t, mount: Mount { frame, rates, speed: F(speed), brake: F(brake), steer: F(steer) }, body, world, dt: F(dt) }
}

/// The plane with every tire's ground query hooked to the case's slope (shared through the cell).
fn wheels() -> Option<(Ee, Rc<RefCell<Vec<Push>>>, Rc<RefCell<Slope>>, [Tire; 3])> {
    let mut ee = plane()?;
    let body = ee.u32_at(PLANE + at::BODY);
    let seen = log_body(&mut ee, body);
    let slope = Rc::new(RefCell::new(Slope { y0: 0.0, sx: 0.0, sz: 0.0, normal: Vf::new(0.0, 1.0, 0.0, 1.0) }));
    let s = slope.clone();
    ee.hooks.insert(addr("ground_query").unwrap(), Box::new(move |ee| {
        let (point, normal) = (ee.r(5) as u32, ee.r(6) as u32);
        let s = *s.borrow();
        let y = s.y(ee.f32_at(point), ee.f32_at(point + 8));
        ee.set_f32(point + 4, y);
        put(ee, normal, s.normal);
        ee.set(2, 1);
    }));
    let shipped = TIRES.map(|o| tire_of(&ee, ee.u32_at(PLANE + o)));
    assert!(shipped[0].braked && !shipped[0].steered && shipped[2].steered, "the tires are not wired the way S39 read them");
    assert_eq!(ee.u32_at(ee.u32_at(PLANE + TIRES[0]) + 0x30), PLANE + BRAKE, "the brake is not the plane's");
    assert_eq!(ee.u32_at(ee.u32_at(PLANE + TIRES[2]) + 0x34), PLANE + STEER, "the steer is not the plane's");
    Some((ee, seen, slope, shipped))
}

#[test]
fn the_tire_probe_matches_the_original() {
    let Some((mut ee, seen, slope, shipped)) = wheels() else { return };
    let (clear, probe) = (addr("biplane_tire_preforce").unwrap(), addr("biplane_tire_probe").unwrap());
    let mut pick = Pick(0x3c6e_f372_fe94_f82b);
    let mut below = 0;
    for case in 0..CASES {
        let mut w = wheel_case(&mut ee, &mut pick, case, &shipped);
        *slope.borrow_mut() = w.world;
        let mut port = tire_of(&ee, w.tire);
        if pick.one_in(4) {
            ee.call(clear, &[u64::from(w.tire)], &[], BUDGET);
            port.clear();
        }
        seen.borrow_mut().clear();
        ee.call(probe, &[u64::from(w.tire)], &[], BUDGET);
        port.probe(&mut w.world, &w.mount);
        same_tire(&ee, case, w.tire, &port);
        assert!(seen.borrow().is_empty(), "case {case}: the probe pushed the body");
        below += usize::from(port.below);
    }
    assert!(below > CASES / 5 && below < CASES * 4 / 5, "only {below} of {CASES} probes found the ground");
    println!("the tire probe: {CASES} cases, bit-exact ({below} with the ground at the tire)");
}

#[test]
fn the_tire_forces_match_the_original() {
    let Some((mut ee, seen, slope, shipped)) = wheels() else { return };
    let force = addr("biplane_tire_force").unwrap();
    let mut pick = Pick(0xa54f_f53a_5f1d_36f1);
    let (mut touching, mut over, mut steered) = (0, 0, 0);
    for case in 0..CASES {
        let mut w = wheel_case(&mut ee, &mut pick, case, &shipped);
        *slope.borrow_mut() = w.world;
        let mut port = tire_of(&ee, w.tire);
        seen.borrow_mut().clear();
        ee.call(force, &[u64::from(w.tire)], &[w.dt.to_f32()], BUDGET);
        port.force(&mut w.body, &w.mount, w.dt);
        same_tire(&ee, case, w.tire, &port);
        same_pushes(case, &w.body.log, &seen.borrow());
        let b = ee.u32_at(PLANE + at::BODY);
        same_vector("body position", case, w.body.pos, vector(&ee, b + BODY_POS).0);
        same_vector("body velocity", case, w.body.vel, vector(&ee, b + BODY_VEL).0);
        same_vector("body force", case, w.body.force, vector(&ee, b + BODY_FORCE).0);
        touching += usize::from(port.touching);
        over += usize::from(w.body.log.iter().any(|p| matches!(p, Push::Velocity(_))));
        steered += usize::from(w.body.log.iter().any(|p| matches!(p, Push::Moment(_))));
    }
    assert!(touching > CASES / 5, "only {touching} of {CASES} cases touched");
    assert!(over > CASES / 50, "only {over} of {CASES} cases bottomed the strut");
    println!("the tire forces: {CASES} cases, bit-exact ({touching} touching, {over} bottomed, {steered} steering)");
}
