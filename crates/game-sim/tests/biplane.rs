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
//!
//! I6 — the biplane's pad against the human's biplane recording played back on the original (C7).
//! The capture is `context/capture/acceptance/biplane.csv` (`watch/biplane.toml`, journal
//! 2026-10-20-c7-c7-biplane-capture): 1,718 ticks of Free06 from frame counter 10971, with both
//! halves of the pad library's buffer, the player record's sticks and control table, and what the
//! plane's input step wrote from them. It is game data, so the tests that read it skip without it.
//!
//! S36 — the biplane's crash against the original: S36's capture of the human's biplane recording played
//! back (`context/capture/s36/biplane-crash.csv`, one row per tick, made by
//! research/journal/2026-10-20-s36-s36-the-biplane-crashes/play_crash.sh) and the disc's `biplane.phy`.

use game_sim::biplane::{Aero, Air, Attitude, Autopilot, Engine, Joystick, Pad, Surfaces, autopilot, controls};
use game_sim::biplane::{Contact, Placement, Plane, Prop, Wreck, Wrecks, map_extent, near_edge};
use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::input::{Axis, AxisRange, Control, Manager, PlaneCommand, PlaneInput, Report};
use game_sim::replay::{Capture, PadTape};
use game_sim::rng::Rng;
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

/// The capture, or `None` (and a line saying how to make it) when this checkout does not have it.
fn capture() -> Option<Capture> {
    let path = match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => std::path::PathBuf::from(dir).join("capture"),
        None => std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/capture"),
    }
    .join("acceptance/biplane.csv");
    let Ok(text) = std::fs::read_to_string(&path) else {
        println!(
            "skipped: no {} — play biplane.p2m2 back with watch/biplane.toml (journal \
             2026-10-20-c7-c7-biplane-capture) and record frame counters 10971..12688",
            path.display()
        );
        return None;
    };
    Some(Capture::parse(&text).expect("the recorder's format"))
}

/// One f32 cell, as its bits.
fn bits(cap: &Capture, row: usize, col: &str) -> u32 {
    let at = cap.columns.iter().position(|c| c == col).unwrap_or_else(|| panic!("no column {col}"));
    let cell = &cap.rows[row][at];
    cell.parse::<f32>().unwrap_or_else(|_| panic!("row {row} {col}: '{cell}'")).to_bits()
}

/// The rows whose stick columns the recorder read before that tick's input step had written them
/// (C7's journal: row 150 is its torn row, and 158–179 are the rows where two playbacks disagree on
/// the sticks). On each, every stick-derived column holds the *previous* tick's value — which this
/// test checks rather than skipping the row.
const STICKS_READ_EARLY: [usize; 6] = [150, 158, 162, 163, 177, 179];

/// I6: the recorded pad, through the port's input layer and the plane's mapping, gives the eight
/// controls the original read — tick for tick, to the bit.
///
/// Every tick of the capture is checked three ways, each against the original's own RAM:
///
/// 1. **The control table**: the sixteen values at the player record's `+0x334` are the port's
///    control strengths ([`game_sim::input::Pad::strength`]) — so a throttle command's value is its
///    pressure byte's `(p + 1) / 256`, and a digital control is 1.0. One exception, measured and
///    not explained: R1's entry reads 0 after every tick R1 is held (154 of 154), though the plane
///    read it as held (its rudder is +1 on each of those ticks); something after the input step
///    clears it.
/// 2. **The sticks**: the player record's `+0x324`/`+0x328` are the left stick through
///    [`AxisRange::PLAYER`]'s ±0.3 dead zone.
/// 3. **The five commands** the plane's input step wrote (`+0x338..+0x348`) are
///    [`PlaneInput::from_pad`]'s.
///
/// Except on [`STICKS_READ_EARLY`]'s six rows, where the sticks and the two stick-derived commands
/// are the previous tick's, every value is the port's to the bit.
#[test]
fn the_biplane_reads_the_pad() {
    let Some(cap) = capture() else { return };
    let tape = PadTape::from_capture(&cap).expect("a dense capture with both pad halves");
    assert_eq!((cap.rows.len(), tape.frames.len()), (1718, 1717), "the capture is C7's");

    let mut input = Manager::connected();
    let mut bad: Vec<String> = Vec::new();
    let mut early = Vec::new();
    let mut r1_cleared = 0;
    let mut last: Option<[(&str, F); 4]> = None;
    for (k, frame) in tape.frames.iter().enumerate() {
        let row = k + 1;
        input.tick(F::from_bits(frame.dt), &[frame.reports[0], Report::missing()]);
        let pad = input.pad(0);
        for c in Control::ALL {
            let col = format!("pctl{}.value", c.index());
            let want = bits(&cap, row, &col);
            if c == Control::R1 && pad.held(c) {
                assert_eq!(want, 0, "row {row}: R1's entry, held");
                r1_cleared += 1;
            } else if want != pad.strength(c).to_bits() {
                bad.push(format!("row {row} {col}: {} vs {}", f32::from_bits(want), pad.strength(c).0));
            }
        }
        let p = PlaneInput::from_pad(pad);
        let stick = (pad.axis_through(Axis::LeftX, &AxisRange::PLAYER), pad.axis_through(Axis::LeftY, &AxisRange::PLAYER));
        let sticky = [("stick_x", stick.0), ("stick_y", stick.1), ("elev_cmd", p.elevator), ("ail_cmd", p.aileron)];
        let this_tick = sticky.iter().all(|(col, v)| bits(&cap, row, col) == v.to_bits());
        if !this_tick {
            let prev = last.expect("not the first tick");
            let as_before = prev.iter().all(|(col, v)| bits(&cap, row, col) == v.to_bits());
            assert!(as_before, "row {row}: the sticks are neither this tick's {sticky:?} nor the last's {prev:?}");
            early.push(row);
        }
        last = Some(sticky);
        for (col, v) in [("thr_up", p.throttle_up), ("thr_down", p.throttle_down), ("rud_cmd", p.rudder)] {
            if bits(&cap, row, col) != v.to_bits() {
                bad.push(format!("row {row} {col}: {} vs {}", f32::from_bits(bits(&cap, row, col)), v.0));
            }
        }
    }
    assert!(bad.is_empty(), "{} values are not the original's:\n{}", bad.len(), bad.join("\n"));
    assert_eq!(early, STICKS_READ_EARLY, "the rows whose sticks are the previous tick's");
    assert_eq!(r1_cleared, 154, "R1's held ticks");
    println!(
        "the biplane reads the pad: {} ticks, 16 table values, 2 sticks and 5 commands each, bit-exact \
         ({} rows with last tick's sticks, {r1_cleared} with R1's entry cleared)",
        tape.frames.len(),
        early.len()
    );
}

/// The biplane state's EE RAM, from this checkout's context or the one `kit.toml` points at.
fn plane_state() -> Option<Ee> {
    const STATE: &str = "biplane.p2m2_SaveState.eeMemory.bin";
    let mut tried = vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/states").join(STATE)];
    if let Ok(kit) = Kit::load() {
        tried.push(kit.ctx("states").join(STATE));
    }
    for at in &tried {
        if let Ok(ee) = Ee::from_ram(at) {
            return Some(ee);
        }
    }
    skip(&format!(
        "no {STATE} — `./kit state ram context/recordings/biplane.p2m2_SaveState.p2s` writes it"
    ));
    None
}

/// I6: the plane's input step against the original's own, over the biplane state's RAM — the
/// shipped binding table in place — on 12,000 generated sticks and control tables, for both pads.
///
/// The capture proves the mapping on what the human pressed; this proves it on what they did not:
/// the d-pad's override of the stick (never pressed in the recording), both of a pair held at once,
/// a stick past ±1, an exact and a negative zero, and every binding the table has for pad 1.
#[test]
fn the_biplane_input_step_is_the_originals() {
    let Some(mut ee) = plane_state() else { return };
    let at = match addr("biplane_input") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    // Scratch for the plane's five fields and the player's table, below the oracle's stack.
    const PLANE: u32 = 0x01ff_0000;
    const TABLE: u32 = 0x01ff_1000;

    let mut seed: u64 = 0x16_b1_91_a9e;
    let mut next = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as u32
    };
    let mut checked = 0;
    for n in 0..12_000u32 {
        let pad = n % 2;
        // A stick value: one a report byte gives through the ride's range, or something no pad can
        // give, or the zeros.
        let mut stick = || -> F {
            match next() % 8 {
                0 => F::ZERO,
                1 => F(-0.0),
                2 => F::from_bits(next() & 0x807f_ffff | 0x3f00_0000), // 0.5..1, either sign
                3 => F::from_bits(next() & 0x80ff_ffff | 0x3f00_0000), // 0.5..2, either sign
                _ => AxisRange::PLAYER.apply(F::from((next() & 0xff) as i32), F(255.0)),
            }
        };
        let (x, y) = (stick(), stick());
        let mut table = [F::ZERO; 16];
        for v in &mut table {
            *v = match next() % 4 {
                0 | 1 => F::ZERO,
                2 => F::ONE,
                _ => F::from((next() % 256) as i32 + 1) * F(0.003_906_25),
            };
        }
        for i in 0..16u32 {
            ee.set_u32(TABLE + i * 0x18 + 4, table[i as usize].to_bits());
        }
        for off in [0x338, 0x33c, 0x340, 0x344, 0x348] {
            ee.set_u32(PLANE + off, 0xdead_beef);
        }
        ee.call(at, &[u64::from(PLANE), u64::from(TABLE), u64::from(pad)], &[x.0, y.0], 10_000);

        let port = PlaneInput::from_sticks((x, y), |c: PlaneCommand| table[c.control().index()]);
        let theirs = [0x338, 0x33c, 0x340, 0x344, 0x348].map(|off| ee.u32_at(PLANE + off));
        let ours = [port.throttle_up, port.throttle_down, port.elevator, port.aileron, port.rudder].map(F::to_bits);
        assert_eq!(ours, theirs, "pad {pad}, stick ({x:?}, {y:?}), table {table:?}");
        checked += 1;
    }
    println!("the biplane input step: {checked} calls, both pads, bit-exact");
}

// --- S36: the crash -------------------------------------------------------------------------------

/// S36's capture: its header and its rows, every cell as the `f32` it was read as.
struct CrashCapture {
    names: Vec<String>,
    rows: Vec<Vec<f32>>,
}

impl CrashCapture {
    fn col(&self, name: &str) -> usize {
        self.names.iter().position(|n| n == name).unwrap_or_else(|| panic!("no column {name}"))
    }
    fn get(&self, row: usize, name: &str) -> f32 {
        self.rows[row][self.col(name)]
    }
    fn tick(&self, row: usize) -> u32 {
        self.get(row, "frame_counter") as u32
    }
    fn v3(&self, row: usize, name: &str) -> Vf {
        let g = |axis: &str| self.get(row, &format!("{name}.{axis}"));
        Vf::new(g("x"), g("y"), g("z"), 1.0)
    }
}

/// The two files, or `None` (and a SKIPPED line) when this checkout has not got them.
fn inputs() -> Option<(Wrecks, F, CrashCapture)> {
    use ps2_oracle::{Kit, skip};
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let phy = kit.ctx("res/machine/biplane/biplane.phy");
    let csv = kit.ctx("capture/s36/biplane-crash.csv");
    let phy = std::fs::read(&phy).map_err(|e| skip(&format!("{}: {e}", phy.display()))).ok()?;
    let csv = std::fs::read_to_string(&csv).map_err(|e| skip(&format!("{}: {e}", csv.display()))).ok()?;
    let ini = game_sim::phy::Ini::parse_bytes(&phy);
    let weight = ini.section("Mass Properties").float_or("Weight", F::ZERO);
    let mut lines = csv.lines();
    let names = lines.next()?.split(',').map(str::to_owned).collect();
    let rows = lines
        .map(|l| l.split(',').map(|c| c.parse::<f64>().expect("a number") as f32).collect())
        .collect();
    Some((Wrecks::read(&ini), weight, CrashCapture { names, rows }))
}

fn vbits(v: Vf) -> [u32; 3] {
    [v.x().to_bits(), v.y().to_bits(), v.z().to_bits()]
}

/// The recording crashes twice, both times into the ground above `MinWreckSpeed` (reason 1). The
/// second wreck is waited out and the first is cut short by the reset chord. The port is given what
/// the original's test reads on each tick — the body contact of that tick, the airspeed and up.y the
/// previous tick's measure left — and has to wreck on the same two ticks and nowhere else, count the
/// same wreck clock, unwreck the waited-out one on the same tick (151 ticks in), and put the plane
/// back where the original did: the spawn, the spawn's facing, standing still, under its own gravity.
#[test]
fn the_biplane_crashes_and_respawns() {
    let Some((w, weight, cap)) = inputs() else { return };
    assert_eq!((w.wreck_time, w.min_wreck_speed, w.wreck_gravity), (F(5.0), F(50.0) * F(1.46666), F(1.5)));
    let spawn = Placement { at: cap.v3(0, "at"), forward: cap.v3(0, "fwd"), up: cap.v3(0, "up") };
    let inertia = Vf::new(1000.0, 2000.0, 3000.0, 1.0);
    let mut plane = Plane::new(spawn, weight, inertia, F::ONE);
    let mut rng = Rng::default();

    // The plane's own gravity, the bits the capture has before any crash.
    assert_eq!(plane.gravity.y().to_bits(), cap.get(0, "gravity_y").to_bits(), "the flying gravity");

    let mut entries = Vec::new();
    let mut exits = Vec::new();
    for r in 1..cap.rows.len() {
        let tick = cap.tick(r);
        assert_eq!(tick, cap.tick(r - 1) + 1, "the capture has a gap before {tick}");
        let dt = F(cap.get(r, "dt"));
        plane.airspeed = F(cap.get(r - 1, "airspeed"));
        plane.up_y = F(cap.get(r - 1, "up_y"));
        plane.grounded = cap.get(r - 1, "grounded") != 0.0;
        plane.throttle = F(cap.get(r - 1, "throttle"));
        let was = plane.wreck;

        // The chord: the one exit the clock cannot account for is the reset chord's (L1+R1 held in the
        // recording just there), and the input slot runs before the measure.
        let theirs = cap.get(r, "wreck") as u32;
        let chord = was.is_some() && theirs == 0 && !(w.wreck_time < plane.clock + dt);
        plane.reset_chord(chord);
        plane.frame(dt);

        let contact = Contact { touching: cap.get(r, "contact") != 0.0, vehicle: false };
        plane.wreck_check(dt, &w, contact, None, &mut rng);

        assert_eq!(plane.wreck_word(), theirs, "the wreck word on tick {tick}");
        if was.is_none() && plane.wreck.is_some() {
            entries.push(tick);
            assert_eq!(plane.throttle, F::ZERO, "a wreck cuts the throttle");
            assert_eq!(plane.prop(), Prop::Bent);
        }
        if was.is_some() && plane.wreck.is_none() {
            exits.push((tick, chord));
            assert_eq!(plane.prop(), Prop::Blur);
        }
        if plane.wreck.is_some() {
            assert_eq!(plane.clock.0.to_bits(), cap.get(r, "wreck_clock").to_bits(), "the clock on {tick}");
        }
        assert_eq!(plane.gravity.y().to_bits(), cap.get(r, "gravity_y").to_bits(), "the gravity on {tick}");
        if was == Some(Wreck::Contact) && plane.wreck.is_none() && !chord {
            // Waited out: the body is exactly where the next tick's integration starts from.
            assert_eq!(vbits(plane.body.at), vbits(cap.v3(r, "pos")), "respawned at the spawn");
            assert_eq!(vbits(plane.body.vel), vbits(cap.v3(r, "vel")), "respawned standing still");
            assert_eq!(vbits(plane.body.spin), vbits(cap.v3(r, "spin")), "respawned not turning");
            assert_eq!((plane.body.forward, plane.body.up), (spawn.forward, spawn.up), "facing the spawn's way");
            assert_eq!(plane.body.inertia, inertia, "the inertia given back");
        }
    }
    assert_eq!(entries, [11664, 12498], "the two crashes' entry ticks");
    assert_eq!(exits, [(11747, true), (12649, false)], "the chord's exit, then the waited-out one");
    assert_eq!(exits[1].0 - entries[1], 151, "a waited-out wreck lasts 151 ticks at the capture's dt");
    assert_eq!(exits[0].0 - entries[0], 83, "the chord cut the first one short");
}

/// What the capture cannot show: the map edge's throw and its longer wreck, the chord's gating, the
/// speed rule's `VEHICLE` exception, and the upside-down rule.
#[test]
fn the_biplane_crash_rules_the_capture_does_not_reach() {
    let w = Wrecks {
        wreck_time: F(5.0),
        min_wreck_speed: F(50.0) * F(1.46666),
        wreck_gravity: F(1.5),
        wreck_damping: Vf::new(0.999, 0.999, 0.999, 1.0),
        wreck_inertia: Vf::new(0.5, 0.5, 0.5, 1.0),
    };
    let spawn = Placement {
        at: Vf::new(2304.0, 138.614_06, 2281.0, 1.0),
        forward: Vf::new(0.0, 0.0, -1.0, 1.0),
        up: Vf::new(0.0, 1.0, 0.0, 1.0),
    };
    let new = || Plane::new(spawn, F(1500.0), Vf::new(10.0, 20.0, 30.0, 1.0), F::ONE);
    let dt = F(0.033_33);
    let mut rng = Rng::default();
    let touch = Contact { touching: true, vehicle: false };

    // Slow, upright and touching: no wreck. Fast on a VEHICLE surface: none either. Upside down: one.
    let mut p = new();
    p.airspeed = w.min_wreck_speed;
    p.wreck_check(dt, &w, touch, None, &mut rng);
    assert_eq!(p.wreck, None, "at MinWreckSpeed exactly it is not a crash");
    p.airspeed = F(200.0);
    p.wreck_check(dt, &w, Contact { touching: true, vehicle: true }, None, &mut rng);
    assert_eq!(p.wreck, None, "a VEHICLE surface is spared the speed rule");
    p.airspeed = F::ZERO;
    p.up_y = F(-0.1);
    p.wreck_check(dt, &w, touch, None, &mut rng);
    assert_eq!(p.wreck, Some(Wreck::Contact), "upside down on anything is a crash");
    assert_eq!(p.body.inertia, Vf::new(5.0, 10.0, 15.0, 1.0));
    assert_eq!(p.body.damping, w.wreck_damping);

    // The chord: not in the air, once a frame, and the frame slot opens it again.
    let mut p = new();
    p.body.vel = Vf::new(1.0, 2.0, 3.0, 1.0);
    p.reset_chord(true);
    assert_eq!(p.body.vel, Vf::new(1.0, 2.0, 3.0, 1.0), "in the air the chord does nothing");
    p.grounded = true;
    p.reset_chord(true);
    assert_eq!(p.body.vel, Vf::new(0.0, 0.0, 0.0, 1.0), "on the wheels it resets");
    p.body.vel = Vf::new(1.0, 2.0, 3.0, 1.0);
    p.reset_chord(true);
    assert_eq!(p.body.vel, Vf::new(1.0, 2.0, 3.0, 1.0), "only once a frame");
    p.frame(dt);
    p.reset_chord(true);
    assert_eq!(p.body.vel, Vf::new(0.0, 0.0, 0.0, 1.0), "the next frame it can again");

    // The edge: a 4096 ft square map, the plane 100 ft in from its west edge.
    let size = map_extent(F(16.0), F(1.0), F(16.0), F(1.0));
    assert_eq!(size, (F(4096.0), F(4096.0)));
    assert!(near_edge(Vf::new(100.0, 0.0, 2048.0, 1.0), size));
    assert!(!near_edge(Vf::new(150.0, 0.0, 3946.0, 1.0), size), "150 ft in is still inside");
    let mut p = new();
    p.body.at = Vf::new(100.0, 500.0, 2048.0, 1.0);
    p.edge_scale = F(2.0);
    p.wreck_check(dt, &w, Contact::default(), Some(size), &mut rng);
    assert_eq!(p.wreck, Some(Wreck::MapEdge));
    assert_eq!(p.clock, dt, "the edge's wreck counts its first tick");
    assert_eq!(p.edge_scale, F(1.0));
    let v = p.body.vel;
    assert!(v.x() > 0.0 && v.y() > 0.0 && v.z() == 0.0, "thrown east and up: {v:?}");
    let s = p.body.spin;
    assert!((0.0..2.0).contains(&s.x()) && (0.0..1.6).contains(&s.y()) && (0.0..2.8).contains(&s.z()));

    // In the air it never comes back; down for 2.5 s then WreckTime, it does.
    for _ in 0..400 {
        p.wreck_check(dt, &w, Contact::default(), Some(size), &mut rng);
    }
    assert_eq!(p.wreck, Some(Wreck::MapEdge), "still flying, still wrecked");
    p.grounded = true;
    let mut ticks = 0;
    while p.wreck.is_some() {
        p.wreck_check(dt, &w, Contact::default(), Some(size), &mut rng);
        ticks += 1;
    }
    assert_eq!(ticks, 152, "one tick to see it is down, then WreckTime");
    assert_eq!(p.edge_scale, F(2.0), "the halving undone");
    assert_eq!(p.body.at, spawn.at);
}
