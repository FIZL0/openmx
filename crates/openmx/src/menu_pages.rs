//! The option pages (P118): Settings' six tabs, the Garage's three, the game modifiers and the
//! mods, each a list of [`Row`]s that change in place. A row knows its name, its value, every choice
//! it has, what it does and, dimmed, why it does not apply — the side panel draws all of it.

use super::*;
use game_sim::modifiers::Category;

/// Settings' tabs, which L1 and R1 step through, round from either end.
/// Display is the screen's HUD and its aspect ratio (P176); Graphics (P128) the world's draw, the picture and its rate, under
/// headings, with the Original Game page's graphics sub-switches (P126). The Original Game page is
/// the Ride tab's, a row there opening it (P179), and no tab of its own.
pub const SETTINGS_TABS: [&str; 6] = ["Ride", "Display", "Graphics", "Audio", "Controls", "Camera"];
pub const TAB_RIDE: usize = 0;
/// The Tricks row's help (MD16): what MX vs. ATV's tricks add, and what is the port's own.
const TRICKS_HELP: &str = "Original is MX Unleashed's tricks. Trick Packs adds the packs switched on below, from the mods folder: \
     extra stunts on Cross and Square (the bike layout's free buttons), alone, together and with Circle, Triangle and R1, \
     and with R1 held a sideways flick off a lip \
     for a 360 and a forward one for a front flip. The front flip is the port's own, in neither game, and shows the \
     backflip's pose. Online rides are always Original.";
/// The Garage's Handling row's help (MD40, P315d).
const HANDLING_HELP: &str = "How this player's bike handles. Original is MX Unleashed's bikes. MX vs. ATV rides that game's own bike files \
     from a trick pack in the mods folder, and its backflip, which only arms with R1 held as you pull back on the lip. Untamed rides the same \
     files with the steps MX vs. ATV Untamed reworked, as far as they are ported. A lobby you host rides 1P's \
     (Untamed's as MX vs. ATV's), and only players with the same trick pack can join its ride.";
/// The Brake Gyro row's help (P226).
const BRAKE_GYRO_HELP: &str = "MX vs. ATV's option: a brake held in the air spins the bike nose-up. On is that game's default. \
     Online and in a lobby the bikes keep the default.";
/// The Gas Gyro row's help (P226).
const GAS_GYRO_HELP: &str = "MX vs. ATV's option: the throttle held in the air spins the bike nose-down, when Brake Gyro is not \
     already pitching it. Off is that game's default. Online and in a lobby the bikes keep the default.";
/// The Air Wrecks row's help (P138).
const AIR_WRECKS_HELP: &str = "Whether the bike's angle in the air throws the rider off: upside down, nosed over or rolled too far. \
     Off, you ride every jump out, and only the landing is judged. On is the original game.";
/// The Backwards Landings Bail row's help (S161).
const BACKWARDS_BAIL_HELP: &str = "Whether a landing with the bike facing back against the way it is going throws the rider off. \
     The original game judges a crossed-up landing by an angle that reads one past about 135 degrees as nearly straight, so the bike \
     rides it out and swings round. Off is the original game.";
pub const TAB_DISPLAY: usize = 1;
pub const TAB_GRAPHICS: usize = 2;
pub const TAB_AUDIO: usize = 3;
pub const TAB_CONTROLS: usize = 4;
pub const TAB_CAMERA: usize = 5;
/// The Garage's tabs, each starting with the same Player row.
pub const GARAGE_TABS: [&str; 3] = ["Rider", "Machines", "Gear"];
/// The Garage's machine picker (P315c): the bike's class, model and paint and every flier kind on
/// one tab, MX Unleashed's beside each installed machine pack's. Its stage shows the bike, or the
/// flier whose row is under the cursor (P315a).
pub const TAB_MACHINES: usize = 1;
/// The tabs the machine picker took the place of (P315c), still words `--screen` takes for it.
const MACHINES_WAS: [&str; 2] = ["bike", "fliers"];

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

/// One preset's sliders (P163), in [`cameras::Look::get`]'s order.
const PRESET_CAMERAS: [(&str, &str); 3] = [
    ("Field of View", "How wide this preset's view is: the original's angle, opened or narrowed by this many degrees."),
    ("Distance", "How far behind its machine this preset sits."),
    ("Height", "How high above its machine this preset sits."),
];

/// What Music Plays offers, as `settings.txt`'s words; One Song is a song's id there.
const PLAYS: [&str; 4] = ["Shuffle", "In Order", "One Song", "Off"];
const PLAYS_ONE: usize = 2;

/// What Music From offers (MD7): every music pack on, or none, the disc's songs alone. A mix of
/// them shows as [`MUSIC_MIX`], which the packs' own rows below it make.
const MUSIC_FROM: [&str; 2] = ["All Games", "MX Unleashed Only"];
const MUSIC_MIX: &str = "A Mix";

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
    Wrecks,
    AirWrecks,
    BackwardsBail,
    /// Whether a wreck rumbles the pad (P183), every player's.
    Vibration,
    Bays,
    Modifiers,
    Hud,
    Speedometer,
    Minimap,
    HudScale,
    DrawDistance,
    WorldSky,
    TextureFiltering,
    /// What casts the sun's shadows (V108, user 2026-10-30): Off, On or All.
    Shadows,
    /// How they are drawn: the Shadows departure's row (the original's), with Low a third choice.
    ShadowQuality,
    /// The Ride tab's Updates (P191), on the desktop alone.
    Updates,
    /// The Display tab's Aspect Ratio (P176): the Picture departure's row, as Draw Distance is the fog's.
    AspectRatio,
    /// The Display tab's Button Icons (P281): Auto, or one style for every controller.
    ButtonIcons,
    RenderScale,
    VSync,
    FrameLimit,
    /// The Graphics tab's Show FPS (2026-10-29): the ride's frame rate in the top right.
    ShowFps,
    /// The Graphics tab's Preset row (P223): Auto, a preset, or Custom.
    VideoPreset,

    /// Model Detail (P223).
    Detail,
    Mix,
    /// A slider by its index in [`Mix::levels`].
    Level(usize),
    MusicPlays,
    /// Which games' music plays (MD7): every music pack's, or the disc's alone.
    MusicFrom,
    Song,
    SkipSong,
    /// Back to the song before, and the music held or played on (P174): the pause player's two.
    BackSong,
    HoldMusic,
    /// A seat's one Controls choice (P81): New, or Original on one of its controller presets.
    BikeLayout,
    /// The new layout's keyboard (P207): modern MX games' or P80's home row, the host's.
    Keyboard,
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
    /// Which preset the camera sliders move (P163): every preset, one of the disc's, or one of the
    /// player's own.
    CameraPreset,
    /// One preset's slider, by its index in [`PRESET_CAMERAS`].
    CameraOwn(usize),
    /// A disc preset back on every preset's numbers.
    CameraFollow,
    CameraSave,
    /// One of the player's own presets framed over by what Save as New Preset copies (P229).
    CameraSaveOver,
    CameraDelete,
    /// What the camera button cycles through.
    CameraCycle,
    Garage(roster::Row),
    Knob(Knob),
    /// The modifiers page's Machine row (P272): whose knobs show under it.
    ModsMachine,
    LoadPreset,
    SavePreset,
    /// The preset Load Preset is on, kept over with the set now (P229).
    SaveOverPreset,
    DeletePreset,
    ResetMods,
    ModsFolder,
    /// Browse the Store (P198).
    Store,
    /// Upload to the Store (P199): the store screen's Upload page.
    Upload,
    /// A mod pack by its place in [`Menu::packs`].
    Pack(usize),
    /// A music pack by its place in [`Menu::music_packs`] (P122).
    MusicPack(usize),
    Tricks,
    /// A trick pack's switch by its place in [`Menu::trick_packs`] (MD77).
    TrickPack(usize),
    /// How a seat's bike handles (MD40): the Garage's Bike tab row since P315d.
    Handling,
    /// MX vs. ATV's Brake Gyro and Gas Gyro options (P226).
    BrakeGyro,
    GasGyro,
    /// A flier kind's row on the Garage's Fliers tab (P315), by its place in
    /// [`roster::flier_kinds`].
    Flier(usize),
    /// A greyed row on the machine picker for a kind of machine openmx cannot ride (P315c), by its
    /// place in [`roster::other_kinds`].
    Machine(usize),
    /// The Original switch (P126), and each departure's under it.
    Original,
    /// The Ride tab's row into the Original Game page (P179).
    OriginalPage,
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

    /// Whether the row is a dropdown (P277, opentennis's B498): a choice of [`LIST`] or more. A row
    /// that picks something for a press to do (Load Preset, Save Over) keeps its left and right.
    pub fn many(&self) -> bool {
        self.kind == Kind::Choice && self.choices.len() >= LIST && !matches!(self.key, Key::LoadPreset | Key::CameraSaveOver | Key::Garage(_) | Key::Flier(_) | Key::Machine(_) | Key::Handling)
    }

    /// Whether Cross opens the row's dropdown now: a [`Row::many`] row that is not dimmed.
    pub fn listed(&self) -> bool {
        self.many() && self.off.is_none()
    }
}

/// P277: rows with this many choices or more open a dropdown list on Cross; an On/Off row and every
/// other two-way row keeps flipping on it.
pub const LIST: usize = 3;

/// P277: a dropdown choice's short hint, beside it in the list and the side panel, where the row's
/// own has none.
fn hint(key: Key, choice: &str) -> &'static str {
    match (key, choice) {
        (Key::Updates, "Ask") => "Square updates",
        (Key::Updates, "Always") => "updates by itself",
        (Key::Updates, "Never") => "never asks GitHub",
        (Key::AspectRatio, "Auto") => "fills the window",
        (Key::AspectRatio, "4:3") => "the original's television",
        (Key::AspectRatio, "16:9") => "widescreen",
        (Key::AspectRatio, "16:10") => "a laptop's shape",
        (Key::AspectRatio, "21:9") => "ultrawide",
        (Key::AspectRatio, "Stretch") => "fills, as Auto",
        (Key::ButtonIcons, "Auto") => "each controller's own",
        (Key::ButtonIcons, "PlayStation") => "Cross, Circle, Square, Triangle",
        (Key::ButtonIcons, "Xbox") => "A, B, X, Y",
        (Key::ButtonIcons, "Switch") => "Nintendo's B, A, Y, X",
        (Key::ButtonIcons, "Generic") => "where each button sits",
        (Key::VideoPreset, "Auto") => "picks and tunes itself",
        (Key::VideoPreset, "High") => "every shadow, far fog",
        (Key::VideoPreset, "Medium") => "High with speed-ups",
        (Key::VideoPreset, "Low") => "less detail, more fps",
        (Key::VideoPreset, "Steam Deck") => "for the Deck's screen",
        (Key::VideoPreset, "Phone") => "for phones and tablets",
        (Key::VideoPreset, "Custom") => "your own mix",
        (Key::DrawDistance, "Whole Map") => "everything, a light haze",
        (Key::DrawDistance, "Farther Fog") => "eight times as far",
        (Key::DrawDistance, "Far Fog") => "four times as far",
        (Key::DrawDistance, "Original Fog") => "the game's haze",
        (Key::TextureFiltering, "16x") => "sharpest far off",
        (Key::TextureFiltering, "8x" | "4x") => "sharp, a little faster",
        (Key::TextureFiltering, "2x") => "softer far off",
        (Key::TextureFiltering, "Off") => "the original's blur",
        (Key::Shadows, "Off") => "the ground's alone",
        (Key::Shadows, "On") => "bikes, riders, machines",
        (Key::Shadows, "All") => "the whole world too",
        (Key::ShadowQuality, "Original") => "the game's soft patch",
        (Key::ShadowQuality, "Low") => "cast onto everything",
        (Key::ShadowQuality, "High") => "sharper up close",
        (Key::RenderScale, "Auto") => "drops when frames run long",
        (Key::RenderScale, "100%") => "the window's resolution",
        (Key::RenderScale, "75%" | "50%") => "faster, softer",
        (Key::RenderScale, "40%") => "fastest",
        (Key::FrameLimit, "Off") => "as fast as it can",
        (Key::FrameLimit, "30 fps") => "the ride's own rate",
        (Key::FrameLimit, "Refresh rate") => "the display's",
        (Key::Mix, "Game Mix") => "the original's",
        (Key::Mix, "Rock-n-Roll Mix") => "music up, no crowd",
        (Key::Mix, "Night-time Mix") => "no music, quiet",
        (Key::Mix, "Engine Mix") => "no music, engines up",
        (Key::Mix, "Custom Mix") => "your own sliders",
        (Key::MusicPlays, "Shuffle") => "any song next",
        (Key::MusicPlays, "In Order") => "down the list",
        (Key::MusicPlays, "One Song") => "repeats the Song row's",
        (Key::MusicPlays, "Off") => "no music",
        (Key::CameraPreset, "All Presets") => "every camera at once",
        (Key::Handling, "Original") => "MX Unleashed's bikes",
        (Key::Handling, "MX vs. ATV") => "a trick pack's bike files",
        (Key::Handling, "Untamed") => "those files, Untamed's steps",
        _ => "",
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
            .or_else(|| MACHINES_WAS.contains(&w).then_some((Screen::Garage, TAB_MACHINES)))
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
        let mut rows = match screen {
            Screen::Settings => match self.tab(screen) {
                TAB_RIDE => self.ride_rows(),
                TAB_DISPLAY => self.display_rows(),
                TAB_GRAPHICS => self.graphics_rows(),
                TAB_AUDIO => self.audio_page(),
                TAB_CONTROLS => self.control_rows(),
                _ => self.camera_page(),
            },
            Screen::Garage => self.garage_rows(),
            Screen::Modifiers => self.modifier_page(),
            Screen::Mods => self.mods_rows(),
            Screen::Original => self.original_rows(),
            _ => Vec::new(),
        };
        // P277: each dropdown's choices with their short hints
        for r in rows.iter_mut().filter(|r| r.many() && r.tips.is_empty()) {
            let tips: Vec<&str> = r.choices.iter().map(|c| hint(r.key, c)).collect();
            if tips.iter().any(|t| !t.is_empty()) {
                r.tips = tips.into_iter().map(str::to_string).collect();
            }
        }
        rows
    }

    /// The open dropdown (P277): its row on the page shown and the choice it is on, while one is.
    pub fn listing(&self) -> Option<(usize, usize)> {
        let (screen, at, j) = self.listing?;
        (self.screen() == Some(screen) && self.cursor(screen) == at).then_some((at, j))
    }

    /// Open the dropdown of the row the cursor is on, if it is one (P277; `--open-list`'s, for a
    /// shot of an open list).
    pub fn open_list(&mut self) {
        let Some(screen) = self.screen() else { return };
        let at = self.cursor(screen);
        if let Some(r) = self.page_of(screen).get(at).filter(|r| r.listed()) {
            self.listing = Some((screen, at, r.now.unwrap_or(0)));
        }
    }

    /// One press while a dropdown is open (P277, opentennis's B498): up and down move round its
    /// choices, Cross picks the one it is on and closes it, Circle closes it as it was; nothing else.
    fn walk_list(&mut self, screen: Screen, p: Press) {
        let Some((at, j)) = self.listing() else {
            self.listing = None;
            return;
        };
        let Some(row) = self.page_of(screen).into_iter().nth(at).filter(Row::listed) else {
            self.listing = None;
            return;
        };
        if p.up || p.down {
            self.listing = Some((screen, at, cycle(j, if p.up { -1 } else { 1 }, row.choices.len())));
        } else if p.ok {
            self.listing = None;
            self.pick(&row, j);
        } else if p.back {
            self.listing = None;
        }
    }

    /// Choice `j` of a dropdown's `row` picked (P277): set as stepping onto it would set it. A choice
    /// that is only shown (the Preset's Custom), or One Song with no song to repeat, changes nothing.
    pub(super) fn pick(&mut self, row: &Row, j: usize) {
        if row.now == Some(j) || j >= row.choices.len() {
            return;
        }
        self.change_to(row, Press { ok: true, ..Press::default() }, Some(j));
    }

    /// Whether a screen is an option page.
    pub fn is_page(screen: Screen) -> bool {
        matches!(screen, Screen::Settings | Screen::Garage | Screen::Modifiers | Screen::Mods | Screen::Original)
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
        let changed = Knob::MENU
            .iter()
            .filter(|k| (0..SEATS_MAX).any(|s| self.settings.modifiers.get(s, **k) != game_sim::modifiers::ORIGINAL))
            .count()
            + self.wrecks_changed();
        let mut mods = Row::new(Key::Modifiers, Kind::Opens, "Game Modifiers", "Speed, grip, gravity, wrecks and the rest, per player. Saved presets live here too.");
        mods.value = match changed {
            0 => "Original".into(),
            n => format!("{n} Changed"),
        };
        // The Original Game page (P179), once a tab of Settings' own: Cross goes in, Circle back.
        let mut original = Row::new(
            Key::OriginalPage,
            Kind::Opens,
            "Original Game",
            "The game as it shipped, or openmx's enhancements: the master switch and the controls, cameras and markers' own switches. Cross opens them.",
        );
        original.value = self.original().label().into();
        // P191: only where there is an updater (the desktop's)
        let updates = UPDATER.get().map(|_| {
            Row::choice(Key::Updates, "Updates", &Updates::ALL.map(Updates::label), Updates::ALL.iter().position(|u| *u == self.settings.updates).unwrap_or(0), UPDATES_HELP)
        });
        let mut rows = vec![
            Row::on_off(Key::Bays, "Machines at Bays", self.settings.bays, "The lot's bikes and the biplane stand at their bays, see-through, and the original's pop-up offers them when you stop beside one."),
            mods,
        ];
        rows.extend(self.trick_rows());
        rows.push(original);
        rows.extend(updates);
        rows
    }

    /// The tricks' rows (MD16, MD77): Tricks, the original's or the trick packs', a switch for each
    /// pack in the folder, Handling, and with a pack in the folder the one Handling reads.
    fn trick_rows(&self) -> Vec<Row> {
        let s = &self.settings;
        let packs = self.trick_packs();
        let ids = self.trick_pack_ids();
        let on = s.trick_packs_on(&ids);
        let folder_off = (!s.mods).then_some("Needs a trick pack in the mods folder");
        let over = self.tricks_without_a_button();
        let warning = (over > 0).then(|| {
            let stunts = if over == 1 { "1 stunt".to_string() } else { format!("{over} stunts") };
            format!("{stunts} of the packs on have no button: past the {} gestures", game_sim::trick::EXTRA_SLOTS)
        });
        let mut tricks = Row::choice(Key::Tricks, "Tricks", &Tricks::ALL.map(Tricks::label), Tricks::ALL.iter().position(|t| *t == s.tricks).unwrap_or(0), TRICKS_HELP);
        if s.tricks == Tricks::MxVsAtv && !packs.is_empty() {
            tricks.value = match on.len() {
                0 => "None On".into(),
                1 => packs.iter().find(|p| p.id == on[0]).map_or_else(|| on[0].clone(), |p| p.name.clone()),
                n => format!("{n} Packs"),
            };
        }
        tricks.note = warning.clone();
        let mut rows = vec![tricks.off_if(folder_off.is_some(), folder_off.unwrap_or_default())];
        for (i, p) in packs.iter().enumerate() {
            let about = format!(
                "Trick pack, {} stunts past the rider's own on Cross, Square and their combinations. Packs on stack: a stunt two of them have is the first's, in this order.",
                p.extras.len()
            );
            let lit = on.contains(&p.id);
            let mut r = Row::on_off(Key::TrickPack(i), &p.name, lit, &about);
            r.note = if lit { warning.clone() } else { None }.or_else(|| Some(format!("tricks/{} in the mods folder", p.id)));
            rows.push(r.off_if(folder_off.is_some(), folder_off.unwrap_or_default()));
        }
        // P226: MX vs. ATV's own two gyro options, which Untamed's steps read as well; the handling
        // itself is each player's, on the Garage's Bike tab (P315d)
        let handled = self.picks.0.iter().any(|p| p.handling_of(roster::BIKE).is_some());
        let why = if folder_off.is_some() { folder_off } else { (!handled).then_some("Only on MX vs. ATV or Untamed handling: the Garage's Machines tab") };
        for (key, name, on, help) in [(Key::BrakeGyro, "Brake Gyro", s.gyro.brake, BRAKE_GYRO_HELP), (Key::GasGyro, "Gas Gyro", s.gyro.gas, GAS_GYRO_HELP)] {
            rows.push(Row::on_off(key, name, on, help).off_if(why.is_some(), why.unwrap_or_default()));
        }
        rows
    }

    /// How many of Wrecks and Air Wrecks are off the original's on (P175), for the Game Modifiers
    /// row's count.
    fn wrecks_changed(&self) -> usize {
        usize::from(!self.settings.wrecks) + usize::from(!self.settings.air_wrecks) + usize::from(self.settings.backwards_bail)
    }

    /// The modifiers page's Wrecks and Air Wrecks (P175, from the Ride tab's P138 rows): the
    /// world's, one switch for every player as the room and the tape carry one, set by the host
    /// alone; Air Wrecks has nothing to switch with Wrecks off, and under the pause it takes as the
    /// pause closes, unless the ride holds it.
    fn wrecks_rows(&self) -> Vec<Row> {
        let host_only = self.mods_own().map(|_| "The world's: only the host sets it.");
        let mut wrecks = Row::on_off(Key::Wrecks, "Wrecks", self.settings.wrecks, "Whether a hard landing throws the rider off. Off, you ride out every crash.");
        wrecks.world = true;
        wrecks.note = Some("The world's: one value for every player. On is the original game.".into());
        let mut air = Row::on_off(Key::AirWrecks, "Air Wrecks", self.settings.air_wrecks, AIR_WRECKS_HELP);
        air.world = true;
        air.note = Some(match self.stack.first() {
            Some(Screen::Pause) => "Changes take when the pause closes.".into(),
            _ => "The world's: one value for every player. On is the original game.".into(),
        });
        let air = air
            .off_if(!self.settings.wrecks, "Wrecks are off")
            .off_if(self.air_held().is_some(), self.air_held().unwrap_or(""))
            .off_if(host_only.is_some(), host_only.unwrap_or(""));
        // S161's, not the original's: off is the original, and like Air Wrecks it is the world's.
        let mut back = Row::on_off(Key::BackwardsBail, "Backwards Landings Bail", self.settings.backwards_bail, BACKWARDS_BAIL_HELP);
        back.world = true;
        back.note = Some(match self.stack.first() {
            Some(Screen::Pause) => "Changes take when the pause closes.".into(),
            _ => "The world's: one value for every player. Off is the original game.".into(),
        });
        // Held where Air Wrecks is (a played tape, an online room), for the same reasons.
        let back = back
            .off_if(!self.settings.wrecks, "Wrecks are off")
            .off_if(self.air_held().is_some(), "The tape or the room holds it for the ride")
            .off_if(host_only.is_some(), host_only.unwrap_or(""));
        vec![Row::heading("Wrecks"), wrecks.off_if(host_only.is_some(), host_only.unwrap_or("")), air, back]
    }

    fn display_rows(&self) -> Vec<Row> {
        let s = &self.settings;
        let hud = Overlay::CHOICES.map(Overlay::label);
        let speed = Speedometer::ALL.map(Speedometer::label);
        let speed_off = (s.overlay == Overlay::None).then_some("The HUD is off");
        vec![
            Row::choice(Key::Hud, "HUD", &hud, at(&Overlay::CHOICES, s.overlay), "What a ride draws over the picture. HUD is openmx's own, sharp at any size; None leaves the picture clean.")
                .tips(&["Sharp at any size", "Nothing over the ride"]),
            Row::choice(Key::Speedometer, "Speedometer", &speed, at(&Speedometer::ALL, s.speedometer), "How the HUD shows your speed: the readout or the dial.")
                .off_if(speed_off.is_some(), speed_off.unwrap_or("")),
            // V52: the radar alone, and the whole HUD's size about its corners.
            Row::on_off(Key::Minimap, "Minimap", s.minimap, "The HUD's radar in the corner. Off, it goes from every view and the speedometer stays.")
                .off_if(speed_off.is_some(), speed_off.unwrap_or("")),
            Row::slider(
                Key::HudScale,
                "HUD Scale",
                format!("{}%", s.hud_scale),
                f32::from(s.hud_scale - HUD_SCALE_MIN) / f32::from(HUD_SCALE_MAX - HUD_SCALE_MIN),
                "The size of the whole HUD, each piece in its own corner. 100% is the original's.",
            )
            .off_if(s.overlay == Overlay::None, "The HUD is off"),
            // P176: opentennis's Aspect Ratio, the Picture departure's switch.
            Row::choice(Key::AspectRatio, "Aspect Ratio", &Picture::ALL.map(Picture::label), at(&Picture::ALL, s.picture), ASPECT_HELP),
            // P281: opentennis's Button Icons, Auto by default.
            Row::choice(Key::ButtonIcons, "Button Icons", &ButtonIcons::ALL.map(ButtonIcons::label), at(&ButtonIcons::ALL, s.button_icons), super::pads::HELP),
        ]
    }

    /// The Graphics tab (P128): each graphics setting once, under the group it is in. The Original
    /// tab's graphics sub-switches (P126) are here and not there: Draw Distance is its fog, and
    /// Leaves its own row (Plant Detail left on 2026-10-29: ours alone). Picture Shape went to the
    /// Display tab as Aspect Ratio (P176).
    fn graphics_rows(&self) -> Vec<Row> {
        let s = &self.settings;
        let scales = RenderScale::ALL.map(RenderScale::label);
        let limits: Vec<String> = FrameLimit::ALL.iter().map(|f| f.label()).collect();
        let limits: Vec<&str> = limits.iter().map(String::as_str).collect();
        // P223: the preset over every row it sets; Custom is only ever shown.
        let presets = super::video::row_labels();
        let preset_at = super::video::row_at(s);
        let mut preset = Row::choice(
            Key::VideoPreset,
            "Preset",
            &presets,
            preset_at,
            "Sets every row below that costs frames. Auto picks for this device and, in the first seconds of each ride, steps them down until the frames keep up with the display. Changing a row yourself turns Auto off.",
        );
        if preset_at == presets.len() - 1 {
            preset.note = Some("Rows set by hand. A press goes back to Auto.".into());
        }
        vec![
            preset,
            Row::heading("World"),
            self.fog_row(),
            self.departure_row(Departure::Foliage),
            self.departure_row(Departure::Textures),
            // MD9: a combined world's sky and light, one for all of it or each map its own.
            Row::choice(
                Key::WorldSky,
                "World Sky",
                &WorldSky::ALL.map(WorldSky::label),
                at(&WorldSky::ALL, s.world_sky),
                "For combined worlds of several maps. Global keeps one sky and fog everywhere, each map still lit by its own sun; Per Map gives each map its own sky too, blended where they meet.",
            )
            .tips(&["One sky for the whole world", "Each map its own"]),
            Row::heading("Picture"),
            // V83: the Original switch's too, but five choices to its two, as Draw Distance's three.
            Row::choice(
                Key::TextureFiltering,
                "Texture Filtering",
                &Filtering::ALL.map(Filtering::label),
                at(&Filtering::ALL, s.filtering),
                FILTERING_HELP,
            ),
            // V108 (user 2026-10-30): what casts, and how it is drawn, the Original switch's.
            Row::choice(Key::Shadows, "Shadows", &Shadows::ALL.map(Shadows::label), at(&Shadows::ALL, s.shadows), SHADOWS_HELP),
            Row::choice(
                Key::ShadowQuality,
                "Shadow Quality",
                &ShadowQuality::ALL.map(ShadowQuality::label),
                at(&ShadowQuality::ALL, s.shadow_quality),
                SHADOW_QUALITY_HELP,
            )
            .off_if(s.shadows == Shadows::Off, "Shadows are off"),
            Row::choice(
                Key::Detail,
                "Model Detail",
                &super::video::Detail::ALL.map(super::video::Detail::label),
                at(&super::video::Detail::ALL, s.detail),
                "Full draws each model as detailed as the game picks for its distance. Reduced switches to the simpler ones at half the distance, which is faster.",
            ),
            Row::heading("Performance"),
            Row::choice(Key::RenderScale, "Render Scale", &scales, at(&RenderScale::ALL, s.render_scale), "How much of the window's resolution the 3D is drawn at. Auto drops it when frames run long and climbs back when there's room."),
            Row::on_off(Key::VSync, "VSync", s.vsync, "Waits for the display before showing a frame. No tearing. Off draws as fast as it can."),
            Row::choice(Key::FrameLimit, "Frame Limit", &limits, at(&FrameLimit::ALL, s.frame_limit), "The most frames a second. Only the picture's rate; the ride itself always ticks at the game's 30 Hz."),
            Row::on_off(Key::ShowFps, "Show FPS", s.show_fps, "The frame rate in the top right corner while you ride, updated twice a second."),
        ]
    }

    /// The Fog slider (P276, user 2026-11-03): the original's haze and far clip at its left end, the
    /// same further out to the right, and the whole map past them. Over a ride it shows how far that
    /// is on this track, in feet.
    fn fog_row(&self) -> Row {
        let g = self.settings.graphics;
        let at = Graphics::SLIDER.iter().position(|s| *s == g).unwrap_or(0);
        let value = match (g.multiple(), self.ride_fog.filter(|_| self.stack.first() == Some(&Screen::Pause))) {
            (Some(k), Some(end)) => format!("{} · {} ft", graphics_label(g), thousands((k * end).round() as u64)),
            _ => graphics_label(g).to_string(),
        };
        let mut r = Row::slider(
            Key::DrawDistance,
            "Fog",
            value,
            at as f32 / (Graphics::SLIDER.len() - 1) as f32,
            "How far out the haze and the draw distance are. Original is the game's own; each step right is the same haze further out, up to sixteen times as far, then Whole Map, which draws everything with only a light haze far off.",
        );
        r.note = Some(match g.multiple() {
            Some(k) if k <= 1.0 => "The original game's fog.".into(),
            Some(k) => format!("{} times as far as the original's.", k as u32),
            None => "No fog end: the whole map, everywhere.".into(),
        });
        r
    }

    /// The Original Game page (P126, the Ride tab's since P179): the master switch, and every
    /// departure's own under it but the Graphics tab's.
    fn original_rows(&self) -> Vec<Row> {
        let mut master = Row::choice(
            Key::Original,
            "Original Game",
            &["Off", "On"],
            0,
            "On puts everything back to the original game: its controls, cameras, fog, leaves, aspect ratio, texture filtering and shadows; the HUD stays openmx's. Each switch below can be set on its own, the fog, leaves, texture filtering and shadows on the Graphics tab, and the aspect ratio on the Display tab. The ride itself is the original's either way; Game Modifiers change it.",
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
        for d in Departure::ALL.into_iter().filter(|d| d.tab() == TAB_RIDE) {
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
        rows.extend(self.music_from_rows());
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
        let off = s.music == "off";
        rows.push(Row::action(Key::BackSong, "Previous Song", "Back to the song before this one.").off_if(off, "The music is off"));
        rows.push(match self.music_held {
            true => Row::action(Key::HoldMusic, "Play Music", "The music is paused. On from where it stopped."),
            false => Row::action(Key::HoldMusic, "Pause Music", "Hold the song where it is, until Play Music."),
        }
        .off_if(off, "The music is off"));
        rows
    }

    /// Which games' music plays (MD7): Music From, then each music pack's switch, the Mods page's
    /// own. None without a pack, so the page is the original's then.
    fn music_from_rows(&self) -> Vec<Row> {
        let packs = self.music_packs();
        if packs.is_empty() {
            return Vec::new();
        }
        let off = &self.settings.mods_off;
        let on = packs.iter().filter(|p| !off.contains(&p.0)).count();
        let folder_off = (!self.settings.mods).then_some("The mods folder is off");
        let mut from = Row::choice(
            Key::MusicFrom,
            "Music From",
            &MUSIC_FROM,
            usize::from(on == 0),
            "MX Unleashed's own songs always play. All Games adds every music pack's; the packs below choose one by one.",
        );
        if on > 0 && on < packs.len() {
            (from.now, from.value) = (None, MUSIC_MIX.into());
        }
        let mut rows = vec![from.off_if(folder_off.is_some(), folder_off.unwrap_or_default())];
        for (i, (id, name, songs)) in packs.iter().enumerate() {
            let songs = if *songs == 1 { "1 song".to_string() } else { format!("{songs} songs") };
            let mut r = Row::on_off(Key::MusicPack(i), name, !off.contains(id), &format!("Music pack, {songs}, in the shuffle beside the disc's."));
            r.note = Some(format!("music/{id} in the mods folder"));
            rows.push(r.off_if(folder_off.is_some(), folder_off.unwrap_or_default()));
        }
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

    /// The Controls tab shows the keyboard's keys rather than the pad (P178): its seat rides the
    /// keyboard, not a pad.
    pub fn controls_on_keys(&self) -> bool {
        self.keys_seat == Some(self.control_seat())
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
        // The new layout's keyboard (P207): which keys, not which buttons, so the original's layout,
        // which keeps the keyboard it always had, has no say in it.
        let keyboard = Row::choice(
            Key::Keyboard,
            "Keyboard",
            &["Modern", "Home Row"],
            usize::from(c.home_row_keys),
            "Modern: W the gas, S the brake, A and D steer, I and K lean, Shift the clutch, F G and E the tricks, R the reset, as modern MX games have it. Home Row: J the gas and K the brake, WASD and YUIO the sticks.",
        )
        .tips(&["W gas, S brake", "J gas, K brake"])
        .off_if(c.layouts[p] == Layout::Original, "The original layout keeps its own keyboard");
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
            keyboard,
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
            Row::on_off(
                Key::Vibration,
                "Vibration",
                self.settings.vibration,
                "Whether a pad rumbles when its rider wrecks, as the original's did. Every player's pad. On is the original game.",
            ),
        ]);
        rows
    }

    fn camera_page(&self) -> Vec<Row> {
        let seat = self.camera_seat();
        let v = self.settings.cameras[seat];
        let set = &self.settings.camera_sets[seat];
        let mut rows = vec![self.player_row("Whose camera these sliders move. Split-screen players each have their own.")];
        let picked = self.camera_picked();
        let mut choices = vec!["All Presets".to_string()];
        choices.extend(self.camera_choices().iter().map(cameras::Pick::label));
        let mut r = Row::new(Key::CameraPreset, Kind::Choice, "Preset", "Which camera the sliders below move: all of them at once, or one preset on its own. A preset you have not moved follows All Presets.");
        r.now = Some(if picked.is_some() { self.camera_pick } else { 0 });
        r.value = choices[r.now.unwrap_or(0)].clone();
        let lone = choices.len() == 1;
        r.choices = choices;
        rows.push(r.off_if(lone, "The disc's camera presets were not read, and you have none of your own."));
        let meter = |slider: usize, v: i32| {
            let (lo, hi) = match slider {
                0 => (i32::from(CAMERA_FOV.0), i32::from(CAMERA_FOV.1)),
                _ => (i32::from(CAMERA_SHARE.0), i32::from(CAMERA_SHARE.1)),
            };
            (v - lo) as f32 / (hi - lo) as f32
        };
        match &picked {
            None => {
                for (i, (label, about)) in CAMERAS.iter().enumerate() {
                    let mut r = Row::slider(Key::Camera(i), label, v.label(i), meter(i, v.get(i)), about);
                    r.note = Some("Cross puts it back to the original's.".into());
                    rows.push(r);
                }
                rows.push(Row::action(Key::CameraDefault, "Reset to Default", "Back to the port's defaults for this player."));
                rows.push(Row::action(Key::CameraOriginal, "Reset to Original", "Back to the disc's camera for this player."));
            }
            Some(pick) => {
                let look = self.camera_look(pick);
                for (i, (label, about)) in PRESET_CAMERAS.iter().enumerate() {
                    let mut r = Row::slider(Key::CameraOwn(i), label, look.label(i), meter(i, look.get(i)), about);
                    r.note = Some("Cross puts it back to the original's.".into());
                    rows.push(r);
                }
                if pick.custom {
                    rows.push(Row::action(Key::CameraDelete, "Delete Preset", "Forget this preset of yours."));
                } else {
                    rows.push(
                        Row::action(Key::CameraFollow, "Same as All Presets", "This preset back on All Presets' numbers.")
                            .off_if(set.own_of(pick.machine, &pick.name).is_none(), "It is on All Presets' numbers already."),
                    );
                }
            }
        }
        rows.push(
            Row::action(
                Key::CameraSave,
                "Save as New Preset",
                "Keep this preset, framed as it is now, under a name of your own: the camera button reaches it after the disc's. On All Presets, the preset this player's rides start on, as All Presets frames it.",
            )
            .off_if(self.camera_to_save().is_none(), "The disc's camera presets were not read, and you have none of your own."),
        );
        let overs: Vec<String> = self.camera_over_choices().iter().map(cameras::Pick::label).collect();
        let about = "What Save as New Preset would keep, written over a preset of your own in its place and under its name. Left and right pick which; Cross asks first.";
        let over = match overs.is_empty() {
            true => Row::choice(Key::CameraSaveOver, "Save Over", &["None of Yours"], 0, about),
            false => {
                let names: Vec<&str> = overs.iter().map(String::as_str).collect();
                Row::choice(Key::CameraSaveOver, "Save Over", &names, self.camera_over, about)
            }
        };
        rows.push(over.off_if(overs.is_empty(), "You have no preset of your own on this machine to save over."));
        let cycles: Vec<&str> = cameras::Cycle::ALL.iter().map(|c| c.label()).collect();
        rows.push(
            Row::choice(Key::CameraCycle, "Camera Button", &cycles, at(&cameras::Cycle::ALL, set.cycle), "What the camera button steps through: the disc's presets, the disc's and then yours, or yours in place of the disc's.")
                .tips(&["The disc's presets alone.", "The disc's, then yours.", "Yours alone, while you have some for that machine."]),
        );
        rows
    }

    /// The pick the Garage shows (P213): the seat its Player row is on, while it is the screen.
    pub fn garage_pick(&self) -> Option<&roster::Pick> {
        (self.screen() == Some(Screen::Garage)).then(|| self.garages.get(self.garage_seat()).map(|g| &g.pick)).flatten()
    }

    /// The picks one step left and one right of the Garage row under the cursor (P258), for the
    /// stage to dress ahead of the press. None off the Garage, or on a row the stage does not show.
    pub fn garage_neighbours(&self) -> Vec<roster::Pick> {
        let Some(pick) = self.garage_pick() else { return Vec::new() };
        let Some(g) = self.garages.get(self.garage_seat()) else { return Vec::new() };
        let page = self.page_of(Screen::Garage);
        let Some(Key::Garage(row)) = page.get(self.cursor(Screen::Garage)).map(|r| r.key) else { return Vec::new() };
        let Some(at) = roster::ROWS.iter().position(|r| *r == row) else { return Vec::new() };
        [1, -1]
            .into_iter()
            .filter_map(|by| {
                let mut next = g.clone();
                next.row = at;
                next.turn(&self.catalogue, by);
                (next.pick != *pick).then_some(next.pick)
            })
            .collect()
    }

    /// The flier kind the Garage's stage shows on its machine picker (P315a, P315c): the kind whose
    /// row is under the cursor. None off a flier row, where the stage shows the bike.
    fn garage_flier_kind(&self) -> Option<String> {
        if !self.on_machines() {
            return None;
        }
        let page = self.page_of(Screen::Garage);
        let Some(Key::Flier(at)) = page.get(self.cursor(Screen::Garage)).map(|r| r.key) else { return None };
        roster::flier_kinds(&self.flier_offer).get(at).cloned()
    }

    /// Whether the Garage is on its machine picker (P315c).
    pub(super) fn on_machines(&self) -> bool {
        self.screen() == Some(Screen::Garage) && self.tab(Screen::Garage) == TAB_MACHINES
    }

    /// The stage's arrows on the machine picker (P315a, P315c), `-1` left or `1` right: the shown
    /// flier's next skin, or with the bike shown its next paint, as left and right on those rows
    /// step them. Whether there was one to step.
    pub(super) fn step_garage_machine(&mut self, by: i32) -> bool {
        if self.garage_flier().is_some() {
            return self.step_garage_flier(by);
        }
        let seat = self.garage_seat();
        let Some(at) = roster::ROWS.iter().position(|r| *r == roster::Row::Livery) else { return false };
        let Some(g) = self.garages.get_mut(seat) else { return false };
        let (was, row) = (g.pick.clone(), g.row);
        g.row = at;
        g.turn(&self.catalogue, by);
        g.row = row;
        let moved = g.pick != was;
        if moved {
            self.changed = true;
        }
        moved
    }

    /// The flier the Garage's stage turns on its Fliers tab (P315a): the seat's pick of the kind
    /// [`Menu::garage_flier_kind`] says, or, for a kind none of whose skins can fly, its first.
    pub fn garage_flier(&self) -> Option<&roster::Flier> {
        let kind = self.garage_flier_kind()?;
        let pick = self.garage_pick()?;
        roster::flier_of(&self.flier_offer, pick, &kind).or_else(|| roster::flier_choices(&self.flier_offer, &kind).first().copied())
    }

    /// The fliers one step either side of the shown one (P315a), for the stage to dress ahead.
    pub fn garage_flier_neighbours(&self) -> Vec<roster::Flier> {
        let (Some(kind), Some(pick), Some(shown)) = (self.garage_flier_kind(), self.garage_pick(), self.garage_flier()) else { return Vec::new() };
        [1, -1]
            .into_iter()
            .filter_map(|by| {
                let mut next = pick.clone();
                roster::step_flier(&self.flier_offer, &mut next, &kind, by);
                roster::flier_of(&self.flier_offer, &next, &kind).filter(|f| *f != shown).cloned()
            })
            .collect()
    }

    /// The stage's arrows on the Fliers tab (P315a), `-1` left or `1` right: the shown kind's next
    /// skin for the seat, as left and right on its row step it. Whether there was one to step.
    pub(super) fn step_garage_flier(&mut self, by: i32) -> bool {
        let Some(kind) = self.garage_flier_kind() else { return false };
        let seat = self.garage_seat();
        let offer = self.flier_offer.clone();
        let Some(g) = self.garages.get_mut(seat) else { return false };
        let was = g.pick.clone();
        roster::step_flier(&offer, &mut g.pick, &kind, by);
        let moved = g.pick != was;
        if moved {
            self.changed = true;
        }
        moved
    }

    /// Where the Garage stands its pick in a `size` frame, in pixels: x, y, width, height. None off
    /// the Garage.
    pub fn garage_stage(&self, size: (usize, usize)) -> Option<[f32; 4]> {
        (self.screen() == Some(Screen::Garage)).then(|| super::screens::garage_stage(size))
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
                rows.push(row(G::Rider, "Gear Set", "Whose gear the rider wears: a maker's, or a pro's own."));
                rows.push(row(G::Colours, "Colours", "The rider's two colours, used on the gear below."));
                rows.push(row(G::Outfit, "Jersey/Pants", "One pick for the two, as they come as a set."));
                let mut save = Row::new(Key::Garage(G::Preset), Kind::Action, "Save as Preset", "Keeps this rider, bike and gear under a name, for the next session or another seat.");
                save.value = value(G::Preset);
                save.note = Some("Left and right load a saved one.".into());
                rows.push(save);
            }
            TAB_MACHINES => {
                rows.extend(self.bike_rows(seat));
                rows.push(row(G::Tier, "Driving Model", "Race Physics, or Pro Physics: faster bikes."));
                rows.push(self.handling_row(seat));
                rows.extend(self.flier_rows(seat));
                rows.extend(self.other_machine_rows());
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

    /// The machine picker's bike rows (P315c): Bike Class, Bike and Paint, stepping the disc's bikes
    /// as the Garage always has, with every installed machine pack's bikes listed beside them by
    /// their class and game, greyed with why while openmx rides only the disc's.
    fn bike_rows(&self, seat: usize) -> Vec<Row> {
        use roster::Row as G;
        let Some(g) = self.garages.get(seat) else { return Vec::new() };
        let c = &self.catalogue;
        let offer = &self.machine_offer;
        let class = roster::cc(g.pick.set.engine).to_string();
        let bike = c.bike(&g.pick.bike);
        let ride = |name: String, why: &Option<String>| if why.is_some() { format!("{name} (cannot ride)") } else { name };
        let mut rows = Vec::new();

        let mut r = Row::new(Key::Garage(G::Class), Kind::Choice, "Bike Class", "The bike's class: MX Unleashed's, and every installed machine pack's.");
        r.value = g.value(c, G::Class);
        let classes = roster::class_choices(c, offer);
        r.now = classes.iter().position(|(k, _)| *k == class);
        r.choices = classes.iter().map(|(k, ok)| ride(roster::class_label(k), &(!ok).then(String::new))).collect();
        rows.push(r);

        let mut r = Row::new(Key::Garage(G::Model), Kind::Choice, "Bike", "The bike in this class, MX Unleashed's first and then every installed machine pack's, grouped by game.");
        r.value = g.value(c, G::Model);
        let models = bike.map_or_else(Vec::new, |b| c.models(b.engine));
        r.now = bike.and_then(|b| models.iter().position(|m| *m == b.model));
        r.choices = models.iter().map(|m| c.model_name(m)).collect();
        let packs = roster::pack_bikes(offer, &class);
        r.choices.extend(packs.iter().map(|m| ride(m.name.clone(), &m.why)));
        r.note = Some(match packs.first() {
            Some(m) => {
                let games: Vec<&str> = packs.iter().fold(Vec::new(), |mut v, m| {
                    if !v.contains(&m.game.as_str()) {
                        v.push(&m.game);
                    }
                    v
                });
                let why = m.why.as_deref().map_or(String::new(), |w| format!(": {w}"));
                format!("MX Unleashed's own. Also {}'s {}{why}.", games.join(", "), roster::class_label(&class))
            }
            None => "MX Unleashed's own.".into(),
        });
        rows.push(r);

        let mut r = Row::new(Key::Garage(G::Livery), Kind::Choice, "Paint", "The bike's livery. The stage's arrows step it too.");
        r.value = g.value(c, G::Livery);
        let paints: Vec<&roster::Bike> = bike.map_or_else(Vec::new, |b| c.liveries(&b.model).into_iter().filter(|x| x.engine == b.engine).collect());
        r.now = paints.iter().position(|x| x.archive == g.pick.bike);
        r.choices = paints.iter().map(|x| c.paint_name(&x.archive)).collect();
        rows.push(r);
        rows
    }

    /// The machine picker's rows for the kinds openmx cannot ride (P315c), one a kind, listing the
    /// packs that bring it, greyed with why: the installed quads until openmx rides one.
    fn other_machine_rows(&self) -> Vec<Row> {
        let offer = &self.machine_offer;
        roster::other_kinds(offer, &self.flier_offer)
            .iter()
            .enumerate()
            .map(|(i, kind)| {
                let of: Vec<&roster::PackMachine> = offer.iter().filter(|m| m.machine == *kind).collect();
                let label = roster::machine_label(kind);
                let mut r = Row::new(Key::Machine(i), Kind::Choice, &label, &format!("The {}s the installed machine packs bring.", label.to_lowercase()));
                r.choices = of.iter().map(|m| format!("{} (cannot ride)", m.name)).collect();
                r.value = of.first().map(|m| m.name.clone()).unwrap_or_default();
                r.note = Some(of.iter().map(|m| m.game.as_str()).fold(Vec::<&str>::new(), |mut v, g| {
                    if !v.contains(&g) {
                        v.push(g);
                    }
                    v
                }).join(", "));
                let why = of.iter().find_map(|m| m.why.clone()).unwrap_or_default();
                r.off_if(true, &why)
            })
            .collect()
    }

    /// The handling a machine's row offers (P315d): the original's, then each trick pack's in the
    /// mods folder that lists bike files, as [`roster::handling_choices`] reads their manifests.
    /// None but the original's with the mods folder off.
    pub(super) fn handling_choices(&self) -> Vec<roster::HandlingChoice> {
        let packs: Vec<roster::HandlingPack> = match self.settings.mods {
            true => self.trick_packs().iter().map(|p| roster::HandlingPack { id: p.id.clone(), name: p.name.clone(), phy: p.phy }).collect(),
            false => Vec::new(),
        };
        roster::handling_choices(&packs, roster::BIKE)
    }

    /// The pack a pick naming none rides (P315d), as an older Handling From's First Pack On did:
    /// the first trick pack on.
    pub(super) fn first_handling_pack(&self) -> Option<String> {
        self.settings.trick_packs_on(&self.trick_pack_ids()).into_iter().next()
    }

    /// The Bike tab's Handling row (P315d): the seat's pick of how its bike handles, out of what the
    /// installed trick packs' manifests offer. With none offering, the original's alone, greyed with
    /// where the others come from.
    fn handling_row(&self, seat: usize) -> Row {
        let choices = self.handling_choices();
        let pick = self.garages.get(seat).map(|g| g.pick.clone()).unwrap_or_default();
        let first = self.first_handling_pack();
        let now = roster::handling_of(&choices, &pick, roster::BIKE, first.as_deref());
        let mut r = Row::new(Key::Handling, Kind::Choice, "Handling", HANDLING_HELP);
        r.choices = choices.iter().map(|c| c.label.clone()).collect();
        r.now = choices.iter().position(|c| c == now);
        r.value = now.label.clone();
        r.note = Some(now.about.clone());
        let why = match (self.settings.mods, choices.len()) {
            (false, _) => Some("Needs a trick pack in the mods folder"),
            (true, 1) => Some("MX vs. ATV's and Untamed's come in trick packs in the mods folder's tricks/"),
            _ => None,
        };
        r.off_if(why.is_some(), why.unwrap_or_default())
    }

    /// The Garage's Fliers tab (P315): a row per kind of flier on offer, stepping through every
    /// installed pack's skins of it after MX Unleashed's, grouped by the game they come from. A skin
    /// openmx cannot fly is listed, greyed with why, and stepped over.
    fn flier_rows(&self, seat: usize) -> Vec<Row> {
        let offer = self.fliers();
        let pick = self.garages.get(seat).map(|g| g.pick.clone()).unwrap_or_default();
        let mut rows = Vec::new();
        for (i, kind) in roster::flier_kinds(offer).iter().enumerate() {
            let label = roster::flier_label(kind);
            let choices = roster::flier_choices(offer, kind);
            let about = format!("The {} the garage and Switch To Machine give this player. Left and right, or L2 and R2, step its skins, every installed machine pack's after MX Unleashed's.", label.to_lowercase());
            let mut r = Row::new(Key::Flier(i), Kind::Choice, &label, &about);
            r.choices = choices.iter().map(|f| if f.why.is_some() { format!("{} (cannot fly)", f.name) } else { f.name.clone() }).collect();
            match roster::flier_of(offer, &pick, kind) {
                Some(f) => {
                    r.now = choices.iter().position(|c| std::ptr::eq(*c, f));
                    r.value = f.name.clone();
                    r.note = Some(format!("{}. {}", f.game, f.about));
                    rows.push(r);
                }
                None => {
                    let why = choices.iter().find_map(|f| f.why.clone()).unwrap_or_default();
                    r.value = choices.first().map(|f| f.name.clone()).unwrap_or_default();
                    rows.push(r.off_if(true, &why));
                }
            }
        }
        if offer.iter().all(|f| f.pack == roster::ORIGINAL_PACK) {
            if let Some(last) = rows.last_mut() {
                last.note = Some(format!("{} More come in machine packs in the mods folder's machines/.", last.note.clone().unwrap_or_default()).trim().to_string());
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
        // P272: online rides have no helicopter to take (P289 carries the cars' to every seat's own).
        if knob.category() == Category::Kiowa && (self.guest || self.lobby.room.is_some()) {
            return Some("Online rides have no Kiowa yet.");
        }
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
        let mut rows = vec![player, self.mods_machine_row()];
        let category = self.mods_category();
        for knob in category.knobs() {
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
        // The wrecks' switches are the rider's on the bike.
        if category == Category::Bike {
            rows.extend(self.wrecks_rows());
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
        let over = match names.get(self.mod_preset.min(names.len().saturating_sub(1))) {
            Some(name) => format!("Save Over {name}"),
            None => "Save Over".into(),
        };
        rows.push(
            Row::action(Key::SaveOverPreset, &over, &format!("Keep {whose} set, with the world's, over the preset Load Preset is on, under its name. Asks first."))
                .off_if(none, "No presets saved yet")
                .off_if(held.is_some(), held.as_deref().unwrap_or("")),
        );
        rows.push(Row::action(Key::DeletePreset, "Delete Preset", "Forgets the preset Load Preset is on.").off_if(none, "No presets saved yet"));
        let reset = match self.mods_seat() {
            None => "Every modifier back to the original game, Wrecks and Air Wrecks on.",
            Some(_) if own.is_some() => "Your own modifiers back to the original game.",
            Some(_) => "This player's modifiers back to the original game; the world's stay.",
        };
        rows.push(Row::action(Key::ResetMods, "Reset to Original", reset).off_if(held.is_some(), held.as_deref().unwrap_or("")));
        rows
    }

    /// The categories the modifiers page's Machine row offers (P272): each machine the game has, then
    /// the world's. A mod machine's goes in here, while its addition is on, once it has knobs.
    pub fn mods_categories(&self) -> Vec<Category> {
        Category::ALL.to_vec()
    }

    /// The category the modifiers page shows: the one picked, or the first if that one is gone.
    pub fn mods_category(&self) -> Category {
        let all = self.mods_categories();
        if all.contains(&self.mods_category) { self.mods_category } else { all[0] }
    }

    /// The Machine row (P272): whose knobs show under it, each choice saying how many of its own
    /// are off the original's.
    fn mods_machine_row(&self) -> Row {
        let all = self.mods_categories();
        let names: Vec<&str> = all.iter().map(|c| c.label()).collect();
        let now = all.iter().position(|c| *c == self.mods_category()).unwrap_or(0);
        let mut r = Row::choice(
            Key::ModsMachine,
            "Machine",
            &names,
            now,
            "Whose modifiers show below: one machine's at a time, or the world's. Every one is kept, shown or not, and a preset holds them all.",
        );
        let m = &self.settings.modifiers;
        r.tips = all
            .iter()
            .map(|c| {
                let mut n = c.knobs().into_iter().filter(|&k| (0..SEATS_MAX).any(|s| m.get(s, k) != game_sim::modifiers::ORIGINAL)).count();
                if *c == Category::Bike {
                    n += self.wrecks_changed();
                }
                if n == 0 { "Original".into() } else { format!("{n} changed") }
            })
            .collect();
        r
    }

    /// The category the page opens on (P272): under the pause, the machine the pausing player is on.
    fn mods_category_for_pause(&self) -> Option<Category> {
        use super::Machine as M;
        (self.stack.first() == Some(&Screen::Pause)).then(|| match self.up[self.pauser.min(SEATS_MAX - 1)] {
            M::Bike => Category::Bike,
            M::Biplane => Category::Biplane,
            M::Buggy | M::Monster | M::Baja => Category::Cars,
            M::Helicopter => Category::Kiowa,
        })
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

    /// Open the Original Game page (P179) from the Ride tab's row, on its master switch.
    pub(super) fn open_original(&mut self) {
        self.push(Screen::Original);
        self.cursor[Screen::Original.index()] = 0;
    }

    /// Open the Game Modifiers page on its Player row, from the Ride tab, the pause or the lobby.
    pub(super) fn open_modifiers(&mut self) {
        if let Some(c) = self.mods_category_for_pause() {
            self.mods_category = c;
        }
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
        rows.push(Row::new(Key::Store, Kind::Opens, "Browse the Store", "Tracks, trick packs and music from the openMX mod store, checked and installed into the mods folder."));
        rows.push(Row::new(Key::Upload, Kind::Opens, "Upload to the Store", "Share mods you made on the openMX mod store, signed in with a Hugging Face token; and take yours down."));
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
        if self.over.as_ref().is_some_and(|o| o.0 == screen) {
            self.answer_over(p);
            return;
        }
        if self.listing.is_some() {
            self.walk_list(screen, p);
            return;
        }
        if p.back || (screen == Screen::Garage && p.start) {
            match screen {
                Screen::Garage => self.close_garage(),
                _ => self.pop(),
            }
            return;
        }
        let i = screen.index();
        // P315f: L2 and R2 step the shown machine's skin on the Machines tab, from any row of it,
        // as the stage's arrows do; L1 and R1 stay the tabs, as on every other page.
        if screen == Screen::Garage && self.on_machines() && (p.l2 || p.r2) {
            self.step_garage_machine(if p.l2 { -1 } else { 1 });
            return;
        }
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
        // P277: Cross opens a many-choice row's list on the choice it is on; left and right still
        // step it, unopened.
        if p.ok && rows[at].listed() {
            self.listing = Some((screen, at, rows[at].now.unwrap_or(0)));
            return;
        }
        self.change(&rows[at], p);
    }

    /// Change a row by a press.
    fn change(&mut self, row: &Row, p: Press) {
        self.change_to(row, p, None);
    }

    /// Change a row by a press, or to choice `pick` of a dropdown's (P277).
    fn change_to(&mut self, row: &Row, p: Press, pick: Option<usize>) {
        let side = p.left || p.right;
        let d = by(p);
        let next = |row: &Row| pick.unwrap_or_else(|| cycle(row.now.unwrap_or(0), d, row.choices.len()));
        // the modifiers page's Player row, and whether it is the host's with All Players (P134)
        let mods_players = (self.screen() == Some(Screen::Modifiers)).then(|| self.mods_own().is_none());
        // P223: a row the presets set, changed by hand (here or by the Original switch), ends Auto.
        let look = super::video::Look::of(&self.settings);
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
            Key::Wrecks => s.wrecks = !s.wrecks,
            Key::AirWrecks => s.air_wrecks = !s.air_wrecks,
            Key::Vibration => s.vibration = !s.vibration,
            Key::BackwardsBail => s.backwards_bail = !s.backwards_bail,
            Key::Bays => s.bays = !s.bays,
            Key::Tricks => s.tricks = Tricks::ALL[next(row)],
            Key::Handling => {
                let seat = self.garage_seat();
                let (choices, first) = (self.handling_choices(), self.first_handling_pack());
                let Some(g) = self.garages.get_mut(seat) else { return };
                roster::step_handling(&choices, &mut g.pick, roster::BIKE, by(p), first.as_deref());
                return;
            }
            Key::BrakeGyro => s.gyro.brake = !s.gyro.brake,
            Key::GasGyro => s.gyro.gas = !s.gyro.gas,
            Key::TrickPack(i) => {
                let ids = self.trick_pack_ids();
                let Some(id) = ids.get(i).cloned() else { return };
                self.settings.toggle_trick_pack(&id, &ids);
            }
            Key::Modifiers if p.ok => {
                self.open_modifiers();
                return;
            }
            Key::OriginalPage if p.ok => {
                self.open_original();
                return;
            }
            Key::Store if p.ok => {
                self.open_store();
                return;
            }
            Key::Upload if p.ok => {
                self.open_store_upload();
                return;
            }
            Key::Hud => s.overlay = Overlay::CHOICES[next(row)],
            Key::Speedometer => s.speedometer = Speedometer::ALL[next(row)],
            Key::Minimap => s.minimap = !s.minimap,
            // Cross steps it up and round from the top, as a slider does.
            Key::HudScale => s.hud_scale = hud_scale_step(s.hud_scale, p.left, p.right),
            Key::DrawDistance => s.graphics = fog_step(s.graphics, p.left, p.right),
            Key::WorldSky => s.world_sky = WorldSky::ALL[next(row)],
            Key::TextureFiltering => s.filtering = Filtering::ALL[next(row)],
            Key::Shadows => s.shadows = Shadows::ALL[next(row)],
            Key::ShadowQuality => s.shadow_quality = ShadowQuality::ALL[next(row)],
            Key::Updates => s.updates = Updates::ALL[next(row)],
            Key::AspectRatio => s.picture = Picture::ALL[next(row)],
            Key::ButtonIcons => s.button_icons = ButtonIcons::ALL[next(row)],
            Key::RenderScale => s.render_scale = RenderScale::ALL[next(row)],
            Key::VSync => s.vsync = !s.vsync,
            Key::ShowFps => s.show_fps = !s.show_fps,
            Key::FrameLimit => s.frame_limit = pick.and_then(|j| FrameLimit::ALL.get(j).copied()).unwrap_or_else(|| s.frame_limit.step(p.left)),
            Key::VideoPreset if pick.is_some() => {
                let j = pick.unwrap_or(0);
                if j + 1 >= row.choices.len() {
                    return;
                }
                super::video::set_row(s, j);
            }
            Key::VideoPreset => {
                // Custom is shown, never stepped onto: past it is Auto again.
                let (n, from) = (row.choices.len() - 1, row.now.unwrap_or(0));
                let to = match from >= n {
                    true if d < 0 => n - 1,
                    true => 0,
                    false => cycle(from, d, n),
                };
                super::video::set_row(s, to);
            }

            Key::Detail => s.detail = super::video::Detail::ALL[next(row)],
            Key::Mix => s.set_mix(Mix::ALL[next(row)]),
            // Cross steps a slider up and round from the top, as the original's page does.
            Key::Level(i) => s.nudge(i, p.left, p.right),
            Key::MusicPlays if pick.is_some() => {
                if !self.play_choice(pick.unwrap_or(0)) {
                    return;
                }
            }
            Key::MusicPlays => self.music_plays(d),
            Key::Song if pick.is_some() => self.pick_song(pick.unwrap_or(0)),
            Key::Song if side => self.step_song(d),
            Key::SkipSong if p.ok => {
                self.music_held = false;
                self.skip_song();
            }
            Key::BackSong if p.ok => {
                self.work_deck(Deck::Back);
            }
            Key::HoldMusic if p.ok => {
                self.work_deck(Deck::Hold);
            }
            Key::BikeLayout => {
                let i = self.control_seat();
                let (layout, preset) = control_choices()[next(row)];
                self.controls.layouts[i] = layout;
                self.controls.presets[i] = preset;
            }
            Key::Keyboard => self.controls.home_row_keys = !self.controls.home_row_keys,
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
                let seat = self.camera_seat();
                self.settings.cameras[seat] = to;
                // Every preset's: the disc presets' own numbers go too (P163).
                self.settings.camera_sets[seat].clear_disc_looks();
            }
            Key::CameraPreset => {
                match pick {
                    Some(j) => self.camera_pick = j,
                    None => self.step_camera_pick(d),
                }
                return;
            }
            Key::CameraOwn(i) => {
                let Some(pick) = self.camera_picked() else { return };
                self.step_camera_look(&pick, i, p.left, p.right);
            }
            Key::CameraFollow if p.ok => {
                let Some(pick) = self.camera_picked() else { return };
                let seat = self.camera_seat();
                self.settings.camera_sets[seat].remove(pick.machine, &pick.name);
            }
            // P246: asked first, on Cancel.
            Key::CameraDelete if p.ok => {
                let (Some(page), Some(pick)) = (self.screen(), self.camera_picked()) else { return };
                self.over = Some((page, super::Over::DeleteCamera(pick), 1));
                return;
            }
            Key::CameraSave if p.ok => {
                self.type_for(Typing::Camera);
                return;
            }
            Key::CameraSaveOver => {
                let overs = self.camera_over_choices();
                let n = overs.len();
                if side {
                    self.camera_over = cycle(self.camera_over, d, n);
                    return;
                }
                let (Some(page), Some(over)) = (self.screen(), overs.get(self.camera_over.min(n.saturating_sub(1)))) else { return };
                self.over = Some((page, super::Over::Camera(over.clone()), 1));
                return;
            }
            Key::CameraCycle => {
                let seat = self.camera_seat();
                self.settings.camera_sets[seat].cycle = cameras::Cycle::ALL[next(row)];
            }
            Key::Garage(g) => {
                self.change_garage(g, p);
                return;
            }
            Key::Machine(_) => return,
            Key::Flier(i) => {
                let seat = self.garage_seat();
                let offer = self.flier_offer.clone();
                let Some(kind) = roster::flier_kinds(&offer).get(i).cloned() else { return };
                let Some(g) = self.garages.get_mut(seat) else { return };
                roster::step_flier(&offer, &mut g.pick, &kind, by(p));
                return;
            }
            Key::ModsMachine => {
                let all = self.mods_categories();
                self.mods_category = all[next(row).min(all.len() - 1)];
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
            Key::SaveOverPreset if p.ok => {
                let (Some(page), n) = (self.screen(), self.settings.presets.len()) else { return };
                self.over = Some((page, super::Over::Mods(self.mod_preset.min(n - 1)), 1));
                return;
            }
            // P246: asked first, on Cancel.
            Key::DeletePreset if p.ok => {
                let (Some(page), n) = (self.screen(), self.settings.presets.len()) else { return };
                self.over = Some((page, super::Over::DeleteMods(self.mod_preset.min(n - 1)), 1));
                return;
            }
            Key::ResetMods if p.ok => {
                self.load_mods(&Mods::original());
                // The host's All Players reset is the whole ride's: the world's wrecks back on too.
                if self.mods_seat().is_none() {
                    self.settings.wrecks = true;
                    if self.air_held().is_none() {
                        self.settings.air_wrecks = true;
                        self.settings.backwards_bail = false;
                    }
                }
            }
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
            Key::MusicFrom => {
                // From All or a mix, every pack off; from MX Unleashed Only, every pack on.
                let ids: Vec<String> = self.music_packs().iter().map(|p| p.0.clone()).collect();
                let all_off = ids.iter().all(|id| self.settings.mods_off.contains(id));
                self.settings.mods_off.retain(|o| !ids.contains(o));
                if !all_off {
                    self.settings.mods_off.extend(ids);
                }
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
        if row.key != Key::VideoPreset && super::video::Look::of(&self.settings) != look {
            self.settings.video_auto = false;
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
        self.play_choice(at);
    }

    /// Music Plays set to its choice `at` (P277's pick, and a step's): whether it could be, One Song
    /// needing a song to repeat.
    fn play_choice(&mut self, at: usize) -> bool {
        let one = self.music_now.clone().or_else(|| self.song_at().map(str::to_string));
        if at == PLAYS_ONE && one.is_none() {
            return false;
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
        true
    }

    /// The Song row's song `i` picked from its list (P277): the one repeated, and it plays.
    fn pick_song(&mut self, i: usize) {
        let Some((id, _)) = self.music_choices.get(i) else { return };
        let id = id.clone();
        self.music_at = i;
        self.settings.music = id.clone();
        self.tunes.push(Tune::Play(id));
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
    pub(super) fn skip_song(&mut self) {
        if self.plays() == PLAYS_ONE {
            self.step_song(1);
        } else {
            self.tunes.push(Tune::Skip);
        }
    }

    /// Back a song (P174): the one played before, which under One Song is the one before in the
    /// list, repeated. The music plays on if it was held.
    pub(super) fn back_song(&mut self) {
        self.music_held = false;
        if self.plays() == PLAYS_ONE {
            self.step_song(-1);
        } else {
            self.tunes.push(Tune::Back);
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
    /// the Original Game page's master switch, which the Ride tab opens (P179).
    pub fn tab(self) -> usize {
        match self {
            Departure::Fog | Departure::Foliage | Departure::Textures | Departure::Filtering | Departure::Shadows => TAB_GRAPHICS,
            // P176: Aspect Ratio, on Display.
            Departure::Picture => TAB_DISPLAY,
            Departure::Controls | Departure::Cameras | Departure::Markers => TAB_RIDE,
        }
    }

    /// The key of its switch's row: the fog's is Draw Distance (V32), the same setting.
    pub fn key(self) -> Key {
        match self {
            Departure::Fog => Key::DrawDistance,
            Departure::Filtering => Key::TextureFiltering,
            Departure::Shadows => Key::ShadowQuality,
            Departure::Picture => Key::AspectRatio,
            d => Key::Departure(d),
        }
    }
}

/// The Fog slider's names for its steps (P276): how many times the original's.
pub fn graphics_label(g: Graphics) -> &'static str {
    match g {
        Graphics::Full => "Whole Map",
        Graphics::Original => "Original",
        Graphics::Far => "4x",
        Graphics::Farther => "8x",
        Graphics::Times(2) => "2x",
        Graphics::Times(3) => "3x",
        Graphics::Times(6) => "6x",
        Graphics::Times(12) => "12x",
        Graphics::Times(16) => "16x",
        Graphics::Times(_) => "Custom",
    }
}

/// The Fog slider a step nearer or further (P276) — or, with neither, a step further and round from
/// the whole map to the original's, as Cross steps a slider. A value off the slider starts from
/// its nearest step at or below it.
pub fn fog_step(g: Graphics, nearer: bool, further: bool) -> Graphics {
    let steps = Graphics::SLIDER;
    let k = g.multiple().unwrap_or(f32::INFINITY);
    let at = steps.iter().rposition(|s| s.multiple().unwrap_or(f32::INFINITY) <= k).unwrap_or(0);
    let last = steps.len() - 1;
    steps[match (nearer, further) {
        (true, _) => at.saturating_sub(1),
        (_, true) => (at + 1).min(last),
        _ if at >= last => 0,
        _ => at + 1,
    }]
}

/// `n` with a comma every three digits.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}
