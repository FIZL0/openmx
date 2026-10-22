//! **Session markers** (P88): a spot saved on the d-pad and ridden back to, to line a trick up and
//! keep trying it. The port's own: the original has nothing like it, so there is nothing here to be
//! 1:1 with — only the d-pad it shares with the ride, which keeps every meaning it has there.
//!
//! Each player has [`SLOTS`] markers for the session, and a selection that steps through them and
//! then through the other players in the ride — a seat nobody rides is skipped (P94). Left and
//! Right are a d-pad direction's **press** (P93): each fires on the tick the direction goes down
//! ([`crate::bindings::Table::pressed`]). Down and Up, which put a marker down and ride the bike away,
//! are a **hold of [`HOLD_MS`]** (P106, P88's hold at 200 ms rather than 250), read off the input
//! layer's own held timer ([`crate::input::ControlState::held_ms`]) and fired on the tick that timer
//! crosses it — a shorter tap does nothing. Each fires once, however long it is held after:
//!
//! - **Down** puts a marker in the selected slot: the whole bike as the tick left it ([`Ride`]) —
//!   where it is, which way it faces, its speed and its spin — so a run-up repeats;
//! - **Up** rides back to the selected slot's marker, or to the selected player where they stand;
//! - **Left** and **Right** step the selection, through the slots and then the other players, round.
//!
//! The d-pad is the original's in a ride (P73's table, commands 0–3): a digital stick (lean, steer,
//! the rider fore and aft) and the direction half of a trick's gesture. In the shell the markers have
//! it instead of the stick: the shell rides on [`Controls::stick_alone`], so the left stick alone
//! leans and steers and a d-pad press moves no bike. Its gesture meaning stays — a press with a trick
//! button down is a trick's gesture and not a marker's, so it fires nothing, and the bike reads it
//! as the original does. P93 had Up and Down fire on the press too; P106 put their hold back, against
//! a brushed d-pad placing over a marker or teleporting (see both journals).
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

/// How long Down or Up is held before it fires, in the input layer's milliseconds (P106).
pub const HOLD_MS: u32 = 200;

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

    /// How many places the selection has with `players` in the session, every one of them riding:
    /// the slots, then everybody else.
    pub fn places(players: usize) -> usize {
        SLOTS + players.saturating_sub(1)
    }

    /// What the selection is on, for the player in seat `me` of `players`, every one of them riding:
    /// [`Markers::pick_among`] with nobody left out.
    pub fn pick(&self, me: usize, players: usize) -> Pick {
        self.pick_among(me, &vec![true; players])
    }

    /// The seats the selection reaches past the slots, for the player in seat `me`: every other seat
    /// in the ride (P94), in seat order. `riding[s]` says whether seat `s` is in it — an online seat
    /// nobody sits in rides on with an idle pad and is not drawn, so it is no pick — and a seat past
    /// the end of `riding` is in.
    pub fn others(me: usize, seats: usize, riding: &[bool]) -> Vec<usize> {
        (0..seats).filter(|&s| s != me && riding.get(s).copied().unwrap_or(true)).collect()
    }

    /// What the selection is on, for the player in seat `me` with `riding` saying who is in the ride
    /// (its length the seat count): past the slots, the other riders in seat order
    /// ([`Markers::others`]). A selection past the end (somebody left) is the first slot.
    pub fn pick_among(&self, me: usize, riding: &[bool]) -> Pick {
        let at = self.at as usize;
        if at < SLOTS {
            return Pick::Slot(at);
        }
        match Markers::others(me, riding.len(), riding).get(at - SLOTS) {
            Some(&p) => Pick::Player(p),
            None => Pick::Slot(0),
        }
    }

    /// Whether a slot holds a marker.
    pub fn holds(&self, slot: usize) -> bool {
        self.slots.get(slot).is_some_and(Option::is_some)
    }

    /// Left or Right through `places` places, round; a selection already past them starts from the
    /// first slot.
    fn step(&mut self, by: isize, places: usize) {
        let n = places as isize;
        let at = if (self.at as isize) < n { self.at as isize } else { 0 };
        self.at = (at + by).rem_euclid(n) as u8;
    }
}

/// The four d-pad directions that fired this tick — left, right, up, down — off the pad the player's
/// controls were read from: Left and Right the tick they went down, Up and Down the tick their held
/// timer crosses [`HOLD_MS`]. None with a trick button down, which makes the d-pad a trick's gesture.
pub fn fired(pad: &crate::input::Pad, table: &crate::bindings::Table, hands: &Controls) -> [bool; 4] {
    if hands.trick.iter().any(|&t| t) {
        return [false; 4];
    }
    let crossed = |c| {
        table.control(pad.index as usize, c).is_some_and(|control| {
            let s = pad.control(control);
            s.is_held() && s.prev_held_ms < HOLD_MS && s.held_ms >= HOLD_MS
        })
    };
    [table.pressed(pad, Command::LEFT), table.pressed(pad, Command::RIGHT), crossed(Command::UP), crossed(Command::DOWN)]
}

/// Whether a d-pad direction is down for the markers, for a display that stays up while it is: none
/// with a trick button down.
pub fn held(pad: &crate::input::Pad, table: &crate::bindings::Table, hands: &Controls) -> bool {
    !hands.trick.iter().any(|&t| t) && DIRECTIONS.iter().any(|&c| table.held(pad, c))
}

/// How far Down or Up is through its hold, `0..=HOLD_MS`, for a bar that fills while the player
/// holds: the longer-held of the two, 0 with neither held or a trick button down. Left and Right fire
/// on the press and have no hold to show.
pub fn holding(pad: &crate::input::Pad, table: &crate::bindings::Table, hands: &Controls) -> u32 {
    if hands.trick.iter().any(|&t| t) {
        return 0;
    }
    [Command::UP, Command::DOWN]
        .iter()
        .filter_map(|&c| table.control(pad.index as usize, c))
        .map(|control| pad.control(control))
        .filter(|s| s.is_held())
        .map(|s| s.held_ms.min(HOLD_MS))
        .max()
        .unwrap_or(0)
}

const DIRECTIONS: [Command; 4] = [Command::LEFT, Command::RIGHT, Command::UP, Command::DOWN];

/// **One tick of a player's markers**, after the tick's step: the player in seat `me` of
/// `spots.len()`, `hands` the controls the tick was stepped with, `spots` where every player was
/// ([`Spot::of`], taken before anybody's tick), every one of them in the ride. On the bike only: a
/// plane ride has none.
pub fn frame(sim: &mut Sim, me: usize, spots: &[Spot], hands: &Controls) -> Option<Did> {
    frame_among(sim, me, spots, &[], hands)
}

/// [`frame`] with `riding` saying which seats are in the ride ([`Markers::others`]): a seat not in
/// it is never a pick, so the selection steps past it (P94). A seat past the end of `riding` is in.
pub fn frame_among(sim: &mut Sim, me: usize, spots: &[Spot], riding: &[bool], hands: &Controls) -> Option<Did> {
    if sim.plane.is_some() {
        return None;
    }
    let [left, right, up, down] = fired(sim.input.pad(0), sim.table(), hands);
    let seats = spots.len().max(1);
    let riding: Vec<bool> = (0..seats).map(|s| riding.get(s).copied().unwrap_or(true)).collect();
    let places = SLOTS + Markers::others(me, seats, &riding).len();
    let m = &mut sim.markers;
    let mut did = None;
    if left != right {
        m.step(if left { -1 } else { 1 }, places);
        did = Some(Did::Stepped(m.pick_among(me, &riding)));
    }
    let pick = m.pick_among(me, &riding);
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

// ---------------------------------------------------------------------------------------------------
// Saved between sessions (P99).
// ---------------------------------------------------------------------------------------------------

/// What a saved-markers file begins with.
const FILE_MAGIC: [u8; 8] = *b"OMXmarks";

/// The saved-markers file's format. Bump it when the file changes shape; a file of another version
/// is dropped ([`Markers::from_file`] says why), never read as this one. A change to the ride's own
/// codec is caught too: the slots are [`crate::snapshot::encode_slots`]'s, which carries the
/// snapshot's format and must read back to the last byte.
pub const FILE_VERSION: u32 = 1;

impl Markers {
    /// Every slot emptied (P99, the pause's Clear Markers), the selection left where it is.
    pub fn clear(&mut self) {
        self.slots = Default::default();
    }

    /// The slots as a file, for keeping between sessions: [`FILE_MAGIC`], [`FILE_VERSION`], the bike
    /// they were placed on (`bike`, the `.phy`'s name — a marker is the whole bike, and another bike's
    /// is no use to this one), then the slots. The selection is not kept: a ride starts on the first
    /// slot.
    pub fn to_file(&self, bike: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 * 1024);
        out.extend_from_slice(&FILE_MAGIC);
        out.extend_from_slice(&FILE_VERSION.to_le_bytes());
        out.extend_from_slice(&(bike.len() as u32).to_le_bytes());
        out.extend_from_slice(bike.as_bytes());
        out.extend_from_slice(&crate::snapshot::encode_slots(&self.slots));
        out
    }

    /// [`Markers::to_file`]'s bytes back, on the first slot, if they are this format's and `bike`'s;
    /// an error saying why not otherwise, for the caller to drop the file on.
    pub fn from_file(bytes: &[u8], bike: &str) -> Result<Markers, String> {
        let (named, markers) = Markers::read_file(bytes)?;
        if named != bike {
            return Err(format!("markers of the {named} bike, not the {bike}"));
        }
        Ok(markers)
    }

    /// [`Markers::to_file`]'s bytes back whatever bike they name, and the bike: a tape's
    /// ([`crate::replay::PadTape::markers`]), which rides the bike it was made on.
    pub fn read_file(bytes: &[u8]) -> Result<(String, Markers), String> {
        let head = FILE_MAGIC.len() + 8;
        if bytes.len() < head || bytes[..FILE_MAGIC.len()] != FILE_MAGIC {
            return Err("not a markers file".into());
        }
        let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().expect("four bytes"));
        let version = word(FILE_MAGIC.len());
        if version != FILE_VERSION {
            return Err(format!("markers format {version}, this build reads {FILE_VERSION}"));
        }
        let n = word(FILE_MAGIC.len() + 4) as usize;
        let end = head.checked_add(n).filter(|&e| e <= bytes.len()).ok_or("the markers file ends early")?;
        let named = String::from_utf8(bytes[head..end].to_vec()).map_err(|_| "the markers file's bike is not text")?;
        let slots = crate::snapshot::decode_slots(&bytes[end..])?;
        Ok((named, Markers { slots, at: 0 }))
    }
}
