//! S221 — the Kiowa's `.phy` reader against the original's: every word the helicopter's reader writes,
//! read back out of S148's live helicopter (`context/states/s148-kiowa.ee.bin`, free09 with the
//! helicopter switched into; the recipe is in research/journal/*-s148-kiowa), against the port's parse
//! of `helicopter\kiowa.phy` and `helicopter\wreckblade.phy` off the disc. No interpreter: the image
//! is the original's answer, and every comparison names its key.

use game_sim::archive::{Psr, Res};
use game_sim::kiowa::{KIOWA_PHY, Phy, WRECK_BLADE_PHY, wing_chord, wreck_blade};
use game_sim::phy::Part;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

const STATE: &str = "s148-kiowa.ee.bin";
const HELI: u32 = 0x007c_cae0;
/// The flown flag, and the word that says the state is the one: `MaxAltitude` (550) in place.
const FLOWN: u32 = 0x364;

/// The helicopter's layout: the rigid-body part, the pointers to what it built, and its own fields.
mod at {
    pub const PART: u32 = 0x50;
    pub const ROTOR: u32 = 0x450;
    pub const AERO: u32 = 0x454;
    pub const WRECK_ROTOR: u32 = 0x458;
    pub const BLADES: [u32; 4] = [0xe48, 0xe4c, 0xe50, 0xe54];
    pub const PELVIS_BOX: u32 = 0xe60;
}
/// A rigid-body part's own pointers to its body and collision record.
const PART_BODY: u32 = 0x110;
const PART_COLLISION: u32 = 0x120;
/// The rotor's wreck probe: its ray count and its rays (from, to; 0x20 apart).
const PROBE_COUNT: u32 = 0x168;
const PROBE_RAYS: u32 = 0x170;

/// A file out of `machine.res` in MX.PSR.
fn machine_file(kit: &Kit, name: &str) -> Option<Vec<u8>> {
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
    let e = res
        .entries
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("machine.res has no {name}"));
    Some(res.read(&bytes, e).expect("its bytes"))
}

/// The words a part's reader leaves, as the car's proof reads them.
fn part_words(want: &mut Vec<(String, u32, u32)>, ee: &Ee, what: &str, part: u32, m: &Part) {
    let body = ee.u32_at(part + PART_BODY);
    let coll = ee.u32_at(part + PART_COLLISION);
    assert_ne!(body, 0, "{what}: no body");
    assert_ne!(coll, 0, "{what}: no collision record");
    let mut f1 = |key: &str, at: u32, v: F| want.push((format!("{what} {key}"), at, v.to_bits()));
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
    f1("HighSpeedRaySpeedScale", coll + 0xc0, m.high_speed_ray_speed_scale);
    f1("HighSpeedUnpenetrationDistance", coll + 0xc4, m.high_speed_unpenetration_distance);
}

fn quad(want: &mut Vec<(String, u32, u32)>, what: &str, at: u32, v: Vf) {
    for (k, (lane, bits)) in "xyzw".chars().zip(v.to_bits()).enumerate() {
        want.push((format!("{what}.{lane}"), at + 4 * k as u32, bits));
    }
}

/// Every word of the Kiowa's file as its reader left it in the live helicopter, against
/// [`Phy::read`]: the part, both rotors, the aerodynamics, the controller, the wrecks, the pelvis box,
/// the rotor's eight wreck rays, and the four thrown blades' `wreckblade.phy`.
#[test]
fn kiowa_phy_matches_the_live_helicopter() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return;
        }
    };
    let path = kit.ctx("states").join(STATE);
    let ee = match Ee::from_ram(&path) {
        Ok(ee) => ee,
        Err(why) => {
            skip(&format!("{why} — S148's Kiowa state (only in s3's context; recipe in research/journal/*-s148-kiowa)"));
            return;
        }
    };
    assert_eq!(ee.u32_at(HELI + FLOWN), 1, "no flown helicopter at {HELI:#x} in {}", path.display());
    let (Some(bytes), Some(blade_bytes)) = (machine_file(&kit, KIOWA_PHY), machine_file(&kit, WRECK_BLADE_PHY)) else {
        return;
    };
    let p = Phy::read_bytes(&bytes);
    let blade = wreck_blade(&blade_bytes);

    let ptr = |o: u32| {
        let q = ee.u32_at(HELI + o);
        assert_ne!(q, 0, "nothing at heli+{o:#x}");
        q
    };
    let (rotor, aero) = (ptr(at::ROTOR), ptr(at::AERO));
    let mut want: Vec<(String, u32, u32)> = Vec::new();

    // [Mass Properties]
    part_words(&mut want, &ee, "[Mass Properties]", HELI + at::PART, &p.mass_properties);

    // [Main Rotor] and [Tail Rotor]
    let mut f1 = |what: &str, at: u32, v: F| want.push((what.to_string(), at, v.to_bits()));
    let r = &p.rotor;
    f1("MaxLateralCyclicPitchAngle", rotor + 0x48, r.max_lateral_cyclic);
    f1("MaxLongitudinalCyclicPitchAngle", rotor + 0x4c, r.max_longitudinal_cyclic);
    f1("MaxCollectivePitchAngle", rotor + 0x50, r.max_collective);
    f1("[Tail Rotor] MaxPitchAngle", rotor + 0x54, r.max_tail_pitch);
    f1("LiftCurveSlope", rotor + 0x58, r.lift_curve_slope);
    f1("BladeChord", rotor + 0x5c, r.blade_chord);
    f1("NumberOfBlades", rotor + 0x60, r.blades);
    f1("Radius", rotor + 0x64, r.radius);
    f1("Radius, squared", rotor + 0x68, r.radius_squared);
    f1("AngularRate", rotor + 0x6c, r.angular_rate);
    for (k, m) in r.max_moment.iter().enumerate() {
        f1(&format!("MaxRotorMoment[{k}]"), rotor + 0xc0 + 4 * k as u32, *m);
    }
    let m = &p.main_rotor;
    f1("Inertia", rotor + 0x70, m.inertia);
    f1("MinAltitude", rotor + 0x30, m.min_altitude);
    f1("MaxAltitude", rotor + 0x34, m.max_altitude);
    let t = &p.tail_rotor;
    f1("[Tail Rotor] LiftCurveSlope", rotor + 0x90, t.lift_curve_slope);
    f1("[Tail Rotor] BladeChord", rotor + 0x94, t.blade_chord);
    f1("[Tail Rotor] NumberOfBlades", rotor + 0x98, t.blades);
    f1("[Tail Rotor] Radius", rotor + 0x9c, t.radius);
    f1("[Tail Rotor] Radius, squared", rotor + 0xa0, t.radius_squared);
    f1("[Tail Rotor] GearRatio", rotor + 0xa4, t.gear_ratio);
    f1("[Tail Rotor] Inertia", rotor + 0xa8, t.inertia);

    // [Aerodynamics]
    let a = &p.aero;
    let k = &p.aero_keys;
    for (what, o, v) in [
        ("the model's density", 0x30, a.density),
        ("Chord", 0x34, a.chord),
        ("Span", 0x38, a.span),
        ("SurfaceArea", 0x3c, a.area),
        ("AspectRatio", 0x40, k.aspect_ratio),
        ("DefaultAngleOfAttack", 0x44, k.default_angle_of_attack),
        ("CLalpha", 0x48, a.cl_alpha),
        ("CLRoundOff", 0x4c, k.cl_round_off),
        ("CLDropOff", 0x50, k.cl_drop_off),
        ("CLScale", 0x54, k.cl_scale),
        ("CLAngleScale", 0x58, k.cl_angle_scale),
        ("CLq", 0x5c, k.cl_q),
        ("CD0", 0x60, a.cd0),
        ("CD1", 0x64, k.cd1),
        ("CYbeta", 0x68, a.cy_beta),
        ("CYp", 0x6c, k.cy_p),
        ("CYr", 0x70, k.cy_r),
        ("Clbeta", 0x74, a.cl_beta),
        ("Clp", 0x78, a.cl_p),
        ("Clr", 0x7c, k.cl_r),
        ("Clda", 0x80, a.cl_da),
        ("CM0", 0x84, a.cm0),
        ("CMalpha", 0x88, a.cm_alpha),
        ("CMq", 0x8c, a.cm_q),
        ("CMde", 0x90, a.cm_de),
        ("Cnbeta", 0x94, a.cn_beta),
        ("Cnp", 0x98, k.cn_p),
        ("Cnr", 0x9c, a.cn_r),
        ("Cndr", 0xa0, a.cn_dr),
        ("CYbetaPedal", 0xa4, k.cy_beta_pedal),
        ("CnbetaPedal", 0xa8, k.cn_beta_pedal),
        ("CnrPedal", 0xac, k.cn_r_pedal),
        ("TurbulenceMagnitude, in ft/s", 0xb0, k.turbulence_magnitude),
        ("TurbulenceTimeScale", 0xb4, k.turbulence_time_scale),
        ("DragHighSpeed, in ft/s", 0xf0, k.drag_high_speed),
        ("LowSpeedDrag", 0xf4, a.low_speed_drag),
        ("HighSpeedDrag", 0xf8, k.high_speed_drag),
        ("LowSpeedAero, in ft/s", 0xfc, a.low_speed_aero),
        ("HighSpeedAero, in ft/s", 0x100, a.high_speed_aero),
    ] {
        f1(what, aero + o, v);
    }

    // [Flight Controller]
    let c = &p.controller;
    for (what, o, v) in [
        ("LateralCyclicGain", 0x368, c.lateral_cyclic_gain),
        ("LongitudinalCyclicGain", 0x36c, c.longitudinal_cyclic_gain),
        ("PedalGain", 0x370, c.pedal_gain),
        ("[Flight Controller] MaxAltitude", 0x374, c.max_altitude),
        ("the constructor's hold delay", 0x380, c.hold_delay),
        ("CollectiveGain", 0x384, c.collective_gain),
        ("ClimbRateGain", 0x388, c.climb_rate),
        ("DescentRateGain", 0x38c, c.descent_rate),
        ("AltitudeHoldGain", 0x390, c.altitude_hold_gain),
        ("CollectiveIntegratorGain", 0x398, c.integrator_gain),
        ("MinCollective", 0x39c, c.min_collective),
        ("MaxPitchCommand, in radians", 0x3a0, c.max_pitch),
        ("MaxRollCommand, in radians", 0x3a4, c.max_roll),
        ("PitchGain", 0x3a8, c.pitch_gain),
        ("RollGain", 0x3ac, c.roll_gain),
        ("SpeedCommandRate, in ft/s", 0x3b4, c.speed_command_rate),
        ("MinSpeedCommand, in ft/s", 0x3b8, c.min_speed_command),
        ("MaxSpeedCommand, in ft/s", 0x3bc, c.max_speed_command),
        ("SpeedGain", 0x3c0, c.speed_gain),
        ("MinPitchRateFeedForwardSpeed, in ft/s", 0x3c4, c.min_feed_forward_speed),
        ("MinPitchRateFeedForwardScale", 0x3c8, c.min_feed_forward_scale),
        ("MaxPitchRateFeedForwardSpeed, in ft/s", 0x3cc, c.max_feed_forward_speed),
        ("MaxPitchRateFeedForwardScale", 0x3d0, c.max_feed_forward_scale),
    ] {
        f1(what, HELI + o, v);
    }
    for k in 0..3u32 {
        f1(&format!("MinRateCommand[{k}]"), HELI + 0x430 + 4 * k, c.min_rate[k as usize]);
        f1(&format!("MaxRateCommand[{k}]"), HELI + 0x440 + 4 * k, c.max_rate[k as usize]);
    }

    // [Wrecks]
    let w = &p.wrecks;
    f1("MinWreckSpeed, in ft/s", HELI + 0x3e0, w.min_wreck_speed);
    f1("WreckTime", HELI + 0x3e8, w.wreck_time);
    f1("WreckGravity", HELI + 0x3ec, w.wreck_gravity);
    let crash = p.crash();
    f1("the crash state's MinWreckSpeed", HELI + 0x3e0, crash.min_wreck_speed);
    f1("the crash state's WreckTime", HELI + 0x3e8, crash.wreck_time);

    // The quadwords.
    quad(&mut want, "Position", rotor + 0x80, p.main_rotor.position);
    quad(&mut want, "[Tail Rotor] Position", rotor + 0xb0, p.tail_rotor.position);
    quad(&mut want, "TurbulenceTimeOffset", aero + 0xc0, k.turbulence_time_offset);
    quad(&mut want, "SteadyWind, in ft/s", aero + 0xd0, k.steady_wind);
    quad(&mut want, "WreckDamping", HELI + 0x3f0, w.wreck_damping);
    quad(&mut want, "WreckInertia", HELI + 0x400, w.wreck_inertia);
    quad(&mut want, "PelvisBoxPosition", HELI + at::PELVIS_BOX, p.pelvis_box_position);
    quad(&mut want, "PelvisBoxSize", HELI + at::PELVIS_BOX + 0x10, p.pelvis_box_size);

    // The four thrown blades, each read from wreckblade.phy's [PhysicsPart1].
    for (n, o) in at::BLADES.into_iter().enumerate() {
        part_words(&mut want, &ee, &format!("blade {n}"), ptr(o), &blade);
    }

    let mut bad = Vec::new();
    for (what, at, bits) in &want {
        let got = ee.u32_at(*at);
        if got != *bits {
            bad.push(format!(
                "{what} at {at:#010x}: the game has {:e} ({got:#010x}), the port {:e} ({bits:#010x})",
                f32::from_bits(got),
                f32::from_bits(*bits)
            ));
        }
    }
    assert!(bad.is_empty(), "{} of {} words differ:\n{}", bad.len(), want.len(), bad.join("\n"));

    // The rotor's wreck rays: the probe keeps them in its tree's order, so as a set.
    let probe = ptr(at::WRECK_ROTOR);
    assert_eq!(ee.u32_at(probe + PROBE_COUNT), 8, "the rotor's probe has not eight rays");
    let rays = ee.u32_at(probe + PROBE_RAYS);
    let mut game: Vec<[u32; 8]> =
        (0..8).map(|k| std::array::from_fn(|i| ee.u32_at(rays + 0x20 * k + 4 * i as u32))).collect();
    let mut port: Vec<[u32; 8]> = p
        .wreck_rotor
        .rays
        .iter()
        .map(|[from, to]| {
            let (a, b) = (from.to_bits(), to.to_bits());
            [a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3]]
        })
        .collect();
    game.sort();
    port.sort();
    assert_eq!(game, port, "the rotor's wreck rays differ (bits, sorted)");
}

/// The aerodynamics' wing setter, which the file never sends down its one branch (its chord is not
/// zero): 10,000 generated chords, spans and aspect ratios through the original's setter on the live
/// helicopter's aerodynamics model, against [`wing_chord`]. The chord is often exactly ±0 and the
/// ratio often ±0 or negative.
#[test]
fn the_wing_setter_matches_the_original() {
    let Ok(kit) = Kit::load() else {
        skip("no kit.toml");
        return;
    };
    let Ok(mut ee) = Ee::from_ram(&kit.ctx("states").join(STATE)) else {
        skip("S148's Kiowa state is not here (only in s3's context)");
        return;
    };
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let func = addr("kiowa_wing_geometry").unwrap();
    let aero = ee.u32_at(HELI + at::AERO);
    let mut seed = 0x5221_0000_beef_0001u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut derived = 0;
    for case in 0..10_000 {
        let mut pick = |zeroes: bool| {
            let r = next();
            let v = ((r >> 40) as f32 / (1u64 << 24) as f32) * 80.0 - 20.0;
            match r % 6 {
                0 if zeroes => 0.0,
                1 if zeroes => -0.0,
                _ => v,
            }
        };
        let (chord, span, area, ratio, aoa) = (pick(true), pick(false), pick(false), pick(true), pick(false));
        ee.call(func, &[u64::from(aero)], &[chord, span, area, ratio, aoa], 1_000);
        let want = ee.f32_at(aero + 0x34);
        let got = wing_chord(F(chord), F(span), F(ratio));
        assert_eq!(
            got.to_bits(),
            want.to_bits(),
            "case {case}: chord {chord:e}, span {span:e}, ratio {ratio:e}: the game {want:e}, the port {:e}",
            got.to_f32()
        );
        if chord == 0.0 && ratio > 0.0 {
            derived += 1;
        }
    }
    assert!(derived > 1_000, "only {derived} cases took the span over the ratio");
}
