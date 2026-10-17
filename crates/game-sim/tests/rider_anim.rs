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
//!
//! The last three need no disc: they are the task's behavioural test and they run everywhere.
//! What is not proved here, and why, is in the journal entry's last section.

use game_sim::motion::{Clip, ClipId, Player};
use game_sim::phy::Phy;
use game_sim::ride::Controls;
use game_sim::rider::{self, Rider};
use game_sim::rider_anim::{
    Ask, Body, Clock, HOLD_LIMB, HOLD_LIMBS, IDLE, IDLES, Params, Pool,
};
use game_sim::rng::Rng;
use game_sim::trick::{SLOTS, Set};
use game_sim::world::Flat;
use game_sim::vehicle_anim::{
    self, AIR, BACKFLIP, GET_READY, HOLD, Library, REVERSE_L, START_POS, STATES, TURN_RIDING,
};
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
        if asked.last() != Some(&rider.body.veh.want) {
            asked.push(rider.body.veh.want);
        }
    }
    // The same four states as (2), reached off a real rider's fields instead of a hand-made request.
    assert_eq!(asked.len(), 4, "the gate asked for {asked:?}");
    assert_eq!(asked[0], TURN_RIDING);
    assert!((IDLE..IDLE + IDLES).contains(&asked[1]), "a get-ready idle, not {}", asked[1]);
    assert_eq!(asked[2], GET_READY);
    assert!((START_POS..STATES as i32).contains(&asked[3]), "a start pose, not {}", asked[3]);
    // The published half of the request, off the rider: the bars and the rider's weight as the
    // animation header has them, the chassis's speed, and the trick clocks' progress and count.
    let r = rider.body_request;
    assert_eq!(r.phase_axis, rider.motion.pose, "the phase axis is the header's pose");
    assert_eq!(r.mix_axis, rider.motion.fore_aft, "the mix axis is the header's fore-aft");
    assert_eq!(r.band, rider.ride.chassis.speed, "the band is the chassis's speed");
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
        rider.ride.chassis.speed
    );
}
