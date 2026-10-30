//! The session markers' slot display (P88), as the shell keeps it a view at a time: what the bar
//! reads of a player's markers ([`MarkerBar`]), when it is up ([`MarkerBanner`]) and what its label
//! says. The ride HUD draws it (`ride_hud.rs`'s marker menu, P121).
//!
//! This file was the shell's debug HUD (P6) too, and the software slot bar it and the original's HUD
//! drew; both left with P170 (user, 2026-10-26: "we likely wont use them").

use game_sim::bindings::Table;
use game_sim::input::Pad;
use game_sim::markers::Pick;
use game_render::wrap::Wrap;
use std::time::Duration;

use crate::clock::Instant;

/// **The markers' slot display** (P88): the slots and the other players, shown for a while
/// whenever the selection or a slot changes, and for as long as a d-pad direction is held for the
/// markers. It is the markers' only feedback, drawn by the ride HUD; not under the `none` overlay,
/// which asked for a bare picture (P117).
/// The port's own, like the markers themselves.
#[derive(Clone, Debug, Default)]
pub struct MarkerBanner {
    /// What was shown last: the selection and which slots hold a marker.
    seen: Option<(Pick, Vec<bool>)>,
    /// Until when it stays up.
    until: Option<Instant>,
    /// The seat's clears seen so far ([`MarkerBanner::cleared`]), and whether the spawn's Down has
    /// cleared since the selection last moved onto it (P135): the bar says so.
    clears: u32,
    said_cleared: bool,
}

impl MarkerBanner {
    /// How long it stays up after a change or a hold.
    pub const SHOWN: Duration = Duration::from_millis(2000);

    /// This frame's markers, and whether to draw them: up for [`MarkerBanner::SHOWN`] after the
    /// selection or a slot changes (not on the first frame, which has nothing to change from) and
    /// while `held`, a d-pad direction down for the markers ([`game_sim::markers::held`]) — so a
    /// press that changes nothing (Up on an empty slot) still shows what it was on.
    pub fn update(&mut self, now: Instant, pick: Pick, filled: &[bool], held: bool) -> bool {
        let seen = Some((pick, filled.to_vec()));
        if (self.seen.is_some() && self.seen != seen) || held {
            self.until = Some(now + MarkerBanner::SHOWN);
        }
        if self.seen.as_ref().is_some_and(|(was, _)| *was != pick) {
            self.said_cleared = false;
        }
        self.seen = seen;
        self.until.is_some_and(|until| now < until)
    }

    /// `clears`, how many times the seat's spawn has cleared its markers (P135, `Did::Cleared`): a
    /// new one puts the bar up saying Cleared, until the selection moves off the spawn.
    pub fn cleared(&mut self, now: Instant, clears: u32) {
        if clears != self.clears {
            self.clears = clears;
            self.said_cleared = true;
            self.until = Some(now + MarkerBanner::SHOWN);
        }
    }

    /// Whether the bar says Cleared ([`MarkerBanner::cleared`]).
    pub fn says_cleared(&self) -> bool {
        self.said_cleared
    }
}

/// What the slot display says about the selection: the slot and whether it holds a marker, or the
/// player by their name — `P1`, `P2`… for a seat with none; on the + (P139) the slot it would add.
pub fn marker_label(pick: Pick, filled: &[bool], names: &[Option<String>]) -> String {
    match pick {
        Pick::Spawn => "spawn".into(),
        Pick::Slot(s) => format!("marker {} {}", s + 1, if filled.get(s).copied().unwrap_or(false) { "set" } else { "empty" }),
        Pick::More => format!("marker {} new", filled.len() + 1),
        Pick::Player(p) => match names.get(p).and_then(|n| n.as_deref()).filter(|n| !n.trim().is_empty()) {
            Some(name) => format!("to {name}"),
            None => format!("to P{}", p + 1),
        },
    }
}

/// What one view's slot display reads of its player's markers, on whichever machine is up (P124):
/// the selection and the filled slots of the set `markers` has up — the switch puts the machine's
/// own up ([`game_sim::markers::Markers::up`]) — and, while `active` (the player riding or flying),
/// whether a direction is held for the markers and how far a Down or Up hold is. With the plane's set
/// up the d-pad is read as the plane's markers read it (`markers::fly`): the shipped bindings. A trick
/// button held with it changes nothing (P129).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkerBar {
    pub pick: Pick,
    /// Each slot of the set up, whether it holds a marker: five, or more off the + (P139).
    pub filled: Vec<bool>,
    pub held: bool,
    /// How far a Down or Up hold is, `0..=HOLD_MS` ([`game_sim::markers::holding_on`]): on the
    /// spawn Down's counts towards the clear's longer hold (P135).
    pub holding: u32,
    /// The spawn has a start to ride to on this map for the machine up (P132, [`MarkerBar::homed`]).
    pub home: bool,
    /// The spawn's Down has just cleared the markers (P135): the label says Cleared.
    pub cleared: bool,
}

impl MarkerBar {
    pub fn read(
        markers: &game_sim::markers::Markers,
        me: usize,
        riding: &[bool],
        pad: &Pad,
        table: &Table,
        active: bool,
    ) -> MarkerBar {
        let flying = markers.on == game_sim::markers::Vehicle::Plane;
        let table = if flying { &Table::SHIPPED } else { table };
        MarkerBar {
            pick: markers.pick_among(me, riding),
            filled: (0..markers.slots.len()).map(|s| markers.holds(s)).collect(),
            held: active && game_sim::markers::held(pad, table),
            holding: if active { game_sim::markers::holding_on(pad, table, markers.pick_among(me, riding)) } else { 0 },
            home: false,
            cleared: false,
        }
    }

    /// The bar with the spawn shown as one to ride to where `home` says the seat has a start for the
    /// machine up ([`game_sim::markers::Home::has`], P132).
    pub fn homed(self, home: bool) -> MarkerBar {
        MarkerBar { home, ..self }
    }

    /// The bar saying Cleared on the spawn while `cleared` ([`MarkerBanner::says_cleared`], P135).
    pub fn cleared(self, cleared: bool) -> MarkerBar {
        MarkerBar { cleared: cleared && self.pick == Pick::Spawn, ..self }
    }

    /// What the slot display says about the selection ([`marker_label`]): on the spawn, Cleared once
    /// its Down has cleared.
    pub fn label(&self, names: &[Option<String>]) -> String {
        match self.pick {
            Pick::Spawn if self.cleared => "cleared".into(),
            pick => marker_label(pick, &self.filled, names),
        }
    }
}

/// Whether view `v` shows its player's markers — the slot display, the portals and the radar's icons
/// — with `online` the session's (P88): offline every view, its own player's; online only this
/// machine's own view, the first. On every machine alike (P124).
pub fn markers_shown_in(v: usize, online: bool) -> bool {
    !online || v == 0
}

// ---- the radar's map names (MD42) ---------------------------------------------------------------

/// The ground directions the radar's compass letters stand for, N E S W, as `x`, `z`: the rose's
/// north is `+z`, the way a bearing of 0 points ([`crate::overlay::bearing`]), and east `+x`.
pub const COMPASS: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 0.0], [0.0, -1.0], [-1.0, 0.0]];

/// **The maps the radar names** (MD42): each map of the ride, the world rectangle it owns
/// (`[x0, z0, x1, z1]` in feet) and the name the HUD gives it. A world's are its tiles (its joins
/// are no map); a lone track, a disc one or a mod, has none, so its radar names nothing (P234). The port's own:
/// the original rides one map and its radar names nothing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapNames {
    pub maps: Vec<([f32; 4], String)>,
}

impl MapNames {
    /// The map a rider at `at` (`x`, `z`) is on: the one whose rectangle holds it, or — on a join,
    /// between maps — the nearest, the one just left or about to be met. `None` with no maps.
    pub fn on(&self, at: [f32; 2]) -> Option<usize> {
        self.on_in(at, None).map(|(i, _)| i)
    }

    /// [`MapNames::on`] on a world that may wrap (MD43): a map across a seam is as near as its copy
    /// one width off, so on a seam's join the map just over the seam is met. With the offset, in
    /// feet `[x, z]`, of the copy it was met at.
    pub fn on_in(&self, at: [f32; 2], wrap: Option<Wrap>) -> Option<(usize, [f32; 2])> {
        let gap = |r: &[f32; 4]| {
            let dx = (r[0] - at[0]).max(at[0] - r[2]).max(0.0);
            let dz = (r[1] - at[1]).max(at[1] - r[3]).max(0.0);
            dx * dx + dz * dz
        };
        let mut best: Option<(f32, usize, [f32; 2])> = None;
        for by in copies(wrap) {
            for (i, (r, _)) in self.maps.iter().enumerate() {
                let g = gap(&moved(r, by));
                if best.is_none_or(|(b, _, _)| g < b) {
                    best = Some((g, i, by));
                }
            }
        }
        best.map(|(_, i, by)| (i, by))
    }

    /// The map that lies `toward` ([`COMPASS`]'s direction) of map `from` for a rider at `at`: of the
    /// maps whose middle is past `from`'s edge that way, the one across from the rider — its span
    /// across that way holds the rider's, or comes nearest it — and of those the nearest past the
    /// edge. `None` where no map lies that way.
    pub fn toward(&self, from: usize, at: [f32; 2], toward: usize) -> Option<usize> {
        self.toward_in(from, [0.0; 2], at, toward, None)
    }

    /// [`MapNames::toward`] on a world that may wrap (MD43), from map `from`'s copy `by` feet off: the
    /// maps' copies one width off on each wrapped axis are looked at too, so past the last map
    /// toward a seam lies the first one over it. A map is never its own neighbour in any copy
    /// (MD49): where the way over a seam leads only back to it (one map wide, or a row with a gap),
    /// the nearest other map that way is named, or none.
    pub fn toward_in(&self, from: usize, by: [f32; 2], at: [f32; 2], toward: usize, wrap: Option<Wrap>) -> Option<usize> {
        let [dx, dz] = COMPASS[toward % 4];
        let (axis, sign) = if dx != 0.0 { (0, dx) } else { (1, dz) };
        let across = 1 - axis;
        let here = moved(&self.maps.get(from)?.0, by);
        let edge = if sign > 0.0 { here[axis + 2] } else { here[axis] };
        let mut best: Option<(f32, f32, usize)> = None;
        for off in copies(wrap) {
            for (i, (r, _)) in self.maps.iter().enumerate() {
                let r = moved(r, off);
                let middle = (r[axis] + r[axis + 2]) / 2.0;
                if i == from || sign * (middle - edge) <= 0.0 {
                    continue;
                }
                let near = if sign > 0.0 { r[axis] } else { r[axis + 2] };
                let ahead = (sign * (near - edge)).max(0.0);
                let off = (r[across] - at[across]).max(at[across] - r[across + 2]).max(0.0);
                if best.is_none_or(|(o, a, _)| (off, ahead) < (o, a)) {
                    best = Some((off, ahead, i));
                }
            }
        }
        best.map(|(_, _, i)| i)
    }

    /// What the radar says for a rider at `at`: the name of the map they are on, and of the map that
    /// lies each way, N E S W.
    pub fn around(&self, at: [f32; 2]) -> (Option<&str>, [Option<&str>; 4]) {
        self.around_in(at, None)
    }

    /// [`MapNames::around`] on a world that may wrap (MD43): across a seam each way's label names the
    /// map over it. Farther than [`OFF_MAP`] from every map the rider is on no map (MD56), and only
    /// the ways are named, from the nearest.
    pub fn around_in(&self, at: [f32; 2], wrap: Option<Wrap>) -> (Option<&str>, [Option<&str>; 4]) {
        let Some((on, by)) = self.on_in(at, wrap) else { return (None, [None; 4]) };
        let name = |i: usize| self.maps[i].1.as_str();
        let r = moved(&self.maps[on].0, by);
        let off = (r[0] - at[0]).max(at[0] - r[2]).max(r[1] - at[1]).max(at[1] - r[3]);
        (Some(name(on)).filter(|_| off <= OFF_MAP), std::array::from_fn(|k| self.toward_in(on, by, at, k, wrap).map(name)))
    }
}

/// Where the radar takes something at `at` (`x`, `z`) for a rider at `own` on a world that may wrap
/// (MD43): at its copy nearest the rider, so another rider, a zone or a marker just over a seam is
/// marked just off the radar's middle and not held to its rim from across the world.
pub fn radar_near(wrap: Option<Wrap>, own: [f32; 2], at: [f32; 2]) -> [f32; 2] {
    let Some(w) = wrap else { return at };
    let near = w.nearest(game_render::mat::v3(own[0], 0.0, own[1]), game_render::mat::v3(at[0], 0.0, at[1]));
    [near.x, near.z]
}

/// The offsets, in feet `[x, z]`, of a world's copies one width off on each wrapped axis, the world
/// itself first: only `[0, 0]` for a world that does not wrap.
fn copies(wrap: Option<Wrap>) -> Vec<[f32; 2]> {
    let mut out = vec![[0.0; 2]];
    if let Some(w) = wrap {
        let laps = |a: usize| if w.on[a] { vec![-1.0, 0.0, 1.0] } else { vec![0.0] };
        for kz in laps(1) {
            for kx in laps(0) {
                if [kx, kz] != [0.0, 0.0] {
                    out.push([kx * w.side[0], kz * w.side[1]]);
                }
            }
        }
    }
    out
}

/// How far past a map's rectangle, in feet on either axis, the radar still says the rider is on it
/// (MD56): the widest band openmx-mods' stitch cuts off a map's edge (its 150-foot shootoff and a
/// berm of up to 400), so the join there stands where the map's own edge ground stood. Past it a
/// join is the stitch's blend between maps, smooth ground no map has; half of world_mxu and
/// world_ultra is that, and named for the nearest map it read as that map ridden flat.
pub const OFF_MAP: f32 = 550.0;

/// A rectangle `[x0, z0, x1, z1]` moved `by` feet.
fn moved(r: &[f32; 4], by: [f32; 2]) -> [f32; 4] {
    [r[0] + by[0], r[1] + by[1], r[2] + by[0], r[3] + by[1]]
}

/// What the iPhone's overlay says while [`NeedsTap`] is up (P233).
pub const NEEDS_TAP_TEXT: &str = "Tap screen to resume controller";

/// **"Tap screen to resume controller"** (P233), opentennis's `needs_tap.rs`: after a screenshot iOS
/// keeps event focus, and GameController reports every button up and the sticks centred until a
/// touch gives it back; the app can't take it back itself. With a controller in, the overlay
/// (`ios_gpu.rs`) shows over everything until the first touch, or until the pad is live again.
/// Not while the pad is still live (P235, user 2026-10-30: 'we dont have to show the overlay for tap
/// if the controller still doing inputs'): a notice only arms it, and it goes up after
/// [`NeedsTap::QUIET`] frames with no live pad input; live input cancels it. Draw only: no input or
/// game state changes. Armed or not, never a count: a screenshot sends its notice twice.
#[derive(Default, Debug, PartialEq)]
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
pub struct NeedsTap {
    /// The overlay is up.
    pub up: bool,
    /// Frames since a notice with no live pad input, while it waits to go up.
    armed: Option<u32>,
}

#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
impl NeedsTap {
    /// Frames of a silent pad after a notice before the overlay shows: a third of a second at 60.
    pub const QUIET: u32 = 20;

    /// A screenshot notice with `controllers` connected: armed only with a controller to resume.
    pub fn notice(&mut self, controllers: usize) {
        if controllers > 0 && !self.up {
            self.armed.get_or_insert(0);
        }
    }

    /// A touch, or pad input that came through (a button held or a stick off centre): focus is
    /// back, or was never lost.
    pub fn input(&mut self, touched: bool, live: bool) {
        if touched || live {
            self.up = false;
            self.armed = None;
        }
    }

    /// A frame's worth: input first, then the wait, then this frame's notice, so a notice outlasts
    /// input from before it.
    pub fn frame(&mut self, touched: bool, live: bool, notice: Option<usize>) {
        self.input(touched, live);
        if let Some(n) = self.armed.as_mut() {
            *n += 1;
            if *n >= Self::QUIET {
                self.up = true;
                self.armed = None;
            }
        }
        if let Some(n) = notice {
            self.notice(n);
        }
    }
}

#[cfg(test)]
mod needs_tap_tests {
    use super::NeedsTap;

    /// `frames` frames with nothing happening.
    fn quiet(t: &mut NeedsTap, frames: u32) {
        for _ in 0..frames {
            t.frame(false, false, None);
        }
    }

    #[test]
    fn up_after_a_screenshot_with_a_silent_controller_until_input() {
        let mut t = NeedsTap::default();
        t.frame(false, false, Some(0));
        quiet(&mut t, NeedsTap::QUIET);
        assert!(!t.up, "no controller: nothing to resume");
        t.frame(false, false, Some(1));
        quiet(&mut t, NeedsTap::QUIET - 1);
        assert!(!t.up, "not before the pad has been quiet a while");
        quiet(&mut t, 1);
        assert!(t.up, "a screenshot with a controller in, and the pad gone silent");
        quiet(&mut t, 100);
        assert!(t.up, "released buttons and centred sticks aren't input");
        t.frame(true, false, None);
        assert!(!t.up, "a touch");
        t.frame(false, false, Some(2));
        quiet(&mut t, NeedsTap::QUIET);
        t.frame(false, true, None);
        assert!(!t.up, "pad input that came through");
    }

    #[test]
    fn a_live_controller_never_shows_it() {
        let mut t = NeedsTap::default();
        t.frame(false, true, Some(1));
        for _ in 0..3 * NeedsTap::QUIET {
            t.frame(false, true, None);
            assert!(!t.up, "the pad still riding: no overlay");
        }
        quiet(&mut t, 3 * NeedsTap::QUIET);
        assert!(!t.up, "live input cancelled the notice; silence later doesn't bring it back");
    }

    #[test]
    fn a_screenshots_double_notice_is_one_tap() {
        let mut t = NeedsTap::default();
        t.frame(false, false, Some(1));
        t.frame(false, false, Some(1));
        quiet(&mut t, NeedsTap::QUIET);
        assert!(t.up, "set twice is still up, never toggled off");
        t.frame(false, false, Some(1));
        assert!(t.up);
        t.frame(true, false, None);
        assert!(!t.up, "one tap clears it");
    }

    #[test]
    fn a_notice_outlasts_input_in_the_same_frame() {
        let mut t = NeedsTap::default();
        t.frame(false, true, Some(1));
        quiet(&mut t, NeedsTap::QUIET);
        assert!(t.up, "the input came before the notice");
        t.frame(false, true, None);
        assert!(!t.up, "the next input clears it");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overlay::radar_mark;

    /// MD43's world: two 567-foot maps in a row along `x`, a 135-foot seam join past them, wrapping on `x`.
    const WIDTH: f32 = 1269.0;

    fn wrap() -> Option<Wrap> {
        Wrap::of([true, false], [WIDTH, 567.0])
    }

    #[test]
    fn the_radars_dot_stays_on_the_radar_over_a_seam() {
        let own = [WIDTH - 20.0, 283.0];
        let other = [20.0, 283.0];
        let span = 400.0;
        let mark = |at: [f32; 2]| radar_mark(own, [1.0, 0.0], at, span);
        let middle = mark(own);
        let dist = |a: (f32, f32)| ((a.0 - middle.0).powi(2) + (a.1 - middle.1).powi(2)).sqrt();
        let rim = dist(mark([own[0] + span, own[1]]));
        let near = radar_near(wrap(), own, other);
        assert_eq!(near, [WIDTH + 20.0, 283.0]);
        assert!(dist(mark(near)) < rim * 0.25, "40 feet ahead is by the middle: {} of a rim at {rim}", dist(mark(near)));
        assert!(dist(mark(other)) >= rim - 0.01, "without the wrap it is held to the rim");
        assert_eq!(radar_near(None, own, other), other);
        // Not across a seam, nothing moves.
        assert_eq!(radar_near(wrap(), [600.0, 283.0], [700.0, 100.0]), [700.0, 100.0]);
    }

    #[test]
    fn the_labels_name_the_map_across_the_seam() {
        let maps = MapNames { maps: vec![([0.0, 0.0, 567.0, 567.0], "A".into()), ([567.0, 0.0, 1134.0, 567.0], "B".into())] };
        let own = [WIDTH - 20.0, 283.0];
        // Without the wrap, A lies one way only and nothing past the world's end.
        let (on, ways) = maps.around([1000.0, 283.0]);
        assert_eq!(on, Some("B"));
        assert_eq!(ways.iter().filter(|w| **w == Some("A")).count(), 1);
        // Wrapped, A is both ways along x: past B and over the seam.
        let (on, ways) = maps.around_in([1000.0, 283.0], wrap());
        assert_eq!(on, Some("B"));
        assert_eq!(ways.iter().filter(|w| **w == Some("A")).count(), 2);
        assert_eq!(ways.iter().filter(|w| w.is_some()).count(), 2, "nothing across z, which does not wrap: {ways:?}");
        let (on, ways) = maps.around_in([10.0, 283.0], wrap());
        assert_eq!(on, Some("A"));
        assert_eq!(ways.iter().filter(|w| **w == Some("B")).count(), 2);
        // On the seam's join, past B and nearer A over the seam, the rider is on A.
        assert_eq!(maps.around_in(own, wrap()).0, Some("A"));
        assert_eq!(maps.around(own).0, Some("B"));
    }

    /// MD56: out on a join's open ground, past [`OFF_MAP`] from every map, the radar names no map
    /// underfoot — the ground there is the stitch's blend, not the nearest map — but still the maps
    /// each way; within it, and over a seam's copy, the nearest map as before.
    #[test]
    fn a_rider_far_out_on_a_join_is_on_no_map() {
        // Two 5000-foot maps 4000 feet apart along x, wrapping with another 4000 feet past the second.
        let maps = MapNames { maps: vec![([0.0, 0.0, 5000.0, 5000.0], "A".into()), ([9000.0, 0.0, 14000.0, 5000.0], "B".into())] };
        let wrap = Wrap::of([true, false], [18000.0, 5000.0]);
        assert_eq!(maps.around([5000.0 + OFF_MAP, 2500.0]).0, Some("A"));
        assert_eq!(maps.around([9000.0 - OFF_MAP, 2500.0]).0, Some("B"));
        let (on, ways) = maps.around([7000.0, 2500.0]);
        assert_eq!(on, None, "2000 feet from either map");
        assert_eq!(ways.iter().flatten().count(), 1, "the nearest's way along x is still named: {ways:?}");
        assert_eq!(maps.around([5000.0 + OFF_MAP + 1.0, 2500.0]).0, None);
        // Over the wrap's seam: 300 feet past B, then 300 feet short of A's copy, then between.
        assert_eq!(maps.around_in([14300.0, 2500.0], wrap).0, Some("B"));
        assert_eq!(maps.around_in([17700.0, 2500.0], wrap).0, Some("A"));
        assert_eq!(maps.around_in([16000.0, 2500.0], wrap).0, None);
        // Across z too: a rider 2000 feet north of A is off it.
        assert_eq!(maps.around([2500.0, 7000.0]).0, None);
    }

    #[test]
    fn no_map_is_its_own_neighbour_on_a_wrapped_world() {
        // world_mxu_free's layout (MD49): 5472-foot tiles kept from 704 to 5448 feet, three along x
        // by two along z, wrapping both ways, with no tile at the second row's first place — north
        // and south of the first tile is only its own copy one world-height over.
        let t = 5472.0;
        let tile = |c: f32, r: f32, n: &str| ([c * t + 704.0, r * t + 696.0, c * t + 5448.0, r * t + 5440.0], n.to_string());
        let maps = MapNames {
            maps: vec![tile(0.0, 0.0, "Dallas"), tile(2.0, 0.0, "C"), tile(2.0, 1.0, "D"), tile(1.0, 0.0, "E"), tile(1.0, 1.0, "F")],
        };
        let wrap = Wrap::of([true, true], [3.0 * t, 2.0 * t]);
        for (i, (r, own)) in maps.maps.iter().enumerate() {
            for fx in [0.02, 0.5, 0.98] {
                for fz in [0.02, 0.5, 0.98] {
                    let at = [r[0] + fx * (r[2] - r[0]), r[1] + fz * (r[3] - r[1])];
                    let (on, ways) = maps.around_in(at, wrap);
                    assert_eq!(on, Some(own.as_str()), "at {at:?}");
                    assert!(!ways.contains(&Some(own.as_str())), "{own} at {at:?} names itself: {ways:?}");
                    for k in 0..4 {
                        assert_ne!(maps.toward_in(i, [0.0; 2], at, k, wrap), Some(i));
                    }
                }
            }
        }
        // Dallas's north and south are other maps (over the seams), not Dallas again.
        let (_, ways) = maps.around_in([2700.0, 2700.0], wrap);
        assert!(ways[0].is_some() && ways[2].is_some(), "{ways:?}");
        // Without the wrap, nothing lies north of Dallas but the row above's maps, off to the east.
        assert_eq!(maps.around([2700.0, 2700.0]).1[2], None);
    }
}
