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
