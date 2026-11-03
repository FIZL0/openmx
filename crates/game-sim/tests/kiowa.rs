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

// --- S220a: the measure listener, its crash test and the MX rotor's spin --------------------------------

const VTABLE: u32 = 0x48;
const CRASH_SLOT: u32 = 0x1b4;
const WRECK_SLOT: u32 = 0x1b8;
const DEBRIS_SLOT: u32 = 0x1b0;
const RESET_SLOT: u32 = 0x198;
const BODY_COFG: u32 = 0x180;

/// The helicopter's measure and crash fields, by offset (research/oracle.txt S220a).
mod measure_at {
    pub const WORLD: u32 = 0x150;
    pub const THROW_SCALE: u32 = 0x194;
    pub const DOWN: u32 = 0x3dc;
    pub const MIN_WRECK_SPEED: u32 = 0x3e0;
    pub const CLOCK: u32 = 0x3e4;
    pub const WRECK_TIME: u32 = 0x3e8;
    pub const WAS_TOUCHING: u32 = 0x420;
    pub const ROTOR_COLLISION: u32 = 0x458;
    pub const SCENE: u32 = 0x45c;
    pub const SURFACE_MASK: u32 = 0x460;
    pub const STRUCK_MASK: u32 = 0x464;
    pub const SOUND: u32 = 0x54c;
    pub const ROWS: [(&str, u32); 9] = [
        ("the world rates", 0x480),
        ("the body velocity", 0x490),
        ("the forward row", 0x4a0),
        ("the up row", 0x4b0),
        ("the Euler angles", 0x4c0),
        ("the matrix row 0", 0x4d0),
        ("the matrix row 1", 0x4e0),
        ("the matrix row 2", 0x4f0),
        ("the matrix row 3", 0x500),
    ];
    pub const MATRIX: u32 = 0x4d0;
    /// In the collision record (heli +0x170).
    pub const TOUCHING: u32 = 0x40;
    pub const ENTRIES: u32 = 0x100;
    pub const ENTRY_ARRAY: u32 = 0x108;
    pub const SURFACE: u32 = 0x1fc;
    pub const FLAGS: u32 = 0x84;
    /// In the rotor's collision object.
    pub const SPHERES: u32 = 0x170;
    pub const SPHERE_COUNT: u32 = 0x168;
    pub const CONTACT_BYTE: u32 = 0x116;
    pub const CONTACTS: u32 = 0x6c;
    /// The terrain, from the scene; its four extent floats.
    pub const TERRAIN: u32 = 0x3b4;
    pub const EXTENT: u32 = 0x530;
    /// MX: the rotor's node, the wrecked blade's, the rotor's angle.
    pub const ROTOR_NODE: u32 = 0xd1c;
    pub const BLADE_NODE: u32 = 0xd40;
    pub const ROTOR_ANGLE: u32 = 0xe58;
}

const SCRATCH: u32 = 0x0100_0000;

fn put(ee: &mut Ee, a: u32, v: Vf) {
    set_quad(ee, a, [v.x(), v.y(), v.z(), v.w()]);
}

/// One bit of the byte at `a`, through the word it is in.
fn set_bit(ee: &mut Ee, a: u32, bit: u8, on: bool) {
    let (word, shift) = (a & !3, 8 * (a & 3));
    let mask = u32::from(bit) << shift;
    let v = ee.u32_at(word) & !mask;
    ee.set_u32(word, v | if on { mask } else { 0 });
}

/// A vector of the given size, its `w` 1.
fn vector(pick: &mut Pick, size: f32) -> Vf {
    Vf::new(pick.range(-size, size), pick.range(-size, size), pick.range(-size, size), 1.0)
}

/// `v` turned by the unit quaternion `q` in host floats: only to aim a generated velocity.
fn host_turn(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let [x, y, z, w] = q;
    let t = [2.0 * (y * v[2] - z * v[1]), 2.0 * (z * v[0] - x * v[2]), 2.0 * (x * v[1] - y * v[0])];
    [
        v[0] + w * t[0] + (y * t[2] - z * t[1]),
        v[1] + w * t[1] + (z * t[0] - x * t[2]),
        v[2] + w * t[2] + (x * t[1] - y * t[0]),
    ]
}

#[test]
fn the_measure_is_the_originals() {
    use game_sim::air::AcosTable;
    use game_sim::kiowa::{Measure, Touch};
    let Some(mut ee) = heli() else { return };
    let func = addr("kiowa_measure").unwrap();
    let table = AcosTable::build();
    let vt = ee.u32_at(HELI + VTABLE);
    // The body wrapper's own measure and the crash test are not the measure's figures: they do nothing
    // here, so the body the measure reads is the one written. The touchdown's sound is counted.
    for a in [addr("body_wrapper_post").unwrap(), ee.u32_at(vt + CRASH_SLOT), addr("sound_trigger").unwrap()] {
        ee.hooks.insert(a, Box::new(|_| {}));
    }
    let sounded = Rc::new(RefCell::new(0));
    let s = sounded.clone();
    ee.hooks.insert(addr("touchdown_sound").unwrap(), Box::new(move |_| *s.borrow_mut() += 1));
    let body = ee.u32_at(HELI + at::BODY);
    let record = ee.u32_at(HELI + at::PILOT);
    let sound = ee.u32_at(HELI + measure_at::SOUND);
    ee.set_u32(record + measure_at::ENTRY_ARRAY, SCRATCH);
    let mut pick = Pick(0x5220_a000_3ea5_0001);
    let (mut kept, mut flipped, mut downs) = (0, 0, 0);
    for case in 0..CASES {
        let q = orientation(&mut pick);
        let tail = pick.one_in(3);
        let abeam = !tail && pick.one_in(8);
        let q = if abeam { [0.0, 0.0, 0.0, 1.0] } else { q };
        let v = if abeam {
            // straight out to the side: beta exactly at the table's middle, either side
            Vf::new(if pick.one_in(2) { 30.0 } else { -30.0 }, if pick.one_in(2) { 0.0 } else { pick.range(-5.0, 5.0) }, 0.0, 1.0)
        } else if tail {
            // flying backwards, nearly straight: beta about pi, either side
            let back = [pick.range(-3.0, 3.0), pick.range(-3.0, 3.0), -pick.range(5.0, 80.0)];
            let w = host_turn(q, back);
            Vf::new(w[0], w[1], w[2], 1.0)
        } else {
            match pick.next() % 6 {
                0 => Vf::new(0.0, 0.0, 0.0, 1.0),
                1 => vector(&mut pick, 1e-3),
                _ => vector(&mut pick, 150.0),
            }
        };
        let port_body = Body {
            position: vector(&mut pick, 4000.0),
            velocity: v,
            quat: Vf::new(q[0], q[1], q[2], q[3]),
            rates: Vf::new(pick.range(-3.0, 3.0), pick.range(-3.0, 3.0), pick.range(-3.0, 3.0), pick.range(-1.0, 1.0)),
            cofg: if pick.one_in(3) { Vf::new(0.0, 5.5, 0.0, 1.0) } else { vector(&mut pick, 8.0) },
            ..Body::default()
        };
        put(&mut ee, body + BODY_POS, port_body.position);
        put(&mut ee, body + BODY_VEL, port_body.velocity);
        put(&mut ee, body + BODY_QUAT, port_body.quat);
        put(&mut ee, body + BODY_RATES, port_body.rates);
        put(&mut ee, body + BODY_COFG, port_body.cofg);
        let old = match pick.next() % 4 {
            0 => 0.0,
            1 => pick.range(-3.1415927, 3.1415927),
            _ => (if pick.one_in(2) { 1.0 } else { -1.0 }) * pick.range(2.9, 3.1415927),
        };
        let old = if abeam { (if pick.one_in(2) { 1.0 } else { -1.0 }) * pick.range(1.5, 1.65) } else { old };
        ee.set_f32(HELI + SPEED + 8, old);
        let was = pick.one_in(2);
        ee.set_u32(HELI + measure_at::WAS_TOUCHING, u32::from(was));
        let touching = !pick.one_in(3);
        ee.set_u32(record + measure_at::TOUCHING, u32::from(touching));
        let entries: Vec<Touch> = (0..pick.next() % 5)
            .map(|_| Touch {
                kind: if pick.one_in(3) { 1 } else { 0 },
                t: F(match pick.next() % 4 {
                    0 => 1.0,
                    _ => pick.range(0.0, 2.0),
                }),
            })
            .collect();
        ee.set_u32(record + measure_at::ENTRIES, entries.len() as u32);
        for (k, e) in entries.iter().enumerate() {
            ee.set_u32(SCRATCH + 0x80 * k as u32, e.kind);
            ee.set_f32(SCRATCH + 0x80 * k as u32 + 0x24, e.t.to_f32());
        }
        let has_sound = !pick.one_in(4);
        ee.set_u32(HELI + measure_at::SOUND, if has_sound { sound } else { 0 });
        *sounded.borrow_mut() = 0;

        let mut port = Measure {
            beta: F(old),
            forward: quad(&ee, HELI + 0x4a0),
            up: quad(&ee, HELI + 0x4b0),
            was_touching: was,
            ..Measure::default()
        };
        ee.call(func, &[u64::from(HELI)], &[1.0 / 30.0], BUDGET);
        port.flight(&port_body, &table);
        let down = port.touchdown(touching, entries.iter().copied());

        same("the speed", case, port.speed, ee.f32_at(HELI + SPEED));
        same("alpha", case, port.alpha, ee.f32_at(HELI + SPEED + 4));
        same("beta", case, port.beta, ee.f32_at(HELI + SPEED + 8));
        let got = [port.world_rates, port.velocity, port.forward, port.up, port.euler]
            .into_iter()
            .chain(port.matrix.0);
        for ((name, o), got) in measure_at::ROWS.into_iter().zip(got) {
            same_lanes(name, case, got, [0, 4, 8, 12].map(|k| ee.f32_at(HELI + o + k)), 4);
        }
        assert_eq!(ee.u32_at(HELI + measure_at::WAS_TOUCHING), u32::from(port.was_touching), "case {case}: the touching flag");
        assert_eq!(*sounded.borrow(), u32::from(down && has_sound), "case {case}: the touchdown sound");
        // and what the force listener reads is what the measure left
        assert_eq!(port.readings(ee.u32_at(HELI + at::FLOWN) != 0, touching, f(&ee, HELI + at::GRAVITY_SCALE)),
                   readings_of(&ee), "case {case}: the readings");
        if port.beta.to_f32() * old < 0.0 {
            flipped += 1;
        }
        if port.beta.to_bits() == old.to_bits() && old != 0.0 && tail {
            kept += 1;
        }
        downs += usize::from(down);
    }
    assert!(kept > CASES / 50 && flipped > CASES / 50 && downs > CASES / 50,
            "{kept} kept betas, {flipped} flipped, {downs} touchdowns: the cases miss a branch");
    println!("the measure: {CASES} cases, bit-exact ({kept} betas kept, {flipped} flipped, {downs} touchdowns)");
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Asked {
    Wreck(u32),
    Debris,
    Reset,
}

#[test]
fn the_crash_test_is_the_originals() {
    use game_sim::biplane::Area;
    use game_sim::kiowa::{Crash, CrashContact, Strike, rotor_in_ground};
    use game_sim::math::Mat4;
    use game_sim::rng::Rng;
    let Some(mut ee) = heli() else { return };
    let func = addr("kiowa_wreck_check").unwrap();
    let state = addr("rand_state").unwrap();
    let vt = ee.u32_at(HELI + VTABLE);
    let asked = Rc::new(RefCell::new(Vec::new()));
    let a = asked.clone();
    ee.hooks.insert(ee.u32_at(vt + WRECK_SLOT), Box::new(move |ee| {
        let (heli, reason) = (ee.r(4) as u32, ee.r(5) as u32);
        ee.set_u32(heli + WRECKED, reason);
        ee.set_u32(heli + measure_at::DOWN, 0);
        a.borrow_mut().push(Asked::Wreck(reason));
    }));
    let a = asked.clone();
    ee.hooks.insert(ee.u32_at(vt + DEBRIS_SLOT), Box::new(move |_| a.borrow_mut().push(Asked::Debris)));
    let a = asked.clone();
    ee.hooks.insert(ee.u32_at(vt + RESET_SLOT), Box::new(move |_| a.borrow_mut().push(Asked::Reset)));
    // The ground answers each point with this case's height, and says what it was asked.
    let ground = Rc::new(RefCell::new((0.0f32, Vec::<[f32; 4]>::new())));
    let g = ground.clone();
    ee.hooks.insert(addr("ground_query").unwrap(), Box::new(move |ee| {
        let p = ee.r(5) as u32;
        let mut g = g.borrow_mut();
        g.1.push([0, 4, 8, 12].map(|k| ee.f32_at(p + k)));
        ee.set_f32(p + 4, g.0);
    }));
    // The rotor's contact search is not the test's: its answer is written below.
    let collision = ee.u32_at(HELI + measure_at::ROTOR_COLLISION);
    ee.hooks.insert(ee.u32_at(ee.u32_at(collision + 0x38) + 0xac), Box::new(|_| {}));
    ee.hooks.insert(addr("collide_against_search").unwrap(), Box::new(|_| {}));
    let (contacts, list, struck_at, surface_at) = (SCRATCH, SCRATCH + 0x100, SCRATCH + 0x200, SCRATCH + 0x300);
    ee.set_u32(collision + measure_at::CONTACTS, contacts);
    ee.set_u32(contacts + 0x9c, list);
    let world = ee.u32_at(HELI + measure_at::WORLD);
    let record = ee.u32_at(HELI + at::PILOT);
    let body = ee.u32_at(HELI + at::BODY);
    let terrain = ee.u32_at(ee.u32_at(HELI + measure_at::SCENE) + measure_at::TERRAIN);
    let shipped_extent: Vec<f32> = (0..4).map(|k| ee.f32_at(terrain + measure_at::EXTENT + 4 * k)).collect();
    let spheres = ee.u32_at(collision + measure_at::SPHERES);
    let centres: Vec<Vf> = (0..ee.u32_at(collision + measure_at::SPHERE_COUNT)).map(|k| quad(&ee, spheres + 0x20 * k + 0x10)).collect();
    let (surface_mask, struck_mask) = (ee.u32_at(HELI + measure_at::SURFACE_MASK), ee.u32_at(HELI + measure_at::STRUCK_MASK));
    let flags = |pick: &mut Pick, mask: u32| if pick.one_in(2) { mask } else { 1u32 << (pick.next() % 32) & !mask };
    let mut pick = Pick(0x5220_a000_c4a5_0001);
    let (mut thrown, mut crashed, mut grounded, mut recovered, mut debris) = (0, 0, 0, 0, 0);
    for case in 0..CASES {
        let extent: Vec<f32> = match pick.next() % 3 {
            0 => vec![3.0, 3.0, pick.range(5.0, 12.0), pick.range(5.0, 12.0)],
            _ => shipped_extent.iter().map(|&x| x * pick.range(0.5, 2.0)).collect(),
        };
        for (k, x) in extent.iter().enumerate() {
            ee.set_f32(terrain + measure_at::EXTENT + 4 * k as u32, *x);
        }
        let area = Area::new(F(extent[0]), F(extent[1]), F(extent[2]), F(extent[3]));
        let (w, d) = (area.width.to_f32(), area.depth.to_f32());
        let coord = |extent: f32, pick: &mut Pick| match pick.next() % 6 {
            0 => pick.range(-200.0, 400.0),
            1 => extent + pick.range(-400.0, 200.0),
            2 => [150.0, extent - 150.0, 0.5 * extent][(pick.next() % 3) as usize],
            _ => pick.range(0.0, extent),
        };
        let pos = Vf::new(coord(w, &mut pick), pick.range(-100.0, 2000.0), coord(d, &mut pick), 1.0);
        let pos = if pick.one_in(200) { Vf::new(0.5 * w, pos.y(), 0.5 * d, 1.0) } else { pos };
        let mut vel = vector(&mut pick, 200.0);
        let mut rates = Vf::new(pick.range(-3.0, 3.0), pick.range(-3.0, 3.0), pick.range(-3.0, 3.0), 0.0);
        put(&mut ee, body + BODY_POS, pos);
        put(&mut ee, body + BODY_VEL, vel);
        put(&mut ee, body + BODY_RATES, rates);
        // The measure's matrix: some turn, at the body.
        let mut rows = [0; 3].map(|_| vector(&mut pick, 1.0));
        for r in &mut rows {
            r[3] = 0.0;
        }
        let matrix = Mat4([rows[0], rows[1], rows[2], pos]);
        for (k, r) in matrix.0.iter().enumerate() {
            put(&mut ee, HELI + measure_at::MATRIX + 16 * k as u32, *r);
        }

        let mut port = Crash {
            reason: match pick.next() % 8 {
                0..=3 => 0,
                4 | 5 => 1,
                6 => 13,
                _ => 2,
            },
            down: pick.one_in(2),
            min_wreck_speed: F(pick.around(73.333)),
            wreck_time: F(if pick.one_in(16) { -1.0 } else { pick.range(0.0, 6.0) }),
            clock: F(if pick.one_in(8) { 0.0 } else { pick.range(0.0, 7.0) }),
        };
        ee.set_u32(HELI + WRECKED, port.reason);
        ee.set_u32(HELI + measure_at::DOWN, u32::from(port.down));
        ee.set_f32(HELI + measure_at::MIN_WRECK_SPEED, port.min_wreck_speed.to_f32());
        ee.set_f32(HELI + measure_at::WRECK_TIME, port.wreck_time.to_f32());
        ee.set_f32(HELI + measure_at::CLOCK, port.clock.to_f32());
        let mut throw_scale = F(pick.around(1.0));
        ee.set_f32(HELI + measure_at::THROW_SCALE, throw_scale.to_f32());

        // The record's contact and surface, the rotor's spheres against the ground, its struck object.
        let has_surface = pick.one_in(2);
        let surface_flags = flags(&mut pick, surface_mask);
        let c = CrashContact {
            touching: pick.one_in(2),
            ignored: has_surface && surface_flags & surface_mask != 0,
            speed: F(if pick.one_in(50) { f32::NAN } else { pick.range(0.0, 150.0) }),
        };
        ee.set_u32(record + measure_at::TOUCHING, u32::from(c.touching));
        ee.set_u32(record + measure_at::SURFACE, if has_surface { surface_at } else { 0 });
        ee.set_u32(surface_at + measure_at::FLAGS, surface_flags);
        ee.set_f32(HELI + SPEED, c.speed.to_f32());
        let has_collision = !pick.one_in(8);
        let has_world = !pick.one_in(8);
        ee.set_u32(HELI + measure_at::ROTOR_COLLISION, if has_collision { collision } else { 0 });
        ee.set_u32(HELI + measure_at::WORLD, if has_world { world } else { 0 });
        let height = pos.y() + if pick.one_in(2) { pick.range(-60.0, 0.0) } else { pick.range(-200.0, 60.0) };
        let height = if pick.one_in(50) { f32::NAN } else { height };
        let contact = pick.one_in(3);
        set_bit(&mut ee, collision + measure_at::CONTACT_BYTE, 0x20, contact);
        let has_struck = !pick.one_in(4);
        let struck_flags = flags(&mut pick, struck_mask);
        ee.set_u32(list, if has_struck { struck_at } else { 0 });
        ee.set_u32(struck_at + measure_at::FLAGS, struck_flags);

        let seed = pick.next();
        ee.set_u32(state, seed as u32);
        ee.set_u32(state + 4, (seed >> 32) as u32);
        let mut rng = Rng::from_state(seed);
        let dt = F(if pick.one_in(16) { 0.0 } else { pick.range(0.01, 0.05) });

        asked.borrow_mut().clear();
        *ground.borrow_mut() = (height, Vec::new());
        ee.call(func, &[u64::from(HELI)], &[dt.to_f32()], BUDGET);
        let mut asked_ground = Vec::new();
        let strike = || Strike {
            in_ground: has_collision && has_world && rotor_in_ground(&matrix, &centres, |p| {
                asked_ground.push(p);
                let mut q = p;
                q[1] = height;
                q
            }),
            struck: has_collision && contact && has_struck && struck_flags & struck_mask == 0,
        };
        let out = port.step(&c, Some(&area), pos, &mut vel, &mut rates, &mut throw_scale, &mut rng, dt, strike);

        let mut want = Vec::new();
        want.extend(out.wreck.map(Asked::Wreck));
        if out.debris {
            want.push(Asked::Debris);
        }
        if out.recover {
            want.push(Asked::Reset);
        }
        assert_eq!(*asked.borrow(), want, "case {case}: what the crash test asked for");
        let seen = &ground.borrow().1;
        assert_eq!(seen.len(), asked_ground.len(), "case {case}: the ground queries");
        for (got, want) in asked_ground.iter().zip(seen) {
            same_lanes("the sphere's centre", case, *got, *want, 4);
        }
        assert_eq!(ee.u32_at(HELI + WRECKED), port.reason, "case {case}: the reason");
        assert_eq!(ee.u32_at(HELI + measure_at::DOWN), u32::from(port.down), "case {case}: down");
        same("the clock", case, port.clock, ee.f32_at(HELI + measure_at::CLOCK));
        same("the throw's scale", case, throw_scale, ee.f32_at(HELI + measure_at::THROW_SCALE));
        same_lanes("the velocity", case, vel, [0, 4, 8, 12].map(|k| ee.f32_at(body + BODY_VEL + k)), 4);
        same_lanes("the rates", case, rates, [0, 4, 8, 12].map(|k| ee.f32_at(body + BODY_RATES + k)), 4);
        let left = u64::from(ee.u32_at(state)) | u64::from(ee.u32_at(state + 4)) << 32;
        assert_eq!(rng.state(), left, "case {case}: the generator");
        thrown += usize::from(out.wreck == Some(13));
        crashed += usize::from(out.wreck == Some(1));
        grounded += usize::from(out.wreck == Some(1) && asked_ground.iter().any(|p| !(height <= p.y())));
        recovered += usize::from(out.recover);
        debris += usize::from(out.debris);
    }
    assert!(thrown > CASES / 10 && crashed > CASES / 20 && grounded > CASES / 50 && recovered > CASES / 20 && debris > CASES / 200,
            "{thrown} thrown, {crashed} crashed ({grounded} rotor in the ground), {recovered} put back, {debris} blades: \
             the cases miss a branch");
    println!("the crash test: {CASES} cases, bit-exact ({thrown} thrown, {crashed} crashed ({grounded} rotor in the ground), \
              {recovered} put back, {debris} blades thrown)");
}

#[test]
fn the_mx_rotor_spins_as_the_originals() {
    use game_sim::kiowa::spin_blades;
    let Some(mut ee) = heli() else { return };
    let func = addr("kiowa_blade_spin").unwrap();
    // The model's nodes are turned and shown by these; none of them is the angle's.
    for name in ["node_set_rotation", "node_world_matrix", "node_set_matrix", "transform_touch"] {
        ee.hooks.insert(addr(name).unwrap(), Box::new(|_| {}));
    }
    let rotor = ee.u32_at(HELI + measure_at::ROTOR_NODE);
    let blade = ee.u32_at(HELI + measure_at::BLADE_NODE);
    let mut pick = Pick(0x5220_a000_b1ad_0001);
    let mut hidden_cases = 0;
    for case in 0..CASES {
        let hidden = pick.one_in(4);
        set_bit(&mut ee, rotor + 3, 0x40, hidden);
        let other = pick.one_in(2);
        set_bit(&mut ee, blade + 3, 0x40, other);
        let mut angle = F(match pick.next() % 4 {
            0 => 0.0,
            1 => pick.range(1e5, 1e7),
            _ => pick.range(0.0, 5000.0),
        });
        ee.set_f32(HELI + measure_at::ROTOR_ANGLE, angle.to_f32());
        let dt = if pick.one_in(2) { 1.0 / 30.0 } else { pick.range(0.0, 0.1) };
        ee.call(func, &[u64::from(HELI)], &[dt], BUDGET);
        spin_blades(&mut angle, hidden, F(dt));
        same("the rotor's angle", case, angle, ee.f32_at(HELI + measure_at::ROTOR_ANGLE));
        hidden_cases += usize::from(hidden);
    }
    println!("the MX rotor's spin: {CASES} cases, bit-exact ({hidden_cases} hidden)");
}

// --- S220b: the input step and the frame -------------------------------------------------------------

/// The player's control table as the input step reads it: sixteen records 0x18 apart, the value at +4.
const CONTROLS: u32 = SCRATCH + 0x1_0000;

/// A command's value: often 0 or 1, sometimes a pressure, now and then an axis's negative half or -0.
fn control_value(pick: &mut Pick) -> F {
    match pick.next() % 10 {
        0..=3 => F::ZERO,
        4 | 5 => F::ONE,
        6 => -F::from((pick.next() % 256) as i32 + 1) * F(0.003_906_25),
        7 => F(-0.0),
        _ => F::from((pick.next() % 256) as i32 + 1) * F(0.003_906_25),
    }
}

/// A stick value: what a report byte gives through the ride's range, something no pad gives, or a zero.
fn stick_value(pick: &mut Pick) -> F {
    use game_sim::input::AxisRange;
    match pick.next() % 8 {
        0 => F::ZERO,
        1 => F(-0.0),
        2 => F::from_bits(pick.next() as u32 & 0x807f_ffff | 0x3f00_0000), // 0.5..1, either sign
        3 => F::from_bits(pick.next() as u32 & 0x80ff_ffff | 0x3f00_0000), // 0.5..2, either sign
        _ => AxisRange::PLAYER.apply(F::from((pick.next() & 0xff) as i32), F(255.0)),
    }
}

/// The table written for the original, and the port's view of it on `pad` through the shipped bindings.
fn write_controls(ee: &mut Ee, table: &[F; 16]) {
    for (k, v) in table.iter().enumerate() {
        ee.set_u32(CONTROLS + 0x18 * k as u32 + 4, v.to_bits());
    }
}

fn port_input(stick: (F, F), table: &[F; 16], pad: usize) -> Pad {
    use game_sim::bindings::Table;
    Pad::from_sticks(stick, |c| Table::SHIPPED.control(pad, c).map_or(F::ZERO, |k| table[k.index()]))
}

/// S220b: the helicopter's input step against the original's on S148's state, the shipped binding
/// table in place, on generated sticks and control tables for both pads: the stick past ±1, both of a
/// pair held, signed zeros, and every d-pad override.
#[test]
fn the_input_step_is_the_originals() {
    let Some(mut ee) = heli() else { return };
    let func = addr("kiowa_input").unwrap();
    let vt = ee.u32_at(HELI + VTABLE);
    assert_eq!(ee.u32_at(vt + 0x20), func, "the helicopter's input slot is not kiowa_input");
    let mut pick = Pick(0x5220_b000_1a9d_0001);
    let (mut overridden, mut pedalled) = (0, 0);
    for case in 0..CASES {
        let pad = case % 2;
        let stick = (stick_value(&mut pick), stick_value(&mut pick));
        let table = [0; 16].map(|_| control_value(&mut pick));
        write_controls(&mut ee, &table);
        for k in 0..6 {
            ee.set_u32(HELI + at::PAD + 4 * k, 0xdead_beef);
        }
        ee.call(func, &[u64::from(HELI), u64::from(CONTROLS), pad as u64], &[stick.0.to_f32(), stick.1.to_f32()], BUDGET);
        let port = port_input(stick, &table, pad);
        let want = pad_of(&ee);
        for (name, got, want) in [
            ("climb", port.climb, want.climb),
            ("descend", port.descend, want.descend),
            ("the lateral", port.lateral, want.lateral),
            ("the longitudinal", port.longitudinal, want.longitudinal),
            ("the pedal", port.pedal, want.pedal),
        ] {
            same(name, case, got, want.to_f32());
        }
        assert_eq!(ee.u32_at(HELI + at::ALTITUDE_FRACTION), 0xdead_beef, "case {case}: the input step wrote the height");
        overridden += usize::from(port.lateral.abs() == F::ONE && stick.0.abs() != F::ONE);
        pedalled += usize::from(port.pedal != F::ZERO);
    }
    assert!(overridden > CASES / 10 && pedalled > CASES / 10, "{overridden} d-pad overrides, {pedalled} pedals");
    println!("the input step: {CASES} cases, both pads, bit-exact ({overridden} d-pad overrides, {pedalled} pedals)");
}

/// The rigid body's fields (research/oracle.txt S38's body_integrate and its neighbours).
mod body_at {
    pub const ENABLED: u32 = 0x00;
    pub const INV_MASS: u32 = 0x10;
    pub const LISTENERS: u32 = 0xd0;
    /// The quadwords, by name; the two accelerations' `w` is whatever the register held.
    pub const QUADS: [(&str, u32, usize); 21] = [
        ("damping", 0x20, 4),
        ("the last position", 0x50, 4),
        ("the position", 0x60, 4),
        ("the velocity", 0x70, 4),
        ("the last force", 0x80, 4),
        ("the force", 0x90, 4),
        ("the acceleration", 0xa0, 3),
        ("the shock peak", 0xb0, 4),
        ("the shock", 0xc0, 4),
        ("the step", 0xf0, 4),
        ("the inverse inertia", 0x120, 4),
        ("the angular damping", 0x130, 4),
        ("the last orientation", 0x150, 4),
        ("the orientation", 0x170, 4),
        ("the centre of gravity", 0x180, 4),
        ("the moment", 0x190, 4),
        ("the last moment", 0x1a0, 4),
        ("the rates", 0x1b0, 4),
        ("the angular acceleration", 0x1c0, 3),
        ("the twist peak", 0x1d0, 4),
        ("the twist", 0x1e0, 4),
    ];
}

fn body_of(ee: &Ee, b: u32) -> Body {
    let q = |o: u32| quad(ee, b + o);
    let [damping, last_position, position, velocity, last_force, force, accel, shock_peak, shock, step, inv_inertia,
         ang_damping, last_quat, quat, cofg, moment, last_moment, rates, ang_accel, twist_peak, twist] =
        body_at::QUADS.map(|(_, o, _)| q(o));
    Body {
        enabled: ee.u32_at(b + body_at::ENABLED) != 0,
        inv_mass: f(ee, b + body_at::INV_MASS),
        damping, last_position, position, velocity, last_force, force, accel, shock_peak, shock, step, inv_inertia,
        ang_damping, last_quat, quat, cofg, moment, last_moment, rates, ang_accel, twist_peak, twist,
    }
}

fn body_quads(b: &Body) -> [Vf; 21] {
    [b.damping, b.last_position, b.position, b.velocity, b.last_force, b.force, b.accel, b.shock_peak, b.shock, b.step,
     b.inv_inertia, b.ang_damping, b.last_quat, b.quat, b.cofg, b.moment, b.last_moment, b.rates, b.ang_accel,
     b.twist_peak, b.twist]
}

/// The dynamics world's fields (S38): `{count, _, array}` per list, and where each list's element keeps
/// its virtual table.
mod world_at {
    pub const ITERATIONS: u32 = 0x00;
    pub const RATE: u32 = 0x04;
    pub const LISTS: [u32; 5] = [0x50, 0x68, 0x80, 0x98, 0xb0];
}

/// The engine round the helicopter for the port, answering as the hooks answer the original.
struct Around {
    touching: bool,
    ignored: bool,
    entries: Vec<game_sim::kiowa::Touch>,
    rotor: bool,
    contact: bool,
    centres: Vec<Vf>,
    height: f32,
    resolved: usize,
    asked_ground: Vec<Vf>,
    crashed: Vec<game_sim::kiowa::Crashed>,
}

impl game_sim::kiowa::Surroundings for Around {
    fn resolve(&mut self, _: &mut Body) {
        self.resolved += 1;
    }
    fn touching(&self) -> bool {
        self.touching
    }
    fn ignored(&self) -> bool {
        self.ignored
    }
    fn entries(&self) -> Vec<game_sim::kiowa::Touch> {
        self.entries.clone()
    }
    fn strike(&mut self, matrix: &game_sim::math::Mat4) -> game_sim::kiowa::Strike {
        let (asked, height) = (&mut self.asked_ground, self.height);
        game_sim::kiowa::Strike {
            in_ground: self.rotor && game_sim::kiowa::rotor_in_ground(matrix, &self.centres, |p| {
                asked.push(p);
                let mut q = p;
                q[1] = height;
                q
            }),
            struck: self.contact,
        }
    }
    fn crashed(&mut self, asked: &game_sim::kiowa::Crashed, _: &mut game_sim::kiowa::Ride, _: &mut game_sim::rng::Rng) {
        self.crashed.push(*asked);
    }
}

/// S220b: one whole frame of the helicopter against the original's on S148's state — the input step,
/// then the engine's world step with the helicopter's own body and listeners in it (the state's
/// elements: the preforce, the force forwarder, the MX measure) — on generated pads, bodies near
/// and away from the terrain's edge, controller and crash states, contacts, sounds and seeds.
///
/// Hooked in the original, and answered the same for the port ([`Around`]): the collision record's
/// response (counted), the body wrapper's pose (it moves no figure), the crash test's wreck, blade
/// and put-back slots (recorded; the wreck's own reason and down are written, as S220a), the ground
/// (one height a case), the rotor's collision update and contact search (the struck answer written),
/// the MX presentation copies and the model's nodes. The touchdown sound runs, on one fake clip
/// whose own slots are no-ops and with the positional mixer hooked, so its `rand()` is the game's.
#[test]
fn the_frame_is_the_originals() {
    use game_sim::air::AcosTable;
    use game_sim::biplane::{Area, World};
    use game_sim::kiowa::{Crash, Measure, Ride, Touch};
    use game_sim::math::Mat4;
    use game_sim::rng::Rng;
    let Some(mut ee) = heli() else { return };
    let (input, step) = (addr("kiowa_input").unwrap(), addr("dynamics_world_step").unwrap());
    let curve = the_curve_table(&ee);
    let acos = AcosTable::build();
    let vt = ee.u32_at(HELI + VTABLE);
    let (r, a) = (ee.u32_at(HELI + ROTOR), ee.u32_at(HELI + at::AERO));
    let body = ee.u32_at(HELI + at::BODY);
    let record = ee.u32_at(HELI + at::PILOT);
    assert_eq!(ee.u32_at(body + body_at::LISTENERS), 0, "the helicopter's body has force listeners");

    // The helicopter's own elements in the state's world: the body, the force forwarder to heli+0x9c,
    // the preforce and post listeners at heli+0x90/+0x94.
    let game = 0x007d_0678;
    assert_eq!((ee.u32_at(game + world_at::ITERATIONS), ee.f32_at(game + world_at::RATE)), (1, 115.0), "no world at {game:#x}");
    let list = |ee: &Ee, l: usize| -> Vec<u32> {
        let (n, arr) = (ee.u32_at(game + world_at::LISTS[l]), ee.u32_at(game + world_at::LISTS[l] + 8));
        (0..n).map(|k| ee.u32_at(arr + 4 * k)).collect()
    };
    let force = *list(&ee, 1).iter().find(|&&e| ee.u32_at(e + 0xc) == HELI + 0x9c).expect("no force element for the helicopter");
    let elements = [Some(body), Some(force), None, Some(HELI + 0x90), Some(HELI + 0x94)];
    for (l, e) in elements.iter().enumerate() {
        if let Some(e) = e {
            assert!(list(&ee, l).contains(e), "{e:#x} is not in the world's list {l}");
        }
    }
    let world = SCRATCH + 0x2_0000;
    ee.set_u32(world + world_at::ITERATIONS, 1);
    ee.set_f32(world + world_at::RATE, 115.0);
    for (l, e) in elements.iter().enumerate() {
        let arr = world + 0x100 + 0x10 * l as u32;
        ee.set_u32(world + world_at::LISTS[l], u32::from(e.is_some()));
        ee.set_u32(world + world_at::LISTS[l] + 8, arr);
        ee.set_u32(arr, e.unwrap_or(0));
    }

    // The engine's parts, as the S220a tests hook them.
    let resolved = Rc::new(RefCell::new(0usize));
    let rs = resolved.clone();
    ee.hooks.insert(ee.u32_at(ee.u32_at(record + 8) + 0x80), Box::new(move |_| *rs.borrow_mut() += 1));
    ee.hooks.insert(addr("body_wrapper_post").unwrap(), Box::new(|_| {}));
    for name in ["kiowa_mx_present_a", "kiowa_mx_present_b", "kiowa_mx_present_c", "node_set_rotation", "node_world_matrix",
                 "node_set_matrix", "transform_touch"] {
        ee.hooks.insert(addr(name).unwrap(), Box::new(|_| {}));
    }
    let asked = Rc::new(RefCell::new(Vec::new()));
    let ak = asked.clone();
    ee.hooks.insert(ee.u32_at(vt + WRECK_SLOT), Box::new(move |ee| {
        let (heli, reason) = (ee.r(4) as u32, ee.r(5) as u32);
        ee.set_u32(heli + WRECKED, reason);
        ee.set_u32(heli + measure_at::DOWN, 0);
        ak.borrow_mut().push(Asked::Wreck(reason));
    }));
    let ak = asked.clone();
    ee.hooks.insert(ee.u32_at(vt + DEBRIS_SLOT), Box::new(move |_| ak.borrow_mut().push(Asked::Debris)));
    let ak = asked.clone();
    ee.hooks.insert(ee.u32_at(vt + RESET_SLOT), Box::new(move |_| ak.borrow_mut().push(Asked::Reset)));
    let ground = Rc::new(RefCell::new((0.0f32, Vec::<[f32; 4]>::new())));
    let g = ground.clone();
    ee.hooks.insert(addr("ground_query").unwrap(), Box::new(move |ee| {
        let p = ee.r(5) as u32;
        let mut g = g.borrow_mut();
        g.1.push([0, 4, 8, 12].map(|k| ee.f32_at(p + k)));
        ee.set_f32(p + 4, g.0);
    }));
    let collision = ee.u32_at(HELI + measure_at::ROTOR_COLLISION);
    ee.hooks.insert(ee.u32_at(ee.u32_at(collision + 0x38) + 0xac), Box::new(|_| {}));
    ee.hooks.insert(addr("collide_against_search").unwrap(), Box::new(|_| {}));
    let (contacts, struck_list, struck_at, surface_at) = (SCRATCH, SCRATCH + 0x100, SCRATCH + 0x200, SCRATCH + 0x300);
    ee.set_u32(collision + measure_at::CONTACTS, contacts);
    ee.set_u32(contacts + 0x9c, struck_list);
    let entries_at = SCRATCH + 0x400;
    ee.set_u32(record + measure_at::ENTRY_ARRAY, entries_at);
    // One clip for the touchdown: its own slots do nothing, the mixer says it is out of earshot.
    let sound = ee.u32_at(HELI + measure_at::SOUND);
    let (clips, clip, clip_vt, at_zero, clip_slot) = (SCRATCH + 0x800, SCRATCH + 0x810, SCRATCH + 0x840, SCRATCH + 0x880, 0x0000_2000);
    ee.set_u32(sound + 0x208, clips);
    ee.set_u32(sound + 0xd4, at_zero);
    ee.set_u32(clips, clip);
    ee.set_u32(clip + 8, clip_vt);
    ee.set_u32(clip_vt + 0x1c, clip_slot);
    ee.set_u32(clip_vt + 0x30, clip_slot);
    assert_eq!(ee.u32_at(sound + 0x22c), 0, "the touchdown sound has a second clip list");
    ee.hooks.insert(clip_slot, Box::new(|_| {}));
    ee.hooks.insert(addr("sound_3d").unwrap(), Box::new(|ee| ee.set(2, 0)));
    let state = addr("rand_state").unwrap();

    let terrain = ee.u32_at(ee.u32_at(HELI + measure_at::SCENE) + measure_at::TERRAIN);
    let extent: Vec<F> = (0..4).map(|k| f(&ee, terrain + measure_at::EXTENT + 4 * k)).collect();
    let area = Area::new(extent[0], extent[1], extent[2], extent[3]);
    let spheres = ee.u32_at(collision + measure_at::SPHERES);
    let centres: Vec<Vf> = (0..ee.u32_at(collision + measure_at::SPHERE_COUNT)).map(|k| quad(&ee, spheres + 0x20 * k + 0x10)).collect();
    let (surface_mask, struck_mask) = (ee.u32_at(HELI + measure_at::SURFACE_MASK), ee.u32_at(HELI + measure_at::STRUCK_MASK));
    let flags = |pick: &mut Pick, mask: u32| if pick.one_in(2) { mask } else { 1u32 << (pick.next() % 32) & !mask };
    let rotor_node = ee.u32_at(HELI + measure_at::ROTOR_NODE);
    let world_port = World { iterations: 1, rate: F(115.0) };
    let start = body_of(&ee, body);
    let pf = ee.u32_at(HELI + at::FILTER);

    let mut pick = Pick(0x5220_b000_f4a3_0001);
    let (mut inputs, mut thrown, mut crashed, mut sounded, mut still) = (0, 0, 0, 0, 0);
    for case in 0..CASES {
        // the pad, when the player has it
        let pad = case % 2;
        let stick = (stick_value(&mut pick), stick_value(&mut pick));
        let table = [0; 16].map(|_| control_value(&mut pick));
        let flown = !pick.one_in(20);
        let given = flown && !pick.one_in(10);
        write_controls(&mut ee, &table);
        ee.set_u32(HELI + at::FLOWN, u32::from(flown));
        for k in [0, 4, 12, 16, 20] {
            let v = pick.stick(-1.0);
            ee.set_f32(HELI + at::PAD + k, v);
        }
        // the controller's loop
        let hold = pick.range(0.0, 600.0);
        ee.set_f32(HELI + at::HOLD, hold);
        let timer = if pick.one_in(3) { 0.0 } else { pick.range(-0.5, 2.0) };
        ee.set_f32(HELI + at::HOLD + 4, timer);
        let fv = pick.range(0.0, 1.0);
        ee.set_f32(pf, fv);
        let integral = pick.range(-50.0, 50.0);
        ee.set_f32(HELI + at::COLLECTIVE + 16, integral);
        // the body: the state's, moved, turned and set going
        let mut b = start;
        b.enabled = !pick.one_in(12);
        let (w, d) = (area.width.to_f32(), area.depth.to_f32());
        let coord = |extent: f32, pick: &mut Pick| match pick.next() % 6 {
            0 => pick.range(-200.0, 400.0),
            1 => extent + pick.range(-400.0, 200.0),
            _ => pick.range(0.0, extent),
        };
        b.position = Vf::new(coord(w, &mut pick), pick.range(-50.0, 900.0), coord(d, &mut pick), 1.0);
        let fast = if pick.one_in(2) { 120.0 } else { 10.0 };
        b.velocity = Vf::new(pick.range(-fast, fast), pick.range(-20.0, 20.0), pick.range(-fast, fast), 1.0);
        if pick.one_in(10) {
            b.velocity = Vf::new(0.0, 0.0, 0.0, 1.0);
            still += 1;
        }
        let q = orientation(&mut pick);
        b.quat = Vf::new(q[0], q[1], q[2], q[3]);
        b.rates = Vf::new(pick.range(-2.0, 2.0), pick.range(-2.0, 2.0), pick.range(-2.0, 2.0), 1.0);
        for ((_, o, _), v) in body_at::QUADS.iter().zip(body_quads(&b)) {
            put(&mut ee, body + o, v);
        }
        ee.set_u32(body + body_at::ENABLED, u32::from(b.enabled));
        // the measure's last figures, the crash state
        ee.set_f32(HELI + SPEED, pick.range(-0.2 * fast, fast));
        ee.set_f32(HELI + SPEED + 4, pick.range(-1.6, 1.6));
        let beta = if pick.one_in(4) { (if pick.one_in(2) { 1.0 } else { -1.0 }) * pick.range(2.9, 3.1415927) } else { pick.range(-0.5, 0.5) };
        ee.set_f32(HELI + SPEED + 8, beta);
        ee.set_f32(HELI + at::AIRSPEED, pick.range(0.0, fast));
        let (pitch, roll) = (angle(&mut pick), angle(&mut pick));
        ee.set_f32(HELI + at::PITCH, pitch);
        ee.set_f32(HELI + at::ROLL, roll);
        let was = pick.one_in(2);
        ee.set_u32(HELI + measure_at::WAS_TOUCHING, u32::from(was));
        let reason = match pick.next() % 8 {
            0..=4 => 0,
            5 => 1,
            6 => 13,
            _ => 2,
        };
        ee.set_u32(HELI + WRECKED, reason);
        let down = pick.one_in(2);
        ee.set_u32(HELI + measure_at::DOWN, u32::from(down));
        let wreck_time = if pick.one_in(16) { -1.0 } else { pick.range(0.0, 6.0) };
        ee.set_f32(HELI + measure_at::WRECK_TIME, wreck_time);
        let clock = if pick.one_in(8) { 0.0 } else { pick.range(0.0, 7.0) };
        ee.set_f32(HELI + measure_at::CLOCK, clock);
        let throw_scale = pick.around(1.0);
        ee.set_f32(HELI + measure_at::THROW_SCALE, throw_scale);
        // the record, the rotor's strike, the sound, the generator, the rotor's spin
        let touching = !pick.one_in(3);
        ee.set_u32(record + measure_at::TOUCHING, u32::from(touching));
        let has_surface = pick.one_in(2);
        let surface_flags = flags(&mut pick, surface_mask);
        ee.set_u32(record + measure_at::SURFACE, if has_surface { surface_at } else { 0 });
        ee.set_u32(surface_at + measure_at::FLAGS, surface_flags);
        let entries: Vec<Touch> = (0..pick.next() % 4)
            .map(|_| Touch { kind: u32::from(pick.one_in(3)), t: F(if pick.one_in(4) { 1.0 } else { pick.range(0.0, 2.0) }) })
            .collect();
        ee.set_u32(record + measure_at::ENTRIES, entries.len() as u32);
        for (k, e) in entries.iter().enumerate() {
            ee.set_u32(entries_at + 0x80 * k as u32, e.kind);
            ee.set_f32(entries_at + 0x80 * k as u32 + 0x24, e.t.to_f32());
        }
        // Always the rotor's collision object, as every helicopter is built: without it the original's
        // crash test reads its rotor-in-the-ground flag from a register it never set (S220b's journal).
        let has_collision = true;
        let height = b.position.y() + if pick.one_in(2) { pick.range(-60.0, 0.0) } else { pick.range(-200.0, 60.0) };
        let contact = pick.one_in(3);
        set_bit(&mut ee, collision + measure_at::CONTACT_BYTE, 0x20, contact);
        let has_struck = !pick.one_in(4);
        let struck_flags = flags(&mut pick, struck_mask);
        ee.set_u32(struck_list, if has_struck { struck_at } else { 0 });
        ee.set_u32(struck_at + measure_at::FLAGS, struck_flags);
        let sound_kind = pick.next() % 3; // none, no clips, one clip
        ee.set_u32(HELI + measure_at::SOUND, if sound_kind == 0 { 0 } else { sound });
        ee.set_u32(sound + 0x200, u32::from(sound_kind == 2));
        let seed = pick.next();
        ee.set_u32(state, seed as u32);
        ee.set_u32(state + 4, (seed >> 32) as u32);
        let hidden = pick.one_in(4);
        set_bit(&mut ee, rotor_node + 3, 0x40, hidden);
        let rotor_angle = pick.range(0.0, 5000.0);
        ee.set_f32(HELI + measure_at::ROTOR_ANGLE, rotor_angle);
        let dt = match pick.next() % 8 {
            0 => 0.0,
            1 => pick.range(0.0, 0.1),
            2 => 1.0 / 60.0,
            _ => 1.0 / 30.0,
        };

        // the port's view of all that
        let mut kiowa = Kiowa {
            controller: controller_of(&ee),
            rotor: rotor_of(&ee, r),
            aero: aero_of(&ee, a),
            pad: pad_of(&ee),
            commands: commands_of(&ee),
            control: loop_of(&ee),
            rotor_state: rotor_state_of(&ee, r),
            aero_state: aero_state_of(&ee, a),
            // stale: the frame takes it from the crash state
            wrecked: pick.one_in(2),
        };
        let row = |o: u32| quad(&ee, HELI + o);
        let mut ride = Ride {
            body: body_of(&ee, body),
            measure: Measure {
                speed: f(&ee, HELI + SPEED),
                alpha: f(&ee, HELI + SPEED + 4),
                beta: f(&ee, HELI + SPEED + 8),
                world_rates: row(0x480),
                velocity: row(0x490),
                forward: row(0x4a0),
                up: row(0x4b0),
                euler: row(0x4c0),
                matrix: Mat4([row(0x4d0), row(0x4e0), row(0x4f0), row(0x500)]),
                was_touching: was,
            },
            crash: Crash {
                reason,
                down,
                min_wreck_speed: f(&ee, HELI + measure_at::MIN_WRECK_SPEED),
                wreck_time: F(wreck_time),
                clock: F(clock),
            },
            flown,
            gravity_scale: f(&ee, HELI + at::GRAVITY_SCALE),
            throw_scale: F(throw_scale),
            gravity: row(0x140),
            area: Some(area),
            touchdown_sound: sound_kind == 2,
            rotor_angle: F(rotor_angle),
            rotor_hidden: hidden,
        };
        let mut around = Around {
            touching,
            ignored: has_surface && surface_flags & surface_mask != 0,
            entries,
            rotor: has_collision,
            contact: has_collision && contact && has_struck && struck_flags & struck_mask == 0,
            centres: centres.clone(),
            height,
            resolved: 0,
            asked_ground: Vec::new(),
            crashed: Vec::new(),
        };
        let mut rng = Rng::from_state(seed);

        asked.borrow_mut().clear();
        *ground.borrow_mut() = (height, Vec::new());
        *resolved.borrow_mut() = 0;
        if given {
            ee.call(input, &[u64::from(HELI), u64::from(CONTROLS), pad as u64], &[stick.0.to_f32(), stick.1.to_f32()], BUDGET);
        }
        ee.call(step, &[u64::from(world)], &[dt], 20 * BUDGET);
        let pad_in = given.then(|| port_input(stick, &table, pad));
        let out = kiowa.tick(&mut ride, &world_port, pad_in, &curve, &acos, &mut rng, F(dt), &mut around);

        // the helicopter
        let want = pad_of(&ee);
        for (name, got, want) in [
            ("climb", kiowa.pad.climb, want.climb),
            ("descend", kiowa.pad.descend, want.descend),
            ("the lateral", kiowa.pad.lateral, want.lateral),
            ("the longitudinal", kiowa.pad.longitudinal, want.longitudinal),
            ("the pedal", kiowa.pad.pedal, want.pedal),
        ] {
            same(name, case, got, want.to_f32());
        }
        let want = commands_of(&ee);
        for (name, got, want) in [
            ("the collective", kiowa.commands.collective, want.collective),
            ("the lateral cyclic", kiowa.commands.lateral, want.lateral),
            ("the longitudinal cyclic", kiowa.commands.longitudinal, want.longitudinal),
            ("the pedal command", kiowa.commands.pedal, want.pedal),
            ("the filtered collective", kiowa.commands.filtered, want.filtered),
            ("the altitude fraction", kiowa.commands.altitude_fraction, want.altitude_fraction),
        ] {
            same(name, case, got, want.to_f32());
        }
        assert_eq!(kiowa.control, loop_of(&ee), "case {case}: the controller's loop");
        assert_eq!(kiowa.rotor_state, rotor_state_of(&ee, r), "case {case}: the rotor's state");
        assert_eq!(kiowa.aero_state, aero_state_of(&ee, a), "case {case}: the aerodynamics' state");
        // the body, every word
        let got = &ride.body;
        assert_eq!(got.enabled, ee.u32_at(body + body_at::ENABLED) != 0, "case {case}: enabled");
        same("the inverse mass", case, got.inv_mass, ee.f32_at(body + body_at::INV_MASS));
        for ((name, o, lanes), v) in body_at::QUADS.iter().zip(body_quads(got)) {
            same_lanes(name, case, v, [0, 4, 8, 12].map(|k| ee.f32_at(body + o + k)), *lanes);
        }
        // the measure
        let m = &ride.measure;
        same("the speed", case, m.speed, ee.f32_at(HELI + SPEED));
        same("alpha", case, m.alpha, ee.f32_at(HELI + SPEED + 4));
        same("beta", case, m.beta, ee.f32_at(HELI + SPEED + 8));
        let rows = [m.world_rates, m.velocity, m.forward, m.up, m.euler].into_iter().chain(m.matrix.0);
        for ((name, o), got) in measure_at::ROWS.into_iter().zip(rows) {
            same_lanes(name, case, got, [0, 4, 8, 12].map(|k| ee.f32_at(HELI + o + k)), 4);
        }
        assert_eq!(ee.u32_at(HELI + measure_at::WAS_TOUCHING), u32::from(m.was_touching), "case {case}: the touching flag");
        // the crash test and what it asked for
        let mut want = Vec::new();
        want.extend(out.wreck.map(Asked::Wreck));
        if out.debris {
            want.push(Asked::Debris);
        }
        if out.recover {
            want.push(Asked::Reset);
        }
        assert_eq!(*asked.borrow(), want, "case {case}: what the crash test asked for");
        assert_eq!(around.crashed, vec![out], "case {case}: the crash test's answer handed on");
        let seen = &ground.borrow().1;
        assert_eq!(seen.len(), around.asked_ground.len(), "case {case}: the ground queries");
        for (got, want) in around.asked_ground.iter().zip(seen) {
            same_lanes("the sphere's centre", case, *got, *want, 4);
        }
        assert_eq!(ee.u32_at(HELI + WRECKED), ride.crash.reason, "case {case}: the reason");
        assert_eq!(ee.u32_at(HELI + measure_at::DOWN), u32::from(ride.crash.down), "case {case}: down");
        same("the clock", case, ride.crash.clock, ee.f32_at(HELI + measure_at::CLOCK));
        same("the throw's scale", case, ride.throw_scale, ee.f32_at(HELI + measure_at::THROW_SCALE));
        let left = u64::from(ee.u32_at(state)) | u64::from(ee.u32_at(state + 4)) << 32;
        assert_eq!(rng.state(), left, "case {case}: the generator");
        same("the rotor's angle", case, ride.rotor_angle, ee.f32_at(HELI + measure_at::ROTOR_ANGLE));
        assert_eq!(around.resolved, *resolved.borrow(), "case {case}: the collision responses");

        inputs += usize::from(given);
        thrown += usize::from(out.wreck == Some(13));
        crashed += usize::from(out.wreck == Some(1));
        // Outside a throw only the touchdown's clip draws.
        if out.wreck != Some(13) && rng.state() != seed {
            assert_eq!(sound_kind, 2, "case {case}: a draw with no clip");
            sounded += 1;
        }
    }
    assert!(inputs > CASES / 2 && thrown > CASES / 50 && crashed > CASES / 50 && sounded > CASES / 50 && still > CASES / 20,
            "{inputs} pads, {thrown} thrown, {crashed} crashed, {sounded} sounded, {still} still: the cases miss a branch");
    println!("the frame: {CASES} cases, bit-exact ({inputs} pads read, {thrown} thrown, {crashed} crashed, \
              {sounded} touchdown sounds drawn, {still} at rest)");
}

// --- S220a1: the wreck, the unwreck, the blade throw and the put-back --------------------------------

mod wreck_at {
    pub const FLAG: u32 = 0x3d8;
    pub const WRECK_GRAVITY: u32 = 0x3ec;
    pub const WRECK_DAMPING: u32 = 0x3f0;
    pub const WRECK_INERTIA: u32 = 0x400;
    pub const INERTIA: u32 = 0x410;
    pub const GRAVITY: u32 = 0x140;
    pub const SOUND: u32 = 0x544;
    pub const PLAYER: u32 = 0x20;
    pub const RUMBLE: u32 = 0x68;
    pub const OWNER: u32 = 0xb94;
    pub const OWNER_FLAGS: u32 = 0x5c;
    pub const PLACE: u32 = 0xe0;
    pub const SPAWN: u32 = 0xcd0;
    pub const MODEL: u32 = 0x18c;
    pub const BLADE_NODES: u32 = 0xd08;
    pub const BLADES: u32 = 0xe48;
    /// The rotor's four nodes, `+0xd3c` and the wrecked blades'.
    pub const NODES: [u32; 6] = [0xd1c, 0xd20, 0xd24, 0xd28, 0xd3c, 0xd40];
    /// A node's world rows, as its lazy rebuild leaves them.
    pub const ROWS: u32 = 0x80;
    /// A body wrapper's body pointer, and a body's mass and inertia.
    pub const WRAPPER_BODY: u32 = 0x110;
    pub const BODY_MASS: u32 = 0x0c;
    pub const BODY_INERTIA: u32 = 0x110;
}

fn node_hidden(ee: &Ee, node: u32) -> bool {
    ee.u8_at(node + 3) & 0x40 != 0
}

fn quads<const N: usize>(ee: &Ee, a: u32) -> [Vf; N] {
    std::array::from_fn(|k| quad(ee, a + 16 * k as u32))
}

/// A lane for the wreck's multipliers and reciprocals: often a value the original treats apart.
fn edgy(pick: &mut Pick, x: f32) -> f32 {
    match pick.next() % 12 {
        0 => 0.0,
        1 => -0.0,
        2 => -pick.range(0.0, x),
        3 => f32::from_bits(1 + (pick.next() % 0x7f_ffff) as u32),
        4 => 1.0,
        _ => pick.range(0.0, x),
    }
}

fn edgy_quad(pick: &mut Pick, x: f32) -> [f32; 4] {
    [edgy(pick, x), edgy(pick, x), edgy(pick, x), edgy(pick, x)]
}

/// A node's world rows: a turn, often stretched, its rows' `w` lanes anything (the unpoint reads one).
fn rows(pick: &mut Pick, size: f32) -> [[f32; 4]; 4] {
    let [x, y, z, w] = orientation(pick);
    let s = if pick.one_in(3) { pick.range(0.5, 2.0) } else { 1.0 };
    let r = [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + z * w), 2.0 * (x * z - y * w)],
        [2.0 * (x * y - z * w), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + x * w)],
        [2.0 * (x * z + y * w), 2.0 * (y * z - x * w), 1.0 - 2.0 * (x * x + y * y)],
    ];
    let mut out = [[0.0; 4]; 4];
    for k in 0..3 {
        let wl = if pick.one_in(3) { 0.0 } else { pick.range(-50.0, 50.0) };
        out[k] = [s * r[k][0], s * r[k][1], s * r[k][2], wl];
    }
    out[3] = [pick.range(-size, size), pick.range(-size, size), pick.range(-size, size), 1.0];
    out
}

/// The S148 helicopter with what the wreck and the put-back reach outside the simulation stubbed: the
/// sound's start and stop, the player's rumble, the camera snap, the presentation copy, and the nodes'
/// lazy rebuild (the test writes their rows). The blades' start and stop run (S220a1a).
fn wreck_heli() -> Option<Ee> {
    let mut ee = heli()?;
    let sound = ee.u32_at(HELI + wreck_at::SOUND);
    assert_ne!(sound, 0, "the helicopter has no wreck sound");
    let sound_vt = ee.u32_at(sound + 8);
    for slot in [0x88, 0x8c] {
        ee.hooks.insert(ee.u32_at(sound_vt + slot), Box::new(|_| {}));
    }
    let player = ee.u32_at(HELI + wreck_at::PLAYER);
    assert!(player != 0 && ee.u32_at(player + 0x34) == 1, "the helicopter is not the player's: no rumble path");
    let rumble = ee.u32_at(player + wreck_at::RUMBLE);
    ee.hooks.insert(ee.u32_at(ee.u32_at(rumble + 0x1c) + 0x4c), Box::new(|_| {}));
    for name in ["camera_snap_player", "kiowa_mx_present_a", "transform_rebuild"] {
        ee.hooks.insert(addr(name).unwrap(), Box::new(|_| {}));
    }
    Some(ee)
}

fn ride_of(ee: &Ee) -> game_sim::kiowa::Ride {
    use game_sim::kiowa::{Crash, Measure, Ride};
    use game_sim::math::Mat4;
    let row = |o: u32| quad(ee, HELI + o);
    Ride {
        body: body_of(ee, ee.u32_at(HELI + at::BODY)),
        measure: Measure {
            speed: F::ZERO,
            alpha: F::ZERO,
            beta: F::ZERO,
            world_rates: row(0x480),
            velocity: row(0x490),
            forward: row(0x4a0),
            up: row(0x4b0),
            euler: row(0x4c0),
            matrix: Mat4([row(0x4d0), row(0x4e0), row(0x4f0), row(0x500)]),
            was_touching: false,
        },
        crash: Crash {
            reason: ee.u32_at(HELI + WRECKED),
            down: ee.u32_at(HELI + measure_at::DOWN) != 0,
            min_wreck_speed: f(ee, HELI + measure_at::MIN_WRECK_SPEED),
            wreck_time: f(ee, HELI + measure_at::WRECK_TIME),
            clock: f(ee, HELI + measure_at::CLOCK),
        },
        flown: true,
        gravity_scale: f(ee, HELI + at::GRAVITY_SCALE),
        throw_scale: f(ee, HELI + measure_at::THROW_SCALE),
        gravity: row(wreck_at::GRAVITY),
        area: None,
        touchdown_sound: false,
        rotor_angle: f(ee, HELI + measure_at::ROTOR_ANGLE),
        rotor_hidden: node_hidden(ee, ee.u32_at(HELI + wreck_at::NODES[0])),
    }
}

fn blade_wrapper(ee: &Ee, k: usize) -> u32 {
    ee.u32_at(HELI + wreck_at::BLADES + 4 * k as u32)
}

/// A blade wrapper's fields (the props' physics object, tests/prop_world.rs), its collision object's
/// probe's rays, and the node's lists.
mod blade_at {
    pub const ACTIVE: u32 = 0x31;
    pub const MOVING: u32 = 0x28;
    pub const PREFORCE: u32 = 0x40;
    pub const POST: u32 = 0x44;
    pub const LISTENER: u32 = 0x50;
    pub const COLLISION_ENABLED: u32 = 0x54;
    pub const LISTENS: u32 = 0x60;
    pub const DETECT: u32 = 0x6c;
    pub const OFF_EDGE: u32 = 0x70;
    pub const PLACE: u32 = 0x90;
    pub const GRAVITY: u32 = 0xf0;
    pub const EDGE: u32 = 0x100;
    pub const FORCE: u32 = 0x114;
    pub const COLLISION: u32 = 0x120;
    /// The ray probe, the mesh and the sweep probe: in the broad phase while `+4` is set.
    pub const PROBES: [u32; 3] = [0x124, 0x128, 0x12c];
    pub const WORLD_NODE: u32 = 0x134;
    pub const WORLD: u32 = 0x138;
    pub const NODE: u32 = 0x13c;
    pub const FRESH: u32 = 0x1b0;
    pub const COUNTDOWN: u32 = 0x1bc;
    pub const RESET_TIME: u32 = 0x1c0;
    pub const THRESHOLDS: u32 = 0x1c4;
    pub const BODY_START_TURN: u32 = 0x160;
    pub const PROBE_COUNT: u32 = 0x168;
    pub const PROBE_RAYS: u32 = 0x170;
    /// The edge object's bounds.
    pub const EDGE_MIN: u32 = 0x30;
    pub const EDGE_MAX: u32 = 0x40;
    /// The world node's filed collision objects and its listeners: `{count, _, array}`.
    pub const NODE_FILED: u32 = 0x30;
    pub const NODE_LISTENERS: u32 = 0x48;
}

/// A file out of `machine.res` in MX.PSR (as tests/kiowa_phy.rs reads it).
fn machine_file(kit: &Kit, name: &str) -> Option<Vec<u8>> {
    use game_sim::archive::{Psr, Res};
    use std::io::{Read, Seek, SeekFrom};
    let at = kit.ctx("iso").join("MX.PSR");
    let Ok(mut f) = std::fs::File::open(&at) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", at.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find("machine.res").expect("machine.res is in MX.PSR");
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    let res = Res::parse(&bytes).expect("machine.res");
    let e = res.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name)).unwrap_or_else(|| panic!("machine.res has no {name}"));
    Some(res.read(&bytes, e).expect("its bytes"))
}

/// A list `{count, _, array}` at `a`.
fn list_at(ee: &Ee, a: u32) -> Vec<u32> {
    let (n, arr) = (ee.u32_at(a), ee.u32_at(a + 8));
    (0..n).map(|k| ee.u32_at(arr + 4 * k)).collect()
}

/// The port's four blades off the disc's files, with the first wrapper's edge (the terrain's).
fn blades_for(ee: &Ee) -> Option<game_sim::kiowa::Blades> {
    use game_sim::kiowa::{BLADE_MODEL, BLADE_RAYS, Blades, WRECK_BLADE_PHY};
    use game_sim::prop_world::Edge;
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let [phy, psg, rays] = [WRECK_BLADE_PHY, BLADE_MODEL, BLADE_RAYS].map(|n| machine_file(&kit, n));
    let edge = ee.u32_at(blade_wrapper(ee, 0) + blade_at::EDGE);
    assert_ne!(edge, 0, "the blade has no edge");
    let lane = |a: u32, k: u32| F(ee.f32_at(edge + a + 4 * k));
    let edge = Edge { min: [lane(blade_at::EDGE_MIN, 0), lane(blade_at::EDGE_MIN, 2)], max: [lane(blade_at::EDGE_MAX, 0), lane(blade_at::EDGE_MAX, 2)] };
    Some(Blades::new(&phy?, &psg?, Some(&rays?), Some(edge)).unwrap_or_else(|e| panic!("the blades: {e}")))
}

/// The state's blade bodies into the port's, all but the enabled flag (the port's own: [`same_blades`]
/// checks it).
fn blades_from(ee: &Ee, blades: &mut game_sim::kiowa::Blades) {
    for (k, bone) in blades.set.bones.iter_mut().enumerate() {
        let enabled = bone.body.enabled;
        bone.body = body_of(ee, ee.u32_at(blade_wrapper(ee, k) + wreck_at::WRAPPER_BODY));
        bone.body.enabled = enabled;
    }
}

/// Everything of the four blades the start, the stop, the throw and the world step leave, the game's
/// wrappers against the port's [`Blades`]: the body, the active bits, the node shown and in the
/// scene, the probes in the broad phase, the fresh flag and `+0x28`'s awake bit, the wrapper's put-back
/// place, and its four elements in the world's lists, its listener on the node and its collision object
/// filed there.
fn same_blades(case: usize, ee: &Ee, blades: &game_sim::kiowa::Blades) {
    use game_sim::prop_world::Listener;
    for k in 0..4 {
        let w = blade_wrapper(ee, k);
        let (s, o, bone) = (&blades.started[k], &blades.set.objects[k], &blades.set.bones[k]);
        let at = |what: &str| format!("case {case}: blade {k}'s {what}");
        same_body(&format!("blade {k}"), case, ee, ee.u32_at(w + wreck_at::WRAPPER_BODY), &bone.body);
        let active = ee.u8_at(w + blade_at::ACTIVE) & 3;
        assert!(active == 0 || active == 3, "{}: {active}", at("active bits"));
        assert_eq!(active == 3, s.out, "{}", at("start"));
        let node = ee.u32_at(w + blade_at::NODE);
        assert_eq!(!node_hidden(ee, node), s.shown, "{}", at("node shown"));
        assert_eq!(ee.u32_at(node + 4) != 0, s.placed, "{}", at("node in the scene"));
        let probes = blade_at::PROBES.map(|p| ee.u32_at(ee.u32_at(w + p) + 4) != 0);
        assert_eq!(probes, [s.probes; 3], "{}", at("probes in the broad phase"));
        assert_eq!(ee.u32_at(w + blade_at::FRESH) != 0, o.fresh, "{}", at("fresh flag"));
        assert_eq!(ee.u32_at(w + blade_at::MOVING) & 2 != 0, bone.body.enabled, "{}", at("+0x28 awake bit"));
        for (q, (name, got)) in [("place", o.place), ("look", o.look), ("up", o.up)].into_iter().enumerate() {
            assert_eq!(got.to_bits(), quad(ee, w + blade_at::PLACE + 16 * q as u32).to_bits(), "{}", at(name));
        }
        assert_eq!(o.countdown.to_bits(), ee.u32_at(w + blade_at::COUNTDOWN), "{}", at("countdown"));
        let world = ee.u32_at(w + blade_at::WORLD);
        let elements = [ee.u32_at(w + wreck_at::WRAPPER_BODY), ee.u32_at(w + blade_at::FORCE), w + blade_at::PREFORCE, w + blade_at::POST];
        let lists = [&blades.set.bodies, &blades.set.forces, &blades.set.preforce, &blades.set.posts];
        for ((l, e), port) in [0usize, 1, 3, 4].into_iter().zip(elements).zip(lists) {
            assert_eq!(list_at(ee, world + world_at::LISTS[l]).contains(&e), port.contains(&k), "{} in the world's list {l}", at("element"));
        }
        let node = ee.u32_at(w + blade_at::WORLD_NODE);
        assert_eq!(list_at(ee, node + blade_at::NODE_LISTENERS).contains(&(w + blade_at::LISTENER)),
                   blades.set.listeners.contains(&Listener::Object(k)), "{}", at("listener on the node"));
        assert_eq!(list_at(ee, node + blade_at::NODE_FILED).contains(&ee.u32_at(w + blade_at::COLLISION)), o.filed, "{}", at("collision object filed"));
    }
}

/// The helicopter's wreckage as the state has it, with the port's `blades` (their bodies the state's:
/// [`blades_from`]).
fn wreckage_of(ee: &Ee, mut blades: game_sim::kiowa::Blades) -> game_sim::kiowa::Wreckage {
    use game_sim::kiowa::{Nodes, Wreckage};
    let body = ee.u32_at(HELI + at::BODY);
    let hidden = wreck_at::NODES.map(|o| node_hidden(ee, ee.u32_at(HELI + o)));
    Wreckage {
        wreck_gravity: f(ee, HELI + wreck_at::WRECK_GRAVITY),
        wreck_damping: quad(ee, HELI + wreck_at::WRECK_DAMPING),
        wreck_inertia: quad(ee, HELI + wreck_at::WRECK_INERTIA),
        inertia: quad(ee, HELI + wreck_at::INERTIA),
        mass: f(ee, body + wreck_at::BODY_MASS),
        body_inertia: quad(ee, body + wreck_at::BODY_INERTIA),
        wrecked: ee.u32_at(HELI + wreck_at::FLAG) != 0,
        place: quads(ee, HELI + wreck_at::PLACE),
        spawn: quads(ee, HELI + wreck_at::SPAWN),
        nodes: Nodes { rotor: [hidden[1], hidden[2], hidden[3]], intact: hidden[4], wrecked: hidden[5] },
        blades: {
            blades_from(ee, &mut blades);
            blades
        },
    }
}

fn throw_of(ee: &Ee) -> game_sim::kiowa::Throw {
    use game_sim::math::Mat4;
    let rows = |node: u32| Mat4(quads(ee, node + wreck_at::ROWS));
    game_sim::kiowa::Throw {
        blades: std::array::from_fn(|k| rows(ee.u32_at(HELI + wreck_at::BLADE_NODES + 4 * k as u32))),
        model: rows(ee.u32_at(HELI + wreck_at::MODEL)),
    }
}

fn same_body(what: &str, case: usize, ee: &Ee, b: u32, port: &Body) {
    assert_eq!(ee.u32_at(b + body_at::ENABLED) != 0, port.enabled, "case {case}: {what}'s enabled flag");
    same(&format!("{what}'s inverse mass"), case, port.inv_mass, ee.f32_at(b + body_at::INV_MASS));
    for ((name, o, lanes), got) in body_at::QUADS.iter().zip(body_quads(port)) {
        same_lanes(&format!("{what}: {name}"), case, got, [0, 4, 8, 12].map(|k| ee.f32_at(b + o + k)), *lanes);
    }
}

/// Generated states for the helicopter, its blades and their nodes, written into the state.
fn scramble(ee: &mut Ee, pick: &mut Pick) {
    let body = ee.u32_at(HELI + at::BODY);
    let lane = |pick: &mut Pick, x: f32| if pick.one_in(10) { f32::from_bits((pick.next() % 0x7f_ffff) as u32) } else { pick.range(-x, x) };
    let rates = [lane(pick, 6.0), lane(pick, 6.0), lane(pick, 6.0), pick.range(-2.0, 2.0)];
    set_quad(ee, body + BODY_RATES, rates);
    let v = vector(pick, 150.0);
    set_quad(ee, body + BODY_VEL, [v.x(), v.y(), v.z(), if pick.one_in(2) { 1.0 } else { pick.range(-5.0, 5.0) }]);
    for o in [0x20, 0x130, 0x120, 0x110] {
        let q = edgy_quad(pick, 2.0);
        set_quad(ee, body + o, q);
    }
    let p = vector(pick, 5000.0);
    put(ee, body + BODY_POS, p);
    set_quad(ee, body + BODY_QUAT, orientation(pick));
    if pick.one_in(2) {
        set_quad(ee, body + BODY_COFG, [pick.range(-3.0, 3.0), pick.range(-3.0, 3.0), pick.range(-3.0, 3.0), 1.0]);
    }
    ee.set_f32(body + wreck_at::BODY_MASS, if pick.one_in(10) { 0.0 } else { pick.range(1.0, 200.0) });

    let reason = [0, 1, 13, (pick.next() % 20) as u32][(pick.next() % 4) as usize];
    ee.set_u32(HELI + WRECKED, reason);
    ee.set_u32(HELI + measure_at::DOWN, u32::from(pick.one_in(2)));
    ee.set_f32(HELI + measure_at::CLOCK, pick.range(0.0, 5.0));
    ee.set_f32(HELI + measure_at::THROW_SCALE, if pick.one_in(4) { 1.0 } else { pick.range(0.0, 2.0) });
    ee.set_f32(HELI + at::GRAVITY_SCALE, edgy(pick, 4.0));
    ee.set_u32(HELI + wreck_at::FLAG, u32::from(pick.one_in(2)));
    ee.set_f32(HELI + wreck_at::WRECK_GRAVITY, edgy(pick, 6.0));
    for o in [wreck_at::WRECK_DAMPING, wreck_at::WRECK_INERTIA] {
        let q = edgy_quad(pick, 2.0);
        set_quad(ee, HELI + o, q);
    }
    let q = [edgy(pick, 20000.0), edgy(pick, 20000.0), edgy(pick, 20000.0), edgy(pick, 3.0)];
    set_quad(ee, HELI + wreck_at::INERTIA, q);
    set_quad(ee, HELI + wreck_at::GRAVITY, edgy_quad(pick, 5000.0));
    for q in 0..3 {
        for o in [wreck_at::PLACE, wreck_at::SPAWN] {
            let v = vector(pick, if q == 0 { 5000.0 } else { 1.0 });
            set_quad(ee, HELI + o + 16 * q, [v.x(), v.y(), v.z(), pick.range(-2.0, 2.0)]);
        }
    }
    if pick.one_in(4) {
        // the shipped nose and up, which need no squaring
        set_quad(ee, HELI + wreck_at::PLACE + 16, [-1.0, 0.0, 0.0, 1.0]);
        set_quad(ee, HELI + wreck_at::PLACE + 32, [0.0, 1.0, 0.0, 1.0]);
    }
    for o in wreck_at::NODES {
        let node = ee.u32_at(HELI + o);
        set_bit(ee, node + 3, 0x40, pick.one_in(2));
    }
    for o in at::PAD..at::PAD + 24 {
        if o % 4 == 0 {
            ee.set_f32(HELI + o, pick.stick(-1.0));
        }
    }
    ee.set_f32(HELI + at::HOLD, pick.range(0.0, 600.0));

    for k in 0..4 {
        let b = ee.u32_at(blade_wrapper(ee, k) + wreck_at::WRAPPER_BODY);
        for (_, o, _) in body_at::QUADS {
            let v = vector(pick, 100.0);
            put(ee, b + o, v);
        }
        set_quad(ee, b + BODY_QUAT, orientation(pick));
        set_quad(ee, b + BODY_COFG, [pick.range(-2.0, 2.0), pick.range(-2.0, 2.0), pick.range(0.0, 12.0), 1.0]);
        let node = ee.u32_at(HELI + wreck_at::BLADE_NODES + 4 * k as u32);
        let r = rows(pick, 5000.0);
        for (q, row) in r.iter().enumerate() {
            set_quad(ee, node + wreck_at::ROWS + 16 * q as u32, *row);
        }
    }
    let model = ee.u32_at(HELI + wreck_at::MODEL);
    let r = rows(pick, 5000.0);
    for (q, row) in r.iter().enumerate() {
        set_quad(ee, model + wreck_at::ROWS + 16 * q as u32, *row);
    }
}

/// Everything the wreck, the throw, the unwreck and the put-back write, the game's against the port's.
fn same_wreckage(case: usize, ee: &Ee, ride: &game_sim::kiowa::Ride, w: &game_sim::kiowa::Wreckage, kiowa: &Kiowa) {
    let body = ee.u32_at(HELI + at::BODY);
    same_body("the helicopter's body", case, ee, body, &ride.body);
    same_lanes("the body's inertia", case, w.body_inertia, [0, 4, 8, 12].map(|k| ee.f32_at(body + wreck_at::BODY_INERTIA + k)), 4);
    same_lanes("the built inertia", case, w.inertia, [0, 4, 8, 12].map(|k| ee.f32_at(HELI + wreck_at::INERTIA + k)), 4);
    same_lanes("the gravity", case, ride.gravity, [0, 4, 8, 12].map(|k| ee.f32_at(HELI + wreck_at::GRAVITY + k)), 4);
    assert_eq!(ee.u32_at(HELI + WRECKED), ride.crash.reason, "case {case}: the reason");
    assert_eq!(ee.u32_at(HELI + measure_at::DOWN) != 0, ride.crash.down, "case {case}: down");
    assert_eq!(ee.u32_at(HELI + wreck_at::FLAG) != 0, w.wrecked, "case {case}: the wrecked flag");
    same("the clock", case, ride.crash.clock, ee.f32_at(HELI + measure_at::CLOCK));
    same("the throw's scale", case, ride.throw_scale, ee.f32_at(HELI + measure_at::THROW_SCALE));
    for (q, got) in w.place.iter().enumerate() {
        same_lanes("the place", case, *got, [0, 4, 8, 12].map(|k| ee.f32_at(HELI + wreck_at::PLACE + 16 * q as u32 + k)), 4);
    }
    let hidden = wreck_at::NODES.map(|o| node_hidden(ee, ee.u32_at(HELI + o)));
    let port = [ride.rotor_hidden, w.nodes.rotor[0], w.nodes.rotor[1], w.nodes.rotor[2], w.nodes.intact, w.nodes.wrecked];
    assert_eq!(hidden, port, "case {case}: the nodes hidden (rotor x4, +0xd3c, the wrecked blades)");
    same_blades(case, ee, &w.blades);
    let pad = pad_of(ee);
    for (name, got, want) in [("climb", kiowa.pad.climb, pad.climb), ("descend", kiowa.pad.descend, pad.descend),
                              ("lateral", kiowa.pad.lateral, pad.lateral), ("longitudinal", kiowa.pad.longitudinal, pad.longitudinal),
                              ("pedal", kiowa.pad.pedal, pad.pedal)] {
        same(&format!("the pad's {name}"), case, got, want.to_f32());
    }
    same("the altitude fraction", case, kiowa.commands.altitude_fraction, ee.f32_at(HELI + at::ALTITUDE_FRACTION));
    same("the held height", case, kiowa.control.hold_altitude, ee.f32_at(HELI + at::HOLD));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wreck {
    Base,
    Mx,
    Throw,
    Unwreck,
    UnwreckMx,
    PutBack,
}

/// S220a1: the wreck (base and MX), the blade throw, the unwreck (base and MX) and the MX put-back, each
/// called in the original on a generated helicopter, blades and nodes, against [`Wreckage`]'s.
#[test]
fn the_wreck_and_the_put_back_are_the_originals() {
    use game_sim::rng::Rng;
    let Some(mut ee) = wreck_heli() else { return };
    let Some(mut blades) = blades_for(&ee) else { return };
    let (start, stop) = (addr("wreck_part_start").unwrap(), addr("wreck_part_stop").unwrap());
    let state = addr("rand_state").unwrap();
    let funcs = [
        (Wreck::Base, "kiowa_wreck"),
        (Wreck::Mx, "kiowa_wreck_mx"),
        (Wreck::Throw, "kiowa_throw_blades"),
        (Wreck::Unwreck, "kiowa_unwreck"),
        (Wreck::UnwreckMx, "kiowa_unwreck_mx"),
        (Wreck::PutBack, "kiowa_put_back_mx"),
    ]
    .map(|(w, name)| (w, addr(name).unwrap()));
    let vt = ee.u32_at(HELI + VTABLE);
    assert_eq!(ee.u32_at(vt + WRECK_SLOT), funcs[1].1, "the MX table's wreck");
    assert_eq!(ee.u32_at(vt + DEBRIS_SLOT), funcs[2].1, "the MX table's blade throw");
    assert_eq!(ee.u32_at(vt + RESET_SLOT), funcs[5].1, "the MX table's put-back");
    let owner = SCRATCH + 0x400;
    let wrappers: [u32; 4] = std::array::from_fn(|k| blade_wrapper(&ee, k));
    let mut pick = Pick(0x5220_a100_b1ad_0001);
    let mut seen = [0usize; 6];
    let (mut thrown, mut owned_cases, mut reciprocal_zero) = (0, 0, 0);
    let (mut started, mut stopped, mut rethrown) = (0, [0usize; 2], 0);
    for case in 0..CASES {
        scramble(&mut ee, &mut pick);
        let (kind, func) = funcs[(pick.next() % 6) as usize];
        let reason = [1, 13, (pick.next() % 20) as u32][(pick.next() % 3) as usize];
        let owned = pick.one_in(2);
        let owner_kind = pick.next() % 3; // none, an owner that keeps the place, one that does not
        ee.set_u32(HELI + wreck_at::OWNER, if owner_kind == 0 { 0 } else { owner });
        ee.set_u32(owner + wreck_at::OWNER_FLAGS, (pick.next() as u32 & !1) | u32::from(owner_kind == 1));
        let owned = if kind == Wreck::PutBack { owner_kind == 1 } else { owned };
        let seed = pick.next();
        ee.set_u32(state, seed as u32);
        ee.set_u32(state + 4, (seed >> 32) as u32);
        // S220a1a: each blade started, stopped (hidden or not) or left as it is, in both.
        blades_from(&ee, &mut blades);
        for (k, &wrapper) in wrappers.iter().enumerate() {
            match pick.next() % 4 {
                0 => {
                    ee.call(start, &[u64::from(wrapper)], &[], BUDGET);
                    blades.start(k);
                    started += 1;
                }
                1 => {
                    let hide = pick.one_in(2);
                    ee.call(stop, &[u64::from(wrapper), u64::from(hide)], &[], BUDGET);
                    blades.stop(k, hide);
                    stopped[usize::from(hide)] += 1;
                }
                _ => {}
            }
        }
        same_blades(case, &ee, &blades);
        let out_before = blades.started.iter().filter(|s| s.out).count();

        let mut ride = ride_of(&ee);
        let mut w = wreckage_of(&ee, blades.clone());
        let throw = throw_of(&ee);
        let mut kiowa = Kiowa { pad: pad_of(&ee), commands: commands_of(&ee), control: loop_of(&ee), ..Kiowa::default() };
        let mut rng = Rng::from_state(seed);
        match kind {
            Wreck::Base => w.wreck(&mut ride, reason),
            Wreck::Mx => w.wreck_mx(&mut ride, reason, &throw, &mut rng),
            Wreck::Throw => w.throw_blades(&mut ride, &throw, &mut rng),
            Wreck::Unwreck => {
                w.unwreck(&mut ride);
                kiowa.unwreck_controls();
            }
            Wreck::UnwreckMx => {
                w.unwreck_mx(&mut ride);
                kiowa.unwreck_controls();
            }
            Wreck::PutBack => {
                let fraction = w.put_back(&mut ride, owned);
                kiowa.put_back_controls(fraction);
            }
        }

        let ints = if matches!(kind, Wreck::Base | Wreck::Mx) { vec![u64::from(HELI), u64::from(reason)] } else { vec![u64::from(HELI)] };
        ee.call(func, &ints, &[], 4 * BUDGET);

        same_wreckage(case, &ee, &ride, &w, &kiowa);
        let left = u64::from(ee.u32_at(state)) | u64::from(ee.u32_at(state + 4)) << 32;
        assert_eq!(rng.state(), left, "case {case}: the generator ({kind:?})");
        let threw = kind == Wreck::Throw || kind == Wreck::Mx && reason != 13;
        blades = w.blades;
        seen[kind as usize] += 1;
        thrown += usize::from(threw);
        rethrown += usize::from(threw && out_before > 0);
        owned_cases += usize::from(kind == Wreck::PutBack && owned);
        reciprocal_zero += usize::from(w.body_inertia.x() <= 0.0 && matches!(kind, Wreck::Base | Wreck::Mx));
    }
    assert!(seen.iter().all(|&n| n > CASES / 10) && thrown > CASES / 5 && owned_cases > CASES / 50 && reciprocal_zero > CASES / 100,
            "{seen:?} by kind, {thrown} throws, {owned_cases} owned put-backs, {reciprocal_zero} unset lanes: the cases miss a branch");
    assert!(started > CASES / 2 && stopped.iter().all(|&n| n > CASES / 4) && rethrown > CASES / 10,
            "{started} starts, {stopped:?} stops (kept, hidden), {rethrown} throws of blades out: the cases miss a branch");
    println!("the wreck and the put-back: {CASES} cases, bit-exact ({seen:?} by kind, {thrown} throws ({rethrown} of blades \
              out), {owned_cases} owned put-backs, {reciprocal_zero} lanes not above 0; {started} starts, {stopped:?} stops)");
}

/// [`Wreckage::act`] is the MX slots in the crash test's order: the wreck, the blade throw, the
/// put-back, with the put-back's altitude fraction handed out.
#[test]
fn a_crash_test_answer_is_acted_on_in_the_originals_order() {
    use game_sim::kiowa::Crashed;
    use game_sim::rng::Rng;
    let Some(mut ee) = wreck_heli() else { return };
    let Some(blades) = blades_for(&ee) else { return };
    let mut pick = Pick(0x5220_a100_0ac7_0001);
    for case in 0..200 {
        scramble(&mut ee, &mut pick);
        let asked = Crashed {
            wreck: [None, Some(1), Some(13), Some(5)][(pick.next() % 4) as usize],
            debris: pick.one_in(2),
            recover: pick.one_in(2),
        };
        let owned = pick.one_in(2);
        let seed = pick.next();
        let (ride, w, throw) = (ride_of(&ee), wreckage_of(&ee, blades.clone()), throw_of(&ee));
        let (mut ride_a, mut w_a, mut rng_a) = (ride, w.clone(), Rng::from_state(seed));
        let got = w_a.act(&asked, &mut ride_a, &throw, owned, &mut rng_a);
        let (mut ride_b, mut w_b, mut rng_b) = (ride, w, Rng::from_state(seed));
        if let Some(reason) = asked.wreck {
            w_b.wreck_mx(&mut ride_b, reason, &throw, &mut rng_b);
        }
        if asked.debris {
            w_b.throw_blades(&mut ride_b, &throw, &mut rng_b);
        }
        let want = asked.recover.then(|| w_b.put_back(&mut ride_b, owned));
        assert_eq!((got, ride_a, &w_a, rng_a.state()), (want, ride_b, &w_b, rng_b.state()), "case {case}: {asked:?}");
        if asked.recover {
            assert_eq!(got, Some(if owned { F(0.5) } else { F::ZERO }), "case {case}: the altitude fraction");
            assert!(!w_a.wrecked && ride_a.crash.reason == 0, "case {case}: still wrecked after the put-back");
        }
    }
}

/// S220a1a: the four blades as the MX constructor built them in S148's state, against
/// [`Blades::new`] off `wreckblade.phy`, `bladebent.psg` and `bladebentm.col`: every body word, the
/// mass and inertia, the start turn, the object's gravity, switches, thresholds, put-back place and
/// countdown, the probe's rays, and the shared world and world node.
#[test]
fn the_blades_are_built_as_the_originals() {
    let Some(ee) = heli() else { return };
    let Some(blades) = blades_for(&ee) else { return };
    same_blades(0, &ee, &blades);
    let heli_wrapper = HELI + 0x50;
    for k in 0..4 {
        let w = blade_wrapper(&ee, k);
        let (o, bone) = (&blades.set.objects[k], &blades.set.bones[k]);
        let b = ee.u32_at(w + wreck_at::WRAPPER_BODY);
        let at = |what: &str| format!("blade {k}'s {what}");
        assert_eq!(bone.mass.to_bits(), ee.u32_at(b + wreck_at::BODY_MASS), "{}", at("mass"));
        assert_eq!(bone.inertia.to_bits(), quad(&ee, b + wreck_at::BODY_INERTIA).to_bits(), "{}", at("inertia"));
        assert_eq!(bone.reference.to_bits(), quad(&ee, b + blade_at::BODY_START_TURN).to_bits(), "{}", at("start turn"));
        assert_eq!(o.gravity.to_bits(), quad(&ee, w + blade_at::GRAVITY).to_bits(), "{}", at("gravity"));
        assert_eq!(o.collision_enabled, ee.u32_at(w + blade_at::COLLISION_ENABLED) != 0, "{}", at("CollisionEnabled"));
        assert_eq!(o.detect_when_disabled, ee.u32_at(w + blade_at::DETECT) != 0, "{}", at("DetectCollisionsWhenDisabled"));
        assert_eq!(o.reset_off_edge, ee.u32_at(w + blade_at::OFF_EDGE) != 0, "{}", at("ResetOffEdgeOfWorld"));
        assert_eq!(o.listens, ee.u32_at(w + blade_at::LISTENS) != 0, "{}", at("object listener"));
        assert_eq!(o.reset_time.to_bits(), ee.u32_at(w + blade_at::RESET_TIME), "{}", at("ResetTime"));
        for (t, got) in o.thresholds.iter().enumerate() {
            assert_eq!(got.to_bits(), ee.u32_at(w + blade_at::THRESHOLDS + 4 * t as u32), "{}", at(&format!("threshold {t}")));
        }
        let probe = ee.u32_at(w + blade_at::PROBES[0]);
        let n = ee.u32_at(probe + blade_at::PROBE_COUNT);
        let rays = ee.u32_at(probe + blade_at::PROBE_RAYS);
        let mut game: Vec<[u32; 8]> = (0..n).map(|r| std::array::from_fn(|i| ee.u32_at(rays + 0x20 * r + 4 * i as u32))).collect();
        let c = o.collision.as_ref().expect("the blade's collision object");
        let mut port: Vec<[u32; 8]> = c.record.rays.as_ref().expect("the blade's rays").iter()
            .map(|[from, to]| {
                let (a, b) = (from.to_bits(), to.to_bits());
                [a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3]]
            })
            .collect();
        game.sort();
        port.sort();
        assert_eq!(port, game, "{}", at("rays"));
        for (name, off) in [("world node", blade_at::WORLD_NODE), ("dynamics world", blade_at::WORLD)] {
            assert_eq!(ee.u32_at(w + off), ee.u32_at(heli_wrapper + off), "{}: not the helicopter's", at(name));
        }
    }
    let world = ee.u32_at(blade_wrapper(&ee, 0) + blade_at::WORLD);
    assert_eq!((ee.i32_at(world + world_at::ITERATIONS), F(ee.f32_at(world + world_at::RATE))),
               (blades.set.world.iterations, blades.set.world.rate), "the shared world's iterations and rate");
    println!("the blades: four bodies, objects and ray probes as built in the state");
}

/// S220a1a: the blades thrown off a generated helicopter (the original's `kiowa_throw_blades`, its
/// starts run), then flown for two seconds a case: each frame the original's world step and the
/// blades' object listeners on the node, against [`Blades::step`] and [`Blades::post`], every blade
/// compared ([`same_blades`]) and its node's pose. On the throw's frame only the posts and the
/// listeners run, as they follow the helicopter's post in the world's list. Some blades are out
/// already when thrown, some are stopped, and many land past the world's edge and are put back.
///
/// The world holds the helicopter's body (so it steps with no blade awake, as the game's does) and
/// the blades' elements; the collide pass is not run on either side, so no blade has a contact.
#[test]
fn thrown_blades_fly_as_the_originals() {
    use game_sim::ragdoll::Tables;
    use game_sim::rng::Rng;
    let Some(mut ee) = wreck_heli() else { return };
    // The blades' posts write their nodes' world rows through the rebuild the wreck tests stub.
    ee.hooks.remove(&addr("transform_rebuild").unwrap());
    let Some(mut blades) = blades_for(&ee) else { return };
    let throw_fn = addr("kiowa_throw_blades").unwrap();
    let (start, stop) = (addr("wreck_part_start").unwrap(), addr("wreck_part_stop").unwrap());
    let (step, post, listen) = (addr("dynamics_world_step").unwrap(), addr("body_wrapper_post").unwrap(), addr("physics_object_listen").unwrap());
    let state = addr("rand_state").unwrap();
    let tables = Tables::build();
    let wrappers: [u32; 4] = std::array::from_fn(|k| blade_wrapper(&ee, k));
    let world = ee.u32_at(wrappers[0] + blade_at::WORLD);
    let node = ee.u32_at(wrappers[0] + blade_at::WORLD_NODE);
    // The world down to the helicopter's body; the node's listeners to none (the blades' go on).
    let heli_body = ee.u32_at(HELI + at::BODY);
    for (l, off) in world_at::LISTS.iter().enumerate() {
        ee.set_u32(world + off, u32::from(l == 0));
    }
    ee.set_u32(ee.u32_at(world + world_at::LISTS[0] + 8), heli_body);
    ee.set_u32(node + blade_at::NODE_LISTENERS, 0);
    // The blades' bodies as built: the generated ones keep these.
    let kept: Vec<Vec<(u32, [u32; 4])>> = wrappers.iter().map(|&w| {
        let b = ee.u32_at(w + wreck_at::WRAPPER_BODY);
        [0x20, 0xf0, 0x120, 0x130, 0x180].iter().map(|&o| (b + o, quad(&ee, b + o).to_bits())).collect()
    }).collect();

    let dt = 1.0f32 / 30.0;
    let (cases, frames) = (40, 60);
    let mut pick = Pick(0x5220_a1a0_f1e5_0001);
    let (mut flown, mut put_back, mut rethrown) = (0, 0, 0);
    for case in 0..cases {
        scramble(&mut ee, &mut pick);
        if !pick.one_in(4) {
            // A calm helicopter in the middle of the map: the blades thrown near it at its own speed.
            let body = ee.u32_at(HELI + at::BODY);
            let v = vector(&mut pick, 10.0);
            set_quad(&mut ee, body + BODY_VEL, [v.x(), v.y(), v.z(), 1.0]);
            let r = vector(&mut pick, 1.0);
            set_quad(&mut ee, body + BODY_RATES, [r.x(), r.y(), r.z(), 0.0]);
            let mid = [pick.range(1500.0, 5400.0), pick.range(0.0, 800.0), pick.range(1500.0, 5400.0)];
            let model = ee.u32_at(HELI + wreck_at::MODEL);
            let mut nodes: Vec<u32> = (0..4).map(|k| ee.u32_at(HELI + wreck_at::BLADE_NODES + 4 * k)).collect();
            nodes.push(model);
            for node in nodes {
                let d = vector(&mut pick, 20.0);
                set_quad(&mut ee, node + wreck_at::ROWS + 48, [mid[0] + d.x(), mid[1] + d.y(), mid[2] + d.z(), 1.0]);
            }
        }
        for quads in &kept {
            for &(a, q) in quads {
                for (i, w) in q.iter().enumerate() {
                    ee.set_u32(a + 4 * i as u32, *w);
                }
            }
        }
        blades_from(&ee, &mut blades);
        for (k, &w) in wrappers.iter().enumerate() {
            if pick.one_in(4) {
                ee.call(start, &[u64::from(w)], &[], BUDGET);
                blades.start(k);
            } else if pick.one_in(4) {
                ee.call(stop, &[u64::from(w), 1], &[], BUDGET);
                blades.stop(k, true);
            }
        }
        rethrown += blades.started.iter().filter(|s| s.out).count();
        let seed = pick.next();
        ee.set_u32(state, seed as u32);
        ee.set_u32(state + 4, (seed >> 32) as u32);
        let mut ride = ride_of(&ee);
        let mut w = wreckage_of(&ee, blades.clone());
        w.throw_blades(&mut ride, &throw_of(&ee), &mut Rng::from_state(seed));
        ee.call(throw_fn, &[u64::from(HELI)], &[], 4 * BUDGET);
        blades = w.blades;
        same_blades(case, &ee, &blades);

        for frame in 0..=frames {
            if frame > 0 {
                ee.call(step, &[u64::from(world)], &[dt], 50 * BUDGET);
                blades.step(F(dt), &tables, &[]);
            } else {
                // The throw's frame: the posts after the helicopter's, the list read as the world reads it.
                let mut i = 0;
                while i < ee.u32_at(world + world_at::LISTS[4]) {
                    let e = ee.u32_at(ee.u32_at(world + world_at::LISTS[4] + 8) + 4 * i);
                    ee.call(post, &[u64::from(e - blade_at::POST)], &[dt], BUDGET);
                    i += 1;
                }
            }
            let mut i = 0;
            while i < ee.u32_at(node + blade_at::NODE_LISTENERS) {
                let arr = ee.u32_at(node + blade_at::NODE_LISTENERS + 8);
                let l = ee.u32_at(arr + 4 * i);
                ee.call(listen, &[u64::from(l - blade_at::LISTENER)], &[dt], BUDGET);
                if ee.v0() as u32 != 0 {
                    let n = ee.u32_at(node + blade_at::NODE_LISTENERS);
                    for j in i..n - 1 {
                        ee.set_u32(arr + 4 * j, ee.u32_at(arr + 4 * (j + 1)));
                    }
                    ee.set_u32(node + blade_at::NODE_LISTENERS, n - 1);
                } else {
                    i += 1;
                }
            }
            blades.post(F(dt));
            let case = case * 1000 + frame;
            same_blades(case, &ee, &blades);
            for (k, &wr) in wrappers.iter().enumerate() {
                let n = &blades.set.objects[k].node;
                let rows = [n.rows[0], n.rows[1], n.rows[2], n.origin].map(|r| r.to_bits());
                let game = quads::<4>(&ee, ee.u32_at(wr + blade_at::NODE) + wreck_at::ROWS).map(|r| r.to_bits());
                assert_eq!(rows, game, "case {case}: blade {k}'s node");
            }
        }
        flown += blades.set.bones.iter().filter(|b| b.body.enabled).count();
        put_back += blades.set.objects.iter().zip(&blades.set.bones).filter(|(o, b)| !b.body.enabled && o.place.to_bits() == b.body.position.to_bits()).count();
    }
    assert!(flown > cases / 2 && put_back > cases / 2 && rethrown > cases / 4,
            "{flown} blades flying at the end, {put_back} put back off the edge, {rethrown} thrown while out: the cases miss a branch");
    println!("thrown blades: {cases} throws of {frames} frames, bit-exact ({flown} flying at the end, {put_back} put back \
              off the edge, {rethrown} thrown while out)");
}

const ROTOR_PROBE: u32 = 0x458;
const PROBE_ROOT: u32 = 0x160;
const PROBE_COUNT: u32 = 0x168;
const PROBE_NODES: u32 = 0x16c;
const PROBE_SEGMENTS: u32 = 0x170;
/// Where the build's blocks are put: RAM the state leaves to nothing the build reads.
const HEAP: u32 = 0x01c0_0000;

/// **C17: the ray tree build is the original's.** The original's build (`ray_probe_build`) rebuilt on
/// the rotor's own probe over random lists of 1 to 12 rays — rays out of one hub as the rotor's are,
/// and rays with both ends loose, some on a lattice so that centres and ends tie — against
/// [`game_sim::kiowa::ray_tree`]: the root, every node's volume, centre, half extent and child words,
/// and the segments in the tree's order, word for word.
#[test]
fn the_ray_tree_build_is_the_originals() {
    use game_sim::kiowa::try_ray_tree;
    use game_sim::ray_probe::Segment;
    let Some(mut ee) = heli() else { return };
    let func = addr("ray_probe_build").unwrap();
    let probe = ee.u32_at(HELI + ROTOR_PROBE);
    let rays = ee.scratch(0);
    // The heap's lock spins in the interpreter: the build's blocks come off a bump heap, and its frees
    // (the probe's old tree, the build's lists) do nothing.
    let next = Rc::new(RefCell::new(HEAP));
    for name in ["mem_alloc", "operator_new", "operator_new_array"] {
        let next = next.clone();
        ee.hooks.insert(
            addr(name).unwrap(),
            Box::new(move |ee: &mut Ee| {
                let size = (ee.r(4) as u32 + 15) & !15;
                let mut at = next.borrow_mut();
                ee.set(2, u64::from(*at));
                *at += size;
            }),
        );
    }
    for name in ["free", "operator_delete", "operator_delete_array"] {
        ee.hooks.insert(addr(name).unwrap(), Box::new(|_: &mut Ee| {}));
    }
    let mut pick = Pick(0x00c1_7c17);
    let cases = 2000;
    let (mut deepest, mut unparted) = (0, 0);
    for case in 0..cases {
        let n = 1 + (pick.next() % 12) as usize;
        let lattice = pick.one_in(3);
        let hub = pick.one_in(2);
        let point = |pick: &mut Pick| {
            let mut c = || if lattice { (pick.next() % 5) as f32 * 2.5 - 5.0 } else { pick.range(-30.0, 30.0) };
            Vf::new(c(), c(), c(), 1.0)
        };
        let centre = point(&mut pick);
        let mut segments = Vec::new();
        while segments.len() < n {
            let from = if hub { centre } else { point(&mut pick) };
            let s = Segment { from, to: point(&mut pick) };
            // Two rays whose centres tie on every axis are a split the original leaves to lists it
            // never wrote; keep them apart.
            let mid = |s: &Segment| s.from.add(s.to).mul(Vf([0.5; 4])).0.map(f32::to_bits);
            if segments.iter().any(|t: &Segment| mid(t)[..3] == mid(&s)[..3]) {
                continue;
            }
            segments.push(s);
        }
        for (k, s) in segments.iter().enumerate() {
            for (j, v) in [s.from, s.to].into_iter().enumerate() {
                for (l, x) in v.0.into_iter().enumerate() {
                    ee.set_f32(rays + 0x20 * k as u32 + 0x10 * j as u32 + 4 * l as u32, x);
                }
            }
        }
        // A list no cut parts the original builds out of lists it never wrote: not a case.
        let Some(want) = try_ray_tree(&segments) else {
            unparted += 1;
            continue;
        };
        *next.borrow_mut() = HEAP;
        ee.call(func, &[u64::from(probe), n as u64, u64::from(rays), 0, 0], &[], 2_000_000);
        let what = format!("case {case}: {n} rays {segments:?}");
        assert_eq!(ee.u32_at(probe + PROBE_COUNT) as usize, n, "{what}");
        assert_eq!(ee.u16_at(probe + PROBE_ROOT), want.root, "the root, {what}");
        let nodes = ee.u32_at(probe + PROBE_NODES);
        for (k, node) in want.nodes.iter().enumerate() {
            let a = nodes + 0x24 * k as u32;
            let words: Vec<u32> = [0, 12, 16, 20, 24, 28, 32].iter().map(|&o| ee.u32_at(a + o)).collect();
            let mine: Vec<u32> = [node.volume].iter().chain(&node.centre).chain(&node.half_extent).map(|v| v.to_bits()).collect();
            assert_eq!(words, mine, "node {k}'s volume, centre and half extent, {what}");
            assert_eq!([ee.u16_at(a + 8), ee.u16_at(a + 10)], node.children, "node {k}'s children, {what}");
        }
        let at = ee.u32_at(probe + PROBE_SEGMENTS);
        for (k, s) in want.segments.iter().enumerate() {
            let got = |o: u32| (0..4).map(|l| ee.u32_at(at + 0x20 * k as u32 + o + 4 * l)).collect::<Vec<_>>();
            assert_eq!((got(0), got(0x10)), (s.from.0.map(f32::to_bits).to_vec(), s.to.0.map(f32::to_bits).to_vec()), "segment {k}, {what}");
        }
        deepest = deepest.max(want.nodes.len());
    }
    assert!(unparted < cases / 10, "{unparted} of {cases} lists no cut parts");
    println!("ray trees: {} lists of 1 to 12 rays built as the original builds them, up to {deepest} nodes ({unparted} no cut parts left out)", cases - unparted);
}

// --- S270: the throw's rows from the helicopter's own nodes ------------------------------------------

/// A node's local rows, its parent, its name.
mod node_at {
    pub const LOCAL: u32 = 0x40;
    pub const PARENT: u32 = 0x100;
    pub const NAME: u32 = 0x120;
}

fn node_name(ee: &Ee, node: u32) -> String {
    let at = ee.u32_at(node + node_at::NAME);
    (0..64).map(|k| ee.u8_at(at + k)).take_while(|&c| c != 0).map(|c| c as char).collect()
}

fn rotor_tree(kit: &Kit, skin: &str) -> Option<game_sim::kiowa::RotorTree> {
    let psg = machine_file(kit, skin)?;
    Some(game_sim::kiowa::RotorTree::read(&psg).unwrap_or_else(|e| panic!("{skin}: {e}")))
}

fn same_rows(what: &str, case: usize, got: &game_sim::math::Mat4, want: [Vf; 4]) {
    for r in 0..4 {
        assert_eq!(got.0[r].to_bits(), want[r].to_bits(), "case {case}: {what} row {r} is {:?} in the game, {:?} in the port", want[r].0, got.0[r].0);
    }
}

#[test]
fn the_rotor_tree_is_the_live_helicopters() {
    let Some(ee) = heli() else { return };
    let kit = Kit::load().unwrap();
    let root = ee.u32_at(HELI + wreck_at::MODEL);
    let spindle_rows = |t: &game_sim::kiowa::RotorTree| t.nodes[t.spindle].0.0[3];
    let mut first = None;
    // The `_ss` skins read, but their blade nodes go round the other way and their blades sit a few
    // bits off: they are not the flown helicopter's.
    for skin in ["helicopter\\anim\\kiowa_ss.psg", "helicopter\\anim\\kiowa2_ss.psg"] {
        let Some(_) = rotor_tree(&kit, skin) else { return };
    }
    for skin in ["helicopter\\anim\\kiowa.psg", "helicopter\\anim\\kiowa2.psg"] {
        let Some(tree) = rotor_tree(&kit, skin) else { return };
        // Both flown skins have the same chain to the blades, rows and all (the shell loads the first).
        let chain = |b: usize| {
            let mut out = vec![];
            let mut at = Some(b);
            while let Some(i) = at {
                out.push(i);
                at = tree.nodes[i].1;
            }
            out
        };
        let picked: Vec<_> = tree.blades.iter().flat_map(|&b| chain(b)).map(|i| tree.nodes[i].0.0.map(|v| v.to_bits())).collect();
        match &first {
            None => first = Some(picked),
            Some(f) => assert_eq!(f, &picked, "{skin}'s chain to the blades is not kiowa.psg's"),
        }
        for k in 0..4 {
            // The file's chain against the live one, node by node up to the root: the spindle's turn is
            // the rotor's, so only its position is the file's.
            let mut node = ee.u32_at(HELI + wreck_at::BLADE_NODES + 4 * k as u32);
            for (depth, &i) in chain(tree.blades[k]).iter().enumerate() {
                let what = format!("{skin}: blade {k}'s node {} ({depth} up)", node_name(&ee, node));
                if i == 0 {
                    assert_eq!(node, root, "{what} is the root in the file, not in the game");
                } else if i == tree.spindle {
                    assert_eq!(node, ee.u32_at(HELI + 0xd04), "{what}: the spindle is not the one the measure turns");
                    let want = quad(&ee, node + node_at::LOCAL + 0x30);
                    assert_eq!(spindle_rows(&tree).0[..3].iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                               want.0[..3].iter().map(|x| x.to_bits()).collect::<Vec<_>>(), "{what}'s position");
                } else {
                    same_rows(&what, 0, &tree.nodes[i].0, quads(&ee, node + node_at::LOCAL));
                }
                node = ee.u32_at(node + node_at::PARENT);
            }
            assert_eq!(node, 0, "{skin}: blade {k}'s live chain goes above the root");
        }
    }
    println!("the rotor tree: both flown skins' chains to the blades are the live helicopter's");
}

#[test]
fn the_throws_rows_are_the_originals() {
    use game_sim::kiowa::{Throw, spin_blades};
    let Some(mut ee) = heli() else { return };
    let kit = Kit::load().unwrap();
    let Some(tree) = rotor_tree(&kit, game_sim::kiowa::KIOWA_MODEL) else { return };
    let curve = the_curve_table(&ee);
    let b = ee.u32_at(HELI + at::BODY);
    let live = body_of(&ee, b);
    // The saved frame's own rows: the post, then the rotor's step, and nothing since.
    let angle = F(ee.f32_at(HELI + measure_at::ROTOR_ANGLE));
    let saved = Throw::of(&tree, &live, angle, &curve);
    let want = throw_of(&ee);
    same_rows("the saved model", 0, &saved.model, want.model.0);
    for k in 0..4 {
        same_rows(&format!("the saved blade {k}"), 0, &saved.blades[k], want.blades[k].0);
    }
    // Then generated poses and angles: the pose put as the body's post puts it (S. the biplane's
    // the_pose_is_put_like_the_original for the origin), the rotor's step, and each blade node's
    // lazy rebuild after them.
    let (spin, set_pose, rebuild) = (addr("kiowa_blade_spin").unwrap(), addr("transform_set_pose").unwrap(), addr("transform_rebuild").unwrap());
    let root = ee.u32_at(HELI + wreck_at::MODEL);
    let rotor = ee.u32_at(HELI + measure_at::ROTOR_NODE);
    let blades: [u32; 4] = std::array::from_fn(|k| ee.u32_at(HELI + wreck_at::BLADE_NODES + 4 * k as u32));
    let mut pick = Pick(0x5270_7a0c_e11d_0001);
    let mut hidden_cases = 0;
    for case in 0..CASES {
        let mut body = live.clone();
        let q = orientation(&mut pick);
        body.last_quat = Vf::new(q[0], q[1], q[2], q[3]);
        let size = if pick.one_in(4) { 30_000.0 } else { 3000.0 };
        body.last_position = vector(&mut pick, size);
        if pick.one_in(2) {
            body.cofg = Vf::new(pick.range(-3.0, 3.0), pick.range(-3.0, 3.0), pick.range(-3.0, 3.0), pick.range(-1.0, 1.0));
        }
        let (quat, origin) = body.pose();
        put(&mut ee, SCRATCH, quat);
        put(&mut ee, SCRATCH + 0x10, origin);
        ee.call(set_pose, &[u64::from(root), 0, u64::from(SCRATCH), u64::from(SCRATCH + 0x10)], &[], BUDGET);
        // A hidden rotor's step does nothing at all: its angle is the one the spindle was last turned
        // by (only this step and the helicopter's construction write it), so it is left as it is.
        let hidden = pick.one_in(4);
        set_bit(&mut ee, rotor + 3, 0x40, hidden);
        if !hidden {
            let a = match pick.next() % 5 {
                0 => 0.0,
                1 => -pick.range(0.0, 50.0),
                2 => pick.range(1e5, 1e7),
                _ => pick.range(0.0, 5000.0),
            };
            ee.set_f32(HELI + measure_at::ROTOR_ANGLE, a);
        }
        let mut angle = F(ee.f32_at(HELI + measure_at::ROTOR_ANGLE));
        let dt = if pick.one_in(2) { 1.0 / 30.0 } else { pick.range(0.0, 0.1) };
        ee.call(spin, &[u64::from(HELI)], &[dt], BUDGET);
        for &n in blades.iter().chain([root].iter()) {
            ee.call(rebuild, &[u64::from(n)], &[], BUDGET);
        }
        spin_blades(&mut angle, hidden, F(dt));
        same("the rotor's angle", case, angle, ee.f32_at(HELI + measure_at::ROTOR_ANGLE));
        let port = Throw::of(&tree, &body, angle, &curve);
        let want = throw_of(&ee);
        same_rows("the model", case, &port.model, want.model.0);
        for k in 0..4 {
            same_rows(&format!("blade {k}"), case, &port.blades[k], want.blades[k].0);
        }
        hidden_cases += usize::from(hidden);
    }
    println!("the throw's rows: the saved frame and {CASES} cases, bit-exact ({hidden_cases} hidden)");
}
