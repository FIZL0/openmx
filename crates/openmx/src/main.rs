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
//! clutch and R2 the front brake on its own, and the left stick steers and leans. The right stick
//! turns the camera round the rider, the original's look-around, and with L3 held moves it in and out
//! (P114); clicking it (R3) puts the camera back where its preset has it, which is the port's own —
//! the original binds R3 to nothing it reads. Reverse is the original's: gas and brake together at a
//! standstill.
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
//! Over each view is the ride HUD ([`ride_hud`], P121): the original's pieces at the original's
//! places — score, trick pop-up, speedometer, radar with every other rider on it — redrawn as vector
//! shapes and our own type, sharp at any size; `--overlay original` draws the original's own HUD
//! ([`overlay`], V26) off the disc's art instead. **H** swaps it for the debug overlay ([`hud`]), which
//! `--hud` (`--overlay debug`) starts on: the sixteen controls as the port's own input manager holds
//! them, both sticks where its dead zone put them, and beside them the numbers the ride is running
//! on. That one is debugging furniture and matches nothing on the disc. `--overlay none`, or the
//! front end's setting, draws neither until H. The original's speed is its digital readout unless
//! `--speedometer analog` or the Settings' Speedometer row asks for its dial (V42). `--minimap off` or the
//! Settings' Minimap row drops its radar, and `--hud-scale` or the HUD scale row sizes every part of
//! both HUDs about its own corner (V52).
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

#![forbid(unsafe_code)]

mod audio;
mod aspect;
mod chase;
// The riders' names and numbers on their jerseys and plates (P86).
mod dress;
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
mod portal;
mod ride_hud;
// N8: the one-seat online ride against the offline one, tick for tick.
#[cfg(test)]
mod same_ride;
mod saved_markers;
// MD10: `--shot-pose`, the first view's camera held where the accuracy harness asks.
mod shot_pose;
mod split;
mod stages;
mod track_mods;
mod track_previews;
mod trick_packs;
mod worlds;
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
    /// Whether this pass's marker Up put the bike somewhere else (P112), which the camera rides out
    /// of at the bike's speed rather than flying in from where it was.
    teleported: bool,
}

impl Seat {
    /// One player's pass: the pad, then the bike, then the d-pad's markers (P88) — `me` is this
    /// seat and `spots` where every seat's rider was before the pass.
    fn pass(&mut self, dt: F, place: &mut dyn sim_world::World, me: usize, spots: &[Spot]) {
        // Nobody's hands on the bars: the pad is not polled either, so the input manager's timers
        // and held buttons are where the pull left them, and the ride goes on from there when the
        // pad comes back.
        if self.waiting {
            self.said = rider::Frame::default();
            self.teleported = false;
            return;
        }
        // The game's poll, with the frame's dt: this is what turns report bytes into held controls,
        // press and release edges, and stick axes with the game's own dead zone — and then the
        // rider's hands off it, because which button means what lives in the ride and not here.
        // The d-pad is the markers' and not the ride's (P93): the stick alone steers.
        let controls = self.sim.poll(dt, &self.reports).stick_alone();

        if self.start_freezes && self.sim.input.pad(0).pressed(Control::Start) {
            self.riding = !self.riding;
        }

        self.said = rider::Frame::default();
        self.teleported = false;
        if self.riding {
            let did;
            (self.said, did) = self.garage.pass(&mut self.sim, dt, &controls, me, spots, place);
            if let Some(did) = did {
                self.teleported = matches!(did, markers::Did::Teleported(_));
                println!("marker: {}", marker_line(did));
            }
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
/// the label, the other players in the ride by seat (P94), and how far a Down or Up hold is (P106).
type MarkerShown = (markers::Pick, [bool; markers::SLOTS], String, Vec<usize>, u32);

/// A marker's tick (P88) as the console says it.
/// Whether the session markers' bar, its hold bar and its label are drawn over a view (P117): with
/// any overlay but `none`, or under `none` once F3 has put the debug HUD up, which they are part of.
/// The markers work all the same, and their portals are the world's, drawn whatever this says.
fn marker_bar_shown(overlay: menu::Overlay, debug_hud: bool) -> bool {
    overlay != menu::Overlay::None || debug_hud
}

fn marker_line(did: markers::Did) -> String {
    use markers::{Did, Pick};
    let what = |p: Pick| match p {
        Pick::Slot(s) => format!("slot {}", s + 1),
        Pick::Player(p) => format!("P{}", p + 1),
    };
    match did {
        Did::Stepped(p) => what(p),
        Did::Placed(s) => format!("placed in slot {}", s + 1),
        Did::Teleported(p) => format!("to {}", what(p)),
        Did::Refused(p) => format!("nothing on {}", what(p)),
    }
}

/// The switch (P19, P102) for one player, from the M key, the pause's "Switch To Machine" (P41) or
/// a tape: `to` is spawned where the player is ([`Garage::switch`]) over `place`, the machine left
/// goes, and the machine taken gets its own camera, which starts where it is rather than lagging in
/// from wherever it was last. The machine now up, or `None` (and no switch) where it cannot be had.
fn switch_machine(seat: &mut Seat, rig: &mut Rig, to: Kind, place: &mut dyn sim_world::World) -> Option<Kind> {
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
        rig.hud.note_camera(&name);
        rig.ride_hud.note_camera(Instant::now(), &name);
    }
    rig.hud.note_vehicle(up.name());
    println!("vehicle: {}", up.name());
}

/// What the original's HUD shows of a player's own ride (V26, V28): their odometer, which machine is
/// up, how hard a preload is shoving the bike, and the score. The port keeps no stunt score yet, so
/// the score reads 0 — what the original's shows until a trick lands.
fn gauges(seat: &Seat, rig: &Rig, speedometer: menu::Speedometer) -> overlay::Gauges {
    overlay::Gauges {
        feet: rig.odometer.feet,
        bike: seat.garage.up == Kind::Bike,
        preload: rig.preload_meter.shown(),
        score: 0,
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
    speedometer: menu::Speedometer,
    names: &'a [Option<String>],
    table: &'a Stunts,
    simulated: bool,
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
        let score = &seat.sim.rider.score;
        rig.ride_hud.update(now, score, |id| self.table.display_name(0, game_sim::trick::Trick(id)).to_string());
        // A split is digital, as the original's code forces it with two riders or more.
        let speedometer = if self.ridden.len() > 1 { menu::Speedometer::Digital } else { self.speedometer };
        let gauges = overlay::Gauges { score: score.total.0.max(0.0) as u32, ..gauges(seat, rig, speedometer) };
        let riders: Vec<ride_hud::Dot> = self
            .ridden
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != v)
            .map(|(_, r)| ride_hud::Dot { at: [r.target.body_point.x(), r.target.body_point.z()], ai: false })
            .collect();
        let markers = marker.map(|(pick, filled, _, others, holding)| ride_hud::Panel {
            pick: *pick,
            filled: *filled,
            others: others.iter().map(|&s| (s, self.names.get(s).cloned().flatten())).collect(),
            holding: *holding,
            machine: seat.garage.up.name().to_string(),
        });
        let shown = ride_hud::Shown {
            speed: me.speed.0,
            gauges: &gauges,
            own: [b.x(), b.z()],
            forward: [me.target.forward.x(), me.target.forward.z()],
            riders: &riders,
            blips: self.art.radar.get(v).map_or(&[], Vec::as_slice),
            span: self.span,
            tag: self.art.tags.get(v).map(overlay::Tag::text).filter(|t| !t.is_empty()),
            offer: self.art.offers.get(v).copied().flatten(),
            challenge: None,
            markers,
            scale: self.art.scale,
            minimap: self.art.minimap,
        };
        draw(&shown, &rig.ride_hud, now);
    }
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
    /// The camera the screen switches to while this player's rider is down (V31).
    wreck_camera: chase::WreckCamera,
    /// How this player's two follow cameras' presets are framed now (P82), the bike's and the
    /// plane's: the original's as they were read, until the frame lays the player's own over them.
    framing: (chase::Framing, chase::Framing),
    /// The two follow cameras' fields of view easing over a framing change (P119), the bike's and the
    /// plane's.
    fov_ease: (chase::FovEase, chase::FovEase),
    hud: Hud,
    /// The distance this player's odometer shows, under the original's overlay (V26).
    odometer: overlay::Odometer,
    /// This player's preload meter, a tick at a time (V53).
    preload_meter: overlay::PreloadMeter,
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
}

/// The spots a seat's [`pace::Cut`] measures a tick's move by: where its machine is — the plane
/// while it is flown, the bike (or its wreck) otherwise — and where its camera is and looks.
fn cut_spots(seat: &Seat, bike: M4, camera: Option<&Camera>) -> [V3; 3] {
    let machine = match seat.garage.plane_flown() {
        Some(plane) => vector(plane.at),
        None => V3 { x: bike.0[3][0], y: bike.0[3][1], z: bike.0[3][2] },
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
        let spots: Vec<Spot> = self.seats.iter().map(|s| s.garage.spot(&s.sim, &mut self.place)).collect();
        for (me, seat) in self.seats.iter_mut().enumerate() {
            seat.pass(dt, &mut self.place, me, &spots);
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
    /// `--shot-pose` (MD10): the first view drawn from this pose, not the ride camera, all the run.
    shot_pose: Option<shot_pose::Pose>,
    /// Draw the riding overlay (P6): the pad as the simulation reads it and the numbers beside it.
    /// Off by default and off is free, because it is debugging furniture and not part of the game.
    hud: bool,
    /// `--overlay hud|original|debug|none` (V26, P121): which overlay a ride draws, the ride HUD by
    /// default. Unset, the front end's setting decides; `--hud` is `--overlay debug`.
    overlay: Option<menu::Overlay>,
    /// `--speedometer digital|analog` (V42): how the original's HUD shows the speed. Unset, the
    /// front end's setting decides, and without one the digital readout.
    speedometer: Option<menu::Speedometer>,
    /// `--minimap on|off` (V52): whether the original's HUD draws its radar. Unset, the front end's
    /// setting decides, and without one it does.
    minimap: Option<bool>,
    /// `--hud-scale 50..150` (V52): the HUD's size in percent of the original's, in the Settings
    /// row's steps of ten. Unset, the front end's setting decides, and without one 100.
    hud_scale: Option<u8>,
    /// `--camera NAME`: which of the track's camera presets to start on, by the name its
    /// `cpresets.ini` gives it. The default is the one the original itself is on in a freeride (P14).
    camera: String,
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
    /// Whether `--aspect` was given: if so it wins over the settings' Picture Shape (P126).
    aspect_asked: bool,
    /// `--plants smooth|original` (P126): how the plants change from model to picture, V55's smooth
    /// way or the original's; the front end's setting when not given.
    plants: Option<game_render::eco::Lod>,
    /// `--headless`: no window and no host pad — every seat rides an idle pad, or the first rides
    /// `--play`'s tape — and the frame is drawn all the same, for `--shot` and for the tests.
    headless: bool,
    /// `--frames N`: quit after N frames, which unlike `--seconds` is the same run on any machine.
    frames: Option<u32>,
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
    /// `--graphics original|full` (V32): the original's fog and far clip, or the whole map with a
    /// haze; the front end's setting when not given. Only the GPU draws `full`.
    graphics: Option<Graphics>,
    /// `--render-scale auto|F`: the fraction of the frame the GPU draws the 3D at (V30); `None` is
    /// auto, picked from the frames' time in a window and whole offscreen.
    render_scale: Option<f32>,
    /// Whether `--render-scale` was given, `auto` included (V34): if so it wins over the settings'.
    render_scale_asked: bool,
    /// The `mods/` folder a track not on the disc is looked for in (MD2), or `None` to read none.
    /// Unless `mods_asked`, the settings' Mods row says, and the folder is beside `settings.txt`.
    mods: Option<std::path::PathBuf>,
    /// Whether `--mods DIR|off` was given, or the front end set [`Args::mods`] from its settings.
    mods_asked: bool,
    /// `--tricks original|mxvatv` (MD16): whose tricks the ride has. Unset, the front end's Tricks
    /// row says, and a ride outside it has the original's. Online is always the original's.
    tricks: Option<menu::Tricks>,
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

    /// VSync and the frame limit (P90): `--vsync` and `--fps-limit` where given, else `settings`'.
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
    args.set = Some(first.set);
    args.look = first.look.clone();
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
    };
    front.menu.music_choices = front.sound.choices();
    // The track mods (MD2), after the disc's tracks in the pick; read again when the Mods row turns.
    let mut mods_on = front.menu.settings.mods;
    let listed = mod_tracks(&args, &front.dir, mods_on);
    // And each one's picture on its card (P70), the stills that are missing taken off this thread.
    let mut stills = mod_pictures(&mut front.art, mods_folder(&args, &front.dir, mods_on).as_deref(), &listed);
    front.menu.set_mod_tracks(listed);
    // Online, this machine's player rides the freeride's bike set unless the lobby changes it, as
    // the rider the command line names.
    front.menu.lobby.bikes = lobby::bikes();
    front.menu.lobby.pick = openmx_net::Pick::new(&args.rider, &roster::Set::FREERIDE.word());
    // In the first seat's look from the garage (P86), which the seat list shows the others.
    front.menu.lobby.pick.look = front.menu.picks.0[0].look.text();
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
    let mut limiter = pace::Limiter::default();
    let mut escape = false;
    // 1P's saved markers on the lobby's track, for this machine's pick (P99).
    let mut pick_markers = saved_markers::PickMarkers::default();
    loop {
        if matches!(act, Act::Ride(_) | Act::Online) {
            pick_markers.stale();
        }
        if let Act::Ride(track) = &act {
            shell.front.reader.reset();
            loop {
                let mut a = args.clone();
                a.track = track.clone();
                if !args.mods_asked {
                    a.mods_asked = true;
                    a.mods = shell.front.menu.settings.mods.then(|| shell.front.dir.join(track_mods::MODS_DIR));
                }
                let overlay = args.overlay.unwrap_or(shell.front.menu.settings.overlay);
                a.overlay = Some(overlay);
                a.hud = overlay == menu::Overlay::Debug;
                a.speedometer = Some(args.speedometer.unwrap_or(shell.front.menu.settings.speedometer));
                a.minimap = Some(args.minimap.unwrap_or(shell.front.menu.settings.minimap));
                a.hud_scale = Some(args.hud_scale.unwrap_or(shell.front.menu.settings.hud_scale));
                a.tricks = Some(args.tricks.unwrap_or(shell.front.menu.settings.tricks));
                a.graphics = Some(args.graphics.unwrap_or(shell.front.menu.settings.graphics));
                // The Original tab's own two (P126).
                a.plants = Some(args.plants.unwrap_or(shell.front.menu.settings.plants));
                if !args.aspect_asked {
                    a.aspect = aspect_of(shell.front.menu.settings.picture);
                }
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
            if let Some((room, _)) = shell.front.menu.lobby.start.clone() {
                let mut a = args.clone();
                a.track = room.track.clone();
                // Online rides the disc's tracks only, until MD6 says how peers agree on a mod.
                (a.mods_asked, a.mods) = (true, None);
                // And the original's tricks, until MD6's check that every seat has the same pack.
                a.tricks = Some(menu::Tricks::Original);
                let overlay = args.overlay.unwrap_or(shell.front.menu.settings.overlay);
                a.overlay = Some(overlay);
                a.hud = overlay == menu::Overlay::Debug;
                a.speedometer = Some(args.speedometer.unwrap_or(shell.front.menu.settings.speedometer));
                a.minimap = Some(args.minimap.unwrap_or(shell.front.menu.settings.minimap));
                a.hud_scale = Some(args.hud_scale.unwrap_or(shell.front.menu.settings.hud_scale));
                a.graphics = Some(args.graphics.unwrap_or(shell.front.menu.settings.graphics));
                // The Original tab's own two (P126).
                a.plants = Some(args.plants.unwrap_or(shell.front.menu.settings.plants));
                if !args.aspect_asked {
                    a.aspect = aspect_of(shell.front.menu.settings.picture);
                }
                if !args.render_scale_asked {
                    a.render_scale = shell.front.menu.settings.render_scale.fraction();
                }
                a.wrecks = room.wrecks;
                // This machine's rider in the first seat's look (P86), as the room was told.
                a.look = shell.front.menu.picks.0[0].look.clone();
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
        // Past the splash 1P's pad and the keyboard drive the menu as one (P109): a push both see
        // is one press.
        let menu = &shell.front.menu;
        let hosted = |d: &Dev| menu.screen() != Some(menu::Screen::Splash) && menu.hosts(d);
        let mut presses = shell.front.readers.read_hosted(&reports, &hosted);
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
        if shell.front.menu.settings.mods != mods_on {
            mods_on = shell.front.menu.settings.mods;
            let listed = mod_tracks(&args, &shell.front.dir, mods_on);
            stills = mod_pictures(&mut shell.front.art, mods_folder(&args, &shell.front.dir, mods_on).as_deref(), &listed);
            shell.front.menu.set_mod_tracks(listed);
            (shown_look, menu_images) = (None, false);
        }
        // A still the job took is the card's picture from the next frame on.
        while let Some((id, got)) = stills.as_ref().and_then(|h| h.try_recv().ok()) {
            match got {
                Ok(picture) => {
                    println!("mods: took a still of {id} from its spawn");
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
        // 1P's ride setup machine into it too (N18), which every machine starts the seat on, as the
        // ride's own start reads it: `--vehicle biplane` over the setup.
        let machine = if args.vehicle == Kind::Biplane { Kind::Biplane.name() } else { shell.front.menu.settings.machines[0].word() };
        shell.front.menu.lobby.set_machine(machine);
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
    }
}

fn ride(args: Args, shell: Option<&mut Shell>) -> Result<Leave, String> {
    // ---- the game modifiers (P55) ----------------------------------------------------------------
    // Applied to what the ride loads, never as a branch in the simulation: at their defaults nothing
    // is touched at all.
    let lobby_room = shell.as_deref().and_then(|s| s.front.menu.lobby.start.as_ref()).map(|(room, _)| room);
    let mut mods = ride_modifiers(&args, lobby_room)?;
    if !mods.is_original() {
        println!("modifiers: {}", mods.text());
    }

    // ---- the disc -------------------------------------------------------------------------------
    let mut disc = Disc::open_configured()?;
    // A track the disc does not have is a track mod's (MD2), looked for in the mods folder only
    // then, so a disc track's ride reads nothing from it.
    let track_member = member_ending(&disc, &format!("{}.res", args.track));
    // A world mod (MD8) is several track mods as one ground: the ride is dressed and lit from its
    // home tile's folder, as a track mod's is, and the simulation asks every tile.
    let mut world_mod: Option<worlds::WorldMod> = None;
    let modded = match &track_member {
        Ok(_) => None,
        Err(e) => match mods_dir(&args) {
            Some(dir) if worlds::is_world(&dir, &args.track) => {
                let w = worlds::find(&dir, &args.track).map_err(|m| format!("{e}, and {m}"))?;
                let home = w.home_track();
                world_mod = Some(w);
                Some(home)
            }
            Some(dir) => Some(track_mods::find(&dir, &args.track).map_err(|m| format!("{e}, and {m}"))?),
            None => return Err(e.clone()),
        },
    };
    // The name the track's other files are built from: a disc track's own, a mod's scene's stem.
    let stem = modded.as_ref().map_or_else(|| args.track.clone(), |t| t.stem.clone());
    let bike_member = member_ending(&disc, &format!("{}.res", args.bike))?;
    // A track is two archives: the props and the terrain in one, the bike's animation set, its
    // physics and the follow camera in the other, which the game loads alongside it. A mod has no
    // second archive and rides with a disc freeride's.
    let second_of = if modded.is_some() { track_mods::FREERIDE } else { args.track.as_str() };
    let second = member_ending(&disc, &format!("{second_of}_2.res")).ok();
    // The rider is a fourth archive, named after the rider rather than the track or the bike: the
    // body, its gear and its motions are all in there.
    let rider_member = match args.rider.as_str() {
        "none" => None,
        name => Some(member_ending(&disc, &format!("{name}.res"))?),
    };
    let track_res = match (&modded, &track_member) {
        (Some(t), _) => {
            println!("mods: {} ({}) from {}", t.id, t.name, t.dir.display());
            t.open()?
        }
        (None, Ok(member)) => disc.res(member)?,
        (None, Err(e)) => return Err(e.clone()),
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
    // MX vs. ATV's tricks (MD16), from the first trick pack in the mods folder: its extra stunts after
    // the 24, and below its moves laid over every bike. Without a pack the ride has the original's.
    let trick_pack = ride_trick_pack(&args);
    let stunts = match &trick_pack {
        Some(pack) => {
            let stunts = pack.stunts(stunts);
            println!("{}", pack.missing_clips(&stunts));
            stunts
        }
        None => stunts,
    };

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
    let mut place = track::Scene::parse(&scn_bytes)?;
    if let Some(t) = &modded {
        t.place_spawn(&mut place)?;
    }
    if let Some(w) = &world_mod {
        w.place_spawn(&mut place)?;
    }
    let mut look = view::Scene::parse(&ini::text(&scn_bytes));
    if let Some(w) = &world_mod {
        worlds::move_look(&mut look, w.home_at());
    }

    let terrain_bytes = terrain_bytes(&archives[0], &place.terrain_file)?;
    // The same file parsed twice, because the two sides of it are owned by different crates: the
    // shell's `Ground` draws it in host floats, the port's answers the ride's ray in the game's.
    let ground = Ground::new(track::Terrain::parse(&terrain_bytes)?, &place.terrain);
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
    let sim_terrain = track::Terrain::parse(&terrain_bytes)?;
    // A world's ground (MD8): every tile and join, each asked in its own coordinates.
    let mut tiled = match &world_mod {
        Some(w) => {
            let t = w.tiled(game_sim::tiles::Reach::DESKTOP)?;
            println!("{}", w.line(&t));
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
    // The first rider's name and number on their bike's plates and, below, their jersey (P86).
    if let Some(k) = &mut kit {
        assets.make_texture(dress::NAME_TEXTURE, k.texture(&args.rider, &args.look, &args.bike));
    }
    // The trees and bushes the scene's ecosystem scatters over the ground (V48).
    match game_render::eco::Grove::load(&archives, &look, &place.terrain, &sim_terrain, &mut assets) {
        // A world's (MD8) would stand in the home tile's coordinates, and only its: none yet.
        Ok(_) if world_mod.is_some() => println!("ecosystem: not drawn in a world"),
        Ok(grove) => look.ecosystem = grove,
        Err(e) => println!("ecosystem: {e}"),
    }
    if let Some(grove) = &mut look.ecosystem {
        grove.lod = args.plants.unwrap_or_default();
        println!("plants: {}", grove.lod.word());
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
    let mut stock_phy = Phy::read_bytes(&phy_bytes, &tables.ride.curve);
    if let Some(pack) = &trick_pack {
        println!("{}", pack.lay(&mut stock_phy, &phy_member));
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
                if let Some(pack) = &trick_pack {
                    println!("player {}: {}", i + 1, pack.lay(&mut phy, &member));
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
        if let Some(k) = &kit {
            assets.rename_materials(&k.repaint(&body.rider.suit, &args.look));
        }
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
    // and physics. A track with no such section has no plane, and `--vehicle biplane` on it is an
    // error.
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
    let plane_phy = machine_res.as_ref().and_then(|res| {
        let e = res.find_suffix(phy::BIPLANE_PHY)?;
        res.read(&e.name).ok()
    });
    // And its collision object's ray probe (S48), which is what it strikes the ground and the
    // track's meshes with; without it the plane meets the world only with its wheels.
    let plane_rays = machine_res.as_ref().and_then(|res| {
        let e = res.find_suffix(game_sim::flight_search::BIPLANE_RAYS)?;
        let bytes = res.read(&e.name).ok()?;
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
            if let Some(rays) = &plane_rays {
                flight = flight.with_rays(rays.clone());
            }
            if let Some(mesh) = &bike_box {
                flight = flight.with_bike(mesh.clone());
            }
            let t = &place.terrain;
            flight.area = Some(Area::new(F(t.width as f32), F(t.breadth as f32), t.width_scale, t.breadth_scale));
            if let Some(area) = world_area {
                flight.area = Some(area);
            }
            flight
        })
    };
    let plane = lot.plane(0, flight_for(0));
    // Every online seat's own biplane bay (N18), which a seat that starts the ride in the plane
    // stands in: the lot parked for the most seats a ride has, a copy's place being its own whatever
    // the count, so every machine has the same off the same disc.
    let online_bays: Vec<vehicle::Spawn> = match &machines {
        Some((_, b)) => {
            let mut probe = sim_world::Ground::new(&sim_terrain, &place.terrain);
            let mut ground = |at: Vf| sim_world::World::ground(&mut probe, Vf::new(at.x(), 1.0e4, at.z(), 1.0)).0.y();
            vehicle::park(b, Machine::Biplane, net::SEATS, &mut ground).into_iter().map(|bay| bay.spawn).collect()
        }
        None => Vec::new(),
    };
    // And the biplane's, for the same (P98).
    let stock_plane = plane_phy.as_deref().map(phy::Plane::read_bytes);
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
            cam.cam.ceiling = Race::ceiling_for_fog(F(look.fog_far));
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
        Some(drawn) => match load_plane_fx(&archives, &stem, drawn, &assets) {
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
        None => Land::Track(Place { ground: sim_world::Ground::new(&sim_terrain, &place.terrain), props }),
    };
    // The cars the track's `.toy` drives along their clips (V41): free06's train, its trucks and
    // scrapers, the other freeride tracks' trailers. Drawn, not ridden into: they reach nothing in the
    // simulation, and their clock is the scene's own, summed from the ticks' `dt` below.
    let traffic = game_render::toy::Traffic::load(&archives, &stem, &mut |p| car_ground(&mut sim, p));
    // A car's model object is a scene prop's, so its pose stands in for the file's root node: the
    // train's turned cars (train_car_03/04/05 carry a half turn there) are drawn without it (V46).
    for (car, _) in &traffic.cars {
        assets.load_placed(&archives, car.stem());
    }
    if !traffic.cars.is_empty() || !traffic.skipped.is_empty() {
        println!("traffic: {} cars on clips, {} left out", traffic.cars.len(), traffic.skipped.len());
    }
    let mut traffic_time = 0.0f64;
    // The scene's clock at the tick before the last, which the picture blends from (V47).
    let mut traffic_before = 0.0f64;
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
        if let Some(col) = &bike_col {
            rider.carries_bike_col(Arc::clone(col));
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
            ground_radius: ground.radius(),
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
        // Its own name and number (P86), on whatever of its bike and body is loaded below.
        let own_look = args.others.get(i - 1).map(|p| p.look.clone()).unwrap_or_default();
        if let Some(k) = &mut kit {
            let pick = args.others.get(i - 1);
            let (rider, bike) = pick.map_or((&args.rider, &args.bike), |p| (&p.rider, &p.bike));
            assets.make_texture(dress::NAME_TEXTURE, k.texture(rider, &own_look, bike));
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
                // Its own plates when anyone has a number (P86): the parts again under its own names.
                let parts = match kit.is_some() {
                    true => vehicle
                        .parts
                        .iter()
                        .map(|part| {
                            let name = format!("{}@{}", part.model, i + 1);
                            assets.load_repainted(&archives, &part.model, &name, &vehicle.materials);
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
        rigs.push(Rig {
            body,
            chassis,
            camera: Race { cam: camera.cam.clone() },
            // Their own plane's camera, on the same presets as the first player's (P87).
            plane_camera: plane_camera.as_ref().map(|c| Race { cam: c.cam.clone() }),
            wreck_camera: chase::WreckCamera::new(false),
            framing: (chase::Framing::ORIGINAL, chase::Framing::ORIGINAL),
            fov_ease: Default::default(),
            hud: Hud::new(args.hud),
            odometer: overlay::Odometer::default(),
            preload_meter: overlay::PreloadMeter::default(),
            ride_hud: ride_hud::Tracker::default(),
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
            framing: (chase::Framing::ORIGINAL, chase::Framing::ORIGINAL),
            fov_ease: Default::default(),
            // The riding overlay (P6). It is the shell's own furniture, so it is created here beside
            // the window rather than inside the simulation's `World`, and it reads the port's own
            // values at draw time instead of being fed them.
            hud: Hud::new(args.hud),
            odometer: overlay::Odometer::default(),
            preload_meter: overlay::PreloadMeter::default(),
            ride_hud: ride_hud::Tracker::default(),
            bike_model: CHASSIS.to_string(),
            body_model,
            parts: vehicle.parts.clone(),
            modified: !mods.seat_is_original(0),
        },
    );

    // Everything the GPU draws, uploaded once: the props, the sky, the whole ground and every model.
    if gpu_on && let Some(r) = renderer_of(&mut offscreen, window) {
        let at = Instant::now();
        // A world's ground is every piece's, unpainted: each tile's paint is its own (MD9).
        let (chunks, paint) = match (&world_mod, &sim) {
            (Some(w), Land::Tiles(t)) => (worlds::chunks(w, t, ground.chunk_cells()), game_render::gpu::GroundPaint::Bare),
            _ => {
                let paint = match (&ground.paint, &ground.pages) {
                    (Some(splat), _) => game_render::gpu::GroundPaint::Splat(splat),
                    (None, Some(pages)) => game_render::gpu::GroundPaint::Pages(pages),
                    (None, None) => game_render::gpu::GroundPaint::Bare,
                };
                (ground.chunks(ground.chunk_cells()), paint)
            }
        };
        r.forget_assets();
        r.set_scene(&look);
        r.set_sky(sky.as_ref());
        r.set_ground_painted(&chunks, paint, ground::DIRT)?;
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
    // The shape is the settings' Picture Shape's unless `--aspect` was given, and changes with it
    // from the pause (P126).
    let mut aspect = args.aspect;
    let frame_size = |window: &Option<Screen>, aspect: Aspect| match (args.size, window) {
        (Some(size), _) => size,
        (None, Some(window)) => aspect.frame((window.width, window.height), window.scale),
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
        .find_map(|a| a.read_suffix(&format!("{}\\hit.dat", args.track)).ok())
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
    // The GPU's copy of its pictures, uploaded with the first frame's glows.
    let mut glow_textures = portal_art.as_ref().map(|a| a.textures.clone());
    let mut overlay_mode = args.overlay.unwrap_or_default();
    let mut speedometer = args.speedometer.unwrap_or_default();
    let mut minimap = args.minimap.unwrap_or(true);
    let mut hud_scale = args.hud_scale.unwrap_or(menu::HUD_SCALE_DEFAULT);
    let mut graphics = args.graphics.unwrap_or_default();
    let mut plants = args.plants.unwrap_or_default();
    // Set when the pause's settings change the picture's shape: the frame is re-made as for a resize.
    let mut reshaped = false;
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
        teleported: false,
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
            teleported: false,
        });
    }
    let mut world = World { place: sim, seats, tape: recorder, played_dt: None };
    // The start zones' portals (V51), laid over the ground once: they never move.
    let zone_portals: Vec<portal::Portal> = portal::shown(&zone_spots, false)
        .iter()
        .map(|spot| portal::Portal::zone(spot, |x, z| portal_ground(&mut world.place, x, spot.at[1] + 50.0, z)))
        .collect();
    let mut marked_portals: Vec<[Option<portal::Portal>; markers::SLOTS]> = vec![Default::default(); world.seats.len()];

    for (seat, rig) in world.seats.iter().zip(&mut rigs) {
        rig.hud.note_vehicle(seat.garage.up.name());
        let first = rig.follow(seat.garage.up);
        if !first.cam.presets.is_empty() {
            let name = first.cam.preset().name.clone();
            rig.hud.note_camera(&name);
            rig.ride_hud.note_camera(Instant::now(), &name);
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
    let mut start_readers: Vec<Reader> = (1..world.seats.len()).map(|_| Reader::at(RIDE_HZ)).collect();
    // H toggles it, on its own edge: the window keeps key state and not key events, so the shell
    // remembers last frame's to tell a press from a hold.
    let mut hud_key = false;
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
            let stock = Phy::read_bytes(&bytes, &tables.ride.curve);
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
            if world.seats.len() != 1 {
                return Err("an online ride from the lobby has one local player: drop --players".into());
            }
            let disc = &front.as_deref().expect("from the lobby").network.disc;
            let mut sims = Vec::with_capacity(room.seats.len());
            // each seat's bike file before the modifiers, which a change of them mid-ride is built
            // on again (P100); an open seat rides this machine's first's
            let mut stock = Vec::with_capacity(room.seats.len());
            for seat in &room.seats {
                let mut sim = world.seats[0].sim.clone();
                stock.push(stock_phys[0].clone());
                if let Some(seat) = seat {
                    let bytes = disc.phy(&seat.pick.bike).ok_or(format!("the disc has no {} bike set", seat.pick.bike))?;
                    // The host's modifiers, 1P's for every seat: each seat's rider is a copy of
                    // this machine's first, built on 1P's (P55).
                    let mut phy = Phy::read_bytes(&bytes, &tables.ride.curve);
                    *stock.last_mut().expect("pushed") = phy.clone();
                    mods.bike(0, &mut phy);
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
        on.bays(online_bays.clone());
        if let Some(flies) = &lobby_planes {
            on.start_planes(flies)?;
        }
    }
    let mut online_want: Option<Kind> = None;
    let mut online_frames = 0u32;
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
    // The pause offers "Open to Online" (N10) on a local freeride of one player on the bike set the
    // disc's own lookup gives (what a joiner's machine loads for it), outside any lobby.
    if let Some(front) = front.as_deref_mut() {
        let set = args.set.unwrap_or(roster::Set::FREERIDE).word();
        let can = online.is_none()
            && head.is_none()
            && world.tape.is_none()
            && world.seats.len() == 1
            && args.phy.is_none()
            && modded.is_none()
            && front.menu.lobby.room.is_none()
            && front.network.disc.phy(&set).is_some_and(|b| *b == phy_bytes);
        front.menu.opening = if can { menu::Opening::Can } else { menu::Opening::No };
        front.menu.mods_lock = mods_lock(head.is_some(), online.as_ref().map(online::Online::host));
        front.menu.leave_to = online.as_ref().map_or(menu::LeaveTo::Main, online::Online::leaves_to);
    }
    // The modifiers the Settings screen last said, so that the pause closing on a set the player
    // changed puts it on the ride (P98) and a `--modifiers` ride the player did not touch keeps its own.
    let mut menu_mods = front.as_deref().map(|f| f.menu.settings.modifiers);
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
    // Each view's markers' slot display (P88).
    let mut banners: Vec<hud::MarkerBanner> = Vec::new();
    let mut before_planes: Vec<Option<M4>> = Vec::new();
    // Each seat's cut test (P120): whether the last tick's picture is a jump from the one before.
    let mut cuts: Vec<pace::Cut> = Vec::new();
    // The moment the picture is of (P120): read once a pass of the loop, after the last picture was
    // shown, and both the ticks owed and the blend are taken at it. Read again after the ticks, the
    // blend came a tick's cost later on the frames that ran one than on those that did not, though
    // each went up at the display's next refresh, and the faster the ride the more that showed.
    let mut moment = Instant::now();
    // What the last tick left for the picture: whether it simulated, and each player's ride for the
    // camera and the overlay.
    let mut simulated = false;
    let mut ridden: Vec<Ridden> = Vec::with_capacity(args.players);
    let mut top = Instant::now();
    let mut limiter = pace::Limiter::default();
    // The settings a ride without the front end paces its picture by (P90).
    let ride_settings = match (&front, &window) {
        (None, Some(_)) => menu::load(&menu::store_dir(Disc::configured_path().ok().as_deref())).0,
        _ => menu::Settings::default(),
    };
    perf.begin();
    loop {
        if pacer.owed(moment) {
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
        if window.as_mut().is_some_and(|w| w.resized()) || std::mem::take(&mut reshaped) {
            let (width, height) = frame_size(&window, aspect);
            if (width, height) != (frame.width, frame.height) {
                frame = Frame::new(width, height, look.fog_color);
                (rects, view_frames) = views_of(width, height);
                if let Some(window) = &window {
                    println!("window {}x{}, drawing {width}x{height}", window.width, window.height);
                }
                let now = Display::of(width, height, aspect);
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
        // The recording changed the modifiers before the frame it hands out next (P98): the ride
        // takes them there, as it did when it was made.
        if let Some(to) = head.as_ref().and_then(Head::change) {
            remodify(&mut world.seats, &mut rigs, &stock_phys, stock_plane.as_ref(), &mods, &to);
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
        let offering = head.is_none() && online.is_none() && front.as_deref().is_none_or(|f| f.menu.riding());
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

        // The pause, under the front end: Start or Escape opens it, and while it is open the ride is
        // frozen exactly as Start's own freeze leaves it, the pass polling the pad and stepping
        // nothing. What it changes is applied at once, and saved.
        if let Some(front) = front.as_deref_mut() {
            // Every player's press: another player's Start pauses for them, and while the pause is
            // theirs their pad drives it (P41). Escape is the keyboard's, and Back for whoever drives.
            // Each player's pad as it is, not as the ride's scheme moved it (P53).
            let menu_report = |i: usize| menu_reports.get(i).copied().unwrap_or((world.seats[i].reports[0], 0));
            let mut seat0 = vec![menu_report(0)];
            // The keyboard pauses and works 1P's pause whoever has 1P (P46), from no seat of its own:
            // read as one with 1P's pad (P109), so a push both see is one press.
            if let Some(keys) = pad_seats.as_ref().and_then(|s| s.keyboard_alone()) {
                seat0.push((keys, 0));
            }
            let first = front.reader.read_joint(&seat0);
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
            let paused = !front.menu.riding();
            let act = front.menu.step_seats(&presses);
            if front.menu.changed() {
                overlay_mode = front.menu.settings.overlay;
                speedometer = front.menu.settings.speedometer;
                minimap = front.menu.settings.minimap;
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
                // A played tape switches where it says and nowhere else. Online the pick goes into
                // this machine's pad (N17), and every machine switches the seat at the tick it lands
                // on; the garage follows below, once the session has stepped it.
                Act::Switch(i, to) if online.is_some() => {
                    match (i, Kind::parse(to.word())) {
                        (0, Ok(to)) if world.seats[0].garage.plane.is_some() || to == Kind::Bike => {
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
                        room.modifiers = mods.text();
                        let mut pick = openmx_net::Pick::new(&front.menu.lobby.pick.rider, &args.set.unwrap_or(roster::Set::FREERIDE).word());
                        // the preset this seat rides on now (N13), which the room tells a joiner's machine
                        pick.preset = net::rides::preset(&world.seats[0].sim).unwrap_or(0) as u8;
                        pick.look = front.menu.lobby.pick.look.clone();
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
                                on.bays(online_bays.clone());
                                online_want = None;
                                front.menu.mods_lock = mods_lock(head.is_some(), Some(on.host()));
                                online = Some(on);
                                front.menu.opening = menu::Opening::Open;
                            }
                            Err(e) => front.menu.note = Some(format!("Could not open the ride: {e}")),
                        }
                    }
                }
                // The pausing player's markers on this track (P99): offline the ride's slots and the
                // file, and a tape being written clears there too; online only the file — the
                // ride's slots are the session's, and stay until it ends.
                Act::ClearMarkers(i) if head.is_none() => {
                    let profile = profile_of(i);
                    let bike = bike_words.get(i).unwrap_or(&bike_words[0]);
                    match saved_markers::save(&markers_dir, &args.track, &profile, bike, &Default::default()) {
                        Err(e) => front.menu.note = Some(format!("Could not clear the markers: {e}")),
                        Ok(()) if online.is_some() => {
                            if let (Some(Some(kept)), Some(seat)) = (kept_markers.get_mut(i), world.seats.get(i)) {
                                kept.last = saved_markers::signature(&seat.sim.markers);
                            }
                            front.menu.note = Some(format!("{profile}'s saved markers on this track cleared; the ride keeps its own until it ends."));
                        }
                        Ok(()) => {
                            if let Some(seat) = world.seats.get_mut(i) {
                                seat.sim.markers.clear();
                                if let Some(Some(kept)) = kept_markers.get_mut(i) {
                                    kept.last = saved_markers::signature(&seat.sim.markers);
                                }
                            }
                            if let Some(tape) = world.tape.as_mut().filter(|_| i == 0) {
                                tape.markers_cleared();
                            }
                            front.menu.note = Some(format!("{profile}'s markers on this track cleared."));
                        }
                    }
                    println!("markers: {profile} on {} cleared", args.track);
                }
                Act::Stay
                | Act::Resume
                | Act::Ride(_)
                | Act::Switch(..)
                | Act::Online
                | Act::OpenOnline
                | Act::ClearMarkers(_) => {}
            }
            // The pause closing on modifiers the player changed in it: the ride takes them now, from
            // where it is (P98), and a tape being written takes the change at the tick it is on.
            if paused && front.menu.riding() && menu_mods != Some(front.menu.settings.modifiers) {
                let to = front.menu.settings.modifiers;
                menu_mods = Some(to);
                // Online the host's change goes to every machine, which all take it at one tick
                // (P100): the ride is moved onto it below, once the session has.
                if let Some(on) = online.as_mut().filter(|_| front.menu.mods_lock.is_none()) {
                    match on.remodify(&to) {
                        Ok(()) => println!("modifiers: {} (from the pause, for every machine at the next tick they agree)", to.text()),
                        Err(e) => front.menu.note = Some(format!("The modifiers did not change: {e}")),
                    }
                } else if front.menu.mods_lock.is_none() && to != mods {
                    remodify(&mut world.seats, &mut rigs, &stock_phys, stock_plane.as_ref(), &mods, &to);
                    mods = to;
                    engine.time_scale = mods.time_scale();
                    front.menu.modified = !mods.is_original();
                    if let Some(tape) = &mut world.tape {
                        tape.modifiers(mods);
                    }
                    println!("modifiers: {} (from the pause)", if mods.is_original() { "the original game's".into() } else { mods.text() });
                }
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
        cuts.resize_with(world.seats.len(), pace::Cut::default);
        for (i, (seat, cut)) in world.seats.iter().zip(&mut cuts).enumerate() {
            cut.before(cut_spots(seat, before[i].bike, cameras.get(i)));
        }
        traffic_before = traffic_time;
        let sim_at = Instant::now();
        let dt: F;
        (dt, simulated) = if let Some(on) = &mut online {
            online_frames += 1;
            // Paused, this machine's rider lets go: the others ride on.
            let mut pad = match front.as_deref().is_some_and(|f| !f.menu.riding()) {
                true => Report::idle(),
                false => world.seats[0].reports[0],
            };
            // The machine this player picked and their biplane scheme, which every machine reads at
            // the tick it lands on (N17).
            let scheme = world.seats[0].garage.plane.as_ref().map_or((false, false), |p| (p.every_bank, p.round_stick));
            pad.0[net::SWITCH] = vehicle::Wanted { to: online_want, every_bank: scheme.0, round_stick: scheme.1 }.byte();
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
            // The host's change of the modifiers, taken by every machine at one tick (P100): the
            // seats came back on it above; the biplane, the marks and the menu say it here.
            if on.mods() != online::first_players(&mods) {
                let to = on.mods();
                for (i, seat) in world.seats.iter_mut().enumerate() {
                    if let (Some(file), Some(flight)) = (stock_plane.as_ref(), seat.garage.plane.as_mut().and_then(|p| p.flight.as_mut())) {
                        to.replane(i, file, flight);
                    }
                }
                for (i, rig) in rigs.iter_mut().enumerate() {
                    rig.modified = !to.seat_is_original(i);
                }
                mods = to;
                if let Some(front) = front.as_deref_mut() {
                    front.menu.modified = !mods.is_original();
                }
                println!("modifiers: {} (the host's, from tick {})", if mods.is_original() { "the original game's".into() } else { mods.text() }, on.remodified_at().unwrap_or(0));
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
                for c in std::mem::take(&mut front.menu.lobby.out) {
                    on.command(c);
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
        if simulated {
            traffic_time += f64::from(dt.0);
        }
        // The preload meter's movie runs a frame a tick, as the original's HUD does (V53).
        if simulated {
            for (seat, rig) in world.seats.iter().zip(&mut rigs) {
                rig.preload_meter.step(preload_sense(seat));
            }
        }
        // A world loads the tiles ahead of the riders and lets the far ones go (MD8).
        if simulated && matches!(world.place, Land::Tiles(_)) {
            let near: Vec<Vf> = world
                .seats
                .iter()
                .flat_map(|s| [Some(game_sim::markers::Spot::of(&s.sim.rider).at), s.garage.plane.as_ref().map(|p| p.at)])
                .flatten()
                .collect();
            if let Some(t) = world.place.tiled() {
                t.stream(&near);
                for line in t.take_log() {
                    println!("{line}");
                }
            }
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
                println!("{who}wreck: reason {} — L1 and R1 together to get back on", why.0);
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

            // The player's framing (P82), laid over the original's presets the frame it changes: the
            // command line's, or the Camera page's — live, so a change under the pause shows as it is
            // made — or without a front end the shipped default.
            let view = args
                .camera_view
                .or_else(|| front.as_deref().map(|f| f.menu.settings.cameras[i.min(menu::SEATS_MAX - 1)]))
                .unwrap_or(menu::CameraView::DEFAULT);
            if framings(view) != rig.framing {
                let bike = cpresets.as_deref().map(|b| layout_presets(b, layout, display, "cpresets.ini")).unwrap_or_default();
                let plane = plane_presets.as_deref().map(|b| layout_presets(b, layout, display, PLANE_PRESETS)).unwrap_or_default();
                println!("{who}camera view: {}", view.text());
                frame_rig(rig, &bike, &plane, framings(view));
            }
            rig.wreck_camera.frame(rig.framing.0);

            // The camera, after the frame, which is where the original's viewport updates it: the shell
            // picks the preset, the ported camera does everything else.
            // Whichever machine has the controller has the camera: the bike's, on the track's presets, or
            // the plane's, on its own file's.
            // With the field of view's ease over a framing change (P119), which is the camera's own.
            let (follow, fov_ease) = match (seat.garage.up, &mut rig.plane_camera) {
                (Kind::Biplane, Some(cam)) => (cam, &mut rig.fov_ease.1),
                _ => (&mut rig.camera, &mut rig.fov_ease.0),
            };
            // The original's plane reset (the chord, a wreck's recovery) snaps the plane's camera onto
            // the spawn rather than letting it fly the whole way back (V29).
            if seat.garage.up == Kind::Biplane && seat.said.respawned {
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
            // A marker's Up at speed (P112) snaps the bike's camera onto where it lands, riding out
            // at the bike's speed (`chase::ride_out`, after the frame) — taken once, so a frame the
            // engine skips does not snap again.
            let ride_out = std::mem::take(&mut seat.teleported) && seat.garage.up == Kind::Bike;
            if ride_out {
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
                    rig.ride_hud.note_camera(Instant::now(), &follow.cam.preset().name);
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
            // The seat's right stick turns its own follow camera, L3 held zooms it, and R3 puts it
            // back (P114) — while the follow camera has the screen, so a wreck's camera is not
            // nudged from under the player. The preset changes above keep or throw the nudges away
            // as the original's do.
            if presets != 0 && !wreck.on {
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
            if let (true, Some(v)) = (args.trace_camera, &view) {
                let c = &follow.cam;
                println!(
                    "camera {i} {} {} preset {:?} distance {} pitch {} yaw {} fov {} eye {} {} {} at {} {} {}",
                    pacer.ticks(),
                    if wreck.on { "wreck" } else { "follow" },
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
                    far: light.fog_far.max(4000.0),
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
        let blend = pace::Blend(pacer.blend(moment));
        // The path cars at the picture's own moment (V47): their clips read at the clock between the
        // last two ticks, as far along as the bike's blend, so they move every frame drawn.
        if !traffic.cars.is_empty() {
            let place = &mut world.place;
            let shown = game_render::toy::between(traffic_before, traffic_time, blend.0);
            traffic_at = traffic.placed(shown, &mut |p| car_ground(place, p));
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
                let hud::MarkerBar { pick, filled, held, holding } = hud::MarkerBar::read(
                    &seat.sim.markers,
                    mine.unwrap_or(v),
                    &riding,
                    seat.sim.input.pad(0),
                    seat.sim.table(),
                    &seat.hands,
                    seat.riding,
                );
                let others = markers::Markers::others(mine.unwrap_or(v), riding.len(), &riding);
                banner
                    .update(Instant::now(), pick, filled, held)
                    .then(|| (pick, filled, hud::marker_label(pick, filled, &names), others, holding))
            })
            .collect();
        // The bays shown (P103), drawn below and marked on the radar.
        let bays = vehicle::bays_shown(&lot, front.as_deref().is_none_or(|f| f.menu.settings.bays));
        // The radar's other icons, each view its own (P92).
        hud_art.radar = (0..world.seats.len()).map(|v| radar_blips(&world.seats, &bays, v, &radar_zones, picks[v])).collect();
        // And the switch pop-up, each view its own player's (P103), as the input stage's take reads it.
        let offering = head.is_none() && online.is_none() && front.as_deref().is_none_or(|f| f.menu.riding());
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
            trace_picture(&world.seats, &rigs, &assets, ragdolled.as_ref(), &before, &cameras, &cut, &blend, pacer.ticks());
        }
        // `--shot-pose` (MD10): the first view looks from the pose instead, whatever the ride does.
        let mut cameras = cameras;
        if let (Some(pose), Some(at), Some(camera)) = (args.shot_pose, rects.first(), cameras.first_mut()) {
            *camera = pose.camera(at.w, at.h, NEAR_CLIP, camera.far);
        }
        // The start zones' and the markers' portals (V51), each view its own: its player's markers,
        // and the zones that player is shown.
        for (v, seat) in world.seats.iter().enumerate() {
            portal::markers(&mut marked_portals[v], &seat.sim.markers, picks[v], |x, y, z| portal_ground(&mut world.place, x, y, z));
        }
        let glows: Vec<Vec<game_render::glow::Glow>> = match &portal_art {
            Some(art) => (0..world.seats.len())
                .map(|v| {
                    let in_machine = world.seats[v].garage.up != Kind::Bike;
                    let seconds = started.elapsed().as_secs_f32();
                    cameras.get(v).map_or_else(Vec::new, |c| portal_glows(art, &zone_portals, &marked_portals[v], in_machine, c, seconds))
                })
                .collect(),
            None => Vec::new(),
        };

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
                rig.hud.scale = hud_art.scale;
                rig.hud.bindings = *seat.sim.table();
                rig.hud.flight = match (bike_up, &seat.garage.plane) {
                    (false, Some(plane)) => plane.gauges().map(|(throttle, air)| (throttle.0, air.0)),
                    _ => None,
                };
                // The ride HUD draws its own marker menu (P121); the debug HUD and the original's keep
                // the slot display.
                let ride = !rig.hud.on && overlay_mode == menu::Overlay::Hud;
                let banner = shown_markers.get(v).and_then(Option::as_ref).filter(|_| marker_bar_shown(overlay_mode, rig.hud.on) && !ride);
                if rig.hud.on || banner.is_some() {
                    let hud = &rig.hud;
                    let draw = |f: &mut Frame| {
                        hud.draw(f, seat.sim.input.pad(0), &seat.sim.rider, seat.riding && simulated);
                        if let Some((pick, filled, label, others, holding)) = banner {
                            hud::draw_markers_scaled(f, *pick, *filled, label, others, *holding, hud.scale);
                        }
                    };
                    gpu_patches.extend(gpu_layers[v].patch(at.w, at.h, (at.x, at.y), draw));
                }
                if !rig.hud.on && overlay_mode == menu::Overlay::Original {
                    let art = &hud_art;
                    // The original's own HUD (V26), as the software path draws it.
                    let b = ridden[v].target.body_point;
                    if seat.riding && simulated {
                        rig.odometer.step([b.x(), b.y(), b.z()]);
                    }
                    let (window, gauges) = ((frame.width, frame.height), gauges(seat, rig, speedometer));
                    art.sprites(&mut gpu_sprites, (at.x, at.y, at.w, at.h), window, &ridden, v, radar_span, &gauges);
                }
                if ride {
                    let marker = shown_markers.get(v).and_then(Option::as_ref);
                    let view = [at.x as f32, at.y as f32, at.w as f32, at.h as f32];
                    let ctx = RideHud { ridden: &ridden, art: &hud_art, span: radar_span, speedometer, names: &names, table: &table, simulated };
                    ctx.with(seat, rig, v, marker, |shown, tracker, now| {
                        ride_hud::sprites(&mut gpu_sprites, hud_base, view, shown, tracker, now);
                    });
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
                        // The finest, always: a rider is the player's own, and thrown off the edge (S59)
                        // the camera falls far enough behind to earn the coarsest group.
                        lod_from: None,
                        alpha,
                    });
                }
            }
            // Every player's plane, in its own livery (P87), while it is flown.
            for (p, seat) in world.seats.iter().enumerate() {
                let Some(plane) = seat.garage.plane_flown() else { continue };
                let Some(drawn) = plane_drawn.get(plane.livery).or(plane_drawn.first()) else { continue };
                let before = before_planes.get(p).copied().flatten().filter(|_| !cut[p]);
                let at = before.map_or(plane_at(plane), |b| blend.slide(b, plane_at(plane)));
                for (model, at) in drawn.turned_parts(at, plane.prop_angle().map(|a| a.0)) {
                    actors.push(Actor { model, at, pose: None, lod_from: None, alpha: 1.0 });
                }
            }
            // And every bay's, see-through where it is parked (P103), the put-back ghost's path.
            for bay in &bays {
                let plane = vehicle::bay_plane(bay);
                let Some(drawn) = plane_drawn.get(plane.livery).or(plane_drawn.first()) else { continue };
                for (model, at) in drawn.turned_parts(plane_at(&plane), plane.prop_angle().map(|a| a.0)) {
                    actors.push(Actor { model, at, pose: None, lod_from: None, alpha: vehicle::BAY_ALPHA });
                }
            }
            for (model, at) in &traffic_at {
                actors.push(Actor { model: model.as_str(), at: *at, pose: None, lod_from: None, alpha: 1.0 });
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
                        .map(|ride| Actor { model: rig.bike_model.as_str(), at: chassis_at(ride), pose: None, lod_from: None, alpha: MARKER_ALPHA })
                        .collect();
                    let livery = seat.garage.plane.as_ref().map_or(0, |p| p.livery);
                    for f in marks.filter_map(|m| m.flight()).filter(|_| seat.garage.up == Kind::Biplane) {
                        let plane = vehicle::marked_plane(f, livery);
                        let Some(drawn) = plane_drawn.get(plane.livery).or(plane_drawn.first()) else { continue };
                        for (model, at) in drawn.turned_parts(plane_at(&plane), plane.prop_angle().map(|a| a.0)) {
                            ghosts.push(Actor { model, at, pose: None, lod_from: None, alpha: MARKER_ALPHA });
                        }
                    }
                    if ghosts.is_empty() { Vec::new() } else { actors.iter().cloned().chain(ghosts).collect() }
                })
                .collect();
            let views: Vec<View> = rects
                .iter()
                .zip(&cameras)
                .take(world.seats.len())
                .enumerate()
                .map(|(v, (at, camera))| {
                    let actors = match marked.get(v) {
                        Some(own) if !own.is_empty() => own,
                        _ => &actors,
                    };
                    View { rect: [at.x as u32, at.y as u32, at.w as u32, at.h as u32], camera, actors }
                })
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
                    set_glows(r, &mut glow_textures, &glows);
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
                    set_glows(&mut w.renderer, &mut glow_textures, &glows);
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
            let mut tris = ground.draw(target, camera, &light, pos, ground.radius());
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
                // With the plane up the bike is nowhere (P102), and the rider is the plane's pilot —
                // whom V21 seats.
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
                    _ => None,
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
                let Some(plane) = seat.garage.plane_flown() else { continue };
                let Some(drawn) = plane_drawn.get(plane.livery).or(plane_drawn.first()) else { continue };
                let before = before_planes.get(p).copied().flatten().filter(|_| !cut[p]);
                let at = before.map_or(plane_at(plane), |b| blend.slide(b, plane_at(plane)));
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
            // The start zones' and the markers' portals (V51), over the particles as the GPU lays them.
            if let (Some(art), Some(glows)) = (&portal_art, glows.get(v)) {
                tris += game_render::glow::draw(target, camera, &art.textures, glows);
            }
            stages.mark(stages::Stage::Scene);
            // And the overlay over the top of it, after everything with a depth test and before anything
            // that reads the frame: what the window shows and what `--shot` writes are the same picture.
            let (seat, rig) = (&world.seats[v], &mut rigs[v]);
            let bike_up = seat.garage.up == Kind::Bike;
            rig.hud.plane = !bike_up;
            rig.hud.scale = hud_art.scale;
            rig.hud.bindings = *seat.sim.table();
            rig.hud.flight = match (bike_up, &seat.garage.plane) {
                (false, Some(plane)) => plane.gauges().map(|(throttle, air)| (throttle.0, air.0)),
                _ => None,
            };
            rig.hud.draw(target, seat.sim.input.pad(0), &seat.sim.rider, seat.riding && simulated && !seat.waiting);
            let ride = !rig.hud.on && overlay_mode == menu::Overlay::Hud;
            if let Some(Some((pick, filled, label, others, holding))) = shown_markers.get(v)
                && marker_bar_shown(overlay_mode, rig.hud.on)
                && !ride
            {
                hud::draw_markers_scaled(target, *pick, *filled, label, others, *holding, rig.hud.scale);
            }
            // The original's own HUD (V26) where the debug one is not up, unless `none` was asked for.
            if !rig.hud.on && overlay_mode == menu::Overlay::Original {
                let art = &hud_art;
                let b = ridden[v].target.body_point;
                if seat.riding && simulated {
                    rig.odometer.step([b.x(), b.y(), b.z()]);
                }
                art.draw(target, (width, height), &ridden, v, radar_span, &gauges(seat, rig, speedometer));
            }
            // Or the ride HUD (P121), the default.
            if ride {
                let marker = shown_markers.get(v).and_then(Option::as_ref);
                let ctx = RideHud { ridden: &ridden, art: &hud_art, span: radar_span, speedometer, names: &names, table: &table, simulated };
                ctx.with(seat, rig, v, marker, |shown, tracker, now| {
                    ride_hud::draw(target, shown, tracker, now);
                });
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
        // VSync and the frame limit (P90), from the front end's settings as they are now, or the
        // file's for a ride without one; `--draw-fps` wins over both.
        let floor = match window.as_mut() {
            Some(w) => picture_floor(w, args.pacing(front.as_ref().map_or(&ride_settings, |f| &f.menu.settings))),
            None => None,
        };
        limiter.wait(args.draw_frame().or(floor));
        stages.mark(stages::Stage::Pace);
        stages.frame();
        loaded.get_or_insert_with(|| (game_render::tally::Tally::now(), stages::bytes_read_mark()));
        if args.timings && stages_since.elapsed() >= Duration::from_secs(1) {
            println!("     {}", stages.line());
            stages_since = Instant::now();
        }
        top = Instant::now();
        moment = top;
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
        // And where the first rider ended, so a ride on a track can be seen to have gone somewhere (MD2).
        let end = world.seats[0].sim.rider.ride.position();
        println!("end: {:.0}, {:.0}, {:.0}", end.x(), end.y(), end.z());
        // And the first player's plane, flown or not, for a tape that switched into it (P102).
        if let Some(flight) = world.seats[0].garage.plane.as_ref().and_then(|p| p.flight.as_ref()) {
            println!("plane hash {:#018x} ({})", game_sim::hash::of(flight), world.seats[0].garage.up.name());
        }
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
    chase::reframe(&mut rig.camera, &chase::framed(bike, framing.0), &mut rig.fov_ease.0);
    if let Some(cam) = &mut rig.plane_camera {
        chase::reframe(cam, &chase::framed(plane, framing.1), &mut rig.fov_ease.1);
    }
    rig.framing = framing;
}

/// A player's Camera page (P82) as the shell frames their bike's camera and their plane's with it.
fn framings(v: menu::CameraView) -> (chase::Framing, chase::Framing) {
    let f = |(fov, distance, height): (f32, f32, f32)| chase::Framing { fov, distance, height };
    (f(v.bike()), f(v.plane()))
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

/// The `mods/` folder a ride may look for a track in (MD2): `--mods`' or the front end's, else beside
/// `settings.txt` when its Mods row is on. `None` reads nothing.
fn mods_dir(args: &Args) -> Option<std::path::PathBuf> {
    if args.mods_asked {
        return args.mods.clone();
    }
    let dir = menu::store_dir(Disc::configured_path().ok().as_deref());
    menu::load(&dir).0.mods.then(|| dir.join(track_mods::MODS_DIR))
}

/// The trick pack a ride lays over the original's tricks (MD16): only with `--tricks mxvatv` or the
/// Tricks row's MX vs. ATV, never online, and only when the mods folder has one; the line says why not.
fn ride_trick_pack(args: &Args) -> Option<trick_packs::Pack> {
    if args.tricks != Some(menu::Tricks::MxVsAtv) {
        return None;
    }
    if args.online.is_some() {
        println!("tricks: the original's online, until every seat can be checked for the same pack (MD6)");
        return None;
    }
    let Some(dir) = mods_dir(args) else {
        println!("tricks: the mods folder is off, so the original's");
        return None;
    };
    match trick_packs::find(&dir) {
        Ok(pack) => {
            println!("tricks: MX vs. ATV's, from {}", pack.id);
            Some(pack)
        }
        Err(e) => {
            println!("tricks: {e}; the original's");
            None
        }
    }
}

/// The track mods a front end lists (MD2), saying why each one left out was: `--mods`' folder if it
/// was given (`off` is none), else the one beside the settings in `dir` while the Mods row is `on`.
/// With none, nothing is read.
fn mod_tracks(args: &Args, dir: &std::path::Path, on: bool) -> Vec<(String, String)> {
    let Some(folder) = mods_folder(args, dir, on) else { return Vec::new() };
    let scan = track_mods::scan(&folder);
    for line in &scan.log {
        println!("{line}");
    }
    let worlds = worlds::listed(&folder);
    if !scan.tracks.is_empty() || !worlds.is_empty() {
        println!("mods: {} tracks, {} worlds", scan.tracks.len(), worlds.len());
    }
    scan.tracks.into_iter().map(|t| (t.id, t.name)).chain(worlds).collect()
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
        println!("{line}");
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
/// restart of the session, and they take it there (P100); a guest's pause cannot change them.
fn mods_lock(playing: bool, online: Option<bool>) -> Option<String> {
    match (playing, online) {
        (true, _) => Some("A played tape rides its own modifiers: they change where it changed them".into()),
        (_, Some(false)) => Some("Online only the host changes the modifiers: every rider takes them at the same moment".into()),
        _ => None,
    }
}

/// **The ride takes another set of modifiers, from where it is** (P98): every seat's bike built again
/// from its stock file `stock[i]` on `to` ([`Mods::rebike`](game_sim::modifiers::Mods::rebike)), its
/// biplane retuned off the stock `plane` the same way, and the HUD's mark said again. `was` is the set
/// the ride had. Nothing is moved, stopped or put back.
fn remodify(
    seats: &mut [Seat],
    rigs: &mut [Rig],
    stock: &[Phy],
    plane: Option<&phy::Plane>,
    was: &game_sim::modifiers::Mods,
    to: &game_sim::modifiers::Mods,
) {
    for (i, seat) in seats.iter_mut().enumerate() {
        if let Some(file) = stock.get(i) {
            to.resim(was, i, file, &mut seat.sim);
        }
        if let (Some(file), Some(flight)) = (plane, seat.garage.plane.as_mut().and_then(|p| p.flight.as_mut())) {
            to.replane(i, file, flight);
        }
    }
    for (i, rig) in rigs.iter_mut().enumerate() {
        rig.modified = !to.seat_is_original(i);
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
        hud: false,
        overlay: None,
        speedometer: None,
        minimap: None,
        hud_scale: None,
        camera: START_CAMERA.to_string(),
        camera_view: None,
        trace_camera: false,
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
        aspect_asked: false,
        plants: None,
        headless: false,
        frames: None,
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
        mods: None,
        mods_asked: false,
        tricks: None,
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
            "--mods" => {
                let v = next("a mods folder, or off")?;
                args.mods_asked = true;
                args.mods = (v != "off").then(|| std::path::PathBuf::from(v));
            }
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
            "--shot-pose" => args.shot_pose = Some(shot_pose::Pose::parse(&next("x,y,z,yaw,pitch,fov")?)?),
            "--hud" => {
                args.hud = true;
                args.overlay = Some(menu::Overlay::Debug);
            }
            "--overlay" => {
                let word = next("hud, original, debug or none")?;
                let overlay = menu::Overlay::from_word(&word)
                    .filter(|o| o.word() == word)
                    .ok_or_else(|| format!("--overlay: {word:?} is not hud, original, debug or none"))?;
                args.hud = overlay == menu::Overlay::Debug;
                args.overlay = Some(overlay);
            }
            "--tricks" => {
                let word = next("original or mxvatv")?;
                let tricks = menu::Tricks::from_word(&word).ok_or_else(|| format!("--tricks: {word:?} is not original or mxvatv"))?;
                args.tricks = Some(tricks);
            }
            "--speedometer" => {
                let word = next("digital or analog")?;
                let style = menu::Speedometer::from_word(&word)
                    .ok_or_else(|| format!("--speedometer: {word:?} is not digital or analog"))?;
                args.speedometer = Some(style);
            }
            "--minimap" => {
                args.minimap = Some(match next("on or off")?.as_str() {
                    "on" => true,
                    "off" => false,
                    word => return Err(format!("--minimap: {word:?} is not on or off")),
                });
            }
            "--hud-scale" => {
                let word = next("a HUD scale from 50 to 150 in tens")?;
                let n = menu::hud_scale_word(&word).ok_or_else(|| format!("--hud-scale: {word:?} is not 50 to 150 in steps of 10"))?;
                args.hud_scale = Some(n);
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
            "--trace-camera" => args.trace_camera = true,
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
            "--aspect" => (args.aspect, args.aspect_asked) = (Aspect::parse(&next("fill or 4:3")?)?, true),
            "--plants" => {
                let word = next("smooth or original")?;
                args.plants = Some(game_render::eco::Lod::from_word(&word).ok_or_else(|| format!("--plants: {word:?} is not smooth or original"))?);
            }
            "--headless" => args.headless = true,
            "--no-sound" => args.sound = false,
            "--timings" => args.timings = true,
            "--trace-picture" => args.trace_picture = true,
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
                            [--moment FILE OUT.png] [--no-wrecks] [--overlay hud|original|debug|none] [--speedometer digital|analog] [--tricks original|mxvatv] [--hud] [--camera NAME]\n\
                            [--minimap on|off] [--hud-scale 50..150]\n\
                            [--camera-view original|default|FOV,DIST,HEIGHT,PDIST,PHEIGHT]\n\
                            [--modifiers 'gravity=50 bike_speed.2=200']\n\
                            [--shot-frame N] [--shot-pose x,y,z,yaw,pitch,fov] [--vehicle bike|biplane]\n\
                            [--players 1..8] [--split h|v] [--headless] [--frames N] [--timings]\n\
                            [--draw-fps N] [--vsync on|off] [--fps-limit off|N|refresh]\n\
                            [--record TAPE] [--play TAPE|RECORDING.p2m2 [--phase second]]\n\
                            [--throttle cross|r2] [--pad-pressure ds2@8|AXIS:CONTROL,...]\n\
                            [--pads k,0,1,-] [--controls new|original|new,original,...]\n\
                            [--biplane-controls new|screen|original|new,original,...]\n\
                            [--preset 0..3|0,2,...]\n\
                            [--bindings FILE] [--renderer gpu|software] [--graphics full|original] [--fps]\n\
                            [--render-scale auto|0.5..1]\n\
                            [--no-sound] [--mods DIR|off]\n\
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
                     zooms), R3 puts it back, Start freezes, Escape quits. --camera starts on a camera preset by its\n\
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
                     parks one, and M switches between the bike and the plane, spawning the\n\
                     one taken where you are and putting the other away. The plane flies over the terrain\n\
                     alone: Cross opens the throttle and Square closes it.\n\
                     --players N splits the screen between N local players (up to 8), each on\n\
                     a pad of their own with a camera and an overlay of their own: two one above\n\
                     the other (--split h, the default) or side by side (--split v), the\n\
                     original's two layouts, and three to eight in quarters or eighths. The first\n\
                     player has the keyboard, the plane and the tape. --headless draws with no\n\
                     window and no pad, and --frames N quits after N frames. --timings prints\n\
                     what each stage of a frame cost, a line a second, and --trace-picture\n\
                     where each drawn frame put the first player's bike, rider and camera.\n\
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
                     --vsync off draws without waiting for the display (it may tear), and\n\
                     --fps-limit draws at most N pictures a second, or the display's refresh;\n\
                     a limit under the refresh holds with VSync on too. Without them the\n\
                     Settings rows say (VSync On, Frame limit Off). The ride's 30 Hz ticks are\n\
                     the same at any of them; --draw-fps, a test's fixed rate, wins over both.\n\
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
                     --trace-camera prints each view's camera a frame (preset, distance, pitch,\n\
                     field of view, eye and look-at) beside a ride;\n\
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
                     Track mods: a --track not on the disc is looked for in mods/tracks/<id>/\n\
                     beside the settings (openmx-mods' standard), or in --mods DIR; --mods off,\n\
                     or Settings' Mods Off, reads nothing there. A mod rides as a freeride.\n\
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
