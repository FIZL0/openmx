//! The session markers' slot display (P88), as the shell keeps it a view at a time: what the bar
//! reads of a player's markers ([`MarkerBar`]), when it is up ([`MarkerBanner`]) and what its label
//! says. The ride HUD draws it (`ride_hud.rs`'s marker menu, P121).
//!
//! This file was the shell's debug HUD (P6) too, and the software slot bar it and the original's HUD
//! drew; both left with P170 (user, 2026-10-26: "we likely wont use them").

use game_sim::bindings::Table;
use game_sim::input::Pad;
use game_sim::markers::Pick;
use std::time::{Duration, Instant};

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
/// are no map); a lone track is one map covering everything, with no neighbours. The port's own:
/// the original rides one map and its radar names nothing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapNames {
    pub maps: Vec<([f32; 4], String)>,
}

impl MapNames {
    /// One map, the whole ride: a lone track's.
    pub fn alone(name: &str) -> MapNames {
        MapNames { maps: vec![([f32::NEG_INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::INFINITY], name.to_string())] }
    }

    /// The map a rider at `at` (`x`, `z`) is on: the one whose rectangle holds it, or — on a join,
    /// between maps — the nearest, the one just left or about to be met. `None` with no maps.
    pub fn on(&self, at: [f32; 2]) -> Option<usize> {
        let gap = |r: &[f32; 4]| {
            let dx = (r[0] - at[0]).max(at[0] - r[2]).max(0.0);
            let dz = (r[1] - at[1]).max(at[1] - r[3]).max(0.0);
            dx * dx + dz * dz
        };
        (0..self.maps.len()).min_by(|&a, &b| gap(&self.maps[a].0).total_cmp(&gap(&self.maps[b].0)))
    }

    /// The map that lies `toward` ([`COMPASS`]'s direction) of map `from` for a rider at `at`: of the
    /// maps whose middle is past `from`'s edge that way, the one across from the rider — its span
    /// across that way holds the rider's, or comes nearest it — and of those the nearest past the
    /// edge. `None` where no map lies that way.
    pub fn toward(&self, from: usize, at: [f32; 2], toward: usize) -> Option<usize> {
        let [dx, dz] = COMPASS[toward % 4];
        let (axis, sign) = if dx != 0.0 { (0, dx) } else { (1, dz) };
        let across = 1 - axis;
        let here = self.maps.get(from)?.0;
        let edge = if sign > 0.0 { here[axis + 2] } else { here[axis] };
        let mut best: Option<(f32, f32, usize)> = None;
        for (i, (r, _)) in self.maps.iter().enumerate() {
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
        best.map(|(_, _, i)| i)
    }

    /// What the radar says for a rider at `at`: the name of the map they are on, and of the map that
    /// lies each way, N E S W.
    pub fn around(&self, at: [f32; 2]) -> (Option<&str>, [Option<&str>; 4]) {
        let Some(on) = self.on(at) else { return (None, [None; 4]) };
        let name = |i: usize| self.maps[i].1.as_str();
        (Some(name(on)), std::array::from_fn(|k| self.toward(on, at, k).map(name)))
    }
}
