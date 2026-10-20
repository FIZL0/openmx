//! **openmx** — the playable shell: a window, a real pad, and the freeride area to ride it in.
//!
//! ```sh
//! cargo run --release -p openmx                    # ride
//! cargo run --release -p openmx -- --seconds 60    # ride for a minute and quit (the task's own test)
//! cargo run --release -p openmx -- --pad /dev/input/js1 --size 512x384 --scale 2
//! cargo run --release -p openmx -- --record context/tapes/spin.tape   # write the pad down
//! cargo run --release -p openmx -- --play context/tapes/spin.tape     # and ride it back
//! ```
//!
//! No emulator, no fixture, no menu. Everything it shows is read at runtime from the user's own disc —
//! the ISO named in `kit.toml`, the track's `.res`, its scene, its terrain, the models the scene places
//! and their textures — and nothing is written anywhere except the one frame `--shot` asks for, which
//! goes under `context/` and is not committed.
//!
//! # What is the port and what is the shell
//!
//! The shell is the thin part. Each piece of the frame below is a proved port, and this file only wires
//! them together in the order the game's own loop uses:
//!
//! * **The pad** is I1 and I2: the host's joystick and keyboard are folded into a DualShock 2 report
//!   ([`pad`]), and the report is read by [`game_sim::input::Manager`] — the game's own poll, dead zone
//!   and press/release edges.
//! * **The frame** is S1: [`game_sim::tick::Engine`] is handed a wall clock sampled the way the
//!   original samples EE Timer 0, and it hands the simulation the same thirty-frame averaged, 30 fps
//!   clamped `dt` the original does. The loop paces itself at the game's 60 Hz and degrades the way the
//!   original degrades when a frame runs long: the clock is real, so `dt` grows.
//! * **The place** is D6: the spawn, the start direction and the terrain come out of `free06`'s own
//!   `.scn` and `.ps2.trn`, through the ported scene reader and the ported height-and-surface query.
//! * **The ride** is P2: [`game_sim::ride::Ride`] is the bike, stepped by the ported ground step with
//!   the suspension, the tires, the rigid body, the air state and the collision sweep behind it, on the
//!   parameters the bike's own `.phy` carries. The ground it asks about and the props it runs into are
//!   [`game_sim::world::Place`] — D6's terrain query and S4's sweep over the scene's own collision
//!   placements. Nothing in this file moves the bike.
//! * **The rider** is P3: [`game_sim::rider::Rider`] puts S11's trick machine, S10's airborne wreck
//!   test and S5's ride state machine on top of that bike, in the order the original's ridden actor
//!   runs them. The 24 stunts a gesture can name are the disc's own, read out of `appstart.res`'s
//!   `stunts.ini` by the ported table reader, and the limits a landing is judged against come off the
//!   same `.phy` as the ride. Nothing in this file decides a trick or a wreck either.
//! * **The picture** is V1: the scene's props, the bike and the parts its `.sub` bolts on, lit and
//!   fogged by the scene's own values, through the port's rasteriser, with the follow camera S7 ported
//!   reading the track's own `cpresets.ini`.
//! * **The body on the bike** is V4, V5, V7 and V8: the rider's own archive, the clips it carries,
//!   and the skeleton they pose — driven by the one animation driver the simulation already runs
//!   inside the rider's frame ([`game_sim::rider::Rider::animation`]), so the stunt the bike is
//!   doing and the stunt the body is doing are the same selection. The body is skinned every frame
//!   and drawn at the bike's transform; the clips put it on the bike themselves.
//!
//! What is **not** a port, and is marked as such where it happens: the bilinear blend inside a terrain
//! cell ([`ground`]), the host-float arithmetic of the camera-to-rasteriser hand-off, and this file's
//! own choice of which pad control means what.
//!
//! # The controls
//!
//! Cross is the throttle and Square the brake (both analogue, as the pad reports them), L2 is the
//! clutch and R2 the front brake on its own, the left stick steers and leans, L3 asks the rider to
//! balance and R3 is reverse.
//!
//! A **trick** is a gesture: hold one, two or three of Circle, Triangle and R1 — which is what names
//! one of the six groups — and push the d-pad in one of four directions, which names one of the 24
//! stunts the rider carries. The d-pad still preloads, and a gesture only counts in the air. Hold the
//! buttons past the stunt's hold time and the rider holds the pose; let go and it runs to its end.
//! Land with one still running and the rider comes off.
//!
//! **L1 and R1 together** put the bike back — on the start line while riding, where the wreck came to
//! rest while down — on the press of whichever goes down second. That chord is the original's own (P20),
//! read off its binding table, and R1 is the third trick button there as well as here.
//! It is how a rider who is stuck gets going again. A wreck no longer needs it: the bike tumbles, the rider comes off it and
//! the ride comes back up on its own, on the wreck's own two files (S26). Start freezes and
//! unfreezes the ride, L1 cycles the track's own camera presets the way the original's next-camera routine
//! does (`--camera` picks the one to start on, by name), and Escape
//! or closing the window quits. The same controls are on the keyboard for a machine with no pad:
//! X/Space, Z, the arrow keys, C/V/D for the three trick buttons, A+D for the reset, Return.
//!
//! **The throttle is a pressure and not a switch**, because the game reads Cross from its pressure
//! byte: part throttle out of a corner is a fraction of that byte and the port has always read it as
//! one. What has to be arranged is a host that can *ask* for a fraction (P9, [`pad`]) — almost no pad
//! sold since the DualShock 2 reports a face button as anything but a bit. So: `--pad-pressure` says
//! which joystick axes a device publishes its buttons' own pressures on, where it publishes them;
//! `--throttle r2` rides gas on a trigger instead, which is travel every pad has, giving up that
//! trigger's own control for the session; and the keyboard's throttle key ramps open over a third of
//! a second rather than stepping, so a stab is part throttle on a machine with no pad at all.
//!
//! **H** draws the riding overlay ([`hud`]), which `--hud` starts on: the sixteen controls as the
//! port's own input manager holds them, both sticks where its dead zone put them, and beside them
//! the numbers the ride is running on. It is debugging furniture and matches nothing on the disc,
//! which is why it is behind a flag and why with the flag off it draws nothing at all.
//!
//! **M** swaps between the bike and the track's biplane, and `--vehicle biplane` starts in the plane
//! ([`vehicle`], P19): the swap is the original's own — the bike is left where it stands, hidden and
//! not stepped, and the plane is put back where the track's `machine.txt` parked it, with its own
//! camera file. With the plane up it flies (P31): its own simulation steps every frame on the seat's
//! pad, the model is drawn at its body's pose with the prop at the engine's angle, the plane's camera
//! follows it, and the overlay shows its throttle and airspeed. It flies over the terrain's
//! heightfield alone — the props are not in its world, because the original's contact search is not
//! ported — so it wrecks only off the map's edge, never on a hard landing or a tree.
//!
//! # Tapes
//!
//! `--record FILE` writes down the pad of every frame the simulation ran — the reports as the pad
//! library left them and the `dt` that frame was handed — and `--play FILE` rides one back instead of
//! reading the pad at all, at the tape's own frame lengths, and quits when it runs out. The file is
//! I3's [`game_sim::replay::PadTape`], so a tape of this shell, a tape read off a capture of the
//! original ([`game_sim::replay::PadTape::from_capture`]) and a tape of a human's PCSX2 recording
//! ([`game_sim::replay::PadTape::from_p2m2`]) are the same kind of file and all three ride here —
//! `--play` reads a `.p2m2` as the tape it is, so `--play a.p2m2 --phase second --record a.tape`
//! turns a human's minute on the original into a tape of this port's own kind by riding it.
//!
//! What it buys: a control complaint stops being a story. "It spins out after ten seconds" becomes a
//! file somebody else can ride, and a physics change can be tried against the same input twice. A
//! played session is a function of its tape — the pad and the frame length both come off it, and
//! nothing else in the frame reaches the simulation — so `--play t --record t2` writes `t2` identical
//! to `t`, and that is the check that the two halves agree.
//!
//! A tape holds values read out of the game, so it is game data: write it under `context/`, which is
//! git-ignored, and never into the repository.
//!
//! Which host button is which is this file's and [`game_sim::ride::Controls::from_pad`]'s own choice
//! and not the disc's — the original's binding table is in its front end and is not ported. What *is*
//! the disc's is the order of the three trick buttons, because that order is what numbers the six
//! groups and so which stunt a gesture names.

#![forbid(unsafe_code)]

mod chase;
mod ground;
mod hud;
mod moment;
mod pad;
mod vehicle;
mod window;

use std::sync::Arc;
use std::time::{Duration, Instant};

use game_render::biplane::{self, Biplane};
use game_render::chassis::{grips as chassis_grips, placed as chassis_at};
use game_render::disc::{Disc, Res};
use game_render::mat::{M4, V3, v3};
use game_render::raster::Frame;
use game_render::rider::{Body, Sex, Skeleton, anchors_of};
use game_render::scene::Vehicle;
use game_render::vehicle::{Chassis, Drive, params as vehicle_params};
use game_render::world::{Assets, Camera, Lighting, draw_scene, vertical_fov};
use game_render::{ini, png, scene as view};
use game_sim::biplane::Area;
use game_sim::camera::{Preset, Race, Switches};
use game_sim::flight::Flight;
use game_sim::input::{Control, PADS, Report};
use game_sim::phy::{self, Phy};
use game_sim::replay::{Head, PadTape, Recorder, TickPhase};
use game_sim::ride::{self, Controls};
use game_sim::rider::{self, Rider};
use game_sim::rng::Rng;
use game_sim::state::Phase;
use game_sim::tick::{Engine, Game, Outcome, Timer0};
use game_sim::trick::{Set, Stunts};
use game_sim::world::Place;
use game_sim::{Input, Sim, track, world as sim_world};
use ps2_float::{F, Vf};

use ground::Ground;
use hud::Hud;
use moment::{Moment, Shot};
use pad::{Pads, Throttle, Wiring};
use vehicle::{Garage, Kind, Plane, Spawn};
use window::{Window, fit_frame, key};

/// The game's own frame rate. The simulation is not fixed to it — it is handed the wall clock's own
/// delta — but this is the rate the loop aims at, and the rate the original's video mode flips at.
const FRAME: Duration = Duration::from_nanos(1_000_000_000 / 60);
/// The PS2's own frame, which `--size 640x448` asks for and which a drawing with no window to fit
/// (`--moment`) is made at.
const PS2_FRAME: (usize, usize) = (640, 448);

/// How far around the rider the ground is drawn, in terrain samples.
///
/// The scene's fog is the real limit and the draw culls against it, but `free06`'s fog is further away
/// than a software rasteriser can fill at 60 Hz, so the dirt fades out at a radius instead. Nine feet
/// a sample makes this about a hundred and twenty yards of ground in every direction.
const GROUND_RADIUS: u32 = 44;

/// The near clip plane the frame is drawn with, in feet, and the number the follow camera is handed
/// as its distance floor, because the original's call site hands its camera the near plane of the
/// viewport it is drawing into — one field, read by the projection and by the camera alike.
///
/// This is the disc's own number: the engine's viewport constructor puts a foot there and the race's
/// viewport setup overwrites it with 2.4 for all three of its layouts, full screen and both splits.
/// So the shell hands the camera 2.4 and draws with 2.4, which is what the original does.
///
/// It is also the one number in the picture that the camera class the disc actually creates reads
/// differently from the one this port has: that class multiplies it by four and a half before the
/// `max`, putting its follow distance on a floor of 10.8 feet, where the ported frame's floor is the
/// preset's own minimum. The frame it uses is a different function altogether; S23's journal entry
/// says what else differs and S24 is the task for porting it.
const NEAR_CLIP: f32 = 2.4;

/// The camera preset a session starts on. The original's own, in a freeride: the human's freeride
/// save state has the follow camera on preset 0 of 11, whose record is named `Race Cam 1` — the
/// race's camera and not `Freestyle Cam` (P14). L1 walks on from it.
const START_CAMERA: &str = "Race Cam 1";

/// The camera preset a session in the biplane starts on: the first record of
/// `AppStart\BiplaneCPresets.ini`, which is the plane's own camera file (R8). Which of the five the
/// original is on in the air was not read off a save state; the first is the one a camera object
/// applies to itself when it is built, which is how the bike's `Race Cam 1` was found (P14).
const PLANE_CAMERA: &str = "Flight Cam 1";
/// The plane's camera file. The game names it `AppStart\BiplaneCPresets.ini`, but the disc ships it
/// in each freeride track's second archive (`free06_2.res`'s `appstart\biplanecpresets.ini`), not in
/// `appstart.res`.
const PLANE_PRESETS: &str = "biplanecpresets.ini";
/// The archive the vehicle loader reads the machines' own files out of, `biplane.phy` among them.
const MACHINE_ARCHIVE: &str = "machine.res";

/// The race camera's reverse threshold: the constant its setup hands the base camera's initialiser.
const REVERSE_SPEED: f32 = 5.0;

/// The speed the camera's look-ahead is measured against, as the original's camera holds it during a
/// freeride: 80 mph in feet a second. Nothing in the camera's own code was found writing it (P14).
const SPEED_REFERENCE: f32 = 117.332_8;

/// The chassis model every bike is built on, which is the name the parts in a `.sub` hang off.
const CHASSIS: &str = "bike";
/// What the vehicle's own clip loader puts in front of a name: the actor's word, which its
/// `stunt_name` does *not* bake in the way the rider's does
/// (`game_sim::vehicle_anim::Library::stunt_name`). This is what turns the stunt table's `nacnac`
/// into `bike_nacnac` and the machine's `air_Mid` into the `.mcf`'s `bike_air_Mid`.
const BIKE_PREFIX: &str = "bike_";

/// Where the rider's stunt table lives: one member of the app's own archive, the same two names the
/// trick task's proofs read it by.
const STUNT_ARCHIVE: &str = "appstart.res";
const STUNT_TABLE: &str = "stunts.ini";

fn main() {
    if let Err(e) = run() {
        eprintln!("openmx: {e}");
        std::process::exit(1);
    }
}

/// Everything the frame needs that is not the renderer's: the ride, the pad and the port's own input
/// manager.
///
/// This is what [`Game`] is implemented on, so the only thing that happens inside a simulation pass is
/// what the game would do in one — reading the pad and moving the world.
struct World<'a> {
    /// What the ride asks about the world: the ported terrain query and the scene's collision
    /// placements, swept by S4's own sweep. The *drawn* ground is the loop's own [`Ground`], which
    /// reads the same file for triangles instead.
    place: Place<'a>,
    /// This frame's reports, written by the app layer before the step the way the original's device
    /// manager fills its buffers before the frame reads them.
    reports: [Report; PADS],
    /// Everything a frame of the simulation reads and writes that is not the world: the port's input
    /// manager, the bike's `.phy`, the tables the step reads, and the rider on the bike. This is the
    /// same value `game_sim::replay::verify` drives, so a tape of this shell and a replay fixture
    /// are a frame of the same thing (P5).
    sim: Sim,
    /// Which machine has the controller, and the track's biplane beside the bike (P19). With the bike
    /// up a pass is `sim.step` and nothing else; with the plane up the bike is not stepped.
    garage: Garage,
    /// Whether the ride is moving. Start toggles it: the original's pause is a whole front-end page
    /// and this shell has no pages, so freezing the ride is the shell's own stand-in for it.
    riding: bool,
    /// What the rider did this frame, kept so the loop can print it after the pass.
    said: rider::Frame,
    /// The hands this pass was stepped with, kept for the body's own animation frame — which runs
    /// after the pass, off the same controls and the same `dt` (V18).
    hands: Controls,
    /// `--record`: the tape this session is being written to, one frame per simulation pass.
    tape: Option<Recorder>,
    /// `--play`: the frame length the tape says this pass is, which replaces the one the engine
    /// measured. The recorded session simulated with the tape's own `dt`, so a playback that used the
    /// wall clock would reproduce the buttons and not the ride.
    played_dt: Option<F>,
}

impl Game for World<'_> {
    /// One simulation pass: the pad, then the bike.
    ///
    /// `input` is the engine's own sampled pad, which this shell does not use — the full 32-byte
    /// reports are in `self.reports` and the port's manager is what reads them. Nothing here draws
    /// from `rng`: the only thing that moves is the bike, and the rider decides where it goes.
    fn simulate(&mut self, dt: F, _input: Input, _rng: &mut Rng) -> bool {
        // Riding a tape: the frame is as long as the tape says it was, not as long as this machine
        // took to draw the last one. Everything below reads this `dt` and nothing reads the engine's.
        let dt = self.played_dt.unwrap_or(dt);
        // Writing one: the pass's own `dt` and the reports it is about to read, which is what a
        // replay of this tape will be handed (`game_sim::replay::Recorder`).
        if let Some(tape) = &mut self.tape {
            tape.frame(dt, &self.reports);
        }

        // The game's poll, with the frame's dt: this is what turns report bytes into held controls,
        // press and release edges, and stick axes with the game's own dead zone — and then the
        // rider's hands off it, because which button means what lives in the ride and not here.
        let controls = self.sim.poll(dt, &self.reports);

        if self.sim.input.pad(0).pressed(Control::Start) {
            self.riding = !self.riding;
        }

        self.said = rider::Frame::default();
        if self.riding {
            self.said = self.garage.step(&mut self.sim, dt, &controls, &mut self.place);
        }
        self.hands = controls;
        true
    }
}

/// What the shell was asked to do.
struct Args {
    track: String,
    bike: String,
    /// Which rider archive to put on the bike, or `none` to ride an empty bike. Nothing on the disc
    /// names a rider — the choice is a menu's, and this shell has no menus.
    rider: String,
    /// The frame's size, from `--size`: the PS2-native 640x448 path and the tests. `None` draws at
    /// the window's own pixels — a window the size of the screen, and a frame that follows it.
    size: Option<(usize, usize)>,
    scale: usize,
    /// Quit after this long, which is how the acceptance run is made to end by itself.
    seconds: Option<f32>,
    pad: Option<String>,
    display: String,
    /// Write the first frame here and keep riding — how an unattended run proves what it drew.
    shot: Option<std::path::PathBuf>,
    /// Which frame `--shot` writes. The first frame of a ride is the one frame where nothing has
    /// happened yet — the bike is on the start line, every number is zero and the rider is still in
    /// its bind pose (V8) — so a shot meant to show the ride, or the overlay with anything in it,
    /// has to be taken a little later.
    shot_frame: u32,
    /// Draw the riding overlay (P6): the pad as the simulation reads it and the numbers beside it.
    /// Off by default and off is free, because it is debugging furniture and not part of the game.
    hud: bool,
    /// `--camera NAME`: which of the track's camera presets to start on, by the name its
    /// `cpresets.ini` gives it. The default is the one the original itself is on in a freeride (P14).
    camera: String,
    /// Draw one frame posed from a moment of the original (V2) instead of riding: the moment file
    /// and the path to write that frame to.
    moment: Option<(std::path::PathBuf, std::path::PathBuf)>,
    /// Whether the session wrecks at all ([`game_sim::state::Sense::wrecks_enabled`]), which is the
    /// original's own per-session flag. On by default, because that is what a freestyle session is.
    ///
    /// `--no-wrecks` was once the only way to ride for a minute, because the ported ride's g loads
    /// oscillate up past the bike's own `SideWreckSpeedLimit` within a few seconds of the start line
    /// and a wreck stayed down for good (P3's journal entry). Since S26 a wreck gets up by itself —
    /// the bike tumbles on `wreckbike.phy`, the rider is thrown and rolls on the rider file's
    /// `[Wrecks]`, and the ride is put back where the wreck came to rest — so the flag is now what
    /// it says it is: a way to ride without being judged, not a way to ride at all. The g loads are
    /// still a task of their own.
    wrecks: bool,
    /// `--trace N`: ride N frames with nobody touching the pad and print one line per frame of what
    /// the solver is doing, then quit without opening a window. S22's measuring stick for the ride's
    /// g loads — the one thing the shell could show about them before was that it had wrecked.
    trace: Option<u32>,
    /// `--trace-dt S`: the frame length `--trace` steps at, when it should not be the original's own
    /// 30 fps clamp — the one knob that tells a solver ringing at the frame rate from one ringing
    /// however small the step is.
    trace_dt: Option<f32>,
    /// `--trace-brake`: hold both brakes through the trace, which is the brief's own case — a bike
    /// standing still on the start line rather than one free-rolling down the first slope.
    trace_brake: bool,
    /// `--trace-gas`: pin the throttle through the trace, which is P8's own case — full throttle
    /// from a standstill on the start line.
    trace_gas: bool,
    /// `--trace-stick X`: hold the left stick over at `x` (-1..1) through the trace — P15's turn at a
    /// held speed and a held stick.
    trace_stick: Option<f32>,
    /// `--trace-feel`: one short line a frame instead of the solver's columns — speed, velocity,
    /// position, heading, how upright, on the ground, wrecked — in the columns P15 measured the
    /// original's captures by, so both rides go through one script.
    trace_feel: bool,
    /// `--trace-air`: `--trace-feel`'s rides, but the line is the chassis' attitude — yaw, pitch and
    /// roll off the transform in degrees, the body's own angular velocity, which wheels touch, the
    /// crossup and the stick the bike is holding. P29's measuring stick for a bike that twists in the
    /// air: one line a frame from the run-up, over the lip, to the landing.
    trace_air: bool,
    /// `--phy NAME`: ride the bike archive's member ending in `NAME` (`pro125a.phy`) instead of the
    /// freeride's own `Bike\Race125P.phy`.
    phy: Option<String>,
    /// `--trace-at X,Z,DEG`: put the bike on the ground at `x, z` facing `DEG` instead of on the start
    /// line.
    trace_at: Option<(f32, f32, f32)>,
    /// `--trace-switch N`: under `--trace-feel`, ride the first N frames on the throttle alone and only
    /// then take the controls the other flags ask for — a full brake from top speed, or a turn at speed.
    trace_switch: Option<u32>,
    /// `--trace-stick-y F:Y,F:Y,...`: under `--trace-feel`, the left stick's y from frame `F` on
    /// (positive pulled back), each entry until the next — P30's pump and flip off a jump, ridden
    /// on the shell's own frame. The line then carries the gesture's columns too.
    trace_stick_y: Vec<(u32, f32)>,
    /// `--record FILE`: write every simulated frame's pad to a tape, and the tape to this file when
    /// the session ends. A tape holds values read out of the game, so the file belongs under
    /// `context/`.
    record: Option<std::path::PathBuf>,
    /// `--play FILE`: ride this tape instead of the pad, and quit when it runs out. A `.p2m2` is a
    /// human's PCSX2 recording of the original and is read as the tape it is.
    play: Option<std::path::PathBuf>,
    /// `--phase first|second`: which of a tick's two video fields the game's pad read landed on, for
    /// a `.p2m2`. The file cannot say, and it is a fact about the recording rather than about the
    /// game, so there is nothing better than asking.
    phase: TickPhase,
    /// `--throttle` and `--pad-pressure`: how the host's pad is wired to the throttle (P9). Both are
    /// facts about the device in somebody's hands — whether its face buttons travel, and which axis
    /// a driver publishes each button's pressure on — so neither can be guessed from here.
    wiring: Wiring,
    /// `--vehicle bike|biplane`: which machine the session starts on (P19). The bike by default,
    /// and a default session is the shell exactly as it was before there was a choice.
    vehicle: Kind,
}

fn run() -> Result<(), String> {
    let Some(args) = parse_args()? else {
        return Ok(());
    };

    // ---- the disc -------------------------------------------------------------------------------
    let mut disc = Disc::open_configured()?;
    let track_member = member_ending(&disc, &format!("{}.res", args.track))?;
    let bike_member = member_ending(&disc, &format!("{}.res", args.bike))?;
    // A track is two archives: the props and the terrain in one, the bike's animation set, its
    // physics and the follow camera in the other, which the game loads alongside it.
    let second = member_ending(&disc, &format!("{}_2.res", args.track)).ok();
    // The rider is a fourth archive, named after the rider rather than the track or the bike: the
    // body, its gear and its motions are all in there.
    let rider_member = match args.rider.as_str() {
        "none" => None,
        name => Some(member_ending(&disc, &format!("{name}.res"))?),
    };
    let mut archives: Vec<Res> = vec![disc.res(&track_member)?, disc.res(&bike_member)?];
    if let Some(name) = &second {
        archives.push(disc.res(name)?);
    }
    if let Some(name) = &rider_member {
        archives.push(disc.res(name)?);
    }
    let archives = archives;

    // The rider's stunts, which are neither the track's nor the bike's: one table in the app's own
    // archive, read by the ported reader (S11). It holds 57 records and a 24-slot gesture map for
    // each player, and the 24 the first player's map reaches are what this rider can do.
    let stunt_bytes = member_ending(&disc, STUNT_ARCHIVE)
        .and_then(|name| disc.res(&name))
        .and_then(|res| res.read_suffix(STUNT_TABLE).map_err(|e| format!("{STUNT_ARCHIVE}: {e}")))?;
    let table = Stunts::parse(&stunt_bytes)?;
    let stunts = Set::new(table.selection(0));
    println!(
        "stunts: {} on the disc, {} this rider can do",
        table.stunts.len(),
        stunts.stunts.iter().filter(|s| !s.name.is_empty()).count()
    );

    let scn = archives[0]
        .entries()
        .iter()
        .find(|e| e.name.to_ascii_lowercase().ends_with(".scn"))
        .ok_or("the track archive has no .scn")?
        .name
        .clone();
    let scn_bytes = archives[0].read(&scn)?;
    // The same file read twice, by two readers that want different things out of it: the simulation's
    // reader for the spawn and the terrain's layout, the renderer's for the props, lights and fog.
    let place = track::Scene::parse(&scn_bytes)?;
    let look = view::Scene::parse(&ini::text(&scn_bytes));

    let terrain_bytes = terrain_bytes(&archives[0], &place.terrain_file)?;
    // The same file parsed twice, because the two sides of it are owned by different crates: the
    // shell's `Ground` draws it in host floats, the port's answers the ride's ray in the game's.
    let ground = Ground::new(track::Terrain::parse(&terrain_bytes)?, &place.terrain);
    let sim_terrain = track::Terrain::parse(&terrain_bytes)?;
    let (extent_x, extent_z) = ground.extent();
    println!(
        "{}: {} props, {} collision shapes, ground {:.0} x {:.0} feet in {} x {} samples",
        place.name,
        place.models.len(),
        place.collision.len(),
        extent_x,
        extent_z,
        ground.samples_x,
        ground.samples_z
    );

    // Everything solid the scene stands on the ground, in its own space with its own place in the
    // world: this is what stops the rider at a fence instead of letting them through it.
    let (props, missed) = sim_world::Props::load(&place, |name| {
        archives.iter().find_map(|a| a.read_suffix(name).ok()).ok_or_else(|| "no such member".into())
    });
    println!(
        "solid: {} of {} collision meshes{}",
        props.props.len(),
        place.collision.len(),
        if missed.is_empty() { String::new() } else { format!(" ({})", missed.len()) }
    );
    for line in missed.iter().take(3) {
        println!("  {line}");
    }

    // ---- what to draw -------------------------------------------------------------------------
    let mut assets = Assets::default();
    // The bike's own build: which parts bolt onto the shared chassis, and what this one is painted
    // with. Without the repaint half its meshes ask for a material no archive has.
    let vehicle = match archives[1].read_suffix(&format!("{}.sub", args.bike)) {
        Ok(bytes) => Vehicle::parse(&ini::text(&bytes)),
        Err(e) => {
            println!("bike: {e}");
            Vehicle::default()
        }
    };
    assets.rename_materials(&vehicle.materials);

    let mut names: Vec<String> = look.models.iter().map(|p| p.model.clone()).collect();
    names.sort();
    names.dedup();
    for name in &names {
        assets.load(&archives, name);
    }
    let has_bike = assets.load(&archives, CHASSIS);
    for part in &vehicle.parts {
        assets.load(&archives, &part.model);
    }
    println!(
        "loaded: {} of {} models, {} textures{}",
        assets.models.len(),
        names.len(),
        assets.textures.len(),
        if has_bike { ", bike" } else { ", no bike" }
    );

    // ---- the bike and where the rider starts ---------------------------------------------------
    // The two tables the simulation shares: the arc cosine curve and the game's {cos, sin} table.
    // They are built once, read by the ride, the camera and the `.phy` loader, and never written.
    let tables = rider::Tables::build();

    // The bike's own physics, off the disc, by the ported loader (S18) — every number the ride runs
    // on comes from here, and a bike whose file is missing is not rideable.
    // By default that is the file the original's vehicle loader asks for on a freeride boot,
    // `Bike\Race125P.phy` (D7: class 125, Race tier, the `P` of the pair), whichever bike model is
    // drawn — the loader builds the name from the event, not from the model. Every 125 archive
    // carries it; the disc's first `.phy` member, which the shell used to fall back to, is
    // `pro125a.phy` (P25). Another bike's archive without it falls back to the model's own name and
    // then to the first `.phy`, and says so.
    // `--phy` names another of the archive's members instead: every bike archive carries the whole
    // set of its class (P15).
    let (phy_bytes, phy_member) = match &args.phy {
        Some(name) => {
            let e = archives[1]
                .find_suffix(name)
                .ok_or_else(|| format!("the bike archive has no member ending in {name}"))?;
            (archives[1].read_suffix(name)?, e.name.clone())
        }
        None => {
            let freeride =
                track::physics_file(track::FREERIDE_ENGINE, track::FREERIDE_PRO, track::Tuning::P);
            let wanted = [freeride.clone(), format!("{}.phy", args.bike), ".phy".to_string()];
            let (want, e) = wanted
                .iter()
                .find_map(|w| archives[1].find_suffix(w).map(|e| (w, e)))
                .ok_or("the bike archive has no .phy")?;
            if *want != freeride {
                println!("phy: the bike archive has no {freeride}; riding {} instead", e.name);
            }
            (archives[1].read_suffix(want)?, e.name.clone())
        }
    };
    let phy = Phy::read_bytes(&phy_bytes, &tables.ride.curve);

    // The crash, which is two more files (S26). The bike the rider leaves behind is a rigid body of
    // its own — `wreckbike.phy`, shipped beside every bike's own file and read by nothing before now
    // — and how the rider comes off it, how high they flail and how long they lie there is in the
    // rider's file, under `[Wrecks]`. Neither is named after the bike or the rider: there is one
    // wreck bike and one `mrider.phy` on the disc, which is why both are looked for by suffix across
    // every archive the session opened. A session that finds neither simply stays down until the
    // reset chord, which is where this port was before S26.
    let wreckage = archives.iter().find_map(|a| a.read_suffix("wreckbike.phy").ok()).zip(
        archives.iter().find_map(|a| a.read_suffix("mrider.phy").ok()),
    );
    match &wreckage {
        Some((bike, rider)) => println!(
            "wrecks: {} bytes of wreck bike, {} bytes of rider wreck set",
            bike.len(),
            rider.len()
        ),
        None => println!("wrecks: no wreckbike.phy and mrider.phy pair — a wreck will not get up"),
    }

    // ---- the rider on it ------------------------------------------------------------------------
    // The body, the gear it is painted in, and the clips the animation driving plays on it (V4, V5,
    // V7, V8). The archive is opened a second time for the clips: the animation set reads members
    // out of its own copy as the simulation asks for them, where `archives` is what the models and
    // textures are loaded from. The blend time the cross-fade falls at is the bike's, off its
    // `.phy` — the same `Params` the simulation's own driver runs on.
    let mut body = match &rider_member {
        None => None,
        Some(member) => {
            let own = vec![disc.res(member)?];
            match Body::load(own, &args.rider, Sex::Male, "", &table.stunts, phy.motion()) {
                Ok(body) => Some(body),
                Err(e) => {
                    println!("rider: {e}");
                    None
                }
            }
        }
    };
    if let Some(body) = &body {
        // The gear is a repaint of the one shared body, exactly as a bike's `.sub` repaints the one
        // shared chassis, so the renames go in before the geometry is loaded.
        assets.rename_materials(&body.rider.suit.materials);
        let drawn = assets.load(&archives, body.geometry());
        let bound = body.rig.set.clips.len();
        let slots = body.rig.lib.stunt.iter().filter(|c| c.is_some()).count();
        println!(
            "rider: {} wearing {} -> {} -> {}.psg, {bound} clips bound, \
             {slots} of {} gesture slots, {} airborne{}",
            args.rider,
            body.rider.mcf,
            body.rider.suit.mcf,
            body.geometry(),
            game_sim::trick::SLOTS,
            [body.rig.lib.air_fwd, body.rig.lib.air_mid, body.rig.lib.air_bck]
                .iter()
                .filter(|c| c.is_some())
                .count(),
            if drawn { "" } else { ", no body to draw" }
        );
        // And the table the body is actually posed out of (V18): the sixty-state machine's, which is
        // the rider's own. Sixty of sixty on a rider's set and twelve on a bike's, which is how a set
        // paired with the wrong machine shows up before anything is drawn.
        let gestures = body.pose_lib.clip[..game_sim::trick::SLOTS].iter().filter(|c| c.is_some());
        let poses = body.pose_lib.clip[game_sim::trick::SLOTS..].iter().filter(|c| c.is_some());
        println!(
            "rider: {} of {} gesture slots and {} of {} pose clips for its own machine",
            gestures.count(),
            game_sim::trick::SLOTS,
            poses.count(),
            game_sim::vehicle_anim::POSE_CLIPS.len(),
        );
        for m in body.rig.set.missing.iter().take(3) {
            println!("  {m:?}");
        }
    }

    // ---- the bike that moves (V9) ---------------------------------------------------------------
    // The chassis' own animation: the `.sub`'s `[Objects] MCF`, the clips the sixty-state machine
    // names, and the chassis skeleton they are bound to. The archive is opened a second time for the
    // same reason the rider's is — the set reads members as the machine asks for them.
    //
    // Without this the bike is a statue: every part sat at its node's *bind* transform, read once per
    // frame off the model the archive shipped, so nothing on it moved. With it the forks compress,
    // the wheels turn, the front end steers and the swingarm follows the rear axle, all from the
    // simulation's own numbers, and the chassis skeleton is posed from the vehicle's own driver.
    let mut chassis = match vehicle.mcf.is_empty() {
        true => None,
        false => {
            let own = vec![disc.res(&bike_member)?];
            match Chassis::load(own, &vehicle.mcf, BIKE_PREFIX, &table.stunts, vehicle_params()) {
                Ok(chassis) => Some(chassis),
                Err(e) => {
                    println!("chassis: {e}");
                    None
                }
            }
        }
    };
    if let Some(chassis) = &chassis {
        let slots = chassis.stance.lib.clip[..game_sim::trick::SLOTS].iter().filter(|c| c.is_some()).count();
        let poses = chassis.stance.lib.clip[game_sim::trick::SLOTS..].iter().filter(|c| c.is_some()).count();
        println!(
            "chassis: {} -> {}.psg, {} clips bound, {slots} of {} gesture slots, {poses} of {}              pose clips, nodes {:?}",
            vehicle.mcf,
            chassis.geometry,
            chassis.stance.set.clips.len(),
            game_sim::trick::SLOTS,
            game_sim::vehicle_anim::POSE_CLIPS.len(),
            chassis.nodes,
        );
    }

    // The follow cameras the track carries, in the file's own order, read by S7's own reader. L1
    // walks the ones the file lets the player cycle to. `reverse_speed` is a constructor argument in the original and the call site was
    // not read; the preset's own switch speed is the closest number the disc has.
    let presets: Vec<Preset> = archives
        .iter()
        .find_map(|a| a.read_suffix("cpresets.ini").ok())
        .map(|b| Preset::read_bytes(&b, ""))
        .unwrap_or_default();
    // The reverse threshold is the race camera's setup's own constant: it hands the base camera's
    // initialiser 5, and the original's camera holds 5 in both freeride save states read (P14).
    // The preset's `ForwardReverseSwitchSpeed` was the stand-in before, and free06's file does not
    // have that key, so it was zero.
    let reverse_speed = F(REVERSE_SPEED);
    // The derived class, not the base one: the base camera's frame is dead code on the disc (S24),
    // and this is the frame the picture comes out of.
    let mut camera = Race::new(presets, reverse_speed);
    // The preset the session starts on, by name rather than by its place in the file. A name the
    // file does not have is an error rather than a quiet fall back to the first record, so that a
    // typo cannot pass for the camera it named.
    if !camera.cam.presets.is_empty() {
        let Some(start) = Preset::find(&camera.cam.presets, &args.camera) else {
            let names: Vec<&str> = camera.cam.presets.iter().map(|p| p.name.as_str()).collect();
            return Err(format!("--camera: the track has no preset called {:?}; it has {names:?}", args.camera));
        };
        camera.cam.set_preset(start);
    }
    // The first frame starts the eight lags where the camera actually is instead of lagging them in
    // from zero, which is what the camera's own snap flag is for. The disc gets it from the race's
    // setup applying a preset over a previous one; the constructor here applies its first preset to
    // itself, and that comparison cannot fire.
    camera.cam.snap = true;
    // What the look-ahead divides the rider's speed by. The constructor's 1.0 is not what the
    // original rides with: both of its cameras hold 117.3328 (80 mph in feet a second) in both
    // freeride save states read, and at 1.0 the race camera — which, unlike the base one, does not
    // cap the ratio — aims hundreds of feet up the track and loses the bike off the screen (P14).
    camera.cam.speed_reference = F(SPEED_REFERENCE);

    // ---- the biplane beside the bike (P19) -----------------------------------------------------
    // Where the track parks it: its own `machine.txt`, `[Biplane1]` for the first player, which is
    // what the original's machine placement reads when that player's machine is the plane (R8). The
    // file is the machines' archive's, `tracks\<track>\machine.txt`, beside the plane's own model
    // and physics. A track with no such section has no plane, and `--vehicle biplane` on it is an
    // error.
    let machine_res = match member_ending(&disc, MACHINE_ARCHIVE) {
        Ok(name) => Some(disc.res(&name)?),
        Err(_) => None,
    };
    let machines = machine_res.as_ref().and_then(|res| {
        let e = res.find_suffix(&format!("{}\\machine.txt", args.track))?;
        Some((e.name.clone(), res.read(&e.name).ok()?))
    });
    // And its flight (P31): the plane's own `biplane.phy`, from the same archive, built the way the
    // original builds it. Without the file the plane is parked and stays parked. The flyable area is
    // the terrain's extent, which only the map-edge throw reads; the pairing of the layout's four
    // fields with the area's is S37's reading, not checked against this track.
    let plane_phy = machine_res.as_ref().and_then(|res| {
        let e = res.find_suffix(phy::BIPLANE_PHY)?;
        res.read(&e.name).ok()
    });
    let plane = machines.as_ref().and_then(|(_, b)| Spawn::biplane(b, 0)).map(|spawn| match &plane_phy {
        Some(bytes) => {
            let mut flight = Flight::new(&phy::Plane::read_bytes(bytes));
            let t = &place.terrain;
            flight.area = Some(Area::new(F(t.width as f32), F(t.breadth as f32), t.width_scale, t.breadth_scale));
            Plane::flying(spawn, flight)
        }
        None => Plane::parked(spawn),
    });
    if machines.is_some() {
        println!("biplane: {}", if plane_phy.is_some() { "flies on its biplane.phy" } else { "no biplane.phy, parked" });
    }
    match (&machines, &plane) {
        (Some((name, _)), Some(p)) => println!(
            "biplane: parked by {name} at {:.0}, {:.0}, {:.0} facing {:.0} degrees",
            p.at.x(),
            p.at.y(),
            p.at.z(),
            p.facing.x().atan2(p.facing.z()).to_degrees()
        ),
        (Some((name, _)), None) => println!("biplane: {name} parks none"),
        (None, _) => println!("biplane: the track has no machine.txt"),
    }
    let garage = Garage::new(args.vehicle, plane)?;
    // The plane's own camera, which the swap hands the player with it: one camera-preset object per
    // machine kind, and the plane's reads `AppStart\BiplaneCPresets.ini` rather than the track's
    // bike presets (R8). The same derived camera and the same two constants the bike's has, because
    // the plane's camera is built by the same constructor; that the original holds the same speed
    // reference for it was not read.
    let mut plane_camera = match &garage.plane {
        None => None,
        Some(_) => {
            let presets: Vec<Preset> = archives
                .iter()
                .find_map(|a| a.read_suffix(PLANE_PRESETS).ok())
                .map(|b| Preset::read_bytes(&b, ""))
                .unwrap_or_default();
            println!("biplane: {} camera presets in {PLANE_PRESETS}", presets.len());
            let mut cam = Race::new(presets, reverse_speed);
            if let Some(start) = Preset::find(&cam.cam.presets, PLANE_CAMERA) {
                cam.cam.set_preset(start);
            }
            cam.cam.snap = true;
            cam.cam.speed_reference = F(SPEED_REFERENCE);
            Some(cam)
        }
    };
    // Its model and both prop cones, out of the machines' archive, where its textures are too (V21).
    let plane_drawn = match (&garage.plane, machine_res) {
        (Some(_), Some(res)) => {
            let own = vec![res];
            let drawn = Biplane::load(&mut assets, &own, 0);
            println!("biplane: {}", if drawn.is_some() { "model loaded" } else { "no model to draw" });
            drawn
        }
        _ => None,
    };

    // The scene's own start line and start direction, as the simulation's reader read them.
    let spawn = point(place.start);
    let facing = point(place.start_direction).norm();
    let at = Vf::new(spawn.x, spawn.y, spawn.z, 1.0);
    let heading = Vf::new(facing.x, facing.y, facing.z, 0.0);
    let mut sim = Place {
        ground: sim_world::Ground::new(&sim_terrain, &place.terrain),
        props,
    };
    // Where the wheels hang off the chassis, which is where each suspension ray starts: the **bike
    // model's own wheel nodes**. `bike_attach_nodes` looks "TireFront" and "TireRear" up in the bike's
    // node by name and writes each one's place, in that node's frame, straight into the wheel's
    // `+0xa0`; "Shocks" and "Handlebars" go the same way into the front wheel's steering axis
    // (S22, `research/oracle.txt`). `BodyTireForcePosition` is a different field of the `.phy`: it
    // lands at the wheel's `+0xb0`, which neither the suspension nor the tire step ever reads. The
    // original has no fallback here — a bike always has a model — so the one below is the port's own,
    // for a `.phy` whose model has no such node.
    // `--trace-at`: start somewhere else on the ground, facing a heading in degrees the way a capture's
    // `atan2(m20, m22)` reads it — P15's flat stretch, which the start line is not.
    let (at, heading) = match args.trace_at {
        None => (at, heading),
        Some((x, z, deg)) => {
            let (on, _) = sim_world::World::ground(&mut sim, Vf::new(x, 1.0e4, z, 1.0));
            let h = deg.to_radians();
            (Vf::new(x, on.y(), z, 1.0), Vf::new(h.sin(), 0.0, h.cos(), 0.0))
        }
    };
    let node_at = |name: &str| -> Option<Vf> {
        let chassis = assets.models.get(CHASSIS)?;
        let i = chassis.model.nodes.iter().position(|n| n.name == name)?;
        let w = chassis.model.world(i);
        Some(Vf::new(w.0[3][0], w.0[3][1], w.0[3][2], 1.0))
    };
    let mounts = (
        node_at("TireFront").unwrap_or(phy.front.body_tire_force_position),
        node_at("TireRear").unwrap_or(phy.rear.body_tire_force_position),
    );
    let mut rider = Rider::new(&phy, mounts, at, heading, stunts);
    // The fork the front wheel steers about, from the model's shocks to its handlebars, which is the
    // axis the bike's loader gives the front wheel (P33). Left straight up with no such nodes.
    if let (Some(h), Some(s)) = (node_at("Handlebars"), node_at("Shocks")) {
        rider.ride.front.steer_axis = game_sim::tire::steering_axis(h, s);
    }
    rider.sense.wrecks_enabled = args.wrecks;
    // The eight numbers the body's *step* reads, which `Rider::new` leaves at zero and says a shell
    // is expected to fill: the five fade rates, the stunt cross-fade's cap and the pose grid's three
    // speed thresholds, as V12 measured them off the body's own object in the running original. With
    // them at zero every fade is a hard cut and the grid's rows all meet at zero, so a grounded body
    // takes the top row at walking pace (V18).
    rider.body_rates = vehicle_params();
    // The wreck's own parameters, if the disc had them: the part the bike becomes, the rider's
    // throw-and-roll set, and the tumbling body's inertia tensor, which is neither file's — it comes
    // off the wreck model's own box ([`ride::WRECK_INERTIA`]).
    if let Some((bike, body)) = &wreckage {
        rider.carries_wreck(
            phy::Part::read_bytes(bike, "PhysicsPart1"),
            phy::Wrecks::read_bytes(body),
            ride::WRECK_INERTIA,
        );
    }
    println!(
        "start: {:.0}, {:.0}, {:.0} facing {:.0} degrees, {} camera presets",
        spawn.x,
        spawn.y,
        spawn.z,
        facing.x.atan2(facing.z).to_degrees(),
        camera.cam.presets.len()
    );
    // The ground under the start line, asked the way the ride asks it, so a start that is not on the
    // dirt says so before the window opens.
    let (on, _) = sim_world::World::ground(&mut sim, at);
    println!("the dirt under the start line is at {:.1} feet", on.y());
    // And under the plane, which `machine.txt` gives a height of its own (P19).
    if let Some(plane) = &garage.plane {
        let (on, _) = sim_world::World::ground(&mut sim, plane.at);
        println!("the dirt under the biplane is at {:.1} feet", on.y());
    }

    // ---- one frame of the original, posed from a save state (V2) -------------------------------
    // Everything the picture needs is loaded by now, and nothing a window or a pad gives is wanted:
    // the pose is the original's, not this shell's.
    if let Some((file, out)) = &args.moment {
        let moment = Moment::read(file)?;
        if !moment.track.is_empty() && !moment.track.eq_ignore_ascii_case(&args.track) {
            return Err(format!(
                "the moment is on {} and the shell was pointed at {}",
                moment.track, args.track
            ));
        }
        // A moment of the biplane (V21): the plane and its pilot instead of the bike and its parts.
        let plane = match moment.vehicle.as_str() {
            "biplane" => Some(load_biplane(&mut disc, &archives, &mut assets, moment.livery)?),
            _ => None,
        };
        let mut pilot = None;
        if let (Some(plane), Some(body)) = (&plane, &mut body) {
            if !biplane::seat(&mut body.rig.set, 0.5, 0.0) {
                println!("biplane: the rider's archive has no {:?}, no pilot", biplane::PILOT_CLIPS);
            } else if let Some(loaded) = assets.models.get_mut(body.geometry()) {
                let held = body.skin_into_at(loaded, 0, Some(&plane.anchors));
                println!("biplane: pilot's feet and hands off their places by {held:.2?} feet");
                pilot = Some(body.geometry());
            }
        }
        let shot = Shot {
            // No window to fit, so the PS2's own frame unless `--size` says otherwise.
            width: args.size.unwrap_or(PS2_FRAME).0,
            height: args.size.unwrap_or(PS2_FRAME).1,
            ground_radius: GROUND_RADIUS,
            chassis: CHASSIS,
            plane: plane.as_ref(),
            pilot,
        };
        return moment::draw(&moment, out, &shot, &ground, &assets, &vehicle, &look);
    }

    // ---- `--trace`: the solver's own numbers, with nobody on the pad (S22) -----------------------
    // No window, no pad, no camera and no drawing: just the ported frame, at the `dt` the original
    // runs a level at (the engine's 30 fps clamp, which is what a live capture of the original shows
    // on every row), and one line per frame of what the ground solver did with it. The columns are
    // the ones a ringing solver shows first — the peak g the wreck test reads, the body's own
    // acceleration in its frame, and each wheel's contact, load and sideways slip.
    if let Some(frames) = args.trace {
        let dt = F(args.trace_dt.unwrap_or(game_sim::tick::DeltaTime::CAP_30));
        let mut controls = Controls::default();
        if args.trace_brake {
            controls = Controls { brake: F::ONE, front_brake: F::ONE, rear_brake: F::ONE, ..controls };
        }
        if args.trace_gas {
            controls.gas = F::ONE;
        }
        if let Some(x) = args.trace_stick {
            controls.stick.0 = F(x);
        }
        let controls = controls;
        println!(
            "trace: {frames} frames at dt {} on {} / {}",
            dt.0, args.track, args.bike
        );
        println!("phy: {phy_member}");
        if args.trace_feel || args.trace_air {
            let gesture = !args.trace_stick_y.is_empty();
            if args.trace_air {
                println!("frame ground fr yaw pitch roll wx wy wz crossup stick_x stick_y speed wreck");
            } else {
                println!(
                    "frame speed vel_x vel_y vel_z x y z heading up ground wreck{}",
                    if gesture { " stick_y lip preload flip nose" } else { "" }
                );
            }
            let throttle = Controls { gas: F::ONE, ..Controls::default() };
            for i in 0..frames {
                let mut now = match args.trace_switch {
                    Some(n) if i < n => throttle.clone(),
                    _ => controls.clone(),
                };
                if let Some(&(_, y)) = args.trace_stick_y.iter().rev().find(|(at, _)| *at <= i) {
                    now.stick.1 = F(y);
                }
                let f = rider.frame(&phy, &tables, &now, dt, &mut sim);
                let r = &rider.ride;
                let (v, p, m) = (r.chassis.vel, r.position(), r.transform());
                if args.trace_air {
                    // Host floats: a diagnostic print. Yaw the way a capture's `atan2(m20, m22)`
                    // reads it, pitch as the nose's rise, roll as the right axis' rise against the
                    // up axis' height — `research/p29_air_attitude.py` reads the original's rows
                    // the same way, so the two columns compare.
                    let (right, up, fwd) = (m.0[0], m.0[1], m.0[2]);
                    let w = r.chassis.ang_vel;
                    println!(
                        "{i} {} {}{} {:.3} {:.3} {:.3} {:.4} {:.4} {:.4} {:.4} {:.3} {:.3} {:.3} {}",
                        if r.air.on_ground { 1 } else { 0 },
                        if r.front.mv.contact { 1 } else { 0 },
                        if r.rear.mv.contact { 1 } else { 0 },
                        fwd[0].atan2(fwd[2]).to_degrees(),
                        fwd[1].clamp(-1.0, 1.0).asin().to_degrees(),
                        right[1].atan2(up[1]).to_degrees(),
                        w.x(), w.y(), w.z(),
                        r.air.crossup.0,
                        r.held.stick.0 .0, r.held.stick.1 .0,
                        r.chassis.speed.0,
                        if f.wreck.is_some() { 1 } else { 0 },
                    );
                    continue;
                }
                // The gesture's columns: the stick, the lip as `air.rs` reads it (the tests'
                // definition), the preload shoving, a backflip turning, and the nose's angle above
                // the line the bike is travelling along, the full circle — summed frame to frame, a
                // backflip is a turn of it.
                let extra = if gesture {
                    let (n, h) = (m.0[2], v.x().hypot(v.z()).max(1e-6));
                    let lip = r.air.on_ground && v.y() > 5.0 && r.air.contact_vel.z() > 30.0 && r.held.brake == F::ZERO;
                    format!(
                        " {:.2} {} {} {} {:.4}",
                        now.stick.1.0,
                        lip as u8,
                        r.air.preload_firing as u8,
                        r.air.flip_active as u8,
                        n[1].atan2((n[0] * v.x() + n[2] * v.z()) / h),
                    )
                } else {
                    String::new()
                };
                println!(
                    "{i} {:.4} {:.4} {:.4} {:.4} {:.3} {:.3} {:.3} {:.5} {:.4} {} {}{extra}",
                    v.x().hypot(v.z()),
                    v.x(), v.y(), v.z(),
                    p.x(), p.y(), p.z(),
                    m.0[2][0].atan2(m.0[2][2]),
                    m.0[1][1],
                    if r.air.on_ground { 1 } else { 0 },
                    if f.wreck.is_some() { 1 } else { 0 },
                );
            }
            return Ok(());
        }
        // The numbers behind the columns, so a trace in a journal entry carries the tuning it was
        // taken on rather than a bike's name.
        {
            let (f, r) = (&rider.ride.front, &rider.ride.rear);
            println!(
                "bike: mass {} inertia {:?} cofg {:?} gravity {:?}",
                phy.mass.mass.0,
                (phy.mass.inertia.x(), phy.mass.inertia.y(), phy.mass.inertia.z()),
                (phy.mv.cofg.x(), phy.mv.cofg.y(), phy.mv.cofg.z()),
                (phy.mv.gravity.x(), phy.mv.gravity.y(), phy.mv.gravity.z()),
            );
            for (name, w, t) in [("front", f, &phy.front), ("rear", r, &phy.rear)] {
                println!(
                    "{name}: mount {:?} travel {} spring {}/{} damp {}/{} maxdamp {} ext {} radius {} inertia {}",
                    (w.w.mount.x(), w.w.mount.y(), w.w.mount.z()),
                    w.w.probe_length.0,
                    t.spring.0, t.spring_when_stopped.0,
                    w.w.damping_in.0, w.w.damping_out.0,
                    w.w.max_damper_speed.0, w.w.extension_rate.0,
                    w.mv.radius.0, t.tire_inertia.0,
                );
                // The arm the drive and side force go in on, which P8 found is the `.phy`'s own
                // vector used as it stands and not the contact patch.
                println!(
                    "{name}: tire force at {:?}",
                    (
                        t.body_tire_force_position.x(),
                        t.body_tire_force_position.y(),
                        t.body_tire_force_position.z(),
                    ),
                );
                let s0 = t.surfaces.first().copied().unwrap_or_default();
                println!(
                    "{name} surface0: friction {} drive {} side {} drag {}",
                    s0.wheel_friction.0, s0.max_drive_force.0, s0.max_side_force.0, s0.wheel_drag.0,
                );
            }
        }
        println!(
            "{:>5} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>3} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}",
            "frame", "gx", "gy", "gz", "vel_x", "vel_y", "vel_z", "pos_y", "fr", "f_comp",
            "r_comp", "f_load", "r_load", "f_side", "r_side", "pitch", "roll", "tq_x",
            "aa_x", "wx", "fz_y"
        );
        let mut peak = (F::ZERO, F::ZERO, F::ZERO);
        for i in 0..frames {
            let f = rider.frame(&phy, &tables, &controls, dt, &mut sim);
            if let Some(w) = f.wreck {
                println!("frame {i}: WRECK {w:?}");
            }
            let r = &rider.ride;
            let g = r.mv.g_peak;
            peak = (
                if F(g.x()).abs() > peak.0.abs() { F(g.x()) } else { peak.0 },
                if F(g.y()).abs() > peak.1.abs() { F(g.y()) } else { peak.1 },
                if F(g.z()).abs() > peak.2.abs() { F(g.z()) } else { peak.2 },
            );
            let g = r.mv.g_peak;
            let v = r.chassis.vel;
            // The sideways slide the tire answers: the contact's velocity across the chassis' own
            // `x`, which is the axis the g load that wrecks the bike is measured along. Host floats:
            // this is a diagnostic print, not a simulation step.
            let m = r.transform();
            let side = |w: &game_sim::ride::Wheel| {
                let (v, x) = (w.w.contact_velocity, m.0[0]);
                v.x() * x.x() + v.y() * x.y() + v.z() * x.z()
            };
            let touch = |w: &game_sim::ride::Wheel| if w.mv.contact { 1 } else { 0 };
            println!(
                "{:>5} {:>9.3} {:>9.3} {:>9.3} {:>9.3} {:>9.3} {:>9.3} {:>9.2} {:>1}{:>1}{:>1} {:>8.4} {:>8.4} {:>8.1} {:>8.1} {:>8.3} {:>8.3} {:>8.4} {:>8.4} {:>8.1} {:>8.2} {:>8.3} {:>8.1}",
                i,
                g.x(), g.y(), g.z(),
                v.x(), v.y(), v.z(),
                r.position().y(),
                touch(&r.front), touch(&r.rear), if r.air.on_ground { 1 } else { 0 },
                r.front.w.compression.0, r.rear.w.compression.0,
                r.front.w.spring_force.0, r.rear.w.spring_force.0,
                side(&r.front), side(&r.rear),
                r.chassis.euler.x(), r.chassis.euler.z(),
                r.chassis.torque.x(), r.chassis.ang_accel.x(), r.chassis.ang_vel.x(),
                r.chassis.force.y(),
            );
        }
        println!(
            "peak g over {frames} frames: {:.3} sideways, {:.3} vertical, {:.3} along — phase {:?}",
            peak.0.0, peak.1.0, peak.2.0, rider.phase()
        );
        return Ok(());
    }

    // ---- the tape ------------------------------------------------------------------------------
    // `--play` reads one in before anything opens, because a tape that will not parse should say so
    // before a window appears; `--record` starts an empty one and the loop writes it out at the end.
    let mut head = match &args.play {
        None => None,
        Some(path) => {
            let tape = read_tape(path, args.phase)?;
            println!(
                "riding {}: {} frames{}",
                path.display(),
                tape.len(),
                match tape.name.is_empty() {
                    true => String::new(),
                    false => format!(" of {}", tape.name),
                }
            );
            Some(Head::new(tape))
        }
    };
    let recorder = args.record.as_ref().map(|path| {
        println!("recording to {}", path.display());
        // What the tape is of, for a human reading a directory of them and for a failure line.
        Recorder::new(format!("{} on {} / {}", args.rider, args.track, args.bike))
    });

    // ---- the window and the pad ---------------------------------------------------------------
    let mut window = Window::open(&args.display, args.size, args.scale, "openmx — freeride")?;
    let mut pads = Pads::open(args.pad.as_deref(), args.wiring.clone());
    match &pads.name {
        Some(name) => println!("pad: {name}"),
        None => println!("pad: none found — riding on the keyboard (X throttle, Z brake, arrows)"),
    }
    // How the throttle is wired, when it is not the plain binding: a session's own log should say,
    // because a rider who mapped the wrong axis sees a bike that will not pull and no reason why.
    match args.wiring.throttle {
        Throttle::Cross => {}
        Throttle::Trigger(control) => println!(
            "throttle: the {0} trigger's travel — part throttle, and no {0} control this session",
            control.name(),
        ),
    }
    let pressures: Vec<String> = args
        .wiring
        .pressure
        .iter()
        .enumerate()
        .filter_map(|(axis, c)| c.map(|c| format!("{axis}:{}", c.name())))
        .collect();
    if !pressures.is_empty() {
        println!("pad pressures: {}", pressures.join(" "));
    }

    // ---- the loop ------------------------------------------------------------------------------
    let light = Lighting::of(&look);
    // The frame: `--size`'s, which stays put whatever the window does, or the largest 4:3 one inside
    // the window, which is re-made whenever the window changes size (below).
    let frame_size = |window: &Window| match args.size {
        Some(size) => size,
        None => fit_frame((window.width, window.height), window.scale),
    };
    let (width, height) = frame_size(&window);
    println!(
        "window {}x{} (screen {}x{}), drawing {width}x{height} at scale {}",
        window.width, window.height, window.screen.0, window.screen.1, window.scale
    );
    let mut frame = Frame::new(width, height, look.fog_color);
    let mut engine = Engine::new();
    let mut world = World {
        place: sim,
        reports: [Report::missing(); PADS],
        sim: Sim::with_rider(0, Arc::new(tables), phy, rider),
        garage,
        riding: true,
        said: rider::Frame::default(),
        hands: Controls::default(),
        tape: recorder,
        played_dt: None,
    };

    // The riding overlay (P6). It is the shell's own furniture, so it is created here beside the
    // window rather than inside the simulation's `World`, and it reads the port's own values at draw
    // time instead of being fed them.
    let mut hud = Hud::new(args.hud);
    hud.note_vehicle(world.garage.up.name());
    let first = match (world.garage.up, &plane_camera) {
        (Kind::Biplane, Some(cam)) => &cam.cam,
        _ => &camera.cam,
    };
    if !first.presets.is_empty() {
        hud.note_camera(&first.preset().name);
    }
    println!("vehicle: {}", world.garage.up.name());
    // H toggles it, on its own edge: the window keeps key state and not key events, so the shell
    // remembers last frame's to tell a press from a hold.
    let mut hud_key = false;
    // M swaps machines, on its edge the same way (P19).
    let mut swap_key = false;

    let mut shot = args.shot.clone();
    let started = Instant::now();
    let mut window_of_frames = (Instant::now(), 0u32, 0usize);
    loop {
        let top = Instant::now();

        window.poll();
        if window.closed() || window.held(key::ESCAPE) {
            break;
        }
        // A window that changed size gets a frame of its new size rather than the old one stretched
        // over it — and with it the aspect the camera's field of view and its viewport test read,
        // which stays 4:3 because that is the shape `fit_frame` keeps.
        if window.resized() {
            let (width, height) = frame_size(&window);
            if (width, height) != (frame.width, frame.height) {
                frame = Frame::new(width, height, look.fog_color);
                println!(
                    "window {}x{}, drawing {width}x{height}",
                    window.width, window.height
                );
            }
        }
        // The pad this frame: the tape's, if one is being ridden, and the host's otherwise. A tape
        // that has run out ends the session — there is no pad behind it to carry on with.
        match &mut head {
            None => world.reports = [pads.report(&window), Report::missing()],
            Some(head) => match head.next() {
                Some(frame) => {
                    world.reports = frame.reports;
                    world.played_dt = Some(frame.dt());
                }
                None => {
                    println!("the tape ran out after {} frames", head.len());
                    break;
                }
            },
        }
        let hud_now = window.held(key::H);
        if hud_now && !hud_key {
            hud.on = !hud.on;
        }
        hud_key = hud_now;
        // The swap, between frames, which is where the original's pause menu and prompt run it: the
        // controller moves and the machine taken gets its own camera, which starts where it is
        // rather than lagging in from wherever it was last. A played tape is the ride it recorded,
        // and the tape has no swap in it, so the key is not read while one is being ridden.
        let swap_now = window.held(key::M);
        if swap_now && !swap_key && head.is_none() {
            match world.garage.swap() {
                Ok(up) => {
                    let cam = match (up, &mut plane_camera) {
                        (Kind::Biplane, Some(cam)) => &mut cam.cam,
                        _ => &mut camera.cam,
                    };
                    cam.snap = true;
                    if !cam.presets.is_empty() {
                        hud.note_camera(&cam.preset().name);
                    }
                    hud.note_vehicle(up.name());
                    println!("vehicle: {}", up.name());
                }
                Err(e) => println!("vehicle: {e}"),
            }
        }
        swap_key = swap_now;

        // One frame of the game's own fixed update, which calls `World::simulate` inside itself.
        let step = engine.step(wall(started), engine_input(&world.reports[0]), &mut world);
        if step.outcome == Outcome::Quit {
            break;
        }
        // A frame the engine did not simulate is a frame of the tape nobody rode, and the rest of the
        // ride would be a different run from the one recorded. Nothing in this shell sets the engine's
        // frozen flag, so this says so and stops rather than carrying on with a hole in the ride.
        if head.is_some() && !step.simulated {
            return Err("the engine skipped a frame while a tape was being ridden".into());
        }
        // What the simulation was actually handed, which the pose and the camera below step with too.
        let dt = world.played_dt.unwrap_or(step.dt);

        // The body on the bike, posed by the body's own sixty-state machine (V18): one call that runs
        // the rider's animation actor off the fields the ride just wrote and writes the pose it chose
        // onto the body's 22 joints. It is a second entry point of the simulation rather than a step
        // of `Rider::frame` — the body is an actor of its own in the original — and it is called from
        // here because the clip table it selects out of is the renderer's. A frozen ride does not
        // call it, so the pose is left where it was.
        //
        // The gate is the two inputs the body has that no ride holds: a start-line clock, which this
        // shell has none of, and the wall clock the idle pool seeds itself from. Both zero, so the
        // body's start sequence is the full breathe-and-wait and two runs of the same tape draw the
        // same idles.
        // Neither moves while the plane is up: the bike was left with no controller and nothing steps it.
        let bike_up = world.garage.up == Kind::Bike;
        if let Some(body) = &mut body {
            if world.riding && bike_up {
                body.ride(dt, &mut world.sim.rider, &world.hands, rider::Gate::default());
            }
        }

        // And the bike under it (V9). Two halves in one call: the chassis skeleton posed from the
        // vehicle's own animation machine, and then the suspension, steering and wheel nodes written
        // over that pose from the ride's own numbers — the fork travel, the steering angle and the two
        // wheels' spin, which are not in any clip because no clip could have them.
        if let Some(chassis) = &mut chassis {
            if world.riding && bike_up {
                let ride = &world.sim.rider.ride;
                let want = Chassis::want(ride, &world.sim.rider.tricks);
                // The rider's two axes and not zeros: in the air they are the crossup sweep the
                // chassis' clip is played at, and zero is the sweep's far end (P29).
                let request = Chassis::riding(&world.sim.rider);
                chassis.frame(dt, want, &request, Drive::of(ride));
                if let Some(loaded) = assets.models.get_mut(CHASSIS) {
                    chassis.skin_into(loaded);
                }
            }
        }

        // What the rider did, as a line: a run nobody is watching still says whether a trick
        // started, landed or put the rider down, and by which of the original's reason codes.
        if let Some(t) = world.said.started {
            println!("trick: {}", table.display_name(0, t));
            hud.note_trick(table.display_name(0, t));
        }
        if let Some(t) = world.said.landed {
            println!("landed: {}", table.display_name(0, t));
            hud.note_trick(&format!("{} landed", table.display_name(0, t)));
        }
        if let Some(why) = world.said.wreck {
            println!("wreck: reason {} — L3 and R3 together to get back on", why.0);
            hud.note_wreck(why);
        }
        if world.said.respawned {
            println!("back on the start line");
        }

        // The camera, after the frame, which is where the original's viewport updates it: the shell
        // picks the preset, the ported camera does everything else.
        // Whichever machine has the controller has the camera: the bike's, on the track's presets, or
        // the plane's, on its own file's.
        let follow = match (world.garage.up, &mut plane_camera) {
            (Kind::Biplane, Some(cam)) => cam,
            _ => &mut camera,
        };
        let presets = follow.cam.presets.len();
        if presets > 1 {
            let pad = world.sim.input.pad(0);
            // L1 only: R1 is the third of the trick decoder's buttons now. The walk is the
            // original's next-camera routine: on to the next preset the file lets the player cycle to.
            // An L1 pressed with R1 already down is the second half of the reset chord, not a camera.
            if pad.pressed(Control::L1) && !pad.held(Control::R1) {
                follow.cam.next_preset();
                println!("camera: {}", follow.cam.preset().name);
                hud.note_camera(&follow.cam.preset().name);
            }
        }
        // The camera's target block, as the original's vehicle fills it once a frame (`chase.rs`).
        // The airborne flag is the one the camera heads along the velocity on: in the air the
        // backflip's upside-down forward row never reaches it (P34).
        let rider = chase::ridden(&world.garage, &world.sim.rider.ride);
        let view = if presets == 0 {
            None
        } else {
            // The camera's one question about the world: the ground under the look-at point, which
            // the track answers (S23). Without it the view sinks into the dirt on the first dip.
            // This frame has no obstacle push-out — the base class's did, and the base class's
            // frame is not the one the disc runs.
            Some(follow.update(
                dt,
                &rider,
                F(NEAR_CLIP),
                &world.sim.tables.ride.curve,
                Switches {
                    // Nothing in the shell sets the engine bit that stops the look-ahead, and the
                    // window is one view: the split-screen pitch trim wants a window less than
                    // 0.55 as tall as it is wide, which is the same test the original makes of its
                    // own viewport's two extents.
                    no_look_ahead: false,
                    short_viewport: (frame.height as f32) / (frame.width as f32) < 0.55,
                },
                &mut world.place,
            ))
        };
        let bike_look = vector(rider.target.forward);
        let pos = vector(rider.target.body_point);
        let camera = match &view {
            Some(view) => Camera {
                eye: vector(view.position),
                at: vector(view.look_at),
                // The file's field of view is the horizontal one; the rasteriser wants the vertical
                // one in radians, which depends on the shape of the window it is drawing into.
                fov: vertical_fov(view.fov.0, aspect(&frame)),
                near: NEAR_CLIP,
                far: 4000.0,
            },
            None => Camera::behind(pos, bike_look),
        };

        // The picture: the dirt, then the scene's props and the bike standing on them.
        clear(&mut frame, look.fog_color);
        let mut drawn = ground.draw(&mut frame, &camera, &light, pos, GROUND_RADIUS);
        let bike_at = chassis_at(&world.sim.rider.ride);
        // Each part rides on one node of the chassis, so its transform is that node's place in the
        // chassis followed by the chassis's place in the world.
        let mut actor: Vec<(&str, M4)> = vec![(CHASSIS, bike_at)];
        // The node's *live* transform where a frame has posed the chassis, and its bind transform
        // where nothing has: the front fender and the number plate hang off `Handlebars`, so a part
        // placed at a bind pose is a part that does not steer with the bike it is bolted to.
        let node_now = |name: &str| -> Option<M4> {
            chassis.as_ref().and_then(|c| c.node(name)).or_else(|| {
                let model = &assets.models.get(CHASSIS)?.model;
                let i = model.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(name))?;
                Some(model.world(i))
            })
        };
        for part in &vehicle.parts {
            let at = match node_now(&part.placement) {
                Some(node) => node.mul(bike_at),
                None => bike_at,
            };
            actor.push((part.model.as_str(), at));
        }
        // Where the chassis wants the body's hands and feet *this* frame: the four dummy anchors at
        // its grips and its pegs, off the live pose. They were read once, at load, from the bind
        // pose — but `Dummy_Hand_L` and `Dummy_Hand_R` hang off `Handlebars`, which the steering
        // drive turns, so a rider aimed at the bind pose lets go of the bars the moment the bike is
        // steered (V20). Falls back to the bind pose before the bike has a frame of its own.
        let grips = match &chassis {
            Some(chassis) => chassis_grips(chassis),
            None => match assets.models.get(CHASSIS) {
                Some(loaded) => anchors_of(&Skeleton::of(&loaded.model), &[]),
                None => Default::default(),
            },
        };
        // And the rider, at the bike's own transform: the clips put the body on the bike
        // themselves, so there is nothing to bolt (`game_render::rider`). Not before a frame has
        // posed it, though: a blend with no clip behind it leaves the bind pose, which is the one
        // pose the game never draws.
        if let Some(body) = &body {
            if body.posed {
                // Which of the body's five levels of detail this camera has earned, and the hands and
                // feet put back where the chassis wants them. Both read the camera, so the skinning
                // is here rather than beside the pose above.
                let away = pos.sub(camera.eye).len();
                let focal = camera.focal(frame.height);
                if let Some(loaded) = assets.models.get_mut(body.geometry()) {
                    let group = loaded.model.group_for(away, loaded.radius, focal);
                    body.skin_into_at(loaded, group, Some(&grips));
                }
                actor.push((body.geometry(), bike_at));
            }
        }
        // With the plane up the bike is hidden, as the swap hides its model, and the rider is the
        // plane's pilot — whom V21 seats. The plane stands in the world either way: parked until it
        // is taken, wherever it was left after.
        if !bike_up {
            actor.clear();
        }
        // At its body's pose, nose and up off the plane's own measure, with the prop at the engine's
        // angle (S35) — or the bent cone, unturned, once it is wrecked.
        if let (Some(drawn), Some(plane)) = (&plane_drawn, &world.garage.plane) {
            let at = M4::placement(vector(plane.at), vector(plane.facing), vector(plane.up));
            actor.extend(drawn.turned_parts(at, plane.prop_angle().map(|a| a.0)));
        }
        drawn += draw_scene(&mut frame, &look, &assets, &actor, &camera);
        // And the overlay over the top of it, after everything with a depth test and before anything
        // that reads the frame: what the window shows and what `--shot` writes are the same picture.
        hud.plane = !bike_up;
        hud.flight = match (bike_up, &world.garage.plane) {
            (false, Some(plane)) => plane.gauges().map(|(throttle, air)| (throttle.0, air.0)),
            _ => None,
        };
        hud.draw(&mut frame, world.sim.input.pad(0), &world.sim.rider, world.riding);
        window.present(&frame.color, frame.width, frame.height)?;

        // One frame, on paper, for a run nobody is watching: the first by default, or the one
        // `--shot-frame` asks for, which is how a shot catches a ride in progress.
        if let Some(path) = shot.take_if(|_| engine.frame >= args.shot_frame) {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            png::write_rgb(&path, frame.width, frame.height, &frame.color)?;
            println!("wrote {}", path.display());
        }

        // A line a second, so a run that is not being watched still says what it did.
        window_of_frames.1 += 1;
        window_of_frames.2 += drawn;
        if window_of_frames.0.elapsed() >= Duration::from_secs(1) {
            let (since, frames, triangles) = window_of_frames;
            println!(
                "{:>3.0}s {:>4.1} fps, {:>6} triangles a frame, {:>5.1} mph{}",
                started.elapsed().as_secs_f32(),
                frames as f32 / since.elapsed().as_secs_f32(),
                triangles / frames.max(1) as usize,
                // Feet a second to miles an hour, for a number a rider can read.
                rider.speed.0 * 0.681_818,
                match (world.riding, world.sim.rider.phase()) {
                    (false, _) => " (frozen)",
                    _ if !bike_up => " (biplane)",
                    (_, Phase::Riding) => "",
                    (_, Phase::Down) => " (down)",
                    (_, Phase::Resetting) => " (getting up)",
                }
            );
            // And the plane's own numbers while it flies: its height, the gauges and whether a
            // wheel is down.
            if let (false, Some(plane)) = (bike_up, &world.garage.plane)
                && let Some((throttle, air)) = plane.gauges()
            {
                println!(
                    "     plane at {:.0}, {:.0}, {:.0}: throttle {:.0}%, airspeed {:.1} mph, {}",
                    plane.at.x(),
                    plane.at.y(),
                    plane.at.z(),
                    throttle.0 * 100.0,
                    air.0 * 0.681_818,
                    if plane.on_ground() { "on the ground" } else { "in the air" }
                );
            }
            window_of_frames = (Instant::now(), 0, 0);
        }

        if let Some(limit) = args.seconds {
            if started.elapsed().as_secs_f32() >= limit {
                break;
            }
        }
        // Pace to 60 Hz. A frame that overran does not sleep, and the engine's own clock is what the
        // simulation sees, so a slow frame becomes a longer step rather than a slower game.
        if let Some(rest) = FRAME.checked_sub(top.elapsed()) {
            std::thread::sleep(rest);
        }
    }

    // The tape, written once the session is over: every frame the simulation ran, in order, in the
    // form `game_sim::replay::PadTape::parse` reads back. It is game data, so it goes where the
    // caller said and the caller is expected to say `context/`.
    if let (Some(path), Some(tape)) = (&args.record, &world.tape) {
        if let Some(dir) = path.parent() {
            if !dir.as_os_str().is_empty() {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
        }
        std::fs::write(path, tape.to_text()).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("wrote {} ({} frames)", path.display(), tape.len());
    }

    // Where a session that was taped ended up, as the number the replay harness compares runs by.
    // A played tape is a function of its file — the pad and the frame length both come off it — so
    // two rides of one tape print the same hash, and a change that moved the physics prints a
    // different one. It is only printed for a taped session: a live ride is paced by the wall clock
    // and its hash would be a different number every time, which is a number worth nothing.
    if head.is_some() || args.record.is_some() {
        println!("ride hash {:#018x}", game_sim::hash::of(&world.sim.rider.ride));
    }

    println!(
        "ridden for {:.1}s over {} frames",
        started.elapsed().as_secs_f32(),
        engine.frame
    );
    Ok(())
}

/// The wall clock, as the game's own timer would have it: whole 100 ms periods and the bus-clock
/// counts into the current one, which is what [`game_sim::tick::clock_ms`] is written to convert.
fn wall(since: Instant) -> Timer0 {
    let us = since.elapsed().as_micros() as u64;
    Timer0 {
        ticks: (us / 100_000) as u32,
        // 576,000 counts a second is 576 per millisecond, and the remainder here is microseconds.
        count: ((us % 100_000) * 576 / 1_000) as u32,
    }
}

/// The engine's own record of the pad for this frame.
///
/// The fixed update stores it and hands it to the simulation; this shell reads the full report through
/// the port's input manager instead, so what goes in here is the summary the engine's own type holds.
fn engine_input(report: &Report) -> Input {
    let axis = |i: usize| (report.axis(i) as i16 - 0x7f).clamp(-128, 127) as i8;
    Input {
        buttons: report.buttons(),
        // The left stick, which is axes 2 and 3 of the report.
        stick: (axis(2), axis(3)),
    }
}

/// Back to the clear colour and an empty depth buffer, which is what [`Frame::new`] does for a frame
/// that is only drawn once.
fn clear(frame: &mut Frame, color: [u8; 3]) {
    for pixel in frame.color.chunks_exact_mut(3) {
        pixel.copy_from_slice(&color);
    }
    frame.depth.fill(0.0);
}

/// A point the simulation read, as the renderer's own vector. The port keeps positions in
/// [`ps2_float::F`] because the simulation's arithmetic has to be the game's; a camera and a triangle
/// do not, and this is the one place the two meet.
fn point(p: [F; 3]) -> V3 {
    v3(p[0].0, p[1].0, p[2].0)
}

/// A quadword the simulation holds, as the renderer's own vector: the other half of [`point`], for a
/// position or an axis the ride has already computed.
fn vector(v: Vf) -> V3 {
    v3(v.x(), v.y(), v.z())
}

/// The shape of the frame being drawn into, which is the shape of the window: what
/// [`vertical_fov`] divides by, and what the rasteriser's own projection uses.
fn aspect(frame: &Frame) -> f32 {
    frame.width as f32 / frame.height as f32
}

/// The terrain file a scene asks for. Most scenes name it `SX02.trn`, but what the archive holds is
/// `sx02.ps2.trn`: the loader tags the file with the platform it was built for, and a few scenes
/// already spell the tag out.
fn terrain_bytes(res: &Res, named: &str) -> Result<Vec<u8>, String> {
    if let Ok(bytes) = res.read_suffix(named) {
        return Ok(bytes);
    }
    let tagged = match named.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem}.ps2.{ext}"),
        None => return Err(format!("the archive has no member ending in {named}")),
    };
    res.read_suffix(&tagged)
        .map_err(|_| format!("the archive has no member ending in {named} or {tagged}"))
}

/// A tape to ride, from the file it is in.
///
/// Two kinds, because a tape of the port and a tape of the original are the same kind of file and the
/// shell should ride either:
///
/// - a `.tape` (anything that is not a `.p2m2`) — the form [`PadTape::to_text`] writes, which is what
///   `--record` leaves behind and what a tape read off a `./kit record` capture of the original is
///   written as;
/// - a `.p2m2` — a human's PCSX2 input recording of the original, read by the ported reader (I4).
///   Which of a tick's two video fields the game actually read is the one thing that file cannot say,
///   so `--phase` says it; `second` is the measured answer for the recording the project has, and
///   that is a fact about that recording rather than about the game.
///
/// Which means `--play a.p2m2 --phase second --record a.tape` converts one into the other, by riding
/// it: the tape written is the pad the port was handed, frame for frame.
fn read_tape(path: &std::path::Path, phase: TickPhase) -> Result<PadTape, String> {
    let named = |e: std::io::Error| format!("{}: {e}", path.display());
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("p2m2")) {
        let bytes = std::fs::read(path).map_err(named)?;
        return PadTape::from_p2m2(&bytes, phase).map_err(|e| format!("{}: {e}", path.display()));
    }
    let text = std::fs::read_to_string(path).map_err(named)?;
    PadTape::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The member of `MX.PSR` whose name ends that way — archives are named after the track or bike they
/// hold, in a directory this code does not need to know.
fn member_ending(disc: &Disc, suffix: &str) -> Result<String, String> {
    let want = suffix.to_ascii_lowercase();
    // The shortest match, because the menus keep their own copy of a bike one directory deeper and
    // that copy holds the spinning showroom model rather than the one a race uses.
    disc.entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with(&want))
        .min_by_key(|e| e.name.len())
        .map(|e| e.name.clone())
        .ok_or_else(|| format!("MX.PSR has no member ending in {suffix}"))
}

/// The biplane (V21), loaded for a moment of it: the model, its two prop cones and their textures
/// from `machine.res`, which is the one archive that has the textures. Says which emitters the plane
/// would attach — the list is in the track's second archive — and that none are drawn.
fn load_biplane(
    disc: &mut Disc,
    archives: &[Res],
    assets: &mut Assets,
    livery: usize,
) -> Result<Biplane, String> {
    let machine = [disc.res(&member_ending(disc, biplane::ARCHIVE)?)?];
    let plane = Biplane::load(assets, &machine, livery)
        .ok_or("biplane: no archive has the plane's model")?;
    let seat = plane.anchors.iter().filter(|a| a.is_some()).count();
    println!(
        "biplane: {} with {} prop, {seat} of 4 seat anchors",
        plane.model,
        if plane.prop.is_some() { "a" } else { "no" }
    );
    let list = archives.iter().find_map(|a| a.read_suffix(biplane::PARTICLES).ok());
    match list {
        Some(bytes) => {
            for e in biplane::emitters(&ini::text(&bytes)) {
                let on = e.node.as_deref().unwrap_or("its collision model");
                println!("  emitter {} ({}) on {on}: not drawn, the port has no particles", e.emitter, e.name);
            }
        }
        None => println!("  no {} in these archives", biplane::PARTICLES),
    }
    Ok(plane)
}

/// The command line. `Ok(None)` means `--help` was asked for and the shell should not start.
fn parse_args() -> Result<Option<Args>, String> {
    let mut args = Args {
        // The freeride area D6 read and proved: no gates, no laps, nobody else on the track.
        track: "free06".to_string(),
        bike: "125sx".to_string(),
        // The stock gear. Every other rider archive on the disc is a repaint of the same body.
        rider: "thq1".to_string(),
        size: None,
        scale: 1,
        seconds: None,
        pad: None,
        shot: None,
        shot_frame: 0,
        hud: false,
        camera: START_CAMERA.to_string(),
        moment: None,
        wrecks: true,
        trace: None,
        trace_dt: None,
        trace_brake: false,
        trace_gas: false,
        trace_stick: None,
        trace_feel: false,
        trace_air: false,
        phy: None,
        trace_at: None,
        trace_switch: None,
        trace_stick_y: Vec::new(),
        record: None,
        play: None,
        phase: TickPhase::default(),
        wiring: Wiring::default(),
        vehicle: Kind::Bike,
        display: std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string()),
    };
    let mut argv = std::env::args().skip(1);
    while let Some(a) = argv.next() {
        let mut next = |what: &str| argv.next().ok_or(format!("{a} wants {what}"));
        match a.as_str() {
            "--track" => args.track = next("a track name, like free06")?,
            "--bike" => args.bike = next("a bike name, like 125sx")?,
            "--rider" => args.rider = next("a rider name, like thq1, or none")?,
            "--pad" => args.pad = Some(next("a device, like /dev/input/js0")?),
            "--shot" => args.shot = Some(next("a path for the first frame")?.into()),
            "--shot-frame" => {
                args.shot_frame = next("which frame to write")?
                    .parse()
                    .map_err(|_| "--shot-frame: not a frame number")?;
            }
            "--hud" => args.hud = true,
            "--camera" => args.camera = next("a camera preset's name, like \"Freestyle Cam\"")?,
            "--moment" => {
                let file: std::path::PathBuf = next("a moment file")?.into();
                let out: std::path::PathBuf = next("a path for the frame")?.into();
                args.moment = Some((file, out));
            }
            "--display" => args.display = next("a display, like :0")?,
            "--no-wrecks" => args.wrecks = false,
            "--record" => args.record = Some(next("a path for the tape")?.into()),
            "--play" => args.play = Some(next("a tape to ride")?.into()),
            "--phase" => {
                args.phase = match next("first or second")?.as_str() {
                    "first" => TickPhase::First,
                    "second" => TickPhase::Second,
                    other => return Err(format!("--phase: first or second, not {other}")),
                }
            }
            "--throttle" => {
                args.wiring.throttle = match next("cross, r2 or l2")?.as_str() {
                    "cross" => Throttle::Cross,
                    "r2" => Throttle::Trigger(Control::R2),
                    "l2" => Throttle::Trigger(Control::L2),
                    other => return Err(format!("--throttle: cross, r2 or l2, not {other}")),
                }
            }
            "--pad-pressure" => {
                args.wiring.pressure =
                    Wiring::parse_pressures(&next("an axis map, like ds2@8, or none")?)
                        .map_err(|why| format!("--pad-pressure: {why}"))?;
            }
            "--trace-brake" => args.trace_brake = true,
            "--trace-gas" => args.trace_gas = true,
            "--trace-feel" => args.trace_feel = true,
            "--trace-air" => args.trace_air = true,
            "--trace-switch" => {
                args.trace_switch = Some(
                    next("a frame to switch at")?
                        .parse()
                        .map_err(|_| "--trace-switch: not a number of frames")?,
                );
            }
            "--trace-stick-y" => {
                let bad = || "--trace-stick-y: FRAME:Y,FRAME:Y,...".to_string();
                for pair in next("frame:y pairs")?.split(',') {
                    let (f, y) = pair.split_once(':').ok_or_else(bad)?;
                    let at = f.trim().parse().map_err(|_| bad())?;
                    args.trace_stick_y.push((at, y.trim().parse().map_err(|_| bad())?));
                }
            }
            "--trace-at" => {
                let v: Vec<f32> = next("x,z,degrees")?
                    .split(',')
                    .map(|w| w.trim().parse())
                    .collect::<Result<_, _>>()
                    .map_err(|_| "--trace-at: x,z,degrees")?;
                let [x, z, deg] = v[..] else { return Err("--trace-at: x,z,degrees".into()) };
                args.trace_at = Some((x, z, deg));
            }
            "--vehicle" => args.vehicle = Kind::parse(&next("a vehicle, bike or biplane")?)?,
            "--phy" => args.phy = Some(next("a .phy on the disc, like race125p.phy")?),
            "--trace-stick" => {
                args.trace_stick = Some(
                    next("a stick position, -1 to 1")?
                        .parse()
                        .map_err(|_| "--trace-stick: not a number")?,
                );
            }
            "--trace-dt" => {
                args.trace_dt = Some(
                    next("a frame length in seconds")?
                        .parse()
                        .map_err(|_| "--trace-dt: not a number of seconds")?,
                );
            }
            "--trace" => {
                args.trace = Some(
                    next("how many frames to trace")?
                        .parse()
                        .map_err(|_| "--trace: not a number of frames")?,
                )
            }
            "--seconds" => {
                args.seconds = Some(
                    next("how long to ride")?
                        .parse()
                        .map_err(|_| "--seconds: not a number of seconds")?,
                )
            }
            "--scale" => {
                args.scale = next("how many window pixels to a frame pixel")?
                    .parse()
                    .map_err(|_| "--scale: not a whole number")?;
                if args.scale == 0 {
                    return Err("--scale: a pixel is at least one pixel".into());
                }
            }
            "--size" => {
                let v = next("WIDTHxHEIGHT")?;
                let (w, h) = v.split_once('x').ok_or("--size wants WIDTHxHEIGHT")?;
                let w: usize = w.parse().map_err(|_| "--size: not a width")?;
                let h: usize = h.parse().map_err(|_| "--size: not a height")?;
                if w == 0 || h == 0 {
                    return Err("--size: a window has to have some pixels in it".into());
                }
                args.size = Some((w, h));
            }
            "--help" | "-h" => {
                println!(
                    "openmx [--track free06] [--bike 125sx] [--rider thq1] [--size 640x448]\n\
                            [--scale 1]\n\
                            [--pad /dev/input/js0] [--display :0] [--seconds N] [--shot PATH]\n\
                            [--moment FILE OUT.png] [--no-wrecks] [--hud] [--camera NAME]\n\
                            [--shot-frame N] [--vehicle bike|biplane]\n\
                            [--record TAPE] [--play TAPE|RECORDING.p2m2 [--phase second]]\n\
                            [--throttle cross|r2|l2] [--pad-pressure ds2@8|AXIS:CONTROL,...]\n\
                     \n\
                     Rides the freeride area from the disc named in kit.toml.\n\
                     Cross throttle, Square brake, R2 front brake, L2 clutch, left stick steers\n\
                     and leans, the d-pad preloads, L3 balances, R3 reverses, Start freezes,\n\
                     L1 changes camera, Escape quits. --camera starts on a camera preset by its\n\
                     name in the track's cpresets.ini (\"Race Cam 1\" unless asked).\n\
                     A trick is a gesture: hold Circle, Triangle and/or R1 and push the d-pad.\n\
                     L1 and R1 together put the bike back (the original's own chord).\n\
                     On the keyboard: X throttle, Z brake, arrows steer, C/V/D trick, A+D reset.\n\
                     The throttle is a pressure and not a switch (the game reads Cross from its\n\
                     pressure byte): a key ramps it, --throttle r2 rides it on a trigger instead,\n\
                     giving up that trigger's own control, and --pad-pressure says which joystick\n\
                     axes a pad reports its buttons' own pressures on.\n\
                     H toggles the riding overlay, which --hud starts on.\n\
                     --vehicle biplane starts in the track's biplane, where its machine.txt\n\
                     parks one, and M swaps between the bike and the plane the way the\n\
                     original's \"Switch To Machine\" does. The plane flies over the terrain\n\
                     alone: Cross opens the throttle and Square closes it.\n\
                     The window opens at the screen's size and the frame is the largest 4:3 one\n\
                     inside it, re-made when the window is resized; --size draws a fixed frame\n\
                     instead (640x448 is the PS2's), and --scale draws each frame pixel N times\n\
                     across and down.\n\
                     --no-wrecks turns the session's wrecks off (a wreck otherwise throws the\n\
                     rider, tumbles the bike and puts you back on it); --moment draws one frame posed\n\
                     from a moment of the original and quits; --trace N rides N frames with\n\
                     nobody on the pad and prints the solver's own numbers, and --trace-brake\n\
                     or --trace-gas hold the brakes or the throttle while it does, --trace-stick X\n\
                     holds the stick over and --trace-feel prints a short line a frame instead;\n\
                     --trace-air prints the chassis' attitude and its rates instead;\n\
                     --trace-switch N rides the throttle alone for N frames first;\n\
                     --trace-stick-y F:Y,... sets the stick's y from frame F on, each until\n\
                     the next, and adds the backflip's columns to --trace-feel's line;\n\
                     --trace-at X,Z,DEG starts the bike there instead of the start line, and\n\
                     --phy NAME rides the bike archive's NAME (pro125a.phy) instead of Race125P.phy.\n\
                     --record writes every simulated frame's pad to a tape (under context/,\n\
                     it is game data); --play rides one instead of the pad, at the tape's own\n\
                     frame lengths, and quits when it runs out. A human's PCSX2 recording\n\
                     of the original (.p2m2) is a tape too and rides the same; --phase says\n\
                     which of a tick's two video fields that recording's game read."
                );
                return Ok(None);
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Some(args))
}
