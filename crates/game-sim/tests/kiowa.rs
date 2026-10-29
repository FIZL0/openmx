//! S148 — the Kiowa's flight controller against the original, on a save state's live helicopter.
//!
//! The state is free09 booted with the event's first machine set to the helicopter, then switched
//! into from the pause menu and saved standing on the ground (`context/states/s148-kiowa.ee.bin`;
//! the recipe is in research/journal/*-s148-kiowa). The interpreter calls the original's controller
//! on generated pads, gains, attitudes and loop states written into that helicopter, and the port's
//! [`Controller::step`] must leave every output and every piece of its state the same, bit for bit.

use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::kiowa::{Commands, Controller, Filter, Flight, Loop, Pad};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

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
const LOW_SPEED_AERO: u32 = 0x100;

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
        low_speed_aero: f(ee, ee.u32_at(HELI + at::AERO) + LOW_SPEED_AERO),
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
        .chain([aero + LOW_SPEED_AERO])
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
