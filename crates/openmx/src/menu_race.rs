//! P327: Race on the main menu, beside Free Ride — its front end. The race-track pick (the disc's
//! Supercross and National tracks, Left and Right between the two), the race's setup (laps and
//! riders), and the results screen after the ride.
//!
//! What the simulation has of a race so far is its start: the grid, the closed gate and its drop
//! (`game_sim::race`). Laps, places and the finish are S293/S294/S13c/S13d's, and the AI riders
//! S12c/S12e's, so the results here are drawn from [`Results`], a stand-in shaped as a race's results
//! are, whose lap and place fields are `None` — shown as dashes — until the simulation has them
//! (P313 wires them in).

use super::{Act, Menu, Press, Screen};

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
        }
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
                if p.ok || p.back || p.start {
                    self.race.results = None;
                    self.stack = vec![Screen::Main];
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
        self.race.results = Some(Results::stand_in(&setup, &name, &number));
        self.stack = vec![Screen::Main, Screen::Results];
        true
    }

    /// A race's ride that did not start (its load failed or was cancelled): no results.
    pub fn race_dropped(&mut self) {
        self.race.racing = None;
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

    #[test]
    fn unknown_numbers_are_dashes() {
        assert_eq!(or_dash(None), "-");
        assert_eq!(or_dash(Some(3)), "3");
        assert_eq!(time_or_dash(None), "-:--.--");
        assert_eq!(time_or_dash(Some(83.456)), "1:23.46");
    }
}
