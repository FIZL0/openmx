// ---------------------------------------------------------------------------------------------------
// S12e — an AI rider's brain spawned: `ai::spawn` and `Racer::take`
// ---------------------------------------------------------------------------------------------------
//
// The brain spawn runs whole in the interpreter (the two constructors, the attach, the mode pick and
// the set-up), with only the allocator hooked to hand out a buffer the test filled. Every word of the
// brain is compared: the ones the port models against the port, the ones no constructor writes against
// the fill, and the few written words the port has no field for are named below. Then the spawn and
// the character reader are put together for free09's three AI riders and their tuning compared with
// what recording2's image holds.

use super::*;
use super::sim::S12E_RIDERS;
use game_sim::ai::{Racer, Spawned, Wander, spawn};
use ps2_oracle::{Ee, Kit};

const SP_BRAIN: u32 = 0x0190_0000;
const SP_IFACE: u32 = 0x0191_0000;
const SP_APP: u32 = 0x0192_0000;
const SP_SETTINGS: u32 = 0x0192_1000;
const SP_EVENT: u32 = 0x0192_2000;
const SP_LEN: u32 = 0x400;

/// Written words the port has no field for: the base object's (its type, count, listener list and
/// bits, below 0x30), the interface pointer (compared on its own), the card and two words after it,
/// the third steer scale, the ignore list, a half at +0x20c, the spot's two other words, a 100 at
/// +0x258, the feeler bundle, the 0.8 the set-up zeroes at +0x2b8, two 0.2s and a 1 after the drop
/// limit, the assist quad's y and w, and the motocross constructor's scenery numbers.
const SP_UNMODELLED: &[u32] = &[
    0x00, 0x04, 0x08, 0x0c, 0x10, 0x14, 0x18, 0x1c, 0x20, 0x24, 0x28, 0x40, 0xd4, 0xd8, 0xdc, 0x108, 0x150,
    0x154, 0x1c4, 0x1cc, 0x20c, 0x224, 0x228, 0x258, 0x2a0, 0x2b8, 0x314, 0x318, 0x31c, 0x39c, 0x3a0,
    0x3b8, 0x3bc, 0x3c0, 0x3c4, 0x3cc,
];

/// Words no constructor writes that the port models (as 0): the spin, the flight's air time, the
/// ground's height, the seeker's aim, the controller's three angles and its flat forward, and the
/// avoidance's far range.
const SP_UNWRITTEN: &[u32] = &[
    0x284, 0x70, 0x74, 0x78, 0x7c, 0x200, 0x204, 0x1b0, 0x1b4, 0x1b8, 0x1bc, 0x1f0, 0x1f4, 0x1f8, 0x270, 0x274,
    0x278, 0x27c,
];

/// The port's fresh brain as words, by offset. A word two of the port's fields share appears twice,
/// and the two must agree.
fn sp_words(s: &Spawned) -> Vec<(u32, u32, &'static str)> {
    let Spawned { racer: r, character: c, wander: w } = s;
    let mut out: Vec<(u32, u32, &'static str)> = Vec::new();
    let mut f = |at: u32, v: F, what: &'static str| out.push((at, v.0.to_bits(), what));
    let b = &r.brain;
    f(0x80, b.speed, "speed");
    f(0x80, r.senses.speed, "senses speed");
    f(0xb4, b.forward_y, "forward y");
    f(0x3f0, b.stuck, "stuck");
    f(0x390, b.climb.hold, "climb hold");
    f(0x394, b.climb.steep, "climb steep");
    f(0x398, b.climb.timer, "climb timer");
    f(0x30c, b.climb.speed, "climb speed");
    f(0x30c, r.vehicle.climb_speed, "vehicle climb speed");
    for (at, v, what) in [(0xf0, b.steer, "steer"), (0xf4, b.pitch, "pitch"), (0xe0, b.throttle, "throttle"), (0xe4, b.brake, "brake")] {
        f(at, v, what);
    }
    let k = &r.controls;
    for (at, v, what) in [(0xf0, k.steer, "controls steer"), (0xf4, k.pitch, "controls pitch"), (0xe0, k.throttle, "controls throttle"), (0xe4, k.brake, "controls brake")] {
        f(at, v, what);
    }
    f(0x1f4, k.heading, "controls heading");
    f(0x1f0, k.aim, "controls aim");
    f(0x1f8, k.roll, "controls roll");
    f(0x1e0, k.target_speed, "controls target speed");
    f(0xf8, r.bar_rest[0], "bar rest 0");
    f(0xfc, r.bar_rest[1], "bar rest 1");
    f(0x204, r.senses.ground, "ground");
    f(0x254, r.min_speed, "min speed");
    f(0x1c0, r.assist_x, "assist x");
    f(0x1c8, r.assist_z, "assist z");
    f(0x210, r.reach.base, "reach base");
    f(0x214, r.reach.max, "reach max");
    f(0x218, r.reach.gain, "reach gain");
    let t = &r.tuning;
    for (at, v, what) in [
        (0x208, t.steering_gain, "steering gain"),
        (0x21c, t.oversteer_gain, "oversteer gain"),
        (0x2d0, t.aim_gain, "aim gain"),
        (0x2b4, t.pitch_distance, "pitch distance"),
        (0x2d4, t.pitch_gain, "pitch gain"),
        (0x2d8, t.pitch_filter, "pitch filter"),
        (0x2ec, t.landing_pitch, "landing pitch"),
        (0x260, t.slow_tolerance, "slow tolerance"),
        (0x264, t.fast_tolerance, "fast tolerance"),
        (0x32c, t.cruise_base, "cruise base"),
        (0x330, t.cruise_gain, "cruise gain"),
        (0x334, t.brake_gain, "brake gain"),
        (0x100, t.steer_scale, "steer scale"),
        (0x104, t.pitch_scale, "pitch scale"),
    ] {
        f(at, v, what);
    }
    let a = &r.avoid;
    for (at, v, what) in [
        (0x280, a.near_range, "near range"),
        (0x284, a.far_range, "far range"),
        (0x3d8, a.far_radius, "far radius"),
        (0x3dc, a.far_response, "far response"),
        (0x3e0, a.far_approach_limit, "far approach limit"),
        (0x3e4, a.far_min_braking_speed, "far min braking speed"),
        (0x3e8, a.far_braking_response, "far braking response"),
        (0x3a4, a.near_separation, "near separation"),
        (0x3ac, a.near_response, "near response"),
        (0x3b0, a.near_braking_response, "near braking response"),
        (0x2f8, a.passing_speed, "passing speed"),
        (0x3b4, a.near_min_braking_speed, "near min braking speed"),
        (0x3d0, a.front_window, "front window"),
        (0x3d4, a.rear_window, "rear window"),
        (0x3a8, a.drift_match, "drift match"),
        (0x3ec, a.far_heading_cos, "far heading cos"),
        (0x288, a.edge_look_ahead, "edge look ahead"),
        (0x3c8, a.edge_push, "edge push"),
        (0x2a8, a.feeler_height, "feeler height"),
        (0x2ac, a.feeler_length, "feeler length"),
    ] {
        f(at, v, what);
    }
    let v = &r.vehicle;
    for (at, x, what) in [
        (0x300, v.probe_ahead, "probe ahead"),
        (0x304, v.drop_run, "drop run"),
        (0x308, v.climb_limit, "climb limit"),
        (0x310, v.drop_limit, "drop limit"),
        (0x328, v.turn_limit, "turn limit"),
    ] {
        f(at, x, what);
    }
    let p = &r.plan;
    f(0x158, p.cooldown, "trick cooldown");
    f(0x15c, p.since, "trick since");
    f(0x160, p.chance, "trick chance");
    f(0x19c, p.estimate, "trick estimate");
    f(0x200, p.air, "trick air");
    f(0x2b0, r.habits.low_air, "low air");
    let s = &r.seeker;
    for (at, x, what) in [
        (0x35c, s.tolerance, "tolerance"),
        (0x360, s.learnt, "learnt"),
        (0x364, s.aim_step, "aim step"),
        (0x368, s.speed_step, "speed step"),
        (0x36c, s.learn_step, "learn step"),
        (0x25c, s.lead, "lead"),
        (0x2fc, s.distance, "distance"),
        (0x1e0, s.speed, "seeker speed"),
    ] {
        f(at, x, what);
    }
    // the character's brain side: the same words again where it shares them
    let ct = &c.tuning;
    f(0x208, ct.steering_gain, "character steering gain");
    f(0x2b4, ct.pitch_distance, "character pitch distance");
    f(0x2a8, c.avoid.feeler_height, "character feeler height");
    for (at, x, what) in [
        (0x44, c.alod.window, "alod window"),
        (0x4c, c.alod.lower_limit, "alod lower limit"),
        (0x48, c.alod.upper_limit, "alod upper limit"),
        (0x2e4, c.alod.speed, "alod speed"),
        (0x2e8, c.alod.lead_window, "alod lead window"),
        (0x17c, c.min_random_offset, "min random offset"),
        (0x180, c.max_random_offset, "max random offset"),
        (0x188, c.max_drift_size, "max drift size"),
        (0x184, c.max_drift_rate, "max drift rate"),
        (0x2f0, c.no_track_look_ahead, "no-track look ahead"),
        (0x2f4, c.no_track_min_turn, "no-track min turn"),
        (0x210, c.min_look_ahead, "min look ahead"),
        (0x214, c.max_look_ahead, "max look ahead"),
        (0x2dc, c.roll_gain, "roll gain"),
        (0x2b0, c.roll_distance, "roll distance"),
        (0x158, c.min_stunt_interval, "min stunt interval"),
        (0x160, c.stunt_probability, "stunt probability"),
    ] {
        f(at, x, what);
    }
    let Wander { draws, drift, kick, kick_time, kick_decay, kick_clock, drift_target, drift_clock, period_low, period_high, period, cruise } = *w;
    for (k, d) in draws.iter().enumerate() {
        f(0x22c + 4 * k as u32, *d, "draw");
    }
    for (at, x, what) in [
        (0x164, drift, "drift"),
        (0x168, kick, "kick"),
        (0x16c, kick_time, "kick time"),
        (0x170, kick_decay, "kick decay"),
        (0x174, kick_clock, "kick clock"),
        (0x178, drift_target, "drift target"),
        (0x18c, drift_clock, "drift clock"),
        (0x190, period_low, "period low"),
        (0x194, period_high, "period high"),
        (0x198, period, "period"),
        (0x2e0, cruise, "cruise"),
    ] {
        f(at, x, what);
    }
    let mut q = |at: u32, v: Vf, what: &'static str| {
        for (k, lane) in v.0.iter().enumerate() {
            out.push((at + 4 * k as u32, lane.to_bits(), what));
        }
    };
    let n = &r.senses;
    q(0x290, n.last_position, "last position");
    for (k, row) in n.frame.iter().enumerate() {
        q(0x90 + 0x10 * k as u32, *row, "frame");
    }
    q(0x50, n.velocity, "velocity");
    q(0x60, n.acceleration, "acceleration");
    q(0x70, n.spin, "spin");
    q(0x2c0, n.ground_normal, "ground normal");
    q(0x110, r.filter.rate, "filter rate");
    q(0x120, r.filter.jitter, "filter jitter");
    q(0x130, r.filter.out, "filter out");
    q(0x140, r.filter.gain, "filter gain");
    q(0x110, c.steering_rate_filter, "character rate filter");
    q(0x120, c.steering_jitter_filter, "character jitter filter");
    q(0x1b0, s.aim, "seeker aim");
    q(0x1d0, s.heading, "seeker heading");
    q(0x370, s.last_miss, "last miss");
    q(0x380, s.miss, "miss");
    q(0x270, k.flat_forward, "flat forward");
    let mut u = |at: u32, v: u32, what: &'static str| out.push((at, v, what));
    for (at, flags, what) in [(0x30, b.flags, "flags"), (0x30, n.flags, "senses flags"), (0x30, s.flags, "seeker flags"), (0x38, r.moves, "moves"), (0x38, n.bumps, "bumps"), (0x38, s.flags2, "seeker flags 2")] {
        u(at, flags as u32, what);
        u(at + 4, (flags >> 32) as u32, what);
    }
    u(0xd0, b.mode as u32, "mode");
    u(0x1a0, p.slot as u32, "trick slot");
    u(0x2a4, a.feelers as u32, "feelers");
    for (at, x, what) in [
        (0x340, s.set, "set"),
        (0x344, s.target, "target"),
        (0x348, s.last_target, "last target"),
        (0x34c, s.jump, "jump"),
    ] {
        assert_eq!(x, None, "a fresh seeker has no {what}");
        u(at, 0, what);
    }
    u(0x350, s.index as u32, "index");
    u(0x354, s.state as u32, "state");
    u(0x358, s.tries as u32, "tries");
    u(0x220, s.follow, "follow");
    out
}

/// The spawn against the original's over ten thousand: the generator anywhere in its chain, the mode
/// given or picked from every event kind, the interface type, and the allocation filled with zeros or
/// with a word nothing writes.
#[test]
fn the_brain_spawn_matches_the_originals() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_mx_spawn", "operator_new", "app_pointer", "rand_state"]) else { return };
    let [spawn_fn, new, app_pointer, rand_state] = at[..] else { unreachable!() };
    ee.hooks.insert(new, Box::new(|ee| ee.set(2, u64::from(SP_BRAIN))));
    ee.set_u32(app_pointer, SP_APP);
    ee.set_u32(SP_APP + 0x3d4, SP_SETTINGS);
    ee.set_u32(SP_SETTINGS + 0xc, SP_EVENT);

    let mut pick = Pick(0x512e_5ba0_0000_0001);
    let mut modes = [0u32; 3];
    for case in 0..10_000 {
        let fill: u32 = if case % 2 == 0 { 0 } else { 0x7fa5_0000 | pick.below(0x1_0000) as u32 };
        let state = pick.next();
        let event_kind = pick.below(6) as i32 - 1;
        let mode = if pick.below(3) == 0 { Some(pick.below(8) as i32) } else { None };
        let kind = pick.below(8) as i32;
        let words: Vec<u8> = (0..SP_LEN / 4).flat_map(|_| fill.to_le_bytes()).collect();
        ee.load(SP_BRAIN, &words);
        ee.load(SP_IFACE, &[0u8; 0x200]);
        ee.set_u32(SP_IFACE + 4, kind as u32);
        ee.set_u32(SP_EVENT, event_kind as u32);
        ee.write(rand_state, 8, u128::from(state));
        let arg = mode.map_or(u64::MAX, |m| m as i64 as u64);
        ee.call(spawn_fn, &[u64::from(SP_IFACE), arg], &[], BUDGET);

        let mut rng = Rng::from_state(state);
        let s = spawn(&mut rng, kind, mode, event_kind);
        let what = format!("case {case}: mode {mode:?}, event kind {event_kind}, fill {fill:#x}, state {state:#x}");
        assert_eq!(rng.state(), ee.read(rand_state, 8) as u64, "{what}: the generator ends elsewhere");
        assert_eq!(ee.u32_at(SP_IFACE), SP_BRAIN, "{what}: the interface holds its brain");
        assert_eq!(ee.u32_at(SP_BRAIN + 0x40), SP_IFACE, "{what}: the brain holds its interface");
        assert_eq!(s.racer.brain.kind, Some(ee.u32_at(SP_IFACE + 4) as i32), "{what}: kind");
        modes[match s.racer.brain.mode { 5 => 0, 2 => 1, _ => 2 }] += 1;

        let port = sp_words(&s);
        let mut wrong = Vec::new();
        for off in (0..SP_LEN).step_by(4) {
            let got = ee.u32_at(SP_BRAIN + off);
            let mine: Vec<_> = port.iter().filter(|(at, _, _)| *at == off).collect();
            if SP_UNWRITTEN.contains(&off) {
                if got != fill {
                    wrong.push(format!("+{off:#x} is written ({got:#010x})"));
                }
                for (_, v, name) in &mine {
                    if *v != 0 {
                        wrong.push(format!("+{off:#x} ({name}) is not 0 in the port"));
                    }
                }
            } else if !mine.is_empty() {
                for (_, v, name) in &mine {
                    if got != *v {
                        wrong.push(format!(
                            "+{off:#x} ({name}) is {got:#010x} ({}), the port has {v:#010x} ({})",
                            f32::from_bits(got),
                            f32::from_bits(*v)
                        ));
                    }
                }
            } else if !SP_UNMODELLED.contains(&off) && got != fill {
                wrong.push(format!("+{off:#x} is written ({got:#010x}) and the port has no field for it"));
            }
        }
        assert!(wrong.is_empty(), "{what}:\n{}", wrong.join("\n"));
    }
    println!("brain spawns: {} riding, {} mode 2, {} other", modes[0], modes[1], modes[2]);
    assert!(modes.iter().all(|&m| m > 500), "the mode pick did not reach every branch: {modes:?}");
}

/// free09's three AI riders as the spawn and the character file make them, against recording2's image:
/// each rider's skill (interface +0x104) and the event's difficulty byte are read off the image, the
/// brain is spawned on the port's interface defaults, the retail `AIChrctr.aip` read into it and the
/// race start run, and every tuning word the tick never writes is compared with the image's brain and
/// interface.
#[test]
fn free09s_ai_riders_spawn_as_the_image_holds_them() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    let path = kit.ctx("states").join("recording2.p2m2_SaveState.eeMemory.bin");
    if !path.exists() {
        return skip(&format!("{} is not there: `./kit state ram` of recording2.p2m2_SaveState.p2s", path.display()));
    }
    let Some(retail) = super::character::ch_retail() else {
        return skip("no MX.PSR (or no Bike\\250sx.res in it): the retail character file is needed");
    };
    let ini = Ini::parse_bytes(&retail);
    let ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    let Some(at) = named(&["app_pointer"]) else { return };
    let app = ee.u32_at(at[0]);
    let event = ee.u32_at(ee.u32_at(app + 0x3d4) + 0xc);
    let difficulty = ee.read(event + 0x117, 1) as u8;
    let event_kind = ee.u32_at(event) as i32;
    let f = |at: u32| ee.u32_at(at);
    for &(b, r) in &S12E_RIDERS {
        let iface = r + 0x10d0;
        assert_eq!(f(b + 0x40), iface, "brain {b:#x}'s interface");
        let skill = f(iface + 0x104) as i32;
        let mut rng = Rng::from_state(1);
        let mut s = spawn(&mut rng, f(iface + 4) as i32, None, event_kind);
        // The interface's own words the reader keeps where the file has no key are the port's
        // constructor defaults (S322's `interface_profile`), as the spawn left them.
        let mut c = s.character;
        read_character(Some(&ini), skill, difficulty, &mut rng, &mut c);
        s.character = c;
        s.racer.take(&s.character);
        // The race start (S322): the reach's gain from the rider's start words. Its draws are not
        // compared (the generator's state at the start is not in the image).
        let keeper = f(r + 0xfa0);
        let keeper_state = (keeper != 0).then(|| f(keeper + 0x64) as i32);
        s.racer.race_start(&s.character.profile, keeper_state, &mut rng);
        let what = format!("brain {b:#x} (skill {skill}, difficulty {difficulty}, event kind {event_kind})");
        assert_eq!(s.racer.brain.mode, f(b + 0xd0) as i32, "{what}: mode");
        let flags = f(b + 0x30) as u64 | (f(b + 0x34) as u64) << 32;
        assert_eq!(flags & game_sim::ai::SPAWN_FLAGS, game_sim::ai::SPAWN_FLAGS, "{what}: the spawn's flags are on");
        // the tunings: everything the spawn and the reader write that the tick reads and never writes
        // and the reach's gain the race start set (not the seeker's lead, which its own states set)
        let tunings = [
            "steering gain", "oversteer gain", "aim gain", "pitch distance", "pitch gain", "pitch filter", "landing pitch",
            "slow tolerance", "fast tolerance", "cruise base", "cruise gain", "brake gain", "steer scale", "pitch scale",
            "near range", "far range", "far radius", "far response", "far approach limit", "far min braking speed",
            "far braking response", "near separation", "near response", "near braking response", "passing speed",
            "near min braking speed", "front window", "rear window", "drift match", "far heading cos", "edge look ahead",
            "edge push", "feeler height", "feeler length", "probe ahead", "drop run", "climb limit", "drop limit",
            "turn limit", "climb speed", "climb steep", "reach base", "reach max", "reach gain", "min speed",
            "trick cooldown", "trick chance", "low air", "tolerance", "aim step", "speed step", "learn step",
            "alod window", "alod lower limit", "alod upper limit", "alod speed", "alod lead window", "min random offset",
            "max random offset", "max drift size", "max drift rate", "no-track look ahead", "no-track min turn",
            "roll gain", "kick time", "period low", "period high", "cruise", "filter rate", "filter jitter",
            "filter gain", "feelers",
        ];
        let mut seen = 0;
        let mut wrong = Vec::new();
        for (off, v, name) in sp_words(&s) {
            if tunings.contains(&name) {
                seen += 1;
                let got = f(b + off);
                if got != v {
                    wrong.push(format!("+{off:#x} ({name}) is {got:#010x} ({}), the port has {v:#010x} ({})", f32::from_bits(got), f32::from_bits(v)));
                }
            }
        }
        assert!(seen >= 76, "{what}: only {seen} tuning words compared");
        // and the rider's side: the habits' and the assist's numbers off the interface (not the gate
        // delay or the wheelie's start, which the reader draws: the generator's state at the spawn is
        // not in the image)
        let h = &s.racer.habits;
        for (off, v, name) in [
            (0x20, h.preload_per_speed, "preload per speed"),
            (0x38, h.pull_back_amount, "pull-back"),
            (0x3c, h.lag_hang_time, "lag hang time"),
            (0x4c, h.lag_chance, "lag chance"),
            (0x50, h.lag_time, "lag time"),
            (0x64, h.horse_power, "horse power"),
            (0x7c, h.brilliant_chance, "brilliant chance"),
            (0x80, h.brilliant_min, "brilliant min"),
            (0x84, h.brilliant_random, "brilliant random"),
            (0xe8, h.cross_up_chance, "cross-up chance"),
            (0xec, h.cross_up_min, "cross-up min"),
            (0xf0, h.cross_up_max, "cross-up max"),
            (0xf4, h.cross_up_height, "cross-up height"),
            (0x68, s.racer.weights.heading, "assist heading weight"),
            (0x6c, s.racer.weights.aim, "assist aim weight"),
        ] {
            if f(iface + off) != v.0.to_bits() {
                wrong.push(format!("interface +{off:#x} ({name}) is {}, the port has {}", f32::from_bits(f(iface + off)), v.0));
            }
        }
        if f(iface + 0x14c) != game_sim::ai::KEEPER_THREE_REACH_GAIN.to_bits() {
            wrong.push(format!("interface +0x14c is {}, the port's state-3 reach gain is 0.2", f32::from_bits(f(iface + 0x14c))));
        }
        assert!(wrong.is_empty(), "{what}:\n{}", wrong.join("\n"));
        let _: &Racer = &s.racer;
    }
}
