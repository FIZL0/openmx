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

use game_render::{Disc, Frame, Texture};
use game_sim::input::{AXIS_CENTRE, Control, Report};
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
        // A pad that is not there holds nothing.
        let now = if r.status() == 0xff { 0 } else { r.buttons() | stick(r) };
        let Some(last) = self.last.replace(now) else {
            return Press::default();
        };
        let dirs = Control::Up.mask() | Control::Down.mask() | Control::Left.mask() | Control::Right.mask();
        let mut edge = now & !last;
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
    /// The riding overlay (`hud`), which H toggles in a ride.
    pub overlay: bool,
    /// Whether a ride wrecks at all.
    pub wrecks: bool,
    /// The last track ridden, which the track pick opens on.
    pub track: String,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            name: "Rider".into(),
            overlay: false,
            wrecks: true,
            track: TRACKS[0].0.into(),
        }
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
                "overlay" => s.overlay = v != "off",
                "wrecks" => s.wrecks = v != "off",
                "track" if TRACKS.iter().any(|t| t.0 == v) => s.track = v.to_string(),
                _ => {}
            }
        }
        s
    }

    pub fn text(&self) -> String {
        format!(
            "# openmx's front end (P36)\nname = {}\noverlay = {}\nwrecks = {}\ntrack = {}\n",
            self.name,
            on_off(self.overlay),
            on_off(self.wrecks),
            self.track
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
             # Cross and Square kept free; original is the game's own (P40)\n\
             throttle = {}\npad = {}\npressure = {}\nlayout = {}\n",
            self.throttle.word(),
            self.pad.as_deref().unwrap_or("auto"),
            if self.pressure.is_empty() { "none" } else { &self.pressure },
            self.layout_text(),
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
}

impl Screen {
    pub const ALL: [Screen; 7] = [
        Screen::Splash,
        Screen::Main,
        Screen::Tracks,
        Screen::Settings,
        Screen::Controls,
        Screen::Keyboard,
        Screen::Pause,
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
        }
    }
}

/// The main menu's rows. P37's garage and P38's join screen go in here.
const MAIN: [&str; 3] = ["Free Ride", "Settings", "Quit"];
const PAUSE: [&str; 5] = ["Resume", "Restart", "Settings", "Main Menu", "Quit"];
const SETTINGS_ROWS: usize = 4;
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
        self.0.retain(|(d, _)| reports.iter().any(|(r, _)| r == d));
        reports
            .iter()
            .map(|(dev, report)| {
                let at = match self.0.iter().position(|(d, _)| d == dev) {
                    Some(at) => at,
                    None => {
                        self.0.push((dev.clone(), Reader::new()));
                        self.0.len() - 1
                    }
                };
                (dev.clone(), self.0[at].1.read(report))
            })
            .collect()
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
    Quit,
}

/// The front end's state: which screens are open, where each one's cursor is, and the settings it
/// edits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu {
    pub settings: Settings,
    pub controls: Controls,
    /// Open screens, the one shown last. Empty is a ride with nothing over it.
    stack: Vec<Screen>,
    /// Each screen's row, in [`Screen::ALL`]'s order.
    cursor: [usize; Screen::ALL.len()],
    /// The keyboard's text while it is being typed, and its cursor (row, column).
    typing: String,
    key: (usize, usize),
    /// Frames shown, for the track stills' slideshow.
    pub tick: u32,
    /// The settings changed since [`Menu::take_changed`] last said so.
    changed: bool,
    /// A line for the ticker, which is how a ride that could not start says why.
    pub note: Option<String>,
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
            key: (0, 0),
            tick: 0,
            changed: false,
            note: None,
            p1: None,
            p1_name: String::new(),
            claimed: None,
            quit: None,
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
    }

    /// The pause over a ride.
    pub fn pause(&mut self) {
        self.stack = vec![Screen::Pause];
        self.cursor[Screen::Pause.index()] = 0;
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
        }
    }

    fn push(&mut self, screen: Screen) {
        self.stack.push(screen);
    }

    fn pop(&mut self) {
        self.stack.pop();
    }

    fn open_keyboard(&mut self) {
        self.typing = self.settings.name.clone();
        self.key = (0, 0);
        self.push(Screen::Keyboard);
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
        let rows = self.rows(screen);
        let at = self.cursor(screen);
        // Up and down walk the rows, wrapping, except on the track pick where left and right do.
        let (prev, next) = match screen {
            Screen::Tracks => (p.left || p.up, p.right || p.down),
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
                (true, _, 0) => {
                    self.cursor[Screen::Tracks.index()] = self.settings.track_index();
                    self.tick = 0;
                    self.push(Screen::Tracks);
                }
                (true, _, 1) => self.push(Screen::Settings),
                (true, _, _) => return Act::Quit,
                _ => {}
            },
            Screen::Tracks => {
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
                            self.settings.overlay = !self.settings.overlay;
                            self.changed = true;
                        }
                        2 => {
                            self.settings.wrecks = !self.settings.wrecks;
                            self.changed = true;
                        }
                        3 if p.ok => self.push(Screen::Controls),
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
                        2 => self.push(Screen::Settings),
                        3 => {
                            self.main();
                            return Act::MainMenu;
                        }
                        _ => return Act::Quit,
                    }
                }
            }
            Screen::Keyboard | Screen::Splash => unreachable!(),
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
            self.finish_name();
        } else if p.ok {
            if row < last {
                if self.typing.chars().count() < NAME_MAX {
                    let c = KEY_ROWS[row].chars().nth(col).unwrap();
                    // The first letter of each word in capitals, as the original's name entry.
                    let word_start = self.typing.is_empty() || self.typing.ends_with(' ');
                    self.typing.push(if word_start { c.to_ascii_uppercase() } else { c });
                }
            } else {
                match col {
                    0 => {
                        if !self.typing.is_empty() && self.typing.chars().count() < NAME_MAX {
                            self.typing.push(' ');
                        }
                    }
                    1 => {
                        self.typing.pop();
                    }
                    2 => self.typing.clear(),
                    _ => self.finish_name(),
                }
            }
        }
        Act::Stay
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
}

// ---- the art -------------------------------------------------------------------------------------

/// One character of a `.cel` font: its box in the atlas and how far it moves the pen.
#[derive(Clone, Debug)]
struct Glyph {
    /// Left, top, right, bottom in atlas texels.
    uv: [f32; 4],
    /// Pen to the box's left edge, the box's width, and the box's right edge to the next pen.
    before: f32,
    width: f32,
    after: f32,
    /// The following character, and how much nearer (negative) it sits.
    kern: Vec<(u32, f32)>,
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
struct Font {
    glyphs: HashMap<u32, Glyph>,
    atlas: Texture,
}

fn le16(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

fn le32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

impl Font {
    fn parse(cel: &[u8], atlas: Texture) -> Result<Font, String> {
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

    fn glyph(&self, c: char) -> &Glyph {
        self.glyphs
            .get(&(c as u32))
            .or_else(|| self.glyphs.get(&0))
            .unwrap_or_else(|| self.glyphs.values().next().unwrap())
    }

    /// How wide `text` is, in layout pixels.
    fn measure(&self, text: &str) -> f32 {
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

/// The layout's screen.
const LAYOUT: (f32, f32) = (640.0, 448.0);

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
        Ok(Art { textures, fonts, stills })
    }

    /// Draw the menu's screen into `frame`. In a ride the frame already holds the ride, which the
    /// pause darkens rather than covers; otherwise the menu draws its own background.
    pub fn draw(&self, menu: &Menu, frame: &mut Frame) {
        let Some(screen) = menu.screen() else {
            return;
        };
        let mut p = Painter { art: self, frame };
        match screen {
            Screen::Pause => p.fill(rect(0.0, 0.0, 640.0, 448.0), [0, 0, 0, 140]),
            Screen::Tracks | Screen::Splash => {}
            _ => p.background(),
        }
        match screen {
            Screen::Main => {
                p.logo();
                p.column(107.0, 1.0);
                p.list(185.0, 1.0, &MAIN.map(|s| s.to_string()), menu.cursor(screen));
                p.legend(&[("nav_x_off", "Select")]);
            }
            Screen::Pause => {
                p.logo();
                p.column(107.0, 1.0);
                p.list(185.0, 1.0, &PAUSE.map(|s| s.to_string()), menu.cursor(screen));
                p.title(screen.title());
                p.legend(&[("nav_circle_off", "Resume"), ("nav_x_off", "Select")]);
            }
            Screen::Settings | Screen::Controls => {
                let rows: Vec<String> = match screen {
                    Screen::Settings => vec![
                        format!("Name  {}", menu.settings.name),
                        format!("Overlay  {}", if menu.settings.overlay { "On" } else { "Off" }),
                        format!("Wrecks  {}", if menu.settings.wrecks { "On" } else { "Off" }),
                        "Controls".to_string(),
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
                p.logo();
                p.column(107.0, 1.75);
                let at = menu.cursor(screen);
                p.list(242.0, 1.75, &rows, at);
                // A row with choices wears the arrows either side of it.
                let choice = match screen {
                    Screen::Settings => at == 1 || at == 2,
                    _ => true,
                };
                if choice {
                    let y = item_y(at) - 5.0;
                    p.tex("arrow_left_off", rect(84.0, y, 32.0, 32.0), CHOSEN);
                    p.tex("arrow_right_off", rect(368.0, y, 32.0, 32.0), CHOSEN);
                }
                if screen == Screen::Controls {
                    let pad = match &menu.controls.pad {
                        Some(dev) => format!("Pad  {dev}"),
                        None => "Pad  the first one found".to_string(),
                    };
                    p.text(Face::Small, &pad, 242.0, item_y(CONTROLS_ROWS + 1), Align::Centre, ITEM);
                    let pressure = match menu.controls.pressure.as_str() {
                        "" => "Pressures  none".to_string(),
                        map => format!("Pressures  {map}"),
                    };
                    p.text(Face::Small, &pressure, 242.0, item_y(CONTROLS_ROWS + 2), Align::Centre, ITEM);
                    p.text(Face::Small, "Pad and pressures: controls.txt", 242.0, item_y(CONTROLS_ROWS + 3), Align::Centre, ITEM);
                }
                p.title(screen.title());
                p.legend(&[("nav_circle_off", "Back"), ("nav_x_off", "Change")]);
            }
            Screen::Tracks => p.tracks(menu),
            Screen::Keyboard => p.keyboard(menu),
            Screen::Splash => p.splash(menu),
        }
        // The ticker: a note when there is one, the rider's name otherwise (not on the splash, where
        // nobody is riding as anyone yet).
        let ticker = match &menu.note {
            Some(n) => n.clone(),
            None if screen == Screen::Splash => return,
            None => format!("Riding as {}", menu.settings.name),
        };
        p.text(Face::Small, &ticker, 50.0, 415.0, Align::Left, ITEM);
    }
}

/// The y of a list's row `i`: `mainmenu.lyt`'s items, thirty apart from 100.
fn item_y(i: usize) -> f32 {
    100.0 + 30.0 * i as f32
}

struct Painter<'a> {
    art: &'a Art,
    frame: &'a mut Frame,
}

impl Painter<'_> {
    fn scale(&self) -> (f32, f32) {
        (self.frame.width as f32 / LAYOUT.0, self.frame.height as f32 / LAYOUT.1)
    }

    /// Blend `colour` over a box, by its alpha.
    fn fill(&mut self, r: Rect, colour: [u8; 4]) {
        let (sx, sy) = self.scale();
        let (x0, x1) = span(r.x * sx, (r.x + r.w) * sx, self.frame.width);
        let (y0, y1) = span(r.y * sy, (r.y + r.h) * sy, self.frame.height);
        for y in y0..y1 {
            for x in x0..x1 {
                blend(self.frame, x, y, colour);
            }
        }
    }

    /// `src` (texels, left top right bottom) of a texture, stretched over `r` and multiplied by
    /// `tint`, filtered bilinearly.
    fn blit(&mut self, t: &Texture, src: [f32; 4], r: Rect, tint: [u8; 4]) {
        self.blit_as(t, src, r, tint, blend);
    }

    /// [`Painter::blit`] with its own way of putting a texel down.
    fn blit_as(
        &mut self,
        t: &Texture,
        src: [f32; 4],
        r: Rect,
        tint: [u8; 4],
        put: fn(&mut Frame, usize, usize, [u8; 4]),
    ) {
        let (sx, sy) = self.scale();
        let (fx0, fx1) = (r.x * sx, (r.x + r.w) * sx);
        let (fy0, fy1) = (r.y * sy, (r.y + r.h) * sy);
        let (x0, x1) = span(fx0, fx1, self.frame.width);
        let (y0, y1) = span(fy0, fy1, self.frame.height);
        for y in y0..y1 {
            let v = src[1] + (y as f32 + 0.5 - fy0) / (fy1 - fy0) * (src[3] - src[1]);
            for x in x0..x1 {
                let u = src[0] + (x as f32 + 0.5 - fx0) / (fx1 - fx0) * (src[2] - src[0]);
                let c = bilinear(t, u, v, src);
                let mul = |i: usize| (c[i] as u32 * tint[i] as u32 / 255) as u8;
                put(self.frame, x, y, [mul(0), mul(1), mul(2), mul(3)]);
            }
        }
    }

    fn tex(&mut self, name: &str, r: Rect, tint: [u8; 4]) {
        let Some(t) = self.art.textures.get(name) else {
            return;
        };
        let all = [0.0, 0.0, t.width as f32, t.height as f32];
        self.blit(t, all, r, tint);
    }

    /// `text` with its left edge, centre or right edge at `x` and its top at `y`.
    fn text(&mut self, face: Face, text: &str, x: f32, y: f32, align: Align, colour: [u8; 4]) {
        let Some(font) = self.art.fonts.get(&face) else {
            return;
        };
        let w = font.measure(text);
        let mut pen = match align {
            Align::Left => x,
            Align::Centre => x - w / 2.0,
        };
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
    }

    /// The grey gradient in orange, and the radial light added over it (`mxfrontendnavigation.lyt`'s
    /// Background, `frontend_background_radial` at 48 of 128).
    fn background(&mut self) {
        self.tex("background", rect(0.0, 0.0, 640.0, 448.0), ORANGE);
        if let Some(t) = self.art.textures.get("frontend_background_radial") {
            let all = [0.0, 0.0, t.width as f32, t.height as f32];
            self.blit_as(t, all, rect(64.0, -40.0, 512.0, 448.0), [255, 150, 80, 96], add);
        }
    }

    /// The logo and its shadow, `mainmenu.lyt`'s.
    fn logo(&mut self) {
        self.tex("mainmenu_logo_shadow", rect(30.0, 35.0, 512.0, 64.0), [0, 0, 0, 40]);
        self.tex("mainmenu_logo", rect(20.0, 25.0, 512.0, 64.0), CHOSEN);
    }

    /// The dark column the rows sit on, `width` times the main menu's, and its shadow ten to the
    /// right.
    fn column(&mut self, x: f32, width: f32) {
        let w = 256.0 * width;
        self.tex("mainmenu_background", rect(x + 10.0, 0.0, w, 448.0), [0, 0, 0, 60]);
        self.tex("mainmenu_background", rect(x, 0.0, w, 448.0), [255, 255, 255, 200]);
    }

    /// Rows centred on `x`, the chosen one in the oblique face on the red bar.
    fn list(&mut self, x: f32, width: f32, rows: &[String], chosen: usize) {
        let bar = 256.0 * (0.25 + 0.75 * width);
        let y = item_y(chosen) - 5.0;
        self.tex("mainmenu_highlight_off_shadow", rect(x - bar / 2.0 + 10.0, y + 10.0, bar, 32.0), [0, 0, 0, 110]);
        self.tex("mainmenu_highlight", rect(x - bar / 2.0, y, bar, 32.0), CHOSEN);
        for (i, row) in rows.iter().enumerate() {
            if i == chosen {
                self.text(Face::Chosen, row, x, item_y(i) - 1.0, Align::Centre, CHOSEN);
            } else {
                self.text(Face::Item, row, x, item_y(i), Align::Centre, ITEM);
            }
        }
    }

    /// The screen's name, bottom left over the ticker, as the navigation layout's Screen Title.
    fn title(&mut self, title: &str) {
        self.text(Face::Title, title, 50.0, 362.0, Align::Left, CHOSEN);
    }

    /// The black bar at the bottom, and the controller legend in its corner, right to left from the
    /// screen's edge: `mxfrontendnavigation.lyt`'s Ticker and Navigation Background panels.
    fn legend(&mut self, buttons: &[(&str, &str)]) {
        self.fill(rect(0.0, 403.0, 640.0, 45.0), BLACK);
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

    /// The track pick: `trackpreview.lyt`'s band of stills between the two masks, the track's name
    /// over it between arrows.
    fn tracks(&mut self, menu: &Menu) {
        let at = menu.cursor(Screen::Tracks);
        self.fill(rect(0.0, 0.0, 640.0, 448.0), BLACK);
        let stills = &self.art.stills[at];
        if !stills.is_empty() {
            let still = &stills[(menu.tick / STILL_FRAMES) as usize % stills.len()];
            // The band is wider than the still is tall: its middle, the full width.
            let (w, h) = (still.width as f32, still.height as f32);
            let rows = w * 309.0 / 640.0;
            let top = (h - rows) / 2.0;
            self.blit(still, [0.0, top, w, top + rows], rect(0.0, 49.0, 640.0, 309.0), CHOSEN);
        }
        // The masks: a dark top band and the bottom one the legend sits on.
        self.fill(rect(0.0, 0.0, 640.0, 49.0), [0, 0, 0, 200]);
        self.fill(rect(0.0, 358.0, 640.0, 90.0), [0, 0, 0, 200]);
        let (_, name) = TRACKS[at];
        self.text(Face::Title, name, 320.0, 8.0, Align::Centre, CHOSEN);
        let w = self.art.fonts[&Face::Title].measure(name);
        self.tex("arrow_left_off", rect(320.0 - w / 2.0 - 44.0, 9.0, 32.0, 32.0), CHOSEN);
        self.tex("arrow_right_off", rect(320.0 + w / 2.0 + 12.0, 9.0, 32.0, 32.0), CHOSEN);
        let count = format!("Freeride {} of {}", at + 1, TRACKS.len());
        self.text(Face::Small, &count, 320.0, 330.0, Align::Centre, ITEM);
        self.title(Screen::Tracks.title());
        self.legend(&[("nav_circle_off", "Back"), ("nav_x_off", "Ride")]);
    }

    /// The splash (P46): a track's still from the disc, dimmed, the logo over it, and what to
    /// press — or, once someone has, who 1P is — and the Quit popup when Back asked for it.
    fn splash(&mut self, menu: &Menu) {
        self.fill(rect(0.0, 0.0, 640.0, 448.0), BLACK);
        // The last track ridden's first still. (The disc's own `titlescreen` has its logo and a
        // block of small print baked in, which the prompt would sit on.)
        let track = menu.settings.track_index();
        let still = self.art.stills.get(track).and_then(|s| s.first());
        if let Some(t) = still {
            // Cover the screen with the picture's middle, keeping its shape.
            let (w, h) = (t.width as f32, t.height as f32);
            let fit = LAYOUT.0 / LAYOUT.1;
            let src = if w / h > fit {
                let cw = h * fit;
                [(w - cw) / 2.0, 0.0, (w + cw) / 2.0, h]
            } else {
                let ch = w / fit;
                [0.0, (h - ch) / 2.0, w, (h + ch) / 2.0]
            };
            self.blit(t, src, rect(0.0, 0.0, 640.0, 448.0), SPLASH_DIM);
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
        }
        if menu.p1.is_none() {
            self.legend(&[("nav_circle_off", "Quit"), ("nav_x_off", "Start")]);
        }
        if let Some(row) = menu.quitting() {
            self.fill(rect(0.0, 0.0, 640.0, 448.0), [0, 0, 0, 150]);
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

    /// The on-screen keyboard: the name being typed, the four rows of ten and the commands.
    fn keyboard(&mut self, menu: &Menu) {
        self.fill(rect(90.0, 60.0, 460.0, 290.0), [0, 0, 0, 200]);
        let typed = format!("{}_", menu.typing());
        self.fill(rect(120.0, 76.0, 400.0, 32.0), [60, 0, 0, 255]);
        self.text(Face::Chosen, &typed, 132.0, 80.0, Align::Left, CHOSEN);
        let (row, col) = menu.key;
        for (r, keys) in KEY_ROWS.iter().enumerate() {
            for (c, k) in keys.chars().enumerate() {
                let (x, y) = (120.0 + 40.0 * c as f32, 124.0 + 40.0 * r as f32);
                let on = (r, c) == (row, col);
                if on {
                    self.tex("mainmenu_highlight", rect(x + 2.0, y - 2.0, 36.0, 32.0), CHOSEN);
                }
                let colour = if on { CHOSEN } else { ITEM };
                let face = if on { Face::Chosen } else { Face::Item };
                self.text(face, &k.to_string(), x + 20.0, y + 4.0, Align::Centre, colour);
            }
        }
        for (c, label) in KEY_COMMANDS.iter().enumerate() {
            let (x, y) = (120.0 + 100.0 * c as f32, 290.0);
            let on = (KEY_ROWS.len(), c) == (row, col);
            if on {
                self.tex("mainmenu_highlight", rect(x + 4.0, y - 4.0, 92.0, 32.0), CHOSEN);
            }
            let face = if on { Face::Chosen } else { Face::Item };
            self.text(face, label, x + 50.0, y, Align::Centre, if on { CHOSEN } else { ITEM });
        }
        self.title(Screen::Keyboard.title());
        self.legend(&[
            ("nav_square_off", "Delete"),
            ("nav_circle_off", "Cancel"),
            ("nav_x_off", "Type"),
        ]);
    }
}

/// The pixels a span of layout covers, clipped to the frame.
fn span(a: f32, b: f32, n: usize) -> (usize, usize) {
    (a.round().clamp(0.0, n as f32) as usize, b.round().clamp(0.0, n as f32) as usize)
}

fn blend(frame: &mut Frame, x: usize, y: usize, c: [u8; 4]) {
    let at = 3 * (y * frame.width + x);
    let a = c[3] as u32;
    for i in 0..3 {
        let d = frame.color[at + i] as u32;
        frame.color[at + i] = ((c[i] as u32 * a + d * (255 - a)) / 255) as u8;
    }
}

/// Light added by its alpha, which is how the radial glow brightens and never darkens.
fn add(frame: &mut Frame, x: usize, y: usize, c: [u8; 4]) {
    let at = 3 * (y * frame.width + x);
    for i in 0..3 {
        let d = frame.color[at + i] as u32 + c[i] as u32 * c[3] as u32 / 255;
        frame.color[at + i] = d.min(255) as u8;
    }
}

/// The texel at `u, v` (texels), blended from its four neighbours and kept inside `src` so a glyph
/// does not bleed in its neighbour in the atlas.
fn bilinear(t: &Texture, u: f32, v: f32, src: [f32; 4]) -> [u8; 4] {
    let lo_x = src[0].min(src[2]).max(0.0);
    let hi_x = (src[0].max(src[2]) - 1.0).min(t.width as f32 - 1.0).max(lo_x);
    let lo_y = src[1].min(src[3]).max(0.0);
    let hi_y = (src[1].max(src[3]) - 1.0).min(t.height as f32 - 1.0).max(lo_y);
    let u = (u - 0.5).clamp(lo_x, hi_x);
    let v = (v - 0.5).clamp(lo_y, hi_y);
    let (x0, y0) = (u.floor() as usize, v.floor() as usize);
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
        out[i] = (top * (1.0 - fy) + bottom * fy).round() as u8;
    }
    out
}
