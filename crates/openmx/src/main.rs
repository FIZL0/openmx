//! **openmx** — the playable shell: a window, a real pad, and the freeride area to ride it in.
//!
//! ```sh
//! cargo run --release -p openmx                    # the front end: pick a track and ride it
//! cargo run --release -p openmx -- --track free06  # straight onto a track, no menu
//! cargo run --release -p openmx -- --ride --seconds 60   # ride for a minute and quit
//! cargo run --release -p openmx -- --pad /dev/input/js1 --size 512x384 --scale 2
//! cargo run --release -p openmx -- --record context/tapes/spin.tape   # write the pad down
//! cargo run --release -p openmx -- --play context/tapes/spin.tape     # and ride it back
//! ```
//!
//! No emulator, no fixture. Everything it shows is read at runtime from the user's own disc — the ISO
//! named in `kit.toml`, the track's `.res`, its scene, its terrain, the models the scene places and
//! their textures, and the menus' art — and nothing is written anywhere except the one frame `--shot`
//! asks for, which goes under `context/` and is not committed, and the front end's two settings
//! files beside the disc.
//!
//! # The front end
//!
//! With no flag that picks a ride (`--track`, `--play`, `--record`, `--moment`, `--vehicle`, `--phy`, `--set`, `--garage`,
//! any `--trace`, `--players`, `--headless`, `--frames`, or `--ride`) the shell opens on its main menu ([`menu`], P36): Free Ride and its
//! track pick, the Garage (P37: every seat's bike, set and rider), Settings with the rider's name on an on-screen keyboard, the overlay, wrecks and the
//! throttle's wiring, and Quit. Start or Escape in a ride opens the pause — Resume, Restart, Settings,
//! Main Menu, Quit — in place of the plain freeze Start is without the front end. `--menu` asks for
//! the front end whatever else is given, and `--screen NAME` opens it on one screen, which is how a
//! `--shot` of each is taken. The settings are kept in `settings.txt` and `controls.txt` beside the
//! disc; a flag given on the command line wins over the file for the session.
//!
//! The main menu has a press-start splash before it (P46): whichever device presses Cross or Start
//! (Enter or Space on the keyboard) is Player 1, the one device that drives the menus — with the
//! keyboard, which always may — and the first seat; an idle or virtual pad moves nothing. Circle or
//! Escape there asks to quit, Back on the main menu goes back to it, and so does 1P's pad being
//! pulled out.
//!
//! # What is the port and what is the shell
//!
//! The shell is the thin part. Each piece of the frame below is a proved port, and this file only wires
//! them together in the order the game's own loop uses:
//!
//! * **The pad** is I1 and I2: the host's joystick and keyboard are folded into a DualShock 2 report
//!   ([`pad`]), and the report is read by [`game_sim::input::Manager`] — the game's own poll, dead zone
//!   and press/release edges.
//! * **The frame** is S1: [`game_sim::tick::Engine`] is handed the Timer 0 a PS2 holding 30 fps reads
//!   ([`pace`], P52), and it hands the simulation the same thirty-frame averaged, 30 fps clamped `dt`
//!   the original does. The ticks run 30 a second on the wall clock whatever the picture's rate: a
//!   slow picture runs several before it is drawn, a fast one draws several between two, blended
//!   between them on the drawing side only.
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
//! **Those are the original's controls**, and since P40 they are a setting: the default is a new
//! layout with R2 the gas and L2 the brake, each on its trigger's travel, Select the camera, and Cross
//! and Square kept free for later. Each seat picks one (`--controls new|original`, or one per seat,
//! and the front end's Controls screen). The new one is a remap of the pad's report in [`pad`]
//! ([`pad::Scheme::ride`]) before the engine's pad port sees it, so the simulation still reads the
//! original's controls, a tape holds what it read, and under the original's every replay is as it was.
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
//! Over each view is the original's own HUD ([`overlay`], V26): its speedometer and its radar with
//! every other rider on it, off the disc's art. **H** swaps it for the debug overlay ([`hud`]), which
//! `--hud` (`--overlay debug`) starts on: the sixteen controls as the port's own input manager holds
//! them, both sticks where its dead zone put them, and beside them the numbers the ride is running
//! on. That one is debugging furniture and matches nothing on the disc. `--overlay none`, or the
//! front end's setting, draws neither until H. The original's speed is its digital readout unless
//! `--speedometer analog` or the Settings' Speedometer row asks for its dial (V42).
//!
//! **M** swaps between the bike and the track's biplane, as does the pause's "Switch To Machine" for
//! the player who opened it, and `--vehicle biplane` or the ride setup's machine pick starts in the
//! plane ([`vehicle`], P19, P41): the swap is the original's own — the bike is left where it stands, hidden and
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

mod audio;
mod aspect;
mod chase;
mod gpuwin;
mod ground;
mod hud;
// What the CPU still draws over the GPU's picture (P44), as patches.
mod layer;
mod lobby;
// The join screen (P38's rules), which the front end's local game seats its players through (P43).
mod join;
mod menu;
mod moment;
mod net;
mod online;
mod overlay;
mod pace;
mod pad;
mod particles;
mod perf;
// N8: the one-seat online ride against the offline one, tick for tick.
#[cfg(test)]
mod same_ride;
mod split;
mod stages;
// The garage (P37). Its screen hangs off the front end (P36), which is not in this tree yet; until it
// is, the `.phy` lookup is the only part of it the shell calls.
#[allow(dead_code)]
mod roster;
mod screen;
mod vehicle;
mod window;

use std::sync::Arc;
use std::time::{Duration, Instant};

use game_render::biplane::{self, Biplane};
use game_render::chassis::{grips as chassis_grips, placed as chassis_at, wrecked as chassis_wrecked};
use game_render::disc::{Disc, Res};
use game_render::mat::{M4, V3, v3};
use game_render::raster::Frame;
use game_render::rider::{Body, Sex, Skeleton, anchors_of};
use game_render::scene::Vehicle;
use game_render::vehicle::{Chassis, Drive, params as vehicle_params};
use game_render::gpu::{Actor, Renderer, Scene3, Target, View, wgpu};
use game_render::world::{Assets, Camera, Graphics, Lighting, draw_scene};
use game_render::{ini, png, scene as view};
use game_sim::biplane::Area;
use game_sim::camera::{Preset, Race, Ridden, Switches};
use game_sim::flight::Flight;
use game_sim::input::{Control, PADS, Report};
use game_sim::phy::{self, Phy};
use game_sim::replay::{Head, PadTape, Recorder, TickPhase};
use game_sim::ride::{self, Controls};
use game_sim::rider::{self, Rider};
use game_sim::rng::Rng;
use game_sim::state::Phase;
use game_sim::tick::{Engine, Game, Outcome};
use game_sim::trick::{Set, Stunts};
use game_sim::world::Place;
use game_sim::{Input, Sim, track, world as sim_world};
use ps2_float::{F, Vf};

use ground::Ground;
use hud::Hud;
use menu::{Act, Art, Dev, Menu, Reader, Readers};
use stages::Stages;
use moment::{Moment, Shot};
use pad::{Bindings, Device, Pads, PlaneScheme, SEATS, Scheme, SeatInput, Seats, Throttle, Wiring};

// `controls.txt` holds a layout for every seat the pad layer has.
const _: () = assert!(menu::SEATS == SEATS);
use perf::Stage;
use split::{Layout, Split};
use vehicle::{Garage, Kind, Lot, Machine, Plane};
use screen::{Backend, Screen};
use window::key;
use aspect::{Aspect, Display};

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
/// How many cells a side of the ground the GPU draws or culls at once (P44).
const GROUND_CHUNK: u32 = 32;

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

/// How far apart the local players stand on the start line, in feet: the port's own (V22), a bike's
/// length or so, so that nobody starts inside anybody else.
const SEAT_SPACING: f32 = 6.0;

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
    // `cargo run -p openmx` without `--release` is the dev profile, opt-level 1: several times
    // slower a frame, which is not what the shell costs (P47).
    if cfg!(debug_assertions) {
        eprintln!("openmx: a debug build, slow to draw; `cargo run --release -p openmx` is the one to ride");
    }
    if let Err(e) = run() {
        eprintln!("openmx: {e}");
        std::process::exit(1);
    }
}

/// Everything the frame needs that is not the renderer's: the ride, the pad and the port's own input
/// manager, once for each local player.
///
/// This is what [`Game`] is implemented on, so the only thing that happens inside a simulation pass is
/// what the game would do in one — reading the pads and moving the world.
struct World<P> {
    /// What the ride asks about the world: the ported terrain query and the scene's collision
    /// placements, swept by S4's own sweep. The *drawn* ground is the loop's own [`Ground`], which
    /// reads the same file for triangles instead. Every player rides the one world. (Any world: the
    /// shell's is that `Place`, N8's test rides a flat floor.)
    place: P,
    /// The local players, the first of them the one a one-player session has always had (V22).
    seats: Vec<Seat>,
    /// `--record`: the tape this session is being written to, one frame per simulation pass. It is
    /// the first player's pad: a tape holds the two ports a PS2 has, and the first is that player's.
    tape: Option<Recorder>,
    /// `--play`: the frame length the tape says this pass is, which replaces the one the engine
    /// measured. The recorded session simulated with the tape's own `dt`, so a playback that used the
    /// wall clock would reproduce the buttons and not the ride.
    played_dt: Option<F>,
}

/// One local player's half of a simulation pass: their pad, their ride and their machine.
struct Seat {
    /// This frame's reports, written by the app layer before the step the way the original's device
    /// manager fills its buffers before the frame reads them. The player's own pad is the first.
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
    /// Whether Start freezes the ride here. Under the front end it opens the pause instead, which
    /// the loop reads off the menu, so the pass leaves it alone.
    start_freezes: bool,
    /// Whether this player's pad was pulled out and their seat waits for it (P35). A waiting player
    /// is paused — not polled, not stepped — while every other player rides on; nothing else in the
    /// world waits on them.
    waiting: bool,
    /// What the rider did this frame, kept so the loop can print it after the pass.
    said: rider::Frame,
    /// Whether the plane was upside down when this frame's report was made, which turns the
    /// biplane's stick round under the screen's biplane controls (P50, `pad::PlaneScheme::fly`).
    inverted: bool,
    /// The hands this pass was stepped with, kept for the body's own animation frame — which runs
    /// after the pass, off the same controls and the same `dt` (V18).
    hands: Controls,
}

impl Seat {
    /// One player's pass: the pad, then the bike.
    fn pass(&mut self, dt: F, place: &mut dyn sim_world::World) {
        // Nobody's hands on the bars: the pad is not polled either, so the input manager's timers
        // and held buttons are where the pull left them, and the ride goes on from there when the
        // pad comes back.
        if self.waiting {
            self.said = rider::Frame::default();
            return;
        }
        // The game's poll, with the frame's dt: this is what turns report bytes into held controls,
        // press and release edges, and stick axes with the game's own dead zone — and then the
        // rider's hands off it, because which button means what lives in the ride and not here.
        let controls = self.sim.poll(dt, &self.reports);

        if self.start_freezes && self.sim.input.pad(0).pressed(Control::Start) {
            self.riding = !self.riding;
        }

        self.said = rider::Frame::default();
        if self.riding {
            self.said = self.garage.step(&mut self.sim, dt, &controls, place);
        }
        self.hands = controls;
    }
}

/// The swap (P19) for one player, from the M key or the pause's "Switch To Machine" (P41): the
/// controller moves, and the machine taken gets its own camera, which starts where it is rather than
/// lagging in from wherever it was last. The machine now up, or `None` (and no swap) with no plane.
fn switch_machine(seat: &mut Seat, rig: &mut Rig) -> Option<Kind> {
    match seat.garage.swap() {
        Ok(up) => {
            let cam = &mut rig.follow_mut(up).cam;
            cam.snap = true;
            let name = (!cam.presets.is_empty()).then(|| cam.preset().name.clone());
            if let Some(name) = name {
                rig.hud.note_camera(&name);
            }
            rig.hud.note_vehicle(up.name());
            println!("vehicle: {}", up.name());
            Some(up)
        }
        Err(e) => {
            println!("vehicle: {e}");
            None
        }
    }
}

/// What the original's HUD shows of a player's own ride (V26, V28): their odometer, which machine is
/// up, how hard a preload is shoving the bike, and the score. The port keeps no stunt score yet, so
/// the score reads 0 — what the original's shows until a trick lands.
fn gauges(seat: &Seat, rig: &Rig, speedometer: menu::Speedometer) -> overlay::Gauges {
    let (air, p) = (&seat.sim.rider.ride.air, &seat.sim.phy.air);
    overlay::Gauges {
        feet: rig.odometer.feet,
        bike: seat.garage.up == Kind::Bike,
        preload: overlay::preload_fill(
            air.preload_firing,
            air.preload_force.y(),
            p.max_preload.0,
            air.preload_timer.0,
            p.preload_time.0,
        ),
        score: 0,
        modified: rig.modified,
        speedometer,
    }
}

/// One local player's half of the picture (V22): the body and bike posed for them, the cameras that
/// follow them, their overlay, and the models their pose is skinned into — the first player's are the
/// ones the archives loaded, every other player's a copy under a name of its own.
struct Rig {
    body: Option<Body>,
    chassis: Option<Chassis>,
    camera: Race,
    plane_camera: Option<Race>,
    /// The camera the screen switches to while this player's rider is down (V31).
    wreck_camera: chase::WreckCamera,
    hud: Hud,
    /// The distance this player's odometer shows, under the original's overlay (V26).
    odometer: overlay::Odometer,
    bike_model: String,
    body_model: Option<String>,
    /// What bolts onto this player's chassis, each part under the model name it is drawn from: the
    /// first player's `.sub`, or the garage pick's own bike's (P37).
    parts: Vec<game_render::scene::Part>,
    /// A game modifier reaches this player's ride (P55): the overlay says it is modified.
    modified: bool,
}

impl Rig {
    /// The camera the machine `up` is followed by: the plane's own, if this player has one, or the
    /// bike's.
    fn follow(&self, up: Kind) -> &Race {
        match (up, &self.plane_camera) {
            (Kind::Biplane, Some(cam)) => cam,
            _ => &self.camera,
        }
    }

    fn follow_mut(&mut self, up: Kind) -> &mut Race {
        match (up, &mut self.plane_camera) {
            (Kind::Biplane, Some(cam)) => cam,
            _ => &mut self.camera,
        }
    }
}

/// What the picture shows of one seat at a tick (P52), kept from the tick before the last so a drawn
/// frame can blend it into the last.
#[derive(Clone)]
struct Shown {
    /// Where the bike is: where the wreck body has taken it while the rider is down (S53), and
    /// where the ride has it otherwise.
    bike: M4,
    /// The chassis' node pose, empty before the chassis' first frame.
    chassis: Vec<M4>,
    /// The body's pose as the GPU draws it — thrown by the ragdoll's bones while the rider is down,
    /// on the bike otherwise — or none: not posed yet, no body, or the software path, which skins at
    /// the draw.
    body: Option<Vec<M4>>,
}

impl Shown {
    fn of(seat: &Seat, rig: &Rig, assets: &Assets, ragdolled: Option<&game_render::rider::Ragdolled>, poses: bool) -> Shown {
        let bike = match &seat.sim.rider.crash {
            Some(crash) => chassis_wrecked(crash),
            None => chassis_at(&seat.sim.rider.ride),
        };
        if !poses {
            return Shown { bike, chassis: Vec::new(), body: None };
        }
        let chassis = rig.chassis.as_ref().map_or(Vec::new(), |c| c.world.clone());
        let down = match (seat.sim.rider.drawn_ragdoll(), &seat.sim.rider.ragdoll_kit, ragdolled) {
            (Some(doll), Some(kit), Some(rigged)) => Some((doll.nodes(kit), rigged)),
            _ => None,
        };
        let body = match (&rig.body, &down) {
            (Some(body), Some((bones, rigged))) => Some(body.ragdoll_palette(rigged, bones, bike)),
            (Some(body), None) if body.posed => {
                let grips = match &rig.chassis {
                    Some(chassis) => chassis_grips(chassis),
                    None => match assets.models.get(CHASSIS) {
                        Some(loaded) => anchors_of(&Skeleton::of(&loaded.model), &[]),
                        None => Default::default(),
                    },
                };
                Some(body.palette(Some(&grips)).0)
            }
            _ => None,
        };
        Shown { bike, chassis, body }
    }

    /// From this, the tick before the last, to `now`, the last.
    fn blend(&self, now: &Shown, by: &pace::Blend) -> Shown {
        Shown {
            bike: by.at(self.bike, now.bike),
            chassis: by.pose(&self.chassis, &now.chassis),
            body: match (&self.body, &now.body) {
                (Some(was), Some(is)) => Some(by.pose(was, is)),
                (_, is) => is.clone(),
            },
        }
    }
}

/// Where the plane is drawn: at its body's pose, nose and up off the plane's own measure.
fn plane_at(plane: &Plane) -> M4 {
    M4::placement(vector(plane.at), vector(plane.facing), vector(plane.up))
}

impl<P: sim_world::World> Game for World<P> {
    /// One simulation pass: each player's pad, then their bike, in player order.
    ///
    /// `input` is the engine's own sampled pad, which this shell does not use — the full 32-byte
    /// reports are in each seat and the port's manager is what reads them. Nothing here draws
    /// from `rng`: the only thing that moves is the bikes, and the riders decide where they go.
    /// The riders do not touch each other (the original's rider-on-rider wreck is not ported), so
    /// the order the seats run in does not reach any of them.
    fn simulate(&mut self, dt: F, _input: Input, _rng: &mut Rng) -> bool {
        // Riding a tape: the frame is as long as the tape says it was, not as long as this machine
        // took to draw the last one. Everything below reads this `dt` and nothing reads the engine's.
        let dt = self.played_dt.unwrap_or(dt);
        // Writing one: the pass's own `dt` and the reports it is about to read, which is what a
        // replay of this tape will be handed (`game_sim::replay::Recorder`).
        // A tape is the first player's, so a pass where they wait for their pad is not on it: the
        // frames it holds are the passes the first player rode, with no hole where they were paused.
        if let Some(tape) = &mut self.tape
            && !self.seats[0].waiting
        {
            tape.frame(dt, &self.seats[0].reports);
        }
        for seat in &mut self.seats {
            seat.pass(dt, &mut self.place);
        }
        true
    }
}

/// What the shell was asked to do.
#[derive(Clone)]
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
    /// `--overlay original|debug|none` (V26): which overlay a ride draws, the original's HUD by
    /// default. Unset, the front end's setting decides; `--hud` is `--overlay debug`.
    overlay: Option<menu::Overlay>,
    /// `--speedometer digital|analog` (V42): how the original's HUD shows the speed. Unset, the
    /// front end's setting decides, and without one the digital readout.
    speedometer: Option<menu::Speedometer>,
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
    /// `--modifiers TEXT` (P55): the game modifiers the ride loads with, as `settings.txt` spells
    /// them (`gravity=50 bike_speed.1=200`). The original game's by default; under the front end the
    /// Settings screen's unless this asked for others. A `--play` tape rides with its own, and a
    /// lobby's ride with its host's.
    modifiers: game_sim::modifiers::Mods,
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
    /// `--pads k,0,1,-,…`: what drives each seat (P35) — the keyboard, a pad by the order it was
    /// found, or nobody — or `None` for pads in join order with the keyboard beside 1P's.
    pads: Option<[Option<pad::Seat>; SEATS]>,
    /// `--bindings FILE` (or `--controls FILE`): the rebindable buttons, `context/controls.txt` when
    /// not said.
    controls: Option<std::path::PathBuf>,
    /// Whether `--controls new|original` was given, so the front end's `controls.txt` does not
    /// override it.
    scheme_flag: bool,
    /// Whether `--biplane-controls new|original` was given (P50), for the same reason.
    plane_flag: bool,
    /// Whether `--preset N` was given (P79), for the same reason.
    preset_flag: bool,
    /// `--vehicle bike|biplane`: which machine the session starts on (P19). The bike by default,
    /// and a default session is the shell exactly as it was before there was a choice.
    vehicle: Kind,
    /// `--players N`: how many local players share the screen, one view each (V22). One by default,
    /// and one is the shell exactly as it was before there was a split.
    players: usize,
    /// `--split h|v`: which of the original's two layouts two players get.
    split: Split,
    /// `--aspect fill|4:3`: the frame at the window's own shape, or the largest 4:3 one inside it
    /// with bars round it, which is the shell as it was before V24.
    aspect: Aspect,
    /// `--headless`: no window and no host pad — every seat rides an idle pad, or the first rides
    /// `--play`'s tape — and the frame is drawn all the same, for `--shot` and for the tests.
    headless: bool,
    /// `--frames N`: quit after N frames, which unlike `--seconds` is the same run on any machine.
    frames: Option<u32>,
    /// `--draw-fps N`: draw at most N pictures a second. Unset, a window draws as fast as its
    /// display takes them and a headless ride draws 60. The ride's ticks are 30 a second either way
    /// (P52).
    draw_fps: Option<f32>,
    /// `--menu`: the front end, whatever else is asked.
    menu: bool,
    /// Whether a flag picked a ride, which skips the front end unless `--menu` says otherwise.
    ride_flag: bool,
    /// `--screen`: the front end's screen to open on.
    screen: Option<menu::Screen>,
    /// Which wiring flags were given, so the front end's `controls.txt` fills in only the others.
    wiring_flags: (bool, bool),
    /// `--online NAME@HOST:PORT`, `--peer NAME@HOST:PORT` (each other machine) and `--seats
    /// NAME,NAME,-,...`: ride online (N2, [`net`]) as `NAME`, from that address, with these seats.
    online: Option<String>,
    peers: Vec<String>,
    seats: Option<String>,
    /// `--set WORD` (`race250a`) or the garage's pick (P37): which of the bike archive's sixteen
    /// `.phy` names the first player rides, the freeride's `Race125P` when neither says.
    set: Option<roster::Set>,
    /// The garage's picks for the second player on, each ridden on its own bike's `.phy` and in its
    /// own rider's gear. A player past the end rides the first player's.
    others: Vec<roster::Pick>,
    /// `--garage`: ride `garage.txt`'s picks without the front end, every seat its own.
    garage: bool,
    /// `--fps`: a line a second with the frame rate, the slow frames and each stage's time ([`perf`]).
    fps: bool,
    /// `--renderer gpu|software`: what draws the picture (P44); the GPU unless asked.
    renderer: Backend,
    /// `--graphics original|full` (V32): the original's fog and far clip, or the whole map with a
    /// haze; the front end's setting when not given. Only the GPU draws `full`.
    graphics: Option<Graphics>,
    /// `--render-scale auto|F`: the fraction of the frame the GPU draws the 3D at (V30); `None` is
    /// auto, picked from the frames' time in a window and whole offscreen.
    render_scale: Option<f32>,
    /// Whether `--render-scale` was given, `auto` included (V34): if so it wins over the settings'.
    render_scale_asked: bool,
    /// The pad seat each player reads, the front end's local game's (P43): player i rides seat
    /// `pad_of[i]`'s controller. Empty is seat i for player i, which is every ride outside one.
    pad_of: Vec<usize>,
    /// `--no-sound`: no sound and no music (P42). A headless ride has none either.
    sound: bool,
    /// `--timings`: a line a second of what each stage of the frame cost (P47, [`stages`]), and
    /// one over the whole session at the end.
    timings: bool,
}

impl Args {
    /// The least time a picture takes (P52): `--draw-fps`', a headless ride's 60 a second, or none.
    fn draw_frame(&self) -> Option<Duration> {
        match self.draw_fps {
            Some(fps) => Some(Duration::from_secs_f32(1.0 / fps)),
            None if self.headless => Some(FRAME),
            None => None,
        }
    }

    /// The pad seat player `i` reads.
    fn pad_seat(&self, i: usize) -> usize {
        self.pad_of.get(i).copied().unwrap_or(i)
    }
}

/// The garage's picks on a ride (P37): the first seat's bike, set and rider are the ride's own, and
/// every other seat's go to that seat.
fn apply_picks(args: &mut Args, picks: &[roster::Pick]) {
    let Some((first, rest)) = picks.split_first() else { return };
    args.bike = first.bike.clone();
    args.rider = first.rider.clone();
    args.set = Some(first.set);
    args.others = rest.to_vec();
}

fn run() -> Result<(), String> {
    let Some(args) = parse_args()? else {
        return Ok(());
    };
    if args.menu || !args.ride_flag {
        return front_end(args);
    }
    let mut args = args;
    if args.garage {
        let mut disc = Disc::open_configured()?;
        let catalogue = roster::Catalogue::read(&mut disc)?;
        let dir = menu::store_dir(Disc::configured_path().ok().as_deref());
        println!("garage: picks from {}", dir.join(roster::GARAGE_FILE).display());
        let menu = Menu::new(Default::default(), Default::default()).with_garage(catalogue, roster::Picks::load(&dir));
        if let Some(picks) = menu.ride_picks(args.players) {
            apply_picks(&mut args, &picks);
        }
    }
    ride(args, None).map(|_| ())
}

/// The window, the pad and the menu, which outlive a ride under the front end.
struct Shell {
    /// Always open under the front end; an `Option` because a ride without one may be headless.
    window: Option<Screen>,
    seats: Option<Seats>,
    front: Front,
}

/// The front end's own half of [`Shell`].
struct Front {
    menu: Menu,
    /// The pause's reader, of 1P's seat, and the keyboard's beside it when a pad is 1P (P46).
    reader: Reader,
    /// The other seats' readers, for the garage's columns (P37).
    seat_readers: Vec<Reader>,
    keys_reader: Reader,
    /// The menus' readers, a device each, so the splash can tell who pressed (P46).
    readers: Readers,
    art: Art,
    /// Where the settings files are.
    dir: std::path::PathBuf,
    /// The wiring the command line asked for, which the file's fills in around.
    args_wiring: Wiring,
    wiring_flags: (bool, bool),
    scheme_flag: bool,
    plane_flag: bool,
    preset_flag: bool,
    /// Online play's network (N3), started when the online screens first ask for it.
    network: lobby::Network,
    /// The sound and the music (P42), which play on across the menus and the rides.
    sound: audio::Speakers,
}

impl Front {
    /// The pad's wiring: the command line's where it was given, `controls.txt`'s otherwise.
    fn wiring(&self) -> Wiring {
        let mut w = self.args_wiring.clone();
        if !self.wiring_flags.0 {
            w.throttle = match self.menu.controls.throttle {
                menu::Gas::Cross => Throttle::Cross,
                menu::Gas::R2 => Throttle::Trigger(Control::R2),
                menu::Gas::L2 => Throttle::Trigger(Control::L2),
            };
        }
        if !self.wiring_flags.1 {
            match Wiring::parse_pressures(&self.menu.controls.pressure) {
                Ok(map) => w.pressure = map,
                Err(e) => println!("{}: pressure: {e}", menu::CONTROLS_FILE),
            }
        }
        w.nintendo_positions = self.menu.controls.nintendo_positions;
        if !self.scheme_flag {
            for (scheme, layout) in w.schemes.iter_mut().zip(self.menu.controls.layouts) {
                *scheme = match layout {
                    menu::Layout::New => Scheme::New,
                    menu::Layout::Original => Scheme::Original,
                };
            }
        }
        if !self.preset_flag {
            w.presets = self.menu.controls.presets;
        }
        if !self.plane_flag {
            for (plane, layout) in w.planes.iter_mut().zip(self.menu.controls.planes) {
                *plane = match layout {
                    menu::Layout::New => PlaneScheme::New,
                    menu::Layout::Original => PlaneScheme::Original,
                };
            }
        }
        w
    }

    /// The settings' music pick and volumes, into the sound (P42), and what the pause's song rows
    /// asked of the music (P56).
    fn hear(&mut self) {
        let s = &self.menu.settings;
        self.sound.apply(&s.music, levels(s));
        for tune in self.menu.take_tunes() {
            match tune {
                menu::Tune::Play(id) => self.sound.play_now(&id),
                menu::Tune::Skip => self.sound.skip(),
            }
        }
    }

    /// Write the settings when the menu changed them, saying so if it could not.
    fn save(&mut self) {
        if self.menu.take_changed() {
            match menu::save(&self.dir, &self.menu.settings, &self.menu.controls) {
                Ok(()) => println!("settings: saved in {}", self.dir.display()),
                Err(e) => {
                    println!("settings: {e}");
                    self.menu.note = Some(format!("Could not save the settings: {e}"));
                }
            }
        }
    }
}

/// The seats (P35): every pad plugged in (or only `pad`), the keyboard as one of them, and the
/// buttons as `controls.txt` binds them. The file is the player's and not game data, but it sits
/// beside the disc's other per-machine files under `context/` unless it is put somewhere else. It is
/// not the front end's `controls.txt` beside the disc, which holds the throttle's wiring.
fn open_seats(args: &Args, pad: Option<&str>, wiring: Wiring) -> Seats {
    let controls = args.controls.clone().unwrap_or_else(|| "context/controls.txt".into());
    let bindings = match controls.parent().is_none_or(|dir| dir.as_os_str().is_empty() || dir.is_dir()) {
        true => {
            let (bindings, said) = Bindings::load(&controls);
            for line in said {
                println!("controls: {line}");
            }
            bindings
        }
        false => Bindings::default(),
    };
    Seats::open(pad, args.pads, wiring, bindings)
}

/// The sound (P42): the music sources — the disc's songs, then the user's own folder beside the
/// settings — and the player it all goes out through, set as `settings` says. Silent with
/// `--no-sound` or `--headless`, and where the desktop has no player.
fn start_sound(args: &Args, disc: &mut Disc, dir: &std::path::Path, settings: &menu::Settings) -> audio::Speakers {
    let image = Disc::configured_path().ok();
    let sources = audio::sources(image.map(|path| (disc, path)), dir);
    // Which song a shuffle starts on is the host's business and nothing of the game's: the clock.
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64);
    let music = audio::Music::new(sources, seed);
    println!(
        "music: {} songs from {}; your own .wav files go in {}",
        music.tracks().len(),
        music.sources().join(" and "),
        audio::folder(dir).display()
    );
    let speakers = audio::Speakers::start(music, args.sound && !args.headless);
    speakers.apply(&settings.music, levels(settings));
    speakers
}

/// The settings' sliders as the sound takes them (P56).
fn levels(s: &menu::Settings) -> audio::Levels {
    audio::Levels {
        music: s.music_volume,
        folder: s.folder_volume,
        effects: s.effects_volume,
        crowd: s.crowd_volume,
        engine: s.engine_volume,
        menu: s.menu_volume,
    }
}

/// How a ride under the front end ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Leave {
    Quit,
    Menu,
    Restart,
}

/// The front end (P36): the menus until one of them starts a ride, the ride, and back.
fn front_end(args: Args) -> Result<(), String> {
    let mut disc = Disc::open_configured()?;
    let art = Art::load(&mut disc)?;
    let catalogue = match roster::Catalogue::read(&mut disc) {
        Ok(c) => c,
        Err(e) => {
            println!("garage: {e}");
            roster::Catalogue::default()
        }
    };
    let dir = menu::store_dir(Disc::configured_path().ok().as_deref());
    let (settings, controls) = menu::load(&dir);
    println!("front end: settings in {}", dir.display());
    let sound = start_sound(&args, &mut disc, &dir, &settings);
    drop(disc);
    let picks = roster::Picks::load(&dir);
    println!(
        "garage: {} bikes, {} riders; seat 1 rides {} on {} as {}",
        catalogue.bikes.len(),
        catalogue.riders.len(),
        picks.0[0].bike,
        picks.0[0].set.word(),
        picks.0[0].rider
    );
    let pad = args.pad.clone().or_else(|| controls.pad.clone());
    let network = lobby::Network::new(&dir);
    let mut front = Front {
        menu: Menu::new(settings, controls).with_garage(catalogue, picks),
        reader: Reader::new(),
        seat_readers: (0..roster::SEATS).map(|_| Reader::new()).collect(),
        keys_reader: Reader::new(),
        readers: Readers::new(),
        art,
        dir,
        args_wiring: args.wiring.clone(),
        wiring_flags: args.wiring_flags,
        scheme_flag: args.scheme_flag,
        plane_flag: args.plane_flag,
        preset_flag: args.preset_flag,
        network,
        sound,
    };
    front.menu.music_choices = front.sound.choices();
    // Online, this machine's player rides the freeride's bike set unless the lobby changes it, as
    // the rider the command line names.
    front.menu.lobby.bikes = lobby::bikes();
    front.menu.lobby.pick = openmx_net::Pick::new(&args.rider, &roster::Set::FREERIDE.word());
    front.menu.lobby.lan = front.network.online().relay == openmx_net::Relay::Off;
    let window = Screen::open(args.renderer, &args.display, args.size, args.scale, "openmx")?;
    let seats = open_seats(&args, pad.as_deref(), front.wiring());
    // The Controls screen lists the keyboard under each layout (P54), as the bindings file has it.
    // And the original's on each of its controller presets (P79), in `menu::keyboard_at`'s order.
    front.menu.keyboard = std::array::from_fn(|i| match i {
        0 => seats.keyboard_layout(Scheme::New),
        p => seats.keyboard_layout_on(Scheme::Original, p - 1),
    });
    match seats.name(0) {
        Some(name) => println!("pad: {name}"),
        None => println!("pad: none found — the menus are on the keyboard (arrows, Space, Return, Escape)"),
    }
    // The ride setup lists a machine for each player who will ride (P41): a Free Ride's are
    // `--players`, a local game's its join screen's (P43).
    front.menu.riders = args.players;
    front.menu.free_riders = args.players;
    // The splash first (P46), unless `--screen` asks for a screen.
    match args.screen {
        Some(screen) => front.menu.open(screen),
        None => front.menu.splash(),
    }
    let mut shell = Shell { window: Some(window), seats: Some(seats), front };

    let frame_size = |window: &Screen| match args.size {
        Some(size) => size,
        None => args.aspect.frame((window.width, window.height), window.scale),
    };
    let (width, height) = frame_size(shell.window.as_ref().expect("the front end has a window"));
    let mut frame = Frame::new(width, height, [0, 0, 0]);
    // A pause asked for by `--screen` is over a ride, so it starts one on the last track.
    let mut act = match shell.front.menu.screen() {
        Some(menu::Screen::Pause) => Act::Ride(shell.front.menu.settings.track.clone()),
        _ => Act::Stay,
    };
    let mut shot = args.shot.clone();
    let started = Instant::now();
    let mut shown = 0u32;
    // The menu last drawn and the frame size it was drawn at, while it is still on the screen.
    let mut shown_look: Option<(menu::Menu, usize, usize)> = None;
    // The menu as GPU sprites, and whether the window's renderer has the menu's pictures.
    let (mut menu_sprites, mut menu_images) = (Vec::new(), false);
    // What the menu's frames cost before their sleep, for `--timings` (P47).
    let mut busy = Duration::ZERO;
    let mut escape = false;
    loop {
        if let Act::Ride(track) = &act {
            shell.front.reader.reset();
            shell.front.keys_reader.reset();
            loop {
                let mut a = args.clone();
                a.track = track.clone();
                let overlay = args.overlay.unwrap_or(shell.front.menu.settings.overlay);
                a.overlay = Some(overlay);
                a.hud = overlay == menu::Overlay::Debug;
                a.speedometer = Some(args.speedometer.unwrap_or(shell.front.menu.settings.speedometer));
                a.graphics = Some(args.graphics.unwrap_or(shell.front.menu.settings.graphics));
                if !args.render_scale_asked {
                    a.render_scale = shell.front.menu.settings.render_scale.fraction();
                }
                a.wrecks = args.wrecks && shell.front.menu.settings.wrecks;
                if args.modifiers.is_original() {
                    a.modifiers = shell.front.menu.settings.modifiers;
                }
                a.wiring = shell.front.wiring();
                // The ride setup's machine for 1P (P41), unless `--vehicle biplane` asked for the plane
                // whatever the setup says. The other seats' picks are read where their garages are.
                if args.vehicle == Kind::Bike {
                    a.vehicle = Kind::parse(shell.front.menu.settings.machines[0].word())?;
                }
                // The shot is the menu's unless it is of the pause, which is drawn over the ride.
                a.shot = shot.take_if(|_| shell.front.menu.screen() == Some(menu::Screen::Pause));
                a.seconds = args.seconds.map(|s| (s - started.elapsed().as_secs_f32()).max(0.0));
                // A local game (P43): one player a seat its join screen has in play, each on that seat's
                // controller, drawn in the split layout it picked.
                if let Some(party) = shell.front.menu.party() {
                    a.players = party.players();
                    a.split = party.split;
                    a.pad_of = party.devices.clone();
                    println!(
                        "local game: {} players on pad seats {:?}, {}",
                        a.players,
                        party.devices.iter().map(|d| d + 1).collect::<Vec<_>>(),
                        party.layout().name()
                    );
                }
                // What every seat picked in the garage is what the ride loads (P37).
                if let Some(picks) = shell.front.menu.ride_picks(a.players) {
                    apply_picks(&mut a, &picks);
                }
                shell.front.note_clear();
                match ride(a, Some(&mut shell)) {
                    Ok(Leave::Restart) => continue,
                    Ok(Leave::Menu) => break,
                    Ok(Leave::Quit) => return Ok(()),
                    Err(e) => {
                        println!("ride: {e}");
                        shell.front.menu.note = Some(format!("Could not ride {track}: {e}"));
                        break;
                    }
                }
            }
            shell.front.menu.main();
            shell.front.reader.reset();
            shell.front.readers.reset();
            shell.front.seat_readers.iter_mut().for_each(Reader::reset);
            // The keyboard took the ride over from 1P's pulled-out pad: it is 1P now.
            if let Some(seats) = &shell.seats
                && seats.first() == Some(&Device::Keys)
                && matches!(shell.front.menu.p1(), Some(Dev::Pad(_)))
            {
                shell.front.menu.set_p1(Dev::Keys);
                shell.front.menu.p1_name = dev_name(seats, &Dev::Keys);
            }
        }
        // The lobby's ride (N3): on its track and its wrecks, over the lobby's connection, and back to
        // the lobby after.
        if act == Act::Online {
            shell.front.reader.reset();
            shell.front.keys_reader.reset();
            if let Some((room, _)) = shell.front.menu.lobby.start.clone() {
                let mut a = args.clone();
                a.track = room.track.clone();
                let overlay = args.overlay.unwrap_or(shell.front.menu.settings.overlay);
                a.overlay = Some(overlay);
                a.hud = overlay == menu::Overlay::Debug;
                a.speedometer = Some(args.speedometer.unwrap_or(shell.front.menu.settings.speedometer));
                a.graphics = Some(args.graphics.unwrap_or(shell.front.menu.settings.graphics));
                if !args.render_scale_asked {
                    a.render_scale = shell.front.menu.settings.render_scale.fraction();
                }
                a.wrecks = room.wrecks;
                a.wiring = shell.front.wiring();
                a.shot = None;
                a.seconds = args.seconds.map(|s| (s - started.elapsed().as_secs_f32()).max(0.0));
                shell.front.note_clear();
                match ride(a, Some(&mut shell)) {
                    Ok(Leave::Quit) => return Ok(()),
                    Ok(_) => {}
                    Err(e) => {
                        println!("ride: {e}");
                        shell.front.network.abandon(&mut shell.front.menu);
                        shell.front.menu.note = Some(format!("Could not ride {}: {e}", room.track));
                    }
                }
            }
            shell.front.menu.back_from_online();
            shell.front.reader.reset();
            shell.front.readers.reset();
        }

        let top = Instant::now();
        let window = shell.window.as_mut().expect("the front end has a window");
        window.poll();
        if window.closed() {
            break;
        }
        if window.resized() {
            let (width, height) = frame_size(window);
            if (width, height) != (frame.width, frame.height) {
                frame = Frame::new(width, height, [0, 0, 0]);
            }
        }
        // The menus are 1P's and the keyboard's (P46): every device read on its own, so the splash
        // can tell who pressed, and past it only those two move anything.
        let seats = shell.seats.as_mut().expect("the front end has seats");
        // A key's state for the frame is down or tapped since the last one, and a pad's report holds
        // its presses since the last frame too, which come back beside it (P53): a press and release
        // inside one slow frame — 5 fps at 4K — is still a press.
        let devices = seats.devices(&|code| window.pressed(code));
        for line in seats.said() {
            println!("{line}");
        }
        let plugged: Vec<String> = devices
            .iter()
            .filter_map(|(d, _, _)| match d {
                Device::Pad(id) => Some(id.clone()),
                Device::Keys => None,
            })
            .collect();
        if shell.front.menu.lost(&plugged) {
            println!("1P: the pad is gone — the splash is back for a new Player 1");
            shell.front.menu.note = Some("Player 1's controller was unplugged".to_string());
            seats.set_first(None);
        }
        // The splash says who it hears and who it skips (P53), so a wrong 1P shows.
        shell.front.menu.heard = devices
            .iter()
            .map(|(d, _, _)| match d {
                Device::Keys => dev_name(seats, &Dev::Keys),
                Device::Pad(id) => dev_name(seats, &Dev::Pad(id.clone())),
            })
            .collect();
        shell.front.menu.skipped = seats.skipped().to_vec();
        let reports: Vec<(Dev, Report, u16)> = devices
            .into_iter()
            .map(|(d, r, taps)| match d {
                Device::Keys => (Dev::Keys, r, taps),
                Device::Pad(id) => (Dev::Pad(id), r, taps),
            })
            .collect();
        let mut presses = shell.front.readers.read_taps(&reports);
        // Escape is the keyboard's Back, on its edge: on the main menu the splash, on the splash
        // the Quit popup.
        let esc = window.pressed(key::ESCAPE);
        if esc
            && !escape
            && let Some((_, p)) = presses.iter_mut().find(|(d, _)| *d == Dev::Keys)
        {
            p.back = true;
        }
        escape = esc;
        // The garage hears every seat: a column for each up to the last one somebody sits in, each
        // walked by the device in that seat (P37) — read from P46's devices, a frame being one or the
        // other of `Seats::devices` and `Seats::frame`.
        let seat_dev = |i: usize| match &seats.slots.held[i] {
            Some(id) => Some(Dev::Pad(id.clone())),
            None if seats.slots.keys() == Some(i) => Some(Dev::Keys),
            None => None,
        };
        let seated = (0..roster::SEATS).rposition(|i| seat_dev(i).is_some() || seats.slots.waiting(i));
        shell.front.menu.seated = seated.map_or(1, |i| i + 1);
        // The join screen's controllers (P43) are the pad seats: which are in, and what each is.
        let present: Vec<usize> = (0..roster::SEATS).filter(|&i| seat_dev(i).is_some()).collect();
        shell.front.menu.present(&present);
        shell.front.menu.devices = (0..roster::SEATS)
            .map(|i| match seats.name(i) {
                Some(name) => name.to_string(),
                None if seats.slots.keys() == Some(i) => "Keyboard".to_string(),
                None => format!("Pad {}", i + 1),
            })
            .collect();
        for (i, reader) in shell.front.seat_readers.iter_mut().enumerate().skip(1) {
            let (report, taps) = seat_dev(i)
                .and_then(|d| reports.iter().find(|(r, _, _)| *r == d))
                .map_or((Report::idle(), 0), |(_, r, taps)| (*r, *taps));
            let p = reader.read_taps(&report, taps);
            shell.front.menu.step_seat(i, p);
        }
        let before = shell.front.menu.p1().cloned();
        shell.front.menu.music_now = shell.front.sound.now();
        act = shell.front.menu.step_devices(&presses);
        let after = shell.front.menu.p1().cloned();
        if after != before {
            match &after {
                Some(dev) => {
                    let name = dev_name(seats, dev);
                    println!("1P: {name} — the menus and the first seat are its");
                    shell.front.menu.p1_name = name;
                    shell.front.menu.note = None;
                    seats.set_first(Some(match dev {
                        Dev::Keys => Device::Keys,
                        Dev::Pad(id) => Device::Pad(id.clone()),
                    }));
                }
                None => seats.set_first(None),
            }
        }
        if shell.front.menu.take_picks_changed() {
            match shell.front.menu.picks.save(&shell.front.dir) {
                Ok(()) => println!("garage: saved in {}", shell.front.dir.display()),
                Err(e) => {
                    println!("garage: {e}");
                    shell.front.menu.note = Some(format!("Could not save the garage: {e}"));
                }
            }
        }
        if shell.front.menu.changed() {
            seats.set_wiring(shell.front.wiring());
            shell.front.hear();
            shell.front.save();
        }
        if shell.front.network.pump(&mut shell.front.menu) == Act::Online {
            act = Act::Online;
        }
        if act == Act::Quit {
            break;
        }
        if matches!(act, Act::Ride(_) | Act::Online) {
            (shown_look, menu_images) = (None, false);
            continue;
        }
        // On the GPU the menu is sprites over its pictures, uploaded once (V30); the CPU draws it
        // for the X window, and for a shot.
        let shooting = shot.is_some() && shown >= args.shot_frame;
        let look = (shell.front.menu.look(), frame.width, frame.height);
        if let Some(w) = window.gpu().filter(|_| !shooting) {
            if !menu_images {
                w.renderer.set_images(&shell.front.art.images());
                menu_images = true;
            }
            menu_sprites.clear();
            shell.front.art.sprites(&shell.front.menu, (frame.width, frame.height), 0, &mut menu_sprites);
            w.draw(frame.width, frame.height, |r, t| r.draw_sprites(t, &menu_sprites))?;
            shown_look = None;
        } else if shown_look.as_ref() != Some(&look) {
            // Drawn again only when it looks different (V30): at 4K a menu is most of a tenth of a
            // second to draw and send.
            frame.color.fill(0);
            shell.front.art.draw(&shell.front.menu, &mut frame);
            window.present(&frame.color, frame.width, frame.height)?;
            shown_look = Some(look);
        } else {
            window.present_again(&frame.color, frame.width, frame.height)?;
        }
        if let Some(path) = shot.take_if(|_| shown >= args.shot_frame) {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            png::write_rgb(&path, frame.width, frame.height, &frame.color)?;
            println!("wrote {}", path.display());
        }
        shown += 1;
        busy += top.elapsed();
        if let Some(limit) = args.seconds {
            if started.elapsed().as_secs_f32() >= limit {
                break;
            }
        }
        if let Some(rest) = FRAME.checked_sub(top.elapsed()) {
            std::thread::sleep(rest);
        }
    }
    if args.timings {
        let ms = busy.as_secs_f64() * 1000.0 / shown.max(1) as f64;
        println!("front end: {shown} frames, {ms:.2} ms a frame before the pace");
    }
    Ok(())
}

/// What the splash calls a device: the keyboard, or a pad by its name without its node.
fn dev_name(seats: &Seats, dev: &Dev) -> String {
    match dev {
        Dev::Keys => "Keyboard".to_string(),
        Dev::Pad(id) => {
            let name = seats.device_name(id).unwrap_or(id);
            name.split(" (/").next().unwrap_or(name).to_string()
        }
    }
}

impl Front {
    /// A note on the ticker lasts until the next ride.
    fn note_clear(&mut self) {
        self.menu.note = None;
    }
}

/// One ride: the old shell, start to finish. Under the front end `shell` is its window, pad and
/// menu, and the ride ends when the pause says so; without one it opens its own and runs until
/// Escape, the window closing, `--seconds` or the tape's end.
fn ride(args: Args, shell: Option<&mut Shell>) -> Result<Leave, String> {
    // ---- the game modifiers (P55) ----------------------------------------------------------------
    // Applied to what the ride loads, never as a branch in the simulation: at their defaults nothing
    // is touched at all.
    let lobby_room = shell.as_deref().and_then(|s| s.front.menu.lobby.start.as_ref()).map(|(room, _)| room);
    let mods = ride_modifiers(&args, lobby_room)?;
    if !mods.is_original() {
        println!("modifiers: {}", mods.text());
    }

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
    let ground = {
        let extent = ground.extent();
        ground.painted(game_render::splat::Splat::load(&archives, &args.track, extent))
    };
    let sim_terrain = track::Terrain::parse(&terrain_bytes)?;
    // The sky the scene names, behind everything (V25); the fog colour where it has none.
    let sky = game_render::sky::Sky::load(&archives, &ini::text(&scn_bytes));
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
        assets.load_placed(&archives, name);
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
        // The garage's lookup (P37), on the seat's picked set, and the freeride's own without one.
        None => {
            let set = args.set.unwrap_or(roster::Set::FREERIDE);
            let (bytes, member, note) = roster::physics(&archives[1], set, &args.bike)?;
            if let Some(note) = note {
                println!("phy: {note}");
            }
            (bytes, member)
        }
    };
    let stock_phy = Phy::read_bytes(&phy_bytes, &tables.ride.curve);
    let mut phy = stock_phy.clone();
    mods.bike(0, &mut phy);
    // The other players' own picks (P37): each rides the `.phy` its set names in its own bike's
    // archive, and wears its own rider's gear. A player the garage has no pick for rides the first's.
    let mut other_phys: Vec<Phy> = Vec::new();
    let mut other_riders: Vec<Option<(String, String)>> = Vec::new();
    // Online every seat of `--seats` is a rider on this track, each machine's in the same places
    // (N5): seat `s` starts where local player `s + 1` would, on the first's bike.
    let riders = match &args.online {
        Some(_) => online::Online::seats(args.seats.as_deref())?.max(args.players),
        None => args.players,
    };
    for i in 1..riders {
        match args.others.get(i - 1) {
            Some(pick) => {
                let res = disc.res(&member_ending(&disc, &format!("{}.res", pick.bike))?)?;
                let (bytes, member, note) = roster::physics(&res, pick.set, &pick.bike)?;
                if let Some(note) = note {
                    println!("player {}: phy: {note}", i + 1);
                }
                println!("player {}: {} on {member}, {}", i + 1, pick.bike, pick.rider);
                let mut phy = Phy::read_bytes(&bytes, &tables.ride.curve);
                mods.bike(i, &mut phy);
                other_phys.push(phy);
                let rider = member_ending(&disc, &format!("{}.res", pick.rider)).ok();
                other_riders.push(rider.map(|m| (pick.rider.clone(), m)));
            }
            None => {
                let mut phy = stock_phy.clone();
                mods.bike(i, &mut phy);
                other_phys.push(phy);
                other_riders.push(None);
            }
        }
    }

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

    // The rider's bones (S53): the riding skeleton the ragdoll is thrown from, read once off the body
    // and its model, and the rider file's parts and pins on top. Every player's body is the same
    // skeleton, so one kit serves them all. Without a body or the rider file a wreck is ridden out as
    // the one point S26 had.
    let ragdolled = match (body.as_mut(), &wreckage) {
        (Some(body), Some((_, rider_file))) => {
            let geometry = body.geometry().to_string();
            assets.models.get(&geometry).and_then(|loaded| body.ragdoll_rig(&loaded.model, rider_file))
        }
        _ => None,
    };
    let ragdoll_kit = match (&ragdolled, &wreckage) {
        (Some(doll), Some((_, rider_file))) => {
            let mut kit = game_sim::ragdoll_ride::Kit::new(rider_file, doll.rig.clone());
            // The wreck bike's collision sphere, which the bones are off the bike past (S65). One
            // file on the disc, looked for by suffix like the wreck bike's own.
            kit.bike = archives
                .iter()
                .find_map(|a| a.read_suffix("wreckbikef.col").ok())
                .and_then(|col| game_sim::ragdoll_ride::bike_sphere(&col));
            Some(Arc::new(kit))
        }
        _ => None,
    };
    match &ragdolled {
        Some(doll) => println!("ragdoll: {} parts off the riding pose", doll.nodes.len()),
        None => println!("ragdoll: none — a wreck is ridden out as one point"),
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
    let chassis = match vehicle.mcf.is_empty() {
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
    //
    // A split screen reads its layout's own copy of every preset (V22): `Preset1h` for two views one
    // above the other, `Preset1v` for two side by side, `Preset1q` for the port's own layouts, the
    // copy the original's loader reads by viewport index. A file without that copy is read as the full
    // screen's, and the shell says so.
    let layout = Layout::for_players(args.players, args.split)?;
    let cpresets = archives.iter().find_map(|a| a.read_suffix("cpresets.ini").ok());
    let presets: Vec<Preset> = cpresets
        .as_ref()
        .map(|b| layout_presets(b, layout, Display::Television, "cpresets.ini"))
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
    // One of each machine per player (P87): the file's two spots for the first two, beside them for
    // the rest, every second copy in the alternate livery. Only the plane is ported, so the lot's
    // other machines are counted and not stood up.
    let lot = {
        let mut probe = sim_world::Ground::new(&sim_terrain, &place.terrain);
        let mut ground = |at: Vf| sim_world::World::ground(&mut probe, Vf::new(at.x(), 1.0e4, at.z(), 1.0)).0.y();
        machines.as_ref().map_or_else(Lot::default, |(_, b)| Lot::new(b, args.players.max(1), &mut ground))
    };
    let flight_for = |seat: usize| {
        plane_phy.as_ref().map(|bytes| {
            let mut plane = phy::Plane::read_bytes(bytes);
            mods.plane(seat, &mut plane);
            let mut flight = Flight::new(&plane);
            let t = &place.terrain;
            flight.area = Some(Area::new(F(t.width as f32), F(t.breadth as f32), t.width_scale, t.breadth_scale));
            flight
        })
    };
    let plane = lot.plane(0, flight_for(0));
    if machines.is_some() {
        let parked: Vec<String> = Machine::ALL
            .iter()
            .map(|&m| {
                let stood = if m.kind().is_some() { "" } else { " (not ported)" };
                format!("{} {}{stood}", lot.of(m).count(), m.section().to_ascii_lowercase())
            })
            .collect();
        println!("machines: {} for {} players", parked.join(", "), args.players.max(1));
    }
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
    let plane_presets = garage.plane.as_ref().and_then(|_| archives.iter().find_map(|a| a.read_suffix(PLANE_PRESETS).ok()));
    let plane_camera = match &garage.plane {
        None => None,
        Some(_) => {
            let presets: Vec<Preset> = plane_presets
                .as_ref()
                .map(|b| layout_presets(b, layout, Display::Television, PLANE_PRESETS))
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
    // Its model and both prop cones, out of the machines' archive, where its textures are too (V21):
    // one model a livery the lot parks (P87), the first copy's plain and the second's the disc's
    // alternate, so a plane is drawn in `plane_drawn[plane.livery]`.
    let plane_drawn: Vec<Biplane> = match (&garage.plane, machine_res) {
        (Some(_), Some(res)) => {
            let own = vec![res];
            let liveries = args.players.clamp(1, vehicle::LIVERIES);
            let drawn: Vec<Biplane> = (0..liveries).map_while(|l| Biplane::load(&mut assets, &own, l)).collect();
            println!("biplane: {} of {liveries} liveries loaded", drawn.len());
            drawn
        }
        _ => Vec::new(),
    };
    // Its particles (P58): the emitter list and its `.emt`s, and the track's particle sheet.
    let (mut plane_fx, mut fx_sheet) = match plane_drawn.first() {
        Some(drawn) => match load_plane_fx(&archives, &args.track, drawn, &assets) {
            Ok((fx, binds, sheet)) => (Some((fx, binds)), Some(sheet)),
            Err(why) => {
                println!("biplane: no particles: {why}");
                (None, None)
            }
        },
        None => (None, None),
    };
    // The software path's own copy (P62): the GPU's is handed over on its first frame, and a GPU
    // that cannot be had leaves the software path drawing.
    let soft_sheet = fx_sheet.clone();

    // The scene's own start line and start direction, as the simulation's reader read them.
    let spawn = point(place.start);
    let facing = point(place.start_direction).norm();
    let at = Vf::new(spawn.x, spawn.y, spawn.z, 1.0);
    let heading = Vf::new(facing.x, facing.y, facing.z, 0.0);
    let mut sim = Place {
        ground: sim_world::Ground::new(&sim_terrain, &place.terrain),
        props,
    };
    // The cars the track's `.toy` drives along their clips (V41): free06's train, its trucks and
    // scrapers, the other freeride tracks' trailers. Drawn, not ridden into: they reach nothing in the
    // simulation, and their clock is the scene's own, summed from the ticks' `dt` below.
    let traffic = game_render::toy::Traffic::load(&archives, &args.track, &mut |p| car_ground(&mut sim, p));
    for (car, _) in &traffic.cars {
        assets.load(&archives, car.stem());
    }
    if !traffic.cars.is_empty() || !traffic.skipped.is_empty() {
        println!("traffic: {} cars on clips, {} left out", traffic.cars.len(), traffic.skipped.len());
    }
    let mut traffic_time = 0.0f64;
    let mut traffic_at: Vec<(String, M4)> = Vec::new();
    // Where the wheels hang off the chassis, which is where each suspension ray starts: the **bike
    // model's own wheel nodes**. `bike_attach_nodes` looks "TireFront" and "TireRear" up in the bike's
    // node by name and writes each one's place, in that node's frame, straight into the wheel's
    // `+0xa0`; "Shocks" and "Handlebars" go the same way into the front wheel's steering axis
    // (S22, `research/oracle.txt`). `BodyTireForcePosition` is a different field of the `.phy`: it
    // lands at the wheel's `+0xb0`, which the suspension never reads: it is the arm the spin step puts
    // the tire's force in on (S58). The original has no fallback here — a bike always has a model — so
    // the one below is the port's own, for a `.phy` whose model has no such node: the stock bike's hubs.
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
        node_at("TireFront").unwrap_or(game_sim::stock::MOUNTS.0),
        node_at("TireRear").unwrap_or(game_sim::stock::MOUNTS.1),
    );
    let new_rider = |phy: &Phy, at: Vf, heading: Vf| -> Rider {
        let mut rider = Rider::new(phy, mounts, at, heading, stunts.clone());
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
        if let Some(kit) = &ragdoll_kit {
            rider.carries_ragdoll(Arc::clone(kit));
        }
        rider
    };
    let mut rider = new_rider(&phy, at, heading);
    mods.ride(0, &mut rider.ride);
    // The other local players (V22), beside the first on the start line: where the original puts a
    // second rider was not read (R9), so this is the port's own — every SEAT_SPACING feet along the
    // start line's own right and left in turn, each stood on the ground under it, all facing the
    // way the first does. The first player is where a one-player session has always started.
    let right = Vf::new(heading.z(), 0.0, -heading.x(), 0.0);
    let (under_first, _) = sim_world::World::ground(&mut sim, at);
    let others: Vec<Rider> = (1..riders)
        .map(|i| {
            let side = if i % 2 == 1 { 1.0 } else { -1.0 };
            let off = side * SEAT_SPACING * i.div_ceil(2) as f32;
            let (x, z) = (at.x() + right.x() * off, at.z() + right.z() * off);
            let (on, _) = sim_world::World::ground(&mut sim, Vf::new(x, 1.0e4, z, 1.0));
            let mut rider = new_rider(&other_phys[i - 1], Vf::new(x, at.y() + (on.y() - under_first.y()), z, 1.0), heading);
            mods.ride(i, &mut rider.ride);
            rider
        })
        .collect();
    for (i, r) in others.iter().enumerate() {
        let p = r.ride.position();
        println!("player {}: starts at {:.0}, {:.0}, {:.0}", i + 2, p.x(), p.y(), p.z());
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
        return moment::draw(&moment, out, &shot, &ground, sky.as_ref(), &assets, &vehicle, &look).map(|()| Leave::Quit);
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
            return Ok(Leave::Quit);
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
        return Ok(Leave::Quit);
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
        let mut recorder = Recorder::new(format!("{} on {} / {}", args.rider, args.track, args.bike));
        // A modified ride's tape carries its modifiers, so it replays on them (P55).
        recorder.tape.modifiers = mods;
        recorder
    });

    // ---- the window and the pad ---------------------------------------------------------------
    // `--headless` opens neither: no window to show the frame in and no host pad to read, so every
    // seat rides an idle pad or the tape, and the frame is drawn all the same for `--shot`. Under the
    // front end (P36) they are its own, opened once and kept across rides; without one, this ride's.
    let mut own: (Option<Screen>, Option<Seats>);
    let (window, pad_seats, mut front) = match shell {
        Some(shell) => (&mut shell.window, &mut shell.seats, Some(&mut shell.front)),
        None => {
            own = match args.headless {
                true => (None, None),
                false => (
                    Some(Screen::open(args.renderer, &args.display, args.size, args.scale, "openmx — freeride")?),
                    Some(open_seats(&args, args.pad.as_deref(), args.wiring.clone())),
                ),
            };
            (&mut own.0, &mut own.1, None)
        }
    };

    // ---- the sound (P42) ----------------------------------------------------------------------
    // The front end's speakers, which play on after the ride; without one, this ride's own, set as
    // the front end's settings file says. Every seat hears its own bike's archive, and every
    // seat's sounds go into the one mix (a split screen is one pair of speakers).
    let mut own_sound = None;
    let mixer = match front.as_deref() {
        Some(f) => f.sound.live(),
        None if args.sound && !args.headless => {
            let dir = menu::store_dir(Disc::configured_path().ok().as_deref());
            let (settings, _) = menu::load(&dir);
            own_sound.insert(start_sound(&args, &mut disc, &dir, &settings)).live()
        }
        None => None,
    };
    let mut sound = match mixer {
        None => None,
        Some(mixer) => {
            let mut bikes = vec![bike_member.clone()];
            for i in 1..args.players {
                let theirs = args.others.get(i - 1).and_then(|p| member_ending(&disc, &format!("{}.res", p.bike)).ok());
                bikes.push(theirs.unwrap_or_else(|| bike_member.clone()));
            }
            let machines = member_ending(&disc, MACHINE_ARCHIVE).unwrap_or_else(|_| MACHINE_ARCHIVE.to_string());
            let read = Disc::open_configured().and_then(|d| {
                audio::Sound::read(
                    mixer,
                    d,
                    &bikes,
                    rider_member.as_deref(),
                    &machines,
                    &track_member,
                    second.as_deref(),
                    &args.track,
                )
            });
            match read {
                Ok(s) => Some(s),
                Err(e) => {
                    println!("sound: {e} — the ride is silent");
                    None
                }
            }
        }
    };
    if let Some(pads) = args.pads {
        println!("seats: {}", pad::Seat::flag(&pads));
    }
    let schemes: [Scheme; SEATS] =
        std::array::from_fn(|i| pad_seats.as_ref().map_or(args.wiring.schemes[i], |s| s.scheme(i)));
    println!("controls: {}", Scheme::list_text(&schemes));
    // Each seat's controller preset (P79), the original layout's pick and preset 0 under the new one.
    // A tape holds the pad and not the table, so a ride on another preset says which, and plays back
    // on it only with the same --preset.
    let presets: [usize; SEATS] = std::array::from_fn(|i| pad_seats.as_ref().map_or(schemes[i].preset(args.wiring.presets[i]), |s| s.preset(i)));
    if presets.iter().any(|p| *p != 0) {
        println!("controller preset: {}", presets.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(","));
    }
    let planes: [PlaneScheme; SEATS] =
        std::array::from_fn(|i| pad_seats.as_ref().map_or(args.wiring.planes[i], |s| s.plane(i)));
    println!("biplane controls: {}", PlaneScheme::list_text(&planes));
    if let Some(seats) = &*pad_seats
        && seats.slots.keys() == Some(0)
        && seats.name(0).is_none()
    {
        match schemes[0] {
            Scheme::New => println!("pad: none found — riding on the keyboard (Space gas, Shift brake, WASD and YUIO the sticks, HJKL the face buttons)"),
            Scheme::Original => println!("pad: none found — riding on the keyboard (X throttle, Z brake, arrows)"),
        }
    }
    // The other players' pads (V22): seat i is player i's. A player whose seat nobody sits in rides
    // an idle pad, standing on the start line, until a pad joins it.
    let mut idle_pads: Vec<Pads> = (0..args.players).map(|_| Pads::unplugged(args.wiring.clone())).collect();
    for i in 1..args.players {
        match pad_seats.as_ref().and_then(|seats| seats.name(args.pad_seat(i))) {
            Some(name) => println!("player {}: pad {name}", i + 1),
            None => println!("player {}: no pad of their own — an idle pad, standing on the start line", i + 1),
        }
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

    // ---- the GPU (P44) -------------------------------------------------------------------------
    // What draws the picture: the window's own GPU, or with no window one drawing offscreen — unless
    // `--renderer software`, or no GPU to be had, leaves it to the rasteriser.
    let mut offscreen: Option<(Renderer, Option<wgpu::Texture>)> = match (&*window, args.renderer) {
        (None, Backend::Gpu) => match Renderer::headless() {
            Ok(r) => {
                println!("gpu: {} offscreen", r.adapter);
                Some((r, None))
            }
            Err(e) => {
                println!("renderer: {e}; drawing in software");
                None
            }
        },
        _ => None,
    };
    let gpu_on = offscreen.is_some() || window.as_mut().is_some_and(|w| w.gpu().is_some());
    println!("renderer: {}", if gpu_on { Backend::Gpu.name() } else { Backend::Software.name() });

    // ---- the other players' bodies and bikes (V22) ---------------------------------------------
    // Each rider on the screen is posed into a model of its own, because a skinned model's triangles
    // are its pose: the first player's are the models the archives loaded, and every other player's
    // a copy under a name of its own. Their bodies and chassis are loaded again for the same reason
    // the first's are opened twice — each animation set reads members as its own machine asks.
    let mut rigs: Vec<Rig> = Vec::with_capacity(args.players);
    let body_model = body.as_ref().map(|b| b.geometry().to_string());
    for i in 1..riders {
        // Its own rider's archive when the garage picked one, the first player's otherwise.
        let motion = other_phys[i - 1].motion();
        let own_rider = other_riders[i - 1].as_ref();
        let body = match (&rider_member, own_rider) {
            (None, _) => None,
            (Some(_), Some((name, member))) => {
                Body::load(vec![disc.res(member)?], name, Sex::Male, "", &table.stunts, motion).ok()
            }
            (Some(member), None) => {
                Body::load(vec![disc.res(member)?], &args.rider, Sex::Male, "", &table.stunts, motion).ok()
            }
        };
        // Its own bike when the garage picked one (P37): that archive's `.sub`, its chassis and parts
        // loaded again under names of their own in that bike's paint.
        let own_bike = match args.others.get(i - 1) {
            Some(pick) if pick.bike != args.bike => {
                let member = member_ending(&disc, &format!("{}.res", pick.bike))?;
                let res = [disc.res(&member)?];
                match res[0].read_suffix(&format!("{}.sub", pick.bike)) {
                    Ok(bytes) => Some((member, Vehicle::parse(&ini::text(&bytes)), res)),
                    Err(e) => {
                        println!("player {}: bike: {e}", i + 1);
                        None
                    }
                }
            }
            _ => None,
        };
        let (own_member, own_vehicle) = match &own_bike {
            Some((member, v, _)) => (member, v),
            None => (&bike_member, &vehicle),
        };
        let chassis = match own_vehicle.mcf.is_empty() {
            true => None,
            false => Chassis::load(vec![disc.res(own_member)?], &own_vehicle.mcf, BIKE_PREFIX, &table.stunts, vehicle_params()).ok(),
        };
        let copy = |assets: &mut Assets, name: &str| -> Option<String> {
            let own = format!("{name}@{}", i + 1);
            let loaded = assets.models.get(name)?.clone();
            assets.models.insert(own.clone(), loaded);
            Some(own)
        };
        let (bike_model, parts) = match &own_bike {
            Some((_, v, res)) => {
                let own = format!("{CHASSIS}@{}", i + 1);
                let drawn = assets.load_repainted(res, CHASSIS, &own, &v.materials);
                let parts = v
                    .parts
                    .iter()
                    .map(|part| {
                        let name = format!("{}@{}", part.model, i + 1);
                        assets.load_repainted(res, &part.model, &name, &v.materials);
                        game_render::scene::Part { model: name, placement: part.placement.clone() }
                    })
                    .collect();
                (if drawn { own } else { CHASSIS.to_string() }, parts)
            }
            // On the GPU a pose is node matrices and not the model's triangles (P44), so a player on the
            // shared bike draws the one model; a picked bike's repaint is its own model either way.
            None => {
                let model = match gpu_on {
                    true => None,
                    false => copy(&mut assets, CHASSIS),
                };
                (model.unwrap_or_else(|| CHASSIS.to_string()), vehicle.parts.clone())
            }
        };
        // A picked rider's body is loaded again under its own name with its own gear's renames, which
        // is the only way a model gets other textures.
        let own_gear = match (body_model.as_deref(), own_rider, &body) {
            (Some(name), Some((_, member)), Some(b)) => {
                let own = format!("{name}@{}", i + 1);
                let res = [disc.res(member)?];
                assets.load_repainted(&res, name, &own, &b.rider.suit.materials).then_some(own)
            }
            _ => None,
        };
        let own_body = own_gear.or_else(|| {
            body_model.as_deref().and_then(|name| match gpu_on {
                true => Some(name.to_string()),
                false => copy(&mut assets, name),
            })
        });
        rigs.push(Rig {
            body,
            chassis,
            camera: Race { cam: camera.cam.clone() },
            // Their own plane's camera, on the same presets as the first player's (P87).
            plane_camera: plane_camera.as_ref().map(|c| Race { cam: c.cam.clone() }),
            wreck_camera: chase::WreckCamera::new(false),
            hud: Hud::new(args.hud),
            odometer: overlay::Odometer::default(),
            bike_model,
            body_model: own_body,
            parts,
            modified: !mods.seat_is_original(i),
        });
    }
    rigs.insert(
        0,
        Rig {
            body,
            chassis,
            camera,
            plane_camera,
            wreck_camera: chase::WreckCamera::new(false),
            // The riding overlay (P6). It is the shell's own furniture, so it is created here beside
            // the window rather than inside the simulation's `World`, and it reads the port's own
            // values at draw time instead of being fed them.
            hud: Hud::new(args.hud),
            odometer: overlay::Odometer::default(),
            bike_model: CHASSIS.to_string(),
            body_model,
            parts: vehicle.parts.clone(),
            modified: !mods.seat_is_original(0),
        },
    );

    // Everything the GPU draws, uploaded once: the props, the sky, the whole ground and every model.
    if gpu_on && let Some(r) = renderer_of(&mut offscreen, window) {
        let at = Instant::now();
        let chunks = ground.chunks(GROUND_CHUNK);
        r.forget_assets();
        r.set_scene(&look);
        r.set_sky(sky.as_ref());
        r.set_ground(&chunks, ground.paint.as_ref(), ground::DIRT)?;
        r.upload_all(&assets);
        let (models, materials, textures) = r.uploaded();
        println!(
            "gpu: {models} models, {materials} materials, {textures} textures and {} ground chunks uploaded in {:.2} s",
            chunks.len(),
            at.elapsed().as_secs_f32()
        );
        // What the graphics setting makes of the disc's fog (V32): the original's is its fog and far
        // clip as the original works them out of the scene file (V33), the whole map's a haze that
        // never hides it.
        let shown = args.graphics.unwrap_or_default();
        r.set_graphics(shown);
        let lit = Lighting::of(&look);
        let (fog, far) = r.fog(&lit);
        println!(
            "graphics: {} (fog {:.0}..{:.0} up to {:.0}%, far clip {:.0}; the disc's fog {:.0}..{:.0} in {:?})",
            shown.word(),
            fog.near,
            fog.far,
            fog.max * 100.0,
            far,
            lit.fog_near,
            lit.fog_far,
            look.fog_color
        );
    } else if args.graphics == Some(Graphics::Full) {
        println!("graphics: the software renderer draws the original's fog and far clip only");
    }
    // The GPU's per-frame lists, kept to reuse their room: the poses, the overlay and its layers.
    let mut gpu_poses: Vec<Vec<M4>> = Vec::new();
    let mut gpu_patches = Vec::new();
    let mut gpu_sprites = Vec::new();
    // The render scale (V30): as asked, or picked from the frames' time against the display's refresh.
    let refresh = window.as_mut().and_then(|w| w.gpu()).and_then(|w| w.refresh).unwrap_or(60.0);
    let mut auto_scale = game_render::gpu::AutoScale::new(Duration::from_secs_f32(0.8 / refresh));
    // The settings' row changes it mid-ride (V34), unless `--render-scale` was given.
    let mut render_scale = args.render_scale;
    if let Some(r) = renderer_of(&mut offscreen, window) {
        r.render_scale = render_scale.unwrap_or(1.0);
        println!(
            "render scale: {}",
            match render_scale {
                Some(f) => format!("{f}"),
                None if offscreen.is_some() => "1 (auto, offscreen)".into(),
                None => format!("auto, for {refresh:.0} Hz"),
            }
        );
    }
    let mut gpu_layers: Vec<layer::Layer> = (0..=args.players).map(|_| layer::Layer::new()).collect();

    // ---- the loop ------------------------------------------------------------------------------
    let light = Lighting::of(&look);
    // The frame: `--size`'s, which stays put whatever the window does, or the window's own shape
    // — the largest 4:3 one inside it with `--aspect 4:3` — which is re-made whenever the window
    // changes size (below). With no window, the PS2's own unless `--size` says otherwise.
    let frame_size = |window: &Option<Screen>| match (args.size, window) {
        (Some(size), _) => size,
        (None, Some(window)) => args.aspect.frame((window.width, window.height), window.scale),
        (None, None) => PS2_FRAME,
    };
    let (width, height) = frame_size(&window);
    // Which of the original's two displays the frame stands for (V24): at 16:9 its cameras read the
    // wide copies of their presets, as the original's loader does in its widescreen modes.
    let mut display = Display::of(width, height, args.aspect);
    match &window {
        Some(window) => println!(
            "window {}x{} (screen {}x{}), drawing {width}x{height} at scale {}, --aspect {}, a {} display",
            window.width,
            window.height,
            window.screen.0,
            window.screen.1,
            window.scale,
            args.aspect.name(),
            display.name()
        ),
        None => println!("headless, drawing {width}x{height}, a {} display", display.name()),
    }
    if display != Display::Television {
        set_display(&mut rigs, layout, display, cpresets.as_deref(), plane_presets.as_deref());
    }
    // The original's HUD (V26): its art off the disc, the world its radar covers, and which overlay is
    // up. Art that does not load stops the ride with the reason (V28) — it used to say so on stderr
    // and ride on with no HUD, which is how a HUD went missing with nobody told why.
    let hud_art = overlay::Art::load(&mut disc).map_err(|e| format!("the original's HUD: {e}"))?;
    // The GPU draws it as sprites from its pictures, uploaded once (V30).
    // The front end's after them, for the pause (V30), which is why the HUD's are numbered from 0.
    let menu_base = hud_art.images().len();
    if gpu_on && let Some(r) = renderer_of(&mut offscreen, window) {
        let mut images = hud_art.images();
        if let Some(front) = &front {
            images.extend(front.art.images());
        }
        r.set_images(&images);
    }
    let radar_span = overlay::radar_span(ground.spacing);
    let mut overlay_mode = args.overlay.unwrap_or_default();
    let mut speedometer = args.speedometer.unwrap_or_default();
    let mut graphics = args.graphics.unwrap_or_default();
    let mut frame = Frame::new(width, height, look.fog_color);
    // The views (V22): where each player's goes in the frame, and a frame of its own size for each to
    // be drawn into. A full screen has none — its one view is drawn straight into the frame, as it
    // always has been.
    let views_of = |width: usize, height: usize| {
        let rects = layout.rects(width, height);
        let frames: Vec<Frame> = match layout {
            Layout::Full => Vec::new(),
            _ => rects.iter().take(args.players).map(|r| Frame::new(r.w, r.h, look.fog_color)).collect(),
        };
        (rects, frames)
    };
    let (mut rects, mut view_frames) = views_of(width, height);
    if layout != Layout::Full {
        println!(
            "views: {} players, {}, {}x{} each",
            args.players,
            layout.name(),
            rects[0].w,
            rects[0].h
        );
    }
    let mut engine = Engine::new();
    // The ride's first frame comes a world load after the menu's, as the original's does (P52).
    pace::after_load(&mut engine);
    engine.time_scale = mods.time_scale();
    let tables = Arc::new(tables);
    let mut seats = vec![Seat {
        reports: [Report::missing(); PADS],
        sim: Sim::with_rider(0, tables.clone(), phy.clone(), rider),
        garage,
        riding: front.as_ref().is_none_or(|f| f.menu.riding()),
        start_freezes: front.is_none(),
        waiting: false,
        said: rider::Frame::default(),
        inverted: false,
        hands: Controls::default(),
    }];
    for (i, (rider, phy)) in others.into_iter().zip(other_phys).enumerate() {
        // Every player has a plane of their own, parked in their bay of the lot (P87), and starts in
        // it if the setup picked it (P41). A track that parks none rides the bike, and says so.
        let pick = front.as_ref().map_or(menu::Machine::Bike, |f| f.menu.settings.machines[i + 1]);
        let plane = lot.plane(i + 1, flight_for(i + 1));
        let up = match (pick, &plane) {
            (menu::Machine::Biplane, Some(_)) => Kind::Biplane,
            (menu::Machine::Bike, _) => Kind::Bike,
            (pick, None) => {
                println!("vehicle: player {} picked the {}, but the track parks none; the bike", i + 2, pick.word());
                Kind::Bike
            }
        };
        seats.push(Seat {
            reports: [Report::missing(); PADS],
            sim: Sim::with_rider(0, tables.clone(), phy, rider),
            garage: Garage::new(up, plane)?,
            riding: front.as_ref().is_none_or(|f| f.menu.riding()),
            start_freezes: front.is_none(),
            waiting: false,
            said: rider::Frame::default(),
            inverted: false,
            hands: Controls::default(),
        });
    }
    let mut world = World { place: sim, seats, tape: recorder, played_dt: None };

    for (seat, rig) in world.seats.iter().zip(&mut rigs) {
        rig.hud.note_vehicle(seat.garage.up.name());
        let first = rig.follow(seat.garage.up);
        if !first.cam.presets.is_empty() {
            let name = first.cam.preset().name.clone();
            rig.hud.note_camera(&name);
        }
    }
    println!("vehicle: {}", world.seats[0].garage.up.name());
    // The pause's switch row reads which machine each player is on and whether they have another
    // (P41), and any player's Start opens the pause for them: 1P's through the front end's reader,
    // the others' through one each here, which only ever look for Start while the ride runs.
    if let Some(front) = front.as_deref_mut() {
        front.menu.riders = world.seats.len();
        front.menu.modified = !mods.is_original();
        front.menu.up = [menu::Machine::Bike; menu::SEATS_MAX];
        front.menu.switchable = [false; menu::SEATS_MAX];
        for (i, seat) in world.seats.iter().enumerate().take(menu::SEATS_MAX) {
            front.menu.up[i] = menu::Machine::from_word(seat.garage.up.name()).unwrap_or_default();
            front.menu.switchable[i] = seat.garage.plane.is_some();
        }
    }
    let mut start_readers: Vec<Reader> = (1..world.seats.len()).map(|_| Reader::new()).collect();
    // H toggles it, on its own edge: the window keeps key state and not key events, so the shell
    // remembers last frame's to tell a press from a hold.
    let mut hud_key = false;
    // M swaps machines, on its edge the same way (P19).
    let mut swap_key = false;
    // A key's state for the frame: down, or tapped since the last poll (P53), so a tap inside one
    // slow frame still presses.
    let held = |window: &Option<Screen>, code: u8| window.as_ref().is_some_and(|w| w.pressed(code));

    // Online (N2): the session, every seat starting where this ride starts. From the lobby (N3) the
    // line-up and the seed are the host's, every seat rides its own bike set's `.phy` — the bytes the
    // handshake hashed — and the session goes over the lobby's connection.
    let from_lobby = front.as_deref().and_then(|f| f.menu.lobby.start.clone().zip(f.network.net()));
    let drop_in = front.as_deref().is_some_and(|f| f.menu.lobby.drop_in);
    // Where the ride starts, for a peer that drops in on it once it is opened online (N10).
    let spawn = world.seats[0].sim.clone();
    let mut online = match &args.online {
        // Into a ride under way (N10): every seat is the host's first's bike, on this machine's
        // constants for it, and the ride's state comes from the host.
        None if from_lobby.is_some() && drop_in => {
            let ((room, me), net) = from_lobby.expect("checked");
            if head.is_some() || world.tape.is_some() || world.seats[0].garage.up != Kind::Bike || world.seats.len() != 1 {
                return Err("dropping in on an online ride rides one player's bike and no tape: drop --play, --record, --vehicle and --players".into());
            }
            let disc = &front.as_deref().expect("from the lobby").network.disc;
            let first = room.seats.iter().flatten().next().ok_or("the ride has nobody in it")?;
            let bytes = disc.phy(&first.pick.bike).ok_or(format!("the disc has no {} bike set", first.pick.bike))?;
            let mut phy = Phy::read_bytes(&bytes, &tables.ride.curve);
            mods.bike(0, &mut phy);
            let mut sim = world.seats[0].sim.clone();
            sim.phy = Arc::new(phy);
            sim.rng = room.seed;
            let mut online = online::Online::drop_in(net, &room, &me, sim)?;
            online.time_scale = mods.time_scale();
            Some(online)
        }
        None if from_lobby.is_some() => {
            let ((room, me), net) = from_lobby.expect("checked");
            if head.is_some() || world.tape.is_some() || world.seats[0].garage.up != Kind::Bike {
                return Err("an online ride rides the bike and no tape: drop --play, --record and --vehicle".into());
            }
            if world.seats.len() != 1 {
                return Err("an online ride from the lobby has one local player: drop --players".into());
            }
            let disc = &front.as_deref().expect("from the lobby").network.disc;
            let mut sims = Vec::with_capacity(room.seats.len());
            for seat in &room.seats {
                let mut sim = world.seats[0].sim.clone();
                if let Some(seat) = seat {
                    let bytes = disc.phy(&seat.pick.bike).ok_or(format!("the disc has no {} bike set", seat.pick.bike))?;
                    // The host's modifiers, 1P's for every seat: each seat's rider is a copy of
                    // this machine's first, built on 1P's (P55).
                    let mut phy = Phy::read_bytes(&bytes, &tables.ride.curve);
                    mods.bike(0, &mut phy);
                    sim.phy = Arc::new(phy);
                }
                sim.rng = room.seed;
                sims.push(sim);
            }
            // The controller preset (P79) is this machine's and is not sent: every seat online rides
            // the shipped table, preset 0.
            if presets[0] != 0 {
                println!("online: controller preset {} is not sent to the other machines — preset 0", presets[0]);
            }
            let mut online = online::Online::lobby(net, &room, &me, sims)?;
            online.time_scale = mods.time_scale();
            Some(online)
        }
        None => None,
        Some(me) => {
            if head.is_some() || world.tape.is_some() || world.seats[0].garage.up != Kind::Bike {
                return Err("--online rides the bike and no tape: drop --play, --record and --vehicle".into());
            }
            // One local player and no front end: the session's seats are `--seats`, and the pause
            // menu would freeze a ride the other machines go on with.
            if args.players != 1 || front.is_some() {
                return Err("--online rides one local player outside the front end: drop --players and --menu".into());
            }
            let starts: Vec<Sim> = world.seats.iter().map(|s| s.sim.clone()).collect();
            // The controller preset (P79) is this machine's and is not sent: every seat online rides
            // the shipped table, preset 0.
            if presets[0] != 0 {
                println!("online: controller preset {} is not sent to the other machines — preset 0", presets[0]);
            }
            let mut online = online::Online::start(me, &args.peers, args.seats.as_deref(), &starts)?;
            online.time_scale = mods.time_scale();
            Some(online)
        }
    };
    let mut online_frames = 0u32;
    // The pause offers "Open to Online" (N10) on a local freeride of one player on the bike set the
    // disc's own lookup gives (what a joiner's machine loads for it), outside any lobby.
    if let Some(front) = front.as_deref_mut() {
        let set = args.set.unwrap_or(roster::Set::FREERIDE).word();
        let can = online.is_none()
            && head.is_none()
            && world.tape.is_none()
            && world.seats.len() == 1
            && args.phy.is_none()
            && front.menu.lobby.room.is_none()
            && front.network.disc.phy(&set).is_some_and(|b| *b == phy_bytes);
        front.menu.opening = if can { menu::Opening::Can } else { menu::Opening::No };
    }
    // Which shell seats have a rider in them online, for the picture: an empty seat is not drawn.
    let mut seated = vec![true; world.seats.len()];

    let mut shot = args.shot.clone();
    let started = Instant::now();
    let mut window_of_frames = (Instant::now(), 0u32, 0usize);
    // Each view's triangles over the session, for the line a split session ends on.
    let mut view_triangles = vec![0usize; args.players];
    let mut cameras: Vec<Camera> = Vec::with_capacity(args.players);
    // How the ride ended, for the front end: anything but the pause's own two is a quit.
    let mut leave = Leave::Quit;
    let mut escape = held(window, key::ESCAPE);
    let mut perf = perf::Perf::new(args.fps);
    let mut hushed = false;
    // What each stage of the frame costs (P47), for `--timings` and for the line the session ends on.
    let mut stages = Stages::new();
    let mut stages_since = Instant::now();
    // The disc reads, texture decodes and bytes read by the end of the first frame: none should
    // follow.
    let mut loaded: Option<(game_render::tally::Tally, Option<u64>)> = None;
    // The ride's ticks on their own schedule (P52): the original's 30 fps frame, online too (N6:
    // `net::rollback::gate` counts every call as a tick of real time, and each tick's `dt` is the
    // one this schedule gives offline), whatever the picture's rate. Each pass of the loop either
    // runs one tick the wall clock owes, or — owing none — draws, blended between the last two ticks.
    let mut pacer = pace::Pacer::new(pace::TICK, started).most(args.frames).until(args.seconds);
    // What the tick before the last one showed, for the picture to blend from: each seat's pose,
    // each view's camera and the plane.
    let mut before: Vec<Shown> = Vec::new();
    let mut before_cameras: Vec<Camera> = Vec::new();
    let mut before_planes: Vec<Option<M4>> = Vec::new();
    // What the last tick left for the picture: whether it simulated, and each player's ride for the
    // camera and the overlay.
    let mut simulated = false;
    let mut ridden: Vec<Ridden> = Vec::with_capacity(args.players);
    let mut top = Instant::now();
    perf.begin();
    loop {
        if pacer.owed(Instant::now()) {
        if let Some(window) = &mut *window {
            window.poll();
            if window.closed() || (front.is_none() && window.held(key::ESCAPE)) {
                break;
            }
        }
        // A window that changed size gets a frame of its new size rather than the old one stretched
        // over it — and with it the aspect the camera's field of view and its viewport test read,
        // each view's own (V24). A resize across the middle of 4:3 and 16:9 changes the display the
        // frame stands for, and the cameras reload their presets for it the way the original's do
        // when their split changes: the same preset, re-set, from the other copy.
        if window.as_mut().is_some_and(|w| w.resized()) {
            let (width, height) = frame_size(&window);
            if (width, height) != (frame.width, frame.height) {
                frame = Frame::new(width, height, look.fog_color);
                (rects, view_frames) = views_of(width, height);
                if let Some(window) = &window {
                    println!("window {}x{}, drawing {width}x{height}", window.width, window.height);
                }
                let now = Display::of(width, height, args.aspect);
                if now != display {
                    display = now;
                    println!("display: {}", display.name());
                    set_display(&mut rigs, layout, display, cpresets.as_deref(), plane_presets.as_deref());
                }
            }
        }
        // The pad this frame: the tape's, if one is being ridden, and the host's otherwise. A tape
        // that has run out ends the session — there is no pad behind it to carry on with.
        // Each player's report as the pad sent it, before its scheme moved anything, with its presses
        // since the last frame: what the pause reads (P53). Under the new scheme the ride's report
        // has R2 where Cross was, so a pause read from it took R2 for OK and Cross did nothing.
        let mut menu_reports: Vec<(Report, u16)> = Vec::new();
        match &mut head {
            // The seats', otherwise: seat i feeds player i's own pad port (V22's players). A seat
            // whose pad was pulled out pauses its own player rather than riding on with nobody's
            // hands on the bars (`Seat::waiting`); the other players ride on.
            None => match &mut *pad_seats {
                Some(seats) => {
                    let now = seats.frame(&|code| held(&window, code));
                    for line in seats.said() {
                        println!("{line}");
                    }
                    // Each through its seat's layout (P40): what the port reads is the original's. A
                    // local game's player reads the pad seat its join screen gave it (P43).
                    for (i, (seat, idle)) in world.seats.iter_mut().zip(&mut idle_pads).enumerate() {
                        let from = args.pad_seat(i);
                        let report = match now.get(from) {
                            Some(SeatInput::Empty) if i > 0 => idle.report(&pad::no_keys),
                            Some(input) => input.report(),
                            None => idle.report(&pad::no_keys),
                        };
                        menu_reports.push((report, seats.taps(from)));
                        // And the plane's stick turned round upside down under the screen's scheme
                        // (P50), off the attitude the last step left, so the plane's simulation
                        // reads the original's pad; under the plane's (P69) the elevator answers at
                        // every bank instead.
                        seat.inverted = seat.garage.inverted(seat.inverted);
                        seat.garage.set_every_bank(seats.plane(from).every_bank());
                        seat.garage.set_round_stick(seats.plane(from).round_stick());
                        let report = seats.plane(from).fly(seats.scheme(from).ride(report), seat.inverted);
                        seat.reports = [report, Report::missing()];
                        // The bike's commands through the seat's controller preset (P79); online, where
                        // the preset is not sent, the shipped one.
                        seat.sim.set_presets([if online.is_some() { 0 } else { seats.preset(from) }, 0]);
                        seat.waiting = now.get(from) == Some(&SeatInput::Waiting);
                    }
                }
                None => {
                    for (seat, idle) in world.seats.iter_mut().zip(&mut idle_pads) {
                        seat.reports = [idle.report(&pad::no_keys), Report::missing()];
                    }
                }
            },
            Some(head) => match head.next() {
                Some(frame) => {
                    // A tape holds what the plane read, already turned round (P50); this is only
                    // the camera's, which reads the turned-round L1 back.
                    world.seats[0].inverted = world.seats[0].garage.inverted(world.seats[0].inverted);
                    // The fade is not in the tape (P69): it rides under the command line's scheme,
                    // so a plane tape plays back as it was made under the same --biplane-controls.
                    world.seats[0].garage.set_every_bank(args.wiring.planes[0].every_bank());
                    world.seats[0].garage.set_round_stick(args.wiring.planes[0].round_stick());
                    world.seats[0].reports = frame.reports;
                    // A tape holds the pad, not the table: the controller preset is --preset's
                    // (P79), whichever layout is said, because the tape is already past the layout.
                    world.seats[0].sim.set_presets([args.wiring.presets[0], 0]);
                    world.played_dt = Some(frame.dt());
                }
                None => {
                    println!("the tape ran out after {} frames", head.len());
                    break;
                }
            },
        }
        // A tape is the first player's ride, so while one is ridden the others stand idle and the
        // session stays a function of its file.
        if head.is_some() {
            for seat in &mut world.seats[1..] {
                seat.reports = [Report::idle(), Report::missing()];
            }
        }
        // F3, or H where the keyboard's layout does not ride on it: the new layout's H is the right
        // stick's left (P54), which would flash the overlay on and off with every trick.
        let h_free = pad_seats.as_ref().is_none_or(|s| !s.binds_key(key::H));
        let hud_now = held(&window, key::F3) || (h_free && held(&window, key::H));
        if hud_now && !hud_key {
            for rig in &mut rigs {
                rig.hud.on = !rig.hud.on;
            }
        }
        hud_key = hud_now;
        // The swap, between frames, which is where the original's pause menu and prompt run it: the
        // controller moves and the machine taken gets its own camera, which starts where it is
        // rather than lagging in from wherever it was last. A played tape is the ride it recorded,
        // and the tape has no swap in it, so the key is not read while one is being ridden.
        // It is the first player's key, and the first player's plane.
        // The pause's "Switch To Machine" (P41) is the same swap for whichever player opened it; the
        // key stays for the keyboard.
        let swap_now = held(&window, key::M);
        if swap_now && !swap_key && head.is_none() && online.is_none() {
            let up = switch_machine(&mut world.seats[0], &mut rigs[0]);
            if let (Some(up), Some(front)) = (up, front.as_deref_mut()) {
                front.menu.switched(0, menu::Machine::from_word(up.name()).unwrap_or_default());
            }
        }
        swap_key = swap_now;
        stages.mark(stages::Stage::Input);

        // The pause, under the front end: Start or Escape opens it, and while it is open the ride is
        // frozen exactly as Start's own freeze leaves it, the pass polling the pad and stepping
        // nothing. What it changes is applied at once, and saved.
        if let Some(front) = front.as_deref_mut() {
            // Every player's press: another player's Start pauses for them, and while the pause is
            // theirs their pad drives it (P41). Escape is the keyboard's, and Back for whoever drives.
            // Each player's pad as it is, not as the ride's scheme moved it (P53).
            let menu_report = |i: usize| menu_reports.get(i).copied().unwrap_or((world.seats[i].reports[0], 0));
            let (report, taps) = menu_report(0);
            let mut first = front.reader.read_taps(&report, taps);
            // The keyboard pauses and works 1P's pause whoever has 1P (P46), from no seat of its own.
            if let Some(keys) = pad_seats.as_ref().and_then(|s| s.keyboard_alone()) {
                first = first.or(front.keys_reader.read(&keys));
            }
            let mut presses = vec![first];
            for (i, r) in (1..world.seats.len()).zip(&mut start_readers) {
                let (report, taps) = menu_report(i);
                presses.push(r.read_taps(&report, taps));
            }
            let esc = held(window, key::ESCAPE);
            if esc && !escape {
                if front.menu.riding() {
                    front.menu.pause();
                } else if let Some(p) = presses.get_mut(front.menu.driver()) {
                    p.back = true;
                }
            }
            escape = esc;
            front.menu.music_now = front.sound.now();
            let act = front.menu.step_seats(&presses);
            if front.menu.changed() {
                overlay_mode = front.menu.settings.overlay;
                speedometer = front.menu.settings.speedometer;
                graphics = front.menu.settings.graphics;
                if !args.render_scale_asked && render_scale != front.menu.settings.render_scale.fraction() {
                    render_scale = front.menu.settings.render_scale.fraction();
                    if let Some(r) = renderer_of(&mut offscreen, window) {
                        r.render_scale = render_scale.unwrap_or(1.0);
                    }
                    println!("render scale: {} (settings)", render_scale.map_or("auto".into(), |f| f.to_string()));
                }
                for rig in &mut rigs {
                    rig.hud.on = overlay_mode == menu::Overlay::Debug;
                }
                for seat in &mut world.seats {
                    seat.sim.rider.sense.wrecks_enabled = args.wrecks && front.menu.settings.wrecks;
                }
                if let Some(seats) = &mut *pad_seats {
                    seats.set_wiring(front.wiring());
                }
                front.hear();
                front.save();
            }
            match act {
                // Online the ride goes on for the others: no restarting it alone.
                Act::Restart if online.is_some() => {}
                Act::Restart => {
                    leave = Leave::Restart;
                    break;
                }
                Act::MainMenu => {
                    if let Some(on) = &online
                        && !on.leave()
                    {
                        front.menu.lobby.room = None;
                    }
                    leave = Leave::Menu;
                    break;
                }
                Act::Quit => break,
                // A played tape has no swap in it, as the key's does not.
                Act::Switch(i) if head.is_none() => {
                    if let (Some(seat), Some(rig)) = (world.seats.get_mut(i), rigs.get_mut(i))
                        && let Some(up) = switch_machine(seat, rig)
                    {
                        front.menu.switched(i, menu::Machine::from_word(up.name()).unwrap_or_default());
                    }
                }
                // Open to online players (N10): host a lobby on this ride, which rides on as the
                // session's from here, alone until a peer drops in.
                Act::OpenOnline if online.is_none() && front.menu.opening == menu::Opening::Can => {
                    if world.seats[0].garage.up != Kind::Bike {
                        front.menu.note = Some("Open to Online rides the bike: switch to it first.".into());
                    } else {
                        let settings = &front.menu.settings;
                        let mut room = openmx_net::Room::new(&settings.name, &args.track, settings.lobby_seats);
                        room.wrecks = args.wrecks && settings.wrecks;
                        room.modifiers = mods.text();
                        let pick = openmx_net::Pick::new(&front.menu.lobby.pick.rider, &args.set.unwrap_or(roster::Set::FREERIDE).word());
                        room.take(openmx_net::HOST, &[pick]);
                        (room.riding, room.drop_in) = (true, true);
                        let net = front.network.open(&mut front.menu, room);
                        match online::Online::open(net, world.seats[0].sim.clone(), &spawn) {
                            Ok(mut on) => {
                                on.time_scale = mods.time_scale();
                                online = Some(on);
                                front.menu.opening = menu::Opening::Open;
                            }
                            Err(e) => front.menu.note = Some(format!("Could not open the ride: {e}")),
                        }
                    }
                }
                Act::Stay | Act::Resume | Act::Ride(_) | Act::Switch(_) | Act::Online | Act::OpenOnline => {}
            }
            for seat in &mut world.seats {
                seat.riding = front.menu.riding();
            }
        }
        stages.mark(stages::Stage::Menu);

        // Online (N2) the session steps the ride instead — every seat, a rollback's ticks again, the
        // body's frame inside each — at its one fixed frame length, and this machine's seat comes back
        // as the first seat's `sim` (the only local seat online), every other seat as one of the
        // seats after it (N5). Its frames count toward `--frames`, the engine's not being run.
        // Where everything is before this tick, which the picture blends from until the next.
        before = world.seats.iter().zip(&rigs).map(|(seat, rig)| Shown::of(seat, rig, &assets, ragdolled.as_ref(), gpu_on)).collect();
        before_cameras.clone_from(&cameras);
        before_planes = world.seats.iter().map(|seat| seat.garage.plane.as_ref().map(plane_at)).collect();
        let sim_at = Instant::now();
        let dt: F;
        (dt, simulated) = if let Some(on) = &mut online {
            online_frames += 1;
            // Paused, this machine's rider lets go: the others ride on.
            let pad = match front.as_deref().is_some_and(|f| !f.menu.riding()) {
                true => Report::idle(),
                false => world.seats[0].reports[0],
            };
            if let Err(end) = on.frame(pad, &mut world.place, rigs[0].body.as_ref()) {
                println!("{end}");
                if front.is_some() {
                    leave = Leave::Menu;
                }
                break;
            }
            // Every seat back into the shell's, this machine's first: the others are drawn beside it.
            for (d, (seat, rig)) in world.seats.iter_mut().zip(&mut rigs).enumerate() {
                seated[d] = on.show(d, &mut seat.sim, rig.body.as_mut());
            }
            world.seats[0].hands = on.hands;
            world.seats[0].said = rider::Frame::default();
            // An opened ride's lobby (N10) is the pause's to show: its code, who is in.
            if let Some(front) = front.as_deref_mut()
                && front.menu.opening == menu::Opening::Open
            {
                for e in on.kept() {
                    front.menu.heard(e);
                }
            }
            (on.dt(), true)
        } else {
        // One frame of the game's own fixed update, which calls `World::simulate` inside itself.
        // The clock a steady PS2 reads at this tick, not the host's (P52): every tick is the same
        // length whatever the picture did, and `dt` is the original's.
        let now = pace::tick_clock(pacer.ticks());
        let step = engine.step(now, engine_input(&world.seats[0].reports[0]), &mut world);
        if step.outcome == Outcome::Quit {
            break;
        }
        // A frame the engine did not simulate is a frame of the tape nobody rode, and the rest of the
        // ride would be a different run from the one recorded. Nothing in the shell sets the engine's
        // frozen flag (a pulled pad pauses its own player), so this says so and stops rather than
        // carrying on with a hole in the ride.
        if head.is_some() && !step.simulated {
            return Err("the engine skipped a frame while a tape was being ridden".into());
        }
        // What the simulation was actually handed, which the pose and the camera below step with too.
        (world.played_dt.unwrap_or(step.dt), step.simulated)
        };
        perf.add(Stage::Sim, sim_at.elapsed());
        stages.mark(stages::Stage::Sim);
        // The plane's particles (P58): a frame of them for each frame the plane was simulated, off
        // where it is now. Nothing here writes the simulation; the ones that collide ask the ride's own
        // terrain query for the ground under them (P61), which reads the terrain and nothing else.
        if simulated && let (Some((fx, binds)), Some(plane)) = (&mut plane_fx, &world.seats[0].garage.plane) {
            let place = &mut world.place;
            fx.step_over(&plane_fx_frame(plane, binds), dt.0, &mut |at| plane_fx_ground(place, at));
        }
        if simulated && !traffic.cars.is_empty() {
            traffic_time += f64::from(dt.0);
            let place = &mut world.place;
            traffic_at = traffic.placed(traffic_time, &mut |p| car_ground(place, p));
        }
        let (scene_at, spent) = (Instant::now(), perf.spent());

        cameras.clear();
        ridden.clear();
        for (i, (seat, rig)) in world.seats.iter_mut().zip(&mut rigs).enumerate() {
            // How a player other than the first is named in the lines below; the first's lines are
            // the ones a one-player session has always printed.
            let who = match i {
                0 => String::new(),
                _ => format!("player {}: ", i + 1),
            };
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
            let bike_up = seat.garage.up == Kind::Bike;
            // A pass the engine skipped, or one this player waited out for their pad, moves neither.
            let moving = seat.riding && simulated && !seat.waiting;
            if let Some(body) = &mut rig.body {
                // Online the session posed it inside its ticks, so a rollback rolls the body back too.
                if moving && bike_up && online.is_none() {
                    perf.time(Stage::Animation, || {
                        body.ride(dt, &mut seat.sim.rider, &seat.hands, rider::Gate::default())
                    });
                    // The pose just drawn is the one the next frame's wreck is thrown from (S65).
                    if let (Some(doll), true) = (&ragdolled, seat.sim.rider.ragdoll_kit.is_some()) {
                        seat.sim.rider.skeleton = Some(Arc::new(body.ragdoll_posed(doll)));
                    }
                }
            }

            // And the bike under it (V9). Two halves in one call: the chassis skeleton posed from the
            // vehicle's own animation machine, and then the suspension, steering and wheel nodes written
            // over that pose from the ride's own numbers — the fork travel, the steering angle and the two
            // wheels' spin, which are not in any clip because no clip could have them.
            if let Some(chassis) = &mut rig.chassis {
                if moving && bike_up {
                    let ride = &seat.sim.rider.ride;
                    let want = Chassis::want(ride, &seat.sim.rider.tricks);
                    // The rider's two axes and not zeros: in the air they are the crossup sweep the
                    // chassis' clip is played at, and zero is the sweep's far end (P29).
                    let request = Chassis::riding(&seat.sim.rider);
                    perf.time(Stage::Animation, || chassis.frame(dt, want, &request, Drive::of(ride)));
                    if let Some(loaded) = assets.models.get_mut(&rig.bike_model).filter(|_| !gpu_on) {
                        perf.time(Stage::Skinning, || chassis.skin_into(loaded));
                    }
                }
            }
            stages.mark(stages::Stage::Animation);

            // What the rider did, as a line: a run nobody is watching still says whether a trick
            // started, landed or put the rider down, and by which of the original's reason codes.
            if let Some(t) = seat.said.started {
                println!("{who}trick: {}", table.display_name(0, t));
                rig.hud.note_trick(table.display_name(0, t));
            }
            if let Some(t) = seat.said.landed {
                println!("{who}landed: {}", table.display_name(0, t));
                rig.hud.note_trick(&format!("{} landed", table.display_name(0, t)));
            }
            if let Some(why) = seat.said.wreck {
                println!("{who}wreck: reason {} — L3 and R3 together to get back on", why.0);
                rig.hud.note_wreck(why);
            }
            if seat.said.respawned {
                println!("{who}put back");
            }
            // Online, the other machines' riders past the local players have no view of their own:
            // posed and drawn, no camera.
            if i >= args.players {
                continue;
            }

            // The camera, after the frame, which is where the original's viewport updates it: the shell
            // picks the preset, the ported camera does everything else.
            // Whichever machine has the controller has the camera: the bike's, on the track's presets, or
            // the plane's, on its own file's.
            let follow = match (seat.garage.up, &mut rig.plane_camera) {
                (Kind::Biplane, Some(cam)) => cam,
                _ => &mut rig.camera,
            };
            // The original's plane reset (the chord, a wreck's recovery) snaps the plane's camera onto
            // the spawn rather than letting it fly the whole way back (V29).
            if seat.garage.up == Kind::Biplane && seat.said.respawned {
                follow.cam.snap = true;
            }
            let presets = follow.cam.presets.len();
            if presets > 1 {
                let pad = seat.sim.input.pad(0);
                // The shipped table's next-camera command, Select under either layout (P73). The walk
                // is the original's next-camera routine: on to the next preset the file lets the player
                // cycle to. The layout is the seat's, or the command line's for a session with no seats.
                let scheme = pad_seats.as_ref().map_or(args.wiring.schemes[i], |s| s.scheme(args.pad_seat(i)));
                let plane = pad_seats.as_ref().map_or(args.wiring.planes[i], |s| s.plane(args.pad_seat(i)));
                // The bike's two hold-back commands are where its controller preset puts them (P79).
                let changes = match seat.garage.up {
                    Kind::Bike => scheme.changes_camera_on(pad, seat.sim.table()),
                    _ => plane.changes_camera(scheme, pad, seat.inverted),
                };
                if changes {
                    follow.cam.next_preset();
                    println!("{who}camera: {}", follow.cam.preset().name);
                    rig.hud.note_camera(&follow.cam.preset().name);
                }
            }
            // The camera's target block, as the original's vehicle fills it once a frame (`chase.rs`).
            // The airborne flag is the one the camera heads along the velocity on: in the air the
            // backflip's upside-down forward row never reaches it (P34).
            let rider = match (&seat.garage.up, &seat.sim.rider.crash) {
                (Kind::Bike, Some(crash)) => chase::wrecked(crash),
                _ => chase::ridden(&seat.garage, &seat.sim.rider.ride),
            };
            // The wreck camera (V31): the moment the rider wrecks, the screen switches to it, starting
            // from the follow camera's eye and look-at as they were last frame; back on, the screen
            // goes back to the follow camera, which snaps rather than lagging in from the wreck.
            let crash = match seat.garage.up {
                Kind::Bike => seat.sim.rider.crash.as_ref(),
                _ => None,
            };
            let wreck = &mut rig.wreck_camera;
            wreck.wide = display == Display::Wide;
            let wreck_targets = crash.map(|crash| {
                let down = match (&seat.sim.rider.ragdoll, &seat.sim.rider.ragdoll_kit) {
                    (Some(down), Some(kit)) => Some((down, &**kit)),
                    _ => None,
                };
                chase::wreck_targets(crash, down)
            });
            wreck.switch(wreck_targets.as_ref(), crash.map_or(0, |c| c.reason.0), &mut follow.cam);
            // The shape of the view this camera draws into (V22), which on a full screen is the frame.
            let (view_w, view_h) = (rects[i].w, rects[i].h);
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
                    &seat.sim.tables.ride.curve,
                    Switches {
                        // Nothing in the shell sets the engine bit that stops the look-ahead. The
                        // split-screen pitch trim wants a view less than 0.55 as tall as it is wide,
                        // which is the same test the original makes of its own viewport's two extents:
                        // each view's own, so the stacked split's two strips have it and a full
                        // screen, the side-by-side halves and the quarters do not.
                        no_look_ahead: false,
                        // Off the television, never: the original skips the test at 16:9.
                        short_viewport: aspect::short_viewport(display, view_w, view_h),
                    },
                    &mut world.place,
                ))
            };
            let view = match (view, &wreck_targets) {
                (Some(_), Some(targets)) if wreck.on => Some(wreck.update(
                    dt.0,
                    targets,
                    aspect::short_viewport(display, view_w, view_h),
                    NEAR_CLIP,
                    &seat.sim.tables.ride.curve,
                    &mut world.place,
                )),
                (view, _) => view,
            };
            let bike_look = vector(rider.target.forward);
            let pos = vector(rider.target.body_point);
            cameras.push(match &view {
                Some(view) => Camera {
                    eye: vector(view.position),
                    at: vector(view.look_at),
                    // The file's field of view is the horizontal one; the rasteriser wants the vertical
                    // one in radians, which depends on the shape of the view it is drawing into.
                    // Past the shapes the original ever drew, the angle along the short side holds
                    // and the long side widens (`view_fov`).
                    fov: aspect::fov(view.fov.0, view_w, view_h),
                    near: NEAR_CLIP,
                    far: 4000.0,
                },
                None => Camera::behind(pos, bike_look),
            });
            ridden.push(rider);
            stages.mark(stages::Stage::Camera);
        }
        perf.lap(Stage::Scene, scene_at, spent);

        // The frame's sound (P42), off what the ride just did, heard from every view's camera. A
        // ride nobody is riding (the pause, a frozen ride) is quiet, and starts again from idle.
        if let Some(sound) = &mut sound {
            if world.seats.iter().any(|seat| seat.riding) {
                let ears: Vec<audio::Listener> = cameras.iter().map(listener).collect();
                sound.listen(&ears);
                let frames: Vec<audio::SeatFrame> = world.seats.iter().map(seat_sound).collect();
                sound.frame(&frames, dt.0);
                hushed = false;
            } else if !hushed {
                sound.quiet();
                hushed = true;
            }
        }

        pacer.ran();
        continue;
        }

        // The picture, owing no tick (P52): every camera, bike, rider and the plane blended from the
        // tick before the last to the last by how far the wall clock is past it. Render-side only:
        // nothing below writes the simulation.
        let blend = pace::Blend(pacer.blend(Instant::now()));
        let cameras: Vec<Camera> = cameras
            .iter()
            .enumerate()
            .map(|(v, c)| before_cameras.get(v).map_or(*c, |b| blend.camera(b, c)))
            .collect();

        // The pictures, one a view (V22), each drawn by its own player's camera into a frame of its
        // own shape and put into the window's frame; a full screen's one view is the frame itself.
        // Each player's body is skinned at the level of detail the camera drawing it has earned, so
        // a rider is re-skinned for a view only where that differs from the view before.
        let mut skinned: Vec<Option<usize>> = vec![None; rigs.len()];
        let mut drawn = 0;
        if gpu_on {
            // The overlay first: each view's HUD, and the pause over the whole frame.
            let draw_at = Instant::now();
            gpu_patches.clear();
            gpu_sprites.clear();
            for (v, at) in rects.iter().enumerate().take(world.seats.len()) {
                let (seat, rig) = (&world.seats[v], &mut rigs[v]);
                let bike_up = seat.garage.up == Kind::Bike;
                rig.hud.plane = !bike_up;
                rig.hud.bindings = *seat.sim.table();
                rig.hud.flight = match (bike_up, &seat.garage.plane) {
                    (false, Some(plane)) => plane.gauges().map(|(throttle, air)| (throttle.0, air.0)),
                    _ => None,
                };
                if rig.hud.on {
                    let hud = &rig.hud;
                    let draw = |f: &mut Frame| hud.draw(f, seat.sim.input.pad(0), &seat.sim.rider, seat.riding && simulated);
                    gpu_patches.extend(gpu_layers[v].patch(at.w, at.h, (at.x, at.y), draw));
                } else if overlay_mode == menu::Overlay::Original {
                    let art = &hud_art;
                    // The original's own HUD (V26), as the software path draws it.
                    let b = ridden[v].target.body_point;
                    if seat.riding && simulated {
                        rig.odometer.step([b.x(), b.y(), b.z()]);
                    }
                    let (window, gauges) = ((frame.width, frame.height), gauges(seat, rig, speedometer));
                    art.sprites(&mut gpu_sprites, (at.x, at.y, at.w, at.h), window, &ridden, v, radar_span, &gauges);
                }
            }
            if let Some(front) = &front
                && front.menu.screen().is_some()
            {
                // As sprites (V30), after the HUDs' and so over them; the patches go under both.
                front.art.sprites(&front.menu, (frame.width, frame.height), menu_base, &mut gpu_sprites);
            }
            perf.add(Stage::Draw, draw_at.elapsed());

            // What stands in the world besides the props, once for every view: each bike, its parts
            // and its rider, posed by node matrices the vertex shader skins with.
            let (scene_at, spent) = (Instant::now(), perf.spent());
            gpu_poses.clear();
            let mut actors: Vec<Actor> = Vec::new();
            for (p, (seat, rig)) in world.seats.iter().zip(&rigs).enumerate() {
                if seat.garage.up != Kind::Bike || !seated[p] {
                    continue;
                }
                let now = perf.time(Stage::Skinning, || Shown::of(seat, rig, &assets, ragdolled.as_ref(), true));
                // A put-back is a cut: the bike and the rider are drawn where it set them, not slid
                // there from the wreck (S66), and so is the body's first tick back on the bike (S74).
                let shown = match before.get(p) {
                    Some(b) if !seat.sim.rider.drawn_jumps() => b.blend(&now, &blend),
                    _ => now.clone(),
                };
                let bike_at = shown.bike;
                let pose = (!shown.chassis.is_empty()).then(|| {
                    gpu_poses.push(shown.chassis.clone());
                    gpu_poses.len() - 1
                });
                // See-through for a while after a put-back, the bike, its parts and the rider alike (P57).
                let alpha = seat.sim.rider.ghost.shown();
                actors.push(Actor { model: rig.bike_model.as_str(), at: bike_at, pose, lod_from: None, alpha });
                let node_now = |name: &str| -> Option<M4> {
                    let blended = rig.chassis.as_ref().and_then(|c| c.stance.set.skeleton.find(name));
                    blended.and_then(|j| shown.chassis.get(j).copied()).or_else(|| {
                        let model = &assets.models.get(CHASSIS)?.model;
                        let i = model.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(name))?;
                        Some(model.world(i))
                    })
                };
                for part in &rig.parts {
                    let at = match node_now(&part.placement) {
                        Some(node) => node.mul(bike_at),
                        None => bike_at,
                    };
                    actors.push(Actor { model: part.model.as_str(), at, pose: None, lod_from: None, alpha });
                }
                // The body, thrown by the ragdoll's bones while the rider is down (S53) and on the
                // bike otherwise ([`Shown::of`]).
                if let (Some(pose), Some(model)) = (shown.body, &rig.body_model) {
                    gpu_poses.push(pose);
                    actors.push(Actor {
                        model: model.as_str(),
                        at: bike_at,
                        pose: Some(gpu_poses.len() - 1),
                        lod_from: Some(vector(seat.sim.rider.ride.position())),
                        alpha,
                    });
                }
            }
            // Every player's plane, in its own livery (P87).
            for (p, seat) in world.seats.iter().enumerate() {
                let Some(plane) = &seat.garage.plane else { continue };
                let Some(drawn) = plane_drawn.get(plane.livery).or(plane_drawn.first()) else { continue };
                let before = before_planes.get(p).copied().flatten();
                let at = before.map_or(plane_at(plane), |b| blend.at(b, plane_at(plane)));
                for (model, at) in drawn.turned_parts(at, plane.prop_angle().map(|a| a.0)) {
                    actors.push(Actor { model, at, pose: None, lod_from: None, alpha: 1.0 });
                }
            }
            for (model, at) in &traffic_at {
                actors.push(Actor { model: model.as_str(), at: *at, pose: None, lod_from: None, alpha: 1.0 });
            }
            let views: Vec<View> = rects
                .iter()
                .zip(&cameras)
                .take(world.seats.len())
                .map(|(at, camera)| View { rect: [at.x as u32, at.y as u32, at.w as u32, at.h as u32], camera, actors: &actors })
                .collect();
            perf.lap(Stage::Scene, scene_at, spent);

            let draw_at = Instant::now();
            let scene = Scene3 { views: &views, poses: &gpu_poses, overlay: &gpu_patches, sprites: &gpu_sprites, light: &light };
            let sprites = plane_fx.as_ref().map_or_else(Vec::new, |(fx, _)| plane_sprites(fx));
            let (fw, fh) = (frame.width as u32, frame.height as u32);
            // A shot this frame is read back from a frame drawn offscreen at the frame's own size.
            let shooting = shot.is_some() && engine.frame >= args.shot_frame;
            let mut waited = Duration::ZERO;
            let out = match (&mut offscreen, window.as_mut().and_then(|w| w.gpu())) {
                (Some((r, texture)), _) => {
                    let t = texture.take().filter(|t| (t.width(), t.height()) == (fw, fh)).unwrap_or_else(|| r.offscreen(fw, fh));
                    let view = t.create_view(&Default::default());
                    r.set_graphics(graphics);
                    set_plane_sprites(r, &mut fx_sheet, &sprites);
                    let d = r.render(&offscreen_target(&view, fw, fh), &assets, &scene);
                    // Nothing else waits on a frame nobody sees: without this the queue runs ahead,
                    // and the times would be the CPU's alone.
                    let wait_at = Instant::now();
                    r.finish();
                    waited = wait_at.elapsed();
                    if shooting {
                        frame.color = r.read(&t);
                    }
                    *texture = Some(t);
                    Some(d)
                }
                (None, Some(w)) => {
                    w.renderer.set_graphics(graphics);
                    set_plane_sprites(&mut w.renderer, &mut fx_sheet, &sprites);
                    let spent = Instant::now();
                    let d = w.draw(frame.width, frame.height, |r, t| r.render(t, &assets, &scene))?;
                    waited = w.waited;
                    if render_scale.is_none() {
                        let was = w.renderer.render_scale;
                        w.renderer.render_scale = auto_scale.frame(spent.elapsed());
                        if w.renderer.render_scale != was {
                            println!("render scale: {} (auto)", w.renderer.render_scale);
                        }
                    }
                    if shooting {
                        let t = w.renderer.offscreen(fw, fh);
                        let view = t.create_view(&Default::default());
                        w.renderer.render(&offscreen_target(&view, fw, fh), &assets, &scene);
                        frame.color = w.renderer.read(&t);
                    }
                    d
                }
                _ => None,
            };
            perf.add(Stage::Draw, draw_at.elapsed().saturating_sub(waited));
            perf.add(Stage::Present, waited);
            if let Some(d) = out {
                for (v, tris) in d.triangles.iter().enumerate() {
                    view_triangles[v] += tris;
                    drawn += tris;
                }
            }
        }
        for (v, at) in rects.iter().enumerate().filter(|_| !gpu_on) {
            if v >= world.seats.len() {
                // A layout with room for more players than there are: the spare views are black.
                split::blank(&mut frame, *at, [0, 0, 0]);
                continue;
            }
            let target = match view_frames.get_mut(v) {
                Some(view) => view,
                None => &mut frame,
            };
            let camera = &cameras[v];
            let pos = vector(ridden[v].target.body_point);
            // The picture: the dirt, then the scene's props and the bikes standing on them.
            let draw_at = Instant::now();
            clear(target, look.fog_color);
            stages.mark(stages::Stage::Sky);
            let mut tris = ground.draw(target, camera, &light, pos, GROUND_RADIUS);
            stages.mark(stages::Stage::Ground);
            // The sky after the ground and before anything that blends (P47): it only paints where
            // nothing is drawn, and the ground is opaque, so it is the same picture as the sky first
            // with the ground over it, for the pixels the ground leaves rather than all of them.
            if let Some(sky) = &sky {
                sky.draw(target, camera);
            }
            perf.add(Stage::Draw, draw_at.elapsed());
            stages.mark(stages::Stage::Sky);
            let (scene_at, spent) = (Instant::now(), perf.spent());
            let mut actor: Vec<(&str, M4)> = Vec::new();
            for (p, (seat, rig)) in world.seats.iter().zip(&rigs).enumerate() {
                // With the plane up the bike is hidden, as the swap hides its model, and the rider is the
                // plane's pilot — whom V21 seats.
                if seat.garage.up != Kind::Bike || !seated[p] {
                    continue;
                }
                // The bike where the wreck body has taken it while the rider is down (S53), and where
                // the ride has it otherwise.
                let bike_at = match &seat.sim.rider.crash {
                    Some(crash) => chassis_wrecked(crash),
                    None => chassis_at(&seat.sim.rider.ride),
                };
                // Between the last two ticks (P52). The software path blends where the machine is
                // and not its pose, which it skins once a tick.
                let bike_at = before.get(p).map_or(bike_at, |b| blend.at(b.bike, bike_at));
                // Each part rides on one node of the chassis, so its transform is that node's place in the
                // chassis followed by the chassis's place in the world.
                actor.push((rig.bike_model.as_str(), bike_at));
                // The node's *live* transform where a frame has posed the chassis, and its bind transform
                // where nothing has: the front fender and the number plate hang off `Handlebars`, so a part
                // placed at a bind pose is a part that does not steer with the bike it is bolted to.
                let node_now = |name: &str| -> Option<M4> {
                    rig.chassis.as_ref().and_then(|c| c.node(name)).or_else(|| {
                        let model = &assets.models.get(CHASSIS)?.model;
                        let i = model.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(name))?;
                        Some(model.world(i))
                    })
                };
                for part in &rig.parts {
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
                let grips = match &rig.chassis {
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
                // While the rider is down and the ride runs their bones (S53), the body is drawn from
                // the bones: thrown off the bike, not sat on it.
                let down = match (seat.sim.rider.drawn_ragdoll(), &seat.sim.rider.ragdoll_kit, &ragdolled) {
                    (Some(doll), Some(kit), Some(rigged)) => Some((doll.nodes(kit), rigged)),
                    _ => None,
                };
                if let (Some(body), Some(model), Some((bones, rigged))) = (&rig.body, &rig.body_model, &down) {
                    let away = vector(seat.sim.rider.ride.position()).sub(camera.eye).len();
                    let focal = camera.focal(target.height);
                    if let Some(loaded) = assets.models.get_mut(model.as_str()) {
                        let group = loaded.model.group_for(away, loaded.radius, focal);
                        body.skin_ragdoll_into(loaded, group, rigged, bones, bike_at);
                        skinned[p] = None;
                    }
                    actor.push((model.as_str(), bike_at));
                } else if let (Some(body), Some(model)) = (&rig.body, &rig.body_model) {
                    if body.posed {
                        // Which of the body's five levels of detail this camera has earned, and the hands and
                        // feet put back where the chassis wants them. Both read the camera, so the skinning
                        // is here rather than beside the pose above.
                        let away = vector(seat.sim.rider.ride.position()).sub(camera.eye).len();
                        let focal = camera.focal(target.height);
                        if let Some(loaded) = assets.models.get_mut(model.as_str()) {
                            let group = loaded.model.group_for(away, loaded.radius, focal);
                            if skinned[p] != Some(group) {
                                perf.time(Stage::Skinning, || body.skin_into_at(loaded, group, Some(&grips)));
                                skinned[p] = Some(group);
                            }
                        }
                        actor.push((model.as_str(), bike_at));
                    }
                }
            }
            // The plane stands in the world either way: parked until it is taken, wherever it was
            // left after. At its body's pose, nose and up off the plane's own measure, with the prop
            // at the engine's angle (S35) — or the bent cone, unturned, once it is wrecked.
            for (p, seat) in world.seats.iter().enumerate() {
                let Some(plane) = &seat.garage.plane else { continue };
                let Some(drawn) = plane_drawn.get(plane.livery).or(plane_drawn.first()) else { continue };
                let before = before_planes.get(p).copied().flatten();
                let at = before.map_or(plane_at(plane), |b| blend.at(b, plane_at(plane)));
                actor.extend(drawn.turned_parts(at, plane.prop_angle().map(|a| a.0)));
            }
            actor.extend(traffic_at.iter().map(|(model, at)| (model.as_str(), *at)));
            perf.lap(Stage::Scene, scene_at, spent);
            let draw_at = Instant::now();
            tris += draw_scene(target, &look, &assets, &actor, camera);
            // The plane's particles (P62), after the models and before the overlay, as the GPU draws
            // them, two triangles a square in the count.
            if let (Some((fx, _)), Some(sheet)) = (&plane_fx, &soft_sheet) {
                tris += 2 * game_render::particle::draw(target, camera, sheet, &plane_particles(fx), &light);
            }
            stages.mark(stages::Stage::Scene);
            // And the overlay over the top of it, after everything with a depth test and before anything
            // that reads the frame: what the window shows and what `--shot` writes are the same picture.
            let (seat, rig) = (&world.seats[v], &mut rigs[v]);
            let bike_up = seat.garage.up == Kind::Bike;
            rig.hud.plane = !bike_up;
            rig.hud.bindings = *seat.sim.table();
            rig.hud.flight = match (bike_up, &seat.garage.plane) {
                (false, Some(plane)) => plane.gauges().map(|(throttle, air)| (throttle.0, air.0)),
                _ => None,
            };
            rig.hud.draw(target, seat.sim.input.pad(0), &seat.sim.rider, seat.riding && simulated && !seat.waiting);
            // The original's own HUD (V26) where the debug one is not up, unless `none` was asked for.
            if !rig.hud.on && overlay_mode == menu::Overlay::Original {
                let art = &hud_art;
                let b = ridden[v].target.body_point;
                if seat.riding && simulated {
                    rig.odometer.step([b.x(), b.y(), b.z()]);
                }
                art.draw(target, (width, height), &ridden, v, radar_span, &gauges(seat, rig, speedometer));
            }
            if let Some(view) = view_frames.get(v) {
                split::put(&mut frame, view, *at);
            }
            view_triangles[v] += tris;
            drawn += tris;
            perf.add(Stage::Draw, draw_at.elapsed());
            stages.mark(stages::Stage::Hud);
        }
        if let Some(front) = front.as_ref().filter(|_| !gpu_on) {
            perf.time(Stage::Draw, || front.art.draw(&front.menu, &mut frame));
        }
        stages.mark(stages::Stage::Hud);
        if let Some(window) = window.as_mut().filter(|_| !gpu_on) {
            let present_at = Instant::now();
            window.present(&frame.color, frame.width, frame.height)?;
            perf.add(Stage::Present, present_at.elapsed());
        }
        stages.mark(stages::Stage::Present);

        // One frame, on paper, for a run nobody is watching: the first by default, or the one
        // `--shot-frame` asks for, which is how a shot catches a ride in progress.
        if let Some(path) = shot.take_if(|_| engine.frame.max(online_frames) >= args.shot_frame) {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            png::write_rgb(&path, frame.width, frame.height, &frame.color)?;
            println!("wrote {}", path.display());
        }

        // A line a second, so a run that is not being watched still says what it did.
        let bike_up = world.seats[0].garage.up == Kind::Bike;
        window_of_frames.1 += 1;
        window_of_frames.2 += drawn;
        if window_of_frames.0.elapsed() >= Duration::from_secs(1) {
            let (since, frames, triangles) = window_of_frames;
            println!(
                "{:>3.0}s {:>4.1} fps, {:>6} triangles a frame, {:>5.1} mph{}{}",
                started.elapsed().as_secs_f32(),
                frames as f32 / since.elapsed().as_secs_f32(),
                triangles / frames.max(1) as usize,
                // Feet a second to miles an hour, for a number a rider can read.
                ridden[0].speed.0 * 0.681_818,
                match (world.seats[0].riding && simulated, world.seats[0].sim.rider.phase()) {
                    _ if world.seats[0].waiting => " (waiting for the pad)",
                    (false, _) => " (frozen)",
                    _ if !bike_up => " (biplane)",
                    (_, Phase::Riding) => "",
                    (_, Phase::Down) => " (down)",
                    (_, Phase::Resetting) => " (getting up)",
                },
                match args.players {
                    1 => String::new(),
                    n => format!(", {n} views"),
                }
            );
            // And the plane's own numbers while it flies: its height, the gauges and whether a
            // wheel is down.
            if let (false, Some(plane)) = (bike_up, &world.seats[0].garage.plane)
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
            // And its particles in the air, by emitter (P58).
            if let (false, Some((fx, _))) = (bike_up, &plane_fx) {
                let counts: Vec<String> = fx.counts().iter().map(|(name, n)| format!("{name} {n}")).collect();
                println!("     particles: {}", counts.join(", "));
            }
            window_of_frames = (Instant::now(), 0, 0);
        }
        stages.mark(stages::Stage::Log);

        if let Some(limit) = args.seconds {
            if started.elapsed().as_secs_f32() >= limit {
                break;
            }
        }
        if args.frames.is_some_and(|n| pacer.ticks() >= u64::from(n)) {
            break;
        }
        perf.end(rects.len().min(world.seats.len()));
        // The picture's own rate (P52): as fast as the machine draws it — a window's present waits
        // for the display's refresh — or `--draw-fps`', which a headless ride, with no display to
        // wait for, takes as 60. The ticks owed meanwhile all run before the next picture, so a slow
        // picture is a jumpier one and never a slower game.
        if let Some(rest) = args.draw_frame().and_then(|frame| frame.checked_sub(top.elapsed())) {
            std::thread::sleep(rest);
        }
        stages.mark(stages::Stage::Pace);
        stages.frame();
        loaded.get_or_insert_with(|| (game_render::tally::Tally::now(), stages::bytes_read_mark()));
        if args.timings && stages_since.elapsed() >= Duration::from_secs(1) {
            println!("     {}", stages.line());
            stages_since = Instant::now();
        }
        top = Instant::now();
        perf.begin();
    }

    // A split session's last word on each view: whose it was, how often that player's simulation
    // ran, where they and their camera ended up, and what their view drew — what a run nobody
    // watched says about whether every player rode and every view followed its own rider.
    if args.players > 1 {
        for (i, (seat, at)) in world.seats.iter().zip(&rects).enumerate() {
            let (p, eye) = (seat.sim.rider.ride.position(), cameras.get(i).map(|c| c.eye));
            println!(
                "player {}: {} ticks, at {:.1}, {:.1}, {:.1}; view {}x{} at {},{}, camera at {}, {} triangles a frame",
                i + 1,
                seat.sim.ticks,
                p.x(),
                p.y(),
                p.z(),
                at.w,
                at.h,
                at.x,
                at.y,
                match eye {
                    Some(e) => format!("{:.1}, {:.1}, {:.1}", e.x, e.y, e.z),
                    None => "none".to_string(),
                },
                view_triangles[i] / (engine.frame.max(1) as usize),
            );
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
    // different one. A headless ride is a function of its ticks too, since every tick reads the PS2's
    // clock and not the host's (P52): `--frames N` rides the same N ticks at any drawn rate. A ride
    // with a pad is a different ride every time, and its number is worth nothing.
    if head.is_some() || args.record.is_some() || args.headless {
        println!("ride hash {:#018x}", game_sim::hash::of(&world.seats[0].sim.rider.ride));
    }

    let opened = front.as_deref().is_some_and(|f| f.menu.opening == menu::Opening::Open);
    if let Some(front) = front.as_deref_mut() {
        front.menu.opening = menu::Opening::No;
    }
    if let Some(on) = &mut online {
        println!("{}", on.summary());
        // What the lobby said while the ride ran (a drop, the lobby as it is now, chat) is the menu's.
        if let Some(front) = front.as_deref_mut() {
            for e in on.kept() {
                front.menu.heard(e);
            }
            // an opened freeride's room closed with it (`Online::leave`): no lobby to go back to
            if opened {
                on.leave();
                front.menu.lobby.room = None;
                front.menu.lobby.code.clear();
            }
        }
    }
    perf.summary();
    // The ride's sounds stop with it; the music plays on into the menus.
    if let Some(sound) = &mut sound {
        sound.quiet();
    }
    if args.timings {
        println!("session {}", stages.session());
        // Bytes read count the window's socket and every pad's events too, so only a ride with
        // neither, a headless one, should say none.
        let (tally, bytes) = loaded.unwrap_or_else(|| (game_render::tally::Tally::now(), None));
        let after = game_render::tally::Tally::now().since(tally);
        let bytes = match (bytes, stages::bytes_read()) {
            (Some(from), Some(now)) => format!("{} bytes read", now.saturating_sub(from)),
            _ => "bytes read unknown".to_string(),
        };
        println!(
            "after the first frame: {} disc reads, {} archive reads, {} texture decodes, {bytes}",
            after.reads, after.inflates, after.decodes
        );
    }
    let at = world.seats[0].sim.rider.ride.position();
    println!(
        "ridden for {:.1}s over {} frames, ending at {:.2}, {:.2}, {:.2}",
        started.elapsed().as_secs_f32(),
        engine.frame,
        at.x(),
        at.y(),
        at.z()
    );
    Ok(leave)
}

/// Where a view hears from (P42): its camera, with its right the way the picture's is.
fn listener(c: &Camera) -> audio::Listener {
    let forward = c.at.sub(c.eye).norm();
    let right = v3(0.0, 1.0, 0.0).cross(forward).norm();
    audio::Listener { at: [c.eye.x, c.eye.y, c.eye.z], right: [right.x, right.y, right.z] }
}

/// What one seat's frame sounds like (P42): the hands and the engine on the bike, the rider's fall
/// and landing, or the plane's engine note when the plane is up.
fn seat_sound(seat: &Seat) -> audio::SeatFrame {
    let r = &seat.sim.rider.ride;
    let p = r.position();
    let v = r.chassis.vel;
    let plane = match (seat.garage.up, &seat.garage.plane) {
        (Kind::Biplane, Some(plane)) => {
            let note = plane.flight.as_ref().map_or(0.0, |f| f.engine.note.0 / f.engine.max_rpm.0.max(1.0));
            Some((note, [plane.at.x(), plane.at.y(), plane.at.z()]))
        }
        _ => None,
    };
    audio::SeatFrame {
        hands: audio::Hands {
            grounded: r.air.on_ground,
            gas: r.held.gas.0,
            // The lever (`+0x644`), not the clutch the engine is driven through: that one is in
            // whenever the bike stands, and the note would rev at every standstill.
            clutch: seat.hands.clutch.0,
            speed: r.chassis.speed.0,
            at: [p.x(), p.y(), p.z()],
            vel: [v.x(), v.y(), v.z()],
        },
        vy: v.y(),
        wreck: seat.said.wreck.map(|w| w.0 as i32),
        landed_trick: seat.said.landed.is_some(),
        plane,
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
/// The renderer that draws the ride (P44): the offscreen one with no window, the window's otherwise.
fn renderer_of<'a>(
    offscreen: &'a mut Option<(Renderer, Option<wgpu::Texture>)>,
    window: &'a mut Option<Screen>,
) -> Option<&'a mut Renderer> {
    match offscreen {
        Some((r, _)) => Some(r),
        None => window.as_mut().and_then(|w| w.gpu()).map(|w| &mut w.renderer),
    }
}

/// The whole of a `width` by `height` offscreen texture as a frame's target.
fn offscreen_target(view: &wgpu::TextureView, width: u32, height: u32) -> Target<'_> {
    Target { view, size: (width, height), frame: [0, 0, width, height], scale: 1 }
}

fn vector(v: Vf) -> V3 {
    v3(v.x(), v.y(), v.z())
}

/// A camera file's presets as a view in `layout` reads them on `display`: the layout's own copy of
/// each record (`Preset1h`, `Preset1v`, `Preset1q`, or at 16:9 `Preset1w`, `Preset1x`, `Preset1z` —
/// [`aspect::preset_suffix`]), or the full screen's where the file has no such copy. The full screen
/// on the television reads exactly what it always has.
fn layout_presets(bytes: &[u8], layout: Layout, display: Display, file: &str) -> Vec<Preset> {
    let suffix = aspect::preset_suffix(layout, display);
    let own = Preset::read_bytes(bytes, suffix);
    if suffix.is_empty() {
        return own;
    }
    // A record the file does not have reads as a preset with no name and no distance, which is what
    // a missing copy looks like.
    let has = own.iter().filter(|p| !p.name.is_empty()).count();
    let on = format!("{} on a {} display", layout.name(), display.name());
    if has == 0 {
        println!("camera: {file} has no Preset1{suffix} copies for {on}; riding the full screen's");
        return Preset::read_bytes(bytes, "");
    }
    println!("camera: {file}'s Preset{{n}}{suffix} copies, {has} of {}, for {on}", own.len());
    own
}

/// Every player's cameras onto `display`'s copy of their presets (V24): the file read again for it,
/// and the preset each camera is on re-set from the new copy — the original's own reload when a
/// camera's split changes, which keeps the preset and clears the player's nudges.
fn set_display(rigs: &mut [Rig], layout: Layout, display: Display, bike: Option<&[u8]>, plane: Option<&[u8]>) {
    let reload = |cam: &mut Race, presets: &[Preset]| {
        if presets.is_empty() || cam.cam.presets.is_empty() {
            return;
        }
        let at = cam.cam.preset.min(presets.len() - 1);
        cam.cam.presets = presets.to_vec();
        cam.cam.preset = at;
        cam.cam.previous = at;
        cam.cam.set_preset(at);
    };
    let bike = bike.map(|b| layout_presets(b, layout, display, "cpresets.ini")).unwrap_or_default();
    let plane = plane.map(|b| layout_presets(b, layout, display, PLANE_PRESETS)).unwrap_or_default();
    for rig in rigs {
        reload(&mut rig.camera, &bike);
        if let Some(cam) = &mut rig.plane_camera {
            reload(cam, &plane);
        }
    }
}

/// The terrain file a scene asks for. Most scenes name it `SX02.trn`, but what the archive holds is
/// `sx02.ps2.trn`: the loader tags the file with the platform it was built for, and a few scenes
/// already spell the tag out.
/// The ground under a path car (V41), as the ride's own query answers it: the point on the surface
/// and the surface's normal. Off the terrain the point stays where it is, standing straight up.
fn car_ground(world: &mut dyn sim_world::World, p: V3) -> (V3, V3) {
    let (on, normal) = world.ground(Vf::new(p.x, p.y, p.z, 1.0));
    if on.y() < -1.0e5 {
        return (p, v3(0.0, 1.0, 0.0));
    }
    (v3(p.x, on.y(), p.z), v3(normal.x(), normal.y(), normal.z()))
}

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
/// The game modifiers a ride loads with (P55): a lobby ride's are its host's, sent in the room
/// before the start; a `--play` tape's are the ones it was recorded on, so it replays as it rode;
/// otherwise `--modifiers`' or the Settings screen's.
fn ride_modifiers(args: &Args, room: Option<&openmx_net::room::Room>) -> Result<game_sim::modifiers::Mods, String> {
    use game_sim::modifiers::Mods;
    if let Some(room) = room {
        return Mods::parse(&room.modifiers).map_err(|e| format!("the host's modifiers: {e}"));
    }
    if let Some(path) = &args.play {
        let tape = read_tape(path, args.phase)?;
        if !args.modifiers.is_original() && args.modifiers != tape.modifiers {
            println!("modifiers: the tape's own, not --modifiers'");
        }
        return Ok(tape.modifiers);
    }
    Ok(args.modifiers)
}

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
                println!("  emitter {} ({}) on {on}: not in a moment, the ride emits it (P58)", e.emitter, e.name);
            }
        }
        None => println!("  no {} in these archives", biplane::PARTICLES),
    }
    Ok(plane)
}

/// The plane's particles (P58): the emitter list out of the track's archives and each `.emt` it names
/// beside it, the track's particle sheet (`particle\<track>particles.prt` and its texture), and where
/// each entry's node is on the plane. Says what it found, and what it skips.
fn load_plane_fx(
    archives: &[Res],
    track: &str,
    drawn: &Biplane,
    assets: &Assets,
) -> Result<(particles::PlaneFx, Vec<(String, M4)>, game_render::pst::Texture), String> {
    let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(&format!("\\{name}")).ok());
    let list = read(biplane::PARTICLES).ok_or(format!("no {}", biplane::PARTICLES))?;
    let sheet = read(&particles::sheet(track)).ok_or(format!("no {}", particles::sheet(track)))?;
    let texture = read(&particles::sheet_texture(track)).ok_or(format!("no {}", particles::sheet_texture(track)))?;
    let texture = game_render::pst::Texture::parse(&texture)?;
    let atlas = particles::Atlas::parse(&ini::text(&sheet), texture.width, texture.height);
    let fx = particles::PlaneFx::new(&biplane::emitters(&ini::text(&list)), atlas, read);
    let mut binds: Vec<(String, M4)> = Vec::new();
    for e in &fx.entries {
        match e.node.as_deref() {
            Some(name) => match drawn.node(assets, name) {
                Some(m) if !binds.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)) => binds.push((name.to_string(), m)),
                Some(_) => {}
                None => println!("biplane: particles: {} rides {name}, which the model does not have", e.name),
            },
            None => {}
        }
        println!("biplane: particles: {} ({:?}) from {:?}", e.name, e.kind, e.emt.group);
    }
    for (name, why) in &fx.skipped {
        println!("biplane: particles: {name} ({why}) not ported");
    }
    Ok((fx, binds, texture))
}

/// What the plane's particles read off it this frame (P58): its emitters' nodes where the plane is,
/// its throttle, airspeed and wreck, and its body's contacts — none: the port's plane has no
/// collision model to touch the ground with until S48, so the wreck's dust is never fed. Its three
/// tires as the tire dust reads them (P60): the ground under the bottom of the tire, the held
/// contact (only ever cleared, so never an object), the strut's compression and the surface under it.
fn plane_fx_frame(plane: &Plane, binds: &[(String, M4)]) -> particles::PlaneFrame {
    let at = plane_at(plane);
    let (throttle, airspeed) = plane.gauges().map_or((0.0, 0.0), |(t, a)| (t.0, a.0));
    particles::PlaneFrame {
        nodes: binds.iter().map(|(n, m)| (n.clone(), m.mul(at))).collect(),
        throttle,
        airspeed,
        wrecked: plane.prop_angle().is_none(),
        vel: vector(plane.vel),
        ground_contacts: Vec::new(),
        object_contacts: Vec::new(),
        tires: plane.flight.as_ref().map_or_else(Vec::new, |f| {
            f.tires
                .iter()
                .map(|t| particles::TireFrame { ground: t.below, object: t.held, compression: t.compression.0, zone: t.surface })
                .collect()
        }),
    }
}

/// The ground under a point for the plane's particles (P60): its height and its surface, from the
/// ride's own terrain query. `None` off the terrain, where the query answers its no-ground depth.
fn plane_fx_ground(place: &mut impl sim_world::World, at: V3) -> Option<(f32, u8)> {
    let p = Vf::new(at.x, at.y, at.z, 1.0);
    let (on, _) = place.ground(p);
    (on.y() > -1.0e5).then(|| (on.y(), place.surface(on)))
}

/// The plane's particles as the GPU draws them.
fn plane_sprites(fx: &particles::PlaneFx) -> Vec<game_render::gpu::Particle> {
    fx.drawn()
        .iter()
        .map(|d| game_render::gpu::Particle { at: [d.at.x, d.at.y, d.at.z], size: d.size, uv: d.uv, alpha: d.alpha, _pad: [0.0; 3] })
        .collect()
}

/// The plane's particles as the software path draws them (P62).
fn plane_particles(fx: &particles::PlaneFx) -> Vec<game_render::particle::Particle> {
    fx.drawn()
        .iter()
        .map(|d| game_render::particle::Particle { at: [d.at.x, d.at.y, d.at.z], size: d.size, uv: d.uv, alpha: d.alpha })
        .collect()
}

/// This frame's particles into `r`, the sheet first if it has not been uploaded yet.
fn set_plane_sprites(r: &mut Renderer, sheet: &mut Option<game_render::pst::Texture>, sprites: &[game_render::gpu::Particle]) {
    if let Some(sheet) = sheet.take() {
        r.set_sprite_sheet(&sheet);
    }
    r.set_sprites(sprites);
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
        overlay: None,
        speedometer: None,
        camera: START_CAMERA.to_string(),
        moment: None,
        wrecks: true,
        modifiers: game_sim::modifiers::Mods::original(),
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
        pads: None,
        controls: None,
        scheme_flag: false,
        plane_flag: false,
        preset_flag: false,
        vehicle: Kind::Bike,
        players: 1,
        split: Split::default(),
        aspect: Aspect::default(),
        headless: false,
        frames: None,
        draw_fps: None,
        online: None,
        peers: Vec::new(),
        seats: None,
        set: None,
        others: Vec::new(),
        garage: false,
        pad_of: Vec::new(),
        sound: true,
        timings: false,
        display: std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string()),
        menu: false,
        ride_flag: false,
        screen: None,
        wiring_flags: (false, false),
        fps: false,
        renderer: Backend::default(),
        graphics: None,
        render_scale: None,
        render_scale_asked: false,
    };
    let mut argv = std::env::args().skip(1);
    while let Some(a) = argv.next() {
        let mut next = |what: &str| argv.next().ok_or(format!("{a} wants {what}"));
        // The flags that pick a ride, which is what the shell did before it had a front end.
        if matches!(
            a.as_str(),
            "--track" | "--play" | "--record" | "--moment" | "--vehicle" | "--phy" | "--ride" | "--headless"
                | "--players" | "--frames" | "--online" | "--set" | "--garage"
        ) || a.starts_with("--trace")
        {
            args.ride_flag = true;
        }
        match a.as_str() {
            "--ride" => {}
            "--menu" => args.menu = true,
            "--screen" => {
                let w = next("a screen: main, tracks, garage, settings, controls, keyboard or pause")?;
                args.screen = Some(menu::Screen::from_word(&w).ok_or(format!(
                    "--screen: main, tracks, garage, settings, controls, keyboard or pause, not {w}"
                ))?);
                args.menu = true;
            }
            "--track" => args.track = next("a track name, like free06")?,
            "--bike" => args.bike = next("a bike name, like 125sx")?,
            "--rider" => args.rider = next("a rider name, like thq1, or none")?,
            "--pad" => args.pad = Some(next("a device, like /dev/input/js0")?),
            "--pads" => {
                args.pads = Some(
                    pad::Seat::parse_list(&next("seats, like k,0,1")?)
                        .map_err(|why| format!("--pads: {why}"))?,
                );
            }
            // A layout (P40), or as before P40 the bindings file, which `--bindings` also names.
            "--controls" => {
                let v = next("new, original, one per seat (new,original), or a bindings file")?;
                match Scheme::parse_list(&v) {
                    Ok(schemes) => {
                        args.scheme_flag = true;
                        args.wiring.schemes = schemes;
                    }
                    Err(_) if v.split(',').all(|w| Scheme::from_word(w).is_some()) => {
                        return Err(format!("--controls: {v}: at most {SEATS} seats"));
                    }
                    Err(_) => args.controls = Some(v.into()),
                }
            }
            // How the biplane's stick reads (P50, P69), per seat like --controls.
            "--biplane-controls" => {
                let v = next("new, screen, original, or one per seat (new,original)")?;
                args.wiring.planes = PlaneScheme::parse_list(&v).map_err(|why| format!("--biplane-controls: {why}"))?;
                args.plane_flag = true;
            }
            // The original's controller preset under its layout (P79), per seat like --controls.
            "--preset" => {
                let v = next("a controller preset from 0 to 3, or one per seat (0,2)")?;
                args.wiring.presets = Scheme::parse_presets(&v).map_err(|why| format!("--preset: {why}"))?;
                args.preset_flag = true;
            }
            "--bindings" => args.controls = Some(next("a bindings file")?.into()),
            "--shot" => args.shot = Some(next("a path for the first frame")?.into()),
            "--shot-frame" => {
                args.shot_frame = next("which frame to write")?
                    .parse()
                    .map_err(|_| "--shot-frame: not a frame number")?;
            }
            "--hud" => {
                args.hud = true;
                args.overlay = Some(menu::Overlay::Debug);
            }
            "--overlay" => {
                let word = next("original, debug or none")?;
                let overlay = menu::Overlay::from_word(&word)
                    .filter(|o| o.word() == word)
                    .ok_or_else(|| format!("--overlay: {word:?} is not original, debug or none"))?;
                args.hud = overlay == menu::Overlay::Debug;
                args.overlay = Some(overlay);
            }
            "--speedometer" => {
                let word = next("digital or analog")?;
                let style = menu::Speedometer::from_word(&word)
                    .ok_or_else(|| format!("--speedometer: {word:?} is not digital or analog"))?;
                args.speedometer = Some(style);
            }
            "--fps" => args.fps = true,
            "--graphics" => {
                let word = next("original or full")?;
                let graphics = Graphics::from_word(&word)
                    .ok_or_else(|| format!("--graphics: {word:?} is not original or full"))?;
                args.graphics = Some(graphics);
            }
            "--renderer" => args.renderer = Backend::parse(&next("gpu or software")?)?,
            "--render-scale" => {
                let v = next("auto, or a fraction of the frame like 0.75")?;
                args.render_scale_asked = true;
                args.render_scale = match v.as_str() {
                    "auto" => None,
                    _ => Some(
                        v.parse::<f32>()
                            .ok()
                            .filter(|f| (game_render::gpu::MIN_SCALE..=1.0).contains(f))
                            .ok_or_else(|| format!("--render-scale: auto, or a fraction from {} to 1, not {v:?}", game_render::gpu::MIN_SCALE))?,
                    ),
                };
            }
            "--camera" => args.camera = next("a camera preset's name, like \"Freestyle Cam\"")?,
            "--moment" => {
                let file: std::path::PathBuf = next("a moment file")?.into();
                let out: std::path::PathBuf = next("a path for the frame")?.into();
                args.moment = Some((file, out));
            }
            "--display" => args.display = next("a display, like :0")?,
            "--no-wrecks" => args.wrecks = false,
            "--modifiers" => {
                args.modifiers =
                    game_sim::modifiers::Mods::parse(&next("modifiers, like gravity=50")?).map_err(|e| format!("--modifiers: {e}"))?
            }
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
                args.wiring_flags.0 = true;
                args.wiring.throttle = match next("cross, r2 or l2")?.as_str() {
                    "cross" => Throttle::Cross,
                    "r2" => Throttle::Trigger(Control::R2),
                    "l2" => Throttle::Trigger(Control::L2),
                    other => return Err(format!("--throttle: cross, r2 or l2, not {other}")),
                }
            }
            "--pad-pressure" => {
                args.wiring_flags.1 = true;
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
            "--players" => {
                let n = next("a number of players, 1 to 8")?;
                args.players = n.parse().map_err(|_| format!("--players: {n:?} is not a number"))?;
                Layout::for_players(args.players, args.split)?;
            }
            "--split" => args.split = Split::parse(&next("h or v")?)?,
            "--aspect" => args.aspect = Aspect::parse(&next("fill or 4:3")?)?,
            "--headless" => args.headless = true,
            "--no-sound" => args.sound = false,
            "--timings" => args.timings = true,
            "--frames" => {
                args.frames = Some(
                    next("a number of frames")?.parse().map_err(|_| "--frames: not a number of frames")?,
                )
            }
            "--draw-fps" => {
                let fps: f32 = next("pictures a second")?.parse().map_err(|_| "--draw-fps: not a number")?;
                if !(fps > 0.0) {
                    return Err("--draw-fps: more than none a second".into());
                }
                args.draw_fps = Some(fps);
            }
            "--online" => args.online = Some(next("this machine, like host@0.0.0.0:7000")?),
            "--peer" => args.peers.push(next("another machine, like g@192.168.1.20:7000")?),
            "--seats" => args.seats = Some(next("who sits where, like host,g,g")?),
            "--phy" => args.phy = Some(next("a .phy on the disc, like race125p.phy")?),
            "--set" => {
                let w = next("one of the sixteen sets, like race250a")?;
                args.set = Some(roster::Set::from_word(&w).ok_or(format!("--set: like race125p or pro500a, not {w}"))?);
            }
            "--garage" => args.garage = true,
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
                    "openmx [--menu [--screen main|tracks|settings|controls|keyboard|pause]]\n\
                            [--ride] [--track free06] [--bike 125sx] [--rider thq1] [--size 640x448]\n\
                            [--scale 1]\n\
                            [--pad /dev/input/js0] [--display :0] [--seconds N] [--shot PATH]\n\
                            [--moment FILE OUT.png] [--no-wrecks] [--overlay original|debug|none] [--speedometer digital|analog] [--hud] [--camera NAME]\n\
                            [--modifiers 'gravity=50 bike_speed.2=200']\n\
                            [--shot-frame N] [--vehicle bike|biplane]\n\
                            [--players 1..8] [--split h|v] [--headless] [--frames N] [--timings]\n\
                            [--draw-fps N]\n\
                            [--record TAPE] [--play TAPE|RECORDING.p2m2 [--phase second]]\n\
                            [--throttle cross|r2|l2] [--pad-pressure ds2@8|AXIS:CONTROL,...]\n\
                            [--pads k,0,1,-] [--controls new|original|new,original,...]\n\
                            [--biplane-controls new|screen|original|new,original,...]\n\
                            [--preset 0..3|0,2,...]\n\
                            [--bindings FILE] [--renderer gpu|software] [--graphics full|original] [--fps]\n\
                            [--render-scale auto|0.5..1]\n\
                            [--no-sound]\n\
                     \n\
                     Opens the front end, or with --track (or --ride, --play, --record, --trace...)\n\
                     rides the freeride area from the disc named in kit.toml straight away.\n\
                     The front end keeps settings.txt and controls.txt beside the disc.\n\
                     R2 throttle, L2 brake, each on its trigger's travel, Select changes camera;\n\
                     Cross and Square are kept free. --controls original (or the front end's\n\
                     Controls screen, per seat) rides the original's instead: Cross throttle,\n\
                     Square brake, R2 front brake, L2 clutch, L1 changes camera. Either way the\n\
                     left stick steers\n\
                     and leans, the d-pad preloads, L3 balances, R3 reverses, Start freezes,\n\
                     Escape quits. --camera starts on a camera preset by its\n\
                     name in the track's cpresets.ini (\"Race Cam 1\" unless asked).\n\
                     A trick is a gesture: hold Circle, Triangle and/or R1 and push the d-pad.\n\
                     L1 and R1 together put the bike back (the original's own chord).\n\
                     The biplane's stick is the plane's own: down pulls the nose toward its\n\
                     belly and left rolls it left, at every bank and upside down too.\n\
                     --biplane-controls original (or the Controls screen) flies the original's,\n\
                     whose stick stops pitching the plane on its side; --biplane-controls screen\n\
                     swaps the stick's up and down, Up and Down, and L1 and R1 upside down, so\n\
                     they move the plane the same way on the screen.\n\
                     --preset 1..3 (or the Controls screen, under Original) rides the original\n\
                     layout on one of the original's other controller presets; 0 is the shipped one.\n\
                     On the keyboard, under the new layout: WASD the left stick (A/D steer, W/S\n\
                     lean), YUIO the right stick, Space gas, Shift brake, Q/E L1/R1, Tab or C\n\
                     the camera, arrows the d-pad, HJKL the face buttons (H Square, J Cross,\n\
                     K Triangle, L Circle), Enter Start, Escape the pause. Under the original's (--controls original): X throttle, Z brake,\n\
                     arrows steer, IJKL the right stick, C/V/D trick, A+D reset.\n\
                     The throttle is a pressure and not a switch (the game reads Cross from its\n\
                     pressure byte): a key ramps it, --throttle r2 rides it on a trigger instead,\n\
                     giving up that trigger's own control, and --pad-pressure says which joystick\n\
                     axes a pad reports its buttons' own pressures on.\n\
                     F3 toggles the debug overlay (and H, where H is not a ride's key), which\n\
                     --hud (--overlay debug) starts on.\n\
                     Up to eight seats: every pad plugged in takes the next one, the keyboard\n\
                     beside 1P's, or --pads says each seat's (k the keyboard, a number the pad\n\
                     in the order found, - nobody). A pad pulled out pauses its player until it\n\
                     is plugged back in; Start on the keyboard rides on without it. The buttons\n\
                     are rebindable in --bindings (context/controls.txt, written on first run).\n\
                     --vehicle biplane starts in the track's biplane, where its machine.txt\n\
                     parks one, and M swaps between the bike and the plane the way the\n\
                     original's \"Switch To Machine\" does. The plane flies over the terrain\n\
                     alone: Cross opens the throttle and Square closes it.\n\
                     --players N splits the screen between N local players (up to 8), each on\n\
                     a pad of their own with a camera and an overlay of their own: two one above\n\
                     the other (--split h, the default) or side by side (--split v), the\n\
                     original's two layouts, and three to eight in quarters or eighths. The first\n\
                     player has the keyboard, the plane and the tape. --headless draws with no\n\
                     window and no pad, and --frames N quits after N frames. --timings prints\n\
                     what each stage of a frame cost, a line a second.\n\
                     The ride runs 30 ticks a second, as the original does, however fast it is\n\
                     drawn: a window draws as fast as its display takes pictures, a headless\n\
                     ride 60 a second, and --draw-fps N at most N.\n\
                     The window opens at the screen's size and the frame fills it, re-made when\n\
                     the window is resized, wider or taller as the original's own 16:9 mode\n\
                     would draw it; --aspect 4:3 draws the largest 4:3 frame inside it instead,\n\
                     with bars. --size draws a fixed frame (640x448 is the PS2's), and --scale\n\
                     draws each frame pixel N times across and down. --render-scale draws the\n\
                     3D at a fraction of the frame and stretches it over, the HUD and menus\n\
                     still sharp; auto, the default, lowers it while the GPU misses the\n\
                     display's refresh and raises it again when there is room. Without it,\n\
                     the front end's Settings row (Auto, 100%, 75%, 50%) says.\n\
                     --no-wrecks turns the session's wrecks off (a wreck otherwise throws the\n\
                     rider, tumbles the bike and puts you back on it); --modifiers sets the\n\
                     game modifiers, percentages of what the ride loads (a tape keeps its own);\n\
                     --moment draws one frame posed\n\
                     from a moment of the original and quits; --trace N rides N frames with\n\
                     nobody on the pad and prints the solver's own numbers, and --trace-brake\n\
                     or --trace-gas hold the brakes or the throttle while it does, --trace-stick X\n\
                     holds the stick over and --trace-feel prints a short line a frame instead;\n\
                     --trace-air prints the chassis' attitude and its rates instead;\n\
                     --trace-switch N rides the throttle alone for N frames first;\n\
                     --trace-stick-y F:Y,... sets the stick's y from frame F on, each until\n\
                     the next, and adds the backflip's columns to --trace-feel's line;\n\
                     --trace-at X,Z,DEG starts the bike there instead of the start line, and\n\
                     --phy NAME rides the bike archive's NAME (pro125a.phy) instead of Race125P.phy,\n\
                     --set WORD the bike's own file of that set (race250a, pro500p) on its class,\n\
                     and --garage rides every seat on what the front end's garage picked for it.\n\
                     --record writes every simulated frame's pad to a tape (under context/,\n\
                     it is game data); --play rides one instead of the pad, at the tape's own\n\
                     frame lengths, and quits when it runs out. A human's PCSX2 recording\n\
                     of the original (.p2m2) is a tape too and rides the same; --phase says\n\
                     which of a tick's two video fields that recording's game read.\n\
                     \n\
                     Sound plays through pw-cat, pacat or aplay, whichever is found: every\n\
                     rider's engine, wrecks and landings, the track's own sounds, and music —\n\
                     the disc's songs, then any .wav in the music folder beside the settings,\n\
                     picked in Settings. --no-sound turns it off; a headless ride has none.\n\
                     \n\
                     Online: --online NAME@HOST:PORT --peer NAME@HOST:PORT ... --seats NAME,NAME,-\n\
                     rides with the other machines over UDP, one bike a seat (up to eight; the\n\
                     first named is the host). This machine's pad rides its first seat, and its\n\
                     other seats stand idle until there are more local pads. Online there is no\n\
                     freeze, no swap and no tape; a desync is written under context/net."
                );
                return Ok(None);
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Some(args))
}
