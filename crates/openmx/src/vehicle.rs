//! Which machine the player is on: the bike, or the freeride's biplane (P19).
//!
//! A freeride world carries more machines than the one the rider starts on. The track's own
//! `machine.txt` parks them — on free06 a biplane, `[Biplane1]`, standing on the dirt well away from
//! the start line — and the original lets the rider take one over in two ways: a prompt when the
//! rider stops within 30 ft of a driverless one ("Would you like to switch to the Biplane?",
//! `STR_SWITCH_BIPLANE`), and the pause menu's "Switch To Machine". Both answer through the one swap
//! routine (R8's reading, `research/oracle.txt`'s `machine_swap`), which hides the bike where it was
//! left and resets the plane to where `machine.txt` parked it.
//!
//! The port's switch is not that swap (P102, the user's, 2026-10-21): [`Garage::switch`] names the
//! machine wanted and **spawns it on the player** — a plane taken from the bike stands landed where
//! the bike was, on its wheels on the ground at rest, nose along the bike's heading; a bike taken
//! from the plane is stood upright on the ground under it, at rest, along the plane's heading. The
//! machine left goes: it is not stepped, not drawn ([`Garage::plane_shown`]) and, as nothing in the
//! port collides with a machine, not collided with. A plane nobody has taken yet still stands in its
//! bay. The original has no such switch.
//!
//! The shell's key and the pause are the menu's path: the switch with no 30 ft and no 40 mph gate,
//! which belong to the prompt.
//!
//! # The bays and the prompt (P103)
//!
//! The lot's machines stand at their bays as markers: drawn see-through ([`BAY_ALPHA`]), colliding
//! with nothing, and there whether or not their player has taken their own copy out — the switch
//! spawns the player's copy on them, so the bay stays where it can be come back to. Riding up to
//! one, slowly and upright, offers it with the original's own pop-up ([`offer`], [`offer_text`];
//! drawn by `overlay.rs`), and its button, Circle, is the switch. A setting turns the bays off, and
//! with them the pop-up; the pause's switch stays.
//!
//! # The plane's flight
//!
//! [`Plane`] carries the plane's own simulation, [`game_sim::flight::Flight`] built off the disc's
//! `biplane.phy`, and [`Garage::step`] runs one frame of it every tick the plane has the controller,
//! on the seat's pad through I6's mapping ([`flight::pad`]) and over the same `world` the bike asks
//! (P31). The plane touches that world only through its three wheels: the collision search its body's
//! contacts would come from is S45/S46/S48's, so over a track it flies over the heightfield alone and
//! passes through anything standing on it. A plane built without its file (`Plane::parked` alone, as
//! the unit tests below do) has no flight and stands where it was parked.
//!
//! The plane is not [`Sim::plane`]: the shell's `Sim` is the bike's ride, which a plane session never
//! steps, so its tick count stays where the switch found it.
//!
//! # One of each per player (P87)
//!
//! The original parks two of every machine on a freeride track, one for each of its two players
//! (`[Biplane1]`, `[Biplane2]`, the same for the helicopter, the buggy, the monster truck and the
//! baja), and its second player's wears the disc's alternate livery. The port's rides have as many
//! players as join, so a [`Lot`] parks one of each machine per player: the file's spots first, then
//! beside them ([`park`]), the liveries alternating ([`livery`]). Copy `n` is player `n`'s, which is
//! what keeps two players off one machine: the swap only ever takes the player's own.
//!
//! This file is std and `game_sim` only, for the same reason `pad.rs` is: the ride test compiles it
//! in by path, because a binary crate has nothing a test can `use`.

use game_sim::Sim;
use game_sim::flight::{self, Flight};
use game_sim::phy::Ini;
use game_sim::ride::Controls;
use game_sim::rider;
use game_sim::markers::{self, Did, Pick, Spot};
use game_sim::world::World;
use ps2_float::{F, Vf};

/// The two machines the shell can put the player on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The motocross bike: the ride, the rider and everything the shell did before P19.
    Bike,
    /// The freeride's biplane: the original's machine kind 1.
    Biplane,
}

impl Kind {
    /// `--vehicle`'s word for it.
    pub fn parse(word: &str) -> Result<Kind, String> {
        match word.to_ascii_lowercase().as_str() {
            "bike" => Ok(Kind::Bike),
            "biplane" | "plane" => Ok(Kind::Biplane),
            _ => Err(format!("--vehicle: {word:?} is not a vehicle; it is bike or biplane")),
        }
    }

    /// What the shell and its overlay call it.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Bike => "bike",
            Kind::Biplane => "biplane",
        }
    }

    /// The machine a swap goes to from this one: there are two.
    pub fn other(self) -> Kind {
        match self {
            Kind::Bike => Kind::Biplane,
            Kind::Biplane => Kind::Bike,
        }
    }
}

/// Where a track's `machine.txt` parks a machine: its `Position` and its `Direction`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spawn {
    pub at: Vf,
    pub facing: Vf,
}

impl Spawn {
    /// Player `player`'s biplane out of a track's `machine.txt`: the section the original's machine
    /// placement reads when that player's vehicle kind is 1, `[Biplane<n>]` with `n` counting from
    /// one. `None` for a track whose file has no such section — of the freeride tracks, free06, 07,
    /// 09, 10 and 11 have one (R8).
    ///
    /// `SetGroundHeight` (on by default) is not applied: what the placement does with it was not
    /// read past the call it changes, and on free06 the position the file gives is the one used.
    pub fn biplane(machine_txt: &[u8], player: usize) -> Option<Spawn> {
        Spawn::machine(machine_txt, Machine::Biplane, player)
    }

    /// Copy `copy` of `machine` out of a track's `machine.txt`: the section the original's machine
    /// placement reads for a player whose vehicle kind is that machine's, its section word and the
    /// player counting from one (`[Helicopter2]`, `[Monster1]`). `None` where the file stops.
    pub fn machine(machine_txt: &[u8], machine: Machine, copy: usize) -> Option<Spawn> {
        Spawn::section(&Ini::parse_bytes(machine_txt), &format!("{}{}", machine.section(), copy + 1))
    }

    fn section(ini: &Ini, name: &str) -> Option<Spawn> {
        if !ini.names().iter().any(|n| n.eq_ignore_ascii_case(name)) {
            return None;
        }
        let s = ini.section(name);
        s.text("Position")?;
        let at = s.vector("Position", Vf::default());
        let facing = s.vector("Direction", Vf::new(0.0, 0.0, 1.0, 0.0));
        Some(Spawn {
            at: Vf::new(at.x(), at.y(), at.z(), 1.0),
            facing: Vf::new(facing.x(), facing.y(), facing.z(), 0.0),
        })
    }
}

/// Every machine the disc carries besides the bike: `machine.res` has a directory for each (model,
/// collision, `.phy`), and the vehicle factory builds them by the original's kind number, 1 to 5
/// (R8, `research/oracle.txt`'s `vehicle_factory`). Every freeride track's `machine.txt` (free06, 07,
/// 09, 10 and 11) parks two of each, `[<section>1]` and `[<section>2]`, one per player of the
/// original's two. Only the biplane is ported ([`Machine::kind`]); the others are each a task of
/// their own, and until one lands the lot parks it on paper and the shell does not stand it up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Machine {
    /// Kind 1, MXBiplane: `biplane\`.
    Biplane,
    /// Kind 2, the Kiowa helicopter: `helicopter\`, `kiowa.phy`.
    Helicopter,
    /// Kind 3, the dune buggy: `buggy\`.
    Buggy,
    /// Kind 4, MXMonsterTruck: `monstertruck\`, sections `[Monster<n>]`.
    Monster,
    /// Kind 5, the baja truck: `baja\`.
    Baja,
}

impl Machine {
    /// In the original's kind order.
    pub const ALL: [Machine; 5] = [Machine::Biplane, Machine::Helicopter, Machine::Buggy, Machine::Monster, Machine::Baja];

    /// The original's kind number: the vehicle factory's switch and the event's per-player kind.
    #[allow(dead_code)] // the disc test's, until a second machine is ported
    pub fn number(self) -> u32 {
        match self {
            Machine::Biplane => 1,
            Machine::Helicopter => 2,
            Machine::Buggy => 3,
            Machine::Monster => 4,
            Machine::Baja => 5,
        }
    }

    /// The word the placement's sections start with, and the `Type` the file gives them.
    pub fn section(self) -> &'static str {
        match self {
            Machine::Biplane => "Biplane",
            Machine::Helicopter => "Helicopter",
            Machine::Buggy => "Buggy",
            Machine::Monster => "Monster",
            Machine::Baja => "Baja",
        }
    }

    /// Its directory in `machine.res`.
    #[allow(dead_code)] // the disc test's, until a second machine is ported
    pub fn dir(self) -> &'static str {
        match self {
            Machine::Biplane => "biplane",
            Machine::Helicopter => "helicopter",
            Machine::Buggy => "buggy",
            Machine::Monster => "monstertruck",
            Machine::Baja => "baja",
        }
    }

    /// Its four models, `<dir>\anim\<name>.psg`, in the factory's skin order: the livery, the
    /// alternate livery, and the split-screen texture sets of the same two (`_ss`, whose textures are
    /// the first two's with `_ss` on the name).
    pub fn models(self) -> [&'static str; 4] {
        match self {
            Machine::Biplane => ["biplane", "biplane2", "biplane_ss", "biplane2_ss"],
            Machine::Helicopter => ["kiowa", "kiowa2", "kiowa_ss", "kiowa2_ss"],
            Machine::Buggy => ["buggy", "buggy2", "buggy_ss", "buggy2_ss"],
            Machine::Monster => ["monstertruck", "monstertruck2", "monstertruck_ss", "monstertruck2_ss"],
            Machine::Baja => ["baja", "baja2", "baja_ss", "baja2_ss"],
        }
    }

    /// The shell's machine for it, where the port has one.
    pub fn kind(self) -> Option<Kind> {
        match self {
            Machine::Biplane => Some(Kind::Biplane),
            _ => None,
        }
    }

    /// How far apart the port parks the copies it adds past the file's ([`park`]), feet: more than
    /// the machine is across, so no two stand in each other. The port's own numbers, from the
    /// models' size and the file's own pairs (its trucks stand 18 ft apart, its planes 25 to 60).
    pub fn spacing(self) -> f32 {
        match self {
            Machine::Biplane => 40.0,
            Machine::Helicopter => 50.0,
            Machine::Buggy | Machine::Baja => 18.0,
            Machine::Monster => 22.0,
        }
    }
}

/// How many liveries the disc has of each machine: the factory's skins 0 and 1. Its 2 and 3 are the
/// same two in split-screen textures, which the port does not need.
pub const LIVERIES: usize = 2;

/// The livery copy `copy` of a machine wears, as an index into [`Machine::models`]. The original
/// parks its second player's machine in skin 1, the alternate (`biplane2`, `kiowa2`, …), and its
/// first in skin 0 (the event's own pick, 0 by default); the port's copies past two cycle the two.
pub fn livery(copy: usize) -> usize {
    copy % LIVERIES
}

/// One parked machine: which, which copy of it (and so whose: copy `n` is player `n`'s), where it
/// stands, and the livery it wears.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bay {
    pub machine: Machine,
    pub copy: usize,
    pub spawn: Spawn,
    pub livery: usize,
    /// Whether the spot is the file's own, rather than one the port found beside it.
    pub original: bool,
}

/// The original's spots for `machine` on a track: `[<section>1]`, `[<section>2]`, … for as many as
/// the file has.
pub fn spots(machine_txt: &[u8], machine: Machine) -> Vec<Spawn> {
    let ini = Ini::parse_bytes(machine_txt);
    (0..).map_while(|i| Spawn::section(&ini, &format!("{}{}", machine.section(), i + 1))).collect()
}

/// `players` copies of `machine`, one a player: the first as many as the file has stand where it
/// parks them, exactly, and the rest beside those in turn — copy `k` beside the file's spot
/// `k % spots`, stepped sideways (across its `Direction`) one [`Machine::spacing`] at a time, right
/// then left, to the first place at least that far from every copy already parked. The height keeps
/// the file spot's clearance over the ground, which `ground` gives (feet, the dirt's height at a
/// point). There is no cap: a ride of any size parks one each. A copy's place depends only on the
/// copies before it, so a ride that grows keeps every machine where it was.
///
/// The original has no copies past two and the nearby rule is the port's; a track whose file parks
/// none of the machine gets none.
pub fn park(machine_txt: &[u8], machine: Machine, players: usize, ground: &mut dyn FnMut(Vf) -> f32) -> Vec<Bay> {
    park_around(&spots(machine_txt, machine), machine, players, ground)
}

/// [`park`]'s rule over spots given as `file`: the first as many copies as there are spots stand on
/// them, and the rest beside them.
fn park_around(file: &[Spawn], machine: Machine, players: usize, ground: &mut dyn FnMut(Vf) -> f32) -> Vec<Bay> {
    let mut bays: Vec<Bay> = Vec::with_capacity(players);
    if file.is_empty() {
        return bays;
    }
    let gap = machine.spacing();
    for copy in 0..players {
        let livery = livery(copy);
        if let Some(&spawn) = file.get(copy) {
            bays.push(Bay { machine, copy, spawn, livery, original: true });
            continue;
        }
        let base = file[copy % file.len()];
        let (fx, fz) = (base.facing.x(), base.facing.z());
        let len = fx.hypot(fz);
        // Across the nose, flat; a spot facing straight up or down steps along x.
        let (rx, rz) = if len > 1e-6 { (-fz / len, fx / len) } else { (1.0, 0.0) };
        let clear = base.at.y() - ground(base.at);
        // A step's own distance comes out a hair either side of `gap`, so a spot one step away counts.
        let free = |x: f32, z: f32, bays: &[Bay]| bays.iter().all(|b| (b.spawn.at.x() - x).hypot(b.spawn.at.z() - z) >= gap - 0.01);
        let (x, z) = (1..)
            .flat_map(|step: i32| [step, -step])
            .map(|step| (base.at.x() + rx * gap * step as f32, base.at.z() + rz * gap * step as f32))
            .find(|&(x, z)| free(x, z, &bays))
            .expect("an unbounded row always has a free place");
        let y = ground(Vf::new(x, base.at.y(), z, 1.0)) + clear;
        let spawn = Spawn { at: Vf::new(x, y, z, 1.0), facing: base.facing };
        bays.push(Bay { machine, copy, spawn, livery, original: false });
    }
    bays
}

/// Where each of `players` copies of `machine` starts on a map whose `machine.txt` parks none of it
/// (P113: a race track, a free track without the section, a mod map): the ride's start line `start`
/// taken as if it were the file's one spot — copy 0 on it, where the first player starts, and the
/// rest beside it one [`Machine::spacing`] apart, as [`park`] stands the copies past the file's — each
/// with its origin `rest` feet over the ground there ([`Plane::rest_height`]: on its wheels), nose
/// along the start's heading flattened. Nothing is parked or drawn there: these are only where a
/// player who starts the ride on the machine (`--vehicle`, the setup's pick, an online seat's) is
/// put, since a switch mid-ride spawns it on the player wherever they are. The port's own spots.
pub fn start_spots(start: Spawn, machine: Machine, players: usize, rest: f32, ground: &mut dyn FnMut(Vf) -> f32) -> Vec<Spawn> {
    let facing = level(start.facing).unwrap_or(Vf::new(0.0, 0.0, 1.0, 0.0));
    let base = Spawn { at: Vf::new(start.at.x(), ground(start.at) + rest, start.at.z(), 1.0), facing };
    park_around(&[base], machine, players, ground).into_iter().map(|b| b.spawn).collect()
}

/// Every machine a ride parks, one of each per player, sized to the player count as players join and
/// leave.
#[derive(Clone, Debug, Default)]
pub struct Lot {
    machine_txt: Vec<u8>,
    pub bays: Vec<Bay>,
}

impl Lot {
    /// A track's lot for `players`: [`park`] for every machine in [`Machine::ALL`].
    pub fn new(machine_txt: &[u8], players: usize, ground: &mut dyn FnMut(Vf) -> f32) -> Lot {
        let mut lot = Lot { machine_txt: machine_txt.to_vec(), bays: Vec::new() };
        lot.resize(players, ground);
        lot
    }

    /// Re-park for a new player count: a player joining adds one of each machine, beside the rest,
    /// and the last one leaving takes theirs away. Every copy that stays keeps its place.
    pub fn resize(&mut self, players: usize, ground: &mut dyn FnMut(Vf) -> f32) {
        self.bays = Machine::ALL.iter().flat_map(|&m| park(&self.machine_txt, m, players, ground)).collect();
    }

    /// The copies of `machine`, in copy order.
    pub fn of(&self, machine: Machine) -> impl Iterator<Item = &Bay> {
        self.bays.iter().filter(move |b| b.machine == machine)
    }

    /// Player `player`'s own copy of `machine`: the one they take and nobody else does.
    pub fn bay(&self, machine: Machine, player: usize) -> Option<&Bay> {
        self.of(machine).find(|b| b.copy == player)
    }

    /// Player `player`'s plane wherever the map is (P113): their own bay's ([`Lot::plane`]) where the
    /// track's `machine.txt` parks one for them, and otherwise a copy of the disc's plane standing at
    /// `start` (their [`start_spots`] spot), in their copy's livery, with no bay drawn anywhere. A
    /// missing section only means no parked copy: `None` only with no bay and no `flight` (a disc
    /// without `biplane.phy`), where there is nothing to fly.
    pub fn plane_anywhere(&self, player: usize, flight: Option<Flight>, start: Spawn) -> Option<Plane> {
        if self.bay(Machine::Biplane, player).is_some() {
            return self.plane(player, flight);
        }
        Some(Plane { livery: livery(player), ..Plane::flying(start, flight?) })
    }

    /// Player `player`'s plane, standing in their bay: parked, or flying when `flight` is given.
    pub fn plane(&self, player: usize, flight: Option<Flight>) -> Option<Plane> {
        let bay = self.bay(Machine::Biplane, player)?;
        let mut plane = match flight {
            Some(f) => Plane::flying(bay.spawn, f),
            None => Plane::parked(bay.spawn),
        };
        plane.livery = bay.livery;
        Some(plane)
    }
}

/// How near the rider has to be to a parked machine for the original's switch pop-up, feet: the
/// trigger's `< 30.0`, a straight-line distance in three dimensions (R8, P103).
pub const OFFER_RANGE: f32 = 30.0;

/// Above this the pop-up is not shown, feet a second: the prompt's 58.667, 40 mph (R8, P103).
pub const OFFER_TOP_SPEED: f32 = 58.667;

/// The alpha a machine standing in its bay is drawn at (P103): the put-back ghost's own starting
/// alpha ([`Ghost::START`]), held. The original draws its parked machines solid and they collide;
/// these are markers of the port's, see-through and colliding with nothing.
pub const BAY_ALPHA: f32 = Ghost::START.0;

/// The alpha a paused rider's machine and body are drawn at (P84): the put-back ghost's starting
/// alpha, held while their pause is open. A port's own state with nothing of the original's in it —
/// the original's pause stops the whole game — so the rider's own [`Ghost`] is left alone and the
/// fade a put-back runs is not started.
pub const PAUSED_ALPHA: f32 = Ghost::START.0;

/// What a seat's machine and rider are drawn at: the put-back ghost's alpha (1 when it is off), no
/// more than [`PAUSED_ALPHA`] while the seat is paused (P84).
pub fn shown_alpha(ghost: f32, paused: bool) -> f32 {
    if paused { ghost.min(PAUSED_ALPHA) } else { ghost }
}

/// The machine the original's switch pop-up offers a rider at `at` going `speed` (feet a second,
/// either way), `wrecked` or not, who is on `up` (P103): `freeride_near_machine`'s scan in
/// `research/oracle.txt`. Kind by kind in the original's order ([`Machine::ALL`]), the nearest bay
/// of that machine, any player's copy; the first kind whose nearest is closer than [`OFFER_RANGE`]
/// (and not on top of the rider, which the original's nearest-actor search skips) is offered. A
/// wrecked rider, or one faster than [`OFFER_TOP_SPEED`], is offered nothing.
///
/// Only the ported machines are looked at ([`Machine::kind`]), and never the one the rider is on.
/// The original also skips a machine its profile has not unlocked and one somebody is driving; the
/// port has no unlocks, and a bay's machine is a marker nobody drives (the switch spawns the
/// player's own copy on them, [`Garage::switch`]).
pub fn offer(bays: &[Bay], up: Kind, at: Vf, speed: f32, wrecked: bool) -> Option<Machine> {
    if wrecked || speed.abs() > OFFER_TOP_SPEED {
        return None;
    }
    let distance = |b: &Bay| {
        let (dx, dy, dz) = (b.spawn.at.x() - at.x(), b.spawn.at.y() - at.y(), b.spawn.at.z() - at.z());
        (dx * dx + dy * dy + dz * dz).sqrt()
    };
    Machine::ALL.into_iter().filter(|m| m.kind().is_some_and(|k| k != up)).find(|&m| {
        let nearest = bays.iter().filter(|b| b.machine == m).map(distance).filter(|&d| d > 1e-6).reduce(f32::min);
        nearest.is_some_and(|d| d < OFFER_RANGE)
    })
}

/// The pop-up's button, one a seat (P103): Circle answers it on the frame it goes down, and only
/// then, so a Circle held into range does not take the machine, and a press with nothing offered
/// does nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct Answer {
    was: bool,
}

impl Answer {
    /// This frame's Circle — `held` now, or `tapped` (down and up again) since the last frame — and
    /// what is offered: the machine to switch to, on a press while one is.
    pub fn take(&mut self, held: bool, tapped: bool, offered: Option<Machine>) -> Option<Kind> {
        let pressed = (held && !self.was) || tapped;
        self.was = held;
        offered.filter(|_| pressed).and_then(Machine::kind)
    }
}

/// The bays a ride shows (P103): every bay of a machine the port has ([`Machine::kind`]) while the
/// setting is `on`, and none when it is off — no ghost there, nothing on the radar, no pop-up.
pub fn bays_shown(lot: &Lot, on: bool) -> Vec<Bay> {
    match on {
        true => lot.bays.iter().filter(|b| b.machine.kind().is_some()).copied().collect(),
        false => Vec::new(),
    }
}

/// The plane a bay shows, standing exactly where it is parked in its copy's livery: what is drawn
/// see-through there ([`BAY_ALPHA`]). It has no flight and is never stepped, so nothing it could
/// touch is ever asked.
pub fn bay_plane(bay: &Bay) -> Plane {
    Plane { livery: bay.livery, ..Plane::parked(bay.spawn) }
}

/// The original's pop-up for `machine`, as the disc's English UI text has it: `STR_SWITCH_BIPLANE`
/// and the rest (`appstart.res`'s `loc\l_ui_eng.bin`).
pub fn offer_text(machine: Machine) -> &'static str {
    match machine {
        Machine::Biplane => "Would you like to switch to the Biplane?",
        Machine::Helicopter => "Would you like to switch to the Helicopter?",
        Machine::Buggy => "Would you like to switch to the Dune Buggy?",
        Machine::Monster => "Would you like to switch to the Monster Truck?",
        Machine::Baja => "Would you like to switch to the Trophy Truck?",
    }
}

/// How far past the horizon the plane's up has to go before [`Plane::inverted`] changes its answer:
/// the sine of about 11 degrees of roll past knife edge. The port's own feel, not the original's,
/// which never asks (P50).
pub const INVERTED_MARGIN: f32 = 0.2;

/// The plane's elevator and aileron off the left stick read round (P89, the plane-relative scheme):
/// `pad` is what the original's input step made of the same pad ([`flight::pad`]), and the stick's
/// two fields of it are replaced unless the d-pad holds them.
///
/// The original reads each axis on its own: a dead zone of 0.3 on each (a square, so a stick 17
/// degrees off straight back has no roll at all), then each axis cubed. A round gate's corner is
/// 0.71 on each axis, which that reading leaves at 0.19 of the pull and 0.20 of the roll — a held
/// diagonal had a fifth of what either straight push has. Read round, the stick's distance from
/// centre goes through the same 0.3, the same segment and the same cube, and its direction is
/// stretched out to the square, so the gate's edge is full travel in every direction: a held corner
/// is the full pull and the full roll, a half tilt the same part of each in the stick's own
/// proportion, and straight along an axis it is the original's reading to within rounding.
///
/// The centre byte reads `-1/255` through the original's arithmetic, which the dead zone hides; the
/// stretch would not, so an axis one byte either side of the centre is centred.
pub fn round_stick(input: &game_sim::input::Pad, mut pad: game_sim::biplane::Pad) -> game_sim::biplane::Pad {
    use game_sim::bindings::{Command, Table};
    use game_sim::input::{Axis, AxisRange, PlaneInput};
    let range = AxisRange::PLAYER;
    let straight = AxisRange { dead_hi: F::ZERO, dead_lo: F::ZERO, ..range };
    let read = |axis: Axis| {
        let v = input.axis_through(axis, &straight);
        if v.abs() < ROUND_STICK_CENTRE { F::ZERO } else { v }
    };
    let (x, y) = (read(Axis::LeftX), read(Axis::LeftY));
    let r = (x * x + y * y).sqrt().min(range.hi);
    let (sx, sy) = if r <= range.dead_hi {
        (F::ZERO, F::ZERO)
    } else {
        let m = (r - range.dead_hi) * ((range.hi - range.centre) / (range.hi - range.dead_hi)) + range.centre;
        let k = m * (m * m) / x.abs().max(y.abs());
        (x * k, y * k)
    };
    // The d-pad's bank and climb still win over the stick, as in the input step: what the step
    // makes of a centred stick is the d-pad's alone.
    let dpad = PlaneInput::from_sticks((F::ZERO, F::ZERO), |c| Table::SHIPPED.value(input, Command(c as u8)));
    if dpad.aileron == F::ZERO {
        pad.aileron = -sx;
    }
    if dpad.elevator == F::ZERO {
        pad.elevator = -sy;
    }
    pad
}

/// A seat's hands off its poll (P129): with its Markers switch on, the d-pad out of the ride
/// ([`Controls::stick_alone`]) — it is the markers' — and the left stick alone steers, leans and names
/// a trick's direction; off, the poll as it is ([`Controls::through`]), the d-pad the original's.
pub fn hands(polled: Controls, markers: bool) -> Controls {
    if markers { polled.stick_alone() } else { polled }
}

/// `pad` with its four d-pad directions let go (P107): what the shell's plane flies on, the d-pad
/// being the session markers' (`game_sim::markers::fly`). Everything else is the pad as it was.
pub fn without_dpad(pad: &game_sim::input::Pad) -> game_sim::input::Pad {
    use game_sim::input::{Control, ControlState};
    let mut pad = pad.clone();
    for c in [Control::Up, Control::Down, Control::Left, Control::Right] {
        pad.controls[c.index()] = ControlState::default();
    }
    pad
}

/// How far from 0 an axis read without a dead zone can be and still be the centre byte or its
/// neighbour (one byte is `2/255` of the travel): [`round_stick`].
const ROUND_STICK_CENTRE: f32 = 0.005;

/// What [`Plane::step`] scales the autopilot's `MaxPitchRate` by under the round stick (P108). The
/// file's 1.25 radians a second is less than full elevator pitches the biplane at speed, so the loop
/// reached it and took the elevator back off: banked 90 degrees at 121 ft/s, a held full pull turned
/// the plane 34 degrees in a second where full elevator turns it 75. At 2.5 times the loop asked for
/// more than full elevator reaches at any speed the plane flies, which the user found pulled a little
/// too hard (2026-10-22); 2.0 keeps most of that pull, and a partial pull still asks the loop for a
/// rate.
const ROUND_STICK_PITCH_RATE: f32 = 2.0;

/// The biplane: where it was parked, where it is, and the flight that moves it.
#[derive(Clone, Debug, PartialEq)]
pub struct Plane {
    /// Where it was last put down: its bay until it is taken, and from then where the switch spawned
    /// it, which is also where its reset chord and a wreck's recovery put it back.
    pub spawn: Spawn,
    /// Its bay in the lot: where `machine.txt` (or [`park`]) stands it before anybody takes it.
    pub bay: Spawn,
    /// Where the model's origin is now, which way its nose points, and its orientation.
    pub at: Vf,
    pub facing: Vf,
    pub quat: Vf,
    /// Its up, the orientation's second row: world up while it is parked.
    pub up: Vf,
    /// Its velocity, feet a second.
    pub vel: Vf,
    /// The plane's own simulation, when the disc's `biplane.phy` was there to build it.
    pub flight: Option<Flight>,
    /// The flight has been put back at the spawn since the last reset (which cannot place it itself:
    /// placing measures, and the measure's table is the `Sim`'s).
    pub placed: bool,
    /// The stick's elevator answers at every bank (P69, `pad::PlaneScheme::every_bank`): the
    /// plane-relative scheme lifts the original's knife-edge fade for each step by flying it as if the
    /// file's `MinElevator` were 1, which makes the fade exactly 1 at every roll. Off, the flight
    /// reads its file as the original does.
    pub every_bank: bool,
    /// The plane's stick is read round and its pitch loop never lets go (P89,
    /// `pad::PlaneScheme::round_stick`): [`round_stick`] in place of the original's square reading,
    /// and the autopilot's `AileronDeadZone` lifted for each step, so the elevator asks for a pitch
    /// rate whatever the aileron does — with the loop's coordinated-turn pull faded out as the
    /// aileron leaves the original's dead zone. Off, the flight reads the pad and its file as the
    /// original does.
    pub round_stick: bool,
    /// The plane reads the d-pad as the original's does (P129): its seat's Markers switch is off,
    /// so the d-pad is no marker's. Off — markers on, the default — it flies on [`without_dpad`].
    pub dpad: bool,
    /// Which of the disc's liveries it wears, an index into [`Machine::models`] ([`livery`]).
    pub livery: usize,
}

impl Plane {
    /// A plane standing where it was parked, with no flight in it.
    pub fn parked(spawn: Spawn) -> Plane {
        Plane {
            spawn,
            bay: spawn,
            at: spawn.at,
            facing: spawn.facing,
            quat: Vf::new(0.0, 0.0, 0.0, 1.0),
            up: Vf::new(0.0, 1.0, 0.0, 0.0),
            vel: Vf::new(0.0, 0.0, 0.0, 0.0),
            flight: None,
            placed: false,
            every_bank: false,
            round_stick: false,
            dpad: false,
            livery: 0,
        }
    }

    /// The plane with its flight: `flight` is [`Flight::new`] off the disc's file, which the next
    /// [`Plane::step`] stands at the spawn.
    pub fn flying(spawn: Spawn, mut flight: Flight) -> Plane {
        // The player's plane: the controls read the pad (an unflown one is the autopilot's alone).
        flight.attitude.flown = true;
        Plane { flight: Some(flight), ..Plane::parked(spawn) }
    }

    /// Stood at `spawn` as where it starts and stands (P113's [`start_spots`], once the ride's ground
    /// is there to measure them on), at rest; its flight is put there by its next step.
    pub fn stand_at(&mut self, spawn: Spawn) {
        (self.spawn, self.bay) = (spawn, spawn);
        self.reset();
    }

    /// The unwreck-and-reset: back to the spawn transform, at rest. The flight is put back by the
    /// next step ([`Flight::reset`]), which is the one with the `Sim`'s tables.
    pub fn reset(&mut self) {
        let (flight, every_bank, round_stick, dpad, livery, bay) =
            (self.flight.take(), self.every_bank, self.round_stick, self.dpad, self.livery, self.bay);
        *self = Plane { flight, every_bank, round_stick, dpad, livery, bay, ..Plane::parked(self.spawn) };
    }

    /// How high its origin stands over the ground under it when it is parked, feet. With its flight,
    /// the lowest of its tires' bottoms is on the ground (the tires' centres are in the origin's
    /// frame); without one, the clearance its bay has over the ground there, `bay_ground` being the
    /// ground's height under the bay.
    pub fn rest_height(&self, bay_ground: f32) -> f32 {
        match &self.flight {
            Some(f) => -f.tires.iter().map(|t| t.center.y() - t.radius.0).fold(f32::MAX, f32::min),
            None => self.bay.at.y() - bay_ground,
        }
    }

    /// One frame of the plane with the controller on it: the reset chord ([`Flight::reset_chord`],
    /// S57) and [`Flight::frame`] on the seat's pad, over `world`, and the drawn transform off its
    /// body. A plane with no flight stands where it is. Whether the chord or a wreck's recovery put it
    /// back at its spawn this frame: both are the original's plane reset, which snaps its camera (V29).
    pub fn step(&mut self, sim: &Sim, dt: F, world: &mut dyn World) -> bool {
        let Some(f) = &mut self.flight else { return false };
        if !self.placed {
            f.reset(self.spawn.at, self.spawn.facing, &sim.tables.ride.acos);
            self.placed = true;
        }
        // The input slot runs before the frame. L1 and R1 are the chord under either scheme: P40's
        // new one moves the gas and the brake, not the shoulder buttons.
        let chord = f.reset_chord(Controls::from_pad(sim.input.pad(0)).respawn, &sim.tables.ride.acos);
        let wrecked = f.crash.reason != 0;
        // The control law's fade runs from 1 down to `MinElevator`; with that 1 too it is 1 at every
        // roll. Put back after the frame, so the flight still holds its file's value.
        let min_elevator = f.joystick.min_elevator;
        if self.every_bank {
            f.joystick.min_elevator = F::ONE;
        }
        // The autopilot's pitch loop runs while the aileron is inside its dead zone, and no aileron
        // is outside one wider than the stick's whole travel. Its coordinated-turn pull is for a bank
        // that is held, not one being rolled through: past the dead zone it fades out over the same
        // width again, or a held roll would have it pushing as the plane goes over the top (it is a
        // push banked past 110 degrees). Both put back after the frame.
        // The loop's full-stick pitch rate is raised too (P108): at the file's own the loop reaches
        // it at speed and takes the elevator back off, so a hard pull on its side turned no faster
        // than a slow one. All three put back after the frame.
        let (dead_zone, feed_forward, max_pitch_rate) =
            (f.autopilot.aileron_dead_zone, f.autopilot.feed_forward, f.autopilot.max_pitch_rate);
        // The d-pad is the session markers' (P107), as the bike's has been since P93: the plane
        // reads the pad without it, under every scheme, and the left stick alone banks and climbs.
        // With the seat's markers off (P129) it is the original's again.
        let input = if self.dpad { sim.input.pad(0).clone() } else { without_dpad(sim.input.pad(0)) };
        let mut pad = flight::pad(&input);
        if self.round_stick {
            pad = round_stick(&input, pad);
            let held = ((dead_zone + dead_zone - pad.aileron.abs()) / dead_zone).max(F::ZERO).min(F::ONE);
            f.autopilot.aileron_dead_zone = F(2.0);
            f.autopilot.feed_forward = feed_forward * held;
            f.autopilot.max_pitch_rate = max_pitch_rate * F(ROUND_STICK_PITCH_RATE);
        }
        f.frame(dt, pad, world, &sim.tables.ride);
        f.joystick.min_elevator = min_elevator;
        (f.autopilot.aileron_dead_zone, f.autopilot.feed_forward, f.autopilot.max_pitch_rate) =
            (dead_zone, feed_forward, max_pitch_rate);
        let recovered = wrecked && f.crash.reason == 0;
        self.pose();
        chord || recovered
    }

    /// The drawn transform off the flight's body: where it is, its nose, its up and its velocity.
    fn pose(&mut self) {
        let Some(f) = &self.flight else { return };
        let (quat, origin) = f.pose();
        let nose = f.measure.forward;
        (self.quat, self.at) = (quat, Vf::new(origin.x(), origin.y(), origin.z(), 1.0));
        self.facing = Vf::new(nose.x(), nose.y(), nose.z(), 0.0);
        let up = f.measure.up;
        self.up = Vf::new(up.x(), up.y(), up.z(), 0.0);
        let v = f.body.velocity;
        self.vel = Vf::new(v.x(), v.y(), v.z(), 0.0);
    }

    /// A plane round a flight already placed (N17's online seat, whose flight is its [`Sim::plane`]):
    /// its spawn the flight's own, the transform off its body, flown on the pad.
    fn around(flight: Flight) -> Plane {
        let (at, facing) = flight.spawn;
        let mut plane = Plane { placed: true, ..Plane::flying(Spawn { at, facing }, flight) };
        plane.pose();
        plane
    }

    /// Whether the plane is standing on something: any wheel down at the last measure. A plane with
    /// no flight is parked, and so is.
    pub fn on_ground(&self) -> bool {
        self.flight.as_ref().is_none_or(|f| f.measure.grounded)
    }

    /// Whether the plane is upside down as the player sees it, for the pad's remap
    /// (`pad::PlaneScheme::fly`, P50): its up pointing below the horizon. `was` is last frame's
    /// answer, and the answer only changes once the up is [`INVERTED_MARGIN`] past the horizon the
    /// other way, so a plane flown along knife edge does not swap its stick back and forth every frame.
    ///
    /// The camera that follows the plane keeps the world's up, so this is the attitude at which
    /// the plane's own climb and the screen's part company. It reads the plane after the last step,
    /// which is the attitude the player was looking at when they moved the stick.
    pub fn inverted(&self, was: bool) -> bool {
        let up = self.up.y();
        if was { up < INVERTED_MARGIN } else { up < -INVERTED_MARGIN }
    }

    /// Its speed over everything, feet a second.
    pub fn speed(&self) -> F {
        F(self.vel.x().hypot(self.vel.y()).hypot(self.vel.z()))
    }

    /// The throttle, `0..1`, and the airspeed, feet a second, as the plane's own simulation holds them.
    pub fn gauges(&self) -> Option<(F, F)> {
        self.flight.as_ref().map(|f| (f.surfaces.throttle, f.measure.airspeed))
    }

    /// The prop's angle, radians, as the engine turns it, or `None` once the plane is wrecked and
    /// the prop is not turned (S35). A parked plane's prop stands at zero.
    pub fn prop_angle(&self) -> Option<F> {
        match &self.flight {
            None => Some(F::ZERO),
            Some(f) => (f.crash.reason == 0).then_some(f.engine.prop_angle),
        }
    }
}

/// Which machine has the player's controller, and the plane beside the bike.
///
/// The bike is the [`Sim`] the shell already runs — its ride, its rider, its pad — and stays there;
/// the garage only decides whether a frame reaches it.
#[derive(Clone, Debug)]
pub struct Garage {
    pub up: Kind,
    /// This player's own copy of the track's biplane ([`Lot::plane`]), if its `machine.txt` parks one.
    pub plane: Option<Plane>,
    /// Whether the player has taken their plane out of its bay. Until they do it stands there parked
    /// and is drawn there; once they have, it goes wherever they take it and is nowhere while they
    /// are on the bike.
    pub taken: bool,
}

/// How far above a machine's origin the ground under it is looked for when the other is spawned
/// there, feet: enough for a bike's origin sitting below a ridge it leans over, and not so much that
/// a machine under a bridge comes out on top of it. The port's own number.
const PROBE: f32 = 20.0;

/// Below this the ground query found nothing ([`game_sim::world::World::ground`]'s no-ground answer is
/// a million feet down).
const NO_GROUND: f32 = -1.0e5;

/// A heading flattened onto the ground plane, or `None` straight up or down.
fn level(v: Vf) -> Option<Vf> {
    let (x, z) = (v.x(), v.z());
    let len = x.hypot(z);
    (len > 1e-4).then(|| Vf::new(x / len, 0.0, z / len, 0.0))
}

/// The ground's height under `at`, looked for from [`PROBE`] above it, or `None` off the map.
fn ground_under(world: &mut dyn World, at: Vf) -> Option<f32> {
    let (on, _) = world.ground(Vf::new(at.x(), at.y() + PROBE, at.z(), 1.0));
    (on.y() > NO_GROUND).then_some(on.y())
}

/// The plane put down landed at `spot` (P102's switch onto it, P107's marker onto a player at rest
/// since P141, [`arrive`]): nose
/// along `spot`'s heading, level — its bay's heading where that points straight up or down — on its
/// wheels on the ground and at rest ([`Plane::rest_height`]), unwrecked and its throttle shut, as a
/// parked plane stands. Its flight is put there by its next step ([`Plane::reset`]). `None`, and
/// nothing moves, with no ground there.
fn land(plane: &mut Plane, spot: Spot, world: &mut dyn World) -> Option<()> {
    let ground = ground_under(world, spot.at)?;
    let bay_ground = ground_under(world, plane.bay.at).unwrap_or(plane.bay.at.y());
    let facing = level(spot.forward).or_else(|| level(plane.bay.facing)).unwrap_or(Vf::new(0.0, 0.0, 1.0, 0.0));
    let y = ground + plane.rest_height(bay_ground);
    plane.spawn = Spawn { at: Vf::new(spot.at.x(), y, spot.at.z(), 1.0), facing };
    plane.reset();
    Some(())
}

/// **The plane sent to another player** (P141, a marker's Up onto them, `spot` being
/// [`Garage::spot`]'s): where they are as their machine is that tick. Beside a flying plane
/// ([`Spot::flight`]), [`markers::BESIDE`] feet off its right wing, flying on as it flies
/// ([`Flight::fly_as`]: its speed, attitude and spin, this plane's own livery and throttle, out of
/// any wreck); over a rider who is moving or in the air, [`markers::CLEARANCE`] feet over their
/// bike, level along their heading and at their velocity, which stalls it ([`Flight::launch`]); and
/// by a rider at rest or down ([`Spot::at_rest`]) landed on the ground there at rest, as before
/// ([`land`]). The flight is placed at once, its spawn the plane's, and the drawn transform off it.
/// `None`, and nothing moves, where the landing finds no ground.
fn arrive(plane: &mut Plane, spot: Spot, world: &mut dyn World, acos: &game_sim::air::AcosTable) -> Option<()> {
    let at_rest = spot.at_rest();
    match (plane.flight.as_mut(), &spot.flight, &spot.motion) {
        (Some(f), Some(theirs), _) => f.fly_as(theirs, F(markers::BESIDE), acos),
        (Some(f), None, Some(m)) if !at_rest => {
            let facing = level(spot.forward).or_else(|| level(f.spawn.1)).unwrap_or(Vf::new(0.0, 0.0, 1.0, 0.0));
            let at = spot.at.add(Vf::new(0.0, markers::CLEARANCE, 0.0, 0.0));
            f.launch(at, facing, m.0.chassis.vel, acos);
        }
        _ => {
            land(plane, spot, world)?;
            if let Some(f) = plane.flight.as_mut() {
                f.reset(plane.spawn.at, plane.spawn.facing, acos);
            }
        }
    }
    if let Some(f) = &plane.flight {
        plane.spawn = Spawn { at: f.spawn.0, facing: f.spawn.1 };
    }
    plane.placed = true;
    plane.pose();
    Some(())
}

/// A plane's marker as the plane it kept (P107), to draw see-through where it was put down: in
/// `livery`, at the flight's pose.
pub fn marked_plane(flight: &Flight, livery: usize) -> Plane {
    Plane { livery, ..Plane::around(flight.clone()) }
}

/// **Where every seat's machines start on this map** (P132), for [`Sim::home`]: each bike stood at
/// its start — `starts`, in seat order, the origins the shell placed them at — along `heading` and
/// upright, which is the placement a ride begins with ([`game_sim::ride::Ride::place`]); and each
/// seat's plane in its bay, `bays` in seat order. Marker slot 1 rides there while nothing is put in
/// it. A disc's track, a mod map and a world all give their start through the place's reader, and
/// their bays through `machine.txt`'s.
pub fn home(starts: &[Vf], heading: Vf, bays: &[Spawn]) -> markers::Home {
    let up = Vf::new(0.0, 1.0, 0.0, 0.0);
    markers::Home {
        bikes: starts.iter().map(|&at| Spot::still(at, heading, up)).collect(),
        planes: bays.iter().map(|b| (b.at, b.facing)).collect(),
    }
}

/// The markers' word for a machine (P107): each has its own set.
pub fn markers_of(kind: Kind) -> markers::Vehicle {
    match kind {
        Kind::Bike => markers::Vehicle::Bike,
        Kind::Biplane => markers::Vehicle::Plane,
    }
}

/// Where the bike stands when it is put under `plane` (P102's switch, P104's marker onto a plane on
/// or near the ground since P141, [`Garage::spot`]): on the ground
/// under it, upright, along its heading flattened — the bike's own while the plane points straight
/// up or down — or `None` with no ground there.
fn under(plane: &Plane, sim: &Sim, world: &mut dyn World) -> Option<Spot> {
    let ground = ground_under(world, plane.at)?;
    let (forward, _) = sim.rider.ride.axes();
    let forward = level(plane.facing).or_else(|| level(forward)).unwrap_or(Vf::new(0.0, 0.0, 1.0, 0.0));
    let at = Vf::new(plane.at.x(), ground, plane.at.z(), 1.0);
    Some(Spot::still(at, forward, Vf::new(0.0, 1.0, 0.0, 0.0)))
}

/// The bike's ghost after a put-back (P57): the rider's own since S63, which moved it into the
/// simulation when it started to decide who passes through whom. The garage's part is only that a
/// bike the plane has the controller from is not updated, and so neither is its ghost.
pub use game_sim::ghost::Ghost;

impl Garage {
    /// A session that starts on `up`. Starting in a plane the ride does not have is an error rather
    /// than a quiet bike, so that `--vehicle biplane` cannot pass for a session it is not. Since P113
    /// every map has one ([`Lot::plane_anywhere`]) when the disc has the plane's `biplane.phy`.
    pub fn new(up: Kind, plane: Option<Plane>) -> Result<Garage, String> {
        if up == Kind::Biplane && plane.is_none() {
            return Err("--vehicle biplane: this ride has no biplane (the disc's machine.res has no biplane.phy)".into());
        }
        Ok(Garage { up, plane, taken: up == Kind::Biplane })
    }

    /// The machines this player can be put on: the bike, and the plane whenever the ride has one —
    /// on every map since P113, parked copy or not.
    pub fn machines(&self) -> Vec<Kind> {
        let mut out = vec![Kind::Bike];
        if self.plane.is_some() {
            out.push(Kind::Biplane);
        }
        out
    }

    /// **The switch** (P102): the controller moves to `to`, which is spawned where the player is now
    /// and on their heading, and the machine left goes. `sim` is the bike's ride and `world` the
    /// ground both stand on.
    ///
    /// * Onto the plane: it stands landed where the bike is — where the rider will be put back, if
    ///   they are down — nose along the bike's heading, level, on its wheels on the ground and at
    ///   rest ([`Plane::rest_height`]), unwrecked and its throttle shut, as a parked plane stands.
    /// * Onto the bike: it is stood on the ground under the plane, upright, along the plane's
    ///   heading and stopped, the rider up out of any wreck ([`rider::Rider::teleport_to`], which
    ///   ghosts it, so it passes through whoever is standing there).
    ///
    /// Switching to the machine already up does nothing. A ride with no plane (a disc without its
    /// `biplane.phy`), or no ground under the player, refuses with the reason and nothing moves. The machine now up, otherwise.
    pub fn switch(&mut self, to: Kind, sim: &mut Sim, world: &mut dyn World) -> Result<Kind, String> {
        if to == self.up {
            return Ok(to);
        }
        match to {
            Kind::Biplane => {
                let plane = self.plane.as_mut().ok_or("this ride has no biplane")?;
                land(plane, Spot::of(&sim.rider), world).ok_or("there is no ground under the bike to land the plane on")?;
                self.taken = true;
            }
            Kind::Bike => {
                // A plane switch only ever starts from a garage with a plane.
                let plane = self.plane.as_ref().ok_or("there is no plane to leave")?;
                let spot = under(plane, sim, world).ok_or("there is no ground under the plane to stand the bike on")?;
                sim.rider.teleport_to(spot);
            }
        }
        self.up = to;
        // Each machine its own markers (P107): the set shown is the one of the machine up.
        sim.markers.up(markers_of(to));
        Ok(to)
    }

    /// **One tick of the plane's markers** (P107), after the pass that flew it: [`markers::fly`] on
    /// the plane's own set, the pad the seat polled, and an Up onto another player put where they
    /// are (`spots[p]`, [`Garage::spot`]'s) by [`arrive`] — in the air beside a flying plane or over a
    /// moving rider (P141), landed by a rider at rest as the switch onto the plane lands it by the
    /// bike, refused with no ground there. A put-back's flight stands as it is: the plane's spawn is the
    /// flight's own again, and its drawn transform off it. Nothing with the bike up.
    pub fn markers(&mut self, sim: &mut Sim, me: usize, spots: &[Spot], riding: &[bool], world: &mut dyn World) -> Option<Did> {
        let (Kind::Biplane, Some(plane)) = (self.up, &mut self.plane) else { return None };
        let f = plane.flight.as_mut()?;
        let did = markers::fly(&mut sim.markers, f, sim.input.pad(0), me, spots, riding, &sim.home, &sim.tables.ride.acos);
        Some(match did? {
            Did::Teleported(Pick::Slot(_)) => {
                plane.placed = true;
                plane.spawn = Spawn { at: f.spawn.0, facing: f.spawn.1 };
                plane.pose();
                did?
            }
            // Placed on the tick it lands (P125), as `lane_markers` places it online: left to the
            // next step's reset, the plane was drawn and filmed at the flight it left for a tick, and
            // its camera snapped onto that rather than onto the plane standing by the player.
            Did::Teleported(Pick::Player(p)) => match arrive(plane, spots[p].clone(), world, &sim.tables.ride.acos) {
                Some(()) => did?,
                None => Did::Refused(Pick::Player(p)),
            },
            other => other,
        })
    }

    /// **One riding pass of the shell's** (`Seat::pass` past its poll, P125): [`Garage::step`] on
    /// whichever machine has the controller, then that machine's session markers — the bike's
    /// ([`markers::frame`]) or the plane's ([`Garage::markers`]) — for the player in seat `me`,
    /// `spots` being where every player was before the pass, every one of them in the ride. What the
    /// rider did, and what the d-pad did if it did anything.
    pub fn pass(&mut self, sim: &mut Sim, dt: F, controls: &Controls, me: usize, spots: &[Spot], world: &mut dyn World) -> (rider::Frame, Option<Did>) {
        self.pass_marked(sim, dt, controls, me, spots, world, true)
    }

    /// [`Garage::pass`] with the seat's Markers switch (P129): off, the pass is the machine's step
    /// alone and the d-pad does nothing of the markers'.
    #[allow(clippy::too_many_arguments)]
    pub fn pass_marked(
        &mut self,
        sim: &mut Sim,
        dt: F,
        controls: &Controls,
        me: usize,
        spots: &[Spot],
        world: &mut dyn World,
        markers: bool,
    ) -> (rider::Frame, Option<Did>) {
        let said = self.step(sim, dt, controls, world);
        if !markers {
            return (said, None);
        }
        let did = match self.up {
            Kind::Bike => markers::frame(sim, me, spots, controls),
            // The plane's own set (P107), on the d-pad the plane no longer reads.
            Kind::Biplane => self.markers(sim, me, spots, &[], world),
        };
        (said, did)
    }

    /// **Where the player is**, for a marker's Up onto them (P104): with the bike up, the bike's
    /// spot ([`Spot::of`], with its motion while they ride, their put-back spot while they are down).
    /// With the plane up and flying — unwrecked, off its wheels and more than [`markers::CLEARANCE`]
    /// over the ground — the plane as it is that tick (P141, [`Spot::of_flight`]): a bike sent there
    /// appears that far under it, upright along its flattened heading, and falls on at its velocity;
    /// a plane sent there flies on beside it. A plane on or near the ground, or wrecked, is where the
    /// switch back to the bike would stand it — on the ground under the plane, upright, along its
    /// flattened heading, stopped — and with no ground under it, the plane's own spot.
    pub fn spot(&self, sim: &Sim, world: &mut dyn World) -> Spot {
        match (self.up, &self.plane) {
            (Kind::Biplane, Some(plane)) => {
                if let Some(f) = plane.flight.as_ref().filter(|f| f.crash.reason == 0 && !f.measure.grounded) {
                    let high = ground_under(world, plane.at).is_none_or(|g| plane.at.y() - g > markers::CLEARANCE);
                    if high {
                        let (forward, _) = sim.rider.ride.axes();
                        let forward =
                            level(plane.facing).or_else(|| level(forward)).unwrap_or(Vf::new(0.0, 0.0, 1.0, 0.0));
                        return Spot::of_flight(f, forward);
                    }
                }
                under(plane, sim, world).unwrap_or(Spot::still(plane.at, plane.facing, plane.up))
            }
            _ => Spot::of(&sim.rider),
        }
    }

    /// The plane where it is drawn, if it is drawn at all: while the player flies it, and in its bay
    /// until they first take it. A plane they have left is drawn nowhere. The shell draws the bays'
    /// own planes instead since P103 ([`Garage::plane_flown`], [`bay_plane`]).
    #[allow(dead_code)] // the tests'
    pub fn plane_shown(&self) -> Option<&Plane> {
        self.plane.as_ref().filter(|_| self.up == Kind::Biplane || !self.taken)
    }

    /// The plane while the player flies it, and nowhere else: what is drawn solid once the bays
    /// show their own see-through machines (P103), the untaken plane's among them.
    pub fn plane_flown(&self) -> Option<&Plane> {
        self.plane.as_ref().filter(|_| self.up == Kind::Biplane)
    }

    /// Whether the bike is drawn: only while it is up, since a switch away leaves it nowhere.
    pub fn bike_shown(&self) -> bool {
        self.up == Kind::Bike
    }

    /// [`Plane::inverted`] for the plane with the controller; never, with the bike up.
    pub fn inverted(&self, was: bool) -> bool {
        match (self.up, &self.plane) {
            (Kind::Biplane, Some(plane)) => plane.inverted(was),
            _ => false,
        }
    }

    /// Whether the plane's elevator answers at every bank (P69): the seat's biplane scheme, which the
    /// shell sets before each pass ([`Plane::every_bank`]). A garage with no plane has nothing to set.
    pub fn set_every_bank(&mut self, on: bool) {
        if let Some(plane) = &mut self.plane {
            plane.every_bank = on;
        }
    }

    /// Whether the plane's stick is read round and its pitch loop kept (P89): the seat's biplane
    /// scheme, set beside [`Garage::set_every_bank`] ([`Plane::round_stick`]).
    pub fn set_round_stick(&mut self, on: bool) {
        if let Some(plane) = &mut self.plane {
            plane.round_stick = on;
        }
    }

    /// Whether the plane reads the d-pad (P129, [`Plane::dpad`]): the seat's Markers switch off.
    pub fn set_dpad(&mut self, on: bool) {
        if let Some(plane) = &mut self.plane {
            plane.dpad = on;
        }
    }

    /// One simulation pass for whichever machine has the controller. With the bike up this is
    /// exactly [`Sim::step`] and nothing else, which is what keeps a `--vehicle bike` session the
    /// shell it was before P19. With the plane up the bike is not stepped at all, the plane flies on
    /// the pad `sim` polled, and the frame the rider reports is empty.
    pub fn step(&mut self, sim: &mut Sim, dt: F, controls: &Controls, world: &mut dyn World) -> rider::Frame {
        match (self.up, &mut self.plane) {
            (Kind::Biplane, Some(plane)) => {
                let respawned = plane.step(sim, dt, world);
                rider::Frame { respawned, ..rider::Frame::default() }
            }
            // The bike's ghost ticks inside its update (`Rider::frame`), so a bike the plane has the
            // controller from does not count its ghost down either.
            _ => sim.step(dt, controls, world),
        }
    }
}

/// **The switch online** (N17): a seat's machine is a rollback input, and its plane is session state.
///
/// Online a seat's whole state is its [`Sim`], which the session saves, rolls back, hashes and sends:
/// so the plane a seat flies is the `Sim`'s own lane, [`Sim::plane`] — `None` on the bike — and not a
/// [`Garage`] beside it. What the player asks for travels in the seat's pad, one byte past the
/// original's ([`Wanted`]), and every machine switches the seat at the tick that byte first asks
/// for another machine than the one the seat is on ([`switch_lane`]), as P102's switch does offline.
/// The shell's garage only follows the lane for the picture ([`Garage::follow`]).
///
/// The byte says which machine the player wants, not that they pressed something: a pad ggrs has
/// to guess repeats the last one, and a guessed repeat of "the plane" is the plane already up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wanted {
    /// The machine the player last picked, `None` before they picked one (the seat stays on what
    /// it is on: an idle pad, an unplugged one, a peer's before its first).
    pub to: Option<Kind>,
    /// The seat's biplane scheme ([`Plane::every_bank`], [`Plane::round_stick`]): how its plane reads
    /// its stick is the seat's choice on its own machine, and a step on every machine must read it
    /// the same, so it rides in the same byte, a tick at a time.
    pub every_bank: bool,
    pub round_stick: bool,
    /// The seat's Markers switch is off (P129): its d-pad fires no marker and rides as the original's,
    /// on the bike and in the plane. A seat's own choice that every step must read the same, so it
    /// rides in the byte too; 0, an idle pad's, is markers on, as offline's default.
    pub dpad_rides: bool,
    /// The seat rides the new layout's reset chord (P67, `Controls::chord_together`): its two halves
    /// pressed together, so a clutch held on L1 and then R1 is a trick and not the put-back. A seat's
    /// own layout that every step must read the same, so it rides in the byte too; 0, an idle pad's,
    /// is the original's chord.
    pub together: bool,
    /// The seat's player has their pause open (P84): on every machine, from the tick the byte lands
    /// on, the seat is frozen where it is — not polled, not stepped, in no other bike's contacts —
    /// and drawn see-through, until a tick whose byte has it off. 0 rides, as an idle pad does.
    pub paused: bool,
}

impl Wanted {
    /// The byte: the machine in the low two bits (0 none, 1 the bike, 2 the biplane), then the
    /// two scheme bits, then the markers-off bit, then the together chord's, then the paused bit.
    pub fn byte(self) -> u8 {
        let to = match self.to {
            None => 0,
            Some(Kind::Bike) => 1,
            Some(Kind::Biplane) => 2,
        };
        to | (self.every_bank as u8) << 2 | (self.round_stick as u8) << 3 | (self.dpad_rides as u8) << 4 | (self.together as u8) << 5 | (self.paused as u8) << 6
    }

    /// [`Wanted::byte`] read back; a machine number this build does not know is no pick.
    pub fn of(byte: u8) -> Wanted {
        let to = match byte & 3 {
            1 => Some(Kind::Bike),
            2 => Some(Kind::Biplane),
            _ => None,
        };
        Wanted { to, every_bank: byte & 4 != 0, round_stick: byte & 8 != 0, dpad_rides: byte & 16 != 0, together: byte & 32 != 0, paused: byte & 64 != 0 }
    }
}

/// The machine a seat's [`Sim`] is on online: the plane while its lane holds one.
pub fn lane_up(sim: &Sim) -> Kind {
    if sim.plane.is_some() { Kind::Biplane } else { Kind::Bike }
}

/// [`Garage::switch`] for a seat whose plane is its [`Sim::plane`] (N17): onto the plane, a copy of
/// `template` (the ride's biplane as built, never flown) put down where the bike is, landed and at
/// rest, into the lane; onto the bike, the bike stood under the plane and the lane emptied. The
/// same spawn as offline, which is the same [`Garage::switch`]; the flight is placed at once rather
/// than by the next step, which is the same thing a tick earlier in the same tick. `Err`, and nothing
/// moves, where offline refuses too, or there is no template to take the plane from.
pub fn switch_lane(to: Kind, template: Option<&Flight>, sim: &mut Sim, world: &mut dyn World) -> Result<Kind, String> {
    let up = lane_up(sim);
    if to == up {
        return Ok(to);
    }
    let plane = match sim.plane.take() {
        Some(f) => Plane::around(f),
        None => {
            let f = template.ok_or("this ride has no biplane to take")?.clone();
            Plane::flying(Spawn { at: f.spawn.0, facing: f.spawn.1 }, f)
        }
    };
    let mut garage = Garage { up, plane: Some(plane), taken: true };
    let result = garage.switch(to, sim, world);
    let mut plane = garage.plane.take().expect("the switch keeps the plane");
    if garage.up == Kind::Biplane {
        let spawn = plane.spawn;
        if let Some(f) = &mut plane.flight {
            f.reset(spawn.at, spawn.facing, &sim.tables.ride.acos);
        }
        sim.plane = plane.flight.take();
    }
    result
}

/// **A seat that starts the ride in the plane online** (N18): a copy of `template` (the ride's
/// biplane as built, never flown) into its [`Sim::plane`], standing in `bay`, the seat's own bay in
/// the lot. Offline a player whose ride setup picked the plane starts in it in their bay (P41,
/// [`Lot::plane`]), placed there by its first step; here it is placed before the first tick, which
/// is the same flight before the same first frame ([`switch_lane`] places its plane at once too).
pub fn start_lane(template: &Flight, bay: Spawn, sim: &mut Sim) {
    let mut plane = Plane::flying(bay, template.clone());
    if let Some(f) = &mut plane.flight {
        f.reset(bay.at, bay.facing, &sim.tables.ride.acos);
    }
    sim.plane = plane.flight.take();
}

/// One tick of a seat's plane online (N17): [`Plane::step`] on the flight in its [`Sim::plane`], the
/// seat's pad and its biplane scheme, the bike not stepped. Whether the reset chord or a wreck's
/// recovery put the plane back this tick. A seat on the bike does nothing.
pub fn fly_lane(sim: &mut Sim, dt: F, wanted: Wanted, world: &mut dyn World) -> bool {
    let Some(f) = sim.plane.take() else { return false };
    let mut plane =
        Plane { every_bank: wanted.every_bank, round_stick: wanted.round_stick, dpad: wanted.dpad_rides, ..Plane::around(f) };
    let respawned = plane.step(sim, dt, world);
    sim.plane = plane.flight.take();
    respawned
}

/// **Where an online seat is**, for another player's marker Up onto them (P107): [`Garage::spot`]
/// for a seat whose plane is its [`Sim::plane`] — the plane as it flies (P141), or the ground under
/// it while it is on or near the ground, the bike's spot on the bike.
pub fn lane_spot(sim: &Sim, world: &mut dyn World) -> Spot {
    match &sim.plane {
        Some(f) => Garage { up: Kind::Biplane, plane: Some(Plane::around(f.clone())), taken: true }.spot(sim, world),
        None => Spot::of(&sim.rider),
    }
}

/// One tick of an online seat's plane markers (P107): [`Garage::markers`] on the flight in its
/// [`Sim::plane`]. An Up onto another player puts the plane where they are ([`arrive`]: beside their
/// plane, over a moving rider, landed by one at rest) and places its flight there at once, as
/// [`switch_lane`] does. Nothing on the bike.
pub fn lane_markers(sim: &mut Sim, me: usize, spots: &[Spot], riding: &[bool], world: &mut dyn World) -> Option<Did> {
    let mut f = sim.plane.take()?;
    let mut did = markers::fly(&mut sim.markers, &mut f, sim.input.pad(0), me, spots, riding, &sim.home, &sim.tables.ride.acos);
    if let Some(Did::Teleported(Pick::Player(p))) = did {
        let mut plane = Plane::around(f);
        if arrive(&mut plane, spots[p].clone(), world, &sim.tables.ride.acos).is_none() {
            did = Some(Did::Refused(Pick::Player(p)));
        }
        f = plane.flight.take().expect("the teleport keeps the flight");
    }
    sim.plane = Some(f);
    did
}

impl Garage {
    /// The shell's garage following a seat online (N17): the machine its [`Sim`]'s lane says is up,
    /// and the plane drawn where that flight is. The plane keeps this seat's livery and scheme. The
    /// machine now up when that changed (the shell snaps the camera onto it), `None` otherwise.
    pub fn follow(&mut self, sim: &Sim) -> Option<Kind> {
        let up = lane_up(sim);
        match (&sim.plane, &mut self.plane) {
            (Some(f), Some(plane)) => {
                plane.flight = Some(f.clone());
                plane.placed = true;
                plane.spawn = Spawn { at: f.spawn.0, facing: f.spawn.1 };
                plane.pose();
                self.taken = true;
            }
            // A seat this machine parked no plane for (a track with fewer bays than seats) is
            // still drawn in the one it flies, in the plain livery.
            (Some(f), None) => {
                self.plane = Some(Plane::around(f.clone()));
                self.taken = true;
            }
            (None, _) => {}
        }
        let was = std::mem::replace(&mut self.up, up);
        (was != up).then_some(up)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `machine.txt` in the disc's shape, with numbers of this file's own rather than a track's.
    const MACHINES: &str = "[Machines]\nNumberOfMachines = 1\n\n[Biplane1]\nType = Biplane\n\
        Position = 100, 20, -50\nDirection = 0, 0, -1\nHasPlayer = 1\n";

    #[test]
    fn the_biplane_is_parked_where_machine_txt_says() {
        let s = Spawn::biplane(MACHINES.as_bytes(), 0).unwrap();
        assert_eq!((s.at.x(), s.at.y(), s.at.z(), s.at.w()), (100.0, 20.0, -50.0, 1.0));
        assert_eq!((s.facing.x(), s.facing.y(), s.facing.z()), (0.0, 0.0, -1.0));
        assert_eq!(Spawn::biplane(MACHINES.as_bytes(), 1), None, "there is no second player's plane");
        assert_eq!(Spawn::biplane(b"[Machines]\n", 0), None);
    }

    #[test]
    fn a_switch_spawns_on_the_player_and_a_track_without_a_plane_refuses() {
        use game_sim::world::Flat;
        let bay = Spawn::biplane(MACHINES.as_bytes(), 0).unwrap();
        let mut sim = Sim::new(0);
        let mut floor = Flat::default();
        let mut g = Garage::new(Kind::Bike, Some(Plane::parked(bay))).unwrap();
        assert_eq!(g.plane_shown().map(|p| p.at), Some(bay.at), "an untaken plane stands in its bay");
        assert!(g.bike_shown());

        // Onto the plane: where the bike is, along its heading, as high over the floor as the bay
        // stands over its ground (a plane with no flight has no wheels to stand on).
        let bike = sim.rider.ride.position();
        assert_eq!(g.switch(Kind::Biplane, &mut sim, &mut floor), Ok(Kind::Biplane));
        let p = g.plane.as_ref().unwrap();
        assert_eq!((p.at.x(), p.at.y(), p.at.z()), (bike.x(), bay.at.y(), bike.z()));
        assert_eq!((p.facing.x(), p.facing.y(), p.facing.z()), (0.0, 0.0, 1.0));
        assert_eq!(p.bay, bay, "the bay stays the file's");
        assert!(!g.bike_shown());
        assert!(g.plane_shown().is_some());

        // Onto the bike: under the plane, on the floor, along its heading, stopped.
        g.plane.as_mut().unwrap().at = Vf::new(-30.0, 400.0, 70.0, 1.0);
        g.plane.as_mut().unwrap().facing = Vf::new(-1.0, 0.3, 0.0, 0.0);
        assert_eq!(g.switch(Kind::Bike, &mut sim, &mut floor), Ok(Kind::Bike));
        let at = sim.rider.ride.position();
        assert_eq!((at.x(), at.y(), at.z()), (-30.0, 0.0, 70.0));
        let (forward, up) = sim.rider.ride.axes();
        assert!((forward.x() + 1.0).abs() < 1e-5 && forward.y().abs() < 1e-5, "{forward:?}");
        assert!((up.y() - 1.0).abs() < 1e-5, "{up:?}");
        assert_eq!(sim.rider.ride.chassis.speed, F::ZERO);
        assert!(g.bike_shown());
        assert_eq!(g.plane_shown(), None, "a plane left is drawn nowhere");
        assert_eq!(g.switch(Kind::Bike, &mut sim, &mut floor), Ok(Kind::Bike), "already on it");

        assert!(Garage::new(Kind::Biplane, None).is_err());
        let mut g = Garage::new(Kind::Bike, None).unwrap();
        assert!(g.switch(Kind::Biplane, &mut sim, &mut floor).is_err());
        assert_eq!(g.up, Kind::Bike);
        assert_eq!(g.machines(), vec![Kind::Bike]);
    }

    #[test]
    fn a_flying_players_spot_is_under_their_plane_not_at_the_bike_they_left() {
        use game_sim::world::Flat;
        let bay = Spawn::biplane(MACHINES.as_bytes(), 0).unwrap();
        let mut sim = Sim::new(0);
        let mut floor = Flat::default();
        let mut g = Garage::new(Kind::Bike, Some(Plane::parked(bay))).unwrap();
        assert_eq!(g.spot(&sim, &mut floor), Spot::of(&sim.rider), "on the bike, the bike's spot");

        let bike = sim.rider.ride.position();
        g.switch(Kind::Biplane, &mut sim, &mut floor).unwrap();
        let plane = g.plane.as_mut().unwrap();
        plane.at = Vf::new(-30.0, 400.0, 70.0, 1.0);
        plane.facing = Vf::new(0.0, 0.6, -0.8, 0.0);
        let spot = g.spot(&sim, &mut floor);
        assert_eq!((spot.at.x(), spot.at.y(), spot.at.z()), (-30.0, 0.0, 70.0), "on the floor under the plane");
        assert!(spot.forward.z() + 1.0 < 1e-5 && spot.forward.y() == 0.0, "{:?}", spot.forward);
        assert_eq!((spot.up.x(), spot.up.y(), spot.up.z()), (0.0, 1.0, 0.0));
        assert_ne!(spot.at.x(), bike.x(), "not where the bike was left");

        // A rider ridden there by a marker's Up lands where the switch back would stand the bike, at
        // rest: a plane with no flight in it carries no motion to take (P111). A flown one in the air
        // is the plane as it flies (P141, `tests/teleport_air.rs`).
        assert_eq!(spot.motion, None, "a flying player's spot carries no bike motion");
        let mut other = Sim::new(0);
        other.rider.ride.chassis.vel = Vf::new(0.0, 0.0, 9.0, 0.0);
        other.rider.teleport_to(spot);
        assert_eq!(other.rider.ride.chassis.vel.to_bits(), Vf::ZERO.to_bits(), "landed at rest");
        g.switch(Kind::Bike, &mut sim, &mut floor).unwrap();
        assert_eq!(other.rider.ride.position(), sim.rider.ride.position());
    }

    /// Two of each, as the disc's files have them, with the heights of this file's own.
    const TWO_OF_EACH: &str = "[Machines]\nNumberOfMachines = 1\n\n\
        [Machine1]\nType = Helicopter\nPosition = 1, 2, 3\nDirection = 0, 0, 1\n\
        [Buggy1]\nPosition = 0, 0, 0\nDirection = 0, 0, 1\n[Buggy2]\nPosition = 20, 0, 0\nDirection = 0, 0, 1\n\
        [Biplane1]\nPosition = 100, 20, -50\nDirection = 0, 0, -1\n\
        [Biplane2]\nPosition = 80, 20, -25\nDirection = -1, 0, 0\n\
        [Helicopter1]\nPosition = 300, 5, 0\nDirection = 1, 0, 0\n[Helicopter2]\nPosition = 300, 5, 60\nDirection = 1, 0, 0\n\
        [Monster1]\nPosition = -100, 0, 0\nDirection = 0, 0, 1\n[Monster2]\nPosition = -80, 0, 0\nDirection = 0, 0, 1\n\
        [Baja1]\nPosition = 0, 0, 200\nDirection = 0, 0, 1\n[Baja2]\nPosition = 20, 0, 200\nDirection = 0, 0, 1\n";

    #[test]
    fn the_lot_parks_one_of_each_a_player() {
        // A ground that slopes up along x, so a copy stepped sideways is lifted with it.
        let mut ground = |at: Vf| at.x() * 0.1;
        for players in [1, 2, 3, 4, 8, 16, 33] {
            let lot = Lot::new(TWO_OF_EACH.as_bytes(), players, &mut ground);
            for m in Machine::ALL {
                let bays: Vec<&Bay> = lot.of(m).collect();
                assert_eq!(bays.len(), players, "{players} players, {m:?}");
                let file = spots(TWO_OF_EACH.as_bytes(), m);
                for (i, b) in bays.iter().enumerate() {
                    assert_eq!((b.copy, b.livery, b.original), (i, i % 2, i < 2));
                    assert_eq!(lot.bay(m, i), Some(*b));
                    if i < 2 {
                        assert_eq!(b.spawn, file[i], "the file's own spot, exactly");
                    } else {
                        let base = file[i % 2];
                        let clear = base.at.y() - base.at.x() * 0.1;
                        assert!((b.spawn.at.y() - (b.spawn.at.x() * 0.1 + clear)).abs() < 1e-3, "{m:?} {i} keeps its clearance");
                        assert_eq!(b.spawn.facing, base.facing);
                    }
                    for c in &bays[..i] {
                        let apart = (b.spawn.at.x() - c.spawn.at.x()).hypot(b.spawn.at.z() - c.spawn.at.z());
                        assert!(c.original && b.original || apart >= m.spacing() - 0.01, "{m:?} {i} and {} are {apart} ft apart", c.copy);
                    }
                }
            }
            // A ride that grows keeps every machine where it was.
            let more = Lot::new(TWO_OF_EACH.as_bytes(), players + 3, &mut ground);
            for b in &lot.bays {
                assert_eq!(more.bay(b.machine, b.copy), Some(b));
            }
        }
        // A file with only the first player's plane parks the rest beside it, and one with none, none.
        let lot = Lot::new(MACHINES.as_bytes(), 4, &mut ground);
        assert_eq!(lot.of(Machine::Biplane).count(), 4);
        assert_eq!(lot.of(Machine::Helicopter).count(), 0);
        let mut lot = Lot::new(b"[Machines]\n", 4, &mut ground);
        assert!(lot.bays.is_empty());
        lot.resize(0, &mut ground);
        assert!(lot.plane(0, None).is_none());
    }

    #[test]
    fn each_player_takes_their_own_plane() {
        let mut ground = |_: Vf| 0.0;
        let lot = Lot::new(TWO_OF_EACH.as_bytes(), 8, &mut ground);
        let mut garages: Vec<Garage> = (0..8).map(|p| Garage::new(Kind::Bike, lot.plane(p, None)).unwrap()).collect();
        let planes: Vec<&Plane> = garages.iter().map(|g| g.plane_shown().unwrap()).collect();
        for (i, p) in planes.iter().enumerate() {
            assert_eq!(p.at, lot.bay(Machine::Biplane, i).unwrap().spawn.at, "player {i}'s plane is in their own bay");
            assert_eq!(p.livery, i % 2, "every second plane wears biplane2");
            for q in &planes[..i] {
                assert_ne!(p.at, q.at, "two players in one plane");
            }
        }
        // Taking it leaves every other player's in its bay, and keeps the livery.
        let (mut sim, mut floor) = (Sim::new(0), game_sim::world::Flat::default());
        garages[1].switch(Kind::Biplane, &mut sim, &mut floor).unwrap();
        garages[1].switch(Kind::Bike, &mut sim, &mut floor).unwrap();
        garages[1].switch(Kind::Biplane, &mut sim, &mut floor).unwrap();
        assert_eq!(garages[1].plane.as_ref().unwrap().livery, 1);
        for (i, g) in garages.iter().enumerate().filter(|(i, _)| *i != 1) {
            assert_eq!(g.plane_shown().unwrap().at, lot.bay(Machine::Biplane, i).unwrap().spawn.at);
        }
        assert_eq!(Machine::Biplane.models()[livery(1)], "biplane2");
    }

    #[test]
    fn the_words() {
        assert_eq!(Kind::parse("bike"), Ok(Kind::Bike));
        assert_eq!(Kind::parse("Biplane"), Ok(Kind::Biplane));
        assert!(Kind::parse("helicopter").is_err());
    }
}
