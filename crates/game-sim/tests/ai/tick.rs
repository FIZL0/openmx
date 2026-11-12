// ---------------------------------------------------------------------------------------------------
// S281 — the AI tick: its order and its own clauses, the lever hand-off, and the rider assist
// ---------------------------------------------------------------------------------------------------
//
// `ai_update` runs whole in the interpreter with every call it makes out hooked: the interface's four
// slots and the brain's two through fake vtables, the route follower, the listener walk and the base
// update by address. Each hook logs its name and plays what the real one would leave behind (the copy
// a new speed, forward height and ground bit; the controller new outputs; the route follower and the
// walk an answer), and the port's `ai::update` is handed the same script through `ai::Steps`. The hand-
// off and the assist are each called as the original's own functions; the route follower's tail, which
// builds the assist's aim, is run as a stretch of its function.

use super::*;

pub(crate) const TK_RIDER: u32 = 0x01a0_0000; // the rider: +0x10d0 its interface (+0 the brain)
pub(crate) const TK_BODY: u32 = 0x01a2_0000;
pub(crate) const TK_BRAIN: u32 = 0x01a3_0000;
pub(crate) const TK_BRAIN_VT: u32 = 0x01a3_1000;
pub(crate) const TK_IFACE_VT: u32 = 0x01a3_2000;
pub(crate) const TK_CARD: u32 = 0x01a3_3000;
pub(crate) const TK_KEEPER: u32 = 0x01a3_4000;
/// Not code: the hooked slots.
pub(crate) const TK_FEED: u32 = 0x01a3_5000;
pub(crate) const TK_HAND_OFF: u32 = 0x01a3_5008;
pub(crate) const TK_APPROACH: u32 = 0x01a3_5010;
const TK_RESCUE: u32 = 0x01a3_5018;
pub(crate) const TK_CONTROLS: u32 = 0x01a3_5020;
pub(crate) const TK_EXTRA: u32 = 0x01a3_5028;
const TK_ASSIST: u32 = 0x01a3_5030;

/// A timer or a weight: often an edge the comparisons turn on.
pub(crate) fn edge(pick: &mut Pick, near: f32, span: f32) -> f32 {
    match pick.below(8) {
        0 => near,
        1 => f32::from_bits(near.to_bits().wrapping_add(1)),
        2 => f32::from_bits(near.to_bits().wrapping_sub(1)),
        3 => 0.0,
        4 => -0.0,
        5 => near + float(pick, 0.05),
        _ => float(pick, span),
    }
}

/// The hand-off against the port over ten thousand ticks: with and without a brain, a race card and the
/// score keeper's running bit; the swerve and the back-up timers either side of 0; on and off the ground.
/// The five levers are compared bit for bit, and the answer.
#[test]
fn the_lever_hand_off_matches_over_ten_thousand_ticks() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_hand_off"]) else { return };
    let mut pick = Pick(0x5281_0ff0_a11e_7e45);
    let mut seen = [0u32; 5]; // let go, air, ground, backing up, swerving
    for case in 0..10_000 {
        let has_brain = !pick.chance(12);
        let live = !pick.chance(12);
        let has_card = live || pick.chance(2);
        let keeper_word = (pick.next() as u32 & !1) | u32::from(live || !has_card && pick.chance(2));
        let brain = Brain {
            kind: Some(1),
            mode: 5,
            flags: pick.next(),
            speed: F(0.0),
            forward_y: F(0.0),
            stuck: F(0.0),
            climb: Climb::default(),
            steer: F(float(&mut pick, 1.5)),
            pitch: F(float(&mut pick, 1.5)),
            throttle: F(float(&mut pick, 1.5)),
            brake: F(float(&mut pick, 1.5)),
        };
        let recovery = Recovery {
            back_up: F(edge(&mut pick, 0.0, 2.0)),
            swerve: F(edge(&mut pick, 0.0, 2.0)),
            swerve_bar: F([1.0, -1.0][pick.below(2) as usize]),
        };
        let before = Levers {
            steer: F(float(&mut pick, 2.0)),
            lean: F(float(&mut pick, 2.0)),
            gas: F(float(&mut pick, 2.0)),
            brake: F(float(&mut pick, 2.0)),
            reverse: F(float(&mut pick, 2.0)),
        };

        ee.load(TK_RIDER, &[0u8; 0x1500]);
        ee.load(TK_BRAIN, &[0u8; 0x400]);
        ee.set_u32(TK_RIDER + 0x10d0, if has_brain { TK_BRAIN } else { 0 });
        ee.set_u32(TK_RIDER + 0xf9c, if has_card { TK_CARD } else { 0 });
        ee.set_u32(TK_RIDER + 0xfa0, TK_KEEPER);
        ee.set_u32(TK_KEEPER + 0x5c, keeper_word);
        ee.write(TK_BRAIN + 0x30, 8, u128::from(brain.flags));
        for (off, v) in [(0xe0, brain.throttle), (0xe4, brain.brake), (0xf0, brain.steer), (0xf4, brain.pitch)] {
            ee.set_f32(TK_BRAIN + off, v.to_f32());
        }
        ee.set_f32(TK_RIDER + 0x14c0, recovery.back_up.to_f32());
        ee.set_f32(TK_RIDER + 0x14c4, recovery.swerve_bar.to_f32());
        ee.set_f32(TK_RIDER + 0x14c8, recovery.swerve.to_f32());
        let lever_offs = [0x628, 0x62c, 0x63c, 0x640, 0x648];
        let lever_of = |l: &Levers| [l.steer, l.lean, l.gas, l.brake, l.reverse];
        for (off, v) in lever_offs.iter().zip(lever_of(&before)) {
            ee.set_f32(TK_RIDER + off, v.to_f32());
        }
        ee.call(at[0], &[u64::from(TK_RIDER)], &[0.033_333], BUDGET);

        let mut levers = before;
        let ok = game_sim::ai::hand_off(has_brain.then_some(&brain), has_card && keeper_word & 1 != 0, &recovery, &mut levers);
        assert_eq!(ee.v0(), u64::from(ok), "case {case}: the answer");
        for ((off, ours), name) in lever_offs.iter().zip(lever_of(&levers)).zip(["bar", "lean", "gas", "brake", "reverse"]) {
            assert_eq!(ee.u32_at(TK_RIDER + off), ours.to_f32().to_bits(), "case {case}: the {name} lever");
        }
        let ground = brain.flags & game_sim::ai::flag::ON_GROUND != 0;
        seen[0] += u32::from(!ok);
        seen[1] += u32::from(ok && !ground);
        seen[2] += u32::from(ok && ground);
        seen[3] += u32::from(ok && ground && !(recovery.back_up <= F(0.0)));
        seen[4] += u32::from(ok && !(recovery.swerve <= F(0.0)));
    }
    println!("let go {}, air {}, ground {}, backing up {}, swerving {}", seen[0], seen[1], seen[2], seen[3], seen[4]);
    for (k, n) in seen.iter().enumerate() {
        assert!(*n >= 300, "branch {k} reached only {n} times: {seen:?}");
    }
}

/// The route follower's tail and the rider assist against the port over ten thousand ticks. The tail
/// (from just after the trick plan to the function's epilogue) runs as a stretch with the interface's
/// assist slot hooked, and the original assist is run on the rider when the tail called it; the aim it leaves on the brain and the
/// body's yaw rate and velocity after are compared bit for bit, with the rest of the body untouched.
#[test]
fn the_route_tail_and_the_rider_assist_match_over_ten_thousand_ticks() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_tick_flag", "ai_rider_assist"]) else { return };
    let [follow, assist_fn] = at[..] else { unreachable!() };
    // The tail's first instruction (after the trick plan's call) and its epilogue, as offsets into the
    // route follower.
    let (tail, epilogue) = (follow + 0x16c0, follow + 0x1708);
    assert_eq!(ee.u32_at(tail), 0xde62_0030, "the tail starts with `ld v0, 48(s3)`");

    let assisted = Rc::new(RefCell::new(0u32));
    let a = assisted.clone();
    ee.hooks.insert(
        TK_ASSIST,
        Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, TK_RIDER + 0x10d0, "the assist is called on the interface");
            *a.borrow_mut() += 1;
        }),
    );
    ee.hooks.insert(epilogue, Box::new(|ee| ee.set(31, u64::from(ps2_oracle::DONE) | 0xffff_ffff_0000_0000)));

    let mut pick = Pick(0x5281_a551_57ed_0001);
    let mut seen = [0u32; 3]; // assisted, on the ground, a -0 yaw rate in
    for case in 0..10_000 {
        let flags = pick.next();
        let w = float(&mut pick, 1.0);
        let direction = pick.vec(1.0, w);
        let target_speed = F(match pick.below(3) {
            0 => 150.0,
            1 => float(&mut pick, 200.0),
            _ => pick.range(0.0, 160.0),
        });
        let weights = Weights { heading: F(edge(&mut pick, 1.0, 1.5)), aim: F(edge(&mut pick, 1.0, 1.5)) };
        let heading = F(float(&mut pick, 3.2));
        let on_ground = pick.below(3) != 0;
        let yaw0 = if pick.chance(10) { -0.0 } else { float(&mut pick, 6.0) };
        let w = float(&mut pick, 2.0);
        let vel0 = pick.vec(160.0, w);
        let old_aim = (float(&mut pick, 100.0), float(&mut pick, 100.0));

        ee.load(TK_RIDER, &[0u8; 0x1500]);
        ee.load(TK_BODY, &[0u8; 0x400]);
        ee.load(TK_BRAIN, &[0u8; 0x400]);
        ee.set_u32(TK_RIDER + 0x10d0, TK_BRAIN);
        ee.set_u32(TK_RIDER + 0x10d4, 1);
        ee.set_u32(TK_RIDER + 0x10d8, TK_IFACE_VT);
        ee.set_u32(TK_IFACE_VT + 0x18, TK_ASSIST);
        ee.set_u32(TK_RIDER + 0x7a4, TK_BODY);
        ee.set_u32(TK_RIDER + 0x574, if on_ground { pick.next() as u32 | 1 } else { 0 });
        ee.set_f32(TK_RIDER + 0x1138, weights.heading.to_f32());
        ee.set_f32(TK_RIDER + 0x113c, weights.aim.to_f32());
        ee.set_u32(TK_BRAIN + 0x40, TK_RIDER + 0x10d0);
        ee.write(TK_BRAIN + 0x30, 8, u128::from(flags));
        put(&mut ee, TK_BRAIN + 0x1d0, direction);
        ee.set_f32(TK_BRAIN + 0x1e0, target_speed.to_f32());
        ee.set_f32(TK_BRAIN + 0x1c0, old_aim.0);
        ee.set_f32(TK_BRAIN + 0x1c8, old_aim.1);
        ee.set_f32(TK_BRAIN + 0x1f4, heading.to_f32());
        for k in 0..0x40u32 {
            ee.set_u32(TK_BODY + 0x200 + 4 * k, pick.next() as u32);
        }
        ee.set_f32(TK_BODY + 0x284, yaw0);
        put(&mut ee, TK_BODY + 0x260, vel0);
        let body_before: Vec<u32> = (0..0x40u32).map(|k| ee.u32_at(TK_BODY + 0x200 + 4 * k)).collect();
        *assisted.borrow_mut() = 0;

        ee.set(19, u64::from(TK_BRAIN));
        ee.set_f(22, 0.033_333);
        ee.call(tail, &[], &[], BUDGET);
        // The assist is the tail's last call, so the original's runs straight after it (calling it from
        // inside the hook would move the hook).
        if *assisted.borrow() > 0 {
            ee.call(assist_fn, &[u64::from(TK_RIDER)], &[], BUDGET);
            assert_eq!(ee.v0(), 1, "case {case}: the assist returns 1");
        }

        let mut yaw = F(yaw0);
        let mut vel = vel0;
        let aim = game_sim::ai::assist_aim(flags, direction, target_speed);
        if let Some(aim) = aim {
            game_sim::ai::assist(&weights, heading, aim, on_ground, &mut yaw, &mut vel);
        }
        assert_eq!(*assisted.borrow(), u32::from(aim.is_some()), "case {case}: the assist calls");
        let (ax, az) = aim.unwrap_or((F(old_aim.0), F(old_aim.1)));
        assert_eq!(ee.u32_at(TK_BRAIN + 0x1c0), ax.to_f32().to_bits(), "case {case}: aim x");
        assert_eq!(ee.u32_at(TK_BRAIN + 0x1c8), az.to_f32().to_bits(), "case {case}: aim z");
        let mut want = body_before;
        want[(0x284 - 0x200) / 4] = yaw.to_f32().to_bits();
        for (k, b) in vel.to_bits().iter().enumerate() {
            want[(0x260 - 0x200) / 4 + k] = *b;
        }
        for (k, w) in want.iter().enumerate() {
            assert_eq!(ee.u32_at(TK_BODY + 0x200 + 4 * k as u32), *w, "case {case}: body +{:#x}", 0x200 + 4 * k);
        }
        seen[0] += u32::from(aim.is_some());
        seen[1] += u32::from(aim.is_some() && on_ground);
        seen[2] += u32::from(aim.is_some() && yaw0.to_bits() == (-0.0f32).to_bits());
    }
    println!("assisted {}, on the ground {}, -0 yaw rate {}", seen[0], seen[1], seen[2]);
    for (k, n) in seen.iter().enumerate() {
        assert!(*n >= 200, "branch {k} reached only {n} times: {seen:?}");
    }
}

/// What a hooked call of the tick plays back.
#[derive(Clone, Default)]
pub(crate) struct TickScript {
    pub(crate) log: Vec<&'static str>,
    pub(crate) fed: (f32, f32, bool),
    pub(crate) controlled: [f32; 4],
    pub(crate) routed: bool,
    pub(crate) heard: bool,
    pub(crate) other: bool,
}

/// The port's side of [`TickScript`].
pub(crate) struct Played(pub(crate) TickScript);

impl Steps for Played {
    fn rescue(&mut self, _: &mut Brain) {
        self.0.log.push("rescue");
    }
    fn feed(&mut self, b: &mut Brain) {
        self.0.log.push("feed");
        let (speed, forward_y, ground) = self.0.fed;
        b.speed = F(speed);
        b.forward_y = F(forward_y);
        b.flags = if ground { b.flags | game_sim::ai::flag::ON_GROUND } else { b.flags & !game_sim::ai::flag::ON_GROUND };
    }
    fn route(&mut self, _: &mut Brain, _: F) -> bool {
        self.0.log.push("route");
        self.0.routed
    }
    fn controls(&mut self, b: &mut Brain) {
        self.0.log.push("controls");
        let [t, k, s, p] = self.0.controlled;
        (b.throttle, b.brake, b.steer, b.pitch) = (F(t), F(k), F(s), F(p));
    }
    fn approach(&mut self, _: &mut Brain, _: F) {
        self.0.log.push("approach");
    }
    fn extra(&mut self, _: &mut Brain, _: F) {
        self.0.log.push("extra");
    }
    fn hand_off(&mut self, _: &mut Brain, _: F) {
        self.0.log.push("hand-off");
    }
    fn listeners(&mut self, _: &mut Brain, _: F) -> bool {
        self.0.log.push("listeners");
        self.0.heard
    }
    fn other_mode(&mut self, _: &mut Brain, _: F) -> bool {
        self.0.log.push("other mode");
        self.0.other
    }
    fn mode_two(&mut self, _: &mut Brain, _: F) -> bool {
        self.0.log.push("mode 2");
        self.0.other
    }
    fn mode_four(&mut self, _: &mut Brain, _: F) -> bool {
        self.0.log.push("mode 4");
        self.0.other
    }
}

/// `ai_update` against `ai::update` over ten thousand ticks: every interface type and none, riding and
/// other modes, the stuck clock and the climb clock about their edges, the flagged steps on and off. The
/// calls made (in order), the answer and every brain field the tick or its scripted calls touch are
/// compared bit for bit.
#[test]
fn the_ai_tick_matches_over_ten_thousand_ticks() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_update", "ai_tick_flag", "ai_listener_walk", "ai_update_base"]) else { return };
    let [update, follow, walk, base] = at[..] else { unreachable!() };

    let script = Rc::new(RefCell::new(TickScript::default()));
    let logged = |name: &'static str, s: &Rc<RefCell<TickScript>>| {
        let s = s.clone();
        Box::new(move |_: &mut ps2_oracle::Ee| s.borrow_mut().log.push(name)) as Box<dyn FnMut(&mut ps2_oracle::Ee)>
    };
    ee.hooks.insert(TK_RESCUE, logged("rescue", &script));
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
    let s = script.clone();
    ee.hooks.insert(follow, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        s.log.push("route");
        ee.set(2, u64::from(s.routed));
    }));
    let s = script.clone();
    ee.hooks.insert(walk, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        s.log.push("listeners");
        ee.set(2, u64::from(s.heard));
    }));
    let s = script.clone();
    ee.hooks.insert(base, Box::new(move |ee| {
        let mut s = s.borrow_mut();
        s.log.push("other mode");
        ee.set(2, u64::from(s.other));
    }));
    let iface = TK_RIDER + 0x10d0;
    // With no interface the original calls the hand-off through a null pointer: low RAM holds a vtable
    // pointer here so that call lands on the hook too.
    let null_vt = ee.u32_at(8);
    ee.set_u32(8, TK_IFACE_VT);

    let mut pick = Pick(0x5281_71c4_0bde_0001);
    let mut seen = [0u32; 8]; // rescued, stuck counting, other mode, climb cut, climb lean, cut starts, approach, extra
    for case in 0..10_000 {
        let kind = match pick.below(10) {
            0 => None,
            1..=5 => Some(1),
            _ => Some([0, 2, 3, 6, 4, 5, 7][pick.below(7) as usize]),
        };
        let mode = if pick.chance(8) { [0, 1, 2, 3, 4, 6][pick.below(6) as usize] } else { 5 };
        let mut flags = pick.next();
        for bit in [tick_flag::APPROACH, tick_flag::EXTRA] {
            if pick.chance(2) {
                flags &= !bit;
            }
        }
        let dt = if pick.chance(2) { 0.033_333_335 } else { pick.range(0.0, 0.1) };
        let climb_speed = if pick.chance(2) { 40.0 } else { pick.range(0.0, 60.0) };
        let steep = if pick.chance(2) { 0.7 } else { pick.range(-1.0, 1.0) };
        let mut brain = Brain {
            kind,
            mode,
            flags,
            speed: F(edge(&mut pick, 10.0, 60.0)),
            forward_y: F(edge(&mut pick, steep, 1.0)),
            stuck: F(edge(&mut pick, 5.0 - dt, 5.0)),
            climb: Climb {
                hold: F(edge(&mut pick, 0.0, 5.0)),
                steep: F(steep),
                timer: F(if pick.chance(2) { 5.0 - dt * pick.fraction() } else { edge(&mut pick, 5.0 - dt, 5.0) }),
                speed: F(climb_speed),
            },
            steer: F(float(&mut pick, 1.0)),
            pitch: F(float(&mut pick, 1.0)),
            throttle: F(edge(&mut pick, 0.0, 1.0)),
            brake: F(float(&mut pick, 1.0)),
        };
        let played = TickScript {
            log: Vec::new(),
            fed: (edge(&mut pick, climb_speed, 60.0), edge(&mut pick, steep, 1.0), pick.chance(2)),
            controlled: [edge(&mut pick, 0.0, 1.0), float(&mut pick, 1.0), float(&mut pick, 1.0), float(&mut pick, 1.0)],
            routed: !pick.chance(4),
            heard: !pick.chance(4),
            other: pick.chance(2),
        };

        ee.load(TK_RIDER, &[0u8; 0x1500]);
        ee.load(TK_BRAIN, &[0u8; 0x400]);
        ee.set_u32(TK_BRAIN + 8, TK_BRAIN_VT);
        ee.set_u32(TK_BRAIN_VT + 0x90, TK_CONTROLS);
        ee.set_u32(TK_BRAIN_VT + 0xa8, TK_EXTRA);
        ee.set_u32(iface, TK_BRAIN);
        ee.set_u32(iface + 4, kind.unwrap_or(0) as u32);
        ee.set_u32(iface + 8, TK_IFACE_VT);
        for (slot, hook) in [(0x8, TK_FEED), (0xc, TK_HAND_OFF), (0x28, TK_APPROACH), (0x40, TK_RESCUE)] {
            ee.set_u32(TK_IFACE_VT + slot, hook);
        }
        ee.set_u32(TK_BRAIN + 0x40, if kind.is_some() { iface } else { 0 });
        ee.set_u32(TK_BRAIN + 0xd0, mode as u32);
        ee.write(TK_BRAIN + 0x30, 8, u128::from(brain.flags));
        let fields = |b: &Brain| {
            [
                (0x80, b.speed, "speed"),
                (0xb4, b.forward_y, "forward height"),
                (0x3f0, b.stuck, "stuck clock"),
                (0x390, b.climb.hold, "climb hold"),
                (0x394, b.climb.steep, "climb steepness"),
                (0x398, b.climb.timer, "climb clock"),
                (0x30c, b.climb.speed, "climb speed"),
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

        ee.call(update, &[u64::from(TK_BRAIN)], &[dt], BUDGET);

        let before = brain;
        let mut ours = Played(played);
        let ok = game_sim::ai::update(&mut brain, &mut ours, F(dt));
        let theirs = script.borrow();
        assert_eq!(theirs.log, ours.0.log, "case {case}: the calls, in order ({before:?})");
        assert_eq!(ee.v0(), u64::from(ok), "case {case}: the answer");
        assert_eq!(ee.read(TK_BRAIN + 0x30, 8) as u64, brain.flags, "case {case}: flags");
        for (off, v, name) in fields(&brain) {
            assert_eq!(ee.u32_at(TK_BRAIN + off), v.to_f32().to_bits(), "case {case}: {name} ({before:?})");
        }
        let riding_mx = mode == 5 && kind == Some(1);
        seen[0] += u32::from(ours.0.log.contains(&"rescue"));
        seen[1] += u32::from(brain.stuck != F(0.0));
        seen[2] += u32::from(mode != 5);
        seen[3] += u32::from(riding_mx && !(before.climb.hold <= F(0.0)));
        seen[4] += u32::from(riding_mx && before.climb.hold <= F(0.0) && brain.climb.timer != F(0.0));
        seen[5] += u32::from(riding_mx && before.climb.hold <= F(0.0) && !(brain.climb.hold <= F(0.0)));
        seen[6] += u32::from(ours.0.log.contains(&"approach"));
        seen[7] += u32::from(ours.0.log.contains(&"extra"));
    }
    ee.set_u32(8, null_vt);
    println!(
        "rescued {}, stuck counting {}, other mode {}, climb cut {}, climb lean {}, cut starts {}, approach {}, extra {}",
        seen[0], seen[1], seen[2], seen[3], seen[4], seen[5], seen[6], seen[7]
    );
    for (k, n) in seen.iter().enumerate() {
        assert!(*n >= 100, "branch {k} reached only {n} times: {seen:?}");
    }
}

/// The tick's live side, on the free09 riders: one AI tick of each of the three, replayed through
/// `ai::update` on every pair of consecutive rows of `context/capture/s281/ai-tick.csv` (watch/s281-ai-
/// tick.toml, slot 44, no pad). The rider-to-brain copy and the controller play the next row's speed,
/// forward height, ground bit and controls; the hand-off runs the port's `ai::hand_off` on the levers of
/// the row before. The port must leave the next row's stuck and climb clocks, its controls and its
/// levers bit for bit, and carry the ground bit into its flag word.
///
/// The swerve timer is read at its own phase: a hit reaction sets it to 1 before the AI tick and the
/// bike counts it down by `dt` after, so the hand-off saw it positive exactly when it changed (and was
/// not set afresh, to 1 exactly, after the countdown) or is still positive.
#[test]
fn the_free09_riders_ticks_are_the_originals() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/capture/s281/ai-tick.csv");
    let Ok(text) = std::fs::read_to_string(&path) else {
        skip(&format!(
            "no {}: `./kit pcsx2 with -- ./kit record watch/s281-ai-tick.toml -o context/capture/s281/ai-tick.csv` \
             (slot 44 of copy 1, journal s12-ai-tick)",
            path.display()
        ));
        return;
    };
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().expect("a header").split(',').collect();
    let rows: Vec<Vec<&str>> = lines.map(|l| l.split(',').collect()).collect();
    let col = |name: &str| head.iter().position(|h| *h == name).unwrap_or_else(|| panic!("no column {name}"));
    let fl = |row: &[&str], name: &str| F(row[col(name)].parse::<f64>().unwrap_or_else(|e| panic!("{name}: {e}")) as f32);
    let word = |row: &[&str], name: &str| row[col(name)].parse::<u64>().unwrap_or_else(|e| panic!("{name}: {e}"));

    struct Live {
        fed: (F, F, bool),
        controlled: [F; 4],
        live: bool,
        recovery: Recovery,
        levers: Levers,
        log: Vec<&'static str>,
    }
    impl Steps for Live {
        fn rescue(&mut self, _: &mut Brain) {
            self.log.push("rescue");
        }
        fn feed(&mut self, b: &mut Brain) {
            (b.speed, b.forward_y) = (self.fed.0, self.fed.1);
            b.flags = if self.fed.2 { b.flags | game_sim::ai::flag::ON_GROUND } else { b.flags & !game_sim::ai::flag::ON_GROUND };
        }
        fn route(&mut self, _: &mut Brain, _: F) -> bool {
            true
        }
        fn controls(&mut self, b: &mut Brain) {
            [b.throttle, b.brake, b.steer, b.pitch] = self.controlled;
        }
        fn approach(&mut self, _: &mut Brain, _: F) {}
        fn extra(&mut self, _: &mut Brain, _: F) {
            self.log.push("extra");
        }
        fn hand_off(&mut self, b: &mut Brain, _: F) {
            game_sim::ai::hand_off(Some(b), self.live, &self.recovery, &mut self.levers);
        }
        fn listeners(&mut self, _: &mut Brain, _: F) -> bool {
            true
        }
        fn other_mode(&mut self, _: &mut Brain, _: F) -> bool {
            self.log.push("other mode");
            true
        }
        fn mode_two(&mut self, _: &mut Brain, _: F) -> bool {
            self.log.push("mode 2");
            true
        }
        fn mode_four(&mut self, _: &mut Brain, _: F) -> bool {
            self.log.push("mode 4");
            true
        }
    }

    let mut pairs = 0;
    let mut swerves = 0;
    let mut air = 0;
    let mut carry_misses = Vec::new();
    for w in rows.windows(2) {
        let (p, r) = (&w[0][..], &w[1][..]);
        let step = fl(r, "clock") - fl(p, "clock");
        // A row the recorder missed (two ticks between rows) or repeated (none).
        if !(step > F(0.03) && step < F(0.04)) {
            continue;
        }
        let dt = F(0.033_333_335);
        for k in 0..3 {
            let b = |n: &str| format!("b{k}_{n}");
            let rr = |n: &str| format!("r{k}_{n}");
            let flags = |row: &[&str]| word(row, &b("flags_lo")) | word(row, &b("flags_hi")) << 32;
            let mut brain = Brain {
                kind: Some(1),
                mode: 5,
                flags: flags(p),
                speed: fl(p, &b("speed")),
                forward_y: fl(p, &b("fwd_y")),
                stuck: fl(p, &b("stuck")),
                climb: Climb {
                    hold: fl(p, &b("climb_hold")),
                    steep: fl(p, &b("climb_steep")),
                    timer: fl(p, &b("climb_timer")),
                    speed: fl(p, &b("climb_speed")),
                },
                throttle: fl(p, &b("throttle")),
                brake: fl(p, &b("brake")),
                steer: fl(p, &b("steer")),
                pitch: fl(p, &b("pitch")),
            };
            let (was, now) = (fl(p, &rr("swerve")), fl(r, &rr("swerve")));
            let swerving = if now.to_bits() == was.to_bits() {
                now > F(0.0)
            } else {
                now != F(1.0) || was > F(0.0)
            };
            let lever = |row: &[&str]| Levers {
                steer: fl(row, &rr("lever_steer")),
                lean: fl(row, &rr("lever_lean")),
                gas: fl(row, &rr("lever_gas")),
                brake: fl(row, &rr("lever_brake")),
                reverse: fl(row, &rr("lever_reverse")),
            };
            let mut live = Live {
                fed: (fl(r, &b("speed")), fl(r, &b("fwd_y")), flags(r) & 0x20 != 0),
                controlled: [fl(r, &b("throttle")), fl(r, &b("brake")), fl(r, &b("steer")), fl(r, &b("pitch"))],
                live: word(r, &rr("card")) != 0,
                recovery: Recovery {
                    back_up: fl(r, &rr("back_up")),
                    swerve: if swerving { F(1.0) } else { F(0.0) },
                    swerve_bar: fl(r, &rr("swerve_bar")),
                },
                levers: lever(p),
                log: Vec::new(),
            };
            let at = format!("frame {} rider {k}", r[col("frame")]);
            assert!(game_sim::ai::update(&mut brain, &mut live, dt), "{at}: the answer");
            assert!(live.log.is_empty(), "{at}: calls the free09 riders never make: {:?}", live.log);
            let want = lever(r);
            assert_eq!(
                [live.levers.steer, live.levers.lean, live.levers.gas, live.levers.brake, live.levers.reverse].map(F::to_bits),
                [want.steer, want.lean, want.gas, want.brake, want.reverse].map(F::to_bits),
                "{at}: the levers (bar, lean, gas, brake, reverse)"
            );
            for (name, ours) in [
                ("stuck", brain.stuck),
                ("climb_hold", brain.climb.hold),
                ("climb_timer", brain.climb.timer),
                ("throttle", brain.throttle),
                ("brake", brain.brake),
                ("steer", brain.steer),
                ("pitch", brain.pitch),
            ] {
                assert_eq!(ours.to_bits(), fl(r, &b(name)).to_bits(), "{at}: {name}");
            }
            if brain.flags & tick_flag::WAS_ON_GROUND != flags(r) & tick_flag::WAS_ON_GROUND {
                carry_misses.push(at);
            }
            pairs += 1;
            swerves += u32::from(swerving);
            air += u32::from(flags(r) & 0x20 == 0);
        }
    }
    println!("{pairs} rider ticks, {swerves} swerving, {air} in the air; carry misses {carry_misses:?}");
    assert!(pairs >= 2500, "only {pairs} rider ticks");
    assert!(swerves >= 50 && air >= 500, "swerving {swerves}, in the air {air}");
    // The ground-bit carry: the route follower writes the flag word too, and on the first airborne tick
    // the row can catch the word between writers (two take-offs in this capture, journal s12-ai-tick).
    assert!(carry_misses.len() <= 2, "the ground-bit carry missed {} ticks: {carry_misses:?}", carry_misses.len());
}
