//! The car loader (S149) against the original's: every word the loader writes, read back out of each
//! car a free-ride track parks — the monster truck free09 parks beside the bike, in a RAM image of the
//! human's first freeride session, and the baja and buggy free07 and free11 park (S149a's images).
//! No interpreter — the image is the original's answer, and every comparison names its key.

use game_sim::archive::{Psr, Res};
use game_sim::car::{self, Car, Kind};
use game_sim::phy;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, skip};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

/// A parked car in a RAM image in `context/states`, and the two words that say it is the one: its
/// vtable (after the base classes' header) and its machine kind.
struct Parked {
    kind: Kind,
    state: &'static str,
    /// How to make the image when it is missing.
    recipe: &'static str,
    at: u32,
    vtable: u32,
    machine: u32,
}
const VTABLE: u32 = 0x48;
const KIND: u32 = 0xd50;
/// The truck's class, and the one the buggy and the baja share.
const TRUCK_VTABLE: u32 = 0x0045ed50;
const CAR_VTABLE: u32 = 0x0045cf80;

const TRUCK: Parked = Parked {
    kind: Kind::MonsterTruck,
    state: "first_recording.p2m2_SaveState.eeMemory.bin",
    recipe: "`./kit state ram context/recordings/first_recording.p2m2_SaveState.p2s` writes it",
    at: 0x00e1e210,
    vtable: TRUCK_VTABLE,
    machine: 4,
};
/// S149a's boots: research/journal/2026-10-29-s149a-car-captures/boot.py, then `./kit state ram`.
const BAJA: Parked = Parked {
    kind: Kind::Baja,
    state: "s149a-free07.ee.bin",
    recipe: "S149a's boot.py free07 2 5 <slot> 5 (the baja parked, the human on the bike), then `./kit state ram <slot> --out` it",
    at: 0x00605270,
    vtable: CAR_VTABLE,
    machine: 5,
};
const BUGGY: Parked = Parked {
    kind: Kind::Buggy,
    state: "s149a-free11.ee.bin",
    recipe: "S149a's boot.py free11 2 5 <slot> 3 (the buggy parked), then `./kit state ram <slot> --out` it",
    at: 0x00611ba0,
    vtable: CAR_VTABLE,
    machine: 3,
};

/// The car's layout: the rigid-body part, then the pointers to what it built.
const PART: u32 = 0x50;
const BODY: u32 = 0x160;
const COLLISION: u32 = 0x170;
const ENGINE: u32 = 0x544;
const TIRES: [u32; 4] = [0x548, 0x54c, 0x550, 0x554];
const WINGS: u32 = 0x4a8;

/// A car's file, out of `machine.res` in MX.PSR.
fn car_file(kit: &Kit, kind: Kind) -> Option<Vec<u8>> {
    let at = kit.ctx("iso").join("MX.PSR");
    let Ok(mut f) = File::open(&at) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", at.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find("machine.res").expect("machine.res is in MX.PSR");
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    let res = Res::parse(&bytes).expect("machine.res");
    // The archive spells its members lowercased and the loader does not, so the lookup folds.
    let e = res
        .entries
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(kind.phy()))
        .unwrap_or_else(|| panic!("machine.res has no {}", kind.phy()));
    Some(res.read(&bytes, e).expect("its bytes"))
}

fn lanes(v: Vf) -> [u32; 4] {
    v.to_bits()
}

/// Every word of the truck's parameters as the loader left them, against the port's parse of
/// `MonsterTruck\monstertruck.phy` off the disc.
#[test]
fn monster_truck_phy_matches_the_parked_truck() {
    prove(&TRUCK);
}

/// The baja free07 parks, against `Baja\baja.phy`.
#[test]
fn baja_phy_matches_the_parked_baja() {
    prove(&BAJA);
}

/// The buggy free11 parks, against `Buggy\buggy.phy`.
#[test]
fn buggy_phy_matches_the_parked_buggy() {
    prove(&BUGGY);
}

fn prove(car_at: &Parked) {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return;
        }
    };
    let at = kit.ctx("states").join(car_at.state);
    let img = match Ee::from_ram(&at) {
        Ok(ee) => ee,
        Err(why) => {
            skip(&format!("{why} — {}", car_at.recipe));
            return;
        }
    };
    let (b, kind) = (car_at.at, car_at.kind);
    if img.u32_at(b + VTABLE) != car_at.vtable || img.u32_at(b + KIND) != car_at.machine {
        skip(&format!("no {kind:?} at {b:#010x} in {} — a different moment, or build", at.display()));
        return;
    }
    let Some(bytes) = car_file(&kit, kind) else { return };
    let c = Car::read_bytes(&bytes, kind);

    let ptr = |at: u32| {
        let p = img.u32_at(at);
        assert_ne!(p, 0, "nothing at {at:#010x}");
        p
    };
    let (body, coll, engine) = (ptr(b + BODY), ptr(b + COLLISION), ptr(b + ENGINE));
    let tires = TIRES.map(|o| ptr(b + o));
    let mut want: Vec<(String, u32, u32)> = Vec::new();
    let mut f1 = |what: &str, at: u32, v: F| want.push((what.to_string(), at, v.to_bits()));
    let m = &c.mass_properties;
    let part = b + PART;

    // [Mass Properties] — the part loader's words, as the plane's proof reads them ...
    f1("GravityScale", part + 0x140, m.gravity_scale);
    f1("ResetTime", part + 0x1c0, m.reset_time);
    f1("SpeedThreshold", part + 0x1c4, m.speed_threshold);
    f1("AngularRateThreshold", part + 0x1c8, m.angular_rate_threshold);
    f1("AccelerationThreshold", part + 0x1cc, m.acceleration_threshold);
    f1("AngularAccelerationThreshold", part + 0x1d0, m.angular_acceleration_threshold);
    f1("SpeedImpulseThreshold", part + 0x1d4, m.speed_impulse_threshold);
    f1("BodyAngularSpeedImpulseThreshold", part + 0x1d8, m.body_angular_speed_impulse_threshold);
    f1("Weight, as the body's mass", body + 0xc, m.mass);
    f1("CollisionWeight, as the collision mass", coll + 0x64, m.collision_mass);
    f1("MaxPenetration", coll + 0x118, m.max_penetration);
    f1("SpringStiffness", coll + 0x11c, m.spring_stiffness);
    f1("SpringDamping", coll + 0x120, m.spring_damping);
    f1("FrictionZone1", coll + 0x190, m.friction_zone1);
    f1("BodyFriction", coll + 0x194, m.body_friction);
    // ... and the car's own.
    f1("YawRateLimit", b + 0x538, c.yaw_rate_limit);
    f1("CofGTravelForward", b + 0x260, c.cofg_travel_forward);
    f1("CofGTravelBack", b + 0x264, c.cofg_travel_back);
    f1("HandBrakeYawRateLimit", b + 0x53c, c.hand_brake_yaw_rate_limit);
    f1("HandBrakeFrictionScale", b + 0x41c, c.hand_brake_friction_scale);
    f1("HandBrakeBrakeScale", b + 0x420, c.hand_brake_brake_scale);
    f1("HandBrakeFadeTime", b + 0x424, c.hand_brake_fade_time);

    // The four tires and their surfaces.
    for (k, (t, tire)) in tires.iter().zip(&c.tires).enumerate() {
        let name = car::TIRES[k];
        let mut key = |what: &str, at: u32, v: F| f1(&format!("[{name}] {what}"), t + at, v);
        key("MaxCompression", 0x3c, tire.max_compression);
        key("MaxCompressionRate", 0x40, tire.max_compression_rate);
        key("MaxDecompressionRate", 0x44, tire.max_decompression_rate);
        key("Spring, as a rate", 0x48, tire.spring);
        key("BounceDampingRatio, as a rate", 0x4c, tire.bounce_damping);
        key("ReboundDampingRatio, as a rate", 0x50, tire.rebound_damping);
        key("TireInertia, inverted", 0x54, tire.inv_inertia);
        key("GyroInertia", 0x58, tire.gyro_inertia);
        key("MaxSteerAngle", 0x5c, tire.max_steer_angle);
        key("MaxTireScale", 0x60, tire.max_tire_scale);
        key("MaxTireAngleRate", 0xa4, tire.max_tire_angle_rate);
        key("MinBrakingSlip", 0xac, tire.min_braking_slip);
        key("MaxBrakingSlip", 0xb0, tire.max_braking_slip);
        key("TireRadius", 0x1f8, tire.radius);
        key("TireWidth", 0x1fc, tire.width);
        key("TireRadius, squared", 0x200, tire.radius_squared);
        key("TireRadius, inverted", 0x204, tire.inv_radius.expect("a positive radius"));
        key("CamberRollRatio", 0x9c0, tire.camber_roll_ratio);
        if let Some(top) = c.top_wheel_speed(k) {
            key("the redline's wheel speed in first", 0xa0, top);
        }
        for (n, s) in tire.surfaces.iter().enumerate() {
            let at = 4 * n as u32;
            let mut key = |what: &str, o: u32, v: F| f1(&format!("[{name} Surface {}] {what}", n + 1), t + o + at, v);
            key("C1", 0x634, s.c1);
            key("C2", 0x674, s.c2);
            key("C3", 0x6b4, s.c3);
            key("C4", 0x6f4, s.c4);
            key("A0", 0x734, s.a0);
            key("A1", 0x774, s.a1);
            key("A3", 0x7b4, s.a3);
            key("A2, inverted", 0x7f4, s.inv_a2);
            key("A4, inverted", 0x834, s.inv_a4);
            key("MomentScale", 0x874, s.moment_scale);
            key("CorneringStiffness", 0x8b4, s.cornering_stiffness);
            key("MaxNormalLoad", 0x8f4, s.max_normal_load);
            key("FrictionPeak", 0x934, s.friction_peak);
            key("FrictionEndPoint", 0x974, s.friction_end_point);
            key("MaxBrakingTorque", 0x5b4, s.max_braking_torque);
            key("WheelFriction", 0x5f4, s.wheel_friction);
        }
    }

    // [Engine]
    let e = &c.engine;
    f1("IdleThrottle", engine + 0x4c, e.idle_throttle);
    f1("EngineBraking, scaled", engine + 0x50, e.engine_braking);
    f1("FinalGearRatio", engine + 0x54, e.final_gear_ratio);
    f1("TransmissionLoss, as a fraction", engine + 0x58, e.transmission_loss);
    f1("EngineInertia", engine + 0x5c, e.inertia);
    f1("EngineInertia, inverted", engine + 0x60, e.inv_inertia.expect("a positive inertia"));
    f1("PumpTorqueScale, scaled", engine + 0x64, e.pump_torque_scale);
    f1("maxK", engine + 0x68, e.max_k);
    f1("MinRPM", engine + 0x70, e.min_rpm);
    f1("MinTractionSlip", engine + 0x78, e.min_traction_slip);
    f1("MaxTractionSlip", engine + 0x7c, e.max_traction_slip);
    f1("NOXVolume", engine + 0x84, e.nox_volume);
    f1("NOXBurnRate", engine + 0x88, e.nox_burn_rate);
    f1("NOXHPBoost", engine + 0x8c, e.nox_hp_boost);
    let s = &e.shift;
    f1("LowLoadRPM", engine + 0x90, s.low_load_rpm);
    f1("HighLoadRPM", engine + 0x94, s.high_load_rpm.expect("the file has it"));
    f1("LowLoadUpShiftRPM", engine + 0x98, s.low_load_up_shift_rpm);
    f1("HighLoadUpShiftRPM", engine + 0x9c, s.high_load_up_shift_rpm);
    f1("LowLoadDownShiftRPM", engine + 0xa0, s.low_load_down_shift_rpm);
    f1("HighLoadDownShiftRPM", engine + 0xa4, s.high_load_down_shift_rpm);
    f1("ShiftTime", engine + 0xa8, s.shift_time);
    f1("ShiftUpDuration", engine + 0xb4, s.shift_up_duration);
    f1("ShiftUpThrottle", engine + 0xb8, s.shift_up_throttle);
    f1("ShiftDownDuration", engine + 0xbc, s.shift_down_duration);
    f1("ShiftDownThrottle", engine + 0xc0, s.shift_down_throttle);
    let first = e.first.expect("a car has gears");
    f1("first gear's ratio", engine + 0xe4, first.ratio);
    f1("first gear's ratio, inverted", engine + 0xe8, first.inv_ratio);
    f1("MaxPumpTorque", engine + 0xf0, e.max_pump_torque);
    f1("the redline", engine + 0x130, e.max_rpm.expect("an RPM table"));
    let (rpm, torque, gears) = (ptr(engine + 0xd4), ptr(engine + 0xd8), ptr(engine + 0xe0));
    for (k, (&r, &t)) in e.rpm.iter().zip(&e.torque).enumerate() {
        f1(&format!("RPM{}", k + 1), rpm + 4 * k as u32, r);
        f1(&format!("HP{}, as a torque", k + 1), torque + 4 * k as u32, t);
    }
    for (k, &g) in e.gear_ratios.iter().enumerate() {
        f1(&format!("GearRatio{}", k + 1), gears + 4 * k as u32, g);
    }
    // [Aerodynamics]
    let a = &c.aero;
    f1("AeroSurfaceArea", b + 0x470, a.aero_surface_area);
    f1("AirborneDamping", b + 0x474, a.airborne_damping);
    f1("UnrollGain", b + 0x478, a.unroll_gain);
    f1("UnrollTorque", b + 0x47c, a.unroll_torque);
    f1("UnyawGain", b + 0x480, a.unyaw_gain);
    f1("UnyawTorque", b + 0x484, a.unyaw_torque);
    f1("BrakeGyroTorque", b + 0x4a0, a.brake_gyro_torque);
    let wings = ptr(b + WINGS);
    for (k, w) in a.wings.iter().enumerate() {
        let at = wings + 0x20 * k as u32;
        f1(&format!("Wing {} LiftSlope", k + 1), at, w.lift_slope);
        f1(&format!("Wing {} AngleOfAttack", k + 1), at + 4, w.angle_of_attack);
        f1(&format!("Wing {} BasicDrag", k + 1), at + 8, w.basic_drag);
        f1(&format!("Wing {} InducedDrag", k + 1), at + 0xc, w.induced_drag);
    }
    // [Preload] and the clutch: present in the file, so each one is written.
    let p = &c.preload;
    f1("PreloadTime", b + 0x44c, p.preload_time.expect("in the file"));
    f1("MinPreloadTime", b + 0x454, p.min_preload_time.expect("in the file"));
    f1("MaxPreloadTime", b + 0x458, p.max_preload_time.expect("in the file"));
    f1("MaxPreload", b + 0x45c, p.max_preload.expect("in the file"));
    f1("ClutchTorqueScaleTime", b + 0x434, c.clutch_torque_scale_time.expect("in the file"));
    f1("MaxClutchTorqueScale", b + 0x438, c.max_clutch_torque_scale.expect("in the file"));
    f1("MaxClutchTorqueScaleSpeed", b + 0x43c, c.max_clutch_torque_scale_speed.expect("in the file"));
    // [Wrecks]
    f1("MaxWreckTime", b + 0x2d4, c.max_wreck_time);
    // [Joystick]
    let j = &c.joystick;
    f1("MaxAnalogThrottle", b + 0x408, j.max_analog_throttle);
    f1("DigitalThrottleSpeed", b + 0x40c, j.digital_throttle_speed);
    f1("DigitalSideSlipAngle", b + 0x410, j.digital_side_slip_angle);
    f1("BrakeRateLimit", b + 0x4c4, j.brake_rate_limit);
    f1("GasRateLimit", b + 0x4c8, j.gas_rate_limit);
    f1("XDPadRateLimitOn", b + 0x4cc, j.x_dpad_rate_limit_on);
    f1("XDPadRateLimitOff", b + 0x4d0, j.x_dpad_rate_limit_off);
    f1("XAxisRateLimit", b + 0x4d4, j.x_axis_rate_limit);
    for (t, (prefix, table)) in
        [("Steer", j.steer), ("WheelSteer", j.wheel_steer), ("DPadSteer", j.dpad_steer)].iter().enumerate()
    {
        for (k, [speed, scale]) in table.iter().enumerate() {
            let at = b + 0x4d8 + 0x18 * t as u32 + 8 * k as u32;
            f1(&format!("{prefix}Speed{}", k + 1), at, *speed);
            f1(&format!("{prefix}Scale{}", k + 1), at + 4, *scale);
        }
    }
    for (k, [speed, scale]) in j.rear_steer.iter().enumerate() {
        f1(&format!("RearSteerSpeed{}", k + 1), b + 0x520 + 8 * k as u32, *speed);
        f1(&format!("RearSteerScale{}", k + 1), b + 0x524 + 8 * k as u32, *scale);
    }
    // [Animation]: the truck's class keeps its pelvis box further on and alone reads a rear-tire
    // steer scale; the buggy's and the baja's reads none.
    let pelvis = match kind {
        Kind::MonsterTruck => {
            f1("RearTireSteerScale", b + 0xfd4, c.animation.rear_tire_steer_scale.expect("the truck's"));
            b + 0x1010
        }
        Kind::Buggy | Kind::Baja => {
            assert_eq!(c.animation.rear_tire_steer_scale, None, "{kind:?} reads no RearTireSteerScale");
            b + 0xf20
        }
    };

    // The quadwords, lane by lane. The loader also copies `CofG` into the car's current centre of
    // mass, but a parked car with no driver has since moved that one to `CofGAI`, so it is the step's
    // word by now and not the loader's — though where the file has no `CofGAI` the two are the same.
    let mut quads: Vec<(String, u32, Vf)> = vec![
        ("CofG, as the car's".into(), b + 0x230, m.cofg),
        ("CofGAI".into(), b + 0x240, c.cofg_ai),
        ("CollisionCofGOffset".into(), coll + 0x90, m.collision_cofg_offset),
        ("HighSpeedRayOffset".into(), coll + 0xb0, m.high_speed_ray_offset),
        ("AirborneControlMoment".into(), b + 0x490, a.airborne_control_moment),
        ("PelvisBoxPosition".into(), pelvis, c.animation.pelvis_box_position),
        ("PelvisBoxSize".into(), pelvis + 0x10, c.animation.pelvis_box_size),
    ];
    if c.cofg_ai == m.cofg {
        quads.push(("CofG, as the car's current (= CofGAI)".into(), b + 0x250, m.cofg));
    }
    for (k, w) in a.wings.iter().enumerate() {
        quads.push((format!("Wing {} Position", k + 1), wings + 0x20 * k as u32 + 0x10, w.position));
    }
    for (what, at, v) in quads {
        for (k, lane) in lanes(v).into_iter().enumerate() {
            want.push((format!("{what}.{}", ["x", "y", "z", "w"][k]), at + 4 * k as u32, lane));
        }
    }

    // The integer words: flags and counts as the getter writes them, and the drive-train's slots as
    // the tire pointers they hold.
    let mut ints: Vec<(String, u32, u32)> = vec![
        ("CollisionEnabled".into(), part + 0x54, m.collision_enabled as u32),
        ("DetectCollisionsWhenDisabled".into(), part + 0x6c, m.detect_collisions_when_disabled as u32),
        ("ResetOffEdgeOfWorld".into(), part + 0x70, m.reset_off_edge_of_world as u32),
        ("CheckForHighSpeedCollisions".into(), coll + 0x3c, m.check_for_high_speed_collisions as u32),
        ("TCSEnabled".into(), engine + 0x74, e.tcs_enabled as u32),
        ("NumberOfRPMPoints".into(), engine + 0xd0, e.rpm.len() as u32),
        ("NumberOfGears".into(), engine + 0xdc, e.gear_ratios.len() as u32),
        ("the gear the box starts in".into(), engine + 0xcc, 1),
        ("NumberOfWings".into(), b + 0x4a4, a.number_of_wings as u32),
        ("SquareSteeringInput".into(), b + 0x4b0, j.square_steering_input as u32),
    ];
    for (k, slot) in e.driven.iter().enumerate() {
        ints.push((format!("drive slot {k}"), engine + 0x30 + 4 * k as u32, slot.map_or(0, |t| tires[t])));
    }
    for (k, (t, tire)) in tires.iter().zip(&c.tires).enumerate() {
        ints.push((format!("[{}] ABSEnabled", car::TIRES[k]), t + 0xa8, tire.abs_enabled as u32));
    }
    want.extend(ints);

    let mut bad = 0;
    for (what, at, ours) in &want {
        let theirs = img.u32_at(*at);
        if theirs != *ours {
            println!(
                "{what}: the parked {kind:?} {theirs:#010x} ({}) ours {ours:#010x} ({})",
                f32::from_bits(theirs),
                f32::from_bits(*ours)
            );
            bad += 1;
        }
    }
    assert_eq!(bad, 0, "{bad} of {} words differ from the parked {kind:?} in {}", want.len(), car_at.state);
    println!("the parked {kind:?}: {} words off {}, bit-exact", want.len(), kind.phy());
}

/// The surface tables are the bulk of the file and the easiest place for a key to slip: a file with
/// one changed coefficient must come out with that one word changed.
#[test]
fn a_changed_surface_key_moves_its_word() {
    let base = "[Mass Properties]\nWeight = 2600\n[FrontLeftTire]\nNumberOfSurfaces = 2\n\
                [FrontLeftTire Surface 2]\nA2 = 4\nC3 = 0.5\n[RearRightTire]\nNumberOfSurfaces = 1\n";
    let c = Car::read(&phy::Ini::parse(base), Kind::Buggy);
    let fl = &c.tires[car::FRONT_LEFT];
    assert_eq!(fl.surfaces.len(), 2);
    assert_eq!(fl.surfaces[1].c3, F(0.5));
    assert_eq!(fl.surfaces[1].inv_a2, F(0.25));
    // A tire without the key keeps the count the tire before it read; one with it starts over.
    assert_eq!(c.tires[car::FRONT_RIGHT].number_of_surfaces, 2);
    assert_eq!(c.tires[car::REAR_LEFT].number_of_surfaces, 2);
    assert_eq!(c.tires[car::REAR_RIGHT].number_of_surfaces, 1);
    // The buggy's class reads no rear-tire steer scale.
    assert_eq!(c.animation.rear_tire_steer_scale, None);
}

/// The buggy's and the baja's files without a RAM image: where the vehicle table says, and read to
/// what they write — the buggy's five gears, the keys the truck has
/// and they lack left at the loader's defaults (the baja does write `TireWidth`).
#[test]
fn the_buggy_and_the_baja_read() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return;
        }
    };
    for (kind, gears) in [(Kind::Buggy, 5), (Kind::Baja, 4)] {
        let Some(bytes) = car_file(&kit, kind) else { return };
        let c = Car::read_bytes(&bytes, kind);
        assert_eq!(c.engine.gear_ratios.len(), gears, "{kind:?}'s gears");
        assert_eq!(c.engine.drive_train, 1, "{kind:?} drives its rear tires");
        assert!(c.engine.first.is_some() && c.engine.max_rpm.is_some(), "{kind:?}'s gearbox");
        for t in &c.tires {
            assert_eq!(t.max_tire_angle_rate, F::MAX, "{kind:?} writes no MaxTireAngleRate");
            assert!(!t.surfaces.is_empty(), "{kind:?}'s tires have surfaces");
        }
        if kind == Kind::Buggy {
            assert!(c.tires.iter().all(|t| t.width == F::ZERO), "the buggy writes no TireWidth");
        } else {
            assert_eq!(c.tires[car::FRONT_LEFT].width, phy::number("-0.4"), "the baja's TireWidth");
        }
        // Both files spell one header `s[FrontRightTire Surface 10]`, which the reader takes for
        // neither a section nor a pair: surface 10's keys land in surface 9, which keeps its own (the
        // first match wins), and surface 10 reads as every default — `1/A2` the EE's `1/0`.
        let ten = c.tires[car::FRONT_RIGHT].surfaces[9];
        assert_eq!((ten.inv_a2, ten.inv_a4, ten.c1, ten.max_braking_torque), (F::MAX, F::MAX, F::ZERO, F::ZERO));
        assert_ne!(c.tires[car::FRONT_LEFT].surfaces[9].c1, F::ZERO, "{kind:?}'s other surface 10s");
        assert_eq!(c.cofg_ai, c.mass_properties.cofg, "{kind:?} writes no CofGAI");
        assert_eq!(c.joystick.rear_steer[1], [F(phy::MPH), F::ONE], "{kind:?}'s rear steer defaults");
        assert_eq!(c.animation.rear_tire_steer_scale, None);
    }
}
