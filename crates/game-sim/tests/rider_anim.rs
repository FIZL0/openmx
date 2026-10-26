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
