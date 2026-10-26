//! V78: the biplane's pilot leans with the stick, as the original's does.
//!
//! What the original does, and where each link is proved:
//! - the plane writes its pilot's request every tick from the stick alone — the aileron into the phase
//!   axis, the elevator into the mix axis — and the body plays the plane's record of two clips off
//!   those two axes, eased by its two lags: S35's `game-sim/tests/biplane_anim.rs`, against the human's
//!   biplane recording, which also proves the drawing side's copy of the lags
//!   ([`game_sim::biplane_anim::Lean`]) answers the body's bit for bit;
//! - the shell keeps that lean per seat off the flown plane ([`vehicle::PilotLean`]) and draws the
//!   pilot's nodes from it ([`game_render::biplane::pilot_palette`]) — here: the stick held each way
//!   on the disc's plane flown through the shell's own garage moves the lean to the clips' ends the
//!   original's axes put it at, and moves the drawn pilot; the rudder and the throttle, which the
//!   original's request does not carry, leave the pilot as the centred stick has it.
//!
//! `src/vehicle.rs` is compiled in by path, the way `tests/biplane_surfaces.rs` takes it.

#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;

use game_render::biplane::{self, Biplane};
use game_render::disc::{Disc, Res};
use game_render::mat::M4;
use game_render::rider::{Body, Sex};
use game_render::world::Assets;
use game_sim::Sim;
use game_sim::collide::{Bundle, Record};
use game_sim::input::{AXIS_CENTRE, Control, PADS, Report};
use game_sim::trick::{Set as Stunts24, Stunts};
use game_sim::world::World;
use game_sim::{phy::Phy, rider};
use ps2_float::{F, Vf};
use vehicle::{Garage, Kind, PilotLean, Plane, Spawn};

const DT: F = F(1.0 / 60.0);

/// Flat ground at height 0, nothing solid on it.
struct Flat;

impl World for Flat {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        (Vf::new(at.x(), 0.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0))
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

fn member(disc: &Disc, ends: &str) -> Option<String> {
    let want = ends.to_ascii_lowercase();
    disc.entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with(&want))
        .min_by_key(|e| e.name.len())
        .map(|e| e.name.clone())
}

fn res(disc: &mut Disc, ends: &str) -> Res {
    let name = member(disc, ends).unwrap_or_else(|| panic!("the disc has no {ends}"));
    disc.res(&name).unwrap_or_else(|why| panic!("{name} does not open: {why}"))
}

/// One pass of the shell's pad into the garage, and the seat's pilot lean after it, as the shell's
/// frame steps it.
fn pass(sim: &mut Sim, garage: &mut Garage, lean: &mut PilotLean, report: Report) {
    let mut reports = [Report::missing(); PADS];
    reports[0] = report;
    let controls = sim.poll(DT, &reports);
    let said = garage.step(sim, DT, &controls, &mut Flat);
    lean.step(DT, garage.plane_flown(), said.respawned);
}

fn apart(a: &[M4], b: &[M4]) -> f32 {
    a.iter()
        .zip(b)
        .flat_map(|(a, b)| (0..4).flat_map(move |i| (0..4).map(move |j| (a.0[i][j] - b.0[i][j]).abs())))
        .fold(0.0, f32::max)
}

#[test]
fn holding_the_stick_each_way_leans_the_drawn_pilot() {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => {
            println!("SKIPPED: {why}");
            return;
        }
    };
    let machines = disc.res("machine.res").expect("the machines' archive");
    let phy = game_sim::phy::Plane::read_bytes(&machines.read_suffix(game_sim::phy::BIPLANE_PHY).expect("biplane.phy"));
    let mut assets = Assets::default();
    let drawn = Biplane::load(&mut assets, &[machines], 0).expect("the biplane's model");

    let app = res(&mut disc, "appstart.res");
    let table = app.read_suffix("rider\\anim\\stunts.ini").expect("the app archive has no stunt table");
    let set = Stunts24::new(Stunts::parse(&table).expect("the stunt table parses").selection(0));
    let tables = rider::Tables::build();
    let bike = res(&mut disc, "250sx.res");
    let p = Phy::read_bytes(&bike.read_suffix(".phy").expect("the bike has a .phy"), &tables.ride.curve);
    let mut body = Body::load(vec![res(&mut disc, "thq1.res")], "thq1", Sex::Male, "", &set.stunts, p.motion())
        .unwrap_or_else(|why| panic!("the body did not load: {why}"));

    // Up and flying on the gas, as tests/biplane_surfaces.rs flies it.
    let flight = game_sim::flight::Flight::new(&phy);
    let lowest = flight.tires.iter().map(|t| t.center.y() - t.radius.0).fold(f32::MAX, f32::min);
    let spawn = Spawn { at: Vf::new(400.0, -lowest, -300.0, 1.0), facing: Vf::new(0.0, 0.0, -1.0, 0.0) };
    let mut sim = Sim::new(0);
    let mut garage = Garage::new(Kind::Biplane, Some(Plane::flying(spawn, flight))).unwrap();
    let mut lean = PilotLean::default();
    let gas = Report::from_buttons(Control::Cross.mask());
    for _ in 0..30 * 60 {
        pass(&mut sim, &mut garage, &mut lean, gas);
    }
    assert!(!garage.plane.as_ref().unwrap().on_ground(), "thirty seconds of gas and the plane is not up");
    // A centred stick sits the pilot in the middle of both axes, the even mix half-way through.
    let (phase, mix) = lean.weights();
    assert!((phase - 0.5).abs() < 1e-3 && (mix - 0.5).abs() < 1e-3, "a centred stick leans the pilot to {phase} {mix}");

    let stick = |x: u8, y: u8| {
        let mut r = gas;
        r.set_axis(2, x);
        r.set_axis(3, y);
        r
    };
    let held = |report: Report| {
        let (mut sim, mut garage, mut lean) = (sim.clone(), garage.clone(), lean);
        for _ in 0..60 {
            pass(&mut sim, &mut garage, &mut lean, report);
        }
        let pad = garage.plane.as_ref().unwrap().flight.as_ref().unwrap().pad;
        (lean.weights(), (pad.elevator.0, pad.aileron.0))
    };
    let mut pose = |weights: (f32, f32)| {
        biplane::pilot_palette(&mut body, &drawn.anchors, weights).expect("the rider's archive has the plane's clips")
    };
    let centred = pose(lean.weights());

    // The original's axes for a full stick: the aileron (left positive) puts the phase at
    // `0.5 * (1 - x)`, the elevator (up positive) the mix at `0.5 * (1 + y)` — a stick held a second
    // has both lags within a percent of their ends.
    let cases = [
        ("stick left", stick(0x00, AXIS_CENTRE), Some(0.0), None),
        ("stick right", stick(0xff, AXIS_CENTRE), Some(1.0), None),
        ("stick up", stick(AXIS_CENTRE, 0x00), None, Some(1.0)),
        ("stick down", stick(AXIS_CENTRE, 0xff), None, Some(0.0)),
    ];
    let mut leaned = Vec::new();
    for (name, report, phase_end, mix_end) in cases {
        let ((phase, mix), (elevator, aileron)) = held(report);
        let want = (phase_end.unwrap_or(0.5), mix_end.unwrap_or(0.5));
        assert!(
            (phase - want.0).abs() < 0.01 && (mix - want.1).abs() < 0.01,
            "{name}: the pilot leans to ({phase}, {mix}), the original's axes to {want:?} (pad elevator {elevator}, aileron {aileron})"
        );
        // The axes are the pad's: each end the stick's own sign puts it at.
        let axes = (0.5 * (1.0 - aileron), 0.5 * (1.0 + elevator));
        assert!((phase - axes.0).abs() < 0.01 && (mix - axes.1).abs() < 0.01, "{name}: off the pad's axes {axes:?}");
        let drawn = pose((phase, mix));
        let moved = apart(&drawn, &centred);
        assert!(moved > 0.05, "{name}: the drawn pilot moved {moved} ft from the centred stick's");
        leaned.push((name, drawn));
    }
    // Each way against the other: the two ends of each axis are two different pictures.
    for (a, b) in [(0, 1), (2, 3)] {
        let moved = apart(&leaned[a].1, &leaned[b].1);
        assert!(moved > 0.05, "{} and {} draw the same pilot ({moved} ft)", leaned[a].0, leaned[b].0);
    }

    // The rudder and the throttle do not reach the pilot: L1, R1 and the gas let go of leave it
    // where the centred stick has it.
    for (name, report) in [
        ("L1", Report::from_buttons(Control::Cross.mask() | Control::L1.mask())),
        ("R1", Report::from_buttons(Control::Cross.mask() | Control::R1.mask())),
        ("no gas", Report::from_buttons(0)),
    ] {
        let ((phase, mix), _) = held(report);
        assert!((phase - 0.5).abs() < 1e-3 && (mix - 0.5).abs() < 1e-3, "{name} leans the pilot to ({phase}, {mix})");
        let moved = apart(&pose((phase, mix)), &centred);
        assert!(moved < 1e-3, "{name}: the drawn pilot moved {moved} ft");
    }
}
