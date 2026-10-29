//! The ride HUD (P121): `docs/hud-concept.html` drawn as vector shapes and our own type over each
//! view, sharp at any size on both paths.
//!
//! [`crate::overlay`] draws the original's HUD out of the disc's textures and bitmap fonts, which
//! are 640x448 pictures stretched to the frame and soft at 4K. This draws the same pieces at the
//! same places of the disc's layouts — the score bar where `stuntscoreoverlays.lyt` has it, the
//! trick pop-up where the Stunt movies play, the speedometer and the preload meter at
//! `speedometer.lyt`'s, the radar where `freestyleradar` sits — but as [`Sprite`]s of
//! [`crate::menu::ink`]'s shapes and type: Saira for the words, DSEG7 for the digital
//! speed. The GPU draws the sprites as they are; the software path rasters them into the view's
//! frame by the same rules ([`crate::menu::ink::raster`]), so the two are one picture.
//!
//! A view's layout is the disc's 640x448 scaled to fit the view (the smaller of its two ratios,
//! times the HUD scale), each piece held to its own corner as the disc's layouts hold them, so a
//! split view's HUD is that view's, shrunk with it.
//!
//! What moves with time — the trick pop-up's pop, its play-out, the score's flash on a bank and the
//! camera toast — is a [`Tracker`] a view, fed the rider's score card each frame.

use crate::menu::Speedometer;
use crate::menu::ink::{self, Box4, Type, Weight};
use crate::overlay::{self, Activity, Blip, Gauges, Icon};
use crate::vehicle::Machine;
use game_render::gpu::{Shape, Sprite};
use game_render::{Frame, Texture};
use game_sim::markers::{HOLD_MS, MAX_SLOTS, Pick};
use game_sim::score::Score;
use std::f32::consts::{FRAC_PI_2, TAU};
use std::time::Duration;

use crate::clock::Instant;

/// The layout's em: the concept's, a view's height over 45, at the disc's 448.
const EM: f32 = 448.0 / 45.0;

/// The original's trick text and preload charge, 255,153,0.
pub const ORANGE: [u8; 4] = [255, 153, 0, 255];
/// Its score text, 200,200,200.
pub const GREY: [u8; 4] = [200, 200, 200, 255];
/// The pop-up's ink once a combo is banked, and once it is lost. The original plays both out the
/// same way: its HUD listener answers a combo's end — landed or not — by setting the Stunt movie's
/// `Mode` to 1 and playing it, which shrinks the orange text away; nothing turns it green or red
/// (research/oracle.txt, P121, and a filmed recording2 playback: a land, a two-trick land and a
/// bail all stay 255,153,0 to the last frame). So both are the orange it was, and what tells them
/// apart is the "2 Stunt Combo Total" a landed chain of two or more holds up a second first.
pub const LAND_COLOUR: [u8; 4] = ORANGE;
pub const BAIL_COLOUR: [u8; 4] = ORANGE;
const OUTLINE: [u8; 4] = [0x2a, 0x12, 0x00, 255];
const MULT: [u8; 4] = [0xff, 0xd8, 0xa0, 255];
const CHALK: [u8; 4] = [0xf5, 0xf0, 0xe8, 255];
const BONE: [u8; 4] = [0xd8, 0xcf, 0xc2, 255];
const DUST: [u8; 4] = [0x9c, 0x8f, 0x80, 255];
const INK: [u8; 4] = [0x05, 0x03, 0x02, 255];
const WHITE: [u8; 4] = [255; 4];
/// The marker hold's bar once it has fired.
const FIRED: [u8; 4] = [0x5e, 0xe0, 0x5e, 255];
const MAROON: ([u8; 4], [u8; 4]) = ([0xb3, 0x1a, 0x23, 255], [0x5a, 0x07, 0x0c, 255]);
/// The Circle's ring.
const CIRCLE: [u8; 4] = [0xe0, 0x47, 0x3f, 255];
/// The Cross button's blue, for the join cue (P171).
const CROSS: [u8; 4] = [0x7c, 0xb2, 0xe8, 255];

const fn shade(a: u8) -> [u8; 4] {
    [0, 0, 0, a]
}

// ---- where things go, in the disc's 640x448 -------------------------------------------------------

/// The score bar (left, top, width, height) and where its darker label end stops: the original's.
const SCORE_BAR: Box4 = [14.0, 25.0, 168.0, 20.5];
const SCORE_LABEL_END: f32 = 82.0;
const SCORE_NUMBER: f32 = 90.0;
/// The lines under it, the rider's tag and MODIFIED: their left and the first one's baseline.
const UNDER_SCORE: (f32, f32) = (31.0, 58.0);
/// The Stunt and Stunt Combo movies' place: the pop-up's middle, low centre.
pub const TRICK_AT: (f32, f32) = (320.0, 373.0);
/// The radar's middle (the original's), its radius — where the original's marks stop — and the
/// concept's units a layout pixel is (its R of 86 to this).
const RADAR: (f32, f32) = overlay::RADAR_AT;
const RADAR_R: f32 = overlay::RADAR_RADIUS / 2.0;
const RK: f32 = RADAR_R / 86.0;
/// The dial's pivot (the original's), and the concept's units a layout pixel is: its 54-long
/// needle is the original's 40.4.
const PIVOT: (f32, f32) = (536.0, 357.0);
const DK: f32 = 0.75;
/// The prompts' foot.
const PROMPT_FOOT: f32 = 448.0 - 1.6 * EM;
/// The top-right column (the marker menu, the camera toast): its right edge and top.
const TOP_RIGHT: (f32, f32) = (640.0 - 3.2 * EM, 2.6 * EM);

/// How long a trick's name pops in, how long a banked chain's total holds before it plays out, and
/// how long the play-out takes: by eye off the filmed original, a name grows to full in about four
/// frames, the total stands about a second, and an ending shrinks and fades in about eight.
const POP: f32 = 0.15;
const HOLD: f32 = 1.0;
const OUT: f32 = 0.25;
/// What a name pops in from, and what the play-out shrinks to, over the full size.
const POP_FROM: f32 = 0.5;
const OUT_TO: f32 = 0.2;
/// How long the score flashes on a bank.
const FLASH: f32 = 0.7;
/// How long the camera toast stays, and the fade at its end: the original's info text's three
/// seconds (P68, [`overlay::INFO_SECONDS`]).
const TOAST: f32 = overlay::INFO_SECONDS;
const TOAST_FADE: f32 = 0.25;

// ---- what a view shows --------------------------------------------------------------------------

/// Another rider on the radar: where (`x`, `z`), and whether the game drives them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dot {
    pub at: [f32; 2],
    pub ai: bool,
}

/// The marker menu's state while it is up (P88, P94, P106, P107).
#[derive(Clone, Debug, PartialEq)]
pub struct Panel {
    pub pick: Pick,
    /// Each slot, whether it holds a marker: five, or more off the + (P139).
    pub filled: Vec<bool>,
    /// The spawn has a start to ride to for the machine up (P132, `hud::MarkerBar::home`). The spawn
    /// is its own entry before the slots (P135), drawn an `S` in a pale ring set apart from them; with
    /// no start it is drawn faint.
    pub home: bool,
    /// The spawn's Down has just cleared the markers (P135, `hud::MarkerBar::cleared`).
    pub cleared: bool,
    /// The other players in the ride, by seat, and their names where the lineup has them.
    pub others: Vec<(usize, Option<String>)>,
    /// How long a Down or Up hold has been down, in ms ([`game_sim::markers::holding`]).
    pub holding: u32,
    /// The machine whose set is up.
    pub machine: String,
}

impl Panel {
    /// Whether the + follows the last slot (P139): every slot set and fewer than [`MAX_SLOTS`].
    pub fn more(&self) -> bool {
        self.filled.len() < MAX_SLOTS && self.filled.iter().all(|&f| f)
    }
}

/// A challenge's start prompt: the activity, its name, what it asks, and how far a hold to start
/// it is, 0 to 1. The freestyle event is the one the port starts (P155).
#[derive(Clone, Debug, PartialEq)]
pub struct Challenge {
    pub activity: Activity,
    pub name: String,
    pub goal: String,
    pub hold: f32,
}

/// What one view's HUD is drawn from.
#[derive(Clone, Debug)]
pub struct Shown<'a> {
    /// The rider's speed, feet a second.
    pub speed: f32,
    pub gauges: &'a Gauges,
    /// The rider's ground position and the way it faces (`x`, `z`).
    pub own: [f32; 2],
    pub forward: [f32; 2],
    /// The other riders.
    pub riders: &'a [Dot],
    /// What the radar marks besides them.
    pub blips: &'a [Blip],
    /// The world the radar spans ([`overlay::radar_span`]).
    pub span: f32,
    /// The rider's name and number under the score.
    pub tag: Option<String>,
    /// The machine the switch prompt offers.
    pub offer: Option<Machine>,
    pub challenge: Option<Challenge>,
    pub markers: Option<Panel>,
    /// The HUD scale (1 is the disc's size) and whether the radar is drawn.
    pub scale: f32,
    pub minimap: bool,
    /// The map the rider is on, named in the radar, and the map that lies each way, N E S W,
    /// named inside the rim by its compass letter (MD42, MD46, `hud::MapNames::around`).
    pub map: Option<&'a str>,
    pub around: [Option<&'a str>; 4],
    /// Whether this view says how to join the ride (P171): a seat is free and a controller idle.
    pub join: bool,
    /// The freestyle event while one is on (P155).
    pub event: Option<EventShown>,
}

/// The freestyle event as a view shows it (P155): [`EventShown::of`] off the ride's event and the
/// view's rider.
#[derive(Clone, Debug, PartialEq)]
pub struct EventShown {
    /// The track's target.
    pub target: i32,
    /// The countdown's number, 3 to 1, while it counts; 0 is its "GO!", up for the clock's first
    /// second.
    pub countdown: Option<u32>,
    /// Seconds left on the rider's clock while it runs.
    pub left: Option<f32>,
    /// The last-seconds warning is up.
    pub warning: bool,
    /// The target is reached.
    pub reached: bool,
    /// The rider's end: the verdict, the event's points and the freeride total with them put back.
    pub verdict: Option<(game_sim::event::Verdict, i32, i32)>,
}

impl EventShown {
    /// What `seat` of the ride's `event` shows, its card `card`; `None` with no competition on or a
    /// rider who is not in it.
    pub fn of(event: &game_sim::event::Event, seat: usize, card: &Score) -> Option<EventShown> {
        let c = event.on.as_ref()?;
        let s = c.seats.get(seat)?;
        let countdown = if c.counting {
            Some(3u32.saturating_sub(c.elapsed.0.max(0.0) as u32).max(1))
        } else if c.running && c.clock.0 < 1.0 {
            Some(0)
        } else {
            None
        };
        let verdict = event.verdicts.get(seat).copied().flatten().map(|(v, profile)| (v, profile.wrapping_sub(s.saved), card.total.0 as i32));
        Some(EventShown {
            target: c.target,
            countdown,
            left: (c.running && verdict.is_none()).then_some(s.left.0),
            warning: s.warning,
            reached: s.reached,
            verdict,
        })
    }
}

/// The speed the speedometer reads off its rider (P182): the ridden speed, and 0 while the rider is
/// down. The original's readout drops to 0 on the wreck's tick and stays there until the put-back
/// (S109's run2.mp4, 29.6–33.6 s); the port's target block follows the tumbling wreck body then
/// ([`crate::chase::wrecked`]), which is the camera's and not the HUD's.
pub fn speed_shown(speed: f32, down: bool) -> f32 {
    if down { 0.0 } else { speed }
}

/// The clock as the event's HUD spells it, `M:SS`, a second shown until it has gone.
pub fn clock_text(left: f32) -> String {
    let s = left.max(0.0).ceil() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// What a verdict says.
pub fn verdict_text(v: game_sim::event::Verdict) -> &'static str {
    match v {
        game_sim::event::Verdict::Win => "YOU WIN",
        game_sim::event::Verdict::Lose => "YOU LOSE",
        game_sim::event::Verdict::Tie => "TIE",
    }
}

/// The announcer's line when the target is reached.
pub const REACHED_TEXT: &str = "TARGET REACHED!";
/// How long it stays up, seconds.
const REACHED_UP: f32 = 3.0;

// ---- what moves with time -------------------------------------------------------------------------

/// How a combo ended.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Out {
    landed: bool,
    at: Instant,
    banked: f32,
    /// How many tricks the combo held: a landed chain of two or more shows its total first.
    tricks: usize,
}

impl Out {
    /// Whether the original's "N Stunt Combo Total" stands before the play-out.
    fn totals(self) -> bool {
        self.landed && self.tricks >= 2
    }
}

/// One view's HUD over time: the trick pop-up off the rider's score card, the score's flash and the
/// camera toast.
#[derive(Clone, Debug, Default)]
pub struct Tracker {
    /// The combo on screen: its nodes' ids and names, oldest first, and its points.
    ids: Vec<u8>,
    names: Vec<String>,
    points: f32,
    /// When its newest node came.
    popped: Option<Instant>,
    out: Option<Out>,
    /// The banked total last frame.
    total: Option<f32>,
    flashed: Option<Instant>,
    camera: Option<(String, Instant)>,
    /// A camera has been noted: the first, the ride's start, is not a change to show.
    camera_seen: bool,
    /// The newest node's points as the card last held them, and whether a backflip was coming round
    /// under it: what the original's Stunt movie writes (P68).
    newest_points: f32,
    flipping: bool,
    /// The event's target was reached at the last frame, and when it was (P155).
    reached: bool,
    reached_at: Option<Instant>,
}

/// The pop-up as it stands.
#[derive(Clone, Debug, PartialEq)]
pub struct Popup {
    /// The chain's entries, oldest first, repeats folded ([`folded`]).
    pub names: Vec<String>,
    pub points: String,
    /// The combo's length, shown beside the points from two on.
    pub mult: Option<usize>,
    /// `Some(true)` banked, `Some(false)` lost, `None` still going.
    pub outcome: Option<bool>,
    pub colour: [u8; 4],
    /// The whole pop-up's scale and alpha (its play-out), and the newest name's and the points' own
    /// (their pop in).
    pub scale: f32,
    pub alpha: f32,
    pub newest: (f32, f32),
}

/// The chain as the pop-up spells it: a trick done again straight after itself is one entry with its
/// count, `Backflip x2`, and the count grows in as a new name would. Only back-to-back repeats fold, so
/// a broken run keeps its order (`Backflip + Can Can + Backflip`). The original draws no chain at all —
/// only the newest name, over its points and the combo's `X n` — so this is ours; the score's nodes
/// and the multiplier still count every trick.
pub fn folded(names: &[String]) -> Vec<String> {
    let mut runs: Vec<(&String, usize)> = Vec::new();
    for n in names {
        match runs.last_mut() {
            Some((last, count)) if *last == n => *count += 1,
            _ => runs.push((n, 1)),
        }
    }
    runs.into_iter().map(|(n, count)| if count > 1 { format!("{n} x{count}") } else { n.clone() }).collect()
}

fn since(now: Instant, then: Instant) -> f32 {
    now.saturating_duration_since(then).as_secs_f32()
}

impl Tracker {
    /// A frame of the rider's `score`, `name` naming a node's id as the HUD does.
    pub fn update(&mut self, now: Instant, score: &Score, name: impl Fn(u8) -> String) {
        let total = score.total.0;
        let banked = self.total.map_or(0.0, |t| total - t);
        if banked > 0.0 {
            self.flashed = Some(now);
        }
        self.total = Some(total);
        if score.combo && !score.nodes.is_empty() {
            let ids: Vec<u8> = score.nodes.iter().rev().map(|n| n.id).collect();
            let same = self.out.is_none() && ids.len() >= self.ids.len() && ids[..self.ids.len()] == self.ids[..];
            if !same {
                self.ids.clear();
                self.out = None;
            }
            if ids.len() > self.ids.len() {
                self.popped = Some(now);
            }
            self.names = ids.iter().map(|&i| name(i)).collect();
            self.ids = ids;
            self.points = score.nodes.iter().map(|n| n.points.0).sum();
            let head = &score.nodes[0];
            self.newest_points = head.points.0;
            self.flipping = score.flip != 0 && head.id != game_sim::score::BACKFLIP;
        } else if !self.ids.is_empty() && self.out.is_none() {
            self.out = Some(Out { landed: banked > 0.0, at: now, banked, tricks: self.ids.len() });
        }
        if let Some(o) = self.out
            && since(now, o.at) >= Tracker::length(o)
        {
            self.ids.clear();
            self.names.clear();
            self.out = None;
        }
    }

    /// How long an ended combo stays: a landed chain holds its total first.
    fn length(o: Out) -> f32 {
        if o.totals() { HOLD + OUT } else { OUT }
    }

    /// A frame of the freestyle event (P155): when its target was reached, for the announcer's line.
    pub fn note_event(&mut self, now: Instant, event: Option<&EventShown>) {
        let reached = event.is_some_and(|e| e.reached);
        if reached && !self.reached {
            self.reached_at = Some(now);
        }
        if event.is_none() {
            self.reached_at = None;
        }
        self.reached = reached;
    }

    /// Whether the announcer's target line is up at `now`.
    pub fn reached_line(&self, now: Instant) -> bool {
        self.reached_at.is_some_and(|t| since(now, t) < REACHED_UP)
    }

    /// A camera preset `name` came up.
    pub fn note_camera(&mut self, now: Instant, name: &str) {
        if self.camera_seen {
            self.camera = Some((name.to_string(), now));
        }
        self.camera_seen = true;
    }

    /// The pop-up at `now`, if there is one.
    pub fn popup(&self, now: Instant) -> Option<Popup> {
        if self.names.is_empty() {
            return None;
        }
        let popped = |at: Option<Instant>| {
            let pop = at.map_or(1.0, |p| (since(now, p) / POP).clamp(0.0, 1.0));
            let ease = 1.0 - (1.0 - pop).powi(3);
            (POP_FROM + (1.0 - POP_FROM) * ease, 0.1 + 0.9 * pop)
        };
        let n = self.names.len();
        let colour = |landed| if landed { LAND_COLOUR } else { BAIL_COLOUR };
        let (names, points, mult, outcome, colour, out, newest) = match self.out {
            None => (folded(&self.names), grouped(self.points), (n >= 2).then_some(n), None, ORANGE, 0.0, popped(self.popped)),
            // A landed chain's total pops in where the chain was, stands, then plays out.
            Some(o) if o.totals() => {
                let t = ((since(now, o.at) - HOLD) / OUT).clamp(0.0, 1.0);
                let total = vec![format!("{} Stunt Combo Total", o.tricks)];
                (total, grouped(o.banked), None, Some(true), colour(true), t, popped(Some(o.at)))
            }
            Some(o) => {
                let t = (since(now, o.at) / OUT).clamp(0.0, 1.0);
                (folded(&self.names), grouped(self.points), (n >= 2).then_some(n), Some(o.landed), colour(o.landed), t, (1.0, 1.0))
            }
        };
        Some(Popup { names, points, mult, outcome, colour, scale: 1.0 - (1.0 - OUT_TO) * out, alpha: 1.0 - out, newest })
    }

    /// What the original's HUD pops up at `now` (P68): its Stunt movie and its info text.
    ///
    /// The movie says what the original's text setter writes — the newest trick's name (after
    /// `BackFlip + ` while a flip comes round under it) over [`overlay::stunt_points`], or a landed
    /// chain's [`overlay::combo_total`] over the banked points — and moves as [`Tracker::popup`]
    /// does: the newest name pops in, an ended combo plays out. The info text is the camera toast.
    /// The original's HUD left the shell in P170; this stays for its drawing in `overlay.rs`.
    #[allow(dead_code)]
    pub fn pops(&self, now: Instant) -> overlay::Pops {
        let stunt = self.popup(now).map(|p| {
            let n = self.ids.len();
            let (name, points) = match self.out {
                Some(o) if o.totals() => (overlay::combo_total(o.tricks), (o.banked.max(0.0) as i32).to_string()),
                _ => {
                    let newest = self.names.last().map_or("", String::as_str);
                    (overlay::stunt_name(newest, self.flipping), overlay::stunt_points(self.newest_points, n))
                }
            };
            overlay::Stunt { name, points, scale: p.scale, alpha: p.alpha, newest: p.newest }
        });
        let info = self.toast(now).map(|(text, alpha)| overlay::Info { text: text.to_string(), alpha });
        overlay::Pops { stunt, info }
    }

    /// How far the score's flash is through, 0 at the bank to 1 when it is gone.
    fn flash(&self, now: Instant) -> Option<f32> {
        self.flashed.map(|f| since(now, f) / FLASH).filter(|k| *k < 1.0)
    }

    /// The camera toast and its alpha.
    fn toast(&self, now: Instant) -> Option<(&str, f32)> {
        let (name, at) = self.camera.as_ref()?;
        let t = since(now, *at);
        (t < TOAST).then(|| (name.as_str(), ((TOAST - t) / TOAST_FADE).min(1.0)))
    }
}

/// A count with thousands grouped, as the concept prints it: `128,450`.
pub fn grouped(points: f32) -> String {
    let digits = (points.max(0.0) as u64).to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// What the marker menu's label says: the slot and whether it holds a marker, or the player.
pub fn panel_label(p: &Panel) -> (String, String) {
    match p.pick {
        Pick::Spawn if p.cleared => ("Cleared".into(), "every marker".into()),
        Pick::Spawn => ("Spawn".into(), String::new()),
        Pick::Slot(s) => (format!("Marker {}", s + 1), if p.filled.get(s).copied().unwrap_or(false) { "set" } else { "empty" }.into()),
        // The + (P139): the slot a Down on it adds.
        Pick::More => (format!("Marker {}", p.filled.len() + 1), "new".into()),
        Pick::Player(seat) => {
            let name = p.others.iter().find(|(o, _)| *o == seat).and_then(|(_, n)| n.clone()).filter(|n| !n.trim().is_empty());
            match name {
                Some(n) => (format!("to {n}"), format!("P{}", seat + 1)),
                None => (format!("to P{}", seat + 1), String::new()),
            }
        }
    }
}

// ---- what was drawn where, for the tests ----------------------------------------------------------

/// A line of type as drawn: its text, its box in frame pixels (left, top, right, bottom) and colour.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    pub text: String,
    pub rect: [f32; 4],
    pub colour: [u8; 4],
}

/// The marker menu as drawn.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PanelDrawn {
    pub rect: [f32; 4],
    /// The spawn's middle, radius and whether it is selected (P135).
    pub spawn: ([f32; 2], f32, bool),
    /// Each slot drawn — from [`PanelDrawn::first`] on — its middle, radius, whether it is selected
    /// and whether it holds a marker.
    pub slots: Vec<([f32; 2], f32, bool, bool)>,
    /// The first of the row's entries drawn (the slots, the +, the players, in order): past 0 when
    /// the row has scrolled (P139), and while it is a slot, the first slot drawn.
    pub first: usize,
    /// The + after the last slot while it is there and in view: its middle, radius and whether it is
    /// selected.
    pub more: Option<([f32; 2], f32, bool)>,
    /// Whether an arrow says there is more off the row's left, and off its right.
    pub arrows: [bool; 2],
    /// Each other player's pill: their seat, whether it is selected, its box.
    pub players: Vec<(usize, bool, [f32; 4])>,
    pub label: String,
    /// The hold bar's fill, 0 to 1, and whether it has fired.
    pub hold: f32,
    pub fired: bool,
}

/// What [`sprites`] put where, in frame pixels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drawn {
    /// Layout pixels to frame pixels.
    pub scale: f32,
    pub score_bar: [f32; 4],
    pub score: Line,
    pub flash: bool,
    pub under_score: Vec<Line>,
    pub trick: Option<(Popup, Vec<Line>)>,
    /// The digital speed, in DSEG7.
    pub speed: Option<Line>,
    /// The dial: its pivot, the needle's angle (degrees anticlockwise off up) and the odometer's text.
    pub dial: Option<([f32; 2], f32, Option<String>)>,
    pub meter: Option<[f32; 4]>,
    pub preload: Option<f32>,
    /// The radar's middle and radius, its compass letters, the riders' dots (index, place, colour)
    /// and the icons.
    pub radar: Option<([f32; 2], f32)>,
    pub compass: Vec<(char, [f32; 2])>,
    /// The map's name in the radar, and each neighbour's inside the rim by its compass letter (MD42, MD46).
    pub map: Option<Line>,
    pub around: Vec<(char, Line)>,
    pub dots: Vec<(usize, [f32; 2], [u8; 4])>,
    pub icons: Vec<(Icon, [f32; 2])>,
    pub prompts: Vec<Line>,
    pub markers: Option<PanelDrawn>,
    pub camera: Option<Line>,
    /// The join cue (P171).
    pub join: Option<Line>,
    /// The freestyle event's lines (P155): the target, the countdown or the clock, the announcer's
    /// line and the verdict, as drawn.
    pub event: Vec<Line>,
}

// ---- the pen --------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Face {
    Words,
    Digits,
}

struct Pen<'a> {
    out: &'a mut Vec<Sprite>,
    /// The view in the frame: left, top, width, height.
    view: [f32; 4],
    s: f32,
    /// Where [`ink::ink`]'s atlas is among the pictures; [`ink::digits`]' is the next.
    base: usize,
    corner: Corner,
    /// What every tint's alpha is multiplied by.
    alpha: f32,
}

impl Pen<'_> {
    /// A layout point, held to the pen's corner, in frame pixels.
    fn px(&self, (x, y): (f32, f32)) -> [f32; 2] {
        let [vx, vy, w, h] = self.view;
        let (lw, lh) = overlay::LAYOUT;
        let s = self.s;
        match self.corner {
            Corner::TopLeft => [vx + x * s, vy + y * s],
            Corner::TopRight => [vx + w - (lw - x) * s, vy + y * s],
            Corner::BottomLeft => [vx + x * s, vy + h - (lh - y) * s],
            Corner::BottomRight => [vx + w - (lw - x) * s, vy + h - (lh - y) * s],
            Corner::Bottom => [vx + w / 2.0 + (x - lw / 2.0) * s, vy + h - (lh - y) * s],
        }
    }

    fn rect(&self, b: Box4) -> Box4 {
        let [x, y] = self.px((b[0], b[1]));
        [x, y, b[2] * self.s, b[3] * self.s]
    }

    fn tint(&self, c: [u8; 4]) -> [u8; 4] {
        [c[0], c[1], c[2], (c[3] as f32 * self.alpha).round() as u8]
    }

    fn round(&mut self, b: Box4, radius: f32, top: [u8; 4], bottom: [u8; 4]) {
        let (t, bt) = (self.tint(top), self.tint(bottom));
        ink::round(self.out, self.rect(b), radius * self.s, t, bt);
    }

    fn ring(&mut self, b: Box4, radius: f32, width: f32, colour: [u8; 4]) {
        let c = self.tint(colour);
        ink::ring(self.out, self.rect(b), radius * self.s, width * self.s, c);
    }

    fn disc(&mut self, (x, y): (f32, f32), r: f32, colour: [u8; 4]) {
        self.round([x - r, y - r, 2.0 * r, 2.0 * r], r, colour, colour);
    }

    fn circle(&mut self, (x, y): (f32, f32), r: f32, width: f32, colour: [u8; 4]) {
        self.ring([x - r, y - r, 2.0 * r, 2.0 * r], r, width, colour);
    }

    fn bar(&mut self, a: (f32, f32), b: (f32, f32), half: f32, tip_half: f32, colour: [u8; 4]) {
        let c = self.tint(colour);
        ink::bar(self.out, self.px(a), self.px(b), half * self.s, tip_half * self.s, c);
    }

    /// A stadium `b`, `left` up to `split` (layout x) and `right` from there.
    fn capsule(&mut self, b: Box4, left: [u8; 4], split: f32, right: [u8; 4]) {
        let [x, y, w, h] = self.rect(b);
        let at = (split - b[0]) * self.s;
        let (x0, y0, x1, y1) = (x - 1.0, y - 1.0, x + w + 1.0, y + h + 1.0);
        let shape = Shape::Capsule { size: [w, h], split: at, right: self.tint(right) };
        self.out.push(Sprite {
            corners: [[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
            uv: [[0.0; 2]; 4],
            local: [[-1.0, -1.0], [w + 1.0, -1.0], [w + 1.0, h + 1.0], [-1.0, h + 1.0]],
            clamp: [0.0; 4],
            image: None,
            tint: self.tint(left),
            shape,
        });
    }

    fn face(face: Face) -> &'static Type {
        match face {
            Face::Words => ink::ink(),
            Face::Digits => ink::digits(),
        }
    }

    fn measure(face: Face, w: Weight, text: &str, size: f32) -> f32 {
        Pen::face(face).measure(w, text, size)
    }

    /// How far a line's capitals (or digits) stand above its baseline at `size`.
    fn cap(face: Face, size: f32) -> f32 {
        let c = if face == Face::Digits { '8' } else { 'H' };
        Pen::face(face).top(Weight::Heavy, c) * size
    }

    /// `text` from `x` on the baseline `y`, all in layout pixels; its box in frame pixels.
    #[allow(clippy::too_many_arguments)]
    fn text(&mut self, face: Face, w: Weight, text: &str, (x, y): (f32, f32), size: f32, slant: f32, colour: [u8; 4]) -> [f32; 4] {
        let [px, py] = self.px((x, y));
        let image = self.base + (face == Face::Digits) as usize;
        let c = self.tint(colour);
        Pen::face(face).text(self.out, image, w, text, px, py, size * self.s, slant, c);
        let width = Pen::measure(face, w, text, size) * self.s;
        [px, py - Pen::cap(face, size) * self.s, px + width, py]
    }

    /// [`Pen::text`] ringed in `edge` `by` layout pixels: the concept's text-shadow outline.
    #[allow(clippy::too_many_arguments)]
    fn edged(&mut self, face: Face, w: Weight, text: &str, (x, y): (f32, f32), size: f32, slant: f32, colour: [u8; 4], edge: [u8; 4], by: f32) -> [f32; 4] {
        for (dx, dy) in [(-by, -by), (by, -by), (-by, by), (by, by), (0.0, by), (0.0, -by), (by, 0.0), (-by, 0.0)] {
            self.text(face, w, text, (x + dx, y + dy), size, slant, edge);
        }
        self.text(face, w, text, (x, y), size, slant, colour)
    }

    /// Letters spaced `gap` layout pixels apart past their own advance; its width in layout pixels.
    #[allow(clippy::too_many_arguments)]
    fn spaced(&mut self, w: Weight, text: &str, (x, y): (f32, f32), size: f32, gap: f32, slant: f32, colour: [u8; 4]) -> f32 {
        let mut at = x;
        for c in text.chars() {
            let s = c.to_string();
            self.text(Face::Words, w, &s, (at, y), size, slant, colour);
            at += Pen::measure(Face::Words, w, &s, size) + gap;
        }
        at - gap - x
    }

    fn spaced_width(w: Weight, text: &str, size: f32, gap: f32) -> f32 {
        let n = text.chars().count();
        text.chars().map(|c| Pen::measure(Face::Words, w, &c.to_string(), size)).sum::<f32>() + gap * n.saturating_sub(1) as f32
    }
}

// ---- drawing --------------------------------------------------------------------------------------

/// The pictures [`sprites`] counts into from its `base`: the type's atlas, then the digits'.
pub fn images() -> [&'static Texture; 2] {
    [&ink::ink().atlas, &ink::digits().atlas]
}

/// The scale a view of `w` by `h` frame pixels draws the layout at.
pub fn scale_of(w: f32, h: f32, scale: f32) -> f32 {
    (w / overlay::LAYOUT.0).min(h / overlay::LAYOUT.1) * scale.max(0.1)
}

/// View `view`'s HUD (left, top, width, height in the frame) as sprites into `out`, [`images`]
/// counted from `base`, at `now`.
pub fn sprites(out: &mut Vec<Sprite>, base: usize, view: [f32; 4], shown: &Shown, tracker: &Tracker, now: Instant) -> Drawn {
    let s = scale_of(view[2], view[3], shown.scale);
    let mut pen = Pen { out, view, s, base, corner: Corner::TopLeft, alpha: 1.0 };
    let mut drawn = Drawn { scale: s, ..Drawn::default() };
    if shown.minimap {
        radar(&mut pen, shown, &mut drawn);
    }
    score(&mut pen, shown, tracker, now, &mut drawn);
    speedometer(&mut pen, shown, &mut drawn);
    prompts(&mut pen, shown, &mut drawn);
    top_right(&mut pen, shown, tracker, now, &mut drawn);
    if let Some(e) = &shown.event {
        event(&mut pen, e, tracker, now, &mut drawn);
    }
    if let Some(p) = tracker.popup(now) {
        let lines = trick(&mut pen, &p);
        drawn.trick = Some((p, lines));
    }
    drawn
}

/// [`sprites`] drawn into `frame`, which is the view's picture, on the CPU.
pub fn draw(frame: &mut Frame, shown: &Shown, tracker: &Tracker, now: Instant) -> Drawn {
    let mut out = Vec::new();
    let view = [0.0, 0.0, frame.width as f32, frame.height as f32];
    let drawn = sprites(&mut out, 0, view, shown, tracker, now);
    ink::raster(frame, &out, &images());
    drawn
}

/// The score, top left: the two-tone capsule, SCORE and the banked points, ringed orange a moment
/// on a bank; under it MODIFIED and the rider's tag.
fn score(pen: &mut Pen, shown: &Shown, tracker: &Tracker, now: Instant, drawn: &mut Drawn) {
    pen.corner = Corner::TopLeft;
    let [x, y, _, h] = SCORE_BAR;
    pen.capsule(SCORE_BAR, shade(0x80), SCORE_LABEL_END, shade(0x66));
    let mid = y + h / 2.0;
    let (size, gap) = (EM, 0.14 * EM);
    let lw = Pen::spaced_width(Weight::Heavy, "SCORE", size, gap);
    let base = mid + Pen::cap(Face::Words, size) / 2.0;
    pen.spaced(Weight::Heavy, "SCORE", ((x + SCORE_LABEL_END) / 2.0 - lw / 2.0, base), size, gap, 0.0, GREY);
    let flash = tracker.flash(now);
    let colour = match flash {
        Some(k) => mix(ORANGE, GREY, k),
        None => GREY,
    };
    let text = grouped(shown.gauges.score as f32);
    let size = 1.25 * EM;
    let rect = pen.text(Face::Words, Weight::Heavy, &text, (SCORE_NUMBER, mid + Pen::cap(Face::Words, size) / 2.0), size, 0.0, colour);
    if let Some(k) = flash {
        let a = ((1.0 - k) * 255.0) as u8;
        pen.ring(SCORE_BAR, h / 2.0, 0.14 * EM, [ORANGE[0], ORANGE[1], ORANGE[2], a]);
    }
    drawn.score_bar = pen.rect(SCORE_BAR);
    drawn.score = Line { text, rect, colour };
    drawn.flash = flash.is_some();
    let mut lines: Vec<String> = Vec::new();
    if shown.gauges.modified {
        lines.push("MODIFIED".into());
    }
    lines.extend(shown.tag.clone().filter(|t| !t.is_empty()));
    let size = 0.8 * EM;
    for (i, l) in lines.into_iter().enumerate() {
        let at = (UNDER_SCORE.0, UNDER_SCORE.1 + i as f32 * 1.4 * size);
        let r = pen.text(Face::Words, Weight::Heavy, &l, at, size, ink::SLANT, GREY);
        drawn.under_score.push(Line { text: l, rect: r, colour: GREY });
    }
}

/// The event's middle, top centre, under the score bar's line.
const EVENT_AT: (f32, f32) = (320.0, 25.0);

/// The freestyle event (P155), top centre: the target over the clock in a dark pill, the clock
/// maroon and blinking under the warning; the countdown's number big in the middle; the announcer's
/// line under the pill a while after the target is reached; and the verdict in the middle over the
/// event's points and the freeride total.
fn event(pen: &mut Pen, e: &EventShown, tracker: &Tracker, now: Instant, drawn: &mut Drawn) {
    pen.corner = Corner::TopLeft;
    let (cx, top) = EVENT_AT;
    let centred = |pen: &mut Pen, text: &str, y: f32, size: f32, colour: [u8; 4], drawn: &mut Drawn| {
        let w = Pen::measure(Face::Words, Weight::Heavy, text, size);
        let r = pen.edged(Face::Words, Weight::Heavy, text, (cx - w / 2.0, y), size, ink::SLANT, colour, OUTLINE, 0.1 * EM);
        drawn.event.push(Line { text: text.to_string(), rect: r, colour });
    };
    let target = format!("TARGET {}", grouped(e.target as f32));
    let clock = e.left.map(clock_text);
    let (small, big) = (0.8 * EM, 1.6 * EM);
    let w = Pen::measure(Face::Words, Weight::Heavy, &target, small).max(clock.as_ref().map_or(0.0, |c| Pen::measure(Face::Words, Weight::Heavy, c, big))) + 2.0 * EM;
    let h = if clock.is_some() { small + big + 1.4 * EM } else { small + 1.2 * EM };
    pen.round([cx - w / 2.0, top, w, h], 0.8 * EM, shade(128), shade(128));
    let colour = if e.reached { ORANGE } else { GREY };
    centred(pen, &target, top + 0.6 * EM + small, small, colour, drawn);
    let under = top + h + 1.6 * EM;
    if let Some(c) = &clock {
        // Blinking four times a second, on the clock's own time.
        let blink = e.left.is_some_and(|l| (l * 4.0) as u32 % 2 == 0);
        let colour = if e.warning { if blink { MAROON.0 } else { WHITE } } else { CHALK };
        centred(pen, c, top + 0.9 * EM + small + big, big, colour, drawn);
    }
    if tracker.reached_line(now) {
        centred(pen, REACHED_TEXT, under, 1.1 * EM, ORANGE, drawn);
    }
    if let Some(n) = e.countdown {
        let text = if n == 0 { "GO!".to_string() } else { n.to_string() };
        centred(pen, &text, 200.0, 4.0 * EM, ORANGE, drawn);
    }
    if let Some((v, points, total)) = e.verdict {
        centred(pen, verdict_text(v), 190.0, 3.0 * EM, ORANGE, drawn);
        centred(pen, &format!("EVENT {}", grouped(points as f32)), 190.0 + 2.2 * EM, 1.1 * EM, CHALK, drawn);
        centred(pen, &format!("FREERIDE TOTAL {}", grouped(total as f32)), 190.0 + 3.7 * EM, 1.1 * EM, CHALK, drawn);
    }
}

fn mix(a: [u8; 4], b: [u8; 4], k: f32) -> [u8; 4] {
    let k = k.clamp(0.0, 1.0);
    std::array::from_fn(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * k).round() as u8)
}

/// The trick pop-up, low centre: the combo's names, a back-to-back repeat folded to `Name xN`
/// ([`folded`]), joined by `+` over its points and its length, in the original's orange ringed dark;
/// the newest name and the points grow in from half their size, and an ended combo shrinks away,
/// fading.
fn trick(pen: &mut Pen, p: &Popup) -> Vec<Line> {
    pen.corner = Corner::Bottom;
    pen.alpha = p.alpha;
    let (cx, cy) = TRICK_AT;
    // The names, the latest three folded entries, shrunk to fit across.
    let mut names: Vec<String> = p.names.iter().rev().take(3).rev().cloned().collect();
    if p.names.len() > 3 {
        names.insert(0, "…".into());
    }
    let mut size = 1.9 * EM;
    let plus = |size: f32| Pen::measure(Face::Words, Weight::Heavy, "+", size) + 0.4 * size;
    let width = |size: f32| names.iter().map(|n| Pen::measure(Face::Words, Weight::Heavy, n, size)).sum::<f32>() + plus(size) * (names.len() - 1) as f32;
    if width(size) > 600.0 {
        size *= 600.0 / width(size);
    }
    let points = 1.7 * EM;
    let chain_mid = cy - 0.6 * points;
    let points_mid = cy + 0.75 * points;
    let mut lines = Vec::new();
    // One piece at `(x, mid)` (its left and middle at scale 1), drawn at the pop-up's scale about
    // the pop-up's middle and at `own` about its own.
    let piece = |pen: &mut Pen, text: &str, x: f32, mid: f32, size: f32, colour: [u8; 4], own: (f32, f32)| {
        let w = Pen::measure(Face::Words, Weight::Heavy, text, size);
        let k = p.scale * own.0;
        let centre = (cx + (x + w / 2.0 - cx) * p.scale, cy + (mid - cy) * p.scale);
        let size = size * k;
        let at = (centre.0 - w * k / 2.0, centre.1 + Pen::cap(Face::Words, size) / 2.0);
        let alpha = pen.alpha;
        pen.alpha *= own.1;
        let shadow = [0, 0, 0, 128];
        pen.text(Face::Words, Weight::Heavy, text, (at.0 + 0.08 * size, at.1 + 0.12 * size), size, ink::SLANT, shadow);
        let r = pen.edged(Face::Words, Weight::Heavy, text, at, size, ink::SLANT, colour, OUTLINE, 0.06 * size);
        pen.alpha = alpha;
        r
    };
    let mut x = cx - width(size) / 2.0;
    let last = names.len() - 1;
    for (i, n) in names.iter().enumerate() {
        if i > 0 {
            piece(pen, "+", x + 0.2 * size, chain_mid, size, MULT, (1.0, 1.0));
            x += plus(size);
        }
        let own = if i == last { p.newest } else { (1.0, 1.0) };
        let r = piece(pen, n, x, chain_mid, size, p.colour, own);
        lines.push(Line { text: n.clone(), rect: r, colour: p.colour });
        x += Pen::measure(Face::Words, Weight::Heavy, n, size);
    }
    let mult = p.mult.map(|n| format!("x{n}"));
    let (pw, mw) = (Pen::measure(Face::Words, Weight::Heavy, &p.points, points), mult.as_ref().map_or(0.0, |m| Pen::measure(Face::Words, Weight::Heavy, m, 1.3 * EM) + 0.45 * EM));
    let left = cx - (pw + mw) / 2.0;
    let r = piece(pen, &p.points, left, points_mid, points, p.colour, p.newest);
    lines.push(Line { text: p.points.clone(), rect: r, colour: p.colour });
    if let Some(m) = mult {
        let r = piece(pen, &m, left + pw + 0.45 * EM, points_mid, 1.3 * EM, MULT, (1.0, 1.0));
        lines.push(Line { text: m, rect: r, colour: MULT });
    }
    pen.alpha = 1.0;
    lines
}

/// The speedometer, bottom right at `speedometer.lyt`'s places: the DSEG7 number with its right
/// edge at the layout's and centred on the meter's height, or the dial; and on the bike the preload meter
/// standing right of either, filling orange from the bottom with the charge (V53).
fn speedometer(pen: &mut Pen, shown: &Shown, drawn: &mut Drawn) {
    pen.corner = Corner::BottomRight;
    let g = shown.gauges;
    let ((right, _), (mx, my)) = overlay::Place::Full.speedometer();
    let (mw, mh) = (20.0, 85.0);
    if g.bike {
        let b = [mx, my, mw, mh];
        pen.capsule(b, shade(0xb3), f32::MAX, shade(0));
        let i = 3.0;
        let inner = mh - 2.0 * i;
        if g.preload.is_some() {
            pen.capsule([mx + i, my + i, mw - 2.0 * i, inner], [88, 44, 10, 255], f32::MAX, shade(0));
        }
        let fill = g.preload.unwrap_or(0.0).clamp(0.0, 1.0);
        if fill > 0.0 {
            let h = (inner * fill).max(mw - 2.0 * i);
            pen.capsule([mx + i, my + i + inner - h, mw - 2.0 * i, h], ORANGE, f32::MAX, shade(0));
        }
        drawn.meter = Some(pen.rect(b));
        drawn.preload = Some(fill);
    }
    if g.speedometer == Speedometer::Analog {
        dial(pen, shown, drawn);
        return;
    }
    let text = overlay::speed_text(shown.speed);
    let size = 4.6 * EM;
    let w = Pen::measure(Face::Digits, Weight::Heavy, &text, size);
    let at = (right - w, my + (mh + Pen::cap(Face::Digits, size)) / 2.0);
    pen.text(Face::Digits, Weight::Heavy, &text, (at.0 + 0.05 * size, at.1 + 0.06 * size), size, 0.0, [0, 0, 0, 115]);
    let rect = pen.text(Face::Digits, Weight::Heavy, &text, at, size, 0.0, WHITE);
    drawn.speed = Some(Line { text, rect, colour: WHITE });
}

/// A point `r` from the pivot at `degrees` anticlockwise off up.
fn polar(c: (f32, f32), r: f32, degrees: f32) -> (f32, f32) {
    let a = degrees.to_radians();
    (c.0 - r * a.sin(), c.1 - r * a.cos())
}

/// The dial as the concept draws it: white dashes and numbers 0 to 100 round from lower left to
/// lower right, on the original's needle angles; the orange needle off the grey hub; and on the
/// bike the odometer's `%07.1f` in a dark capsule under it.
fn dial(pen: &mut Pen, shown: &Shown, drawn: &mut Drawn) {
    let mph_angle = |mph: f32| overlay::NEEDLE_REST - mph * overlay::NEEDLE_PER_MPH;
    for i in 0..=10 {
        let a = mph_angle(i as f32 * 10.0);
        pen.bar(polar(PIVOT, 47.0 * DK, a), polar(PIVOT, 57.0 * DK, a), 2.5 * DK, 2.5 * DK, WHITE);
        let label = (i * 10).to_string();
        let size = 13.0 * DK;
        let (x, y) = polar(PIVOT, 71.0 * DK, a);
        let w = Pen::measure(Face::Words, Weight::Bold, &label, size);
        pen.edged(Face::Words, Weight::Bold, &label, (x - w / 2.0, y + Pen::cap(Face::Words, size) / 2.0), size, 0.0, WHITE, shade(255), DK);
    }
    let degrees = overlay::needle_degrees(shown.speed);
    let tip = polar(PIVOT, 54.0 * DK, degrees);
    pen.bar(PIVOT, polar(PIVOT, 55.0 * DK, degrees), 5.5 * DK, 0.0, [0x5a, 0x2a, 0x00, 255]);
    pen.bar(PIVOT, tip, 4.5 * DK, 0.0, ORANGE);
    pen.disc(PIVOT, 12.0 * DK, [0x2b, 0x2b, 0x2b, 255]);
    pen.disc(PIVOT, 10.0 * DK, [0x6b, 0x6b, 0x6b, 255]);
    pen.disc((PIVOT.0 - 2.0 * DK, PIVOT.1 - 2.0 * DK), 5.0 * DK, [0x9a, 0x9a, 0x9a, 255]);
    let mut odometer = None;
    if shown.gauges.bike {
        let b = [PIVOT.0 - 40.0 * DK, PIVOT.1 + 62.0 * DK, 80.0 * DK, 22.0 * DK];
        pen.round(b, 11.0 * DK, shade(0xb3), shade(0xb3));
        let text = overlay::odometer_text(shown.gauges.feet);
        let (size, gap) = (15.0 * DK, 1.6 * DK);
        let w = Pen::spaced_width(Weight::Heavy, &text, size, gap);
        pen.spaced(Weight::Heavy, &text, (PIVOT.0 - w / 2.0, PIVOT.1 + 78.0 * DK), size, gap, 0.0, WHITE);
        odometer = Some(text);
    }
    drawn.dial = Some((pen.px(PIVOT), degrees, odometer));
}

/// The radar, bottom left where the original's sits: a dark disc, the rose's two axes and N E S W
/// outside its rim turning with the heading, the zones as their letters in their colours, standing
/// machines and bikes, the markers in their slots' colours, the other riders' dots — yellow, the
/// game's red — and the player's arrow fixed pointing up. Anything held to the rim is a chevron.
fn radar(pen: &mut Pen, shown: &Shown, drawn: &mut Drawn) {
    pen.corner = Corner::BottomLeft;
    pen.disc(RADAR, RADAR_R, shade(140));
    let turn = overlay::radar_turn(shown.forward);
    for k in 0..4 {
        let a = turn + k as f32 * FRAC_PI_2;
        let e = (a.sin(), -a.cos());
        if k < 2 {
            let (p, q) = ((RADAR.0 + e.0 * RADAR_R, RADAR.1 + e.1 * RADAR_R), (RADAR.0 - e.0 * RADAR_R, RADAR.1 - e.1 * RADAR_R));
            pen.bar(p, q, 0.55 * RK, 0.55 * RK, [200, 200, 200, 140]);
        }
        let letter = ['N', 'E', 'S', 'W'][k];
        let at = (RADAR.0 + e.0 * 95.0 * RK, RADAR.1 + e.1 * 95.0 * RK);
        let size = 13.0 * RK;
        let w = Pen::measure(Face::Words, Weight::Heavy, &letter.to_string(), size);
        let base = at.1 + Pen::cap(Face::Words, size) / 2.0;
        pen.edged(Face::Words, Weight::Heavy, &letter.to_string(), (at.0 - w / 2.0, base), size, ink::SLANT, WHITE, shade(255), 1.3 * RK);
        drawn.compass.push((letter, pen.px(at)));
    }
    map_names(pen, shown, turn, drawn);
    let rim = |at: (f32, f32)| ((at.0 - RADAR.0).powi(2) + (at.1 - RADAR.1).powi(2)).sqrt() >= RADAR_R - 0.5;
    let chevron = |pen: &mut Pen, at: (f32, f32), colour: [u8; 4]| {
        let a = (at.1 - RADAR.1).atan2(at.0 - RADAR.0);
        let (c, s) = (a.cos(), a.sin());
        let r = RADAR_R - 3.0 * RK;
        let inner = (RADAR.0 + c * (r - 3.0 * RK), RADAR.1 + s * (r - 3.0 * RK));
        let outer = (RADAR.0 + c * (r + 3.5 * RK), RADAR.1 + s * (r + 3.5 * RK));
        let (i2, o2) = ((inner.0 - c * RK, inner.1 - s * RK), (outer.0 + c * RK, outer.1 + s * RK));
        pen.bar(i2, o2, 5.0 * RK, 0.0, INK);
        pen.bar(inner, outer, 4.0 * RK, 0.0, colour);
    };
    for &blip in shown.blips {
        let at = overlay::radar_mark(shown.own, shown.forward, blip.at, shown.span);
        let colour = match blip.icon {
            Icon::Activity(a) => overlay::activity_colour(a),
            Icon::Machine(m, livery) => overlay::machine_colour(m, livery),
            Icon::Bike => GREY,
            Icon::Point { done } => if done { [0x70, 0x70, 0x70, 255] } else { [0x30, 0xcf, 0x00, 255] },
            Icon::Marker { slot, .. } => overlay::marker_colour(slot),
        };
        if rim(at) {
            chevron(pen, at, colour);
        } else {
            match blip.icon {
                Icon::Activity(a) => {
                    pen.disc(at, 7.5 * RK, shade(153));
                    pen.circle(at, 8.5 * RK, 2.0 * RK, colour);
                    let letter = match a {
                        Activity::Hit => "H",
                        Activity::Run => "R",
                        Activity::Race => "M",
                        Activity::Stunt => "S",
                    };
                    let size = 10.0 * RK;
                    let w = Pen::measure(Face::Words, Weight::Heavy, letter, size);
                    pen.text(Face::Words, Weight::Heavy, letter, (at.0 - w / 2.0, at.1 + Pen::cap(Face::Words, size) / 2.0), size, 0.0, colour);
                }
                Icon::Machine(..) | Icon::Bike => {
                    let h = 7.5 * RK;
                    pen.round([at.0 - h, at.1 - h, 2.0 * h, 2.0 * h], 4.5 * RK, INK, INK);
                    let h = 6.5 * RK;
                    pen.round([at.0 - h, at.1 - h, 2.0 * h, 2.0 * h], 3.5 * RK, colour, colour);
                }
                Icon::Point { .. } => pen.disc(at, 4.0 * RK, colour),
                Icon::Marker { slot, selected } => {
                    if selected {
                        pen.circle(at, 9.4 * RK, 1.6 * RK, CHALK);
                    }
                    pen.disc(at, 6.6 * RK, INK);
                    pen.disc(at, 6.0 * RK, colour);
                    let n = (slot + 1).to_string();
                    let size = 8.0 * RK;
                    let w = Pen::measure(Face::Words, Weight::Heavy, &n, size);
                    pen.text(Face::Words, Weight::Heavy, &n, (at.0 - w / 2.0, at.1 + Pen::cap(Face::Words, size) / 2.0), size, 0.0, INK);
                }
            }
        }
        drawn.icons.push((blip.icon, pen.px(at)));
    }
    for (i, r) in shown.riders.iter().enumerate() {
        let at = overlay::radar_mark(shown.own, shown.forward, r.at, shown.span);
        let colour = if r.ai { overlay::AI_MARK } else { overlay::HUMAN_MARK };
        if rim(at) {
            chevron(pen, at, colour);
        } else {
            pen.disc(at, 4.0 * RK, colour);
        }
        drawn.dots.push((i, pen.px(at), colour));
    }
    // The player: the concept's arrow, ringed blue, pointing the way the rider faces.
    let (tail, tip) = ((RADAR.0, RADAR.1 + 5.0 * RK), (RADAR.0, RADAR.1 - 9.0 * RK));
    pen.bar((tail.0, tail.1 + 1.6 * RK), (tip.0, tip.1 - 2.2 * RK), 8.6 * RK, 0.0, [0x1b, 0x3f, 0xa8, 255]);
    pen.bar(tail, tip, 7.0 * RK, 0.0, overlay::HUMAN_MARK);
    drawn.radar = Some((pen.px(RADAR), RADAR_R * pen.s));
}

/// How near the radar's middle a neighbour's name may come, of the radius: nearer, it is no longer
/// by its letter.
const AROUND_IN: f32 = 0.6;

/// The radar's map names (MD42, inside the disc since MD46): the map the rider is on just under the
/// arrow, and each neighbour's just inside the rim by its compass letter, turning with the radar —
/// in small type, made smaller still (and at last cut short) until the box lies in the disc, by the
/// rim, and off the arrow, the map's own name and the labels already placed.
fn map_names(pen: &mut Pen, shown: &Shown, turn: f32, drawn: &mut Drawn) {
    let meets = |a: [f32; 4], b: [f32; 4]| a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3];
    // Whether a box (left, top, right, bottom in layout pixels) lies in the disc, short of its rim.
    let inside = |b: [f32; 4]| {
        let r = RADAR_R - 2.5 * RK;
        [(b[0], b[1]), (b[2], b[1]), (b[0], b[3]), (b[2], b[3])].iter().all(|p| (p.0 - RADAR.0).powi(2) + (p.1 - RADAR.1).powi(2) <= r * r)
    };
    let boxed = |c: (f32, f32), w: f32, h: f32| [c.0 - w / 2.0, c.1 - h / 2.0, c.0 + w / 2.0, c.1 + h / 2.0];
    let arrow = [RADAR.0 - 6.0 * RK, RADAR.1 - 12.0 * RK, RADAR.0 + 6.0 * RK, RADAR.1 + 8.0 * RK];
    let write = |pen: &mut Pen, text: &str, b: [f32; 4], size: f32, colour: [u8; 4]| {
        let r = pen.edged(Face::Words, Weight::Heavy, text, (b[0], b[3]), size, ink::SLANT, colour, shade(255), 0.9 * RK);
        Line { text: text.to_string(), rect: r, colour }
    };
    // The text at each size from `big` down to `small`, then cut ever shorter at `small`.
    let tries = |name: &str, big: f32, small: f32| {
        let mut out: Vec<(String, f32)> = Vec::new();
        let mut size = big;
        while size > small {
            out.push((name.to_string(), size));
            size -= 0.5 * RK;
        }
        let chars: Vec<char> = name.chars().collect();
        for n in (1..=chars.len()).rev() {
            let cut: String = chars[..n].iter().collect();
            out.push((if n == chars.len() { cut } else { format!("{}.", cut.trim_end()) }, small));
        }
        out
    };
    let mut taken = vec![arrow];
    if let Some(name) = shown.map.filter(|n| !n.is_empty()) {
        let mut last = None;
        for (text, size) in tries(name, 9.0 * RK, 6.0 * RK) {
            let (w, h) = (Pen::measure(Face::Words, Weight::Heavy, &text, size), Pen::cap(Face::Words, size));
            let b = boxed((RADAR.0, arrow[3] + 4.0 * RK + h / 2.0), w, h);
            last = Some((text, size, b));
            if inside(b) {
                break;
            }
        }
        if let Some((text, size, b)) = last {
            drawn.map = Some(write(pen, &text, b, size, CHALK));
            taken.push(b);
        }
    }
    for (k, name) in shown.around.iter().enumerate() {
        let Some(name) = name.filter(|n| !n.is_empty()) else { continue };
        let a = turn + k as f32 * FRAC_PI_2;
        let e = (a.sin(), -a.cos());
        let mut last = None;
        'fit: for (text, size) in tries(name, 7.5 * RK, 5.0 * RK) {
            let (w, h) = (Pen::measure(Face::Words, Weight::Heavy, &text, size), Pen::cap(Face::Words, size));
            // In from the rim along `e`, the farthest out the box lies in the disc — no nearer the
            // middle than [`AROUND_IN`] of the radius, or the text is made smaller.
            let mut t = RADAR_R;
            while t >= AROUND_IN * RADAR_R {
                let b = boxed((RADAR.0 + e.0 * t, RADAR.1 + e.1 * t), w, h);
                if inside(b) {
                    last = Some((text.clone(), size, b));
                    if !taken.iter().any(|&o| meets(b, o)) {
                        break 'fit;
                    }
                    break;
                }
                t -= 0.5 * RK;
            }
        }
        let Some((text, size, b)) = last else { continue };
        let line = write(pen, &text, b, size, BONE);
        taken.push(b);
        drawn.around.push((['N', 'E', 'S', 'W'][k], line));
    }
}

/// A Circle button centred on `at`, `d` across.
fn circle_button(pen: &mut Pen, at: (f32, f32), d: f32) {
    pen.disc(at, d / 2.0, [0x1b, 0x14, 0x10, 255]);
    pen.circle(at, d / 2.0, 0.1 * d / 1.6, [255, 255, 255, 153]);
    pen.circle(at, d * 0.2, d * 0.07, CIRCLE);
}

/// The prompts, low and small at the bottom's middle: the machine switch's question with its black
/// Circle Accept pill (P103), and a challenge's start held on Circle with a ring filling.
fn prompts(pen: &mut Pen, shown: &Shown, drawn: &mut Drawn) {
    pen.corner = Corner::Bottom;
    let pad = 0.3 * EM;
    let act = 0.85 * EM;
    let btn = 1.6 * act;
    let pill_h = btn + 0.4 * act;
    let height = pill_h + 2.0 * pad;
    // Each prompt's width and how to draw it from its left.
    let mut parts: Vec<(f32, Box<dyn Fn(&mut Pen, f32, f32) -> Line>)> = Vec::new();
    let pill = |words: &str| btn + 0.25 * act + 0.35 * act + Pen::measure(Face::Words, Weight::Heavy, words, act) + 0.8 * act;
    if let Some(m) = shown.offer {
        let q = crate::vehicle::offer_text(m).to_string();
        let qw = Pen::measure(Face::Words, Weight::Heavy, &q, EM);
        let w = pad + 0.35 * EM + qw + 0.6 * EM + pill("Accept") + pad;
        parts.push((
            w,
            Box::new(move |pen: &mut Pen, x: f32, top: f32| {
                pen.round([x, top, w, height], 0.8 * EM, shade(115), shade(115));
                let mid = top + height / 2.0;
                let r = pen.text(Face::Words, Weight::Heavy, &q, (x + pad + 0.35 * EM, mid + Pen::cap(Face::Words, EM) / 2.0), EM, ink::SLANT, WHITE);
                act_pill(pen, x + pad + 0.35 * EM + qw + 0.6 * EM, mid, "Accept", None);
                Line { text: q.clone(), rect: r, colour: WHITE }
            }),
        ));
    }
    if let Some(c) = shown.challenge.clone() {
        let ico = 1.9 * 0.95 * EM;
        let nw = Pen::measure(Face::Words, Weight::Heavy, &c.name, EM).max(Pen::measure(Face::Words, Weight::Bold, &c.goal, 0.72 * EM));
        let w = pad + ico + 0.6 * EM + nw + 0.6 * EM + pill("Hold to start") + pad;
        parts.push((
            w,
            Box::new(move |pen: &mut Pen, x: f32, top: f32| {
                pen.round([x, top, w, height], 0.8 * EM, shade(115), shade(115));
                let mid = top + height / 2.0;
                let colour = overlay::activity_colour(c.activity);
                let at = (x + pad + ico / 2.0, mid);
                pen.disc(at, ico / 2.0, colour);
                let letter = match c.activity {
                    Activity::Hit => "H",
                    Activity::Run => "R",
                    Activity::Race => "M",
                    Activity::Stunt => "S",
                };
                let lw = Pen::measure(Face::Words, Weight::Heavy, letter, 0.95 * EM);
                pen.text(Face::Words, Weight::Heavy, letter, (at.0 - lw / 2.0, mid + Pen::cap(Face::Words, 0.95 * EM) / 2.0), 0.95 * EM, 0.0, INK);
                let nx = x + pad + ico + 0.6 * EM;
                let r = pen.text(Face::Words, Weight::Heavy, &c.name, (nx, mid - 0.05 * EM), EM, ink::SLANT, WHITE);
                pen.text(Face::Words, Weight::Bold, &c.goal, (nx, mid + 0.85 * EM), 0.72 * EM, ink::SLANT, BONE);
                act_pill(pen, nx + nw + 0.6 * EM, mid, "Hold to start", Some(c.hold));
                Line { text: c.name.clone(), rect: r, colour: WHITE }
            }),
        ));
    }
    if parts.is_empty() {
        return;
    }
    let gap = 0.8 * EM;
    let total: f32 = parts.iter().map(|p| p.0).sum::<f32>() + gap * (parts.len() - 1) as f32;
    let mut x = overlay::LAYOUT.0 / 2.0 - total / 2.0;
    let top = PROMPT_FOOT - height;
    for (w, f) in &parts {
        let line = f(pen, x, top);
        drawn.prompts.push(line);
        x += w + gap;
    }
}

/// The black pill with the Circle and its word, its left at `x` and its middle at `mid`; a hold's
/// ring round the button, filled to `hold`.
fn act_pill(pen: &mut Pen, x: f32, mid: f32, words: &str, hold: Option<f32>) {
    let act = 0.85 * EM;
    let btn = 1.6 * act;
    let h = btn + 0.4 * act;
    let w = 0.25 * act + btn + 0.35 * act + Pen::measure(Face::Words, Weight::Heavy, words, act) + 0.8 * act;
    pen.round([x, mid - h / 2.0, w, h], h / 2.0, shade(255), shade(255));
    let at = (x + 0.25 * act + btn / 2.0, mid);
    circle_button(pen, at, btn);
    if let Some(k) = hold {
        // The ring: short dashes round the button, as far as the hold has come.
        let r = btn / 2.0 + 0.22 * act;
        let steps = 32;
        for i in 0..(k.clamp(0.0, 1.0) * steps as f32).round() as usize {
            let (a, b) = (i as f32 / steps as f32 * TAU, (i + 1) as f32 / steps as f32 * TAU);
            let p = |t: f32| (at.0 + r * t.sin(), at.1 - r * t.cos());
            pen.bar(p(a), p(b), 0.12 * act, 0.12 * act, CHALK);
        }
    }
    let tx = x + 0.25 * act + btn + 0.35 * act;
    pen.text(Face::Words, Weight::Heavy, words, (tx, mid + Pen::cap(Face::Words, act) / 2.0), act, ink::SLANT, WHITE);
}

/// The top right: the marker menu while it is up, and under it the camera toast.
fn top_right(pen: &mut Pen, shown: &Shown, tracker: &Tracker, now: Instant, drawn: &mut Drawn) {
    pen.corner = Corner::TopRight;
    let (right, mut top) = TOP_RIGHT;
    if let Some(p) = &shown.markers {
        // Never nearer than an EM to the score bar's right (P139), in this corner's layout x.
        let least = overlay::LAYOUT.0 - pen.view[2] / pen.s + SCORE_BAR[0] + SCORE_BAR[2] + EM;
        let d = panel(pen, p, right, top, least);
        top = d.rect_layout_bottom + 0.7 * EM;
        drawn.markers = Some(d.drawn);
    }
    if let Some((name, alpha)) = tracker.toast(now) {
        let size = 1.1 * EM;
        let w = Pen::measure(Face::Words, Weight::Heavy, name, size) + 1.35 * EM;
        let h = size * 1.2 + 0.7 * EM;
        pen.alpha = alpha;
        pen.round([right - w, top, w, h], h / 2.0, shade(128), shade(128));
        let r = pen.text(Face::Words, Weight::Heavy, name, (right - w + 0.9 * EM, top + h / 2.0 + Pen::cap(Face::Words, size) / 2.0), size, ink::SLANT, GREY);
        pen.alpha = 1.0;
        drawn.camera = Some(Line { text: name.to_string(), rect: r, colour: GREY });
        top += h + 0.5 * EM;
    }
    if shown.join {
        drawn.join = Some(join_cue(pen, right, top));
    }
}

/// What the join cue says after its Cross (P171).
pub const JOIN_WORDS: &str = "to join";

/// The join cue (P171), its right edge at `right` and its top at `top`: a dark pill holding
/// "Press", the Cross button and [`JOIN_WORDS`] — the join screen's open seat's words.
fn join_cue(pen: &mut Pen, right: f32, top: f32) -> Line {
    let size = 0.95 * EM;
    let btn = 1.45 * size;
    let gap = 0.35 * size;
    let (pw, jw) = (Pen::measure(Face::Words, Weight::Heavy, "Press", size), Pen::measure(Face::Words, Weight::Heavy, JOIN_WORDS, size));
    let w = 0.9 * EM + pw + gap + btn + gap + jw + 0.9 * EM;
    let h = btn + 0.5 * EM;
    let left = right - w;
    pen.round([left, top, w, h], h / 2.0, shade(128), shade(128));
    let (mid, base) = (top + h / 2.0, top + h / 2.0 + Pen::cap(Face::Words, size) / 2.0);
    let first = pen.text(Face::Words, Weight::Heavy, "Press", (left + 0.9 * EM, base), size, ink::SLANT, WHITE);
    cross_button(pen, (left + 0.9 * EM + pw + gap + btn / 2.0, mid), btn);
    let last = pen.text(Face::Words, Weight::Heavy, JOIN_WORDS, (left + 0.9 * EM + pw + gap + btn + gap, base), size, ink::SLANT, WHITE);
    Line { text: format!("Press \u{2715} {JOIN_WORDS}"), rect: [first[0], first[1].min(last[1]), last[2], first[3].max(last[3])], colour: WHITE }
}

/// The Cross button, `d` across, as the Circle's is drawn: a black disc, its rim and the blue X.
fn cross_button(pen: &mut Pen, at: (f32, f32), d: f32) {
    pen.disc(at, d / 2.0, [0x1b, 0x14, 0x10, 255]);
    pen.circle(at, d / 2.0, 0.1 * d / 1.6, [255, 255, 255, 153]);
    let k = d * 0.2;
    pen.bar((at.0 - k, at.1 - k), (at.0 + k, at.1 + k), d * 0.06, d * 0.06, CROSS);
    pen.bar((at.0 - k, at.1 + k), (at.0 + k, at.1 - k), d * 0.06, d * 0.06, CROSS);
}

/// The join cue alone (P171) in view `view`'s top right, for a ride drawn under the original's HUD,
/// which has none of the ride HUD's pieces to stand among: as sprites into `out`, [`images`] counted
/// from `base`, at the HUD scale `scale`.
pub fn join_sprites(out: &mut Vec<Sprite>, base: usize, view: [f32; 4], scale: f32) -> Line {
    let s = scale_of(view[2], view[3], scale);
    let mut pen = Pen { out, view, s, base, corner: Corner::TopRight, alpha: 1.0 };
    let (right, top) = TOP_RIGHT;
    join_cue(&mut pen, right, top)
}

/// [`join_sprites`] drawn into `frame`, which is the view's picture, on the CPU.
pub fn join_draw(frame: &mut Frame, scale: f32) -> Line {
    let mut out = Vec::new();
    let view = [0.0, 0.0, frame.width as f32, frame.height as f32];
    let line = join_sprites(&mut out, 0, view, scale);
    ink::raster(frame, &out, &images());
    line
}

struct PanelOut {
    drawn: PanelDrawn,
    rect_layout_bottom: f32,
}

/// A d-pad, `d` across, centred on `at`, `lit` its arms that count: left, right, down, up.
fn dpad(pen: &mut Pen, at: (f32, f32), d: f32, lit: [bool; 4]) {
    let (t, l) = (d / 3.0, d / 2.0);
    let dim = [DUST[0], DUST[1], DUST[2], 110];
    pen.round([at.0 - l, at.1 - t / 2.0, d, t], t * 0.3, dim, dim);
    pen.round([at.0 - t / 2.0, at.1 - l, t, d], t * 0.3, dim, dim);
    let arms = [[at.0 - l, at.1 - t / 2.0, l - t / 2.0, t], [at.0 + t / 2.0, at.1 - t / 2.0, l - t / 2.0, t], [at.0 - t / 2.0, at.1 + t / 2.0, t, l - t / 2.0], [at.0 - t / 2.0, at.1 - l, t, l - t / 2.0]];
    for (arm, on) in arms.iter().zip(lit) {
        if on {
            pen.round(*arm, t * 0.3, BONE, BONE);
        }
    }
}

/// The marker menu (the concept's, for `hud::draw_markers`): the dark panel, MARKERS and the
/// machine whose set it is, the five slots in their colours — filled where a marker is, the
/// selected one grown and ringed — the other players' pills, the selection's label, the Down/Up
/// hold's bar filling over [`HOLD_MS`] and the d-pad's legend.
///
/// P139: the + after the last slot while [`Panel::more`], and a row that grows leftward a slot at a
/// time until its left edge would come past `least` (layout x, the pen's corner); past that it keeps
/// its width and scrolls, a window of the row — the slots, the + and the players' pills after them —
/// that always holds the selection, an arrow on a side with more off it; the spawn (P135) stays
/// before it.
fn panel(pen: &mut Pen, p: &Panel, right: f32, top: f32, least: f32) -> PanelOut {
    let (pad_x, pad_top, pad_bottom, gap) = (0.8 * EM, 0.55 * EM, 0.5 * EM, 0.45 * EM);
    let head = 0.72 * EM;
    let slot_d = 1.75 * 0.95 * EM;
    let slot_gap = 0.4 * EM;
    let pill_size = 0.85 * EM;
    let pill_h = pill_size * 1.7;
    let label_size = 1.05 * EM;
    let small = 0.78 * label_size;
    let keys = 0.72 * EM;
    let dp = 1.6 * keys;
    let (label, note) = panel_label(p);
    // Widths.
    let pill_w = |seat: usize, name: &Option<String>| {
        let tag = format!("P{}", seat + 1);
        let mut w = Pen::measure(Face::Words, Weight::Bold, &tag, pill_size) + 1.2 * pill_size;
        if let Some(n) = name.as_ref().filter(|n| !n.trim().is_empty()) {
            w += 0.3 * pill_size + Pen::measure(Face::Words, Weight::Bold, n, pill_size);
        }
        w
    };
    // The spawn (P135) and a rule like the players' after it, before the row and never scrolled.
    let spawn_w = slot_d + slot_gap + 0.5 * EM;
    let legend = if p.pick == Pick::Spawn {
        [("pick", [true, true, false, false]), ("long hold: clear all", [false, false, true, false]), ("hold: ride to", [false, false, false, true])]
    } else {
        [("pick", [true, true, false, false]), ("hold: place", [false, false, true, false]), ("hold: ride to", [false, false, false, true])]
    };
    let legend_w: f32 = legend.iter().map(|(w, _)| dp + 0.3 * keys + Pen::measure(Face::Words, Weight::Bold, w, keys)).sum::<f32>() + 2.0 * 0.9 * keys;
    let label_w = Pen::measure(Face::Words, Weight::Heavy, &label, label_size) + 0.4 * label_size + Pen::measure(Face::Words, Weight::Bold, &note, small);
    let head_w = Pen::spaced_width(Weight::Bold, "MARKERS", head, 0.16 * head) + EM + Pen::spaced_width(Weight::Heavy, &p.machine.to_uppercase(), head, 0.16 * head);
    // The row (P139): the slots, the +, then the players' pills, each its lead (the gap before it)
    // and its width. All of it while the panel fits right of `least`; past that a window of it that
    // holds the selection, grown from the selection outward as far as fits.
    let n = p.filled.len();
    let mut items: Vec<(Pick, f32, f32)> = (0..n).map(|s| (Pick::Slot(s), slot_gap, slot_d)).collect();
    if p.more() {
        items.push((Pick::More, slot_gap, slot_d));
    }
    // The first pill's lead holds the bar that sets the players apart.
    let parted = 0.2 * EM + 0.1 * EM + 0.3 * EM;
    for (i, (seat, name)) in p.others.iter().enumerate() {
        items.push((Pick::Player(*seat), if i == 0 { parted + slot_gap } else { slot_gap }, pill_w(*seat, name)));
    }
    let arrow_w = 0.9 * EM;
    let span = |a: usize, b: usize| items[a..b].iter().map(|i| i.1 + i.2).sum::<f32>() - items[a].1;
    let rest = legend_w.max(label_w).max(head_w);
    let room = ((right - least) - 2.0 * pad_x - spawn_w).max(rest - spawn_w);
    let on = items.iter().position(|i| i.0 == p.pick).unwrap_or(0);
    let (first, last) = if span(0, items.len()) <= room {
        (0, items.len())
    } else {
        let fits = |a: usize, b: usize| span(a, b) + ((a > 0) as u8 + (b < items.len()) as u8) as f32 * arrow_w <= room;
        let (mut a, mut b) = (on, on + 1);
        loop {
            let right_ok = b < items.len() && fits(a, b + 1);
            if right_ok {
                b += 1;
            }
            let left_ok = a > 0 && fits(a - 1, b);
            if left_ok {
                a -= 1;
            }
            if !right_ok && !left_ok {
                break;
            }
        }
        (a, b)
    };
    let arrows = [first > 0, last < items.len()];
    let arrows_w = arrows.iter().filter(|&&a| a).count() as f32 * arrow_w;
    let line_w = spawn_w + span(first, last) + arrows_w;
    let inner = line_w.max(rest);
    let w = inner + 2.0 * pad_x;
    let rows = [head * 1.2, slot_d * 1.18, label_size * 1.3, 0.3 * EM, dp];
    let h = pad_top + rows.iter().sum::<f32>() + gap * (rows.len() - 1) as f32 + pad_bottom;
    let (x, left) = (right - w, right - w + pad_x);
    pen.round([x, top, w, h], 1.2 * EM, shade(128), shade(128));
    let mut drawn = PanelDrawn { rect: pen.rect([x, top, w, h]), label: format!("{label} {note}").trim().to_string(), ..PanelDrawn::default() };
    // The head.
    let mut y = top + pad_top;
    let base = y + rows[0] / 2.0 + Pen::cap(Face::Words, head) / 2.0;
    pen.spaced(Weight::Bold, "MARKERS", (left, base), head, 0.16 * head, ink::SLANT, DUST);
    let machine = p.machine.to_uppercase();
    let mw = Pen::spaced_width(Weight::Heavy, &machine, head, 0.16 * head);
    pen.spaced(Weight::Heavy, &machine, (right - pad_x - mw, base), head, 0.16 * head, ink::SLANT, BONE);
    y += rows[0] + gap;
    // The slots, the + and the players.
    let mid = y + rows[1] / 2.0;
    drawn.first = first;
    drawn.arrows = arrows;
    let chevron = |pen: &mut Pen, at: f32, left: bool| {
        let (w, h) = (0.3 * EM, 0.55 * EM);
        let (tip, back) = if left { (at + 0.25 * EM, at + 0.25 * EM + w) } else { (at + arrow_w - 0.25 * EM, at + arrow_w - 0.25 * EM - w) };
        pen.bar((back, mid - h), (tip, mid), 0.07 * EM, 0.07 * EM, BONE);
        pen.bar((tip, mid), (back, mid + h), 0.07 * EM, 0.07 * EM, BONE);
    };
    let mut at = left;
    {
        let on = p.pick == Pick::Spawn;
        let c = (at + slot_d / 2.0, mid);
        let r = slot_d / 2.0 * if on { 1.18 } else { 1.0 };
        if on {
            pen.disc(c, r + 0.3 * EM, CHALK);
            pen.disc(c, r + 0.14 * EM, INK);
        }
        let ring = if p.home { BONE } else { [245, 240, 232, 89] };
        pen.disc(c, r, shade(77));
        pen.circle(c, r, 0.13 * EM, ring);
        let size = 0.95 * EM * if on { 1.18 } else { 1.0 };
        let nw = Pen::measure(Face::Words, Weight::Heavy, "S", size);
        pen.text(Face::Words, Weight::Heavy, "S", (c.0 - nw / 2.0, mid + Pen::cap(Face::Words, size) / 2.0), size, 0.0, if p.home { CHALK } else { DUST });
        drawn.spawn = (pen.px(c), r * pen.s, on);
        at += slot_d + slot_gap;
        pen.round([at, mid - 0.7 * EM, 0.1 * EM, 1.4 * EM], 0.05 * EM, [245, 240, 232, 64], [245, 240, 232, 64]);
        at += 0.5 * EM;
    }
    if arrows[0] {
        chevron(pen, at, true);
        at += arrow_w;
    }
    for (k, &(item, lead, width)) in items.iter().enumerate().take(last).skip(first) {
        if k > first {
            at += lead;
            if matches!(item, Pick::Player(_)) && lead > slot_gap {
                let bar = at - lead + 0.2 * EM;
                pen.round([bar, mid - 0.7 * EM, 0.1 * EM, 1.4 * EM], 0.05 * EM, [245, 240, 232, 64], [245, 240, 232, 64]);
            }
        }
        let on = p.pick == item;
        match item {
            Pick::Slot(_) | Pick::More => {
                let plus = item == Pick::More;
                let s = if let Pick::Slot(s) = item { s } else { n };
                let colour = if plus { CHALK } else { overlay::marker_colour(s) };
                let set = !plus && p.filled[s];
                let c = (at + slot_d / 2.0, mid);
                let r = slot_d / 2.0 * if on { 1.18 } else { 1.0 };
                if on {
                    pen.disc(c, r + 0.3 * EM, CHALK);
                    pen.disc(c, r + 0.14 * EM, INK);
                }
                if set {
                    pen.disc(c, r, colour);
                } else {
                    pen.disc(c, r, shade(77));
                    pen.circle(c, r, 0.13 * EM, if plus { [CHALK[0], CHALK[1], CHALK[2], 140] } else { colour });
                }
                if plus {
                    let arm = 0.45 * r;
                    pen.bar((c.0 - arm, mid), (c.0 + arm, mid), 0.09 * EM, 0.09 * EM, CHALK);
                    pen.bar((c.0, mid - arm), (c.0, mid + arm), 0.09 * EM, 0.09 * EM, CHALK);
                    drawn.more = Some((pen.px(c), r * pen.s, on));
                } else {
                    let label = (s + 1).to_string();
                    let size = 0.95 * EM * if on { 1.18 } else { 1.0 };
                    let nw = Pen::measure(Face::Words, Weight::Heavy, &label, size);
                    pen.text(Face::Words, Weight::Heavy, &label, (c.0 - nw / 2.0, mid + Pen::cap(Face::Words, size) / 2.0), size, 0.0, if set { INK } else { CHALK });
                    drawn.slots.push((pen.px(c), r * pen.s, on, set));
                }
            }
            Pick::Player(seat) => {
                let name = p.others.iter().find(|(o, _)| *o == seat).and_then(|(_, n)| n.as_ref());
                let b = [at, mid - pill_h / 2.0, width, pill_h];
                if on {
                    pen.round(b, pill_h / 2.0, MAROON.0, MAROON.1);
                }
                pen.ring(b, pill_h / 2.0, 0.1 * pill_size, if on { CHALK } else { [245, 240, 232, 89] });
                let tag = format!("P{}", seat + 1);
                let base = mid + Pen::cap(Face::Words, pill_size) / 2.0;
                let mut tx = at + 0.6 * pill_size;
                pen.text(Face::Words, Weight::Bold, &tag, (tx, base), pill_size, ink::SLANT, if on { BONE } else { DUST });
                tx += Pen::measure(Face::Words, Weight::Bold, &tag, pill_size) + 0.3 * pill_size;
                if let Some(n) = name.filter(|n| !n.trim().is_empty()) {
                    pen.text(Face::Words, Weight::Bold, n, (tx, base), pill_size, ink::SLANT, if on { CHALK } else { BONE });
                }
                drawn.players.push((seat, on, pen.rect(b)));
            }
            Pick::Spawn => unreachable!("the spawn is drawn before the row"),
        }
        at += width;
    }
    if arrows[1] {
        chevron(pen, at, false);
    }
    y += rows[1] + gap;
    // The label.
    let base = y + rows[2] / 2.0 + Pen::cap(Face::Words, label_size) / 2.0;
    pen.text(Face::Words, Weight::Heavy, &label, (left, base), label_size, ink::SLANT, GREY);
    let lw = Pen::measure(Face::Words, Weight::Heavy, &label, label_size);
    pen.text(Face::Words, Weight::Bold, &note, (left + lw + 0.4 * label_size, base), small, ink::SLANT, DUST);
    y += rows[2] + gap;
    // The hold.
    let k = p.holding.min(HOLD_MS) as f32 / HOLD_MS as f32;
    let fired = p.holding >= HOLD_MS;
    pen.round([left, y, inner, rows[3]], rows[3] / 2.0, [255, 255, 255, 31], [255, 255, 255, 31]);
    if k > 0.0 {
        let c = if fired { FIRED } else { BONE };
        pen.round([left, y, (inner * k).max(rows[3]), rows[3]], rows[3] / 2.0, c, c);
    }
    drawn.hold = k;
    drawn.fired = fired;
    y += rows[3] + gap;
    // The legend.
    let mid = y + dp / 2.0;
    let mut at = left;
    for (words, lit) in legend {
        dpad(pen, (at + dp / 2.0, mid), dp, lit);
        at += dp + 0.3 * keys;
        pen.text(Face::Words, Weight::Bold, words, (at, mid + Pen::cap(Face::Words, keys) / 2.0), keys, ink::SLANT, DUST);
        at += Pen::measure(Face::Words, Weight::Bold, words, keys) + 0.9 * keys;
    }
    PanelOut { drawn, rect_layout_bottom: top + h }
}

/// How long [`Tracker`]'s pieces last, for the tests: the pop in, a banked combo's hold, the
/// play-out, the score's flash and the camera toast.
pub const TIMES: [Duration; 5] = [
    Duration::from_millis((POP * 1000.0) as u64),
    Duration::from_millis((HOLD * 1000.0) as u64),
    Duration::from_millis((OUT * 1000.0) as u64),
    Duration::from_millis((FLASH * 1000.0) as u64),
    Duration::from_millis((TOAST * 1000.0) as u64),
];
