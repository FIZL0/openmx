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
//!
//! [`Drivetrain`] runs the engine and the automatic gearbox the loader set up (S149c): the shift once
//! a frame, the engine step every physics substep, and the drive torque each driven tire gets.
//! `tests/car_engine.rs` proves it against the original and against S149a's captured drives.

#![deny(clippy::float_arithmetic)]

use crate::phy::{DEG, Ini, MAX_SURFACES, MPH, Part, Section, damping, spring_rate};
use crate::camera::CurveTable;
use ps2_float::{Acc, F, Vf, fpu};

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

/// Engine revs per radian a second: `60 / 2pi`, as the step multiplies by it.
const RAD_TO_RPM: f32 = 9.549296;
/// The engine pitch's floor, top and the two thresholds the step picks its wobble by.
const PITCH_FLOOR: f32 = 0.1;
const PITCH_HIGH: f32 = 0.95;
const PITCH_LOW: f32 = 0.2;

/// What the engine reads of a driven tire, and the two words it writes back.
///
/// The step takes the drive slots in the engine's order (see [`Engine::driven`]): slots 0 and 1
/// are always there; slots 2 and 3 only on a four-wheel drive, and the step leaves them alone
/// otherwise.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DriveTire {
    /// On the ground: the tire's contact word, nonzero when it touches something.
    pub contact: bool,
    /// The wheel's spin, radians a second.
    pub spin: F,
    /// The tire's longitudinal slip ratio, which traction control reads.
    pub slip_ratio: F,
    /// The suspension's compression, and the tire's `MaxCompression` (only slot 0's is read).
    pub compression: F,
    pub max_compression: F,
    /// The torque the drivetrain hands the tire this substep: each step adds its share.
    pub drive_torque: F,
    /// The wheel speed the gear tops out at, which every gear change rewrites.
    pub top_wheel_speed: F,
}

/// The three pedal words the engine reads through its pointers: the car's throttle, clutch and
/// brake as the car left them this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pedals {
    pub throttle: F,
    pub clutch: F,
    pub brake: F,
}

/// Which way [`Drivetrain::shift`] moved the box: the original starts the shift sound on either.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shifted {
    Up,
    Down,
}

/// **The car's engine and automatic gearbox, running**: the loader's [`Engine`] plus every word
/// the step keeps between substeps.
///
/// Two calls a frame make it go. [`Drivetrain::shift`] runs once a frame, before the forces: it
/// counts the shift timer down, and once it has run out picks the gear by revs against shift points
/// that slide with the load (the revs the converter is slipping). [`Drivetrain::step`] runs every
/// physics substep, before the tires: the torque curve at the revs, nitrous, the throttle (or the
/// box's own while a shift is under way), engine braking, the torque converter's pump torque and
/// multiplication, the revs' next value, and each driven tire's share of the torque, cut by
/// traction control when the tire slips. Last it works out the engine sound's pitch.
///
/// Field names follow what each word does; the original's layout is in research/oracle.txt.
#[derive(Clone, Debug, PartialEq)]
pub struct Drivetrain {
    /// The torque table: its RPM axis and the torque at each point.
    pub rpm_points: Vec<F>,
    pub torque: Vec<F>,
    /// The table's last RPM, the most the revs can reach.
    pub redline: F,
    pub gear_ratios: Vec<F>,
    pub final_gear_ratio: F,
    /// Slots 2 and 3 are driven too.
    pub four_wheel: bool,
    pub idle_throttle: F,
    pub engine_braking: F,
    /// The fraction of the drive torque the box loses.
    pub transmission_loss: F,
    pub inv_inertia: F,
    pub min_rpm: F,
    pub pump_torque_scale: F,
    pub max_pump_torque: F,
    pub max_k: F,
    /// What the car scales the converter's output by while the box is driving forwards; 1 unless
    /// the car says otherwise.
    pub launch_boost: F,
    pub tcs: bool,
    pub min_traction_slip: F,
    pub max_traction_slip: F,
    /// Nitrous: on (the car sets it), what is left in the bottle, how fast it burns and the
    /// horsepower it adds.
    pub nox_on: bool,
    pub nox_left: F,
    pub nox_burn_rate: F,
    pub nox_hp_boost: F,
    /// The shift points and the shift's throttle; `high_load_rpm` as the setter kept it.
    pub shift_points: Shift,
    pub high_load_rpm: F,

    /// Time until the box may shift again.
    pub shift_timer: F,
    /// The last shift went up, so the shift's throttle is the up-shift one.
    pub shifting_up: bool,
    /// Time left of the shift's own throttle and clutch.
    pub shift_throttle_timer: F,
    pub reverse: bool,
    /// The gear, counted from 1; the overall ratio and its reciprocal.
    pub gear: i32,
    pub ratio: F,
    pub inv_ratio: F,
    /// The driven wheels' speed in engine revs (the engine's own revs while the clutch is out).
    pub wheel_rpm: F,
    /// The torque converter's pump torque, its multiplication, and the torque it hands the box.
    pub converter_torque: F,
    pub multiplication: F,
    pub output_torque: F,
    /// The torque curve's cursor: the point it sits on and the weight back to the one below.
    pub cursor_index: u32,
    pub cursor_frac: F,
    /// The engine's net torque and the engine braking taken off it.
    pub engine_torque: F,
    pub braking_torque: F,
    /// Each drive slot's torque this substep.
    pub drive_torque: [F; 4],
    pub rpm: F,
    /// The revs the converter slips by: the load the shift points slide with.
    pub slip_rpm: F,
    /// The engine sound's clock and its pitch, in the revs the sound plays at.
    pub sound_clock: F,
    pub pitch: F,
}

impl Drivetrain {
    /// The loader's engine, idle in first.
    pub fn new(engine: &Engine) -> Drivetrain {
        let first = engine.first.unwrap_or(FirstGear { ratio: F::ZERO, inv_ratio: F::ZERO, top_wheel_speed: F::ZERO });
        Drivetrain {
            rpm_points: engine.rpm.clone(),
            torque: engine.torque.clone(),
            redline: engine.max_rpm.unwrap_or(F::ZERO),
            gear_ratios: engine.gear_ratios.clone(),
            final_gear_ratio: engine.final_gear_ratio,
            four_wheel: engine.driven[2].is_some() && engine.driven[3].is_some(),
            idle_throttle: engine.idle_throttle,
            engine_braking: engine.engine_braking,
            transmission_loss: engine.transmission_loss,
            inv_inertia: engine.inv_inertia.unwrap_or(F::ZERO),
            min_rpm: engine.min_rpm,
            pump_torque_scale: engine.pump_torque_scale,
            max_pump_torque: engine.max_pump_torque,
            max_k: engine.max_k,
            launch_boost: F::ONE,
            tcs: engine.tcs_enabled != 0,
            min_traction_slip: engine.min_traction_slip,
            max_traction_slip: engine.max_traction_slip,
            nox_on: false,
            nox_left: engine.nox_volume,
            nox_burn_rate: engine.nox_burn_rate,
            nox_hp_boost: engine.nox_hp_boost,
            shift_points: engine.shift,
            high_load_rpm: engine.shift.high_load_rpm.unwrap_or(F::ZERO),
            shift_timer: F::ZERO,
            shifting_up: false,
            shift_throttle_timer: F::ZERO,
            reverse: false,
            gear: 1,
            ratio: first.ratio,
            inv_ratio: first.inv_ratio,
            wheel_rpm: F::ZERO,
            converter_torque: F::ZERO,
            multiplication: F::ZERO,
            output_torque: F::ZERO,
            cursor_index: 0,
            cursor_frac: F::ZERO,
            engine_torque: F::ZERO,
            braking_torque: F::ZERO,
            drive_torque: [F::ZERO; 4],
            rpm: F::ZERO,
            slip_rpm: F::ZERO,
            sound_clock: F::ZERO,
            pitch: F::ZERO,
        }
    }

    /// Put the box in gear `gear` (from 1): nothing in reverse or off the ends. The new overall
    /// ratio sets every driven tire's top wheel speed, the redline's.
    pub fn set_gear(&mut self, gear: i32, wheels: &mut [DriveTire; 4]) {
        if gear <= 0 || self.gear_ratios.len() < gear as usize || self.reverse {
            return;
        }
        self.gear = gear;
        self.ratio = self.final_gear_ratio * self.gear_ratios[gear as usize - 1];
        self.inv_ratio = F::ONE / self.ratio;
        let top = F(RPM_TO_RAD) * self.redline * self.inv_ratio;
        let driven = if self.four_wheel { 4 } else { 2 };
        for w in &mut wheels[..driven] {
            w.top_wheel_speed = top;
        }
    }

    /// Into reverse — first's ratio turned round, the tires' top speeds left alone — or out of it,
    /// into first.
    pub fn set_reverse(&mut self, on: bool, wheels: &mut [DriveTire; 4]) {
        if on {
            self.reverse = true;
            self.gear = 1;
            self.ratio = self.final_gear_ratio * -self.gear_ratios[0];
            self.inv_ratio = F::ONE / self.ratio;
        } else {
            self.reverse = false;
            self.set_gear(1, wheels);
        }
    }

    /// Stopped: no revs, and first gear's number (the ratio is left as it was).
    pub fn reset(&mut self) {
        self.rpm = F::ZERO;
        self.wheel_rpm = F::ZERO;
        self.gear = 1;
    }

    /// A shift point between its low-load and high-load RPMs, by where the slip sits between the
    /// two loads, held between them.
    fn shift_point(&self, low: F, high: F) -> F {
        let (lo, hi) = (self.shift_points.low_load_rpm, self.high_load_rpm);
        if lo == hi {
            return low;
        }
        let t = (self.slip_rpm - lo) / (hi - lo);
        Acc::adda(F::ZERO, low).madd(t, high - low).max(low.min(high)).min(low.max(high))
    }

    /// **The automatic box, once a frame.** Nothing while both front drive slots are off the ground
    /// or the box is in reverse. A running shift timer counts down by `dt` and that is all; else,
    /// with the clutch pedal up (`clutch <= 0`), revs over the up-shift point go up a gear and
    /// revs under the down-shift point go down one, each starting the shift's throttle and the
    /// timer. Revs over the point in top gear restart the timer with no shift.
    pub fn shift(&mut self, clutch: F, dt: F, wheels: &mut [DriveTire; 4]) -> Option<Shifted> {
        if (!wheels[0].contact && !wheels[1].contact) || self.reverse {
            return None;
        }
        if !(self.shift_timer <= F::ZERO) {
            self.shift_timer = self.shift_timer - dt;
            return None;
        }
        if !(clutch <= F::ZERO) {
            return None;
        }
        let s = self.shift_points;
        let up = self.shift_point(s.low_load_up_shift_rpm, s.high_load_up_shift_rpm);
        let down = self.shift_point(s.low_load_down_shift_rpm, s.high_load_down_shift_rpm);
        let mut shifted = None;
        if !(self.rpm <= up) {
            let next = self.gear + 1;
            if next as usize <= self.gear_ratios.len() {
                self.shifting_up = true;
                self.shift_throttle_timer = s.shift_up_duration;
                self.set_gear(next, wheels);
                shifted = Some(Shifted::Up);
            }
        } else if self.rpm < down {
            if self.gear < 2 {
                return None;
            }
            self.shifting_up = false;
            self.shift_throttle_timer = s.shift_down_duration;
            self.set_gear(self.gear - 1, wheels);
            shifted = Some(Shifted::Down);
        } else {
            return None;
        }
        self.shift_timer = s.shift_time;
        shifted
    }

    /// Walk the torque curve's cursor to `x`: [`crate::r#move::curve_seek`] over this table.
    fn seek(&mut self, x: F) {
        let xs = &self.rpm_points;
        let points = xs.len() as u32;
        let mut i = self.cursor_index;
        while i < points && !(x < xs[i as usize]) {
            i += 1;
        }
        while i != 0 && x < xs[i as usize - 1] {
            i -= 1;
        }
        if i < points {
            self.cursor_frac = if i == 0 {
                F::ZERO
            } else {
                let hi = xs[i as usize];
                (hi - x) / (hi - xs[i as usize - 1])
            };
        } else {
            self.cursor_frac = F::ZERO;
            i = points.wrapping_sub(1);
        }
        self.cursor_index = i;
    }

    fn curve_torque(&self) -> F {
        let i = self.cursor_index as usize;
        if i == 0 {
            self.torque[0]
        } else {
            let hi = self.torque[i];
            hi - self.cursor_frac * (hi - self.torque[i - 1])
        }
    }

    /// Traction control's cut for one tire: 1 under the minimum slip, falling to 0 at the maximum.
    fn traction(&self, slip_ratio: F) -> F {
        let (lo, hi) = (self.min_traction_slip, self.max_traction_slip);
        if lo == hi {
            return F::ONE;
        }
        let t = (slip_ratio.abs() - lo) / (hi - lo);
        Acc::adda(F::ZERO, F::ONE).madd(t, F::ZERO - F::ONE).max(F::ONE.min(F::ZERO)).min(F::ONE.max(F::ZERO))
    }

    /// **The engine, once a physics substep** of `dt`, before the tires step.
    ///
    /// - While the shift's timer runs the box holds the clutch in and sets its own throttle (the up
    ///   or the down shift's); otherwise the pedals'.
    /// - The torque curve at the revs, plus nitrous's `5252 * boost / rpm` while it lasts, scaled by
    ///   the throttle plus the idle throttle (at most 1), less engine braking `rpm * (k * rpm)`.
    /// - With the clutch up the driven wheels' mean spin through the ratio is the wheel revs, and
    ///   the engine's lead over them (never more than the revs) is the converter's slip. With it in,
    ///   the wheel revs are the engine's and nothing slips.
    /// - The converter pumps `slip² * scale` (signed, held to the maximum) and multiplies it by `maxK`
    ///   at a stall, easing to 1 as the wheels catch up; the launch boost scales it while driving
    ///   forwards.
    /// - The revs move by the net torque less the pump's over the inertia, damped implicitly by the
    ///   pump's slope, held between the minimum and the redline.
    /// - Unbraked, each driven tire gets the converter's output through the ratio, its share of the
    ///   drive less the box's loss, cut by traction control on the ground.
    /// - The sound's pitch: the revs' place between the minimum and the redline, wobbled by smooth
    ///   noise at the bottom and the curve table's sine at the top, dropped as the suspension
    ///   squats, mapped back onto the revs.
    ///
    /// `noise` is the engine's shared smooth noise ([`crate::noise::SmoothNoise::at`]).
    pub fn step(
        &mut self,
        pedals: Pedals,
        wheels: &mut [DriveTire; 4],
        dt: F,
        curve: &CurveTable,
        noise: &mut impl FnMut(F) -> F,
    ) {
        let (clutch, throttle) = if !(self.shift_throttle_timer <= F::ZERO) {
            let s = &self.shift_points;
            let throttle = if self.shifting_up { s.shift_up_throttle } else { s.shift_down_throttle };
            self.shift_throttle_timer = self.shift_throttle_timer - dt;
            (F::ONE, throttle)
        } else {
            (pedals.clutch, pedals.throttle)
        };

        self.seek(self.rpm);
        let mut torque = self.curve_torque();
        if self.nox_on && !(self.rpm <= F::ZERO) && !(self.nox_left <= F::ZERO) {
            let boost = F(HP_TORQUE) * self.nox_hp_boost / self.rpm;
            self.nox_left = (self.nox_left - self.nox_burn_rate * dt).max(F::ZERO);
            torque = torque + boost;
        }
        self.engine_torque = torque * (throttle + self.idle_throttle).min(F::ONE);
        self.braking_torque = self.rpm * (self.engine_braking * self.rpm);
        self.engine_torque = self.engine_torque - self.braking_torque;

        let (share, spin) = if clutch <= F::ZERO {
            let mut spin = wheels[0].spin + wheels[1].spin;
            let share = if self.four_wheel {
                spin = spin + (wheels[2].spin + wheels[3].spin);
                F(0.25)
            } else {
                F(0.5)
            };
            self.wheel_rpm = F(RAD_TO_RPM) * (self.ratio * (share * spin));
            self.slip_rpm = if self.wheel_rpm < F::ZERO { self.rpm } else { self.rpm - self.wheel_rpm };
            (share, spin)
        } else {
            self.wheel_rpm = self.rpm;
            self.slip_rpm = F::ZERO;
            (F::ZERO, F::ZERO)
        };

        let slip = self.slip_rpm;
        let sign = F(fpu::from_i32(if slip < F::ZERO { -1 } else { 1 }));
        self.converter_torque = slip * (slip * (self.pump_torque_scale * sign));
        self.converter_torque = self.converter_torque.max(-self.max_pump_torque).min(self.max_pump_torque);

        let (wheel_rpm, rpm) = (self.wheel_rpm, self.rpm);
        self.multiplication = if rpm <= wheel_rpm || rpm <= F::ZERO || wheel_rpm <= F::ZERO {
            self.max_k
        } else {
            self.max_k + (F::ONE - self.max_k) * (wheel_rpm / rpm)
        };
        let boost = if self.ratio <= F::ZERO || spin <= F::ZERO { F::ONE } else { self.launch_boost };
        self.output_torque = self.converter_torque * (boost * self.multiplication);

        // The pump torque's slope in the revs, for the implicit step.
        let slope = F(0.5) * (slip * (self.pump_torque_scale * (F(2.0) * sign)));
        let net = self.inv_inertia * (self.engine_torque - self.converter_torque);
        let damp = F::ONE + dt * (slope * self.inv_inertia);
        let accel = if damp <= F::ZERO { F::ONE * net } else { (F::ONE / damp) * net };
        self.rpm = self.rpm + F(RAD_TO_RPM) * (dt * accel);
        self.rpm = self.rpm.max(self.min_rpm).min(self.redline);

        let drive =
            if pedals.brake <= F::ZERO { self.ratio * (share * (F::ONE - self.transmission_loss)) } else { F::ZERO };
        let pairs: &[(usize, usize)] = if self.four_wheel { &[(0, 1), (2, 3)] } else { &[(0, 1)] };
        for &(a, b) in pairs {
            let (cut_a, cut_b) = if self.tcs && wheels[a].contact && wheels[b].contact {
                (self.traction(wheels[a].slip_ratio), self.traction(wheels[b].slip_ratio))
            } else {
                (F::ONE, F::ONE)
            };
            self.drive_torque[a] = self.output_torque * (cut_a * drive);
            self.drive_torque[b] = self.output_torque * (cut_b * drive);
            wheels[a].drive_torque = wheels[a].drive_torque + self.drive_torque[a];
            wheels[b].drive_torque = wheels[b].drive_torque + self.drive_torque[b];
        }

        self.sound(wheels, dt, curve, noise);
    }

    /// The engine sound's pitch, the last thing the step works out.
    fn sound(&mut self, wheels: &[DriveTire; 4], dt: F, curve: &CurveTable, noise: &mut impl FnMut(F) -> F) {
        let (lo, hi) = (self.min_rpm, self.redline);
        let floor = F(PITCH_FLOOR);
        self.pitch = if lo == hi {
            floor
        } else {
            let t = (self.rpm - lo) / (hi - lo);
            Acc::adda(F::ZERO, floor).madd(t, F::ONE - floor).max(floor.min(F::ONE)).min(floor.max(F::ONE))
        };
        if !(self.pitch <= PITCH_HIGH) {
            self.pitch = self.pitch + F(0.0125) * curve.coarse_sin(F(35.0) * self.sound_clock);
        } else if self.pitch < PITCH_LOW {
            self.pitch = self.pitch + F(0.11) * noise(F(0.25) * self.sound_clock);
        }
        let squat = F(0.5) * (wheels[0].compression + wheels[1].compression) / wheels[0].max_compression;
        if !(squat < floor) {
            self.pitch = self.pitch - floor * (squat - floor);
        }
        self.sound_clock = self.sound_clock + dt;
        let t = (self.pitch - F::ZERO) / (F::ONE - F::ZERO);
        self.pitch = Acc::adda(F::ZERO, lo).madd(t, hi - lo).max(lo.min(hi)).min(lo.max(hi));
    }
}
