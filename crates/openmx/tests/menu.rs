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
// The look painted onto the ride (P86).
#[path = "../src/dress.rs"]
#[allow(dead_code)]
mod dress;
#[path = "../src/join.rs"]
#[allow(dead_code)]
mod join;
#[path = "../src/split.rs"]
#[allow(dead_code)]
mod split;
// The shell's pad layer, for P53's presses between frames: a binary crate's module, compiled in by
// path as tests/ride.rs does, its own unit tests coming with it.
#[allow(dead_code)]
#[path = "../src/window.rs"]
mod window;
#[allow(dead_code)]
#[path = "../src/pad.rs"]
mod pad;
// The shell's sound, for the pause's music page (P56).
#[allow(dead_code)]
#[path = "../src/audio.rs"]
mod audio;

use game_sim::input::{AXIS_CENTRE, Control, Report};
use menu::{Act, CLAIMED_FRAMES, Controls, Dev, Gas, Layout, Machine, Menu, FrameLimit, Overlay, Press, Reader, Readers, RenderScale, Screen, Settings, Speedometer};
use vehicle::{Garage, Kind, Plane, Spawn};
use game_render::world::Graphics;
use game_sim::modifiers::{Knob, Mods};
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
    let (mut menu, reader) = fresh();
    // A second pad has joined a local game before (P51): the split screen is two players' at least,
    // and this walk taps one pad only. One alone is `one_pad_cannot_start_a_local_game_but_can_free_ride`.
    menu.join = join::Join::with(&[Some(0), Some(1)], join::ON_SCREEN, join::Opened::Menu);
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
    // Settings from the pause (past the machine switch, P41, and the audio options, P56), and back
    // to it.
    for _ in 0..4 {
        tap(&mut m, &mut r, Control::Down);
    }
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Settings));
    tap(&mut m, &mut r, Control::Circle);
    assert_eq!(m.screen(), Some(Screen::Pause));
    // Restart and Main Menu.
    for _ in 0..3 {
        tap(&mut m, &mut r, Control::Up);
    }
    let acts = tap(&mut m, &mut r, Control::Cross);
    assert!(acts.contains(&Act::Restart), "{acts:?}");
    tap(&mut m, &mut r, Control::Start);
    for _ in 0..5 {
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
        music_volume: 35,
        effects_volume: 100,
        folder_volume: 5,
        crowd_volume: 0,
        engine_volume: 90,
        menu_volume: 55,
        mix: menu::Mix::Custom,
        custom: [35, 5, 100, 0, 90, 55],
        graphics: Graphics::Original,
        render_scale: RenderScale::ThreeQuarters,
        speedometer: Speedometer::Analog,
        // VSync off and a hand-written limit the row does not offer (P90).
        vsync: false,
        frame_limit: FrameLimit::Fps(75),
        // Gravity halved for everyone and 2P's bike twice as fast (P55), and a preset kept by name.
        modifiers: Mods::parse("gravity=50 bike_speed.2=200").unwrap(),
        presets: vec![("Moon".into(), Mods::parse("gravity=20").unwrap())],
        // A lobby of sixteen (N12).
        lobby_seats: 16,
    };
    let controls = Controls {
        throttle: Gas::R2,
        pad: Some("/dev/input/js1".into()),
        pressure: "ds2@8".into(),
        // One seat on the original's controls among the new (P40), which writes one word a seat.
        layouts: [Layout::New, Layout::Original, Layout::New, Layout::New, Layout::New, Layout::New, Layout::New, Layout::New],
        // And another flying the original's biplane (P50).
        planes: [Layout::New, Layout::New, Layout::Original, Layout::New, Layout::New, Layout::New, Layout::New, Layout::New],
        // 2P on the original's controller preset 2 (P79), which writes one number a seat.
        presets: [0, 2, 0, 0, 0, 0, 0, 0],
        // A Nintendo pad's B as Cross, by where it sits (P53).
        nintendo_positions: true,
    };
    menu::save(&dir, &settings, &controls).unwrap();
    assert_eq!(menu::load(&dir), (settings, controls));
    // A directory with neither file is the defaults, and so is a file of nonsense.
    let empty = scratch("empty");
    assert_eq!(menu::load(&empty), (Settings::default(), Controls::default()));
    std::fs::write(empty.join(menu::SETTINGS_FILE), "track = nowhere\nwrecks\n= on\n").unwrap();
    assert_eq!(menu::load(&empty).0, Settings::default());
    // A file saved before V26 (P36's `overlay = on`, the debug HUD then the only one) rides the
    // original's HUD (V28), and so does its `off`.
    for word in ["on", "off"] {
        std::fs::write(empty.join(menu::SETTINGS_FILE), format!("name = Rider\noverlay = {word}\nwrecks = on\n")).unwrap();
        assert_eq!(menu::load(&empty).0.overlay, Overlay::Original, "overlay = {word}");
    }
    // The defaults themselves round trip too.
    menu::save(&empty, &Settings::default(), &Controls::default()).unwrap();
    assert_eq!(menu::load(&empty), (Settings::default(), Controls::default()));
    std::fs::remove_dir_all(dir).ok();
    std::fs::remove_dir_all(empty).ok();
}

#[test]
fn the_graphics_row_swaps_the_whole_map_for_the_originals_fog() {
    let (mut m, mut r) = fresh();
    assert_eq!(m.settings.graphics, Graphics::Full, "the whole map unless asked");
    for _ in 0..4 {
        tap(&mut m, &mut r, Control::Down);
    }
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Settings));
    // Past the name, overlay, wrecks, controls, music and the two volumes.
    for _ in 0..7 {
        tap(&mut m, &mut r, Control::Down);
    }
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.graphics, Graphics::Original);
    assert!(m.take_changed());
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.settings.graphics, Graphics::Full);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.settings.graphics, Graphics::Original);
    assert!(Settings::parse(&m.settings.text()).graphics == Graphics::Original);
    assert_eq!(Settings::parse("graphics = full\n").graphics, Graphics::Full);
    assert_eq!(Settings::parse("graphics = fancy\n").graphics, Graphics::Full);
}

/// V42: the readout unless asked, and the row under the render scale's switches it to the dial and
/// back by pad, and says so to be applied and saved.
#[test]
fn the_speedometer_row_switches_the_dial_and_is_saved() {
    let (mut m, mut r) = fresh();
    assert_eq!(m.settings.speedometer, Speedometer::Digital, "the readout unless asked");
    assert_eq!(Settings::parse("").speedometer, Speedometer::Digital);
    for _ in 0..4 {
        tap(&mut m, &mut r, Control::Down);
    }
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Settings));
    for _ in 0..9 {
        tap(&mut m, &mut r, Control::Down);
    }
    tap(&mut m, &mut r, Control::Right);
    assert!(m.take_changed());
    assert_eq!(m.settings.speedometer, Speedometer::Analog);
    assert!(m.settings.text().contains("speedometer = analog\n"));
    assert_eq!(m.settings.render_scale, RenderScale::Auto, "the render scale's row is untouched");
    tap(&mut m, &mut r, Control::Right);
    assert!(m.take_changed());
    assert_eq!(m.settings.speedometer, Speedometer::Digital);
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.settings.speedometer, Speedometer::Analog);
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.take_changed());
    assert_eq!(m.settings.speedometer, Speedometer::Digital);
    assert!(m.settings.text().contains("speedometer = digital\n"));
    // Saved to the file and read back from it.
    tap(&mut m, &mut r, Control::Left);
    let dir = scratch("speedometer");
    menu::save(&dir, &m.settings, &Controls::default()).unwrap();
    assert_eq!(menu::load(&dir).0.speedometer, Speedometer::Analog);
    std::fs::remove_dir_all(&dir).unwrap();
    for style in Speedometer::ALL {
        let s = Settings { speedometer: style, ..Settings::default() };
        assert_eq!(Settings::parse(&s.text()).speedometer, style);
        assert_eq!(Speedometer::from_word(style.word()), Some(style));
    }
    // The readout when the file has a word it cannot read.
    assert_eq!(Settings::parse("speedometer = sundial\n").speedometer, Speedometer::Digital);
}

#[test]
fn the_render_scale_row_cycles_and_is_saved() {
    let (mut m, mut r) = fresh();
    assert_eq!(m.settings.render_scale, RenderScale::Auto, "auto unless asked");
    for _ in 0..4 {
        tap(&mut m, &mut r, Control::Down);
    }
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Settings));
    // The last row, under the graphics'.
    for _ in 0..8 {
        tap(&mut m, &mut r, Control::Down);
    }
    let mut seen = Vec::new();
    for _ in 0..4 {
        tap(&mut m, &mut r, Control::Right);
        assert!(m.take_changed());
        seen.push(m.settings.render_scale);
    }
    assert_eq!(seen, [RenderScale::Full, RenderScale::ThreeQuarters, RenderScale::Half, RenderScale::Auto]);
    assert_eq!(m.settings.graphics, Graphics::Full, "the graphics row is untouched");
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.settings.render_scale, RenderScale::Half);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.settings.render_scale, RenderScale::Auto);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.settings.render_scale, RenderScale::Full);
    // Down from the last row wraps to the first.
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.render_scale, RenderScale::Full);
    // Each one is written and read back, and is what `--render-scale` would say.
    for scale in RenderScale::ALL {
        let s = Settings { render_scale: scale, ..Settings::default() };
        assert!(s.text().contains(&format!("render_scale = {}\n", scale.word())));
        assert_eq!(Settings::parse(&s.text()).render_scale, scale);
    }
    assert_eq!(RenderScale::ALL.map(RenderScale::fraction), [None, Some(1.0), Some(0.75), Some(0.5)]);
    // Auto when the file has no key, or one it cannot read; a hand-written `%` reads too.
    assert_eq!(Settings::parse("graphics = original\n").render_scale, RenderScale::Auto);
    assert_eq!(Settings::parse("render_scale = 33\n").render_scale, RenderScale::Auto);
    assert_eq!(Settings::parse("render_scale = 50%\n").render_scale, RenderScale::Half);
    let dir = scratch("render-scale");
    std::fs::write(dir.join(menu::SETTINGS_FILE), "name = Rider\ngraphics = full\n").unwrap();
    assert_eq!(menu::load(&dir).0.render_scale, RenderScale::Auto);
    std::fs::remove_dir_all(dir).ok();
}

/// P90: a fresh settings file has VSync on and no frame limit; the two rows under the speedometer's
/// change by pad input, and what they say is saved and read back.
#[test]
fn the_vsync_and_frame_limit_rows_change_by_pad_and_are_saved() {
    let (mut m, mut r) = fresh();
    assert!(m.settings.vsync, "VSync on unless turned off");
    assert_eq!(m.settings.frame_limit, FrameLimit::Off);
    let dir = scratch("vsync");
    assert!(menu::load(&dir).0.vsync, "no file is VSync on");
    std::fs::write(dir.join(menu::SETTINGS_FILE), "name = Rider\nrender_scale = 75\n").unwrap();
    assert!(menu::load(&dir).0.vsync, "a file from before P90 is VSync on");
    assert_eq!(menu::load(&dir).0.frame_limit, FrameLimit::Off);

    for _ in 0..4 {
        tap(&mut m, &mut r, Control::Down);
    }
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Settings));
    for _ in 0..10 {
        tap(&mut m, &mut r, Control::Down);
    }
    tap(&mut m, &mut r, Control::Right);
    assert!(m.take_changed());
    assert!(!m.settings.vsync);
    assert_eq!(m.settings.speedometer, Speedometer::Digital, "the speedometer's row is untouched");
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.settings.vsync);
    tap(&mut m, &mut r, Control::Left);
    assert!(!m.settings.vsync);

    tap(&mut m, &mut r, Control::Down);
    let mut seen = Vec::new();
    for _ in 0..FrameLimit::ALL.len() {
        tap(&mut m, &mut r, Control::Right);
        assert!(m.take_changed());
        seen.push(m.settings.frame_limit);
    }
    assert_eq!(seen[..6], FrameLimit::ALL[1..]);
    assert_eq!(seen[6], FrameLimit::Off, "round from the refresh rate to off");
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.settings.frame_limit, FrameLimit::Refresh);
    tap(&mut m, &mut r, Control::Left);
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.settings.frame_limit, FrameLimit::Fps(144));
    assert!(!m.settings.vsync, "the VSync row is untouched");
    // The next row is still the modifiers', and the last.
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Modifiers));

    let text = m.settings.text();
    assert!(text.contains("vsync = off\n") && text.contains("frame_limit = 144\n"), "{text}");
    menu::save(&dir, &m.settings, &m.controls).unwrap();
    let back = menu::load(&dir).0;
    assert_eq!((back.vsync, back.frame_limit), (false, FrameLimit::Fps(144)));
    for limit in FrameLimit::ALL {
        assert_eq!(FrameLimit::from_word(&limit.word()), Some(limit));
        let s = Settings { frame_limit: limit, ..Settings::default() };
        assert_eq!(Settings::parse(&s.text()).frame_limit, limit);
    }
    assert_eq!(FrameLimit::from_word("90fps"), Some(FrameLimit::Fps(90)));
    assert_eq!(Settings::parse("frame_limit = fast\n").frame_limit, FrameLimit::Off);
    assert_eq!(Settings::parse("frame_limit = 5000\n").frame_limit, FrameLimit::Off);
    assert_eq!(FrameLimit::Refresh.fps(165.0), Some(165.0));
    assert_eq!(FrameLimit::Fps(60).fps(165.0), Some(60.0));
    assert_eq!(FrameLimit::Off.fps(165.0), None);
    std::fs::remove_dir_all(dir).ok();
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

/// How far apart two RGB frames are: the mean difference per channel, and the share of pixels with a
/// channel more than 32 off.
fn apart(a: &[u8], b: &[u8]) -> (f64, f64) {
    let sum: u64 = a.iter().zip(b).map(|(&x, &y)| x.abs_diff(y) as u64).sum();
    let off = a.chunks(3).zip(b.chunks(3)).filter(|(x, y)| x.iter().zip(y.iter()).any(|(&p, &q)| p.abs_diff(q) > 32)).count();
    (sum as f64 / a.len() as f64, off as f64 / (a.len() / 3) as f64)
}

/// V30: the GPU draws every screen as sprites the way the CPU draws it into a frame — the same
/// boxes, the same texels read, the same blending — so a 4K menu costs the CPU a few thousand
/// vertices instead of a frame drawn and sent.
#[test]
fn the_gpu_draws_every_screen_as_the_cpu_does() {
    let mut disc = match game_render::Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let mut gpu = match game_render::gpu::Renderer::headless() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let art = menu::Art::load(&mut disc).expect("the menus' art");
    gpu.set_images(&art.images());
    // Two shapes: the layout's own scaled up, and a wide one whose sides the layout leaves.
    for (w, h) in [(1280usize, 896usize), (1600, 720)] {
        let texture = gpu.offscreen(w as u32, h as u32);
        let view = texture.create_view(&Default::default());
        let target = game_render::gpu::Target { view: &view, size: (w as u32, h as u32), frame: [0, 0, w as u32, h as u32], scale: 1 };
        for s in Screen::ALL {
            let mut m = Menu::new(Settings::default(), Controls::default());
            m.open(s);
            m.tick = 40;
            let mut frame = game_render::Frame::new(w, h, [0, 0, 0]);
            art.draw(&m, &mut frame);
            let mut sprites = Vec::new();
            art.sprites(&m, (w, h), 0, &mut sprites);
            gpu.draw_sprites(&target, &sprites);
            let got = gpu.read(&texture);
            let (mean, off) = apart(&frame.color, &got);
            eprintln!("{s:?} {w}x{h}: {} sprites, {mean:.3} apart on average, {:.4}% of pixels off", sprites.len(), off * 100.0);
            assert!(sprites.len() > 3, "{s:?}: {} sprites", sprites.len());
            assert!(mean < 1.0 && off < 0.002, "{s:?} {w}x{h}: {mean:.3} apart on average, {:.3}% of pixels off", off * 100.0);
        }
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
                        | Screen::Modifiers
                        | Screen::Audio
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
    // Four list screens, the three online ones (N3), the split layout (P43), the game modifiers (P55),
    // the keyboard and the pause's audio options (P56), twelve looks each, at four shapes.
    assert_eq!(pills, 11 * 12 * 4);
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

// ---- the rider's look (P86) ----------------------------------------------------------------------

/// `n` taps of `control` on a seat's garage column; seat 0 is the menu's own pad.
fn taps(m: &mut Menu, reader: &mut Reader, seat: usize, control: Control, n: usize) {
    for _ in 0..n {
        match seat {
            0 => {
                tap(m, reader, control);
            }
            _ => tap_garage_seat(m, reader, seat, control),
        }
    }
}

/// Type `text` on a seat's open keys from the cursor at the first key: walk to each character's key,
/// Cross on it.
fn type_on_keys(m: &mut Menu, reader: &mut Reader, seat: usize, text: &str) {
    for c in text.chars() {
        let e = m.garage(seat).unwrap().entry.clone().expect("the keys are open");
        let keys = e.keys();
        let (row, col) = keys
            .iter()
            .enumerate()
            .find_map(|(r, k)| k.find(c.to_ascii_lowercase()).map(|i| (r, i)))
            .unwrap_or_else(|| panic!("no key for {c:?}"));
        let (at_row, at_col) = e.key;
        let down = (row + keys.len() + 1 - at_row) % (keys.len() + 1);
        taps(m, reader, seat, Control::Down, down);
        let now = m.garage(seat).unwrap().entry.as_ref().unwrap().key;
        assert_eq!(now.0, row, "walked down to {c:?}'s row from ({at_row}, {at_col})");
        let width = keys[row].len();
        taps(m, reader, seat, Control::Right, (col + width - now.1) % width);
        assert_eq!(m.garage(seat).unwrap().entry.as_ref().unwrap().key, (row, col));
        taps(m, reader, seat, Control::Cross, 1);
    }
}

#[test]
fn a_seat_dresses_its_rider_saves_the_look_and_a_second_seat_rides_it_after_a_restart() {
    use roster::{Catalogue, Piece, Picks, Presets, Row, ROWS};

    let mut disc = match game_render::Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let catalogue = Catalogue::read(&mut disc).unwrap();
    assert!(!catalogue.wardrobe.outfits.is_empty() && Piece::ALL.iter().all(|&p| catalogue.wardrobe.pieces[p as usize].len() > 1));
    let row = |r: Row| ROWS.iter().position(|&x| x == r).unwrap();
    let mut reader = Reader::new();
    let mut m = Menu::new(Settings::default(), Controls::default()).with_garage(catalogue.clone(), Picks::default()).with_looks(Presets::default());
    m.step(reader.read(&Report::idle()));
    let mut two = Reader::new();
    m.step_seat(1, two.read(&Report::idle()));
    m.seated = 2;
    taps(&mut m, &mut reader, 0, Control::Down, 3);
    tap(&mut m, &mut reader, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Garage));
    let stock = m.garage(0).unwrap().pick.clone();

    // Seat 1: the next colours, the next jersey and pants.
    taps(&mut m, &mut reader, 0, Control::Down, row(Row::Colours));
    tap(&mut m, &mut reader, Control::Right);
    tap(&mut m, &mut reader, Control::Down);
    tap(&mut m, &mut reader, Control::Right);
    // Its name: Cross opens the keys, the letters, Start keeps them.
    tap(&mut m, &mut reader, Control::Down);
    assert_eq!(m.garage(0).unwrap().row, row(Row::Name));
    tap(&mut m, &mut reader, Control::Cross);
    type_on_keys(&mut m, &mut reader, 0, "Kaito");
    assert_eq!(m.screen(), Some(Screen::Garage), "typing does not leave the garage");
    tap(&mut m, &mut reader, Control::Start);
    assert_eq!(m.garage(0).unwrap().entry, None);
    // Its number: one key too many is deleted with Square, then Done on the command row.
    tap(&mut m, &mut reader, Control::Down);
    tap(&mut m, &mut reader, Control::Cross);
    type_on_keys(&mut m, &mut reader, 0, "720");
    tap(&mut m, &mut reader, Control::Square);
    let e = m.garage(0).unwrap().entry.clone().unwrap();
    taps(&mut m, &mut reader, 0, Control::Down, e.keys().len() - e.key.0);
    let e = m.garage(0).unwrap().entry.clone().unwrap();
    let done = e.commands().iter().position(|c| *c == "Done").unwrap();
    taps(&mut m, &mut reader, 0, Control::Right, (done + e.commands().len() - e.key.1) % e.commands().len());
    tap(&mut m, &mut reader, Control::Cross);
    assert_eq!(m.garage(0).unwrap().entry, None, "Done closes the keys");
    // A helmet, the second goggles and the last boots.
    tap(&mut m, &mut reader, Control::Down);
    tap(&mut m, &mut reader, Control::Right);
    tap(&mut m, &mut reader, Control::Down);
    taps(&mut m, &mut reader, 0, Control::Right, 2);
    taps(&mut m, &mut reader, 0, Control::Down, 2);
    tap(&mut m, &mut reader, Control::Left);
    let one = m.garage(0).unwrap().pick.clone();
    println!("seat 1: {} | {}", one.rider, one.look.text());
    assert_eq!((one.look.name.as_str(), one.look.number.as_str()), ("Kaito", "72"));
    assert_ne!(one.rider, stock.rider, "the colours changed the gear set");
    assert!(!one.look.outfit.is_empty());
    assert!(Piece::ALL.iter().filter(|&&p| p != Piece::Gloves).all(|&p| !one.look.piece(p).is_empty()), "{:?}", one.look);
    assert_eq!(one.look.piece(Piece::Gloves), "", "gloves stay the gear set's");
    assert_eq!(one.look.piece(Piece::Boots), catalogue.wardrobe.pieces[Piece::Boots as usize].last().unwrap());

    // Cross on Preset saves it under its name and number.
    tap(&mut m, &mut reader, Control::Down);
    assert_eq!(m.garage(0).unwrap().row, row(Row::Preset));
    tap(&mut m, &mut reader, Control::Cross);
    assert!(m.take_looks_changed());
    assert_eq!(m.looks.keys(), ["Kaito 72"]);
    assert_eq!(m.screen(), Some(Screen::Garage), "saving does not leave the garage");

    // Seat 2, on its own pad: up from the top is the Preset row, right loads the saved look.
    tap_garage_seat(&mut m, &mut two, 1, Control::Up);
    assert_eq!(m.garage(1).unwrap().row, row(Row::Preset));
    assert_ne!(m.garage(1).unwrap().pick.look, one.look);
    tap_garage_seat(&mut m, &mut two, 1, Control::Right);
    let loaded = m.garage(1).unwrap().pick.clone();
    assert_eq!((&loaded.bike, loaded.set, &loaded.rider, &loaded.look, loaded.preset.as_str()), (&one.bike, one.set, &one.rider, &one.look, "Kaito 72"));

    // Circle leaves; the picks carry the looks into the ride.
    tap(&mut m, &mut reader, Control::Circle);
    assert_eq!(m.screen(), Some(Screen::Main));
    let picks = m.ride_picks(2).unwrap();
    assert_eq!(picks[0].look, picks[1].look);
    assert_eq!(picks[0].preset, "Kaito 72", "the first seat's save is its pick too");
    assert_eq!(m.lobby.pick.look, one.look.text(), "the online pick sends the look");

    // The ride dresses both alike: the same body repaint and the same painted name and number.
    let mut kit = dress::Kit::open(&mut disc);
    let dressed: Vec<_> = picks
        .iter()
        .map(|p| {
            let rider = catalogue.rider(&p.rider).unwrap();
            let suit = game_render::rider::Suit { materials: rider.suit.clone(), ..Default::default() };
            let tex = kit.texture(&p.rider, &p.look, &p.bike).expect("a name to paint");
            (kit.repaint(&suit, &p.look), tex.rgba)
        })
        .collect();
    assert_eq!(dressed[0], dressed[1]);
    assert!(dressed[0].1.chunks(4).any(|t| t[3] > 0), "the name and number drew");

    // A restart: the files written and read back by a fresh menu, where a seat with no pick of its
    // own loads the look by its name.
    let dir = scratch("looks");
    m.picks.save(&dir).unwrap();
    m.looks.save(&dir).unwrap();
    let again = Menu::new(Settings::default(), Controls::default()).with_garage(catalogue.clone(), Picks::load(&dir)).with_looks(Presets::load(&dir));
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(again.looks, m.looks);
    assert_eq!(again.ride_picks(2).unwrap(), picks, "each seat's look and preset pick are remembered");
    let mut third = roster::Garage::new(&catalogue, &roster::Pick::default());
    assert!(third.load_preset(&catalogue, &again.looks, "kaito 72"));
    assert_eq!(third.pick, picks[1]);
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

/// Pad input alone, from the main menu to the ride: Local Game by pad seat 0 (the host), Cross from
/// each of `joiners`, Start from the host, the split screen's first row, a track. The local game it
/// starts, if it does.
fn local_game(pads: usize, joiners: &[usize]) -> (Menu, Vec<Act>) {
    let all: Vec<usize> = (0..pads).collect();
    let (mut m, mut rd) = fresh_with(pads);
    tap_device(&mut m, &mut rd, &all, 0, Control::Down);
    tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Join));
    for &d in joiners {
        tap_device(&mut m, &mut rd, &all, d, Control::Cross);
    }
    let mut acts = tap_device(&mut m, &mut rd, &all, 0, Control::Start);
    if m.screen() == Some(Screen::Split) {
        acts.extend(tap_device(&mut m, &mut rd, &all, 0, Control::Cross));
        assert_eq!(m.screen(), Some(Screen::Tracks));
        acts.extend(tap_device(&mut m, &mut rd, &all, 0, Control::Cross));
    }
    (m, acts)
}

#[test]
fn two_or_three_pads_start_a_local_game_with_a_view_each() {
    // Two (P51): the host and one more is enough; the ride has the original's two-player layout.
    for split in [split::Split::Stacked, split::Split::SideBySide] {
        let (mut m, mut rd) = fresh_with(2);
        m.settings.split = split;
        let all = [0, 1];
        tap_device(&mut m, &mut rd, &all, 0, Control::Down);
        tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
        tap_device(&mut m, &mut rd, &all, 1, Control::Cross);
        assert!(m.join.can_start());
        tap_device(&mut m, &mut rd, &all, 0, Control::Start);
        assert_eq!(m.screen(), Some(Screen::Split));
        // The split screen opens on the setting's layout; Cross takes it.
        tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
        let acts = tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
        assert!(acts.iter().any(|a| matches!(a, Act::Ride(_))), "{acts:?}");
        assert_eq!(m.riders, 2);
        let party = m.party().expect("a local game");
        assert_eq!(party.devices, vec![0, 1], "each player on their own pad");
        let want = match split {
            split::Split::Stacked => split::Layout::Stacked,
            split::Split::SideBySide => split::Layout::SideBySide,
        };
        assert_eq!(party.layout(), want);
        assert_eq!(party.layout().rects(640, 448).len(), 2, "exactly two views");
        assert_eq!(m.ride_picks(party.players()).map_or(2, |p| p.len()), 2);
    }
    let (m, acts) = local_game(2, &[1]);
    assert!(acts.iter().any(|a| matches!(a, Act::Ride(_))), "{acts:?}");
    assert_eq!(m.party().map(|p| p.layout()), Some(split::Layout::for_players(2, m.settings.split).unwrap()));

    // Three, of four pads plugged in: three views in the port's quarters, the fourth quarter nobody's
    // (the ride draws it black), and each rides their own pad.
    let (m, acts) = local_game(4, &[3, 1]);
    assert!(acts.iter().any(|a| matches!(a, Act::Ride(_))), "{acts:?}");
    let party = m.party().expect("a local game");
    assert_eq!((party.players(), m.riders), (3, 3));
    assert_eq!(party.devices, vec![0, 3, 1], "seats in join order, each on its own pad");
    assert_eq!(party.layout(), split::Layout::Quarters);
    assert_eq!(party.layout().rects(640, 448).len(), 4);
    assert_eq!(menu::layouts(3), vec![split::Layout::Quarters]);

    // Four still start, and Start may come from any seated pad, not only the host.
    let (m, acts) = local_game(4, &[1, 2, 3]);
    assert!(acts.iter().any(|a| matches!(a, Act::Ride(_))), "{acts:?}");
    assert_eq!(m.party().map(|p| p.players()), Some(4));
}

#[test]
fn one_pad_cannot_start_a_local_game_but_can_free_ride() {
    // Alone on the join screen, Start stays there, with other pads plugged in that have not joined.
    for pads in [1, 2] {
        let (m, acts) = local_game(pads, &[]);
        assert_eq!(m.screen(), Some(Screen::Join), "{pads} pads plugged in, one joined");
        assert!(acts.iter().all(|a| *a == Act::Stay), "{acts:?}");
        assert!(!m.join.can_start());
        assert!(m.party().is_none());
    }
    // A pad that joins and leaves again leaves the host alone.
    let all = [0, 1];
    let (mut m, mut rd) = fresh_with(2);
    tap_device(&mut m, &mut rd, &all, 0, Control::Down);
    tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
    tap_device(&mut m, &mut rd, &all, 1, Control::Cross);
    tap_device(&mut m, &mut rd, &all, 1, Control::Circle);
    tap_device(&mut m, &mut rd, &all, 0, Control::Start);
    assert_eq!(m.screen(), Some(Screen::Join));
    // Back on the main menu, the same pad rides a Free Ride on its own.
    tap_device(&mut m, &mut rd, &all, 0, Control::Circle);
    tap_device(&mut m, &mut rd, &all, 1, Control::Circle);
    assert_eq!(m.screen(), Some(Screen::Main));
    tap_device(&mut m, &mut rd, &all, 0, Control::Up);
    tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Tracks));
    let acts = tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
    assert!(acts.iter().any(|a| matches!(a, Act::Ride(_))), "{acts:?}");
    assert!(m.party().is_none());
    assert_eq!(m.riders, 1);
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
    assert_eq!(m.music_label(), "In Order");
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.music, "disc:ooc");
    assert_eq!(m.music_label(), "Out of Control - Hoobastank");
    tap(&mut m, &mut r, Control::Right);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.music, "off", "past the last song it wraps to Off");
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.settings.music, "folder:mine.wav");
    // The music volume, down to nothing and no further, by fives from 70 (P56).
    tap(&mut m, &mut r, Control::Down);
    for _ in 0..16 {
        tap(&mut m, &mut r, Control::Left);
    }
    assert_eq!(m.settings.music_volume, 0);
    // The effects, up to the top and no further.
    tap(&mut m, &mut r, Control::Down);
    for _ in 0..6 {
        tap(&mut m, &mut r, Control::Right);
    }
    assert_eq!(m.settings.effects_volume, menu::VOLUME_MAX);
    assert!(m.take_changed());
    let back = Settings::parse(&m.settings.text());
    assert_eq!((back.music.as_str(), back.music_volume, back.effects_volume), ("folder:mine.wav", 0, 100));
    assert_eq!(back.mix, menu::Mix::Custom, "a slider moved is the user's own mix");
    // An older file, with no sound keys, opens on the defaults: the original's Game Mix.
    let old = Settings::parse("name = Old\n");
    assert_eq!((old.music.as_str(), old.music_volume, old.effects_volume), ("shuffle", 70, 80));
    assert_eq!(old.mix, menu::Mix::Game);
    // One from before P56 counted its two sliders to ten.
    let tens = Settings::parse("music_volume = 3\neffects_volume = 10\n");
    assert_eq!((tens.music_volume, tens.effects_volume, tens.mix), (30, 100, menu::Mix::Custom));
}

// ---- presses between frames, and pads that are not Xbox pads (P53) --------------------------------

/// The menu frames of a ride, at the frame rate of the user's 4K GPU window: a press and release
/// that both land between two of them are in neither frame's report, only in its taps — and the
/// reader takes them as the press they were.
#[test]
fn a_press_and_release_between_two_frames_still_opens_the_pause_and_moves_its_cursor() {
    let (mut m, mut r) = fresh();
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.riding());
    // The held state alone, as before: the pad was at rest at both frames, so nothing happened.
    m.step(r.read(&Report::idle()));
    assert!(m.riding());
    // With the frame's taps: Start, then Down twice inside one frame, then Cross.
    m.step(r.read_taps(&Report::idle(), Control::Start.mask()));
    assert_eq!(m.screen(), Some(Screen::Pause));
    assert_eq!(m.cursor(Screen::Pause), 0);
    m.step(r.read_taps(&Report::idle(), Control::Down.mask()));
    assert_eq!(m.cursor(Screen::Pause), 1);
    m.step(r.read_taps(&Report::idle(), 0));
    assert_eq!(m.cursor(Screen::Pause), 1);
    let act = m.step(r.read_taps(&Report::idle(), Control::Cross.mask()));
    assert_eq!(act, Act::Restart);
    // A press still held at the frame is one press, not two: the tap and the held bit together.
    m.pause();
    let down = Report::from_buttons(Control::Down.mask());
    m.step(r.read_taps(&Report::idle(), 0));
    let at = m.cursor(Screen::Pause);
    m.step(r.read_taps(&down, Control::Down.mask()));
    assert_eq!(m.cursor(Screen::Pause), at + 1);
}

/// A Switch Pro Controller in hid-nintendo's own numbering, its keys and axes from the module.
fn pro_controller(dev: &std::path::Path, sys: &std::path::Path, node: &str) -> pad::tests::FakePad {
    let name = "Nintendo Switch Pro Controller";
    pad::tests::sys_device(sys, node, name, (0x057e, 0x2009), &pad::tests::PRO_KEYS, &pad::tests::PRO_ABS, &[]);
    let mut pro = pad::tests::FakePad::plug(dev, node);
    pro.init(14, &[0; 6]);
    pro
}

/// The ride's frame of the shell's seats, as main.rs takes it: 1P's report as the pad sent it, its
/// presses since the last frame, and the report the ride's scheme makes of it.
fn seat_frame(seats: &mut pad::Seats) -> (Report, u16) {
    let now = pad::tests::frame(seats, &|_| false);
    (now[0].report(), seats.taps(0))
}

/// The user's pad, seated as 1P, in a ride: its + opens the pause, the hat's down moves the cursor,
/// and A takes the row — tapped between frames, and held across them. The report the new scheme
/// rides on has no Cross in it (R2 is the gas there), which is why the pause reads the pad's own.
#[test]
fn a_seated_pad_s_start_opens_the_pause_and_its_d_pad_and_cross_work_it() {
    let _fifos = pad::tests::FIFOS.lock().unwrap_or_else(|e| e.into_inner());
    let root = pad::tests::scratch("menu-pause");
    let (dev, sys) = (root.join("dev"), root.join("sys"));
    std::fs::create_dir_all(&dev).unwrap();
    let mut pro = pro_controller(&dev, &sys, "js0");
    let mut seats = pad::tests::seats_in(&dev, &sys, pad::Wiring::default());
    let id = match &pad::tests::devices_now(&mut seats, &|_| false)[1].0 {
        pad::Device::Pad(id) => id.clone(),
        other => panic!("{other:?}"),
    };
    seats.set_first(Some(pad::Device::Pad(id)));

    let (mut m, mut r) = fresh();
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.riding());
    let mut step = |m: &mut Menu, seats: &mut pad::Seats| {
        let (report, taps) = seat_frame(seats);
        m.step_seats(&[r.read_taps(&report, taps)])
    };
    step(&mut m, &mut seats);

    // + (js 10), down and up before the frame.
    pro.button(10, true);
    pro.button(10, false);
    step(&mut m, &mut seats);
    assert_eq!(m.screen(), Some(Screen::Pause));
    // The hat's down, the same.
    pro.axis(5, i16::MAX);
    pro.axis(5, 0);
    step(&mut m, &mut seats);
    assert_eq!(m.cursor(Screen::Pause), 1);
    // A (js 1), held across a frame: the scheme's report has lost it, the pad's own has not.
    pro.button(1, true);
    let (report, taps) = seat_frame(&mut seats);
    assert!(report.buttons() & Control::Cross.mask() != 0);
    assert!(pad::Scheme::New.ride(report).buttons() & Control::Cross.mask() == 0, "the new scheme moves R2 onto Cross and Cross off");
    let act = m.step_seats(&[r.read_taps(&report, taps)]);
    assert_eq!(act, Act::Restart);
    pro.button(1, false);
    seat_frame(&mut seats);
    std::fs::remove_dir_all(&root).unwrap();
}

/// The splash at any frame rate: an idle pad plugged in first (a virtual one some program left,
/// its stick wandering), and the user's pad's A pressed for three 60 Hz ticks somewhere — the
/// menu's frames every tick, or every twelfth (5 fps), at every phase. The user's pad is 1P.
#[test]
fn on_the_splash_a_pad_s_cross_makes_it_1p_even_while_an_idle_virtual_pad_is_plugged_in_at_any_frame_rate() {
    let _fifos = pad::tests::FIFOS.lock().unwrap_or_else(|e| e.into_inner());
    let root = pad::tests::scratch("menu-splash");
    let (dev, sys) = (root.join("dev"), root.join("sys"));
    std::fs::create_dir_all(&dev).unwrap();
    pad::tests::sys_device(&sys, "js0", "Microsoft X-Box 360 pad", (0x045e, 0x028e), &pad::tests::XPAD_KEYS, &pad::tests::PAD_ABS, &[]);
    let mut idle = pad::tests::FakePad::plug(&dev, "js0");
    idle.init(11, &[0, 0, -32767, 0, 0, -32767, 0, 0]);
    let mut pro = pro_controller(&dev, &sys, "js1");
    let dev_of = |d: pad::Device<String>| match d {
        pad::Device::Keys => Dev::Keys,
        pad::Device::Pad(id) => Dev::Pad(id),
    };
    for every in [1u32, 2, 3, 4, 6, 12] {
        for phase in 0..every {
            let mut seats = pad::tests::seats_in(&dev, &sys, pad::Wiring::default());
            let (mut m, mut readers) = on_the_splash();
            let mut users = None;
            for tick in 0..120u32 {
                // The idle pad's stick, every tick.
                let wander = |k: u32| (((tick * k) % 160) as i32 - 80) * 250;
                idle.axis(0, wander(7) as i16);
                idle.axis(1, wander(13) as i16);
                if tick == 60 + phase {
                    pro.button(1, true);
                }
                if tick == 63 + phase {
                    pro.button(1, false);
                }
                if tick % every != 0 {
                    continue;
                }
                let got = pad::tests::devices_now(&mut seats, &|_| false);
                users = users.or_else(|| got.iter().map(|(d, _, _)| d).nth(2).cloned().map(dev_of));
                let reports: Vec<(Dev, Report, u16)> = got.into_iter().map(|(d, r, t)| (dev_of(d), r, t)).collect();
                m.step_devices(&readers.read_taps(&reports));
                if tick < 60 {
                    assert_eq!(m.p1(), None, "every {every} phase {phase} tick {tick}");
                }
            }
            assert_eq!(m.p1(), users.as_ref(), "every {every} phase {phase}");
            assert!(users.is_some());
        }
    }
    std::fs::remove_dir_all(&root).unwrap();
}

/// P55: Settings' last row opens the game modifiers; the arrows step a modifier by ten percent, for
/// every player or for one; a preset saved by name loads back; and Reset To Original puts every
/// modifier back, which writes the settings file as it was before there were modifiers.
#[test]
fn the_game_modifiers_screen_sets_saves_and_resets() {
    let (mut m, mut r) = fresh();
    m.open(Screen::Settings);
    // Up from the first row wraps to the last: Game Modifiers.
    tap(&mut m, &mut r, Control::Up);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Modifiers));
    let rows = m.modifier_rows();
    assert_eq!(rows.len(), 1 + Knob::ALL.len() + 4, "{rows:?}");
    assert!(rows.last().unwrap().contains("Reset To Original"), "{rows:?}");

    // Gravity, a world modifier, down twice: 80 for everyone.
    let gravity = 1 + Knob::ALL.iter().position(|k| *k == Knob::Gravity).unwrap();
    for _ in 0..gravity {
        tap(&mut m, &mut r, Control::Down);
    }
    tap(&mut m, &mut r, Control::Left);
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.settings.modifiers.get(0, Knob::Gravity), 80);
    assert_eq!(m.settings.modifiers.get(3, Knob::Gravity), 80);
    assert!(m.take_changed());

    // Bike top speed for 2P alone: the player row to 2P, then the speed row right once.
    for _ in 0..gravity {
        tap(&mut m, &mut r, Control::Up);
    }
    tap(&mut m, &mut r, Control::Right);
    tap(&mut m, &mut r, Control::Right);
    assert!(m.modifier_rows()[0].contains("2P"), "{:?}", m.modifier_rows());
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.modifiers.get(1, Knob::BikeSpeed), 110);
    assert_eq!(m.settings.modifiers.get(0, Knob::BikeSpeed), 100);
    assert!(m.settings.modifiers.seat_is_original(0) == false, "gravity reaches 1P too");

    // Saved as "Low", on the keyboard: L, o, w over the first letter row's keys is long-winded, so
    // the name is typed the way the keyboard test does — through its own text.
    let save = 1 + Knob::ALL.len() + 1;
    let preset = save - 1;
    while m.cursor(Screen::Modifiers) != save {
        tap(&mut m, &mut r, Control::Down);
    }
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Keyboard));
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Start);
    assert_eq!(m.screen(), Some(Screen::Modifiers));
    assert_eq!(m.settings.presets.len(), 1);
    let saved = m.settings.modifiers;
    assert_eq!(m.settings.presets[0].1, saved);

    // Reset To Original, and the preset loads it all back.
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.settings.modifiers.is_original());
    while m.cursor(Screen::Modifiers) != preset {
        tap(&mut m, &mut r, Control::Up);
    }
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.settings.modifiers, saved);

    // The file keeps both, and an original set with no presets writes no modifier line at all.
    let dir = scratch("modifiers");
    menu::save(&dir, &m.settings, &m.controls).unwrap();
    assert_eq!(menu::load(&dir).0, m.settings);
    let plain = Settings::default();
    assert!(!plain.text().contains("modifiers") && !plain.text().contains("preset"), "{}", plain.text());
    std::fs::remove_dir_all(dir).ok();
    // A modifier line the reader cannot make sense of is the original game, not half a set.
    assert!(Settings::parse("modifiers = gravity=50 warp=9\n").modifiers.is_original());
}

// ---- the pause's Game Audio Options (P56) -----------------------------------------------------------

/// Songs that never end, each its own level, so which one is playing is heard as well as named.
struct Hum;

struct Level(i16, usize);

impl audio::Stream for Level {
    fn rate(&self) -> u32 {
        audio::RATE
    }
    fn read(&mut self, out: &mut [i16]) -> usize {
        let n = (out.len() / 2).min(self.1);
        out[..n * 2].fill(self.0);
        self.1 -= n;
        n
    }
}

impl audio::MusicSource for Hum {
    fn name(&self) -> &str {
        "hum"
    }
    fn tracks(&self) -> Vec<audio::Track> {
        ["one", "two", "three"]
            .into_iter()
            .map(|t| audio::Track { id: format!("hum:{t}"), title: t.to_string(), artist: "Hum".into() })
            .collect()
    }
    fn open(&self, id: &str) -> Result<Box<dyn audio::Stream>, String> {
        let level = match id {
            "hum:one" => 1000,
            "hum:two" => 2000,
            "hum:three" => 3000,
            _ => return Err(format!("no {id}")),
        };
        Ok(Box::new(Level(level, audio::RATE as usize * 600)))
    }
}

/// The shell's sound, as `start_sound` makes it, on [`Hum`]'s songs and with nothing playing it out.
fn hum_speakers(settings: &Settings) -> audio::Speakers {
    let mut music = audio::Music::new(vec![std::sync::Arc::new(Hum)], 5);
    music.wait = true;
    let speakers = audio::Speakers::start(music, false);
    speakers.apply(&settings.music, levels(settings));
    speakers
}

/// main.rs's `levels`.
fn levels(s: &Settings) -> audio::Levels {
    audio::Levels {
        music: s.music_volume,
        folder: s.folder_volume,
        effects: s.effects_volume,
        crowd: s.crowd_volume,
        engine: s.engine_volume,
        menu: s.menu_volume,
    }
}

/// A tenth of a second of the mix.
fn hum_listen(speakers: &audio::Speakers) -> Vec<i16> {
    let mut out = vec![0i16; audio::RATE as usize / 10 * 2];
    speakers.mixer().lock().unwrap().render(&mut out);
    out
}

/// [`tap`] with the shell's sound around it, as its menu loop has it: the song playing told to the
/// menu before each frame, and after one that changed the settings, `Front::hear` and the save.
fn tap_heard(m: &mut Menu, r: &mut Reader, speakers: &audio::Speakers, dir: &std::path::Path, control: Control) {
    for report in [Report::from_buttons(control.mask()), Report::idle()] {
        m.music_now = speakers.now();
        m.step(r.read(&report));
        if m.changed() {
            speakers.apply(&m.settings.music, levels(&m.settings));
            for tune in m.take_tunes() {
                match tune {
                    menu::Tune::Play(id) => speakers.play_now(&id),
                    menu::Tune::Skip => speakers.skip(),
                }
            }
            if m.take_changed() {
                menu::save(dir, &m.settings, &m.controls).unwrap();
            }
        }
        hum_listen(speakers);
    }
}

#[test]
fn the_pause_picks_a_song_skips_it_and_its_music_slider_is_kept() {
    let dir = scratch("p56");
    let (mut m, mut r) = fresh();
    let speakers = hum_speakers(&m.settings);
    m.music_choices = speakers.choices();
    let tap = |m: &mut Menu, r: &mut Reader, c: Control| tap_heard(m, r, &speakers, &dir, c);
    // A ride, and its pause.
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.riding());
    tap(&mut m, &mut r, Control::Start);
    assert_eq!(m.screen(), Some(Screen::Pause));
    assert_eq!(m.pause_rows()[3], "Game Audio Options");
    for _ in 0..3 {
        tap(&mut m, &mut r, Control::Down);
    }
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Audio));
    // The shuffle the defaults ask for is playing, and the song row starts on its song.
    let first = speakers.now().expect("a song under Shuffle");
    let rows = m.audio_rows();
    assert_eq!(rows[0], format!("Song  {}", m.music_choices.iter().find(|c| c.0 == first).unwrap().1));
    assert_eq!(rows[3..].to_vec(), ["Audio Presets  Game Mix", "Music  70", "Sound FX  80", "Crowd  45", "Engine  60", "Menu FX  100", "User Soundtracks  70"]);
    assert_eq!(m.audio_ticker(), format!("Now playing {}", m.music_choices.iter().find(|c| c.0 == first).unwrap().1));

    // Right steps the row to the next song without playing it; Cross plays it.
    let ids: Vec<String> = m.music_choices.iter().map(|c| c.0.clone()).collect();
    let at = ids.iter().position(|i| *i == first).unwrap();
    let picked = ids[(at + 1) % ids.len()].clone();
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(speakers.now().as_deref(), Some(first.as_str()));
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(speakers.now(), Some(picked.clone()), "the song picked plays");
    let level = |id: &str| match id {
        "hum:one" => 1000.0,
        "hum:two" => 2000.0,
        _ => 3000.0,
    };
    let heard = hum_listen(&speakers);
    assert!((heard[100] as f32 - level(&picked) * 0.7).abs() <= 2.0, "it is heard at the music's 70: {}", heard[100]);

    // In Order, then Skip Song: the song after it.
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.audio_rows()[2], "Play Order  In Order");
    assert_eq!(speakers.now(), Some(picked.clone()), "the order changing keeps the song");
    tap(&mut m, &mut r, Control::Up);
    tap(&mut m, &mut r, Control::Cross);
    let next = ids[(ids.iter().position(|i| *i == picked).unwrap() + 1) % ids.len()].clone();
    assert_eq!(speakers.now(), Some(next.clone()), "skip moves to the next");

    // The music slider, down a step: the mixer's music gain with it, while paused, and the mix is
    // the user's own.
    for _ in 0..3 {
        tap(&mut m, &mut r, Control::Down);
    }
    assert!(m.audio_rows()[m.cursor(Screen::Audio)].starts_with("Music  "));
    tap(&mut m, &mut r, Control::Left);
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.settings.music_volume, 60);
    assert_eq!(m.settings.mix, menu::Mix::Custom);
    assert_eq!(speakers.mixer().lock().unwrap().music_gain(), 0.6);
    let heard = hum_listen(&speakers);
    assert!((heard[100] as f32 - level(&next) * 0.6).abs() <= 2.0, "{}", heard[100]);
    // Back is the pause, the ride still paused under it.
    tap(&mut m, &mut r, Control::Circle);
    assert_eq!(m.screen(), Some(Screen::Pause));

    // A restart: the settings read back from where the shell saved them, into a new sound.
    let (settings, controls) = menu::load(&dir);
    assert_eq!((settings.music.as_str(), settings.music_volume, settings.mix), ("order", 60, menu::Mix::Custom));
    let again = hum_speakers(&settings);
    let mut m = Menu::new(settings, controls);
    m.music_choices = again.choices();
    assert_eq!(again.mixer().lock().unwrap().music_gain(), 0.6);
    m.open(Screen::Audio);
    assert_eq!(m.audio_rows()[4], "Music  60");
    assert_eq!(m.audio_rows()[3], "Audio Presets  Custom Mix");

    // The presets set every slider, and Custom brings the user's back.
    let (mut m, mut r) = (m, Reader::new());
    m.step(r.read(&Report::idle()));
    let tap = |m: &mut Menu, r: &mut Reader, c: Control| tap_heard(m, r, &again, &dir, c);
    for _ in 0..3 {
        tap(&mut m, &mut r, Control::Down);
    }
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.mix, menu::Mix::Game);
    tap(&mut m, &mut r, Control::Right);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.mix, menu::Mix::Nighttime);
    assert_eq!(m.settings.levels(), [0, 0, 50, 30, 50, 50]);
    let mixer = again.mixer().lock().unwrap();
    assert_eq!((mixer.music_volume, mixer.effects_volume, mixer.crowd_volume, mixer.engine_volume), (0.0, 0.5, 0.3, 0.5));
    drop(mixer);
    tap(&mut m, &mut r, Control::Right);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.mix, menu::Mix::Custom);
    assert_eq!(m.settings.music_volume, 60);
}

/// P79: a seat's Controls row offers the original's controller presets under the original layout —
/// New on its two biplanes, then Original on presets 0 to 3 on each — and the file keeps the pick.
#[test]
fn the_controls_row_offers_the_originals_presets() {
    let (mut m, mut r) = fresh();
    m.open(Screen::Controls);
    tap(&mut m, &mut r, Control::Down);
    let one = |m: &Menu| (m.controls.layouts[0], m.controls.presets[0], m.controls.planes[0]);
    let mut seen = vec![one(&m)];
    for _ in 0..9 {
        tap(&mut m, &mut r, Control::Right);
        seen.push(one(&m));
    }
    let (n, o) = (Layout::New, Layout::Original);
    assert_eq!(
        seen,
        vec![(n, 0, n), (n, 0, o), (o, 0, n), (o, 0, o), (o, 1, n), (o, 1, o), (o, 2, n), (o, 2, o), (o, 3, n), (o, 3, o)]
    );
    // On round to the start, and back the other way.
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(one(&m), (n, 0, n));
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(one(&m), (o, 3, o));
    tap(&mut m, &mut r, Control::Left);
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(one(&m), (o, 2, o));
    assert_eq!(m.controls.presets[1..], [0; 7], "2P's preset moved with 1P's");
    assert!(m.take_changed());
    assert_eq!(menu::controls_label(o, 2), "Original preset 2");
    assert_eq!(menu::controls_label(o, 0), "Original");
    assert_eq!(menu::controls_label(n, 0), "New");

    // The file: one number for every seat, or one a seat with the last carried on; nonsense and
    // presets past 3 are the default.
    let dir = scratch("presets");
    menu::save(&dir, &m.settings, &m.controls).unwrap();
    let (_, controls) = menu::load(&dir);
    assert_eq!(controls.presets, [2, 0, 0, 0, 0, 0, 0, 0]);
    assert!(m.controls.text().contains("\npreset = 2,0,0,0,0,0,0,0\n"));
    std::fs::remove_dir_all(dir).ok();
    assert_eq!(Controls::parse("preset = 3").presets, [3; 8]);
    assert_eq!(Controls::parse("preset = 1,2").presets, [1, 2, 2, 2, 2, 2, 2, 2]);
    assert_eq!(Controls::parse("preset = 4").presets, [0; 8]);
    assert_eq!(Controls::parse("preset = one").presets, [0; 8]);
    assert!(Controls::default().text().contains("\npreset = 0\n"));
    // Where the Controls screen keeps each one's keyboard list.
    assert_eq!(menu::keyboard_at(n, 2), 0);
    assert_eq!((0..4).map(|p| menu::keyboard_at(o, p)).collect::<Vec<_>>(), [1, 2, 3, 4]);
}
