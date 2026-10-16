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
//! * **The picture** is V1: the scene's props, the bike and the parts its `.sub` bolts on, lit and
//!   fogged by the scene's own values, through the port's rasteriser, with the follow camera S7 ported
//!   reading the track's own `cpresets.ini`.
//!
//! What is **not** a port, and is marked as such where it happens: the bilinear blend inside a terrain
//! cell ([`ground`]), the host-float arithmetic of the camera-to-rasteriser hand-off, and this file's
//! own choice of which pad control means what.
//!
//! # The controls
//!
//! Cross is the throttle and Square the brake (both analogue, as the pad reports them), L2 is the
//! clutch and R2 the front brake on its own, the left stick steers and leans, Circle holds a trick, the
//! d-pad preloads, L3 asks the rider to balance and R3 is reverse. Triangle puts the bike back on the
//! start line, Start freezes and unfreezes the ride, L1 and R1 cycle the track's own camera presets,
//! and Escape or closing the window quits. The same controls are on the keyboard for a machine with no
//! pad: X/Space, Z, the arrow keys, V, Return.

#![forbid(unsafe_code)]

mod ground;
mod moment;
mod pad;
mod window;

use std::time::{Duration, Instant};

use game_render::disc::{Disc, Res};
use game_render::mat::{M4, V3, v3};
use game_render::raster::Frame;
use game_render::scene::Vehicle;
use game_render::world::{Assets, Camera, Lighting, draw_scene};
use game_render::{ini, png, scene as view};
use game_sim::camera::{Follow, Preset, Target};
use game_sim::input::{Axis, Control, Manager, PADS, Report};
use game_sim::phy::Phy;
use game_sim::ride::{Controls, Ride, Tables};
use game_sim::rng::Rng;
use game_sim::tick::{Engine, Game, Outcome, Timer0};
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

/// The chassis model every bike is built on, which is the name the parts in a `.sub` hang off.
const CHASSIS: &str = "bike";

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
    /// The bike's own parameters, as its `.phy` carries them, and the two tables the step reads.
    phy: Phy,
    tables: Tables,
    ride: Ride,
    /// Whether the ride is moving. Start toggles it: the original's pause is a whole front-end page
    /// and this shell has no pages, so freezing the ride is the shell's own stand-in for it.
    riding: bool,
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

        // Which control means what is the shell's own choice (the original's binding table is the
        // front end's). Everything past this point is the port's.
        let held = |c: Control| if pad.held(c) { F::ONE } else { F::ZERO };
        let dpad = [Control::Up, Control::Right, Control::Down, Control::Left];
        let controls = Controls {
            gas: pad.strength(Control::Cross),
            brake: pad.strength(Control::Square),
            front_brake: pad.strength(Control::R2),
            rear_brake: F::ZERO,
            clutch: pad.strength(Control::L2),
            reverse: held(Control::R3),
            stick: (pad.axis(Axis::LeftX), pad.axis(Axis::LeftY)),
            dpad_y: pad.held(Control::Up) || pad.held(Control::Down),
            // Zero for nothing and a direction otherwise, which is all the air step reads of it; the
            // eight-way code the original builds out of the pad is the front end's.
            dpad_dir: dpad.iter().position(|c| pad.held(*c)).map_or(0, |i| i as i32 * 2 + 1),
            stunt: pad.held(Control::Circle),
            balance: pad.held(Control::L3),
            respawn: pad.pressed(Control::Triangle),
        };
        if self.riding {
            self.ride.frame(&self.phy, &self.tables, &controls, dt, &mut self.place);
        }
        true
    }
}

/// What the shell was asked to do.
struct Args {
    track: String,
    bike: String,
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
    let mut archives: Vec<Res> = vec![disc.res(&track_member)?, disc.res(&bike_member)?];
    if let Some(name) = &second {
        archives.push(disc.res(name)?);
    }
    let archives = archives;

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
    let tables = Tables::build();

    // The bike's own physics, off the disc, by the ported loader (S18) — every number the ride runs
    // on comes from here, and a bike whose file is missing is not rideable.
    let phy_bytes = archives[1]
        .read_suffix(&format!("{}.phy", args.bike))
        .or_else(|_| archives[1].read_suffix(".phy"))
        .map_err(|e| format!("the bike archive has no .phy: {e}"))?;
    let phy = Phy::read_bytes(&phy_bytes, &tables.curve);

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
    let mut camera = Follow::new(presets, reverse_speed);
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
    // Where the wheels hang from the chassis: the `.phy`'s own force positions, which is what the
    // suspension's ray starts from.
    let mounts = (phy.front.body_tire_force_position, phy.rear.body_tire_force_position);
    let ride = Ride::new(&phy, mounts, at, heading);
    println!(
        "start: {:.0}, {:.0}, {:.0} facing {:.0} degrees, {} camera presets",
        spawn.x,
        spawn.y,
        spawn.z,
        facing.x.atan2(facing.z).to_degrees(),
        camera.presets.len()
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
        ride,
        riding: true,
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

        // The camera, after the frame, which is where the original's viewport updates it: the shell
        // picks the preset, S7's own camera does everything else.
        let presets = camera.presets.len();
        if presets > 1 {
            let pad = world.input.pad(0);
            if pad.pressed(Control::R1) {
                which = (which + 1) % presets;
                camera.set_preset(which);
            }
            if pad.pressed(Control::L1) {
                which = (which + presets - 1) % presets;
                camera.set_preset(which);
            }
        }
        let (forward, up) = world.ride.axes();
        let bike_pos = world.ride.position();
        let target = Target {
            body_point: bike_pos,
            forward,
            up,
            vel: world.ride.chassis.vel,
            lean: world.ride.held.stick.0,
        };
        let view = if presets == 0 {
            None
        } else {
            // Nothing limits the camera's distance here: the original's limit comes from a collision
            // probe the camera owns, which is not ported, and zero is what that probe answers in the
            // open.
            Some(camera.update(step.dt, &target, F::ZERO, &world.tables.curve, &world.tables.acos))
        };
        let bike_look = vector(forward);
        let bike_up = vector(up);
        let pos = vector(bike_pos);
        let camera = match &view {
            Some(view) => Camera {
                eye: vector(view.position),
                at: vector(view.look_at),
                // The file's field of view is the horizontal one at 4:3; the rasteriser wants the
                // vertical one in radians.
                fov: vertical_fov(view.fov.0),
                near: 0.05,
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
        if let Some(chassis) = assets.models.get(CHASSIS) {
            for part in &vehicle.parts {
                let node = chassis
                    .model
                    .nodes
                    .iter()
                    .position(|n| n.name.eq_ignore_ascii_case(&part.placement));
                let at = match node {
                    Some(i) => chassis.model.world(i).mul(bike_at),
                    None => bike_at,
                };
                actor.push((part.model.as_str(), at));
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
                world.ride.chassis.speed.0 * 0.681_818,
                if world.riding { "" } else { " (frozen)" }
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

/// The disc's field of view — horizontal, in degrees, at 4:3 — as the vertical one in radians that
/// [`Camera`] and the rasteriser's projection want.
fn vertical_fov(horizontal_degrees: f32) -> f32 {
    let horizontal = horizontal_degrees.to_radians();
    2.0 * (horizontal / 2.0).tan().atan2(4.0 / 3.0)
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
        width: 640,
        height: 448,
        scale: 1,
        seconds: None,
        pad: None,
        shot: None,
        moment: None,
        display: std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string()),
    };
    let mut argv = std::env::args().skip(1);
    while let Some(a) = argv.next() {
        let mut next = |what: &str| argv.next().ok_or(format!("{a} wants {what}"));
        match a.as_str() {
            "--track" => args.track = next("a track name, like free06")?,
            "--bike" => args.bike = next("a bike name, like 125sx")?,
            "--pad" => args.pad = Some(next("a device, like /dev/input/js0")?),
            "--shot" => args.shot = Some(next("a path for the first frame")?.into()),
            "--moment" => {
                let file: std::path::PathBuf = next("a moment file")?.into();
                let out: std::path::PathBuf = next("a path for the frame")?.into();
                args.moment = Some((file, out));
            }
            "--display" => args.display = next("a display, like :0")?,
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
                    "openmx [--track free06] [--bike 125sx] [--size 640x448] [--scale 1]\n\
                            [--pad /dev/input/js0] [--display :0] [--seconds N] [--shot PATH]\n\
                            [--moment FILE OUT.png]\n\
                     \n\
                     Rides the freeride area from the disc named in kit.toml.\n\
                     Cross throttle, Square brake, R2 front brake, L2 clutch, left stick steers\n\
                     and leans, Circle holds a trick, the d-pad preloads, L3 balances, R3 reverses,\n\
                     Triangle respawns, Start freezes, L1/R1 change camera, Escape quits.\n\
                     On the keyboard: X throttle, Z brake, arrows steer, V respawn, Return start.\n\
                     --moment draws one frame posed from a moment of the original and quits."
                );
                return Ok(None);
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Some(args))
}
