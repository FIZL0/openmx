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
#[path = "../src/roster.rs"]
#[allow(dead_code)]
mod roster;

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

/// [`tap`] on another seat's pad, which only the garage hears.
fn tap_seat(menu: &mut Menu, reader: &mut Reader, seat: usize, control: Control) {
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

    // Seat 1 opens the garage from the main menu: Down, Cross.
    tap(&mut m, &mut reader, Control::Down);
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
    tap_seat(&mut m, r2, 1, Control::Left);
    for _ in 0..4 {
        tap_seat(&mut m, r2, 1, Control::Down);
    }
    tap_seat(&mut m, r2, 1, Control::Right);
    tap_seat(&mut m, r2, 1, Control::Down);
    tap_seat(&mut m, r2, 1, Control::Right);
    tap_seat(&mut m, r2, 1, Control::Right);
    tap_seat(&mut m, r2, 1, Control::Down);
    tap_seat(&mut m, r2, 1, Control::Right);
    // Seat 3 is empty, so its pad moves nothing, and a seat's Cross does not close the screen.
    let before = m.garage(2).cloned();
    tap_seat(&mut m, &mut readers[2], 2, Control::Right);
    tap_seat(&mut m, &mut readers[1], 1, Control::Cross);
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
    tap(&mut m, &mut reader, Control::Up);
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
