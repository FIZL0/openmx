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
//! S38 adds the body the plane flies on, the same way, on the plane's own body in the same state:
//!
//! 5. **the integrator**, **the end of a substep**, **the local force and moment** and **gravity**,
//!    each with the whole body generated (a pinned body, a clear inverse inertia and signed zeros
//!    included) and every field compared after;
//! 6. **the pose** the wrapper hands the model after the substeps, with the actor's virtuals hooked;
//! 7. **the measure**, with the surfaces' animation and the crash test hooked out;
//! 8. **the world step**, on a world built in scratch memory whose listeners and bodies are hooks
//!    that log their calls and now and then grow their own lists.
//!
//! What is not proved here, and why, is in the journal entries' last sections.

use game_sim::air::AcosTable;
use game_sim::biplane::{
    Aero, Air, Attitude, Autopilot, Body, Dynamics, Engine, Joystick, List, Measure, Pad, Surfaces, World, autopilot,
    controls,
};
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

// ---------------------------------------------------------------------------------------------------
// S38 — the rigid body, the world step and the measure
// ---------------------------------------------------------------------------------------------------

/// The body's fields, all of them quadwords but the first two.
mod body_at {
    pub const ENABLED: u32 = 0x00;
    pub const INV_MASS: u32 = 0x10;
    pub const DAMPING: u32 = 0x20;
    pub const LAST_POSITION: u32 = 0x50;
    pub const POSITION: u32 = 0x60;
    pub const VELOCITY: u32 = 0x70;
    pub const LAST_FORCE: u32 = 0x80;
    pub const FORCE: u32 = 0x90;
    pub const ACCEL: u32 = 0xa0;
    pub const SHOCK_PEAK: u32 = 0xb0;
    pub const SHOCK: u32 = 0xc0;
    pub const LISTENERS: u32 = 0xd0;
    pub const STEP: u32 = 0xf0;
    pub const INV_INERTIA: u32 = 0x120;
    pub const ANG_DAMPING: u32 = 0x130;
    pub const LAST_QUAT: u32 = 0x150;
    pub const QUAT: u32 = 0x170;
    pub const COFG: u32 = 0x180;
    pub const MOMENT: u32 = 0x190;
    pub const LAST_MOMENT: u32 = 0x1a0;
    pub const RATES: u32 = 0x1b0;
    pub const ANG_ACCEL: u32 = 0x1c0;
    pub const TWIST_PEAK: u32 = 0x1d0;
    pub const TWIST: u32 = 0x1e0;
    /// The body's vtable slots.
    pub const END_SUBSTEP: u32 = 0x30;
    pub const INTEGRATE: u32 = 0x44;
}
/// The plane's wrapper round the body (its actor), and the wrapper's fields.
const WRAPPER: u32 = 0x50;
const WRAPPER_BODY: u32 = 0x110;
const WRAPPER_GRAVITY: u32 = 0xf0;
const WRAPPER_COLLISION: u32 = 0x120;
const WRAPPER_ACTOR_VTABLE: u32 = 0x38;
/// The plane's own vtable, and the measure's fields past S34's.
const PLANE_VTABLE: u32 = 0x48;
const PLANE_COLLISION: u32 = 0x170;
const PLANE_WHEELS: u32 = 0x408;
const PLANE_SOUND: u32 = 0x67c;

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| ee.f32_at(a + k)))
}

fn set_quad(ee: &mut Ee, a: u32, v: Vf) {
    for k in 0..4 {
        ee.set_f32(a + 4 * k as u32, v[k]);
    }
}

fn the_body(ee: &Ee) -> u32 {
    ee.u32_at(PLANE + at::BODY)
}

/// The body as the port holds it, off the original's fields.
fn body_of(ee: &Ee, b: u32) -> Body {
    use body_at::*;
    Body {
        enabled: ee.u32_at(b + ENABLED) != 0,
        inv_mass: F(ee.f32_at(b + INV_MASS)),
        damping: quad(ee, b + DAMPING),
        last_position: quad(ee, b + LAST_POSITION),
        position: quad(ee, b + POSITION),
        velocity: quad(ee, b + VELOCITY),
        last_force: quad(ee, b + LAST_FORCE),
        force: quad(ee, b + FORCE),
        accel: quad(ee, b + ACCEL),
        shock_peak: quad(ee, b + SHOCK_PEAK),
        shock: quad(ee, b + SHOCK),
        step: quad(ee, b + STEP),
        inv_inertia: quad(ee, b + INV_INERTIA),
        ang_damping: quad(ee, b + ANG_DAMPING),
        last_quat: quad(ee, b + LAST_QUAT),
        quat: quad(ee, b + QUAT),
        cofg: quad(ee, b + COFG),
        moment: quad(ee, b + MOMENT),
        last_moment: quad(ee, b + LAST_MOMENT),
        rates: quad(ee, b + RATES),
        ang_accel: quad(ee, b + ANG_ACCEL),
        twist_peak: quad(ee, b + TWIST_PEAK),
        twist: quad(ee, b + TWIST),
    }
}

fn write_body(ee: &mut Ee, b: u32, body: &Body) {
    use body_at::*;
    ee.set_u32(b + ENABLED, u32::from(body.enabled));
    ee.set_f32(b + INV_MASS, body.inv_mass.to_f32());
    for (at, v) in [
        (DAMPING, body.damping),
        (LAST_POSITION, body.last_position),
        (POSITION, body.position),
        (VELOCITY, body.velocity),
        (LAST_FORCE, body.last_force),
        (FORCE, body.force),
        (ACCEL, body.accel),
        (SHOCK_PEAK, body.shock_peak),
        (SHOCK, body.shock),
        (STEP, body.step),
        (INV_INERTIA, body.inv_inertia),
        (ANG_DAMPING, body.ang_damping),
        (LAST_QUAT, body.last_quat),
        (QUAT, body.quat),
        (COFG, body.cofg),
        (MOMENT, body.moment),
        (LAST_MOMENT, body.last_moment),
        (RATES, body.rates),
        (ANG_ACCEL, body.ang_accel),
        (TWIST_PEAK, body.twist_peak),
        (TWIST, body.twist),
    ] {
        set_quad(ee, b + at, v);
    }
}

/// Every field of the port's body against the original's. The two accelerations' `w` lanes are what
/// the vector registers held (the integrator stores whole quadwords), so they are compared only where
/// `w` says.
fn check_body(case: usize, ee: &Ee, b: u32, got: &Body, accel_w: bool, ang_accel_w: bool) {
    let want = body_of(ee, b);
    assert_eq!(got.enabled, want.enabled, "case {case}: enabled");
    same("the inverse mass", case, got.inv_mass, want.inv_mass.to_f32());
    for (name, g, w, lanes) in [
        ("the damping", got.damping, want.damping, 4),
        ("the last position", got.last_position, want.last_position, 4),
        ("the position", got.position, want.position, 4),
        ("the velocity", got.velocity, want.velocity, 4),
        ("the last force", got.last_force, want.last_force, 4),
        ("the force", got.force, want.force, 4),
        ("the acceleration", got.accel, want.accel, if accel_w { 4 } else { 3 }),
        ("the shock peak", got.shock_peak, want.shock_peak, 4),
        ("the shock", got.shock, want.shock, 4),
        ("the step", got.step, want.step, 4),
        ("the inverse inertia", got.inv_inertia, want.inv_inertia, 4),
        ("the angular damping", got.ang_damping, want.ang_damping, 4),
        ("the last orientation", got.last_quat, want.last_quat, 4),
        ("the orientation", got.quat, want.quat, 4),
        ("the centre of mass", got.cofg, want.cofg, 4),
        ("the moment", got.moment, want.moment, 4),
        ("the last moment", got.last_moment, want.last_moment, 4),
        ("the rates", got.rates, want.rates, 4),
        ("the angular acceleration", got.ang_accel, want.ang_accel, if ang_accel_w { 4 } else { 3 }),
        ("the twist peak", got.twist_peak, want.twist_peak, 4),
        ("the twist", got.twist, want.twist, 4),
    ] {
        for k in 0..lanes {
            assert_eq!(g[k].to_bits(), w[k].to_bits(),
                       "case {case}: {name}[{k}] is {:e} in the game, {:e} in the port ({w:?} / {g:?})", w[k], g[k]);
        }
    }
}

impl Pick {
    fn vector(&mut self, scale: f32, w: f32) -> Vf {
        Vf::new(self.range(-scale, scale), self.range(-scale, scale), self.range(-scale, scale), w)
    }
    /// A vector with any `w`: the lanes the original carries along without reading.
    fn loose(&mut self, scale: f32) -> Vf {
        let w = self.range(-2.0, 2.0);
        self.vector(scale, w)
    }
    /// A unit quaternion as the host rounds it, now and then a little off the sphere or an axis one.
    fn quat(&mut self) -> Vf {
        match self.next() % 12 {
            0 => Vf::new(0.0, 0.0, 0.0, 1.0),
            1 => Vf::new(0.0, 1.0, 0.0, 0.0),
            _ => {
                let q = [0; 4].map(|_| self.range(-1.0, 1.0));
                let n = q.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3) * self.range(0.999, 1.001);
                Vf(q.map(|x| x / n))
            }
        }
    }
    /// One lane value from a small set, so that equal magnitudes, signed zeros and sign flips come up.
    fn lane(&mut self, near: f32) -> f32 {
        match self.next() % 7 {
            0 => 0.0,
            1 => -0.0,
            2 => near,
            3 => -near,
            _ => self.range(-1000.0, 1000.0),
        }
    }
}

/// A body around the plane's own: the live values each scaled, with the edges (a pinned body, a
/// clear inverse inertia, zero damping) mixed in.
fn random_body(pick: &mut Pick, live: &Body) -> Body {
    let inv_mass = match pick.next() % 8 {
        0 => 0.0,
        1 => -0.0,
        _ => pick.around(live.inv_mass.to_f32()),
    };
    let inv_inertia = match pick.next() % 8 {
        0 => Vf::new(0.0, 0.0, 0.0, 1.0),
        1 => Vf::new(-0.0, 0.0, 0.0, 1.0),
        _ => Vf::new(pick.around(live.inv_inertia[0]), pick.around(live.inv_inertia[1]),
                     pick.around(live.inv_inertia[2]), if pick.one_in(8) { 0.0 } else { 1.0 }),
    };
    let damping = |pick: &mut Pick| if pick.one_in(3) { Vf::splat(1.0) } else { pick.vector(1.0, 1.0) };
    Body {
        enabled: !pick.one_in(8),
        inv_mass: F(inv_mass),
        damping: damping(pick),
        last_position: pick.vector(500.0, 1.0),
        position: pick.loose(2000.0),
        velocity: pick.loose(80.0),
        last_force: pick.vector(5000.0, 1.0),
        force: pick.loose(30_000.0),
        accel: pick.vector(10.0, 1.0),
        shock_peak: pick.vector(100.0, 1.0),
        shock: pick.vector(100.0, 1.0),
        step: Vf::new(pick.range(0.0, 0.05), pick.range(0.0, 0.01), pick.range(0.0, 0.1), pick.range(-2.0, 2.0)),
        inv_inertia,
        ang_damping: damping(pick),
        last_quat: pick.quat(),
        quat: pick.quat(),
        cofg: if pick.one_in(2) { live.cofg } else { pick.vector(5.0, 1.0) },
        moment: pick.loose(20_000.0),
        last_moment: pick.vector(100.0, 1.0),
        rates: pick.loose(4.0),
        ang_accel: pick.loose(10.0),
        twist_peak: pick.vector(100.0, 1.0),
        twist: pick.vector(100.0, 1.0),
    }
}

/// A substep length: mostly a 30 Hz frame cut by the game's rate, sometimes anything, sometimes 0.
fn substep(pick: &mut Pick) -> f32 {
    match pick.next() % 8 {
        0 => 0.0,
        1 => 1.0 / 120.0,
        2 => pick.range(0.0, 0.2),
        _ => pick.range(0.002, 0.02),
    }
}

#[test]
fn the_body_integrates_like_the_original() {
    let Some(mut ee) = plane() else { return };
    let b = the_body(&ee);
    let live = body_of(&ee, b);
    let integrate = ee.u32_at(ee.u32_at(b + BODY_VTABLE) + body_at::INTEGRATE);
    assert_eq!(ee.i32_at(b + body_at::LISTENERS), 0, "the plane's body has listeners in this state");
    let mut pick = Pick(0x3c6e_f372_fe94_f82b);
    for case in 0..CASES {
        let mut port = random_body(&mut pick, &live);
        write_body(&mut ee, b, &port);
        let h = substep(&mut pick);
        ee.call(integrate, &[u64::from(b)], &[h], BUDGET);
        port.integrate(F(h));
        // the linear acceleration's w is a register nothing loads; the angular one's is the position's
        // w only when the linear half ran
        check_body(case, &ee, b, &port, false, port.inv_mass != 0.0);
    }
    println!("the integrator: {CASES} cases, bit-exact");
}

#[test]
fn the_body_ends_a_substep_like_the_original() {
    let Some(mut ee) = plane() else { return };
    let b = the_body(&ee);
    let live = body_of(&ee, b);
    let end = ee.u32_at(ee.u32_at(b + BODY_VTABLE) + body_at::END_SUBSTEP);
    let mut pick = Pick(0xa54f_f53a_5f1d_36f1);
    for case in 0..CASES {
        let mut port = random_body(&mut pick, &live);
        let near = pick.range(-100.0, 100.0);
        for v in [&mut port.shock, &mut port.shock_peak, &mut port.twist, &mut port.twist_peak] {
            for k in 0..4 {
                v[k] = pick.lane(near);
            }
        }
        write_body(&mut ee, b, &port);
        ee.call(end, &[u64::from(b)], &[], BUDGET);
        port.end_substep();
        check_body(case, &ee, b, &port, true, true);
    }
    println!("the end of a substep: {CASES} cases, bit-exact");
}

#[test]
fn local_forces_and_moments_match_the_original() {
    let Some(mut ee) = plane() else { return };
    let b = the_body(&ee);
    let live = body_of(&ee, b);
    let vtable = ee.u32_at(b + BODY_VTABLE);
    let (force, moment) = (ee.u32_at(vtable + FORCE_SLOT), ee.u32_at(vtable + MOMENT_SLOT));
    assert_eq!((force, moment), (addr("body_force_local").unwrap(), addr("body_moment_local").unwrap()));
    let v = ee.scratch(0);
    let mut pick = Pick(0x510e_527f_ade6_82d1);
    for case in 0..CASES {
        let mut port = random_body(&mut pick, &live);
        write_body(&mut ee, b, &port);
        let x = pick.loose(30_000.0);
        set_quad(&mut ee, v, x);
        if pick.one_in(2) {
            ee.call(force, &[u64::from(b), u64::from(v)], &[], BUDGET);
            port.add_force_local(x);
        } else {
            ee.call(moment, &[u64::from(b), u64::from(v)], &[], BUDGET);
            port.add_moment_local(x);
        }
        check_body(case, &ee, b, &port, true, true);
    }
    println!("the local force and moment: {CASES} cases, bit-exact");
}

#[test]
fn gravity_is_pushed_like_the_original() {
    let Some(mut ee) = plane() else { return };
    let b = the_body(&ee);
    let live = body_of(&ee, b);
    let w = PLANE + WRAPPER;
    assert_eq!(ee.u32_at(w + WRAPPER_BODY), b, "the wrapper does not hold the plane's body");
    let shipped = quad(&ee, w + WRAPPER_GRAVITY);
    assert_eq!(shipped.to_bits(), Vf::new(0.0, -1499.9999, 0.0, 1.0).to_bits(), "the plane's gravity moved");
    let listener = addr("body_wrapper_force").unwrap();
    let collision = ee.u32_at(w + WRAPPER_COLLISION);
    let response = ee.u32_at(ee.u32_at(collision + 8) + 0x80);
    let responded = Rc::new(RefCell::new(0));
    let r = responded.clone();
    ee.hooks.insert(response, Box::new(move |_| *r.borrow_mut() += 1));
    let mut pick = Pick(0x9b05_688c_2b3e_6c1f);
    for case in 0..CASES {
        let mut port = random_body(&mut pick, &live);
        write_body(&mut ee, b, &port);
        let g = if pick.one_in(4) { shipped } else { pick.loose(3000.0) };
        set_quad(&mut ee, w + WRAPPER_GRAVITY, g);
        ee.set_u32(w + 0x1b0, 7);
        *responded.borrow_mut() = 0;
        ee.call(listener, &[u64::from(w)], &[substep(&mut pick)], BUDGET);
        port.push_gravity(g);
        check_body(case, &ee, b, &port, true, true);
        let n = u32::from(port.enabled);
        assert_eq!(*responded.borrow(), n, "case {case}: the collision response ran {} times", responded.borrow());
        assert_eq!(ee.u32_at(w + 0x1b0), if port.enabled { 0 } else { 7 }, "case {case}: wrapper+0x1b0");
    }
    println!("gravity: {CASES} cases, bit-exact");
}

#[test]
fn the_pose_is_put_like_the_original() {
    let Some(mut ee) = plane() else { return };
    let b = the_body(&ee);
    let live = body_of(&ee, b);
    let w = PLANE + WRAPPER;
    let post = addr("body_wrapper_post").unwrap();
    // the actor's virtuals do the transform's work; the pose they are handed is what is compared
    let actor = ee.u32_at(w + WRAPPER_ACTOR_VTABLE);
    let put = Rc::new(RefCell::new(Vec::<(u32, [f32; 4])>::new()));
    let say_yes = Rc::new(RefCell::new(true));
    let yes = say_yes.clone();
    ee.hooks.insert(ee.u32_at(actor + 0x10c), Box::new(move |ee| ee.set(2, u64::from(*yes.borrow()))));
    for slot in [0x128, 0xdc, 0xd8, 0x110] {
        ee.hooks.insert(ee.u32_at(actor + slot), Box::new(|_| {}));
    }
    ee.hooks.insert(addr("transform_touch").unwrap(), Box::new(|_| {}));
    let p = put.clone();
    ee.hooks.insert(addr("transform_set_pose").unwrap(), Box::new(move |ee| {
        let at = ee.r(7) as u32;
        p.borrow_mut().push((ee.r(6) as u32, [0, 4, 8, 12].map(|k| ee.f32_at(at + k))));
    }));
    let mut pick = Pick(0x1f83_d9ab_fb41_bd6b);
    for case in 0..CASES {
        let port = random_body(&mut pick, &live);
        write_body(&mut ee, b, &port);
        *say_yes.borrow_mut() = !pick.one_in(8);
        put.borrow_mut().clear();
        ee.call(post, &[u64::from(w)], &[substep(&mut pick)], BUDGET);
        let put = put.borrow();
        if !port.enabled || !*say_yes.borrow() {
            assert!(put.is_empty(), "case {case}: posed while disabled or refused");
            continue;
        }
        assert_eq!(put.len(), 1, "case {case}");
        let (quat, origin) = port.pose();
        assert_eq!(put[0].0, b + body_at::LAST_QUAT, "case {case}: not the last orientation");
        same_vector("the orientation", case, quat, [0, 1, 2, 3].map(|k| ee.f32_at(b + body_at::LAST_QUAT + 4 * k)));
        same_vector("the origin", case, origin, put[0].1);
    }
    println!("the pose: {CASES} cases, bit-exact");
}

#[test]
fn the_measure_matches_the_original() {
    let Some(mut ee) = plane() else { return };
    let b = the_body(&ee);
    let live = body_of(&ee, b);
    let measure = addr("biplane_measure").unwrap();
    let table = AcosTable::build();
    // the pose (proved above), the surfaces' animation (S35's) and the crash test (S39's) are not
    // the measure's own: they do nothing here, so the body the measure reads is the one written
    let vtable = ee.u32_at(PLANE + PLANE_VTABLE);
    for at in [addr("body_wrapper_post").unwrap(), ee.u32_at(vtable + 0x1b8), ee.u32_at(vtable + 0x1bc),
               addr("sound_trigger").unwrap()] {
        ee.hooks.insert(at, Box::new(|_| {}));
    }
    let sounded = Rc::new(RefCell::new(0));
    let s = sounded.clone();
    ee.hooks.insert(addr("touchdown_sound").unwrap(), Box::new(move |_| *s.borrow_mut() += 1));
    let sound = ee.u32_at(PLANE + PLANE_SOUND);
    let collision = ee.u32_at(PLANE + PLANE_COLLISION);
    let aero = ee.u32_at(PLANE + at::AERO);
    let wheels = [0, 4, 8].map(|k| ee.u32_at(PLANE + PLANE_WHEELS + k));
    let mut pick = Pick(0x5be0_cd19_137e_2179);
    for case in 0..CASES {
        let mut body = random_body(&mut pick, &live);
        if pick.one_in(3) {
            // slow, or straight along an axis: the clamps and the table's ends
            body.velocity = match pick.next() % 4 {
                0 => Vf::new(0.0, 0.0, 0.0, 1.0),
                1 => Vf::new(0.0, 0.0, pick.range(-50.0, 50.0), 1.0),
                2 => pick.vector(1e-3, 1.0),
                _ => Vf::new(pick.range(-50.0, 50.0), 0.0, 0.0, 1.0),
            };
        }
        write_body(&mut ee, b, &body);
        let was = pick.one_in(2);
        ee.set_u32(PLANE + at::GROUNDED, u32::from(was));
        let down = [0; 3].map(|_| match pick.next() % 4 {
            0 => 1,
            1 => 5,
            _ => 0,
        });
        for (w, d) in wheels.iter().zip(down) {
            ee.set_u32(w + 0x88, d);
        }
        let touching = pick.one_in(4);
        ee.set_u32(collision + 0x40, u32::from(touching));
        let has_sound = !pick.one_in(4);
        ee.set_u32(PLANE + PLANE_SOUND, if has_sound { sound } else { 0 });
        *sounded.borrow_mut() = 0;

        let mut port = Measure {
            grounded: was,
            forward: quad(&ee, PLANE + 0x5d0),
            up: quad(&ee, PLANE + 0x5e0),
            ..Measure::default()
        };
        ee.call(measure, &[u64::from(PLANE)], &[substep(&mut pick)], BUDGET);
        let (touchdown, true_speed) = port.contacts(down.map(|d| d != 0), touching);
        port.flight(&body, &table);

        assert_eq!(ee.u32_at(PLANE + at::GROUNDED), u32::from(port.grounded), "case {case}: grounded");
        assert_eq!(ee.u32_at(PLANE + 0x594), u32::from(port.was_grounded), "case {case}: was grounded");
        assert_eq!(ee.u32_at(aero + 0x30), u32::from(true_speed), "case {case}: the true speed flag");
        assert_eq!(*sounded.borrow(), u32::from(touchdown && has_sound), "case {case}: the touchdown sound");
        same("the airspeed", case, port.airspeed, ee.f32_at(PLANE + 0x598));
        same("alpha", case, port.alpha, ee.f32_at(PLANE + at::ALPHA));
        same("beta", case, port.beta, ee.f32_at(PLANE + at::BETA));
        for (name, got, at) in [
            ("the world rates", port.world_rates, 0x5b0),
            ("the body velocity", port.velocity, 0x5c0),
            ("the forward row", port.forward, 0x5d0),
            ("the up row", port.up, 0x5e0),
            ("the Euler angles", port.euler, 0x5f0),
            ("the matrix row 0", port.matrix.0[0], 0x600),
            ("the matrix row 1", port.matrix.0[1], 0x610),
            ("the matrix row 2", port.matrix.0[2], 0x620),
            ("the matrix row 3", port.matrix.0[3], 0x630),
        ] {
            same_vector(name, case, got, quad(&ee, PLANE + at).0);
        }
        // and what S34's functions read is what the measure left
        let flown = pick.one_in(2);
        ee.set_u32(PLANE + at::FLOWN, u32::from(flown));
        assert_eq!(port.attitude(flown, &body), attitude_of(&ee), "case {case}: the attitude");
        let s = surfaces_of(&ee);
        let air = port.air(&s, &body);
        same("Air::speed", case, air.speed, ee.f32_at(PLANE + at::SPEED));
        same_vector("Air::rates", case, air.rates, quad(&ee, b + body_at::RATES).0);
    }
    println!("the measure: {CASES} cases, bit-exact");
}

/// The world step against the original on a world built in scratch memory: five lists of fake
/// elements whose virtuals are hooks that log `(what, which, argument)`, against the port driving a
/// [`Dynamics`] that logs the same. Listeners grow their own lists now and then, which is what the
/// re-read counts are for.
mod world_at {
    pub const ITERATIONS: u32 = 0x00;
    pub const RATE: u32 = 0x04;
    /// Each list is `{count, _, array}` at these.
    pub const LISTS: [u32; 5] = [0x50, 0x68, 0x80, 0x98, 0xb0];
    /// Where each list's element keeps its virtual table, and the slot called.
    pub const TABLE_AT: [u32; 5] = [0x100, 0x08, 0x10, 0x00, 0x00];
    pub const SLOT: [u32; 5] = [0x44, 0x0c, 0x18, 0x08, 0x08];
    pub const END_SLOT: u32 = 0x30;
    pub const ID: u32 = 0x20;
}
const CAPACITY: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ev {
    Pre(u32, u32),
    Force(u32, u32),
    Integrate(u32, u32),
    End(u32),
    Constrain(u32, bool),
    Post(u32, u32),
}

/// Which call on each list grows it by one (counted from 0 over the whole frame).
#[derive(Clone, Copy, Default)]
struct Growth {
    at: [Option<usize>; 5],
    calls: [usize; 5],
}

impl Growth {
    /// One call on `list`: whether it is the one that grows it.
    fn call(&mut self, list: usize) -> bool {
        let now = self.calls[list];
        self.calls[list] += 1;
        self.at[list] == Some(now)
    }
}

struct Fake {
    counts: [i32; 5],
    enabled: Vec<bool>,
    growth: Growth,
    log: Vec<Ev>,
}

fn index(list: List) -> usize {
    match list {
        List::Bodies => 0,
        List::Force => 1,
        List::Constraints => 2,
        List::Preforce => 3,
        List::Post => 4,
    }
}

impl Fake {
    fn grow(&mut self, list: usize) {
        if self.growth.call(list) && (self.counts[list] as usize) < CAPACITY {
            self.counts[list] += 1;
        }
    }
}

impl Dynamics for Fake {
    fn count(&self, list: List) -> i32 {
        self.counts[index(list)]
    }
    fn preforce(&mut self, i: i32, dt: F) {
        self.log.push(Ev::Pre(i as u32, dt.to_bits()));
        self.grow(3);
    }
    fn force(&mut self, i: i32, h: F) {
        self.log.push(Ev::Force(i as u32, h.to_bits()));
        self.grow(1);
    }
    fn enabled(&self, body: i32) -> bool {
        self.enabled[body as usize]
    }
    fn integrate(&mut self, body: i32, h: F) {
        self.log.push(Ev::Integrate(body as u32, h.to_bits()));
        self.grow(0);
    }
    fn end_substep(&mut self, body: i32) {
        self.log.push(Ev::End(body as u32));
    }
    fn constrain(&mut self, i: i32, first: bool) {
        self.log.push(Ev::Constrain(i as u32, first));
        self.grow(2);
    }
    fn post(&mut self, i: i32, dt: F) {
        self.log.push(Ev::Post(i as u32, dt.to_bits()));
        self.grow(4);
    }
}

#[test]
fn the_world_steps_like_the_original() {
    let Some(mut ee) = plane() else { return };
    use world_at::*;
    // the game's own world: what the port's numbers are, and that nothing constrains anything
    let game = addr("dynamics_world").unwrap();
    assert_eq!(ee.i32_at(game + ITERATIONS), 1);
    assert_eq!(ee.f32_at(game + RATE).to_bits(), 115.0f32.to_bits());
    assert_eq!(ee.i32_at(game + LISTS[2]), 0, "the state has constraints");
    let bodies = ee.u32_at(game + LISTS[0] + 8);
    let n = ee.i32_at(game + LISTS[0]) as u32;
    assert!((0..n).any(|k| ee.u32_at(bodies + 4 * k) == the_body(&ee)), "the plane's body is not in the world");

    let step = addr("dynamics_world_step").unwrap();
    // scratch: the world, then per list an array and its elements, then the virtual tables
    let world = ee.scratch(0);
    let mut next = world + 0x100;
    let mut alloc = |size: u32| {
        let at = next;
        next += size;
        at
    };
    let arrays: [u32; 5] = [0; 5].map(|_| alloc(4 * CAPACITY as u32 + 12));
    let elements: Vec<Vec<u32>> =
        (0..5).map(|l| (0..CAPACITY).map(|_| alloc(if l == 0 { 0x110 } else { 0x30 })).collect()).collect();
    let tables: [u32; 5] = [0; 5].map(|_| alloc(0x50));
    for l in 0..5 {
        ee.set_u32(world + LISTS[l] + 8, arrays[l]);
        for (k, &e) in elements[l].iter().enumerate() {
            ee.set_u32(arrays[l] + 4 * k as u32, e);
            ee.set_u32(e + TABLE_AT[l], tables[l]);
            ee.set_u32(e + ID, k as u32);
        }
    }
    let log = Rc::new(RefCell::new(Vec::<Ev>::new()));
    let growth = Rc::new(RefCell::new(Growth::default()));
    let hook = |l: usize| 0x0000_1000 + 0x10 * l as u32;
    for l in 0..5 {
        ee.set_u32(tables[l] + SLOT[l], hook(l));
        let (log, growth) = (log.clone(), growth.clone());
        let count_at = world + LISTS[l];
        ee.hooks.insert(hook(l), Box::new(move |ee| {
            let id = ee.u32_at(ee.r(4) as u32 + ID);
            let arg = ee.f(12).to_bits();
            log.borrow_mut().push(match l {
                0 => Ev::Integrate(id, arg),
                1 => Ev::Force(id, arg),
                2 => Ev::Constrain(id, ee.r(5) != 0),
                3 => Ev::Pre(id, arg),
                _ => Ev::Post(id, arg),
            });
            let count = ee.i32_at(count_at);
            if growth.borrow_mut().call(l) && (count as usize) < CAPACITY {
                ee.set_u32(count_at, (count + 1) as u32);
            }
        }));
    }
    let end = hook(5);
    ee.set_u32(tables[0] + END_SLOT, end);
    let l = log.clone();
    ee.hooks.insert(end, Box::new(move |ee| {
        let id = ee.u32_at(ee.r(4) as u32 + ID);
        l.borrow_mut().push(Ev::End(id));
    }));

    let mut pick = Pick(0x6295_8d5e_c0ba_0df1);
    for case in 0..CASES {
        let port = World {
            iterations: match pick.next() % 8 {
                0 => 0,
                1 => -1,
                2 => 2,
                3 => 3,
                _ => 1,
            },
            rate: F(if pick.one_in(4) { pick.range(1.0, 300.0) } else { 115.0 }),
        };
        let dt = match pick.next() % 8 {
            0 => 0.0,
            1 => 1.0 / 60.0,
            2 => pick.range(0.0, 0.1),
            _ => 1.0 / 30.0,
        };
        let count = |pick: &mut Pick| match pick.next() % 8 {
            0 => 0,
            1 => -1,
            _ => (pick.next() % 4) as i32,
        };
        let counts = [0; 5].map(|_| count(&mut pick));
        let enabled: Vec<bool> = (0..CAPACITY).map(|_| !pick.one_in(4)).collect();
        let mut g = Growth::default();
        for at in g.at.iter_mut() {
            if pick.one_in(3) {
                *at = Some((pick.next() % 6) as usize);
            }
        }
        ee.set_u32(world + ITERATIONS, port.iterations as u32);
        ee.set_f32(world + RATE, port.rate.to_f32());
        for l in 0..5 {
            ee.set_u32(world + LISTS[l], counts[l] as u32);
        }
        for (k, &e) in elements[0].iter().enumerate() {
            ee.set_u32(e, u32::from(enabled[k]));
        }
        *growth.borrow_mut() = g;
        log.borrow_mut().clear();
        ee.call(step, &[u64::from(world)], &[dt], 4_000_000);

        let mut fake = Fake { counts, enabled, growth: g, log: Vec::new() };
        port.step(F(dt), &mut fake);
        assert_eq!(*log.borrow(), fake.log, "case {case}: {port:?} dt {dt} counts {counts:?}");
    }
    println!("the world step: {CASES} cases, the same calls in the same order with the same arguments");
}
