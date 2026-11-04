//! **Back where you left off** (P298): where the player was when the app went away — a menu screen,
//! or the ride — written beside `settings.txt` as it goes, and offered on the next launch when iOS
//! has ended the suspended app in between rather than resuming it.
//!
//! iOS keeps a suspended app in memory and resumes it as it was; the relaunch the user saw is iOS
//! ending it, most likely for its memory while it sat in the background. Going away (the app
//! resigning active, then entering the background) pauses the ride and writes the point here; coming
//! back to the same process takes the file away again, so it is only there while the app is away and
//! a launch finds it only after the app was ended away. A launch that finds one offers it on the main
//! menu once the splash has 1P ([`crate::menu::Offer`]), and the point is spent either way.
//!
//! **The ride** is its track and its first seat's pick (bike, set, rider and look), and the seat's
//! whole state in the resync's codec ([`crate::net::rides::seat_bytes`], [`game_sim::snapshot`]):
//! where the bike is, its speed and spin, the rider, the markers, the event's score, the track's cars
//! and props. Resumed, the ride loads as a new one on that pick and the state is put over it, the way
//! a resync puts a host's state over a joiner's: the same tick, the same everything, the pause up. A
//! point written by another build keeps its track and pick and drops the state, since the codec may
//! read it otherwise, and the ride starts from the spawn. Only a local ride of one player on the
//! bike saves its state; a split screen, an online ride, a played tape or a seat on another machine
//! is offered as its track from the spawn, or not at all online.
//!
//! The file is text, `key = value` a line, the state in hex, written to a side file and renamed over
//! the last one so a write cut short by iOS never leaves half a point.

use std::path::{Path, PathBuf};

/// The file beside `settings.txt`.
pub const FILE: &str = "resume.txt";

/// Where the player was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Point {
    /// A menu screen, by its `--screen` word ([`crate::menu::Screen::word`]).
    Screen(String),
    Ride(Ride),
}

/// A ride to resume.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ride {
    pub track: String,
    /// The first seat's pick: the bike archive, its `.phy` set's word, the rider archive and the look's
    /// text, as `garage.txt` spells them.
    pub bike: String,
    pub set: String,
    pub rider: String,
    pub look: String,
    /// The seat's state ([`crate::net::rides::seat_bytes`]), or none to ride from the spawn.
    pub state: Option<Vec<u8>>,
}

impl Point {
    /// The point as the file holds it, for build `build`.
    pub fn text(&self, build: &str) -> String {
        let mut out = String::from("# openmx's resume point (P298): where the player was when the app went away\n");
        out.push_str(&format!("build = {build}\n"));
        match self {
            Point::Screen(word) => out.push_str(&format!("screen = {word}\n")),
            Point::Ride(r) => {
                for (key, value) in [("track", &r.track), ("bike", &r.bike), ("set", &r.set), ("rider", &r.rider), ("look", &r.look)] {
                    out.push_str(&format!("{key} = {}\n", value.replace('\n', " ")));
                }
                if let Some(state) = &r.state {
                    let hex: String = state.iter().map(|b| format!("{b:02x}")).collect();
                    out.push_str(&format!("state = {hex}\n"));
                }
            }
        }
        out
    }

    /// [`Point::text`] back, read by build `build`: none for a file that names neither a screen nor
    /// a track. Another build's state is dropped (see the module docs), said in the second value.
    pub fn parse(text: &str, build: &str) -> Option<(Point, Option<String>)> {
        let mut values = std::collections::HashMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                values.insert(k.trim(), v.trim());
            }
        }
        let get = |k: &str| values.get(k).map(|v| v.to_string()).unwrap_or_default();
        if let Some(word) = values.get("screen").filter(|w| !w.is_empty()) {
            return Some((Point::Screen(word.to_string()), None));
        }
        let track = get("track");
        if track.is_empty() {
            return None;
        }
        let mut said = None;
        let state = match values.get("state") {
            None => None,
            Some(_) if get("build") != build => {
                said = Some(format!("the ride's state is build {}'s, not this one's: riding {track} from its start", get("build")));
                None
            }
            Some(hex) => match unhex(hex) {
                Some(bytes) => Some(bytes),
                None => {
                    said = Some(format!("the ride's state is not hex: riding {track} from its start"));
                    None
                }
            },
        };
        Some((Point::Ride(Ride { track, bike: get("bike"), set: get("set"), rider: get("rider"), look: get("look"), state }), said))
    }
}

fn unhex(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok()).collect()
}

/// The file in the settings' folder `dir`.
pub fn path(dir: &Path) -> PathBuf {
    dir.join(FILE)
}

/// Write `point` into `dir` for build `build`: a side file renamed over the last point.
pub fn save(dir: &Path, point: &Point, build: &str) -> Result<(), String> {
    let to = path(dir);
    let side = dir.join(format!("{FILE}.new"));
    std::fs::write(&side, point.text(build)).map_err(|e| format!("{}: {e}", side.display()))?;
    std::fs::rename(&side, &to).map_err(|e| format!("{}: {e}", to.display()))
}

/// The point in `dir`, if there is one, for build `build`, and what was dropped of it.
pub fn load(dir: &Path, build: &str) -> Option<(Point, Option<String>)> {
    Point::parse(&std::fs::read_to_string(path(dir)).ok()?, build)
}

/// The point spent, or the app back before it was needed.
pub fn clear(dir: &Path) {
    let _ = std::fs::remove_file(path(dir));
}

/// Whether this platform goes away and comes back (iOS): elsewhere nothing writes a point.
pub const ON: bool = cfg!(target_os = "ios");

/// The app has gone away since [`answered`] was last called: the count to answer with.
pub fn away() -> Option<usize> {
    #[cfg(target_os = "ios")]
    return openmx_ios::away();
    #[cfg(not(target_os = "ios"))]
    None
}

/// Going away is answered: the ride is paused and the point written, up to [`away`]'s `asked`.
pub fn answered(asked: usize) {
    #[cfg(target_os = "ios")]
    openmx_ios::answered_away(asked);
    #[cfg(not(target_os = "ios"))]
    let _ = asked;
}

/// Whether the app came back to the foreground since the last call: its point is not needed.
pub fn back() -> bool {
    #[cfg(target_os = "ios")]
    return openmx_ios::take_back();
    #[cfg(not(target_os = "ios"))]
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_sim::input::{Control, PADS, Report};
    use game_sim::world::Flat;
    use ps2_float::F;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("openmx-resume-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Cross held all the way down, the bars at `bars`.
    fn gas(bars: u8) -> Report {
        let mut r = Report::from_buttons(Control::Cross.mask());
        r.0[14] = 0xff;
        r.set_axis(2, bars);
        r
    }

    /// A seat ridden `ticks` ticks on the gas and the stick, as the shell's first seat is built.
    fn ridden(ticks: u32) -> game_sim::Sim {
        let mut sim = game_sim::Sim::new(7);
        sim.rider.body_rates = game_render::vehicle::params();
        sim.rider.no_start_line();
        let mut world = Flat::default();
        for t in 0..ticks {
            let mut reports = [Report::missing(); PADS];
            reports[0] = gas((128 + (t % 90) as i32 - 45) as u8);
            sim.frame(F(1.0 / 30.0), &reports, &mut world);
        }
        sim
    }

    #[test]
    fn a_screen_point_restores_the_same_screen() {
        let d = dir("screen");
        let mut kept = 0;
        for at in crate::menu::Screen::ALL {
            // the screen the menu shows, as the shell asks it on going away
            let mut was = crate::menu::Menu::new(Default::default(), Default::default());
            was.open(at);
            let Some(screen) = was.screen().and_then(crate::menu::Screen::resumed_as) else { continue };
            kept += 1;
            save(&d, &Point::Screen(screen.word().into()), "b1").unwrap();
            let (point, said) = load(&d, "b1").expect("a point");
            assert_eq!(said, None);
            let Point::Screen(word) = point else { panic!("{screen:?}: not a screen") };
            let mut menu = crate::menu::Menu::new(Default::default(), Default::default());
            menu.open(crate::menu::Screen::from_word(&word).expect("a screen's word"));
            assert_eq!(menu.screen(), Some(screen), "{word} (left on {at:?}): another screen opened");
        }
        assert!(kept >= 8, "only {kept} screens are kept");
        clear(&d);
        assert!(load(&d, "b1").is_none(), "a cleared point is gone");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_ride_point_restores_the_same_ride() {
        let d = dir("ride");
        let sim = ridden(240);
        let at = sim.rider.ride.position();
        assert!(at.x().abs() + at.z().abs() > 1.0, "the ride went nowhere: {at:?}");
        let ride = Ride {
            track: "free06".into(),
            bike: "125sx".into(),
            set: "race125p".into(),
            rider: "rider1".into(),
            look: "a, 1, 2, 3, 4, 7, Rider = Me".into(),
            state: Some(crate::net::rides::seat_bytes(&sim)),
        };
        save(&d, &Point::Ride(ride.clone()), "b1").unwrap();
        let (point, said) = load(&d, "b1").expect("a point");
        assert_eq!(said, None);
        assert_eq!(point, Point::Ride(ride.clone()), "the file gave another ride back");
        // put over a new ride's seat, as the shell does: the same state, tick for tick after it
        let Point::Ride(back) = point else { unreachable!() };
        let mut resumed = game_sim::Sim::new(0);
        resumed.rider.body_rates = game_render::vehicle::params();
        crate::net::rides::from_seat_bytes(&mut resumed, back.state.as_deref().unwrap()).unwrap();
        assert_eq!(resumed.hash(), sim.hash(), "the resumed state is not the one saved");
        let (mut a, mut b) = (sim, resumed);
        let mut world = Flat::default();
        for t in 0..60 {
            let mut reports = [Report::missing(); PADS];
            reports[0] = gas(0x80);
            a.frame(F(1.0 / 30.0), &reports, &mut world);
            b.frame(F(1.0 / 30.0), &reports, &mut world);
            assert_eq!(a.hash(), b.hash(), "tick {t} after the resume: the rides part");
        }
        // another build's point keeps the ride and drops the state
        let (other, said) = load(&d, "b2").expect("a point");
        assert_eq!(other, Point::Ride(Ride { state: None, ..ride }));
        assert!(said.is_some_and(|s| s.contains("from its start")));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_main_menu_offers_the_point_until_it_is_answered() {
        use crate::menu::{Menu, Press, Screen};
        for (moves, want) in [(0, true), (1, false), (2, true)] {
            let mut menu = Menu::new(Default::default(), Default::default());
            menu.main();
            menu.offer.put("the ride on free06".into());
            for _ in 0..moves {
                menu.step(Press { down: true, ..Press::default() });
            }
            assert_eq!(menu.offer.asking(), Some(("the ride on free06", moves % 2)));
            assert_eq!(menu.screen(), Some(Screen::Main), "a move under the popup moved the menu");
            menu.step(Press { ok: true, ..Press::default() });
            assert_eq!(menu.screen(), Some(Screen::Main), "Cross on the popup opened a row");
            assert_eq!(menu.offer.take(), Some(want));
            assert_eq!(menu.offer.take(), None, "answered once");
            assert_eq!(menu.offer.asking(), None);
        }
        let mut menu = Menu::new(Default::default(), Default::default());
        menu.main();
        menu.offer.put("Garage".into());
        menu.step(Press { back: true, ..Press::default() });
        assert_eq!((menu.offer.take(), menu.screen()), (Some(false), Some(Screen::Main)), "Circle is Not Now");
    }

    #[test]
    fn a_file_with_no_place_is_no_point() {
        assert_eq!(Point::parse("# nothing\nbuild = b1\n", "b1"), None);
        assert_eq!(Point::parse("", "b1"), None);
        let (p, said) = Point::parse("track = free06\nstate = 0g\n", "").unwrap();
        assert!(matches!(p, Point::Ride(Ride { state: None, .. })));
        assert!(said.is_some());
    }
}
