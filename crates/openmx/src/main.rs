//! **openmx** — the playable shell: a window, a real pad, and the freeride area to ride it in.
//!
//! ```sh
//! cargo run --release -p openmx                    # ride
//! cargo run --release -p openmx -- --seconds 60    # ride for a minute and quit (the task's own test)
//! cargo run --release -p openmx -- --pad /dev/input/js1 --size 512x384 --scale 2
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
//! **L3 and R3 clicked together** put the bike back on the start line, which is how a rider who is
//! stuck gets going again. A wreck no longer needs it: the bike tumbles, the rider comes off it and
//! the ride comes back up on its own, on the wreck's own two files (S26). Start freezes and
//! unfreezes the ride, L1 cycles the track's own camera presets, and Escape
//! or closing the window quits. The same controls are on the keyboard for a machine with no pad:
//! X/Space, Z, the arrow keys, C/V/D for the three trick buttons, Q+E for the reset, Return.
//!
//! Which host button is which is this file's and [`game_sim::ride::Controls::from_pad`]'s own choice
//! and not the disc's — the original's binding table is in its front end and is not ported. What *is*
//! the disc's is the order of the three trick buttons, because that order is what numbers the six
//! groups and so which stunt a gesture names.

#![forbid(unsafe_code)]

mod ground;
mod moment;
mod pad;
mod window;

use std::time::{Duration, Instant};

use game_render::disc::{Disc, Res};
use game_render::mat::{M4, V3, v3};
use game_render::raster::Frame;
use game_render::rider::{Body, Sex};
use game_render::scene::Vehicle;
use game_render::vehicle::{Chassis, Drive, params as vehicle_params};
use game_render::world::{Assets, Camera, Lighting, draw_scene, vertical_fov};
use game_render::{ini, png, scene as view};
use game_sim::camera::{Preset, Race, Ridden, Switches, Target};
use game_sim::input::{Control, Manager, PADS, Report};
use game_sim::phy::{self, Phy};
use game_sim::ride::{self, Controls};
use game_sim::rider::{self, Rider};
use game_sim::rng::Rng;
use game_sim::state::Phase;
use game_sim::tick::{Engine, Game, Outcome, Timer0};
use game_sim::trick::{Set, Stunts};
use game_sim::world::Place;
use game_sim::{Input, track, world as sim_world};
use ps2_float::{F, Vf};

use ground::Ground;
use moment::{Moment, Shot};
use pad::Pads;
use window::{Window, key};

/// The game's own frame rate. The simulation is not fixed to it — it is handed the wall clock's own
/// delta — but this is the rate the loop aims at, and the rate the original's video mode flips at.
const FRAME: Duration = Duration::from_nanos(1_000_000_000 / 60);

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
    /// The port's input manager, ticked with the frame's own `dt` at the top of the pass.
    input: Manager,
    /// This frame's reports, written by the app layer before the step the way the original's device
    /// manager fills its buffers before the frame reads them.
    reports: [Report; PADS],
    /// The bike's own parameters, as its `.phy` carries them, and the three tables the step reads.
    phy: Phy,
    tables: rider::Tables,
    /// The bike and the person on it: P3's rider, which owns P2's ride and steps it.
    rider: Rider,
    /// Whether the ride is moving. Start toggles it: the original's pause is a whole front-end page
    /// and this shell has no pages, so freezing the ride is the shell's own stand-in for it.
    riding: bool,
    /// What the rider did this frame, kept so the loop can print it after the pass.
    said: rider::Frame,
}

impl Game for World<'_> {
    /// One simulation pass: the pad, then the bike.
    ///
    /// `input` is the engine's own sampled pad, which this shell does not use — the full 32-byte
    /// reports are in `self.reports` and the port's manager is what reads them. Nothing here draws
    /// from `rng`: the only thing that moves is the bike, and the rider decides where it goes.
    fn simulate(&mut self, dt: F, _input: Input, _rng: &mut Rng) -> bool {
        // The game's poll, with the frame's dt: this is what turns report bytes into held controls,
        // press and release edges, and stick axes with the game's own dead zone.
        self.input.tick(dt, &self.reports);
        let pad = self.input.pad(0);

        if pad.pressed(Control::Start) {
            self.riding = !self.riding;
        }

        // Which button means what lives in the ride, so the shell and a replay fixture read a pad
        // the same way.
        let controls = Controls::from_pad(pad);
        self.said = rider::Frame::default();
        if self.riding {
            self.said = self.rider.frame(&self.phy, &self.tables, &controls, dt, &mut self.place);
        }
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
    width: usize,
    height: usize,
    scale: usize,
    /// Quit after this long, which is how the acceptance run is made to end by itself.
    seconds: Option<f32>,
    pad: Option<String>,
    display: String,
    /// Write the first frame here and keep riding — how an unattended run proves what it drew.
    shot: Option<std::path::PathBuf>,
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
    let phy_bytes = archives[1]
        .read_suffix(&format!("{}.phy", args.bike))
        .or_else(|_| archives[1].read_suffix(".phy"))
        .map_err(|e| format!("the bike archive has no .phy: {e}"))?;
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
        let slots = chassis.lib.clip[..game_sim::trick::SLOTS].iter().filter(|c| c.is_some()).count();
        let poses = chassis.lib.clip[game_sim::trick::SLOTS..].iter().filter(|c| c.is_some()).count();
        println!(
            "chassis: {} -> {}.psg, {} clips bound, {slots} of {} gesture slots, {poses} of {}              pose clips, nodes {:?}",
            vehicle.mcf,
            chassis.geometry,
            chassis.set.clips.len(),
            game_sim::trick::SLOTS,
            game_sim::vehicle_anim::POSE_CLIPS.len(),
            chassis.nodes,
        );
    }

    // The follow cameras the track carries, in the file's own order, read by S7's own reader. L1 and
    // R1 walk them. `reverse_speed` is a constructor argument in the original and the call site was
    // not read; the preset's own switch speed is the closest number the disc has.
    let presets: Vec<Preset> = archives
        .iter()
        .find_map(|a| a.read_suffix("cpresets.ini").ok())
        .map(|b| Preset::read_bytes(&b, ""))
        .unwrap_or_default();
    let reverse_speed =
        presets.first().map_or(F(5.0), |p| p.forward_reverse_switch_speed);
    // The derived class, not the base one: the base camera's frame is dead code on the disc (S24),
    // and this is the frame the picture comes out of.
    let mut camera = Race::new(presets, reverse_speed);
    // The first frame starts the eight lags where the camera actually is instead of lagging them in
    // from zero, which is what the camera's own snap flag is for. The disc gets it from the race's
    // setup applying a preset over a previous one; the constructor here applies its first preset to
    // itself, and that comparison cannot fire.
    camera.cam.snap = true;
    let mut which = 0usize;

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
    rider.sense.wrecks_enabled = args.wrecks;
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
        let shot = Shot {
            width: args.width,
            height: args.height,
            ground_radius: GROUND_RADIUS,
            chassis: CHASSIS,
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
        let controls = if args.trace_brake {
            Controls { brake: F::ONE, front_brake: F::ONE, rear_brake: F::ONE, ..Controls::default() }
        } else {
            Controls::default()
        };
        println!(
            "trace: {frames} frames at dt {} on {} / {}",
            dt.0, args.track, args.bike
        );
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

    // ---- the window and the pad ---------------------------------------------------------------
    let mut window = Window::open(
        &args.display,
        args.width,
        args.height,
        args.scale,
        "openmx — freeride",
    )?;
    let mut pads = Pads::open(args.pad.as_deref());
    match &pads.name {
        Some(name) => println!("pad: {name}"),
        None => println!("pad: none found — riding on the keyboard (X throttle, Z brake, arrows)"),
    }

    // ---- the loop ------------------------------------------------------------------------------
    let light = Lighting::of(&look);
    let mut frame = Frame::new(args.width, args.height, look.fog_color);
    let mut engine = Engine::new();
    let mut world = World {
        place: sim,
        input: Manager::connected(),
        reports: [Report::missing(); PADS],
        phy,
        tables,
        rider,
        riding: true,
        said: rider::Frame::default(),
    };

    let mut shot = args.shot.clone();
    let started = Instant::now();
    let mut window_of_frames = (Instant::now(), 0u32, 0usize);
    loop {
        let top = Instant::now();

        window.poll();
        if window.closed() || window.held(key::ESCAPE) {
            break;
        }
        world.reports = [pads.report(&window), Report::missing()];

        // One frame of the game's own fixed update, which calls `World::simulate` inside itself.
        let step = engine.step(wall(started), engine_input(&world.reports[0]), &mut world);
        if step.outcome == Outcome::Quit {
            break;
        }

        // The body on the bike, posed from the frame the simulation's own animation driving just
        // answered: the clip it asked for, at the phase its clock reached, faded by its ramp. A
        // frozen ride does not step the driver, so the pose is left where it was.
        if let Some(body) = &mut body {
            if world.riding {
                body.pose(step.dt, world.rider.animation);
                if let Some(loaded) = assets.models.get_mut(body.geometry()) {
                    body.skin_into(loaded);
                }
            }
        }

        // And the bike under it (V9). Two halves in one call: the chassis skeleton posed from the
        // vehicle's own animation machine, and then the suspension, steering and wheel nodes written
        // over that pose from the ride's own numbers — the fork travel, the steering angle and the two
        // wheels' spin, which are not in any clip because no clip could have them.
        if let Some(chassis) = &mut chassis {
            if world.riding {
                let ride = &world.rider.ride;
                let want = Chassis::want(ride, &world.rider.tricks);
                let request = Chassis::request(ride, &world.rider.tricks);
                chassis.frame(step.dt, want, &request, Drive::of(ride));
                if let Some(loaded) = assets.models.get_mut(CHASSIS) {
                    chassis.skin_into(loaded);
                }
            }
        }

        // What the rider did, as a line: a run nobody is watching still says whether a trick
        // started, landed or put the rider down, and by which of the original's reason codes.
        if let Some(t) = world.said.started {
            println!("trick: {}", table.display_name(0, t));
        }
        if let Some(t) = world.said.landed {
            println!("landed: {}", table.display_name(0, t));
        }
        if let Some(why) = world.said.wreck {
            println!("wreck: reason {} — L3 and R3 together to get back on", why.0);
        }
        if world.said.respawned {
            println!("back on the start line");
        }

        // The camera, after the frame, which is where the original's viewport updates it: the shell
        // picks the preset, the ported camera does everything else.
        let presets = camera.cam.presets.len();
        if presets > 1 {
            let pad = world.input.pad(0);
            // L1 only: R1 is the third of the trick decoder's buttons now.
            if pad.pressed(Control::L1) {
                which = (which + 1) % presets;
                camera.cam.set_preset(which);
            }
        }
        let (forward, up) = world.rider.ride.axes();
        let bike_pos = world.rider.ride.position();
        // The grounded flag and the speed are read off the actor by the frame rather than worked
        // out from the velocity; the ride's own two fields are what the actor's are written from.
        let rider = Ridden {
            target: Target {
                body_point: bike_pos,
                forward,
                up,
                vel: world.rider.ride.chassis.vel,
                lean: world.rider.ride.held.stick.0,
            },
            grounded: world.rider.ride.air.on_ground,
            speed: world.rider.ride.chassis.speed,
        };
        let view = if presets == 0 {
            None
        } else {
            // The camera's one question about the world: the ground under the look-at point, which
            // the track answers (S23). Without it the view sinks into the dirt on the first dip.
            // This frame has no obstacle push-out — the base class's did, and the base class's
            // frame is not the one the disc runs.
            Some(camera.update(
                step.dt,
                &rider,
                F(NEAR_CLIP),
                &world.tables.ride.curve,
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
        let bike_look = vector(forward);
        let bike_up = vector(up);
        let pos = vector(bike_pos);
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
        let bike_at = M4::placement(pos, bike_look, bike_up);
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
        // And the rider, at the bike's own transform: the clips put the body on the bike
        // themselves, so there is nothing to bolt (`game_render::rider`). Not before a frame has
        // posed it, though: a blend with no clip behind it leaves the bind pose, which is the one
        // pose the game never draws.
        if let Some(body) = &body {
            if body.posed {
                actor.push((body.geometry(), bike_at));
            }
        }
        drawn += draw_scene(&mut frame, &look, &assets, &actor, &camera);
        window.present(&frame.color, frame.width, frame.height)?;

        // The first frame, on paper, for a run nobody is watching.
        if let Some(path) = shot.take() {
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
                world.rider.ride.chassis.speed.0 * 0.681_818,
                match (world.riding, world.rider.phase()) {
                    (false, _) => " (frozen)",
                    (_, Phase::Riding) => "",
                    (_, Phase::Down) => " (down)",
                    (_, Phase::Resetting) => " (getting up)",
                }
            );
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

/// The command line. `Ok(None)` means `--help` was asked for and the shell should not start.
fn parse_args() -> Result<Option<Args>, String> {
    let mut args = Args {
        // The freeride area D6 read and proved: no gates, no laps, nobody else on the track.
        track: "free06".to_string(),
        bike: "125sx".to_string(),
        // The stock gear. Every other rider archive on the disc is a repaint of the same body.
        rider: "thq1".to_string(),
        width: 640,
        height: 448,
        scale: 1,
        seconds: None,
        pad: None,
        shot: None,
        moment: None,
        wrecks: true,
        trace: None,
        trace_dt: None,
        trace_brake: false,
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
            "--moment" => {
                let file: std::path::PathBuf = next("a moment file")?.into();
                let out: std::path::PathBuf = next("a path for the frame")?.into();
                args.moment = Some((file, out));
            }
            "--display" => args.display = next("a display, like :0")?,
            "--no-wrecks" => args.wrecks = false,
            "--trace-brake" => args.trace_brake = true,
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
                args.width = w.parse().map_err(|_| "--size: not a width")?;
                args.height = h.parse().map_err(|_| "--size: not a height")?;
                if args.width == 0 || args.height == 0 {
                    return Err("--size: a window has to have some pixels in it".into());
                }
            }
            "--help" | "-h" => {
                println!(
                    "openmx [--track free06] [--bike 125sx] [--rider thq1] [--size 640x448]\n\
                            [--scale 1]\n\
                            [--pad /dev/input/js0] [--display :0] [--seconds N] [--shot PATH]\n\
                            [--moment FILE OUT.png] [--no-wrecks]\n\
                     \n\
                     Rides the freeride area from the disc named in kit.toml.\n\
                     Cross throttle, Square brake, R2 front brake, L2 clutch, left stick steers\n\
                     and leans, the d-pad preloads, L3 balances, R3 reverses, Start freezes,\n\
                     L1 changes camera, Escape quits.\n\
                     A trick is a gesture: hold Circle, Triangle and/or R1 and push the d-pad.\n\
                     L3 and R3 clicked together put the bike back on the start line.\n\
                     On the keyboard: X throttle, Z brake, arrows steer, C/V/D trick, Q+E reset.\n\
                     --no-wrecks turns the session's wrecks off (a wreck otherwise throws the\n\
                     rider, tumbles the bike and puts you back on it); --moment draws one frame posed\n\
                     from a moment of the original and quits; --trace N rides N frames with\n\
                     nobody on the pad and prints the solver's own numbers."
                );
                return Ok(None);
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Some(args))
}
