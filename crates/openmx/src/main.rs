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
//! track pick, the Garage (P37: every seat's bike, set and rider), Settings with the rider's name on an on-screen keyboard, the overlay, wrecks and each
//! seat's controls (P81), and Quit. Start or Escape in a ride opens the pause — Resume, Restart, Settings,
//! Main Menu, Quit — in place of the plain freeze Start is without the front end. `--menu` asks for
//! the front end whatever else is given, and `--screen NAME` opens it on one screen, or on one of
//! Settings' or the Garage's tabs by its name (`display`, `gear`; P123), which is how a
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
//! Cross is the throttle and Square the brake (switches on the bike, as the original's are: any press
//! is the whole lever, P77), L2 is the
//! clutch and R2 the front brake on its own, and the left stick steers and leans. The right stick
//! turns the camera round the rider, the original's look-around, and with L3 held moves it in and out
//! (P114); clicking it (R3) puts the camera back where its preset has it, which is the port's own —
//! the original binds R3 to nothing it reads. Reverse is the original's: gas and brake together below
//! 5 ft/s, under either layout (`game_sim::ride::Controls::reverse_rule`, P72); at speed the two
//! together are the brake, and the rider backing up plays the reverse pair.
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
//! and Square kept free for later, L1 the clutch as in the original's, and the L1+R1 reset pressed
//! together so a clutch held into a trick on R1 is not one (P67). Each seat picks one (`--controls
//! new|original`, or one per seat, and the front end's Controls screen). The new one is a remap of the pad's report in [`pad`]
//! ([`pad::Scheme::ride`]) before the engine's pad port sees it, so the simulation still reads the
//! original's controls, a tape holds what it read, and under the original's every replay is as it was.
//!
//! **The bike's throttle is a switch** (P77): the original's bike takes any strength of its gas, brake
//! and clutch commands as 1.0, and its limiter walks the lever up at the bike's rate, so under either
//! layout a part-pressed Cross or R2 is the whole throttle. **The biplane's throttle is a pressure**,
//! because the game reads Cross from its pressure byte there: part throttle is a fraction of that
//! byte. What has to be arranged is a host that can *ask* for a fraction (P9, [`pad`]) — almost no pad
//! sold since the DualShock 2 reports a face button as anything but a bit. So: `--pad-pressure` says
//! which joystick axes a device publishes its buttons' own pressures on, where it publishes them;
//! `--throttle r2` rides gas on a trigger instead, which is travel every pad has, giving up that
//! trigger's own control for the session; and the keyboard's throttle key ramps open over a third of
//! a second rather than stepping, so a stab is part throttle on a machine with no pad at all.
//!
//! Over each view is the ride HUD ([`ride_hud`], P121): the original's pieces at the original's
//! places — score, trick pop-up, speedometer, radar with every other rider on it — redrawn as vector
//! shapes and our own type, sharp at any size, many of them laid out by [`overlay`]'s (V26) reading
//! of the disc. The original's own HUD and the debug HUD are gone (P170): `--overlay original`,
//! `--overlay debug` and `--hud` are refused. `--overlay none`, or the front end's setting, draws
//! none. The speed is its digital readout unless
//! `--speedometer analog` or the Settings' Speedometer row asks for its dial (V42). `--minimap off` or the
//! Settings' Minimap row drops its radar, and `--hud-scale` or the HUD scale row sizes every part of
//! the HUD about its own corner (V52).
//!
//! **M** swaps between the bike and the track's biplane, as does the pause's "Switch To Machine" for
//! the player who opened it, and `--vehicle biplane` or the ride setup's machine pick starts in the
//! plane ([`vehicle`], P19, P41) — online too, every seat in its own bay at tick 0 on every machine
//! (N18). The switch is the port's own (P102): the machine taken is spawned
//! where the player is — a plane landed on its wheels where the bike was, a bike upright on the
//! ground under the plane — and the machine left goes, not drawn and not stepped; a plane nobody has
//! taken stands parked in its bay. The plane has its own camera file. A tape writes the switch down
//! at its tick (`switch <frame> <machine>`) and rides it there again. With the plane up it flies (P31): its own simulation steps every frame on the seat's
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

// The Android app's one exception (P308): `android.rs`'s `android_main`, the symbol the
// NativeActivity looks up by name, which `#[unsafe(no_mangle)]` makes. Every JNI call is in
// `openmx-android`.
#![cfg_attr(not(target_os = "android"), forbid(unsafe_code))]
#![cfg_attr(target_os = "android", deny(unsafe_code))]

// The shell's modules, crate-visible: the browser's build (P196, `web.rs`) mounts this file as a module of
// its own and takes them up to its root, where their `crate::` paths look for them.
macro_rules! modules {
    ($($(#[$m:meta])* mod $name:ident;)*) => { $($(#[$m])* pub(crate) mod $name;)* };
}
modules! {
mod audio;
mod aspect;
mod chase;
// The clock, the threads and the shell's files, which the browser has its own of (P196).
mod clock;
mod files;
mod thread;
// The fixed update on a thread of its own (N23a).
mod sim_thread;
// Whether the mouse cursor is drawn over the window (P167).
mod cursor;
// The riders' names and numbers on their jerseys and plates (P86).
mod dress;
// The browser's GPU window is the page's canvas (P48).
#[cfg_attr(target_arch = "wasm32", path = "web_gpu.rs")]
#[cfg_attr(target_os = "ios", path = "ios_gpu.rs")]
#[cfg_attr(target_os = "android", path = "android_gpu.rs")]
mod gpuwin;
// The iPhone app (P200).
#[cfg(target_os = "ios")]
mod ios;
// The Android app (P308).
#[cfg(target_os = "android")]
mod android;
// The Windows build's pads and sound (P192).
#[cfg(windows)]
mod win;
mod ground;
mod hud;
// What the CPU still draws over the GPU's picture (P44), as patches.
mod loading;
mod lobby;
// The join screen (P38's rules), which the front end's local game seats its players through (P43).
mod join;
mod menu;
// The mod store (P198): Browse and Install from the public dataset.
mod modstore;
mod moment;
mod net;
mod online;
mod peer_looks;
mod garage_view;
// The original's own HUD (V26) is drawn by none of the shell since P170, which took it off the
// menu; its drawing stays, held by tests/overlay.rs, beside the layouts and readings the ride HUD
// takes from it.
#[allow(dead_code)]
mod overlay;
mod pace;
mod pad;
// A desktop build a player can start (P190): the disc from the command line, kit.toml, the path
// remembered or a picker, and the per-user data folder.
#[cfg(not(target_arch = "wasm32"))]
mod player;
// The menu's own disc browser (P220), for a player's start with no dialog program and no terminal.
#[cfg(not(target_arch = "wasm32"))]
// Android picks its disc with the system's picker (`android.rs`).
#[cfg_attr(target_os = "android", allow(dead_code))]
mod disc_browser;
mod particles;
mod plane_draw;
mod kiowa_draw;
mod car_draw;
mod perf;
mod portal;
mod ride_hud;
// N8: the one-seat online ride against the offline one, tick for tick.
#[cfg(test)]
mod same_ride;
// P298: where the player was when the app went away, offered on the next launch.
mod resume;
// The desktop updater (P191): a newer release found, downloaded, checked and swapped in.
#[cfg(not(target_arch = "wasm32"))]
mod update;
mod saved_markers;
// MD10: `--shot-pose`, the first view's camera held where the accuracy harness asks.
mod shot_pose;
mod split;
mod disc_worlds;
mod stages;
mod tire_tracks;
mod weather;
mod textures;
mod track_mods;
mod track_previews;
mod trick_packs;
mod trick_view;
mod machine_packs;
mod music_packs;
mod worlds;
mod world_dress;
mod paint_stream;
// The garage (P37). Its screen hangs off the front end (P36), which is not in this tree yet; until it
// is, the `.phy` lookup is the only part of it the shell calls.
#[allow(dead_code)]
mod roster;
mod screen;
mod touch_pad;
mod vehicle;
mod window;
}
// The phone's pads and touch pad, for the pad layer: the iPhone's or the Android phone's.
#[cfg(target_os = "ios")]
use ios as phone;
#[cfg(target_os = "android")]
use android as phone;

use std::sync::Arc;
use std::time::Duration;

use clock::Instant;

use game_render::biplane::{self, Biplane};
use game_render::kiowa::{Kiowa, Rotor};
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
use game_sim::markers::{self, Spot};
use game_sim::phy::{self, Phy};
use game_sim::replay::{Head, PadTape, Recorder, TickPhase};
use game_sim::ride::{self, Controls};
use game_sim::rider::{self, Rider};
use game_sim::rng::Rng;
use game_sim::state::Phase;
use game_sim::tick::{Engine, Game, Outcome};
use game_sim::trick::{Set, Stunts};
use game_sim::tiles::Land;
use game_sim::world::Place;
use game_sim::{Input, Sim, track, world as sim_world};
use ps2_float::{F, Vf};

use ground::Ground;
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

/// How long a world's piece takes to fade in over its stand-in when it streams in mid-ride, and out
/// when it is let go (V92), in seconds.
const GROUND_FADE: f32 = 1.0;

/// How far before a world's dressing reach its props and plants fade out to nothing (V92), in feet.
const PROP_FADE_BAND: f32 = 1500.0;

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
/// The helicopter's camera preset a flight starts on (P216): the first of
/// [`game_render::kiowa::PRESETS`]'s four, as [`PLANE_CAMERA`] is the plane's first.
const HELI_CAMERA: &str = "Aero Cam 1";
/// The preset a car's own camera starts on (S246), in each of the three cars' files.
const CAR_CAMERA: &str = "Race Cam 1";

/// The archive a ride on a track without the plane's own camera presets and particle list reads them
/// from (P113): a freeride track's second archive carries both. The particle sheet stays the track's.
const PLANE_KIT_ARCHIVE: &str = "free06_2.res";
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

#[cfg_attr(target_os = "ios", allow(unreachable_code))]
// On Android the shell is a library the activity loads, and `android_main` its entry.
#[cfg_attr(target_os = "android", allow(dead_code))]
fn main() {
    launched();
    // The iPhone runs UIKit on this thread and the shell on another (P200).
    #[cfg(target_os = "ios")]
    return ios::main();
    // `cargo run -p openmx` without `--release` is the dev profile, opt-level 1: several times
    // slower a frame, which is not what the shell costs (P47).
    if cfg!(debug_assertions) {
        eprintln!("openmx: a debug build, slow to draw; `cargo run --release -p openmx` is the one to ride");
    }
    // The loop is a future (P196) so the browser can step it a frame at a time; here it is waited
    // on whole, boxed for its size.
    if let Err(e) = pollster::block_on(Box::pin(run())) {
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
///
/// Cloned for the picture drawn while a tick runs on the sim thread (N23a).
#[derive(Clone)]
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
    /// Whether this player has their own pause open (P84), offline off the front end's menu and online
    /// off the tick's pads: frozen where they are, drawn see-through ([`vehicle::PAUSED_ALPHA`]) and
    /// passed through, while every other player rides on.
    paused: bool,
    /// What the rider did this frame, kept so the loop can print it after the pass.
    said: rider::Frame,
    /// Whether the plane was upside down when this frame's report was made, which turns the
    /// biplane's stick round under the screen's biplane controls (P50, `pad::PlaneScheme::fly`).
    inverted: bool,
    /// The hands this pass was stepped with, kept for the body's own animation frame — which runs
    /// after the pass, off the same controls and the same `dt` (V18).
    hands: Controls,
    /// Whether this pass's marker Up put the bike somewhere else (P112), which the camera rides out
    /// of at the bike's speed rather than flying in from where it was.
    teleported: bool,
    /// How many times the spawn's Down has cleared this player's markers (P135), for the bar to say
    /// Cleared after however many passes a frame runs.
    clears: u32,
    /// The player's Markers switch (P129): on, the d-pad is the session markers' and the left stick
    /// alone rides (`Controls::stick_alone`); off, the d-pad fires no marker and rides as the
    /// original's (`Controls::through` as it is), on the bike and in the plane.
    markers: bool,
    /// The reset chord's two halves must go down together (P67, `Controls::chord_together`): the
    /// new layout's, where L1 is a clutch held through a launch and R1 on it is a trick, not the
    /// put-back. Under the original's, the original's chord.
    together: bool,
    /// The black over this player's view while their wreck's page is up (S85).
    shade: PageShade,
}

/// The viewport's fade to black and back over a wreck's page (S85, [`game_sim::wreck_page::Shade`]):
/// the original's fader as the picture sees it, stepped after each pass with the front end's
/// millisecond clock — the passes' own `dt`s summed, each pass the whole milliseconds the clock moved,
/// 33, 33 and 34 at 30 Hz.
#[derive(Clone, Copy, Default)]
struct PageShade {
    shade: game_sim::wreck_page::Shade,
    seconds: f64,
}

impl PageShade {
    fn step(&mut self, page: &game_sim::wreck_page::Page, dt: F) {
        let before = (self.seconds * 1000.0).floor();
        self.seconds += f64::from(dt.0);
        self.shade.step(page, ((self.seconds * 1000.0).floor() - before) as i32);
    }

    /// The black's alpha over the view: 0 with no page.
    fn alpha(&self) -> u8 {
        self.shade.alpha()
    }
}

/// The page's black over a software view (S85): every pixel toward black by `alpha`.
fn shade_view(frame: &mut Frame, alpha: u8) {
    if alpha == 0 {
        return;
    }
    let keep = 255 - u32::from(alpha);
    for c in frame.color.iter_mut() {
        *c = ((u32::from(*c) * keep + 127) / 255) as u8;
    }
}

#[cfg(test)]
mod page_shade {
    use super::{Frame, PageShade, shade_view};
    use game_sim::wreck_page::Page;
    use ps2_float::F;

    /// The shell's millisecond clock at the 30 Hz pass (S85): 33, 33 and 34, the original's whole
    /// milliseconds, so a page fades 84, 168, black, then 171, 87 and clear.
    #[test]
    fn a_page_fades_by_the_passes_milliseconds() {
        let (mut page, mut shade) = (Page::default(), PageShade::default());
        let dt = F(1.0 / 30.0);
        page.raise();
        let drawn: Vec<u8> = (0..7)
            .map(|_| {
                page.update();
                shade.step(&page, dt);
                shade.alpha()
            })
            .collect();
        assert_eq!(drawn, [84, 168, 255, 171, 87, 0, 0]);
    }

    /// The black over a software view: none at 0, all at 255, and halfway between.
    #[test]
    fn the_black_darkens_the_view() {
        let mut frame = Frame::new(2, 1, [0; 3]);
        frame.color.copy_from_slice(&[200, 100, 0, 255, 255, 255]);
        shade_view(&mut frame, 0);
        assert_eq!(frame.color, [200, 100, 0, 255, 255, 255]);
        shade_view(&mut frame, 128);
        assert_eq!(frame.color, [100, 50, 0, 127, 127, 127]);
        shade_view(&mut frame, 255);
        assert_eq!(frame.color, [0; 6]);
    }
}

impl Seat {
    /// One player's pass: the pad, then the bike, then the d-pad's markers (P88) — `me` is this
    /// seat and `spots` where every seat's rider was before the pass. `among` is the other seats as
    /// this one meets them (N22): its contacts, the riders it can joust and the bikes its plane can
    /// strike.
    fn pass(&mut self, dt: F, place: &mut dyn sim_world::World, me: usize, spots: &[Spot], among: vehicle::Among<'_>) {
        let Some(controls) = self.pass_front(dt) else { return };
        let mut did = None;
        if self.riding {
            (self.said, did) = self.garage.pass_among(&mut self.sim, dt, &controls, me, spots, place, self.markers, among);
        }
        self.pass_back(dt, me, controls, did);
    }

    /// [`Seat::pass`] up to the machine's step: the pad polled and the hands off it, `None` while
    /// the seat waits for its pad (and then the pass is over).
    fn pass_front(&mut self, dt: F) -> Option<Controls> {
        // Nobody's hands on the bars: the pad is not polled either, so the input manager's timers
        // and held buttons are where the pull left them, and the ride goes on from there when the
        // pad comes back.
        if self.waiting {
            self.said = rider::Frame::default();
            self.teleported = false;
            return None;
        }
        // The game's poll, with the frame's dt: this is what turns report bytes into held controls,
        // press and release edges, and stick axes with the game's own dead zone — and then the
        // rider's hands off it, because which button means what lives in the ride and not here.
        // The d-pad is the markers' and not the ride's (P93): the stick alone steers. With the
        // player's markers off (P129) it is the original's, every meaning it has there.
        let mut controls = vehicle::hands(self.sim.poll(dt, &self.reports), self.markers);
        if self.together {
            controls = controls.chord_together(self.sim.input.pad(0), self.sim.table());
        }
        self.garage.set_dpad(!self.markers);

        if self.start_freezes && self.sim.input.pad(0).pressed(Control::Start) {
            self.riding = !self.riding;
        }

        self.said = rider::Frame::default();
        self.teleported = false;
        Some(controls)
    }

    /// [`Seat::pass`] after the machine's step: what its markers did, then the page's fader.
    fn pass_back(&mut self, dt: F, me: usize, controls: Controls, did: Option<markers::Did>) {
        if let Some(did) = did {
            self.teleported = matches!(did, markers::Did::Teleported(_));
            self.clears += u32::from(did == markers::Did::Cleared);
            println!("marker: {}", marker_line(did));
        }
        // The page's fader, after the bike's update as the original's (S85).
        let was = self.shade.alpha();
        self.shade.step(&self.sim.rider.page, dt);
        if self.shade.alpha() != was {
            println!("page: seat {me} alpha {}", self.shade.alpha());
        }
        self.hands = controls;
    }
}

/// How see-through a placed marker's bike is drawn for its owner (P88): faint, a fifth of solid.
const MARKER_ALPHA: f32 = 0.2;

/// The level of detail every rider's body is drawn at: the finest of its five, whatever the
/// camera's distance. The original picks by distance, but a rider is a player's own, and thrown off
/// the edge (S59) the camera falls far enough behind to draw the coarsest.
const RIDER_GROUP: usize = 0;

/// A view's markers' slot display while it is up (P88): the selection, which slots hold a marker,
/// the label, the other players in the ride by seat (P94), how far a Down or Up hold is (P106),
/// whether the spawn has a start to ride to (P132), and whether its Down has just cleared (P135).
type MarkerShown = (markers::Pick, Vec<bool>, String, Vec<usize>, u32, bool, bool);

/// A marker's tick (P88) as the console says it.
fn marker_line(did: markers::Did) -> String {
    use markers::{Did, Pick};
    let what = |p: Pick| match p {
        Pick::Spawn => "the spawn".to_string(),
        Pick::Slot(s) => format!("slot {}", s + 1),
        Pick::Player(p) => format!("P{}", p + 1),
        Pick::More => "the +".to_string(),
    };
    match did {
        Did::Stepped(p) => what(p),
        Did::Placed(s) => format!("placed in slot {}", s + 1),
        Did::Teleported(p) => format!("to {}", what(p)),
        Did::Refused(p) => format!("nothing on {}", what(p)),
        Did::Cleared => "cleared every marker".to_string(),
    }
}

/// The switch (P19, P102) for one player, from the M key, the pause's "Switch To Machine" (P41) or
/// a tape: `to` is spawned where the player is ([`Garage::switch`]) over `place`, the machine left
/// goes, and the machine taken gets its own camera, which starts where it is rather than lagging in
/// from wherever it was last. The machine now up, or `None` (and no switch) where it cannot be had.
fn switch_machine(seat: &mut Seat, rig: &mut Rig, to: Kind, place: &mut dyn sim_world::World) -> Option<Kind> {
    // The car left, as it is left (S149e).
    if let Some(said) = seat.garage.car_up().and_then(vehicle::CarRide::report) {
        println!("{said}");
    }
    // And the helicopter (P216).
    if let Some(said) = seat.garage.heli_up().and_then(vehicle::KiowaRide::report) {
        println!("{said}");
    }
    match seat.garage.switch(to, &mut seat.sim, place) {
        Ok(up) => {
            took(rig, up);
            Some(up)
        }
        Err(e) => {
            println!("vehicle: {e}");
            None
        }
    }
}

/// The machine the original's switch pop-up offers this seat (P103), among the bays shown: only to a
/// rider on the bike, the one machine the port parks nothing of, and only a machine this seat's
/// garage has (its own copy is what the switch spawns on it).
fn seat_offer(seat: &Seat, bays: &[vehicle::Bay]) -> Option<Machine> {
    if !seat.riding || seat.garage.up != Kind::Bike {
        return None;
    }
    let rider = &seat.sim.rider;
    let speed = rider.ride.chassis.speed.0;
    vehicle::offer(bays, seat.garage.up, rider.ride.position(), speed, rider.crash.is_some())
        .filter(|m| m.kind().is_some_and(|k| seat.garage.machines().contains(&k)))
}

/// A player's machine `up` just taken, offline or online (N17): its camera starts where it is, and
/// the HUD and the console say so.
fn took(rig: &mut Rig, up: Kind) {
    let cam = &mut rig.follow_mut(up).cam;
    cam.snap = true;
    let name = (!cam.presets.is_empty()).then(|| cam.preset().name.clone());
    if let Some(name) = name {
        rig.ride_hud.note_camera(Instant::now(), &name);
    }
    println!("vehicle: {}", up.name());
}

/// What an online peer's rig is dressed from mid-ride (N16): the ride's disc, models and wardrobe,
/// and what the first player's rig was built on — a seat the session takes on after the ride
/// loaded, or one a player of another look sits in now.
struct PeerDress<'a> {
    disc: &'a mut Disc,
    assets: &'a mut Assets,
    /// Opened here the first time a peer wears a look, if no rider of the ride's load did.
    kit: &'a mut Option<dress::Kit>,
    /// The first player's rider archive: without one the ride draws no bodies.
    rider_member: Option<&'a str>,
    /// The first player's rider and bike model, and the bike's archive, `.sub` and archives.
    own: &'a roster::Pick,
    bike_member: &'a str,
    vehicle: &'a Vehicle,
    archives: &'a [Res],
    stunts: &'a [game_sim::trick::Stunt],
    pack: Option<(&'a game_render::anim::PackClips, &'a [game_sim::trick::Stunt])>,
    motion: game_sim::motion::Params,
    /// The first player's body model, which a peer's own gear is a repaint of.
    body_model: Option<&'a str>,
    gpu_on: bool,
    cameras: &'a (game_sim::camera::Follow, Option<game_sim::camera::Follow>),
    /// The cars' own cameras as the ride loaded them (S246), each kind's on its presets file: a peer
    /// seat a player on this machine sits in (N20) is looked through, in a car too (S268).
    car_cameras: &'a [(Kind, Race)],
}

/// An online peer's rig in `pick`'s look (N16), as the ride's load dresses a split screen's other
/// players: its rider's archive and its gear repainted in its look, its name and number on the
/// jersey and the plates, its bike model's `.sub` — every model under names of its own (`@peer<n>`),
/// which the GPU is handed after. Posed and drawn, with no view: its cameras are never looked through.
fn dress_peer(p: &mut PeerDress, n: usize, pick: &roster::Pick) -> Result<(Rig, Vec<String>), String> {
    let tag = format!("@peer{n}");
    let mut made = Vec::new();
    let theirs = member_ending(p.disc, &format!("{}.res", pick.rider)).ok();
    let body = match (p.rider_member, &theirs) {
        (None, _) => None,
        (Some(_), Some(member)) => Body::load(vec![p.disc.res(member)?], &pick.rider, Sex::Male, "", p.stunts, p.motion).ok(),
        (Some(member), None) => Body::load(vec![p.disc.res(member)?], &p.own.rider, Sex::Male, "", p.stunts, p.motion).ok(),
    };
    let body = body.map(|mut b| {
        if let Some((clips, stunts)) = p.pack {
            b.add_pack(clips, stunts);
        }
        b
    });
    if p.kit.is_none() && pick.look != roster::Look::default() {
        *p.kit = Some(dress::Kit::open(p.disc));
    }
    // Its own name and number, on whatever of its bike and body is loaded below.
    if let Some(k) = p.kit.as_mut() {
        p.assets.make_texture(dress::NAME_TEXTURE, k.texture(p.disc, &pick.rider, &pick.look, &pick.bike));
    }
    // Its own bike model's `.sub` when it is not the first player's.
    let own_bike = match pick.bike != p.own.bike {
        true => match member_ending(p.disc, &format!("{}.res", pick.bike)) {
            Ok(member) => {
                let res = [p.disc.res(&member)?];
                match res[0].read_suffix(&format!("{}.sub", pick.bike)) {
                    Ok(bytes) => Some((member, Vehicle::parse(&ini::text(&bytes)), res)),
                    Err(e) => {
                        println!("online: peer bike: {e}");
                        None
                    }
                }
            }
            Err(e) => {
                println!("online: peer bike: {e}; the first player's");
                None
            }
        },
        false => None,
    };
    let (member, vehicle) = match &own_bike {
        Some((member, v, _)) => (member.as_str(), v),
        None => (p.bike_member, p.vehicle),
    };
    let chassis = match vehicle.mcf.is_empty() {
        true => None,
        false => Chassis::load(vec![p.disc.res(member)?], &vehicle.mcf, BIKE_PREFIX, p.stunts, vehicle_params()).ok(),
    };
    let mut chassis = chassis.map(|mut c| {
        if let Some((clips, stunts)) = p.pack {
            c.add_pack(clips, stunts);
        }
        c
    });
    let (bike_model, parts) = match &own_bike {
        Some((_, v, res)) => {
            let own = format!("{CHASSIS}{tag}");
            let file = game_render::vehicle::chassis_model(res, v);
            let drawn = p.assets.load_repainted(res, &file, &own, &v.materials);
            let parts = v
                .parts
                .iter()
                .map(|part| {
                    let name = format!("{}{tag}", part.model);
                    p.assets.load_repainted(res, &part.model, &name, &v.materials);
                    made.push(name.clone());
                    game_render::scene::Part { model: name, placement: part.placement.clone() }
                })
                .collect();
            (if drawn { own } else { CHASSIS.to_string() }, parts)
        }
        None => {
            // On the GPU a pose is node matrices, so the shared chassis draws for every bike; in
            // software a pose is the model's triangles, so the peer's is a copy.
            let model = match (p.gpu_on, p.assets.models.get(CHASSIS).cloned()) {
                (false, Some(loaded)) => {
                    let own = format!("{CHASSIS}{tag}");
                    p.assets.models.insert(own.clone(), loaded);
                    own
                }
                _ => CHASSIS.to_string(),
            };
            // Its own plates when the wardrobe is open: the parts again under its own names.
            let parts = match p.kit.is_some() {
                true => p
                    .vehicle
                    .parts
                    .iter()
                    .map(|part| {
                        let name = format!("{}{tag}", part.model);
                        p.assets.own(1);
                        p.assets.load_repainted(p.archives, &part.model, &name, &p.vehicle.materials);
                        p.assets.own(0);
                        made.push(name.clone());
                        game_render::scene::Part { model: name, placement: part.placement.clone() }
                    })
                    .collect(),
                false => p.vehicle.parts.clone(),
            };
            (model, parts)
        }
    };
    made.push(bike_model.clone());
    // Its body again under its own name, in its own gear's renames and its look's paint.
    let body_model = match (p.body_model, &body) {
        (Some(name), Some(b)) => {
            let own = format!("{name}{tag}");
            let mut res = vec![p.disc.res(theirs.as_deref().or(p.rider_member).expect("a body was loaded"))?];
            let pairs = match p.kit.as_ref() {
                Some(k) => {
                    if let Some(custom) = &k.custom {
                        res.push(p.disc.res(custom)?);
                    }
                    k.repaint(&b.rider.suit, &pick.look)
                }
                None => b.rider.suit.materials.clone(),
            };
            match p.assets.load_repainted(&res, name, &own, &pairs) {
                true => {
                    made.push(own.clone());
                    Some(own)
                }
                false => Some(name.to_string()),
            }
        }
        (name, _) => name.map(str::to_string),
    };
    if let Some(c) = &mut chassis {
        c.stop_fork_in(&p.assets.models, &bike_model, &parts);
    }
    let rig = Rig {
        body,
        chassis,
        camera: Race { cam: p.cameras.0.clone() },
        plane_camera: p.cameras.1.as_ref().map(|c| Race { cam: c.clone() }),
        // Online rides no helicopter (P216); its cars have their own cameras (S268).
        heli_camera: None,
        car_cameras: p.car_cameras.iter().map(|(k, c)| (*k, Race { cam: c.cam.clone() })).collect(),
        wreck_camera: chase::WreckCamera::new(false),
        framing: (chase::Framing::ORIGINAL, chase::Framing::ORIGINAL),
        camera_presets: Default::default(),
        camera_started: false,
        fov_ease: Default::default(),
        look_back: Default::default(),
        thrill: Default::default(),
        odometer: overlay::Odometer::default(),
        preload_meter: overlay::PreloadMeter::default(),
        pilot_lean: vehicle::PilotLean::default(),
        ride_hud: ride_hud::Tracker::default(),
        bike_model,
        body_model,
        parts,
        modified: false,
    };
    Ok((rig, made))
}

/// Park the lot's machines for `players` (P87): a copy of each per player, every staying copy where it
/// was, and the cars stood on the ground under their bays.
fn park_lot(lot: &mut vehicle::Lot, place: &mut impl sim_world::World, players: usize) {
    lot.resize(players, &mut |at: Vf| sim_world::World::ground(place, Vf::new(at.x(), 1.0e4, at.z(), 1.0)).0.y());
    lot.stand_cars(&mut |from: Vf| sim_world::World::ground(place, from));
}

/// What the original's HUD shows of a player's own ride (V26, V28): their odometer, which machine is
/// up, how hard a preload is shoving the bike, and the score — the rider's banked total (S79, P68).
fn gauges(seat: &Seat, rig: &Rig, speedometer: menu::Speedometer) -> overlay::Gauges {
    overlay::Gauges {
        feet: rig.odometer.feet,
        bike: seat.garage.up == Kind::Bike,
        preload: rig.preload_meter.shown(),
        score: seat.sim.rider.score.total.0.max(0.0) as u32,
        modified: rig.modified,
        speedometer,
    }
}

/// What every view's ride HUD (P121) is drawn from besides its own seat: every rider's camera
/// target, the original's HUD's per-view lists (tags, radar icons, switch offers), the radar's span,
/// the speedometer style, the lineup's names and the stunt table that names the tricks.
struct RideHud<'a> {
    ridden: &'a [Ridden],
    art: &'a overlay::Art,
    span: f32,
    /// The maps the radar names (MD42).
    maps: &'a hud::MapNames,
    /// A world that wraps (MD43): the radar marks everything at its copy nearest the rider, and
    /// names the maps across a seam.
    wrap: Option<game_render::wrap::Wrap>,
    speedometer: menu::Speedometer,
    names: &'a [Option<String>],
    table: &'a Stunts,
    simulated: bool,
    /// Whether this view carries the join cue (P171).
    join: bool,
    /// Each view's buttons, and the join cue's (P281).
    buttons: &'a [menu::pads::Buttons],
    join_buttons: &'a menu::pads::Buttons,
}

impl RideHud<'_> {
    /// View `v`'s HUD, its seat `seat`: the odometer and the tracker a frame on, then `draw` given
    /// what it shows. `marker` is the markers' slot display while it is up.
    fn with(&self, seat: &Seat, rig: &mut Rig, v: usize, marker: Option<&MarkerShown>, draw: impl FnOnce(&ride_hud::Shown, &ride_hud::Tracker, Instant)) {
        let now = Instant::now();
        let me = &self.ridden[v];
        let b = me.target.body_point;
        if seat.riding && self.simulated {
            rig.odometer.step([b.x(), b.y(), b.z()]);
        }
        track_score(seat, rig, self.table, now);
        // A split is digital, as the original's code forces it with two riders or more.
        let speedometer = if self.ridden.len() > 1 { menu::Speedometer::Digital } else { self.speedometer };
        let gauges = gauges(seat, rig, speedometer);
        let riders: Vec<ride_hud::Dot> = self
            .ridden
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != v)
            .map(|(_, r)| ride_hud::Dot { at: hud::radar_near(self.wrap, [b.x(), b.z()], [r.target.body_point.x(), r.target.body_point.z()]), ai: false })
            .collect();
        let markers = marker.map(|(pick, filled, _, others, holding, home, cleared)| ride_hud::Panel {
            pick: *pick,
            filled: filled.clone(),
            home: *home,
            cleared: *cleared,
            others: others.iter().map(|&s| (s, self.names.get(s).cloned().flatten())).collect(),
            holding: *holding,
            machine: seat.garage.up.name().to_string(),
        });
        let (map, around) = self.maps.around_in([b.x(), b.z()], self.wrap);
        let blips = self.art.radar.get(v).map_or(&[][..], Vec::as_slice);
        let near_blips: Vec<overlay::Blip> = match self.wrap {
            Some(_) => blips.iter().map(|&k| overlay::Blip { at: hud::radar_near(self.wrap, [b.x(), b.z()], k.at), ..k }).collect(),
            None => Vec::new(),
        };
        let shown = ride_hud::Shown {
            speed: ride_hud::speed_shown(me.speed.0, seat.garage.up == Kind::Bike && seat.sim.rider.crash.is_some()),
            gauges: &gauges,
            own: [b.x(), b.z()],
            forward: [me.target.forward.x(), me.target.forward.z()],
            riders: &riders,
            blips: if self.wrap.is_some() { &near_blips } else { blips },
            span: self.span,
            tag: self.art.tags.get(v).map(overlay::Tag::text).filter(|t| !t.is_empty()),
            offer: self.art.offers.get(v).copied().flatten(),
            challenge: stunt_challenge(&seat.sim),
            markers,
            scale: self.art.scale,
            minimap: self.art.minimap,
            map,
            around,
            join: self.join,
            event: ride_hud::EventShown::of(&seat.sim.event, 0, &seat.sim.rider.score),
            buttons: self.buttons.get(v).cloned().unwrap_or_default(),
            join_buttons: self.join_buttons.clone(),
        };
        rig.ride_hud.note_event(now, shown.event.as_ref());
        draw(&shown, &rig.ride_hud, now);
    }
}

/// The freestyle event's start prompt (P155): up while the seat's rider is asked to start it — on
/// the bike, slow, in a stunt zone, no event on — with the hold so far.
fn stunt_challenge(sim: &Sim) -> Option<ride_hud::Challenge> {
    let e = &sim.event;
    e.asks(&sim.event_rider(0)).then(|| ride_hud::Challenge {
        activity: overlay::Activity::Stunt,
        name: "Stunt Competition".into(),
        goal: format!("Score over {} in 2 minutes", ride_hud::grouped(e.target as f32)),
        hold: e.held(0).0,
    })
}

/// The view the join cue (P171) is drawn in: the one in the frame's top right corner, which is the
/// whole frame alone, the top view stacked and the right one side by side.
fn cue_view(rects: &[split::Rect], width: usize) -> usize {
    rects.iter().position(|r| r.y == 0 && r.x + r.w == width).unwrap_or(0)
}

/// The 1 s line's word on seat 0's rider (P212a): of the frames its bike drew in, how many drew no
/// rider over it — all of them is "no rider drawn", some of them says how many, none says nothing.
/// `frames` is (frames the bike drew in, those of them the rider drew none in).
fn rider_left_out(frames: (u32, u32)) -> String {
    match frames {
        (_, 0) => String::new(),
        (bike, none) if none >= bike => " (no rider drawn)".into(),
        (bike, none) => format!(" (no rider drawn in {none} of {bike} frames)"),
    }
}

#[cfg(test)]
mod rider_left_out_line {
    use super::rider_left_out;

    #[test]
    fn the_line_says_when_the_rider_was_not_drawn() {
        assert_eq!(rider_left_out((0, 0)), "");
        assert_eq!(rider_left_out((60, 0)), "");
        assert_eq!(rider_left_out((60, 60)), " (no rider drawn)");
        assert_eq!(rider_left_out((60, 3)), " (no rider drawn in 3 of 60 frames)");
    }
}

/// The view's HUD tracker a frame on from its seat's score card (P121, P68): each view follows its
/// own rider's, so a split or an online ride shows every player their own tricks.
fn track_score(seat: &Seat, rig: &mut Rig, table: &Stunts, now: Instant) {
    // A trick pack's extras (MD16) are named by the rider's set, as the ride log names them.
    let set = &seat.sim.rider.set;
    rig.ride_hud.update(now, &seat.sim.rider.score, |id| {
        let t = game_sim::trick::Trick(id);
        match t.extra().and_then(|_| set.stunt(t)) {
            Some(stunt) => stunt.display_name.clone(),
            None => table.display_name(0, t).to_string(),
        }
    });
}

/// What the speedometer reads off a seat's rider for the preload meter (V53): nothing on the
/// plane, where the original shows no meter.
fn preload_sense(seat: &Seat) -> overlay::PreloadSense {
    match seat.garage.up {
        Kind::Bike => overlay::PreloadSense::of(
            &seat.sim.rider.ride.air,
            seat.sim.phy.air.max_preload.0,
            seat.sim.rider.state.wreck.is_some(),
        ),
        _ => overlay::PreloadSense::default(),
    }
}

/// What view `v`'s radar marks besides the riders (P92): the activities' start zones as the original
/// shows them to a player who has started none (`zones`, [`overlay::shown_zones`]), every plane
/// standing with nobody in it — every bay shown (P103), `bays` — every bike whose rider is in the
/// plane — the view's own too, as the original marks its own player's — and the view's own player's
/// session markers (P88), each slot its own colour and number, the selected one ringed (P94). `pick`
/// is the markers' selection, as the slot display takes it, or `None` for a view that shows no markers.
fn radar_blips(seats: &[Seat], bays: &[vehicle::Bay], v: usize, zones: &[overlay::Zone], pick: Option<markers::Pick>) -> Vec<overlay::Blip> {
    use overlay::{Blip, Icon};
    let ground = |at: Vf| [at.x(), at.z()];
    let view = &seats[v];
    // In the original's order, each over the one before: the machines, the bikes, the zones.
    let mut out = Vec::new();
    // A plane left, and a bike left, are nowhere (P102): the machines standing are the bays'.
    for bay in bays {
        out.push(Blip { icon: Icon::Machine(bay.machine, bay.livery), at: ground(bay.spawn.at) });
    }
    let zones = overlay::shown_zones(zones, view.garage.up != Kind::Bike);
    out.extend(zones.into_iter().map(|z| Blip { icon: Icon::Activity(z.activity), at: z.at }));
    if let Some(pick) = pick {
        out.extend(overlay::marker_blips(&view.sim.markers, pick));
    }
    out
}

/// The ground's height under `x`, `z` for a portal's disc (V51), from the ride's own terrain query
/// looking down from `y`; `None` off the terrain, where the query answers its no-ground depth.
fn portal_ground(place: &mut dyn sim_world::World, x: f32, y: f32, z: f32) -> Option<f32> {
    let (on, _) = place.ground(Vf::new(x, y, z, 1.0));
    (on.y() > -1.0e5).then(|| on.y())
}

/// What view `v` sees of the portals (V51): the start zones the original shows its player (not the
/// machine race's while they are in a machine, [`portal::shown`]) and the player's own markers'.
fn portal_glows(art: &portal::Art, zones: &[portal::Portal], marked: &[Option<portal::Portal>], in_machine: bool, camera: &Camera, seconds: f32) -> Vec<game_render::glow::Glow> {
    let (eye, at) = (camera.eye, camera.at);
    let forward = [at.x - eye.x, at.y - eye.y, at.z - eye.z];
    let race = |p: &&portal::Portal| in_machine && p.kind == portal::Kind::Zone(overlay::Activity::Race);
    let shown = zones.iter().filter(|p| !race(p)).chain(marked.iter().flatten());
    shown.flat_map(|p| p.glows(art, [eye.x, eye.y, eye.z], forward, seconds)).collect()
}

/// One local player's half of the picture (V22): the body and bike posed for them, the cameras that
/// follow them, their overlay, and the models their pose is skinned into — the first player's are the
/// ones the archives loaded, every other player's a copy under a name of its own.
struct Rig {
    body: Option<Body>,
    chassis: Option<Chassis>,
    camera: Race,
    plane_camera: Option<Race>,
    /// The helicopter's own camera (P216), on `HeloCPresets.ini`, while this player has one. It
    /// shares the plane's framing, field-of-view ease and look-back slots.
    heli_camera: Option<Race>,
    /// The cars' own cameras (S246), one a car kind this player can drive, each on its kind's presets
    /// file ([`chase::car_presets`]). They share the bike's framing, field-of-view ease and look-back
    /// slots; a car without one is followed by the bike's.
    car_cameras: Vec<(Kind, Race)>,
    /// The camera the screen switches to while this player's rider is down (V31).
    wreck_camera: chase::WreckCamera,
    /// How this player's two follow cameras' presets are framed now (P82), the bike's and the
    /// plane's: the original's as they were read, until the frame lays the player's own over them.
    framing: (chase::Framing, chase::Framing),
    /// The seat's own camera presets laid over the two follow cameras' now (P163): the disc presets'
    /// own numbers, the player's own presets, and what the camera button cycles: none until a front
    /// end's Camera page has some.
    camera_presets: (Vec<menu::cameras::Own>, menu::cameras::Cycle),
    /// Whether the two cameras were put on the presets the seat was last left on (P163).
    camera_started: bool,
    /// The two follow cameras' fields of view easing over a framing change (P119), the bike's and the
    /// plane's.
    fov_ease: (chase::FovEase, chase::FovEase),
    /// The look-back button on the two follow cameras (P78), the bike's and the plane's.
    look_back: (chase::LookBack, chase::LookBack),
    /// The other-camera button and the thrill camera it brings up over the bike's follow camera (P157).
    thrill: chase::OtherCamera,
    /// The distance this player's odometer shows (V26).
    odometer: overlay::Odometer,
    /// This player's preload meter, a tick at a time (V53).
    preload_meter: overlay::PreloadMeter,
    /// This player's pilot's lean in the plane, a tick at a time (V78).
    pilot_lean: vehicle::PilotLean,
    /// This player's ride HUD over time (P121): the trick pop-up, the score's flash, the camera toast.
    ride_hud: ride_hud::Tracker,
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
        match (up, &self.plane_camera, &self.heli_camera, self.car_cameras.iter().find(|(k, _)| *k == up)) {
            (Kind::Biplane, Some(cam), _, _) | (Kind::Kiowa, _, Some(cam), _) | (_, _, _, Some((_, cam))) => cam,
            _ => &self.camera,
        }
    }

    fn follow_mut(&mut self, up: Kind) -> &mut Race {
        match (up, &mut self.plane_camera, &mut self.heli_camera, self.car_cameras.iter_mut().find(|(k, _)| *k == up)) {
            (Kind::Biplane, Some(cam), _, _) | (Kind::Kiowa, _, Some(cam), _) | (_, _, _, Some((_, cam))) => cam,
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
    /// Whether `body` is the ragdoll's, thrown off the bike, which is blended where it is in the world
    /// rather than where it is on the bike (P120, [`pace::Blend::thrown`]).
    thrown: bool,
}

impl Shown {
    fn of(seat: &Seat, rig: &Rig, assets: &Assets, ragdolled: Option<&game_render::rider::Ragdolled>, poses: bool) -> Shown {
        let bike = match &seat.sim.rider.crash {
            Some(crash) => chassis_wrecked(crash),
            None => chassis_at(&seat.sim.rider.ride),
        };
        if !poses {
            return Shown { bike, chassis: Vec::new(), body: None, thrown: false };
        }
        let chassis = rig.chassis.as_ref().map_or(Vec::new(), |c| c.world.clone());
        let down = match (seat.sim.rider.drawn_ragdoll(), &seat.sim.rider.ragdoll_kit, ragdolled) {
            (Some(doll), Some(kit), Some(rigged)) => Some((doll.nodes(kit), rigged)),
            _ => None,
        };
        let riding = riding_bones(seat, ragdolled).filter(|_| down.is_none());
        let body = match (&rig.body, down.as_ref().or(riding.as_ref())) {
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
        let thrown = body.is_some() && down.is_some();
        Shown { bike, chassis, body, thrown }
    }

    /// From this, the tick before the last, to `now`, the last, when the seat's [`pace::Cut`] says
    /// the two are no jump apart. A rider thrown at both is blended in the world, off the bike that
    /// tumbles beside them; one on the bike, on the bike.
    fn blend(&self, now: &Shown, by: &pace::Blend) -> Shown {
        let bike = by.slide(self.bike, now.bike);
        Shown {
            bike,
            chassis: by.pose(&self.chassis, &now.chassis),
            body: match (&self.body, &now.body) {
                (Some(was), Some(is)) if self.thrown && now.thrown => Some(by.thrown(was, self.bike, is, now.bike, bike)),
                (Some(was), Some(is)) => Some(by.pose(was, is)),
                (_, is) => is.clone(),
            },
            thrown: now.thrown,
        }
    }
}

/// **What the drawn bike hands the riding ragdoll** (S109, S110): the chassis' "Bike" node (below the
/// chassis node) and its grips and pegs as they are drawn this frame, in the chassis' model frame. The
/// rider's next frame steps its riding ragdoll held to them ([`game_sim::rider::Rider::holds`]), which is
/// what a wreck is thrown from. `None` before the chassis has a frame or without all four holds.
fn ride_holds(rig: &Rig) -> Option<game_sim::ragdoll_riding::Holds> {
    let chassis = rig.chassis.as_ref()?;
    let [Some(foot_l), Some(foot_r), Some(hand_l), Some(hand_r)] = chassis_grips(chassis) else { return None };
    let point = |a: V3| Vf::new(a.x, a.y, a.z, 1.0);
    let root = chassis.nodes.root.and_then(|i| chassis.world.get(i));
    let root = root.map_or(game_sim::math::Mat4::IDENTITY, |r| game_sim::math::Mat4(r.0.map(Vf)));
    Some(game_sim::ragdoll_riding::Holds { root, hands: [point(hand_l), point(hand_r)], feet: [point(foot_l), point(foot_r)] })
}

/// Whether the riding rider is drawn from the riding ragdoll, as the original's is (S90): **on by
/// default** since the user lifted the 10-24 ruling on 2026-11-02 ("make sure we match or close to
/// match the original springyness"). `OPENMX_RIDING_RAGDOLL=0` draws the clip pose plus hand IK
/// instead. It was off from 2026-10-23 ("our guy is super floppy") while the put-back entry was wrong;
/// S201 matched the entry, and game-sim's `riding_spring` tests hold the body's give and settle to the
/// original's over S74's and S201's put-backs, plain riding and landings.
fn riding_ragdoll_on() -> bool {
    riding_ragdoll_asked(std::env::var_os("OPENMX_RIDING_RAGDOLL").as_deref())
}

/// [`riding_ragdoll_on`] on the variable's value: only an explicit `0` turns the riding ragdoll off.
fn riding_ragdoll_asked(var: Option<&std::ffi::OsStr>) -> bool {
    var.is_none_or(|v| v != "0")
}

#[cfg(test)]
mod riding_ragdoll_default {
    use super::riding_ragdoll_asked;
    use std::ffi::OsStr;

    /// The user's call (2026-11-02): the riding rider is the riding ragdoll, as the original's is,
    /// unless `OPENMX_RIDING_RAGDOLL=0` asks for the clip pose.
    #[test]
    fn riding_ragdoll_is_on_by_default() {
        assert!(riding_ragdoll_asked(None), "the riding ragdoll is not drawn with no variable set");
        for on in ["", "1", "true", "yes", "on"] {
            assert!(riding_ragdoll_asked(Some(OsStr::new(on))), "OPENMX_RIDING_RAGDOLL={on} draws the clip");
        }
        assert!(!riding_ragdoll_asked(Some(OsStr::new("0"))));
    }
}

/// The riding ragdoll's nodes to draw the body from, once it has been placed (S109).
fn riding_bones<'a>(
    seat: &Seat,
    ragdolled: Option<&'a game_render::rider::Ragdolled>,
) -> Option<(Vec<game_sim::math::Mat4>, &'a game_render::rider::Ragdolled)> {
    match (&seat.sim.rider.riding, &seat.sim.rider.ragdoll_kit, ragdolled) {
        (Some(r), Some(kit), Some(rigged)) if riding_ragdoll_on() && r.placed() && seat.sim.rider.drawn_ragdoll().is_none() => {
            Some((r.nodes(kit), rigged))
        }
        _ => None,
    }
}

/// `--trace-picture`'s line for a drawn frame (P120): the first player's bike, the middle of their
/// rider's nodes and their camera as drawn, and the bike's and the rider's spots at the two ticks the
/// frame is blended between, which a test reads to prove the picture never steps back.
#[allow(clippy::too_many_arguments)]
fn trace_picture(
    seats: &[Seat],
    rigs: &[Rig],
    assets: &Assets,
    ragdolled: Option<&game_render::rider::Ragdolled>,
    before: &[Shown],
    cameras: &[Camera],
    cut: &[bool],
    blend: &pace::Blend,
    tick: u64,
    before_planes: &[Option<M4>],
    before_helis: &[Option<(M4, Rotor)>],
) {
    let (Some(seat), Some(rig), Some(camera)) = (seats.first(), rigs.first(), cameras.first()) else { return };
    let now = Shown::of(seat, rig, assets, ragdolled, true);
    let was = before.first().unwrap_or(&now);
    let cut = cut.first().copied().unwrap_or(true);
    let shown = if cut { now.clone() } else { was.blend(&now, blend) };
    let spot = |m: &M4| format!("{:.3} {:.3} {:.3}", m.0[3][0], m.0[3][1], m.0[3][2]);
    let p3 = |v: V3| format!("{:.3} {:.3} {:.3}", v.x, v.y, v.z);
    // The middle of the rider's nodes, in the world.
    let body = |s: &Shown| {
        let nodes = s.body.as_ref().filter(|b| !b.is_empty())?;
        let sum = nodes.iter().map(|m| m.mul(s.bike)).fold(V3 { x: 0.0, y: 0.0, z: 0.0 }, |a, m| {
            a.add(V3 { x: m.0[3][0], y: m.0[3][1], z: m.0[3][2] })
        });
        Some(sum.scale(1.0 / nodes.len() as f32))
    };
    let rider = match (body(was), body(&now), body(&shown)) {
        (Some(a), Some(b), Some(d)) => format!("rider from {} to {} drawn {}", p3(a), p3(b), p3(d)),
        _ => "rider none".to_string(),
    };
    println!(
        "picture: tick {tick} blend {:.4} cut {} down {} bike from {} to {} drawn {} eye {} at {} {rider}",
        blend.0,
        cut as u8,
        seat.sim.rider.crash.is_some() as u8,
        spot(&was.bike),
        spot(&now.bike),
        spot(&shown.bike),
        p3(camera.eye),
        p3(camera.at),
    );
    // And the rider's drawn nodes on the bike, each its spot in the bike's frame (P270): what a test
    // reads a put-back's pose by, whatever the bike turned through.
    if let Some(nodes) = shown.body.as_ref().filter(|b| !b.is_empty()) {
        let at: Vec<String> = nodes.iter().map(|m| format!("{:.3} {:.3} {:.3}", m.0[3][0], m.0[3][1], m.0[3][2])).collect();
        println!("picture pose: tick {tick} cut {} nodes {} {}", cut as u8, nodes.len(), at.join(" "));
    }
    // And the plane while it is flown, as its draw takes it: from the tick before to this one, and
    // slid between them unless the seat cuts.
    if let Some(plane) = seat.garage.plane_flown() {
        let now = plane_at(plane);
        let was = before_planes.first().copied().flatten();
        let drawn = was.filter(|_| !cut).map_or(now, |b| blend.slide(b, now));
        println!(
            "picture plane: tick {tick} blend {:.4} cut {} from {} to {} drawn {} eye {} at {}",
            blend.0,
            cut as u8,
            was.as_ref().map_or_else(|| "none".to_string(), spot),
            spot(&now),
            spot(&drawn),
            p3(camera.eye),
            p3(camera.at),
        );
    }
    // And the helicopter's the same (P216b).
    if let Some(heli) = seat.garage.heli_up()
        && let Some(now) = heli_at(heli)
    {
        let was = before_helis.first().copied().flatten();
        let drawn = heli_drawn(heli, was.filter(|_| !cut), blend).map_or(now, |(m, _)| m);
        println!(
            "picture heli: tick {tick} blend {:.4} cut {} from {} to {} drawn {} eye {} at {}",
            blend.0,
            cut as u8,
            was.as_ref().map_or_else(|| "none".to_string(), |(m, _)| spot(m)),
            spot(&now),
            spot(&drawn),
            p3(camera.eye),
            p3(camera.at),
        );
    }
}

/// The spots a seat's [`pace::Cut`] measures a tick's move by: where its machine is — the plane
/// while it is flown, the bike (or its wreck) otherwise — and where its camera is and looks.
fn cut_spots(seat: &Seat, bike: M4, camera: Option<&Camera>) -> [V3; 3] {
    let heli = seat.garage.heli_up().and_then(heli_at);
    let machine = match (seat.garage.plane_flown(), heli) {
        (Some(plane), _) => vector(plane.at),
        (None, Some(m)) => V3 { x: m.0[3][0], y: m.0[3][1], z: m.0[3][2] },
        (None, None) => V3 { x: bike.0[3][0], y: bike.0[3][1], z: bike.0[3][2] },
    };
    match camera {
        Some(c) => [machine, c.eye, c.at],
        None => [machine; 3],
    }
}

/// How far `plane` jumped over the last tick, if it moved further than it flies (P125): its spot
/// `before` the tick (where it was drawn, [`plane_at`]) against where it is ([`pace::jump`]), its
/// speed over the tick's `dt` the move it was making. What a marker's put-back or an Up onto another
/// player does, offline, on a tape and online alike.
fn plane_jumped(before: Option<M4>, plane: &Plane, dt: F) -> Option<V3> {
    let b = before?;
    pace::jump(V3 { x: b.0[3][0], y: b.0[3][1], z: b.0[3][2] }, vector(plane.at), plane.speed().0 * dt.0)
}

/// The whole widths a bike moved over a frame on a world that wraps (MD29), `[x, 0, z]` in feet:
/// a seam's move, which a frame's own riding never comes near (half the world). `None` with none.
fn seam_moved((wrap, side): ([bool; 2], [f32; 2]), was: Option<M4>, now: Vf) -> Option<[f32; 3]> {
    let was = was?;
    let lap = |axis: usize, d: f32| if wrap[axis] && d.abs() > side[axis] / 2.0 { (d / side[axis]).round() * side[axis] } else { 0.0 };
    let by = [lap(0, now.x() - was.0[3][0]), 0.0, lap(1, now.z() - was.0[3][2])];
    (by != [0.0; 3]).then_some(by)
}

/// The whole widths a seat's machine moved over a seam this tick (MD29, MD44): the plane's while it
/// is flown, from its spot `plane` before the tick, else the bike's from `bike` ([`seam_moved`]). The
/// helicopter's, flown, is the plane's: `plane` is then its spot before the tick (P216b).
fn seat_seam(seams: ([bool; 2], [f32; 2]), seat: &Seat, bike: Option<M4>, plane: Option<M4>) -> Option<[f32; 3]> {
    match (seat.garage.plane_flown(), seat.garage.heli_up().and_then(|h| h.matrix())) {
        (Some(p), _) => seam_moved(seams, plane, p.at),
        (None, Some(m)) => seam_moved(seams, plane, m.0[3]),
        (None, None) => seam_moved(seams, bike, seat.sim.bike_pos()),
    }
}

/// What the picture slides from, moved `by` a seam the seat's machine crossed this tick: the
/// machine's spot before the tick — the plane's when `flying`, the bike's otherwise, so a parked one
/// stays put — the camera before it, and the spots the seat's cut is measured from. The tick is
/// then drawn slid from just short of the seam to just past it, as any other tick, and does not cut
/// (P120): before, a crossing cut the seat's picture, so its frames between the two ticks all showed
/// the later one — the plane (or bike) stepped two frames' way and stood a frame.
fn seam_carry(by: [f32; 3], flying: bool, before: Option<&mut Shown>, plane: Option<&mut Option<M4>>, camera: Option<&mut Camera>, cut: Option<&mut pace::Cut>) {
    let d = V3 { x: by[0], y: by[1], z: by[2] };
    let carried = |m: &mut M4| {
        m.0[3][0] += by[0];
        m.0[3][2] += by[2];
    };
    match (flying, plane, before) {
        (true, Some(Some(p)), _) => carried(p),
        (false, _, Some(b)) => carried(&mut b.bike),
        _ => {}
    }
    if let Some(c) = camera {
        c.eye = c.eye.add(d);
        c.at = c.at.add(d);
    }
    if let Some(c) = cut {
        c.carry(d);
    }
}

/// A world that wraps (MD29) as its draw takes it (MD43): which axes, and its sides.
fn drawn_wrap(t: &game_sim::tiles::Tiled) -> Option<game_render::wrap::Wrap> {
    game_render::wrap::Wrap::of(t.wraps(), t.extent().map(|v| v.0))
}

/// When the shell started (P283): the first call's moment, which `main` makes.
fn launched() -> Instant {
    static AT: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    *AT.get_or_init(Instant::now)
}

/// The most memory the shell has held so far and holds now (P283), as Linux counts it
/// (`/proc/self/status`'s VmHWM and VmRSS): what a Steam Deck's log shows of a world's load. On an
/// iPhone (P301) it is the footprint iOS closes the app by, which counts the GPU's memory too, and
/// what is left of it.
fn peak_memory() -> String {
    #[cfg(target_os = "ios")]
    if let Some(f) = openmx_ios::footprint() {
        let gpu = openmx_ios::gpu_bytes().map(|b| format!(" ({} MB of it the GPU's)", b >> 20)).unwrap_or_default();
        return format!("peak {} MB, now {} MB{gpu}, {} MB left before iOS closes the app", f.peak >> 20, f.now >> 20, f.left >> 20);
    }
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let kb = |key: &str| status.lines().find_map(|l| l.strip_prefix(key)?.trim().strip_suffix("kB")?.trim().parse::<u64>().ok());
    match (kb("VmHWM:"), kb("VmRSS:")) {
        (Some(peak), Some(now)) => format!("peak {} MB, now {} MB", peak / 1024, now / 1024),
        _ => "not known here".to_string(),
    }
}

/// The memory held now, for the line a second (P301): an iPhone's footprint, what is left before iOS
/// closes the app, and the GPU's share. None elsewhere: Linux's figure is a file read, and a ride
/// reads nothing after its first frame (`one_fps`).
fn memory_now() -> Option<String> {
    #[cfg(target_os = "ios")]
    {
        let gpu = openmx_ios::gpu_bytes().map(|b| format!(", GPU {} MB", b >> 20)).unwrap_or_default();
        openmx_ios::footprint().map(|f| format!("{} MB ({} MB left{gpu})", f.now >> 20, f.left >> 20))
    }
    #[cfg(not(target_os = "ios"))]
    None
}

/// The points a world's pieces are streamed in around (MD17), with each point's copies one width
/// off on each wrapped axis (MD43): a piece near a rider's copy is a piece drawn across the seam.
fn wrapped_near(wrap: Option<game_render::wrap::Wrap>, near: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let Some(w) = wrap else { return near.to_vec() };
    let laps = |a: usize| if w.on[a] { -1..=1 } else { 0..=0 };
    let mut out = near.to_vec();
    for &[x, z] in near {
        for kz in laps(1) {
            for kx in laps(0) {
                if [kx, kz] != [0, 0] {
                    out.push([x - kx as f32 * w.side[0], z - kz as f32 * w.side[1]]);
                }
            }
        }
    }
    out
}

/// `at` moved to its copy nearest `eye` on a wrapped world (MD43), as the sim takes a rival
/// ([`game_sim::nearest_copy`]): another rider, a plane or a prop is drawn where it is nearest the
/// view, across a seam when that is nearer. Unchanged on a world that does not wrap.
fn nearest_drawn(wrap: Option<game_render::wrap::Wrap>, eye: V3, at: M4) -> M4 {
    let Some(w) = wrap else { return at };
    let near = w.nearest(eye, v3(at.0[3][0], at.0[3][1], at.0[3][2]));
    let mut m = at;
    (m.0[3][0], m.0[3][2]) = (near.x, near.z);
    m
}

/// Where the plane is drawn: at its body's pose, nose and up off the plane's own measure.
fn plane_at(plane: &Plane) -> M4 {
    M4::placement(vector(plane.at), vector(plane.facing), vector(plane.up))
}

/// Where the flown helicopter is drawn (P216, P216b): at its measure's matrix, which is where the
/// original draws it. `None` while it is not out.
fn heli_at(heli: &vehicle::KiowaRide) -> Option<M4> {
    let m = heli.matrix()?;
    let row = |r: Vf, w: f32| [r.x(), r.y(), r.z(), w];
    Some(M4([row(m.0[0], 0.0), row(m.0[1], 0.0), row(m.0[2], 0.0), row(m.0[3], 1.0)]))
}

/// The flown helicopter's rotor as a tick leaves it ([`vehicle::KiowaRide::rotor`]).
fn heli_rotor(heli: &vehicle::KiowaRide) -> Rotor {
    let (angle, blades, wrecked) = heli.rotor();
    Rotor { angle, blades, wrecked }
}

/// The helicopter a frame draws for a seat flying one (P216b), as the plane is drawn: slid from
/// where the tick before left it (`before`, `None` on a cut) to where this one does, its rotor turned
/// on between the two ([`Rotor::between`]).
fn heli_drawn(heli: &vehicle::KiowaRide, before: Option<(M4, Rotor)>, blend: &pace::Blend) -> Option<(M4, Rotor)> {
    let (now, rotor) = (heli_at(heli)?, heli_rotor(heli));
    Some(match before {
        Some((b, was)) => (blend.slide(b, now), Rotor::between(&was, &rotor, blend.0)),
        None => (now, rotor),
    })
}

/// How far the flown helicopter jumped over the last tick, as [`plane_jumped`] (P216b): a put-back
/// or a marker's Up, which takes its camera with it.
fn heli_jumped(before: Option<M4>, heli: &vehicle::KiowaRide, dt: F) -> Option<V3> {
    let (b, now) = (before?, heli_at(heli)?);
    let speed = heli.flown.as_ref()?.ride.measure.speed;
    pace::jump(V3 { x: b.0[3][0], y: b.0[3][1], z: b.0[3][2] }, V3 { x: now.0[3][0], y: now.0[3][1], z: now.0[3][2] }, speed.0 * dt.0)
}

/// A flying seat's pilot as a frame draws them (V39): the rider's own body, its root in the plane's
/// seat ([`Biplane::pilot`]) and its nodes posed by the plane's clips ([`biplane::pilot_palette`]).
struct Pilot {
    /// The plane where this frame draws it, between the last two ticks as the plane is.
    plane: M4,
    /// The body's root in the world.
    at: M4,
    /// The body's node matrices in its own space, held on to the plane's seat.
    palette: Vec<M4>,
}

/// Every seat's [`Pilot`], `None` for a seat that is not flying, has no body, or whose rider's
/// archive has no plane clips. Both renderers draw from it, so the GPU's pilot and the software
/// path's are the same body in the same seat.
fn pilots(seats: &[Seat], rigs: &mut [Rig], plane_drawn: &[Biplane], before: &[Option<M4>], cut: &[bool], blend: &pace::Blend) -> Vec<Option<Pilot>> {
    seats
        .iter()
        .zip(rigs.iter_mut())
        .enumerate()
        .map(|(p, (seat, rig))| {
            let plane = seat.garage.plane_flown()?;
            let drawn = plane_drawn.get(plane.livery).or(plane_drawn.first())?;
            let body = rig.body.as_mut()?;
            let was = before.get(p).copied().flatten().filter(|_| !cut.get(p).copied().unwrap_or(true));
            let at = was.map_or(plane_at(plane), |b| blend.slide(b, plane_at(plane)));
            let palette = biplane::pilot_palette(body, &drawn.anchors, rig.pilot_lean.weights())?;
            Some(Pilot { plane: at, at: drawn.pilot(at), palette })
        })
        .collect()
}

/// Every seat's helicopter [`Pilot`] (P216b), as [`pilots`] the plane's: the rider's own body posed
/// by the plane's clips (which the original loads for the helicopter's pilot too), its root at
/// [`game_render::kiowa::PILOT_SEAT`] (an MX vs. ATV one's at its model's pelvis box, P316). Unlike the plane's, its limbs are not held: the original's
/// helicopter hands its pilot no limb targets, and the clips are played as they are (P216b1).
/// `None` for a seat not flying one.
fn heli_pilots(seats: &[Seat], rigs: &mut [Rig], before: &[Option<(M4, Rotor)>], cut: &[bool], blend: &pace::Blend) -> Vec<Option<Pilot>> {
    seats
        .iter()
        .zip(rigs.iter_mut())
        .enumerate()
        .map(|(p, (seat, rig))| {
            let heli = seat.garage.heli_up()?;
            let was = before.get(p).copied().flatten().filter(|_| !cut.get(p).copied().unwrap_or(true));
            let (at, _) = heli_drawn(heli, was, blend)?;
            // MX vs. ATV's sits on its model's pelvis box, as SLUS-21104 seats it (P316).
            let pelvis = match heli.mxvatv {
                Some(_) => biplane::mxvatv_seat(heli.phy.pelvis_box_position),
                None => game_render::kiowa::PILOT_SEAT,
            };
            let body = rig.body.as_mut()?;
            let palette = biplane::pilot_palette(body, &[None; 4], rig.pilot_lean.weights())?;
            let mut root = M4::IDENTITY;
            root.0[3] = [pelvis.x, pelvis.y, pelvis.z, 1.0];
            Some(Pilot { plane: at, at: root.mul(at), palette })
        })
        .collect()
}

/// Where a seat's driven car is drawn this frame (S245): its matrix, slid from the tick before's
/// (`before`) as the helicopter is.
fn car_drawn(car: &vehicle::CarRide, before: Option<M4>, blend: &pace::Blend) -> Option<M4> {
    let now = car_draw::placed(car.driven.as_ref()?);
    Some(before.map_or(now, |b| blend.slide(b, now)))
}

/// Every seat's car driver (S245), as [`heli_pilots`] the helicopter's: the rider's own body posed
/// by the plane's clips, its root in the car's seat ([`game_render::car::seat`]) and its limbs held
/// to the model's seat nodes. `None` for a seat not driving one.
fn car_drivers(seats: &[Seat], rigs: &mut [Rig], assets: &Assets, before: &[Option<M4>], cut: &[bool], blend: &pace::Blend) -> Vec<Option<Pilot>> {
    seats
        .iter()
        .zip(rigs.iter_mut())
        .enumerate()
        .map(|(p, (seat, rig))| {
            let car = seat.garage.car_up()?;
            let drawn = game_render::car::Car::found(assets, car_draw::model_name(car.machine, car.livery))?;
            let was = before.get(p).copied().flatten().filter(|_| !cut.get(p).copied().unwrap_or(true));
            let at = car_drawn(car, was, blend)?;
            let loaded = &assets.models.get(&drawn.model)?.model;
            let pelvis = game_render::car::seat(loaded)?;
            let anchors = game_render::car::SEAT.map(|n| game_render::car::node_at(loaded, n).map(|v| v.sub(pelvis)));
            let body = rig.body.as_mut()?;
            let palette = biplane::pilot_palette(body, &anchors, rig.pilot_lean.weights())?;
            let mut root = M4::IDENTITY;
            root.0[3] = [pelvis.x, pelvis.y, pelvis.z, 1.0];
            Some(Pilot { plane: at, at: root.mul(at), palette })
        })
        .collect()
}

/// `--trace-picture`'s line for a pilot drawn this frame (V39): the renderer, the model, and where
/// the body's root and the middle of what was drawn of it sit in the plane's own space — the
/// software path's skinned triangles, the GPU's posed nodes, which its vertex shader skins with.
fn trace_pilot(p: usize, renderer: &str, model: &str, pilot: &Pilot, drawn: &[V3]) {
    let o = V3 { x: pilot.plane.0[3][0], y: pilot.plane.0[3][1], z: pilot.plane.0[3][2] };
    let axis = |i: usize| V3 { x: pilot.plane.0[i][0], y: pilot.plane.0[i][1], z: pilot.plane.0[i][2] };
    // The plane's placement has no scale, so its rows are its own axes and the inverse is a dot each.
    let local = |w: V3| {
        let d = w.sub(o);
        format!("{:.3} {:.3} {:.3}", d.dot(axis(0)), d.dot(axis(1)), d.dot(axis(2)))
    };
    let root = V3 { x: pilot.at.0[3][0], y: pilot.at.0[3][1], z: pilot.at.0[3][2] };
    let n = drawn.len().max(1) as f32;
    let middle = drawn.iter().fold(V3 { x: 0.0, y: 0.0, z: 0.0 }, |a, &v| a.add(v)).scale(1.0 / n);
    println!("pilot: seat {p} {renderer} {model} root {} middle {} of {}", local(root), local(middle), drawn.len());
}

impl<P: sim_world::World> Game for World<P> {
    /// One simulation pass: each player's pad, then their bike, in player order.
    ///
    /// `input` is the engine's own sampled pad, which this shell does not use — the full 32-byte
    /// reports are in each seat and the port's manager is what reads them. Nothing here draws
    /// from `rng`: the only thing that moves is the bikes, and the riders decide where they go.
    ///
    /// The riders meet as online's seats do (N22, `net::rides`): every bike's contacts from where
    /// the last pass left them (S78), each seat's pass resolving its own and pushing the others,
    /// the riders it can joust head-on (S224), and the bikes in its plane's world (S225). The last
    /// seat first, as the original walks its riders (S88). A seat waiting for its pad, frozen on
    /// Start, or on the plane has no bike in the others' world; a ghost is in none (P57).
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
        let spots: Vec<Spot> = self.seats.iter().map(|s| s.garage.spot(&s.sim, &mut self.place)).collect();
        let on: Vec<bool> = self.seats.iter().map(|s| !s.waiting && s.riding && s.garage.up == Kind::Bike).collect();
        fn riders<'a>(seats: &'a [Seat], on: &[bool]) -> Vec<(&'a rider::Rider, bool)> {
            seats.iter().zip(on).map(|(s, &o)| (&s.sim.rider, o)).collect()
        }
        // A flown plane is in the riders' world too (N22a): its hull struck by their bikes, and the
        // planes stepped before the bikes, as the original's dynamics world steps before its riders.
        let flying: Vec<bool> = self.seats.iter().map(|s| !s.waiting && s.riding && s.garage.flight_flown().is_some()).collect();
        let mut hulls: Vec<game_sim::rider_contact::Hull<'_>> = (self.seats.iter().enumerate())
            .filter(|&(i, _)| flying[i])
            .filter_map(|(i, s)| s.garage.flight_flown()?.hull(i))
            .collect();
        let flown: Vec<usize> = hulls.iter().map(|h| h.seat).collect();
        // And a driven car's (F26), after the planes': its `f.col` struck by the bikes' body rays,
        // its body pushed by their resolve, and the mirrors on its record after its own search.
        hulls.extend(
            (self.seats.iter().enumerate())
                .filter(|&(_, s)| !s.waiting && s.riding)
                .filter_map(|(i, s)| s.garage.car_up()?.hull(i)),
        );
        let driven: Vec<usize> = hulls[flown.len()..].iter().map(|h| h.seat).collect();
        let met = game_sim::rider_contact::contacts_met(&riders(&self.seats, &on), &hulls, dt, &self.place);
        drop(hulls);
        let (mut lists, mut mirrors) = match met {
            Some((l, m)) => (Some(l), m),
            None => (None, Vec::new()),
        };
        let n = self.seats.len();
        // What each flown plane's own rays put on each bike's list (N22b), by plane, then by seat:
        // ahead of the bike's own entries, as the original searches its planes first.
        let mut struck: Vec<Vec<Vec<game_sim::contact::Contact>>> = vec![Vec::new(); flown.len()];
        let order: Vec<usize> = (0..n).rev().filter(|&i| flying[i]).chain((0..n).rev().filter(|&i| !flying[i])).collect();
        // The seats' one prop set for the pass (S197c): every seat's bike strikes it, it steps once
        // after them all, and seat 0's, which is drawn, is every seat's.
        let mut props = game_sim::prop_seats::Shared::take(&mut self.seats.iter_mut().map(|s| &mut s.sim).collect::<Vec<_>>());
        // Two planes or more fly their frames together (N22a1), as the original's level node steps
        // its planes in one dynamics world: every flying seat's pad first, then one step of all
        // their planes, then each seat's markers, in the walk's order.
        let together = flown.len() > 1;
        let mut held: Vec<Held> = Vec::new();
        for (step, &me) in order.iter().enumerate() {
            let at = self.seats[me].sim.rider.ride.transform().0[3];
            let firsts: Vec<Vec<game_sim::contact::Contact>> = struck.iter_mut().map(|s| s.get_mut(me).map(std::mem::take).unwrap_or_default()).collect();
            let own = lists.as_mut().map(|l| std::mem::take(&mut l[me]));
            let mut bump = own.map(|l| game_sim::rider_contact::struck_first(&firsts, l, at, &self.place)).filter(|l| !l.is_empty()).map(|l| {
                let phys: Vec<(&rider::Rider, &game_sim::phy::Phy)> = self.seats.iter().map(|s| (&s.sim.rider, &*s.sim.phy)).collect();
                let mut b = game_sim::rider_contact::bump_riders(&phys, me, l, &self.place);
                let from = self.seats[me].sim.rider.ride.transform().0[3];
                b.planes = flown
                    .iter()
                    .filter_map(|&j| Some(self.seats[j].garage.flight_flown()?.flown(j, from, &self.place)))
                    .chain(driven.iter().filter_map(|&j| self.seats[j].garage.car_up()?.flown(j, from, &self.place)))
                    .collect();
                b
            });
            // A flown plane meets the bikes both ways (N22a, N22b): the mirrors of their strikes on
            // its hull, and the bikes its own rays strike, which it pushes.
            let plane = flown.iter().position(|&j| j == me);
            let mut met = plane.and_then(|k| {
                let m = mirrors.get_mut(k).map(std::mem::take).unwrap_or_default();
                let phys: Vec<(&rider::Rider, &game_sim::phy::Phy)> = self.seats.iter().map(|s| (&s.sim.rider, &*s.sim.phy)).collect();
                let at = self.seats[me].garage.flight_flown()?.body.position;
                Some(game_sim::rider_contact::met_plane(&phys, me, n + k, m, at, &self.place))
            });
            // A driven car meets them by the mirrors of their strikes on its hull (F26).
            if let Some(j) = driven.iter().position(|&j| j == me) {
                let m = mirrors.get_mut(flown.len() + j).map(std::mem::take).unwrap_or_default();
                let phys: Vec<(&rider::Rider, &game_sim::phy::Phy)> = self.seats.iter().map(|s| (&s.sim.rider, &*s.sim.phy)).collect();
                met = (self.seats[me].garage.car_up().and_then(|c| c.driven.as_ref()))
                    .map(|c| game_sim::rider_contact::met_riders(&phys, m, c.body.position, &self.place));
            }
            let near = riders(&self.seats, &on);
            let rivals = game_sim::rider_contact::rivals_among(&near, me, &self.place);
            let bikes = self.seats[me]
                .garage
                .plane_flown()
                .map(|p| game_sim::rider_contact::plane_bikes(&near, me, p.at, &self.place))
                .unwrap_or_default();
            drop(near);
            if together && flying[me] {
                let seat = &mut self.seats[me];
                let controls = seat.pass_front(dt);
                let ready = controls.as_ref().filter(|_| seat.riding).and_then(|_| seat.garage.ready_plane(&mut seat.sim, dt, &mut self.place, props.as_mut()));
                held.push(Held { me, plane, controls, ready, met, bikes, bump });
                if order.get(step + 1).is_none_or(|&j| !flying[j]) {
                    self.fly_held(dt, &spots, &on, std::mem::take(&mut held), &mut struck, props.as_mut());
                }
                continue;
            }
            // A downed rider's wreck bike meets a lone flown plane (S276): its search with the hull in
            // its world before the plane's frame, the mirrors on the plane's list, and its step after,
            // which the rider's own pass later in the walk does not repeat.
            let downed = match met.as_mut().filter(|_| plane.is_some()) {
                Some(m) => match flown_among(&mut self.seats, me, &on) {
                    (mut riders, Some(f)) => game_sim::wreck_plane::search_downed(&mut riders, f, dt, &mut self.place, m),
                    _ => game_sim::wreck_plane::Downed::default(),
                },
                None => game_sim::wreck_plane::Downed::default(),
            };
            let among = vehicle::Among { bump: bump.as_mut(), rivals: &rivals, bikes: &bikes, met: met.as_mut(), props: props.as_mut().map(|p| (p, me)) };
            self.seats[me].pass(dt, &mut self.place, me, &spots, among);
            if !downed.is_empty()
                && let Some(m) = met.as_mut()
                && let (mut riders, Some(f)) = flown_among(&mut self.seats, me, &on)
            {
                game_sim::wreck_plane::step_downed(&mut riders, f, downed, m, dt, &self.place, true);
                self.seats[me].garage.plane_reposed();
            }
            self.met_after(plane, met, bump.as_ref(), &mut struck);
        }
        // A flying seat's plane is its striker in the set's step (S259); the garage holds it.
        if let Some(p) = props {
            let (mut sims, flights): (Vec<&mut Sim>, Vec<Option<&mut game_sim::flight::Flight>>) =
                self.seats.iter_mut().map(|s| (&mut s.sim, s.garage.flight_flown_mut())).unzip();
            p.finish_flying(&mut sims, flights, dt, &mut self.place);
            for s in &mut self.seats {
                s.garage.plane_reposed();
            }
        }
        true
    }
}

/// Seat `me`'s flown plane and the riders of the other seats `on` holds in the ride (S276,
/// [`game_sim::wreck_plane`]): the downed among them meet the plane.
fn flown_among<'s>(seats: &'s mut [Seat], me: usize, on: &[bool]) -> (Vec<&'s mut rider::Rider>, Option<&'s mut game_sim::flight::Flight>) {
    let mut flight = None;
    let mut riders = Vec::new();
    for (j, s) in seats.iter_mut().enumerate() {
        if j == me {
            flight = s.garage.flight_flown_mut();
        } else if on[j] {
            riders.push(&mut s.sim.rider);
        }
    }
    (riders, flight)
}

/// **A flying seat's pass, held at its plane's frame** (N22a1): what [`World::fly_held`] needs to
/// fly it with the other planes and finish the seat's pass.
struct Held {
    me: usize,
    /// Its place among the flown planes.
    plane: Option<usize>,
    controls: Option<Controls>,
    ready: Option<vehicle::Readied>,
    met: Option<game_sim::flight::Met>,
    bikes: Vec<game_sim::flight_search::Bike>,
    bump: Option<game_sim::rider_contact::Bump>,
}

impl<P: sim_world::World> World<P> {
    /// What a seat's pass leaves for the others (N22a, N22b): its plane's pushes on the riders'
    /// bikes and the mirrors of its strikes for their lists, and its bike's bumps.
    fn met_after(
        &mut self,
        plane: Option<usize>,
        met: Option<game_sim::flight::Met>,
        bump: Option<&game_sim::rider_contact::Bump>,
        struck: &mut [Vec<Vec<game_sim::contact::Contact>>],
    ) {
        if let (Some(k), Some(m)) = (plane, met) {
            let mut all: Vec<&mut rider::Rider> = self.seats.iter_mut().map(|s| &mut s.sim.rider).collect();
            game_sim::rider_contact::pushed_riders(&mut all, &m);
            struck[k] = m.struck;
        }
        if let Some(b) = bump {
            let mut all: Vec<&mut rider::Rider> = self.seats.iter_mut().map(|s| &mut s.sim.rider).collect();
            game_sim::rider_contact::bumped_riders(&mut all, b);
            for f in &b.planes {
                self.seats[f.seat].garage.hull_pushed(f);
            }
        }
    }

    /// The flying seats' held passes finished (N22a1): their readied planes flown in one
    /// [`vehicle::fly_together`], in the walk's order, then each seat's settle, markers and fader,
    /// and what its plane met handed on ([`World::met_after`]). Among the seats' one prop set
    /// (S259a) the planes strike it, each named by its seat. The downed riders of the seats `on` holds
    /// in the ride meet them (S276a, [`game_sim::wreck_plane::search_downed_by`]): each wreck bike
    /// searched with every plane's hull before the frame and stepped with every plane's body after,
    /// which the rider's own pass later in the walk does not repeat.
    fn fly_held(
        &mut self,
        dt: F,
        spots: &[Spot],
        on: &[bool],
        mut held: Vec<Held>,
        struck: &mut [Vec<Vec<game_sim::contact::Contact>>],
        props: Option<&mut game_sim::prop_seats::Shared>,
    ) {
        {
            let mut seats: Vec<Option<&mut Seat>> = self.seats.iter_mut().map(Some).collect();
            let mut tables = None;
            let mut fleet = Vec::new();
            let mut flown = Vec::new();
            for h in held.iter_mut().filter(|h| h.ready.is_some()) {
                let Some(seat) = seats[h.me].take() else { continue };
                let Some(flight) = seat.garage.readied_flight() else { continue };
                tables.get_or_insert_with(|| seat.sim.tables.clone());
                let pad = h.ready.as_ref().expect("filtered on ready").pad;
                fleet.push(game_sim::flight::Fly { flight, pad, met: h.met.as_mut(), bikes: &h.bikes });
                flown.push(h.me);
            }
            let mut riders: Vec<&mut rider::Rider> =
                seats.iter_mut().zip(on).filter(|&(_, &o)| o).filter_map(|(s, _)| Some(&mut s.take()?.sim.rider)).collect();
            let (planes, mut mets): (Vec<&game_sim::flight::Flight>, Vec<&mut game_sim::flight::Met>) = fleet
                .iter_mut()
                .filter_map(|f| {
                    let game_sim::flight::Fly { flight, met, .. } = f;
                    Some((&**flight, met.as_deref_mut()?))
                })
                .unzip();
            let downed = game_sim::wreck_plane::search_downed_by(&mut riders, &planes, dt, &mut self.place, &mut mets);
            drop((planes, mets));
            if let Some(tables) = tables {
                let props = props.map(|p| (p.set_mut(), &flown[..]));
                vehicle::fly_together(dt, &mut fleet, &mut self.place, &tables.ride, props);
            }
            if !downed.is_empty() {
                let (mut planes, mut mets): (Vec<&mut game_sim::flight::Flight>, Vec<&mut game_sim::flight::Met>) = fleet
                    .iter_mut()
                    .filter_map(|f| {
                        let game_sim::flight::Fly { flight, met, .. } = f;
                        Some((&mut **flight, met.as_deref_mut()?))
                    })
                    .unzip();
                game_sim::wreck_plane::step_downed_by(&mut riders, &mut planes, downed, &mut mets, dt, &self.place, true);
            }
        }
        for h in held {
            let Held { me, plane, controls, ready, met, bump, .. } = h;
            let Some(controls) = controls else { continue };
            let seat = &mut self.seats[me];
            let mut did = None;
            if let Some(ready) = ready {
                (seat.said, did) = seat.garage.settle_plane(&mut seat.sim, me, spots, &mut self.place, seat.markers, ready);
            }
            seat.pass_back(dt, me, controls, did);
            self.met_after(plane, met, bump.as_ref(), struck);
        }
    }
}

#[cfg(test)]
mod split_contact {
    //! N22, S224: split screen's seats meet as online's do — their bikes push apart, and a rider
    //! under another jousts.
    use super::{Controls, Garage, Kind, PADS, PageShade, Report, Seat, World};
    use game_sim::tick::Game;
    use game_sim::rider::Rider;
    use game_sim::world::Flat;
    use game_sim::{Input, Sim};
    use ps2_float::{F, Vf};

    const DT: F = F(game_sim::tick::DeltaTime::CAP_30);

    fn seat(sim: Sim) -> Seat {
        Seat {
            reports: [Report::idle(); PADS],
            sim,
            garage: Garage::new(Kind::Bike, None).unwrap(),
            riding: true,
            start_freezes: false,
            waiting: false,
            paused: false,
            said: Default::default(),
            inverted: false,
            hands: Controls::default(),
            teleported: false,
            clears: 0,
            markers: false,
            together: false,
            shade: PageShade::default(),
        }
    }

    /// A rider on the stock bike at `at`, moving at `vel`, wrecks on but for the airborne clause:
    /// placed in the air, the stand-in bike's answers `NoseDive` on its first frame (`Sim::new` says
    /// why), and a landing clause's wreck is asked before the hit (S191a).
    fn rider_at(at: Vf, vel: Vf) -> Sim {
        let mut sim = Sim::new(0x5354);
        let mounts = game_sim::stock::mounts(&sim.phy);
        sim.rider = Rider::new(&sim.phy, mounts, at, game_sim::stock::FACING, Default::default());
        sim.rider.sense.wrecks_enabled = true;
        sim.rider.sense.air_wrecks = false;
        sim.rider.ride.chassis.vel = vel;
        sim
    }

    fn split(sims: Vec<Sim>) -> World<Flat> {
        World { place: Flat::default(), seats: sims.into_iter().map(seat).collect(), tape: None, played_dt: None }
    }

    /// `game-sim/tests/ride.rs`'s stack, one seat each: the lower rider's head ray strikes the body
    /// of the one above, who is higher by more than `JoustWreckHeight`, and it comes off with the
    /// original's joust (reason 7) on the first pass. With the upper seat waiting for its pad, its
    /// bike is in nobody's world and the lower rider rides on.
    #[test]
    fn a_split_rider_under_another_jousts() {
        let s = game_sim::stock::SPAWN;
        let joust = Some(game_sim::trick::Wreck::from(game_sim::air::Wreck::Joust));
        let two = || {
            split(vec![
                rider_at(Vf::new(s.x(), s.y() + 3.0, s.z(), 1.0), Vf::new(0.0, 0.0, 0.0, 0.0)),
                rider_at(s, Vf::new(1.0, 0.0, 2.0, 0.0)),
            ])
        };
        let mut world = two();
        world.simulate(DT, Input::default(), &mut Default::default());
        assert_eq!(world.seats[1].said.wreck, joust, "the lower seat");
        assert_ne!(world.seats[0].said.wreck, joust, "the upper seat has nobody above");
        let mut world = two();
        world.seats[0].waiting = true;
        world.simulate(DT, Input::default(), &mut Default::default());
        assert_ne!(world.seats[1].said.wreck, joust, "nobody above a seat whose pad is out");
    }

    /// `netplay.rs`'s pair: two seats 1.5 ft apart with the disc's bike collision files push apart
    /// in split screen as online, where the same two without the files stay where they stood.
    #[test]
    fn split_riders_push_apart() {
        use game_render::disc::Disc;
        use game_sim::bike_col::{self, BikeCol};
        let Ok(mut disc) = Disc::open_configured() else {
            eprintln!("skipped: no disc configured");
            return;
        };
        let res = disc.res("Tracks\\free09_ss_2.res").expect("the split track's archive");
        let read = |name: String| res.read_suffix(&name).unwrap_or_else(|e| panic!("{e}"));
        let col = std::sync::Arc::new(
            BikeCol::load(&read(bike_col::rays_file(bike_col::MODEL)), &read(bike_col::shape_file(bike_col::MODEL)))
                .expect("the bike's collision files"),
        );
        let bikes: Vec<String> =
            disc.entries().iter().map(|e| e.name.clone()).filter(|n| n.to_ascii_lowercase().starts_with("bike\\")).collect();
        let phy = bikes.iter().find_map(|b| disc.res(b).ok()?.read_suffix("race125p.phy").ok()).expect("race125p.phy");
        let tables = game_sim::ride::Tables::build();
        let phy = game_sim::phy::Phy::read_bytes(&phy, &tables.curve);
        let s = game_sim::stock::SPAWN;
        let ride = |files: bool| {
            let sims = (0..2)
                .map(|i| {
                    let mut sim = Sim::new(0x5378);
                    std::sync::Arc::make_mut(&mut sim.phy).collisions = phy.collisions;
                    let mounts = game_sim::stock::mounts(&sim.phy);
                    let at = Vf::new(s.x() + 1.5 * i as f32, s.y(), s.z(), 1.0);
                    sim.rider = Rider::new(&sim.phy, mounts, at, game_sim::stock::FACING, Default::default());
                    sim.rider.sense.wrecks_enabled = false;
                    if files {
                        sim.rider.carries_bike_col(std::sync::Arc::clone(&col));
                    }
                    sim
                })
                .collect();
            let mut world = split(sims);
            for _ in 0..30 {
                world.simulate(DT, Input::default(), &mut Default::default());
            }
            let gap = world.seats[1].sim.rider.ride.position().x() - world.seats[0].sim.rider.ride.position().x();
            (world.seats[1].sim.rider.ride.chassis.clone(), gap)
        };
        let ((pushed, gap), (alone, still)) = (ride(true), ride(false));
        assert_ne!(pushed, alone, "side by side with the files, the riders push apart");
        assert!(gap > still, "the gap {gap} is no wider than {still} without the files");
    }

    /// N22a1: split screen's planes meet as online's do (`netplay.rs`'s
    /// `two_online_planes_strike_each_other`): seat 1 flies level at seat 2's plane, which flies back
    /// at it, high over the ground, and the two strike each other's hulls and are held apart. With
    /// seat 2 waiting for its pad, its plane is in nobody's world and seat 1's flies on past where
    /// it was held.
    #[test]
    fn split_planes_strike_each_other() {
        use game_render::disc::Disc;
        use game_sim::flight_search::{self, PLANES, Rays};
        let Ok(mut disc) = Disc::open_configured() else {
            eprintln!("skipped: no disc configured");
            return;
        };
        let name = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("machine.res")).expect("machine.res").name.clone();
        let res = disc.res(&name).expect("machine.res");
        let plane = game_sim::phy::Plane::read_bytes(&res.read_suffix(game_sim::phy::BIPLANE_PHY).expect("biplane.phy"));
        let rays = game_sim::track::Rays::parse(&res.read_suffix(flight_search::BIPLANE_RAYS).expect("biplanem.col")).expect("its rays");
        let hull = game_sim::track::Mesh::parse(&res.read_suffix(game_sim::flight::HULL).expect("biplanef.col")).expect("its hull");
        let flight = game_sim::flight::Flight::new(&plane).with_rays(Rays::of(&rays)).with_hull(hull);
        let fly = |seat_two: bool| {
            let tables = game_sim::ride::Tables::build();
            let at = |z: f32, v: f32| {
                let mut f = flight.clone();
                f.place(Vf::new(0.0, 200.0, z, 1.0), Vf::new(0.0, 0.0, v.signum(), 0.0), &tables.acos);
                f.body.velocity = Vf::new(0.0, 0.0, v, 1.0);
                let mut s = seat(Sim::new(0x2a1));
                // the garage takes the flight as online's follows a seat's (N17)
                s.sim.plane = Some(f);
                s.garage.follow(&s.sim);
                s.sim.plane = None;
                s
            };
            let mut world = World { place: Flat::default(), seats: vec![at(0.0, 120.0), at(80.0, -120.0)], tape: None, played_dt: None };
            world.seats[1].waiting = !seat_two;
            let mut struck = [None, None];
            for _ in 0..20 {
                world.simulate(DT, Input::default(), &mut Default::default());
                for (s, seat) in world.seats.iter().enumerate() {
                    struck[s] = struck[s].or(seat.garage.flight_flown().unwrap().record.struck);
                }
            }
            let z: Vec<f32> = world.seats.iter().map(|s| s.garage.flight_flown().unwrap().body.position.z()).collect();
            (z, struck)
        };
        let (z, struck) = fly(true);
        assert!(struck.iter().all(|s| s.is_some_and(|s| s >= PLANES)), "the planes' records struck {struck:?}, not each other");
        assert!(z[0] < z[1], "the planes flew through each other: z {z:?}");
        let (away, struck) = fly(false);
        assert_eq!(struck[0], None, "nothing was struck");
        assert!(away[0] > z[0] + 30.0, "seat 1's plane stopped at z {} with no plane in its way (held at {})", away[0], z[0]);
    }

    /// F26: a split-screen bike meets the other seat's car as it meets a flown plane (N22a). Seat 1
    /// takes the buggy where its bike stands, 25 ft up seat 0's way and facing along it, and leaves
    /// it parked; seat 0's bike rolls into its tail at 30 ft/s. Its body rays strike the car's hull (`buggyf.col`):
    /// the bike is stopped short of where it rolls on to through a car with no hull, and its
    /// resolve pushes the car along its way, where the car with no hull stands still.
    #[test]
    fn a_split_bike_is_stopped_by_the_other_seats_car() {
        use game_render::disc::Disc;
        use game_sim::bike_col::{self, BikeCol};
        let Ok(mut disc) = Disc::open_configured() else {
            eprintln!("skipped: no disc configured");
            return;
        };
        let res = disc.res("Tracks\\free09_ss_2.res").expect("the split track's archive");
        let read = |name: String| res.read_suffix(&name).unwrap_or_else(|e| panic!("{e}"));
        let col = std::sync::Arc::new(
            BikeCol::load(&read(bike_col::rays_file(bike_col::MODEL)), &read(bike_col::shape_file(bike_col::MODEL)))
                .expect("the bike's collision files"),
        );
        let bikes: Vec<String> =
            disc.entries().iter().map(|e| e.name.clone()).filter(|n| n.to_ascii_lowercase().starts_with("bike\\")).collect();
        let phy = bikes.iter().find_map(|b| disc.res(b).ok()?.read_suffix("race125p.phy").ok()).expect("race125p.phy");
        let tables = game_sim::ride::Tables::build();
        let phy = game_sim::phy::Phy::read_bytes(&phy, &tables.curve);
        let name = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("machine.res")).expect("machine.res").name.clone();
        let machines = disc.res(&name).expect("machine.res");
        let buggy = crate::vehicle::CarRide::new(crate::vehicle::Machine::Buggy, 0, None, &mut |n| machines.read_suffix(n).ok())
            .expect("the buggy's files");
        assert!(buggy.hull.is_some(), "machine.res has no buggyf.col");
        let ride = |hull: bool| {
            let sims: Vec<Sim> = [(0.0, game_sim::stock::FACING), (25.0, game_sim::stock::FACING)]
                .into_iter()
                .map(|(z, facing)| {
                    let mut sim = Sim::new(0xf26);
                    std::sync::Arc::make_mut(&mut sim.phy).collisions = phy.collisions;
                    let mounts = game_sim::stock::mounts(&sim.phy);
                    sim.rider = Rider::new(&sim.phy, mounts, Vf::new(0.0, 1.0, z, 1.0), facing, Default::default());
                    sim.rider.sense.wrecks_enabled = false;
                    sim.rider.carries_bike_col(std::sync::Arc::clone(&col));
                    sim
                })
                .collect();
            let mut world = split(sims);
            let mut car = buggy.clone();
            if !hull {
                car.hull = None;
            }
            let seat = &mut world.seats[1];
            seat.garage.cars.push(car);
            seat.garage.switch(Kind::Buggy, &mut seat.sim, &mut world.place).expect("the buggy is taken");
            // Settled on its springs before the bike sets off.
            for _ in 0..30 {
                world.simulate(DT, Input::default(), &mut Default::default());
            }
            let parked = world.seats[1].garage.car_up().and_then(|c| c.driven.as_ref()).expect("the buggy is out").body.position;
            world.seats[0].sim.rider.ride.chassis.vel = Vf::new(0.0, 0.0, 30.0, 0.0);
            let mut fastest = 0.0f32;
            for _ in 0..45 {
                world.simulate(DT, Input::default(), &mut Default::default());
                let car = world.seats[1].garage.car_up().and_then(|c| c.driven.as_ref()).expect("the buggy is out");
                fastest = fastest.max(car.body.velocity.z());
            }
            let bike = world.seats[0].sim.rider.ride.position().z();
            let car = world.seats[1].garage.car_up().and_then(|c| c.driven.as_ref()).expect("the buggy is out").body.position;
            (bike, car.z() - parked.z(), fastest)
        };
        let ((bike, pushed, fastest), (through, still, idle)) = (ride(true), ride(false));
        println!(
            "bike at z {bike}, the car pushed {pushed} ft, at most {fastest} ft/s; with no hull the bike at z {through}, the car moved {still} ft, at most {idle} ft/s"
        );
        assert!(through > 30.0, "with no hull the bike stopped at z {through}, short of the car's far side");
        assert!(bike < 25.0, "the bike went on to z {bike}, through the car at z 25");
        // Parked on an idle pad the buggy creeps on along its nose a little either way.
        assert!(fastest > idle + 1.0, "the car was pushed to {fastest} ft/s along the bike's way (with no hull {idle})");
        assert!(pushed > still + 0.5, "the car moved {pushed} ft along the bike's way (with no hull {still})");
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
    /// `--shot-pose` (MD10): the first view drawn from this pose, not the ride camera, all the run.
    shot_pose: Option<shot_pose::Pose>,
    /// `--shot-pose` given again (P142): the second view's pose, then the third's, and so on.
    more_poses: Vec<shot_pose::Pose>,
    /// `--overview` (P166): a still of the map for the track pick — no machine or rider drawn, and
    /// the first view from [`track_previews::overview`]'s framing unless `--shot-pose` says one.
    overview: bool,
    /// `--overlay hud|none` (V26, P121, P170): which overlay a ride draws, the ride HUD by default.
    /// Unset, the front end's setting decides.
    overlay: Option<menu::Overlay>,
    /// `--speedometer digital|analog` (V42): how the original's HUD shows the speed. Unset, the
    /// front end's setting decides, and without one the digital readout.
    speedometer: Option<menu::Speedometer>,
    /// `--minimap on|off` (V52): whether the original's HUD draws its radar. Unset, the front end's
    /// setting decides, and without one it does.
    minimap: Option<bool>,
    /// `--show-fps on|off` (2026-10-29): whether a ride draws its frame rate in the top right, the
    /// Settings' Show FPS. Unset, the front end's setting decides, and without one it does not.
    show_fps: Option<bool>,
    /// `--world-sky global|per-map` (MD9): a combined world's sky and light, one for all of it or the
    /// map under the camera's. Unset, the front end's setting decides, and without one global.
    world_sky: Option<menu::WorldSky>,
    /// `--hud-scale 50..150` (V52): the HUD's size in percent of the original's, in the Settings
    /// row's steps of ten. Unset, the front end's setting decides, and without one 100.
    hud_scale: Option<u8>,
    /// `--camera NAME`: which of the track's camera presets to start on, by the name its
    /// `cpresets.ini` gives it. The default is the one the original itself is on in a freeride (P14).
    camera: String,
    /// Whether `--camera` was given, which wins over the preset a seat was last left on (P163).
    camera_asked: bool,
    /// `--camera-view original|default|FOV,DIST,HEIGHT,PDIST,PHEIGHT` (P82): every player's camera
    /// framing, over the front end's Camera page. Unset, the page's, and without a front end the
    /// shipped default.
    camera_view: Option<menu::CameraView>,
    /// `--trace-camera` (P119): one line a frame for each view of what its follow camera did — the
    /// preset, the distance, the pitch, the field of view, the eye and the look-at — for finding a
    /// camera that swings.
    trace_camera: bool,
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
    /// Whether the bike's attitude in the air can throw the rider off
    /// ([`game_sim::state::Sense::air_wrecks`], P138): the Settings screen's Air Wrecks row, which
    /// `--no-air-wrecks` turns off. On by default, the original's.
    air_wrecks: bool,
    /// Whether a landing with the bike facing back against its way throws the rider off
    /// ([`game_sim::state::Sense::backwards_bail`], S161): the Settings screen's Backwards Landings
    /// Bail row, which `--backwards-bail` turns on. Off by default, the original's.
    backwards_bail: bool,
    /// Whether a wreck rumbles the rider's pad (P183, [`pad::WreckRumble`]): `--no-vibration` turns
    /// it off, and under the front end the Controls tab's Vibration row does, live.
    vibration: bool,
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
    /// `--seams`: on a world, walk every seam between its pieces (and the wrap's) a foot a step and
    /// say what the ride's ground has on either side of each (MD47).
    seams: bool,
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
    /// `--fly-at X,Z,DEG,FT` (V96): the first seat's plane put in the air `FT` feet over the ground
    /// at `X,Z`, heading `DEG`, already flying — a high flight to measure without the climb.
    fly_at: Option<[f32; 4]>,
    /// `--world-ground all`: build a world's whole ground before the first frame instead of the
    /// pieces in reach of the start (MD17).
    world_ground_all: bool,
    /// `--world-reach LOAD,DROP`: how near a world's pieces are loaded and how far they are let go,
    /// in feet, instead of the desktop's ([`game_sim::tiles::Reach::DESKTOP`]).
    world_reach: Option<game_sim::tiles::Reach>,
    /// `--world-fine LOAD,DROP|off`: how near a world's GPU ground is built at full detail, the rest
    /// in reach coarse (P286), instead of [`worlds::FINE_REACH`]; `off` builds it all at full detail.
    world_fine: Option<game_sim::tiles::Reach>,
    /// `--world-chunks N`: the most ground chunks a frame of a world ride hands the GPU instead of
    /// [`worlds::CHUNKS_A_FRAME`] (MD28).
    world_chunks: Option<usize>,
    /// `--world-stand-in on|off`: draw a world's whole ground at its coarsest where its pieces are
    /// not loaded, and fade each piece in over it (V92). On by default.
    world_stand_in: bool,
    /// `--warm on|off`: draw the ride's first picture once offscreen, and cut a world's near shapes,
    /// before the ride's clock starts (P150), so its first frame is not the slowest. On by default.
    warm: bool,
    /// `--world-sweep X0,Z0,X1,Z1`: a world's ground streams about a point that goes from `x0, z0` to
    /// `x1, z1` in world feet over the ride's `--frames` instead of about the riders (MD23), so a
    /// short ride can be seen to load and let go a whole map's ground; the riders and the
    /// simulation's tiles are as they would be without it.
    world_sweep: Option<[f32; 4]>,
    /// `--trace-switch N`: under `--trace-feel`, ride the first N frames on the throttle alone and only
    /// then take the controls the other flags ask for — a full brake from top speed, or a turn at speed.
    trace_switch: Option<u32>,
    /// `--online-script F:WORD,...`: online, from the session's tick `F` on (K29: the tick, as
    /// `--frames` counts online, not the shell's frame, which waits on a peer), its first player picks
    /// the machine `WORD` names (bike, biplane, buggy, monster, baja) as the M key or the pause's
    /// switch would, or holds Cross down (`gas`) or lets it go (`lift`) — a played tape's switch and
    /// throttle for a ride that rides no tape (S249's two-shell test).
    online_script: Vec<(u32, String)>,
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
    /// `--aspect auto|4:3|16:9|16:10|21:9|stretch`: the frame at the window's own shape, or the
    /// largest one of the fixed shape inside it with bars round it (P176); 4:3 is the shell as it
    /// was before V24.
    aspect: Aspect,
    /// Whether `--aspect` was given: if so it wins over the settings' Picture Shape (P126).
    aspect_asked: bool,
    /// `--plants smooth|original` (P126): how the plants change from model to picture, V55's smooth
    /// way or the original's; the front end's setting when not given.
    plants: Option<game_render::eco::Lod>,
    /// `--foliage cutout|original` (V57): the plants' leaves cut out (V56) or the original's
    /// blended ones; the front end's setting when not given.
    foliage: Option<game_render::eco::Foliage>,
    /// `--textures upscaled|original` (V84): the world's textures from texture packs,
    /// `mods/texture-replacements/` and mods' own upscales, or the disc's alone; the front end's
    /// Upscaled Textures setting when not given.
    textures: Option<bool>,
    /// `--texture-filtering off|2|4|8|16` (V83): how far the GPU filters textures anisotropically;
    /// the front end's setting when not given, 16x without one.
    filtering: Option<menu::Filtering>,
    /// `--shadows off|on|all` (V108, user 2026-10-30): what casts the sun's shadows, the movers or
    /// the whole world too; V108's `better` and `original` still taken, each setting the quality it
    /// meant too. The front end's setting when not given, On without one.
    shadows: Option<game_render::shadow::Shadows>,
    /// `--shadow-quality original|low|high` (user 2026-10-30): the original's patch under each
    /// racer, or our maps at Low or High; the front end's setting when not given, High without one.
    shadow_quality: Option<game_render::shadow::ShadowQuality>,
    /// `--wheels original` (V59): the bike's wheels drawn turning at the original's own angle and
    /// never blurred, rather than held under the wagon-wheel alias and blurred when they spin fast.
    original_wheels: bool,
    /// `--headless`: no window and no host pad — every seat rides an idle pad, or the first rides
    /// `--play`'s tape — and the frame is drawn all the same, for `--shot` and for the tests.
    headless: bool,
    /// `--frames N`: quit after N frames, which unlike `--seconds` is the same run on any machine;
    /// `--online`, after the session's Nth tick, however long the peers took to load (K10).
    frames: Option<u32>,
    /// `--steady-clock`: the ride's clock moves on by each picture's floor (`--draw-fps`', a headless
    /// ride's 60 a second) and not with the wall's, so an offline ride is the same ride on a busy
    /// machine as on an idle one (`pace::Clock`). For the tests; online peers keep the wall's.
    steady_clock: bool,
    /// `--sim-thread auto|on|off` (N23a): whether a tick may run on a thread of its own while the
    /// picture before it is drawn. Auto when a window's split screen overruns its pictures' time.
    sim_thread: sim_thread::Mode,
    /// `--draw-fps N`: draw at most N pictures a second. Unset, a window draws as fast as its
    /// display takes them and a headless ride draws 60. The ride's ticks are 30 a second either way
    /// (P52).
    draw_fps: Option<f32>,
    /// `--vsync on|off` (P90): whether the picture waits for the display; the settings' when not given.
    vsync: Option<bool>,
    /// `--fps-limit off|N|refresh` (P90): the most pictures a second; the settings' when not given.
    fps_limit: Option<menu::FrameLimit>,
    /// `--menu`: the front end, whatever else is asked.
    menu: bool,
    /// Whether a flag picked a ride, which skips the front end unless `--menu` says otherwise.
    ride_flag: bool,
    /// `--screen`: the front end's screen to open on.
    screen: Option<menu::Screen>,
    /// `--screen`'s tab of Settings or the Garage, when its word names one (P123).
    screen_tab: Option<usize>,
    /// `--open-list`: the dropdown of the row `--screen` opens on, open (P277's shot).
    open_list: bool,
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
    /// The garage's look for the first player (P86): what they wear over the gear set `rider` names,
    /// and their name and number.
    look: roster::Look,
    /// `--garage`: ride `garage.txt`'s picks without the front end, every seat its own.
    garage: bool,
    /// `--fps`: a line a second with the frame rate, the slow frames and each stage's time ([`perf`]).
    fps: bool,
    /// `--renderer gpu|software`: what draws the picture (P44); the GPU unless asked.
    renderer: Backend,
    /// `--graphics original|far|farther|full` (V32): the original's fog and far clip, the same four times as
    /// far, eight times as far (V95), or the whole map with a haze, and the Fog slider's other steps
    /// (`x2`, `x3`, `x6`, `x12`, `x16`, P276); the front end's setting when not given. Only the GPU
    /// draws any but `original`.
    graphics: Option<Graphics>,
    /// `--render-scale auto|F`: the fraction of the frame the GPU draws the 3D at (V30); `None` is
    /// auto, picked from the frames' time in a window and whole offscreen.
    render_scale: Option<f32>,
    /// Whether `--render-scale` was given, `auto` included (V34): if so it wins over the settings'.
    render_scale_asked: bool,

    /// `--detail full|reduced` (P223): the models' level of detail; the front end's Model Detail row
    /// when not given.
    detail: Option<menu::video::Detail>,
    /// Whether any graphics row's flag was given (P223): no Auto preset stepping them then.
    video_asked: bool,
    /// The `mods/` folder a track not on the disc is looked for in (MD2), or `None` to read none.
    /// Unless `mods_asked`, the settings' Mods row says, and the folder is beside `settings.txt`.
    mods: Option<std::path::PathBuf>,
    /// Whether `--mods DIR|off` was given, or the front end set [`Args::mods`] from its settings.
    mods_asked: bool,
    /// `--mod-store URL` (P198): the mod store's address in place of Hugging Face's, a test's own.
    mod_store: Option<String>,
    /// `--tricks original|mxvatv` (MD16): whose tricks the ride has. Unset, the front end's Tricks
    /// row says, and a ride outside it has the original's. `--online` is always the original's; a
    /// ride from the lobby is its room's ([`Args::room_tricks`]).
    tricks: Option<menu::Tricks>,
    /// `--tricks ID[,ID…]` (MD77): which trick packs MX vs. ATV's tricks stack, by id; unset or empty,
    /// the front end's rows say, and a ride outside it has the first pack in the folder alone.
    trick_packs: Option<Vec<String>>,
    /// `--handling-pack ID` (MD77): the pack MX vs. ATV's handling reads; unset or empty, the front
    /// end's Handling From row says, else the first trick pack on, else the first in the folder.
    handling_pack: Option<String>,
    /// `--handling original|mxvatv|untamed` (MD40, MD93): how the bikes handle. Unset, the front end's Handling row
    /// says, and a ride outside it has the original's. `--online` is always the original's; a ride
    /// from the lobby is its room's ([`Args::room_handling`], N21).
    handling: Option<game_sim::handling::Handling>,
    /// `--brake-gyro on|off` and `--gas-gyro on|off` (P226): MX vs. ATV's two gyro options, read on its
    /// handling (and Untamed's) alone. Unset, the front end's Brake Gyro and Gas Gyro rows say, and a
    /// ride outside it has the game's default, brake on and gas off. A lobby's ride keeps the default,
    /// as its handshake holds no seat's gyros.
    brake_gyro: Option<bool>,
    gas_gyro: Option<bool>,
    /// `--fliers original|mxvatv` (MD80e): whose biplane and Kiowa the ride has. Unset, the front
    /// end's Fliers row says, and a ride outside it has MX Unleashed's. `--online` always has MX
    /// Unleashed's.
    fliers: Option<bool>,
    /// `--flier-skin 1..4` (MD80e): which of MX vs. ATV's four skins the first player's fliers wear.
    /// Unset, the front end's Flier Skin row says, else the first.
    flier_skin: Option<usize>,
    /// The pack whose `mx_*.phy` a lobby's ride rides on MX vs. ATV's handling (N21): this machine's
    /// mods folder and the pack's id, as the room names it and the handshake held every seat to.
    /// `None` outside the lobby and on the original's handling.
    room_handling: Option<(std::path::PathBuf, String)>,
    /// The trick pack a lobby's ride rides (MD19): this machine's mods folder and the pack's id, as
    /// the room names it and the handshake held every seat to. `None` outside the lobby.
    room_tricks: Option<(std::path::PathBuf, String)>,
    /// `--markers on|off` (P129), one word a seat (`on,off`), the last for every seat past it: each
    /// player's Markers switch. Unset, the front end's Markers rows say, and a ride outside it has
    /// them on.
    markers: Option<[bool; menu::SEATS_MAX]>,
    /// `--where` (P190): say which disc and which settings the shell would start with, and stop.
    where_only: bool,
    /// The pad seat each player reads, the front end's local game's (P43): player i rides seat
    /// `pad_of[i]`'s controller. Empty is seat i for player i, which is every ride outside one.
    pad_of: Vec<usize>,
    /// `--no-sound`: no sound and no music (P42). A headless ride has none either.
    sound: bool,
    /// `--timings`: a line a second of what each stage of the frame cost (P47, [`stages`]), and
    /// one over the whole session at the end.
    timings: bool,
    /// `--trace-picture`: a line a drawn frame of where the first player's bike, rider and camera
    /// were drawn, and the blend between the ticks they were drawn at (P120).
    trace_picture: bool,
    /// `--trace-hash`: a line a tick of every seat's state hash (N23a), which a ride with the sim
    /// thread and one without hold against each other.
    trace_hash: bool,
}

impl Args {
    /// Player `i`'s Markers switch (P129): `--markers`' where given, else the front end's Markers
    /// row for the seat, else on.
    fn markers_of(&self, i: usize, front: Option<&Front>) -> bool {
        let i = i.min(menu::SEATS_MAX - 1);
        self.markers.map(|m| m[i]).or_else(|| front.map(|f| f.menu.settings.markers[i])).unwrap_or(true)
    }

    /// The least time a picture takes (P52): `--draw-fps`', a headless ride's 60 a second, or none.
    fn draw_frame(&self) -> Option<Duration> {
        match self.draw_fps {
            Some(fps) => Some(Duration::from_secs_f32(1.0 / fps)),
            None if self.headless => Some(FRAME),
            None => None,
        }
    }

    /// VSync and the frame limit (P90): `--vsync` and `--fps-limit` where given, else `settings`'.
    /// Whether a graphics row was given on the command line (P223), before the front end fills the
    /// ride's from its settings: the Auto preset then leaves the rows be.
    fn video_flags(&self) -> bool {
        self.graphics.is_some()
            || self.plants.is_some()
            || self.filtering.is_some()
            || self.shadows.is_some()
            || self.shadow_quality.is_some()
            || self.detail.is_some()
            || self.render_scale_asked
            || self.fps_limit.is_some()
    }

    fn pacing(&self, settings: &menu::Settings) -> (bool, menu::FrameLimit) {
        (self.vsync.unwrap_or(settings.vsync), self.fps_limit.unwrap_or(settings.frame_limit))
    }

    /// The pad seat player `i` reads.
    fn pad_seat(&self, i: usize) -> usize {
        self.pad_of.get(i).copied().unwrap_or(i)
    }
}

/// The window presented with VSync or without, live, and the least time a picture takes for the
/// frame limit (P90, [`pace::frame_floor`]). A display that does not say its refresh is taken as 60.
fn picture_floor(window: &mut Screen, (vsync, limit): (bool, menu::FrameLimit)) -> Option<Duration> {
    window.set_vsync(vsync);
    // A phone's screen runs at a limit that divides its refresh, which then reads as the refresh (P248).
    #[cfg(target_os = "ios")]
    window.screen_rate(limit, vsync);
    let refresh = window.refresh().unwrap_or(60.0);
    pace::frame_floor(vsync, limit.fps(refresh), refresh, window.waits())
}

/// The menu's floor (P90): its counts — the key repeat, the slideshow — are 60 Hz frames, so it
/// keeps 60 whatever the ride's picture does, and only a lower limit slows it. A present that waits
/// on a 60 Hz display is that pace already, and a sleep on top of it would miss refreshes.
fn menu_floor(window: &Screen, picture: Option<Duration>) -> Option<Duration> {
    let refresh = window.refresh().unwrap_or(60.0);
    let own = match window.waits() && (refresh - 60.0).abs() < 1.0 {
        true => None,
        false => Some(FRAME),
    };
    own.max(picture)
}

/// The garage's picks on a ride (P37): the first seat's bike, set and rider are the ride's own, and
/// every other seat's go to that seat.
fn apply_picks(args: &mut Args, picks: &[roster::Pick]) {
    let Some((first, rest)) = picks.split_first() else { return };
    args.bike = first.bike.clone();
    args.rider = first.rider.clone();
    args.set = Some(first.set.ridden());
    args.look = first.look.clone();
    args.others = rest.to_vec();
}

/// The first player's pick as the ride draws it (N16): what an online peer whose pick does not say
/// is drawn as.
fn own_pick(args: &Args) -> roster::Pick {
    roster::Pick {
        bike: args.bike.clone(),
        set: args.set.unwrap_or(roster::Set::FREERIDE),
        rider: args.rider.clone(),
        look: args.look.clone(),
        preset: String::new(),
    }
}

/// The command line: the desktop's own, or in the browser what the page gave `openmx_run` (P48:
/// its `?args=`).
fn argv() -> Vec<String> {
    // the disc a player starts it with comes first, and is [`player::start`]'s (P190)
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut argv = std::env::args().skip(1).collect();
        player::take_disc(&mut argv);
        argv
    }
    #[cfg(target_arch = "wasm32")]
    return crate::page_args();
}

/// A `--timings` line: standard output on the desktop; in the browser, whose standard output goes
/// nowhere, the page's console (P205: the ride's profile in headless chromium).
fn timing(line: String) {
    #[cfg(not(target_arch = "wasm32"))]
    println!("{line}");
    #[cfg(target_arch = "wasm32")]
    crate::host_say(&line);
}

/// This build's online id (P189, `build.rs`): its version and commit, so two builds refuse each other's lobbies.
const BUILD_ID: &str = env!("OPENMX_BUILD_ID");

pub(crate) async fn run() -> Result<(), String> {
    // before any lobby: the desktop's, the iPhone's and the browser's front ends all come through here
    openmx_net::handshake::set_build(BUILD_ID);
    let argv: Vec<String> = argv();
    if argv.first().is_some_and(|a| a == "--dump-textures") {
        return dump_textures(&argv[1..]);
    }
    if argv.first().is_some_and(|a| a == "--machine-packs") {
        return machine_packs_report(&argv[1..]);
    }
    // P199: the store maintainer's card, index and uploads, on a thread of its own (its own runtime)
    #[cfg(not(target_arch = "wasm32"))]
    if argv.first().is_some_and(|a| a == "--mod-store-admin") {
        let rest = argv[1..].to_vec();
        return std::thread::spawn(move || modstore::up::admin(&rest)).join().map_err(|_| "the store admin stopped".to_string())?;
    }
    let Some(args) = parse_args()? else {
        return Ok(());
    };
    // P223: a phone's textures no wider than 512 (opentennis's B547), before any is loaded.
    let cap = menu::video::texture_cap(&menu::video::Device::here());
    game_render::gpu::TEXTURE_CAP.store(cap, std::sync::atomic::Ordering::Relaxed);
    if cap > 0 {
        println!("textures: {cap} px at most (a phone's)");
    }
    // the disc and where the settings are (P190), once the command line is known to be one
    #[cfg(not(target_arch = "wasm32"))]
    {
        let start = player::start(&mut std::env::args().skip(1).collect())?;
        // P191: the main menu's updater, and an update's leftover (Windows' .old) gone. iOS has no
        // binary to swap: its newer release offers Download, the release page.
        update::cleanup();
        let _ = menu::UPDATER.set(update::updater());
        if args.where_only {
            let disc = Disc::configured_path()?;
            let dir = menu::store_dir(Some(&disc));
            println!("disc: {} (from {})", disc.display(), start.from.word());
            match &start.data {
                Some(data) => println!("data: {}", data.display()),
                None => println!("data: none, a dev checkout's kit.toml"),
            }
            println!("settings: {}", dir.join(menu::SETTINGS_FILE).display());
            println!("rider: {}", menu::load(&dir).0.name);
            return Ok(());
        }
    }
    if args.menu || !args.ride_flag {
        return front_end(args).await;
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
    ride(args, None).await.map(|_| ())
}

/// The window, the pad and the menu, which outlive a ride under the front end.
struct Shell {
    /// Always open under the front end; an `Option` because a ride without one may be headless.
    window: Option<Screen>,
    seats: Option<Seats>,
    front: Front,
}

/// The rate the pause's readers are read at: the ride's ticks, which step it ([`pace::TICK`]).
const RIDE_HZ: u32 = 30;

/// The front end's own half of [`Shell`].
struct Front {
    menu: Menu,
    /// The pause's reader, of 1P's seat and the keyboard with it when a pad is 1P (P46, P109), read
    /// at the ride's ticks.
    reader: Reader,
    /// The other seats' readers, for the garage's columns (P37).
    seat_readers: Vec<Reader>,
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
    /// `--mods` was given (MD7): the music packs are its folder's whatever the Mods row says.
    mods_asked: bool,
    /// P298: the resume point the main menu offers until it is answered, the ride it was answered
    /// with, and that ride's state, for the next ride to put over its first seat.
    resume: Resume,
}

/// [`Front::resume`].
#[derive(Default)]
struct Resume {
    offered: Option<resume::Point>,
    ride: Option<resume::Ride>,
    state: Option<Vec<u8>>,
}

impl Front {
    /// The pad's wiring: the command line's where it was given, `controls.txt`'s otherwise.
    fn wiring(&self) -> Wiring {
        let mut w = self.args_wiring.clone();
        if !self.wiring_flags.0 {
            w.throttle = match self.menu.controls.throttle {
                menu::Gas::Cross => Throttle::Cross,
                menu::Gas::R2 => Throttle::Trigger(Control::R2),
            };
        }
        if !self.wiring_flags.1 {
            match Wiring::parse_pressures(&self.menu.controls.pressure) {
                Ok(map) => w.pressure = map,
                Err(e) => println!("{}: pressure: {e}", menu::CONTROLS_FILE),
            }
        }
        w.nintendo_positions = self.menu.controls.nintendo_positions;
        w.keys = if self.menu.controls.home_row_keys { pad::Keys::HomeRow } else { pad::Keys::Modern };
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
        // The music packs the Mods page has on (MD7), and the song lists again when that changed them.
        if self.sound.music_packs(s.mods || self.mods_asked, &s.mods_off) {
            self.menu.music_choices = self.sound.choices();
        }
        for tune in self.menu.take_tunes() {
            match tune {
                menu::Tune::Play(id) => self.sound.play_now(&id),
                menu::Tune::Skip => self.sound.skip(),
                menu::Tune::Back => self.sound.back(),
                menu::Tune::Hold(on) => self.sound.hold(on),
            }
        }
    }

    /// The front end's music in place of the ride's songs while `front_end` is up, and the clicks
    /// the menu's presses made this frame (P65).
    fn cue(&mut self, front_end: bool) {
        self.sound.front_end(front_end);
        for cue in self.menu.take_cues() {
            self.sound.click(match cue {
                menu::Cue::Move => audio::Click::UpDown,
                menu::Cue::Side => audio::Click::LeftRight,
                menu::Cue::Accept => audio::Click::Accept,
                menu::Cue::Back => audio::Click::Cancel,
                menu::Cue::Refused => audio::Click::Locked,
            });
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

/// A GPU drawing offscreen, for `--headless`; the browser has none to offer yet (P48).
fn headless_gpu() -> Result<Renderer, String> {
    #[cfg(not(target_arch = "wasm32"))]
    return Renderer::headless();
    #[cfg(target_arch = "wasm32")]
    return Err("the browser draws in software for now (P48)".into());
}

/// The sound (P42): the music sources — the disc's songs, then the user's own folder beside the
/// settings, then the music packs in the mods folder (MD7) — and the player it all goes out through,
/// set as `settings` says. Silent with
/// `--no-sound` or `--headless`, and where the desktop has no player.
/// `front_end` when the menus come first: no song until a ride.
fn start_sound(args: &Args, disc: &mut Disc, dir: &std::path::Path, settings: &menu::Settings, front_end: bool) -> audio::Speakers {
    let image = Disc::configured_path().ok();
    let mut sources = audio::sources(image.map(|path| (disc, path)), dir);
    // The packs are read whatever the Mods row says, and only play while it is on.
    sources.push(std::sync::Arc::new(read_music_packs(mods_folder(args, dir, true).as_deref())));
    // Which song a shuffle starts on is the host's business and nothing of the game's: the clock.
    let seed = clock::SystemTime::now().duration_since(clock::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64);
    let music = audio::Music::new(sources, seed);
    println!(
        "music: {} songs from {}; your own .wav files go in {}",
        music.tracks().len(),
        music.sources().join(" and "),
        audio::folder(dir).display()
    );
    let speakers = audio::Speakers::start(music, args.sound && !args.headless);
    if front_end {
        speakers.open_on_front_end();
    }
    speakers.apply(&settings.music, levels(settings));
    speakers.music_packs(settings.mods || args.mods_asked, &settings.mods_off);
    speakers
}

/// The music packs in `mods` (MD7), saying how many songs and what was left out.
fn read_music_packs(mods: Option<&std::path::Path>) -> music_packs::PackMusic {
    let (packs, log) = music_packs::PackMusic::read(mods);
    for line in log {
        println!("{line}");
    }
    if packs.len() > 0 {
        println!("music: {} songs from music packs", packs.len());
    }
    packs
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
    /// The load was cancelled (P127): back to the track pick.
    Pick,
}

/// The front end (P36): the menus until one of them starts a ride, the ride, and back.
async fn front_end(args: Args) -> Result<(), String> {
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
    let mut sound = start_sound(&args, &mut disc, &dir, &settings, true);
    let camera_names = camera_names(&mut disc);
    let heli_camera_names = heli_camera_names(&mut disc);
    let car_camera_names = car_camera_names(&mut disc);
    // The rider's own 24 (MD77), which the Ride tab's trick pack rows count each pack's extras past.
    let rider_stunts = front_rider_stunts(&mut disc);
    drop(disc);
    // The front end's music and clicks (P65), when anything plays them.
    if sound.live().is_some() {
        match Disc::open_configured().and_then(audio::MenuSounds::load) {
            Ok(m) => sound.menu_sounds(m),
            Err(e) => println!("sound: the menus' sounds: {e}"),
        }
    }
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
        menu: Menu::new(settings, controls).with_garage(catalogue, picks).with_looks(roster::Presets::load(&dir)),
        reader: Reader::at(RIDE_HZ),
        seat_readers: (0..roster::SEATS).map(|_| Reader::new()).collect(),
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
        mods_asked: args.mods_asked,
        resume: Resume::default(),
    };
    front.menu.music_choices = front.sound.choices();
    front.menu.camera_names = camera_names;
    front.menu.heli_camera_names = heli_camera_names;
    front.menu.car_camera_names = car_camera_names;
    // The track mods (MD2), after the disc's tracks in the pick; read again when the Mods row turns.
    let mut mods_on = front.menu.settings.mods;
    let listed = mod_tracks(&args, &front.dir, mods_on);
    // And each one's picture on its card (P70), the stills that are missing taken off this thread.
    let mut stills = mod_pictures(&mut front.art, mods_folder(&args, &front.dir, mods_on).as_deref(), &listed);
    front.menu.set_mod_games(mod_games(&args, &front.dir, mods_on, &listed));
    front.menu.set_mod_categories(mod_categories(&args, &front.dir, mods_on, &listed));
    front.menu.set_mod_tracks(listed);
    front.menu.set_music_packs(music_packs(&args, &front.dir, mods_on));
    front.menu.set_trick_packs(trick_pack_rows(&args, &front.dir, &rider_stunts));
    // The mod store (P198): into the mods folder beside the settings, or `--mods DIR`'s; none under
    // `--mods off`.
    let mut store = StoreFront::new(&args, &front.dir);
    front.menu.store.off = store.is_none();
    // Online, this machine's player rides the freeride's bike set unless the lobby changes it, as
    // the rider the command line names.
    front.menu.lobby.bikes = lobby::bikes();
    front.menu.lobby.pick = openmx_net::Pick::new(&args.rider, &roster::Set::FREERIDE.word());
    // In the first seat's look from the garage (P86), which the seat list shows the others.
    front.menu.lobby.pick.look = front.menu.picks.0[0].look.text();
    // And on the bike model this machine draws its own as, which every other machine draws (N16).
    front.menu.lobby.pick.model = args.bike.clone();
    front.menu.lobby.lan = front.network.online().relay == openmx_net::Relay::Off;
    let window = Screen::open(args.renderer, &args.display, args.size, args.scale, "openmx")?;
    let seats = open_seats(&args, pad.as_deref(), front.wiring());
    // The Controls screen lists the keyboard under each layout (P54), as the bindings file has it.
    // And the original's on each of its controller presets (P79), in `menu::keyboard_at`'s order.
    // The new layout's on both its keyboards (P207), modern then the home row.
    front.menu.keyboard = std::array::from_fn(|i| match i {
        0 => seats.keyboard_layout_keys(pad::Keys::Modern),
        1 => seats.keyboard_layout_keys(pad::Keys::HomeRow),
        p => seats.keyboard_layout_on(Scheme::Original, p - 2),
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
        Some(screen) => match args.screen_tab {
            Some(tab) => front.menu.open_tab(screen, tab),
            None => front.menu.open(screen),
        },
        None => front.menu.splash(),
    }
    if args.open_list {
        front.menu.open_list();
    }
    // `--screen store` (P198's browser check) opens the store without Browse's ask for its listing.
    if front.menu.screen() == Some(menu::Screen::Store) {
        front.menu.ask_store_listing();
    }
    // P293's shots: `OPENMX_TRANSFERS` puts made-up downloads and uploads, or the reload, over it.
    front.menu.transfers_sample();
    // P298: the app was ended while it was away; where the player was is offered past the splash.
    if resume::ON && let Some((point, said)) = resume::load(&front.dir, BUILD_ID) {
        if let Some(said) = said {
            println!("resume: {said}");
        }
        let what = match &point {
            resume::Point::Screen(word) => menu::Screen::from_word(word).map(|s| s.name().to_string()),
            resume::Point::Ride(r) => Some(format!("the ride on {}", r.track)),
        };
        match what {
            Some(what) => {
                println!("resume: offering {what}");
                front.menu.offer.put(what);
                front.resume.offered = Some(point);
            }
            None => resume::clear(&front.dir),
        }
    }
    let mut shell = Shell { window: Some(window), seats: Some(seats), front };

    let frame_size = |window: &Screen| match args.size {
        Some(size) => size,
        None => args.aspect.frame((window.width, window.height), window.scale),
    };
    let (width, height) = frame_size(shell.window.as_ref().expect("the front end has a window"));
    let mut frame = Frame::new(width, height, [0, 0, 0]);
    // A pause asked for by `--screen` is over a ride, so it starts one on the last track; so is its
    // Tricks page (P259).
    let mut act = match shell.front.menu.screen() {
        Some(menu::Screen::Pause | menu::Screen::Tricks) => Act::Ride(shell.front.menu.settings.track.clone()),
        _ => Act::Stay,
    };
    let mut shot = args.shot.clone();
    let started = Instant::now();
    let mut shown = 0u32;
    // The menu last drawn and the frame size it was drawn at, while it is still on the screen.
    let mut shown_look: Option<(menu::Menu, usize, usize)> = None;
    // The menu as GPU sprites, and whether the window's renderer has the menu's pictures.
    let (mut menu_sprites, mut menu_images) = (Vec::new(), false);
    // The Garage's bike and rider (P213), dressed in the pick it shows.
    let mut garage = garage_view::Stage::default();
    // What the menu's frames cost before their sleep, for `--timings` (P47).
    let mut busy = Duration::ZERO;
    let mut limiter = pace::Limiter::default();
    let mut escape = false;
    // 1P's saved markers on the lobby's track, for this machine's pick (P99).
    let mut pick_markers = saved_markers::PickMarkers::default();
    loop {
        // The picture before this one is the page's until its next frame (P196).
        pace::shown().await;
        if matches!(act, Act::Ride(_) | Act::Online) {
            pick_markers.stale();
        }
        if let Act::Ride(track) = &act {
            shell.front.reader.reset();
            let mut picked = false;
            // P298: a resumed ride is on the pick it was ridden on, every restart of it too.
            let mut resumed = shell.front.resume.ride.take();
            loop {
                let mut a = args.clone();
                a.track = track.clone();
                if !args.mods_asked {
                    a.mods_asked = true;
                    a.mods = shell.front.menu.settings.mods.then(|| shell.front.dir.join(track_mods::MODS_DIR));
                }
                let overlay = args.overlay.unwrap_or(shell.front.menu.settings.overlay);
                a.overlay = Some(overlay);
                a.speedometer = Some(args.speedometer.unwrap_or(shell.front.menu.settings.speedometer));
                a.minimap = Some(args.minimap.unwrap_or(shell.front.menu.settings.minimap));
                a.show_fps = Some(args.show_fps.unwrap_or(shell.front.menu.settings.show_fps));
                a.world_sky = Some(args.world_sky.unwrap_or(shell.front.menu.settings.world_sky));
                a.hud_scale = Some(args.hud_scale.unwrap_or(shell.front.menu.settings.hud_scale));
                a.tricks = Some(args.tricks.unwrap_or(shell.front.menu.settings.tricks));
                a.trick_packs = Some(args.trick_packs.clone().unwrap_or_else(|| shell.front.menu.settings.trick_packs.clone()));
                a.handling = Some(args.handling.unwrap_or(shell.front.menu.settings.handling));
                a.handling_pack = Some(args.handling_pack.clone().unwrap_or_else(|| shell.front.menu.settings.handling_pack.clone()));
                a.brake_gyro = Some(args.brake_gyro.unwrap_or(shell.front.menu.settings.gyro.brake));
                a.gas_gyro = Some(args.gas_gyro.unwrap_or(shell.front.menu.settings.gyro.gas));
                a.fliers = Some(args.fliers.unwrap_or(shell.front.menu.settings.fliers));
                a.flier_skin = Some(args.flier_skin.unwrap_or(shell.front.menu.settings.flier_skin));
                a.graphics = Some(args.graphics.unwrap_or(shell.front.menu.settings.graphics));
                // The Original tab's own two (P126).
                a.plants = Some(args.plants.unwrap_or(shell.front.menu.settings.plants));
                a.foliage = Some(args.foliage.unwrap_or(shell.front.menu.settings.foliage));
                a.textures = Some(args.textures.unwrap_or(shell.front.menu.settings.textures));
                a.filtering = Some(args.filtering.unwrap_or(shell.front.menu.settings.filtering));
                a.shadows = Some(args.shadows.unwrap_or(shell.front.menu.settings.shadows));
                a.shadow_quality = Some(args.shadow_quality.unwrap_or(shell.front.menu.settings.shadow_quality));
                a.detail = Some(args.detail.unwrap_or(shell.front.menu.settings.detail));
                if !args.aspect_asked {
                    a.aspect = aspect_of(shell.front.menu.settings.picture);
                }
                if !args.render_scale_asked {
                    a.render_scale = shell.front.menu.settings.render_scale.fraction();
                }
                a.wrecks = args.wrecks && shell.front.menu.settings.wrecks;
                a.air_wrecks = args.air_wrecks && shell.front.menu.settings.air_wrecks;
                a.backwards_bail = args.backwards_bail || shell.front.menu.settings.backwards_bail;
                if args.modifiers.is_original() {
                    a.modifiers = shell.front.menu.settings.modifiers;
                }
                a.wiring = shell.front.wiring();
                // The shot is the menu's unless it is of the pause, which is drawn over the ride.
                a.shot = shot.take_if(|_| matches!(shell.front.menu.screen(), Some(menu::Screen::Pause | menu::Screen::Tricks)));
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
                if let Some(r) = resumed.as_mut() {
                    resume_pick(&mut a, r);
                    shell.front.resume.state = r.state.take();
                }
                shell.front.note_clear();
                // No still of a mod's map is shot beside the ride, loading or running (P282).
                let ride_on = track_previews::Ride::on();
                let left = ride(a, Some(&mut shell)).await;
                drop(ride_on);
                shell.front.menu.loading = None;
                match left {
                    Ok(Leave::Restart) => continue,
                    Ok(Leave::Menu) => break,
                    // A cancelled load is back on the pick (P127), which it has put on the screen.
                    Ok(Leave::Pick) => {
                        picked = true;
                        break;
                    }
                    Ok(Leave::Quit) => return Ok(()),
                    Err(e) => {
                        println!("ride: {e}");
                        shell.front.menu.note = Some(format!("Could not ride {track}: {e}"));
                        break;
                    }
                }
            }
            if !std::mem::take(&mut picked) {
                shell.front.menu.main();
            }
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
            if let Some((room, _)) = shell.front.menu.lobby.start.clone() {
                let mut a = args.clone();
                a.track = room.track.clone();
                // A track mod (MD6) from this machine's mods folder, whose files the handshake held
                // every seat to; a disc track's ride reads nothing from the folder.
                let mods = shell.front.menu.settings.mods.then(|| shell.front.dir.join(track_mods::MODS_DIR));
                (a.mods_asked, a.mods) = (true, mods.filter(|d| track_mods::find(d, &room.track).is_ok()));
                // And the room's tricks (MD19): the host's trick pack, which the handshake held every
                // seat to the same files of, read from this machine's mods folder.
                a.tricks = Some(if room.tricks.is_empty() { menu::Tricks::Original } else { menu::Tricks::MxVsAtv });
                a.room_tricks = (!room.tricks.is_empty()).then(|| (shell.front.dir.join(track_mods::MODS_DIR), room.tricks.clone()));
                // The room's handling (N21), which the handshake held every seat to, read from this
                // machine's mods folder.
                a.handling = Some(if room.handling.is_empty() { game_sim::handling::Handling::Original } else { game_sim::handling::Handling::MxVsAtv });
                a.room_handling = (!room.handling.is_empty()).then(|| (shell.front.dir.join(track_mods::MODS_DIR), room.handling.clone()));
                // MX Unleashed's fliers (MD80e): the room's handshake names no machine pack.
                a.fliers = Some(false);
                let overlay = args.overlay.unwrap_or(shell.front.menu.settings.overlay);
                a.overlay = Some(overlay);
                a.speedometer = Some(args.speedometer.unwrap_or(shell.front.menu.settings.speedometer));
                a.minimap = Some(args.minimap.unwrap_or(shell.front.menu.settings.minimap));
                a.show_fps = Some(args.show_fps.unwrap_or(shell.front.menu.settings.show_fps));
                a.world_sky = Some(args.world_sky.unwrap_or(shell.front.menu.settings.world_sky));
                a.hud_scale = Some(args.hud_scale.unwrap_or(shell.front.menu.settings.hud_scale));
                a.graphics = Some(args.graphics.unwrap_or(shell.front.menu.settings.graphics));
                // The Original tab's own two (P126).
                a.plants = Some(args.plants.unwrap_or(shell.front.menu.settings.plants));
                a.foliage = Some(args.foliage.unwrap_or(shell.front.menu.settings.foliage));
                a.textures = Some(args.textures.unwrap_or(shell.front.menu.settings.textures));
                a.filtering = Some(args.filtering.unwrap_or(shell.front.menu.settings.filtering));
                a.shadows = Some(args.shadows.unwrap_or(shell.front.menu.settings.shadows));
                a.shadow_quality = Some(args.shadow_quality.unwrap_or(shell.front.menu.settings.shadow_quality));
                a.detail = Some(args.detail.unwrap_or(shell.front.menu.settings.detail));
                if !args.aspect_asked {
                    a.aspect = aspect_of(shell.front.menu.settings.picture);
                }
                if !args.render_scale_asked {
                    a.render_scale = shell.front.menu.settings.render_scale.fraction();
                }
                a.wrecks = room.wrecks;
                a.air_wrecks = room.air_wrecks;
                a.backwards_bail = room.backwards_bail;
                // This machine's rider in the first seat's look (P86), as the room was told.
                a.look = shell.front.menu.picks.0[0].look.clone();
                a.wiring = shell.front.wiring();
                a.shot = None;
                a.seconds = args.seconds.map(|s| (s - started.elapsed().as_secs_f32()).max(0.0));
                shell.front.note_clear();
                // No still of a mod's map is shot beside the ride, loading or running (P282).
                let ride_on = track_previews::Ride::on();
                let left = ride(a, Some(&mut shell)).await;
                drop(ride_on);
                shell.front.menu.loading = None;
                match left {
                    Ok(Leave::Quit) => return Ok(()),
                    // A cancelled load leaves the lobby's ride for this machine (P127).
                    Ok(Leave::Pick) => shell.front.network.abandon(&mut shell.front.menu),
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
        // P298: the resume offer answered, the app gone away or come back.
        if let Some(yes) = shell.front.menu.offer.take() {
            resume::clear(&shell.front.dir);
            match shell.front.resume.offered.take().filter(|_| yes) {
                Some(resume::Point::Screen(word)) => {
                    println!("resume: back on {word}");
                    if let Some(screen) = menu::Screen::from_word(&word) {
                        shell.front.menu.open(screen);
                    }
                }
                Some(resume::Point::Ride(r)) => {
                    println!("resume: back on {}, {}", r.track, if r.state.is_some() { "where the ride was" } else { "from its start" });
                    act = Act::Ride(r.track.clone());
                    shell.front.resume.ride = Some(r);
                    (shown_look, menu_images) = (None, false);
                    garage.uploaded = false;
                    continue;
                }
                None => println!("resume: not now"),
            }
        }
        if let Some(asked) = resume::away() {
            let point = match &shell.front.resume.offered {
                // Ended again before the offer was answered: the offer stays.
                Some(offered) => Some(offered.clone()),
                None => shell.front.menu.screen().and_then(menu::Screen::resumed_as).map(|s| resume::Point::Screen(s.word().into())),
            };
            keep_resume_point(&shell.front.dir, point.as_ref());
            // What the menu draws with that is built again when it is next drawn, given back: the
            // Garage's models and the menu's pictures on the GPU.
            if let Some(w) = window.gpu() {
                w.renderer.forget_assets();
                (shown_look, menu_images) = (None, false);
                garage.uploaded = false;
            }
            resume::answered(asked);
            println!("resume: away on {:?}", shell.front.menu.screen());
        }
        if resume::back() && shell.front.resume.offered.is_none() {
            resume::clear(&shell.front.dir);
        }
        window.show_cursor(cursor::shown(cursor::Showing::Front, window.focused()));
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
        // inside one slow frame — 5 fps at 4K — is still a press. Each comes with its moments, which
        // time its presses and holds by when they were made rather than by this frame (P165).
        seats.keys_at(window.key_moments());
        // A pad pressed while the keyboard or the touch pad is 1P takes 1P over (P210), on the
        // screens no pad presses its own way into a seat on.
        seats.set_take_over(shell.front.menu.takes_over());
        let first = seats.first().cloned();
        let devices = seats.devices(&|code| window.pressed(code));
        let now = crate::window::shell_us();
        for line in seats.said() {
            println!("{line}");
        }
        took_over(seats, &mut shell.front.menu, first);
        let plugged: Vec<String> = devices
            .iter()
            .filter_map(|(d, _, _)| match d {
                Device::Pad(id) => Some(id.clone()),
                Device::Keys => None,
            })
            .collect();
        // The touch pad handed 1P to a controller, or took it back (P212, PadSlots::hand_over): the
        // menus follow, rather than lose 1P and bring the splash back.
        #[cfg(target_os = "ios")]
        if let (Some(Dev::Pad(was)), Some(Device::Pad(to))) = (shell.front.menu.p1().cloned(), seats.first().cloned())
            && was != to
        {
            let dev = Dev::Pad(to);
            let name = dev_name(seats, &dev);
            println!("1P: {name} — handed over from the touch pad, or back to it");
            shell.front.menu.p1_name = name;
            shell.front.menu.set_p1(dev);
        }
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
        let reports = timed_reports(seats, devices);
        // Past the splash 1P's pad and the keyboard drive the menu as one (P109): a push both see
        // is one press.
        let menu = &shell.front.menu;
        // The Controllers screen hears every device on its own (P210).
        let hosted = |d: &Dev| !matches!(menu.screen(), Some(menu::Screen::Splash | menu::Screen::Assign)) && menu.hosts(d);
        let mut presses = shell.front.readers.read_at(&reports, &hosted, now);
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
        shell.front.menu.keys_seat = (0..roster::SEATS).find(|&i| seats.on_keys(i));
        shell.front.menu.key_caps = seats.key_caps();
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
        give_controllers(seats, &mut shell.front.menu);
        shell.front.menu.elapse_to(now);
        for (i, reader) in shell.front.seat_readers.iter_mut().enumerate().skip(1) {
            let (report, taps, moments) = seat_dev(i)
                .and_then(|d| reports.iter().find(|(r, _, _, _)| *r == d))
                .map_or((Report::idle(), 0, None), |(_, r, taps, m)| (*r, *taps, m.as_deref()));
            let p = reader.read_timed(&[(report, taps, moments)], now);
            shell.front.menu.step_seat(i, p);
            while let Some(p) = reader.more() {
                shell.front.menu.step_seat(i, p);
            }
        }
        // A card dragged on the Controllers screen (P214), drawn over the whole frame here.
        let (fw, fh) = (frame.width, frame.height);
        drag_cards(&mut shell.front.menu, window.mouse(), (window.width, window.height, window.scale), (fw, fh), [0, 0, fw, fh]);
        let before = shell.front.menu.p1().cloned();
        shell.front.menu.music_now = shell.front.sound.now();
        act = shell.front.menu.step_devices(&presses);
        shell.front.cue(true);
        // Every press the frame held, a step each (P165): a slow frame's second tap, or the repeats
        // a hold made inside it.
        while act == Act::Stay
            && let Some(more) = shell.front.readers.more()
        {
            act = shell.front.menu.step_devices(&more);
            shell.front.cue(true);
        }
        take_controllers(seats, &mut shell.front.menu);
        let after = shell.front.menu.p1().cloned();
        if after != before {
            match &after {
                Some(dev) => {
                    let name = dev_name(seats, dev);
                    println!("1P: {name} — the menus and the first seat are its");
                    #[cfg(target_arch = "wasm32")]
                    crate::web_host::say(&format!("pads: 1P: {name} — the menus and the first seat are its"));
                    shell.front.menu.p1_name = name;
                    shell.front.menu.note = None;
                    let first = Some(match dev {
                        Dev::Keys => Device::Keys,
                        Dev::Pad(id) => Device::Pad(id.clone()),
                    });
                    // The Controllers screen already put it on 1P (P210): its seats stand.
                    if seats.first() != first.as_ref() {
                        seats.set_first(first);
                    }
                }
                None => seats.set_first(None),
            }
        }
        // A seat saved its look (P86): `riders.txt`, beside the settings.
        if shell.front.menu.take_looks_changed() {
            match shell.front.menu.looks.save(&shell.front.dir) {
                Ok(()) => println!("garage: looks saved in {}", shell.front.dir.join(roster::PRESETS_FILE).display()),
                Err(e) => {
                    println!("garage: {e}");
                    shell.front.menu.note = Some(format!("Could not save the look: {e}"));
                }
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
        // The store's asks and what it did; a removal reads the mods folder again, and so does a
        // batch of installs once it is in and the player says Reload (P293).
        let (rescan, art) = store.as_mut().map_or((false, false), |s| s.frame(&mut shell.front.menu, &mut shell.front.art));
        if art {
            (shown_look, menu_images) = (None, false);
        }
        if shell.front.menu.settings.mods != mods_on || rescan {
            mods_on = shell.front.menu.settings.mods;
            shell.front.menu.mods_reloaded();
            let listed = mod_tracks(&args, &shell.front.dir, mods_on);
            stills = mod_pictures(&mut shell.front.art, mods_folder(&args, &shell.front.dir, mods_on).as_deref(), &listed);
            shell.front.menu.set_mod_games(mod_games(&args, &shell.front.dir, mods_on, &listed));
            shell.front.menu.set_mod_categories(mod_categories(&args, &shell.front.dir, mods_on, &listed));
            shell.front.menu.set_mod_tracks(listed);
            shell.front.menu.set_music_packs(music_packs(&args, &shell.front.dir, mods_on));
            // and the trick packs (MD77): one the store installed has its row at once
            shell.front.menu.set_trick_packs(trick_pack_rows(&args, &shell.front.dir, &rider_stunts));
            // And their songs (MD7): a pack the store installed plays at once.
            if rescan {
                shell.front.sound.set_source(std::sync::Arc::new(read_music_packs(mods_folder(&args, &shell.front.dir, true).as_deref())));
                shell.front.menu.music_choices = shell.front.sound.choices();
            }
            (shown_look, menu_images) = (None, false);
        }
        // A still the job took is the card's picture from the next frame on.
        while let Some((id, got)) = stills.as_ref().and_then(|h| h.try_recv().ok()) {
            match got {
                Ok(picture) => {
                    println!("mods: took a still of {id} over the map");
                    shell.front.art.set_mod_picture(&id, picture);
                    (shown_look, menu_images) = (None, false);
                }
                Err(why) => println!("mods: no still of {id}: {why}"),
            }
        }
        if shell.front.menu.changed() {
            seats.set_wiring(shell.front.wiring());
            shell.front.hear();
            shell.front.save();
        }
        // 1P's saved markers on the room's track into this machine's lobby pick (P99), so every
        // machine starts the ride with them; the preset's line below sends a changed pick.
        let profile = saved_markers::profile(&roster::clean_rider_name(&shell.front.menu.picks.0[0].look.name), 0);
        // and a ride this machine drops in on brings them with its handshake (P105)
        shell.front.network.disc.bring_markers(&shell.front.dir, &profile);
        if let Some(track) = shell.front.menu.lobby.room.as_ref().map(|r| r.track.clone()) {
            let bike = shell.front.menu.lobby.pick.bike.clone();
            let text = pick_markers.of(&shell.front.dir, &track, &profile, &bike).to_string();
            shell.front.menu.lobby.set_markers(text);
        }
        // 1P's start machine into it too (N18), which every machine starts the seat on: the bike
        // (P177), or the plane under `--vehicle biplane`.
        let machine = args.vehicle.name();
        shell.front.menu.lobby.set_machine(machine);
        // 1P's own modifiers into it too, and the host's room every seat's (P134).
        shell.front.menu.lobby_mods();
        // 1P's controller preset into this machine's lobby pick (N13), for every machine's ride.
        shell.front.menu.lobby.preset(seats.preset(args.pad_seat(0)));
        if shell.front.network.pump(&mut shell.front.menu) == Act::Online {
            act = Act::Online;
        }
        if act == Act::Quit {
            break;
        }
        if matches!(act, Act::Ride(_) | Act::Online) {
            (shown_look, menu_images) = (None, false);
            // the ride's models take the GPU's place of the garage's
            garage.uploaded = false;
            continue;
        }
        // On the GPU the menu is sprites over its pictures, uploaded once (V30); the CPU draws it
        // for the X window, and for a shot.
        let shooting = shot.is_some() && shown >= args.shot_frame;
        // The Garage's pick, dressed again when it changes, and how far round its stand is (P213).
        let stage = garage_view::rect(&shell.front.menu, (frame.width, frame.height));
        match garage_view::shown_pick(&shell.front.menu) {
            Some(pick) => {
                // Off the menu's thread (P258): the stage shows the last pick it has until this one
                // is dressed, and dresses the row's neighbours ahead.
                garage.dress(pick);
                garage.ahead(&shell.front.menu.garage_neighbours());
                if shot.is_some() {
                    garage.settle();
                }
            }
            None => garage.leave(),
        }
        // A shot stands it where the stand starts, so two shots of two picks compare.
        let turn = garage_view::Stage::turn(if shot.is_some() { 0 } else { shell.front.menu.tick });
        if let Some(w) = window.gpu().filter(|_| !shooting) {
            if !menu_images {
                w.renderer.set_images(&shell.front.art.images());
                menu_images = true;
            }
            menu_sprites.clear();
            shell.front.art.sprites(&shell.front.menu, (frame.width, frame.height), 0, &mut menu_sprites);
            w.draw(frame.width, frame.height, |r, t| {
                r.draw_sprites(t, &menu_sprites);
                if let Some([x, y, sw, sh]) = stage {
                    garage.draw_gpu(r, t, [x as u32, y as u32, sw as u32, sh as u32], turn);
                }
            })?;
            shown_look = None;
        } else {
            // Drawn again only when it looks different (V30): at 4K a menu is most of a tenth of a
            // second to draw and send. The Garage's stand turns under it every frame. The GPU's path
            // never compares, so it never makes the copy (P254).
            let look = (shell.front.menu.look(), frame.width, frame.height);
            let fresh = shown_look.as_ref() != Some(&look);
            if fresh {
                frame.color.fill(0);
                shell.front.art.draw(&shell.front.menu, &mut frame);
                shown_look = Some(look);
            }
            let turned = stage.is_some_and(|rect| garage.draw_soft(&mut frame, rect, turn, fresh));
            if fresh || turned {
                window.present(&frame.color, frame.width, frame.height)?;
            } else {
                window.present_again(&frame.color, frame.width, frame.height)?;
            }
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
        let window = shell.window.as_mut().expect("the front end has a window");
        let floor = picture_floor(window, args.pacing(&shell.front.menu.settings));
        limiter.wait(args.draw_frame().or(menu_floor(window, floor)));
    }
    if args.timings {
        let ms = busy.as_secs_f64() * 1000.0 / shown.max(1) as f64;
        println!("front end: {shown} frames, {ms:.2} ms a frame before the pace");
    }
    Ok(())
}

/// What the splash calls a device: the keyboard, or a pad by its name without its node.
/// A take-over in this frame's read of the devices (P210): 1P's device changed from `first` without
/// the menus, so they are the new one's too.
fn took_over(seats: &Seats, menu: &mut Menu, first: Option<Device<String>>) {
    let now = seats.first().cloned();
    if now == first || first.is_none() {
        return;
    }
    let Some(dev) = now.map(|d| match d {
        Device::Keys => Dev::Keys,
        Device::Pad(id) => Dev::Pad(id),
    }) else {
        return;
    };
    if menu.p1() != Some(&dev) && menu.p1().is_some() {
        menu.p1_name = dev_name(seats, &dev);
        menu.set_p1(dev);
    }
}

/// The Controllers screen's view of the seats and of every device plugged in (P210), the keyboard
/// first.
fn give_controllers(seats: &Seats, menu: &mut Menu) {
    let assigned: Vec<Option<Dev>> = seats
        .assigned()
        .into_iter()
        .map(|d| {
            d.map(|d| match d {
                Device::Keys => Dev::Keys,
                Device::Pad(id) => Dev::Pad(id),
            })
        })
        .collect();
    let mut present = vec![(Dev::Keys, "Keyboard".to_string())];
    for id in seats.plugged() {
        let dev = Dev::Pad(id);
        present.push((dev.clone(), dev_name(seats, &dev)));
    }
    menu.controllers(&assigned, &present);
}

/// The mouse dragging the Controllers screen's cards (P214): `events` in window pixels, into `view`
/// (x, y, width, height), the part of the `frame` the screen is drawn in.
fn drag_cards(menu: &mut Menu, events: &[window::MouseEvent], window: (usize, usize, usize), frame: (usize, usize), view: [usize; 4]) {
    for &(what, x, y) in events {
        let what = match what {
            window::Mouse::Down => menu::Mouse::Down,
            window::Mouse::Move => menu::Mouse::Move,
            window::Mouse::Up => menu::Mouse::Up,
        };
        let [fx, fy] = cursor::onto_frame((x, y), (window.0, window.1), frame, window.2);
        menu.mouse(what, [fx - view[0] as f32, fy - view[1] as f32], (view[2], view[3]));
    }
}

/// The seats the Controllers screen moved, handed to the pads (P210).
fn take_controllers(seats: &mut Seats, menu: &mut Menu) {
    let Some(moved) = menu.take_assigned() else { return };
    let mut to: [Option<Device<String>>; pad::SEATS] = Default::default();
    for (i, d) in moved.into_iter().enumerate().take(pad::SEATS) {
        to[i] = d.map(|d| match d {
            Dev::Keys => Device::Keys,
            Dev::Pad(id) => Device::Pad(id),
        });
    }
    let said: Vec<String> = to
        .iter()
        .enumerate()
        .filter_map(|(i, d)| {
            let d = d.as_ref()?;
            let dev = match d {
                Device::Keys => Dev::Keys,
                Device::Pad(id) => Dev::Pad(id.clone()),
            };
            Some(format!("{}P {}", i + 1, dev_name(seats, &dev)))
        })
        .collect();
    println!("controllers: {}", if said.is_empty() { "no seats".to_string() } else { said.join(", ") });
    seats.assign(&to);
}

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
/// The settings' Picture Shape (P126) as the shell draws it.
fn aspect_of(picture: menu::Picture) -> Aspect {
    match picture {
        menu::Picture::Fill => Aspect::Fill,
        menu::Picture::Television => Aspect::Television,
        menu::Picture::Wide => Aspect::Wide,
        menu::Picture::Monitor => Aspect::Monitor,
        menu::Picture::Cinema => Aspect::Cinema,
        menu::Picture::Stretch => Aspect::Stretch,
    }
}

/// Models a slice uploads between two loading frames (P127).
const UPLOAD_SLICE: usize = 16;

/// The loading screen's own state across a ride's load (P127): its frame, its pace, and what it drew.
#[derive(Default)]
struct LoadScreen {
    frame: Option<Frame>,
    /// Whether the window's renderer has the menu's pictures.
    images: bool,
    sprites: Vec<game_render::gpu::Sprite>,
    /// The software frame's menu, while it is still what the window shows.
    shown: Option<Menu>,
    limiter: pace::Limiter,
    /// Escape held at the last frame, for its edge.
    escape: bool,
    frames: u32,
    since: Option<Instant>,
}

/// What a loading frame heard.
#[derive(Debug, PartialEq)]
enum Wait {
    On,
    /// Circle or Escape: back to the pick.
    Cancel,
    /// The window was closed.
    Quit,
}

/// The ride's build `build` on a thread of its own (P127), the loading screen drawn until it ends:
/// what it built, or what to leave for if the window was closed or the load cancelled, which stops
/// the thread before this returns. While the stopped build winds down the window draws the pick it
/// went back to and hears its close (P260): a load is never a frozen window, cancelled or not.
fn load_in_front<'env, T: Send + 'env>(
    shell: &mut Shell,
    args: &Args,
    progress: &loading::Progress,
    screen: &mut LoadScreen,
    build: impl FnOnce() -> Result<Result<T, Leave>, String> + Send + 'env,
) -> Result<Result<T, Leave>, String> {
    let Shell { window, seats, front } = shell;
    let mut stopped: Option<Instant> = None;
    // Never called in the browser, whose load runs before the loading screen would draw (`thread`).
    let wait = |stopping: Option<&mut Leave>| match stopping {
        None => pollster::block_on(load_tick(window, seats, Some(&mut *front), args, progress, screen)),
        Some(leave) => {
            stopped.get_or_insert_with(Instant::now);
            match leave {
                // A closed window has nothing to draw: its events are still pumped.
                Leave::Quit => {
                    if let Some(w) = window.as_mut() {
                        w.poll();
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                _ => {
                    if pollster::block_on(loading_frame(window, seats, front, args, progress, screen))? == Wait::Quit {
                        println!("loading: closed while the cancelled load ended");
                        *leave = Leave::Quit;
                    }
                }
            }
            Ok(None)
        }
    };
    match loading::on_worker_winding(progress, build, wait)? {
        Ok(built) => Ok(built),
        Err(leave) => {
            let took = stopped.map_or(0.0, |at| at.elapsed().as_secs_f32());
            println!("loading: stopped in {took:.2} s, the load's thread is gone ({} running)", loading::workers());
            Ok(Err(leave))
        }
    }
}

/// P298: `args`'s ride as a resume point, its first seat in `state` ([`resume::Ride::state`]).
fn resume_point(args: &Args, state: Option<Vec<u8>>) -> resume::Point {
    resume::Point::Ride(resume::Ride {
        track: args.track.clone(),
        bike: args.bike.clone(),
        set: args.set.unwrap_or(roster::Set::FREERIDE).word(),
        rider: args.rider.clone(),
        look: args.look.text(),
        state,
    })
}

/// P298: a resumed ride's pick on `args`, its first seat's: the bike, the set, the rider and the look
/// it was ridden on.
fn resume_pick(args: &mut Args, r: &resume::Ride) {
    if r.bike.is_empty() {
        return;
    }
    args.bike = r.bike.clone();
    args.rider = r.rider.clone();
    if let Some(set) = roster::Set::from_word(&r.set) {
        args.set = Some(set.ridden());
    }
    args.look = roster::Look::parse(&r.look);
}

/// P298: a ride's resume point: online, none — the room is gone by the next launch, and the online
/// screen is where the player goes back to it; a local ride of one player on the bike with its state;
/// any other (a split screen, a played tape, a seat on another machine) from the track's start.
fn ride_resume_point<P>(args: &Args, world: &World<P>, played: bool, lobby: bool) -> Option<resume::Point> {
    if args.online.is_some() || lobby {
        return Some(resume::Point::Screen(menu::Screen::Online.word().into()));
    }
    let alone = world.seats.len() == 1 && !played && world.seats[0].garage.up == Kind::Bike;
    Some(resume_point(args, alone.then(|| net::rides::seat_bytes(&world.seats[0].sim))))
}

/// P298: `point` written beside the settings in `dir` as where the player is, or none there.
fn keep_resume_point(dir: &std::path::Path, point: Option<&resume::Point>) {
    match point {
        Some(point) => match resume::save(dir, point, BUILD_ID) {
            Ok(()) => println!("resume: saved {}", match point {
                resume::Point::Screen(w) => w.clone(),
                resume::Point::Ride(r) => format!("the ride on {}{}", r.track, if r.state.is_some() { "" } else { ", from its start" }),
            }),
            Err(e) => println!("resume: {e}"),
        },
        None => resume::clear(dir),
    }
}

/// One loading frame where the front end has one, between two steps of a ride's load (P127): what
/// to leave the ride for, if anything. A cancel puts the pick back.
/// Wait while the app is away from the front (MD97: on the iPhone, the app switcher mid-load left
/// ground out for good, the GPU work made in the background refused): `what` says which upload waits.
fn wait_in_front(what: &str) {
    if gpuwin::in_front() {
        return;
    }
    let at = Instant::now();
    while !gpuwin::in_front() {
        std::thread::sleep(Duration::from_millis(50));
    }
    println!("loading: {what} waited {:.1} s for the app to be in front again", at.elapsed().as_secs_f32());
}

async fn load_tick(
    window: &mut Option<Screen>,
    seats: &mut Option<Seats>,
    front: Option<&mut Front>,
    args: &Args,
    progress: &loading::Progress,
    screen: &mut LoadScreen,
) -> Result<Option<Leave>, String> {
    let Some(front) = front else { return Ok(None) };
    if front.menu.loading.is_none() {
        front.menu.start_loading(&args.track);
    }
    match loading_frame(window, seats, front, args, progress, screen).await? {
        Wait::On => Ok(None),
        Wait::Cancel => {
            front.menu.back_to_pick();
            println!("loading: cancelled at {}, back on {:?}", progress.line(), front.menu.screen());
            Ok(Some(Leave::Pick))
        }
        Wait::Quit => {
            println!("loading: the window was closed at {}", progress.line());
            Ok(Some(Leave::Quit))
        }
    }
}

/// The menus' devices as [`Readers::read_at`] reads them (P165): each by its name for the menus,
/// with its moments since the last frame.
fn timed_reports(seats: &Seats, devices: Vec<(Device<String>, Report, u16)>) -> Vec<(Dev, Report, u16, Option<Vec<menu::Moment>>)> {
    devices
        .into_iter()
        .map(|(d, r, taps)| {
            let moments = seats.moments(&d);
            match d {
                Device::Keys => (Dev::Keys, r, taps, moments),
                Device::Pad(id) => (Dev::Pad(id), r, taps, moments),
            }
        })
        .collect()
}

/// The loading screen, drawn once, and the window's events and the menus' devices read: the
/// front end's frame (`front_end`'s loop) with the load's bar for its menu.
async fn loading_frame(
    window: &mut Option<Screen>,
    seats: &mut Option<Seats>,
    front: &mut Front,
    args: &Args,
    progress: &loading::Progress,
    screen: &mut LoadScreen,
) -> Result<Wait, String> {
    let Some(window) = window.as_mut() else { return Ok(Wait::On) };
    screen.since.get_or_insert_with(Instant::now);
    window.poll();
    if window.closed() {
        return Ok(Wait::Quit);
    }
    // P298: away while a ride loads: the ride it is loading, and the state it will be put in.
    if let Some(asked) = resume::away() {
        let point = (args.online.is_none() && front.menu.lobby.start.is_none()).then(|| resume_point(args, front.resume.state.clone()));
        keep_resume_point(&front.dir, point.as_ref());
        resume::answered(asked);
    }
    if resume::back() {
        resume::clear(&front.dir);
    }
    window.show_cursor(cursor::shown(cursor::Showing::Front, window.focused()));
    let (width, height) = match args.size {
        Some(size) => size,
        None => args.aspect.frame((window.width, window.height), window.scale),
    };
    if screen.frame.as_ref().is_none_or(|f| (f.width, f.height) != (width, height)) {
        screen.frame = Some(Frame::new(width, height, [0, 0, 0]));
        screen.shown = None;
    }
    // Back is Circle on the pads that drive the menus, or Escape on its edge.
    let esc = window.pressed(key::ESCAPE);
    let mut back = esc && !screen.escape;
    screen.escape = esc;
    if let Some(seats) = seats.as_mut() {
        seats.keys_at(window.key_moments());
        let devices = seats.devices(&|code| window.pressed(code));
        let now = crate::window::shell_us();
        let reports = timed_reports(seats, devices);
        let menu = &front.menu;
        let presses = front.readers.read_at(&reports, &|d: &Dev| menu.hosts(d), now);
        back |= presses.iter().any(|(_, p)| p.back);
        while let Some(more) = front.readers.more() {
            back |= more.iter().any(|(_, p)| p.back);
        }
    }
    back |= loading::hook_fires(loading::CANCEL, screen.frames, progress);
    if loading::hook_fires(loading::CLOSE, screen.frames, progress) {
        println!("loading: closed at {} ({})", progress.line(), loading::CLOSE);
        return Ok(Wait::Quit);
    }
    if let Some(l) = &mut front.menu.loading {
        l.done = progress.done();
        l.stage = progress.line();
    }
    front.menu.elapse_to(crate::window::shell_us());
    let ticks = front.menu.take_ticks();
    front.menu.tick = front.menu.tick.wrapping_add(ticks);
    let frame = screen.frame.as_mut().expect("made above");
    if let Some(w) = window.gpu() {
        if !screen.images {
            w.renderer.set_images(&front.art.images());
            screen.images = true;
        }
        screen.sprites.clear();
        front.art.sprites(&front.menu, (frame.width, frame.height), 0, &mut screen.sprites);
        let sprites = &screen.sprites;
        w.draw(frame.width, frame.height, |r, t| r.draw_sprites(t, sprites))?;
    } else {
        let look = front.menu.look();
        if screen.shown.as_ref() != Some(&look) {
            frame.color.fill(0);
            front.art.draw(&front.menu, frame);
            window.present(&frame.color, frame.width, frame.height)?;
            screen.shown = Some(look);
        } else {
            window.present_again(&frame.color, frame.width, frame.height)?;
        }
    }
    screen.frames += 1;
    let floor = picture_floor(window, args.pacing(&front.menu.settings));
    screen.limiter.wait(args.draw_frame().or(menu_floor(window, floor)).or(Some(Duration::from_millis(16))));
    pace::shown().await;
    Ok(if back { Wait::Cancel } else { Wait::On })
}

async fn ride(mut args: Args, shell: Option<&mut Shell>) -> Result<Leave, String> {
    // In the browser a WebGPU validation error from here on ends the ride, not the front end (P257).
    #[cfg(target_arch = "wasm32")]
    gpuwin::riding();
    // The world's replaced textures (V84), set before anything of the ride is decoded and kept while
    // it rides: a world's tiles stream in later and are replaced as they load.
    let mut texture_reload = install_textures(&args);
    // ---- the game modifiers (P55) ----------------------------------------------------------------
    // Applied to what the ride loads, never as a branch in the simulation: at their defaults nothing
    // is touched at all.
    let lobby_room = shell.as_deref().and_then(|s| s.front.menu.lobby.start.as_ref()).map(|(room, _)| room.clone());
    // A ride from the lobby draws every seat's rider in their own look (N16): the room's picks are
    // the shell's other seats, this machine's first, each loaded as a split screen's other players
    // are, with no view of their own. A drop-in's seats come as the host takes it on, and are
    // dressed then.
    let mut lobby_riders: Option<usize> = None;
    let mut peer_dress = peer_looks::Dressed::default();
    if let Some(((room, me), false)) = shell.as_deref().and_then(|s| s.front.menu.lobby.start.as_ref().map(|start| (start, s.front.menu.lobby.drop_in)))
        && let Some(others) = peer_looks::lobby_others(room, me, &own_pick(&args))
    {
        for (d, pick) in others.iter().enumerate() {
            println!("online: seat {} rides {} on the {} as {:?}", d + 2, pick.rider, pick.bike, pick.look.text());
        }
        lobby_riders = Some(others.len() + 1);
        args.others = others;
        peer_dress = peer_looks::Dressed::lobby(room, me);
    }
    // Everything up to the window is built in `build` (P127): under the front end on a thread of its
    // own while the window draws the loading screen and keeps answering, here without one. The
    // drawn ground's chunks are built in it too when the window's GPU will want them.
    // The terrain's bytes and the simulation's parse of them are kept out here, where they outlive
    // the build: the simulation's ground points into them.
    let progress = &loading::Progress::new();
    let (keep_bytes, keep_terrain) = (&std::sync::OnceLock::new(), &std::sync::OnceLock::new());
    let mut shell = shell;
    let want_chunks = shell.as_deref_mut().and_then(|s| s.window.as_mut()).is_some_and(|w| w.gpu().is_some());
    let build = {
        let args = args.clone();
        move || -> Result<Result<_, Leave>, String> {
    progress.reach(loading::Stage::Scene)?;
    let mods = ride_modifiers(&args, lobby_room.as_ref())?;
    if !mods.is_original() {
        println!("modifiers: {}", mods.text());
    }

    // ---- the disc -------------------------------------------------------------------------------
    let mut disc = Disc::open_configured()?;
    // A track the disc does not have is a track mod's (MD2), looked for in the mods folder only
    // then, so a disc track's ride reads nothing from it.
    // MX Unleashed's worlds (P232, `disc_worlds`) are the disc's own tracks as one ground: the ride is
    // the home map's, dressed and lit from its archives, and the simulation asks every map and the
    // stitcher's joins. An installed world mod of the same id is ridden instead.
    let disc_world =
        disc_worlds::is_disc_world(&args.track) && !mods_dir(&args).is_some_and(|d| worlds::is_world(&d, &args.track));
    let disc_home = if disc_world { disc_worlds::home_track(&args.track) } else { None };
    let home = if disc_world { disc_home.as_deref().unwrap_or(track_mods::FREERIDE) } else { args.track.as_str() };
    let track_member = member_ending(&disc, &format!("{home}.res"));
    // A world mod (MD8) is several track mods as one ground: the ride is dressed and lit from its
    // home tile's folder, as a track mod's is, and the simulation asks every tile.
    let mut world_mod: Option<worlds::WorldMod> = None;
    let modded = match &track_member {
        Ok(_) if disc_world => {
            // Its maps read, its joins built the first time and the world read, each said under
            // the bar and each answering a cancel (P260).
            let watch = |what: &str, n, of| progress.doing(what, n, of);
            world_mod = Some(disc_worlds::world_watched(&mut disc, &disc_worlds_cache(&args), &args.track, &mut |l| println!("{l}"), &watch)?);
            None
        }
        Ok(_) => None,
        Err(e) => match mods_dir(&args) {
            Some(dir) if worlds::is_world(&dir, &args.track) => {
                let w = worlds::find_watched(&dir, &args.track, &|what, n, of| progress.doing(what, n, of))
                    .map_err(|m| format!("{e}, and {m}"))?;
                let home = w.home_track();
                world_mod = Some(w);
                Some(home)
            }
            Some(dir) => Some(track_mods::find(&dir, &args.track).map_err(|m| format!("{e}, and {m}"))?),
            None => return Err(e.clone()),
        },
    };
    // The name the track's other files are built from: a disc track's own, a mod's scene's stem.
    let stem = match &modded {
        Some(t) => t.stem.clone(),
        None => home.to_string(),
    };
    let bike_member = member_ending(&disc, &format!("{}.res", args.bike))?;
    // A track is two archives: the props and the terrain in one, the bike's animation set, its
    // physics and the follow camera in the other, which the game loads alongside it. A mod has no
    // second archive and rides with a disc freeride's.
    let second_of = if modded.is_some() { track_mods::FREERIDE } else { stem.as_str() };
    let second = member_ending(&disc, &format!("{second_of}_2.res")).ok();
    // The rider is a fourth archive, named after the rider rather than the track or the bike: the
    // body, its gear and its motions are all in there.
    let rider_member = match args.rider.as_str() {
        "none" => None,
        name => Some(member_ending(&disc, &format!("{name}.res"))?),
    };
    let track_res = match (&modded, &track_member) {
        (Some(t), _) => {
            mods_line(format!("mods: {} ({}) from {}", t.id, t.name, t.dir.display()));
            t.open()?
        }
        (None, Ok(member)) => disc.res(member)?,
        (None, Err(e)) => return Err(e.clone()),
    };
    // Two views on a disc freeride ride its split-screen twin's `.toy` (V105), as the original loads
    // the whole `<track>_SS` there: free09_ss's windmills turn out of view, free11_ss's trucks and
    // police car run at their own speeds. Only the `.toy` is the twin's; the rest is still the
    // track's own. The views at the load decide it: a seat joining mid-ride (P76) keeps the file.
    // Online every machine reads the one file, whatever its views, as the cars are the session's.
    let split = Layout::for_players(args.players, args.split).is_ok_and(|l| l != Layout::Full)
        && args.online.is_none()
        && lobby_room.is_none();
    let track_res = match split && modded.is_none() && !disc_world {
        true => split_screen_toy(&mut disc, track_res, &stem)?,
        false => track_res,
    };
    let mut archives: Vec<Res> = vec![track_res, disc.res(&bike_member)?];
    if let Some(name) = &second {
        archives.push(disc.res(name)?);
    }
    if let Some(name) = &rider_member {
        archives.push(disc.res(name)?);
    }
    // The garage's looks (P86): a ride where someone wears more than their gear set or has a name or
    // number opens the wardrobe — the pieces' paint last, behind every archive above — and the fonts.
    let dressed = args.look != roster::Look::default() || args.others.iter().any(|p| p.look != roster::Look::default());
    let mut kit = dressed.then(|| dress::Kit::open(&mut disc));
    if let Some(member) = kit.as_ref().and_then(|k| k.custom.as_ref()) {
        archives.push(disc.res(member)?);
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
    // MX vs. ATV's tricks (MD16), from the trick packs on in the mods folder (MD77): their extra
    // stunts stacked after the 24, and below the first's moves laid over every bike. Without a pack
    // the ride has the original's.
    let trick_packs = ride_trick_packs(&args)?;
    let stunts = match trick_packs.is_empty() {
        false => {
            let (stunts, over) = trick_packs::stunts(&trick_packs, stunts);
            let extras = stunts.stunts.len() - game_sim::trick::SLOTS;
            match over {
                0 => println!("tricks: {extras} extra stunts, each on a gesture"),
                n => println!("tricks: {extras} extra stunts on the {} gestures, {n} more without a button", game_sim::trick::EXTRA_SLOTS),
            }
            stunts
        }
        true => stunts,
    };
    // Their clips (MD21), bound below onto every body and chassis the ride loads.
    let pack_clips = match trick_packs::clips(&trick_packs) {
        Ok(clips) => clips,
        Err(e) => {
            println!("tricks: no clips: {e}");
            None
        }
    };

    let scn = archives[0]
        .entries()
        .iter()
        .find(|e| e.name.to_ascii_lowercase().ends_with(".scn"))
        .ok_or("the track archive has no .scn")?
        .name
        .clone();
    // A world's home scene is the one its find read (MD18), not read again.
    let scn_bytes = match world_mod.as_ref().and_then(|w| w.pieces[w.home].scene_bytes.clone()) {
        Some(kept) => kept.to_vec(),
        None => archives[0].read(&scn)?,
    };
    // The same file read twice, by two readers that want different things out of it: the simulation's
    // reader for the spawn and the terrain's layout, the renderer's for the props, lights and fog.
    // A mod's scene is first read here (MD18), so what is wrong with it is said here.
    let mut place = track::Scene::parse(&scn_bytes).map_err(|e| format!("{scn}: {e}"))?;
    if let Some(t) = &modded {
        t.place_spawn(&mut place)?;
    }
    if let Some(w) = &world_mod {
        w.place_spawn(&mut place)?;
        if let worlds::Spawn::Tile(i) = w.spawn {
            println!("world spawn: {} at {:.1},{:.1}", w.pieces[i].tile.name, place.start[0].0, place.start[2].0);
        }
    }
    let mut look = view::Scene::parse(&ini::text(&scn_bytes));
    if let Some(w) = &world_mod {
        let cut = worlds::cut_look(&mut look, &w.pieces[w.home].tile);
        if cut > 0 {
            println!("world: {cut} of {}'s models stand outside its keep, not drawn", w.pieces[w.home].tile.name);
        }
        worlds::move_look(&mut look, w.home_at());
    }

    progress.reach(loading::Stage::Tiles)?;
    let read = terrain_bytes(&archives[0], &place.terrain_file)?;
    let terrain_bytes: &Vec<u8> = keep_bytes.get_or_init(|| read);
    // The same file parsed twice, because the two sides of it are owned by different crates: the
    // shell's `Ground` draws it in host floats, the port's answers the ride's ray in the game's.
    let ground = Ground::new(track::Terrain::parse(&terrain_bytes).map_err(|e| format!("{}: {e}", place.terrain_file))?, &place.terrain);
    let ground = {
        let extent = ground.extent();
        ground.painted(game_render::splat::Splat::load(&archives, &stem, extent))
    };
    // A mod track with no splat is painted from its terrain's own pages, as MX vs. ATV draws its maps
    // (MD4). The disc's tracks keep their own ground.
    let ground = match &modded {
        Some(_) if ground.paint.is_none() => {
            let detail = game_render::terrain::Pages::load_detail(&archives[..1], &stem);
            let ground = ground.paged(detail, game_render::terrain::detail_multiplier(&ini::text(&scn_bytes)));
            if let Some(p) = &ground.pages {
                let paged = p.page_of.iter().filter(|p| p.is_some()).count();
                let detail = if p.detail.is_some() { "with" } else { "without" };
                println!("ground: {} terrain pages on {paged} of {} leaves, {detail} a detail map", p.pages.len(), p.page_of.len());
            }
            ground
        }
        _ => ground,
    };
    let sim_terrain = track::Terrain::parse(terrain_bytes)?;
    let sim_terrain: &track::Terrain = keep_terrain.get_or_init(|| sim_terrain);
    // A world's ground (MD8): every tile and join, each asked in its own coordinates.
    let mut tiled = match &world_mod {
        Some(w) => {
            let t = w.tiled(args.world_reach.unwrap_or(game_sim::tiles::Reach::DESKTOP))?;
            println!("{}", w.line(&t));
            if let Some(line) = w.wrap_line(&t) {
                println!("{line}");
            }
            println!("{}", worlds::WorldMod::holes_line(&t));
            let mut t = t;
            if args.seams {
                println!("{}", worlds::WorldMod::seams_line(&mut t));
            }
            Some(Box::new(t))
        }
        None => None,
    };
    let world_area = tiled.as_ref().map(|t| t.area);
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

    progress.reach(loading::Stage::Props)?;
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
    // The trees the scene's ecosystem stands, each trunk a solid under its plant (S97, V50): what
    // stops the rider at a free09 tree. A world's tiles bring theirs to the picture only (MD15).
    let trees = match look.ecosystem_file.as_deref() {
        Some(file) if world_mod.is_none() => {
            let ground = sim_world::Ground::new(sim_terrain, &place.terrain);
            let curve = game_sim::camera::CurveTable::build();
            match archives.iter().find_map(|a| a.read_suffix(file).ok()) {
                Some(esb) => match game_sim::eco::Trees::on_ground(&esb, &ground, &curve) {
                    Ok((trees, missed)) => {
                        println!("trees: {} trunks under {} plants", trees.props.props.len(), trees.groups().len());
                        for line in missed.iter().take(3) {
                            println!("  {line}");
                        }
                        Some(trees)
                    }
                    Err(e) => {
                        println!("trees: {file}: {e}");
                        None
                    }
                },
                None => {
                    println!("trees: no {file} in the track's archives");
                    None
                }
            }
        }
        _ => None,
    };

    // ---- what to draw -------------------------------------------------------------------------
    progress.reach(loading::Stage::Models)?;
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

    // A world's tiles dress themselves (MD15), the home tile's props among them, each under its own
    // name: the home scene's are not loaded twice.
    if world_mod.is_some() {
        look.models.clear();
    }
    let mut names: Vec<String> = look.models.iter().map(|p| p.model.clone()).collect();
    names.sort();
    names.dedup();
    for name in &names {
        assets.load_placed(&archives, name);
    }
    // The first rider's name and number on their bike's plates and, below, their jersey (P86).
    if let Some(k) = &mut kit {
        assets.make_texture(dress::NAME_TEXTURE, k.texture(&mut disc, &args.rider, &args.look, &args.bike));
    }
    // The trees and bushes the scene's ecosystem scatters over the ground (V48).
    match game_render::eco::Grove::load(&archives, &look, &place.terrain, &sim_terrain, &mut assets) {
        // A world's are its tiles', each stood on its own tile (MD15, `world_dress`).
        Ok(_) if world_mod.is_some() => {}
        Ok(grove) => look.ecosystem = grove,
        Err(e) => println!("ecosystem: {e}"),
    }
    if let Some(grove) = &mut look.ecosystem {
        grove.lod = args.plants.unwrap_or_default();
        println!("plants: {}", grove.lod.word());
        grove.foliage = args.foliage.unwrap_or_default();
        println!("foliage: {}", grove.foliage.word());
    }
    // Kept as `CHASSIS` whichever file the bike's set draws it from: `bike50` under a fast50 (V37).
    let chassis_file = game_render::vehicle::chassis_model(&archives[1..2], &vehicle);
    // The bike is MX Unleashed's whatever the map (MD67): its own archives first, the track's last.
    assets.own(1);
    let has_bike = assets.load_as(&archives, &chassis_file, CHASSIS);
    for part in &vehicle.parts {
        assets.load(&archives, &part.model);
    }
    assets.own(0);
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
    // MX vs. ATV's handling (MD40): its own bike file from the trick pack, in place of the disc's.
    let handling_pack = ride_handling_pack(&args, &trick_packs)?;
    let mut stock_phy = handling_phy(handling_pack.as_ref(), &phy_member, &tables.ride.curve, "")
        .unwrap_or_else(|| Phy::read_bytes(&phy_bytes, &tables.ride.curve));
    untamed_handling(&args, &mut stock_phy);
    gyro_options(&args, &mut stock_phy);
    if !trick_packs.is_empty() {
        println!("{}", trick_packs::lay(&trick_packs, &mut stock_phy, &phy_member));
    }
    let mut phy = stock_phy.clone();
    mods.bike(0, &mut phy);
    // The other players' own picks (P37): each rides the `.phy` its set names in its own bike's
    // archive, and wears its own rider's gear. A player the garage has no pick for rides the first's.
    let mut other_phys: Vec<Phy> = Vec::new();
    // Each player's file as the disc has it, which a change of modifiers in the ride builds the
    // bike's numbers again from (P98).
    let mut stock_phys: Vec<Phy> = vec![stock_phy.clone()];
    // Each player's bike as their saved markers name it (P99): its `.phy`'s name.
    let mut bike_words: Vec<String> = vec![saved_markers::bike_word(&phy_member)];
    let mut other_riders: Vec<Option<(String, String)>> = Vec::new();
    // Online every seat of `--seats` is a rider on this track, each machine's in the same places
    // (N5): seat `s` starts where local player `s + 1` would, on the first's bike.
    let riders = match &args.online {
        Some(_) => online::Online::seats(args.seats.as_deref())?.max(args.players),
        None => lobby_riders.unwrap_or(args.players).max(args.players),
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
                let mut phy = handling_phy(handling_pack.as_ref(), &member, &tables.ride.curve, &format!("player {}: ", i + 1))
                    .unwrap_or_else(|| Phy::read_bytes(&bytes, &tables.ride.curve));
                untamed_handling(&args, &mut phy);
                gyro_options(&args, &mut phy);
                if !trick_packs.is_empty() {
                    println!("player {}: {}", i + 1, trick_packs::lay(&trick_packs, &mut phy, &member));
                }
                stock_phys.push(phy.clone());
                bike_words.push(saved_markers::bike_word(&member));
                mods.bike(i, &mut phy);
                other_phys.push(phy);
                let rider = member_ending(&disc, &format!("{}.res", pick.rider)).ok();
                other_riders.push(rider.map(|m| (pick.rider.clone(), m)));
            }
            None => {
                let mut phy = stock_phy.clone();
                stock_phys.push(stock_phy.clone());
                bike_words.push(bike_words[0].clone());
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
    // What the pack's clips left unbound on the first body and chassis (MD21), for the line below.
    let mut pack_missing: (Option<Vec<String>>, Option<Vec<String>>) = (None, None);
    let mut body = match &rider_member {
        None => None,
        Some(member) => {
            let own = vec![disc.res(member)?];
            match Body::load(own, &args.rider, Sex::Male, "", &table.stunts, phy.motion()) {
                Ok(mut body) => {
                    pack_missing.0 = pack_clips.as_ref().map(|p| body.add_pack(p, &stunts.stunts));
                    Some(body)
                }
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
        if let Some(k) = &kit {
            assets.rename_materials(&k.repaint(&body.rider.suit, &args.look));
        }
        assets.own(1);
        let drawn = assets.load(&archives, body.geometry());
        assets.own(0);
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
            let col = archives.iter().find_map(|a| a.read_suffix("wreckbikef.col").ok());
            kit.bike = col.as_deref().and_then(game_sim::ragdoll_ride::bike_sphere);
            // The same file's mesh, which the bones' spheres hit (S105).
            kit.bike_mesh = col.as_deref().and_then(game_sim::ragdoll_scene::Bike::parse).map(Arc::new);
            Some(Arc::new(kit))
        }
        _ => None,
    };
    match &ragdolled {
        Some(doll) => println!("ragdoll: {} parts off the riding pose", doll.nodes.len()),
        None => println!("ragdoll: none — a wreck is ridden out as one point"),
    }
    // The bike's two collision files (S78): its body rays, and the frame other riders' rays strike.
    // Looked for by their whole path, which `wreckbikef.col` does not end in.
    let bike_col = archives
        .iter()
        .find_map(|a| {
            use game_sim::bike_col::{MODEL, rays_file, shape_file};
            Some((a.read_suffix(&rays_file(MODEL)).ok()?, a.read_suffix(&shape_file(MODEL)).ok()?))
        })
        .and_then(|(rays, shape)| game_sim::bike_col::BikeCol::load(&rays, &shape).ok())
        .map(Arc::new);
    match &bike_col {
        Some(_) => println!("rider contact: the bikes push each other apart"),
        None => println!("rider contact: no bike collision files — the riders pass through each other"),
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
                Ok(mut chassis) => {
                    pack_missing.1 = pack_clips.as_ref().map(|p| chassis.add_pack(p, &stunts.stunts));
                    Some(chassis)
                }
                Err(e) => {
                    println!("chassis: {e}");
                    None
                }
            }
        }
    };
    if pack_clips.is_some() {
        println!("{}", trick_packs::missing_clips(&trick_packs, &stunts, pack_missing.0.as_deref(), pack_missing.1.as_deref()));
    }
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
        // Which tricks the bike stays still under (MD41): the drawn rider holds the bars instead.
        let bare = chassis.bare_slots(&stunts.stunts);
        if !bare.is_empty() {
            println!(
                "chassis: no bike clip on the disc{} for {}; the rider's hands hold the bars through them",
                if pack_clips.is_some() { " or in the pack" } else { "" },
                bare.join(", ")
            );
        }
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
    // The eye's ceiling, which the race's setup sets off the scene's fog (S103).
    camera.cam.ceiling = Race::ceiling_for_fog(F(look.fog_far));
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
    // and physics. A track with no such section parks no plane, but still has one (P113): it spawns
    // on the player, and starts by the start line.
    let machine_res = match member_ending(&disc, MACHINE_ARCHIVE) {
        Ok(name) => Some(disc.res(&name)?),
        Err(_) => None,
    };
    let machines = machine_res.as_ref().and_then(|res| {
        let e = res.find_suffix(&format!("{stem}\\machine.txt"))?;
        Some((e.name.clone(), res.read(&e.name).ok()?))
    });
    // And its flight (P31): the plane's own `biplane.phy`, from the same archive, built the way the
    // original builds it. Without the file the plane is parked and stays parked. The flyable area is
    // the terrain's extent, which only the map-edge throw reads; the pairing of the layout's four
    // fields with the area's is S37's reading, not checked against this track.
    // MX vs. ATV's fliers (MD80e), where the Fliers setting is on and the pack is there: the plane's
    // file, collision pair and model off the pack, in place of the disc's, and its flight on
    // SLUS-21104's steps; `plane_mxvatv` is whether it is model one. Off, nothing below changes.
    let fliers = ride_fliers(&args);
    let plane_mxvatv = fliers.as_ref().filter(|f| f.plane_phy().is_some()).map(|f| f.model_one());
    let plane_phy = match fliers.as_ref().and_then(|f| f.plane_phy()) {
        Some(bytes) => Some(bytes),
        None => machine_res.as_ref().and_then(|res| {
            let e = res.find_suffix(phy::BIPLANE_PHY)?;
            res.read(&e.name).ok()
        }),
    };
    // The plane's stock file the modifiers go on: MX vs. ATV's read for the skin's model.
    let read_plane = move |b: &[u8]| match plane_mxvatv {
        Some(one) => phy::Plane::read_mxvatv_bytes(b, one).plane,
        None => phy::Plane::read_bytes(b),
    };
    // And its collision object's ray probe (S48), which is what it strikes the ground and the
    // track's meshes with; without it the plane meets the world only with its wheels.
    let plane_rays = machine_res.as_ref().and_then(|res| {
        let bytes = match fliers.as_ref().and_then(|f| f.rays(Machine::Biplane)) {
            Some(bytes) => bytes,
            None => res.read(&res.find_suffix(game_sim::flight_search::BIPLANE_RAYS)?.name).ok()?,
        };
        match game_sim::track::Rays::parse(&bytes) {
            Ok(r) => Some(game_sim::flight_search::Rays::of(&r)),
            Err(e) => {
                eprintln!("biplane: {}: {e}", game_sim::flight_search::BIPLANE_RAYS);
                None
            }
        }
    });
    // And the bikes' collision box (S95), which the other riders' bikes are to the plane's search.
    let bike_box = archives.iter().find_map(|a| a.read(game_sim::flight_search::BIKE_COLLISION).ok()).and_then(|bytes| {
        game_sim::track::Mesh::parse(&bytes)
            .map_err(|e| eprintln!("biplane: {}: {e}", game_sim::flight_search::BIKE_COLLISION))
            .ok()
    });
    // And the plane's own hull (N22a), which the riders' bikes strike while a player flies it.
    let plane_hull = machine_res.as_ref().and_then(|res| {
        let bytes = match fliers.as_ref().and_then(|f| f.hull(Machine::Biplane)) {
            Some(bytes) => bytes,
            None => res.read(&res.find_suffix(game_sim::flight::HULL)?.name).ok()?,
        };
        game_sim::track::Mesh::parse(&bytes).map_err(|e| eprintln!("biplane: {}: {e}", game_sim::flight::HULL)).ok()
    });
    // And the wreck bike's hull (F35), which the plane's rays strike on a downed rider's wreck bike.
    let wreck_hull = archives.iter().find_map(|a| a.read_suffix("wreckbikef.col").ok()).and_then(|b| game_sim::ragdoll_scene::Bike::parse(&b)).map(|b| b.mesh);
    // One of each machine per player (P87): the file's two spots for the first two, beside them for
    // the rest, every second copy in the alternate livery. Only the plane is ported, so the lot's
    // other machines are counted and not stood up.
    // In a world (MD24) the file is the home tile's, in that track's own feet: its spots are moved
    // as one to stand the first plane beside the world's spawn.
    let line_start = {
        let (s, f) = (point(place.start), point(place.start_direction).norm());
        vehicle::Spawn { at: Vf::new(s.x, s.y, s.z, 1.0), facing: Vf::new(f.x, f.y, f.z, 0.0) }
    };
    // A home tile's spot its own ground does not reach stands the plane on its wheels (MD55).
    let spot_rest = plane_phy.as_deref().map_or(0.0, |b| {
        let mut plane = read_plane(b);
        mods.plane(0, &mut plane);
        vehicle::wheels_height(&Flight::new(&plane))
    });
    // A car parks only by its own track's file (S245), a world's by its home tile's.
    let lot_spots = |name: &str, b: &[u8], ground: &mut dyn FnMut(Vf) -> f32| {
        vehicle::parked_by(
            name,
            match &world_mod {
                Some(w) => vehicle::world_spots(b, w.home_at().map(|a| a.0), line_start, spot_rest, ground),
                None => Machine::ALL.iter().map(|&m| vehicle::spots(b, m)).collect(),
            },
        )
    };
    let lot = {
        let mut probe = sim_world::Ground::new(&sim_terrain, &place.terrain);
        let mut ground = |at: Vf| lot_ground(tiled.as_deref_mut(), &mut probe, at);
        machines.as_ref().map_or_else(Lot::default, |(n, b)| Lot::around(lot_spots(n, b, &mut ground), args.players.max(1), &mut ground))
    };
    // And how high the original stands the plane's body over a file bay's ground (S164): the
    // model's bounds, off its own `.psg`.
    let mut lot = lot;
    lot.lift = machine_res.as_ref().and_then(|res| {
        let bytes = match fliers.as_ref().and_then(|f| f.psg(Machine::Biplane)) {
            Some(bytes) => bytes,
            None => res.read(&res.find_suffix(game_sim::flight::BIPLANE_MODEL)?.name).ok()?,
        };
        game_sim::flight::spawn_lift(&bytes).map_err(|e| eprintln!("biplane: {}: {e}", game_sim::flight::BIPLANE_MODEL)).ok()
    });
    // And whether each file bay is put on the ground there (S205).
    lot.ground_heights = machines.as_ref().map_or_else(Vec::new, |(_, b)| vehicle::ground_heights(b, Machine::Biplane));
    // And the cars' files (S149e): each car's `.phy` and its two liveries' models, which every
    // player's garage builds its cars from ([`vehicle::cars_of`]).
    let car_files: Vec<(String, Vec<u8>)> = machine_res.as_ref().map_or_else(Vec::new, |res| {
        let mut names = Vec::new();
        for m in [Machine::Buggy, Machine::Monster, Machine::Baja] {
            names.extend(m.car_physics().map(|k| k.phy().to_string()));
            names.extend(m.models()[..vehicle::LIVERIES].iter().map(|n| format!("{}\\anim\\{n}.psg", m.dir())));
        }
        names.into_iter().filter_map(|n| Some((n.clone(), res.read(&res.find_suffix(&n)?.name).ok()?))).collect()
    });
    lot.cars = vehicle::car_kit(&mut |name: &str| car_files.iter().find(|(n, _)| n == name).map(|(_, b)| b.clone()));
    // And each car's bay on the ground under it (S245): free07's file parks its bajas at 0.
    {
        let mut probe = sim_world::Ground::new(&sim_terrain, &place.terrain);
        lot.stand_cars(&mut |from: Vf| lot_ground_normal(tiled.as_deref_mut(), &mut probe, from));
    }
    // And the helicopter's (P216): `kiowa.phy`, which every player's garage flies a copy of.
    lot.kiowa = machine_res.as_ref().and_then(|res| {
        // MX vs. ATV's (MD80e): the pack's file, collision pair and model, where the Fliers setting
        // has it; the thrown blades' own files stay the disc's, which the pack has not.
        let pack = fliers.as_ref().filter(|f| f.kiowa_phy().is_some());
        let heli = match pack {
            Some(f) => vehicle::KiowaRide::new_mxvatv(&f.kiowa_phy()?, f.model_one(), 0, None),
            None => vehicle::KiowaRide::new(&res.read(&res.find_suffix(game_sim::kiowa::KIOWA_PHY)?.name).ok()?, 0, None),
        };
        // And its collision object's ray probe (P216a), what it stands on the ground and strikes
        // the world with.
        let rays = match pack {
            Some(f) => f.rays(Machine::Helicopter),
            None => res.find_suffix(game_sim::kiowa::KIOWA_RAYS).and_then(|e| res.read(&e.name).ok()),
        };
        let heli = match rays {
            Some(col) => heli.with_rays(&col),
            None => heli,
        };
        // And its model's nodes (S270), where its wreck throws the blades from.
        let psg = match pack {
            Some(f) => f.psg(Machine::Helicopter),
            None => res.find_suffix(game_sim::kiowa::KIOWA_MODEL).and_then(|e| res.read(&e.name).ok()),
        };
        let heli = match psg {
            Some(psg) => heli.with_model(&psg),
            None => heli,
        };
        // And the blades it throws (P265), bodies of their own.
        let read = |name: &str| res.find_suffix(name).and_then(|e| res.read(&e.name).ok());
        let heli = match (read(game_sim::kiowa::WRECK_BLADE_PHY), read(game_sim::kiowa::BLADE_MODEL)) {
            (Some(phy), Some(psg)) => heli.with_blades(&phy, &psg, read(game_sim::kiowa::BLADE_RAYS).as_deref(), read(game_sim::kiowa::BLADE_SHAPE).as_deref()),
            _ => heli,
        };
        // And the hull those blades meet (C21).
        let hull = match pack {
            Some(f) => f.hull(Machine::Helicopter),
            None => read(game_sim::kiowa::KIOWA_HULL),
        };
        Some(match hull {
            Some(col) => heli.with_hull(&col),
            None => heli,
        })
    });
    // What it reads its own (P127): it outlives the load's thread.
    let flight_for = {
        let (plane_phy, plane_rays, bike_box, t) = (plane_phy.clone(), plane_rays.clone(), bike_box.clone(), place.terrain.clone());
        let plane_hull = plane_hull.clone();
        let wreck_hull = wreck_hull.clone();
        move |seat: usize| {
            plane_phy.as_ref().map(|bytes| {
                let mut flight = match plane_mxvatv {
                    // MX vs. ATV's (MD80e), on SLUS-21104's steps
                    Some(one) => vehicle::mxvatv_flight(bytes, one, |plane| mods.plane(seat, plane)),
                    None => {
                        let mut plane = phy::Plane::read_bytes(bytes);
                        mods.plane(seat, &mut plane);
                        Flight::new(&plane)
                    }
                };
                if let Some(rays) = &plane_rays {
                    flight = flight.with_rays(rays.clone());
                }
                if let Some(mesh) = &bike_box {
                    flight = flight.with_bike(mesh.clone());
                }
                if let Some(mesh) = &plane_hull {
                    flight = flight.with_hull(mesh.clone());
                }
                if let Some(mesh) = &wreck_hull {
                    flight = flight.with_wreck_hull(mesh.clone());
                }
                flight.area = Some(Area::new(F(t.width as f32), F(t.breadth as f32), t.width_scale, t.breadth_scale));
                if let Some(area) = world_area {
                    flight.area = Some(area);
                }
                flight
            })
        }
    };
    // Every map has the plane (P113): a track whose machine.txt parks none for a player only means
    // no parked copy, and theirs starts on the start line, measured once the ride's ground is up.
    let plane = lot.plane_anywhere(0, flight_for(0), line_start);
    // Every online seat's own biplane bay (N18), which a seat that starts the ride in the plane
    // stands in: the lot parked for the most seats a ride has, a copy's place being its own whatever
    // the count, so every machine has the same off the same disc.
    // Each file bay's with the body's spot and up the original stands an offline plane at there
    // (B6): the model's lift, the bay's SetGroundHeight and the ground's normal.
    let (mut online_bays, online_bodies): (Vec<vehicle::Spawn>, Vec<Option<(Vf, Vf)>>) = match &machines {
        Some((n, b)) => {
            let mut probe = sim_world::Ground::new(&sim_terrain, &place.terrain);
            let mut seats = {
                let mut ground = |at: Vf| lot_ground(tiled.as_deref_mut(), &mut probe, at);
                Lot::around(lot_spots(n, b, &mut ground), net::SEATS, &mut ground)
            };
            (seats.lift, seats.ground_heights) = (lot.lift, lot.ground_heights.clone());
            let mut ground = |from: Vf| lot_ground_normal(tiled.as_deref_mut(), &mut probe, from);
            // And every seat's cars in their bays there (S249), stood as the offline lot's are.
            seats.cars = lot.cars.clone();
            seats.stand_cars(&mut ground);
            lot.online_cars = (0..net::SEATS).map(|i| seats.cars_for(i)).collect();
            seats.of(Machine::Biplane).map(|bay| (bay.spawn, seats.body_spot(bay, &mut ground))).unzip()
        }
        None => (Vec::new(), Vec::new()),
    };
    // And the biplane's, for the same (P98).
    let stock_plane = plane_phy.as_deref().map(read_plane);
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
        println!("biplane: {}", match (plane_phy.is_some(), plane_mxvatv) {
            (true, Some(_)) => "flies on MX vs. ATV's biplane.phy, on its steps",
            (true, None) => "flies on its biplane.phy",
            (false, _) => "no biplane.phy, parked",
        });
    }
    match (&machines, plane.as_ref().filter(|_| lot.bay(Machine::Biplane, 0).is_some())) {
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
    if plane.is_some() && lot.bay(Machine::Biplane, 0).is_none() {
        println!("biplane: no parked copy here; it spawns on the player, and starts on the start line");
    }
    let mut garage = Garage::new(args.vehicle, plane)?;
    garage.cars = lot.cars_for(0);
    garage.heli = lot.heli_for(0);
    garage.modify(&lot, &mods, 0);
    if !garage.cars.is_empty() {
        let names: Vec<&str> = garage.cars.iter().map(|c| c.machine.section()).collect();
        let parked: Vec<&str> = garage.cars.iter().filter(|c| c.bay.is_some()).map(|c| c.machine.section()).collect();
        let parked = if parked.is_empty() { "none parked here".to_string() } else { format!("{} parked here", parked.join(", ")) };
        println!("cars: {} (switch from the pause); {}", names.join(", ").to_ascii_lowercase(), parked.to_ascii_lowercase());
    }
    // The plane's own camera, which the swap hands the player with it: one camera-preset object per
    // machine kind, and the plane's reads `AppStart\BiplaneCPresets.ini` rather than the track's
    // bike presets (R8). The same derived camera and the same two constants the bike's has, because
    // the plane's camera is built by the same constructor; that the original holds the same speed
    // reference for it was not read.
    // The plane's camera presets and its particle list sit in the freeride tracks' second archives;
    // a track without them that has the plane all the same (P113: a race track, a mod map) reads
    // free06's.
    let plane_kit: Option<Res> = match garage.plane.is_some() && !archives.iter().any(|a| a.find_suffix(PLANE_PRESETS).is_some()) {
        true => member_ending(&disc, PLANE_KIT_ARCHIVE).ok().and_then(|name| disc.res(&name).ok()),
        false => None,
    };
    let plane_presets = garage.plane.as_ref().and_then(|_| {
        archives.iter().chain(&plane_kit).find_map(|a| a.read_suffix(PLANE_PRESETS).ok())
    });
    let plane_camera = match &garage.plane {
        None => None,
        Some(_) => {
            let presets: Vec<Preset> = plane_presets
                .as_ref()
                .map(|b| layout_presets(b, layout, Display::Television, PLANE_PRESETS))
                .unwrap_or_default();
            println!("biplane: {} camera presets in {PLANE_PRESETS}", presets.len());
            let mut cam = Race::new(presets, reverse_speed);
            // The fog's ceiling, unless the first player's plane has none (P136).
            cam.cam.ceiling = mods.plane_eye_ceiling(0, Race::ceiling_for_fog(F(look.fog_far)));
            if let Some(start) = Preset::find(&cam.cam.presets, PLANE_CAMERA) {
                cam.cam.set_preset(start);
            }
            cam.cam.snap = true;
            cam.cam.speed_reference = F(SPEED_REFERENCE);
            Some(cam)
        }
    };
    // The Kiowa's skins and rotor parts (V113), where the lot parks one: one skin a livery, as the
    // plane's, out of the machines' archive. The frame finds them in `assets` ([`Kiowa::found`]).
    // MX vs. ATV's (MD80e) in the skin's models off the pack, its rotor parts the pack's too.
    let kiowa_pack = fliers.as_ref().filter(|_| lot.kiowa.as_ref().is_some_and(|k| k.mxvatv.is_some()));
    lot.mxvatv_kiowa = kiowa_pack.map_or_else(Vec::new, |f| (0..vehicle::LIVERIES).filter_map(|l| f.model(Machine::Helicopter, l)).collect());
    if let (Some(res), true) = (machine_res.as_ref(), lot.of(Machine::Helicopter).next().is_some() || garage.heli.is_some()) {
        let liveries = args.players.clamp(1, vehicle::LIVERIES);
        let loaded = match kiowa_pack {
            Some(f) => lot.mxvatv_kiowa[..liveries].iter().filter(|m| Kiowa::load_model(&mut assets, std::slice::from_ref(&f.files), m).is_some()).count(),
            None => (0..liveries).filter(|&l| Kiowa::load(&mut assets, std::slice::from_ref(res), l).is_some()).count(),
        };
        let presets = archives.iter().find_map(|a| a.read_suffix(game_render::kiowa::PRESETS).ok());
        let cameras = presets.map_or(0, |b| Preset::read_bytes(&b, "").len());
        let flown = if garage.heli.is_some() { "flown from the pause or its bay" } else { "parked, no kiowa.phy to fly" };
        println!("kiowa: {loaded} of {liveries} skins loaded, {flown}; {cameras} camera presets in {}", game_render::kiowa::PRESETS);
        if kiowa_pack.is_some() {
            println!("kiowa: MX vs. ATV's, on its steps, drawn in {}", lot.mxvatv_kiowa[..liveries].join(", "));
        }
    }
    // The cars' models (S245), one a livery in play, out of the machines' archive: every car the
    // garage can switch to, parked here or not. The frame finds them in `assets` ([`Car::found`]).
    if let (Some(res), false) = (machine_res.as_ref(), garage.cars.is_empty()) {
        let liveries = args.players.clamp(1, vehicle::LIVERIES);
        let mut loaded = 0;
        for car in &garage.cars {
            for l in 0..liveries {
                loaded += usize::from(game_render::car::Car::load(&mut assets, std::slice::from_ref(res), car_draw::model_name(car.machine, l)).is_some());
            }
        }
        println!("cars: {loaded} of {} models loaded", garage.cars.len() * liveries);
    }
    // Its model and both prop cones, out of the machines' archive, where its textures are too (V21):
    // one model a livery the lot parks (P87), the first copy's plain and the second's the disc's
    // alternate, so a plane is drawn in `plane_drawn[plane.livery]`.
    // MX vs. ATV's (MD80e) in the skin's models off the pack, where its plane flies.
    let plane_pack = fliers.as_ref().filter(|_| plane_mxvatv.is_some());
    let plane_drawn: Vec<Biplane> = match (&garage.plane, machine_res) {
        (Some(_), Some(res)) => {
            let own = vec![res];
            let liveries = args.players.clamp(1, vehicle::LIVERIES);
            let drawn: Vec<Biplane> = match plane_pack {
                // Its pilot sits on the skin's model's pelvis box, as SLUS-21104 seats it (P316).
                Some(f) => {
                    let seat = f.plane_phy().map(|b| biplane::mxvatv_seat(phy::Plane::read_mxvatv_bytes(&b, f.model_one()).plane.pelvis_box.position));
                    (0..liveries)
                        .map_while(|l| Biplane::load_model(&mut assets, std::slice::from_ref(&f.files), f.model(Machine::Biplane, l)?))
                        .map(|d| match seat {
                            Some(seat) => d.with_seat(seat),
                            None => d,
                        })
                        .collect()
                }
                None => (0..liveries).map_while(|l| Biplane::load(&mut assets, &own, l)).collect(),
            };
            println!("biplane: {} of {liveries} liveries loaded", drawn.len());
            if plane_pack.is_some() {
                println!("biplane: MX vs. ATV's, drawn in {}", drawn.iter().map(|d| d.model.as_str()).collect::<Vec<_>>().join(", "));
            }
            drawn
        }
        _ => Vec::new(),
    };
    // Its particles (P58): the emitter list and its `.emt`s, and the track's particle sheet.
    let (mut plane_fx, mut fx_sheet) = match plane_drawn.first() {
        Some(drawn) => match load_plane_fx(&archives, plane_kit.as_ref(), &stem, drawn, &assets) {
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
    // The world's ground and its pieces' shapes in place of the one track's (MD8).
    let mut sim = match tiled.take() {
        Some(t) => Land::Tiles(t),
        None => {
            let track = Place { ground: sim_world::Ground::new(&sim_terrain, &place.terrain), props };
            match trees {
                Some(trees) => Land::Woods(Box::new(game_sim::eco::Wooded { world: track, trees })),
                None => Land::Track(track),
            }
        }
    };
    // The cars the track's `.toy` drives along their clips (V41): free06's train, its trucks and
    // scrapers, the other freeride tracks' trailers. Driven by the simulation (S81): every seat's
    // `Sim::traffic` steps them at the top of its tick and stands their collision meshes, appended to
    // the track's props here, where they are; the picture draws them where the first seat's are.
    // In a world the home tile's `.toy` is still in the tile's own feet: its cars find their ground at
    // the tile's offset and are drawn there (MD8).
    let car_shift = world_mod.as_ref().map_or([0.0; 2], |w| w.home_at().map(|a| a.0));
    // Every tile's cars are its own in a world (MD15, `world_dress`), the home tile's too.
    let traffic = match &world_mod {
        Some(_) => game_render::toy::Traffic::default(),
        None => game_render::toy::Traffic::load(&archives, &stem, &mut |p| car_ground(&mut sim, p, car_shift)),
    };
    // A car's model object is a scene prop's, so its pose stands in for the file's root node: the
    // train's turned cars (train_car_03/04/05 carry a half turn there) are drawn without it (V46).
    for (car, _) in &traffic.cars {
        assets.load_placed(&archives, car.stem());
    }
    if !traffic.cars.is_empty() || !traffic.skipped.is_empty() {
        println!("traffic: {} cars on clips, {} left out", traffic.cars.len(), traffic.skipped.len());
    }
    // A world's tiles' cars stay the picture's alone (MD15): a tiled world has no props list to put
    // their meshes in.
    // The track's animated toys (V43): the windmills, the shovel, the gondolas, the gates and blimps,
    // each a model on a clip of its own. Their clips are the simulation's, stepped with the cars and
    // their collision moved with them; a world's tiles have none (no props list, as for the cars).
    let mut archives = archives;
    let animated = match &world_mod {
        Some(_) => game_render::animated_toy::Animated::default(),
        None => game_render::animated_toy::Animated::load(&mut archives, &stem),
    };
    let archives = archives;
    for model in &animated.models {
        assets.load(&archives, model);
    }
    if !animated.models.is_empty() || !animated.skipped.is_empty() {
        println!("animated toys: {} on clips, {} left out", animated.models.len(), animated.skipped.len());
        animated.skipped.iter().for_each(|why| println!("  {why}"));
    }
    let mut cars = traffic.sim(&archives);
    cars.toys = animated.sim.clone();
    if let Some(props) = sim_world::World::moving(&mut sim) {
        cars.attach(props);
    }
    // The knockable props (S131): the `.toy`'s collections over the scene's models, each a body asleep
    // where its model stands, stepped with the cars and drawn where their bodies are. A world's tiles
    // have none, as they have no cars.
    if world_mod.is_none() {
        let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(name).ok());
        if let Some(toy) = read(&format!("{stem}.toy")) {
            let area = sim_world::World::area(&sim).unwrap_or_else(|| {
                Area::new(F(place.terrain.width as f32), F(place.terrain.breadth as f32), place.terrain.width_scale, place.terrain.breadth_scale)
            });
            // A `.pco` by its path, or (a mod's folder) by its file name wherever it is.
            let mut pco = |p: &str| archives.iter().find_map(|a| a.read(p).ok()).or_else(|| read(p.rsplit('\\').next().unwrap_or(p)));
            let (props, said) = game_sim::prop_set::PropSet::load(
                &phy::Ini::parse_bytes(&toy),
                &format!("Tracks\\{stem}\\"),
                &place.models,
                &mut pco,
                &mut |p| read(p),
                Some(area),
            );
            if !props.is_empty() || !said.is_empty() {
                println!("props: {} collections, {} bodies", props.collections.len(), props.nodes().count());
                said.iter().for_each(|why| println!("  {why}"));
            }
            cars.props = props;
            // The bikes' frame box, which the props' probes are heard striking (F27a2).
            cars.props.bike_frame = bike_box.clone().map(Arc::new);
            // The struck `HasPhysics` cars' rigid bodies (S186), put back past the terrain's edge.
            cars.physics = traffic.physics(&archives, Some(game_sim::prop_world::Edge { min: [F::ZERO, F::ZERO], max: [area.width, area.depth] }));
        }
    }
    let traffic_time = 0.0f64;
    // The scene's clock at the tick before the last, which the picture blends from (V47).
    let traffic_before = 0.0f64;
    let traffic_at: Vec<(String, M4)> = Vec::new();
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
    // The wreck emitters' clip lengths (S173), read whether the ride's sound is on or not: how long a
    // pick keeps them busy moves the game's draws. Every seat's are the first bike's.
    let wreck_kit = Disc::open_configured()
        .and_then(|d| audio::wreck_kit(d, &bike_member, rider_member.as_deref()))
        .unwrap_or_else(|e| {
            println!("wreck sounds: {e} — the sim's emitters count every clip as lasting nothing");
            game_sim::wreck_sound::Kit::default()
        });
    // A mod's stadium (P242): its source game arms no map edge there (`track_mods::in_stadium`), so no
    // bike is shot off its edge and no edge sound plays. A world's tiles and the disc's tracks keep it.
    let stadium = modded.is_some()
        && world_mod.is_none()
        && archives.iter().find_map(|a| a.read_suffix(&format!("{stem}.toy")).ok()).is_some_and(|toy| track_mods::in_stadium(&toy));
    if stadium {
        println!("stadium: {} has no map edge", args.track);
    }
    let new_rider = |phy: &Phy, at: Vf, heading: Vf| -> Rider {
        let mut rider = Rider::new(phy, mounts, at, heading, stunts.clone());
        rider.edge_armed &= !stadium;
        // A put-back draws the rider at once on the riding pose (P270, the user's ask), not sprung
        // over from the trick or the wreck.
        rider.cut_put_back = true;
        // The fork the front wheel steers about, from the model's shocks to its handlebars, which is the
        // axis the bike's loader gives the front wheel (P33). Left straight up with no such nodes.
        if let (Some(h), Some(s)) = (node_at("Handlebars"), node_at("Shocks")) {
            rider.ride.front.steer_axis = game_sim::tire::steering_axis(h, s);
        }
        rider.sense.wrecks_enabled = args.wrecks;
        rider.sense.air_wrecks = args.air_wrecks;
        rider.sense.backwards_bail = args.backwards_bail;
        // A crash on a landing loses its trick (P243, the user's ask): the bank waits out the landing.
        rider.score.lockout = true;
        // The eight numbers the body's *step* reads, which `Rider::new` leaves at zero and says a shell
        // is expected to fill: the five fade rates, the stunt cross-fade's cap and the pose grid's three
        // speed thresholds, as V12 measured them off the body's own object in the running original. With
        // them at zero every fade is a hard cut and the grid's rows all meet at zero, so a grounded body
        // takes the top row at walking pace (V18).
        rider.body_rates = vehicle_params();
        // No start line (V88): `Rider::new` puts the body at a race's gate, and a free ride has none.
        // Left up, a ride started in the plane hands the bike back playing the get-ready idles.
        rider.no_start_line();
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
        if let Some(col) = &bike_col {
            rider.carries_bike_col(Arc::clone(col));
        }
        rider.wreck_kit = wreck_kit.clone();
        rider
    };
    let mut rider = new_rider(&phy, at, heading);
    // The wreck test's 50cc reach (S157) goes by the file's name, which the 500s' names pass too.
    rider.fifty = game_sim::track::fifty_reach(&bike_words[0]);
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
            rider.fifty = game_sim::track::fifty_reach(&bike_words[i]);
            mods.ride(i, &mut rider.ride);
            rider
        })
        .collect();
    for (i, r) in others.iter().enumerate() {
        let p = r.ride.position();
        println!("player {}: starts at {:.0}, {:.0}, {:.0}", i + 2, p.x(), p.y(), p.z());
    }
    // Where a plane with no parked copy starts (P113): beside the start line on this ride's own
    // ground — a mod map's or a world's too — on its wheels, one a seat, a local seat's or an online
    // one's. Empty without the plane.
    let plane_starts: Vec<vehicle::Spawn> = match garage.plane.as_ref().filter(|p| p.flight.is_some()) {
        Some(p) => {
            let rest = p.rest_height(0.0);
            let mut ground = |at: Vf| sim_world::World::ground(&mut sim, Vf::new(at.x(), 1.0e4, at.z(), 1.0)).0.y();
            vehicle::start_spots(line_start, Machine::Biplane, riders.max(net::SEATS), rest, &mut ground)
        }
        None => Vec::new(),
    };
    if lot.bay(Machine::Biplane, 0).is_none()
        && let (Some(p), Some(&start)) = (garage.plane.as_mut(), plane_starts.first())
    {
        p.stand_at(start);
    }
    if online_bays.is_empty() {
        online_bays = plane_starts.clone();
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
            if !biplane::seat(&mut body.rig.set, 0.5, 0.5) {
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
            ground_radius: ground.radius(),
            chassis: CHASSIS,
            plane: plane.as_ref(),
            pilot,
        };
        return moment::draw(&moment, out, &shot, &ground, sky.as_ref(), &assets, &vehicle, &look).map(|()| Err(Leave::Quit));
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
            let mut warned = false;
            for i in 0..frames {
                let mut now = match args.trace_switch {
                    Some(n) if i < n => throttle.clone(),
                    _ => controls.clone(),
                };
                if let Some(&(_, y)) = args.trace_stick_y.iter().rev().find(|(at, _)| *at <= i) {
                    now.stick.1 = F(y);
                }
                // The path cars first, as a tick of the ride steps them (S81).
                cars.frame(dt, &mut sim);
                let f = rider.frame(&phy, &tables, &now, dt, &mut sim);
                // Over a seam (MD29), as a tick of the ride is.
                rider.wrap_round(&sim);
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
                // The map edge's throw apart from every other wreck (MD27), so a ride can say where
                // in a world it was shot off and a crash on the way is not mistaken for one.
                if f.wreck == Some(game_sim::trick::Wreck::CANNONBALL) {
                    println!("thrown: tick {i} at {:.1}, {:.1}", p.x(), p.z());
                }
                // The shootoff's warning as the seat's sound asks it (P231): the first tick the
                // music would start to fade and the rumble to rise.
                if !warned && rider.edge_armed && f.wreck.is_none() {
                    let c = rider.ride.chassis.cofg_world;
                    if let Some(d) = sim_world::World::area(&sim).and_then(|a| audio::world_edge(a, &sim, c)) {
                        println!("shootoff warning: tick {i} at {:.1}, {:.1}, {d:.1} ft from the edge", c.x(), c.z());
                        warned = true;
                    }
                }
            }
            return Ok(Err(Leave::Quit));
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
            cars.frame(dt, &mut sim);
            let f = rider.frame(&phy, &tables, &controls, dt, &mut sim);
            rider.wrap_round(&sim);
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
        return Ok(Err(Leave::Quit));
    }

    // ---- the tape ------------------------------------------------------------------------------
    // `--play` reads one in before anything opens, because a tape that will not parse should say so
    // before a window appears; `--record` starts an empty one and the loop writes it out at the end.
    let head = match &args.play {
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
            // P262: a tape of the other sim (native vs. exact) plays, but drifts from the ride it was
            if let Some(warning) = tape.sim_warning(ps2_float::NATIVE) {
                println!("{warning}");
            }
            Some(Head::new(tape))
        }
    };
    let recorder = args.record.as_ref().map(|path| {
        println!("recording to {}", path.display());
        // What the tape is of, for a human reading a directory of them and for a failure line.
        let mut recorder = Recorder::new(format!("{} on {} / {}", args.rider, args.track, args.bike));
        // A modified ride's tape carries its modifiers, so it replays on them (P55).
        recorder.tape.modifiers = mods;
        // and a ride with Air Wrecks off says so before its first frame (P138)
        if !args.air_wrecks {
            recorder.air_wrecks(false);
        }
        // and one with Backwards Landings Bail on, the same (S161)
        if args.backwards_bail {
            recorder.backwards_bail(true);
        }
        recorder
    });
    // The drawn ground's chunks (P127), the longest part of a world's load, counted for the bar.
    let pre_chunks = match want_chunks {
        false => None,
        true => {
            progress.reach(loading::Stage::Ground)?;
            Some(match (&world_mod, &sim) {
                // A world's ground streams in after the upload (MD17).
                (Some(_), Land::Tiles(_)) => Vec::new(),
                _ => ground.chunks(ground.chunk_cells()),
            })
        }
    };
    Ok(Ok((mods, disc, track_member, world_mod, modded, stem, bike_member, second, rider_member, archives, kit, table, place, look, ground, sky, assets, vehicle, tables, phy_bytes, phy, other_phys, stock_phys, bike_words, other_riders, riders, body, ragdolled, chassis, layout, cpresets, camera, lot, flight_for, online_bays, online_bodies, plane_starts, stock_plane, garage, plane_presets, plane_camera, plane_drawn, plane_fx, fx_sheet, soft_sheet, sim, traffic, animated, cars, traffic_time, traffic_before, traffic_at, rider, others, head, recorder, pre_chunks, at, car_shift, pack_clips, stunts)))
        }
    };
    let mut load_screen = LoadScreen::default();
    let built = match shell.as_deref_mut() {
        Some(shell) => load_in_front(shell, &args, progress, &mut load_screen, build)?,
        None if std::env::var_os(loading::THREADED).is_some() => loading::on_worker(progress, build, || Ok(None::<Leave>))?.unwrap_or_else(Err),
        None => build()?,
    };
    let (mut mods, mut disc, track_member, world_mod, modded, stem, bike_member, second, rider_member, archives, mut kit, table, place, mut look, ground, sky, mut assets, vehicle, tables, phy_bytes, phy, other_phys, mut stock_phys, mut bike_words, other_riders, riders, body, ragdolled, chassis, mut layout, cpresets, camera, mut lot, flight_for, online_bays, online_bodies, plane_starts, stock_plane, garage, plane_presets, plane_camera, plane_drawn, mut plane_fx, mut fx_sheet, soft_sheet, mut sim, traffic, animated, cars, mut traffic_time, mut traffic_before, mut traffic_at, rider, others, mut head, recorder, pre_chunks, at, car_shift, pack_clips, stunts) = match built {
        Ok(built) => built,
        Err(leave) => return Ok(leave),
    };
    // The pause's Tricks page (P259) lists what this ride can do: the 24, the packs' and their moves.
    if let Some(s) = shell.as_deref_mut() {
        s.front.menu.trick_rows = menu::trick_list::rows(&stunts, phy.tricks.as_ref());
        println!("tricks page: {} tricks", s.front.menu.trick_rows.len());
        // The Fog slider's feet (P276): the original's fog end here.
        s.front.menu.ride_fog = Some(look.fog_far);
    }
    // And its rider, loaded the first time a view shows the page: `Some(None)` when it could not be.
    let mut trick_stage: Option<Option<trick_view::TrickView>> = None;
    // What a picture drawn while a tick runs on the sim thread asks of the place (N23a): the same
    // ground, and none of the props, which the tick has. A tiled world's ticks stay on the loop.
    let mut drawn_land = match (&sim, keep_terrain.get()) {
        (Land::Tiles(_), _) | (_, None) => None,
        (_, Some(terrain)) => Some(Land::Track(Place { ground: sim_world::Ground::new(terrain, &place.terrain), props: Default::default() })),
    };
    // The Kiowa's skins the load put in `assets` (V113), one a livery.
    let kiowa_drawn: Vec<Kiowa> = match lot.mxvatv_kiowa.is_empty() {
        true => (0..vehicle::LIVERIES).map_while(|l| Kiowa::found(&assets, l)).collect(),
        // MX vs. ATV's (MD80e), in the skin's models
        false => lot.mxvatv_kiowa.iter().map_while(|m| Kiowa::found_model(&assets, m)).collect(),
    };
    // And the cars' models (S245), each car in each livery the load put there.
    let car_models: Vec<game_render::car::Car> = Machine::ALL
        .iter()
        .flat_map(|&m| (0..vehicle::LIVERIES).map(move |l| car_draw::model_name(m, l)))
        .filter_map(|name| game_render::car::Car::found(&assets, name))
        .collect();
    // The helicopter's own camera (P216), built as the plane's is, on `HeloCPresets.ini` (in a
    // freeride track's second archive, or free06's) and starting on its first preset.
    // The file's bytes are kept, as the plane's are, for a split's copy of its presets and the
    // seat's own laid over them (P216b).
    let heli_presets: Option<Vec<u8>> = garage.heli.as_ref().and_then(|_| {
        let kit = match archives.iter().any(|a| a.find_suffix(game_render::kiowa::PRESETS).is_some()) {
            true => None,
            false => member_ending(&disc, PLANE_KIT_ARCHIVE).ok().and_then(|name| disc.res(&name).ok()),
        };
        archives.iter().chain(&kit).find_map(|a| a.read_suffix(game_render::kiowa::PRESETS).ok())
    });
    let heli_camera: Option<Race> = heli_presets.as_ref().map(|bytes| {
        let presets = layout_presets(bytes, layout, Display::Television, game_render::kiowa::PRESETS);
        let mut cam = Race::new(presets, F(REVERSE_SPEED));
        // The fog's ceiling, unless the first player's flying has none (P136, P216b).
        cam.cam.ceiling = mods.plane_eye_ceiling(0, Race::ceiling_for_fog(F(look.fog_far)));
        if let Some(start) = Preset::find(&cam.cam.presets, HELI_CAMERA) {
            cam.cam.set_preset(start);
        }
        if !cam.cam.presets.is_empty() {
            println!("kiowa: its camera starts on {}", cam.cam.preset().name);
        }
        cam.cam.snap = true;
        cam.cam.speed_reference = F(SPEED_REFERENCE);
        cam
    });
    // The cars' own cameras (S246), each built as the bike's is (they are ground machines: the fog's
    // ceiling, no plane's), on its kind's presets file (in a freeride track's second archive, or
    // free06's) and starting on its first preset, "Race Cam 1", where S149a's states have all three.
    // The files' bytes are kept, as the helicopter's are, for a display's copy of their presets.
    let car_presets: Vec<(Kind, Vec<u8>)> = garage
        .cars
        .iter()
        .filter_map(|c| {
            let (kind, file) = (c.machine.kind()?, chase::car_presets(c.machine.car_physics()?));
            let kit = match archives.iter().any(|a| a.find_suffix(file).is_some()) {
                true => None,
                false => member_ending(&disc, PLANE_KIT_ARCHIVE).ok().and_then(|name| disc.res(&name).ok()),
            };
            Some((kind, archives.iter().chain(&kit).find_map(|a| a.read_suffix(file).ok())?))
        })
        .collect();
    let car_cameras: Vec<(Kind, Race)> = car_presets
        .iter()
        .map(|(kind, bytes)| {
            let file = car_presets_file(*kind);
            let mut cam = Race::new(layout_presets(bytes, layout, Display::Television, file), F(REVERSE_SPEED));
            cam.cam.ceiling = Race::ceiling_for_fog(F(look.fog_far));
            if let Some(start) = Preset::find(&cam.cam.presets, CAR_CAMERA) {
                cam.cam.set_preset(start);
            }
            if !cam.cam.presets.is_empty() {
                println!("{}: its camera starts on {}, {} presets in {file}", kind.name(), cam.cam.preset().name, cam.cam.presets.len());
            }
            cam.cam.snap = true;
            cam.cam.speed_reference = F(SPEED_REFERENCE);
            (*kind, cam)
        })
        .collect();
    // Where the first seat's path cars stood before the last tick (S81), which the picture blends from.
    let mut cars_before: Vec<game_sim::math::Mat4> = Vec::new();
    // The knockable props' bodies (S131), each by its scene model's place in the picture's list (found
    // by its name and place), with where that model stands as loaded and the body's node at rest
    // there: a body off that node is drawn at its node, one put back where the scene stands it.
    let prop_looks: Vec<Option<(usize, view::Placement, game_sim::contact::Node)>> = cars
        .props
        .nodes()
        .map(|(m, rest)| {
            let p = &place.models[m];
            let at = v3(p.position[0].0, p.position[1].0, p.position[2].0);
            let name = view::slt_name(&p.file);
            // The two readers' numbers may part by a rounding (the simulation's is the game's own).
            let k = look.models.iter().position(|l| l.pos.sub(at).len() < 0.01 && l.model.eq_ignore_ascii_case(&name))?;
            Some((k, look.models[k].clone(), *rest))
        })
        .collect();
    let unseen = prop_looks.iter().filter(|p| p.is_none()).count();
    if unseen > 0 {
        println!("props: {unseen} bodies with no model in the picture, drawn where the scene stands them");
    }
    if let Some(since) = load_screen.since {
        println!("loading: built, {} frames drawn over {:.2} s", load_screen.frames, since.elapsed().as_secs_f32());
    }
    // The track's birds and sequence-animated toys (V44): stepped by the ticks below, as their
    // triggers answer the riders, and drawn between them. A world has none, as it has no home
    // tile's cars.
    let mut creatures = match &world_mod {
        Some(_) => game_render::creature::Creatures::default(),
        None => game_render::creature::Creatures::load(&archives, &stem),
    };
    for model in creatures.models() {
        assets.load(&archives, model);
    }
    if !creatures.birds.is_empty() || !creatures.toys.is_empty() || !creatures.skipped.is_empty() {
        println!("creatures: {} birds, {} toys, {} left out", creatures.birds.len(), creatures.toys.len(), creatures.skipped.len());
    }
    let mut creatures_at: Vec<game_render::creature::Placed> = Vec::new();
    // The software renderer skins each one into a copy of its model of its own.
    let creature_copies: Vec<String> = (0..creatures.birds.len() + creatures.toys.len()).map(|i| format!("creature#{i}")).collect();
    // The bikes' particles (P74): the track's list, one manager a seat stepped off it, and the
    // track's sheet — the plane's, which is the same sheet, where the plane has loaded it already.
    let (bike_fx_list, soft_sheet) = match load_bike_fx(&archives, &stem) {
        Ok((fx, sheet)) => {
            let soft_sheet = soft_sheet.or_else(|| Some(sheet.clone()));
            if plane_fx.is_none() && fx_sheet.is_none() {
                fx_sheet = Some(sheet);
            }
            (Some(fx), soft_sheet)
        }
        Err(why) => {
            println!("bike: no particles: {why}");
            (None, soft_sheet)
        }
    };
    let mut bike_fx: Vec<particles::PlaneFx> = Vec::new();
    // The track's own smokes, fires and mists (V45): the `.toy`'s emitters, on the same sheet. A
    // world's tiles carry theirs in their own feet, which nothing here shifts, so a world has none.
    let (mut track_fx, soft_sheet) = match (&world_mod, load_track_fx(&archives, &stem)) {
        (None, Ok((fx, sheet))) => {
            if fx_sheet.is_none() {
                fx_sheet = Some(sheet.clone());
            }
            (Some(fx), soft_sheet.or(Some(sheet)))
        }
        (None, Err(why)) => {
            println!("track: no particles: {why}");
            (None, soft_sheet)
        }
        (Some(_), _) => (None, soft_sheet),
    };
    // The Kiowa's (V114): the track's `helicopterparticles.dat`, the wash under its rotor and its
    // wreck's dust, one manager a seat stepped off it as the bikes' are, and the track's sheet where
    // nothing above loaded it. Only where the disc has a Kiowa to fly.
    let (heli_fx_list, soft_sheet) = match (garage.heli.is_some(), kiowa_drawn.first()) {
        (true, Some(drawn)) => match load_heli_fx(&archives, &stem, drawn, &assets) {
            Ok((fx, binds, sheet)) => {
                if fx_sheet.is_none() {
                    fx_sheet = Some(sheet.clone());
                }
                (Some((fx, binds)), soft_sheet.or(Some(sheet)))
            }
            Err(why) => {
                println!("kiowa: no particles: {why}");
                (None, soft_sheet)
            }
        },
        _ => (None, soft_sheet),
    };
    let mut heli_fx: Vec<particles::PlaneFx> = Vec::new();
    // The cars' (S248): each car the garage can switch to, its list out of the track's archives
    // (`buggyparticles.dat`, …) on its first livery's nodes. A seat keeps one manager a car, the one
    // it drives stepped off the car, the rest off, so what they threw runs out.
    let mut car_fx_lists: Vec<(Machine, particles::PlaneFx, Vec<(String, M4)>)> = Vec::new();
    let mut soft_sheet = soft_sheet;
    for car in &garage.cars {
        if car_fx_lists.iter().any(|(m, _, _)| *m == car.machine) {
            continue;
        }
        match load_car_fx(&archives, &stem, car.machine, car_draw::find(&car_models, car.machine, 0), &assets) {
            Ok((fx, binds, sheet)) => {
                if fx_sheet.is_none() {
                    fx_sheet = Some(sheet.clone());
                }
                soft_sheet = soft_sheet.or(Some(sheet));
                car_fx_lists.push((car.machine, fx, binds));
            }
            Err(why) => println!("car: {}: no particles: {why}", car.machine.dir()),
        }
    }
    let mut car_fx: Vec<Vec<particles::PlaneFx>> = Vec::new();
    // nat01's rain (V73): the `.toy`'s weather box and the triggers that turn it up and down, drawn as
    // glows of the box's texture. A world's tiles have none.
    let (mut rain, rain_art) = match world_mod {
        None => load_rain(&archives, &stem).map_or((None, None), |(rain, art)| (Some(rain), Some(art))),
        Some(_) => (None, None),
    };
    let mut rain_dt = 0.0f32;

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
            own_sound.insert(start_sound(&args, &mut disc, &dir, &settings, false)).live()
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
            // A track mod's own `.aud` names sounds from its own game's bank, which this disc's
            // does not hold (MD2): it rides with the race events' sounds and no track emitters.
            let track_res = match &track_member {
                Ok(member) => member.clone(),
                Err(_) => second.clone().unwrap_or_default(),
            };
            let read = Disc::open_configured().and_then(|d| {
                audio::Sound::read(
                    mixer,
                    d,
                    &bikes,
                    rider_member.as_deref(),
                    &machines,
                    &track_res,
                    second.as_deref(),
                    &stem,
                )
            });
            // A world of the disc's own tracks (P71) sounds every map's emitters where each map is.
            let read = read.and_then(|mut s| {
                if let Some(w) = &world_mod {
                    let maps = w.map_sounds();
                    if !maps.is_empty() {
                        s.set_map_emitters(Disc::open_configured()?, &maps)?;
                    }
                }
                Ok(s)
            });
            // The path cars' loops (V74), off the same `.toy` the cars are: each rides its car.
            let read = read.and_then(|mut s| {
                let names: Vec<Vec<String>> = traffic.cars.iter().map(|(car, _)| car.sounds.clone()).collect();
                if track_member.is_ok() && names.iter().any(|n| !n.is_empty()) {
                    s.set_toys(Disc::open_configured()?, &track_res, &stem, &names)?;
                }
                Ok(s)
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
    if let Some(leave) = load_tick(window, pad_seats, front.as_deref_mut(), &args, &progress, &mut load_screen).await? {
        return Ok(leave);
    }
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
            Scheme::New if seats.keys() == pad::Keys::HomeRow => println!("pad: none found — riding on the keyboard (J or Space gas, K or Shift brake, WASD and YUIO the sticks, H L G ; the face buttons)"),
            Scheme::New => println!("pad: none found — riding on the keyboard (W gas, S brake, A D steer, I K lean, Shift clutch, F G E tricks, R reset, C camera)"),
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
            "throttle: the {0} trigger's travel — the plane's part throttle, the bike's switch, and no {0} control this session",
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
        (None, Backend::Gpu) => match headless_gpu() {
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
        if let Some(leave) = load_tick(window, pad_seats, front.as_deref_mut(), &args, &progress, &mut load_screen).await? {
            return Ok(leave);
        }
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
        let body = body.map(|mut b| {
            if let Some(p) = &pack_clips {
                b.add_pack(p, &stunts.stunts);
            }
            b
        });
        // Its own name and number (P86), on whatever of its bike and body is loaded below.
        let own_look = args.others.get(i - 1).map(|p| p.look.clone()).unwrap_or_default();
        if let Some(k) = &mut kit {
            let pick = args.others.get(i - 1);
            let (rider, bike) = pick.map_or((&args.rider, &args.bike), |p| (&p.rider, &p.bike));
            assets.make_texture(dress::NAME_TEXTURE, k.texture(&mut disc, rider, &own_look, bike));
        }
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
        let chassis = chassis.map(|mut c| {
            if let Some(p) = &pack_clips {
                c.add_pack(p, &stunts.stunts);
            }
            c
        });
        let copy = |assets: &mut Assets, name: &str| -> Option<String> {
            let own = format!("{name}@{}", i + 1);
            let loaded = assets.models.get(name)?.clone();
            assets.models.insert(own.clone(), loaded);
            Some(own)
        };
        let (bike_model, parts) = match &own_bike {
            Some((_, v, res)) => {
                let own = format!("{CHASSIS}@{}", i + 1);
                let file = game_render::vehicle::chassis_model(res, v);
                let drawn = assets.load_repainted(res, &file, &own, &v.materials);
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
                // Its own plates when anyone has a number (P86): the parts again under its own names.
                let parts = match kit.is_some() {
                    true => vehicle
                        .parts
                        .iter()
                        .map(|part| {
                            let name = format!("{}@{}", part.model, i + 1);
                            assets.own(1);
                            assets.load_repainted(&archives, &part.model, &name, &vehicle.materials);
                            assets.own(0);
                            game_render::scene::Part { model: name, placement: part.placement.clone() }
                        })
                        .collect(),
                    false => vehicle.parts.clone(),
                };
                (model.unwrap_or_else(|| CHASSIS.to_string()), parts)
            }
        };
        // A picked rider's body is loaded again under its own name with its own gear's renames, which
        // is the only way a model gets other textures.
        let own_gear = match (body_model.as_deref(), own_rider, &body) {
            (Some(name), Some((_, member)), Some(b)) => {
                let own = format!("{name}@{}", i + 1);
                let mut res = vec![disc.res(member)?];
                // In its own look over its gear set (P86), the pieces' paint out of the wardrobe.
                let pairs = match &kit {
                    Some(k) => {
                        if let Some(custom) = &k.custom {
                            res.push(disc.res(custom)?);
                        }
                        k.repaint(&b.rider.suit, &own_look)
                    }
                    None => b.rider.suit.materials.clone(),
                };
                assets.load_repainted(&res, name, &own, &pairs).then_some(own)
            }
            _ => None,
        };
        let own_body = own_gear.or_else(|| {
            body_model.as_deref().and_then(|name| match gpu_on {
                true => Some(name.to_string()),
                false => copy(&mut assets, name),
            })
        });
        // The drawn fork stops short of what hangs off the bars (V62).
        let mut chassis = chassis;
        if let Some(c) = &mut chassis {
            c.stop_fork_in(&assets.models, &bike_model, &parts);
        }
        rigs.push(Rig {
            body,
            chassis,
            camera: Race { cam: camera.cam.clone() },
            // Their own plane's camera, on the same presets as the first player's (P87).
            // Its ceiling theirs (P136).
            plane_camera: plane_camera.as_ref().map(|c| {
                let mut cam = Race { cam: c.cam.clone() };
                cam.cam.ceiling = mods.plane_eye_ceiling(i, Race::ceiling_for_fog(F(look.fog_far)));
                cam
            }),
            // Their own helicopter's the same (P216b).
            heli_camera: heli_camera.as_ref().map(|c| {
                let mut cam = Race { cam: c.cam.clone() };
                cam.cam.ceiling = mods.plane_eye_ceiling(i, Race::ceiling_for_fog(F(look.fog_far)));
                cam
            }),
            // And their own cars' (S246).
            car_cameras: car_cameras.iter().map(|(k, c)| (*k, Race { cam: c.cam.clone() })).collect(),
            wreck_camera: chase::WreckCamera::new(false),
            framing: (chase::Framing::ORIGINAL, chase::Framing::ORIGINAL),
            camera_presets: Default::default(),
            camera_started: false,
            fov_ease: Default::default(),
            look_back: Default::default(),
            thrill: Default::default(),
            odometer: overlay::Odometer::default(),
            preload_meter: overlay::PreloadMeter::default(),
            pilot_lean: vehicle::PilotLean::default(),
            ride_hud: ride_hud::Tracker::default(),
            bike_model,
            body_model: own_body,
            parts,
            modified: !mods.seat_is_original(i),
        });
    }
    // The drawn fork stops short of what hangs off the bars (V62).
    let mut chassis = chassis;
    if let Some(c) = &mut chassis {
        println!("chassis: the fork is drawn to a compression of {:.3}", c.stop_fork_in(&assets.models, CHASSIS, &vehicle.parts));
    }
    // What a player who joins the running ride later is made of (P76): the cameras as they stand
    // before anyone rides, and the first player's rider model, which they wear.
    let join_cameras = (camera.cam.clone(), plane_camera.as_ref().map(|c| c.cam.clone()));
    let join_body_model = body_model.clone();
    rigs.insert(
        0,
        Rig {
            body,
            chassis,
            camera,
            plane_camera,
            heli_camera: heli_camera.as_ref().map(|c| Race { cam: c.cam.clone() }),
            car_cameras: car_cameras.iter().map(|(k, c)| (*k, Race { cam: c.cam.clone() })).collect(),
            wreck_camera: chase::WreckCamera::new(false),
            framing: (chase::Framing::ORIGINAL, chase::Framing::ORIGINAL),
            camera_presets: Default::default(),
            camera_started: false,
            fov_ease: Default::default(),
            look_back: Default::default(),
            thrill: Default::default(),
            odometer: overlay::Odometer::default(),
            preload_meter: overlay::PreloadMeter::default(),
            pilot_lean: vehicle::PilotLean::default(),
            ride_hud: ride_hud::Tracker::default(),
            bike_model: CHASSIS.to_string(),
            body_model,
            parts: vehicle.parts.clone(),
            modified: !mods.seat_is_original(0),
        },
    );

    // A world's tiles dressed (MD15): every tile's props, plants and cars near the start now, the
    // rest as the riders come, each read from its own folder and moved by its offset.
    let mut dress_stream: Option<world_dress::DressStream> = None;
    // The near pieces' terrains, for the software renderer's world ground (MD15).
    let mut soft_terrains: std::collections::HashMap<usize, Vec<u8>> = std::collections::HashMap::new();
    let mut dressed: Vec<world_dress::Dressed> = Vec::new();
    // Pictures drawn, which staggers the far cars' poses (MD28).
    let mut car_pictures = 0u64;
    // Tiles dressed mid-ride whose models are still going up, a few a frame, and not drawn till
    // they all are: by dressing index, the models left, and their slow textures' levels being made
    // on a thread of their own (MD28: a tile's at once was a 57 ms frame).
    type Levels = std::sync::mpsc::Receiver<Vec<(usize, u8, game_render::gpu::Mips)>>;
    let mut unshown: Vec<(usize, Vec<String>, Option<Levels>)> = Vec::new();
    let (world_reach, wrap) = match &sim {
        Land::Tiles(t) => (Some(t.reach), drawn_wrap(t)),
        _ => (None, None),
    };
    if let (Some(w), Some(reach)) = (&world_mod, world_reach) {
        let starts: Vec<[f32; 2]> = std::iter::once(at).chain(garage.plane_flown().map(|p| p.at)).map(|p| [p.x(), p.z()]).collect();
        // With what stands across a wrapped world's seams (MD43).
        let starts = wrapped_near(wrap, &starts);
        let dress_at = Instant::now();
        let (mut stream, first) = world_dress::DressStream::start(w, reach, &starts, args.plants.unwrap_or_default());
        for b in first {
            let mut d = world_dress::take(b, w, &mut assets, &mut |p, by| car_ground(&mut sim, p, by));
            if let Some(g) = &mut d.grove {
                g.foliage = args.foliage.unwrap_or_default();
            }
            dressed.push(d);
        }
        for line in stream.take_log() {
            println!("{line}");
        }
        println!(
            "world props: {} of {} tiles dressed before the ride in {:.2} s, {} cars on clips",
            dressed.len(),
            w.tile_count,
            dress_at.elapsed().as_secs_f32(),
            dressed.iter().map(|d| d.traffic.cars.len()).sum::<usize>()
        );
        dress_stream = Some(stream);
    }
    // A world's ground on the GPU (MD17): the pieces near the start now, the rest as the riders come.
    let mut ground_stream: Option<worlds::GroundStream> = None;
    // How many times the app had been sent away when the ground's uploads began (MD97).
    let mut ground_away = gpuwin::times_away();
    // How many times the app had entered the background when the ride last gave its models back
    // (P298a).
    let mut backgrounded = resume::backgrounds();
    // And its maps' pages, near the riders (P283).
    let mut paint_stream: Option<paint_stream::PaintStream> = None;
    // The most ground chunks a frame of the ride handed the GPU, how many in all, and in how many
    // frames (MD28).
    let mut ground_written = (0usize, 0usize, 0usize);
    // The most chunks the GPU's ground held at once in the ride, and the most it had room for (MD23).
    let mut ground_most = (0usize, 0usize);
    // The picture time a world's ground fades were last moved to (V92), in seconds.
    let mut ground_shown = 0.0f32;
    // A world's look by map (MD9): every tile's ground paint, light and sky, read from its folder.
    // On the GPU only the maps near the start keep their pages, the rest their far pages, and the
    // ride streams them (P283: world_ultra's 77 maps' pages were 1.3 GB read and as much uploaded).
    // `--seams` reads every map's paint, and `--world-ground all` draws the whole world.
    let paint_near = match (gpu_on && !args.seams && !args.world_ground_all, world_reach) {
        (true, Some(reach)) => {
            let starts: Vec<[f32; 2]> = std::iter::once(at).chain(garage.plane_flown().map(|p| p.at)).map(|p| [p.x(), p.z()]).collect();
            Some((wrapped_near(wrap, &starts), reach))
        }
        _ => None,
    };
    let mut world_look = world_mod.as_ref().map(|w| {
        let owned = w.regions().owned;
        let keep = |i: usize| {
            paint_near.as_ref().is_some_and(|(starts, reach)| {
                let o = owned[i];
                starts.iter().any(|p| (o[0] - p[0]).max(p[0] - o[2]).max(0.0).hypot((o[1] - p[1]).max(p[1] - o[3]).max(0.0)) <= reach.load.0)
            })
        };
        let near = paint_near.as_ref().map(|(_, reach)| (&keep as &(dyn Fn(usize) -> bool + Sync), reach.drop.0));
        let l = w.look_near(&archives[1..], ground::DIRT, Lighting::of(&look), near);
        let paged = l.paint.tiles.iter().filter(|t| t.pages.is_some()).count();
        let skies = l.skies.iter().filter(|s| s.is_some()).count();
        println!("world look: {paged} of {} tiles painted from their pages, {skies} with a sky of their own", l.paint.tiles.len());
        l
    });
    // `--seams` (MD51): the ground and its paint between each two maps that face, the wrap's too.
    if let (true, Some(w), Some(l), Land::Tiles(t)) = (args.seams, &world_mod, &world_look, &sim) {
        for line in w.crossings(t, &l.paint) {
            println!("{line}");
        }
    }
    // Everything the GPU draws, uploaded once: the props, the sky, the ground and every model.
    // Under the front end the models go up a slice between loading frames (P127).
    if let Some(leave) = load_tick(window, pad_seats, front.as_deref_mut(), &args, &progress, &mut load_screen).await? {
        return Ok(leave);
    }
    // A world's tiles within reach of the start read while loading, not on the ride's first frame
    // (MD28: 27 ms on world_mxu's), and every tile's shapes, which the first wreck's search would
    // read (63-132 ms).
    if let Land::Tiles(t) = &mut sim {
        t.load_near(&std::iter::once(at).chain(garage.plane_flown().map(|p| p.at)).collect::<Vec<_>>());
        t.every_shape();
        for line in t.take_log() {
            println!("{line}");
        }
    }
    if gpu_on && renderer_of(&mut offscreen, window).is_some() {
        // Where the ride's streaming will look from on its first tick: the rider, and the plane if it
        // is the one up (MD28: a parked plane's pieces were built here and dropped on the first frame).
        let starts: Vec<[f32; 2]> = std::iter::once(at).chain(garage.plane_flown().map(|p| p.at)).map(|p| [p.x(), p.z()]).collect();
        let at = Instant::now();
        let mut pre_chunks = pre_chunks;
        // A world's ground is its pieces', streamed (MD17), each tile painted from its own pages and
        // the joins blended from the tiles either side (MD9).
        let (chunks, paint) = match (&world_mod, &sim) {
            (Some(_), Land::Tiles(_)) => (
                Vec::new(),
                world_look.as_ref().map_or(game_render::gpu::GroundPaint::Bare, |l| game_render::gpu::GroundPaint::World(&l.paint)),
            ),
            _ => {
                let paint = match (&ground.paint, &ground.pages) {
                    (Some(splat), _) => game_render::gpu::GroundPaint::Splat(splat),
                    (None, Some(pages)) => game_render::gpu::GroundPaint::Pages(pages),
                    (None, None) => game_render::gpu::GroundPaint::Bare,
                };
                (pre_chunks.take().unwrap_or_else(|| ground.chunks(ground.chunk_cells())), paint)
            }
        };
        progress.reach(loading::Stage::Upload)?;
        wait_in_front("the upload");
        let r = renderer_of(&mut offscreen, window).expect("checked");
        r.forget_assets();
        r.set_scene(&look);
        for d in &dressed {
            r.add_props(d.index as u32, &d.props, d.grove.clone());
        }
        r.set_sky(sky.as_ref());
        r.set_ground_painted(&chunks, paint, ground::DIRT)?;
        r.set_wrap(None);
        r.set_prop_fade(None);
        if let (Some(w), Land::Tiles(t)) = (&world_mod, &sim) {
            let (reach, cells) = (t.reach, ground.chunk_cells());
            let ground_at = Instant::now();
            // A wrapped world's copies' ground is built with the world's own (MD43).
            let starts = wrapped_near(drawn_wrap(t), &starts);
            r.set_ground_streamed(cells, paint, ground::DIRT, t.area.width.0.hypot(t.area.depth.0))?;
            // And drawn again one width off across each seam.
            r.set_wrap(drawn_wrap(t));
            // Which maps' pages went up, and at what size (MD31): a map past the device's layers is dirt.
            if let Some(l) = &world_look {
                let limits = r.device.limits();
                let packed = l.paint.pack_for(limits.max_texture_array_layers, limits.max_texture_dimension_2d);
                let (arrays, detail) = game_render::gpu::world_paint_arrays(&l.paint, &packed);
                let arrays: Vec<String> = arrays.iter().map(|a| format!("{}x{} x {}", a.width, a.height, a.layers)).collect();
                // And the disc maps' splats drawn per pixel (MD35).
                let splats = game_render::gpu::world_splat_array(&l.paint, &packed)
                    .map_or("none".to_string(), |a| format!("{}x{} x {}", a.width, a.height, a.layers));
                println!(
                    "world paint: {} of {} paged maps on the GPU, pages {}, detail {}x{} x {}, {} sharp splats, layers {splats}",
                    packed.iter().flatten().count(),
                    l.paint.tiles.iter().filter(|t| t.pages.is_some()).count(),
                    arrays.join(", "),
                    detail.width,
                    detail.height,
                    detail.layers,
                    l.paint.tiles.iter().zip(&packed).filter(|(t, at)| at.is_some() && t.sharp().is_some()).count(),
                );
                // And the roads and ruts laid over them round each eye (V115).
                let meshed = game_render::gpu::world_mesh_tiles(&l.paint, &packed);
                let names: Vec<&str> = meshed.iter().filter_map(|&i| l.names.get(i).map(String::as_str)).collect();
                match (r.ground_meshed(), game_render::gpu::world_meshes(&l.paint, &packed)) {
                    (true, Some(m)) => println!("world paint: roads and ruts of {} ({} triangles, {} textures)", names.join(", "), m.tris.len(), m.textures.len()),
                    (false, Some(_)) => println!("world paint: roads and ruts of {} left out: the device samples too few textures", names.join(", ")),
                    (_, None) => println!("world paint: no map has roads or ruts"),
                }
            }
            // The whole world at its coarsest under it, where a piece is not loaded yet, and its
            // props and plants faded out by the reach they come in at (V92).
            if args.world_stand_in {
                wait_in_front("the stand-in");
                r.set_prop_fade(Some([reach.load.0, PROP_FADE_BAND.min(reach.load.0 * 0.5)]));
                let at = Instant::now();
                match worlds::stand_in(w, t, cells).and_then(|s| {
                    let line = format!("{} x {} cells of {:.0} ft, {} chunks", s.cells[0], s.cells[1], s.step, s.piece.chunks.len());
                    r.set_ground_stand_in(s).map(|_| line)
                }) {
                    Ok(line) => println!("world stand-in: {line} in {:.2} s", at.elapsed().as_secs_f32()),
                    Err(e) => println!("world stand-in: none: {e}"),
                }
            }
            // Each piece up as it is built, not all held till the last is (P283).
            let (mut built, mut failed) = (0, Ok(()));
            let mut stream = worlds::GroundStream::start_into(w, cells, reach, args.world_fine, &starts, args.world_ground_all, &mut |i, made| {
                if failed.is_ok() {
                    wait_in_front("the world's ground");
                    failed = r.add_ground_piece_of(i as u32, made);
                    r.show_ground_piece(i as u32, 0.0);
                    built += 1;
                }
            });
            failed?;
            for line in stream.take_log() {
                println!("{line}");
            }
            println!(
                "world ground: {} of {} pieces built before the ride in {:.2} s ({}; load {:.0} ft, drop {:.0} ft; {})",
                built,
                w.pieces.len(),
                ground_at.elapsed().as_secs_f32(),
                if args.world_ground_all { "all" } else { "near the start" },
                reach.load.0,
                reach.drop.0,
                match args.world_fine.filter(|_| !args.world_ground_all) {
                    Some(f) => format!("{} coarse past {:.0} ft", stream.held_coarse(), f.load.0),
                    None => "all at full detail".to_string(),
                }
            );
            if let Some(n) = args.world_chunks {
                stream.chunks_a_frame = n;
            }
            ground_stream = Some(stream);
            // The maps' pages are the GPU's now: the CPU's let go, and the far maps' streamed (P283).
            if let (Some(l), true) = (&mut world_look, paint_near.is_some()) {
                for tile in &mut l.paint.tiles {
                    if let Some(pages) = &mut tile.pages {
                        worlds::hollow(pages, &mut tile.sharp);
                    }
                    // And a disc map's splat textures and light map, on the GPU or baked in now: its
                    // passes and roads stay, which say how it is drawn.
                    if let Some(splat) = tile.sharp.as_mut().and_then(|s| std::sync::Arc::get_mut(&mut s.splat)) {
                        splat.textures = Vec::new();
                        splat.shadow = None;
                    }
                }
                wait_in_front("the world's paint");
                let stream = paint_stream::PaintStream::start(w, reach, r);
                if let Some((used, shown)) = r.world_paint_pool() {
                    let used: Vec<String> = used.iter().map(|(u, c)| format!("{u} of {c}")).collect();
                    println!(
                        "world paint: {shown} maps on their own pages at the start, the rest on far pages; layers held {} (load {:.0} ft, drop {:.0} ft)",
                        used.join(", "),
                        reach.load.0,
                        reach.drop.0
                    );
                }
                paint_stream = Some(stream);
            }
        }
        // `upload_all`'s order, a slice at a time.
        let mut names: Vec<&String> = assets.models.keys().collect();
        names.sort();
        for (i, slice) in names.chunks(UPLOAD_SLICE).enumerate() {
            progress.count(i * UPLOAD_SLICE, names.len())?;
            if let Some(leave) = load_tick(window, pad_seats, front.as_deref_mut(), &args, &progress, &mut load_screen).await? {
                return Ok(leave);
            }
            wait_in_front("the models");
            renderer_of(&mut offscreen, window).expect("checked").upload_models(&assets, slice);
        }
        let r = renderer_of(&mut offscreen, window).expect("checked");
        let (models, materials, textures) = r.uploaded();
        println!(
            "gpu: {models} models, {materials} materials, {textures} textures and {} ground chunks uploaded in {:.2} s",
            r.ground_chunks().0,
            at.elapsed().as_secs_f32()
        );
        // What the graphics setting makes of the disc's fog (V32): the original's is its fog and far
        // clip as the original works them out of the scene file (V33), the whole map's a haze that
        // never hides it.
        let shown = args.graphics.unwrap_or_default();
        r.set_graphics(shown);
        // Texture Filtering (V83), before the first frame: the assets are uploaded already, and
        // every sampler and bind group is made again under it.
        let filtering = args.filtering.unwrap_or_default();
        r.set_anisotropy(filtering.anisotropy());
        let (aniso, most) = r.anisotropy();
        println!("texture filtering: {} (anisotropy {aniso}, this device {most} at most)", filtering.label());
        // Shadows and Shadow Quality (V108, user 2026-10-30), from the first frame.
        let (shadows, quality) = game_render::shadow::drawn(args.shadows.unwrap_or_default(), args.shadow_quality.unwrap_or_default(), true);
        r.set_shadows(shadows, quality);
        let tint = r.shadow_tint();
        let [tr, tg, tb] = tint.rgb;
        println!("shadows: {}, quality {} (the original's in {tr},{tg},{tb} at {:.3})", shadows.word(), quality.word(), tint.alpha);
        // Model Detail (P223), from the first frame.
        let detail = args.detail.unwrap_or_default();
        r.lod_bias = detail.lod_bias();
        println!("model detail: {}", detail.word());
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
    } else if args.graphics.is_some_and(|g| g != Graphics::Original) {
        println!("graphics: the software renderer draws the original's fog and far clip only");
    }
    // The software renderer draws no shadows (game_render::shadow::drawn): Off, the setting kept.
    if !(gpu_on && renderer_of(&mut offscreen, window).is_some()) && args.shadows.is_some_and(|s| s != game_render::shadow::Shadows::Off) {
        println!("shadows: the software renderer draws none");
    }
    // The load is over (P127): the ride is on the screen from its first frame.
    if let Some(f) = front.as_deref_mut()
        && f.menu.loading.take().is_some()
    {
        let seconds = load_screen.since.map_or(0.0, |t| t.elapsed().as_secs_f32());
        println!("loading: done, {} frames drawn over {seconds:.2} s", load_screen.frames);
    }
    // The GPU's per-frame lists, kept to reuse their room: the poses, the overlay and its layers.
    let mut gpu_poses: Vec<Vec<M4>> = Vec::new();
    // The software path's own copy of each livery's plane for each seat, by name: what its moving
    // parts are skinned into (V38). Grows as seats join.
    let mut plane_copies: Vec<Vec<String>> = Vec::new();
    // Each toy's model box, for whether a frame drew it (V76): only the toys that step on nothing
    // else want one.
    let toy_bounds: Vec<Option<(V3, V3)>> = animated
        .models
        .iter()
        .zip(animated.sim.specs.iter())
        .map(|(model, spec)| match spec.unseen {
            true => None,
            false => assets.models.get(model).and_then(|loaded| game_render::animated_toy::bound(&loaded.tris)),
        })
        .collect();
    // The software path's own copy of each animated toy's model (V43), which its pose is skinned into:
    // one model can be on many toys at many moments (nat06's nineteen windmills).
    let toy_copies: Vec<String> = animated.models.iter().enumerate().map(|(k, m)| format!("{m}@toy{k}")).collect();
    // The path cars' wheels (V82), each car's or `None` where its model has no `TireFrontL`: turned
    // render-side by the distance the car has run, so the simulation's cars carry nothing for them.
    let traffic_wheels: Vec<Option<game_render::toy::Wheels>> = traffic
        .cars
        .iter()
        .map(|(car, path)| game_render::toy::Wheels::new(&assets.models.get(car.stem())?.model, car, path))
        .collect();
    // Each drawn car's palette with its wheels turned, and its wheels, beside `traffic_at`.
    let mut traffic_pose: Vec<Option<(Vec<M4>, game_render::toy::Wheels)>> = Vec::new();
    // The renderer's CPU-drawn patches (P44): none since the debug HUD went (P170).
    let gpu_patches: Vec<game_render::gpu::Patch> = Vec::new();
    let mut gpu_sprites = Vec::new();
    // The render scale (V30): as asked, or picked from the frames' time against the display's refresh.
    let refresh = window.as_mut().and_then(|w| w.gpu()).and_then(|w| w.refresh).unwrap_or(60.0);
    let mut auto_scale = game_render::gpu::AutoScale::new(Duration::from_secs_f32(0.8 / refresh));
    // When the last window draw began and how long it waited on the surface: the frame since, less
    // that, is its CPU part, for where the GPU cannot time itself (V122).
    let mut scale_frame_at: Option<(Instant, Duration)> = None;
    // The settings' row changes it mid-ride (V34), unless `--render-scale` was given.
    let mut render_scale = args.render_scale;
    // The Auto preset (P223): the ride's first seconds timed against the display's rate (the page's
    // measured one in a browser) and the Graphics rows stepped down until it holds. Only in a window,
    // under the front end, with no graphics flag given.
    #[cfg(target_arch = "wasm32")]
    let tune_target = menu::video::target(crate::host_refresh());
    #[cfg(not(target_arch = "wasm32"))]
    let tune_target = menu::video::target(Some(refresh));
    let mut tune = menu::video::Tune::default();
    let tuning = offscreen.is_none() && window.is_some() && !args.video_asked;
    let mut tune_at: Option<Instant> = None;
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

    // ---- the loop ------------------------------------------------------------------------------
    let mut light = Lighting::of(&look);
    // A world's views' own lights (P142), each off its own camera; empty on a disc map, whose views
    // all take `light`. `light` is the first view's.
    let mut lights: Vec<Lighting> = Vec::new();
    // Which tile's sky and which tile's sun each view of a world's ride shows (MD9, P142, V116), the
    // sky `None` while it shows the whole world's one; `None` before its first frame.
    let mut shown_look: Vec<Option<(Option<usize>, Option<usize>)>> = Vec::new();
    // Each view's pair of skies bound for the crossfade (MD26), `Some(None)` while the whole world's
    // one is.
    let mut shown_skies: Vec<Option<Option<[usize; 2]>>> = Vec::new();
    if let Some(w) = &world_mod {
        println!("world sky: {} (the whole world's from {})", args.world_sky.unwrap_or_default().word(), w.pieces[w.home].tile.name);
    }
    // The frame: `--size`'s, which stays put whatever the window does, or the window's own shape
    // — the largest 4:3 one inside it with `--aspect 4:3` — which is re-made whenever the window
    // changes size (below). With no window, the PS2's own unless `--size` says otherwise.
    // The shape is the settings' Picture Shape's unless `--aspect` was given, and changes with it
    // from the pause (P126).
    let mut aspect = args.aspect;
    let frame_size = |window: &Option<Screen>, aspect: Aspect| match (args.size, window) {
        (Some(size), _) => size,
        // Never narrower than a split's views (P271): a window that reports no size (one minimised,
        // or `--headless` with no display) made a 1x1 frame, and a player joining split it into
        // views no pixel wide, which the sky and the rasteriser cannot draw.
        (None, Some(window)) => {
            let (w, h) = aspect.frame((window.width, window.height), window.scale);
            (w.max(split::SMALLEST_FRAME), h.max(split::SMALLEST_FRAME))
        }
        (None, None) => PS2_FRAME,
    };
    let (width, height) = frame_size(&window, aspect);
    // Which of the original's two displays the frame stands for (V24): at 16:9 its cameras read the
    // wide copies of their presets, as the original's loader does in its widescreen modes.
    let mut display = Display::of(width, height, aspect);
    match &window {
        Some(window) => println!(
            "window {}x{} (screen {}x{}), drawing {width}x{height} at scale {}, --aspect {}, a {} display",
            window.width,
            window.height,
            window.screen.0,
            window.screen.1,
            window.scale,
            aspect.name(),
            display.name()
        ),
        None => println!("headless, drawing {width}x{height}, a {} display", display.name()),
    }
    if args.size.is_none() {
        println!("picture shape: {}", aspect.name());
    }
    if display != Display::Television {
        set_display(&mut rigs, layout, display, cpresets.as_deref(), plane_presets.as_deref());
        set_heli_display(&mut rigs, layout, display, heli_presets.as_deref());
        set_car_display(&mut rigs, layout, display, &car_presets);
    }
    // The original's HUD (V26): its art off the disc, the world its radar covers, and which overlay is
    // up. Art that does not load stops the ride with the reason (V28) — it used to say so on stderr
    // and ride on with no HUD, which is how a HUD went missing with nobody told why.
    let mut hud_art = overlay::Art::load(&mut disc).map_err(|e| format!("the original's HUD: {e}"))?;
    // Each rider's name and number on it (P86), in seat order.
    hud_art.tags = std::iter::once(&args.look)
        .chain(args.others.iter().map(|p| &p.look))
        .map(|l| overlay::Tag { name: l.name.clone(), number: l.number.clone() })
        .collect();
    // The GPU draws it as sprites from its pictures, uploaded once (V30).
    // The front end's after them, for the pause (V30), which is why the HUD's are numbered from 0.
    // The ride HUD's two atlases next (P121), then the front end's.
    let hud_base = hud_art.images().len();
    let menu_base = hud_base + ride_hud::images().len();
    if gpu_on && let Some(r) = renderer_of(&mut offscreen, window) {
        let mut images = hud_art.images();
        images.extend(ride_hud::images());
        if let Some(front) = &front {
            images.extend(front.art.images());
        }
        r.set_images(&images);
    }
    // The radar's world is the scene's terrain scales' (MD3): the original reads the larger of the
    // two the map edge measures with, which on the disc is the terrain file's own spacing.
    let radar_span = overlay::radar_span(place.terrain.width_scale.0.max(place.terrain.breadth_scale.0));
    // The maps the radar names (MD42): a world's tiles. A lone track, a disc one or a mod, names
    // none under the rider's arrow (P234).
    let map_names = world_mod.as_ref().map(|w| w.map_names()).unwrap_or_default();
    if !map_names.maps.is_empty() {
        let names: Vec<&str> = map_names.maps.iter().map(|(_, n)| n.as_str()).collect();
        println!("radar maps: {}: {}", names.len(), names.join(", "));
    }
    // And on a world that wraps (MD43), across its seams.
    let hud_wrap = match &sim {
        Land::Tiles(t) => drawn_wrap(t),
        _ => None,
    };
    // A world's edge is its whole rectangle's (MD8), and so is the radar's ground.
    let edge = sim_world::World::area(&sim).unwrap_or_else(|| Area::new(
        F(place.terrain.width as f32),
        F(place.terrain.breadth as f32),
        place.terrain.width_scale,
        place.terrain.breadth_scale,
    ));
    println!(
        "map: {:.0} x {:.0} feet to its edge, shot back from {:.0} feet short of it; radar {:.0} feet across",
        edge.width.0,
        edge.depth.0,
        game_sim::off_map::MARGIN.0,
        radar_span
    );
    // The activities' start zones the radar marks (P92): the track's `hit.dat`, in its second
    // archive. A track with none (the MX tracks) marks none.
    let radar_zones = archives
        .iter()
        .find_map(|a| a.read_suffix(&format!("{stem}\\hit.dat")).ok())
        .map(|b| overlay::zones(&String::from_utf8_lossy(&b)))
        .unwrap_or_default();
    // And drawn in the world (V51) as the original draws them, the session markers as small ones:
    // the same file's zones, and the portal's art out of the same archive — or, on a track whose
    // archive lacks the numbers (the MX tracks'), a free-ride track's, which has them all.
    let zone_spots = archives
        .iter()
        .find_map(|a| a.read_suffix(&format!("{stem}\\hit.dat")).ok())
        .map(|b| portal::spots(&String::from_utf8_lossy(&b)))
        .unwrap_or_default();
    let portal_art = {
        let read = |archives: &[Res], name: &str| archives.iter().find_map(|a| a.read_suffix(name).ok());
        let own = portal::Art::load(|name| read(&archives, name)).filter(portal::Art::numbered);
        own.or_else(|| {
            let free = [disc.res(&member_ending(&disc, portal::ART_ARCHIVE).ok()?).ok()?];
            portal::Art::load(|name| read(&free, name))
        })
    };
    // The bike's tire tracks (V40): the track's `TireTrack` entries and their tread, drawn as glows
    // after the portals' pictures, one set of strips a seat.
    let tread = load_tire_tracks(&archives);
    // The cars' (S269): each car's list's four, on the same tread. A seat keeps one set a car, so the
    // marks a car left stay when the seat changes machine.
    let car_tread: Vec<(Machine, Vec<tire_tracks::Params>)> = load_car_tire_tracks(&archives, &garage.cars);
    let mut car_marks: Vec<Vec<tire_tracks::Tracks>> = Vec::new();
    let mut glow_list: Vec<game_render::pst::Texture> = portal_art
        .as_ref()
        .map(|a| a.textures.clone())
        .unwrap_or_default()
        .into_iter()
        .chain(tread.as_ref().map(|(_, t)| t.clone()))
        .collect();
    let tread_texture = tread.as_ref().map(|_| glow_list.len() - 1);
    let rain_texture = rain_art.map(|art| {
        glow_list.push(art);
        glow_list.len() - 1
    });
    // The sun's flares (V72): the `.toy`'s list and each one's picture, after the tread among the
    // glows' pictures. A world mod's tiles have none of their own.
    let sun_flares: Vec<(game_render::sun_flare::SunFlare, usize)> = match &world_mod {
        None => load_sun_flares(&archives, &stem)
            .into_iter()
            .map(|(flare, picture)| {
                glow_list.push(picture);
                (flare, glow_list.len() - 1)
            })
            .collect(),
        Some(_) => Vec::new(),
    };
    // Each view's share of each flare's sample rays that reach the sky, and the simulated frame it
    // was asked on.
    let mut flare_sky: std::collections::HashMap<(usize, usize), (u64, f32)> = std::collections::HashMap::new();
    let mut tire_marks: Vec<tire_tracks::Tracks> = Vec::new();
    // The GPU's copy of its pictures, uploaded with the first frame's glows.
    let mut glow_textures = (!glow_list.is_empty()).then(|| glow_list.clone());
    let mut overlay_mode = args.overlay.unwrap_or_default();
    let mut speedometer = args.speedometer.unwrap_or_default();
    let mut minimap = args.minimap.unwrap_or(true);
    let mut show_fps = args.show_fps.unwrap_or(false);
    let mut fps_meter = ride_hud::FpsMeter::default();
    let mut world_sky = args.world_sky.unwrap_or_default();
    let mut hud_scale = args.hud_scale.unwrap_or(menu::HUD_SCALE_DEFAULT);
    let mut graphics = args.graphics.unwrap_or_default();
    let mut plants = args.plants.unwrap_or_default();
    let mut foliage = args.foliage.unwrap_or_default();
    let mut filtering = args.filtering.unwrap_or_default();
    let mut shadows = (args.shadows.unwrap_or_default(), args.shadow_quality.unwrap_or_default());
    // Set when the pause's settings change the picture's shape: the frame is re-made as for a resize.
    let mut reshaped = false;
    let mut frame = Frame::new(width, height, look.fog_color);
    // The views (V22): where each player's goes in the frame, and a frame of its own size for each to
    // be drawn into. A full screen has none — its one view is drawn straight into the frame, as it
    // always has been.
    // Of the layout and the player count as they are now: a player joining or leaving (P76) lays
    // the views out again.
    // The fog's colour taken out of the scene's look, which the picture (in a closure since N23a)
    // borrows whole.
    let fog_color = look.fog_color;
    let views_of = |width: usize, height: usize, layout: Layout, players: usize| {
        let rects = layout.rects(width, height);
        let frames: Vec<Frame> = match layout {
            Layout::Full => Vec::new(),
            _ => rects.iter().take(players).map(|r| Frame::new(r.w, r.h, fog_color)).collect(),
        };
        (rects, frames)
    };
    let (mut rects, mut view_frames) = views_of(width, height, layout, args.players);
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
        riding: front.as_ref().is_none_or(|f| !f.menu.paused(0)),
        start_freezes: front.is_none(),
        waiting: false,
        paused: false,
        said: rider::Frame::default(),
        inverted: false,
        hands: Controls::default(),
        teleported: false,
        clears: 0,
        markers: args.markers_of(0, front.as_deref()),
        together: false,
        shade: PageShade::default(),
    }];
    for (i, (rider, phy)) in others.into_iter().zip(other_phys).enumerate() {
        // Every player has a plane of their own, parked in their bay of the lot (P87) or, on a map
        // that parks none for them, starting beside the start line (P113). A local seat starts on the
        // bike (P177) and switches from the pause or at a bay.
        // An online peer's seat (N16) starts as its own machine says, at tick 0 or as it is taken on.
        let plane = match plane_starts.get(i + 1) {
            Some(&start) => lot.plane_anywhere(i + 1, flight_for(i + 1), start),
            None => lot.plane(i + 1, flight_for(i + 1)),
        };
        // Where it stands, as the first player's is said (MD32: a world's in world feet).
        if let Some(p) = plane.as_ref().filter(|_| lot.bay(Machine::Biplane, i + 1).is_some()) {
            println!("player {}: biplane parked at {:.0}, {:.0}, {:.0}", i + 2, p.at.x(), p.at.y(), p.at.z());
        }
        let up = Kind::Bike;
        seats.push(Seat {
            reports: [Report::missing(); PADS],
            sim: Sim::with_rider(0, tables.clone(), phy, rider),
            garage: {
                let mut garage = Garage::new(up, plane)?;
                garage.cars = lot.cars_for(i + 1);
                garage.heli = lot.heli_for(i + 1);
                garage.modify(&lot, &mods, i + 1);
                garage
            },
            riding: front.as_ref().is_none_or(|f| !f.menu.paused(i + 1)),
            start_freezes: front.is_none(),
            waiting: false,
            paused: false,
            said: rider::Frame::default(),
            inverted: false,
            hands: Controls::default(),
            teleported: false,
            clears: 0,
            markers: args.markers_of(i + 1, front.as_deref()),
            together: false,
            shade: PageShade::default(),
        });
    }
    // Where each seat's machines start (P132), marker slot 1 while nothing is put there: every seat
    // shares the one list and picks its own out of it by seat.
    // The heading the riders were stood on, as the build worked it out: the scene's start direction,
    // or `--trace-at`'s degrees.
    let heading = match args.trace_at {
        None => {
            let f = point(place.start_direction).norm();
            Vf::new(f.x, f.y, f.z, 0.0)
        }
        Some((_, _, deg)) => {
            let h = deg.to_radians();
            Vf::new(h.sin(), 0.0, h.cos(), 0.0)
        }
    };
    // `--overview` (P166): the track pick's still looks over the map from above and behind the spawn.
    if args.overview && args.shot_pose.is_none() {
        let extent = match &sim {
            Land::Tiles(t) => (t.area.width.0, t.area.depth.0),
            _ => ground.extent(),
        };
        let framing = track_previews::overview([at.x(), at.y(), at.z()], [heading.x(), heading.z()], extent, |x, z| {
            let (on, _) = sim_world::World::ground(&mut sim, Vf::new(x, 1.0e4, z, 1.0));
            Some(on.y()).filter(|y| *y > -1.0e5)
        });
        println!("{}", track_previews::overview_line(&framing));
        args.shot_pose = Some(shot_pose::Pose { eye: framing.eye, yaw: framing.yaw, pitch: framing.pitch, fov: framing.fov });
    }
    let starts: Vec<Vf> = seats.iter().map(|s| s.sim.rider.ride.position()).collect();
    let bays: Vec<vehicle::Spawn> = (0..seats.len()).map_while(|p| lot.bay(Machine::Biplane, p).map(|b| b.spawn)).collect();
    let home = Arc::new(vehicle::home(&starts, heading, &bays));
    // Each player's start, which a player joining the ride (P76) adds theirs to and a leaver takes theirs out of.
    let mut join_starts = starts.clone();
    seats.iter_mut().for_each(|s| s.sim.home = Arc::clone(&home));
    // The path cars (S81) on every seat's state, from the track's load: each seat steps them before
    // its rider, so each rider meets them where its own state has them.
    seats.iter_mut().for_each(|s| s.sim.traffic = cars.clone());
    // The freestyle event (P155) on every seat's state: the track's stunt zones and target.
    let hit_dat = archives.iter().find_map(|a| a.read_suffix(&format!("{stem}\\hit.dat")).ok()).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let stunt = portal::stunt_event(&zone_spots, &hit_dat);
    if !stunt.zones.is_empty() {
        println!("freestyle event: {} stunt zones, target {}", stunt.zones.len(), stunt.target);
    }
    seats.iter_mut().for_each(|s| s.sim.event = stunt.clone());
    let mut world = World { place: sim, seats, tape: recorder, played_dt: None };
    // P298: a resumed ride's first seat put where it was, as a resync puts a host's state over a
    // joiner's, under the pause it went away with.
    if let Some(state) = front.as_deref_mut().and_then(|f| f.resume.state.take()) {
        match net::rides::from_seat_bytes(&mut world.seats[0].sim, &state) {
            Ok(()) if world.seats.len() == 1 => {
                println!("resume: the ride is where it was, tick {}", world.seats[0].sim.ticks);
                if let Some(f) = front.as_deref_mut() {
                    f.menu.pause();
                }
            }
            Ok(()) => println!("resume: the ride is where it was (seat 1 of {})", world.seats.len()),
            Err(e) => println!("resume: the ride's state: {e}; riding from the start"),
        }
    }
    // `--fly-at` (V96): the first seat's plane flying high over the world from the first tick, at
    // the biplane's cruise.
    if let Some([x, z, deg, ft]) = args.fly_at {
        let (on, _) = sim_world::World::ground(&mut world.place, Vf::new(x, 1.0e4, z, 1.0));
        let h = deg.to_radians();
        let seat = &mut world.seats[0];
        let tables = Arc::clone(&seat.sim.tables);
        seat.garage.launch_plane(Vf::new(x, on.y().max(0.0) + ft, z, 1.0), Vf::new(h.sin(), 0.0, h.cos(), 0.0), 120.0, &tables.ride.acos);
    }
    // A played tape rides Air Wrecks as it was made (P138): on, the original's, until the tape says
    // otherwise, whatever the Settings screen or the command line say.
    if head.is_some() {
        world.seats[0].sim.rider.sense.air_wrecks = true;
        // and Backwards Landings Bail off, the original's (S161)
        world.seats[0].sim.rider.sense.backwards_bail = false;
    }
    // The start zones' portals (V51), laid over the ground once: they never move.
    let zone_portals: Vec<portal::Portal> = portal::shown(&zone_spots, false)
        .iter()
        .map(|spot| portal::Portal::zone(spot, |x, z| portal_ground(&mut world.place, x, spot.at[1] + 50.0, z)))
        .collect();
    let mut marked_portals: Vec<Vec<Option<portal::Portal>>> = vec![Vec::new(); world.seats.len()];

    for (seat, rig) in world.seats.iter().zip(&mut rigs) {
        let first = rig.follow(seat.garage.up);
        if !first.cam.presets.is_empty() {
            let name = first.cam.preset().name.clone();
            rig.ride_hud.note_camera(Instant::now(), &name);
        }
    }
    println!("vehicle: {}", world.seats[0].garage.up.name());
    // The pause's switch row reads which machine each player is on and whether they have another
    // (P41), and any player's Start opens the pause for them: 1P's through the front end's reader,
    // the others' through one each here, which only ever look for Start while the ride runs.
    if let Some(front) = front.as_deref_mut() {
        // The local players: online the other machines' seats are drawn beside them (N16).
        front.menu.riders = args.players.min(world.seats.len());
        front.menu.modified = !mods.is_original();
        front.menu.up = [menu::Machine::Bike; menu::SEATS_MAX];
        front.menu.switchable = [false; menu::SEATS_MAX];
        for (i, seat) in world.seats.iter().enumerate().take(menu::SEATS_MAX) {
            front.menu.up[i] = menu::Machine::from_word(seat.garage.up.name()).unwrap_or_default();
            front.menu.switchable[i] = seat.garage.plane.is_some();
            for (k, kind) in [Kind::Buggy, Kind::Monster, Kind::Baja].into_iter().enumerate() {
                front.menu.cars[i][k] = seat.garage.machines().contains(&kind);
            }
            front.menu.heli[i] = seat.garage.heli.is_some();
        }
    }
    let mut start_readers: Vec<Reader> = (1..world.seats.len()).map(|_| Reader::at(RIDE_HZ)).collect();
    // Every pad seat's presses (P76), read each frame so that a controller with no player in the
    // ride joins it on Cross, and one that just left does not join again on the press that left.
    let mut idle_readers: Vec<Reader> = (0..pad::SEATS).map(|_| Reader::at(RIDE_HZ)).collect();
    // And the keyboard's while it drives no seat (1P a pad), which joins on a free pad seat (P173).
    let mut keys_reader = Reader::at(RIDE_HZ);
    // Every device's own presses while 1P's pause has the Controllers screen up (P210).
    let mut assign_readers = Readers::new();
    // Software models copied for joiners, each under a name of its own.
    let mut joined_models = 0usize;
    // H toggles it, on its own edge: the window keeps key state and not key events, so the shell
    // remembers last frame's to tell a press from a hold.
    // M swaps machines, on its edge the same way (P19).
    let mut swap_key = false;
    // Each seat's Circle as the last frame had it, for the bays' pop-up (P103).
    let mut answers: Vec<vehicle::Answer> = Vec::new();
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
    // The lobby's seats that start in the biplane (N18), each its pick's machine.
    let mut lobby_planes: Option<Vec<bool>> = None;
    let mut online = match &args.online {
        // Into a ride under way (N10): every seat is the host's first's bike, on this machine's
        // constants for it, and the ride's state comes from the host — this machine's seat in the
        // plane if its setup picked it (N19), which its pick tells the host.
        None if from_lobby.is_some() && drop_in => {
            let ((room, me), net) = from_lobby.expect("checked");
            if head.is_some() || world.tape.is_some() || world.seats.len() != 1 {
                return Err("dropping in on an online ride rides one player and no tape: drop --play, --record and --players".into());
            }
            let disc = &front.as_deref().expect("from the lobby").network.disc;
            let first = room.seats.iter().flatten().next().ok_or("the ride has nobody in it")?;
            let bytes = disc.phy(&first.pick.bike).ok_or(format!("the disc has no {} bike set", first.pick.bike))?;
            // on the room's handling (N21), as the host's machine reads it
            let handling_pack = args.room_handling.as_ref().map(|(dir, id)| trick_packs::by_id(dir, id)).transpose()?;
            let mut stock = room_handling_phy(handling_pack.as_ref(), &first.pick.bike, &tables.ride.curve)
                .unwrap_or_else(|| Phy::read_bytes(&bytes, &tables.ride.curve));
            // the room's trick packs over it (MD19, MD77), as the host's machine has
            let packs = args.room_tricks.as_ref().map(|(dir, ids)| trick_packs::on(dir, &trick_packs::ids_of_room(ids))).transpose()?.unwrap_or_default();
            lay_room_tricks(&packs, &mut stock, &first.pick.bike);
            let mut phy = stock.clone();
            mods.bike(0, &mut phy);
            let mut sim = world.seats[0].sim.clone();
            sim.phy = Arc::new(phy);
            sim.rng = room.seed;
            // its saved markers came with its handshake on the ride's bike, and are kept on it (P105)
            bike_words[0] = first.pick.bike.clone();
            let mut online = online::Online::drop_in(net, &room, &me, sim)?;
            online.modifiers(mods, vec![stock]);
            Some(online)
        }
        None if from_lobby.is_some() => {
            let ((room, me), net) = from_lobby.expect("checked");
            // Each seat's machine is its pick's (N18), started at tick 0 below, and the local
            // player's the setup's, which this machine's pick says.
            if head.is_some() || world.tape.is_some() {
                return Err("an online ride rides no tape: drop --play and --record".into());
            }
            if args.players != 1 {
                return Err("an online ride from the lobby has one local player: drop --players".into());
            }
            let disc = &front.as_deref().expect("from the lobby").network.disc;
            // the room's trick packs (MD19, MD77), laid over every seat's bike as on every machine
            let packs = args.room_tricks.as_ref().map(|(dir, ids)| trick_packs::on(dir, &trick_packs::ids_of_room(ids))).transpose()?.unwrap_or_default();
            // and the room's handling (N21), every seat's bike read from its pack
            let handling_pack = args.room_handling.as_ref().map(|(dir, id)| trick_packs::by_id(dir, id)).transpose()?;
            let mut sims = Vec::with_capacity(room.seats.len());
            // each seat's bike file before the modifiers, which a change of them mid-ride is built
            // on again (P100); an open seat rides this machine's first's
            let mut stock = Vec::with_capacity(room.seats.len());
            for (i, seat) in room.seats.iter().enumerate() {
                let mut sim = world.seats[0].sim.clone();
                stock.push(stock_phys[0].clone());
                if let Some(seat) = seat {
                    let bytes = disc.phy(&seat.pick.bike).ok_or(format!("the disc has no {} bike set", seat.pick.bike))?;
                    // The room's modifiers, a row a seat (P134): each seat's player's own, which
                    // the host gathered from the picks, the world's the host's.
                    // on the room's handling (N21), then its trick pack, as on every machine
                    let mut phy = room_handling_phy(handling_pack.as_ref(), &seat.pick.bike, &tables.ride.curve)
                        .unwrap_or_else(|| Phy::read_bytes(&bytes, &tables.ride.curve));
                    lay_room_tricks(&packs, &mut phy, &seat.pick.bike);
                    *stock.last_mut().expect("pushed") = phy.clone();
                    mods.bike(i, &mut phy);
                    sim.phy = Arc::new(phy);
                }
                // Each seat's bike through its player's controller preset (N13), as the room says
                // it and the handshake held every machine to; an open seat's is 0.
                sim.set_presets([seat.as_ref().map_or(0, |s| s.pick.preset as usize), 0]);
                sim.rng = room.seed;
                // Each player's saved markers on this track (P99), as their pick brought them to every
                // machine: every seat starts the ride with the same slots everywhere. An open seat's
                // are none.
                sim.markers = seat.as_ref().map_or_else(Default::default, |s| saved_markers::from_pick(&s.pick.markers, &s.pick.bike, &room.track));
                sims.push(sim);
            }
            let mine = room.seats.iter().flatten().find(|s| s.peer == me).map_or(0, |s| s.pick.preset as usize);
            let here = pad_seats.as_ref().map_or(presets[0], |s| s.preset(args.pad_seat(0)));
            if mine != here {
                println!("online: this machine's seat rides controller preset {mine}, its lobby pick's, not {here}");
            }
            let mut online = online::Online::lobby(net, &room, &me, sims)?;
            online.modifiers(mods, stock);
            lobby_planes = Some(online::Online::picked_planes(&room));
            Some(online)
        }
        None => None,
        Some(me) => {
            // `--vehicle biplane` starts this machine's seat in the plane at tick 0 on every
            // machine (N18), which its datagrams say.
            if head.is_some() || world.tape.is_some() {
                return Err("--online rides no tape: drop --play and --record".into());
            }
            // One local player and no front end: the session's seats are `--seats`, and the pause
            // menu would freeze a ride the other machines go on with.
            if args.players != 1 || front.is_some() {
                return Err("--online rides one local player outside the front end: drop --players and --menu".into());
            }
            let starts: Vec<Sim> = world.seats.iter().map(|s| s.sim.clone()).collect();
            // This machine's controller preset (P79) goes ahead of its every datagram, and every
            // seat rides its own machine's (N15), as from the lobby (N13).
            let here = pad_seats.as_ref().map_or(presets[0], |s| s.preset(args.pad_seat(0)));
            let mut online = online::Online::start(me, &args.peers, args.seats.as_deref(), &starts, here as u8, world.seats[0].garage.up == Kind::Biplane)?;
            online.modifiers(mods, stock_phys.clone());
            // this machine's player's saved markers on the track (P105), which every machine's
            // seat for it starts on: they go to every peer before the first tick
            let dir = menu::store_dir(Disc::configured_path().ok().as_deref());
            let profile = saved_markers::profile(&roster::clean_rider_name(&args.look.name), 0);
            let mine = saved_markers::load(&dir, &args.track, &profile, &bike_words[0]);
            online.saved_markers(&mine, &args.track, &bike_words[0]);
            Some(online)
        }
    };
    // The ride's biplane, which a seat's switch online takes a copy of (N17), on the ride's
    // modifiers; and the machine this machine's player last picked online, which its pad carries.
    let plane_like = world.seats[0].garage.plane.as_ref().and_then(|p| p.flight.clone());
    if let Some(on) = online.as_mut() {
        on.planes(stock_plane, plane_like.as_ref());
        // Each seat that starts in the plane stands in its own bay at tick 0 (N18): the lobby's
        // now, from the picks; `--peer`s' once every peer's card has said its own.
        on.bays_on(online_bays.clone(), online_bodies.clone());
        // And every seat's cars (S249), which its switch to a car takes.
        on.cars_on(lot.online_cars.clone());
        if let Some(flies) = &lobby_planes {
            on.start_planes(flies)?;
        }
    }
    let mut online_want: Option<Kind> = None;
    let mut online_frames = 0u32;
    // How many of `--online-script`'s words have gone (K29).
    let mut online_scripted = 0usize;
    let mut online_gas = false;
    // In the browser (P204): the last ten seconds of the ride settled here that the page's console
    // was told of (`Online::settled_line`).
    #[cfg(target_arch = "wasm32")]
    let mut online_told = 0usize;
    // The online peers' rigs dressed mid-ride (N16), each under model names of its own, and the
    // first player's pick, which a peer whose pick does not say is drawn as.
    let mut peers_dressed = 0usize;
    // Online (N20): the pad seats whose players asked the session for a seat, in the order they
    // asked, and this machine's seats as the views were last laid out for them.
    let mut online_sits: Vec<usize> = Vec::new();
    let mut online_mine: Vec<usize> = online.as_ref().map(online::Online::my_seats).unwrap_or_default();
    let own = own_pick(&args);
    // The session markers kept between rides (P99), in `markers/` beside `settings.txt`. A played
    // tape starts its first player on the markers it was made with and keeps nothing — its ride is
    // the tape's, not the player's; online the seats' came with the lobby's picks, a drop-in's handshake
    // or the `--peer`s' pieces (P105) and this machine's seat is kept as it is placed; otherwise every local player starts on the ones they kept on
    // this track, and keeps what they place.
    let markers_dir = front.as_deref().map_or_else(|| menu::store_dir(Disc::configured_path().ok().as_deref()), |f| f.dir.clone());
    let rider_names: Vec<String> = std::iter::once(&args.look).chain(args.others.iter().map(|p| &p.look)).map(|l| roster::clean_rider_name(&l.name)).collect();
    let profile_of = |i: usize| saved_markers::profile(rider_names.get(i).map_or("", String::as_str), i);
    let mut kept_markers: Vec<Option<saved_markers::Kept>> = vec![None; world.seats.len()];
    if let Some(head) = &head {
        if let Some(m) = head.tape.start_markers().map_err(|e| format!("the tape's markers: {e}"))? {
            println!("markers: the tape's, {} placed", m.placed());
            world.seats[0].sim.markers = m;
        }
    } else if online.is_some() {
        let first = &world.seats[0].sim.markers;
        kept_markers[0] = Some(saved_markers::Kept::new(&args.track, &profile_of(0), &bike_words[0], first));
    } else {
        for (i, seat) in world.seats.iter_mut().enumerate() {
            let bike = bike_words.get(i).unwrap_or(&bike_words[0]);
            seat.sim.markers = saved_markers::load(&markers_dir, &args.track, &profile_of(i), bike);
            let placed = seat.sim.markers.placed();
            if placed > 0 {
                println!("markers: {} on {}: {placed} kept", profile_of(i), args.track);
            }
            kept_markers[i] = Some(saved_markers::Kept::new(&args.track, &profile_of(i), bike, &seat.sim.markers));
        }
    }
    // Each seat shows the set of the machine it starts on (P107): a ride may start in the plane.
    for seat in &mut world.seats {
        let up = if seat.sim.plane.is_some() { Kind::Biplane } else { seat.garage.up };
        seat.sim.markers.up(vehicle::markers_of(up));
    }
    // A tape carries the markers its first player started with, every machine's, and replays on them.
    if let Some(tape) = &mut world.tape
        && world.seats[0].sim.markers.any()
    {
        tape.tape.markers = world.seats[0].sim.markers.to_files(&saved_markers::names(&bike_words[0]));
    }
    // Whether the ride opens to online players once it is down to one local player again (P76).
    let mut opens_alone = false;
    // The pause offers "Open to Online" (N10) on a local freeride of one player on the bike set the
    // disc's own lookup gives (what a joiner's machine loads for it), outside any lobby; on a disc
    // track or a track mod the lobby finds as this ride did (F6), not a world mod.
    if let Some(front) = front.as_deref_mut() {
        let set = args.set.unwrap_or(roster::Set::FREERIDE).word();
        let alone = online.is_none()
            && head.is_none()
            && world.tape.is_none()
            && args.phy.is_none()
            && opens_on_mod(modded.as_ref(), world_mod.is_some(), front.menu.settings.mods.then(|| front.dir.join(track_mods::MODS_DIR)).as_deref(), &args.track)
            && front.menu.lobby.room.is_none()
            && front.network.disc.phy(&set).is_some_and(|b| *b == phy_bytes);
        opens_alone = alone;
        let can = alone && world.seats.len() == 1;
        front.menu.opening = if can { menu::Opening::Can } else { menu::Opening::No };
        front.menu.mods_lock = mods_lock(head.is_some(), online.as_ref().map(|on| (on.host(), on.own_rows())));
        front.menu.guest = online.as_ref().is_some_and(|on| !on.host());
        front.menu.air_lock = air_lock(head.is_some(), online.is_some());
        // Local players join and leave a local freeride as it runs (P76), and online through the
        // session's join and leave (N20) once this machine is in a lobby's ride; a tape is its first
        // player's ride.
        front.menu.seats_open = online.as_ref().is_none_or(online::Online::can_sit) && head.is_none() && world.tape.is_none();
        front.menu.leave_to = online.as_ref().map_or(menu::LeaveTo::Main, online::Online::leaves_to);
        front.menu.session = online.is_some();
    }
    // The modifiers the Settings screen last said, so that the pause closing on a set the player
    // changed puts it on the ride (P98) and a `--modifiers` ride the player did not touch keeps its own.
    let mut menu_mods = front.as_deref().map(|f| f.menu.settings.modifiers);
    // And the Air Wrecks switch it last said (P138), taken the same way at the pause's close.
    let mut menu_air = front.as_deref().map(|f| f.menu.settings.air_wrecks);
    // And Backwards Landings Bail's (S161).
    let mut menu_back = front.as_deref().map(|f| f.menu.settings.backwards_bail);
    // Each seat's wreck rumble (P183): the plane's crash word last frame, to see it go up.
    let mut wreck_rumbles: Vec<pad::WreckRumble> = Vec::new();
    // Which shell seats have a rider in them online, for the picture: an empty seat is not drawn.
    // `--overview` (P166) draws nobody's.
    let mut seated = vec![!args.overview; world.seats.len()];
    // What the online ride's riders said between drawn frames (P184), for the sound and the lines.
    let mut online_said = audio::Said::default();
    // Whether the ride says how to join it (P171): a seat free and a controller idle.
    let mut join_cue = false;

    let mut shot = args.shot.clone();
    let started = Instant::now();
    // The ride's clock: the wall's, or `--steady-clock`'s, which only the pictures move.
    let mut clock = if args.steady_clock { pace::Clock::steady(started) } else { pace::Clock::wall(started) };
    let mut window_of_frames = (Instant::now(), 0u32, 0usize);
    // Seat 0's rider on the GPU over the line's second (P212a): frames its bike drew in, and those
    // of them its rider drew no triangle in.
    let mut rider_frames = (0u32, 0u32);
    // Each view's triangles over the session, for the line a split session ends on.
    let mut view_triangles = vec![0usize; args.players];
    // The GPU ground's quadtree nodes looked at (V65): over the session, the most in a frame, frames.
    let mut ground_visits = (0usize, 0usize, 0usize);
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
    // `--online`'s `--frames` counts the session's ticks, below, and not the loop's frames (K10).
    let mut pacer = pace::Pacer::new(pace::TICK, started).most(args.frames.filter(|_| args.online.is_none())).until(args.seconds);
    // What the tick before the last one showed, for the picture to blend from: each seat's pose,
    // each view's camera and the plane.
    let mut before: Vec<Shown> = Vec::new();
    let mut before_cameras: Vec<Camera> = Vec::new();
    // Each view's markers' slot display (P88).
    let mut banners: Vec<hud::MarkerBanner> = Vec::new();
    let mut before_planes: Vec<Option<M4>> = Vec::new();
    // And each seat's flown helicopter, with its rotor (P216b).
    let mut before_helis: Vec<Option<(M4, Rotor)>> = Vec::new();
    // And each seat's driven car's (S245).
    let mut before_cars: Vec<Option<M4>> = Vec::new();
    // Each plane's moving parts before the tick, which the picture blends them from (V79).
    let mut before_surfaces: Vec<Option<game_sim::biplane_anim::Surfaces>> = Vec::new();
    // Each seat's cut test (P120): whether the last tick's picture is a jump from the one before.
    let mut cuts: Vec<pace::Cut> = Vec::new();
    // Each seat's camera over its own pause (P172).
    let mut pause_holds: Vec<chase::PauseHold> = Vec::new();
    // The moment the picture is of (P120): read once a pass of the loop, after the last picture was
    // shown, and both the ticks owed and the blend are taken at it. Read again after the ticks, the
    // blend came a tick's cost later on the frames that ran one than on those that did not, though
    // each went up at the display's next refresh, and the faster the ride the more that showed.
    let mut moment = clock.now();
    // What the last tick left for the picture: whether it simulated, and each player's ride for the
    // camera and the overlay.
    let mut simulated = false;
    // Whether the ride's first picture has been drawn behind the loading screen (P150).
    let mut warmed = !args.warm;
    // The load's time and memory said once, with the ride's first picture (P283).
    let mut load_said = false;
    let mut ridden: Vec<Ridden> = Vec::with_capacity(args.players);
    let mut top = Instant::now();
    let mut limiter = pace::Limiter::default();
    // The settings a ride without the front end paces its picture by (P90).
    let ride_settings = match (&front, &window) {
        (None, Some(_)) => menu::load(&menu::store_dir(Disc::configured_path().ok().as_deref())).0,
        _ => menu::Settings::default(),
    };
    // A world's shapes and the near sets around each rider cut now, behind the loading screen,
    // not in tick 1 (P150).
    let near_at = Instant::now();
    let starts: Vec<Vf> = world.seats.iter().map(|s| s.sim.rider.ride.position()).collect();
    if let Some(t) = world.place.tiled().filter(|_| args.warm) {
        for at in starts {
            t.prepare_near(at);
        }
        println!("world shapes: near sets cut in {:.2} ms", near_at.elapsed().as_secs_f64() * 1000.0);
        stages.skip();
    }
    // The near sets cut so far, and whether the ride's first frame has said what it built (K43).
    let near_cut_at_load = world.place.tiled().map_or(0, |t| t.near_cuts());
    let mut first_said = false;
    perf.begin();
    // Whether the last time round ran a tick and drew nothing (P205).
    let mut ticked = false;
    // The sim thread (N23a), the tick on it if one is, and what the ticks and the pictures have
    // been taking, which `--sim-thread auto` reads.
    let sim_worker = match args.sim_thread {
        sim_thread::Mode::Off => None,
        sim_thread::Mode::Auto if window.is_none() => None,
        _ if online.is_some() || drawn_land.is_none() => None,
        _ => sim_thread::Worker::new(),
    };
    let mut flight = None;
    let mut gauge = sim_thread::Gauge::default();
    loop {
        // The picture before this one is the page's until its next frame (P196). A tick is not a
        // picture: waiting for the page's frame after one as well spent a display frame on every
        // tick, which held a 60 Hz page to 30 pictures a second and a slower one to fewer still.
        if !std::mem::take(&mut ticked) {
            pace::shown().await;
        }
        // A replacement changed while riding (V86): its textures' pixels swapped where the software
        // renderer reads them, and uploaded again for the GPU's.
        for (key, texture) in texture_reload.as_mut().map(textures::Reload::poll).unwrap_or_default() {
            let at = assets.replace_keyed(&key, &texture);
            if let Some(r) = renderer_of(&mut offscreen, window) {
                r.reload_textures(&assets, &at);
            }
        }
        // A tick that ran on the sim thread while the picture before it was drawn (N23a) lands here,
        // its first half run before it went: only the simulation and what comes after it are left.
        if flight.is_some() || pacer.owed(moment) {
        let landing = flight.take();
        // Whether a player joined or left the ride this tick (P76), which keeps the tick off the sim
        // thread: its second half lays out the views' riders and cameras for the new count, and a
        // picture drawn beside it would draw the new views from the old tick's (P271).
        let mut reseated = false;
        if landing.is_none() {
        if let Some(window) = &mut *window {
            window.poll();
            if window.closed() || (front.is_none() && window.held(key::ESCAPE)) {
                break;
            }
            // Hidden while every seat rides with no menu up, and back for any pause (P167).
            let showing = cursor::Showing::Ride(front.as_deref().map(|f| &f.menu));
            window.show_cursor(cursor::shown(showing, window.focused()));
        }
        // P298: gone away, the ride is paused and where it is saved; back, the save is not needed.
        if let Some(f) = front.as_deref_mut() {
            if let Some(asked) = resume::away() {
                if !f.menu.paused(0) {
                    f.menu.pause();
                }
                keep_resume_point(&f.dir, ride_resume_point(&args, &world, head.is_some(), f.menu.lobby.start.is_some()).as_ref());
                // On its way to being suspended (P298a: a 5.2 GiB ride was jetsam's first pick), the
                // ride's models and their textures go back before the answer lets iOS suspend it.
                // The window draws nothing till it is in front again, and that frame, under the
                // pause, uploads what it draws from the assets still held.
                if resume::backgrounds() != backgrounded {
                    backgrounded = resume::backgrounds();
                    if let Some(r) = renderer_of(&mut offscreen, window) {
                        let (models, materials, textures) = r.give_back_models();
                        println!("resume: gave back {models} models, {materials} materials and {textures} textures in the background");
                    }
                }
                resume::answered(asked);
            }
            if resume::back() {
                resume::clear(&f.dir);
            }
        }
        // A window that changed size gets a frame of its new size rather than the old one stretched
        // over it — and with it the aspect the camera's field of view and its viewport test read,
        // each view's own (V24). A resize across the middle of 4:3 and 16:9 changes the display the
        // frame stands for, and the cameras reload their presets for it the way the original's do
        // when their split changes: the same preset, re-set, from the other copy.
        if window.as_mut().is_some_and(|w| w.resized()) || std::mem::take(&mut reshaped) {
            let (width, height) = frame_size(&window, aspect);
            if (width, height) != (frame.width, frame.height) {
                frame = Frame::new(width, height, look.fog_color);
                (rects, view_frames) = views_of(width, height, layout, args.players);
                if let Some(window) = &window {
                    println!("window {}x{}, drawing {width}x{height}", window.width, window.height);
                }
                let now = Display::of(width, height, aspect);
                if now != display {
                    display = now;
                    println!("display: {}", display.name());
                    set_display(&mut rigs, layout, display, cpresets.as_deref(), plane_presets.as_deref());
                    set_heli_display(&mut rigs, layout, display, heli_presets.as_deref());
                    set_car_display(&mut rigs, layout, display, &car_presets);
                }
            }
        }
        // The pad this frame: the tape's, if one is being ridden, and the host's otherwise. A tape
        // that has run out ends the session — there is no pad behind it to carry on with.
        // Each player's report as the pad sent it, before its scheme moved anything, with its presses
        // since the last frame: what the pause reads (P53). Under the new scheme the ride's report
        // has R2 where Cross was, so a pause read from it took R2 for OK and Cross did nothing.
        let mut menu_reports: Vec<(Report, u16)> = Vec::new();
        // And each one's moments, with the shell's clock when the seats were read (P165): the pause
        // times its presses and holds by them, not by the ride's ticks. A tape has none, and its
        // pause steps a tick's time a tick.
        let mut menu_moments: Vec<Option<Vec<menu::Moment>>> = Vec::new();
        let mut menu_us: Option<u64> = None;
        // The recording changed the modifiers before the frame it hands out next (P98): the ride
        // takes them there, as it did when it was made.
        if let Some(to) = head.as_ref().and_then(Head::change) {
            remodify(&mut world.seats, &mut rigs, &stock_phys, stock_plane.as_ref(), &lot, &mods, &to, Race::ceiling_for_fog(F(look.fog_far)));
            mods = to;
            engine.time_scale = mods.time_scale();
            if let Some(front) = front.as_deref_mut() {
                front.menu.modified = !mods.is_original();
            }
            // A tape being written off the one being played (`--play a --record b`) changes there too.
            if let Some(tape) = &mut world.tape {
                tape.modifiers(mods);
            }
            println!("modifiers: {} (the tape's, from frame {})", mods.text(), head.as_ref().map_or(0, Head::at));
        }
        // And switched machine there (P102): the first player's, spawned where they are at that
        // tick, so the ride after it is the one recorded.
        for word in head.as_ref().map(Head::switches).unwrap_or_default() {
            match Kind::parse(&word) {
                Ok(to) => {
                    let up = switch_machine(&mut world.seats[0], &mut rigs[0], to, &mut world.place);
                    if let (Some(up), Some(tape)) = (up, &mut world.tape) {
                        tape.switch(up.name());
                    }
                    if let (Some(up), Some(front)) = (up, front.as_deref_mut()) {
                        front.menu.switched(0, menu::Machine::from_word(up.name()).unwrap_or_default());
                    }
                }
                Err(e) => println!("the tape's switch at frame {}: {e}", head.as_ref().map_or(0, Head::at)),
            }
        }
        // The recording's first player cleared their markers from the pause before this frame (P99).
        if head.as_ref().is_some_and(Head::cleared) && world.seats[0].sim.markers.any() {
            world.seats[0].sim.markers.clear();
            if let Some(tape) = &mut world.tape {
                tape.markers_cleared();
            }
            println!("markers: cleared (the tape's, before frame {})", head.as_ref().map_or(0, Head::at));
        }
        let mut idle_presses: Vec<(usize, menu::Press)> = Vec::new();
        // Every device's report and presses, each on its own, for the Controllers screen (P210).
        let mut assign_reports: Vec<(Dev, Report, u16)> = Vec::new();
        // The free pad seat the keyboard's presses this frame are for, if it drives none (P173).
        let mut keys_join: Option<usize> = None;
        // And set Air Wrecks there (P138).
        if let Some(on) = head.as_ref().and_then(Head::air_wrecks) {
            world.seats[0].sim.rider.sense.air_wrecks = on;
            if let Some(tape) = &mut world.tape {
                tape.air_wrecks(on);
            }
            println!("air wrecks: {} (the tape's, before frame {})", if on { "on" } else { "off" }, head.as_ref().map_or(0, Head::at));
        }
        // And Backwards Landings Bail (S161).
        if let Some(on) = head.as_ref().and_then(Head::backwards_bail) {
            world.seats[0].sim.rider.sense.backwards_bail = on;
            if let Some(tape) = &mut world.tape {
                tape.backwards_bail(on);
            }
            println!("backwards landings bail: {} (the tape's, before frame {})", if on { "on" } else { "off" }, head.as_ref().map_or(0, Head::at));
        }
        match &mut head {
            // The seats', otherwise: seat i feeds player i's own pad port (V22's players). A seat
            // whose pad was pulled out pauses its own player rather than riding on with nobody's
            // hands on the bars (`Seat::waiting`); the other players ride on.
            None => match &mut *pad_seats {
                Some(seats) => {
                    if let Some(w) = window.as_ref() {
                        seats.keys_at(w.key_moments());
                    }
                    // A free pad pressed while the keyboard or the touch pad is 1P takes 1P over
                    // (P210); a rider's pad is not free.
                    let ride: Vec<usize> = (0..world.seats.len()).map(|i| args.pad_seat(i)).collect();
                    seats.set_riding(&ride);
                    seats.set_take_over(front.as_deref().is_some_and(|f| f.menu.takes_over()));
                    let first = seats.first().cloned();
                    let now = seats.frame(&|code| held(&window, code));
                    let read_us = crate::window::shell_us();
                    menu_us = Some(read_us);
                    for line in seats.said() {
                        println!("{line}");
                    }
                    if let Some(front) = front.as_deref_mut() {
                        took_over(seats, &mut front.menu, first);
                        give_controllers(seats, &mut front.menu);
                        // Whose buttons each view's hints draw (P281): its player's pad seat's.
                        let dev = |k: usize| seats.slots.held.get(k).cloned().flatten().map_or(Dev::Keys, Dev::Pad);
                        front.menu.ride_devs = ride.iter().map(|&k| dev(k)).collect();
                        front.menu.key_caps = seats.key_caps();
                        front.menu.free_dev = (0..seats.slots.held.len()).filter(|k| !ride.contains(k)).find_map(|k| seats.slots.held[k].clone()).map(Dev::Pad);
                    }
                    for (i, input) in now.iter().enumerate() {
                        let dev = match (&seats.slots.held[i], input) {
                            (Some(id), SeatInput::Playing(_)) => Dev::Pad(id.clone()),
                            (None, SeatInput::Playing(_)) => Dev::Keys,
                            _ => continue,
                        };
                        assign_reports.push((dev, input.report(), seats.taps(i)));
                    }
                    for (id, report, taps) in seats.loose() {
                        assign_reports.push((Dev::Pad(id.clone()), *report, *taps));
                    }
                    if let Some(keys) = seats.keyboard_alone() {
                        assign_reports.push((Dev::Keys, keys, 0));
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
                        menu_moments.push(seats.seat_moments(from));
                        // And the plane's stick turned round upside down under the screen's scheme
                        // (P50), off the attitude the last step left, so the plane's simulation
                        // reads the original's pad; under the plane's (P69) the elevator answers at
                        // every bank instead.
                        seat.inverted = seat.garage.inverted(seat.inverted);
                        seat.garage.set_every_bank(seats.plane(from).every_bank());
                        seat.garage.set_round_stick(seats.plane(from).round_stick());
                        // The player's Markers switch (P129), live: a change under the pause
                        // rides from the frame it is made.
                        seat.markers = args.markers_of(i, front.as_deref());
                        // And the layout's reset chord (P67), live like the layout itself.
                        seat.together = seats.scheme(from) == Scheme::New;
                        // A bike with MX vs. ATV's tricks (MD16) reads the new layout's Cross and Square as
                        // its two extra trick buttons.
                        let tricks = seat.sim.phy.tricks.is_some();
                        let report = seats.plane(from).fly(seats.scheme(from).ride_tricks(report, tricks), seat.inverted);
                        seat.reports = [report, Report::missing()];
                        // The bike's commands through the seat's controller preset (P79). Online the
                        // ride's own seats hold theirs, the room's pick (N13), and this one is drawn
                        // from them.
                        seat.sim.set_presets([seats.preset(from), 0]);
                        seat.waiting = now.get(from) == Some(&SeatInput::Waiting);
                    }
                    // A controller on a pad seat no player reads, for the join (P76).
                    for (d, (reader, input)) in idle_readers.iter_mut().zip(&now).enumerate() {
                        let playing = matches!(input, SeatInput::Playing(_));
                        let report = if playing { input.report() } else { Report::idle() };
                        let moments = if playing { seats.seat_moments(d) } else { None };
                        let press = reader.read_timed(&[(report, seats.taps(d), moments.as_deref())], read_us);
                        if playing && !ride.contains(&d) {
                            idle_presses.push((d, press));
                            while let Some(p) = reader.more() {
                                idle_presses.push((d, p));
                            }
                        }
                    }
                    // The keyboard beside a pad 1P (P173): its presses are a controller on the first
                    // free pad seat, so its Cross joins as an idle pad's does. Not while 1P's pause
                    // is open, which the keyboard works (P46).
                    let report = seats.keyboard_alone().unwrap_or_else(Report::idle);
                    let moments = seats.keyboard_alone_moments();
                    let press = keys_reader.read_timed(&[(report, 0, moments.as_deref())], read_us);
                    let pause_one = front.as_deref().is_some_and(|f| f.menu.paused(0));
                    if let Some(d) = seats.keys_free_seat(&ride).filter(|_| !pause_one && seats.keyboard_alone().is_some()) {
                        keys_join = Some(d);
                        idle_presses.push((d, press));
                        while let Some(p) = keys_reader.more() {
                            idle_presses.push((d, p));
                        }
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
                    // Nor is the Markers switch (P129): the command line's, on unless --markers off.
                    world.seats[0].markers = args.markers.map_or(true, |m| m[0]);
                    // Nor is the layout's reset chord (P67): the command line's layout says which.
                    world.seats[0].together = args.wiring.schemes[0] == Scheme::New;
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
        // The swap, between frames, which is where the original's pause menu and prompt run it: the
        // controller moves and the machine taken gets its own camera, which starts where it is
        // rather than lagging in from wherever it was last. A played tape is the ride it recorded,
        // and the tape has no swap in it, so the key is not read while one is being ridden.
        // It is the first player's key, and the first player's plane.
        // The pause's "Switch To Machine" (P41) is the same swap for whichever player opened it; the
        // key stays for the keyboard.
        // A tape being written takes the switch at the tick it is on (P102), as it takes modifiers.
        let swap_now = held(&window, key::M);
        if swap_now && !swap_key && online.is_some() {
            // Online the key is a pick in the pad like the pause's (N17).
            online_want = Some(world.seats[0].garage.up.other()).filter(|&to| to == Kind::Bike || world.seats[0].garage.plane.is_some());
        } else if swap_now && !swap_key && head.is_none() {
            let to = world.seats[0].garage.up.other();
            let up = switch_machine(&mut world.seats[0], &mut rigs[0], to, &mut world.place);
            if let (Some(up), Some(tape)) = (up, &mut world.tape) {
                tape.switch(up.name());
            }
            if let (Some(up), Some(front)) = (up, front.as_deref_mut()) {
                front.menu.switched(0, menu::Machine::from_word(up.name()).unwrap_or_default());
            }
        }
        swap_key = swap_now;
        // The bays (P103): the lot's machines standing see-through where they are parked, unless the
        // settings turned them off. Riding up to one offers it, and Circle — the button the
        // original's pop-up shows, its pad's own, not as a scheme moved it — takes it, through the
        // same switch as the key and the pause. Not on a played tape, whose switches are its own
        // lines, nor online, where the ride is the bike's.
        let bays = vehicle::bays_shown(&lot, front.as_deref().is_none_or(|f| f.menu.settings.bays));
        // A paused player is offered nothing (`seat_offer`), and the others ride on and are (P84).
        let offering = head.is_none() && online.is_none();
        answers.resize(world.seats.len(), vehicle::Answer::default());
        for (i, answer) in answers.iter_mut().enumerate() {
            let (report, taps) = menu_reports.get(i).copied().unwrap_or((world.seats[i].reports[0], 0));
            let circle = Control::Circle.mask();
            let held = report.status() != 0xff && report.buttons() & circle != 0;
            let offered = seat_offer(&world.seats[i], &bays).filter(|_| offering);
            let Some(to) = answer.take(held, taps & circle != 0, offered) else { continue };
            if let Some(up) = switch_machine(&mut world.seats[i], &mut rigs[i], to, &mut world.place) {
                if let (0, Some(tape)) = (i, &mut world.tape) {
                    tape.switch(up.name());
                }
                if let Some(front) = front.as_deref_mut() {
                    front.menu.switched(i, menu::Machine::from_word(up.name()).unwrap_or_default());
                }
            }
        }
        stages.mark(stages::Stage::Input);

        // The pause, under the front end: Start or Escape opens it, and while it is open its player's
        // ride is frozen exactly as Start's own freeze leaves it, the pass polling the pad and stepping
        // nothing. What it changes is applied at once, and saved.
        if let Some(front) = front.as_deref_mut() {
            // Every player's press: each player's Start opens their own pause, which their pad drives
            // (P41) in their own view while the others ride on (P84). Escape is the keyboard's, 1P's.
            // Each player's pad as it is, not as the ride's scheme moved it (P53).
            let menu_report = |i: usize| menu_reports.get(i).copied().unwrap_or((world.seats[i].reports[0], 0));
            let menu_moment = |i: usize| menu_moments.get(i).cloned().flatten();
            let mut seat0 = vec![(menu_report(0), menu_moment(0))];
            // The keyboard pauses and works 1P's pause whoever has 1P (P46), from no seat of its own:
            // read as one with 1P's pad (P109), so a push both see is one press.
            if let Some(keys) = pad_seats.as_ref().and_then(|s| s.keyboard_alone()) {
                seat0.push(((keys, 0), pad_seats.as_ref().and_then(|s| s.keyboard_alone_moments())));
            }
            // By the time they were pushed when the seats were read, by the ride's ticks off a tape.
            let read = |r: &mut Reader, devices: &[((Report, u16), Option<Vec<menu::Moment>>)]| match menu_us {
                Some(now) => {
                    let timed: Vec<menu::Timed> = devices.iter().map(|((r, t), m)| (*r, *t, m.as_deref())).collect();
                    r.read_timed(&timed, now)
                }
                None => r.read_joint(&devices.iter().map(|(rt, _)| *rt).collect::<Vec<_>>()),
            };
            let first = read(&mut front.reader, &seat0);
            let mut presses = vec![first];
            for (i, r) in (1..world.seats.len().min(args.players)).zip(&mut start_readers) {
                presses.push(read(r, &[(menu_report(i), menu_moment(i))]));
            }
            match menu_us {
                Some(now) => front.menu.elapse_to(now),
                None => front.menu.elapse(1_000_000 / u64::from(RIDE_HZ)),
            }
            let esc = held(window, key::ESCAPE);
            let esc_now = esc && !escape;
            // The keyboard's own player's once it joined the ride on a seat of its own (P173).
            let keys_player = front.menu.keys_seat.filter(|&k| k > 0 && k < presses.len());
            if esc && !escape {
                if let Some(k) = keys_player {
                    if !front.menu.paused(k) {
                        front.menu.pause_for(k);
                    } else {
                        presses[k].back = true;
                    }
                } else if !front.menu.paused(0) {
                    front.menu.pause();
                } else if let Some(p) = presses.get_mut(front.menu.driver()) {
                    p.back = true;
                }
            }
            escape = esc;
            front.menu.music_now = front.sound.now();
            front.menu.music_held = front.sound.held();
            // In a ride the menu's players are the ride's seats, each on its own pad seat.
            front.menu.keys_seat = pad_seats.as_ref().and_then(|s| (0..world.seats.len()).find(|&i| s.on_keys(args.pad_seat(i))));
            let paused: Vec<bool> = (0..world.seats.len()).map(|i| front.menu.paused(i)).collect();
            // 1P's pause has the Controllers screen up (P210): every device works its own card, the
            // riders' and the ones in no seat alike, and 1P's joint press is not stepped again.
            if front.menu.assigning() {
                // A card dragged with the mouse (P214), in the view 1P's pause draws the screen in.
                if let Some(w) = window.as_ref()
                    && let Some((_, r)) = front.menu.pause_views(&rects, world.seats.len()).into_iter().find(|(v, _)| *v == 0)
                {
                    drag_cards(&mut front.menu, w.mouse(), (w.width, w.height, w.scale), (frame.width, frame.height), [r.x, r.y, r.w, r.h]);
                }
                let mut each = assign_readers.read_taps(&assign_reports);
                if esc_now {
                    each.push((Dev::Keys, menu::Press { back: true, ..menu::Press::default() }));
                }
                front.menu.step_assign(&each);
                presses[0] = menu::Press::default();
                if let Some(seats) = pad_seats.as_mut() {
                    take_controllers(seats, &mut front.menu);
                }
            } else {
                assign_readers.reset();
            }
            let mut act = front.menu.step_seats(&presses);
            // The presses the tick held past each seat's first, a step a round (P165).
            while act == Act::Stay {
                let mut round = vec![front.reader.more()];
                round.extend(start_readers.iter_mut().take(presses.len() - 1).map(Reader::more));
                if round.iter().all(Option::is_none) {
                    break;
                }
                act = front.menu.step_seats(&round.into_iter().map(Option::unwrap_or_default).collect::<Vec<_>>());
            }
            for &(d, p) in &idle_presses {
                if act == Act::Stay {
                    act = front.menu.step_idle(d, p);
                }
            }
            // A click on a pause's music player (P174), in the view that pause is drawn in.
            if let Some(w) = window.as_ref() {
                for &at in w.clicks() {
                    let Some([x, y]) = cursor::to_frame(at, (w.width, w.height), (frame.width, frame.height), w.scale) else { continue };
                    let views = front.menu.pause_views(&rects, world.seats.len());
                    if let Some((v, r)) = views.into_iter().find(|(_, r)| x >= r.x as f32 && y >= r.y as f32 && x < (r.x + r.w) as f32 && y < (r.y + r.h) as f32) {
                        front.menu.with_seat(v, |m| m.click([x - r.x as f32, y - r.y as f32], (r.w, r.h)));
                    }
                }
            }
            front.cue(false);
            if front.menu.changed() {
                overlay_mode = front.menu.settings.overlay;
                speedometer = front.menu.settings.speedometer;
                minimap = front.menu.settings.minimap;
                show_fps = front.menu.settings.show_fps;
                world_sky = front.menu.settings.world_sky;
                hud_scale = front.menu.settings.hud_scale;
                graphics = front.menu.settings.graphics;
                if plants != front.menu.settings.plants {
                    plants = front.menu.settings.plants;
                    if let Some(grove) = &mut look.ecosystem {
                        grove.lod = plants;
                    }
                    if let Some(r) = renderer_of(&mut offscreen, window) {
                        r.set_plant_lod(plants);
                    }
                }
                // The leaves (V57), on the ride's grove, every world tile's and the renderer's.
                if foliage != front.menu.settings.foliage {
                    foliage = front.menu.settings.foliage;
                    for g in look.ecosystem.iter_mut().chain(dressed.iter_mut().filter_map(|d| d.grove.as_mut())) {
                        g.foliage = foliage;
                    }
                    if let Some(r) = renderer_of(&mut offscreen, window) {
                        r.set_plant_foliage(foliage);
                    }
                }
                // Texture Filtering (V83): every sampler made again, from the next frame.
                if filtering != front.menu.settings.filtering {
                    filtering = front.menu.settings.filtering;
                    if let Some(r) = renderer_of(&mut offscreen, window) {
                        r.set_anisotropy(filtering.anisotropy());
                    }
                }
                // Model Detail (P223), from the next frame.
                if let Some(r) = renderer_of(&mut offscreen, window) {
                    r.lod_bias = front.menu.settings.detail.lod_bias();
                }
                // Shadows and Shadow Quality (V108, user 2026-10-30), from the next frame.
                if shadows != (front.menu.settings.shadows, front.menu.settings.shadow_quality) {
                    shadows = (front.menu.settings.shadows, front.menu.settings.shadow_quality);
                    if let Some(r) = renderer_of(&mut offscreen, window) {
                        let (s, q) = game_render::shadow::drawn(shadows.0, shadows.1, true);
                        r.set_shadows(s, q);
                    }
                }
                if !args.aspect_asked && aspect != aspect_of(front.menu.settings.picture) {
                    aspect = aspect_of(front.menu.settings.picture);
                    reshaped = true;
                }
                if !args.render_scale_asked && render_scale != front.menu.settings.render_scale.fraction() {
                    render_scale = front.menu.settings.render_scale.fraction();
                    if let Some(r) = renderer_of(&mut offscreen, window) {
                        r.render_scale = render_scale.unwrap_or(1.0);
                    }
                    println!("render scale: {} (settings)", render_scale.map_or("auto".into(), |f| f.to_string()));
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
                // Leave Session (N11): this machine's seats go at a tick every machine takes them out
                // at, and the ride carries on for the others; the session ends here once it has
                // passed on what it owes them.
                Act::LeaveSession(seat) => {
                    if let Some(on) = &mut online
                        && let Err(why) = on.leave_session(seat)
                    {
                        println!("online: cannot leave: {why}");
                        front.menu.note = Some(format!("Cannot leave the session: {why}"));
                    }
                }
                // A played tape switches where it says and nowhere else. Online the pick goes into
                // this machine's pad (N17), and every machine switches the seat at the tick it lands
                // on; the garage follows below, once the session has stepped it.
                Act::Switch(i, to) if online.is_some() => {
                    // A car is had where this seat's copy of it is (S249).
                    let car = |to: Kind| online.as_ref().is_some_and(|on| on.cars(on.seat_of().1).iter().any(|c| Some(c.machine) == to.car()));
                    match (i, Kind::parse(to.word())) {
                        (0, Ok(to)) if (to == Kind::Biplane && world.seats[0].garage.plane.is_some()) || to == Kind::Bike || car(to) => {
                            online_want = Some(to);
                            println!("vehicle: {} at the next tick every machine agrees", to.name());
                        }
                        _ => front.menu.note = Some("This ride has no such machine for you.".into()),
                    }
                }
                Act::Switch(i, to) if head.is_none() => {
                    let to = Kind::parse(to.word()).ok();
                    if let (Some(to), Some(seat), Some(rig)) = (to, world.seats.get_mut(i), rigs.get_mut(i))
                        && let Some(up) = switch_machine(seat, rig, to, &mut world.place)
                    {
                        if let (0, Some(tape)) = (i, &mut world.tape) {
                            tape.switch(up.name());
                        }
                        front.menu.switched(i, menu::Machine::from_word(up.name()).unwrap_or_default());
                    }
                }
                // Open to online players (N10): host a lobby on this ride, which rides on as the
                // session's from here, alone until a peer drops in.
                Act::OpenOnline if online.is_none() && front.menu.opening == menu::Opening::Can => {
                    {
                        let settings = &front.menu.settings;
                        let mut room = openmx_net::Room::new(&settings.name, &args.track, settings.lobby_seats);
                        room.wrecks = args.wrecks && settings.wrecks;
                        room.air_wrecks = args.air_wrecks && settings.air_wrecks;
                        room.backwards_bail = args.backwards_bail || settings.backwards_bail;
                        room.modifiers = mods.text();
                        // the trick pack this ride rides (MD19), which every joiner's machine lays
                        // (`ride_trick_packs`' pick, which this ride was built on)
                        let named = args.trick_packs.clone().unwrap_or_default();
                        room.tricks = lobby::room_tricks(args.tricks.unwrap_or_default(), &named, mods_dir(&args).as_deref());
                        // and the handling it rides on (N21): the pack its bikes were read from
                        // (`ride_handling_pack`'s pick), if any
                        let named = args.handling_pack.clone().unwrap_or_default();
                        room.handling = lobby::room_handling(args.handling.unwrap_or_default(), &named, &room.tricks, mods_dir(&args).as_deref());
                        let mut pick = openmx_net::Pick::new(&front.menu.lobby.pick.rider, &args.set.unwrap_or(roster::Set::FREERIDE).word());
                        // the preset this seat rides on now (N13), which the room tells a joiner's machine
                        pick.preset = net::rides::preset(&world.seats[0].sim).unwrap_or(0) as u8;
                        pick.look = front.menu.lobby.pick.look.clone();
                        pick.model = args.bike.clone();
                        room.take(openmx_net::HOST, &[pick]);
                        (room.riding, room.drop_in) = (true, true);
                        let net = front.network.open(&mut front.menu, room);
                        // A player in the plane takes it online with them (N17): the session's seat
                        // is its `Sim`, and its plane that `Sim`'s own lane.
                        let mut sim = world.seats[0].sim.clone();
                        if world.seats[0].garage.up == Kind::Biplane {
                            sim.plane = world.seats[0].garage.plane.as_ref().and_then(|p| p.flight.clone());
                        }
                        match online::Online::open(net, sim, &spawn) {
                            Ok(mut on) => {
                                on.modifiers(mods, stock_phys[..1].to_vec());
                                on.planes(stock_plane, plane_like.as_ref());
                                // a joiner whose setup picked the plane rides in from its own bay (N19)
                                on.bays_on(online_bays.clone(), online_bodies.clone());
                                on.cars_on(lot.online_cars.clone());
                                online_want = None;
                                front.menu.mods_lock = mods_lock(head.is_some(), Some((on.host(), on.own_rows())));
                                front.menu.air_lock = air_lock(head.is_some(), true);
                                online = Some(on);
                                front.menu.opening = menu::Opening::Open;
                            }
                            Err(e) => front.menu.note = Some(format!("Could not open the ride: {e}")),
                        }
                    }
                }
                // A player on a free pad joining an online ride (N20): the host seats them at a tick
                // every machine takes, and the views are laid out again once the session has it.
                Act::Join(d) if online.is_some() => {
                    let preset = pad_seats.as_ref().map_or(args.wiring.presets[0], |s| s.preset(d));
                    if let Some(on) = &mut online {
                        match on.sit(preset as u8) {
                            Ok(()) => {
                                online_sits.push(d);
                                front.menu.note = Some("Asking for a seat...".into());
                            }
                            Err(why) => {
                                println!("online: cannot add a player: {why}");
                                front.menu.note = Some(format!("Cannot join: {why}"));
                            }
                        }
                    }
                }
                Act::Stay
                | Act::Resume
                | Act::Ride(_)
                | Act::Switch(..)
                | Act::Online
                | Act::OpenOnline
                | Act::Join(_)
                | Act::Leave(_) => {}
            }
            // A player joining or leaving the running ride (P76): every per-player list changed the
            // one way, so each staying player keeps their machine, camera, markers and pad where
            // they were, and the views laid out again for the new count.
            let change = match act {
                _ if online.is_some() => None,
                Act::Join(d) if front.menu.seats_open && world.seats.len() < join::IN_RIDE => Some(join::Change::Join(d)),
                Act::Leave(i) if front.menu.seats_open && i < world.seats.len() && world.seats.len() > 1 => Some(join::Change::Leave(i)),
                _ => None,
            };
            if let Some(change) = change {
                let n = match change {
                    join::Change::Join(_) => world.seats.len() + 1,
                    join::Change::Leave(_) => world.seats.len() - 1,
                };
                // The lot parks a machine for each player now (P87), every staying copy where it was.
                park_lot(&mut lot, &mut world.place, n);
            }
            // A joiner's seat and rig, made before anything is reseated: one that cannot be made
            // (a disc read failing under it) is said, and the ride goes on with the players it had
            // rather than ending (P271).
            let joined = match change {
                Some(join::Change::Join(_)) => {
                    let made = (|| -> Result<_, String> {
                        let i = world.seats.len();
                        // At the track's spawn on the first player's bike, see-through until clear
                        // of whoever stands there (P57), on the markers kept under their seat's name.
                        let mut sim = spawn.clone();
                        sim.rider.sense.wrecks_enabled = args.wrecks && front.menu.settings.wrecks;
                        sim.rider.ghost.start();
                        let (profile, bike) = (saved_markers::profile("", i), &bike_words[0]);
                        sim.markers = saved_markers::load(&markers_dir, &args.track, &profile, bike);
                        sim.markers.up(vehicle::markers_of(Kind::Bike));
                        let kept = saved_markers::Kept::new(&args.track, &profile, bike, &sim.markers);
                        let mut garage = Garage::new(Kind::Bike, lot.plane(i, flight_for(i)))?;
                        garage.cars = lot.cars_for(i);
                        garage.heli = lot.heli_for(i);
                        garage.modify(&lot, &mods, i);
                        let body = match &rider_member {
                            Some(member) => Body::load(vec![disc.res(member)?], &args.rider, Sex::Male, "", &table.stunts, phy.motion()).ok(),
                            None => None,
                        };
                        let body = body.map(|mut b| {
                            if let Some(p) = &pack_clips {
                                b.add_pack(p, &stunts.stunts);
                            }
                            b
                        });
                        let chassis = match vehicle.mcf.is_empty() {
                            true => None,
                            false => Chassis::load(vec![disc.res(&bike_member)?], &vehicle.mcf, BIKE_PREFIX, &table.stunts, vehicle_params()).ok(),
                        };
                        let chassis = chassis.map(|mut c| {
                            if let Some(p) = &pack_clips {
                                c.add_pack(p, &stunts.stunts);
                            }
                            c
                        });
                        // On the GPU every bike and body is the one model posed by matrices; in software
                        // a pose is the model's triangles, so the joiner's are copies of their own.
                        joined_models += 1;
                        let mut copy = |name: &str| -> String {
                            let own = format!("{name}@join{joined_models}");
                            match assets.models.get(name).cloned().filter(|_| !gpu_on) {
                                Some(loaded) => {
                                    assets.models.insert(own.clone(), loaded);
                                    own
                                }
                                None => name.to_string(),
                            }
                        };
                        let bike_model = copy(CHASSIS);
                        let body_model = join_body_model.as_deref().map(&mut copy);
                        let rig = Rig {
                            body,
                            chassis,
                            camera: Race { cam: join_cameras.0.clone() },
                            plane_camera: join_cameras.1.as_ref().map(|c| Race { cam: c.clone() }),
                            heli_camera: heli_camera.as_ref().map(|c| Race { cam: c.cam.clone() }),
                            car_cameras: car_cameras.iter().map(|(k, c)| (*k, Race { cam: c.cam.clone() })).collect(),
                            wreck_camera: chase::WreckCamera::new(false),
                            framing: (chase::Framing::ORIGINAL, chase::Framing::ORIGINAL),
                            camera_presets: Default::default(),
                            camera_started: false,
                            fov_ease: Default::default(),
                            look_back: Default::default(),
                            thrill: Default::default(),
                            odometer: overlay::Odometer::default(),
                            preload_meter: overlay::PreloadMeter::default(),
                            pilot_lean: vehicle::PilotLean::default(),
                            ride_hud: ride_hud::Tracker::default(),
                            bike_model,
                            body_model,
                            parts: vehicle.parts.clone(),
                            modified: !mods.seat_is_original(0),
                        };
                        let seat = Seat {
                            reports: [Report::missing(); PADS],
                            sim,
                            garage,
                            riding: false,
                            start_freezes: false,
                            waiting: false,
                            paused: false,
                            said: rider::Frame::default(),
                            inverted: false,
                            hands: Controls::default(),
                            teleported: false,
                            clears: 0,
                            markers: args.markers_of(i, Some(&*front)),
                            together: false,
                            shade: PageShade::default(),
                        };
                        Ok((seat, rig, kept))
                    })();
                    match made {
                        Ok(joiner) => Some(joiner),
                        Err(why) => {
                            park_lot(&mut lot, &mut world.place, world.seats.len());
                            println!("players: player {} could not join: {why}", world.seats.len() + 1);
                            front.menu.note = Some(format!("Player {} could not join: {why}", world.seats.len() + 1));
                            None
                        }
                    }
                }
                _ => None,
            };
            let change = change.filter(|c| !matches!(c, join::Change::Join(_)) || joined.is_some());
            if let Some(change) = change {
                reseated = true;
                let devices: Vec<usize> = (0..world.seats.len()).map(|i| args.pad_seat(i)).collect();
                let n = match change {
                    join::Change::Join(_) => world.seats.len() + 1,
                    join::Change::Leave(_) => world.seats.len() - 1,
                };
                let (mut seat, mut rig, mut kept) = match joined {
                    Some((seat, rig, kept)) => (Some(seat), Some(rig), Some(kept)),
                    None => (None, None, None),
                };
                let start = spawn.rider.ride.position();
                let (bike_word, stock_phy) = (bike_words[0].clone(), stock_phys[0].clone());
                change.apply(&mut world.seats, || seat.take().expect("a joiner's seat"));
                change.apply(&mut rigs, || rig.take().expect("a joiner's rig"));
                change.apply(&mut kept_markers, || kept.take());
                change.apply(&mut idle_pads, || Pads::unplugged(args.wiring.clone()));
                change.apply(&mut marked_portals, Default::default);
                change.apply(&mut seated, || true);
                change.apply(&mut view_triangles, || 0);
                change.apply(&mut cuts, pace::Cut::default);
                change.apply(&mut banners, hud::MarkerBanner::default);
                change.apply(&mut answers, vehicle::Answer::default);
                change.apply(&mut join_starts, || start);
                change.apply(&mut bike_words, || bike_word);
                change.apply(&mut stock_phys, || stock_phy);
                // The last tick's cameras, which the next blends from: a joiner's starts where it is.
                if let join::Change::Leave(i) = change
                    && i < cameras.len()
                {
                    cameras.remove(i);
                }
                match change {
                    // The first player's reader is the front end's; the others' are one each.
                    join::Change::Join(_) => start_readers.push(Reader::at(RIDE_HZ)),
                    join::Change::Leave(i) if !start_readers.is_empty() => {
                        start_readers.remove(i.saturating_sub(1));
                    }
                    join::Change::Leave(_) => {}
                }
                if let Some(sound) = &mut sound {
                    match change {
                        join::Change::Join(_) => sound.seat_joined(),
                        join::Change::Leave(i) => sound.seat_left(i),
                    }
                }
                // The keyboard rides the pad seat it joined on, and lets it go when its player leaves (P173).
                if let Some(seats) = pad_seats.as_mut() {
                    match change {
                        join::Change::Join(d) if keys_join == Some(d) => seats.join_keys(d),
                        join::Change::Leave(i) => seats.leave_keys(devices.get(i).copied().unwrap_or(usize::MAX)),
                        join::Change::Join(_) => {}
                    }
                }
                // Every player's pad seat, kept as the players are.
                args.pad_of = change.devices(&devices);
                args.players = n;
                // Where each machine starts again (P132), every seat on the one list.
                let bays: Vec<vehicle::Spawn> = (0..n).map_while(|p| lot.bay(Machine::Biplane, p).map(|b| b.spawn)).collect();
                let home = Arc::new(vehicle::home(&join_starts, heading, &bays));
                world.seats.iter_mut().for_each(|s| s.sim.home = Arc::clone(&home));
                if let join::Change::Join(_) = change {
                    let (seat, rig) = (&world.seats[n - 1], &mut rigs[n - 1]);
                    let first = rig.follow(seat.garage.up);
                    if !first.cam.presets.is_empty() {
                        let name = first.cam.preset().name.clone();
                        rig.ride_hud.note_camera(Instant::now(), &name);
                    }
                }
                // The views for the new count, each camera on its presets for its view's shape.
                front.menu.reseated(change);
                layout = Layout::for_players(n, front.menu.party().map_or(args.split, |p| p.split))?;
                (rects, view_frames) = views_of(frame.width, frame.height, layout, n);
                set_display(&mut rigs, layout, display, cpresets.as_deref(), plane_presets.as_deref());
                set_heli_display(&mut rigs, layout, display, heli_presets.as_deref());
                set_car_display(&mut rigs, layout, display, &car_presets);
                front.menu.opening = match opens_alone && n == 1 {
                    true => menu::Opening::Can,
                    false => menu::Opening::No,
                };
                println!(
                    "players: {}; views: {n} players, {}, {}x{} each",
                    match change {
                        join::Change::Join(d) => format!("player {n} joined on pad seat {}", d + 1),
                        join::Change::Leave(i) => format!("player {} left", i + 1),
                    },
                    layout.name(),
                    rects[0].w,
                    rects[0].h
                );
            }
            // The join cue (P171) while a seat is free and a controller on a pad seat no player reads
            // is in; gone once it is seated, or the ride is full.
            // The keyboard is always there, so it is no cue of its own (P173).
            let idle: Vec<usize> = idle_presses.iter().map(|(d, _)| *d).filter(|d| Some(*d) != keys_join).collect();
            join_cue = front.menu.join_cue(&idle);
            // The pause closing on modifiers the player changed in it: the ride takes them now, from
            // where it is (P98), and a tape being written takes the change at the tick it is on.
            let closed = paused.iter().enumerate().any(|(i, &was)| was && !front.menu.paused(i));
            if closed && menu_mods != Some(front.menu.settings.modifiers) {
                let to = front.menu.settings.modifiers;
                menu_mods = Some(to);
                // the knobs the host set through All Players, every seat's online too (P134)
                let all = front.menu.take_mods_all();
                // Online the host's change goes to every machine, which all take it at one tick
                // (P100), and so does a guest's of its own seat's (P134), through the host: the
                // ride is moved onto it below, once the session has.
                if let Some(on) = online.as_mut().filter(|_| front.menu.mods_lock.is_none()) {
                    let sent = if on.host() { on.remodify(&to, &all) } else { on.remodify_own(&to) };
                    match sent {
                        Ok(()) => println!("modifiers: {} (from the pause, for every machine at the next tick they agree)", to.text()),
                        Err(e) => front.menu.note = Some(format!("The modifiers did not change: {e}")),
                    }
                } else if front.menu.mods_lock.is_none() && to != mods {
                    remodify(&mut world.seats, &mut rigs, &stock_phys, stock_plane.as_ref(), &lot, &mods, &to, Race::ceiling_for_fog(F(look.fog_far)));
                    mods = to;
                    engine.time_scale = mods.time_scale();
                    front.menu.modified = !mods.is_original();
                    if let Some(tape) = &mut world.tape {
                        tape.modifiers(mods);
                    }
                    println!("modifiers: {} (from the pause)", if mods.is_original() { "the original game's".into() } else { mods.text() });
                }
            }
            // And on an Air Wrecks switch the player changed (P138): every seat takes it at this
            // tick, as Wrecks is every seat's, and a tape being written says so before its next frame.
            if closed && menu_air != Some(front.menu.settings.air_wrecks) {
                let on = front.menu.settings.air_wrecks;
                menu_air = Some(on);
                if front.menu.air_lock.is_none() {
                    let on = args.air_wrecks && on;
                    for seat in &mut world.seats {
                        seat.sim.rider.sense.air_wrecks = on;
                    }
                    if let Some(tape) = &mut world.tape {
                        tape.air_wrecks(on);
                    }
                    println!("air wrecks: {} (from the pause)", if on { "on" } else { "off" });
                }
            }
            // And on a Backwards Landings Bail switch the player changed (S161), the same way.
            if closed && menu_back != Some(front.menu.settings.backwards_bail) {
                let on = front.menu.settings.backwards_bail;
                menu_back = Some(on);
                if front.menu.air_lock.is_none() {
                    let on = args.backwards_bail || on;
                    for seat in &mut world.seats {
                        seat.sim.rider.sense.backwards_bail = on;
                    }
                    if let Some(tape) = &mut world.tape {
                        tape.backwards_bail(on);
                    }
                    println!("backwards landings bail: {} (from the pause)", if on { "on" } else { "off" });
                }
            }
            // Each player frozen while their own pause is open (P84), the others riding on. Online the
            // seats are the session's, frozen at the tick their pads say so (below).
            for (i, seat) in world.seats.iter_mut().enumerate() {
                seat.paused = front.menu.paused(i);
                seat.riding = !seat.paused;
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
        before_helis = world.seats.iter().map(|seat| seat.garage.heli_up().and_then(|h| Some((heli_at(h)?, heli_rotor(h))))).collect();
        before_cars = world.seats.iter().map(|seat| Some(car_draw::placed(seat.garage.car_up()?.driven.as_ref()?))).collect();
        before_surfaces = world.seats.iter().map(|seat| seat.garage.plane.as_ref().map(Plane::surfaces)).collect();
        cuts.resize_with(world.seats.len(), pace::Cut::default);
        pause_holds.resize_with(world.seats.len(), chase::PauseHold::default);
        for (i, (seat, cut)) in world.seats.iter().zip(&mut cuts).enumerate() {
            cut.before(cut_spots(seat, before[i].bike, cameras.get(i)));
        }
        traffic_before = traffic_time;
        cars_before = world.seats[0].sim.traffic.cars.iter().map(|c| c.pose).collect();
        }
        // Off to the sim thread (N23a): the world and the engine go, and the picture drawn meanwhile
        // is the one the last tick ended on, from a copy of the seats and a place with its ground.
        // Only a tick a picture follows goes: a backlog's run here, one after another, as they would
        // have. So do the tick that ends `--frames`, before the last picture, and the first, whose
        // second half the first picture is drawn from.
        let threaded = match args.sim_thread {
            sim_thread::Mode::Off => false,
            sim_thread::Mode::On => true,
            // A player may join a ride that began alone (P76).
            sim_thread::Mode::Auto => world.seats.len() >= 2 && gauge.wants(),
        };
        if args.timings && sim_worker.is_some() && threaded != gauge.engaged {
            timing(format!("     sim thread {}: {}", if threaded { "on" } else { "off" }, gauge.line()));
        }
        gauge.engaged = threaded;
        if threaded
            && !reseated
            && landing.is_none()
            && online.is_none()
            && sim_worker.is_some()
            && pacer.ticks() > 0
            && !pacer.backlog(moment)
            && !args.frames.is_some_and(|n| pacer.ticks() + 1 >= u64::from(n))
            && let Some(place) = drawn_land.take()
        {
            let stand_in = World { place, seats: world.seats.clone(), tape: None, played_dt: world.played_dt };
            let input = engine_input(&world.seats[0].reports[0]);
            let now = pace::tick_clock(pacer.ticks());
            let world = std::mem::replace(&mut world, stand_in);
            flight = Some(Ticking { world, engine: engine.clone(), now, input, step: None, took: Duration::ZERO });
        }
        if flight.is_none() {
        let sim_at = Instant::now();
        let dt: F;
        (dt, simulated) = if let Some(on) = &mut online {
            online_frames += 1;
            // `--online-script`'s picks and throttle up to the tick these pads go out on (S249), each
            // once: on the session's ticks, as `--frames` counts (K29), since a frame waiting on a
            // peer runs none, and a frame catching up runs several.
            let due = args.online_script[online_scripted..].iter().take_while(|(f, _)| u64::from(*f) <= on.ticks() + 1).count();
            online_scripted += due;
            for (_, word) in &args.online_script[online_scripted - due..online_scripted] {
                match (word.as_str(), Kind::parse(word)) {
                    ("gas", _) => online_gas = true,
                    ("lift", _) => online_gas = false,
                    (_, Ok(to)) => {
                        online_want = Some(to);
                        println!("vehicle: {} at the next tick every machine agrees", to.name());
                    }
                    _ => {}
                }
            }
            // Paused, this machine's rider lets go of the pad and says so in it (P84): from the tick
            // that lands on, every machine freezes this seat where it is, and the others ride on.
            // Each local player's (N20: one a view, this machine's seats first).
            let local = online_mine.len().clamp(1, args.players.min(world.seats.len()));
            let pads: Vec<Report> = (0..local)
                .map(|k| {
                    let paused = front.as_deref().is_some_and(|f| f.menu.paused(k));
                    let mut pad = match paused {
                        true => Report::idle(),
                        // Cross held down by `--online-script` (S249).
                        false if k == 0 && online_gas => Report::from_buttons(0x0040),
                        false => world.seats[k].reports[0],
                    };
                    // The machine this player picked and their biplane scheme, which every machine
                    // reads at the tick it lands on (N17).
                    let scheme = world.seats[k].garage.plane.as_ref().map_or((false, false), |p| (p.every_bank, p.round_stick));
                    // And their Markers switch (P129) and their layout's reset chord (P67), which
                    // every machine's step reads too.
                    pad.0[net::SWITCH] =
                        vehicle::Wanted {
                            to: if k == 0 { online_want } else { None },
                            every_bank: scheme.0,
                            round_stick: scheme.1,
                            dpad_rides: !world.seats[k].markers,
                            together: world.seats[k].together,
                            paused,
                        }
                        .byte();
                    pad
                })
                .collect();
            let ended = on.frame_seats(&pads, &mut world.place, rigs[0].body.as_ref());
            // The cars left on the ticks settled now, as offline's switch says them (N25).
            for said in on.cars_left(false) {
                println!("{said}");
            }
            if let Err(end) = ended {
                println!("{end}");
                if front.is_some() {
                    leave = Leave::Menu;
                }
                break;
            }
            #[cfg(target_arch = "wasm32")]
            if let Some((at, line)) = on.settled_line()
                && at > online_told
            {
                online_told = at;
                crate::host_say(&line);
            }
            // The pause's leave rows as this machine stands in the ride now (N11): a guest the room
            // was handed to hosts it, and one on its way out has nothing more to leave.
            if let Some(front) = front.as_deref_mut() {
                front.menu.leave_to = on.leaves_to();
                front.menu.session = !on.leaving();
                // and whether a player on a free pad can take a seat in it (N20): once seated, until
                // on its way out
                front.menu.seats_open = on.can_sit() && world.tape.is_none();
            }
            // Every peer's rider in their own look (N16): a seat the session took on since the ride
            // loaded (a freeride opened online, a drop-in) gets a shell seat of its own, posed and
            // drawn with no view, and a seat sat in by a player it is not dressed for is dressed
            // again, in their pick's rider, look and bike model.
            while world.seats.len() < on.seat_of().0 {
                let i = world.seats.len();
                let garage = Garage::new(Kind::Bike, lot.plane(i, flight_for(i)))?;
                let mut seat = Seat {
                    reports: [Report::missing(); PADS],
                    sim: spawn.clone(),
                    garage,
                    riding: true,
                    start_freezes: false,
                    waiting: false,
                    paused: false,
                    said: rider::Frame::default(),
                    inverted: false,
                    hands: Controls::default(),
                    teleported: false,
                    clears: 0,
                    markers: true,
                    together: false,
                    shade: PageShade::default(),
                };
                seat.sim.markers.up(vehicle::markers_of(Kind::Bike));
                world.seats.push(seat);
                // Its rig as the first player's until the room says who sits in it, just below.
                peers_dressed += 1;
                let mut dress = PeerDress {
                    disc: &mut disc,
                    assets: &mut assets,
                    kit: &mut kit,
                    rider_member: rider_member.as_deref(),
                    own: &own,
                    bike_member: &bike_member,
                    vehicle: &vehicle,
                    archives: &archives,
                    stunts: &table.stunts,
                    pack: pack_clips.as_ref().map(|c| (c, stunts.stunts.as_slice())),
                    motion: phy.motion(),
                    body_model: join_body_model.as_deref(),
                    gpu_on,
                    cameras: &join_cameras,
                    car_cameras: &car_cameras,
                };
                let (rig, made) = dress_peer(&mut dress, peers_dressed, &own)?;
                if let (true, Some(r)) = (gpu_on, renderer_of(&mut offscreen, window)) {
                    r.upload_models(&assets, &made.iter().collect::<Vec<_>>());
                }
                rigs.push(rig);
                kept_markers.push(None);
                marked_portals.push(Vec::new());
                seated.push(false);
                bike_words.push(bike_words[0].clone());
                stock_phys.push(stock_phys[0].clone());
            }
            let sat: Vec<Option<openmx_net::room::Seat>> = (0..world.seats.len()).map(|d| on.seat_pick(d).cloned()).collect();
            for d in peer_dress.changed(&sat.iter().map(Option::as_ref).collect::<Vec<_>>()) {
                let Some(taken) = &sat[d] else { continue };
                let pick = peer_looks::garage_pick(&taken.pick, &own);
                peers_dressed += 1;
                let mut dress = PeerDress {
                    disc: &mut disc,
                    assets: &mut assets,
                    kit: &mut kit,
                    rider_member: rider_member.as_deref(),
                    own: &own,
                    bike_member: &bike_member,
                    vehicle: &vehicle,
                    archives: &archives,
                    stunts: &table.stunts,
                    pack: pack_clips.as_ref().map(|c| (c, stunts.stunts.as_slice())),
                    motion: phy.motion(),
                    body_model: join_body_model.as_deref(),
                    gpu_on,
                    cameras: &join_cameras,
                    car_cameras: &car_cameras,
                };
                match dress_peer(&mut dress, peers_dressed, &pick) {
                    Ok((rig, made)) => {
                        if let (true, Some(r)) = (gpu_on, renderer_of(&mut offscreen, window)) {
                            r.upload_models(&assets, &made.iter().collect::<Vec<_>>());
                        }
                        rigs[d] = rig;
                        if hud_art.tags.len() <= d {
                            hud_art.tags.resize(d + 1, Default::default());
                        }
                        hud_art.tags[d] = overlay::Tag { name: pick.look.name.clone(), number: pick.look.number.clone() };
                        println!("online: seat {} ({}) rides {} on the {} as {:?}", d + 1, taken.peer, pick.rider, pick.bike, pick.look.text());
                    }
                    Err(e) => println!("online: seat {} ({}): not dressed: {e}", d + 1, taken.peer),
                }
                peer_dress.dress(d, taken);
            }
            // A player on this machine took a seat or gave theirs up (N20), at the tick every machine
            // took it at: a view each, laid out again for the local count as a local freeride's are
            // (P76). This machine's seats lead the shell's ([`online::Online::drawn`]), so the shell
            // seats past them are the others' again once [`online::Online::show`] fills them below.
            let mine = on.my_seats();
            if online_mine.is_empty() || mine.is_empty() {
                // a drop-in seated for the first time, or this machine on its way out
                online_mine.clone_from(&mine);
            }
            while online_mine.len() != mine.len() && mine.len() <= world.seats.len() {
                let devices: Vec<usize> = (0..online_mine.len()).map(|i| args.pad_seat(i)).collect();
                let change = match mine.iter().find(|s| !online_mine.contains(s)) {
                    Some(&s) if mine.len() > online_mine.len() => {
                        online_mine.push(s);
                        online_mine.sort_unstable();
                        // the pad seat that asked first; one this machine did not ask for (a host's
                        // restart batching two) on the next pad seat no player reads
                        let d = match online_sits.is_empty() {
                            false => online_sits.remove(0),
                            true => (0..pad::SEATS).find(|d| !devices.contains(d)).unwrap_or(0),
                        };
                        join::Change::Join(d)
                    }
                    _ => {
                        let Some(i) = online_mine.iter().position(|s| !mine.contains(s)) else { break };
                        online_mine.remove(i);
                        join::Change::Leave(i)
                    }
                };
                let n = online_mine.len();
                change.apply(&mut idle_pads, || Pads::unplugged(args.wiring.clone()));
                view_triangles.resize(n.max(view_triangles.len()), 0);
                if let join::Change::Leave(i) = change
                    && i < cameras.len()
                {
                    cameras.remove(i);
                }
                match change {
                    join::Change::Join(_) if start_readers.len() + 1 < n => start_readers.push(Reader::at(RIDE_HZ)),
                    join::Change::Leave(i) if i > 0 && i - 1 < start_readers.len() => {
                        start_readers.remove(i - 1);
                    }
                    _ => {}
                }
                args.pad_of = change.devices(&devices);
                args.players = n;
                if let Some(front) = front.as_deref_mut() {
                    front.menu.reseated(change);
                    layout = Layout::for_players(n, front.menu.party().map_or(args.split, |p| p.split))?;
                } else {
                    layout = Layout::for_players(n, args.split)?;
                }
                (rects, view_frames) = views_of(frame.width, frame.height, layout, n);
                set_display(&mut rigs, layout, display, cpresets.as_deref(), plane_presets.as_deref());
                set_heli_display(&mut rigs, layout, display, heli_presets.as_deref());
                set_car_display(&mut rigs, layout, display, &car_presets);
                if let join::Change::Join(_) = change {
                    let (seat, rig) = (&world.seats[n - 1], &mut rigs[n - 1]);
                    rig.follow(seat.garage.up);
                }
                println!(
                    "players: online, {}; views: {n} players, {}, {}x{} each",
                    match change {
                        join::Change::Join(d) => format!("player {n} joined on pad seat {}", d + 1),
                        join::Change::Leave(i) => format!("player {} left", i + 1),
                    },
                    layout.name(),
                    rects[0].w,
                    rects[0].h
                );
            }
            if online_mine.len() == mine.len() {
                online_mine = mine;
            }
            // Every seat back into the shell's, this machine's first: the others are drawn beside it.
            for (d, (seat, rig)) in world.seats.iter_mut().zip(&mut rigs).enumerate() {
                seated[d] = on.show(d, &mut seat.sim, rig.body.as_mut());
                // Frozen and see-through on the ticks its pads say it is paused (P84), as on every
                // machine.
                seat.paused = on.paused(d);
                seat.riding = !seat.paused;
                // A seat in a car (S249) is drawn in its own copy of it, the one its session seat
                // drives. The car left says where it was left from the settled ticks (N25,
                // `Online::cars_left`), not from this frame's, which can be a prediction.
                if let Some(car) = on.drawn().get(d).and_then(|&s| vehicle::lane_car(&seat.sim, on.cars(s)))
                    && !seat.garage.cars.iter().any(|c| c.machine == car.machine && c.livery == car.livery)
                {
                    seat.garage.cars.retain(|c| c.machine != car.machine);
                    seat.garage.cars.push(vehicle::CarRide { driven: None, struck: None, ..car });
                }
                // Its machine is its `Sim`'s (N17): the garage follows, and a switch the session
                // made snaps the camera onto the machine taken, as offline. Not before every seat's
                // starting machine is known (N18), or a seat starting in the plane would flick to
                // the bike and back while `--peer`s say theirs.
                if on.machines_known()
                    && let Some(up) = seat.garage.follow(&seat.sim)
                {
                    took(rig, up);
                    if let (0, Some(front)) = (d, front.as_deref_mut()) {
                        front.menu.switched(0, menu::Machine::from_word(up.name()).unwrap_or_default());
                    }
                }
            }
            // The host's change of the modifiers, or a guest's of its own seat's (P134), taken by
            // every machine at one tick (P100): the seats came back on it above; the biplane, the
            // marks and the menu say it here.
            if on.mods() != on.set_of(&mods) {
                let to = on.mods();
                for (i, seat) in world.seats.iter_mut().enumerate() {
                    if let (Some(file), Some(flight)) = (stock_plane.as_ref(), seat.garage.plane.as_mut().and_then(|p| p.flight.as_mut())) {
                        to.replane(i, file, flight);
                    }
                }
                for (i, rig) in rigs.iter_mut().enumerate() {
                    rig.modified = !to.seat_is_original(i);
                    if let Some(cam) = &mut rig.plane_camera {
                        cam.cam.ceiling = to.plane_eye_ceiling(i, Race::ceiling_for_fog(F(look.fog_far)));
                    }
                    if let Some(cam) = &mut rig.heli_camera {
                        cam.cam.ceiling = to.plane_eye_ceiling(i, Race::ceiling_for_fog(F(look.fog_far)));
                    }
                }
                mods = to;
                if let Some(front) = front.as_deref_mut() {
                    front.menu.modified = !mods.is_original();
                }
                println!("modifiers: {} (the host's, from tick {})", if mods.is_original() { "the original game's".into() } else { mods.text() }, on.remodified_at().unwrap_or(0));
            }
            world.seats[0].hands = on.hands;
            // What each rider did since the last drawn frame (P184), off the ride as drawn: the
            // session ran the rider's own frames, re-running some on a rollback, and kept none.
            let drawn = on.drawn();
            for (d, seat) in world.seats.iter_mut().enumerate() {
                seat.said = drawn.get(d).map_or_else(rider::Frame::default, |&s| online_said.of(s, &seat.sim.rider, seat.sim.plane.as_ref()));
            }
            // An opened ride's lobby (N10) is the pause's to show: its code, who is in.
            if let Some(front) = front.as_deref_mut()
                && front.menu.opening == menu::Opening::Open
            {
                for e in on.kept() {
                    front.menu.heard(e);
                }
                for c in std::mem::take(&mut front.menu.lobby.out) {
                    on.command(c);
                }
            }
            (on.dt(), true)
        } else {
        // One frame of the game's own fixed update, which calls `World::simulate` inside itself.
        // The clock a steady PS2 reads at this tick, not the host's (P52): every tick is the same
        // length whatever the picture did, and `dt` is the original's.
        let step = match landing {
            Some(mut ran) => {
                if ran.step.is_none() {
                    ran.run();
                }
                drawn_land = Some(std::mem::replace(&mut world, ran.world).place);
                engine = ran.engine;
                gauge.ticked(ran.took);
                ran.step.expect("the sim thread ran its tick")
            }
            None => {
                let now = pace::tick_clock(pacer.ticks());
                let at = Instant::now();
                let step = engine.step(now, engine_input(&world.seats[0].reports[0]), &mut world);
                gauge.ticked(at.elapsed());
                step
            }
        };
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
        if args.trace_hash {
            let hashes: Vec<String> = world.seats.iter().map(|s| format!("{:#018x}", game_sim::hash::of(&s.sim))).collect();
            println!("tick {} hash {}", pacer.ticks() + 1, hashes.join(" "));
        }
        stages.mark(stages::Stage::Sim);
        // A slot placed or cleared this frame writes its player's file (P99).
        for (kept, seat) in kept_markers.iter_mut().zip(&world.seats) {
            if let Some(line) = kept.as_mut().and_then(|k| k.frame(&markers_dir, &seat.sim.markers)) {
                println!("{line}");
            }
        }
        // The plane's particles (P58): a frame of them for each frame the plane was simulated, off
        // where it is now. Nothing here writes the simulation; the ones that collide ask the ride's own
        // terrain query for the ground under them (P61), which reads the terrain and nothing else.
        if simulated && let (Some((fx, binds)), Some(plane)) = (&mut plane_fx, &world.seats[0].garage.plane) {
            let place = &mut world.place;
            fx.step_over(&plane_fx_frame(plane, binds), dt.0, &mut |at| plane_fx_ground(place, at));
        }
        // The track's (V45): a frame for each frame simulated, as the bikes' are.
        if simulated && let Some(fx) = &mut track_fx {
            // V71: the riders in the triggers switch theirs on, and the ones on a toy ride its node
            // where the first seat's simulation has it. The bike's body rays strike them, as they
            // strike the bird triggers (V103).
            let riders: Vec<game_render::creature::Actor> = world.seats.iter().map(|s| creature_rays(&s.sim.rider)).collect();
            fx.touch_among(&riders);
            if fx.on_toys() {
                let toys = &world.seats[0].sim.traffic.toys;
                fx.place(&mut |toy, node| toy_node(&animated, toys, toy, node));
            }
            let place = &mut world.place;
            fx.step_over(dt.0, &mut |at| plane_fx_ground(place, at));
        }
        // The track's rain (V73): who rode into its triggers, then a frame of its box where the
        // camera it follows is.
        if simulated && let Some(rain) = &mut rain {
            let riders: Vec<game_render::creature::Actor> = world.seats.iter().map(|s| creature_rays(&s.sim.rider)).collect();
            rain.touch_among(&riders);
            rain.update(dt.0);
            rain.emit(cameras.get(rain.weather.camera).map(|c| c.eye));
            rain_dt = dt.0;
        }
        // The bike's tire tracks (V40): a tick of each seat's that rode its bike this frame. They read
        // the ride and write nothing back. None under `--overview` (P166).
        if simulated && !args.overview && let Some((list, _)) = &tread {
            step_tire_marks(&mut tire_marks, list, &world.seats);
            step_car_tire_marks(&mut car_marks, &car_tread, &world.seats);
        }
        if simulated {
            traffic_time += f64::from(dt.0);
            let riders: Vec<game_render::creature::Actor> = world.seats.iter().map(|s| creature_rays(&s.sim.rider)).collect();
            creatures.step_among(dt.0, &riders);
        }
        // The preload meter's movie runs a frame a tick, as the original's HUD does (V53).
        if simulated {
            for (seat, rig) in world.seats.iter().zip(&mut rigs) {
                rig.preload_meter.step(preload_sense(seat));
                match seat.garage.heli_up() {
                    // The helicopter's pilot leans with its stick (P216b).
                    Some(heli) => rig.pilot_lean.step_heli(dt, heli),
                    None => rig.pilot_lean.step(dt, seat.garage.plane_flown(), seat.said.respawned),
                }
            }
        }
        // A world loads the tiles ahead of the riders and lets the far ones go (MD8): a plane only
        // while it is flown, since every seat has one standing by the start since P113.
        if simulated && matches!(world.place, Land::Tiles(_)) {
            let near: Vec<Vf> = world
                .seats
                .iter()
                .flat_map(|s| [Some(game_sim::markers::Spot::of(&s.sim.rider).at), s.garage.plane_flown().map(|p| p.at)])
                .flatten()
                .collect();
            // A wrapped world's ground and dressing come in round each rider's copies too (MD43).
            let wrap = world.place.tiled().and_then(|t| drawn_wrap(t));
            if let Some(t) = world.place.tiled() {
                t.stream(&near);
                for line in t.take_log() {
                    println!("{line}");
                }
            }
            // Its ground on the GPU, the same way (MD17), but not while the iPhone app is away (MD97):
            // the pieces wait, and those held are built again once it is back, as what went up
            // while it went may not have.
            let in_front = gpuwin::in_front();
            if let Some(stream) = &mut ground_stream
                && in_front
                && gpuwin::times_away() != ground_away
            {
                ground_away = gpuwin::times_away();
                println!("world ground: back in front at tick {}, {} pieces built again", pacer.ticks(), stream.build_again());
            }
            if let Some(stream) = ground_stream.as_mut().filter(|_| in_front) {
                let near: Vec<[f32; 2]> = match (args.world_sweep, args.frames) {
                    (Some([x0, z0, x1, z1]), Some(n)) => {
                        let t = (pacer.ticks() as f32 / n.max(1) as f32).min(1.0);
                        vec![[x0 + (x1 - x0) * t, z0 + (z1 - z0) * t]]
                    }
                    _ => wrapped_near(wrap, &near.iter().map(|p| [p.x(), p.z()]).collect::<Vec<_>>()),
                };
                let changes = stream.update(&near);
                let written: usize = changes
                    .iter()
                    .map(|c| match c {
                        worlds::Change::Add(_, made) | worlds::Change::More(_, made) => made.chunks.len(),
                        worlds::Change::Drop(_) | worlds::Change::Show(_) => 0,
                    })
                    .sum();
                if written > 0 {
                    ground_written = (ground_written.0.max(written), ground_written.1 + written, ground_written.2 + 1);
                }
                if let Some(r) = renderer_of(&mut offscreen, window) {
                    for change in changes {
                        match change {
                            worlds::Change::Add(i, made) => r.add_ground_piece_of(i as u32, made)?,
                            worlds::Change::More(i, made) => r.more_ground_piece(i as u32, made)?,
                            worlds::Change::Drop(i) => r.fade_out_ground_piece(i as u32, GROUND_FADE),
                            worlds::Change::Show(i) => r.show_ground_piece(i as u32, GROUND_FADE),
                        }
                    }
                    let (held, room) = r.ground_chunks();
                    ground_most = (ground_most.0.max(held), ground_most.1.max(room));
                    if let Some(paint) = &mut paint_stream {
                        paint.update(&near, r);
                        for line in paint.take_log() {
                            println!("{line} at tick {}", pacer.ticks());
                        }
                    }
                }
                for line in stream.take_log() {
                    println!("{line} at tick {}", pacer.ticks());
                }
            }
            // And what stands on it (MD15).
            if let (Some(stream), Some(w)) = (&mut dress_stream, &world_mod) {
                let near = wrapped_near(wrap, &near.iter().map(|p| [p.x(), p.z()]).collect::<Vec<_>>());
                for change in stream.update(&near) {
                    match change {
                        world_dress::Change::Add(b) => {
                            let place = &mut world.place;
                            let mut d = world_dress::take(b, w, &mut assets, &mut |p, by| car_ground(place, p, by));
                            if let Some(g) = &mut d.grove {
                                g.foliage = foliage;
                            }
                            if let Some(r) = renderer_of(&mut offscreen, window) {
                                let work: Vec<_> =
                                    r.mips_wanted(&assets, &d.taken.models).into_iter().map(|(t, keep)| (t, keep, assets.textures[t].clone())).collect();
                                let (send, levels) = std::sync::mpsc::channel();
                                thread::spawn(move || {
                                    let _ = send.send(work.into_iter().map(|(t, keep, tex)| (t, keep, game_render::gpu::Mips::of(&tex, keep))).collect());
                                });
                                unshown.push((d.index, d.taken.models.iter().rev().cloned().collect(), Some(levels)));
                            }
                            dressed.push(d);
                        }
                        world_dress::Change::Drop(i) => {
                            unshown.retain(|u| u.0 != i);
                            let Some(at) = dressed.iter().position(|d| d.index == i) else { continue };
                            let d = dressed.remove(at);
                            if let Some(r) = renderer_of(&mut offscreen, window) {
                                r.drop_owner(i as u32, &d.taken.models, d.taken.textures.clone());
                            }
                            assets.let_go(&d.taken);
                            // And the software path's wheel-turned copies of its cars (V82), which name its textures.
                            assets.models.retain(|name, _| !(name.starts_with(&d.prefix) && name.contains("#car")));
                        }
                    }
                }
                for line in stream.take_log() {
                    println!("{line} at tick {}", pacer.ticks());
                }
            }
            if let Some(r) = renderer_of(&mut offscreen, window) {
                let at = Instant::now();
                while let Some((index, left, levels)) = unshown.first_mut() {
                    if let Some(made) = levels {
                        match made.try_recv() {
                            Ok(made) => {
                                for (t, keep, mips) in &made {
                                    r.put_mips(*t, *keep, mips);
                                }
                                *levels = None;
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => break,
                            // Made here as the models go up, then.
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => *levels = None,
                        }
                    }
                    while at.elapsed() < worlds::UPLOADS_A_FRAME
                        && let Some(name) = left.pop()
                    {
                        r.upload_models(&assets, &[&name]);
                    }
                    if !left.is_empty() {
                        break;
                    }
                    let index = *index;
                    unshown.remove(0);
                    if let Some(d) = dressed.iter().find(|d| d.index == index) {
                        r.add_props(index as u32, &d.props, d.grove.clone());
                    }
                }
            }
        }
        stages.mark(stages::Stage::Stream);
        let (scene_at, spent) = (Instant::now(), perf.spent());

        cameras.clear();
        ridden.clear();
        // A world that wraps (MD29): which axes, and the width a seam moves a bike or a plane by on each.
        let seams = world.place.tiled().filter(|t| t.wraps() != [false, false]).map(|t| (t.wraps(), t.extent().map(|v| v.0)));
        // The seam each seat's machine crossed this tick, and everything the picture slides from — the
        // machine's spot, the camera, the cut's spots — moved over with it ([`seam_carry`]).
        let seamed: Vec<Option<[f32; 3]>> = (0..world.seats.len())
            .map(|i| {
                let seat = &world.seats[i];
                // The flown helicopter's spot stands in the plane's (P216b).
                let heli = seat.garage.heli_up().is_some();
                let flown_before = match heli {
                    true => before_helis.get(i).copied().flatten().map(|(m, _)| m),
                    false => before_planes.get(i).copied().flatten(),
                };
                let by = seams.and_then(|s| seat_seam(s, seat, before.get(i).map(|b| b.bike), flown_before))?;
                let mut heli_slot = flown_before.filter(|_| heli);
                let slot = match heli {
                    true => Some(&mut heli_slot),
                    false => before_planes.get_mut(i),
                };
                seam_carry(by, seat.garage.plane_flown().is_some() || heli, before.get_mut(i), slot, before_cameras.get_mut(i), cuts.get_mut(i));
                if let (Some(Some((m, _))), Some(carried)) = (before_helis.get_mut(i), heli_slot) {
                    *m = carried;
                }
                Some(by)
            })
            .collect();
        // Whether a wreck rumbles a pad this frame (P183): `--no-vibration`, and the Vibration row
        // as it stands now, the pause's change taken at once.
        let vibration = args.vibration && front.as_deref().is_none_or(|f| f.menu.settings.vibration);
        wreck_rumbles.resize(world.seats.len(), pad::WreckRumble::default());
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
                    // And the nodes as this tick draws them, which the next restart fades from (F12).
                    body.drawn = riding_bones(seat, ragdolled.as_ref())
                        .map(|(bones, rigged)| body.ragdoll_palette(rigged, &bones, chassis_at(&seat.sim.rider.ride)));
                }
            }

            // And the bike under it (V9). Two halves in one call: the chassis skeleton posed from the
            // vehicle's own animation machine, and then the suspension, steering and wheel nodes written
            // over that pose from the ride's own numbers — the fork travel, the steering angle and the two
            // wheels' spin, which are not in any clip because no clip could have them.
            if let Some(chassis) = &mut rig.chassis {
                if moving && bike_up {
                    let ride = &seat.sim.rider.ride;
                    // In the trick the body's machine chose, so a trick started whipped does not
                    // leave the bike crossed up under it (V64), and in its backflip when the bike
                    // has a clip for it (MD41); without a body, the pack's own.
                    let want = match rig.body {
                        Some(_) => chassis.want_under(ride, &seat.sim.rider.tricks, seat.sim.rider.body.veh.want),
                        None => chassis.want_with_pack(ride, &seat.sim.rider.tricks),
                    };
                    // The rider's two axes and not zeros: in the air they are the crossup sweep the
                    // chassis' clip is played at, and zero is the sweep's far end (P29) — centred
                    // under a trick the bike has no clip for, so the whip comes out with the rider's (V98).
                    let request = chassis.riding_under(&seat.sim.rider, want);
                    chassis.original_wheels = args.original_wheels;
                    // Posed off the rider's motion driver, as the original's bike is (S253a).
                    perf.time(Stage::Animation, || chassis.ride(dt, want, &request, &seat.sim.rider, Drive::of(ride)));
                    // A trick the bike has no clip for: the drawn hands stay on the bars (MD41).
                    if let Some(body) = &mut rig.body {
                        body.bare = chassis.bare_under(&seat.sim.rider.body.veh);
                    }
                    if let Some(loaded) = assets.models.get_mut(&rig.bike_model).filter(|_| !gpu_on) {
                        perf.time(Stage::Skinning, || chassis.skin_into(loaded));
                    }
                }
            }
            // The grips just drawn, which the rider's riding ragdoll is held to on its next frame
            // (S110): the bones a wreck is thrown from. Offline only, like the skeleton. Drawing the
            // body from it is riding_ragdoll_on's (S90).
            if moving && bike_up && online.is_none() && seat.sim.rider.ragdoll_kit.is_some() {
                seat.sim.rider.holds = ride_holds(rig).map(Arc::new);
            }
            stages.mark(stages::Stage::Animation);

            // What the rider did, as a line: a run nobody is watching still says whether a trick
            // started, landed or put the rider down, and by which of the original's reason codes.
            // A trick pack's extras (MD16) are not in the disc's table: their names are the rider's set's.
            let name = |t: game_sim::trick::Trick| match t.extra().and_then(|_| seat.sim.rider.set.stunt(t)) {
                Some(stunt) => stunt.display_name.clone(),
                None => table.display_name(0, t).to_owned(),
            };
            if let Some(t) = seat.said.started {
                println!("{who}trick: {}", name(t));
            }
            if let Some(t) = seat.said.landed {
                println!("{who}landed: {}", name(t));
            }
            if let Some(why) = seat.said.wreck {
                println!("{who}wreck: reason {} — L1 and R1 together to get back on", why.0);
            }
            if seat.said.respawned {
                println!("{who}put back");
            }
            // The wreck's rumble (P183), the original's on a human's pad: the bike's wreck entry, or
            // the plane's crash word going up, asks this player's pad seat. Online only the players
            // at this machine have a pad here.
            let crashed = seat.garage.plane_flown().and_then(|p| p.flight.as_ref()).is_some_and(|f| f.crash.reason != 0);
            if let Some(r) = wreck_rumbles[i].frame(seat.said.wreck.is_some(), crashed, vibration)
                && i < args.players
                && let Some(seats) = pad_seats.as_mut()
            {
                seats.rumble(args.pad_seat(i), r);
            }
            // Online, the other machines' riders past the local players have no view of their own:
            // posed and drawn, no camera.
            if i >= args.players {
                continue;
            }

            // The player's framing (P82), laid over the original's presets the frame it changes: the
            // command line's, or the Camera page's — live, so a change under the pause shows as it is
            // made — or without a front end the shipped default.
            let view = args
                .camera_view
                .or_else(|| front.as_deref().map(|f| f.menu.settings.cameras[i.min(menu::SEATS_MAX - 1)]))
                .unwrap_or(menu::CameraView::DEFAULT);
            // And the seat's own presets (P163), the Camera page's too; `--camera-view` is every
            // preset's alone.
            let seat_sets = front.as_deref().filter(|_| args.camera_view.is_none()).map(|f| &f.menu.settings.camera_sets[i.min(menu::SEATS_MAX - 1)]);
            let laid = seat_sets.map(menu::cameras::Seat::framing_part).unwrap_or_default();
            if framings(view) != rig.framing || laid != rig.camera_presets {
                let bike = cpresets.as_deref().map(|b| layout_presets(b, layout, display, "cpresets.ini")).unwrap_or_default();
                let plane = plane_presets.as_deref().map(|b| layout_presets(b, layout, display, PLANE_PRESETS)).unwrap_or_default();
                println!("{who}camera view: {}", view.text());
                if laid != rig.camera_presets {
                    println!("{who}camera presets: {} of the seat's own, the button on {}", laid.0.len(), laid.1.label());
                }
                rig.camera_presets = laid;
                frame_rig(rig, &bike, &plane, framings(view));
                let heli = heli_presets.as_deref().map(|b| layout_presets(b, layout, display, game_render::kiowa::PRESETS)).unwrap_or_default();
                frame_heli(rig, &heli);
                frame_cars(rig, &cars_for_display(layout, display, &car_presets));
            }
            // The seat's first frame (P163): each camera on the preset the seat's camera button last
            // left it on, unless the command line named one; under Mine Only, a camera on a preset
            // that is not in the cycle on the first of the seat's own.
            if !std::mem::replace(&mut rig.camera_started, true) && !args.camera_asked {
                let start = seat_sets.map(|s| s.start.clone()).unwrap_or_default();
                let mine_only = rig.camera_presets.1 == menu::cameras::Cycle::Mine;
                for (cam, start) in std::iter::once(&mut rig.camera).chain(rig.plane_camera.as_mut()).zip(&start) {
                    if chase::start_on(cam, start, mine_only) {
                        println!("{who}camera: starts on {}, where it was left", cam.cam.preset().name);
                    }
                }
                // The helicopter's on its own (P216b).
                let heli_start = seat_sets.map(|s| s.heli_start.as_str()).unwrap_or_default();
                if let Some(cam) = rig.heli_camera.as_mut()
                    && chase::start_on(cam, heli_start, mine_only)
                {
                    println!("{who}camera: the helicopter's starts on {}, where it was left", cam.cam.preset().name);
                }
                // Each car's on its own (P264).
                for (kind, cam) in &mut rig.car_cameras {
                    let start = seat_sets.zip(menu::Machine::from_word(kind.name())).map(|(s, car)| s.car_start(car)).unwrap_or_default();
                    if chase::start_on(cam, start, mine_only) {
                        println!("{who}camera: the {}'s starts on {}, where it was left", kind.name(), cam.cam.preset().name);
                    }
                }
            }
            rig.wreck_camera.frame(rig.framing.0);

            // The seat's own pause (P172): its camera stays exactly where the pause's tick left it,
            // the view the last tick drew, while the others ride on; Resume carries on from there.
            // Only a Camera page change under the pause moves it (P82), on the frozen bike.
            let paused = pause_holds[i].frame(!seat.riding, rig.framing);
            if paused == chase::Paused::Held
                && let Some(held) = before_cameras.get(i)
            {
                cameras.push(held.clone());
                ridden.push(match (&seat.garage.up, &seat.sim.rider.crash) {
                    (Kind::Bike, Some(crash)) => chase::wrecked(crash),
                    _ => chase::ridden(&seat.garage, &seat.sim.rider.ride),
                });
                stages.mark(stages::Stage::Camera);
                continue;
            }
            // The pad drives the pause's menu, not the camera.
            let steered = paused == chase::Paused::Riding;

            // The camera, after the frame, which is where the original's viewport updates it: the shell
            // picks the preset, the ported camera does everything else.
            // Whichever machine has the controller has the camera: the bike's, on the track's presets, or
            // the plane's, on its own file's.
            // With the field of view's ease over a framing change (P119), which is the camera's own.
            let flying = seat.garage.up == Kind::Biplane && rig.plane_camera.is_some();
            // A car has its own (S246), on its kind's presets, sharing the bike's slots.
            let up = seat.garage.up;
            let car = rig.car_cameras.iter_mut().find(|(k, _)| *k == up);
            let (follow, fov_ease, look_back) = match (up, &mut rig.plane_camera, &mut rig.heli_camera, car) {
                (Kind::Biplane, Some(cam), _, _) | (Kind::Kiowa, _, Some(cam), _) => (cam, &mut rig.fov_ease.1, &mut rig.look_back.1),
                (_, _, _, Some((_, cam))) => (cam, &mut rig.fov_ease.0, &mut rig.look_back.0),
                _ => (&mut rig.camera, &mut rig.fov_ease.0, &mut rig.look_back.0),
            };
            // The original's plane reset (the chord, a wreck's recovery) snaps the plane's camera onto
            // the spawn rather than letting it fly the whole way back (V29). The bike's put-back does
            // too (P270, the user's ask): the chord mid-trick left the camera chasing the bike for a
            // second and more from where it had been in the air.
            if matches!(seat.garage.up, Kind::Biplane | Kind::Bike) && seat.said.respawned {
                follow.cam.snap = true;
            }
            // A jump the flight makes between two ticks (P125) — a marker's Up putting the plane
            // back into the flight it kept, or landing it by another player — takes the camera with
            // it ([`chase::follow_jump`]). Left alone, the camera lagging far behind a flying plane
            // stayed where the plane had been, looking away from it, while the plane flew on out of
            // the picture: an Up looked like it did nothing.
            else if simulated
                && let Some((plane, by)) = seat.garage.plane_flown().and_then(|p| Some((p, plane_jumped(before_planes.get(i).copied().flatten(), p, dt)?)))
            {
                chase::follow_jump(&mut follow.cam, [by.x, by.y, by.z], plane.speed());
            }
            // The helicopter's the same (P216b): a put-back after a wreck, or a marker's Up.
            else if simulated
                && let Some((heli, by)) = seat.garage.heli_up().and_then(|h| Some((h, heli_jumped(before_helis.get(i).copied().flatten().map(|(m, _)| m), h, dt)?)))
            {
                let speed = heli.flown.as_ref().map_or(F::ZERO, |h| h.ride.measure.speed);
                chase::follow_jump(&mut follow.cam, [by.x, by.y, by.z], speed);
            }
            // A marker's Up at speed (P112) snaps the bike's camera onto where it lands, riding out
            // at the bike's speed (`chase::ride_out`, after the frame) — taken once, so a frame the
            // engine skips does not snap again.
            let ride_out = std::mem::take(&mut seat.teleported) && seat.garage.up == Kind::Bike;
            if ride_out {
                follow.cam.snap = true;
            }
            // A seam the bike rode or the plane flew over (MD29, MD44) takes the camera over with it,
            // one width and no more, so it lags on from beside the machine rather than flying the
            // world's width back to it — or, measured as a jump, being carried a tick's flight too far.
            if let Some(by) = seamed.get(i).copied().flatten() {
                chase::carry(&mut follow.cam, by);
            }
            // The look-back command (P78), held: the follow camera on its look-back preset until it
            // is let go — L2 under the original's layout, Square under the new one. A ride with MX
            // vs. ATV's tricks has those buttons as its two extra tricks, so it has no look-back.
            // Under the thrill camera (P157) it takes no press, as the original's does not.
            if steered && seat.sim.phy.tricks.is_none() && look_back.frame(&mut follow.cam, seat.sim.input.pad(0), seat.sim.table(), !rig.wreck_camera.on && !rig.thrill.up) {
                println!("{who}camera: {}", follow.cam.preset().name);
            }
            let presets = follow.cam.presets.len();
            if steered && presets > 1 {
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
                    rig.ride_hud.note_camera(Instant::now(), &follow.cam.preset().name);
                    // The seat's next ride on this machine starts on it (P163).
                    // A car's preset is its own file's (S246), kept as the car's start (P264).
                    let machine = match seat.garage.up {
                        Kind::Kiowa => menu::Machine::Helicopter,
                        _ if flying => menu::Machine::Biplane,
                        up => menu::Machine::from_word(up.name()).filter(|_| up.car().is_some()).unwrap_or(menu::Machine::Bike),
                    };
                    if let Some(f) = front.as_deref_mut()
                        && f.menu.camera_left_on(i.min(menu::SEATS_MAX - 1), machine, &follow.cam.preset().name)
                    {
                        f.save();
                    }
                }
            }
            // The camera's target block, as the original's vehicle fills it once a frame (`chase.rs`).
            // The airborne flag is the one the camera heads along the velocity on: in the air the
            // backflip's upside-down forward row never reaches it (P34).
            let rider = match (&seat.garage.up, &seat.sim.rider.crash) {
                (Kind::Bike, Some(crash)) => chase::wrecked(crash),
                _ => chase::ridden(&seat.garage, &seat.sim.rider.ride),
            };
            // The other-camera command (P157), held: the thrill camera has the screen while the bike
            // is going faster than 3 ft/s as it goes down, until it is let go — R2 under the original's
            // layout, Cross under the new one. It is the bike's camera manager's: with the plane up,
            // or a ride with MX vs. ATV's tricks on those buttons, there is none.
            if seat.garage.up != Kind::Bike || seat.sim.phy.tricks.is_some() {
                rig.thrill.up = false;
            } else if steered
                && rig.thrill.frame(&mut follow.cam, look_back, seat.sim.input.pad(0), seat.sim.table(), !rig.wreck_camera.on, &rider, F(NEAR_CLIP), &mut world.place)
            {
                println!("{who}camera: {}", if rig.thrill.up { "thrill" } else { follow.cam.preset().name.as_str() });
            }
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
            // The seat's right stick turns its own follow camera, L3 held zooms it, and R3 puts it
            // back (P114) — while the follow camera has the screen, so a wreck's camera is not
            // nudged from under the player. The preset changes above keep or throw the nudges away
            // as the original's do.
            if steered && presets != 0 && !wreck.on {
                let pad = seat.sim.input.pad(0);
                let scheme = pad_seats.as_ref().map_or(args.wiring.schemes[i], |s| s.scheme(args.pad_seat(i)));
                chase::look_around(follow, pad, seat.sim.table(), scheme.resets_camera(pad));
            }
            // The shape of the view this camera draws into (V22), which on a full screen is the frame.
            let (view_w, view_h) = (rects[i].w, rects[i].h);
            let view = if presets == 0 {
                None
            } else {
                // The camera's two questions about the world, both the ground under a point, which
                // the track answers: under the look-at point (S23), without which the view sinks into
                // the dirt on the first dip, and under the eye, which keeps it 4.8 ft over the ground
                // (S103). This frame has no obstacle push-out — the base class's did, and the base
                // class's frame is not the one the disc runs.
                let mut view = follow.update(
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
                );
                if ride_out {
                    chase::ride_out(follow, &mut view, rider.target.vel, dt);
                }
                view.fov = fov_ease.ease(view.fov, dt.0);
                // The plane kept in the picture through a climb or a loop (P275), which the original's
                // camera, looking along the plane's flattened heading, loses off the top.
                if flying {
                    let fov = aspect::fov(view.fov.0, view_w, view_h);
                    chase::keep_in_view(&mut view, rider.target.body_point, fov, view_w as f32 / view_h.max(1) as f32);
                }
                Some(view)
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
            // The thrill camera over both (P157): the original's wreck switch leaves the screen to it.
            let view = match (view, rig.thrill.view(&rider)) {
                (Some(_), Some(thrill)) => Some(thrill),
                (view, _) => view,
            };
            if let (true, Some(v)) = (args.trace_camera, &view) {
                let c = &follow.cam;
                println!(
                    "camera {i} {} {} preset {:?} distance {} pitch {} yaw {} fov {} eye {} {} {} at {} {} {}",
                    pacer.ticks(),
                    if rig.thrill.up { "thrill" } else if wreck.on { "wreck" } else { "follow" },
                    c.preset().name,
                    c.distance.0,
                    c.pitch.0,
                    c.yaw.0,
                    v.fov.0,
                    v.position.x(),
                    v.position.y(),
                    v.position.z(),
                    v.look_at.x(),
                    v.look_at.y(),
                    v.look_at.z(),
                );
            }
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
                    // The software renderer's far plane: 4,000 feet, or the scene's fog's end where
                    // that is further (MD3: a mod's map may fog out past it). The GPU's is its own
                    // (`Graphics::far_clip`).
                    far: lights.get(i).unwrap_or(&light).fog_far.max(4000.0),
                },
                None => Camera::behind(pos, bike_look),
            });
            ridden.push(rider);
            stages.mark(stages::Stage::Camera);
        }
        // The animated toys that step only when drawn (V76): whether this tick's picture draws each,
        // in any view, which the next tick's step reads, as the original's update reads the draw
        // after it. Every seat's copy of the toys is told the same. Online the toys are the
        // session's, rolled back and stepped alike on every machine, which no one machine's picture
        // may steer: they all step.
        if simulated && online.is_none() && toy_bounds.iter().any(Option::is_some) {
            let seen: Vec<bool> = toy_bounds
                .iter()
                .enumerate()
                .map(|(k, b)| {
                    let shown = &world.seats[0].sim.traffic.toys;
                    b.is_none_or(|b| {
                        cameras.iter().zip(&rects).any(|(c, r)| game_render::animated_toy::drawn(shown, k, b, c, r.w, r.h))
                    })
                })
                .collect();
            for seat in &mut world.seats {
                if seat.sim.traffic.toys.seen.len() == seen.len() {
                    seat.sim.traffic.toys.seen.clone_from(&seen);
                }
            }
        }
        // The bikes' (P74): a frame for each seat, off its rider as the simulation left them and its
        // chassis as this tick posed it (P162: read before the pose, the tire nodes were the tick
        // before's swing arm and bars on this tick's placement). A seat
        // with the plane up has no bike in the world: its emitters are off and what is in the air
        // runs out. Nobody's under `--overview` (P166), which draws no bike.
        if simulated && !args.overview && let Some(list) = &bike_fx_list {
            bike_fx.resize_with(world.seats.len(), || list.clone());
            let place = &mut world.place;
            for ((seat, rig), fx) in world.seats.iter().zip(&rigs).zip(&mut bike_fx) {
                let frame = match seat.garage.up {
                    Kind::Bike => bike_fx_frame(fx, &seat.sim.rider, rig, &assets),
                    _ => particles::PlaneFrame::default(),
                };
                fx.step_over(&frame, dt.0, &mut |at| plane_fx_ground(place, at));
            }
        }
        // The Kiowas' (V114): a frame for each seat flying one, off its flight after the tick. A seat
        // not in one has its emitters off, and what is in the air runs out.
        if simulated && !args.overview && let Some((list, binds)) = &heli_fx_list {
            heli_fx.resize_with(world.seats.len(), || list.clone());
            let place = &mut world.place;
            for (seat, fx) in world.seats.iter().zip(&mut heli_fx) {
                let frame = seat.garage.heli_up().map_or_else(particles::PlaneFrame::default, |h| heli_fx_frame(h, binds));
                fx.step_over(&frame, dt.0, &mut |at| plane_fx_ground(place, at));
            }
        }
        // The cars' (S248): each seat's manager for the car it drives off the car after the tick.
        if simulated && !args.overview && !car_fx_lists.is_empty() {
            car_fx.resize_with(world.seats.len(), || car_fx_lists.iter().map(|(_, fx, _)| fx.clone()).collect());
            let place = &mut world.place;
            for (seat, fxs) in world.seats.iter().zip(&mut car_fx) {
                let up = seat.garage.car_up();
                for ((machine, _, binds), fx) in car_fx_lists.iter().zip(fxs) {
                    let frame = up.filter(|c| c.machine == *machine).and_then(|c| Some((c, c.driven.as_ref()?))).map_or_else(particles::PlaneFrame::default, |(c, m)| {
                        let at = car_draw::placed(m);
                        particles::PlaneFrame::car(m, binds.iter().map(|(n, b)| (n.clone(), b.mul(at))).collect(), c.struck.as_ref())
                    });
                    fx.step_over(&frame, dt.0, &mut |at| plane_fx_ground(place, at));
                }
            }
        }
        perf.lap(Stage::Scene, scene_at, spent);

        // The frame's sound (P42), off what the ride just did, heard from every view's camera. A
        // ride nobody is riding (the pause, a frozen ride) is quiet, and starts again from idle.
        // A seat paused while others ride holds its own sounds where they are (P172).
        if let Some(sound) = &mut sound {
            if world.seats.iter().any(|seat| seat.riding) {
                let ears: Vec<audio::Listener> = cameras.iter().map(listener).collect();
                sound.listen(&ears);
                let area = sim_world::World::area(&world.place);
                let frames: Vec<audio::SeatFrame> = world.seats.iter().map(|s| seat_sound(s, area, &world.place)).collect();
                // By the first seat's cars, which are the `.toy`'s as long as none was left out.
                if !sound.toys.is_empty() && world.seats[0].sim.traffic.cars.len() == sound.toys.len() {
                    let cars: Vec<[f32; 3]> = world.seats[0].sim.traffic.cars.iter().map(|c| {
                        let t = c.pose.0[3].0;
                        [t[0] + car_shift[0], t[1], t[2] + car_shift[1]]
                    }).collect();
                    sound.toys_at(&cars);
                }
                sound.frame(&frames, dt.0);
                // The material table's collision sounds (F27a): what this tick's probes struck.
                if simulated {
                    let ride = HeardRide { seats: &world.seats };
                    let mut queue = game_sim::material_hit::queue(&ride.heard(), &ride);
                    sound.hits(&mut queue);
                }
                hushed = false;
            } else if !hushed {
                sound.quiet();
                hushed = true;
            }
        }

        pacer.ran();
        ticked = true;
        continue;
        }
        }

        // The picture, owing no tick (P52): every camera, bike, rider and the plane blended from the
        // tick before the last to the last by how far the wall clock is past it. Render-side only:
        // nothing below writes the simulation.
        let blend = pace::Blend(pacer.blend(moment));
        let drawing_at = Instant::now();
        // A tick sent off above runs on the sim thread while this picture is drawn (N23a).
        let drawn = sim_thread::beside(sim_worker.as_ref().zip(flight.as_mut()), Ticking::run, || -> Result<(), String> {
        // A world's pieces fading in over their stand-in (V92), on the picture's own clock.
        if ground_stream.is_some() {
            let shown = (pacer.ticks() as f32 - 1.0 + blend.0).max(0.0) * pace::TICK.as_secs_f32();
            if let Some(r) = renderer_of(&mut offscreen, window) {
                r.advance_ground_fades((shown - ground_shown).max(0.0));
            }
            ground_shown = shown;
        }
        // The path cars at the picture's own moment (V47): where the first seat's simulation stood
        // them on the tick before the last and on the last (S81), blended by the bike's blend, so they
        // move every frame drawn and are drawn where the bike meets them.
        let driven = &world.seats[0].sim.traffic.cars;
        if !traffic.cars.is_empty() && driven.len() == traffic.cars.len() {
            traffic_at = traffic
                .cars
                .iter()
                .zip(driven)
                .enumerate()
                .map(|(k, ((car, _), now))| (car.stem().to_string(), car_between(cars_before.get(k).unwrap_or(&now.pose), &now.pose, blend.0)))
                .collect();
            for (_, at) in &mut traffic_at {
                at.0[3][0] += car_shift[0];
                at.0[3][2] += car_shift[1];
            }
            let shown = game_render::toy::between(traffic_before, traffic_time, blend.0);
            traffic_pose = traffic
                .cars
                .iter()
                .zip(&traffic_wheels)
                .map(|((car, path), wheels)| {
                    let model = &assets.models.get(car.stem())?.model;
                    let wheels = wheels.as_ref()?;
                    Some((wheels.palette_at(model, car, path, shown), wheels.clone()))
                })
                .collect();
        }
        if !creature_copies.is_empty() {
            creatures_at = creatures.placed(blend.0);
        }
        // The knockable props (S131) where the first seat's bodies are, on the last tick: both
        // pictures draw the scene's list, so a moved prop is moved there.
        for ((_, node), seen) in world.seats[0].sim.traffic.props.nodes().zip(&prop_looks) {
            let Some((k, made, rest)) = seen else { continue };
            let shown = match node == rest {
                true => made.clone(),
                false => {
                    let xyz = |v: Vf| v3(v.x(), v.y(), v.z());
                    view::Placement { model: made.model.clone(), pos: xyz(node.origin), look: xyz(node.rows[2]), up: xyz(node.rows[1]) }
                }
            };
            let drawn = &mut look.models[*k];
            if (drawn.pos, drawn.look, drawn.up) != (shown.pos, shown.look, shown.up) {
                if let Some(r) = renderer_of(&mut offscreen, window) {
                    r.move_scene_prop(*k, shown.transform());
                }
                *drawn = shown;
            }
        }
        // The animated toys (V43) where the first seat's simulation has them: at their places, posed
        // at their clips' clocks blended between the tick before the last and the last.
        let shown_toys = &world.seats[0].sim.traffic.toys;
        let toys_at: Vec<(&str, M4, Vec<M4>)> = match shown_toys.specs.len() == animated.models.len() {
            true => (0..animated.models.len())
                .map(|k| {
                    use game_render::animated_toy::{palette, place};
                    (animated.models[k].as_str(), place(shown_toys, k), palette(shown_toys, k, blend.0))
                })
                .collect(),
            false => Vec::new(),
        };
        // A world's are each tile's own (MD15), on the same clock, each moved by its tile's offset.
        if !dressed.is_empty() {
            let place = &mut world.place;
            let shown = game_render::toy::between(traffic_before, traffic_time, blend.0);
            traffic_at.clear();
            // The far ones less often (MD28).
            let eyes: Vec<[f32; 2]> = cameras.iter().map(|c| [c.eye.x, c.eye.z]).collect();
            car_pictures += 1;
            traffic_pose.clear();
            for d in dressed.iter_mut().filter(|d| !d.traffic.cars.is_empty() && !unshown.iter().any(|u| u.0 == d.index)) {
                traffic_at.extend(d.cars_near(shown, &eyes, car_pictures, &mut |p, by| car_ground(place, p, by)));
                traffic_pose.extend(d.wheel_poses(shown, &eyes, &assets));
            }
        }
        // The markers' slot display (P88), each view its own player's: the selection, the slots, the
        // label and the hold, or `None` while it is down. Online the selection's players are the
        // session's seats, by the lineup's names.
        banners.resize_with(world.seats.len(), hud::MarkerBanner::default);
        // Only the players in the ride are picks (P94): online a seat nobody sits in is not, as the
        // session's ticks count it (`net::rides`); offline every seat is somebody's.
        let (names, riding, mine) = match &online {
            Some(on) => {
                let names = on.names();
                let riding: Vec<bool> = names.iter().map(Option::is_some).collect();
                (names, riding, Some(on.seat_of().1))
            }
            None => (Vec::new(), vec![true; world.seats.len()], None),
        };
        // Each view's selection, `None` where it shows no markers: online every view but this
        // machine's own. On every machine, of the set of the machine up (P124).
        let picks: Vec<Option<markers::Pick>> = world
            .seats
            .iter()
            .enumerate()
            .map(|(v, seat)| {
                hud::markers_shown_in(v, mine.is_some()).then(|| seat.sim.markers.pick_among(mine.unwrap_or(v), &riding))
            })
            .collect();
        let shown_markers: Vec<Option<MarkerShown>> = world
            .seats
            .iter()
            .zip(banners.iter_mut())
            .enumerate()
            .map(|(v, (seat, banner))| {
                picks[v]?;
                // A player with their markers off (P129) has no bar: the d-pad is not the markers'.
                if !seat.markers {
                    return None;
                }
                let me = mine.unwrap_or(v);
                // The spawn rides to the machine up's start where the map has one (P132).
                let home = seat.sim.home.has(seat.sim.markers.on, me);
                let bar = hud::MarkerBar::read(&seat.sim.markers, me, &riding, seat.sim.input.pad(0), seat.sim.table(), seat.riding).homed(home);
                let now = Instant::now();
                banner.cleared(now, seat.clears);
                let shown = banner.update(now, bar.pick, &bar.filled, bar.held);
                let bar = bar.cleared(banner.says_cleared());
                let label = bar.label(&names);
                let hud::MarkerBar { pick, filled, holding, home, cleared, .. } = bar;
                let others = markers::Markers::others(me, riding.len(), &riding);
                shown.then(|| (pick, filled, label, others, holding, home, cleared))
            })
            .collect();
        // The bays shown (P103), drawn below and marked on the radar.
        let bays = vehicle::bays_shown(&lot, front.as_deref().is_none_or(|f| f.menu.settings.bays));
        // And the Kiowas parked (V113), bays since P216.
        let kiowa_bays: Vec<vehicle::Bay> = bays.iter().filter(|b| b.machine == Machine::Helicopter).copied().collect();
        // The radar's other icons, each view its own (P92).
        hud_art.radar = (0..world.seats.len()).map(|v| radar_blips(&world.seats, &bays, v, &radar_zones, picks[v])).collect();
        // And the switch pop-up, each view its own player's (P103), as the input stage's take reads it.
        let offering = head.is_none() && online.is_none();
        hud_art.offers = world.seats.iter().map(|seat| offering.then(|| seat_offer(seat, &bays)).flatten()).collect();
        // Which seats' pictures cut this frame (P120): a put-back, or a move far past the seat's own
        // speed. A seat's machine, rider and camera cut or slide together.
        let cut: Vec<bool> = world
            .seats
            .iter()
            .enumerate()
            .map(|(p, seat)| {
                let bike = match &seat.sim.rider.crash {
                    Some(crash) => chassis_wrecked(crash),
                    None => chassis_at(&seat.sim.rider.ride),
                };
                cuts.get(p).is_none_or(|c| c.cuts(cut_spots(seat, bike, cameras.get(p)), seat.sim.rider.drawn_jumps()))
            })
            .collect();
        // The settings' minimap and HUD scale (V52), for the original's HUD and the shell's.
        hud_art.minimap = minimap;
        hud_art.scale = f32::from(hud_scale) / 100.0;
        let cameras: Vec<Camera> = cameras
            .iter()
            .enumerate()
            .map(|(v, c)| match (before_cameras.get(v), cut.get(v)) {
                (Some(b), Some(false)) => blend.pan(b, c),
                _ => *c,
            })
            .collect();
        if args.trace_picture {
            trace_picture(&world.seats, &rigs, &assets, ragdolled.as_ref(), &before, &cameras, &cut, &blend, pacer.ticks(), &before_planes, &before_helis);
        }
        // `--shot-pose` (MD10): the first view looks from the pose instead, whatever the ride does.
        let mut cameras = cameras;
        if let (Some(pose), Some(at), Some(camera)) = (args.shot_pose, rects.first(), cameras.first_mut()) {
            *camera = pose.camera(at.w, at.h, NEAR_CLIP, camera.far);
        }
        // And `--shot-pose` again, the views after it (P142).
        for ((pose, at), camera) in args.more_poses.iter().zip(rects.iter().skip(1)).zip(cameras.iter_mut().skip(1)) {
            *camera = pose.camera(at.w, at.h, NEAR_CLIP, camera.far);
        }
        // A world's sky and light (MD9): under Per Map the tile each view's camera is over sets its
        // own (P142), mixed across a join and its sky the larger share's; under Global the whole
        // world's sky and fog for every view, but the sun and ambient still the map's below (V116).
        if let Some(wl) = &world_look {
            let views = cameras.len().min(world.seats.len());
            shown_look.resize(views, None);
            shown_skies.resize(views, None);
            lights.clear();
            for (v, camera) in cameras.iter().enumerate().take(views) {
                // `over` is the map whose sky and fog these are, `sun_of` the one whose sun.
                let (lit, over, sun_of) = match world_sky {
                    menu::WorldSky::PerMap => match wl.looks.at(camera.eye.x, camera.eye.z) {
                        Some((lit, over)) => (lit, Some(over), Some(over)),
                        None => (Lighting::of(&look), None, None),
                    },
                    menu::WorldSky::Global => match wl.looks.sunlit_at(camera.eye.x, camera.eye.z, Lighting::of(&look)) {
                        Some((lit, sun_of)) => (lit, None, Some(sun_of)),
                        None => (Lighting::of(&look), None, None),
                    },
                };
                lights.push(lit);
                // The skybox crossfades across a join (MD26): the two maps' skies bound while the eye
                // is between them, the nearer's share of the picture growing as it is ridden into.
                let skies = match world_sky {
                    menu::WorldSky::PerMap => wl.looks.skies(camera.eye.x, camera.eye.z),
                    menu::WorldSky::Global => None,
                };
                if let Some(r) = renderer_of(&mut offscreen, window) {
                    let pair = skies.map(|(pair, _)| pair);
                    if shown_skies[v] != Some(pair) {
                        shown_skies[v] = Some(pair);
                        // Each map's cube is uploaded the first time a view is over it, and kept.
                        r.set_view_skies(v, pair.map(|p| p.map(|m| (m, wl.skies[m].as_ref()))));
                    }
                    r.set_view_sky_mix(v, skies.map_or(0.0, |(_, share)| share));
                }
                if shown_look[v] != Some((over, sun_of)) {
                    shown_look[v] = Some((over, sun_of));
                    let name = over.map_or("the whole world", |i| wl.names[i].as_str());
                    let sun_name = sun_of.map_or("the whole world", |i| wl.names[i].as_str());
                    println!(
                        "world sky: over {name} at {:.0},{:.0} in view {}: sun of {sun_name} {:.3},{:.3},{:.3} ambient {:.3},{:.3},{:.3} fog {},{},{} {:.0}..{:.0}",
                        camera.eye.x,
                        camera.eye.z,
                        v + 1,
                        lit.sun.x,
                        lit.sun.y,
                        lit.sun.z,
                        lit.ambient[0],
                        lit.ambient[1],
                        lit.ambient[2],
                        lit.fog_color[0],
                        lit.fog_color[1],
                        lit.fog_color[2],
                        lit.fog_near,
                        lit.fog_far
                    );
                }
            }
            light = lights.first().copied().unwrap_or(light);
            if let Some(r) = renderer_of(&mut offscreen, window) {
                r.set_view_lights(&lights);
            }
        }
        // The start zones' and the markers' portals (V51), each view its own: its player's markers,
        // and the zones that player is shown.
        for (v, seat) in world.seats.iter().enumerate() {
            portal::markers(&mut marked_portals[v], &seat.sim.markers, picks[v], |x, y, z| portal_ground(&mut world.place, x, y, z));
        }
        let mut glows: Vec<Vec<game_render::glow::Glow>> = match &portal_art {
            Some(art) => (0..world.seats.len())
                .map(|v| {
                    let in_machine = world.seats[v].garage.up != Kind::Bike;
                    let seconds = clock.elapsed().as_secs_f32();
                    cameras.get(v).map_or_else(Vec::new, |c| portal_glows(art, &zone_portals, &marked_portals[v], in_machine, c, seconds))
                })
                .collect(),
            None => Vec::new(),
        };
        // The tire tracks (V40), every seat's in every view, laid under the portals.
        if let Some(texture) = tread_texture {
            let mut marks = tire_mark_glow(&tire_marks, texture);
            marks.tris.extend(car_marks.iter().flat_map(|m| tire_mark_glow(m, texture).tris));
            glows.resize_with(world.seats.len(), Vec::new);
            for view in &mut glows {
                view.insert(0, marks.clone());
            }
        }
        // The rain (V73), over the portals, in every view, each turned to its own eye and drawn where
        // it was between the last two ticks.
        if let (Some(rain), Some(texture)) = (&rain, rain_texture) {
            glows.resize_with(world.seats.len(), Vec::new);
            for (view, camera) in glows.iter_mut().zip(&cameras) {
                view.push(rain.glows(texture, camera.eye, (1.0 - blend.0) * rain_dt, RAIN_TINT));
            }
        }
        // The sun's flares (V72), over everything else the views lay, each by its own view's camera
        // and at the share of its sample rays that reach the sky.
        if !sun_flares.is_empty() {
            glows.resize_with(world.seats.len(), Vec::new);
            for (v, view) in glows.iter_mut().enumerate() {
                let (Some(camera), Some(at)) = (cameras.get(v), rects.get(v)) else { continue };
                for (flare, texture) in &sun_flares {
                    if !flare.on_screen(camera, at.w, at.h) {
                        continue;
                    }
                    // Asked again once a simulated frame, not every picture: the ground query is
                    // the simulation's, and a few hundred of them cost a millisecond.
                    let key = (v, *texture);
                    let sky = match flare_sky.get(&key) {
                        Some(&(tick, sky)) if tick == pacer.ticks() => sky,
                        _ => {
                            let rays = flare.sample_rays(camera);
                            let sky = game_render::sun_flare::sky_share(camera.eye, &rays, camera.far, |x, _, z| portal_ground(&mut world.place, x, 1.0e4, z));
                            flare_sky.insert(key, (pacer.ticks(), sky));
                            sky
                        }
                    };
                    view.extend(flare.glow(camera, at.w, at.h, sky, *texture));
                }
            }
        }

        // The pictures, one a view (V22), each drawn by its own player's camera into a frame of its
        // own shape and put into the window's frame; a full screen's one view is the frame itself.
        // Each player's body is skinned at the level of detail the camera drawing it has earned, so
        // a rider is re-skinned for a view only where that differs from the view before.
        // The Tricks page's rider (P259), posed once a frame for each paused view showing the page.
        let trick_shown = front.as_deref_mut().map_or(Vec::new(), |f| trick_view::shown(&mut f.menu, &rects, world.seats.len()));
        if trick_stage.is_none() && !trick_shown.is_empty() {
            let rider = rider_member.as_deref().map(|m| (m, args.rider.as_str()));
            let bike = Some((bike_member.as_str(), vehicle.mcf.as_str()));
            let bike_model = rigs.first().map_or(CHASSIS, |r| r.bike_model.as_str());
            trick_stage = Some(trick_view::load(&mut disc, rider, bike, BIKE_PREFIX, &table.stunts, phy.motion(), pack_clips.as_ref(), &stunts, &assets, bike_model));
        }
        let trick_posed: Vec<([usize; 4], trick_view::Posed)> = match trick_stage.as_mut().and_then(Option::as_mut) {
            Some(stage) => {
                let rows: Vec<&menu::trick_list::TrickRow> = trick_shown.iter().map(|(_, row)| row).collect();
                trick_shown.iter().map(|(rect, _)| *rect).zip(stage.poses(&rows, Instant::now())).collect()
            }
            None => Vec::new(),
        };
        let mut skinned: Vec<Option<usize>> = vec![None; rigs.len()];
        let mut drawn = 0;
        // Each flying player's pilot (V39), posed once for every view and both renderers.
        let pilots = pilots(&world.seats, &mut rigs, &plane_drawn, &before_planes, &cut, &blend);
        // And each flying the helicopter's (P216b).
        let heli_pilots = heli_pilots(&world.seats, &mut rigs, &before_helis, &cut, &blend);
        let car_drivers = car_drivers(&world.seats, &mut rigs, &assets, &before_cars, &cut, &blend);
        // Each view's buttons and the join cue's, under Button Icons (P281).
        let hud_buttons: Vec<menu::pads::Buttons> = (0..world.seats.len()).map(|v| front.as_deref().map(|f| f.menu.seat_buttons(v)).unwrap_or_default()).collect();
        let join_buttons = front.as_deref().map(|f| f.menu.join_buttons()).unwrap_or_default();
        if gpu_on {
            // The overlay first: each view's HUD, and the pause over the whole frame.
            let draw_at = Instant::now();
            gpu_sprites.clear();
            let join_view = join_cue.then(|| cue_view(&rects, frame.width));
            for (v, at) in rects.iter().enumerate().take(world.seats.len()) {
                let (seat, rig) = (&world.seats[v], &mut rigs[v]);
                // The page's black over the view (S85), first, so the HUD is drawn over it.
                if seat.shade.alpha() != 0 {
                    menu::ink::fill(&mut gpu_sprites, [at.x as f32, at.y as f32, at.w as f32, at.h as f32], [0, 0, 0, seat.shade.alpha()]);
                }
                // The ride HUD (P121), with its own marker menu, trick pop-up and info text, unless
                // `none` was asked for.
                if overlay_mode == menu::Overlay::Hud {
                    let marker = shown_markers.get(v).and_then(Option::as_ref);
                    let view = [at.x as f32, at.y as f32, at.w as f32, at.h as f32];
                    let join = join_view == Some(v);
                    let ctx = RideHud { ridden: &ridden, art: &hud_art, span: radar_span, maps: &map_names, wrap: hud_wrap, speedometer, names: &names, table: &table, simulated, join, buttons: &hud_buttons, join_buttons: &join_buttons };
                    ctx.with(seat, rig, v, marker, |shown, tracker, now| {
                        // clear of the page's touch pad while it is up (P208)
                        let pad = menu::touch_room::Room::of((frame.width, frame.height));
                        ride_hud::sprites_clear_of(&mut gpu_sprites, hud_base, view, pad, shown, tracker, now);
                    });
                }
            }
            // The Show FPS counter (2026-10-29), once over the whole frame, under the pause.
            if show_fps {
                let whole = [0.0, 0.0, frame.width as f32, frame.height as f32];
                ride_hud::fps_sprites(&mut gpu_sprites, hud_base, whole, f32::from(hud_scale) / 100.0, &fps_meter.text());
            }
            if let Some(front) = front.as_deref_mut() {
                // As sprites (V30), after the HUDs' and so over them; the patches go under both. Over a
                // split each player's own pause in their own view (P84), the whole frame's otherwise.
                let (art, menu) = (&front.art, &mut front.menu);
                for (v, at) in menu.pause_views(&rects, world.seats.len()) {
                    let from = gpu_sprites.len();
                    menu.with_seat(v, |m| art.sprites(m, (at.w, at.h), menu_base, &mut gpu_sprites));
                    for corner in gpu_sprites[from..].iter_mut().flat_map(|s| &mut s.corners) {
                        corner[0] += at.x as f32;
                        corner[1] += at.y as f32;
                    }
                }
            }
            perf.add(Stage::Draw, draw_at.elapsed());
            stages.mark(stages::Stage::Hud);

            // What stands in the world besides the props, once for every view: each bike, its parts
            // and its rider, posed by node matrices the vertex shader skins with.
            let (scene_at, spent) = (Instant::now(), perf.spent());
            gpu_poses.clear();
            let mut actors: Vec<Actor> = Vec::new();
            // Seat 0's bike and rider in the list (P212a), to ask the frame what each drew.
            let (mut bike_actor, mut rider_actor) = (None, None);
            // Seat 0's body drawn thrown, where the ragdoll lies and not on the bike (P224).
            let mut rider_thrown = false;
            for (p, (seat, rig)) in world.seats.iter().zip(&rigs).enumerate() {
                if seat.garage.up != Kind::Bike || !seated[p] {
                    continue;
                }
                let now = perf.time(Stage::Skinning, || Shown::of(seat, rig, &assets, ragdolled.as_ref(), true));
                // A put-back is a cut: the bike and the rider are drawn where it set them, not slid
                // there from the wreck (S66), and so is the body's first tick back on the bike (S74).
                // So is any other jump, the seat's camera with them (P120).
                let shown = match before.get(p) {
                    Some(b) if !cut[p] => b.blend(&now, &blend),
                    _ => now.clone(),
                };
                let bike_at = shown.bike;
                let pose = (!shown.chassis.is_empty()).then(|| {
                    gpu_poses.push(shown.chassis.clone());
                    gpu_poses.len() - 1
                });
                // See-through for a while after a put-back, the bike, its parts and the rider alike (P57).
                // And while the player's own pause is open (P84). Their shadow stays whole all the while
                // (user 2026-10-30: 'when respawning the shadow isnt present for a few seconds, make it
                // always there'): `casts`.
                let alpha = vehicle::shown_alpha(seat.sim.rider.ghost.shown(), seat.paused);
                if p == 0 {
                    bike_actor = Some(actors.len());
                    rider_thrown = shown.thrown;
                }
                actors.push(Actor { model: rig.bike_model.as_str(), at: bike_at, pose, lod_from: None, alpha, casts: true });
                // A wheel spinning fast is drawn blurred (V59): see-through copies of it trailing
                // behind, the chassis' own model with every other node shrunk away. Not on a put-back
                // ghost, which is see-through already.
                if let (Some(chassis), true) = (&rig.chassis, alpha >= 1.0) {
                    for copy in chassis.blur_copies(&shown.chassis) {
                        gpu_poses.push(copy.pose);
                        let pose = Some(gpu_poses.len() - 1);
                        actors.push(Actor { model: rig.bike_model.as_str(), at: bike_at, pose, lod_from: None, alpha: copy.alpha, casts: false });
                    }
                }
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
                    actors.push(Actor { model: part.model.as_str(), at, pose: None, lod_from: None, alpha, casts: true });
                }
                // The body, thrown by the ragdoll's bones while the rider is down (S53) and on the
                // bike otherwise ([`Shown::of`]).
                if let (Some(pose), Some(model)) = (shown.body, &rig.body_model) {
                    gpu_poses.push(pose);
                    if p == 0 {
                        rider_actor = Some(actors.len());
                    }
                    actors.push(Actor {
                        model: model.as_str(),
                        at: bike_at,
                        pose: Some(gpu_poses.len() - 1),
                        // The finest, always: a rider is the player's own, and thrown off the edge (S59)
                        // the camera falls far enough behind to earn the coarsest group.
                        lod_from: None,
                        alpha,
                        casts: true,
                    });
                }
            }
            // Every player's plane, in its own livery (P87), while it is flown.
            for (p, seat) in world.seats.iter().enumerate() {
                let Some(plane) = seat.garage.plane_flown().filter(|_| !args.overview) else { continue };
                let Some(drawn) = plane_drawn.get(plane.livery).or(plane_drawn.first()) else { continue };
                let before = before_planes.get(p).copied().flatten().filter(|_| !cut[p]);
                let at = before.map_or(plane_at(plane), |b| blend.slide(b, plane_at(plane)));
                let alpha = vehicle::shown_alpha(1.0, seat.paused);
                let was = before_surfaces.get(p).and_then(Option::as_ref).filter(|_| !cut[p]);
                let surfaces = plane_draw::shown(was, plane, blend.0);
                actors.extend(plane_draw::actors_with(drawn, &assets, surfaces, at, alpha, &mut gpu_poses));
                // And its pilot in the seat (V39), the rider's own body posed by the plane's clips.
                if let (Some(Some(pilot)), Some(model)) = (pilots.get(p), &rigs[p].body_model) {
                    gpu_poses.push(pilot.palette.clone());
                    actors.push(Actor { model: model.as_str(), at: pilot.at, pose: Some(gpu_poses.len() - 1), lod_from: None, alpha, casts: false });
                    if args.trace_picture {
                        let nodes: Vec<V3> = pilot.palette.iter().map(|m| m.mul(pilot.at)).map(|m| V3 { x: m.0[3][0], y: m.0[3][1], z: m.0[3][2] }).collect();
                        trace_pilot(p, "gpu", model, pilot, &nodes);
                    }
                }
            }
            // And every bay's, see-through where it is parked (P103), the put-back ghost's path.
            for bay in bays.iter().filter(|b| !args.overview && b.machine == Machine::Biplane) {
                let plane = vehicle::bay_plane(bay);
                let Some(drawn) = plane_drawn.get(plane.livery).or(plane_drawn.first()) else { continue };
                actors.extend(plane_draw::actors(drawn, &assets, &plane, plane_at(&plane), vehicle::BAY_ALPHA, &mut gpu_poses));
            }
            // And every Kiowa's, the same, its rotor at rest (V113).
            for bay in kiowa_bays.iter().filter(|_| !args.overview) {
                let Some(drawn) = kiowa_drawn.get(bay.livery).or(kiowa_drawn.first()) else { continue };
                let at = M4::placement(vector(bay.spawn.at), vector(bay.spawn.facing), v3(0.0, 1.0, 0.0));
                actors.extend(kiowa_draw::actors(drawn, &assets, &Rotor::WHOLE, at, vehicle::BAY_ALPHA, &mut gpu_poses));
            }
            // And every car's bay (S245), at rest, where the port parks it: on its own track.
            for bay in bays.iter().filter(|b| !args.overview && b.machine.car_kind().is_some()) {
                let Some(drawn) = car_draw::find(&car_models, bay.machine, bay.livery) else { continue };
                actors.extend(car_draw::actors(drawn, &assets, &[game_render::car::Tire::REST; 4], car_draw::bay_at(bay), vehicle::BAY_ALPHA, &mut gpu_poses));
                if args.trace_picture {
                    car_draw::trace_bay(drawn, car_draw::bay_at(bay));
                }
            }
            // And every player's helicopter while it is flown (P216), solid, in its own livery, at
            // the measure's matrix (where the original draws it), its rotor at the flight's angle;
            // slid between the last two ticks as the plane is unless the seat cuts (P216b).
            for (p, seat) in world.seats.iter().enumerate().filter(|_| !args.overview) {
                let Some(heli) = seat.garage.heli_up() else { continue };
                let Some(drawn) = kiowa_drawn.get(heli.livery).or(kiowa_drawn.first()) else { continue };
                let before = before_helis.get(p).copied().flatten().filter(|_| !cut[p]);
                let Some((at, rotor)) = heli_drawn(heli, before, &blend) else { continue };
                let alpha = vehicle::shown_alpha(1.0, seat.paused);
                actors.extend(kiowa_draw::actors(drawn, &assets, &rotor, at, alpha, &mut gpu_poses));
                // And its thrown blades (P265), each at its own node, as the tick left it.
                actors.extend(kiowa_draw::blades(heli).into_iter().map(|at| Actor { model: game_render::kiowa::BLADE_BENT, at, pose: None, lod_from: None, alpha, casts: false }));
                // And its pilot in the seat, as the plane's (P216b).
                if let (Some(Some(pilot)), Some(model)) = (heli_pilots.get(p), &rigs[p].body_model) {
                    gpu_poses.push(pilot.palette.clone());
                    actors.push(Actor { model: model.as_str(), at: pilot.at, pose: Some(gpu_poses.len() - 1), lod_from: None, alpha, casts: false });
                    if args.trace_picture {
                        let nodes: Vec<V3> = pilot.palette.iter().map(|m| m.mul(pilot.at)).map(|m| V3 { x: m.0[3][0], y: m.0[3][1], z: m.0[3][2] }).collect();
                        trace_pilot(p, "gpu", model, pilot, &nodes);
                    }
                }
            }
            // And every player's car while it is driven (S245), solid, at its matrix, posed by its
            // tires, slid as the helicopter is; with its driver in the seat.
            for (p, seat) in world.seats.iter().enumerate().filter(|_| !args.overview) {
                let Some(car) = seat.garage.car_up() else { continue };
                let Some(drawn) = car_draw::find(&car_models, car.machine, car.livery) else { continue };
                let before = before_cars.get(p).copied().flatten().filter(|_| !cut[p]);
                let Some(at) = car_drawn(car, before, &blend) else { continue };
                let alpha = vehicle::shown_alpha(1.0, seat.paused);
                actors.extend(car_draw::actors(drawn, &assets, &car_draw::tires(car), at, alpha, &mut gpu_poses));
                if args.trace_picture {
                    car_draw::trace(p, "gpu", drawn, at, &car_draw::tires(car));
                }
                if let (Some(Some(pilot)), Some(model)) = (car_drivers.get(p), &rigs[p].body_model) {
                    gpu_poses.push(pilot.palette.clone());
                    actors.push(Actor { model: model.as_str(), at: pilot.at, pose: Some(gpu_poses.len() - 1), lod_from: None, alpha, casts: false });
                    if args.trace_picture {
                        let nodes: Vec<V3> = pilot.palette.iter().map(|m| m.mul(pilot.at)).map(|m| V3 { x: m.0[3][0], y: m.0[3][1], z: m.0[3][2] }).collect();
                        trace_pilot(p, "gpu", model, pilot, &nodes);
                    }
                }
            }
            for (k, (model, at)) in traffic_at.iter().enumerate() {
                let pose = traffic_pose.get(k).and_then(|p| p.as_ref()).map(|(p, _)| {
                    gpu_poses.push(p.clone());
                    gpu_poses.len() - 1
                });
                actors.push(Actor { model: model.as_str(), at: *at, pose, lod_from: None, alpha: 1.0, casts: false });
            }
            for c in &creatures_at {
                gpu_poses.push(c.palette.clone());
                actors.push(Actor { model: c.model.as_str(), at: c.at, pose: Some(gpu_poses.len() - 1), lod_from: None, alpha: 1.0, casts: false });
            }
            for (model, at, pose) in &toys_at {
                gpu_poses.push(pose.clone());
                actors.push(Actor { model, at: *at, pose: Some(gpu_poses.len() - 1), lod_from: None, alpha: 1.0, casts: false });
            }
            // A view's own player's markers (P88), faint bikes where each was put down: in that
            // player's view and nobody else's, so a view with any has the shared list and them.
            let marked: Vec<Vec<Actor>> = world
                .seats
                .iter()
                .zip(&rigs)
                .enumerate()
                .map(|(v, (seat, rig))| {
                    let local = online.is_none() || v == 0;
                    // The machine up's set only (P107): a plane's markers are faint planes.
                    let marks = seat.sim.markers.slots.iter().flatten().filter(|_| local);
                    let mut ghosts: Vec<Actor> = marks
                        .clone()
                        .filter_map(|m| m.ride())
                        .filter(|_| seat.garage.up == Kind::Bike)
                        .map(|ride| Actor { model: rig.bike_model.as_str(), at: chassis_at(ride), pose: None, lod_from: None, alpha: MARKER_ALPHA, casts: false })
                        .collect();
                    let livery = seat.garage.plane.as_ref().map_or(0, |p| p.livery);
                    for f in marks.filter_map(|m| m.flight()).filter(|_| seat.garage.up == Kind::Biplane) {
                        let plane = vehicle::marked_plane(f, livery);
                        let Some(drawn) = plane_drawn.get(plane.livery).or(plane_drawn.first()) else { continue };
                        ghosts.extend(plane_draw::actors(drawn, &assets, &plane, plane_at(&plane), MARKER_ALPHA, &mut gpu_poses));
                    }
                    if ghosts.is_empty() { Vec::new() } else { actors.iter().cloned().chain(ghosts).collect() }
                })
                .collect();
            // On a wrapped world each view has its own list, everything in it at its copy nearest
            // the view's camera (MD43).
            let wrap = world.place.tiled().and_then(|t| drawn_wrap(t));
            let nearest: Vec<Vec<Actor>> = match wrap {
                Some(_) => cameras
                    .iter()
                    .take(world.seats.len())
                    .enumerate()
                    .map(|(v, camera)| {
                        let list = marked.get(v).filter(|own| !own.is_empty()).unwrap_or(&actors);
                        list.iter().map(|a| Actor { at: nearest_drawn(wrap, camera.eye, a.at), ..*a }).collect()
                    })
                    .collect(),
                None => Vec::new(),
            };
            let views: Vec<View> = rects
                .iter()
                .zip(&cameras)
                .take(world.seats.len())
                .enumerate()
                .map(|(v, (at, camera))| {
                    let actors = match (nearest.get(v), marked.get(v)) {
                        (Some(own), _) => own,
                        (None, Some(own)) if !own.is_empty() => own,
                        _ => &actors,
                    };
                    View { rect: [at.x as u32, at.y as u32, at.w as u32, at.h as u32], camera, actors }
                })
                .collect();
            perf.lap(Stage::Scene, scene_at, spent);
            stages.mark(stages::Stage::Scene);

            let mut draw_at = Instant::now();
            let scene = Scene3 { views: &views, poses: &gpu_poses, overlay: &gpu_patches, sprites: &gpu_sprites, light: &light };
            let mut sprites = plane_fx.as_ref().map_or_else(Vec::new, |(fx, _)| plane_sprites(fx, blend.0));
            sprites.extend(bike_fx.iter().enumerate().flat_map(|(p, fx)| plane_sprites(fx, fx_blend(blend.0, &cut, p))));
            sprites.extend(track_fx.iter().flat_map(|t| plane_sprites(&t.fx, blend.0)));
            sprites.extend(heli_fx.iter().enumerate().flat_map(|(p, fx)| plane_sprites(fx, fx_blend(blend.0, &cut, p))));
            sprites.extend(car_fx.iter().enumerate().flat_map(|(p, fxs)| {
                let b = fx_blend(blend.0, &cut, p);
                fxs.iter().flat_map(move |fx| plane_sprites(fx, b))
            }));
            let (fw, fh) = (frame.width as u32, frame.height as u32);
            // A shot this frame is read back from a frame drawn offscreen at the frame's own size.
            let shooting = shot.is_some() && engine.frame >= args.shot_frame;
            // The ride's first picture drawn once offscreen before it is shown (P150), while the
            // window still shows the loading screen: the first view's ground index lists, the
            // pipelines and the first uploads were tick 1's frame (MD28: 26 ms on world_mxu). The
            // ride's clock and the stages leave it out; it is the load's.
            if !warmed {
                warmed = true;
                if let Some(r) = renderer_of(&mut offscreen, window) {
                    let warm_at = Instant::now();
                    r.set_graphics(graphics);
                    set_plane_sprites(r, &mut fx_sheet, &sprites);
                    set_glows(r, &mut glow_textures, &glows);
                    let t = r.offscreen(fw, fh);
                    let view = t.create_view(&Default::default());
                    r.render(&offscreen_target(&view, fw, fh), &assets, &scene);
                    r.finish();
                    let took = warm_at.elapsed();
                    // The steady clock never ran on for it.
                    if !clock.is_steady() {
                        pacer.hold(took);
                    }
                    stages.skip();
                    draw_at += took;
                    println!("warm frame: {:.2} ms behind the loading screen", took.as_secs_f64() * 1000.0);
                }
            }
            if !load_said {
                load_said = true;
                let cpu = match (paint_stream::cpu_time(false), paint_stream::cpu_time(true)) {
                    (Some(all), Some(main)) => format!("{:.2} s ({:.2} s on this thread)", all.as_secs_f32(), main.as_secs_f32()),
                    _ => "not known here".to_string(),
                };
                println!("load: the ride's first frame {:.2} s after launch, memory {}, cpu {cpu}", launched().elapsed().as_secs_f32(), peak_memory());
            }
            // The Tricks page's rider over the pause (P259), after the frame and before it is read.
            let trick_draw = |r: &mut Renderer, t: &Target| {
                if let (Some(Some(stage)), Some(rig)) = (&trick_stage, rigs.first()) {
                    let models = trick_view::Models { bike: &rig.bike_model, parts: &rig.parts, body: rig.body_model.as_deref() };
                    stage.draw_gpu_each(r, t, &assets, &models, &trick_posed);
                }
            };
            let mut waited = Duration::ZERO;
            let out = match (&mut offscreen, window.as_mut().and_then(|w| w.gpu())) {
                (Some((r, texture)), _) => {
                    let t = texture.take().filter(|t| (t.width(), t.height()) == (fw, fh)).unwrap_or_else(|| r.offscreen(fw, fh));
                    let view = t.create_view(&Default::default());
                    r.set_graphics(graphics);
                    set_plane_sprites(r, &mut fx_sheet, &sprites);
                    set_glows(r, &mut glow_textures, &glows);
                    let d = r.render(&offscreen_target(&view, fw, fh), &assets, &scene);
                    trick_draw(r, &offscreen_target(&view, fw, fh));
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
                    set_glows(&mut w.renderer, &mut glow_textures, &glows);
                    let spent = Instant::now();
                    let d = w.draw(frame.width, frame.height, |r, t| {
                        let d = r.render(t, &assets, &scene);
                        trick_draw(r, t);
                        d
                    })?;
                    waited = w.waited;
                    // Judged on the GPU's own time where it has timestamps (V122); without, on the
                    // draw's wall time, leaving out frames the CPU made late.
                    let gpu_times = w.renderer.gpu_times();
                    let cpu = scale_frame_at.map_or(Duration::ZERO, |(t, w)| spent.duration_since(t).saturating_sub(w));
                    scale_frame_at = Some((spent, waited));
                    if render_scale.is_none() {
                        let was = w.renderer.render_scale;
                        w.renderer.render_scale = match w.renderer.timed() {
                            true => gpu_times.iter().fold(was, |_, &(t, at)| auto_scale.gpu(t, at)),
                            false => auto_scale.frame(cpu, spent.elapsed()),
                        };
                        if w.renderer.render_scale != was {
                            println!("render scale: {} (auto)", w.renderer.render_scale);
                        }
                    }
                    if shooting {
                        let t = w.renderer.offscreen(fw, fh);
                        let view = t.create_view(&Default::default());
                        w.renderer.render(&offscreen_target(&view, fw, fh), &assets, &scene);
                        trick_draw(&mut w.renderer, &offscreen_target(&view, fw, fh));
                        frame.color = w.renderer.read(&t);
                    }
                    d
                }
                _ => None,
            };
            perf.add(Stage::Draw, draw_at.elapsed().saturating_sub(waited));
            perf.add(Stage::Present, waited);
            if let Some(front) = front.as_deref_mut().filter(|f| tuning && f.menu.settings.video_auto) {
                let now = Instant::now();
                let dt = tune_at.map_or(0.0, |t| now.duration_since(t).as_secs_f32());
                tune_at = Some(now);
                if let Some(next) = tune.frame_time(menu::video::Look::of(&front.menu.settings), dt, tune_target) {
                    next.apply(&mut front.menu.settings);
                    front.menu.mark_changed();
                }
            }
            // The GPU's whole frame, its ground's choosing and drawing among it, is its present.
            stages.mark(stages::Stage::Present);
            if let Some(d) = out {
                // The work P150's warm-up moves to the load, as counts, which wall time under load
                // cannot show (K43): near sets the ride's ticks cut, ground parts this frame wrote.
                if args.timings && !first_said {
                    first_said = true;
                    let cut = world.place.tiled().map_or(0, |t| t.near_cuts()) - near_cut_at_load;
                    println!("first frame: at tick {}, near sets cut in the ride {cut}, ground parts written {}", pacer.ticks(), d.ground_written);
                }
                stages.shift(stages::Stage::Present, stages::Stage::Ground, d.ground_time);
                ground_visits = (ground_visits.0 + d.ground_visits, ground_visits.1.max(d.ground_visits), ground_visits.2 + 1);
                for (v, tris) in d.triangles.iter().enumerate() {
                    view_triangles[v] += tris;
                    drawn += tris;
                }
                // Seat 0's own view, where its actors keep their places in every list (P212a). Not
                // while the body is drawn thrown (P224): on the put-back's first tick the original still
                // draws it where the ragdoll lay (S74), often out of the view the bike is in.
                if !rider_thrown && bike_actor.is_some_and(|i| d.actor(0, i) > 0) {
                    rider_frames.0 += 1;
                    if rider_actor.is_none_or(|i| d.actor(0, i) == 0) {
                        rider_frames.1 += 1;
                    }
                }
            }
        }
        let split_views = !view_frames.is_empty();
        let join_view = join_cue.then(|| cue_view(&rects, frame.width));
        // A wrapped world (MD43), whose ground the software path draws again across each seam.
        let soft_wrap = if gpu_on { None } else { world.place.tiled().and_then(|t| drawn_wrap(t)) };
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
            // Its own light and fog (P142): a world's view over its own map's, a disc map's the scene's.
            let light = lights.get(v).copied().unwrap_or(light);
            // The picture: the dirt, then the scene's props and the bikes standing on them.
            let draw_at = Instant::now();
            clear(target, light.fog_color);
            stages.mark(stages::Stage::Sky);
            // A world's is every near piece's (MD15), each cell its owner's.
            let mut tris = match (&world_mod, &world.place) {
                (Some(w), Land::Tiles(t)) => {
                    let paint = world_look.as_ref().map(|l| &l.paint);
                    world_dress::draw_ground(target, camera, &light, pos, w, t, &mut soft_terrains, paint, soft_wrap)
                }
                _ => ground.draw(target, camera, &light, pos, ground.radius()),
            };
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
            while plane_copies.len() < world.seats.len() {
                let p = plane_copies.len();
                plane_copies.push(plane_drawn.iter().map(|d| format!("{}@seat{p}", d.model)).collect());
            }
            let traffic_copies: Vec<String> = traffic_at.iter().enumerate().map(|(k, (m, _))| format!("{m}#car{k}")).collect();
            // Each seat's own copy of its flown helicopter's model, which its rotor's pose is skinned
            // into (P216b).
            let heli_copies: Vec<String> = (0..world.seats.len()).map(|p| format!("kiowa@seat{p}")).collect();
            // And of its driven car's, which its tires' pose is skinned into (S245).
            let car_copies: Vec<String> = (0..world.seats.len()).map(|p| format!("car@seat{p}")).collect();
            let mut actor: Vec<(&str, M4)> = Vec::new();
            for (p, (seat, rig)) in world.seats.iter().zip(&rigs).enumerate() {
                // With the plane up the bike is nowhere (P102), and the rider is the plane's pilot —
                // drawn with the plane below (V39).
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
                let bike_at = before.get(p).filter(|_| !cut[p]).map_or(bike_at, |b| blend.slide(b.bike, bike_at));
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
                    _ => riding_bones(seat, ragdolled.as_ref()),
                };
                if let (Some(body), Some(model), Some((bones, rigged))) = (&rig.body, &rig.body_model, &down) {
                    if let Some(loaded) = assets.models.get_mut(model.as_str()) {
                        body.skin_ragdoll_into(loaded, RIDER_GROUP, rigged, bones, bike_at);
                        skinned[p] = None;
                    }
                    actor.push((model.as_str(), bike_at));
                } else if let (Some(body), Some(model)) = (&rig.body, &rig.body_model) {
                    if body.posed {
                        // The body's finest level of detail ([`RIDER_GROUP`]), and the hands and feet put
                        // back where the chassis wants them.
                        if let Some(loaded) = assets.models.get_mut(model.as_str()) {
                            let group = RIDER_GROUP;
                            if skinned[p] != Some(group) {
                                perf.time(Stage::Skinning, || body.skin_into_at(loaded, group, Some(&grips)));
                                skinned[p] = Some(group);
                            }
                        }
                        actor.push((model.as_str(), bike_at));
                    }
                }
            }
            // The plane is drawn only while it is flown (P102). At its body's pose, nose and up off the
            // plane's own measure, with the prop at the engine's angle (S35) — or the bent cone,
            // unturned, once it is wrecked. The bays' see-through planes (P103) are the GPU's alone:
            // this path draws solid or nothing, and a solid one would be a machine that is not there.
            for (p, seat) in world.seats.iter().enumerate() {
                let Some(plane) = seat.garage.plane_flown().filter(|_| !args.overview) else { continue };
                let livery = if plane.livery < plane_drawn.len() { plane.livery } else { 0 };
                let Some(drawn) = plane_drawn.get(livery) else { continue };
                let before = before_planes.get(p).copied().flatten().filter(|_| !cut[p]);
                let at = before.map_or(plane_at(plane), |b| blend.slide(b, plane_at(plane)));
                let was = before_surfaces.get(p).and_then(Option::as_ref).filter(|_| !cut[p]);
                let surfaces = plane_draw::shown(was, plane, blend.0);
                actor.extend(plane_draw::skinned_with(drawn, &plane_copies[p][livery], &mut assets, surfaces, at));
                // Its pilot (V39), skinned here from the same pose the GPU draws, at the body's
                // finest level of detail; the bike's rider is not drawn while the plane is up, so
                // the body's mesh is the pilot's for this seat.
                if let (Some(Some(pilot)), Some(model)) = (pilots.get(p), &rigs[p].body_model) {
                    if let Some(loaded) = assets.models.get_mut(model.as_str()) {
                        loaded.tris = perf.time(Stage::Skinning, || game_render::skin::skin(&loaded.model, &pilot.palette, RIDER_GROUP));
                        skinned[p] = None;
                        if args.trace_picture {
                            let corners: Vec<V3> = loaded.tris.iter().flat_map(|t| t.v.iter().map(|v| pilot.at.point(v.pos))).collect();
                            trace_pilot(p, "software", model, pilot, &corners);
                        }
                    }
                    actor.push((model.as_str(), pilot.at));
                }
            }
            // And the flown helicopter (P216b), solid, slid as the GPU's is, with its pilot.
            for (p, seat) in world.seats.iter().enumerate().filter(|_| !args.overview) {
                let Some(heli) = seat.garage.heli_up() else { continue };
                let Some(drawn) = kiowa_drawn.get(heli.livery).or(kiowa_drawn.first()) else { continue };
                let before = before_helis.get(p).copied().flatten().filter(|_| !cut[p]);
                let Some((at, rotor)) = heli_drawn(heli, before, &blend) else { continue };
                actor.extend(kiowa_draw::skinned(drawn, &heli_copies[p], &mut assets, &rotor, at));
                actor.extend(kiowa_draw::blades(heli).into_iter().map(|at| (game_render::kiowa::BLADE_BENT, at)));
                if let (Some(Some(pilot)), Some(model)) = (heli_pilots.get(p), &rigs[p].body_model) {
                    if let Some(loaded) = assets.models.get_mut(model.as_str()) {
                        loaded.tris = perf.time(Stage::Skinning, || game_render::skin::skin(&loaded.model, &pilot.palette, RIDER_GROUP));
                        skinned[p] = None;
                        if args.trace_picture {
                            let corners: Vec<V3> = loaded.tris.iter().flat_map(|t| t.v.iter().map(|v| pilot.at.point(v.pos))).collect();
                            trace_pilot(p, "software", model, pilot, &corners);
                        }
                    }
                    actor.push((model.as_str(), pilot.at));
                }
            }
            // And the driven car (S245), solid, slid as the GPU's is, with its driver.
            for (p, seat) in world.seats.iter().enumerate().filter(|_| !args.overview) {
                let Some(car) = seat.garage.car_up() else { continue };
                let Some(drawn) = car_draw::find(&car_models, car.machine, car.livery) else { continue };
                let before = before_cars.get(p).copied().flatten().filter(|_| !cut[p]);
                let Some(at) = car_drawn(car, before, &blend) else { continue };
                actor.push(car_draw::skinned(drawn, &car_copies[p], &mut assets, &car_draw::tires(car), at));
                if args.trace_picture {
                    car_draw::trace(p, "software", drawn, at, &car_draw::tires(car));
                }
                if let (Some(Some(pilot)), Some(model)) = (car_drivers.get(p), &rigs[p].body_model) {
                    if let Some(loaded) = assets.models.get_mut(model.as_str()) {
                        loaded.tris = perf.time(Stage::Skinning, || game_render::skin::skin(&loaded.model, &pilot.palette, RIDER_GROUP));
                        skinned[p] = None;
                        if args.trace_picture {
                            let corners: Vec<V3> = loaded.tris.iter().flat_map(|t| t.v.iter().map(|v| pilot.at.point(v.pos))).collect();
                            trace_pilot(p, "software", model, pilot, &corners);
                        }
                    }
                    actor.push((model.as_str(), pilot.at));
                }
            }
            for (k, (model, at)) in traffic_at.iter().enumerate() {
                let Some(Some((pose, wheels))) = traffic_pose.get(k) else {
                    actor.push((model.as_str(), *at));
                    continue;
                };
                // The software path's own copy of the car's model, by the model and its place in
                // `traffic_at`: skinned whole once, then only its wheels moved (`Wheels::reskin`).
                let copy = &traffic_copies[k];
                if !assets.models.contains_key(copy.as_str())
                    && let Some(mut own) = assets.models.get(model).cloned()
                {
                    own.tris.clear();
                    assets.models.insert(copy.clone(), own);
                }
                if let Some(loaded) = assets.models.get_mut(copy.as_str()) {
                    wheels.reskin(&loaded.model, pose, &mut loaded.tris);
                    actor.push((copy.as_str(), *at));
                }
            }
            for (c, copy) in creatures_at.iter().zip(&creature_copies) {
                if !assets.models.contains_key(copy.as_str())
                    && let Some(own) = assets.models.get(&c.model).cloned()
                {
                    assets.models.insert(copy.clone(), own);
                }
                if let Some(loaded) = assets.models.get_mut(copy.as_str()) {
                    loaded.tris = game_render::skin::skin(&loaded.model, &c.palette, 0);
                    actor.push((copy.as_str(), c.at));
                }
            }
            for ((model, at, pose), copy) in toys_at.iter().zip(&toy_copies) {
                if !assets.models.contains_key(copy)
                    && let Some(own) = assets.models.get(*model).cloned()
                {
                    assets.models.insert(copy.clone(), own);
                }
                if let Some(loaded) = assets.models.get_mut(copy) {
                    loaded.tris = game_render::skin::skin(&loaded.model, pose, 0);
                    actor.push((copy.as_str(), *at));
                }
            }
            // On a wrapped world (MD43) every rider, plane and car at its copy nearest the camera.
            if soft_wrap.is_some() {
                for a in &mut actor {
                    a.1 = nearest_drawn(soft_wrap, camera.eye, a.1);
                }
            }
            // A world's tiles' props and plants (MD15), culled at the fog as `draw_scene` culls the
            // scene's; a plant only once it is solid, as there. On a wrapped world each at its copy
            // nearest the camera, and the plants of the copies the fog reaches too.
            let mut plants = Vec::new();
            let mut plant_cameras = vec![*camera];
            if let Some(w) = soft_wrap {
                plant_cameras.extend(w.copies(camera.eye, light.fog_far).into_iter().map(|k| w.moved(camera, k)));
            }
            for d in &dressed {
                for p in &d.props {
                    let radius = assets.models.get(&p.model).map_or(0.0, |l| l.radius);
                    let at = nearest_drawn(soft_wrap, camera.eye, p.transform());
                    if v3(at.0[3][0], at.0[3][1], at.0[3][2]).sub(camera.eye).len() - radius <= light.fog_far {
                        actor.push((p.model.as_str(), at));
                    }
                }
                if let Some(grove) = &d.grove {
                    let fov = game_render::eco::horizontal_fov(camera, target.width as f32 / target.height.max(1) as f32);
                    for c in &plant_cameras {
                        grove.draws(c, fov, light.fog_far, &mut plants);
                    }
                }
            }
            actor.extend(plants.iter().filter(|d| d.alpha >= 1.0).map(|d| (d.model, nearest_drawn(soft_wrap, camera.eye, d.at))));
            perf.lap(Stage::Scene, scene_at, spent);
            let draw_at = Instant::now();
            tris += draw_scene(target, &look, &assets, &actor, camera);
            // The plane's particles (P62), after the models and before the overlay, as the GPU draws
            // them, two triangles a square in the count.
            if let (Some((fx, _)), Some(sheet)) = (&plane_fx, &soft_sheet) {
                tris += 2 * game_render::particle::draw(target, camera, sheet, &plane_particles(fx, blend.0), &light);
            }
            // And the bikes' (P74), the same way.
            if let Some(sheet) = &soft_sheet {
                for (p, fx) in bike_fx.iter().enumerate() {
                    tris += 2 * game_render::particle::draw(target, camera, sheet, &plane_particles(fx, fx_blend(blend.0, &cut, p)), &light);
                }
                for fx in track_fx.iter().map(|t| &t.fx) {
                    tris += 2 * game_render::particle::draw(target, camera, sheet, &plane_particles(fx, blend.0), &light);
                }
                for (p, fx) in heli_fx.iter().enumerate() {
                    tris += 2 * game_render::particle::draw(target, camera, sheet, &plane_particles(fx, fx_blend(blend.0, &cut, p)), &light);
                }
                for (p, fxs) in car_fx.iter().enumerate() {
                    for fx in fxs {
                        tris += 2 * game_render::particle::draw(target, camera, sheet, &plane_particles(fx, fx_blend(blend.0, &cut, p)), &light);
                    }
                }
            }
            // The start zones' and the markers' portals (V51), over the particles as the GPU lays them.
            if let Some(glows) = glows.get(v) {
                tris += game_render::glow::draw(target, camera, &glow_list, glows);
            }
            stages.mark(stages::Stage::Scene);
            // And the overlay over the top of it, after everything with a depth test and before anything
            // that reads the frame: what the window shows and what `--shot` writes are the same picture.
            let (seat, rig) = (&world.seats[v], &mut rigs[v]);
            // The page's black (S85), over the world and under the HUD, as the GPU lays it.
            shade_view(target, seat.shade.alpha());
            // The ride HUD (P121), unless `none` was asked for.
            if overlay_mode == menu::Overlay::Hud {
                let marker = shown_markers.get(v).and_then(Option::as_ref);
                let join = join_view == Some(v);
                let ctx = RideHud { ridden: &ridden, art: &hud_art, span: radar_span, maps: &map_names, wrap: hud_wrap, speedometer, names: &names, table: &table, simulated, join, buttons: &hud_buttons, join_buttons: &join_buttons };
                ctx.with(seat, rig, v, marker, |shown, tracker, now| {
                    ride_hud::draw(target, shown, tracker, now);
                });
            }
            // The player's own pause over their view (P84), when the frame is split.
            if let Some(front) = front.as_deref_mut().filter(|_| split_views)
                && front.menu.paused(v)
            {
                let (art, menu) = (&front.art, &mut front.menu);
                menu.with_seat(v, |m| art.draw(m, target));
            }
            if let Some(view) = view_frames.get(v) {
                split::put(&mut frame, view, *at);
            }
            view_triangles[v] += tris;
            drawn += tris;
            perf.add(Stage::Draw, draw_at.elapsed());
            stages.mark(stages::Stage::Hud);
        }
        // The Show FPS counter (2026-10-29) on the software path, over every view and under the pause.
        if show_fps && !gpu_on {
            ride_hud::fps_draw(&mut frame, f32::from(hud_scale) / 100.0, &fps_meter.text());
        }
        fps_meter.frame(Instant::now());
        if let Some(front) = front.as_ref().filter(|_| !gpu_on && view_frames.is_empty()) {
            perf.time(Stage::Draw, || front.art.draw(&front.menu, &mut frame));
        }
        // The Tricks page's rider over the software pause (P259), each view's already put in the frame.
        if let (false, Some(Some(stage)), Some(rig)) = (gpu_on, &mut trick_stage, rigs.first()) {
            let models = trick_view::Models { bike: &rig.bike_model, parts: &rig.parts, body: rig.body_model.as_deref() };
            for (rect, posed) in &trick_posed {
                stage.draw_soft(&mut frame, &assets, *rect, &models, posed);
            }
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
                "{:>3.0}s {:>4.1} fps, {:>6} triangles a frame, {:>5.1} mph{}{}{}{}",
                clock.elapsed().as_secs_f32(),
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
                // Only while riding (P212a): a bike with nobody on it, as the user saw on the phone.
                match (world.seats[0].riding && simulated && !world.seats[0].waiting, world.seats[0].sim.rider.phase()) {
                    (true, Phase::Riding) if bike_up => rider_left_out(rider_frames),
                    _ => String::new(),
                },
                memory_now().map(|m| format!(", memory {m}")).unwrap_or_default(),
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
            // The first rider's bike's (P74), the emitters with any in the air.
            if let (true, Some(fx)) = (bike_up, bike_fx.first()) {
                let counts: Vec<String> = fx.counts().iter().filter(|(_, n)| *n > 0).map(|(name, n)| format!("{name} {n}")).collect();
                println!("     bike particles: {}", if counts.is_empty() { "none".to_string() } else { counts.join(", ") });
            }
            window_of_frames = (Instant::now(), 0, 0);
            rider_frames = (0, 0);
        }
        stages.mark(stages::Stage::Log);
        Ok(())
        });
        drawn?;

        if let Some(limit) = args.seconds {
            if clock.elapsed().as_secs_f32() >= limit {
                break;
            }
        }
        // Online, the session's ticks: the loop's frames go on the wall clock, waiting for a peer
        // still loading or stalled for its pads, so N of them were a shorter ride the busier the
        // machine (K10), and `--frames` is the same run on any machine.
        let ridden = online.as_ref().filter(|_| args.online.is_some()).map_or(pacer.ticks(), online::Online::ticks);
        if args.frames.is_some_and(|n| ridden >= u64::from(n)) {
            break;
        }
        perf.end(rects.len().min(world.seats.len()));
        // The picture's own rate (P52): as fast as the machine draws it — a window's present waits
        // for the display's refresh — or `--draw-fps`', which a headless ride, with no display to
        // wait for, takes as 60. The ticks owed meanwhile all run before the next picture, so a slow
        // picture is a jumpier one and never a slower game.
        // VSync and the frame limit (P90), from the front end's settings as they are now, or the
        // file's for a ride without one; `--draw-fps` wins over both.
        let floor = match window.as_mut() {
            Some(w) => picture_floor(w, args.pacing(front.as_ref().map_or(&ride_settings, |f| &f.menu.settings))),
            None => None,
        };
        let refresh = window.as_ref().and_then(|w| w.refresh()).map(|hz| Duration::from_secs_f32(1.0 / hz));
        gauge.drew(drawing_at.elapsed(), args.draw_frame().or(floor).or(refresh).unwrap_or_default());
        clock.picture(&mut limiter, args.draw_frame().or(floor));
        stages.mark(stages::Stage::Pace);
        stages.frame();
        loaded.get_or_insert_with(|| (game_render::tally::Tally::now(), stages::bytes_read_mark()));
        if let Some(slow) = stages.take_slow().filter(|_| args.timings) {
            timing(format!("     {slow} at tick {}", pacer.ticks()));
        }
        if args.timings && stages_since.elapsed() >= Duration::from_secs(1) {
            timing(format!("     {}; {}", stages.line(), stages.worst_line()));
            stages_since = Instant::now();
        }
        top = Instant::now();
        moment = clock.now();
        perf.begin();
    }

    // A tick still on the sim thread when the ride ended (N23a) is the world's last.
    if let Some(mut ran) = flight.take() {
        if ran.step.is_none() {
            ran.run();
        }
        world = ran.world;
        engine = ran.engine;
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
        // And where the first rider ended, so a ride on a track can be seen to have gone somewhere (MD2).
        let end = world.seats[0].sim.rider.ride.position();
        println!("end: {:.0}, {:.0}, {:.0}", end.x(), end.y(), end.z());
        // And a world's ground there as the GPU holds it, to compare a streamed ground with the
        // whole one (MD17).
        if ground_stream.is_some()
            && let (Some(w), Some(t)) = (&world_mod, world.place.tiled())
            && let Some((i, _, _)) = t.owner(F(end.x()), F(end.z()))
            && let Some(r) = renderer_of(&mut offscreen, window)
        {
            let hash = r.ground_piece_hash(i as u32).map_or("not held".to_string(), |h| format!("{h:#018x}"));
            println!("world ground under the rider: {} {hash}", w.pieces[i].tile.name);
        }
        // And how much of it a frame of the ride wrote at most (MD28).
        if let Some(stream) = &ground_stream {
            let (most, all, frames) = ground_written;
            println!(
                "world ground: at most {most} chunks written in a frame of the ride ({all} in {frames} frames, budget {})",
                stream.chunks_a_frame
            );
            println!("world ground: held at most {} chunks at once in the ride, room for {}", ground_most.0, ground_most.1);
        }
        // And the maps' pages it wrote (P283).
        if let Some(paint) = &paint_stream {
            println!(
                "world paint: {} maps shown and {} let go in the ride, at most {:.1} MB written in a frame ({:.1} MB in all), slowest update {:.2} ms, {:.2} ms of cpu",
                paint.shown,
                paint.let_go,
                paint.most_bytes as f32 / 1048576.0,
                paint.bytes as f32 / 1048576.0,
                paint.slowest * 1000.0,
                paint.slowest_cpu * 1000.0
            );
            if let Some((used, shown)) = renderer_of(&mut offscreen, window).and_then(|r| r.world_paint_pool()) {
                let used: Vec<String> = used.iter().map(|(u, c)| format!("{u} of {c}")).collect();
                println!("world paint: {shown} maps on their own pages at the end, layers held {}", used.join(", "));
            }
        }
        // And where each dressed tile's props stand as the GPU draws them, so a tile's props can be
        // seen moved by its offset (MD15).
        if let Some(w) = &world_mod
            && !dressed.is_empty()
            && let Some(r) = renderer_of(&mut offscreen, window)
        {
            for d in &dressed {
                let at = r.props_of(d.index as u32);
                let (x, z) = at.iter().fold((f32::MAX, f32::MAX), |(x, z), p| (x.min(p.x), z.min(p.z)));
                println!("world props drawn: {} {} props, lowest x {x:.0}, lowest z {z:.0}", w.pieces[d.index].tile.name, at.len());
            }
        }
        // And the first player's plane, flown or not, for a tape that switched into it (P102).
        if let Some(flight) = world.seats[0].garage.plane.as_ref().and_then(|p| p.flight.as_ref()) {
            println!("plane hash {:#018x} ({})", game_sim::hash::of(flight), world.seats[0].garage.up.name());
        }
        // And the first player's car, if they end the ride in one (S149e).
        if let Some(said) = world.seats[0].garage.car_up().and_then(vehicle::CarRide::report) {
            println!("{said}");
            // And its particles in the air (S248).
            let up = world.seats[0].garage.car_up().map(|c| c.machine);
            if let Some(fx) = car_fx.first().and_then(|fxs| car_fx_lists.iter().zip(fxs).find(|((m, _, _), _)| Some(*m) == up)).map(|(_, fx)| fx) {
                let counts: Vec<String> = fx.counts().iter().filter(|(_, n)| *n > 0).map(|(name, n)| format!("{name} {n}")).collect();
                println!("car: particles: {}", counts.join(", "));
            }
            // And the triangles each of its tire tracks has laid (S269).
            if let Some(tracks) = car_marks.first().and_then(|m| car_tread.iter().zip(m).find(|((m, _), _)| Some(*m) == up)).map(|(_, t)| t) {
                let laid: Vec<String> = tracks.tracks.iter().map(|t| format!("{} {}", t.params.name, t.strip.ring.triangles().len())).collect();
                println!("car: tire tracks: {}", laid.join(", "));
            }
        }
        // Or the helicopter (P216), and its particles in the air (V114).
        if let Some(said) = world.seats[0].garage.heli_up().and_then(vehicle::KiowaRide::report) {
            println!("{said}");
            if let Some(fx) = heli_fx.first() {
                let counts: Vec<String> = fx.counts().iter().filter(|(_, n)| *n > 0).map(|(name, n)| format!("{name} {n}")).collect();
                println!("kiowa: particles: {}", counts.join(", "));
            }
            // And its thrown blades (P265).
            let place = &mut world.place;
            if let Some(said) = world.seats[0].garage.heli_up().and_then(|h| h.blades_report(place)) {
                println!("{said}");
            }
        }
    }

    let opened = front.as_deref().is_some_and(|f| f.menu.opening == menu::Opening::Open);
    if let Some(front) = front.as_deref_mut() {
        front.menu.opening = menu::Opening::No;
    }
    if let Some(on) = &mut online {
        // And any left on ticks run but not settled when the ride ended.
        for said in on.cars_left(true) {
            println!("{said}");
        }
        println!("{}", on.summary());
        // What the lobby said while the ride ran (a drop, the lobby as it is now, chat) is the menu's.
        if let Some(front) = front.as_deref_mut() {
            for e in on.kept() {
                front.menu.heard(e);
            }
            // an opened freeride's room closed with it (`Online::leave`), and Leave Session (N11) left
            // the lobby with the ride: no lobby to go back to
            if opened || on.leaving() {
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
        println!("session {}; {}", stages.session(), stages.session_worst());
        if ground_visits.2 > 0 {
            println!("ground nodes looked at a frame: {} on average, {} at most", ground_visits.0 / ground_visits.2, ground_visits.1);
        }
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
        // And the memory the ride ended holding, read after the bytes above are counted (P283).
        println!("memory: {}", peak_memory());
    }
    let at = world.seats[0].sim.rider.ride.position();
    println!(
        "ridden for {:.1}s over {} frames, ending at {:.2}, {:.2}, {:.2}",
        clock.elapsed().as_secs_f32(),
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
/// **The seats as the material table's queue reads them** (F27a): with more than one seat, each is a
/// one-player ride and bike `g` is seat `g`'s; with one, bike `g` is its ride's player `g`. The props
/// are the first seat's, which every seat shares (S197c).
struct HeardRide<'a> {
    seats: &'a [Seat],
}

impl HeardRide<'_> {
    fn rider(&self, g: usize) -> Option<&game_sim::rider::Rider> {
        match self.seats {
            [one] => (g < one.sim.players()).then(|| one.sim.rider_of(g)),
            seats => seats.get(g).map(|s| &s.sim.rider),
        }
    }

    fn flight(&self, s: usize) -> Option<&game_sim::flight::Flight> {
        let seat = self.seats.get(s).filter(|seat| seat.garage.up == Kind::Biplane)?;
        seat.garage.plane.as_ref()?.flight.as_ref()
    }

    /// What every listened probe heard this tick, in the node's order: the bikes', the planes', the
    /// props'.
    fn heard(&self) -> Vec<(game_sim::material_hit::Owner, game_sim::material_hit::Struck)> {
        use game_sim::material_hit::Owner;
        let mut out = Vec::new();
        let bikes = match self.seats {
            [one] if one.riding && one.garage.up == Kind::Bike => one.sim.players(),
            [_] => 0,
            seats => seats.len(),
        };
        for g in 0..bikes {
            if self.seats.len() > 1 && !(self.seats[g].riding && self.seats[g].garage.up == Kind::Bike) {
                continue;
            }
            let Some(r) = self.rider(g) else { continue };
            out.extend(r.ride.heard.0.iter().map(|&s| (Owner::Bike(g), s)));
        }
        for s in 0..self.seats.len() {
            if let Some(f) = self.flight(s).filter(|_| self.seats[s].riding) {
                out.extend(f.heard.0.iter().map(|&h| (Owner::Plane(s), h)));
            }
        }
        if let Some(seat) = self.seats.first() {
            for (c, col) in seat.sim.traffic.props.collections.iter().enumerate() {
                out.extend(col.heard.0.iter().map(|&(k, h)| (Owner::Prop { collection: c, object: k }, h)));
            }
        }
        out
    }

    fn prop_material(&self, c: usize, k: usize) -> u8 {
        let shapes = self.seats.first().map(|s| s.sim.traffic.props.shapes.clone());
        shapes.as_ref().and_then(|s| s.get(c)?.get(k)?.as_ref().map(game_sim::material_hit::material_of)).unwrap_or(0)
    }
}

impl game_sim::material_hit::Ride for HeardRide<'_> {
    fn owner(&self, o: game_sim::material_hit::Owner) -> Option<(u8, Vf, Vf)> {
        use game_sim::material_hit::{BIKE, Owner};
        match o {
            Owner::Bike(g) => {
                // The chassis' impact accumulator, not its velocity (B27): the table's speed.
                let r = &self.rider(g)?.ride;
                Some((BIKE, r.impact.0, r.position()))
            }
            Owner::Plane(s) => {
                let f = self.flight(s)?;
                let material = f.hull.as_deref().map_or(0, game_sim::material_hit::material_of);
                Some((material, f.body.shock_peak, f.body.position))
            }
            Owner::Prop { collection, object } => {
                let col = self.seats.first()?.sim.traffic.props.collections.get(collection)?;
                let body = &col.bones.get(object)?.body;
                Some((self.prop_material(collection, object), body.shock_peak, body.position))
            }
        }
    }

    fn struck(&self, s: game_sim::material_hit::Struck) -> (u8, bool) {
        use game_sim::material_hit::{BIKE, Struck};
        match s {
            Struck::Bike(_) => (BIKE, false),
            Struck::Plane(j) => (self.flight(j).and_then(|f| f.hull.as_deref()).map_or(0, game_sim::material_hit::material_of), false),
            Struck::Prop { collection, object } => (self.prop_material(collection, object), false),
            Struck::Mesh { material, foliage, .. } => (material, foliage),
        }
    }
}

fn seat_sound(seat: &Seat, area: Option<game_sim::biplane::Area>, land: &dyn sim_world::World) -> audio::SeatFrame {
    let r = &seat.sim.rider.ride;
    // The shootoff's (P85): the edge asked of the chassis' centre, as the throw asks it — none on a
    // wrapped axis, whose edge is a seam (P231), and a world's holes as well as its rectangle (MD8).
    let c = r.chassis.cofg_world;
    let armed = area.is_some() && seat.sim.rider.edge_armed;
    let edge = area.filter(|_| armed).and_then(|a| audio::world_edge(a, land, c));
    let edge = audio::EdgeFrame { armed, reason: seat.sim.rider.state.wreck.map_or(0, |w| w.0 as i32), edge };
    let p = r.position();
    let v = r.chassis.vel;
    let plane = match (seat.garage.up, &seat.garage.plane) {
        (Kind::Biplane, Some(plane)) => {
            let note = plane.flight.as_ref().map_or(0.0, audio::plane_note);
            Some((note, [plane.at.x(), plane.at.y(), plane.at.z()]))
        }
        _ => None,
    };
    // The plane's wreck clank (P164): its wreck word, its collision record and its body's shock peak.
    let crash = match (seat.garage.up, seat.garage.plane.as_ref().and_then(|p| p.flight.as_ref())) {
        (Kind::Biplane, Some(f)) => audio::PlaneCrash::of(f),
        _ => audio::PlaneCrash::default(),
    };
    // The plane's wheels (P146): the measure's grounded flag rising, and the body's speed then.
    let touchdown = match (seat.garage.up, seat.garage.plane.as_ref().and_then(|p| p.flight.as_ref())) {
        (Kind::Biplane, Some(f)) if f.measure.grounded && !f.measure.was_grounded => {
            let v = f.body.velocity;
            Some(audio::length([v.x(), v.y(), v.z()]))
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
        }
        .wrecked(seat.sim.rider.state.wreck.is_some()),
        wreck: seat.said.wreck.map(|w| w.0 as i32),
        landed_trick: seat.said.landed.is_some(),
        plane,
        crash,
        // The Kiowa's loop and clank (V114), while it is up and out.
        heli: seat.garage.heli_up().and_then(|k| k.flown.as_ref()).map(|h| audio::HeliFrame::of(&h.kiowa, &h.ride, h.touching())),
        // A driven car's engine voice (S247): its sound pitch, its top revs and the gas.
        car: seat.garage.car_up().and_then(vehicle::CarRide::engine_sound).map(|e| audio::CarFrame {
            profile: e.profile,
            pitch: e.pitch,
            top: e.top,
            throttle: e.throttle,
            at: e.at,
            vel: e.vel,
        }),
        bike: audio::BikeCrash::of(&seat.sim.rider),
        touchdown,
        edge,
        body: audio::RiderBody::of(&seat.sim.rider),
        // The sim's wreck emitters (P202): the voice and the clank play the clips it picked.
        emitters: Some(audio::WreckEmitters::of(&seat.sim.rider)),
        // The rear wheel on a prop (P145): its material, where it touches, and the body's velocity.
        drive_on: audio::DriveOnFrame {
            material: r.rear_material,
            at: [r.rear.w.contact_point.x(), r.rear.w.contact_point.y(), r.rear.w.contact_point.z()],
            vel: [v.x(), v.y(), v.z()],
        },
        // Not riding — their own pause, a pad pulled out, Start's freeze — their sounds hold where
        // they are while the others ride on (P172).
        paused: !seat.riding,
    }
}

/// An offline tick taken whole to the sim thread (N23a): the loop's world and engine, moved there and
/// back, and the engine step the loop would have run on them, run there instead.
struct Ticking<P> {
    world: World<P>,
    engine: Engine,
    now: game_sim::tick::Timer0,
    input: Input,
    step: Option<game_sim::tick::Step>,
    /// What the step took, for the gauge.
    took: Duration,
}

impl<P: sim_world::World> Ticking<P> {
    fn run(&mut self) {
        let at = Instant::now();
        self.step = Some(self.engine.step(self.now, self.input, &mut self.world));
        self.took = at.elapsed();
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

/// The disc's camera presets the camera button cycles through (P163), the bike's and the plane's, by
/// name, for the Camera page's Preset row: free06's files, whose names every track's shares (the
/// freerides' and the races' read alike). Empty where they cannot be read.
fn camera_names(disc: &mut Disc) -> [Vec<String>; 2] {
    let Some(kit) = member_ending(disc, PLANE_KIT_ARCHIVE).ok().and_then(|name| disc.res(&name).ok()) else {
        println!("camera: no {PLANE_KIT_ARCHIVE}, so no presets to list on the Camera page");
        return Default::default();
    };
    let names = |file: &str| -> Vec<String> {
        let presets = kit.read_suffix(file).map(|b| Preset::read_bytes(&b, "")).unwrap_or_default();
        presets.into_iter().filter(|p| p.cycle_thru_this_preset).map(|p| p.name).collect()
    };
    let out = [names("cpresets.ini"), names(PLANE_PRESETS)];
    println!("camera: the Camera page lists {:?} and {:?}", out[0], out[1]);
    out
}

/// The helicopter's presets the camera button cycles through (P216b), by name, for the Camera page:
/// free06's `HeloCPresets.ini`, as [`camera_names`] reads the bike's and the plane's. Empty where it
/// cannot be read.
fn heli_camera_names(disc: &mut Disc) -> Vec<String> {
    let Some(kit) = member_ending(disc, PLANE_KIT_ARCHIVE).ok().and_then(|name| disc.res(&name).ok()) else { return Vec::new() };
    let presets = kit.read_suffix(game_render::kiowa::PRESETS).map(|b| Preset::read_bytes(&b, "")).unwrap_or_default();
    let out: Vec<String> = presets.into_iter().filter(|p| p.cycle_thru_this_preset).map(|p| p.name).collect();
    println!("camera: the Camera page lists the helicopter's {out:?}");
    out
}

/// Each car's presets the camera button cycles through (P264), by name, for the Camera page: free06's
/// copy of the car's own file ([`chase::car_presets`]), as [`heli_camera_names`] reads the
/// helicopter's. A car whose file cannot be read is left out.
fn car_camera_names(disc: &mut Disc) -> Vec<(menu::Machine, Vec<String>)> {
    let Some(kit) = member_ending(disc, PLANE_KIT_ARCHIVE).ok().and_then(|name| disc.res(&name).ok()) else { return Vec::new() };
    let mut out = Vec::new();
    for car in [menu::Machine::Buggy, menu::Machine::Monster, menu::Machine::Baja] {
        let Some(kind) = vehicle::Machine::ALL.into_iter().find(|m| m.kind().is_some_and(|k| k.name() == car.word())).and_then(vehicle::Machine::car_physics) else { continue };
        let presets = kit.read_suffix(chase::car_presets(kind)).map(|b| Preset::read_bytes(&b, "")).unwrap_or_default();
        let names: Vec<String> = presets.into_iter().filter(|p| p.cycle_thru_this_preset).map(|p| p.name).collect();
        if !names.is_empty() {
            out.push((car, names));
        }
    }
    println!("camera: the Camera page lists the cars' {out:?}");
    out
}

/// Every player's cameras onto `display`'s copy of their presets (V24): the file read again for it,
/// and the preset each camera is on re-set from the new copy — the original's own reload when a
/// camera's split changes, which keeps the preset and clears the player's nudges.
fn set_display(rigs: &mut [Rig], layout: Layout, display: Display, bike: Option<&[u8]>, plane: Option<&[u8]>) {
    let bike = bike.map(|b| layout_presets(b, layout, display, "cpresets.ini")).unwrap_or_default();
    let plane = plane.map(|b| layout_presets(b, layout, display, PLANE_PRESETS)).unwrap_or_default();
    for rig in rigs {
        let framing = rig.framing;
        frame_rig(rig, &bike, &plane, framing);
    }
}

/// A player's cameras onto the original's presets `bike` and `plane` as `framing` frames them (P82):
/// the same reload as [`set_display`]'s, which keeps the preset each camera is on and clears the
/// player's nudges, and eases the field of view over ([`chase::reframe`], P119). At
/// [`chase::Framing::ORIGINAL`] the presets are the file's, untouched.
fn frame_rig(rig: &mut Rig, bike: &[Preset], plane: &[Preset], framing: (chase::Framing, chase::Framing)) {
    // With the seat's own presets over them (P163): a disc preset's own numbers, and the player's
    // own presets after the disc's unless the camera button is on the disc's alone.
    let (own, cycle) = (&rig.camera_presets.0, rig.camera_presets.1);
    let laid = |m: menu::Machine| -> Vec<chase::Own> {
        menu::cameras::laid(own, cycle, m)
            .map(|o| {
                let (fov, distance, height) = o.look.framing();
                chase::Own { base: &o.base, name: &o.name, framing: chase::Framing { fov, distance, height } }
            })
            .collect()
    };
    let mine_only = cycle == menu::cameras::Cycle::Mine;
    chase::reframe(&mut rig.camera, &chase::seat_presets(bike, framing.0, &laid(menu::Machine::Bike), mine_only), &mut rig.fov_ease.0);
    if let Some(cam) = &mut rig.plane_camera {
        chase::reframe(cam, &chase::seat_presets(plane, framing.1, &laid(menu::Machine::Biplane), mine_only), &mut rig.fov_ease.1);
    }
    rig.framing = framing;
}

/// [`frame_rig`] for the helicopter's camera (P216b): `heli` its presets for the seat's display,
/// with the seat's own on the helicopter laid over them, framed by the Camera page's one set as the
/// bike's camera is ([`menu::cameras::Look::of`]), easing over on the flying cameras' slot as its
/// follow does.
fn frame_heli(rig: &mut Rig, heli: &[Preset]) {
    let Some(cam) = &mut rig.heli_camera else { return };
    let (own, cycle) = (&rig.camera_presets.0, rig.camera_presets.1);
    let laid: Vec<chase::Own> = menu::cameras::laid(own, cycle, menu::Machine::Helicopter)
        .map(|o| {
            let (fov, distance, height) = o.look.framing();
            chase::Own { base: &o.base, name: &o.name, framing: chase::Framing { fov, distance, height } }
        })
        .collect();
    let mine_only = cycle == menu::cameras::Cycle::Mine;
    chase::reframe(cam, &chase::seat_presets(heli, rig.framing.0, &laid, mine_only), &mut rig.fov_ease.1);
}

/// A car kind's presets file (S246), [`chase::car_presets`] by the shell's kind.
fn car_presets_file(kind: Kind) -> &'static str {
    kind.car().and_then(Machine::car_physics).map_or("", chase::car_presets)
}

/// The cars' presets (S246) for `display`, each kind's off its file's bytes.
fn cars_for_display(layout: Layout, display: Display, cars: &[(Kind, Vec<u8>)]) -> Vec<(Kind, Vec<Preset>)> {
    cars.iter().map(|(kind, bytes)| (*kind, layout_presets(bytes, layout, display, car_presets_file(*kind)))).collect()
}

/// [`frame_rig`] for the cars' cameras (S246): each onto `cars`' presets for its kind, with the seat's
/// own on that car laid over them (P264), framed by the Camera page's one set as the bike's camera is,
/// easing over on the bike's slot as its follow does.
fn frame_cars(rig: &mut Rig, cars: &[(Kind, Vec<Preset>)]) {
    let (own, cycle) = (&rig.camera_presets.0, rig.camera_presets.1);
    let mine_only = cycle == menu::cameras::Cycle::Mine;
    for (kind, cam) in &mut rig.car_cameras {
        let Some((_, presets)) = cars.iter().find(|(k, _)| k == kind) else { continue };
        let car = menu::Machine::from_word(kind.name()).unwrap_or_default();
        let laid: Vec<chase::Own> = menu::cameras::laid(own, cycle, car)
            .map(|o| {
                let (fov, distance, height) = o.look.framing();
                chase::Own { base: &o.base, name: &o.name, framing: chase::Framing { fov, distance, height } }
            })
            .collect();
        chase::reframe(cam, &chase::seat_presets(presets, rig.framing.0, &laid, mine_only), &mut rig.fov_ease.0);
    }
}

/// Every player's car cameras onto `display`'s copy of their presets, as [`set_heli_display`] puts
/// the helicopter's (S246).
fn set_car_display(rigs: &mut [Rig], layout: Layout, display: Display, cars: &[(Kind, Vec<u8>)]) {
    let cars = cars_for_display(layout, display, cars);
    for rig in rigs {
        frame_cars(rig, &cars);
    }
}

/// Every player's helicopter camera onto `display`'s copy of its presets, as [`set_display`] puts
/// the bike's and the plane's (P216b).
fn set_heli_display(rigs: &mut [Rig], layout: Layout, display: Display, heli: Option<&[u8]>) {
    let heli = heli.map(|b| layout_presets(b, layout, display, game_render::kiowa::PRESETS)).unwrap_or_default();
    for rig in rigs {
        frame_heli(rig, &heli);
    }
}

/// A player's Camera page (P82) as the shell frames their bike's camera and their plane's with it.
fn framings(v: menu::CameraView) -> (chase::Framing, chase::Framing) {
    let f = |(fov, distance, height): (f32, f32, f32)| chase::Framing { fov, distance, height };
    (f(v.bike()), f(v.plane()))
}

/// The ground's height under `at` for parking the lot: a world's tiles' ([`game_sim::tiles::Tiled`],
/// MD24), whose home tile is not in its own feet, or else the one track's.
fn lot_ground(tiled: Option<&mut game_sim::tiles::Tiled>, track: &mut sim_world::Ground, at: Vf) -> f32 {
    let from = Vf::new(at.x(), 1.0e4, at.z(), 1.0);
    match tiled {
        Some(t) => sim_world::World::ground(t, from).0.y(),
        None => sim_world::World::ground(track, from).0.y(),
    }
}

/// The ground under `from` and its normal there, as [`lot_ground`] finds it, asked from `from` itself.
fn lot_ground_normal(tiled: Option<&mut game_sim::tiles::Tiled>, track: &mut sim_world::Ground, from: Vf) -> (Vf, Vf) {
    match tiled {
        Some(t) => sim_world::World::ground(t, from),
        None => sim_world::World::ground(track, from),
    }
}

/// The terrain file a scene asks for. Most scenes name it `SX02.trn`, but what the archive holds is
/// `sx02.ps2.trn`: the loader tags the file with the platform it was built for, and a few scenes
/// already spell the tag out.
/// The ground under a path car (V41), as the ride's own query answers it: the point on the surface
/// and the surface's normal. Off the terrain the point stays where it is, standing straight up. `p`
/// is in the track's own feet, which sit `by` from the world's (a world's home tile, MD8).
/// A path car's picture between two ticks' poses (S81): each row slid `by` of the way, which on a
/// tick's few feet and fraction of a degree is the turn's own blend to the eye.
fn car_between(before: &game_sim::math::Mat4, now: &game_sim::math::Mat4, by: f32) -> M4 {
    let by = by.clamp(0.0, 1.0);
    M4(std::array::from_fn(|r| std::array::from_fn(|c| {
        let (a, b) = (before.0[r].0[c], now.0[r].0[c]);
        a + (b - a) * by
    })))
}

fn car_ground(world: &mut dyn sim_world::World, p: V3, by: [f32; 2]) -> (V3, V3) {
    let (on, normal) = world.ground(Vf::new(p.x + by[0], p.y, p.z + by[1], 1.0));
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

/// The `mods/` folder a ride may look for a track in (MD2): `--mods`' or the front end's, else beside
/// `settings.txt` when its Mods row is on. `None` reads nothing.
/// Where MX Unleashed's worlds keep their built joins (P232): under `--mods DIR`, else the mods folder
/// beside the settings — with mods off too, as the joins are the disc's, not a mod.
fn disc_worlds_cache(args: &Args) -> std::path::PathBuf {
    match (&args.mods, args.mods_asked) {
        (Some(dir), true) => dir.clone(),
        _ => menu::store_dir(Disc::configured_path().ok().as_deref()).join(track_mods::MODS_DIR),
    }
}

fn mods_dir(args: &Args) -> Option<std::path::PathBuf> {
    if args.mods_asked {
        return args.mods.clone();
    }
    let dir = menu::store_dir(Disc::configured_path().ok().as_deref());
    menu::load(&dir).0.mods.then(|| dir.join(track_mods::MODS_DIR))
}

/// The world's replaced textures for the next ride (V84, `textures.rs`): `--textures`, else the
/// front end's or settings.txt's Upscaled Textures; over the mods folder beside the settings, or
/// `--mods DIR` (`--mods off` reads none), in settings.txt's texture pack order. The line says what
/// was found. With `OPENMX_TEXTURE_RELOAD=1` (V86) the folders are kept watched while the ride runs.
fn install_textures(args: &Args) -> Option<textures::Reload> {
    let dir = menu::store_dir(Disc::configured_path().ok().as_deref());
    let settings = menu::load(&dir).0;
    let on = args.textures.unwrap_or(settings.textures);
    let mods = if args.mods_asked { args.mods.clone() } else { Some(dir.join(track_mods::MODS_DIR)) };
    let (line, reload) = textures::install_live(mods.as_deref(), on, &settings.texture_packs, &settings.texture_packs_off, textures::reload_asked());
    println!("{line}");
    if reload.is_some() {
        println!("textures: OPENMX_TEXTURE_RELOAD: changed files are drawn while riding");
    }
    reload
}

/// `openmx --machine-packs [MODS]` (MD37c): every machine pack in `MODS/machines/` (the mods folder
/// beside the settings by default) read through openmx's readers, a report each. Loading one is all
/// it does; nothing rides a machine pack yet.
fn machine_packs_report(rest: &[String]) -> Result<(), String> {
    let mods = match rest {
        [] => menu::store_dir(Disc::configured_path().ok().as_deref()).join(track_mods::MODS_DIR),
        [dir] => std::path::PathBuf::from(dir),
        _ => return Err("--machine-packs wants at most a mods folder".into()),
    };
    let (packs, why) = machine_packs::find(&mods);
    for pack in &packs {
        for line in pack.report() {
            println!("{line}");
        }
    }
    for w in &why {
        println!("machine packs: {w}");
    }
    if packs.is_empty() {
        println!("machine packs: none in {} (openmx-mods' tools/import-atv makes one)", mods.join(machine_packs::MACHINES_DIR).display());
    }
    Ok(())
}

/// `openmx --dump-textures [OUT] [--from DIR]` (V84): every disc texture, or with `--from` every one a
/// mod's folder carries, written at its native size as `<path>-<key>.png` under OUT
/// (`mods/textures-src/` beside the settings by default): the sources to edit or upscale into
/// `mods/texture-replacements/`, a texture pack, or a mod's `textures/upscaled/`.
fn dump_textures(rest: &[String]) -> Result<(), String> {
    let (mut out, mut from) = (None, None);
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--from" => from = Some(std::path::PathBuf::from(it.next().ok_or("--from wants a mod's folder")?)),
            a if !a.starts_with("--") && out.is_none() => out = Some(std::path::PathBuf::from(a)),
            other => return Err(format!("--dump-textures: unknown argument {other}")),
        }
    }
    let out = match out {
        Some(o) => o,
        None => menu::store_dir(Disc::configured_path().ok().as_deref()).join(track_mods::MODS_DIR).join(textures::SOURCES),
    };
    let n = match &from {
        Some(dir) => textures::dump_folder(dir, &out)?,
        None => textures::dump_disc(&mut Disc::open_configured()?, &out)?,
    };
    println!("textures: {n} written to {}", out.display());
    Ok(())
}

/// The trick packs a ride stacks over the original's tricks (MD16, MD77): only with `--tricks` or
/// the Tricks row's Trick Packs, never on `--online`, and only those of [`Args::trick_packs`] the
/// mods folder has (none named: its first), in the folder's order; the line says which, or why
/// none. A lobby's ride rides its room's list (MD19) or none at all: without it this machine would
/// ride other tricks than every other seat, which the handshake should have refused.
fn ride_trick_packs(args: &Args) -> Result<Vec<trick_packs::Pack>, String> {
    if let Some((dir, word)) = &args.room_tricks {
        let packs = trick_packs::on(dir, &trick_packs::ids_of_room(word)).map_err(|e| format!("the room's tricks: {e}"))?;
        println!("tricks: MX vs. ATV's, from the room's {word}");
        return Ok(packs);
    }
    if args.tricks != Some(menu::Tricks::MxVsAtv) {
        return Ok(Vec::new());
    }
    if args.online.is_some() {
        println!("tricks: the original's on --online, which has no handshake to check every seat's pack");
        return Ok(Vec::new());
    }
    let Some(dir) = mods_dir(args) else {
        println!("tricks: the mods folder is off, so the original's");
        return Ok(Vec::new());
    };
    let (installed, why) = trick_packs::listed(&dir);
    for line in why {
        println!("tricks: {line}");
    }
    let named = args.trick_packs.clone().unwrap_or_default();
    let packs: Vec<trick_packs::Pack> = match named.is_empty() {
        true => installed.into_iter().take(1).collect(),
        false => installed.into_iter().filter(|p| named.contains(&p.id)).collect(),
    };
    if let Some(id) = named.iter().find(|id| !packs.iter().any(|p| p.id == **id)) {
        println!("tricks: no trick pack {id} in {}", dir.join(trick_packs::TRICKS_DIR).display());
    }
    match packs.is_empty() {
        true => println!("tricks: no trick pack on in {}; the original's", dir.join(trick_packs::TRICKS_DIR).display()),
        false => println!("tricks: MX vs. ATV's, from {}", packs.iter().map(|p| p.id.as_str()).collect::<Vec<_>>().join(" then ")),
    }
    Ok(packs)
}

/// The trick pack whose `mx_*.phy` the bikes ride on MX vs. ATV's handling (MD40): only with
/// `--handling mxvatv` or the Handling row's MX vs. ATV, never on `--online`, and only when the mods
/// folder has one; the line says why not. [`Args::handling_pack`] when the folder has it (MD77),
/// else the first of the ride's trick packs, else the folder's first. A lobby's ride rides its
/// room's pack (N21) or the original's handling.
fn ride_handling_pack(args: &Args, tricks: &[trick_packs::Pack]) -> Result<Option<trick_packs::Pack>, String> {
    if let Some((dir, id)) = &args.room_handling {
        let pack = trick_packs::by_id(dir, id).map_err(|e| format!("the room's handling: {e}"))?;
        println!("handling: MX vs. ATV's, from the room's {}", pack.id);
        return Ok(Some(pack));
    }
    if !matches!(args.handling, Some(game_sim::handling::Handling::MxVsAtv | game_sim::handling::Handling::Untamed)) {
        return Ok(None);
    }
    if args.online.is_some() {
        println!("handling: the original's on --online, which has no handshake to hold every seat to it");
        return Ok(None);
    }
    let Some(dir) = mods_dir(args) else {
        println!("handling: the mods folder is off, so the original's");
        return Ok(None);
    };
    let named = args.handling_pack.clone().unwrap_or_default();
    let found = match (named.is_empty(), tricks.first()) {
        (false, _) => trick_packs::by_id(&dir, &named).or_else(|e| {
            println!("handling: {e}");
            match tricks.first() {
                Some(pack) => trick_packs::by_id(&dir, &pack.id),
                None => trick_packs::find(&dir),
            }
        }),
        (true, Some(pack)) => trick_packs::by_id(&dir, &pack.id),
        (true, None) => trick_packs::find(&dir),
    };
    match found {
        Ok(pack) => Ok(Some(pack)),
        Err(e) => {
            println!("handling: {e}; the original's");
            Ok(None)
        }
    }
}

/// MX vs. ATV's biplane and Kiowa (MD80e): the machine pack of kind `fliers` in the mods folder, with
/// `--fliers mxvatv` or the Fliers row's MX vs. ATV, never on `--online` (no handshake holds every
/// seat to it, and the plane's switch is off the wire, MD100), and the skin the first player's wear.
/// `None`, with the line saying why, flies MX Unleashed's.
fn ride_fliers(args: &Args) -> Option<vehicle::MxvatvFliers> {
    if args.fliers != Some(true) {
        return None;
    }
    if args.online.is_some() || args.room_handling.is_some() || args.room_tricks.is_some() {
        println!("fliers: MX Unleashed's online, which has no handshake to hold every seat to MX vs. ATV's");
        return None;
    }
    let Some(dir) = mods_dir(args) else {
        println!("fliers: the mods folder is off, so MX Unleashed's");
        return None;
    };
    let (packs, why) = machine_packs::find(&dir);
    for w in &why {
        println!("fliers: {w}");
    }
    let skin = args.flier_skin.unwrap_or(0).min(3);
    match packs.into_iter().find(|p| p.machine == vehicle::MxvatvFliers::MACHINE) {
        Some(pack) => {
            let fliers = vehicle::MxvatvFliers::new(pack.id, pack.files, skin);
            println!("fliers: MX vs. ATV's, from {}, skin {} (model {})", fliers.id, skin + 1, Machine::mxvatv_model(skin));
            Some(fliers)
        }
        None => {
            println!("fliers: no fliers pack in {}; MX Unleashed's", dir.join(machine_packs::MACHINES_DIR).display());
            None
        }
    }
}

/// Untamed's handling (MD93): a bike read for MX vs. ATV's rides Untamed's reworked steps with
/// `--handling untamed` or the Handling row's Untamed. Online it stays MX vs. ATV's, as a room names
/// only its pack.
fn untamed_handling(args: &Args, phy: &mut Phy) {
    if args.handling == Some(game_sim::handling::Handling::Untamed) && phy.handling == game_sim::handling::Handling::MxVsAtv {
        phy.handling = game_sim::handling::Handling::Untamed;
    }
}

/// MX vs. ATV's Brake Gyro and Gas Gyro options (P226) on a bike read for its handling: `--brake-gyro`
/// and `--gas-gyro`, each unset the game's default. A lobby's ride keeps the default on every seat.
fn gyro_options(args: &Args, phy: &mut Phy) {
    if phy.handling == game_sim::handling::Handling::Original || args.room_handling.is_some() {
        return;
    }
    let d = game_sim::handling::Gyro::default();
    phy.gyro = game_sim::handling::Gyro { brake: args.brake_gyro.unwrap_or(d.brake), gas: args.gas_gyro.unwrap_or(d.gas) };
    if phy.gyro != d {
        println!("handling: brake gyro {}, gas gyro {}", on_off_word(phy.gyro.brake), on_off_word(phy.gyro.gas));
    }
}

/// `on` or `off`, as a line says a switch.
fn on_off_word(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

/// A bike read for MX vs. ATV's handling (MD40) from `pack`'s file for `member`, saying so after
/// `who`; `None`, with the reason said, rides the disc's file.
fn handling_phy(pack: Option<&trick_packs::Pack>, member: &str, curve: &game_sim::camera::CurveTable, who: &str) -> Option<Phy> {
    match pack?.handling(member, curve) {
        Ok((phy, line)) => {
            println!("{who}{line}");
            Some(phy)
        }
        Err(line) => {
            println!("{who}{line}");
            None
        }
    }
}

/// A lobby seat's bike on the room's handling (N21): `pack`'s file for the bike set `set`
/// (`race125p`), read as on every machine; `None`, with the reason said, rides the disc's file.
fn room_handling_phy(pack: Option<&trick_packs::Pack>, set: &str, curve: &game_sim::camera::CurveTable) -> Option<Phy> {
    // A person's seat, so the P file the ride loads (`Set::ridden`), as `lay_room_tricks` reads it.
    let set = roster::Set::from_player_word(set)?;
    handling_phy(pack, &set.file(), curve, &format!("{}: ", set.word()))
}

/// Lay a lobby ride's trick packs (MD19, MD77) over a seat's bike file, read for the bike set `set`
/// (`race125p`) as the handshake hashed it: what every machine lays over that seat.
fn lay_room_tricks(packs: &[trick_packs::Pack], phy: &mut Phy, set: &str) {
    // A person's seat, so the P file the ride loads (`Set::ridden`), whatever the word's tuning.
    if let (false, Some(set)) = (packs.is_empty(), roster::Set::from_player_word(set)) {
        println!("{set}: {}", trick_packs::lay(packs, phy, &set.file()), set = set.word());
    }
}

/// The track mods a front end lists (MD2), saying why each one left out was: `--mods`' folder if it
/// was given (`off` is none), else the one beside the settings in `dir` while the Mods row is `on`.
/// With none, nothing is read.
fn mod_tracks(args: &Args, dir: &std::path::Path, on: bool) -> Vec<(String, String)> {
    let Some(folder) = mods_folder(args, dir, on) else { return Vec::new() };
    let scan = track_mods::scan(&folder);
    for line in &scan.log {
        mods_line(line.clone());
    }
    let worlds = worlds::listed(&folder);
    if !scan.tracks.is_empty() || !worlds.is_empty() {
        mods_line(format!("mods: {} tracks, {} worlds", scan.tracks.len(), worlds.len()));
    }
    scan.tracks.into_iter().map(|t| (t.id, t.name)).chain(worlds).collect()
}

/// A `mods:` line: standard output on the desktop; in the browser, whose standard output goes
/// nowhere, the page's console (P217: the installed track listed, and ridden, in headless chromium).
fn mods_line(line: String) {
    #[cfg(not(target_arch = "wasm32"))]
    println!("{line}");
    #[cfg(target_arch = "wasm32")]
    crate::host_say(&line);
}

/// The music packs in the same folder as [`mod_tracks`]' (P122), for the Mods page.
fn music_packs(args: &Args, dir: &std::path::Path, on: bool) -> Vec<(String, String, usize)> {
    let Some(folder) = mods_folder(args, dir, on) else { return Vec::new() };
    let packs = track_mods::music_packs(&folder);
    if !packs.is_empty() {
        println!("mods: {} music packs", packs.len());
    }
    packs
}

/// The trick packs in the mods folder (MD77), for the Ride tab's rows: listed while the Mods row is
/// off too, as the rows grey; with the extras each could add past `stunts`, the rider's own 24.
fn trick_pack_rows(args: &Args, dir: &std::path::Path, stunts: &Set) -> Vec<menu::TrickPack> {
    let Some(folder) = mods_folder(args, dir, true) else { return Vec::new() };
    let (packs, why) = trick_packs::listed(&folder);
    for line in why {
        println!("tricks: {line}");
    }
    packs.iter().map(|p| menu::TrickPack { id: p.id.clone(), name: p.name.clone(), extras: p.extra_names(stunts) }).collect()
}

/// The first rider's 24 stunts off the disc, as a ride reads them, for the front end's count of
/// each trick pack's extras (MD77); none when the disc's table cannot be read, which counts every
/// row of a pack.
fn front_rider_stunts(disc: &mut Disc) -> Set {
    let read = member_ending(disc, STUNT_ARCHIVE)
        .and_then(|name| disc.res(&name))
        .and_then(|res| res.read_suffix(STUNT_TABLE).map_err(|e| format!("{STUNT_ARCHIVE}: {e}")))
        .and_then(|bytes| Stunts::parse(&bytes));
    match read {
        Ok(table) => Set::new(table.selection(0)),
        Err(e) => {
            println!("tricks: the rider's stunts: {e}");
            Set::default()
        }
    }
}

/// A mod kind as the store's pages name it: its tab's name, but World for a world (P241).
fn store_kind(kind: modstore::Kind) -> &'static str {
    match kind {
        modstore::Kind::World => "World",
        k => k.tab().map_or("Mod", |t| menu::STORE_TABS[t]),
    }
}

/// The mod store as the front end runs it (P198): `modstore.rs`'s store, its last listing, and the
/// previews asked for, by the zip's sha256.
struct StoreFront {
    store: modstore::Store,
    listing: Vec<modstore::Entry>,
    asked: std::collections::HashSet<String>,
    /// P199: the signed-in account's mods on the store (My Mods), and the player's own mods (Upload).
    mine: Vec<modstore::Entry>,
    locals: Vec<modstore::up::Local>,
}

impl StoreFront {
    /// The store, at `--mod-store`'s address if given (a test's own), installing into the mods
    /// folder whether or not it is read; none under `--mods off`.
    fn new(args: &Args, dir: &std::path::Path) -> Option<StoreFront> {
        let mods = mods_folder(args, dir, true)?;
        let base = args.mod_store.clone().unwrap_or_else(|| modstore::HF.to_string());
        Some(StoreFront { store: modstore::Store::new(&base, dir, &mods), listing: Vec::new(), asked: Default::default(), mine: Vec::new(), locals: Vec::new() })
    }

    fn entry(&self, key: &str) -> Option<&modstore::Entry> {
        self.listing.iter().find(|e| e.key() == key)
    }

    /// The menu's cards from the listing, against what is installed; an install's bar kept.
    fn cards(&self, menu: &mut menu::Menu) {
        let (stamps, own) = (self.store.stamps(), self.store.own_ids());
        menu.store.web = cfg!(target_arch = "wasm32");
        let old = std::mem::take(&mut menu.store.cards);
        menu.store.cards = self
            .listing
            .iter()
            .filter_map(|e| {
                let got = match modstore::Store::have(e, &stamps, &own) {
                    modstore::Have::No => menu::Got::No,
                    modstore::Have::Installed => menu::Got::Installed,
                    modstore::Have::Update => menu::Got::Update,
                    modstore::Have::Own => menu::Got::Own,
                };
                let progress = old.iter().find(|c| c.key == e.key()).and_then(|c| c.progress.clone());
                Some(menu::Card {
                    key: e.key(),
                    tab: e.meta.kind.tab()?,
                    name: e.meta.name.clone(),
                    author: e.meta.author.clone(),
                    source: e.meta.source.clone(),
                    category: e.meta.category_word(),
                    size: e.meta.size,
                    world: e.meta.kind == modstore::Kind::World,
                    verified: e.verified,
                    got,
                    picture: format!("store:{}", e.meta.sha256),
                    progress,
                })
            })
            .collect();
    }

    /// P199: the Upload page's mods from the mods folder (an upload's state kept), and the account.
    fn locals(&mut self, menu: &mut menu::Menu) {
        self.locals = self.store.locals();
        menu.store.account = self.store.signed_in();
        menu.store.web = cfg!(target_arch = "wasm32");
        let old = std::mem::take(&mut menu.store.locals);
        menu.store.locals = self
            .locals
            .iter()
            .map(|l| menu::Upload {
                id: l.id.clone(),
                name: l.name.clone(),
                kind: store_kind(l.kind).to_string(),
                status: old.iter().find(|o| o.id == l.id).and_then(|o| o.status.clone()),
            })
            .collect();
    }

    /// P199: My Mods from the account's mods on the store.
    fn posted(&self, menu: &mut menu::Menu) {
        menu.store.mine = self
            .mine
            .iter()
            .map(|e| menu::Posted {
                key: e.key(),
                name: e.meta.name.clone(),
                kind: store_kind(e.meta.kind).to_string(),
                size: e.meta.size,
                verified: e.verified,
                pr: modstore::up::pr_of(&e.rev),
                busy: false,
            })
            .collect();
    }

    /// P230: Pack Order's texture packs from the mods folder, in settings.txt's order and switches.
    fn packs(&self, menu: &mut menu::Menu) {
        let s = &menu.settings;
        let listed = textures::listed(self.store.mods(), &s.texture_packs, &s.texture_packs_off);
        menu.store.packs = listed.into_iter().map(|l| menu::PackRow { folder: l.folder, name: l.name, textures: l.textures, on: l.on }).collect();
    }

    /// P199's asks: the sign-in, uploads, My Mods and take-downs; P230's Pack Order.
    fn upload_ask(&mut self, ask: menu::StoreAsk, menu: &mut menu::Menu) {
        match ask {
            menu::StoreAsk::Packs => self.packs(menu),
            menu::StoreAsk::Locals => self.locals(menu),
            menu::StoreAsk::Tokens => {
                if let Err(why) = modstore::open(modstore::up::TOKENS) {
                    menu.store.note = Some(format!("Make a write token at huggingface.co/settings/tokens: {why}"));
                }
            }
            menu::StoreAsk::Paste => match modstore::up::clipboard() {
                Ok(text) => self.store.sign_in(&text),
                Err(why) => {
                    menu.store.signing = false;
                    menu.store.note = Some(why);
                }
            },
            menu::StoreAsk::SignOut => {
                self.store.sign_out();
                self.mine.clear();
                println!("mod store: signed out");
            }
            menu::StoreAsk::Upload(ids) => {
                let go: Vec<_> = self.locals.iter().filter(|l| ids.contains(&l.id)).cloned().collect();
                println!("mod store: uploading {}", ids.join(", "));
                self.store.upload(go);
            }
            menu::StoreAsk::Mine => self.store.mine(),
            menu::StoreAsk::TakeDown(key) => {
                if let Some(e) = self.mine.iter().find(|e| e.key() == key) {
                    println!("mod store: taking down {key} ({})", e.rev);
                    self.store.take_down(e, &self.mine);
                }
            }
            _ => {}
        }
    }

    /// One frame: the menu's asks to the store, the store's events to the menu and its previews to
    /// the art. Whether the mods folder changed, and whether the art did.
    fn frame(&mut self, menu: &mut menu::Menu, art: &mut Art) -> (bool, bool) {
        let (mut rescan, mut drawn) = (false, false);
        for ask in menu.take_store_asks() {
            match ask {
                menu::StoreAsk::List { fresh } => self.store.list(fresh),
                menu::StoreAsk::Reload => rescan = true,
                menu::StoreAsk::Install(key) => {
                    if let Some(e) = self.entry(&key) {
                        println!("mod store: installing {key} ({}, {} bytes)", e.rev, e.meta.size);
                        self.store.install(e);
                    }
                }
                menu::StoreAsk::Remove(key) => {
                    if let Some(e) = self.entry(&key) {
                        self.store.remove(&key, e.meta.kind, &e.meta.id);
                    }
                }
                menu::StoreAsk::Report(key) => {
                    if let Some(e) = self.entry(&key) {
                        let url = modstore::report_url(e);
                        println!("mod store: report {key}: {url}");
                        if let Err(why) = modstore::open(&url) {
                            menu.store.note = Some(format!("Report it at huggingface.co/datasets/{}: {why}", modstore::REPO));
                        }
                    }
                }
                ask => self.upload_ask(ask, menu),
            }
        }
        let name = |s: &StoreFront, key: &str| s.entry(key).map_or(key.to_string(), |e| e.meta.name.clone());
        for ev in self.store.take() {
            match ev {
                modstore::Event::Listing(Ok(l)) => {
                    self.listing = l;
                    menu.store.listed = menu::Listed::Shown;
                    self.cards(menu);
                }
                modstore::Event::Listing(Err(why)) => menu.store.listed = menu::Listed::Failed(why),
                modstore::Event::Note(n) => menu.store.note = Some(n),
                modstore::Event::Progress(key, share, line) => {
                    if let Some(c) = menu.store.cards.iter_mut().find(|c| c.key == key) {
                        c.progress = Some((share, line));
                    }
                }
                modstore::Event::Installed(ref key, ref got) | modstore::Event::Removed(ref key, ref got) => {
                    let (key, got) = (key.clone(), got.clone());
                    let what = name(self, &key);
                    let removed = matches!(ev, modstore::Event::Removed(..));
                    if removed {
                        if let Some(c) = menu.store.cards.iter_mut().find(|c| c.key == key) {
                            c.progress = None;
                        }
                    } else {
                        // P293: the batch's last one in asks to reload, which reads the folder
                        menu.store_installed(&key, got.is_ok());
                    }
                    let done = if removed { "Removed" } else { "Installed" };
                    menu.store.note = Some(match &got {
                        Ok(_) => format!("{done} {what}"),
                        Err(why) => format!("{what}: {why}"),
                    });
                    println!("mod store: {key}: {got:?}");
                    rescan |= removed && got.is_ok();
                    self.cards(menu);
                }
                modstore::Event::Preview(sha, Some(p)) => {
                    art.set_mod_picture(&format!("store:{sha}"), game_render::pst::Texture { width: p.width, height: p.height, rgba: p.rgba });
                    drawn = true;
                }
                modstore::Event::Preview(_, None) => {}
                modstore::Event::Signed(got) => {
                    menu.store.signing = false;
                    println!("mod store: sign-in: {}", got.as_ref().map_or_else(|e| e.as_str(), |n| n.as_str()));
                    menu.store.note = Some(match &got {
                        Ok(name) => format!("Signed in as {name}"),
                        Err(why) => why.clone(),
                    });
                    if got.is_ok() {
                        self.locals(menu);
                    }
                }
                modstore::Event::Upload(id, up) => {
                    use modstore::up::Up;
                    let phase = match &up {
                        Up::Waiting | Up::Working(_) => menu::Phase::Busy,
                        Up::Done(_) | Up::Skipped(_) => menu::Phase::Done,
                        Up::Failed(_) => menu::Phase::Failed,
                    };
                    if up.over() {
                        println!("mod store: upload {id}: {}", up.line());
                        menu.store.note = Some(up.line().to_string());
                    }
                    if let Some(l) = menu.store.locals.iter_mut().find(|l| l.id == id) {
                        l.status = Some((phase, up.line().to_string()));
                    }
                }
                modstore::Event::Mine(Ok(l)) => {
                    self.mine = l;
                    menu.store.mine_listed = menu::Listed::Shown;
                    self.posted(menu);
                }
                modstore::Event::Mine(Err(why)) => menu.store.mine_listed = menu::Listed::Failed(why),
                modstore::Event::TakenDown(key, got) => {
                    println!("mod store: take-down {key}: {got:?}");
                    if got.is_ok() {
                        // the store's own copy may linger in a PR the index read: My Mods reads again
                        self.mine.retain(|e| e.key() != key);
                        self.posted(menu);
                    } else if let Some(m) = menu.store.mine.iter_mut().find(|m| m.key == key) {
                        m.busy = false;
                    }
                    menu.store.note = Some(got.unwrap_or_else(|why| format!("Not taken down: {why}")));
                }
            }
        }
        // The lit tab's previews, each zip's once (B259: an update's new zip is a new picture).
        if menu.screen() == Some(menu::Screen::Store) {
            for i in menu.store.shown() {
                let key = menu.store.cards[i].key.clone();
                if let Some(e) = self.listing.iter().find(|e| e.key() == key)
                    && self.asked.insert(e.meta.sha256.clone())
                {
                    self.store.preview(e);
                }
            }
        }
        (rescan, drawn)
    }
}

/// The listed mod tracks' source games (MD11), which the pick's filters are by.
fn mod_games(args: &Args, dir: &std::path::Path, on: bool, listed: &[(String, String)]) -> Vec<(String, String)> {
    let Some(folder) = mods_folder(args, dir, on) else { return Vec::new() };
    let ids: Vec<String> = listed.iter().map(|(id, _)| id.clone()).collect();
    track_mods::source_games(&folder, &ids)
}

/// The listed mod tracks' categories (MD66), which the pick's second filter is by.
fn mod_categories(args: &Args, dir: &std::path::Path, on: bool, listed: &[(String, String)]) -> Vec<(String, String)> {
    let Some(folder) = mods_folder(args, dir, on) else { return Vec::new() };
    let ids: Vec<String> = listed.iter().map(|(id, _)| id.clone()).collect();
    track_mods::categories(&folder, &ids)
}

/// The mods folder a front end reads, as [`mod_tracks`] says: `--mods`' if it was given, else the
/// one beside the settings in `dir` while the Mods row is `on`.
fn mods_folder(args: &Args, dir: &std::path::Path, on: bool) -> Option<std::path::PathBuf> {
    match (args.mods_asked, on) {
        (true, _) => args.mods.clone(),
        (false, true) => Some(dir.join(track_mods::MODS_DIR)),
        (false, false) => None,
    }
}

/// The listed mod tracks' pictures into the menu's art (P70): each mod's own or the still taken
/// before, and the job that takes the stills still missing, one headless ride's first frame each.
fn mod_pictures(
    art: &mut Art,
    folder: Option<&std::path::Path>,
    listed: &[(String, String)],
) -> Option<std::sync::mpsc::Receiver<(String, Result<game_render::pst::Texture, String>)>> {
    let folder = folder?;
    let ids: Vec<String> = listed.iter().map(|(id, _)| id.clone()).collect();
    let mut log = Vec::new();
    let (have, take) = track_previews::gather(folder, &ids, &mut log);
    for line in log {
        mods_line(line);
    }
    if !have.is_empty() {
        mods_line(format!("mods: pictures of {}", have.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>().join(", ")));
    }
    for (id, picture) in have {
        art.set_mod_picture(&id, picture);
    }
    if take.is_empty() {
        return None;
    }
    let exe = std::env::current_exe().map_err(|e| println!("mods: no stills of the spawns: {e}")).ok()?;
    println!("mods: taking a still of {} from the spawn", take.join(", "));
    Some(track_previews::still_job(exe, folder.to_path_buf(), take))
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
/// Why a ride cannot take a change of modifiers from its pause (P98), or `None` when it can. A played
/// tape rides the modifiers it was made on and changes them where it did. Online (`Some(host)`) every
/// machine's ride has to change at the same tick: the host's change goes to all of them with a
/// restart of the session, and they take it there (P100). `Some((host, own_rows))`: a guest changes
/// its own seat's the same way on a ride from the lobby, where each seat has its own (P134), and
/// cannot on one opened to online, which rides the host's for every seat.
fn mods_lock(playing: bool, online: Option<(bool, bool)>) -> Option<String> {
    match (playing, online) {
        (true, _) => Some("A played tape rides its own modifiers: they change where it changed them".into()),
        (_, Some((false, false))) => Some("Online only the host changes the modifiers: every rider takes them at the same moment".into()),
        _ => None,
    }
}

/// Why a ride cannot take a change of Air Wrecks from its pause (P138), or `None` when it can. A
/// played tape switches it where it was switched; online the room's switch, agreed by every machine at
/// the handshake, holds for the ride.
fn air_lock(playing: bool, online: bool) -> Option<String> {
    match (playing, online) {
        (true, _) => Some("A played tape switches Air Wrecks where it was switched".into()),
        (_, true) => Some("Online the room's Air Wrecks holds for the ride".into()),
        _ => None,
    }
}

/// Whether a ride on `ride` (its track mod, `None` on a disc track) can open to online (F6): a disc
/// track can, and a track mod can when the lobby's mods folder `mods` (the settings' Mods row's, which
/// the handshake hashes, `lobby::DiscFiles::track`) finds `track` in the folder the ride was built
/// from, so a joiner's ride is held to the same files and finds them as the lobby's ride does. A world
/// mod (MD8, `world_mod`) cannot: the handshake hashes its home tile's folder only.
fn opens_on_mod(ride: Option<&track_mods::ModTrack>, world_mod: bool, mods: Option<&std::path::Path>, track: &str) -> bool {
    match ride {
        None => true,
        Some(_) if world_mod => false,
        Some(t) => mods.and_then(|d| track_mods::find(d, track).ok()).is_some_and(|found| found.dir == t.dir),
    }
}

/// **The ride takes another set of modifiers, from where it is** (P98): every seat's bike built again
/// from its stock file `stock[i]` on `to` ([`Mods::rebike`](game_sim::modifiers::Mods::rebike)), its
/// biplane retuned off the stock `plane` the same way, its cars and helicopter put on `to` off `lot`'s
/// stock files ([`Garage::modify`], P272), and the HUD's mark said again. `was` is the set
/// the ride had. Nothing is moved, stopped or put back. Each plane camera's eye takes `fog`, the
/// scene's ceiling for it, or none if the seat's plane has no ceiling (P136).
fn remodify(
    seats: &mut [Seat],
    rigs: &mut [Rig],
    stock: &[Phy],
    plane: Option<&phy::Plane>,
    lot: &Lot,
    was: &game_sim::modifiers::Mods,
    to: &game_sim::modifiers::Mods,
    fog: F,
) {
    for (i, seat) in seats.iter_mut().enumerate() {
        if let Some(file) = stock.get(i) {
            to.resim(was, i, file, &mut seat.sim);
        }
        seat.garage.modify(lot, to, i);
        if let (Some(file), Some(flight)) = (plane, seat.garage.plane.as_mut().and_then(|p| p.flight.as_mut())) {
            to.replane(i, file, flight);
        }
    }
    for (i, rig) in rigs.iter_mut().enumerate() {
        rig.modified = !to.seat_is_original(i);
        if let Some(cam) = &mut rig.plane_camera {
            cam.cam.ceiling = to.plane_eye_ceiling(i, fog);
        }
        if let Some(cam) = &mut rig.heli_camera {
            cam.cam.ceiling = to.plane_eye_ceiling(i, fog);
        }
    }
}

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
/// The track's archive with its `<stem>.toy` swapped for its split-screen twin's, `<stem>_ss.toy`
/// out of `<stem>_ss.res` (V105). A track with no twin on the disc (the supercross and national
/// tracks) is handed back as it was.
fn split_screen_toy(disc: &mut Disc, track: Res, stem: &str) -> Result<Res, String> {
    let Ok(member) = member_ending(disc, &format!("{stem}_ss.res")) else { return Ok(track) };
    let twin = disc.res(&member)?.read_suffix(&format!("{stem}_ss.toy"))?;
    let own = track.find_suffix(&format!("{stem}.toy")).map(|e| e.name.clone()).ok_or(format!("{stem} has no {stem}.toy"))?;
    println!("split screen: {own} read from {member}");
    track.with_member(&own, twin)
}

#[cfg(test)]
mod split_screen_toy {
    //! V105: two views read the `_ss` twin's `.toy`, and a track without a twin keeps its own.
    use super::{Disc, member_ending, split_screen_toy};
    use game_render::animated_toy::Animated;

    #[test]
    fn two_views_read_the_twins_toy() {
        let Ok(mut disc) = Disc::open_configured() else {
            eprintln!("skipped: no disc configured");
            return;
        };
        let open = |disc: &mut Disc, stem: &str| {
            let track = disc.res(&member_ending(disc, &format!("{stem}.res")).unwrap()).unwrap();
            let second = disc.res(&member_ending(disc, &format!("{stem}_2.res")).unwrap()).unwrap();
            (track, second)
        };
        // free09: AnimateWhenNotVisible False on its five windmills, True on free09_ss's.
        for split in [false, true] {
            let (track, second) = open(&mut disc, "free09");
            let track = if split { split_screen_toy(&mut disc, track, "free09").unwrap() } else { track };
            let animated = Animated::load(&mut vec![track, second], "free09");
            let unseen: Vec<bool> = animated.sim.specs.iter().map(|s| s.unseen).collect();
            assert_eq!(unseen, [split; 5], "free09's windmills, split {split}");
        }
        // free11: its first three cars' TimeScale, 0.3 alone and 0.5, 0.5, 0.2 in split screen, each
        // the float at or just below the text, as the reader truncates.
        for (split, want) in [(false, [0.29999998, 0.29999998, 0.29999998]), (true, [0.5, 0.5, 0.19999999])] {
            let (track, _) = open(&mut disc, "free11");
            let track = if split { split_screen_toy(&mut disc, track, "free11").unwrap() } else { track };
            let text = game_render::ini::text(&track.read_suffix("free11.toy").unwrap());
            let scales: Vec<f32> = game_render::toy::cars(&text).iter().take(3).map(|c| c.time_scale).collect();
            assert_eq!(scales, want, "free11's cars, split {split}");
        }
        // sx01 has no twin on the disc: its own file, unchanged.
        let (track, _) = open(&mut disc, "sx01");
        let own = track.read_suffix("sx01.toy").unwrap();
        assert_eq!(split_screen_toy(&mut disc, track, "sx01").unwrap().read_suffix("sx01.toy").unwrap(), own);
    }
}

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
    kit: Option<&Res>,
    track: &str,
    drawn: &Biplane,
    assets: &Assets,
) -> Result<(particles::PlaneFx, Vec<(String, M4)>, game_render::pst::Texture), String> {
    // The track's own archives first, then the plane's kit's where the track has none of its files (P113).
    let read = |name: &str| archives.iter().chain(kit).find_map(|a| a.read_suffix(&format!("\\{name}")).ok());
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
/// its throttle, airspeed and wreck, and its collision object's contacts (P63, S48's search: the
/// wreck's dust and chunks). Its three
/// tires as the tire dust reads them (P60): the ground under the bottom of the tire, the held
/// contact (only ever cleared, so never an object), the strut's compression and the surface under it.
fn plane_fx_frame(plane: &Plane, binds: &[(String, M4)]) -> particles::PlaneFrame {
    let at = plane_at(plane);
    let (throttle, airspeed) = plane.gauges().map_or((0.0, 0.0), |(t, a)| (t.0, a.0));
    let mut frame = particles::PlaneFrame {
        nodes: binds.iter().map(|(n, m)| (n.clone(), m.mul(at))).collect(),
        throttle,
        airspeed,
        wrecked: plane.prop_angle().is_none(),
        vel: vector(plane.vel),
        tires: plane.flight.as_ref().map_or_else(Vec::new, |f| {
            f.tires
                .iter()
                .map(|t| particles::TireFrame { ground: t.below, object: t.held, compression: t.compression.0, zone: t.surface, ..Default::default() })
                .collect()
        }),
        ..Default::default()
    };
    // Its collision object's contacts (P63), which the wreck's dust and chunks come off.
    if let Some(f) = &plane.flight {
        frame.touch(&f.record, f.collider.relative);
    }
    frame
}

/// The Kiowa's particles (V114): `helicopterparticles.dat` out of the track's archives (a `_ss`
/// track's own copy, or the freeride one in its second archive) and each `.emt` it names, the
/// track's particle sheet, and where each entry's node is on the helicopter's model. Says what it
/// found, and what it skips.
fn load_heli_fx(
    archives: &[Res],
    track: &str,
    drawn: &Kiowa,
    assets: &Assets,
) -> Result<(particles::PlaneFx, Vec<(String, M4)>, game_render::pst::Texture), String> {
    let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(&format!("\\{name}")).ok());
    let list = read(particles::KIOWA_PARTICLES).ok_or(format!("no {}", particles::KIOWA_PARTICLES))?;
    let sheet = read(&particles::sheet(track)).ok_or(format!("no {}", particles::sheet(track)))?;
    let texture = read(&particles::sheet_texture(track)).ok_or(format!("no {}", particles::sheet_texture(track)))?;
    let texture = game_render::pst::Texture::parse(&texture)?;
    let atlas = particles::Atlas::parse(&ini::text(&sheet), texture.width, texture.height);
    let fx = particles::PlaneFx::new(&biplane::emitters(&ini::text(&list)), atlas, read);
    let model = assets.models.get(&drawn.model).map(|l| &l.model);
    let mut binds: Vec<(String, M4)> = Vec::new();
    for e in &fx.entries {
        if let Some(name) = e.node.as_deref() {
            let at = model.and_then(|m| m.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(name)).map(|i| m.world(i)));
            match at {
                Some(m) if !binds.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)) => binds.push((name.to_string(), m)),
                Some(_) => {}
                None => println!("kiowa: particles: {} rides {name}, which the model does not have", e.name),
            }
        }
        println!("kiowa: particles: {} ({:?}) from {:?}", e.name, e.kind, e.emt.group);
    }
    for (name, why) in &fx.skipped {
        println!("kiowa: particles: {name} ({why}) not ported");
    }
    Ok((fx, binds, texture))
}

/// A car's particles (S248): its list ([`particles::car_list`]) out of the track's archives (a
/// freeride track's second one) and each `.emt` it names, the track's particle sheet, and where each
/// entry's node is on the car's model (`drawn`), all on its root. Says what it found, and what it
/// skips: the tire tracks, which are not particles.
fn load_car_fx(
    archives: &[Res],
    track: &str,
    machine: Machine,
    drawn: Option<&game_render::car::Car>,
    assets: &Assets,
) -> Result<(particles::PlaneFx, Vec<(String, M4)>, game_render::pst::Texture), String> {
    let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(&format!("\\{name}")).ok());
    let name = particles::car_list(machine.dir()).ok_or("no list for it")?;
    let list = read(name).ok_or(format!("no {name}"))?;
    let sheet = read(&particles::sheet(track)).ok_or(format!("no {}", particles::sheet(track)))?;
    let texture = read(&particles::sheet_texture(track)).ok_or(format!("no {}", particles::sheet_texture(track)))?;
    let texture = game_render::pst::Texture::parse(&texture)?;
    let atlas = particles::Atlas::parse(&ini::text(&sheet), texture.width, texture.height);
    let fx = particles::PlaneFx::new(&biplane::emitters(&ini::text(&list)), atlas, read);
    let model = drawn.and_then(|d| assets.models.get(&d.model)).map(|l| &l.model);
    let dir = machine.dir();
    let mut binds: Vec<(String, M4)> = Vec::new();
    for e in &fx.entries {
        if let Some(name) = e.node.as_deref() {
            let at = model.and_then(|m| m.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(name)).map(|i| m.world(i)));
            match at {
                Some(m) if !binds.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)) => binds.push((name.to_string(), m)),
                Some(_) => {}
                None => println!("car: {dir}: particles: {} rides {name}, which the model does not have", e.name),
            }
        }
    }
    // The `TireTrack` entries are not emitters: the tire tracks lay them (S269).
    let (laid, skipped): (Vec<_>, Vec<_>) = fx.skipped.iter().partition(|(_, why)| why.eq_ignore_ascii_case("TireTrack"));
    let names = |l: &[&(String, String)]| l.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", ");
    println!(
        "car: {dir}: particles: {} entries off {name}; {} laid by the tire tracks ({}); {} not run ({})",
        fx.entries.len(),
        laid.len(),
        names(&laid),
        skipped.len(),
        names(&skipped)
    );
    Ok((fx, binds, texture))
}

/// What the Kiowa's particles read off it this frame (V114): its nodes where the measure's matrix
/// puts the model, the wreck, the body's velocity, and the body's contacts. The shell's ground
/// stand-in for its collision record ([`vehicle::KiowaRide::skids`]) touches at one point, under the
/// skids; the thrown blades are not bodies here, so their models (1 to 4) touch nothing.
fn heli_fx_frame(heli: &vehicle::KiowaRide, binds: &[(String, M4)]) -> particles::PlaneFrame {
    let Some(m) = heli.matrix() else { return particles::PlaneFrame::default() };
    let Some(h) = heli.flown.as_ref() else { return particles::PlaneFrame::default() };
    let row = |r: Vf, w: f32| [r.x(), r.y(), r.z(), w];
    let at = M4([row(m.0[0], 0.0), row(m.0[1], 0.0), row(m.0[2], 0.0), row(m.0[3], 1.0)]);
    let vel = vector(h.ride.body.velocity);
    particles::PlaneFrame {
        nodes: binds.iter().map(|(n, b)| (n.clone(), b.mul(at))).collect(),
        wrecked: h.ride.crash.reason != 0,
        vel,
        airspeed: h.ride.measure.speed.0,
        ground_contacts: heli.skids().filter(|_| h.touching()).map(|p| vec![vector(p)]).unwrap_or_default(),
        contact_vel: Some(vel),
        ..Default::default()
    }
}

/// The ground under a point for the plane's particles (P60): its height and its surface, from the
/// ride's own terrain query. `None` off the terrain, where the query answers its no-ground depth.
fn plane_fx_ground(place: &mut impl sim_world::World, at: V3) -> Option<(f32, u8)> {
    let p = Vf::new(at.x, at.y, at.z, 1.0);
    let (on, _) = place.ground(p);
    (on.y() > -1.0e5).then(|| (on.y(), place.surface(on)))
}

/// The bike's particles (P74): the track's emitter list (`tracks\<track>\mxparticles.dat`) and each
/// `.emt` it names, out of the track's archives, and the track's particle sheet. Every seat is a
/// player's, so every seat rides the player's list and not `mxaiparticles.dat`. Says what it found.
fn load_bike_fx(archives: &[Res], track: &str) -> Result<(particles::PlaneFx, game_render::pst::Texture), String> {
    let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(&format!("\\{name}")).ok());
    let list = read(particles::BIKE_PARTICLES).ok_or(format!("no {}", particles::BIKE_PARTICLES))?;
    let sheet = read(&particles::sheet(track)).ok_or(format!("no {}", particles::sheet(track)))?;
    let texture = read(&particles::sheet_texture(track)).ok_or(format!("no {}", particles::sheet_texture(track)))?;
    let texture = game_render::pst::Texture::parse(&texture)?;
    let atlas = particles::Atlas::parse(&ini::text(&sheet), texture.width, texture.height);
    let fx = particles::PlaneFx::new(&biplane::emitters(&ini::text(&list)), atlas, read);
    for e in &fx.entries {
        println!("bike: particles: {} ({:?}) from {:?}", e.name, e.kind, e.emt.group);
    }
    for (name, why) in &fx.skipped {
        // The `TireTrack` entries are not emitters: `tire_tracks.rs` lays them (V40, P151).
        let how = if why.eq_ignore_ascii_case("TireTrack") { "laid by the tire tracks" } else { "not ported" };
        println!("bike: particles: {name} ({why}) {how}");
    }
    Ok((fx, texture))
}

/// How the rain's streaks are tinted (V73): matched by eye against the original's nat01.
const RAIN_TINT: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// The track's weather box (V73), its triggers and its material's texture, out of the track's
/// archives. Says what it found; `None` for a track with none.
fn load_rain(archives: &[Res], track: &str) -> Option<(weather::Rain, game_render::pst::Texture)> {
    let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(&format!("\\{name}")).ok());
    let toy = ini::text(&read(&format!("{track}.toy"))?);
    let weather = weather::boxes(&toy).into_iter().find(|b| b.rain)?;
    let texture = match read(&format!("{}.pst", weather.material)).map(|t| game_render::pst::Texture::parse(&t)) {
        Some(Ok(t)) => t,
        Some(Err(why)) => {
            println!("track: no rain: {}.pst: {why}", weather.material);
            return None;
        }
        None => {
            println!("track: no rain: no {}.pst", weather.material);
            return None;
        }
    };
    let rain = weather::Rain::new(weather, weather::triggers(&toy));
    println!("track: rain: {} ({} packets, density {})", rain.weather.name, rain.packets.len(), rain.density);
    for (t, actions, ..) in &rain.triggers {
        println!("track: rain: trigger {} ({} actions, {}%)", t.name, actions.len(), t.percent);
    }
    Some((rain, texture))
}

/// The track's own emitters (V45): the `.toy`'s list, each `.emt` it names, and the track's sheet,
/// out of the track's archives. Says what it runs and what it leaves out.
fn load_track_fx(archives: &[Res], track: &str) -> Result<(particles::TrackFx, game_render::pst::Texture), String> {
    let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(&format!("\\{name}")).ok());
    let toy = read(&format!("{track}.toy")).ok_or(format!("no {track}.toy"))?;
    let list = game_render::toy::emitters(&ini::text(&toy));
    if list.is_empty() {
        return Err(format!("{track}.toy has none"));
    }
    let sheet = read(&particles::sheet(track)).ok_or(format!("no {}", particles::sheet(track)))?;
    let texture = read(&particles::sheet_texture(track)).ok_or(format!("no {}", particles::sheet_texture(track)))?;
    let texture = game_render::pst::Texture::parse(&texture)?;
    let atlas = particles::Atlas::parse(&ini::text(&sheet), texture.width, texture.height);
    let fx = particles::TrackFx::new(&list, atlas, read).with_triggers(game_render::toy::particle_triggers(&ini::text(&toy)));
    for t in &fx.triggers {
        println!("track: particles: trigger {} ({} actions, {}%)", t.trigger.name, t.trigger.actions.len(), t.trigger.percent);
    }
    for e in &fx.fx.entries {
        println!("track: particles: {} ({}) from {:?}", e.name, e.node.as_deref().unwrap_or(""), e.emt.group);
    }
    for (name, why) in &fx.skipped {
        println!("track: particles: {name} ({why}) not run");
    }
    Ok((fx, texture))
}

/// What of a rider strikes a creature trigger (V75), and the track's fx and rain triggers (V103):
/// the bike's body rays at its chassis, as the original's collision pass meets the trigger's box (a
/// probe, struck down its tree and exit-tested by its root box since V104); a bike with no
/// collision files is its rider's position.
fn creature_rays(r: &game_sim::rider::Rider) -> game_render::creature::Actor<'_> {
    let Some(col) = r.bike_col.as_deref() else {
        let p = r.ride.position();
        return game_render::creature::Actor::Rays(game_render::creature::Rays::at(V3 { x: p.x(), y: p.y(), z: p.z() }));
    };
    game_render::creature::Actor::Probe(col.probe(r.ride.transform().0))
}

/// A track toy's node in the world (V71): the animated toy named `toy`'s joint named `node` at its
/// clip's last tick, on its place. `None` where the toy or the node is not there.
fn toy_node(animated: &game_render::animated_toy::Animated, sim: &game_sim::toys::Toys, toy: &str, node: &str) -> Option<M4> {
    use game_render::animated_toy::{palette, place};
    if sim.specs.len() != animated.names.len() {
        return None;
    }
    let k = animated.names.iter().position(|n| n.eq_ignore_ascii_case(toy))?;
    let j = animated.bones[k].iter().position(|b| b.eq_ignore_ascii_case(node))?;
    Some(palette(sim, k, 1.0).get(j)?.mul(place(sim, k)))
}

/// The track's sun flares (V72), each with its picture, out of the track's archives. Says what it
/// draws and what it cannot.
fn load_sun_flares(archives: &[Res], track: &str) -> Vec<(game_render::sun_flare::SunFlare, game_render::pst::Texture)> {
    let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(&format!("\\{name}")).ok());
    let Some(toy) = read(&format!("{track}.toy")) else {
        println!("track: no sun flares: no {track}.toy");
        return Vec::new();
    };
    let flares = game_render::sun_flare::flares(&ini::text(&toy));
    if flares.is_empty() {
        println!("track: no sun flares");
    }
    flares
        .into_iter()
        .filter_map(|flare| {
            let name = format!("{}.pst", flare.texture.to_ascii_lowercase());
            match read(&name).map(|d| game_render::pst::Texture::parse(&d)) {
                Some(Ok(picture)) => {
                    println!("track: sun flare: {} ({} px at {})", flare.texture, flare.screen_size, flare.alpha);
                    Some((flare, picture))
                }
                _ => {
                    println!("track: sun flare: {} has no picture, not drawn", flare.texture);
                    None
                }
            }
        })
        .collect()
}

/// What the bike's particles read off one rider this frame (P74), as the original's manager reads
/// the bike's snapshot: each entry's node where it is — the chassis' (the tires' particle nodes) or
/// a part's (the exhaust pipe's), on the bike's place — the throttle, the clutch and the chassis'
/// speed, the wreck, the two tires (front, rear: `AttachedToTire`'s 0 and 1) with their ground and
/// object contacts, surface, spin and side force; while down, the wreck body's contacts and their
/// relative velocity; and the
/// rider's collision spheres, the ragdoll's while down and the riding body's while up (P152), each with
/// the surface its own ground query last wrote (P186).
fn bike_fx_frame(fx: &particles::PlaneFx, rider: &Rider, rig: &Rig, assets: &Assets) -> particles::PlaneFrame {
    let ride = &rider.ride;
    let bike_at = match &rider.crash {
        Some(crash) => chassis_wrecked(crash),
        None => chassis_at(ride),
    };
    let chassis_node = |name: &str| -> Option<M4> {
        rig.chassis.as_ref().and_then(|c| c.node(name)).or_else(|| {
            let model = &assets.models.get(CHASSIS)?.model;
            let i = model.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(name))?;
            Some(model.world(i))
        })
    };
    let mut nodes: Vec<(String, M4)> = Vec::new();
    for name in fx.entries.iter().filter_map(|e| e.node.as_deref()) {
        if nodes.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)) {
            continue;
        }
        let part_node = || {
            rig.parts.iter().find_map(|part| {
                let model = &assets.models.get(part.model.as_str())?.model;
                let i = model.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(name))?;
                Some(model.world(i).mul(chassis_node(&part.placement).unwrap_or(M4::IDENTITY)))
            })
        };
        if let Some(m) = chassis_node(name).or_else(part_node) {
            nodes.push((name.to_string(), m.mul(bike_at)));
        }
    }
    let tire = |w: &game_sim::ride::Wheel| particles::TireFrame {
        ground: w.w.ray_hit != 0,
        object: w.w.swept_contact != 0,
        zone: w.surface,
        spinning: w.slip.spinning,
        side: w.side_per_load.0,
        ..Default::default()
    };
    let mut frame = particles::PlaneFrame {
        nodes,
        throttle: ride.held.gas.0,
        airspeed: ride.chassis.speed.0,
        clutch: ride.mv.clutch_in.0,
        wrecked: rider.crash.is_some(),
        vel: vector(ride.chassis.vel),
        tires: vec![tire(&ride.front), tire(&ride.rear)],
        ..Default::default()
    };
    // While down, the wreck bike's collision object's contacts, which its `CollisionModel` entries'
    // points spread along the object's relative velocity at its last resolved contact (P156), as the
    // plane's do (P63); the body's velocity stays the frame's for everything else.
    if let Some(crash) = &rider.crash {
        frame.vel = vector(crash.bike.body.velocity);
        frame.touch(&crash.bike.record, crash.bike.collider.relative);
    }
    // The rider's bone collisions, in the kit's order (the entries' mask bits): the wreck's ragdoll
    // while down, and while up the riding body's (P152), whose feet (collisions 0 and 1) raise Foot
    // Dust when they touch the ground. The riding body runs only where the shell hands it the grips.
    match (&rider.ragdoll, rider.riding.as_deref(), &rider.ragdoll_kit) {
        (Some(down), _, Some(kit)) => frame.constraints = constraint_frames(&down.contacts, &down.ragdoll, kit),
        (None, Some(riding), Some(kit)) => frame.constraints = constraint_frames(&riding.actor.contacts, &riding.actor.ragdoll, kit),
        _ => {}
    }
    frame
}

/// The rider's bone collisions as the ParticleCollisionConstraint entries read them (P74, P152): each
/// one's ground and scene flags (the manager sets the two on their own, so a sphere on both is an
/// object contact too, which every rider entry's Disable turns off), the point it was pushed from,
/// its bone's velocity, and the surface byte its own ground query last wrote (P186), which the
/// original's entries read as the sphere's zone.
fn constraint_frames(
    contacts: &[game_sim::ragdoll_contact::Contact],
    ragdoll: &game_sim::ragdoll::Ragdoll,
    kit: &game_sim::ragdoll_ride::Kit,
) -> Vec<particles::ConstraintFrame> {
    contacts
        .iter()
        .zip(&kit.spheres)
        .map(|(c, sphere)| {
            particles::ConstraintFrame {
                ground: c.ground,
                object: c.scene,
                at: vector(c.point),
                vel: ragdoll.bones.get(sphere.bone).map_or(V3::default(), |b| vector(b.body.velocity)),
                zone: c.surface,
            }
        })
        .collect()
}

/// The bike's tire tracks off the archives (V40): the `TireTrack` entries of the track's
/// `mxparticles.dat`, their zones, and their material's texture. Says what it found.
fn load_tire_tracks(archives: &[Res]) -> Option<(Vec<tire_tracks::Params>, game_render::pst::Texture)> {
    let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(&format!("\\{name}")).ok());
    let Some(list) = read(tire_tracks::LIST) else {
        println!("tire tracks: no {}", tire_tracks::LIST);
        return None;
    };
    let zones = read(tire_tracks::ZONES).map(|z| ini::text(&z));
    let list = tire_tracks::list(&ini::text(&list), zones.as_deref());
    let material = list.first()?.material.clone();
    // The tread sits at the archive's root (`bike\tex\`), so its name is its whole path.
    let name = format!("bike\\tex\\{material}.pst");
    let texture = match archives.iter().find_map(|a| a.read_suffix(&name).ok()).map(|b| game_render::pst::Texture::parse(&b).map(|t| game_render::replace::swap_file(t, &b))) {
        Some(Ok(t)) => t,
        _ => {
            println!("tire tracks: no {name}");
            return None;
        }
    };
    for p in &list {
        println!("tire tracks: {} on tire {}, {} strips, {}", p.name, p.tire, p.strips, p.material);
    }
    Some((list, texture))
}

/// A seat's bike as its tracks read it after the tick (V40): each wheel where its suspension meets
/// the ground, that ground's normal, its two contacts, its compression, its spin and its surface; the
/// chassis' speed and the wreck.
fn tire_bike(seat: &Seat) -> tire_tracks::Bike {
    let r = &seat.sim.rider;
    let tire = |w: &ride::Wheel| tire_tracks::Tire {
        at: vector(w.w.load_point),
        normal: vector(w.w.contact_normal),
        ground: w.w.ray_hit != 0,
        object: w.w.swept_contact != 0,
        compression: w.w.compression.0,
        spinning: w.slip.spinning,
        zone: w.surface,
    };
    tire_tracks::Bike { tires: vec![tire(&r.ride.front), tire(&r.ride.rear)], speed: r.ride.chassis.speed.0, wrecking: r.crash.is_some() }
}

/// A tick of every seat's tracks whose bike rode it: on its bike, not frozen, paused or waiting. A seat
/// that joined gets its own strips; one in a machine keeps the marks it left.
fn step_tire_marks(marks: &mut Vec<tire_tracks::Tracks>, list: &[tire_tracks::Params], seats: &[Seat]) {
    while marks.len() < seats.len() {
        marks.push(tire_tracks::Tracks::new(list, marks.len() as u32 + 1));
    }
    for (seat, m) in seats.iter().zip(marks.iter_mut()) {
        if seat.riding && !seat.paused && !seat.waiting && seat.garage.up == Kind::Bike {
            m.step(&tire_bike(seat));
        }
    }
}

/// The cars' tire tracks off the archives (S269): the `TireTrack` entries of each car's list
/// ([`particles::car_list`]), four a car, one an `AttachedToTire` of 0 to 3, with `mxparticlezones.dat`'s
/// zones. They are the bike's `ttrack`, drawn with its tread. Says what it found.
fn load_car_tire_tracks(archives: &[Res], cars: &[vehicle::CarRide]) -> Vec<(Machine, Vec<tire_tracks::Params>)> {
    let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(&format!("\\{name}")).ok());
    let zones = read(tire_tracks::ZONES).map(|z| ini::text(&z));
    let mut out: Vec<(Machine, Vec<tire_tracks::Params>)> = Vec::new();
    for car in cars {
        if out.iter().any(|(m, _)| *m == car.machine) {
            continue;
        }
        let Some(list) = particles::car_list(car.machine.dir()).and_then(|name| read(name)) else { continue };
        let list = tire_tracks::list(&ini::text(&list), zones.as_deref());
        let tires: Vec<String> = list.iter().map(|p| p.tire.to_string()).collect();
        println!("car: {}: tire tracks: {} on tires {}", car.machine.dir(), list.len(), tires.join(", "));
        out.push((car.machine, list));
    }
    out
}

/// A tick of the tracks of the car each seat drives (S269), as [`step_tire_marks`] steps the bike's:
/// riding, not paused or waiting. A seat's other cars' marks stay where they were left.
fn step_car_tire_marks(marks: &mut Vec<Vec<tire_tracks::Tracks>>, lists: &[(Machine, Vec<tire_tracks::Params>)], seats: &[Seat]) {
    while marks.len() < seats.len() {
        // Seeded past the bikes' (a seat's number), so no car's noise is a bike's.
        let p = marks.len() as u32;
        marks.push(lists.iter().enumerate().map(|(k, (_, list))| tire_tracks::Tracks::new(list, 0x100 + p * 8 + k as u32)).collect());
    }
    for (seat, m) in seats.iter().zip(marks.iter_mut()) {
        if !seat.riding || seat.paused || seat.waiting {
            continue;
        }
        let Some(car) = seat.garage.car_up() else { continue };
        let Some(driven) = car.driven.as_ref() else { continue };
        if let Some(k) = lists.iter().position(|(machine, _)| *machine == car.machine) {
            m[k].step(&tire_tracks::Bike::car(driven));
        }
    }
}

/// Every seat's tracks as one glow, cut from glow texture `texture`.
fn tire_mark_glow(marks: &[tire_tracks::Tracks], texture: usize) -> game_render::glow::Glow {
    let mut all = game_render::glow::Glow { texture, additive: false, tris: Vec::new() };
    for m in marks {
        all.tris.extend(m.glows(texture).tris);
    }
    all
}

/// The plane's particles as the GPU draws them.
fn plane_sprites(fx: &particles::PlaneFx, by: f32) -> Vec<game_render::gpu::Particle> {
    fx.drawn_at(by)
        .iter()
        .map(|d| game_render::gpu::Particle { at: [d.at.x, d.at.y, d.at.z], size: d.size, uv: d.uv, alpha: d.alpha, _pad: [0.0; 3] })
        .collect()
}

/// How far between its last two ticks seat `p`'s particles are drawn (P162): the picture's blend, as
/// its bike is, and the last tick where the seat's picture cuts this frame.
fn fx_blend(blend: f32, cut: &[bool], p: usize) -> f32 {
    if cut.get(p).copied().unwrap_or(true) { 1.0 } else { blend }
}

/// The plane's particles as the software path draws them (P62).
fn plane_particles(fx: &particles::PlaneFx, by: f32) -> Vec<game_render::particle::Particle> {
    fx.drawn_at(by)
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

/// This frame's portals into `r` (V51), their pictures first if they have not been uploaded yet.
fn set_glows(r: &mut Renderer, textures: &mut Option<Vec<game_render::pst::Texture>>, glows: &[Vec<game_render::glow::Glow>]) {
    if let Some(textures) = textures.take() {
        r.set_glow_textures(&textures);
    }
    r.set_glows(glows);
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
        shot_pose: None,
        more_poses: Vec::new(),
        overview: false,
        overlay: None,
        speedometer: None,
        minimap: None,
        show_fps: None,
        world_sky: None,
        hud_scale: None,
        camera: START_CAMERA.to_string(),
        camera_asked: false,
        camera_view: None,
        trace_camera: false,
        moment: None,
        wrecks: true,
        air_wrecks: true,
        backwards_bail: false,
        vibration: true,
        modifiers: game_sim::modifiers::Mods::original(),
        trace: None,
        trace_dt: None,
        trace_brake: false,
        trace_gas: false,
        trace_stick: None,
        trace_feel: false,
        seams: false,
        trace_air: false,
        phy: None,
        trace_at: None,
        fly_at: None,
        world_ground_all: false,
        world_reach: None,
        world_fine: Some(worlds::FINE_REACH),
        world_chunks: None,
        world_stand_in: true,
        warm: true,
        world_sweep: None,
        trace_switch: None,
        online_script: Vec::new(),
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
        aspect_asked: false,
        plants: None,
        foliage: None,
        textures: None,
        filtering: None,
        shadows: None,
        shadow_quality: None,
        original_wheels: false,
        headless: false,
        frames: None,
        steady_clock: false,
        sim_thread: sim_thread::Mode::Auto,
        draw_fps: None,
        vsync: None,
        fps_limit: None,
        online: None,
        peers: Vec::new(),
        seats: None,
        set: None,
        others: Vec::new(),
        look: roster::Look::default(),
        garage: false,
        pad_of: Vec::new(),
        sound: true,
        timings: false,
        trace_picture: false,
        trace_hash: false,
        display: std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string()),
        menu: false,
        ride_flag: false,
        screen: None,
        screen_tab: None,
        open_list: false,
        wiring_flags: (false, false),
        fps: false,
        renderer: Backend::default(),
        graphics: None,
        render_scale: None,
        render_scale_asked: false,
        detail: None,
        video_asked: false,
        mods: None,
        mods_asked: false,
        mod_store: None,
        tricks: None,
        handling: None,
        fliers: None,
        flier_skin: None,
        room_handling: None,
        room_tricks: None,
        trick_packs: None,
        handling_pack: None,
        brake_gyro: None,
        gas_gyro: None,
        markers: None,
        where_only: false,
    };
    // A phone's world is held nearer (P301); `--world-reach` and `--world-fine` still say otherwise.
    if let Some((reach, fine)) = menu::video::world_reach(&menu::video::Device::here()) {
        args.world_reach = Some(reach);
        args.world_fine = Some(fine);
    }
    // And its ground held lean on the GPU (P301a); `--world-lean` does it anywhere.
    if menu::video::world_lean(&menu::video::Device::here()) {
        game_render::gpu::WORLD_LEAN.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let mut argv = argv().into_iter();
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
            "--open-list" => args.open_list = true,
            "--where" => args.where_only = true,
            // P191: the updater asks nothing this run (a flag the browser's build takes and ignores)
            "--no-update-check" => {
                #[cfg(not(target_arch = "wasm32"))]
                update::NO_CHECK.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            "--screen" => {
                let w = next("a screen: main, tracks, garage, settings, controls, keyboard or pause, or a tab")?;
                match (menu::Screen::from_word(&w), menu::Screen::tab_from_word(&w)) {
                    (Some(screen), _) => args.screen = Some(screen),
                    (None, Some((screen, tab))) => (args.screen, args.screen_tab) = (Some(screen), Some(tab)),
                    (None, None) => {
                        return Err(format!(
                            "--screen: main, tracks, garage, settings, controls, keyboard or pause, or a tab \
                             (ride, display, graphics, audio, camera, original, rider, bike, gear), not {w}"
                        ));
                    }
                }
                args.menu = true;
            }
            "--track" => args.track = disc_worlds::id_of(&next("a track name, like free06")?).to_string(),
            "--mods" => {
                let v = next("a mods folder, or off")?;
                args.mods_asked = true;
                args.mods = (v != "off").then(|| std::path::PathBuf::from(v));
            }
            "--mod-store" => args.mod_store = Some(next("the mod store's address, like http://127.0.0.1:8000")?),
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
            "--overview" => args.overview = true,
            "--shot-pose" => {
                let pose = shot_pose::Pose::parse(&next("x,y,z,yaw,pitch,fov")?)?;
                match args.shot_pose {
                    None => args.shot_pose = Some(pose),
                    Some(_) => args.more_poses.push(pose),
                }
            }
            "--hud" => return Err("--hud: the debug HUD is gone (P170); the ride HUD is the default, --overlay none hides it".into()),
            "--overlay" => {
                let word = next("hud or none")?;
                if menu::Overlay::GONE.contains(&word.as_str()) {
                    return Err(format!("--overlay: {word:?} is gone (P170): there is the HUD, hud, or none"));
                }
                let overlay = menu::Overlay::from_word(&word).ok_or_else(|| format!("--overlay: {word:?} is not hud or none"))?;
                args.overlay = Some(overlay);
            }
            "--markers" => {
                let v = next("on, off, or one per seat (on,off)")?;
                args.markers = Some(menu::markers_seats(&v).ok_or_else(|| format!("--markers: {v:?} is not on or off, one per seat"))?);
            }
            "--handling" => {
                let word = next("original or mxvatv")?;
                let h = game_sim::handling::Handling::from_word(&word).ok_or_else(|| format!("--handling: {word:?} is not original, mxvatv or untamed"))?;
                args.handling = Some(h);
            }
            "--tricks" => {
                // MD77: original, mxvatv (the first pack in the folder), or the packs to stack by id
                let word = next("original, mxvatv, or trick pack ids")?;
                args.tricks = Some(menu::Tricks::from_word(&word).unwrap_or(menu::Tricks::MxVsAtv));
                args.trick_packs = Some(match menu::Tricks::from_word(&word) {
                    Some(_) => Vec::new(),
                    None => trick_packs::ids_of_room(&word),
                });
            }
            "--handling-pack" => args.handling_pack = Some(next("a trick pack id")?),
            "--fliers" => {
                let word = next("original or mxvatv")?;
                args.fliers = Some(match word.as_str() {
                    "original" => false,
                    "mxvatv" => true,
                    _ => return Err(format!("--fliers: {word:?} is not original or mxvatv")),
                });
            }
            "--flier-skin" => {
                let word = next("a skin, 1 to 4")?;
                let n = word.parse::<usize>().ok().filter(|n| (1..=4).contains(n)).ok_or_else(|| format!("--flier-skin: {word:?} is not 1, 2, 3 or 4"))?;
                args.flier_skin = Some(n - 1);
            }
            "--speedometer" => {
                let word = next("digital or analog")?;
                let style = menu::Speedometer::from_word(&word)
                    .ok_or_else(|| format!("--speedometer: {word:?} is not digital or analog"))?;
                args.speedometer = Some(style);
            }
            "--world-sky" => {
                let word = next("global or per-map")?;
                args.world_sky = Some(menu::WorldSky::from_word(&word).ok_or_else(|| format!("--world-sky: {word:?} is not global or per-map"))?);
            }
            flag @ ("--brake-gyro" | "--gas-gyro") => {
                let on = match next("on or off")?.as_str() {
                    "on" => true,
                    "off" => false,
                    word => return Err(format!("{flag}: {word:?} is not on or off")),
                };
                match flag {
                    "--brake-gyro" => args.brake_gyro = Some(on),
                    _ => args.gas_gyro = Some(on),
                }
            }
            "--minimap" => {
                args.minimap = Some(match next("on or off")?.as_str() {
                    "on" => true,
                    "off" => false,
                    word => return Err(format!("--minimap: {word:?} is not on or off")),
                });
            }
            "--show-fps" => {
                args.show_fps = Some(match next("on or off")?.as_str() {
                    "on" => true,
                    "off" => false,
                    word => return Err(format!("--show-fps: {word:?} is not on or off")),
                });
            }
            "--hud-scale" => {
                let word = next("a HUD scale from 50 to 150 in tens")?;
                let n = menu::hud_scale_word(&word).ok_or_else(|| format!("--hud-scale: {word:?} is not 50 to 150 in steps of 10"))?;
                args.hud_scale = Some(n);
            }
            "--fps" => args.fps = true,
            "--graphics" => {
                let word = next("original, far, farther or full")?;
                let graphics = Graphics::from_word(&word)
                    .ok_or_else(|| format!("--graphics: {word:?} is not original, x2, x3, far, x6, farther, x12, x16 or full"))?;
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
            "--camera" => {
                args.camera = next("a camera preset's name, like \"Freestyle Cam\"")?;
                args.camera_asked = true;
            }
            "--camera-view" => {
                let v = next("original, default, or five numbers: fov,distance,height,plane distance,plane height")?;
                args.camera_view = Some(match v.as_str() {
                    "original" => menu::CameraView::ORIGINAL,
                    "default" => menu::CameraView::DEFAULT,
                    _ => menu::CameraView::parse(&v.replace(',', " "))
                        .ok_or_else(|| format!("--camera-view: original, default or five numbers like 4,115,110,120,115, not {v:?}"))?,
                });
            }
            "--moment" => {
                let file: std::path::PathBuf = next("a moment file")?.into();
                let out: std::path::PathBuf = next("a path for the frame")?.into();
                args.moment = Some((file, out));
            }
            "--display" => args.display = next("a display, like :0")?,
            "--no-wrecks" => args.wrecks = false,
            "--no-air-wrecks" => args.air_wrecks = false,
            "--backwards-bail" => args.backwards_bail = true,
            "--no-vibration" => args.vibration = false,
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
                // L2 is no throttle since P81 (user, 2026-10-22): it is the new layout's brake.
                args.wiring.throttle = match next("cross or r2")?.as_str() {
                    "cross" => Throttle::Cross,
                    "r2" => Throttle::Trigger(Control::R2),
                    other => return Err(format!("--throttle: cross or r2, not {other}")),
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
            "--seams" => args.seams = true,
            "--trace-camera" => args.trace_camera = true,
            "--trace-air" => args.trace_air = true,
            "--online-script" => {
                let bad = || "--online-script: FRAME:WORD,... (a machine, gas or lift)".to_string();
                for pair in next("frame:word pairs")?.split(',') {
                    let (f, word) = pair.split_once(':').ok_or_else(bad)?;
                    let word = word.trim();
                    if word != "gas" && word != "lift" && Kind::parse(word).is_err() {
                        return Err(bad());
                    }
                    args.online_script.push((f.trim().parse().map_err(|_| bad())?, word.to_string()));
                }
                args.online_script.sort_by_key(|&(f, _)| f);
            }
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
            "--fly-at" => {
                let v: Vec<f32> = next("x,z,degrees,feet")?
                    .split(',')
                    .map(|w| w.trim().parse())
                    .collect::<Result<_, _>>()
                    .map_err(|_| "--fly-at: x,z,degrees,feet")?;
                let [x, z, deg, ft] = v[..] else { return Err("--fly-at: x,z,degrees,feet".into()) };
                args.fly_at = Some([x, z, deg, ft]);
            }
            "--vehicle" => args.vehicle = Kind::parse(&next("a vehicle, bike or biplane")?)?,
            "--players" => {
                let n = next("a number of players, 1 to 8")?;
                args.players = n.parse().map_err(|_| format!("--players: {n:?} is not a number"))?;
                Layout::for_players(args.players, args.split)?;
            }
            "--split" => args.split = Split::parse(&next("h or v")?)?,
            "--aspect" => (args.aspect, args.aspect_asked) = (Aspect::parse(&next("auto, 4:3, 16:9, 16:10, 21:9 or stretch")?)?, true),
            "--wheels" => match next("blurred or original")?.as_str() {
                "original" => args.original_wheels = true,
                "blurred" => args.original_wheels = false,
                word => return Err(format!("--wheels: {word:?} is not blurred or original")),
            },
            "--plants" => {
                let word = next("smooth or original")?;
                args.plants = Some(game_render::eco::Lod::from_word(&word).ok_or_else(|| format!("--plants: {word:?} is not smooth or original"))?);
            }
            "--textures" => {
                args.textures = match next("upscaled or original")?.as_str() {
                    "upscaled" => Some(true),
                    "original" => Some(false),
                    word => return Err(format!("--textures: {word:?} is not upscaled or original")),
                };
            }
            "--foliage" => {
                let word = next("cutout or original")?;
                args.foliage = Some(game_render::eco::Foliage::from_word(&word).ok_or_else(|| format!("--foliage: {word:?} is not cutout or original"))?);
            }
            "--texture-filtering" => {
                let word = next("off, 2, 4, 8 or 16")?;
                args.filtering = Some(menu::Filtering::from_word(&word).ok_or_else(|| format!("--texture-filtering: {word:?} is not off, 2, 4, 8 or 16"))?);
            }

            "--detail" => {
                let word = next("full or reduced")?;
                args.detail = Some(menu::video::Detail::from_word(&word).ok_or_else(|| format!("--detail: {word:?} is not full or reduced"))?);
            }
            // P223: every row a preset sets, as the Graphics tab's Preset row sets them; a row's own
            // flag given after it wins.
            "--video-preset" => {
                let word = next("high, medium, low, deck or phone")?;
                let p = menu::video::Preset::ALL
                    .into_iter()
                    .find(|p| p.label().split_whitespace().last().is_some_and(|w| w.eq_ignore_ascii_case(&word)))
                    .ok_or_else(|| format!("--video-preset: {word:?} is not high, medium, low, deck or phone"))?;
                let l = p.look();
                args.graphics = Some(l.draw);
                args.filtering = Some(l.filtering);
                args.shadows = Some(l.shadows);
                args.shadow_quality = Some(l.shadow_quality);
                args.detail = Some(l.detail);
                (args.render_scale, args.render_scale_asked) = (l.scale.fraction(), true);
                args.fps_limit = Some(l.limit);
            }
            "--shadows" => {
                let word = next("off, on or all")?;
                let (shadows, quality) = game_render::shadow::Shadows::from_old_word(&word).ok_or_else(|| format!("--shadows: {word:?} is not off, on or all"))?;
                args.shadows = Some(shadows);
                args.shadow_quality = quality.or(args.shadow_quality);
            }
            "--shadow-quality" => {
                let word = next("original, low or high")?;
                args.shadow_quality = Some(game_render::shadow::ShadowQuality::from_word(&word).ok_or_else(|| format!("--shadow-quality: {word:?} is not original, low or high"))?);
            }
            "--headless" => args.headless = true,
            "--no-sound" => args.sound = false,
            "--timings" => args.timings = true,
            "--world-ground" => {
                args.world_ground_all = match next("all or near")?.as_str() {
                    "all" => true,
                    "near" => false,
                    w => return Err(format!("--world-ground: all or near, not {w}")),
                }
            }
            "--world-stand-in" => {
                args.world_stand_in = match next("on or off")?.as_str() {
                    "on" => true,
                    "off" => false,
                    w => return Err(format!("--world-stand-in: on or off, not {w}")),
                }
            }
            "--warm" => {
                args.warm = match next("on or off")?.as_str() {
                    "on" => true,
                    "off" => false,
                    w => return Err(format!("--warm: on or off, not {w}")),
                }
            }
            "--world-chunks" => {
                args.world_chunks = Some(next("chunks")?.parse().ok().filter(|&n| n > 0).ok_or("--world-chunks: a count over 0")?);
            }
            "--world-sweep" => {
                let v: Vec<f32> = next("x0,z0,x1,z1")?
                    .split(',')
                    .map(|w| w.trim().parse())
                    .collect::<Result<_, _>>()
                    .map_err(|_| "--world-sweep: x0,z0,x1,z1 in world feet")?;
                let [x0, z0, x1, z1] = v[..] else { return Err("--world-sweep: x0,z0,x1,z1 in world feet".into()) };
                args.world_sweep = Some([x0, z0, x1, z1]);
            }
            "--world-reach" => {
                let v: Vec<f32> =
                    next("load,drop")?.split(',').map(|w| w.trim().parse()).collect::<Result<_, _>>().map_err(|_| "--world-reach: load,drop in feet")?;
                let [load, drop] = v[..] else { return Err("--world-reach: load,drop in feet".into()) };
                if !(load > 0.0 && drop >= load) {
                    return Err("--world-reach: 0 < load <= drop".into());
                }
                args.world_reach = Some(game_sim::tiles::Reach { load: F(load), drop: F(drop) });
            }
            "--world-lean" => game_render::gpu::WORLD_LEAN.store(true, std::sync::atomic::Ordering::Relaxed),
            "--world-fine" => {
                let w = next("load,drop or off")?;
                args.world_fine = if w == "off" {
                    None
                } else {
                    let v: Vec<f32> = w.split(',').map(|w| w.trim().parse()).collect::<Result<_, _>>().map_err(|_| "--world-fine: load,drop in feet, or off")?;
                    let [load, drop] = v[..] else { return Err("--world-fine: load,drop in feet, or off".into()) };
                    if !(load > 0.0 && drop >= load) {
                        return Err("--world-fine: 0 < load <= drop".into());
                    }
                    Some(game_sim::tiles::Reach { load: F(load), drop: F(drop) })
                };
            }
            "--trace-picture" => args.trace_picture = true,
            "--trace-hash" => args.trace_hash = true,
            "--frames" => {
                args.frames = Some(
                    next("a number of frames")?.parse().map_err(|_| "--frames: not a number of frames")?,
                )
            }
            "--steady-clock" => args.steady_clock = true,
            "--sim-thread" => {
                args.sim_thread = sim_thread::Mode::parse(&next("auto, on or off")?).ok_or("--sim-thread: auto, on or off")?
            }
            "--draw-fps" => {
                let fps: f32 = next("pictures a second")?.parse().map_err(|_| "--draw-fps: not a number")?;
                if !(fps > 0.0) {
                    return Err("--draw-fps: more than none a second".into());
                }
                args.draw_fps = Some(fps);
            }
            "--vsync" => {
                args.vsync = Some(match next("on or off")?.as_str() {
                    "on" => true,
                    "off" => false,
                    w => return Err(format!("--vsync: on or off, not {w:?}")),
                })
            }
            "--fps-limit" => {
                let w = next("off, a number of pictures a second, or refresh")?;
                args.fps_limit = Some(
                    menu::FrameLimit::from_word(&w).ok_or_else(|| format!("--fps-limit: off, 1 to 1000, or refresh, not {w:?}"))?,
                );
            }
            "--online" => args.online = Some(next("this machine, like host@0.0.0.0:7000")?),
            "--peer" => args.peers.push(next("another machine, like g@192.168.1.20:7000")?),
            "--seats" => args.seats = Some(next("who sits where, like host,g,g")?),
            "--phy" => args.phy = Some(next("a .phy on the disc, like race125p.phy")?),
            "--set" => {
                let w = next("one of the bike sets, like race250p")?;
                // The player rides P: an A word (the AI riders' files) rides its P twin (Set::ridden).
                args.set = Some(roster::Set::from_player_word(&w).ok_or(format!("--set: like race125p or pro500p, not {w}"))?);
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
            "--version" | "-V" => {
                println!("{}", menu::VERSION_LINE);
                return Ok(None);
            }
            "--help" | "-h" => {
                println!(
                    "openmx [DISC.iso] [--where] [--no-update-check] [--version]\n\
                     \x20      (the disc: this, else kit.toml's above the working directory, else the one\n\
                     \x20      remembered in the data folder, else a picker asks; --where says which, and\n\
                     \x20      where settings.txt, controls.txt and mods/ are; --no-update-check: the main\n\
                     \x20      menu doesn't ask GitHub for a newer build this run)\n\
                     openmx [--menu [--screen main|tracks|settings|controls|keyboard|pause|TAB [--open-list]]]\n\
                            [--ride] [--track free06] [--bike 125sx] [--rider thq1] [--size 640x448]\n\
                            [--scale 1]\n\
                            [--pad /dev/input/js0] [--display :0] [--seconds N] [--shot PATH]\n\
                            [--moment FILE OUT.png] [--no-wrecks] [--no-air-wrecks] [--backwards-bail] [--no-vibration] [--overlay hud|none] [--speedometer digital|analog] [--tricks original|mxvatv|ID,ID…] [--handling original|mxvatv|untamed] [--handling-pack ID] [--fliers original|mxvatv] [--flier-skin 1-4] [--brake-gyro on|off] [--gas-gyro on|off] [--markers on|off] [--camera NAME]\n\
                            [--minimap on|off] [--show-fps on|off] [--hud-scale 50..150] [--world-sky global|per-map]\n\
                            [--camera-view original|default|FOV,DIST,HEIGHT,PDIST,PHEIGHT]\n\
                            [--modifiers 'gravity=50 bike_speed.2=200']\n\
                            [--shot-frame N] [--shot-pose x,y,z,yaw,pitch,fov]... [--overview] [--vehicle bike|biplane]\n\
                            [--players 1..8] [--split h|v] [--headless] [--frames N] [--timings]\n\
                            [--world-ground all|near] [--world-reach LOAD,DROP] [--world-fine LOAD,DROP|off] [--world-lean] [--world-chunks N]\n\
                            [--world-sweep X0,Z0,X1,Z1] [--world-stand-in on|off] [--warm on|off]\n\
                            [--draw-fps N] [--steady-clock] [--sim-thread auto|on|off] [--vsync on|off] [--fps-limit off|N|refresh]\n\
                            [--record TAPE] [--play TAPE|RECORDING.p2m2 [--phase second]]\n\
                            [--throttle cross|r2] [--pad-pressure ds2@8|AXIS:CONTROL,...]\n\
                            [--pads k,0,1,-] [--controls new|original|new,original,...]\n\
                            [--biplane-controls new|screen|original|new,original,...]\n\
                            [--preset 0..3|0,2,...]\n\
                            [--bindings FILE] [--renderer gpu|software] [--graphics original|x2|x3|far|x6|farther|x12|x16|full] [--fps]\n\
                            [--render-scale auto|0.5..1]\n\
                            [--video-preset high|medium|low|deck|phone] [--shadows off|on|all]\n\
                            [--shadow-quality original|low|high] [--detail full|reduced]\n\
                            [--no-sound] [--mods DIR|off] [--mod-store URL]\n\
                     \n\
                     Opens the front end, or with --track (or --ride, --play, --record, --trace...)\n\
                     rides the freeride area from the disc named in kit.toml straight away.\n\
                     The front end keeps settings.txt and controls.txt beside the disc.\n\
                     R2 throttle, L2 brake, each on its trigger's travel, Select changes camera;\n\
                     Cross and Square are kept free. --controls original (or the front end's\n\
                     Controls screen, per seat) rides the original's instead: Cross throttle,\n\
                     Square brake, R2 front brake, L2 clutch, L1 changes camera. Either way the\n\
                     left stick steers\n\
                     and leans, the d-pad preloads, the right stick turns the camera (L3 held\n\
                     zooms), R3 puts it back, the gas and the brake held together under 5 ft/s\n\
                     back the bike up, Start freezes, Escape quits. --camera starts on a camera preset by its\n\
                     name in the track's cpresets.ini (\"Race Cam 1\" unless asked).\n\
                     --camera-view frames every player's cameras (the Camera page's, P82):\n\
                     original is the disc's numbers exactly; five numbers are degrees added to\n\
                     the field of view and the bike's and the biplane's distance and height in\n\
                     percent of the original's. Unset, the settings' or the shipped default.\n\
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
                     On the keyboard, under the new layout: W gas, S brake, A/D steer (J/L too),\n\
                     I/K lean, Shift or Q clutch (L1), F G E the three tricks (Circle, Triangle,\n\
                     R1), Space and V Cross and Square (MX vs. ATV's tricks 4 and 5), R reset,\n\
                     C or Tab the camera, B and Z R3 and L3, 7890 the right stick, arrows the\n\
                     d-pad, Enter Start, Escape the pause; keys = home-row in controls.txt (or\n\
                     the Controls screen) brings back the home row's J gas, K brake, WASD and\n\
                     YUIO the sticks, H Square, L Circle, G Cross, ; Triangle. Under the original's (--controls original): X throttle, Z brake,\n\
                     arrows steer, IJKL the right stick, C/V/D trick, A+D reset.\n\
                     The throttle is a pressure and not a switch (the game reads Cross from its\n\
                     pressure byte): a key ramps it, --throttle r2 rides it on a trigger instead,\n\
                     giving up that trigger's own control, and --pad-pressure says which joystick\n\
                     axes a pad reports its buttons' own pressures on.\n\
                     Up to eight seats: every pad plugged in takes the next one, the keyboard\n\
                     beside 1P's, or --pads says each seat's (k the keyboard, a number the pad\n\
                     in the order found, - nobody). A pad pulled out pauses its player until it\n\
                     is plugged back in; Start on the keyboard rides on without it. The buttons\n\
                     are rebindable in --bindings (context/controls.txt, written on first run).\n\
                     --vehicle biplane starts in the biplane, on any map: in the track's parked\n\
                     one where its machine.txt parks one, by the start line otherwise; M switches between the bike and the plane, spawning the\n\
                     one taken where you are and putting the other away. The plane flies over the terrain\n\
                     alone: Cross opens the throttle and Square closes it.\n\
                     --players N splits the screen between N local players (up to 8), each on\n\
                     a pad of their own with a camera and an overlay of their own: two one above\n\
                     the other (--split h) or side by side (--split v, the default), the\n\
                     original's two layouts, and three to eight in quarters or eighths. The first\n\
                     player has the keyboard, the plane and the tape. --headless draws with no\n\
                     window and no pad, and --frames N quits after N frames. --timings prints\n\
                     what each stage of a frame cost, a line a second, and --trace-picture\n\
                     where each drawn frame put the first player's bike, rider and camera.\n\
                     --trace-hash prints every seat's state hash after each tick.\n\
                     The ride runs 30 ticks a second, as the original does, however fast it is\n\
                     drawn: a window draws as fast as its display takes pictures, a headless\n\
                     ride 60 a second, and --draw-fps N at most N. --steady-clock times the ride\n\
                     by its pictures alone, each one exactly its floor, so a busy machine rides\n\
                     the same ride as an idle one (offline; for the tests). --sim-thread on\n\
                     runs each tick on a thread of its own while the picture before it is drawn\n\
                     (offline, not in a tiled world), the same ride; auto does when a window's\n\
                     split screen cannot fit a tick and a picture in a picture's time.\n\
                     The window opens at the screen's size and the frame fills it, re-made when\n\
                     the window is resized, wider or taller as the original's own 16:9 mode\n\
                     would draw it; --aspect 4:3, 16:9, 16:10 or 21:9 draws the largest frame of\n\
                     that shape inside it instead, with bars, the HUD, menus and split screens\n\
                     too (auto and stretch fill the window). --size draws a fixed frame (640x448 is the PS2's), and --scale\n\
                     draws each frame pixel N times across and down. --render-scale draws the\n\
                     3D at a fraction of the frame and stretches it over, the HUD and menus\n\
                     still sharp; auto, the default, lowers it while the GPU misses the\n\
                     display's refresh and raises it again when there is room. Without it,\n\
                     the front end's Settings row (Auto, 100%, 75%, 50%) says.\n\
                     --video-preset sets every graphics row as the Graphics tab's Preset row\n\
                     does; --shadows on casts the movers' shadows alone, all the props' and\n\
                     trees' too (baked once), and --shadow-quality original draws the game's\n\
                     patch under each rider, low and high our maps; --detail reduced swaps\n\
                     models to their coarser levels nearer.\n\
                     Any graphics flag keeps the Auto preset from stepping the rows down.\n\
                     --vsync off draws without waiting for the display (it may tear), and\n\
                     --fps-limit draws at most N pictures a second, or the display's refresh;\n\
                     a limit under the refresh holds with VSync on too. Without them the\n\
                     Settings rows say (VSync On, Frame limit Off). The ride's 30 Hz ticks are\n\
                     the same at any of them; --draw-fps, a test's fixed rate, wins over both.\n\
                     --no-wrecks turns the session's wrecks off (a wreck otherwise throws the\n\
                     rider, tumbles the bike and puts you back on it); --no-air-wrecks keeps\n\
                     the rider on whatever the bike's attitude in the air (a landing is still\n\
                     judged); --backwards-bail throws the rider off a landing\n\
                     with the bike facing back against its way, which the original rides\n\
                     out (off by default); --no-vibration keeps the pad still\n\
                     when its rider wrecks; --modifiers sets the\n\
                     game modifiers, percentages of what the ride loads (a tape keeps its own);\n\
                     --moment draws one frame posed\n\
                     from a moment of the original and quits; --trace N rides N frames with\n\
                     nobody on the pad and prints the solver's own numbers, and --trace-brake\n\
                     or --trace-gas hold the brakes or the throttle while it does, --trace-stick X\n\
                     holds the stick over and --trace-feel prints a short line a frame instead;\n\
                     --trace-air prints the chassis' attitude and its rates instead;\n\
                     --trace-switch N rides the throttle alone for N frames first;\n\
                     --online-script F:WORD,... online, picks a machine (bike, biplane, buggy,\n\
                     monster, baja) at session tick F, or holds Cross (gas) or lets it go (lift);\n\
                     --trace-stick-y F:Y,... sets the stick's y from frame F on, each until\n\
                     the next, and adds the backflip's columns to --trace-feel's line;\n\
                     --trace-camera prints each view's camera a frame (preset, distance, pitch,\n\
                     field of view, eye and look-at) beside a ride;\n\
                     --fly-at X,Z,DEG,FT starts the plane flying FT feet over the ground there;\n\
                     --trace-at X,Z,DEG starts the bike there instead of the start line, and\n\
                     --phy NAME rides the bike archive's NAME (pro125a.phy) instead of Race125P.phy,\n\
                     --set WORD the bike's own file of that set (race250p, pro500p) on its class,\n\
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
                     then the music packs in mods/music/<id>/ (Settings, Audio: Music From),\n\
                     picked in Settings. --no-sound turns it off; a headless ride has none.\n\
                     \n\
                     Track mods: a --track not on the disc is looked for in mods/tracks/<id>/\n\
                     beside the settings (openmx-mods' standard), or in --mods DIR; --mods off,\n\
                     or Settings' Mods Off, reads nothing there. A mod rides as a freeride.\n\
                     Mods' Browse the Store installs from the openmx-mods store on Hugging\n\
                     Face (P198), or from --mod-store URL's.\n\
                     \n\
                     Textures (V84): --dump-textures [OUT] [--from DIR] writes every disc\n\
                     texture (or a mod folder's) as <name>-<key>.png to mods/textures-src/;\n\
                     key-named PNGs in mods/texture-replacements/, texture packs and mods'\n\
                     textures/upscaled/ are drawn in their place. --textures original draws\n\
                     the disc's alone (Settings' Upscaled Textures).\n\
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
    args.video_asked = args.video_flags();
    if args.world_sweep.is_some() && args.frames.is_none() {
        return Err("--world-sweep goes over the ride's --frames: give --frames too".into());
    }
    Ok(Some(args))
}

#[cfg(test)]
mod opens_on_mod {
    //! F6: a freeride on a track mod opens to online when the lobby's mods folder finds the same
    //! folder the ride was built from; a world mod's ride, or a mod the lobby's folder lacks, does not.
    use super::{opens_on_mod, track_mods};
    use std::path::{Path, PathBuf};

    /// A `mods/` folder holding the track mod `f6_flat`: a `mod.json` and a scene, all `find` reads.
    fn folder(name: &str) -> PathBuf {
        let mods = std::env::temp_dir().join(format!("openmx-f6-{}-{name}", std::process::id())).join("mods");
        let _ = std::fs::remove_dir_all(&mods);
        let dir = mods.join("tracks/f6_flat");
        std::fs::create_dir_all(dir.join("Env/F6FLAT")).unwrap();
        let json = r#"{"standard": 1, "kind": "track", "id": "f6_flat", "name": "F6 Flat",
            "source": {"game": "none", "data": "written by openmx's F6 test", "date": "2026-11-04"},
            "version": "1.0.0", "author": "crates/openmx/src/main.rs", "permission": "public-domain", "todo": []}"#;
        std::fs::write(dir.join("mod.json"), json).unwrap();
        std::fs::write(dir.join("Env/F6FLAT/F6FLAT.scn"), b"").unwrap();
        mods
    }

    #[test]
    fn a_track_mod_opens_where_the_lobby_finds_it() {
        let (mine, other) = (folder("mine"), folder("other"));
        let ride = track_mods::find(&mine, "f6_flat").unwrap();
        // A disc track (no mod) opens, whatever the Mods row says; a disc world too (P232).
        assert!(opens_on_mod(None, false, None, "free06"));
        assert!(opens_on_mod(None, true, Some(&mine), "free06"));
        // The mod, with the lobby's folder the ride's own.
        assert!(opens_on_mod(Some(&ride), false, Some(&mine), "f6_flat"));
        // A world mod's home tile does not; nor does a mod with the Mods row off, another folder's
        // copy (`--mods DIR`), or a folder without it.
        assert!(!opens_on_mod(Some(&ride), true, Some(&mine), "f6_flat"));
        assert!(!opens_on_mod(Some(&ride), false, None, "f6_flat"));
        assert!(!opens_on_mod(Some(&ride), false, Some(&other), "f6_flat"));
        assert!(!opens_on_mod(Some(&ride), false, Some(Path::new("/nonexistent/mods")), "f6_flat"));
        for d in [mine, other] {
            let _ = std::fs::remove_dir_all(d.parent().unwrap());
        }
    }
}
