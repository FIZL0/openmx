//! P327: Race on the main menu, beside Free Ride — its front end. The race-track pick (the disc's
//! Supercross and National tracks, Left and Right between the two), the race's setup (laps and
//! riders), and the results screen after the ride.
//!
//! What the simulation has of a race so far is its start: the grid, the closed gate and its drop
//! (`game_sim::race`). Laps, places and the finish are S293/S294/S13c/S13d's, and the AI riders
//! S12c/S12e's, so the results here are drawn from [`Results`], a stand-in shaped as a race's results
//! are, whose lap and place fields are `None` — shown as dashes — until the simulation has them
//! (P313 wires them in).

use super::results::{Board, FameMarks, HandOver, Records, Tip, tips_of};
use super::{Act, Menu, Press, Screen};
use ps2_float::F;

/// The two kinds of race track the pick steps between, as the original's front end names them.
pub const RACE_KINDS: [&str; 2] = ["Supercross", "National"];

/// How many Supercross tracks the disc has: `Tracks\sx01.res` to `sx24.res`.
pub const SUPERCROSS: usize = 24;

/// The Supercross tracks by the disc's names, their stadiums' cities: the STR_SX01..STR_SX24
/// strings of appstart/loc's l_ui_eng.bin, which tracks.dat's `[SXnn] TextID` points at.
pub const RACE_SUPERCROSS: [(&str, &str); SUPERCROSS] = [
    ("sx01", "Los Angeles"),
    ("sx02", "Jackson"),
    ("sx03", "San Diego"),
    ("sx04", "Phoenix"),
    ("sx05", "Memphis"),
    ("sx06", "San Antonio"),
    ("sx07", "Lincoln"),
    ("sx08", "Denver"),
    ("sx09", "Provo"),
    ("sx10", "Las Vegas"),
    ("sx11", "Detroit"),
    ("sx12", "Pittsburgh"),
    ("sx13", "Lynchburg"),
    ("sx14", "Charlotte"),
    ("sx15", "Chicago"),
    ("sx16", "Springfield"),
    ("sx17", "Dallas"),
    ("sx18", "Norfolk"),
    ("sx19", "Bridgeport"),
    ("sx20", "Rochester"),
    ("sx21", "Cleveland"),
    ("sx22", "Cincinnati"),
    ("sx23", "Houston"),
    ("sx24", "Atlanta"),
];

/// The National tracks by the disc's names: [`crate::worlds::NATIONALS`]'s, copied here as the
/// test crates mount the menu without the worlds.
pub const RACE_NATIONALS: [(&str, &str); 22] = [
    ("nat01", "Seneca Butte"),
    ("nat02", "Bodie"),
    ("nat03", "Racine: Long"),
    ("nat04", "Doha"),
    ("nat05", "Blueridge"),
    ("nat06", "Summerhaven"),
    ("nat07", "Pine Bluff"),
    ("nat08", "Slack Jaw"),
    ("nat09", "Goblin Valley"),
    ("nat10", "Foxpark"),
    ("nat11", "Hillsboro"),
    ("nat12", "Black Hills"),
    ("nat13", "Rattlesnake Pass"),
    ("nat14", "Grandview"),
    ("nat15", "Cedar Mills"),
    ("nat16", "Casa De Cobre"),
    ("nat17", "Waimea"),
    ("nat18", "Manassas Park"),
    ("nat19", "Ensenada"),
    ("nat20", "Sliderock"),
    ("nat21", "Signal Butte"),
    ("nat22", "Racine: Short"),
];

/// The setup's ranges: laps from one to [`MOST_LAPS`], riders from one to the grid's eight lanes.
pub const MOST_LAPS: u32 = 20;
pub const MOST_RIDERS: u32 = game_sim::race::LANES as u32;

/// The setup screen's rows.
pub const SETUP_ROWS: [&str; 3] = ["Laps", "Riders", "Start Race"];
const ROW_LAPS: usize = 0;
const ROW_RIDERS: usize = 1;
const ROW_START: usize = 2;

/// A race as the setup picked it: the track, the laps and how many riders the grid is laid out for
/// (the original's `NumberOfRacers`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RaceSetup {
    pub track: String,
    pub laps: u32,
    pub riders: u32,
}

impl Default for RaceSetup {
    fn default() -> RaceSetup {
        RaceSetup { track: "sx01".into(), laps: 3, riders: 4 }
    }
}

/// One rider's line on the results screen. The fields the simulation does not have yet are `None`
/// and drawn as dashes.
#[derive(Clone, Debug, PartialEq)]
pub struct ResultRow {
    pub name: String,
    pub number: String,
    pub place: Option<u32>,
    pub laps: Option<u32>,
    /// The best lap and the whole race, in seconds.
    pub best_lap: Option<f32>,
    pub total: Option<f32>,
}

/// A race's results, as the results screen draws them: the stand-in for the simulation's own
/// (`race::Results`, S13d), one row a rider in finishing order.
#[derive(Clone, Debug, PartialEq)]
pub struct Results {
    pub track: String,
    pub laps: u32,
    pub riders: u32,
    pub rows: Vec<ResultRow>,
    /// P330: the results menu's pages, when the race handed its cards over; the page shown; what
    /// the screen is showing of the race's end; the tip popup's lines; the Hall of Fame's marks.
    pub board: Option<Board>,
    pub page: usize,
    pub stage: Stage,
    pub tips: Vec<Tip>,
    pub fame: FameMarks,
}

/// What the results screen shows of the race's end, in turn: the preload and clutch tip popup
/// (armed at the race's end, so first), the results menu's pages, and the Hall of Fame's call for
/// a new entry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stage {
    Tips,
    #[default]
    Board,
    Fame,
}

impl Results {
    /// What a race's end can say before the simulation counts laps and places: the human's row with
    /// every number unknown.
    pub fn stand_in(setup: &RaceSetup, name: &str, number: &str) -> Results {
        Results {
            track: setup.track.clone(),
            laps: setup.laps,
            riders: setup.riders,
            rows: vec![ResultRow { name: name.to_string(), number: number.to_string(), place: None, laps: None, best_lap: None, total: None }],
            board: None,
            page: 0,
            stage: Stage::Board,
            tips: Vec::new(),
            fame: FameMarks::default(),
        }
    }

    /// The results of a race that handed its end over (P330): the results menu's pages from the
    /// cards when `race_end` handed them on (a single race), the tip popup's lines, and the Hall
    /// of Fame marks each player's finish earned.
    pub fn handed(setup: &RaceSetup, name: &str, number: &str, hand: &HandOver) -> Results {
        let mut r = Results::stand_in(setup, name, number);
        if hand.end.results {
            r.board = Some(Board::take(&hand.cards, false));
            r.tips = tips_of(&hand.end.tips);
        }
        for e in hand.overs.iter().flat_map(|o| &o.fame) {
            r.fame.mark(e.seat, e.track);
        }
        r.stage = if r.tips.is_empty() { Stage::Board } else { Stage::Tips };
        r
    }

    /// Cross on the results screen: on to the next of its stages, `false` when it was the last.
    pub fn next_stage(&mut self) -> bool {
        let next = match self.stage {
            Stage::Tips => Some(Stage::Board),
            Stage::Board if self.fame.waiting => Some(Stage::Fame),
            _ => None,
        };
        next.map(|n| self.stage = n).is_some()
    }
}

/// A number the simulation may not have, or a dash.
pub fn or_dash(n: Option<u32>) -> String {
    n.map_or_else(|| "-".to_string(), |n| n.to_string())
}

/// A time in seconds as `m:ss.cc`, or dashes.
pub fn time_or_dash(t: Option<f32>) -> String {
    match t {
        Some(t) => {
            let cs = (t.max(0.0) * 100.0).round() as u32;
            format!("{}:{:02}.{:02}", cs / 6000, cs / 100 % 60, cs % 100)
        }
        None => "-:--.--".to_string(),
    }
}

/// The Supercross track `k` (from 0), by its archive.
pub fn supercross_id(k: usize) -> String {
    format!("sx{:02}", k + 1)
}

/// The race tracks of kind `kind` ([`RACE_KINDS`]), `(id, name)`, by the disc's names
/// ([`RACE_SUPERCROSS`], [`RACE_NATIONALS`]).
pub fn race_tracks(kind: usize) -> Vec<(String, String)> {
    let tracks: &[(&str, &str)] = match kind {
        0 => &RACE_SUPERCROSS,
        _ => &RACE_NATIONALS,
    };
    tracks.iter().map(|(id, name)| (id.to_string(), name.to_string())).collect()
}

/// A race track's name, or `None` for a track that is not one.
pub fn race_track_name(track: &str) -> Option<String> {
    (0..RACE_KINDS.len()).flat_map(race_tracks).find(|(id, _)| id == track).map(|(_, n)| n)
}

/// A race track's kind ([`RACE_KINDS`]), or `None`.
pub fn race_kind_of(track: &str) -> Option<usize> {
    (0..RACE_KINDS.len()).find(|&k| race_tracks(k).iter().any(|(id, _)| id == track))
}

/// The league a disc track is ridden in, as the original's bike builder reads it off the event
/// (P332): a Supercross track (`sxNN`, [`RACE_SUPERCROSS`]) is a stadium, a National
/// ([`RACE_NATIONALS`]) is outdoors, and everything else — the free-ride towns, the disc worlds —
/// is a free ride. The builder arms the map edge only outdoors ([`game_sim::off_map::armed`]), so a
/// Supercross rider is never shot off, however near the stadium's edge the grid stands.
pub fn league_of(track: &str) -> game_sim::off_map::League {
    use game_sim::off_map::League;
    let is = |list: &[(&str, &str)]| list.iter().any(|(id, _)| id.eq_ignore_ascii_case(track));
    if is(&RACE_SUPERCROSS) {
        League::Supercross
    } else if is(&RACE_NATIONALS) {
        League::Nationals
    } else {
        League::FreeRide
    }
}

/// The front end's race state: the pick's kind, the setup being made, the race under way and the
/// last race's results.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RaceMenu {
    pub kind: usize,
    pub setup: RaceSetup,
    /// The ride under the menu is this race: set when the setup's Start Race starts it, cleared by
    /// a Free Ride's start and when the race is left.
    pub racing: Option<RaceSetup>,
    /// The results screen's, once a race has been left.
    pub results: Option<Results>,
    /// P330: what the race under way handed over at its end, for the results.
    pub hand: Option<HandOver>,
    /// The riders' records (`records.txt`), and whether a race's win changed them since the shell
    /// last saved them.
    pub records: Records,
    pub records_changed: bool,
}

impl Menu {
    /// Main menu's Race: the track pick, on the kind and track last set up.
    pub(super) fn open_race(&mut self) {
        self.race.kind = race_kind_of(&self.race.setup.track).unwrap_or(0);
        let at = race_tracks(self.race.kind).iter().position(|(id, _)| *id == self.race.setup.track).unwrap_or(0);
        self.cursor[Screen::Race.index()] = at;
        self.push(Screen::Race);
    }

    /// The rows the race screens have.
    pub(super) fn race_rows(&self, screen: Screen) -> usize {
        match screen {
            Screen::Race => race_tracks(self.race.kind).len(),
            Screen::RaceSetup => SETUP_ROWS.len(),
            _ => 1,
        }
    }

    /// One press on a race screen, after Up and Down have walked its rows.
    pub(super) fn race_press(&mut self, screen: Screen, at: usize, p: Press) -> Act {
        match screen {
            Screen::Race => {
                if p.left || p.right || p.l1 || p.r1 {
                    let n = RACE_KINDS.len();
                    self.race.kind = if p.left || p.l1 { (self.race.kind + n - 1) % n } else { (self.race.kind + 1) % n };
                    self.cursor[Screen::Race.index()] = 0;
                } else if p.ok || p.start {
                    if let Some((id, _)) = race_tracks(self.race.kind).get(at) {
                        self.race.setup.track = id.clone();
                        self.cursor[Screen::RaceSetup.index()] = ROW_START;
                        self.push(Screen::RaceSetup);
                    }
                } else if p.back {
                    self.pop();
                }
            }
            Screen::RaceSetup => {
                let step = |v: u32, most: u32, down: bool| if down { if v <= 1 { most } else { v - 1 } } else if v >= most { 1 } else { v + 1 };
                let s = &mut self.race.setup;
                match at {
                    ROW_LAPS if p.left || p.right || p.ok => s.laps = step(s.laps, MOST_LAPS, p.left),
                    ROW_RIDERS if p.left || p.right || p.ok => s.riders = step(s.riders, MOST_RIDERS, p.left),
                    ROW_START if p.ok || p.start => return self.start_race(),
                    _ if p.start => return self.start_race(),
                    _ if p.back => self.pop(),
                    _ => {}
                }
            }
            Screen::Results => {
                let pages = super::results::PAGES.len();
                if let Some(r) = self.race.results.as_mut().filter(|r| r.board.is_some() && r.stage == Stage::Board)
                    && (p.left || p.right || p.l1 || p.r1)
                {
                    r.page = if p.left || p.l1 { (r.page + pages - 1) % pages } else { (r.page + 1) % pages };
                } else if p.ok || p.back || p.start {
                    if !self.race.results.as_mut().is_some_and(Results::next_stage) {
                        self.race.results = None;
                        self.stack = vec![Screen::Main];
                    }
                }
            }
            _ => {}
        }
        Act::Stay
    }

    /// The setup's Start Race: a ride on the race's track, the race under way.
    fn start_race(&mut self) -> Act {
        let setup = self.race.setup.clone();
        if self.party.take().is_some() {
            self.riders = self.free_riders;
        }
        self.race.racing = Some(setup.clone());
        self.race.hand = None;
        self.ride_stack = std::mem::take(&mut self.stack);
        Act::Ride(setup.track)
    }

    /// The race under way, if the ride under the menu is one.
    pub fn racing(&self) -> Option<&RaceSetup> {
        self.race.racing.as_ref()
    }

    /// The setup the next race starts with, which `--race` and the tests set.
    pub fn set_race(&mut self, setup: RaceSetup) {
        self.race.kind = race_kind_of(&setup.track).unwrap_or(0);
        self.race.setup = setup;
    }

    /// The race left (or ended): its results to show on the way back to the main menu, from the
    /// human's garage name and number. Nothing for a ride that was no race.
    pub fn race_left(&mut self) -> bool {
        let Some(setup) = self.race.racing.take() else { return false };
        let name = self.rider_name(0);
        let number = self.garages.first().map(|g| g.pick.look.number.clone()).unwrap_or_default();
        self.race.results = Some(match self.race.hand.take() {
            Some(hand) => Results::handed(&setup, &name, &number, &hand),
            None => Results::stand_in(&setup, &name, &number),
        });
        self.stack = vec![Screen::Main, Screen::Results];
        true
    }

    /// A race's ride that did not start (its load failed or was cancelled): no results.
    pub fn race_dropped(&mut self) {
        self.race.racing = None;
        self.race.hand = None;
    }

    /// The race under way has ended (P330): what its two answers handed over, kept for the results
    /// the ride's leave shows. A player's win counts in their seat's record now, as the original's
    /// profile counts it at the finish (`race_over`'s [`game_sim::race::RaceOver::win`]).
    pub fn race_ended(&mut self, hand: HandOver) {
        if self.race.racing.is_none() {
            return;
        }
        for seat in hand.overs.iter().filter_map(|o| o.win) {
            if let Ok(seat) = usize::try_from(seat) {
                self.race.records.seat(seat).add_wins(F(1.0));
                self.race.records_changed = true;
            }
        }
        self.race.hand = Some(hand);
    }

    /// The name a seat's rider goes by on the results (its profile's), for the cards the race hands
    /// over (S301).
    pub fn player_name(&self, seat: usize) -> String {
        self.rider_name(seat)
    }

    /// Whether the riders' records changed since the last call, for the shell to save them.
    pub fn take_records_changed(&mut self) -> bool {
        std::mem::take(&mut self.race.records_changed)
    }

    /// The results the results screen shows: the last race's, or for `--screen results` a stand-in
    /// of the setup.
    pub fn results(&self) -> Results {
        self.race.results.clone().unwrap_or_else(|| Results::stand_in(&self.race.setup, &self.rider_name(0), ""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_supercross_and_national_tracks_are_listed_by_their_archives() {
        let sx = race_tracks(0);
        assert_eq!(sx.len(), 24);
        assert_eq!(sx[0].0, "sx01");
        assert_eq!(sx[23].0, "sx24");
        assert!(sx.iter().enumerate().all(|(k, (id, _))| *id == supercross_id(k)));
        assert_eq!(sx[0].1, "Los Angeles");
        assert_eq!(sx[23].1, "Atlanta");
        assert_eq!(race_track_name("sx16"), Some("Springfield".to_string()));
        let nat = race_tracks(1);
        assert_eq!(nat.len(), 22);
        assert_eq!(nat[0], ("nat01".to_string(), "Seneca Butte".to_string()));
        assert_eq!(race_kind_of("nat05"), Some(1));
        assert_eq!(race_kind_of("free06"), None);
    }

    /// P332: a Supercross race's bike has no map edge, a National's and a free ride's do — the
    /// stadium's grid stands inside the 150 ft margin, and an armed edge threw the rider at spawn.
    #[test]
    fn only_the_supercross_tracks_disarm_the_edge() {
        use game_sim::off_map::{Built, League, armed};
        for (id, _) in race_tracks(0) {
            assert_eq!(league_of(&id), League::Supercross, "{id}");
            assert!(!armed(Built::Player, league_of(&id)), "{id}");
        }
        assert_eq!(league_of("SX07"), League::Supercross);
        for (id, _) in race_tracks(1) {
            assert_eq!(league_of(&id), League::Nationals, "{id}");
            assert!(armed(Built::Player, league_of(&id)), "{id}");
        }
        for id in ["free06", "free09", "world_mxu", "some_mod"] {
            assert_eq!(league_of(id), League::FreeRide, "{id}");
            assert!(armed(Built::Player, league_of(id)), "{id}");
        }
    }

    #[test]
    fn unknown_numbers_are_dashes() {
        assert_eq!(or_dash(None), "-");
        assert_eq!(or_dash(Some(3)), "3");
        assert_eq!(time_or_dash(None), "-:--.--");
        assert_eq!(time_or_dash(Some(83.456)), "1:23.46");
    }
}
