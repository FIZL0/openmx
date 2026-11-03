//! C14: the original flying the Kiowa on free09, replayed through the port's frame.
//!
//! The capture (`context/capture/c14/flight.csv`, the C14 journal entry) is S148's boot with the
//! player switched into the helicopter, flown from a generated tape: rest, climb, hover, stick
//! forward, rest, right rudder, stick left, rest, descend, land. One row a tick, read after the tick:
//! the pad buffers, the pad and command words, the controller's whole field run (+0x330..+0x3e0), the
//! collective filter and the measure's figures (+0x460..+0x4d0), the rotor and aero models' states
//! and the body's 0x200 bytes. The helicopter's constants (the controller's, rotor's and aero's file
//! values, the curve table, the terrain's area, gravity) are read off the tape's start state,
//! `context/states/c14-kiowa.ee.bin`, whose helicopter and body bytes are the first row's (1204).
//!
//! [`the_flight_replays_bit_exact`] seeds the port's [`Kiowa`] and [`Ride`] from the last row on the
//! ground's take-off (the collision record's touching flag, captured as `pilot_flag`, has just
//! dropped) and runs [`Kiowa::tick`] on each following row's pad until the row before the touchdown:
//! the climb, the hover, the stick forward and the turn, the descent. Every tick it compares every
//! word the capture holds that the frame writes: the five pad words, the commands, the controller's
//! loop, the rotor's and the aerodynamics' states, the measure's figures, and all 21 quadwords of the
//! body with its two flags.
//!
//! In the air the collision record has no entries, so its response adds nothing and the crash
//! test's strike answers no: the capture's wreck word stays 0 on every row.
//!
//! C14a: [`the_ground_at_both_ends_replays_bit_exact`] takes the ground at both ends too. It seeds
//! everything from the image alone and runs every row from 1205 to 2057, through the take-off,
//! the flight, the touchdown at 1816 and its bounces. Its surroundings are [`Grounded`]: the
//! helicopter's [`Collision`] (built from `kiowam.col` and the disc's `.phy`, checked against the
//! image's record) searches free09's terrain, trees and props before each world step, and
//! `contact::resolve` answers on it each substep. The crash test's rotor-in-ground and
//! MinWreckSpeed tests run on every tick and ask nothing. The touchdown's sound draws no `rand()`
//! because the image's touchdown sound has no clips. The disc (`context/iso/MX.PSR`) is needed as
//! well; without it the test skips.
//!
//! C17: [`a_captured_rotor_strike_replays_bit_exact`] replays a second tape from the same image
//! (`context/capture/c17/strike.csv`). The helicopter hovers low by a tree, and the left rudder
//! swings the rotor into the trunk. The crash test wrecks it there through the rotor's contact
//! search ([`Collision::rotor_struck`]). C19 carries it through the wreck to the put-back, and checks
//! the rotor probe's first hit against the original's ray record (`context/capture/c19/probe.csv`,
//! the same tape captured with `watch/c19-kiowa-strike.toml`).
//!
//! The capture and the image are game data, git-ignored: absent, the test skips.

use game_sim::air::AcosTable;
use game_sim::biplane::{Area, Body, World};
use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::input::{EventQueue, REPORT_BYTES, Report};
use game_sim::kiowa::{Aero, AeroState, Commands, Controller, Crash, Crashed, Filter, Kiowa, Loop, Measure, Pad, Ride};
use game_sim::kiowa::{Rotor, RotorState, Strike, Surroundings, Touch};
use game_sim::kiowa::{CRASH_MASK, Collision, Grounded, KIOWA_PHY, KIOWA_RAYS, Phy};
use game_sim::eco::{Trees, Wooded};
use game_sim::flight_search::Rays;
use game_sim::track::{self, Terrain};
use game_sim::world::{Ground, Place, Props};
use game_sim::rng::Rng;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::collections::HashMap;
use std::path::PathBuf;

const CAPTURE: &str = "capture/c14/flight.csv";
const STATE: &str = "states/c14-kiowa.ee.bin";
const HELI: u32 = 0x007c_cae0;
/// The first tick the port steps (the row before it is the seed) and the last: the touching flag
/// drops on 1238's row and comes back on 1816's.
const FIRST: u32 = 1239;
const LAST: u32 = 1815;

/// The helicopter's fields, by offset (research/oracle.txt, S148 and S220).
mod at {
    pub const ALTITUDE_FRACTION: u32 = 0x33c;
    pub const PAD: u32 = 0x334; // climb, descend, (fraction), lateral, longitudinal, pedal
    pub const OUT: u32 = 0x34c; // collective, lateral, longitudinal, pedal, filtered
    pub const LAST_STICK: u32 = 0x360;
    pub const FLOWN: u32 = 0x364;
    pub const GAINS: u32 = 0x368; // lateral, longitudinal, pedal, MaxAltitude
    pub const HOLD: u32 = 0x378; // height, timer, delay
    pub const COLLECTIVE: u32 = 0x384; // gain, climb, descent, hold gain, integral, integrator gain, min
    pub const PITCH_ROLL: u32 = 0x3a0; // MaxPitch, MaxRoll, PitchGain, RollGain, command, rate, min, max, gain
    pub const FEED_FORWARD: u32 = 0x3c4;
    pub const WRECKED: u32 = 0x3d4;
    pub const DOWN: u32 = 0x3dc;
    pub const MIN_WRECK_SPEED: u32 = 0x3e0;
    pub const CLOCK: u32 = 0x3e4;
    pub const WRECK_TIME: u32 = 0x3e8;
    pub const RATES: u32 = 0x430;
    pub const GRAVITY: u32 = 0x140;
    pub const GRAVITY_SCALE: u32 = 0x190;
    pub const THROW_SCALE: u32 = 0x194;
    pub const ROTOR: u32 = 0x450;
    pub const AERO: u32 = 0x454;
    pub const SCENE: u32 = 0x45c;
    pub const FILTER: u32 = 0x46c;
    pub const SPEED: u32 = 0x470; // speed, alpha, beta
    pub const ROWS: u32 = 0x480; // world rates, body velocity, forward, up, Euler
    /// The scene's terrain, and its four extent floats.
    pub const TERRAIN: u32 = 0x3b4;
    pub const EXTENT: u32 = 0x530;
    /// The capture's two raw blocks of the helicopter.
    pub const BLOCK_A: u32 = 0x330;
    pub const BLOCK_B: u32 = 0x460;
}

/// The rotor model's state (rotor +0xd0, the capture's `rotor_state`) and the aero model's outputs
/// (aero +0x128, `aero_state`).
const ROTOR_STATE: u32 = 0xd0;
const AERO_OUT: u32 = 0x128;
const AERO_KEYS: [u32; 20] =
    [0x30, 0x34, 0x38, 0x3c, 0x48, 0x60, 0x68, 0x74, 0x78, 0x80, 0x84, 0x88, 0x8c, 0x90, 0x94, 0x9c, 0xa0, 0xf4, 0xfc, 0x100];

/// The body's quadwords, by name; the two accelerations' `w` is whatever the register held.
const QUADS: [(&str, u32, usize); 21] = [
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

fn ctx(rel: &str) -> Option<PathBuf> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let local = kit.root.join("context").join(rel);
    let path = if local.exists() { local } else { kit.ctx(rel) };
    if !path.exists() {
        skip(&format!("{} is not there: C14's Kiowa capture (in s1's context)", path.display()));
        return None;
    }
    Some(path)
}

/// The capture: its header and rows, as text.
struct Capture {
    cols: HashMap<String, usize>,
    rows: Vec<Vec<String>>,
}

impl Capture {
    fn load(path: &PathBuf) -> Capture {
        let text = std::fs::read_to_string(path).expect("read the capture");
        let mut lines = text.lines();
        let cols = lines.next().expect("a header").split(',').enumerate().map(|(k, c)| (c.to_string(), k)).collect();
        let rows = lines.map(|l| l.split(',').map(str::to_string).collect()).collect();
        Capture { cols, rows }
    }
    fn text(&self, row: usize, col: &str) -> &str {
        &self.rows[row][*self.cols.get(col).unwrap_or_else(|| panic!("no column {col}"))]
    }
    fn u32(&self, row: usize, col: &str) -> u32 {
        self.text(row, col).parse().unwrap_or_else(|e| panic!("{col} at row {row}: {e}"))
    }
    fn f32(&self, row: usize, col: &str) -> f32 {
        self.text(row, col).parse().unwrap_or_else(|e| panic!("{col} at row {row}: {e}"))
    }
    fn bytes(&self, row: usize, col: &str) -> Vec<u8> {
        let hex = self.text(row, col).as_bytes();
        hex.chunks(2).map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap()).collect()
    }
    fn row_at(&self, frame: u32) -> usize {
        (0..self.rows.len()).find(|&r| self.u32(r, "frame_counter") == frame).unwrap_or_else(|| panic!("no row {frame}"))
    }
    /// One of the raw blocks, readable by the helicopter's (or the body's) own offsets.
    fn block(&self, row: usize, col: &str, base: u32) -> Block {
        Block { base, bytes: self.bytes(row, col) }
    }
}

struct Block {
    base: u32,
    bytes: Vec<u8>,
}

impl Block {
    fn u32(&self, a: u32) -> u32 {
        let k = (a - self.base) as usize;
        u32::from_le_bytes(self.bytes[k..k + 4].try_into().unwrap())
    }
    fn f(&self, a: u32) -> F {
        F(f32::from_bits(self.u32(a)))
    }
    fn quad(&self, a: u32) -> Vf {
        Vf::new(self.f(a).to_f32(), self.f(a + 4).to_f32(), self.f(a + 8).to_f32(), self.f(a + 12).to_f32())
    }
}

fn f(ee: &Ee, a: u32) -> F {
    F(ee.f32_at(a))
}

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
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

fn rotor_of(ee: &Ee, r: u32) -> Rotor {
    let g = |o: u32| f(ee, r + o);
    Rotor {
        max_lateral_cyclic: g(0x48),
        max_longitudinal_cyclic: g(0x4c),
        max_collective: g(0x50),
        max_tail_pitch: g(0x54),
        lift_curve_slope: g(0x58),
        blade_chord: g(0x5c),
        blades: g(0x60),
        radius: g(0x64),
        radius_squared: g(0x68),
        angular_rate: g(0x6c),
        max_moment: [g(0xc0), g(0xc4), g(0xc8)],
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

/// The row's helicopter words, as the port keeps them.
fn pad_of(a: &Block) -> Pad {
    let h = |o: u32| a.f(at::PAD + o);
    Pad { climb: h(0), descend: h(4), lateral: h(12), longitudinal: h(16), pedal: h(20) }
}

fn commands_of(a: &Block) -> Commands {
    let h = |o: u32| a.f(at::OUT + o);
    Commands {
        collective: h(0),
        lateral: h(4),
        longitudinal: h(8),
        pedal: h(12),
        filtered: h(16),
        altitude_fraction: a.f(at::ALTITUDE_FRACTION),
    }
}

/// The controller's loop off a row; the collective filter's value is the filtered collective it
/// last gave (+0x35c), its time constant the image's.
fn loop_of(a: &Block, filter: Filter) -> Loop {
    Loop {
        speed_command: a.f(at::PITCH_ROLL + 16),
        last_longitudinal: a.f(at::LAST_STICK),
        hold_altitude: a.f(at::HOLD),
        hold_timer: a.f(at::HOLD + 4),
        integral: a.f(at::COLLECTIVE + 16),
        filter: Filter { value: a.f(at::OUT + 16), ..filter },
    }
}

fn rotor_state_of(r: &Block) -> RotorState {
    let g = |o: u32| r.f(o);
    RotorState {
        collective_pitch: g(ROTOR_STATE),
        tail_pitch: g(ROTOR_STATE + 4),
        tilt: [g(0xe0), g(0xe4), g(0xe8)],
        thrust: r.quad(0x100),
        force: r.quad(0x110),
    }
}

fn aero_state_of(a: &Block) -> AeroState {
    let g = |o: u32| a.f(AERO_OUT + o);
    AeroState { fade: g(0), lift: g(4), drag: g(8), side: g(12), roll: g(16), pitch: g(20), yaw: g(24) }
}

fn body_of(b: &Block) -> Body {
    let [damping, last_position, position, velocity, last_force, force, accel, shock_peak, shock, step, inv_inertia,
         ang_damping, last_quat, quat, cofg, moment, last_moment, rates, ang_accel, twist_peak, twist] =
        QUADS.map(|(_, o, _)| b.quad(o));
    Body {
        enabled: b.u32(0) != 0,
        inv_mass: b.f(0x10),
        damping, last_position, position, velocity, last_force, force, accel, shock_peak, shock, step, inv_inertia,
        ang_damping, last_quat, quat, cofg, moment, last_moment, rates, ang_accel, twist_peak, twist,
    }
}

fn body_quads(b: &Body) -> [Vf; 21] {
    [b.damping, b.last_position, b.position, b.velocity, b.last_force, b.force, b.accel, b.shock_peak, b.shock, b.step,
     b.inv_inertia, b.ang_damping, b.last_quat, b.quat, b.cofg, b.moment, b.last_moment, b.rates, b.ang_accel,
     b.twist_peak, b.twist]
}

/// The measure's figures the capture holds: speed, alpha, beta, then the five rows.
fn measure_words(m: &Measure) -> Vec<(&'static str, F)> {
    let mut out = vec![("the speed", m.speed), ("alpha", m.alpha), ("beta", m.beta)];
    let names = ["the world rates", "the body velocity", "the forward row", "the up row", "the Euler angles"];
    for (name, v) in names.into_iter().zip([m.world_rates, m.velocity, m.forward, m.up, m.euler]) {
        for k in 0..4 {
            out.push((name, F(v[k])));
        }
    }
    out
}

fn captured_measure(b: &Block) -> Vec<F> {
    let mut out: Vec<F> = (0..3).map(|k| b.f(at::SPEED + 4 * k)).collect();
    out.extend((0..20).map(|k| b.f(at::ROWS + 4 * k)));
    out
}

/// The engine round the helicopter in the air: the collision record has no entries, so its
/// response pushes nothing, and the rotor strikes nothing.
#[derive(Default)]
struct Air {
    resolved: usize,
    crashed: Vec<Crashed>,
}

impl Surroundings for Air {
    fn resolve(&mut self, _: &mut Body) {
        self.resolved += 1;
    }
    fn touching(&self) -> bool {
        false
    }
    fn ignored(&self) -> bool {
        false
    }
    fn entries(&self) -> Vec<Touch> {
        Vec::new()
    }
    fn strike(&mut self, _: &game_sim::math::Mat4) -> Strike {
        Strike::default()
    }
    fn crashed(&mut self, asked: &Crashed, _: &mut Ride, _: &mut Rng) {
        self.crashed.push(*asked);
    }
}

fn report_of(cap: &Capture, row: usize) -> Report {
    let bytes = cap.bytes(row, "pad_a");
    let mut b = [0u8; REPORT_BYTES];
    b[..bytes.len().min(REPORT_BYTES)].copy_from_slice(&bytes[..bytes.len().min(REPORT_BYTES)]);
    Report::from_bytes(b)
}

fn same(what: &str, frame: u32, got: F, want: F) {
    assert_eq!(got.to_bits(), want.to_bits(), "tick {frame}: {what} is {:e} in the game, {:e} in the port", want.to_f32(), got.to_f32());
}

/// Which of the five pad words a row has seen off zero.
fn moved_by(moved: &mut [bool; 5], want: &Pad) {
    for (k, v) in [want.climb, want.descend, want.lateral, want.longitudinal, want.pedal].into_iter().enumerate() {
        moved[k] |= v != F::ZERO;
    }
}

/// **A row against the port**, bit for bit: every word the capture holds that the frame writes — the
/// five pad words, the commands, the controller's loop, the rotor's and the aerodynamics' states, the
/// measure's figures, the wreck word and all 21 quadwords of the body with its two flags.
fn compare(cap: &Capture, row: usize, kiowa: &Kiowa, ride: &Ride, filter: Filter) {
    let frame = cap.u32(row, "frame_counter");
    let (wa, wb) = (cap.block(row, "heli_a", at::BLOCK_A), cap.block(row, "heli_b", at::BLOCK_B));
    assert_eq!(wa.u32(at::WRECKED), ride.crash.reason, "tick {frame}: the wreck word");
    let want = pad_of(&wa);
    for (name, got, want) in [
        ("climb", kiowa.pad.climb, want.climb),
        ("descend", kiowa.pad.descend, want.descend),
        ("the lateral", kiowa.pad.lateral, want.lateral),
        ("the longitudinal", kiowa.pad.longitudinal, want.longitudinal),
        ("the pedal", kiowa.pad.pedal, want.pedal),
    ] {
        same(name, frame, got, want);
    }
    if let Some(differs) = flight_mismatch(cap, row, kiowa, filter) {
        panic!("tick {frame}: {differs}");
    }
    for ((name, got), want) in measure_words(&ride.measure).into_iter().zip(captured_measure(&wb)) {
        same(name, frame, got, want);
    }
    let want = body_of(&cap.block(row, "body_raw", 0));
    assert_eq!(ride.body.enabled, want.enabled, "tick {frame}: enabled");
    same("the inverse mass", frame, ride.body.inv_mass, want.inv_mass);
    for ((name, _, lanes), (got, want)) in QUADS.iter().zip(body_quads(&ride.body).into_iter().zip(body_quads(&want))) {
        for k in 0..*lanes {
            same(&format!("{name}[{k}]"), frame, F(got[k]), F(want[k]));
        }
    }
}

/// The first word of the controller's, the rotor's and the aerodynamics' states (in [`compare`]'s
/// order) that differs from the row bit for bit, said as [`compare`] says it.
fn flight_mismatch(cap: &Capture, row: usize, kiowa: &Kiowa, filter: Filter) -> Option<String> {
    let wa = cap.block(row, "heli_a", at::BLOCK_A);
    let differs = |what: &str, got: F, want: F| {
        (got.to_bits() != want.to_bits()).then(|| format!("{what} is {:e} in the game, {:e} in the port", want.to_f32(), got.to_f32()))
    };
    let want = commands_of(&wa);
    let commands = [
        ("the collective", kiowa.commands.collective, want.collective),
        ("the lateral cyclic", kiowa.commands.lateral, want.lateral),
        ("the longitudinal cyclic", kiowa.commands.longitudinal, want.longitudinal),
        ("the pedal command", kiowa.commands.pedal, want.pedal),
        ("the filtered collective", kiowa.commands.filtered, want.filtered),
        ("the altitude fraction", kiowa.commands.altitude_fraction, want.altitude_fraction),
    ];
    let want = loop_of(&wa, filter);
    let control = [
        ("the speed command", kiowa.control.speed_command, want.speed_command),
        ("the last longitudinal", kiowa.control.last_longitudinal, want.last_longitudinal),
        ("the hold altitude", kiowa.control.hold_altitude, want.hold_altitude),
        ("the hold timer", kiowa.control.hold_timer, want.hold_timer),
        ("the integral", kiowa.control.integral, want.integral),
        ("the filter's value", kiowa.control.filter.value, want.filter.value),
    ];
    if let Some(d) = commands.into_iter().chain(control).find_map(|(name, got, want)| differs(name, got, want)) {
        return Some(d);
    }
    let want = rotor_state_of(&cap.block(row, "rotor_state", ROTOR_STATE));
    if kiowa.rotor_state != want {
        return Some(format!("the rotor's state is {want:?} in the game, {:?} in the port", kiowa.rotor_state));
    }
    let want = aero_state_of(&cap.block(row, "aero_state", AERO_OUT));
    if kiowa.aero_state != want {
        return Some(format!("the aerodynamics' state is {want:?} in the game, {:?} in the port", kiowa.aero_state));
    }
    None
}

/// **The capture's flight, tick by tick through the port's frame**: see the module's docs.
#[test]
fn the_flight_replays_bit_exact() {
    let (Some(cap_path), Some(image)) = (ctx(CAPTURE), ctx(STATE)) else { return };
    let cap = Capture::load(&cap_path);
    let ee = Ee::from_ram(&image).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(ee.u32_at(HELI + at::FLOWN), 1, "no flown helicopter at {HELI:#x} in {}", image.display());
    assert_eq!(ee.f32_at(HELI + at::GAINS + 12), 550.0, "MaxAltitude is not 550 at {HELI:#x}");

    // The helicopter's constants, off the start state.
    let (r, a) = (ee.u32_at(HELI + at::ROTOR), ee.u32_at(HELI + at::AERO));
    let pf = ee.u32_at(HELI + at::FILTER);
    let filter = Filter { value: f(&ee, pf), time: f(&ee, pf + 4), step: f(&ee, pf + 8) };
    assert_eq!(filter.value.to_bits(), f(&ee, HELI + at::OUT + 16).to_bits(), "the filter's value is not the filtered collective");
    let terrain = ee.u32_at(ee.u32_at(HELI + at::SCENE) + at::TERRAIN);
    let e: Vec<F> = (0..4).map(|k| f(&ee, terrain + at::EXTENT + 4 * k)).collect();
    let curve = the_curve_table(&ee);
    let acos = AcosTable::build();
    let world = World { iterations: 1, rate: F(115.0) };

    // The seed: the row of the take-off's last touch, the helicopter just off the ground.
    let seed = cap.row_at(FIRST - 1);
    assert_eq!(cap.u32(seed, "pilot_flag"), 0, "the helicopter is not off the ground on {}'s row", FIRST - 1);
    assert_eq!(cap.u32(seed - 1, "pilot_flag"), 1, "the helicopter was not on the ground on {}'s row", FIRST - 2);
    let block_a = |row: usize| cap.block(row, "heli_a", at::BLOCK_A);
    let block_b = |row: usize| cap.block(row, "heli_b", at::BLOCK_B);
    let rotor_block = |row: usize| cap.block(row, "rotor_state", ROTOR_STATE);
    let aero_block = |row: usize| cap.block(row, "aero_state", AERO_OUT);
    let body_block = |row: usize| cap.block(row, "body_raw", 0);
    let (sa, sb) = (block_a(seed), block_b(seed));
    let mut kiowa = Kiowa {
        controller: controller_of(&ee),
        rotor: rotor_of(&ee, r),
        aero: aero_of(&ee, a),
        pad: pad_of(&sa),
        commands: commands_of(&sa),
        control: loop_of(&sa, filter),
        rotor_state: rotor_state_of(&rotor_block(seed)),
        aero_state: aero_state_of(&aero_block(seed)),
        wrecked: false,
    };
    // The measure as the seed's tick left it: its own figures again off the seed's body (beta's
    // last value the row's), the matrix with them; the touching flag down.
    let body = body_of(&body_block(seed));
    let mut measure = Measure { beta: sb.f(at::SPEED + 8), ..Measure::default() };
    measure.forward = sb.quad(at::ROWS + 0x20);
    measure.up = sb.quad(at::ROWS + 0x30);
    measure.flight(&body, &acos);
    for ((name, got), want) in measure_words(&measure).into_iter().zip(captured_measure(&sb)) {
        same(&format!("the seed's {name}"), FIRST - 1, got, want);
    }
    let mut ride = Ride {
        body,
        measure,
        crash: Crash {
            reason: sa.u32(at::WRECKED),
            down: sa.u32(at::DOWN) != 0,
            min_wreck_speed: f(&ee, HELI + at::MIN_WRECK_SPEED),
            wreck_time: f(&ee, HELI + at::WRECK_TIME),
            clock: f(&ee, HELI + at::CLOCK),
        },
        flown: sa.u32(at::FLOWN) != 0,
        gravity_scale: f(&ee, HELI + at::GRAVITY_SCALE),
        throw_scale: f(&ee, HELI + at::THROW_SCALE),
        gravity: quad(&ee, HELI + at::GRAVITY),
        area: Some(Area::new(e[0], e[1], e[2], e[3])),
        touchdown_sound: false,
        rotor_angle: F::ZERO,
        rotor_hidden: false,
    };
    assert_eq!(ride.crash.reason, 0, "the helicopter is wrecked at the seed");
    assert!(ride.flown, "the helicopter is not flown at the seed");

    // The player's pad, polled off the buffers from the capture's start: a row's tick reads the
    // row before's buffer (the pad words lag the tape by a tick).
    let mut pad = game_sim::input::Pad::new(0);
    pad.connected = true;
    let mut events = EventQueue::new();
    for row in 0..seed {
        pad.poll(F(cap.f32(row + 1, "dt")), &report_of(&cap, row), &mut events);
    }

    let mut rng = Rng::from_state(0x0c14_0c14_0c14_0c14);
    let start_rng = rng.state();
    let mut air = Air::default();
    let mut ticks = 0;
    let mut moved = [false; 5];
    for row in seed + 1..=cap.row_at(LAST) {
        let frame = cap.u32(row, "frame_counter");
        let dt = F(cap.f32(row, "dt"));
        let out = kiowa.tick(&mut ride, &world, None, &curve, &acos, &mut rng, dt, &mut air);
        assert_eq!(out, Crashed::default(), "tick {frame}: the port's crash test asked for something");
        events.clear();
        pad.poll(dt, &report_of(&cap, row - 1), &mut events);
        kiowa.pad = Pad::from_pad(&pad);

        assert_eq!(cap.u32(row, "pilot_flag"), 0, "tick {frame}: the helicopter touches something");
        moved_by(&mut moved, &pad_of(&block_a(row)));
        compare(&cap, row, &kiowa, &ride, filter);
        ticks += 1;
    }
    // The helicopter's own frame draws nothing in the air: the capture's generator moves for others.
    assert_eq!(rng.state(), start_rng, "the port's frame drew from the generator in the air");
    assert!(air.resolved > 0 && air.crashed.len() == ticks, "the frame ran no substeps");
    assert!(moved.iter().all(|&m| m), "the flight does not use every pad word: {moved:?}");
    let (top, last) = (cap.row_at(1394), cap.row_at(LAST));
    println!("the Kiowa's flight: {ticks} ticks ({FIRST}..={LAST}) bit-exact, every pad word used; up to y {} at {}, down to {} at {}",
             cap.f32(top, "body_pos.y"), 1394, cap.f32(last, "body_pos.y"), LAST);
}

// --- the ground at both ends (C14a) ------------------------------------------------------------------

/// The collision object's fields (`tests/flight_collision.rs`'s, the same class as the plane's).
mod rec {
    pub const TOUCHING: u32 = 0x40;
    pub const CAPACITY: u32 = 0x104;
    pub const RAY_PROBE: u32 = 0x1f0;
    pub const COLLIDABLE: u32 = 0x1f8;
    pub const STRUCK: u32 = 0x1fc;
    /// The ray probe's count and rays (from, to; 0x20 apart).
    pub const RAY_COUNT: u32 = 0x168;
    pub const RAYS: u32 = 0x170;
    pub const CATEGORY: u32 = 0x84;
}

/// More of the helicopter's fields (research/oracle.txt, S220a).
mod heli_at {
    pub const BODY: u32 = 0x160;
    pub const COLLISION: u32 = 0x170;
    pub const LAST_TOUCHING: u32 = 0x420;
    pub const CRASH_MASK: u32 = 0x460;
    pub const TOUCHDOWN_SOUND: u32 = 0x54c;
    /// The touchdown sound's clip count: its pick draws a `rand()` only when it is not 0.
    pub const CLIPS: u32 = 0x200;
    /// The body's inertia tensor.
    pub const INERTIA: u32 = 0x110;
}

/// A file out of MX.PSR, and a member of a `.res` in it by its name's end.
struct Disc {
    file: std::fs::File,
    psr: game_sim::archive::Psr,
}

impl Disc {
    fn open() -> Option<Disc> {
        let path = ctx("iso/MX.PSR")?;
        let mut file = std::fs::File::open(&path).expect("open MX.PSR");
        let mut front = vec![0u8; 64 << 10];
        std::io::Read::read_exact(&mut file, &mut front).expect("the front of MX.PSR");
        let psr = game_sim::archive::Psr::parse(&front).expect("MX.PSR's directory");
        Some(Disc { file, psr })
    }
    fn member(&mut self, name: &str) -> Option<Vec<u8>> {
        let e = self.psr.find(name)?;
        std::io::Seek::seek(&mut self.file, std::io::SeekFrom::Start(e.at)).expect("seek");
        let mut bytes = vec![0u8; e.size as usize];
        std::io::Read::read_exact(&mut self.file, &mut bytes).expect("read");
        Some(bytes)
    }
}

fn entry(res: &game_sim::archive::Res, bytes: &[u8], suffix: &str) -> Option<Vec<u8>> {
    let want = suffix.to_ascii_lowercase();
    let e = res.entries.iter().find(|e| {
        let n = e.name.to_ascii_lowercase();
        n == want || n.ends_with(&format!("\\{want}"))
    })?;
    res.read(bytes, e).ok()
}

/// What the disc gives the replay: `helicopter\kiowa.phy`, the ray probe's `kiowam.col`, and
/// free09's archives (its terrain bytes, scene and props, and its ecosystem).
struct Free09 {
    phy: Phy,
    rays: track::Rays,
    terrain: Vec<u8>,
    scene: track::Scene,
    props: Props,
    esb: Vec<u8>,
}

fn the_disc() -> Option<Free09> {
    use game_sim::archive::Res;
    let mut disc = Disc::open()?;
    let machine = disc.member("machine.res").expect("machine.res is in MX.PSR");
    let res = Res::parse(&machine).expect("machine.res");
    let phy = Phy::read_bytes(&entry(&res, &machine, KIOWA_PHY).expect("kiowa.phy"));
    let rays = track::Rays::parse(&entry(&res, &machine, KIOWA_RAYS).expect("kiowam.col")).expect("kiowam.col's rays");
    let bytes = disc.member("Tracks\\free09.res").expect("free09.res");
    let res = Res::parse(&bytes).expect("free09.res");
    let by_end = |end: &str| {
        let e = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(end)).unwrap_or_else(|| panic!("free09 has no {end}"));
        res.read(&bytes, e).expect("a free09 member")
    };
    let scene = track::Scene::parse(&by_end(".scn")).expect("free09's scene");
    let terrain = by_end(&scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn"));
    let esb = by_end(".esb");
    let mut parts = vec![(bytes.clone(), res.clone())];
    if let Some(b) = disc.member("Tracks\\free09_2.res")
        && let Ok(r) = Res::parse(&b)
    {
        parts.push((b, r));
    }
    let (props, missing) = Props::load(&scene, |want| {
        parts.iter().find_map(|(b, r)| entry(r, b, want)).ok_or_else(|| "no archive of this track has it".to_string())
    });
    // free09's utility shack is a hierarchy, which the port's world does not build (S66's
    // `tests/back_search.rs` says the same); the ground replay matches without it, so the
    // helicopter's searches never reach it.
    assert!(missing.iter().all(|m| m.starts_with("UTILITYSHACK1.col")), "free09's props the archives lack: {missing:?}");
    Some(Free09 { phy, rays, terrain, scene, props, esb })
}

/// `len` bytes of the image from `base`, readable by their own addresses.
fn image_block(ee: &Ee, base: u32, len: u32) -> Block {
    Block { base, bytes: (0..len).map(|k| ee.u8_at(base + k)).collect() }
}

/// The helicopter's collision object in the image: the port's built off the disc, against the
/// original's — every field of its response's tuning, its search's capacity, its collidable's
/// categories and the ray probe's 22 rays.
fn check_collision(ee: &Ee, c: &Collision) {
    let r = ee.u32_at(HELI + heli_at::COLLISION);
    let q = |a: u32| quad(ee, a);
    let k = &c.collider;
    let fl = |a: u32| F(ee.f32_at(r + a));
    for (what, got, want) in [
        ("the mass", k.mass, fl(0x64)),
        ("the inverse mass", k.inv_mass, fl(0x68)),
        ("the max depth", k.max_depth, fl(0x118)),
        ("the scenery stiffness", k.scenery.stiffness, fl(0x124)),
        ("the scenery damping", k.scenery.damping, fl(0x128)),
        ("the scenery restitution", k.scenery.restitution, fl(0x12c)),
        ("the actor stiffness", k.actor.stiffness, fl(0x13c)),
        ("the actor damping", k.actor.damping, fl(0x140)),
        ("the actor restitution", k.actor.restitution, fl(0x144)),
        ("the friction", k.scenery.friction, fl(0x190)),
        ("the actor friction", k.actor.friction, fl(0x194)),
    ] {
        assert_eq!(got.to_bits(), want.to_bits(), "the collision object's {what}: the game {want:?}, the port {got:?}");
    }
    for (what, got, want) in [
        ("inverse inertia", k.inv_inertia, q(r + 0x80)),
        ("centre of gravity", k.cofg, q(r + 0x90)),
        ("spring mask", k.scenery.spring_mask, q(r + 0x150)),
        ("friction mask", k.scenery.friction_mask, q(r + 0x160)),
        ("actor spring mask", k.actor.spring_mask, q(r + 0x170)),
        ("actor friction mask", k.actor.friction_mask, q(r + 0x180)),
    ] {
        assert_eq!(got.to_bits(), want.to_bits(), "the collision object's {what}: the game {want:?}, the port {got:?}");
    }
    assert_eq!((k.actors, k.collides_with), (ee.u32_at(r + 0x110), ee.u32_at(r + 0x114)), "the actor masks");
    assert_eq!(k.collidable, Some(ee.u32_at(ee.u32_at(r + rec::COLLIDABLE) + rec::CATEGORY)), "the collidable's categories");
    assert_eq!(c.record.capacity as u32, ee.u32_at(r + rec::CAPACITY), "the contact capacity");
    let probe = ee.u32_at(r + rec::RAY_PROBE);
    let (n, at) = (ee.u32_at(probe + rec::RAY_COUNT), ee.u32_at(probe + rec::RAYS));
    assert_eq!(c.rays.segments.len() as u32, n, "the ray count");
    for (i, s) in c.rays.segments.iter().enumerate() {
        let a = at + 0x20 * i as u32;
        assert_eq!((s.from.to_bits(), s.to.to_bits()), (q(a).to_bits(), q(a + 0x10).to_bits()), "ray {i}");
    }
    assert_eq!(ee.u32_at(HELI + heli_at::CRASH_MASK), CRASH_MASK, "the crash test's surface mask");
}

/// **The capture's whole flight, the ground at both ends too**: the port seeded from the tape's start
/// state (the tick before the first row, the helicopter at rest on free09) and stepped through every
/// row, 1204..=2057 — the take-off on the collision record's contacts, the flight, the touchdown and
/// the bounces and the slide on the slope — with the helicopter's own collision object
/// ([`Collision`], built off the disc and checked against the image's) searched against free09's
/// terrain, props and plants before each world step, and resolved in the force listener
/// ([`Grounded`]). Every row is compared as [`the_flight_replays_bit_exact`] compares it, and the
/// record's touching flag against the capture's.
///
/// The crash test runs on the record (its speed test against `MinWreckSpeed` while touching, with the
/// struck surface's mask) and asks nothing, as the capture's wreck word says; the touchdown runs on the
/// record's entries and comes on each tick the flag comes back, and the helicopter's touchdown sound has
/// no clips in the image, so the frame draws no `rand()`.
#[test]
fn the_ground_at_both_ends_replays_bit_exact() {
    let (Some(cap_path), Some(image)) = (ctx(CAPTURE), ctx(STATE)) else { return };
    let Some(disc) = the_disc() else { return };
    let cap = Capture::load(&cap_path);
    let ee = Ee::from_ram(&image).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(ee.u32_at(HELI + at::FLOWN), 1, "no flown helicopter at {HELI:#x} in {}", image.display());

    // The helicopter as the image holds it: its constants, and its state at the tick before row 0.
    let (r, a) = (ee.u32_at(HELI + at::ROTOR), ee.u32_at(HELI + at::AERO));
    let pf = ee.u32_at(HELI + at::FILTER);
    let filter = Filter { value: f(&ee, pf), time: f(&ee, pf + 4), step: f(&ee, pf + 8) };
    let terrain_at = ee.u32_at(ee.u32_at(HELI + at::SCENE) + at::TERRAIN);
    let e: Vec<F> = (0..4).map(|k| f(&ee, terrain_at + at::EXTENT + 4 * k)).collect();
    let curve = the_curve_table(&ee);
    let acos = AcosTable::build();
    let world_step = World { iterations: 1, rate: F(115.0) };
    let ha = image_block(&ee, at::BLOCK_A + HELI, 0xb0);
    let ha = Block { base: at::BLOCK_A, bytes: ha.bytes };
    let b_at = ee.u32_at(HELI + heli_at::BODY);
    let body = body_of(&Block { base: 0, bytes: image_block(&ee, b_at, 0x200).bytes });
    let mut kiowa = Kiowa {
        controller: controller_of(&ee),
        rotor: rotor_of(&ee, r),
        aero: aero_of(&ee, a),
        pad: pad_of(&ha),
        commands: commands_of(&ha),
        control: loop_of(&ha, filter),
        rotor_state: rotor_state_of(&Block { base: ROTOR_STATE, bytes: image_block(&ee, r + ROTOR_STATE, 0x50).bytes }),
        aero_state: aero_state_of(&Block { base: AERO_OUT, bytes: image_block(&ee, a + AERO_OUT, 0x1c).bytes }),
        wrecked: false,
    };
    let rows = |o: u32| quad(&ee, HELI + at::ROWS + o);
    let m = |o: u32| quad(&ee, HELI + 0x4d0 + o);
    let measure = Measure {
        speed: f(&ee, HELI + at::SPEED),
        alpha: f(&ee, HELI + at::SPEED + 4),
        beta: f(&ee, HELI + at::SPEED + 8),
        world_rates: rows(0),
        velocity: rows(0x10),
        forward: rows(0x20),
        up: rows(0x30),
        euler: rows(0x40),
        matrix: game_sim::math::Mat4([m(0), m(0x10), m(0x20), m(0x30)]),
        was_touching: ee.u32_at(HELI + heli_at::LAST_TOUCHING) != 0,
    };
    let clips = ee.u32_at(ee.u32_at(HELI + heli_at::TOUCHDOWN_SOUND) + heli_at::CLIPS);
    let mut ride = Ride {
        body,
        measure,
        crash: Crash {
            reason: ha.u32(at::WRECKED),
            down: ha.u32(at::DOWN) != 0,
            min_wreck_speed: f(&ee, HELI + at::MIN_WRECK_SPEED),
            wreck_time: f(&ee, HELI + at::WRECK_TIME),
            clock: f(&ee, HELI + at::CLOCK),
        },
        flown: ha.u32(at::FLOWN) != 0,
        gravity_scale: f(&ee, HELI + at::GRAVITY_SCALE),
        throw_scale: f(&ee, HELI + at::THROW_SCALE),
        gravity: quad(&ee, HELI + at::GRAVITY),
        area: Some(Area::new(e[0], e[1], e[2], e[3])),
        touchdown_sound: clips != 0,
        rotor_angle: F::ZERO,
        rotor_hidden: false,
    };
    assert_eq!(ride.crash.reason, 0, "the helicopter is wrecked in the image");
    assert!(ride.measure.was_touching, "the helicopter is not on the ground in the image");
    assert_eq!(ee.u32_at(ee.u32_at(HELI + heli_at::COLLISION) + rec::TOUCHING), 1, "the image's record is not touching");
    same("the image's MinWreckSpeed", 1203, ride.crash.min_wreck_speed, disc.phy.wrecks.min_wreck_speed);

    // Its collision object, off the disc and the image's body, against the image's; the struck
    // object empty, as the image's is; the nearby list as the last post left it.
    let inertia = quad(&ee, b_at + heli_at::INERTIA);
    let mut collision = Collision::new(&disc.phy.mass_properties, &ride.body, inertia, Rays::of(&disc.rays));
    check_collision(&ee, &collision);
    let r_at = ee.u32_at(HELI + heli_at::COLLISION);
    assert_eq!(ee.u32_at(r_at + rec::STRUCK), 0, "the image's record has struck something");

    // free09, as the shell builds it.
    let terrain = Terrain::parse(&disc.terrain).expect("free09's terrain");
    let ground = Ground::new(&terrain, &disc.scene.terrain);
    let (trees, missed) = Trees::on_ground(&disc.esb, &ground, &curve).expect("free09's ecosystem");
    assert!(missed.is_empty(), "free09's trees missing: {missed:?}");
    let mut place = Wooded { world: Place { ground, props: disc.props }, trees };
    collision.refresh(&ride.body, &mut place);

    // The image is row 0's state (its blocks are row 0's byte for byte): the replay ticks from row 1.
    compare(&cap, 0, &kiowa, &ride, filter);

    // The player's pad: the first tick flies the image's words; a row's input step reads the row
    // before's buffer.
    let mut pad = game_sim::input::Pad::new(0);
    pad.connected = true;
    let mut events = EventQueue::new();

    let mut rng = Rng::from_state(0x0c14_0c14_0c14_0c14);
    let start_rng = rng.state();
    let (mut ticks, mut touched, mut downs, mut cleared) = (0, 0, Vec::new(), Vec::new());
    let mut moved = [false; 5];
    for row in 1..cap.rows.len() {
        let frame = cap.u32(row, "frame_counter");
        let dt = F(cap.f32(row, "dt"));
        let movers = collision.search(&mut ride.body, &mut place, dt);
        let was = ride.measure.was_touching;
        let mut around = Grounded { collision: &mut collision, world: &mut place, movers: &movers, rotor: &disc.phy.wreck_rotor.rays, asked: Vec::new() };
        let out = kiowa.tick(&mut ride, &world_step, None, &curve, &acos, &mut rng, dt, &mut around);
        assert_eq!(out, Crashed::default(), "tick {frame}: the port's crash test asked for something");
        assert_eq!(around.asked, vec![Crashed::default()], "tick {frame}: the crash test ran other than once");
        events.clear();
        pad.poll(dt, &report_of(&cap, row - 1), &mut events);
        kiowa.pad = Pad::from_pad(&pad);

        // The row's flag is the record's, but a row read after the next frame's search has cleared
        // it shows 0: those reads fall on the same ticks in both runs, and on each the port's
        // contact is needed for the body to match (taking it out puts the body off at once).
        let touching = collision.record.touching;
        if touching != (cap.u32(row, "pilot_flag") != 0) {
            assert!(touching, "tick {frame}: the game's record touches, the port's does not");
            cleared.push(frame);
        }
        touched += usize::from(touching);
        if touching && !was {
            downs.push(frame);
        }
        moved_by(&mut moved, &pad_of(&cap.block(row, "heli_a", at::BLOCK_A)));
        compare(&cap, row, &kiowa, &ride, filter);
        ticks += 1;
    }
    assert_eq!(cap.u32(0, "frame_counter"), 1204, "the capture does not start on the image's tick");
    assert_eq!(cap.u32(cap.rows.len() - 1, "frame_counter"), 2057, "the capture does not end on 2057");
    assert_eq!(clips, 0, "the helicopter's touchdown sound has clips: its pick draws");
    assert_eq!(rng.state(), start_rng, "the port's frame drew from the generator");
    assert!(moved.iter().all(|&m| m), "the flight does not use every pad word: {moved:?}");
    assert_eq!(downs.first(), Some(&1816), "the first touchdown is not 1816's");
    assert_eq!(cleared, [1838, 1843, 1862, 1874, 1876, 1877, 1881, 1893], "the rows read after the next search's clear");
    println!("the Kiowa's whole capture: {ticks} ticks (1205..=2057) bit-exact, {touched} of them touching (the flag read cleared on {cleared:?}); touchdowns at {downs:?}");
}

/// **C17: the rotor's probe is the image's.** The rotor's eight wreck rays off the disc's `kiowa.phy`,
/// built into their tree ([`game_sim::kiowa::WreckRotor::probe`]), against the probe C14's image
/// holds at the helicopter's `+0x458`: the root, the seven nodes' volume, centre, half extent and child
/// words, and the eight rays in the tree's order, word for word.
#[test]
fn the_rotors_probe_is_the_images() {
    let Some(disc) = the_disc() else { return };
    let ee = Ee::from_ram(&ctx(STATE).unwrap()).unwrap();
    let p = ee.u32_at(HELI + 0x458);
    let want = disc.phy.wreck_rotor.probe();
    assert_eq!(ee.u16_at(p + 0x160), want.root, "the root");
    assert_eq!(ee.u32_at(p + 0x168) as usize, want.segments.len(), "the ray count");
    assert_eq!(want.nodes.len(), 7, "the nodes");
    let nodes = ee.u32_at(p + 0x16c);
    for (k, node) in want.nodes.iter().enumerate() {
        let a = nodes + 0x24 * k as u32;
        let words: Vec<u32> = [0, 12, 16, 20, 24, 28, 32].iter().map(|&o| ee.u32_at(a + o)).collect();
        let mine: Vec<u32> = [node.volume].iter().chain(&node.centre).chain(&node.half_extent).map(|v| v.to_bits()).collect();
        assert_eq!(words, mine, "node {k}'s volume, centre and half extent");
        assert_eq!([ee.u16_at(a + 8), ee.u16_at(a + 10)], node.children, "node {k}'s children");
    }
    let at = ee.u32_at(p + 0x170);
    for (k, s) in want.segments.iter().enumerate() {
        let got = |o: u32| (0..4).map(|l| ee.u32_at(at + 0x20 * k as u32 + o + 4 * l)).collect::<Vec<_>>();
        assert_eq!((got(0), got(0x10)), (s.from.0.map(f32::to_bits).to_vec(), s.to.0.map(f32::to_bits).to_vec()), "ray {k}");
    }
    println!("the rotor's probe: {} rays, {} nodes, word for word", want.segments.len(), want.nodes.len());
}

/// The helicopter, its world and its collision object as C14's image holds them at the tick before
/// the tapes' first row (the seed [`the_ground_at_both_ends_replays_bit_exact`] builds), for a tape
/// played from the same state.
struct Start<'a> {
    kiowa: Kiowa,
    ride: Ride,
    collision: Collision,
    place: Wooded<Place<'a>>,
    filter: Filter,
    curve: CurveTable,
    rotor: Vec<[Vf; 2]>,
}

fn c14_start<'a>(ee: &Ee, disc: &Free09, props: Props, terrain: &'a Terrain<'a>) -> Start<'a> {
    let (r, a) = (ee.u32_at(HELI + at::ROTOR), ee.u32_at(HELI + at::AERO));
    let pf = ee.u32_at(HELI + at::FILTER);
    let filter = Filter { value: f(ee, pf), time: f(ee, pf + 4), step: f(ee, pf + 8) };
    let terrain_at = ee.u32_at(ee.u32_at(HELI + at::SCENE) + at::TERRAIN);
    let e: Vec<F> = (0..4).map(|k| f(ee, terrain_at + at::EXTENT + 4 * k)).collect();
    let curve = the_curve_table(ee);
    let ha = image_block(ee, at::BLOCK_A + HELI, 0xb0);
    let ha = Block { base: at::BLOCK_A, bytes: ha.bytes };
    let b_at = ee.u32_at(HELI + heli_at::BODY);
    let body = body_of(&Block { base: 0, bytes: image_block(ee, b_at, 0x200).bytes });
    let kiowa = Kiowa {
        controller: controller_of(ee),
        rotor: rotor_of(ee, r),
        aero: aero_of(ee, a),
        pad: pad_of(&ha),
        commands: commands_of(&ha),
        control: loop_of(&ha, filter),
        rotor_state: rotor_state_of(&Block { base: ROTOR_STATE, bytes: image_block(ee, r + ROTOR_STATE, 0x50).bytes }),
        aero_state: aero_state_of(&Block { base: AERO_OUT, bytes: image_block(ee, a + AERO_OUT, 0x1c).bytes }),
        wrecked: false,
    };
    let rows = |o: u32| quad(ee, HELI + at::ROWS + o);
    let m = |o: u32| quad(ee, HELI + 0x4d0 + o);
    let measure = Measure {
        speed: f(ee, HELI + at::SPEED),
        alpha: f(ee, HELI + at::SPEED + 4),
        beta: f(ee, HELI + at::SPEED + 8),
        world_rates: rows(0),
        velocity: rows(0x10),
        forward: rows(0x20),
        up: rows(0x30),
        euler: rows(0x40),
        matrix: game_sim::math::Mat4([m(0), m(0x10), m(0x20), m(0x30)]),
        was_touching: ee.u32_at(HELI + heli_at::LAST_TOUCHING) != 0,
    };
    let clips = ee.u32_at(ee.u32_at(HELI + heli_at::TOUCHDOWN_SOUND) + heli_at::CLIPS);
    let ride = Ride {
        body,
        measure,
        crash: Crash {
            reason: ha.u32(at::WRECKED),
            down: ha.u32(at::DOWN) != 0,
            min_wreck_speed: f(ee, HELI + at::MIN_WRECK_SPEED),
            wreck_time: f(ee, HELI + at::WRECK_TIME),
            clock: f(ee, HELI + at::CLOCK),
        },
        flown: ha.u32(at::FLOWN) != 0,
        gravity_scale: f(ee, HELI + at::GRAVITY_SCALE),
        throw_scale: f(ee, HELI + at::THROW_SCALE),
        gravity: quad(ee, HELI + at::GRAVITY),
        area: Some(Area::new(e[0], e[1], e[2], e[3])),
        touchdown_sound: clips != 0,
        rotor_angle: F::ZERO,
        rotor_hidden: false,
    };
    let inertia = quad(ee, b_at + heli_at::INERTIA);
    let mut collision = Collision::new(&disc.phy.mass_properties, &ride.body, inertia, Rays::of(&disc.rays));
    check_collision(ee, &collision);
    let ground = Ground::new(terrain, &disc.scene.terrain);
    let (trees, missed) = Trees::on_ground(&disc.esb, &ground, &curve).expect("free09's ecosystem");
    assert!(missed.is_empty(), "free09's trees missing: {missed:?}");
    let mut place = Wooded { world: Place { ground, props }, trees };
    collision.refresh(&ride.body, &mut place);
    Start { kiowa, ride, collision, place, filter, curve, rotor: disc.phy.wreck_rotor.rays.to_vec() }
}

/// C17's strike tape: C14's state, then Cross for 6 ticks, 40 ticks' rest, L1 for 15 and rest
/// (`research/c14/make_tape.py` fields `60 12:cross 80 30:l1 418`). The helicopter climbs about 7 ft,
/// settles, and the left rudder swings a rotor ray into tree 138's trunk.
const STRIKE: &str = "capture/c17/strike.csv";
/// C19's capture of the same tape with the rotor probe's ray record (`watch/c19-kiowa-strike.toml`).
const PROBE: &str = "capture/c19/probe.csv";
/// The row the original's wreck word turns to 1 on.
const STRUCK_ON: u32 = 1298;
/// The row the wrecked helicopter is put back on: its clock past `WreckTime`.
const PUT_BACK_ON: u32 = 1449;
/// The rows the thrown blades push the hull on (C21): their rays on `kiowaf.col` the only contacts on
/// the hull's record (its mirrors), the original's body read after each of these frames.
const PUSHED: std::ops::RangeInclusive<u32> = 1299..=1302;

/// More of the helicopter's fields the wreck reads (research/oracle.txt, S220a1).
mod wreck_at {
    /// The body as it was built: the inertia the wreck scales.
    pub const INERTIA: u32 = 0x410;
    /// The body wrapper's place, and the MX spawn: three quads each.
    pub const PLACE: u32 = 0xe0;
    pub const SPAWN: u32 = 0xcd0;
    /// The owner: none for a player's helicopter, whose put-back goes to the spawn.
    pub const OWNER: u32 = 0xb94;
    pub const ROTOR_ANGLE: u32 = 0xe58;
    /// The four blade wrappers, and a wrapper's edge object with its bounds.
    pub const BLADES: u32 = 0xe48;
    pub const BLADE_EDGE: u32 = 0x100;
    pub const EDGE_MIN: u32 = 0x30;
    pub const EDGE_MAX: u32 = 0x40;
}

/// Files out of `machine.res`, by their names' ends.
fn machine_files<const N: usize>(names: [&str; N]) -> Option<[Vec<u8>; N]> {
    let mut disc = Disc::open()?;
    let machine = disc.member("machine.res").expect("machine.res is in MX.PSR");
    let res = game_sim::archive::Res::parse(&machine).expect("machine.res");
    Some(names.map(|n| entry(&res, &machine, n).unwrap_or_else(|| panic!("machine.res has no {n}"))))
}

/// The engine round the struck helicopter: C14a's [`Grounded`], and what the crash test asks for
/// done as the MX helicopter does it ([`game_sim::kiowa::Wreckage::act`]), the blades thrown from
/// where the model's nodes stand them that frame ([`game_sim::kiowa::Throw::of`]).
struct Struck<'a> {
    ground: Grounded<'a>,
    wreckage: &'a mut game_sim::kiowa::Wreckage,
    tree: &'a game_sim::kiowa::RotorTree,
    curve: &'a CurveTable,
    put_back: Option<F>,
    /// The rotor probe's first hit, the frame's strike searched it.
    first_hit: Option<game_sim::contact_search::Hit>,
    /// What the blades' springs read and their contacts' movers, for their share of the world's step.
    tables: &'a game_sim::ragdoll::Tables,
    blade_movers: &'a [game_sim::contact::Mover],
    /// The body as each substep's response found it and left it, in order: where a row the capture
    /// read inside the next frame's step stands ([`compare_torn`]).
    substeps: Vec<Body>,
    /// The body as the frame's post found it, before a put-back moved it (B24).
    posted: Option<Body>,
}

impl Surroundings for Struck<'_> {
    fn post(&mut self, body: &Body) {
        self.posted = Some(*body);
        self.ground.post(body);
        self.wreckage.blades.gather_round(self.ground.collision, body);
    }
    fn resolve(&mut self, body: &mut Body) {
        let ground = &mut self.ground;
        self.substeps.push(*body);
        self.wreckage.blades.with_others(|others| ground.resolve_among(body, &mut *others));
        self.substeps.push(*body);
    }
    fn among(&mut self) -> Option<game_sim::kiowa::Among<'_>> {
        Some(self.wreckage.blades.among(self.tables, self.blade_movers, &mut *self.ground.collision))
    }
    fn among_listed(&self, list: game_sim::biplane::List) -> i32 {
        self.wreckage.blades.set.listed(list)
    }
    fn touching(&self) -> bool {
        self.ground.touching()
    }
    fn ignored(&self) -> bool {
        self.ground.ignored()
    }
    fn entries(&self) -> Vec<Touch> {
        self.ground.entries()
    }
    fn strike(&mut self, matrix: &game_sim::math::Mat4) -> Strike {
        let g = &mut self.ground;
        self.first_hit = g.collision.rotor_first_hit(g.rotor, matrix, &mut *g.world);
        self.ground.strike(matrix)
    }
    fn crashed(&mut self, asked: &Crashed, ride: &mut Ride, rng: &mut Rng) {
        self.ground.crashed(asked, ride, rng);
        let throw = game_sim::kiowa::Throw::of(self.tree, &ride.body, ride.rotor_angle, self.curve);
        // S148's helicopter has no owner: the put-back goes to the spawn.
        if let Some(f) = self.wreckage.act(asked, ride, &throw, false, rng) {
            self.put_back = Some(f);
        }
    }
}

/// **A row of the wrecked fall** (C21): every word as [`compare`] has it, but the body may be the one
/// the capture read inside the next frame's world step, the original's frame having gone on by the
/// time PINE read the row: the port's body after the row's frame, or as one of the next frame's
/// substep responses found it or left it (`next`, in order), whichever equals the row's every
/// quadword bit for bit. A row read part-way through a response (between two contacts) has its
/// four accumulators — the force and the moment, and their last — part-summed: when no candidate
/// equals the row whole, the first whose every other quadword does is taken, and those four are
/// not compared. With none, the row is compared after its frame (and fails).
///
/// A row read after its frame's step but before or part-way through its post (from 1474) has the
/// body after the frame and the measure's figures as [`Measure::flight`] was leaving them: its first
/// few groups, in the order it writes them ([`measure_cut`]), the frame's, the rest still `before`'s
/// (the measure the frame started with). The cut is the most of them that equals the row; the body
/// must then be the one after the frame.
///
/// Gives where the row was read — `0` after its frame, `1 + i` at `next[i]` — whether its
/// accumulators were left out, and how many of the measure's groups were written.
fn compare_torn(cap: &Capture, row: usize, kiowa: &Kiowa, ride: &Ride, before: &Measure, filter: Filter, next: &[Body]) -> (usize, bool, usize) {
    let want = body_of(&cap.block(row, "body_raw", 0));
    let candidates: Vec<Body> = std::iter::once(ride.body).chain(next.iter().copied()).collect();
    let (phase, part) = body_read(cap, row, &candidates).unwrap_or((0, false));
    let wb = cap.block(row, "heli_b", at::BLOCK_B);
    let measured = |m: &Measure| measure_words(m).into_iter().zip(captured_measure(&wb)).all(|((_, got), want)| got.to_bits() == want.to_bits());
    let cut = (0..=MEASURE_GROUPS).rev().find(|&cut| measured(&measure_cut(&ride.measure, before, cut))).unwrap_or(MEASURE_GROUPS);
    let frame = cap.u32(row, "frame_counter");
    assert!(cut == MEASURE_GROUPS || (phase, part) == (0, false), "tick {frame}: a row read in its post has its body other than after the frame");
    let mut ride = *ride;
    ride.body = candidates[phase];
    ride.measure = measure_cut(&ride.measure, before, cut);
    if part {
        let b = &mut ride.body;
        (b.last_force, b.force, b.moment, b.last_moment) = (want.last_force, want.force, want.moment, want.last_moment);
    }
    compare(cap, row, kiowa, &ride, filter);
    (phase, part, cut)
}

/// The force and the moment, and their last: the body's quadwords a row read part-way through a
/// substep's response has part-summed ([`compare_torn`]).
const ACCUMULATORS: [u32; 4] = [0x80, 0x90, 0x190, 0x1a0];

/// Which of `candidates` the row's body is ([`compare_torn`]): the first equal to its every quadword
/// bit for bit, else the first equal to all but its [`ACCUMULATORS`] (`true`: read part-way).
fn body_read(cap: &Capture, row: usize, candidates: &[Body]) -> Option<(usize, bool)> {
    let want = body_of(&cap.block(row, "body_raw", 0));
    let equal = |b: &Body, all: bool| {
        QUADS.iter().zip(body_quads(b).into_iter().zip(body_quads(&want))).all(|((_, at, lanes), (got, want))| {
            (!all && ACCUMULATORS.contains(at)) || (0..*lanes).all(|k| got.0[k].to_bits() == want.0[k].to_bits())
        })
    };
    let at = |all: bool| candidates.iter().position(|b| equal(b, all)).map(|i| (i, !all));
    at(true).or_else(|| at(false))
}

/// [`Measure::flight`]'s groups, in the order it writes them: the speed, the body velocity, the world
/// rates, alpha, beta, the Euler angles one lane at a time as [`game_sim::math::quat_to_euler`] works
/// them out (`y`, `z`, `x`: 1474 has `y` and `z` written and not `x`), the matrix with its forward
/// and up rows.
const MEASURE_GROUPS: usize = 9;

/// The measure part-way through [`Measure::flight`]: `after`'s first `cut` groups
/// ([`MEASURE_GROUPS`]), `before`'s rest.
fn measure_cut(after: &Measure, before: &Measure, cut: usize) -> Measure {
    let mut m = *before;
    let groups: [&dyn Fn(&mut Measure); MEASURE_GROUPS] = [
        &|m| m.speed = after.speed,
        &|m| m.velocity = after.velocity,
        &|m| m.world_rates = after.world_rates,
        &|m| m.alpha = after.alpha,
        &|m| m.beta = after.beta,
        &|m| m.euler.0[1] = after.euler.0[1],
        &|m| m.euler.0[2] = after.euler.0[2],
        &|m| m.euler.0[0] = after.euler.0[0],
        &|m| (m.matrix, m.forward, m.up) = (after.matrix, after.forward, after.up),
    ];
    groups.iter().take(cut).for_each(|g| g(&mut m));
    m
}

/// What the original's rotor probe recorded on the strike (`rotor probe + 0x6c`), against the port's
/// first hit: one hit, its point the port's bit for bit, and `objects[0]` a trunk (`TREE`).
fn the_probe_record(probe: &Capture, frame: u32, first: Option<game_sim::contact_search::Hit>) {
    use game_sim::flight_search::{TREE, TREES, category};
    let row = probe.row_at(frame);
    let first = first.unwrap_or_else(|| panic!("tick {frame}: the port's rotor probe struck nothing"));
    assert_eq!(probe.u32(row, "rotor_count"), 1, "tick {frame}: the original's probe struck other than once");
    let points = probe.bytes(row, "rotor_points");
    let lane = |i: usize| u32::from_le_bytes(points[i * 4..i * 4 + 4].try_into().unwrap());
    let point = [first.point.x(), first.point.y(), first.point.z()];
    for (i, got) in point.into_iter().enumerate() {
        assert_eq!(got.to_bits(), lane(i), "tick {frame}: the first hit's point, lane {i}");
    }
    assert_eq!(probe.u32(row, "struck_category"), TREE, "tick {frame}: the original's objects[0] is not a trunk");
    assert_eq!(category(first.struck.object), TREE, "tick {frame}: the port's first hit is not a trunk");
    println!("tick {frame}: the rotor probe struck tree {} at {point:?}", first.struck.object - TREES);
}

/// **C17/C19: a captured rotor strike replays bit-exact up to the wreck, and the wreck to the
/// put-back.** The port's frame replays the strike tape's capture from C14's image, as
/// [`the_ground_at_both_ends_replays_bit_exact`] replays the flight:
///
/// * up to the strike, every row bit for bit, with no wreck asked;
/// * on [`STRUCK_ON`], bit for bit. The crash test wrecks the helicopter (`CRASHED`) with the rotor
///   neither in the ground nor touching, so by the rotor's contact search alone. Its first hit is
///   what the original's probe recorded ([`PROBE`]): one hit, on the same point, and `objects[0]` a
///   trunk (the port's tree 138). [`game_sim::kiowa::Wreckage::act`] does it as the MX helicopter
///   does: the base wreck, then the four blades thrown off the disc's `wreckblade.phy` from where
///   `kiowa.psg`'s nodes stand them. Their twelve `rand()`s are the twelve the original's generator
///   moved on that row. The port's generator is seeded from the row before's `rng_state` and must
///   end on the row's;
/// * the wrecked fall, with the thrown blades in the helicopter's world, as the shell runs them, every
///   row up to the put-back bit for bit (C21). On [`PUSHED`] the blades' rays meet the hull's
///   `kiowaf.col` ([`game_sim::kiowa::Blades::search_on`]) and the hull's record holds only their
///   mirrors; the blades step after the helicopter in its world's step ([`Surroundings::among`]),
///   each blade's contact on the hull naming the helicopter and each mirror its blade. From 1373 the
///   capture reads many rows inside the next frame's step: those are compared against the port's
///   substeps ([`compare_torn`]). The crash test asks nothing more while the clock runs;
/// * on [`PUT_BACK_ON`], the put-back by the clock ([`game_sim::kiowa::Wreckage::put_back`]), the
///   placed body stepped once from the next frame, as in C24's eight wrecks. The capture's reads
///   have drifted a frame late by then (R26): through the fall they move deeper into the next
///   frame's substeps, and from [`PUT_BACK_ON`] on each row holds the frame after it, so each is
///   compared after the next frame, and from 1474 many inside that frame's post, part-way through
///   the measure ([`compare_torn`]). The last row's next frame is not in the capture.
///
/// Off the strike row the port's frame draws nothing (the original's generator moves on other
/// rows: see the C19 journal entry).
#[test]
fn a_captured_rotor_strike_replays_bit_exact() {
    use game_sim::kiowa::{BLADE_MODEL, BLADE_RAYS, BLADE_SHAPE, Blades, KIOWA_HULL, KIOWA_MODEL, RotorTree, WRECK_BLADE_PHY, Wreckage, rotor_in_ground};
    let (Some(cap_path), Some(image)) = (ctx(STRIKE), ctx(STATE)) else { return };
    let Some(mut disc) = the_disc() else { return };
    let Some([blade_phy, blade_psg, blade_rays, blade_shape, kiowa_psg, hull]) =
        machine_files([WRECK_BLADE_PHY, BLADE_MODEL, BLADE_RAYS, BLADE_SHAPE, KIOWA_MODEL, KIOWA_HULL])
    else {
        return;
    };
    let cap = Capture::load(&cap_path);
    let probe = ctx(PROBE).map(|p| Capture::load(&p));
    let ee = Ee::from_ram(&image).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(cap.u32(0, "frame_counter"), 1204, "the capture does not start on the image's tick");
    let props = std::mem::take(&mut disc.props);
    let terrain = Terrain::parse(&disc.terrain).expect("free09's terrain");
    let mut s = c14_start(&ee, &disc, props, &terrain);
    s.ride.rotor_angle = f(&ee, HELI + wreck_at::ROTOR_ANGLE);
    compare(&cap, 0, &s.kiowa, &s.ride, s.filter);

    // What the wreck reads: the image's figures, place and spawn, and the blades off the disc with
    // the first wrapper's edge (the terrain's).
    assert_eq!(ee.u32_at(HELI + wreck_at::OWNER), 0, "the helicopter has an owner: its put-back keeps its place");
    let edge = ee.u32_at(ee.u32_at(HELI + wreck_at::BLADES) + wreck_at::BLADE_EDGE);
    let lane = |a: u32| F(ee.f32_at(edge + a));
    let edge = game_sim::prop_world::Edge {
        min: [lane(wreck_at::EDGE_MIN), lane(wreck_at::EDGE_MIN + 8)],
        max: [lane(wreck_at::EDGE_MAX), lane(wreck_at::EDGE_MAX + 8)],
    };
    let blades = Blades::new(&blade_phy, &blade_psg, Some(&blade_rays), Some(&blade_shape), Some(edge)).and_then(|b| b.with_hull(&hull)).expect("the blades");
    let mut wreckage = Wreckage::new(&disc.phy, quad(&ee, HELI + wreck_at::INERTIA), disc.phy.mass_properties.mass);
    let three = |a: u32| [quad(&ee, a), quad(&ee, a + 16), quad(&ee, a + 32)];
    wreckage.place = three(HELI + wreck_at::PLACE);
    wreckage.spawn = three(HELI + wreck_at::SPAWN);
    wreckage.blades = blades;
    let tree = RotorTree::read(&kiowa_psg).expect("kiowa.psg's nodes");
    let tables = game_sim::ragdoll::Tables::build();

    let acos = AcosTable::build();
    let world_step = World { iterations: 1, rate: F(115.0) };
    let mut pad = game_sim::input::Pad::new(0);
    pad.connected = true;
    let mut events = EventQueue::new();
    let mut rng = Rng::from_state(0x0c17_0c17_0c17_0c17);
    let mut moved = [false; 5];
    let (mut wrecks, mut put_backs, mut thrown) = (Vec::new(), Vec::new(), 0);
    // A row of the fall, waiting on the next frame's substeps for its body, and how many rows stood
    // at each place in that frame.
    let mut torn: Option<(usize, Kiowa, Ride, Measure)> = None;
    // A row read a frame late (from the put-back on), waiting for the end of the next frame.
    let mut late: Option<usize> = None;
    let mut phases = std::collections::BTreeMap::new();
    let (mut parts, mut in_post, mut struck_blades) = (0, 0, 0);
    let rng_of = |row: usize| -> u64 { cap.text(row, "rng_state").parse().expect("rng_state") };
    for row in 1..cap.rows.len() {
        let frame = cap.u32(row, "frame_counter");
        let dt = F(cap.f32(row, "dt"));
        if frame == STRUCK_ON {
            rng = Rng::from_state(rng_of(row - 1));
        }
        let drawn_from = rng.state();
        let measured_before = s.ride.measure;
        // The helicopter's rays meet the blades its post gathered (C24), its mirrors on them
        // filed ahead of their own entries.
        let (movers, struck) = s.collision.search_knocking(&mut s.ride.body, &mut s.place, dt, &wreckage.blades.gathered());
        struck_blades += struck.len();
        // The blades share the helicopter's world, as the shell's `KiowaRide::step` runs them: they
        // search after it, their rays meeting its hull, and step after it in its world's step.
        let blades_out = wreckage.blades.started.iter().any(|b| b.out);
        let mut blade_movers = Vec::new();
        if blades_out {
            let (field, props, trees) = game_sim::world::World::wooded(&mut s.place);
            wreckage.blades.search_on(dt, field.as_deref(), props, trees, &mut s.collision, &s.ride.body);
            wreckage.blades.file_struck(struck);
            blade_movers = props.map(|p| game_sim::flight_search::movers(&p.props)).unwrap_or_default();
        }
        let ground = Grounded { collision: &mut s.collision, world: &mut s.place, movers: &movers, rotor: &s.rotor, asked: Vec::new() };
        let mut around = Struck {
            ground,
            wreckage: &mut wreckage,
            tree: &tree,
            curve: &s.curve,
            put_back: None,
            first_hit: None,
            tables: &tables,
            blade_movers: &blade_movers,
            substeps: Vec::new(),
            posted: None,
        };
        let out = s.kiowa.tick(&mut s.ride, &world_step, None, &s.curve, &acos, &mut rng, dt, &mut around);
        let asked = std::mem::take(&mut around.ground.asked);
        let put_back = around.put_back;
        let first_hit = around.first_hit;
        let substeps = std::mem::take(&mut around.substeps);
        if let Some((at, kiowa, ride, before)) = torn.take() {
            let (phase, part, cut) = compare_torn(&cap, at, &kiowa, &ride, &before, s.filter, &substeps);
            let read = cap.u32(at, "frame_counter");
            if PUSHED.contains(&read) {
                assert_eq!((phase, part), (0, false), "tick {read}: the blades' push was not read after its frame");
            }
            *phases.entry(phase).or_insert(0) += 1;
            parts += part as usize;
            in_post += usize::from(cut < MEASURE_GROUPS);
        }
        if blades_out || wreckage.blades.started.iter().any(|b| b.out) {
            wreckage.blades.post(dt);
        }
        if let Some(fraction) = put_back {
            s.kiowa.put_back_controls(fraction);
            s.collision.refresh(&s.ride.body, &mut s.place);
            wreckage.blades.gather_round(&s.collision, &s.ride.body);
            put_backs.push(frame);
        }
        events.clear();
        pad.poll(dt, &report_of(&cap, row - 1), &mut events);
        s.kiowa.pad = Pad::from_pad(&pad);
        assert_eq!(asked, vec![out], "tick {frame}: the crash test ran other than once");
        if let Some(reason) = out.wreck {
            wrecks.push((frame, reason));
        }
        assert!(!out.debris, "tick {frame}: the crash test threw the blades on their own");
        if frame == STRUCK_ON {
            assert_eq!(out.wreck, Some(game_sim::biplane::CRASHED), "tick {frame}: the port's crash test did not wreck");
            // What wrecked it: the rotor's contact search, not the ground under it nor a contact.
            let matrix = s.ride.measure.matrix;
            let ends: Vec<Vf> = s.rotor.iter().map(|r| r[1]).collect();
            assert!(!rotor_in_ground(&matrix, &ends, |p| game_sim::world::World::ground(&mut s.place, p).0), "tick {frame}: the rotor is in the ground");
            assert!(!s.collision.record.touching, "tick {frame}: the helicopter touches");
            // The four blades' three draws each are the original's whole move on this row.
            assert_eq!(rng.state(), rng_of(row), "tick {frame}: the blade throw's draws are not the original's");
            let mut twelve = Rng::from_state(drawn_from);
            (0..12).for_each(|_| _ = twelve.rand());
            assert_eq!(rng.state(), twelve.state(), "tick {frame}: the throw drew other than 12");
            thrown = wreckage.blades.started.iter().filter(|b| b.out).count();
        } else {
            assert_eq!(rng.state(), drawn_from, "tick {frame}: the port's frame drew from the generator");
        }
        moved_by(&mut moved, &pad_of(&cap.block(row, "heli_a", at::BLOCK_A)));
        if let Some(at) = late.take() {
            let (phase, _, cut) = compare_torn(&cap, at, &s.kiowa, &s.ride, &measured_before, s.filter, &[]);
            *phases.entry(phase).or_insert(0) += 1;
            in_post += usize::from(cut < MEASURE_GROUPS);
        }
        if frame <= STRUCK_ON {
            compare(&cap, row, &s.kiowa, &s.ride, s.filter);
        } else if frame < PUT_BACK_ON {
            torn = Some((row, s.kiowa, s.ride, measured_before));
        } else {
            late = Some(row);
        }
        if frame == STRUCK_ON
            && let Some(probe) = &probe
        {
            the_probe_record(probe, frame, first_hit);
        }
    }
    // The last row was read in the frame after it, which the capture does not reach.
    assert!(torn.is_none(), "the fall's last row is before the put-back");
    assert_eq!(late, Some(cap.rows.len() - 1), "the last row is not one read a frame late");
    assert_eq!(wrecks, vec![(STRUCK_ON, game_sim::biplane::CRASHED)], "the wrecks asked");
    assert_eq!(put_backs, vec![PUT_BACK_ON], "the put-backs");
    assert_eq!(thrown, 4, "the blades thrown");
    println!("the fall's rows by where they were read (0 after the frame, 1 + i at the next frame's next[i]): {phases:?}, {parts} part-way through a response, {in_post} inside a post");
    // The blades fly off outward from the hub: the helicopter's rays never meet one (C24).
    assert_eq!(struck_blades, 0, "frames whose helicopter rays met a gathered blade");
    assert!(moved[0] && moved[4], "the tape's climb and pedal are not in the capture: {moved:?}");
    let last = cap.u32(cap.rows.len() - 1, "frame_counter");
    println!("the rotor strike: 1205..={STRUCK_ON} bit-exact, run on to {last}; wrecked on {STRUCK_ON} by the rotor's contact search, 4 blades thrown on the original's 12 draws, put back on {PUT_BACK_ON}");
}

/// Whether the row's flight block — the wreck word, the pad's words and the flight state — is
/// `kiowa`'s with the wreck word `reason`, bit for bit.
fn flight_block_is(cap: &Capture, row: usize, kiowa: &Kiowa, reason: u32, filter: Filter) -> bool {
    let wa = cap.block(row, "heli_a", at::BLOCK_A);
    let (want, got) = (pad_of(&wa), kiowa.pad);
    let pad = [(got.climb, want.climb), (got.descend, want.descend), (got.lateral, want.lateral), (got.longitudinal, want.longitudinal), (got.pedal, want.pedal)];
    wa.u32(at::WRECKED) == reason && pad.iter().all(|(g, w)| g.to_bits() == w.to_bits()) && flight_mismatch(cap, row, kiowa, filter).is_none()
}

/// The helicopter's flight state as each of the next frame's force listeners leaves it (B24): its
/// controller's commands and loop, the rotor's state and the aerodynamics', from the next frame's
/// readings and pad. They read the substep's body (`body` for the first, then `next`'s bodies as each
/// substep's response found them, every other one), the frame's measure and record.
fn next_forces(kiowa: &Kiowa, ride: &Ride, touching: bool, curve: &CurveTable, h: F, next: &[Body]) -> Vec<Kiowa> {
    let r = ride.measure.readings(ride.flown, touching, ride.gravity_scale);
    let mut kiowa = *kiowa;
    kiowa.wrecked = ride.crash.reason != 0;
    let bodies = std::iter::once(ride.body).chain(next.iter().skip(2).step_by(2).copied());
    bodies
        .enumerate()
        .map(|(i, mut body)| {
            if i == 0 {
                kiowa.preforce(&mut body);
            }
            kiowa.force(&r, &mut body, curve, h, |_| {});
            kiowa
        })
        .collect()
}

/// The flight state's groups, in the order the force listener writes them: the controller's commands
/// and loop; the rotor's step's tilt and blade pitches ([`game_sim::kiowa::Rotor::step`] writes those
/// first), its thrust, its force; the aerodynamics' state. 1422 of C24's fast2 was read with the
/// rotor's pitches written and its thrust not.
const FLIGHT_GROUPS: usize = 5;

/// The flight state part-way through one of the next frame's force listeners: `early`'s (the
/// listener's) first `cut` groups ([`FLIGHT_GROUPS`]), `after`'s (the state it started on) rest.
fn flight_cut(after: &Kiowa, early: &Kiowa, cut: usize) -> Kiowa {
    let mut k = *after;
    let groups: [&dyn Fn(&mut Kiowa); FLIGHT_GROUPS] = [
        &|k| (k.commands, k.control) = (early.commands, early.control),
        &|k| {
            let (r, e) = (&mut k.rotor_state, &early.rotor_state);
            (r.tilt, r.collective_pitch, r.tail_pitch) = (e.tilt, e.collective_pitch, e.tail_pitch);
        },
        &|k| k.rotor_state.thrust = early.rotor_state.thrust,
        &|k| k.rotor_state.force = early.rotor_state.force,
        &|k| k.aero_state = early.aero_state,
    ];
    groups.iter().take(cut).for_each(|g| g(&mut k));
    k
}

/// **Where a flight row's flight state was read** (B24): the earliest point in the next frame's
/// force listeners ([`next_forces`], each written group by group, [`flight_cut`]) at which the row's
/// every word equals the port's. Gives the flight state and the point, `0` after the frame and
/// `k * FLIGHT_GROUPS + c` with `c` of substep `k`'s groups written. With none, the frame's own,
/// which [`compare`] then fails on.
fn flight_read(cap: &Capture, row: usize, after: &Kiowa, next: &[Kiowa], filter: Filter) -> (Kiowa, usize) {
    let bases = std::iter::once(after).chain(next.iter());
    let points = bases.zip(next).flat_map(|(base, early)| (1..=FLIGHT_GROUPS).map(move |cut| flight_cut(base, early, cut)));
    std::iter::once(*after)
        .chain(points)
        .enumerate()
        .find(|(_, k)| flight_mismatch(cap, row, k, filter).is_none())
        .map_or((*after, 0), |(at, k)| (k, at))
}

/// C24's roll tape: C14's state, 11 ticks' rest, then the stick hard right for 150 ticks on the ground
/// and rest (`research/c14/make_tape.py` fields `22 300:-:255,127 250`). The helicopter rolls onto its
/// right side, the rotor goes into the ground, and blade 3 comes down under the hull; the wreck stays
/// where it fell.
const ROLL: &str = "capture/c24/roll.csv";
/// C24's fast tape: C14's state, 30 ticks' rest, Cross for 40, the stick forward for 100 and then
/// forward with Square held (fields `60 80:cross 200:-:127,0 200:square:127,0 400`). The helicopter
/// climbs to about 210 ft, noses over and flies into the ground at about 30 ft/s; the wreck slides
/// some 70 ft with its blades thrown about it.
const FAST: &str = "capture/c24/fast1.csv";
/// C24's tip tape: C14's state, 20 ticks' rest, then the stick hard forward and left for 150 ticks on
/// the ground and rest (fields `40 300:-:0,0 250`). The helicopter tips over its nose and left side;
/// blade 1 comes down within the hull's rays, a few feet from the probe.
const TIP: &str = "capture/c24/try4.csv";
/// C24's right tip tape: as [`TIP`] but the stick hard forward and right (fields
/// `40 300:-:255,0 250`). Its row for 1304 was read a frame late (B23).
const TIP_RIGHT: &str = "capture/c24/try5.csv";

/// What [`replay_wreck`] saw.
struct Replayed {
    wrecks: Vec<(u32, u32)>,
    put_backs: Vec<u32>,
    /// Frames whose helicopter search was offered at least one blade, and the nearest offered.
    offered: usize,
    nearest: Option<F>,
    /// Frames whose helicopter rays met a blade.
    struck: Vec<u32>,
    /// Rows read a frame late: each holds the body after the next frame (B23).
    late: Vec<u32>,
    /// Rows before the wreck read inside the next frame's step, after its controller had run (B24).
    torn: Vec<u32>,
    /// The last row, when it was read after a frame the capture does not hold (B24).
    unread: Option<u32>,
    last: u32,
}

/// **A wreck replayed from C14's image** (C24), as [`a_captured_rotor_strike_replays_bit_exact`]
/// replays the strike, the helicopter's search offered the blades in its nearby list
/// ([`game_sim::kiowa::Blades::gather_round`], [`game_sim::kiowa::Collision::search_knocking`]),
/// every row compared where it was read ([`compare_torn`]). The wreck's 12 draws are the original's: the generator is reseeded from the
/// row before the wreck, the tapes' draws having parted from the port's by then (the original draws
/// on input). The put-back steps once, as the strike's does (R26).
///
/// A row whose body equals the next row's was read a frame late, after the next frame (R26 has
/// the same in C21's capture from its put-back on), and is compared there.
///
/// Where a row was read drifts (B24, C24's fast2). A flying row read inside the next frame's step
/// has the flight state of one of its force listeners, as far as that listener had written it
/// ([`flight_read`]). A row whose body is neither the frame's nor one of the next frame's substeps'
/// was read after the next frame and is compared there: with the pad that frame ran on, its flight
/// block read before the next poll; and on a put-back with the wreck word, the flight state and the
/// body from before its post, when the row's flight block shows the put-back not yet done.
fn replay_wreck(capture: &str) -> Option<Replayed> {
    use game_sim::kiowa::{BLADE_MODEL, BLADE_RAYS, BLADE_SHAPE, Blades, KIOWA_HULL, KIOWA_MODEL, RotorTree, WRECK_BLADE_PHY, Wreckage};
    let (cap_path, image) = (ctx(capture)?, ctx(STATE)?);
    let mut disc = the_disc()?;
    let [blade_phy, blade_psg, blade_rays, blade_shape, kiowa_psg, hull] =
        machine_files([WRECK_BLADE_PHY, BLADE_MODEL, BLADE_RAYS, BLADE_SHAPE, KIOWA_MODEL, KIOWA_HULL])?;
    let cap = Capture::load(&cap_path);
    let ee = Ee::from_ram(&image).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(cap.u32(0, "frame_counter"), 1204, "the capture does not start on the image's tick");
    let props = std::mem::take(&mut disc.props);
    let terrain = Terrain::parse(&disc.terrain).expect("free09's terrain");
    let mut s = c14_start(&ee, &disc, props, &terrain);
    s.ride.rotor_angle = f(&ee, HELI + wreck_at::ROTOR_ANGLE);

    let edge = ee.u32_at(ee.u32_at(HELI + wreck_at::BLADES) + wreck_at::BLADE_EDGE);
    let lane = |a: u32| F(ee.f32_at(edge + a));
    let edge = game_sim::prop_world::Edge {
        min: [lane(wreck_at::EDGE_MIN), lane(wreck_at::EDGE_MIN + 8)],
        max: [lane(wreck_at::EDGE_MAX), lane(wreck_at::EDGE_MAX + 8)],
    };
    let blades = Blades::new(&blade_phy, &blade_psg, Some(&blade_rays), Some(&blade_shape), Some(edge)).and_then(|b| b.with_hull(&hull)).expect("the blades");
    let mut wreckage = Wreckage::new(&disc.phy, quad(&ee, HELI + wreck_at::INERTIA), disc.phy.mass_properties.mass);
    let three = |a: u32| [quad(&ee, a), quad(&ee, a + 16), quad(&ee, a + 32)];
    wreckage.place = three(HELI + wreck_at::PLACE);
    wreckage.spawn = three(HELI + wreck_at::SPAWN);
    wreckage.blades = blades;
    let tree = RotorTree::read(&kiowa_psg).expect("kiowa.psg's nodes");
    let tables = game_sim::ragdoll::Tables::build();

    let acos = AcosTable::build();
    let world_step = World { iterations: 1, rate: F(115.0) };
    let mut pad = game_sim::input::Pad::new(0);
    pad.connected = true;
    let mut events = EventQueue::new();
    let mut rng = Rng::from_state(0x0c24_0c24_0c24_0c24);
    let wrecked_on = (0..cap.rows.len())
        .map(|r| (cap.u32(r, "frame_counter"), cap.block(r, "heli_a", at::BLOCK_A).u32(at::WRECKED)))
        .find(|w| w.1 != 0)
        .expect("a wreck in the capture")
        .0;
    let rng_of = |row: usize| -> u64 { cap.text(row, "rng_state").parse().expect("rng_state") };
    let mut seen = Replayed { wrecks: Vec::new(), put_backs: Vec::new(), offered: 0, nearest: None, struck: Vec::new(), late: Vec::new(), torn: Vec::new(), unread: None, last: 0 };
    let mut torn: Option<(usize, Kiowa, Ride, Measure)> = None;
    // A row read a frame late, waiting for the end of the next frame (B23).
    let mut late: Option<usize> = None;
    let read_late = |row: usize| row + 1 < cap.rows.len() && cap.text(row, "body_raw") == cap.text(row + 1, "body_raw");
    for row in 1..cap.rows.len() {
        let frame = cap.u32(row, "frame_counter");
        let dt = F(cap.f32(row, "dt"));
        if frame == wrecked_on {
            rng = Rng::from_state(rng_of(row - 1));
        }
        let drawn_from = rng.state();
        // The flight state, the pad and the wreck word the frame starts on (B24).
        let (kiowa_before, reason_before) = (s.kiowa, s.ride.crash.reason);
        let measured_before = s.ride.measure;
        let touching = s.collision.record.touching;
        let offered = wreckage.blades.gathered();
        if !offered.is_empty() {
            seen.offered += 1;
            let near = offered.iter().map(|&(_, _, d)| d).fold(F(f32::INFINITY), F::min);
            seen.nearest = Some(seen.nearest.map_or(near, |n| n.min(near)));
        }
        let (movers, struck) = s.collision.search_knocking(&mut s.ride.body, &mut s.place, dt, &offered);
        if !struck.is_empty() {
            seen.struck.push(frame);
        }
        let blades_out = wreckage.blades.started.iter().any(|b| b.out);
        let mut blade_movers = Vec::new();
        if blades_out {
            let (field, props, trees) = game_sim::world::World::wooded(&mut s.place);
            wreckage.blades.search_on(dt, field.as_deref(), props, trees, &mut s.collision, &s.ride.body);
            wreckage.blades.file_struck(struck);
            blade_movers = props.map(|p| game_sim::flight_search::movers(&p.props)).unwrap_or_default();
        }
        let ground = Grounded { collision: &mut s.collision, world: &mut s.place, movers: &movers, rotor: &s.rotor, asked: Vec::new() };
        let mut around = Struck {
            ground,
            wreckage: &mut wreckage,
            tree: &tree,
            curve: &s.curve,
            put_back: None,
            first_hit: None,
            tables: &tables,
            blade_movers: &blade_movers,
            substeps: Vec::new(),
            posted: None,
        };
        let out = s.kiowa.tick(&mut s.ride, &world_step, None, &s.curve, &acos, &mut rng, dt, &mut around);
        let (put_back, posted) = (around.put_back, around.posted);
        let substeps = std::mem::take(&mut around.substeps);
        let pending = torn.take().filter(|(at, _, ride, _)| {
            // A row whose body is not the frame's, nor one of the next frame's substeps', was read
            // after the next frame (as C21's are from its put-back on, R26): compared there.
            let candidates: Vec<Body> = std::iter::once(ride.body).chain(substeps.iter().copied()).collect();
            let within = body_read(&cap, *at, &candidates).is_some();
            if !within {
                late = Some(*at);
                seen.late.push(cap.u32(*at, "frame_counter"));
            }
            within
        });
        if let Some((at, kiowa, ride, before)) = pending {
            let next = next_forces(&kiowa, &ride, touching, &s.curve, world_step.substeps(dt).1, &substeps);
            let (kiowa, cut) = flight_read(&cap, at, &kiowa, &next, s.filter);
            let read = compare_torn(&cap, at, &kiowa, &ride, &before, s.filter, &substeps);
            let at_frame = cap.u32(at, "frame_counter");
            // The row reads the flight state's blocks before the body's: the body cannot stand earlier
            // in the next frame than they do. Until the first substep's gravity the body is the frame's.
            // A body read part-way through its accumulators stands anywhere up to its substep's
            // integrate, so only a whole one is held to it.
            let (body_substep, flight_substep) = (read.0.saturating_sub(1) / 2, cut.saturating_sub(1) / FLIGHT_GROUPS);
            let (phase, part, _) = read;
            assert!(part || (flight_substep <= body_substep && (phase > 0 || cut <= 1)), "tick {at_frame}: the flight state read at {cut}, after the body ({phase})");
            if at_frame <= wrecked_on && (cut, read) != (0, (0, false, MEASURE_GROUPS)) {
                seen.torn.push(at_frame);
            }
        }
        if blades_out || wreckage.blades.started.iter().any(|b| b.out) {
            wreckage.blades.post(dt);
        }
        if let Some(fraction) = put_back {
            s.kiowa.put_back_controls(fraction);
            s.collision.refresh(&s.ride.body, &mut s.place);
            wreckage.blades.gather_round(&s.collision, &s.ride.body);
            seen.put_backs.push(frame);
        }
        // The placed body steps once, from the next frame: every one of these captures has it on its
        // put-back's row, read after the frame, and the first step on the next.
        events.clear();
        pad.poll(dt, &report_of(&cap, row - 1), &mut events);
        s.kiowa.pad = Pad::from_pad(&pad);
        if let Some(reason) = out.wreck {
            seen.wrecks.push((frame, reason));
        }
        if frame != wrecked_on {
            assert_eq!(rng.state(), drawn_from, "tick {frame}: the port's frame drew from the generator");
        }
        if let Some(at) = late.take() {
            // It was read before the next pad poll, so its pad is the one the frame ran on; and on a
            // put-back it may be before the frame's post put it back (1592 of C24's fast2): the
            // wreck word, the flight state and the body as they were.
            let after = Kiowa { pad: kiowa_before.pad, ..s.kiowa };
            let (kiowa, ride) = if put_back.is_some() && !flight_block_is(&cap, at, &after, s.ride.crash.reason, s.filter) {
                let body = posted.expect("a put-back in the frame's post");
                (kiowa_before, Ride { crash: game_sim::kiowa::Crash { reason: reason_before, ..s.ride.crash }, body, ..s.ride })
            } else {
                (after, s.ride)
            };
            compare_torn(&cap, at, &kiowa, &ride, &measured_before, s.filter, &[]);
        }
        if read_late(row) {
            late = Some(row);
            seen.late.push(frame);
        } else {
            torn = Some((row, s.kiowa, s.ride, measured_before));
        }
        seen.last = frame;
    }
    if let Some((at, kiowa, ride, before)) = torn.take() {
        // The last row, read after a frame the capture does not hold, cannot be compared.
        if body_read(&cap, at, &[ride.body]).is_some() {
            compare_torn(&cap, at, &kiowa, &ride, &before, s.filter, &[]);
        } else {
            seen.unread = Some(cap.u32(at, "frame_counter"));
        }
    }
    assert!(late.is_none(), "the last row was read a frame late");
    Some(seen)
}

/// The roll (C24): bit-exact through its wreck, blade throw, fall and put-back to the capture's
/// end. The wreck does not move far enough for the helicopter to gather its list again, so no blade
/// is ever offered to its rays.
#[test]
fn a_rolled_wreck_replays_bit_exact() {
    let Some(seen) = replay_wreck(ROLL) else { return };
    assert_eq!(seen.wrecks, vec![(1257, game_sim::biplane::CRASHED)], "the wrecks");
    assert_eq!(seen.put_backs, vec![1408], "the put-backs");
    assert_eq!(seen.late, Vec::<u32>::new(), "the rows read a frame late");
    assert_eq!((seen.torn, seen.unread), (Vec::<u32>::new(), None), "the flying rows read in the next frame, the last row unread");
    assert_eq!(seen.offered, 0, "frames whose helicopter search was offered a blade");
    println!("the roll: 1205..={} bit-exact, wrecked on 1257, put back on 1408, no blade offered", seen.last);
}

/// The tip (C24): bit-exact to the capture's end. Blade 1 lies where the helicopter's rays would meet
/// it on 1270, but the wreck never gathers its list again, so it is never offered them and the
/// original's record holds only the blade's own mirrors. (Offered every blade on every post, the port
/// meets it and parts from the game on 1270's speed.)
#[test]
fn a_tipped_wreck_never_meets_its_blades() {
    let Some(seen) = replay_wreck(TIP) else { return };
    assert_eq!(seen.wrecks, vec![(1267, game_sim::biplane::CRASHED)], "the wrecks");
    assert_eq!(seen.put_backs, vec![1418], "the put-backs");
    assert_eq!(seen.late, Vec::<u32>::new(), "the rows read a frame late");
    assert_eq!((seen.torn, seen.unread), (Vec::<u32>::new(), None), "the flying rows read in the next frame, the last row unread");
    assert_eq!(seen.offered, 0, "frames whose helicopter search was offered a blade");
    println!("the tip: 1205..={} bit-exact, wrecked on 1267, put back on 1418, no blade offered", seen.last);
}

/// The fast wreck (C24): bit-exact to the capture's end, its slide gathering the helicopter's list
/// again with the blades out, so its rays are offered them, from 0 ft away, and meet none: nor does
/// the original's record ever hold an entry on a blade but the blades' own mirrors.
#[test]
fn a_fast_wreck_offers_its_blades_bit_exact() {
    let Some(seen) = replay_wreck(FAST) else { return };
    assert_eq!(seen.wrecks, vec![(1451, game_sim::biplane::CRASHED)], "the wrecks");
    assert_eq!(seen.put_backs, vec![1602], "the put-backs");
    assert_eq!(seen.late, Vec::<u32>::new(), "the rows read a frame late");
    assert_eq!((seen.torn, seen.unread), (Vec::<u32>::new(), None), "the flying rows read in the next frame, the last row unread");
    assert!(seen.offered > 0, "the slide never gathered the blades");
    assert_eq!(seen.struck, Vec::<u32>::new(), "frames whose helicopter rays met a blade");
    println!(
        "the fast wreck: 1205..={} bit-exact, wrecked on 1451, put back on 1602; blades offered on {} frames, the nearest {:?} ft off, none met",
        seen.last, seen.offered, seen.nearest
    );
}


/// The right tip (C24's try5, B23): bit-exact to the capture's end. Its row for 1304 holds the same
/// body as 1305's, two frames' motion past 1303's: PINE read it a frame late, and it equals the
/// port after 1305. (Compared after its own frame, it parts on 1304's speed, 2.388822 in the game
/// and 2.5328536 in the port.) The heli's captured contact counts are short of the port's on 11
/// rows from 1275 to 1319, its body bit-exact on each: the record read part-way through the collide
/// pass that clears and refills it.
#[test]
fn a_right_tipped_wreck_read_a_frame_late_replays_bit_exact() {
    let Some(seen) = replay_wreck(TIP_RIGHT) else { return };
    assert_eq!(seen.wrecks, vec![(1268, game_sim::biplane::CRASHED)], "the wrecks");
    assert_eq!(seen.put_backs, vec![1419], "the put-backs");
    assert_eq!(seen.late, vec![1304], "the rows read a frame late");
    assert_eq!((seen.torn, seen.unread), (Vec::<u32>::new(), None), "the flying rows read in the next frame, the last row unread");
    assert_eq!(seen.offered, 0, "frames whose helicopter search was offered a blade");
    println!("the right tip: 1205..={} bit-exact, wrecked on 1268, put back on 1419, 1304 read a frame late, no blade offered", seen.last);
}

/// C24's second fast tape: as [`FAST`] but the stick forward and left with Square held (fields
/// `60 80:cross 200:-:127,0 200:square:0,0 400`). The helicopter descends banked left at speed and
/// wrecks on 1442.
const FAST_LEFT: &str = "capture/c24/fast2.csv";

/// The fast left descent (C24's fast2, B24): bit-exact to the capture's end, its rows read ever
/// later. Seven flying rows from 1416 were read inside the next frame's step, the controller and
/// part of the rotor's and aerodynamics' states already the next frame's first force listener's:
/// 1416's lateral cyclic is -0.08864668, the port's after 1417's first controller, -0.08283156 after
/// 1416. From 1468 the rows are read after the next frame, as C21's are from its put-back (R26),
/// but for 1576, 1584 and 1585, read inside it. 1592's flight block was read before 1593's put-back
/// and the last row, 1672, after a frame the capture does not hold. The slide gathers the blades
/// and its rays meet none.
#[test]
fn a_fast_left_descent_read_ever_later_replays_bit_exact() {
    let Some(seen) = replay_wreck(FAST_LEFT) else { return };
    assert_eq!(seen.wrecks, vec![(1442, game_sim::biplane::CRASHED)], "the wrecks");
    assert_eq!(seen.put_backs, vec![1593], "the put-backs");
    assert_eq!(seen.torn, vec![1416, 1420, 1422, 1436, 1437, 1438, 1439], "the flying rows read in the next frame");
    let late: Vec<u32> = (1468..=1671).filter(|f| ![1576, 1584, 1585].contains(f)).collect();
    assert_eq!(seen.late, late, "the rows read a frame late");
    assert_eq!(seen.unread, Some(1672), "the last row, read after a frame the capture does not hold");
    assert!(seen.offered > 0, "the slide never gathered the blades");
    assert_eq!(seen.struck, Vec::<u32>::new(), "frames whose helicopter rays met a blade");
    println!(
        "the fast left descent: 1205..=1671 bit-exact, {} unread, wrecked on 1442, put back on 1593; {} flying rows read in the next frame, {} rows a frame late; blades offered on {} frames, none met",
        seen.last,
        seen.torn.len(),
        seen.late.len(),
        seen.offered
    );
}
