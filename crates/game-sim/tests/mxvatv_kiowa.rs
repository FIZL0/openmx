//! MD80c — MX vs. ATV's (SLUS-21104's) Kiowa step against the original.
//!
//! The proofs of tests/kiowa.rs, on SLUS-21104's helicopter: MD80a's Desert Oasis image of the Kiowa
//! (`context/mxvatv-ps2/md80a/kiowa.ee.bin`, model one) is the interpreter's RAM, each case's inputs
//! are written into the helicopter's own fields (and the rotor's and the aero model's, through the
//! pointers they hold), the original runs, and every field it writes is compared bit for bit with the
//! port's. Ten thousand generated cases each:
//!
//! 1. **the flight controller** (`Controller::step_mxvatv`), the forward sign handed in through the
//!    node call it is read from;
//! 2. **the rotor** (MX Unleashed's `Rotor::step`, unchanged);
//! 3. **the aerodynamics** (`Aero::step_mxvatv`), the force and moment recorded at the body's entries;
//! 4. **MXVehicle's wreck check** on the helicopter: the throw (`biplane::throw_mxvatv`), as
//!    tests/mxvatv_biplane.rs runs it on the plane;
//! 5. **the crash test's clocks** (`Crash::step_mxvatv`), its vtable calls hooked.
//!
//! 6. **the wreck and the unwreck** (MD63f): SLUS-21104's own (the Kiowa's vtable `+0x368`/`+0x36c`, with
//!    MXVehicle's halves) against `Crash::wreck_mxvatv` with `Wreckage::wreck_mx`, and
//!    `Wreckage::unwreck_mx` with `Kiowa::unwreck_controls`: the gravity they write above all.
//!
//! And **the reader** (`Phy::read_mxvatv`): MX vs. ATV's `kiowa.phy` read as model one and model two
//! against what the game loaded into `kiowa` (model one) and `kiowa3` (model two).
//!
//! The images and the file are game data, so every test skips without them. What is not proved here,
//! and why, is in the journal's last section (research/journal/2026-11-05-md80c-mxvatv-kiowa-step).

use game_sim::biplane::{Area, throw_mxvatv};
use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::kiowa::{
    Aero, AeroState, Airflow, Commands, Controller, Crash, CrashContact, Filter, Flight, Loop, Pad, Pedal, Phy, Rotor,
    RotorState, Strike,
};
use game_sim::rng::Rng;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::cell::RefCell;
use std::rc::Rc;

const CASES: usize = 10_000;
const BUDGET: u64 = 200_000;

/// The helicopters in MD80a's two images (research/oracle.txt # MD80a, # MD80c).
const HELI: u32 = 0x01ab_7d20;
const HELI_TWO: u32 = 0x014c_8f00;
/// SLUS-21104's `$gp`, and the Kiowa's vtable.
const GP: u64 = 0x003d_baf0;
const KIOWA_VTABLE: u32 = 0x003c_fad0;

/// A function or global by its research/oracle.txt name (# MD80, # MD80b, # MD80c).
fn f(name: &str) -> u32 {
    addr(name).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The helicopter's fields, by offset (# MD80c).
mod at {
    pub const BODY: u32 = 0xb8;
    pub const RECORD: u32 = 0xc8; // the collision record: +0x34 touching
    pub const GRAVITY: u32 = 0xe8; // +0 the controller's scale, +4 the one the throw halves
    pub const GATE: u32 = 0x308;
    pub const SPEED: u32 = 0x32c;
    pub const BETA: u32 = 0x330;
    pub const ALPHA: u32 = 0x334;
    pub const LOCAL: u32 = 0x350;
    pub const AIRSPEED: u32 = 0x358;
    pub const PITCH: u32 = 0x360;
    pub const ROLL: u32 = 0x368;
    pub const REASON: u32 = 0xc80;
    pub const CLOCK: u32 = 0xc84;
    pub const WRECK_TIME: u32 = 0xc88;
    pub const DOWN: u32 = 0xc8c;
    pub const CLIMB: u32 = 0x10f0;
    pub const DESCEND: u32 = 0x10f4;
    pub const FRACTION: u32 = 0x10f8;
    pub const PAD: u32 = 0x10fc; // lateral, longitudinal, pedal
    pub const OUT: u32 = 0x1108; // collective, lateral, longitudinal, pedal, filtered
    pub const LAST_STICK: u32 = 0x111c;
    pub const FLOWN: u32 = 0x1124;
    pub const HOLD: u32 = 0x1138;
    pub const HOLD_TIMER: u32 = 0x113c;
    pub const INTEGRAL: u32 = 0x1154;
    pub const SPEED_COMMAND: u32 = 0x1170;
    pub const MIN_WRECK_SPEED: u32 = 0x1194;
    pub const ROTOR: u32 = 0x11c0;
    pub const AERO: u32 = 0x11c4;
    pub const FILTER: u32 = 0x11d8;
    pub const MODEL: u32 = 0x1340;
}
/// The controller's keys, by offset, in [`Controller`]'s order (the rate commands after).
const CONTROLLER_KEYS: [u32; 23] = [
    0x1128, 0x112c, 0x1130, 0x1134, 0x1140, 0x1144, 0x1148, 0x114c, 0x1150, 0x1158, 0x115c, 0x1160, 0x1164, 0x1168,
    0x116c, 0x1174, 0x1178, 0x117c, 0x1180, 0x1184, 0x1188, 0x118c, 0x1190,
];
const MIN_RATE: u32 = 0x11a0;
const MAX_RATE: u32 = 0x11b0;

/// The rotor model's fields, by offset.
mod rotor_at {
    pub const MAX_PITCH: u32 = 0x3c; // lateral cyclic, longitudinal cyclic, collective, tail
    pub const BLADE: u32 = 0x4c; // lift curve slope, chord, blades, radius, radius^2, angular rate
    pub const MAX_MOMENT: u32 = 0xb0;
    pub const PITCH: u32 = 0xc0; // collective, tail
    pub const TILT: u32 = 0xd0;
    pub const THRUST: u32 = 0xf0;
    pub const FORCE: u32 = 0x100;
}

/// The aero model's keys the step reads, by offset, in [`Aero`]'s order; the pedals'; the outputs.
const AERO_KEYS: [u32; 20] =
    [0x24, 0x28, 0x2c, 0x30, 0x3c, 0x54, 0x5c, 0x68, 0x6c, 0x74, 0x78, 0x7c, 0x80, 0x84, 0x88, 0x90, 0x94, 0xf4, 0xfc, 0x100];
const AERO_PEDAL: u32 = 0x98; // CYbetaPedal, CnbetaPedal, CnrPedal, timer, time
const AERO_OUT: u32 = 0x128; // fade, lift, drag, side, roll, pitch, yaw

/// The rigid body's fields, and its vtable's force and moment entries.
const BODY_POS: u32 = 0x70;
const BODY_VEL: u32 = 0x80;
const BODY_RATES: u32 = 0x1d0;
const BODY_VTABLE: u32 = 0x110;
const FORCE_SLOT: u32 = 0x74;
const MOMENT_SLOT: u32 = 0x78;

/// The interpreter on one of MD80a's images, with `$gp` set, or why not.
fn image(name: &str, heli: u32) -> Option<Ee> {
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
    assert_eq!(ee.u32_at(heli + 4), KIOWA_VTABLE, "no Kiowa at {heli:#x} in {}", path.display());
    Some(ee)
}

fn heli() -> Option<Ee> {
    let ee = image("kiowa", HELI)?;
    // the parts are wired the way # MD80c reads them
    let (r, a) = (ee.u32_at(HELI + at::ROTOR), ee.u32_at(HELI + at::AERO));
    assert_eq!(ee.u32_at(r + 0x2c), HELI + at::OUT + 4, "the rotor is not wired to the controller");
    assert_eq!(ee.u32_at(a + 0x104), HELI + at::SPEED, "the aero model is not wired to the helicopter");
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
    fn button(&mut self) -> f32 {
        if self.one_in(3) { 1.0 } else { 0.0 }
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

fn g(ee: &Ee, a: u32) -> F {
    F(ee.f32_at(a))
}

fn vector(ee: &Ee, at: u32) -> [f32; 4] {
    [0, 4, 8, 12].map(|k| ee.f32_at(at + k))
}

fn quad(ee: &Ee, at: u32) -> Vf {
    let v = vector(ee, at);
    Vf::new(v[0], v[1], v[2], v[3])
}

fn set_quad(ee: &mut Ee, at: u32, v: [f32; 4]) {
    for (k, x) in v.into_iter().enumerate() {
        ee.set_f32(at + 4 * k as u32, x);
    }
}

fn same(what: &str, case: usize, got: F, want: f32) {
    assert_eq!(got.to_bits(), want.to_bits(), "case {case}: {what} is {want:e} in the game, {:e} in the port", got.to_f32());
}

fn same_lanes(what: &str, case: usize, got: Vf, want: [f32; 4], lanes: usize) {
    for k in 0..lanes {
        assert_eq!(got[k].to_bits(), want[k].to_bits(),
                   "case {case}: {what}[{k}] is {:e} in the game, {:e} in the port ({want:?} / {got:?})", want[k], got[k]);
    }
}

fn the_curve_table(ee: &Ee) -> CurveTable {
    let base = ee.u32_at(f("atv_curve_table_ptr"));
    let entries: Vec<[f32; 2]> =
        (0..CURVE_ENTRIES as u32).map(|k| [ee.f32_at(base + 8 * k), ee.f32_at(base + 8 * k + 4)]).collect();
    CurveTable::from_entries(&entries)
}

/// Hooks the body's force and moment entries to record each vector they are handed.
fn record_body(ee: &mut Ee, body: u32) -> Rc<RefCell<Vec<[f32; 4]>>> {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let vtable = ee.u32_at(body + BODY_VTABLE);
    for slot in [FORCE_SLOT, MOMENT_SLOT] {
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

fn pad_of(ee: &Ee) -> Pad {
    let h = |o: u32| g(ee, HELI + o);
    Pad { climb: h(at::CLIMB), descend: h(at::DESCEND), lateral: h(at::PAD), longitudinal: h(at::PAD + 4), pedal: h(at::PAD + 8) }
}

// ---------------------------------------------------------------------------------------------------
// 1 — the flight controller
// ---------------------------------------------------------------------------------------------------

fn controller_of(ee: &Ee) -> Controller {
    let k = CONTROLLER_KEYS.map(|o| g(ee, HELI + o));
    let v = |o: u32| [g(ee, HELI + o), g(ee, HELI + o + 4), g(ee, HELI + o + 8)];
    Controller {
        lateral_cyclic_gain: k[0],
        longitudinal_cyclic_gain: k[1],
        pedal_gain: k[2],
        max_altitude: k[3],
        hold_delay: k[4],
        collective_gain: k[5],
        climb_rate: k[6],
        descent_rate: k[7],
        altitude_hold_gain: k[8],
        integrator_gain: k[9],
        min_collective: k[10],
        max_pitch: k[11],
        max_roll: k[12],
        pitch_gain: k[13],
        roll_gain: k[14],
        speed_command_rate: k[15],
        min_speed_command: k[16],
        max_speed_command: k[17],
        speed_gain: k[18],
        min_feed_forward_speed: k[19],
        min_feed_forward_scale: k[20],
        max_feed_forward_speed: k[21],
        max_feed_forward_scale: k[22],
        min_rate: v(MIN_RATE),
        max_rate: v(MAX_RATE),
    }
}

fn loop_of(ee: &Ee) -> Loop {
    let h = |o: u32| g(ee, HELI + o);
    let filter = ee.u32_at(HELI + at::FILTER);
    Loop {
        speed_command: h(at::SPEED_COMMAND),
        last_longitudinal: h(at::LAST_STICK),
        hold_altitude: h(at::HOLD),
        hold_timer: h(at::HOLD_TIMER),
        integral: h(at::INTEGRAL),
        filter: Filter { value: g(ee, filter), time: g(ee, filter + 4), step: g(ee, filter + 8) },
    }
}

fn commands_of(ee: &Ee) -> Commands {
    let h = |o: u32| g(ee, HELI + at::OUT + o);
    Commands {
        collective: h(0),
        lateral: h(4),
        longitudinal: h(8),
        pedal: h(12),
        filtered: h(16),
        altitude_fraction: g(ee, HELI + at::FRACTION),
    }
}

fn flight_of(ee: &Ee) -> Flight {
    let body = ee.u32_at(HELI + at::BODY);
    let h = |o: u32| g(ee, HELI + o);
    Flight {
        flown: ee.u32_at(HELI + at::FLOWN) != 0,
        pilot_flag: ee.u32_at(ee.u32_at(HELI + at::RECORD) + 0x34) != 0,
        height: g(ee, body + BODY_POS + 4),
        velocity: [g(ee, body + BODY_VEL), g(ee, body + BODY_VEL + 4), g(ee, body + BODY_VEL + 8)],
        rates: quad(ee, body + BODY_RATES),
        airspeed: h(at::AIRSPEED),
        pitch: h(at::PITCH),
        roll: h(at::ROLL),
        gravity_scale: g(ee, ee.u32_at(HELI + at::GRAVITY)),
        high_speed_aero: F::ZERO,
    }
}

#[test]
fn the_mxvatv_flight_controller_matches_the_original() {
    let Some(mut ee) = heli() else { return };
    let table = the_curve_table(&ee);
    let body = ee.u32_at(HELI + at::BODY);
    let record = ee.u32_at(HELI + at::RECORD);
    let filter = ee.u32_at(HELI + at::FILTER);
    let gravity = ee.u32_at(HELI + at::GRAVITY);
    // the forward component the controller reads off the node, handed in
    let forward = Rc::new(RefCell::new(0.0f32));
    let fw = forward.clone();
    ee.hooks.insert(f("atv_node_local"), Box::new(move |ee| {
        let out = ee.r(4) as u32;
        let z = *fw.borrow();
        for (k, v) in [0.0, 0.0, z, 0.0].into_iter().enumerate() {
            ee.set_f32(out + 4 * k as u32, v);
        }
    }));
    let keys: Vec<(u32, f32)> = CONTROLLER_KEYS
        .iter()
        .copied()
        .chain((0..3).map(|k| MIN_RATE + 4 * k))
        .chain((0..3).map(|k| MAX_RATE + 4 * k))
        .map(|o| (HELI + o, ee.f32_at(HELI + o)))
        .collect();
    let filter_time = ee.f32_at(filter + 4);
    let max_altitude = ee.f32_at(HELI + 0x1134);
    let mut pick = Pick(0x6a09_e667_f3bc_c908);
    for case in 0..CASES {
        ee.set_u32(HELI + at::FLOWN, u32::from(!pick.one_in(16)));
        ee.set_u32(record + 0x34, u32::from(pick.one_in(4)));
        for &(a, x) in &keys {
            let v = pick.around(x);
            ee.set_f32(a, v);
        }
        if pick.one_in(8) {
            // the feed-forward's two speeds the same
            ee.set_f32(HELI + 0x118c, ee.f32_at(HELI + 0x1184));
        }
        ee.set_f32(HELI + at::CLIMB, pick.button());
        ee.set_f32(HELI + at::DESCEND, pick.button());
        for k in 0..3 {
            let v = if pick.one_in(2) { 0.0 } else { pick.stick(-1.0) };
            ee.set_f32(HELI + at::PAD + 4 * k, v);
        }
        let v = if pick.one_in(3) { 0.0 } else { pick.range(-1.0, 1.0) };
        ee.set_f32(HELI + at::LAST_STICK, v);
        ee.set_f32(HELI + at::SPEED_COMMAND, pick.range(-60.0, 120.0));
        ee.set_f32(HELI + at::HOLD, pick.range(0.0, 600.0));
        let v = match pick.next() % 3 {
            0 => 0.0,
            1 => -pick.range(0.0, 0.1),
            _ => pick.range(0.0, 2.0),
        };
        ee.set_f32(HELI + at::HOLD_TIMER, v);
        ee.set_f32(HELI + at::INTEGRAL, pick.range(-0.2, 1.2));
        ee.set_f32(filter, pick.range(-0.2, 1.2));
        ee.set_f32(filter + 4, pick.around(filter_time).abs());
        let height = if pick.one_in(4) { pick.around(max_altitude) } else { pick.range(100.0, 700.0) };
        ee.set_f32(body + BODY_POS + 4, height);
        for k in 0..3 {
            let v = if pick.one_in(8) { 0.0 } else { pick.range(-100.0, 100.0) };
            ee.set_f32(body + BODY_VEL + 4 * k, v);
            ee.set_f32(body + BODY_RATES + 4 * k, pick.range(-2.0, 2.0));
        }
        let airspeed = match pick.next() % 4 {
            0 => pick.range(-5.0, 5.0),
            _ => pick.range(-40.0, 120.0),
        };
        ee.set_f32(HELI + at::AIRSPEED, airspeed);
        ee.set_f32(HELI + at::PITCH, pick.range(-1.0, 1.0));
        ee.set_f32(HELI + at::ROLL, angle(&mut pick));
        ee.set_f32(gravity, if pick.one_in(2) { 1.0 } else { pick.range(0.0, 3.0) });
        let fwd = match pick.next() % 4 {
            0 => 0.0,
            1 => -pick.range(0.0, 50.0),
            _ => pick.range(-50.0, 50.0),
        };
        *forward.borrow_mut() = fwd;
        let dt = if pick.one_in(2) { 1.0 / 30.0 / 4.0 } else { pick.range(0.0, 0.05) };

        let (c, pad, at) = (controller_of(&ee), pad_of(&ee), flight_of(&ee));
        let (mut state, mut out) = (loop_of(&ee), commands_of(&ee));
        ee.call(f("atv_kiowa_controller_step"), &[u64::from(HELI)], &[dt], BUDGET);
        c.step_mxvatv(&pad, &at, F(fwd), &mut state, &mut out, &table, F(dt));

        let (want, want_out) = (loop_of(&ee), commands_of(&ee));
        for (name, got, want) in [
            ("the collective", out.collective, want_out.collective),
            ("the lateral cyclic", out.lateral, want_out.lateral),
            ("the longitudinal cyclic", out.longitudinal, want_out.longitudinal),
            ("the pedal", out.pedal, want_out.pedal),
            ("the filtered target", out.filtered, want_out.filtered),
            ("the altitude fraction", out.altitude_fraction, want_out.altitude_fraction),
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
    println!("the flight controller: {CASES} cases, bit-exact");
}

// ---------------------------------------------------------------------------------------------------
// 2 — the rotor
// ---------------------------------------------------------------------------------------------------

fn rotor_of(ee: &Ee, r: u32) -> Rotor {
    let h = |o: u32| g(ee, r + o);
    Rotor {
        max_lateral_cyclic: h(rotor_at::MAX_PITCH),
        max_longitudinal_cyclic: h(rotor_at::MAX_PITCH + 4),
        max_collective: h(rotor_at::MAX_PITCH + 8),
        max_tail_pitch: h(rotor_at::MAX_PITCH + 12),
        lift_curve_slope: h(rotor_at::BLADE),
        blade_chord: h(rotor_at::BLADE + 4),
        blades: h(rotor_at::BLADE + 8),
        radius: h(rotor_at::BLADE + 12),
        radius_squared: h(rotor_at::BLADE + 16),
        angular_rate: h(rotor_at::BLADE + 20),
        max_moment: [h(rotor_at::MAX_MOMENT), h(rotor_at::MAX_MOMENT + 4), h(rotor_at::MAX_MOMENT + 8)],
    }
}

fn rotor_state_of(ee: &Ee, r: u32) -> RotorState {
    let h = |o: u32| g(ee, r + o);
    RotorState {
        collective_pitch: h(rotor_at::PITCH),
        tail_pitch: h(rotor_at::PITCH + 4),
        tilt: [h(rotor_at::TILT), h(rotor_at::TILT + 4), h(rotor_at::TILT + 8)],
        thrust: quad(ee, r + rotor_at::THRUST),
        force: quad(ee, r + rotor_at::FORCE),
    }
}

#[test]
fn the_mxvatv_rotor_is_mx_unleashed_s() {
    let Some(mut ee) = heli() else { return };
    let table = the_curve_table(&ee);
    let r = ee.u32_at(HELI + at::ROTOR);
    // the four commands' pointers: lateral, longitudinal, collective, pedal
    for (k, want) in [4u32, 8, 0, 12].into_iter().enumerate() {
        assert_eq!(ee.u32_at(r + 0x2c + 4 * k as u32), HELI + at::OUT + want, "the rotor's command pointer {k}");
    }
    let body = ee.u32_at(HELI + at::BODY);
    let seen = record_body(&mut ee, body);
    let params: Vec<(u32, f32)> = (0..4)
        .map(|k| r + rotor_at::MAX_PITCH + 4 * k)
        .chain((0..6).map(|k| r + rotor_at::BLADE + 4 * k))
        .chain((0..3).map(|k| r + rotor_at::MAX_MOMENT + 4 * k))
        .map(|a| (a, ee.f32_at(a)))
        .collect();
    let mut pick = Pick(0xd80c_0000_0707_0002);
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
        ee.call(f("atv_kiowa_rotor_step"), &[u64::from(r)], &[1.0 / 120.0], BUDGET);
        let (force, moment) = rotor.step(&c, &mut state, &table);

        let want = rotor_state_of(&ee, r);
        same("the collective pitch", case, state.collective_pitch, want.collective_pitch.to_f32());
        same("the tail pitch", case, state.tail_pitch, want.tail_pitch.to_f32());
        for k in [0, 2] {
            same("the tilt", case, state.tilt[k], want.tilt[k].to_f32());
        }
        same_lanes("the thrust", case, state.thrust, [0, 1, 2, 3].map(|k| want.thrust[k]), 4);
        let seen = seen.borrow();
        assert_eq!(seen.len(), 2, "case {case}: the rotor handed the body {} vectors", seen.len());
        same_lanes("the force", case, force, seen[0], 4);
        same_lanes("the moment", case, moment, seen[1], 3);
    }
    println!("the rotor: {CASES} cases, bit-exact");
}

// ---------------------------------------------------------------------------------------------------
// 3 — the aerodynamics
// ---------------------------------------------------------------------------------------------------

fn aero_of(ee: &Ee, a: u32) -> Aero {
    let k = AERO_KEYS.map(|o| g(ee, a + o));
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

fn pedal_of(ee: &Ee, a: u32) -> Pedal {
    let h = |k: u32| g(ee, a + AERO_PEDAL + 4 * k);
    Pedal { cy_beta: h(0), cn_beta: h(1), cn_r: h(2), timer: h(3), time: h(4) }
}

fn aero_state_of(ee: &Ee, a: u32) -> AeroState {
    let h = |o: u32| g(ee, a + AERO_OUT + o);
    AeroState { fade: h(0), lift: h(4), drag: h(8), side: h(12), roll: h(16), pitch: h(20), yaw: h(24) }
}

#[test]
fn the_mxvatv_aerodynamics_match_the_original() {
    let Some(mut ee) = heli() else { return };
    let a = ee.u32_at(HELI + at::AERO);
    let body = ee.u32_at(a + 0x124);
    assert_eq!(body, ee.u32_at(HELI + at::BODY), "the aero model's body");
    for (off, want) in [
        (0x104, HELI + at::SPEED),
        (0x108, HELI + at::ALPHA),
        (0x10c, HELI + at::BETA),
        (0x110, HELI + at::PAD),
        (0x114, HELI + at::PAD + 4),
        (0x118, HELI + at::PAD + 8),
        (0x11c, body + BODY_RATES),
        (0x120, HELI),
    ] {
        assert_eq!(ee.u32_at(a + off), want, "the aero model's pointer at +{off:#x}");
    }
    let seen = record_body(&mut ee, body);
    let keys: Vec<(u32, f32)> =
        AERO_KEYS.iter().chain(&[AERO_PEDAL, AERO_PEDAL + 4, AERO_PEDAL + 8, AERO_PEDAL + 16]).map(|&o| (a + o, ee.f32_at(a + o))).collect();
    let mut pick = Pick(0xbb67_ae85_84ca_a73b);
    for case in 0..CASES {
        for &(at, x) in &keys {
            let v = pick.around(x);
            ee.set_f32(at, v);
        }
        if pick.one_in(8) {
            ee.set_f32(a + 0x100, ee.f32_at(a + 0xfc));
        }
        if pick.one_in(8) {
            ee.set_f32(a + AERO_PEDAL + 16, 0.0);
        }
        let timer = match pick.next() % 4 {
            0 => 0.0,
            1 => -pick.range(0.0, 0.1),
            _ => pick.range(0.0, 6.0),
        };
        ee.set_f32(a + AERO_PEDAL + 12, timer);
        let alpha = if pick.one_in(4) { pick.range(-3.0, 3.0) } else { pick.range(-0.5, 0.5) };
        let beta = if pick.one_in(4) { pick.range(-3.0, 3.0) } else { pick.range(-0.6, 0.6) };
        let speed = match pick.next() % 8 {
            0 => pick.range(-2.0, 2.0),
            1 => pick.range(-60.0, 0.0),
            _ => pick.range(0.0, 120.0),
        };
        ee.set_f32(HELI + at::ALPHA, alpha);
        ee.set_f32(HELI + at::BETA, beta);
        ee.set_f32(HELI + at::SPEED, speed);
        for k in 0..3 {
            let v = if pick.one_in(2) { 0.0 } else { pick.stick(-1.0) };
            ee.set_f32(HELI + at::PAD + 4 * k, v);
        }
        for k in 0..3 {
            ee.set_f32(body + BODY_RATES + 4 * k, pick.range(-3.0, 3.0));
            ee.set_f32(body + BODY_VEL + 4 * k, pick.range(-100.0, 100.0));
            ee.set_f32(HELI + at::LOCAL + 4 * k, pick.range(-100.0, 100.0));
        }
        ee.set_f32(HELI + at::LOCAL + 12, if pick.one_in(2) { 1.0 } else { pick.range(-2.0, 2.0) });
        let dt = if pick.one_in(2) { 1.0 / 30.0 / 4.0 } else { pick.range(0.0, 0.05) };

        let (aero, mut pedal, pad) = (aero_of(&ee, a), pedal_of(&ee, a), pad_of(&ee));
        let air = Airflow {
            speed: F(speed),
            alpha: F(alpha),
            beta: F(beta),
            velocity: quad(&ee, body + BODY_VEL),
            quat: quad(&ee, body + 0x170),
            rates: quad(&ee, body + BODY_RATES),
        };
        let local = quad(&ee, HELI + at::LOCAL);
        let mut state = AeroState::default();
        seen.borrow_mut().clear();
        ee.call(f("atv_kiowa_aero_step"), &[u64::from(a)], &[dt], BUDGET);
        let (force, moment) = aero.step_mxvatv(&mut pedal, &pad, &air, local, &mut state, F(dt));

        let want = aero_state_of(&ee, a);
        for (name, got, want) in [
            ("the fade", state.fade, want.fade),
            ("CL", state.lift, want.lift),
            ("CD", state.drag, want.drag),
            ("CY", state.side, want.side),
            ("Cl", state.roll, want.roll),
            ("CM", state.pitch, want.pitch),
            ("Cn", state.yaw, want.yaw),
            ("the pedal timer", pedal.timer, pedal_of(&ee, a).timer),
        ] {
            same(name, case, got, want.to_f32());
        }
        let pushed = seen.borrow();
        assert_eq!(pushed.len(), 2, "case {case}: the aero model pushed the body {} times", pushed.len());
        same_lanes("the force", case, force, pushed[0], 4);
        same_lanes("the moment", case, moment, pushed[1], 3);
    }
    println!("the aerodynamics: {CASES} cases, bit-exact");
}

// ---------------------------------------------------------------------------------------------------
// 4 — MXVehicle's wreck check on the helicopter
// ---------------------------------------------------------------------------------------------------

#[test]
fn the_mxvatv_kiowa_throw_matches_the_original() {
    let Some(mut ee) = heli() else { return };
    let body = ee.u32_at(HELI + at::BODY);
    let gravity = ee.u32_at(HELI + at::GRAVITY) + 4;
    assert_eq!(ee.u32_at(ee.u32_at(HELI + 4) + 0x16c), f("atv_vehicle_wreck_check"), "the Kiowa's wreck check slot");
    let extents = Rc::new(RefCell::new((0.0f32, 0.0f32)));
    let x = extents.clone();
    ee.hooks.insert(f("atv_terrain_edge"), Box::new(move |ee| {
        let (w, d) = *x.borrow();
        let (a2, a3) = (ee.r(6) as u32, ee.r(7) as u32);
        ee.set_f32(a2, w);
        ee.set_f32(a3, d);
        ee.set(2, 1);
    }));
    let rng = Rc::new(RefCell::new(Rng::from_seed(0x0d80_c0c0)));
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
        ee.set_u32(HELI + at::GATE, 1);

        let mut port_rng = rng.borrow().clone();
        ee.call(f("atv_vehicle_wreck_check"), &[u64::from(HELI)], &[], BUDGET);
        assert_eq!(ee.r(2), 1, "case {case}: the wreck check did not throw");
        let mut v = Vf::new(velocity[0], velocity[1], velocity[2], velocity[3]);
        let mut spin = Vf::new(rates[0], rates[1], rates[2], rates[3]);
        let mut s = F(scale);
        let area = Area { width: F(w), depth: F(d) };
        throw_mxvatv(&area, position, &mut v, &mut spin, &mut s, &mut port_rng);
        same_lanes("the velocity", case, v, vector(&ee, body + BODY_VEL), 4);
        same_lanes("the spin", case, spin, vector(&ee, body + BODY_RATES), 4);
        same("the gravity scale", case, s, ee.f32_at(gravity));
        assert_eq!(port_rng.state(), rng.borrow().state(), "case {case}: the port drew a different count of numbers");
    }
    println!("the throw: {CASES} cases, bit-exact (gate word in the image: {})", ee.u32_at(HELI + at::GATE));
}

// ---------------------------------------------------------------------------------------------------
// 5 — the crash test's clocks
// ---------------------------------------------------------------------------------------------------

/// The crash test on a wrecked helicopter (reasons 1 and 13: the reason-0 path asks the terrain and
/// the rotor's collision object, which are not set up here), with the wreck check hooked to say no
/// and the blades, the put-back and the sound hooked to record that they were asked for.
#[test]
fn the_mxvatv_crash_clocks_match_the_original() {
    let Some(mut ee) = heli() else { return };
    let vtable = ee.u32_at(HELI + 4);
    let body = ee.u32_at(HELI + at::BODY);
    let record = ee.u32_at(HELI + at::RECORD);
    let asked = Rc::new(RefCell::new(Vec::new()));
    for (slot, name) in [(0x16c, "check"), (0x478, "blades"), (0x36c, "recover"), (0x368, "wreck")] {
        let a = asked.clone();
        ee.hooks.insert(ee.u32_at(vtable + slot), Box::new(move |ee| {
            a.borrow_mut().push(name);
            ee.set(2, 0);
        }));
    }
    let a = asked.clone();
    ee.hooks.insert(f("atv_post_event"), Box::new(move |ee| {
        a.borrow_mut().push("sound");
        ee.set(2, 0);
    }));
    let mut pick = Pick(0x510e_527f_ade6_82d1);
    let mut counts = [0usize; 3];
    for case in 0..CASES {
        let reason = if pick.one_in(2) { 1 } else { 13 };
        let down = pick.one_in(2);
        let wreck_time = pick.range(0.0, 8.0);
        let clock = match pick.next() % 4 {
            0 => 2.5,
            1 => wreck_time,
            _ => pick.range(0.0, 9.0),
        };
        let touching = pick.one_in(2);
        let dt = if pick.one_in(2) { 1.0 / 30.0 } else { pick.range(0.0, 0.1) };
        ee.set_u32(HELI + at::REASON, reason);
        ee.set_u32(HELI + at::DOWN, u32::from(down));
        ee.set_f32(HELI + at::WRECK_TIME, wreck_time);
        ee.set_f32(HELI + at::CLOCK, clock);
        ee.set_u32(record + 0x34, u32::from(touching));
        let velocity = [0; 4].map(|_| pick.range(-100.0, 100.0));
        set_quad(&mut ee, body + BODY_VEL, velocity);
        asked.borrow_mut().clear();

        let mut crash = Crash { reason, down, wreck_time: F(wreck_time), clock: F(clock), ..Crash::default() };
        let mut v = Vf::new(velocity[0], velocity[1], velocity[2], velocity[3]);
        let (mut spin, mut scale, mut rng) = (Vf::default(), F::ONE, Rng::from_seed(1));
        let c = CrashContact { touching, ignored: false, speed: F::ZERO };
        ee.call(f("atv_kiowa_crash_test"), &[u64::from(HELI)], &[dt], BUDGET);
        let out = crash.step_mxvatv(&c, None, Vf::default(), &mut v, &mut spin, &mut scale, &mut rng, F(dt), Strike::default);

        let seen = asked.borrow();
        assert_eq!(seen.first() == Some(&"check"), reason != 13, "case {case}: the wreck check ({seen:?})");
        assert_eq!(seen.contains(&"blades"), out.debris, "case {case}: the blades ({seen:?})");
        assert_eq!(seen.contains(&"sound"), out.debris, "case {case}: the sound ({seen:?})");
        assert_eq!(seen.contains(&"recover"), out.recover, "case {case}: the put-back ({seen:?})");
        assert!(!seen.contains(&"wreck") && out.wreck.is_none(), "case {case}: a wreck ({seen:?})");
        same("the clock", case, crash.clock, ee.f32_at(HELI + at::CLOCK));
        assert_eq!(u32::from(crash.down), ee.u32_at(HELI + at::DOWN), "case {case}: the down flag");
        same_lanes("the velocity", case, v, vector(&ee, body + BODY_VEL), 4);
        counts[0] += usize::from(out.debris);
        counts[1] += usize::from(out.recover);
        counts[2] += usize::from(reason == 13);
    }
    println!("the crash clocks: {CASES} cases ({} at reason 13, {} blades, {} put back), bit-exact", counts[2], counts[0], counts[1]);
}

// ---------------------------------------------------------------------------------------------------
// 6 — the wreck and the unwreck (MD63f)
// ---------------------------------------------------------------------------------------------------

/// The Kiowa's words the wreck and the unwreck touch (research/oracle.txt # MD63f).
mod wreck_at {
    pub const WRECK_GRAVITY: u32 = 0xc90;
    pub const WRECK_DAMPING: u32 = 0xca0;
    pub const WRECK_INERTIA: u32 = 0xcb0;
    pub const INERTIA: u32 = 0xcc0;
    pub const KIND: u32 = 0xbbc;
    pub const SOUND: u32 = 0x11dc;
    /// The rotor's four nodes (the first gates the unwreck's node swap), `+0x1220` and the wrecked
    /// blades'.
    pub const NODES: [u32; 6] = [0x1200, 0x1204, 0x1208, 0x120c, 0x1220, 0x1224];
    pub const BLADES: u32 = 0x132c;
    /// In the gravity block (`*(+0xe8)`): the force.
    pub const FORCE: u32 = 0x40;
    /// In the body: the mass, the damping, the inertia and its inverse, the angular damping.
    pub const MASS: u32 = 0x18;
    pub const DAMPING: u32 = 0x30;
    pub const BODY_INERTIA: u32 = 0x130;
    pub const INV_INERTIA: u32 = 0x140;
    pub const ANG_DAMPING: u32 = 0x150;
}

fn node_hidden(ee: &Ee, node: u32) -> bool {
    ee.u8_at(node + 3) & 0x40 != 0
}

/// 0, -0, a stock value, or anything in ±`x`.
fn edgy(pick: &mut Pick, stock: f32, x: f32) -> f32 {
    match pick.next() % 8 {
        0 => 0.0,
        1 => -0.0,
        2 => stock,
        _ => pick.range(-x, x),
    }
}

fn edgy_quad(pick: &mut Pick, x: f32) -> [f32; 4] {
    [0; 4].map(|_| edgy(pick, 1.0, x))
}

/// SLUS-21104's wreck (`0x004d3f90`) and unwreck (`0x004d40a0`) on generated helicopters, every word
/// they write against the port's: above all the gravity block's force, which the wreck sets from
/// `WreckGravity` and the body's mass and the unwreck from the gravity scale and the mass — MX
/// Unleashed's formula, not the load's `-GravityScale * Weight`. Their calls out are hooked: the sound,
/// the blade throw, the put-back (vtable `+0xe8`), the blades' stop, the rumble's test and the event
/// post; the body's damping and inertia setters run as themselves.
#[test]
fn the_mxvatv_wreck_and_unwreck_are_the_originals() {
    use game_sim::biplane::{Body, THROWN};
    use game_sim::kiowa::{Crashed, Kiowa, Measure, Nodes, Ride, Throw, Wreckage};
    use game_sim::math::Mat4;
    let Some(mut ee) = heli() else { return };
    let vtable = ee.u32_at(HELI + 4);
    assert_eq!(ee.u32_at(vtable + 0x368), 0x004d_3f90, "the Kiowa's wreck slot");
    assert_eq!(ee.u32_at(vtable + 0x36c), 0x004d_40a0, "the Kiowa's unwreck slot");
    assert_eq!(ee.u32_at(HELI + wreck_at::KIND), 3, "the Kiowa's kind word (the damping gate)");
    let body = ee.u32_at(HELI + at::BODY);
    let block = ee.u32_at(HELI + at::GRAVITY);
    let asked = Rc::new(RefCell::new(Vec::<String>::new()));
    let hook = |ee: &mut Ee, at: u32, name: &'static str| {
        let a = asked.clone();
        ee.hooks.insert(at, Box::new(move |ee| {
            a.borrow_mut().push(name.to_string());
            ee.set(2, 0);
        }));
    };
    let sound_vt = ee.u32_at(ee.u32_at(HELI + wreck_at::SOUND) + 4);
    let at = ee.u32_at(sound_vt + 0x94);
    hook(&mut ee, at, "sound on");
    let at = ee.u32_at(sound_vt + 0x90);
    hook(&mut ee, at, "sound off");
    let at = ee.u32_at(vtable + 0x478);
    hook(&mut ee, at, "blades thrown");
    let at = ee.u32_at(vtable + 0xe8);
    hook(&mut ee, at, "put back");
    let stop = ee.u32_at(ee.u32_at(ee.u32_at(HELI + wreck_at::BLADES) + 4) + 0x134);
    for k in 0..4 {
        let b = ee.u32_at(HELI + wreck_at::BLADES + 4 * k);
        assert_eq!(ee.u32_at(ee.u32_at(b + 4) + 0x134), stop, "blade {k}'s stop slot");
    }
    hook(&mut ee, stop, "blade stopped");
    // the rumble's test, which the player's helicopter reaches; 0, no rumble
    hook(&mut ee, 0x0051_a250, "rumble test");
    let a = asked.clone();
    ee.hooks.insert(f("atv_post_event"), Box::new(move |ee| {
        a.borrow_mut().push(format!("event {} {}", ee.r(6), ee.r(7)));
        ee.set(2, 0);
    }));
    let nodes: [u32; 6] = wreck_at::NODES.map(|o| ee.u32_at(HELI + o));
    assert!(nodes.iter().all(|&n| n != 0) && ee.u32_at(HELI + wreck_at::BLADES) != 0, "the nodes and blades are built");

    let mut pick = Pick(0x9b05_688c_2b3e_6c1f);
    let mut counts = [0usize; 3];
    for case in 0..CASES {
        let unwreck = pick.one_in(2);
        let reason = if unwreck {
            [0, 1, THROWN, (pick.next() % 20) as u32][(pick.next() % 4) as usize]
        } else {
            [1, THROWN, (pick.next() % 20) as u32][(pick.next() % 3) as usize]
        };
        let before = [0, 1, THROWN, (pick.next() % 20) as u32][(pick.next() % 4) as usize];
        let clock = pick.range(0.0, 6.0);
        let down = pick.one_in(2);
        let mass = match pick.next() % 10 {
            0 => 0.0,
            1 => pick.range(-50.0, 0.0),
            2 => 42.546_58,
            _ => pick.range(1.0, 200.0),
        };
        let wreck_gravity = edgy(&mut pick, 3.0, 6.0);
        let gravity_scale = edgy(&mut pick, 1.0, 4.0);
        let throw_scale = edgy(&mut pick, 0.5, 2.0);
        let wreck_damping = edgy_quad(&mut pick, 2.0);
        let wreck_inertia = edgy_quad(&mut pick, 2.0);
        let inertia = [edgy(&mut pick, 10795.494, 20000.0), edgy(&mut pick, 10212.026, 20000.0), edgy(&mut pick, 1301.4453, 20000.0), edgy(&mut pick, 1.0, 3.0)];
        let rates = edgy_quad(&mut pick, 6.0);
        let hidden: [bool; 6] = [0; 6].map(|_| pick.one_in(2));
        let pad = [0; 7].map(|_| pick.range(-1.0, 1.0));

        ee.set_u32(HELI + at::REASON, if unwreck { reason } else { before });
        ee.set_f32(HELI + at::CLOCK, clock);
        ee.set_u32(HELI + at::DOWN, u32::from(down));
        ee.set_f32(body + wreck_at::MASS, mass);
        ee.set_f32(HELI + wreck_at::WRECK_GRAVITY, wreck_gravity);
        ee.set_f32(block, gravity_scale);
        ee.set_f32(block + 4, throw_scale);
        set_quad(&mut ee, HELI + wreck_at::WRECK_DAMPING, wreck_damping);
        set_quad(&mut ee, HELI + wreck_at::WRECK_INERTIA, wreck_inertia);
        set_quad(&mut ee, HELI + wreck_at::INERTIA, inertia);
        set_quad(&mut ee, body + BODY_RATES, rates);
        for o in [wreck_at::DAMPING, wreck_at::BODY_INERTIA, wreck_at::INV_INERTIA, wreck_at::ANG_DAMPING] {
            let q = edgy_quad(&mut pick, 9.0);
            set_quad(&mut ee, body + o, q);
        }
        let force = edgy_quad(&mut pick, 5000.0);
        set_quad(&mut ee, block + wreck_at::FORCE, force);
        for (n, h) in nodes.iter().zip(hidden) {
            // byte +3's 0x40, the top byte of the word
            let word = ee.u32_at(*n) & !0x4000_0000 | if h { 0x4000_0000 } else { 0 };
            ee.set_u32(*n, word);
        }
        for (k, o) in [at::CLIMB, at::DESCEND, at::FRACTION, at::PAD, at::PAD + 4, at::PAD + 8, at::HOLD].into_iter().enumerate() {
            ee.set_f32(HELI + o, pad[k]);
        }
        asked.borrow_mut().clear();

        let damping = quad(&ee, body + wreck_at::DAMPING);
        let ang_damping = quad(&ee, body + wreck_at::ANG_DAMPING);
        let inv_inertia = quad(&ee, body + wreck_at::INV_INERTIA);
        let mut ride = Ride {
            body: Body { rates: Vf::new(rates[0], rates[1], rates[2], rates[3]), damping, ang_damping, inv_inertia, ..Body::default() },
            measure: Measure::default(),
            crash: Crash { reason: if unwreck { reason } else { before }, down, clock: F(clock), ..Crash::default() },
            flown: true,
            gravity_scale: F(gravity_scale),
            throw_scale: F(throw_scale),
            gravity: Vf::new(force[0], force[1], force[2], force[3]),
            area: None,
            touchdown_sound: false,
            rotor_angle: F::ZERO,
            rotor_hidden: hidden[0],
        };
        let mut w = Wreckage {
            wreck_gravity: F(wreck_gravity),
            wreck_damping: Vf::new(wreck_damping[0], wreck_damping[1], wreck_damping[2], wreck_damping[3]),
            wreck_inertia: Vf::new(wreck_inertia[0], wreck_inertia[1], wreck_inertia[2], wreck_inertia[3]),
            inertia: Vf::new(inertia[0], inertia[1], inertia[2], inertia[3]),
            mass: F(mass),
            body_inertia: quad(&ee, body + wreck_at::BODY_INERTIA),
            nodes: Nodes { rotor: [hidden[1], hidden[2], hidden[3]], intact: hidden[4], wrecked: hidden[5] },
            ..Wreckage::default()
        };
        let mut kiowa = Kiowa::default();
        let want_asked: Vec<String>;
        if unwreck {
            ee.call(0x004d_40a0, &[u64::from(HELI), 1], &[], BUDGET);
            w.unwreck_mx(&mut ride);
            kiowa.unwreck_controls();
            want_asked = ["sound off"].into_iter().map(String::from)
                .chain((0..4).map(|_| "blade stopped".to_string()))
                .chain(["put back".to_string(), "event 8 13".to_string()])
                .collect();
            same_lanes("the built inertia", case, w.inertia, vector(&ee, HELI + wreck_at::INERTIA), 4);
            let pads = [kiowa.pad.climb, kiowa.pad.descend, kiowa.commands.altitude_fraction, kiowa.pad.lateral,
                        kiowa.pad.longitudinal, kiowa.pad.pedal, kiowa.control.hold_altitude];
            for (k, o) in [at::CLIMB, at::DESCEND, at::FRACTION, at::PAD, at::PAD + 4, at::PAD + 8, at::HOLD].into_iter().enumerate() {
                same(&format!("the control word at +{o:#x}"), case, pads[k], ee.f32_at(HELI + o));
            }
            same("the throw's scale", case, ride.throw_scale, ee.f32_at(block + 4));
            counts[1] += 1;
        } else {
            ee.call(0x004d_3f90, &[u64::from(HELI), u64::from(reason)], &[], BUDGET);
            let mut out = Crashed::default();
            ride.crash.wreck_mxvatv(reason, &mut out);
            let throw = Throw { blades: [Mat4([Vf::default(); 4]); 4], model: Mat4([Vf::default(); 4]) };
            w.wreck_mx(&mut ride, reason, &throw, &mut Rng::from_seed(1));
            assert_eq!(out.wreck, Some(reason), "case {case}: the wreck asked");
            want_asked = ["rumble test", "event 8 11", "sound on"].into_iter().map(String::from)
                .chain((reason != THROWN).then(|| "blades thrown".to_string()))
                .collect();
            counts[0] += 1;
            counts[2] += usize::from(reason == THROWN);
        }
        assert_eq!(*asked.borrow(), want_asked, "case {case}: the calls out (reason {reason})");
        assert_eq!(ee.u32_at(HELI + at::REASON), ride.crash.reason, "case {case}: the reason");
        same("the clock", case, ride.crash.clock, ee.f32_at(HELI + at::CLOCK));
        assert_eq!(ee.u32_at(HELI + at::DOWN) != 0, ride.crash.down, "case {case}: the latch");
        same_lanes("the gravity", case, ride.gravity, vector(&ee, block + wreck_at::FORCE), 4);
        same("the gravity scale", case, ride.gravity_scale, ee.f32_at(block));
        same_lanes("the rates", case, ride.body.rates, vector(&ee, body + BODY_RATES), 4);
        same_lanes("the damping", case, ride.body.damping, vector(&ee, body + wreck_at::DAMPING), 4);
        same_lanes("the angular damping", case, ride.body.ang_damping, vector(&ee, body + wreck_at::ANG_DAMPING), 4);
        same_lanes("the body's inertia", case, w.body_inertia, vector(&ee, body + wreck_at::BODY_INERTIA), 4);
        same_lanes("the inverse inertia", case, ride.body.inv_inertia, vector(&ee, body + wreck_at::INV_INERTIA), 3);
        // The wreck's blade throw is hooked in the original, so only the two nodes the wreck itself
        // swaps are the port's there; the unwreck swaps all six.
        let got = nodes.map(|n| node_hidden(&ee, n));
        let port = [ride.rotor_hidden, w.nodes.rotor[0], w.nodes.rotor[1], w.nodes.rotor[2], w.nodes.intact, w.nodes.wrecked];
        let swapped = if unwreck { 0 } else { 4 };
        assert_eq!(got[swapped..], port[swapped..], "case {case}: the nodes hidden (rotor x4, +0x1220, the wrecked blades)");
    }
    println!("the wreck and unwreck: {} wrecks ({} thrown), {} unwrecks, bit-exact", counts[0], counts[2], counts[1]);
}

/// The gravity the player's helicopter is taken with (MD63f): in both of MD80a's images the player's
/// Kiowa, never wrecked, holds the unwreck's `32.2 * (-scale * mass)`, and the idle twin the load's
/// `-scale * Weight` (`Part::read`'s), each from `kiowa.phy` as the port reads it.
#[test]
fn the_taken_mxvatv_kiowa_holds_the_unwrecks_gravity() {
    let Some(bytes) = phy() else { return };
    for (name, heli, twin, model_one) in [("kiowa", HELI, 0x01b2_0740, true), ("kiowa3", HELI_TWO, 0x0153_7000, false)] {
        let Some(ee) = image(name, heli) else { return };
        let part = Phy::read_mxvatv_bytes(&bytes, model_one).phy.mass_properties;
        for (who, h) in [("the player's", heli), ("the idle twin's", twin)] {
            assert_eq!(ee.u32_at(h + 4), KIOWA_VTABLE, "{name}: no Kiowa at {h:#x}");
            assert_eq!(ee.u32_at(h + 0x6c4), 0, "{name}: {who} Kiowa has wrecked");
            let block = ee.u32_at(h + at::GRAVITY);
            same_word(&format!("{name}: {who} gravity scale"), part.gravity_scale, &ee, block);
            same_word(&format!("{name}: {who} mass"), part.mass, &ee, ee.u32_at(h + at::BODY) + wreck_at::MASS);
        }
        let taken = game_sim::kiowa::mxvatv_taken_gravity(part.gravity_scale, part.mass);
        same_quad(&format!("{name}: the player's gravity"), taken, &ee, ee.u32_at(heli + at::GRAVITY) + wreck_at::FORCE);
        same_quad(&format!("{name}: the idle twin's gravity"), part.gravity, &ee, ee.u32_at(twin + at::GRAVITY) + wreck_at::FORCE);
        assert_ne!(taken.to_bits(), part.gravity.to_bits(), "{name}: the two gravities round alike");
        println!("{name}: taken {:?}, loaded {:?}", taken, part.gravity);
    }
}

// ---------------------------------------------------------------------------------------------------
// 7 — the reader
// ---------------------------------------------------------------------------------------------------

fn phy() -> Option<Vec<u8>> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2/md80c").join("kiowa.phy");
    match std::fs::read(&path) {
        Ok(b) => Some(b),
        Err(_) => {
            skip(&format!("{} is not there: the disc's Veh\\kiowa.phy (in MXVSATV.PSR)", path.display()));
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
fn the_mxvatv_kiowa_file_reads_as_the_original_loads_it() {
    let Some(bytes) = phy() else { return };
    for (name, heli, model_one) in [("kiowa", HELI, true), ("kiowa3", HELI_TWO, false)] {
        let Some(ee) = image(name, heli) else { return };
        assert_eq!(ee.u32_at(heli + at::MODEL), u32::from(model_one), "{name}'s model word");
        let p = Phy::read_mxvatv_bytes(&bytes, model_one);
        let w = &p.phy.wrecks;
        same_word("MaxWreckTime", w.wreck_time, &ee, heli + at::WRECK_TIME);
        same_word("WreckGravity", w.wreck_gravity, &ee, heli + 0xc90);
        same_quad("WreckDamping", w.wreck_damping, &ee, heli + 0xca0);
        same_quad("WreckInertia", w.wreck_inertia, &ee, heli + 0xcb0);
        same_word("MinWreckSpeed", w.min_wreck_speed, &ee, heli + at::MIN_WRECK_SPEED);
        same_quad("PelvisBoxPosition", p.phy.pelvis_box_position, &ee, heli + 0x1350);
        same_quad("PelvisBoxSize", p.phy.pelvis_box_size, &ee, heli + 0x1360);
        same_quad("Left1stPerShoulderOffset", p.left_shoulder_offset, &ee, heli + 0x980);
        same_quad("Right1stPerShoulderOffset", p.right_shoulder_offset, &ee, heli + 0x990);
        same_quad("FPCameraOffset", p.fp_camera_offset, &ee, heli + 0xda0);
        let a = ee.u32_at(heli + at::AERO);
        same_word("CYbetaPedal", p.pedal.cy_beta, &ee, a + AERO_PEDAL);
        same_word("CnbetaPedal", p.pedal.cn_beta, &ee, a + AERO_PEDAL + 4);
        same_word("CnrPedal", p.pedal.cn_r, &ee, a + AERO_PEDAL + 8);
        same_word("the pedals' fade time", p.pedal.time, &ee, a + AERO_PEDAL + 16);
        for (k, o) in AERO_KEYS.into_iter().enumerate() {
            let got = [
                p.phy.aero.density, p.phy.aero.chord, p.phy.aero.span, p.phy.aero.area, p.phy.aero.cl_alpha, p.phy.aero.cd0,
                p.phy.aero.cy_beta, p.phy.aero.cl_beta, p.phy.aero.cl_p, p.phy.aero.cl_da, p.phy.aero.cm0,
                p.phy.aero.cm_alpha, p.phy.aero.cm_q, p.phy.aero.cm_de, p.phy.aero.cn_beta, p.phy.aero.cn_r,
                p.phy.aero.cn_dr, p.phy.aero.low_speed_drag, p.phy.aero.low_speed_aero, p.phy.aero.high_speed_aero,
            ][k];
            same_word(&format!("the aero key at +{o:#x}"), got, &ee, a + o);
        }
        let c = &p.phy.controller;
        let keys = [
            c.lateral_cyclic_gain, c.longitudinal_cyclic_gain, c.pedal_gain, c.max_altitude, c.hold_delay,
            c.collective_gain, c.climb_rate, c.descent_rate, c.altitude_hold_gain, c.integrator_gain, c.min_collective,
            c.max_pitch, c.max_roll, c.pitch_gain, c.roll_gain, c.speed_command_rate, c.min_speed_command,
            c.max_speed_command, c.speed_gain, c.min_feed_forward_speed, c.min_feed_forward_scale,
            c.max_feed_forward_speed, c.max_feed_forward_scale,
        ];
        for (got, o) in keys.into_iter().zip(CONTROLLER_KEYS) {
            same_word(&format!("the controller key at +{o:#x}"), got, &ee, heli + o);
        }
        for k in 0..3 {
            same_word("MinRateCommand", c.min_rate[k], &ee, heli + MIN_RATE + 4 * k as u32);
            same_word("MaxRateCommand", c.max_rate[k], &ee, heli + MAX_RATE + 4 * k as u32);
        }
        let r = ee.u32_at(heli + at::ROTOR);
        let want = rotor_of(&ee, r);
        assert_eq!(p.phy.rotor, want, "{name}'s rotor keys");
        println!("{name}: model {}, read as loaded", if model_one { "one" } else { "two" });
    }
}
