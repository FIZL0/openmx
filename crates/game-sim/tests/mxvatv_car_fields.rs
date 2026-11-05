//! MD39b — SLUS-21104's car field map (research/oracle.txt `# MD39`), proved on MD39a's RAM images:
//! every word MX vs. ATV's car loaders write, read back out of each player car at its Desert Oasis
//! spawn, against `Car::read_mxvatv` of the car's file. The map is MX Unleashed's (tests/car_phy.rs)
//! moved block by block; each comparison names its key.
//!
//! The images are `context/mxvatv-ps2/md39a/<car>.ee.bin` (research/journal/2026-11-03-md39a-mxvatv-car-captures,
//! `research/md39a/ram.py`), the files `context/mxvatv-ps2/md39/phy`. A car without its image skips,
//! and says so.

use game_sim::car::{self, Car, MxvatvMachine};
use ps2_float::{F, Vf};
use ps2_oracle::{Kit, skip};

struct Spawned {
    name: &'static str,
    phy: &'static str,
    machine: MxvatvMachine,
    /// The player's car object; its vtable is at +4.
    obj: u32,
    vtable: u32,
}

const CARS: [Spawned; 5] = [
    Spawned { name: "buggy", phy: "buggy.phy", machine: MxvatvMachine::Buggy, obj: 0x0159_8e10, vtable: 0x003c_ec60 },
    Spawned { name: "baja", phy: "baja.phy", machine: MxvatvMachine::Baja, obj: 0x0159_8e10, vtable: 0x003c_ec60 },
    Spawned { name: "sandrail", phy: "sandrail.phy", machine: MxvatvMachine::Sandrail, obj: 0x01b1_58a0, vtable: 0x003c_ec60 },
    Spawned { name: "golf", phy: "golfcart.phy", machine: MxvatvMachine::GolfCart, obj: 0x01b0_dac0, vtable: 0x003c_f600 },
    Spawned { name: "truck", phy: "monstertruck.phy", machine: MxvatvMachine::MonsterTruck, obj: 0x0159_8e10, vtable: 0x003c_ff80 },
];

// The car object's pointers.
const BODY: u32 = 0xb8;
const ENGINE: u32 = 0x1250;
const TIRES: [u32; 4] = [0x1254, 0x1258, 0x125c, 0x1260];
const WINGS: u32 = 0x11b8;

fn u32_at(b: &[u8], a: u32) -> u32 {
    u32::from_le_bytes(b[a as usize..a as usize + 4].try_into().unwrap())
}

fn lanes(v: Vf) -> [u32; 4] {
    v.to_bits()
}

/// Every word the loaders write, as `(key, address, bits)`.
fn words(ram: &[u8], s: &Spawned, m: &car::MxvatvCar) -> Vec<(String, u32, u32)> {
    let o = s.obj;
    let c = &m.car;
    let ptr = |at: u32| {
        let p = u32_at(ram, at);
        assert!(p != 0 && (p as usize) < ram.len(), "{}: nothing at {at:#x}", s.name);
        p
    };
    let (engine, tires) = (ptr(o + ENGINE), TIRES.map(|t| ptr(o + t)));
    let mut w: Vec<(String, u32, u32)> = Vec::new();
    let mut f1 = |what: &str, at: u32, v: F| w.push((what.to_string(), at, v.to_bits()));

    // The car block.
    f1("CofGTravelForward", o + 0x1110, c.cofg_travel_forward);
    f1("CofGTravelBack", o + 0x1114, c.cofg_travel_back);
    f1("YawRateLimit", o + 0x1244, c.yaw_rate_limit);
    f1("HandBrakeYawRateLimit", o + 0x1248, c.hand_brake_yaw_rate_limit);
    f1("HandBrakeFrictionScale", o + 0x1158, c.hand_brake_friction_scale);
    f1("HandBrakeBrakeScale", o + 0x115c, c.hand_brake_brake_scale);
    f1("HandBrakeFadeTime", o + 0x1160, c.hand_brake_fade_time);
    for (k, (&t, tire)) in tires.iter().zip(&c.tires).enumerate() {
        let name = car::TIRES[k];
        let mut key = |what: &str, at: u32, v: F| f1(&format!("[{name}] {what}"), t + at, v);
        key("MaxCompression", 0x30, tire.max_compression);
        key("MaxCompressionRate", 0x34, tire.max_compression_rate);
        key("MaxDecompressionRate", 0x38, tire.max_decompression_rate);
        key("Spring, as a rate", 0x3c, tire.spring);
        key("BounceDampingRatio, as a rate", 0x40, tire.bounce_damping);
        key("ReboundDampingRatio, as a rate", 0x44, tire.rebound_damping);
        key("TireInertia, inverted", 0x48, tire.inv_inertia);
        key("GyroInertia", 0x4c, tire.gyro_inertia);
        key("MaxSteerAngle", 0x50, tire.max_steer_angle);
        key("MaxTireScale", 0x54, tire.max_tire_scale);
        key("MaxTireAngleRate", 0x94, tire.max_tire_angle_rate);
        key("MinBrakingSlip", 0x9c, tire.min_braking_slip);
        key("MaxBrakingSlip", 0xa0, tire.max_braking_slip);
        key("TireRadius", 0x1e8, tire.radius);
        key("TireWidth", 0x1ec, tire.width);
        key("TireRadius, squared", 0x1f0, tire.radius_squared);
        key("TireRadius, inverted", 0x1f4, tire.inv_radius.expect("a positive radius"));
        key("CamberRollRatio", 0x9b0, tire.camber_roll_ratio);
        if let Some(top) = c.top_wheel_speed(k) {
            key("the redline's wheel speed in first", 0x90, top);
        }
        for (i, sf) in tire.surfaces.iter().enumerate() {
            let i = 4 * i as u32;
            let mut key = |what: &str, at: u32, v: F| key(&format!("surface {} {what}", i / 4 + 1), at + i, v);
            key("C1", 0x624, sf.c1);
            key("C2", 0x664, sf.c2);
            key("C3", 0x6a4, sf.c3);
            key("C4", 0x6e4, sf.c4);
            key("A0", 0x724, sf.a0);
            key("A1", 0x764, sf.a1);
            key("A3", 0x7a4, sf.a3);
            key("A2, inverted", 0x7e4, sf.inv_a2);
            key("A4, inverted", 0x824, sf.inv_a4);
            key("MomentScale", 0x864, sf.moment_scale);
            key("CorneringStiffness", 0x8a4, sf.cornering_stiffness);
            key("MaxNormalLoad", 0x8e4, sf.max_normal_load);
            key("FrictionPeak", 0x924, sf.friction_peak);
            key("FrictionEndPoint", 0x964, sf.friction_end_point);
            key("MaxBrakingTorque", 0x5a4, sf.max_braking_torque);
            key("WheelFriction", 0x5e4, sf.wheel_friction);
        }
    }
    let e = &c.engine;
    let en = |at: u32| engine + at - 0xc;
    f1("IdleThrottle", en(0x4c), e.idle_throttle);
    f1("EngineBraking, scaled", en(0x50), e.engine_braking);
    f1("FinalGearRatio", en(0x54), e.final_gear_ratio);
    f1("TransmissionLoss, as a fraction", en(0x58), e.transmission_loss);
    f1("EngineInertia", en(0x5c), e.inertia);
    f1("EngineInertia, inverted", en(0x60), e.inv_inertia.expect("a positive inertia"));
    f1("PumpTorqueScale, scaled", en(0x64), e.pump_torque_scale);
    f1("maxK", en(0x68), e.max_k);
    f1("MinRPM", en(0x70), e.min_rpm);
    f1("MinTractionSlip", en(0x78), e.min_traction_slip);
    f1("MaxTractionSlip", en(0x7c), e.max_traction_slip);
    f1("NOXVolume", en(0x84), e.nox_volume);
    f1("NOXBurnRate", en(0x88), e.nox_burn_rate);
    f1("NOXHPBoost", en(0x8c), e.nox_hp_boost);
    let sh = &e.shift;
    f1("LowLoadRPM", en(0x90), sh.low_load_rpm);
    if let Some(h) = sh.high_load_rpm {
        f1("HighLoadRPM", en(0x94), h);
    }
    f1("LowLoadUpShiftRPM", en(0x98), sh.low_load_up_shift_rpm);
    f1("HighLoadUpShiftRPM", en(0x9c), sh.high_load_up_shift_rpm);
    f1("LowLoadDownShiftRPM", en(0xa0), sh.low_load_down_shift_rpm);
    f1("HighLoadDownShiftRPM", en(0xa4), sh.high_load_down_shift_rpm);
    f1("ShiftTime", en(0xa8), sh.shift_time);
    f1("ShiftUpDuration", en(0xb4), sh.shift_up_duration);
    f1("ShiftUpThrottle", en(0xb8), sh.shift_up_throttle);
    f1("ShiftDownDuration", en(0xbc), sh.shift_down_duration);
    f1("ShiftDownThrottle", en(0xc0), sh.shift_down_throttle);
    let first = e.first.expect("a car has gears");
    f1("first gear's ratio", en(0xe4), first.ratio);
    f1("first gear's ratio, inverted", en(0xe8), first.inv_ratio);
    f1("MaxPumpTorque", en(0xf0), e.max_pump_torque);
    f1("the redline", en(0x130), e.max_rpm.expect("an RPM table"));
    let (rpm, torque, gears) = (ptr(en(0xd4)), ptr(en(0xd8)), ptr(en(0xe0)));
    for (k, (&r, &t)) in e.rpm.iter().zip(&e.torque).enumerate() {
        f1(&format!("RPM{}", k + 1), rpm + 4 * k as u32, r);
        f1(&format!("HP{}, as a torque", k + 1), torque + 4 * k as u32, t);
    }
    for (k, &g) in e.gear_ratios.iter().enumerate() {
        f1(&format!("GearRatio{}", k + 1), gears + 4 * k as u32, g);
    }
    let a = &c.aero;
    f1("AeroSurfaceArea", o + 0x117c, a.aero_surface_area);
    f1("AirborneDamping", o + 0x1180, a.airborne_damping);
    f1("UnrollGain", o + 0x1184, a.unroll_gain);
    f1("UnrollTorque", o + 0x1188, a.unroll_torque);
    f1("UnyawGain", o + 0x118c, a.unyaw_gain);
    f1("UnyawTorque", o + 0x1190, a.unyaw_torque);
    f1("BrakeGyroTorque", o + 0x11b0, a.brake_gyro_torque);
    let wings = if a.wings.is_empty() { 0 } else { ptr(o + WINGS) };
    for (k, wg) in a.wings.iter().enumerate() {
        let at = wings + 0x20 * k as u32;
        f1(&format!("Wing {} LiftSlope", k + 1), at, wg.lift_slope);
        f1(&format!("Wing {} AngleOfAttack", k + 1), at + 4, wg.angle_of_attack);
        f1(&format!("Wing {} BasicDrag", k + 1), at + 8, wg.basic_drag);
        f1(&format!("Wing {} InducedDrag", k + 1), at + 0xc, wg.induced_drag);
    }
    let p = &c.preload;
    for (what, at, v) in [
        ("PreloadTime", 0xc5c, p.preload_time),
        ("MinPreloadTime", 0xc64, p.min_preload_time),
        ("MaxPreloadTime", 0xc68, p.max_preload_time),
        ("MaxPreload", 0xc6c, p.max_preload),
        ("ClutchTorqueScaleTime", 0x1170, c.clutch_torque_scale_time),
        ("MaxClutchTorqueScale", 0x1174, c.max_clutch_torque_scale),
        ("MaxClutchTorqueScaleSpeed", 0x1178, c.max_clutch_torque_scale_speed),
    ] {
        if let Some(v) = v {
            f1(what, o + at, v);
        }
    }
    // MXVehicle's [Wrecks].
    f1("MaxWreckTime", o + 0xc88, c.max_wreck_time);
    f1("WreckGravity", o + 0xc90, m.wrecks.wreck_gravity);
    let j = &c.joystick;
    f1("MaxAnalogThrottle", o + 0x1144, j.max_analog_throttle);
    f1("DigitalThrottleSpeed", o + 0x1148, j.digital_throttle_speed);
    f1("DigitalSideSlipAngle", o + 0x114c, j.digital_side_slip_angle);
    f1("BrakeRateLimit", o + 0x11d4, j.brake_rate_limit);
    f1("GasRateLimit", o + 0x11d8, j.gas_rate_limit);
    f1("XDPadRateLimitOn", o + 0x11dc, j.x_dpad_rate_limit_on);
    f1("XDPadRateLimitOff", o + 0x11e0, j.x_dpad_rate_limit_off);
    f1("XAxisRateLimit", o + 0x11e4, j.x_axis_rate_limit);
    for (t, (prefix, table)) in
        [("Steer", j.steer), ("WheelSteer", j.wheel_steer), ("DPadSteer", j.dpad_steer)].iter().enumerate()
    {
        for (k, [speed, scale]) in table.iter().enumerate() {
            let at = o + 0x11e8 + 0x18 * t as u32 + 8 * k as u32;
            f1(&format!("{prefix}Speed{}", k + 1), at, *speed);
            f1(&format!("{prefix}Scale{}", k + 1), at + 4, *scale);
        }
    }
    for (k, [speed, scale]) in j.rear_steer.iter().enumerate() {
        f1(&format!("RearSteerSpeed{}", k + 1), o + 0x1230 + 8 * k as u32, *speed);
        f1(&format!("RearSteerScale{}", k + 1), o + 0x1234 + 8 * k as u32, *scale);
    }
    if let Some(r) = c.animation.rear_tire_steer_scale {
        f1("RearTireSteerScale", o + 0x1494, r);
    }

    let mut quads: Vec<(String, u32, Vf)> = vec![
        ("CofG, as the car's".into(), o + 0x10f0, c.mass_properties.cofg),
        ("CofGAI".into(), o + 0x1100, c.cofg_ai),
        ("AirborneControlMoment".into(), o + 0x11a0, a.airborne_control_moment),
        ("WreckDamping".into(), o + 0xca0, m.wrecks.wreck_damping),
        ("WreckInertia".into(), o + 0xcb0, m.wrecks.wreck_inertia),
        ("Left1stPerShoulderOffset".into(), o + 0x980, m.camera.left_shoulder_offset),
        ("Right1stPerShoulderOffset".into(), o + 0x990, m.camera.right_shoulder_offset),
        ("FPCameraOffset".into(), o + 0xda0, m.camera.fp_camera_offset),
    ];
    if s.machine == MxvatvMachine::MonsterTruck {
        quads.push(("PelvisBoxPosition".into(), o + 0x14c0, c.animation.pelvis_box_position));
        quads.push(("PelvisBoxSize".into(), o + 0x14d0, c.animation.pelvis_box_size));
    }
    for (k, wg) in a.wings.iter().enumerate() {
        quads.push((format!("Wing {} Position", k + 1), wings + 0x20 * k as u32 + 0x10, wg.position));
    }
    for (what, at, v) in quads {
        for (k, lane) in lanes(v).into_iter().enumerate() {
            w.push((format!("{what}.{}", ["x", "y", "z", "w"][k]), at + 4 * k as u32, lane));
        }
    }

    let mut ints: Vec<(String, u32, u32)> = vec![
        ("TCSEnabled".into(), en(0x74), e.tcs_enabled as u32),
        ("NumberOfRPMPoints".into(), en(0xd0), e.rpm.len() as u32),
        ("NumberOfGears".into(), en(0xdc), e.gear_ratios.len() as u32),
        ("NumberOfWings".into(), o + 0x11b4, a.number_of_wings as u32),
        ("SquareSteeringInput".into(), o + 0x11c0, j.square_steering_input as u32),
    ];
    for (k, slot) in e.driven.iter().enumerate() {
        ints.push((format!("drive slot {k}"), en(0x30) + 4 * k as u32, slot.map_or(0, |t| tires[t])));
    }
    for (k, (t, tire)) in tires.iter().zip(&c.tires).enumerate() {
        ints.push((format!("[{}] ABSEnabled", car::TIRES[k]), t + 0x98, tire.abs_enabled as u32));
    }
    // FCamFileName: the 0x20-byte buffer, its name and terminator.
    let name = m.camera.camera_file.as_bytes();
    for k in 0..=name.len() {
        ints.push((format!("FCamFileName[{k}]"), o + 0xdb0 + k as u32, u32::from(name.get(k).copied().unwrap_or(0))));
    }
    w.extend(ints);
    w
}

/// Every word of each spawned car's loaders, at the map's address, bit for bit. A byte of the camera
/// name is compared as a byte.
#[test]
fn the_spawned_cars_words_are_the_loaders() {
    let Ok(kit) = Kit::load().map_err(|w| skip(&w)) else { return };
    let phy = kit.ctx("mxvatv-ps2/md39/phy");
    let mut proved = 0;
    for s in &CARS {
        let image = kit.ctx("mxvatv-ps2/md39a").join(format!("{}.ee.bin", s.name));
        let (Ok(ram), Ok(file)) = (std::fs::read(&image), std::fs::read(phy.join(s.phy))) else {
            skip(&format!(
                "{}: {} or its file is missing — research/journal/2026-11-03-md39a-mxvatv-car-captures makes it",
                s.name,
                image.display()
            ));
            continue;
        };
        assert_eq!(u32_at(&ram, s.obj + 4), s.vtable, "{}: the object's vtable", s.name);
        let m = Car::read_mxvatv_bytes(&file, s.machine);
        let want = words(&ram, s, &m);
        let mut bad = 0;
        for (what, at, ours) in &want {
            let theirs = if what.starts_with("FCamFileName") {
                u32::from(ram[*at as usize])
            } else {
                u32_at(&ram, *at)
            };
            if theirs != *ours {
                println!(
                    "{}: {what} at {at:#x}: the original's {theirs:#010x} ({}) ours {ours:#010x} ({})",
                    s.name,
                    f32::from_bits(theirs),
                    f32::from_bits(*ours)
                );
                bad += 1;
            }
        }
        assert_eq!(bad, 0, "{}: {bad} of {} words differ", s.name, want.len());
        println!("{}: {} words, bit-exact", s.name, want.len());
        proved += 1;
    }
    println!("{proved} of {} cars proved", CARS.len());
}
