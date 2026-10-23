//! The option pages (P118): Settings' five tabs, the Garage's three, the game modifiers and the
//! mods, each a list of [`Row`]s that change in place. A row knows its name, its value, every choice
//! it has, what it does and, dimmed, why it does not apply — the side panel draws all of it.

use super::*;

/// Settings' tabs, which L1 and R1 step through, round from either end.
pub const SETTINGS_TABS: [&str; 5] = ["Ride", "Display", "Audio", "Controls", "Camera"];
pub const TAB_RIDE: usize = 0;
pub const TAB_DISPLAY: usize = 1;
pub const TAB_AUDIO: usize = 2;
pub const TAB_CONTROLS: usize = 3;
pub const TAB_CAMERA: usize = 4;
/// The Garage's tabs, each starting with the same Player row.
pub const GARAGE_TABS: [&str; 3] = ["Rider", "Bike", "Gear"];

/// The players a Player row steps through: the concept's 1P to 4P.
pub const PLAYERS: usize = 4;

/// The six sliders in the concept's order, by their index in [`Mix::levels`].
const LEVELS: [(usize, &str, &str); 6] = [
    (0, "Music", "The disc's soundtrack."),
    (2, "Sound FX", "Landings, crashes, the world."),
    (4, "Engine", "Your bike and everyone else's."),
    (3, "Crowd", "The stands at the tracks that have them."),
    (5, "Menu FX", "Clicks and swooshes in these menus."),
    (1, "User Soundtracks", "Songs from the music folder beside the disc, and from mods' music packs."),
];

/// The camera sliders' names and what they do, in [`CameraView::get`]'s order.
const CAMERAS: [(&str, &str); 5] = [
    ("Field of View", "How wide the view is: the original's angle, opened or narrowed by this many degrees."),
    ("Bike Distance", "How far behind the bike the follow camera sits."),
    ("Bike Height", "How high above the bike it sits."),
    ("Biplane Distance", "How far behind the biplane."),
    ("Biplane Height", "How high above the biplane."),
];

/// What Music Plays offers, as `settings.txt`'s words; One Song is a song's id there.
const PLAYS: [&str; 4] = ["Shuffle", "In Order", "One Song", "Off"];
const PLAYS_ONE: usize = 2;

/// What a row is, which is how it changes and how it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// One of a few choices, left and right stepping them and Cross the next.
    Choice,
    /// A number with a meter: left and right step it.
    Slider,
    /// Cross does it.
    Action,
    /// Cross opens another page.
    Opens,
    /// A heading between rows, which the cursor steps over.
    Heading,
    /// Whose the rows under it are.
    Player,
}

/// Which setting a row is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Player,
    StartOn,
    Wrecks,
    Bays,
    Modifiers,
    Hud,
    Speedometer,
    DrawDistance,
    RenderScale,
    VSync,
    FrameLimit,
    Mix,
    /// A slider by its index in [`Mix::levels`].
    Level(usize),
    MusicPlays,
    Song,
    SkipSong,
    BikeLayout,
    ControllerPreset,
    PlaneLayout,
    Throttle,
    Nintendo,
    Controller,
    Pressure,
    /// A camera slider by its index in [`CAMERAS`].
    Camera(usize),
    CameraDefault,
    CameraOriginal,
    Garage(roster::Row),
    Knob(Knob),
    LoadPreset,
    SavePreset,
    DeletePreset,
    ResetMods,
    ModsFolder,
    /// A mod pack by its place in [`Menu::packs`].
    Pack(usize),
    Heading,
}

/// One row of an option page as drawn and changed.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub key: Key,
    pub kind: Kind,
    pub label: String,
    pub value: String,
    /// Every choice, and the one it is on: the side panel lists them.
    pub choices: Vec<String>,
    pub now: Option<usize>,
    /// A line under each choice in the side panel, where the concept has one.
    pub tips: Vec<String>,
    /// What the row does.
    pub about: String,
    /// Why the row does not apply, which dims it and keeps it as it is.
    pub off: Option<String>,
    /// A slider's fill, 0 to 1.
    pub meter: Option<f32>,
    /// A modifier that is the world's, one value for every player.
    pub world: bool,
    /// A last line for the side panel.
    pub note: Option<String>,
}

impl Row {
    fn new(key: Key, kind: Kind, label: &str, about: &str) -> Row {
        Row {
            key,
            kind,
            label: label.into(),
            value: String::new(),
            choices: Vec::new(),
            now: None,
            tips: Vec::new(),
            about: about.into(),
            off: None,
            meter: None,
            world: false,
            note: None,
        }
    }

    fn choice(key: Key, label: &str, choices: &[&str], now: usize, about: &str) -> Row {
        let mut r = Row::new(key, Kind::Choice, label, about);
        r.choices = choices.iter().map(|c| c.to_string()).collect();
        r.now = Some(now.min(choices.len().saturating_sub(1)));
        r.value = r.choices.get(now).cloned().unwrap_or_default();
        r
    }

    fn on_off(key: Key, label: &str, on: bool, about: &str) -> Row {
        Row::choice(key, label, &["On", "Off"], usize::from(!on), about)
    }

    fn slider(key: Key, label: &str, value: String, meter: f32, about: &str) -> Row {
        let mut r = Row::new(key, Kind::Slider, label, about);
        r.value = value;
        r.meter = Some(meter.clamp(0.0, 1.0));
        r
    }

    fn action(key: Key, label: &str, about: &str) -> Row {
        Row::new(key, Kind::Action, label, about)
    }

    fn heading(label: &str) -> Row {
        Row::new(Key::Heading, Kind::Heading, label, "")
    }

    fn off_if(mut self, off: bool, why: &str) -> Row {
        if off {
            self.off = Some(why.into());
        }
        self
    }

    fn tips(mut self, tips: &[&str]) -> Row {
        self.tips = tips.iter().map(|t| t.to_string()).collect();
        self
    }

    /// Whether the cursor can stop on it.
    pub fn selectable(&self) -> bool {
        self.kind != Kind::Heading
    }
}

/// A step `by` through `n` choices from `i`, round from either end.
fn cycle(i: usize, by: i32, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    (i as i64 + by as i64).rem_euclid(n as i64) as usize
}

/// Where `v` is in `all`, or the first.
fn at<T: PartialEq>(all: &[T], v: T) -> usize {
    all.iter().position(|x| *x == v).unwrap_or(0)
}

/// The way a press steps a row: left back, right and Cross on.
fn by(p: Press) -> i32 {
    if p.left { -1 } else { 1 }
}

/// A mod pack (P118): the tracks whose ids share a prefix, `mxvatv` for `mxvatv_nat13`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pack {
    pub prefix: String,
    pub name: String,
    pub tracks: usize,
}

/// A pack's name for its prefix: the importers' own, or the prefix.
pub fn pack_name(prefix: &str) -> String {
    match prefix {
        "mxvatv" => "MX vs. ATV Unleashed".into(),
        "untamed" => "MX vs. ATV Untamed".into(),
        "elev" => "Real Elevation".into(),
        "world" => "Worlds".into(),
        p => {
            let mut c = p.chars();
            c.next().map_or(String::new(), |f| f.to_ascii_uppercase().to_string() + c.as_str())
        }
    }
}

/// The prefix a mod track's id is in the pack of.
pub fn pack_of(id: &str) -> &str {
    id.split('_').next().unwrap_or(id)
}

impl Menu {
    /// The tabs a screen has.
    pub fn tabs(&self, screen: Screen) -> &'static [&'static str] {
        match screen {
            Screen::Settings => &SETTINGS_TABS,
            Screen::Garage => &GARAGE_TABS,
            _ => &[],
        }
    }

    /// The tab a screen is on.
    pub fn tab(&self, screen: Screen) -> usize {
        self.tab[screen.index()]
    }

    /// Whose rows the Player rows show, 0 for 1P: one for every page that has one.
    pub fn player(&self) -> usize {
        self.player
    }

    /// The page shown, or none on a screen that is not one.
    pub fn page(&self) -> Vec<Row> {
        self.screen().map_or_else(Vec::new, |s| self.page_of(s))
    }

    /// A screen's rows, on the tab it is on.
    pub fn page_of(&self, screen: Screen) -> Vec<Row> {
        match screen {
            Screen::Settings => match self.tab(screen) {
                TAB_RIDE => self.ride_rows(),
                TAB_DISPLAY => self.display_rows(),
                TAB_AUDIO => self.audio_page(),
                TAB_CONTROLS => self.control_rows(),
                _ => self.camera_page(),
            },
            Screen::Garage => self.garage_rows(),
            Screen::Modifiers => self.modifier_page(),
            Screen::Mods => self.mods_rows(),
            _ => Vec::new(),
        }
    }

    /// Whether a screen is an option page.
    pub fn is_page(screen: Screen) -> bool {
        matches!(screen, Screen::Settings | Screen::Garage | Screen::Modifiers | Screen::Mods)
    }

    fn player_row(&self, about: &str) -> Row {
        let names: Vec<String> = (1..=PLAYERS).map(|i| format!("{i}P")).collect();
        let mut r = Row::new(Key::Player, Kind::Player, "Player", about);
        r.value = names[self.player.min(PLAYERS - 1)].clone();
        r.now = Some(self.player.min(PLAYERS - 1));
        r.choices = names;
        r
    }

    fn ride_rows(&self) -> Vec<Row> {
        let machines: Vec<&str> = Machine::ALL.iter().map(|m| m.label()).collect();
        let at = Machine::ALL.iter().position(|m| *m == self.settings.machines[0]).unwrap_or(0);
        let changed = Knob::MENU
            .iter()
            .filter(|k| (0..SEATS_MAX).any(|s| self.settings.modifiers.get(s, **k) != game_sim::modifiers::ORIGINAL))
            .count();
        let mut mods = Row::new(Key::Modifiers, Kind::Opens, "Game Modifiers", "Speed, grip, gravity and the rest, per player. Saved presets live here too.");
        mods.value = match changed {
            0 => "Original".into(),
            n => format!("{n} Changed"),
        };
        vec![
            Row::choice(Key::StartOn, "Start On", &machines, at, "The machine a ride starts on. You can switch at any time from the pause, or by stopping at a bay."),
            Row::on_off(Key::Wrecks, "Wrecks", self.settings.wrecks, "Whether a hard landing throws the rider off. Off, you ride out every crash."),
            Row::on_off(Key::Bays, "Machines at Bays", self.settings.bays, "The lot's bikes and the biplane stand at their bays, see-through, and the original's pop-up offers them when you stop beside one."),
            mods,
        ]
    }

    fn display_rows(&self) -> Vec<Row> {
        let s = &self.settings;
        let hud = Overlay::ALL.map(Overlay::label);
        let speed = Speedometer::ALL.map(Speedometer::label);
        let scales = RenderScale::ALL.map(RenderScale::label);
        let limits: Vec<String> = FrameLimit::ALL.iter().map(|f| f.label()).collect();
        let limits: Vec<&str> = limits.iter().map(String::as_str).collect();
        let speed_off = match s.overlay {
            Overlay::Original => None,
            Overlay::Debug => Some("Only the Original HUD has one"),
            Overlay::None => Some("The HUD is off"),
        };
        vec![
            Row::choice(Key::Hud, "HUD", &hud, at(&Overlay::ALL, s.overlay), "What a ride draws over the picture. Original is the game's own; Debug adds the port's numbers; None leaves the picture clean.")
                .tips(&["The game's own", "Speeds, inputs, timings", "Nothing over the ride"]),
            Row::choice(Key::Speedometer, "Speedometer", &speed, at(&Speedometer::ALL, s.speedometer), "How the original's HUD shows your speed: the readout or the dial.")
                .off_if(speed_off.is_some(), speed_off.unwrap_or("")),
            Row::choice(
                Key::DrawDistance,
                "Draw Distance",
                &Graphics::ALL.map(graphics_label),
                at(&Graphics::ALL, s.graphics),
                "Original Fog is the game's haze and far clip. Whole Map draws everything, with only a light haze far off.",
            ),
            Row::choice(Key::RenderScale, "Render Scale", &scales, at(&RenderScale::ALL, s.render_scale), "How much of the window's resolution the 3D is drawn at. Auto drops it when frames run long and climbs back when there's room."),
            Row::on_off(Key::VSync, "VSync", s.vsync, "Waits for the display before showing a frame. No tearing. Off draws as fast as it can."),
            Row::choice(Key::FrameLimit, "Frame Limit", &limits, at(&FrameLimit::ALL, s.frame_limit), "The most frames a second. Only the picture's rate; the ride itself always ticks at the game's 30 Hz."),
        ]
    }

    /// Music Plays' choice for the setting: One Song for a song's id.
    fn plays(&self) -> usize {
        match self.settings.music.as_str() {
            "" | "shuffle" => 0,
            "order" => 1,
            "off" => 3,
            _ => PLAYS_ONE,
        }
    }

    fn audio_page(&self) -> Vec<Row> {
        let s = &self.settings;
        let mixes = Mix::ALL.map(Mix::label);
        let at = Mix::ALL.iter().position(|m| *m == s.mix).unwrap_or(0);
        let mut rows = vec![Row::choice(Key::Mix, "Audio Preset", &mixes, at, "The original's mixes set all six sliders at once. Moving any slider makes it a Custom Mix, which is kept.")];
        let l = s.levels();
        for (i, label, about) in LEVELS {
            let mut r = Row::slider(Key::Level(i), label, l[i].to_string(), f32::from(l[i]) / f32::from(VOLUME_MAX), about);
            if s.mix != Mix::Custom {
                r.note = Some("Moving this makes it a Custom Mix.".into());
            }
            rows.push(r);
        }
        rows.push(Row::heading("Soundtrack"));
        rows.push(Row::choice(Key::MusicPlays, "Music Plays", &PLAYS, self.plays(), "How the next song is picked. One Song repeats the song below."));
        let one = self.plays() == PLAYS_ONE;
        let mut song = Row::new(Key::Song, Kind::Choice, "Song", "The song One Song repeats.");
        song.choices = self.music_choices.iter().map(|(_, l)| l.clone()).collect();
        song.now = if one { self.music_choices.iter().position(|(id, _)| *id == s.music) } else { None };
        song.value = match (one, song.now) {
            (true, Some(i)) => song.choices[i].clone(),
            (true, None) => self.song_label(&s.music),
            _ => self.music_now.as_deref().or(self.song_at()).map_or("None".into(), |id| self.song_label(id)),
        };
        rows.push(song.off_if(!one, "Only with Music Plays: One Song"));
        let mut skip = Row::action(Key::SkipSong, "Skip Song", "On to the next song.").off_if(s.music == "off", "The music is off");
        if let Some(now) = &self.music_now {
            skip.value = self.song_label(now);
        }
        rows.push(skip);
        rows
    }

    fn control_rows(&self) -> Vec<Row> {
        let c = &self.controls;
        let p = self.player.min(SEATS - 1);
        let presets: Vec<String> = (1..=PRESETS).map(|i| format!("Preset {i}")).collect();
        let presets: Vec<&str> = presets.iter().map(String::as_str).collect();
        let first = p != 0;
        let pad = c.pad.clone().or_else(|| self.pad_kept.clone());
        let mut device = match &pad {
            Some(d) => Row::choice(Key::Controller, "Controller", &["First Found", d], usize::from(c.pad.is_some()), "Which device this player rides on."),
            None => Row::choice(Key::Controller, "Controller", &["First Found"], 0, "Which device this player rides on."),
        };
        device = device.off_if(first, "Other players ride the pad they joined on").off_if(!first && pad.is_none(), "Name a device in controls.txt");
        let map = if c.pressure.is_empty() { self.pressure_kept.clone() } else { Some(c.pressure.clone()) };
        let mut pressure = match &map {
            Some(m) => Row::choice(Key::Pressure, "Pressure Axes", &["None", m], usize::from(!c.pressure.is_empty()), "Maps a pad's analog axes onto the PS2's pressure-sensitive buttons."),
            None => Row::choice(Key::Pressure, "Pressure Axes", &["None"], 0, "Maps a pad's analog axes onto the PS2's pressure-sensitive buttons."),
        };
        pressure = pressure.off_if(first, "Other players ride the pad they joined on").off_if(!first && map.is_none(), "Name a map in controls.txt");
        vec![
            self.player_row("Whose controls these rows change. Every player's pad keeps its own."),
            Row::choice(
                Key::BikeLayout,
                "Bike Layout",
                &Layout::ALL.map(Layout::label),
                usize::from(c.layouts[p] == Layout::Original),
                "New is the port's: gas and brake on the triggers' pressure, stunts on the face buttons. Original is the disc's, with its four controller presets.",
            ),
            Row::choice(Key::ControllerPreset, "Controller Preset", &presets, c.presets[p], "The original's Controller Presets screen, one row. Preset 1 is the one the game ships with.")
                .off_if(c.layouts[p] != Layout::Original, "Only with the Original layout"),
            Row::choice(
                Key::PlaneLayout,
                "Biplane Layout",
                &Layout::ALL.map(Layout::label),
                usize::from(c.planes[p] == Layout::Original),
                "New steers the biplane relative to the plane at every bank. Original is the disc's stick, which goes dead on its side.",
            ),
            Row::choice(Key::Throttle, "Throttle", &Gas::ALL.map(Gas::label), Gas::ALL.iter().position(|g| *g == c.throttle).unwrap_or(0), "Which button is the gas."),
            Row::choice(
                Key::Nintendo,
                "Nintendo A/B",
                &["By Position", "By Label"],
                usize::from(!c.nintendo_positions),
                "By Position: the bottom button confirms, as on every other pad. By Label: A confirms, as on a Switch.",
            ),
            device,
            pressure,
        ]
    }

    fn camera_page(&self) -> Vec<Row> {
        let v = self.settings.cameras[self.player.min(SEATS_MAX - 1)];
        let mut rows = vec![self.player_row("Whose camera these sliders move. Split-screen players each have their own.")];
        for (i, (label, about)) in CAMERAS.iter().enumerate() {
            let (lo, hi) = match i {
                0 => (i32::from(CAMERA_FOV.0), i32::from(CAMERA_FOV.1)),
                _ => (i32::from(CAMERA_SHARE.0), i32::from(CAMERA_SHARE.1)),
            };
            let meter = (v.get(i) - lo) as f32 / (hi - lo) as f32;
            let mut r = Row::slider(Key::Camera(i), label, v.label(i), meter, about);
            r.note = Some("Cross puts it back to the original's.".into());
            rows.push(r);
        }
        rows.push(Row::action(Key::CameraDefault, "Reset to Default", "Back to the port's defaults for this player."));
        rows.push(Row::action(Key::CameraOriginal, "Reset to Original", "Back to the disc's camera for this player."));
        rows
    }

    /// The garage seat the Player row is on.
    fn garage_seat(&self) -> usize {
        self.player.min(self.garages.len().saturating_sub(1))
    }

    fn garage_rows(&self) -> Vec<Row> {
        use roster::{Piece, Row as G};
        let seat = self.garage_seat();
        let value = |row: G| self.garages.get(seat).map_or(String::new(), |g| g.value(&self.catalogue, row));
        let row = |row: G, label: &str, about: &str| {
            let mut r = Row::new(Key::Garage(row), Kind::Choice, label, about);
            r.value = value(row);
            r
        };
        let mut rows = vec![self.player_row("Whose rider and bike this is. Each player in a local game has their own.")];
        match self.tab(Screen::Garage) {
            0 => {
                let mut name = Row::new(Key::Garage(G::Name), Kind::Opens, "Name", "Painted on the back of the jersey and shown to other players online. Opens the on-screen keyboard.");
                name.value = self.rider_name(seat);
                rows.push(name);
                let mut number = row(G::Number, "Number", "The number on the plate and the jersey.");
                number.note = Some("Cross types one.".into());
                rows.push(number);
                rows.push(row(G::Rider, "Rider", "Who rides."));
                rows.push(row(G::Colours, "Colours", "The rider's two colours, used on the gear below."));
                rows.push(row(G::Outfit, "Jersey & Pants", "One pick for the two, as they come as a set."));
                let mut save = Row::new(Key::Garage(G::Preset), Kind::Action, "Save as Preset", "Keeps this rider, bike and gear under a name, for the next session or another seat.");
                save.value = value(G::Preset);
                save.note = Some("Left and right load a saved one.".into());
                rows.push(save);
            }
            1 => {
                rows.push(row(G::Class, "Class", "The bike's class."));
                rows.push(row(G::Model, "Bike", "The bike in this class."));
                rows.push(row(G::Livery, "Paint", "The bike's livery."));
                rows.push(row(G::Tier, "Tier", "Higher tiers are faster bikes."));
                rows.push(row(G::Tuning, "Tuning", "Suspension and gearing set-up."));
            }
            _ => {
                rows.push(row(G::Piece(Piece::Helmet), "Helmet", "The helmet, in the rider's colours."));
                rows.push(row(G::Piece(Piece::Goggles), "Goggles", "The goggles."));
                rows.push(row(G::Piece(Piece::Gloves), "Gloves", "The gloves."));
                rows.push(row(G::Piece(Piece::Boots), "Boots", "The boots."));
            }
        }
        rows
    }

    /// A seat's rider name as its Name row shows it: 1P's is the settings' name until the look has
    /// its own.
    pub(super) fn rider_name(&self, seat: usize) -> String {
        let look = self.garages.get(seat).map(|g| g.pick.look.name.clone()).unwrap_or_default();
        match (look.is_empty(), seat) {
            (false, _) => look,
            (true, 0) => self.settings.name.clone(),
            _ => "None".into(),
        }
    }

    fn modifier_page(&self) -> Vec<Row> {
        let held = self.mods_held().map(str::to_string);
        let seat = self.player.min(SEATS_MAX - 1);
        let mut player = self.player_row("Whose modifiers these are. Gravity, air drag and time scale are the world's, so they're everyone's.");
        // When a change takes (P98): the ride under the pause's as it closes, else the next ride's.
        player.note = Some(match (self.stack.first(), &held) {
            (_, Some(why)) => why.clone(),
            (Some(Screen::Pause), None) => "Changes take when the pause closes.".into(),
            _ => "Changes are the next ride's.".into(),
        });
        let mut rows = vec![player];
        let most = game_sim::modifiers::MOST;
        for knob in Knob::MENU {
            let v = self.settings.modifiers.get(if knob.per_seat() { seat } else { 0 }, knob);
            let meter = f32::from(v.saturating_sub(knob.least())) / f32::from(most - knob.least());
            let mut r = Row::slider(Key::Knob(knob), knob.label(), format!("{v}%"), meter, knob.blurb());
            r.world = !knob.per_seat();
            r.note = Some(if r.world { "The world's: one value for every player." } else { "This player's. 100% is the original game." }.into());
            rows.push(r.off_if(held.is_some(), held.as_deref().unwrap_or("")));
        }
        rows.push(Row::heading("Presets"));
        let names: Vec<&str> = self.settings.presets.iter().map(|(n, _)| n.as_str()).collect();
        let none = names.is_empty();
        let load = match none {
            true => Row::choice(Key::LoadPreset, "Load Preset", &["None Saved"], 0, "A saved set of modifiers."),
            false => Row::choice(Key::LoadPreset, "Load Preset", &names, self.mod_preset, "A saved set of modifiers."),
        };
        rows.push(load.off_if(none, "No presets saved yet").off_if(held.is_some(), held.as_deref().unwrap_or("")));
        rows.push(Row::action(Key::SavePreset, "Save Preset", "Name and keep this set."));
        rows.push(Row::action(Key::DeletePreset, "Delete Preset", "Forgets the preset Load Preset is on.").off_if(none, "No presets saved yet"));
        rows.push(Row::action(Key::ResetMods, "Reset to Original", "Every modifier back to the original game.").off_if(held.is_some(), held.as_deref().unwrap_or("")));
        rows
    }

    /// The mod packs the Mods page lists: every pack the folder has had while it was read this
    /// session, in the order their tracks came.
    pub fn packs(&self) -> Vec<Pack> {
        let mut out: Vec<Pack> = Vec::new();
        for (id, _) in &self.mods_seen {
            let prefix = pack_of(id);
            match out.iter_mut().find(|p| p.prefix == prefix) {
                Some(p) => p.tracks += 1,
                None => out.push(Pack { prefix: prefix.into(), name: pack_name(prefix), tracks: 1 }),
            }
        }
        out
    }

    fn mods_rows(&self) -> Vec<Row> {
        let mut rows = vec![Row::on_off(Key::ModsFolder, "Read the Mods Folder", self.settings.mods, "Off, nothing is read from mods/ beside the disc and every pick is the disc's alone.")];
        rows.push(Row::heading("Installed"));
        let packs = self.packs();
        if packs.is_empty() {
            rows.push(Row::heading("Nothing in the mods folder"));
        }
        for (i, p) in packs.iter().enumerate() {
            let on = !self.settings.mods_off.contains(&p.prefix);
            let tracks = if p.tracks == 1 { "1 track".to_string() } else { format!("{} tracks", p.tracks) };
            let mut r = Row::on_off(Key::Pack(i), &p.name, on, &format!("{tracks}, listed under Mods on Free Ride."));
            r.note = Some(format!("{} in the folder", p.prefix));
            rows.push(r.off_if(!self.settings.mods, "The mods folder is off"));
        }
        rows
    }

    /// The first row the cursor can stop on.
    fn first_row(rows: &[Row]) -> usize {
        rows.iter().position(Row::selectable).unwrap_or(0)
    }

    /// Open Settings on a tab, its cursor on the first row.
    pub(super) fn open_settings(&mut self, tab: usize) {
        self.tab[Screen::Settings.index()] = tab.min(SETTINGS_TABS.len() - 1);
        if tab == TAB_AUDIO {
            // The song row starts on the song playing.
            if let Some(i) = self.music_now.as_ref().and_then(|now| self.music_choices.iter().position(|(id, _)| id == now)) {
                self.music_at = i;
            }
        }
        self.push(Screen::Settings);
        self.cursor[Screen::Settings.index()] = Menu::first_row(&self.page_of(Screen::Settings));
    }

    /// A press on an option page: Circle leaves, L1 and R1 step the tabs round, up and down walk the
    /// rows over the headings, and left, right and Cross change the row in place.
    pub(super) fn walk_page(&mut self, screen: Screen, p: Press) {
        if p.back || (screen == Screen::Garage && p.start) {
            match screen {
                Screen::Garage => self.close_garage(),
                _ => self.pop(),
            }
            return;
        }
        let i = screen.index();
        let tabs = self.tabs(screen).len();
        if tabs > 0 && (p.l1 || p.r1) {
            self.tab[i] = cycle(self.tab[i], if p.l1 { -1 } else { 1 }, tabs);
            self.cursor[i] = Menu::first_row(&self.page_of(screen));
            return;
        }
        let rows = self.page_of(screen);
        if rows.is_empty() {
            return;
        }
        let n = rows.len();
        let mut at = self.cursor[i].min(n - 1);
        if !rows[at].selectable() {
            at = Menu::first_row(&rows);
        }
        self.cursor[i] = at;
        if p.up || p.down {
            let step = if p.up { -1 } else { 1 };
            let mut to = at;
            for _ in 0..n {
                to = cycle(to, step, n);
                if rows[to].selectable() {
                    break;
                }
            }
            self.cursor[i] = to;
            return;
        }
        if !(p.ok || p.left || p.right) || rows[at].off.is_some() {
            return;
        }
        self.change(&rows[at], p);
    }

    /// Change a row by a press.
    fn change(&mut self, row: &Row, p: Press) {
        let side = p.left || p.right;
        let d = by(p);
        let next = |row: &Row| cycle(row.now.unwrap_or(0), d, row.choices.len());
        let s = &mut self.settings;
        match row.key {
            Key::Heading => return,
            Key::Player => {
                let n = if self.screen() == Some(Screen::Garage) { self.garages.len().clamp(1, PLAYERS) } else { PLAYERS };
                self.player = cycle(self.player, d, n);
                return;
            }
            Key::StartOn => s.machines[0] = Machine::ALL[next(row)],
            Key::Wrecks => s.wrecks = !s.wrecks,
            Key::Bays => s.bays = !s.bays,
            Key::Modifiers if p.ok => {
                self.push(Screen::Modifiers);
                self.cursor[Screen::Modifiers.index()] = 0;
                return;
            }
            Key::Hud => s.overlay = Overlay::ALL[next(row)],
            Key::Speedometer => s.speedometer = Speedometer::ALL[next(row)],
            Key::DrawDistance => s.graphics = Graphics::ALL[next(row)],
            Key::RenderScale => s.render_scale = RenderScale::ALL[next(row)],
            Key::VSync => s.vsync = !s.vsync,
            Key::FrameLimit => s.frame_limit = s.frame_limit.step(p.left),
            Key::Mix => s.set_mix(Mix::ALL[next(row)]),
            // Cross steps a slider up and round from the top, as the original's page does.
            Key::Level(i) => s.nudge(i, p.left, p.right),
            Key::MusicPlays => self.music_plays(d),
            Key::Song if side => self.step_song(d),
            Key::SkipSong if p.ok => self.skip_song(),
            Key::BikeLayout => {
                let i = self.player.min(SEATS - 1);
                self.controls.layouts[i] = Layout::ALL[next(row)];
            }
            Key::ControllerPreset => {
                let i = self.player.min(SEATS - 1);
                self.controls.presets[i] = next(row);
            }
            Key::PlaneLayout => {
                let i = self.player.min(SEATS - 1);
                self.controls.planes[i] = Layout::ALL[next(row)];
            }
            Key::Throttle => self.controls.throttle = Gas::ALL[next(row)],
            Key::Nintendo => self.controls.nintendo_positions = !self.controls.nintendo_positions,
            Key::Controller => {
                // First Found keeps the named device to come back to.
                match self.controls.pad.take() {
                    Some(d) => self.pad_kept = Some(d),
                    None => self.controls.pad = self.pad_kept.clone(),
                }
            }
            Key::Pressure => {
                if self.controls.pressure.is_empty() {
                    self.controls.pressure = self.pressure_kept.clone().unwrap_or_default();
                } else {
                    self.pressure_kept = Some(std::mem::take(&mut self.controls.pressure));
                }
            }
            Key::Camera(i) => {
                let seat = self.player.min(SEATS_MAX - 1);
                // Cross puts the slider back to the original's.
                self.settings.cameras[seat].step(i, p.left, p.right);
            }
            Key::CameraDefault | Key::CameraOriginal if p.ok => {
                let to = if row.key == Key::CameraDefault { CameraView::DEFAULT } else { CameraView::ORIGINAL };
                self.settings.cameras[self.player.min(SEATS_MAX - 1)] = to;
            }
            Key::Garage(g) => {
                self.change_garage(g, p);
                return;
            }
            Key::Knob(knob) => {
                let now = row.value.trim_end_matches('%').parse::<u16>().unwrap_or(game_sim::modifiers::ORIGINAL);
                let step = game_sim::modifiers::STEP;
                let to = match (p.left, p.right) {
                    (true, _) => now.saturating_sub(step).max(knob.least()),
                    (_, true) => (now + step).min(game_sim::modifiers::MOST),
                    // Cross puts the one modifier back.
                    _ => game_sim::modifiers::ORIGINAL,
                };
                let seat = if knob.per_seat() { self.player.min(SEATS_MAX - 1) } else { 0 };
                self.settings.modifiers.set(seat, knob, to);
            }
            Key::LoadPreset => {
                let n = self.settings.presets.len();
                if side {
                    self.mod_preset = cycle(self.mod_preset, d, n);
                    return;
                }
                self.settings.modifiers = self.settings.presets[self.mod_preset.min(n - 1)].1;
            }
            Key::SavePreset if p.ok => {
                self.type_for(Typing::Preset);
                return;
            }
            Key::DeletePreset if p.ok => {
                let n = self.settings.presets.len();
                self.settings.presets.remove(self.mod_preset.min(n - 1));
                self.mod_preset = self.mod_preset.min(self.settings.presets.len().saturating_sub(1));
            }
            Key::ResetMods if p.ok => self.settings.modifiers = Mods::original(),
            Key::ModsFolder => s.mods = !s.mods,
            Key::Pack(i) => {
                let Some(pack) = self.packs().get(i).map(|p| p.prefix.clone()) else { return };
                match self.settings.mods_off.iter().position(|p| *p == pack) {
                    Some(at) => {
                        self.settings.mods_off.remove(at);
                    }
                    None => self.settings.mods_off.push(pack),
                }
                self.cursor[Screen::Tracks.index()] = self.track_index();
            }
            _ => return,
        }
        self.changed = true;
    }

    /// Music Plays stepped: One Song repeats the song playing, or the song row's, and is passed over
    /// with no songs at all.
    fn music_plays(&mut self, d: i32) {
        let one = self.music_now.clone().or_else(|| self.song_at().map(str::to_string));
        let mut at = self.plays();
        loop {
            at = cycle(at, d, PLAYS.len());
            if at != PLAYS_ONE || one.is_some() {
                break;
            }
        }
        self.settings.music = match at {
            0 => "shuffle".into(),
            1 => "order".into(),
            3 => "off".into(),
            _ => {
                let id = one.unwrap_or_default();
                if let Some(i) = self.music_choices.iter().position(|(c, _)| *c == id) {
                    self.music_at = i;
                }
                self.tunes.push(Tune::Play(id.clone()));
                id
            }
        };
    }

    /// The Song row stepped: the next song is the one repeated, and plays.
    fn step_song(&mut self, d: i32) {
        let n = self.music_choices.len();
        if n == 0 {
            return;
        }
        let i = self.music_choices.iter().position(|(id, _)| *id == self.settings.music).unwrap_or(self.music_at);
        self.music_at = cycle(i, d, n);
        let id = self.music_choices[self.music_at].0.clone();
        self.settings.music = id.clone();
        self.tunes.push(Tune::Play(id));
    }

    /// Skip Song: on to the next song, which under One Song is the next one repeated.
    fn skip_song(&mut self) {
        if self.plays() == PLAYS_ONE {
            self.step_song(1);
        } else {
            self.tunes.push(Tune::Skip);
        }
    }

    /// A Garage row changed: left and right turn it, Cross types the name or number, or saves the
    /// look.
    fn change_garage(&mut self, row: roster::Row, p: Press) {
        let seat = self.garage_seat();
        let Some(at) = roster::ROWS.iter().position(|r| *r == row) else { return };
        let side = p.left || p.right;
        match row {
            roster::Row::Name if p.ok => self.type_for(Typing::Name),
            roster::Row::Name => {}
            roster::Row::Number if p.ok => self.type_for(Typing::Number),
            roster::Row::Number => {
                let Some(g) = self.garages.get_mut(seat) else { return };
                let n: i32 = g.pick.look.number.parse().unwrap_or(0);
                g.pick.look.number = (cycle(n.max(1) as usize - 1, by(p), 999) + 1).to_string();
            }
            roster::Row::Preset => {
                let Some(g) = self.garages.get_mut(seat) else { return };
                g.row = at;
                if side {
                    g.turn_preset(&self.catalogue, &self.looks, by(p));
                } else if g.cross(&mut self.looks) == roster::Cross::Saved {
                    self.looks_changed = true;
                }
            }
            _ => {
                let Some(g) = self.garages.get_mut(seat) else { return };
                g.row = at;
                g.turn(&self.catalogue, by(p));
            }
        }
    }
}

/// Draw Distance's names for the world's two.
pub fn graphics_label(g: Graphics) -> &'static str {
    match g {
        Graphics::Full => "Whole Map",
        Graphics::Original => "Original Fog",
    }
}
