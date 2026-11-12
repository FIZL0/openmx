// ---------------------------------------------------------------------------------------------------
// S312 — the AI tick's callouts: the base update, the stuck rescue, the hit reaction and the bike's
// countdown, the listener walk, the steering filter and the rider's habits
// ---------------------------------------------------------------------------------------------------
//
// Each is the original's own function (or, for the countdown, the stretch of the player update that is
// it) run in the interpreter against its port, over ten thousand cases. The base update's calls out are
// hooked as in the tick's test; the walk's listeners and the rescue's two answers are hooks that log
// what they were handed.

use super::*;

const TK_MODE_TWO: u32 = 0x01a3_5038;
const TK_MODE_FOUR: u32 = 0x01a3_5040;
const TK_NODE: u32 = 0x01a3_6000;
const TK_ENGINE: u32 = 0x01a3_7000;
const TK_TRACK: u32 = 0x01a3_8000;
const TK_LISTENERS: u32 = 0x01a3_9000;
const TK_LISTENER_VT: u32 = 0x01a3_a000;
const TK_TOLD: u32 = 0x01a3_a800;
const TK_APP: u32 = 0x01a3_b000;
const TK_RACE_HOLDER: u32 = 0x01a3_c000;
const TK_RACE: u32 = 0x01a3_d000;
const TK_RIDER_VT: u32 = 0x01a3_e000;
const TK_PUT_BACK: u32 = 0x01a3_e800;

/// A quadword's lane `k` of its first three.
fn lane3(v: Vf, k: usize) -> f32 {
    [v.x(), v.y(), v.z()][k]
}

/// `ai_update_base` against `ai::update_base` over ten thousand ticks: with and without an interface,
/// every mode about 2 and 4, the flagged steps on and off. The calls (in order), the answer, the flag
/// word and the fields the scripted calls write are compared bit for bit.
#[test]
fn the_base_update_matches_over_ten_thousand_ticks() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_update_base", "ai_listener_walk"]) else { return };
    let [base, walk] = at[..] else { unreachable!() };

    let script = Rc::new(RefCell::new(TickScript::default()));
    let logged = |name: &'static str, s: &Rc<RefCell<TickScript>>| {
        let s = s.clone();
        Box::new(move |_: &mut ps2_oracle::Ee| s.borrow_mut().log.push(name)) as Box<dyn FnMut(&mut ps2_oracle::Ee)>
    };
    ee.hooks.insert(TK_APPROACH, logged("approach", &script));
    ee.hooks.insert(TK_EXTRA, logged("extra", &script));
    ee.hooks.insert(TK_HAND_OFF, logged("hand-off", &script));
    let s = script.clone();
    ee.hooks.insert(
        TK_FEED,
        Box::new(move |ee| {
            let mut s = s.borrow_mut();
            s.log.push("feed");
            let (speed, forward_y, ground) = s.fed;
            ee.set_f32(TK_BRAIN + 0x80, speed);
            ee.set_f32(TK_BRAIN + 0xb4, forward_y);
            let f = ee.read(TK_BRAIN + 0x30, 8) as u64 & !0x20;
            ee.write(TK_BRAIN + 0x30, 8, u128::from(f | if ground { 0x20 } else { 0 }));
        }),
    );
    let s = script.clone();
    ee.hooks.insert(
        TK_CONTROLS,
        Box::new(move |ee| {
            let mut s = s.borrow_mut();
            s.log.push("controls");
            for (off, v) in [0xe0, 0xe4, 0xf0, 0xf4].into_iter().zip(s.controlled) {
                ee.set_f32(TK_BRAIN + off, v);
            }
        }),
    );
    for (hook, name) in [(TK_MODE_TWO, "mode 2"), (TK_MODE_FOUR, "mode 4")] {
        let s = script.clone();
        ee.hooks.insert(hook, Box::new(move |ee| {
            let mut s = s.borrow_mut();
            s.log.push(name);
            ee.set(2, u64::from(s.other));
        }));
    }
    let s = script.clone();
    ee.hooks.insert(walk, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        s.log.push("listeners");
        ee.set(2, u64::from(s.heard));
    }));
    let iface = TK_RIDER + 0x10d0;
    let null_vt = ee.u32_at(8);
    ee.set_u32(8, TK_IFACE_VT);

    let mut pick = Pick(0x5312_0ba5_e000_0001);
    let mut seen = [0u32; 6]; // no interface, mode 2, mode 4, approach, extra, true answers
    for case in 0..10_000 {
        let kind = if pick.chance(4) { None } else { Some(pick.below(8) as i32) };
        let mode = [0, 1, 2, 2, 3, 4, 4, 5, 6][pick.below(9) as usize];
        let mut flags = pick.next();
        for bit in [tick_flag::APPROACH, tick_flag::EXTRA] {
            if pick.chance(2) {
                flags &= !bit;
            }
        }
        let dt = if pick.chance(2) { 0.033_333_335 } else { pick.range(0.0, 0.1) };
        let mut brain = Brain {
            kind,
            mode,
            flags,
            speed: F(float(&mut pick, 60.0)),
            forward_y: F(float(&mut pick, 1.0)),
            stuck: F(0.0),
            climb: Climb::default(),
            steer: F(float(&mut pick, 1.0)),
            pitch: F(float(&mut pick, 1.0)),
            throttle: F(float(&mut pick, 1.0)),
            brake: F(float(&mut pick, 1.0)),
        };
        let played = TickScript {
            log: Vec::new(),
            fed: (float(&mut pick, 60.0), float(&mut pick, 1.0), pick.chance(2)),
            controlled: [float(&mut pick, 1.0), float(&mut pick, 1.0), float(&mut pick, 1.0), float(&mut pick, 1.0)],
            routed: false,
            heard: !pick.chance(4),
            other: !pick.chance(4),
        };

        ee.load(TK_RIDER, &[0u8; 0x1500]);
        ee.load(TK_BRAIN, &[0u8; 0x400]);
        ee.set_u32(TK_BRAIN + 8, TK_BRAIN_VT);
        for (slot, hook) in [(0x90, TK_CONTROLS), (0x98, TK_MODE_TWO), (0x9c, TK_MODE_FOUR), (0xa8, TK_EXTRA)] {
            ee.set_u32(TK_BRAIN_VT + slot, hook);
        }
        ee.set_u32(iface, TK_BRAIN);
        ee.set_u32(iface + 4, kind.unwrap_or(0) as u32);
        ee.set_u32(iface + 8, TK_IFACE_VT);
        for (slot, hook) in [(0x8, TK_FEED), (0xc, TK_HAND_OFF), (0x28, TK_APPROACH)] {
            ee.set_u32(TK_IFACE_VT + slot, hook);
        }
        ee.set_u32(TK_BRAIN + 0x40, if kind.is_some() { iface } else { 0 });
        ee.set_u32(TK_BRAIN + 0xd0, mode as u32);
        ee.write(TK_BRAIN + 0x30, 8, u128::from(brain.flags));
        let fields = |b: &Brain| {
            [
                (0x80, b.speed, "speed"),
                (0xb4, b.forward_y, "forward height"),
                (0xe0, b.throttle, "throttle"),
                (0xe4, b.brake, "brake"),
                (0xf0, b.steer, "steer"),
                (0xf4, b.pitch, "pitch"),
            ]
        };
        for (off, v, _) in fields(&brain) {
            ee.set_f32(TK_BRAIN + off, v.to_f32());
        }
        *script.borrow_mut() = played.clone();

        ee.call(base, &[u64::from(TK_BRAIN)], &[dt], BUDGET);

        let before = brain;
        let mut ours = Played(played);
        let ok = game_sim::ai::update_base(&mut brain, &mut ours, F(dt));
        let theirs = script.borrow();
        assert_eq!(theirs.log, ours.0.log, "case {case}: the calls, in order ({before:?})");
        assert_eq!(ee.v0(), u64::from(ok), "case {case}: the answer");
        assert_eq!(ee.read(TK_BRAIN + 0x30, 8) as u64, brain.flags, "case {case}: flags");
        for (off, v, name) in fields(&brain) {
            assert_eq!(ee.u32_at(TK_BRAIN + off), v.to_f32().to_bits(), "case {case}: {name} ({before:?})");
        }
        seen[0] += u32::from(kind.is_none());
        seen[1] += u32::from(ours.0.log.contains(&"mode 2"));
        seen[2] += u32::from(ours.0.log.contains(&"mode 4"));
        seen[3] += u32::from(ours.0.log.contains(&"approach"));
        seen[4] += u32::from(ours.0.log.contains(&"extra"));
        seen[5] += u32::from(ok);
    }
    ee.set_u32(8, null_vt);
    println!(
        "no interface {}, mode 2 {}, mode 4 {}, approach {}, extra {}, true {}",
        seen[0], seen[1], seen[2], seen[3], seen[4], seen[5]
    );
    for (k, n) in seen.iter().enumerate() {
        assert!(*n >= 200, "branch {k} reached only {n} times: {seen:?}");
    }
}

/// `ai_stuck_rescue` against `ai::rescue` over ten thousand calls: no race object, and race states about
/// 3. Which of the two the original called, with what, is compared.
#[test]
fn the_stuck_rescue_matches_over_ten_thousand_calls() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_stuck_rescue", "ride_reset_request", "app_pointer"]) else { return };
    let [rescue, reset, app_pointer] = at[..] else { unreachable!() };

    let called: Rc<RefCell<Vec<(&'static str, u64, u64, u64)>>> = Rc::default();
    for (hook, name) in [(TK_PUT_BACK, "put back"), (reset, "reset")] {
        let c = called.clone();
        ee.hooks.insert(hook, Box::new(move |ee| c.borrow_mut().push((name, ee.r(4), ee.r(5), ee.r(6)))));
    }
    let app_was = ee.u32_at(app_pointer);
    ee.set_u32(app_pointer, TK_APP);
    ee.set_u32(TK_APP + 0x3cc, TK_RACE_HOLDER);
    ee.set_u32(TK_RIDER + 0x48, TK_RIDER_VT);
    ee.set_u32(TK_RIDER_VT + 0x138, TK_PUT_BACK);

    let mut pick = Pick(0x5312_7e5c_0e00_0002);
    let mut seen = [0u32; 3]; // no race, put back in state 3, reset
    for case in 0..10_000 {
        let state = match pick.below(4) {
            0 => None,
            1 => Some(3),
            2 => Some(pick.below(8) as i32),
            _ => Some(pick.next() as i32),
        };
        ee.set_u32(TK_RACE_HOLDER + 0x74, if state.is_some() { TK_RACE } else { 0 });
        ee.set_u32(TK_RACE + 0x34, state.unwrap_or(0) as u32);
        called.borrow_mut().clear();

        ee.call(rescue, &[u64::from(TK_RIDER)], &[], BUDGET);

        let want = match game_sim::ai::rescue(state) {
            game_sim::ai::Rescue::PutBack => ("put back", 1, 1),
            game_sim::ai::Rescue::Reset => ("reset", 1, 0),
        };
        assert_eq!(*called.borrow(), [(want.0, u64::from(TK_RIDER), want.1, want.2)], "case {case}: race state {state:?}");
        seen[0] += u32::from(state.is_none());
        seen[1] += u32::from(state == Some(3));
        seen[2] += u32::from(want.0 == "reset");
    }
    ee.set_u32(app_pointer, app_was);
    println!("no race {}, state 3 {}, reset {}", seen[0], seen[1], seen[2]);
    for (k, n) in seen.iter().enumerate() {
        assert!(*n >= 1000, "branch {k} reached only {n} times: {seen:?}");
    }
}

/// `ai_hit_reaction` against `ai::hit_reaction` over ten thousand hits of every kind: the two timers and
/// the bar, the brain's flag word, the reply and the generator are compared bit for bit, and the answer.
#[test]
fn the_hit_reaction_matches_over_ten_thousand_hits() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_hit_reaction", "rand_state"]) else { return };
    let [hit_fn, rand_state] = at[..] else { unreachable!() };
    let reply_at = TK_CARD;
    let dir_at = TK_CARD + 0x10;

    let mut pick = Pick(0x5312_4170_0000_0003);
    let mut seen = [0u32; 5]; // back up, swerve only, neither, left, right
    for case in 0..10_000 {
        let kind = match pick.below(6) {
            0 => [0, 2, 4, 6, 0x10, 0x12, 0x14, 0x16, 0x1, 0x8, 0x20][pick.below(11) as usize],
            1 => 0x10 | (pick.next() & !0x6),
            2 => pick.next() & !0x16,
            _ => pick.next(),
        };
        let flags = pick.next();
        let mut recovery = Recovery {
            back_up: F(float(&mut pick, 2.0)),
            swerve: F(float(&mut pick, 2.0)),
            swerve_bar: F(float(&mut pick, 1.0)),
        };
        let mut reply = F(float(&mut pick, 200.0));
        let w = float(&mut pick, 1.0);
        let dir = if pick.chance(8) { Vf::new(0.0, 0.0, 0.0, 0.0) } else { pick.vec(10.0, w) };
        let state = pick.next();

        ee.load(TK_RIDER, &[0u8; 0x1500]);
        ee.load(TK_BRAIN, &[0u8; 0x400]);
        ee.set_u32(TK_RIDER + 0x10d0, TK_BRAIN);
        ee.write(TK_BRAIN + 0x30, 8, u128::from(flags));
        ee.set_f32(TK_RIDER + 0x14c0, recovery.back_up.to_f32());
        ee.set_f32(TK_RIDER + 0x14c4, recovery.swerve_bar.to_f32());
        ee.set_f32(TK_RIDER + 0x14c8, recovery.swerve.to_f32());
        ee.set_f32(reply_at, reply.to_f32());
        put(&mut ee, dir_at, dir);
        ee.write(rand_state, 8, u128::from(state));

        ee.call(hit_fn, &[u64::from(TK_RIDER), kind, 0, u64::from(reply_at), u64::from(dir_at)], &[], BUDGET);

        let mut ours = flags;
        let mut rng = Rng::from_state(state);
        let before = recovery;
        game_sim::ai::hit_reaction(kind, &mut ours, &mut recovery, &mut reply, &mut rng);
        let what = format!("case {case}: kind {kind:#x}, {before:?}");
        assert_eq!(ee.v0(), 1, "{what}: the answer");
        assert_eq!(ee.read(TK_BRAIN + 0x30, 8) as u64, ours, "{what}: brain flags");
        assert_eq!(ee.u32_at(TK_RIDER + 0x14c0), recovery.back_up.to_f32().to_bits(), "{what}: back-up timer");
        assert_eq!(ee.u32_at(TK_RIDER + 0x14c4), recovery.swerve_bar.to_f32().to_bits(), "{what}: swerve bar");
        assert_eq!(ee.u32_at(TK_RIDER + 0x14c8), recovery.swerve.to_f32().to_bits(), "{what}: swerve timer");
        assert_eq!(ee.u32_at(reply_at), reply.to_f32().to_bits(), "{what}: reply");
        assert_eq!(rng.state(), ee.read(rand_state, 8) as u64, "{what}: the generator");
        let back = kind & 0x6 != 0;
        let swerve = !back && kind & 0x10 != 0;
        seen[0] += u32::from(back);
        seen[1] += u32::from(swerve);
        seen[2] += u32::from(!back && !swerve);
        seen[3] += u32::from((back || swerve) && recovery.swerve_bar == F(-1.0));
        seen[4] += u32::from((back || swerve) && recovery.swerve_bar == F(1.0));
    }
    println!("back up {}, swerve {}, neither {}, left {}, right {}", seen[0], seen[1], seen[2], seen[3], seen[4]);
    for (k, n) in seen.iter().enumerate() {
        assert!(*n >= 500, "branch {k} reached only {n} times: {seen:?}");
    }
}

/// The player update's countdown of the two hit timers (a stretch of it, from the first pc to the one
/// after) against `Recovery::count_down` over ten thousand ticks, the timers about 0.
#[test]
fn the_recovery_countdown_matches_over_ten_thousand_ticks() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_recovery_countdown", "ai_recovery_countdown_end"]) else { return };
    let [start, end] = at[..] else { unreachable!() };
    ee.hooks.insert(end, Box::new(|ee| ee.set(31, ps2_oracle::DONE as i32 as i64 as u64)));

    let mut pick = Pick(0x5312_c0d0_0000_0004);
    let mut seen = [0u32; 4]; // back-up counted, held; swerve counted, held
    for case in 0..10_000 {
        let dt = if pick.chance(2) { 0.033_333_335 } else { edge(&mut pick, 0.0, 0.1) };
        let mut recovery = Recovery {
            back_up: F(edge(&mut pick, 0.0, 1.0)),
            swerve: F(edge(&mut pick, 0.0, 1.0)),
            swerve_bar: F(float(&mut pick, 1.0)),
        };
        ee.load(TK_RIDER, &[0u8; 0x1500]);
        ee.set_f32(TK_RIDER + 0x14c0, recovery.back_up.to_f32());
        ee.set_f32(TK_RIDER + 0x14c4, recovery.swerve_bar.to_f32());
        ee.set_f32(TK_RIDER + 0x14c8, recovery.swerve.to_f32());
        ee.set(18, u64::from(TK_RIDER));
        ee.set_f(20, dt);

        ee.call(start, &[], &[], BUDGET);

        let before = recovery;
        recovery.count_down(F(dt));
        let what = format!("case {case}: dt {dt}, {before:?}");
        assert_eq!(ee.u32_at(TK_RIDER + 0x14c0), recovery.back_up.to_f32().to_bits(), "{what}: back-up timer");
        assert_eq!(ee.u32_at(TK_RIDER + 0x14c4), recovery.swerve_bar.to_f32().to_bits(), "{what}: swerve bar");
        assert_eq!(ee.u32_at(TK_RIDER + 0x14c8), recovery.swerve.to_f32().to_bits(), "{what}: swerve timer");
        let counted = |a: F, b: F| a.to_bits() != b.to_bits();
        seen[0] += u32::from(counted(before.back_up, recovery.back_up));
        seen[1] += u32::from(!counted(before.back_up, recovery.back_up));
        seen[2] += u32::from(counted(before.swerve, recovery.swerve));
        seen[3] += u32::from(!counted(before.swerve, recovery.swerve));
    }
    println!("back-up counted {}, held {}; swerve counted {}, held {}", seen[0], seen[1], seen[2], seen[3]);
    for (k, n) in seen.iter().enumerate() {
        assert!(*n >= 1000, "branch {k} reached only {n} times: {seen:?}");
    }
}

/// `ai_listener_walk` against `ai::listener_walk` over ten thousand walks of up to eight listeners, each
/// with every state byte and wants word about the bits the walk reads: who was told, in order, and the
/// answer. Each listener's tick is a hook that logs the listener and the `dt` it was handed.
#[test]
fn the_listener_walk_matches_over_ten_thousand_walks() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_listener_walk"]) else { return };
    let [walk] = at[..] else { unreachable!() };
    let told: Rc<RefCell<Vec<(u32, u32)>>> = Rc::default();
    let t = told.clone();
    ee.hooks.insert(TK_TOLD, Box::new(move |ee| t.borrow_mut().push((ee.r(4) as u32, ee.f(12).to_bits()))));
    ee.set_u32(TK_LISTENER_VT + 0x38, TK_TOLD);

    let mut pick = Pick(0x5312_115e_0000_0005);
    let mut seen = [0u32; 4]; // not listening, empty, someone told, someone passed over
    for case in 0..10_000 {
        let listening = pick.next() as u32 | if pick.chance(4) { 0 } else { 2 };
        let listening = if pick.chance(4) { listening & !2 } else { listening };
        let n = pick.below(9) as usize;
        let listeners: Vec<ai::Listener> = (0..n)
            .map(|_| ai::Listener {
                state: if pick.chance(2) { 1 | (pick.below(16) as u8 & 0b10) | (pick.next() as u8 & 0xf0) } else { pick.next() as u8 },
                wants: pick.next() as u32 | if pick.chance(2) { 2 } else { 0 },
            })
            .collect();
        let dt = pick.range(0.0, 0.1);
        ee.load(TK_LISTENERS, &[0u8; 0x1000]);
        ee.set_u32(TK_BRAIN + 0x24, listening);
        ee.set_u32(TK_BRAIN + 0x18, if n > 0 { TK_LISTENERS } else { 0 });
        for (i, l) in listeners.iter().enumerate() {
            let at = TK_LISTENERS + 0x100 * i as u32;
            ee.set_u32(at + 8, TK_LISTENER_VT);
            ee.set_u32(at + 0x14, if i + 1 < n { at + 0x100 } else { 0 });
            ee.set_u32(at + 0x20, l.wants);
            ee.write(at + 0x29, 1, u128::from(l.state));
        }
        told.borrow_mut().clear();

        ee.call(walk, &[u64::from(TK_BRAIN)], &[dt], BUDGET);

        let mut ours = Vec::new();
        let ok = ai::listener_walk(listening & 2 != 0, &listeners, |i| ours.push((TK_LISTENERS + 0x100 * i as u32, dt.to_bits())));
        assert_eq!(*told.borrow(), ours, "case {case}: who was told ({listening:#x}, {listeners:?})");
        assert_eq!(ee.v0(), u64::from(ok), "case {case}: the answer");
        seen[0] += u32::from(listening & 2 == 0);
        seen[1] += u32::from(listening & 2 != 0 && n == 0);
        seen[2] += u32::from(!ours.is_empty());
        seen[3] += u32::from(listening & 2 != 0 && ours.len() < n);
    }
    println!("not listening {}, empty {}, told {}, passed over {}", seen[0], seen[1], seen[2], seen[3]);
    for (k, n) in seen.iter().enumerate() {
        assert!(*n >= 500, "branch {k} reached only {n} times: {seen:?}");
    }
}

/// The brain's steering filter (its vt+0xa8) against `ai::filter_steering` over ten thousand ticks, each
/// lane's move about the jitter and the rate either way: the filtered quadword and the controls are
/// compared bit for bit, all four lanes, and the answer.
#[test]
fn the_steering_filter_matches_over_ten_thousand_ticks() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_steer_filter"]) else { return };
    let [filter] = at[..] else { unreachable!() };

    let mut pick = Pick(0x5312_f117_0000_0006);
    let mut seen = [0u32; 6]; // up: gained, passed, clamped; down: gained, passed, clamped
    for case in 0..10_000 {
        let dt = if pick.chance(2) { 0.033_333_335 } else { pick.range(0.0, 0.1) };
        let rate = Vf::new(pick.range(0.0, 10.0), pick.range(0.0, 10.0), pick.range(0.0, 10.0), float(&mut pick, 10.0));
        let jitter = Vf::new(pick.range(0.0, 0.2), pick.range(0.0, 0.2), pick.range(0.0, 0.2), float(&mut pick, 1.0));
        let gain = Vf::new(pick.range(0.0, 1.0), pick.range(0.0, 1.0), pick.range(0.0, 1.0), float(&mut pick, 1.0));
        let zero_out = pick.chance(3);
        let w = float(&mut pick, 1.0);
        let out = if zero_out { Vf::new(0.0, 0.0, 0.0, w) } else { pick.vec(1.0, w) };
        let mut lanes = [0.0f32; 3];
        for (k, lane) in lanes.iter_mut().enumerate() {
            let step = (F(lane3(rate, k)) * F(dt)).to_f32();
            let j = lane3(jitter, k);
            let d = match pick.below(8) {
                0 => j,
                1 => step,
                2 => f32::from_bits(j.to_bits().wrapping_sub(1)),
                3 => f32::from_bits(step.to_bits().wrapping_add(1)),
                4 => 0.0,
                _ => float(&mut pick, 0.6),
            };
            let d = if pick.chance(2) { -d } else { d };
            *lane = if zero_out { d } else { (F(lane3(out, k)) + F(d)).to_f32() };
        }
        let mut wish = Vf::new(lanes[0], lanes[1], lanes[2], float(&mut pick, 1.0));

        ee.load(TK_BRAIN, &[0u8; 0x400]);
        put(&mut ee, TK_BRAIN + 0xf0, wish);
        put(&mut ee, TK_BRAIN + 0x110, rate);
        put(&mut ee, TK_BRAIN + 0x120, jitter);
        put(&mut ee, TK_BRAIN + 0x130, out);
        put(&mut ee, TK_BRAIN + 0x140, gain);

        ee.call(filter, &[u64::from(TK_BRAIN)], &[dt], BUDGET);

        let mut f = ai::SteerFilter { rate, jitter, gain, out };
        let before = wish;
        ai::filter_steering(&mut f, &mut wish, F(dt));
        let what = format!("case {case}: dt {dt}, wish {before:?}, {:?}", ai::SteerFilter { rate, jitter, gain, out });
        assert_eq!(ee.v0(), 1, "{what}: the answer");
        assert_eq!(got(&ee, TK_BRAIN + 0x130), f.out.to_bits(), "{what}: the filtered");
        assert_eq!(got(&ee, TK_BRAIN + 0xf0), wish.to_bits(), "{what}: the controls");
        for k in 0..3 {
            let d = F(lane3(before, k)) - F(lane3(out, k));
            let moved = F(lane3(f.out, k)) - F(lane3(out, k));
            let step = F(lane3(rate, k)) * F(dt);
            let up = d.to_f32() >= 0.0;
            let lane = if moved.to_bits() == step.to_bits() || moved.to_bits() == (-step).to_bits() {
                2
            } else if d.to_bits() == moved.to_bits() {
                1
            } else {
                0
            };
            seen[if up { 0 } else { 3 } + lane] += 1;
        }
    }
    println!(
        "up gained {}, passed {}, clamped {}; down gained {}, passed {}, clamped {}",
        seen[0], seen[1], seen[2], seen[3], seen[4], seen[5]
    );
    for (k, n) in seen.iter().enumerate() {
        assert!(*n >= 500, "branch {k} reached only {n} times: {seen:?}");
    }
}

/// Where each of [`ai::Habits`]' floats sits: the brain's, the rider's, its bike's, its engine's and
/// body's, its card's; the node's only when it has one.
fn habit_floats(h: &ai::Habits) -> Vec<(u32, F, &'static str)> {
    let (r, b) = (TK_RIDER, TK_BRAIN);
    let mut v = vec![
        (b + 0x360, h.start_preload, "start preload"),
        (b + 0x80, h.speed, "speed"),
        (b + 0xf0, h.steer, "steer"),
        (b + 0xf4, h.pitch, "pitch"),
        (b + 0xe0, h.throttle, "throttle"),
        (b + 0xe4, h.brake, "brake"),
        (b + 0x204, h.ground, "ground"),
        (b + 0x2b0, h.low_air, "low air"),
        (r + 0x10e4, h.preload_base, "preload base"),
        (r + 0x10f0, h.preload_per_speed, "preload per speed"),
        (r + 0x10ec, h.preload, "preload"),
        (r + 0x1108, h.pull_back_amount, "pull-back"),
        (r + 0x110c, h.lag_hang_time, "lag hang time"),
        (r + 0x111c, h.lag_chance, "lag chance"),
        (r + 0x1120, h.lag_time, "lag time"),
        (r + 0x1124, h.lag, "lag"),
        (r + 0x1134, h.horse_power, "horse power"),
        (r + 0x114c, h.brilliant_chance, "brilliant chance"),
        (r + 0x1150, h.brilliant_min, "brilliant min"),
        (r + 0x1154, h.brilliant_random, "brilliant random"),
        (r + 0x1158, h.brilliant, "brilliant"),
        (r + 0x115c, h.brilliant_roll, "brilliant roll"),
        (r + 0x1170, h.gate_delay, "gate delay"),
        (r + 0x1174, h.gate_clock, "gate clock"),
        (r + 0x11ec, h.gate_wait, "gate wait"),
        (r + 0x1184, h.wheelie_max, "wheelie max"),
        (r + 0x1198, h.wheelie, "wheelie"),
        (r + 0x11a8, h.wheelie_lean, "wheelie lean"),
        (r + 0x11ac, h.cross_up_side, "cross-up side"),
        (r + 0x11b0, h.cross_up_time, "cross-up time"),
        (r + 0x11b4, h.cross_up, "cross-up clock"),
        (r + 0x11b8, h.cross_up_chance, "cross-up chance"),
        (r + 0x11bc, h.cross_up_min, "cross-up min"),
        (r + 0x11c0, h.cross_up_max, "cross-up max"),
        (r + 0x11c4, h.cross_up_height, "cross-up height"),
        (r + 0x11cc, h.low_air_bar, "low-air bar"),
        (TK_ENGINE + 0xbc, h.power_scale, "power scale"),
        (r + 0x494, h.preload_force, "preload force"),
        (r + 0x4b0, h.preload_clock, "preload clock"),
        (r + 0x4ac, h.preload_clock_start, "preload clock start"),
        (r + 0x4c4, h.max_preload, "max preload"),
        (r + 0x644, h.clutch, "clutch"),
        (TK_BODY + 0x264, h.forward_speed, "forward speed"),
        (TK_CARD + 0xfc, h.air_time, "air time"),
        (TK_CARD + 0x1b0, h.lag_clock, "lag clock"),
    ];
    if let Some(n) = h.node {
        v.push((TK_NODE + 0x44, n.speed, "node speed"));
        v.push((TK_NODE + 0x4c, n.gain, "node gain"));
    }
    v
}

/// `ai_habits` against `ai::habits` over ten thousand ticks: riding and not, with and without a node
/// of every flag, at the start and off it, every second-word bit, on the ground, just off it and in the
/// air about the two heights, racing inside the lag and out, with and without a track (whose spot is a
/// hook that plays a scripted point). Every field, the second flag word, the preload's firing word and
/// the generator are compared bit for bit, and the answer.
#[test]
fn the_rider_habits_match_over_ten_thousand_ticks() {
    use game_sim::ai::{HabitNode, Habits, habit};
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_habits", "path_spot_point", "rand_state"]) else { return };
    let [habits_fn, spot_fn, rand_state] = at[..] else { unreachable!() };
    let spot = Rc::new(RefCell::new((Vf::new(0.0, 0.0, 0.0, 0.0), 0u32)));
    let s = spot.clone();
    ee.hooks.insert(spot_fn, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        assert_eq!((ee.r(4) as u32, ee.r(5) as u32), (TK_TRACK, TK_CARD + 0x88), "the spot's track and card");
        put(ee, ee.r(6) as u32, s.0);
        s.1 += 1;
    }));

    let mut pick = Pick(0x5312_4ab1_7500_0007);
    // preload set, clamped, start preload, brilliant on, brilliant over, pull-back, lag rolled, racing lag,
    // gate wait, fired, cross-up due, clutched, gate clutch, wheelie over, cross-up on a track, off one,
    // low-air bar, cross-up steer, cross-up over, just off
    let mut seen = [0u32; 20];
    for case in 0..10_000 {
        let dt = if pick.chance(2) { 0.033_333_335 } else { pick.range(0.0, 0.1) };
        let node = (!pick.chance(5)).then(|| HabitNode {
            flags: (pick.next() as u32 & !0x148) | [0, 8, 0x40, 0x100, 0x48, 0x140, 0x108, 0x148][pick.below(8) as usize],
            speed: F(if pick.chance(4) { -pick.range(0.0, 10.0) } else { pick.range(0.0, 80.0) }),
            gain: F(pick.range(0.0, 2.0)),
        });
        let ground = pick.range(-50.0, 50.0);
        let low_air = pick.range(0.0, 10.0);
        let cross_up_height = pick.range(0.0, 10.0);
        let height = match pick.below(4) {
            0 => edge(&mut pick, low_air, 10.0),
            1 => edge(&mut pick, cross_up_height, 10.0),
            _ => pick.range(0.0, 30.0),
        };
        let position = Vf::new(pick.range(-500.0, 500.0), (F(ground) + F(height)).to_f32(), pick.range(-500.0, 500.0), 1.0);
        let brilliant_min = pick.range(0.0, 2.0);
        let gate_delay = pick.range(0.0, 5.0);
        let gate_clock = edge(&mut pick, gate_delay, 5.0);
        let wheelie_max = pick.range(0.0, 2.0);
        let cross_up_time = pick.range(0.0, 1.0);
        let lag_hang_time = pick.range(0.0, 2.0);
        let lag_clock = pick.range(0.0, 2.0);
        let mut moves = pick.next();
        if pick.chance(3) {
            moves |= habit::PRELOAD;
            moves &= !habit::PRELOAD_SET;
        }
        let mut flags = pick.next();
        if pick.chance(3) {
            flags = (flags & !game_sim::ai::flag::ON_GROUND) | tick_flag::WAS_ON_GROUND;
        }
        let h0 = Habits {
            mode: if pick.chance(4) { 5 } else { pick.below(7) as i32 },
            node,
            start_state: if pick.chance(2) { 3 } else { pick.below(6) as i32 },
            start_preload: F(pick.range(0.0, 100.0)),
            flags,
            moves,
            speed: F(pick.range(0.0, 80.0)),
            steer: F(if pick.chance(2) { float(&mut pick, 0.2) } else { float(&mut pick, 1.0) }),
            pitch: F(edge(&mut pick, 0.9, 1.0)),
            throttle: F(float(&mut pick, 1.0)),
            brake: F(float(&mut pick, 1.0)),
            position,
            forward: if pick.chance(16) { Vf::new(0.0, 0.0, 0.0, 0.0) } else { pick.vec(1.0, 0.0) },
            ground: F(ground),
            low_air: F(low_air),
            direction: if pick.chance(16) { Vf::new(0.0, 0.0, 0.0, 0.0) } else { { let w = float(&mut pick, 1.0); pick.vec(1.0, w) } },
            preload_base: F(pick.range(-0.5, 1.5)),
            preload_per_speed: F(pick.range(-0.05, 0.05)),
            preload: F(float(&mut pick, 1.0)),
            pull_back_amount: F(float(&mut pick, 1.0)),
            lag_hang_time: F(lag_hang_time),
            lag_chance: F(pick.range(0.0, 100.0)),
            lag_time: F(pick.range(0.0, 2.0)),
            lag: F(if pick.chance(4) { -1.0 } else { edge(&mut pick, lag_clock, 2.0) }),
            horse_power: F(if pick.chance(4) { 100.0 } else { pick.range(60.0, 110.0) }),
            brilliant_chance: F(pick.range(0.0, 100.0)),
            brilliant_min: F(brilliant_min),
            brilliant_random: F(pick.range(0.0, 2.0)),
            brilliant: F(edge(&mut pick, dt, 2.0)),
            brilliant_roll: F(edge(&mut pick, 1.0, 2.0)),
            gate_delay: F(gate_delay),
            gate_clock: F(gate_clock),
            gate_wait: F(if pick.chance(2) { edge(&mut pick, gate_clock, 5.0) } else { pick.range(0.0, 1.0) }),
            wheelie_max: F(wheelie_max),
            wheelie: F(edge(&mut pick, wheelie_max - dt, 2.0)),
            wheelie_lean: F(float(&mut pick, 1.0)),
            cross_up_side: F(if pick.chance(2) { 1.0 } else { -1.0 }),
            cross_up_time: F(cross_up_time),
            cross_up: F(edge(&mut pick, cross_up_time - dt, 1.0)),
            cross_up_chance: F(pick.range(0.0, 100.0)),
            cross_up_min: F(pick.range(0.0, 0.5)),
            cross_up_max: F(pick.range(0.5, 1.0)),
            cross_up_height: F(cross_up_height),
            low_air_bar: F(pick.range(0.0, 1.0)),
            power_scale: F(pick.range(0.0, 1.0)),
            preload_force: F(float(&mut pick, 100.0)),
            preload_firing: pick.next() as i32,
            preload_clock: F(float(&mut pick, 1.0)),
            preload_clock_start: F(pick.range(0.0, 1.0)),
            max_preload: F(pick.range(0.0, 1000.0)),
            clutch: F(float(&mut pick, 1.0)),
            forward_speed: F(if pick.chance(2) { pick.range(0.0, 50.0) } else { edge(&mut pick, 0.0, 50.0) }),
            air_time: F(edge(&mut pick, lag_hang_time, 2.0)),
            racing: pick.chance(2),
            lag_clock: F(lag_clock),
            keeper_state: if pick.chance(4) { 3 } else { pick.below(6) as i32 },
        };
        let track = pick.chance(2);
        let w = float(&mut pick, 1.0);
        let point = pick.vec(500.0, w);
        let state = pick.next();

        ee.load(TK_RIDER, &[0u8; 0x1500]);
        ee.load(TK_BRAIN, &[0u8; 0x400]);
        ee.load(TK_NODE, &[0u8; 0x100]);
        ee.load(TK_CARD, &[0u8; 0x200]);
        ee.load(TK_KEEPER, &[0u8; 0x100]);
        ee.load(TK_ENGINE, &[0u8; 0x100]);
        ee.load(TK_BODY, &[0u8; 0x300]);
        ee.set_u32(TK_RIDER + 0x10d0, TK_BRAIN);
        ee.set_u32(TK_RIDER + 0x7b8, TK_ENGINE);
        ee.set_u32(TK_RIDER + 0x7a4, TK_BODY);
        ee.set_u32(TK_RIDER + 0xf9c, TK_CARD);
        ee.set_u32(TK_RIDER + 0xfa0, TK_KEEPER);
        ee.set_u32(TK_KEEPER + 0x64, h0.keeper_state as u32);
        ee.set_u32(TK_KEEPER + 0x68, if track { TK_TRACK } else { 0 });
        let card_word = (pick.next() & !0x8_0000) | if h0.racing { 0x8_0000 } else { 0 };
        ee.write(TK_CARD + 8, 8, u128::from(card_word));
        ee.set_u32(TK_BRAIN + 0xd0, h0.mode as u32);
        ee.set_u32(TK_BRAIN + 0x224, if node.is_some() { TK_NODE } else { 0 });
        if let Some(n) = node {
            ee.set_u32(TK_NODE + 4, n.flags);
        }
        ee.set_u32(TK_BRAIN + 0x354, h0.start_state as u32);
        ee.write(TK_BRAIN + 0x30, 8, u128::from(h0.flags));
        ee.write(TK_BRAIN + 0x38, 8, u128::from(h0.moves));
        put(&mut ee, TK_BRAIN + 0xc0, h0.position);
        put(&mut ee, TK_BRAIN + 0xb0, h0.forward);
        put(&mut ee, TK_BRAIN + 0x1d0, h0.direction);
        ee.set_u32(TK_RIDER + 0x4a4, h0.preload_firing as u32);
        for (at, v, _) in habit_floats(&h0) {
            ee.set_f32(at, v.to_f32());
        }
        ee.write(rand_state, 8, u128::from(state));
        *spot.borrow_mut() = (point, 0);

        ee.call(habits_fn, &[u64::from(TK_RIDER)], &[dt], BUDGET);

        let mut h = h0;
        let mut rng = Rng::from_state(state);
        let mut spotted = 0;
        let mut spot_point = |v: &mut Vf| {
            *v = point;
            spotted += 1;
        };
        game_sim::ai::habits(&mut h, if track { Some(&mut spot_point) } else { None }, &mut rng, F(dt));
        let what = format!("case {case}: dt {dt}, track {track}, {h0:?}");
        assert_eq!(ee.v0(), 1, "{what}: the answer");
        assert_eq!(spot.borrow().1, spotted, "{what}: the spot asked");
        assert_eq!(ee.read(TK_BRAIN + 0x38, 8) as u64, h.moves, "{what}: the second flag word");
        assert_eq!(ee.u32_at(TK_RIDER + 0x4a4) as i32, h.preload_firing, "{what}: the preload's firing");
        for (at, v, name) in habit_floats(&h) {
            assert_eq!(ee.u32_at(at), v.to_f32().to_bits(), "{what}: {name}");
        }
        assert_eq!(rng.state(), ee.read(rand_state, 8) as u64, "{what}: the generator");

        let on_ground = h0.flags & game_sim::ai::flag::ON_GROUND != 0;
        let just_off = !on_ground && h0.flags & tick_flag::WAS_ON_GROUND != 0;
        let gone = |bit: u64| h0.moves & bit != 0 && h.moves & bit == 0;
        let came = |bit: u64| h0.moves & bit == 0 && h.moves & bit != 0;
        let height = F(h0.position.y()) - h0.ground;
        let low = height <= h0.low_air || height < h0.cross_up_height;
        seen[0] += u32::from(came(habit::PRELOAD_SET) && h0.mode != 5 && node.is_some());
        seen[1] += u32::from(came(habit::PRELOAD_SET) && (h.preload == F(0.0) || h.preload == F(1.0)));
        seen[2] += u32::from(came(habit::PRELOAD_SET) && (h0.mode == 5 || node.is_none()));
        seen[3] += u32::from(came(habit::BRILLIANT));
        seen[4] += u32::from(gone(habit::BRILLIANT));
        seen[5] += u32::from(on_ground && node.is_some_and(|n| n.flags & 8 != 0));
        seen[6] += u32::from(h.lag != h0.lag && h.lag != F(-1.0));
        seen[7] += u32::from(h0.racing && h0.lag_clock <= h0.lag);
        seen[8] += u32::from(h.gate_clock != h0.gate_clock);
        seen[9] += u32::from(h.preload_firing == 1 && h0.preload_firing != 1);
        seen[10] += u32::from(came(habit::CROSS_UP_DUE));
        seen[11] += u32::from(h.clutch == F(1.0));
        seen[12] += u32::from(gone(habit::GATE_CLUTCH));
        seen[13] += u32::from(gone(habit::GATE_WHEELIE));
        seen[14] += u32::from(spotted > 0);
        seen[15] += u32::from(!track && gone(habit::CROSS_UP_DUE));
        seen[16] += u32::from(!on_ground && low && h.steer != h0.steer && h.steer != F(0.0));
        seen[17] += u32::from(!on_ground && !low && h.moves & habit::CROSS_UP != 0 && h.steer == h.cross_up_side);
        seen[18] += u32::from(!on_ground && !low && gone(habit::CROSS_UP));
        seen[19] += u32::from(just_off && h.moves & habit::CROSS_UP_DUE == h0.moves & habit::CROSS_UP_DUE);
    }
    println!("{seen:?}");
    for (k, n) in seen.iter().enumerate() {
        assert!(*n >= 50, "branch {k} reached only {n} times: {seen:?}");
    }
}
