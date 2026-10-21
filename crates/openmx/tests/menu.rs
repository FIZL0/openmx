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
#[path = "../src/roster.rs"]
#[allow(dead_code)]
mod roster;
#[path = "../src/join.rs"]
#[allow(dead_code)]
mod join;
#[path = "../src/split.rs"]
#[allow(dead_code)]
mod split;

use game_sim::input::{AXIS_CENTRE, Control, Report};
use menu::{Act, CLAIMED_FRAMES, Controls, Dev, Gas, Layout, Machine, Menu, Overlay, Press, Reader, Readers, Screen, Settings};
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
        // Seven: Controls is the fourth of the settings' seven rows since P42's sound rows, so the
        // wrap from the first row no longer reaches it in one.
        if found.len() == Screen::ALL.len() || path.len() >= 7 {
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
        overlay: Overlay::Debug,
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
        split: split::Split::SideBySide,
        music: "disc:headstrong".into(),
        music_volume: 3,
        effects_volume: 10,
    };
    let controls = Controls {
        throttle: Gas::R2,
        pad: Some("/dev/input/js1".into()),
        pressure: "ds2@8".into(),
        // One seat on the original's controls among the new (P40), which writes one word a seat.
        layouts: [Layout::New, Layout::Original, Layout::New, Layout::New, Layout::New, Layout::New, Layout::New, Layout::New],
        // And another flying the original's biplane (P50).
        planes: [Layout::New, Layout::New, Layout::Original, Layout::New, Layout::New, Layout::New, Layout::New, Layout::New],
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
    // Settings (under Local Game, Online and Garage), the name: clear it and type "Mx" on the keyboard,
    // then Done.
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Down);
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
    // Overlay on to debug, wrecks off, and the throttle onto L2 (Cross, then left from Cross wraps to L2).
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
    // 1P's row steps its layout and its biplane's together (P50), the biplane's the faster, and
    // wraps either way.
    tap(&mut m, &mut r, Control::Down);
    let one = |m: &Menu| (m.controls.layouts[0], m.controls.planes[0]);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(one(&m), (Layout::New, Layout::Original));
    tap(&mut m, &mut r, Control::Left);
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(one(&m), (Layout::Original, Layout::Original));
    tap(&mut m, &mut r, Control::Right);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(one(&m), (Layout::New, Layout::Original));
    assert_eq!(m.controls.planes[1], Layout::New, "2P's biplane moved with 1P's");

    let dir = scratch("pad");
    menu::save(&dir, &m.settings, &m.controls).unwrap();
    let (settings, controls) = menu::load(&dir);
    assert_eq!(
        settings,
        Settings { name: "Mx".into(), overlay: Overlay::Debug, wrecks: false, ..Settings::default() }
    );
    assert_eq!(controls.throttle, Gas::L2);
    assert_eq!(controls.planes[0], Layout::Original);
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

/// Whether two boxes (left, top, width, height) share any pixels.
fn overlap(a: menu::Bounds, b: menu::Bounds) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}

#[test]
fn every_pill_fits_its_label_and_nothing_covers_the_logo() {
    let mut disc = match game_render::Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let art = menu::Art::load(&mut disc).expect("the menus' art");
    // The layout's own 4:3-ish shape, 16:9, 21:9 and a portrait phone.
    let windows = [(640, 448), (960, 540), (1260, 540), (360, 640)];
    let mut pills = 0;
    for (w, h) in windows {
        let s = (w as f32 / 640.0).min(h as f32 / 448.0);
        for screen in Screen::ALL {
            let (mut m, mut reader) = fresh();
            m.open(screen);
            // Every row of a list, and a walk across the keyboard's keys and down to its commands.
            for step in 0..12 {
                assert_eq!(m.screen(), Some(screen));
                let mut frame = game_render::Frame::new(w, h, [90, 90, 90]);
                let parts = art.parts(&m, &mut frame);
                let logos: Vec<_> = parts.iter().filter_map(|p| match p {
                    menu::Part::Logo(b) => Some(*b),
                    _ => None,
                }).collect();
                for p in &parts {
                    match p {
                        menu::Part::Pill { pill, label } => {
                            pills += 1;
                            let want = label[2] + 2.0 * menu::PILL_PAD * s;
                            assert!(
                                (pill[2] - want).abs() <= 1.0,
                                "{screen:?} at {w}x{h}: a pill {} wide on a label {} wide",
                                pill[2],
                                label[2]
                            );
                            let (pc, lc) = (pill[0] + pill[2] / 2.0, label[0] + label[2] / 2.0);
                            assert!((pc - lc).abs() <= 1.0, "{screen:?} at {w}x{h}: a pill off its label");
                            for logo in &logos {
                                assert!(!overlap(*logo, *pill), "{screen:?} at {w}x{h}: a pill on the logo");
                                assert!(!overlap(*logo, *label), "{screen:?} at {w}x{h}: a label on the logo");
                            }
                        }
                        menu::Part::Item(b) => {
                            for logo in &logos {
                                assert!(!overlap(*logo, *b), "{screen:?} at {w}x{h}: an item on the logo");
                            }
                        }
                        // Unstretched: 512x64 at any shape.
                        menu::Part::Logo(b) => assert!((b[2] / b[3] - 8.0).abs() < 0.01, "{screen:?} at {w}x{h}"),
                    }
                }
                let lists = matches!(
                    screen,
                    Screen::Main
                        | Screen::Pause
                        | Screen::Settings
                        | Screen::Controls
                        | Screen::Online
                        | Screen::Games
                        | Screen::Lobby
                        | Screen::Split
                );
                if lists {
                    assert_eq!(logos.len(), 1, "{screen:?} at {w}x{h}: no logo");
                }
                if lists || screen == Screen::Keyboard {
                    let n = parts.iter().filter(|p| matches!(p, menu::Part::Pill { .. })).count();
                    assert_eq!(n, 1, "{screen:?} at {w}x{h}, step {step}: one pill under the cursor");
                }
                let control = if screen == Screen::Keyboard && step % 3 != 2 { Control::Right } else { Control::Down };
                tap(&mut m, &mut reader, control);
            }
        }
    }
    // Four list screens, the three online ones (N3), the split layout (P43) and the keyboard, twelve
    // looks each, at four shapes.
    assert_eq!(pills, 9 * 12 * 4);
}

/// [`tap`] on another seat's pad, which only the garage hears.
fn tap_garage_seat(menu: &mut Menu, reader: &mut Reader, seat: usize, control: Control) {
    for report in [Report::from_buttons(control.mask()), Report::idle()] {
        menu.step_seat(seat, reader.read(&report));
    }
}

#[test]
fn a_picked_bike_and_rider_reach_the_ride_that_starts_per_seat() {
    use game_render::rider::{Rider as Body, Sex};
    use game_sim::phy::Phy;
    use game_sim::track::{Engine, Tuning};
    use roster::{Catalogue, Picks};

    let mut disc = match game_render::Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let catalogue = Catalogue::read(&mut disc).unwrap();
    let mut reader = Reader::new();
    let mut m = Menu::new(Settings::default(), Controls::default()).with_garage(catalogue.clone(), Picks::default());
    m.step(reader.read(&Report::idle()));
    let mut readers: Vec<Reader> = (0..roster::SEATS).map(|_| Reader::new()).collect();
    for (i, r) in readers.iter_mut().enumerate().skip(1) {
        m.step_seat(i, r.read(&Report::idle()));
    }
    // Two seats sit down; the third has nobody in it.
    m.seated = 2;

    // Seat 1 opens the garage from the main menu: Down three times (past Local Game and Online), Cross.
    for _ in 0..3 {
        tap(&mut m, &mut reader, Control::Down);
    }
    tap(&mut m, &mut reader, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Garage));
    // Seat 1: class right (125 to 250), then Tier to Pro, then the next rider.
    tap(&mut m, &mut reader, Control::Right);
    for _ in 0..3 {
        tap(&mut m, &mut reader, Control::Down);
    }
    tap(&mut m, &mut reader, Control::Right);
    tap(&mut m, &mut reader, Control::Down);
    tap(&mut m, &mut reader, Control::Down);
    tap(&mut m, &mut reader, Control::Right);
    // Seat 2, on its own pad and its own column: class left (125 to 50), Tuning to A, and the
    // rider two along in its second colours.
    let r2 = &mut readers[1];
    tap_garage_seat(&mut m, r2, 1, Control::Left);
    for _ in 0..4 {
        tap_garage_seat(&mut m, r2, 1, Control::Down);
    }
    tap_garage_seat(&mut m, r2, 1, Control::Right);
    tap_garage_seat(&mut m, r2, 1, Control::Down);
    tap_garage_seat(&mut m, r2, 1, Control::Right);
    tap_garage_seat(&mut m, r2, 1, Control::Right);
    tap_garage_seat(&mut m, r2, 1, Control::Down);
    tap_garage_seat(&mut m, r2, 1, Control::Right);
    // Seat 3 is empty, so its pad moves nothing, and a seat's Cross does not close the screen.
    let before = m.garage(2).cloned();
    tap_garage_seat(&mut m, &mut readers[2], 2, Control::Right);
    tap_garage_seat(&mut m, &mut readers[1], 1, Control::Cross);
    assert_eq!(m.garage(2).cloned(), before);
    assert_eq!(m.screen(), Some(Screen::Garage));
    let one = m.garage(0).unwrap().pick.clone();
    let two = m.garage(1).unwrap().pick.clone();
    // Seat 1 leaves, and the picks are kept.
    tap(&mut m, &mut reader, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Main));
    assert!(m.take_picks_changed());
    assert_eq!((&m.picks.0[0], &m.picks.0[1]), (&one, &two));
    assert_eq!(m.picks.0[2], roster::Pick::default());

    assert_eq!((one.set.engine, one.set.pro, one.set.tuning), (Engine::Cc250, true, Tuning::P));
    assert_eq!((two.set.engine, two.set.pro, two.set.tuning), (Engine::Cc50, false, Tuning::A));
    assert_ne!(one.rider, two.rider);
    assert_ne!(one.rider, "thq1");

    // Persisted: `garage.txt` reads back as the screen left it.
    let dir = scratch("garage");
    m.picks.save(&dir).unwrap();
    assert_eq!(Picks::load(&dir), m.picks);
    std::fs::remove_dir_all(&dir).ok();

    // Free Ride, Cross on the first track: the ride starts, and it is the two seats' picks.
    for _ in 0..3 {
        tap(&mut m, &mut reader, Control::Up);
    }
    tap(&mut m, &mut reader, Control::Cross);
    let acts = tap(&mut m, &mut reader, Control::Cross);
    assert!(acts.contains(&Act::Ride("free06".into())), "{acts:?}");
    let picks = m.ride_picks(2).expect("a catalogue");
    assert_eq!(picks, vec![one.clone(), two.clone()]);

    // What the ride opens for each: the bike archive's own file of the set, which the ported loader
    // reads, and the rider archive with its own gear — every one different per seat.
    let tables = game_sim::rider::Tables::build();
    let mut seen = Vec::new();
    for (i, p) in picks.iter().enumerate() {
        let (bike, rider) = p.members();
        let res = disc.res(&bike).unwrap();
        let (bytes, member, note) = roster::physics(&res, p.set, &p.bike).unwrap();
        assert_eq!(note, None, "seat {}", i + 1);
        assert_eq!(member.to_ascii_lowercase(), format!("bike\\{}.phy", p.set.word()), "seat {}", i + 1);
        let phy = format!("{:?}", Phy::read_bytes(&bytes, &tables.ride.curve));
        let body = Body::load(&[disc.res(&rider).unwrap()], &p.rider, Sex::Male).unwrap();
        println!("seat {}: {} on {member}, {} in {}", i + 1, p.bike, p.rider, body.suit.mcf);
        seen.push((phy, body.suit.materials.clone()));
    }
    assert_ne!(seen[0].0, seen[1].0, "the two seats ride the same .phy");
    assert_ne!(seen[0].1, seen[1].1, "the two seats wear the same gear");
}

// ---- the splash (P46) ----------------------------------------------------------------------------

/// One frame of every device's report through their readers, the menu stepping on it.
fn devices(menu: &mut Menu, readers: &mut Readers, reports: &[(Dev, Report)]) -> Act {
    menu.step_devices(&readers.read(reports))
}

fn pad(name: &str) -> Dev {
    Dev::Pad(name.to_string())
}

/// A pad sat idle with its stick drifting: the left stick wandering round the centre, never
/// quite home, and on some frames well past the menus' lean — a worn stick, or a virtual pad
/// some program left with its axes anywhere.
fn drifting(frame: u32) -> Report {
    let mut r = Report::idle();
    let wander = |f: u32, k: u32| (AXIS_CENTRE as i32 + (((f * k) % 160) as i32 - 80)).clamp(0, 255) as u8;
    r.set_axis(2, wander(frame, 7));
    r.set_axis(3, wander(frame, 13));
    r
}

/// A second pad's stick pushed round all four ways, a direction for ten frames each.
fn circling(frame: u32) -> Report {
    let mut r = Report::idle();
    let (x, y) = [(0xff, AXIS_CENTRE), (AXIS_CENTRE, 0xff), (0, AXIS_CENTRE), (AXIS_CENTRE, 0)][(frame / 10 % 4) as usize];
    r.set_axis(2, x);
    r.set_axis(3, y);
    r
}

fn cross() -> Report {
    Report::from_buttons(Control::Cross.mask())
}

fn on_the_splash() -> (Menu, Readers) {
    let mut m = Menu::new(Settings::default(), Controls::default());
    m.splash();
    (m, Readers::new())
}

/// Frames until the "Player 1" card hands over to the main menu, every device at rest.
fn past_the_card(m: &mut Menu, r: &mut Readers, rest: &[(Dev, Report)]) {
    for _ in 0..CLAIMED_FRAMES {
        devices(m, r, rest);
    }
    assert_eq!(m.screen(), Some(Screen::Main));
}

#[test]
fn on_the_splash_an_idle_pad_s_drift_and_a_second_pad_s_stick_move_nothing() {
    let (mut m, mut r) = on_the_splash();
    for f in 0..600 {
        let act = devices(
            &mut m,
            &mut r,
            &[(Dev::Keys, Report::idle()), (pad("idle"), drifting(f)), (pad("second"), circling(f))],
        );
        assert_eq!(act, Act::Stay);
        assert_eq!(m.screen(), Some(Screen::Splash), "frame {f}");
        assert_eq!(m.p1(), None, "frame {f}");
        assert_eq!(m.quitting(), None, "frame {f}");
    }
}

#[test]
fn the_device_that_presses_cross_is_1p_and_the_only_one_driving_the_main_menu() {
    let rest = |a: Report, b: Report| [(Dev::Keys, Report::idle()), (pad("a"), a), (pad("b"), b)];
    let (mut m, mut r) = on_the_splash();
    devices(&mut m, &mut r, &rest(Report::idle(), Report::idle()));
    // Pad b's Cross: b is 1P; a's Cross the frame after is nobody's.
    devices(&mut m, &mut r, &rest(Report::idle(), cross()));
    assert_eq!(m.p1(), Some(&pad("b")));
    devices(&mut m, &mut r, &rest(cross(), Report::idle()));
    assert_eq!(m.p1(), Some(&pad("b")));
    past_the_card(&mut m, &mut r, &rest(Report::idle(), Report::idle()));
    assert_eq!(m.cursor(Screen::Main), 0);

    // Pad a, idle or drifting, and its Cross, move nothing on the main menu.
    for f in 0..300 {
        devices(&mut m, &mut r, &rest(drifting(f), Report::idle()));
        devices(&mut m, &mut r, &rest(circling(f), Report::idle()));
    }
    devices(&mut m, &mut r, &rest(cross(), Report::idle()));
    assert_eq!((m.screen(), m.cursor(Screen::Main)), (Some(Screen::Main), 0));

    // 1P's d-pad does.
    let down = Report::from_buttons(Control::Down.mask());
    devices(&mut m, &mut r, &rest(Report::idle(), down));
    devices(&mut m, &mut r, &rest(Report::idle(), Report::idle()));
    assert_eq!(m.cursor(Screen::Main), 1);
    // And the keyboard still works the menus whoever has 1P.
    let keys = [(Dev::Keys, Report::from_buttons(Control::Up.mask())), (pad("a"), Report::idle()), (pad("b"), Report::idle())];
    devices(&mut m, &mut r, &keys);
    assert_eq!(m.cursor(Screen::Main), 0);
    devices(&mut m, &mut r, &rest(Report::idle(), Report::idle()));
    devices(&mut m, &mut r, &rest(Report::idle(), cross()));
    assert_eq!(m.screen(), Some(Screen::Tracks));
}

#[test]
fn the_keyboard_s_enter_or_space_takes_1p_and_then_no_pad_drives() {
    // Enter is the keyboard's Start and Space its Cross (pad.rs's bindings): both take 1P.
    for key in [Control::Start, Control::Cross] {
        let rest = [(Dev::Keys, Report::idle()), (pad("a"), Report::idle())];
        let (mut m, mut r) = on_the_splash();
        devices(&mut m, &mut r, &rest);
        let press = [(Dev::Keys, Report::from_buttons(key.mask())), (pad("a"), Report::idle())];
        devices(&mut m, &mut r, &press);
        assert_eq!(m.p1(), Some(&Dev::Keys), "{key:?}");
        past_the_card(&mut m, &mut r, &rest);
        let down = [(Dev::Keys, Report::idle()), (pad("a"), Report::from_buttons(Control::Down.mask()))];
        devices(&mut m, &mut r, &down);
        devices(&mut m, &mut r, &rest);
        assert_eq!(m.cursor(Screen::Main), 0, "a pad moved the keyboard's menus");
    }
}

#[test]
fn an_unplug_of_1p_s_device_brings_the_splash_back() {
    let (mut m, mut r) = on_the_splash();
    let both = |a: Report| [(Dev::Keys, Report::idle()), (pad("a"), a), (pad("b"), Report::idle())];
    devices(&mut m, &mut r, &both(Report::idle()));
    devices(&mut m, &mut r, &both(cross()));
    past_the_card(&mut m, &mut r, &both(Report::idle()));
    // Another pad going changes nothing; 1P's going is the splash, with nobody 1P.
    assert!(!m.lost(&["a".to_string()]));
    assert_eq!(m.screen(), Some(Screen::Main));
    assert!(m.lost(&["b".to_string()]));
    assert_eq!((m.screen(), m.p1()), (Some(Screen::Splash), None));
    // And the next press is the new 1P.
    let only_b = |b: Report| [(Dev::Keys, Report::idle()), (pad("b"), b)];
    devices(&mut m, &mut r, &only_b(Report::idle()));
    devices(&mut m, &mut r, &only_b(cross()));
    assert_eq!(m.p1(), Some(&pad("b")));
    // Over a ride the seat waits for its pad (P35) instead: the pause is left alone.
    past_the_card(&mut m, &mut r, &only_b(Report::idle()));
    m.pause();
    assert!(!m.lost(&[]));
    assert_eq!(m.screen(), Some(Screen::Pause));
}

#[test]
fn back_on_the_splash_asks_to_quit_and_only_the_asker_answers() {
    let circle = Report::from_buttons(Control::Circle.mask());
    let rest = |a: Report, b: Report| [(Dev::Keys, Report::idle()), (pad("a"), a), (pad("b"), b)];
    let (mut m, mut r) = on_the_splash();
    devices(&mut m, &mut r, &rest(Report::idle(), Report::idle()));
    devices(&mut m, &mut r, &rest(circle, Report::idle()));
    assert_eq!(m.quitting(), Some(1), "it opens on Stay");
    // Pad b's stick and Cross answer nothing.
    for f in 0..40 {
        devices(&mut m, &mut r, &rest(Report::idle(), circling(f)));
    }
    assert_eq!(devices(&mut m, &mut r, &rest(Report::idle(), cross())), Act::Stay);
    assert_eq!((m.quitting(), m.p1()), (Some(1), None));
    // Stay closes it; asked again, up and Cross quit.
    devices(&mut m, &mut r, &rest(cross(), Report::idle()));
    assert_eq!(m.quitting(), None);
    devices(&mut m, &mut r, &rest(circle, Report::idle()));
    devices(&mut m, &mut r, &rest(Report::from_buttons(Control::Up.mask()), Report::idle()));
    assert_eq!(m.quitting(), Some(0));
    devices(&mut m, &mut r, &rest(Report::idle(), Report::idle()));
    assert_eq!(devices(&mut m, &mut r, &rest(cross(), Report::idle())), Act::Quit);
    // Back on the main menu is the splash again, 1P free.
    let (mut m, mut r) = on_the_splash();
    devices(&mut m, &mut r, &rest(Report::idle(), Report::idle()));
    devices(&mut m, &mut r, &rest(cross(), Report::idle()));
    past_the_card(&mut m, &mut r, &rest(Report::idle(), Report::idle()));
    devices(&mut m, &mut r, &rest(Report::idle(), circle));
    assert_eq!(m.screen(), Some(Screen::Main), "only 1P and the keyboard go back");
    devices(&mut m, &mut r, &rest(circle, Report::idle()));
    assert_eq!((m.screen(), m.p1()), (Some(Screen::Splash), None));
}

/// One frame of the front end as main.rs's loop runs it (P43): which pad seats have a controller
/// in, then pad seat 0's press through [`Menu::step`] and every other's through [`Menu::step_seat`].
fn front_frame(m: &mut Menu, readers: &mut [Reader], present: &[usize], reports: &[Report]) -> Act {
    m.present(present);
    for i in 1..readers.len() {
        let p = readers[i].read(&reports[i]);
        m.step_seat(i, p);
    }
    let p = readers[0].read(&reports[0]);
    m.step(p)
}

/// One press and release of `control` on pad seat `device`'s controller, every other at rest.
fn tap_device(m: &mut Menu, readers: &mut [Reader], present: &[usize], device: usize, control: Control) -> Vec<Act> {
    let mut acts = Vec::new();
    for pressed in [true, false] {
        let reports: Vec<Report> = (0..readers.len())
            .map(|i| match pressed && i == device {
                true => Report::from_buttons(control.mask()),
                false => Report::idle(),
            })
            .collect();
        acts.push(front_frame(m, readers, present, &reports));
    }
    acts
}

/// A menu with `pads` readers that have seen their controllers at rest, as the shell's have.
fn fresh_with(pads: usize) -> (Menu, Vec<Reader>) {
    let mut m = Menu::new(Settings::default(), Controls::default());
    let mut readers: Vec<Reader> = (0..pads).map(|_| Reader::new()).collect();
    let present: Vec<usize> = (0..pads).collect();
    front_frame(&mut m, &mut readers, &present, &vec![Report::idle(); pads]);
    (m, readers)
}

#[test]
fn two_and_four_players_join_pick_a_track_and_ride_that_many_views() {
    // Two: four pads in, the first and the third play. Local Game is the main menu's second row.
    let all = [0, 1, 2, 3];
    let (mut m, mut rd) = fresh_with(4);
    tap_device(&mut m, &mut rd, &all, 0, Control::Down);
    tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Join));
    // The pad that chose Local Game is 1P already; the third joins and moves a seat right.
    assert_eq!(m.join.views(), vec![0]);
    tap_device(&mut m, &mut rd, &all, 2, Control::Cross);
    tap_device(&mut m, &mut rd, &all, 2, Control::Right);
    assert_eq!(m.join.riders()[..3], [Some(0), None, Some(2)]);
    // The second and fourth stay out: their Left and Right and Square move nothing.
    for c in [Control::Left, Control::Right, Control::Square, Control::Start] {
        tap_device(&mut m, &mut rd, &all, 1, c);
        tap_device(&mut m, &mut rd, &all, 3, c);
    }
    assert_eq!(m.join.views(), vec![0, 2]);
    assert_eq!(m.screen(), Some(Screen::Join));
    // Start from the joined pad: the split layout, the original's two for a pair. Side by side.
    tap_device(&mut m, &mut rd, &all, 2, Control::Start);
    assert_eq!(m.screen(), Some(Screen::Split));
    assert_eq!(menu::layouts(2), vec![split::Layout::Stacked, split::Layout::SideBySide]);
    tap_device(&mut m, &mut rd, &all, 0, Control::Down);
    tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Tracks));
    // The ride setup lists a machine for each of the two.
    assert_eq!(m.riders, 2);
    tap_device(&mut m, &mut rd, &all, 0, Control::Right);
    let acts = tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
    assert!(acts.contains(&Act::Ride("free07".into())), "{acts:?}");
    let party = m.party().expect("a local game").clone();
    assert_eq!(party.players(), 2);
    // Player 1 rides pad seat 0's controller and player 2 pad seat 2's.
    assert_eq!(party.devices, vec![0, 2]);
    assert_eq!(party.layout(), split::Layout::SideBySide);
    let views = party.layout().rects(640, 448);
    assert_eq!(views.len(), 2);
    assert!(views.iter().all(|v| (v.w, v.h) == (320, 448)), "{views:?}");
    // The pick is the file's, so the next local game of two opens on it.
    assert_eq!(m.settings.split, split::Split::SideBySide);
    assert!(m.settings.text().contains("split = v"));
    assert_eq!(Settings::parse(&m.settings.text()), m.settings);
    assert!(m.riding());

    // Back on the main menu after the ride, a Free Ride is `--players`' again and no local game.
    m.main();
    tap_device(&mut m, &mut rd, &all, 0, Control::Up);
    tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Tracks));
    assert!(m.party().is_none());
    assert_eq!(m.riders, 1);

    // Four: every pad joins; the last is pulled out before Start, which keeps its seat and its view
    // (the rider waits on the start line for it).
    let (mut m, mut rd) = fresh_with(4);
    tap_device(&mut m, &mut rd, &all, 0, Control::Down);
    tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
    for d in 1..4 {
        tap_device(&mut m, &mut rd, &all, d, Control::Cross);
    }
    assert_eq!(m.join.views(), vec![0, 1, 2, 3]);
    let three = [0, 1, 2];
    front_frame(&mut m, &mut rd, &three, &vec![Report::idle(); 4]);
    assert!(matches!(m.join.seats[3], join::Seat::Gone(3)));
    tap_device(&mut m, &mut rd, &three, 1, Control::Start);
    assert_eq!(m.screen(), Some(Screen::Split));
    // Four have one layout, the port's quarters, so the screen has one row.
    assert_eq!(menu::layouts(4), vec![split::Layout::Quarters]);
    tap_device(&mut m, &mut rd, &three, 0, Control::Cross);
    assert_eq!(m.riders, 4);
    let acts = tap_device(&mut m, &mut rd, &three, 0, Control::Cross);
    assert!(acts.contains(&Act::Ride("free06".into())), "{acts:?}");
    let party = m.party().expect("a local game");
    assert_eq!(party.devices, vec![0, 1, 2, 3]);
    let views = party.layout().rects(640, 448);
    assert_eq!(views.len(), 4);
    assert!(views.iter().all(|v| (v.w, v.h) == (320, 224)), "{views:?}");
}

#[test]
fn backing_out_of_a_local_game_walks_its_screens_back_to_the_main_menu() {
    let all = [0, 1];
    let (mut m, mut rd) = fresh_with(2);
    tap_device(&mut m, &mut rd, &all, 0, Control::Down);
    tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
    tap_device(&mut m, &mut rd, &all, 1, Control::Cross);
    tap_device(&mut m, &mut rd, &all, 0, Control::Start);
    tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Tracks));
    tap_device(&mut m, &mut rd, &all, 0, Control::Circle);
    assert_eq!(m.screen(), Some(Screen::Split));
    tap_device(&mut m, &mut rd, &all, 0, Control::Circle);
    assert_eq!(m.screen(), Some(Screen::Join));
    // Circle on the join screen is a seated pad leaving, then, with no seat, back.
    tap_device(&mut m, &mut rd, &all, 1, Control::Circle);
    assert_eq!(m.join.views(), vec![0]);
    tap_device(&mut m, &mut rd, &all, 1, Control::Circle);
    assert_eq!(m.screen(), Some(Screen::Main));
    // The seats are kept for the next local game.
    tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Join));
    assert_eq!(m.join.views(), vec![0]);
    // Online sits beside it: N3's online screen.
    tap_device(&mut m, &mut rd, &all, 1, Control::Circle);
    tap_device(&mut m, &mut rd, &all, 0, Control::Down);
    tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Online));
}

/// P42's three sound rows, below Controls: the music steps Off, Shuffle and every song the shell's
/// sources listed, and the two sliders stop at their ends.
#[test]
fn the_sound_rows_pick_the_music_and_the_volumes() {
    let (mut m, mut r) = fresh();
    m.music_choices = vec![("disc:ooc".into(), "Out of Control - Hoobastank".into()), ("folder:mine.wav".into(), "mine".into())];
    // Settings, under Local Game, Online and Garage.
    for _ in 0..4 {
        tap(&mut m, &mut r, Control::Down);
    }
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Settings));
    for _ in 0..4 {
        tap(&mut m, &mut r, Control::Down);
    }
    assert_eq!(m.settings.music, "shuffle");
    assert_eq!(m.music_label(), "Shuffle");
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.music, "disc:ooc");
    assert_eq!(m.music_label(), "Out of Control - Hoobastank");
    tap(&mut m, &mut r, Control::Right);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.music, "off", "past the last song it wraps to Off");
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.settings.music, "folder:mine.wav");
    // The music volume, down to nothing and no further.
    tap(&mut m, &mut r, Control::Down);
    for _ in 0..12 {
        tap(&mut m, &mut r, Control::Left);
    }
    assert_eq!(m.settings.music_volume, 0);
    // The effects, up to the top and no further.
    tap(&mut m, &mut r, Control::Down);
    for _ in 0..12 {
        tap(&mut m, &mut r, Control::Right);
    }
    assert_eq!(m.settings.effects_volume, menu::VOLUME_MAX);
    assert!(m.take_changed());
    let back = Settings::parse(&m.settings.text());
    assert_eq!((back.music.as_str(), back.music_volume, back.effects_volume), ("folder:mine.wav", 0, 10));
    // An older file, with no sound keys, opens on the defaults.
    let old = Settings::parse("name = Old\n");
    assert_eq!((old.music.as_str(), old.music_volume, old.effects_volume), ("shuffle", 7, 8));
}
