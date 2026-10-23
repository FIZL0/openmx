//! **Game modifiers** (P55): multipliers on numbers the ride already reads, set before it loads.
//!
//! Every modifier is a percentage on one or more fields the loaders fill — the bike's `.phy`
//! ([`Phy`]), the biplane's ([`phy::Plane`]), the integrator's speed limit the ride is built with
//! ([`Ride`]) and the engine's time-scale object ([`crate::tick::Engine::time_scale`]). They are
//! applied to those values when a ride is set up, and again between two ticks when the ride's pause
//! closes on a changed set ([`Mods::rebike`], [`Mods::replane`], P98) — never inside the simulation:
//! no step of the ride has a branch for them, so a modified ride is the original's code on other
//! numbers.
//!
//! **At 100% a modifier touches nothing.** Each one is skipped outright at its default rather than
//! multiplied by one, so a ride whose modifiers are all at their defaults ([`Mods::is_original`])
//! loads bit for bit the parameters it did before modifiers existed, and every replay and
//! acceptance test rides unchanged.
//!
//! Some are a player's own (one player's bike faster than the rest), [`Knob::per_seat`]; the rest are
//! the world's (gravity, air drag, time), held once for everyone in seat 0's slot.
//!
//! P95 split acceleration from top speed: [`Knob::BikeAccel`] beside the bike's two, and the
//! biplane's one speed knob into [`Knob::PlaneAccel`] and [`Knob::PlaneTopSpeed`]. The old
//! [`Knob::PlaneSpeed`] stays, off the menu ([`Knob::MENU`]), so a preset or a tape that carries
//! `plane_speed` still loads and rides the numbers it was made with.
//!
//! P96 made the flips' knob a speed: [`Knob::Rotation`] turns every rotation in the air — the
//! backflip, flips and 360s off the stick, and any rotation the port adds — `k` times as fast, through
//! the one factor the air code reads ([`crate::air::RotationSpeed`]). P55's [`Knob::SpinRate`], which only
//! scaled the backflip's torques, stays off the menu so a preset or a tape with `spin_rate` still
//! rides as it did.
//!
//! P137's [`Knob::PlaneMaxG`] is the one exception to both: its value is a limit in g ([`MAX_G`])
//! or off, and on, the flight runs a step of its own ([`crate::flight::Flight::max_g`]) that holds
//! the elevator to the pull the limit allows and eases the ailerons above the corner speed. Off, the
//! default, the flight never enters it.
//!
//! The text form ([`Mods::text`], [`Mods::parse`]) is what a tape, a preset and the online lobby
//! carry: `word=percent` for a world modifier, `word.N=percent` for player N's, only the ones off
//! their default. An empty text is the original game.
#![deny(clippy::float_arithmetic)]

use crate::MAX_PLAYERS;
use crate::air;
use crate::phy::{self, Phy};
use crate::r#move as mv;
use crate::off_map;
use crate::ride::Ride;
use ps2_float::F;
use std::sync::Arc;

/// One modifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Knob {
    BikeSpeed,
    BikeAccel,
    BikePower,
    Grip,
    Suspension,
    AirControl,
    SpinRate,
    Rotation,
    Jump,
    CrashTolerance,
    PlaneSpeed,
    PlaneAccel,
    PlaneTopSpeed,
    PlaneTurn,
    PlaneMaxG,
    Gravity,
    AirDrag,
    TimeScale,
}

/// How many modifiers there are.
pub const KNOBS: usize = 18;

/// A modifier's default, which leaves its fields untouched: 100%.
pub const ORIGINAL: u16 = 100;

/// The slider's step and its top, in percent.
pub const STEP: u16 = 10;
pub const MOST: u16 = 500;

/// [`Knob::PlaneMaxG`]'s limits, in whole g (P137): its value is the limit itself, and its default,
/// [`ORIGINAL`], is no limit at all.
pub const MAX_G: (u16, u16) = (3, 9);

impl Knob {
    pub const ALL: [Knob; KNOBS] = [
        Knob::BikeSpeed,
        Knob::BikeAccel,
        Knob::BikePower,
        Knob::Grip,
        Knob::Suspension,
        Knob::AirControl,
        Knob::SpinRate,
        Knob::Rotation,
        Knob::Jump,
        Knob::CrashTolerance,
        Knob::PlaneSpeed,
        Knob::PlaneAccel,
        Knob::PlaneTopSpeed,
        Knob::PlaneTurn,
        Knob::PlaneMaxG,
        Knob::Gravity,
        Knob::AirDrag,
        Knob::TimeScale,
    ];

    /// The ones the menu has a row for, in its order: all but [`Knob::PlaneSpeed`], which P95 split
    /// into the biplane's acceleration and top speed, and [`Knob::SpinRate`], which P96's
    /// [`Knob::Rotation`] took over — only an older preset or tape still sets either.
    pub const MENU: [Knob; KNOBS - 2] = [
        Knob::BikeSpeed,
        Knob::BikeAccel,
        Knob::BikePower,
        Knob::Grip,
        Knob::Suspension,
        Knob::AirControl,
        Knob::Rotation,
        Knob::Jump,
        Knob::CrashTolerance,
        Knob::PlaneAccel,
        Knob::PlaneTopSpeed,
        Knob::PlaneTurn,
        Knob::PlaneMaxG,
        Knob::Gravity,
        Knob::AirDrag,
        Knob::TimeScale,
    ];

    fn index(self) -> usize {
        Knob::ALL.iter().position(|k| *k == self).expect("every knob is in ALL")
    }

    /// The text form's word.
    pub fn word(self) -> &'static str {
        match self {
            Knob::BikeSpeed => "bike_speed",
            Knob::BikeAccel => "bike_accel",
            Knob::BikePower => "bike_power",
            Knob::Grip => "grip",
            Knob::Suspension => "suspension",
            Knob::AirControl => "air_control",
            Knob::SpinRate => "spin_rate",
            Knob::Rotation => "rotation_speed",
            Knob::Jump => "jump",
            Knob::CrashTolerance => "crash_tolerance",
            Knob::PlaneSpeed => "plane_speed",
            Knob::PlaneAccel => "plane_accel",
            Knob::PlaneTopSpeed => "plane_top_speed",
            Knob::PlaneTurn => "plane_turn",
            Knob::PlaneMaxG => "plane_max_g",
            Knob::Gravity => "gravity",
            Knob::AirDrag => "air_drag",
            Knob::TimeScale => "time_scale",
        }
    }

    pub fn from_word(w: &str) -> Option<Knob> {
        Knob::ALL.into_iter().find(|k| k.word() == w)
    }

    /// The menu's name for it.
    pub fn label(self) -> &'static str {
        match self {
            Knob::BikeSpeed => "Bike top speed",
            Knob::BikeAccel => "Bike acceleration",
            Knob::BikePower => "Engine power",
            Knob::Grip => "Tyre grip",
            Knob::Suspension => "Suspension",
            Knob::AirControl => "Air control",
            Knob::SpinRate => "Flip spin",
            Knob::Rotation => "Flip rotation speed",
            Knob::Jump => "Jump strength",
            Knob::CrashTolerance => "Crash tolerance",
            Knob::PlaneSpeed => "Biplane speed",
            Knob::PlaneAccel => "Biplane acceleration",
            Knob::PlaneTopSpeed => "Biplane top speed",
            Knob::PlaneTurn => "Biplane turn speed",
            Knob::PlaneMaxG => "Biplane max G",
            Knob::Gravity => "Gravity",
            Knob::AirDrag => "Air drag",
            Knob::TimeScale => "Time scale",
        }
    }

    /// Whether each player has their own, or there is one for the world.
    pub fn per_seat(self) -> bool {
        !matches!(self, Knob::Gravity | Knob::AirDrag | Knob::TimeScale)
    }

    /// The lowest percentage the slider goes to: nothing at all, where nothing is a sensible world
    /// (no gravity, no drag), a tenth otherwise. [`Knob::PlaneMaxG`] is in g, not percent: its least
    /// limit.
    pub fn least(self) -> u16 {
        match self {
            Knob::Gravity | Knob::AirDrag => 0,
            Knob::PlaneMaxG => MAX_G.0,
            _ => STEP,
        }
    }

    /// The highest the slider goes to: [`MOST`] percent, or for [`Knob::PlaneMaxG`] its highest limit
    /// in g. Above that it is off ([`ORIGINAL`]), the original's plane with no limit at all.
    pub fn most(self) -> u16 {
        match self {
            Knob::PlaneMaxG => MAX_G.1,
            _ => MOST,
        }
    }

    /// One step of the slider from `v`, up or down. [`Knob::PlaneMaxG`] goes a g at a time round a
    /// ring, off past its highest limit: up from the top is off and up from off the lowest, down from
    /// off is the top and down from the lowest off (so a press from the default changes it).
    pub fn step(self, v: u16, up: bool) -> u16 {
        match (self, up) {
            (Knob::PlaneMaxG, true) if v == ORIGINAL => self.least(),
            (Knob::PlaneMaxG, true) if v >= self.most() => ORIGINAL,
            (Knob::PlaneMaxG, true) => v + 1,
            (Knob::PlaneMaxG, false) if v == ORIGINAL => self.most(),
            (Knob::PlaneMaxG, false) if v <= self.least() => ORIGINAL,
            (Knob::PlaneMaxG, false) => v - 1,
            (_, true) => (v + STEP).min(MOST),
            (_, false) => v.saturating_sub(STEP).max(self.least()),
        }
    }

    /// The value as the menu shows it: `150%`, or for [`Knob::PlaneMaxG`] `6 g` or `Off`.
    pub fn shown(self, v: u16) -> String {
        match self {
            Knob::PlaneMaxG if v == ORIGINAL => "Off".into(),
            Knob::PlaneMaxG => format!("{v} g"),
            _ => format!("{v}%"),
        }
    }

    /// What it does, short enough for the menu's ticker.
    pub fn blurb(self) -> &'static str {
        match self {
            Knob::BikeSpeed => "the top speed, not the pull off the line",
            Knob::BikeAccel => "how fast the bike gets up to speed, not its top",
            Knob::BikePower => "engine torque: quicker off the line, and faster",
            Knob::Grip => "every tire's grip on every surface",
            Knob::Suspension => "spring stiffness",
            Knob::AirControl => "lean and turn in the air",
            Knob::SpinRate => "flip torque",
            Knob::Rotation => "how fast every flip and spin turns",
            Knob::Jump => "preload, the jump off the lip",
            Knob::CrashTolerance => "the speeds a landing wrecks at",
            Knob::PlaneSpeed => "biplane thrust and top speed",
            Knob::PlaneAccel => "the biplane's pick-up and braking, not its top",
            Knob::PlaneTopSpeed => "the biplane's top speed",
            Knob::PlaneTurn => "how hard the biplane pulls round a turn",
            Knob::PlaneMaxG => "the most g a pull takes a fast biplane to, and its roll eased with it",
            Knob::Gravity => "gravity, bike and biplane",
            Knob::AirDrag => "air drag, bike and biplane",
            Knob::TimeScale => "how fast the game runs",
        }
    }

    /// What it scales, in the files' own words: the journal's table and the menu's help line.
    pub fn fields(self) -> &'static str {
        match self {
            Knob::BikeSpeed => {
                "[Engine] HP curve (torque), ClutchStatic/KineticFriction, EngineBraking x k; FinalGearRatio / k; [Aerodynamics] Drag, every tire surface's WheelDrag / k^2; ReverseSpeed, MaxClutchTorqueScaleSpeed x k; the integrator's speed limit x k"
            }
            Knob::BikeAccel => "[Engine] HP curve (torque), ClutchStatic/KineticFriction x k; [Aerodynamics] Drag, every tire surface's WheelDrag, WheelFriction x k; above 100%, the rear tire's MaxDriveForce, SkidDriveForce x k",
            Knob::BikePower => "[Engine] HP curve (torque), ClutchStatic/KineticFriction x k; above 100%, the rear tire's MaxDriveForce, SkidDriveForce and the integrator's speed limit x k",
            Knob::Grip => "every tire surface's SideForce, SlippingSideForce, MaxDriveForce, MaxSideForce, MaxSlippingSideForce, Skid*Force x k",
            Knob::Suspension => "[Front/Rear Tire] Spring, SpringWhenStopped x k",
            Knob::AirControl => "[Airborne Control] AirborneControlMoment, AirborneControlForce x k",
            Knob::SpinRate => "[Backflip] Min/MaxBackflipTorque, BackflipTorqueRateIncrease/Decrease x k",
            Knob::Rotation => {
                "every rotation's torques ([Backflip] Min/MaxBackflipTorque) x k^2 and their rates (BackflipTorqueRateIncrease/Decrease) x k^3; [Airborne Control] AirborneControlMoment, BrakeGyroTorque x k^2, AirborneDamping, UnrollGain/Torque, UnyawGain/Torque x k; the control-authority time / k; the chassis' angular windows (YawRateLimit) x k"
            }
            Knob::Jump => "[Preload] MaxPreload x k",
            Knob::CrashTolerance => "[Wrecks] Front/Back/Side/RacingDown/FreestyleDown/NoHander WreckSpeedLimit x k",
            Knob::PlaneSpeed => "biplane [Engine] MaxPower, MinPropEfficiencySpeed x k; [Aerodynamics] CD0 / k^2",
            Knob::PlaneAccel => "biplane [Engine] MaxPower x k; [Aerodynamics] CD0, CD1, CDde x k; [Joystick] ThrottleIncrement x k; every tire's MaxBrakingForce x k (above 100%, the extra through the centre of gravity)",
            Knob::PlaneTopSpeed => "biplane [Aerodynamics] CD0 / k^3",
            Knob::PlaneTurn => "biplane [Aerodynamics] CLScale, CMq x k; CMalpha, CMde, CMStall x k^2; CD1 / k^2; [Autopilot] MaxPitchRate x k",
            Knob::PlaneMaxG => "not a key: each step the elevator command held to the angle of attack whose lift is k times the weight, and the ailerons x corner speed / speed above the corner speed",
            Knob::Gravity => "bike [Mass Properties] Gravity (the gravity force), biplane GravityScale x k",
            Knob::AirDrag => "bike [Aerodynamics] Drag, biplane CD0 x k",
            Knob::TimeScale => "the engine's time-scale object: dt + (k - 1) dt",
        }
    }
}

/// Every modifier, for every player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Mods {
    /// Percentages, a row a player; a world modifier lives in row 0.
    pct: [[u16; KNOBS]; MAX_PLAYERS],
}

impl Default for Mods {
    fn default() -> Mods {
        Mods { pct: [[ORIGINAL; KNOBS]; MAX_PLAYERS] }
    }
}

impl Mods {
    /// The original game: every modifier at its default.
    pub fn original() -> Mods {
        Mods::default()
    }

    /// The row a knob's value lives in for `seat`.
    fn row(knob: Knob, seat: usize) -> usize {
        if knob.per_seat() { seat.min(MAX_PLAYERS - 1) } else { 0 }
    }

    /// Player `seat`'s value of `knob`, in percent (the world's, for a world modifier).
    pub fn get(&self, seat: usize, knob: Knob) -> u16 {
        self.pct[Mods::row(knob, seat)][knob.index()]
    }

    /// Set it, clamped to the slider's range (a max G's off, [`ORIGINAL`], kept as it is).
    pub fn set(&mut self, seat: usize, knob: Knob, pct: u16) {
        let pct = if pct == ORIGINAL { pct } else { pct.clamp(knob.least(), knob.most()) };
        self.pct[Mods::row(knob, seat)][knob.index()] = pct;
    }

    /// Nothing off its default: the original game.
    pub fn is_original(&self) -> bool {
        *self == Mods::original()
    }

    /// Nothing off its default that reaches player `seat`'s ride: their own, and the world's.
    pub fn seat_is_original(&self, seat: usize) -> bool {
        Knob::ALL.iter().all(|&k| self.get(seat, k) == ORIGINAL)
    }

    /// The text form: the modifiers off their default, `word=pct` and `word.N=pct` (N from 1),
    /// space-separated, world first. A player's modifier every player has alike is one `word=pct`.
    /// Empty for the original game.
    pub fn text(&self) -> String {
        let mut out = Vec::new();
        let alike = |k: Knob| (1..MAX_PLAYERS).all(|s| self.get(s, k) == self.get(0, k));
        for k in Knob::ALL.into_iter().filter(|&k| !k.per_seat() || alike(k)) {
            if self.get(0, k) != ORIGINAL {
                out.push(format!("{}={}", k.word(), self.get(0, k)));
            }
        }
        for seat in 0..MAX_PLAYERS {
            for k in Knob::ALL.into_iter().filter(|&k| k.per_seat() && !alike(k)) {
                if self.get(seat, k) != ORIGINAL {
                    out.push(format!("{}.{}={}", k.word(), seat + 1, self.get(seat, k)));
                }
            }
        }
        out.join(" ")
    }

    /// [`Mods::text`] back. A pair it cannot read is an error naming it, so a tape or a preset never
    /// quietly rides different numbers from the ones it was made with.
    pub fn parse(text: &str) -> Result<Mods, String> {
        let mut m = Mods::original();
        for pair in text.split_whitespace() {
            let bad = || format!("modifier '{pair}' is not word=percent or word.player=percent");
            let (key, value) = pair.split_once('=').ok_or_else(bad)?;
            let pct: u16 = value.parse().map_err(|_| bad())?;
            let (word, seat) = match key.split_once('.') {
                Some((w, n)) => {
                    let n: usize = n.parse().map_err(|_| bad())?;
                    if n == 0 || n > MAX_PLAYERS {
                        return Err(format!("modifier '{pair}': players are 1 to {MAX_PLAYERS}"));
                    }
                    (w, Some(n - 1))
                }
                None => (key, None),
            };
            let knob = Knob::from_word(word).ok_or_else(|| format!("no modifier is called '{word}'"))?;
            if pct != ORIGINAL && (pct < knob.least() || pct > knob.most()) {
                return Err(format!("modifier '{pair}': {} to {}", knob.least(), knob.most()));
            }
            match (knob.per_seat(), seat) {
                (true, Some(s)) => m.set(s, knob, pct),
                // A player's modifier without a player is every player's.
                (true, None) => (0..MAX_PLAYERS).for_each(|s| m.set(s, knob, pct)),
                (false, None) => m.set(0, knob, pct),
                (false, Some(_)) => return Err(format!("modifier '{pair}': {word} is the world's, not a player's")),
            }
        }
        Ok(m)
    }

    /// Player `seat`'s biplane g limit in g, or `None` with it off (P137).
    pub fn max_g(&self, seat: usize) -> Option<F> {
        match self.get(seat, Knob::PlaneMaxG) {
            ORIGINAL => None,
            g => Some(F::from(g as i32)),
        }
    }

    /// The factor of a knob for `seat`, or `None` at its default — the case that must touch nothing.
    /// [`Knob::PlaneMaxG`] is no factor: [`Mods::max_g`] reads it.
    pub fn factor(&self, seat: usize, knob: Knob) -> Option<F> {
        match self.get(seat, knob) {
            ORIGINAL => None,
            pct => Some(F::from(pct as i32) / F(100.0)),
        }
    }

    /// Player `seat`'s bike: their own modifiers and the world's, on the loaded file.
    pub fn bike(&self, seat: usize, phy: &mut Phy) {
        if let Some(k) = self.factor(seat, Knob::BikePower) {
            // k times the torque, on the file's gearing. The disc's bikes spin the rear tire off the
            // line on the file's own torque (P97: race125p's launch is held by the tire, not the
            // engine), so more torque alone only spins it harder — 200% was 10% quicker to 40 ft/s
            // and 500% no quicker than 200%. The driven tire's grip along itself goes up with the
            // torque, so the extra reaches the ground; below 100% the tire already passes all of
            // the smaller torque and keeps the file's grip.
            engine_power(phy, k);
            if k > F::ONE {
                drive_grip(phy, k);
            }
        }
        if let Some(k) = self.factor(seat, Knob::BikeAccel) {
            // k times the torque and k times everything that holds the bike back on the flat: the
            // air's drag, the tires' rolling drag and their wheel friction. The force left over to
            // speed the bike up is k times the file's at every speed below the top, and the top,
            // where the drive meets the drag, is where it was (to the tires' slip: within 2%). The
            // stock bike tops out on the drag, well under its rev limit in top gear, so the gearing
            // is left alone. Above 100% the driven tire's grip along itself goes up with the torque,
            // as for the engine's power: the file's bike is already spinning its tire off the line.
            engine_power(phy, k);
            if k > F::ONE {
                drive_grip(phy, k);
            }
            phy.mv.drag = phy.mv.drag * k;
            for tire in [&mut phy.front, &mut phy.rear] {
                for s in &mut tire.surfaces {
                    s.wheel_drag = s.wheel_drag * k;
                    s.wheel_friction = s.wheel_friction * k;
                }
            }
        }
        if let Some(k) = self.factor(seat, Knob::BikeSpeed) {
            // The torque and the gearing move together, so the force at the wheel at any revs is
            // the file's and the wheel turns k times as fast for them; the drag is taken down so that
            // k times the speed meets the file's drag at the file's speed — the air's, and the v²
            // rolling drag some surfaces give the tires (race125p's rear on surface 2). The top
            // speed is then k times the file's, by the gearing and by the drag alike, and the pull
            // off the line, where neither drag is anything yet, is the file's.
            engine_power(phy, k);
            phy.engine.engine_braking = phy.engine.engine_braking * k;
            phy.engine.final_drive = phy.engine.final_drive / k;
            let gear = phy.engine.gear.max(1);
            mv::set_gear(&mut phy.engine, gear);
            phy.mv.drag = phy.mv.drag / (k * k);
            for tire in [&mut phy.front, &mut phy.rear] {
                for s in &mut tire.surfaces {
                    s.wheel_drag = s.wheel_drag / (k * k);
                }
            }
            phy.mv.reverse_speed = phy.mv.reverse_speed * k;
            phy.mv.max_clutch_torque_scale_speed = phy.mv.max_clutch_torque_scale_speed * k;
        }
        if let Some(k) = self.factor(seat, Knob::Grip) {
            for tire in [&mut phy.front, &mut phy.rear] {
                for s in &mut tire.surfaces {
                    for f in [
                        &mut s.side_force,
                        &mut s.slipping_side_force,
                        &mut s.max_drive_force,
                        &mut s.max_side_force,
                        &mut s.max_slipping_side_force,
                        &mut s.skid_drive_force,
                        &mut s.skid_side_force,
                        &mut s.skid_slipping_side_force,
                    ] {
                        *f = *f * k;
                    }
                }
            }
        }
        if let Some(k) = self.factor(seat, Knob::Suspension) {
            for tire in [&mut phy.front, &mut phy.rear] {
                tire.spring = tire.spring * k;
                tire.spring_when_stopped = tire.spring_when_stopped * k;
            }
            for f in [
                &mut phy.mv.front_spring,
                &mut phy.mv.front_spring_when_stopped,
                &mut phy.mv.rear_spring,
                &mut phy.mv.rear_spring_when_stopped,
            ] {
                *f = *f * k;
            }
        }
        if let Some(k) = self.factor(seat, Knob::AirControl) {
            phy.air.airborne_control_moment = phy.air.airborne_control_moment.mul_q(k.0);
            phy.air.airborne_control_force = phy.air.airborne_control_force * k;
        }
        if let Some(k) = self.factor(seat, Knob::SpinRate) {
            let a = &mut phy.air;
            for f in [
                &mut a.min_backflip_torque,
                &mut a.max_backflip_torque,
                &mut a.backflip_torque_rate_increase,
                &mut a.backflip_torque_rate_decrease,
            ] {
                *f = *f * k;
            }
        }
        if let Some(k) = self.factor(seat, Knob::Rotation) {
            // One factor for every rotation, read where the air code reads a rotation's numbers
            // (`air::RotationSpeed`): the rows' tuning, the stick's moments and the damping. Not a field a
            // trick at a time, so a rotation added later turns at this speed with nothing to add.
            air::RotationSpeed(k).params(&mut phy.air);
        }
        if let Some(k) = self.factor(seat, Knob::Jump) {
            phy.air.max_preload = phy.air.max_preload * k;
        }
        if let Some(k) = self.factor(seat, Knob::CrashTolerance) {
            let a = &mut phy.air;
            for f in [
                &mut a.front_wreck_speed_limit,
                &mut a.back_wreck_speed_limit,
                &mut a.side_wreck_speed_limit,
                &mut a.racing_down_wreck_speed_limit,
                &mut a.freestyle_down_wreck_speed_limit,
                &mut a.no_hander_wreck_speed_limit,
            ] {
                *f = *f * k;
            }
        }
        if let Some(k) = self.factor(seat, Knob::Gravity) {
            phy.mv.gravity = phy.mv.gravity.mul_q(k.0);
            // The `w` the loader writes is a literal 1, not a part of the force.
            phy.mv.gravity.0[3] = 1.0;
            phy.mass.gravity = phy.mass.gravity * k;
        }
        if let Some(k) = self.factor(seat, Knob::AirDrag) {
            phy.mv.drag = phy.mv.drag * k;
        }
    }

    /// Player `seat`'s ride once it is built: the integrator's speed limit, which is the ride's own
    /// constant rather than the file's, raised with [`Knob::BikeSpeed`] and [`Knob::BikePower`] above
    /// 100% (k² on the square, the product of the two when both are) so the limit is not what stops a
    /// faster bike. Engine power at 200% was held to it at 135 ft/s on the flat before P97. The
    /// put-back after a wreck sets the limit back to the ride's [`Ride::riding_speed_sq`], which is
    /// raised alike; before P97 it went back to the original's, and a faster bike was a 135 ft/s
    /// bike from its first wreck on. And its angular windows, the yaw's from the file's
    /// `YawRateLimit`, moved with [`Knob::Rotation`] so a 360 is not held to the original's rate.
    pub fn ride(&self, seat: usize, ride: &mut Ride) {
        ride.chassis.max_speed_sq = self.speed_limit(seat, ride.chassis.max_speed_sq);
        ride.riding_speed_sq = self.speed_limit(seat, ride.riding_speed_sq);
        self.windows(seat, &mut ride.chassis);
    }

    /// Player `seat`'s chassis' angular windows and angular speed limit, as a ride is built with
    /// them, moved with [`Knob::Rotation`] (P96): each window `k` times as wide, the squared limit
    /// `k^2`.
    fn windows(&self, seat: usize, c: &mut crate::chassis::Chassis) {
        if let Some(k) = self.factor(seat, Knob::Rotation) {
            let spin = air::RotationSpeed(k);
            c.max_ang_speed_sq = spin.rate(spin.rate(c.max_ang_speed_sq));
            for w in c.ang_vel_max.iter_mut().chain(c.ang_vel_min.iter_mut()) {
                *w = spin.rate(*w);
            }
        }
    }

    /// The integrator's squared speed limit [`Mods::ride`] leaves a ride built with `limit`.
    fn speed_limit(&self, seat: usize, limit: F) -> F {
        let mut raise = None;
        for knob in [Knob::BikeSpeed, Knob::BikePower] {
            if let Some(k) = self.factor(seat, knob)
                && k > F::ONE
            {
                raise = Some(raise.map_or(k, |r: F| r * k));
            }
        }
        match raise {
            Some(k) => limit * (k * k),
            None => limit,
        }
    }

    /// **Player `seat`'s ride, moved onto these modifiers while it goes on** (P98): the bike's file
    /// built again from its stock bytes `stock` with these on it ([`Mods::bike`]), handed to the ride
    /// as its new `.phy`, and what the ride copied out of the old one when it was built taken again
    /// ([`Ride::retune`]). `was` is the set the ride had: the speed limit [`Mods::ride`] raised for it
    /// is moved to this set's, unless a wreck holds its own limit just now; the put-back's riding
    /// limit ([`Ride::riding_speed_sq`]) moves either way, so the put-back after that wreck sets
    /// this set's (P101). The bike is not moved, not stopped and not put back: the next tick steps it
    /// from where it is on the new numbers.
    pub fn rebike(&self, was: &Mods, seat: usize, stock: &Phy, phy: &mut Arc<Phy>, ride: &mut Ride) {
        let mut file = stock.clone();
        self.bike(seat, &mut file);
        ride.retune(&file);
        let riding = off_map::RIDING_MAX_SPEED * off_map::RIDING_MAX_SPEED;
        if ride.chassis.max_speed_sq == was.speed_limit(seat, riding) {
            ride.chassis.max_speed_sq = self.speed_limit(seat, riding);
        }
        ride.riding_speed_sq = self.speed_limit(seat, riding);
        // The windows, when the rotation speed moved: the bike's as it is built, then moved again.
        if was.factor(seat, Knob::Rotation) != self.factor(seat, Knob::Rotation) {
            let c = &mut ride.chassis;
            (c.max_ang_speed_sq, c.ang_vel_max, c.ang_vel_min) = crate::ride::bike_ang_limits(&file);
            self.windows(seat, c);
        }
        *phy = Arc::new(file);
    }

    /// [`Mods::rebike`] on a one-player simulation's rider, as player `seat`.
    pub fn resim(&self, was: &Mods, seat: usize, stock: &Phy, sim: &mut crate::Sim) {
        self.rebike(was, seat, stock, &mut sim.phy, &mut sim.rider.ride);
    }

    /// Player `seat`'s biplane moved onto these modifiers in flight (P98): its stock file `stock`
    /// with these on it ([`Mods::plane`]), and the flight retuned to it where it is
    /// ([`crate::flight::Flight::retune`]).
    pub fn replane(&self, seat: usize, stock: &phy::Plane, flight: &mut crate::flight::Flight) {
        let mut plane = *stock;
        self.plane(seat, &mut plane);
        flight.retune(&plane);
    }

    /// Player `seat`'s biplane, on the loaded file, before its flight is built from it.
    pub fn plane(&self, seat: usize, plane: &mut phy::Plane) {
        if let Some(k) = self.factor(seat, Knob::PlaneSpeed) {
            // The same trade as the bike's: k times the power, and the drag down so that k times the
            // speed costs the file's drag — power is drag times speed, so k times the power holds it.
            plane.engine.max_power = plane.engine.max_power * k;
            plane.engine.min_prop_efficiency_speed = plane.engine.min_prop_efficiency_speed * k;
            plane.aero.cd0 = plane.aero.cd0 / (k * k);
        }
        if let Some(k) = self.factor(seat, Knob::PlaneAccel) {
            // k times the power and k times every drag coefficient: thrust less drag is k times the
            // file's at every speed, so the plane gets up to speed k times as hard and tops out where
            // the file's does.
            plane.engine.max_power = plane.engine.max_power * k;
            plane.aero.cd0 = plane.aero.cd0 * k;
            plane.aero.cd1 = plane.aero.cd1 * k;
            plane.aero.cd_de = plane.aero.cd_de * k;
            // And slows k times as hard (P116). Held back, the throttle closes at ThrottleIncrement a
            // second while the thrust it still gives fights the brakes, so on the runway the file's
            // brakes against k times the power stopped a 200% plane later than the file's; k times the
            // rate the throttle moves (both ways) and k times each wheel's braking force (its rolling
            // friction, BrakeFriction of it, with it) put every force and the throttle's clock on k,
            // and the run from top speed to a stop takes 1 / k the time as the run up to speed does.
            // Above 100% the extra braking goes through the centre of gravity: at the contacts, twice
            // the file's stands the plane on its nose, whose weight is still the file's.
            plane.joystick.throttle_increment = plane.joystick.throttle_increment * k;
            for g in &mut plane.gear {
                if k > F::ONE {
                    g.cg_braking = k - F::ONE;
                } else {
                    g.max_braking_force = g.max_braking_force * k;
                }
            }
        }
        if let Some(k) = self.factor(seat, Knob::PlaneTopSpeed) {
            // The power is the drag times the speed, and the drag at a speed goes with its square, so
            // the speed the file's power holds goes with the cube root of 1 / CD0: CD0 / k^3 tops out
            // at k times the speed. Low down the drag is small beside the thrust, so the getting
            // there is near enough the file's.
            plane.aero.cd0 = plane.aero.cd0 / (k * k * k);
        }
        if let Some(k) = self.factor(seat, Knob::PlaneTurn) {
            // The turn is the lift: with the stick held the elevator sits at its stop, which trims the
            // wing to the top of its lift curve, so the plane turns as hard as CLScale lets it — k
            // times the lift is k times the pull at every speed. The pitch has to keep up: the moments
            // the wing is held at its angle by (CMalpha against CMde, and the stall's break) x k^2
            // pitch it k times as fast to the same angle, the damping CMq x k keeps it as damped, and
            // the autopilot's pitch-rate target (MaxPitchRate, which the round stick's 2.5 multiplies)
            // asks for k times the rate. CD1 / k^2 leaves the drag of the harder pull the file's.
            // The trim (the angle each elevator holds) is unchanged, so level flight and the banked
            // hold fly as the file's do.
            plane.aero.cl_scale = plane.aero.cl_scale * k;
            plane.aero.cm_q = plane.aero.cm_q * k;
            plane.aero.cm_alpha = plane.aero.cm_alpha * (k * k);
            plane.aero.cm_de = plane.aero.cm_de * (k * k);
            plane.aero.cm_stall = plane.aero.cm_stall * (k * k);
            plane.aero.cd1 = plane.aero.cd1 / (k * k);
            plane.autopilot.max_pitch_rate = plane.autopilot.max_pitch_rate * k;
        }
        if let Some(g) = self.max_g(seat) {
            // Not a number of the file's: the limit the flight's hold keeps every step's pull under
            // (P137, [`crate::flight::Flight::max_g`]). The file's lift and moments are left alone,
            // so below the corner speed the limit implies the plane flies the file's numbers. Above
            // it the roll is eased, but never past where the sped-up plane rolls as the file's does
            // at the file's top speed: so many times the top speed (the speed knobs' k, and the air
            // drag's k^-1/3, as their notes have it) is 1 / that of the aileron at most.
            plane.autopilot.max_g = g;
            let mut faster = F::ONE;
            for knob in [Knob::PlaneSpeed, Knob::PlaneTopSpeed] {
                if let Some(k) = self.factor(seat, knob) {
                    faster = faster * k;
                }
            }
            if let Some(k) = self.factor(seat, Knob::AirDrag) {
                faster = faster / F(ps2_float::libm::powf(k.to_f32(), (F::ONE / F(3.0)).to_f32()));
            }
            plane.autopilot.max_g_roll = (F::ONE / faster).min(F::ONE);
        }
        if let Some(k) = self.factor(seat, Knob::Gravity) {
            plane.mass_properties.gravity_scale = plane.mass_properties.gravity_scale * k;
        }
        if let Some(k) = self.factor(seat, Knob::AirDrag) {
            plane.aero.cd0 = plane.aero.cd0 * k;
        }
    }

    /// The engine's time-scale factor (`dt += factor × dt`): `None`, the original's normal case,
    /// unless the time scale is off its default.
    pub fn time_scale(&self) -> Option<F> {
        self.factor(0, Knob::TimeScale).map(|k| k - F::ONE)
    }
}

/// The power curve's torque and the clutch that has to pass it, `k` times.
fn engine_power(phy: &mut Phy, k: F) {
    for t in &mut phy.engine.curve_torque {
        *t = *t * k;
    }
    phy.peak.torque = phy.peak.torque * k;
    phy.engine.clutch_static = phy.engine.clutch_static * k;
    phy.engine.clutch_kinetic = phy.engine.clutch_kinetic * k;
}

/// The driven tire's grip along itself, `k` times: the force it passes before it spins, and the
/// force it drives with once it does.
fn drive_grip(phy: &mut Phy, k: F) {
    for s in &mut phy.rear.surfaces {
        s.max_drive_force = s.max_drive_force * k;
        s.skid_drive_force = s.skid_drive_force * k;
    }
}

/// The step the engine's time-scale object adds to a frame's `dt`, as the fixed update does it
/// ([`crate::tick::Engine::step`]): unscaled before the first positive frame, and never below the
/// floor. Online play's ticks take their `dt` off a schedule rather than an engine, and are scaled
/// with this.
pub fn scaled_dt(dt: F, scale: Option<F>) -> F {
    let Some(scale) = scale else { return dt };
    if dt > F::ZERO {
        let dt = dt + scale * dt;
        if dt < F(crate::tick::DeltaTime::FLOOR) { F(crate::tick::DeltaTime::FLOOR) } else { dt }
    } else {
        dt
    }
}
