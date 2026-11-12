// ---------------------------------------------------------------------------------------------------
// S12d — when an AI rider asks for a trick, and which
// ---------------------------------------------------------------------------------------------------

use super::*;

/// The rider, with its stunt records, and the AI brain, in RAM nothing else uses. The rider's AI
/// interface is the sub-object at +0x10d0, whose vtable pointer is at its +8.
const RIDER: u32 = 0x0190_0000;
const IFACE: u32 = RIDER + 0x10d0;
const RECORDS: u32 = 0x0193_0000;
const BRAIN: u32 = 0x0194_0000;
const NODE: u32 = 0x0195_0000;
/// The rider's trick fields the decision writes, and its stunt array.
const ARMED: u32 = 0xa18;
pub(crate) const IN_PROGRESS: u32 = 0xa0c;
const STUNTS: u32 = 0xa34;
const REQUESTED: u32 = 0xa7c;
const RESTART: u32 = 0xaf8;
const RECORD: u32 = 0x54;

/// A float in `-span..span`, now and then an edge the comparisons turn on.
pub(crate) fn float(pick: &mut Pick, span: f32) -> f32 {
    match pick.below(16) {
        0 => 0.0,
        1 => -0.0,
        _ => {
            let r = pick.next();
            let unit = (r >> 40) as f32 / (1u32 << 24) as f32;
            if r & 1 == 0 { span * unit } else { -span * unit }
        }
    }
}

/// Stand in for the library's `strcmp`, which uses two quadword compares the interpreter lacks (as in
/// tests/trick.rs).
fn hook_strcmp(ee: &mut ps2_oracle::Ee, strcmp: u32) {
    ee.hooks.insert(
        strcmp,
        Box::new(|ee| {
            let (mut a, mut b) = (ee.r(4) as u32, ee.r(5) as u32);
            loop {
                let (x, y) = (ee.u8_at(a), ee.u8_at(b));
                if x != y || x == 0 {
                    ee.set(2, (i64::from(x) - i64::from(y)) as u64);
                    return;
                }
                a += 1;
                b += 1;
            }
        }),
    );
}

/// The rider's 24 stunt records as its own copy lays them out.
fn write_stunts(ee: &mut ps2_oracle::Ee, stunts: &[Stunt]) {
    for (k, s) in stunts.iter().enumerate() {
        let at = RECORDS + k as u32 * RECORD;
        let mut name = [0u8; 0x20];
        name[..s.name.len()].copy_from_slice(s.name.as_bytes());
        ee.load(at, &name);
        ee.set_f32(at + 0x48, s.duration.to_f32());
        ee.set_u32(at + 0x50, s.index as u32);
    }
    ee.set_u32(RIDER + STUNTS, RECORDS);
}

/// The brain's whole trick decision against the port, with the original's own interface vtable, its
/// chooser and its "do this trick" call behind it, and the original's `rand()`.
#[test]
fn the_trick_decision_matches_over_ten_thousand_frames() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_trick_plan", "ai_stunt_vtable", "rand_state", "strcmp"]) else {
        return;
    };
    let [plan_fn, vtable, rand_state, strcmp] = at[..] else { unreachable!() };
    hook_strcmp(&mut ee, strcmp);

    // flag bits the decision reads, and two it does not, so a port that clears too much shows.
    const BITS: [u64; 8] = [0x2, 0x8, 0x10, 0x20, 0x2000, 0x8_0000, 0x1, 0x1_0000_0000];
    let mut pick = Pick(0x2bd1_77e0_5eed_0d12);
    let mut seen = [0u32; 10];
    for case in 0..10_000 {
        let mut flags: u64 = 0;
        for (k, b) in BITS.iter().enumerate() {
            // The roll needs 2 and 8 set and 0x10, 0x20 clear: lean the draws that way.
            let p = match k {
                0 | 1 => 7,
                2 => 1,
                3 => 3,
                _ => 4,
            };
            if pick.below(8) < p {
                flags |= b;
            }
        }
        let attached = pick.below(16) != 0;
        let jump = match pick.below(4) {
            0 => None,
            1 => Some(Jump { flags: pick.below(0x100) as u32 & !JUMP, airtime: F(float(&mut pick, 4.0)) }),
            _ => Some(Jump { flags: pick.below(0x100) as u32 | JUMP, airtime: F(float(&mut pick, 4.0)) }),
        };
        let mut plan = TrickPlan {
            cooldown: F(float(&mut pick, 12.0).abs()),
            since: F(float(&mut pick, 12.0).abs()),
            chance: F([0.0, 100.0, 10.0, 50.0][pick.below(4) as usize] + float(&mut pick, 1.0)),
            estimate: F(float(&mut pick, 4.0)),
            slot: pick.below(24) as i32,
            air: F(float(&mut pick, 2.0).abs()),
        };
        let flight = Flight {
            vertical_speed: F(float(&mut pick, 40.0)),
            height: F(float(&mut pick, 60.0)),
            ground: F(float(&mut pick, 20.0)),
            jump,
        };
        let dt = F([1.0 / 30.0, 1.0 / 60.0, 0.0][pick.below(3) as usize]);
        let throwaway = pick.below(SLOTS as u32 + 1) as usize;
        let stunts: Vec<Stunt> = (0..SLOTS)
            .map(|i| Stunt {
                name: if i == throwaway { "throwaway".to_owned() } else { format!("stunt{i}") },
                duration: F(float(&mut pick, 3.0).abs()),
                index: i as i32,
                ..Stunt::default()
            })
            .collect();
        let requested = pick.below(30) as u8;
        let restart = pick.below(2) == 1;
        let armed = pick.below(2) == 1;
        let state = pick.next();

        ee.load(BRAIN, &[0u8; 0x400]);
        ee.load(RIDER, &[0u8; 0x1100]);
        write_stunts(&mut ee, &stunts);
        ee.set_u32(IFACE + 8, vtable);
        ee.set_u32(BRAIN + 0x40, if attached { IFACE } else { 0 });
        ee.write(BRAIN + 0x30, 8, u128::from(flags));
        match jump {
            Some(j) => {
                ee.set_u32(NODE + 4, j.flags);
                ee.set_f32(NODE + 0x48, j.airtime.to_f32());
                ee.set_u32(BRAIN + 0x224, NODE);
            }
            None => ee.set_u32(BRAIN + 0x224, 0),
        }
        ee.set_f32(BRAIN + 0x158, plan.cooldown.to_f32());
        ee.set_f32(BRAIN + 0x15c, plan.since.to_f32());
        ee.set_f32(BRAIN + 0x160, plan.chance.to_f32());
        ee.set_f32(BRAIN + 0x19c, plan.estimate.to_f32());
        ee.set_u32(BRAIN + 0x1a0, plan.slot as u32);
        ee.set_f32(BRAIN + 0x200, plan.air.to_f32());
        ee.set_f32(BRAIN + 0x54, flight.vertical_speed.to_f32());
        ee.set_f32(BRAIN + 0xc4, flight.height.to_f32());
        ee.set_f32(BRAIN + 0x204, flight.ground.to_f32());
        ee.set_u32(RIDER + REQUESTED, u32::from(requested));
        ee.set_u32(RIDER + RESTART, u32::from(restart));
        ee.set_u32(RIDER + ARMED, u32::from(armed));
        ee.write(rand_state, 8, u128::from(state));

        ee.call(plan_fn, &[u64::from(BRAIN)], &[dt.to_f32()], BUDGET);
        assert_eq!(ee.v0(), 1, "case {case}: the decision always returns 1");

        let mut rng = Rng::from_state(state);
        let set = Set::new(stunts.clone());
        let mut tricks = Tricks::default();
        tricks.requested = Trick(requested);
        tricks.restart = restart;
        tricks.gesture.armed = armed;
        let mut got_flags = flags;
        let before = plan;
        plan_trick(dt, &mut got_flags, &mut plan, flight, attached.then_some((&set, &mut tricks)), &mut rng);

        let what = format!("case {case}: flags {flags:#x} {before:?} {flight:?} dt {dt:?} attached {attached}");
        let want = (
            ee.read(BRAIN + 0x30, 8) as u64,
            ee.u32_at(BRAIN + 0x15c),
            ee.u32_at(BRAIN + 0x19c),
            ee.i32_at(BRAIN + 0x1a0),
            ee.u32_at(BRAIN + 0x200),
            ee.u32_at(RIDER + REQUESTED),
            ee.u32_at(RIDER + RESTART) != 0,
            ee.u32_at(RIDER + ARMED) != 0,
            ee.read(rand_state, 8) as u64,
        );
        let got = (
            got_flags,
            plan.since.to_bits(),
            plan.estimate.to_bits(),
            plan.slot,
            plan.air.to_bits(),
            u32::from(tricks.requested.0),
            tricks.restart,
            tricks.gesture.armed,
            rng.state(),
        );
        assert_eq!(got, want, "{what}: (flags, since, estimate, slot, air, requested, restart, armed, rng)");

        // Which way the frame went, so the run can show it reached every one.
        let rolled = rng.state() != state;
        let k = if !rolled && got_flags & 0x8 == 0 && flags & 0x8 != 0 {
            1 // gave up on the cool-down (or a roll it could not make)
        } else if !rolled {
            0
        } else if got_flags & 0x8_0000 != 0 && flags & 0x8_0000 == 0 {
            2 // asked for a trick
        } else if got_flags & 0x8 == 0 {
            3 // the roll failed
        } else {
            4 // the chooser found nothing
        };
        seen[k] += 1;
        if rolled {
            let ballistic = flags & 0x2000 != 0;
            seen[5 + usize::from(ballistic) + usize::from(ballistic && flight.vertical_speed <= F::ZERO)] += 1;
        }
        if flags & 0x20 != 0 && flags & 0x8_0000 != 0 {
            seen[8] += 1; // a tricked flight's landing
        }
        if !attached {
            seen[9] += 1;
        }
    }
    println!(
        "the trick decision: {seen:?} (none, cool-down, asked, failed roll, nothing fits, route, rising, falling, \
         landings, no rider)"
    );
    assert!(seen.iter().all(|&n| n > 0), "a branch was never reached: {seen:?}");
}

/// The "do this trick" call over every slot either side of the 24, against the original's.
#[test]
fn ask_for_matches_every_slot() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_ask_for"]) else { return };
    for slot in -40i32..60 {
        for (restart, armed) in [(false, false), (false, true), (true, false), (true, true)] {
            ee.load(RIDER, &[0u8; 0xb00]);
            ee.set_u32(RIDER + REQUESTED, 7);
            ee.set_u32(RIDER + RESTART, u32::from(restart));
            ee.set_u32(RIDER + ARMED, u32::from(armed));
            ee.call(at[0], &[u64::from(RIDER), slot as i64 as u64], &[], BUDGET);
            let want = (
                ee.v0() != 0,
                ee.u32_at(RIDER + REQUESTED),
                ee.u32_at(RIDER + RESTART) != 0,
                ee.u32_at(RIDER + ARMED) != 0,
            );
            let mut t = Tricks::default();
            t.requested = Trick(7);
            t.restart = restart;
            t.gesture.armed = armed;
            let ok = ask_for(&mut t, slot);
            assert_eq!((ok, u32::from(t.requested.0), t.restart, t.gesture.armed), want, "slot {slot}");
        }
    }
}

/// The AI rider's request gate over every state it reads: whether it holds, or hands over to the pad.
#[test]
fn the_gesture_gate_matches_every_state() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["ai_gesture_gate", "stunt_request"]) else { return };
    let (gate, request) = (at[0], at[1]);
    let asked = Rc::new(RefCell::new(false));
    let a = asked.clone();
    ee.hooks.insert(
        request,
        Box::new(move |ee| {
            *a.borrow_mut() = true;
            ee.set(2, 0);
        }),
    );
    for brain in [0u32, 0x0196_0000] {
        for restart in [0u32, 1, 0x100] {
            for in_progress in [0u32, 1, 0x8000_0000] {
                ee.load(RIDER, &[0u8; 0x1100]);
                ee.set_u32(RIDER + 0x10d0, brain);
                ee.set_u32(RIDER + RESTART, restart);
                ee.set_u32(RIDER + IN_PROGRESS, in_progress);
                *asked.borrow_mut() = false;
                ee.call(gate, &[u64::from(RIDER), 0xff], &[], BUDGET);
                let want = (ee.v0() != 0 && !*asked.borrow(), *asked.borrow());
                let mut t = Tricks::default();
                t.restart = restart != 0;
                t.in_progress = in_progress != 0;
                let holds = gate_holds(brain != 0, &t);
                assert_eq!((holds, !holds), want, "brain {brain:#x} restart {restart} in progress {in_progress}");
            }
        }
    }
}
