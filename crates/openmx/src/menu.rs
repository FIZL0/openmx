//! The front end (P36): a main menu, the free-ride track pick, the settings and the pause in a ride,
//! drawn from the disc's own menu art.
//!
//! The original's front end is not ported and is not meant to be — AGENT.md leaves the menus to us —
//! so this is the port's own, built on opentennis's structure (`crates/hst/src/play/main_menu.rs`): a
//! pure state machine of screens fed one [`Press`] a frame, an on-screen keyboard of four rows of ten,
//! and the settings in `settings.txt` and `controls.txt` beside the disc, `key = value` a line, the way
//! `hst-base`'s `store` keeps them. What changed: opentennis is a Bevy app that runs a match as a
//! child process, and this shell is one window and one loop, so a ride runs in the same process and
//! the menu draws into the same software frame the ride does.
//!
//! What makes it read as MX Unleashed's is the art and the layout, both off the disc: `UI\ui.res`
//! holds the menus' textures (`ui\tex\*`), the Yearling fonts (`font\eng\*.cel` and their atlases)
//! and the layouts that place them (`ui\mainmenu.lyt`, `ui\mxfrontendnavigation.lyt`), and
//! `UI\trackpreview.res` the five stills of every track. The layout numbers below are those files',
//! in the PS2's 640x448 screen, which [`Art::draw`] scales to whatever the frame is.
//!
//! The state machine reads nothing but the pad, so `tests/menu.rs` walks every screen with reports
//! alone, and the drawing is a separate half that needs the disc.
//!
//! The garage (P37) is one more screen: a column per seat, each walked by its own seat's pad through
//! [`roster::Garage`], and the picks kept in `garage.txt` beside the other two files.
//!
//! **Online** (N3) is opentennis's `main_menu/online.rs` flow cut to a ride's needs: Quickplay, Host,
//! Find Games and Join by Code, then the lobby — the players with their riders and bikes, the host's
//! track and wrecks, Ready, Start, chat on the same keyboard as the name — and back to the lobby when
//! a peer drops out of a ride. The menu does not touch the network: what it asks for queues up in
//! [`Lobby::out`] as `openmx_net` commands for the shell to send, and what the network says comes in
//! through [`Menu::heard`], so the tests drive the screens with reports and events alone.
//!
//! A local game (P43) is opentennis's Local (`main_menu/home.rs`, `assign.rs`): Local Game on the main
//! menu opens the join screen ([`join::Join`], P38's rules), where every controller presses its own
//! way into a seat; Start from a seated one goes on to the split layout the views are drawn in, then
//! the track pick, and the ride starts with one view a seat in play ([`Menu::party`]). Online (above) sits
//! beside it on the main menu.

use crate::join::{self, Join, Opened, Outcome};
use crate::roster::{self, Catalogue, Garage, Pick, Picks};
use crate::split::{self, Split};
use game_render::{Disc, Frame, Texture};
use game_sim::input::{AXIS_CENTRE, Control, Report};
use openmx_net::{Cmd, Event, HOST, Listing, Room};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ---- the pad -------------------------------------------------------------------------------------

/// One frame of menu input: edges, never holds, with the directions auto-repeating the way a held
/// d-pad walks a list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Press {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    /// Cross.
    pub ok: bool,
    /// Circle or Triangle — and Escape on the keyboard, which the shell adds itself.
    pub back: bool,
    /// Square: the on-screen keyboard's delete.
    pub delete: bool,
    pub start: bool,
}

impl Press {
    /// Both devices' presses as one, for the menus' two drivers (P46).
    pub fn or(self, o: Press) -> Press {
        Press {
            up: self.up || o.up,
            down: self.down || o.down,
            left: self.left || o.left,
            right: self.right || o.right,
            ok: self.ok || o.ok,
            back: self.back || o.back,
            delete: self.delete || o.delete,
            start: self.start || o.start,
        }
    }
}

/// How far the left stick has to lean before it counts as a direction, out of 127.
const STICK_DIRECTION: i32 = 64;
/// Frames a direction is held before it repeats, and then frames between repeats.
const REPEAT_AFTER: u32 = 20;
const REPEAT_EVERY: u32 = 5;

/// Turns a report a frame into [`Press`]es.
///
/// The first report it sees is taken as already held, so the Cross that chose a track, or the Start
/// that paused, does not press again on the screen it opened.
#[derive(Clone, Debug, Default)]
pub struct Reader {
    last: Option<u16>,
    held_for: u32,
}

/// A direction from the stick, as the d-pad bit it stands in for.
fn stick(r: &Report) -> u16 {
    let lean = |i: usize| r.axis(i) as i32 - AXIS_CENTRE as i32;
    let (x, y) = (lean(2), lean(3));
    let mut m = 0;
    if x <= -STICK_DIRECTION {
        m |= Control::Left.mask();
    }
    if x >= STICK_DIRECTION {
        m |= Control::Right.mask();
    }
    if y <= -STICK_DIRECTION {
        m |= Control::Up.mask();
    }
    if y >= STICK_DIRECTION {
        m |= Control::Down.mask();
    }
    m
}

impl Reader {
    pub fn new() -> Reader {
        Reader::default()
    }

    /// Forget what was held, so the next report is the first again.
    pub fn reset(&mut self) {
        *self = Reader::default();
    }

    pub fn read(&mut self, r: &Report) -> Press {
        self.read_taps(r, 0)
    }

    /// [`Reader::read`], with the controls the pad layer saw go down since the last report (P53:
    /// `pad.rs`'s `Pads::taps`). Each of those is a press whatever the two reports say: a button
    /// let go and pressed again between two frames is down in both, and a slow frame — 5 fps at 4K
    /// — is longer than a tap. Not on the first report: that one is what was already held.
    pub fn read_taps(&mut self, r: &Report, taps: u16) -> Press {
        // A pad that is not there holds nothing.
        let now = if r.status() == 0xff { 0 } else { r.buttons() | stick(r) };
        let Some(last) = self.last.replace(now) else {
            return Press::default();
        };
        let dirs = Control::Up.mask() | Control::Down.mask() | Control::Left.mask() | Control::Right.mask();
        let mut edge = now & !last | taps;
        if now & dirs != 0 && now & dirs == last & dirs {
            self.held_for += 1;
            if self.held_for >= REPEAT_AFTER && (self.held_for - REPEAT_AFTER) % REPEAT_EVERY == 0 {
                edge |= now & dirs;
            }
        } else {
            self.held_for = 0;
        }
        let on = |c: Control| edge & c.mask() != 0;
        Press {
            up: on(Control::Up),
            down: on(Control::Down),
            left: on(Control::Left),
            right: on(Control::Right),
            ok: on(Control::Cross),
            back: on(Control::Circle) || on(Control::Triangle),
            delete: on(Control::Square),
            start: on(Control::Start),
        }
    }
}

// ---- the settings files --------------------------------------------------------------------------

/// The free-ride tracks, by archive and by the town the track is named for.
pub const TRACKS: [(&str, &str); 5] = [
    ("free06", "Dallas"),
    ("free07", "Norfolk"),
    ("free09", "Bridgeport"),
    ("free10", "Rochester"),
    ("free11", "Cleveland"),
];

/// `settings.txt`: the rider and the ride.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// The rider's name, typed on the on-screen keyboard.
    pub name: String,
    /// Which overlay a ride draws (V26), `--overlay`'s word; H toggles the debug one in a ride.
    pub overlay: Overlay,
    /// Whether a ride wrecks at all.
    pub wrecks: bool,
    /// The last track ridden, which the track pick opens on.
    pub track: String,
    /// The machine each seat's ride starts on, the ride setup's pick (P41), 1P's first.
    pub machines: [Machine; SEATS_MAX],
    /// How a local game of two shares the screen, the split layout's pick (P43): `--split`'s.
    pub split: Split,
    /// The music (P42): `off`, `shuffle`, or one song's id (`disc:headstrong`, `folder:mine.wav`).
    pub music: String,
    /// The two volume sliders, `0..=VOLUME_MAX`.
    pub music_volume: u8,
    pub effects_volume: u8,
}

/// The volume sliders' top step.
pub const VOLUME_MAX: u8 = 10;

/// The most players a ride seats, as P35's seats count them.
pub const SEATS_MAX: usize = 8;

/// The machines a player can be on: the bike, or the freeride's biplane (P19). The shell's
/// `vehicle::Kind` is the same two; this one is the menu's, which knows words and labels and nothing
/// of the simulation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Machine {
    #[default]
    Bike,
    Biplane,
}

impl Machine {
    pub const ALL: [Machine; 2] = [Machine::Bike, Machine::Biplane];

    /// The file's word, which is also `--vehicle`'s.
    pub fn word(self) -> &'static str {
        match self {
            Machine::Bike => "bike",
            Machine::Biplane => "biplane",
        }
    }

    pub fn from_word(w: &str) -> Option<Machine> {
        Machine::ALL.into_iter().find(|m| m.word() == w)
    }

    pub fn label(self) -> &'static str {
        match self {
            Machine::Bike => "Bike",
            Machine::Biplane => "Biplane",
        }
    }

    pub fn other(self) -> Machine {
        match self {
            Machine::Bike => Machine::Biplane,
            Machine::Biplane => Machine::Bike,
        }
    }
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            name: "Rider".into(),
            overlay: Overlay::Original,
            wrecks: true,
            track: TRACKS[0].0.into(),
            machines: [Machine::Bike; SEATS_MAX],
            split: Split::default(),
            music: "shuffle".into(),
            music_volume: 7,
            effects_volume: 8,
        }
    }
}

/// `--split`'s word for a layout of two, as `settings.txt` keeps it.
fn split_word(s: Split) -> &'static str {
    match s {
        Split::Stacked => "h",
        Split::SideBySide => "v",
    }
}

/// The longest name the keyboard takes.
pub const NAME_MAX: usize = 12;

/// `key = value` pairs of a settings file: `#` starts a comment, and a line without `=` is skipped.
fn pairs(text: &str) -> impl Iterator<Item = (&str, &str)> {
    text.lines()
        .filter_map(|l| l.split('#').next()?.split_once('='))
        .map(|(k, v)| (k.trim(), v.trim()))
}

fn on_off(v: bool) -> &'static str {
    if v { "on" } else { "off" }
}

impl Settings {
    /// Read a file's text. Keys it does not know, and values it cannot read, keep their defaults, so
    /// an older or hand-edited file still opens.
    pub fn parse(text: &str) -> Settings {
        let mut s = Settings::default();
        for (k, v) in pairs(text) {
            match k {
                "name" => {
                    let name = clean_name(v);
                    if !name.is_empty() {
                        s.name = name;
                    }
                }
                "overlay" => s.overlay = Overlay::from_word(v).unwrap_or(s.overlay),
                "wrecks" => s.wrecks = v != "off",
                "track" if TRACKS.iter().any(|t| t.0 == v) => s.track = v.to_string(),
                // One word a seat, 1P's first; a seat the line does not reach rides the bike.
                "machines" => {
                    for (i, w) in v.split_whitespace().take(SEATS_MAX).enumerate() {
                        if let Some(m) = Machine::from_word(w) {
                            s.machines[i] = m;
                        }
                    }
                }
                "split" => s.split = Split::parse(v).unwrap_or(s.split),
                "music" if !v.is_empty() => s.music = v.to_string(),
                "music_volume" => s.music_volume = v.parse().map_or(s.music_volume, |n: u8| n.min(VOLUME_MAX)),
                "effects_volume" => s.effects_volume = v.parse().map_or(s.effects_volume, |n: u8| n.min(VOLUME_MAX)),
                _ => {}
            }
        }
        s
    }

    pub fn text(&self) -> String {
        format!(
            "# openmx's front end (P36)\nname = {}\noverlay = {}\nwrecks = {}\ntrack = {}\nmachines = {}\nsplit = {}\nmusic = {}\nmusic_volume = {}\neffects_volume = {}\n",
            self.name,
            self.overlay.word(),
            on_off(self.wrecks),
            self.track,
            self.machines.map(Machine::word).join(" "),
            split_word(self.split),
            self.music,
            self.music_volume,
            self.effects_volume
        )
    }

    /// The track pick's index of [`Settings::track`].
    pub fn track_index(&self) -> usize {
        TRACKS.iter().position(|t| t.0 == self.track).unwrap_or(0)
    }
}

/// What a name may hold: the keyboard's own characters, at most [`NAME_MAX`] of them.
fn clean_name(v: &str) -> String {
    v.chars()
        .filter(|c| KEY_ROWS.iter().any(|row| row.contains(*c)) || *c == ' ' || c.is_ascii_uppercase())
        .take(NAME_MAX)
        .collect::<String>()
        .trim()
        .to_string()
}

/// Which overlay a ride draws over each view, as `--overlay` spells it (V26).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Overlay {
    /// The original's own HUD: its speedometer and its radar, off the disc (`overlay.rs`).
    #[default]
    Original,
    /// The shell's debug HUD (`hud.rs`).
    Debug,
    /// Nothing over the picture.
    None,
}

impl Overlay {
    pub const ALL: [Overlay; 3] = [Overlay::Original, Overlay::Debug, Overlay::None];

    /// The file's word, which is also `--overlay`'s.
    pub fn word(self) -> &'static str {
        match self {
            Overlay::Original => "original",
            Overlay::Debug => "debug",
            Overlay::None => "none",
        }
    }

    /// A word back, and the two P36 wrote before there were three: `on` was the debug HUD, and `off`
    /// was no debug HUD, which is now the original's.
    pub fn from_word(w: &str) -> Option<Overlay> {
        match w {
            "on" => Some(Overlay::Debug),
            "off" => Some(Overlay::Original),
            _ => Overlay::ALL.into_iter().find(|o| o.word() == w),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Overlay::Original => "Original",
            Overlay::Debug => "Debug",
            Overlay::None => "None",
        }
    }
}

/// Where the throttle comes from, as `--throttle` spells it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Gas {
    #[default]
    Cross,
    R2,
    L2,
}

impl Gas {
    pub const ALL: [Gas; 3] = [Gas::Cross, Gas::R2, Gas::L2];

    /// The file's word, which is also `--throttle`'s.
    pub fn word(self) -> &'static str {
        match self {
            Gas::Cross => "cross",
            Gas::R2 => "r2",
            Gas::L2 => "l2",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Gas::Cross => "Cross",
            Gas::R2 => "R2",
            Gas::L2 => "L2",
        }
    }
}

/// Which controls a seat rides on (P40), as `--controls` spells it: the shell's `pad::Scheme`, which
/// this file cannot name because `tests/menu.rs` compiles it on its own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layout {
    /// R2 the gas, L2 the brake, Select the camera; Cross and Square kept free.
    #[default]
    New,
    /// The original's: Cross the gas, Square the brake, L1 the camera.
    Original,
}

impl Layout {
    pub const ALL: [Layout; 2] = [Layout::New, Layout::Original];

    pub fn word(self) -> &'static str {
        match self {
            Layout::New => "new",
            Layout::Original => "original",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Layout::New => "New",
            Layout::Original => "Original",
        }
    }
}

/// How many seats the file holds a layout for: the shell's `pad::SEATS`.
pub const SEATS: usize = 8;

/// The seats the Controls screen has a row for; the rest are the file's.
pub const LAYOUT_ROWS: usize = 4;

/// `controls.txt`: how the host's pad is wired, which is the device's and not the rider's, and which
/// layout each seat rides on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Controls {
    pub throttle: Gas,
    /// `--pad`'s device, or none for the first one found.
    pub pad: Option<String>,
    /// `--pad-pressure`'s map of axes to controls, as its text; empty for none.
    pub pressure: String,
    /// `--controls`' layout per seat, 1P first.
    pub layouts: [Layout; SEATS],
    /// A Nintendo pad's A and B by position (`nintendo = positions`): B, the bottom one, confirms.
    /// The default (`labels`) has A confirm and B go back, as on a Switch (P53).
    pub nintendo_positions: bool,
}

impl Controls {
    pub fn parse(text: &str) -> Controls {
        let mut c = Controls::default();
        for (k, v) in pairs(text) {
            match k {
                "throttle" => {
                    if let Some(g) = Gas::ALL.into_iter().find(|g| g.word() == v) {
                        c.throttle = g;
                    }
                }
                "pad" => c.pad = (!v.is_empty() && v != "auto").then(|| v.to_string()),
                "pressure" => c.pressure = if v == "none" { String::new() } else { v.to_string() },
                "nintendo" => c.nintendo_positions = v == "positions",
                // One word for every seat, or one per seat with the last carried on.
                "layout" => {
                    let words: Vec<Option<Layout>> = v
                        .split(',')
                        .map(|w| Layout::ALL.into_iter().find(|l| l.word() == w.trim()))
                        .collect();
                    if words.iter().all(Option::is_some) && words.len() <= SEATS {
                        let mut last = Layout::default();
                        for (i, slot) in c.layouts.iter_mut().enumerate() {
                            last = words.get(i).copied().flatten().unwrap_or(last);
                            *slot = last;
                        }
                    }
                }
                _ => {}
            }
        }
        c
    }

    pub fn text(&self) -> String {
        format!(
            "# openmx's pad wiring (P36): throttle is cross, r2 or l2; pad is a device or auto;\n\
             # pressure is --pad-pressure's axis:control list, or none; layout is new or original,\n\
             # or one per seat (new,original,...) — new is R2 gas, L2 brake, Select camera, with\n\
             # Cross and Square kept free; original is the game's own (P40); nintendo is labels\n\
             # (a Switch pad's A confirms and B goes back) or positions (B, the bottom one, confirms)\n\
             throttle = {}\npad = {}\npressure = {}\nlayout = {}\nnintendo = {}\n",
            self.throttle.word(),
            self.pad.as_deref().unwrap_or("auto"),
            if self.pressure.is_empty() { "none" } else { &self.pressure },
            self.layout_text(),
            if self.nintendo_positions { "positions" } else { "labels" },
        )
    }

    /// The `layout` line's value: one word when every seat agrees.
    pub fn layout_text(&self) -> String {
        if self.layouts.iter().all(|l| *l == self.layouts[0]) {
            return self.layouts[0].word().to_string();
        }
        self.layouts.iter().map(|l| l.word()).collect::<Vec<_>>().join(",")
    }
}

/// Where the two files live: beside the disc, as opentennis keeps its own, or here when there is no
/// disc path to be beside.
pub fn store_dir(iso: Option<&Path>) -> PathBuf {
    iso.and_then(|p| p.parent())
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .to_path_buf()
}

pub const SETTINGS_FILE: &str = "settings.txt";
pub const CONTROLS_FILE: &str = "controls.txt";

/// Both files out of `dir`; a missing file is the defaults.
pub fn load(dir: &Path) -> (Settings, Controls) {
    let read = |name| std::fs::read_to_string(dir.join(name)).unwrap_or_default();
    (Settings::parse(&read(SETTINGS_FILE)), Controls::parse(&read(CONTROLS_FILE)))
}

/// Both files into `dir`.
pub fn save(dir: &Path, settings: &Settings, controls: &Controls) -> Result<(), String> {
    for (name, text) in [(SETTINGS_FILE, settings.text()), (CONTROLS_FILE, controls.text())] {
        let path = dir.join(name);
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

// ---- the screens ---------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// Press start (P46): whichever device presses Cross or Start becomes 1P.
    Splash,
    Main,
    Tracks,
    Settings,
    Controls,
    Keyboard,
    Pause,
    /// Quickplay, Host, Find Games, Join by Code (N3).
    Online,
    /// The lobbies Find Games turned up.
    Games,
    /// The lobby: its players, the ride, Ready and Start.
    Lobby,
    Garage,
    /// A local game's join screen (P43 over P38's [`join`]).
    Join,
    /// A local game's split layout.
    Split,
}

impl Screen {
    pub const ALL: [Screen; 13] = [
        Screen::Splash,
        Screen::Main,
        Screen::Tracks,
        Screen::Garage,
        Screen::Join,
        Screen::Split,
        Screen::Settings,
        Screen::Controls,
        Screen::Keyboard,
        Screen::Pause,
        Screen::Online,
        Screen::Games,
        Screen::Lobby,
    ];

    fn index(self) -> usize {
        Screen::ALL.iter().position(|s| *s == self).unwrap()
    }

    /// `--screen`'s word.
    pub fn word(self) -> &'static str {
        match self {
            Screen::Splash => "splash",
            Screen::Main => "main",
            Screen::Tracks => "tracks",
            Screen::Settings => "settings",
            Screen::Controls => "controls",
            Screen::Keyboard => "keyboard",
            Screen::Pause => "pause",
            Screen::Garage => "garage",
            Screen::Online => "online",
            Screen::Games => "games",
            Screen::Lobby => "lobby",
            Screen::Join => "join",
            Screen::Split => "split",
        }
    }

    pub fn from_word(w: &str) -> Option<Screen> {
        Screen::ALL.into_iter().find(|s| s.word() == w)
    }

    fn title(self) -> &'static str {
        match self {
            Screen::Splash => "",
            Screen::Main => "Main Menu",
            Screen::Tracks => "Free Ride",
            Screen::Settings => "Settings",
            Screen::Controls => "Controls",
            Screen::Keyboard => "Rider Name",
            Screen::Pause => "Paused",
            Screen::Garage => "Garage",
            Screen::Online => "Online",
            Screen::Games => "Find Games",
            Screen::Lobby => "Lobby",
            Screen::Join => "Local Game",
            Screen::Split => "Split Screen",
        }
    }
}

/// The main menu's rows: opentennis's Local and Online beside Free Ride, as `home.rs` lists them.
const MAIN: [&str; 6] = ["Free Ride", "Local Game", "Online", "Garage", "Settings", "Quit"];
const MAIN_FREE: usize = 0;
const MAIN_LOCAL: usize = 1;
const MAIN_ONLINE: usize = 2;
const MAIN_GARAGE: usize = 3;
const MAIN_SETTINGS: usize = 4;
/// The pause's rows. The third is the original pause menu's "Switch To Machine: %s", with the
/// machine a switch would go to.
const PAUSE: [&str; 6] = ["Resume", "Restart", "Switch To Machine", "Settings", "Main Menu", "Quit"];
const PAUSE_SWITCH: usize = 2;
const SETTINGS_ROWS: usize = 7;
/// The settings' sound rows (P42), after the four P36 shipped.
const SETTINGS_MUSIC: usize = 4;
const SETTINGS_MUSIC_VOLUME: usize = 5;
const SETTINGS_EFFECTS_VOLUME: usize = 6;
const CONTROLS_ROWS: usize = 1 + LAYOUT_ROWS;

/// The on-screen keyboard: opentennis's four rows of ten, then a row of four commands.
const KEY_ROWS: [&str; 4] = ["abcdefghij", "klmnopqrst", "uvwxyz0123", "456789-'.&"];
const KEY_COMMANDS: [&str; 4] = ["Space", "Delete", "Clear", "Done"];

/// A device that can press a menu (P46): the keyboard, or a pad by the name the seats know it by across an unplug (`pad.rs`'s `DeviceInfo::identity`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Dev {
    Keys,
    Pad(String),
}

/// A [`Reader`] per device, so each one's presses are its own edges: a second pad's held stick is
/// not the first one's press, and a pad plugged in holding a button does not press it.
#[derive(Clone, Debug, Default)]
pub struct Readers(Vec<(Dev, Reader)>);

impl Readers {
    pub fn new() -> Readers {
        Readers::default()
    }

    /// Forget every device's last report, so the next frame's are all first ones again.
    pub fn reset(&mut self) {
        self.0.clear();
    }

    /// This frame's reports, a device each, as presses. A device missing from them is forgotten.
    pub fn read(&mut self, reports: &[(Dev, Report)]) -> Vec<(Dev, Press)> {
        let with: Vec<(Dev, Report, u16)> = reports.iter().map(|(d, r)| (d.clone(), *r, 0)).collect();
        self.read_taps(&with)
    }

    /// [`Readers::read`] with each device's presses since the last frame ([`Reader::read_taps`]).
    pub fn read_taps(&mut self, reports: &[(Dev, Report, u16)]) -> Vec<(Dev, Press)> {
        self.0.retain(|(d, _)| reports.iter().any(|(r, _, _)| r == d));
        reports
            .iter()
            .map(|(dev, report, taps)| {
                let at = match self.0.iter().position(|(d, _)| d == dev) {
                    Some(at) => at,
                    None => {
                        self.0.push((dev.clone(), Reader::new()));
                        self.0.len() - 1
                    }
                };
                (dev.clone(), self.0[at].1.read_taps(report, *taps))
            })
            .collect()
    }
}

/// Device names on one line of the splash: the first three, and how many more.
fn names_line(names: &[String]) -> String {
    let shown: Vec<&str> = names.iter().take(3).map(String::as_str).collect();
    match names.len() {
        n if n > 3 => format!("{}, +{} more", shown.join(", "), n - 3),
        _ => shown.join(", "),
    }
}

/// Frames the "Player 1" card stays up after the press, before the main menu: opentennis's 1.2 s.
pub const CLAIMED_FRAMES: u32 = 72;

/// The splash's Quit popup's rows.
const QUIT_ROWS: [&str; 2] = ["Quit", "Stay"];

/// What a frame of the menu asks of the shell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    Stay,
    /// Start a ride on this track.
    Ride(String),
    /// Close the pause and carry on.
    Resume,
    /// Ride the same track again from its start.
    Restart,
    /// Leave the ride for the main menu.
    MainMenu,
    /// Put this seat's player on their other machine, and carry on (P41). The shell answers with
    /// [`Menu::switched`].
    Switch(usize),
    Quit,
    /// The lobby's ride starts: [`Lobby::start`] says on what.
    Online,
}

/// The front end's state: which screens are open, where each one's cursor is, and the settings it
/// edits.
#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
    pub settings: Settings,
    pub controls: Controls,
    /// Open screens, the one shown last. Empty is a ride with nothing over it.
    stack: Vec<Screen>,
    /// Each screen's row, in [`Screen::ALL`]'s order.
    cursor: [usize; Screen::ALL.len()],
    /// The keyboard's text while it is being typed, what it is for, and its cursor (row, column).
    typing: String,
    typing_for: Typing,
    key: (usize, usize),
    /// Frames shown, for the track stills' slideshow.
    pub tick: u32,
    /// The settings changed since [`Menu::take_changed`] last said so.
    changed: bool,
    /// A line for the ticker, which is how a ride that could not start says why.
    pub note: Option<String>,
    /// The ride under the menu, as the shell tells it (P41): how many players ride, the machine each
    /// one is on now, and which of them have a machine to switch to.
    pub riders: usize,
    pub up: [Machine; SEATS_MAX],
    pub switchable: [bool; SEATS_MAX],
    /// The seat whose Start opened the pause, whose machine its switch row is.
    pauser: usize,
    /// The seat whose machine the ride setup's machine row is picking.
    setup_seat: usize,
    /// What the garage offers, off the disc; empty until [`Menu::with_garage`] gives it one.
    pub catalogue: Catalogue,
    /// Every seat's pick, as `garage.txt` keeps them.
    pub picks: Picks,
    /// The garage's seats while it is open, one [`Garage`] each.
    garages: Vec<Garage>,
    /// How many seats have a player, which is how many columns the garage shows.
    pub seated: usize,
    /// The picks changed since [`Menu::take_picks_changed`] last said so.
    picks_changed: bool,
    /// Online play's side of the menu (N3).
    pub lobby: Lobby,
    /// 1P (P46): the device the splash's press came from, the one that drives the menus — with the
    /// keyboard, which always may. `None` before anyone has pressed (or under `--screen`, which
    /// skips the splash and lets every device drive, as before P46).
    p1: Option<Dev>,
    /// What 1P is called on the splash's card, which the shell knows and the menu does not.
    pub p1_name: String,
    /// Frames since 1P pressed, while the splash's card is up.
    claimed: Option<u32>,
    /// The splash's Quit popup: the device that asked, which alone answers it, and its row.
    quit: Option<(Dev, usize)>,
    /// A local game's seats (P43): the controller on each, by the shell's pad seat number, which is
    /// the device id the join screen is over. Kept between rides, so the next local game opens on
    /// the last one's players.
    pub join: Join<usize>,
    /// What each pad seat's controller is called, for the join screen's cards; the shell fills it.
    pub devices: Vec<String>,
    /// Every device the shell reads, keyboard first, and the js devices it skips, by name: the
    /// splash lists both, so a pad that cannot become 1P — or one that should not — shows (P53).
    pub heard: Vec<String>,
    pub skipped: Vec<String>,
    /// The local game the ride setup is for, once the join screen is done; none for a Free Ride.
    party: Option<Party>,
    /// How many ride a Free Ride, which is `--players`: the shell says, and a Free Ride after a
    /// local game goes back to it.
    pub free_riders: usize,
    /// The songs the music row steps through after Off and Shuffle, `(id, label)`, as the shell's
    /// music sources list them (P42). Empty until the shell says.
    pub music_choices: Vec<(String, String)>,
}

/// A local game as the ride takes it (P43): one player a seat in play, in seat order, each on the
/// controller of their seat, and the layout their views are drawn in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Party {
    /// Player i's controller, as a pad seat number: the ride's player i reads that seat's pad.
    pub devices: Vec<usize>,
    pub split: Split,
}

impl Party {
    /// How many players ride, which is how many views.
    pub fn players(&self) -> usize {
        self.devices.len()
    }

    /// The layout the views are drawn in.
    pub fn layout(&self) -> split::Layout {
        split::Layout::for_players(self.players(), self.split).unwrap_or(split::Layout::Full)
    }
}

/// The layouts `players` can be drawn in, the split screen's rows: the original's two for a pair,
/// and the one the port has for any other count.
pub fn layouts(players: usize) -> Vec<split::Layout> {
    match players {
        2 => vec![split::Layout::Stacked, split::Layout::SideBySide],
        n => vec![split::Layout::for_players(n.clamp(1, split::MOST), Split::default()).unwrap_or(split::Layout::Full)],
    }
}

/// A layout's name on the split screen.
fn layout_label(l: split::Layout) -> &'static str {
    match l {
        split::Layout::Full => "Full Screen",
        split::Layout::Stacked => "One Above The Other",
        split::Layout::SideBySide => "Side By Side",
        split::Layout::Quarters => "Quarters",
        split::Layout::Eighths => "Eighths",
    }
}

impl Menu {
    pub fn new(settings: Settings, controls: Controls) -> Menu {
        let mut cursor = [0; Screen::ALL.len()];
        cursor[Screen::Tracks.index()] = settings.track_index();
        Menu {
            settings,
            controls,
            stack: vec![Screen::Main],
            cursor,
            typing: String::new(),
            typing_for: Typing::Name,
            key: (0, 0),
            tick: 0,
            changed: false,
            note: None,
            riders: 1,
            up: [Machine::Bike; SEATS_MAX],
            switchable: [false; SEATS_MAX],
            pauser: 0,
            setup_seat: 0,
            catalogue: Catalogue::default(),
            picks: Picks::default(),
            garages: Vec::new(),
            seated: 1,
            picks_changed: false,
            lobby: Lobby::default(),
            p1: None,
            p1_name: String::new(),
            claimed: None,
            quit: None,
            join: Join::new(join::ON_SCREEN, Opened::Menu),
            devices: Vec::new(),
            heard: Vec::new(),
            skipped: Vec::new(),
            party: None,
            free_riders: 1,
            music_choices: Vec::new(),
        }
    }

    /// The music row's choices: Off, Shuffle, then every song, as `(settings word, label)`.
    pub fn music_options(&self) -> Vec<(String, String)> {
        let mut out = vec![("off".to_string(), "Off".to_string()), ("shuffle".to_string(), "Shuffle".to_string())];
        out.extend(self.music_choices.iter().cloned());
        out
    }

    /// What the music row shows for the setting as it stands.
    pub fn music_label(&self) -> String {
        let all = self.music_options();
        match all.iter().find(|(id, _)| *id == self.settings.music) {
            Some((_, label)) => label.clone(),
            None => self.settings.music.clone(),
        }
    }

    /// The local game the ride that starts is, or none for a Free Ride (P43).
    pub fn party(&self) -> Option<&Party> {
        self.party.as_ref()
    }

    /// Which pad seats have a controller in them now, as the shell sees them: a join screen seat
    /// whose controller went is shown gone, and taken again when it is back.
    pub fn present(&mut self, devices: &[usize]) {
        self.join.update(devices);
    }

    fn open_join(&mut self) {
        // The controller that chose Local Game is in it: 1P, the host's seat, as opentennis's is.
        if self.join.views().is_empty() {
            self.join.press(&0, join::Press::Join);
        }
        self.push(Screen::Join);
    }

    /// A press on the join screen from pad seat `device`'s controller.
    fn walk_join(&mut self, device: usize, p: Press) {
        let presses = [
            (p.ok, join::Press::Join),
            (p.back, join::Press::Leave),
            (p.left, join::Press::Left),
            (p.right, join::Press::Right),
            (p.delete, join::Press::LetGo),
            (p.start, join::Press::Start),
        ];
        for (_, jp) in presses.into_iter().filter(|(on, _)| *on) {
            match self.join.press(&device, jp) {
                Outcome::Stay => {}
                Outcome::Back => {
                    self.pop();
                    return;
                }
                Outcome::Done => {
                    let riders = self.join.riders();
                    let devices: Vec<usize> = self.join.views().into_iter().filter_map(|i| riders[i]).collect();
                    self.riders = devices.len();
                    self.setup_seat = 0;
                    let at = layouts(self.riders).iter().position(|l| {
                        *l == split::Layout::for_players(self.riders, self.settings.split).unwrap_or(split::Layout::Full)
                    });
                    self.cursor[Screen::Split.index()] = at.unwrap_or(0);
                    self.party = Some(Party { devices, split: self.settings.split });
                    self.push(Screen::Split);
                    return;
                }
            }
        }
    }

    /// The garage's catalogue and the picks it starts from.
    pub fn with_garage(mut self, catalogue: Catalogue, picks: Picks) -> Menu {
        self.catalogue = catalogue;
        self.picks = picks;
        self
    }

    /// Whether the picks changed since the last time this was asked, which is when the shell writes
    /// `garage.txt`.
    pub fn take_picks_changed(&mut self) -> bool {
        std::mem::take(&mut self.picks_changed)
    }

    /// What the first `players` seats ride, each fitted to what the disc has: the bike archive, the
    /// set the simulation loads out of it and the rider archive. `None` without a catalogue, which
    /// leaves the ride to the command line.
    pub fn ride_picks(&self, players: usize) -> Option<Vec<Pick>> {
        if self.catalogue.bikes.is_empty() {
            return None;
        }
        let seat = |i: usize| self.picks.0.get(i).cloned().unwrap_or_default();
        Some((0..players.max(1)).map(|i| self.catalogue.fit(&seat(i))).collect())
    }

    /// One seat's garage while the screen is open.
    pub fn garage(&self, seat: usize) -> Option<&Garage> {
        self.garages.get(seat)
    }

    fn open_garage(&mut self) {
        self.garages = self.picks.0.iter().map(|p| Garage::new(&self.catalogue, p)).collect();
        self.push(Screen::Garage);
    }

    /// Leave the garage, keeping what every seat picked.
    fn close_garage(&mut self) {
        for (i, g) in self.garages.iter().enumerate() {
            if self.picks.0[i] != g.pick {
                self.picks.0[i] = g.pick.clone();
                self.picks_changed = true;
            }
        }
        self.pop();
    }

    /// A frame of another seat's pad (seat 0's is [`Menu::step`]'s). Only the garage hears it: each
    /// seat walks its own column, and only the first seat leaves the screen.
    /// The join screen hears every one, each pressing for its own seat (P43).
    pub fn step_seat(&mut self, seat: usize, p: Press) {
        if seat > 0 && self.screen() == Some(Screen::Join) {
            self.walk_join(seat, p);
            return;
        }
        if seat == 0 || seat >= self.seated || self.screen() != Some(Screen::Garage) {
            return;
        }
        self.walk_garage(seat, p);
    }

    /// Up and down choose a seat's row, left and right change it.
    fn walk_garage(&mut self, seat: usize, p: Press) {
        let Some(g) = self.garages.get_mut(seat) else { return };
        if p.up {
            g.up();
        } else if p.down {
            g.down();
        } else if p.left || p.right {
            g.turn(&self.catalogue, if p.left { -1 } else { 1 });
        }
    }

    /// The splash, 1P free for the next press.
    pub fn splash(&mut self) {
        self.stack = vec![Screen::Splash];
        self.p1 = None;
        self.claimed = None;
        self.quit = None;
        self.tick = 0;
    }

    /// 1P's device, once the splash has one.
    pub fn p1(&self) -> Option<&Dev> {
        self.p1.as_ref()
    }

    /// Hand 1P to another device without the splash: the keyboard taking over a ride whose 1P pad
    /// was pulled out (`pad.rs`'s `Seats::frame`).
    pub fn set_p1(&mut self, dev: Dev) {
        self.p1 = Some(dev);
    }

    /// Whether 1P's pad is gone from `plugged` (the pads plugged in, by name): then the splash
    /// comes back for a new 1P, and this says so. Over a ride the seat waits for its pad instead
    /// (P35), so a ride or its pause is left alone; the front end asks again when the ride is over.
    pub fn lost(&mut self, plugged: &[String]) -> bool {
        let gone = matches!(&self.p1, Some(Dev::Pad(id)) if !plugged.contains(id));
        if gone && !self.riding() && !self.stack.contains(&Screen::Pause) {
            self.splash();
            return true;
        }
        false
    }

    /// Whether the splash's Quit popup is up, and the row it is on.
    pub fn quitting(&self) -> Option<usize> {
        self.quit.as_ref().map(|q| q.1)
    }

    /// Whether `dev` drives the menus: 1P's device and the keyboard, or anyone before there is a 1P.
    pub fn hosts(&self, dev: &Dev) -> bool {
        match &self.p1 {
            None => true,
            Some(p1) => p1 == dev || *dev == Dev::Keys,
        }
    }

    /// One frame of every device's presses: on the splash the first Cross or Start (a pad's; Space
    /// or Enter on the keyboard, which its bindings make the same two) is 1P, and anything else
    /// moves nothing; past it, 1P's and the keyboard's presses together drive the screen.
    pub fn step_devices(&mut self, presses: &[(Dev, Press)]) -> Act {
        if self.screen() == Some(Screen::Splash) {
            return self.splash_step(presses);
        }
        let p = presses.iter().filter(|(d, _)| self.hosts(d)).fold(Press::default(), |p, (_, q)| p.or(*q));
        self.step(p)
    }

    /// The splash's frame.
    fn splash_step(&mut self, presses: &[(Dev, Press)]) -> Act {
        self.tick = self.tick.wrapping_add(1);
        if let Some(n) = &mut self.claimed {
            *n += 1;
            if *n >= CLAIMED_FRAMES {
                self.claimed = None;
                self.stack = vec![Screen::Main];
                self.cursor[Screen::Main.index()] = 0;
            }
            return Act::Stay;
        }
        if let Some((asked, row)) = self.quit.clone() {
            let Some((_, p)) = presses.iter().find(|(d, _)| *d == asked) else {
                return Act::Stay;
            };
            if p.up || p.down || p.left || p.right {
                self.quit = Some((asked, 1 - row));
            } else if p.ok && row == 0 {
                return Act::Quit;
            } else if p.ok || p.back || p.start {
                self.quit = None;
            }
            return Act::Stay;
        }
        if let Some((dev, _)) = presses.iter().find(|(_, p)| p.ok || p.start) {
            self.p1 = Some(dev.clone());
            self.claimed = Some(0);
        } else if let Some((dev, _)) = presses.iter().find(|(_, p)| p.back) {
            self.quit = Some((dev.clone(), 1));
        }
        Act::Stay
    }

    /// The screen shown, or none in a ride.
    pub fn screen(&self) -> Option<Screen> {
        self.stack.last().copied()
    }

    /// Whether a ride is running with nothing over it.
    pub fn riding(&self) -> bool {
        self.stack.is_empty()
    }

    /// Back to the main menu, with nothing under it.
    pub fn main(&mut self) {
        self.stack = vec![Screen::Main];
        self.pauser = 0;
    }

    /// The pause over a ride, 1P's.
    pub fn pause(&mut self) {
        self.pause_for(0);
    }

    /// The pause over a ride, opened by `seat`'s Start: its switch row is that player's machine.
    pub fn pause_for(&mut self, seat: usize) {
        self.stack = vec![Screen::Pause];
        self.cursor[Screen::Pause.index()] = 0;
        self.pauser = seat.min(SEATS_MAX - 1);
    }

    /// The shell's answer to [`Act::Switch`]: `seat` is on `up` now.
    pub fn switched(&mut self, seat: usize, up: Machine) {
        if let Some(m) = self.up.get_mut(seat) {
            *m = up;
        }
    }

    /// The pause's rows as drawn: the switch row names the machine it would go to, or says there is
    /// none for this player.
    pub fn pause_rows(&self) -> Vec<String> {
        let seat = self.pauser;
        PAUSE
            .iter()
            .enumerate()
            .map(|(i, row)| match i {
                PAUSE_SWITCH if self.switchable[seat] => format!("{row}: {}", self.up[seat].other().label()),
                PAUSE_SWITCH => format!("{row}: None"),
                _ => row.to_string(),
            })
            .collect()
    }

    /// Open `screen` the way the pad would reach it, for `--screen`: over the main menu, or for the
    /// pause over a ride.
    pub fn open(&mut self, screen: Screen) {
        match screen {
            Screen::Splash => self.splash(),
            Screen::Main => self.main(),
            Screen::Pause => self.pause(),
            Screen::Controls => self.stack = vec![Screen::Main, Screen::Settings, Screen::Controls],
            Screen::Keyboard => {
                self.stack = vec![Screen::Main, Screen::Settings];
                self.open_keyboard();
            }
            Screen::Garage => {
                self.main();
                self.open_garage();
            }
            Screen::Games | Screen::Lobby => self.stack = vec![Screen::Main, Screen::Online, screen],
            Screen::Join => {
                self.main();
                self.open_join();
            }
            Screen::Split => {
                self.main();
                self.open_join();
                self.walk_join(0, Press { start: true, ..Press::default() });
            }
            s => self.stack = vec![Screen::Main, s],
        }
    }

    /// Whether the settings changed since the last time this was asked, which is when the shell
    /// writes them.
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    /// Whether they changed, without forgetting it.
    pub fn changed(&self) -> bool {
        self.changed
    }

    pub fn cursor(&self, screen: Screen) -> usize {
        self.cursor[screen.index()]
    }

    fn rows(&self, screen: Screen) -> usize {
        match screen {
            Screen::Splash => 1,
            Screen::Main => MAIN.len(),
            Screen::Tracks => TRACKS.len(),
            Screen::Settings => SETTINGS_ROWS,
            Screen::Controls => CONTROLS_ROWS,
            Screen::Keyboard => KEY_ROWS.len() + 1,
            Screen::Pause => PAUSE.len(),
            Screen::Garage => roster::ROWS.len(),
            Screen::Online => ONLINE.len() + 1,
            Screen::Games => self.lobby.found.len() + 1,
            Screen::Lobby => self.lobby.items().len(),
            Screen::Join => join::ON_SCREEN,
            Screen::Split => layouts(self.riders).len(),
        }
    }

    fn push(&mut self, screen: Screen) {
        self.stack.push(screen);
    }

    fn pop(&mut self) {
        self.stack.pop();
    }

    fn open_keyboard(&mut self) {
        self.type_for(Typing::Name);
    }

    fn type_for(&mut self, purpose: Typing) {
        self.typing = match purpose {
            Typing::Name => self.settings.name.clone(),
            _ => String::new(),
        };
        self.typing_for = purpose;
        self.key = (0, 0);
        self.push(Screen::Keyboard);
    }

    /// One frame of a ride's menu, with every rider's press, 1P's first (P41): while the ride runs any
    /// player's Start opens the pause for them, 1P's winning a tie, and an open pause hears only the
    /// player who opened it — their switch row, their pad.
    pub fn step_seats(&mut self, presses: &[Press]) -> Act {
        let first = presses.first().copied().unwrap_or_default();
        if self.riding() {
            if let Some(seat) = presses.iter().position(|p| p.start)
                && seat > 0
            {
                self.tick = self.tick.wrapping_add(1);
                self.pause_for(seat);
                return Act::Stay;
            }
            return self.step(first);
        }
        let p = match self.stack.first() {
            Some(Screen::Pause) => presses.get(self.pauser).copied().unwrap_or_default(),
            _ => first,
        };
        self.step(p)
    }

    /// The seat whose press drives the menu this frame under [`Menu::step_seats`].
    pub fn driver(&self) -> usize {
        match self.stack.first() {
            Some(Screen::Pause) => self.pauser,
            _ => 0,
        }
    }

    /// One frame of one device's presses, the keyboard's on the splash.
    pub fn step(&mut self, p: Press) -> Act {
        if self.screen() == Some(Screen::Splash) {
            return self.splash_step(&[(Dev::Keys, p)]);
        }
        self.tick = self.tick.wrapping_add(1);
        let Some(screen) = self.screen() else {
            // In a ride the only thing the menu hears is Start.
            if p.start {
                self.pause();
            }
            return Act::Stay;
        };
        if screen == Screen::Keyboard {
            return self.keyboard(p);
        }
        if screen == Screen::Garage {
            // Cross, Circle and Start all leave it: a pick is made by turning it, so there is
            // nothing to confirm and nothing to cancel.
            if p.ok || p.back || p.start {
                self.close_garage();
            } else {
                self.walk_garage(0, p);
            }
            return Act::Stay;
        }
        if screen == Screen::Join {
            self.walk_join(0, p);
            return Act::Stay;
        }
        let rows = self.rows(screen);
        // The lobby's and the list's rows come and go with what the network says.
        let at = self.cursor(screen).min(rows - 1);
        self.cursor[screen.index()] = at;
                // Up and down walk the rows, wrapping, except on the ride setup where left and right walk the
        // tracks and up and down the players whose machine Square changes.
        let (prev, next) = match screen {
            Screen::Tracks => (p.left, p.right),
            _ => (p.up, p.down),
        };
        if prev || next {
            let to = if prev { (at + rows - 1) % rows } else { (at + 1) % rows };
            self.cursor[screen.index()] = to;
            if screen == Screen::Tracks {
                self.tick = 0;
            }
            return Act::Stay;
        }
        let side = p.left || p.right;
        match screen {
            // Back on the main menu is the splash again, 1P free for the next press (opentennis's
            // B533); Quit is a row, and Back on the splash asks.
            Screen::Main if p.back => self.splash(),
            Screen::Main => match (p.ok || p.start, p.back, at) {
                (true, _, MAIN_FREE) => {
                    if self.party.take().is_some() {
                        self.riders = self.free_riders;
                    }
                    self.cursor[Screen::Tracks.index()] = self.settings.track_index();
                    self.tick = 0;
                    self.push(Screen::Tracks);
                }
                (true, _, MAIN_LOCAL) => self.open_join(),
                (true, _, MAIN_ONLINE) => self.push(Screen::Online),
                (true, _, MAIN_GARAGE) => self.open_garage(),
                (true, _, MAIN_SETTINGS) => self.push(Screen::Settings),
                (true, _, _) => return Act::Quit,
                _ => {}
            },
            Screen::Split => {
                if p.back {
                    self.pop();
                } else if p.ok || p.start {
                    let layout = layouts(self.riders)[at.min(rows - 1)];
                    let split = match layout {
                        split::Layout::SideBySide => Split::SideBySide,
                        split::Layout::Stacked => Split::Stacked,
                        _ => self.settings.split,
                    };
                    if split != self.settings.split {
                        self.settings.split = split;
                        self.changed = true;
                    }
                    if let Some(party) = &mut self.party {
                        party.split = split;
                    }
                    self.cursor[Screen::Tracks.index()] = self.settings.track_index();
                    self.tick = 0;
                    self.push(Screen::Tracks);
                }
            }
            Screen::Tracks => {
                let riders = self.riders.clamp(1, SEATS_MAX);
                if p.up || p.down {
                    let at = self.setup_seat.min(riders - 1);
                    self.setup_seat = if p.up { (at + riders - 1) % riders } else { (at + 1) % riders };
                    return Act::Stay;
                }
                if p.delete {
                    let seat = self.setup_seat.min(riders - 1);
                    self.settings.machines[seat] = self.settings.machines[seat].other();
                    self.changed = true;
                    return Act::Stay;
                }
                if p.ok || p.start {
                    let track = TRACKS[at].0.to_string();
                    if self.settings.track != track {
                        self.settings.track = track.clone();
                        self.changed = true;
                    }
                    self.stack.clear();
                    return Act::Ride(track);
                }
                if p.back {
                    self.pop();
                }
            }
            Screen::Settings => {
                if p.back {
                    self.pop();
                } else if p.ok || side {
                    match at {
                        0 if p.ok => self.open_keyboard(),
                        1 => {
                            let all = Overlay::ALL;
                            let i = all.iter().position(|o| *o == self.settings.overlay).unwrap_or(0);
                            let i = if p.left { (i + all.len() - 1) % all.len() } else { (i + 1) % all.len() };
                            self.settings.overlay = all[i];
                            self.changed = true;
                        }
                        2 => {
                            self.settings.wrecks = !self.settings.wrecks;
                            self.changed = true;
                        }
                        3 if p.ok => self.push(Screen::Controls),
                        SETTINGS_MUSIC => {
                            let all = self.music_options();
                            let n = all.len();
                            let i = all.iter().position(|(id, _)| *id == self.settings.music).unwrap_or(1);
                            let i = if p.left { (i + n - 1) % n } else { (i + 1) % n };
                            self.settings.music = all[i].0.clone();
                            self.changed = true;
                        }
                        SETTINGS_MUSIC_VOLUME | SETTINGS_EFFECTS_VOLUME => {
                            let v = if at == SETTINGS_MUSIC_VOLUME {
                                &mut self.settings.music_volume
                            } else {
                                &mut self.settings.effects_volume
                            };
                            *v = match (p.left, p.right) {
                                (true, _) => v.saturating_sub(1),
                                (_, true) => (*v + 1).min(VOLUME_MAX),
                                _ => (*v + 1) % (VOLUME_MAX + 1),
                            };
                            self.changed = true;
                        }
                        _ => {}
                    }
                }
            }
            Screen::Controls => {
                if p.back {
                    self.pop();
                } else if (p.ok || side) && at == 0 {
                    let i = Gas::ALL.iter().position(|g| *g == self.controls.throttle).unwrap_or(0);
                    let n = Gas::ALL.len();
                    let i = if p.left { (i + n - 1) % n } else { (i + 1) % n };
                    self.controls.throttle = Gas::ALL[i];
                    self.changed = true;
                } else if (p.ok || side) && at <= LAYOUT_ROWS {
                    // Two layouts, so either way is the other one.
                    let seat = &mut self.controls.layouts[at - 1];
                    *seat = if *seat == Layout::New { Layout::Original } else { Layout::New };
                    self.changed = true;
                }
            }
            Screen::Pause => {
                if p.back || p.start {
                    self.stack.clear();
                    return Act::Resume;
                }
                if p.ok {
                    match at {
                        0 => {
                            self.stack.clear();
                            return Act::Resume;
                        }
                        1 => {
                            self.stack.clear();
                            return Act::Restart;
                        }
                        PAUSE_SWITCH => {
                            if self.switchable[self.pauser] {
                                self.stack.clear();
                                return Act::Switch(self.pauser);
                            }
                        }
                        3 => self.push(Screen::Settings),
                        4 => {
                            self.main();
                            return Act::MainMenu;
                        }
                        _ => return Act::Quit,
                    }
                }
            }
            Screen::Online => self.online(p, at),
            Screen::Games => self.games(p, at),
            Screen::Lobby => self.in_lobby(p, at),
            Screen::Keyboard | Screen::Garage | Screen::Splash | Screen::Join => unreachable!(),
        }
        Act::Stay
    }

    /// The on-screen keyboard: the cursor over the four rows of ten and the command row, Cross
    /// pressing the key under it, Square deleting, Start finishing and Circle leaving the name as it
    /// was.
    fn keyboard(&mut self, p: Press) -> Act {
        let (row, col) = self.key;
        let last = KEY_ROWS.len();
        let width = |r: usize| if r == last { KEY_COMMANDS.len() } else { 10 };
        if p.up || p.down {
            let to = if p.up { (row + last) % (last + 1) } else { (row + 1) % (last + 1) };
            // Into and out of the command row the column scales, so the cursor stays roughly above
            // where it was.
            let col = col * width(to) / width(row);
            self.key = (to, col.min(width(to) - 1));
        } else if p.left || p.right {
            let n = width(row);
            self.key.1 = if p.left { (col + n - 1) % n } else { (col + 1) % n };
        } else if p.delete {
            self.typing.pop();
        } else if p.back {
            self.pop();
        } else if p.start {
            self.finish_typing();
        } else if p.ok {
            let max = self.typing_for.max();
            if row < last {
                if self.typing.chars().count() < max {
                    let c = KEY_ROWS[row].chars().nth(col).unwrap();
                    // The first letter of each word in capitals, as the original's name entry; a room
                    // code is all capitals.
                    let word_start = self.typing.is_empty() || self.typing.ends_with(' ');
                    let upper = match self.typing_for {
                        Typing::Name => word_start,
                        Typing::Code => true,
                        Typing::Chat => false,
                    };
                    self.typing.push(if upper { c.to_ascii_uppercase() } else { c });
                }
            } else {
                match col {
                    0 => {
                        if !self.typing.is_empty() && self.typing.chars().count() < max {
                            self.typing.push(' ');
                        }
                    }
                    1 => {
                        self.typing.pop();
                    }
                    2 => self.typing.clear(),
                    _ => self.finish_typing(),
                }
            }
        }
        Act::Stay
    }

    fn finish_typing(&mut self) {
        match self.typing_for {
            Typing::Name => self.finish_name(),
            Typing::Code => {
                let code = self.typing.trim().to_string();
                self.pop();
                if !code.is_empty() {
                    self.lobby.join(&code);
                    self.push(Screen::Lobby);
                }
            }
            Typing::Chat => {
                let text = self.typing.trim().to_string();
                self.pop();
                if !text.is_empty() {
                    self.lobby.out.push(Cmd::Chat(text.clone()));
                    self.lobby.said(&self.settings.name, &text);
                }
            }
        }
    }

    fn finish_name(&mut self) {
        let name = self.typing.trim().to_string();
        if !name.is_empty() && name != self.settings.name {
            self.settings.name = name;
            self.changed = true;
        }
        self.pop();
    }

    /// The keyboard's text as it stands.
    pub fn typing(&self) -> &str {
        &self.typing
    }

    // ---- online (N3) ----

    /// The online menu: Quickplay, Host, Find Games, Join by Code, and the network it uses.
    fn online(&mut self, p: Press, at: usize) {
        let side = p.left || p.right;
        if p.back {
            self.pop();
        } else if at == ONLINE.len() && (p.ok || side) {
            self.lobby.lan = !self.lobby.lan;
            self.lobby.network_changed = true;
        } else if p.ok || p.start {
            let room = self.lobby.room_for(&self.settings);
            match at {
                0 => {
                    self.lobby.begin("Looking for a game...");
                    self.lobby.out.push(Cmd::Quick(room));
                    self.push(Screen::Lobby);
                }
                1 => {
                    self.lobby.begin("Opening the lobby...");
                    self.lobby.out.push(Cmd::Host(room));
                    self.push(Screen::Lobby);
                }
                2 => {
                    self.lobby.found.clear();
                    self.lobby.status = "Looking for games...".into();
                    self.lobby.out.push(Cmd::Find);
                    self.push(Screen::Games);
                }
                _ => self.type_for(Typing::Code),
            }
        }
    }

    /// The lobbies found: OK on one joins it, on the last row looks again.
    fn games(&mut self, p: Press, at: usize) {
        if p.back {
            self.pop();
        } else if p.ok || p.start {
            match self.lobby.found.get(at).cloned() {
                Some(l) => {
                    self.lobby.join(&l.id);
                    self.pop();
                    self.push(Screen::Lobby);
                }
                None => {
                    self.lobby.found.clear();
                    self.lobby.status = "Looking for games...".into();
                    self.lobby.out.push(Cmd::Find);
                }
            }
        }
    }

    /// The lobby's rows: the host sets the ride and starts it, a guest says it is ready, and anyone
    /// changes their bike, chats or leaves.
    fn in_lobby(&mut self, p: Press, at: usize) {
        if p.back {
            self.leave_lobby();
            return;
        }
        let side = p.left || p.right;
        let by = if p.left { -1 } else { 1 };
        let Some(&item) = self.lobby.items().get(at) else { return };
        let l = &mut self.lobby;
        match item {
            Item::Track if p.ok || side => {
                if let Some(room) = &l.room {
                    let i = TRACKS.iter().position(|t| t.0 == room.track).unwrap_or(0) as i32;
                    let n = TRACKS.len() as i32;
                    let track = TRACKS[(i + by).rem_euclid(n) as usize].0.to_string();
                    l.out.push(Cmd::Ride { track, wrecks: room.wrecks, biplane: false });
                }
            }
            Item::Wrecks if p.ok || side => {
                if let Some(room) = &l.room {
                    l.out.push(Cmd::Ride { track: room.track.clone(), wrecks: !room.wrecks, biplane: false });
                }
            }
            Item::Bike if (p.ok || side) && !l.bikes.is_empty() => {
                let i = l.bikes.iter().position(|b| b.0 == l.pick.bike).unwrap_or(0) as i32;
                let n = l.bikes.len() as i32;
                l.pick.bike = l.bikes[(i + by).rem_euclid(n) as usize].0.clone();
                l.out.push(Cmd::Picks(vec![l.pick.clone()]));
            }
            Item::Private if p.ok || side => {
                if let Some(room) = &l.room {
                    l.out.push(Cmd::Private(!room.private));
                }
            }
            Item::Chat if p.ok => self.type_for(Typing::Chat),
            Item::Start if p.ok || p.start => {
                l.status = "Starting...".into();
                l.out.push(Cmd::Start);
            }
            Item::Ready if p.ok => {
                l.ready = !l.ready;
                l.out.push(Cmd::Prepared(l.ready));
            }
            Item::Leave if p.ok => self.leave_lobby(),
            _ => {}
        }
    }

    fn leave_lobby(&mut self) {
        self.lobby.out.push(Cmd::Leave);
        self.lobby.room = None;
        self.lobby.ready = false;
        self.lobby.status.clear();
        self.pop();
    }

    /// What the network said. `Act::Online` when the lobby's ride starts.
    pub fn heard(&mut self, e: Event) -> Act {
        let l = &mut self.lobby;
        match e {
            Event::Status(s) => l.status = s,
            Event::Hosting => l.status = "No game to join: hosting one.".into(),
            Event::Code(_) => {}
            Event::Short(code) => l.code = code,
            Event::Me(me) => l.me = me,
            Event::Found(found) => {
                l.status = match found.len() {
                    0 => "No open games found.".into(),
                    n => format!("{n} found"),
                };
                l.found = found;
            }
            Event::Room(room) => {
                if !room.short.is_empty() {
                    l.code = room.short.clone();
                }
                l.ready = room.ready.contains(&l.me);
                l.room = Some(room);
            }
            Event::Start(room, me) => {
                l.me = me.clone();
                l.room = Some(room.clone());
                l.start = Some((room, me));
                l.status.clear();
                self.stack.clear();
                return Act::Online;
            }
            Event::Dropped(why) => {
                l.status = why.clone();
                self.note = Some(why);
            }
            Event::Failed(why) => {
                l.room = None;
                l.ready = false;
                l.status = why.clone();
                self.note = Some(why);
                if self.screen() == Some(Screen::Lobby) {
                    self.pop();
                }
            }
            Event::Chat { name, text, .. } => l.said(&name, &text),
            Event::Ride(..) | Event::Datagram(..) => {}
        }
        Act::Stay
    }

    /// Back from an online ride: the lobby if this machine is still in one, else the online menu.
    pub fn back_from_online(&mut self) {
        self.lobby.start = None;
        self.stack = vec![Screen::Main, Screen::Online];
        if self.lobby.room.is_some() {
            self.stack.push(Screen::Lobby);
        }
    }
}

// ---- online (N3) ---------------------------------------------------------------------------------

/// The online menu's rows; a last one switches the network.
const ONLINE: [&str; 4] = ["Quickplay", "Host", "Find Games", "Join by Code"];

/// The players a lobby this machine opens has seats for.
pub const LOBBY_SEATS: usize = 4;

/// The chat lines the lobby keeps.
const CHAT_LINES: usize = 5;

/// What the keyboard is typing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Typing {
    Name,
    Code,
    Chat,
}

impl Typing {
    fn max(self) -> usize {
        match self {
            Typing::Name => NAME_MAX,
            // A code is four; a little over lets a slip be seen and deleted.
            Typing::Code => 8,
            Typing::Chat => 40,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Typing::Name => "Rider Name",
            Typing::Code => "Room Code",
            Typing::Chat => "Chat",
        }
    }
}

/// A row of the lobby.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Track,
    Wrecks,
    Bike,
    Private,
    Chat,
    Start,
    Ready,
    Leave,
}

impl Item {
    fn label(self, l: &Lobby) -> String {
        let room = l.room.as_ref();
        let on = |b: bool| if b { "On" } else { "Off" };
        match self {
            Item::Track => {
                let t = room.map_or("", |r| r.track.as_str());
                format!("Track  {}", TRACKS.iter().find(|x| x.0 == t).map_or(t, |x| x.1))
            }
            Item::Wrecks => format!("Wrecks  {}", on(room.is_some_and(|r| r.wrecks))),
            Item::Bike => format!("Bike  {}", l.bike_label(&l.pick.bike)),
            Item::Private => format!("Private  {}", on(room.is_some_and(|r| r.private))),
            Item::Chat => "Chat".into(),
            Item::Start => "Start".into(),
            Item::Ready => if l.ready { "Not Ready" } else { "Ready" }.into(),
            Item::Leave => "Leave".into(),
        }
    }
}

/// Online play's side of the menu: what the network last said, and what the menu asks of it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lobby {
    /// Commands for the shell to hand the network, oldest first.
    pub out: Vec<Cmd>,
    /// The lobby as the host last sent it; none when not in one.
    pub room: Option<Room>,
    /// This machine's peer name in the seats ([`HOST`] when it hosts).
    pub me: String,
    /// The room code to pass on.
    pub code: String,
    pub found: Vec<Listing>,
    /// The network's last word, for the lobby's ticker.
    pub status: String,
    /// The last few chat lines, `name: text`.
    pub chat: Vec<String>,
    /// This machine's player's rider and bike set.
    pub pick: openmx_net::Pick,
    /// The bike sets the lobby offers, as (`race125p`, `125 Race P`); the shell fills it from the
    /// garage's sixteen.
    pub bikes: Vec<(String, String)>,
    /// Said ready.
    pub ready: bool,
    /// The LAN only: no relays, no board, no code servers — nothing leaves the network it is on.
    pub lan: bool,
    /// [`Lobby::lan`] changed, which the shell writes to `online.txt` and starts the network over for.
    pub network_changed: bool,
    /// The line-up and this machine's peer name, once the host has started the ride.
    pub start: Option<(Room, String)>,
}

impl Lobby {
    /// The rows: the host's, a guest's, or only Leave while there is no lobby yet.
    pub fn items(&self) -> Vec<Item> {
        match &self.room {
            None => vec![Item::Leave],
            Some(_) if self.hosting() => {
                vec![Item::Track, Item::Wrecks, Item::Bike, Item::Private, Item::Chat, Item::Start, Item::Leave]
            }
            Some(_) => vec![Item::Bike, Item::Chat, Item::Ready, Item::Leave],
        }
    }

    pub fn hosting(&self) -> bool {
        self.me == HOST
    }

    /// A lobby for this machine's player to open: the last free-ride track, the settings' wrecks.
    fn room_for(&self, settings: &Settings) -> Room {
        let mut room = Room::new(&settings.name, &settings.track, LOBBY_SEATS);
        room.wrecks = settings.wrecks;
        room.take(HOST, std::slice::from_ref(&self.pick));
        room
    }

    fn begin(&mut self, status: &str) {
        self.room = None;
        self.me.clear();
        self.ready = false;
        self.code.clear();
        self.chat.clear();
        self.status = status.into();
    }

    fn join(&mut self, code: &str) {
        self.begin(&format!("Joining {code}..."));
        self.out.push(Cmd::Join(code.to_string(), vec![self.pick.clone()]));
    }

    fn said(&mut self, name: &str, text: &str) {
        self.chat.push(format!("{name}: {text}"));
        if self.chat.len() > CHAT_LINES {
            self.chat.remove(0);
        }
    }

    fn bike_label(&self, set: &str) -> String {
        self.bikes.iter().find(|b| b.0 == set).map_or(set.to_string(), |b| b.1.clone())
    }

    /// A seat's line: its player, their rider and bike, ready or not, and their ping.
    fn seat_line(&self, room: &Room, i: usize) -> String {
        let Some(seat) = room.seats.get(i).and_then(|s| s.as_ref()) else {
            return format!("{}  open", i + 1);
        };
        let name = room.names.get(&seat.peer).cloned().unwrap_or_else(|| {
            if seat.peer == HOST { room.name.clone() } else { seat.peer.chars().take(6).collect() }
        });
        let state = if seat.peer == HOST {
            "Host"
        } else if room.ready.contains(&seat.peer) {
            "Ready"
        } else {
            "Not ready"
        };
        let ping = room.ping.get(&seat.peer).map_or(String::new(), |ms| format!("  {ms} ms"));
        format!("{}  {name}  {}  {}  {state}{ping}", i + 1, seat.pick.rider, self.bike_label(&seat.pick.bike))
    }
}

// ---- the art -------------------------------------------------------------------------------------

/// One character of a `.cel` font: its box in the atlas and how far it moves the pen.
#[derive(Clone, Debug)]
pub(crate) struct Glyph {
    /// Left, top, right, bottom in atlas texels.
    pub(crate) uv: [f32; 4],
    /// Pen to the box's left edge, the box's width, and the box's right edge to the next pen.
    pub(crate) before: f32,
    pub(crate) width: f32,
    pub(crate) after: f32,
    /// The following character, and how much nearer (negative) it sits.
    pub(crate) kern: Vec<(u32, f32)>,
}

/// A `.cel` font and its atlas.
///
/// ```text
/// 0   f32 (2.71 in every font; unexplained), u16 point size, s16 -1, u32 glyph count
/// 12  per glyph: u32 code, 4 x f32 its box in the atlas (0..1), u32 code again,
///     s16 width, s16 space before, s16 space after, u16 line height (unread), u16 kerning pairs,
///     then each pair as u16 next code, s16 adjustment
/// ```
///
/// The box's width is the `width` field to the texel in every glyph looked at. Bytes after the last
/// glyph (some hundreds in each font) are not read. Glyph 0 is the box drawn for a missing code.
pub(crate) struct Font {
    glyphs: HashMap<u32, Glyph>,
    pub(crate) atlas: Texture,
}

fn le16(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

fn le32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

impl Font {
    pub(crate) fn parse(cel: &[u8], atlas: Texture) -> Result<Font, String> {
        let bad = || "a .cel font cut short".to_string();
        let count = le32(cel, 8).ok_or_else(bad)? as usize;
        let (aw, ah) = (atlas.width as f32, atlas.height as f32);
        let mut glyphs = HashMap::new();
        let mut at = 12;
        for _ in 0..count {
            let code = le32(cel, at).ok_or_else(bad)?;
            let f = |i: usize| le32(cel, at + 4 + 4 * i).map(f32::from_bits).ok_or_else(bad);
            let uv = [f(0)? * aw, f(1)? * ah, f(2)? * aw, f(3)? * ah];
            let s = |o: usize| le16(cel, at + o).map(|v| v as i16 as f32).ok_or_else(bad);
            let (width, before, after) = (s(24)?, s(26)?, s(28)?);
            let pairs = le16(cel, at + 32).ok_or_else(bad)? as usize;
            at += 34;
            let mut kern = Vec::with_capacity(pairs);
            for k in 0..pairs {
                let next = le16(cel, at + 4 * k).ok_or_else(bad)? as u32;
                let by = le16(cel, at + 4 * k + 2).ok_or_else(bad)? as i16 as f32;
                kern.push((next, by));
            }
            at += 4 * pairs;
            glyphs.insert(code, Glyph { uv, before, width, after, kern });
        }
        if glyphs.is_empty() {
            return Err("a .cel font with no glyphs".into());
        }
        Ok(Font { glyphs, atlas })
    }

    pub(crate) fn glyph(&self, c: char) -> &Glyph {
        self.glyphs
            .get(&(c as u32))
            .or_else(|| self.glyphs.get(&0))
            .unwrap_or_else(|| self.glyphs.values().next().unwrap())
    }

    /// How wide `text` is, in layout pixels.
    pub(crate) fn measure(&self, text: &str) -> f32 {
        let mut w = 0.0;
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            let g = self.glyph(c);
            w += g.before + g.width + g.after;
            if let Some(&n) = chars.peek() {
                w += g.kern.iter().find(|k| k.0 == n as u32).map_or(0.0, |k| k.1);
            }
        }
        w
    }
}

/// The fonts the menus use, by `ui\mxfrontend.sty`'s styles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Face {
    /// MenuItem: `yearlingdashbold_20`.
    Item,
    /// MenuItem selected: `yearlingdashboldoblique_22`.
    Chosen,
    /// Titles: `yearlingdashboldoblique_32`.
    Title,
    /// The ticker and small print: `yearlingdashbold_18`.
    Small,
}

const FACES: [(Face, &str); 4] = [
    (Face::Item, "yearlingdashbold_20"),
    (Face::Chosen, "yearlingdashboldoblique_22"),
    (Face::Title, "yearlingdashboldoblique_32"),
    (Face::Small, "yearlingdashbold_18"),
];

/// The textures, by their name under `ui\tex\`.
const TEXTURES: [&str; 16] = [
    "background",
    "frontend_background_radial",
    "mainmenu_background",
    "mainmenu_highlight",
    "mainmenu_highlight_off_shadow",
    "mainmenu_logo",
    "mainmenu_logo_shadow",
    "arrow_left_off",
    "arrow_right_off",
    "arrow_up_off",
    "arrow_down_off",
    "nav_bkgcorner",
    "nav_x_off",
    "nav_circle_off",
    "nav_square_off",
    "nav_start_off",
];

/// The stills per track in `UI\trackpreview.res`.
const STILLS: usize = 5;
/// Frames each still shows for on the track pick.
const STILL_FRAMES: u32 = 150;

/// MenuItem's colour, and its alpha unselected; `mxfrontend.sty`.
const ITEM: [u8; 4] = [204, 204, 204, 210];
const CHOSEN: [u8; 4] = [255, 255, 255, 255];
/// The orange the grey background gradient is drawn in. The original tints it from a colour
/// animator this does not read; this is the main menu's colour off `plan/HUMAN.md`'s screenshot.
const ORANGE: [u8; 4] = [210, 112, 52, 255];
/// The splash's still, dimmed to a backdrop the logo and the prompt read over.
const SPLASH_DIM: [u8; 4] = [110, 110, 110, 255];
/// Who 1P is, on the splash once someone has pressed: the main menu's orange, lighter, for text.
const ORANGE_TEXT: [u8; 4] = [255, 170, 90, 255];
/// The bar of the controller legend and the ticker under it.
const BLACK: [u8; 4] = [0, 0, 0, 255];

/// Everything the menus draw with, off the disc.
pub struct Art {
    textures: HashMap<&'static str, Texture>,
    fonts: HashMap<Face, Font>,
    /// Each free-ride track's stills, in [`TRACKS`] order.
    stills: Vec<Vec<Texture>>,
    /// The big stretches drawn last frame, by what they were drawn from and over (P47).
    blits: std::sync::Mutex<Vec<Blitted>>,
}

/// One big [`Painter::blit_as`] as it came out: the texture, its part, the box, the tint and the
/// way of putting texels down, the rows it covered as they were before, and the same rows after.
/// The background is the same full-frame stretch over the same black every frame, so the next
/// frame that asks for it over the same pixels gets the pixels it got, without a texel read.
struct Blitted {
    key: (usize, [u32; 4], [usize; 4], [u8; 4], usize),
    before: Vec<u8>,
    after: Vec<u8>,
}

/// Where a box goes, in the 640x448 layout.
#[derive(Clone, Copy, Debug)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect { x, y, w, h }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Align {
    Left,
    Centre,
}

/// The layout's screen. A window of another shape gets the layout at one scale both ways (the
/// smaller of its two), and the room left over goes to whichever side each part is anchored away
/// from, so nothing stretches.
const LAYOUT: (f32, f32) = (640.0, 448.0);

/// Which corner of the frame a part's layout numbers count from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Anchor {
    TopLeft,
    BottomLeft,
    BottomRight,
    Centre,
}

/// How far a pill reaches past each end of its label, in layout pixels.
pub const PILL_PAD: f32 = 12.0;
/// The list's rows are centred on this, `mainmenu.lyt`'s items.
const LIST_X: f32 = 185.0;
/// `mainmenu.lyt`'s logo, 512x64.
const LOGO: Rect = Rect { x: 20.0, y: 25.0, w: 512.0, h: 64.0 };

/// A box the menu drew, in frame pixels: left, top, width, height.
pub type Bounds = [f32; 4];

/// What a screen drew where, so its layout can be checked without reading pixels back.
#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    /// The logo's box.
    Logo(Bounds),
    /// A highlight pill and the label on it.
    Pill { pill: Bounds, label: Bounds },
    /// A row's label with no pill under it.
    Item(Bounds),
}

impl Art {
    pub fn load(disc: &mut Disc) -> Result<Art, String> {
        let ui = disc.res("UI\\ui.res")?;
        let tex = |name: &str| -> Result<Texture, String> {
            let path = format!("ui\\tex\\{name}.pst");
            Texture::parse(&ui.read_suffix(&path)?).map_err(|e| format!("{path}: {e}"))
        };
        let mut textures = HashMap::new();
        for name in TEXTURES {
            textures.insert(name, tex(name)?);
        }
        let mut fonts = HashMap::new();
        for (face, name) in FACES {
            let cel = ui.read_suffix(&format!("font\\eng\\{name}.cel"))?;
            let atlas = Texture::parse(&ui.read_suffix(&format!("font\\eng\\{name}.pst"))?)
                .map_err(|e| format!("{name}.pst: {e}"))?;
            fonts.insert(face, Font::parse(&cel, atlas).map_err(|e| format!("{name}.cel: {e}"))?);
        }
        let previews = disc.res("UI\\trackpreview.res")?;
        let mut stills = Vec::new();
        for (track, _) in TRACKS {
            let mut these = Vec::new();
            for i in 1..=STILLS {
                let path = format!("ui\\tex\\{track}_preview_{i:02}.pst");
                if let Ok(bytes) = previews.read_suffix(&path) {
                    these.push(Texture::parse(&bytes).map_err(|e| format!("{path}: {e}"))?);
                }
            }
            stills.push(these);
        }
        Ok(Art { textures, fonts, stills, blits: Default::default() })
    }

    /// Draw the menu's screen into `frame`. In a ride the frame already holds the ride, which the
    /// pause darkens rather than covers; otherwise the menu draws its own background.
    pub fn draw(&self, menu: &Menu, frame: &mut Frame) {
        self.paint(menu, frame);
    }

    /// [`Art::draw`], saying where the logo, the pills and the labels went.
    pub fn parts(&self, menu: &Menu, frame: &mut Frame) -> Vec<Part> {
        self.paint(menu, frame)
    }

    fn paint(&self, menu: &Menu, frame: &mut Frame) -> Vec<Part> {
        let Some(screen) = menu.screen() else {
            return Vec::new();
        };
        let mut p = Painter::new(self, frame);
        match screen {
            Screen::Pause => p.fill(p.whole(), [0, 0, 0, 140]),
            Screen::Tracks | Screen::Splash => {}
            _ => p.background(),
        }
        match screen {
            Screen::Main => {
                p.column();
                p.list(&MAIN.map(|s| s.to_string()), menu.cursor(screen));
                p.logo();
                p.legend(&[("nav_x_off", "Select")]);
            }
            Screen::Pause => {
                p.column();
                p.list(&menu.pause_rows(), menu.cursor(screen));
                p.logo();
                // With more than one rider the pause is one player's, and says whose.
                match menu.riders > 1 {
                    true => p.title(&format!("{}P {}", menu.pauser + 1, screen.title())),
                    false => p.title(screen.title()),
                }
                p.legend(&[("nav_circle_off", "Resume"), ("nav_x_off", "Select")]);
            }
            Screen::Settings | Screen::Controls => {
                let rows: Vec<String> = match screen {
                    Screen::Settings => vec![
                        format!("Name  {}", menu.settings.name),
                        format!("Overlay  {}", menu.settings.overlay.label()),
                        format!("Wrecks  {}", if menu.settings.wrecks { "On" } else { "Off" }),
                        "Controls".to_string(),
                        format!("Music  {}", menu.music_label()),
                        format!("Music volume  {}", menu.settings.music_volume),
                        format!("Effects volume  {}", menu.settings.effects_volume),
                    ],
                    _ => std::iter::once(format!("Original throttle  {}", menu.controls.throttle.label()))
                        .chain(
                            menu.controls.layouts[..LAYOUT_ROWS]
                                .iter()
                                .enumerate()
                                .map(|(i, l)| format!("{}P controls  {}", i + 1, l.label())),
                        )
                        .collect(),
                };
                p.column();
                let at = menu.cursor(screen);
                let bar = p.list(&rows, at);
                // A row with choices wears the arrows either side of its pill.
                let choice = match screen {
                    Screen::Settings => at == 1 || at == 2 || at >= SETTINGS_MUSIC,
                    _ => true,
                };
                if choice {
                    p.tex("arrow_left_off", rect(bar.x - 36.0, bar.y, 32.0, 32.0), CHOSEN);
                    p.tex("arrow_right_off", rect(bar.x + bar.w + 4.0, bar.y, 32.0, 32.0), CHOSEN);
                }
                if screen == Screen::Controls {
                    let pad = match &menu.controls.pad {
                        Some(dev) => format!("Pad  {dev}"),
                        None => "Pad  the first one found".to_string(),
                    };
                    p.text(Face::Small, &pad, LIST_X, item_y(CONTROLS_ROWS + 1), Align::Centre, ITEM);
                    let pressure = match menu.controls.pressure.as_str() {
                        "" => "Pressures  none".to_string(),
                        map => format!("Pressures  {map}"),
                    };
                    p.text(Face::Small, &pressure, LIST_X, item_y(CONTROLS_ROWS + 2), Align::Centre, ITEM);
                    p.text(Face::Small, "Pad and pressures: controls.txt", LIST_X, item_y(CONTROLS_ROWS + 3), Align::Centre, ITEM);
                }
                p.logo();
                p.title(screen.title());
                p.legend(&[("nav_circle_off", "Back"), ("nav_x_off", "Change")]);
            }
            Screen::Tracks => p.tracks(menu),
            Screen::Keyboard => p.keyboard(menu),
            Screen::Garage => p.garage(menu),
            Screen::Online => {
                let network = if menu.lobby.lan { "LAN only" } else { "Internet and LAN" };
                let rows: Vec<String> =
                    ONLINE.iter().map(|s| s.to_string()).chain([format!("Network  {network}")]).collect();
                p.column();
                p.list(&rows, menu.cursor(screen));
                p.logo();
                p.title(screen.title());
                p.legend(&[("nav_circle_off", "Back"), ("nav_x_off", "Select")]);
            }
            Screen::Join => p.join(menu),
            Screen::Split => {
                p.column();
                let rows: Vec<String> = layouts(menu.riders).into_iter().map(|l| layout_label(l).to_string()).collect();
                let at = menu.cursor(screen).min(rows.len() - 1);
                p.list(&rows, at);
                p.views(layouts(menu.riders)[at], menu.riders);
                p.logo();
                p.title(screen.title());
                p.legend(&[("nav_circle_off", "Back"), ("nav_x_off", "Select")]);
            }
            Screen::Games => p.games(menu),
            Screen::Lobby => p.lobby(menu),
            Screen::Splash => p.splash(menu),
        }
        // The ticker: a note when there is one, the network's word online, the rider's name otherwise
        // (not on the splash, where nobody is riding as anyone yet).
        let online = matches!(screen, Screen::Online | Screen::Games | Screen::Lobby);
        let ticker = match &menu.note {
            Some(n) => n.clone(),
            None if screen == Screen::Splash => return p.parts,
            None if online && !menu.lobby.status.is_empty() => menu.lobby.status.clone(),
            None => format!("Riding as {}", menu.settings.name),
        };
        p.anchor(Anchor::BottomLeft);
        p.text(Face::Small, &ticker, 50.0, 415.0, Align::Left, ITEM);
        p.parts
    }
}

/// The y of a list's row `i`: `mainmenu.lyt`'s items, thirty apart from 100.
fn item_y(i: usize) -> f32 {
    100.0 + 30.0 * i as f32
}

/// The y of the garage's row `i`: two lines each, so closer together than a list's.
fn garage_y(i: usize) -> f32 {
    88.0 + 38.0 * i as f32
}

fn garage_label(row: roster::Row) -> &'static str {
    match row {
        roster::Row::Class => "Class",
        roster::Row::Model => "Bike",
        roster::Row::Livery => "Paint",
        roster::Row::Tier => "Tier",
        roster::Row::Tuning => "Tuning",
        roster::Row::Rider => "Rider",
        roster::Row::Colours => "Colours",
    }
}

struct Painter<'a> {
    art: &'a Art,
    frame: &'a mut Frame,
    /// Frame pixels per layout pixel, the same both ways.
    s: f32,
    /// The whole frame in layout pixels: 640x448, or more one way.
    view: (f32, f32),
    /// Where the layout's origin sits for the part being drawn, by its [`Anchor`].
    at: (f32, f32),
    parts: Vec<Part>,
}

impl<'a> Painter<'a> {
    fn new(art: &'a Art, frame: &'a mut Frame) -> Painter<'a> {
        let (w, h) = (frame.width.max(1) as f32, frame.height.max(1) as f32);
        let s = (w / LAYOUT.0).min(h / LAYOUT.1);
        Painter { art, frame, s, view: (w / s, h / s), at: (0.0, 0.0), parts: Vec::new() }
    }

    /// Count the layout's numbers from `a` from here on.
    fn anchor(&mut self, a: Anchor) {
        let (dx, dy) = (self.view.0 - LAYOUT.0, self.view.1 - LAYOUT.1);
        self.at = match a {
            Anchor::TopLeft => (0.0, 0.0),
            Anchor::BottomLeft => (0.0, dy),
            Anchor::BottomRight => (dx, dy),
            Anchor::Centre => (dx / 2.0, dy / 2.0),
        };
    }

    /// The whole frame, in the current anchor's numbers.
    fn whole(&self) -> Rect {
        rect(-self.at.0, -self.at.1, self.view.0, self.view.1)
    }

    /// A layout box in frame pixels.
    fn px(&self, r: Rect) -> Rect {
        let s = self.s;
        rect((r.x + self.at.0) * s, (r.y + self.at.1) * s, r.w * s, r.h * s)
    }

    fn bounds(&self, r: Rect) -> Bounds {
        let p = self.px(r);
        [p.x, p.y, p.w, p.h]
    }

    /// Blend `colour` over a box, by its alpha.
    fn fill(&mut self, r: Rect, colour: [u8; 4]) {
        let p = self.px(r);
        let (x0, x1) = span(p.x, p.x + p.w, self.frame.width);
        let (y0, y1) = span(p.y, p.y + p.h, self.frame.height);
        for y in y0..y1 {
            for x in x0..x1 {
                blend(self.frame, x, y, colour);
            }
        }
    }

    /// `src` (texels, left top right bottom) of a texture, stretched over `r` and multiplied by
    /// `tint`, filtered bilinearly.
    fn blit(&mut self, t: &Texture, src: [f32; 4], r: Rect, tint: [u8; 4]) {
        self.blit_as(t, src, r, tint, blend_into);
    }

    /// [`Painter::blit`] with its own way of putting a texel down.
    fn blit_as(
        &mut self,
        t: &Texture,
        src: [f32; 4],
        r: Rect,
        tint: [u8; 4],
        put: fn(&mut [u8], [u8; 4]),
    ) {
        let p = self.px(r);
        let (fx0, fx1) = (p.x, p.x + p.w);
        let (fy0, fy1) = (p.y, p.y + p.h);
        let (x0, x1) = span(fx0, fx1, self.frame.width);
        let (y0, y1) = span(fy0, fy1, self.frame.height);
        let width = self.frame.width;
        // Rows `from..` of the frame, starting at `rows`' first byte.
        let paint = |rows: &mut [u8], from: usize| {
            for (dy, row) in rows.chunks_mut(width * 3).enumerate() {
                let y = from + dy;
                let v = src[1] + (y as f32 + 0.5 - fy0) / (fy1 - fy0) * (src[3] - src[1]);
                for x in x0..x1 {
                    let u = src[0] + (x as f32 + 0.5 - fx0) / (fx1 - fx0) * (src[2] - src[0]);
                    let c = bilinear(t, u, v, src);
                    let mul = |i: usize| (c[i] as u32 * tint[i] as u32 / 255) as u8;
                    put(&mut row[x * 3..x * 3 + 3], [mul(0), mul(1), mul(2), mul(3)]);
                }
            }
        };
        if y0 >= y1 {
            return;
        }
        let rows = &mut self.frame.color[y0 * width * 3..y1 * width * 3];
        // A big one asked for again over the same pixels is the pixels it made last time.
        let big = x1.saturating_sub(x0) * (y1 - y0) >= 1 << 15;
        let key = (
            t as *const Texture as usize,
            src.map(f32::to_bits),
            [x0, x1, y0, y1],
            tint,
            put as usize,
        );
        let mut blits = self.art.blits.lock().unwrap_or_else(|e| e.into_inner());
        if big {
            if let Some(done) = blits.iter().find(|b| b.key == key && b.before[..] == rows[..]) {
                rows.copy_from_slice(&done.after);
                return;
            }
        }
        let before = if big { rows.to_vec() } else { Vec::new() };
        // The background is a full-screen bilinear stretch, every frame: on one core it was most of
        // a menu frame at the window's own size (P47). Each band of rows is its own, so the bands
        // can be filled at once and the pixels are the same.
        // A thread is worth starting for about a 128x256 box of texels, not for a glyph.
        let threads = game_render::raster::threads().min(x1.saturating_sub(x0) * (y1 - y0) / (1 << 15));
        if threads <= 1 {
            paint(rows, y0);
        } else {
            let band = (y1 - y0).div_ceil(threads).max(8);
            std::thread::scope(|scope| {
                for (b, rows) in rows.chunks_mut(band * width * 3).enumerate() {
                    let paint = &paint;
                    scope.spawn(move || paint(rows, y0 + b * band));
                }
            });
        }
        if big {
            // A frame's worth of them at most: a screen draws a handful, and a new one, or the same
            // over other pixels, takes the place of the one that was there the longest.
            blits.retain(|b| b.key != key);
            if blits.len() >= 8 {
                blits.remove(0);
            }
            blits.push(Blitted { key, before, after: rows.to_vec() });
        }
    }

    fn tex(&mut self, name: &str, r: Rect, tint: [u8; 4]) {
        let Some(t) = self.art.textures.get(name) else {
            return;
        };
        let all = [0.0, 0.0, t.width as f32, t.height as f32];
        self.blit(t, all, r, tint);
    }

    /// A rounded bar over `r`: the texture's two ends (half its height each) kept their shape and
    /// only its middle stretched, so a short pill still has round ends.
    fn bar(&mut self, name: &str, r: Rect, tint: [u8; 4]) {
        let Some(t) = self.art.textures.get(name) else {
            return;
        };
        let (tw, th) = (t.width as f32, t.height as f32);
        let end_t = (th / 2.0).min(tw / 2.0);
        let end = (r.h / 2.0).min(r.w / 2.0);
        self.blit(t, [0.0, 0.0, end_t, th], rect(r.x, r.y, end, r.h), tint);
        self.blit(t, [end_t, 0.0, tw - end_t, th], rect(r.x + end, r.y, r.w - 2.0 * end, r.h), tint);
        self.blit(t, [tw - end_t, 0.0, tw, th], rect(r.x + r.w - end, r.y, end, r.h), tint);
    }

    /// `text` with its left edge, centre or right edge at `x` and its top at `y`; returns the box
    /// it took, as wide as the font measures it and as tall as a capital.
    fn text(&mut self, face: Face, text: &str, x: f32, y: f32, align: Align, colour: [u8; 4]) -> Rect {
        let Some(font) = self.art.fonts.get(&face) else {
            return rect(x, y, 0.0, 0.0);
        };
        let w = font.measure(text);
        let left = match align {
            Align::Left => x,
            Align::Centre => x - w / 2.0,
        };
        let m = font.glyph('M');
        let h = m.uv[3] - m.uv[1];
        let mut pen = left;
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            let g = font.glyph(c);
            pen += g.before;
            if c != ' ' {
                let h = g.uv[3] - g.uv[1];
                self.blit(&font.atlas, g.uv, rect(pen, y, g.width, h), colour);
            }
            pen += g.width + g.after;
            if let Some(&n) = chars.peek() {
                pen += g.kern.iter().find(|k| k.0 == n as u32).map_or(0.0, |k| k.1);
            }
        }
        rect(left, y, w, h)
    }

    /// The chosen label in the oblique face on a red pill [`PILL_PAD`] wider than it each side,
    /// centred on `x`, the pill's top at `top` and the text's at `y`; with the pill's shadow ten
    /// down and right when `shadow`. Returns the pill.
    fn pill(&mut self, text: &str, x: f32, y: f32, top: f32, shadow: bool) -> Rect {
        let w = self.art.fonts.get(&Face::Chosen).map_or(0.0, |f| f.measure(text));
        let bar = rect(x - w / 2.0 - PILL_PAD, top, w + 2.0 * PILL_PAD, 32.0);
        if shadow {
            let off = rect(bar.x + 10.0, bar.y + 10.0, bar.w, bar.h);
            self.bar("mainmenu_highlight_off_shadow", off, [0, 0, 0, 110]);
        }
        self.bar("mainmenu_highlight", bar, CHOSEN);
        let label = self.text(Face::Chosen, text, x, y, Align::Centre, CHOSEN);
        let part = Part::Pill { pill: self.bounds(bar), label: self.bounds(label) };
        self.parts.push(part);
        bar
    }

    /// A label in the plain face with nothing under it.
    fn item(&mut self, text: &str, x: f32, y: f32) {
        let label = self.text(Face::Item, text, x, y, Align::Centre, ITEM);
        let part = Part::Item(self.bounds(label));
        self.parts.push(part);
    }

    /// The grey gradient in orange over the whole frame, and the radial light added over it
    /// (`mxfrontendnavigation.lyt`'s Background, `frontend_background_radial` at 48 of 128), kept
    /// round and centred across.
    fn background(&mut self) {
        self.anchor(Anchor::TopLeft);
        self.tex("background", self.whole(), ORANGE);
        if let Some(t) = self.art.textures.get("frontend_background_radial") {
            let all = [0.0, 0.0, t.width as f32, t.height as f32];
            let r = rect((self.view.0 - 512.0) / 2.0, -40.0, 512.0, 448.0);
            self.blit_as(t, all, r, [255, 150, 80, 96], add);
        }
    }

    /// The logo and its shadow, `mainmenu.lyt`'s, drawn last so nothing covers it.
    fn logo(&mut self) {
        self.anchor(Anchor::TopLeft);
        let shadow = rect(LOGO.x + 10.0, LOGO.y + 10.0, LOGO.w, LOGO.h);
        self.tex("mainmenu_logo_shadow", shadow, [0, 0, 0, 40]);
        self.tex("mainmenu_logo", LOGO, CHOSEN);
        let part = Part::Logo(self.bounds(LOGO));
        self.parts.push(part);
    }

    /// The dark column the rows sit on, `mainmenu.lyt`'s, top to bottom of the frame, and its
    /// shadow ten to the right.
    fn column(&mut self) {
        self.anchor(Anchor::TopLeft);
        let h = self.view.1;
        self.tex("mainmenu_background", rect(117.0, 0.0, 256.0, h), [0, 0, 0, 60]);
        self.tex("mainmenu_background", rect(107.0, 0.0, 256.0, h), [255, 255, 255, 200]);
    }

    /// Rows centred on the list's line, the chosen one on its pill; returns the pill.
    fn list(&mut self, rows: &[String], chosen: usize) -> Rect {
        self.anchor(Anchor::TopLeft);
        let mut bar = rect(LIST_X, item_y(chosen) - 5.0, 0.0, 32.0);
        for (i, row) in rows.iter().enumerate() {
            if i == chosen {
                bar = self.pill(row, LIST_X, item_y(i) - 1.0, item_y(i) - 5.0, true);
            } else {
                self.item(row, LIST_X, item_y(i));
            }
        }
        bar
    }

    /// The screen's name, bottom left over the ticker, as the navigation layout's Screen Title.
    fn title(&mut self, title: &str) {
        self.anchor(Anchor::BottomLeft);
        self.text(Face::Title, title, 50.0, 362.0, Align::Left, CHOSEN);
    }

    /// The black bar along the bottom, and the controller legend in the bottom right corner, right
    /// to left from the frame's edge: `mxfrontendnavigation.lyt`'s Ticker and Navigation
    /// Background panels.
    fn legend(&mut self, buttons: &[(&str, &str)]) {
        self.anchor(Anchor::BottomLeft);
        self.fill(rect(0.0, 403.0, self.view.0, 45.0), BLACK);
        self.anchor(Anchor::BottomRight);
        let font = &self.art.fonts[&Face::Item];
        let mut widths = Vec::new();
        for (_, label) in buttons {
            widths.push(font.measure(label));
        }
        let total: f32 = widths.iter().map(|w| w + 34.0).sum::<f32>() + 16.0;
        let left = 640.0 - total;
        self.tex("nav_bkgcorner", rect(left - 64.0, 377.0, 64.0, 32.0), BLACK);
        self.fill(rect(left, 377.0, 640.0 - left, 26.0), BLACK);
        let mut x = left;
        for ((icon, label), w) in buttons.iter().zip(widths) {
            self.tex(icon, rect(x - 16.0, 358.0, 64.0, 64.0), CHOSEN);
            self.text(Face::Item, label, x + 30.0, 379.0, Align::Left, CHOSEN);
            x += w + 34.0;
        }
    }

    /// The track pick: `trackpreview.lyt`'s band of stills between the two masks, across the
    /// whole frame, the track's name over it between arrows.
    fn tracks(&mut self, menu: &Menu) {
        self.anchor(Anchor::TopLeft);
        let at = menu.cursor(Screen::Tracks);
        let (vw, vh) = self.view;
        self.fill(self.whole(), BLACK);
        let band = rect(0.0, 49.0, vw, vh - 49.0 - 90.0);
        let stills = &self.art.stills[at];
        if !stills.is_empty() {
            let still = &stills[(menu.tick / STILL_FRAMES) as usize % stills.len()];
            // The band's shape cut from the still's middle, never stretched.
            let (w, h) = (still.width as f32, still.height as f32);
            let rows = w * band.h / band.w;
            let src = if rows <= h {
                let top = (h - rows) / 2.0;
                [0.0, top, w, top + rows]
            } else {
                let cols = h * band.w / band.h;
                let left = (w - cols) / 2.0;
                [left, 0.0, left + cols, h]
            };
            self.blit(still, src, band, CHOSEN);
        }
        // The masks: a dark top band and the bottom one the legend sits on.
        self.fill(rect(0.0, 0.0, vw, 49.0), [0, 0, 0, 200]);
        self.fill(rect(0.0, vh - 90.0, vw, 90.0), [0, 0, 0, 200]);
        let (_, name) = TRACKS[at];
        let mid = vw / 2.0;
        self.text(Face::Title, name, mid, 8.0, Align::Centre, CHOSEN);
        let w = self.art.fonts[&Face::Title].measure(name);
        self.tex("arrow_left_off", rect(mid - w / 2.0 - 44.0, 9.0, 32.0, 32.0), CHOSEN);
        self.tex("arrow_right_off", rect(mid + w / 2.0 + 12.0, 9.0, 32.0, 32.0), CHOSEN);
        let count = format!("Freeride {} of {}", at + 1, TRACKS.len());
        self.text(Face::Small, &count, mid, vh - 118.0, Align::Centre, ITEM);
        // The machine each player starts on (P41), in a panel over the stills' right edge: one line
        // a rider, the one Square changes in the chosen colour between up and down arrows when there
        // is more than one.
        let riders = menu.riders.clamp(1, SEATS_MAX);
        let seat = menu.setup_seat.min(riders - 1);
        let left = vw - 200.0;
        self.fill(rect(left, 60.0, 180.0, 12.0 + 22.0 * riders as f32), [0, 0, 0, 170]);
        for i in 0..riders {
            let y = 66.0 + 22.0 * i as f32;
            let line = match riders {
                1 => format!("Machine  {}", menu.settings.machines[i].label()),
                _ => format!("{}P  {}", i + 1, menu.settings.machines[i].label()),
            };
            let colour = if i == seat { CHOSEN } else { ITEM };
            self.text(Face::Small, &line, left + 16.0, y, Align::Left, colour);
        }
        if riders > 1 {
            let y = 66.0 + 22.0 * seat as f32 - 6.0;
            self.tex("arrow_up_off", rect(left + 150.0, y - 8.0, 24.0, 24.0), CHOSEN);
            self.tex("arrow_down_off", rect(left + 150.0, y + 8.0, 24.0, 24.0), CHOSEN);
        }
        self.title(Screen::Tracks.title());
        self.legend(&[("nav_square_off", "Machine"), ("nav_circle_off", "Back"), ("nav_x_off", "Ride")]);
    }

    /// The garage: a column per seated player, opentennis's roster columns in the main menu's dark
    /// column and red bar, each row a name over its value between arrows on the chosen one.
    fn garage(&mut self, menu: &Menu) {
        self.anchor(Anchor::TopLeft);
        let n = menu.seated.clamp(1, roster::SEATS);
        // The columns share 560 of the 640 across, centred in the frame, the one-seat column as
        // wide as the settings'.
        let (vw, vh) = self.view;
        let w = (560.0 / n as f32).min(448.0);
        let left = vw / 2.0 - w * n as f32 / 2.0;
        for seat in 0..n {
            let x = left + w * seat as f32;
            let centre = x + w / 2.0;
            self.tex("mainmenu_background", rect(x + 14.0, 0.0, w - 8.0, vh), [0, 0, 0, 60]);
            self.tex("mainmenu_background", rect(x + 4.0, 0.0, w - 8.0, vh), [255, 255, 255, 200]);
            self.text(Face::Item, &format!("Player {}", seat + 1), centre, 64.0, Align::Centre, CHOSEN);
            let Some(g) = menu.garage(seat) else { continue };
            let bar = w - 16.0;
            let y = garage_y(g.row) - 3.0;
            self.tex("mainmenu_highlight_off_shadow", rect(x + 18.0, y + 10.0, bar, 40.0), [0, 0, 0, 110]);
            self.tex("mainmenu_highlight", rect(x + 8.0, y, bar, 40.0), CHOSEN);
            for (i, row) in roster::ROWS.iter().enumerate() {
                let on = i == g.row;
                let y = garage_y(i);
                let colour = if on { CHOSEN } else { ITEM };
                self.text(Face::Small, garage_label(*row), centre, y, Align::Centre, colour);
                let value = g.value(&menu.catalogue, *row);
                let face = if on { Face::Chosen } else { Face::Item };
                self.text(face, &value, centre, y + 13.0, Align::Centre, colour);
                if on {
                    self.tex("arrow_left_off", rect(x + 6.0, y + 4.0, 24.0, 24.0), CHOSEN);
                    self.tex("arrow_right_off", rect(x + w - 30.0, y + 4.0, 24.0, 24.0), CHOSEN);
                }
            }
        }
        self.title(Screen::Garage.title());
        self.legend(&[("nav_x_off", "Done")]);
    }

    /// The splash (P46): a track's still from the disc, dimmed, the logo over it, and what to
    /// press — or, once someone has, who 1P is — and the Quit popup when Back asked for it.
    fn splash(&mut self, menu: &Menu) {
        // The picture covers the whole frame at any shape (V24); the rest is the layout's, centred.
        self.anchor(Anchor::Centre);
        let whole = self.whole();
        self.fill(whole, BLACK);
        // The last track ridden's first still. (The disc's own `titlescreen` has its logo and a
        // block of small print baked in, which the prompt would sit on.)
        let track = menu.settings.track_index();
        let still = self.art.stills.get(track).and_then(|s| s.first());
        if let Some(t) = still {
            // Cover the screen with the picture's middle, keeping its shape.
            let (w, h) = (t.width as f32, t.height as f32);
            let fit = whole.w / whole.h;
            let src = if w / h > fit {
                let cw = h * fit;
                [(w - cw) / 2.0, 0.0, (w + cw) / 2.0, h]
            } else {
                let ch = w / fit;
                [0.0, (h - ch) / 2.0, w, (h + ch) / 2.0]
            };
            self.blit(t, src, whole, SPLASH_DIM);
        }
        // The logo a quarter up on the main menu's, its lettering (the left 0.68 of the texture)
        // centred.
        let (w, h) = (512.0 * 1.25, 64.0 * 1.25);
        let x = 320.0 - w * 0.68 / 2.0;
        self.tex("mainmenu_logo_shadow", rect(x + 10.0, 100.0, w, h), [0, 0, 0, 90]);
        self.tex("mainmenu_logo", rect(x, 90.0, w, h), CHOSEN);
        if menu.claimed.is_some() || menu.p1.is_some() {
            let who = format!("Player 1  {}", menu.p1_name);
            self.text(Face::Title, &who, 320.0, 270.0, Align::Centre, ORANGE_TEXT);
        } else {
            // The prompt breathes until it is answered.
            let breath = (menu.tick as f32 * 2.6 / 60.0).cos().abs();
            let a = (255.0 * (0.6 + 0.4 * breath)) as u8;
            let colour = [255, 255, 255, a];
            let font = &self.art.fonts[&Face::Title];
            let (press, or) = (font.measure("Press"), font.measure("or"));
            let w = press + 12.0 + 40.0 + 12.0 + or + 12.0 + 40.0;
            let mut x = 320.0 - w / 2.0;
            let y = 270.0;
            self.text(Face::Title, "Press", x, y, Align::Left, colour);
            x += press + 12.0;
            self.tex("nav_start_off", rect(x - 12.0, y - 12.0, 64.0, 64.0), colour);
            x += 40.0 + 12.0;
            self.text(Face::Title, "or", x, y, Align::Left, colour);
            x += or + 12.0;
            self.tex("nav_x_off", rect(x - 12.0, y - 12.0, 64.0, 64.0), colour);
            self.text(Face::Small, "Enter or Space on the keyboard", 320.0, 316.0, Align::Centre, ITEM);
            self.text(Face::Small, "Whoever presses it rides as Player 1", 320.0, 338.0, Align::Centre, ITEM);
            if !menu.heard.is_empty() {
                let line = format!("Controllers: {}", names_line(&menu.heard));
                self.text(Face::Small, &line, 320.0, 372.0, Align::Centre, ITEM);
            }
            if !menu.skipped.is_empty() {
                let line = format!("Not read: {}", names_line(&menu.skipped));
                self.text(Face::Small, &line, 320.0, 394.0, Align::Centre, [255, 255, 255, 120]);
            }
        }
        if menu.p1.is_none() {
            self.legend(&[("nav_circle_off", "Quit"), ("nav_x_off", "Start")]);
        }
        if let Some(row) = menu.quitting() {
            self.anchor(Anchor::Centre);
            self.fill(self.whole(), [0, 0, 0, 150]);
            self.fill(rect(170.0, 150.0, 300.0, 140.0), [0, 0, 0, 220]);
            self.text(Face::Chosen, "Quit the game?", 320.0, 164.0, Align::Centre, CHOSEN);
            for (i, label) in QUIT_ROWS.iter().enumerate() {
                let y = 210.0 + 36.0 * i as f32;
                if i == row {
                    self.tex("mainmenu_highlight", rect(320.0 - 96.0, y - 5.0, 192.0, 32.0), CHOSEN);
                    self.text(Face::Chosen, label, 320.0, y - 1.0, Align::Centre, CHOSEN);
                } else {
                    self.text(Face::Item, label, 320.0, y, Align::Centre, ITEM);
                }
            }
        }
    }

    /// The join screen (P43): a card a seat across the frame, the garage's columns, each saying who
    /// is on it — a controller's name on the red bar, the same dimmed when it is gone, or the button
    /// that joins an open one — with arrows either side of a seated card that has room to move.
    fn join(&mut self, menu: &Menu) {
        self.anchor(Anchor::TopLeft);
        let seats = &menu.join.seats;
        let n = seats.len();
        let (vw, vh) = self.view;
        let w = (560.0 / n as f32).min(448.0);
        let left = vw / 2.0 - w * n as f32 / 2.0;
        let name = |d: &usize| menu.devices.get(*d).cloned().unwrap_or_else(|| format!("Pad {}", d + 1));
        let open = |i: usize| seats[i] == join::Seat::Open;
        for (i, seat) in seats.iter().enumerate() {
            let x = left + w * i as f32;
            let centre = x + w / 2.0;
            self.tex("mainmenu_background", rect(x + 14.0, 0.0, w - 8.0, vh), [0, 0, 0, 60]);
            self.tex("mainmenu_background", rect(x + 4.0, 0.0, w - 8.0, vh), [255, 255, 255, 200]);
            self.text(Face::Item, &format!("Player {}", i + 1), centre, 120.0, Align::Centre, CHOSEN);
            let y = 160.0;
            match seat {
                join::Seat::Open => {
                    self.tex("nav_x_off", rect(centre - 32.0, y - 18.0, 64.0, 64.0), CHOSEN);
                    self.text(Face::Small, "to join", centre, y + 36.0, Align::Centre, ITEM);
                }
                join::Seat::Taken(d) | join::Seat::Gone(d) => {
                    let gone = matches!(seat, join::Seat::Gone(_));
                    let bar = w - 16.0;
                    let tint = if gone { [120, 120, 120, 160] } else { CHOSEN };
                    self.tex("mainmenu_highlight_off_shadow", rect(x + 18.0, y + 7.0, bar, 32.0), [0, 0, 0, 110]);
                    self.tex("mainmenu_highlight", rect(x + 8.0, y - 3.0, bar, 32.0), tint);
                    let face = if gone { Face::Item } else { Face::Chosen };
                    let colour = if gone { ITEM } else { CHOSEN };
                    // A pad's name can be long; the card shows what fits of it.
                    let mut label = name(d);
                    let room = bar - 16.0;
                    while label.chars().count() > 1 && self.art.fonts[&face].measure(&label) > room {
                        label.pop();
                    }
                    self.text(face, &label, centre, y, Align::Centre, colour);
                    if gone {
                        self.text(Face::Small, "Gone - plug it back in", centre, y + 36.0, Align::Centre, ITEM);
                    } else {
                        if (0..i).any(open) {
                            self.tex("arrow_left_off", rect(x + 6.0, y + 36.0, 24.0, 24.0), CHOSEN);
                        }
                        if (i + 1..n).any(open) {
                            self.tex("arrow_right_off", rect(x + w - 30.0, y + 36.0, 24.0, 24.0), CHOSEN);
                        }
                    }
                }
            }
        }
        self.title(Screen::Join.title());
        let mut buttons = vec![("nav_circle_off", "Leave"), ("nav_x_off", "Join")];
        if seats.iter().any(|s| matches!(s, join::Seat::Gone(_))) {
            buttons.insert(0, ("nav_square_off", "Let Go"));
        }
        buttons.push(("nav_start_off", "Ride"));
        self.legend(&buttons);
    }

    /// The split layout's picture, right of the list: the frame's shape cut into the layout's
    /// views, each numbered for its player, and the views nobody has dark.
    fn views(&mut self, layout: split::Layout, players: usize) {
        self.anchor(Anchor::TopLeft);
        let (fw, fh) = (224usize, 156usize);
        let (x0, y0) = (390.0, 120.0);
        self.fill(rect(x0 - 4.0, y0 - 4.0, fw as f32 + 8.0, fh as f32 + 8.0), BLACK);
        for (i, r) in layout.rects(fw, fh).into_iter().enumerate() {
            let at = rect(x0 + r.x as f32 + 2.0, y0 + r.y as f32 + 2.0, r.w as f32 - 4.0, r.h as f32 - 4.0);
            match i < players {
                true => {
                    self.fill(at, [150, 72, 30, 255]);
                    let label = format!("{}P", i + 1);
                    self.text(Face::Item, &label, at.x + at.w / 2.0, at.y + at.h / 2.0 - 8.0, Align::Centre, CHOSEN);
                }
                false => self.fill(at, [40, 40, 40, 255]),
            }
        }
    }

    /// The on-screen keyboard, in the middle of the frame: the name being typed, the four rows of
    /// ten and the commands.
    fn keyboard(&mut self, menu: &Menu) {
        self.anchor(Anchor::Centre);
        self.fill(rect(90.0, 60.0, 460.0, 290.0), [0, 0, 0, 200]);
        let typed = format!("{}_", menu.typing());
        self.fill(rect(120.0, 76.0, 400.0, 32.0), [60, 0, 0, 255]);
        self.text(Face::Chosen, &typed, 132.0, 80.0, Align::Left, CHOSEN);
        let (row, col) = menu.key;
        for (r, keys) in KEY_ROWS.iter().enumerate() {
            for (c, k) in keys.chars().enumerate() {
                let (x, y) = (120.0 + 40.0 * c as f32, 124.0 + 40.0 * r as f32);
                if (r, c) == (row, col) {
                    self.pill(&k.to_string(), x + 20.0, y + 4.0, y - 2.0, false);
                } else {
                    self.item(&k.to_string(), x + 20.0, y + 4.0);
                }
            }
        }
        for (c, label) in KEY_COMMANDS.iter().enumerate() {
            let (x, y) = (120.0 + 100.0 * c as f32, 290.0);
            if (KEY_ROWS.len(), c) == (row, col) {
                self.pill(label, x + 50.0, y, y - 4.0, false);
            } else {
                self.item(label, x + 50.0, y);
            }
        }
        self.title(menu.typing_for.title());
        self.legend(&[
            ("nav_square_off", "Delete"),
            ("nav_circle_off", "Cancel"),
            ("nav_x_off", "Type"),
        ]);
    }
}

impl Painter<'_> {
    /// The lobbies Find turned up, a row each, and Refresh under them.
    fn games(&mut self, menu: &Menu) {
        let found = &menu.lobby.found;
        let rows: Vec<String> = found
            .iter()
            .map(|l| {
                let track = TRACKS.iter().find(|t| t.0 == l.track).map_or(l.track.as_str(), |t| t.1);
                let build = if l.same_build() { "" } else { "  (another version)" };
                format!("{}  {track}  {}/{}{build}", l.name, l.taken, l.seats)
            })
            .chain(["Refresh".to_string()])
            .collect();
        self.column();
        self.list(&rows, menu.cursor(Screen::Games));
        if found.is_empty() {
            self.text(Face::Small, "No games listed yet: Refresh to look again.", LIST_X, item_y(2), Align::Centre, ITEM);
        }
        self.logo();
        self.title(Screen::Games.title());
        self.legend(&[("nav_circle_off", "Back"), ("nav_x_off", "Join")]);
    }

    /// The lobby: its rows on the left, the code, the seats and the chat on the right.
    fn lobby(&mut self, menu: &Menu) {
        let l = &menu.lobby;
        let rows: Vec<String> = l.items().iter().map(|i| i.label(l)).collect();
        self.column();
        self.list(&rows, menu.cursor(Screen::Lobby));
        // The panel sits right of the column and takes what the window has to spare.
        let x = 383.0;
        self.fill(rect(x, 92.0, (self.view.0 - x - 10.0).max(237.0), 262.0), [0, 0, 0, 170]);
        let code = match (&l.room, l.code.as_str()) {
            (None, _) => "Not in a lobby yet".to_string(),
            (Some(_), "") => "Room code: waiting".to_string(),
            (Some(_), code) => format!("Room code  {code}"),
        };
        self.text(Face::Item, &code, x + 12.0, 98.0, Align::Left, CHOSEN);
        if let Some(room) = &l.room {
            for i in 0..room.seats.len() {
                let line = l.seat_line(room, i);
                let colour = if room.seats[i].as_ref().is_some_and(|s| s.peer == l.me) { CHOSEN } else { ITEM };
                self.text(Face::Small, &line, x + 12.0, 128.0 + 20.0 * i as f32, Align::Left, colour);
            }
        }
        for (i, line) in l.chat.iter().enumerate() {
            self.text(Face::Small, line, x + 12.0, 254.0 + 18.0 * i as f32, Align::Left, ITEM);
        }
        self.logo();
        self.title(Screen::Lobby.title());
        let ok = match l.items().get(menu.cursor(Screen::Lobby)) {
            Some(Item::Start) => "Start",
            Some(Item::Ready) => "Ready",
            Some(Item::Leave) => "Leave",
            Some(Item::Chat) => "Chat",
            _ => "Change",
        };
        self.legend(&[("nav_circle_off", "Leave"), ("nav_x_off", ok)]);
    }
}

/// The pixels a span of layout covers, clipped to the frame.
pub(crate) fn span(a: f32, b: f32, n: usize) -> (usize, usize) {
    (a.round().clamp(0.0, n as f32) as usize, b.round().clamp(0.0, n as f32) as usize)
}

pub(crate) fn blend(frame: &mut Frame, x: usize, y: usize, c: [u8; 4]) {
    let at = 3 * (y * frame.width + x);
    blend_into(&mut frame.color[at..at + 3], c);
}

/// [`blend`], into one pixel's three bytes.
fn blend_into(px: &mut [u8], c: [u8; 4]) {
    let a = c[3] as u32;
    for i in 0..3 {
        let d = px[i] as u32;
        px[i] = ((c[i] as u32 * a + d * (255 - a)) / 255) as u8;
    }
}

/// Light added by its alpha, which is how the radial glow brightens and never darkens: into one
/// pixel's three bytes.
fn add(px: &mut [u8], c: [u8; 4]) {
    for i in 0..3 {
        let d = px[i] as u32 + c[i] as u32 * c[3] as u32 / 255;
        px[i] = d.min(255) as u8;
    }
}

/// The texel at `u, v` (texels), blended from its four neighbours and kept inside `src` so a glyph
/// does not bleed in its neighbour in the atlas.
pub(crate) fn bilinear(t: &Texture, u: f32, v: f32, src: [f32; 4]) -> [u8; 4] {
    let lo_x = src[0].min(src[2]).max(0.0);
    let hi_x = (src[0].max(src[2]) - 1.0).min(t.width as f32 - 1.0).max(lo_x);
    let lo_y = src[1].min(src[3]).max(0.0);
    let hi_y = (src[1].max(src[3]) - 1.0).min(t.height as f32 - 1.0).max(lo_y);
    let u = (u - 0.5).clamp(lo_x, hi_x);
    let v = (v - 0.5).clamp(lo_y, hi_y);
    // Both are clamped to 0 or more, where the cast is the floor (and the library's floor a call).
    let (x0, y0) = (u as usize, v as usize);
    let (x1, y1) = ((x0 + 1).min(hi_x as usize), (y0 + 1).min(hi_y as usize));
    let (fx, fy) = (u - x0 as f32, v - y0 as f32);
    let px = |x: usize, y: usize| {
        let at = 4 * (y * t.width + x);
        [t.rgba[at], t.rgba[at + 1], t.rgba[at + 2], t.rgba[at + 3]]
    };
    let (a, b, c, d) = (px(x0, y0), px(x1, y0), px(x0, y1), px(x1, y1));
    let mut out = [0u8; 4];
    for i in 0..4 {
        let top = a[i] as f32 * (1.0 - fx) + b[i] as f32 * fx;
        let bottom = c[i] as f32 * (1.0 - fx) + d[i] as f32 * fx;
        out[i] = round(top * (1.0 - fy) + bottom * fy);
    }
    out
}

/// `x.round() as u8` for an `x` in 0..=255, without the library call: the fraction left by the
/// truncating cast is exact, and half rounds up, as `round` rounds half away from zero.
fn round(x: f32) -> u8 {
    let t = x as u32;
    (t + (x - t as f32 >= 0.5) as u32) as u8
}

#[cfg(test)]
mod tests {
    use super::round;

    #[test]
    fn the_inline_round_is_the_library_round() {
        // Every 997th f32 from 0 to 255, and every half and its neighbours.
        let mut x = 0.0f32;
        while x <= 255.0 {
            assert_eq!(round(x), x.round() as u8, "{x}");
            x = f32::from_bits(x.to_bits() + 997);
        }
        for n in 0..255 {
            let half = n as f32 + 0.5;
            for y in [f32::from_bits(half.to_bits() - 1), half, f32::from_bits(half.to_bits() + 1)] {
                assert_eq!(round(y), y.round() as u8, "{y}");
            }
        }
    }
}
