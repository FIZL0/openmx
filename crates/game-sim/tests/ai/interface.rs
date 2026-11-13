// ---------------------------------------------------------------------------------------------------
// S322 — the rider's AI side the brain spawn leaves to others: the interface's defaults
// (`ai::interface_profile`, `ai::interface_habits`) and the bike's race start (`Racer::race_start`)
// ---------------------------------------------------------------------------------------------------
//
// The interface's words are written in two places: its own constructor, and the bike's constructor
// straight after it, which gives the habits their numbers. Both run in the interpreter as one stretch
// of the bike's constructor (from the interface constructor's call to the epilogue) on a bike filled
// with zeros or with a word nothing writes; every profile and habit word the port has is compared, and
// the words nothing writes must still hold the fill. The race start's AI branch runs whole, over ten
// thousand starts, against `Racer::race_start`.

use super::*;
use game_sim::ai::{KEEPER_THREE_REACH_GAIN, Racer, habit, interface_habits, interface_profile, spawn};

const IF_BIKE: u32 = 0x0193_0000;
const IF_LEN: u32 = 0x14e0;
const IF_IFACE: u32 = IF_BIKE + 0x10d0;

/// The profile words nothing writes: the ALOD pairs and `LookAheadGain`.
const IF_UNWRITTEN: &[u32] = &[0x120, 0x124, 0x128, 0x12c, 0x130, 0x134, 0x138, 0x13c, 0x140, 0x144, 0x148];

/// The rider's own habit words on the interface, by offset, as a fresh racer has them.
fn if_habit_words(r: &Racer) -> Vec<(u32, F, &'static str)> {
    let h = &r.habits;
    vec![
        (0x14, h.preload_base, "preload base"),
        (0x1c, h.preload, "preload"),
        (0x20, h.preload_per_speed, "preload per speed"),
        (0x38, h.pull_back_amount, "pull-back"),
        (0x3c, h.lag_hang_time, "lag hang time"),
        (0x4c, h.lag_chance, "lag chance"),
        (0x50, h.lag_time, "lag time"),
        (0x54, h.lag, "lag"),
        (0x64, h.horse_power, "horse power"),
        (0x7c, h.brilliant_chance, "brilliant chance"),
        (0x80, h.brilliant_min, "brilliant min"),
        (0x84, h.brilliant_random, "brilliant random"),
        (0x88, h.brilliant, "brilliant"),
        (0x8c, h.brilliant_roll, "brilliant roll"),
        (0xa0, h.gate_delay, "gate delay"),
        (0xa4, h.gate_clock, "gate clock"),
        (0x11c, h.gate_wait, "gate wait"),
        (0xb4, h.wheelie_max, "wheelie max"),
        (0xc8, h.wheelie, "wheelie"),
        (0xd8, h.wheelie_lean, "wheelie lean"),
        (0xdc, h.cross_up_side, "cross-up side"),
        (0xe0, h.cross_up_time, "cross-up time"),
        (0xe4, h.cross_up, "cross-up clock"),
        (0xe8, h.cross_up_chance, "cross-up chance"),
        (0xec, h.cross_up_min, "cross-up min"),
        (0xf0, h.cross_up_max, "cross-up max"),
        (0xf4, h.cross_up_height, "cross-up height"),
        (0xfc, h.low_air_bar, "low-air bar"),
        (0x68, r.weights.heading, "assist heading weight"),
        (0x6c, r.weights.aim, "assist aim weight"),
    ]
}

/// The interface as the bike's constructor leaves it, against the port's defaults: the profile's words
/// (through the character slots the reader's own oracle uses), the racer's habit words after the spawn,
/// and the state-3 start's reach gain. Words nothing writes must keep the fill.
#[test]
fn the_interface_defaults_are_the_constructors() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["bike_construct_interface", "bike_construct_epilogue", "actor_class_descriptor_ctor"]) else {
        return;
    };
    let [start, end, descriptor] = at[..] else { unreachable!() };
    // The class descriptors register their names in a table that allocates (and takes the kernel's
    // thread id); none of them is on the interface's profile, so each just keeps its owner.
    ee.hooks.insert(descriptor, Box::new(|ee| {
        let (this, owner) = (ee.r(4) as u32, ee.r(7) as u32);
        ee.set_u32(this, owner);
    }));
    ee.hooks.insert(end, Box::new(|ee| ee.set(31, u64::from(ps2_oracle::DONE) | 0xffff_ffff_0000_0000)));
    let mut c = spawn(&mut Rng::from_state(1), 0, None, 2).character;
    assert_eq!(c.profile, interface_profile(), "the spawn's character starts from the interface's profile");
    let racer = spawn(&mut Rng::from_state(1), 0, None, 2).racer;
    let mut compared = 0;
    for fill in [0u32, 0x7fa5_1234, 0xffa5_4321] {
        let words: Vec<u8> = (0..IF_LEN / 4).flat_map(|_| fill.to_le_bytes()).collect();
        ee.load(IF_BIKE, &words);
        ee.set(16, u64::from(IF_BIKE));
        ee.set_f(20, 1.0);
        ee.call(start, &[], &[], BUDGET);
        let f = |off: u32| ee.u32_at(IF_IFACE + off);
        let mut wrong = Vec::new();
        for (in_brain, off, v) in super::character::ch_slots(&mut c) {
            if in_brain {
                continue;
            }
            compared += 1;
            if IF_UNWRITTEN.contains(&off) {
                if f(off) != fill {
                    wrong.push(format!("+{off:#x} is written ({:#010x})", f(off)));
                }
                if v.to_bits() != 0 {
                    wrong.push(format!("+{off:#x} is not 0 in the port"));
                }
            } else if f(off) != v.to_bits() {
                wrong.push(format!("profile +{off:#x} is {}, the port has {}", f32::from_bits(f(off)), *v));
            }
        }
        for (off, v, name) in if_habit_words(&racer) {
            compared += 1;
            if f(off) != v.0.to_bits() {
                wrong.push(format!("+{off:#x} ({name}) is {}, the port has {}", f32::from_bits(f(off)), v.0));
            }
        }
        if f(0x14c) != KEEPER_THREE_REACH_GAIN.to_bits() {
            wrong.push(format!("+0x14c is {}, the port has {KEEPER_THREE_REACH_GAIN}", f32::from_bits(f(0x14c))));
        }
        assert!(wrong.is_empty(), "fill {fill:#x}:\n{}", wrong.join("\n"));
    }
    assert!(compared >= 3 * 86, "only {compared} words compared");
}

/// A chance as a character file might give it: anywhere from below 0 to past 100, often an edge.
fn if_chance(pick: &mut Pick) -> f32 {
    match pick.below(6) {
        0 => 0.0,
        1 => 100.0,
        2 => -0.0,
        _ => pick.range(-10.0, 110.0),
    }
}

/// The race start's AI branch against the original's over ten thousand: the generator anywhere in its
/// chain; no score keeper, one in state 3, or in another; the start words, the chances and the pull-back
/// pairs drawn wide; the flag word and every word the start may write filled at random beforehand. The
/// generator, the gate's clock and wait, the reach's gain, the flag word, the wheelie's lean and clock
/// are compared bit for bit.
#[test]
fn the_race_start_matches_the_originals() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["bike_race_start", "rand_state"]) else { return };
    let [race_start, rand_state] = at[..] else { unreachable!() };
    const BIKE: u32 = 0x0194_0000;
    const CHASSIS: u32 = 0x0195_0000;
    const BRAIN: u32 = 0x0195_1000;
    const KEEPER: u32 = 0x0195_2000;
    const IFACE: u32 = BIKE + 0x10d0;
    let mut pick = Pick(0x5322_0ff0_57a2_7001);
    let mut seen = [0u32; 6]; // state 3, other, clutch, no clutch, wheelie, no wheelie
    let base = spawn(&mut Rng::from_state(1), 0, None, 2);
    for case in 0..10_000 {
        ee.load(BIKE, &[0u8; 0x1300]);
        ee.load(CHASSIS, &[0u8; 0x400]);
        ee.load(BRAIN, &[0u8; 0x400]);
        ee.set_u32(BIKE + 0x7a4, CHASSIS);
        ee.set_u32(IFACE, BRAIN);
        let keeper = match pick.below(3) {
            0 => None,
            1 => Some(3),
            _ => Some(pick.below(8) as i32 - 1),
        };
        ee.set_u32(BIKE + 0xfa0, if keeper.is_some() { KEEPER } else { 0 });
        ee.set_u32(KEEPER + 0x64, keeper.unwrap_or(0) as u32);

        let mut r = base.racer.clone();
        let mut p = base.character.profile;
        p.start_gate_delay = F(edge(&mut pick, 1.0, 5.0));
        p.look_ahead_gain = F(float(&mut pick, 10.0));
        p.clutch_start_probability = F(if_chance(&mut pick));
        p.wheelie_start_probability = F(if_chance(&mut pick));
        for pair in [&mut p.wheelie_pull_back_clutch, &mut p.wheelie_pull_back] {
            pair.min = F(float(&mut pick, 2.0));
            pair.max = if pick.chance(4) { pair.min } else { F(float(&mut pick, 2.0)) };
        }
        r.habits.gate_delay = p.start_gate_delay;
        r.moves = pick.next();
        let h = &mut r.habits;
        for v in [&mut h.gate_clock, &mut h.gate_wait, &mut h.wheelie, &mut h.wheelie_lean, &mut r.reach.gain] {
            *v = F(float(&mut pick, 50.0));
        }
        for (off, v) in [
            (0xa0, p.start_gate_delay),
            (0x148, p.look_ahead_gain),
            (0x14c, F(KEEPER_THREE_REACH_GAIN)),
            (0xa8, p.clutch_start_probability),
            (0xac, p.wheelie_start_probability),
            (0xb8, p.wheelie_pull_back_clutch.min),
            (0xbc, p.wheelie_pull_back_clutch.max),
            (0xc0, p.wheelie_pull_back.min),
            (0xc4, p.wheelie_pull_back.max),
            (0xa4, r.habits.gate_clock),
            (0x11c, r.habits.gate_wait),
            (0xc8, r.habits.wheelie),
            (0xd8, r.habits.wheelie_lean),
        ] {
            ee.set_f32(IFACE + off, v.to_f32());
        }
        ee.set_f32(BRAIN + 0x218, r.reach.gain.to_f32());
        ee.write(BRAIN + 0x38, 8, u128::from(r.moves));
        let state = pick.next();
        ee.write(rand_state, 8, u128::from(state));
        ee.call(race_start, &[u64::from(BIKE)], &[], BUDGET);

        let mut rng = Rng::from_state(state);
        r.race_start(&p, keeper, &mut rng);
        let what = format!("case {case}: keeper {keeper:?}, state {state:#x}, {p:?}");
        assert_eq!(rng.state(), ee.read(rand_state, 8) as u64, "{what}: the generator ends elsewhere");
        assert_eq!(r.moves, ee.read(BRAIN + 0x38, 8) as u64, "{what}: the flag word");
        let h = &r.habits;
        for (at, v, name) in [
            (IFACE + 0xa4, h.gate_clock, "gate clock"),
            (IFACE + 0x11c, h.gate_wait, "gate wait"),
            (IFACE + 0xc8, h.wheelie, "wheelie"),
            (IFACE + 0xd8, h.wheelie_lean, "wheelie lean"),
            (BRAIN + 0x218, r.reach.gain, "reach gain"),
        ] {
            assert_eq!(ee.u32_at(at), v.0.to_bits(), "{what}: {name} is {}, the port has {}", f32::from_bits(ee.u32_at(at)), v.0);
        }
        seen[usize::from(keeper != Some(3))] += 1;
        seen[2 + usize::from(r.moves & habit::GATE_CLUTCH == 0)] += 1;
        seen[4 + usize::from(r.moves & habit::GATE_WHEELIE == 0)] += 1;
    }
    println!("race starts: {seen:?}");
    assert!(seen.iter().all(|&n| n > 1000), "a branch was hardly taken: {seen:?}");
}
