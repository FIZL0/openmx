// --- the character file (S283) ----------------------------------------------------------------------
//
// The whole of the original's character reader runs in the interpreter, the engine's own ini reader
// included: it opens the file through the four file calls (served from a generated text or the
// retail file), parses it, selects its sections and runs strtod and the double-to-float on every
// value. Hooked: the heap (a bump allocator), the C library's lock and memset, and sbrk. Compared: every
// word of the brain's and the rider's objects (the fields the reader writes, as the port computes
// them, and every other word untouched), the generator's state, and the file name it opened.

use super::*;

const CH_BRAIN: u32 = 0x0100_0000;
const CH_BRAIN_LEN: u32 = 0x400;
const CH_RIDER: u32 = 0x0101_0000;
const CH_RIDER_LEN: u32 = 0x200;
const CH_APP: u32 = 0x0102_0000;
const CH_SETTINGS: u32 = 0x0102_1000;
const CH_EVENT: u32 = 0x0102_2000;
const CH_HEAP: u32 = 0x0120_0000;
const CH_HEAP_END: u32 = 0x01a0_0000;
const CH_BREAK: u32 = 0x01a0_0000;
const CH_BREAK_END: u32 = 0x01f0_0000;
/// The folder the game has put in the reader's path prefix by the time it runs.
const CH_PREFIX: &str = "Rider\\";
const CH_FILE: &str = "Rider\\AIChrctr.aip";

/// The scalar keys of an `AI_<n>` section, and the two quadword ones.
const CH_KEYS: [&str; 87] = [
    "SteeringGain", "MinRandomOffset", "MaxRandomOffset", "MaxDriftSize", "MaxDesiredOffsetDriftRate",
    "NoTrackOffsetLookAhead", "NoTrackOffsetMinTurnDP", "MaxLookAheadDistance", "MinLookAheadDistance",
    "LookAheadGain", "OversteerCorrectionGain", "SpeedMinTolerance", "SpeedMaxTolerance",
    "MinNearCollisionAvoidanceResponseScale", "MaxNearCollisionAvoidanceResponseScale",
    "OverDriveNearCollisionAvoidanceResponseScale", "NearCollisionRange", "FarCollisionRange",
    "FarCollisionAvoidanceRadius", "FarCollisionAvoidanceResponseScale", "FarCollisionBrakingApproachRateLimit",
    "FarCollisionMinBrakingSpeed", "FarCollisionBrakingResponseScale", "NearCollisionSeperationRate",
    "NearCollisionBrakingResponse", "PassingSpeed", "NearCollisionMinBrakingSpeed", "FrontLateralAvoidanceWindow",
    "RearLateralAvoidanceWindow", "RollGain", "RollControlDistance", "PitchControlDistance", "PitchGain",
    "PitchJitterDeadZone", "MinimumStuntInterval", "StuntProbability", "MinPreloadSuccessProbability",
    "MaxPreloadSuccessProbability", "OverDrivePreloadSuccessProbability", "PullBackSpeed", "PullBackAmount",
    "LandingPitchAngle", "MinHangTimeForLandingLag", "MinLandingLagProbability", "MaxLandingLagProbability",
    "OverDriveLandingLagProbability", "MaxLandingLagTime", "MinHorsePowerScale", "MaxHorsePowerScale",
    "OverDriveHorsePowerScale", "OverPowerPhysicsTurnRateTweak", "OverPowerPhysicsVelocityTweak",
    "MinBrilliantMomentPercent", "MaxBrilliantMomentPercent", "OverDriveBrilliantMomentPercent",
    "BrilliantMomentMinDuration", "BrilliantMomentRandomDuration", "PreloadPercentagePerMPHDifference",
    "MinGLoadWreckScale", "MaxGLoadWreckScale", "OverDriveGLoadWreckScale", "RandomStartGateDelay",
    "ClutchStartProbability", "WheelieStartProbability", "WheelieStartMinDuration", "WheelieStartMaxDuration",
    "WheelieMinPullBackWithClutch", "WheelieMaxPullBackWithClutch", "WheelieMinPullBackWithoutClutch",
    "WheelieMaxPullBackWithoutClutch", "CrossUpProbability", "CrossUpMinDuration", "CrossUpMaxDuration",
    "CrossUpMinHeight", "MinALODWindow", "MaxALODWindow", "MinALODLowerLimit", "MaxALODLowerLimit",
    "MinALODUpperLimit", "MaxALODUpperLimit", "MinALODSpeed", "MaxALODSpeed", "MinALODLeadWindow",
    "MaxALODLeadWindow", "SteeringJitterFilter", "SteeringRateFilter", "Unused",
];

/// Every field the reader writes, with where the original keeps it: the brain (`true`) or the rider,
/// and the offset. The quadwords come as four lanes.
pub(crate) fn ch_slots(c: &mut Character) -> Vec<(bool, u32, &mut f32)> {
    let Character { tuning: t, avoid: a, alod, profile: p, .. } = c;
    let mut out: Vec<(bool, u32, &mut f32)> = vec![
        (true, 0x44, &mut alod.window.0),
        (true, 0x4c, &mut alod.lower_limit.0),
        (true, 0x48, &mut alod.upper_limit.0),
        (true, 0x2e4, &mut alod.speed.0),
        (true, 0x2e8, &mut alod.lead_window.0),
        (true, 0x208, &mut t.steering_gain.0),
        (true, 0x21c, &mut t.oversteer_gain.0),
        (true, 0x260, &mut t.slow_tolerance.0),
        (true, 0x264, &mut t.fast_tolerance.0),
        (true, 0x2b4, &mut t.pitch_distance.0),
        (true, 0x2d4, &mut t.pitch_gain.0),
        (true, 0x2d8, &mut t.pitch_filter.0),
        (true, 0x2ec, &mut t.landing_pitch.0),
        (true, 0x3ac, &mut a.near_response.0),
        (true, 0x280, &mut a.near_range.0),
        (true, 0x284, &mut a.far_range.0),
        (true, 0x3d8, &mut a.far_radius.0),
        (true, 0x3dc, &mut a.far_response.0),
        (true, 0x3e0, &mut a.far_approach_limit.0),
        (true, 0x3e4, &mut a.far_min_braking_speed.0),
        (true, 0x3e8, &mut a.far_braking_response.0),
        (true, 0x3a4, &mut a.near_separation.0),
        (true, 0x3b0, &mut a.near_braking_response.0),
        (true, 0x2f8, &mut a.passing_speed.0),
        (true, 0x3b4, &mut a.near_min_braking_speed.0),
        (true, 0x3d0, &mut a.front_window.0),
        (true, 0x3d4, &mut a.rear_window.0),
        (false, 0x120, &mut p.alod_window.min.0),
        (false, 0x124, &mut p.alod_window.max.0),
        (false, 0x128, &mut p.alod_lower_limit.min.0),
        (false, 0x12c, &mut p.alod_lower_limit.max.0),
        (false, 0x130, &mut p.alod_upper_limit.min.0),
        (false, 0x134, &mut p.alod_upper_limit.max.0),
        (false, 0x140, &mut p.alod_speed.min.0),
        (false, 0x144, &mut p.alod_speed.max.0),
        (false, 0x138, &mut p.alod_lead_window.min.0),
        (false, 0x13c, &mut p.alod_lead_window.max.0),
        (false, 0x148, &mut p.look_ahead_gain.0),
        (false, 0xcc, &mut p.near_response.min.0),
        (false, 0xd0, &mut p.near_response.max.0),
        (false, 0xd4, &mut p.near_response.overdrive.0),
        (false, 0x34, &mut p.pull_back_speed.0),
        (false, 0x38, &mut p.pull_back_amount.0),
        (false, 0x3c, &mut p.landing_lag_hang_time.0),
        (false, 0x50, &mut p.landing_lag_time.0),
        (false, 0x68, &mut p.turn_rate_tweak.0),
        (false, 0x6c, &mut p.velocity_tweak.0),
        (false, 0x80, &mut p.brilliant_min_duration.0),
        (false, 0x84, &mut p.brilliant_random_duration.0),
        (false, 0x20, &mut p.preload_per_speed.0),
        (false, 0xa0, &mut p.start_gate_delay.0),
        (false, 0xa8, &mut p.clutch_start_probability.0),
        (false, 0xac, &mut p.wheelie_start_probability.0),
        (false, 0xb0, &mut p.wheelie_start_min.0),
        (false, 0xb4, &mut p.wheelie_start_max.0),
        (false, 0xb8, &mut p.wheelie_pull_back_clutch.min.0),
        (false, 0xbc, &mut p.wheelie_pull_back_clutch.max.0),
        (false, 0xc0, &mut p.wheelie_pull_back.min.0),
        (false, 0xc4, &mut p.wheelie_pull_back.max.0),
        (false, 0xe8, &mut p.crossup_probability.0),
        (false, 0xec, &mut p.crossup_min_duration.0),
        (false, 0xf0, &mut p.crossup_max_duration.0),
        (false, 0xf4, &mut p.crossup_min_height.0),
    ];
    for (at, s) in [
        (0x24, &mut p.preload_success),
        (0x40, &mut p.landing_lag),
        (0x58, &mut p.horse_power),
        (0x70, &mut p.brilliant_moment),
        (0x90, &mut p.g_load_wreck),
    ] {
        let Spread { min, max, overdrive, value } = s;
        out.extend([(false, at, &mut min.0), (false, at + 4, &mut max.0), (false, at + 8, &mut overdrive.0), (false, at + 12, &mut value.0)]);
    }
    for (at, v) in [(0x120, &mut c.steering_jitter_filter), (0x110, &mut c.steering_rate_filter)] {
        // Four lanes of a quadword, written back as one.
        let lanes: &mut [f32; 4] = &mut v.0;
        for (k, lane) in lanes.iter_mut().enumerate() {
            out.push((true, at + 4 * k as u32, lane));
        }
    }
    let scalars: [(u32, &mut F); 12] = [
        (0x17c, &mut c.min_random_offset),
        (0x180, &mut c.max_random_offset),
        (0x188, &mut c.max_drift_size),
        (0x184, &mut c.max_drift_rate),
        (0x2f0, &mut c.no_track_look_ahead),
        (0x2f4, &mut c.no_track_min_turn),
        (0x214, &mut c.max_look_ahead),
        (0x210, &mut c.min_look_ahead),
        (0x2dc, &mut c.roll_gain),
        (0x2b0, &mut c.roll_distance),
        (0x158, &mut c.min_stunt_interval),
        (0x160, &mut c.stunt_probability),
    ];
    out.extend(scalars.into_iter().map(|(at, v)| (true, at, &mut v.0)));
    out
}

/// A number as a file might write it: plain, signed, with an exponent, with a trailing comment, or
/// spaced out (the reader squeezes the spaces away).
fn ch_number(p: &mut Pick) -> String {
    let span = [1.0, 10.0, 200.0, 5000.0][p.below(4) as usize];
    let x = float(p, span);
    match p.below(9) {
        0 => format!("{}", x as i32),
        1 => format!("{x:.1}"),
        2 => format!("{x:e}"),
        3 => format!("{x:.6} ;- a comment, 12"),
        4 => format!(" {x:.3} "),
        5 => format!("{:.2}", x.abs()),
        6 => "0.1".to_string(),
        _ => format!("{x}"),
    }
}

/// A generated character file: some of `DefaultALODSettings` and of a few `AI_<n>` sections (now and
/// then the reader's own, twice), each with a random part of the keys, in a random order and case.
fn ch_file(p: &mut Pick, skill: i32) -> String {
    let mut text = String::new();
    // No pair before the first section: the original's reader files it under section −1 and writes
    // outside its tables (a fault in the interpreter); no shipped file has one.
    let mut sections: Vec<String> = Vec::new();
    if !p.chance(5) {
        sections.push("DefaultALODSettings".into());
    }
    for _ in 0..p.below(4) {
        sections.push(format!("AI_{}", skill + p.below(5) - 2));
    }
    if !p.chance(5) {
        sections.push(format!("AI_{skill}"));
    }
    if p.chance(6) {
        sections.push(format!("ai_{skill}"));
    }
    let n = sections.len();
    for k in 0..n {
        let j = k + p.below((n - k) as u32) as usize;
        sections.swap(k, j);
    }
    for name in &sections {
        text.push_str(&format!("[{name}]\n; a comment\n\n"));
        let share = p.below(4);
        for key in CH_KEYS {
            if p.below(4) < share {
                continue;
            }
            let key = if p.chance(10) { key.to_ascii_lowercase() } else { key.to_string() };
            let value = if key.eq_ignore_ascii_case("SteeringJitterFilter") || key.eq_ignore_ascii_case("SteeringRateFilter") {
                let lanes = 3 + p.below(2);
                let sep = [",", ", ", " "][p.below(3) as usize];
                (0..lanes).map(|_| format!("{:.3}", float(p, 10.0))).collect::<Vec<_>>().join(sep)
            } else if p.chance(30) {
                String::new() // an empty value: no pair at all
            } else {
                ch_number(p)
            };
            text.push_str(&format!("{key} = {value}\n"));
        }
    }
    text
}

/// The retail character file, out of the bike archive that carries it.
pub(crate) fn ch_retail() -> Option<Vec<u8>> {
    let kit = ps2_oracle::Kit::load().ok()?;
    let mut f = std::fs::File::open(kit.ctx("iso").join("MX.PSR")).ok()?;
    use std::io::{Read, Seek, SeekFrom};
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).ok()?;
    let psr = game_sim::archive::Psr::parse(&front).ok()?;
    let e = psr.find("Bike\\250sx.res")?;
    f.seek(SeekFrom::Start(e.at)).ok()?;
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).ok()?;
    let res = game_sim::archive::Res::parse(&bytes).ok()?;
    let e = res.entries.iter().find(|e| e.name.to_ascii_lowercase().ends_with("aichrctr.aip"))?;
    res.read(&bytes, e).ok()
}

/// The heap and the library pieces the reader reaches (as tests/phy.rs): a bump allocator over every
/// allocation, `realloc` copying, `free` a no-op, `sbrk` growing a fixed break, the lock and memset.
/// The heap's state comes back so that each case can start it empty.
#[allow(clippy::type_complexity)]
fn ch_machine(ee: &mut ps2_oracle::Ee) -> Rc<RefCell<(u32, std::collections::BTreeMap<u32, u32>)>> {
    let heap = Rc::new(RefCell::new((CH_HEAP, std::collections::BTreeMap::<u32, u32>::new())));
    let take = |state: &Rc<RefCell<(u32, std::collections::BTreeMap<u32, u32>)>>, size: u32| {
        let mut s = state.borrow_mut();
        let at = s.0;
        s.0 = at + size.max(16).next_multiple_of(16);
        assert!(s.0 < CH_HEAP_END, "the heap ran out at {size} bytes");
        s.1.insert(at, size);
        at
    };
    for name in ["malloc", "new_array", "operator_new"] {
        let heap = Rc::clone(&heap);
        ee.hooks.insert(addr(name).unwrap(), Box::new(move |ee| ee.set(2, u64::from(take(&heap, ee.r(4) as u32)))));
    }
    {
        let heap = Rc::clone(&heap);
        ee.hooks.insert(
            addr("realloc").unwrap(),
            Box::new(move |ee| {
                let (old, size) = (ee.r(4) as u32, ee.r(5) as u32);
                let was = heap.borrow().1.get(&old).copied().unwrap_or(0);
                let at = take(&heap, size);
                let keep: Vec<u8> = (0..was.min(size)).map(|k| ee.u8_at(old + k)).collect();
                ee.load(at, &keep);
                ee.set(2, u64::from(at));
            }),
        );
    }
    for name in ["free", "delete_array", "operator_delete", "libc_lock", "libc_unlock"] {
        ee.hooks.insert(addr(name).unwrap(), Box::new(|ee| ee.set(2, 0)));
    }
    let brk = Rc::new(RefCell::new(CH_BREAK));
    ee.hooks.insert(
        addr("sbrk").unwrap(),
        Box::new(move |ee| {
            let incr = ee.r(5) as i32;
            let mut at = brk.borrow_mut();
            let was = *at;
            *at = was.wrapping_add(incr as u32);
            assert!((CH_BREAK..CH_BREAK_END).contains(&*at), "the C library's heap ran out");
            ee.set(2, u64::from(was));
        }),
    );
    ee.hooks.insert(
        addr("memset").unwrap(),
        Box::new(|ee| {
            let (at, byte, n) = (ee.r(4) as u32, ee.r(5) as u8, ee.r(6) as u32);
            ee.load(at, &vec![byte; n as usize]);
            ee.set(2, u64::from(at));
        }),
    );
    heap
}

/// Serve `file` (or nothing) to the four file calls, and remember the names asked for.
fn ch_serve(ee: &mut ps2_oracle::Ee, file: Option<Vec<u8>>, asked: Rc<RefCell<Vec<String>>>) {
    const HANDLE: u64 = 7;
    let size = file.as_ref().map_or(0, |f| f.len() as u64);
    let there = file.is_some();
    ee.hooks.insert(
        addr("fs_open").unwrap(),
        Box::new(move |ee| {
            let at = ee.r(5) as u32;
            let name: Vec<u8> = (0..0x100).map(|k| ee.u8_at(at + k)).take_while(|&c| c != 0).collect();
            asked.borrow_mut().push(String::from_utf8_lossy(&name).into_owned());
            ee.set(2, if there { HANDLE } else { u64::MAX });
        }),
    );
    ee.hooks.insert(addr("fs_size").unwrap(), Box::new(move |ee| ee.set(2, size)));
    let data = file.unwrap_or_default();
    ee.hooks.insert(
        addr("fs_read").unwrap(),
        Box::new(move |ee| {
            let (dst, len) = (ee.r(6) as u32, (ee.r(7) as u32 as usize).min(data.len()));
            ee.load(dst, &data[..len]);
            ee.set(2, len as u64);
        }),
    );
    ee.hooks.insert(addr("fs_close").unwrap(), Box::new(|ee| ee.set(2, 0)));
}

/// Stand the port's parse of `ini` in for the original's three lookups: the open (it has the file), the
/// section select (a miss selects nothing) and the raw text of a key, copied with its terminator as
/// the original copies it. Its number reading, the quadword split and the reader's arithmetic still
/// run.
fn ch_lookups(ee: &mut ps2_oracle::Ee, ini: Ini) {
    let ini = Rc::new(ini);
    let selected = Rc::new(RefCell::new(None::<String>));
    ee.hooks.insert(addr("ini_open").unwrap(), Box::new(|ee| ee.set(2, 1)));
    {
        let (ini, selected) = (Rc::clone(&ini), Rc::clone(&selected));
        ee.hooks.insert(
            addr("ini_section").unwrap(),
            Box::new(move |ee| {
                let at = ee.r(5) as u32;
                let name: Vec<u8> = (0..0x100).map(|k| ee.u8_at(at + k)).take_while(|&c| c != 0).collect();
                let name = String::from_utf8_lossy(&name).into_owned();
                let found = ini.section(&name).found();
                *selected.borrow_mut() = found.then_some(name);
                ee.set(2, u64::from(found));
            }),
        );
    }
    ee.hooks.insert(
        addr("ini_text").unwrap(),
        Box::new(move |ee| {
            let (key_at, dst, size) = (ee.r(5) as u32, ee.r(6) as u32, ee.r(7) as u32 as usize);
            let key: Vec<u8> = (0..0x100).map(|k| ee.u8_at(key_at + k)).take_while(|&c| c != 0).collect();
            let key = String::from_utf8_lossy(&key).into_owned();
            let value = selected.borrow().as_ref().and_then(|s| ini.section(s).text(&key).map(str::to_string));
            match value {
                Some(v) => {
                    let mut bytes: Vec<u8> = v.into_bytes();
                    bytes.truncate(size - 1);
                    bytes.push(0);
                    ee.load(dst, &bytes);
                    ee.set(2, 1);
                }
                None => ee.set(2, 0),
            }
        }),
    );
}

/// A float to start a field from: anything finite, small or large, signed or zero.
fn ch_start(p: &mut Pick) -> f32 {
    let span = [1.0, 100.0, 1.0e4][p.below(3) as usize];
    float(p, span)
}

// Ten thousand files in four quarters, which the test harness runs side by side.
#[test]
fn the_character_reader_matches_over_ten_thousand_files_1() {
    ch_run(0);
}

#[test]
fn the_character_reader_matches_over_ten_thousand_files_2() {
    ch_run(1);
}

#[test]
fn the_character_reader_matches_over_ten_thousand_files_3() {
    ch_run(2);
}

#[test]
fn the_character_reader_matches_over_ten_thousand_files_4() {
    ch_run(3);
}

/// One quarter of the character reader's ten thousand cases.
fn ch_run(quarter: u64) {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_character_read", "app_pointer", "rand_state", "fs_open", "realloc", "sbrk", "memset", "ini_open", "ini_section", "ini_text"]) else {
        return;
    };
    let [read, app_pointer, rand_state, ..] = at[..] else { unreachable!() };
    let retail = ch_retail();
    if retail.is_none() {
        skip("no MX.PSR (or no Bike\\250sx.res in it): the retail character file is left out");
    }
    let heap = ch_machine(&mut ee);
    ee.set_u32(app_pointer, CH_APP);
    ee.set_u32(CH_APP + 0x3d4, CH_SETTINGS);
    ee.set_u32(CH_SETTINGS + 0xc, CH_EVENT);
    // The reader's folder: the startup fill (path_prefix_init in research/oracle.txt) puts "Rider\" in
    // the prefix 0x150 bytes below the bike one. The fill itself starts other systems, so it is not run.
    let prefix = addr("path_prefix_bike").unwrap() - 0x150;
    let mut bytes = CH_PREFIX.as_bytes().to_vec();
    bytes.push(0);
    ee.load(prefix, &bytes);

    let mut p = Pick(0x5283_c4a7_0000_0001 ^ (quarter << 40));
    let (mut lerped, mut missing, mut fallback, mut retail_runs, mut parsed) = (0, 0, 0, 0, 0);
    for case in quarter * 2500..(quarter + 1) * 2500 {
        let difficulty: u8 = match case % 4 {
            0 => [0, 69, 70, 71, 85, 99, 100, 101, 125, 255][p.below(10) as usize],
            _ => p.below(256) as u8,
        };
        let skill = p.below(14) as i32 - 2;
        let use_retail = retail.is_some() && case % 800 == 0;
        let text: Option<Vec<u8>> = if use_retail {
            retail_runs += 1;
            retail.clone()
        } else if p.chance(50) {
            None
        } else {
            Some(ch_file(&mut p, skill).into_bytes())
        };
        let skill = if use_retail { (case / 800) as i32 } else { skill };

        // The two objects, every word random, and the fields read back into the port's start.
        let mut brain: Vec<u32> = (0..CH_BRAIN_LEN / 4).map(|_| ch_start(&mut p).to_bits()).collect();
        let mut rider: Vec<u32> = (0..CH_RIDER_LEN / 4).map(|_| ch_start(&mut p).to_bits()).collect();
        rider[0] = CH_BRAIN;
        rider[0x104 / 4] = skill as u32;
        let mut c = ch_blank();
        for (in_brain, off, v) in ch_slots(&mut c) {
            *v = f32::from_bits(if in_brain { brain[off as usize / 4] } else { rider[off as usize / 4] });
        }
        for (k, w) in brain.iter().enumerate() {
            ee.set_u32(CH_BRAIN + 4 * k as u32, *w);
        }
        for (k, w) in rider.iter().enumerate() {
            ee.set_u32(CH_RIDER + 4 * k as u32, *w);
        }
        ee.write(CH_EVENT + 0x117, 1, u128::from(difficulty));
        let state = p.next();
        ee.write(rand_state, 8, u128::from(state));
        let asked = Rc::new(RefCell::new(Vec::new()));
        // The original's own parse is slow in the interpreter (most of a second for the retail file),
        // and tests/phy.rs proves the port's reading of it; so the first 40 generated files of each quarter and the
        // retail ones run it, and the rest hand the original's lookups the port's parse.
        let real_parse = case % 2500 < 40 || use_retail || text.is_none();
        if real_parse {
            ch_serve(&mut ee, text.clone(), Rc::clone(&asked));
            for name in ["ini_open", "ini_section", "ini_text"] {
                ee.hooks.remove(&addr(name).unwrap());
            }
            parsed += 1;
        } else {
            ch_lookups(&mut ee, Ini::parse_bytes(text.as_ref().unwrap()));
        }

        *heap.borrow_mut() = (CH_HEAP, Default::default());
        ee.call(read, &[u64::from(CH_RIDER)], &[], 400_000_000);

        let ini = text.as_ref().map(|t| Ini::parse_bytes(t));
        if let Some(ini) = &ini {
            let own = ini.section(&format!("AI_{skill}")).found();
            if !own {
                fallback += 1;
            }
        } else {
            missing += 1;
        }
        let mut rng = Rng::from_state(state);
        read_character(ini.as_ref(), skill, difficulty, &mut rng, &mut c);
        if ini.is_some() && difficulty != 100 {
            lerped += 1;
        }
        for (in_brain, off, v) in ch_slots(&mut c) {
            let w = v.to_bits();
            if in_brain { brain[off as usize / 4] = w } else { rider[off as usize / 4] = w }
        }
        let what = format!("case {case}: skill {skill}, difficulty {difficulty}, state {state:#x}, retail {use_retail}");
        if real_parse {
            assert_eq!(asked.borrow().as_slice(), [CH_FILE.to_string()], "{what}: the name it opened");
        }
        assert_eq!(rng.state(), ee.read(rand_state, 8) as u64, "{what}: the generator ends elsewhere");
        for (k, want) in brain.iter().enumerate() {
            let got = ee.u32_at(CH_BRAIN + 4 * k as u32);
            if got != *want {
                let file = text.as_ref().map(|t| String::from_utf8_lossy(t).into_owned()).unwrap_or_default();
                let file = if use_retail { String::new() } else { file };
                panic!("{what}: brain +{:#x} is {got:#010x} ({}), the port has {want:#010x} ({})\n{file}", 4 * k, f32::from_bits(got), f32::from_bits(*want));
            }
        }
        for (k, want) in rider.iter().enumerate() {
            let got = ee.u32_at(CH_RIDER + 4 * k as u32);
            if got != *want {
                let file = text.as_ref().map(|t| String::from_utf8_lossy(t).into_owned()).unwrap_or_default();
                let file = if use_retail { String::new() } else { file };
                panic!("{what}: rider +{:#x} is {got:#010x} ({}), the port has {want:#010x} ({})\n{file}", 4 * k, f32::from_bits(got), f32::from_bits(*want));
            }
        }
    }
    println!("character files: {lerped} put between, {missing} missing, {fallback} without their own section, {retail_runs} retail, {parsed} parsed by the original");
    assert!(missing > 10 && fallback > 100, "the cases did not reach every branch");
}

/// A [`Character`] with every field zero, for [`ch_slots`] to fill.
fn ch_blank() -> Character {
    let z = F(0.0);
    let v = ps2_float::Vf::new(0.0, 0.0, 0.0, 0.0);
    Character {
        tuning: game_sim::ai::Tuning {
            steering_gain: z, oversteer_gain: z, aim_gain: z, pitch_distance: z, pitch_gain: z, pitch_filter: z,
            landing_pitch: z, slow_tolerance: z, fast_tolerance: z, cruise_base: z, cruise_gain: z, brake_gain: z,
            steer_scale: z, pitch_scale: z,
        },
        avoid: game_sim::ai::AvoidTuning::default(),
        alod: Default::default(),
        steering_jitter_filter: v,
        steering_rate_filter: v,
        min_random_offset: z, max_random_offset: z, max_drift_size: z, max_drift_rate: z,
        no_track_look_ahead: z, no_track_min_turn: z, min_look_ahead: z, max_look_ahead: z,
        roll_gain: z, roll_distance: z, min_stunt_interval: z, stunt_probability: z,
        profile: Profile { near_response: Triple::default(), wheelie_pull_back: Pair::default(), ..Default::default() },
    }
}
