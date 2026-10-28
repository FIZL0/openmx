//! The front end (P36): every screen is reachable from the main menu by pad input alone, and the
//! settings files read back as they were written. And the machine in the menus (P41): the pause's
//! switch puts the player who opened it in the other machine, and the ride setup's pick is the
//! machine the ride starts on.
//!
//! The menu is a state machine fed by [`menu::Reader`], which turns the 32-byte reports the shell's
//! pad layer hands the ride into edges, so these tests drive it with nothing but reports — the same
//! bytes a DualShock or the keyboard would put there. The drawing half is checked against the disc
//! when there is one.

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
// The follow cameras' framing (P163), which lays a seat's presets over the disc's.
#[path = "../src/chase.rs"]
#[allow(dead_code)]
mod chase;
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
// MD11's filters read each mod's source game.
#[path = "../src/track_mods.rs"]
#[allow(dead_code)]
mod track_mods;

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

/// One press and release of `control`: a frame down and three up through the reader, the menu
/// stepping on each — a hand's pace, the up longer than a bounce (P109).
fn tap(menu: &mut Menu, reader: &mut Reader, control: Control) -> Vec<Act> {
    let mut acts = Vec::new();
    for report in [Report::from_buttons(control.mask()), Report::idle(), Report::idle(), Report::idle()] {
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

/// Taps of `control` until `done` holds, which 64 taps must reach.
fn tap_until(m: &mut Menu, r: &mut Reader, control: Control, done: impl Fn(&Menu) -> bool) {
    for _ in 0..64 {
        if done(m) {
            return;
        }
        tap(m, r, control);
    }
    assert!(done(m), "64 taps of {} did not get there", control.name());
}

/// R1 along the open screen's tabs to the one named `tab` (P118).
fn tab_to(m: &mut Menu, r: &mut Reader, tab: &str) {
    // The Original Game page is the Ride tab's since P179: Circle out of it, and its row to go in.
    if m.screen() == Some(Screen::Original) {
        tap(m, r, Control::Circle);
    }
    if tab == "Original" {
        tab_to(m, r, "Ride");
        down_to(m, r, menu::Key::OriginalPage);
        tap(m, r, Control::Cross);
        assert_eq!(m.screen(), Some(Screen::Original));
        return;
    }
    let screen = m.screen().unwrap();
    assert!(m.tabs(screen).contains(&tab), "{screen:?} has no {tab} tab");
    tap_until(m, r, Control::R1, |m| m.tabs(screen)[m.tab(screen)] == tab);
}

/// Down the main menu to the row named `row`, and Cross.
fn main_to(m: &mut Menu, r: &mut Reader, row: &str) {
    let at = menu::MAIN.iter().position(|n| *n == row).unwrap();
    tap_until(m, r, Control::Down, |m| m.cursor(Screen::Main) == at);
    tap(m, r, Control::Cross);
}

/// Settings from the main menu by pad alone (P118), on the tab named `tab`.
fn settings_at(m: &mut Menu, r: &mut Reader, tab: &str) {
    main_to(m, r, "Settings");
    assert_eq!(m.screen(), Some(Screen::Settings));
    tab_to(m, r, tab);
}

/// Down an option page to the row that is `key`, round from the bottom if it is above.
fn down_to(m: &mut Menu, r: &mut Reader, key: menu::Key) {
    let screen = m.screen().unwrap();
    assert!(m.page().iter().any(|row| row.key == key), "{screen:?} has no {key:?} row");
    tap_until(m, r, Control::Down, |m| m.page()[m.cursor(screen)].key == key);
}

/// Type `text` on the menu's keyboard: down to each character's row, right to its key, Cross. The
/// keyboard gives a word's first letter its capital.
fn type_keys(m: &mut Menu, r: &mut Reader, text: &str) {
    for c in text.chars() {
        let (row, col) = menu::KEY_ROWS
            .iter()
            .enumerate()
            .find_map(|(i, k)| k.find(c.to_ascii_lowercase()).map(|j| (i, j)))
            .unwrap_or_else(|| panic!("no key for {c:?}"));
        tap_until(m, r, Control::Down, |m| m.key_at().0 == row);
        tap_until(m, r, Control::Right, |m| m.key_at().1 == col);
        tap(m, r, Control::Cross);
    }
}

/// Cross on the keyboard's command `col` under its letters: Space, Delete, Clear, Done.
fn key_command(m: &mut Menu, r: &mut Reader, col: usize) {
    tap_until(m, r, Control::Down, |m| m.key_at().0 == menu::KEY_ROWS.len());
    tap_until(m, r, Control::Right, |m| m.key_at().1 == col);
    tap(m, r, Control::Cross);
}

/// Press every row of the open option page in turn, walking to each by Down, top to bottom or
/// bottom to top: Right, or Cross where Right is not the row's press. A row that opens a page or the
/// keyboard is back by Circle. A dimmed row must say why, and neither press may change anything.
/// The rows whose press changed the menu.
fn press_every_row(m: &mut Menu, r: &mut Reader, upward: bool) -> Vec<menu::Key> {
    let screen = m.screen().unwrap();
    let n = m.page().len();
    let order: Vec<usize> = if upward { (0..n).rev().collect() } else { (0..n).collect() };
    let mut changed = Vec::new();
    for i in order {
        // A row can change which rows follow it (the Camera tab's Preset row, P163): past the end
        // of the shorter page, the other walk has them.
        let Some(row) = m.page().get(i).cloned() else { continue };
        if !row.selectable() {
            continue;
        }
        tap_until(m, r, Control::Down, |m| m.cursor(screen) == i);
        let before = key(m);
        if let Some(why) = &row.off {
            assert!(!why.is_empty(), "{screen:?} {:?} is dimmed without saying why", row.key);
            tap(m, r, Control::Right);
            tap(m, r, Control::Cross);
            assert_eq!(key(m), before, "{screen:?} {:?} is dimmed ({why}) but a press changed it", row.key);
            continue;
        }
        tap(m, r, Control::Right);
        if key(m) == before {
            tap(m, r, Control::Cross);
        }
        if m.screen() != Some(screen) {
            tap(m, r, Control::Circle);
            assert_eq!(m.screen(), Some(screen), "{screen:?} {:?}: Circle is not back on the page", row.key);
            changed.push(row.key);
        } else if key(m) != before {
            changed.push(row.key);
        }
    }
    changed
}

/// Every selectable row of the open page changes by pad in at least one of two walks, top down and
/// bottom up, each from the menu as `start` leaves it (one row's change can dim another, which the
/// other walk then reaches first).
fn every_row_changes(start: &dyn Fn() -> (Menu, Reader), what: &str) {
    let (m, _) = start();
    let rows: Vec<menu::Row> = m.page().into_iter().filter(|r| r.selectable()).collect();
    assert!(!rows.is_empty(), "{what}: no rows");
    let mut changed = Vec::new();
    for upward in [false, true] {
        let (mut m, mut r) = start();
        changed.extend(press_every_row(&mut m, &mut r, upward));
    }
    for row in rows {
        assert!(changed.contains(&row.key), "{what}: {:?} ({}) never changed", row.key, row.label);
    }
}

#[test]
fn every_screen_is_reachable_from_the_main_menu_by_pad_input_alone() {
    let (mut menu, reader) = fresh();
    // A second pad has joined a local game before (P51): the split screen is two players' at least,
    // and this walk taps one pad only. One alone is `one_pad_cannot_start_a_local_game_but_can_free_ride`.
    menu.join = join::Join::with(&[Some(0), Some(1)], join::ON_SCREEN, join::Opened::Menu);
    assert_eq!(menu.screen(), Some(Screen::Main));
    // Controls, Audio and Camera are Settings' tabs since P118, Graphics since P128, never a screen of
    // their own on the stack; `l1_and_r1_walk_every_tab_round_from_either_end` walks those.
    let wanted: Vec<Screen> =
        Screen::ALL.into_iter().filter(|s| !matches!(s, Screen::Controls | Screen::Audio | Screen::Camera | Screen::Graphics)).collect();
    // Breadth first over single button taps, so each screen is found by its shortest path.
    let mut found: Vec<(Screen, Vec<Control>)> = vec![(Screen::Main, vec![])];
    let mut seen = HashSet::from([key(&menu)]);
    let mut queue = VecDeque::from([(menu, reader, Vec::<Control>::new())]);
    while let Some((menu, reader, path)) = queue.pop_front() {
        // Seven: the garage's Name row, a row under its Player row, opens the keyboard at six.
        if found.len() == wanted.len() || path.len() >= 7 {
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
    for s in wanted {
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
    // Free Ride, then down twice (P118's pick is a list): Bridgeport.
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Tracks));
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Down);
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
    // Settings from the pause (past the machine switch, P41, and the audio, P56), and back to it.
    for _ in 0..4 {
        tap(&mut m, &mut r, Control::Down);
    }
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Settings));
    tap(&mut m, &mut r, Control::Circle);
    assert_eq!(m.screen(), Some(Screen::Pause));
    // Restart and Main Menu (under Clear Markers since P118).
    for _ in 0..3 {
        tap(&mut m, &mut r, Control::Up);
    }
    let acts = tap(&mut m, &mut r, Control::Cross);
    assert!(acts.contains(&Act::Restart), "{acts:?}");
    tap(&mut m, &mut r, Control::Start);
    // under Game Modifiers since P134
    for _ in 0..7 {
        tap(&mut m, &mut r, Control::Down);
    }
    // Main Menu asks first, on Stay (P110): Up onto the leaving row, then Cross.
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Up);
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
    // The stick leaned down, held for a second: one step, then the repeats.
    let mut down = Report::idle();
    down.set_axis(3, 0xff);
    for _ in 0..60 {
        m.step(r.read(&down));
    }
    let moved = m.cursor(Screen::Tracks);
    assert!(moved >= 3, "a second of stick moved the pick {moved} times");
    // And a centred stick moves nothing.
    let mut centre = Report::idle();
    centre.set_axis(3, AXIS_CENTRE);
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
        overlay: Overlay::None,
        wrecks: false,
        // Air Wrecks off (P138).
        air_wrecks: false,
        // Backwards Landings Bail on (S161).
        backwards_bail: true,
        track: "free11".into(),
        track_filter: "MX vs. ATV Untamed".into(),
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
        // No machines at their bays (P103).
        bays: false,
        // 1P on the original's camera and 3P on one of their own, the rest on the default (P82).
        cameras: {
            let mut c = [menu::CameraView::DEFAULT; menu::SEATS_MAX];
            c[0] = menu::CameraView::ORIGINAL;
            c[2] = menu::CameraView { fov: -5, distance: 140, height: 95, plane_distance: 100, plane_height: 180 };
            c
        },
        // 2P's cameras left on Freestyle Cam and Flight Cam 2, Freestyle framed of its own and a
        // preset of their own on Slot Cam's lags; 3P's camera button on their own presets alone (P163).
        camera_sets: {
            use menu::cameras::{Cycle, Look, Own, Seat};
            let mut c: [Seat; menu::SEATS_MAX] = Default::default();
            c[1].start = ["Freestyle Cam".into(), "Flight Cam 2".into()];
            c[1].own.push(Own { machine: Machine::Bike, base: "Freestyle Cam".into(), name: "Freestyle Cam".into(), look: Look { fov: 3, distance: 130, height: 90 } });
            c[1].own.push(Own { machine: Machine::Bike, base: "Slot Cam".into(), name: "Low Slot".into(), look: Look { fov: -2, distance: 60, height: 50 } });
            c[2].cycle = Cycle::Mine;
            c[2].own.push(Own { machine: Machine::Biplane, base: "Flight Cam 1".into(), name: "Far Plane".into(), look: Look { fov: 10, distance: 200, height: 150 } });
            c
        },
        // The mods folder not read (MD2).
        mods: false,
        // One pack of it switched off (P118).
        mods_off: vec!["md2".into()],
        // No radar, and the HUD at 130 % (V52).
        minimap: false,
        hud_scale: 130,
        // MX vs. ATV's tricks (MD16).
        tricks: menu::Tricks::MxVsAtv,
        // The Original tab's own two at the original's (P126).
        plants: game_render::eco::Lod::Original,
        picture: menu::Picture::Television,
        // The original's blended leaves (V57).
        foliage: game_render::eco::Foliage::Original,
        // Textures filtered 4x (V83).
        filtering: menu::Filtering::X4,
        // A world's sky taken from the map below (MD9).
        world_sky: menu::WorldSky::PerMap,
        // The disc's textures, and two texture packs ordered and one off (V84).
        textures: false,
        texture_packs: vec!["hd_ground".into(), "packs/hd_bikes".into()],
        texture_packs_off: vec!["old".into()],
        // 2P with their markers off (P129).
        markers: [true, false, true, true, true, true, true, true],
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
    // A file saved before V26 (P36's `overlay = on`, the debug HUD then the only one) read as the
    // original's HUD (V28), and so did its `off`; being from before P121 too, they now ride the
    // port's own HUD.
    for word in ["on", "off"] {
        std::fs::write(empty.join(menu::SETTINGS_FILE), format!("name = Rider\noverlay = {word}\nwrecks = on\n")).unwrap();
        assert_eq!(menu::load(&empty).0.overlay, Overlay::Hud, "overlay = {word}");
    }
    // The defaults themselves round trip too.
    menu::save(&empty, &Settings::default(), &Controls::default()).unwrap();
    assert_eq!(menu::load(&empty), (Settings::default(), Controls::default()));
    std::fs::remove_dir_all(dir).ok();
    std::fs::remove_dir_all(empty).ok();
}

/// P83, P170: every launch starts on a HUD a rider is meant to see. A fresh file and an old one
/// never read as a HUD there is no more of — `on` (P36's word for the debug HUD), `off`, `debug` and
/// `original` all ride the HUD, with or without P121's `hud = 1` beside them — and Settings' row
/// offers HUD and None, round either way.
#[test]
fn the_settings_offer_the_hud_and_none_alone() {
    assert_eq!(Settings::default().overlay, Overlay::Hud);
    assert_eq!(Settings::parse("").overlay, Overlay::Hud);
    for word in ["on", "off", "debug", "original"] {
        let text = format!("name = Rider\noverlay = {word}\n");
        assert_eq!(Settings::parse(&text).overlay, Overlay::Hud, "{text:?}");
        let text = format!("{text}hud = 1\n");
        assert_eq!(Settings::parse(&text).overlay, Overlay::Hud, "{text:?}");
    }
    assert_eq!(Settings::parse("overlay = none\n").overlay, Overlay::None);
    // Saved from a fresh menu, the file says hud and reads back so.
    let dir = scratch("p83");
    menu::save(&dir, &Settings::default(), &Controls::default()).unwrap();
    assert_eq!(menu::load(&dir).0.overlay, Overlay::Hud);
    for word in ["debug", "original"] {
        std::fs::write(dir.join(menu::SETTINGS_FILE), format!("name = Rider\noverlay = {word}\nhud = 1\n")).unwrap();
        assert_eq!(menu::load(&dir).0.overlay, Overlay::Hud, "a saved {word} falls back");
    }
    std::fs::remove_dir_all(dir).ok();

    let (mut m, mut r) = fresh();
    settings_at(&mut m, &mut r, "Display");
    down_to(&mut m, &mut r, menu::Key::Hud);
    let row = m.page()[m.cursor(Screen::Settings)].clone();
    assert_eq!(row.choices, ["HUD", "None"]);
    let mut seen = vec![m.settings.overlay];
    for press in [Control::Right, Control::Right, Control::Left, Control::Left, Control::Cross, Control::Cross] {
        tap(&mut m, &mut r, press);
        seen.push(m.settings.overlay);
    }
    use Overlay::{Hud, None as Off};
    assert_eq!(seen, [Hud, Off, Hud, Off, Hud, Off, Hud]);
    let text = m.settings.text();
    assert!(!text.contains("debug") && !text.contains("original") && !text.contains("hud = "), "{text}");
}

/// The draw distance row on the Graphics tab (P118's name for V-era's graphics row; P128's tab), whose words in
/// the file are as they were.
#[test]
fn the_graphics_row_swaps_the_whole_map_for_the_originals_fog() {
    let (mut m, mut r) = fresh();
    assert_eq!(m.settings.graphics, Graphics::Full, "the whole map unless asked");
    settings_at(&mut m, &mut r, "Graphics");
    down_to(&mut m, &mut r, menu::Key::DrawDistance);
    assert_eq!(m.page()[m.cursor(Screen::Settings)].value, "Whole Map");
    // V95's Farther Fog sits between the whole map and Far Fog.
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.graphics, Graphics::Farther);
    assert_eq!(m.page()[m.cursor(Screen::Settings)].value, "Farther Fog");
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.graphics, Graphics::Far);
    assert_eq!(m.page()[m.cursor(Screen::Settings)].value, "Far Fog");
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.graphics, Graphics::Original);
    assert_eq!(m.page()[m.cursor(Screen::Settings)].value, "Original Fog");
    assert!(m.take_changed());
    for _ in 0..3 {
        tap(&mut m, &mut r, Control::Left);
    }
    assert_eq!(m.settings.graphics, Graphics::Full);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.settings.graphics, Graphics::Farther);
    assert!(Settings::parse(&m.settings.text()).graphics == Graphics::Farther);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.settings.graphics, Graphics::Far);
    assert!(Settings::parse(&m.settings.text()).graphics == Graphics::Far);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.settings.graphics, Graphics::Original);
    assert!(Settings::parse(&m.settings.text()).graphics == Graphics::Original);
    assert_eq!(Settings::parse("graphics = far\n").graphics, Graphics::Far);
    assert_eq!(Settings::parse("graphics = farther\n").graphics, Graphics::Farther);
    assert_eq!(Settings::parse("graphics = full\n").graphics, Graphics::Full);
    assert_eq!(Settings::parse("graphics = fancy\n").graphics, Graphics::Full);
}

/// V42: the readout unless asked, and the Display tab's row switches it to the dial and back by
/// pad, and says so to be applied and saved. It is the original HUD's, so it dims with the others
/// (P118).
#[test]
fn the_speedometer_row_switches_the_dial_and_is_saved() {
    let (mut m, mut r) = fresh();
    assert_eq!(m.settings.speedometer, Speedometer::Digital, "the readout unless asked");
    assert_eq!(Settings::parse("").speedometer, Speedometer::Digital);
    settings_at(&mut m, &mut r, "Display");
    down_to(&mut m, &mut r, menu::Key::Speedometer);
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
    tap(&mut m, &mut r, Control::Left);
    // With no HUD (the row's other choice since P170) it dims, says why, and keeps its word.
    tap(&mut m, &mut r, Control::Up);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.overlay, Overlay::None);
    tap(&mut m, &mut r, Control::Down);
    let row = m.page()[m.cursor(Screen::Settings)].clone();
    assert_eq!((row.key, row.off.as_deref()), (menu::Key::Speedometer, Some("The HUD is off")));
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.speedometer, Speedometer::Analog);
    // Saved to the file and read back from it.
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

/// V52: the Display tab's Minimap row under the speedometer's turns the radar off and on, and the HUD scale row
/// under it steps 50 % to 150 % by ten; both are saved to settings.txt and read back, and a file
/// without them is the radar at the original's size, written as it was.
#[test]
fn the_minimap_and_hud_scale_rows_change_and_are_saved() {
    let (mut m, mut r) = fresh();
    assert!(m.settings.minimap);
    assert_eq!(m.settings.hud_scale, 100);
    let plain = Settings::parse("name = Rider\n");
    assert!(plain.minimap && plain.hud_scale == 100);
    assert!(!Settings::default().text().contains("minimap") && !Settings::default().text().contains("hud_scale"));
    settings_at(&mut m, &mut r, "Display");
    down_to(&mut m, &mut r, menu::Key::Minimap);
    tap(&mut m, &mut r, Control::Right);
    assert!(m.take_changed());
    assert!(!m.settings.minimap);
    assert_eq!(m.settings.speedometer, Speedometer::Digital, "the speedometer's row is untouched");
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.settings.minimap);
    tap(&mut m, &mut r, Control::Left);
    assert!(!m.settings.minimap);
    assert!(m.settings.text().contains("minimap = off\n"));

    // The HUD scale: up to 150 and no further, down to 50 and no further, Cross round from the top.
    tap(&mut m, &mut r, Control::Down);
    let mut seen = Vec::new();
    for _ in 0..7 {
        tap(&mut m, &mut r, Control::Right);
        assert!(m.take_changed());
        seen.push(m.settings.hud_scale);
    }
    assert_eq!(seen, [110, 120, 130, 140, 150, 150, 150]);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.settings.hud_scale, 50, "round from the largest to the smallest");
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.settings.hud_scale, 50);
    for _ in 0..8 {
        tap(&mut m, &mut r, Control::Right);
    }
    assert_eq!(m.settings.hud_scale, 130);
    assert!(!m.settings.minimap, "the minimap's row is untouched");
    assert!(m.settings.vsync, "the VSync row is untouched");
    assert!(m.settings.text().contains("hud_scale = 130\n"));

    // Saved to the file and read back from it.
    let dir = scratch("hud");
    menu::save(&dir, &m.settings, &Controls::default()).unwrap();
    let back = menu::load(&dir).0;
    assert_eq!((back.minimap, back.hud_scale), (false, 130));
    std::fs::remove_dir_all(&dir).unwrap();
    // A step the row does not offer, or nonsense, is the original's size; a percent sign is fine.
    assert_eq!(Settings::parse("hud_scale = 135\n").hud_scale, 100);
    assert_eq!(Settings::parse("hud_scale = 900\n").hud_scale, 100);
    assert_eq!(Settings::parse("hud_scale = big\n").hud_scale, 100);
    assert_eq!(Settings::parse("hud_scale = 60%\n").hud_scale, 60);
    assert!(Settings::parse("minimap = on\n").minimap);
}

#[test]
fn the_render_scale_row_cycles_and_is_saved() {
    let (mut m, mut r) = fresh();
    assert_eq!(m.settings.render_scale, RenderScale::Auto, "auto unless asked");
    settings_at(&mut m, &mut r, "Graphics");
    down_to(&mut m, &mut r, menu::Key::RenderScale);
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
    // The row under it is VSync's, which the render scale's press does not reach.
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.render_scale, RenderScale::Full);
    assert!(!m.settings.vsync);
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

/// P90: a fresh settings file has VSync on and no frame limit; the Graphics tab's last two rows
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

    settings_at(&mut m, &mut r, "Graphics");
    down_to(&mut m, &mut r, menu::Key::VSync);
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
    // On to the Ride tab: the machines at their bays (P103), which Cross turns off and on, and the
    // game modifiers under them.
    tab_to(&mut m, &mut r, "Ride");
    down_to(&mut m, &mut r, menu::Key::Bays);
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.take_changed());
    assert!(!m.settings.bays);
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.settings.bays);
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

/// P82: the Camera tab under Settings. Its sliders move by left and right a notch at a time, Cross
/// puts one back to the original's, its resets put every slider back, the Player row sets a seat
/// apart from the rest, and all of it is saved and read back. The Original notch hands the shell the
/// original's distance, height and field of view untouched.
#[test]
fn the_camera_rows_change_by_pad_and_are_saved() {
    use menu::{CameraView, Key};
    let (mut m, mut r) = fresh();
    assert_eq!(m.settings.cameras, [CameraView::DEFAULT; menu::SEATS_MAX], "the shipped default");
    // The Original notch is the original's numbers: no field-of-view offset and each distance and
    // height at one times the original's.
    assert_eq!(CameraView::ORIGINAL.bike(), (0.0, 1.0, 1.0));
    assert_eq!(CameraView::ORIGINAL.plane(), (0.0, 1.0, 1.0));
    // The default sits further back, higher and a few degrees wider, both machines.
    let (fov, distance, height) = CameraView::DEFAULT.bike();
    assert!(fov > 0.0 && distance > 1.0 && height > 1.0);
    let (fov, distance, height) = CameraView::DEFAULT.plane();
    assert!(fov > 0.0 && distance > 1.0 && height > 1.0);

    settings_at(&mut m, &mut r, "Camera");
    let rows = m.page();
    assert_eq!(rows.iter().map(|r| r.key).collect::<Vec<_>>(), [
            Key::Player,
            Key::CameraPreset,
            Key::Camera(0),
            Key::Camera(1),
            Key::Camera(2),
            Key::Camera(3),
            Key::Camera(4),
            Key::CameraDefault,
            Key::CameraOriginal,
            Key::CameraSave,
            Key::CameraCycle
        ]
    );
    assert_eq!(rows[0].value, "1P");

    // The field of view: a notch right is a step wider for 1P alone, Cross the original's.
    down_to(&mut m, &mut r, Key::Camera(0));
    tap(&mut m, &mut r, Control::Right);
    assert!(m.take_changed());
    assert_eq!(m.settings.cameras[0].fov, CameraView::DEFAULT.fov + 1);
    assert_eq!(m.settings.cameras[1], CameraView::DEFAULT, "2P's is their own");
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.settings.cameras[0].fov, 0);
    assert_eq!(m.page()[2].value, "Original");
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.settings.cameras[0].fov, -1);
    // The bike's distance, a notch left: five points nearer.
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.settings.cameras[0].distance, CameraView::DEFAULT.distance - menu::CAMERA_STEP);

    // 2P: the Player row to 2P, then the biplane's height up, which leaves 1P's.
    down_to(&mut m, &mut r, Key::Player);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.page()[0].value, "2P");
    assert_eq!(m.page()[2].value, CameraView::DEFAULT.label(0), "the sliders show 2P's");
    down_to(&mut m, &mut r, Key::Camera(4));
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.cameras[1].plane_height, CameraView::DEFAULT.plane_height + menu::CAMERA_STEP);
    assert_eq!(m.settings.cameras[0].plane_height, CameraView::DEFAULT.plane_height);
    // Reset to Original puts every slider of 2P's back, and nobody else's.
    down_to(&mut m, &mut r, Key::CameraOriginal);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.settings.cameras[1], CameraView::ORIGINAL);
    assert_eq!(m.settings.cameras[1].bike(), (0.0, 1.0, 1.0));
    assert_eq!(m.settings.cameras[1].plane(), (0.0, 1.0, 1.0));
    assert_ne!(m.settings.cameras[0], CameraView::ORIGINAL);
    // And Reset to Default 2P's to the port's.
    tap(&mut m, &mut r, Control::Up);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.settings.cameras[1], CameraView::DEFAULT);
    tap(&mut m, &mut r, Control::Down);
    tap(&mut m, &mut r, Control::Cross);

    // Saved and read back, one view a seat.
    let dir = scratch("camera");
    menu::save(&dir, &m.settings, &m.controls).unwrap();
    let back = menu::load(&dir).0;
    assert_eq!(back.cameras, m.settings.cameras);
    assert!(m.settings.text().contains("camera = "), "{}", m.settings.text());
    // A file from before P82 is the default, and a short line carries its last view on.
    assert_eq!(Settings::parse("name = Rider\n").cameras, [CameraView::DEFAULT; menu::SEATS_MAX]);
    let one = Settings::parse("camera = 0 100 100 100 100\n").cameras;
    assert_eq!(one, [CameraView::ORIGINAL; menu::SEATS_MAX]);
    // Out of range is clamped to the slider's ends.
    let wild = Settings::parse("camera = 90 1000 0 100 100\n").cameras[0];
    assert_eq!((wild.fov, wild.distance, wild.height), (menu::CAMERA_FOV.1, menu::CAMERA_SHARE.1, menu::CAMERA_SHARE.0));
    // Back out to the main menu.
    tap(&mut m, &mut r, Control::Circle);
    assert_eq!(m.screen(), Some(Screen::Main));
    std::fs::remove_dir_all(dir).ok();
}

// --- P163: each seat's camera presets -----------------------------------------------------------------

/// A camera file of free06's second archive, read as the shell reads it (the full screen's copy);
/// without the disc, a stand-in with the disc's names in its order and a number of each its own.
fn camera_file(file: &str) -> Vec<game_sim::camera::Preset> {
    use game_sim::archive::{Psr, Res};
    use game_sim::camera::Preset;
    use std::io::{Read, Seek, SeekFrom};
    let disc = || -> Option<Vec<Preset>> {
        let iso = match std::env::var_os("GAME_SIM_FIXTURES") {
            Some(dir) => std::path::PathBuf::from(dir).join("iso"),
            None => std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/iso"),
        };
        let mut f = std::fs::File::open(iso.join("MX.PSR")).ok()?;
        let mut front = vec![0u8; 64 << 10];
        f.read_exact(&mut front).ok()?;
        let e = Psr::parse(&front).ok()?.entries.into_iter().find(|e| e.name.to_ascii_lowercase().ends_with("free06_2.res"))?;
        f.seek(SeekFrom::Start(e.at)).ok()?;
        let mut bytes = vec![0u8; e.size as usize];
        f.read_exact(&mut bytes).ok()?;
        let res = Res::parse(&bytes).ok()?;
        let ini = res.entries.iter().find(|e| {
            let n = e.name.to_ascii_lowercase();
            n == file || n.ends_with(&format!("\\{file}"))
        })?;
        Some(Preset::read_bytes(&res.read(&bytes, ini).ok()?, "")).filter(|p| !p.is_empty())
    };
    disc().unwrap_or_else(|| {
        eprintln!("no disc: stand-in {file}");
        let names: &[(&str, bool, bool)] = match file {
            "cpresets.ini" => &[("Race Cam 1", true, false), ("Race Cam 2", true, false), ("Freestyle Cam", true, false), ("Slot Cam", true, false), ("Lookback", false, true), ("1P Stunt", false, false)],
            _ => &[("Flight Cam 1", true, false), ("Flight Cam 2", true, false), ("Slot Cam", true, false), ("Lookback Cam", false, true)],
        };
        names
            .iter()
            .enumerate()
            .map(|(i, &(name, cycle, back))| Preset {
                name: name.into(),
                angle_x: ps2_float::F(0.2 + 0.1 * i as f32),
                following_distance: ps2_float::F(10.0 + 3.0 * i as f32),
                min_following_distance: ps2_float::F(5.0),
                max_following_distance: ps2_float::F(40.0),
                fov: ps2_float::F(80.0 + i as f32),
                min_fov: ps2_float::F(30.0),
                max_fov: ps2_float::F(100.0),
                filter_this_preset: true,
                cycle_thru_this_preset: cycle,
                is_look_back_cam: back,
                ..Preset::default()
            })
            .collect()
    })
}

/// The names of a camera file's presets the camera button cycles through, as the shell hands them
/// to the Camera page.
fn cycling(presets: &[game_sim::camera::Preset]) -> Vec<String> {
    presets.iter().filter(|p| p.cycle_thru_this_preset).map(|p| p.name.clone()).collect()
}

/// What the shell lays over `disc` for seat `seat`'s `machine` (`main.rs`'s `frame_rig`): the
/// Camera page's one set, the seat's own presets, and its cycle.
fn laid(m: &Menu, seat: usize, machine: Machine, disc: &[game_sim::camera::Preset]) -> Vec<game_sim::camera::Preset> {
    let set = &m.settings.camera_sets[seat];
    let view = m.settings.cameras[seat];
    let (fov, distance, height) = if machine == Machine::Bike { view.bike() } else { view.plane() };
    let own: Vec<chase::Own> = menu::cameras::laid(&set.own, set.cycle, machine)
        .map(|o| {
            let (fov, distance, height) = o.look.framing();
            chase::Own { base: &o.base, name: &o.name, framing: chase::Framing { fov, distance, height } }
        })
        .collect();
    chase::seat_presets(disc, chase::Framing { fov, distance, height }, &own, set.cycle == menu::cameras::Cycle::Mine)
}

/// The shell's follow camera on `presets`, on `start` as the shell starts it (`--camera`'s).
fn follow_on(presets: Vec<game_sim::camera::Preset>, start: &str) -> game_sim::camera::Race {
    let mut cam = game_sim::camera::Race::new(presets, ps2_float::F(5.0));
    let at = game_sim::camera::Preset::find(&cam.cam.presets, start).unwrap();
    cam.cam.set_preset(at);
    cam
}

/// The camera button pressed until it comes back round: the presets it stepped through.
fn walk(cam: &mut game_sim::camera::Race) -> Vec<String> {
    let from = cam.cam.preset;
    let mut seen = Vec::new();
    for _ in 0..cam.cam.presets.len() + 1 {
        cam.cam.next_preset();
        seen.push(cam.cam.preset().name.clone());
        if cam.cam.preset == from {
            break;
        }
    }
    seen
}

/// P163 (1): the preset the camera button last left a seat's bike and biplane on is saved per seat
/// in settings.txt, and after the round trip the seat's next ride starts each camera on it, where
/// another seat starts on the shell's own.
#[test]
fn a_seat_left_on_a_preset_starts_the_next_ride_on_it() {
    let (mut m, _) = fresh();
    let (bike, plane) = (camera_file("cpresets.ini"), camera_file("biplanecpresets.ini"));
    // 2P's camera button left the bike on Freestyle Cam and the plane on Flight Cam 2; a second
    // word of the same is nothing to save.
    assert!(m.camera_left_on(1, Machine::Bike, "Freestyle Cam"));
    assert!(m.take_changed());
    assert!(!m.camera_left_on(1, Machine::Bike, "Freestyle Cam"));
    assert!(!m.take_changed());
    assert!(m.camera_left_on(1, Machine::Biplane, "Flight Cam 2"));
    let dir = scratch("camera-start");
    menu::save(&dir, &m.settings, &m.controls).unwrap();
    let text = std::fs::read_to_string(dir.join("settings.txt")).unwrap();
    assert!(text.contains("camera_start = 2 | Freestyle Cam | Flight Cam 2\n"), "{text}");
    let back = menu::load(&dir).0;
    assert_eq!(back.camera_sets, m.settings.camera_sets);
    // A file from before P163 has no start for anybody.
    assert!(Settings::parse("name = Rider\n").camera_sets.iter().all(|s| s.start == [String::new(), String::new()]));

    // The next ride: every camera made on the shell's start, then each seat's own.
    for (seat, want) in [(0, ["Race Cam 1", "Flight Cam 1"]), (1, ["Freestyle Cam", "Flight Cam 2"])] {
        let start = &back.camera_sets[seat].start;
        let mut cam = follow_on(bike.clone(), "Race Cam 1");
        let mut flown = follow_on(plane.clone(), "Flight Cam 1");
        assert_eq!(chase::start_on(&mut cam, &start[0], false), seat == 1);
        chase::start_on(&mut flown, &start[1], false);
        assert_eq!([cam.cam.preset().name.as_str(), flown.cam.preset().name.as_str()], want, "seat {seat}");
    }
    // A name the track's file does not have leaves the shell's start.
    let mut cam = follow_on(bike.clone(), "Race Cam 1");
    assert!(!chase::start_on(&mut cam, "Gone Cam", false));
    assert_eq!(cam.cam.preset().name, "Race Cam 1");
    std::fs::remove_dir_all(dir).ok();
}

/// P163 (2): a preset of the player's own, saved on the Camera page off one of the disc's, comes up
/// in the camera button's cycle after the disc's with its own numbers — or in their place — while
/// the disc's presets are as they were; a disc preset's own numbers are its alone, and at the
/// original's every disc preset is the file's to the bit. All of it per seat, and saved.
#[test]
fn a_custom_camera_preset_joins_the_cycle_with_its_own_numbers() {
    use menu::cameras::{Cycle, Look};
    use menu::{CameraView, Key};
    let (mut m, mut r) = fresh();
    let (bike, plane) = (camera_file("cpresets.ini"), camera_file("biplanecpresets.ini"));
    m.camera_names = [cycling(&bike), cycling(&plane)];
    let before = laid(&m, 0, Machine::Bike, &bike);
    assert_eq!(before, chase::framed(&bike, chase::Framing { fov: 4.0, distance: 1.15, height: 1.10 }), "no presets of the seat's own: P82's");

    settings_at(&mut m, &mut r, "Camera");
    // The Preset row: All Presets, the disc's that cycle (the bike's, then the biplane's).
    let row = m.page().into_iter().find(|r| r.key == Key::CameraPreset).unwrap();
    assert_eq!(row.value, "All Presets");
    assert_eq!(row.choices[1..].len(), m.camera_names[0].len() + m.camera_names[1].len());
    assert!(row.choices.contains(&"Biplane: Flight Cam 2".to_string()), "{:?}", row.choices);
    down_to(&mut m, &mut r, Key::CameraPreset);
    tap_until(&mut m, &mut r, Control::Right, |m| m.camera_picked().is_some_and(|p| p.name == "Slot Cam" && p.machine == Machine::Bike));
    // Its sliders, on All Presets' numbers until moved.
    assert_eq!(m.page().iter().find(|r| r.key == Key::CameraOwn(1)).unwrap().value, format!("{}%", CameraView::DEFAULT.distance));
    assert!(m.page().iter().find(|r| r.key == Key::CameraFollow).unwrap().off.is_some());

    // Saved as one of 1P's own, named on the keyboard: the Preset row goes onto it.
    down_to(&mut m, &mut r, Key::CameraSave);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Keyboard));
    type_keys(&mut m, &mut r, "low");
    tap(&mut m, &mut r, Control::Start);
    assert_eq!(m.screen(), Some(Screen::Settings));
    assert!(m.take_changed());
    let pick = m.camera_picked().unwrap();
    assert_eq!((pick.name.as_str(), pick.custom, pick.label()), ("Low", true, "Low (Mine)".to_string()));
    // Its own numbers: nearer by four notches, lower by two, three degrees wider.
    down_to(&mut m, &mut r, Key::CameraOwn(1));
    for _ in 0..4 {
        tap(&mut m, &mut r, Control::Left);
    }
    tap(&mut m, &mut r, Control::Down);
    for _ in 0..2 {
        tap(&mut m, &mut r, Control::Left);
    }
    down_to(&mut m, &mut r, Key::CameraOwn(0));
    for _ in 0..3 {
        tap(&mut m, &mut r, Control::Right);
    }
    let look = Look { fov: CameraView::DEFAULT.fov + 3, distance: CameraView::DEFAULT.distance - 20, height: CameraView::DEFAULT.height - 10 };
    assert_eq!(m.camera_look(&pick), look);
    assert_eq!(m.settings.camera_sets[0].own_of(Machine::Bike, "Slot Cam"), None, "Slot Cam has no numbers of its own");
    assert_eq!(m.settings.camera_sets[1], Default::default(), "2P's are their own");

    // What the shell lays: the disc's as they were, then Low, Slot Cam's copy on its own numbers.
    let now = laid(&m, 0, Machine::Bike, &bike);
    assert_eq!(now[..bike.len()], before[..], "the disc's presets are unchanged");
    let slot = bike.iter().find(|p| p.name == "Slot Cam").unwrap();
    let (fov, distance, height) = look.framing();
    let mut want = chase::framed(std::slice::from_ref(slot), chase::Framing { fov, distance, height }).remove(0);
    want.name = "Low".into();
    assert_eq!(now.len(), bike.len() + 1);
    assert_eq!(now[bike.len()], want);
    assert!(now[bike.len()].following_distance.0 < before.iter().find(|p| p.name == "Slot Cam").unwrap().following_distance.0);
    assert_eq!(laid(&m, 0, Machine::Biplane, &plane), chase::framed(&plane, chase::Framing { fov: 4.0, distance: 1.20, height: 1.15 }), "the plane has none");
    // The camera button: the disc's, then Low, round to the start.
    let mut cam = follow_on(now.clone(), "Race Cam 1");
    let mut want: Vec<String> = cycling(&bike)[1..].to_vec();
    want.extend(["Low".to_string(), "Race Cam 1".to_string()]);
    assert_eq!(walk(&mut cam), want);
    // The look-back preset is still the disc's.
    assert_eq!(cam.cam.look_back_preset(), bike.iter().position(|p| p.is_look_back_cam));

    // Mine Only: Low alone, and a ride starts on it.
    down_to(&mut m, &mut r, Key::CameraCycle);
    tap_until(&mut m, &mut r, Control::Right, |m| m.settings.camera_sets[0].cycle == Cycle::Mine);
    let mine = laid(&m, 0, Machine::Bike, &bike);
    let mut cam = follow_on(mine.clone(), "Race Cam 1");
    assert!(chase::start_on(&mut cam, "", true));
    assert_eq!(cam.cam.preset().name, "Low");
    assert_eq!(walk(&mut cam), ["Low"]);
    // The plane, with none of 1P's own, keeps the disc's cycle under Mine Only.
    assert_eq!(laid(&m, 0, Machine::Biplane, &plane), chase::framed(&plane, chase::Framing { fov: 4.0, distance: 1.20, height: 1.15 }));
    // Disc's: Low is out, and the list is the disc's again.
    tap_until(&mut m, &mut r, Control::Right, |m| m.settings.camera_sets[0].cycle == Cycle::Disc);
    assert_eq!(laid(&m, 0, Machine::Bike, &bike), before);
    tap_until(&mut m, &mut r, Control::Right, |m| m.settings.camera_sets[0].cycle == Cycle::Both);

    // A disc preset's own numbers: Freestyle Cam a notch further out, which no other preset takes.
    down_to(&mut m, &mut r, Key::CameraPreset);
    tap_until(&mut m, &mut r, Control::Right, |m| m.camera_picked().is_some_and(|p| p.name == "Freestyle Cam"));
    down_to(&mut m, &mut r, Key::CameraOwn(1));
    tap(&mut m, &mut r, Control::Right);
    let own = laid(&m, 0, Machine::Bike, &bike);
    for (i, p) in bike.iter().enumerate() {
        if p.name == "Freestyle Cam" {
            assert!(own[i].following_distance.0 > before[i].following_distance.0);
        } else {
            assert_eq!(own[i], before[i], "{}", p.name);
        }
    }
    assert!(!m.page().iter().find(|r| r.key == Key::CameraFollow).unwrap().off.is_some());

    // Saved and read back.
    let dir = scratch("camera-presets");
    menu::save(&dir, &m.settings, &m.controls).unwrap();
    assert_eq!(menu::load(&dir).0.camera_sets, m.settings.camera_sets);
    std::fs::remove_dir_all(dir).ok();

    // Original: Reset to Original on All Presets puts the disc's numbers back exactly, Freestyle
    // Cam's own numbers with them; Low stays 1P's, after the disc's, on its own numbers.
    down_to(&mut m, &mut r, Key::CameraPreset);
    tap_until(&mut m, &mut r, Control::Right, |m| m.camera_picked().is_none());
    down_to(&mut m, &mut r, Key::CameraOriginal);
    tap(&mut m, &mut r, Control::Cross);
    let original = laid(&m, 0, Machine::Bike, &bike);
    assert_eq!(original[..bike.len()], bike[..], "the disc's to the bit");
    assert_eq!(original[bike.len()].name, "Low");
    // And the Original switch's camera row takes Low out of the cycle too, for every seat.
    m.set_departure(menu::Departure::Cameras, true);
    assert!(m.is_original(menu::Departure::Cameras));
    assert_eq!(laid(&m, 0, Machine::Bike, &bike), bike);
    assert_eq!(laid(&m, 1, Machine::Biplane, &plane), plane);
    // A name a disc preset has is taken as the player's own with a word after it.
    let kept = m.settings.camera_sets[0].save_custom(Machine::Biplane, "Flight Cam 1", "Slot Cam", Look::ORIGINAL, &m.camera_names[1].clone());
    assert_eq!(kept, "Slot Cam Mine");
}

/// P179 (1): Save as New Preset saves from wherever the Camera page is. On All Presets it copies the
/// preset the seat's rides start on, framed as All Presets frames it: the disc's first in the cycle
/// (the shell's own start) before the camera button has moved, else where it was last left, on the
/// bike every ride starts on (P177). The typed name lands in the seat's set, the camera
/// button reaches it, and it is there after a restart.
#[test]
fn a_camera_preset_saves_from_all_presets_and_survives_a_restart() {
    use menu::Key;
    use menu::cameras::Look;
    let (mut m, mut r) = fresh();
    let (bike, plane) = (camera_file("cpresets.ini"), camera_file("biplanecpresets.ini"));
    m.camera_names = [cycling(&bike), cycling(&plane)];
    // The shell's own start, an empty start word, is the file's first preset, which cycles.
    let shell = game_sim::camera::Race::new(bike.clone(), ps2_float::F(5.0));
    assert_eq!(shell.cam.preset().name, m.camera_names[0][0]);
    let shell = game_sim::camera::Race::new(plane.clone(), ps2_float::F(5.0));
    assert_eq!(shell.cam.preset().name, m.camera_names[1][0]);

    settings_at(&mut m, &mut r, "Camera");
    assert_eq!(m.camera_picked(), None, "All Presets");
    let save = m.page().into_iter().find(|r| r.key == Key::CameraSave).unwrap();
    assert_eq!(save.off, None, "not dimmed on All Presets");
    down_to(&mut m, &mut r, Key::CameraSave);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Keyboard));
    type_keys(&mut m, &mut r, "wide");
    tap(&mut m, &mut r, Control::Start);
    assert_eq!(m.screen(), Some(Screen::Settings));
    assert!(m.take_changed());
    // In 1P's set, over the start preset, on All Presets' bike numbers; the Preset row goes onto it.
    let set = &m.settings.camera_sets[0];
    let own: Vec<_> = set.customs(Machine::Bike).collect();
    assert_eq!(own.len(), 1);
    assert_eq!((own[0].name.as_str(), own[0].base.as_str()), ("Wide", m.camera_names[0][0].as_str()));
    assert_eq!(own[0].look, Look::of(&m.settings.cameras[0], Machine::Bike));
    assert_eq!(m.camera_picked().map(|p| p.label()), Some("Wide (Mine)".to_string()));
    assert_eq!(m.settings.camera_sets[1], Default::default(), "2P's are their own");

    // The camera button: the disc's, then Wide.
    let now = laid(&m, 0, Machine::Bike, &bike);
    let mut cam = follow_on(now, "Race Cam 1");
    assert!(walk(&mut cam).contains(&"Wide".to_string()));

    // Where the camera button was last left: the copy is that preset's, on its own numbers.
    m.camera_left_on(0, Machine::Bike, "Freestyle Cam");
    let mut look = Look::of(&m.settings.cameras[0], Machine::Bike);
    look.step(1, true, false);
    m.settings.camera_sets[0].set_look(Machine::Bike, "Freestyle Cam", look);
    down_to(&mut m, &mut r, Key::CameraPreset);
    tap_until(&mut m, &mut r, Control::Right, |m| m.camera_picked().is_none());
    down_to(&mut m, &mut r, Key::CameraSave);
    tap(&mut m, &mut r, Control::Cross);
    type_keys(&mut m, &mut r, "near");
    tap(&mut m, &mut r, Control::Start);
    let near = m.settings.camera_sets[0].customs(Machine::Bike).find(|o| o.name == "Near").cloned().unwrap();
    assert_eq!((near.base.as_str(), near.look), ("Freestyle Cam", look));

    // A restart: written, read back into a new menu, the Preset row and the camera button have them.
    let dir = scratch("camera-save-all");
    menu::save(&dir, &m.settings, &m.controls).unwrap();
    let (settings, controls) = menu::load(&dir);
    assert_eq!(settings.camera_sets, m.settings.camera_sets);
    let mut again = Menu::new(settings, controls);
    again.camera_names = m.camera_names.clone();
    let labels: Vec<String> = again.camera_choices().iter().map(|p| p.label()).collect();
    for want in ["Wide (Mine)", "Near (Mine)"] {
        assert!(labels.contains(&want.to_string()), "{labels:?}");
    }
    let mut cam = follow_on(laid(&again, 0, Machine::Bike, &bike), "Race Cam 1");
    let seen = walk(&mut cam);
    assert!(seen.contains(&"Wide".to_string()) && seen.contains(&"Near".to_string()), "{seen:?}");
    std::fs::remove_dir_all(dir).ok();

    // With no presets read and none of the player's, there is nothing to copy: the row is dim.
    let (mut bare, mut r) = fresh();
    settings_at(&mut bare, &mut r, "Camera");
    assert!(bare.page().into_iter().find(|r| r.key == Key::CameraSave).unwrap().off.is_some());
}

#[test]
fn what_the_pad_sets_is_what_the_file_says() {
    let (mut m, mut r) = fresh();
    // The name is the Garage's since P118: its Name row, under the Player row, opens the keyboard.
    main_to(&mut m, &mut r, "Garage");
    assert_eq!(m.screen(), Some(Screen::Garage));
    down_to(&mut m, &mut r, menu::Key::Garage(roster::Row::Name));
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Keyboard));
    assert_eq!(m.typing(), m.settings.name, "1P's look has no name of its own yet");
    // Clear, on the command row under the letters, then "mx".
    key_command(&mut m, &mut r, 2);
    assert_eq!(m.typing(), "");
    type_keys(&mut m, &mut r, "mx");
    assert_eq!(m.typing(), "Mx");
    // Square deletes, and the letter goes back.
    tap(&mut m, &mut r, Control::Square);
    assert_eq!(m.typing(), "M");
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Start);
    assert_eq!(m.screen(), Some(Screen::Garage));
    assert_eq!(m.settings.name, "Mx");
    assert_eq!(m.page()[m.cursor(Screen::Garage)].value, "Mx");
    tap(&mut m, &mut r, Control::Circle);
    assert_eq!(m.screen(), Some(Screen::Main));
    // The HUD off (Display: the row's other choice since P170), wrecks off (Ride's Game Modifiers
    // since P175).
    settings_at(&mut m, &mut r, "Display");
    down_to(&mut m, &mut r, menu::Key::Hud);
    tap(&mut m, &mut r, Control::Right);
    tap(&mut m, &mut r, Control::L1);
    down_to(&mut m, &mut r, menu::Key::Modifiers);
    tap(&mut m, &mut r, Control::Cross);
    down_to(&mut m, &mut r, menu::Key::Wrecks);
    tap(&mut m, &mut r, Control::Right);
    assert!(m.take_changed());
    tap(&mut m, &mut r, Control::Circle);
    assert_eq!(m.screen(), Some(Screen::Settings));
    tab_to(&mut m, &mut r, "Controls");
    // 1P's bike and biplane controls are rows of their own, each round either way (P50, P81): the
    // bike's left from New wraps to the original's last preset.
    let one = |m: &Menu| (m.controls.layouts[0], m.controls.planes[0]);
    down_to(&mut m, &mut r, menu::Key::BikeLayout);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(one(&m), (Layout::Original, Layout::New));
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(one(&m), (Layout::New, Layout::New));
    tap(&mut m, &mut r, Control::Left);
    assert_eq!((one(&m), m.controls.presets[0]), ((Layout::Original, Layout::New), 3));
    tap(&mut m, &mut r, Control::Right);
    assert_eq!((one(&m), m.controls.presets[0]), ((Layout::New, Layout::New), 0), "New is on preset 0");
    down_to(&mut m, &mut r, menu::Key::PlaneLayout);
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(one(&m), (Layout::New, Layout::Original));
    tap(&mut m, &mut r, Control::Right);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(one(&m), (Layout::New, Layout::Original));
    assert_eq!(m.controls.planes[1], Layout::New, "2P's biplane moved with 1P's");

    let dir = scratch("pad");
    menu::save(&dir, &m.settings, &m.controls).unwrap();
    let (settings, controls) = menu::load(&dir);
    assert_eq!(
        settings,
        Settings { name: "Mx".into(), overlay: Overlay::None, wrecks: false, ..Settings::default() }
    );
    assert_eq!(controls.throttle, Gas::Cross);
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
        // Every tab of a screen that has them (P118).
        let shown = m.screen().unwrap();
        for _ in 0..m.tabs(shown).len().max(1) {
            let tab = m.tab(shown);
            // A grey stand-in for the ride, which the pause draws over rather than covers.
            const RIDE: [u8; 3] = [90, 90, 90];
            let mut frame = game_render::Frame::new(320, 224, RIDE);
            art.draw(&m, &mut frame);
            let drawn = frame.color.chunks(3).filter(|p| *p != RIDE).count();
            assert!(drawn > frame.width * frame.height * 9 / 10, "{s:?} tab {tab} drew over {drawn} pixels");
            m.step(Press { r1: true, ..Press::default() });
        }
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
    if let Act::Switch(seat, to) = act {
        // The menu's part is which machine; where it spawns is the garage's, over a floor here.
        let (mut sim, mut floor) = (game_sim::Sim::new(0), game_sim::world::Flat::default());
        let up = garages[seat].switch(Kind::parse(to.word()).unwrap(), &mut sim, &mut floor).unwrap();
        m.switched(seat, Machine::from_word(up.name()).unwrap());
    }
    act
}

/// One press and release of `control` on `seat`'s pad, the others at rest.
fn tap_seat(m: &mut Menu, readers: &mut [Reader], garages: &mut [Garage], seat: usize, control: Control) -> Vec<Act> {
    let mut acts = Vec::new();
    for pressed in [true, false, false, false] {
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

    // 2P's Start pauses for 2P alone (P84), and 1P's pad does nothing to it.
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Start);
    assert_eq!(m.view(1).screen(), Some(Screen::Pause));
    assert!(m.paused(1) && !m.paused(0) && m.screen().is_none());
    assert_eq!(m.view(1).pause_rows()[2], "Switch To Machine: Biplane");
    tap_seat(&mut m, &mut readers, &mut garages, 0, Control::Down);
    assert_eq!(m.view(1).cursor(Screen::Pause), 0);
    // Down twice and Cross on 2P's pad: 2P is in the plane, the ride carries on, 1P is on the bike.
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Down);
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Down);
    let acts = tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Cross);
    assert!(acts.contains(&Act::Switch(1, Machine::Biplane)), "{acts:?}");
    assert!(m.riding());
    assert_eq!((garages[0].up, garages[1].up), (Kind::Bike, Kind::Biplane));
    assert_eq!(m.up[..2], [Machine::Bike, Machine::Biplane]);

    // 1P's own pause switches 1P, and the row now offers 2P the bike back.
    tap_seat(&mut m, &mut readers, &mut garages, 0, Control::Start);
    assert!(m.paused(0) && !m.paused(1));
    tap_seat(&mut m, &mut readers, &mut garages, 0, Control::Down);
    tap_seat(&mut m, &mut readers, &mut garages, 0, Control::Down);
    let acts = tap_seat(&mut m, &mut readers, &mut garages, 0, Control::Cross);
    assert!(acts.contains(&Act::Switch(0, Machine::Biplane)), "{acts:?}");
    assert_eq!((garages[0].up, garages[1].up), (Kind::Biplane, Kind::Biplane));
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Start);
    assert_eq!(m.view(1).pause_rows()[2], "Switch To Machine: Bike");
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Start);
    assert!(m.riding(), "2P's Start resumes 2P's pause");

    // A player with no plane to take: the row says so and pressing it switches nothing.
    m.switchable[1] = false;
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Start);
    assert_eq!(m.view(1).pause_rows()[2], "Switch To Machine: None");
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Down);
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Down);
    let acts = tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Cross);
    assert!(!acts.iter().any(|a| matches!(a, Act::Switch(..))), "{acts:?}");
    assert_eq!(m.view(1).screen(), Some(Screen::Pause));
    assert_eq!(garages[1].up, Kind::Biplane);
}

/// P177: no pick of a start machine is left. The ride setup's Square, L1 and R1 change nothing,
/// the ride starts on the bike, and an old settings.txt with its machines line loads and drops it.
#[test]
fn a_ride_starts_on_the_bike() {
    let (mut m, mut r) = fresh();
    m.riders = 2;
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Tracks));
    let before = m.settings.clone();
    for c in [Control::Square, Control::R1, Control::Square, Control::L1] {
        let acts = tap(&mut m, &mut r, c);
        assert!(acts.iter().all(|a| *a == Act::Stay), "{c:?}: {acts:?}");
        assert_eq!(m.screen(), Some(Screen::Tracks));
    }
    assert_eq!(m.settings, before);
    assert!(!m.take_changed());
    let acts = tap(&mut m, &mut r, Control::Cross);
    assert!(acts.iter().any(|a| matches!(a, Act::Ride(_))), "{acts:?}");
    // The Ride tab has no Start On row.
    let (mut m, mut r) = fresh();
    main_to(&mut m, &mut r, "Settings");
    tab_to(&mut m, &mut r, "Ride");
    assert!(m.page().iter().all(|row| row.label != "Start On"));
    // An old file's machines line loads, and is not written back.
    let old = Settings::parse("track = free07\nmachines = biplane biplane bike\n");
    assert_eq!(old, Settings { track: "free07".into(), ..Settings::default() });
    assert!(!old.text().contains("machines"));
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
    // The PS2's shape, 1080p, 21:9 and a portrait phone.
    let windows = [(640, 448), (1920, 1080), (2520, 1080), (360, 640)];
    let mut pills = 0;
    for (w, h) in windows {
        for screen in Screen::ALL {
            let (mut m, mut reader) = fresh();
            m.open(screen);
            let shown = m.screen().unwrap();
            // Every row of a list, and a walk across the keyboard's keys and down to its commands.
            for step in 0..12 {
                assert_eq!(m.screen(), Some(shown));
                let mut frame = game_render::Frame::new(w, h, [90, 90, 90]);
                let parts = art.parts(&m, &mut frame);
                let logos: Vec<_> = parts
                    .iter()
                    .filter_map(|p| match p {
                        menu::Part::Logo(b) => Some(*b),
                        _ => None,
                    })
                    .collect();
                for p in &parts {
                    match p {
                        menu::Part::Pill { pill, label } => {
                            pills += 1;
                            // The label inside its pill, the slant's lean included, and on its middle.
                            assert!(
                                label[0] >= pill[0] - 1.0 && label[0] + label[2] <= pill[0] + pill[2] + 1.0,
                                "{screen:?} at {w}x{h}: a label {label:?} past its pill {pill:?}"
                            );
                            let (pc, lc) = (pill[1] + pill[3] / 2.0, label[1] + label[3] / 2.0);
                            assert!((pc - lc).abs() <= pill[3] * 0.2, "{screen:?} at {w}x{h}: a label {label:?} off its pill {pill:?}");
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
                        menu::Part::Card { card, .. } => {
                            for logo in &logos {
                                assert!(!overlap(*logo, *card), "{screen:?} at {w}x{h}: a card on the logo");
                            }
                        }
                    }
                }
                let lit = matches!(
                    shown,
                    Screen::Main | Screen::Pause | Screen::Settings | Screen::Online | Screen::Games | Screen::Lobby | Screen::Split | Screen::Modifiers | Screen::Garage | Screen::Mods | Screen::Keyboard
                );
                if lit {
                    let n = parts.iter().filter(|p| matches!(p, menu::Part::Pill { .. })).count();
                    assert_eq!(n, 1, "{screen:?} at {w}x{h}, step {step}: one pill under the cursor");
                }
                if matches!(shown, Screen::Main | Screen::Splash) {
                    assert_eq!(logos.len(), 1, "{screen:?} at {w}x{h}: no logo");
                }
                let control = if shown == Screen::Keyboard && step % 3 != 2 { Control::Right } else { Control::Down };
                tap(&mut m, &mut reader, control);
            }
        }
    }
    assert!(pills >= 11 * 12 * 4, "{pills} pills");
}

#[test]
fn a_picked_bike_and_rider_reach_the_ride_that_starts_per_seat() {
    use game_render::rider::{Rider as Body, Sex};
    use game_sim::phy::Phy;
    use game_sim::track::{Engine, Tuning};
    use menu::Key;
    use roster::{Catalogue, Picks, Row as G};
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
    let r = &mut reader;

    // The garage from the main menu, on 1P (P118: one pad dresses every seat by its Player row).
    main_to(&mut m, r, "Garage");
    assert_eq!(m.screen(), Some(Screen::Garage));
    assert_eq!(m.page()[0].value, "1P");
    // 1P: the Bike tab's class right (125 to 250) and Tier to Pro, then the Rider tab's next rider.
    tab_to(&mut m, r, "Bike");
    down_to(&mut m, r, Key::Garage(G::Class));
    tap(&mut m, r, Control::Right);
    down_to(&mut m, r, Key::Garage(G::Tier));
    tap(&mut m, r, Control::Right);
    tab_to(&mut m, r, "Rider");
    down_to(&mut m, r, Key::Garage(G::Rider));
    tap(&mut m, r, Control::Right);
    // 2P: class left (125 to 50), Tuning to A, and the rider two along in its second colours.
    tab_to(&mut m, r, "Bike");
    down_to(&mut m, r, Key::Player);
    tap(&mut m, r, Control::Right);
    assert_eq!(m.page()[0].value, "2P");
    down_to(&mut m, r, Key::Garage(G::Class));
    tap(&mut m, r, Control::Left);
    down_to(&mut m, r, Key::Garage(G::Tuning));
    tap(&mut m, r, Control::Right);
    tab_to(&mut m, r, "Rider");
    assert_eq!(m.page()[0].value, "2P", "the Player row is every tab's");
    down_to(&mut m, r, Key::Garage(G::Rider));
    tap(&mut m, r, Control::Right);
    tap(&mut m, r, Control::Right);
    down_to(&mut m, r, Key::Garage(G::Colours));
    tap(&mut m, r, Control::Right);
    // Nobody touched 3P's.
    assert_eq!(m.garage(2).unwrap().pick, roster::Pick::default());
    let one = m.garage(0).unwrap().pick.clone();
    let two = m.garage(1).unwrap().pick.clone();
    // Circle leaves, and the picks are kept.
    tap(&mut m, r, Control::Circle);
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
    main_to(&mut m, r, "Free Ride");
    let acts = tap(&mut m, r, Control::Cross);
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

    // And every row of the three tabs is reached by pad and changes the seat's pick.
    for tab in menu::GARAGE_TABS {
        let start = || {
            let mut reader = Reader::new();
            let mut m = Menu::new(Settings::default(), Controls::default()).with_garage(catalogue.clone(), Picks::default());
            m.step(reader.read(&Report::idle()));
            main_to(&mut m, &mut reader, "Garage");
            tab_to(&mut m, &mut reader, tab);
            (m, reader)
        };
        every_row_changes(&start, &format!("Garage {tab}"));
    }
}

// ---- the rider's look (P86) ----------------------------------------------------------------------

#[test]
fn a_seat_dresses_its_rider_saves_the_look_and_a_second_seat_rides_it_after_a_restart() {
    use menu::Key;
    use roster::{Catalogue, Piece, Picks, Presets, Row};

    let mut disc = match game_render::Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let catalogue = Catalogue::read(&mut disc).unwrap();
    assert!(!catalogue.wardrobe.outfits.is_empty() && Piece::ALL.iter().all(|&p| catalogue.wardrobe.pieces[p as usize].len() > 1));
    let mut reader = Reader::new();
    let mut m = Menu::new(Settings::default(), Controls::default()).with_garage(catalogue.clone(), Picks::default()).with_looks(Presets::default());
    m.step(reader.read(&Report::idle()));
    let r = &mut reader;
    main_to(&mut m, r, "Garage");
    assert_eq!(m.screen(), Some(Screen::Garage));
    let stock = m.garage(0).unwrap().pick.clone();

    // 1P: the next colours, the next jersey and pants.
    down_to(&mut m, r, Key::Garage(Row::Colours));
    tap(&mut m, r, Control::Right);
    down_to(&mut m, r, Key::Garage(Row::Outfit));
    tap(&mut m, r, Control::Right);
    // Its name: Cross opens the menu's keyboard, the letters, Start keeps them.
    down_to(&mut m, r, Key::Garage(Row::Name));
    tap(&mut m, r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Keyboard));
    key_command(&mut m, r, 2);
    type_keys(&mut m, r, "Kaito");
    tap(&mut m, r, Control::Start);
    assert_eq!(m.screen(), Some(Screen::Garage), "the keys go back to the garage");
    assert_eq!(m.garage(0).unwrap().pick.look.name, "Kaito");
    // Its number: one key too many is deleted with Square, then Done on the command row.
    down_to(&mut m, r, Key::Garage(Row::Number));
    tap(&mut m, r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Keyboard));
    key_command(&mut m, r, 2);
    type_keys(&mut m, r, "720");
    tap(&mut m, r, Control::Square);
    key_command(&mut m, r, 3);
    assert_eq!(m.screen(), Some(Screen::Garage), "Done closes the keys");
    // Left and right step the number too.
    tap(&mut m, r, Control::Right);
    tap(&mut m, r, Control::Left);
    // The Gear tab: a helmet, the second goggles and the last boots.
    tab_to(&mut m, r, "Gear");
    down_to(&mut m, r, Key::Garage(Row::Piece(Piece::Helmet)));
    tap(&mut m, r, Control::Right);
    down_to(&mut m, r, Key::Garage(Row::Piece(Piece::Goggles)));
    tap(&mut m, r, Control::Right);
    tap(&mut m, r, Control::Right);
    down_to(&mut m, r, Key::Garage(Row::Piece(Piece::Boots)));
    tap(&mut m, r, Control::Left);
    let one = m.garage(0).unwrap().pick.clone();
    println!("seat 1: {} | {}", one.rider, one.look.text());
    assert_eq!((one.look.name.as_str(), one.look.number.as_str()), ("Kaito", "72"));
    assert_ne!(one.rider, stock.rider, "the colours changed the gear set");
    assert!(!one.look.outfit.is_empty());
    assert!(Piece::ALL.iter().filter(|&&p| p != Piece::Gloves).all(|&p| !one.look.piece(p).is_empty()), "{:?}", one.look);
    assert_eq!(one.look.piece(Piece::Gloves), "", "gloves stay the gear set's");
    assert_eq!(one.look.piece(Piece::Boots), catalogue.wardrobe.pieces[Piece::Boots as usize].last().unwrap());

    // Cross on Save as Preset, back on the Rider tab, saves it under its name and number.
    tab_to(&mut m, r, "Rider");
    down_to(&mut m, r, Key::Garage(Row::Preset));
    tap(&mut m, r, Control::Cross);
    assert!(m.take_looks_changed());
    assert_eq!(m.looks.keys(), ["Kaito 72"]);
    assert_eq!(m.screen(), Some(Screen::Garage), "saving does not leave the garage");

    // 2P, by the Player row: right on the Preset row loads the saved look.
    down_to(&mut m, r, Key::Player);
    tap(&mut m, r, Control::Right);
    assert_eq!(m.page()[0].value, "2P");
    assert_ne!(m.garage(1).unwrap().pick.look, one.look);
    tap(&mut m, r, Control::Up);
    assert_eq!(m.page()[m.cursor(Screen::Garage)].key, Key::Garage(Row::Preset), "up from the top is the last row");
    tap(&mut m, r, Control::Right);
    let loaded = m.garage(1).unwrap().pick.clone();
    assert_eq!((&loaded.bike, loaded.set, &loaded.rider, &loaded.look, loaded.preset.as_str()), (&one.bike, one.set, &one.rider, &one.look, "Kaito 72"));

    // Circle leaves; the picks carry the looks into the ride.
    tap(&mut m, r, Control::Circle);
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
            let tex = kit.texture(&mut disc, &p.rider, &p.look, &p.bike).expect("a name to paint");
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

// ---- the dressed rider beside the original (P91) -----------------------------------------------

#[test]
fn a_long_name_and_a_three_digit_number_are_painted_as_the_original_paints_them() {
    let mut disc = match game_render::Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let mut kit = dress::Kit::open(&mut disc);
    // A pro rider's gear set wears a sponsor's `[JerseyPants]` entry, as the original's uniform
    // table in RAM has it; a stock set its own.
    let worn = |kit: &mut dress::Kit, disc: &mut game_render::Disc, set: &str| kit.uniform(disc, set).map(|o| (o.id.clone(), o.name_font.clone()));
    for (set, entry, font) in [
        ("thq1", "thq1", "name3"),
        ("thor1", "thor1", "name3_black"),
        ("parsons1", "skin1", "name3_black"),
        ("parsons2", "skin2", "name3"),
        ("faisst1", "generic1", "name3"),
        ("renner1", "tld1", "name3"),
        ("langston1", "thor1", "name3_black"),
        ("lusk1", "generic2", "name3"),
        ("preston2", "shift2", "name3"),
    ] {
        assert_eq!(worn(&mut kit, &mut disc, set), Some((entry.to_string(), font.to_string())), "{set}");
    }

    // A custom THQ1 rider called Maximilianus, 888, on a white-digit and a black-digit bike: the
    // original's own NameDefault0 for each, read out of PCSX2's RAM, has this many texels painted on
    // each row. The name is too wide for the texture and the original does not fit it: its first
    // letters come round at the end of the row above (row 10) and its last at the start of the row
    // below (row 21).
    const ROWS: [usize; 128] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10, 55, 56, 56, 57, 56, 57, 59, 60, 60, 59, 10, 0, 0, 0, 24, 30, 36, 36, 42, 42, 42, 39, 39, 39, 39, 42,
        42, 36, 33, 42, 48, 48, 42, 45, 45, 42, 45, 48, 48, 48, 42, 39, 33, 27, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 21, 27, 33, 33, 33, 24, 24,
        24, 24, 24, 27, 27, 33, 39, 30, 30, 30, 30, 39, 39, 39, 33, 27, 0, 0, 0, 0, 0, 0, 0, 21, 27, 33, 33, 33, 24, 24, 24, 24, 24, 27, 27,
        33, 39, 30, 30, 30, 30, 39, 39, 39, 33, 27, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    let look = roster::Look { name: "Maximilianus".into(), number: "888".into(), ..Default::default() };
    for (bike, plate, digit) in [("cr125_thq_01", "plate3", 255), ("cr250_thq_01", "plate3_black", 0)] {
        let worn = kit.uniform(&mut disc, "thq1").cloned();
        let faces = dress::Faces::of(&kit.wardrobe, "thq1", worn.as_ref(), &look, bike);
        assert_eq!(faces, dress::Faces { name: "name3".into(), number: "number3".into(), plate: plate.into() });
        let t = kit.texture(&mut disc, "thq1", &look, bike).unwrap();
        let px = |x: usize, y: usize| &t.rgba[(y * 64 + x) * 4..][..4];
        let rows: Vec<usize> = (0..128).map(|y| (0..64).filter(|&x| px(x, y)[3] > 0).count()).collect();
        assert_eq!(rows, ROWS, "{bike}");
        // Colours as the original's palette has them, within its quantizer's 5: the name's dark
        // edge, the number's white fill, and the plate digit white on one bike and black on the other.
        let near = |p: &[u8], want: [u8; 4]| p.iter().zip(want).all(|(&a, b)| a.abs_diff(b) <= 5);
        assert!(near(px(0, 21), [0, 0, 8, 255]), "{:?}", px(0, 21));
        assert!(near(px(14, 30), [248, 248, 248, 255]), "{:?}", px(14, 30));
        assert!(near(px(20, 100), [digit, digit, digit, 255]), "{bike} {:?}", px(20, 100));
    }
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
    for pressed in [true, false, false, false] {
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
    tap_device(&mut m, &mut rd, &all, 0, Control::Down);
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
fn one_pad_starts_a_local_game_alone_and_can_free_ride() {
    // Alone on the join screen (P171), Start goes on — with other pads plugged in that have not
    // joined too — straight to the track pick, there being no layout to pick for one view, and the
    // ride is a local game of one on the host's pad seat, open to the others as it runs (P76).
    for pads in [1, 2] {
        let all: Vec<usize> = (0..pads).collect();
        let (mut m, mut rd) = fresh_with(pads);
        tap_device(&mut m, &mut rd, &all, 0, Control::Down);
        tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
        assert!(m.join.can_start(), "{pads} pads plugged in, one joined");
        tap_device(&mut m, &mut rd, &all, 0, Control::Start);
        assert_eq!(m.screen(), Some(Screen::Tracks), "{pads} pads: no split screen for one");
        let party = m.party().expect("a local game of one").clone();
        assert_eq!((party.players(), party.devices.clone(), m.riders), (1, vec![0], 1));
        assert_eq!(party.layout(), split::Layout::Full);
        // Back is the join screen again, the host still on it.
        tap_device(&mut m, &mut rd, &all, 0, Control::Circle);
        assert_eq!(m.screen(), Some(Screen::Join));
        assert_eq!(m.join.views().len(), 1);
        tap_device(&mut m, &mut rd, &all, 0, Control::Start);
        let acts = tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
        assert!(acts.iter().any(|a| matches!(a, Act::Ride(_))), "{acts:?}");
        assert_eq!(m.ride_devices(), [0]);
    }
    // A pad that joins and leaves again leaves the host alone, who still starts.
    let all = [0, 1];
    let (mut m, mut rd) = fresh_with(2);
    tap_device(&mut m, &mut rd, &all, 0, Control::Down);
    tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
    tap_device(&mut m, &mut rd, &all, 1, Control::Cross);
    tap_device(&mut m, &mut rd, &all, 1, Control::Circle);
    // and a joiner who is not the host may be the one alone: the host leaves, pad seat 1 starts
    tap_device(&mut m, &mut rd, &all, 1, Control::Cross);
    tap_device(&mut m, &mut rd, &all, 0, Control::Circle);
    tap_device(&mut m, &mut rd, &all, 1, Control::Start);
    assert_eq!(m.screen(), Some(Screen::Tracks));
    assert_eq!(m.party().map(|p| p.devices.clone()), Some(vec![1]));
    // Back on the main menu, the same pad rides a Free Ride on its own.
    tap_device(&mut m, &mut rd, &all, 0, Control::Circle);
    tap_device(&mut m, &mut rd, &all, 1, Control::Circle);
    tap_device(&mut m, &mut rd, &all, 0, Control::Circle);
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

/// P42's sound rows, on the Audio tab since P118: Music Plays steps Shuffle, In Order, One Song and
/// Off, the Song row picks the one song among the shell's sources, and the sliders stop at their
/// ends.
#[test]
fn the_sound_rows_pick_the_music_and_the_volumes() {
    use menu::Key;
    let (mut m, mut r) = fresh();
    m.music_choices = vec![("disc:ooc".into(), "Out of Control - Hoobastank".into()), ("folder:mine.wav".into(), "mine".into())];
    settings_at(&mut m, &mut r, "Audio");
    let value = |m: &Menu| m.page()[m.cursor(Screen::Settings)].value.clone();
    down_to(&mut m, &mut r, Key::MusicPlays);
    assert_eq!(m.settings.music, "shuffle");
    assert_eq!(value(&m), "Shuffle");
    tap(&mut m, &mut r, Control::Right);
    assert_eq!((m.settings.music.as_str(), value(&m).as_str()), ("order", "In Order"));
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.music, "disc:ooc", "One Song is on the song row's");
    assert_eq!(value(&m), "One Song");
    assert_eq!(m.take_tunes(), [menu::Tune::Play("disc:ooc".into())]);
    // The Song row: right is the next song, which plays.
    tap(&mut m, &mut r, Control::Down);
    assert_eq!(value(&m), "Out of Control - Hoobastank");
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.music, "folder:mine.wav");
    assert_eq!(value(&m), "mine");
    assert_eq!(m.take_tunes(), [menu::Tune::Play("folder:mine.wav".into())]);
    // Off dims the Song row and Skip Song and says why.
    tap(&mut m, &mut r, Control::Up);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.music, "off");
    let rows = m.page();
    let off = |k: Key| rows.iter().find(|r| r.key == k).unwrap().off.clone();
    assert_eq!(off(Key::Song).as_deref(), Some("Only with Music Plays: One Song"));
    assert_eq!(off(Key::SkipSong).as_deref(), Some("The music is off"));
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.settings.music, "folder:mine.wav", "back to the one song");
    // The music volume, down to nothing and no further, by fives from 70 (P56).
    down_to(&mut m, &mut r, Key::Level(0));
    for _ in 0..16 {
        tap(&mut m, &mut r, Control::Left);
    }
    assert_eq!(m.settings.music_volume, 0);
    // The effects, up to the top and no further.
    down_to(&mut m, &mut r, Key::Level(2));
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
/// and B, the bottom button, takes the row (P64) — tapped between frames, and held across them. The report the new scheme
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
    // B (js 0), the bottom one, held across a frame: the scheme's report has lost it, the pad's own
    // has not.
    pro.button(0, true);
    let (report, taps) = seat_frame(&mut seats);
    assert!(report.buttons() & Control::Cross.mask() != 0);
    assert!(pad::Scheme::New.ride(report).buttons() & Control::Cross.mask() == 0, "the new scheme moves R2 onto Cross and Cross off");
    let act = m.step_seats(&[r.read_taps(&report, taps)]);
    assert_eq!(act, Act::Restart);
    pro.button(0, false);
    seat_frame(&mut seats);
    std::fs::remove_dir_all(&root).unwrap();
}

/// The splash at any frame rate: an idle pad plugged in first (a virtual one some program left,
/// its stick wandering), and the user's pad's B, the bottom button (P64), pressed for three 60 Hz ticks somewhere — the
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
                    pro.button(0, true);
                }
                if tick == 63 + phase {
                    pro.button(0, false);
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

/// P64: a Switch Pro Controller is laid out like every other pad unless its player asks — its
/// bottom button (B, js 0) takes the splash and confirms, its right one (A, js 1) goes back — even
/// with a `controls.txt` that P53's default wrote `nintendo = labels` into. The Controls tab's
/// Nintendo A/B row, worked by the pad alone, puts it on its labels at once (the shell's
/// `seats.set_wiring` on a changed menu, as main.rs does): A confirms and B goes back. The pick is
/// saved, and the next start comes up on labels: A takes the splash, and + does either way.
#[test]
fn a_nintendo_pad_s_bottom_button_confirms_until_its_row_says_labels() {
    let _fifos = pad::tests::FIFOS.lock().unwrap_or_else(|e| e.into_inner());
    let root = pad::tests::scratch("menu-nintendo");
    let (dev, sys, dir) = (root.join("dev"), root.join("sys"), root.join("files"));
    std::fs::create_dir_all(&dev).unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(menu::CONTROLS_FILE), "throttle = cross\npad = auto\npressure = none\nlayout = new\nnintendo = labels\n").unwrap();
    let mut pro = pro_controller(&dev, &sys, "js0");
    let wiring = |c: &Controls| pad::Wiring { nintendo_positions: c.nintendo_positions, ..pad::Wiring::default() };
    let dev_of = |d: pad::Device<String>| match d {
        pad::Device::Keys => Dev::Keys,
        pad::Device::Pad(id) => Dev::Pad(id),
    };
    // One menu frame of every device, and the shell's answer to a changed menu.
    let frame = |m: &mut Menu, seats: &mut pad::Seats, readers: &mut Readers| {
        let got = pad::tests::devices_now(seats, &|_| false);
        let reports: Vec<(Dev, Report, u16)> = got.into_iter().map(|(d, r, t)| (dev_of(d), r, t)).collect();
        m.step_devices(&readers.read_taps(&reports));
        if m.take_changed() {
            seats.set_wiring(wiring(&m.controls));
            menu::save(&dir, &m.settings, &m.controls).unwrap();
        }
    };
    // A press and release of one js button between two frames, or of the hat along one axis.
    let press = |m: &mut Menu, seats: &mut pad::Seats, readers: &mut Readers, pro: &mut pad::tests::FakePad, js: u8| {
        pro.button(js, true);
        pro.button(js, false);
        // Rested, as a hand is between two taps.
        for _ in 0..3 {
            frame(m, seats, readers);
        }
    };
    let hat = |m: &mut Menu, seats: &mut pad::Seats, readers: &mut Readers, pro: &mut pad::tests::FakePad, axis: u8, v: i16| {
        pro.axis(axis, v);
        pro.axis(axis, 0);
        // Rested, as a hand is between two taps.
        for _ in 0..3 {
            frame(m, seats, readers);
        }
    };
    let (b, a, plus, r) = (0u8, 1u8, 10u8, 6u8);
    let start = |seats: &mut pad::Seats| {
        let (settings, controls) = menu::load(&dir);
        seats.set_wiring(wiring(&controls));
        let mut m = Menu::new(settings, controls);
        m.splash();
        (m, Readers::new())
    };

    // The old file's `labels` is not honoured: by position.
    let mut seats = pad::tests::seats_in(&dev, &sys, pad::Wiring::default());
    let (mut m, mut readers) = start(&mut seats);
    assert!(m.controls.nintendo_positions, "P53's default labels migrate to positions");
    frame(&mut m, &mut seats, &mut readers);
    // A on the splash goes back, which asks to quit; A again leaves the question.
    press(&mut m, &mut seats, &mut readers, &mut pro, a);
    assert_eq!(m.p1(), None);
    assert_eq!(m.screen(), Some(Screen::Splash));
    press(&mut m, &mut seats, &mut readers, &mut pro, a);
    // B is 1P's press start.
    press(&mut m, &mut seats, &mut readers, &mut pro, b);
    assert!(m.p1().is_some(), "the bottom button takes the splash");
    for _ in 0..CLAIMED_FRAMES {
        frame(&mut m, &mut seats, &mut readers);
    }
    assert_eq!(m.screen(), Some(Screen::Main));
    // B confirms Settings, A goes back out, B goes in again.
    let at = menu::MAIN.iter().position(|n| *n == "Settings").unwrap();
    for _ in 0..64 {
        if m.cursor(Screen::Main) == at {
            break;
        }
        hat(&mut m, &mut seats, &mut readers, &mut pro, 5, i16::MAX);
    }
    assert_eq!(m.cursor(Screen::Main), at);
    press(&mut m, &mut seats, &mut readers, &mut pro, b);
    assert_eq!(m.screen(), Some(Screen::Settings), "B confirms");
    press(&mut m, &mut seats, &mut readers, &mut pro, a);
    assert_eq!(m.screen(), Some(Screen::Main), "A goes back");
    press(&mut m, &mut seats, &mut readers, &mut pro, b);
    assert_eq!(m.screen(), Some(Screen::Settings));

    // R along the tabs to Controls, the hat down to the row, which reads By Position.
    for _ in 0..8 {
        if m.tabs(Screen::Settings)[m.tab(Screen::Settings)] == "Controls" {
            break;
        }
        press(&mut m, &mut seats, &mut readers, &mut pro, r);
    }
    assert_eq!(m.tabs(Screen::Settings)[m.tab(Screen::Settings)], "Controls");
    for _ in 0..16 {
        if m.page()[m.cursor(Screen::Settings)].key == menu::Key::Nintendo {
            break;
        }
        hat(&mut m, &mut seats, &mut readers, &mut pro, 5, i16::MAX);
    }
    let row = &m.page()[m.cursor(Screen::Settings)];
    assert_eq!((row.key, row.label.as_str(), row.value.as_str()), (menu::Key::Nintendo, "Nintendo A/B", "By Position"));
    // The hat's right flips it, and from the next frame the pad is on its labels.
    hat(&mut m, &mut seats, &mut readers, &mut pro, 4, i16::MAX);
    assert_eq!(m.page()[m.cursor(Screen::Settings)].value, "By Label");
    assert!(!m.controls.nintendo_positions);
    assert!(std::fs::read_to_string(dir.join(menu::CONTROLS_FILE)).unwrap().contains("nintendo = by-label\n"));
    // B goes back now, and A confirms.
    press(&mut m, &mut seats, &mut readers, &mut pro, b);
    assert_eq!(m.screen(), Some(Screen::Main), "B goes back under labels");
    press(&mut m, &mut seats, &mut readers, &mut pro, a);
    assert_eq!(m.screen(), Some(Screen::Settings), "A confirms under labels");

    // A restart: labels, picked in the menu, are kept. B on the splash asks to quit; A takes it.
    drop(seats);
    let mut seats = pad::tests::seats_in(&dev, &sys, pad::Wiring::default());
    let (mut m, mut readers) = start(&mut seats);
    assert!(!m.controls.nintendo_positions, "the pick survives a restart");
    frame(&mut m, &mut seats, &mut readers);
    press(&mut m, &mut seats, &mut readers, &mut pro, b);
    assert_eq!(m.p1(), None, "B is not press start under labels");
    press(&mut m, &mut seats, &mut readers, &mut pro, b);
    press(&mut m, &mut seats, &mut readers, &mut pro, a);
    assert!(m.p1().is_some(), "A takes the splash under labels");

    // And + takes the splash whichever way the pad reads.
    for positions in [true, false] {
        let mut seats = pad::tests::seats_in(&dev, &sys, pad::Wiring { nintendo_positions: positions, ..pad::Wiring::default() });
        let mut m = Menu::new(Settings::default(), Controls { nintendo_positions: positions, ..Controls::default() });
        m.splash();
        let mut readers = Readers::new();
        frame(&mut m, &mut seats, &mut readers);
        press(&mut m, &mut seats, &mut readers, &mut pro, plus);
        assert!(m.p1().is_some(), "+ with positions {positions}");
    }
    std::fs::remove_dir_all(&root).unwrap();
}

/// P55: the Ride tab's Game Modifiers row opens the modifiers; the arrows step a modifier by ten
/// percent, the world's for everyone and the rest for the Player row's player; a preset saved by
/// name loads back; and Reset to Original puts every modifier back, which writes the settings file
/// as it was before there were modifiers.
#[test]
fn the_game_modifiers_screen_sets_saves_and_resets() {
    use menu::Key;
    let (mut m, mut r) = fresh();
    settings_at(&mut m, &mut r, "Ride");
    down_to(&mut m, &mut r, Key::Modifiers);
    assert_eq!(m.page()[m.cursor(Screen::Settings)].value, "Original");
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Modifiers));
    let rows = m.page();
    assert_eq!(rows.len(), 1 + Knob::MENU.len() + 4 + 1 + 4, "{rows:?}");
    assert_eq!(rows.last().unwrap().label, "Reset to Original");
    assert!(rows.iter().filter(|r| r.world).all(|r| !matches!(r.key, Key::Knob(k) if k.per_seat())));

    // Gravity, a world modifier, down twice: 80 for everyone.
    down_to(&mut m, &mut r, Key::Knob(Knob::Gravity));
    tap(&mut m, &mut r, Control::Left);
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.settings.modifiers.get(0, Knob::Gravity), 80);
    assert_eq!(m.settings.modifiers.get(3, Knob::Gravity), 80);
    assert!(m.take_changed());

    // Bike top speed for 2P alone: the player row to 2P, then the speed row right once.
    down_to(&mut m, &mut r, Key::Player);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.page()[0].value, "2P");
    down_to(&mut m, &mut r, Key::Knob(Knob::BikeSpeed));
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.modifiers.get(1, Knob::BikeSpeed), 110);
    assert_eq!(m.settings.modifiers.get(0, Knob::BikeSpeed), 100);
    assert!(!m.settings.modifiers.seat_is_original(0), "gravity reaches 1P too");

    // Saved by a name typed on the keyboard: its first key, then Start.
    down_to(&mut m, &mut r, Key::SavePreset);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Keyboard));
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Start);
    assert_eq!(m.screen(), Some(Screen::Modifiers));
    assert_eq!(m.settings.presets.len(), 1);
    let saved = m.settings.modifiers;
    // 2P's set (P143): 2P's own top speed, for any player to load, and the world's gravity.
    assert_eq!(m.settings.presets[0].1, menu::preset_of(&saved, 1));
    assert_eq!(m.settings.presets[0].1.get(0, Knob::BikeSpeed), 110);
    // Saved on 2P's page, it is 2P's for every player, with the world's (P134).
    let mut kept = Mods::original();
    kept.set(0, Knob::Gravity, 80);
    (0..8).for_each(|s| kept.set(s, Knob::BikeSpeed, 110));
    assert_eq!(m.settings.presets[0].1, kept);

    // Reset to Original, and the preset loads it all back.
    down_to(&mut m, &mut r, Key::ResetMods);
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.settings.modifiers.is_original());
    down_to(&mut m, &mut r, Key::LoadPreset);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.settings.modifiers, saved);
    // Back on the Ride tab, its row counts them.
    tap(&mut m, &mut r, Control::Circle);
    assert_eq!(m.page()[m.cursor(Screen::Settings)].value, "2 Changed");

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

/// P136: Biplane max height steps past 500% to Uncapped, which the row says in words, and back.
#[test]
fn the_biplane_max_height_tops_out_uncapped() {
    use menu::Key;
    let (mut m, mut r) = fresh();
    settings_at(&mut m, &mut r, "Ride");
    down_to(&mut m, &mut r, Key::Modifiers);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Modifiers));
    m.settings.modifiers.set(0, Knob::PlaneCeiling, 490);
    down_to(&mut m, &mut r, Key::Knob(Knob::PlaneCeiling));
    let row = |m: &Menu| m.page().into_iter().find(|r| r.key == Key::Knob(Knob::PlaneCeiling)).unwrap();
    assert_eq!(row(&m).value, "490%");
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(row(&m).value, "500%");
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(row(&m).value, "Uncapped");
    assert!(m.settings.modifiers.uncapped(0) && !m.settings.modifiers.uncapped(1));
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(row(&m).value, "Uncapped", "there is no step past Uncapped");
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.settings.modifiers.get(0, Knob::PlaneCeiling), 500);
    // Cross puts it back to the original's.
    tap(&mut m, &mut r, Control::Right);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.settings.modifiers.get(0, Knob::PlaneCeiling), 100);
}

/// P98: the modifiers page under the ride's pause sets the ride's modifiers, which take when the
/// pause closes, and says so; a ride the shell holds them on (online, a played tape) shows them dimmed
/// and does not change them, and says why.
#[test]
fn the_pauses_modifiers_take_when_it_closes_unless_held() {
    use menu::Key;
    let (mut m, mut r) = fresh();
    m.pause();
    tap_until(&mut m, &mut r, Control::Down, |m| m.pause_rows()[m.cursor(Screen::Pause)] == "Settings");
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Settings));
    tab_to(&mut m, &mut r, "Ride");
    down_to(&mut m, &mut r, Key::Modifiers);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Modifiers));
    let note = |m: &Menu| m.page()[0].note.clone().unwrap_or_default();
    assert!(note(&m).contains("when the pause closes"), "{}", note(&m));
    down_to(&mut m, &mut r, Key::Knob(Knob::Gravity));
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.settings.modifiers.get(0, Knob::Gravity), 90);
    assert!(m.take_changed());
    // Held: the arrows, Cross and Reset to Original leave them be, and the page says why.
    let held = "Online the ride keeps its modifiers";
    m.mods_lock = Some(held.into());
    tap(&mut m, &mut r, Control::Left);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.settings.modifiers.get(0, Knob::Gravity), 90);
    assert_eq!(note(&m), held);
    let gravity = m.page().into_iter().find(|r| r.key == Key::Knob(Knob::Gravity)).unwrap();
    assert_eq!(gravity.off.as_deref(), Some(held));
    down_to(&mut m, &mut r, Key::ResetMods);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.settings.modifiers.get(0, Knob::Gravity), 90, "Reset to Original went through the hold");
    // Out to the ride: Back twice, then Resume.
    tap(&mut m, &mut r, Control::Circle);
    tap(&mut m, &mut r, Control::Circle);
    assert_eq!(m.screen(), Some(Screen::Pause));
    tap_until(&mut m, &mut r, Control::Up, |m| m.cursor(Screen::Pause) == 0);
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.riding());
    // The main menu's page is never held: what it sets is the next ride's.
    m.main();
    m.open(Screen::Modifiers);
    assert!(note(&m).contains("next"), "{}", note(&m));
    assert!(m.page().iter().all(|r| r.off.is_none() || matches!(r.key, Key::LoadPreset | Key::DeletePreset)), "{:?}", m.page());
}

/// P143: a modifier preset is one player's set. Saved, it keeps the Player row's player's own
/// modifiers and the world's; loaded, those go onto the Player row's player and the world, and no
/// other player's change; so 1P's preset loads onto 2P and 2P's onto 1P. A whole-table preset line
/// from before is its first player's set, and under the pause a loaded preset is what the ride takes
/// as the pause closes.
#[test]
fn a_modifier_preset_is_one_players_set() {
    use menu::Key;
    let (mut m, mut r) = fresh();
    settings_at(&mut m, &mut r, "Ride");
    down_to(&mut m, &mut r, Key::Modifiers);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Modifiers));
    assert_eq!(m.page()[0].value, "1P");
    let save = |m: &mut Menu, r: &mut Reader, name: &str| {
        down_to(m, r, Key::SavePreset);
        tap(m, r, Control::Cross);
        type_keys(m, r, name);
        tap(m, r, Control::Start);
        assert_eq!(m.screen(), Some(Screen::Modifiers));
    };
    let load = |m: &mut Menu, r: &mut Reader, name: &str| {
        down_to(m, r, Key::LoadPreset);
        tap_until(m, r, Control::Right, |m| m.page()[m.cursor(Screen::Modifiers)].value == name);
        tap(m, r, Control::Cross);
    };
    let step = |m: &mut Menu, r: &mut Reader, knob: Knob, c: Control, n: usize| {
        down_to(m, r, Key::Knob(knob));
        for _ in 0..n {
            tap(m, r, c);
        }
    };
    let player = |m: &mut Menu, r: &mut Reader, who: &str| {
        down_to(m, r, Key::Player);
        tap_until(m, r, Control::Right, |m| m.page()[0].value == who);
    };

    // 1P at 150% top speed and the world at 90% gravity, kept as Fast.
    step(&mut m, &mut r, Knob::BikeSpeed, Control::Right, 5);
    step(&mut m, &mut r, Knob::Gravity, Control::Left, 1);
    save(&mut m, &mut r, "fast");
    let fast = m.settings.presets.iter().find(|(n, _)| n == "Fast").unwrap().1;
    assert_eq!((fast.get(0, Knob::BikeSpeed), fast.get(5, Knob::BikeSpeed), fast.get(0, Knob::Gravity)), (150, 150, 90));
    // 1P changes since, and so does the world.
    step(&mut m, &mut r, Knob::BikeSpeed, Control::Left, 1);
    step(&mut m, &mut r, Knob::Gravity, Control::Left, 2);
    assert_eq!(m.settings.modifiers.get(0, Knob::Gravity), 70);

    // 2P loads it: 2P at 150%, 1P keeps its own 140%, 3P is untouched, the world back at 90%.
    player(&mut m, &mut r, "2P");
    load(&mut m, &mut r, "Fast");
    let mods = m.settings.modifiers;
    assert_eq!(mods.get(1, Knob::BikeSpeed), 150);
    assert_eq!(mods.get(0, Knob::BikeSpeed), 140, "1P's own change stays");
    assert_eq!(mods.get(2, Knob::BikeSpeed), 100, "3P was not the one loading it");
    assert_eq!(mods.get(0, Knob::Gravity), 90, "the world follows the preset");
    assert!(m.take_changed());

    // 2P's own set, saved: grip down, kept as Slip; 1P loads it.
    step(&mut m, &mut r, Knob::Grip, Control::Left, 3);
    save(&mut m, &mut r, "slip");
    player(&mut m, &mut r, "1P");
    load(&mut m, &mut r, "Slip");
    let mods = m.settings.modifiers;
    assert_eq!((mods.get(0, Knob::BikeSpeed), mods.get(0, Knob::Grip)), (150, 70), "1P rides 2P's set");
    assert_eq!((mods.get(1, Knob::BikeSpeed), mods.get(1, Knob::Grip)), (150, 70));
    assert_eq!(mods.get(2, Knob::Grip), 100);

    // The file writes a preset as its one set, and reads it back so.
    let text = m.settings.text();
    let line = text.lines().find(|l| l.starts_with("preset = Fast")).unwrap();
    assert!(!line.contains('.'), "one player's set has no player numbers: {line}");
    assert_eq!(Settings::parse(&text).presets, m.settings.presets);

    // A preset line from before P143 is the whole table: it reads as its seat 0.
    let old = Settings::parse("preset = Old | bike_speed.1=150 bike_speed.2=300 grip.2=50 gravity=50\n");
    let (name, set) = &old.presets[0];
    assert_eq!(name, "Old");
    assert_eq!((set.get(1, Knob::BikeSpeed), set.get(1, Knob::Grip), set.get(0, Knob::Gravity)), (150, 100, 50));
    let mut onto = Mods::original();
    onto.set(0, Knob::BikeSpeed, 200);
    menu::load_preset_onto(&mut onto, set, 0);
    assert_eq!((onto.get(0, Knob::BikeSpeed), onto.get(1, Knob::BikeSpeed)), (150, 100), "it loads onto 1P as it did");

    // Under the pause, 2P loads Fast over a set of 1P's alone, and the ride takes it as the pause closes.
    let (mut m, mut r) = fresh();
    m.settings.presets = vec![("Fast".into(), fast)];
    m.pause();
    tap_until(&mut m, &mut r, Control::Down, |m| m.pause_rows()[m.cursor(Screen::Pause)] == "Settings");
    tap(&mut m, &mut r, Control::Cross);
    tab_to(&mut m, &mut r, "Ride");
    down_to(&mut m, &mut r, Key::Modifiers);
    tap(&mut m, &mut r, Control::Cross);
    player(&mut m, &mut r, "2P");
    load(&mut m, &mut r, "Fast");
    assert!(m.take_changed());
    tap(&mut m, &mut r, Control::Circle);
    tap(&mut m, &mut r, Control::Circle);
    assert_eq!(m.screen(), Some(Screen::Pause));
    tap_until(&mut m, &mut r, Control::Up, |m| m.cursor(Screen::Pause) == 0);
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.riding());
    let mods = m.settings.modifiers;
    assert_eq!((mods.get(0, Knob::BikeSpeed), mods.get(1, Knob::BikeSpeed), mods.get(0, Knob::Gravity)), (100, 150, 90));
}

/// P134: the host's Game Modifiers page has All Players at the head of its Player row: a knob set
/// there is every seat's, the page says Mixed where the seats differ, and the knobs set so are the
/// ones the shell sends to every seat online. The pause has its own Game Modifiers row.
#[test]
fn the_hosts_all_players_sets_every_seats_knob() {
    use menu::Key;
    let (mut m, mut r) = fresh();
    m.pause();
    tap_until(&mut m, &mut r, Control::Down, |m| m.pause_rows()[m.cursor(Screen::Pause)] == "Game Modifiers");
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Modifiers));
    assert_eq!(m.page()[0].value, "1P");
    assert_eq!(m.page()[0].choices[0], "All Players");
    // Left from 1P is All Players.
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.page()[0].value, "All Players");
    assert_eq!(m.mods_whose(), "All");
    down_to(&mut m, &mut r, Key::Knob(Knob::BikeSpeed));
    tap(&mut m, &mut r, Control::Right);
    assert!((0..8).all(|s| m.settings.modifiers.get(s, Knob::BikeSpeed) == 110), "{:?}", m.settings.modifiers);
    // and the world's, the host's to set
    down_to(&mut m, &mut r, Key::Knob(Knob::Gravity));
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.settings.modifiers.get(0, Knob::Gravity), 90);
    assert_eq!(m.take_mods_all(), vec![Knob::BikeSpeed], "the world's is one for everyone already");
    assert!(m.take_mods_all().is_empty(), "taken once");

    // 2P alone faster: All Players then says Mixed, and its arrow puts every seat on one again.
    down_to(&mut m, &mut r, Key::Player);
    tap(&mut m, &mut r, Control::Right);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.page()[0].value, "2P");
    down_to(&mut m, &mut r, Key::Knob(Knob::BikeSpeed));
    tap(&mut m, &mut r, Control::Right);
    assert_eq!((m.settings.modifiers.get(0, Knob::BikeSpeed), m.settings.modifiers.get(1, Knob::BikeSpeed)), (110, 120));
    assert!(m.take_mods_all().is_empty(), "a seat's own is not All's");
    down_to(&mut m, &mut r, Key::Player);
    tap_until(&mut m, &mut r, Control::Left, |m| m.page()[0].value == "All Players");
    let speed = |m: &Menu| m.page().into_iter().find(|r| r.key == Key::Knob(Knob::BikeSpeed)).unwrap();
    assert_eq!(speed(&m).value, "Mixed");
    down_to(&mut m, &mut r, Key::Knob(Knob::BikeSpeed));
    tap(&mut m, &mut r, Control::Right);
    assert!((0..8).all(|s| m.settings.modifiers.get(s, Knob::BikeSpeed) == 120), "{:?}", m.settings.modifiers);
    assert_ne!(speed(&m).value, "Mixed");

    // Reset to Original on All Players is every seat's.
    down_to(&mut m, &mut r, Key::ResetMods);
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.settings.modifiers.is_original());
}

/// P134: a player who is not the host opens Game Modifiers from the pause their own pad opened and
/// sets their own seat's knobs alone: their Player row is theirs and stays so, and the world's are
/// shown but not theirs to change. An online guest's page is their machine's 1P's, the same way.
#[test]
fn a_guests_page_changes_only_its_own_seat_and_not_the_world() {
    use menu::Key;
    let (mut m, mut r) = fresh();
    m.riders = 2;
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.riding());
    let mut garages = vec![garage(0, Kind::Bike), garage(1, Kind::Bike)];
    let mut readers = vec![r, Reader::new()];
    ride_frame(&mut m, &mut readers, &[Report::idle(), Report::idle()], &mut garages);
    // 2P's Start, down 2P's pad to Game Modifiers.
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Start);
    assert_eq!(m.view(1).screen(), Some(Screen::Pause), "2P's own pause (P84)");
    for _ in 0..12 {
        let v = m.view(1);
        if v.pause_rows()[v.cursor(Screen::Pause)] == "Game Modifiers" {
            break;
        }
        tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Down);
    }
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Cross);
    assert_eq!(m.view(1).screen(), Some(Screen::Modifiers));
    let row = |m: &Menu, key: Key| m.page().into_iter().find(|r| r.key == key).unwrap_or_else(|| panic!("{key:?} not in {:?}", m.page().iter().map(|r| (r.key, r.value.clone())).collect::<Vec<_>>()));
    assert_eq!(row(&m.view(1), Key::Player).value, "2P");
    assert!(row(&m.view(1), Key::Player).off.is_some(), "only the host picks another player");
    assert!(!row(&m.view(1), Key::Player).choices.iter().any(|c| c == "All Players"));
    assert!(row(&m.view(1), Key::Knob(Knob::Gravity)).off.is_some(), "the world's is the host's");
    assert!(row(&m.view(1), Key::Knob(Knob::BikeSpeed)).off.is_none());
    let to = |m: &mut Menu, readers: &mut [Reader], garages: &mut [Garage], key: Key| {
        for _ in 0..40 {
            let v = m.view(1);
            if v.page()[v.cursor(Screen::Modifiers)].key == key {
                return;
            }
            tap_seat(m, readers, garages, 1, Control::Down);
        }
        panic!("no {key:?} row under 2P's pad");
    };
    to(&mut m, &mut readers, &mut garages, Key::Knob(Knob::BikeSpeed));
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Right);
    assert_eq!(m.settings.modifiers.get(1, Knob::BikeSpeed), 110);
    assert_eq!(m.settings.modifiers.get(0, Knob::BikeSpeed), 100, "1P's own is 1P's");
    to(&mut m, &mut readers, &mut garages, Key::Knob(Knob::Gravity));
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Left);
    assert_eq!(m.settings.modifiers.get(0, Knob::Gravity), 100, "2P moved the world");
    to(&mut m, &mut readers, &mut garages, Key::Player);
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Left);
    assert_eq!(row(&m.view(1), Key::Player).value, "2P");
    // Reset to Original puts back 2P's alone.
    m.settings.modifiers.set(0, Knob::Gravity, 50);
    to(&mut m, &mut readers, &mut garages, Key::ResetMods);
    tap_seat(&mut m, &mut readers, &mut garages, 1, Control::Cross);
    assert_eq!((m.settings.modifiers.get(1, Knob::BikeSpeed), m.settings.modifiers.get(0, Knob::Gravity)), (100, 50));

    // An online guest: its machine's 1P, the world's the host's, and none of it All's.
    let (mut m, mut r) = fresh();
    m.guest = true;
    m.pause();
    tap_until(&mut m, &mut r, Control::Down, |m| m.pause_rows()[m.cursor(Screen::Pause)] == "Game Modifiers");
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.page()[0].value, "1P");
    down_to(&mut m, &mut r, Key::Knob(Knob::Gravity));
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.settings.modifiers.get(0, Knob::Gravity), 100);
    assert!(row(&m, Key::Knob(Knob::PlaneTopSpeed)).off.is_some(), "online every seat flies the host's biplane");
    down_to(&mut m, &mut r, Key::Knob(Knob::Grip));
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.modifiers.get(0, Knob::Grip), 110);
    assert!(m.take_mods_all().is_empty());
}

/// P138: the Air Wrecks row, under Wrecks, turns the airborne wrecks off and on, is written to the
/// file only when off and read back after a restart; it is dimmed while Wrecks are off, held in the
/// pause of a ride the shell keeps it on, and Original mode puts it back on. Since P175 both rows are
/// the Game Modifiers page's, the world's, and no longer the Ride tab's.
#[test]
fn the_air_wrecks_row_toggles_and_survives_a_restart() {
    use menu::Key;
    let (mut m, mut r) = fresh();
    assert!(m.settings.air_wrecks, "on is the default, the original's");
    settings_at(&mut m, &mut r, "Ride");
    assert!(!m.page().iter().any(|r| matches!(r.key, Key::Wrecks | Key::AirWrecks)), "the Ride tab keeps no wrecks rows");
    down_to(&mut m, &mut r, Key::Modifiers);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Modifiers));
    down_to(&mut m, &mut r, Key::AirWrecks);
    let row = |m: &Menu| m.page().into_iter().find(|r| r.key == Key::AirWrecks).unwrap();
    let keys: Vec<Key> = m.page().iter().map(|r| r.key).collect();
    let at = keys.iter().position(|k| *k == Key::AirWrecks).unwrap();
    assert_eq!(keys[at - 1], Key::Wrecks, "Air Wrecks sits under Wrecks: {keys:?}");
    assert_eq!(row(&m).value, "On");
    assert!(row(&m).world, "one Air Wrecks for every player");
    tap(&mut m, &mut r, Control::Right);
    assert!(!m.settings.air_wrecks);
    assert_eq!(row(&m).value, "Off");
    assert!(m.take_changed());
    assert!(m.settings.text().contains("air_wrecks = off\n"));
    assert_ne!(m.original(), menu::Original::On, "Air Wrecks off is not the original");

    // Saved, read back after a restart, and the menu opened on it shows it off.
    let dir = scratch("air-wrecks");
    menu::save(&dir, &m.settings, &m.controls).unwrap();
    let (settings, controls) = menu::load(&dir);
    assert!(!settings.air_wrecks);
    let again = Menu::new(settings, controls);
    assert_eq!(again.settings, m.settings);
    // And Cross turns it back on, which the file no longer mentions.
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.settings.air_wrecks);
    assert!(!m.settings.text().contains("air_wrecks"), "{}", m.settings.text());
    std::fs::remove_dir_all(dir).ok();

    // Dimmed while Wrecks are off: there is nothing in the air to judge.
    m.settings.wrecks = false;
    assert_eq!(row(&m).off.as_deref(), Some("Wrecks are off"));
    m.settings.wrecks = true;
    assert_eq!(row(&m).off, None);

    // In the pause it takes when the pause closes, and a held ride says why and leaves it be.
    tap(&mut m, &mut r, Control::Circle);
    m.pause();
    tap_until(&mut m, &mut r, Control::Down, |m| m.pause_rows()[m.cursor(Screen::Pause)] == "Settings");
    tap(&mut m, &mut r, Control::Cross);
    tab_to(&mut m, &mut r, "Ride");
    down_to(&mut m, &mut r, Key::Modifiers);
    tap(&mut m, &mut r, Control::Cross);
    down_to(&mut m, &mut r, Key::AirWrecks);
    assert!(row(&m).note.unwrap_or_default().contains("when the pause closes"));
    let held = "Online the room's Air Wrecks holds for the ride";
    m.air_lock = Some(held.into());
    assert_eq!(row(&m).off.as_deref(), Some(held));
    tap(&mut m, &mut r, Control::Right);
    assert!(m.settings.air_wrecks, "a held row changed");
    m.air_lock = None;
    tap(&mut m, &mut r, Control::Right);
    assert!(!m.settings.air_wrecks);

    // Original mode on puts it back on.
    m.set_original(true);
    assert!(m.settings.air_wrecks);
    assert_eq!(m.original(), menu::Original::On);
}

/// P175: Wrecks and Air Wrecks on the Game Modifiers page count as its changes on the Ride tab's
/// row, are the host's alone, go back on with the host's All Players Reset to Original (not with one
/// player's), stay out of saved presets, and settings.txt's old words still read.
#[test]
fn wrecks_are_game_modifiers() {
    use menu::Key;
    let (mut m, mut r) = fresh();
    settings_at(&mut m, &mut r, "Ride");
    let count = |m: &Menu| m.page().into_iter().find(|r| r.key == Key::Modifiers).unwrap().value;
    m.settings.wrecks = false;
    assert_eq!(count(&m), "1 Changed");
    m.settings.air_wrecks = false;
    assert_eq!(count(&m), "2 Changed");
    down_to(&mut m, &mut r, Key::Modifiers);
    tap(&mut m, &mut r, Control::Cross);
    let row = |m: &Menu, k: Key| m.page().into_iter().find(|r| r.key == k).unwrap();
    let keys: Vec<Key> = m.page().iter().map(|r| r.key).collect();
    let at = keys.iter().position(|k| *k == Key::Wrecks).unwrap();
    assert_eq!(keys[at + 1], Key::AirWrecks, "{keys:?}");
    assert!(matches!(keys[at - 2], Key::Knob(_)) && keys[at - 1] == Key::Heading, "after the modifiers, under a heading: {keys:?}");
    assert!(row(&m, Key::Wrecks).world);

    // A preset saved now and loaded later leaves the wrecks as they are.
    let saved = m.settings.modifiers;
    m.settings.wrecks = true;
    m.settings.air_wrecks = true;
    m.settings.presets.push(("p".into(), saved));
    down_to(&mut m, &mut r, Key::LoadPreset);
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.settings.wrecks && m.settings.air_wrecks, "presets do not carry the wrecks");

    // One player's reset leaves the world's wrecks; All Players' puts them back on.
    m.settings.wrecks = false;
    m.settings.air_wrecks = false;
    down_to(&mut m, &mut r, Key::Player);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.mods_whose(), "2P");
    down_to(&mut m, &mut r, Key::ResetMods);
    tap(&mut m, &mut r, Control::Cross);
    assert!(!m.settings.wrecks && !m.settings.air_wrecks, "2P's reset moved the world's wrecks");
    down_to(&mut m, &mut r, Key::Player);
    tap_until(&mut m, &mut r, Control::Left, |m| m.mods_whose() == "All");
    down_to(&mut m, &mut r, Key::ResetMods);
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.settings.wrecks && m.settings.air_wrecks);

    // Not the host's page (an online guest's pause): the rows are held.
    m.guest = true;
    m.pause();
    tap_until(&mut m, &mut r, Control::Down, |m| m.pause_rows()[m.cursor(Screen::Pause)] == "Settings");
    tap(&mut m, &mut r, Control::Cross);
    tab_to(&mut m, &mut r, "Ride");
    down_to(&mut m, &mut r, Key::Modifiers);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.mods_own(), Some(0));
    assert_eq!(row(&m, Key::Wrecks).off.as_deref(), Some("The world's: only the host sets it."));
    assert_eq!(row(&m, Key::AirWrecks).off.as_deref(), Some("The world's: only the host sets it."));

    // settings.txt's words are the old ones, and a file from before reads.
    let old = Settings::parse("wrecks = off\nair_wrecks = off\n");
    assert!(!old.wrecks && !old.air_wrecks);
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
    for report in [Report::from_buttons(control.mask()), Report::idle(), Report::idle(), Report::idle()] {
        m.music_now = speakers.now();
        m.music_held = speakers.held();
        m.step(r.read(&report));
        hear_tunes(m, speakers, dir);
        hum_listen(speakers);
    }
}

/// `Front::hear` after a frame that changed the settings, and the save.
fn hear_tunes(m: &mut Menu, speakers: &audio::Speakers, dir: &std::path::Path) {
    if m.changed() {
        speakers.apply(&m.settings.music, levels(&m.settings));
        for tune in m.take_tunes() {
            match tune {
                menu::Tune::Play(id) => speakers.play_now(&id),
                menu::Tune::Skip => speakers.skip(),
                menu::Tune::Back => speakers.back(),
                menu::Tune::Hold(on) => speakers.hold(on),
            }
        }
        if m.take_changed() {
            menu::save(dir, &m.settings, &m.controls).unwrap();
        }
    }
}

#[test]
fn the_pauses_player_goes_back_holds_and_skips_by_pad_and_click() {
    use menu::{Deck, Key};
    let dir = scratch("p174");
    let (mut m, mut r) = fresh();
    let speakers = hum_speakers(&m.settings);
    m.music_choices = speakers.choices();
    let tap = |m: &mut Menu, r: &mut Reader, c: Control| tap_heard(m, r, &speakers, &dir, c);
    let size = (1280, 720);
    // A click as the shell gives one, the music told to the menu first and heard after.
    let click = |m: &mut Menu, at: [f32; 2]| {
        m.music_now = speakers.now();
        m.music_held = speakers.held();
        let hit = m.click(at, size);
        hear_tunes(m, &speakers, &dir);
        hum_listen(&speakers);
        hit
    };
    let loud = || hum_listen(&speakers)[100];
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Start);
    assert_eq!(m.screen(), Some(Screen::Pause));
    assert!(m.player_shown());
    let first = speakers.now().expect("a song under Shuffle");

    // R1 skips, L1 goes back to the song before, the pause still up.
    tap(&mut m, &mut r, Control::R1);
    let second = speakers.now().unwrap();
    assert_ne!(second, first, "R1 is the next song");
    tap(&mut m, &mut r, Control::L1);
    assert_eq!(speakers.now(), Some(first.clone()), "L1 is the song before");
    assert_eq!(m.screen(), Some(Screen::Pause));
    assert!(m.paused(0));

    // Square holds it, silent where it is, and plays it on.
    tap(&mut m, &mut r, Control::Square);
    assert!(speakers.held() && m.music_held);
    assert_eq!(loud(), 0, "held is silent");
    assert_eq!(speakers.now(), Some(first.clone()), "and the same song");
    tap(&mut m, &mut r, Control::Square);
    assert!(!speakers.held());
    assert!(loud() > 500);

    // A click on a control is its press, at once.
    assert!(click(&mut m, menu::deck_point(Deck::Skip, size)));
    let third = speakers.now().unwrap();
    assert_ne!(third, first);
    assert!(click(&mut m, menu::deck_point(Deck::Back, size)));
    assert_eq!(speakers.now(), Some(first.clone()));
    assert!(click(&mut m, menu::deck_point(Deck::Hold, size)));
    assert!(speakers.held());
    assert!(!click(&mut m, [10.0, 10.0]), "nowhere near the player");
    // A skip from held plays the next song.
    tap(&mut m, &mut r, Control::R1);
    assert!(!speakers.held() && !m.music_held);
    assert!(loud() > 500);
    assert_eq!(m.screen(), Some(Screen::Pause));

    // Settings > Audio has the same two beside Skip Song.
    m.open(Screen::Audio);
    let rows = m.page();
    let label = |k: Key| rows.iter().find(|r| r.key == k).map(|r| r.label.clone());
    assert_eq!(label(Key::BackSong).as_deref(), Some("Previous Song"));
    assert_eq!(label(Key::HoldMusic).as_deref(), Some("Pause Music"));

    // With the music off the player does nothing, a click on it refused.
    let (mut m, mut r) = fresh();
    m.settings.music = "off".into();
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Start);
    tap(&mut m, &mut r, Control::R1);
    assert!(m.take_tunes().is_empty());
    assert_eq!(m.player_off(), Some("The music is off"));
    assert!(m.click(menu::deck_point(Deck::Hold, size), size));
    assert!(m.take_tunes().is_empty() && !m.music_held);
    // Nor is it there under the pause's confirm.
    let (mut m, mut r) = fresh();
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Start);
    while m.pause_rows()[m.cursor(Screen::Pause)] != "Main Menu" {
        tap(&mut m, &mut r, Control::Down);
    }
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.confirming().is_some());
    assert!(!m.player_shown());
    assert!(!m.click(menu::deck_point(Deck::Skip, size), size));
}

#[test]
fn the_pause_picks_a_song_skips_it_and_its_music_slider_is_kept() {
    use menu::Key;
    let dir = scratch("p56");
    let (mut m, mut r) = fresh();
    let speakers = hum_speakers(&m.settings);
    m.music_choices = speakers.choices();
    let tap = |m: &mut Menu, r: &mut Reader, c: Control| tap_heard(m, r, &speakers, &dir, c);
    // Down or up the Settings page to the row that is `key`, every press heard.
    let to = |m: &mut Menu, r: &mut Reader, key: Key| {
        for _ in 0..32 {
            if m.page()[m.cursor(Screen::Settings)].key == key {
                return;
            }
            tap(m, r, Control::Down);
        }
        panic!("no {key:?} row");
    };
    // A ride, and its pause.
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.riding());
    tap(&mut m, &mut r, Control::Start);
    assert_eq!(m.screen(), Some(Screen::Pause));
    assert_eq!(m.pause_rows()[3], "Audio");
    for _ in 0..3 {
        tap(&mut m, &mut r, Control::Down);
    }
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Settings));
    assert_eq!(m.tabs(Screen::Settings)[m.tab(Screen::Settings)], "Audio", "the pause's Audio row is Settings' Audio tab (P118)");
    // The shuffle the defaults ask for is playing, and the Song row names it, dimmed until One Song.
    let first = speakers.now().expect("a song under Shuffle");
    let label = |m: &Menu, id: &str| m.music_choices.iter().find(|c| c.0 == id).unwrap().1.clone();
    let row = |m: &Menu, key: Key| m.page().into_iter().find(|r| r.key == key).unwrap();
    assert_eq!(row(&m, Key::Song).value, label(&m, &first));
    assert!(row(&m, Key::Song).off.is_some());
    assert_eq!(row(&m, Key::SkipSong).value, label(&m, &first), "Skip Song says what it skips");
    let values: Vec<String> = m.page()[..7].iter().map(|r| r.value.clone()).collect();
    assert_eq!(values, ["Game Mix", "70", "80", "60", "45", "100", "70"]);

    let ids: Vec<String> = m.music_choices.iter().map(|c| c.0.clone()).collect();
    let after = |id: &str| ids[(ids.iter().position(|i| i == id).unwrap() + 1) % ids.len()].clone();
    let level = |id: &str| match id {
        "hum:one" => 1000.0,
        "hum:two" => 2000.0,
        _ => 3000.0,
    };
    // In Order keeps the song; Skip Song is the one after it.
    to(&mut m, &mut r, Key::MusicPlays);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.music, "order");
    assert_eq!(speakers.now().as_deref(), Some(first.as_str()), "the order changing keeps the song");
    to(&mut m, &mut r, Key::SkipSong);
    tap(&mut m, &mut r, Control::Cross);
    let next = after(&first);
    assert_eq!(speakers.now(), Some(next.clone()), "skip moves to the next");
    // One Song repeats the one playing; the Song row's right is the next, which plays.
    to(&mut m, &mut r, Key::MusicPlays);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.music, next, "One Song is the song playing");
    assert_eq!(speakers.now(), Some(next.clone()));
    to(&mut m, &mut r, Key::Song);
    assert!(row(&m, Key::Song).off.is_none());
    tap(&mut m, &mut r, Control::Right);
    let picked = after(&next);
    assert_eq!(speakers.now(), Some(picked.clone()), "the song picked plays");
    let heard = hum_listen(&speakers);
    assert!((heard[100] as f32 - level(&picked) * 0.7).abs() <= 2.0, "it is heard at the music's 70: {}", heard[100]);

    // The music slider, down a step: the mixer's music gain with it, while paused, and the mix is
    // the user's own.
    to(&mut m, &mut r, Key::Level(0));
    tap(&mut m, &mut r, Control::Left);
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.settings.music_volume, 60);
    assert_eq!(m.settings.mix, menu::Mix::Custom);
    assert_eq!(speakers.mixer().lock().unwrap().music_gain(), 0.6);
    let heard = hum_listen(&speakers);
    assert!((heard[100] as f32 - level(&picked) * 0.6).abs() <= 2.0, "{}", heard[100]);
    // Back is the pause, the ride still paused under it.
    tap(&mut m, &mut r, Control::Circle);
    assert_eq!(m.screen(), Some(Screen::Pause));

    // A restart: the settings read back from where the shell saved them, into a new sound.
    let (settings, controls) = menu::load(&dir);
    assert_eq!((settings.music.as_str(), settings.music_volume, settings.mix), (picked.as_str(), 60, menu::Mix::Custom));
    let again = hum_speakers(&settings);
    let mut m = Menu::new(settings, controls);
    m.music_choices = again.choices();
    assert_eq!(again.mixer().lock().unwrap().music_gain(), 0.6);
    m.open(Screen::Audio);
    assert_eq!(m.page()[m.cursor(Screen::Settings)].key, Key::Mix);
    assert_eq!((m.page()[0].value.as_str(), m.page()[1].value.as_str()), ("Custom Mix", "60"));

    // The presets set every slider, and Custom brings the user's back.
    let (mut m, mut r) = (m, Reader::new());
    m.step(r.read(&Report::idle()));
    let tap = |m: &mut Menu, r: &mut Reader, c: Control| tap_heard(m, r, &again, &dir, c);
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

/// P81: the Controls tab is one choice a seat for the bike and one for the plane's stick. A fresh
/// file is on New and the stick that never turns round; there is no throttle row; the Player row is
/// there only with more than one seat; both rows step by the pad and are saved; and the plane row's
/// hint says what the choice it is on does.
#[test]
fn the_controls_tab_is_one_choice_a_seat_and_a_plane_stick() {
    use menu::Key;
    let (mut m, mut r) = fresh();
    settings_at(&mut m, &mut r, "Controls");
    let row = |m: &Menu, k: Key| m.page().into_iter().find(|r| r.key == k).unwrap();
    let keys: Vec<Key> = m.page().iter().map(|r| r.key).collect();
    assert_eq!(keys, [Key::BikeLayout, Key::PlaneLayout, Key::Markers, Key::Nintendo, Key::Controller, Key::Pressure], "one seat: no Player row");
    assert!(!m.page().iter().any(|r| r.label.contains("Throttle")), "no throttle row");
    assert_eq!(row(&m, Key::BikeLayout).value, "New");
    let plane = row(&m, Key::PlaneLayout);
    assert_eq!((plane.label.as_str(), plane.value.as_str()), ("Plane Stick", "Normal"));
    assert_eq!(m.controls.planes, [Layout::New; menu::SEATS], "the plane-relative stick is the default");
    let normal = plane.about.clone();
    assert!(normal.contains("relative to the plane"), "{normal}");

    // Both rows by the pad.
    down_to(&mut m, &mut r, Key::BikeLayout);
    tap(&mut m, &mut r, Control::Right);
    down_to(&mut m, &mut r, Key::PlaneLayout);
    tap(&mut m, &mut r, Control::Right);
    let plane = row(&m, Key::PlaneLayout);
    assert_eq!((row(&m, Key::BikeLayout).value.as_str(), plane.value.as_str()), ("Original", "Like the Original"));
    assert_ne!(plane.about, normal, "the hint follows the value");
    assert!(plane.about.starts_with("Like the original"), "{}", plane.about);
    assert!(m.take_changed());
    let dir = scratch("p81");
    menu::save(&dir, &m.settings, &m.controls).unwrap();
    let (_, controls) = menu::load(&dir);
    assert_eq!((controls.layouts[0], controls.planes[0]), (Layout::Original, Layout::Original));
    assert_eq!(controls.throttle, Gas::Cross, "the throttle line is still written, untouched");
    std::fs::remove_dir_all(dir).ok();
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(row(&m, Key::PlaneLayout).about, normal);

    // Two seats: the Player row comes first, and the rows are the seat it is on.
    m.seated = 2;
    assert_eq!(m.page()[0].key, Key::Player);
    down_to(&mut m, &mut r, Key::Player);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(row(&m, Key::BikeLayout).value, "New", "2P's own");
    down_to(&mut m, &mut r, Key::PlaneLayout);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!((m.controls.planes[0], m.controls.planes[1]), (Layout::New, Layout::Original));
}

/// P79, P81: the Controls tab offers the original's controller presets inside Original, on the one
/// choice a seat has, and the file keeps the pick.
#[test]
fn the_controls_row_offers_the_originals_presets() {
    use menu::Key;
    let (mut m, mut r) = fresh();
    m.seated = 2;
    settings_at(&mut m, &mut r, "Controls");
    let (n, o) = (Layout::New, Layout::Original);
    let bike = |m: &Menu| m.page().into_iter().find(|r| r.key == Key::BikeLayout).unwrap();
    assert_eq!(bike(&m).choices, ["New", "Original", "Original, Preset 2", "Original, Preset 3", "Original, Preset 4"]);
    assert_eq!(bike(&m).tips.len(), 5, "a line under every choice");
    down_to(&mut m, &mut r, Key::BikeLayout);
    let mut seen = vec![(m.controls.layouts[0], m.controls.presets[0])];
    for _ in 0..5 {
        tap(&mut m, &mut r, Control::Right);
        seen.push((m.controls.layouts[0], m.controls.presets[0]));
    }
    assert_eq!(seen, [(n, 0), (o, 0), (o, 1), (o, 2), (o, 3), (n, 0)], "on round to the start");
    tap(&mut m, &mut r, Control::Left);
    assert_eq!((m.controls.layouts[0], m.controls.presets[0]), (o, 3));
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(bike(&m).value, "Original, Preset 3");
    // The biplane's stick is a row of its own.
    down_to(&mut m, &mut r, Key::PlaneLayout);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!((m.controls.layouts[0], m.controls.presets[0], m.controls.planes[0]), (o, 2, o));
    assert_eq!(m.controls.presets[1..], [0; 7], "2P's preset moved with 1P's");
    // 2P is still on New.
    down_to(&mut m, &mut r, Key::Player);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(bike(&m).value, "New");
    assert!(m.take_changed());
    assert_eq!(menu::controls_label(o, 2), "Original, Preset 3");
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
    // Where the pad diagram keeps each one's keyboard list.
    assert_eq!(menu::keyboard_at(n, 2), 0);
    assert_eq!((0..4).map(|p| menu::keyboard_at(o, p)).collect::<Vec<_>>(), [1, 2, 3, 4]);
}

/// Left and Right step the pick's categories: the disc's towns, the worlds right after them, then
/// each pack in its own; up and down wrap inside the lit one, and a pack turned off leaves with its
/// category (user, 2026-10-23).
#[test]
fn left_and_right_step_the_picks_categories_with_the_worlds_first() {
    let (mut m, mut r) = fresh();
    m.set_mod_tracks(vec![
        ("mxvatv_nat01".into(), "Nat 01".into()),
        ("mxvatv_nat02".into(), "Nat 02".into()),
        ("untamed_nat_az".into(), "Arizona".into()),
        ("world_mxu".into(), "MX Unleashed: every map".into()),
        ("world_ultra".into(), "Ultra".into()),
    ]);
    let names: Vec<String> = m.track_groups().into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, ["All Games", "MX Unleashed", "Worlds", "MX vs. ATV Unleashed", "MX vs. ATV Untamed"]);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Tracks));
    assert_eq!(m.track_filter_group(), 0, "All Games unless the settings say");
    // Two presses to the worlds (MD11's All Games first).
    tap(&mut m, &mut r, Control::Right);
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.track_at(m.cursor(Screen::Tracks)), ("world_mxu", "MX Unleashed: every map"));
    tap(&mut m, &mut r, Control::Down);
    assert_eq!(m.track_at(m.cursor(Screen::Tracks)).0, "world_ultra");
    tap(&mut m, &mut r, Control::Down);
    assert_eq!(m.track_at(m.cursor(Screen::Tracks)).0, "world_mxu", "down wraps inside the worlds");
    tap(&mut m, &mut r, Control::Up);
    assert_eq!(m.track_at(m.cursor(Screen::Tracks)).0, "world_ultra");
    // Left from the first category wraps to the last.
    for _ in 0..3 {
        tap(&mut m, &mut r, Control::Left);
    }
    assert_eq!(m.track_at(m.cursor(Screen::Tracks)).0, "untamed_nat_az");
    for _ in 0..3 {
        tap(&mut m, &mut r, Control::Right);
    }
    assert_eq!(m.track_filter_group(), 2);
    let acts = tap(&mut m, &mut r, Control::Cross);
    assert!(acts.contains(&Act::Ride("world_mxu".into())), "{acts:?}");
    // Back on the pick it opens on the world, and a category comes back to the ride's track.
    let mut next = Menu::new(Settings::parse(&m.settings.text()), Controls::default());
    next.set_mod_tracks(m.mod_tracks.clone());
    assert_eq!(next.track_at(next.cursor(Screen::Tracks)).0, "world_mxu");
    // A pack switched off takes its category with it.
    m.settings.mods_off = vec!["world".into()];
    let names: Vec<String> = m.track_groups().into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, ["All Games", "MX Unleashed", "MX vs. ATV Unleashed", "MX vs. ATV Untamed"]);
}

/// P149: every free-ride map spliced into one (P71) is the pick's row after the five maps, in the
/// disc's own filter: Cross on it rides `free_all`, the settings open on it next time, the loading
/// screen names it with its picture, and a mod still comes after it. Online keeps to the towns.
#[test]
fn the_pick_lists_the_spliced_maps_after_the_five_and_rides_them() {
    let (mut m, mut r) = fresh();
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Tracks));
    assert_eq!(m.cursor(Screen::Tracks), 0);
    for _ in 0..menu::TRACKS.len() {
        tap(&mut m, &mut r, Control::Down);
    }
    assert_eq!(m.cursor(Screen::Tracks), menu::TRACKS.len());
    assert_eq!(m.track_at(m.cursor(Screen::Tracks)), ("free_all", "Every free ride"));
    assert_eq!(m.track_game("free_all"), menu::DISC_GAME);
    let acts = tap(&mut m, &mut r, Control::Cross);
    assert!(acts.contains(&Act::Ride("free_all".into())), "{acts:?}");
    assert_eq!(m.settings.track, "free_all");
    m.start_loading("free_all");
    let l = m.loading.as_ref().expect("the loading screen");
    assert_eq!((l.name.as_str(), l.pack.as_str(), l.still), ("Every free ride", "MX Unleashed", menu::TRACKS.len()));
    // Kept: the next start's pick is on it.
    let again = Settings::parse(&m.settings.text());
    assert_eq!(again.track, "free_all");
    let mut next = Menu::new(again, Controls::default());
    assert_eq!(next.cursor(Screen::Tracks), menu::TRACKS.len());
    assert_eq!(next.online_tracks(), menu::TRACKS.map(|t| t.0), "the splice is not ridden online");
    // With a mod, the mod is after it and the splice stays in MX Unleashed's filter.
    next.set_mod_tracks(vec![("md2_flat".into(), "MD2 Flat".into())]);
    assert_eq!(next.track_at(menu::DISC_PICK), ("md2_flat", "MD2 Flat"));
    let groups = next.track_groups();
    let disc = groups.iter().find(|(n, _)| n == menu::DISC_GAME).expect("the disc's filter");
    assert_eq!(disc.1, 0..menu::DISC_PICK);
    assert_eq!(next.cursor(Screen::Tracks), menu::TRACKS.len());
}

/// MD2: the track pick lists the mods folder's tracks after the disc's, a mod rides by its id, and
/// the Mods page turns the folder, or one pack of it, off and is saved. With no mods the pick is the
/// disc's five and their splice (P149).
#[test]
fn the_pick_lists_mod_tracks_after_the_discs_and_the_mods_row_is_saved() {
    use menu::Key;
    let (mut m, mut r) = fresh();
    assert_eq!(m.track_count(), menu::DISC_PICK);
    for (i, t) in menu::TRACKS.iter().enumerate() {
        assert_eq!(m.track_at(i), *t);
    }
    assert_eq!(m.track_at(menu::TRACKS.len()), menu::SPLICED);
    assert!(m.settings.mods, "the folder is read unless asked");
    assert!(!m.settings.text().contains("mods"), "a file without mods says nothing of them");

    m.set_mod_tracks(vec![("md2_flat".into(), "MD2 Flat".into())]);
    assert_eq!(m.track_count(), menu::DISC_PICK + 1);
    assert_eq!(m.track_at(menu::DISC_PICK), ("md2_flat", "MD2 Flat"));
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Tracks));
    tap(&mut m, &mut r, Control::Up);
    assert_eq!(m.cursor(Screen::Tracks), menu::DISC_PICK, "up from the first wraps inside All Games");
    tap(&mut m, &mut r, Control::Right);
    tap(&mut m, &mut r, Control::Up);
    assert_eq!(m.cursor(Screen::Tracks), menu::DISC_PICK - 1, "up from the first wraps inside the disc's own, to the splice");
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.cursor(Screen::Tracks), menu::DISC_PICK, "right is the mods' game");
    let acts = tap(&mut m, &mut r, Control::Cross);
    assert!(acts.contains(&Act::Ride("md2_flat".into())), "{acts:?}");
    assert_eq!(m.settings.track, "md2_flat");
    // The mod is the track the file opens on next time, while the folder still has it.
    let again = Settings::parse(&m.settings.text());
    assert_eq!(again.track, "md2_flat");
    let mut next = Menu::new(again, Controls::default());
    next.set_mod_tracks(vec![("md2_flat".into(), "MD2 Flat".into())]);
    assert_eq!(next.cursor(Screen::Tracks), menu::DISC_PICK);
    // Gone from the folder, it opens on the first track; a name that cannot be a mod's is not kept.
    next.set_mod_tracks(Vec::new());
    assert_eq!(next.cursor(Screen::Tracks), 0);
    assert_eq!(Settings::parse("track = ../free06\n").track, Settings::default().track);

    // The Mods page, the main menu's: its first row is the folder.
    let (mut m, mut r) = fresh();
    m.set_mod_tracks(vec![("md2_flat".into(), "MD2 Flat".into()), ("mxvatv_nat13".into(), "Nat 13".into())]);
    main_to(&mut m, &mut r, "Mods");
    assert_eq!(m.screen(), Some(Screen::Mods));
    assert_eq!(m.page()[m.cursor(Screen::Mods)].key, Key::ModsFolder);
    // One pack off: its track leaves the pick, and the file says which.
    down_to(&mut m, &mut r, Key::Pack(0));
    let pack = m.page()[m.cursor(Screen::Mods)].clone();
    assert_eq!(pack.value, "On");
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.take_changed());
    assert_eq!(m.settings.mods_off, [menu::pack_of("md2_flat")]);
    assert_eq!(m.track_count(), menu::DISC_PICK + 1);
    assert!(m.settings.text().contains(&format!("mods_off = {}\n", menu::pack_of("md2_flat"))), "{}", m.settings.text());
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.settings.mods_off.is_empty());
    // The folder off: every pack dims and says why.
    down_to(&mut m, &mut r, Key::ModsFolder);
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.take_changed());
    assert!(!m.settings.mods);
    assert!(m.page().iter().filter(|r| matches!(r.key, Key::Pack(_))).all(|r| r.off.as_deref() == Some("The mods folder is off")));
    assert!(m.settings.text().contains("mods = off\n"));
    let dir = scratch("mods");
    menu::save(&dir, &m.settings, &Controls::default()).unwrap();
    assert!(!menu::load(&dir).0.mods);
    std::fs::remove_dir_all(&dir).unwrap();
    tap(&mut m, &mut r, Control::Right);
    assert!(m.settings.mods);
}

/// P122: the Mods page lists the music packs after the track packs, each On or Off by its whole id
/// (so `mxvatv_music` is not the `mxvatv` track pack), saved in `mods_off` beside them and read back,
/// dimmed with the folder; the main menu's count has them; a pack the folder lost stays listed.
#[test]
fn the_mods_page_lists_music_packs_beside_the_track_packs() {
    use menu::Key;
    let (mut m, mut r) = fresh();
    m.set_mod_tracks(vec![("mxvatv_nat13".into(), "Nat 13".into())]);
    m.set_music_packs(vec![("mxvatv_music".into(), "Unleashed music".into(), 30), ("untamed_music".into(), "Untamed music".into(), 1)]);
    main_to(&mut m, &mut r, "Mods");
    let rows = m.page();
    let keys: Vec<&Key> = rows.iter().filter(|r| r.selectable()).map(|r| &r.key).collect();
    assert_eq!(keys, [&Key::ModsFolder, &Key::Pack(0), &Key::MusicPack(0), &Key::MusicPack(1)]);
    let music = rows.iter().find(|r| r.key == Key::MusicPack(1)).unwrap();
    assert_eq!((music.label.as_str(), music.value.as_str()), ("Untamed music", "On"));
    assert!(music.about.contains("1 song,"), "{}", music.about);
    assert!(rows.iter().find(|r| r.key == Key::MusicPack(0)).unwrap().about.contains("30 songs"));

    // Off by its id, the track pack of the same prefix left on, and the file says so.
    down_to(&mut m, &mut r, Key::MusicPack(0));
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.take_changed());
    assert_eq!(m.settings.mods_off, ["mxvatv_music"]);
    assert_eq!(m.page().iter().find(|r| r.key == Key::MusicPack(0)).unwrap().value, "Off");
    assert_eq!(m.page().iter().find(|r| r.key == Key::Pack(0)).unwrap().value, "On");
    assert_eq!(m.track_count(), menu::DISC_PICK + 1, "a music pack off leaves the tracks alone");
    let text = m.settings.text();
    assert!(text.contains("mods_off = mxvatv_music\n"), "{text}");
    assert_eq!(Settings::parse(&text).mods_off, ["mxvatv_music"]);
    // The track pack off too: both kept, each by its own word.
    down_to(&mut m, &mut r, Key::Pack(0));
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(Settings::parse(&m.settings.text()).mods_off, ["mxvatv_music", "mxvatv"]);
    // A 40-letter id is kept too.
    let long = "a_".to_string() + &"b".repeat(38);
    assert_eq!(Settings::parse(&format!("mods_off = {long}\n")).mods_off, [long]);
    // And back on.
    down_to(&mut m, &mut r, Key::MusicPack(0));
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.mods_off, ["mxvatv"]);
    assert_eq!(m.packs_on(), (2, 3), "the main menu counts the music packs");

    // The folder off: the music packs dim and say why, and stay listed though the shell reads none.
    down_to(&mut m, &mut r, Key::ModsFolder);
    tap(&mut m, &mut r, Control::Cross);
    m.set_mod_tracks(Vec::new());
    m.set_music_packs(Vec::new());
    let music: Vec<_> = m.page().into_iter().filter(|r| matches!(r.key, Key::MusicPack(_))).collect();
    assert_eq!(music.len(), 2);
    assert!(music.iter().all(|r| r.off.as_deref() == Some("The mods folder is off")));

    // Music packs alone are not "nothing in the folder".
    let (mut m, mut r) = fresh();
    m.set_music_packs(vec![("untamed_music".into(), "Untamed music".into(), 22)]);
    main_to(&mut m, &mut r, "Mods");
    assert!(m.page().iter().all(|r| r.label != "Nothing in the mods folder"));
}

/// P118: L1 and R1 walk Settings' six tabs (P128's Graphics third; P126's Original a page of Ride's
/// since P179) and the Garage's three round from either end, each tab opening on its first row.
#[test]
fn l1_and_r1_walk_every_tab_round_from_either_end() {
    use menu::Key;
    let (mut m, mut r) = fresh();
    main_to(&mut m, &mut r, "Settings");
    assert_eq!(m.tabs(Screen::Settings), menu::SETTINGS_TABS);
    assert_eq!(menu::SETTINGS_TABS, ["Ride", "Display", "Graphics", "Audio", "Controls", "Camera"]);
    tab_to(&mut m, &mut r, "Ride");
    // Controls has no Player row with one seat (P81); Graphics opens past its World heading.
    let first = [Key::Bays, Key::Hud, Key::DrawDistance, Key::Mix, Key::BikeLayout, Key::Player];
    let here = |m: &Menu, s: Screen| (m.tab(s), m.page()[m.cursor(s)].key);
    for (i, key) in first.iter().enumerate() {
        assert_eq!(here(&m, Screen::Settings), (i, *key));
        // A row down, so the next tab's first row is the tab's doing.
        tap(&mut m, &mut r, Control::Down);
        tap(&mut m, &mut r, Control::R1);
    }
    assert_eq!(here(&m, Screen::Settings), (0, Key::Bays), "R1 from Camera is Ride");
    tap(&mut m, &mut r, Control::L1);
    assert_eq!(here(&m, Screen::Settings), (5, Key::Player), "L1 from Ride is Camera");
    for i in (0..5).rev() {
        tap(&mut m, &mut r, Control::L1);
        assert_eq!(here(&m, Screen::Settings), (i, first[i]));
    }
    // The Garage: Rider, Bike, Gear, each with its Player row first.
    tap(&mut m, &mut r, Control::Circle);
    main_to(&mut m, &mut r, "Garage");
    assert_eq!(m.screen(), Some(Screen::Garage));
    assert_eq!(m.tabs(Screen::Garage), menu::GARAGE_TABS);
    assert_eq!(menu::GARAGE_TABS, ["Rider", "Bike", "Gear"]);
    for i in [0, 1, 2, 0] {
        assert_eq!(here(&m, Screen::Garage), (i, Key::Player));
        tap(&mut m, &mut r, Control::R1);
    }
    assert_eq!(m.tab(Screen::Garage), 1);
    tap(&mut m, &mut r, Control::L1);
    tap(&mut m, &mut r, Control::L1);
    assert_eq!(here(&m, Screen::Garage), (2, Key::Player), "L1 from Rider is Gear");
}

/// P128: Settings' Graphics tab, reached by L1 and R1 from either side and by `--screen graphics`,
/// has every graphics setting once, under its group's heading, and no other tab has any of them;
/// Draw Distance changed there is `Settings::graphics`, and saved.
#[test]
fn the_graphics_tab_has_each_graphics_setting_once_under_its_group() {
    use menu::{Departure, Key};
    let groups = [
        ("World", vec![Key::DrawDistance, Key::Departure(Departure::Plants), Key::Departure(Departure::Foliage), Key::Departure(Departure::Textures), Key::WorldSky]),
        ("Picture", vec![Key::TextureFiltering]),
        ("Performance", vec![Key::RenderScale, Key::VSync, Key::FrameLimit]),
    ];
    let graphics: Vec<Key> = groups.iter().flat_map(|(_, keys)| keys.clone()).collect();
    let (mut m, mut r) = fresh();
    main_to(&mut m, &mut r, "Settings");
    let tab = |m: &Menu| m.tabs(Screen::Settings)[m.tab(Screen::Settings)];
    // R1 from Display, L1 from Audio.
    tab_to(&mut m, &mut r, "Display");
    tap(&mut m, &mut r, Control::R1);
    assert_eq!(tab(&m), "Graphics");
    tap(&mut m, &mut r, Control::R1);
    assert_eq!(tab(&m), "Audio");
    tap(&mut m, &mut r, Control::L1);
    assert_eq!(tab(&m), "Graphics");
    // Each under its heading, in order, and once.
    let page = m.page();
    let mut under = None;
    let mut seen = Vec::new();
    for row in &page {
        if row.kind == menu::Kind::Heading {
            under = Some(row.label.clone());
            continue;
        }
        let group = groups.iter().find(|(_, keys)| keys.contains(&row.key)).map(|(g, _)| g.to_string());
        assert!(group.is_some(), "{:?} ({}) is not a graphics setting", row.key, row.label);
        assert_eq!(under, group, "{} under the wrong heading", row.label);
        seen.push(row.key);
    }
    assert_eq!(seen, graphics, "each graphics setting once, in its group's order");
    let headings: Vec<String> = page.iter().filter(|r| r.kind == menu::Kind::Heading).map(|r| r.label.clone()).collect();
    assert_eq!(headings, ["World", "Picture", "Performance"]);
    // The Original tab's graphics sub-switches (P126) are these rows.
    for d in Departure::ALL {
        assert_eq!(graphics.contains(&d.key()), menu::SETTINGS_TABS[d.tab()] == "Graphics", "{d:?}");
    }
    // On no other tab: Display is the HUD's, Ride no longer repeats any, Original keeps the rest.
    for other in menu::SETTINGS_TABS.into_iter().filter(|t| *t != "Graphics").chain(["Original"]) {
        tab_to(&mut m, &mut r, other);
        for row in m.page() {
            assert!(!graphics.contains(&row.key), "{other} has {} too", row.label);
        }
    }
    // The cursor opens on Draw Distance, past the heading, and the d-pad steps over the next one.
    tab_to(&mut m, &mut r, "Graphics");
    assert_eq!(m.page()[m.cursor(Screen::Settings)].key, Key::DrawDistance);
    for _ in 0..5 {
        tap(&mut m, &mut r, Control::Down);
    }
    assert_eq!(m.page()[m.cursor(Screen::Settings)].key, Key::TextureFiltering);
    for _ in 0..5 {
        tap(&mut m, &mut r, Control::Up);
    }
    // Draw Distance changed by the pad is the setting, and is saved.
    assert_eq!(m.settings.graphics, Graphics::Full);
    tap(&mut m, &mut r, Control::Right);
    assert!(m.take_changed());
    // V95's Farther Fog is next after the Whole Map
    assert_eq!(m.settings.graphics, Graphics::Farther);
    let dir = scratch("graphics-tab");
    menu::save(&dir, &m.settings, &Controls::default()).unwrap();
    assert_eq!(menu::load(&dir).0.graphics, Graphics::Farther);
    std::fs::remove_dir_all(&dir).ok();
    // `--screen graphics` opens the same tab.
    assert_eq!(Screen::from_word("graphics"), Some(Screen::Graphics));
    let (mut m, _) = fresh();
    m.open(Screen::Graphics);
    assert_eq!(m.screen(), Some(Screen::Settings));
    assert_eq!(tab(&m), "Graphics");
    assert_eq!(m.page()[m.cursor(Screen::Settings)].key, Key::DrawDistance);
}

/// P176: the Display tab's Aspect Ratio row has opentennis's six choices, steps through them by the
/// pad into `Settings::picture`, which is saved by `--aspect`'s word, and is the Picture departure's
/// switch: the Original switch puts it at 4:3 and back to Auto.
#[test]
fn the_display_tab_has_opentennis_aspect_ratios() {
    use menu::{Departure, Key, Picture};
    let (mut m, mut r) = fresh();
    main_to(&mut m, &mut r, "Settings");
    tab_to(&mut m, &mut r, "Display");
    let row = m.page().into_iter().find(|row| row.key == Key::AspectRatio).expect("an Aspect Ratio row on Display");
    assert_eq!(row.label, "Aspect Ratio");
    assert_eq!(row.choices, ["Auto", "4:3", "16:9", "16:10", "21:9", "Stretch"]);
    assert_eq!(row.value, "Auto");
    assert_eq!(Departure::Picture.key(), Key::AspectRatio);
    assert_eq!(menu::SETTINGS_TABS[Departure::Picture.tab()], "Display");
    while m.page()[m.cursor(Screen::Settings)].key != Key::AspectRatio {
        tap(&mut m, &mut r, Control::Down);
    }
    for want in [Picture::Television, Picture::Wide, Picture::Monitor, Picture::Cinema, Picture::Stretch, Picture::Fill] {
        tap(&mut m, &mut r, Control::Right);
        assert!(m.take_changed());
        assert_eq!(m.settings.picture, want);
        let dir = scratch("aspect-ratio-row");
        menu::save(&dir, &m.settings, &Controls::default()).unwrap();
        assert_eq!(menu::load(&dir).0.picture, want, "{want:?} saved");
        std::fs::remove_dir_all(&dir).ok();
    }
    for p in Picture::ALL {
        assert_eq!(Picture::from_word(p.word()), Some(p));
    }
    // 16:9 is ours, 4:3 the original's.
    m.settings.picture = Picture::Wide;
    assert!(!m.is_original(Departure::Picture));
    m.set_departure(Departure::Picture, true);
    assert_eq!(m.settings.picture, Picture::Television);
    m.set_departure(Departure::Picture, false);
    assert_eq!(m.settings.picture, Picture::Fill);
}

/// P118: every row of every Settings tab, of the modifiers page and of the Mods page is reachable by
/// the d-pad and changes its setting by a press, or opens what it opens; a dimmed row says why and
/// ignores presses.
#[test]
fn every_row_of_every_settings_tab_and_page_changes_by_pad() {
    // A shell with a device and a pressure map named, two songs, and One Song on, so no row is dim
    // for want of something to pick.
    let shell = || {
        let (mut m, r) = fresh();
        m.controls.pad = Some("/dev/input/js1".into());
        m.controls.pressure = "ds2@8".into();
        m.music_choices = vec![("disc:a".into(), "A".into()), ("disc:b".into(), "B".into())];
        m.settings.music = "disc:a".into();
        m.settings.presets = vec![("Moon".into(), Mods::parse("gravity=20").unwrap()), ("Fast".into(), Mods::parse("bike_speed=150").unwrap())];
        m.set_mod_tracks(vec![("md2_flat".into(), "MD2 Flat".into()), ("mxvatv_nat13".into(), "Nat 13".into())]);
        m.set_music_packs(vec![("mxvatv_music".into(), "Unleashed music".into(), 30)]);
        // The disc's camera presets, which the Camera tab's Preset row lists (P163).
        m.camera_names = [vec!["Race Cam 1".into(), "Freestyle Cam".into()], vec!["Flight Cam 1".into()]];
        (m, r)
    };
    for tab in menu::SETTINGS_TABS {
        every_row_changes(
            &|| {
                let (mut m, mut r) = shell();
                settings_at(&mut m, &mut r, tab);
                (m, r)
            },
            tab,
        );
    }
    every_row_changes(
        &|| {
            let (mut m, mut r) = shell();
            settings_at(&mut m, &mut r, "Ride");
            down_to(&mut m, &mut r, menu::Key::Modifiers);
            tap(&mut m, &mut r, Control::Cross);
            assert_eq!(m.screen(), Some(Screen::Modifiers));
            (m, r)
        },
        "Modifiers",
    );
    every_row_changes(
        &|| {
            let (mut m, mut r) = shell();
            main_to(&mut m, &mut r, "Mods");
            assert_eq!(m.screen(), Some(Screen::Mods));
            (m, r)
        },
        "Mods",
    );
    // The Original Game page, the Ride tab's since P179.
    every_row_changes(
        &|| {
            let (mut m, mut r) = shell();
            settings_at(&mut m, &mut r, "Original");
            (m, r)
        },
        "Original",
    );
}

/// P118: the pause's Audio row is Settings' Audio tab, and Back from it is the pause.
#[test]
fn the_pauses_audio_row_opens_settings_on_its_audio_tab() {
    let (mut m, mut r) = fresh();
    m.pause();
    assert_eq!(m.pause_rows()[3], "Audio");
    tap_until(&mut m, &mut r, Control::Down, |m| m.cursor(Screen::Pause) == 3);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Settings));
    assert_eq!(m.tabs(Screen::Settings)[m.tab(Screen::Settings)], "Audio");
    assert_eq!(m.page()[m.cursor(Screen::Settings)].key, menu::Key::Mix);
    // Its other tabs are there too, and Back is the pause, on its Audio row.
    tap(&mut m, &mut r, Control::R1);
    assert_eq!(m.tabs(Screen::Settings)[m.tab(Screen::Settings)], "Controls");
    tap(&mut m, &mut r, Control::Circle);
    assert_eq!(m.screen(), Some(Screen::Pause));
    assert_eq!(m.cursor(Screen::Pause), 3);
}

/// P118 renamed rows and moved them, but every key of the two files is read as it was: a
/// settings.txt and a controls.txt written before the redo, every key in them, read as the same
/// settings, and written back word for word.
#[test]
fn a_settings_file_from_before_the_redo_reads_as_the_same_settings() {
    use menu::CameraView;
    let custom = CameraView { fov: -5, distance: 140, height: 95, plane_distance: 100, plane_height: 180 };
    let old = format!(
        "# openmx's front end (P36)\nname = Ricky C\noverlay = none\nspeedometer = analog\nwrecks = off\ntrack = free11\n\
         machines = biplane bike biplane bike bike bike bike biplane\nsplit = v\nmusic = disc:headstrong\nmusic_volume = 35\n\
         effects_volume = 100\nfolder_volume = 5\ncrowd_volume = 0\nengine_volume = 90\nmenu_volume = 55\nmix = custom\n\
         custom = 35 5 100 0 90 55\ngraphics = original\nrender_scale = 75\nvsync = off\nframe_limit = 75\nmachine_bays = off\n\
         mods = off\nlobby_seats = 16\ncamera = {}, {}, {}, {}\nmodifiers = gravity=50 bike_speed.2=200\npreset = Moon | gravity=20\n",
        CameraView::ORIGINAL.text(),
        CameraView::DEFAULT.text(),
        custom.text(),
        CameraView::DEFAULT.text()
    );
    let want = Settings {
        name: "Ricky C".into(),
        overlay: Overlay::None,
        speedometer: Speedometer::Analog,
        wrecks: false,
        track: "free11".into(),
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
        vsync: false,
        frame_limit: FrameLimit::Fps(75),
        bays: false,
        mods: false,
        lobby_seats: 16,
        cameras: {
            let mut c = [CameraView::DEFAULT; menu::SEATS_MAX];
            c[0] = CameraView::ORIGINAL;
            c[2] = custom;
            c
        },
        modifiers: Mods::parse("gravity=50 bike_speed.2=200").unwrap(),
        presets: vec![("Moon".into(), Mods::parse("gravity=20").unwrap())],
        ..Settings::default()
    };
    assert_eq!(Settings::parse(&old), want);
    // Every line but the ride setup's machines, which went with Start On (P177).
    let machines = "machines = biplane bike biplane bike bike bike bike biplane\n";
    assert!(old.contains(machines));
    assert_eq!(want.text(), old.replace(machines, ""), "the redo writes an old file's settings as they were");
    // The original's few keys from P36 alone.
    let p36 = Settings::parse("name = Rider\noverlay = on\nwrecks = on\n");
    assert_eq!(p36, Settings { name: "Rider".into(), ..Settings::default() });

    let old = "throttle = r2\npad = /dev/input/js1\npressure = ds2@8\nlayout = new,original,new,new,new,new,new,new\n\
               biplane = new,new,original,new,new,new,new,new\nnintendo = positions\npreset = 0,2,0,0,0,0,0,0\n";
    let (n, o) = (Layout::New, Layout::Original);
    let want = Controls {
        throttle: Gas::R2,
        pad: Some("/dev/input/js1".into()),
        pressure: "ds2@8".into(),
        layouts: [n, o, n, n, n, n, n, n],
        planes: [n, n, o, n, n, n, n, n],
        presets: [0, 2, 0, 0, 0, 0, 0, 0],
        nintendo_positions: true,
    };
    assert_eq!(Controls::parse(old), want);
    assert!(want.text().ends_with(old), "{}", want.text());
    // One from before P40, P50, P53 and P79: a throttle, a pad, the rest the defaults. L2 is no
    // throttle since P81, so its word reads as the default.
    let early = Controls::parse("throttle = l2\npad = auto\npressure = none\n");
    assert_eq!(early, Controls::default());
    assert_eq!(Controls::parse("throttle = r2").throttle, Gas::R2);
    // P64: positions are the default, P53's default `labels` reads as positions, and only the
    // menu's `by-label` puts a Nintendo pad on its labels, which round trips.
    assert!(early.nintendo_positions);
    assert!(Controls::parse("nintendo = labels\n").nintendo_positions);
    let labels = Controls::parse("nintendo = by-label\n");
    assert!(!labels.nintendo_positions);
    assert_eq!(Controls::parse(&labels.text()), labels);
}

/// How soft the edges of white ink on black in `rgb` are: of the pixels part way between, the share
/// with no near-full and no near-empty pixel both among their eight neighbours and themselves. An
/// edge a pixel wide has nearly none; an edge two pixels wide has many.
fn soft_share(rgb: &[u8], w: usize, h: usize) -> (f64, usize) {
    let at = |x: usize, y: usize| rgb[(y * w + x) * 3];
    let (mut partial, mut soft) = (0usize, 0usize);
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let v = at(x, y);
            if !(26..=229).contains(&v) {
                continue;
            }
            partial += 1;
            let (mut full, mut empty) = (false, false);
            for dy in 0..3 {
                for dx in 0..3 {
                    let n = at(x + dx - 1, y + dy - 1);
                    full |= n >= 230;
                    empty |= n <= 25;
                }
            }
            if !(full && empty) {
                soft += 1;
            }
        }
    }
    (soft as f64 / partial.max(1) as f64, partial)
}

/// `rgb` through a 3x3 box blur: a label drawn small and scaled up, as the old menus' were.
fn blurred(rgb: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = rgb.to_vec();
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            for c in 0..3 {
                let mut sum = 0u32;
                for dy in 0..3 {
                    for dx in 0..3 {
                        sum += u32::from(rgb[((y + dy - 1) * w + x + dx - 1) * 3 + c]);
                    }
                }
                out[(y * w + x) * 3 + c] = (sum / 9) as u8;
            }
        }
    }
    out
}

/// A slanted title, a label and a stadium at the stage's em for a `w` by `h` screen, white on black,
/// as sprites over a frame sized to hold them.
fn ink_sample(h: usize) -> (Vec<game_render::gpu::Sprite>, usize, usize) {
    use menu::ink::{self, Weight};
    let em = h as f32 / 45.0;
    let title = "SETTINGS · Draw Distance";
    let t = ink::ink();
    let fw = (t.measure(Weight::Heavy, title, 1.7 * em) + 6.0 * em).ceil() as usize;
    let fh = (7.0 * em).ceil() as usize;
    let mut out = Vec::new();
    let white = [255, 255, 255, 255];
    t.text(&mut out, 0, Weight::Heavy, title, 2.0 * em, 2.2 * em, 1.7 * em, ink::SLANT, white);
    t.text(&mut out, 0, Weight::Bold, "Render Scale  75%", 2.0 * em, 4.0 * em, em, 0.0, white);
    ink::round(&mut out, [2.0 * em, 4.8 * em, 14.0 * em, 1.6 * em], 0.8 * em, white, white);
    (out, fw, fh)
}

/// P118: the menus' ink stays sharp at any size. A label, a slanted title and a stadium drawn at
/// 1080p and at 4K, on the CPU and (where there is a GPU) the GPU, have edges no wider than a pixel:
/// nearly every pixel part way between ink and background touches both full ink and none.
#[test]
fn a_label_at_1080p_and_4k_has_edges_no_wider_than_a_pixel() {
    let atlas = &menu::ink::ink().atlas;
    let mut gpu = game_render::gpu::Renderer::headless().map_err(|e| eprintln!("the GPU half skipped: {e}")).ok();
    if let Some(g) = gpu.as_mut() {
        g.set_images(&[atlas]);
    }
    for h in [1080usize, 2160] {
        let (sprites, w, fh) = ink_sample(h);
        let mut frame = game_render::Frame::new(w, fh, [0, 0, 0]);
        menu::ink::raster(&mut frame, &sprites, &[atlas]);
        let (soft, partial) = soft_share(&frame.color, w, fh);
        let (soft_blur, _) = soft_share(&blurred(&frame.color, w, fh), w, fh);
        eprintln!("{h}p cpu: {partial} edge pixels, {:.2}% soft; blurred {:.2}%", soft * 100.0, soft_blur * 100.0);
        assert!(partial > 200, "{h}p: only {partial} edge pixels drawn");
        assert!(soft < 0.03, "{h}p cpu: {:.2}% of edge pixels are inside a wider edge", soft * 100.0);
        assert!(soft_blur > 3.0 * soft && soft_blur > 0.2, "the measure tells a soft edge: {:.2}%", soft_blur * 100.0);
        if let Some(g) = gpu.as_mut() {
            let texture = g.offscreen(w as u32, fh as u32);
            let view = texture.create_view(&Default::default());
            let target = game_render::gpu::Target { view: &view, size: (w as u32, fh as u32), frame: [0, 0, w as u32, fh as u32], scale: 1 };
            g.draw_sprites(&target, &sprites);
            let got = g.read(&texture);
            let (soft, partial) = soft_share(&got, w, fh);
            eprintln!("{h}p gpu: {partial} edge pixels, {:.2}% soft", soft * 100.0);
            assert!(partial > 200 && soft < 0.03, "{h}p gpu: {:.2}% of {partial} edge pixels soft", soft * 100.0);
        }
    }
}

// ---- P109: one push, one row -----------------------------------------------------------------------

/// Every screen and tab a pad's d-pad walks (P109), each as the pad reaches it from the main menu:
/// the main menu itself, the Free Ride pick, every tab of Settings and the Garage (P118's L1/R1
/// tabs), Mods, Online, the modifiers, the on-screen keyboard and the pause.
fn every_walked_screen() -> Vec<(String, Menu, Reader)> {
    let mut out = Vec::new();
    let (m, r) = fresh();
    out.push(("Main".to_string(), m, r));
    let (mut m, mut r) = fresh();
    main_to(&mut m, &mut r, "Free Ride");
    out.push(("Free Ride".to_string(), m, r));
    for (row, tabs) in [("Settings", &menu::SETTINGS_TABS[..]), ("Garage", &menu::GARAGE_TABS[..])] {
        for tab in tabs {
            let (mut m, mut r) = fresh();
            main_to(&mut m, &mut r, row);
            tab_to(&mut m, &mut r, tab);
            out.push((format!("{row}: {tab}"), m, r));
        }
    }
    for row in ["Mods", "Online"] {
        let (mut m, mut r) = fresh();
        main_to(&mut m, &mut r, row);
        out.push((row.to_string(), m, r));
    }
    let (mut m, mut r) = fresh();
    settings_at(&mut m, &mut r, "Ride");
    down_to(&mut m, &mut r, menu::Key::Modifiers);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Modifiers));
    out.push(("Modifiers".to_string(), m, r));
    // The Ride tab's Original Game page (P179), once Settings' last tab.
    let (mut m, mut r) = fresh();
    settings_at(&mut m, &mut r, "Original");
    out.push(("Original".to_string(), m, r));
    let (mut m, mut r) = fresh();
    main_to(&mut m, &mut r, "Garage");
    down_to(&mut m, &mut r, menu::Key::Garage(roster::Row::Name));
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Keyboard));
    out.push(("Keyboard".to_string(), m, r));
    let (mut m, mut r) = fresh();
    main_to(&mut m, &mut r, "Free Ride");
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.riding());
    tap(&mut m, &mut r, Control::Start);
    assert_eq!(m.screen(), Some(Screen::Pause));
    out.push(("Pause".to_string(), m, r));
    out
}

/// `m` after exactly one step of `press` and then `rest` frames of nothing: what one push must do.
fn after_one(m: &Menu, press: Press, rest: usize) -> String {
    let mut m = m.clone();
    m.step(press);
    for _ in 0..rest {
        m.step(Press::default());
    }
    key(&m)
}

/// The user's case (P109): one push of the d-pad's down — or up — moves exactly one row, on every
/// screen and tab, however long a hand holds it short of the repeat: a single frame, or most of a
/// third of a second. And through a contact's bounce, up for a frame and down again: still one.
#[test]
fn one_d_pad_push_moves_exactly_one_row_on_every_screen_and_tab() {
    for (name, m, r) in every_walked_screen() {
        for (control, press) in [(Control::Down, Press { down: true, ..Press::default() }), (Control::Up, Press { up: true, ..Press::default() })] {
            let push = Report::from_buttons(control.mask());
            let idle = Report::idle();
            let shapes: Vec<Vec<bool>> = (1..20)
                .map(|n| vec![true; n])
                .chain([vec![true, false, true, true], vec![true, true, false, true, true, true]])
                .collect();
            // The Mods screen is one row without a mods folder, as here: there a push is no move.
            if after_one(&m, press, 0) == key(&m) {
                assert_eq!(name, "Mods", "{name}: {} moves nothing", control.name());
                continue;
            }
            for shape in shapes {
                let (mut mm, mut rr) = (m.clone(), r.clone());
                for &down in &shape {
                    mm.step(rr.read(if down { &push } else { &idle }));
                }
                for _ in 0..4 {
                    mm.step(rr.read(&Report::idle()));
                }
                let frames = shape.len() + 4 - 1;
                assert_eq!(key(&mm), after_one(&m, press, frames), "{name}: {} held {shape:?} is not one row", control.name());
            }
        }
    }
}

/// The same push through the shell's pad layer, the user's Pro Controller's hat (P109): pressed and
/// let go between two frames, held over several, and bouncing — up for three milliseconds inside a
/// frame, and up for a frame — every one of them a single row of the pause and of the main menu.
#[test]
fn one_push_of_a_pro_controller_s_hat_moves_one_row_through_the_pad_layer() {
    let _fifos = pad::tests::FIFOS.lock().unwrap_or_else(|e| e.into_inner());
    let root = pad::tests::scratch("menu-one-row");
    let (dev, sys) = (root.join("dev"), root.join("sys"));
    std::fs::create_dir_all(&dev).unwrap();
    let mut pro = pro_controller(&dev, &sys, "js0");
    let mut seats = pad::tests::seats_in(&dev, &sys, pad::Wiring::default());
    let (mut m, mut r) = fresh();
    let mut frame = |m: &mut Menu, seats: &mut pad::Seats| {
        let got = pad::tests::devices_now(seats, &|_| false);
        let (_, report, taps) = got[1];
        m.step(r.read_taps(&report, taps));
    };
    frame(&mut m, &mut seats);
    let pushes: [&dyn Fn(&mut pad::tests::FakePad, &mut dyn FnMut()); 4] = [
        // Down and up between two frames.
        &|p, _| {
            p.gap(50);
            p.axis(5, i16::MAX);
            p.axis(5, 0);
        },
        // Held across six frames.
        &|p, f| {
            p.gap(50);
            p.axis(5, i16::MAX);
            for _ in 0..6 {
                f();
            }
            p.axis(5, 0);
        },
        // Bouncing inside a frame.
        &|p, f| {
            p.gap(50);
            p.axis(5, i16::MAX);
            p.gap(3);
            p.axis(5, 0);
            p.axis(5, i16::MAX);
            for _ in 0..3 {
                f();
            }
            p.gap(50);
            p.axis(5, 0);
        },
        // Bouncing across a frame: up in one, down again in the next.
        &|p, f| {
            p.gap(50);
            p.axis(5, i16::MAX);
            f();
            p.gap(3);
            p.axis(5, 0);
            f();
            p.axis(5, i16::MAX);
            for _ in 0..3 {
                f();
            }
            p.gap(50);
            p.axis(5, 0);
        },
    ];
    for (i, push) in pushes.iter().enumerate() {
        let at = m.cursor(Screen::Main);
        let mut f = || frame(&mut m, &mut seats);
        push(&mut pro, &mut f);
        // The frames the push asked for, then some at rest.
        for _ in 0..5 {
            f();
        }
        assert_eq!(m.cursor(Screen::Main), (at + 1) % menu::MAIN.len(), "push {i}");
    }
    std::fs::remove_dir_all(&root).unwrap();
}

/// A ride's menu as main.rs steps it (P109): at the ride's 30 Hz ticks, with a reader of that rate.
fn riding_at_30() -> (Menu, Reader) {
    let (mut m, mut r) = fresh();
    main_to(&mut m, &mut r, "Free Ride");
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.riding());
    let mut r = Reader::at(30);
    m.step_seats(&[r.read(&Report::idle())]);
    (m, r)
}

/// Start held for many ticks opens the pause once and leaves it open (P109): the press that opened
/// it is not the Resume too, nor is the hold. Only a fresh press of it closes the pause.
#[test]
fn start_held_for_many_frames_opens_the_pause_once_and_leaves_it_open() {
    let (mut m, mut r) = riding_at_30();
    let start = Report::from_buttons(Control::Start.mask());
    for tick in 0..90 {
        let act = m.step_seats(&[r.read(&start)]);
        assert_eq!(act, Act::Stay, "tick {tick}");
        assert_eq!(m.screen(), Some(Screen::Pause), "tick {tick}");
        assert_eq!(m.cursor(Screen::Pause), 0, "tick {tick}");
    }
    for _ in 0..3 {
        m.step_seats(&[r.read(&Report::idle())]);
    }
    assert_eq!(m.screen(), Some(Screen::Pause));
    assert_eq!(m.step_seats(&[r.read(&start)]), Act::Resume);
    assert!(m.riding());
    // A second rider's Start held over a split opens it once for them, and theirs closes it.
    let mut r2 = Reader::at(30);
    m.step_seats(&[r.read(&Report::idle()), r2.read(&Report::idle())]);
    for tick in 0..60 {
        m.step_seats(&[r.read(&Report::idle()), r2.read(&start)]);
        assert_eq!(m.view(1).screen(), Some(Screen::Pause), "tick {tick}");
    }
    m.step_seats(&[r.read(&Report::idle()), r2.read(&Report::idle())]);
    // 1P's Start is not theirs to close it with: it opens 1P's own (P84).
    m.step_seats(&[r.read(&start), r2.read(&Report::idle())]);
    assert_eq!(m.view(1).screen(), Some(Screen::Pause));
    assert!(m.paused(0) && m.paused(1));
}

/// The left stick leaned to `y` out of 127 from the centre, down positive.
fn stick_y(y: i32) -> Report {
    let mut r = Report::idle();
    r.set_axis(3, (AXIS_CENTRE as i32 + y).clamp(0, 255) as u8);
    r
}

/// A stick resting just off centre moves nothing, however long (P109), and one resting on the
/// line of the lean — a worn stick's noise either side of it — is one press, not one each time it
/// crosses: the direction lets go only once the stick is well back towards the centre.
#[test]
fn a_stick_resting_just_off_centre_does_not_move_the_cursor() {
    for (name, m, r) in every_walked_screen() {
        let (mut mm, mut rr) = (m.clone(), r.clone());
        for f in 0..300 {
            let wobble = [20, 30, 45, 55, 60, 50, 35, 25][f % 8];
            mm.step(rr.read(&stick_y(wobble)));
            mm.step(rr.read(&stick_y(-wobble)));
        }
        assert_eq!(key(&mm), key(&m), "{name}: a stick off centre moved it");
        // On the line, wobbling over and under it, for under the repeat: one row.
        let (mut mm, mut rr) = (m.clone(), r.clone());
        let line = [62, 66, 60, 70, 58, 64, 61, 67, 59, 65];
        for &y in &line {
            mm.step(rr.read(&stick_y(y)));
        }
        for _ in 0..4 {
            mm.step(rr.read(&Report::idle()));
        }
        let once = after_one(&m, Press { down: true, ..Press::default() }, line.len() + 3);
        assert_eq!(key(&mm), once, "{name}: a stick on the line is not one row");
    }
}

/// How many rows `frames` of a held Down move the pause's cursor, read `hz` times a second.
fn held_rows(hz: u32, frames: usize) -> usize {
    let (mut m, _) = riding_at_30();
    m.pause();
    let mut r = Reader::at(hz);
    m.step(r.read(&Report::idle()));
    let down = Report::from_buttons(Control::Down.mask());
    let mut moved = 0;
    for _ in 0..frames {
        let at = m.cursor(Screen::Pause);
        m.step(r.read(&down));
        moved += usize::from(m.cursor(Screen::Pause) != at);
    }
    moved
}

/// A held push repeats at the front end's rate (P109): the front end's frames are 60 a second and
/// the pause's the ride's 30 ticks, and either way a held Down waits a third of a second and then
/// moves twelve rows a second. Before, the pause counted frames and repeated at half the rate.
#[test]
fn a_held_push_repeats_at_the_front_ends_rate() {
    // One second, and two: the push, then the repeats.
    assert_eq!(held_rows(60, 60), 1 + 8);
    assert_eq!(held_rows(30, 30), held_rows(60, 60));
    assert_eq!(held_rows(30, 60), held_rows(60, 120));
    // Not before a third of a second.
    assert_eq!(held_rows(60, 19), 1);
    assert_eq!(held_rows(30, 9), 1);
}

/// Two devices seeing the one push (P109) — 1P's pad and the keyboard a desktop maps its d-pad and
/// buttons to, one a frame behind the other — are one press: one row, and a Start that opens the
/// pause rather than opening and closing it.
#[test]
fn a_push_both_the_pad_and_the_keyboard_report_is_one_press() {
    let (mut m, mut readers) = on_the_splash();
    let rest = [(Dev::Keys, Report::idle()), (pad("pro"), Report::idle())];
    let hosted = |m: &Menu| {
        let m = m.clone();
        move |d: &Dev| m.screen() != Some(Screen::Splash) && m.hosts(d)
    };
    let frame = |m: &mut Menu, readers: &mut Readers, reports: &[(Dev, Report)]| {
        let with: Vec<(Dev, Report, u16)> = reports.iter().map(|(d, r)| (d.clone(), *r, 0)).collect();
        let presses = readers.read_hosted(&with, &hosted(m));
        m.step_devices(&presses)
    };
    frame(&mut m, &mut readers, &rest);
    frame(&mut m, &mut readers, &[(Dev::Keys, Report::idle()), (pad("pro"), cross())]);
    assert_eq!(m.p1(), Some(&pad("pro")));
    for _ in 0..CLAIMED_FRAMES + 2 {
        frame(&mut m, &mut readers, &rest);
    }
    assert_eq!(m.screen(), Some(Screen::Main));
    let down = Report::from_buttons(Control::Down.mask());
    // The pad first, the keyboard a frame later, and the keyboard let go a frame after the pad.
    frame(&mut m, &mut readers, &[(Dev::Keys, Report::idle()), (pad("pro"), down)]);
    frame(&mut m, &mut readers, &[(Dev::Keys, down), (pad("pro"), down)]);
    frame(&mut m, &mut readers, &[(Dev::Keys, down), (pad("pro"), Report::idle())]);
    for _ in 0..4 {
        frame(&mut m, &mut readers, &rest);
    }
    assert_eq!(m.cursor(Screen::Main), 1);
    // A second pad nobody hosts holding Down does not hold 1P's back.
    frame(&mut m, &mut readers, &[(Dev::Keys, Report::idle()), (pad("pro"), Report::idle()), (pad("other"), down)]);
    frame(&mut m, &mut readers, &[(Dev::Keys, Report::idle()), (pad("pro"), down), (pad("other"), down)]);
    assert_eq!(m.cursor(Screen::Main), 2);

    // The pause's Start, in the ride: 1P's pad and the keyboard read as one, as main.rs does.
    let (mut m, _) = riding_at_30();
    let mut r = Reader::at(30);
    let start = Report::from_buttons(Control::Start.mask());
    m.step_seats(&[r.read_joint(&[(Report::idle(), 0), (Report::idle(), 0)])]);
    m.step_seats(&[r.read_joint(&[(start, 0), (Report::idle(), 0)])]);
    assert_eq!(m.screen(), Some(Screen::Pause));
    m.step_seats(&[r.read_joint(&[(start, 0), (start, 0)])]);
    m.step_seats(&[r.read_joint(&[(Report::idle(), 0), (start, 0)])]);
    m.step_seats(&[r.read_joint(&[(Report::idle(), 0), (Report::idle(), 0)])]);
    assert_eq!(m.screen(), Some(Screen::Pause), "the keyboard's Start, a tick behind the pad's, closed it");
}

// ---- leaving from the pause asks first (P110) -------------------------------------------------------

/// A plain press of one kind, for [`Menu::step_seats`].
fn press(f: impl Fn(&mut Press)) -> Press {
    let mut p = Press::default();
    f(&mut p);
    p
}

/// The pause's Main Menu and Quit rows open a two-row confirm on Stay instead of acting: the ride
/// stays under it, Stay, Circle and Start go back to the pause on the same row, and only the leaving
/// row yields [`Act::MainMenu`] (or [`Act::Quit`]) — worded for where the shell says it goes.
#[test]
fn leaving_from_the_pause_asks_first_on_stay() {
    use menu::LeaveTo;
    for (to, ask, go) in [
        (LeaveTo::Main, "Main menu?", "Main Menu"),
        (LeaveTo::Lobby, "Return to lobby?", "Return to Lobby"),
        (LeaveTo::Online, "Leave the online game?", "Leave Game"),
    ] {
        let (mut m, mut r) = fresh();
        m.leave_to = to;
        m.pause();
        tap_until(&mut m, &mut r, Control::Down, |m| m.pause_rows()[m.cursor(Screen::Pause)] == "Main Menu");
        let at = m.cursor(Screen::Pause);
        let acts = tap(&mut m, &mut r, Control::Cross);
        assert!(acts.iter().all(|a| *a == Act::Stay), "{to:?}: {acts:?}");
        assert_eq!(m.screen(), Some(Screen::Pause), "{to:?}");
        assert_eq!(m.confirming(), Some((ask, [go, "Stay"], 1)), "{to:?}");
        // Cross on Stay, Circle and Start: back to the pause, on the row it was, the ride not left.
        for back in [Control::Cross, Control::Circle, Control::Start] {
            if m.confirming().is_none() {
                tap(&mut m, &mut r, Control::Cross);
            }
            assert_eq!(m.confirming().map(|c| c.2), Some(1), "{to:?} {back:?}");
            let acts = tap(&mut m, &mut r, back);
            assert!(acts.iter().all(|a| *a == Act::Stay), "{to:?} {back:?}: {acts:?}");
            assert_eq!(m.confirming(), None, "{to:?} {back:?}");
            assert_eq!(m.screen(), Some(Screen::Pause), "{to:?} {back:?}");
            assert_eq!(m.cursor(Screen::Pause), at, "{to:?} {back:?}");
        }
        // Up or Down moves between the two rows, round; Cross on the first leaves.
        tap(&mut m, &mut r, Control::Cross);
        tap(&mut m, &mut r, Control::Down);
        assert_eq!(m.confirming().map(|c| c.2), Some(0), "{to:?}");
        tap(&mut m, &mut r, Control::Up);
        assert_eq!(m.confirming().map(|c| c.2), Some(1), "{to:?}");
        tap(&mut m, &mut r, Control::Up);
        let acts = tap(&mut m, &mut r, Control::Cross);
        assert!(acts.contains(&Act::MainMenu), "{to:?}: {acts:?}");
        assert_eq!(m.screen(), Some(Screen::Main), "{to:?}");
        assert_eq!(m.confirming(), None, "{to:?}");
    }

    // The pause's Quit asks the same, worded as the splash's.
    let (mut m, mut r) = fresh();
    m.pause();
    tap_until(&mut m, &mut r, Control::Down, |m| m.pause_rows()[m.cursor(Screen::Pause)] == "Quit");
    let acts = tap(&mut m, &mut r, Control::Cross);
    assert!(acts.iter().all(|a| *a == Act::Stay), "{acts:?}");
    assert_eq!(m.confirming(), Some(("Quit the game?", ["Quit", "Stay"], 1)));
    let acts = tap(&mut m, &mut r, Control::Circle);
    assert!(acts.iter().all(|a| *a == Act::Stay) && m.screen() == Some(Screen::Pause), "{acts:?}");
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Left);
    let acts = tap(&mut m, &mut r, Control::Cross);
    assert!(acts.contains(&Act::Quit), "{acts:?}");

    // A fresh pause has no confirm up from the last one.
    let (mut m, mut r) = fresh();
    m.pause();
    tap_until(&mut m, &mut r, Control::Down, |m| m.pause_rows()[m.cursor(Screen::Pause)] == "Main Menu");
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.confirming().is_some());
    m.pause();
    assert_eq!(m.confirming(), None);
}

/// Over a split, the confirm is the pauser's: 2P's pause hears 2P's pad alone, so 1P's Cross on its
/// leaving row does nothing, and 2P's does — and since P84 that pause is 2P's own, 1P riding on.
#[test]
fn over_a_split_only_the_pausers_pad_answers_the_confirm() {
    // In the ride: a pause is a seat's own since P84, and only a ride has seats to pause.
    let (mut m, mut r) = fresh();
    m.riders = 2;
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.riding());
    let idle = Press::default();
    let (cross, down, up) = (press(|p| p.ok = true), press(|p| p.down = true), press(|p| p.up = true));
    // 2P's Start, as a ride hears it.
    m.pause_for(1);
    assert!(m.paused(1) && !m.paused(0));
    while m.view(1).pause_rows()[m.view(1).cursor(Screen::Pause)] != "Main Menu" {
        m.step_seats(&[idle, down]);
    }
    assert_eq!(m.step_seats(&[idle, cross]), Act::Stay);
    assert_eq!(m.view(1).confirming().map(|c| c.2), Some(1));
    // 1P moves and presses: nothing.
    m.step_seats(&[up, idle]);
    assert_eq!(m.view(1).confirming().map(|c| c.2), Some(1));
    m.step_seats(&[press(|p| p.back = true), idle]);
    assert_eq!(m.view(1).confirming().map(|c| c.2), Some(1));
    // 2P onto the leaving row; 1P's Cross still nothing, 2P's leaves.
    m.step_seats(&[idle, up]);
    assert_eq!(m.step_seats(&[cross, idle]), Act::Stay);
    assert_eq!(m.view(1).screen(), Some(Screen::Pause));
    assert!(m.screen().is_none(), "1P rides on under 2P's pause (P84)");
    assert_eq!(m.step_seats(&[idle, cross]), Act::MainMenu);
    assert_eq!(m.screen(), Some(Screen::Main));
}

/// MD16: the Ride tab's Tricks row, Original unless changed, saved only when it is MX vs. ATV — so a
/// file from before it is written as it was — and greyed with the mods folder off.
#[test]
fn the_tricks_row_picks_mx_vs_atv_and_is_saved() {
    use menu::Tricks;
    let (mut m, mut r) = fresh();
    assert_eq!(m.settings.tricks, Tricks::Original);
    assert!(!m.settings.text().contains("tricks"), "the original's tricks write no line");
    settings_at(&mut m, &mut r, "Ride");
    down_to(&mut m, &mut r, menu::Key::Tricks);
    let row = m.page()[m.cursor(Screen::Settings)].clone();
    assert_eq!(row.value, "Original");
    assert!(row.about.contains("front flip is the port's own"), "{}", row.about);
    tap(&mut m, &mut r, Control::Right);
    assert!(m.take_changed());
    assert_eq!(m.settings.tricks, Tricks::MxVsAtv);
    assert_eq!(m.page()[m.cursor(Screen::Settings)].value, "MX vs. ATV");
    let text = m.settings.text();
    assert!(text.contains("tricks = mxvatv\n"), "{text}");
    assert_eq!(Settings::parse(&text).tricks, Tricks::MxVsAtv);
    assert_eq!(Settings::parse("tricks = unleashed2\n").tricks, Tricks::Original, "a word it does not know");
    for t in Tricks::ALL {
        assert_eq!(Tricks::from_word(t.word()), Some(t));
    }
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.settings.tricks, Tricks::Original, "round to the original's");
    m.settings.mods = false;
    let row = m.page().into_iter().find(|row| row.key == menu::Key::Tricks).unwrap();
    assert!(row.off.is_some(), "greyed with the mods folder off");
}

/// MD30: MD25's 360 Style row is gone. A settings file that still has its `three_sixty` line reads as
/// if it had none, writes it no more, and the Original tab has no 360 row.
#[test]
fn an_old_three_sixty_line_is_read_and_ignored() {
    let old = Settings::parse("tricks = mxvatv\nthree_sixty = mxvatv\n");
    assert_eq!(old.tricks, menu::Tricks::MxVsAtv, "the lines around it still read");
    assert_eq!(old.text(), Settings::parse("tricks = mxvatv\n").text());
    assert!(!old.text().contains("three_sixty"));
    assert!(menu::Departure::ALL.iter().all(|d| !d.label().contains("360")));
}

/// P126: a fresh settings file has every enhancement on and the Original switch off.
#[test]
fn a_fresh_settings_file_has_every_enhancement_on_and_original_off() {
    use menu::{Departure, Original};
    let (m, _) = fresh();
    let empty = Menu::new(Settings::parse(""), Controls::parse(""));
    for m in [&m, &empty] {
        assert_eq!(m.original(), Original::Off);
        for d in Departure::ALL {
            assert!(!m.some_original(d), "{d:?} starts at the original's");
        }
        // Since P121 the ride HUD is the port's own.
        assert_eq!(m.settings.overlay, Overlay::Hud);
    }
    assert_eq!(m.settings.plants, game_render::eco::Lod::Smooth);
    assert_eq!(m.settings.picture, menu::Picture::Fill);
    // Neither new setting writes a line at its default, so an older file is written as it was.
    let text = m.settings.text();
    assert!(!text.contains("plants") && !text.contains("picture"), "{text}");
}

/// P126: the Original tab's switch, by pad, puts every sub-switch at the original's — the controls
/// and cameras of every seat, the fog, the plants and the picture — and back; one sub-switch turned
/// back leaves the others and the switch reads Mixed. Since P170 it leaves the HUD as it is: on still
/// rides openmx's.
#[test]
fn original_on_sets_every_sub_switch_and_one_turned_back_leaves_the_rest() {
    use menu::{Departure, Key, Original};
    let (mut m, mut r) = fresh();
    // A HUD of the user's own, which the switch leaves.
    (m.settings.minimap, m.settings.hud_scale, m.settings.overlay) = (false, 120, Overlay::None);
    settings_at(&mut m, &mut r, "Original");
    let value = |m: &Menu, key: Key| m.page().into_iter().find(|row| row.key == key).unwrap().value;
    assert_eq!(value(&m, Key::Original), "Off");
    assert_eq!(m.page()[m.cursor(Screen::Original)].key, Key::Original, "the page opens on the switch");
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.take_changed());
    assert_eq!(m.original(), Original::On);
    assert_eq!(value(&m, Key::Original), "On");
    let (s, c) = (&m.settings, &m.controls);
    assert_eq!(c.layouts, [Layout::Original; menu::SEATS]);
    assert_eq!(c.planes, [Layout::Original; menu::SEATS]);
    assert_eq!(s.cameras, [menu::CameraView::ORIGINAL; menu::SEATS_MAX]);
    assert_eq!(s.graphics, Graphics::Original);
    assert_eq!(s.plants, game_render::eco::Lod::Original);
    assert_eq!(s.picture, menu::Picture::Television);
    assert_eq!((s.minimap, s.hud_scale, s.overlay), (false, 120, Overlay::None), "the HUD is left as it was");
    // The sim is none of them (P55).
    assert!(s.modifiers.is_original());
    // Each sub-switch on its tab: the graphics ones on Graphics since P128.
    let on_tab = |m: &mut Menu, r: &mut Reader, d: Departure| match menu::SETTINGS_TABS[d.tab()] {
        // the Ride tab's Original Game page (P179)
        "Ride" => tab_to(m, r, "Original"),
        t => tab_to(m, r, t),
    };
    for d in Departure::ALL {
        on_tab(&mut m, &mut r, d);
        assert_eq!(value(&m, d.key()), d.choices()[1], "{d:?}");
    }
    tab_to(&mut m, &mut r, "Original");
    // Each sub-switch turned back on its own leaves every other at the original's.
    for d in Departure::ALL {
        let mut m = m.clone();
        let mut r = r.clone();
        on_tab(&mut m, &mut r, d);
        down_to(&mut m, &mut r, d.key());
        tap(&mut m, &mut r, Control::Right);
        assert!(!m.some_original(d), "{d:?} back to ours");
        // Aspect Ratio's six (P176) step on from 4:3 to 16:9, another of ours.
        let ours = if d == Departure::Picture { "16:9" } else { d.choices()[0] };
        assert_eq!(value(&m, d.key()), ours, "{d:?}");
        tab_to(&mut m, &mut r, "Original");
        for o in Departure::ALL.into_iter().filter(|o| *o != d) {
            assert!(m.is_original(o), "{d:?} turned back moved {o:?}");
        }
        assert_eq!(m.original(), Original::Mixed, "{d:?}");
        assert_eq!(value(&m, Key::Original), "Mixed");
        // The HUD is not read (P170): any HUD settings at all read On.
        let mut h = m.clone();
        h.set_departure(d, true);
        assert_eq!(h.original(), Original::On);
        (h.settings.minimap, h.settings.hud_scale, h.settings.overlay) = (true, 70, Overlay::Hud);
        assert_eq!(h.original(), Original::On);
        // From Mixed the switch's press is On; a second is Off, every enhancement back.
        down_to(&mut m, &mut r, Key::Original);
        tap(&mut m, &mut r, Control::Cross);
        assert_eq!(m.original(), Original::On, "{d:?}");
        tap(&mut m, &mut r, Control::Cross);
        assert_eq!(m.original(), Original::Off, "{d:?}");
        assert_eq!(m.settings.graphics, Graphics::Full);
        assert_eq!(m.controls.layouts, [Layout::New; menu::SEATS]);
        assert_eq!(m.settings.cameras, [menu::CameraView::DEFAULT; menu::SEATS_MAX]);
        // Off leaves the HUD as it was too: the user's own, still.
        assert_eq!((m.settings.minimap, m.settings.hud_scale, m.settings.overlay), (false, 120, Overlay::None));
    }
    // One player on the original's layout is Mixed on the Controls row and on the switch.
    let (mut m, mut r) = fresh();
    m.controls.layouts[1] = Layout::Original;
    assert_eq!(m.original(), Original::Mixed);
    settings_at(&mut m, &mut r, "Original");
    assert_eq!(value(&m, Key::Departure(Departure::Controls)), "Mixed");
    assert_eq!(value(&m, Key::Departure(Departure::Cameras)), "Further Out");
    let mut n = m.clone();
    n.set_departure(Departure::Controls, false);
    assert_eq!(n.original(), Original::Off);
}

/// P179 (2): the Original tab is gone; its rows are a page the Ride tab's Original Game row opens,
/// whose value is the master switch's, and Circle there is back on the Ride tab, on that row.
/// `--screen original` opens the page as the pad reaches it.
#[test]
fn the_original_game_page_is_a_page_of_the_ride_tab() {
    use menu::{Departure, Key, Kind};
    let (mut m, mut r) = fresh();
    assert!(!menu::SETTINGS_TABS.contains(&"Original"));
    settings_at(&mut m, &mut r, "Ride");
    let row = m.page().into_iter().find(|r| r.key == Key::OriginalPage).unwrap();
    assert_eq!((row.kind, row.label.as_str(), row.value.as_str()), (Kind::Opens, "Original Game", "Off"));
    down_to(&mut m, &mut r, Key::OriginalPage);
    let at = m.cursor(Screen::Settings);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Original));
    assert!(m.tabs(Screen::Original).is_empty(), "no tabs of its own");
    let keys: Vec<Key> = m.page().into_iter().filter(|r| r.selectable()).map(|r| r.key).collect();
    let mut want = vec![Key::Original];
    want.extend(Departure::ALL.into_iter().filter(|d| menu::SETTINGS_TABS[d.tab()] == "Ride").map(Key::Departure));
    assert_eq!(keys, want);
    // L1 and R1 do nothing here; Cross the switch.
    tap(&mut m, &mut r, Control::R1);
    assert_eq!(m.screen(), Some(Screen::Original));
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.original(), menu::Original::On);
    tap(&mut m, &mut r, Control::Circle);
    assert_eq!(m.screen(), Some(Screen::Settings));
    assert_eq!((m.tab(Screen::Settings), m.cursor(Screen::Settings)), (0, at), "back on the Ride tab's row");
    assert_eq!(m.page()[at].value, "On");
    m.set_departure(Departure::Markers, false);
    assert_eq!(m.page()[at].value, "Mixed");
    // From the pause's Settings too, and back to the pause by Circle twice.
    let (mut m, mut r) = fresh();
    m.pause();
    tap_until(&mut m, &mut r, Control::Down, |m| m.cursor(Screen::Pause) == 4);
    tap(&mut m, &mut r, Control::Cross);
    tab_to(&mut m, &mut r, "Ride");
    down_to(&mut m, &mut r, Key::OriginalPage);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Original));
    tap(&mut m, &mut r, Control::Circle);
    tap(&mut m, &mut r, Control::Circle);
    assert_eq!(m.screen(), Some(Screen::Pause));
    // `--screen original`.
    assert_eq!(Screen::from_word("original"), Some(Screen::Original));
    let (mut m, mut r) = fresh();
    m.open(Screen::Original);
    assert_eq!(m.screen(), Some(Screen::Original));
    assert_eq!(m.page()[m.cursor(Screen::Original)].key, Key::Original);
    tap(&mut m, &mut r, Control::Circle);
    assert_eq!((m.screen(), m.tab(Screen::Settings)), (Some(Screen::Settings), 0));
}

/// P170: Original Game on from a fresh menu rides openmx's HUD — the switch reads On with the HUD
/// untouched, the file it saves says no other HUD, and its row says the HUD stays.
#[test]
fn original_game_on_still_rides_openmxs_hud() {
    use menu::{Key, Original};
    let (mut m, mut r) = fresh();
    settings_at(&mut m, &mut r, "Original");
    let row = m.page().into_iter().find(|row| row.key == Key::Original).unwrap();
    assert!(row.about.contains("the HUD stays openmx's"), "{}", row.about);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.original(), Original::On);
    let s = &m.settings;
    assert_eq!((s.overlay, s.minimap, s.hud_scale), (Overlay::Hud, true, menu::HUD_SCALE_DEFAULT));
    let back = Menu::new(Settings::parse(&s.text()), m.controls.clone());
    assert_eq!((back.settings.overlay, back.original()), (Overlay::Hud, Original::On));
}

/// P126: settings.txt (and controls.txt for the layouts) round-trips each sub-switch on its own and
/// the switch on, and a word it cannot read keeps ours.
#[test]
fn the_original_switches_round_trip_through_the_files() {
    use menu::{Departure, Original};
    let dir = scratch("original");
    for d in Departure::ALL {
        let (mut m, _) = fresh();
        m.set_departure(d, true);
        menu::save(&dir, &m.settings, &m.controls).unwrap();
        let (s, c) = menu::load(&dir);
        assert_eq!((&s, &c), (&m.settings, &m.controls), "{d:?}");
        let back = Menu::new(s, c);
        for o in Departure::ALL {
            assert_eq!(back.is_original(o), o == d, "{d:?} saved, {o:?} read back");
        }
        assert_eq!(back.original(), Original::Mixed);
    }
    let (mut m, _) = fresh();
    m.set_original(true);
    menu::save(&dir, &m.settings, &m.controls).unwrap();
    let (s, c) = menu::load(&dir);
    assert_eq!(Menu::new(s, c).original(), Original::On);
    assert_eq!(Settings::parse("plants = original\n").plants, game_render::eco::Lod::Original);
    assert_eq!(Settings::parse("plants = fancy\npicture = 3:2\n"), Settings::default());
    std::fs::remove_dir_all(dir).ok();
}

/// V57: V56's cut-out leaves are a departure of their own, Leaves on the Graphics tab beside Plant
/// Detail: ours by default and not in a fresh settings.txt; the Original switch on sets the
/// original's blended leaves, and Leaves turned back by pad leaves every other departure at the
/// original's; only the original's is written to settings.txt, and it reads back.
#[test]
fn original_on_sets_the_leaves_and_turning_them_back_leaves_the_others() {
    use game_render::eco::Foliage;
    use menu::{Departure, Key, Original};
    let (mut m, mut r) = fresh();
    assert_eq!(m.settings.foliage, Foliage::Cutout);
    assert!(!m.settings.text().contains("foliage"), "ours writes no line");
    settings_at(&mut m, &mut r, "Original");
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.original(), Original::On);
    assert_eq!(m.settings.foliage, Foliage::Original);
    assert!(m.settings.text().contains("foliage = original\n"), "{}", m.settings.text());
    let value = |m: &Menu, key: Key| m.page().into_iter().find(|row| row.key == key).unwrap().value;
    tab_to(&mut m, &mut r, "Graphics");
    assert_eq!(value(&m, Key::Departure(Departure::Foliage)), "Original");
    down_to(&mut m, &mut r, Key::Departure(Departure::Foliage));
    tap(&mut m, &mut r, Control::Right);
    assert!(m.take_changed());
    assert_eq!(m.settings.foliage, Foliage::Cutout);
    assert_eq!(value(&m, Key::Departure(Departure::Foliage)), "Crisp");
    for o in Departure::ALL.into_iter().filter(|o| *o != Departure::Foliage) {
        assert!(m.is_original(o), "Leaves turned back moved {o:?}");
    }
    assert_eq!(m.original(), Original::Mixed);
    assert!(!m.settings.text().contains("foliage"), "{}", m.settings.text());
    // The file's word, and one it cannot read keeps ours.
    assert_eq!(Settings::parse("foliage = original\n").foliage, Foliage::Original);
    assert_eq!(Settings::parse("foliage = fuzzy\n"), Settings::default());
}

/// V84: Upscaled Textures is a departure on the Graphics tab, on by default and not in a fresh
/// settings.txt; the Original switch turns it off and writes `textures = original`. The texture
/// packs' order and off list are settings.txt lines alone (opentennis's), read back as written, and a
/// name that would climb out of `mods/` is dropped.
#[test]
fn upscaled_textures_is_a_graphics_departure_and_the_pack_lines_round_trip() {
    use menu::{Departure, Key, Original};
    let (mut m, mut r) = fresh();
    assert!(m.settings.textures);
    assert!(!m.settings.text().contains("texture"), "{}", m.settings.text());
    let value = |m: &Menu, key: Key| m.page().into_iter().find(|row| row.key == key).map(|row| row.value);
    settings_at(&mut m, &mut r, "Original");
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.original(), Original::On);
    assert!(!m.settings.textures);
    assert!(m.settings.text().contains("textures = original\n"), "{}", m.settings.text());
    tab_to(&mut m, &mut r, "Graphics");
    assert_eq!(value(&m, Key::Departure(Departure::Textures)).as_deref(), Some("Original"));
    m.set_departure(Departure::Textures, false);
    assert!(m.settings.textures);
    assert_eq!(value(&m, Key::Departure(Departure::Textures)).as_deref(), Some("Upscaled"));

    let s = Settings::parse("texture_packs = hd_ground, packs/hd_bikes\ntexture_packs_off = old, ../up, a\\b, /abs\n");
    assert_eq!(s.texture_packs, ["hd_ground", "packs/hd_bikes"]);
    assert_eq!(s.texture_packs_off, ["old"]);
    assert!(s.textures);
    assert_eq!(Settings::parse(&s.text()), s);
    assert!(s.text().contains("texture_packs = hd_ground, packs/hd_bikes\ntexture_packs_off = old\n"), "{}", s.text());
}

/// V83: Texture Filtering is a departure with five choices, on the Graphics tab: 16x by default and
/// not in a fresh settings.txt; the Original switch on sets it Off, and stepping it on by pad leaves
/// every other departure at the original's; every choice but 16x is written and reads back.
#[test]
fn texture_filtering_defaults_to_16x_and_the_original_switch_turns_it_off() {
    use menu::{Departure, Filtering, Key, Original};
    let (mut m, mut r) = fresh();
    assert_eq!(m.settings.filtering, Filtering::X16);
    assert_eq!(m.settings.filtering.anisotropy(), game_render::gpu::ANISOTROPY_MAX);
    assert!(!m.settings.text().contains("texture_filtering"), "16x writes no line");
    settings_at(&mut m, &mut r, "Original");
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.original(), Original::On);
    assert_eq!(m.settings.filtering, Filtering::Off);
    assert_eq!(m.settings.filtering.anisotropy(), 1);
    assert!(m.settings.text().contains("texture_filtering = off\n"), "{}", m.settings.text());
    let value = |m: &Menu, key: Key| m.page().into_iter().find(|row| row.key == key).unwrap().value;
    tab_to(&mut m, &mut r, "Graphics");
    assert_eq!(value(&m, Key::TextureFiltering), "Off");
    down_to(&mut m, &mut r, Key::TextureFiltering);
    // Left steps up from Off, through every choice.
    tap(&mut m, &mut r, Control::Left);
    assert!(m.take_changed());
    assert_eq!(m.settings.filtering, Filtering::X2);
    assert_eq!(value(&m, Key::TextureFiltering), "2x");
    for o in Departure::ALL.into_iter().filter(|o| *o != Departure::Filtering) {
        assert!(m.is_original(o), "Texture Filtering stepped moved {o:?}");
    }
    assert_eq!(m.original(), Original::Mixed);
    // Every choice round-trips the file; 16x is the missing line.
    for f in Filtering::ALL {
        let mut s = Settings::default();
        s.filtering = f;
        assert_eq!(Settings::parse(&s.text()).filtering, f, "{}", s.text());
        assert_eq!(s.text().contains("texture_filtering"), f != Filtering::X16);
    }
    // One it cannot read keeps 16x.
    assert_eq!(Settings::parse("texture_filtering = fuzzy\n"), Settings::default());
    // The Original tab's switch back to ours is 16x again.
    m.set_departure(Departure::Filtering, false);
    assert_eq!(m.settings.filtering, Filtering::X16);
}

/// V83: the renderer's samplers are made again under each Texture Filtering choice, and every
/// material's bind group and the ground's with them, clamped to what the device does. A cut-out
/// that reads one mip level (V58) stays at 1: wgpu filters anisotropically only with every filter
/// linear.
#[test]
fn texture_filtering_rebuilds_every_sampler_with_its_clamp() {
    use game_render::gpu::{ANISOTROPY_MAX, GroundChunk, GroundPaint, GroundVertex, Renderer};
    use game_render::mat::{M4, v3};
    use game_render::psg::{Mesh, Model, Node, Strip, Vertex};
    use game_render::world::{Assets, Loaded, Surface};
    use menu::Filtering;
    let mut gpu = match Renderer::headless() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let (now, most) = gpu.anisotropy();
    assert!(most == ANISOTROPY_MAX || most == 1, "{most}");
    assert_eq!(now, most, "a new renderer filters as far as it can");
    // A flat ground, and a model of three materials: repeating, clamped, and a kept-coverage cut-out.
    let cells = 4;
    let vertices = (0..=cells)
        .flat_map(|j| (0..=cells).map(move |i| (i, j)))
        .map(|(i, j)| GroundVertex { pos: [i as f32 * 10.0, 0.0, j as f32 * 10.0], normal: [0.0, 1.0, 0.0] })
        .collect();
    let chunk = GroundChunk { vertices, cells, at: (0, 0), missing: Vec::new(), centre: v3(20.0, 0.0, 20.0), radius: 30.0 };
    gpu.set_ground_painted(&[chunk], GroundPaint::Bare, [0.5; 3]).expect("a bare ground");
    let strip = |x: f32| Strip {
        verts: [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)]
            .iter()
            .map(|&(u, v)| Vertex { pos: v3(x + u, v, 0.0), normal: v3(0.0, 0.0, -1.0), uv: [u, v], color: [128; 4], node: 0, adc: false })
            .collect(),
    };
    let model = Model {
        nodes: vec![Node { name: "root".into(), local: M4::IDENTITY, parent: None, center: v3(0.0, 0.0, 0.0), half_extents: v3(2.0, 1.0, 0.1), radius: 3.0 }],
        materials: vec!["plain".into(), "clamped".into(), "leaves".into()],
        meshes: (0..3).map(|m| Mesh { material: m, group: 0, strips: vec![strip(m as f32)] }).collect(),
        groups: 1,
        group_distance: vec![f32::MAX],
    };
    let surfaces = vec![
        Surface::default(),
        Surface { clamp: [true, false], ..Surface::default() },
        Surface { keep_coverage: true, cutoff: 64, ..Surface::default() },
    ];
    let mut assets = Assets::default();
    assets.textures.push(game_render::Texture { width: 4, height: 4, rgba: vec![200; 64] });
    let tris = model.triangles(0);
    assets.models.insert("three".into(), Loaded { model, tris, textures: vec![Some(0); 3], surfaces, radius: 3.0 });
    gpu.upload_all(&assets);
    assert_eq!(gpu.uploaded().1, 3, "three materials");
    for f in Filtering::ALL.into_iter().rev().chain(Filtering::ALL) {
        gpu.set_anisotropy(f.anisotropy());
        let want = f.anisotropy().min(most);
        assert_eq!(gpu.anisotropy().0, want, "{f:?}");
        // Plain, clamped, the cut-out at 1, then the ground.
        assert_eq!(gpu.bound_anisotropy(), vec![want, want, 1, want], "{f:?}: every bind group made again");
    }
    // A material uploaded under a setting is made with it.
    gpu.set_anisotropy(Filtering::X4.anisotropy());
    gpu.forget_assets();
    gpu.upload_all(&assets);
    assert_eq!(gpu.bound_anisotropy()[0], 4u16.min(most));
}

/// V91: the roads' and ruts' paint targets a freeride ground reads (V67's near and far, group 2 of
/// its pipeline) are read through a sampler of the renderer's own, made again with the ground's and
/// every material's under each Texture Filtering choice: Off is plain trilinear, 16x as far as the
/// device goes.
#[test]
fn texture_filtering_rebuilds_the_road_targets_sampler() {
    use game_render::ground_mesh::{Flat, GroundMeshes};
    use game_render::gpu::{GroundChunk, GroundPaint, GroundVertex, Renderer};
    use game_render::mat::v3;
    use game_render::splat::{Pass, Splat};
    use menu::Filtering;
    let mut gpu = match Renderer::headless() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let most = gpu.anisotropy().1;
    let cells = 4;
    let vertices = (0..=cells)
        .flat_map(|j| (0..=cells).map(move |i| (i, j)))
        .map(|(i, j)| GroundVertex { pos: [i as f32 * 10.0, 0.0, j as f32 * 10.0], normal: [0.0, 1.0, 0.0] })
        .collect();
    let chunk = GroundChunk { vertices, cells, at: (0, 0), missing: Vec::new(), centre: v3(20.0, 0.0, 20.0), radius: 30.0 };
    let layer = || game_render::Texture { width: 2, height: 2, rgba: vec![128; 16] };
    let pass = Pass {
        texture: 0,
        scale: [1.0, 1.0],
        angle: 0.0,
        turn: (0.0, 1.0),
        rgba: [128.0; 4],
        blend: Some([0, 1, 0, 1]),
        fixed: 128.0,
        reads: (true, true),
        mip: None,
    };
    let road = Flat::new([[0.0, 0.0], [40.0, 0.0], [0.0, 40.0]], [[0.0; 2]; 3], [[128.0; 4]; 3], pass.clone());
    let splat = Splat { textures: vec![layer()], passes: vec![pass], shadow: None, meshes: GroundMeshes::of(vec![layer()], vec![road]) };
    // A ground set under a choice is made with it: Off, plain trilinear.
    gpu.set_anisotropy(Filtering::Off.anisotropy());
    gpu.set_ground_painted(&[chunk], GroundPaint::Splat(&splat), [0.5; 3]).expect("a meshed ground");
    assert_eq!(gpu.bound_anisotropy(), vec![1, 1]);
    for f in Filtering::ALL.into_iter().chain(Filtering::ALL.into_iter().rev()) {
        gpu.set_anisotropy(f.anisotropy());
        let want = f.anisotropy().min(most);
        // The ground's group, then its near target's group 2 (which reads the far one too).
        assert_eq!(gpu.bound_anisotropy(), vec![want, want], "{f:?}: the road targets' bind group made again");
    }
}

/// P129: each seat has a Markers row on the Controls tab, On by default, saved in settings.txt; the
/// Original switch turns every seat's off, and the switch's own Markers row reads them all.
#[test]
fn each_seat_has_a_markers_row_saved_in_the_settings() {
    use menu::{Departure, Key, Original};
    let (mut m, mut r) = fresh();
    assert_eq!(m.settings.markers, [true; menu::SEATS_MAX], "on by default");
    assert!(!m.settings.text().contains("markers"), "no line at the default");
    m.seated = 2;
    settings_at(&mut m, &mut r, "Controls");
    let row = |m: &Menu| m.page().into_iter().find(|r| r.key == Key::Markers).unwrap();
    assert_eq!(row(&m).choices, ["On", "Off"]);
    assert_eq!(row(&m).value, "On");
    // 2P's row: Off for 2P alone.
    down_to(&mut m, &mut r, Key::Player);
    tap(&mut m, &mut r, Control::Right);
    down_to(&mut m, &mut r, Key::Markers);
    tap(&mut m, &mut r, Control::Right);
    assert!(m.take_changed());
    assert_eq!(row(&m).value, "Off");
    assert_eq!(m.settings.markers, [true, false, true, true, true, true, true, true]);
    assert_eq!(m.original(), Original::Mixed, "one seat's markers off is a departure turned back");
    assert!(m.some_original(Departure::Markers) && !m.is_original(Departure::Markers));
    // The file: one word a seat as far as it differs, the last for every seat past it.
    let dir = scratch("markers");
    menu::save(&dir, &m.settings, &m.controls).unwrap();
    let (s, _) = menu::load(&dir);
    assert_eq!(s.markers, m.settings.markers);
    assert!(m.settings.text().contains("\nmarkers = on off on\n"), "{}", m.settings.text());
    std::fs::remove_dir_all(dir).ok();
    assert_eq!(Settings::parse("markers = off").markers, [false; menu::SEATS_MAX]);
    assert_eq!(Settings::parse("markers = off,on").markers, [false, true, true, true, true, true, true, true]);
    assert_eq!(Settings::parse("markers = maybe").markers, [true; menu::SEATS_MAX], "a word it cannot read keeps them on");
    // Under Original every seat's are off; back to ours, every seat's on.
    m.set_original(true);
    assert_eq!(m.settings.markers, [false; menu::SEATS_MAX]);
    assert!(m.is_original(Departure::Markers));
    assert_eq!(row(&m).value, "Off");
    m.set_original(false);
    assert_eq!(m.settings.markers, [true; menu::SEATS_MAX]);
}

/// P70: the track pick is a card a track, each with the track's own picture — a disc track's first
/// still out of `UI\trackpreview.res`, the lit one turning through its five — and a mod track's card
/// shows the picture the shell gave it, or none until it has one.
#[test]
fn the_pick_is_a_card_a_track_with_its_own_picture() {
    let mut disc = match game_render::Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let mut art = menu::Art::load(&mut disc).expect("the menus' art");
    let previews = disc.res("UI\\trackpreview.res").expect("the stills");
    let still = |id: &str, i: usize| {
        let bytes = previews.read_suffix(&format!("ui\\tex\\{id}_preview_{i:02}.pst")).unwrap();
        game_render::pst::Texture::parse(&bytes).unwrap()
    };
    // A mod's picture, and a second mod with none yet.
    let picture = game_render::pst::Texture { width: 32, height: 18, rgba: (0..32 * 18 * 4).map(|i| (i * 7 % 256) as u8).collect() };
    art.set_mod_picture("p70_pictured", picture.clone());
    let cards = |art: &menu::Art, m: &Menu, w: usize, h: usize| -> Vec<(String, [f32; 4], [f32; 4], Option<usize>, bool)> {
        let mut frame = game_render::Frame::new(w, h, [90, 90, 90]);
        art.parts(m, &mut frame)
            .into_iter()
            .filter_map(|p| match p {
                menu::Part::Card { id, card, picture, image, lit } => Some((id, card, picture, image, lit)),
                _ => None,
            })
            .collect()
    };
    for (w, h) in [(640, 448), (1920, 1080), (2520, 1080), (360, 640)] {
        let (mut m, mut r) = fresh();
        tap(&mut m, &mut r, Control::Cross);
        assert_eq!(m.screen(), Some(Screen::Tracks));
        let shown = cards(&art, &m, w, h);
        let ids: Vec<&str> = shown.iter().map(|c| c.0.as_str()).collect();
        let mut want: Vec<&str> = menu::TRACKS.map(|t| t.0).to_vec();
        want.push(menu::SPLICED.0);
        assert_eq!(ids, want, "a card for every disc track, then the splice (P149), at {w}x{h}");
        let images = art.images();
        for (i, (id, card, pic, image, lit)) in shown.iter().enumerate() {
            assert_eq!(*lit, i == m.cursor(Screen::Tracks), "{id}");
            let image = image.unwrap_or_else(|| panic!("{id} has no picture"));
            // The splice's first picture is its first map's.
            let first = if id == menu::SPLICED.0 { "free06" } else { id.as_str() };
            assert_eq!(images[image].rgba, still(first, 1).rgba, "{id}'s card shows its first still");
            // The picture on its card, 16:9, and the cards one under the other.
            assert!(pic[0] >= card[0] && pic[1] >= card[1] && pic[0] + pic[2] <= card[0] + card[2] + 0.5 && pic[1] + pic[3] <= card[1] + card[3] + 0.5, "{id} at {w}x{h}");
            assert!((pic[2] / pic[3] - 16.0 / 9.0).abs() < 0.01);
            if i > 0 {
                assert!(card[1] >= shown[i - 1].1[1] + shown[i - 1].1[3], "{id} on the card above at {w}x{h}");
            }
        }
        // The lit card turns through its stills.
        let lit = m.cursor(Screen::Tracks);
        for _ in 0..150 {
            m.step(r.read(&Report::idle()));
        }
        let turned = cards(&art, &m, w, h);
        assert_eq!(images[turned[lit].3.unwrap()].rgba, still(menu::TRACKS[lit].0, 2).rgba);
        assert_eq!(images[turned[(lit + 1) % 5].3.unwrap()].rgba, still(menu::TRACKS[(lit + 1) % 5].0, 1).rgba, "an unlit card stays");
    }
    // The splice's card (P149) turns through each map's stills in turn, west to east.
    let images = art.images();
    for i in 0..10 {
        let map = menu::TRACKS[i % 5].0;
        assert_eq!(images[art.track_image("free_all", i).unwrap()].rgba, still(map, i / 5 + 1).rgba, "picture {i}: {map}");
    }
    // The mods, in a category of their own: the pictured one shows its picture, the other none.
    let (mut m, mut r) = fresh();
    m.set_mod_tracks(vec![("p70_pictured".into(), "Pictured".into()), ("p70_bare".into(), "Bare".into())]);
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Right);
    tap(&mut m, &mut r, Control::Right);
    let shown = cards(&art, &m, 1920, 1080);
    let ids: Vec<&str> = shown.iter().map(|c| c.0.as_str()).collect();
    assert_eq!(ids, ["p70_pictured", "p70_bare"]);
    assert_eq!(art.images()[shown[0].3.unwrap()].rgba, picture.rgba);
    assert_eq!(shown[1].3, None, "no picture yet");
    // A picture the shell takes later is the card's from then on.
    art.set_mod_picture("p70_bare", still("free07", 3));
    let shown = cards(&art, &m, 1920, 1080);
    assert_eq!(art.images()[shown[0].3.unwrap()].rgba, picture.rgba);
    assert_eq!(art.images()[shown[1].3.unwrap()].rgba, still("free07", 3).rgba);
}

/// MD11: with mods from two generated source games, the pick's filters are All Games (every
/// track), MX Unleashed (exactly the disc's), the worlds, and one a game, each listing only its own
/// as its `mod.json`'s source says (not its id's prefix); the filter is kept in the settings, and an
/// online room's pick steps the lit filter's disc tracks.
#[test]
fn the_pick_filters_by_each_mods_source_game_and_keeps_the_filter() {
    let mods = scratch("md11").join("mods");
    let _ = std::fs::remove_dir_all(&mods);
    // Listed as the shell lists them, folder order then the worlds; the games interleave, and one
    // id's prefix names the other game.
    let listed: Vec<(String, String)> = [
        ("gen_a1", "Alpha One", Some("Game Alpha")),
        ("gen_b1", "Beta One", Some("Game Beta")),
        ("gen_a2", "Alpha Two", Some("Game Alpha")),
        ("beta_a3", "Alpha Three", Some("Game Alpha")),
        ("gen_b2", "Beta Two", Some("  Game Beta ")),
        ("world_gen", "Generated World", None),
    ]
    .iter()
    .map(|&(id, name, game)| {
        let source = game.map_or(String::new(), |g| format!(r#""source": {{"game": "{g}"}},"#));
        let dir = mods.join("tracks").join(id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("mod.json"), format!(r#"{{"kind": "track", "id": "{id}", {source} "name": "{name}"}}"#)).unwrap();
        (id.to_string(), name.to_string())
    })
    .collect();
    let ids: Vec<String> = listed.iter().map(|(id, _)| id.clone()).collect();
    let games = track_mods::source_games(&mods, &ids);
    assert_eq!(games.len(), 5, "{games:?}");

    let (mut m, mut r) = fresh();
    m.set_mod_tracks(listed.clone());
    m.set_mod_games(games.clone());
    let groups = m.track_groups();
    let names: Vec<&str> = groups.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["All Games", "MX Unleashed", "Worlds", "Game Alpha", "Game Beta"]);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Tracks));
    // Each filter by the pad: Down walks it round, and what it lists is its own alone.
    let walk = |m: &mut Menu, r: &mut Reader| -> Vec<String> {
        let mut seen = vec![m.track_at(m.cursor(Screen::Tracks)).0.to_string()];
        loop {
            tap(m, r, Control::Down);
            let id = m.track_at(m.cursor(Screen::Tracks)).0.to_string();
            if id == seen[0] {
                return seen;
            }
            seen.push(id);
        }
    };
    let towns: Vec<String> = menu::TRACKS.iter().map(|t| t.0.to_string()).collect();
    let mut disc = towns.clone();
    disc.push(menu::SPLICED.0.to_string());
    let mut every = disc.clone();
    every.extend(["world_gen", "gen_a1", "gen_a2", "beta_a3", "gen_b1", "gen_b2"].map(String::from));
    let mut all = walk(&mut m, &mut r);
    all.sort();
    let mut want = every.clone();
    want.sort();
    assert_eq!(all, want, "All Games lists every track");
    let own: [(&str, Vec<String>); 4] = [
        ("MX Unleashed", disc.clone()),
        ("Worlds", vec!["world_gen".into()]),
        ("Game Alpha", ["gen_a1", "gen_a2", "beta_a3"].map(String::from).to_vec()),
        ("Game Beta", ["gen_b1", "gen_b2"].map(String::from).to_vec()),
    ];
    for (name, tracks) in &own {
        tap(&mut m, &mut r, Control::Right);
        assert_eq!(m.track_groups()[m.track_filter_group()].0, *name);
        assert_eq!(m.settings.track_filter, *name);
        let mut seen = walk(&mut m, &mut r);
        seen.sort();
        let mut tracks = tracks.clone();
        tracks.sort();
        assert_eq!(seen, tracks, "{name} lists only its own");
        assert!(m.track_game(&seen[0]) == *name);
    }
    assert!(m.take_changed());
    // Right from the last is All Games again, which the file does not name.
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.track_filter_group(), 0);
    assert!(!m.settings.text().contains("track_filter"));
    // Kept: ride a Game Beta track, and the next session's pick opens on Game Beta and on it.
    tap(&mut m, &mut r, Control::Left);
    tap(&mut m, &mut r, Control::Down);
    let acts = tap(&mut m, &mut r, Control::Cross);
    assert!(acts.contains(&Act::Ride("gen_b2".into())), "{acts:?}");
    assert!(m.settings.text().contains("track_filter = Game Beta\n"), "{}", m.settings.text());
    let mut next = Menu::new(Settings::parse(&m.settings.text()), Controls::default());
    next.set_mod_tracks(listed.clone());
    next.set_mod_games(games.clone());
    assert_eq!(next.track_groups()[next.track_filter_group()].0, "Game Beta");
    assert_eq!(next.track_at(next.cursor(Screen::Tracks)).0, "gen_b2");
    // Online the room's pick is the filter's disc tracks: the disc's for a mod game (MD6 not yet),
    // and with no mods a kept game's filter is All, which is the disc's.
    assert_eq!(next.online_tracks(), towns.iter().map(String::as_str).collect::<Vec<_>>(), "the splice is not ridden online");
    next.set_mod_tracks(Vec::new());
    assert_eq!(next.track_groups().len(), 1);
    assert_eq!(next.cursor(Screen::Tracks), 0);
    assert_eq!(next.settings.track_filter, "Game Beta", "a game gone from the folder is kept for when it is back");
    // A name the file cannot keep is All Games.
    assert_eq!(Settings::parse("track_filter = a\u{7}b\n").track_filter, "");
    std::fs::remove_dir_all(&mods).unwrap();
}

/// main.rs's `Front::cue`: a menu's cue as the click the original plays for it.
fn click_for(cue: menu::Cue) -> audio::Click {
    match cue {
        menu::Cue::Move => audio::Click::UpDown,
        menu::Cue::Side => audio::Click::LeftRight,
        menu::Cue::Accept => audio::Click::Accept,
        menu::Cue::Back => audio::Click::Cancel,
        menu::Cue::Refused => audio::Click::Locked,
    }
}

/// The front end's sounds as the disc has them in shape (P65): a stereo 32 kHz loop, left 5000 and
/// right -5000, and five short clicks.
fn menu_sounds() -> audio::MenuSounds {
    let mut pcm = Vec::new();
    for _ in 0..32000 {
        pcm.extend([5000i16, -5000]);
    }
    let music = Some(std::sync::Arc::new(audio::Clip { rate: 32000, channels: 2, pcm }));
    let clicks = (1..=5).map(|i| Some(std::sync::Arc::new(audio::Clip { rate: 22050, channels: 1, pcm: vec![100 * i; 2000] }))).collect();
    audio::MenuSounds { music, clicks }
}

#[test]
fn the_menus_play_their_music_click_on_every_move_and_hand_over_to_the_ride() {
    let (mut m, mut r) = fresh();
    let speakers = hum_speakers(&m.settings);
    let sounds = menu_sounds();
    let mixer = speakers.mixer().clone();
    {
        let mut x = mixer.lock().unwrap();
        x.menu_music = sounds.music.clone();
        x.front_end = true;
    }
    let listen = |secs: usize| {
        let mut last = Vec::new();
        for _ in 0..secs * 10 {
            last = hum_listen(&speakers);
        }
        last
    };
    // On the main menu the loop plays alone, under the music's 70; the shuffle's song holds.
    assert_eq!(m.screen(), Some(Screen::Main));
    let heard = listen(2);
    assert!((mixer.lock().unwrap().menu_fade() - 1.0).abs() < 1e-6);
    assert!((heard[100] as f32 - 5000.0 * 0.7).abs() <= 3.0, "the menu's loop, left: {}", heard[100]);
    assert!((heard[101] as f32 + 5000.0 * 0.7).abs() <= 3.0, "the menu's loop, right: {}", heard[101]);

    // Each press its cue, one, and the cue's click is one more voice under Menu FX.
    let press = |m: &mut Menu, r: &mut Reader, c: Control| {
        m.take_cues();
        tap(m, r, c);
        let cues = m.take_cues();
        let mut x = mixer.lock().unwrap();
        let before = x.voices();
        for cue in &cues {
            sounds.play(&mut x, click_for(*cue)).expect("a click for every cue");
        }
        assert_eq!(x.voices(), before + cues.len());
        cues
    };
    assert_eq!(press(&mut m, &mut r, Control::Down), [menu::Cue::Move], "down is one move click");
    assert_eq!(press(&mut m, &mut r, Control::Up), [menu::Cue::Move]);
    assert_eq!(press(&mut m, &mut r, Control::Right), [], "a sideways press means nothing on the main menu, and is silent");
    assert_eq!(press(&mut m, &mut r, Control::Cross), [menu::Cue::Accept], "confirm is its confirm sound");
    assert_eq!(m.screen(), Some(Screen::Tracks));
    assert_eq!(press(&mut m, &mut r, Control::Circle), [menu::Cue::Back], "back is its back sound");
    assert_eq!(m.screen(), Some(Screen::Main));
    assert_eq!(press(&mut m, &mut r, Control::Cross), [menu::Cue::Accept]);
    let cues = press(&mut m, &mut r, Control::Cross);
    assert!(m.riding());
    assert_eq!(cues, [menu::Cue::Accept], "the ride starting is a confirm");
    assert_eq!(press(&mut m, &mut r, Control::Square), [], "a ride is silent");
    assert_eq!(press(&mut m, &mut r, Control::Start), [], "and so is its pause opening");
    assert_eq!(m.screen(), Some(Screen::Pause));
    assert_eq!(press(&mut m, &mut r, Control::Down), [menu::Cue::Move], "the pause clicks (P56)");
    assert_eq!(press(&mut m, &mut r, Control::Circle), [menu::Cue::Back]);
    assert!(m.riding());

    // The ride hands over to its music: the loop fades out over a second and the song comes in.
    mixer.lock().unwrap().front_end = false;
    let heard = listen(2);
    assert_eq!(mixer.lock().unwrap().menu_fade(), 0.0);
    let song = speakers.now().expect("the shuffle's song");
    let level = match song.as_str() {
        "hum:one" => 1000.0,
        "hum:two" => 2000.0,
        _ => 3000.0,
    };
    assert!((heard[100] as f32 - level * 0.7).abs() <= 2.0, "the ride's song: {} for {song}", heard[100]);
    assert!((heard[101] as f32 - level * 0.7).abs() <= 2.0, "the loop is gone on the right: {}", heard[101]);
    // And back in the front end, the loop again.
    mixer.lock().unwrap().front_end = true;
    let heard = listen(2);
    assert!((heard[101] as f32 + 5000.0 * 0.7).abs() <= 3.0, "the loop is back: {}", heard[101]);
}

/// A slider steps with the sideways click, and at its end refuses, once for a held press (P65).
#[test]
fn a_slider_clicks_each_step_and_refuses_at_its_end_once() {
    let (mut m, mut r) = fresh();
    settings_at(&mut m, &mut r, "Audio");
    m.take_cues();
    down_to(&mut m, &mut r, menu::Key::Level(0));
    assert!(m.take_cues().iter().all(|c| *c == menu::Cue::Move), "down the page, each row a move");
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.take_cues(), [menu::Cue::Side]);
    tap_until(&mut m, &mut r, Control::Right, |m| m.settings.music_volume == 100);
    m.take_cues();
    tap(&mut m, &mut r, Control::Right);
    assert_eq!(m.take_cues(), [menu::Cue::Refused], "the slider at its end refuses");
    // A second press well after the first refuses again; held, its repeats are quiet.
    for _ in 0..30 {
        m.step(r.read(&Report::idle()));
    }
    for _ in 0..60 {
        m.step(r.read(&Report::from_buttons(Control::Right.mask())));
    }
    m.step(r.read(&Report::idle()));
    assert_eq!(m.take_cues(), [menu::Cue::Refused], "held, it refuses once, not on every repeat");
    tap(&mut m, &mut r, Control::Left);
    assert_eq!(m.take_cues(), [menu::Cue::Side]);
}

/// A running freeride as main.rs keeps it (P76), by pad input alone: four pad seats, each player
/// reading the one the menu gives it, a controller in no player's seat read for the join, and every
/// per-player list — here each player's ride, garage and Start reader — changed by the one
/// [`join::Change`] the shell's ride loop applies. Each player is known by the pad seat they ride on.
struct DropIn {
    m: Menu,
    /// Player `i`'s presses; the first is the front end's own.
    readers: Vec<Reader>,
    /// Every pad seat's, for the join.
    idle: Vec<Reader>,
    rides: Vec<(usize, game_sim::Sim)>,
    garages: Vec<Garage>,
    lot: vehicle::Lot,
    /// Every player's ride after each tick it was stepped, by pad seat.
    ticks: std::collections::HashMap<usize, Vec<u64>>,
    /// Every frame's pads, and whether the ride ran on it.
    frames: Vec<([Report; 4], bool)>,
    /// The frame each pad seat's player last joined on.
    joined: std::collections::HashMap<usize, usize>,
}

/// Each pad seat's own hands on the gas: the third's stick held over, so a mix-up of whose ride
/// steps on whose pad shows.
fn rider_pad(device: usize) -> Report {
    let mut r = Report::from_buttons(0x0040);
    if device == 2 {
        r.set_axis(2, 0xe0);
    }
    r
}

impl DropIn {
    /// One frame: the menu, a change of players answered, then the ride, on `taps` — the controls
    /// tapped this frame by pad seat. Riding, each player holds their gas.
    fn frame(&mut self, taps: &[(usize, Control)]) -> Act {
        let riding = self.m.riding();
        let devices = self.m.ride_devices();
        let pads: [Report; 4] = std::array::from_fn(|d| {
            let tapped = taps.iter().filter(|(t, _)| *t == d).fold(0, |b, (_, c)| b | c.mask());
            match (riding && devices.contains(&d), tapped) {
                (true, 0) => rider_pad(d),
                (true, t) => Report::from_buttons(rider_pad(d).buttons() | t),
                (false, 0) => Report::idle(),
                (false, t) => Report::from_buttons(t),
            }
        });
        let presses: Vec<Press> = self.readers.iter_mut().zip(&devices).map(|(r, &d)| r.read(&pads[d])).collect();
        let mut act = self.m.step_seats(&presses);
        for (d, r) in self.idle.iter_mut().enumerate() {
            let p = r.read(&pads[d]);
            if act == Act::Stay && !devices.contains(&d) {
                act = self.m.step_idle(d, p);
            }
        }
        let change = match act {
            Act::Join(d) => Some(join::Change::Join(d)),
            Act::Leave(i) => Some(join::Change::Leave(i)),
            _ => None,
        };
        if let Some(change) = change {
            let n = match change {
                join::Change::Join(_) => self.rides.len() + 1,
                join::Change::Leave(_) => self.rides.len() - 1,
            };
            self.lot.resize(n, &mut |_| 0.0);
            let i = self.rides.len();
            // At the spawn, see-through until clear of whoever stands there (P57), with the plane
            // the lot parks for the new seat (P87).
            let mut sim = game_sim::Sim::new(0x88);
            sim.rider.ghost.start();
            let id = match change {
                join::Change::Join(d) => d,
                join::Change::Leave(_) => usize::MAX,
            };
            self.joined.insert(id, self.frames.len());
            let garage = Garage::new(Kind::Bike, self.lot.plane(i, None)).unwrap();
            change.apply(&mut self.rides, || (id, sim));
            change.apply(&mut self.garages, || garage);
            // Player 1's Start is the front end's reader; the others' one each.
            change.apply(&mut self.readers, Reader::new);
            self.m.reseated(change);
        }
        let devices = self.m.ride_devices();
        let ran = self.m.riding();
        if ran {
            for ((id, sim), &d) in self.rides.iter_mut().zip(&devices) {
                assert_eq!(*id, d, "a player is on another's pad");
                step_on(sim, pads[d]);
                self.ticks.entry(*id).or_default().push(game_sim::hash::of(&sim.rider.ride));
            }
        }
        self.frames.push((pads, ran));
        act
    }

    /// One press and release of `control` on pad seat `device`, as [`tap_device`] taps: the acts.
    fn tap(&mut self, device: usize, control: Control) -> Vec<Act> {
        let mut acts = vec![self.frame(&[(device, control)])];
        for _ in 0..3 {
            acts.push(self.frame(&[]));
        }
        acts
    }

    /// The players' pad seats, in player order.
    fn ids(&self) -> Vec<usize> {
        self.rides.iter().map(|(id, _)| *id).collect()
    }

    /// The layout the views are in now, and how many there are: one a player.
    fn views(&self) -> (split::Layout, usize) {
        let layout = self.m.party().map_or(split::Layout::Full, |p| p.layout());
        (layout, layout.rects(640, 448).len().min(self.m.riders))
    }
}

/// One tick of a ride on `pad`, the shell's pass on a flat floor at the 30 Hz clamp.
fn step_on(sim: &mut game_sim::Sim, pad: Report) {
    let dt = ps2_float::F(game_sim::tick::DeltaTime::CAP_30);
    let mut reports = [Report::missing(); game_sim::input::PADS];
    reports[0] = pad;
    let hands = sim.poll(dt, &reports).stick_alone();
    sim.step(dt, &hands, &mut game_sim::world::Flat::default());
}

#[test]
fn players_drop_into_a_running_freeride_and_leave_it_and_the_others_ride_on() {
    // A Free Ride from the main menu by pad seat 0, the shell saying its seats are open.
    let all = [0, 1, 2, 3];
    let (mut m, mut rd) = fresh_with(4);
    let acts = tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Tracks), "{acts:?}");
    let acts = tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
    assert!(acts.iter().any(|a| matches!(a, Act::Ride(_))), "{acts:?}");
    assert!(m.riding() && m.riders == 1);
    m.seats_open = true;
    let first = rd.remove(0);
    let mut lot = vehicle::Lot::new(MACHINES.as_bytes(), 1, &mut |_| 0.0);
    let garage = Garage::new(Kind::Bike, lot.plane(0, None)).unwrap();
    let parked = |lot: &vehicle::Lot, p: usize| lot.bay(vehicle::Machine::Biplane, p).map(|b| b.spawn);
    let first_bay = parked(&lot, 0);
    let mut r = DropIn {
        m,
        readers: vec![first],
        idle: (0..4).map(|_| Reader::new()).collect(),
        rides: vec![(0, game_sim::Sim::new(0x88))],
        garages: vec![garage],
        lot,
        ticks: Default::default(),
        frames: Vec::new(),
        joined: [(0, 0)].into(),
    };
    for _ in 0..20 {
        r.frame(&[]);
    }
    assert_eq!(r.views(), (split::Layout::Full, 1));

    // Pad seat 1's Cross while the ride runs: a second player, at the spawn, see-through, and the
    // screen split in two.
    let acts = r.tap(1, Control::Cross);
    assert!(acts.contains(&Act::Join(1)), "{acts:?}");
    assert!(r.m.riding(), "a join does not stop the ride");
    assert_eq!(r.ids(), vec![0, 1]);
    assert_eq!(r.m.riders, 2);
    assert!(r.rides[1].1.rider.ghost.shown() < 1.0, "the joiner is not see-through");
    assert_eq!(r.views(), (split::Layout::Stacked, 2));
    assert!(r.garages[1].plane.is_some(), "the lot parks the joiner no plane");
    assert_eq!(parked(&r.lot, 0), first_bay, "the first player's plane moved");
    // The same controller again joins nobody: it is riding.
    let acts = r.tap(1, Control::Cross);
    assert!(!acts.iter().any(|a| matches!(a, Act::Join(_))), "{acts:?}");
    for _ in 0..15 {
        r.frame(&[]);
    }

    // Over the pause the pause says how, and pad seat 2's Cross is a third player: quarters.
    r.tap(0, Control::Start);
    assert_eq!(r.m.screen(), Some(Screen::Pause));
    assert!(r.m.pause_rows().iter().any(|row| row == "Leave Session"), "{:?}", r.m.pause_rows());
    let acts = r.tap(2, Control::Cross);
    assert!(acts.contains(&Act::Join(2)), "{acts:?}");
    assert_eq!(r.m.screen(), Some(Screen::Pause), "the pause stays over the ride");
    assert_eq!(r.ids(), vec![0, 1, 2]);
    assert_eq!(r.views(), (split::Layout::Quarters, 3));
    r.tap(0, Control::Start);
    assert!(r.m.riding());
    for _ in 0..30 {
        r.frame(&[]);
    }

    // The second leaves from their own pause: their bike goes, the third moves up, the others
    // ride on, two views again.
    r.tap(1, Control::Start);
    assert_eq!(r.m.view(1).screen(), Some(Screen::Pause), "their own pause (P84)");
    let leave = r.m.view(1).pause_rows().iter().position(|row| row == "Leave Session").unwrap();
    for _ in 0..leave {
        r.tap(1, Control::Down);
    }
    let acts = r.tap(1, Control::Cross);
    assert!(acts.contains(&Act::Leave(1)), "{acts:?}");
    assert!(r.m.riding(), "the ride goes on for the others");
    assert_eq!(r.ids(), vec![0, 2]);
    assert_eq!(r.m.party().map(|p| p.devices.clone()), Some(vec![0, 2]));
    assert_eq!(r.views(), (split::Layout::Stacked, 2));
    assert_eq!(r.m.note.as_deref(), Some("Player 2 left the ride."));
    for _ in 0..30 {
        r.frame(&[]);
    }
    // And the controller that left can come back in, as a new player after the rest.
    let acts = r.tap(1, Control::Cross);
    assert!(acts.contains(&Act::Join(1)), "{acts:?}");
    assert_eq!(r.ids(), vec![0, 2, 1]);
    for _ in 0..10 {
        r.frame(&[]);
    }

    // The staying players rode on tick for tick as they would have alone: each one's ride against
    // one stepped on nothing but its own pad's frames, frozen where the ride was.
    for id in [0, 2] {
        let mut alone = game_sim::Sim::new(0x88);
        if id != 0 {
            alone.rider.ghost.start();
        }
        let mut want = Vec::new();
        for (pads, _) in r.frames[r.joined[&id]..].iter().filter(|f| f.1) {
            step_on(&mut alone, pads[id]);
            want.push(game_sim::hash::of(&alone.rider.ride));
        }
        assert!(want.len() > 40, "player on pad seat {id} rode {} ticks", want.len());
        assert_eq!(r.ticks[&id].len(), want.len(), "player on pad seat {id}");
        for (k, (got, want)) in r.ticks[&id].iter().zip(&want).enumerate() {
            assert_eq!(got, want, "player on pad seat {id}: tick {k} parts from their ride alone");
        }
    }

    // The last two leave; the last one out is asked, and goes back to the main menu.
    let acts = r.tap(2, Control::Start);
    assert!(acts.iter().all(|a| *a == Act::Stay));
    let leave = r.m.view(1).pause_rows().iter().position(|row| row == "Leave Session").unwrap();
    for _ in 0..leave {
        r.tap(2, Control::Down);
    }
    assert!(r.tap(2, Control::Cross).contains(&Act::Leave(1)));
    assert_eq!(r.ids(), vec![0, 1]);
    r.tap(1, Control::Start);
    for _ in 0..leave {
        r.tap(1, Control::Down);
    }
    assert!(r.tap(1, Control::Cross).contains(&Act::Leave(1)));
    assert_eq!(r.ids(), vec![0]);
    assert_eq!(r.views(), (split::Layout::Full, 1));
    r.tap(0, Control::Start);
    for _ in 0..leave {
        r.tap(0, Control::Down);
    }
    let acts = r.tap(0, Control::Cross);
    assert!(acts.iter().all(|a| *a == Act::Stay), "the last one out is asked first: {acts:?}");
    assert!(r.m.confirming().is_some());
    r.tap(0, Control::Up);
    let acts = r.tap(0, Control::Cross);
    assert!(acts.contains(&Act::MainMenu), "{acts:?}");
    assert_eq!(r.m.screen(), Some(Screen::Main));
}

/// A ride of one under way on `menu` (P171), its rider on pad seat `device`, as the shell's loop
/// keeps it: [`DropIn`] over four pad seats, `first` the front end's own reader.
fn alone_on(mut m: Menu, first: Reader, device: usize) -> DropIn {
    assert!(m.riding() && m.riders == 1);
    assert_eq!(m.ride_devices(), [device], "the ride reads its rider's own pad seat");
    m.seats_open = true;
    let mut lot = vehicle::Lot::new(MACHINES.as_bytes(), 1, &mut |_| 0.0);
    let garage = Garage::new(Kind::Bike, lot.plane(0, None)).unwrap();
    DropIn {
        m,
        readers: vec![first],
        idle: (0..4).map(|_| Reader::new()).collect(),
        rides: vec![(device, game_sim::Sim::new(0x88))],
        garages: vec![garage],
        lot,
        ticks: Default::default(),
        frames: Vec::new(),
        joined: [(device, 0)].into(),
    }
}

#[test]
fn a_lone_rider_on_any_pad_seat_is_joined_by_another_and_the_cue_says_how() {
    // A Free Ride from the main menu by pad input alone. The shell puts 1P — a pad, or the keyboard
    // when the splash was taken on it — on pad seat 0, and every other pad behind it in join order
    // (pad.rs's PadSlots, tested there): so a lone rider on the keyboard, on the first pad or on the
    // second is the rider on pad seat 0 here, and the second pad is pad seat 1.
    let all = [0, 1, 2, 3];
    let (mut m, mut rd) = fresh_with(4);
    tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
    let acts = tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
    assert!(acts.iter().any(|a| matches!(a, Act::Ride(_))), "{acts:?}");
    let mut r = alone_on(m, rd.remove(0), 0);
    for _ in 0..10 {
        r.frame(&[]);
    }
    // The cue while a pad seat no player reads has a controller in it, not for the rider's own.
    assert!(r.m.join_cue(&[1]), "a second pad in, a seat free: the cue");
    assert!(!r.m.join_cue(&[]), "no other controller: no cue");
    assert!(!r.m.join_cue(&[0]), "the rider's own pad is not idle");
    let acts = r.tap(1, Control::Cross);
    assert!(acts.contains(&Act::Join(1)), "{acts:?}");
    assert_eq!(r.ids(), vec![0, 1]);
    assert_eq!(r.views(), (split::Layout::Stacked, 2), "the split lays out again for two");
    assert!(!r.m.join_cue(&[]), "seated, the cue is gone");
    assert!(r.m.join_cue(&[2]), "a third pad in: the cue again");
    r.tap(2, Control::Cross);
    r.tap(3, Control::Cross);
    assert_eq!(r.ids(), vec![0, 1, 2, 3]);
    assert!(!r.m.join_cue(&[4]), "full: no cue, whoever else is plugged in");
    // The seats stay open over the pause too, and closed (online, a tape) there is no cue at all.
    r.m.seats_open = false;
    r.tap(3, Control::Circle);
    assert!(!r.m.join_cue(&[4]));

    // A local game of one on pad seat 1 (P171): the host's pad 0 left the join screen, pad 1
    // started alone. Its ride reads pad seat 1, so pad seat 0 is the idle one and joins as 2P, and
    // the rider's own Cross is their throttle, not a join.
    let (mut m, mut rd) = fresh_with(4);
    tap_device(&mut m, &mut rd, &all, 0, Control::Down);
    tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
    tap_device(&mut m, &mut rd, &all, 1, Control::Cross);
    tap_device(&mut m, &mut rd, &all, 0, Control::Circle);
    tap_device(&mut m, &mut rd, &all, 1, Control::Start);
    assert_eq!(m.screen(), Some(Screen::Tracks));
    let acts = tap_device(&mut m, &mut rd, &all, 0, Control::Cross);
    assert!(acts.iter().any(|a| matches!(a, Act::Ride(_))), "{acts:?}");
    let mut r = alone_on(m, rd.remove(0), 1);
    for _ in 0..10 {
        r.frame(&[]);
    }
    assert!(r.m.join_cue(&[0]) && !r.m.join_cue(&[1]));
    let acts = r.tap(1, Control::Cross);
    assert!(!acts.iter().any(|a| matches!(a, Act::Join(_))), "the rider's own pad joined: {acts:?}");
    let acts = r.tap(0, Control::Cross);
    assert!(acts.contains(&Act::Join(0)), "{acts:?}");
    assert_eq!(r.ids(), vec![1, 0], "1P stays on pad seat 1, the joiner rides pad seat 0");
    assert_eq!(r.m.ride_devices(), [1, 0]);
    assert_eq!(r.views(), (split::Layout::Stacked, 2));
    for _ in 0..10 {
        r.frame(&[]);
    }
    // each player's ride stepped on their own pad all along (DropIn::frame asserts it per tick)
    assert!(r.ticks[&0].len() >= 10 && r.ticks[&1].len() >= 20);
}

/// `tests/ride.rs`'s wreck limits, appended to the stand-in bike as `tests/right_stick_camera.rs`
/// does: without a `[Wrecks]` section this bike never gets going.
const PAUSE_RIDE_LIMITS: &str = "
[Wrecks]
MaxWreckTime = 2
CannonballWreckTime = 5
FrontWreckSpeedLimit = 60
BackWreckSpeedLimit = 60
SideWreckSpeedLimit = 40
RacingDownWreckSpeedLimit = 40
FreestyleDownWreckSpeedLimit = 40
GroundPitchDownWreckAngle = 60
GroundPitchUpWreckAngle = 60
AirbornePitchDownWreckAngle = 120
AirbornePitchUpWreckAngle = 120
AirborneRollWreckAngle = 100
NoHanderWreckSpeedLimit = 400
NoHanderMaxRollAngle = 45
JoustWreckHeight = 2
CrossupWreck = 1
CrossupWreckTime = 0.3
";

/// A level floor at height 0, `tests/right_stick_camera.rs`'s.
struct PauseFloor;

impl game_sim::world::World for PauseFloor {
    fn ground(&mut self, at: ps2_float::Vf) -> (ps2_float::Vf, ps2_float::Vf) {
        (ps2_float::Vf::new(at.x(), 0.0, at.z(), 1.0), ps2_float::Vf::new(0.0, 1.0, 0.0, 0.0))
    }

    fn solid(&mut self, _bundle: &game_sim::collide::Bundle, _record: &mut game_sim::collide::Record) -> bool {
        false
    }
}

fn pause_sim(seat: u32) -> game_sim::Sim {
    let tables = std::sync::Arc::new(game_sim::rider::Tables::build());
    let bike = format!("{}{PAUSE_RIDE_LIMITS}", game_sim::stock::BIKE);
    let phy = game_sim::phy::Phy::read_bytes(bike.as_bytes(), &tables.ride.curve);
    game_sim::Sim::ride(seat, tables, phy, game_sim::stock::SPAWN, game_sim::stock::FACING, Default::default())
}

/// One seat's tick the shell's way (main.rs's `Seat::pass`): the pad polled, and the machine
/// stepped only while that player's pause is shut.
fn pause_pass(sim: &mut game_sim::Sim, garage: &mut Garage, report: Report, paused: bool) {
    const DT: ps2_float::F = ps2_float::F(1.0 / 60.0);
    let controls = vehicle::hands(sim.poll(DT, &[report, Report::missing()]), false);
    if !paused {
        garage.pass_marked(sim, DT, &controls, 0, &[], &mut PauseFloor, false);
    }
}

/// P84: in a two-player split 2P's Start pauses 2P alone. 2P's view has the menu, driven by 2P's
/// pad; 1P rides on tick for tick as a lone 1P does; 2P's bike holds still, drawn see-through,
/// until 2P resumes it from where it stopped.
#[test]
fn in_a_split_2p_pauses_alone_and_1p_rides_on_tick_for_tick() {
    let (mut m, r) = fresh();
    m.riders = 2;
    let mut r = r;
    tap(&mut m, &mut r, Control::Cross);
    tap(&mut m, &mut r, Control::Cross);
    assert!(m.riding());
    let mut readers = vec![r, Reader::new()];
    let mut garages = vec![garage(0, Kind::Bike), garage(1, Kind::Bike)];
    let mut sims = [pause_sim(0), pause_sim(1)];
    // A lone 1P beside them on the same pad, the ride 1P's must stay.
    let (mut lone, mut lone_garage) = (pause_sim(0), garage(0, Kind::Bike));
    let gas = Report::from_buttons(Control::Cross.mask());
    let start = Report::from_buttons(Control::Start.mask());
    let down = Report::from_buttons(Control::Down.mask());
    let rects = split::Layout::Stacked.rects(640, 448);

    // 2P's pad over the run: gas, Start, rest with a Down in the menu, Start again, gas.
    let two = |t: usize| match t {
        0..60 => gas,
        60 => start,
        61..80 => Report::idle(),
        80 => down,
        81..120 => Report::idle(),
        120 => start,
        121..125 => Report::idle(),
        _ => gas,
    };
    let mut held = None;
    let mut acts = Vec::new();
    for t in 0..200 {
        let reports = [gas, two(t)];
        acts.push(ride_frame(&mut m, &mut readers, &reports, &mut garages));
        for (seat, sim) in sims.iter_mut().enumerate() {
            pause_pass(sim, &mut garages[seat], reports[seat], m.paused(seat));
        }
        pause_pass(&mut lone, &mut lone_garage, gas, false);

        assert!(!m.paused(0) && m.screen().is_none(), "1P never pauses, tick {t}");
        assert_eq!(sims[0].hash(), lone.hash(), "1P rides on tick for tick, tick {t}");
        let ghost = sims[1].rider.ghost.shown();
        let at = sims[1].rider.ride.position();
        if m.paused(1) {
            assert_eq!(m.view(1).screen(), Some(Screen::Pause));
            assert_eq!(m.pause_views(&rects, 2), vec![(1, rects[1])], "the menu is in 2P's view alone");
            assert_eq!(vehicle::shown_alpha(ghost, true), vehicle::PAUSED_ALPHA);
            let was = *held.get_or_insert((at, sims[1].rider.ride.chassis.speed));
            assert_eq!((at, sims[1].rider.ride.chassis.speed), was, "2P's bike holds still, tick {t}");
        } else {
            assert_eq!(vehicle::shown_alpha(ghost, false), ghost);
        }
        match t {
            59 => assert!(sims[1].rider.ride.chassis.speed.0 > 1.0, "2P is moving when it pauses"),
            60 => assert!(m.paused(1), "2P's Start pauses 2P"),
            79 => assert_eq!(m.view(1).cursor(Screen::Pause), 0),
            80 => assert_eq!(m.view(1).cursor(Screen::Pause), 1, "2P's Down moves 2P's cursor"),
            119 => assert!(m.paused(1)),
            120 => assert!(!m.paused(1) && m.riding(), "2P's Start again resumes"),
            _ => {}
        }
    }
    assert!(acts.contains(&Act::Resume), "{acts:?}");
    // Resumed where it stopped: the first ticks after go on from the held place and speed.
    let (stop, _) = held.unwrap();
    let gone = sims[1].rider.ride.position() - stop;
    assert!(gone.x().abs() + gone.z().abs() > 1.0, "2P rides on after the pause");
}

/// P123: every tab of Settings and the Garage has a `--screen` word, so each gets a `--shot`: the
/// tab's name in lower case opens its screen on it, cursor on the first row and Back to the main
/// menu, as the pad reaches it; the tabs that were screens keep their screen's word.
#[test]
fn every_settings_and_garage_tab_has_a_screen_word() {
    for (screen, tabs) in [(Screen::Settings, &menu::SETTINGS_TABS[..]), (Screen::Garage, &menu::GARAGE_TABS[..])] {
        for (i, name) in tabs.iter().enumerate() {
            let w = name.to_lowercase();
            let (mut m, mut r) = fresh();
            match Screen::from_word(&w) {
                // Graphics, Audio, Controls and Camera: their screens' words open the same tab.
                Some(s) => {
                    assert!(matches!(s, Screen::Graphics | Screen::Audio | Screen::Controls | Screen::Camera), "{w}");
                    m.open(s);
                }
                None => {
                    assert_eq!(Screen::tab_from_word(&w), Some((screen, i)), "{w}");
                    m.open_tab(screen, i);
                }
            }
            assert_eq!(m.screen(), Some(screen), "{w}");
            assert_eq!(m.tabs(screen)[m.tab(screen)], *name, "{w}");
            let rows = m.page();
            assert!(rows[m.cursor(screen)].selectable(), "{w}: the cursor is on a row");
            // Audio's word opens it over the pause, as the pause's Audio row does.
            if w == "audio" {
                continue;
            }
            tap(&mut m, &mut r, Control::Circle);
            assert_eq!(m.screen(), Some(Screen::Main), "{w}: Back is the main menu");
        }
    }
    assert_eq!(Screen::tab_from_word("display"), Some((Screen::Settings, 1)));
    assert_eq!(Screen::tab_from_word("gear"), Some((Screen::Garage, 2)));
    assert_eq!(Screen::tab_from_word("Display"), None);
    assert_eq!(Screen::tab_from_word("splash"), None);
}

/// P172: a seat's pause, drawn into its own view the way the shell's GPU path does (the view's own
/// size, then moved to its place), lies wholly inside that view — its hint tab included, which ran
/// off its view's right edge into the next one — in every split, for every seat, at a 16:9 window
/// and the original's 4:3, both with the pause's own hints and its confirm's.
#[test]
fn a_seats_pause_stays_in_its_own_view() {
    let mut disc = match game_render::Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let art = menu::Art::load(&mut disc).expect("the menus' art");
    let mut checked = 0;
    for (w, h) in [(1920usize, 1080usize), (640, 448)] {
        for players in 2..=split::MOST {
            for s in [split::Split::Stacked, split::Split::SideBySide] {
                let layout = split::Layout::for_players(players, s).unwrap();
                let rects = layout.rects(w, h);
                for seat in 0..players {
                    for confirm in [false, true] {
                        // Into a ride, as 3985's test goes in, and that seat's Start.
                        let (mut m, mut r) = fresh();
                        m.riders = players;
                        tap(&mut m, &mut r, Control::Cross);
                        tap(&mut m, &mut r, Control::Cross);
                        assert!(m.riding());
                        m.tick = 40;
                        m.pause_for(seat);
                        if confirm {
                            // Down to Quit and in: its confirm, whose hints are Stay and Select.
                            let rows = m.view(seat).pause_rows();
                            let quit = rows.iter().position(|r| r.starts_with("Quit")).expect("a Quit row");
                            m.with_seat(seat, |m| {
                                for _ in 0..quit {
                                    m.step(menu::Press { down: true, ..Default::default() });
                                    m.step(menu::Press::default());
                                }
                                m.step(menu::Press { ok: true, ..Default::default() });
                            });
                            assert!(m.view(seat).confirming().is_some(), "{players}P seat {seat}: no confirm");
                        }
                        let views = m.pause_views(&rects, players);
                        assert_eq!(views.iter().map(|v| v.0).collect::<Vec<_>>(), vec![seat]);
                        let at = views[0].1;
                        let mut sprites = Vec::new();
                        m.with_seat(seat, |m| art.sprites(m, (at.w, at.h), 0, &mut sprites));
                        for (k, sprite) in sprites.iter().enumerate() {
                            for c in sprite.corners {
                                let (x, y) = (c[0] + at.x as f32, c[1] + at.y as f32);
                                let inside = x >= at.x as f32 - 0.01
                                    && x <= (at.x + at.w) as f32 + 0.01
                                    && y >= at.y as f32 - 0.01
                                    && y <= (at.y + at.h) as f32 + 0.01;
                                assert!(
                                    inside,
                                    "{w}x{h} {layout:?} {players}P, seat {seat}'s pause{}: sprite {k} of {} has a corner at {x:.1},{y:.1}, outside its view {at:?}",
                                    if confirm { " confirm" } else { "" },
                                    sprites.len()
                                );
                            }
                        }
                        checked += 1;
                    }
                }
            }
        }
    }
    assert!(checked > 100, "{checked} pauses checked");
}

// ---- P165: the same counts at every frame rate ------------------------------------------------------

/// How a run's frames fall (P165): every `1/hz` s, the frame limit Off's uneven ones as fast as they
/// go, or the display's refresh with now and then one missed.
#[derive(Clone, Copy, Debug)]
enum Rate {
    Hz(u64),
    Off,
    Refresh,
}

/// The rates every count is taken at, the front end's 60 first: the one the others must equal.
const RATES: [Rate; 7] = [Rate::Hz(60), Rate::Hz(5), Rate::Hz(30), Rate::Hz(120), Rate::Hz(165), Rate::Off, Rate::Refresh];

impl Rate {
    /// The time after frame `k`, in microseconds.
    fn gap(self, k: u64) -> u64 {
        match self {
            Rate::Hz(hz) => 1_000_000 / hz,
            Rate::Off => 1_000 + k * 7_919 % 11 * 1_000 + k * 31 % 997,
            Rate::Refresh if k % 7 == 6 => 33_333,
            Rate::Refresh => 16_667,
        }
    }
}

/// What a hand pushes with: the Pro Controller's d-pad (its hat) or its left stick, or the
/// keyboard's arrows. L1 and R1 are the pad's shoulders under both of the pad's, A and D on the keys.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Hand {
    Dpad,
    Stick,
    Keys,
}

/// The pushes a hand makes: when each goes down and comes up, in milliseconds from the start — a
/// tap, a short hold, a second's and three seconds' holds, and two taps inside one slow frame.
const GESTURES: [(&str, &[(u64, u64)]); 5] = [
    ("tap", &[(0, 50)]),
    ("short hold", &[(0, 250)]),
    ("1 s hold", &[(0, 1_000)]),
    ("3 s hold", &[(0, 3_000)]),
    ("two taps in 120 ms", &[(0, 40), (80, 120)]),
];

/// The shell's input over a test's pad and keyboard, by a test's clock (P165): what main.rs's
/// front end and ride read each frame, with the events the hand made between frames stamped when
/// it made them.
struct Hands {
    seats: pad::Seats,
    pro: pad::tests::FakePad,
    root: std::path::PathBuf,
    /// The keys down, and pressed since the last frame.
    keys: HashSet<u8>,
    tapped: HashSet<u8>,
    /// The hand's events not yet made: when, in microseconds, what, and whether down.
    events: VecDeque<(u64, Hand, Control, bool)>,
    /// The test's clock, in microseconds.
    now: u64,
}

impl Hands {
    fn new(name: &str) -> Hands {
        let root = pad::tests::scratch(name);
        let (dev, sys) = (root.join("dev"), root.join("sys"));
        std::fs::create_dir_all(&dev).unwrap();
        pad::tests::sys_device(&sys, "js0", "Nintendo Switch Pro Controller", (0x057e, 0x2009), &pad::tests::PRO_KEYS, &pad::tests::PRO_ABS, &[]);
        let mut pro = pad::tests::FakePad::plug(&dev, "js0");
        let mut seats = pad::tests::seats_in(&dev, &sys, pad::Wiring::default());
        // The pad's greeting at the test clock's zero, read then: its clock against the shell's.
        seats.at(0);
        pro.gap(0);
        pro.at(0);
        pro.init(14, &[0; 6]);
        pro.gap(50);
        let got = pad::tests::devices_now(&mut seats, &|_| false);
        let id = match &got[1].0 {
            pad::Device::Pad(id) => id.clone(),
            other => panic!("{other:?}"),
        };
        seats.set_first(Some(pad::Device::Pad(id)));
        Hands { seats, pro, root, keys: HashSet::new(), tapped: HashSet::new(), events: VecDeque::new(), now: 0 }
    }

    /// Make every event up to `now`: the pad's into its device with their own stamps, the keys'
    /// as the window's key moments. Then the frame's key moments.
    fn reach(&mut self, now: u64) -> Vec<window::KeyMoment> {
        self.now = now;
        self.seats.at(now);
        self.tapped.clear();
        let mut moments = Vec::new();
        while self.events.front().is_some_and(|e| e.0 <= now) {
            let (t, hand, control, down) = self.events.pop_front().unwrap();
            match hand {
                Hand::Keys => {
                    let code = match control {
                        Control::Down => window::key::DOWN,
                        Control::Right => window::key::RIGHT,
                        Control::L1 => window::key::A,
                        Control::R1 => window::key::D,
                        Control::Start => window::key::RETURN,
                        other => panic!("no key for {other:?}"),
                    };
                    if down {
                        self.keys.insert(code);
                        self.tapped.insert(code);
                    } else {
                        self.keys.remove(&code);
                    }
                    moments.push((t, code, down));
                }
                Hand::Dpad | Hand::Stick => {
                    self.pro.at((t / 1_000) as u32);
                    let full = if down { i16::MAX } else { 0 };
                    match (hand, control) {
                        (_, Control::L1) => self.pro.button(5, down),
                        (_, Control::R1) => self.pro.button(6, down),
                        (_, Control::Cross) => self.pro.button(0, down),
                        (_, Control::Start) => self.pro.button(10, down),
                        (Hand::Dpad, Control::Down) => self.pro.axis(5, full),
                        (Hand::Dpad, Control::Right) => self.pro.axis(4, full),
                        (Hand::Stick, Control::Down) => self.pro.axis(1, full),
                        (Hand::Stick, Control::Right) => self.pro.axis(0, full),
                        other => panic!("no pad input for {other:?}"),
                    }
                }
            }
        }
        moments
    }

    fn held(&self) -> impl Fn(u8) -> bool + '_ {
        |c| self.keys.contains(&c) || self.tapped.contains(&c)
    }

    /// A front-end frame at `now`, as main.rs reads and steps it: the steps that changed the menu.
    fn front_frame(&mut self, m: &mut Menu, readers: &mut Readers, now: u64) -> usize {
        let moments = self.reach(now);
        self.seats.keys_at(&moments);
        let held = |c: u8| self.keys.contains(&c) || self.tapped.contains(&c);
        let devices = pad::tests::devices_now(&mut self.seats, &held);
        let reports: Vec<(Dev, Report, u16, Option<Vec<menu::Moment>>)> = devices
            .into_iter()
            .map(|(d, r, taps)| {
                let moments = self.seats.moments(&d);
                match d {
                    pad::Device::Keys => (Dev::Keys, r, taps, moments),
                    pad::Device::Pad(id) => (Dev::Pad(id), r, taps, moments),
                }
            })
            .collect();
        let hosting: Vec<bool> = reports.iter().map(|(d, _, _, _)| m.screen() != Some(Screen::Splash) && m.hosts(d)).collect();
        let hosted = |d: &Dev| reports.iter().position(|(r, _, _, _)| r == d).is_some_and(|i| hosting[i]);
        let presses = readers.read_at(&reports, &hosted, now);
        m.elapse_to(now);
        let mut changed = 0;
        let (act, c) = counted(m, |m| m.step_devices(&presses), presses.iter().any(|(_, p)| *p != Press::default()));
        changed += c;
        let mut act = act;
        while act == Act::Stay
            && let Some(more) = readers.more()
        {
            let (a, c) = counted(m, |m| m.step_devices(&more), true);
            (act, changed) = (a, changed + c);
        }
        changed
    }

    /// A ride's tick at `now`, as main.rs reads and steps the pause: 1P's pad and the keyboard as
    /// one. The steps that changed the menu.
    fn ride_tick(&mut self, m: &mut Menu, reader: &mut Reader, now: u64) -> usize {
        let moments = self.reach(now);
        self.seats.keys_at(&moments);
        let held = |c: u8| self.keys.contains(&c) || self.tapped.contains(&c);
        let seats = pad::tests::frame(&mut self.seats, &held);
        let mut devices: Vec<(Report, u16, Option<Vec<menu::Moment>>)> =
            vec![(seats[0].report(), self.seats.taps(0), self.seats.seat_moments(0))];
        if let Some(keys) = self.seats.keyboard_alone() {
            devices.push((keys, 0, self.seats.keyboard_alone_moments()));
        }
        let timed: Vec<menu::Timed> = devices.iter().map(|(r, t, m)| (*r, *t, m.as_deref())).collect();
        let p = reader.read_timed(&timed, now);
        m.elapse_to(now);
        let (mut act, mut changed) = counted(m, |m| m.step_seats(&[p]), p != Press::default());
        while act == Act::Stay
            && let Some(p) = reader.more()
        {
            let (a, c) = counted(m, |m| m.step_seats(&[p]), true);
            (act, changed) = (a, changed + c);
        }
        changed
    }
}

impl Drop for Hands {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Step `m` with `step`, and whether it changed it: only a press can.
fn counted(m: &mut Menu, step: impl FnOnce(&mut Menu) -> Act, pressed: bool) -> (Act, usize) {
    if !pressed {
        return (step(m), 0);
    }
    let before = key(m);
    let act = step(m);
    (act, usize::from(key(m) != before))
}

/// The "Player 1" card stays up its 1.2 s at every frame rate (P165): it counted frames, so at
/// 165 fps it went in under half a second and at 5 fps it stayed fourteen.
#[test]
fn the_player_1_card_lasts_as_long_at_every_frame_rate() {
    let _fifos = pad::tests::FIFOS.lock().unwrap_or_else(|e| e.into_inner());
    let mut hands = Hands::new("menu-card");
    for rate in RATES {
        let (mut m, mut readers) = on_the_splash();
        let start = hands.now + 1_000_000;
        let at = (start + 37_000).div_ceil(1_000) * 1_000;
        hands.events.push_back((at, Hand::Dpad, Control::Cross, true));
        hands.events.push_back((at + 50_000, Hand::Dpad, Control::Cross, false));
        let (mut now, mut k, mut claimed, mut main) = (start, 0, None, None);
        while main.is_none() && now < start + 5_000_000 {
            hands.front_frame(&mut m, &mut readers, now);
            if claimed.is_none() && m.p1().is_some() {
                claimed = Some((now, rate.gap(k)));
            }
            if m.screen() == Some(Screen::Main) {
                main = Some(now);
            }
            now += rate.gap(k);
            k += 1;
        }
        let (seen, gap) = claimed.unwrap_or_else(|| panic!("{rate:?}: nobody took 1P"));
        let up = main.unwrap_or_else(|| panic!("{rate:?}: the card never went")) - seen;
        let card = u64::from(CLAIMED_FRAMES) * 1_000_000 / 60;
        assert!(up + 16_667 >= card && up <= card + gap.max(16_667), "{rate:?}: the card was up {up} µs");
    }
}

/// A second rider's pause over a split (P165): their pad's pushes, read at the ride's ticks inside
/// frames of every rate, move their own pause as far as at 60 fps.
#[test]
fn a_second_seat_s_pause_moves_the_same_at_every_frame_rate() {
    let start_tap = [(Control::Start, 0, 50)];
    for (gesture, pushes) in GESTURES {
        let mut reference = None;
        for rate in RATES {
            let (mut m, _) = riding_at_30();
            let (mut r0, mut r1) = (Reader::at(30), Reader::at(30));
            // Start, then the gesture of Down a second later, by seat 1's hand.
            let mut events: Vec<(u64, u16)> = Vec::new();
            let at = 1_037_000;
            for &(c, down, up) in &start_tap {
                events.push((at + down * 1_000, c.mask()));
                events.push((at + up * 1_000, 0));
            }
            for &(down, up) in pushes {
                events.push((at + 1_000_000 + down * 1_000, Control::Down.mask()));
                events.push((at + 1_000_000 + up * 1_000, 0));
            }
            let end = events.last().unwrap().0 + 600_000;
            let (mut now, mut k, mut ticks, mut since, mut changed) = (0u64, 0, 0, 0u64, 0);
            while now <= end {
                let owed = now * 30 / 1_000_000 + 1;
                while ticks < owed {
                    let moments: Vec<menu::Moment> =
                        events.iter().filter(|e| e.0 > since && e.0 <= now).map(|&(t, b)| (t, Report::from_buttons(b))).collect();
                    let held = events.iter().rev().find(|e| e.0 <= now).map_or(0, |e| e.1);
                    since = now;
                    let p0 = r0.read_timed(&[(Report::idle(), 0, Some(&[]))], now);
                    let p1 = r1.read_timed(&[(Report::from_buttons(held), 0, Some(&moments))], now);
                    m.elapse_to(now);
                    let (mut act, c) = counted(&mut m, |m| m.step_seats(&[p0, p1]), p1 != Press::default());
                    changed += c;
                    while act == Act::Stay {
                        let round = [r0.more(), r1.more()];
                        if round.iter().all(Option::is_none) {
                            break;
                        }
                        let (a, c) = counted(&mut m, |m| m.step_seats(&round.map(Option::unwrap_or_default)), true);
                        (act, changed) = (a, changed + c);
                    }
                    ticks += 1;
                }
                now += rate.gap(k);
                k += 1;
            }
            assert_eq!(m.view(1).screen(), Some(Screen::Pause), "{gesture} at {rate:?}: seat 2's pause is not open");
            let got = (changed, key(&m.view(1)));
            match &reference {
                None => {
                    assert!(got.0 > 0, "{gesture} at {rate:?}: seat 2's pause never moved");
                    reference = Some(got);
                }
                Some((count, state)) => {
                    assert_eq!(got.0, *count, "{gesture} at {rate:?}");
                    assert!(got.1 == *state, "{gesture} at {rate:?} ends elsewhere");
                }
            }
        }
    }
}

/// P178: the Controls tab shows the keyboard's keys only to a player riding the keyboard. A lone 1P
/// on a pad gets the pad, as does every seat while nobody rides the keyboard.
#[test]
fn the_controls_tab_shows_the_keyboard_only_to_its_rider() {
    let (mut m, _) = fresh();
    assert_eq!(m.keys_seat, None);
    assert!(!m.controls_on_keys(), "1P on a pad");
    m.keys_seat = Some(0);
    assert!(m.controls_on_keys(), "1P on the keyboard");
    m.keys_seat = Some(1);
    assert!(!m.controls_on_keys(), "one seat: the keyboard is not 1P's");
    m.seated = 2;
    assert!(!m.controls_on_keys(), "two seats, 1P's rows: 2P rides the keyboard");
    m.keys_seat = Some(0);
    assert!(m.controls_on_keys());
}

/// S161: the Backwards Landings Bail row sits under Air Wrecks on the Game Modifiers page, the
/// world's; off is the default and the original's. On, it is written to the file and read back after
/// a restart, it makes Original mode Mixed, and Original mode's On and the host's All Players reset
/// turn it back off. Dimmed while Wrecks are off, and held where Air Wrecks is.
#[test]
fn the_backwards_bail_row_toggles_and_survives_a_restart() {
    use menu::Key;
    let (mut m, mut r) = fresh();
    assert!(!m.settings.backwards_bail, "off is the default, the original's");
    assert!(!m.settings.text().contains("backwards_bail"), "{}", m.settings.text());
    settings_at(&mut m, &mut r, "Ride");
    down_to(&mut m, &mut r, Key::Modifiers);
    tap(&mut m, &mut r, Control::Cross);
    assert_eq!(m.screen(), Some(Screen::Modifiers));
    down_to(&mut m, &mut r, Key::BackwardsBail);
    let row = |m: &Menu| m.page().into_iter().find(|r| r.key == Key::BackwardsBail).unwrap();
    let keys: Vec<Key> = m.page().iter().map(|r| r.key).collect();
    let at = keys.iter().position(|k| *k == Key::BackwardsBail).unwrap();
    assert_eq!(keys[at - 1], Key::AirWrecks, "under Air Wrecks: {keys:?}");
    assert_eq!(row(&m).value, "Off");
    assert!(row(&m).world);
    tap(&mut m, &mut r, Control::Right);
    assert!(m.settings.backwards_bail);
    assert_eq!(row(&m).value, "On");
    assert!(m.settings.text().contains("backwards_bail = on\n"));
    assert_ne!(m.original(), menu::Original::On, "Backwards Landings Bail on is not the original");

    let dir = scratch("backwards-bail");
    menu::save(&dir, &m.settings, &m.controls).unwrap();
    let (settings, _) = menu::load(&dir);
    assert!(settings.backwards_bail);
    std::fs::remove_dir_all(dir).ok();
    assert!(!Settings::parse("backwards_bail = off\n").backwards_bail);

    m.settings.wrecks = false;
    assert_eq!(row(&m).off.as_deref(), Some("Wrecks are off"));
    m.settings.wrecks = true;
    assert_eq!(row(&m).off, None);

    m.set_original(true);
    assert!(!m.settings.backwards_bail);
    assert_eq!(m.original(), menu::Original::On);

    m.settings.backwards_bail = true;
    down_to(&mut m, &mut r, Key::Player);
    tap_until(&mut m, &mut r, Control::Left, |m| m.mods_whose() == "All");
    down_to(&mut m, &mut r, Key::ResetMods);
    tap(&mut m, &mut r, Control::Cross);
    assert!(!m.settings.backwards_bail, "the host's reset left it on");
}
