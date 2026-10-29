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
//! the track pick, and the ride starts with one view a seat in play ([`Menu::party`]). One seat is
//! enough (P171): alone there is no layout to pick, the track pick is next, and the others drop in on
//! the ride with Cross as it runs. Online (above) sits beside it on the main menu.

use crate::join::{self, Join, Opened, Outcome};
use crate::roster::{self, Catalogue, Garage, Pick, Picks};
use crate::split::{self, Split};
use game_render::eco::{Foliage, Lod};
use game_render::shadow::Shadows;
use game_render::world::Graphics;
use game_render::gpu::{Shape, Sprite};
use game_render::{Disc, Frame, Texture};
use game_sim::input::{AXIS_CENTRE, Control, Report};
use game_sim::modifiers::online as online_mods;
use game_sim::modifiers::{Knob, Mods};
use openmx_net::{Cmd, Event, HOST, Listing, Room};
use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

#[path = "menu_art.rs"]
pub mod ink;
#[path = "menu_pages.rs"]
mod pages;
#[path = "menu_cameras.rs"]
pub mod cameras;
#[path = "menu_screens.rs"]
mod screens;
#[path = "menu_player.rs"]
mod player;
#[allow(unused_imports)]
pub use player::{Deck, deck_point};
#[allow(unused_imports)] // the tests' and the shell's, not every includer's
pub use pages::{GARAGE_TABS, Key, Kind, PLAYERS, Pack, Row, SETTINGS_TABS, graphics_label, pack_name, pack_of};

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
    /// L1 and R1: the tabs of Settings and the Garage, and the Free Ride strip's player (P118).
    pub l1: bool,
    pub r1: bool,
    /// Triangle alone, which is also [`Press::back`]: the Free Ride strip's Wrecks (P118).
    pub triangle: bool,
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
            l1: self.l1 || o.l1,
            r1: self.r1 || o.r1,
            triangle: self.triangle || o.triangle,
        }
    }
}

/// How far the left stick has to lean before it counts as a direction, out of 127.
const STICK_DIRECTION: i32 = 64;
/// How far back towards the centre a leaning stick has to come before it lets the direction go
/// (P109): a stick resting on the line would otherwise press and let go every few frames.
const STICK_LET_GO: i32 = 40;
/// How long a direction is held before it repeats, and then the time between repeats, in
/// microseconds: the front end's twenty frames and five at 60 Hz, which the pause, read at the
/// ride's 30 Hz ticks, keeps too (P109).
const REPEAT_AFTER_US: u64 = 20 * 16_666;
const REPEAT_EVERY_US: u64 = 5 * 16_666;
/// A control let go for less than this and down again is still the press it was (P109): a d-pad's
/// contact bouncing, or one push reported up and down again across a frame. A pad's presses that
/// came further apart than this inside a frame reach the menu as its taps ([`Reader::read_taps`]),
/// which `pad.rs` debounces the same by the events' own times.
const BOUNCE_US: u64 = 35_000;
/// The rate a [`Reader`] is read at unless it says otherwise: the front end's frames.
const MENU_HZ: u64 = 60;
/// The d-pad's four, which a hold repeats.
const DIRS: u16 = Control::Up.mask() | Control::Down.mask() | Control::Left.mask() | Control::Right.mask();

/// A device's menu controls at one moment between two reads (P165): when, in microseconds on the
/// shell's clock (`window.rs`'s `shell_us`), and its report then — the buttons, the d-pad and the
/// left stick as they stood after one of its events. The pad layer keeps them by the events' own
/// times, so a press, a bounce and a hold are timed by when the hand did them and not by the frame
/// that happened to see it: the same at 5 fps as at 165.
pub type Moment = (u64, Report);

/// One device for [`Reader::read_timed`]: its report now, its presses since the last read
/// ([`Reader::read_taps`]), and its moments since then — `None` for a device that keeps none, whose
/// changes are taken at the read.
pub type Timed<'a> = (Report, u16, Option<&'a [Moment]>);

/// How many repeats a direction held `us` has made, the push itself not counted.
fn repeats(us: u64) -> u64 {
    if us < REPEAT_AFTER_US { 0 } else { 1 + (us - REPEAT_AFTER_US) / REPEAT_EVERY_US }
}

/// When a hold's `k`th repeat (from 1) is due, from the push.
fn repeat_due(k: u64) -> u64 {
    REPEAT_AFTER_US + (k - 1) * REPEAT_EVERY_US
}

/// Turns a report a frame into [`Press`]es.
///
/// The first report it sees is taken as already held, so the Cross that chose a track, or the Start
/// that paused, does not press again on the screen it opened.
///
/// Everything it counts is counted in time, never in reads (P165): a push is a press however many
/// frames it spans, a hold repeats by how long it was held, and a read that a slow frame makes long
/// hands out every press that fell inside it — the first as its own [`Press`], the rest through
/// [`Reader::more`] — so the menu steps as often at 5 fps as at 165.
#[derive(Clone, Debug)]
pub struct Reader {
    last: Option<u16>,
    /// Each device's controls after the last read, in the order the read gave them (P165): where
    /// its moments start from.
    devices: Vec<u16>,
    /// The directions held now, together: since when, and how many repeats that hold has made.
    hold: Option<(u64, u64)>,
    /// When each control ([`Control`]'s bit order) last went up, on the reader's clock.
    up_at: [Option<u64>; 16],
    /// The reader's clock, in microseconds: the last read's moment.
    clock: u64,
    /// The time an untimed read ([`Reader::read`]) stands for: the rate it is read at.
    frame_us: u64,
    /// The presses the last read made past its first, oldest first ([`Reader::more`]).
    owed: std::collections::VecDeque<Press>,
}

impl Default for Reader {
    fn default() -> Reader {
        Reader::at(MENU_HZ as u32)
    }
}

/// A direction from the stick, as the d-pad bit it stands in for. A direction `held` lets go only
/// once the stick is back inside [`STICK_LET_GO`].
fn stick(r: &Report, held: u16) -> u16 {
    let lean = |i: usize| r.axis(i) as i32 - AXIS_CENTRE as i32;
    let (x, y) = (lean(2), lean(3));
    let past = |c: Control, by: i32| {
        let edge = if held & c.mask() != 0 { STICK_LET_GO } else { STICK_DIRECTION };
        if by >= edge { c.mask() } else { 0 }
    };
    past(Control::Left, -x) | past(Control::Right, x) | past(Control::Up, -y) | past(Control::Down, y)
}

/// What a report holds of the menu's controls, its stick's directions among them. A pad that is
/// not there holds nothing.
fn holds(r: &Report, held: u16) -> u16 {
    if r.status() != 0xff { r.buttons() | stick(r, held) } else { 0 }
}

/// The press of the controls in `edge`.
fn press_of(edge: u16) -> Press {
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
        l1: on(Control::L1),
        r1: on(Control::R1),
        triangle: on(Control::Triangle),
    }
}

impl Reader {
    pub fn new() -> Reader {
        Reader::default()
    }

    /// A reader read `hz` times a second by [`Reader::read`]: the pause's, at the ride's ticks,
    /// holds a direction as long in time before it repeats as the front end's does.
    pub fn at(hz: u32) -> Reader {
        Reader {
            last: None,
            devices: Vec::new(),
            hold: None,
            up_at: [None; 16],
            clock: 0,
            frame_us: 1_000_000 / u64::from(hz.max(1)),
            owed: std::collections::VecDeque::new(),
        }
    }

    /// Forget what was held, so the next report is the first again.
    pub fn reset(&mut self) {
        *self = Reader { frame_us: self.frame_us, clock: self.clock, ..Reader::default() };
    }

    pub fn read(&mut self, r: &Report) -> Press {
        self.read_taps(r, 0)
    }

    /// [`Reader::read`], with the controls the pad layer saw go down since the last report (P53:
    /// `pad.rs`'s `Pads::taps`). Each of those is a press whatever the two reports say: a button
    /// let go and pressed again between two frames is down in both, and a slow frame — 5 fps at 4K
    /// — is longer than a tap. Not on the first report: that one is what was already held.
    pub fn read_taps(&mut self, r: &Report, taps: u16) -> Press {
        self.read_joint(&[(*r, taps)])
    }

    /// Several devices' reports as one driver's (P109): what any of them holds is held, so the
    /// same push seen by two — a pad and the keyboard a desktop maps its d-pad to — one frame
    /// apart is one press, not one each. Read [`Reader::at`]'s rate apart.
    pub fn read_joint(&mut self, reports: &[(Report, u16)]) -> Press {
        let devices: Vec<Timed> = reports.iter().map(|&(r, taps)| (r, taps, None)).collect();
        let now = self.clock + self.frame_us;
        self.read_timed(&devices, now)
    }

    /// [`Reader::read_joint`] at `now`, microseconds on the shell's clock, with each device's
    /// moments since the last read (P165): its changes are taken when they happened, a device
    /// without them changing at `now`. The first press it made comes back; any more — a slow
    /// frame's double tap, or a hold's repeats that fell inside it — wait in [`Reader::more`].
    pub fn read_timed(&mut self, devices: &[Timed], now: u64) -> Press {
        self.owed.clear();
        let since = self.clock;
        let now = now.max(since);
        self.clock = now;
        self.devices.resize(devices.len(), 0);
        let Some(mut held) = self.last else {
            for (i, (r, _, _)) in devices.iter().enumerate() {
                self.devices[i] = holds(r, 0);
            }
            let held = self.devices.iter().fold(0, |a, b| a | b);
            self.last = Some(held);
            self.hold = (held & DIRS != 0).then_some((now, 0));
            return Press::default();
        };
        // Each press, by when it was made.
        let mut presses: Vec<(u64, u16)> = Vec::new();
        let mut moments: Vec<(u64, usize, Report)> = Vec::new();
        for (i, (_, _, m)) in devices.iter().enumerate() {
            moments.extend(m.unwrap_or_default().iter().map(|&(t, r)| (t.clamp(since, now), i, r)));
        }
        moments.sort_by_key(|&(t, _, _)| t);
        let mut seen = vec![0u16; devices.len()];
        for (t, i, r) in moments {
            self.devices[i] = holds(&r, held);
            seen[i] |= self.devices[i];
            let after = self.devices.iter().fold(0, |a, b| a | b);
            self.change(t, held, after, &mut presses);
            held = after;
        }
        // Then each device as its report has it now. A timed device's report holds its taps down
        // ([`Reader::read_taps`]), and a key the keyboard tapped inside the frame, which its moments
        // already pressed and let go: those are not held. An untimed device's taps are presses.
        let mut taps = 0;
        for (i, (r, t, m)) in devices.iter().enumerate() {
            let mut now_holds = holds(r, held);
            match m {
                Some(_) => now_holds &= !((seen[i] | t) & !self.devices[i]),
                None => taps |= t,
            }
            self.devices[i] = now_holds;
        }
        let after = self.devices.iter().fold(0, |a, b| a | b);
        let before = presses.len();
        self.change(now, held, after, &mut presses);
        let fresh = presses[before..].iter().filter(|p| p.0 == now).fold(0, |a, p| a | p.1);
        if taps & !fresh != 0 {
            presses.push((now, taps & !fresh));
        }
        held = after;
        self.last = Some(held);
        // And the repeats the hold has made by now.
        if let Some((start, made)) = &mut self.hold {
            let due = repeats(now - *start);
            presses.extend((*made + 1..=due).map(|k| (*start + repeat_due(k), held & DIRS)));
            *made = (*made).max(due);
        }
        presses.sort_by_key(|p| p.0);
        // One press a step: a control pressed twice waits for the next.
        let mut steps: Vec<u16> = Vec::new();
        for (_, m) in presses {
            match steps.last_mut() {
                Some(s) if *s & m == 0 => *s |= m,
                _ => steps.push(m),
            }
        }
        self.owed = steps.iter().skip(1).map(|&s| press_of(s)).collect();
        steps.first().map_or_else(Press::default, |&s| press_of(s))
    }

    /// The next of the last read's presses past its first (P165), until there are none: the menu
    /// steps once for each, in the same frame, so a slow frame moves it as far as the time it took.
    pub fn more(&mut self) -> Option<Press> {
        self.owed.pop_front()
    }

    /// The controls going from `before` to `after` at `t`: the ones down again after a bounce's
    /// time up are presses, and a change of the directions held ends the hold, with the repeats it
    /// made before it ended, and starts the next.
    fn change(&mut self, t: u64, before: u16, after: u16, presses: &mut Vec<(u64, u16)>) {
        if before == after {
            return;
        }
        let mut fresh = 0;
        for (bit, up) in self.up_at.iter_mut().enumerate() {
            let m = 1u16 << bit;
            if before & m != 0 && after & m == 0 {
                *up = Some(t);
            } else if before & m == 0 && after & m != 0 && up.is_none_or(|u| t.saturating_sub(u) >= BOUNCE_US) {
                fresh |= m;
            }
        }
        if fresh != 0 {
            presses.push((t, fresh));
        }
        if before & DIRS != after & DIRS {
            if let Some((start, made)) = self.hold.take() {
                // Only the repeats due while it was still down.
                let due = repeats(t.saturating_sub(start).saturating_sub(1));
                presses.extend((made + 1..=due).map(|k| (start + repeat_due(k), before & DIRS)));
            }
            if after & DIRS != 0 {
                self.hold = Some((t, 0));
            }
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

/// Every free-ride map spliced into one (P71, `crate::spliced`), `(id, name)`: the pick's row after
/// the five maps, ridden as `--track free_all` is.
pub const SPLICED: (&str, &str) = ("free_all", "Every free ride");

/// The pick's rows the disc gives: its five towns, then [`SPLICED`]. The mods' come after.
pub const DISC_PICK: usize = TRACKS.len() + 1;

/// `settings.txt`: the rider and the ride.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// The rider's name, typed on the on-screen keyboard.
    pub name: String,
    /// Which overlay a ride draws (V26), `--overlay`'s word: the ride HUD or none (P170).
    pub overlay: Overlay,
    /// How the HUD shows the speed (V42): the digital readout, or the dial.
    pub speedometer: Speedometer,
    /// Whether the HUD draws its free-ride radar (V52): off, the speedometer stays and
    /// the radar, its icons and the other riders' marks go from every view. On unless turned off.
    pub minimap: bool,
    /// The whole HUD's size, in percent of the original's (V52): [`HUD_SCALE_MIN`] to
    /// [`HUD_SCALE_MAX`] in steps of [`HUD_SCALE_STEP`], each part about its own corner.
    pub hud_scale: u8,
    /// A combined world's sky and light (MD9): one for the whole world, or each map's own.
    pub world_sky: WorldSky,
    /// Whether a ride wrecks at all.
    pub wrecks: bool,
    /// Whether the bike's attitude in the air throws the rider off (P138): upside down, nosed over or
    /// rolled past the bike's airborne limits. Off, the jump is ridden out and only the landing is
    /// judged ([`game_sim::state::Sense::air_wrecks`]). On, the original's, unless turned off.
    pub air_wrecks: bool,
    /// Whether a landing with the bike facing back against its way throws the rider off (S161,
    /// [`game_sim::state::Sense::backwards_bail`]). Not the original's: off, the default, is the
    /// original, which rides such a landing out.
    pub backwards_bail: bool,
    /// Whether a wreck rumbles the rider's pad (P183): the original's controller vibration option.
    /// On by default, the original's.
    pub vibration: bool,
    /// The last track ridden, which the track pick opens on.
    pub track: String,
    /// How a local game of two shares the screen, the split layout's pick (P43): `--split`'s.
    pub split: Split,
    /// The music (P42): `off`, `shuffle`, `order` (P56), or one song's id, repeated
    /// (`disc:headstrong`, `folder:mine.wav`).
    pub music: String,
    /// The volume sliders, `0..=VOLUME_MAX` in steps of [`VOLUME_STEP`]: the original's Game Audio
    /// Options page's six (P56) — Music, User Soundtracks (the user's own folder), Sound FX, Crowd,
    /// Engine and Menu FX.
    pub music_volume: u8,
    pub effects_volume: u8,
    pub folder_volume: u8,
    pub crowd_volume: u8,
    pub engine_volume: u8,
    pub menu_volume: u8,
    /// The page's Audio Presets row: which mix the sliders were last set from.
    pub mix: Mix,
    /// The Custom Mix's sliders, in [`Mix::levels`]' order: what they were when one was last moved.
    pub custom: [u8; 6],
    /// What the GPU draws of the track (V32): the whole map, or the original's fog and far clip.
    pub graphics: Graphics,
    /// The fraction of the frame the GPU draws the 3D at (V34): `--render-scale`'s, as a choice of four.
    pub render_scale: RenderScale,
    /// Whether the picture waits for the display (P90): presented once a refresh, never torn. On
    /// unless turned off.
    pub vsync: bool,
    /// The most pictures a second (P90): with VSync off, or below the refresh with it on.
    pub frame_limit: FrameLimit,
    /// Whether the lot's machines stand at their bays (P103): drawn see-through there, and offered by
    /// the original's pop-up when the rider stops beside one. Off, neither is; the pause's switch
    /// stays. On unless turned off.
    pub bays: bool,
    /// The game modifiers the next ride loads with (P55): the original game's unless changed.
    pub modifiers: Mods,
    /// Modifier presets saved by name, in the order they were saved. Each is one player's set
    /// (P143), [`preset_of`]'s: that player's own modifiers alike on every row, and the world's.
    pub presets: Vec<(String, Mods)>,
    /// The seats a lobby or an opened ride this machine hosts has (N12), the lobby's Seats row's:
    /// 2 to [`openmx_net::SEATS`], everyone's together.
    pub lobby_seats: usize,
    /// Each seat's camera (P82), the Camera page's: the field of view, and how far behind and how
    /// high above the bike and the biplane the follow cameras sit. 1P's first.
    pub cameras: [CameraView; SEATS_MAX],
    /// Each seat's camera presets (P163): the preset each machine's camera was last left on, the
    /// disc presets' own numbers and the player's own presets, and what the camera button cycles.
    pub camera_sets: [cameras::Seat; SEATS_MAX],
    /// Whether the shell reads the `mods/` folder beside this file (MD2): Off, nothing is read from it
    /// and the pick is the disc's alone. On unless turned off.
    pub mods: bool,
    /// The mod packs switched off on the Mods page (P118), by the prefix their tracks' ids share
    /// (`mxvatv` for `mxvatv_nat13`): their tracks are left off the Free Ride list. A music pack
    /// (P122) is by its whole id (`untamed_music`), which has the underscore no prefix has.
    pub mods_off: Vec<String>,
    /// The track pick's filter (MD11), the source game whose tracks it lists: a [`Menu::track_groups`]
    /// name, empty for All Games. A game the pick does not list now shows All and is kept.
    pub track_filter: String,
    /// Whose tricks a ride has (MD16): the original's, or MX vs. ATV's read from a trick pack in the
    /// mods folder. The original's unless changed.
    pub tricks: Tricks,
    /// How the plants change from model to picture (V55): ours, smoothed, unless the Original
    /// switch's Plants (P126) is on.
    pub plants: Lod,
    /// How the plants' leaves lie over what is behind them (V56): ours, cut out, unless the
    /// Original switch's Leaves (V57) is on.
    pub foliage: Foliage,
    /// The ride's picture's shape (P126): the window's own, or the original's 4:3 with bars.
    pub picture: Picture,
    /// Whether the world's textures are taken from texture packs, `mods/texture-replacements/` and
    /// mods' own `textures/upscaled/` (V84) over the disc's. On unless turned off; Original is the
    /// disc's alone.
    pub textures: bool,
    /// The texture packs' order, top (wins) first, and the ones switched off (V84), by their folder
    /// under `mods/`: opentennis's `texture_packs` / `texture_packs_off`. A pack in neither is on, on
    /// top.
    pub texture_packs: Vec<String>,
    pub texture_packs_off: Vec<String>,
    /// How far the GPU filters every texture anisotropically (V83): 16x unless turned down, or Off
    /// under the Original switch's Texture Filtering.
    pub filtering: Filtering,
    /// The sun's shadows of what moves and stands (V108): ours, the original's under each racer, or
    /// none. Ours unless turned to another; Original under the Original switch.
    pub shadows: Shadows,
    /// Each seat's session markers (P129), the Controls tab's Markers row, 1P's first: on, the d-pad
    /// is the markers' and the left stick alone rides; off, the d-pad fires no marker, the marker
    /// bar hides, and the d-pad rides as the original's — steering, leaning and the trick gestures.
    /// On unless turned off.
    pub markers: [bool; SEATS_MAX],
}

/// The shape a ride's picture is drawn in (P126), as `settings.txt` spells it: the shell's
/// `aspect::Aspect`, which this file cannot name because `tests/menu.rs` compiles it on its own.
/// The Display tab's Aspect Ratio row (P176), opentennis's six.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Picture {
    /// The window's own shape, whatever it is: Auto.
    #[default]
    Fill,
    /// The original's television, the largest 4:3 frame that fits, with bars round it.
    Television,
    /// 16:9, with bars.
    Wide,
    /// 16:10, with bars.
    Monitor,
    /// 21:9, with bars.
    Cinema,
    /// The whole window, as Auto: opentennis's Stretch drops the safe area, which the shell has none of.
    Stretch,
}

impl Picture {
    pub const ALL: [Picture; 6] = [Picture::Fill, Picture::Television, Picture::Wide, Picture::Monitor, Picture::Cinema, Picture::Stretch];

    /// The file's word, which is also `--aspect`'s.
    pub fn word(self) -> &'static str {
        match self {
            Picture::Fill => "fill",
            Picture::Television => "4:3",
            Picture::Wide => "16:9",
            Picture::Monitor => "16:10",
            Picture::Cinema => "21:9",
            Picture::Stretch => "stretch",
        }
    }

    /// The Aspect Ratio row's name for it.
    pub fn label(self) -> &'static str {
        match self {
            Picture::Fill => "Auto",
            Picture::Stretch => "Stretch",
            p => p.word(),
        }
    }

    pub fn from_word(w: &str) -> Option<Picture> {
        Picture::ALL.into_iter().find(|p| p.word() == w)
    }
}

/// Aspect Ratio's row's help, on the Display tab and in the Original switch's reach (P176).
pub const ASPECT_HELP: &str = "The shape of the picture, the HUD and the menus on screen, split screens too. Auto fills the window; 4:3, 16:9, 16:10 and 21:9 keep that shape with bars round it. The original is the television's 4:3. Stretch fills the window as Auto does.";

/// Texture Filtering's row's help, on the Graphics tab and in the Original switch's reach (V83).
pub const FILTERING_HELP: &str = "How sharp textures stay when seen at a glance, like the ground down a straight. 16x keeps them sharp into the distance; Off blurs them as the original did. The software renderer draws without it either way.";

/// Shadows' row's help, on the Graphics tab and in the Original switch's reach (V108).
pub const SHADOWS_HELP: &str = "Shadows of the bikes, riders, machines and the world's models in the sun. Better casts everything near onto the ground and onto each other, softly, as dark as the ground's own shadows; Original is the game's soft patch under each rider. The software renderer draws none.";

/// The Texture Filtering setting (V83): how far the GPU filters textures anisotropically, so a
/// texture seen edge-on (the ground down a straight) stays sharp along it. Off is the original's
/// look: one plain trilinear mip, chosen by the steep direction's footprint, which blurs the ground.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Filtering {
    Off,
    X2,
    X4,
    X8,
    #[default]
    X16,
}

impl Filtering {
    /// Ours first and the original's last, as Draw Distance's: a step on from Off is 16x again.
    pub const ALL: [Filtering; 5] = [Filtering::X16, Filtering::X8, Filtering::X4, Filtering::X2, Filtering::Off];

    /// The sampler's anisotropy clamp: 1 for Off.
    pub fn anisotropy(self) -> u16 {
        match self {
            Filtering::Off => 1,
            Filtering::X2 => 2,
            Filtering::X4 => 4,
            Filtering::X8 => 8,
            Filtering::X16 => 16,
        }
    }

    /// Its row's name for it.
    pub fn label(self) -> &'static str {
        match self {
            Filtering::Off => "Off",
            Filtering::X2 => "2x",
            Filtering::X4 => "4x",
            Filtering::X8 => "8x",
            Filtering::X16 => "16x",
        }
    }

    /// The file's word, which is also `--texture-filtering`'s: `off`, `2` to `16`.
    pub fn word(self) -> &'static str {
        match self {
            Filtering::Off => "off",
            Filtering::X2 => "2",
            Filtering::X4 => "4",
            Filtering::X8 => "8",
            Filtering::X16 => "16",
        }
    }

    pub fn from_word(w: &str) -> Option<Filtering> {
        let w = w.trim_end_matches('x');
        Filtering::ALL.into_iter().find(|f| f.word() == w || (w == "1" && *f == Filtering::Off))
    }
}

/// What the Original switch reads (P126): every departure below it at the original's and the HUD the
/// original's, every one of them ours, or some of each. Mixed is its own reading, not Off: Off means
/// every enhancement is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Original {
    Off,
    On,
    Mixed,
}

impl Original {
    pub fn label(self) -> &'static str {
        match self {
            Original::Off => "Off",
            Original::On => "On",
            Original::Mixed => "Mixed",
        }
    }
}

/// Each place the port departs from the original's look or feel on purpose (P126): a switch of its
/// own under the Original one, ours by default. The simulation is none of them: the game modifiers
/// (P55) stay as they are whatever these say. A new enhancement adds itself here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Departure {
    /// Every seat's bike layout and plane stick (P40, P50, P81): New, or the disc's (P73, P69).
    Controls,
    /// Every seat's camera (P82): the port's further-out view, or the disc's numbers.
    Cameras,
    /// The fog and far clip (V32): the whole map, or the original's.
    Fog,
    /// The plants' model-to-picture change (V55): the smooth one, or the original's.
    Plants,
    /// The plants' leaves (V56): cut out, so a tree shows through another's gaps, or the original's
    /// blended ones.
    Foliage,
    /// The ride's picture (V24): the window's shape, or the original's 4:3.
    Picture,
    /// The textures' anisotropic filtering (V83): 16x, or the original's plain trilinear blur, Off.
    Filtering,
    /// Every seat's session markers (P129): on, the d-pad the markers'; or off, the original's d-pad.
    Markers,
    /// The world's textures (V84): texture packs' and upscales over the disc's, or the disc's alone.
    Textures,
    /// The sun's shadows (V108): ours of everything near, or the original's under each racer.
    Shadows,
}

impl Departure {
    pub const ALL: [Departure; 10] = [
        Departure::Controls,
        Departure::Cameras,
        Departure::Fog,
        Departure::Plants,
        Departure::Foliage,
        Departure::Picture,
        Departure::Filtering,
        Departure::Markers,
        Departure::Textures,
        Departure::Shadows,
    ];

    /// Its row's name.
    pub fn label(self) -> &'static str {
        match self {
            Departure::Controls => "Controls",
            Departure::Cameras => "Cameras",
            Departure::Fog => "Fog and Draw Distance",
            Departure::Plants => "Plant Detail",
            Departure::Foliage => "Leaves",
            Departure::Picture => "Aspect Ratio",
            Departure::Filtering => "Texture Filtering",
            Departure::Markers => "Markers",
            Departure::Textures => "Upscaled Textures",
            Departure::Shadows => "Shadows",
        }
    }

    /// Its row's two choices, ours first.
    pub fn choices(self) -> [&'static str; 2] {
        match self {
            Departure::Controls => ["New", "Original"],
            Departure::Cameras => ["Further Out", "Original"],
            Departure::Fog => ["Whole Map", "Original Fog"],
            Departure::Plants => ["Smooth", "Original"],
            Departure::Foliage => ["Crisp", "Original"],
            Departure::Picture => ["Auto", "4:3"],
            Departure::Filtering => ["16x", "Off"],
            Departure::Markers => ["On", "Off"],
            Departure::Textures => ["Upscaled", "Original"],
            Departure::Shadows => ["Better", "Original"],
        }
    }

    /// What its row says it does.
    pub fn about(self) -> &'static str {
        match self {
            Departure::Controls => "Every player's bike buttons and plane stick. Original is the disc's own layout, on each player's controller preset.",
            Departure::Cameras => "Every player's follow cameras. Further Out sits a little back and higher; Original is the disc's.",
            Departure::Fog => "Original Fog is the game's haze and far clip. Whole Map draws everything, with only a light haze far off.",
            Departure::Plants => "How bushes and trees turn into flat pictures far off. Smooth dissolves them over a wide band; Original swaps them as the game did.",
            Departure::Foliage => "How leaves lie over what is behind them. Crisp cuts each leaf out, so trees show through each other's gaps; Original blends them as the game did, and a near tree's soft edges can hide the trees behind it.",
            Departure::Picture => ASPECT_HELP,
            Departure::Filtering => FILTERING_HELP,
            Departure::Markers => "Every player's session markers on the d-pad. Off, the d-pad steers, leans and does tricks as in the original.",
            Departure::Textures => "The world's textures from texture packs, mods/texture-replacements and mods' own upscales, where there are any. Original is the disc's alone. From the next ride; the software renderer draws them too, slower.",
            Departure::Shadows => SHADOWS_HELP,
        }
    }
}

impl Menu {
    /// Whether a departure is at the original's: for every seat, where it is per seat.
    pub fn is_original(&self, d: Departure) -> bool {
        let (s, c) = (&self.settings, &self.controls);
        match d {
            Departure::Controls => c.layouts.iter().chain(&c.planes).all(|l| *l == Layout::Original),
            Departure::Cameras => s.cameras.iter().all(|v| *v == CameraView::ORIGINAL) && s.camera_sets.iter().all(cameras::Seat::is_disc),
            Departure::Fog => s.graphics == Graphics::Original,
            Departure::Plants => s.plants == Lod::Original,
            Departure::Foliage => s.foliage == Foliage::Original,
            Departure::Picture => s.picture == Picture::Television,
            Departure::Filtering => s.filtering == Filtering::Off,
            Departure::Markers => s.markers.iter().all(|m| !*m),
            Departure::Textures => !s.textures,
            Departure::Shadows => s.shadows == Shadows::Original,
        }
    }

    /// Whether any seat has a departure at the original's: [`Menu::is_original`] but for one seat
    /// enough.
    pub fn some_original(&self, d: Departure) -> bool {
        let (s, c) = (&self.settings, &self.controls);
        match d {
            Departure::Controls => c.layouts.iter().chain(&c.planes).any(|l| *l == Layout::Original),
            Departure::Cameras => s.cameras.contains(&CameraView::ORIGINAL),
            Departure::Markers => s.markers.contains(&false),
            _ => self.is_original(d),
        }
    }

    /// One departure to the original's or back to ours, leaving the others. Back to ours is the
    /// port's default, not whatever it was before: a seat's controller preset and a camera's own
    /// slider moves are not kept.
    pub fn set_departure(&mut self, d: Departure, original: bool) {
        let (s, c) = (&mut self.settings, &mut self.controls);
        match d {
            Departure::Controls => {
                let to = if original { Layout::Original } else { Layout::New };
                c.layouts = [to; SEATS];
                c.planes = [to; SEATS];
                if !original {
                    c.presets = [0; SEATS];
                }
            }
            Departure::Cameras => {
                s.cameras = [if original { CameraView::ORIGINAL } else { CameraView::DEFAULT }; SEATS_MAX];
                // The disc presets' own numbers go with the sliders (P163); the player's own presets
                // stay, out of the cycle at the original's.
                for set in &mut s.camera_sets {
                    set.clear_disc_looks();
                    set.cycle = if original { cameras::Cycle::Disc } else { cameras::Cycle::Both };
                }
            }
            Departure::Fog => s.graphics = if original { Graphics::Original } else { Graphics::Full },
            Departure::Plants => s.plants = if original { Lod::Original } else { Lod::Smooth },
            Departure::Foliage => s.foliage = if original { Foliage::Original } else { Foliage::Cutout },
            Departure::Picture => s.picture = if original { Picture::Television } else { Picture::Fill },
            Departure::Filtering => s.filtering = if original { Filtering::Off } else { Filtering::X16 },
            Departure::Markers => s.markers = [!original; SEATS_MAX],
            Departure::Textures => s.textures = !original,
            Departure::Shadows => s.shadows = if original { Shadows::Original } else { Shadows::Better },
        }
    }

    /// The Original switch's reading: On with every departure and Air Wrecks (P138, not a
    /// departure: its default is the original's) the original's, Off with every departure ours for
    /// every seat (whatever Air Wrecks), Mixed otherwise. The HUD is not read: since P170 there is
    /// only openmx's, or none.
    pub fn original(&self) -> Original {
        if Departure::ALL.iter().all(|d| self.is_original(*d)) && self.settings.air_wrecks && !self.settings.backwards_bail {
            Original::On
        } else if !Departure::ALL.iter().any(|d| self.some_original(*d)) {
            Original::Off
        } else {
            Original::Mixed
        }
    }

    /// The Original switch thrown: on puts every departure and Air Wrecks back to the original's;
    /// off turns every enhancement back on and leaves Air Wrecks as it is. Either way the HUD, the
    /// radar and the scale stay as they are: the original's HUD left the menu in P170, so On rides
    /// openmx's.
    pub fn set_original(&mut self, on: bool) {
        for d in Departure::ALL {
            self.set_departure(d, on);
        }
        if on {
            // Air Wrecks back on (P138): the original throws the rider off in the air.
            self.settings.air_wrecks = true;
            // Backwards Landings Bail off (S161): the original rides a backwards landing out.
            self.settings.backwards_bail = false;
        }
    }
}

/// The settings' render scale (V34): auto, picked from the frames' time against the display's
/// refresh (V30), or a fixed fraction of the frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RenderScale {
    #[default]
    Auto,
    Full,
    ThreeQuarters,
    Half,
}

impl RenderScale {
    pub const ALL: [RenderScale; 4] = [RenderScale::Auto, RenderScale::Full, RenderScale::ThreeQuarters, RenderScale::Half];

    /// The settings file's word: `auto`, or the percentage.
    pub fn word(self) -> &'static str {
        match self {
            RenderScale::Auto => "auto",
            RenderScale::Full => "100",
            RenderScale::ThreeQuarters => "75",
            RenderScale::Half => "50",
        }
    }

    /// A word of the file's, with or without its `%`.
    pub fn from_word(w: &str) -> Option<RenderScale> {
        let w = w.trim_end_matches('%');
        RenderScale::ALL.into_iter().find(|r| r.word() == w)
    }

    pub fn label(self) -> &'static str {
        match self {
            RenderScale::Auto => "Auto",
            RenderScale::Full => "100%",
            RenderScale::ThreeQuarters => "75%",
            RenderScale::Half => "50%",
        }
    }

    /// What `--render-scale` would say: `None` for auto, else the fraction of the frame.
    pub fn fraction(self) -> Option<f32> {
        match self {
            RenderScale::Auto => None,
            RenderScale::Full => Some(1.0),
            RenderScale::ThreeQuarters => Some(0.75),
            RenderScale::Half => Some(0.5),
        }
    }
}

/// The settings' frame limit (P90): the most pictures drawn a second. Only the picture's rate: the
/// ride's ticks are [`crate::pace`]'s 30 Hz whatever this says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FrameLimit {
    /// None of its own: the display's refresh with VSync on, as fast as it draws without.
    #[default]
    Off,
    /// This many a second. The row offers [`FrameLimit::ALL`]'s; the file and `--fps-limit` take any.
    Fps(u16),
    /// The display's refresh, which with VSync off is a limit that does not wait for it.
    Refresh,
}

impl FrameLimit {
    pub const ALL: [FrameLimit; 7] = [
        FrameLimit::Off,
        FrameLimit::Fps(30),
        FrameLimit::Fps(60),
        FrameLimit::Fps(120),
        FrameLimit::Fps(144),
        FrameLimit::Fps(240),
        FrameLimit::Refresh,
    ];

    /// The settings file's and `--fps-limit`'s word: `off`, `refresh`, or the number.
    pub fn word(self) -> String {
        match self {
            FrameLimit::Off => "off".into(),
            FrameLimit::Fps(n) => n.to_string(),
            FrameLimit::Refresh => "refresh".into(),
        }
    }

    /// A word of the file's: `off`, `refresh`, or a number from 1 to 1000, `fps` after it or not.
    pub fn from_word(w: &str) -> Option<FrameLimit> {
        match w.trim().trim_end_matches("fps").trim() {
            "off" | "0" => Some(FrameLimit::Off),
            "refresh" => Some(FrameLimit::Refresh),
            n => n.parse::<u16>().ok().filter(|n| (1..=1000).contains(n)).map(FrameLimit::Fps),
        }
    }

    pub fn label(self) -> String {
        match self {
            FrameLimit::Off => "Off".into(),
            FrameLimit::Fps(n) => format!("{n} fps"),
            FrameLimit::Refresh => "Refresh rate".into(),
        }
    }

    /// The pictures a second it allows on a display of `refresh` hertz; `None` for no limit.
    pub fn fps(self, refresh: f32) -> Option<f32> {
        match self {
            FrameLimit::Off => None,
            FrameLimit::Fps(n) => Some(f32::from(n)),
            FrameLimit::Refresh => Some(refresh),
        }
    }

    /// The row's next choice, or the one before; a number the row does not offer steps to the
    /// first above it, or below.
    fn step(self, back: bool) -> FrameLimit {
        let all = FrameLimit::ALL;
        let n = all.len();
        match all.iter().position(|l| *l == self) {
            Some(i) if back => all[(i + n - 1) % n],
            Some(i) => all[(i + 1) % n],
            None => {
                let FrameLimit::Fps(fps) = self else { return FrameLimit::Off };
                let listed = |l: &&FrameLimit| matches!(l, FrameLimit::Fps(m) if if back { *m < fps } else { *m > fps });
                match back {
                    true => all.iter().rev().find(listed).copied().unwrap_or(FrameLimit::Off),
                    false => all.iter().find(listed).copied().unwrap_or(FrameLimit::Refresh),
                }
            }
        }
    }
}

/// The volume sliders' top, and the step a press moves them: the original's 0 to 100 by fives.
pub const VOLUME_MAX: u8 = 100;
pub const VOLUME_STEP: u8 = 5;

/// The original's audio presets (P56), in its order, each a value for
/// the six sliders, read out of a save state's RAM (journal pause-music) — and Custom, the user's own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mix {
    #[default]
    Game,
    RockNRoll,
    Nighttime,
    Engine,
    Custom,
}

impl Mix {
    pub const ALL: [Mix; 5] = [Mix::Game, Mix::RockNRoll, Mix::Nighttime, Mix::Engine, Mix::Custom];

    pub fn word(self) -> &'static str {
        match self {
            Mix::Game => "game",
            Mix::RockNRoll => "rock",
            Mix::Nighttime => "night",
            Mix::Engine => "engine",
            Mix::Custom => "custom",
        }
    }

    pub fn from_word(w: &str) -> Option<Mix> {
        Mix::ALL.into_iter().find(|m| m.word() == w)
    }

    /// The disc's English names for them.
    pub fn label(self) -> &'static str {
        match self {
            Mix::Game => "Game Mix",
            Mix::RockNRoll => "Rock-n-Roll Mix",
            Mix::Nighttime => "Night-time Mix",
            Mix::Engine => "Engine Mix",
            Mix::Custom => "Custom Mix",
        }
    }

    /// A preset's sliders — music, user soundtracks, sound FX, crowd, engine, menu FX — or `None`
    /// for Custom, which is the user's.
    pub fn levels(self) -> Option<[u8; 6]> {
        match self {
            Mix::Game => Some([70, 70, 80, 45, 60, 100]),
            Mix::RockNRoll => Some([100, 100, 100, 0, 60, 100]),
            Mix::Nighttime => Some([0, 0, 50, 30, 50, 50]),
            Mix::Engine => Some([0, 0, 80, 35, 75, 100]),
            Mix::Custom => None,
        }
    }
}

/// One player's camera (P82), as the Camera page sets it: the field of view's offset from the
/// original's, in degrees, and how far behind and how high above its machine each follow camera sits,
/// the bike's and the biplane's, in percent of the original's. The menu knows numbers only; the shell
/// lays them over the original's presets (`chase::framed`), and at [`CameraView::ORIGINAL`] it hands
/// the camera the disc's numbers untouched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CameraView {
    pub fov: i16,
    pub distance: u16,
    pub height: u16,
    pub plane_distance: u16,
    pub plane_height: u16,
}

/// The field of view slider's span and step, in degrees off the original's.
pub const CAMERA_FOV: (i16, i16) = (-10, 20);
/// The distance and height sliders' span and step, in percent of the original's.
pub const CAMERA_SHARE: (u16, u16) = (50, 200);
pub const CAMERA_STEP: u16 = 5;

impl CameraView {
    /// The original's numbers, each slider's Original notch.
    pub const ORIGINAL: CameraView = CameraView { fov: 0, distance: 100, height: 100, plane_distance: 100, plane_height: 100 };
    /// What ships (user, 2026-10-21: "the default cameras sit a bit further out"): somewhat further
    /// back, a little higher, a few degrees wider, picked by eye in the shell (journal
    /// camera-fov-settings).
    pub const DEFAULT: CameraView = CameraView { fov: 4, distance: 115, height: 110, plane_distance: 120, plane_height: 115 };

    /// The five numbers, fov first, as `settings.txt` keeps them.
    pub fn text(&self) -> String {
        format!("{} {} {} {} {}", self.fov, self.distance, self.height, self.plane_distance, self.plane_height)
    }

    /// [`CameraView::text`] back, each number held to its slider's span; `None` unless it is five
    /// numbers.
    pub fn parse(v: &str) -> Option<CameraView> {
        let n: Vec<i32> = v.split_whitespace().map(|w| w.parse::<i32>().ok()).collect::<Option<_>>()?;
        let [fov, distance, height, plane_distance, plane_height] = n[..] else { return None };
        let share = |v: i32| v.clamp(CAMERA_SHARE.0 as i32, CAMERA_SHARE.1 as i32) as u16;
        Some(CameraView {
            fov: fov.clamp(CAMERA_FOV.0 as i32, CAMERA_FOV.1 as i32) as i16,
            distance: share(distance),
            height: share(height),
            plane_distance: share(plane_distance),
            plane_height: share(plane_height),
        })
    }

    /// The `camera` line's value: one set for every seat, or one a seat with the last carried on.
    pub fn parse_seats(v: &str) -> Option<[CameraView; SEATS_MAX]> {
        let views: Vec<CameraView> = v.split(',').map(CameraView::parse).collect::<Option<_>>()?;
        if views.is_empty() || views.len() > SEATS_MAX {
            return None;
        }
        Some(std::array::from_fn(|i| views[i.min(views.len() - 1)]))
    }

    /// Slider `slider` (the Camera tab's order: field of view, bike distance and height, biplane distance and height) as a number: degrees, or percent.
    pub fn get(&self, slider: usize) -> i32 {
        match slider {
            0 => self.fov as i32,
            1 => self.distance as i32,
            2 => self.height as i32,
            3 => self.plane_distance as i32,
            _ => self.plane_height as i32,
        }
    }

    fn set(&mut self, slider: usize, v: i32) {
        let share = |v: i32| v.clamp(CAMERA_SHARE.0 as i32, CAMERA_SHARE.1 as i32) as u16;
        match slider {
            0 => self.fov = v.clamp(CAMERA_FOV.0 as i32, CAMERA_FOV.1 as i32) as i16,
            1 => self.distance = share(v),
            2 => self.height = share(v),
            3 => self.plane_distance = share(v),
            _ => self.plane_height = share(v),
        }
    }

    /// Slider `slider` a step left or right, held at its ends; with neither, back to the original's.
    fn step(&mut self, slider: usize, left: bool, right: bool) {
        let step = if slider == 0 { 1 } else { CAMERA_STEP as i32 };
        let v = self.get(slider);
        let to = match (left, right) {
            (true, _) => v - step,
            (_, true) => v + step,
            _ => CameraView::ORIGINAL.get(slider),
        };
        self.set(slider, to);
    }

    /// A slider's value as its row shows it: `Original` on the original's notch.
    pub fn label(&self, slider: usize) -> String {
        let v = self.get(slider);
        match slider {
            _ if v == CameraView::ORIGINAL.get(slider) => "Original".to_string(),
            0 => format!("{v:+} deg"),
            _ => format!("{v}%"),
        }
    }

    /// The bike's camera as the shell frames it: (degrees added, distance share, height share).
    pub fn bike(&self) -> (f32, f32, f32) {
        (self.fov as f32, self.distance as f32 / 100.0, self.height as f32 / 100.0)
    }

    /// The biplane's.
    pub fn plane(&self) -> (f32, f32, f32) {
        (self.fov as f32, self.plane_distance as f32 / 100.0, self.plane_height as f32 / 100.0)
    }
}


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
            overlay: Overlay::default(),
            speedometer: Speedometer::default(),
            minimap: true,
            hud_scale: HUD_SCALE_DEFAULT,
            world_sky: WorldSky::Global,
            wrecks: true,
            air_wrecks: true,
            backwards_bail: false,
            vibration: true,
            track: TRACKS[0].0.into(),
            split: Split::default(),
            music: "shuffle".into(),
            music_volume: 70,
            effects_volume: 80,
            folder_volume: 70,
            crowd_volume: 45,
            engine_volume: 60,
            menu_volume: 100,
            mix: Mix::Game,
            custom: [70, 70, 80, 45, 60, 100],
            graphics: Graphics::default(),
            render_scale: RenderScale::default(),
            vsync: true,
            frame_limit: FrameLimit::default(),
            bays: true,
            modifiers: Mods::original(),
            presets: Vec::new(),
            lobby_seats: LOBBY_SEATS,
            cameras: [CameraView::DEFAULT; SEATS_MAX],
            camera_sets: Default::default(),
            mods: true,
            mods_off: Vec::new(),
            track_filter: String::new(),
            tricks: Tricks::default(),
            plants: Lod::default(),
            foliage: Foliage::default(),
            picture: Picture::default(),
            textures: true,
            texture_packs: Vec::new(),
            texture_packs_off: Vec::new(),
            filtering: Filtering::default(),
            shadows: Shadows::default(),
            markers: [true; SEATS_MAX],
        }
    }
}

/// A `texture_packs` line's folder names (V84): comma-separated, each part of a path under `mods/`
/// that climbs nowhere.
fn pack_names(v: &str) -> Vec<String> {
    let plain = |p: &str| !p.is_empty() && p != "." && p != ".." && !p.contains('\\');
    v.split(',').map(str::trim).filter(|n| !n.is_empty() && n.split('/').all(plain)).map(str::to_string).collect()
}

/// `markers`' line (P129): `on` or `off` a seat, 1P's first, the last for every seat past it; `None`
/// for a line with a word it cannot read, or more words than seats.
pub fn markers_seats(v: &str) -> Option<[bool; SEATS_MAX]> {
    let words: Vec<bool> = v
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|w| !w.is_empty())
        .map(|w| match w {
            "on" => Some(true),
            "off" => Some(false),
            _ => None,
        })
        .collect::<Option<_>>()?;
    if words.is_empty() || words.len() > SEATS_MAX {
        return None;
    }
    Some(std::array::from_fn(|i| words[i.min(words.len() - 1)]))
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
        // A file from before P56 has no mix, and its two sliders count to 10.
        let tens = !pairs(text).any(|(k, _)| k == "mix");
        let level = |v: &str, was: u8| match v.parse::<u8>() {
            Ok(n) if tens => n.min(10) * 10,
            Ok(n) => n.min(VOLUME_MAX),
            Err(_) => was,
        };
        for (k, v) in pairs(text) {
            match k {
                "name" => {
                    let name = clean_name(v);
                    if !name.is_empty() {
                        s.name = name;
                    }
                }
                "overlay" => s.overlay = Overlay::from_word(v).unwrap_or(s.overlay),
                "speedometer" => s.speedometer = Speedometer::from_word(v).unwrap_or(s.speedometer),
                "minimap" => s.minimap = v != "off",
                "hud_scale" => s.hud_scale = hud_scale_word(v).unwrap_or(s.hud_scale),
                "world_sky" => s.world_sky = WorldSky::from_word(v).unwrap_or(s.world_sky),
                "wrecks" => s.wrecks = v != "off",
                "air_wrecks" => s.air_wrecks = v != "off",
                "backwards_bail" => s.backwards_bail = v == "on",
                "vibration" => s.vibration = v != "off",
                "track" if TRACKS.iter().any(|t| t.0 == v) || mod_id(v) => s.track = v.to_string(),
                // The ride setup's machine a seat (P41), gone with Start On (P177): every ride starts
                // on the bike, so an old file's line loads and is dropped.
                "machines" => {}
                "split" => s.split = Split::parse(v).unwrap_or(s.split),
                "music" if !v.is_empty() => s.music = v.to_string(),
                "music_volume" => s.music_volume = level(v, s.music_volume),
                "effects_volume" => s.effects_volume = level(v, s.effects_volume),
                "folder_volume" => s.folder_volume = level(v, s.folder_volume),
                "crowd_volume" => s.crowd_volume = level(v, s.crowd_volume),
                "engine_volume" => s.engine_volume = level(v, s.engine_volume),
                "menu_volume" => s.menu_volume = level(v, s.menu_volume),
                "mix" => s.mix = Mix::from_word(v).unwrap_or(s.mix),
                "custom" => {
                    for (i, n) in v.split_whitespace().take(6).enumerate() {
                        s.custom[i] = n.parse::<u8>().map_or(s.custom[i], |n| n.min(VOLUME_MAX));
                    }
                }
                "graphics" => s.graphics = Graphics::from_word(v).unwrap_or(s.graphics),
                "render_scale" => s.render_scale = RenderScale::from_word(v).unwrap_or(s.render_scale),
                "vsync" => s.vsync = v != "off",
                "frame_limit" => s.frame_limit = FrameLimit::from_word(v).unwrap_or(s.frame_limit),
                "machine_bays" => s.bays = v != "off",
                "mods" => s.mods = v != "off",
                "tricks" => s.tricks = Tricks::from_word(v).unwrap_or(s.tricks),
                "mods_off" => {
                    s.mods_off = v.split_whitespace().filter(|w| mod_id(w) || mod_id(&format!("{w}_x"))).map(str::to_string).collect();
                }
                "track_filter" => s.track_filter = clean_filter(v),
                "plants" => s.plants = Lod::from_word(v).unwrap_or(s.plants),
                "foliage" => s.foliage = Foliage::from_word(v).unwrap_or(s.foliage),
                "picture" => s.picture = Picture::from_word(v).unwrap_or(s.picture),
                "textures" => s.textures = v != "original",
                "texture_packs" => s.texture_packs = pack_names(v),
                "texture_packs_off" => s.texture_packs_off = pack_names(v),
                "texture_filtering" => s.filtering = Filtering::from_word(v).unwrap_or(s.filtering),
                "shadows" => s.shadows = Shadows::from_word(v).unwrap_or(s.shadows),
                "camera" => s.cameras = CameraView::parse_seats(v).unwrap_or(s.cameras),
                "camera_start" | "camera_cycle" | "camera_preset" => cameras::parse(&mut s.camera_sets, k, v),
                "markers" => s.markers = markers_seats(v).unwrap_or(s.markers),
                "lobby_seats" => s.lobby_seats = v.parse::<usize>().map_or(s.lobby_seats, |n| n.clamp(2, openmx_net::SEATS)),
                // A line it cannot read leaves the original game rather than half a set (P55).
                "modifiers" => s.modifiers = Mods::parse(v).unwrap_or_default(),
                "preset" => {
                    if let Some((name, mods)) = v.split_once('|')
                        && let Ok(mods) = Mods::parse(mods)
                    {
                        let name = clean_name(name);
                        if !name.is_empty() {
                            s.presets.retain(|(n, _)| *n != name);
                            // A whole table from before P143 is its first player's set.
                            s.presets.push((name, preset_of(&mods, 0)));
                        }
                    }
                }
                _ => {}
            }
        }
        // An older file's two sliders are a mix of the user's own.
        if tens && text.contains("_volume") {
            s.mix = Mix::Custom;
            s.custom = s.levels();
        }
        s
    }

    /// The six sliders in [`Mix::levels`]' order.
    pub fn levels(&self) -> [u8; 6] {
        [self.music_volume, self.folder_volume, self.effects_volume, self.crowd_volume, self.engine_volume, self.menu_volume]
    }

    fn set_levels(&mut self, l: [u8; 6]) {
        [self.music_volume, self.folder_volume, self.effects_volume, self.crowd_volume, self.engine_volume, self.menu_volume] = l;
    }

    /// One slider (in [`Mix::levels`]' order) a step down or up — or, with neither, a step up and
    /// round from the top to nothing, as Cross steps a row — which makes the mix the user's own.
    fn nudge(&mut self, slider: usize, down: bool, up: bool) {
        let mut l = self.levels();
        let v = l[slider];
        l[slider] = match (down, up) {
            (true, _) => v.saturating_sub(VOLUME_STEP),
            (_, true) => (v + VOLUME_STEP).min(VOLUME_MAX),
            _ if v >= VOLUME_MAX => 0,
            _ => (v + VOLUME_STEP).min(VOLUME_MAX),
        };
        self.set_levels(l);
        self.mix = Mix::Custom;
        self.custom = l;
    }

    /// The Audio Presets row: a preset sets every slider, and Custom brings the user's back.
    fn set_mix(&mut self, mix: Mix) {
        self.mix = mix;
        self.set_levels(mix.levels().unwrap_or(self.custom));
    }

    pub fn text(&self) -> String {
        format!(
            "# openmx's front end (P36)\nname = {}\noverlay = {}\nspeedometer = {}\nwrecks = {}\ntrack = {}\nsplit = {}\nmusic = {}\nmusic_volume = {}\neffects_volume = {}\nfolder_volume = {}\ncrowd_volume = {}\nengine_volume = {}\nmenu_volume = {}\nmix = {}\ncustom = {}\ngraphics = {}\nrender_scale = {}\n",
            self.name,
            self.overlay.word(),
            self.speedometer.word(),
            on_off(self.wrecks),
            self.track,
            split_word(self.split),
            self.music,
            self.music_volume,
            self.effects_volume,
            self.folder_volume,
            self.crowd_volume,
            self.engine_volume,
            self.menu_volume,
            self.mix.word(),
            self.custom.map(|n| n.to_string()).join(" "),
            self.graphics.word(),
            self.render_scale.word()
        ) + &self.display_text()
            + &self.bays_text()
            + &self.air_wrecks_text()
            + &self.backwards_bail_text()
            + &self.vibration_text()
            + &self.hud_text()
            + &self.original_text()
            + &self.filtering_text()
            + &self.shadows_text()
            + &self.world_sky_text()
            + &self.mods_text()
            + &self.tricks_text()
            + &self.lobby_seats_text()
            + &self.cameras_text()
            + &cameras::text(&self.camera_sets)
            + &self.markers_text()
            + &self.modifiers_text()

    }

    /// The picture's pacing (P90): VSync and the frame limit.
    fn display_text(&self) -> String {
        format!("vsync = {}\nframe_limit = {}\n", on_off(self.vsync), self.frame_limit.word())
    }

    /// The machines' bays (P103), only when they are off, so a file from before them is written as
    /// it was.
    fn bays_text(&self) -> String {
        if self.bays { String::new() } else { "machine_bays = off\n".into() }
    }

    /// The Air Wrecks switch (P138), only when it is off, so a file from before it is written as it
    /// was.
    fn air_wrecks_text(&self) -> String {
        if self.air_wrecks { String::new() } else { "air_wrecks = off
".into() }
    }

    /// The Backwards Landings Bail switch (S161), only when it is on, so a file from before it is
    /// written as it was.
    fn backwards_bail_text(&self) -> String {
        if self.backwards_bail { "backwards_bail = on\n".into() } else { String::new() }
    }

    /// The Vibration switch (P183), only when it is off, so a file from before it is written as it
    /// was.
    fn vibration_text(&self) -> String {
        if self.vibration { String::new() } else { "vibration = off\n".into() }
    }

    /// The minimap and the HUD's scale (V52), only where they are not the defaults, so a file from
    /// before them is written as it was.
    fn hud_text(&self) -> String {
        let minimap = if self.minimap { String::new() } else { "minimap = off\n".into() };
        let scale = match self.hud_scale {
            HUD_SCALE_DEFAULT => String::new(),
            n => format!("hud_scale = {n}\n"),
        };
        minimap + &scale
    }

    /// The Original switch's own settings (P126), only where they are not ours, so a file from
    /// before them is written as it was.
    fn original_text(&self) -> String {
        let plants = if self.plants == Lod::default() { String::new() } else { format!("plants = {}\n", self.plants.word()) };
        let foliage = if self.foliage == Foliage::default() { String::new() } else { format!("foliage = {}\n", self.foliage.word()) };
        let picture = if self.picture == Picture::default() { String::new() } else { format!("picture = {}\n", self.picture.word()) };
        let textures = if self.textures { String::new() } else { "textures = original\n".into() };
        plants + &foliage + &picture + &textures + &self.texture_packs_text()
    }

    /// The texture packs' order (V84), opentennis's two lines, none while nothing is ordered or off.
    fn texture_packs_text(&self) -> String {
        let line = |k: &str, v: &[String]| if v.is_empty() { String::new() } else { format!("{k} = {}\n", v.join(", ")) };
        line("texture_packs", &self.texture_packs) + &line("texture_packs_off", &self.texture_packs_off)
    }

    /// Texture Filtering (V83), only when it is not 16x, so a file from before it is written as it
    /// was.
    fn filtering_text(&self) -> String {
        if self.filtering == Filtering::default() { String::new() } else { format!("texture_filtering = {}\n", self.filtering.word()) }
    }

    /// Shadows (V108), only when not ours, so a file from before it is written as it was.
    fn shadows_text(&self) -> String {
        if self.shadows == Shadows::default() { String::new() } else { format!("shadows = {}\n", self.shadows.word()) }
    }

    /// A world's sky (MD9), only when it is not the default, so a file from before it is written as
    /// it was.
    fn world_sky_text(&self) -> String {
        match self.world_sky {
            WorldSky::Global => String::new(),
            w => format!("world_sky = {}\n", w.word()),
        }
    }

    /// The mods' switch (MD2), only when it is off, so a file from before it is written as it was.
    fn mods_text(&self) -> String {
        let mut out = if self.mods { String::new() } else { "mods = off\n".to_string() };
        // The packs switched off (P118), only when there are some.
        if !self.mods_off.is_empty() {
            out += &format!("mods_off = {}\n", self.mods_off.join(" "));
        }
        // The pick's filter (MD11), only when it is not All Games.
        if !self.track_filter.is_empty() {
            out += &format!("track_filter = {}\n", self.track_filter);
        }
        out
    }

    /// The tricks' line (MD16), only when they are not the original's, so a file from before it is
    /// written as it was.
    fn tricks_text(&self) -> String {
        if self.tricks == Tricks::Original { String::new() } else { format!("tricks = {}\n", self.tricks.word()) }
    }

    /// The lobby's seat count (N12), only when it is not the usual [`LOBBY_SEATS`], so a file from
    /// before it is written as it was.
    fn lobby_seats_text(&self) -> String {
        if self.lobby_seats == LOBBY_SEATS { String::new() } else { format!("lobby_seats = {}\n", self.lobby_seats) }
    }

    /// The cameras' line (P82), only when a seat's is not the shipped default, so a file from before
    /// them is written as it was: one seat's five numbers for every seat, or one set a seat.
    /// Each seat's Markers switch (P129), only when one is off, so a file from before it is written
    /// as it was: one word a seat as far as the last that differs from the seat after it, the last
    /// word standing for every seat past it (as `camera`'s line).
    fn markers_text(&self) -> String {
        if self.markers.iter().all(|m| *m) {
            return String::new();
        }
        let n = match self.markers.iter().rposition(|m| *m != self.markers[SEATS_MAX - 1]) {
            Some(i) => i + 2,
            None => 1,
        };
        format!("markers = {}\n", self.markers[..n].iter().map(|m| on_off(*m)).collect::<Vec<_>>().join(" "))
    }

    fn cameras_text(&self) -> String {
        if self.cameras.iter().all(|c| *c == CameraView::DEFAULT) {
            return String::new();
        }
        let n = match self.cameras.iter().rposition(|c| *c != self.cameras[SEATS_MAX - 1]) {
            Some(i) => i + 2,
            None => 1,
        };
        format!("camera = {}\n", self.cameras[..n].iter().map(CameraView::text).collect::<Vec<_>>().join(", "))
    }

    /// The modifiers' lines (P55): the set the next ride loads, and the presets. Nothing at all for
    /// the original game with no presets, so a file from before P55 is written as it was.
    fn modifiers_text(&self) -> String {
        let mut out = String::new();
        if !self.modifiers.is_original() {
            out += &format!("modifiers = {}\n", self.modifiers.text());
        }
        for (name, mods) in &self.presets {
            out += &format!("preset = {name} | {}\n", mods.text());
        }
        out
    }

    /// The track pick's index of [`Settings::track`]: a town's, or [`SPLICED`]'s after them.
    pub fn track_index(&self) -> usize {
        match TRACKS.iter().position(|t| t.0 == self.track) {
            Some(i) => i,
            None if self.track == SPLICED.0 => TRACKS.len(),
            None => 0,
        }
    }
}

/// Whether a settings file's `track` may be a track mod's id (MD2): `[a-z0-9_]`, 1 to 40 of them,
/// `<prefix>_<map>` — `track_mods::valid_id`'s rule, which this file is compiled without. Whether
/// the mod is there is the pick's to find out; a track it does not list opens on the first.
/// A filter's name as `settings.txt` keeps it (MD11): one line of printable text with no comment
/// mark, at most 60 characters; anything else is All Games.
fn clean_filter(v: &str) -> String {
    if v.chars().count() <= 60 && v.chars().all(|c| !c.is_control() && c != '#') { v.to_string() } else { String::new() }
}

fn mod_id(v: &str) -> bool {
    (1..=40).contains(&v.len())
        && v.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        && v.trim_matches('_').contains('_')
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

/// The HUD's scale (V52), in percent: the Settings row's range and step, and the original's size.
pub const HUD_SCALE_MIN: u8 = 50;
pub const HUD_SCALE_MAX: u8 = 150;
pub const HUD_SCALE_STEP: u8 = 10;
pub const HUD_SCALE_DEFAULT: u8 = 100;

/// A HUD scale as `settings.txt` and `--hud-scale` spell it, `130` or `130%`: one of the row's
/// steps, or nothing.
pub fn hud_scale_word(w: &str) -> Option<u8> {
    let n = w.trim().trim_end_matches('%').parse::<u8>().ok()?;
    ((HUD_SCALE_MIN..=HUD_SCALE_MAX).contains(&n) && (n - HUD_SCALE_MIN) % HUD_SCALE_STEP == 0).then_some(n)
}

/// The HUD scale a step down or up — or, with neither, a step up and round from the largest to the
/// smallest, as Cross steps a row.
pub fn hud_scale_step(n: u8, down: bool, up: bool) -> u8 {
    match (down, up) {
        (true, _) => n.saturating_sub(HUD_SCALE_STEP).max(HUD_SCALE_MIN),
        (_, true) => (n + HUD_SCALE_STEP).min(HUD_SCALE_MAX),
        _ if n >= HUD_SCALE_MAX => HUD_SCALE_MIN,
        _ => (n + HUD_SCALE_STEP).min(HUD_SCALE_MAX),
    }
}

/// Which overlay a ride draws over each view, as `--overlay` spells it (V26).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Overlay {
    /// The port's own ride HUD (P121): the concept's, in vector shapes and our type (`ride_hud.rs`).
    #[default]
    Hud,
    /// Nothing over the picture.
    None,
}

impl Overlay {
    /// Both, as Settings offers them. The original's HUD and the shell's debug HUD left in P170
    /// (user, 2026-10-26: "we likely wont use them").
    pub const ALL: [Overlay; 2] = [Overlay::Hud, Overlay::None];
    pub const CHOICES: [Overlay; 2] = Overlay::ALL;

    /// The file's word, which is also `--overlay`'s.
    pub fn word(self) -> &'static str {
        match self {
            Overlay::Hud => "hud",
            Overlay::None => "none",
        }
    }

    /// The words an older file wrote for a HUD there is no more of: `original` and `debug` (V26),
    /// and P36's `on` and `off`. A settings file reads each as [`Overlay::Hud`]; `--overlay`
    /// refuses them by name.
    pub const GONE: [&'static str; 4] = ["original", "debug", "on", "off"];

    /// A word back, an old one ([`Overlay::GONE`]) as the HUD.
    pub fn from_word(w: &str) -> Option<Overlay> {
        if Overlay::GONE.contains(&w) {
            return Some(Overlay::Hud);
        }
        Overlay::ALL.into_iter().find(|o| o.word() == w)
    }

    fn label(self) -> &'static str {
        match self {
            Overlay::Hud => "HUD",
            Overlay::None => "None",
        }
    }
}

/// How the original's HUD shows the speed (V42), as `settings.txt` and `--speedometer` spell it. The
/// original offers the same choice in its own overlay options; here the readout is the default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Speedometer {
    /// The number in `yearlingdashboldoblique_55`, with the preload meter beside it.
    #[default]
    Digital,
    /// The dial and its needle, the odometer under it and the preload meter beside it.
    Analog,
}

impl Speedometer {
    pub const ALL: [Speedometer; 2] = [Speedometer::Digital, Speedometer::Analog];

    pub fn word(self) -> &'static str {
        match self {
            Speedometer::Digital => "digital",
            Speedometer::Analog => "analog",
        }
    }

    pub fn from_word(w: &str) -> Option<Speedometer> {
        Speedometer::ALL.into_iter().find(|s| s.word() == w)
    }

    fn label(self) -> &'static str {
        match self {
            Speedometer::Digital => "Digital",
            Speedometer::Analog => "Analog",
        }
    }
}

/// Whose tricks a ride has (MD16), as `settings.txt` and `--tricks` spell it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tricks {
    /// MX Unleashed's, and nothing else: what every replay and acceptance ride is.
    #[default]
    Original,
    /// MX vs. ATV's laid over them from a trick pack (`mods/tricks/<id>/`): its extra stunts on Cross
    /// and Square under the new layout, and under R1 its 360s, its whip and the port's own front flip.
    MxVsAtv,
}

impl Tricks {
    pub const ALL: [Tricks; 2] = [Tricks::Original, Tricks::MxVsAtv];

    pub fn word(self) -> &'static str {
        match self {
            Tricks::Original => "original",
            Tricks::MxVsAtv => "mxvatv",
        }
    }

    pub fn from_word(w: &str) -> Option<Tricks> {
        Tricks::ALL.into_iter().find(|t| t.word() == w)
    }

    pub fn label(self) -> &'static str {
        match self {
            Tricks::Original => "Original",
            Tricks::MxVsAtv => "MX vs. ATV",
        }
    }
}

/// Where a combined world's (MD8) sky, sun, ambient and fog come from (MD9), as `settings.txt` and
/// `--world-sky` spell it. A single track has one of each either way.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WorldSky {
    /// One for the whole world: the map the world mod names for its sky, else its first.
    #[default]
    Global,
    /// The map the camera is over sets them, mixed across a join.
    PerMap,
}

impl WorldSky {
    pub const ALL: [WorldSky; 2] = [WorldSky::Global, WorldSky::PerMap];

    pub fn word(self) -> &'static str {
        match self {
            WorldSky::Global => "global",
            WorldSky::PerMap => "per-map",
        }
    }

    pub fn from_word(w: &str) -> Option<WorldSky> {
        WorldSky::ALL.into_iter().find(|s| s.word() == w)
    }

    fn label(self) -> &'static str {
        match self {
            WorldSky::Global => "Global",
            WorldSky::PerMap => "Per Map",
        }
    }
}

/// Where the throttle comes from, as `--throttle` spells it. Not a Controls row since P81: the new
/// layout's gas is R2 already and the original's is its own binding, so this is `controls.txt`'s
/// `throttle` line and `--throttle` only. L2 is gone from it (user, 2026-10-22): an old file's
/// `throttle = l2` reads as the default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Gas {
    #[default]
    Cross,
    R2,
}

impl Gas {
    pub const ALL: [Gas; 2] = [Gas::Cross, Gas::R2];

    /// The file's word, which is also `--throttle`'s.
    pub fn word(self) -> &'static str {
        match self {
            Gas::Cross => "cross",
            Gas::R2 => "r2",
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
    /// The original's: its binding table exactly — Cross the gas, Square the brake, L1 the clutch,
    /// Select the camera (P73).
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

    /// What the Controls tab's plane row calls a biplane scheme (P81): the plane-relative one is the
    /// normal one, and the disc's is named for what it is.
    pub fn plane_label(self) -> &'static str {
        match self {
            Layout::New => "Normal",
            Layout::Original => "Like the Original",
        }
    }
}

/// A modifier preset as it is kept (P143): player `seat`'s own modifiers, alike on every row, and the
/// world's — one player's set, which any seat can load, and which writes as plain `word=pct`s.
pub fn preset_of(mods: &Mods, seat: usize) -> Mods {
    let mut set = Mods::original();
    for knob in Knob::ALL {
        let pct = mods.get(seat, knob);
        match knob.per_seat() {
            true => (0..game_sim::MAX_PLAYERS).for_each(|s| set.set(s, knob, pct)),
            false => set.set(0, knob, pct),
        }
    }
    set
}

/// Load `preset` onto player `seat` (P143): its own modifiers into that seat's, its world's into the
/// world's, every other seat left as it is.
pub fn load_preset_onto(mods: &mut Mods, preset: &Mods, seat: usize) {
    for knob in Knob::ALL {
        mods.set(if knob.per_seat() { seat } else { 0 }, knob, preset.get(0, knob));
    }
}

/// Where [`Menu::keyboard`] keeps the keyboard of a layout on a controller preset (P79), the new
/// layout's on P80's home row or not (P207).
pub fn keyboard_at(layout: Layout, preset: usize, home_row: bool) -> usize {
    match layout {
        Layout::New => home_row as usize,
        Layout::Original => 2 + preset.min(PRESETS - 1),
    }
}

/// How many seats the file holds a layout for: the shell's `pad::SEATS`.
pub const SEATS: usize = 8;

/// `controls.txt`: how the host's pad is wired, which is the device's and not the rider's, and which
/// layout each seat rides on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Controls {
    pub throttle: Gas,
    /// `--pad`'s device, or none for the first one found.
    pub pad: Option<String>,
    /// `--pad-pressure`'s map of axes to controls, as its text; empty for none.
    pub pressure: String,
    /// `--controls`' layout per seat, 1P first.
    pub layouts: [Layout; SEATS],
    /// `--biplane-controls`' per seat (P50, P69): under `New` the biplane's stick is relative to the
    /// plane at every bank, under `Original` it is the disc's. P50's screen-relative scheme is
    /// `--biplane-controls screen` only, not a choice here.
    pub planes: [Layout; SEATS],
    /// `--preset`'s per seat (P79): which of the original's four controller presets the seat's bike
    /// rides on under the `Original` layout, 0 the shipped one. The original's Controller Presets
    /// screen picks it; here it is the seat's row. Under `New` it is 0 and not read.
    pub presets: [usize; SEATS],
    /// A Nintendo pad's A and B by position (`nintendo = positions`, the default since P64): B, the
    /// bottom one, confirms, as on every other pad. `by-label` has A confirm and B go back, as on a
    /// Switch (P53). P53's default wrote `labels` into every file, so that word reads as positions:
    /// labels are only honoured once picked on the Controls page, which writes `by-label`.
    pub nintendo_positions: bool,
    /// The new layout's keyboard (P207): modern MX games' (`keys = modern`, the default — W the gas,
    /// S the brake, A and D the steer, `IJKL` the lean) or P80's home row (`keys = home-row`, J the
    /// gas and K the brake). The original's layout keeps its own keyboard either way.
    pub home_row_keys: bool,
}

impl Default for Controls {
    fn default() -> Controls {
        Controls {
            throttle: Gas::default(),
            pad: None,
            pressure: String::new(),
            layouts: [Layout::default(); SEATS],
            planes: [Layout::default(); SEATS],
            presets: [0; SEATS],
            nintendo_positions: true,
            home_row_keys: false,
        }
    }
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
                // `labels` is P53's default, written whether or not anyone chose it (P64).
                "nintendo" => c.nintendo_positions = v != "by-label",
                "keys" => c.home_row_keys = v == "home-row",
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
                "biplane" => {
                    if let Some(planes) = per_seat(v) {
                        c.planes = planes;
                    }
                }
                "preset" => {
                    if let Some(presets) = per_seat_preset(v) {
                        c.presets = presets;
                    }
                }
                _ => {}
            }
        }
        c
    }

    pub fn text(&self) -> String {
        format!(
            "# openmx's pad wiring (P36): throttle is cross or r2; pad is a device or auto;\n\
             # pressure is --pad-pressure's axis:control list, or none; layout is new or original,\n\
             # or one per seat (new,original,...) — new is R2 gas, L2 brake, Select camera, with\n\
             # Cross and Square kept free; original is the game's own (P40); biplane is new or\n\
             # original, or one per seat — new is the plane's own stick at every bank, down\n\
             # always pulling the nose toward its belly; original is the disc's (P50, P69); nintendo is\n\
             # positions (a Switch pad's B, the bottom one, confirms) or by-label (A confirms and B\n\
             # goes back) (P53, P64); preset is the original's controller preset, 0 to 3, or one per seat,\n\
             # read under the original layout only — 0 is the shipped one (P79); keys is the new\n\
             # layout's keyboard, modern (W gas, S brake, A D steer, IJKL lean) or home-row (P80's\n\
             # J gas, K brake) (P207)\n\
             throttle = {}\npad = {}\npressure = {}\nlayout = {}\nbiplane = {}\nnintendo = {}\npreset = {}\nkeys = {}\n",
            self.throttle.word(),
            self.pad.as_deref().unwrap_or("auto"),
            if self.pressure.is_empty() { "none" } else { &self.pressure },
            self.layout_text(),
            seat_text(&self.planes),
            if self.nintendo_positions { "positions" } else { "by-label" },
            preset_text(&self.presets),
            if self.home_row_keys { "home-row" } else { "modern" },
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

/// A `controls.txt` value of one layout word for every seat, or one per seat with the last carried
/// on; `None` for anything else.
fn per_seat(v: &str) -> Option<[Layout; SEATS]> {
    let words: Vec<Option<Layout>> = v.split(',').map(|w| Layout::ALL.into_iter().find(|l| l.word() == w.trim())).collect();
    if !words.iter().all(Option::is_some) || words.len() > SEATS {
        return None;
    }
    let mut out = [Layout::default(); SEATS];
    let mut last = Layout::default();
    for (i, slot) in out.iter_mut().enumerate() {
        last = words.get(i).copied().flatten().unwrap_or(last);
        *slot = last;
    }
    Some(out)
}

/// A `controls.txt` `preset` value (P79): one of the original's four controller presets for every
/// seat, or one per seat with the last carried on; `None` for anything else.
fn per_seat_preset(v: &str) -> Option<[usize; SEATS]> {
    let words: Vec<Option<usize>> =
        v.split(',').map(|w| w.trim().parse::<usize>().ok().filter(|p| *p < PRESETS)).collect();
    if !words.iter().all(Option::is_some) || words.len() > SEATS {
        return None;
    }
    let mut out = [0; SEATS];
    let mut last = 0;
    for (i, slot) in out.iter_mut().enumerate() {
        last = words.get(i).copied().flatten().unwrap_or(last);
        *slot = last;
    }
    Some(out)
}

/// [`per_seat_preset`]'s text back: one number when every seat agrees.
fn preset_text(presets: &[usize; SEATS]) -> String {
    if presets.iter().all(|p| *p == presets[0]) {
        return presets[0].to_string();
    }
    presets.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",")
}

/// How many controller presets the original has: `game_sim::bindings::PRESETS`, which this file
/// cannot name because `tests/menu.rs` compiles it on its own.
pub const PRESETS: usize = 4;

/// A seat's layout and its controller preset as its Controls row says them (P79, P81): the presets
/// are inside Original, numbered from 1 as the original's Controller Presets screen does, and its
/// shipped one, Preset 1, is plain `Original`.
pub fn controls_label(l: Layout, preset: usize) -> String {
    match (l, preset) {
        (Layout::Original, p) if p > 0 => format!("Original, Preset {}", p.min(PRESETS - 1) + 1),
        _ => l.label().to_string(),
    }
}

/// The Controls tab's one choice per seat (P81) in the order it steps: New, then Original on each of
/// the original's controller presets.
pub fn control_choices() -> Vec<(Layout, usize)> {
    std::iter::once((Layout::New, 0)).chain((0..PRESETS).map(|p| (Layout::Original, p))).collect()
}

/// [`per_seat`]'s text back: one word when every seat agrees.
fn seat_text(layouts: &[Layout; SEATS]) -> String {
    if layouts.iter().all(|l| *l == layouts[0]) {
        return layouts[0].word().to_string();
    }
    layouts.iter().map(|l| l.word()).collect::<Vec<_>>().join(",")
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
    let read = |name| crate::files::read_to_string(dir.join(name)).unwrap_or_default();
    (Settings::parse(&read(SETTINGS_FILE)), Controls::parse(&read(CONTROLS_FILE)))
}

/// Both files into `dir`.
pub fn save(dir: &Path, settings: &Settings, controls: &Controls) -> Result<(), String> {
    for (name, text) in [(SETTINGS_FILE, settings.text()), (CONTROLS_FILE, controls.text())] {
        let path = dir.join(name);
        crate::files::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
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
    /// The game modifiers (P55), under Settings.
    Modifiers,
    /// The pause's Game Audio Options (P56): the original's page of a song, a preset and six
    /// sliders.
    Audio,
    /// The cameras (P82), under Settings: the field of view and the follow cameras' distance and
    /// height, per player.
    Camera,
    /// The mods folder's switch and its packs, each on or off (P118).
    Mods,
    /// Settings' Graphics tab (P128): the world's draw and the picture's rate, `--screen graphics`.
    Graphics,
    /// The Original Game switch and its departures (P126), a page of the Ride tab's (P179).
    Original,
}

impl Screen {
    pub const ALL: [Screen; 19] = [
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
        Screen::Modifiers,
        Screen::Audio,
        Screen::Camera,
        Screen::Mods,
        Screen::Graphics,
        Screen::Original,
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
            Screen::Modifiers => "modifiers",
            Screen::Audio => "audio",
            Screen::Camera => "camera",
            Screen::Mods => "mods",
            Screen::Graphics => "graphics",
            Screen::Original => "original",
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
            Screen::Modifiers => "Game Modifiers",
            Screen::Audio => "Game Audio Options",
            Screen::Camera => "Camera",
            Screen::Mods => "Mods",
            Screen::Graphics => "Graphics",
            Screen::Original => "Original Game",
        }
    }
}

/// The main menu's rows: opentennis's Local and Online beside Free Ride, as `home.rs` lists them.
/// Mods is P118's: the mods folder's switch and its packs, out of Settings.
pub const MAIN: [&str; 7] = ["Free Ride", "Local Game", "Online", "Garage", "Mods", "Settings", "Quit"];
const MAIN_FREE: usize = 0;
const MAIN_LOCAL: usize = 1;
const MAIN_ONLINE: usize = 2;
const MAIN_GARAGE: usize = 3;
const MAIN_MODS: usize = 4;
const MAIN_SETTINGS: usize = 5;
/// The pause's rows. The third is the original pause menu's "Switch To Machine: %s", with the
/// machine a switch would go to. "Game Modifiers" (P134) is the old front end's way in from the
/// pause, put back: the host's page of every seat's, or the pausing player's own. Audio is only
/// Settings' tab and the port's Clear Markers is gone (P211); a ride's markers clear from the
/// spawn's Down (P135). The online rows go in above Main Menu, which stays right above Quit
/// ([`Menu::pause_layout`]).
pub const PAUSE: [&str; 7] = ["Resume", "Restart", "Switch To Machine", "Settings", "Game Modifiers", "Main Menu", "Quit"];
const PAUSE_SWITCH: usize = 2;
const PAUSE_SETTINGS: usize = 3;
const PAUSE_MODIFIERS: usize = 4;
const PAUSE_MAIN: usize = 5;
/// The pause's "Open to Online" row (N10), on a ride that can be opened ([`Menu::opening`]); opened,
/// the room code.
const PAUSE_OPEN_ROW: &str = "Open to Online";
/// The pause's rows as they stand: [`PAUSE`]'s by index, and the ones that come and go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PauseRow {
    /// One of [`PAUSE`]'s.
    Fixed(usize),
    /// "Open to Online", or the room code once opened (N10).
    Online,
    /// The opened room's seats (N14): only once the ride is open (P211); until then the online row's
    /// chip says how many it opens with.
    Seats,
    /// "Leave Session" (N11, P76).
    Leave,
}
/// The pause's "Leave Session" row (N11), on an online ride ([`Menu::session`]): under the online
/// rows when there are any, above Main Menu. The pausing player's seats go and the ride goes on for
/// the rest. On a local ride whose seats are open ([`Menu::seats_open`], P76) the pausing player
/// gives up their seat and the others ride on; the last one leaving is back on the main menu.
const PAUSE_LEAVE_ROW: &str = "Leave Session";

/// Whether the ride under the pause can be opened to online players (N10), and whether it is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Opening {
    /// Not this ride (online already, more than one local player, a tape, no front end).
    #[default]
    No,
    /// A local freeride: the pause offers "Open to Online".
    Can,
    /// Hosting: the row shows the room code, for others to join by.
    Open,
}

/// The on-screen keyboard: opentennis's four rows of ten, then a row of four commands.
pub const KEY_ROWS: [&str; 4] = ["abcdefghij", "klmnopqrst", "uvwxyz0123", "456789-'.&"];
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
pub struct Readers(Vec<(Dev, Reader)>, Reader, Vec<(Dev, bool)>);

impl Readers {
    pub fn new() -> Readers {
        Readers::default()
    }

    /// Forget every device's last report, so the next frame's are all first ones again.
    pub fn reset(&mut self) {
        self.0.clear();
        self.1.reset();
        self.2.clear();
    }

    /// This frame's reports, a device each, as presses. A device missing from them is forgotten.
    pub fn read(&mut self, reports: &[(Dev, Report)]) -> Vec<(Dev, Press)> {
        let with: Vec<(Dev, Report, u16)> = reports.iter().map(|(d, r)| (d.clone(), *r, 0)).collect();
        self.read_taps(&with)
    }

    /// [`Readers::read`] with each device's presses since the last frame ([`Reader::read_taps`]).
    pub fn read_taps(&mut self, reports: &[(Dev, Report, u16)]) -> Vec<(Dev, Press)> {
        let with: Vec<(Dev, Report, u16, Option<Vec<Moment>>)> = reports.iter().map(|(d, r, t)| (d.clone(), *r, *t, None)).collect();
        self.read_with(&with, None, None)
    }

    /// [`Readers::read_taps`], with the devices that drive the menu together — `hosted`, 1P's and
    /// the keyboard past the splash ([`Menu::hosts`]) — read as one ([`Reader::read_joint`], P109):
    /// the first of them carries their press and the rest none, so one push both report, a frame
    /// apart or in the same one, is one press under [`Menu::step_devices`]. Every device is still
    /// read on its own as well, for the splash, which asks who pressed.
    pub fn read_hosted(&mut self, reports: &[(Dev, Report, u16)], hosted: &dyn Fn(&Dev) -> bool) -> Vec<(Dev, Press)> {
        let with: Vec<(Dev, Report, u16, Option<Vec<Moment>>)> = reports.iter().map(|(d, r, t)| (d.clone(), *r, *t, None)).collect();
        self.read_with(&with, Some(hosted), None)
    }

    /// [`Readers::read_hosted`] at `now` on the shell's clock, each device with its moments since
    /// the last frame ([`Reader::read_timed`], P165). The presses the frame made past each device's
    /// first come after, a step's worth at a time, from [`Readers::more`].
    pub fn read_at(&mut self, reports: &[(Dev, Report, u16, Option<Vec<Moment>>)], hosted: &dyn Fn(&Dev) -> bool, now: u64) -> Vec<(Dev, Press)> {
        self.read_with(reports, Some(hosted), Some(now))
    }

    /// Every device on its own, and with `hosted` the ones it names as one as well.
    fn read_with(
        &mut self,
        reports: &[(Dev, Report, u16, Option<Vec<Moment>>)],
        hosted: Option<&dyn Fn(&Dev) -> bool>,
        now: Option<u64>,
    ) -> Vec<(Dev, Press)> {
        self.0.retain(|(d, _)| reports.iter().any(|(r, _, _, _)| r == d));
        let joint = hosted.is_some();
        let hosted = |d: &Dev| hosted.is_some_and(|h| h(d));
        self.2 = reports.iter().map(|(d, _, _, _)| (d.clone(), hosted(d))).collect();
        let mut presses: Vec<(Dev, Press)> = reports
            .iter()
            .map(|(dev, report, taps, moments)| {
                let at = match self.0.iter().position(|(d, _)| d == dev) {
                    Some(at) => at,
                    None => {
                        self.0.push((dev.clone(), Reader::new()));
                        self.0.len() - 1
                    }
                };
                let reader = &mut self.0[at].1;
                let p = match now {
                    Some(now) => reader.read_timed(&[(*report, *taps, moments.as_deref())], now),
                    None => reader.read_taps(report, *taps),
                };
                (dev.clone(), p)
            })
            .collect();
        if !joint {
            return presses;
        }
        let together: Vec<Timed> = reports.iter().filter(|(d, _, _, _)| hosted(d)).map(|(_, r, t, m)| (*r, *t, m.as_deref())).collect();
        if together.is_empty() {
            self.1.reset();
            return presses;
        }
        let joint = match now {
            Some(now) => self.1.read_timed(&together, now),
            None => {
                let untimed: Vec<(Report, u16)> = together.iter().map(|&(r, t, _)| (r, t)).collect();
                self.1.read_joint(&untimed)
            }
        };
        let mut first = true;
        for (d, p) in &mut presses {
            if hosted(d) {
                *p = if first { joint } else { Press::default() };
                first = false;
            }
        }
        presses
    }

    /// The last frame's next presses past the first (P165, [`Reader::more`]), every device's in the
    /// order [`Readers::read_at`] gave them, the hosted ones' together on the first of them; `None`
    /// once there are none left.
    pub fn more(&mut self) -> Option<Vec<(Dev, Press)>> {
        let mut any = false;
        let mut first = true;
        let mut out = Vec::with_capacity(self.2.len());
        for (dev, hosted) in &self.2 {
            let p = if *hosted {
                if std::mem::take(&mut first) { self.1.more() } else { None }
            } else {
                self.0.iter_mut().find(|(d, _)| d == dev).and_then(|(_, r)| r.more())
            };
            any |= p.is_some();
            out.push((dev.clone(), p.unwrap_or_default()));
        }
        any.then_some(out)
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

/// Where the pause's Main Menu row takes this machine (P110), as the shell says, which its confirm
/// is worded for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LeaveTo {
    /// The main menu: a local ride, or a freeride opened to online, whose room closes with it.
    #[default]
    Main,
    /// The lobby: the host of a lobby's ride takes everyone back to it.
    Lobby,
    /// Out of the online game: a guest leaves the lobby, and the others ride on.
    Online,
}

/// What the pause's confirm (P110) is asking about: its Main Menu row, its Quit row, or its Leave
/// Session row (N11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Leaving {
    Main,
    Quit,
    Session,
}

/// What the pause's song rows ask of the music (P56), beside the settings it changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tune {
    /// Play this song now.
    Play(String),
    /// On to the next song.
    Skip,
    /// Back to the song before (P174).
    Back,
    /// Hold the music where it is (`true`), or let it play on (P174).
    Hold(bool),
}

/// What a press did, for the sound it makes (P65): the original's menus play one of `ui.aud`'s
/// `[UIResponseEmitter]` sounds on each — up_down, left_right, accept, cancel, locked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    /// The highlight moved to another row.
    Move,
    /// A sideways step: a choice, a slider, a tab.
    Side,
    Accept,
    Back,
    /// A press that changed nothing: a slider at its end, a row with nothing to choose.
    Refused,
}

/// A refused press held down repeats silently: the original plays `locked` on a fresh press, not on
/// the d-pad's repeats. Frames a repeat comes within (the repeat's own gap is five).
const REFUSED_QUIET: u64 = 25;

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
    /// Put this seat's player on the machine named, spawned where they are, and carry on (P41,
    /// P102). The shell answers with [`Menu::switched`].
    Switch(usize, Machine),
    Quit,
    /// The lobby's ride starts: [`Lobby::start`] says on what (with [`Lobby::drop_in`], a ride
    /// under way this machine joins).
    Online,
    /// Open the ride under the pause to online players (N10): host a lobby on it, and carry on.
    OpenOnline,
    /// Leave the online ride (N11) for this seat's player: every seat of this machine's for the first
    /// one, that player's alone for another; the ride goes on for the rest.
    LeaveSession(usize),
    /// The controller on this pad seat claims a free seat in the running ride (P76): a new player,
    /// spawned at the track's spawn. The shell answers with [`Menu::reseated`].
    Join(usize),
    /// This player gives up their seat (P76): their bike goes and the others ride on. The shell
    /// answers with [`Menu::reseated`].
    Leave(usize),
}

impl Act {
    /// Whether it ends the ride for every rider, or starts it again (P84): every pause closes.
    pub fn leaves(&self) -> bool {
        matches!(self, Act::Restart | Act::MainMenu | Act::Quit | Act::OpenOnline)
    }
}

/// What the loading screen shows (P127): the track, as the pick names it, and how far its ride's
/// load has got.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Loading {
    /// The track's place in the pick for its still: a disc town's in [`TRACKS`], [`SPLICED`]'s after
    /// them, or `usize::MAX` for a mod's, which has none.
    pub still: usize,
    pub name: String,
    pub pack: String,
    /// The share of the load done, 0 to 1, and what it is doing now ("Ground 12 of 55").
    pub done: f32,
    pub stage: String,
}

/// The front end's state: which screens are open, where each one's cursor is, and the settings it
/// edits.
#[derive(Clone, Debug, PartialEq)]
pub struct Menu {
    pub settings: Settings,
    pub controls: Controls,
    /// The track mods that read (MD2), `(id, name)`, which the track pick lists after the disc's
    /// [`TRACKS`]: empty with no `mods/` folder or with the Mods setting off. Set with
    /// [`Menu::set_mod_tracks`].
    pub mod_tracks: Vec<(String, String)>,
    /// Each mod track's source game (MD11), `(id, game)`, its `mod.json`'s `source.game`, which the
    /// pick's filters are by. A mod not in it is its pack's ([`pack_name`]). Set with
    /// [`Menu::set_mod_games`].
    mod_games: Vec<(String, String)>,
    /// Open screens, the one shown last. Empty is a ride with nothing over it.
    stack: Vec<Screen>,
    /// Each screen's row, in [`Screen::ALL`]'s order.
    cursor: [usize; Screen::ALL.len()],
    /// The keyboard's text while it is being typed, what it is for, and its cursor (row, column).
    typing: String,
    typing_for: Typing,
    key: (usize, usize),
    /// The disc's camera presets the camera button cycles through (P163), the bike's and the
    /// biplane's, by the names their files give them: the Camera page's Preset row lists them. The
    /// shell reads them off the disc; empty, the row has the player's own presets alone.
    pub camera_names: [Vec<String>; 2],
    /// The Camera page's Preset row: 0 for every preset, then [`Menu::camera_choices`]' order.
    camera_pick: usize,
    /// Frames shown, for the track stills' slideshow.
    pub tick: u32,
    /// The settings changed since [`Menu::take_changed`] last said so.
    changed: bool,
    /// A line for the ticker, which is how a ride that could not start says why.
    pub note: Option<String>,
    /// The ride being loaded (P127), which the loading screen shows over everything while it is.
    pub loading: Option<Loading>,
    /// The screens the pick's ride was started from, which a cancelled load goes back to.
    ride_stack: Vec<Screen>,
    /// The ride under the menu, as the shell tells it (P41): how many players ride, the machine each
    /// one is on now, and which of them have a machine to switch to.
    pub riders: usize,
    pub up: [Machine; SEATS_MAX],
    pub switchable: [bool; SEATS_MAX],
    /// The seat whose Start opened the pause, whose machine its switch row is.
    pauser: usize,
    /// The machine the pause's switch row has picked (P102): left and right step it through the
    /// pauser's [`Menu::choices`], and OK switches to it. `None` until a step, which is the first.
    switch_pick: Option<Machine>,
    /// What the garage offers, off the disc; empty until [`Menu::with_garage`] gives it one.
    pub catalogue: Catalogue,
    /// Every seat's pick, as `garage.txt` keeps them.
    pub picks: Picks,
    /// The garage's seats while it is open, one [`Garage`] each.
    garages: Vec<Garage>,
    /// How many seats have a player, which is how many columns the garage shows.
    pub seated: usize,
    /// The player riding the keyboard rather than a pad, if one is (P178): the Controls tab shows
    /// that player the keyboard's keys and every other the pad. The shell fills it.
    pub keys_seat: Option<usize>,
    /// The picks changed since [`Menu::take_picks_changed`] last said so.
    picks_changed: bool,
    /// The saved looks (P86), as `riders.txt` keeps them.
    pub looks: roster::Presets,
    /// A seat saved a look since [`Menu::take_looks_changed`] last said so.
    looks_changed: bool,
    /// Online play's side of the menu (N3).
    pub lobby: Lobby,
    /// The pause's "Open to Online" (N10), as the shell says the ride under it stands.
    pub opening: Opening,
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
    /// The pause's confirm (P110): what its leave row asks, and its row, 1 being Stay. The pauser
    /// alone answers it, as they alone drive the pause.
    leaving: Option<(Leaving, usize)>,
    /// Where the pause's Main Menu row goes, as the shell says.
    pub leave_to: LeaveTo,
    /// The ride under the pause is online (N11), as the shell says: its Leave Session row.
    pub session: bool,
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
    /// The keyboard under each layout, as the shell's bindings have it (P54): what each key does in
    /// a ride, for the Controls screen. The new layout first, on the modern keyboard then on P80's
    /// home row (P207), then the original's on each of its controller presets, 0 to 3 (P79) —
    /// [`keyboard_at`]'s order. Empty without a shell.
    pub keyboard: [Vec<(String, String)>; 2 + PRESETS],
    /// The local game the ride setup is for, once the join screen is done; none for a Free Ride.
    party: Option<Party>,
    /// How many ride a Free Ride, which is `--players`: the shell says, and a Free Ride after a
    /// local game goes back to it.
    pub free_riders: usize,
    /// Whether the ride under the menu takes players in and lets them go while it runs (P76), as the
    /// shell says: a local freeride, not online and riding no tape.
    pub seats_open: bool,
    /// The songs the music row steps through after Off and Shuffle, `(id, label)`, as the shell's
    /// music sources list them (P42). Empty until the shell says.
    pub music_choices: Vec<(String, String)>,
    /// The preset the modifiers' Load Preset row is on (P55).
    mod_preset: usize,
    /// Each screen's tab, in [`Screen::ALL`]'s order (P118): Settings' and the Garage's.
    tab: [usize; Screen::ALL.len()],
    /// Whose rows the pages' Player rows show, 0 for 1P (P118).
    player: usize,
    /// The modifiers page's Player row is on All Players (P134), the host's head of the row: a
    /// modifier set there is every seat's.
    mods_all: bool,
    /// The modifiers the host set through All Players since the shell last took them
    /// ([`Menu::take_mods_all`]): online every seat's, not only this machine's (P134).
    mods_all_set: Vec<Knob>,
    /// This machine is a guest of an online game, as the shell says (P134): its Game Modifiers page
    /// sets its own player's alone and shows the world's, which are the host's.
    pub guest: bool,
    /// The device and the pressure map `controls.txt` named, kept while their rows are on First
    /// Found and None, to switch back to.
    pad_kept: Option<String>,
    pressure_kept: Option<String>,
    /// Every mod track the folder listed this session, for the Mods page's packs, which stay listed
    /// while the folder is off.
    mods_seen: Vec<(String, String)>,
    /// Every music pack the folder listed this session (P122), `(id, name, songs)`, for the Mods
    /// page, which keeps them listed while the folder is off. Set with [`Menu::set_music_packs`].
    music_seen: Vec<(String, String, usize)>,
    /// The ride under the menu has a modifier off the original game's, as the shell tells it: the
    /// pause says so.
    pub modified: bool,
    /// Why the ride under the pause cannot take a change of modifiers (P98), as the shell says: online,
    /// or riding a tape. With it set the pause's modifiers screen shows them and does not change them;
    /// without it a change takes the moment the pause closes. The main menu's screen is never held.
    pub mods_lock: Option<String>,
    /// Why the ride under the pause cannot take a change of Air Wrecks (P138), as the shell says: a
    /// played tape rides its own, and online every machine rides the room's from the start. `None`:
    /// a change takes as the pause closes.
    pub air_lock: Option<String>,
    /// The song playing, by id, as the shell last said (P56).
    pub music_now: Option<String>,
    /// Whether the music is held, as the shell last said, or as the pause's player just asked (P174).
    pub music_held: bool,
    /// The song the Game Audio Options' song row is on, in [`Menu::music_choices`].
    music_at: usize,
    /// What the song rows asked since [`Menu::take_tunes`] last took it.
    tunes: Vec<Tune>,
    /// The sounds the presses made since [`Menu::take_cues`] last took them (P65).
    cues: Cues,
    /// The time the shell says has passed (P165), as the ticks the menu's own timers count.
    pace: Pace,
    /// Every other rider's own pause (P84), by seat, 1P's being the menu's own fields: each a stack
    /// and cursors of its own, driven by that rider's pad and drawn in their view
    /// ([`Menu::with_seat`]). Empty until a seat after the first pauses.
    sides: Vec<Side>,
    /// The seat whose pause is in the menu's own fields while [`Menu::with_seat`] runs, 0 otherwise.
    focus: usize,
    /// What other seats' pauses asked in a frame that one had already asked something in, handed out
    /// by the next frames' [`Menu::step_seats`] (P84).
    later: Vec<Act>,
}

/// The sounds the presses made, and what decides them (P65). Not the menu's state: two menus that
/// show and do the same are equal whatever they last sounded.
#[derive(Clone, Default)]
struct Cues {
    list: Vec<Cue>,
    /// Frames stepped, and the last refused press and its frame, which a held repeat does not sound
    /// again.
    frames: u64,
    refused: Option<(Press, u64)>,
}

impl PartialEq for Cues {
    fn eq(&self, _: &Cues) -> bool {
        true
    }
}

impl std::fmt::Debug for Cues {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Cues")
    }
}

/// The rate of the menu's own timers' ticks: the Player 1 card, the splash's breath, the track
/// stills and a refused press's quiet are counted in sixtieths of a second (P165).
const TICK_HZ: u64 = 60;
/// The most time one [`Menu::elapse_to`] moves the menu's timers by.
const ELAPSE_MAX_US: u64 = 250_000;

/// The menu's clock (P165): what the shell says has passed ([`Menu::elapse`]), as [`TICK_HZ`] ticks.
/// A menu nobody times steps a tick a step, as every menu did before. Not the menu's state, like
/// [`Cues`]: two menus that show and do the same are equal whenever they were stepped.
#[derive(Clone, Default)]
struct Pace {
    /// Whether the shell keeps time for this menu.
    timed: bool,
    /// The time passed short of a whole tick, in millionths of a tick.
    carry: u64,
    /// The whole ticks passed that the next step takes.
    due: u32,
    /// The ticks the step being taken stands for.
    step: u32,
    /// The shell's clock at the last [`Menu::elapse_to`].
    at: Option<u64>,
}

impl PartialEq for Pace {
    fn eq(&self, _: &Pace) -> bool {
        true
    }
}

impl std::fmt::Debug for Pace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Pace")
    }
}

/// One rider's pause (P84): everything of the menu a pause and the pages under it move, kept for a
/// seat whose pause is not the one in the menu's own fields.
#[derive(Clone, Debug, PartialEq)]
struct Side {
    stack: Vec<Screen>,
    cursor: [usize; Screen::ALL.len()],
    tab: [usize; Screen::ALL.len()],
    player: usize,
    pauser: usize,
    switch_pick: Option<Machine>,
    leaving: Option<(Leaving, usize)>,
    typing: String,
    typing_for: Typing,
    key: (usize, usize),
}

impl Side {
    /// Seat `seat`'s, riding: nothing open, every cursor on its first row.
    fn new(seat: usize) -> Side {
        Side {
            stack: Vec::new(),
            cursor: [0; Screen::ALL.len()],
            tab: [0; Screen::ALL.len()],
            // the pages' Player rows open on the rider whose pause it is
            player: seat,
            pauser: seat,
            switch_pick: None,
            leaving: None,
            typing: String::new(),
            typing_for: Typing::Name,
            key: (0, 0),
        }
    }
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
            mod_tracks: Vec::new(),
            mod_games: Vec::new(),
            stack: vec![Screen::Main],
            cursor,
            typing: String::new(),
            typing_for: Typing::Name,
            key: (0, 0),
            camera_names: Default::default(),
            camera_pick: 0,
            tick: 0,
            changed: false,
            note: None,
            loading: None,
            ride_stack: Vec::new(),
            riders: 1,
            up: [Machine::Bike; SEATS_MAX],
            switchable: [false; SEATS_MAX],
            pauser: 0,
            switch_pick: None,
            catalogue: Catalogue::default(),
            picks: Picks::default(),
            garages: Vec::new(),
            seated: 1,
            keys_seat: None,
            picks_changed: false,
            looks: roster::Presets::default(),
            looks_changed: false,
            lobby: Lobby::default(),
            opening: Opening::No,
            p1: None,
            p1_name: String::new(),
            claimed: None,
            quit: None,
            leaving: None,
            leave_to: LeaveTo::Main,
            session: false,
            seats_open: false,
            join: Join::new(join::ON_SCREEN, Opened::Menu),
            devices: Vec::new(),
            heard: Vec::new(),
            skipped: Vec::new(),
            keyboard: Default::default(),
            party: None,
            free_riders: 1,
            music_choices: Vec::new(),
            mod_preset: 0,
            tab: [0; Screen::ALL.len()],
            player: 0,
            mods_all: false,
            mods_all_set: Vec::new(),
            guest: false,
            pad_kept: None,
            pressure_kept: None,
            mods_seen: Vec::new(),
            music_seen: Vec::new(),
            modified: false,
            mods_lock: None,
            air_lock: None,
            music_now: None,
            music_held: false,
            music_at: 0,
            tunes: Vec::new(),
            cues: Cues::default(),
            pace: Pace::default(),
            sides: Vec::new(),
            focus: 0,
            later: Vec::new(),
        }
    }

    /// The sounds the menu's presses made since this was last asked (P65), oldest first.
    pub fn take_cues(&mut self) -> Vec<Cue> {
        std::mem::take(&mut self.cues.list)
    }

    /// Step the menu with `walk` for the press `p`, and queue the sound the press made: what the
    /// menu looks like after it against before. A ride running is silent; so is the press that
    /// pauses it.
    fn cued(&mut self, p: Press, walk: impl FnOnce(&mut Menu) -> Act) -> Act {
        self.pace.step = self.take_ticks();
        self.cues.frames += u64::from(self.pace.step);
        if p == Press::default() || self.riding() {
            return walk(self);
        }
        let mut before = self.clone();
        let act = walk(self);
        before.tick = self.tick;
        let same = before == *self;
        let cue = Menu::cue_for(&before, self, p, &act, same);
        let c = &mut self.cues;
        match cue {
            Some(Cue::Refused) => {
                let quiet = matches!(c.refused, Some((q, at)) if q == p && c.frames - at <= REFUSED_QUIET);
                c.refused = Some((p, c.frames));
                if !quiet {
                    c.list.push(Cue::Refused);
                }
            }
            Some(cue) => {
                c.refused = None;
                c.list.push(cue);
            }
            None => {}
        }
        act
    }

    /// The sound a press made, from what it did.
    fn cue_for(before: &Menu, after: &Menu, p: Press, act: &Act, same: bool) -> Option<Cue> {
        let backing = p.back && !p.ok;
        if *act != Act::Stay {
            return Some(if backing { Cue::Back } else { Cue::Accept });
        }
        if same {
            // An option row that would not change: a slider at its end, a dimmed row. The original
            // is silent on a press that means nothing where it is (a sideways press on its main
            // menu), so is this.
            let Some(row) = before.screen().and_then(|s| before.page().get(before.cursor(s)).cloned()) else { return None };
            let side = p.left || p.right;
            let refused = (row.off.is_some() && (side || p.ok)) || (side && !row.value.is_empty());
            return refused.then_some(Cue::Refused);
        }
        Some(if after.stack.len() < before.stack.len() || (backing && after.stack != before.stack) {
            Cue::Back
        } else if after.stack.len() > before.stack.len() || after.stack != before.stack {
            Cue::Accept
        } else if p.up || p.down {
            Cue::Move
        } else if p.left || p.right || p.l1 || p.r1 {
            Cue::Side
        } else if p.ok || p.start || p.delete || p.triangle {
            Cue::Accept
        } else {
            Cue::Back
        })
    }

    /// What the Game Audio Options' song rows asked of the music since this was last asked (P56).
    pub fn take_tunes(&mut self) -> Vec<Tune> {
        std::mem::take(&mut self.tunes)
    }

    /// A song's label, by id.
    fn song_label(&self, id: &str) -> String {
        match self.music_choices.iter().find(|(i, _)| i == id) {
            Some((_, label)) => label.clone(),
            None => id.to_string(),
        }
    }

    /// The song row's song, by id.
    fn song_at(&self) -> Option<&str> {
        self.music_choices.get(self.music_at).map(|(id, _)| id.as_str())
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
                    let at = layouts(self.riders).iter().position(|l| {
                        *l == split::Layout::for_players(self.riders, self.settings.split).unwrap_or(split::Layout::Full)
                    });
                    self.cursor[Screen::Split.index()] = at.unwrap_or(0);
                    self.party = Some(Party { devices, split: self.settings.split });
                    if self.riders == 1 {
                        // Alone (P171) there is no layout to pick: straight on to the track.
                        self.cursor[Screen::Tracks.index()] = self.track_index();
                        self.tick = 0;
                        self.push(Screen::Tracks);
                    } else {
                        self.push(Screen::Split);
                    }
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

    /// The saved looks the garage's Preset row loads (P86).
    pub fn with_looks(mut self, looks: roster::Presets) -> Menu {
        self.looks = looks;
        self
    }

    /// Whether a seat saved a look since the last time this was asked, which is when the shell
    /// writes `riders.txt`.
    pub fn take_looks_changed(&mut self) -> bool {
        std::mem::take(&mut self.looks_changed)
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
        // Online this machine's first seat is its rider (N3), in its look (P86).
        self.lobby.pick.look = self.picks.0[0].look.text();
        self.pop();
    }

    /// A frame of another seat's pad (seat 0's is [`Menu::step`]'s). Only the join screen hears it,
    /// each pressing for its own seat (P43); since P118 the garage is 1P's, its Player row choosing
    /// whose rider it dresses.
    pub fn step_seat(&mut self, seat: usize, p: Press) {
        if seat > 0 && self.screen() == Some(Screen::Join) {
            // Its step takes none of the frame's time (P165): 1P's, after it, does.
            let due = std::mem::take(&mut self.pace.due);
            self.cued(p, |m| {
                m.walk_join(seat, p);
                Act::Stay
            });
            self.pace.due += due;
        }
    }

    /// The splash, 1P free for the next press.
    pub fn splash(&mut self) {
        self.stack = vec![Screen::Splash];
        self.p1 = None;
        self.claimed = None;
        self.quit = None;
        self.tick = 0;
        self.close_sides();
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
        if gone && !self.stack.is_empty() && !self.stack.contains(&Screen::Pause) {
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
            let any = presses.iter().fold(Press::default(), |p, (_, q)| p.or(*q));
            return self.cued(any, |m| m.splash_step(presses));
        }
        let p = presses.iter().filter(|(d, _)| self.hosts(d)).fold(Press::default(), |p, (_, q)| p.or(*q));
        self.step(p)
    }

    /// The splash's frame.
    fn splash_step(&mut self, presses: &[(Dev, Press)]) -> Act {
        self.tick = self.tick.wrapping_add(self.pace.step);
        if let Some(n) = &mut self.claimed {
            *n += self.pace.step;
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

    /// The menu as [`Art::draw`] shows it (V30): itself, but with `tick` only as far as the screen
    /// reads it — the splash's prompt breathes each frame, the track stills turn every
    /// [`STILL_FRAMES`], the rest stand still — so two equal looks draw the same picture, and a menu
    /// standing still is drawn once and kept.
    pub fn look(&self) -> Menu {
        let mut look = self.clone();
        look.tick = match self.screen() {
            Some(Screen::Splash) => self.tick,
            Some(Screen::Tracks) => self.tick / STILL_FRAMES,
            _ => 0,
        };
        look
    }

    /// Whether a ride is running with nothing over it: no rider has their pause open (P84).
    pub fn riding(&self) -> bool {
        self.stack.is_empty() && self.sides.iter().all(|s| s.stack.is_empty())
    }

    /// Whether `seat`'s rider has their pause open (P84), and with it their machine frozen. 1P's is
    /// the menu's own: under the front end anything over the ride.
    pub fn paused(&self, seat: usize) -> bool {
        match self.side_index(seat) {
            None => !self.stack.is_empty(),
            Some(i) => self.sides.get(i).is_some_and(|s| !s.stack.is_empty()),
        }
    }

    /// Where seat `seat`'s pause is kept, `None` for the one in the menu's own fields. While
    /// [`Menu::with_seat`] has seat `k` in them, `k`'s side holds 1P's.
    fn side_index(&self, seat: usize) -> Option<usize> {
        match (seat, self.focus) {
            (s, f) if s == f => None,
            (0, f) => Some(f),
            (s, _) if s < SEATS_MAX => Some(s),
            _ => None,
        }
    }

    /// The menu with `seat`'s own pause in it (P84): what that rider's view draws, and what `f` steps.
    /// 1P's is the menu as it is; any other seat's state is swapped into the menu's own fields for
    /// `f` and back after it, so every screen and page works the same for every rider.
    pub fn with_seat<R>(&mut self, seat: usize, f: impl FnOnce(&mut Menu) -> R) -> R {
        if seat == 0 || seat >= SEATS_MAX || self.focus != 0 {
            return f(self);
        }
        self.swap_side(seat);
        self.focus = seat;
        let r = f(self);
        self.focus = 0;
        self.swap_side(seat);
        r
    }

    /// Where the pauses are drawn over a ride in `rects`' views (P84), `(seat, view)`: over a split
    /// each paused player's own in their own view, the first `players` views being theirs in order;
    /// over one view the menu's own over all of it, whatever is on it.
    pub fn pause_views(&self, rects: &[split::Rect], players: usize) -> Vec<(usize, split::Rect)> {
        match rects {
            [] => Vec::new(),
            [whole] => self.screen().is_some().then_some((0, *whole)).into_iter().collect(),
            views => views.iter().copied().enumerate().take(players).filter(|&(v, _)| self.paused(v)).collect(),
        }
    }

    /// A copy of the menu as `seat`'s view shows it: [`Menu::with_seat`]'s, kept.
    pub fn view(&self, seat: usize) -> Menu {
        let mut m = self.clone();
        if seat > 0 && seat < SEATS_MAX && self.focus == 0 {
            m.swap_side(seat);
            m.focus = seat;
        }
        m
    }

    /// Seat `seat`'s side and the menu's own fields traded.
    fn swap_side(&mut self, seat: usize) {
        if self.sides.len() <= seat {
            let have = self.sides.len();
            self.sides.extend((have..=seat).map(Side::new));
        }
        let side = &mut self.sides[seat];
        std::mem::swap(&mut self.stack, &mut side.stack);
        std::mem::swap(&mut self.cursor, &mut side.cursor);
        std::mem::swap(&mut self.tab, &mut side.tab);
        std::mem::swap(&mut self.player, &mut side.player);
        std::mem::swap(&mut self.pauser, &mut side.pauser);
        std::mem::swap(&mut self.switch_pick, &mut side.switch_pick);
        std::mem::swap(&mut self.leaving, &mut side.leaving);
        std::mem::swap(&mut self.typing, &mut side.typing);
        std::mem::swap(&mut self.typing_for, &mut side.typing_for);
        std::mem::swap(&mut self.key, &mut side.key);
    }

    /// Every other rider's pause closed (P84): the ride is over, or started again, for all of them.
    fn close_sides(&mut self) {
        if self.focus == 0 {
            self.sides.clear();
            self.later.clear();
        }
    }

    /// A ride on `track` is loading (P127): the loading screen, with the track's name and its pack
    /// as the pick shows them, and nothing done yet.
    pub fn start_loading(&mut self, track: &str) {
        let disc = TRACKS.iter().position(|(id, _)| *id == track).or((track == SPLICED.0).then_some(TRACKS.len()));
        let name = match disc {
            Some(i) if i == TRACKS.len() => SPLICED.1.to_string(),
            Some(i) => TRACKS[i].1.to_string(),
            None => self.mod_tracks.iter().find(|(id, _)| id == track).map_or_else(|| track.to_string(), |(_, n)| n.clone()),
        };
        let pack = match disc {
            Some(_) => "MX Unleashed".to_string(),
            None => self.packs().into_iter().find(|p| p.prefix == pack_of(track)).map_or("Mods".into(), |p| p.name),
        };
        self.loading = Some(Loading { still: disc.unwrap_or(usize::MAX), name, pack, done: 0.0, stage: String::new() });
    }

    /// A cancelled load (P127): back to the screens the ride was picked from, the pick on top.
    pub fn back_to_pick(&mut self) {
        self.loading = None;
        self.stack = match std::mem::take(&mut self.ride_stack) {
            s if s.last() == Some(&Screen::Tracks) => s,
            _ => vec![Screen::Main, Screen::Tracks],
        };
        self.cursor[Screen::Tracks.index()] = self.track_index();
    }

    /// Back to the main menu, with nothing under it.
    pub fn main(&mut self) {
        self.stack = vec![Screen::Main];
        self.ride_stack.clear();
        self.pauser = self.focus;
        self.leaving = None;
        self.close_sides();
    }

    /// The pause over a ride, 1P's.
    pub fn pause(&mut self) {
        self.pause_for(0);
    }

    /// The pause over a ride, opened by `seat`'s Start: that rider's own (P84), in their view and
    /// driven by their pad, its switch row their machine. The others ride on with theirs.
    pub fn pause_for(&mut self, seat: usize) {
        let seat = seat.min(SEATS_MAX - 1);
        self.with_seat(seat, |m| {
            m.pauser = seat;
            m.open_pause();
        });
    }

    /// The pause opened over the ride in the menu's own fields, for [`Menu::pauser`]'s seat.
    fn open_pause(&mut self) {
        self.stack = vec![Screen::Pause];
        self.cursor[Screen::Pause.index()] = 0;
        self.switch_pick = None;
        self.leaving = None;
    }

    /// The pause's confirm, if it is up (P110): its question, its two rows, the leaving one first
    /// and Stay second, and the row it is on.
    pub fn confirming(&self) -> Option<(&'static str, [&'static str; 2], usize)> {
        let (what, row) = self.leaving.filter(|_| self.screen() == Some(Screen::Pause))?;
        let (ask, go) = match (what, self.leave_to) {
            (Leaving::Quit, _) => ("Quit the game?", QUIT_ROWS[0]),
            (Leaving::Main, LeaveTo::Main) => ("Main menu?", "Main Menu"),
            (Leaving::Main, LeaveTo::Lobby) => ("Return to lobby?", "Return to Lobby"),
            (Leaving::Main, LeaveTo::Online) => ("Leave the online game?", "Leave Game"),
            (Leaving::Session, _) if self.pauser > 0 => ("Leave the session? The others ride on.", PAUSE_LEAVE_ROW),
            (Leaving::Session, _) => ("Leave the session? Everyone on this machine goes; the others ride on.", PAUSE_LEAVE_ROW),
        };
        Some((ask, [go, QUIT_ROWS[1]], row))
    }

    /// One press on the pause's confirm: any direction moves between its two rows, Cross on the
    /// first leaves, and Cross on Stay, Circle or Start go back to the pause with the cursor where it
    /// was.
    fn confirm(&mut self, p: Press) -> Act {
        let Some((what, row)) = self.leaving else { return Act::Stay };
        if p.up || p.down || p.left || p.right {
            self.leaving = Some((what, 1 - row));
        } else if p.ok && row == 0 {
            self.leaving = None;
            return match what {
                Leaving::Main => {
                    self.main();
                    Act::MainMenu
                }
                Leaving::Quit => Act::Quit,
                // back to the ride while it lets this machine's seats go
                Leaving::Session => {
                    self.stack.clear();
                    Act::LeaveSession(self.pauser)
                }
            };
        } else if p.ok || p.back || p.start {
            self.leaving = None;
        }
        Act::Stay
    }

    /// The machines `seat` can be switched to from the one they are on, in [`Machine::ALL`]'s
    /// order: every one but that, for a player the shell says has another ([`Menu::switchable`]),
    /// and none for one it says has not.
    pub fn choices(&self, seat: usize) -> Vec<Machine> {
        let (up, has) = (self.up.get(seat).copied().unwrap_or_default(), self.switchable.get(seat).copied().unwrap_or(false));
        Machine::ALL.into_iter().filter(|&m| has && m != up).collect()
    }

    /// The machine the pause's switch row would put its player on, if any.
    pub fn switch_to(&self) -> Option<Machine> {
        let choices = self.choices(self.pauser);
        self.switch_pick.filter(|m| choices.contains(m)).or(choices.first().copied())
    }

    /// The shell's answer to [`Act::Switch`]: `seat` is on `up` now.
    pub fn switched(&mut self, seat: usize, up: Machine) {
        if let Some(m) = self.up.get_mut(seat) {
            *m = up;
        }
    }

    /// Why Air Wrecks cannot be changed here: the ride under the pause holds it (P138).
    pub(crate) fn air_held(&self) -> Option<&str> {
        match self.stack.first() {
            Some(Screen::Pause) => self.air_lock.as_deref(),
            _ => None,
        }
    }

    /// Why the modifiers cannot be changed here: the ride under the pause holds them (P98).
    fn mods_held(&self) -> Option<&str> {
        match self.stack.first() {
            Some(Screen::Pause) => self.mods_lock.as_deref(),
            _ => None,
        }
    }

    /// Keep the modifiers now set as preset `name`, over one already called that.
    fn save_preset(&mut self, name: &str) {
        let mods = self.preset_now();
        match self.settings.presets.iter().position(|(n, _)| n == name) {
            Some(i) => {
                self.settings.presets[i].1 = mods;
                self.mod_preset = i;
            }
            None => {
                self.settings.presets.push((name.to_string(), mods));
                self.mod_preset = self.settings.presets.len() - 1;
            }
        }
        self.changed = true;
    }

    /// The pause's rows in order (P211): [`PAUSE`]'s up to Game Modifiers, then the online row, its
    /// Seats once opened and Leave Session, then Main Menu and Quit, always the last two.
    fn pause_layout(&self) -> Vec<PauseRow> {
        let mut rows: Vec<PauseRow> = (0..PAUSE_MAIN).map(PauseRow::Fixed).collect();
        if self.opening != Opening::No {
            rows.push(PauseRow::Online);
        }
        if self.opening == Opening::Open {
            rows.push(PauseRow::Seats);
        }
        if self.session || self.seats_open {
            rows.push(PauseRow::Leave);
        }
        rows.extend((PAUSE_MAIN..PAUSE.len()).map(PauseRow::Fixed));
        rows
    }

    /// The pause's rows as drawn: the switch row names the machine it would go to, or says there is
    /// none for this player.
    pub fn pause_rows(&self) -> Vec<String> {
        let seat = self.pauser;
        self.pause_layout()
            .into_iter()
            .map(|row| match row {
                PauseRow::Fixed(PAUSE_SWITCH) => {
                    let row = PAUSE[PAUSE_SWITCH];
                    match self.switch_to() {
                        Some(m) if self.choices(seat).len() > 1 => format!("{row}: < {} >", m.label()),
                        Some(m) => format!("{row}: {}", m.label()),
                        None => format!("{row}: None"),
                    }
                }
                PauseRow::Fixed(i) => PAUSE[i].to_string(),
                PauseRow::Online => match self.opening {
                    Opening::Open if self.lobby.code.is_empty() => "Online: opening...".into(),
                    Opening::Open => format!("Online: {}", self.lobby.code),
                    _ => PAUSE_OPEN_ROW.into(),
                },
                PauseRow::Seats => format!("Seats: {}", self.pause_seats()),
                PauseRow::Leave => PAUSE_LEAVE_ROW.into(),
            })
            .collect()
    }

    /// Player `i`'s pad seat in the ride under the menu: a local game's, or a Free Ride's own seat.
    pub fn ride_devices(&self) -> Vec<usize> {
        match &self.party {
            Some(party) => party.devices.clone(),
            None => (0..self.riders.max(1)).collect(),
        }
    }

    /// One frame of a controller on a pad seat that has no player in the ride (P76): its Cross,
    /// riding or over the pause, claims a free seat, the way the join screen takes one.
    pub fn step_idle(&mut self, device: usize, p: Press) -> Act {
        let over = matches!(self.stack.as_slice(), [] | [Screen::Pause]) && self.leaving.is_none();
        if !(self.seats_open && over && p.ok) || self.ride_devices().contains(&device) {
            return Act::Stay;
        }
        if self.riders >= join::IN_RIDE {
            self.note = Some(format!("The ride is full: {} players.", join::IN_RIDE));
            return Act::Stay;
        }
        Act::Join(device)
    }

    /// Whether the ride says how to join it (P171): its seats are open, one is free, and a controller
    /// is plugged into a pad seat no player reads — `idle`, the pad seats whose controllers are in
    /// and playing. The cue is gone once that controller is seated, or the ride is full.
    pub fn join_cue(&self, idle: &[usize]) -> bool {
        let riding = self.ride_devices();
        self.seats_open && self.riders < join::IN_RIDE && idle.iter().any(|d| !riding.contains(d))
    }

    /// The shell's answer to [`Act::Join`] and [`Act::Leave`] (P76): the ride has its players so
    /// changed, each staying one's machine where it was, and the next ride of it (a Restart) starts
    /// with them, in the split screen's layout for the new count.
    pub fn reseated(&mut self, change: join::Change) {
        let devices = change.devices(&self.ride_devices());
        match change {
            join::Change::Join(_) => {
                let i = devices.len() - 1;
                if i < SEATS_MAX {
                    self.up[i] = Machine::Bike;
                    self.switchable[i] = false;
                }
                self.note = Some(format!("Player {} joined the ride.", i + 1));
            }
            join::Change::Leave(i) => {
                for k in i.min(SEATS_MAX)..SEATS_MAX {
                    self.up[k] = self.up.get(k + 1).copied().unwrap_or_default();
                    self.switchable[k] = self.switchable.get(k + 1).copied().unwrap_or(false);
                }
                // each later rider's own pause (P84) moves up a seat with them
                let gone = i.max(1);
                if gone < self.sides.len() {
                    if i == 0 {
                        self.swap_side(1);
                        self.player = self.player.saturating_sub(1);
                    }
                    self.sides.remove(gone);
                    for s in &mut self.sides[gone..] {
                        s.pauser -= 1;
                        s.player = s.player.saturating_sub(1);
                    }
                }
                self.note = Some(format!("Player {} left the ride.", i + 1));
            }
        }
        self.riders = devices.len();
        self.pauser = 0;
        let split = self.party.as_ref().map_or(self.settings.split, |p| p.split);
        self.party = Some(Party { devices, split });
    }

    /// The pause's seat count (N14): the opened room's once the network has said it, else the one it
    /// opens (or is opening) with, the lobby's last pick.
    fn pause_seats(&self) -> usize {
        match (&self.lobby.room, self.opening) {
            // not a lobby left over from before the ride
            (Some(room), Opening::Open) if room.riding && room.drop_in => room.seats.len(),
            _ => self.settings.lobby_seats,
        }
    }

    /// The pause's Seats row stepped `by` one, round from the cap to two as the lobby's is. The row is
    /// there once the ride is opened (P211), so the network is asked, and it refuses under the last
    /// taken seat (its reason comes back as the status).
    fn pause_seats_by(&mut self, by: i32) {
        let span = (openmx_net::SEATS - 1) as i32;
        let n = 2 + (self.pause_seats() as i32 - 2 + by).rem_euclid(span) as usize;
        if self.opening == Opening::Open {
            self.lobby.out.push(Cmd::Seats(n));
        }
        self.settings.lobby_seats = n;
        self.changed = true;
    }

    /// Open `screen` the way the pad would reach it, for `--screen`: over the main menu, or for the
    /// pause over a ride.
    pub fn open(&mut self, screen: Screen) {
        match screen {
            Screen::Splash => self.splash(),
            Screen::Main => self.main(),
            Screen::Pause => self.pause(),
            // Settings' tabs (P118), which were screens of their own: Audio over the pause, as
            // Settings there opens on it.
            Screen::Audio => {
                self.pause();
                self.open_settings(pages::TAB_AUDIO);
            }
            Screen::Controls | Screen::Camera | Screen::Graphics => {
                self.main();
                self.open_settings(match screen {
                    Screen::Controls => pages::TAB_CONTROLS,
                    Screen::Camera => pages::TAB_CAMERA,
                    _ => pages::TAB_GRAPHICS,
                });
            }
            Screen::Settings => {
                self.main();
                self.open_settings(self.tab(Screen::Settings));
            }
            Screen::Modifiers => {
                self.main();
                self.open_settings(pages::TAB_RIDE);
                self.open_modifiers();
            }
            Screen::Original => {
                self.main();
                self.open_settings(pages::TAB_RIDE);
                self.open_original();
            }
            Screen::Keyboard => {
                self.main();
                self.open_garage();
                self.type_for(Typing::Name);
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
                // The split screen is two at least: pad seat 1 joins beside the host.
                if self.join.views().len() < 2 {
                    self.join.press(&1, join::Press::Join);
                }
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
            Screen::Tracks => self.track_count(),
            Screen::Settings | Screen::Garage | Screen::Modifiers | Screen::Mods | Screen::Original => self.page_of(screen).len().max(1),
            Screen::Controls | Screen::Audio | Screen::Camera | Screen::Graphics => 1,
            Screen::Keyboard => KEY_ROWS.len() + 1,
            Screen::Pause => self.pause_layout().len(),
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

    fn type_for(&mut self, purpose: Typing) {
        let garage = self.screen() == Some(Screen::Garage);
        let seat = self.player.min(self.garages.len().saturating_sub(1));
        self.typing = match purpose {
            Typing::Name if garage => match self.rider_name(seat).as_str() {
                "None" => String::new(),
                n => n.to_string(),
            },
            Typing::Name => self.settings.name.clone(),
            Typing::Number if garage => self.garages.get(seat).map(|g| g.pick.look.number.clone()).unwrap_or_default(),
            _ => String::new(),
        };
        self.typing_for = purpose;
        self.key = (0, 0);
        self.push(Screen::Keyboard);
    }

    /// One frame of a ride's menu, with every rider's press, 1P's first (P41, P84): each rider's
    /// Start opens their own pause, which hears only their pad — their switch row, their pages — while
    /// every other rider rides on and works their own. Off a ride (the front end's screens) only 1P's
    /// press counts. A frame in which two riders ask something hands the first out now and the rest
    /// in the frames after; one that leaves the ride (Restart, Main Menu, Quit, Open to Online)
    /// closes every pause.
    pub fn step_seats(&mut self, presses: &[Press]) -> Act {
        let first = presses.first().copied().unwrap_or_default();
        let in_ride = self.stack.first().is_none_or(|s| *s == Screen::Pause);
        let mut acts = std::mem::take(&mut self.later);
        acts.push(self.step(first));
        if in_ride {
            // The menu's frame count is 1P's step's, whoever else stepped.
            let tick = self.tick;
            for (seat, &p) in presses.iter().enumerate().take(SEATS_MAX).skip(1) {
                if acts.iter().any(Act::leaves) {
                    break;
                }
                acts.push(self.with_seat(seat, |m| m.step(p)));
            }
            self.tick = tick;
        }
        acts.retain(|a| *a != Act::Stay);
        if let Some(leave) = acts.iter().position(Act::leaves) {
            let act = acts.swap_remove(leave);
            self.close_sides();
            match act {
                Act::MainMenu => self.main(),
                Act::Restart | Act::OpenOnline => self.stack.clear(),
                _ => {}
            }
            return act;
        }
        if acts.is_empty() {
            return Act::Stay;
        }
        let act = acts.remove(0);
        self.later = acts;
        act
    }

    /// The seat whose press drives 1P's side of the menu under [`Menu::step_seats`]: 1P's own, the
    /// keyboard with it, since every other rider's pause is theirs (P84).
    pub fn driver(&self) -> usize {
        0
    }

    /// One frame of one device's presses, the keyboard's on the splash, and the sound it made
    /// (P65).
    pub fn step(&mut self, p: Press) -> Act {
        self.cued(p, |m| m.walk(p))
    }

    /// `us` microseconds have passed on the shell's clock since the last call (P165): the menu's
    /// timers ([`TICK_HZ`]) move by them at the next step, and the steps after it in the same frame
    /// — a slow frame's further presses ([`Readers::more`]) — move them no further. From the first
    /// call on the menu keeps this time and no longer a tick a step.
    pub fn elapse(&mut self, us: u64) {
        let p = &mut self.pace;
        p.timed = true;
        p.carry += us * TICK_HZ;
        p.due = p.due.saturating_add((p.carry / 1_000_000) as u32);
        p.carry %= 1_000_000;
    }

    /// [`Menu::elapse`] up to `now` on the shell's clock: the time since the last call, a quarter of
    /// a second at most — a ride or a load in between is not time the menu's timers missed.
    pub fn elapse_to(&mut self, now: u64) {
        let since = self.pace.at.replace(now).map_or(0, |at| now.saturating_sub(at));
        self.elapse(since.min(ELAPSE_MAX_US));
    }

    /// The ticks passed since the last step took them: what [`Menu::elapse`] has said, or one for a
    /// menu nobody times. The loading screen, which steps nothing, moves its tick by these too.
    pub fn take_ticks(&mut self) -> u32 {
        if self.pace.timed { std::mem::take(&mut self.pace.due) } else { 1 }
    }

    /// [`Menu::step`]'s frame.
    fn walk(&mut self, p: Press) -> Act {
        if self.screen() == Some(Screen::Splash) {
            return self.splash_step(&[(Dev::Keys, p)]);
        }
        self.tick = self.tick.wrapping_add(self.pace.step);
        let Some(screen) = self.screen() else {
            // In a ride the only thing the menu hears is Start, which opens this rider's pause.
            if p.start {
                self.open_pause();
            }
            return Act::Stay;
        };
        if screen == Screen::Keyboard {
            return self.keyboard(p);
        }
        if Menu::is_page(screen) {
            self.walk_page(screen, p);
            return Act::Stay;
        }
        if screen == Screen::Join {
            self.walk_join(0, p);
            return Act::Stay;
        }
        if screen == Screen::Pause && self.leaving.is_some() {
            return self.confirm(p);
        }
        let rows = self.rows(screen);
        // The lobby's and the list's rows come and go with what the network says.
        let at = self.cursor(screen).min(rows - 1);
        self.cursor[screen.index()] = at;
        // Up and down walk the rows, wrapping; on Free Ride they walk the lit category's tracks, and
        // left and right step the categories (user, 2026-10-23).
        let (prev, next) = (p.up, p.down);
        if screen == Screen::Tracks && (prev || next || p.left || p.right) {
            let groups = self.track_groups();
            let g = self.track_filter_group();
            let to = if prev || next {
                let r = &groups[g].1;
                let n = r.len();
                let at = if r.contains(&at) { at } else { r.start };
                r.start + (at - r.start + if prev { n - 1 } else { 1 }) % n
            } else {
                // The next filter (MD11), kept in the settings, onto the ride's track where it has
                // it, else its first.
                let n = groups.len();
                let name = &groups[if p.left { (g + n - 1) % n } else { (g + 1) % n }].0;
                let filter = if name == ALL_GAMES { String::new() } else { name.clone() };
                if self.settings.track_filter != filter {
                    self.settings.track_filter = filter;
                    self.changed = true;
                }
                self.track_index()
            };
            self.cursor[screen.index()] = to;
            self.tick = 0;
            return Act::Stay;
        }
        if prev || next {
            let to = if prev { (at + rows - 1) % rows } else { (at + 1) % rows };
            self.cursor[screen.index()] = to;
            return Act::Stay;
        }
        match screen {
            // Back on the main menu is the splash again, 1P free for the next press (opentennis's
            // B533); Quit is a row, and Back on the splash asks.
            Screen::Main if p.back => self.splash(),
            Screen::Main => match (p.ok || p.start, p.back, at) {
                (true, _, MAIN_FREE) => {
                    if self.party.take().is_some() {
                        self.riders = self.free_riders;
                    }
                    self.cursor[Screen::Tracks.index()] = self.track_index();
                    self.tick = 0;
                    self.push(Screen::Tracks);
                }
                (true, _, MAIN_LOCAL) => self.open_join(),
                (true, _, MAIN_ONLINE) => self.push(Screen::Online),
                (true, _, MAIN_GARAGE) => self.open_garage(),
                (true, _, MAIN_MODS) => {
                    self.push(Screen::Mods);
                    self.cursor[Screen::Mods.index()] = 0;
                }
                (true, _, MAIN_SETTINGS) => self.open_settings(self.tab(Screen::Settings)),
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
                    self.cursor[Screen::Tracks.index()] = self.track_index();
                    self.tick = 0;
                    self.push(Screen::Tracks);
                }
            }
            Screen::Tracks => {
                // Triangle is the strip's Wrecks; it is also Back, so it goes first.
                if p.triangle {
                    self.settings.wrecks = !self.settings.wrecks;
                    self.changed = true;
                    return Act::Stay;
                }
                if p.ok || p.start {
                    let track = self.track_at(at).0.to_string();
                    if self.settings.track != track {
                        self.settings.track = track.clone();
                        self.changed = true;
                    }
                    self.ride_stack = std::mem::take(&mut self.stack);
                    return Act::Ride(track);
                }
                if p.back {
                    self.pop();
                }
            }
            Screen::Pause => {
                if p.back || p.start {
                    self.stack.clear();
                    return Act::Resume;
                }
                // The pause's music player (P174): L1 back, Square pause or play, R1 skip.
                if let Some(c) = player::press(p) {
                    self.work_deck(c);
                    return Act::Stay;
                }
                let row = self.pause_layout().get(at).copied();
                if row == Some(PauseRow::Seats) {
                    if p.ok || p.left || p.right {
                        self.pause_seats_by(if p.left { -1 } else { 1 });
                    }
                    return Act::Stay;
                }
                if row == Some(PauseRow::Fixed(PAUSE_SWITCH)) && (p.left || p.right) {
                    let choices = self.choices(self.pauser);
                    if let Some(now) = self.switch_to().and_then(|m| choices.iter().position(|c| *c == m)) {
                        let n = choices.len();
                        self.switch_pick = Some(choices[if p.left { (now + n - 1) % n } else { (now + 1) % n }]);
                    }
                    return Act::Stay;
                }
                if p.ok && row == Some(PauseRow::Leave) && self.session {
                    self.leaving = Some((Leaving::Session, 1));
                    return Act::Stay;
                }
                if p.ok && row == Some(PauseRow::Leave) {
                    // The last player out leaves the ride, which asks first as Main Menu does.
                    if self.riders <= 1 {
                        self.leaving = Some((Leaving::Main, 1));
                        return Act::Stay;
                    }
                    self.stack.clear();
                    return Act::Leave(self.pauser);
                }
                if p.ok {
                    match row {
                        Some(PauseRow::Fixed(0)) => {
                            self.stack.clear();
                            return Act::Resume;
                        }
                        Some(PauseRow::Fixed(1)) => {
                            self.stack.clear();
                            return Act::Restart;
                        }
                        Some(PauseRow::Fixed(PAUSE_SWITCH)) => {
                            if let Some(to) = self.switch_to() {
                                self.stack.clear();
                                return Act::Switch(self.pauser, to);
                            }
                        }
                        Some(PauseRow::Fixed(PAUSE_SETTINGS)) => self.open_settings(self.tab(Screen::Settings)),
                        Some(PauseRow::Fixed(PAUSE_MODIFIERS)) => self.open_modifiers(),
                        // Leaving asks first, on Stay (P110).
                        Some(PauseRow::Fixed(PAUSE_MAIN)) => self.leaving = Some((Leaving::Main, 1)),
                        Some(PauseRow::Online) if self.opening == Opening::Can => {
                            self.stack.clear();
                            return Act::OpenOnline;
                        }
                        Some(PauseRow::Fixed(_)) => self.leaving = Some((Leaving::Quit, 1)),
                        _ => {}
                    }
                }
            }
            // Settings' tabs now; never on the stack.
            Screen::Controls | Screen::Audio | Screen::Camera | Screen::Graphics => self.pop(),
            Screen::Online => self.online(p, at),
            Screen::Games => self.games(p, at),
            Screen::Lobby => self.in_lobby(p, at),
            Screen::Keyboard | Screen::Splash | Screen::Join | Screen::Settings | Screen::Garage | Screen::Modifiers | Screen::Mods | Screen::Original => unreachable!(),
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
                        Typing::Name | Typing::Preset | Typing::Camera => word_start,
                        Typing::Code => true,
                        Typing::Chat | Typing::Number => false,
                    };
                    // A number takes the digits alone.
                    if self.typing_for != Typing::Number || c.is_ascii_digit() {
                        self.typing.push(if upper { c.to_ascii_uppercase() } else { c });
                    }
                }
            } else {
                match col {
                    0 => {
                        if !self.typing.is_empty() && self.typing.chars().count() < max && self.typing_for != Typing::Number {
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
            Typing::Preset => {
                let name = clean_name(&self.typing);
                self.pop();
                if !name.is_empty() {
                    self.save_preset(&name);
                }
            }
            Typing::Camera => {
                let name = clean_name(&self.typing);
                self.pop();
                if !name.is_empty() {
                    self.save_camera(&name);
                }
            }
            Typing::Number => {
                let number = roster::clean_number(&self.typing);
                self.pop();
                let seat = self.player.min(self.garages.len().saturating_sub(1));
                if let Some(g) = self.garages.get_mut(seat) {
                    g.pick.look.number = number;
                }
            }
        }
    }

    fn finish_name(&mut self) {
        // The Garage's Name row (P118): the seat's look, and 1P's is the settings' name too.
        if self.stack.len() > 1 && self.stack[self.stack.len() - 2] == Screen::Garage {
            let name = roster::clean_rider_name(&self.typing);
            self.pop();
            let seat = self.player.min(self.garages.len().saturating_sub(1));
            if let Some(g) = self.garages.get_mut(seat) {
                g.pick.look.name = name.clone();
            }
            let name = clean_name(&name);
            if seat == 0 && !name.is_empty() && name != self.settings.name {
                self.settings.name = name;
                self.changed = true;
            }
            return;
        }
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

    /// The keyboard's key under the cursor: a row of [`KEY_ROWS`], or the commands' row under them.
    pub fn key_at(&self) -> (usize, usize) {
        self.key
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
        let list = self.online_tracks();
        let l = &mut self.lobby;
        match item {
            Item::Track if p.ok || side => {
                if let Some(room) = &l.room {
                    // The lit filter's tracks (MD11); a room on one it does not list steps onto its ends.
                    let i = list.iter().position(|t| *t == room.track).map_or(if by > 0 { -1 } else { 0 }, |i| i as i32);
                    let n = list.len() as i32;
                    let track = list[(i + by).rem_euclid(n) as usize].to_string();
                    l.out.push(Cmd::Ride { track, wrecks: room.wrecks, biplane: false });
                }
            }
            Item::Wrecks if p.ok || side => {
                if let Some(room) = &l.room {
                    l.out.push(Cmd::Ride { track: room.track.clone(), wrecks: !room.wrecks, biplane: false });
                }
            }
            // One more or one fewer, round from the cap to two: never under the last seat taken,
            // which the network says rather than does.
            Item::Seats if p.ok || side => {
                if let Some(room) = &l.room {
                    let span = (openmx_net::SEATS - 1) as i32;
                    let n = 2 + (room.seats.len() as i32 - 2 + by).rem_euclid(span) as usize;
                    l.out.push(Cmd::Seats(n));
                    self.settings.lobby_seats = n;
                    self.changed = true;
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
            Item::Modifiers if p.ok => self.open_modifiers(),
            _ => {}
        }
    }

    /// This machine's player's own modifiers into its lobby pick, and on the host every seat's
    /// gathered into the room's (P134): the shell's every frame. A guest's pick carries its own
    /// knobs; the host's room takes a seat's when its pick's change, and the knobs the host set
    /// through All Players every seat's.
    pub fn lobby_mods(&mut self) {
        let mods = self.settings.modifiers;
        self.lobby.set_mods(&mods);
        if !self.lobby.hosting() || self.lobby.room.as_ref().is_none_or(|r| r.riding) || self.lobby.start.is_some() {
            return;
        }
        let all = self.take_mods_all();
        if let Some(text) = self.lobby.gather(&mods, &all) {
            self.lobby.out.push(Cmd::Modifiers(text));
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
                l.drop_in = false;
                l.status.clear();
                self.stack.clear();
                return Act::Online;
            }
            // into a ride under way (N10): the same ride from the lobby, its state from the host
            Event::DropIn(room, me) => {
                l.me = me.clone();
                l.room = Some(room.clone());
                l.start = Some((room, me));
                l.drop_in = true;
                l.status.clear();
                self.stack.clear();
                return Act::Online;
            }
            // the host's ride takes the peer on (`online.rs`); the lobby has nothing to show for it
            Event::Arrived { .. } => {}
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
            // the host went and the room is another peer's (N11), maybe this machine's
            Event::HandedOver(host) => {
                l.handed |= host == l.me;
                let name = l.room.as_ref().and_then(|r| r.names.get(&host).cloned()).unwrap_or_else(|| host.chars().take(6).collect());
                l.status = if host == l.me { "The host left: you host the game now.".into() } else { format!("The host left: {name} hosts the game now.") };
            }
        }
        Act::Stay
    }

    /// Back from an online ride: the lobby if this machine is still in one, else the online menu.
    pub fn back_from_online(&mut self) {
        self.lobby.start = None;
        self.lobby.drop_in = false;
        self.stack = vec![Screen::Main, Screen::Online];
        if self.lobby.room.is_some() {
            self.stack.push(Screen::Lobby);
        }
    }
}

// ---- online (N3) ---------------------------------------------------------------------------------

/// The online menu's rows; a last one switches the network.
const ONLINE: [&str; 4] = ["Quickplay", "Host", "Find Games", "Join by Code"];

/// The players a lobby this machine opens has seats for, until its host's Seats row says otherwise
/// ([`Settings::lobby_seats`]).
pub const LOBBY_SEATS: usize = 4;

/// The seats the lobby lists a line each; a bigger room's go in columns of [`SEAT_COLUMN`] (N12).
const SEAT_LINES: usize = 6;
const SEAT_COLUMN: usize = 8;

/// The chat lines the lobby keeps.
const CHAT_LINES: usize = 5;

/// What the keyboard is typing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Typing {
    Name,
    Code,
    Chat,
    /// A modifier preset's name (P55).
    Preset,
    /// A rider's number, digits only (P118).
    Number,
    /// A camera preset of the player's own (P163).
    Camera,
}

impl Typing {
    fn max(self) -> usize {
        match self {
            Typing::Name => NAME_MAX,
            // A code is four; a little over lets a slip be seen and deleted.
            Typing::Code => 8,
            Typing::Chat => 40,
            Typing::Preset | Typing::Camera => NAME_MAX,
            Typing::Number => roster::NUMBER_MAX,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Typing::Name => "Rider Name",
            Typing::Code => "Room Code",
            Typing::Chat => "Chat",
            Typing::Preset => "Preset Name",
            Typing::Camera => "Camera Name",
            Typing::Number => "Number",
        }
    }
}

/// A row of the lobby.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Track,
    Wrecks,
    /// The host's seat count (N12), 2 to [`openmx_net::SEATS`].
    Seats,
    Bike,
    Private,
    Chat,
    Start,
    Ready,
    /// The player's own Game Modifiers page (P134); the host's sets every seat's too.
    Modifiers,
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
            Item::Seats => format!("Seats  {}", room.map_or(0, |r| r.seats.len())),
            Item::Bike => format!("Bike  {}", l.bike_label(&l.pick.bike)),
            Item::Private => format!("Private  {}", on(room.is_some_and(|r| r.private))),
            Item::Chat => "Chat".into(),
            Item::Start => "Start".into(),
            Item::Ready => if l.ready { "Not Ready" } else { "Ready" }.into(),
            Item::Modifiers => "Game Modifiers".into(),
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
    /// [`Lobby::start`] is a ride under way this machine drops in on (N10), not one starting.
    pub drop_in: bool,
    /// [`Lobby::pick`]'s controller preset changed in a lobby that could not hear of it then (a
    /// ride under way): it goes out once the lobby is open again (N13).
    pub repick: bool,
    /// The host left and handed its room to this machine (N11), which hosts it under its own name.
    pub handed: bool,
    /// The host's: each seat's (peer, pick modifiers) last taken into the room's (P134).
    mods_taken: Vec<(String, String)>,
    /// The host's: the room's modifiers as last sent, before the room comes back with them.
    mods_sent: Option<String>,
}

impl Lobby {
    /// The rows: the host's, a guest's, or only Leave while there is no lobby yet.
    pub fn items(&self) -> Vec<Item> {
        match &self.room {
            None => vec![Item::Leave],
            Some(_) if self.hosting() => {
                vec![
                    Item::Track,
                    Item::Wrecks,
                    Item::Bike,
                    Item::Private,
                    Item::Seats,
                    Item::Chat,
                    Item::Start,
                    Item::Modifiers,
                    Item::Leave,
                ]
            }
            Some(_) => vec![Item::Bike, Item::Chat, Item::Ready, Item::Modifiers, Item::Leave],
        }
    }

    pub fn hosting(&self) -> bool {
        self.me == HOST || self.handed
    }

    /// A lobby for this machine's player to open: the last free-ride track, the settings' wrecks.
    fn room_for(&self, settings: &Settings) -> Room {
        // A track mod is not ridden online until MD6: a lobby opens on the disc's tracks only.
        let track = if TRACKS.iter().any(|t| t.0 == settings.track) { settings.track.as_str() } else { TRACKS[0].0 };
        let mut room = Room::new(&settings.name, track, settings.lobby_seats);
        room.wrecks = settings.wrecks;
        room.air_wrecks = settings.air_wrecks;
        room.backwards_bail = settings.backwards_bail;
        room.take(HOST, std::slice::from_ref(&self.pick));
        // The host's own on its seat and the world's (P134); a guest's seat takes theirs as they come.
        let mine: Vec<usize> = room.seats.iter().enumerate().filter(|(_, s)| s.as_ref().is_some_and(|s| s.peer == HOST)).map(|(i, _)| i).collect();
        room.modifiers = online_mods::host_set(&Mods::original(), &settings.modifiers, &mine, &[]).text();
        room
    }

    fn begin(&mut self, status: &str) {
        self.room = None;
        self.mods_taken.clear();
        self.mods_sent = None;
        self.me.clear();
        self.handed = false;
        self.ready = false;
        self.code.clear();
        self.chat.clear();
        self.status = status.into();
    }

    /// This machine's player's controller preset (P79) as its pick's (N13), which every machine's
    /// ride reads the player's seat through: the shell's every frame. A lobby not riding hears of a
    /// change; a ride under way keeps the preset it started on, as the original builds its table
    /// once a ride, and its lobby hears of it once it is back. With no lobby the next one's Host or
    /// Join carries the pick.
    pub fn preset(&mut self, preset: usize) {
        let preset = preset.min(PRESETS - 1) as u8;
        if self.pick.preset != preset {
            self.pick.preset = preset;
            self.repick = self.room.is_some();
        }
        if self.repick && self.room.as_ref().is_some_and(|r| !r.riding) && self.start.is_none() {
            self.out.push(Cmd::Picks(vec![self.pick.clone()]));
            self.repick = false;
        }
    }

    /// This machine's player's saved markers on the room's track as its pick's (P99,
    /// `saved_markers::to_pick`), which every machine starts the player's seat on: the shell's every
    /// frame. A change goes out with [`Lobby::preset`]'s, on the same terms.
    pub fn set_markers(&mut self, text: String) {
        if self.pick.markers != text {
            self.pick.markers = text;
            self.repick = self.room.is_some();
        }
    }

    /// This machine's player's own modifiers (P134) as its pick's: its 1P's own knobs, and the
    /// host's the world's and the biplane's too. A change goes out with [`Lobby::preset`]'s.
    pub fn set_mods(&mut self, own: &Mods) {
        let text = online_mods::own_set(own, 0, self.hosting()).text();
        if self.pick.modifiers != text {
            self.pick.modifiers = text;
            self.repick = self.room.is_some();
        }
    }

    /// The host's room modifiers with every seat's own (P134): a seat's pick's when they changed
    /// since last taken, the host's seats and the world's off its own set `host`, and the knobs
    /// `all` every seat's. `Some` when that differs from what the room has.
    pub fn gather(&mut self, host: &Mods, all: &[Knob]) -> Option<String> {
        let room = self.room.as_ref()?;
        let was = self.mods_sent.clone().unwrap_or_else(|| room.modifiers.clone());
        let mut set = Mods::parse(&was).unwrap_or_default();
        self.mods_taken.resize(room.seats.len(), Default::default());
        let mut mine = Vec::new();
        for (i, seat) in room.seats.iter().enumerate() {
            let Some(seat) = seat else {
                self.mods_taken[i] = Default::default();
                continue;
            };
            if seat.peer == HOST {
                mine.push(i);
                continue;
            }
            let now = (seat.peer.clone(), seat.pick.modifiers.clone());
            if self.mods_taken[i] != now && online_mods::own_row(i, room.seats.len()) {
                if let Ok(own) = Mods::parse(&now.1) {
                    set = online_mods::with_own(&set, &[i], &own);
                }
                self.mods_taken[i] = now;
            }
        }
        let text = online_mods::host_set(&set, host, &mine, all).text();
        (text != was).then(|| {
            self.mods_sent = Some(text.clone());
            text
        })
    }

    /// The machine this machine's player starts a ride on (N18), the ride setup's pick, as its pick's
    /// word (`biplane`, or empty for the bike): every machine starts the player's seat on it. A
    /// change goes out with [`Lobby::preset`]'s, on the same terms.
    pub fn set_machine(&mut self, word: &str) {
        let word = if word == "bike" { "" } else { word };
        if self.pick.machine != word {
            self.pick.machine = word.to_string();
            self.repick = self.room.is_some();
        }
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
    /// A seat in a room of more than [`SEAT_LINES`] (N12): its number, the player's name cut short,
    /// and `H` the host, `R` ready, `-` not yet; an open one says so.
    fn seat_short(&self, room: &Room, i: usize) -> String {
        let Some(seat) = room.seats.get(i).and_then(|s| s.as_ref()) else {
            return format!("{}  open", i + 1);
        };
        let name = room.names.get(&seat.peer).cloned().unwrap_or_else(|| {
            if seat.peer == HOST { room.name.clone() } else { seat.peer.clone() }
        });
        let mark = if seat.peer == HOST {
            'H'
        } else if room.ready.contains(&seat.peer) {
            'R'
        } else {
            '-'
        };
        format!("{} {} {mark}", i + 1, name.chars().take(7).collect::<String>())
    }

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
        // The rider's own name and number from their look (P86), before the gear set they wear.
        let look = roster::Look::parse(&seat.pick.look);
        let rider = match (look.number.is_empty(), look.name.is_empty()) {
            (true, true) => String::new(),
            (true, false) => format!("{}  ", look.name),
            (false, true) => format!("#{}  ", look.number),
            (false, false) => format!("#{} {}  ", look.number, look.name),
        };
        let mods = Self::seat_mods(room, i);
        format!("{}  {name}  {rider}{}  {}  {state}{ping}{mods}", i + 1, seat.pick.rider, self.bike_label(&seat.pick.bike))
    }

    /// A seat's own modifiers as the room rides them (P134), for its line: the first off the
    /// original and how many more, or nothing on the original's.
    pub fn seat_mods(room: &Room, i: usize) -> String {
        let Ok(set) = Mods::parse(&room.modifiers) else { return String::new() };
        let off: Vec<Knob> = Knob::ALL.into_iter().filter(|&k| online_mods::seat_own(k) && set.get(i, k) != game_sim::modifiers::ORIGINAL).collect();
        match off.as_slice() {
            [] => String::new(),
            [k] => format!("  {} {}%", k.label(), set.get(i, *k)),
            [k, rest @ ..] => format!("  {} {}% +{}", k.label(), set.get(i, *k), rest.len()),
        }
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

/// The disc's textures the menus draw, by their name under `ui\\tex\\`, sorted.
const TEXTURES: [&str; 4] = ["background", "frontend_background_radial", "mainmenu_logo", "mainmenu_logo_shadow"];

// ---- the track mods' place in the pick (MD2) -------------------------------------------------------

impl Menu {
    /// The tracks the pick lists: the disc's [`TRACKS`] and [`SPLICED`], then the mods'.
    pub fn track_count(&self) -> usize {
        DISC_PICK + self.shown_mods().count()
    }

    /// The mod tracks the pick lists: the mods' less the packs switched off (P118).
    fn shown_mods(&self) -> impl Iterator<Item = &(String, String)> {
        self.mod_tracks.iter().filter(|(id, _)| !self.settings.mods_off.iter().any(|p| p == pack_of(id)))
    }

    /// The pick's track `at`, `(id, name)`: a disc track's archive and town, the spliced maps', or a
    /// mod's id and name.
    pub fn track_at(&self, at: usize) -> (&str, &str) {
        match TRACKS.get(at) {
            Some(&(id, name)) => (id, name),
            None if at == TRACKS.len() => SPLICED,
            None => self
                .shown_mods()
                .nth(at - DISC_PICK)
                .map_or(TRACKS[0], |(id, name)| (id.as_str(), name.as_str())),
        }
    }

    /// The pick's index of [`Settings::track`] in the lit filter, the filter's first track where it
    /// does not list it.
    pub fn track_index(&self) -> usize {
        let r = self.track_groups()[self.track_filter_group()].1.clone();
        r.clone().find(|&i| self.track_at(i).0 == self.settings.track).unwrap_or(r.start)
    }

    /// The pick's filters by source game (MD11), which Left and Right step between, each its name
    /// and its tracks' range: with mods listed, All Games (every track) first, then MX Unleashed (the
    /// disc's towns, and a mod made from MX Unleashed after them), the worlds (the combined and ultra
    /// maps) right after (user, 2026-10-23), then each other source game that has a track, by name.
    /// With none, the disc's alone.
    pub fn track_groups(&self) -> Vec<(String, Range<usize>)> {
        let mut out = vec![(DISC_GAME.to_string(), 0..DISC_PICK)];
        for (i, (id, _)) in self.shown_mods().enumerate() {
            let at = DISC_PICK + i;
            let game = self.track_game(id);
            match out.last_mut() {
                Some((name, r)) if *name == game => r.end = at + 1,
                _ => out.push((game, at..at + 1)),
            }
        }
        if self.track_count() > DISC_PICK {
            out.insert(0, (ALL_GAMES.to_string(), 0..self.track_count()));
        }
        out
    }

    /// Which of [`Menu::track_groups`] the pick's track `at` is in: the lit filter where it has it,
    /// else the first that does.
    pub fn track_group(&self, at: usize) -> usize {
        let groups = self.track_groups();
        let lit = self.track_filter_group();
        if groups[lit].1.contains(&at) {
            return lit;
        }
        groups.iter().position(|(_, r)| r.contains(&at)).unwrap_or(0)
    }

    /// Which of [`Menu::track_groups`] [`Settings::track_filter`] is, All Games (the first) where the
    /// pick does not list its game.
    pub fn track_filter_group(&self) -> usize {
        let want = if self.settings.track_filter.is_empty() { ALL_GAMES } else { self.settings.track_filter.as_str() };
        self.track_groups().iter().position(|(name, _)| name == want).unwrap_or(0)
    }

    /// The game a track of the pick is from (MD11): MX Unleashed for the disc's and their splice,
    /// Worlds for a combined map, else its `mod.json`'s source game, or its pack's name where the
    /// shell gave none.
    pub fn track_game(&self, id: &str) -> String {
        if TRACKS.iter().any(|t| t.0 == id) || id == SPLICED.0 {
            return DISC_GAME.into();
        }
        if pack_of(id) == WORLDS {
            return pack_name(WORLDS);
        }
        match self.mod_games.iter().find(|(i, _)| i == id) {
            Some((_, game)) if !game.trim().is_empty() && game != ALL_GAMES => game.trim().to_string(),
            _ => pack_name(pack_of(id)),
        }
    }

    /// The tracks an online room's pick steps through (MD11): the lit filter's disc tracks, since a
    /// mod is not ridden online until MD6, or every disc track where the filter has none.
    pub fn online_tracks(&self) -> Vec<&'static str> {
        let r = self.track_groups()[self.track_filter_group()].1.clone();
        let disc: Vec<&'static str> = TRACKS.iter().enumerate().filter(|(i, _)| r.contains(i)).map(|(_, t)| t.0).collect();
        if disc.is_empty() { TRACKS.iter().map(|t| t.0).collect() } else { disc }
    }

    /// The mods the pick lists after the disc's (MD2), the pick's cursor kept on its track. They
    /// are kept by source game ([`Menu::track_groups`]' order), so each game is one of the filters.
    pub fn set_mod_tracks(&mut self, tracks: Vec<(String, String)>) {
        if !tracks.is_empty() {
            self.mods_seen = tracks.clone();
        }
        self.mod_tracks = tracks;
        self.arrange_mods();
    }

    /// The music packs in the mods folder (P122), `(id, name, songs)`, for the Mods page. An empty
    /// list (the folder off) keeps the last one listed, as the track packs are.
    pub fn set_music_packs(&mut self, packs: Vec<(String, String, usize)>) {
        if !packs.is_empty() {
            self.music_seen = packs;
        }
    }

    /// Each mod track's source game (MD11), `(id, game)`, as its `mod.json` says.
    pub fn set_mod_games(&mut self, games: Vec<(String, String)>) {
        self.mod_games = games;
        self.arrange_mods();
    }

    /// The mod tracks in the filters' order, MX Unleashed's first, the worlds next, then each game
    /// by name, each game's in the order the shell listed them; the cursor back on the ride's track.
    fn arrange_mods(&mut self) {
        let mut tracks = std::mem::take(&mut self.mod_tracks);
        tracks.sort_by_cached_key(|(id, _)| {
            let game = self.track_game(id);
            (if game == DISC_GAME { 0 } else if pack_of(id) == WORLDS { 1 } else { 2 }, game)
        });
        self.mod_tracks = tracks;
        self.cursor[Screen::Tracks.index()] = self.track_index();
    }
}

/// The track pick's filter of every track (MD11), which an empty [`Settings::track_filter`] is.
pub const ALL_GAMES: &str = "All Games";
/// The track pick's filter of the disc's own tracks (MD11).
pub const DISC_GAME: &str = "MX Unleashed";

/// The pack prefix of the world mods (MD8), whose category the pick lists right after the disc's.
pub const WORLDS: &str = "world";

/// The stills per track in `UI\trackpreview.res`.
const STILLS: usize = 5;
/// Frames each still shows for on the track pick.
const STILL_FRAMES: u32 = 150;

/// Everything the menus draw with: the disc's backdrop, logo and stills; the type and the shapes
/// are our own ([`ink`]).
pub struct Art {
    textures: HashMap<&'static str, Texture>,
    /// Each free-ride track's stills, in [`TRACKS`] order.
    stills: Vec<Vec<Texture>>,
    /// The mod tracks' pictures (P70), by id: a mod's own, or the still the shell took of its
    /// spawn. Counted after the stills in [`Art::images`].
    mod_pictures: Vec<(String, Texture)>,
}

/// How far a pill reaches past each end of its label, in the stage's ems (the concept's
/// `.95em` of a 1.7em item).
pub const PILL_PAD: f32 = 1.615;

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
    /// A track pick's card (P70): its box, its picture's box, and which of [`Art::images`] the
    /// picture is, `None` for a track with no picture yet.
    Card { id: String, card: Bounds, picture: Bounds, image: Option<usize>, lit: bool },
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
        Ok(Art { textures, stills, mod_pictures: Vec::new() })
    }

    /// A mod track's picture (P70), in place of the one it had. The GPU has to be given
    /// [`Art::images`] again after it.
    pub fn set_mod_picture(&mut self, id: &str, picture: Texture) {
        match self.mod_pictures.iter_mut().find(|(i, _)| i == id) {
            Some(p) => p.1 = picture,
            None => self.mod_pictures.push((id.to_string(), picture)),
        }
    }

    /// Which of [`Art::images`] a track's card shows as its `i`th picture: a disc track's still
    /// (turning through its five), the spliced maps' (each map's in turn, west to east), or a mod's
    /// picture; `None` where there is none.
    pub fn track_image(&self, id: &str, i: usize) -> Option<usize> {
        let first = TEXTURES.len() + 1;
        if id == SPLICED.0 {
            let n = TRACKS.len();
            return (0..n).find_map(|k| self.track_image(TRACKS[(i + k) % n].0, (i + k) / n));
        }
        if let Some(track) = TRACKS.iter().position(|t| t.0 == id) {
            let n = self.stills[track].len();
            let before: usize = self.stills[..track].iter().map(Vec::len).sum();
            return (n > 0).then(|| first + before + i % n);
        }
        let stills: usize = self.stills.iter().map(Vec::len).sum();
        self.mod_pictures.iter().position(|(m, _)| m == id).map(|at| first + stills + at)
    }

    /// Draw the menu's screen into `frame`. In a ride the frame already holds the ride, which the
    /// pause darkens rather than covers; otherwise the menu draws its own background.
    pub fn draw(&self, menu: &Menu, frame: &mut Frame) {
        self.parts(menu, frame);
    }

    /// [`Art::draw`], saying where the logo, the pills and the labels went.
    pub fn parts(&self, menu: &Menu, frame: &mut Frame) -> Vec<Part> {
        let mut out = Vec::new();
        let parts = self.layout(menu, (frame.width, frame.height), 0, &mut out);
        ink::raster(frame, &out, &self.images());
        parts
    }

    /// Every picture [`Art::sprites`] counts into, in its order: the textures in [`TEXTURES`]'
    /// order, the type's atlas, the stills, the mods' pictures. The GPU takes them once
    /// ([`game_render::gpu::Renderer::set_images`]).
    pub fn images(&self) -> Vec<&Texture> {
        let mut all: Vec<&Texture> = TEXTURES.iter().map(|n| &self.textures[n]).collect();
        all.push(&ink::ink().atlas);
        all.extend(self.stills.iter().flatten());
        all.extend(self.mod_pictures.iter().map(|(_, t)| t));
        all
    }

    /// [`Art::draw`] as the GPU draws it (V30): the menu's screen over a `size` frame as sprites into
    /// `out`, whose pictures are [`Art::images`] counted from `base`. The same sprites the CPU
    /// rasters, so the two agree and a 4K menu is as sharp as its pixels.
    pub fn sprites(&self, menu: &Menu, size: (usize, usize), base: usize, out: &mut Vec<Sprite>) {
        self.layout(menu, size, base, out);
    }

    fn layout(&self, menu: &Menu, size: (usize, usize), base: usize, out: &mut Vec<Sprite>) -> Vec<Part> {
        let mut pen = screens::Pen::new(self, out, base, size);
        pen.screen(menu);
        pen.parts
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
