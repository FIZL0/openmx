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
    pub surfaces: Vec<Surface>,    /// Not a key: what Game Modifiers' Car acceleration raises a driven tire's forward grip by while
    /// it drives the car forward (P373), held under what would lift the front tires off the floor
    /// ([`Wheel::forces`]). 1 as read.
    pub drive_grip: F,
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

// --- MX vs. ATV's car files (MD39b) ------------------------------------------------------------------

/// MX vs. ATV's five cars, by the machine kind word its vehicle table gives them (4 to 8). The PC
/// factory builds the buggy, the baja and the sand rail with one class, the golf cart and the monster
/// truck with their own; all three derive from MX Unleashed's car class and read its file with its
/// loader ([`Car::read`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MxvatvMachine {
    Buggy,
    MonsterTruck,
    Baja,
    GolfCart,
    Sandrail,
}

/// `(0, 5.1, -0.6, 1)`: `FPCameraOffset`'s default for every machine but the quad, whose is `4.1` up.
const FP_CAMERA_OFFSET: Vf = Vf([0.0, 5.1, -0.6, 1.0]);
/// `(1, 1, 1, 1)`: `WreckDamping`'s default.
const ONES: Vf = Vf([1.0, 1.0, 1.0, 1.0]);
/// The string getter's buffer: the name keeps at most this many bytes less its terminator.
const CAMERA_FILE_LEN: usize = 0x20;

impl MxvatvMachine {
    /// All five, in kind-word order.
    pub const ALL: [MxvatvMachine; 5] = [
        MxvatvMachine::Buggy,
        MxvatvMachine::MonsterTruck,
        MxvatvMachine::Baja,
        MxvatvMachine::GolfCart,
        MxvatvMachine::Sandrail,
    ];

    /// The machine kind word, which also names the file's `vehicletype_<n>.res`.
    pub fn kind_word(self) -> u32 {
        match self {
            MxvatvMachine::Buggy => 4,
            MxvatvMachine::MonsterTruck => 5,
            MxvatvMachine::Baja => 6,
            MxvatvMachine::GolfCart => 7,
            MxvatvMachine::Sandrail => 8,
        }
    }

    /// The file inside that `.res`, as the archive spells it.
    pub fn phy(self) -> &'static str {
        match self {
            MxvatvMachine::Buggy => "Veh\\buggy.phy",
            MxvatvMachine::MonsterTruck => "Veh\\monstertruck.phy",
            MxvatvMachine::Baja => "Veh\\baja.phy",
            MxvatvMachine::GolfCart => "Veh\\golfcart.phy",
            MxvatvMachine::Sandrail => "Veh\\sandrail.phy",
        }
    }

    /// What [`Car::read`] reads it as. Only the truck's class reads anything past the car's loader
    /// that the step keeps (`RearTireSteerScale`); the golf cart's rider boxes are the picture's.
    pub fn kind(self) -> Kind {
        match self {
            MxvatvMachine::MonsterTruck => Kind::MonsterTruck,
            MxvatvMachine::Baja => Kind::Baja,
            MxvatvMachine::Buggy | MxvatvMachine::GolfCart | MxvatvMachine::Sandrail => Kind::Buggy,
        }
    }

    /// The follow camera's file before `[Camera]` is read, set from the kind word when the machine is
    /// made.
    pub fn default_camera_file(self) -> &'static str {
        match self {
            MxvatvMachine::Buggy => "FC_BuggyDefault.pb",
            MxvatvMachine::MonsterTruck => "FC_MonsterDefault.pb",
            MxvatvMachine::Baja => "FC_BajaDefault.pb",
            MxvatvMachine::GolfCart => "FC_GolfCartDefault.pb",
            MxvatvMachine::Sandrail => "FC_SandrailDefault.pb",
        }
    }
}

/// **MX vs. ATV's car file**, as SLUS-21104's car loads it: MX Unleashed's car loader with the same
/// keys, defaults and order ([`Car::read`]), wrapped by the two loaders every MX vs. ATV machine
/// shares.
///
/// * Before the car's own keys, MXVehicle's base loader reads `[Mass Properties]` through the part
///   reader and then `[Wrecks]`: `MaxWreckTime` (default 5), `WreckGravity` (3), `WreckDamping`
///   (ones) and `WreckInertia` (`(0, 0, 0, 1)`). `MaxWreckTime` is read there and only there, so
///   [`Car::max_wreck_time`] is the same key with the same default, read first.
/// * After them, the machine's camera loader reads `[Camera]`: the two first-person shoulder offsets
///   (`(0, 0, 0, 1)`), `FPCameraOffset` (`(0, 5.1, -0.6, 1)` for a car) and `FCamFileName`, which keeps
///   [`MxvatvMachine::default_camera_file`] when the key is missing.
#[derive(Clone, Debug, PartialEq)]
pub struct MxvatvCar {
    pub machine: MxvatvMachine,
    pub car: Car,
    pub wrecks: MxvatvWrecks,
    pub camera: MxvatvCamera,
}

/// MXVehicle's `[Wrecks]` keys past `MaxWreckTime` (which is [`Car::max_wreck_time`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MxvatvWrecks {
    /// `WreckGravity`: the gravity scale while wrecked.
    pub wreck_gravity: F,
    /// `WreckDamping`, `WreckInertia`: per-axis quadwords for the wrecked body.
    pub wreck_damping: Vf,
    pub wreck_inertia: Vf,
}

/// `[Camera]`.
#[derive(Clone, Debug, PartialEq)]
pub struct MxvatvCamera {
    /// `Left1stPerShoulderOffset`, `Right1stPerShoulderOffset`.
    pub left_shoulder_offset: Vf,
    pub right_shoulder_offset: Vf,
    /// `FPCameraOffset`.
    pub fp_camera_offset: Vf,
    /// `FCamFileName`: the follow camera's presets file in `Cam\CameraPresets.res`.
    pub camera_file: String,
}

impl Car {
    /// Read MX vs. ATV's file out of its bytes, for one of its five cars.
    pub fn read_mxvatv_bytes(bytes: &[u8], machine: MxvatvMachine) -> MxvatvCar {
        Car::read_mxvatv(&Ini::parse_bytes(bytes), machine)
    }

    /// Load MX vs. ATV's file as SLUS-21104's car does ([`MxvatvCar`]). With MX Unleashed's files
    /// nothing changes: [`Car::read`] is the switch's other side.
    pub fn read_mxvatv(ini: &Ini, machine: MxvatvMachine) -> MxvatvCar {
        let s = ini.section("Wrecks");
        let max_wreck_time = s.float_or("MaxWreckTime", F(5.0));
        let wrecks = MxvatvWrecks {
            wreck_gravity: s.float_or("WreckGravity", F(3.0)),
            wreck_damping: s.vector("WreckDamping", ONES),
            wreck_inertia: s.vector("WreckInertia", UNIT_W),
        };
        let mut car = Car::read(ini, machine.kind());
        car.max_wreck_time = max_wreck_time;

        let s = ini.section("Camera");
        let camera_file = match s.text("FCamFileName") {
            Some(name) => {
                let mut end = name.len().min(CAMERA_FILE_LEN - 1);
                while !name.is_char_boundary(end) {
                    end -= 1;
                }
                name[..end].to_string()
            }
            None => machine.default_camera_file().to_string(),
        };
        let camera = MxvatvCamera {
            left_shoulder_offset: s.vector("Left1stPerShoulderOffset", UNIT_W),
            right_shoulder_offset: s.vector("Right1stPerShoulderOffset", UNIT_W),
            fp_camera_offset: s.vector("FPCameraOffset", FP_CAMERA_OFFSET),
            camera_file,
        };
        MxvatvCar { machine, car, wrecks, camera }
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
            drive_grip: F::ONE,
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
    /// First gear as the loader puts the box in it, off these ratios and the table's redline: what
    /// [`Engine::first`] holds, worked out again after a ratio is moved (P272's car top speed).
    pub fn first_gear(&self) -> Option<FirstGear> {
        self.gear_ratios.first().map(|&g| {
            let ratio = self.final_gear_ratio * g;
            let inv_ratio = F::ONE / ratio;
            let redline = self.max_rpm.unwrap_or(F::ZERO);
            FirstGear { ratio, inv_ratio, top_wheel_speed: redline * RPM_TO_RAD * inv_ratio }
        })
    }

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

// --- the controls ------------------------------------------------------------------------------------
//
// Once a frame, in the vehicle's input step, the pad's commands land in the car as plain values
// ([`Controls::input`]). Then the dynamics world's preforce listener runs the controls step
// ([`Controls::step`]) on the frame's `dt`: the steer through its rate limit and the speed's steer
// table, the rear steer, the brakes and gas through theirs, the clutch, the hand brake's friction and
// its fade, and the yaw rate limit the measure holds the body to ([`hold_yaw_rate`]). Both pedals held
// at once is the original's reverse: the step hands the gearbox the request ([`Reverse`]) and the
// caller puts the box in or out of it.

/// The car's pad state and what its controls make of it. The offsets are the car object's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Controls {
    /// +0x4ac: someone is driving. Without a driver the step clears every control.
    pub driven: bool,
    /// +0x4b0: the file's `SquareSteeringInput`, as the car keeps it.
    pub square_steering: bool,
    /// +0x4b8, +0x4bc: the stick's steer and lean as the input step was handed them, the d-pad's ±1
    /// written over them. Squared steering squares the steer in place, in the step.
    pub stick: F,
    pub lean: F,
    /// +0x4b4: the d-pad steered last. Set by a d-pad press; only a stick to the right clears it.
    pub dpad: bool,
    /// +0x3e8: a word of the pad record (+0x31c) the input step copies in; the preload reads its size.
    pub pad_word: F,
    /// +0x3e0 gas, +0x3dc brake, +0x3e4 clutch, +0x3fc hand brake: the pedals as pressed.
    pub gas: F,
    pub brake: F,
    pub clutch: F,
    pub hand_brake: F,
    /// +0x4c0: the steer after its rate limit.
    pub steer_input: F,
    /// +0x400, +0x404: the steer angles the front and the rear tires read.
    pub steer: F,
    pub rear_steer: F,
    /// +0x3f4, +0x3f8: the brakes the front and the rear tires read.
    pub front_brake: F,
    pub rear_brake: F,
    /// +0x3ec: the gas after its rate limit, the engine's throttle.
    pub throttle: F,
    /// +0x3f0: the clutch as the engine reads it.
    pub engaged_clutch: F,
    /// +0x428: what is left of the hand brake's fade.
    pub hand_brake_fade: F,
    /// +0x540: the yaw rate the measure holds the body to.
    pub yaw_rate_limit: F,
    /// The rear tires' friction scale (each rear's +0x9b4), which the hand brake lowers.
    pub rear_friction: F,
}

/// The gearbox move the controls ask for on their way through: the original calls the engine's
/// reverse switch from the middle of the step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reverse {
    /// Both pedals down: into reverse, first gear's ratio negated.
    In,
    /// Out of reverse, into first.
    Out,
}

/// The throttle both pedals together hold the car to while it reverses.
const REVERSE_THROTTLE: f32 = f32::from_bits(0x3ecc_cccd);
/// The speed over which the hand brake's brake scale comes in, feet per second.
const HAND_BRAKE_SPEED: f32 = 10.0;
/// The hand brake's command (Circle by default), which the bindings have no name for.
const HAND_BRAKE: crate::bindings::Command = crate::bindings::Command(0x16);

/// `now` moved toward `want` by at most `rate`: the original's max/min pair, then the add.
fn rate_limit(now: F, want: F, rate: F) -> F {
    now + (want - now).max(-rate).min(rate)
}

/// A pedal through its rate limit: it rises no faster than `rate`, and drops at once.
fn pedal(now: F, want: F, rate: F) -> F {
    if want <= now { want } else { rate_limit(now, want, rate) }
}

/// A three-point `(speed, scale)` table at `speed`: the first segment below the second point's
/// speed, the second at or above it, each clamped to its ends; a segment with no length is its
/// first scale.
fn steer_scale(table: &[[F; 2]; 3], speed: F) -> F {
    let [a, b, c] = *table;
    if speed < b[0] {
        crate::motion::remap(speed, a[0], b[0], a[1], b[1])
    } else {
        crate::motion::remap(speed, b[0], c[0], b[1], c[1])
    }
}

impl Controls {
    /// The car as built: no pedals, the brakes released (1 is "no brake" only until the first step
    /// writes them), the yaw limit the file's.
    pub fn new(car: &Car) -> Controls {
        Controls {
            driven: false,
            square_steering: car.joystick.square_steering_input != 0,
            stick: F::ZERO,
            lean: F::ZERO,
            dpad: false,
            pad_word: F::ZERO,
            gas: F::ZERO,
            brake: F::ZERO,
            clutch: F::ZERO,
            hand_brake: F::ZERO,
            steer_input: F::ZERO,
            steer: F::ZERO,
            rear_steer: F::ZERO,
            front_brake: F::ONE,
            rear_brake: F::ONE,
            throttle: F::ZERO,
            engaged_clutch: F::ZERO,
            hand_brake_fade: F::ZERO,
            yaw_rate_limit: car.yaw_rate_limit,
            rear_friction: F::ONE,
        }
    }

    /// The vehicle's input step: the stick, the pad word, and each pedal's command. `speed` is the
    /// measure's speed (+0x55c) and `side_slip` its sideways angle (+0x564): slow, or sliding past
    /// `DigitalSideSlipAngle`, the gas is the button's pressure over `MaxAnalogThrottle`; otherwise
    /// any press is the whole pedal. `value` is a command's strength on the player's pad
    /// ([`crate::bindings::Table::value`]).
    pub fn input(
        &mut self,
        car: &Car,
        stick: (F, F),
        pad_word: F,
        speed: F,
        side_slip: F,
        value: impl Fn(crate::bindings::Command) -> F,
    ) {
        use crate::bindings::Command;
        let j = &car.joystick;
        (self.stick, self.lean) = stick;
        self.pad_word = pad_word;
        let gas = value(Command::GAS);
        self.gas = if speed <= j.digital_throttle_speed || j.digital_side_slip_angle <= side_slip.abs() {
            let max = j.max_analog_throttle;
            if max != 0.0 { crate::motion::remap(gas, F::ZERO, max, F::ZERO, F::ONE) } else { F::ZERO }
        } else if gas > 0.0 {
            F::ONE
        } else {
            F::ZERO
        };
        self.brake = if value(Command::BRAKE) > 0.0 { F::ONE } else { F::ZERO };
        self.hand_brake = if value(HAND_BRAKE) > 0.0 { F::ONE } else { F::ZERO };
        self.clutch = if value(Command::CLUTCH) == 0.0 { F::ZERO } else { F::ONE };
        if value(Command::LEFT) != 0.0 {
            self.stick = F(-1.0);
            self.dpad = true;
        } else if value(Command::RIGHT) != 0.0 {
            self.stick = F::ONE;
            self.dpad = true;
        } else if self.stick > 0.0 {
            self.dpad = false;
        }
        if value(Command::UP) != 0.0 {
            self.lean = F(-1.0);
        } else if value(Command::DOWN) != 0.0 {
            self.lean = F::ONE;
        }
    }

    /// The controls step, once a frame. `wheel` is a steering wheel on the driver's controller (the
    /// wheel's table, no rate limit); `speed` the car's forward speed in its own frame (+0x588) and
    /// `ground_speed` the measure's speed (+0x55c); `reversing` the gearbox's reverse flag.
    pub fn step(&mut self, car: &Car, dt: F, wheel: bool, speed: F, ground_speed: F, reversing: bool) -> Option<Reverse> {
        let j = &car.joystick;
        if !self.driven {
            self.stick = F::ZERO;
            self.lean = F::ZERO;
            self.pad_word = F::ZERO;
            self.gas = F::ZERO;
            self.brake = F::ZERO;
            self.clutch = F::ZERO;
            self.steer_input = F::ZERO;
            self.throttle = F::ZERO;
            self.steer = F::ZERO;
            self.hand_brake = F::ZERO;
            self.front_brake = F::ONE;
            self.rear_brake = F::ONE;
            self.yaw_rate_limit = car.yaw_rate_limit;
            return Some(Reverse::Out);
        }

        let scale = if self.dpad {
            let rate = if self.stick.abs() == 0.0 { j.x_dpad_rate_limit_off } else { j.x_dpad_rate_limit_on };
            self.steer_input = rate_limit(self.steer_input, self.stick, rate * dt);
            steer_scale(&j.dpad_steer, speed)
        } else if wheel {
            self.steer_input = self.stick;
            steer_scale(&j.wheel_steer, speed)
        } else {
            if self.square_steering {
                let sign = if self.stick < 0.0 { -1 } else { 1 };
                self.stick = self.stick * (self.stick * F::from(sign));
            }
            self.steer_input = rate_limit(self.steer_input, self.stick, j.x_axis_rate_limit * dt);
            steer_scale(&j.steer, speed)
        };
        self.steer = scale * self.steer_input;
        let [r1, r2] = j.rear_steer;
        self.rear_steer = crate::motion::remap(speed, r1[0], r2[0], r1[1], r2[1]) * self.steer;

        let mut reverse = None;
        if self.gas == 0.0 || self.brake == 0.0 {
            if reversing {
                reverse = Some(Reverse::Out);
            }
            self.front_brake = pedal(self.front_brake, self.brake, j.brake_rate_limit * dt);
            self.rear_brake = self.front_brake;
        } else {
            reverse = Some(Reverse::In);
            self.front_brake = F::ZERO;
            self.rear_brake = F::ZERO;
            self.gas = F(REVERSE_THROTTLE);
        }
        self.throttle = pedal(self.throttle, self.gas, j.gas_rate_limit * dt);
        self.engaged_clutch = self.clutch;

        if self.hand_brake == 0.0 {
            if self.hand_brake_fade > 0.0 {
                self.rear_friction = crate::motion::remap(
                    self.hand_brake_fade,
                    F::ZERO,
                    car.hand_brake_fade_time,
                    F::ONE,
                    car.hand_brake_friction_scale,
                );
                self.hand_brake_fade = self.hand_brake_fade - dt;
            } else {
                self.rear_friction = F::ONE;
                self.yaw_rate_limit = car.yaw_rate_limit;
            }
        } else {
            self.rear_friction = car.hand_brake_friction_scale;
            self.yaw_rate_limit = car.hand_brake_yaw_rate_limit;
            self.rear_brake = crate::motion::remap(
                ground_speed,
                F::ZERO,
                F(HAND_BRAKE_SPEED),
                F::ONE,
                car.hand_brake_brake_scale,
            );
            self.hand_brake_fade = car.hand_brake_fade_time;
        }
        reverse
    }
}

/// The measure's first act each substep: the body's yaw rate (its body-frame `y`) held to the
/// controls' limit, keeping its sign.
pub fn hold_yaw_rate(rates: &mut Vf, limit: F) {
    let y = F(rates.y());
    if limit < y.abs() {
        let sign = if y < 0.0 { -1 } else { 1 };
        rates.0[1] = (limit * F::from(sign)).0;
    }
}

/// The centre of mass moved by the lean, once a frame before the world steps: the class's `CofG`
/// (the AI's `CofGAI` for a car no human drives) slid back along `z` by `CofGTravelForward` times a
/// forward lean or `CofGTravelBack` times a back one; the body's position (and its last position)
/// then moves by the change, turned into the world, so the car itself stays put. `last` is the
/// centre the last frame left (car +0x250), and comes back as this frame's.
pub fn travel_cofg(car: &Car, body: &mut crate::biplane::Body, last: &mut Vf, human: bool, lean: F) {
    let mut cofg = if human { car.mass_properties.cofg } else { car.cofg_ai };
    let travel = if lean < 0.0 { car.cofg_travel_forward } else { car.cofg_travel_back };
    cofg.0[2] = (F(cofg.z()) + (-travel) * lean).0;
    body.cofg = cofg;
    let delta = cofg.merge(Dest::XYZ, cofg.sub(*last));
    let moved = crate::chassis::turn(body.quat, delta);
    body.position = body.position.merge(Dest::XYZ, body.position.add(moved));
    body.last_position = body.last_position.merge(Dest::XYZ, body.last_position.add(moved));
    *last = cofg;
}

/// The body's centre of mass turned into the world by its orientation (the quaternion's turn of
/// `+0x180` by `+0x170`): what the car's camera writers move the body point by (S246).
pub fn turned_cofg(body: &crate::biplane::Body) -> Vf {
    crate::chassis::turn(body.quat, body.cofg)
}

// --- the car step (S149d) --------------------------------------------------------------------------

/// The angle (rad, 5°) whose cosine two of the probe's normals must agree past before it stops asking
/// the ground again.
const PROBE_AGREE: f32 = f32::from_bits(0x3db2_b8c1);
/// The most times the probe asks.
const PROBE_TRIES: i32 = 5;
/// The cosine (45°) a sorted contact's normal must stand within of the car's up, and of the axle,
/// to stay the tire's: `0x3f34fdf4`.
const SORTED_FACING: f32 = 0.707;
/// How far (ft) the terrain must rise past a sorted contact to hold the tire, and fall below it to
/// let go.
const TERRAIN_HOLD: f32 = 0.1;
/// Below this forward speed, off the gas, gravity's pull along the ground is held off.
const ANTI_SLIDE_SPEED: f32 = 10.0;
/// The preload winds up only climbing faster than this and going faster than [`PRELOAD_SPEED`].
const PRELOAD_RISE: f32 = 5.0;
const PRELOAD_SPEED: f32 = 30.0;
/// The lean that arms the preload (forward of it) and fires it (back past its negative).
const PRELOAD_LEAN: f32 = f32::from_bits(0x3f4c_cccd);
/// Half the air's density: the wings' dynamic pressure per (ft/s)².
const AIR: f32 = f32::from_bits(0x3a9b_c5c4);
/// The air control works only faster than this.
const AIR_CONTROL_SPEED: f32 = 10.0;
/// The unroll and unyaw work with the stick inside this, and the car's up `y` above [`UPRIGHT`].
const UNROLL_STICK: f32 = f32::from_bits(0x3dcc_cccd);
const UPRIGHT: f32 = f32::from_bits(0x3eae_147b);
/// Slower than this on the gas, touching something, the car counts as stuck.
const STUCK_SPEED: f32 = 10.0;
/// How near the map's edge throws the car.
const EDGE_MARGIN: f32 = 150.0;
/// The edge throw's arc (S337): the fall its time is reckoned on, the farthest it carries, and its
/// lift's two terms, `rate · k + reach / k`.
const THROW_G: f32 = f32::from_bits(0x413c_d1e3);
const THROW_REACH: f32 = f32::from_bits(0x44c9_9999);
const THROW_LIFT_RATE: f32 = f32::from_bits(0x4180_cccd);
const THROW_LIFT_REACH: f32 = 190.0;
/// The thrown car's random spin about x, y and z, each times a `rand()` unit.
const THROW_SPIN: [f32; 3] = [0.5, f32::from_bits(0x3ecc_cccd), f32::from_bits(0x3f33_3333)];
/// What of its velocity a car keeps when the edge throw's wreck ends.
const UNWRECK_KEEP: f32 = f32::from_bits(0x3e99_999a);
/// [`Machine::edge_scale`] as the vehicle's constructor sets it.
const EDGE_SCALE: f32 = 20.0;

/// One corner of the car: the tire's own state and the words its probe and suspension keep.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wheel {
    pub s: TireState,
    /// Where the axle's top hangs, in the car's frame.
    pub mount: Vf,
    /// The spin the engine (or the constructor) holds the wheel to.
    pub top_wheel_speed: F,
    /// The last ground normal the probe found (world), and it in the car's frame.
    pub probe_normal: Vf,
    pub car_normal: Vf,
    /// The axle's drop from the mount to the bottom of the wheel, in the car's frame: the radius
    /// down the normal's `yz`.
    pub drop: Vf,
    /// The axle's bottom against the centre of mass, in the car's frame (the hand-off's without it).
    pub lever: Vf,
    /// The axle's top and bottom in the world.
    pub upper: Vf,
    pub lower: Vf,
    /// How many times this frame's probe asked the ground.
    pub probes: i32,
    /// The probe met the ground: the point and the unit normal it found.
    pub solid: bool,
    pub ground_point: Vf,
    pub ground_normal: Vf,
    /// The contact point's world velocity, the body's, and the body's in the car's frame.
    pub point_velocity: Vf,
    pub body_velocity: Vf,
    pub local_velocity: Vf,
    /// The tire's frame on the ground: right, up (the normal), forward along the steer.
    pub frame: crate::math::Mat4,
    /// A front tire of a car only its fronts touch: its force puts no moment on the body.
    pub lone_front: bool,
    /// The contact sort handed the tire this frame's ground (S229a, [`Machine::sort_contacts`]):
    /// `ground_point` and `ground_normal` are a scenery mesh's, where the tire's sort ray met it.
    pub sorted: bool,
    /// The terrain holds the tire over its sorted contact: taken once the terrain under the axle
    /// stands 0.1 ft above the sorted point (or at once when the sorted normal leans 45° or more off
    /// the axle), let go once it is 0.1 ft below. Kept from frame to frame.
    pub terrain_held: bool,
}

/// What the car's step asks of whatever owns it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarEvent {
    /// Near the map's edge: the edge throw, wreck reason 13.
    Thrown,
    /// Stuck on the gas for `MaxWreckTime`: the car is reset.
    Reset,
    /// A wrecked car that left the ground has come back down on it.
    Unwreck,
}

/// The launch: the clutch's torque scale after a clutch drop, from the file's
/// `MaxClutchTorqueScale` (less with speed up to `MaxClutchTorqueScaleSpeed`) back to 1 over
/// `ClutchTorqueScaleTime`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Launch {
    pub timer: F,
    /// The scale the launch started at.
    pub start: F,
    pub time: F,
    pub max_scale: F,
    pub max_scale_speed: F,
}

/// The preload: a forward lean arms it, the time to a back lean winds it, and the back lean pushes
/// the car up for `PreloadTime`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PreloadState {
    /// It may wind up this frame.
    pub ready: bool,
    /// Pushing now, and armed by a forward lean.
    pub active: bool,
    pub armed: bool,
    pub time: F,
    /// The wind-up while armed; the push's time left while active.
    pub timer: F,
    pub min_time: F,
    pub max_time: F,
    pub max_preload: F,
    /// The push (world), its `y` set by the pad word when it fires.
    pub force: Vf,
}

/// A whole car on the ground: its body, controls, engine and four corners, and what its measure
/// reads off them once a frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Machine {
    pub body: crate::biplane::Body,
    /// The body's weight (world).
    pub gravity: Vf,
    pub controls: Controls,
    pub drivetrain: Drivetrain,
    pub wheels: [Wheel; 4],
    /// The centre of mass the last frame left ([`travel_cofg`]).
    pub last_cofg: Vf,
    /// A person drives (the class's `CofG`, not the AI's).
    pub human: bool,
    /// The driver's controller is a steering wheel.
    pub steering_wheel: bool,
    /// Wrecked, and (since) off the ground and clear of everything.
    pub wrecked: bool,
    pub wreck_lifted: bool,
    /// The collision object touches something this frame.
    pub touching: bool,
    /// Touching something and none of the tires down.
    pub airborne_touch: bool,
    /// Time stuck on the gas.
    pub stuck_time: F,
    pub launch: Launch,
    pub preload: PreloadState,
    /// Two flags the measure passes along, the first cleared once the second has seen it.
    pub latch: bool,
    pub latched: bool,
    /// Any tire down, and last frame's.
    pub grounded: bool,
    pub was_grounded: bool,
    /// The body's speed, its velocity in the car's frame, and a grounded tire's view of it.
    pub ground_speed: F,
    pub velocity: Vf,
    pub ground_velocity: Vf,
    /// The rates in the world, the angle of attack and the side slip.
    pub world_rates: Vf,
    pub alpha: F,
    pub side_slip: F,
    pub euler: Vf,
    /// The car's frame at its origin (the body's position less the centre of mass), and its
    /// forward and up rows.
    pub matrix: crate::math::Mat4,
    pub forward: Vf,
    pub up: Vf,
    /// What the last measure asked for.
    pub event: Option<CarEvent>,
    /// The vehicle's scale the edge throw halves and the unwreck doubles back ([`EDGE_SCALE`] to
    /// start). Nothing in the car reads it; kept as the original keeps it.
    pub edge_scale: F,
}

impl Wheel {
    /// The axle's top in the world: the mount against the centre of mass.
    fn upper_point(&self, body: &crate::biplane::Body) -> Vf {
        use crate::contact::{minus, plus};
        plus(crate::chassis::turn(body.quat, minus(self.mount, body.cofg)), body.position)
    }

    /// The axle's bottom: the mount and drop against the centre of mass, in the world.
    fn lower_point(&mut self, body: &crate::biplane::Body) {
        use crate::contact::{minus, plus};
        self.lever = minus(plus(self.mount, self.drop), body.cofg);
        self.lower = plus(crate::chassis::turn(body.quat, self.lever), body.position);
    }

    /// The probe, once a frame: the axle's bottom along last frame's normal, and the ground under it,
    /// asked again from the new normal until two agree within 5° or it has asked five times. The
    /// wheel is solid on ground at or above its bottom.
    ///
    /// A tire the contact sort handed a contact ([`Wheel::sorted`]) keeps it unless the sorted normal
    /// is 45° or more off the car's `up` (`up · n < 0.707`); with ground at or above its bottom, it
    /// takes the terrain instead only while the terrain holds it ([`Wheel::terrain_held`]).
    pub fn probe(&mut self, tire: &Tire, body: &crate::biplane::Body, up: Vf, world: &mut impl crate::world::World, curve: &CurveTable) {
        use crate::contact::{dot, minus, scaled, unit};
        let agree = curve.coarse(F(PROBE_AGREE)).0;
        self.solid = false;
        self.upper = self.upper_point(body);
        self.probes = 0;
        let mut d = F::ZERO;
        let (mut point, mut normal) = (self.lower, self.probe_normal);
        while d < agree && self.probes < PROBE_TRIES {
            self.car_normal = crate::biplane::unturn(body.quat, self.probe_normal);
            let dir = unit(Vf::new(0.0, self.car_normal.y(), self.car_normal.z(), 1.0));
            self.drop = scaled(dir, -tire.radius);
            self.lower_point(body);
            (point, normal) = world.ground(self.lower);
            if let Some(surface) = world.wheel_surface(self.lower) {
                self.s.surface = usize::from(surface);
            }
            normal = unit(normal);
            d = dot(normal, self.probe_normal);
            self.probe_normal = normal;
            self.probes += 1;
        }
        let facing = |a: Vf, b: Vf| Acc::mula(a.x(), b.x()).madda(a.y(), b.y()).madd(a.z(), b.z());
        if self.sorted && facing(up, self.ground_normal) < F(SORTED_FACING) {
            self.sorted = false;
        }
        if point.y() < self.lower.y() {
            return;
        }
        if self.sorted {
            let (ground, sorted) = (F(point.y()), F(self.ground_point.y()));
            let axle = unit(minus(self.upper, self.lower));
            if facing(axle, self.ground_normal) < F(SORTED_FACING) {
                self.terrain_held = true;
            } else if !self.terrain_held {
                if !(ground <= F(TERRAIN_HOLD) + sorted) {
                    self.terrain_held = true;
                }
            } else if !(sorted <= F(TERRAIN_HOLD) + ground) {
                self.terrain_held = false;
            }
            if !self.terrain_held {
                return;
            }
            self.sorted = false;
        }
        self.solid = true;
        self.ground_point = point;
        self.ground_normal = normal;
    }

    /// The contact sort's hand-off (S229a): a sort ray's hit on a scenery mesh, its point and unit
    /// normal, as this frame's ground. The struck object and its mover are not kept: no capture has
    /// a tire sorted onto anything that moves ([`Machine::sort_contacts`]). The normal in the car's
    /// frame and the drop along it the original works out here too are worked out again by the
    /// probe, which always follows.
    pub fn set_contact(&mut self, point: Vf, normal: Vf) {
        self.ground_point = point;
        self.ground_normal = normal;
        self.sorted = true;
    }

    /// The contact point's velocity: the body's plus the spin's about the centre, and the body's in
    /// its own frame.
    fn contact_velocity(&mut self, body: &crate::biplane::Body) {
        use crate::contact::{minus, plus};
        let q = body.quat;
        let local = crate::biplane::unturn(q, minus(self.s.contact, body.position));
        let r = body.rates;
        let spin = r.merge(Dest::XYZ, Vf::opmsub(Vf::opmula(r, local), local, r));
        let pv = plus(body.velocity, crate::chassis::turn(q, spin));
        self.point_velocity = minus(pv, UNIT_W);
        self.body_velocity = minus(body.velocity, UNIT_W);
        self.local_velocity = crate::biplane::unturn(q, self.body_velocity);
    }

    /// The suspension, each substep: the contact on the probe's ground, the body pushed out of it
    /// when the tire bottoms out, and the spring's load at the contact. A wheel in the air carries the
    /// load handed it, its velocity from the axle's bottom without the centre of mass.
    pub fn suspension(&mut self, tire: &Tire, body: &mut crate::biplane::Body, h: F) {
        use crate::contact::{dot, plus, scaled};
        self.s.compression = F::ZERO;
        self.s.grounded = false;
        if self.solid || self.sorted {
            self.upper = self.upper_point(body);
            self.lower_point(body);
            // Only the terrain bottoms the tire out whatever the depth; a sorted contact does when
            // the axle's bottom is not twice through it.
            let ground = Ground { point: self.ground_point, normal: self.ground_normal, solid: self.solid };
            let push = tire.contact(&mut self.s, (self.upper, self.lower), Some(&ground));
            if self.s.grounded {
                self.contact_velocity(body);
            }
            let n = self.ground_normal;
            if let Some(push) = push {
                body.position = plus(body.position, scaled(n, push));
                let into = dot(self.body_velocity, n).min(F::ZERO);
                body.add_velocity(scaled(n, -into));
                self.contact_velocity(body);
            }
            if self.s.grounded {
                let load = tire.spring(&mut self.s, self.point_velocity, n);
                body.add_force_at(scaled(n, load), self.s.contact, Vf::splat(1.0));
            }
        }
        if !self.s.grounded {
            if let Some(load) = self.s.handoff {
                self.s.load = load;
                self.lever = plus(self.mount, self.drop);
                self.lower = crate::chassis::turn(body.quat, self.lever);
                self.contact_velocity(body);
            } else {
                self.s.load = F::ZERO;
                self.point_velocity = UNIT_W;
            }
        }
        tire.settle(&mut self.s, h);
    }

    /// The tire's forces, each substep after the roll: a grounded tire's frame along its steer on
    /// the ground, the contact's velocity in it, [`Tire::forces`], and the force back into the world
    /// on the body (with its moment, unless only the fronts touch).
    #[allow(clippy::too_many_arguments)]
    pub fn forces(
        &mut self,
        tire: &Tire,
        body: &mut crate::biplane::Body,
        h: F,
        steer: F,
        brake: F,
        acos: &AcosTable,
        curve: &CurveTable,
    ) {
        use crate::contact::minus;
        use ps2_float::Field;
        let q = body.quat;
        let mut velocity = UNIT_W;
        if self.s.grounded {
            let angle = steer * tire.max_steer_angle;
            let forward = if angle.abs() > 0.0 {
                let (cos, sin) = curve.coarse(angle);
                crate::chassis::turn(q, Vf::new(sin.0, 0.0, cos.0, 0.0))
            } else {
                crate::chassis::turn(q, Vf::new(0.0, 0.0, 1.0, 1.0))
            };
            self.frame = crate::math::basis(UNIT_W, forward, self.ground_normal, true, false);
            let m = &self.frame.0;
            let col = |k: usize| Vf::new(m[0][k], m[1][k], m[2][k], 0.0);
            let v = self.point_velocity;
            let acc = Vf::madd(col(0).mul(v.bc(Field::X)), col(1), v.bc(Field::Y));
            velocity = v.merge(Dest::XYZ, Vf::madd(acc, col(2), v.bc(Field::Z)));
        }
        let normal = (F(self.car_normal.x()), F(self.car_normal.y()));
        tire.forces(&mut self.s, h, Some(brake), Some(steer), self.top_wheel_speed, velocity, normal, acos, curve);
        if !self.s.grounded {
            return;
        }
        if tire.drive_grip > F::ONE {
            self.drive_grip(tire, body);
        }
        let m = &self.frame.0;
        let f = self.s.force;
        let acc = Vf::madd(m[0].mul(f.bc(Field::X)), m[1], f.bc(Field::Y));
        let world = f.merge(Dest::XYZ, Vf::madd(acc, m[2], f.bc(Field::Z)));
        body.add_force(world);
        if !self.lone_front {
            let lever = minus(self.s.contact, body.position);
            let arm = Vf::opmsub(Vf::opmula(lever, world), world, lever);
            body.add_moment_local(crate::biplane::unturn(q, arm));
        }
    }
}

impl Wheel {
    /// Game Modifiers' raise of a driven tire's forward force (P373): times [`Tire::drive_grip`]
    /// while it pushes the car forward on a forward-spinning wheel, but never past `b / (h mu)`,
    /// the grip at which the drive force on this tire, with all the car's weight on it, would lift
    /// the other axle (`b` the centre of mass's distance ahead of the contact along the ground, `h`
    /// its height over it, `mu` the surface's friction). A car whose file's grip is already past
    /// that (the monster truck) gets no raise. The force read back next tick (`last_force`) is the
    /// raised one, so the wheel spins down on it.
    fn drive_grip(&mut self, tire: &Tire, body: &crate::biplane::Body) {
        use crate::contact::minus;
        let s = &mut self.s;
        let along = F(s.force.z());
        if along <= F::ZERO || s.spin <= F::ZERO {
            return;
        }
        let m = &self.frame.0;
        let lever = minus(body.position, s.contact);
        let (b, h) = (dot3(lever, m[2]), dot3(lever, m[1]));
        let mu = s.friction_scale * tire.surface(s.surface).friction_peak;
        if b <= F::ZERO || h <= F::ZERO || mu <= F::ZERO {
            return;
        }
        let g = tire.drive_grip.min(b / (h * mu));
        s.force[2] = (along * g).0;
        s.last_force[2] = s.force[2];
    }
}

impl Machine {
    /// The engine's four drive slots' tires, as the drivetrain reads them.
    fn drive_tires(&self, car: &Car) -> [DriveTire; 4] {
        car.engine.driven.map(|slot| match slot {
            Some(k) => {
                let w = &self.wheels[k];
                DriveTire {
                    contact: w.s.grounded,
                    spin: w.s.spin,
                    slip_ratio: w.s.slip_ratio,
                    compression: w.s.compression,
                    max_compression: car.tires[k].max_compression,
                    drive_torque: w.s.torque,
                    top_wheel_speed: w.top_wheel_speed,
                }
            }
            None => DriveTire::default(),
        })
    }

    /// What the drivetrain wrote back: each driven tire's torque and top speed.
    fn put_drive_tires(&mut self, car: &Car, tires: &[DriveTire; 4]) {
        for (slot, t) in car.engine.driven.iter().zip(tires) {
            if let Some(k) = *slot {
                self.wheels[k].s.torque = t.drive_torque;
                self.wheels[k].top_wheel_speed = t.top_wheel_speed;
            }
        }
    }

    /// One frame of the car: the physics world's step on its one body, with the car's three listeners.
    /// The controls' input (the pedals, stick and pad word) is set before.
    pub fn step(
        &mut self,
        car: &Car,
        world: &mut impl crate::world::World,
        dt: F,
        acos: &AcosTable,
        curve: &CurveTable,
        noise: &mut impl FnMut(F) -> F,
    ) {
        let physics = crate::biplane::World { iterations: 1, rate: F(115.0) };
        physics.step(dt, &mut Stepping { m: self, car, world, acos, curve, noise, struck: None, met: None, props: &mut [], knocks: &[] });
    }

    /// Once a frame before the substeps: the controls, the centre of mass's travel, and the probes.
    pub fn preforce(&mut self, car: &Car, world: &mut impl crate::world::World, dt: F, curve: &CurveTable) {
        self.preforce_with(car, world, dt, curve, |_, _| {});
    }

    /// [`Machine::preforce`] with the contact sort (`sort`) between the tires' clear and their probes,
    /// as [`Machine::step_struck`] has it.
    fn preforce_with<W: crate::world::World>(
        &mut self,
        car: &Car,
        world: &mut W,
        dt: F,
        curve: &CurveTable,
        sort: impl FnOnce(&mut Machine, &mut W),
    ) {
        self.body.clear_shock_peak();
        let reversing = self.drivetrain.reverse;
        let speed = F(self.velocity.z());
        if let Some(r) = self.controls.step(car, dt, self.steering_wheel, speed, self.ground_speed, reversing) {
            let mut tires = self.drive_tires(car);
            self.drivetrain.set_reverse(r == Reverse::In, &mut tires);
            self.put_drive_tires(car, &tires);
        }
        for w in &mut self.wheels[REAR_LEFT..] {
            w.s.friction_scale = self.controls.rear_friction;
        }
        travel_cofg(car, &mut self.body, &mut self.last_cofg, self.human, self.controls.lean);
        for w in &mut self.wheels {
            w.solid = false;
            w.sorted = false;
        }
        sort(self, world);
        let up = self.up;
        for (w, tire) in self.wheels.iter_mut().zip(&car.tires) {
            w.probe(tire, &self.body, up, world, curve);
        }
        let down = |w: &Wheel| w.solid || w.sorted;
        let front = down(&self.wheels[FRONT_LEFT]) || down(&self.wheels[FRONT_RIGHT]);
        let rear = down(&self.wheels[REAR_LEFT]) || down(&self.wheels[REAR_RIGHT]);
        for w in &mut self.wheels[..REAR_LEFT] {
            w.lone_front = front && !rear;
        }
    }

    /// Each substep: gravity, the anti-slide, the preload, the launch, the engine, the four tires
    /// (the rears handed the fronts' load) and the air.
    pub fn force(&mut self, car: &Car, h: F, acos: &AcosTable, curve: &CurveTable, noise: &mut impl FnMut(F) -> F) {
        self.force_with(car, h, acos, curve, noise, |_| {});
    }

    /// [`Machine::force`] with the collision object's response (`resolve`) right after gravity, as the
    /// body wrapper's force listener has it ([`Machine::step_struck`]).
    fn force_with(
        &mut self,
        car: &Car,
        h: F,
        acos: &AcosTable,
        curve: &CurveTable,
        noise: &mut impl FnMut(F) -> F,
        resolve: impl FnOnce(&mut crate::biplane::Body),
    ) {
        self.body.push_gravity(self.gravity);
        resolve(&mut self.body);
        self.anti_slide();
        self.preload_step(h);
        self.launch_step(h);
        let mut tires = self.drive_tires(car);
        let c = &self.controls;
        let pedals = Pedals { throttle: c.throttle, clutch: c.engaged_clutch, brake: c.rear_brake };
        self.drivetrain.step(pedals, &mut tires, h, curve, noise);
        self.put_drive_tires(car, &tires);
        for k in [FRONT_LEFT, FRONT_RIGHT] {
            self.tire_step(car, k, h, acos, curve);
        }
        let handoff = hand_off(&self.wheels[FRONT_LEFT].s, &self.wheels[FRONT_RIGHT].s);
        for k in [REAR_LEFT, REAR_RIGHT] {
            self.wheels[k].s.handoff = handoff;
            self.tire_step(car, k, h, acos, curve);
        }
        self.aero(car);
    }

    /// One tire's substep: the suspension, the roll and brake, the forces.
    fn tire_step(&mut self, car: &Car, k: usize, h: F, acos: &AcosTable, curve: &CurveTable) {
        let tire = &car.tires[k];
        let c = &self.controls;
        let (steer, brake) = if k < REAR_LEFT { (c.steer, c.front_brake) } else { (c.rear_steer, c.rear_brake) };
        let w = &mut self.wheels[k];
        w.suspension(tire, &mut self.body, h);
        tire.roll(&mut w.s, Some(brake));
        w.forces(tire, &mut self.body, h, steer, brake, acos, curve);
    }

    /// Slow and off the gas on the ground, gravity's pull along the ground is pushed back against,
    /// the share along the car's heading too while braking.
    fn anti_slide(&mut self) {
        use crate::contact::{dot, minus, scaled};
        let speed = F(self.velocity.z());
        if !self.grounded || self.controls.throttle != 0.0 || !(speed.abs() < ANTI_SLIDE_SPEED) {
            return;
        }
        let n = self.wheels[FRONT_LEFT].ground_normal;
        let g = self.gravity;
        let along = minus(g, scaled(n, dot(n, g)));
        let v = self.forward;
        let mut heading = minus(v, scaled(n, dot(v, n)));
        let len2 = dot(heading, heading);
        if len2 == 0.0 {
            heading = UNIT_W;
        } else {
            let r = F::ONE.rsqrt(len2);
            heading = Vf::new(
                (F(heading.x()) * r).0,
                (F(heading.y()) * r).0,
                (F(heading.z()) * r).0,
                heading.w(),
            );
        }
        let ahead = scaled(heading, dot(heading, along));
        let rest = minus(along, ahead);
        let s = crate::motion::remap(speed, F::ZERO, F(ANTI_SLIDE_SPEED), F::ONE, F::ONE);
        let mut force = scaled(rest, -s);
        if self.controls.brake != 0.0 || self.controls.hand_brake != 0.0 {
            force = minus(force, ahead);
        }
        self.body.add_force(force);
    }

    /// The preload, each substep.
    fn preload_step(&mut self, h: F) {
        if self.wrecked {
            return;
        }
        let p = &mut self.preload;
        if p.active {
            p.timer = p.timer - h;
            if !(p.timer <= 0.0) {
                self.body.add_force(p.force);
            } else {
                p.active = false;
            }
            return;
        }
        if !self.grounded
            || F(self.body.velocity.y()) <= PRELOAD_RISE
            || F(self.velocity.z()) <= PRELOAD_SPEED
            || self.controls.brake != 0.0
        {
            p.ready = false;
            return;
        }
        p.ready = true;
        let lean = self.controls.lean;
        if !(lean <= PRELOAD_LEAN) {
            p.armed = true;
            p.timer = F::ZERO;
        } else if !(lean < -PRELOAD_LEAN) {
            if p.armed {
                p.timer = p.timer + h;
            }
        } else if p.armed {
            let f = self.controls.pad_word.abs();
            p.force.0[1] = crate::motion::remap(f, p.min_time, p.max_time, F::ZERO, p.max_preload).0;
            p.timer = p.time;
            p.armed = false;
            p.active = true;
        }
    }

    /// The launch, each substep: the clutch let out from standing, the converter's boost eases from
    /// the launch's start back to 1.
    fn launch_step(&mut self, h: F) {
        let l = &mut self.launch;
        let clutch = self.controls.clutch;
        if clutch == 1.0 {
            l.timer = F::ZERO;
            return;
        }
        if clutch != 0.0 || !(l.timer <= l.time) {
            return;
        }
        if l.timer == 0.0 {
            l.start = crate::motion::remap(F(self.velocity.z()), F::ZERO, l.max_scale_speed, l.max_scale, F::ONE);
        }
        l.timer = l.timer + h;
        self.drivetrain.launch_boost = crate::motion::remap(l.timer, F::ZERO, l.time, l.start, F::ONE);
    }

    /// The air, each substep: the wings' lift and drag, and off the ground the air control —
    /// the stick's moment, the damping, the unroll and unyaw, and the brakes' gyro.
    fn aero(&mut self, car: &Car) {
        use crate::contact::scaled;
        if self.wrecked {
            return;
        }
        let a = &car.aero;
        let gs = self.ground_speed;
        if !(F(self.velocity.z()) <= 0.0) {
            let q = F(AIR) * gs * gs;
            for wing in a.wings.iter().take(a.number_of_wings.max(0) as usize) {
                let cl = wing.lift_slope * (self.alpha + wing.angle_of_attack);
                let cd = wing.basic_drag + cl * (cl * wing.induced_drag);
                let force = Vf::new(0.0, (a.aero_surface_area * (q * cl)).0, (a.aero_surface_area * (-q * cd)).0, 1.0);
                let p = wing.position;
                let m = p.merge(Dest::XYZ, Vf::opmsub(Vf::opmula(p, force), force, p));
                self.body.add_moment_local(m);
                self.body.add_force(crate::chassis::turn(self.body.quat, force));
            }
        }
        if self.grounded || self.touching || gs <= AIR_CONTROL_SPEED {
            return;
        }
        let c = &self.controls;
        let cm = a.airborne_control_moment;
        let r = self.body.rates;
        let m = Vf::new((F(cm.x()) * c.lean).0, (F(cm.y()) * c.stick).0, (F(cm.z()) * c.stick).0, 1.0);
        let mut m = m.merge(Dest::XYZ, m.sub(scaled(r, a.airborne_damping)));
        if c.stick.abs() < UNROLL_STICK && !(F(self.up.y()) <= UPRIGHT) {
            m.0[2] = (F(m.z()) + a.unroll_torque * (a.unroll_gain * F(self.euler.z()) - F(r.z()))).0;
            m.0[1] = (F(m.y()) + a.unyaw_torque * (a.unyaw_gain * self.side_slip - F(r.y()))).0;
        }
        if c.front_brake != 0.0 || c.rear_brake != 0.0 {
            m.0[0] = (F(m.x()) + a.brake_gyro_torque).0;
        }
        self.body.add_moment_local(m);
    }

    /// Once a frame after the substeps: the yaw-rate hold, the gear change, the wreck and stuck
    /// timer (and the stuck reset it calls, [`Machine::put_upright`]), and the car's speeds, angles
    /// and frame. The wreck step runs ahead of the speeds and the frame, as the original's measure
    /// calls it (C34's capture: the reset frame's speeds and frame are the reset body's, and its turn
    /// is built from the frame before's forward).
    pub fn measure(&mut self, car: &Car, world: &mut impl crate::world::World, dt: F, acos: &AcosTable) {
        use crate::contact::{dot, minus};
        hold_yaw_rate(&mut self.body.rates, self.controls.yaw_rate_limit);
        let mut tires = self.drive_tires(car);
        self.drivetrain.shift(self.controls.engaged_clutch, dt, &mut tires);
        self.put_drive_tires(car, &tires);
        self.event = self.wreck_timer(car, world, dt);
        if self.event == Some(CarEvent::Reset) {
            self.put_upright(world);
        }
        self.was_grounded = self.grounded;
        self.grounded = self.wheels.iter().any(|w| w.s.grounded);
        let b = &self.body;
        let q = b.quat;
        self.ground_speed = dot(b.velocity, b.velocity).sqrt();
        self.velocity = crate::biplane::unturn(q, b.velocity);
        self.ground_velocity = self.wheels.iter().find(|w| w.s.grounded).map_or(self.velocity, |w| w.local_velocity);
        self.world_rates = crate::chassis::turn(q, b.rates);
        let v = self.velocity;
        self.alpha = crate::biplane::flow_angle(F(v.z()), F(v.y()), acos);
        self.side_slip = crate::biplane::flow_angle(F(v.z()), F(v.x()), acos);
        self.euler = crate::math::quat_to_euler(q, acos);
        let origin = minus(b.position, crate::chassis::turn(q, b.cofg));
        self.matrix = crate::math::quat_to_mat4(q, origin);
        self.forward = self.forward.merge(Dest::XYZ, self.matrix.0[2]);
        self.up = self.up.merge(Dest::XYZ, self.matrix.0[1]);
        self.airborne_touch = self.touching && !self.grounded;
        if self.latch && self.latched {
            self.latch = false;
        }
        self.latched = self.latch;
    }

    /// **Over a seam** (MD120, the port's own; [`crate::flight::Flight::wrap_round`]'s for a car): on
    /// a world that wraps, a car whose body has crossed a seam is moved back one width on that axis —
    /// its body, its corners' places ([`crate::seam::car`]) and its collision object's record,
    /// if it has one — and the answer is the widths it was past, `[x, z]`. Its speeds,
    /// spins and attitude are not places, so it drives on as it was driving. `[0, 0]` on every world
    /// that does not wrap, which it leaves alone.
    pub fn wrap_round(&mut self, world: &dyn crate::world::World, collision: Option<&mut Collision>) -> [i8; 2] {
        let Some((laps, by)) = crate::seam::past(world, self.body.position) else { return [0, 0] };
        crate::seam::car(self, by);
        if let Some(c) = collision {
            crate::seam::collision(c, by);
        }
        laps
    }

    /// A wrecked car waits to leave the ground and come back; a whole one near the map's edge is
    /// thrown, and one stuck on the gas against something for `MaxWreckTime` is reset. Only asks:
    /// the post acts on [`CarEvent::Reset`] and [`CarEvent::Unwreck`], the owner on
    /// [`CarEvent::Thrown`] ([`Machine::throw_back`]).
    pub fn wreck_timer(&mut self, car: &Car, world: &impl crate::world::World, dt: F) -> Option<CarEvent> {
        use crate::contact::dot;
        if self.wrecked {
            if !self.grounded && !self.touching {
                self.wreck_lifted = true;
            }
            return (self.wreck_lifted && (self.grounded || self.touching)).then_some(CarEvent::Unwreck);
        }
        // On a world that wraps (MD120), only an axis that does not wrap has an edge, as the plane's
        // ([`crate::off_map::outside_axes`]); elsewhere the original's test.
        let (wrap, at) = (world.wrap(), self.body.position);
        let edge = |a: &crate::biplane::Area| match wrap {
            [false, false] => a.outside(at, F(EDGE_MARGIN)),
            _ => crate::off_map::outside_axes(a, wrap, at, F(EDGE_MARGIN)),
        };
        if world.area().is_some_and(|a| edge(&a)) {
            return Some(CarEvent::Thrown);
        }
        let v = self.ground_velocity;
        if self.controls.gas <= 0.0 || !(dot(v, v).sqrt() < STUCK_SPEED) || !self.touching {
            self.stuck_time = F::ZERO;
            return None;
        }
        self.stuck_time = self.stuck_time + dt;
        if !(self.stuck_time <= car.max_wreck_time) {
            self.stuck_time = F::ZERO;
            return Some(CarEvent::Reset);
        }
        None
    }

    /// **The stuck reset** (S335): what the original does to a car its stuck timer gives up on
    /// ([`CarEvent::Reset`]) — a car on its roof with the gas held, or one wedged against something.
    /// The ground under the body is asked where it is and which way it faces; the body is turned to
    /// stand on that ground, its nose along the car's forward as the frame before's measure left it,
    /// and moved so that a point half a foot under the ground along its normal is the model's origin.
    /// Every speed, push and spin is cleared. The integrator's history (the last position and turn) is
    /// left as it was. Bit-exact against the original's reset (C34, car_drive.rs `the_roof_reset_*`).
    pub fn put_upright(&mut self, world: &mut impl crate::world::World) {
        use crate::chassis::{CLEAR, turn};
        let (point, normal) = world.ground(self.body.position);
        // The ground's answer goes through an object that hands it back with a lift of zero, which
        // the original takes off along the normal.
        let point = point.merge(Dest::XYZ, point.sub(normal.merge(Dest::XYZ, normal.mul(Vf::splat(0.0)))));
        let q = crate::math::mat4_to_quat(&crate::math::basis(Vf::new(0.0, 0.0, 0.0, 1.0), self.forward, normal, false, true));
        self.body.quat = q;
        let below = normal.merge(Dest::XYZ, normal.mul(Vf::splat(-0.5)));
        let lift = below.merge(Dest::XYZ, below.add(turn(q, self.body.cofg)));
        self.body.position = point.merge(Dest::XYZ, point.add(lift));
        let b = &mut self.body;
        (b.velocity, b.force, b.rates, b.moment, b.twist) = (CLEAR, CLEAR, CLEAR, CLEAR, CLEAR);
    }

    /// **The edge throw** (S337): what the original's wreck step does to a car its measure finds near
    /// the map's edge ([`CarEvent::Thrown`]). The body is sent toward the middle of the map, level
    /// with where it is, as fast as an arc of half the map's width (at most [`THROW_REACH`]) needs,
    /// with the arc's lift on top, and tumbled by three `rand()` units; [`Machine::edge_scale`] is
    /// halved, and the car is wrecked (reason 13) until it has lifted off and come down again
    /// ([`Machine::unwreck`]). `area` is the map's, `rng` the session's `rand()`.
    pub fn throw_back(&mut self, area: &crate::biplane::Area, rng: &mut crate::rng::Rng) {
        let half = F(0.5);
        let mid_x = half * area.width;
        let at = self.body.position;
        let target = Vf::new(mid_x.0, at.y(), (half * area.depth).0, 1.0);
        let d = target.merge(Dest::XYZ, target.sub(at));
        let (x, y, z) = (F(d.x()), F(d.y()), F(d.z()));
        let len2 = Acc::mula(x, x).madda(y, y).madd(z, z);
        let dir = if F::ZERO == len2 {
            Vf::new(0.0, 0.0, 0.0, 1.0)
        } else {
            let k = F::ONE.rsqrt(len2);
            Vf::new((x * k).0, (y * k).0, (z * k).0, d.w())
        };
        let root = F(2.0) * F(THROW_G).sqrt();
        let speed = F(THROW_REACH).min(mid_x) / root;
        let lift = F(THROW_LIFT_RATE) * root + F(THROW_LIFT_REACH) / root;
        let v = dir.merge(Dest::XYZ, dir.mul(Vf::splat(speed.0)));
        self.body.velocity = Vf::new(v.x(), (F(v.y()) + lift).0, v.z(), v.w());
        self.edge_scale = self.edge_scale * half;
        let [sx, sy, sz] = THROW_SPIN.map(|s| (F(s) * rng.unit()).0);
        self.body.rates = Vf::new(sx, sy, sz, 1.0);
        self.wrecked = true;
        self.wreck_lifted = false;
    }

    /// **The unwreck** (S337): the edge throw's wreck ends — [`Machine::edge_scale`] doubled back,
    /// the measure's latch raised, and the body keeping [`UNWRECK_KEEP`] of its velocity.
    pub fn unwreck(&mut self) {
        if self.wrecked {
            self.edge_scale = self.edge_scale / F(0.5);
        }
        self.wrecked = false;
        self.latch = true;
        let v = self.body.velocity;
        self.body.velocity = v.merge(Dest::XYZ, v.mul(Vf::splat(UNWRECK_KEEP)));
    }
}

/// The car's listeners on the physics world: one body, and one of each list but the constraints.
struct Stepping<'a, W, N> {
    m: &'a mut Machine,
    car: &'a Car,
    world: &'a mut W,
    acos: &'a AcosTable,
    curve: &'a CurveTable,
    noise: &'a mut N,
    /// The collision object, searched already this frame, and the movers its contacts name.
    struck: Option<(&'a mut Collision, &'a [crate::contact::Mover])>,
    /// The riders' bikes its mirrors name (F26), the others of its resolve.
    met: Option<&'a mut crate::flight::Met>,
    /// The knockable props it struck (S335), the others of its resolve after the bikes.
    props: &'a mut [crate::prop_contact::Pushed],
    /// The knockable props' meshes, in its tires' sort as in its search (S335).
    knocks: &'a [crate::flight_search::Knock<'a>],
}

impl<W: crate::world::World, N: FnMut(F) -> F> crate::biplane::Dynamics for Stepping<'_, W, N> {
    fn count(&self, list: crate::biplane::List) -> i32 {
        i32::from(list != crate::biplane::List::Constraints)
    }
    fn preforce(&mut self, _: i32, dt: F) {
        match &mut self.struck {
            None => self.m.preforce(self.car, self.world, dt, self.curve),
            Some((c, _)) => {
                let knocks = self.knocks;
                self.m.preforce_with(self.car, self.world, dt, self.curve, |m, world| m.sort_contacts_knocking(self.car, c, world, knocks))
            }
        }
    }
    fn force(&mut self, _: i32, h: F) {
        match &mut self.struck {
            None => self.m.force(self.car, h, self.acos, self.curve, self.noise),
            Some((c, movers)) => {
                let (met, props) = (&mut self.met, &mut *self.props);
                self.m.force_with(self.car, h, self.acos, self.curve, self.noise, |body| {
                    let mut weighed = crate::contact::Weighed { body, mass: c.mass };
                    let mut others: Vec<crate::contact::Other<'_>> = match met.as_deref_mut() {
                        Some(m) => m
                            .colliders
                            .iter_mut()
                            .zip(m.vehicles.iter_mut())
                            .map(|(collider, v)| crate::contact::Other { collider, side: crate::contact::Side::Vehicle(v) })
                            .collect(),
                        None => Vec::new(),
                    };
                    let mut bodies: Vec<_> = props.iter_mut().map(|p| p.weighed()).collect();
                    others.extend(
                        bodies.iter_mut().map(|(collider, body)| crate::contact::Other { collider: &mut **collider, side: crate::contact::Side::Body(body) }),
                    );
                    crate::contact::resolve(&mut c.collider, &mut weighed, &mut c.record.contacts, &mut others, movers, true);
                })
            }
        }
    }
    fn enabled(&self, _: i32) -> bool {
        self.m.body.enabled
    }
    fn integrate(&mut self, _: i32, h: F) {
        self.m.body.integrate(h);
    }
    fn end_substep(&mut self, _: i32) {
        self.m.body.end_substep();
    }
    fn constrain(&mut self, _: i32, _: bool) {}
    fn post(&mut self, _: i32, dt: F) {
        // Over a seam (MD120), first: the refill gathers where the car now is.
        self.m.wrap_round(&*self.world, self.struck.as_mut().map(|(c, _)| &mut **c));
        if let Some((c, _)) = &mut self.struck {
            match self.met.as_deref().filter(|m| strikes_bikes(m)) {
                Some(m) => refresh_met(c, &self.m.body, &mut *self.world, m),
                None => c.refresh(&self.m.body, &mut *self.world),
            }
        }
        self.m.measure(self.car, self.world, dt, self.acos);
        // The unwreck runs in the post's wreck step, after the measure (S337); the stuck reset runs
        // inside the measure (C34). The edge throw draws on the session's `rand()`, which the owner
        // holds ([`Machine::throw_back`]).
        if self.m.event == Some(CarEvent::Unwreck) {
            self.m.unwreck();
        }
    }
}

/// The model nodes the car's constructor hangs its tires on, in [`TIRES`] order: the model's own
/// names, the rears' with an underscore.
pub const TIRE_NODES: [&str; 4] = ["FrontLeftTire", "FrontRightTire", "RearLeft_Tire", "RearRight_Tire"];

/// What the car's constructor reads off its model (`<dir>\anim\<name>.psg`, the livery's): where each
/// tire's node stands in the model's frame, and the bounds of the whole model.
#[derive(Clone, Debug, PartialEq)]
pub struct CarModel {
    /// Each [`TIRE_NODES`] node's origin, its local matrices composed down from the root, `w` 1.
    pub tire_nodes: [Vf; 4],
    /// The root's box widened by every node under it ([`crate::prop_world::node_bounds`]).
    pub bounds: crate::props::NodeBox,
}

impl CarModel {
    /// A car's model out of its `.psg` bytes. An error for a file that is no `.psg`, or that lacks
    /// one of the four tire nodes.
    pub fn read(psg: &[u8]) -> Result<CarModel, String> {
        use crate::math::{Mat4, mul};
        use crate::prop_world::BoundsNode;
        use crate::props::NodeBox;
        const NODES: usize = 12;
        const RECORD: usize = 224;
        const NAME: usize = 128;
        const MATRIX: usize = 128;
        const CENTRE: usize = 192;
        const HALF: usize = 204;
        const PARENT: usize = 220;
        let word = |at: usize| -> Result<u32, String> {
            psg.get(at..at + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .ok_or_else(|| format!(".psg ends at {} before byte {at}", psg.len()))
        };
        if psg.get(..4) != Some(b"psg\0".as_slice()) {
            return Err("not a .psg: no psg\\0 at the front".into());
        }
        let count = word(8)? as usize;
        let at = |i: usize| NODES + i * RECORD;
        let mut parents = Vec::with_capacity(count);
        let mut names = Vec::with_capacity(count);
        let mut worlds: Vec<Mat4> = Vec::with_capacity(count);
        for i in 0..count {
            let parent = word(at(i) + PARENT)? as i32;
            let name = psg.get(at(i)..at(i) + NAME).ok_or("a .psg cut short in its nodes")?;
            names.push(name.iter().take_while(|&&c| c != 0).map(|&c| c as char).collect::<String>());
            let row = |r: usize| -> Result<Vf, String> {
                let b = at(i) + MATRIX + r * 16;
                Ok(Vf::from_bits([word(b)?, word(b + 4)?, word(b + 8)?, word(b + 12)?]))
            };
            let local = Mat4([row(0)?, row(1)?, row(2)?, row(3)?]);
            // A node comes after its parent in the file.
            let world = match usize::try_from(parent) {
                Ok(p) if p < i => mul(&local, &worlds[p]),
                Ok(_) => return Err(format!("node {i}'s parent {parent} comes after it")),
                Err(_) => local,
            };
            parents.push(parent);
            worlds.push(world);
        }
        if count == 0 || parents[0] != -1 {
            return Err("a .psg whose first node is not its root".into());
        }
        let three = |at: usize| -> Result<Vf, String> { Ok(Vf::from_bits([word(at)?, word(at + 4)?, word(at + 8)?, 1f32.to_bits()])) };
        fn tree(i: usize, parents: &[i32], worlds: &[Mat4], own: &dyn Fn(usize) -> Result<NodeBox, String>) -> Result<BoundsNode, String> {
            let mut children = Vec::new();
            for (k, &p) in parents.iter().enumerate() {
                if p == i as i32 && k > i {
                    children.push(tree(k, parents, worlds, own)?);
                }
            }
            Ok(BoundsNode { own: Some(own(i)?), world: worlds[i], children })
        }
        let own = |i: usize| -> Result<NodeBox, String> { Ok(NodeBox { centre: three(at(i) + CENTRE)?, half_extent: three(at(i) + HALF)? }) };
        let bounds = crate::prop_world::node_bounds(&tree(0, &parents, &worlds, &own)?);
        let mut tire_nodes = [Vf::default(); 4];
        for (k, want) in TIRE_NODES.iter().enumerate() {
            let i = names.iter().position(|n| n == want).ok_or_else(|| format!("the model has no {want} node"))?;
            let t = worlds[i].0[3];
            tire_nodes[k] = Vf::new(t.x(), t.y(), t.z(), 1.0);
        }
        Ok(CarModel { tire_nodes, bounds })
    }
}

impl Machine {
    /// **A fresh car, as its constructor and loader leave it** (S149e), parked at `at` with its nose
    /// along `facing`, squared to `up` ([`crate::flight::spawn_turn`]): the body out of
    /// `[Mass Properties]` (the AI's `CofGAI` until someone drives it), its inertia off the model's
    /// whole bounds ([`Part::inertia`] of their full extents), each tire hung at its model node moved
    /// out by the tire's `Width`, the undriven tires spun to the constructor's top wheel speed and the
    /// driven ones to the engine's, the launch counting down from the start and the preload unarmed.
    /// The body stands at `at` itself, not `at` moved by the centre of mass (S484: the original's
    /// place puts recording2's truck's body at its spawn to the bit).
    ///
    /// The turn is the load's put-back's to the bit (S498: free07's baja). The original's put-back
    /// squares its stored nose and up in place, so each later put-back squares them again and the
    /// up's last bits drift; a second put-back is this called with [`crate::flight::spawn_axes`] of
    /// the first's pair.
    ///
    /// The original takes each node's place with the model already placed at the spot and turns it
    /// back into the root's frame; this takes it with the root unplaced, so a mount differs from the
    /// original's by that round trip's rounding (under 0.002 ft on the three parked cars, the most on the buggy, whose spot leans).
    pub fn new(car: &Car, model: &CarModel, at: Vf, facing: Vf, up: Vf) -> Machine {
        let part = &car.mass_properties;
        let quat = crate::flight::spawn_turn(facing, up);
        let h = model.bounds.half_extent;
        let two = F(2.0);
        let dims = Vf::new((F(h.x()) * two).0, (F(h.y()) * two).0, (F(h.z()) * two).0, 1.0);
        let inertia = part.inertia(dims);
        let recip = |v: f32| (F::ONE / F(v)).0;
        let cofg = car.cofg_ai;
        let position = Vf::new(at.x(), at.y(), at.z(), 1.0);
        let body = crate::biplane::Body {
            enabled: true,
            inv_mass: F::ONE / part.mass,
            damping: part.velocity_damping,
            last_position: position,
            position,
            inv_inertia: Vf::new(recip(inertia.x()), recip(inertia.y()), recip(inertia.z()), 1.0),
            ang_damping: part.angular_rate_damping,
            last_quat: quat,
            quat,
            cofg,
            ..Default::default()
        };
        let wheels = [0, 1, 2, 3].map(|k| {
            let node = model.tire_nodes[k];
            Wheel {
                s: TireState { friction_scale: F::ONE, ..Default::default() },
                mount: Vf::new((F(node.x()) + car.tires[k].width).0, node.y(), node.z(), 1.0),
                top_wheel_speed: car.top_wheel_speed(k).unwrap_or(UNDRIVEN_TOP_WHEEL_SPEED),
                probe_normal: Vf::new(0.0, 1.0, 0.0, 0.0),
                car_normal: Vf::new(0.0, 1.0, 0.0, 0.0),
                drop: Vf::default(),
                lever: Vf::default(),
                upper: Vf::default(),
                lower: Vf::default(),
                probes: 0,
                solid: false,
                ground_point: Vf::default(),
                ground_normal: Vf::new(0.0, 1.0, 0.0, 0.0),
                point_velocity: Vf::default(),
                body_velocity: Vf::default(),
                local_velocity: Vf::default(),
                frame: crate::math::Mat4::IDENTITY,
                lone_front: false,
                sorted: false,
                terrain_held: false,
            }
        });
        let launch_time = car.clutch_torque_scale_time.unwrap_or(F::ZERO);
        let max_scale = car.max_clutch_torque_scale.unwrap_or(F::ONE);
        let p = car.preload;
        Machine {
            body,
            gravity: part.gravity,
            controls: Controls::new(car),
            drivetrain: Drivetrain::new(&car.engine),
            wheels,
            last_cofg: cofg,
            human: false,
            steering_wheel: false,
            wrecked: false,
            wreck_lifted: false,
            touching: false,
            airborne_touch: false,
            stuck_time: F::ZERO,
            launch: Launch {
                timer: launch_time,
                start: max_scale,
                time: launch_time,
                max_scale,
                max_scale_speed: car.max_clutch_torque_scale_speed.unwrap_or(F::ZERO),
            },
            preload: PreloadState {
                time: p.preload_time.unwrap_or(F::ZERO),
                min_time: p.min_preload_time.unwrap_or(F::ZERO),
                max_time: p.max_preload_time.unwrap_or(F::ZERO),
                max_preload: p.max_preload.unwrap_or(F::ZERO),
                force: Vf::new(0.0, 0.0, 0.0, 1.0),
                ..Default::default()
            },
            latch: false,
            latched: false,
            grounded: false,
            was_grounded: false,
            ground_speed: F::ZERO,
            velocity: Vf::default(),
            ground_velocity: Vf::default(),
            world_rates: Vf::default(),
            alpha: F::ZERO,
            side_slip: F::ZERO,
            euler: Vf::default(),
            matrix: crate::math::quat_to_mat4(quat, position),
            forward: Vf::default(),
            up: Vf::default(),
            event: None,
            edge_scale: F(EDGE_SCALE),
        }
    }

    /// Someone takes the wheel (or lets it go): the controls drive, and the centre of mass moves to
    /// the class's `CofG` on the next frame's travel.
    pub fn drive(&mut self, on: bool) {
        self.human = on;
        self.controls.driven = on;
    }
}

// --- the parked car's settle and sleep (S484) -----------------------------------------------------

/// **Where a parked car is spawned** (S484): its map spot's x and z, and its height the ground's
/// under the spot plus the model's bounds centre — so the body starts with its centre where the
/// model's middle would stand on the ground, and drops onto its tires from there. free09's truck
/// (`machine.txt`'s 4594, 135, 3838) gives the original's spawn bit for bit (141.38817).
pub fn parked_spawn(world: &mut impl crate::world::World, model: &CarModel, at: Vf) -> Vf {
    let (ground, _) = world.ground(Vf::new(at.x(), at.y(), at.z(), 1.0));
    Vf::new(at.x(), (F(ground.y()) + F(model.bounds.centre.y())).0, at.z(), 1.0)
}

/// **The up a parked car is spawned with** (B47): the ground's normal under its map spot, which the
/// original keeps as the car's spawn up (its `+0x290`) and squares its nose to. free07's baja stands
/// on a 30-degree face: turned upright instead, it leans onto the slope differently and rolls off
/// 30 frames before the original's.
pub fn parked_up(world: &mut impl crate::world::World, at: Vf) -> Vf {
    let (_, normal) = world.ground(Vf::new(at.x(), at.y(), at.z(), 1.0));
    Vf::new(normal.x(), normal.y(), normal.z(), 0.0)
}

/// `CofGAI` turned to a car spawned along `facing` and squared to `up`: what [`Machine::new`] lifted
/// the body by before S484 put it at the spot itself. The shell's taken car keeps the lift, so it
/// is still dropped from where its tires were tuned.
pub fn cofg_ai_lift(car: &Car, facing: Vf, up: Vf) -> Vf {
    let lift = crate::chassis::turn(crate::flight::spawn_turn(facing, up), car.cofg_ai);
    Vf::new(lift.x(), lift.y(), lift.z(), 0.0)
}

impl Machine {
    /// **Whether the car may sleep** (S484): the generic physics object's at-rest test, the props'
    /// ([`crate::prop_world::Collection::at_rest`]), with the six thresholds of the car's
    /// `[Mass Properties]` — the body's speed, angular rate, acceleration, angular acceleration,
    /// shock and twist each under its own. The original's test also refuses a body its enable has
    /// just woken until the first force clears that; a car is pushed by its own forces every frame,
    /// so after a step that never holds and is not kept here.
    pub fn at_rest(&self, car: &Car) -> bool {
        let p = &car.mass_properties;
        let thresholds = [
            p.speed_threshold,
            p.angular_rate_threshold,
            p.acceleration_threshold,
            p.angular_acceleration_threshold,
            p.speed_impulse_threshold,
            p.body_angular_speed_impulse_threshold,
        ];
        let size = |v: Vf| Acc::mula(F(v.x()), F(v.x())).madda(F(v.y()), F(v.y())).madd(F(v.z()), F(v.z())).sqrt();
        let b = &self.body;
        [b.velocity, b.rates, b.accel, b.ang_accel, b.shock, b.twist].iter().zip(thresholds).all(|(&v, t)| size(v) < t)
    }

    /// **The parked car's settle** (S484): stepped with nobody in it, `dt` a frame, until its body
    /// is at rest ([`Machine::at_rest`]), when the object listener's sleep disables the body; the
    /// frame it slept on (0 the first), or `None` if it was still moving after `frames`. The truck
    /// has no `ResetTime` (-1), so it is never put back and sleeps for good; a parked car's body
    /// then stands as its hull ([`crate::ai::parked_hull`], [`crate::Sim::park`]).
    ///
    /// Proved on recording2's truck put back at its spawn and stepped by the original's own node
    /// update at 0.03333 s a frame: the body bit for bit each frame and asleep on the same frame
    /// (56), with the tire mounts and inertia the original's (see [`Machine::new`]).
    pub fn settle(
        &mut self,
        car: &Car,
        world: &mut impl crate::world::World,
        collision: &mut Collision,
        dt: F,
        acos: &AcosTable,
        curve: &CurveTable,
        frames: usize,
    ) -> Option<usize> {
        for frame in 0..frames {
            self.step_struck(car, world, collision, dt, acos, curve, &mut |_| F::ZERO);
            if self.at_rest(car) {
                self.body.enabled = false;
                return Some(frame);
            }
        }
        None
    }
}

// --- a parked car struck by the riders' bikes (S485) -----------------------------------------------

/// **A parked car in the riders' world** (S485), as [`crate::Sim::parked_cars`] holds it: its `.phy` and
/// hull (constants of the disc, shared by every clone), the car settled where it was parked
/// ([`Machine::settle`]) and its collision object. Asleep, it is not stepped, but its hull is in the
/// riders' list, as a driven car's is (F26): a bike's body rays strike it, the strike wakes its body
/// (the struck listener's enable), and the bike's resolve pushes it. Awake, it is stepped with nobody
/// in it, ahead of the bikes, until it is at rest again ([`Machine::at_rest`]) and sleeps.
///
/// S484's live capture (`context/capture/s484/ride1.csv`): a bike ridden into free09's truck at
/// ~55 ft/s wakes it on the strike's frame with the push already in its velocity and its place not
/// yet moved, so the car's node is stepped before the riders' in the original's frame; and a bike
/// that leans on it after wakes it for a frame without a step, so the wake is the bike's, after the
/// car's step. The enable also sets the car's driven word, which the car's first step clears again
/// when nobody is in it, before its controls read it: a parked car's controls stay undriven here.
/// (S489a: R48's oracle clears it only after the controls step, so a struck car's brakes come off on
/// the frames it is struck; [`InPlane::struck`] has that for the plane's world. S491: not here.
/// Driven on the step after a bike's strike, the port's truck keeps vz 2.53 on strike+1 where
/// ride1.csv's brakes to 1.75, and drives on at ~1.7 ft/s while the wreck bike leans on it; undriven
/// it is within 0.01. So the original's truck is braked then: something clears the word between
/// the riders' strike and the car's next controls step, or the bike's wake does not set it.)
#[derive(Clone, Debug)]
pub struct Parked {
    pub car: std::sync::Arc<Car>,
    pub hull: std::sync::Arc<crate::track::Mesh>,
    pub machine: Machine,
    pub collision: Collision,
    /// Where its body last fell asleep (S486): the node the AI riders' hull of it stands at
    /// ([`Parked::ai_hull`]), which moves only when it sleeps again, and which a resync carries.
    pub rest: crate::math::Mat4,
    /// Where each put-back puts it (S500, [`Parked::put_back`]).
    pub store: Store,
    /// Its model, which each put-back builds it off again.
    pub model: std::sync::Arc<CarModel>,
}

/// **A parked car's put-back store** (S500): the spot, nose and up its physics object keeps and puts
/// it back at on every world reset (the load's and each Restart's). Each put-back squares the nose
/// and up in place ([`crate::flight::spawn_axes`]) and turns the car by them, so after the load's
/// they hold the pair squared once, and restart N turns the car by the pair squared N+1 times.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Store {
    pub at: Vf,
    pub nose: Vf,
    pub up: Vf,
}

/// How far the safe-spot recorder's probe stands off the root node's place (S501): five feet under.
const SAFE_SPOT_LIFT: f32 = -5.0;
/// A safe spot is kept only where the ground normal's y is over this (ground flatter than ~37°).
const SAFE_SPOT_LEVEL: f32 = 0.8;
/// How far along the ground's normal a safe spot stands off the ground.
const SAFE_SPOT_RAISE: f32 = 0.5;

impl Store {
    /// **The safe-spot recorder** (S501, R68): what a car the human has switched into (it holds a
    /// score card and no activity) does each frame, from both its post and the score manager. A
    /// probe five feet under its root node's `place` drops onto the ground; with nothing solid over
    /// it ([`crate::ride::open_sky`]) and the ring around the spot clear ([`crate::ride::clearance`],
    /// the bike's put-back test, which the car's runs probe for probe), on ground whose normal `n`
    /// has `y` over 0.8, the store becomes: the spot half a foot up `n`, the root's `nose` flattened,
    /// made unit and squared against `n`, and `n` itself. `true` when it stored.
    ///
    /// `cache` is the car's own put-back query state; it gathers the way free ride's card asks (the
    /// props and plants, kind 3) and never counts checkpoints (no activity). The spot is also kept
    /// only where the world keeps it ([`crate::world::World::keeps`]), as the bike's record is: the
    /// port's own test, which refuses only a spot with no ground under it.
    ///
    /// Proved in the interpreter (tests/car_spawn.rs) on the original's recorder in its whole frame
    /// off s342's driven free07 baja, from the same place and nose.
    pub fn record(
        &mut self,
        cache: &mut Option<Box<crate::back_search::Cache>>,
        place: Vf,
        nose: Vf,
        world: &mut dyn crate::world::World,
        curve: &CurveTable,
    ) -> bool {
        let at = Vf::new(place.x(), (F(place.y()) + F(SAFE_SPOT_LIFT)).0, place.z(), place.w());
        let Some(ground) = crate::ride::open_sky(at, world, cache) else {
            return false;
        };
        let spot = Vf::new(at.x(), ground.0, at.z(), at.w());
        if !world.keeps(spot) {
            return false;
        }
        let (point, n, clear) = crate::ride::clearance(spot, nose, world, curve, cache);
        if !clear || !(F(n.y()) > F(SAFE_SPOT_LEVEL)) {
            return false;
        }
        let raise = Vf::new(SAFE_SPOT_RAISE, SAFE_SPOT_RAISE, SAFE_SPOT_RAISE, SAFE_SPOT_RAISE);
        let at = point.merge(Dest::XYZ, point.add(n.mul(raise)));
        let flat = crate::ride::flat_unit(nose);
        let side = flat.merge(Dest::XYZ, Vf::opmsub(Vf::opmula(flat, n), n, flat));
        let nose = n.merge(Dest::XYZ, Vf::opmsub(Vf::opmula(n, side), side, n));
        *self = Store { at, nose, up: n };
        true
    }
}

/// The node `body` stands at, as a matrix ([`crate::wreck_bike::node_of`]).
fn node_matrix(body: &crate::biplane::Body) -> crate::math::Mat4 {
    let n = crate::wreck_bike::node_of(body);
    crate::math::Mat4([n.rows[0], n.rows[1], n.rows[2], n.origin])
}

/// How many frames a parked car's load settle may take before it is left as it stands, still awake,
/// for play to step on: the truck sleeps on its 57th ([`Machine::settle`]). B47: free07's baja, on a
/// slope, never sleeps, and R52's two images of it put the original's load at 88 and 98 frames
/// before play; 400 left the port's 21 ft further down the slope than the original's at the start.
pub const SETTLE_FRAMES: usize = 90;

/// The frame time free09's load steps a parked car at (S484).
pub const LOAD_DT: f32 = 0.03333;

impl Parked {
    /// A car parked at its map spot `at` (`machine.txt`'s), its nose along `facing`: spawned over the
    /// ground ([`parked_spawn`]) and squared to it ([`parked_up`]), built ([`Machine::new`]) and settled at the load's frame time
    /// ([`LOAD_DT`]) until it sleeps, as the original's load leaves it.
    #[allow(clippy::too_many_arguments)]
    pub fn settled(
        car: std::sync::Arc<Car>,
        model: &CarModel,
        rays: crate::flight_search::Rays,
        hull: std::sync::Arc<crate::track::Mesh>,
        world: &mut dyn crate::world::World,
        at: Vf,
        facing: Vf,
        acos: &AcosTable,
        curve: &CurveTable,
    ) -> Parked {
        Parked::settled_after(car, model, rays, hull, world, at, facing, acos, curve, 0)
    }

    /// **The same after `restarts` Restarts** (S500): put back that many times more
    /// ([`Parked::put_back`]) before the settle, so it is turned by its store squared `restarts` + 1
    /// times, as the original's car is after that many world resets. The settle after the last is
    /// the load's: the shell's Restart loads the ride again.
    #[allow(clippy::too_many_arguments)]
    pub fn settled_after(
        car: std::sync::Arc<Car>,
        model: &CarModel,
        rays: crate::flight_search::Rays,
        hull: std::sync::Arc<crate::track::Mesh>,
        world: &mut dyn crate::world::World,
        at: Vf,
        facing: Vf,
        acos: &AcosTable,
        curve: &CurveTable,
        restarts: usize,
    ) -> Parked {
        Parked::settled_from(car, model, rays, hull, world, at, facing, acos, curve, restarts, None)
    }

    /// **The same after a Restart from the store the ride ended with** (S501): `carried`, the store
    /// as the last ride left it (the load's squared pair, each Restart's squared again, or a safe
    /// spot a driven car recorded, [`Store::record`]), taken in place of the load's and put back
    /// from once, as the original's world reset puts the car back from whatever its store holds.
    /// `None` is [`Parked::settled_after`]'s `restarts` put-backs from the load's.
    #[allow(clippy::too_many_arguments)]
    pub fn settled_from(
        car: std::sync::Arc<Car>,
        model: &CarModel,
        rays: crate::flight_search::Rays,
        hull: std::sync::Arc<crate::track::Mesh>,
        world: &mut dyn crate::world::World,
        at: Vf,
        facing: Vf,
        acos: &AcosTable,
        curve: &CurveTable,
        restarts: usize,
        carried: Option<Store>,
    ) -> Parked {
        let world = &mut Through(world);
        let spawn = parked_spawn(world, model, at);
        let up = parked_up(world, at);
        let machine = Machine::new(&car, model, spawn, facing, up);
        let collision = collision_of_model(&car, model, &machine.body, rays);
        // The load's put-back squared the store once.
        let (nose, up) = crate::flight::spawn_axes(facing, up);
        let rest = node_matrix(&machine.body);
        let mut p = Parked { car, hull, machine, collision, rest, store: Store { at: spawn, nose, up }, model: std::sync::Arc::new(model.clone()) };
        let puts = match carried {
            Some(store) => {
                p.store = store;
                1
            }
            None => restarts,
        };
        for _ in 0..puts {
            p.put_back();
        }
        let Parked { car, machine, collision, .. } = &mut p;
        machine.settle(car, world, collision, F(LOAD_DT), acos, curve, SETTLE_FRAMES);
        p.rest = node_matrix(&p.machine.body);
        p
    }

    /// **The put-back of a world reset** (S500): the car built again at its store's spot, turned by
    /// its store's nose and up squared ([`Machine::new`]), its collision object with it, and the
    /// store's pair squared in place, as the original's car reset leaves it (R67: the reset of every
    /// actor, which the pause menu's Restart runs). Its body is awake and where it last slept is
    /// where it now stands.
    ///
    /// Proved in the interpreter on R52's free07 baja image (tests/car_spawn.rs): from the image's
    /// store, the body's place, turn and the store bit for bit after one and two of the original's
    /// world resets. The port's own spot is the ground's last bits off the original's (5e-5 ft).
    pub fn put_back(&mut self) {
        let Store { at, nose, up } = self.store;
        self.machine = Machine::new(&self.car, &self.model, at, nose, up);
        self.collision = collision_of_model(&self.car, &self.model, &self.machine.body, (*self.collision.rays).clone());
        let (nose, up) = crate::flight::spawn_axes(nose, up);
        self.store = Store { at, nose, up };
        self.rest = node_matrix(&self.machine.body);
    }

    /// Whether its body is awake.
    pub fn awake(&self) -> bool {
        self.machine.body.enabled
    }

    /// **One frame of a woken parked car** (S485): [`Machine::step_struck`] with nobody in it. Asleep,
    /// nothing. True when it stepped and is at rest ([`Machine::at_rest`]): the object listener puts
    /// it to sleep after the riders' frames ([`Parked::sleep`]), so a wreck bike that strikes it in
    /// the same frame does not keep it awake (S486).
    pub fn frame(&mut self, world: &mut dyn crate::world::World, dt: F, acos: &AcosTable, curve: &CurveTable) -> bool {
        if !self.awake() {
            return false;
        }
        let Parked { car, machine, collision, .. } = self;
        machine.step_struck(car, &mut Through(world), collision, dt, acos, curve, &mut |_| F::ZERO);
        machine.at_rest(car)
    }

    /// The object listener's sleep (S486), after the frame in which [`Parked::frame`] found it at
    /// rest: its body disabled whatever struck it since, and its hull's place for the AI riders
    /// moved to where it now rests.
    ///
    /// S484's capture: after the strike the truck sleeps (row 67), the wrecked rider's bike sliding
    /// back along its nose wakes it without a step (68), it steps 0.001 ft and sleeps again (69),
    /// is woken (70), steps and sleeps (71) and stays asleep once the bike has slid clear. A strike
    /// on a car stepped that frame is undone by its sleep; one on a car asleep at the frame's start
    /// leaves it awake for the next.
    pub fn sleep(&mut self) {
        self.machine.body.enabled = false;
        self.rest = node_matrix(&self.machine.body);
    }

    /// Its hull as the riders' bikes meet it, awake or asleep ([`Machine::hull`]), for seat `seat`.
    pub fn hull(&self, seat: usize) -> crate::rider_contact::Hull<'_> {
        self.machine.hull(seat, &self.hull, &self.collision)
    }

    /// Its hull as the AI riders' avoidance meets it ([`crate::ai::parked_hull`]), where it last fell
    /// asleep ([`Parked::rest`]).
    pub fn ai_hull(&self) -> crate::world::Prop {
        crate::world::Prop::on((*self.hull).clone(), &self.rest)
    }

    /// The struck listener (S485): a bike's body rays struck its hull, and its body is enabled.
    pub fn struck(&mut self) {
        self.machine.body.enabled = true;
    }

    /// Its body as a bike's resolve pushes it ([`Machine::flown`]).
    pub fn flown(&self, seat: usize, from: Vf, world: &dyn crate::world::World) -> crate::rider_contact::Flown {
        self.machine.flown(seat, &self.collision, from, world)
    }

    /// The body a bike's resolve pushed, written back ([`Machine::pushed`]).
    pub fn pushed(&mut self, by: &crate::rider_contact::Flown) {
        self.machine.pushed(&mut self.collision, by);
    }
}

/// **A woken parked car in a flown plane's dynamics world** (S489): R48's oracle (the original's
/// node update run on its state) has the plane and the truck in one node, each substep the plane's
/// resolve and then the truck's, as the downed wreck bikes are ([`crate::wreck_plane::InWorld`]). Its
/// search runs after the plane's and before the world step: the mirrors the plane's rays put on its
/// hull at the head of its list, its own entries on the plane's hull after them, and their mirrors on
/// the plane's list ([`crate::flight::Met::cars`]). Its body is the one the plane's resolve pushes
/// (`place` among the frame's pushed bodies, as [`crate::flight::Met::wrecks`]' copy).
#[derive(Clone, Debug)]
pub struct InPlane {
    pub parked: Parked,
    /// Its place in [`crate::flight::Met::wrecks`] (within the frame, among its pushed bodies).
    pub place: usize,
    pub movers: Vec<crate::contact::Mover>,
    /// Where the plane's copy of its body stands off its own (a world that wraps), and whether it
    /// was searched and stepped in the plane's world this frame.
    pub by: Vf,
    pub stepped: bool,
}

impl InPlane {
    /// A parked car for the plane's world, its body `place` in the met's wrecks.
    pub fn new(parked: Parked, place: usize) -> InPlane {
        InPlane { parked, place, movers: Vec::new(), by: Vf::ZERO, stepped: false }
    }

    /// Its search, awake, after the plane's (`hull` the plane's, where it stood for its search): the
    /// plane's `mirrors` on its hull at the head of its list (naming the plane, side 0), then its own
    /// entries. Gives the mirrors of its own entries on the plane's hull, for the plane's list.
    pub fn search(&mut self, world: &mut dyn crate::world::World, dt: F, mirrors: &[crate::contact::Contact], hull: Option<crate::flight_search::Knock<'_>>) -> Vec<crate::contact::Contact> {
        let Parked { machine, collision, .. } = &mut self.parked;
        let node = crate::wreck_bike::node_of(&machine.body);
        let knocks: Vec<(usize, crate::flight_search::Knock<'_>, F)> = hull.into_iter().map(|h| (0, h, crate::flight_search::knock_distance(&collision.rays, &node, &h))).collect();
        let (movers, theirs) = collision.search_knocking(&mut machine.body, world, dt, &knocks);
        self.movers = movers;
        if !mirrors.is_empty() {
            let r = &mut collision.record;
            (r.touching, r.on_object, r.touching_other) = (true, true, true);
            let mut list: Vec<crate::contact::Contact> = mirrors.iter().map(|e| crate::contact::Contact { other: Some(0), ..*e }).collect();
            list.append(&mut r.contacts);
            list.truncate(r.capacity);
            r.contacts = list;
        }
        machine.touching = collision.record.touching;
        self.stepped = true;
        theirs.into_iter().flat_map(|(_, m)| m).collect()
    }

    /// Its body as the plane's world holds it (`body`) taken into the car, and given back after `run`.
    fn through(&mut self, body: &mut crate::prop_contact::Pushed, run: impl FnOnce(&mut InPlane)) {
        let by = self.by;
        let b = &mut self.parked.machine.body;
        *b = body.body;
        (b.position, b.last_position) = (b.position.sub(by), b.last_position.sub(by));
        self.parked.collision.collider = body.collider;
        run(self);
        let b = self.parked.machine.body;
        body.body = b;
        (body.body.position, body.body.last_position) = (b.position.add(self.by), b.last_position.add(self.by));
        body.collider = self.parked.collision.collider;
    }

    /// The car's step's listeners ([`Stepping`]), `plane` its resolve's other side.
    #[allow(clippy::too_many_arguments)]
    fn listen(
        &mut self,
        body: &mut crate::prop_contact::Pushed,
        plane: &mut crate::prop_contact::Pushed,
        world: &mut dyn crate::world::World,
        acos: &AcosTable,
        curve: &CurveTable,
        call: impl FnOnce(&mut dyn crate::biplane::Dynamics),
    ) {
        self.through(body, |me| {
            let InPlane { parked: Parked { car, machine, collision, .. }, movers, .. } = me;
            let mut s = Stepping {
                m: machine,
                car,
                world: &mut Through(world),
                acos,
                curve,
                noise: &mut |_| F::ZERO,
                struck: Some((collision, movers)),
                met: None,
                props: std::slice::from_mut(plane),
                knocks: &[],
            };
            call(&mut s);
        });
    }

    /// The plane's rays struck it this frame (S489a): its body woken, and the enable's driven word
    /// set. R48's oracle clears that word only after the car's controls step has read it, so on a
    /// frame it is struck its controls drive with nobody's pedals: the brakes off, the engine's
    /// torque reaching the rear tires. On a frame it is not struck they brake again.
    pub fn struck(&mut self) {
        self.parked.struck();
        self.parked.machine.controls.driven = true;
    }

    /// Its preforce listener, with the frame's `dt`: the controls step, then the driven word a strike
    /// set cleared again with nobody in it.
    pub fn preforce(&mut self, body: &mut crate::prop_contact::Pushed, plane: &mut crate::prop_contact::Pushed, world: &mut dyn crate::world::World, dt: F, acos: &AcosTable, curve: &CurveTable) {
        self.listen(body, plane, world, acos, curve, |s| s.preforce(0, dt));
        let machine = &mut self.parked.machine;
        machine.controls.driven = machine.human;
    }

    /// Its force listener, after the plane's: tires and its list resolved against the plane's body.
    pub fn force(&mut self, body: &mut crate::prop_contact::Pushed, plane: &mut crate::prop_contact::Pushed, world: &mut dyn crate::world::World, h: F, acos: &AcosTable, curve: &CurveTable) {
        self.listen(body, plane, world, acos, curve, |s| s.force(0, h));
    }

    /// Its post listener: the measure, and the next frame's terrain switch.
    pub fn post(&mut self, body: &mut crate::prop_contact::Pushed, plane: &mut crate::prop_contact::Pushed, world: &mut dyn crate::world::World, dt: F, acos: &AcosTable, curve: &CurveTable) {
        self.listen(body, plane, world, acos, curve, |s| s.post(0, dt));
        let Parked { machine, collision, .. } = &mut self.parked;
        collision.record.search_ground = machine.searches_terrain(&mut Through(world));
    }
}

/// **The parked cars into a driven car's world** (S499, [`Machine::step_cars`]): `parked` moved into
/// `met`'s cars, each its own place. Gives which were awake before, for [`cars_from_car`].
pub fn cars_in_car(parked: &mut Vec<Parked>, met: &mut crate::flight::Met) -> Vec<bool> {
    let awake = parked.iter().map(Parked::awake).collect();
    met.cars = std::mem::take(parked).into_iter().enumerate().map(|(k, p)| InPlane::new(p, k)).collect();
    awake
}

/// **The parked cars back out of a driven car's world** (S499): `met`'s cars into `parked`. Gives
/// which stepped there awake since before the frame and came to rest, which sleep after the bikes,
/// as a plane's lane has them ([`crate::Sim::parked_cars_rest`]): a car woken this frame sleeps at
/// the earliest after the next (S487).
pub fn cars_from_car(parked: &mut Vec<Parked>, met: &mut crate::flight::Met, was_awake: &[bool]) -> Vec<bool> {
    let cars = std::mem::take(&mut met.cars);
    let resting = cars.iter().zip(was_awake).map(|(c, a)| c.stepped && *a && c.parked.machine.at_rest(&c.parked.car)).collect();
    *parked = cars.into_iter().map(|c| c.parked).collect();
    resting
}

/// A driven car's entries on parked car `j`'s hull name `HULL_FROM + j` between its search and
/// [`Machine::cars_met`] (S499), clear of [`crate::flight::WRECK_FROM`]'s wreck-bike mirrors.
const HULL_FROM: usize = 1 << 22;

/// **A parked car's hull as a driven car's search meets it** (S499): where its collision object's
/// node stands it, on a world that wraps the copy nearest `from` (the driven car), moving with its
/// body, as the plane's search has it ([`crate::parked_wreck::cars_in_plane`]).
fn parked_knock<'a>(p: &'a Parked, from: Vf, world: &dyn crate::world::World) -> crate::flight_search::Knock<'a> {
    let h = p.hull(0);
    let mut at = h.at;
    at.0[3] = crate::nearest_copy(world, from, at.0[3]);
    let (to_world, centre, radius) = crate::world::Prop::posed(h.mesh, &at);
    crate::flight_search::Knock { mesh: h.mesh, to_world, centre, radius, velocity: p.machine.body.velocity, capacity: h.capacity }
}

/// **A driven car's dynamics world with the parked cars awake in it** (S499): body 0 the car
/// ([`Stepping`]), then each of `live` (the cars that searched this frame, [`InPlane::stepped`]),
/// its body `props[at + k]`, its listeners after the car's with the car's body on its resolve, as
/// a flown plane's world has them (S489, R48's node: the plane's resolve, then the truck's).
struct AmongCars<'a, W, N> {
    m: &'a mut Machine,
    car: &'a Car,
    world: &'a mut W,
    acos: &'a AcosTable,
    curve: &'a CurveTable,
    noise: &'a mut N,
    collision: &'a mut Collision,
    movers: &'a [crate::contact::Mover],
    met: Option<&'a mut crate::flight::Met>,
    props: &'a mut [crate::prop_contact::Pushed],
    knocks: &'a [crate::flight_search::Knock<'a>],
    cars: &'a mut [InPlane],
    live: Vec<usize>,
    at: usize,
}

impl<W: crate::world::World, N: FnMut(F) -> F> AmongCars<'_, W, N> {
    /// The car's own listener `call`.
    fn own(&mut self, call: impl FnOnce(&mut dyn crate::biplane::Dynamics)) {
        let mut s = Stepping {
            m: &mut *self.m,
            car: self.car,
            world: &mut *self.world,
            acos: self.acos,
            curve: self.curve,
            noise: &mut *self.noise,
            struck: Some((&mut *self.collision, self.movers)),
            met: self.met.as_deref_mut(),
            props: &mut *self.props,
            knocks: self.knocks,
        };
        call(&mut s);
    }

    /// The parked car body `i` of the lists is, if it is one: its index in `cars`.
    fn parked(&self, i: i32) -> Option<usize> {
        self.live.get((i as usize).checked_sub(1)?).copied()
    }

    /// Parked car `k`'s listener `call`, its body and the driven car's (the copy nearest it) handed
    /// over, and the driven car's written back as its resolve left it.
    #[allow(clippy::type_complexity)]
    fn listens(&mut self, k: usize, call: impl FnOnce(&mut InPlane, &mut crate::prop_contact::Pushed, &mut crate::prop_contact::Pushed, &mut dyn crate::world::World, &AcosTable, &CurveTable)) {
        let c = &mut self.cars[k];
        let f = self.m.flown(0, self.collision, c.parked.machine.body.position, &*self.world);
        let mut by = crate::prop_contact::Pushed { collection: 0, object: 0, collider: f.collider, body: f.body, mass: f.mass };
        call(c, &mut self.props[self.at + c.place], &mut by, &mut *self.world, self.acos, self.curve);
        self.m.pushed(self.collision, &crate::rider_contact::Flown { collider: by.collider, body: by.body, mass: by.mass, ..f });
    }

    fn body(&mut self, k: usize) -> &mut crate::biplane::Body {
        &mut self.props[self.at + self.cars[k].place].body
    }
}

impl<W: crate::world::World, N: FnMut(F) -> F> crate::biplane::Dynamics for AmongCars<'_, W, N> {
    fn count(&self, list: crate::biplane::List) -> i32 {
        match list {
            crate::biplane::List::Constraints => 0,
            _ => 1 + self.live.len() as i32,
        }
    }
    fn preforce(&mut self, i: i32, dt: F) {
        match self.parked(i) {
            Some(k) => self.listens(k, |c, body, by, world, acos, curve| c.preforce(body, by, world, dt, acos, curve)),
            None => self.own(|s| s.preforce(0, dt)),
        }
    }
    fn force(&mut self, i: i32, h: F) {
        match self.parked(i) {
            Some(k) => self.listens(k, |c, body, by, world, acos, curve| c.force(body, by, world, h, acos, curve)),
            None => self.own(|s| s.force(0, h)),
        }
    }
    fn enabled(&self, i: i32) -> bool {
        match self.parked(i) {
            Some(k) => self.props[self.at + self.cars[k].place].body.enabled,
            None => self.m.body.enabled,
        }
    }
    fn integrate(&mut self, i: i32, h: F) {
        match self.parked(i) {
            Some(k) => self.body(k).integrate(h),
            None => self.m.body.integrate(h),
        }
    }
    fn end_substep(&mut self, i: i32) {
        match self.parked(i) {
            Some(k) => self.body(k).end_substep(),
            None => self.m.body.end_substep(),
        }
    }
    fn constrain(&mut self, _: i32, _: bool) {}
    fn post(&mut self, i: i32, dt: F) {
        match self.parked(i) {
            Some(k) => self.listens(k, |c, body, by, world, acos, curve| c.post(body, by, world, dt, acos, curve)),
            None => self.own(|s| s.post(0, dt)),
        }
    }
}

/// A `dyn World` for the car's step, which takes its world by type: every query passed through.
struct Through<'a>(&'a mut dyn crate::world::World);

impl crate::world::World for Through<'_> {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.0.ground(at)
    }
    fn surface(&mut self, at: Vf) -> u8 {
        self.0.surface(at)
    }
    fn wheel_surface(&mut self, at: Vf) -> Option<u8> {
        self.0.wheel_surface(at)
    }
    fn mxvatv_terrain(&self) -> Option<crate::atv_ground::Terrain<'_>> {
        self.0.mxvatv_terrain()
    }
    fn solid(&mut self, bundle: &crate::collide::Bundle, record: &mut crate::collide::Record) -> bool {
        self.0.solid(bundle, record)
    }
    fn groups(&mut self, bundle: &crate::collide::Bundle, record: &mut crate::collide::Record) -> bool {
        self.0.groups(bundle, record)
    }
    fn crowded(&mut self, centre: Vf, radius: F) -> bool {
        self.0.crowded(centre, radius)
    }
    fn area(&self) -> Option<crate::biplane::Area> {
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
    fn heightfield(&mut self) -> Option<Box<dyn crate::contact_search::Heightfield + '_>> {
        self.0.heightfield()
    }
    #[allow(clippy::type_complexity)]
    fn collision(&mut self) -> (Option<Box<dyn crate::contact_search::Heightfield + '_>>, Option<&crate::world::Props>) {
        self.0.collision()
    }
    #[allow(clippy::type_complexity)]
    fn wooded(&mut self) -> (Option<Box<dyn crate::contact_search::Heightfield + '_>>, Option<&crate::world::Props>, Option<&crate::eco::Trees>) {
        self.0.wooded()
    }
    #[allow(clippy::type_complexity)]
    fn wooded_near(
        &mut self,
        at: Vf,
    ) -> (Option<Box<dyn crate::contact_search::Heightfield + '_>>, Option<&crate::world::Props>, Option<&crate::eco::Trees>) {
        self.0.wooded_near(at)
    }
    fn near_set(&self, at: Vf) -> Option<[i32; 2]> {
        self.0.near_set(at)
    }
    fn moving(&mut self) -> Option<&mut crate::world::Props> {
        self.0.moving()
    }
}

// --- the collision object (S229) -------------------------------------------------------------------

/// The cars' ray probe files in `machine.res`: the rigid-body part's `%sm.col` for the stock skin
/// (S229 checked the truck's 41 rays against its collision object in S149a's switched image).
pub const MONSTER_TRUCK_RAYS: &str = "MonsterTruck\\col\\monstertruckm.col";
pub const BUGGY_RAYS: &str = "Buggy\\col\\buggym.col";
pub const BAJA_RAYS: &str = "Baja\\col\\bajam.col";

impl Kind {
    /// The machine's ray probe file in `machine.res`.
    pub fn rays(self) -> &'static str {
        match self {
            Kind::MonsterTruck => MONSTER_TRUCK_RAYS,
            Kind::Buggy => BUGGY_RAYS,
            Kind::Baja => BAJA_RAYS,
        }
    }
}

/// The cars' collidable meshes in `machine.res`, beside their ray probes: the stock skin's `%sf.col`
/// (F26), the hull another object's rays strike (a rider's bike's body rays,
/// [`crate::rider_contact::gather_met`]). The alternate liveries' files are the same sizes.
pub const MONSTER_TRUCK_HULL: &str = "MonsterTruck\\col\\monstertruckf.col";
pub const BUGGY_HULL: &str = "Buggy\\col\\buggyf.col";
pub const BAJA_HULL: &str = "Baja\\col\\bajaf.col";

impl Kind {
    /// The machine's collidable mesh in `machine.res` (F26).
    pub fn hull(self) -> &'static str {
        match self {
            Kind::MonsterTruck => MONSTER_TRUCK_HULL,
            Kind::Buggy => BUGGY_HULL,
            Kind::Baja => BAJA_HULL,
        }
    }
}

/// A car's collidable's categories (the collision object's `+0x1f8`, its `+0x84`), off S149a's
/// switched truck: not a key of the file. Its actor and collides-with masks are the helicopter's.
pub const COLLIDABLE: u32 = 0x0005_2058;

/// The car's collision object: the rigid-body part's, the class the helicopter's is
/// ([`crate::kiowa::Collision`]): its response's tuning off `[Mass Properties]` and the body's inertia
/// (`inertia`, the body's tensor, not its inverse), its search's record with the ray probe out of the
/// car's `m.col` ([`Kind::rays`], one contact a ray), and its nearby list.
pub type Collision = crate::kiowa::Collision;

/// The collision object as the part's loader builds it for a car whose body is `body`.
pub fn collision(car: &Car, body: &crate::biplane::Body, inertia: Vf, rays: crate::flight_search::Rays) -> Collision {
    let mut c = Collision::new(&car.mass_properties, body, inertia, rays);
    c.collider.collidable = Some(COLLIDABLE);
    // Upright, as the loader leaves it: the measure turns the terrain on ([`Machine::searches_terrain`]).
    c.record.search_ground = false;
    c
}

/// The collision object for a car [`Machine::new`] built off `model`: its response tuned by the body's
/// inertia worked out as the constructor works it (the model's whole bounds, [`Part::inertia`]).
pub fn collision_of_model(car: &Car, model: &CarModel, body: &crate::biplane::Body, rays: crate::flight_search::Rays) -> Collision {
    let h = model.bounds.half_extent;
    let two = F(2.0);
    let dims = Vf::new((F(h.x()) * two).0, (F(h.y()) * two).0, (F(h.z()) * two).0, 1.0);
    collision(car, body, car.mass_properties.inertia(dims), rays)
}

impl Machine {
    /// **One frame of the car against the world's objects** (S229): [`Machine::step`] with its
    /// collision object. Before the world steps, the object's search (cleared, the sweep back, the
    /// rays against the terrain, the meshes and the plants: [`Collision::search`]), whose touching flag
    /// is the car's for the frame; every substep, gravity and then the object's response on the
    /// record's contacts with impulses ([`crate::contact::resolve`]: the body wrapper's force
    /// listener, ahead of the car's own forces); and in the measure the body wrapper's post, which
    /// gathers the nearby list again where the body now stands. The preforce's tire contact sort
    /// ([`Machine::sort_contacts`], S229a) searches the same nearby list.
    #[allow(clippy::too_many_arguments)]
    pub fn step_struck(
        &mut self,
        car: &Car,
        world: &mut impl crate::world::World,
        collision: &mut Collision,
        dt: F,
        acos: &AcosTable,
        curve: &CurveTable,
        noise: &mut impl FnMut(F) -> F,
    ) {
        self.step_met(car, world, collision, None, dt, acos, curve, noise);
    }

    /// **[`Machine::step_struck`] among the riders' bikes** (F26): with `met`, the mirrors of the
    /// bikes' strikes on the car's hull ([`crate::rider_contact::gather_met`]) filed on its record
    /// after its own search's entries, up to the record's room ([`Collision::file_mirrors`]), and
    /// resolved every substep against the bikes' chassis ([`crate::flight::Met`]'s colliders and
    /// vehicles, by player), which a mirror's spring and friction read and never move. Without one,
    /// [`Machine::step_struck`] exactly.
    ///
    /// F76: a `met` that names the car among the bikes' others and holds their chassis
    /// ([`crate::flight::Met::rides`], [`crate::flight::Met::bike_mesh`]) has the car's own rays strike
    /// them too ([`search_met`]), as R22's split-screen buggy does: its entries name each struck
    /// player, whose chassis its resolve pushes ([`crate::flight::Met::pushed`]), and the mirrors go
    /// to the bikes' lists ([`crate::flight::Met::struck`]).
    #[allow(clippy::too_many_arguments)]
    pub fn step_met(
        &mut self,
        car: &Car,
        world: &mut impl crate::world::World,
        collision: &mut Collision,
        mut met: Option<&mut crate::flight::Met>,
        dt: F,
        acos: &AcosTable,
        curve: &CurveTable,
        noise: &mut impl FnMut(F) -> F,
    ) {
        let movers = match met.as_deref_mut().filter(|m| strikes_bikes(m)) {
            Some(m) => search_met(collision, &mut self.body, &mut *world, dt, &[], m).0,
            None => collision.search(&mut self.body, &mut *world, dt),
        };
        if let Some(m) = met.as_deref() {
            collision.file_mirrors(&m.mirrors);
        }
        self.touching = collision.record.touching;
        let physics = crate::biplane::World { iterations: 1, rate: F(115.0) };
        physics.step(dt, &mut Stepping { m: self, car, world, acos, curve, noise, struck: Some((collision, &movers)), met, props: &mut [], knocks: &[] });
        collision.record.search_ground = self.searches_terrain(world);
    }

    /// **[`Machine::step_met`] among the knockable props** (S335): with `props`, every prop of the set
    /// with a collidable mesh is in the collision object's search after the world
    /// ([`Collision::search_props`]) and in its tires' sort; the props its rays struck are woken with
    /// their mirrors on their lists naming the car as [`CarProps::me`] ([`PropSet::strike`]), the
    /// props' own rays strike its hull where it has one ([`crate::prop_wreck::struck_on`]), and its
    /// resolve pushes the struck props' bodies, after the riders' bikes among its others, which are
    /// written back ([`PropSet::pushed`]). All of it as the plane's frame has it
    /// ([`crate::flight::Flight::frame_props`]). The props' own step, which resolves their mirrors
    /// against the car, is the caller's, after this. Without `props`, [`Machine::step_met`] exactly.
    ///
    /// [`PropSet::strike`]: crate::prop_set::PropSet::strike
    /// [`PropSet::pushed`]: crate::prop_set::PropSet::pushed
    #[allow(clippy::too_many_arguments)]
    pub fn step_props(
        &mut self,
        car: &Car,
        world: &mut impl crate::world::World,
        collision: &mut Collision,
        met: Option<&mut crate::flight::Met>,
        props: Option<CarProps<'_>>,
        dt: F,
        acos: &AcosTable,
        curve: &CurveTable,
        noise: &mut impl FnMut(F) -> F,
    ) {
        self.step_among(car, world, collision, met, props, None, dt, acos, curve, noise);
    }

    /// **[`Machine::step_props`] in the props' collide pass** (S340): a prop the car's strike wakes
    /// and files searches at once ([`PropSet::strike_in`]), as the original's node walk reaches it at
    /// the list's end behind the car, filed first: its terrain and scene entries go on its list after
    /// the car's mirrors and before its rays' on the hull, in `world`, its set's movers added to the
    /// pass's `movers` (the props' search's, [`crate::traffic::Traffic::frame_held`]), as the plane's
    /// [`crate::flight::Flight::frame_props_in`] has it. C34's ball strike: the ball's ground entry
    /// is on its list in its first step.
    ///
    /// [`PropSet::strike_in`]: crate::prop_set::PropSet::strike_in
    #[allow(clippy::too_many_arguments)]
    pub fn step_props_in(
        &mut self,
        car: &Car,
        world: &mut impl crate::world::World,
        collision: &mut Collision,
        met: Option<&mut crate::flight::Met>,
        props: CarProps<'_>,
        movers: &mut crate::prop_set::PropMovers,
        dt: F,
        acos: &AcosTable,
        curve: &CurveTable,
        noise: &mut impl FnMut(F) -> F,
    ) {
        self.step_among(car, world, collision, met, Some(props), Some(movers), dt, acos, curve, noise);
    }

    /// **[`Machine::step_met`] among the parked cars** (S499, [`crate::flight::Met::cars`]), on a
    /// track with no knockable props: [`Machine::step_props`] with none, so the cars meet it as a
    /// flown plane's frame has them (S489). Its rays strike their hulls after the world's and wake
    /// the car they strike; each awake car searches after it with `hull` (the car's, [`Kind::hull`])
    /// in its world, and steps in its world, its listeners after the car's: the car's resolve pushes
    /// the parked cars' bodies, and theirs the car's. [`crate::flight::Met::cars`] holds them after,
    /// each [`InPlane::stepped`] when it searched. With no cars, [`Machine::step_met`] exactly.
    #[allow(clippy::too_many_arguments)]
    pub fn step_cars(
        &mut self,
        car: &Car,
        world: &mut impl crate::world::World,
        collision: &mut Collision,
        met: &mut crate::flight::Met,
        hull: Option<&crate::track::Mesh>,
        dt: F,
        acos: &AcosTable,
        curve: &CurveTable,
        noise: &mut impl FnMut(F) -> F,
    ) {
        if met.cars.is_empty() {
            return self.step_met(car, world, collision, Some(met), dt, acos, curve, noise);
        }
        let mut none = crate::prop_set::PropSet::default();
        let props = CarProps { set: &mut none, me: 0, hull };
        self.step_among(car, world, collision, Some(met), Some(props), None, dt, acos, curve, noise);
    }

    #[allow(clippy::too_many_arguments)]
    fn step_among(
        &mut self,
        car: &Car,
        world: &mut impl crate::world::World,
        collision: &mut Collision,
        mut met: Option<&mut crate::flight::Met>,
        props: Option<CarProps<'_>>,
        pass: Option<&mut crate::prop_set::PropMovers>,
        dt: F,
        acos: &AcosTable,
        curve: &CurveTable,
        noise: &mut impl FnMut(F) -> F,
    ) {
        let Some(mut props) = props else { return self.step_met(car, world, collision, met, dt, acos, curve, noise) };
        let shapes = std::sync::Arc::clone(&props.set.shapes);
        let posed = crate::flight::knocks(props.set, &shapes);
        let knocks: Vec<crate::flight_search::Knock<'_>> = posed.iter().map(|p| p.knock).collect();
        // The parked cars' hulls in its search after the props' (S499), each where it stands nearest.
        let mut cars = met.as_deref_mut().map(|m| std::mem::take(&mut m.cars)).unwrap_or_default();
        let mut all = knocks.clone();
        all.extend(cars.iter().map(|c| parked_knock(&c.parked, self.body.position, &*world)));
        let (movers, mut knocked) = match met.as_deref_mut().filter(|m| strikes_bikes(m)) {
            Some(m) => search_met(collision, &mut self.body, &mut *world, dt, &all, m),
            None => collision.search_props(&mut self.body, &mut *world, dt, &all),
        };
        drop(all);
        let hit = knocked.split_off(knocks.len());
        for e in &mut collision.record.contacts {
            if let Some(j) = e.other.filter(|&o| o < crate::flight::WRECK_FROM).and_then(|o| o.checked_sub(crate::flight::KNOCK_FROM + knocks.len())) {
                e.other = Some(HULL_FROM + j);
            }
        }
        if let Some(m) = met.as_deref() {
            collision.file_mirrors(&m.mirrors);
        }
        let first = met.as_deref().map_or(0, |m| m.vehicles.len());
        let late = pass.map(|m| (dt, &mut *world as &mut dyn crate::world::World, m));
        let mut bodies = self.knocked(collision, &mut props, first, &posed, knocked, late);
        let at = bodies.len();
        if !cars.is_empty() {
            self.cars_met(collision, &mut cars, &hit, props.hull, &mut bodies, first, &mut *world, dt);
        }
        let live: Vec<usize> = cars.iter().enumerate().filter(|(_, c)| c.stepped).map(|(k, _)| k).collect();
        self.touching = collision.record.touching;
        let physics = crate::biplane::World { iterations: 1, rate: F(115.0) };
        if live.is_empty() {
            let met = met.as_deref_mut();
            physics.step(
                dt,
                &mut Stepping { m: self, car, world, acos, curve, noise, struck: Some((collision, &movers)), met, props: &mut bodies, knocks: &knocks },
            );
        } else {
            let met = met.as_deref_mut();
            let props = &mut bodies;
            physics.step(dt, &mut AmongCars { m: self, car, world, acos, curve, noise, collision, movers: &movers, met, props, knocks: &knocks, cars: &mut cars, live, at });
        }
        collision.record.search_ground = self.searches_terrain(world);
        // What its resolve did to the parked cars' bodies, where each really is (S499).
        for (c, b) in cars.iter_mut().zip(&bodies[at..]) {
            c.parked.pushed(&crate::wreck_plane::flown_of(b));
        }
        bodies.truncate(at);
        if let Some(m) = met {
            m.cars = cars;
        }
        props.set.pushed(&bodies);
    }

    /// **[`Machine::step_among`]'s parked cars** (S499), as a flown plane's frame has them (S489,
    /// [`crate::flight::Flight::frame_met`]): each its rays struck (`hit`, the mirrors its search put
    /// on each car's hull) woken, its body after the `first` vehicles and the struck props in
    /// `bodies`, and each awake one searched after the car, `hull` (the car's) in its world: the car's
    /// mirrors at the head of its list, its own entries after them, and their mirrors on the car's
    /// list. The car's entries on hull `j` ([`HULL_FROM`]` + j`) renamed to its body.
    #[allow(clippy::too_many_arguments)]
    fn cars_met(
        &self,
        collision: &mut Collision,
        cars: &mut [InPlane],
        hit: &[Vec<crate::contact::Contact>],
        hull: Option<&crate::track::Mesh>,
        bodies: &mut Vec<crate::prop_contact::Pushed>,
        first: usize,
        world: &mut dyn crate::world::World,
        dt: F,
    ) {
        let at = bodies.len();
        bodies.extend(cars.iter().enumerate().map(|(k, c)| {
            let f = c.parked.flown(0, self.body.position, &*world);
            crate::prop_contact::Pushed { collection: 0, object: k, collider: f.collider, body: f.body, mass: f.mass }
        }));
        for e in &mut collision.record.contacts {
            if let Some(j) = e.other.and_then(|o| o.checked_sub(HULL_FROM)) {
                e.other = Some(first + at + j);
            }
        }
        for (j, c) in cars.iter_mut().enumerate() {
            c.stepped = false;
            let mirrors = hit.get(j).map_or(&[][..], Vec::as_slice);
            if !mirrors.is_empty() {
                c.struck();
            }
            if !c.parked.awake() {
                continue;
            }
            let from = c.parked.machine.body.position;
            let knock = hull.map(|mesh| {
                let h = self.hull(0, mesh, collision);
                let mut at = h.at;
                at.0[3] = crate::nearest_copy(&*world, from, at.0[3]);
                let (to_world, centre, radius) = crate::world::Prop::posed(mesh, &at);
                crate::flight_search::Knock { mesh, to_world, centre, radius, velocity: self.body.velocity, capacity: h.capacity }
            });
            let theirs = c.search(&mut *world, dt, mirrors, knock);
            // Its own rays on the car's hull reach its hit listener, which calls the same enable
            // (S507, as S489 found in the plane's world): the driven word set, its brakes off.
            if !theirs.is_empty() {
                c.struck();
            }
            let near = self.body.position;
            let r = &mut collision.record;
            if !theirs.is_empty() {
                (r.touching, r.on_object, r.touching_other) = (true, true, true);
            }
            let room = r.capacity.saturating_sub(r.contacts.len());
            r.contacts.extend(
                theirs
                    .into_iter()
                    .take(room)
                    .map(|e| crate::contact::Contact { other: Some(first + at + j), point: crate::nearest_copy(&*world, near, e.point), ..e }),
            );
            let body = &mut bodies[at + j];
            c.by = body.body.position.sub(c.parked.machine.body.position);
            let mine = c.parked.machine.body;
            body.body = mine;
            (body.body.position, body.body.last_position) = (mine.position.add(c.by), mine.last_position.add(c.by));
            body.collider = c.parked.collision.collider;
        }
    }

    /// [`Machine::step_props`]' strike: the props the search struck (`knocked`, the mirrors on each of
    /// `posed`) woken and their mirrors filed, in the order the car's entries first name them, then
    /// the props' rays on its hull; the car's entries and the hull's mirrors renamed to the struck
    /// bodies' places after the `first` others of its resolve. Answers the struck bodies. With
    /// `late`, put on in the props' collide pass ([`Machine::step_props_in`]).
    fn knocked(
        &mut self,
        collision: &mut Collision,
        props: &mut CarProps<'_>,
        first: usize,
        posed: &[crate::flight::Posed<'_>],
        knocked: Vec<Vec<crate::contact::Contact>>,
        late: Option<(F, &mut dyn crate::world::World, &mut crate::prop_set::PropMovers)>,
    ) -> Vec<crate::prop_contact::Pushed> {
        use crate::flight::KNOCK_FROM;
        let me = props.me;
        let mut order: Vec<usize> = Vec::new();
        // Names from [`WRECK_FROM`] on are no props: the wreck bikes' mirrors, and (S499) its entries
        // on the parked cars' hulls ([`HULL_FROM`]).
        //
        // [`WRECK_FROM`]: crate::flight::WRECK_FROM
        let prop = |o: usize| (o < crate::flight::WRECK_FROM).then_some(o).and_then(|o| o.checked_sub(KNOCK_FROM));
        for k in collision.record.contacts.iter().filter_map(|e| e.other).filter_map(prop) {
            if !order.contains(&k) {
                order.push(k);
            }
        }
        for (k, m) in knocked.iter().enumerate() {
            if !m.is_empty() && !order.contains(&k) {
                order.push(k);
            }
        }
        let struck = order
            .iter()
            .map(|&k| {
                let mirrors = knocked[k].iter().map(|e| crate::contact::Contact { other: Some(me), ..*e }).collect();
                (posed[k].collection, posed[k].object, mirrors)
            })
            .collect();
        let mut strike = crate::prop_contact::Strike { contacts: Vec::new(), struck, ..Default::default() };
        let mut bodies = match late {
            // Named as the props' step's striker `me`; the late props' probes' mirrors that
            // [`PropSet::strike_in`] puts on the strike's entries are a riding bike's frame's (B29),
            // not the car's: its hull is met below.
            //
            // [`PropSet::strike_in`]: crate::prop_set::PropSet::strike_in
            Some((dt, world, movers)) => {
                strike.by = Some((me, first));
                props.set.strike_in(&mut strike, dt, world, movers)
            }
            None => props.set.strike(&strike),
        };
        for e in &mut collision.record.contacts {
            if let Some(k) = e.other.and_then(prop) {
                e.other = order.iter().position(|&j| j == k).map(|i| first + i);
            }
        }
        let Some(hull) = props.hull else { return bodies };
        let node = &collision.record.node;
        let at = [node.rows[0], node.rows[1], node.rows[2], node.origin];
        let on = crate::prop_wreck::struck_on(props.set, hull, at, COLLIDABLE, collision.collider.collides_with, me);
        if on.is_empty() {
            return bodies;
        }
        let places: Vec<usize> = on
            .struck
            .iter()
            .map(|p| match bodies.iter().position(|b| (b.collection, b.object) == (p.collection, p.object)) {
                Some(i) => i,
                None => {
                    bodies.push(p.clone());
                    bodies.len() - 1
                }
            })
            .collect();
        let r = &mut collision.record;
        (r.touching, r.on_object, r.touching_other) = (true, true, true);
        let room = r.capacity.saturating_sub(r.contacts.len());
        r.contacts.extend(on.mirrors.iter().map(|e| crate::contact::Contact { other: e.other.map(|i| first + places[i]), ..*e }).take(room));
        bodies
    }

    /// **Its hull as the riders' bikes meet it** (F26): `mesh` (the car's `f.col`, [`Kind::hull`])
    /// where its collision object's node stands it (the body's pose, as its next search will place
    /// it), the room `collision`'s record has, and the car's category ([`COLLIDABLE`]). Seat `seat`
    /// is the car's player.
    pub fn hull<'a>(&self, seat: usize, mesh: &'a crate::track::Mesh, collision: &Collision) -> crate::rider_contact::Hull<'a> {
        let n = crate::wreck_bike::node_of(&self.body);
        let at = crate::math::Mat4([n.rows[0], n.rows[1], n.rows[2], n.origin]);
        crate::rider_contact::Hull { seat, mesh, at, capacity: collision.record.capacity, category: COLLIDABLE }
    }

    /// Its body as a bike's resolve pushes it (F26, [`crate::rider_contact::Bump::planes`]): the
    /// collision object's tuning and the body, the copy nearest `from` (the bike) on a world that
    /// wraps, as a flown plane's is ([`crate::flight::Flight::flown`]).
    pub fn flown(&self, seat: usize, collision: &Collision, from: Vf, world: &dyn crate::world::World) -> crate::rider_contact::Flown {
        let mut body = self.body;
        let near = crate::nearest_copy(world, from, body.position);
        if near != body.position {
            let by = near.sub(body.position);
            body.position = near;
            body.last_position = body.last_position.add(by);
        }
        crate::rider_contact::Flown { seat, collider: collision.collider, body, mass: collision.mass, quad: None }
    }

    /// The body a bike's resolve pushed, written back (F26): its speeds, spins and what the resolve
    /// wrote, where the car really is, as a plane's is ([`crate::flight::Flight::pushed`]).
    pub fn pushed(&mut self, collision: &mut Collision, p: &crate::rider_contact::Flown) {
        let (position, last_position) = (self.body.position, self.body.last_position);
        self.body = p.body;
        (self.body.position, self.body.last_position) = (position, last_position);
        collision.collider = p.collider;
    }

    /// The measure's last word, on the collision record's terrain switch: the next frame's search
    /// takes the terrain with the body rays too when the car's up is 45° or more off the terrain's
    /// normal under its centre of mass (`up · n <= 0.707`, on the accumulator). Upright, the tires
    /// alone meet the ground.
    pub fn searches_terrain(&self, world: &mut impl crate::world::World) -> bool {
        let (_, n) = world.ground(self.body.position);
        let u = self.up;
        Acc::mula(u.x(), n.x()).madda(u.y(), n.y()).madd(u.z(), n.z()) <= F(0.707)
    }
}

/// Whether `met` has the car's rays strike the bikes (F76): it names the car among their others,
/// and holds every named bike's chassis and their frame mesh.
pub fn strikes_bikes(met: &crate::flight::Met) -> bool {
    met.named.is_some() && met.bike_col.is_some() && !met.bikes.is_empty() && met.rides.len() == met.bikes.len()
}

/// **The car's search with the riders' bikes in it** (F76): [`Collision::search_props`] with
/// `met`'s bikes ([`crate::flight::Met::rides`]) in its scene as a plane's search has them
/// ([`crate::flight::Flight::frame_met`]): each bike's frame collidable its collision record's, a
/// record after the car's, ahead of the knockable props'. R22's capture: the buggy's three body rays
/// that strike the bike's tail are its record's entries naming the bike (kind 0), their mirrors at
/// the head of the bike's list, before the bike's own entry.
///
/// An entry on bike `k` names its player ([`crate::flight::Met::bikes`]), which the car's resolve
/// pushes ([`crate::flight::Met::pushed`]); one on knock `k` names [`crate::flight::KNOCK_FROM`]` + k`,
/// as [`Collision::search_props`] names it. The mirrors each bike's record took are
/// [`crate::flight::Met::struck`]'s, by player, naming the car as [`crate::flight::Met::named`];
/// the answer's second list is the mirrors each knock's record took, in `knocks`' order.
#[allow(clippy::type_complexity)]
pub fn search_met(
    collision: &mut Collision,
    body: &mut crate::biplane::Body,
    world: &mut dyn crate::world::World,
    dt: F,
    knocks: &[crate::flight_search::Knock<'_>],
    met: &mut crate::flight::Met,
) -> (Vec<crate::contact::Mover>, Vec<Vec<crate::contact::Contact>>) {
    use crate::contact_search::{self, Search};
    use crate::flight_search::{self, Bikes, Scene};
    let rays = collision.rays.clone();
    collision.record.node = crate::wreck_bike::node_of(body);
    contact_search::clear(&mut collision.record, Some(&mut *body));
    let (field, props, trees) = flight_search::near_world(world, &rays, &collision.record.node, &mut collision.nearby);
    let bounds = field.as_deref().map(|f| {
        let (lo, hi) = (f.min(), f.max());
        ([F(lo.x()), F(lo.z())], [F(hi.x()), F(hi.z())])
    });
    let col = met.bike_col.clone().expect("strikes_bikes");
    let rides = std::mem::take(&mut met.rides);
    let velocities: Vec<Vf> = met.bikes.iter().map(|&p| met.vehicles[p].velocity).collect();
    let owned = velocities.len();
    let bikes = Some(Bikes { mesh: &col.frame, bikes: &rides });
    let mut scene = Scene::flown(props, trees, bikes, bounds, &rays, &collision.record.node, &mut collision.nearby)
        .sweep_of(crate::prop_world::sweep_categories())
        .owned(&velocities);
    if !knocks.is_empty() {
        scene = scene.knocking(knocks, 1 + owned);
    }
    let mut records = vec![std::mem::take(&mut collision.record)];
    records.extend(velocities.iter().map(|_| contact_search::Record { capacity: crate::rider_contact::CAPACITY, ..contact_search::Record::default() }));
    records.extend(knocks.iter().map(|k| contact_search::Record { capacity: k.capacity, ..contact_search::Record::default() }));
    contact_search::collide(&mut records, 0, Some(body), field.as_deref(), &mut scene, dt);
    let movers = Search::movers(&scene);
    drop(scene);
    let mut records = records.into_iter();
    collision.record = records.next().unwrap_or_default();
    let n = met.vehicles.len();
    met.struck = vec![Vec::new(); n];
    met.pushed = vec![false; n];
    for e in &mut collision.record.contacts {
        match e.other {
            Some(k) if (1..=owned).contains(&k) => {
                let p = met.bikes[k - 1];
                e.other = Some(p);
                met.pushed[p] = true;
            }
            Some(k) if k > owned => e.other = Some(crate::flight::KNOCK_FROM + k - 1 - owned),
            _ => {}
        }
    }
    for (bike, &p) in records.by_ref().take(owned).zip(&met.bikes) {
        met.struck[p] = bike.contacts.into_iter().map(|e| crate::contact::Contact { other: met.named, ..e }).collect();
    }
    met.rides = rides;
    (movers, records.map(|r| r.contacts).collect())
}

/// **The car's nearby list gathered with the riders' bikes in it** (F78): [`Collision::refresh`]
/// with `met`'s bikes ([`crate::flight::Met::rides`]) in the world, as a flown plane's refill has
/// them ([`crate::flight::Flight`]'s post). The search reads this list, so a car whose list was
/// gathered without them never strikes a bike ([`search_met`]).
pub fn refresh_met(collision: &mut Collision, body: &crate::biplane::Body, world: &mut dyn crate::world::World, met: &crate::flight::Met) {
    use crate::flight_search::{self, Bikes};
    let Some(col) = met.bike_col.as_deref() else { return collision.refresh(body, world) };
    let node = crate::wreck_bike::node_of(body);
    let (field, props, trees) = flight_search::near_world(world, &collision.rays, &node, &mut collision.nearby);
    let bounds = field.as_deref().map(|f| {
        let (lo, hi) = (f.min(), f.max());
        ([F(lo.x()), F(lo.z())], [F(hi.x()), F(hi.z())])
    });
    let bikes = Bikes { mesh: &col.frame, bikes: &met.rides };
    flight_search::refresh_flown(props, trees, Some(&bikes), bounds, &collision.rays, &node, &mut collision.nearby);
}

// --- the tire contact sort (S229a) -----------------------------------------------------------------

/// **The contact sort's ray probe** (S229a), off the tires' `mounts` ([`TIRES`] order): the car's second ray probe, which the `.phy` loader
/// builds once after the tires are read. One ray a tire, in the car's frame, straight down its `y`
/// from the suspension's fully compressed top (the mount plus `MaxCompression`) to a radius below the
/// mount; its tree a root over all four and a node over each axle's pair, which the build's stable
/// partition (the greater half first) leaves front then rear and, within each, right (`+x`) then
/// left. Checked against the baja's in S229a's image; the build's general split, which a list of
/// four never reaches past, is not ported.
pub fn sort_rays(mounts: [Vf; 4], tires: &[Tire; 4]) -> crate::flight_search::Rays {
    let segment = |k: usize| {
        let (m, t) = (mounts[k], &tires[k]);
        let top = F(m.y()) + t.max_compression;
        let bottom = F(m.y()) - t.radius;
        crate::ray_probe::Segment { from: Vf::new(m.x(), top.0, m.z(), m.w()), to: Vf::new(m.x(), bottom.0, m.z(), m.w()) }
    };
    let segments: Vec<_> = [FRONT_RIGHT, FRONT_LEFT, REAR_RIGHT, REAR_LEFT].map(segment).into();
    let ends = |s: &[crate::ray_probe::Segment]| s.iter().flat_map(|s| [s.from, s.to]).collect::<Vec<_>>();
    let mut root = crate::ride::ray_node(&ends(&segments));
    root.children = [1, 2];
    let mut front = crate::ride::ray_node(&ends(&segments[..2]));
    front.children = [0x8000, 0x8001];
    let mut rear = crate::ride::ray_node(&ends(&segments[2..]));
    rear.children = [0x8002, 0x8003];
    crate::flight_search::Rays { root: 0, nodes: vec![root, front, rear], segments }
}

/// The tire a sort ray's hit goes to, by where the ray hangs (`from`, the car's frame): forward of
/// the origin a front, right of it (`+x`) the right one; zero is rear and left.
pub fn sorted_tire(from: Vf) -> usize {
    match (F(from.z()) > F::ZERO, F(from.x()) > F::ZERO) {
        (true, true) => FRONT_RIGHT,
        (true, false) => FRONT_LEFT,
        (false, true) => REAR_RIGHT,
        (false, false) => REAR_LEFT,
    }
}

impl Machine {
    /// **The tire contact sort** (S229a), once a frame between the tires' clear and their probes: the
    /// sort's ray probe ([`sort_rays`]) placed at the car's frame as the last measure left it
    /// ([`Machine::matrix`]) and searched against the collision object's nearby list as last
    /// gathered (the scenery's meshes and plants; never the terrain), each ray's hit handed to its
    /// tire ([`sorted_tire`], [`Wheel::set_contact`]).
    ///
    /// Not here: a struck object's own listeners, which the original calls on every hit, and the
    /// struck object and mover a tire keeps for its contact's velocity — S229a's capture sorts onto a
    /// still prop only, whose ground velocity is zero as the terrain's is.
    pub fn sort_contacts(&mut self, car: &Car, collision: &mut Collision, world: &mut impl crate::world::World) {
        self.sort_contacts_knocking(car, collision, world, &[]);
    }

    /// [`Machine::sort_contacts`] with the knockable props' meshes in its scene after the world's
    /// (S335, [`flight_search::Scene::knocking`]): a tire meets a prop as it meets a scenery one.
    pub fn sort_contacts_knocking(
        &mut self,
        car: &Car,
        collision: &mut Collision,
        world: &mut impl crate::world::World,
        knocks: &[crate::flight_search::Knock<'_>],
    ) {
        use crate::contact_search::Search;
        use crate::flight_search::{self, Scene};
        let rays = sort_rays(self.wheels.each_ref().map(|w| w.mount), &car.tires);
        let node = crate::rider_contact::node(&self.matrix);
        let world: &mut dyn crate::world::World = world;
        let (field, props, trees) = flight_search::near_world(world, &rays, &node, &mut collision.nearby);
        if props.is_none() && trees.is_none() && knocks.is_empty() {
            return;
        }
        let bounds = field.as_deref().map(|f| {
            let (lo, hi) = (f.min(), f.max());
            ([F(lo.x()), F(lo.z())], [F(hi.x()), F(hi.z())])
        });
        let mut scene = Scene::flown(props, trees, None, bounds, &rays, &node, &mut collision.nearby);
        if !knocks.is_empty() {
            scene = scene.knocking(knocks, 1);
        }
        for hit in scene.rays(true) {
            self.wheels[sorted_tire(hit.from)].set_contact(hit.point, hit.normal);
        }
    }
}

/// The knockable props a car drives among (S335, [`Machine::step_props`]): the set, the index the
/// props' step gives the car among its strikers, which their mirrors name, and its collidable mesh
/// (the car's `f.col`, [`Kind::hull`]) for the props' own rays, if the disc has it.
pub struct CarProps<'a> {
    pub set: &'a mut crate::prop_set::PropSet,
    pub me: usize,
    pub hull: Option<&'a crate::track::Mesh>,
}

// --- a car in a seat's lane (S249) -----------------------------------------------------------------

/// **A seat's car while it is out** (S249), as [`crate::Sim::car`] holds it: which car, the car
/// itself, and its collision object. What a tick changes and a rollback puts back, so it lives in
/// the seat's state as the plane's flight does ([`crate::Sim::plane`]); the car's `.phy` and its
/// model are constants the shell keeps, the collision's ray probe one the codec leaves out.
#[derive(Clone, Debug)]
pub struct Lane {
    pub kind: Kind,
    pub machine: Machine,
    pub struck: Option<Collision>,
}
