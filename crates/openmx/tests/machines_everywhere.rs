//! P113 — every machine the port rides, for every seat, on every map.
//!
//! A track's `machine.txt` only parks copies; it never decides whether a player can have a machine.
//! On a race track (nat19, whose machines' archive has no `machine.txt` at all) and on a map with no
//! file and nothing but a floor under it (what a generated mod map is to the garage), every seat of a
//! four-player split opens its own pause, picks the biplane, and gets its own copy spawned where it
//! is; then picks the bike back, stood under its plane. A seat starting the ride in the plane starts
//! beside the start line, one a seat, on its wheels. The online lane does the same switch on the race
//! track's ground. Parked copies a file does give stay where it puts them.
//!
//! The shell's half — `--vehicle biplane` on a race track and on a mod map, a tape's switches there,
//! and two online machines starting their seats in the plane on a race track — rides the binary in
//! `tests/switch_tape.rs`-style tests below and in `tests/track_mods.rs`.
//!
//! Everything here needs the disc's `biplane.phy` and skips without it, saying so.

// the browser's own clock, threads and files, which are std's here (P196)
#[allow(unused)]
#[path = "../src/clock.rs"]
mod clock;
#[allow(unused)]
#[path = "../src/thread.rs"]
mod thread;
#[allow(unused)]
#[path = "../src/files.rs"]
mod files;
#[path = "../src/menu.rs"]
#[allow(dead_code)]
mod menu;
#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;
#[path = "../src/roster.rs"]
#[allow(dead_code)]
mod roster;
#[path = "../src/dress.rs"]
#[allow(dead_code)]
mod dress;
#[path = "../src/join.rs"]
#[allow(dead_code)]
mod join;
#[path = "../src/split.rs"]
#[allow(dead_code)]
mod split;
#[allow(dead_code)]
#[path = "../src/window.rs"]
mod window;
#[allow(dead_code)]
#[path = "../src/pad.rs"]
mod pad;
#[allow(dead_code)]
#[path = "../src/audio.rs"]
mod audio;

use game_render::Disc;
use game_sim::Sim;
use game_sim::archive::Res;
use game_sim::flight::Flight;
use game_sim::input::{Control, Report};
use game_sim::markers::Spot;
use game_sim::track::{Scene, Terrain};
use game_sim::world::{Flat, Ground, World};
use menu::{Act, Controls, Machine, Menu, Press, Reader, Screen, Settings};
use ps2_float::{F, Vf};
use std::path::Path;
use std::process::Command;
use vehicle::{Garage, Kind, Lot, Plane, Spawn};

const SEATS: usize = 4;
const DT: F = F(1.0 / 60.0);

fn disc() -> Option<Disc> {
    match Disc::open_configured() {
        Ok(d) => Some(d),
        Err(why) => {
            eprintln!("skipped: {why}");
            None
        }
    }
}

/// The disc's biplane as the shell builds it, before any modifier.
fn flight(disc: &mut Disc) -> Flight {
    let name = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("machine.res")).expect("machine.res").name.clone();
    let res = disc.res(&name).expect("machine.res");
    assert!(res.find_suffix("tracks\\nat19\\machine.txt").is_none(), "nat19 was meant to be a track with no machine.txt");
    let bytes = res.read_suffix(game_sim::phy::BIPLANE_PHY).expect("machine.res carries biplane.phy");
    Flight::new(&game_sim::phy::Plane::read_bytes(&bytes))
}

/// A track's scene and terrain bytes, out of its own archive.
fn terrain(disc: &mut Disc, track: &str) -> (Vec<u8>, Scene) {
    let res_bytes = disc.member(&format!("Tracks\\{track}.res")).expect("the track's archive");
    let res = Res::parse(&res_bytes).expect("the track's archive parses");
    let scene_entry = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(".scn")).expect("a scene");
    let scene = Scene::parse(&res.read(&res_bytes, scene_entry).expect("the scene")).expect("the scene parses");
    let leaf = scene.terrain_file.to_ascii_lowercase().replace(".trn", ".ps2.trn");
    let te = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with(&leaf)).expect("the terrain");
    (res.read(&res_bytes, te).expect("the terrain bytes"), scene)
}

/// A scene's start line and its heading, flattened.
fn scene_start(scene: &Scene) -> Spawn {
    let ([x, y, z], [dx, _, dz]) = (scene.start.map(|v| v.0), scene.start_direction.map(|v| v.0));
    let len = dx.hypot(dz);
    Spawn { at: Vf::new(x, y, z, 1.0), facing: Vf::new(dx / len, 0.0, dz / len, 0.0) }
}

/// The ground's height at `at`'s x and z, from high above, as the shell measures its start spots.
fn ground_at(world: &mut dyn World, at: Vf) -> f32 {
    world.ground(Vf::new(at.x(), 1.0e4, at.z(), 1.0)).0.y()
}

/// A menu riding, as the shell's is once the ride is under way, told each seat's machine.
fn riding_menu(garages: &[Garage]) -> (Menu, Vec<Reader>) {
    let mut reader = Reader::new();
    let mut m = Menu::new(Settings::default(), Controls::default());
    m.step(reader.read(&Report::idle()));
    m.riders = garages.len();
    for _ in 0..2 {
        // Up for three frames between taps: a re-press sooner is a bounce (P109).
        for report in [Report::from_buttons(Control::Cross.mask()), Report::idle(), Report::idle(), Report::idle()] {
            m.step(reader.read(&report));
        }
    }
    assert!(m.riding());
    for (i, g) in garages.iter().enumerate() {
        m.up[i] = Machine::from_word(g.up.name()).unwrap();
        m.switchable[i] = g.machines().len() > 1;
    }
    let mut readers = vec![reader];
    readers.extend((1..garages.len()).map(|_| Reader::new()));
    (m, readers)
}

/// One press and release of `control` on `seat`'s pad, every seat's press through the menu and a
/// switch answered by that seat's garage over `world`, as main.rs's ride loop does.
fn tap_seat(
    m: &mut Menu,
    readers: &mut [Reader],
    garages: &mut [Garage],
    sims: &mut [Sim],
    world: &mut dyn World,
    seat: usize,
    control: Control,
) -> Vec<Act> {
    let mut acts = Vec::new();
    // A press, then three frames up so the next tap is not a bounce (P109).
    for pressed in [true, false, false, false] {
        let presses: Vec<Press> = readers
            .iter_mut()
            .enumerate()
            .map(|(i, r)| r.read(&if pressed && i == seat { Report::from_buttons(control.mask()) } else { Report::idle() }))
            .collect();
        let act = m.step_seats(&presses);
        if let Act::Switch(i, to) = act {
            let up = garages[i].switch(Kind::parse(to.word()).unwrap(), &mut sims[i], world).unwrap();
            m.switched(i, Machine::from_word(up.name()).unwrap());
        }
        acts.push(act);
    }
    acts
}

/// `seat`'s pause, Down twice to the switch row, which offers `offered`, and Cross on it.
#[allow(clippy::too_many_arguments)]
fn switch_from_the_pause(
    m: &mut Menu,
    readers: &mut [Reader],
    garages: &mut [Garage],
    sims: &mut [Sim],
    world: &mut dyn World,
    seat: usize,
    offered: Machine,
) {
    tap_seat(m, readers, garages, sims, world, seat, Control::Start);
    // the seat's own pause (P84), in its own view
    assert_eq!(m.view(seat).screen(), Some(Screen::Pause));
    assert_eq!(m.view(seat).pause_rows()[2], format!("Switch To Machine: {}", offered.label()));
    tap_seat(m, readers, garages, sims, world, seat, Control::Down);
    tap_seat(m, readers, garages, sims, world, seat, Control::Down);
    let acts = tap_seat(m, readers, garages, sims, world, seat, Control::Cross);
    assert!(acts.contains(&Act::Switch(seat, offered)), "seat {seat}: {acts:?}");
    assert!(m.riding());
}

/// The four seats of a split on a map whose garage parks no plane (`lot`), each stood somewhere of
/// its own off `start`, switch to the plane from their own pause and back; every plane lands on its
/// own player and flies a second where it landed, and every bike stands under its own plane.
fn four_seats_switch_and_back(map: &str, world: &mut dyn World, start: Spawn, lot: &Lot, template: &Flight) {
    let rest = Plane::flying(start, template.clone()).rest_height(0.0);
    let starts = {
        let mut ground = |at: Vf| ground_at(world, at);
        vehicle::start_spots(start, vehicle::Machine::Biplane, SEATS, rest, &mut ground)
    };
    let mut garages: Vec<Garage> = (0..SEATS)
        .map(|i| {
            assert!(lot.bay(vehicle::Machine::Biplane, i).is_none(), "{map}: seat {i} has a parked copy");
            Garage::new(Kind::Bike, lot.plane_anywhere(i, Some(template.clone()), starts[i])).unwrap()
        })
        .collect();
    // Each seat somewhere of its own: across the start and along it, on the ground there.
    let (fx, fz) = (start.facing.x(), start.facing.z());
    let mut sims: Vec<Sim> = (0..SEATS)
        .map(|i| {
            let (x, z) = (start.at.x() - fz * 70.0 * i as f32 + fx * 45.0 * i as f32, start.at.z() + fx * 70.0 * i as f32 + fz * 45.0 * i as f32);
            let y = ground_at(world, Vf::new(x, 0.0, z, 1.0));
            let mut sim = Sim::new(0);
            sim.rider.teleport_to(Spot { at: Vf::new(x, y, z, 1.0), forward: start.facing, up: Vf::new(0.0, 1.0, 0.0, 0.0), motion: None, flight: None });
            sim
        })
        .collect();
    let (mut m, mut readers) = riding_menu(&garages);
    assert!(m.switchable[..SEATS].iter().all(|&s| s), "{map}: a seat has no plane to switch to");

    for seat in 0..SEATS {
        let bike = sims[seat].rider.ride.position();
        switch_from_the_pause(&mut m, &mut readers, &mut garages, &mut sims, world, seat, Machine::Biplane);
        assert_eq!(garages[seat].up, Kind::Biplane, "{map}: seat {seat}");
        let p = garages[seat].plane_flown().expect("the plane is up");
        let under = ground_at(world, p.at);
        assert_eq!((p.at.x(), p.at.z()), (bike.x(), bike.z()), "{map}: seat {seat}'s plane is not on its player");
        assert!((p.at.y() - (under + rest)).abs() < 1e-3, "{map}: seat {seat}'s plane is not on its wheels: {} over {under}", p.at.y());
        assert_eq!(p.livery, vehicle::livery(seat));
        // The others are where they were: on their bikes.
        for other in (0..SEATS).filter(|&o| o > seat) {
            assert_eq!(garages[other].up, Kind::Bike, "{map}: seat {other} moved with seat {seat}");
        }
    }
    // Each plane stays on its wheels where it landed: a second at idle, rolling at most a little
    // down whatever slope it is on, neither sunk into the ground nor dropped onto it from a height.
    for seat in 0..SEATS {
        let landed = garages[seat].plane_flown().unwrap().at;
        for _ in 0..60 {
            let Garage { plane, .. } = &mut garages[seat];
            plane.as_mut().unwrap().step(&sims[seat], DT, world);
        }
        let now = garages[seat].plane_flown().unwrap().at;
        let rolled = (now.x() - landed.x()).hypot(now.z() - landed.z());
        let over = now.y() - ground_at(world, now);
        assert!(rolled < 10.0, "{map}: seat {seat}'s idling plane went {rolled} ft in a second");
        // Level on a slope its origin stands higher over the dirt straight under it than on the
        // flat (nat19's seat 2 lands on a jump face 45 degrees steep: 1.9 ft).
        assert!(over > -0.5 && over < rest + 2.5, "{map}: seat {seat}'s plane is {over} ft over the ground, its wheels {rest}");
    }
    for seat in 0..SEATS {
        let plane = garages[seat].plane_flown().unwrap().at;
        switch_from_the_pause(&mut m, &mut readers, &mut garages, &mut sims, world, seat, Machine::Bike);
        assert_eq!(garages[seat].up, Kind::Bike);
        assert!(garages[seat].plane_flown().is_none(), "{map}: seat {seat}'s plane is still drawn");
        let bike = sims[seat].rider.ride.position();
        assert_eq!((bike.x(), bike.z()), (plane.x(), plane.z()), "{map}: seat {seat}'s bike is not under its plane");
    }
    println!("{map}: {SEATS} seats took their planes on their own spots and their bikes back under them");
}

#[test]
fn every_seat_switches_to_the_plane_and_back_on_a_race_track_and_a_bare_map() {
    let Some(mut disc) = disc() else { return };
    let template = flight(&mut disc);

    // nat19, a race track: its machines' archive has no machine.txt, so the shell's lot is empty.
    let (bytes, scene) = terrain(&mut disc, "nat19");
    let field = Terrain::parse(&bytes).expect("nat19's terrain");
    let mut ground = Ground::new(&field, &scene.terrain);
    let start = scene_start(&scene);
    four_seats_switch_and_back("nat19", &mut ground, start, &Lot::default(), &template);

    // A map with nothing but a floor and a machine.txt that parks other machines and no plane: a
    // mod map's garage, or a free track without a [Biplane<n>] section.
    let no_plane = "[Machines]\nNumberOfMachines = 1\n\n[Monster1]\nType = Monster\nPosition = 10, 0, 10\nDirection = 0, 0, 1\n";
    let mut floor = Flat::default();
    let lot = Lot::new(no_plane.as_bytes(), SEATS, &mut |_| 0.0);
    assert_eq!(lot.of(vehicle::Machine::Monster).count(), SEATS);
    let start = Spawn { at: Vf::new(512.0, 0.0, 300.0, 1.0), facing: Vf::new(0.0, 0.0, 1.0, 0.0) };
    four_seats_switch_and_back("a bare map", &mut floor, start, &lot, &template);
}

#[test]
fn a_ride_started_in_the_plane_puts_each_seat_beside_the_start() {
    let Some(mut disc) = disc() else { return };
    let template = flight(&mut disc);
    let (bytes, scene) = terrain(&mut disc, "nat19");
    let field = Terrain::parse(&bytes).expect("nat19's terrain");
    let mut world = Ground::new(&field, &scene.terrain);
    let start = scene_start(&scene);
    let s = start.at;
    let rest = Plane::flying(start, template.clone()).rest_height(0.0);
    assert!(rest > 0.0 && rest < 10.0, "the plane's wheels put its origin {rest} ft up");
    for seats in [1, 2, 4, 8] {
        let starts = vehicle::start_spots(start, vehicle::Machine::Biplane, seats, rest, &mut |at| ground_at(&mut world, at));
        assert_eq!(starts.len(), seats);
        assert_eq!((starts[0].at.x(), starts[0].at.z()), (s.x(), s.z()), "the first is on the start line");
        for (i, a) in starts.iter().enumerate() {
            let under = ground_at(&mut world, a.at);
            assert!((a.at.y() - (under + rest)).abs() < 1e-3, "{seats} seats: copy {i} is not on its wheels");
            assert_eq!(a.facing.y(), 0.0, "a start spot faces level");
            for b in &starts[..i] {
                let apart = (a.at.x() - b.at.x()).hypot(a.at.z() - b.at.z());
                assert!(apart >= vehicle::Machine::Biplane.spacing() - 0.01, "{seats} seats: copies {apart} ft apart");
            }
            // A seat whose setup picked the plane starts in it there; online the same spot.
            let g = Garage::new(Kind::Biplane, Lot::default().plane_anywhere(i, Some(template.clone()), *a)).unwrap();
            assert_eq!(g.plane_flown().map(|p| p.at), Some(a.at));
        }
    }
    // Only a disc without the plane has none to start in.
    assert!(Lot::default().plane_anywhere(0, None, start).is_none());
    assert!(Garage::new(Kind::Biplane, None).is_err());
}

#[test]
fn parked_copies_stay_where_the_file_puts_them() {
    // No disc needed: a file's copies come out of the lot as P87 parks them, whatever start is given.
    let machines = "[Machines]\nNumberOfMachines = 1\n\n[Biplane1]\nType = Biplane\nPosition = 100, 20, -50\nDirection = 0, 0, -1\n";
    let lot = Lot::new(machines.as_bytes(), SEATS, &mut |_| 0.0);
    let start = Spawn { at: Vf::new(900.0, 0.0, 900.0, 1.0), facing: Vf::new(1.0, 0.0, 0.0, 0.0) };
    for i in 0..SEATS {
        let bay = lot.bay(vehicle::Machine::Biplane, i).expect("P87 parks one a seat beside the file's");
        let p = lot.plane_anywhere(i, None, start).expect("a parked copy");
        assert_eq!((p.bay, p.at), (bay.spawn, bay.spawn.at), "seat {i}");
    }
    assert_eq!(lot.bay(vehicle::Machine::Biplane, 0).unwrap().spawn.at.x(), 100.0);
}

#[test]
fn the_same_switch_online_on_a_race_track() {
    let Some(mut disc) = disc() else { return };
    let template = flight(&mut disc);
    let (bytes, scene) = terrain(&mut disc, "nat19");
    let field = Terrain::parse(&bytes).expect("nat19's terrain");
    let mut world = Ground::new(&field, &scene.terrain);
    let start = scene_start(&scene);
    let (s, d) = (start.at, start.facing);
    let wanted = vehicle::Wanted::default();
    let mut sims: Vec<Sim> = (0..SEATS)
        .map(|i| {
            let (x, z) = (s.x() + 80.0 * i as f32, s.z() + 30.0 * i as f32);
            let y = ground_at(&mut world, Vf::new(x, 0.0, z, 1.0));
            let mut sim = Sim::new(0);
            sim.rider.teleport_to(Spot { at: Vf::new(x, y, z, 1.0), forward: d, up: Vf::new(0.0, 1.0, 0.0, 0.0), motion: None, flight: None });
            sim
        })
        .collect();
    for (i, sim) in sims.iter_mut().enumerate() {
        let bike = sim.rider.ride.position();
        assert_eq!(vehicle::switch_lane(Kind::Biplane, Some(&template), sim, &mut world), Ok(Kind::Biplane));
        let f = sim.plane.as_ref().expect("the lane holds the plane");
        assert_eq!((f.spawn_origin().x(), f.spawn_origin().z()), (bike.x(), bike.z()), "seat {i}'s plane is not on its player");
        for _ in 0..60 {
            vehicle::fly_lane(sim, DT, wanted, &mut world);
        }
    }
    for (i, sim) in sims.iter_mut().enumerate() {
        let plane = vehicle::lane_spot(sim, &mut world).at;
        assert_eq!(vehicle::switch_lane(Kind::Bike, Some(&template), sim, &mut world), Ok(Kind::Bike));
        assert!(sim.plane.is_none());
        let bike = sim.rider.ride.position();
        assert_eq!((bike.x(), bike.z()), (plane.x(), plane.z()), "seat {i}'s bike is not under its plane");
    }
}

// ---- through the shell -------------------------------------------------------------------------

/// Ride the binary headless with these arguments, or `None` with no disc to ride. On the steady
/// clock: a busy machine's late frames dropped ticks off the wall clock's `--seconds`, so the
/// switching tape's last switch (tick 180 of a 9 s ride) never came.
fn shell(args: &[&str]) -> Option<String> {
    disc()?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut shell = Command::new(env!("CARGO_BIN_EXE_openmx"));
    shell.current_dir(&root).args(["--headless", "--steady-clock"]).args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let out = pad::tests::spawn(&mut shell).wait_with_output().expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    Some(said)
}

fn line<'a>(said: &'a str, prefix: &str) -> &'a str {
    said.lines().find(|l| l.starts_with(prefix)).unwrap_or_else(|| panic!("no `{prefix}` line:\n{said}"))
}

/// Full throttle at the engine's capped `dt` for `frames` ticks, switching to the plane at `up` and
/// back to the bike at `down` (tests/switch_tape.rs's tape).
fn switching_tape(name: &str, frames: usize, up: usize, down: usize) -> String {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape throttle and two switches\n");
    let report = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    text += &format!("switch {up} biplane\nswitch {down} bike\n");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("p113");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("the tape is written");
    path.to_str().expect("a path").to_string()
}

#[test]
fn the_shell_flies_the_plane_on_a_race_track() {
    // --vehicle biplane starts a four-player split there, every seat in its plane.
    let Some(said) = shell(&["--track", "nat19", "--vehicle", "biplane", "--players", "4", "--seconds", "2"]) else { return };
    assert!(said.contains("biplane: no parked copy here"), "\n{said}");
    assert_eq!(line(&said, "vehicle: "), "vehicle: biplane");
    assert!(line(&said, "plane hash").ends_with("(biplane)"), "\n{said}");
    // Its camera and particles from free06's archive, which a race track's lack.
    assert!(said.contains("biplane: 5 camera presets"), "the plane has no camera presets:\n{said}");
    assert!(!said.contains("biplane: no particles"), "\n{said}");
    // And a tape's switch there takes the plane and gives the bike back.
    let tape = switching_tape("nat19.tape", 240, 60, 180);
    let said = shell(&["--track", "nat19", "--seconds", "9", "--play", &tape]).unwrap();
    let switched: Vec<&str> = said.lines().filter(|l| l.starts_with("vehicle: ")).collect();
    assert_eq!(switched, ["vehicle: bike", "vehicle: biplane", "vehicle: bike"], "\n{said}");
}

#[test]
fn two_online_machines_start_their_seats_in_the_plane_on_a_race_track() {
    if disc().is_none() {
        return;
    }
    // Two ports the system has free now: fixed ones (47613/47614) were taken by the same test in
    // another slot's gate running at the same moment.
    let port = || std::net::UdpSocket::bind("127.0.0.1:0").and_then(|s| s.local_addr()).expect("a free port").port();
    let (hp, mut gp) = (port(), port());
    while gp == hp {
        gp = port();
    }
    let (h, g) = (format!("host@127.0.0.1:{hp}"), format!("g@127.0.0.1:{gp}"));
    let (h, g) = (h.as_str(), g.as_str());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ride = |me: &str, peer: &str| {
        let mut shell = Command::new(env!("CARGO_BIN_EXE_openmx"));
        shell
            .current_dir(&root)
            .args(["--headless", "--track", "nat19", "--vehicle", "biplane", "--frames", "400", "--seats", "host,g"])
            .args(["--online", me, "--peer", peer])
            .stdout(std::process::Stdio::piped());
        pad::tests::spawn(&mut shell)
    };
    let (host, guest) = (ride(h, g), ride(g, h));
    let said: Vec<String> = [host, guest]
        .into_iter()
        .map(|c| {
            let out = c.wait_with_output().expect("the shell ends");
            let said = String::from_utf8_lossy(&out.stdout).to_string();
            assert!(out.status.success(), "an online shell failed:\n{said}");
            said
        })
        .collect();
    for s in &said {
        assert_eq!(line(s, "online: 2 seats"), "online: 2 seats start in the biplane", "\n{s}");
        assert!(line_with(s, "desyncs").contains(" 0 desyncs"), "\n{s}");
    }
    let state = |s: &str| line_with(s, "state before tick 300").rsplit(' ').next().unwrap().to_string();
    assert_eq!(state(&said[0]), state(&said[1]), "the two machines' rides parted");
}

fn line_with<'a>(said: &'a str, part: &str) -> &'a str {
    said.lines().find(|l| l.contains(part)).unwrap_or_else(|| panic!("no line with `{part}`:\n{said}"))
}
