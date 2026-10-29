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

use crate::phy::{DEG, Ini, MAX_SURFACES, MPH, Part, Section, damping, spring_rate};
use ps2_float::{F, Vf};

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
