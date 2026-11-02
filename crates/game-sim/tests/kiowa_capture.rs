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
    assert_eq!(wa.u32(at::WRECKED), 0, "tick {frame}: the helicopter wrecked");
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
    let want = commands_of(&wa);
    for (name, got, want) in [
        ("the collective", kiowa.commands.collective, want.collective),
        ("the lateral cyclic", kiowa.commands.lateral, want.lateral),
        ("the longitudinal cyclic", kiowa.commands.longitudinal, want.longitudinal),
        ("the pedal command", kiowa.commands.pedal, want.pedal),
        ("the filtered collective", kiowa.commands.filtered, want.filtered),
        ("the altitude fraction", kiowa.commands.altitude_fraction, want.altitude_fraction),
    ] {
        same(name, frame, got, want);
    }
    let want = loop_of(&wa, filter);
    for (name, got, want) in [
        ("the speed command", kiowa.control.speed_command, want.speed_command),
        ("the last longitudinal", kiowa.control.last_longitudinal, want.last_longitudinal),
        ("the hold altitude", kiowa.control.hold_altitude, want.hold_altitude),
        ("the hold timer", kiowa.control.hold_timer, want.hold_timer),
        ("the integral", kiowa.control.integral, want.integral),
        ("the filter's value", kiowa.control.filter.value, want.filter.value),
    ] {
        same(name, frame, got, want);
    }
    assert_eq!(kiowa.rotor_state, rotor_state_of(&cap.block(row, "rotor_state", ROTOR_STATE)), "tick {frame}: the rotor's state");
    assert_eq!(kiowa.aero_state, aero_state_of(&cap.block(row, "aero_state", AERO_OUT)), "tick {frame}: the aerodynamics' state");
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
