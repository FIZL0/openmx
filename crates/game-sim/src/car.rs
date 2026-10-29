//! The four-wheeled machines' physics file: the monster truck, the buggy and the baja, which the
//! freeride tracks park beside the bike as they park the biplane (free09 the truck, free07 the baja,
//! free11 the buggy).
//!
//! All three are one class in the original, and one loader reads all three files. It reads a
//! `[Mass Properties]` section through the shared rigid-body part reader ([`Part::read`]); four
//! tires, each with up to sixteen tire-model surfaces; an engine with its torque table and gearbox;
//! the airborne controls and wings; the preload jump; the wreck time; and the pad's rate limits and
//! steer tables. Each machine's own class then reads its `[Animation]` box for the driver's pelvis,
//! and the truck alone reads a rear-tire steer scale.
//!
//! What the loader works out on the way is here, so a stepper reads the same words the original's
//! step does:
//!
//! - Each tire's spring becomes a force per foot against the car's **collision** mass
//!   ([`spring_rate`]), and both of its dampers become rates against that spring ([`damping`]). Its
//!   radius is kept with its square and reciprocal, its inertia as a reciprocal, its steer lock in
//!   radians. The surfaces keep `A2` and `A4` as reciprocals, because the tire model divides by them.
//! - The engine's horsepower curve becomes a torque curve, `5252 * HP / RPM` plus the engine
//!   braking's `k * RPM * RPM`. The gearbox starts in first, and first's overall ratio and the
//!   table's last RPM give each driven tire the wheel speed it tops out at.
//! - Speeds in the file are miles per hour ([`MPH`]); angles are degrees ([`DEG`]).
//!
//! Never read by this loader: `[Skidding Limits]`, `[Camera Properties]`, `VelocityFilter` (read into
//! a temporary that nothing uses), `ImpactVelocityDamping`/`ImpactAngularRateDamping`, and
//! `ClutchCofGTravelBack`.
//!
//! `tests/car_phy.rs` proves it against the monster truck the human's freeride session left parked.

#![deny(clippy::float_arithmetic)]

use crate::air::AcosTable;
use crate::camera::CurveTable;
use crate::phy::{DEG, Ini, MAX_SURFACES, MPH, Part, Section, damping, spring_rate};
use ps2_float::{Acc, Dest, F, Vf, fpu};

/// The three files in `machine.res`, as the vehicle table spells them.
pub const MONSTER_TRUCK_PHY: &str = "MonsterTruck\\monstertruck.phy";
pub const BUGGY_PHY: &str = "Buggy\\buggy.phy";
pub const BAJA_PHY: &str = "Baja\\baja.phy";

/// `(0, 0, 0, 1)`: the default of every quadword this loader reads.
const UNIT_W: Vf = Vf([0.0, 0.0, 0.0, 1.0]);
/// Horsepower times this over RPM is torque in foot-pounds.
const HP_TORQUE: f32 = 5252.0;
/// RPM to radians per second, as the original rounds `2*pi/60`.
const RPM_TO_RAD: f32 = 0.1047198;

/// Which of the three machines a file is for. The loader is shared; the derived classes differ only
/// in what they read after it, under `[Animation]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    MonsterTruck,
    Buggy,
    Baja,
}

impl Kind {
    /// The machine's file in `machine.res`.
    pub fn phy(self) -> &'static str {
        match self {
            Kind::MonsterTruck => MONSTER_TRUCK_PHY,
            Kind::Buggy => BUGGY_PHY,
            Kind::Baja => BAJA_PHY,
        }
    }
}

/// The tire sections in the loader's order, which is also the order the engine's drive-train slots
/// name them in.
pub const TIRES: [&str; 4] = ["FrontLeftTire", "FrontRightTire", "RearLeftTire", "RearRightTire"];
pub const FRONT_LEFT: usize = 0;
pub const FRONT_RIGHT: usize = 1;
pub const REAR_LEFT: usize = 2;
pub const REAR_RIGHT: usize = 3;

/// A car's `.phy`, loaded.
#[derive(Clone, Debug, PartialEq)]
pub struct Car {
    /// `[Mass Properties]`, through the rigid-body part loader.
    pub mass_properties: Part,
    /// `CofGAI`, which defaults to the part's `CofG`: where the AI's car carries its mass.
    pub cofg_ai: Vf,
    /// `CofGTravelForward`, `CofGTravelBack` — how far the mass may slide under gas and brake.
    pub cofg_travel_forward: F,
    pub cofg_travel_back: F,
    /// `YawRateLimit`, and the handbrake's `HandBrakeYawRateLimit`, `HandBrakeFrictionScale`,
    /// `HandBrakeBrakeScale`, `HandBrakeFadeTime`.
    pub yaw_rate_limit: F,
    pub hand_brake_yaw_rate_limit: F,
    pub hand_brake_friction_scale: F,
    pub hand_brake_brake_scale: F,
    pub hand_brake_fade_time: F,
    /// The four tires, in [`TIRES`] order.
    pub tires: [Tire; 4],
    pub engine: Engine,
    /// The clutch's launch boost (under `[Engine]`), read with the getter that leaves a missing key
    /// alone: `None` keeps whatever the car was built with.
    pub clutch_torque_scale_time: Option<F>,
    pub max_clutch_torque_scale: Option<F>,
    pub max_clutch_torque_scale_speed: Option<F>,
    pub aero: Aero,
    /// `[Preload]`, the held-jump spring, read with the same leave-alone getter.
    pub preload: Preload,
    /// `[Wrecks]` `MaxWreckTime`, default 5.
    pub max_wreck_time: F,
    pub joystick: Joystick,
    pub animation: Animation,
}

/// One tire section, and its surfaces.
#[derive(Clone, Debug, PartialEq)]
pub struct Tire {
    /// `MaxCompression`, `MaxCompressionRate`, `MaxDecompressionRate` (all default 1).
    pub max_compression: F,
    pub max_compression_rate: F,
    pub max_decompression_rate: F,
    /// `Spring` as a rate ([`spring_rate`] against the collision mass), and the two dampers
    /// (`BounceDampingRatio`, `ReboundDampingRatio`) as rates against it.
    pub spring: F,
    pub bounce_damping: F,
    pub rebound_damping: F,
    /// `TireRadius` (default 1), its square, and its reciprocal — which the setter only writes for a
    /// positive radius, leaving the tire's constructed value otherwise (`None`).
    pub radius: F,
    pub radius_squared: F,
    pub inv_radius: Option<F>,
    /// `TireWidth` (default 0). The setter also slides the tire's attachment point out by it, which
    /// is the model's business and not the file's.
    pub width: F,
    /// `1/TireInertia` (default 1; zero for an inertia that is not positive), and `GyroInertia`.
    pub inv_inertia: F,
    pub gyro_inertia: F,
    /// `MaxSteerAngle` in radians.
    pub max_steer_angle: F,
    /// `MaxTireAngleRate`, default `f32::MAX`.
    pub max_tire_angle_rate: F,
    /// `MaxTireScale`, `CamberRollRatio`.
    pub max_tire_scale: F,
    pub camber_roll_ratio: F,
    /// `ABSEnabled` as the integer word, and its slip window `MinBrakingSlip`, `MaxBrakingSlip`.
    pub abs_enabled: i32,
    pub min_braking_slip: F,
    pub max_braking_slip: F,
    /// `NumberOfSurfaces` as this tire saw it. The count lives in one variable across all four
    /// tires, so a tire without the key uses the previous tire's count (zero for the first).
    pub number_of_surfaces: i32,
    /// `<Tire> Surface 1` up to the count — but only the first [`MAX_SURFACES`] reach the tire.
    pub surfaces: Vec<Surface>,
}

/// One `<Tire> Surface N` section: the tire model's coefficients for one ground type. Every key
/// defaults to 0.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Surface {
    pub c1: F,
    pub c2: F,
    pub c3: F,
    pub c4: F,
    pub a0: F,
    pub a1: F,
    /// `1/A2` — the setter divides unguarded, so a missing `A2` is the EE's `1/0`.
    pub inv_a2: F,
    pub a3: F,
    /// `1/A4`, likewise.
    pub inv_a4: F,
    pub moment_scale: F,
    pub cornering_stiffness: F,
    pub max_normal_load: F,
    pub friction_peak: F,
    pub friction_end_point: F,
    pub max_braking_torque: F,
    pub wheel_friction: F,
}

/// `[Engine]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Engine {
    /// `DriveTrain`: 0 drives the front pair, 1 the rear pair, anything else all four.
    pub drive_train: i32,
    /// The four drive slots as the engine keeps them: tire indices into [`Car::tires`], the unused
    /// ones `None`.
    pub driven: [Option<usize>; 4],
    /// `RPM1..` and `HP1..`, `NumberOfRPMPoints` of each.
    pub rpm: Vec<F>,
    pub hp: Vec<F>,
    /// The torque curve the setter builds: `5252*HP/RPM + k*RPM*RPM` for a positive RPM, else 0.
    pub torque: Vec<F>,
    /// `EngineBraking * 1e-6`: the `k` above.
    pub engine_braking: F,
    /// The table's last RPM — the redline. `None` for an empty table, where the original reads the
    /// word before its allocation.
    pub max_rpm: Option<F>,
    /// `GearRatio1..` (`NumberOfGears` of them) and `FinalGearRatio`.
    pub gear_ratios: Vec<F>,
    pub final_gear_ratio: F,
    /// The gearbox put in first: `FinalGearRatio * GearRatio1` and its reciprocal, and the wheel
    /// speed the redline makes in it, which each driven tire keeps as its top speed. `None` with no
    /// gears.
    pub first: Option<FirstGear>,
    /// `EngineInertia`, and its reciprocal for a positive one (left alone otherwise).
    pub inertia: F,
    pub inv_inertia: Option<F>,
    /// `TransmissionLoss`, a percentage, as a fraction.
    pub transmission_loss: F,
    /// `MinRPM`, `IdleThrottle`.
    pub min_rpm: F,
    pub idle_throttle: F,
    /// `maxK`, `PumpTorqueScale * 0.001`, `MaxPumpTorque`: the torque converter.
    pub max_k: F,
    pub pump_torque_scale: F,
    pub max_pump_torque: F,
    /// The automatic gearbox's shift points.
    pub shift: Shift,
    /// `NOXVolume`, `NOXBurnRate`, `NOXHPBoost`.
    pub nox_volume: F,
    pub nox_burn_rate: F,
    pub nox_hp_boost: F,
    /// `TCSEnabled` as the integer word, `MinTractionSlip`, `MaxTractionSlip`.
    pub tcs_enabled: i32,
    pub min_traction_slip: F,
    pub max_traction_slip: F,
}

/// What putting the gearbox in first works out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FirstGear {
    pub ratio: F,
    pub inv_ratio: F,
    /// `redline * (2pi/60) / ratio`: the driven tires' top wheel speed, radians per second.
    pub top_wheel_speed: F,
}

/// The shift points: `LowLoad…` and `HighLoad…` RPMs, `ShiftTime`, and the throttle the box holds
/// for how long while it shifts each way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shift {
    pub low_load_rpm: F,
    pub low_load_up_shift_rpm: F,
    pub low_load_down_shift_rpm: F,
    /// Read with the leave-alone getter into a temporary, so a file without it hands the setter
    /// whatever the stack held. Every retail file has it; `None` marks the case.
    pub high_load_rpm: Option<F>,
    pub high_load_up_shift_rpm: F,
    pub high_load_down_shift_rpm: F,
    pub shift_time: F,
    pub shift_up_duration: F,
    pub shift_up_throttle: F,
    pub shift_down_duration: F,
    pub shift_down_throttle: F,
}

/// `[Aerodynamics]` and its wings.
#[derive(Clone, Debug, PartialEq)]
pub struct Aero {
    pub airborne_damping: F,
    /// `AirborneControlMoment`, the pad's moment in the air per axis.
    pub airborne_control_moment: Vf,
    pub unroll_gain: F,
    pub unroll_torque: F,
    pub unyaw_gain: F,
    pub unyaw_torque: F,
    pub brake_gyro_torque: F,
    pub aero_surface_area: F,
    /// `NumberOfWings` as the integer word, and `Wing 1..` that many.
    pub number_of_wings: i32,
    pub wings: Vec<Wing>,
}

/// One `Wing N`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wing {
    pub lift_slope: F,
    /// `AngleOfAttack` in radians.
    pub angle_of_attack: F,
    pub basic_drag: F,
    pub induced_drag: F,
    pub position: Vf,
}

/// `[Preload]`: each `None` when missing, which leaves the car's constructed value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preload {
    pub preload_time: Option<F>,
    pub max_preload: Option<F>,
    pub min_preload_time: Option<F>,
    pub max_preload_time: Option<F>,
}

/// `[Joystick]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Joystick {
    pub square_steering_input: i32,
    pub x_axis_rate_limit: F,
    pub x_dpad_rate_limit_on: F,
    pub x_dpad_rate_limit_off: F,
    pub brake_rate_limit: F,
    pub gas_rate_limit: F,
    /// `MaxAnalogThrottle`, default 1.
    pub max_analog_throttle: F,
    /// `DigitalThrottleSpeed` as a speed, `DigitalSideSlipAngle` in radians.
    pub digital_throttle_speed: F,
    pub digital_side_slip_angle: F,
    /// The stick's, the wheel's and the d-pad's steer tables, three `(speed, scale)` points each,
    /// speeds converted.
    pub steer: [[F; 2]; 3],
    pub wheel_steer: [[F; 2]; 3],
    pub dpad_steer: [[F; 2]; 3],
    /// `RearSteerSpeed1/Scale1/Speed2/Scale2`: the rear tires' share of the steer. Every value but
    /// the first speed defaults to 1 — the second *speed* too, before it is converted.
    pub rear_steer: [[F; 2]; 2],
}

/// `[Animation]`, read by the machine's own class after the shared loader.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Animation {
    /// `PelvisBoxPosition`, `PelvisBoxSize`: the box the driver's pelvis sits in.
    pub pelvis_box_position: Vf,
    pub pelvis_box_size: Vf,
    /// `RearTireSteerScale` (default 1): the truck's only. `None` for the buggy and the baja.
    pub rear_tire_steer_scale: Option<F>,
}

impl Car {
    /// Read a machine's file out of its bytes.
    pub fn read_bytes(bytes: &[u8], kind: Kind) -> Car {
        Car::read(&Ini::parse_bytes(bytes), kind)
    }

    /// Load a parsed file, in the loader's order. Two orders matter: the part comes first, because
    /// its mass is what every tire's spring is worked out against and its `CofG` is `CofGAI`'s
    /// default; and the RPM table comes before the gears, because its last entry sets the top speed
    /// putting the box in first hands the driven tires.
    pub fn read(ini: &Ini, kind: Kind) -> Car {
        let s = ini.section("Mass Properties");
        let mass_properties = Part::read(s);
        let zero = |s: &Section, key: &str| s.float_or(key, F::ZERO);
        let cofg_ai = s.vector("CofGAI", mass_properties.cofg);
        let cofg_travel_forward = zero(&s, "CofGTravelForward");
        let cofg_travel_back = zero(&s, "CofGTravelBack");
        let yaw_rate_limit = zero(&s, "YawRateLimit");
        let hand_brake_yaw_rate_limit = zero(&s, "HandBrakeYawRateLimit");
        let hand_brake_friction_scale = zero(&s, "HandBrakeFrictionScale");
        let hand_brake_brake_scale = zero(&s, "HandBrakeBrakeScale");
        let hand_brake_fade_time = zero(&s, "HandBrakeFadeTime");

        // The tire's spring setter reads the mass off the body the part loader has just filled in.
        let mass = mass_properties.mass;
        let mut surfaces = 0;
        let tires = TIRES.map(|name| Tire::read(ini, name, mass, &mut surfaces));

        let s = ini.section("Engine");
        let engine = Engine::read(&s);
        let clutch_torque_scale_time = s.float("ClutchTorqueScaleTime");
        let max_clutch_torque_scale_speed = s.float("MaxClutchTorqueScaleSpeed");
        let max_clutch_torque_scale = s.float("MaxClutchTorqueScale");

        let s = ini.section("Aerodynamics");
        let number_of_wings = s.int("NumberOfWings").unwrap_or(0);
        let aero = Aero {
            airborne_damping: zero(&s, "AirborneDamping"),
            airborne_control_moment: s.vector("AirborneControlMoment", UNIT_W),
            unroll_gain: zero(&s, "UnrollGain"),
            unroll_torque: zero(&s, "UnrollTorque"),
            unyaw_gain: zero(&s, "UnyawGain"),
            unyaw_torque: zero(&s, "UnyawTorque"),
            brake_gyro_torque: zero(&s, "BrakeGyroTorque"),
            aero_surface_area: zero(&s, "AeroSurfaceArea"),
            number_of_wings,
            wings: (1..=number_of_wings)
                .map(|k| {
                    let s = ini.section(&format!("Wing {k}"));
                    Wing {
                        lift_slope: zero(&s, "LiftSlope"),
                        angle_of_attack: zero(&s, "AngleOfAttack") * DEG,
                        basic_drag: zero(&s, "BasicDrag"),
                        induced_drag: zero(&s, "InducedDrag"),
                        position: s.vector("Position", UNIT_W),
                    }
                })
                .collect(),
        };

        let s = ini.section("Preload");
        let preload = Preload {
            preload_time: s.float("PreloadTime"),
            max_preload: s.float("MaxPreload"),
            min_preload_time: s.float("MinPreloadTime"),
            max_preload_time: s.float("MaxPreloadTime"),
        };

        let max_wreck_time = ini.section("Wrecks").float_or("MaxWreckTime", F(5.0));

        let s = ini.section("Joystick");
        // Every steer table's speed is read in miles per hour and converted after the whole table is
        // in; the scales are stored as written.
        let table = |prefix: &str| {
            [1, 2, 3].map(|k| {
                [zero(&s, &format!("{prefix}Speed{k}")) * MPH, zero(&s, &format!("{prefix}Scale{k}"))]
            })
        };
        let joystick = Joystick {
            square_steering_input: s.int("SquareSteeringInput").unwrap_or(0),
            x_axis_rate_limit: zero(&s, "XAxisRateLimit"),
            x_dpad_rate_limit_on: zero(&s, "XDPadRateLimitOn"),
            x_dpad_rate_limit_off: zero(&s, "XDPadRateLimitOff"),
            brake_rate_limit: zero(&s, "BrakeRateLimit"),
            gas_rate_limit: zero(&s, "GasRateLimit"),
            max_analog_throttle: s.float_or("MaxAnalogThrottle", F::ONE),
            digital_throttle_speed: zero(&s, "DigitalThrottleSpeed") * MPH,
            digital_side_slip_angle: zero(&s, "DigitalSideSlipAngle") * DEG,
            steer: table("Steer"),
            wheel_steer: table("WheelSteer"),
            dpad_steer: table("DPadSteer"),
            rear_steer: [
                [zero(&s, "RearSteerSpeed1") * MPH, s.float_or("RearSteerScale1", F::ONE)],
                [s.float_or("RearSteerSpeed2", F::ONE) * MPH, s.float_or("RearSteerScale2", F::ONE)],
            ],
        };

        let s = ini.section("Animation");
        let animation = Animation {
            pelvis_box_position: s.vector("PelvisBoxPosition", UNIT_W),
            pelvis_box_size: s.vector("PelvisBoxSize", UNIT_W),
            rear_tire_steer_scale: match kind {
                Kind::MonsterTruck => Some(s.float_or("RearTireSteerScale", F::ONE)),
                Kind::Buggy | Kind::Baja => None,
            },
        };

        Car {
            mass_properties,
            cofg_ai,
            cofg_travel_forward,
            cofg_travel_back,
            yaw_rate_limit,
            hand_brake_yaw_rate_limit,
            hand_brake_friction_scale,
            hand_brake_brake_scale,
            hand_brake_fade_time,
            tires,
            engine,
            clutch_torque_scale_time,
            max_clutch_torque_scale,
            max_clutch_torque_scale_speed,
            aero,
            preload,
            max_wreck_time,
            joystick,
            animation,
        }
    }

    /// A driven tire's top wheel speed: what putting the box in first wrote to it. `None` for an
    /// undriven tire, or with no gears.
    pub fn top_wheel_speed(&self, tire: usize) -> Option<F> {
        let first = self.engine.first?;
        self.engine.driven.contains(&Some(tire)).then_some(first.top_wheel_speed)
    }
}

impl Tire {
    /// One tire section and its surfaces, against the car's collision mass. `surfaces` is the
    /// count carried over from the tire before.
    fn read(ini: &Ini, name: &str, mass: F, surfaces: &mut i32) -> Tire {
        let s = ini.section(name);
        let zero = |key: &str| s.float_or(key, F::ZERO);
        let one = |key: &str| s.float_or(key, F::ONE);
        let max_compression = one("MaxCompression");
        let max_compression_rate = one("MaxCompressionRate");
        let max_decompression_rate = one("MaxDecompressionRate");
        let radius = one("TireRadius");
        let width = zero("TireWidth");
        let inertia = one("TireInertia");
        let gyro_inertia = one("GyroInertia");
        let spring = spring_rate(zero("Spring"), mass);
        let bounce = zero("BounceDampingRatio");
        let rebound = zero("ReboundDampingRatio");
        let max_steer_angle = zero("MaxSteerAngle") * DEG;
        let max_tire_angle_rate = s.float_or("MaxTireAngleRate", F::MAX);
        let max_tire_scale = zero("MaxTireScale");
        let camber_roll_ratio = zero("CamberRollRatio");
        let abs_enabled = s.int("ABSEnabled").unwrap_or(0);
        let min_braking_slip = zero("MinBrakingSlip");
        let max_braking_slip = zero("MaxBrakingSlip");
        if let Some(n) = s.int("NumberOfSurfaces") {
            *surfaces = n;
        }
        let number_of_surfaces = *surfaces;
        let surfaces = (1..=number_of_surfaces)
            .take(MAX_SURFACES)
            .map(|k| Surface::read(ini.section(&format!("{name} Surface {k}"))))
            .collect();
        Tire {
            max_compression,
            max_compression_rate,
            max_decompression_rate,
            spring,
            bounce_damping: damping(bounce, mass, spring),
            rebound_damping: damping(rebound, mass, spring),
            radius,
            radius_squared: radius * radius,
            inv_radius: (radius > F::ZERO).then(|| F::ONE / radius),
            width,
            inv_inertia: if inertia > F::ZERO { F::ONE / inertia } else { F::ZERO },
            gyro_inertia,
            max_steer_angle,
            max_tire_angle_rate,
            max_tire_scale,
            camber_roll_ratio,
            abs_enabled,
            min_braking_slip,
            max_braking_slip,
            number_of_surfaces,
            surfaces,
        }
    }
}

impl Surface {
    fn read(s: Section) -> Surface {
        let f = |key: &str| s.float_or(key, F::ZERO);
        Surface {
            c1: f("C1"),
            c2: f("C2"),
            c3: f("C3"),
            c4: f("C4"),
            a0: f("A0"),
            a1: f("A1"),
            inv_a2: F::ONE / f("A2"),
            a3: f("A3"),
            inv_a4: F::ONE / f("A4"),
            moment_scale: f("MomentScale"),
            cornering_stiffness: f("CorneringStiffness"),
            max_normal_load: f("MaxNormalLoad"),
            friction_peak: f("FrictionPeak"),
            friction_end_point: f("FrictionEndPoint"),
            max_braking_torque: f("MaxBrakingTorque"),
            wheel_friction: f("WheelFriction"),
        }
    }
}

impl Engine {
    fn read(s: &Section) -> Engine {
        let zero = |key: &str| s.float_or(key, F::ZERO);
        let drive_train = s.int("DriveTrain").unwrap_or(0);
        let points = s.int("NumberOfRPMPoints").unwrap_or(0).max(0);
        let (rpm, hp): (Vec<F>, Vec<F>) =
            (1..=points).map(|k| (zero(&format!("RPM{k}")), zero(&format!("HP{k}")))).unzip();
        let gears = s.int("NumberOfGears").unwrap_or(0).max(0);
        let gear_ratios: Vec<F> = (1..=gears).map(|k| zero(&format!("GearRatio{k}"))).collect();
        let final_gear_ratio = zero("FinalGearRatio");
        let transmission_loss = zero("TransmissionLoss");
        let inertia = zero("EngineInertia");
        let min_rpm = zero("MinRPM");
        let idle_throttle = zero("IdleThrottle");
        let engine_braking = zero("EngineBraking") * F(1e-6);

        let driven = match drive_train {
            0 => [Some(FRONT_LEFT), Some(FRONT_RIGHT), None, None],
            1 => [Some(REAR_LEFT), Some(REAR_RIGHT), None, None],
            _ => [Some(FRONT_LEFT), Some(FRONT_RIGHT), Some(REAR_LEFT), Some(REAR_RIGHT)],
        };
        let torque = rpm
            .iter()
            .zip(&hp)
            .map(|(&r, &h)| if r > F::ZERO { h * HP_TORQUE / r + engine_braking * r * r } else { F::ZERO })
            .collect();
        let max_rpm = rpm.last().copied();
        let first = gear_ratios.first().map(|&g| {
            let ratio = final_gear_ratio * g;
            let inv_ratio = F::ONE / ratio;
            // An empty RPM table would have the original read past its allocation; zero stands in.
            let redline = max_rpm.unwrap_or(F::ZERO);
            FirstGear { ratio, inv_ratio, top_wheel_speed: redline * RPM_TO_RAD * inv_ratio }
        });

        Engine {
            drive_train,
            driven,
            rpm,
            hp,
            torque,
            engine_braking,
            max_rpm,
            gear_ratios,
            final_gear_ratio,
            first,
            inertia,
            inv_inertia: (inertia > F::ZERO).then(|| F::ONE / inertia),
            transmission_loss: transmission_loss * F(0.01),
            min_rpm,
            idle_throttle,
            max_k: zero("maxK"),
            pump_torque_scale: zero("PumpTorqueScale") * F(0.001),
            max_pump_torque: zero("MaxPumpTorque"),
            shift: Shift {
                low_load_rpm: zero("LowLoadRPM"),
                low_load_up_shift_rpm: zero("LowLoadUpShiftRPM"),
                low_load_down_shift_rpm: zero("LowLoadDownShiftRPM"),
                high_load_rpm: s.float("HighLoadRPM"),
                high_load_up_shift_rpm: zero("HighLoadUpShiftRPM"),
                high_load_down_shift_rpm: zero("HighLoadDownShiftRPM"),
                shift_time: zero("ShiftTime"),
                shift_up_duration: zero("ShiftUpDuration"),
                shift_up_throttle: zero("ShiftUpThrottle"),
                shift_down_duration: zero("ShiftDownDuration"),
                shift_down_throttle: zero("ShiftDownThrottle"),
            },
            nox_volume: zero("NOXVolume"),
            nox_burn_rate: zero("NOXBurnRate"),
            nox_hp_boost: zero("NOXHPBoost"),
            tcs_enabled: s.int("TCSEnabled").unwrap_or(0),
            min_traction_slip: zero("MinTractionSlip"),
            max_traction_slip: zero("MaxTractionSlip"),
        }
    }
}

// --- the tire model ----------------------------------------------------------------------------------
//
// Each tire is stepped in three parts, in this order, front pair first: its suspension
// ([`Tire::contact`], [`Tire::spring`], [`Tire::settle`]), its rolling and brake torque ([`Tire::roll`]),
// and its forces ([`Tire::forces`]). The engine's drive torque goes in before all of them
// ([`TireState::torque`]), and between the front and rear pairs the car hands the fronts' load to the
// rears ([`hand_off`]). `tests/car_tire.rs` replays each of these against the S149 drives, tick by tick.

/// The original's own literals for the circle, to the bit (as [`crate::camera`] holds them).
const PI: f32 = 3.1415927;
const TWO_PI: f32 = 6.2831855;
const INV_TWO_PI: f32 = 0.15915494;
/// `pi/2` and `-pi/2` as the slip-angle clamp writes them.
const HALF_PI: f32 = f32::from_bits(0x3fc9_0fdb);
/// The slip ratio the brake locks a wheel past, and the slip ratio and angle a tire counts as
/// slipping and sliding past.
const LOCK_SLIP: f32 = 0.95;
const SLIP_FLAG: f32 = 0.2;
/// Below this forward speed (ft/s) the slip comes from a relaxation, not from the speeds.
const SLIP_SPEED: f32 = 20.0;
/// Below this forward speed the low-speed grip may switch on, above it it switches off.
const GRIP_SPEED: f32 = 5.0;
/// The relaxation rate of the low-speed slip ratio.
const SLIP_RELAX: f32 = 5.0;
/// The low-speed grip: a sideways force of `1.4 * v * sqrt(100 * N)` against the slide.
const GRIP_SCALE: f32 = 1.4;
const GRIP_LOAD: f32 = 100.0;
/// The rational arc tangent's coefficient.
const ATAN_K: f32 = 0.28;
/// `4/pi`, the linear term of the tire's force shape.
const FOUR_OVER_PI: f32 = 1.2732395;
/// The clamps on the slip ratio and the camber.
const MAX_SLIP_RATIO: f32 = f32::from_bits(0x3f7a_e148);
const MAX_CAMBER: f32 = f32::from_bits(0x3db8_51ec);
/// The rough-ground ratio past which a bottomed-out tire is not pushed back up, unless its contact
/// is a solid one.
const BOTTOM_RATIO: f32 = 2.0;
/// What the tire's constructor leaves as the top wheel speed of a tire no engine slot drives
/// (rad/s).
pub const UNDRIVEN_TOP_WHEEL_SPEED: F = F(f32::from_bits(0x4392_aa7f));

/// One tire's running state: every word of the tire object the model reads or writes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TireState {
    /// Which of the tire's sixteen surfaces it is on.
    pub surface: usize,
    /// On the ground this tick.
    pub grounded: bool,
    /// The load the car handed a rear tire whose fronts are down ([`hand_off`]), for a tick the rear
    /// itself is off the ground.
    pub handoff: Option<F>,
    /// The suspension's compression this tick, and the limited one it carries to the next.
    pub compression: F,
    pub last_compression: F,
    /// The normal force.
    pub load: F,
    /// Where the tire meets the ground, on the line between its axle points.
    pub contact: Vf,
    /// The wheel's turn, and its spin rate (rad/s).
    pub angle: F,
    pub spin: F,
    /// The torque on the wheel this tick (the engine's, then rolling and brake), and last tick's.
    pub torque: F,
    pub last_torque: F,
    /// The slip ratio, the slip angle, and the tangent of the slip angle.
    pub slip_ratio: F,
    pub slip_angle: F,
    pub slip_tan: F,
    /// The contact's velocity in the tire's frame: `x` sideways, `y` along the normal, `z` forward;
    /// and the last one the low-speed path kept.
    pub velocity: Vf,
    pub last_velocity: Vf,
    /// The ground's force in the tire's frame (`x` sideways, `z` forward), and last tick's.
    pub force: Vf,
    pub last_force: Vf,
    /// The steer angle.
    pub steer: F,
    /// The sideways force over the load.
    pub side_ratio: F,
    /// The camber, from the ground normal as the car sees it.
    pub camber: F,
    /// The scale on the surface's friction peak (1 as built).
    pub friction_scale: F,
    /// The low-speed grip is on.
    pub low_speed_grip: bool,
    /// `|slip ratio| > 0.2`, `|slip angle| > 0.2`, and the brake has locked the wheel.
    pub slipping: bool,
    pub sliding: bool,
    pub locked: bool,
}

/// What a tire is touching this tick: the ground point its probe found and the ground's normal, and
/// whether it is a contact of the kind that pushes a bottomed-out tire back up however far the
/// ground came through.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    pub point: Vf,
    pub normal: Vf,
    pub solid: bool,
}

/// What the car's step hands a rear tire before stepping it: when either front is down, the larger
/// front load. A rear tire off the ground then rides on it.
pub fn hand_off(front_left: &TireState, front_right: &TireState) -> Option<F> {
    (front_left.grounded || front_right.grounded).then(|| {
        let (fl, fr) = (front_left.load, front_right.load);
        if fr < fl { fl } else { fr }
    })
}

impl Tire {
    /// The suspension's contact: the ground point taken onto the line from the tire's upper axle point
    /// to its lower one (`axle`, world space), and the compression as the distance along it. A
    /// compression past the limit is held at the limit, and if the ground is solid or came less than
    /// twice the travel through, the distance it went past is returned for the car to push the body
    /// up by along the normal (and to stop the point's inward speed), as the original does before it
    /// carries on.
    pub fn contact(&self, s: &mut TireState, axle: (Vf, Vf), ground: Option<&Ground>) -> Option<F> {
        s.compression = F::ZERO;
        s.grounded = false;
        let g = ground?;
        let (top, bottom) = axle;
        let n = g.normal;
        let travel = top.merge(Dest::XYZ, top.sub(bottom));
        let along = dot3(travel, n);
        if along <= 0.0 {
            return None;
        }
        let ratio = dot3(g.point.merge(Dest::XYZ, g.point.sub(bottom)), n) / along;
        if ratio <= 0.0 {
            return None;
        }
        let reach = travel.merge(Dest::XYZ, travel.mul_q(ratio.0));
        s.contact = reach.merge(Dest::XYZ, reach.add(bottom));
        let d = s.contact.merge(Dest::XYZ, s.contact.sub(bottom));
        s.compression = dot3(d, d).sqrt();
        if s.compression <= 0.0 {
            return None;
        }
        s.grounded = true;
        let max = self.max_compression;
        if s.compression <= max {
            return None;
        }
        let push = (s.compression - max).max(F::ZERO).min(max);
        s.compression = max;
        (g.solid || ratio < BOTTOM_RATIO).then_some(push)
    }

    /// The spring and dampers of a grounded tire: the load from the compression, less the damper on
    /// the contact point's world velocity along the normal, limited to the compression rate. A tire
    /// off the ground takes the load its car handed it, or none. Returns the load, which the car
    /// applies along the normal at the contact.
    pub fn spring(&self, s: &mut TireState, point_velocity: Vf, normal: Vf) -> F {
        if !s.grounded {
            s.load = s.handoff.unwrap_or(F::ZERO);
            return s.load;
        }
        let rate = self.max_compression_rate;
        let v = dot3(point_velocity, normal).max(-rate).min(rate);
        let damper = if v <= 0.0 { self.rebound_damping } else { self.bounce_damping };
        let damped = damper * v;
        s.load = F::ZERO.max(self.spring * s.compression - damped);
        s.load
    }

    /// The suspension's last part: the compression may come back out no faster than the
    /// decompression rate, and what is carried to the next tick is held to the limit.
    pub fn settle(&self, s: &mut TireState, dt: F) {
        let last = s.last_compression;
        if s.compression < last {
            s.compression = last - (last - s.compression).min(self.max_decompression_rate * dt);
        }
        s.last_compression = s.compression.min(self.max_compression);
    }

    /// The rolling resistance and the brake, as torque on the wheel. ABS (on a grounded tire) scales
    /// the brake down across the slip window.
    pub fn roll(&self, s: &mut TireState, brake: Option<F>) {
        let surface = self.surface(s.surface);
        let half = (F(0.5) * s.spin).max(F(-1.0)).min(F::ONE);
        s.torque = s.torque + (-half) * surface.wheel_friction;
        let Some(brake) = brake.filter(|b| *b > 0.0) else { return };
        let mut scale = F::ONE;
        if self.abs_enabled != 0 && s.grounded {
            let (lo, hi) = (self.min_braking_slip, self.max_braking_slip);
            if lo != hi {
                let f = (s.slip_ratio.abs() - lo) / (hi - lo);
                scale = Acc::adda(F::ZERO, F::ONE).madd(f, F(-1.0)).max(F::ZERO).min(F::ONE);
            }
        }
        let held = scale * (-brake.max(F::ZERO));
        s.torque = s.torque + surface.max_braking_torque * (half * held);
    }

    /// The ground's force on the tire, and the wheel's turn: the steer, the spin from the torque, the
    /// slip from the contact's velocity in the tire's frame (`velocity`, which only a grounded tire
    /// has), the surface's forces, and then the bookkeeping the next tick reads. The force is left in
    /// [`TireState::force`] for the car to turn into world space and apply.
    ///
    /// `top_wheel_speed` is the spin the tire is held to, `camber_normal` the ground normal's `x` and
    /// `y` as the car sees them.
    #[allow(clippy::too_many_arguments)]
    pub fn forces(
        &self,
        s: &mut TireState,
        dt: F,
        brake: Option<F>,
        steer: Option<F>,
        top_wheel_speed: F,
        velocity: Vf,
        camber_normal: (F, F),
        acos: &AcosTable,
        curve: &CurveTable,
    ) {
        if let Some(steer) = steer {
            s.steer = steer * self.max_steer_angle;
        }
        s.force = UNIT_W;
        s.velocity = if s.grounded { velocity } else { UNIT_W };
        self.slip(s, dt, brake, top_wheel_speed, camber_normal, acos, curve);
        if s.grounded {
            self.surface_force(s, curve);
        }
        s.last_force = s.force;
        let rate = self.max_tire_angle_rate;
        s.angle = s.angle + dt * s.spin.max(-rate).min(rate);
        s.angle = wrap_turn(s.angle);
        s.last_torque = s.torque;
        s.torque = F::ZERO;
        s.side_ratio = if s.load <= 0.0 { F::ZERO } else { F(s.force.x()) / s.load };
    }

    /// The wheel's spin and the tire's slip ratio, slip angle and camber.
    #[allow(clippy::too_many_arguments)]
    fn slip(
        &self,
        s: &mut TireState,
        dt: F,
        brake: Option<F>,
        top_wheel_speed: F,
        camber_normal: (F, F),
        acos: &AcosTable,
        curve: &CurveTable,
    ) {
        let (lateral, forward) = (F(s.velocity.x()), F(s.velocity.z()));
        let speed = forward.abs();
        if brake.is_some_and(|b| b > 0.0) && s.slip_ratio > LOCK_SLIP {
            s.slipping = false;
            s.locked = true;
            s.spin = F::ZERO;
        } else {
            s.slipping = s.slip_ratio.abs() > SLIP_FLAG;
            s.locked = false;
            let rolling = F(s.last_force.z()) * self.radius;
            s.spin = s.spin + self.inv_inertia * (s.torque - rolling) * dt;
            s.spin = s.spin.max(-top_wheel_speed).min(top_wheel_speed);
        }
        if speed > SLIP_SPEED {
            self.slip_at_speed(s, acos, curve);
        } else {
            if speed < GRIP_SPEED {
                if F(s.last_velocity.x()) * lateral < 0.0 {
                    s.low_speed_grip = true;
                }
            } else if speed > GRIP_SPEED {
                s.low_speed_grip = false;
            }
            s.last_velocity = s.velocity;
            if s.low_speed_grip {
                s.force[0] = (F(s.force.x()) - grip(s)).0;
            }
            s.slip_tan = s.slip_tan + dt * (lateral - speed * s.slip_tan);
            let x = s.slip_tan.abs();
            let atan = if x <= 1.0 {
                x / (F::ONE + F(ATAN_K) * x * x)
            } else {
                F(HALF_PI) - x / (F(ATAN_K) + x * x)
            };
            s.slip_angle = if s.slip_tan < 0.0 { -atan } else { atan };
            let creep = Acc::suba(forward, speed * s.slip_ratio).msub(self.radius, s.spin);
            s.slip_ratio = s.slip_ratio + dt * (F(SLIP_RELAX) * creep);
        }
        s.sliding = s.slip_angle.abs() > SLIP_FLAG;
        s.camber = self.camber(camber_normal, acos);
        clamp_slip(s);
    }

    /// The slip of a tire moving forward faster than 20 ft/s, straight from its contact velocity and
    /// spin: the slip angle by the arc cosine of the forward share of the speed, the slip ratio as
    /// the forward speed the spin does not account for, and the angle's tangent off the curve table.
    /// Unclamped ([`clamp_slip`] follows).
    pub fn slip_at_speed(&self, s: &mut TireState, acos: &AcosTable, curve: &CurveTable) {
        let (lateral, forward) = (F(s.velocity.x()), F(s.velocity.z()));
        let speed = forward.abs();
        let cos = speed * F::ONE.rsqrt(Acc::mula(lateral, lateral).madd(speed, speed));
        let angle = acos.at(clamp_unit(clamp_unit(cos)));
        s.slip_angle = if lateral < 0.0 { -angle } else { angle };
        s.slip_ratio = (forward - s.spin * self.radius) / speed;
        let (cos, sin) = curve.coarse(s.slip_angle);
        s.slip_tan = sin / cos;
    }

    /// The camber from the ground normal's `x` and `y` as the car sees them: the lean off the car's
    /// up axis, by the roll ratio. Unclamped ([`clamp_slip`] follows).
    pub fn camber(&self, normal: (F, F), acos: &AcosTable) -> F {
        let (x, y) = normal;
        let cos = y * F::ONE.rsqrt(Acc::mula(x, x).madd(y, y));
        let lean = acos.at(clamp_unit(clamp_unit(cos)));
        let lean = if x < 0.0 { -lean } else { lean };
        (-self.camber_roll_ratio) * lean
    }

    /// The surface's forces on a grounded tire, added to [`TireState::force`]: the combined-slip
    /// magnitude through the surface's rational shape (`C1`..`C4`), shared between the sideways and
    /// forward directions by the slip, with the friction falling off toward its end point, and the
    /// camber's sideways thrust (`A3`, `A4`). The load is capped at the surface's maximum.
    pub fn surface_force(&self, s: &mut TireState, curve: &CurveTable) {
        let sf = self.surface(s.surface);
        let sr = s.slip_ratio;
        let (cos, sin) = curve.coarse(s.slip_angle);
        let sin2 = sin * sin;
        let tan = sin / cos;
        let cos2 = cos * cos;
        let tan2 = tan * tan;
        let sr2 = sr * sr;
        let mu = s.friction_scale * sf.friction_peak;
        let load = s.load.min(sf.max_normal_load);
        let (lateral, forward) = if load == 0.0 {
            (F::ZERO, F::ZERO)
        } else {
            let load2 = load * load;
            let a1_a2 = sf.a1 * sf.inv_a2;
            let d = F(2.0) * Acc::adda(sf.a0, sf.a1 * load).msub(load2, a1_a2);
            let combined = (sin2 + sr2 * cos2).sqrt();
            let cs = (F(2.0) * load) * sf.cornering_stiffness;
            let cx = cs + combined * (d - cs);
            let q = (cs * sr) / (F::ONE - sr);
            let mue = mu * (F::ONE - sf.friction_end_point * combined);
            let d2tan2 = (d * d) * tan2;
            let sigma = (F(PI) * (q * q + d2tan2).sqrt()) / ((F(8.0) * mu) * load);
            let a3_a4 = sf.a3 * sf.inv_a4;
            let sigma2 = sigma * sigma;
            let c3 = sf.c3 * sigma2;
            let sigma3 = sigma2 * sigma;
            let c1 = sf.c1 * sigma3;
            let c2 = sf.c2 * sigma2;
            let num = Acc::adda(c1, c2).madd(F(FOUR_OVER_PI), sigma);
            let c4 = sf.c4 * sigma;
            let den = F::ONE + (c4 + (c1 + c3));
            let camber_k = Acc::mula(sf.a3, load).msub(load2, a3_a4);
            let shape = num / den;
            let norm = (d2tan2 + (cx * cx) * sr2).sqrt();
            let (x, y) = if norm.abs() > 0.0 {
                let inv = F::ONE / norm;
                (inv * (sr * (shape * cx)), inv * (tan * ((-shape) * d)))
            } else {
                (F::ZERO, F::ZERO)
            };
            (Acc::mula(load, y * mue).msub(camber_k, s.camber), load * ((-x) * mue))
        };
        s.force[0] = (F(s.force.x()) + lateral).0;
        s.force[2] = (F(s.force.z()) + forward).0;
    }

    /// The tire's surface `i`; one the file leaves out is all zeros, as the loader leaves it.
    fn surface(&self, i: usize) -> Surface {
        self.surfaces.get(i).copied().unwrap_or_default()
    }
}

/// The low-speed grip's sideways force, `1.4 * v * sqrt(100 * N)`, which the slip takes off the
/// tire's sideways force while the grip is on.
pub fn grip(s: &TireState) -> F {
    F(GRIP_SCALE) * F(s.velocity.x()) * (F(GRIP_LOAD) * s.load).sqrt()
}

/// The slip's last step: the slip angle held to ±pi/2, the slip ratio to ±0.98, the camber to ±0.09.
pub fn clamp_slip(s: &mut TireState) {
    s.slip_angle = s.slip_angle.max(F(-HALF_PI)).min(F(HALF_PI));
    s.slip_ratio = s.slip_ratio.max(F(-MAX_SLIP_RATIO)).min(F(MAX_SLIP_RATIO));
    s.camber = s.camber.max(F(-MAX_CAMBER)).min(F(MAX_CAMBER));
}

/// `a·b` over `xyz` as the FPU's `mula`/`madda`/`madd` chain takes it.
fn dot3(a: Vf, b: Vf) -> F {
    Acc::mula(F(a.x()), F(b.x())).madda(F(a.y()), F(b.y())).madd(F(a.z()), F(b.z()))
}

/// An angle into `-pi..pi`: whole turns off by truncating `angle / 2pi`, then one correction.
fn wrap_turn(angle: F) -> F {
    let turns = F(fpu::from_i32((angle * INV_TWO_PI).to_i32()));
    let a = angle - F(TWO_PI) * turns;
    if a > PI {
        a - TWO_PI
    } else if a < -PI {
        a + TWO_PI
    } else {
        a
    }
}

/// `x` held inside −1..1 by the original's pair of selects, the high test first.
fn clamp_unit(x: F) -> F {
    if x <= 1.0 {
        if x >= -1.0 { x } else { F(-1.0) }
    } else {
        F::ONE
    }
}
