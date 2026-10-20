//! The front end (P36): every screen is reachable from the main menu by pad input alone, and the
//! settings files read back as they were written.
//!
//! The menu is a state machine fed by [`menu::Reader`], which turns the 32-byte reports the shell's
//! pad layer hands the ride into edges, so these tests drive it with nothing but reports — the same
//! bytes a DualShock or the keyboard would put there. The drawing half is checked against the disc
//! when there is one.

#[path = "../src/menu.rs"]
#[allow(dead_code)]
mod menu;

use game_sim::input::{AXIS_CENTRE, Control, Report};
use menu::{Act, Controls, Gas, Menu, Reader, Screen, Settings};
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
    // Settings from the pause, and back to it.
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Settings));
    tap(&mut m, &mut r, Control::Circle);
    assert_eq!(m.screen(), Some(Screen::Pause));
    // Restart and Main Menu.
    tap(&mut m, &mut r, Control::Up);
    let acts = tap(&mut m, &mut r, Control::Cross);
    assert!(acts.contains(&Act::Restart), "{acts:?}");
    tap(&mut m, &mut r, Control::Start);
    for _ in 0..3 {
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
                let lists = matches!(screen, Screen::Main | Screen::Pause | Screen::Settings | Screen::Controls);
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
    // Four list screens and the keyboard, twelve looks each, at four shapes.
    assert_eq!(pills, 5 * 12 * 4);
}
