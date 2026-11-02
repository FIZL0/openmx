//! S148 — the Kiowa's flight controller against the original, on a save state's live helicopter.
//!
//! The state is free09 booted with the event's first machine set to the helicopter, then switched
//! into from the pause menu and saved standing on the ground (`context/states/s148-kiowa.ee.bin`;
//! the recipe is in research/journal/*-s148-kiowa). The interpreter calls the original's controller
//! on generated pads, gains, attitudes and loop states written into that helicopter, and the port's
//! [`Controller::step`] must leave every output and every piece of its state the same, bit for bit.
//!
//! S220 does the same for the rest of a substep's flight: the tilt's Euler quaternion, the rotor
//! model, the aerodynamics model, and the force and preforce listeners that run them.

use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::biplane::Body;
use game_sim::kiowa::{Aero, AeroState, Airflow, Commands, Controller, Filter, Flight, Kiowa, Loop, Pad, Readings};
use game_sim::kiowa::{Rotor, RotorState, euler_quat};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::cell::RefCell;
use std::rc::Rc;

const STATE: &str = "s148-kiowa.ee.bin";
const HELI: u32 = 0x007c_cae0;
const CASES: usize = 10_000;
const BUDGET: u64 = 200_000;

/// The helicopter's fields, by offset.
mod at {
    pub const PAD: u32 = 0x334; // climb, descend, (fraction), lateral, longitudinal, pedal
    pub const ALTITUDE_FRACTION: u32 = 0x33c;
    pub const OUT: u32 = 0x34c; // collective, lateral, longitudinal, pedal, filtered
    pub const LAST_STICK: u32 = 0x360;
    pub const FLOWN: u32 = 0x364;
    pub const GAINS: u32 = 0x368; // lateral, longitudinal, pedal, MaxAltitude
    pub const HOLD: u32 = 0x378; // height, timer, delay
    pub const COLLECTIVE: u32 = 0x384; // gain, climb, descent, hold gain, integral, integrator gain, min
    pub const PITCH_ROLL: u32 = 0x3a0; // MaxPitch, MaxRoll, PitchGain, RollGain, command, rate, min, max, gain
    pub const FEED_FORWARD: u32 = 0x3c4; // four floats
    pub const RATES: u32 = 0x430; // MinRateCommand, MaxRateCommand (vectors)
    pub const GRAVITY_SCALE: u32 = 0x190;
    pub const BODY: u32 = 0x160;
    pub const PILOT: u32 = 0x170;
    pub const AERO: u32 = 0x454;
    pub const FILTER: u32 = 0x46c;
    pub const AIRSPEED: u32 = 0x498;
    pub const PITCH: u32 = 0x4c0;
    pub const ROLL: u32 = 0x4c8;
}
const BODY_HEIGHT: u32 = 0x64;
const BODY_VEL: u32 = 0x70;
const BODY_RATES: u32 = 0x1b0;
const HIGH_SPEED_AERO: u32 = 0x100;

fn heli() -> Option<Ee> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: S148's Kiowa state (only in s3's context)", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    // The helicopter is where this file says: flown, with its filter and its file's gains in place.
    assert_eq!(ee.u32_at(HELI + at::FLOWN), 1, "no flown helicopter at {HELI:#x} in {}", path.display());
    assert_eq!(ee.f32_at(HELI + at::GAINS + 12), 550.0, "MaxAltitude is not 550 at {HELI:#x}");
    assert_eq!(ee.f32_at(ee.u32_at(HELI + at::FILTER) + 4), 1.0, "the collective filter is not wired");
    Some(ee)
}

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
    /// A stick or button: often exactly 0 (either sign) or an end.
    fn stick(&mut self, lo: f32) -> f32 {
        match self.next() % 7 {
            0 => 0.0,
            1 => -0.0,
            2 => 1.0,
            3 => lo,
            _ => self.range(lo, 1.0),
        }
    }
}

fn f(ee: &Ee, a: u32) -> F {
    F(ee.f32_at(a))
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

fn controller_of(ee: &Ee) -> Controller {
    let h = |o: u32| f(ee, HELI + o);
    let v = |o: u32| [h(o), h(o + 4), h(o + 8)];
    Controller {
        lateral_cyclic_gain: h(at::GAINS),
        longitudinal_cyclic_gain: h(at::GAINS + 4),
        pedal_gain: h(at::GAINS + 8),
        max_altitude: h(at::GAINS + 12),
        hold_delay: h(at::HOLD + 8),
        collective_gain: h(at::COLLECTIVE),
        climb_rate: h(at::COLLECTIVE + 4),
        descent_rate: h(at::COLLECTIVE + 8),
        altitude_hold_gain: h(at::COLLECTIVE + 12),
        integrator_gain: h(at::COLLECTIVE + 20),
        min_collective: h(at::COLLECTIVE + 24),
        max_pitch: h(at::PITCH_ROLL),
        max_roll: h(at::PITCH_ROLL + 4),
        pitch_gain: h(at::PITCH_ROLL + 8),
        roll_gain: h(at::PITCH_ROLL + 12),
        speed_command_rate: h(at::PITCH_ROLL + 20),
        min_speed_command: h(at::PITCH_ROLL + 24),
        max_speed_command: h(at::PITCH_ROLL + 28),
        speed_gain: h(at::PITCH_ROLL + 32),
        min_feed_forward_speed: h(at::FEED_FORWARD),
        min_feed_forward_scale: h(at::FEED_FORWARD + 4),
        max_feed_forward_speed: h(at::FEED_FORWARD + 8),
        max_feed_forward_scale: h(at::FEED_FORWARD + 12),
        min_rate: v(at::RATES),
        max_rate: v(at::RATES + 16),
    }
}

fn pad_of(ee: &Ee) -> Pad {
    let h = |o: u32| f(ee, HELI + at::PAD + o);
    Pad { climb: h(0), descend: h(4), lateral: h(12), longitudinal: h(16), pedal: h(20) }
}

fn commands_of(ee: &Ee) -> Commands {
    let h = |o: u32| f(ee, HELI + at::OUT + o);
    Commands {
        collective: h(0),
        lateral: h(4),
        longitudinal: h(8),
        pedal: h(12),
        filtered: h(16),
        altitude_fraction: f(ee, HELI + at::ALTITUDE_FRACTION),
    }
}

fn loop_of(ee: &Ee) -> Loop {
    let h = |o: u32| f(ee, HELI + o);
    let pf = ee.u32_at(HELI + at::FILTER);
    Loop {
        speed_command: h(at::PITCH_ROLL + 16),
        last_longitudinal: h(at::LAST_STICK),
        hold_altitude: h(at::HOLD),
        hold_timer: h(at::HOLD + 4),
        integral: h(at::COLLECTIVE + 16),
        filter: Filter { value: f(ee, pf), time: f(ee, pf + 4), step: f(ee, pf + 8) },
    }
}

fn flight_of(ee: &Ee) -> Flight {
    let h = |o: u32| f(ee, HELI + o);
    let body = ee.u32_at(HELI + at::BODY);
    let b = |o: u32| f(ee, body + o);
    Flight {
        flown: ee.u32_at(HELI + at::FLOWN) != 0,
        pilot_flag: ee.u32_at(ee.u32_at(HELI + at::PILOT) + 0x40) != 0,
        height: b(BODY_HEIGHT),
        velocity: [b(BODY_VEL), b(BODY_VEL + 4), b(BODY_VEL + 8)],
        rates: Vf::new(ee.f32_at(body + BODY_RATES), ee.f32_at(body + BODY_RATES + 4), ee.f32_at(body + BODY_RATES + 8), 0.0),
        airspeed: h(at::AIRSPEED),
        pitch: h(at::PITCH),
        roll: h(at::ROLL),
        gravity_scale: h(at::GRAVITY_SCALE),
        high_speed_aero: f(ee, ee.u32_at(HELI + at::AERO) + HIGH_SPEED_AERO),
    }
}

/// An angle: mostly a bank or a pitch, sometimes several turns out.
fn angle(pick: &mut Pick) -> f32 {
    match pick.next() % 8 {
        0 => 0.0,
        1 => pick.range(-20.0, 20.0),
        _ => pick.range(-1.2, 1.2),
    }
}

#[test]
fn the_flight_controller_matches_the_original() {
    let Some(mut ee) = heli() else { return };
    let func = addr("kiowa_flight_controller").unwrap();
    let table = the_curve_table(&ee);
    let body = ee.u32_at(HELI + at::BODY);
    let pilot = ee.u32_at(HELI + at::PILOT);
    let aero = ee.u32_at(HELI + at::AERO);
    let pf = ee.u32_at(HELI + at::FILTER);
    // The file's gains, each varied about the shipped value.
    let gains: Vec<(u32, f32)> = [(at::GAINS, 4), (at::HOLD + 8, 1), (at::COLLECTIVE, 4), (at::COLLECTIVE + 20, 2)]
        .into_iter()
        .chain([(at::PITCH_ROLL, 4), (at::PITCH_ROLL + 20, 4), (at::FEED_FORWARD, 4), (at::RATES, 3), (at::RATES + 16, 3)])
        .flat_map(|(o, n)| (0..n).map(move |k| HELI + o + 4 * k))
        .chain([aero + HIGH_SPEED_AERO])
        .map(|a| (a, ee.f32_at(a)))
        .collect();
    let mut pick = Pick(0x5148_0000_c0ff_ee01);
    let mut climbed = 0;
    let mut fed = 0;
    for case in 0..CASES {
        ee.set_u32(HELI + at::FLOWN, u32::from(!pick.one_in(30)));
        ee.set_u32(pilot + 0x40, u32::from(!pick.one_in(4)));
        for (k, lo) in [(0, 0.0), (4, 0.0), (12, -1.0), (16, -1.0), (20, -1.0)] {
            let v = pick.stick(lo);
            ee.set_f32(HELI + at::PAD + k, v);
        }
        for &(a, x) in &gains {
            let v = pick.around(x);
            ee.set_f32(a, v);
        }
        if pick.one_in(10) {
            // no feed-forward interpolation
            let s = ee.f32_at(HELI + at::FEED_FORWARD);
            ee.set_f32(HELI + at::FEED_FORWARD + 8, s);
        }
        if pick.one_in(3) {
            ee.set_f32(HELI + at::GAINS + 12, 550.0);
        }
        // The loop's state.
        let last = pick.stick(-1.0);
        ee.set_f32(HELI + at::LAST_STICK, last);
        let command = pick.range(-150.0, 150.0);
        ee.set_f32(HELI + at::PITCH_ROLL + 16, if pick.one_in(8) { 0.0 } else { command });
        let hold = pick.range(0.0, 600.0);
        ee.set_f32(HELI + at::HOLD, hold);
        let timer = if pick.one_in(3) { 0.0 } else { pick.range(-0.5, 2.0) };
        ee.set_f32(HELI + at::HOLD + 4, timer);
        let integral = pick.range(-0.5, 1.5);
        ee.set_f32(HELI + at::COLLECTIVE + 16, integral);
        let fv = pick.range(0.0, 1.0);
        ee.set_f32(pf, fv);
        let ft = if pick.one_in(4) { 1.0 } else { pick.range(0.01, 2.0) };
        ee.set_f32(pf + 4, ft);
        // The attitude.
        let height = if pick.one_in(4) { pick.range(500.0, 600.0) } else { pick.range(0.0, 600.0) };
        ee.set_f32(body + BODY_HEIGHT, height);
        let fast = pick.one_in(2);
        for k in 0..3 {
            let v = if fast { pick.range(-120.0, 120.0) } else { pick.range(-10.0, 10.0) };
            ee.set_f32(body + BODY_VEL + 4 * k, v);
            let r = pick.range(-2.0, 2.0);
            ee.set_f32(body + BODY_RATES + 4 * k, r);
        }
        let airspeed = if fast { pick.range(0.0, 150.0) } else { pick.range(0.0, 20.0) };
        ee.set_f32(HELI + at::AIRSPEED, airspeed);
        let pitch = angle(&mut pick);
        ee.set_f32(HELI + at::PITCH, pitch);
        let roll = angle(&mut pick);
        ee.set_f32(HELI + at::ROLL, roll);
        let g = pick.around(1.0);
        ee.set_f32(HELI + at::GRAVITY_SCALE, g);
        let dt = if pick.one_in(2) { 1.0 / 30.0 } else { pick.range(0.0, 0.1) };

        let (pad, at, c) = (pad_of(&ee), flight_of(&ee), controller_of(&ee));
        let (mut state, mut out) = (loop_of(&ee), commands_of(&ee));
        if at.flown && pad.climb != 0.0 {
            climbed += 1;
        }
        if at.flown && c.min_feed_forward_speed < at.airspeed {
            fed += 1;
        }
        ee.call(func, &[u64::from(HELI)], &[dt], BUDGET);
        c.step(&pad, &at, &mut state, &mut out, &table, F(dt));

        let want = commands_of(&ee);
        for (name, got, want) in [
            ("the collective", out.collective, want.collective),
            ("the lateral cyclic", out.lateral, want.lateral),
            ("the longitudinal cyclic", out.longitudinal, want.longitudinal),
            ("the pedal", out.pedal, want.pedal),
            ("the filtered collective", out.filtered, want.filtered),
            ("the altitude fraction", out.altitude_fraction, want.altitude_fraction),
        ] {
            same(name, case, got, want.to_f32());
        }
        let want = loop_of(&ee);
        for (name, got, want) in [
            ("the speed command", state.speed_command, want.speed_command),
            ("the last stick", state.last_longitudinal, want.last_longitudinal),
            ("the held height", state.hold_altitude, want.hold_altitude),
            ("the hold timer", state.hold_timer, want.hold_timer),
            ("the integral", state.integral, want.integral),
            ("the filter", state.filter.value, want.filter.value),
            ("the filter's step", state.filter.step, want.filter.step),
        ] {
            same(name, case, got, want.to_f32());
        }
    }
    assert!(climbed > CASES / 10 && fed > CASES / 10, "the cases missed a branch: {climbed} climbing, {fed} fed forward");
    println!("the flight controller: {CASES} cases, bit-exact ({climbed} climbing, {fed} with the turn feed-forward)");
}

// --- S220: the rotor, the aerodynamics and the listeners ---------------------------------------------

const ROTOR: u32 = 0x450;
const WRECKED: u32 = 0x3d4;
const SPEED: u32 = 0x470; // speed, alpha, beta
const BODY_POS: u32 = 0x60;
const BODY_SHOCK_PEAK: u32 = 0xb0;
const BODY_VTABLE: u32 = 0x100;
const BODY_QUAT: u32 = 0x170;
const FORCE_SLOT: u32 = 0x58;
const MOMENT_SLOT: u32 = 0x5c;

/// The rotor model's fields, by offset.
mod rotor_at {
    pub const MAX_PITCH: u32 = 0x48; // lateral cyclic, longitudinal cyclic, collective, tail
    pub const BLADE: u32 = 0x58; // lift curve slope, chord, blades, radius, radius^2, angular rate
    pub const MAX_MOMENT: u32 = 0xc0;
    pub const PITCH: u32 = 0xd0; // collective, tail
    pub const TILT: u32 = 0xe0;
    pub const THRUST: u32 = 0x100;
    pub const FORCE: u32 = 0x110;
}

/// The aerodynamics model's fields the step reads, by offset, in [`Aero`]'s order.
const AERO_KEYS: [u32; 20] =
    [0x30, 0x34, 0x38, 0x3c, 0x48, 0x60, 0x68, 0x74, 0x78, 0x80, 0x84, 0x88, 0x8c, 0x90, 0x94, 0x9c, 0xa0, 0xf4, 0xfc, 0x100];
const AERO_OUT: u32 = 0x128; // fade, lift, drag, side, roll, pitch, yaw

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

fn set_quad(ee: &mut Ee, a: u32, v: [f32; 4]) {
    for (k, x) in v.into_iter().enumerate() {
        ee.set_f32(a + 4 * k as u32, x);
    }
}

fn rotor_of(ee: &Ee, r: u32) -> Rotor {
    let g = |o: u32| f(ee, r + o);
    Rotor {
        max_lateral_cyclic: g(rotor_at::MAX_PITCH),
        max_longitudinal_cyclic: g(rotor_at::MAX_PITCH + 4),
        max_collective: g(rotor_at::MAX_PITCH + 8),
        max_tail_pitch: g(rotor_at::MAX_PITCH + 12),
        lift_curve_slope: g(rotor_at::BLADE),
        blade_chord: g(rotor_at::BLADE + 4),
        blades: g(rotor_at::BLADE + 8),
        radius: g(rotor_at::BLADE + 12),
        radius_squared: g(rotor_at::BLADE + 16),
        angular_rate: g(rotor_at::BLADE + 20),
        max_moment: [g(rotor_at::MAX_MOMENT), g(rotor_at::MAX_MOMENT + 4), g(rotor_at::MAX_MOMENT + 8)],
    }
}

fn rotor_state_of(ee: &Ee, r: u32) -> RotorState {
    let g = |o: u32| f(ee, r + o);
    RotorState {
        collective_pitch: g(rotor_at::PITCH),
        tail_pitch: g(rotor_at::PITCH + 4),
        tilt: [g(rotor_at::TILT), g(rotor_at::TILT + 4), g(rotor_at::TILT + 8)],
        thrust: quad(ee, r + rotor_at::THRUST),
        force: quad(ee, r + rotor_at::FORCE),
    }
}

fn aero_of(ee: &Ee, a: u32) -> Aero {
    let k = AERO_KEYS.map(|o| f(ee, a + o));
    Aero {
        density: k[0],
        chord: k[1],
        span: k[2],
        area: k[3],
        cl_alpha: k[4],
        cd0: k[5],
        cy_beta: k[6],
        cl_beta: k[7],
        cl_p: k[8],
        cl_da: k[9],
        cm0: k[10],
        cm_alpha: k[11],
        cm_q: k[12],
        cm_de: k[13],
        cn_beta: k[14],
        cn_r: k[15],
        cn_dr: k[16],
        low_speed_drag: k[17],
        low_speed_aero: k[18],
        high_speed_aero: k[19],
    }
}

fn aero_state_of(ee: &Ee, a: u32) -> AeroState {
    let g = |o: u32| f(ee, a + AERO_OUT + o);
    AeroState { fade: g(0), lift: g(4), drag: g(8), side: g(12), roll: g(16), pitch: g(20), yaw: g(24) }
}

fn readings_of(ee: &Ee) -> Readings {
    let at = flight_of(ee);
    let h = |o: u32| f(ee, HELI + o);
    Readings {
        flown: at.flown,
        pilot_flag: at.pilot_flag,
        airspeed: at.airspeed,
        pitch: at.pitch,
        roll: at.roll,
        gravity_scale: at.gravity_scale,
        speed: h(SPEED),
        alpha: h(SPEED + 4),
        beta: h(SPEED + 8),
    }
}

/// The body's force and moment entries, hooked to record what they are handed.
fn record_body(ee: &mut Ee, body: u32) -> Rc<RefCell<Vec<[f32; 4]>>> {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let vtable = ee.u32_at(body + BODY_VTABLE);
    for slot in [FORCE_SLOT, MOMENT_SLOT] {
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

fn same_lanes(what: &str, case: usize, got: Vf, want: [f32; 4], lanes: usize) {
    for k in 0..lanes {
        assert_eq!(got[k].to_bits(), want[k].to_bits(),
                   "case {case}: {what}[{k}] is {:e} in the game, {:e} in the port ({want:?} / {got:?})", want[k], got[k]);
    }
}

/// A unit quaternion, sometimes the identity or a pure turn about the vertical.
fn orientation(pick: &mut Pick) -> [f32; 4] {
    match pick.next() % 6 {
        0 => [0.0, 0.0, 0.0, 1.0],
        1 => {
            let a = pick.range(-3.2, 3.2);
            [0.0, (0.5 * a).sin(), 0.0, (0.5 * a).cos()]
        }
        _ => {
            let q = [0; 4].map(|_| pick.range(-1.0, 1.0));
            let n = q.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-3);
            q.map(|x| x / n)
        }
    }
}

#[test]
fn the_tilts_quaternion_is_the_originals() {
    let Some(mut ee) = heli() else { return };
    let func = addr("euler_to_quat").unwrap();
    let table = the_curve_table(&ee);
    let (out, angles) = (0x0100_0000u32, 0x0100_0040u32);
    let mut pick = Pick(0x5220_0000_0e1e_0001);
    for case in 0..CASES {
        let a = [0; 3].map(|_| angle(&mut pick));
        set_quad(&mut ee, angles, [a[0], a[1], a[2], 0.0]);
        ee.call(func, &[u64::from(out), u64::from(angles)], &[], BUDGET);
        let got = euler_quat(a.map(F), &table);
        same_lanes("the quaternion", case, got, [0, 4, 8, 12].map(|k| ee.f32_at(out + k)), 4);
    }
    println!("the tilt's quaternion: {CASES} cases, bit-exact");
}

#[test]
fn the_rotor_is_the_originals() {
    let Some(mut ee) = heli() else { return };
    let func = addr("kiowa_rotor_step").unwrap();
    let table = the_curve_table(&ee);
    let r = ee.u32_at(HELI + ROTOR);
    let body = ee.u32_at(HELI + at::BODY);
    let seen = record_body(&mut ee, body);
    let params: Vec<(u32, f32)> = (0..4)
        .map(|k| r + rotor_at::MAX_PITCH + 4 * k)
        .chain((0..6).map(|k| r + rotor_at::BLADE + 4 * k))
        .chain((0..3).map(|k| r + rotor_at::MAX_MOMENT + 4 * k))
        .map(|a| (a, ee.f32_at(a)))
        .collect();
    let mut pick = Pick(0x5220_0000_0707_0002);
    for case in 0..CASES {
        for &(a, x) in &params {
            let v = pick.around(x);
            ee.set_f32(a, v);
        }
        for k in 0..4 {
            let v = pick.stick(if k == 0 { 0.0 } else { -1.0 });
            let v = if pick.one_in(10) { pick.range(-3.0, 3.0) } else { v };
            ee.set_f32(HELI + at::OUT + 4 * k, v);
        }
        // What the rotor kept: only the thrust's untouched lanes carry over into this step.
        for k in 0..4 {
            let v = pick.range(-50.0, 50.0);
            ee.set_f32(r + rotor_at::PITCH + 4 * k, v);
        }
        let t = [pick.range(-1.0, 1.0), pick.range(-1.0, 1.0), pick.range(-1.0, 1.0), 1.0];
        set_quad(&mut ee, r + rotor_at::THRUST, if pick.one_in(2) { [0.0, 0.0, 0.0, 1.0] } else { t });
        for k in 0..3 {
            let v = pick.range(-2.0, 2.0);
            ee.set_f32(body + BODY_RATES + 4 * k, v);
        }
        let (rotor, c, mut state) = (rotor_of(&ee, r), commands_of(&ee), rotor_state_of(&ee, r));
        seen.borrow_mut().clear();
        ee.call(func, &[u64::from(r)], &[1.0 / 120.0], BUDGET);
        let (force, moment) = rotor.step(&c, &mut state, &table);

        let want = rotor_state_of(&ee, r);
        same("the collective pitch", case, state.collective_pitch, want.collective_pitch.to_f32());
        same("the tail pitch", case, state.tail_pitch, want.tail_pitch.to_f32());
        for k in 0..3 {
            same("the tilt", case, state.tilt[k], want.tilt[k].to_f32());
        }
        same_lanes("the thrust", case, state.thrust, [0, 1, 2, 3].map(|k| want.thrust[k]), 4);
        same_lanes("the tilted thrust", case, state.force, [0, 1, 2, 3].map(|k| want.force[k]), 4);
        let seen = seen.borrow();
        assert_eq!(seen.len(), 2, "case {case}: the rotor handed the body {} vectors", seen.len());
        same_lanes("the force", case, force, seen[0], 4);
        same_lanes("the moment", case, moment, seen[1], 3);
    }
    println!("the rotor: {CASES} cases, bit-exact");
}

#[test]
fn the_aerodynamics_are_the_originals() {
    let Some(mut ee) = heli() else { return };
    let func = addr("kiowa_aero_step").unwrap();
    let a = ee.u32_at(HELI + at::AERO);
    let body = ee.u32_at(HELI + at::BODY);
    let seen = record_body(&mut ee, body);
    let params: Vec<(u32, f32)> = AERO_KEYS.iter().map(|&o| (a + o, ee.f32_at(a + o))).collect();
    let (lo, hi) = (ee.f32_at(a + 0xfc), ee.f32_at(a + 0x100));
    let mut pick = Pick(0x5220_0000_0a1e_0003);
    let (mut faded, mut slow_drag, mut back_drag, mut still) = (0, 0, 0, 0);
    for case in 0..CASES {
        for &(at, x) in &params {
            let v = pick.around(x);
            ee.set_f32(at, v);
        }
        if pick.one_in(2) {
            ee.set_f32(a + 0xfc, lo);
            ee.set_f32(a + 0x100, hi);
        }
        if pick.one_in(20) {
            let v = ee.f32_at(a + 0xfc);
            ee.set_f32(a + 0x100, v);
        }
        for k in [0x340, 0x344, 0x348] {
            let v = match pick.next() % 6 {
                0 => 0.3 * if pick.one_in(2) { 1.0 } else { -1.0 },
                1 => pick.range(-0.35, 0.35),
                _ => pick.stick(-1.0),
            };
            ee.set_f32(HELI + k, v);
        }
        let fast = match pick.next() % 4 {
            0 => 1.5,
            1 => 25.0,
            _ => 80.0,
        };
        let speed = match pick.next() % 20 {
            0 => 1.0,
            1 => -1.0,
            2 | 3 => pick.range(-1.5, 1.5),
            _ => pick.range(-0.2 * fast, fast),
        };
        ee.set_f32(HELI + SPEED, speed);
        let alpha = pick.range(-1.6, 1.6);
        ee.set_f32(HELI + SPEED + 4, alpha);
        let beta = if pick.one_in(2) { pick.range(-0.5, 0.5) } else { pick.range(-3.2, 3.2) };
        ee.set_f32(HELI + SPEED + 8, beta);
        let v = [pick.range(-fast, fast), pick.range(-20.0, 20.0), pick.range(-fast, fast), pick.range(-1.0, 1.0)];
        set_quad(&mut ee, body + BODY_VEL, v);
        let q = orientation(&mut pick);
        set_quad(&mut ee, body + BODY_QUAT, q);
        for k in 0..3 {
            let v = pick.range(-3.0, 3.0);
            ee.set_f32(body + BODY_RATES + 4 * k, v);
        }

        let (aero, pad, mut state) = (aero_of(&ee, a), pad_of(&ee), aero_state_of(&ee, a));
        let air = Airflow {
            speed: F(speed),
            alpha: F(alpha),
            beta: F(beta),
            velocity: quad(&ee, body + BODY_VEL),
            quat: quad(&ee, body + BODY_QUAT),
            rates: quad(&ee, body + BODY_RATES),
        };
        seen.borrow_mut().clear();
        ee.call(func, &[u64::from(a)], &[1.0 / 120.0], BUDGET);
        let (force, moment) = aero.step(&pad, &air, &mut state);

        let want = aero_state_of(&ee, a);
        for (name, got, want) in [
            ("the fade", state.fade, want.fade),
            ("the lift", state.lift, want.lift),
            ("the drag", state.drag, want.drag),
            ("the side force", state.side, want.side),
            ("the roll", state.roll, want.roll),
            ("the pitch", state.pitch, want.pitch),
            ("the yaw", state.yaw, want.yaw),
        ] {
            same(name, case, got, want.to_f32());
        }
        let seen = seen.borrow();
        assert_eq!(seen.len(), 2, "case {case}: the aerodynamics handed the body {} vectors", seen.len());
        same_lanes("the force", case, force, seen[0], 4);
        same_lanes("the moment", case, moment, seen[1], 4);

        faded += usize::from(state.fade > 0.0 && state.fade < 1.0);
        still += usize::from(speed.abs() <= 1.0);
        let stick = pad.longitudinal.to_f32();
        if speed.abs() < aero.high_speed_aero.to_f32() && stick < 0.3 {
            slow_drag += 1;
        } else if stick <= -0.3 {
            back_drag += 1;
        }
    }
    assert!(faded > CASES / 20 && slow_drag > CASES / 20 && back_drag > CASES / 50 && still > CASES / 50,
            "the cases missed a branch: {faded} faded, {slow_drag} slow, {back_drag} pulled back, {still} still");
    println!("the aerodynamics: {CASES} cases, bit-exact ({faded} faded, {slow_drag} slow, {back_drag} pulled back, {still} still)");
}

#[test]
fn the_force_listener_is_the_originals() {
    let Some(mut ee) = heli() else { return };
    let func = addr("kiowa_forces").unwrap();
    let table = the_curve_table(&ee);
    let (r, a) = (ee.u32_at(HELI + ROTOR), ee.u32_at(HELI + at::AERO));
    let body = ee.u32_at(HELI + at::BODY);
    let seen = record_body(&mut ee, body);
    let wrapped = Rc::new(RefCell::new(0usize));
    let w = wrapped.clone();
    ee.hooks.insert(addr("body_wrapper_force").unwrap(), Box::new(move |_| *w.borrow_mut() += 1));
    let pf = ee.u32_at(HELI + at::FILTER);
    let mut pick = Pick(0x5220_0000_f0ce_0004);
    let mut wrecks = 0;
    for case in 0..CASES {
        let wrecked = pick.one_in(8);
        wrecks += usize::from(wrecked);
        ee.set_u32(HELI + WRECKED, u32::from(wrecked));
        ee.set_u32(HELI + at::FLOWN, u32::from(!pick.one_in(30)));
        for (k, lo) in [(0, 0.0), (4, 0.0), (12, -1.0), (16, -1.0), (20, -1.0)] {
            let v = pick.stick(lo);
            ee.set_f32(HELI + at::PAD + k, v);
        }
        let hold = pick.range(0.0, 600.0);
        ee.set_f32(HELI + at::HOLD, hold);
        let timer = if pick.one_in(3) { 0.0 } else { pick.range(-0.5, 2.0) };
        ee.set_f32(HELI + at::HOLD + 4, timer);
        let fv = pick.range(0.0, 1.0);
        ee.set_f32(pf, fv);
        let height = pick.range(0.0, 600.0);
        ee.set_f32(body + BODY_HEIGHT, height);
        let fast = if pick.one_in(2) { 120.0 } else { 10.0 };
        let v = [pick.range(-fast, fast), pick.range(-20.0, 20.0), pick.range(-fast, fast), 0.0];
        set_quad(&mut ee, body + BODY_VEL, v);
        let q = orientation(&mut pick);
        set_quad(&mut ee, body + BODY_QUAT, q);
        for k in 0..3 {
            let v = pick.range(-2.0, 2.0);
            ee.set_f32(body + BODY_RATES + 4 * k, v);
        }
        let airspeed = pick.range(0.0, fast);
        ee.set_f32(HELI + at::AIRSPEED, airspeed);
        let pitch = angle(&mut pick);
        ee.set_f32(HELI + at::PITCH, pitch);
        let roll = angle(&mut pick);
        ee.set_f32(HELI + at::ROLL, roll);
        let speed = pick.range(-0.2 * fast, fast);
        ee.set_f32(HELI + SPEED, speed);
        let alpha = pick.range(-1.6, 1.6);
        ee.set_f32(HELI + SPEED + 4, alpha);
        let beta = pick.range(-0.5, 0.5);
        ee.set_f32(HELI + SPEED + 8, beta);
        let h = if pick.one_in(2) { 1.0 / 120.0 } else { pick.range(0.0, 0.05) };

        let mut kiowa = Kiowa {
            controller: controller_of(&ee),
            rotor: rotor_of(&ee, r),
            aero: aero_of(&ee, a),
            pad: pad_of(&ee),
            commands: commands_of(&ee),
            control: loop_of(&ee),
            rotor_state: rotor_state_of(&ee, r),
            aero_state: aero_state_of(&ee, a),
            wrecked,
        };
        let readings = readings_of(&ee);
        let start = Body {
            position: quad(&ee, body + BODY_POS),
            velocity: quad(&ee, body + BODY_VEL),
            quat: quad(&ee, body + BODY_QUAT),
            rates: quad(&ee, body + BODY_RATES),
            ..Body::default()
        };
        seen.borrow_mut().clear();
        *wrapped.borrow_mut() = 0;
        ee.call(func, &[u64::from(HELI)], &[h], BUDGET);
        let mut port = start;
        let mut ran = 0;
        kiowa.force(&readings, &mut port, &table, F(h), |_| ran += 1);

        assert_eq!((ran, *wrapped.borrow()), (1, 1), "case {case}: the wrapper's force ran a different number of times");
        let want = commands_of(&ee);
        for (name, got, want) in [
            ("the collective", kiowa.commands.collective, want.collective),
            ("the lateral cyclic", kiowa.commands.lateral, want.lateral),
            ("the longitudinal cyclic", kiowa.commands.longitudinal, want.longitudinal),
            ("the pedal", kiowa.commands.pedal, want.pedal),
            ("the filtered collective", kiowa.commands.filtered, want.filtered),
        ] {
            same(name, case, got, want.to_f32());
        }
        assert_eq!(kiowa.rotor_state, rotor_state_of(&ee, r), "case {case}: the rotor's state");
        assert_eq!(kiowa.aero_state, aero_state_of(&ee, a), "case {case}: the aerodynamics' state");
        // The game's body took these vectors; the same, handed to the port's body, must leave it as
        // the port's listener did.
        let seen = seen.borrow();
        assert_eq!(seen.len(), if wrecked { 0 } else { 4 }, "case {case}: {} vectors handed to the body", seen.len());
        let mut want = start;
        for (k, v) in seen.iter().enumerate() {
            let v = Vf::new(v[0], v[1], v[2], v[3]);
            if k % 2 == 0 { want.add_force_local(v) } else { want.add_moment_local(v) }
        }
        same_lanes("the body's force", case, port.force, [0, 1, 2, 3].map(|k| want.force[k]), 4);
        same_lanes("the body's moment", case, port.moment, [0, 1, 2, 3].map(|k| want.moment[k]), 3);
    }
    assert!(wrecks > CASES / 20, "only {wrecks} wrecked cases");
    println!("the force listener: {CASES} cases, bit-exact ({wrecks} wrecked)");
}

#[test]
fn the_preforce_listener_is_the_originals() {
    let Some(mut ee) = heli() else { return };
    let func = addr("kiowa_preforce").unwrap();
    let body = ee.u32_at(HELI + at::BODY);
    set_quad(&mut ee, body + BODY_SHOCK_PEAK, [3.0, -4.0, 5.0, 6.0]);
    ee.call(func, &[u64::from(HELI)], &[1.0 / 30.0], BUDGET);
    let mut port = Body { shock_peak: Vf::new(3.0, -4.0, 5.0, 6.0), ..Body::default() };
    Kiowa::default().preforce(&mut port);
    same_lanes("the shock peak", 0, port.shock_peak, [0, 4, 8, 12].map(|k| ee.f32_at(body + BODY_SHOCK_PEAK + k)), 4);
}
