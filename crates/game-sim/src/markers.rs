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
//! - **Up** rides back to the selected slot's marker, or to the selected player where they are —
//!   at their speed and spin while they ride (P111, [`Spot::motion`]), stopped where they will be
//!   put back while they are down; slot 1 with nothing put there rides to where the machine started
//!   this map (P132, [`Home`]), at rest;
//! - **Left** and **Right** step the selection, through the slots and then the other players, round.
//!
//! The d-pad is the original's in a ride (P73's table, commands 0–3): a digital stick (lean, steer,
//! the rider fore and aft) and the direction half of a trick's gesture. In the shell the markers have
//! it instead of the stick: the shell rides on [`Controls::stick_alone`], so the left stick alone
//! leans and steers and a d-pad press moves no bike. Since P129 that includes the gesture: a press
//! with a trick button down is the markers' too and does no trick (the stick names the gesture's
//! direction). P93 had Up and Down fire on the press too; P106 put their hold back, against a brushed
//! d-pad placing over a marker or teleporting (see both journals). A seat with its Markers switch off
//! (P129, the shell's) runs none of this, and its d-pad rides as the original's.
//!
//! A teleport sets the bike down the way a put-back does ([`crate::rider::Rider::teleport`]): out of
//! any wreck, the trick machine cleared, and ghosted (P57's poof, S63's pass-through), so a rider
//! landed on another collides with nothing. It runs **after** the tick's step, so the next tick
//! starts from exactly the state the marker kept — the tick after a placement, ridden again.
//!
//! Markers are pad input on the ticks the session agrees, and their state is the simulation's
//! ([`crate::Sim::markers`]): saved with a rollback snapshot, sent with a resync, hashed. A session
//! that never touches them walks, hashes and encodes exactly as it did before there were any.
//!
//! **Every machine has a set of its own** (P107): the bike's five slots and selection, and the
//! plane's five and its own selection, never shared. [`Markers::slots`] and [`Markers::at`] are
//! always the set of the machine that is up ([`Markers::on`]); the others wait in [`Markers::away`]
//! until their machine is taken again ([`Markers::up`]). So everything that shows the markers — the
//! slot display, the radar, the world's portals — shows the machine that is up's and only those. A
//! plane's marker is its whole flight ([`Mark::Plane`]: where it is, its attitude, its speed and
//! spin, its throttle and surfaces), and Up puts the plane back into it in the air ([`fly`]).
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use ps2_float::Vf;

use crate::bindings::Command;
use crate::flight::Flight;
use crate::hash::{State, Walk};
use crate::ride::{Controls, Ride};
use crate::rider::Rider;
use crate::Sim;

/// How many markers each player has.
pub const SLOTS: usize = 5;

/// How long Down or Up is held before it fires, in the input layer's milliseconds (P106).
pub const HOLD_MS: u32 = 200;

/// One player's markers: the slots and what is selected, of the machine that is up, and every other
/// machine's set beside them (P107).
#[derive(Clone, Debug, Default)]
pub struct Markers {
    /// The five slots of the machine that is up ([`Markers::on`]), each the machine as it was when
    /// the marker was put down, or empty.
    pub slots: [Option<Box<Mark>>; SLOTS],
    /// The selection: `0..SLOTS` a slot, past it the other players in seat order ([`Markers::pick`]).
    pub at: u8,
    /// The machine [`Markers::slots`] and [`Markers::at`] are the set of.
    pub on: Vehicle,
    /// The other machines' sets, in [`Vehicle`] order: only ones that hold a marker or a selection
    /// off the first slot, so a session that never touched another machine's markers has none here.
    pub away: Vec<Set>,
}

/// The machines a player keeps markers on (P107), each its own set. The bike is the first: a
/// session that never left it has only the bike's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Vehicle {
    #[default]
    Bike,
    Plane,
}

impl Vehicle {
    /// Every machine with markers, in order.
    pub const ALL: [Vehicle; 2] = [Vehicle::Bike, Vehicle::Plane];

    /// The byte a saved-markers file names it by.
    pub fn byte(self) -> u8 {
        self as u8
    }

    /// [`Vehicle::byte`] back.
    pub fn of_byte(b: u8) -> Option<Vehicle> {
        Vehicle::ALL.get(b as usize).copied()
    }
}

/// What a plane's markers are kept under (P107): its `.phy`'s name, as a bike's are under theirs
/// (`Biplane\biplane.phy`, [`crate::phy::BIPLANE_PHY`], is `biplane`).
pub const PLANE: &str = "biplane";

/// One marker: the whole machine as the tick left it.
#[derive(Clone, Debug)]
pub enum Mark {
    /// The bike ([`Ride`]): where it is, which way it faces, its speed and its spin.
    Bike(Ride),
    /// The plane's whole flight ([`Flight`]): its body (where it is, its attitude, its speed and
    /// spin), the throttle and the surfaces, the engine, the wheels and the collision object's search.
    Plane(Flight),
}

impl Mark {
    /// The machine it is a marker of.
    pub fn vehicle(&self) -> Vehicle {
        match self {
            Mark::Bike(_) => Vehicle::Bike,
            Mark::Plane(_) => Vehicle::Plane,
        }
    }

    /// Where the machine was put down: the bike's origin, the plane model's.
    pub fn position(&self) -> Vf {
        match self {
            Mark::Bike(ride) => ride.position(),
            Mark::Plane(f) => {
                let at = f.pose().1;
                Vf::new(at.x(), at.y(), at.z(), 1.0)
            }
        }
    }

    /// Its forward and its up, as [`Ride::axes`] gives a bike's: a plane's nose and up as the measure
    /// left them.
    pub fn axes(&self) -> (Vf, Vf) {
        match self {
            Mark::Bike(ride) => ride.axes(),
            Mark::Plane(f) => {
                let (n, u) = (f.measure.forward, f.measure.up);
                (Vf::new(n.x(), n.y(), n.z(), 0.0), Vf::new(u.x(), u.y(), u.z(), 0.0))
            }
        }
    }

    /// The bike, for a bike's marker.
    pub fn ride(&self) -> Option<&Ride> {
        match self {
            Mark::Bike(ride) => Some(ride),
            Mark::Plane(_) => None,
        }
    }

    /// The flight, for a plane's marker.
    pub fn flight(&self) -> Option<&Flight> {
        match self {
            Mark::Plane(f) => Some(f),
            Mark::Bike(_) => None,
        }
    }
}

/// One machine's markers while another is up ([`Markers::away`]).
#[derive(Clone, Debug, Default)]
pub struct Set {
    pub on: Vehicle,
    pub slots: [Option<Box<Mark>>; SLOTS],
    pub at: u8,
}

impl Set {
    /// Nothing placed and the first slot selected.
    pub fn fresh(&self) -> bool {
        self.at == 0 && self.slots.iter().all(Option::is_none)
    }
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

/// Where a teleport onto a player puts the bike: their bike's origin, its forward and its up, and
/// while they ride it the bike's motion (P111), which the teleport rides on with.
#[derive(Clone, Debug, PartialEq)]
pub struct Spot {
    pub at: Vf,
    pub forward: Vf,
    pub up: Vf,
    /// The bike the player rides, as the tick before the teleport left it, for the teleporter to
    /// ride on as ([`crate::ride::Ride::ride_as`]): its speed, its spin, its wheels'. `None` for a
    /// player who is down or flies the plane, who has no bike motion to take, so the teleport sets
    /// the bike down there stopped.
    pub motion: Option<Motion>,
}

/// **Where each seat's machines start on this map** (P132): marker slot 1's spot while the player
/// has put nothing there, so an Up on it rides back to the start at rest. The bike's is the map's
/// start line (a world's spawn, a mod map's: whatever the place's reader gave) or the seat's place
/// beside it; the plane's the seat's own bay off `machine.txt` ([`Flight::reset`]'s origin and
/// facing). A constant of the ride like the bike's `.phy` ([`Sim::home`]): every machine derives
/// the same from the same map, so it is never hashed, encoded or sent, and a resync keeps this
/// machine's. Empty — a fixture's, a tape's harness — slot 1 is empty as it always was.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Home {
    /// Each seat's bike start, in seat order, still. A seat past the end starts where the first
    /// does, as every online seat does (N5).
    pub bikes: Vec<Spot>,
    /// Each seat's plane bay, its origin and its facing, in seat order; a seat past the end has none.
    pub planes: Vec<(Vf, Vf)>,
}

impl Home {
    /// Seat `seat`'s bike start, the first seat's where the list has none of its own.
    pub fn bike(&self, seat: usize) -> Option<&Spot> {
        self.bikes.get(seat).or(self.bikes.first())
    }

    /// Seat `seat`'s plane bay.
    pub fn plane(&self, seat: usize) -> Option<(Vf, Vf)> {
        self.planes.get(seat).copied()
    }

    /// Whether seat `seat` has a start for machine `on`: slot 1 is its spawn while nothing is there.
    pub fn has(&self, on: Vehicle, seat: usize) -> bool {
        match on {
            Vehicle::Bike => self.bike(seat).is_some(),
            Vehicle::Plane => self.plane(seat).is_some(),
        }
    }

    /// The first seat's bike start only, and `planes`: an online ride's, where every seat's bike is
    /// a copy of the first's and every seat has its bay (N18).
    pub fn online(&self, planes: Vec<(Vf, Vf)>) -> Home {
        Home { bikes: self.bikes.first().cloned().into_iter().collect(), planes }
    }
}

/// The slot an empty selection rides to the start from (P132): the first.
pub const HOME_SLOT: usize = 0;

/// A riding player's bike, for [`Spot::motion`]: shared, as every peer's spot of them is the same.
#[derive(Clone, Debug)]
pub struct Motion(pub std::sync::Arc<Ride>);

impl PartialEq for Motion {
    /// The same bike, field for field, as the state hash walks it.
    fn eq(&self, other: &Motion) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0) || crate::hash::Dump::of(&*self.0) == crate::hash::Dump::of(&*other.0)
    }
}

impl Spot {
    /// A spot with no motion: the bike is set down there stopped.
    pub fn still(at: Vf, forward: Vf, up: Vf) -> Spot {
        Spot { at, forward, up, motion: None }
    }

    /// Where a player is: their bike while they ride it, with its motion, and the put-back record —
    /// where they will be put back — while they are down, with none.
    pub fn of(rider: &Rider) -> Spot {
        if rider.state.wreck.is_some() || rider.crash.is_some() {
            let b = rider.ride.back;
            return Spot::still(b.at, b.facing, b.up);
        }
        let (forward, up) = rider.ride.axes();
        Spot { at: rider.ride.position(), forward, up, motion: Some(Motion(std::sync::Arc::new(rider.ride.clone()))) }
    }
}

impl Markers {
    /// Nothing placed and the first slot selected, on every machine: a session that has not touched
    /// the markers.
    pub fn fresh(&self) -> bool {
        self.at == 0 && self.slots.iter().all(Option::is_none) && self.away.is_empty()
    }

    /// **The machine up changes** (P107): its own set out of [`Markers::away`] into
    /// [`Markers::slots`] and [`Markers::at`] — empty, on the first slot, for a machine that has
    /// none — and the set it replaces put away. The switch calls it, and so does each machine's
    /// tick ([`frame`], [`fly`]), so the set shown is always the machine up's.
    pub fn up(&mut self, on: Vehicle) {
        if on == self.on {
            return;
        }
        let gone = Set { on: self.on, slots: std::mem::take(&mut self.slots), at: self.at };
        let back = match self.away.iter().position(|s| s.on == on) {
            Some(i) => self.away.remove(i),
            None => Set { on, ..Set::default() },
        };
        if !gone.fresh() {
            self.away.push(gone);
            self.away.sort_by_key(|s| s.on);
        }
        (self.slots, self.at, self.on) = (back.slots, back.at, on);
    }

    /// `on`'s slots, whether it is up or away; `None` for a machine with no set.
    pub fn slots_of(&self, on: Vehicle) -> Option<&[Option<Box<Mark>>; SLOTS]> {
        if on == self.on {
            return Some(&self.slots);
        }
        self.away.iter().find(|s| s.on == on).map(|s| &s.slots)
    }

    /// Whether `on`'s set holds any marker.
    pub fn any_on(&self, on: Vehicle) -> bool {
        self.slots_of(on).is_some_and(|s| s.iter().any(Option::is_some))
    }

    /// Whether any machine's set holds a marker.
    pub fn any(&self) -> bool {
        Vehicle::ALL.iter().any(|&v| self.any_on(v))
    }

    /// How many markers are placed, every machine's.
    pub fn placed(&self) -> usize {
        Vehicle::ALL.iter().filter_map(|&v| self.slots_of(v)).flatten().flatten().count()
    }

    /// `set` as its machine's, in place of whatever that machine had: a saved file's slots, loaded
    /// for a machine that may or may not be up.
    pub fn put(&mut self, set: Set) {
        if set.on == self.on {
            (self.slots, self.at) = (set.slots, set.at);
            return;
        }
        self.away.retain(|s| s.on != set.on);
        if !set.fresh() {
            self.away.push(set);
            self.away.sort_by_key(|s| s.on);
        }
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
/// timer crosses [`HOLD_MS`]. A trick button held with it changes nothing (P129): the d-pad is no
/// part of a trick's gesture in the shell ([`Controls::stick_alone`]).
pub fn fired(pad: &crate::input::Pad, table: &crate::bindings::Table) -> [bool; 4] {
    let crossed = |c| {
        table.control(pad.index as usize, c).is_some_and(|control| {
            let s = pad.control(control);
            s.is_held() && s.prev_held_ms < HOLD_MS && s.held_ms >= HOLD_MS
        })
    };
    [table.pressed(pad, Command::LEFT), table.pressed(pad, Command::RIGHT), crossed(Command::UP), crossed(Command::DOWN)]
}

/// Whether a d-pad direction is down for the markers, for a display that stays up while it is.
pub fn held(pad: &crate::input::Pad, table: &crate::bindings::Table) -> bool {
    DIRECTIONS.iter().any(|&c| table.held(pad, c))
}

/// How far Down or Up is through its hold, `0..=HOLD_MS`, for a bar that fills while the player
/// holds: the longer-held of the two, 0 with neither held. Left and Right fire on the press and
/// have no hold to show.
pub fn holding(pad: &crate::input::Pad, table: &crate::bindings::Table) -> u32 {
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
/// `spots.len()`, `hands` the controls the tick was stepped with (which since P129 change nothing
/// here: a trick button no longer holds the markers' d-pad), `spots` where every player was
/// ([`Spot::of`], taken before anybody's tick), every one of them in the ride. On the bike only: a
/// plane ride has none.
pub fn frame(sim: &mut Sim, me: usize, spots: &[Spot], hands: &Controls) -> Option<Did> {
    frame_among(sim, me, spots, &[], hands)
}

/// [`frame`] with `riding` saying which seats are in the ride ([`Markers::others`]): a seat not in
/// it is never a pick, so the selection steps past it (P94). A seat past the end of `riding` is in.
pub fn frame_among(sim: &mut Sim, me: usize, spots: &[Spot], riding: &[bool], _hands: &Controls) -> Option<Did> {
    if sim.plane.is_some() {
        return None;
    }
    sim.markers.up(Vehicle::Bike);
    let fired = fired(sim.input.pad(0), sim.table());
    let m = &mut sim.markers;
    let (mut did, pick) = select(m, fired, me, spots.len(), riding);
    let [_, _, up, down] = fired;
    if down {
        did = Some(match pick {
            Pick::Slot(s) if sim.rider.state.wreck.is_none() && sim.rider.crash.is_none() => {
                m.slots[s] = Some(Box::new(Mark::Bike(sim.rider.ride.clone())));
                Did::Placed(s)
            }
            _ => Did::Refused(pick),
        });
    }
    if up {
        did = Some(match pick {
            Pick::Slot(s) => match m.slots[s].as_deref() {
                Some(Mark::Bike(ride)) => {
                    sim.rider.teleport(ride.clone());
                    Did::Teleported(pick)
                }
                // Slot 1 with nothing put there is the start (P132): there at rest, as the ride began.
                None if s == HOME_SLOT => match sim.home.bike(me) {
                    Some(start) => {
                        sim.rider.teleport_to(Spot::still(start.at, start.forward, start.up));
                        Did::Teleported(pick)
                    }
                    None => Did::Refused(pick),
                },
                _ => Did::Refused(pick),
            },
            Pick::Player(p) => match spots.get(p) {
                Some(spot) => {
                    sim.rider.teleport_to(spot.clone());
                    Did::Teleported(pick)
                }
                None => Did::Refused(pick),
            },
        });
    }
    did
}

/// Left and Right's step, for the player in seat `me` of `seats` with `riding` saying who is in the
/// ride (a seat past its end is in): what it did, if it fired, and what the selection is on after it.
fn select(m: &mut Markers, [left, right, ..]: [bool; 4], me: usize, seats: usize, riding: &[bool]) -> (Option<Did>, Pick) {
    let seats = seats.max(1);
    let riding: Vec<bool> = (0..seats).map(|s| riding.get(s).copied().unwrap_or(true)).collect();
    let places = SLOTS + Markers::others(me, seats, &riding).len();
    let mut did = None;
    if left != right {
        m.step(if left { -1 } else { 1 }, places);
        did = Some(Did::Stepped(m.pick_among(me, &riding)));
    }
    (did, m.pick_among(me, &riding))
}

/// **One tick of a player's markers in the plane** (P107), after the plane's step: [`frame_among`]'s
/// d-pad on the plane's own set, `flight` the plane the player flies and `pad` the pad it was flown
/// on. The plane has no tricks, so every press is the markers' (and the shell's plane does not read
/// the d-pad: `vehicle::Plane::step`); its pad is read through the shipped table, as the plane's is
/// on any profile.
///
/// - **Down** keeps the whole flight in the selected slot ([`Mark::Plane`]), unless the plane is
///   wrecked — as a rider off the bike places nothing;
/// - **Up** on a slot puts the plane back into the flight it kept, in the air where it was, at the
///   speed and attitude and throttle it had ([`Flight::put_back`]): out of whatever wreck it is in,
///   and the tick after it the tick after the placement flown again;
/// - **Up** on a player answers [`Did::Teleported`] and leaves the plane where it is: landing it on
///   the ground where the player is reads the ground and the plane's wheels, which is the garage's
///   (`vehicle::land_on`), as the switch onto the plane is.
///
/// Slot 1 with nothing put there is the plane's bay (P132, [`Home::plane`] of `home`): Up stands the
/// plane there at rest, unwrecked and its throttle shut ([`Flight::reset`], on `acos`), as an
/// online seat starts in it.
#[allow(clippy::too_many_arguments)]
pub fn fly(
    markers: &mut Markers,
    flight: &mut Flight,
    pad: &crate::input::Pad,
    me: usize,
    spots: &[Spot],
    riding: &[bool],
    home: &Home,
    acos: &crate::air::AcosTable,
) -> Option<Did> {
    markers.up(Vehicle::Plane);
    let fired = fired(pad, &crate::bindings::Table::SHIPPED);
    let (mut did, pick) = select(markers, fired, me, spots.len(), riding);
    let [_, _, up, down] = fired;
    if down {
        did = Some(match pick {
            Pick::Slot(s) if flight.crash.reason == 0 => {
                markers.slots[s] = Some(Box::new(Mark::Plane(flight.clone())));
                Did::Placed(s)
            }
            _ => Did::Refused(pick),
        });
    }
    if up {
        did = Some(match pick {
            Pick::Slot(s) => match markers.slots[s].as_deref() {
                Some(Mark::Plane(kept)) => {
                    flight.put_back(kept);
                    Did::Teleported(pick)
                }
                None if s == HOME_SLOT => match home.plane(me) {
                    Some((at, facing)) => {
                        flight.reset(at, facing, acos);
                        Did::Teleported(pick)
                    }
                    None => Did::Refused(pick),
                },
                _ => Did::Refused(pick),
            },
            Pick::Player(p) if p < spots.len() => Did::Teleported(pick),
            Pick::Player(_) => Did::Refused(pick),
        });
    }
    did
}

impl State for Mark {
    /// A bike's marker walks as its ride, under the name P88's slots gave it, so a bike's markers
    /// hash as they did before a plane could have any.
    fn walk(&self, w: &mut Walk<'_>) {
        match self {
            Mark::Bike(ride) => ride.walk(w),
            Mark::Plane(f) => f.walk(w),
        }
    }
}

impl State for Markers {
    fn walk(&self, w: &mut Walk<'_>) {
        w.u8("at", self.at);
        walk_slots(w, &self.slots);
        // P107's: only once a machine but the bike has had markers, so a bike's alone walk as before.
        if self.on != Vehicle::Bike || !self.away.is_empty() {
            w.u8("on", self.on.byte());
            for set in &self.away {
                w.group(AWAY_NAMES[set.on.byte() as usize], |w| {
                    w.u8("at", set.at);
                    walk_slots(w, &set.slots);
                });
            }
        }
    }
}

fn walk_slots(w: &mut Walk<'_>, slots: &[Option<Box<Mark>>; SLOTS]) {
    for (i, slot) in slots.iter().enumerate() {
        w.group(SLOT_NAMES[i], |w| {
            w.bool("set", slot.is_some());
            match slot.as_deref() {
                Some(Mark::Bike(ride)) => w.field("ride", ride),
                Some(Mark::Plane(f)) => w.field("plane", f),
                None => {}
            }
        });
    }
}

const AWAY_NAMES: [&str; Vehicle::ALL.len()] = ["away_bike", "away_plane"];

const SLOT_NAMES: [&str; SLOTS] = ["slot0", "slot1", "slot2", "slot3", "slot4"];

// ---------------------------------------------------------------------------------------------------
// Saved between sessions (P99).
// ---------------------------------------------------------------------------------------------------

/// What a saved-markers file begins with.
const FILE_MAGIC: [u8; 8] = *b"OMXmarks";

/// The saved-markers file's format. Bump it when the file changes shape; a file of another version
/// is dropped ([`Markers::from_file`] says why), never read as this one. A change to the machine's own
/// codec is caught too: the slots are [`crate::snapshot::encode_slots`]'s (a plane's
/// [`crate::snapshot::encode_flights`]'s), which carry the snapshot's format and must read back to
/// the last byte. Version 2 (P107) names the machine after the name; P99's version 1, which has no
/// machine, is still read, as the bike's.
pub const FILE_VERSION: u32 = 2;

/// What a bundle of saved-markers files begins with ([`Markers::to_files`]).
const BUNDLE_MAGIC: [u8; 8] = *b"OMXmarkS";

impl Markers {
    /// Every slot emptied (P99, the pause's Clear Markers) on every machine, the selections left
    /// where they are.
    pub fn clear(&mut self) {
        self.slots = Default::default();
        for set in &mut self.away {
            set.slots = Default::default();
        }
        self.away.retain(|s| !s.fresh());
    }

    /// The bike's slots as a file ([`Markers::set_file`]).
    pub fn to_file(&self, bike: &str) -> Vec<u8> {
        self.set_file(Vehicle::Bike, bike)
    }

    /// `on`'s slots as a file, for keeping between sessions: [`FILE_MAGIC`], [`FILE_VERSION`], the
    /// machine they were placed on by name (`name`, the `.phy`'s — a marker is the whole machine, and
    /// another bike's is no use to this one) and kind, then the slots. The selection is not kept: a
    /// ride starts on the first slot.
    pub fn set_file(&self, on: Vehicle, name: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 * 1024);
        out.extend_from_slice(&FILE_MAGIC);
        out.extend_from_slice(&FILE_VERSION.to_le_bytes());
        out.extend_from_slice(&(name.len() as u32).to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.push(on.byte());
        let empty = Default::default();
        let slots = self.slots_of(on).unwrap_or(&empty);
        match on {
            Vehicle::Bike => {
                let rides = slots.each_ref().map(|m| m.as_deref().and_then(Mark::ride).map(|r| Box::new(r.clone())));
                out.extend_from_slice(&crate::snapshot::encode_slots(&rides));
            }
            Vehicle::Plane => {
                let flights = slots.each_ref().map(|m| m.as_deref().and_then(Mark::flight).map(|f| Box::new(f.clone())));
                out.extend_from_slice(&crate::snapshot::encode_flights(&flights));
            }
        }
        out
    }

    /// [`Markers::set_file`]'s bytes back, on the first slot, if they are this format's and `name`'s
    /// (P99's of a bike); an error saying why not otherwise, for the caller to drop the file on. The
    /// set is its machine's, whichever is up.
    pub fn from_file(bytes: &[u8], name: &str) -> Result<Markers, String> {
        let (named, markers) = Markers::read_file(bytes)?;
        if named != name {
            return Err(format!("markers of the {named}, not the {name}"));
        }
        Ok(markers)
    }

    /// [`Markers::set_file`]'s bytes back whatever machine they name, and its name: a tape's
    /// ([`crate::replay::PadTape::markers`]), which rides the machines it was made on. A bundle
    /// ([`Markers::to_files`]) reads too, every machine's set in it, under the first file's name.
    pub fn read_file(bytes: &[u8]) -> Result<(String, Markers), String> {
        if bytes.starts_with(&BUNDLE_MAGIC) {
            let mut markers = Markers::default();
            let mut first = None;
            for file in bundle(bytes)? {
                let (named, set) = Markers::read_file(file)?;
                first.get_or_insert(named);
                markers.join(set);
            }
            return Ok((first.unwrap_or_default(), markers));
        }
        let head = FILE_MAGIC.len() + 8;
        if bytes.len() < head || bytes[..FILE_MAGIC.len()] != FILE_MAGIC {
            return Err("not a markers file".into());
        }
        let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().expect("four bytes"));
        let version = word(FILE_MAGIC.len());
        if version != FILE_VERSION && version != 1 {
            return Err(format!("markers format {version}, this build reads {FILE_VERSION}"));
        }
        let n = word(FILE_MAGIC.len() + 4) as usize;
        let mut end = head.checked_add(n).filter(|&e| e <= bytes.len()).ok_or("the markers file ends early")?;
        let named = String::from_utf8(bytes[head..end].to_vec()).map_err(|_| "the markers file's machine is not text")?;
        let on = if version == 1 {
            Vehicle::Bike
        } else {
            let b = *bytes.get(end).ok_or("the markers file ends early")?;
            end += 1;
            Vehicle::of_byte(b).ok_or_else(|| format!("markers of a machine {b} this build has not"))?
        };
        let slots = match on {
            Vehicle::Bike => crate::snapshot::decode_slots(&bytes[end..])?.map(|r| r.map(|r| Box::new(Mark::Bike(*r)))),
            Vehicle::Plane => crate::snapshot::decode_flights(&bytes[end..])?.map(|f| f.map(|f| Box::new(Mark::Plane(*f)))),
        };
        let mut markers = Markers::default();
        markers.put(Set { on, slots, at: 0 });
        Ok((named, markers))
    }

    /// Every machine's slots in one (P107), for a tape and an online pick: `names` names each
    /// machine's file ([`Markers::set_file`]). Only the machines with a marker go in; with only the
    /// first's (or none), it is that machine's file alone, so a ride of bike markers only carries the
    /// bytes it did before a plane could have any. [`Markers::read_file`] reads either.
    pub fn to_files(&self, names: &[(Vehicle, &str)]) -> Vec<u8> {
        let with: Vec<&(Vehicle, &str)> = names.iter().filter(|(v, _)| self.any_on(*v)).collect();
        match (with.as_slice(), names.first()) {
            ([], Some(&(v, n))) => self.set_file(v, n),
            ([(v, n)], Some(first)) if *v == first.0 => self.set_file(*v, n),
            _ => {
                let mut out = BUNDLE_MAGIC.to_vec();
                out.extend_from_slice(&(with.len() as u32).to_le_bytes());
                for (v, n) in with {
                    let file = self.set_file(*v, n);
                    out.extend_from_slice(&(file.len() as u32).to_le_bytes());
                    out.extend_from_slice(&file);
                }
                out
            }
        }
    }

    /// [`Markers::to_files`]'s bytes back, every machine's file checked against its name in `names`
    /// ([`Markers::from_file`]).
    pub fn from_files(bytes: &[u8], names: &[(Vehicle, &str)]) -> Result<Markers, String> {
        let files = if bytes.starts_with(&BUNDLE_MAGIC) { bundle(bytes)? } else { vec![bytes] };
        let mut markers = Markers::default();
        for file in files {
            let (named, set) = Markers::read_file(file)?;
            let on = Vehicle::ALL.into_iter().find(|&v| set.any_on(v)).unwrap_or_default();
            match names.iter().find(|(v, _)| *v == on) {
                Some((_, n)) if *n == named => markers.join(set),
                Some((_, n)) => return Err(format!("markers of the {named}, not the {n}")),
                None => return Err(format!("markers of the {named}, which this ride has not")),
            }
        }
        Ok(markers)
    }

    /// `other`'s sets put in as their machines' ([`Markers::put`]), the ones it has no marker on left
    /// alone.
    pub fn join(&mut self, other: Markers) {
        let Markers { slots, at, on, away } = other;
        for set in std::iter::once(Set { on, slots, at }).chain(away) {
            if !set.fresh() {
                self.put(set);
            }
        }
    }
}

/// A bundle's files, all of them or an error.
fn bundle(bytes: &[u8]) -> Result<Vec<&[u8]>, String> {
    let short = || "the markers bundle ends early".to_string();
    let word = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_le_bytes(b.try_into().expect("four bytes")) as usize);
    let n = word(BUNDLE_MAGIC.len()).ok_or_else(short)?;
    if n > Vehicle::ALL.len() {
        return Err(format!("a markers bundle of {n} files"));
    }
    let mut at = BUNDLE_MAGIC.len() + 4;
    let mut files = Vec::with_capacity(n);
    for _ in 0..n {
        let len = word(at).ok_or_else(short)?;
        let file = bytes.get(at + 4..at + 4 + len).ok_or_else(short)?;
        files.push(file);
        at += 4 + len;
    }
    if at != bytes.len() {
        return Err(format!("{} bytes left over after the markers bundle", bytes.len() - at));
    }
    Ok(files)
}
