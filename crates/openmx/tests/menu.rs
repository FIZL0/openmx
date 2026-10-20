//! The front end (P36): every screen is reachable from the main menu by pad input alone, and the
//! settings files read back as they were written. And the machine in the menus (P41): the pause's
//! switch puts the player who opened it in the other machine, and the ride setup's pick is the
//! machine the ride starts on.
//!
//! The menu is a state machine fed by [`menu::Reader`], which turns the 32-byte reports the shell's
//! pad layer hands the ride into edges, so these tests drive it with nothing but reports — the same
//! bytes a DualShock or the keyboard would put there. The drawing half is checked against the disc
//! when there is one.

#[path = "../src/menu.rs"]
#[allow(dead_code)]
mod menu;
#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;

use game_sim::input::{AXIS_CENTRE, Control, Report};
use menu::{Act, Controls, Gas, Machine, Menu, Press, Reader, Screen, Settings};
use vehicle::{Garage, Kind, Plane, Spawn};
use std::collections::{HashSet, VecDeque};

/// The buttons a hand can press on a menu.
const BUTTONS: [Control; 8] = [
    Control::Up,
    Control::Down,
    Control::Left,
    Control::Right,
    Control::Cross,
    Control::Circle,
    Control::Square,
    Control::Start,
];

/// One press and release of `control`: two frames of reports through the reader, the menu stepping
/// on each.
fn tap(menu: &mut Menu, reader: &mut Reader, control: Control) -> Vec<Act> {
    let mut acts = Vec::new();
    for report in [Report::from_buttons(control.mask()), Report::idle()] {
        acts.push(menu.step(reader.read(&report)));
    }
    acts
}

/// A menu and a reader that has seen the pad at rest, as the shell's has by the first menu frame.
fn fresh() -> (Menu, Reader) {
    let mut reader = Reader::new();
    let mut menu = Menu::new(Settings::default(), Controls::default());
    menu.step(reader.read(&Report::idle()));
    (menu, reader)
}

/// The menu as a state, without the slideshow's clock.
fn key(menu: &Menu) -> String {
    let mut m = menu.clone();
    m.tick = 0;
    format!("{m:?}")
}

#[test]
fn every_screen_is_reachable_from_the_main_menu_by_pad_input_alone() {
    let (menu, reader) = fresh();
    assert_eq!(menu.screen(), Some(Screen::Main));
    // Breadth first over single button taps, so each screen is found by its shortest path.
    let mut found: Vec<(Screen, Vec<Control>)> = vec![(Screen::Main, vec![])];
    let mut seen = HashSet::from([key(&menu)]);
    let mut queue = VecDeque::from([(menu, reader, Vec::<Control>::new())]);
    while let Some((menu, reader, path)) = queue.pop_front() {
        if found.len() == Screen::ALL.len() || path.len() >= 6 {
            break;
        }
        for b in BUTTONS {
            let (mut m, mut r) = (menu.clone(), reader.clone());
            let acts = tap(&mut m, &mut r, b);
            if acts.contains(&Act::Quit) {
                continue;
            }
            let mut p = path.clone();
            p.push(b);
            if let Some(s) = m.screen()
                && !found.iter().any(|f| f.0 == s)
            {
                found.push((s, p.clone()));
            }
            if seen.insert(key(&m)) {
                queue.push_back((m, r, p));
            }
        }
    }
    for s in Screen::ALL {
        let path = found.iter().find(|f| f.0 == s);
        assert!(path.is_some(), "{s:?} is not reachable from the main menu");
    }
    for (s, path) in &found {
        let names: Vec<&str> = path.iter().map(|c| c.name()).collect();
        println!("{s:?}: {}", names.join(" "));
    }
}

#[test]
fn the_walk_a_rider_takes_rides_pauses_and_comes_back() {
    let (mut m, mut r) = fresh();
    // Free Ride, then right twice: Bridgeport.
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Tracks));
    tap(&mut m, &mut r, Control::Right);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.cursor(Screen::Tracks), 2);
    let acts = tap(&mut m, &mut r, Control::Cross);
    assert!(acts.contains(&Act::Ride("free09".into())), "{acts:?}");
    assert!(m.riding());
    assert_eq!(m.settings.track, "free09");
    assert!(m.take_changed());
    // In the ride Cross is the throttle and nothing else; Start pauses.
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.riding());
    tap(&mut m, &mut r, Control::Start);
    assert_eq!(m.screen(), Some(Screen::Pause));
    // Settings from the pause (past the machine switch, P41), and back to it.
    for _ in 0..3 {
        tap(&mut m, &mut r, Control::Down);
    }
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Settings));
    tap(&mut m, &mut r, Control::Circle);
    assert_eq!(m.screen(), Some(Screen::Pause));
    // Restart and Main Menu.
    tap(&mut m, &mut r, Control::Up);
    tap(&mut m, &mut r, Control::Up);
    let acts = tap(&mut m, &mut r, Control::Cross);
    assert!(acts.contains(&Act::Restart), "{acts:?}");
    tap(&mut m, &mut r, Control::Start);
    for _ in 0..4 {
        tap(&mut m, &mut r, Control::Down);
    }
    let acts = tap(&mut m, &mut r, Control::Cross);
    assert!(acts.contains(&Act::MainMenu), "{acts:?}");
    assert_eq!(m.screen(), Some(Screen::Main));
    // Circle on the pause resumes, as Start does.
    m.pause();
    let acts = tap(&mut m, &mut r, Control::Circle);
    assert!(acts.contains(&Act::Resume), "{acts:?}");
    assert!(m.riding());
}

#[test]
fn a_held_direction_repeats_and_the_stick_counts_as_one() {
    let (mut m, mut r) = fresh();
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Tracks));
    // The stick leaned right, held for a second: one step, then the repeats.
    let mut right = Report::idle();
    right.set_axis(2, 0xff);
    for _ in 0..60 {
        m.step(r.read(&right));
    }
    let moved = m.cursor(Screen::Tracks);
    assert!(moved >= 3, "a second of stick moved the pick {moved} times");
    // And a centred stick moves nothing.
    let mut centre = Report::idle();
    centre.set_axis(2, AXIS_CENTRE);
    for _ in 0..60 {
        m.step(r.read(&centre));
    }
    assert_eq!(m.cursor(Screen::Tracks), moved);
    // The first report a fresh reader sees is a hold, not a press: the Cross that opened a screen
    // does not press on it too.
    let mut fresh_reader = Reader::new();
    let first = fresh_reader.read(&Report::from_buttons(Control::Cross.mask()));
    assert!(!first.ok);
}

/// A scratch directory of this test's own.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("openmx-menu-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_settings_file_reads_back_as_it_was_written() {
    let dir = scratch("files");
    let settings = Settings {
        name: "Ricky C".into(),
        overlay: true,
        wrecks: false,
        track: "free11".into(),
        machines: [
            Machine::Biplane,
            Machine::Bike,
            Machine::Biplane,
            Machine::Bike,
            Machine::Bike,
            Machine::Bike,
            Machine::Bike,
            Machine::Biplane,
        ],
    };
    let controls = Controls {
        throttle: Gas::R2,
        pad: Some("/dev/input/js1".into()),
        pressure: "ds2@8".into(),
    };
    menu::save(&dir, &settings, &controls).unwrap();
    assert_eq!(menu::load(&dir), (settings, controls));
    // A directory with neither file is the defaults, and so is a file of nonsense.
    let empty = scratch("empty");
    assert_eq!(menu::load(&empty), (Settings::default(), Controls::default()));
    std::fs::write(empty.join(menu::SETTINGS_FILE), "track = nowhere\nwrecks\n= on\n").unwrap();
    assert_eq!(menu::load(&empty).0, Settings::default());
    // The defaults themselves round trip too.
    menu::save(&empty, &Settings::default(), &Controls::default()).unwrap();
    assert_eq!(menu::load(&empty), (Settings::default(), Controls::default()));
    std::fs::remove_dir_all(dir).ok();
    std::fs::remove_dir_all(empty).ok();
}

#[test]
fn what_the_pad_sets_is_what_the_file_says() {
    let (mut m, mut r) = fresh();
    // Settings, the name: clear it and type "Mx" on the keyboard, then Done.
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Settings));
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Keyboard));
    // Up from the letters wraps to the command row: Space Delete Clear Done.
    tap(&mut m, &mut r, Control::Up);
    tap(&mut m, &mut r, Control::Right);
    tap(&mut m, &mut r, Control::Right);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.typing(), "");
    // m is row 1, column 2; x is row 2, column 3.
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Down);
    // Down from Clear lands over column 5 of the letters; three left is m.
    for _ in 0..3 {
        tap(&mut m, &mut r, Control::Left);
    }
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.typing(), "M");
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Right);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.typing(), "Mx");
    // Square deletes, and the letter goes back.
    tap(&mut m, &mut r, Control::Square);
    assert_eq!(m.typing(), "M");
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Start);
    assert_eq!(m.screen(), Some(Screen::Settings));
    assert_eq!(m.settings.name, "Mx");
    // Overlay on, wrecks off, and the throttle onto L2 (Cross, then left from Cross wraps to L2).
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Right);
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Controls));
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.controls.throttle, Gas::L2);
    assert!(m.take_changed());

    let dir = scratch("pad");
    menu::save(&dir, &m.settings, &m.controls).unwrap();
    let (settings, controls) = menu::load(&dir);
    assert_eq!(
        settings,
        Settings { name: "Mx".into(), overlay: true, wrecks: false, ..Settings::default() }
    );
    assert_eq!(controls.throttle, Gas::L2);
    // And a menu opened on what was read is the menu that wrote it.
    let again = Menu::new(settings, controls);
    assert_eq!((&again.settings, &again.controls), (&m.settings, &m.controls));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn every_screen_draws_from_the_disc() {
    let mut disc = match game_render::Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let art = menu::Art::load(&mut disc).expect("the menus' art");
    for s in Screen::ALL {
        let mut m = Menu::new(Settings::default(), Controls::default());
        m.open(s);
        // A grey stand-in for the ride, which the pause draws over rather than covers.
        const RIDE: [u8; 3] = [90, 90, 90];
        let mut frame = game_render::Frame::new(320, 224, RIDE);
        art.draw(&m, &mut frame);
        let drawn = frame.color.chunks(3).filter(|p| *p != RIDE).count();
        assert!(drawn > frame.width * frame.height * 9 / 10, "{s:?} drew over {drawn} pixels");
    }
}

/// A track's `machine.txt` with a plane for each of two players, in the disc's shape and with
/// numbers of this file's own.
const MACHINES: &str = "[Machines]\nNumberOfMachines = 2\n\n\
    [Biplane1]\nType = Biplane\nPosition = 100, 20, -50\nDirection = 0, 0, -1\nHasPlayer = 1\n\n\
    [Biplane2]\nType = Biplane\nPosition = 300, 20, -50\nDirection = 0, 0, -1\nHasPlayer = 1\n";

/// Player `seat`'s garage on a ride starting on `up`, with their plane parked where the file says.
fn garage(seat: usize, up: Kind) -> Garage {
    let plane = Spawn::biplane(MACHINES.as_bytes(), seat).map(Plane::parked);
    Garage::new(up, plane).unwrap()
}

/// What the shell does with a frame of the ride's menu: every seat's press through
/// [`Menu::step_seats`], and a switch answered by that seat's garage, as main.rs's ride loop does.
fn ride_frame(m: &mut Menu, readers: &mut [Reader], reports: &[Report], garages: &mut [Garage]) -> Act {
    let presses: Vec<Press> = readers.iter_mut().zip(reports).map(|(r, rep)| r.read(rep)).collect();
    let act = m.step_seats(&presses);
    if let Act::Switch(seat) = act {
        let up = garages[seat].swap().unwrap();
        m.switched(seat, Machine::from_word(up.name()).unwrap());
    }
    act
}

/// One press and release of `control` on `seat`'s pad, the others at rest.
fn tap_seat(m: &mut Menu, readers: &mut [Reader], garages: &mut [Garage], seat: usize, control: Control) -> Vec<Act> {
    let mut acts = Vec::new();
    for pressed in [true, false] {
        let reports: Vec<Report> = (0..readers.len())
            .map(|i| match pressed && i == seat {
                true => Report::from_buttons(control.mask()),
                false => Report::idle(),
            })
            .collect();
        acts.push(ride_frame(m, readers, &reports, garages));
    }
    acts
}

#[test]
fn picking_the_biplane_in_the_pause_puts_that_player_in_it() {
    // Two riders on the bike, each with a plane parked (the second only in this file: the disc's
    // tracks park the first player's), and the menu told so the way the ride tells it.
    let (mut m, mut r) = fresh();
    m.riders = 2;
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.riding());
    let mut garages = vec![garage(0, Kind::Bike), garage(1, Kind::Bike)];
    for (i, g) in garages.iter().enumerate() {
        m.up[i] = Machine::from_word(g.up.name()).unwrap();
        m.switchable[i] = g.plane.is_some();
    }
    let mut readers = vec![r, Reader::new()];
    ride_frame(&mut m, &mut readers, &[Report::idle(), Report::idle()], &mut garages);

    // 2P's Start pauses for 2P, and 1P's pad does nothing to it.
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Start);
    assert_eq!(m.screen(), Some(Screen::Pause));
    assert_eq!(m.driver(), 1);
    assert_eq!(m.pause_rows()[2], "Switch To Machine: Biplane");
    tap_seat(&mut m, &mut readers, &mut garages, 0, Control::Down);
    assert_eq!(m.cursor(Screen::Pause), 0);
    // Down twice and Cross on 2P's pad: 2P is in the plane, the ride carries on, 1P is on the bike.
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Down);
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Down);
    let acts = tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Cross);
    assert!(acts.contains(&Act::Switch(1)), "{acts:?}");
    assert!(m.riding());
    assert_eq!((garages[0].up, garages[1].up), (Kind::Bike, Kind::Biplane));
    assert_eq!(m.up[..2], [Machine::Bike, Machine::Biplane]);

    // 1P's own pause switches 1P, and the row now offers 2P the bike back.
    tap_seat(&mut m, &mut readers, &mut garages, 0, Control::Start);
    assert_eq!(m.driver(), 0);
    tap_seat(&mut m, &mut readers, &mut garages, 0, Control::Down);
    tap_seat(&mut m, &mut readers, &mut garages, 0, Control::Down);
    let acts = tap_seat(&mut m, &mut readers, &mut garages, 0, Control::Cross);
    assert!(acts.contains(&Act::Switch(0)), "{acts:?}");
    assert_eq!((garages[0].up, garages[1].up), (Kind::Biplane, Kind::Biplane));
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Start);
    assert_eq!(m.pause_rows()[2], "Switch To Machine: Bike");
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Start);
    assert!(m.riding(), "2P's Start resumes 2P's pause");

    // A player with no plane to take: the row says so and pressing it switches nothing.
    m.switchable[1] = false;
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Start);
    assert_eq!(m.pause_rows()[2], "Switch To Machine: None");
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Down);
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Down);
    let acts = tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Cross);
    assert!(!acts.iter().any(|a| matches!(a, Act::Switch(_))), "{acts:?}");
    assert_eq!(m.screen(), Some(Screen::Pause));
    assert_eq!(garages[1].up, Kind::Biplane);
}

#[test]
fn the_setups_pick_starts_the_ride_on_it() {
    let (mut m, mut r) = fresh();
    m.riders = 2;
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Tracks));
    // Square changes 1P's machine; down to 2P and back to bike-then-plane leaves 2P on the plane;
    // down again wraps to 1P. Left and right still walk the tracks and nothing else.
    tap(&mut m, &mut r, Control::Square);
    assert_eq!(m.settings.machines[0], Machine::Biplane);
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Square);
    tap(&mut m, &mut r, Control::Square);
    tap(&mut m, &mut r, Control::Square);
    assert_eq!(m.settings.machines[..2], [Machine::Biplane, Machine::Biplane]);
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Square);
    assert_eq!(m.settings.machines[..3], [Machine::Bike, Machine::Biplane, Machine::Bike]);
    tap(&mut m, &mut r, Control::Square);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.cursor(Screen::Tracks), 1);
    assert_eq!(m.settings.machines[..2], [Machine::Biplane, Machine::Biplane]);
    let acts = tap(&mut m, &mut r, Control::Cross);
    assert!(acts.contains(&Act::Ride("free07".into())), "{acts:?}");
    assert!(m.take_changed());

    // The ride starts on the pick: the shell reads 1P's as `--vehicle`'s word into the garage.
    let up = Kind::parse(m.settings.machines[0].word()).unwrap();
    assert_eq!(garage(0, up).up, Kind::Biplane);
    // And the pick is what the file keeps, so the next session's setup opens on it.
    let dir = scratch("machines");
    menu::save(&dir, &m.settings, &m.controls).unwrap();
    assert_eq!(menu::load(&dir).0.machines, m.settings.machines);
    assert!(std::fs::read_to_string(dir.join(menu::SETTINGS_FILE)).unwrap().contains("machines = biplane biplane bike"));
    std::fs::remove_dir_all(dir).ok();
    // An older file, with no machines line, rides the bike.
    assert_eq!(Settings::parse("track = free07\n").machines, [Machine::Bike; menu::SEATS_MAX]);
}
