//! The rider's own animation selection against the original, and then against the two things the task
//! asked for in words.
//!
//! 1. **the selection, one state at a time** — thirty thousand generated frames of the original's
//!    selection with the step behind it hooked away, steered arm by arm through every state the
//!    machine can ask for: the twenty-four gesture slots, the no-hander, the airborne pose, the
//!    backflip, both halves of the reverse pair, the wreck's flail and its five held limbs, the gate's
//!    four steps, and the five branches of the idle-and-look-around. Compared per frame: the state
//!    asked for, all four of the body's own clocks and flags, the motion object's finished flag, the
//!    cut flag the gate clears, the ten-word no-repeat list and its count, the generator's own
//!    sixty-four-bit state — which is what proves the *number* and order of the random draws, not just
//!    their values — and the two fields the tail latches off the request.
//! 2. **a body dropped on a bike gets ready by itself** — the gate sequence driven through the ported
//!    selection *and* the ported step, with nothing else writing the state at all: a get-ready idle,
//!    then the breathe, then a start pose, then riding when the throttle opens.
//! 3. **a wreck stops it riding** — the same, with the request saying the body is off the bike.
//! 4. **the wiring** — the same gate sequence again, but driven through a whole
//!    [`game_sim::rider::Rider`] on the hand-written test bike: the request block published by
//!    [`game_sim::rider::Rider::body_frame`] off the rider's own fields, and the player's six-step
//!    update running in between every pair of body frames without touching the body's state.
//! 5. **a wreck that settles** — a whole rider with wreck files under it, dropped onto flat ground
//!    hard enough to come off: the body's *mode* machine (V19) is the only thing that writes
//!    `settled`, so a wrecked body flails while it is thrown about, settles the tick its roll runs
//!    past `RiderRollTime`, holds one of its five limbs and keeps that one, and has the settle
//!    cleared again when the machine puts the rider back.
//! 6. **the wreck arm of the mode machine** — ten thousand generated frames of the original's *other*
//!    vtable slot (V19), its ten callees hooked away so that only the mode chain and the wreck's own
//!    clocks, roll and settle test run. Compared per frame: the mode and the previous mode, the settle
//!    and flung flags, the contact flag, all three clocks, and which of the two bone damping sets the
//!    frame asked for. The ground query's hook is not a no-op — it answers with the ground height
//!    the case asks for, which is how the settle test's clearance is reached from a test — and the
//!    port reads its three sensors off the same bones with `Thrown::from_bones` (S52).
//!
//! The middle four need no disc: they are the task's behavioural tests and they run everywhere.
//! What is not proved here, and why, is in the journal entry's last section.

use game_sim::biplane::Body as Bone;
use game_sim::collide::{Bundle, Record};
use game_sim::motion::{Clip, ClipId, Player};
use game_sim::phy::{self, Phy};
use game_sim::ragdoll_contact::Contact;
use game_sim::ride::Controls;
use game_sim::rider::{self, Rider};
use game_sim::rider_anim::{
    Ask, Body, Clock, Thrown, HOLD_LIMB, HOLD_LIMBS, IDLE, IDLES, Params, Pool,
};
use game_sim::rng::Rng;
use game_sim::trick::{SLOTS, Set};
use game_sim::world::{Flat, World};
use game_sim::vehicle_anim::{
    self, AIR, BACKFLIP, GET_READY, HOLD, Library, REVERSE_L, START_POS, STATES, TURN_RIDING,
};
use std::cell::Cell;
use std::rc::Rc;

use ps2_float::{F, Vf};
use ps2_oracle::{addr, game_or_skip, skip};

/// Instruction budget per call. The selection itself is a couple of hundred instructions; the gate's
/// no-repeat draw can spend a few thousand rejecting duplicates when nine of the ten are taken.
const BUDGET: u64 = 2_000_000;

// ---------------------------------------------------------------------------------------------------
// where this test puts things
// ---------------------------------------------------------------------------------------------------

/// Well above the program image: the body and the request block it is pointed at.
const BODY: u32 = 0x0120_0000;
const REQ: u32 = 0x0120_2000;

/// The body's own fields, by the offsets the original uses. Named here because a test may say where
/// things live; the port may not.
const B_FINISHED: u32 = 0x3c;
const B_REQUEST: u32 = 0x488;
const B_RATE_READY: u32 = 0x694;
const B_SETTLED: u32 = 0x6b4;
const B_WANT: u32 = 0x8f4;
const B_START_PHASE: u32 = 0x8f8;
const B_SEQ: u32 = 0x8fc;
const B_PICKED: u32 = 0xa0c;
const B_STOPPED: u32 = 0xa10;
const B_BAND: u32 = 0xa14;
const B_READY: u32 = 0xa28;
const B_LOOKING: u32 = 0xa2c;
const B_LOOK: u32 = 0xa30;
const B_LOOK_MIN: u32 = 0xa34;
const B_LOOK_MAX: u32 = 0xa38;

/// The request block's other half: the fields only the selection reads.
const R_AIRBORNE: u32 = 0xf8;
const R_BACKFLIP: u32 = 0xfc;
const R_NO_HANDER: u32 = 0x100;
const R_WRECK: u32 = 0x104;
const R_CUT: u32 = 0x140;
const R_COUNTDOWN: u32 = 0x144;
const R_THROTTLE: u32 = 0x160;
const R_REVERSING: u32 = 0x16c;
const R_PHASE_AXIS: u32 = 0x170;
const R_MIX_AXIS: u32 = 0x174;
const R_BAND: u32 = 0x178;
const R_GESTURE: u32 = 0x1a0;
const R_PROGRESS: u32 = 0x1a4;
const R_SEQ: u32 = 0x1a8;

// ---------------------------------------------------------------------------------------------------
// the oracle
// ---------------------------------------------------------------------------------------------------

/// Resolve names from `research/oracle.txt`, or say which one is missing and skip.
fn named(names: &[&str]) -> Option<Vec<u32>> {
    let mut out = Vec::new();
    for n in names {
        match addr(n) {
            Ok(a) => out.push(a),
            Err(why) => {
                skip(&why);
                return None;
            }
        }
    }
    Some(out)
}

/// A tiny xorshift, so every generated input is the same on every machine and a failure is
/// reproducible from the seed in this file alone.
struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }
    /// A float in `-span..=span`, with the exponent spread so small and large magnitudes both appear.
    fn float(&mut self, span: f32) -> f32 {
        let r = self.next();
        let unit = (r >> 8) as f32 / (1u32 << 24) as f32;
        let sign = if r & 1 == 0 { 1.0 } else { -1.0 };
        sign * span * unit
    }
    fn upto(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

/// Every state the selection can ask for, which is also every state the step's jump table has an arm
/// for. The ten the step leaves to its free-running advance — 24..=29, 31, 32, 36, 37 — are not in it,
/// and the last assertion of the first proof is that the machine never asks for one.
fn reachable(state: i32) -> bool {
    (0..SLOTS as i32).contains(&state)
        || state == TURN_RIDING
        || (REVERSE_L..=AIR).contains(&state)
        || (BACKFLIP..STATES as i32).contains(&state)
}

/// How the first proof steers the case: which arm of the chain is to run, and which branch of it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Arm {
    /// One of the twenty-four gesture slots.
    Gesture(i32),
    /// A gesture that is a no-hander instead.
    NoHander,
    Airborne,
    Backflip,
    /// The reverse pair, left or right.
    Reverse(bool),
    /// Off the bike, still falling.
    Flailing,
    /// Off the bike and settled, which draws a limb to hold.
    Settled,
    /// Standing still with the look timer running, about to fire, or run out and re-arming.
    Looking(u8),
    /// Riding: moving, or holding a look idle that has just ended.
    Riding(bool),
    /// The gate's five branches, in the order they happen.
    Gate(u8),
    /// Nothing steered at all.
    Anything,
}

/// The whole of what one frame of the selection decided, read the same way off both sides.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Decision {
    want: i32,
    ready: u32,
    picked: bool,
    look: u32,
    looking: bool,
    finished: bool,
    cut: bool,
    start_phase: u32,
    seq: u8,
    pool: [i32; IDLES as usize],
    count: usize,
    rng: u64,
}

#[test]
fn the_selection_matches_over_thirty_thousand_frames() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&[
        "rider_animation",
        "vehicle_animation",
        "srand",
        "rand_state",
        "rider_idle_pool",
        "rider_idle_count",
        "rand_seeded",
    ]) else {
        return;
    };
    let (select, step, srand) = (at[0], at[1], at[2]);
    let (rand_state, pool_at, count_at, seeded_at) = (at[3], at[4], at[5], at[6]);

    // The step behind the selection, which S25 proved on its own and which this proof wants nothing
    // from: a hook returns to the caller, so the step becomes the no-op the selection calls and the
    // two fields the tail latches afterwards are still written.
    ee.hooks.insert(step, Box::new(|_| {}));

    let mut pick = Pick(0x243f_6a88_85a3_08d3);
    let mut per_want = [0u32; STATES];
    let mut per_arm = [0u32; 12];
    let (mut draws, mut gate_runs, mut look_fires, mut rearms, mut cuts_cleared) = (0, 0, 0, 0, 0);
    for k in 0..30_000u32 {
        // One arm of the chain per case, in turn, so that every state the machine can ask for is
        // asked for by the branch that asks for it rather than by chance.
        let arm = match k % 42 {
            s @ 0..=23 => Arm::Gesture(s as i32),
            24 => Arm::NoHander,
            25 => Arm::Airborne,
            26 => Arm::Backflip,
            27 => Arm::Reverse(false),
            28 => Arm::Reverse(true),
            29 => Arm::Flailing,
            30 => Arm::Settled,
            31 => Arm::Looking(0),
            32 => Arm::Looking(1),
            33 => Arm::Looking(2),
            34 => Arm::Riding(false),
            35 => Arm::Riding(true),
            36 => Arm::Gate(0),
            37 => Arm::Gate(1),
            38 => Arm::Gate(2),
            39 => Arm::Gate(3),
            40 => Arm::Gate(4),
            _ => Arm::Anything,
        };
        per_arm[match arm {
            Arm::Gesture(_) => 0,
            Arm::NoHander => 1,
            Arm::Airborne => 2,
            Arm::Backflip => 3,
            Arm::Reverse(_) => 4,
            Arm::Flailing => 5,
            Arm::Settled => 6,
            Arm::Looking(_) => 7,
            Arm::Riding(_) => 8,
            Arm::Gate(_) => 9,
            Arm::Anything => 10,
        }] += 1;

        let dt = match pick.upto(4) {
            0 => 1.0 / 60.0,
            1 => 1.0 / 30.0,
            2 => 0.0,
            _ => pick.float(0.2).abs(),
        };
        // The state the selection asked for last, which three of its branches read back: the wreck's
        // "am I already holding a limb", the gate's "have I asked for the breathe", and every branch
        // that leaves the field alone.
        let want0 = match (arm, pick.upto(6)) {
            (Arm::Settled, 0 | 1) => HOLD_LIMB + pick.upto(HOLD_LIMBS as u32) as i32,
            (Arm::Gate(2 | 3), 0 | 1 | 2) => GET_READY,
            (_, 0) => pick.upto(STATES as u32) as i32,
            _ => {
                // Out of the reachable set, which is what a live body always has in it.
                let all: Vec<i32> = (0..STATES as i32).filter(|&s| reachable(s)).collect();
                all[pick.upto(all.len() as u32) as usize]
            }
        };

        // The body's own clocks and flags. Each arm pins the ones its branch is chosen by and the rest
        // are generated, so an arm reached with a clock in any state is still compared.
        let ready0 = match arm {
            Arm::Gate(0) => pick.float(4.0).abs().max(1e-6),
            Arm::Gate(_) => -pick.float(2.0).abs(),
            _ => pick.float(4.0),
        };
        let picked0 = match arm {
            Arm::Gate(1) => false,
            Arm::Gate(_) => true,
            _ => pick.upto(2) == 1,
        };
        let look0 = match arm {
            // Counting down and not about to run out, versus about to, versus run out already.
            Arm::Looking(0) => dt + pick.float(10.0).abs() + 1.0,
            Arm::Looking(1) => dt * 0.5,
            Arm::Looking(2) => -pick.float(2.0).abs(),
            _ => pick.float(20.0),
        };
        let looking0 = match arm {
            Arm::Riding(true) | Arm::Looking(_) => true,
            Arm::Riding(false) => false,
            _ => pick.upto(2) == 1,
        };
        let finished0 = match arm {
            Arm::Gate(4) => true,
            Arm::Gate(_) => false,
            Arm::Riding(true) => true,
            _ => pick.upto(2) == 1,
        };
        let settled0 = match arm {
            Arm::Flailing => false,
            Arm::Settled => true,
            _ => pick.upto(2) == 1,
        };

        // The body's numbers, as the actor is built: the standing-still limit, the two look bounds and
        // the ready rate. Sometimes generated, so a disc that configures them differently is covered.
        let (stopped, look_min, look_max, rate_ready) = match pick.upto(3) {
            0 => (1.466_666_7, 15.0, 45.0, pick.float(5.0).abs()),
            1 => (pick.float(5.0).abs(), pick.float(30.0), pick.float(60.0), pick.float(5.0)),
            _ => (1.466_666_7, pick.float(20.0).abs(), pick.float(60.0).abs(), 0.0),
        };
        // The pose grid's first threshold, which doubles as the speed that cancels the look timer.
        let band0 = match pick.upto(4) {
            0 => 7.333_333,
            1 => 0.0,
            _ => pick.float(10.0),
        };

        // The request. Each arm pins the flags that select it; the rest are generated.
        let cut = matches!(arm, Arm::Gate(_)) || (arm == Arm::Anything && pick.upto(3) == 0);
        let wreck = match arm {
            Arm::Flailing | Arm::Settled => true,
            Arm::Gate(_) => pick.upto(2) == 1,
            Arm::Anything => pick.upto(3) == 0,
            _ => false,
        };
        let gesture = match arm {
            Arm::Gesture(s) => s,
            Arm::NoHander => pick.upto(SLOTS as u32) as i32,
            Arm::Anything => pick.upto(STATES as u32) as i32,
            _ => SLOTS as i32,
        };
        let no_hander = match arm {
            Arm::NoHander => true,
            Arm::Gesture(_) => false,
            _ => pick.upto(2) == 1,
        };
        let backflip = match arm {
            Arm::Backflip => true,
            Arm::Airborne | Arm::Reverse(_) | Arm::Looking(_) | Arm::Riding(_) => false,
            _ => pick.upto(2) == 1,
        };
        let airborne = match arm {
            Arm::Airborne => true,
            Arm::Reverse(_) | Arm::Looking(_) | Arm::Riding(_) => false,
            _ => pick.upto(2) == 1,
        };
        let reversing = match arm {
            Arm::Reverse(_) => pick.float(5.0) + 1.0,
            Arm::Looking(_) | Arm::Riding(_) => 0.0,
            _ => {
                if pick.upto(2) == 1 {
                    0.0
                } else {
                    pick.float(5.0)
                }
            }
        };
        // The two axes. Centred is what the look timer needs; which side of centre is what picks the
        // half of the reverse pair.
        let axis = |pick: &mut Pick| match pick.upto(4) {
            0 => 0.5,
            1 => 0.499_999_97,
            2 => 0.500_000_03,
            _ => pick.float(1.5),
        };
        let phase_axis = match arm {
            Arm::Reverse(false) => 0.5 - pick.float(1.0).abs() - 1e-6,
            Arm::Reverse(true) => 0.5 + pick.float(1.0).abs(),
            Arm::Looking(_) => 0.5,
            _ => axis(&mut pick),
        };
        let mix_axis = match arm {
            Arm::Looking(_) => 0.5,
            _ => axis(&mut pick),
        };
        let throttle = match arm {
            Arm::Looking(_) => 0.0,
            Arm::Gate(3) => pick.float(1.0) + 1.0,
            _ => {
                if pick.upto(2) == 1 {
                    0.0
                } else {
                    pick.float(1.0)
                }
            }
        };
        // The number the pose grid cuts up. The look timer needs it inside the standing-still limit,
        // and anything above the grid's first threshold cancels the timer whatever else happened.
        let band = match arm {
            Arm::Looking(_) => stopped * 0.5 * if pick.upto(2) == 0 { 1.0 } else { -1.0 },
            Arm::Riding(_) => stopped + 1.0 + pick.float(5.0).abs(),
            _ => match pick.upto(4) {
                0 => 0.0,
                1 => band0,
                _ => pick.float(20.0),
            },
        };
        let countdown = match pick.upto(3) {
            0 => 0.0,
            1 => 3.0,
            _ => pick.float(5.0),
        };
        let progress = pick.float(1.0).abs();
        let seq = pick.upto(400) as i32;

        // The no-repeat list, from empty to full, with distinct entries so that a redraw is a redraw.
        let count0 = match pick.upto(4) {
            0 => 0usize,
            1 => IDLES as usize,
            _ => pick.upto(IDLES as u32) as usize,
        };
        let mut pool0 = [0i32; IDLES as usize];
        {
            let mut taken: Vec<i32> = (IDLE..IDLE + IDLES).collect();
            for slot in pool0.iter_mut().take(count0) {
                let i = pick.upto(taken.len() as u32) as usize;
                *slot = taken.remove(i);
            }
            // Past the count the list is whatever was left there, which the scan must not look at.
            for slot in pool0.iter_mut().skip(count0) {
                *slot = IDLE + pick.upto(IDLES as u32) as i32;
            }
        }
        let seed = pick.next();

        ee.load(BODY, &vec![0u8; 0xb00]);
        ee.load(REQ, &vec![0u8; 0x200]);
        ee.set_u32(BODY + B_REQUEST, REQ);
        ee.set_u32(BODY + B_FINISHED, u32::from(finished0));
        ee.set_f32(BODY + B_RATE_READY, rate_ready);
        ee.set_u32(BODY + B_SETTLED, u32::from(settled0));
        ee.set_u32(BODY + B_WANT, want0 as u32);
        ee.set_u32(BODY + B_PICKED, u32::from(picked0));
        ee.set_f32(BODY + B_STOPPED, stopped);
        ee.set_f32(BODY + B_BAND, band0);
        ee.set_f32(BODY + B_READY, ready0);
        ee.set_u32(BODY + B_LOOKING, u32::from(looking0));
        ee.set_f32(BODY + B_LOOK, look0);
        ee.set_f32(BODY + B_LOOK_MIN, look_min);
        ee.set_f32(BODY + B_LOOK_MAX, look_max);
        ee.set_u32(REQ + R_AIRBORNE, u32::from(airborne));
        ee.set_u32(REQ + R_BACKFLIP, u32::from(backflip));
        ee.set_u32(REQ + R_NO_HANDER, u32::from(no_hander));
        ee.set_u32(REQ + R_WRECK, u32::from(wreck));
        ee.set_u32(REQ + R_CUT, u32::from(cut));
        ee.set_f32(REQ + R_COUNTDOWN, countdown);
        ee.set_f32(REQ + R_THROTTLE, throttle);
        ee.set_f32(REQ + R_REVERSING, reversing);
        ee.set_f32(REQ + R_PHASE_AXIS, phase_axis);
        ee.set_f32(REQ + R_MIX_AXIS, mix_axis);
        ee.set_f32(REQ + R_BAND, band);
        ee.set_u32(REQ + R_GESTURE, gesture as u32);
        ee.set_f32(REQ + R_PROGRESS, progress);
        ee.set_u32(REQ + R_SEQ, seq as u32);
        for (i, &v) in pool0.iter().enumerate() {
            ee.set_u32(pool_at + 4 * i as u32, v as u32);
        }
        ee.set_u32(count_at, count0 as u32);
        // The generator, through the original's own `srand`, and the flag that says it has been
        // seeded — so the clock read behind every draw is skipped, which is the one thing in this
        // function a simulation cannot have.
        ee.call(srand, &[u64::from(seed)], &[], BUDGET);
        ee.set_u32(seeded_at, seed.max(1));

        ee.call(select, &[u64::from(BODY)], &[dt], BUDGET);

        let want_side = Decision {
            want: ee.u32_at(BODY + B_WANT) as i32,
            ready: ee.f32_at(BODY + B_READY).to_bits(),
            picked: ee.u32_at(BODY + B_PICKED) != 0,
            look: ee.f32_at(BODY + B_LOOK).to_bits(),
            looking: ee.u32_at(BODY + B_LOOKING) != 0,
            finished: ee.u32_at(BODY + B_FINISHED) != 0,
            cut: ee.u32_at(REQ + R_CUT) != 0,
            start_phase: ee.f32_at(BODY + B_START_PHASE).to_bits(),
            seq: (ee.read(BODY + B_SEQ, 1) & 0xff) as u8,
            pool: std::array::from_fn(|i| ee.u32_at(pool_at + 4 * i as u32) as i32),
            count: ee.u32_at(count_at) as usize,
            rng: ee.read(rand_state, 8) as u64,
        };

        let mut body = Body {
            veh: vehicle_anim::Driver { want: want0, ..Body::default().veh },
            player: Player { finished: finished0, ..Default::default() },
            ready: F(ready0),
            picked: picked0,
            look: F(look0),
            looking: looking0,
            settled: settled0,
            // The mode machine's own state, which this oracle does not drive: the selection reads
            // none of it (it reads `settled`, which the machine writes, and that is set above).
            machine: Default::default(),
        };
        let p = Params { stopped: F(stopped), look_min: F(look_min), look_max: F(look_max) };
        let mut vp = vehicle_anim::Params {
            rate_ready: F(rate_ready),
            band: [F(band0), F::ZERO, F::ZERO],
            ..Default::default()
        };
        let mut r = vehicle_anim::Request {
            cut,
            phase_axis: F(phase_axis),
            mix_axis: F(mix_axis),
            band: F(band),
            progress: F(progress),
            seq,
            ..Default::default()
        };
        let a = Ask {
            airborne,
            backflip,
            no_hander,
            wreck,
            countdown: F(countdown),
            throttle: F(throttle),
            reversing: F(reversing),
            gesture,
            spin: None,
        };
        let mut pool =
            Pool { used: pool0, count: count0, rng: Rng::from_seed(seed), seeded: true };
        body.select(F(dt), &p, &vp, &mut r, &a, &mut pool, Clock::default());
        body.latch(&mut vp, &r);
        let got_side = Decision {
            want: body.veh.want,
            ready: body.ready.to_bits(),
            picked: body.picked,
            look: body.look.to_bits(),
            looking: body.looking,
            finished: body.player.finished,
            cut: r.cut,
            start_phase: vp.start_phase.to_bits(),
            seq: body.veh.seq,
            pool: pool.used,
            count: pool.count,
            rng: pool.rng.state(),
        };

        let why = format!(
            "case {k} ({arm:?}): want {want0} dt {dt:e} ready {ready0:e} picked {picked0} \
             look {look0:e} looking {looking0} finished {finished0} settled {settled0} \
             cut {cut} wreck {wreck} gesture {gesture} no_hander {no_hander} \
             backflip {backflip} airborne {airborne} reversing {reversing:e} \
             axes {phase_axis:e}/{mix_axis:e} throttle {throttle:e} band {band:e} of {band0:e} \
             stopped {stopped:e} look {look_min:e}..{look_max:e} ready_rate {rate_ready:e} \
             countdown {countdown:e} pool {count0} seed {seed}"
        );
        assert_eq!(got_side, want_side, "{why}");

        // The structural claim the whole module rests on: the selection only ever asks for a state the
        // step has an arm for. Two kinds of case are excluded, and both exclusions are findings: one
        // that starts from an unreachable state, because those are the cases that test "leave the
        // field alone"; and one whose request names a gesture slot that is not a gesture slot,
        // because the gesture arm copies the request's id into the state with no range test at all.
        if reachable(want0) && gesture <= SLOTS as i32 {
            assert!(
                reachable(got_side.want),
                "{why}: asked for {}, which the step has no arm for",
                got_side.want
            );
            per_want[got_side.want as usize] += 1;
        }
        if got_side.rng != Rng::from_seed(seed).state() {
            draws += 1;
        }
        if cut {
            gate_runs += 1;
        }
        if !cut && got_side.want >= IDLE && got_side.want < IDLE + IDLES && !wreck {
            look_fires += 1;
        }
        if got_side.look != look0.to_bits() && !cut {
            rearms += 1;
        }
        if cut && !got_side.cut {
            cuts_cleared += 1;
        }
    }

    // Every state the machine can ask for, asked for; and none of the ten it cannot.
    for s in 0..STATES {
        let n = per_want[s];
        if reachable(s as i32) {
            assert!(n > 50, "state {s} was asked for only {n} times");
        } else {
            assert_eq!(n, 0, "state {s} has no arm in the step and was asked for {n} times");
        }
    }
    assert!(draws > 3_000, "only {draws} frames drew a random number");
    assert!(cuts_cleared > 50, "the gate's last step ran only {cuts_cleared} times");
    assert!(look_fires > 50, "the look timer fired only {look_fires} times");
    assert!(rearms > 500, "the look timer was touched only {rearms} times");
    println!(
        "the selection: 30,000 frames bit-exact. {draws} with a random draw, {gate_runs} at the \
         gate ({cuts_cleared} of them ending it), {look_fires} look-arounds fired, {rearms} timer \
         writes. Per arm: {per_arm:?}"
    );
}

// ---------------------------------------------------------------------------------------------------
// what the task asked for in words
// ---------------------------------------------------------------------------------------------------

/// A clip handle per animation state, and the state behind one.
fn handle(state: i32) -> ClipId {
    ClipId(0x100 + state as u32)
}
fn state_of(id: ClipId) -> i32 {
    (id.0 - 0x100) as i32
}

/// A clip table with every state bound, and a clip behind each handle.
///
/// The durations and the loop flags are this test's, not the disc's: one second each, and looping for
/// the clips a body is meant to *sit* in — the riding rows, the flail, `get_ready_breathe` (which is
/// what makes the gate's wait a wait rather than a clip that ends) and the three start poses (a body
/// whose start pose ends goes back to breathing and waits again, which is the machine working as
/// written and not what a start line looks like). Which clips of a rider's own set really loop is a
/// question for the disc; see the journal's last section.
fn table() -> (Library, impl Fn(ClipId) -> Clip) {
    let mut lib = Library::default();
    for s in 0..STATES {
        lib.clip[s] = Some(handle(s as i32));
    }
    let clips = |id: ClipId| {
        let s = state_of(id);
        let looping = (24..=37).contains(&s)
            || s == HOLD
            || s == GET_READY
            || (START_POS..STATES as i32).contains(&s);
        Clip::new(F(1.0), looping)
    };
    (lib, clips)
}

/// The body's numbers as the actor is built, and the step's as a `.phy` leaves them.
fn numbers() -> (Params, vehicle_anim::Params) {
    let p = Params::built();
    let vp = vehicle_anim::Params {
        rate_stunt: F(3.0),
        rate_default: F(3.0),
        rate_pose: F(3.0),
        rate_hold: F(3.0),
        rate_ready: F(1.0),
        cross_cap: F(0.25),
        start_phase: F::ZERO,
        band: [F(7.333_333), F(14.666_666), F(22.0)],
    };
    (p, vp)
}

/// A request for a body sitting on a bike doing nothing: the bars centred, the throttle shut, still.
fn resting() -> (vehicle_anim::Request, Ask) {
    let r = vehicle_anim::Request {
        held: false,
        at_base: true,
        cut: false,
        offset: Vf::default(),
        base: Vf::default(),
        rotation: Vf::new(0.0, 0.0, 0.0, 1.0),
        phase_axis: F(0.5),
        mix_axis: F(0.5),
        band: F::ZERO,
        progress: F::ZERO,
        seq: 0,
    };
    let a = Ask { gesture: SLOTS as i32, ..Default::default() };
    (r, a)
}

#[test]
fn a_body_dropped_on_a_bike_gets_ready_without_being_asked() {
    let (lib, clips) = table();
    let (p, mut vp) = numbers();
    let (mut r, mut a) = resting();
    // The start line: the gate flag set, and the request's own clock at zero so the breathe-and-wait
    // is the full three and a half seconds less the ready rate.
    r.cut = true;
    a.countdown = F::ZERO;

    // The body as the actor is built: asking for the riding pose, with nothing playing.
    let mut body = Body::default();
    let mut pool = Pool::default();
    let clock = Clock { second: 0x21, minute: 0x15, hour: 0x12, day: 0x03 };
    let dt = F(1.0) / F(60.0);

    // The whole claim of "without the vehicle asking for anything" is this: between one frame and the
    // next, nothing writes the state the body wants except the selection itself.
    let mut asked: Vec<i32> = Vec::new();
    let mut last = body.veh.want;
    let push = |asked: &mut Vec<i32>, w: i32| {
        if asked.last() != Some(&w) {
            asked.push(w);
        }
    };
    push(&mut asked, last);
    for frame in 0..600 {
        assert_eq!(body.veh.want, last, "frame {frame}: something else wrote the state");
        body.frame(dt, &p, &mut vp, &mut r, &a, &lib, &mut pool, clock, false, &clips);
        last = body.veh.want;
        push(&mut asked, last);
        // The clip the body is in is always the state it asked for, which is the step's job and is
        // what makes the sequence below a sequence of clips and not of numbers.
        if let Some(on) = body.player.clip {
            assert_eq!(state_of(on), body.veh.state, "frame {frame}: the wrong clip is playing");
        }
    }
    // Ten seconds of standing at the gate: a get-ready idle, then the breathe, then a start pose. No
    // other state is asked for, and the vehicle asked for none of them.
    assert_eq!(asked.len(), 4, "the gate asked for {asked:?}");
    assert_eq!(asked[0], TURN_RIDING, "the body starts in the pose the actor is built with");
    assert!(
        (IDLE..IDLE + IDLES).contains(&asked[1]),
        "the gate opens with one of the ten get-ready idles, not {}",
        asked[1]
    );
    assert_eq!(asked[2], GET_READY, "the idle is followed by the breathe");
    assert!(
        (START_POS..STATES as i32).contains(&asked[3]),
        "the breathe is followed by a start pose, not {}",
        asked[3]
    );
    // And the gate's only way out: the throttle.
    a.throttle = F(1.0);
    body.frame(dt, &p, &mut vp, &mut r, &a, &lib, &mut pool, clock, false, &clips);
    assert!(!r.cut, "opening the throttle ends the start sequence");
    assert_eq!(body.veh.want, TURN_RIDING, "and the body rides");
    println!("the gate: {asked:?} then riding, in 600 frames, with nothing else asking");
}

#[test]
fn a_wreck_puts_the_body_into_a_wreck_clip() {
    let (lib, clips) = table();
    let (p, mut vp) = numbers();
    let (mut r, mut a) = resting();
    let mut body = Body::default();
    let mut pool = Pool::default();
    let clock = Clock { second: 0x07, minute: 0x30, hour: 0x09, day: 0x11 };
    let dt = F(1.0) / F(60.0);

    // Riding, flat out, so the look timer is nowhere near firing.
    a.throttle = F(1.0);
    r.band = F(20.0);
    for _ in 0..30 {
        body.frame(dt, &p, &mut vp, &mut r, &a, &lib, &mut pool, clock, false, &clips);
    }
    assert_eq!(body.veh.want, TURN_RIDING, "it is riding");
    assert_eq!(body.player.clip, None, "the riding rows are phase-driven, not free-running");

    // Off the bike. The body flails for as long as it is in the air.
    a.wreck = true;
    for frame in 0..60 {
        body.frame(dt, &p, &mut vp, &mut r, &a, &lib, &mut pool, clock, false, &clips);
        assert_eq!(body.veh.want, HOLD, "frame {frame}: a wrecked body flails");
        assert_eq!(body.player.clip, Some(handle(HOLD)), "frame {frame}: in the flail clip");
    }

    // Settled: it grabs one of five limbs and keeps the one it grabbed.
    body.settled = true;
    body.frame(dt, &p, &mut vp, &mut r, &a, &lib, &mut pool, clock, false, &clips);
    let held = body.veh.want;
    assert!(
        (HOLD_LIMB..HOLD_LIMB + HOLD_LIMBS).contains(&held),
        "a settled body holds one of its five limbs, not {held}"
    );
    for frame in 0..120 {
        body.frame(dt, &p, &mut vp, &mut r, &a, &lib, &mut pool, clock, false, &clips);
        assert_eq!(body.veh.want, held, "frame {frame}: it keeps the limb it grabbed");
        assert_eq!(body.player.clip, Some(handle(held)), "frame {frame}: in that clip");
    }
    assert_ne!(body.veh.want, TURN_RIDING, "a wrecked body is not riding");
    println!("the wreck: flailed for 60 frames, then held state {held} for 120");
}

/// The hand-written bike the rest of this crate's tests ride, which is not the disc's — the body's
/// animation reads three of its fields (the speed, and the two pose axes the header writes) and none
/// of them is a number this test asserts on.
const BIKE: &str = include_str!("freeride-bike.phy.txt");

#[test]
fn the_rider_publishes_the_body_its_own_request() {
    let tables = rider::Tables::build();
    let p = Phy::read_bytes(BIKE.as_bytes(), &tables.ride.curve);
    // The two hubs `stand.rs` puts on this bike: a tire radius up and half a wheelbase out, because a
    // test with no bike model has to put them somewhere.
    let mounts = (
        Vf::new(0.0, p.front.tire_radius.0, 2.1, 1.0),
        Vf::new(0.0, p.rear.tire_radius.0, -2.1, 1.0),
    );
    let at = Vf::new(0.0, 1.0, 0.0, 1.0);
    // An empty stunt table: no gesture this rider can make names a slot, which is the point — the
    // body is to get itself ready with nothing asking it to.
    let mut rider = Rider::new(&p, mounts, at, Vf::new(0.0, 0.0, 1.0, 0.0), Set::new(Vec::new()));
    // The one thing `Rider::new` cannot fill, and says so: the step's fade rates and the pose grid's
    // thresholds, which no loader in the image has been found for. A shell would set these.
    rider.body_rates = numbers().1;
    // Standing at the gate, not down: the bike settling onto the floor from its spawn height is a
    // wreck otherwise (reason 8 on the first tick), which until S91 put the body straight into its
    // wreck mode and past the gate mode's entry.
    rider.sense.wrecks_enabled = false;

    let (lib, clips) = table();
    let mut floor = Flat::default();
    let mut hands = Controls::default();
    let gate = rider::Gate { countdown: F::ZERO, clock: Clock { second: 0x44, minute: 0x02, hour: 0x11, day: 0x07 } };
    let dt = F(1.0) / F(60.0);

    let mut asked: Vec<i32> = vec![rider.body.veh.want];
    for frame in 0..600 {
        let before = rider.body.veh.want;
        // The player's own update: the physics, the pad, the trick machine, the clocks, the state
        // machine. None of it is the body's, and this is the assertion that says so.
        rider.frame(&p, &tables, &hands, dt, &mut floor);
        assert_eq!(rider.body.veh.want, before, "frame {frame}: the player update wrote the body");
        rider.body_frame(dt, &hands, gate, &lib, &clips);
        assert_eq!(rider.body.machine.mode, game_sim::rider_anim::Mode::Gate, "frame {frame}: the body is at the gate");
        if asked.last() != Some(&rider.body.veh.want) {
            asked.push(rider.body.veh.want);
        }
    }
    // The same states as (2), reached off a real rider's fields instead of a hand-made request, behind
    // the breathe the mode machine's gate entry starts (up to half a second), which (2) has no mode
    // machine to run.
    assert_eq!(asked.len(), 5, "the gate asked for {asked:?}");
    assert_eq!(asked[0], TURN_RIDING);
    assert_eq!(asked[1], GET_READY, "the gate entry's breathe");
    assert!((IDLE..IDLE + IDLES).contains(&asked[2]), "a get-ready idle, not {}", asked[2]);
    assert_eq!(asked[3], GET_READY);
    assert!((START_POS..STATES as i32).contains(&asked[4]), "a start pose, not {}", asked[4]);
    // The published half of the request, off the rider: the bars and the rider's weight as the
    // state pack latched them in front of the header, the contact velocity's forward lane (S112), and
    // the trick clocks' progress and count.
    let r = rider.body_request;
    assert_eq!(r.phase_axis, rider.packed_pose, "the phase axis is the packed pose");
    assert_eq!(r.mix_axis, rider.packed_fore_aft, "the mix axis is the packed fore-aft");
    assert_eq!(r.band.0.to_bits(), rider.ride.air.contact_vel.z().to_bits(), "the band is the contact velocity's z");
    assert_eq!(r.progress, rider.tricks.progress, "the progress is the trick clocks'");
    assert_eq!(r.seq, rider.tricks.seq, "and so is the count");
    assert_eq!(rider.body_rates.start_phase, r.progress, "which the tail latches back");

    // The gate's only way out, through the pad this time: the throttle.
    hands.gas = F(1.0);
    rider.frame(&p, &tables, &hands, dt, &mut floor);
    rider.body_frame(dt, &hands, gate, &lib, &clips);
    assert!(!rider.body_request.cut, "opening the throttle ends the start sequence");
    assert_eq!(rider.body.veh.want, TURN_RIDING, "and the rider rides");
    println!(
        "the wiring: {asked:?} then riding, through a whole rider on the flat, band {:?}",
        rider.body_request.band
    );
}

/// P159: a front flip's rider is the airborne lean held full forward, not the backflip's clip; a
/// backflip — on MX vs. ATV's rows or the original's — keeps it. The bike is put in the air and
/// turning by hand: what is under test is what the body asks for off the ride's flags, and the frame
/// asks for it from the flip's first tick, whatever the stick's lean was.
#[test]
fn a_front_flip_leans_forward_and_a_backflip_keeps_its_clip() {
    use game_sim::vehicle_anim::Blend;
    let tables = rider::Tables::build();
    let p = Phy::read_bytes(BIKE.as_bytes(), &tables.ride.curve);
    let mounts = (
        Vf::new(0.0, p.front.tire_radius.0, 2.1, 1.0),
        Vf::new(0.0, p.rear.tire_radius.0, -2.1, 1.0),
    );
    let (lib, clips) = table();
    let gate = rider::Gate { countdown: F::ZERO, clock: Clock::default() };
    let dt = F(1.0) / F(60.0);
    // `row`: which of MX vs. ATV's rotations is turning, or `None` for a ride on the original's.
    let flip = |row: Option<usize>| {
        let mut rider = Rider::new(&p, mounts, Vf::new(0.0, 1.0, 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), Set::new(Vec::new()));
        rider.body_rates = numbers().1;
        rider.body_request.cut = false;
        rider.ride.tricks = row.map(|r| Box::new(game_sim::trick_pack::State { flip_row: r, ..Default::default() }));
        rider.ride.air.airborne = true;
        rider.ride.air.flipping = true;
        // Leaning back off the lip, as a flip is fired: the lean the front flip must not show.
        rider.motion.fore_aft = F(0.9);
        let hands = Controls::default();
        (0..40).map(|_| (rider.body_frame(dt, &hands, gate, &lib, &clips), rider.body.veh.want, rider.body.veh.mix.value)).collect::<Vec<_>>()
    };
    let front = flip(Some(1));
    for (k, (frame, want, mix)) in front.iter().enumerate() {
        assert_eq!(*want, AIR, "tick {k}: the front flip asks for the airborne lean, not BACKFLIP");
        assert_eq!(*mix, game_sim::rider_anim::FULL_FORWARD, "tick {k}: held full forward");
        match frame.blend {
            Blend::Two { mix, a, b, .. } => {
                assert_eq!((a, b, mix), (Some(handle(AIR + 1)), Some(handle(AIR)), F::ZERO), "tick {k}: air_Fwd alone");
            }
            other => panic!("tick {k}: the airborne pair, not {other:?}"),
        }
    }
    for (name, row) in [("MX vs. ATV's backflip", Some(0)), ("the original's backflip", None)] {
        for (k, (frame, want, _)) in flip(row).iter().enumerate() {
            assert_eq!(*want, BACKFLIP, "{name}, tick {k}");
            assert!(matches!(frame.blend, Blend::One { clip, phase, .. } if clip == Some(handle(BACKFLIP)) && phase == F(0.1)), "{name}, tick {k}: {:?}", frame.blend);
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// 5. a wreck that settles
// ---------------------------------------------------------------------------------------------------

/// The wreck limits a clean drop onto flat ground is judged against, appended to [`BIKE`]: sideways
/// and fore-and-aft are out of reach and both pitch limits are generous, so the only wreck this drop
/// can get is the vertical one, and `MaxWreckTime` is how long the settled wreck lies there before the
/// machine puts the rider back. The same block `tests/ride.rs` wrecks on, and like it these are
/// plausible numbers and not the disc's.
const WRECK_LIMITS: &str = "
[Wrecks]
MaxWreckTime = 2
CannonballWreckTime = 5
FrontWreckSpeedLimit = 400
BackWreckSpeedLimit = 400
SideWreckSpeedLimit = 400
RacingDownWreckSpeedLimit = 400
FreestyleDownWreckSpeedLimit = 5
GroundPitchDownWreckAngle = 85
GroundPitchUpWreckAngle = 85
AirbornePitchDownWreckAngle = 175
AirbornePitchUpWreckAngle = 175
AirborneRollWreckAngle = 175
NoHanderWreckSpeedLimit = 400
NoHanderMaxRollAngle = 45
JoustWreckHeight = 2
CrossupWreck = 1
CrossupWreckTime = 0.3
";

/// The bike the rider leaves behind: one rigid-body part in the shape of the game's own
/// `Wreckbike.phy`. `tests/ride.rs`'s, not the disc's.
const WRECK_PART: &str = "
[PhysicsPart1]
Type = RigidBody
Name = wreck_physics
Weight = 250.0
CollisionWeight = 0.0001
GravityScale = 1.0
InertiaScale = 1.0, 1.0, 1.0
SpringStiffness = 5.0
SpringDamping = 0.4
MaxPenetration = 0.5
CofG = 0.0, 1.5, 0.0
SpringMomentScale = 1.0, 1.0, 1.0
FrictionMomentScale = 1.0, 1.0, 1.0
FrictionZone1 = 0.5
Radius = 1.0
AccelerationThreshold = 0.01
AngularAccelerationThreshold = 0.01
";

/// The rider's own half of a wreck, in the shape of the rider file's `[Wrecks]`. The three numbers
/// the mode machine reads are the last three: a body is thrown about while it is rising faster than
/// `RiderFlailHeight`, and its roll ends after `RiderRollTime` **or** below `RiderRollSpeed`. With the
/// speed limit at zero the only way out is the clock, which is what makes the tick the settle lands on
/// a number this test can predict. `tests/ride.rs`'s block, kept the same so the two measure the same
/// wreck.
const RIDER_WRECKS: &str = "
[Wrecks]
HitBottomForceTime = 0.1
HitBottomHandHoldTime = 0.3
HitBottomForce = 0.0, 800.0, 0.0
GroundAngularRateDamping = 1.0
RiderFlailHeight = 3.0
RiderRollTime = 0.75
RiderRollSpeed = 0.0
";

/// Reason 5 as the state machine carries it: landing too hard, which is the only wreck the limits
/// above leave this drop, and the one the human's capture hit twice.
const FLAT: game_sim::trick::Wreck = game_sim::trick::Wreck(game_sim::air::Wreck::Flat as u8);

/// The box the wreck body's inertia is built out of, and how high the bike is dropped from, in feet —
/// far enough that it comes down past the vertical wreck limit and the rider goes over the bars.
const WRECK_BOX: Vf = Vf([2.5, 4.0, 7.0, 1.0]);
const DROP: f32 = 30.0;

/// The frame this one steps at: the human recording's own `dt`, which is what the wreck's tick counts
/// in `tests/ride.rs` are counted in.
const WRECK_DT: F = F(0.03333);

/// `RiderRollTime` in ticks of [`WRECK_DT`]: how many frames of roll the limit takes to pass.
/// 23 × 0.03333 = 0.766 > 0.75, and 22 × 0.03333 = 0.733 is not. The tick the body is first seen on
/// the ground has already had one of them — the rider point is stepped by the rider's own frame,
/// which runs in front of the body's — so the settle lands `ROLL_TICKS - 1` ticks after that one.
const ROLL_TICKS: usize = 23;

/// **V19.** A wrecked body settles by itself, and a settled body holds one limb and keeps it.
///
/// The whole point is what is *not* in the loop: nothing writes [`Body::settled`]. The wreck arm of
/// the selection used to be unreachable past its first state because the only writer of that flag was
/// the body's mode machine, which was not ported — so this drops a whole [`Rider`] with wreck files
/// under it, wrecks it on the floor, and lets [`Rider::body_frame`] run both of the body actor's slots
/// per frame. What the body does is then the machine's own doing: it flails while it is being thrown
/// about, it settles the tick its roll runs past `RiderRollTime`, it grabs one of five limbs and stays
/// on that one, and when the rider is put back the settle is cleared again.
#[test]
fn a_wrecked_body_settles_and_then_holds_still() {
    use game_sim::rider_anim::Mode;
    let tables = rider::Tables::build();
    let p = Phy::read_bytes(format!("{BIKE}{WRECK_LIMITS}").as_bytes(), &tables.ride.curve);
    let hubs = game_sim::stock::MOUNTS;
    let at = Vf::new(0.0, DROP, 0.0, 1.0);
    let mut rider =
        Rider::new(&p, hubs, at, Vf::new(0.0, 0.0, 1.0, 0.0), Set::default());
    // The rider's wreck files: the bike it leaves behind and the throw it is thrown with. Without
    // these the wreck has no numbers and the machine is handed nothing to settle on.
    let part = phy::Part::read_bytes(WRECK_PART.as_bytes(), "PhysicsPart1");
    let wrecks = phy::Wrecks::read_bytes(RIDER_WRECKS.as_bytes());
    rider.carries_wreck(part, wrecks, part.inertia(WRECK_BOX));
    rider.body_rates = numbers().1;
    // Not at a gate: this rider is dropped onto a field, so the selection's start-line arm is not to
    // outrank the wreck. (The mode machine puts the wreck above the gate either way; the selection
    // does not, and it is the selection that picks the clip.)
    rider.body_request.cut = false;

    let (lib, clips) = table();
    // The dirt, the same plane `tests/ride.rs` wrecks on.
    let mut world = Flat { y: F::ZERO, normal: Vf::new(0.0, 1.0, 0.0, 0.0) };
    let hands = Controls::default();
    let gate = rider::Gate {
        countdown: F::ZERO,
        clock: Clock { second: 0x19, minute: 0x27, hour: 0x14, day: 0x17 },
    };

    // The three ticks this test is about, all counted in frames of the loop.
    let mut reason = None;
    let mut wrecked_at = None;
    let mut grounded_at = None;
    let mut settled_at = None;
    let mut back_at = None;
    // The limb the settled body grabbed, and the state it flailed in before it did.
    let mut held = None;
    for k in 0..1200 {
        let f = rider.frame(&p, &tables, &hands, WRECK_DT, &mut world);
        // Nobody but the machine writes the flag: the rider's own six-step update does not, and the
        // test itself never assigns it.
        let before = rider.body.settled;
        rider.body_frame(WRECK_DT, &hands, gate, &lib, &clips);
        if let Some(w) = f.wreck {
            if wrecked_at.is_none() {
                wrecked_at = Some(k);
                reason = Some(w);
            }
        }
        if wrecked_at.is_none() {
            continue;
        }
        // The body sees the wreck a tick late, as the original's does (S91): its request is filled
        // before the machine's verdict, so on the entry tick it is still riding.
        if wrecked_at == Some(k) {
            assert_ne!(rider.body.machine.mode, Mode::Wrecked, "tick {k}: the body saw the wreck's entry");
            continue;
        }
        if wrecked_at == Some(k - 1) {
            assert_eq!(rider.body.machine.mode, Mode::Wrecked, "tick {k}: the body entered its wreck late");
        }
        if f.respawned {
            back_at = Some(k);
            break;
        }
        // On the ground, as the port's stand-in sensor has it: the rider point has reached the dirt.
        if rider.crash.as_ref().is_some_and(|c| c.landed) && grounded_at.is_none() {
            grounded_at = Some(k);
        }
        match (rider.body.settled, settled_at) {
            (true, None) => {
                settled_at = Some(k);
                held = Some(rider.body.veh.want);
            }
            // Once settled, it stays settled for the rest of the wreck, and the limb does not change.
            (settled, Some(_)) => {
                assert!(settled, "tick {k}: the settle was dropped mid-wreck");
                assert_eq!(rider.body.veh.want, held.unwrap(), "tick {k}: it changed limb");
                assert_eq!(
                    rider.body.player.clip,
                    Some(handle(held.unwrap())),
                    "tick {k}: and the clip with it"
                );
            }
            // Before it settles it flails, which is the one state the wreck arm could reach before
            // this task.
            (false, None) => {
                assert!(!before, "tick {k}: something cleared a settle nothing had written");
                assert_eq!(rider.body.veh.want, HOLD, "tick {k}: an unsettled wreck flails");
                assert_eq!(
                    rider.body.player.clip,
                    Some(handle(HOLD)),
                    "tick {k}: in the flail clip"
                );
            }
        }
    }

    let wrecked_at = wrecked_at.expect("the drop wrecked");
    assert_eq!(reason, Some(FLAT), "the drop wrecked for the wrong reason");
    let grounded_at = grounded_at.expect("the thrown rider reached the ground");
    let settled_at = settled_at.expect("the wrecked body settled, with nothing writing the flag");
    let back_at = back_at.expect("and the machine put the rider back by itself");
    // The settle is the roll's own clock running out, on the tick the numbers say.
    assert_eq!(
        settled_at - grounded_at,
        ROLL_TICKS - 1,
        "the body settled {} ticks after it landed, not RiderRollTime's {}",
        settled_at - grounded_at,
        ROLL_TICKS - 1
    );
    // And it settled while it was still down, not on the way back up.
    assert!(settled_at < back_at, "it settled after the respawn");
    let held = held.expect("a settled body holds a limb");
    assert!(
        (HOLD_LIMB..HOLD_LIMB + HOLD_LIMBS).contains(&held),
        "a settled body holds one of its five limbs, not {held}"
    );
    // Back on the bike, as the original goes (S74): on the clock's tick the body's request has not
    // seen the put-back, so the body is still in the wreck; the next two it is held on the bike; then
    // it rides. Leaving the wreck for `Held` is the arm that cleans nothing up, so the settle is kept,
    // as the original's is — the next wreck's entry is what clears it.
    assert_eq!(rider.body.machine.mode, Mode::Wrecked, "the clock's tick: the body still down");
    assert!(rider.drawn_jumps(), "the clock's tick: the bike is set down, a cut");
    let mut modes = Vec::new();
    let mut jumps = Vec::new();
    for _ in 0..3 {
        rider.frame(&p, &tables, &hands, WRECK_DT, &mut world);
        jumps.push(rider.drawn_jumps());
        rider.body_frame(WRECK_DT, &hands, gate, &lib, &clips);
        modes.push(rider.body.machine.mode);
        // Held, the body is in the riding row with its ramp zeroed and its axes snapped to the middle,
        // which is where all nine captured put-backs hold it (`motion.rs` compares the joints).
        if rider.body.machine.mode == Mode::Held {
            let veh = &rider.body.veh;
            assert_eq!((veh.state, veh.ramp, veh.phase.value, veh.mix.value), (30, F::ZERO, F(0.5), F(0.5)));
        }
    }
    assert_eq!(modes[..2], [Mode::Held, Mode::Held], "held on the bike for two ticks");
    assert_ne!(modes[2], Mode::Held, "and only two");
    assert_eq!(jumps, [true, false, false], "and the body's own cut back onto it, the tick after");
    assert!(rider.body.settled, "the put-back cleared the settle, which the original keeps");
    println!(
        "the wreck: reason {reason:?} down at {wrecked_at}, on the ground at {grounded_at}, settled at {settled_at} \
         holding state {held}, back up at {back_at} with the settle cleared"
    );
}

// ---------------------------------------------------------------------------------------------------
// 6. the mode machine's wreck frame, against the original
// ---------------------------------------------------------------------------------------------------

/// The other objects the body's mode machine reaches through, in the same unused RAM as the body.
const RAG: u32 = 0x0120_4000;
const MAIN: u32 = 0x0120_4100;
const VTABLE: u32 = 0x0120_4300;
const BONES: u32 = 0x0120_4400;
const BONE: u32 = 0x0120_4500;
const NIL: u32 = 0x0120_4600;
const MORE_BONES: u32 = 0x0120_4a00;

/// The mode machine's own fields, by the offsets the original uses — the five the wreck arm is made of
/// and the pointers it reaches the ragdoll through. A test may say where these live; the port may not.
const B_MODE: u32 = 0x47c;
const B_PREV: u32 = 0x480;
const B_TOUCHING: u32 = 0x48c;
const B_ROLL: u32 = 0x490;
const B_RAGDOLL: u32 = 0x2bc;
const B_MAIN: u32 = 0x2f4;
const B_SCENE: u32 = 0x420;
const B_HEAD_A: u32 = 0x5d0;
const B_HEAD_B: u32 = 0x5d4;
const B_PART_COUNT: u32 = 0x620;
const B_PARTS: u32 = 0x624;
const B_FOLLOW: u32 = 0x628;
const B_HANDS: u32 = 0x62c;
const B_SPIN_FRAME: u32 = 0x6a8;
const B_OFF_BIKE: u32 = 0x6b0;
const B_FLUNG: u32 = 0x6b8;
const B_THROWN: u32 = 0x6bc;
const B_LOOSE: u32 = 0x6c0;
const B_DAMPING: u32 = 0x8c0;
const B_FLAIL_HEIGHT: u32 = 0x8d0;
const B_ROLL_TIME: u32 = 0x8d4;
const B_ROLL_SPEED: u32 = 0x8d8;
/// The main bone: how high it is, how fast it is going, and the vtable whose damping slot the settle
/// hands the body's own damping record to.
const M_HEIGHT: u32 = 0x64;
const M_VEL: u32 = 0x70;
const M_VTABLE: u32 = 0x100;
const M_SET_DAMPING: u32 = 0x54;
/// One bone: the contact flag the roll's clock is driven by, and the handle its follower is given.
const BONE_TOUCH: u32 = 0x14;
const BONE_FOLLOW: u32 = 0x88;

/// A ground at one height that remembers the point it was last asked about.
struct Under {
    y: f32,
    asked: Option<Vf>,
}

impl World for Under {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        self.asked = Some(at);
        (Vf::new(at.x(), self.y, at.z(), at.w()), Vf::new(0.0, 1.0, 0.0, 0.0))
    }
    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// Whether the body settled this frame, and everything it decided on the way there — read the same way
/// off both sides.
#[derive(Debug, PartialEq)]
struct Rest {
    mode: i32,
    prev: i32,
    settled: bool,
    flung: bool,
    touching: bool,
    roll: f32,
    thrown: f32,
    loose: f32,
    /// What the damping call was handed this frame, or `None` for a frame that made no call: the
    /// stiffer set (`true`) or the limp one.
    stiff: Option<bool>,
}

/// **V19.** The wreck arm of the body's mode machine, frame by frame against the original.
///
/// The original's own function is run with its ten callees hooked away — the ground query, the
/// bone followers, the damping setter — so what is left running is the mode chain and the wreck's own
/// clocks, roll and settle test, which is the whole of what this task ported. The ground query's hook
/// is *not* a no-op: it answers with the ground height the case asks for, which is what the settle
/// test measures the main bone's clearance from (S52; V19 read it as a rise across a ragdoll step).
/// The port's sensors come off the same bones the original reads: one to four collisions' contact
/// flags, and the main bone's whole position and velocity, with the ground query's point and
/// arguments compared.
///
/// Compared per frame: the mode and the previous mode, the settle flag, the flung flag, the contact
/// flag, all three clocks, and which of the two damping sets the frame asked for — the last one off a
/// recording hook, because in the original it is inside the bones and not a field.
#[test]
fn the_wreck_arm_of_the_mode_machine_matches_frame_by_frame() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&[
        "rider_body_mode",
        "rider_body_wreck_enter",
        "rider_body_damping",
        "ground_query",
        "rider_bone_follow",
        "rider_bone_post",
        "rider_bone_detach",
        "rider_body_look_reset",
        "rider_body_ride_enter",
        "rand_raw",
        "quat_axes",
    ]) else {
        return;
    };
    let (mode_at, damping_at, ground_at) = (at[0], at[2], at[3]);
    // The main bone's own damping slot is a *virtual* call, so the test gives the bone a vtable of its
    // own and points that slot at another hooked address — one the wreck arm cannot otherwise reach —
    // which is how the record it is handed becomes visible here.
    let sink_at = at[6];

    // The ground height the query is to answer, and what the damping setter was handed. Shared with
    // the hooks, which is the only way a hook can carry anything in or out.
    let ground = Rc::new(Cell::new(0.0f32));
    // What the ground query was handed: the point (all four lanes, read before the hook moves it) and
    // the other four arguments.
    let asked = Rc::new(Cell::new(None));
    let stiff = Rc::new(Cell::new(None));
    let record = Rc::new(Cell::new(None));
    {
        let ground = Rc::clone(&ground);
        let asked = Rc::clone(&asked);
        ee.hooks.insert(
            ground_at,
            Box::new(move |ee: &mut ps2_oracle::Ee| {
                // The point is moved onto the ground: only its height is read back.
                let point = ee.r(5) as u32;
                let at = [0, 4, 8, 12].map(|k| ee.u32_at(point + k));
                asked.set(Some((at, [ee.r(4), ee.r(6), ee.r(7), ee.r(8)])));
                ee.set_f32(point + 4, ground.get());
            }),
        );
        let stiff = Rc::clone(&stiff);
        ee.hooks.insert(
            damping_at,
            Box::new(move |ee: &mut ps2_oracle::Ee| stiff.set(Some(ee.r(5) & 1 != 0))),
        );
        let record = Rc::clone(&record);
        ee.hooks.insert(
            sink_at,
            Box::new(move |ee: &mut ps2_oracle::Ee| record.set(Some(ee.r(5) as u32))),
        );
        // Everything else the function calls, and nothing of it is this task's: the wreck's entry (the
        // frames below never enter, so it is only hooked so that a generated case cannot reach it), the
        // bone followers, the two other modes' entries, the generator and the quaternion helper.
        for &a in &[at[1], at[4], at[5], at[7], at[8], at[9], at[10]] {
            ee.hooks.insert(a, Box::new(|_| {}));
        }
    }

    let mut pick = Pick(0x9e37_79b9_7f4a_7c15);
    let (mut settles, mut flings, mut rolls, mut held) = (0, 0, 0, 0);
    for k in 0..10_000u32 {
        // The frame's own numbers. Each one is pinned often enough that every branch of the test is
        // reached, and generated otherwise.
        let dt = match pick.upto(4) {
            0 => 1.0 / 60.0,
            1 => 0.03333,
            2 => 0.0,
            _ => pick.float(0.2).abs(),
        };
        let roll0 = match pick.upto(4) {
            0 => 0.0,
            1 => 0.74,
            2 => 0.76,
            _ => pick.float(2.0).abs(),
        };
        let thrown0 = match pick.upto(3) {
            0 => 0.0,
            1 => -1.0,
            _ => pick.float(0.5),
        };
        let loose0 = match pick.upto(3) {
            0 => 0.0,
            1 => 0.3,
            _ => pick.float(0.5),
        };
        let flung0 = pick.upto(2) == 1;
        let settled0 = pick.upto(4) == 0;
        // One to four bone collisions, each with its own contact flag: none touching a third of the
        // time, and otherwise any of them.
        let collisions = 1 + pick.upto(4) as usize;
        let none = pick.upto(3) == 0;
        let touched: Vec<bool> = (0..collisions).map(|_| !none && pick.upto(2) == 1).collect();
        let touching = touched.iter().any(|&t| t);
        // The limit the clearance is tested against — which is sometimes negative, so that the arm
        // which decides a body has been flung off the ground again is reachable here.
        let flail_height = match pick.upto(4) {
            0 => 3.0,
            1 => -1.0,
            2 => 0.0,
            _ => pick.float(5.0),
        };
        let roll_time = match pick.upto(3) {
            0 => 0.75,
            1 => 0.0,
            _ => pick.float(2.0).abs(),
        };
        let roll_speed = match pick.upto(3) {
            0 => 0.0,
            1 => 5.0,
            _ => pick.float(20.0).abs(),
        };
        let height0 = pick.float(50.0);
        // The ground under the main bone: level with it, exactly the flail height below it, or
        // anywhere within six feet either side.
        let ground_k = match pick.upto(4) {
            0 => height0,
            1 => height0 - flail_height,
            2 => height0 + 1.0,
            _ => height0 - pick.float(6.0),
        };
        let vel = [pick.float(30.0), pick.float(30.0), pick.float(30.0), pick.float(2.0)];
        // The rest of the main bone's position, which the ground query is handed whole.
        let (x0, z0, w0) = (pick.float(300.0), pick.float(300.0), pick.float(2.0));

        // The body, its request and the five objects the wreck arm reaches through.
        ee.load(BODY, &vec![0u8; 0xb00]);
        ee.load(REQ, &vec![0u8; 0x200]);
        for &a in &[RAG, MAIN, VTABLE, BONES, BONE, NIL, MORE_BONES] {
            ee.load(a, &vec![0u8; 0x200]);
        }
        ee.set_u32(BODY + B_REQUEST, REQ);
        // Off the bike: a wreck reason in the request is what the chain picks mode 2 on, and a previous
        // mode of 2 is what makes this a frame of the wreck and not an entry into it.
        ee.set_u32(REQ + R_WRECK, 5);
        ee.set_u32(BODY + B_PREV, 2);
        // The ragdoll, and the main bone the settle test reads.
        ee.set_u32(BODY + B_RAGDOLL, RAG);
        ee.set_u32(BODY + B_MAIN, RAG);
        ee.set_u32(RAG + 0xa4, MAIN);
        ee.set_u32(MAIN + M_VTABLE, VTABLE);
        ee.set_u32(VTABLE + M_SET_DAMPING, sink_at);
        ee.set_f32(MAIN + M_HEIGHT - 4, x0);
        ee.set_f32(MAIN + M_HEIGHT, height0);
        ee.set_f32(MAIN + M_HEIGHT + 4, z0);
        ee.set_f32(MAIN + M_HEIGHT + 8, w0);
        for (i, &v) in vel.iter().enumerate() {
            ee.set_f32(MAIN + M_VEL + 4 * i as u32, v);
        }
        // The bone collisions, with the contact flags the roll runs on. Only the first one's follower
        // is read.
        ee.set_u32(BODY + B_PART_COUNT, collisions as u32);
        ee.set_u32(BODY + B_PARTS, BONES);
        for (i, &t) in touched.iter().enumerate() {
            let c = if i == 0 { BONE } else { MORE_BONES + 0x20 * i as u32 };
            ee.set_u32(BONES + 4 * i as u32, c);
            ee.set_u32(c + BONE_TOUCH, u32::from(t));
        }
        ee.set_u32(BONE + BONE_FOLLOW, NIL);
        ee.set_u32(BODY + B_FOLLOW, NIL);
        // The three pointers the arm reads and this frame wants nothing from: the hand dummies (a zero
        // first word turns the block off), the two head dummies, and the scene (its mode is not the one
        // the placement block runs in).
        ee.set_u32(BODY + B_HANDS, NIL);
        ee.set_u32(BODY + B_HEAD_A, NIL);
        ee.set_u32(BODY + B_HEAD_B, NIL);
        ee.set_u32(BODY + B_SCENE, NIL);
        // Already off the bike and past the frame the thrown orientation is chosen in: two blocks of
        // renderer work this port has no answer for, and both are skipped by a flag the original keeps.
        ee.set_u32(BODY + B_OFF_BIKE, 1);
        ee.set_u32(BODY + B_SPIN_FRAME, 1);
        // The frame's own state.
        ee.set_f32(BODY + B_ROLL, roll0);
        ee.set_f32(BODY + B_THROWN, thrown0);
        ee.set_f32(BODY + B_LOOSE, loose0);
        ee.set_u32(BODY + B_FLUNG, u32::from(flung0));
        ee.set_u32(BODY + B_SETTLED, u32::from(settled0));
        ee.set_u32(BODY + B_TOUCHING, u32::from(pick.upto(2) == 1));
        ee.set_f32(BODY + B_FLAIL_HEIGHT, flail_height);
        ee.set_f32(BODY + B_ROLL_TIME, roll_time);
        ee.set_f32(BODY + B_ROLL_SPEED, roll_speed);

        ground.set(ground_k);
        asked.set(None);
        stiff.set(None);
        record.set(None);
        ee.call(mode_at, &[u64::from(BODY)], &[dt], BUDGET);
        let theirs = Rest {
            mode: ee.u32_at(BODY + B_MODE) as i32,
            prev: ee.u32_at(BODY + B_PREV) as i32,
            settled: ee.u32_at(BODY + B_SETTLED) != 0,
            flung: ee.u32_at(BODY + B_FLUNG) != 0,
            touching: ee.u32_at(BODY + B_TOUCHING) != 0,
            roll: ee.f32_at(BODY + B_ROLL),
            thrown: ee.f32_at(BODY + B_THROWN),
            loose: ee.f32_at(BODY + B_LOOSE),
            stiff: stiff.get(),
        };

        // The same frame through the port. Its machine is handed the same state, and its sensors are
        // read off the same bones: the collisions' flags, the main bone, and a ground that answers the
        // case's height and logs what it was asked.
        let mut body = Body::default();
        body.settled = settled0;
        body.machine.prev = game_sim::rider_anim::Mode::Wrecked;
        body.machine.roll = F(roll0);
        body.machine.thrown = F(thrown0);
        body.machine.loose = F(loose0);
        body.machine.flung = flung0;
        body.machine.touching = ee.u32_at(BODY + B_TOUCHING) != 0;
        body.machine.stiff = false;
        let mut wrecks = game_sim::phy::Wrecks::default();
        wrecks.rider_flail_height = F(flail_height);
        wrecks.rider_roll_time = F(roll_time);
        wrecks.rider_roll_speed = F(roll_speed);
        let contacts: Vec<Contact> = touched.iter().map(|&t| Contact { touched: t, ..Contact::default() }).collect();
        let main = Bone {
            position: Vf::new(x0, height0, z0, w0),
            velocity: Vf::new(vel[0], vel[1], vel[2], vel[3]),
            ..Bone::default()
        };
        let mut under = Under { y: ground_k, asked: None };
        let thrown = Thrown::from_bones(phy::Throw::default(), &contacts, &main, &mut under);
        let (at, args) = asked.get().expect("the wreck arm queries the ground every frame");
        assert_eq!(under.asked.map(|v| v.to_bits()), Some(at), "case {k}: the point the ground was asked about");
        assert_eq!(args, [u64::from(RAG), 0, 0, 0], "case {k}: the world, and no normal, flags or record");
        let (mut r, mut a) = resting();
        r.held = false;
        a.wreck = true;
        let mut pool = Pool::default();
        let clock = Clock { second: 0, minute: 0, hour: 0, day: 0 };
        let before = body.machine.stiff;
        let mode =
            body.mode(F(dt), &r, &a, &wrecks, Some(thrown), &mut pool, clock);
        let ours = Rest {
            mode: match mode {
                game_sim::rider_anim::Mode::Wrecked => 2,
                other => panic!("case {k}: the port left the wreck for {other:?}"),
            },
            prev: 2,
            settled: body.settled,
            flung: body.machine.flung,
            touching: body.machine.touching,
            roll: body.machine.roll.0,
            thrown: body.machine.thrown.0,
            loose: body.machine.loose.0,
            // The port keeps the choice as a flag; the original makes a call. A frame that changed
            // neither made no call.
            stiff: if body.machine.stiff == before && theirs.stiff.is_none() {
                None
            } else {
                Some(body.machine.stiff)
            },
        };
        assert_eq!(
            ours, theirs,
            "case {k}: dt {dt}, roll {roll0}, thrown {thrown0}, loose {loose0}, flung {flung0}, \
             settled {settled0}, touching {touching}, ground {ground_k}, flail {flail_height}, \
             time {roll_time}, speed {roll_speed}, vel {vel:?}"
        );
        if theirs.settled && !settled0 {
            // The one thing the settle does besides raising the flag: the main bone is handed the
            // body's own `GroundAngularRateDamping` record, which is where the rider file's quadword
            // sits on this actor.
            assert_eq!(
                record.get(),
                Some(BODY + B_DAMPING),
                "case {k}: the settle did not hand the main bone the damping record"
            );
            settles += 1;
        }
        if theirs.flung && !flung0 {
            flings += 1;
        }
        if theirs.roll > roll0 {
            rolls += 1;
        }
        if theirs.settled && settled0 {
            held += 1;
        }
    }
    println!(
        "the wreck arm: 10000 frames, {settles} settled, {flings} flung, {rolls} rolling, \
         {held} already at rest"
    );
    assert!(settles > 1000 && flings > 100, "too few settles ({settles}) or flings ({flings}) to prove the test");
}

// ---------------------------------------------------------------------------------------------------
// 7. the put-back's hold on the body, against the original (S74)
// ---------------------------------------------------------------------------------------------------
//
// S74 recorded the body actor once a tick through S66's five runs (free06 riding, wrecked and put back
// by the clock and by the chord; free09 riding and wrecked) with
// `research/journal/2026-10-21-s74-putback-bones/capture/bones_watch.py`: the bike's reason and ghost
// clock, the request block the body reads, the body's mode and every joint's node. The captures sit in
// `context/s74/`; without them the test says so and passes, and with them it counts what it compared.

/// One tick of a bones capture, as far as the put-back's hold goes.
struct BonesTick {
    frame: u32,
    /// The bike's wreck reason and its ghost clock, at the end of the tick.
    reason: u32,
    ghost: f32,
    /// The request as the body read it: held, and the wreck reason it was filled with.
    held: bool,
    asked_wreck: bool,
    /// The body's mode.
    mode: u32,
    /// The body's main node and the bike's node, in the world.
    body_at: Vf,
    bike_at: Vf,
}

fn bones_capture(text: &str) -> Vec<BonesTick> {
    let word = |b: &[u8], o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let quad = |b: &[u8], o: usize| {
        let f = |k: usize| f32::from_bits(word(b, o + 4 * k));
        Vf::new(f(0), f(1), f(2), f(3))
    };
    let mut rows = Vec::new();
    for chunk in text.split("\nf ").skip(1) {
        let field = |name: &str| -> Option<Vec<u8>> {
            let line = chunk.lines().find(|l| l.split(' ').next() == Some(name))?;
            let hex = line.split(' ').nth(1)?;
            (0..hex.len() / 2).map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok()).collect()
        };
        let (Some(bike), Some(ghost), Some(req), Some(body), Some(node0), Some(bike_node)) = (
            field("bike.748"),
            field("bike.7dc"),
            field("req"),
            field("body.47c"),
            field("node0"),
            field("bike.node"),
        ) else {
            continue;
        };
        rows.push(BonesTick {
            frame: chunk.split_whitespace().next().unwrap().parse().unwrap(),
            reason: word(&bike, 0),
            ghost: f32::from_bits(word(&ghost, 0)),
            held: word(&req, 0xc) != 0,
            asked_wreck: word(&req, 0x104) != 0,
            mode: word(&body, 0),
            body_at: quad(&node0, 0xb0),
            bike_at: quad(&bike_node, 0x70),
        });
    }
    rows
}

/// **The put-back holds the body on the bike for two ticks** (S74). On every tick of the five
/// captures, [`PutBack`] — set down where the port sets it, by the chord before the request's fill and
/// by the wreck clock after it, and filled with the wreck as it stood before the machine's verdict —
/// tells the body what the original's request told it: held, and down. On every captured put-back the
/// body is held on exactly the two ticks the port holds it, still in the wreck on the clock's own tick,
/// and drawn on the bike from its first held tick.
///
/// The one place the body's own mode parts from the port's is the wreck's entry, which the original's
/// request lags by the same fill and the port does not (S74's journal): counted, not compared.
#[test]
fn the_put_back_holds_the_body_as_the_original_does() {
    use game_sim::rider::PutBack;
    let kit = match ps2_oracle::Kit::load() {
        Ok(kit) => kit,
        Err(why) => {
            println!("no kit.toml ({why}): no put-back hold verdict");
            return;
        }
    };
    let (mut ticks, mut chords, mut clocks, mut entries, mut entered) = (0, 0, 0, 0, 0);
    for name in ["free06-ride", "free06-down", "free06-downchord", "free09-ride", "free09-down"] {
        let Ok(text) = std::fs::read_to_string(kit.ctx("s74").join(format!("{name}.txt"))) else {
            println!("no context/s74/{name}.txt: no put-back hold verdict for it");
            continue;
        };
        let rows = bones_capture(&text);
        let mut back = PutBack::default();
        // The ticks since the last put-back, and what kind it was.
        let mut since: Option<(usize, bool)> = None;
        // The frame of the last wreck's entry, until the tick after it has been checked.
        let mut entry: Option<u32> = None;
        for pair in rows.windows(2) {
            let (before, now) = (&pair[0], &pair[1]);
            assert_eq!(now.frame, before.frame + 1, "{name}: a gap at frame {}", now.frame);
            let started = now.ghost > before.ghost;
            // The clock's put-back starts the ghost after the ghost's tick, the chord's before it.
            let clock = started && now.ghost == 3.0;
            let chord = started && !clock;
            if chord {
                back.place();
            }
            back.fill(!chord && before.reason != 0);
            if clock {
                back.place();
            }
            let f = now.frame;
            assert_eq!(back.held, now.held, "{name} frame {f}: held");
            assert_eq!(back.wreck, now.asked_wreck, "{name} frame {f}: the wreck the request was filled with");
            ticks += 1;

            if started {
                since = Some((0, clock));
                if clock {
                    clocks += 1;
                } else {
                    chords += 1;
                }
            } else if let Some((k, kind)) = since {
                since = Some((k + 1, kind));
            }
            if before.reason == 0 && now.reason != 0 {
                // The wreck's entry: the request has not seen it, and nor has the body — the port's
                // (`Rider::body_frame` asks with `PutBack::wreck` alone, S91) or the original's.
                assert!(!now.asked_wreck && now.mode != 2, "{name} frame {f}: the body saw the wreck's entry");
                assert!(!back.wreck, "{name} frame {f}: the port's body saw the wreck's entry");
                entry = Some(f);
                entries += 1;
            } else if entry.take().is_some_and(|e| e + 1 == f) {
                // And the tick after, both bodies are in the wreck.
                assert!(now.asked_wreck && now.mode == 2, "{name} frame {f}: the body missed the wreck's entry");
                assert!(back.wreck, "{name} frame {f}: the port's body missed the wreck's entry");
                entered += 1;
            }
            let Some((k, clock)) = since else { continue };
            let first_held = if clock { 1 } else { 0 };
            match k {
                _ if k < first_held => {
                    assert_eq!(now.mode, 2, "{name} frame {f}: the clock's tick, the body still down");
                    assert!(back.wreck && !back.held, "{name} frame {f}: and the port's with it");
                }
                _ if k < first_held + 2 => {
                    assert_eq!(now.mode, 0, "{name} frame {f}: held on the bike");
                    let away = now.body_at.sub(now.bike_at);
                    let away = (away.x() * away.x() + away.y() * away.y() + away.z() * away.z()).sqrt();
                    assert!(away < 6.0, "{name} frame {f}: held, and drawn {away} ft off the bike");
                }
                _ if k == first_held + 2 => {
                    assert_ne!(now.mode, 0, "{name} frame {f}: held for two ticks only");
                    since = None;
                }
                _ => {}
            }
        }
    }
    println!("the put-back's hold: {ticks} ticks, {chords} chords, {clocks} clocks, {entries} wreck entries");
    if ticks > 0 {
        assert!(chords + clocks >= 9, "only {} put-backs compared", chords + clocks);
        // context/s74 holds three wreck entries, each proved on its own tick and the one after.
        assert!(entries >= 3 && entered == entries, "only {entered} of {entries} wreck entries proved, not three");
    }
}

// ---------------------------------------------------------------------------------------------------
// V90: the 2p riders' poses
// ---------------------------------------------------------------------------------------------------

mod two_p_poses {
    use game_render::disc::Disc;
    use game_render::rider::{Body as Drawn, Sex};
    use game_sim::input::{EventQueue, Pad};
    use game_sim::phy;
    use game_sim::replay::two_p;
    use game_sim::ride::Controls;
    use game_sim::rider_anim::{Ask, Body, Clock, Mode, Params, Pool};
    use game_sim::trick::{SLOTS, Set, Stunts, Trick};
    use game_sim::vehicle_anim::Request;
    use ps2_float::F;

    const A_RIDER: &str = "thq1";
    const STUNT_ARCHIVE: &str = "appstart.res";
    const STUNT_TABLE: &str = "rider\\anim\\stunts.ini";

    fn member(disc: &Disc, ends: &str) -> String {
        let want = ends.to_ascii_lowercase();
        disc.entries()
            .iter()
            .filter(|e| e.name.to_ascii_lowercase().ends_with(&want))
            .min_by_key(|e| e.name.len())
            .map(|e| e.name.clone())
            .unwrap_or_else(|| panic!("the disc has no {ends}"))
    }

    /// The drawn rider: its sixty-state clip table and the clip lengths the body's machine asks for.
    pub fn drawn() -> Option<Drawn> {
        let mut disc = match Disc::open_configured() {
            Ok(d) => d,
            Err(why) => {
                println!("SKIPPED: {why}");
                return None;
            }
        };
        let stunts = {
            let name = member(&disc, STUNT_ARCHIVE);
            let res = disc.res(&name).expect("the app archive opens");
            let bytes = res.read_suffix(STUNT_TABLE).expect("the stunt table");
            Set::new(Stunts::parse(&bytes).expect("the stunt table parses").selection(0))
        };
        let name = member(&disc, &format!("{A_RIDER}.res"));
        let res = disc.res(&name).expect("the rider archive opens");
        let params = game_sim::motion::Params::default();
        Some(Drawn::load(vec![res], A_RIDER, Sex::Male, "", &stunts.stunts, params).expect("the body loads"))
    }

    /// The capture of record with the columns this needs: the request words the body reads, what the
    /// body played, and both pads.
    fn capture() -> Option<game_sim::replay::Capture> {
        read(false)
    }

    /// The same with the 22 posed joints of both riders, which are most of the file.
    pub fn with_joints() -> Option<game_sim::replay::Capture> {
        read(true)
    }

    fn read(joints: bool) -> Option<game_sim::replay::Capture> {
        read_at(two_p::CAPTURE, joints)
    }

    /// C11's second playback of the same tape, the spec with the body's two lag records added
    /// (`lag_mix`, `lag_phase`: `{value, tau, ratio}` each). Its rows are a playback of their own,
    /// not the capture of record's rows with columns added: they part from it in the low bits.
    pub const EASED: &str = "capture/2p/2p-eased.csv";

    /// [`EASED`], with the 22 posed joints if `joints`.
    pub fn eased(joints: bool) -> Option<game_sim::replay::Capture> {
        read_at(EASED, joints)
    }

    /// Both pads off the capture of record, and the frame_counter of its first row.
    fn record_pads() -> Option<(game_sim::replay::PadTape, i64)> {
        let kit = ps2_oracle::Kit::load().ok()?;
        let local = kit.root.join("context").join(two_p::CAPTURE);
        let path = if local.exists() { local } else { kit.ctx(two_p::CAPTURE) };
        let Ok(file) = std::fs::File::open(&path) else {
            println!("SKIPPED: no {}", path.display());
            return None;
        };
        let keep = |c: &str| ["frame_counter", "dt"].contains(&c) || c.starts_with("pad") || c.starts_with("stamp");
        let cap = two_p::read(std::io::BufReader::new(file), keep).expect("the 2p capture reads");
        Some((two_p::tape(&cap).expect("both pads off the capture of record"), Col(&cap).u(0, "frame_counter")))
    }

    fn read_at(rel: &str, joints: bool) -> Option<game_sim::replay::Capture> {
        let kit = match ps2_oracle::Kit::load() {
            Ok(kit) => kit,
            Err(why) => {
                println!("SKIPPED: no kit.toml ({why})");
                return None;
            }
        };
        let local = kit.root.join("context").join(rel);
        let path = if local.exists() { local } else { kit.ctx(rel) };
        let Ok(file) = std::fs::File::open(&path) else {
            println!("SKIPPED: no {}", path.display());
            return None;
        };
        let lane = [
            "airborne", "flip_ramp", "trick_id", "req_placing", "req_wreck", "req_axes.x", "req_axes.y",
            "req_axes.z", "req_progress", "d_cross", "d_flags", "m_clock", "m_finished", "mode", "settled",
            "lag_mix.x", "lag_mix.y", "lag_mix.z", "lag_phase.x", "lag_phase.y", "lag_phase.z", "d_reverse",
        ];
        let keep = |c: &str| {
            ["frame_counter", "dt"].contains(&c)
                || c.starts_with("pad")
                || c.starts_with("stamp")
                || c.strip_prefix("p1_").or_else(|| c.strip_prefix("p2_")).is_some_and(|c| {
                    lane.contains(&c) || (joints && c.starts_with('j') && c.as_bytes().get(1).is_some_and(u8::is_ascii_digit))
                })
        };
        Some(two_p::read(std::io::BufReader::new(file), keep).expect("the 2p capture reads"))
    }

    pub struct Col<'a>(pub &'a game_sim::replay::Capture);
    impl Col<'_> {
        fn at(&self, name: &str) -> usize {
            self.0.columns.iter().position(|c| c == name).unwrap_or_else(|| panic!("no column {name}"))
        }
        pub fn text(&self, row: usize, name: &str) -> &str {
            &self.0.rows[row][self.at(name)]
        }
        pub fn f(&self, row: usize, name: &str) -> f32 {
            self.text(row, name).parse().unwrap_or_else(|_| panic!("{name} in row {row}"))
        }
        pub fn u(&self, row: usize, name: &str) -> i64 {
            self.text(row, name).parse().unwrap_or_else(|_| panic!("{name} in row {row}"))
        }
        pub fn word(&self, row: usize, name: &str, off: usize) -> u32 {
            let hex = self.text(row, name);
            let b = |i: usize| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("hex");
            u32::from_le_bytes([b(off), b(off + 1), b(off + 2), b(off + 3)])
        }
    }

    /// One rider's body as the original's request drove it, tick by tick: what the port's machine
    /// plays (state, mode) beside what the original's did.
    pub struct Tick {
        pub frame: u32,
        pub theirs: (i32, i32),
        pub ours: (i32, i32),
        /// The cross-fade ramp, the original's and the port's, as bits.
        pub ramp: (u32, u32),
        /// How far the drawn rider's joints are from the original's, the worst lane of the 22
        /// quaternions and translations, when the replay was asked to pose them.
        pub joints: Option<f32>,
        /// The two eased axes after the tick's step, mix then phase, the original's and the port's as
        /// bits, on a capture that has the lag records (C11).
        pub eased: Option<[(u32, u32); 2]>,
        /// The reverse pair's own phase clock after the tick, the original's and the port's, as bits.
        pub reverse: (u32, u32),
    }

    fn mode_number(m: Mode) -> i32 {
        match m {
            Mode::Held => 0,
            Mode::Riding => 1,
            Mode::Wrecked => 2,
            Mode::Posed => 3,
            Mode::Gate => 4,
        }
    }

    trait Lag {
        fn lag(&self) -> usize;
    }
    impl Lag for two_p::Rider {
        fn lag(&self) -> usize {
            match self {
                two_p::Rider::P1 => 0,
                two_p::Rider::P2 => 1,
            }
        }
    }

    pub fn replay() -> Option<Vec<Vec<Tick>>> {
        run(capture()?, false)
    }

    /// The same, posing each rider's drawn body off its machine's frame as the shell does
    /// ([`Drawn::posed_by`]) and holding its joints against the captured ones.
    pub fn replay_posed() -> Option<Vec<Vec<Tick>>> {
        run(with_joints()?, true)
    }

    fn run(cap: game_sim::replay::Capture, posed: bool) -> Option<Vec<Vec<Tick>>> {
        run_with(cap, posed, false)
    }

    /// [`run`] on a capture with the lag records. With `seed`, each tick's two axes and the reverse
    /// pair's clock start from the original's as the last row left them, so the port's
    /// [`Lag`](game_sim::vehicle_anim::Lag) and clock step from where the original's did and the pose
    /// is sampled off the original's eased values rather than the port's own run of them.
    pub fn run_with(cap: game_sim::replay::Capture, posed: bool, seed: bool) -> Option<Vec<Vec<Tick>>> {
        let mut drawn = drawn()?;
        let c = Col(&cap);
        // A capture the recorder dropped rows from (C11's) has no tape of its own: its pads are the
        // tape's fields, so the capture of record's, by frame_counter.
        let (tape, first) = match two_p::tape(&cap) {
            Ok(tape) => (tape, c.u(0, "frame_counter")),
            Err(_) => record_pads()?,
        };
        let mut out = vec![];
        for rider in two_p::Rider::BOTH {
            if rider != two_p::Rider::P1 {
                drawn = self::drawn()?;
            }
            let p = rider.prefix();
            let n = |s: &str| format!("{p}{s}");
            let has_eased = cap.columns.iter().any(|col| *col == n("lag_mix.x"));
            let mut body = Body::default();
            let numbers = Params::built();
            let mut rates = game_render::vehicle::params();
            let mut pool = Pool::default();
            let clock = Clock::default();
            let mut r = Request::default();
            let mut pad = Pad::new(rider.pad() as u32);
            pad.connected = true;
            let mut events = EventQueue::new();
            let mut ticks = vec![];
            let mut hands = vec![F::ZERO];
            for row in 1..cap.rows.len() {
                // A 2p row's dt is the next frame's: this tick ran on the last row's.
                let dt = F(c.f(row - 1, "dt"));
                // The pad of each tick since the last row: one, but for a row after a dropped one.
                let (from, to) = (c.u(row - 1, "frame_counter") + 1, c.u(row, "frame_counter"));
                if (to - first) as usize > tape.frames.len() {
                    break;
                }
                for f in from..=to {
                    let frame = &tape.frames[(f - first - 1) as usize];
                    pad.poll(dt, &frame.reports[rider.pad()], &mut events);
                }
                let (speed, up) = (F(c.f(row - 1, &n("req_axes.z"))), c.u(row - 1, &n("airborne")) != 0);
                let controls = Controls::from_pad(&pad).reverse_rule(speed, up);
                hands.push(controls.reverse);
                r.held = c.u(row, &n("req_placing")) != 0;
                r.phase_axis = F(c.f(row, &n("req_axes.x")));
                r.mix_axis = F(c.f(row, &n("req_axes.y")));
                r.band = F(c.f(row, &n("req_axes.z")));
                r.progress = F(c.f(row, &n("req_progress")));
                // The request's count is what the tail latches into the machine: this row's latch.
                r.seq = (c.word(row, &n("d_cross"), 0x20) & 0xff) as i32;
                // The bike's own words are read a tick late: the body asks off the bike as the
                // last tick left it, before this tick's bike update writes them again.
                let late = row - rider.lag();
                let id = Trick(c.u(row, &n("trick_id")) as u8);
                let ask = Ask {
                    airborne: c.u(late, &n("airborne")) != 0,
                    backflip: c.u(late, &n("flip_ramp")) != 0,
                    no_hander: false,
                    wreck: c.u(row, &n("req_wreck")) != 0,
                    countdown: F::ZERO,
                    throttle: controls.gas,
                    reversing: hands[hands.len() - 1 - rider.lag()],
                    gesture: id.slot().map_or(SLOTS as i32, |s| s as i32),
                    spin: None,
                };
                // The settle is the ragdoll's (S160), which this does not run: the original's own flag
                // as its mode machine left it last tick, for this tick's entries to clear.
                body.settled = c.u(row - 1, &n("settled")) != 0;
                body.mode(dt, &r, &ask, &phy::Wrecks::default(), None, &mut pool, clock);
                if seed && has_eased {
                    body.veh.mix.value = F(c.f(row - 1, &n("lag_mix.x")));
                    body.veh.phase.value = F(c.f(row - 1, &n("lag_phase.x")));
                    body.veh.reverse = F(c.f(row - 1, &n("d_reverse")));
                }
                let lib = &drawn.pose_lib;
                let (frame, clocked) =
                    body.frame_clocked(dt, &numbers, &mut rates, &mut r, &ask, lib, &mut pool, clock, true, &|id| drawn.timing(id));
                let joints = posed.then(|| {
                    drawn.posed_by(frame, &body.player, clocked);
                    let pose = &drawn.rig.set.pose;
                    // The worst lane against the original's joints of this row, or of the row either side
                    // of it, whichever is closest: a lane's joints are read either side of its body's
                    // step (the rows' phase).
                    let off = |rr: usize| {
                        let mut w = 0.0f32;
                        for j in 0..pose.rot.len() {
                            for (lane, axis) in ["x", "y", "z", "w"].iter().enumerate() {
                                for (mine, what) in [(pose.rot[j][lane], "q"), (pose.pos[j][lane], "t")] {
                                    w = w.max((mine - c.f(rr, &n(&format!("j{j}.{what}.{axis}")))).abs());
                                }
                            }
                        }
                        w
                    };
                    (row.saturating_sub(1)..(row + 2).min(cap.rows.len())).map(off).fold(f32::MAX, f32::min)
                });
                ticks.push(Tick {
                    frame: c.u(row, "frame_counter") as u32,
                    theirs: (c.word(row, &n("d_cross"), 0x14) as i32, c.u(row, &n("mode")) as i32),
                    ours: (body.veh.state, mode_number(body.machine.mode)),
                    ramp: (c.word(row, &n("d_flags"), 0xc), body.veh.ramp.to_f32().to_bits()),
                    joints,
                    reverse: (c.f(row, &n("d_reverse")).to_bits(), body.veh.reverse.to_f32().to_bits()),
                    eased: has_eased.then(|| {
                        let theirs = |col: &str| c.f(row, &n(col)).to_bits();
                        [
                            (theirs("lag_mix.x"), body.veh.mix.value.to_f32().to_bits()),
                            (theirs("lag_phase.x"), body.veh.phase.value.to_f32().to_bits()),
                        ]
                    }),
                });
            }
            out.push(ticks);
        }
        Some(out)
    }
}

/// Every tick of the 2p state where the two pinned parts sit, by rider: the frame, the original's
/// (state, mode) and the port's. Both are a row's phase that two ticks of slack does not cover:
///
/// * P1 134857: a one-tick riding pose in the middle of a take-off, on a row whose airborne word
///   already reads 1 — the original's body saw the bike down for one more tick.
/// * P2 140364: the wreck request is on the row a tick before the original's body takes it, and
///   the port goes down on it.
const TWO_P_PARTS: [&[(u32, (i32, i32), (i32, i32))]; 2] =
    [&[(134857, (30, 1), (35, 1))], &[(140364, (30, 1), (40, 2))]];

/// The ticks of each rider whose machine played the original's state and mode, its ramp as well,
/// bit for bit (V90).
const TWO_P_SAME: [(usize, usize); 2] = [(12880, 11864), (12875, 11891)];

/// **V90 — the 2p riders' poses against the original's.** Each rider's body machine
/// ([`Body::mode`] and [`Body::frame_clocked`] on the drawn rider's own clip table) is driven by the
/// original's request words off C10's capture (held, wreck, the three axes, progress, the latch) and
/// by the bike's (airborne, the backflip, the trick) with the pads' reverse rule, and each tick's
/// state and mode are held against the original's: riding lean, air, reverse, every trick and
/// backflip of the ride, the wrecks and their limbs, and the put-backs.
///
/// What is allowed: a limb drawn differently (41 to 45; the random pool is not the original's), and a
/// state the port plays within two ticks of the original's (the rows' phase). Two ticks are past that
/// and pinned ([`TWO_P_PARTS`]). The cross-fade ramp is held too: on ticks where both played the same
/// state twice running, it is the original's bit for bit or a whole one or two ticks of the state's
/// rate off it (a restart a tick apart) — except the run after P1's 134857, where the original
/// restarted twice.
#[test]
fn the_2p_riders_poses() {
    let Some(riders) = two_p_poses::replay() else { return };
    // The limb a settled body holds is a random draw, and the pool is not the original's: any of the
    // five limbs is the same pose for this.
    let same = |a: (i32, i32), b: (i32, i32)| a == b || (a.1 == b.1 && (41..=45).contains(&a.0) && (41..=45).contains(&b.0));
    for (k, ticks) in riders.iter().enumerate() {
        // A tick whose clip the port plays within two ticks of it is the row's phase, not a part.
        let near = |i: usize| (i.saturating_sub(2)..(i + 3).min(ticks.len())).any(|j| same(ticks[i].theirs, ticks[j].ours));
        let parts: Vec<_> =
            ticks.iter().enumerate().filter(|&(i, _)| !near(i)).map(|(_, t)| (t.frame, t.theirs, t.ours)).collect();
        let mut clips = std::collections::BTreeMap::<i32, (usize, usize)>::new();
        for t in ticks {
            let e = clips.entry(t.theirs.0).or_default();
            e.0 += 1;
            e.1 += usize::from(same(t.theirs, t.ours));
        }
        let exact = ticks.iter().filter(|t| same(t.theirs, t.ours)).count();
        println!("P{}: {exact} of {} ticks the same; by clip (ticks, the same) {clips:?}", k + 1, ticks.len());

        // The ramp, on the ticks where both played the same state this tick and the last.
        let both = |i: usize| i > 0 && ticks[i].theirs == ticks[i].ours && ticks[i - 1].theirs == ticks[i - 1].ours;
        let steady: Vec<usize> = (0..ticks.len()).filter(|&i| both(i)).collect();
        let ramps = steady.iter().filter(|&&i| ticks[i].ramp.0 == ticks[i].ramp.1).count();
        println!("  ramp bit for bit on {ramps} of {} steady ticks", steady.len());
        // The ramp runs down at one of the state rates (4, 3.25 or 0.25 a second): a ramp one or two
        // of those 60 Hz ticks off the original's is the same run started a tick or two apart, and
        // one that has run out a tick before the other is that too, the ramp stopping at nothing.
        // Each tick's own dt moves a ramp a few ten-thousandths off the even steps.
        let shifted = |i: usize| {
            let (a, b) = (f32::from_bits(ticks[i].ramp.0), f32::from_bits(ticks[i].ramp.1));
            let off = (a - b).abs();
            let rates = [4.0f32, 3.25, 0.25];
            let out = (a == 0.0 || b == 0.0) && rates.iter().any(|r| off <= 2.0 * r / 60.0);
            out || rates.iter().any(|r| (0..=2).any(|n| (off - n as f32 * r / 60.0).abs() < 5e-4))
        };
        let odd: Vec<u32> =
            steady.iter().filter(|&&i| ticks[i].ramp.0 != ticks[i].ramp.1 && !shifted(i)).map(|&i| ticks[i].frame).collect();
        for t in &parts {
            println!("  part {} theirs (state, mode) {:?} ours {:?}", t.0, t.1, t.2);
        }
        if !odd.is_empty() {
            println!("  ramp off on {odd:?}");
        }

        assert_eq!(parts, TWO_P_PARTS[k], "P{}'s parts past the phase", k + 1);
        assert_eq!((exact, ramps), TWO_P_SAME[k], "P{}'s ticks the same and its ramps bit for bit", k + 1);
        // Only the run after P1's pinned blip, where the original restarted the ramp twice.
        assert!(odd.iter().all(|&f| k == 0 && (134858..134876).contains(&f)), "P{}'s ramps off on {odd:?}", k + 1);
    }
}

/// **V90 — the 2p riders' joints against the original's**, the way V12 held the human's recording:
/// on every tick of the 2p ride where the original's body sampled one clip and nothing else (a
/// gesture come into from no gesture, or a played-once clip running free) with no fade running and
/// the snapshot ramp run out, that clip of the drawn rider's own table is posed at the second the
/// original sampled it, and each of the 22 joints' quaternion and translation is held against the
/// captured ones lane by lane within V12's 1e-4.
///
/// The clip is the state's slot of the port's table: the original's table is not in this capture, so
/// what stands for V12's pointer check is that every pointer the original sampled answers one state
/// only over the whole ride.
#[test]
fn the_2p_riders_joints() {
    use game_sim::replay::two_p;
    const JOINTS: usize = 22;
    const TOLERANCE: f32 = 1e-4;
    let Some(drawn) = two_p_poses::drawn() else { return };
    let Some(cap) = two_p_poses::with_joints() else { return };
    let c = two_p_poses::Col(&cap);
    let set = &drawn.rig.set;
    assert_eq!(set.skeleton.len(), JOINTS, "the drawn rider is not the original's 22 joints");
    let float = |row: usize, col: &str, off: usize| f32::from_bits(c.word(row, col, off));
    let mut pose = game_render::skin::Pose::new(JOINTS);
    for rider in two_p::Rider::BOTH {
        let p = rider.prefix();
        let n = |s: &str| format!("{p}{s}");
        let mut slot_of = std::collections::BTreeMap::<u32, i32>::new();
        let mut by_state = std::collections::BTreeMap::<i32, usize>::new();
        let mut worst = (0.0f32, 0usize, 0i64);
        for row in 0..cap.rows.len() {
            let (state, prev) = (c.word(row, &n("d_cross"), 0x14) as i32, c.word(row, &n("d_cross"), 0x10) as i32);
            let ramp = float(row, &n("d_flags"), 0xc);
            // The motion block from +0x48: the free-running clip, the last sample's clip and second,
            // the transition flag at +0x80 and the clip clock at +0x84.
            let m = n("m_clock");
            let (playing, last, sample) = (c.word(row, &m, 0), c.word(row, &m, 8), float(row, &m, 0xc));
            let (fading, clock) = (c.word(row, &m, 0x38) != 0, float(row, &m, 0x3c));
            let single = match state {
                0..=23 => !(0..=23).contains(&prev),
                40..=59 => playing != 0,
                _ => false,
            };
            if !single || fading || ramp != 0.0 {
                continue;
            }
            let ptr = if playing != 0 { playing } else { last };
            let was = *slot_of.entry(ptr).or_insert(state);
            assert_eq!(was, state, "{p} row {row}: the clip {ptr:#x} the original sampled answers states {was} and {state}");
            let id = drawn.pose_lib.clip[state as usize].unwrap_or_else(|| panic!("the drawn rider has no clip for state {state}"));
            let bound = set.get(id).expect("a bound clip of the drawn rider's set");
            let at = if playing != 0 { clock } else { sample };
            bound.clip.pose(at, &bound.joints, &mut pose);
            let frame = c.u(row, "frame_counter");
            for j in 0..JOINTS {
                for (lane, axis) in ["x", "y", "z", "w"].iter().enumerate() {
                    for (mine, what) in [(pose.rot[j][lane], "q"), (pose.pos[j][lane], "t")] {
                        let theirs = c.f(row, &n(&format!("j{j}.{what}.{axis}")));
                        let off = (mine - theirs).abs();
                        if off > worst.0 {
                            worst = (off, j, frame);
                        }
                        assert!(
                            off <= TOLERANCE,
                            "{p} {frame}: joint {j} ({}) {what}.{axis} is {mine}, the original's {theirs}, in {} at {at}s",
                            set.skeleton.bones[j].name,
                            bound.name
                        );
                    }
                }
            }
            *by_state.entry(state).or_default() += 1;
        }
        let compared: usize = by_state.values().sum();
        println!("{p}: {compared} single-clip ticks posed, by state {by_state:?}; worst {:e} on joint {} at {}", worst.0, worst.1, worst.2);
        assert!(compared >= 100, "{p}: only {compared} single-clip ticks");
    }
}

/// The ticks of each rider where the port played the original's state and mode, and of those the
/// ones whose drawn joints are the original's within V12's 1e-4 (this row or either side of it).
const TWO_P_DRAWN: [(usize, usize); 2] = [(12244, 9959), (12463, 6428)];

/// **V90 — the 2p riders' drawn joints, tick by tick.** The same replay as
/// [`the_2p_riders_poses`], with the drawn rider posed off each tick's frame
/// ([`posed_by`](game_render::rider::Body::posed_by)) and its 22 joints held against the
/// original's. Not every tick is within 1e-4 — the blended arms (riding with the lean on, the air,
/// reverse) sample their clips off the port's eased axes and the original's own eased records are
/// not in the capture (the journal's table by state) — so the count of those that are is pinned
/// and may only go up.
#[test]
fn the_2p_riders_drawn_poses() {
    let Some(riders) = two_p_poses::replay_posed() else { return };
    for (k, ticks) in riders.iter().enumerate() {
        let mut by = std::collections::BTreeMap::<(i32, i32), (usize, usize, usize, f32)>::new();
        for t in ticks.iter().filter(|t| t.theirs == t.ours) {
            let off = t.joints.expect("posed");
            let e = by.entry(t.theirs).or_default();
            e.0 += 1;
            e.1 += usize::from(off <= 1e-4);
            e.2 += usize::from(off <= 1e-2);
            e.3 = e.3.max(off);
        }
        println!("P{}: (state, mode) -> (ticks, within 1e-4, within 1e-2, worst)", k + 1);
        for (key, v) in &by {
            println!("  {key:?} {v:?}");
        }
        let total = (by.values().map(|v| v.0).sum::<usize>(), by.values().map(|v| v.1).sum::<usize>());
        println!("P{}: {} of {} ticks within 1e-4", k + 1, total.1, total.0);
        assert_eq!(total.0, TWO_P_DRAWN[k].0, "P{}'s ticks on the original's clip", k + 1);
        assert!(total.1 >= TWO_P_DRAWN[k].1, "P{}: {} ticks within 1e-4, down from {}", k + 1, total.1, TWO_P_DRAWN[k].1);
    }
}

// ---------------------------------------------------------------------------------------------------
// C11: the 2p riders' eased axes
// ---------------------------------------------------------------------------------------------------

/// Per rider, per axis (mix, phase), on C11's capture: the riding ticks stepped, the ones the port's
/// [`Lag`] steps to bit for bit, the torn rows (see the test) and the ones left unexplained.
const TWO_P_EASED_STEPS: [[(usize, usize, usize, usize); 2]; 2] =
    [[(10983, 10947, 29, 7), (10983, 10921, 43, 19)], [(11179, 11055, 114, 10), (11179, 10852, 291, 36)]];

/// **C11 — the body's two lag records, step by step.** C11's playback of the 2p tape carries the
/// original's two eased-axis records (`{value, time constant, ratio}`, the mix axis stepped first)
/// beside the request's two targets. On every riding tick each record is stepped by the port's
/// [`Lag::step`] from the original's own value of the row before, on that row's dt (a 2p row's dt is
/// the next frame's) and the request's target — snapped to it first when the request is holding, as
/// the body's frame does — and the value and the ratio it leaves are held against the original's bit
/// for bit. The time constants are the port's on every row.
///
/// The target is this row's or the row before's: the request is written at its own phase of the tick,
/// and P2's lane is read on the other side of its body's step (V90), so a row's target is either side
/// of the one the step read. A tick that matches neither is a torn row when the record did not move
/// (the row was read before the step), moved two steps (read after the next one) or moved towards a
/// target between two rows' targets (the request was written twice between reads, and the step saw
/// the write in the middle). What is left is counted and pinned.
#[test]
fn the_2p_eased_axes_step_as_the_ports_lag() {
    use game_sim::replay::two_p;
    use vehicle_anim::{Lag, MIX_TAU, PHASE_TAU};
    let Some(cap) = two_p_poses::eased(false) else { return };
    let c = two_p_poses::Col(&cap);
    let bits = |v: F| v.to_f32().to_bits();
    let mut all = vec![];
    for (k, rider) in two_p::Rider::BOTH.into_iter().enumerate() {
        let p = rider.prefix();
        let n = |s: &str| format!("{p}{s}");
        let mut counts = [(0usize, 0usize, 0usize, 0usize); 2];
        let mut unexplained = vec![];
        for row in 2..cap.rows.len() - 1 {
            // A row after one the recorder missed has no row before it to step from.
            let frame = c.u(row, "frame_counter");
            if c.u(row, &n("mode")) != 1 || frame != c.u(row - 1, "frame_counter") + 1 {
                continue;
            }
            let dt = F(c.f(row - 1, "dt"));
            for (a, (lag, target, tau)) in
                [("lag_mix", "req_axes.y", MIX_TAU), ("lag_phase", "req_axes.x", PHASE_TAU)].into_iter().enumerate()
            {
                let col = |r: usize, l: &str| F(c.f(r, &n(&format!("{lag}.{l}"))));
                assert_eq!(col(row, "y"), tau, "{p} row {row}: {lag}'s time constant");
                let (theirs, ratio, before) = (col(row, "x"), col(row, "z"), col(row - 1, "x"));
                let from = Lag { value: before, tau, ratio: F::ZERO };
                let step = |at: usize| {
                    let target = F(c.f(at, &n(target)));
                    let mut l = from;
                    if c.u(at, &n("req_placing")) != 0 && target != F(f32::MAX) {
                        l.value = target;
                    }
                    l.step(dt, target);
                    l
                };
                let e = &mut counts[a];
                e.0 += 1;
                if [row, row - 1].into_iter().map(step).any(|l| bits(l.value) == bits(theirs) && bits(l.ratio) == bits(ratio)) {
                    e.1 += 1;
                    continue;
                }
                let mut twice = Lag { value: col(row - 2, "x"), tau, ratio: F::ZERO };
                twice.step(F(c.f(row - 2, "dt")), F(c.f(row - 1, &n(target))));
                twice.step(dt, F(c.f(row, &n(target))));
                // The target the step must have read, off the ratio the original left.
                let implied = (before + (theirs - before) / ratio).to_f32();
                let targets: Vec<f32> = (row - 2..=row + 1).map(|r| c.f(r, &n(target))).collect();
                let lo = targets.iter().copied().fold(f32::MAX, f32::min) - 1e-4;
                let hi = targets.iter().copied().fold(f32::MIN, f32::max) + 1e-4;
                if bits(theirs) == bits(before)
                    || (twice.value - theirs).abs().to_f32() <= 1e-6
                    || (lo..=hi).contains(&implied)
                {
                    e.2 += 1;
                } else {
                    e.3 += 1;
                    unexplained.push((frame, lag, theirs.to_f32(), implied, targets));
                }
            }
        }
        println!("{p}: (riding ticks, bit for bit, torn, unexplained) mix {:?} phase {:?}", counts[0], counts[1]);
        for u in unexplained.iter().take(12) {
            println!("  {u:?}");
        }
        all.push(counts);
        let _ = k;
    }
    assert_eq!(all, TWO_P_EASED_STEPS, "the eased steps, P1 then P2");
}

/// Per rider on C11's capture: the ticks on the original's (state, mode), and of those the ones whose
/// eased mix and phase the port's own run left bit for bit, and the ones whose drawn joints are within
/// 1e-4 posed off the port's eased axes and off the original's (seeded).
const TWO_P_EASED_POSES: [(usize, usize, usize, usize, usize); 2] =
    [(12234, 11526, 11409, 9956, 10156), (12452, 6458, 3823, 6242, 7736)];

/// **C11 — V90's blended arms against the original's eased axes.** V90's replay of both riders'
/// bodies, on C11's capture: each tick the port's eased mix and phase are held against the original's
/// records, and the drawn rider's 22 joints are held against the captured ones twice — posed off the
/// port's own eased axes, and with each tick's two axes and reverse clock stepped from the original's
/// values of the row before (`seed`), so whatever still parts on a blended arm (riding lean, air,
/// reverse) is not the eased axes. The table by (state, mode) also counts the reverse clock bit for
/// bit, both ways; the totals are pinned (the journal, 2026-10-28-c11, reads them).
#[test]
fn the_2p_riders_eased_poses() {
    let Some(free) = two_p_poses::eased(true).and_then(|cap| two_p_poses::run_with(cap, true, false)) else { return };
    let Some(seeded) = two_p_poses::eased(true).and_then(|cap| two_p_poses::run_with(cap, true, true)) else { return };
    let mut all = vec![];
    for (k, (free, seeded)) in free.iter().zip(&seeded).enumerate() {
        let mut by = std::collections::BTreeMap::<(i32, i32), [usize; 7]>::new();
        for (f, s) in free.iter().zip(seeded) {
            if f.theirs != f.ours {
                continue;
            }
            let eased = f.eased.expect("the capture has the lag records");
            let e = by.entry(f.theirs).or_default();
            e[0] += 1;
            e[1] += usize::from(eased[0].0 == eased[0].1);
            e[2] += usize::from(eased[1].0 == eased[1].1);
            e[3] += usize::from(f.joints.expect("posed") <= 1e-4);
            e[4] += usize::from(s.theirs == s.ours && s.joints.expect("posed") <= 1e-4);
            e[5] += usize::from(f.reverse.0 == f.reverse.1);
            e[6] += usize::from(s.reverse.0 == s.reverse.1);
        }
        println!("P{}: (state, mode) -> [ticks, mix bit for bit, phase bit for bit, joints ≤1e-4 ours, seeded, reverse clock bit for bit ours, seeded]", k + 1);
        for (key, v) in &by {
            println!("  {key:?} {v:?}");
        }
        let sum = |i: usize| by.values().map(|v| v[i]).sum::<usize>();
        let total = (sum(0), sum(1), sum(2), sum(3), sum(4));
        println!("P{}: {total:?}", k + 1);
        all.push(total);
    }
    assert_eq!(all, TWO_P_EASED_POSES, "the eased axes and joints, P1 then P2");
}
