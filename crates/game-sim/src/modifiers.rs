//! **Game modifiers** (P55): multipliers on numbers the ride already reads, set before it loads.
//!
//! Every modifier is a percentage on one or more fields the loaders fill — the bike's `.phy`
//! ([`Phy`]), the biplane's ([`phy::Plane`]), the integrator's speed limit the ride is built with
//! ([`Ride`]) and the engine's time-scale object ([`crate::tick::Engine::time_scale`]). They are
//! applied to those values once, when a ride is set up, and never inside the simulation: no step
//! of the ride has a branch for them, so a modified ride is the original's code on other numbers.
//!
//! **At 100% a modifier touches nothing.** Each one is skipped outright at its default rather than
//! multiplied by one, so a ride whose modifiers are all at their defaults ([`Mods::is_original`])
//! loads bit for bit the parameters it did before modifiers existed, and every replay and
//! acceptance test rides unchanged.
//!
//! Some are a player's own (one player's bike faster than the rest), [`Knob::per_seat`]; the rest are
//! the world's (gravity, air drag, time), held once for everyone in seat 0's slot.
//!
//! The text form ([`Mods::text`], [`Mods::parse`]) is what a tape, a preset and the online lobby
//! carry: `word=percent` for a world modifier, `word.N=percent` for player N's, only the ones off
//! their default. An empty text is the original game.
#![deny(clippy::float_arithmetic)]

use crate::MAX_PLAYERS;
use crate::phy::{self, Phy};
use crate::r#move as mv;
use crate::ride::Ride;
use ps2_float::F;

/// One modifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Knob {
    BikeSpeed,
    BikePower,
    Grip,
    Suspension,
    AirControl,
    SpinRate,
    Jump,
    CrashTolerance,
    PlaneSpeed,
    Gravity,
    AirDrag,
    TimeScale,
}

/// How many modifiers there are.
pub const KNOBS: usize = 12;

/// A modifier's default, which leaves its fields untouched: 100%.
pub const ORIGINAL: u16 = 100;

/// The slider's step and its top, in percent.
pub const STEP: u16 = 10;
pub const MOST: u16 = 500;

impl Knob {
    pub const ALL: [Knob; KNOBS] = [
        Knob::BikeSpeed,
        Knob::BikePower,
        Knob::Grip,
        Knob::Suspension,
        Knob::AirControl,
        Knob::SpinRate,
        Knob::Jump,
        Knob::CrashTolerance,
        Knob::PlaneSpeed,
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
            Knob::BikePower => "bike_power",
            Knob::Grip => "grip",
            Knob::Suspension => "suspension",
            Knob::AirControl => "air_control",
            Knob::SpinRate => "spin_rate",
            Knob::Jump => "jump",
            Knob::CrashTolerance => "crash_tolerance",
            Knob::PlaneSpeed => "plane_speed",
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
            Knob::BikePower => "Engine power",
            Knob::Grip => "Tyre grip",
            Knob::Suspension => "Suspension",
            Knob::AirControl => "Air control",
            Knob::SpinRate => "Flip spin",
            Knob::Jump => "Jump strength",
            Knob::CrashTolerance => "Crash tolerance",
            Knob::PlaneSpeed => "Biplane speed",
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
    /// (no gravity, no drag), a tenth otherwise.
    pub fn least(self) -> u16 {
        match self {
            Knob::Gravity | Knob::AirDrag => 0,
            _ => STEP,
        }
    }

    /// What it does, short enough for the menu's ticker.
    pub fn blurb(self) -> &'static str {
        match self {
            Knob::BikeSpeed => "engine power and top speed",
            Knob::BikePower => "engine torque, top speed as geared",
            Knob::Grip => "every tire's grip on every surface",
            Knob::Suspension => "spring stiffness",
            Knob::AirControl => "lean and turn in the air",
            Knob::SpinRate => "flip torque",
            Knob::Jump => "preload, the jump off the lip",
            Knob::CrashTolerance => "the speeds a landing wrecks at",
            Knob::PlaneSpeed => "biplane thrust and top speed",
            Knob::Gravity => "gravity, bike and biplane",
            Knob::AirDrag => "air drag, bike and biplane",
            Knob::TimeScale => "how fast the game runs",
        }
    }

    /// What it scales, in the files' own words: the journal's table and the menu's help line.
    pub fn fields(self) -> &'static str {
        match self {
            Knob::BikeSpeed => {
                "[Engine] HP curve (torque), ClutchStatic/KineticFriction, EngineBraking x k; FinalGearRatio / k; [Aerodynamics] Drag / k^2; ReverseSpeed, MaxClutchTorqueScaleSpeed x k; the integrator's speed limit x k"
            }
            Knob::BikePower => "[Engine] HP curve (torque), ClutchStatic/KineticFriction x k",
            Knob::Grip => "every tire surface's SideForce, SlippingSideForce, MaxDriveForce, MaxSideForce, MaxSlippingSideForce, Skid*Force x k",
            Knob::Suspension => "[Front/Rear Tire] Spring, SpringWhenStopped x k",
            Knob::AirControl => "[Airborne Control] AirborneControlMoment, AirborneControlForce x k",
            Knob::SpinRate => "[Backflip] Min/MaxBackflipTorque, BackflipTorqueRateIncrease/Decrease x k",
            Knob::Jump => "[Preload] MaxPreload x k",
            Knob::CrashTolerance => "[Wrecks] Front/Back/Side/RacingDown/FreestyleDown/NoHander WreckSpeedLimit x k",
            Knob::PlaneSpeed => "biplane [Engine] MaxPower, MinPropEfficiencySpeed x k; [Aerodynamics] CD0 / k^2",
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

    /// Set it, clamped to the slider's range.
    pub fn set(&mut self, seat: usize, knob: Knob, pct: u16) {
        self.pct[Mods::row(knob, seat)][knob.index()] = pct.clamp(knob.least(), MOST);
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
            if pct < knob.least() || pct > MOST {
                return Err(format!("modifier '{pair}': {} to {MOST}", knob.least()));
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

    /// The factor of a knob for `seat`, or `None` at its default — the case that must touch nothing.
    pub fn factor(&self, seat: usize, knob: Knob) -> Option<F> {
        match self.get(seat, knob) {
            ORIGINAL => None,
            pct => Some(F::from(pct as i32) / F(100.0)),
        }
    }

    /// Player `seat`'s bike: their own modifiers and the world's, on the loaded file.
    pub fn bike(&self, seat: usize, phy: &mut Phy) {
        if let Some(k) = self.factor(seat, Knob::BikePower) {
            engine_power(phy, k);
        }
        if let Some(k) = self.factor(seat, Knob::BikeSpeed) {
            // The torque and the gearing move together, so the force at the wheel at any revs is
            // the file's and the wheel turns k times as fast for them; the drag is taken down so that
            // k times the speed meets the file's drag at the file's speed. The top speed is then k
            // times the file's, by the gearing and by the drag alike.
            engine_power(phy, k);
            phy.engine.engine_braking = phy.engine.engine_braking * k;
            phy.engine.final_drive = phy.engine.final_drive / k;
            let gear = phy.engine.gear.max(1);
            mv::set_gear(&mut phy.engine, gear);
            phy.mv.drag = phy.mv.drag / (k * k);
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
    /// constant rather than the file's, raised with [`Knob::BikeSpeed`] so the limit is not what
    /// stops a faster bike.
    pub fn ride(&self, seat: usize, ride: &mut Ride) {
        if let Some(k) = self.factor(seat, Knob::BikeSpeed)
            && k > F::ONE
        {
            ride.chassis.max_speed_sq = ride.chassis.max_speed_sq * (k * k);
        }
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
