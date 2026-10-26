//! The option pages (P118): Settings' six tabs, the Garage's three, the game modifiers and the
//! mods, each a list of [`Row`]s that change in place. A row knows its name, its value, every choice
//! it has, what it does and, dimmed, why it does not apply — the side panel draws all of it.

use super::*;

/// Settings' tabs, which L1 and R1 step through, round from either end.
/// Display is the screen's HUD; Graphics (P128) the world's draw, the picture and its rate, under
/// headings, with the Original tab's graphics sub-switches (P126).
pub const SETTINGS_TABS: [&str; 7] = ["Ride", "Display", "Graphics", "Audio", "Controls", "Camera", "Original"];
pub const TAB_RIDE: usize = 0;
/// The Tricks row's help (MD16): what MX vs. ATV's tricks add, and what is the port's own.
const TRICKS_HELP: &str = "Original is MX Unleashed's tricks. MX vs. ATV adds that game's from a trick pack in the mods folder: \
     extra stunts on Cross and Square (the bike layout's free buttons), and with R1 held a sideways flick off a lip \
     for a 360 and a forward one for a front flip. The front flip is the port's own, in neither game, and shows the \
     backflip's pose. Online rides are always Original.";
/// The Air Wrecks row's help (P138).
const AIR_WRECKS_HELP: &str = "Whether the bike's angle in the air throws the rider off: upside down, nosed over or rolled too far. \
     Off, you ride every jump out, and only the landing is judged. On is the original game.";
pub const TAB_DISPLAY: usize = 1;
pub const TAB_GRAPHICS: usize = 2;
pub const TAB_AUDIO: usize = 3;
pub const TAB_CONTROLS: usize = 4;
pub const TAB_CAMERA: usize = 5;
pub const TAB_ORIGINAL: usize = 6;
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
    AirWrecks,
    Bays,
    Modifiers,
    Hud,
    Speedometer,
    Minimap,
    HudScale,
    DrawDistance,
    WorldSky,
    RenderScale,
    VSync,
    FrameLimit,
    Mix,
    /// A slider by its index in [`Mix::levels`].
    Level(usize),
    MusicPlays,
    Song,
    SkipSong,
    /// A seat's one Controls choice (P81): New, or Original on one of its controller presets.
    BikeLayout,
    PlaneLayout,
    /// A seat's session markers (P129), on or off.
    Markers,
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
    /// A music pack by its place in [`Menu::music_packs`] (P122).
    MusicPack(usize),
    Tricks,
    /// The Original switch (P126), and each departure's under it.
    Original,
    Departure(Departure),
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

impl Screen {
    /// `--screen`'s word for a tab (P123): Settings' and the Garage's tabs by their names in lower
    /// case, `display` or `gear`, so every tab gets a `--shot`. The tabs that were screens of their
    /// own (graphics, audio, controls, camera) keep [`Screen::from_word`]'s words, which win.
    pub fn tab_from_word(w: &str) -> Option<(Screen, usize)> {
        [(Screen::Settings, &SETTINGS_TABS[..]), (Screen::Garage, &GARAGE_TABS[..])]
            .into_iter()
            .find_map(|(s, tabs)| tabs.iter().position(|t| t.to_lowercase() == w).map(|i| (s, i)))
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
                TAB_GRAPHICS => self.graphics_rows(),
                TAB_AUDIO => self.audio_page(),
                TAB_CONTROLS => self.control_rows(),
                TAB_ORIGINAL => self.original_rows(),
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
        // Air Wrecks (P138): beside Wrecks, and nothing to switch with Wrecks off. Under the pause a
        // change takes as it closes, unless the ride holds it.
        let mut air = Row::on_off(Key::AirWrecks, "Air Wrecks", self.settings.air_wrecks, AIR_WRECKS_HELP);
        if self.stack.first() == Some(&Screen::Pause) {
            air.note = Some("Changes take when the pause closes.".into());
        }
        let air = air.off_if(!self.settings.wrecks, "Wrecks are off").off_if(self.air_held().is_some(), self.air_held().unwrap_or(""));
        vec![
            Row::choice(Key::StartOn, "Start On", &machines, at, "The machine a ride starts on. You can switch at any time from the pause, or by stopping at a bay."),
            Row::on_off(Key::Wrecks, "Wrecks", self.settings.wrecks, "Whether a hard landing throws the rider off. Off, you ride out every crash."),
            air,
            Row::on_off(Key::Bays, "Machines at Bays", self.settings.bays, "The lot's bikes and the biplane stand at their bays, see-through, and the original's pop-up offers them when you stop beside one."),
            mods,
            Row::choice(Key::Tricks, "Tricks", &Tricks::ALL.map(Tricks::label), Tricks::ALL.iter().position(|t| *t == self.settings.tricks).unwrap_or(0), TRICKS_HELP)
                .off_if(!self.settings.mods, "Needs a trick pack in the mods folder"),
        ]
    }

    fn display_rows(&self) -> Vec<Row> {
        let s = &self.settings;
        let hud = Overlay::CHOICES.map(Overlay::label);
        let speed = Speedometer::ALL.map(Speedometer::label);
        let speed_off = match s.overlay {
            Overlay::Hud | Overlay::Original => None,
            Overlay::Debug => Some("Only the HUD and the Original have one"),
            Overlay::None => Some("The HUD is off"),
        };
        vec![
            Row::choice(Key::Hud, "HUD", &hud, at(&Overlay::CHOICES, s.overlay), "What a ride draws over the picture. HUD is openmx's own, sharp at any size; Original is the game's; None leaves the picture clean.")
                .tips(&["Sharp at any size", "The game's own", "Nothing over the ride"]),
            Row::choice(Key::Speedometer, "Speedometer", &speed, at(&Speedometer::ALL, s.speedometer), "How the HUD shows your speed: the readout or the dial.")
                .off_if(speed_off.is_some(), speed_off.unwrap_or("")),
            // V52: the radar alone, and the whole HUD's size about its corners.
            Row::on_off(Key::Minimap, "Minimap", s.minimap, "The original HUD's radar in the corner. Off, it goes from every view and the speedometer stays.")
                .off_if(speed_off.is_some(), speed_off.unwrap_or("")),
            Row::slider(
                Key::HudScale,
                "HUD Scale",
                format!("{}%", s.hud_scale),
                f32::from(s.hud_scale - HUD_SCALE_MIN) / f32::from(HUD_SCALE_MAX - HUD_SCALE_MIN),
                "The size of the whole HUD, each piece in its own corner. 100% is the original's.",
            )
            .off_if(s.overlay == Overlay::None, "The HUD is off"),
        ]
    }

    /// The Graphics tab (P128): each graphics setting once, under the group it is in. The Original
    /// tab's graphics sub-switches (P126) are here and not there: Draw Distance is its fog, and
    /// Plant Detail and Picture Shape its own rows.
    fn graphics_rows(&self) -> Vec<Row> {
        let s = &self.settings;
        let scales = RenderScale::ALL.map(RenderScale::label);
        let limits: Vec<String> = FrameLimit::ALL.iter().map(|f| f.label()).collect();
        let limits: Vec<&str> = limits.iter().map(String::as_str).collect();
        vec![
            Row::heading("World"),
            Row::choice(
                Key::DrawDistance,
                "Draw Distance",
                &Graphics::ALL.map(graphics_label),
                at(&Graphics::ALL, s.graphics),
                "Original Fog is the game's haze and far clip. Far Fog is the same haze four times as far out. Whole Map draws everything, with only a light haze far off.",
            ),
            self.departure_row(Departure::Plants),
            self.departure_row(Departure::Foliage),
            // MD9: a combined world's sky and light, one for all of it or each map its own.
            Row::choice(
                Key::WorldSky,
                "World Sky",
                &WorldSky::ALL.map(WorldSky::label),
                at(&WorldSky::ALL, s.world_sky),
                "For combined worlds of several maps. Global keeps one sky and light everywhere; Per Map gives each map its own, blended where they meet.",
            )
            .tips(&["One sky for the whole world", "Each map its own"]),
            Row::heading("Picture"),
            self.departure_row(Departure::Picture),
            Row::heading("Performance"),
            Row::choice(Key::RenderScale, "Render Scale", &scales, at(&RenderScale::ALL, s.render_scale), "How much of the window's resolution the 3D is drawn at. Auto drops it when frames run long and climbs back when there's room."),
            Row::on_off(Key::VSync, "VSync", s.vsync, "Waits for the display before showing a frame. No tearing. Off draws as fast as it can."),
            Row::choice(Key::FrameLimit, "Frame Limit", &limits, at(&FrameLimit::ALL, s.frame_limit), "The most frames a second. Only the picture's rate; the ride itself always ticks at the game's 30 Hz."),
        ]
    }

    /// The Original tab (P126): the master switch, and every departure's own under it.
    fn original_rows(&self) -> Vec<Row> {
        let mut master = Row::choice(
            Key::Original,
            "Original Game",
            &["Off", "On"],
            0,
            "On puts everything back to the original game: its controls, cameras, fog, plants, leaves, picture shape and HUD. Each switch below can be set on its own, and the fog, plants, leaves and picture shape on the Graphics tab. The ride itself is the original's either way; Game Modifiers change it.",
        )
        .tips(&["Every enhancement on", "The game as it shipped"]);
        match self.original() {
            Original::Off => {}
            Original::On => (master.now, master.value) = (Some(1), "On".into()),
            Original::Mixed => {
                (master.now, master.value) = (None, Original::Mixed.label().into());
                master.note = Some("Some switches below are the original's and some are ours.".into());
            }
        }
        let mut rows = vec![master, Row::heading("Enhancements")];
        for d in Departure::ALL.into_iter().filter(|d| d.tab() == TAB_ORIGINAL) {
            rows.push(self.departure_row(d));
        }
        rows
    }

    /// A departure's own switch, Mixed where only some players are on the original's.
    fn departure_row(&self, d: Departure) -> Row {
        let mut r = Row::choice(Key::Departure(d), d.label(), &d.choices(), usize::from(self.is_original(d)), d.about());
        if !self.is_original(d) && self.some_original(d) {
            (r.now, r.value) = (None, Original::Mixed.label().into());
            r.note = Some("Some players are on the original's. A press sets every player.".into());
        }
        r
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

    /// Whether more than one seat is set up (P81): the Controls tab has its Player row only then, and
    /// is 1P's alone otherwise.
    pub fn many_seats(&self) -> bool {
        self.seated.max(self.riders) > 1
    }

    /// The seat the Controls tab's rows change: the Player row's, or 1P's when it has none.
    pub fn control_seat(&self) -> usize {
        if self.many_seats() { self.player.min(SEATS - 1) } else { 0 }
    }

    fn control_rows(&self) -> Vec<Row> {
        let c = &self.controls;
        let p = self.control_seat();
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
        // One choice for the bike (P81): New, or Original with the original's controller presets
        // inside it. The throttle is no row: each layout's gas is its own.
        let choices = control_choices();
        let names: Vec<String> = choices.iter().map(|(l, n)| controls_label(*l, *n)).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let preset = if c.layouts[p] == Layout::Original { c.presets[p] } else { 0 };
        let now = choices.iter().position(|ch| *ch == (c.layouts[p], preset)).unwrap_or(0);
        let tips = ["R2 gas, L2 brake", "Cross gas, Square brake", "The disc's second preset", "Its third", "Its fourth"];
        let bike = Row::choice(
            Key::BikeLayout,
            "Button Layout",
            &names,
            now,
            "New: R2 the gas and L2 the brake, by how far they are pulled. Original: the disc's own buttons, on any of its four controller presets.",
        )
        .tips(&tips);
        // The plane's stick (P50, P69): the hint under the cursor says what the choice it is on does.
        let planes = Layout::ALL.map(Layout::plane_label);
        let plane_now = usize::from(c.planes[p] == Layout::Original);
        let hints = [
            "Normal: down pulls the nose up, always relative to the plane, at every bank and upside down.",
            "Like the original: the disc's own stick, which stops pitching the plane while it is on its side.",
        ];
        let plane = Row::choice(Key::PlaneLayout, "Plane Stick", &planes, plane_now, hints[plane_now])
            .tips(&["Relative to the plane", "Dead on its side"]);
        // The d-pad's job (P129): the session markers', or the original's ride and trick gestures.
        let markers = Row::choice(
            Key::Markers,
            "Markers",
            &["On", "Off"],
            usize::from(!self.settings.markers[p.min(SEATS_MAX - 1)]),
            "On: the d-pad saves and rides back to markers, and the left stick alone steers and does tricks. Off: the d-pad steers, leans and does tricks as in the original.",
        );
        let mut rows = Vec::new();
        if self.many_seats() {
            rows.push(self.player_row("Whose controls these rows change. Every player's pad keeps its own."));
        }
        rows.extend([
            bike,
            plane,
            markers,
            Row::choice(
                Key::Nintendo,
                "Nintendo A/B",
                &["By Position", "By Label"],
                usize::from(!c.nintendo_positions),
                "By Position: the bottom button confirms, as on every other pad. By Label: A confirms, as on a Switch.",
            ),
            device,
            pressure,
        ]);
        rows
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

    /// The one seat whose modifiers a player who is not the host sees and sets (P134): an online
    /// guest's own (their machine's 1P, in the lobby and in the ride), or under the pause the seat
    /// whose pad opened it when that is not 1P. `None`: the host's page, every seat's.
    pub fn mods_own(&self) -> Option<usize> {
        let lobby_guest = self.lobby.room.is_some() && !self.lobby.hosting();
        // the ride's guest flag holds under its pause; the main menu after it is the next ride's
        let ride_guest = self.guest && self.stack.first() == Some(&Screen::Pause);
        match (ride_guest || lobby_guest, self.stack.first()) {
            (true, _) => Some(0),
            (false, Some(Screen::Pause)) if self.pauser != 0 => Some(self.pauser.min(SEATS_MAX - 1)),
            _ => None,
        }
    }

    /// Whose modifiers the Game Modifiers page sets now: one seat's, or `None` for every seat's
    /// (the host on All Players).
    pub fn mods_seat(&self) -> Option<usize> {
        match (self.mods_own(), self.mods_all) {
            (Some(s), _) => Some(s),
            (None, true) => None,
            (None, false) => Some(self.player.min(SEATS_MAX - 1)),
        }
    }

    /// Whose the page is, for its tag: `All` or `2P`.
    pub fn mods_whose(&self) -> String {
        self.mods_seat().map_or_else(|| "All".into(), |s| format!("{}P", s + 1))
    }

    /// The knobs that are not this page's to set: the world's for anyone but the host, and online
    /// the biplane's for a guest, every seat flying the host's.
    fn knob_held(&self, knob: Knob) -> Option<&'static str> {
        match self.mods_own() {
            None => None,
            Some(_) if !knob.per_seat() => Some("The world's: only the host sets it."),
            Some(_) if self.guest || self.lobby.room.is_some() => {
                (!game_sim::modifiers::online::seat_own(knob)).then_some("Online every seat flies the host's biplane: only the host sets it.")
            }
            Some(_) => None,
        }
    }

    /// A knob's value on the page: the seat's, or on All Players every seat's, `None` where they
    /// differ (Mixed).
    fn knob_value(&self, knob: Knob) -> Option<u16> {
        let m = &self.settings.modifiers;
        match self.mods_seat() {
            Some(s) => Some(m.get(s, knob)),
            None => {
                let v = m.get(0, knob);
                (0..SEATS_MAX).all(|s| m.get(s, knob) == v).then_some(v)
            }
        }
    }

    fn modifier_page(&self) -> Vec<Row> {
        let held = self.mods_held().map(str::to_string);
        let own = self.mods_own();
        let mut player = match own {
            // Not the host: their own seat, and nobody else's.
            Some(s) => {
                let mut r = self.player_row("Your own modifiers. The world's, and everyone else's, are the host's to set.");
                r.choices = vec![format!("{}P", s + 1)];
                (r.now, r.value) = (Some(0), r.choices[0].clone());
                r.off_if(true, "Only the host sets another player's.")
            }
            None => {
                let mut r = self.player_row("Whose modifiers these are. All Players sets every player's at once. Gravity, air drag and time scale are the world's, so they're everyone's.");
                r.choices.insert(0, "All Players".into());
                let at = if self.mods_all { 0 } else { self.player.min(PLAYERS - 1) + 1 };
                (r.now, r.value) = (Some(at), r.choices[at].clone());
                r
            }
        };
        // When a change takes (P98): the ride under the pause's as it closes, else the next ride's.
        player.note = Some(match (self.stack.first(), &held) {
            (_, Some(why)) => why.clone(),
            (Some(Screen::Pause), None) => "Changes take when the pause closes.".into(),
            _ => "Changes are the next ride's.".into(),
        });
        let mut rows = vec![player];
        for knob in Knob::MENU {
            let v = self.knob_value(knob);
            let shown = v.unwrap_or_else(|| self.settings.modifiers.get(0, knob));
            let meter = match knob {
                // Off sits past the highest limit (P137): a full meter.
                Knob::PlaneMaxG if shown == game_sim::modifiers::ORIGINAL => 1.0,
                _ => f32::from(shown.saturating_sub(knob.least())) / f32::from(knob.most() - knob.least()),
            };
            let value = v.map_or_else(|| "Mixed".into(), |v| knob.shown(v));
            let mut r = Row::slider(Key::Knob(knob), knob.label(), value, meter, knob.blurb());
            r.world = !knob.per_seat();
            r.note = Some(
                match (r.world, v, knob) {
                    (true, _, _) => "The world's: one value for every player.",
                    (false, None, _) => "Players differ. A step sets every player's from 1P's; Cross puts them all back.",
                    (false, Some(_), Knob::PlaneMaxG) if self.mods_seat().is_none() => "Every player's. Off is the original game.",
                    (false, Some(_), Knob::PlaneMaxG) => "This player's. Off is the original game.",
                    (false, Some(_), _) if self.mods_seat().is_none() => "Every player's. 100% is the original game.",
                    _ => "This player's. 100% is the original game.",
                }
                .into(),
            );
            let lock = held.as_deref().or(self.knob_held(knob));
            rows.push(r.off_if(lock.is_some(), lock.unwrap_or("")));
        }
        rows.push(Row::heading("Presets"));
        let names: Vec<&str> = self.settings.presets.iter().map(|(n, _)| n.as_str()).collect();
        let none = names.is_empty();
        let whose = match self.mods_seat() {
            None => "every player's",
            Some(_) => "this player's",
        };
        let load = match none {
            true => Row::choice(Key::LoadPreset, "Load Preset", &["None Saved"], 0, "A saved set of modifiers."),
            false => Row::choice(Key::LoadPreset, "Load Preset", &names, self.mod_preset, &format!("A saved set of modifiers, as {whose}.")),
        };
        rows.push(load.off_if(none, "No presets saved yet").off_if(held.is_some(), held.as_deref().unwrap_or("")));
        rows.push(Row::action(Key::SavePreset, "Save Preset", &format!("Name and keep {whose} set, with the world's. Any player can load it.")));
        rows.push(Row::action(Key::DeletePreset, "Delete Preset", "Forgets the preset Load Preset is on.").off_if(none, "No presets saved yet"));
        let reset = match self.mods_seat() {
            None => "Every modifier back to the original game.",
            Some(_) if own.is_some() => "Your own modifiers back to the original game.",
            Some(_) => "This player's modifiers back to the original game; the world's stay.",
        };
        rows.push(Row::action(Key::ResetMods, "Reset to Original", reset).off_if(held.is_some(), held.as_deref().unwrap_or("")));
        rows
    }

    /// The knobs a preset or a reset moves from the page: the seat's own, and the world's for the host.
    fn page_knobs(&self) -> Vec<Knob> {
        Knob::ALL.into_iter().filter(|&k| self.knob_held(k).is_none() && (k.per_seat() || self.mods_own().is_none())).collect()
    }

    /// Set `knob` to `to` from the page: every seat's on All Players (kept for the shell to take), the
    /// one seat's otherwise, the world's in row 0.
    fn set_knob(&mut self, knob: Knob, to: u16) {
        match (knob.per_seat(), self.mods_seat()) {
            (false, _) => self.settings.modifiers.set(0, knob, to),
            (true, Some(s)) => self.settings.modifiers.set(s, knob, to),
            (true, None) => {
                for s in 0..SEATS_MAX {
                    self.settings.modifiers.set(s, knob, to);
                }
                if !self.mods_all_set.contains(&knob) {
                    self.mods_all_set.push(knob);
                }
            }
        }
    }

    /// `from`'s values put on the page's seats (P134): on All Players the whole of it, seat by seat;
    /// on one seat that seat's row of it.
    fn load_mods(&mut self, from: &Mods) {
        for k in self.page_knobs() {
            match self.mods_seat() {
                None if k.per_seat() => {
                    (0..SEATS_MAX).for_each(|s| self.settings.modifiers.set(s, k, from.get(s, k)));
                    if !self.mods_all_set.contains(&k) {
                        self.mods_all_set.push(k);
                    }
                }
                Some(s) if k.per_seat() => self.settings.modifiers.set(s, k, from.get(s, k)),
                _ => self.settings.modifiers.set(0, k, from.get(0, k)),
            }
        }
    }

    /// The set Save Preset keeps, one player's (P143's [`preset_of`]): on one seat that seat's, on
    /// All Players 1P's (every seat's where they agree), with the world's.
    pub(super) fn preset_now(&self) -> Mods {
        preset_of(&self.settings.modifiers, self.mods_seat().unwrap_or(0))
    }

    /// Open the Game Modifiers page on its Player row, from the Ride tab, the pause or the lobby.
    pub(super) fn open_modifiers(&mut self) {
        self.push(Screen::Modifiers);
        self.cursor[Screen::Modifiers.index()] = 0;
    }

    /// The knobs the host set through All Players since this was last asked (P134), which online
    /// go to every seat and not only this machine's.
    pub fn take_mods_all(&mut self) -> Vec<Knob> {
        std::mem::take(&mut self.mods_all_set)
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

    /// The music packs the Mods page lists (P122), `(id, name, songs)`: every one the folder has had
    /// while it was read this session, in id order.
    pub fn music_packs(&self) -> &[(String, String, usize)] {
        &self.music_seen
    }

    /// How many of the Mods page's packs are on, of how many: the track packs and the music packs.
    pub fn packs_on(&self) -> (usize, usize) {
        let (packs, music) = (self.packs(), self.music_packs());
        let off = &self.settings.mods_off;
        let on = packs.iter().filter(|p| !off.contains(&p.prefix)).count() + music.iter().filter(|m| !off.contains(&m.0)).count();
        (on, packs.len() + music.len())
    }

    fn mods_rows(&self) -> Vec<Row> {
        let mut rows = vec![Row::on_off(Key::ModsFolder, "Read the Mods Folder", self.settings.mods, "Off, nothing is read from mods/ beside the disc and every pick is the disc's alone.")];
        rows.push(Row::heading("Installed"));
        let packs = self.packs();
        if packs.is_empty() && self.music_seen.is_empty() {
            rows.push(Row::heading("Nothing in the mods folder"));
        }
        for (i, p) in packs.iter().enumerate() {
            let on = !self.settings.mods_off.contains(&p.prefix);
            let tracks = if p.tracks == 1 { "1 track".to_string() } else { format!("{} tracks", p.tracks) };
            let mut r = Row::on_off(Key::Pack(i), &p.name, on, &format!("{tracks}, listed under Mods on Free Ride."));
            r.note = Some(format!("{} in the folder", p.prefix));
            rows.push(r.off_if(!self.settings.mods, "The mods folder is off"));
        }
        // The music packs beside them (P122), each switched by its whole id.
        for (i, (id, name, songs)) in self.music_packs().iter().enumerate() {
            let on = !self.settings.mods_off.contains(id);
            let songs = if *songs == 1 { "1 song".to_string() } else { format!("{songs} songs") };
            let mut r = Row::on_off(Key::MusicPack(i), name, on, &format!("Music pack, {songs}, for the User Soundtracks slider."));
            r.note = Some(format!("music/{id} in the folder"));
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

    /// Open `screen` (Settings or the Garage) on tab `tab` over the main menu, its cursor on the
    /// first row, for `--screen`'s tab words (P123).
    pub fn open_tab(&mut self, screen: Screen, tab: usize) {
        self.main();
        match screen {
            Screen::Garage => {
                self.open_garage();
                self.tab[Screen::Garage.index()] = tab.min(GARAGE_TABS.len() - 1);
                self.cursor[Screen::Garage.index()] = Menu::first_row(&self.page_of(Screen::Garage));
            }
            _ => self.open_settings(tab),
        }
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
        // the modifiers page's Player row, and whether it is the host's with All Players (P134)
        let mods_players = (self.screen() == Some(Screen::Modifiers)).then(|| self.mods_own().is_none());
        let s = &mut self.settings;
        match row.key {
            Key::Heading => return,
            Key::Player if mods_players.is_some() => {
                // All Players at the head of the row, the host's alone (P134).
                if mods_players == Some(true) {
                    let at = if self.mods_all { 0 } else { self.player.min(PLAYERS - 1) + 1 };
                    let to = cycle(at, d, PLAYERS + 1);
                    self.mods_all = to == 0;
                    if to > 0 {
                        self.player = to - 1;
                    }
                }
                return;
            }
            Key::Player => {
                let n = if self.screen() == Some(Screen::Garage) { self.garages.len().clamp(1, PLAYERS) } else { PLAYERS };
                self.player = cycle(self.player, d, n);
                return;
            }
            Key::StartOn => s.machines[0] = Machine::ALL[next(row)],
            Key::Wrecks => s.wrecks = !s.wrecks,
            Key::AirWrecks => s.air_wrecks = !s.air_wrecks,
            Key::Bays => s.bays = !s.bays,
            Key::Tricks => s.tricks = Tricks::ALL[next(row)],
            Key::Modifiers if p.ok => {
                self.open_modifiers();
                return;
            }
            Key::Hud => s.overlay = Overlay::CHOICES[next(row)],
            Key::Speedometer => s.speedometer = Speedometer::ALL[next(row)],
            Key::Minimap => s.minimap = !s.minimap,
            // Cross steps it up and round from the top, as a slider does.
            Key::HudScale => s.hud_scale = hud_scale_step(s.hud_scale, p.left, p.right),
            Key::DrawDistance => s.graphics = Graphics::ALL[next(row)],
            Key::WorldSky => s.world_sky = WorldSky::ALL[next(row)],
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
                let i = self.control_seat();
                let (layout, preset) = control_choices()[next(row)];
                self.controls.layouts[i] = layout;
                self.controls.presets[i] = preset;
            }
            Key::PlaneLayout => {
                let i = self.control_seat();
                self.controls.planes[i] = Layout::ALL[next(row)];
            }
            Key::Markers => {
                let i = self.control_seat().min(SEATS_MAX - 1);
                self.settings.markers[i] = next(row) == 0;
            }
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
                // From Mixed a step goes from 1P's.
                let now = self.knob_value(knob).unwrap_or_else(|| self.settings.modifiers.get(0, knob));
                let to = match (p.left, p.right) {
                    (true, _) => knob.step(now, false),
                    (_, true) => knob.step(now, true),
                    // Cross puts the one modifier back.
                    _ => game_sim::modifiers::ORIGINAL,
                };
                self.set_knob(knob, to);
            }
            Key::LoadPreset => {
                let n = self.settings.presets.len();
                if side {
                    self.mod_preset = cycle(self.mod_preset, d, n);
                    return;
                }
                let preset = self.settings.presets[self.mod_preset.min(n - 1)].1;
                self.load_mods(&preset);
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
            Key::ResetMods if p.ok => self.load_mods(&Mods::original()),
            Key::ModsFolder => s.mods = !s.mods,
            // From Mixed, a press is On first.
            Key::Original => self.set_original(next(row) == 1 || row.now.is_none()),
            Key::Departure(dep) => self.set_departure(dep, next(row) == 1 || row.now.is_none()),
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
            Key::MusicPack(i) => {
                let Some(id) = self.music_packs().get(i).map(|p| p.0.clone()) else { return };
                match self.settings.mods_off.iter().position(|p| *p == id) {
                    Some(at) => {
                        self.settings.mods_off.remove(at);
                    }
                    None => self.settings.mods_off.push(id),
                }
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

impl Departure {
    /// The Settings tab its switch is on (P128): the graphics ones are on Graphics, the rest under
    /// the Original tab's master switch.
    pub fn tab(self) -> usize {
        match self {
            Departure::Fog | Departure::Plants | Departure::Foliage | Departure::Picture => TAB_GRAPHICS,
            Departure::Controls | Departure::Cameras | Departure::Markers => TAB_ORIGINAL,
        }
    }

    /// The key of its switch's row: the fog's is Draw Distance (V32), the same setting.
    pub fn key(self) -> Key {
        match self {
            Departure::Fog => Key::DrawDistance,
            d => Key::Departure(d),
        }
    }
}

/// Draw Distance's names for the world's three.
pub fn graphics_label(g: Graphics) -> &'static str {
    match g {
        Graphics::Full => "Whole Map",
        Graphics::Far => "Far Fog",
        Graphics::Original => "Original Fog",
    }
}
