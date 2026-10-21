//! **Session markers** (P88): a spot saved on the d-pad and ridden back to, to line a trick up and
//! keep trying it. The port's own: the original has nothing like it, so there is nothing here to be
//! 1:1 with — only the d-pad it shares with the ride, which keeps every meaning it has there.
//!
//! Each player has [`SLOTS`] markers for the session, and a selection that steps through them and
//! then through the other players. All four are a d-pad direction **held [`HOLD_MS`]**, read off the
//! input layer's own held timer ([`crate::input::ControlState::held_ms`]), and each fires once, on
//! the tick that timer crosses it — so a tap shorter than that does nothing and a hold fires once
//! until it is let go:
//!
//! - **Down** puts a marker in the selected slot: the whole bike as the tick left it ([`Ride`]) —
//!   where it is, which way it faces, its speed and its spin — so a run-up repeats;
//! - **Up** rides back to the selected slot's marker, or to the selected player where they stand;
//! - **Left** and **Right** step the selection, through the slots and then the other players, round.
//!
//! The d-pad is the original's in a ride (P73's table, commands 0–3): a digital stick (lean, steer,
//! the rider fore and aft) and the direction half of a trick's gesture. All of that still reads the
//! d-pad exactly as before — the markers take nothing from the bike — and a hold with a trick button
//! down is a trick's gesture and not a marker's, so it fires nothing.
//!
//! A teleport sets the bike down the way a put-back does ([`crate::rider::Rider::teleport`]): out of
//! any wreck, the trick machine cleared, and ghosted (P57's poof, S63's pass-through), so a rider
//! landed on another collides with nothing. It runs **after** the tick's step, so the next tick
//! starts from exactly the state the marker kept — the tick after a placement, ridden again.
//!
//! Markers are pad input on the ticks the session agrees, and their state is the simulation's
//! ([`crate::Sim::markers`]): saved with a rollback snapshot, sent with a resync, hashed. A session
//! that never touches them walks, hashes and encodes exactly as it did before there were any.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use ps2_float::Vf;

use crate::bindings::Command;
use crate::hash::{State, Walk};
use crate::ride::{Controls, Ride};
use crate::rider::Rider;
use crate::Sim;

/// How many markers each player has.
pub const SLOTS: usize = 5;

/// How long a d-pad direction is held before it fires, in the input layer's milliseconds.
pub const HOLD_MS: u32 = 250;

/// One player's markers: the slots and what is selected.
#[derive(Clone, Debug, Default)]
pub struct Markers {
    /// The five slots, each the bike as it was when the marker was put down, or empty.
    pub slots: [Option<Box<Ride>>; SLOTS],
    /// The selection: `0..SLOTS` a slot, past it the other players in seat order ([`Markers::pick`]).
    pub at: u8,
}

/// What the selection is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// One of the player's own slots.
    Slot(usize),
    /// Another player, by their seat (0 is the first).
    Player(usize),
}

/// What a tick's d-pad did, for whoever shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Did {
    /// Left or Right moved the selection.
    Stepped(Pick),
    /// Down put a marker in this slot.
    Placed(usize),
    /// Up rode back to this.
    Teleported(Pick),
    /// Down or Up fired and did nothing: Up on an empty slot, Down on a player, or Down with the rider
    /// off the bike.
    Refused(Pick),
}

/// Where a teleport onto a player puts the bike: their bike's origin, its forward and its up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub at: Vf,
    pub forward: Vf,
    pub up: Vf,
}

impl Spot {
    /// Where a player is: their bike while they ride it, and the put-back record — where they will be
    /// put back — while they are down.
    pub fn of(rider: &Rider) -> Spot {
        if rider.state.wreck.is_some() || rider.crash.is_some() {
            let b = rider.ride.back;
            return Spot { at: b.at, forward: b.facing, up: b.up };
        }
        let (forward, up) = rider.ride.axes();
        Spot { at: rider.ride.position(), forward, up }
    }
}

impl Markers {
    /// Nothing placed and the first slot selected: a session that has not touched the markers.
    pub fn fresh(&self) -> bool {
        self.at == 0 && self.slots.iter().all(Option::is_none)
    }

    /// How many places the selection has with `players` in the session: the slots, then everybody
    /// else.
    pub fn places(players: usize) -> usize {
        SLOTS + players.saturating_sub(1)
    }

    /// What the selection is on, for the player in seat `me` of `players`: past the slots, the other
    /// players in seat order with `me` left out. A selection past the end (somebody left) is the
    /// first slot.
    pub fn pick(&self, me: usize, players: usize) -> Pick {
        let at = self.at as usize;
        if at < SLOTS || at >= Markers::places(players) {
            return Pick::Slot(if at < SLOTS { at } else { 0 });
        }
        let k = at - SLOTS;
        Pick::Player(if k < me { k } else { k + 1 })
    }

    /// Whether a slot holds a marker.
    pub fn holds(&self, slot: usize) -> bool {
        self.slots.get(slot).is_some_and(Option::is_some)
    }

    fn step(&mut self, by: isize, players: usize) {
        let n = Markers::places(players) as isize;
        let at = if (self.at as isize) < n { self.at as isize } else { 0 };
        self.at = (at + by).rem_euclid(n) as u8;
    }
}

/// The four d-pad directions that fired this tick — left, right, up, down — off the pad the player's
/// controls were read from: each the tick its held timer crosses [`HOLD_MS`]. None with a trick button
/// down, which makes the d-pad a trick's gesture.
pub fn fired(pad: &crate::input::Pad, table: &crate::bindings::Table, hands: &Controls) -> [bool; 4] {
    if hands.trick.iter().any(|&t| t) {
        return [false; 4];
    }
    [Command::LEFT, Command::RIGHT, Command::UP, Command::DOWN].map(|c| {
        table.control(pad.index as usize, c).is_some_and(|control| {
            let s = pad.control(control);
            s.is_held() && s.prev_held_ms < HOLD_MS && s.held_ms >= HOLD_MS
        })
    })
}

/// How far a d-pad direction is through its hold, `0..=HOLD_MS`, for a display that fills while the
/// player holds: the longest-held of the four, 0 with none held or a trick button down.
pub fn holding(pad: &crate::input::Pad, table: &crate::bindings::Table, hands: &Controls) -> u32 {
    if hands.trick.iter().any(|&t| t) {
        return 0;
    }
    [Command::LEFT, Command::RIGHT, Command::UP, Command::DOWN]
        .iter()
        .filter_map(|&c| table.control(pad.index as usize, c))
        .map(|control| pad.control(control))
        .filter(|s| s.is_held())
        .map(|s| s.held_ms.min(HOLD_MS))
        .max()
        .unwrap_or(0)
}

/// **One tick of a player's markers**, after the tick's step: the player in seat `me` of
/// `spots.len()`, `hands` the controls the tick was stepped with, `spots` where every player was
/// ([`Spot::of`], taken before anybody's tick). On the bike only: a plane ride has none.
pub fn frame(sim: &mut Sim, me: usize, spots: &[Spot], hands: &Controls) -> Option<Did> {
    if sim.plane.is_some() {
        return None;
    }
    let [left, right, up, down] = fired(sim.input.pad(0), sim.table(), hands);
    let players = spots.len().max(1);
    let m = &mut sim.markers;
    let mut did = None;
    if left != right {
        m.step(if left { -1 } else { 1 }, players);
        did = Some(Did::Stepped(m.pick(me, players)));
    }
    let pick = m.pick(me, players);
    if down {
        did = Some(match pick {
            Pick::Slot(s) if sim.rider.state.wreck.is_none() && sim.rider.crash.is_none() => {
                m.slots[s] = Some(Box::new(sim.rider.ride.clone()));
                Did::Placed(s)
            }
            _ => Did::Refused(pick),
        });
    }
    if up {
        did = Some(match pick {
            Pick::Slot(s) => match &m.slots[s] {
                Some(ride) => {
                    sim.rider.teleport((**ride).clone());
                    Did::Teleported(pick)
                }
                None => Did::Refused(pick),
            },
            Pick::Player(p) => match spots.get(p) {
                Some(spot) => {
                    sim.rider.teleport_to(*spot);
                    Did::Teleported(pick)
                }
                None => Did::Refused(pick),
            },
        });
    }
    did
}

impl State for Markers {
    fn walk(&self, w: &mut Walk<'_>) {
        w.u8("at", self.at);
        for (i, slot) in self.slots.iter().enumerate() {
            w.group(SLOT_NAMES[i], |w| {
                w.bool("set", slot.is_some());
                if let Some(ride) = slot {
                    w.field("ride", &**ride);
                }
            });
        }
    }
}

const SLOT_NAMES: [&str; SLOTS] = ["slot0", "slot1", "slot2", "slot3", "slot4"];
