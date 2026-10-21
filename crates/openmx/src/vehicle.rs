//! Which machine the player is on: the bike, or the freeride's biplane (P19).
//!
//! A freeride world carries more machines than the one the rider starts on. The track's own
//! `machine.txt` parks them — on free06 a biplane, `[Biplane1]`, standing on the dirt well away from
//! the start line — and the original lets the rider take one over in two ways: a prompt when the
//! rider stops within 30 ft of a driverless one ("Would you like to switch to the Biplane?",
//! `STR_SWITCH_BIPLANE`), and the pause menu's "Switch To Machine". Both answer through the one swap routine, and that routine is
//! what [`Garage::swap`] is (R8's reading, `research/oracle.txt`'s `machine_swap`):
//!
//! * **The machine left** loses the player's controller. If it is the bike its model is hidden, and
//!   no transform is written: the bike stays where it was left and nothing steps it.
//! * **The machine taken** gets the controller and the player's camera off its own list entry, and a
//!   biplane is unwrecked and reset — put back where `machine.txt` parked it, not beside the rider.
//!
//! The shell has no pause menu and no prompt, so its key is the menu's path: the swap with no 30 ft
//! and no 40 mph gate, which belong to the prompt and not to the swap.
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
//! The plane is not [`Sim::plane`]: the shell's `Sim` is the bike's ride, which the swap leaves where it
//! stands and which a plane session never steps, so its tick count stays where the swap found it.
//!
//! This file is std and `game_sim` only, for the same reason `pad.rs` is: the ride test compiles it
//! in by path, because a binary crate has nothing a test can `use`.

use game_sim::Sim;
use game_sim::flight::{self, Flight};
use game_sim::phy::Ini;
use game_sim::ride::Controls;
use game_sim::rider;
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
        let ini = Ini::parse_bytes(machine_txt);
        let name = format!("Biplane{}", player + 1);
        if !ini.names().iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            return None;
        }
        let s = ini.section(&name);
        s.text("Position")?;
        let at = s.vector("Position", Vf::default());
        let facing = s.vector("Direction", Vf::new(0.0, 0.0, 1.0, 0.0));
        Some(Spawn {
            at: Vf::new(at.x(), at.y(), at.z(), 1.0),
            facing: Vf::new(facing.x(), facing.y(), facing.z(), 0.0),
        })
    }
}

/// How far past the horizon the plane's up has to go before [`Plane::inverted`] changes its answer:
/// the sine of about 11 degrees of roll past knife edge. The port's own feel, not the original's,
/// which never asks (P50).
pub const INVERTED_MARGIN: f32 = 0.2;

/// The biplane: where it was parked, where it is, and the flight that moves it.
#[derive(Clone, Debug, PartialEq)]
pub struct Plane {
    /// Where `machine.txt` parked it, which is where every swap into it puts it back.
    pub spawn: Spawn,
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
}

impl Plane {
    /// A plane standing where it was parked, with no flight in it.
    pub fn parked(spawn: Spawn) -> Plane {
        Plane {
            spawn,
            at: spawn.at,
            facing: spawn.facing,
            quat: Vf::new(0.0, 0.0, 0.0, 1.0),
            up: Vf::new(0.0, 1.0, 0.0, 0.0),
            vel: Vf::new(0.0, 0.0, 0.0, 0.0),
            flight: None,
            placed: false,
        }
    }

    /// The plane with its flight: `flight` is [`Flight::new`] off the disc's file, which the next
    /// [`Plane::step`] stands at the spawn.
    pub fn flying(spawn: Spawn, mut flight: Flight) -> Plane {
        // The player's plane: the controls read the pad (an unflown one is the autopilot's alone).
        flight.attitude.flown = true;
        Plane { flight: Some(flight), ..Plane::parked(spawn) }
    }

    /// The swap's unwreck-and-reset: back to the spawn transform, at rest. The flight is put back by
    /// the next step ([`Flight::reset`]), which is the one with the `Sim`'s tables.
    pub fn reset(&mut self) {
        let flight = self.flight.take();
        *self = Plane { flight, ..Plane::parked(self.spawn) };
    }

    /// One frame of the plane with the controller on it: [`Flight::frame`] on the seat's pad, over
    /// `world`, and the drawn transform off its body. A plane with no flight stands where it is.
    pub fn step(&mut self, sim: &Sim, dt: F, world: &mut dyn World) {
        let Some(f) = &mut self.flight else { return };
        if !self.placed {
            f.reset(self.spawn.at, self.spawn.facing, &sim.tables.ride.acos);
            self.placed = true;
        }
        f.frame(dt, flight::pad(sim.input.pad(0)), world, &sim.tables.ride);
        let (quat, origin) = f.pose();
        let nose = f.measure.forward;
        (self.quat, self.at) = (quat, Vf::new(origin.x(), origin.y(), origin.z(), 1.0));
        self.facing = Vf::new(nose.x(), nose.y(), nose.z(), 0.0);
        let up = f.measure.up;
        self.up = Vf::new(up.x(), up.y(), up.z(), 0.0);
        let v = f.body.velocity;
        self.vel = Vf::new(v.x(), v.y(), v.z(), 0.0);
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
    /// The track's biplane, if its `machine.txt` parks one.
    pub plane: Option<Plane>,
}

impl Garage {
    /// A session that starts on `up`. Starting in a plane the track does not have is an error rather
    /// than a quiet bike, so that `--vehicle biplane` cannot pass for a session it is not.
    pub fn new(up: Kind, plane: Option<Plane>) -> Result<Garage, String> {
        if up == Kind::Biplane && plane.is_none() {
            return Err("--vehicle biplane: this track's machine.txt parks no biplane".into());
        }
        Ok(Garage { up, plane })
    }

    /// The swap: the controller moves to the other machine. The bike is left exactly as it is — no
    /// transform written, nothing stepped — and a plane being taken is reset to where it was parked.
    /// Returns the machine now up, or an error (and no swap) on a track with no plane.
    pub fn swap(&mut self) -> Result<Kind, String> {
        let to = self.up.other();
        if to == Kind::Biplane {
            let plane = self.plane.as_mut().ok_or("this track's machine.txt parks no biplane")?;
            plane.reset();
        }
        self.up = to;
        Ok(to)
    }

    /// [`Plane::inverted`] for the plane with the controller; never, with the bike up.
    pub fn inverted(&self, was: bool) -> bool {
        match (self.up, &self.plane) {
            (Kind::Biplane, Some(plane)) => plane.inverted(was),
            _ => false,
        }
    }

    /// One simulation pass for whichever machine has the controller. With the bike up this is
    /// exactly [`Sim::step`] and nothing else, which is what keeps a `--vehicle bike` session the
    /// shell it was before P19. With the plane up the bike is not stepped at all, the plane flies on
    /// the pad `sim` polled, and the frame the rider reports is empty.
    pub fn step(&mut self, sim: &mut Sim, dt: F, controls: &Controls, world: &mut dyn World) -> rider::Frame {
        match (self.up, &mut self.plane) {
            (Kind::Biplane, Some(plane)) => {
                plane.step(sim, dt, world);
                rider::Frame::default()
            }
            _ => sim.step(dt, controls, world),
        }
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
    fn a_swap_resets_the_plane_and_a_track_without_one_refuses() {
        let spawn = Spawn::biplane(MACHINES.as_bytes(), 0).unwrap();
        let mut plane = Plane::parked(spawn);
        plane.at = Vf::new(0.0, 500.0, 0.0, 1.0);
        let mut g = Garage::new(Kind::Bike, Some(plane)).unwrap();
        assert_eq!(g.swap(), Ok(Kind::Biplane));
        assert_eq!(g.plane.as_ref().unwrap().at, spawn.at);
        assert_eq!(g.swap(), Ok(Kind::Bike));

        assert!(Garage::new(Kind::Biplane, None).is_err());
        let mut g = Garage::new(Kind::Bike, None).unwrap();
        assert!(g.swap().is_err());
        assert_eq!(g.up, Kind::Bike);
    }

    #[test]
    fn the_words() {
        assert_eq!(Kind::parse("bike"), Ok(Kind::Bike));
        assert_eq!(Kind::parse("Biplane"), Ok(Kind::Biplane));
        assert!(Kind::parse("helicopter").is_err());
    }
}
