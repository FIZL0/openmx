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
use game_sim::biplane_anim;
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
    /// The cars (S149e): the dune buggy, the monster truck and the baja truck, kinds 3, 4 and 5
    /// ([`CarRide`]).
    Buggy,
    Monster,
    Baja,
    /// The Kiowa helicopter (P216), the original's machine kind 2 ([`KiowaRide`]).
    Kiowa,
}

impl Kind {
    /// `--vehicle`'s word for it.
    pub fn parse(word: &str) -> Result<Kind, String> {
        match word.to_ascii_lowercase().as_str() {
            "bike" => Ok(Kind::Bike),
            "biplane" | "plane" => Ok(Kind::Biplane),
            "buggy" => Ok(Kind::Buggy),
            "monster" | "truck" | "monstertruck" => Ok(Kind::Monster),
            "baja" => Ok(Kind::Baja),
            "helicopter" | "kiowa" | "heli" => Ok(Kind::Kiowa),
            _ => Err(format!("--vehicle: {word:?} is not a vehicle; it is bike, biplane, helicopter, buggy, monster or baja")),
        }
    }

    /// What the shell and its overlay call it.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Bike => "bike",
            Kind::Biplane => "biplane",
            Kind::Buggy => "buggy",
            Kind::Monster => "monster",
            Kind::Baja => "baja",
            Kind::Kiowa => "helicopter",
        }
    }

    /// The machine a swap goes to from this one: the bike and the plane each other's, a car or the
    /// helicopter the bike.
    pub fn other(self) -> Kind {
        match self {
            Kind::Bike => Kind::Biplane,
            Kind::Biplane | Kind::Buggy | Kind::Monster | Kind::Baja | Kind::Kiowa => Kind::Bike,
        }
    }

    /// The car it is, for a car.
    pub fn car(self) -> Option<Machine> {
        match self {
            Kind::Buggy => Some(Machine::Buggy),
            Kind::Monster => Some(Machine::Monster),
            Kind::Baja => Some(Machine::Baja),
            Kind::Bike | Kind::Biplane | Kind::Kiowa => None,
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
    /// Its `SetGroundHeight` is [`ground_heights`]'s, which the plane's first step reads ([`Plane::step`]).
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
/// original's two. Every one is ported ([`Machine::kind`]); the port parks each car only on its own
/// track ([`Machine::home`], S245).
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

    /// The shell's machine for it, where the port has one: what the bays draw and offer. Every
    /// machine since S245, the cars too.
    pub fn kind(self) -> Option<Kind> {
        match self {
            Machine::Biplane => Some(Kind::Biplane),
            Machine::Helicopter => Some(Kind::Kiowa),
            Machine::Buggy | Machine::Monster | Machine::Baja => self.car_kind(),
        }
    }

    /// The one track the port parks a car on (S245): the monster truck on free09, the baja on
    /// free07, the buggy on free11, the tracks whose RAM images hold that car parked (S149a's
    /// captures). Every freeride track's `machine.txt` parks two of each car, but the port stands a
    /// car only on its own, so each freeride has one car to ride up to. `None` for the plane and the
    /// helicopter, which stand wherever the file parks them.
    pub fn home(self) -> Option<&'static str> {
        match self {
            Machine::Monster => Some("free09"),
            Machine::Baja => Some("free07"),
            Machine::Buggy => Some("free11"),
            Machine::Biplane | Machine::Helicopter => None,
        }
    }

    /// Whether the port parks this machine by a `machine.txt` named `file` (a member name,
    /// `tracks\free09\machine.txt`): the plane and the helicopter by any, a car by its own track's
    /// alone ([`Machine::home`]).
    pub fn parks_by(self, file: &str) -> bool {
        match self.home() {
            None => true,
            Some(track) => file.to_ascii_lowercase().replace('/', "\\").split('\\').any(|part| part == track),
        }
    }

    /// The shell's machine for a car (S149e).
    pub fn car_kind(self) -> Option<Kind> {
        match self {
            Machine::Buggy => Some(Kind::Buggy),
            Machine::Monster => Some(Kind::Monster),
            Machine::Baja => Some(Kind::Baja),
            Machine::Biplane | Machine::Helicopter => None,
        }
    }

    /// The car's own physics in `game_sim`, for a car.
    pub fn car_physics(self) -> Option<game_sim::car::Kind> {
        match self {
            Machine::Buggy => Some(game_sim::car::Kind::Buggy),
            Machine::Monster => Some(game_sim::car::Kind::MonsterTruck),
            Machine::Baja => Some(game_sim::car::Kind::Baja),
            Machine::Biplane | Machine::Helicopter => None,
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

/// Whether the original's placement puts each of `machine`'s spots on a track onto the ground, in
/// [`spots`]' order: the section's `SetGroundHeight`, on unless the file says otherwise (S205). On,
/// the spot's height is the ground's under it; off, the file's height is kept, and only the ground's
/// normal there is taken. Every freeride track's `machine.txt` leaves it out.
pub fn ground_heights(machine_txt: &[u8], machine: Machine) -> Vec<bool> {
    let ini = Ini::parse_bytes(machine_txt);
    (0..spots(machine_txt, machine).len())
        .map(|i| ini.section(&format!("{}{}", machine.section(), i + 1)).flag("SetGroundHeight", true))
        .collect()
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
#[allow(dead_code)] // the rule the docs name; the lot parks through `Lot::around` since MD24
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

/// A home tile's `machine.txt` spots in a world (MD24), one list a machine in [`Machine::ALL`]'s
/// order. The file's coordinates are its own track's, and a world neither starts where that track
/// does nor puts the tile at the origin, so its spots are moved as one: by the step that stands
/// `[Biplane1]` one [`Machine::spacing`] to the right of the world's spawn `start` (across its
/// heading flattened, as [`park`] steps a copy), every other spot keeping its place beside it. Each
/// keeps the clearance it had over its own ground, which in the world is the home tile's ground at
/// the spot moved by the tile's offset `home` (`x`, `z`); `ground` is the world's. A home tile whose
/// ground does not reach the file's place (a mod tile smaller than the track the file is for, MD55)
/// has no clearance to keep there, and the spot stands `rest` feet over the world's ground instead
/// ([`Plane::rest_height`]: on its wheels). A file with no plane is moved by the first spot it does
/// have. The port's own placement: the original has no worlds.
pub fn world_spots(machine_txt: &[u8], home: [f32; 2], start: Spawn, rest: f32, ground: &mut dyn FnMut(Vf) -> f32) -> Vec<Vec<Spawn>> {
    let file: Vec<Vec<Spawn>> = Machine::ALL.iter().map(|&m| spots(machine_txt, m)).collect();
    let Some(anchor) = file.iter().find_map(|s| s.first().copied()) else {
        return file;
    };
    let facing = level(start.facing).unwrap_or(Vf::new(0.0, 0.0, 1.0, 0.0));
    let gap = Machine::Biplane.spacing();
    let (x, z) = (start.at.x() - facing.z() * gap, start.at.z() + facing.x() * gap);
    let (dx, dz) = (x - anchor.at.x(), z - anchor.at.z());
    file.into_iter()
        .map(|s| {
            s.into_iter()
                .map(|spot| {
                    let under = ground(Vf::new(spot.at.x() + home[0], spot.at.y(), spot.at.z() + home[1], 1.0));
                    let clear = if under > NO_GROUND { spot.at.y() - under } else { rest };
                    let (x, z) = (spot.at.x() + dx, spot.at.z() + dz);
                    let y = ground(Vf::new(x, spot.at.y(), z, 1.0)) + clear;
                    Spawn { at: Vf::new(x, y, z, 1.0), facing: spot.facing }
                })
                .collect()
        })
        .collect()
}

/// `file`'s spots, one list a machine in [`Machine::ALL`]'s order ([`spots`], [`world_spots`]), with
/// those of a machine the port does not park by the `machine.txt` named `name` emptied
/// ([`Machine::parks_by`]): a car's away from its own track (S245).
pub fn parked_by(name: &str, file: Vec<Vec<Spawn>>) -> Vec<Vec<Spawn>> {
    file.into_iter().zip(Machine::ALL).map(|(s, m)| if m.parks_by(name) { s } else { Vec::new() }).collect()
}

/// Every machine a ride parks, one of each per player, sized to the player count as players join and
/// leave.
#[derive(Clone, Debug, Default)]
pub struct Lot {
    /// The spots each machine of [`Machine::ALL`] is parked around, in that order: the file's own
    /// ([`spots`]), or the same moved into a world ([`world_spots`]).
    file: Vec<Vec<Spawn>>,
    pub bays: Vec<Bay>,
    /// How far over the ground the original stands its plane's body when it spawns it on a file
    /// spot ([`flight::spawn_lift`] off the disc's model, S164), which every plane [`Lot::plane`]
    /// stands in such a bay carries ([`Plane::lift`]); `None` without the model.
    pub lift: Option<F>,
    /// The file's planes' `SetGroundHeight`s, in copy order ([`ground_heights`]); a copy past them is
    /// put on the ground.
    pub ground_heights: Vec<bool>,
    /// Every car the disc has in each of its liveries, built once off `machine.res` ([`car_kit`]),
    /// which each player's garage takes its cars from ([`Lot::cars_for`]). Empty without the files.
    pub cars: Vec<CarRide>,
    /// The helicopter as the disc builds it (P216), off `machine.res`'s `kiowa.phy`, which each
    /// player's garage takes a copy of ([`Lot::heli_for`]). `None` without the file.
    pub kiowa: Option<KiowaRide>,
}

impl Lot {
    /// A track's lot for `players`: [`park`] for every machine in [`Machine::ALL`].
    pub fn new(machine_txt: &[u8], players: usize, ground: &mut dyn FnMut(Vf) -> f32) -> Lot {
        let mut lot = Lot::around(Machine::ALL.iter().map(|&m| spots(machine_txt, m)).collect(), players, ground);
        lot.ground_heights = ground_heights(machine_txt, Machine::Biplane);
        lot
    }

    /// [`Lot::new`] over spots already found, one list a machine in [`Machine::ALL`]'s order: a
    /// world's ([`world_spots`]).
    pub fn around(file: Vec<Vec<Spawn>>, players: usize, ground: &mut dyn FnMut(Vf) -> f32) -> Lot {
        let mut lot = Lot { file, bays: Vec::new(), lift: None, ground_heights: Vec::new(), cars: Vec::new(), kiowa: None };
        lot.resize(players, ground);
        lot
    }

    /// Re-park for a new player count: a player joining adds one of each machine, beside the rest,
    /// and the last one leaving takes theirs away. Every copy that stays keeps its place.
    pub fn resize(&mut self, players: usize, ground: &mut dyn FnMut(Vf) -> f32) {
        let file = &self.file;
        self.bays = Machine::ALL
            .iter()
            .enumerate()
            .flat_map(|(i, &m)| park_around(file.get(i).map_or(&[][..], |s| &s[..]), m, players, ground))
            .collect();
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

    /// Where the original stands the body of a plane in `bay` (B6), as [`Plane::ready`]'s first
    /// placement has it: on a file bay with the model's [`Lot::lift`], the body over the ground
    /// under the spot (or at the file's height, without `SetGroundHeight`) and its up the ground's
    /// normal there, `ground` answering a query from [`PROBE`] over the spot with the point and the
    /// normal. `None` for a spot the port found, without the model, or with no ground under it, where
    /// the model's origin stands on the spot.
    pub fn body_spot(&self, bay: &Bay, ground: &mut dyn FnMut(Vf) -> (Vf, Vf)) -> Option<(Vf, Vf)> {
        let lift = self.lift.filter(|_| bay.original)?;
        let at = bay.spawn.at;
        let (on, normal) = ground(Vf::new(at.x(), at.y() + PROBE, at.z(), 1.0));
        let y = if self.ground_heights.get(bay.copy).copied().unwrap_or(true) { on.y() } else { at.y() };
        (on.y() > NO_GROUND).then(|| (flight::bay_spot(Vf::new(at.x(), y, at.z(), 1.0), lift), normal))
    }

    /// Player `player`'s plane, standing in their bay: parked, or flying when `flight` is given.
    pub fn plane(&self, player: usize, flight: Option<Flight>) -> Option<Plane> {
        let bay = self.bay(Machine::Biplane, player)?;
        let mut plane = match flight {
            Some(f) => Plane::flying(bay.spawn, f),
            None => Plane::parked(bay.spawn),
        };
        plane.livery = bay.livery;
        plane.lift = self.lift.filter(|_| bay.original);
        plane.ground_height = self.ground_heights.get(bay.copy).copied().unwrap_or(true);
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
    /// Its bay is one of `machine.txt`'s own spots, and this is the lift the original spawns its body
    /// at over the ground there ([`Lot::lift`], S164): while it is put back at the bay its flight
    /// stands where the original's does ([`flight::bay_spot`]), not with the model's origin on the
    /// spot.
    pub lift: Option<F>,
    /// Its bay's `SetGroundHeight` ([`ground_heights`]): with a [`Plane::lift`], whether the bay's
    /// spot is moved onto the ground or keeps the file's height (S205).
    pub ground_height: bool,
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
            lift: None,
            ground_height: true,
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
        let (flight, every_bank, round_stick, dpad, livery, bay, lift) =
            (self.flight.take(), self.every_bank, self.round_stick, self.dpad, self.livery, self.bay, self.lift);
        *self = Plane { flight, every_bank, round_stick, dpad, livery, bay, lift, ..Plane::parked(self.spawn) };
    }

    /// How high its origin stands over the ground under it when it is parked, feet. With its flight,
    /// the lowest of its tires' bottoms is on the ground (the tires' centres are in the origin's
    /// frame); without one, the clearance its bay has over the ground there, `bay_ground` being the
    /// ground's height under the bay.
    pub fn rest_height(&self, bay_ground: f32) -> f32 {
        match &self.flight {
            Some(f) => wheels_height(f),
            None => self.bay.at.y() - bay_ground,
        }
    }

    /// One frame of the plane with the controller on it: the reset chord ([`Flight::reset_chord`],
    /// S57) and [`Flight::frame`] on the seat's pad, over `world`, and the drawn transform off its
    /// body. A plane with no flight stands where it is. Whether the chord or a wreck's recovery put it
    /// back at its spawn this frame: both are the original's plane reset, which snaps its camera (V29).
    pub fn step(&mut self, sim: &Sim, dt: F, world: &mut dyn World) -> bool {
        self.step_among(sim, dt, world, &[])
    }

    /// [`Plane::step`] with the other players' `bikes` in the plane's world (S225,
    /// [`Flight::frame_among`]): it strikes them as it strikes any vehicle. What their struck
    /// listeners keep goes no further, as in [`Sim::step`]: a plane's probe never wrecks a bike
    /// ([`game_sim::flight_search::struck_wreck`]).
    pub fn step_among(&mut self, sim: &Sim, dt: F, world: &mut dyn World, bikes: &[game_sim::flight_search::Bike]) -> bool {
        self.step_met(sim, dt, world, bikes, None)
    }

    /// [`Plane::step_among`] with the riders' bikes that struck its hull this tick (N22a,
    /// [`Flight::frame_met`]): their mirrors on its list, resolved against their chassis. A `met`
    /// that names the bikes ([`flight::Met::bikes`], N22b) has the plane's own strikes on them push
    /// their chassis too, and leaves the mirrors for their lists in [`flight::Met::struck`]; the
    /// struck bike's wreck is still only [`game_sim::flight_search::struck_wreck`]'s.
    pub fn step_met(
        &mut self,
        sim: &Sim,
        dt: F,
        world: &mut dyn World,
        bikes: &[game_sim::flight_search::Bike],
        met: Option<&mut flight::Met>,
    ) -> bool {
        let Some(ready) = self.ready(sim, world) else { return false };
        let f = self.flight.as_mut().expect("a readied plane has its flight");
        let _struck = f.frame_met(dt, ready.pad, world, &sim.tables.ride, bikes, met);
        self.settle(ready)
    }

    /// **The track's traffic and the plane, among the knockable props** (S241): the traffic's
    /// held frame ([`game_sim::traffic::Traffic::frame_held`]), the plane's ([`Plane::step_met`],
    /// its frame [`Flight::frame_props`]: its rays strike the props, theirs its hull), then the
    /// props' step with the plane as their striker 0, and the traffic's late frame. With no props
    /// on the track, [`game_sim::traffic::Traffic::frame`] then [`Plane::step_met`], as before.
    pub fn step_traffic(
        &mut self,
        sim: &mut Sim,
        dt: F,
        world: &mut dyn World,
        bikes: &[game_sim::flight_search::Bike],
        met: Option<&mut flight::Met>,
    ) -> bool {
        if sim.traffic.props.is_empty() || self.flight.is_none() {
            sim.traffic.frame(dt, world);
            return self.step_met(sim, dt, world, bikes, met);
        }
        let movers = sim.traffic.frame_held(dt, world);
        let mut props = std::mem::take(&mut sim.traffic.props);
        let respawned = match self.ready(sim, world) {
            Some(ready) => {
                let f = self.flight.as_mut().expect("a readied plane has its flight");
                let _struck = f.frame_props(dt, ready.pad, world, &sim.tables.ride, bikes, met, &mut props, 0);
                self.settle(ready)
            }
            None => false,
        };
        if let Some(f) = &mut self.flight {
            let mut weighed = game_sim::contact::Weighed { body: &mut f.body, mass: f.mass };
            let mut strikers =
                [game_sim::contact::Other { collider: &mut f.collider, side: game_sim::contact::Side::Body(&mut weighed) }];
            props.step_with(dt, &movers, &mut strikers);
        }
        sim.traffic.props = props;
        sim.traffic.frame_late(dt, world);
        self.pose();
        respawned
    }

    /// [`Plane::step_met`] up to the frame (N22a1): the plane placed, the chord read, the seat's
    /// scheme put on the flight for the frame and the pad it flies it with. What
    /// [`Plane::settle`] needs after the frame, `None` with no flight.
    pub fn ready(&mut self, sim: &Sim, world: &mut dyn World) -> Option<Readied> {
        let f = self.flight.as_mut()?;
        if !self.placed {
            // At its file bay the original's spot: the body on the ground under the spot (or at the
            // file's height, without `SetGroundHeight`), lifted by the model's bounds (S164), its
            // up the ground's normal there (S205). Anywhere else, and with no ground under the
            // bay, the model's origin on the spot.
            let at = self.spawn.at;
            let ground = self.lift.filter(|_| self.spawn == self.bay).and_then(|lift| {
                let (on, normal) = world.ground(Vf::new(at.x(), at.y() + PROBE, at.z(), 1.0));
                let y = if self.ground_height { on.y() } else { at.y() };
                (on.y() > NO_GROUND).then(|| (flight::bay_spot(Vf::new(at.x(), y, at.z(), 1.0), lift), normal))
            });
            match ground {
                Some((body, up)) => f.reset_body(body, self.spawn.facing, up, &sim.tables.ride.acos),
                None => f.reset(at, self.spawn.facing, &sim.tables.ride.acos),
            }
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
        Some(Readied { chord, wrecked, min_elevator, dead_zone, feed_forward, max_pitch_rate, pad })
    }

    /// [`Plane::step_met`] after the frame (N22a1): the flight's own scheme put back, the plane
    /// posed, and whether it was put back on its spawn (the chord, or a wreck recovered).
    pub fn settle(&mut self, ready: Readied) -> bool {
        let Some(f) = self.flight.as_mut() else { return false };
        let Readied { chord, wrecked, min_elevator, dead_zone, feed_forward, max_pitch_rate, .. } = ready;
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
        let (at, facing) = (flight.spawn_origin(), flight.spawn.1);
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

    /// Its moving parts as the original's surface step poses them (S35, V38): the ailerons on the
    /// wing flaps, the elevator on the rear flaps, the rudder, and the prop — off the plane's own
    /// control state and engine, which the stick and L1/R1 reach through the flight's controls. A
    /// parked plane's are all at rest, its prop at zero; a wrecked one's prop is not turned and wears
    /// the bent cone, the same test [`Plane::prop_angle`] makes.
    pub fn surfaces(&self) -> biplane_anim::Surfaces {
        match &self.flight {
            None => biplane_anim::surfaces(F::ZERO, F::ZERO, F::ZERO, F::ZERO, F::ZERO, false),
            Some(f) => biplane_anim::surfaces(
                f.surfaces.elevator,
                f.joystick.max_elevator,
                f.surfaces.aileron,
                f.surfaces.rudder,
                f.engine.prop_angle,
                f.crash.reason != 0,
            ),
        }
    }
}

/// **The pilot's lean** (V78): what the original's plane asks its pilot's body for every tick and
/// how the body eases it, kept per seat by the drawing side, which has no body step of its own for
/// the pilot ([`biplane_anim::Lean`]).
///
/// The plane's measure step runs before its controls, so the pilot leans to the pad the *previous*
/// tick's controls left, a tick behind the surfaces; a plane reset — the chord, a wreck's recovery,
/// and the swap onto the plane — raises the plane's reset flag, which snaps both axes onto the stick
/// for the two measures that see it ([`biplane_anim::Reset`]). Presentation only: it reads the flight
/// and writes nothing back.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PilotLean {
    pub lean: biplane_anim::Lean,
    pub reset: biplane_anim::Reset,
    /// The flight's pad as the last tick's controls left it, elevator and aileron; `None` while no
    /// plane is flown.
    pub stick: Option<(F, F)>,
}

impl PilotLean {
    /// One tick, after the plane's step: `plane` the seat's flown plane ([`Garage::plane_flown`]), or
    /// `None` on the bike, which lets go of the lean so the next swap onto the plane snaps it again;
    /// `put_back` whether this tick's step reset the plane.
    pub fn step(&mut self, dt: F, plane: Option<&Plane>, put_back: bool) {
        let Some(plane) = plane else {
            *self = PilotLean::default();
            return;
        };
        if self.stick.is_none() || put_back {
            self.reset.reset();
        }
        // The controls zero the pad while the plane is not flown, which is the pad a swap's first
        // measure reads.
        let (elevator, aileron) = self.stick.unwrap_or((F::ZERO, F::ZERO));
        let pilot = biplane_anim::Pilot { elevator, aileron, held: self.reset.just, ..Default::default() };
        self.lean.step(dt, &pilot);
        self.reset.frame();
        self.stick = Some(plane.flight.as_ref().map_or((F::ZERO, F::ZERO), |f| (f.pad.elevator, f.pad.aileron)));
    }

    /// [`PilotLean::step`] for a seat flying the helicopter (P216b), whose pilot is posed by the
    /// plane's two clips too (the original loads them for both kinds): its stick's forward and back
    /// stand in for the elevator and its left and right for the aileron, both signed as the plane's
    /// are. Which of the helicopter's numbers the original's pilot reads was not traced.
    pub fn step_heli(&mut self, dt: F, heli: &KiowaRide, put_back: bool) {
        let Some(h) = &heli.flown else {
            *self = PilotLean::default();
            return;
        };
        if self.stick.is_none() || put_back {
            self.reset.reset();
        }
        let (elevator, aileron) = self.stick.unwrap_or((F::ZERO, F::ZERO));
        let pilot = biplane_anim::Pilot { elevator, aileron, held: self.reset.just, ..Default::default() };
        self.lean.step(dt, &pilot);
        self.reset.frame();
        self.stick = Some((h.kiowa.pad.longitudinal, h.kiowa.pad.lateral));
    }

    /// The two weights the pilot's clips are drawn at, `(phase, mix)` ([`game_render::biplane::seat`]).
    pub fn weights(&self) -> (f32, f32) {
        let (phase, mix) = self.lean.weights();
        (phase.0, mix.0)
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
    /// This player's cars (S149e), one of each the disc has ([`CarRide`]).
    pub cars: Vec<CarRide>,
    /// This player's helicopter (P216), where the disc has its `kiowa.phy` ([`KiowaRide`]).
    pub heli: Option<KiowaRide>,
}

/// How far above a machine's origin the ground under it is looked for when the other is spawned
/// there, feet: enough for a bike's origin sitting below a ridge it leans over, and not so much that
/// a machine under a bridge comes out on top of it. The port's own number.
const PROBE: f32 = 20.0;

/// Below this the ground query found nothing ([`game_sim::world::World::ground`]'s no-ground answer is
/// a million feet down).
const NO_GROUND: f32 = -1.0e5;

/// How high a plane's origin stands over the ground on its wheels, feet: the lowest of its tires'
/// bottoms is on the ground (the tires' centres are in the origin's frame). [`Plane::rest_height`]
/// with its flight.
pub fn wheels_height(flight: &Flight) -> f32 {
    -flight.tires.iter().map(|t| t.center.y() - t.radius.0).fold(f32::MAX, f32::min)
}

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
        plane.spawn = Spawn { at: f.spawn_origin(), facing: f.spawn.1 };
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
        // A car keeps the bike's set; its pass runs no markers (S149e).
        Kind::Buggy | Kind::Monster | Kind::Baja => markers::Vehicle::Bike,
        // So does the helicopter (P216).
        Kind::Kiowa => markers::Vehicle::Bike,
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
        if up.car().is_some() || up == Kind::Kiowa {
            return Err(format!("--vehicle {}: a ride starts on the bike or the plane; switch to it from the pause", up.name()));
        }
        Ok(Garage { up, plane, taken: up == Kind::Biplane, cars: Vec::new(), heli: None })
    }

    /// **The plane put in the air** (V96's `--fly-at`, a measurement start): stood at `at` along
    /// `facing`, wings level, flying at `speed` feet a second along it ([`Flight::launch`]), its spawn
    /// left where the ride put it. Nothing for a ride with no plane.
    pub fn launch_plane(&mut self, at: Vf, facing: Vf, speed: f32, acos: &game_sim::air::AcosTable) {
        let Some(plane) = self.plane.as_mut() else { return };
        let Some(f) = plane.flight.as_mut() else { return };
        f.launch(at, facing, facing.mul(Vf::splat(speed)), acos);
        plane.placed = true;
        plane.pose();
    }

    /// The machines this player can be put on: the bike, and the plane whenever the ride has one —
    /// on every map since P113, parked copy or not.
    pub fn machines(&self) -> Vec<Kind> {
        let mut out = vec![Kind::Bike];
        if self.plane.is_some() {
            out.push(Kind::Biplane);
        }
        if self.heli.is_some() {
            out.push(Kind::Kiowa);
        }
        out.extend(self.cars.iter().filter_map(|c| c.machine.car_kind()));
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
        if detour(to) || detour(self.up) {
            return self.switch_car(to, sim, world);
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
            Kind::Buggy | Kind::Monster | Kind::Baja | Kind::Kiowa => return self.switch_car(to, sim, world),
        }
        // A trick or combo in progress fails as a bail fails it (P161): the trick stopped, the
        // chain's points lost and not banked, so the HUD plays the pop-up out as a bail's and the
        // bike taken back finds no trick going.
        sim.rider.trick_lost();
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
                plane.spawn = Spawn { at: f.spawn_origin(), facing: f.spawn.1 };
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
        self.pass_among(sim, dt, controls, me, spots, world, markers, Among::default())
    }

    /// [`Garage::pass_marked`] among the other seats (N22): their contacts, rivals and bikes.
    #[allow(clippy::too_many_arguments)]
    pub fn pass_among(
        &mut self,
        sim: &mut Sim,
        dt: F,
        controls: &Controls,
        me: usize,
        spots: &[Spot],
        world: &mut dyn World,
        markers: bool,
        among: Among<'_>,
    ) -> (rider::Frame, Option<Did>) {
        let said = self.step_among(sim, dt, controls, world, among);
        if !markers {
            return (said, None);
        }
        let did = match self.up {
            Kind::Bike => markers::frame(sim, me, spots, controls),
            // The plane's own set (P107), on the d-pad the plane no longer reads.
            Kind::Biplane => self.markers(sim, me, spots, &[], world),
            // A car has no markers yet (S149e): the d-pad steers it. Nor the helicopter (P216),
            // whose d-pad is its stick's.
            Kind::Buggy | Kind::Monster | Kind::Baja | Kind::Kiowa => None,
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
        if let Some(car) = self.car_up() {
            return car.spot(world);
        }
        if let (Kind::Kiowa, Some(heli)) = (self.up, &self.heli) {
            return heli.spot(world);
        }
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

    /// The flight of the plane the player flies, once it is placed (N22a): what the riders' bikes
    /// meet ([`Flight::hull`], [`Flight::flown`]).
    pub fn flight_flown(&self) -> Option<&Flight> {
        self.plane_flown().filter(|p| p.placed)?.flight.as_ref()
    }

    /// Its plane's body as a rider's bike pushed it (N22a, [`Flight::pushed`]), written back.
    pub fn plane_pushed(&mut self, by: &game_sim::rider_contact::Flown) {
        if let Some(p) = self.plane.as_mut()
            && let Some(f) = p.flight.as_mut()
        {
            f.pushed(by);
            p.pose();
        }
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
        self.step_among(sim, dt, controls, world, Among::default())
    }

    /// [`Garage::step`] among the other seats (N22): the bike with their contacts and rivals
    /// ([`Sim::step_among`]), the plane with their bikes ([`Plane::step_among`]). Alone, it is
    /// [`Garage::step`] exactly.
    pub fn step_among(&mut self, sim: &mut Sim, dt: F, controls: &Controls, world: &mut dyn World, among: Among<'_>) -> rider::Frame {
        if detour(self.up) {
            return self.drive(sim, dt, world);
        }
        match (self.up, &mut self.plane) {
            (Kind::Biplane, Some(plane)) => {
                // The track's path cars run on whichever machine is up (S81).
                let respawned = plane.step_traffic(sim, dt, world, among.bikes, among.met);
                rider::Frame { respawned, ..rider::Frame::default() }
            }
            // The bike's ghost ticks inside its update (`Rider::frame`), so a bike the plane has the
            // controller from does not count its ghost down either.
            _ => match among.props {
                Some((props, seat)) => sim.step_shared(dt, controls, world, among.bump, among.rivals, props, seat),
                None => sim.step_among(dt, controls, world, among.bump, among.rivals),
            },
        }
    }

    /// [`Garage::step_among`] on the plane up to its frame (N22a1): the track's path cars, then
    /// [`Plane::ready`]. `None` but with the plane up, and then nothing ran.
    pub fn ready_plane(&mut self, sim: &mut Sim, dt: F, world: &mut dyn World) -> Option<Readied> {
        if detour(self.up) || self.up != Kind::Biplane {
            return None;
        }
        let plane = self.plane.as_mut()?;
        sim.traffic.frame(dt, world);
        plane.ready(sim, world)
    }

    /// The flight [`Garage::ready_plane`] readied, for [`fly_together`] to fly.
    pub fn readied_flight(&mut self) -> Option<&mut Flight> {
        self.plane.as_mut()?.flight.as_mut()
    }

    /// [`Garage::pass_among`] on the plane after its frame (N22a1): [`Plane::settle`], then the
    /// plane's markers if the seat has them on.
    pub fn settle_plane(
        &mut self,
        sim: &mut Sim,
        me: usize,
        spots: &[Spot],
        world: &mut dyn World,
        markers: bool,
        ready: Readied,
    ) -> (rider::Frame, Option<Did>) {
        let respawned = self.plane.as_mut().is_some_and(|p| p.settle(ready));
        let said = rider::Frame { respawned, ..rider::Frame::default() };
        let did = if markers { self.markers(sim, me, spots, &[], world) } else { None };
        (said, did)
    }
}

/// **The ride's flown planes' frames, together** (N22a1): every [`Plane::ready`]'d plane in one
/// [`flight::frames_together`], so they strike each other as the original's level node has its
/// planes meet — one dynamics step, their resolves interleaved substep by substep. One plane alone
/// flies its own [`Flight::frame_met`], as [`Plane::step_met`] does.
pub fn fly_together(dt: F, fleet: &mut [flight::Fly<'_>], world: &mut dyn World, tables: &game_sim::ride::Tables) {
    match fleet {
        [] => {}
        [one] => {
            let _ = one.flight.frame_met(dt, one.pad, world, tables, one.bikes, one.met.as_deref_mut());
        }
        _ => {
            let _ = flight::frames_together(dt, fleet, world, tables);
        }
    }
}

/// **A plane readied for its frame** (N22a1, [`Plane::ready`]): the pad it flies, whether the chord
/// put it back, whether it was wrecked going in, and the flight's own values the seat's scheme
/// stands in for during the frame ([`Plane::settle`] puts them back).
pub struct Readied {
    pub pad: game_sim::biplane::Pad,
    chord: bool,
    wrecked: bool,
    min_elevator: F,
    dead_zone: F,
    feed_forward: F,
    max_pitch_rate: F,
}

/// **The other seats, as one seat's step meets them** (N22, S224, S225): what split screen hands
/// each seat's pass, as the online ride does each seat's step — this seat's share of the tick's
/// rider-against-rider contacts ([`game_sim::rider_contact::Bump`]), the riders it can joust, and the
/// bikes in its plane's world. Empty, a seat steps as it does alone.
#[derive(Default)]
pub struct Among<'a> {
    pub bump: Option<&'a mut game_sim::rider_contact::Bump>,
    pub rivals: &'a [game_sim::ride::Rival],
    pub bikes: &'a [game_sim::flight_search::Bike],
    /// The bikes that struck its plane's hull this tick (N22a): their mirrors and chassis.
    pub met: Option<&'a mut flight::Met>,
    /// The seats' one prop set and this seat's place among them (S197c, [`game_sim::prop_seats`]):
    /// its bike strikes the set the others do.
    pub props: Option<(&'a mut game_sim::prop_seats::Shared, usize)>,
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
            // Online rides no car (S149e): a car pick is no pick.
            Some(Kind::Buggy | Kind::Monster | Kind::Baja) => 0,
            // Nor the helicopter (P216).
            Some(Kind::Kiowa) => 0,
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
            Plane::flying(Spawn { at: f.spawn_origin(), facing: f.spawn.1 }, f)
        }
    };
    let mut garage = Garage { up, plane: Some(plane), taken: true, cars: Vec::new(), heli: None };
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
    start_lane_on(template, bay, None, sim);
}

/// [`start_lane`] with the body's spot and up the original gives a file bay (B6, [`Lot::body_spot`]):
/// with one, the plane stands as an offline plane in the same bay does at its first step; without,
/// the model's origin on `bay`, world up.
pub fn start_lane_on(template: &Flight, bay: Spawn, body: Option<(Vf, Vf)>, sim: &mut Sim) {
    let mut plane = Plane::flying(bay, template.clone());
    if let Some(f) = &mut plane.flight {
        match body {
            Some((at, up)) => f.reset_body(at, bay.facing, up, &sim.tables.ride.acos),
            None => f.reset(bay.at, bay.facing, &sim.tables.ride.acos),
        }
    }
    sim.plane = plane.flight.take();
}

/// One tick of a seat's plane online (N17): [`Plane::step`] on the flight in its [`Sim::plane`], the
/// seat's pad and its biplane scheme, the bike not stepped. Whether the reset chord or a wreck's
/// recovery put the plane back this tick. A seat on the bike does nothing.
pub fn fly_lane(sim: &mut Sim, dt: F, wanted: Wanted, world: &mut dyn World) -> bool {
    fly_lane_among(sim, dt, wanted, world, &[])
}

/// [`fly_lane`] with the other seats' `bikes` in the plane's world (S225, [`Plane::step_among`]).
pub fn fly_lane_among(sim: &mut Sim, dt: F, wanted: Wanted, world: &mut dyn World, bikes: &[game_sim::flight_search::Bike]) -> bool {
    fly_lane_met(sim, dt, wanted, world, bikes, None)
}

/// [`fly_lane_among`] with the bikes that struck the seat's plane this tick (N22a, [`Plane::step_met`]).
pub fn fly_lane_met(
    sim: &mut Sim,
    dt: F,
    wanted: Wanted,
    world: &mut dyn World,
    bikes: &[game_sim::flight_search::Bike],
    met: Option<&mut flight::Met>,
) -> bool {
    let Some(f) = sim.plane.take() else { return false };
    let mut plane =
        Plane { every_bank: wanted.every_bank, round_stick: wanted.round_stick, dpad: wanted.dpad_rides, ..Plane::around(f) };
    let respawned = plane.step_traffic(sim, dt, world, bikes, met);
    sim.plane = plane.flight.take();
    respawned
}

/// [`fly_lane_met`] up to the frame (N22a1): the track's path cars, then [`Plane::ready`] on the
/// seat's flight, which the plane holds until [`settle_lane`]. `None` on the bike.
pub fn ready_lane(sim: &mut Sim, dt: F, wanted: Wanted, world: &mut dyn World) -> Option<(Plane, Readied)> {
    let f = sim.plane.take()?;
    sim.traffic.frame(dt, world);
    let mut plane =
        Plane { every_bank: wanted.every_bank, round_stick: wanted.round_stick, dpad: wanted.dpad_rides, ..Plane::around(f) };
    match plane.ready(sim, world) {
        Some(ready) => Some((plane, ready)),
        None => {
            sim.plane = plane.flight.take();
            None
        }
    }
}

/// [`fly_lane_met`] after the frame (N22a1): [`Plane::settle`], the flight back in the seat's lane.
pub fn settle_lane(sim: &mut Sim, mut plane: Plane, ready: Readied) -> bool {
    let respawned = plane.settle(ready);
    sim.plane = plane.flight.take();
    respawned
}

/// **Where an online seat is**, for another player's marker Up onto them (P107): [`Garage::spot`]
/// for a seat whose plane is its [`Sim::plane`] — the plane as it flies (P141), or the ground under
/// it while it is on or near the ground, the bike's spot on the bike.
pub fn lane_spot(sim: &Sim, world: &mut dyn World) -> Spot {
    match &sim.plane {
        Some(f) => Garage { up: Kind::Biplane, plane: Some(Plane::around(f.clone())), taken: true, cars: Vec::new(), heli: None }.spot(sim, world),
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
                plane.spawn = Spawn { at: f.spawn_origin(), facing: f.spawn.1 };
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

// ---- the cars (S149e) ----------------------------------------------------------------------------

/// **One of a player's cars** (S149e): the buggy, the monster truck or the baja, its `.phy` and its
/// model off `machine.res`, the bay `machine.txt` parks this player's copy in, and the car itself
/// once the player has taken it ([`game_sim::car::Machine`]).
///
/// The switch takes it out of its bay (S245): a fresh car ([`game_sim::car::Machine::new`]) stood on
/// the ground where `machine.txt` parks it, at rest. Away from its own track it has no bay
/// ([`Machine::home`]) and the switch spawns it on the player as it spawns the plane (P102). The car
/// left goes, as the plane left does. Its bay is drawn see-through as the plane's and offered by the
/// pop-up ([`offer`]).
#[derive(Clone, Debug)]
pub struct CarRide {
    pub machine: Machine,
    /// The livery it wears, an index into [`Machine::models`] ([`livery`]).
    pub livery: usize,
    pub car: game_sim::car::Car,
    pub model: game_sim::car::CarModel,
    pub bay: Option<Spawn>,
    /// The car while it is out: from the switch onto it until the switch off.
    pub driven: Option<game_sim::car::Machine>,
    /// Where it was last taken (its origin).
    pub taken_at: Vf,
    /// Its collision object's ray probe, the car's `m.col` ([`game_sim::car::Kind::rays`]), and the
    /// object itself while the car is out (S229). A disc without the file leaves the car to its tires.
    pub rays: Option<game_sim::flight_search::Rays>,
    pub struck: Option<game_sim::car::Collision>,
    /// The engine's shared smooth noise under the sound's pitch (S247), built off [`CarRide::rng`] on
    /// its first sample, as the original builds it off `rand()` wherever the session's generator
    /// stands then; kept from one take to the next, as the original's is for the session.
    pub noise: Option<game_sim::noise::SmoothNoise>,
    /// The generator the noise's tables are drawn from: the port's own seed, the session's `rand()`
    /// being the wall clock's.
    pub rng: game_sim::rng::Rng,
}

/// The seed of a car's [`CarRide::rng`]. The port's own number.
const CAR_NOISE_SEED: u32 = 0x0247_6361;

/// How far over the ground a car taken is stood, past where its tires' bottoms hang, feet: it drops
/// onto its springs from there. The port's own number.
const CAR_DROP: f32 = 0.5;

impl CarRide {
    /// Copy `copy` of `machine`, in its livery ([`livery`]), out of `machine.res` (`read` answers a
    /// member's bytes by name), parked at `bay`. An error for a machine that is no car, or a disc
    /// without the car's files.
    pub fn new(machine: Machine, copy: usize, bay: Option<Spawn>, read: &mut dyn FnMut(&str) -> Option<Vec<u8>>) -> Result<CarRide, String> {
        let kind = machine.car_physics().ok_or_else(|| format!("{machine:?} is not a car"))?;
        let phy = read(kind.phy()).ok_or_else(|| format!("machine.res has no {}", kind.phy()))?;
        let name = format!("{}\\anim\\{}.psg", machine.dir(), machine.models()[livery(copy)]);
        let psg = read(&name).ok_or_else(|| format!("machine.res has no {name}"))?;
        let model = game_sim::car::CarModel::read(&psg).map_err(|e| format!("{name}: {e}"))?;
        let rays = read(kind.rays()).and_then(|col| match game_sim::track::Rays::parse(&col) {
            Ok(r) => Some(game_sim::flight_search::Rays::of(&r)),
            Err(e) => {
                eprintln!("car: {}: {e}", kind.rays());
                None
            }
        });
        Ok(CarRide { machine, livery: livery(copy), car: game_sim::car::Car::read_bytes(&phy, kind), model, bay, driven: None, taken_at: Vf::default(), rays, struck: None, noise: None, rng: game_sim::rng::Rng::from_seed(CAR_NOISE_SEED) })
    }

    /// How high the car's origin stands over the ground with its tires' bottoms on it and its springs
    /// unloaded, and [`CAR_DROP`] more.
    pub fn rest_height(&self) -> f32 {
        let lowest = (0..4).map(|k| self.model.tire_nodes[k].y() - 2.0 * self.car.tires[k].radius.0).fold(f32::MAX, f32::min);
        CAR_DROP - lowest
    }

    /// The car taken: a fresh one, driven, standing in its bay where it has one (S245: where
    /// `machine.txt` parks it, along the bay's heading flattened, as the original's swap resets a
    /// machine to its spot), and otherwise at `spot`, along its heading flattened (or +z, where that
    /// points straight up or down); on the ground there either way. `None`, and nothing changes, with
    /// no ground there.
    pub fn take(&mut self, spot: &Spot, world: &mut dyn World) -> Option<()> {
        let (place, heading) = match self.bay {
            Some(bay) => (bay.at, bay.facing),
            None => (spot.at, spot.forward),
        };
        let ground = ground_under(world, place)?;
        let facing = level(heading).unwrap_or(Vf::new(0.0, 0.0, 1.0, 0.0));
        let at = Vf::new(place.x(), ground + self.rest_height(), place.z(), 1.0);
        let mut m = game_sim::car::Machine::new(&self.car, &self.model, at, facing, Vf::new(0.0, 1.0, 0.0, 0.0));
        m.drive(true);
        self.struck = self.rays.clone().map(|rays| {
            let mut c = game_sim::car::collision_of_model(&self.car, &self.model, &m.body, rays);
            c.refresh(&m.body, world);
            c
        });
        self.driven = Some(m);
        self.taken_at = at;
        Some(())
    }

    /// Where the car's origin is and which way it points: its frame's, once it has stepped.
    pub fn place(&self) -> Option<(Vf, Vf)> {
        let m = self.driven.as_ref()?;
        let at = m.matrix.0[3];
        let forward = m.matrix.0[2];
        Some((Vf::new(at.x(), at.y(), at.z(), 1.0), Vf::new(forward.x(), forward.y(), forward.z(), 0.0)))
    }

    /// What its engine voice reads while it is out (S247): the drivetrain's sound pitch and top revs,
    /// the gas after its rate limit, and where the body is and how fast it goes. Nothing for a car not
    /// out.
    pub fn engine_sound(&self) -> Option<EngineSound> {
        let m = self.driven.as_ref()?;
        let (at, _) = self.place()?;
        let v = m.body.velocity;
        Some(EngineSound {
            profile: self.machine.dir(),
            pitch: m.drivetrain.pitch.0,
            top: m.drivetrain.redline.0,
            throttle: m.controls.throttle.0,
            at: [at.x(), at.y(), at.z()],
            vel: [v.x(), v.y(), v.z()],
        })
    }

    /// What the shell says of it while it is out: how far it has gone from where it was taken, how
    /// fast, whether a tire is down and how upright it is.
    pub fn report(&self) -> Option<String> {
        let m = self.driven.as_ref()?;
        let (at, _) = self.place()?;
        let gone = (at.x() - self.taken_at.x()).hypot(at.z() - self.taken_at.z());
        let down = if m.grounded { "on its tires" } else { "off the ground" };
        Some(format!(
            "car: {} {gone:.1} ft from where it was taken, at {:.1}, {:.1}, {:.1}, {:.1} ft/s, {down}, up {:.3}",
            self.machine.section().to_ascii_lowercase(),
            at.x(),
            at.y(),
            at.z(),
            m.ground_speed.0,
            m.up.y()
        ))
    }

    /// The player in it, for whatever is put where they are: on the ground under the car, upright,
    /// along its heading flattened, stopped; the car's own place with no ground under it.
    pub fn spot(&self, world: &mut dyn World) -> Spot {
        let up = Vf::new(0.0, 1.0, 0.0, 0.0);
        let Some((at, forward)) = self.place() else {
            let bay = self.bay.unwrap_or(Spawn { at: Vf::new(0.0, 0.0, 0.0, 1.0), facing: Vf::new(0.0, 0.0, 1.0, 0.0) });
            return Spot::still(bay.at, bay.facing, up);
        };
        let forward = level(forward).unwrap_or(Vf::new(0.0, 0.0, 1.0, 0.0));
        match ground_under(world, at) {
            Some(g) => Spot::still(Vf::new(at.x(), g, at.z(), 1.0), forward, up),
            None => Spot::still(at, forward, up),
        }
    }

    /// One frame of the car on `pad` through `table` (the ride's bindings): the stick steers and
    /// leans, the commands are the pedals ([`game_sim::car::Controls::input`]), then the car's step
    /// over `world`, against the world's objects through its collision object where it has one
    /// ([`game_sim::car::Machine::step_struck`]). The engine's sound pitch samples the shared smooth
    /// noise ([`CarRide::noise`]). Nothing for a car not out.
    pub fn step(&mut self, pad: &game_sim::input::Pad, table: &game_sim::bindings::Table, tables: &game_sim::ride::Tables, dt: F, world: &mut dyn World) {
        use game_sim::input::{Axis, AxisRange};
        let Some(m) = self.driven.as_mut() else { return };
        let stick = (pad.axis_through(Axis::LeftX, &AxisRange::PLAYER), pad.axis_through(Axis::LeftY, &AxisRange::PLAYER));
        m.controls.input(&self.car, stick, F::ZERO, m.ground_speed, m.side_slip, |c| table.value(pad, c));
        let (noise, rng) = (&mut self.noise, &mut self.rng);
        let mut sample = |t: F| noise.get_or_insert_with(|| game_sim::noise::SmoothNoise::build(rng)).at(t);
        match self.struck.as_mut() {
            Some(c) => m.step_struck(&self.car, &mut Through(world), c, dt, &tables.acos, &tables.curve, &mut sample),
            None => m.step(&self.car, &mut Through(world), dt, &tables.acos, &tables.curve, &mut sample),
        }
    }
}

/// What a driven car's engine voice reads ([`CarRide::engine_sound`]).
#[derive(Clone, Copy, Debug)]
pub struct EngineSound {
    /// The machine's directory name, its `carengine.aud` profile.
    pub profile: &'static str,
    pub pitch: f32,
    pub top: f32,
    pub throttle: f32,
    pub at: [f32; 3],
    pub vel: [f32; 3],
}

/// A `dyn World` for the car's step, which takes its world by type: every query passed through.
struct Through<'a>(&'a mut dyn World);

impl World for Through<'_> {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.0.ground(at)
    }
    fn surface(&mut self, at: Vf) -> u8 {
        self.0.surface(at)
    }
    fn wheel_surface(&mut self, at: Vf) -> Option<u8> {
        self.0.wheel_surface(at)
    }
    fn solid(&mut self, bundle: &game_sim::collide::Bundle, record: &mut game_sim::collide::Record) -> bool {
        self.0.solid(bundle, record)
    }
    fn groups(&mut self, bundle: &game_sim::collide::Bundle, record: &mut game_sim::collide::Record) -> bool {
        self.0.groups(bundle, record)
    }
    fn crowded(&mut self, centre: Vf, radius: F) -> bool {
        self.0.crowded(centre, radius)
    }
    fn area(&self) -> Option<game_sim::biplane::Area> {
        self.0.area()
    }
    fn wrap(&self) -> [bool; 2] {
        self.0.wrap()
    }
    fn off_world(&self, at: Vf, margin: F) -> bool {
        self.0.off_world(at, margin)
    }
    fn keeps(&mut self, at: Vf) -> bool {
        self.0.keeps(at)
    }
    fn heightfield(&mut self) -> Option<Box<dyn game_sim::contact_search::Heightfield + '_>> {
        self.0.heightfield()
    }
    fn collision(&mut self) -> (Option<Box<dyn game_sim::contact_search::Heightfield + '_>>, Option<&game_sim::world::Props>) {
        self.0.collision()
    }
    fn wooded(
        &mut self,
    ) -> (Option<Box<dyn game_sim::contact_search::Heightfield + '_>>, Option<&game_sim::world::Props>, Option<&game_sim::eco::Trees>) {
        self.0.wooded()
    }
    fn wooded_near(
        &mut self,
        at: Vf,
    ) -> (Option<Box<dyn game_sim::contact_search::Heightfield + '_>>, Option<&game_sim::world::Props>, Option<&game_sim::eco::Trees>) {
        self.0.wooded_near(at)
    }
    fn near_set(&self, at: Vf) -> Option<[i32; 2]> {
        self.0.near_set(at)
    }
    fn moving(&mut self) -> Option<&mut game_sim::world::Props> {
        self.0.moving()
    }
}

/// The cars the disc has, the buggy, the monster truck and the baja, each in every livery (copy `n`
/// is livery `n`'s), those whose files `read` finds: what a [`Lot`] keeps ([`Lot::cars`]).
pub fn car_kit(read: &mut dyn FnMut(&str) -> Option<Vec<u8>>) -> Vec<CarRide> {
    let mut out = Vec::new();
    for m in [Machine::Buggy, Machine::Monster, Machine::Baja] {
        for copy in 0..LIVERIES {
            match CarRide::new(m, copy, None, read) {
                Ok(c) => out.push(c),
                Err(e) => eprintln!("cars: {e}"),
            }
        }
    }
    out
}

impl Lot {
    /// Player `copy`'s cars: one of each car [`Lot::cars`] has, in their livery, parked in their bay.
    /// Each car's bay stood on the ground under it (S245), where the file's height is not: free07's
    /// `machine.txt` parks its bajas at a height of 0, under the dirt. The car stands on its tires
    /// with its springs unloaded ([`CarRide::rest_height`] less [`CAR_DROP`]) over the ground
    /// `ground` finds asked from [`PROBE`] over the spot, which is what a take stands it on; a spot
    /// off the map keeps the file's height. Bays without the car's files are left as they are.
    pub fn stand_cars(&mut self, ground: &mut dyn FnMut(Vf) -> (Vf, Vf)) {
        for bay in self.bays.iter_mut().filter(|b| b.machine.car_kind().is_some()) {
            let Some(car) = self.cars.iter().find(|c| c.machine == bay.machine) else { continue };
            let at = bay.spawn.at;
            let (on, _) = ground(Vf::new(at.x(), at.y() + PROBE, at.z(), 1.0));
            if on.y() > NO_GROUND {
                bay.spawn.at = Vf::new(at.x(), on.y() + car.rest_height() - CAR_DROP, at.z(), 1.0);
            }
        }
    }

    pub fn cars_for(&self, copy: usize) -> Vec<CarRide> {
        [Machine::Buggy, Machine::Monster, Machine::Baja]
            .into_iter()
            .filter_map(|m| {
                let mut car = self.cars.iter().filter(|c| c.machine == m).nth(livery(copy))?.clone();
                car.bay = self.bay(m, copy).map(|b| b.spawn);
                Some(car)
            })
            .collect()
    }
}

impl Garage {
    /// The car up, if one is.
    pub fn car_up(&self) -> Option<&CarRide> {
        let m = self.up.car()?;
        self.cars.iter().find(|c| c.machine == m)
    }

    /// [`Garage::switch`] onto or off a car (S149e): the machine taken spawned where the player is
    /// ([`Garage::spot`]), the car left gone. Onto the bike from a car, the bike stands on the ground
    /// under it; onto the plane, the plane is landed there.
    fn switch_car(&mut self, to: Kind, sim: &mut Sim, world: &mut dyn World) -> Result<Kind, String> {
        let from = self.spot(sim, world);
        match to.car() {
            Some(m) => {
                let car = self.cars.iter_mut().find(|c| c.machine == m).ok_or_else(|| format!("this ride has no {}", to.name()))?;
                car.take(&from, world).ok_or("there is no ground here to stand the car on")?;
            }
            None if to == Kind::Biplane => {
                let plane = self.plane.as_mut().ok_or("this ride has no biplane")?;
                land(plane, from, world).ok_or("there is no ground under the car to land the plane on")?;
                self.taken = true;
            }
            None if to == Kind::Kiowa => {
                let heli = self.heli.as_mut().ok_or("this ride has no helicopter")?;
                heli.take(&from, world).ok_or("there is no ground here to stand the helicopter on")?;
            }
            None => sim.rider.teleport_to(from),
        }
        let left = self.up.car();
        if let Some(c) = self.cars.iter_mut().find(|c| Some(c.machine) == left) {
            c.driven = None;
            c.struck = None;
        }
        if let (Kind::Kiowa, Some(heli)) = (self.up, self.heli.as_mut()) {
            heli.flown = None;
        }
        sim.rider.trick_lost();
        self.up = to;
        sim.markers.up(markers_of(to));
        Ok(to)
    }

    /// One pass of the car up: the track's path cars, then the car on the seat's pad. The bike is
    /// not stepped, and the frame the rider reports is empty.
    fn drive(&mut self, sim: &mut Sim, dt: F, world: &mut dyn World) -> rider::Frame {
        sim.traffic.frame(dt, world);
        let m = self.up.car();
        if let Some(car) = self.cars.iter_mut().find(|c| Some(c.machine) == m) {
            car.step(sim.input.pad(0), sim.table(), &sim.tables.ride, dt, world);
        }
        if let (Kind::Kiowa, Some(heli)) = (self.up, self.heli.as_mut()) {
            heli.step(sim.input.pad(0), sim.table(), &sim.tables.ride, dt, world);
        }
        rider::Frame::default()
    }
}

// ---- the Kiowa (P216) ----------------------------------------------------------------------------

/// The helicopter's body's inverse inertia, as the original holds it once the helicopter is built
/// (C14's start state, `c14-kiowa` in `context/states/`): the
/// original works it out of the model's box, which `game_sim` does not read, so the built value is
/// carried, as [`flight`]'s plane carries its own.
const KIOWA_INV_INERTIA: Vf = Vf([f32::from_bits(0x38c2_4305), f32::from_bits(0x38cd_5c6b), f32::from_bits(0x3a49_6ce0), 1.0]);

/// The collective's filter as the constructor builds it (value 0.5, time 1).
const KIOWA_FILTER: game_sim::kiowa::Filter = game_sim::kiowa::Filter { value: F(0.5), time: F(1.0), step: F(0.0) };

/// The engine's dynamics world the helicopter is stepped in: the plane's, 115 substeps a second.
const KIOWA_WORLD: game_sim::biplane::World = game_sim::biplane::World { iterations: 1, rate: F(115.0) };

/// The helicopter's body's inertia tensor as the built helicopter holds it (the body's `+0x110` in
/// C14's start state), which its collision object's response is tuned by
/// ([`game_sim::kiowa::Collision::new`]); the inverse is [`KIOWA_INV_INERTIA`].
const KIOWA_INERTIA: Vf = Vf([f32::from_bits(0x4628_adfa), f32::from_bits(0x461f_901b), f32::from_bits(0x44a2_ae40), 1.0]);

/// How far over the ground a helicopter taken is stood, feet: it settles onto its skids from there.
/// The port's own number.
const KIOWA_DROP: f32 = 0.25;

/// The world round a flown helicopter for [`game_sim::kiowa::Kiowa::tick`]: the original's ground
/// (C14a's [`game_sim::kiowa::Grounded`]: [`game_sim::contact::resolve`] on the collision record,
/// its touching flag and entries, the rotor's wreck rays' ends in the ground for the strike), and the
/// wreck and put-back as the port's own until S220a1 ports the original's: a wreck hides the blades
/// (they go, the tail rotor shows bent) and the put-back stands the helicopter back on the ground
/// where it is, whole and at rest.
struct KiowaAround<'a> {
    ground: game_sim::kiowa::Grounded<'a>,
    /// The put-back asked for, done after the tick (it builds the helicopter again).
    recover: bool,
}

/// `v` (`xyz`) turned by the unit quaternion `q` (`x, y, z, w`), in plain arithmetic.
fn rotate(q: Vf, v: Vf) -> [f32; 3] {
    let (qx, qy, qz, qw) = (q.x(), q.y(), q.z(), q.w());
    let (vx, vy, vz) = (v.x(), v.y(), v.z());
    let (tx, ty, tz) = (2.0 * (qy * vz - qz * vy), 2.0 * (qz * vx - qx * vz), 2.0 * (qx * vy - qy * vx));
    [vx + qw * tx + (qy * tz - qz * ty), vy + qw * ty + (qz * tx - qx * tz), vz + qw * tz + (qx * ty - qy * tx)]
}

use game_sim::kiowa::Surroundings as _;

impl game_sim::kiowa::Surroundings for KiowaAround<'_> {
    fn post(&mut self, body: &game_sim::biplane::Body) {
        self.ground.post(body);
    }
    fn resolve(&mut self, body: &mut game_sim::biplane::Body) {
        self.ground.resolve(body);
    }
    fn touching(&self) -> bool {
        self.ground.touching()
    }
    fn ignored(&self) -> bool {
        self.ground.ignored()
    }
    fn entries(&self) -> Vec<game_sim::kiowa::Touch> {
        self.ground.entries()
    }
    fn strike(&mut self, matrix: &game_sim::math::Mat4) -> game_sim::kiowa::Strike {
        self.ground.strike(matrix)
    }
    fn crashed(&mut self, asked: &game_sim::kiowa::Crashed, ride: &mut game_sim::kiowa::Ride, rng: &mut game_sim::rng::Rng) {
        self.ground.crashed(asked, ride, rng);
        if asked.wreck.is_some() {
            ride.rotor_hidden = true;
        }
        self.recover |= asked.recover;
    }
}

/// A flown helicopter: its flight ([`game_sim::kiowa::Kiowa`]), the frame's state round it, its
/// collision object, and the `rand()` its wreck and touchdown draw from.
#[derive(Clone, Debug)]
pub struct Heli {
    pub kiowa: game_sim::kiowa::Kiowa,
    pub ride: game_sim::kiowa::Ride,
    pub collision: game_sim::kiowa::Collision,
    pub rng: game_sim::rng::Rng,
}

impl Heli {
    /// Its collision record's touching flag, as the last tick left it.
    pub fn touching(&self) -> bool {
        self.collision.record.touching
    }
}

/// **A player's helicopter** (P216): `kiowa.phy`, the bay `machine.txt` parks their copy in and its
/// livery, and the helicopter itself while the player has it.
///
/// The switch spawns it on the player as it spawns a car (S149e): stood on the ground where they
/// are, nose along their heading, level, at rest, its rotor turning, a fresh helicopter. It flies on
/// the seat's pad through the ride's bindings ([`game_sim::kiowa::Pad::from_sticks`]): Cross climbs,
/// Square descends, L1 and R1 are the rudder, the left stick (or the d-pad) asks for ground speed
/// forward and back and a bank left and right. The helicopter left goes, as the plane left does.
#[derive(Clone, Debug)]
pub struct KiowaRide {
    pub phy: game_sim::kiowa::Phy,
    pub livery: usize,
    pub bay: Option<Spawn>,
    pub flown: Option<Heli>,
    /// Where it was last taken (its model's origin).
    pub taken_at: Vf,
    /// Its collision object's ray probe, `kiowam.col` ([`KiowaRide::with_rays`]): without it the
    /// helicopter meets nothing, ground or wall.
    pub rays: game_sim::flight_search::Rays,
    /// How many times it has wrecked since it was last taken.
    pub wrecks: u32,
}

impl KiowaRide {
    /// The helicopter off `kiowa.phy`'s bytes, in `livery`, parked at `bay`.
    pub fn new(phy: &[u8], livery: usize, bay: Option<Spawn>) -> KiowaRide {
        let rays = game_sim::flight_search::Rays::default();
        KiowaRide { phy: game_sim::kiowa::Phy::read_bytes(phy), livery, bay, flown: None, taken_at: Vf::default(), rays, wrecks: 0 }
    }

    /// It with its collision object's ray probe off `kiowam.col`'s bytes
    /// ([`game_sim::kiowa::KIOWA_RAYS`]); as it was where they do not parse.
    pub fn with_rays(mut self, col: &[u8]) -> KiowaRide {
        match game_sim::track::Rays::parse(col) {
            Ok(r) => self.rays = game_sim::flight_search::Rays::of(&r),
            Err(e) => eprintln!("kiowa: {}: {e}", game_sim::kiowa::KIOWA_RAYS),
        }
        self
    }

    /// The helicopter taken at `spot`: a fresh one built off the file ([`KiowaRide::built`]) on the
    /// ground there along the spot's heading flattened (its bay's, or +z, where that points straight
    /// up or down), flown. `None`, and nothing changes, with no ground there.
    pub fn take(&mut self, spot: &Spot, world: &mut dyn World) -> Option<()> {
        let ground = ground_under(world, spot.at)?;
        let facing = level(spot.forward).or_else(|| self.bay.and_then(|b| level(b.facing))).unwrap_or(Vf::new(0.0, 0.0, 1.0, 0.0));
        let at = Vf::new(spot.at.x(), ground + KIOWA_DROP, spot.at.z(), 1.0);
        self.flown = Some(self.built(at, facing, world));
        self.taken_at = at;
        self.wrecks = 0;
        Some(())
    }

    /// The helicopter as the constructor leaves it, its model's origin at `origin` along `facing`,
    /// upright and at rest, being flown: the file's controller, rotor and aerodynamics with their
    /// states zero but the collective's filter ([`KIOWA_FILTER`]) and the held height (where it
    /// stands); the body off `[Mass Properties]` (its mass, centre of gravity, gravity scale and
    /// gravity force; undamped, as C14's state holds it) with the built inertia
    /// ([`KIOWA_INV_INERTIA`]); the file's wreck figures; the throw's scale 0, as C14's state holds
    /// it; the rotor at angle 0; its collision object off `[Mass Properties]`, the body, the built
    /// inertia ([`KIOWA_INERTIA`]) and the ray probe ([`KiowaRide::rays`]), its nearby list gathered
    /// where it stands, its record empty.
    pub fn built(&self, origin: Vf, facing: Vf, world: &mut dyn World) -> Heli {
        use game_sim::kiowa::{Kiowa, Loop, Measure, Ride};
        let p = &self.phy;
        let part = &p.mass_properties;
        let up = Vf::new(0.0, 1.0, 0.0, 0.0);
        let quat = game_sim::math::mat4_to_quat(&game_sim::math::basis(origin, facing, up, true, false));
        let c = rotate(quat, part.cofg);
        let position = Vf::new(origin.x() + c[0], origin.y() + c[1], origin.z() + c[2], 1.0);
        let clear = Vf::new(0.0, 0.0, 0.0, 1.0);
        let body = game_sim::biplane::Body {
            enabled: true,
            inv_mass: if part.mass.0 > 0.0 { F(1.0 / part.mass.0) } else { F::ZERO },
            damping: Vf::new(1.0, 1.0, 1.0, 1.0),
            last_position: position,
            position,
            velocity: clear,
            last_force: clear,
            force: clear,
            shock_peak: clear,
            shock: clear,
            inv_inertia: KIOWA_INV_INERTIA,
            ang_damping: Vf::new(1.0, 1.0, 1.0, 1.0),
            last_quat: quat,
            quat,
            cofg: part.cofg,
            moment: clear,
            last_moment: clear,
            rates: clear,
            twist_peak: clear,
            twist: clear,
            ..Default::default()
        };
        let mut measure = Measure::default();
        measure.flight(&body, &game_sim::air::AcosTable::build());
        let kiowa = Kiowa {
            controller: p.controller,
            rotor: p.rotor,
            aero: p.aero,
            control: Loop { filter: KIOWA_FILTER, hold_altitude: F(position.y()), ..Loop::default() },
            ..Kiowa::default()
        };
        let ride = Ride {
            body,
            measure,
            crash: p.crash(),
            flown: true,
            gravity_scale: part.gravity_scale,
            throw_scale: F::ZERO,
            gravity: part.gravity,
            area: world.area(),
            // Its sound is V114's: no clips, no draw.
            touchdown_sound: false,
            rotor_angle: F::ZERO,
            rotor_hidden: false,
        };
        let mut collision = game_sim::kiowa::Collision::new(part, &ride.body, KIOWA_INERTIA, self.rays.clone());
        collision.refresh(&ride.body, world);
        Heli { kiowa, ride, collision, rng: game_sim::rng::Rng::from_seed(0x0216_6b69) }
    }

    /// One frame of the helicopter on `pad` through `table`: the input step
    /// ([`game_sim::kiowa::Pad::from_sticks`], the left stick through the player's range), then its
    /// frame over `world` ([`game_sim::kiowa::Kiowa::tick`], [`KiowaAround`]), its collision object
    /// searched first ([`game_sim::kiowa::Collision::search`]), as the level's collision node searches
    /// every object before any dynamics world runs. A put-back asked for builds it again on the ground
    /// where it is, along its heading. Nothing for a helicopter not out.
    pub fn step(&mut self, pad: &game_sim::input::Pad, table: &game_sim::bindings::Table, tables: &game_sim::ride::Tables, dt: F, world: &mut dyn World) {
        use game_sim::input::{Axis, AxisRange};
        let Some(h) = self.flown.as_mut() else { return };
        let stick = (pad.axis_through(Axis::LeftX, &AxisRange::PLAYER), pad.axis_through(Axis::LeftY, &AxisRange::PLAYER));
        let input = game_sim::kiowa::Pad::from_sticks(stick, |c| table.value(pad, c));
        let movers = h.collision.search(&mut h.ride.body, world, dt);
        let ground = game_sim::kiowa::Grounded { collision: &mut h.collision, world: &mut *world, movers: &movers, rotor: &self.phy.wreck_rotor.rays, asked: Vec::new() };
        let mut around = KiowaAround { ground, recover: false };
        let asked = h.kiowa.tick(&mut h.ride, &KIOWA_WORLD, Some(input), &tables.curve, &tables.acos, &mut h.rng, dt, &mut around);
        self.wrecks += u32::from(asked.wreck.is_some());
        if around.recover {
            let rng = h.rng;
            let (at, forward) = self.place().expect("a flown helicopter has a place");
            let ground = ground_under(world, at).unwrap_or(at.y());
            let facing = level(forward).unwrap_or(Vf::new(0.0, 0.0, 1.0, 0.0));
            let mut fresh = self.built(Vf::new(at.x(), ground + KIOWA_DROP, at.z(), 1.0), facing, world);
            fresh.rng = rng;
            self.flown = Some(fresh);
        }
    }

    /// Where the helicopter's model's origin is and which way its nose points: the measure's matrix,
    /// which is where the original draws it.
    pub fn place(&self) -> Option<(Vf, Vf)> {
        let m = &self.flown.as_ref()?.ride.measure.matrix;
        let (at, forward) = (m.0[3], m.0[2]);
        Some((Vf::new(at.x(), at.y(), at.z(), 1.0), Vf::new(forward.x(), forward.y(), forward.z(), 0.0)))
    }

    /// Where the ground stand-in ([`KiowaAround`]) asks for the ground: under the skids, the body's
    /// centre less its centre of gravity turned into the world. The particles' body contact (V114).
    pub fn skids(&self) -> Option<Vf> {
        let body = &self.flown.as_ref()?.ride.body;
        let c = rotate(body.quat, body.cofg);
        let p = body.position;
        Some(Vf::new(p.x() - c[0], p.y() - c[1], p.z() - c[2], 1.0))
    }

    /// The matrix it is drawn by (rows `x`, `y`, `z` and the origin), while it is out.
    pub fn matrix(&self) -> Option<game_sim::math::Mat4> {
        Some(self.flown.as_ref()?.ride.measure.matrix)
    }

    /// Its rotor as the drawing wants it: the angle, whether the main blades are on, whether it is
    /// wrecked (the tail rotor bent). A parked one's is at rest and whole.
    pub fn rotor(&self) -> (f32, bool, bool) {
        match &self.flown {
            Some(h) => (h.ride.rotor_angle.0, !h.ride.rotor_hidden, h.ride.crash.reason != 0),
            None => (0.0, true, false),
        }
    }

    /// What the shell says of it while it is out: how far it went from where it was taken, how high
    /// it is over that, how fast, whether it is on the ground or wrecked, and how many times it
    /// wrecked since it was taken.
    pub fn report(&self) -> Option<String> {
        let h = self.flown.as_ref()?;
        let (at, _) = self.place()?;
        let gone = (at.x() - self.taken_at.x()).hypot(at.z() - self.taken_at.z());
        let state = match (h.ride.crash.reason, h.touching()) {
            (0, true) => "on its skids",
            (0, false) => "flying",
            _ => "wrecked",
        };
        Some(format!(
            "kiowa: {gone:.1} ft from where it was taken, {:.1} ft over it, at {:.1}, {:.1}, {:.1}, {:.1} ft/s, {state}, wrecked {} times",
            at.y() - self.taken_at.y(),
            at.x(),
            at.y(),
            at.z(),
            h.ride.measure.speed.0,
            self.wrecks,
        ))
    }

    /// The player in it, for whatever is put where they are: on the ground under the helicopter,
    /// upright, along its heading flattened, stopped; its own place with no ground under it.
    pub fn spot(&self, world: &mut dyn World) -> Spot {
        let up = Vf::new(0.0, 1.0, 0.0, 0.0);
        let Some((at, forward)) = self.place() else {
            let bay = self.bay.unwrap_or(Spawn { at: Vf::new(0.0, 0.0, 0.0, 1.0), facing: Vf::new(0.0, 0.0, 1.0, 0.0) });
            return Spot::still(bay.at, bay.facing, up);
        };
        let forward = level(forward).unwrap_or(Vf::new(0.0, 0.0, 1.0, 0.0));
        match ground_under(world, at) {
            Some(g) => Spot::still(Vf::new(at.x(), g, at.z(), 1.0), forward, up),
            None => Spot::still(at, forward, up),
        }
    }
}

/// A machine the switch spawns on the player and takes away again: a car or the helicopter.
fn detour(kind: Kind) -> bool {
    kind.car().is_some() || kind == Kind::Kiowa
}

impl Lot {
    /// Player `copy`'s helicopter: the disc's ([`Lot::kiowa`]) in their livery, parked in their bay.
    pub fn heli_for(&self, copy: usize) -> Option<KiowaRide> {
        let mut heli = self.kiowa.clone()?;
        heli.livery = livery(copy);
        heli.bay = self.bay(Machine::Helicopter, copy).map(|b| b.spawn);
        Some(heli)
    }
}

impl Garage {
    /// The helicopter up, if it is.
    pub fn heli_up(&self) -> Option<&KiowaRide> {
        self.heli.as_ref().filter(|_| self.up == Kind::Kiowa)
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
    fn a_world_parks_the_plane_beside_its_spawn() {
        // The home tile sits 5000, 3000 feet into the world, whose ground slopes up along z there.
        let home = [5000.0, 3000.0];
        let mut ground = |at: Vf| at.z() * 0.05;
        // The world's spawn faces +x, so its right (as park steps a copy) is +z.
        let start = Spawn { at: Vf::new(7000.0, 0.0, 9000.0, 1.0), facing: Vf::new(2.0, 0.5, 0.0, 0.0) };
        let moved = world_spots(TWO_OF_EACH.as_bytes(), home, start, 4.0, &mut ground);
        let gap = Machine::Biplane.spacing();
        for (i, m) in Machine::ALL.iter().enumerate() {
            let file = spots(TWO_OF_EACH.as_bytes(), *m);
            assert_eq!(moved[i].len(), file.len(), "{m:?}");
            for (a, b) in file.iter().zip(&moved[i]) {
                // One step for every spot: the file's layout is kept.
                assert_eq!((b.at.x() - a.at.x(), b.at.z() - a.at.z()), (7000.0 - 100.0, 9000.0 + gap + 50.0), "{m:?}");
                assert_eq!(b.facing, a.facing);
                // Its clearance over the home tile's ground, moved by the tile's offset.
                let clear = a.at.y() - (a.at.z() + home[1]) * 0.05;
                assert!((b.at.y() - (b.at.z() * 0.05 + clear)).abs() < 1e-2, "{m:?} keeps its clearance");
            }
        }
        // The first plane stands one spacing to the right of the spawn, out of the offer's reach.
        let lot = Lot::around(moved, 2, &mut ground);
        let first = lot.bay(Machine::Biplane, 0).unwrap().spawn.at;
        assert_eq!((first.x(), first.z()), (7000.0, 9000.0 + gap));
        assert!(gap > OFFER_RANGE);
        assert_eq!(lot.of(Machine::Helicopter).count(), 2);
        // A file with no spots stays empty.
        assert!(world_spots(b"[Machines]\n", home, start, 4.0, &mut ground).iter().all(Vec::is_empty));
    }

    #[test]
    fn a_spot_off_the_home_ground_stands_on_the_worlds_ground() {
        // A home tile 1143 feet square at 5000, 3000 in a world sloping up along z: the file's spots,
        // for a bigger track, sit past the tile's edge, where it answers no ground.
        let (home, size) = ([5000.0, 3000.0], 1143.0);
        let mut ground = |at: Vf| {
            let (x, z) = (at.x() - home[0], at.z() - home[1]);
            let tile = (0.0..size).contains(&x) && (0.0..size).contains(&z);
            if tile || at.z() > 8000.0 { at.z() * 0.05 } else { -1.0e6 }
        };
        let start = Spawn { at: Vf::new(7000.0, 0.0, 9000.0, 1.0), facing: Vf::new(1.0, 0.0, 0.0, 0.0) };
        let txt = "[Biplane1]\nPosition = 2000, 40, 1500\nDirection = 1, 0, 0\n";
        let rest = 4.0;
        let plane = Machine::ALL.iter().position(|&m| m == Machine::Biplane).unwrap();
        let at = world_spots(txt.as_bytes(), home, start, rest, &mut ground)[plane][0].at;
        assert_eq!((at.x(), at.z()), (7000.0, 9000.0 + Machine::Biplane.spacing()));
        assert!((at.y() - (at.z() * 0.05 + rest)).abs() < 1e-2, "on its wheels on the world's ground, not {}", at.y());
        // A spot the tile does cover keeps its own clearance.
        let txt = "[Biplane1]\nPosition = 500, 40, 500\nDirection = 1, 0, 0\n";
        let at = world_spots(txt.as_bytes(), home, start, rest, &mut ground)[plane][0].at;
        assert!((at.y() - (at.z() * 0.05 + 40.0 - 3500.0 * 0.05)).abs() < 1e-2, "{}", at.y());
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
        assert_eq!(Kind::parse("helicopter"), Ok(Kind::Kiowa));
        assert!(Kind::parse("tank").is_err());
    }
}
