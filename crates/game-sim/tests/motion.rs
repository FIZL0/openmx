//! Animation driving against the original.
//!
//! Four proofs:
//!
//! 1. **the driving** — ten thousand generated frames of the rider's own animation section: the
//!    cross-fade ramp, the restart test, the pose fields it leaves behind, and which of the motion
//!    library's four entry points it called with which clips and weights. The original ends in VU0
//!    skinning that needs a real skeleton, so the call is stopped at the library entry, which is the
//!    last thing in front of it.
//! 2. **the frame advance** — ten thousand generated frames of the library's own dt path against the
//!    original, including the clip's end, its loop, and the clip-to-clip transition's weight.
//! 3. **the action-frame window** — the inclusive scan, against the original's own binary search over
//!    generated key lists, and the split window a wrapped frame fires in two halves, against the
//!    original's wrap branch with its event objects called for real.
//! 4. **the frame counts** — what the numbers mean at thirty frames a second, which is the part of
//!    "to the frame" no oracle call can state: the cross-fade's length, a stunt's phase per frame, and
//!    the double fire of an action frame that lands on a sample time.
//!
//! The channel sampling, the quaternion blending and the skinning are the renderer's and are not
//! ported; what else is not proved here, and why, is in the journal entry's last section.

use game_sim::motion::{Blend, Clip, ClipId, Clips, Driver, Params, Player, Pose, Request, Slot, Window};
use game_sim::trick::{Bike, SLOTS, Set, Stunt, Trick, Tricks};
use ps2_float::F;
use ps2_oracle::{DONE, Ee, addr, game_or_skip, skip};
use std::cell::Cell;
use std::rc::Rc;

/// Instruction budget per call. The longest of these is one frame of the rider's animation section.
const BUDGET: u64 = 200_000;

/// Where this test puts things in the oracle's RAM: well above the program image.
const OBJ: u32 = 0x0100_0000;
const RECORDS: u32 = 0x0104_0000;
const ANIM_OBJ: u32 = 0x0108_0000;
const VTABLE_OBJ: u32 = 0x0108_8000;
/// The fabricated clip handles: `handle[0]` is the joint map, `handle[1]` the clip itself.
const HANDLES: u32 = 0x0109_0000;
const CLIPS: u32 = 0x0109_8000;
/// Proof 3's key list, its per-key action lists, the action objects and their shared vtable.
const KEYS: u32 = 0x010a_0000;
const KEY_LISTS: u32 = 0x010a_1000;
const LISTS: u32 = 0x010a_2000;
const EVENTS: u32 = 0x010a_3000;
const EVT_VTABLE: u32 = 0x010a_4000;
const OUT: u32 = 0x010a_5000;
const OUT_DATA: u32 = 0x010a_6000;
const HEAP: u32 = 0x010b_0000;

/// Addresses that are not code: each one is hooked, so a vtable can be a table of them.
const HOOK_HAND_HOLD: u32 = 0x0070_0000;
const HOOK_STOP: u32 = 0x0070_0010;
const HOOK_START: u32 = 0x0070_0020;
const HOOK_FIRE: u32 = 0x0070_0030;
const HOOK_PRE: u32 = 0x0070_0040;

/// The rider's own fields, by the offsets the original uses. Named here because a test may say where
/// things live; the port may not.
const GROUND: u32 = 0x574;
const CROSSUP: u32 = 0x600;
const LEAN: u32 = 0x670;
const FORE_AFT_IN: u32 = 0x674;
const SHOWN_CROSSUP: u32 = 0x788;
const ANIM: u32 = 0x7a0;
const ANIM_STATE: u32 = 0x7a4;
const LATCHED: u32 = 0x864;
const LATCHED_SEQ: u32 = 0x8a0;
const FROZEN: u32 = 0x3a0;
const VTABLE: u32 = 0x48;
const SET_HAND_HOLD: u32 = 0x118;
const STOP_ANIM: u32 = 0x110;
const START_ANIM: u32 = 0x10c;
/// The animation driving's own fields.
const MAX_CROSSUP: u32 = 0x9e4;
const POSE: u32 = 0x9e8;
const FORE_AFT: u32 = 0x9ec;
const FADED: u32 = 0x9f0;
const RAMP: u32 = 0x9f4;
const BLEND_RATE: u32 = 0x9f8;
/// The trick fields the clock section inside the gesture arm reads and writes.
const GROUP: u32 = 0x9fc;
const PREV_SEQ: u32 = 0xa08;
const IN_PROGRESS: u32 = 0xa0c;
const SAFE: u32 = 0xa10;
const HOLD_DONE: u32 = 0xa14;
const ELAPSED: u32 = 0xa1c;
const HOLD_ELAPSED: u32 = 0xa24;
const WIGGLE_TIME: u32 = 0xa28;
const PROGRESS: u32 = 0xa2c;
const HELD: u32 = 0xa30;
const STUNT_ARRAY: u32 = 0xa34;
const CURRENT: u32 = 0xa78;
const REQUESTED: u32 = 0xa7c;
const NO_HANDER_ID: u32 = 0xa80;
/// The clip the rider plays per action id: one entry per id, so the three airborne poses sit in the
/// entries of the ids whose arms do not look a clip up (the wheelie, the stoppie, and the id with no
/// name between the stoppie and "nothing").
const CLIP_ARRAY: u32 = 0xa88;
const RESTART: u32 = 0xaf8;
/// Inside one stunt record.
const RECORD: u32 = 0x54;

/// The motion object's own fields, which the library's dt path advances.
const M_FINISHED: u32 = 0x3c;
const M_SCRATCH: u32 = 0x58;
const M_CLIP: u32 = 0x48;
const M_SAMPLER: u32 = 0x4c;
const M_LAST: u32 = 0x50;
const M_PREV: u32 = 0x54;
const M_FADE_ON: u32 = 0x80;
const M_TIME: u32 = 0x84;
const M_CLOCK: u32 = 0x88;
const M_END: u32 = 0x8c;
const M_DELAY: u32 = 0x90;
const M_LENGTH: u32 = 0x94;
/// Inside one clip.
const C_KEYS: u32 = 0x14;
const C_CHANNELS: u32 = 0x1c;
const C_ROTATIONS: u32 = 0x20;
const C_POSITIONS: u32 = 0x24;
const C_FLAGS: u32 = 0x28;
const C_DURATION: u32 = 0x3c;

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

fn clear_object(ee: &mut Ee) {
    ee.load(OBJ, &vec![0u8; 0xb00]);
}

/// A stand-in for the engine's smooth noise, chosen so that both sides get the identical single from
/// the identical argument without either of them doing any arithmetic.
fn fake_noise(x: f32) -> f32 {
    const WAVE: [f32; 8] = [0.0, 1.0, -1.0, 0.5, -0.25, 0.937_5, -0.812_5, 0.125];
    WAVE[(x.to_bits() >> 3) as usize & 7]
}

/// Lay 24 stunt records out in RAM the way the rider's own copy is, and point the object's array at
/// them. The clock section inside the gesture arm reads the durations out of these.
fn write_records(ee: &mut Ee, stunts: &[Stunt]) {
    for (k, s) in stunts.iter().enumerate() {
        let at = RECORDS + k as u32 * RECORD;
        let mut name = [0u8; 0x20];
        let bytes = s.name.as_bytes();
        name[..bytes.len().min(0x1f)].copy_from_slice(&bytes[..bytes.len().min(0x1f)]);
        ee.load(at, &name);
        ee.set_f32(at + 0x40, s.safe_time.to_f32());
        ee.set_f32(at + 0x44, s.hold_time.to_f32());
        ee.set_f32(at + 0x48, s.duration.to_f32());
        ee.set_u32(at + 0x4c, u32::from(s.for_bike));
        ee.set_u32(at + 0x50, s.index as u32);
    }
    ee.set_u32(OBJ + STUNT_ARRAY, RECORDS);
}

/// One fabricated clip handle per action id, far enough apart to be told from each other in a
/// register. The entries the gesture arm looks up may be absent; the three airborne ones may not.
fn handle(id: u32) -> u32 {
    HANDLES + id * 0x10
}

/// Where a clip's own data goes, for the handle of the same id.
fn clip_at(id: u32) -> u32 {
    CLIPS + id * 0x80
}

/// The handle the port's clip slot names.
fn slot_handle(slot: Slot) -> u32 {
    match slot {
        Slot::Stunt(i) => handle(u32::from(i)),
        Slot::AirFwd => handle(25),
        Slot::AirMid => handle(26),
        Slot::AirBck => handle(27),
    }
}

// ---------------------------------------------------------------------------------------------------
// 1 — the driving
// ---------------------------------------------------------------------------------------------------

/// Which of the motion library's four entry points the frame called, and with what. `clip_b` is the
/// clip a two-clip blend mixes *from*; `ramp` is the cross-fade weight every blend takes.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct Call {
    mode: Option<u8>,
    w0: u32,
    w1: u32,
    ramp: u32,
    clip_a: u32,
    clip_b: u32,
}

/// What the port's answer looks like in the same shape, so a mismatch prints both sides the same way.
fn as_call(blend: Blend, dt: F) -> Call {
    match blend {
        Blend::Free => Call { mode: Some(0), w0: dt.to_bits(), ..Call::default() },
        Blend::One { phase, ramp, clip } => Call {
            mode: Some(1),
            w0: phase.to_bits(),
            ramp: ramp.to_bits(),
            clip_a: slot_handle(clip),
            ..Call::default()
        },
        Blend::Two { phase, mix, ramp, a, b } => Call {
            mode: Some(2),
            w0: phase.to_bits(),
            w1: mix.to_bits(),
            ramp: ramp.to_bits(),
            clip_a: slot_handle(a),
            clip_b: slot_handle(b),
        },
    }
}

#[test]
fn the_driving_matches_over_ten_thousand_frames() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&[
        "stunt_clocks",
        "smooth_noise",
        "anim_rewind",
        "motion_advance",
        "motion_blend_one",
        "motion_blend_two",
        "motion_blend_three",
    ]) else {
        return;
    };
    let (clocks, noise, snapshot) = (at[0], at[1], at[2]);
    let entries = [at[3], at[4], at[5], at[6]];

    // The engine's noise, and the pose snapshot a restart takes — which reads the rider's skeleton, so
    // a fabricated object cannot run it. Its own effect is proved by the restart it is called on.
    ee.hooks.insert(noise, Box::new(|ee| {
        let x = ee.f(12);
        ee.set_f(0, fake_noise(x));
    }));
    ee.hooks.insert(snapshot, Box::new(|_| {}));
    // The three vtable entries the clock section reaches, which this test's vtable is aimed at.
    for hook in [HOOK_HAND_HOLD, HOOK_STOP, HOOK_START] {
        ee.hooks.insert(hook, Box::new(|_| {}));
    }
    // The four library entry points. Everything this test reads has been written by the time one of
    // them is reached — and past it the original skins the rider through VU0 — so the hook records its
    // arguments and ends the whole call at the sentinel `Ee::call` waits for.
    let call = Rc::new(Cell::new(Call::default()));
    for (mode, entry) in entries.iter().enumerate() {
        let seen = Rc::clone(&call);
        let mode = [0u8, 1, 2, 4][mode];
        ee.hooks.insert(
            *entry,
            Box::new(move |ee| {
                seen.set(Call {
                    mode: Some(mode),
                    w0: ee.f(12).to_bits(),
                    w1: if mode == 2 { ee.f(13).to_bits() } else { 0 },
                    ramp: match mode {
                        1 => ee.f(13).to_bits(),
                        2 => ee.f(14).to_bits(),
                        _ => 0,
                    },
                    clip_a: if mode == 0 { 0 } else { ee.r(5) as u32 },
                    clip_b: if mode == 2 { ee.r(6) as u32 } else { 0 },
                });
                ee.set(31, u64::from(DONE));
            }),
        );
    }

    let mut pick = Pick(0x9e37_79b9_7f4a_7c15);
    let (mut cases, mut restarts, mut air, mut fallbacks) = (0, 0, 0, 0);
    for k in 0..10_000 {
        // A table whose clocks reach every branch, as the trick task's own proof generates it: the
        // phase a gesture's clip plays at is this table's doing.
        let no_hander = pick.upto(SLOTS as u32) as usize;
        let stunts: Vec<Stunt> = (0..SLOTS)
            .map(|i| Stunt {
                name: if i == no_hander { "nohandedlander".to_owned() } else { format!("stunt{i}") },
                safe_time: F(pick.float(2.0).abs()),
                hold_time: if pick.upto(4) == 0 { F::ZERO } else { F(pick.float(1.5).abs()) },
                duration: F(pick.float(2.0).abs()),
                index: i as i32,
                ..Stunt::default()
            })
            .collect();
        let set = Set::new(stunts.clone());

        // The ids past the 24 gesture slots on purpose: 24 and 26..=28 are the backflip, the stoppie,
        // the unnamed one and "nothing", and 25 is the airborne idle the landing is judged on.
        let id = match k % 10 {
            0 => 28,
            1 | 2 => 25,
            3 => 24,
            4 => 26,
            5 => 27,
            6 => 29 + pick.upto(4),
            _ => pick.upto(SLOTS as u32),
        };
        let latched = if pick.upto(3) == 0 { pick.upto(33) } else { id };
        // Which gesture slots the animation set has a clip for. The three airborne ones always.
        let present: Vec<bool> = (0..SLOTS).map(|_| pick.upto(5) != 0).collect();
        let hold = stunts[(latched as usize).min(SLOTS - 1)].hold_time.to_f32();
        let elapsed = match pick.upto(8) {
            0 => hold,
            1 => hold + 0.05,
            2 => hold + 0.1,
            3 => hold - 0.05,
            4 => hold - 0.000_01,
            5 => hold + 0.100_001,
            _ => pick.float(2.0),
        };
        let (seq, prev) = match pick.upto(4) {
            0 => (0u32, 0u32),
            1 => (1, 1),
            2 => (3, 3),
            _ => (pick.upto(5), pick.upto(5)),
        };
        let restart = pick.upto(3) == 0;
        let frozen = pick.upto(8) == 0;
        let group = if pick.upto(3) == 0 { 0 } else { pick.upto(7) };
        let ground = pick.upto(2);
        let hold_done = pick.upto(4) == 0;
        let in_progress = pick.upto(2) == 1;
        let held = match pick.upto(3) {
            0 => 0.0,
            1 => pick.float(0.5).abs(),
            _ => pick.float(0.5),
        };
        let hold_elapsed = pick.float(0.6).abs();
        let wiggle_time = pick.float(2.0).abs();
        let dt = match pick.upto(4) {
            0 => 1.0 / 60.0,
            1 => 1.0 / 30.0,
            2 => 0.0,
            _ => pick.float(0.2).abs(),
        };
        // The animation's own state and the two parameters. The blend rate is the rider's 3 most of
        // the time, and the crossup limit is zero often enough to reach the degenerate remap.
        let ramp0 = match pick.upto(4) {
            0 => 1.0,
            1 => 0.0,
            2 => pick.float(1.0).abs(),
            _ => pick.float(3.0),
        };
        let faded0 = pick.upto(2);
        let pose0 = pick.float(1.0);
        let shown0 = pick.float(1.0);
        let blend_rate = if pick.upto(3) == 0 { pick.float(20.0) } else { 3.0 };
        let max_crossup = match pick.upto(5) {
            0 => 0.0,
            1 => 0.785_398_2,
            _ => pick.float(2.0).abs(),
        };
        let lean = pick.float(1.5);
        let fore_aft_in = match pick.upto(5) {
            0 => 0.0,
            1 => -1.0,
            2 => 1.0,
            _ => pick.float(1.5),
        };
        let crossup = match pick.upto(4) {
            0 => max_crossup,
            1 => -max_crossup,
            _ => pick.float(3.0),
        };

        clear_object(&mut ee);
        write_records(&mut ee, &stunts);
        ee.load(ANIM_OBJ, &vec![0u8; 0x400]);
        ee.load(VTABLE_OBJ, &vec![0u8; 0x200]);
        ee.load(HANDLES, &vec![0u8; 0x400]);
        ee.load(CLIPS, &vec![0u8; 0x2000]);
        ee.set_u32(VTABLE_OBJ + SET_HAND_HOLD, HOOK_HAND_HOLD);
        ee.set_u32(VTABLE_OBJ + STOP_ANIM, HOOK_STOP);
        ee.set_u32(VTABLE_OBJ + START_ANIM, HOOK_START);
        ee.set_u32(OBJ + VTABLE, VTABLE_OBJ);
        ee.set_u32(OBJ + ANIM, ANIM_OBJ);
        ee.set_u32(OBJ + ANIM_STATE, ANIM_OBJ);
        for i in 0..32u32 {
            let there = if (i as usize) < SLOTS { present[i as usize] } else { i >= 25 && i <= 27 };
            ee.set_u32(OBJ + CLIP_ARRAY + i * 4, if there { handle(i) } else { 0 });
            ee.set_u32(handle(i), handle(i) + 8); // the joint map, which nothing here reads
            ee.set_u32(handle(i) + 4, clip_at(i));
        }
        ee.set_u32(OBJ + CURRENT, id);
        ee.set_u32(OBJ + LATCHED, latched);
        ee.set_u32(OBJ + REQUESTED, latched);
        ee.set_u32(OBJ + RESTART, u32::from(restart));
        ee.load(OBJ + LATCHED_SEQ, &[seq as u8]);
        ee.set_u32(OBJ + PREV_SEQ, prev);
        ee.set_u32(OBJ + FROZEN, u32::from(frozen));
        ee.set_u32(OBJ + GROUP, group);
        ee.set_u32(OBJ + GROUND, ground);
        ee.set_u32(OBJ + NO_HANDER_ID, no_hander as u32);
        ee.set_u32(OBJ + IN_PROGRESS, u32::from(in_progress));
        ee.set_u32(OBJ + HOLD_DONE, u32::from(hold_done));
        ee.set_u32(OBJ + SAFE, 1);
        ee.set_f32(OBJ + ELAPSED, elapsed);
        ee.set_f32(OBJ + HELD, held);
        ee.set_f32(OBJ + HOLD_ELAPSED, hold_elapsed);
        ee.set_f32(OBJ + WIGGLE_TIME, wiggle_time);
        ee.set_f32(OBJ + PROGRESS, 0.0);
        ee.set_f32(OBJ + RAMP, ramp0);
        ee.set_u32(OBJ + FADED, faded0);
        ee.set_f32(OBJ + POSE, pose0);
        ee.set_f32(OBJ + SHOWN_CROSSUP, shown0);
        ee.set_f32(OBJ + BLEND_RATE, blend_rate);
        ee.set_f32(OBJ + MAX_CROSSUP, max_crossup);
        ee.set_f32(OBJ + LEAN, lean);
        ee.set_f32(OBJ + FORE_AFT_IN, fore_aft_in);
        ee.set_f32(OBJ + CROSSUP, crossup);
        call.set(Call::default());

        // a1 = 0: the prologue reads the rider's skeleton, which a fabricated object does not have,
        // and it touches none of the driving.
        ee.call(clocks, &[u64::from(OBJ), 0], &[dt], BUDGET);
        let want = (
            ee.u32_at(OBJ + CURRENT),
            ee.f32_at(OBJ + RAMP).to_bits(),
            ee.u32_at(OBJ + FADED) != 0,
            ee.f32_at(OBJ + FORE_AFT).to_bits(),
            ee.f32_at(OBJ + POSE).to_bits(),
            ee.f32_at(OBJ + SHOWN_CROSSUP).to_bits(),
        );
        let progress = ee.f32_at(OBJ + PROGRESS);

        // The trick clocks first, which is the order the original runs them in: the phase a gesture's
        // clip plays at is their `progress`, so the driving is only right if they are.
        let mut t = Tricks::default();
        t.gesture.group = group as u8;
        t.current = Trick(id as u8);
        t.requested = Trick(latched as u8);
        t.restart = restart;
        t.seq = seq as i32;
        t.prev_seq = prev as i32;
        t.in_progress = in_progress;
        t.hold_done = hold_done;
        t.elapsed = F(elapsed);
        t.held = F(held);
        t.hold_elapsed = F(hold_elapsed);
        t.wiggle_time = F(wiggle_time);
        t.safe = true;
        let r = Request {
            action: Trick(latched as u8),
            force: restart,
            seq: seq as u8,
            prev_seq: prev as i32,
            frozen,
            progress: F::ZERO,
        };
        let bike = Bike { on_ground: ground != 0, ..Bike::default() };
        t.update(F(dt), &set, bike, frozen, |x| F(fake_noise(x.to_f32())));
        if Trick(latched as u8).slot().is_some() {
            assert_eq!(
                t.progress.to_bits(),
                progress.to_bits(),
                "case {k}: the trick clocks' progress, which is the phase the clip plays at"
            );
        }

        let mut d = Driver {
            action: Trick(id as u8),
            ramp: F(ramp0),
            faded: faded0 != 0,
            fore_aft: F::ZERO,
            pose: F(pose0),
            shown_crossup: F(shown0),
        };
        let mut stunt = [false; SLOTS];
        stunt.copy_from_slice(&present);
        let frame = d.frame(
            F(dt),
            Params { blend_rate: F(blend_rate), max_crossup: F(max_crossup) },
            Request { progress: t.progress, ..r },
            Pose { lean: F(lean), fore_aft: F(fore_aft_in), crossup: F(crossup) },
            Clips { stunt },
        );
        let got = (
            u32::from(d.action.0),
            d.ramp.to_bits(),
            d.faded,
            d.fore_aft.to_bits(),
            d.pose.to_bits(),
            d.shown_crossup.to_bits(),
        );
        let why = format!(
            "case {k}: id {id} latched {latched} seq {seq}/{prev} restart {restart} frozen {frozen} \
             rate {blend_rate:e} ramp {ramp0:e} max crossup {max_crossup:e} crossup {crossup:e} \
             lean {lean:e} fore-aft {fore_aft_in:e} dt {dt:e}"
        );
        assert_eq!(got, want, "{why}: (action, ramp, faded, fore_aft, pose, shown_crossup)");
        assert_eq!(
            as_call(frame.blend, F(dt)),
            call.get(),
            "{why}: which library entry, and with what"
        );
        assert_eq!(
            frame.restart,
            !frozen && (latched != id || restart || (seq >= 2 && seq != prev)),
            "{why}: whether the motion started over"
        );

        if frame.restart {
            restarts += 1;
        }
        if latched == 25 {
            air += 1;
        }
        if matches!(frame.blend, Blend::One { clip: Slot::AirMid, .. })
            && Trick(latched as u8).slot().is_some()
        {
            fallbacks += 1;
        }
        cases += 1;
    }
    // A generated case set is only a proof of what it reaches, so the three branches that are easy to
    // stop reaching by accident say so out loud.
    assert!(restarts > 100, "only {restarts} frames started a motion over");
    assert!(air > 100, "only {air} airborne frames");
    assert!(fallbacks > 100, "only {fallbacks} frames fell back on the neutral pose");
    println!(
        "the driving: {cases} frames, bit-exact ({restarts} restarted, {air} airborne, \
         {fallbacks} with no clip for the gesture)"
    );
}

// ---------------------------------------------------------------------------------------------------
// 2 — the frame advance
// ---------------------------------------------------------------------------------------------------

/// Where this test puts a motion object, and the one clip it plays.
const MOTION: u32 = 0x010c_0000;
const ONE_HANDLE: u32 = 0x010c_1000;
const ONE_CLIP: u32 = 0x010c_2000;

/// What one advance did that is not a field: the window it fired its action frames from, and the
/// transition's weight if one was running.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct Advanced {
    window: Option<(u32, u32, bool)>,
    weight: Option<u32>,
    plain: bool,
}

/// The port's answer in the same shape. The split of a wrapped window into its two halves is the event
/// entry's own doing, and proof 3 is where that is checked.
fn as_advanced(played: game_sim::motion::Played) -> Advanced {
    Advanced {
        window: match played.window {
            Window::None => None,
            Window::Span(a, b) => Some((a.to_bits(), b.to_bits(), false)),
            Window::Wrapped { tail, head } => Some((tail.0.to_bits(), head.1.to_bits(), true)),
        },
        weight: played.fade.map(|w| w.to_bits()),
        plain: false,
    }
}

#[test]
fn the_frame_advance_matches_over_ten_thousand_frames() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&[
        "motion_advance",
        "motion_events",
        "motion_transition_weighted",
        "motion_transition_plain",
    ]) else {
        return;
    };
    let (advance, events, weighted, plain) = (at[0], at[1], at[2], at[3]);

    let seen = Rc::new(Cell::new(Advanced::default()));
    let out = Rc::clone(&seen);
    ee.hooks.insert(events, Box::new(move |ee| {
        let mut a = out.get();
        a.window = Some((ee.f(12).to_bits(), ee.f(13).to_bits(), ee.r(6) != 0));
        out.set(a);
    }));
    let out = Rc::clone(&seen);
    ee.hooks.insert(weighted, Box::new(move |ee| {
        let mut a = out.get();
        a.weight = Some(ee.f(12).to_bits());
        out.set(a);
    }));
    let out = Rc::clone(&seen);
    ee.hooks.insert(plain, Box::new(move |_| {
        let mut a = out.get();
        a.plain = true;
        a.weight = None;
        out.set(a);
    }));

    let mut pick = Pick(0x0123_4567_89ab_cdef);
    let (mut cases, mut loops, mut ends, mut fades, mut inert) = (0, 0, 0, 0, 0);
    for k in 0..10_000 {
        let duration = match pick.upto(6) {
            0 => 0.0,
            1 => 1.0 / 30.0,
            2 => 0.5,
            _ => pick.float(3.0).abs(),
        };
        let flags = pick.next() & 0xff;
        let looping = flags & 1 != 0;
        // Clip times aimed at the end, because a uniform draw almost never lands near it.
        let dt = match pick.upto(4) {
            0 => 1.0 / 60.0,
            1 => 1.0 / 30.0,
            2 => 0.0,
            _ => pick.float(0.2).abs(),
        };
        let time0 = match pick.upto(6) {
            0 => duration,
            1 => duration - dt,
            2 => 0.0,
            3 => duration + 0.000_01,
            _ => pick.float(3.0).abs(),
        };
        let prev0 = pick.float(3.0).abs();
        let finished = pick.upto(6) == 0;
        let playing = pick.upto(8) != 0;
        let last0 = if pick.upto(3) == 0 { 0 } else { ONE_HANDLE };
        let fade_on = pick.upto(2) == 1;
        let fade_plain = pick.upto(4) == 0;
        let clock0 = pick.float(1.0).abs();
        let delay0 = match pick.upto(4) {
            0 => 0.0,
            1 => clock0,
            _ => pick.float(0.5).abs(),
        };
        let length0 = match pick.upto(4) {
            0 => 1.0,
            1 => 0.0,
            _ => pick.float(1.0).abs(),
        };
        let end0 = match pick.upto(4) {
            0 => clock0 + dt,
            1 => 0.0,
            _ => pick.float(1.5).abs(),
        };

        ee.load(MOTION, &vec![0u8; 0x300]);
        ee.load(ONE_CLIP, &vec![0u8; 0x80]);
        ee.set_u32(ONE_HANDLE, ONE_HANDLE + 8); // the joint map, which a channel-less clip never reads
        ee.set_u32(ONE_HANDLE + 4, ONE_CLIP);
        ee.set_f32(ONE_CLIP + C_DURATION, duration);
        ee.set_u32(ONE_CLIP + C_FLAGS, flags);
        ee.set_u32(ONE_CLIP + C_KEYS, 0); // no action frames: the window is read off the hook
        ee.set_u32(ONE_CLIP + C_ROTATIONS, 0);
        ee.set_u32(ONE_CLIP + C_POSITIONS, 0);
        ee.set_u32(ONE_CLIP + C_CHANNELS, 0);
        ee.set_u32(MOTION + M_CLIP, if playing { ONE_HANDLE } else { 0 });
        ee.set_u32(MOTION + M_FINISHED, u32::from(finished));
        ee.set_u32(MOTION + M_LAST, last0);
        ee.set_f32(MOTION + M_PREV, prev0);
        ee.set_f32(MOTION + M_TIME, time0);
        ee.set_u32(MOTION + M_FADE_ON, u32::from(fade_on));
        ee.set_u32(MOTION + M_SAMPLER, u32::from(fade_plain));
        ee.set_f32(MOTION + M_CLOCK, clock0);
        ee.set_f32(MOTION + M_DELAY, delay0);
        ee.set_f32(MOTION + M_LENGTH, length0);
        ee.set_f32(MOTION + M_END, end0);
        seen.set(Advanced::default());

        ee.call(advance, &[u64::from(MOTION), 0, 0, 0], &[dt], BUDGET);
        let want = (
            ee.f32_at(MOTION + M_TIME).to_bits(),
            ee.f32_at(MOTION + M_PREV).to_bits(),
            ee.u32_at(MOTION + M_LAST),
            ee.u32_at(MOTION + M_FINISHED) != 0,
            ee.u32_at(MOTION + M_FADE_ON) != 0,
            ee.f32_at(MOTION + M_CLOCK).to_bits(),
            ee.u32_at(MOTION + M_CLIP),
        );

        let mut p = Player {
            clip: playing.then_some(ClipId(ONE_HANDLE)),
            last: (last0 != 0).then_some(ClipId(ONE_HANDLE)),
            time: F(time0),
            prev: F(prev0),
            finished,
            fade: game_sim::motion::Fade {
                on: fade_on,
                plain: fade_plain,
                clock: F(clock0),
                delay: F(delay0),
                length: F(length0),
                end: F(end0),
            },
        };
        let clip = Clip { duration: F(duration), looping, keys: Vec::new() };
        let played = p.advance(F(dt), &clip);
        let got = (
            p.time.to_bits(),
            p.prev.to_bits(),
            p.last.map_or(0, |c| c.0),
            p.finished,
            p.fade.on,
            p.fade.clock.to_bits(),
            p.clip.map_or(0, |c| c.0),
        );
        let why = format!(
            "case {k}: duration {duration:e} flags {flags:#x} time {time0:e} prev {prev0:e} \
             dt {dt:e} finished {finished} playing {playing} last {last0:#x} fade {fade_on}/\
             {fade_plain} clock {clock0:e} delay {delay0:e} length {length0:e} end {end0:e}"
        );
        assert_eq!(got, want, "{why}: (time, prev, last, finished, fade on, clock, clip)");
        let mut answer = as_advanced(played);
        answer.plain = seen.get().plain;
        if answer.plain {
            answer.weight = None;
        }
        assert_eq!(answer, seen.get(), "{why}: (the action-frame window, the transition's weight)");

        if played.looped {
            loops += 1;
        }
        if played.finished {
            ends += 1;
        }
        if played.fade.is_some() {
            fades += 1;
        }
        if played.window == Window::None {
            inert += 1;
        }
        cases += 1;
    }
    assert!(loops > 100, "only {loops} frames looped");
    assert!(ends > 100, "only {ends} frames ran out of clip");
    assert!(fades > 100, "only {fades} frames were in a transition");
    assert!(inert > 100, "only {inert} frames did nothing");
    println!(
        "the frame advance: {cases} frames, bit-exact ({loops} looped, {ends} ended, \
         {fades} in a transition, {inert} inert)"
    );
}

// ---------------------------------------------------------------------------------------------------
// 3 — the action-frame window
// ---------------------------------------------------------------------------------------------------

/// The key list container and the one-item lists hanging off it.
const KEYLIST: u32 = 0x010a_7000;
const ITEMS: u32 = 0x010a_8000;
const SCRATCH: u32 = 0x010a_9000;
const CLIP3: u32 = 0x010a_a000;

/// Lay a clip's action frames out the way the original's reader expects them: a count, a stride-eight
/// key array, and one list of things to call per key — here exactly one thing, so a fired key can be
/// told from the object that was called.
fn write_keys(ee: &mut Ee, times: &[f32]) {
    ee.load(KEYS, &vec![0u8; 0x400]);
    ee.load(KEY_LISTS, &vec![0u8; 0x400]);
    ee.load(LISTS, &vec![0u8; 0x400]);
    ee.load(ITEMS, &vec![0u8; 0x400]);
    ee.load(EVENTS, &vec![0u8; 0x400]);
    for (i, &t) in times.iter().enumerate() {
        let i = i as u32;
        ee.set_f32(KEYS + i * 8, t);
        ee.set_u32(KEYS + i * 8 + 4, 0);
        ee.set_u32(ITEMS + i * 4, EVENTS + i * 0x10);
        ee.set_u32(EVENTS + i * 0x10, EVT_VTABLE);
        let list = LISTS + i * 0x20;
        ee.set_u32(list, 1); // one thing to call
        ee.set_u32(list + 8, ITEMS + i * 4);
        ee.set_u32(KEY_LISTS + i * 4, list);
    }
    ee.load(KEYLIST, &vec![0u8; 0x20]);
    ee.set_u32(KEYLIST, times.len() as u32);
    ee.set_u32(KEYLIST + 8, KEYS);
    ee.set_u32(KEYLIST + 0xc, KEY_LISTS);
}

/// An output container with room for everything, so no append has to grow it.
fn write_out(ee: &mut Ee) {
    ee.load(OUT, &vec![0u8; 0x20]);
    ee.load(OUT_DATA, &vec![0u8; 0x400]);
    ee.set_u32(OUT + 8, OUT_DATA);
    ee.set_u32(OUT + 0xc, 0x10);
    ee.set_u32(OUT + 0x10, 0x100);
    ee.set_u32(OUT + 0x14, 0x7fff_ffff);
}

/// The keys the original appended, as their indices.
fn appended(ee: &Ee) -> Vec<usize> {
    (0..ee.u32_at(OUT))
        .map(|i| ((ee.u32_at(OUT_DATA + i * 4) - EVENTS) / 0x10) as usize)
        .collect()
}

#[test]
fn the_action_frame_window_matches() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["motion_event_window", "motion_events", "new_array", "delete_array"])
    else {
        return;
    };
    let (window, events, alloc, free) = (at[0], at[1], at[2], at[3]);

    // The wrap branch builds a second list on the heap, which no test has started. A bump allocator
    // out of spare RAM is all it needs; the port models the two halves, not the container.
    let next = Rc::new(Cell::new(HEAP));
    let bump = Rc::clone(&next);
    ee.hooks.insert(alloc, Box::new(move |ee| {
        let at = bump.get();
        bump.set(at + ((ee.r(4) as u32 + 0xf) & !0xf));
        ee.set(2, u64::from(at));
    }));
    ee.hooks.insert(free, Box::new(|_| {}));
    // The action frames fire through each thing's own vtable. Both slots are hooked; the second takes
    // the thing itself, which is how this test knows which key fired.
    let fired = Rc::new(std::cell::RefCell::new(Vec::new()));
    ee.hooks.insert(HOOK_PRE, Box::new(|_| {}));
    let seen = Rc::clone(&fired);
    ee.hooks.insert(HOOK_FIRE, Box::new(move |ee| {
        seen.borrow_mut().push(((ee.r(4) as u32 - EVENTS) / 0x10) as usize);
    }));
    ee.load(EVT_VTABLE, &vec![0u8; 0x40]);
    ee.set_u32(EVT_VTABLE + 0x14, HOOK_FIRE);
    ee.set_u32(EVT_VTABLE + 0x18, HOOK_PRE);

    let mut pick = Pick(0xdead_beef_cafe_f00d);
    let (mut cases, mut hits, mut doubles) = (0, 0, 0);
    for k in 0..10_000 {
        // Sorted key times, with duplicates and exact zeroes allowed: a clip may fire two things on
        // the same frame of it.
        let n = 1 + pick.upto(12) as usize;
        let mut times: Vec<f32> = (0..n)
            .map(|_| match pick.upto(6) {
                0 => 0.0,
                1 => 0.5,
                2 => 1.0,
                _ => pick.float(2.0).abs(),
            })
            .collect();
        times.sort_by(|a, b| a.partial_cmp(b).expect("no NaN keys"));
        // The window, aimed at the keys themselves so that the inclusive ends are reached.
        let on_key = |p: &mut Pick| times[p.upto(n as u32) as usize];
        let from = match pick.upto(5) {
            0 => on_key(&mut pick),
            1 => 0.0,
            2 => on_key(&mut pick) - 0.000_01,
            _ => pick.float(2.0).abs(),
        };
        let to = match pick.upto(5) {
            0 => on_key(&mut pick),
            1 => from,
            2 => from + 1.0 / 30.0,
            _ => pick.float(2.0).abs(),
        };

        write_keys(&mut ee, &times);
        write_out(&mut ee);
        ee.call(window, &[u64::from(KEYLIST), u64::from(OUT)], &[from, to], BUDGET);

        let clip = Clip { duration: F(2.0), looping: false, keys: times.iter().map(|&t| F(t)).collect() };
        let got = clip.fired(Window::Span(F(from), F(to)));
        assert_eq!(
            got,
            appended(&ee),
            "case {k}: keys {times:?} window {from:e}..={to:e}: which action frames fired"
        );
        if !got.is_empty() {
            hits += 1;
        }
        // A key exactly on a sample time is in both that frame's window and the next one's.
        if times.contains(&to) && clip.fired(Window::Span(F(to), F(to) + F(1.0 / 30.0))).contains(
            &got.last().copied().unwrap_or(usize::MAX),
        ) {
            doubles += 1;
        }
        cases += 1;
    }
    assert!(hits > 1000, "only {hits} windows fired anything");
    assert!(doubles > 100, "only {doubles} keys fired on two frames running");
    println!("the action-frame window: {cases} windows, exact ({hits} fired, {doubles} fired twice)");

    // The wrapped window, through the entry that splits it: the tail of the clip first, then its head,
    // which is the order the port's two spans come in.
    let mut cases = 0;
    for k in 0..2_000 {
        let duration = 1.0 + pick.float(2.0).abs();
        let n = 1 + pick.upto(8) as usize;
        let mut times: Vec<f32> = (0..n).map(|_| pick.float(duration).abs()).collect();
        times.sort_by(|a, b| a.partial_cmp(b).expect("no NaN keys"));
        let from = pick.float(duration).abs();
        let to = pick.float(duration).abs();
        let wrapped = pick.upto(2) == 1;

        write_keys(&mut ee, &times);
        ee.load(MOTION, &vec![0u8; 0x300]);
        ee.load(SCRATCH, &vec![0u8; 0x40]);
        ee.load(CLIP3, &vec![0u8; 0x80]);
        ee.set_u32(MOTION + M_SCRATCH, SCRATCH);
        ee.set_u32(CLIP3 + C_KEYS, KEYLIST);
        ee.set_f32(CLIP3 + C_DURATION, duration);
        fired.borrow_mut().clear();
        next.set(HEAP);
        ee.call(
            events,
            &[u64::from(MOTION), u64::from(CLIP3), u64::from(wrapped)],
            &[from, to],
            BUDGET,
        );

        let clip = Clip {
            duration: F(duration),
            looping: true,
            keys: times.iter().map(|&t| F(t)).collect(),
        };
        let w = if wrapped {
            Window::Wrapped { tail: (F(from), F(duration)), head: (F::ZERO, F(to)) }
        } else {
            Window::Span(F(from), F(to))
        };
        assert_eq!(
            clip.fired(w),
            *fired.borrow(),
            "case {k}: keys {times:?} window {from:e}..={to:e} of {duration:e}, wrapped {wrapped}: \
             which action frames fired, in which order"
        );
        cases += 1;
    }
    println!("the split window: {cases} frames, exact");
}

// ---------------------------------------------------------------------------------------------------
// 4 — the frame counts
// ---------------------------------------------------------------------------------------------------

/// Thirty frames a second, which is what the game runs a race at.
const DT: F = F(1.0 / 30.0);

/// What the numbers above mean in frames. None of this is something an oracle call can say — each one
/// is a run of the port over whole frames, and the counts are the point: a motion that is one frame
/// out moves every action frame in it by one frame too.
#[test]
fn the_frame_counts_are_the_games() {
    // The cross-fade. The rider's own blend rate is 3 per second, so a restarted motion is fully faded
    // in after ten frames and not before.
    let p = Params { blend_rate: F(3.0), max_crossup: F(0.785_398_2) };
    let mut d = Driver { action: Trick::NONE, ramp: F::ONE, ..Driver::default() };
    let still = Request { action: Trick::NONE, ..Request::default() };
    let mut frames = 0;
    while d.ramp != F::ZERO {
        d.frame(DT, p, still, Pose::default(), Clips::default());
        frames += 1;
        assert!(frames <= 60, "the cross-fade never ended");
    }
    assert_eq!(frames, 10, "the cross-fade's length in frames at thirty a second");

    // A restart puts it back to one on the frame of the restart, so the fade starts from the whole
    // of the old pose rather than from wherever the last one got to.
    let asked = Request { action: Trick(3), ..Request::default() };
    let frame = d.frame(DT, p, asked, Pose::default(), Clips::default());
    assert!(frame.restart);
    assert_eq!(d.ramp, F::ONE);
    // And a frozen rider has no cross-fade at all, not even the one it was in the middle of.
    let frozen = Request { action: Trick(3), frozen: true, ..Request::default() };
    d.frame(DT, p, frozen, Pose::default(), Clips::default());
    assert_eq!(d.ramp, F::ZERO);
    assert!(!d.frame(DT, p, frozen, Pose::default(), Clips::default()).restart);
    // A held rider's idle writes the two pose fields itself, and the next frame takes the fore-aft
    // one straight back off the rider's lean.
    d.freeze();
    assert_eq!((d.pose, d.fore_aft), (F(0.5), F(0.5)));
    d.frame(DT, p, Request { action: Trick(3), ..Request::default() }, Pose { fore_aft: F::ONE, ..Pose::default() }, Clips::default());
    assert_eq!((d.pose, d.fore_aft), (F(0.5), F::ONE));

    // The airborne pose, which is what a landing is judged on: the crossup limit maps to the ends of
    // the sweep and straight ahead to the middle, and past the limit it stops rather than running on.
    let mut d = Driver { action: Trick::WHEELIE, ..Driver::default() };
    for (crossup, want) in [
        (F::ZERO, F(0.5)),
        (p.max_crossup, F::ONE),
        (-p.max_crossup, F::ZERO),
        (p.max_crossup * F(4.0), F::ONE),
        (-p.max_crossup * F(4.0), F::ZERO),
    ] {
        let r = Request { action: Trick::WHEELIE, ..Request::default() };
        d.frame(DT, p, r, Pose { crossup, ..Pose::default() }, Clips::default());
        assert_eq!(d.shown_crossup, want, "the shown crossup at {crossup:?}");
        assert_eq!(d.pose, want, "and the pose it also writes");
    }
    // With no limit set there is no sweep: everything reads as the near end.
    let flat = Params { max_crossup: F::ZERO, ..p };
    d.frame(
        DT,
        flat,
        Request { action: Trick::WHEELIE, ..Request::default() },
        Pose { crossup: F(1.0), ..Pose::default() },
        Clips::default(),
    );
    assert_eq!(d.shown_crossup, F::ZERO);

    // Which pair of airborne clips, and the mix. The middle clip is in both halves, so the mix runs
    // 0..1 twice across the rider's travel rather than once.
    for (fore_aft, a, b, mix) in [
        (F(-1.0), Slot::AirMid, Slot::AirFwd, F::ZERO),
        (F(-0.5), Slot::AirMid, Slot::AirFwd, F(0.5)),
        (F::ZERO, Slot::AirBck, Slot::AirMid, F::ZERO),
        (F(0.5), Slot::AirBck, Slot::AirMid, F(0.5)),
        (F::ONE, Slot::AirBck, Slot::AirMid, F::ONE),
    ] {
        let r = Request { action: Trick::WHEELIE, ..Request::default() };
        let got = d.frame(DT, p, r, Pose { fore_aft, ..Pose::default() }, Clips::default());
        match got.blend {
            Blend::Two { a: ga, b: gb, mix: gm, .. } => {
                assert_eq!((ga, gb, gm), (a, b, mix), "the airborne pair at fore-aft {fore_aft:?}");
            }
            other => panic!("the airborne idle blends two clips, not {other:?}"),
        }
    }

    // A stunt's clip is played at the trick's progress, not advanced by dt — so the frame an action
    // frame fires on is the frame the trick's clock reaches it, and a key that lands exactly on a
    // sample time fires on that frame and on the next one.
    let clip = Clip { duration: F::ONE, looping: false, keys: vec![F(0.25)] };
    let mut player = Player::default();
    let mut fired = Vec::new();
    for i in 0..32 {
        let phase = F(i as f32) * F(1.0 / 64.0); // exact in binary: no clock drift to hide behind
        let w = player.drive(phase, &clip, ClipId(1));
        if !clip.fired(w).is_empty() {
            fired.push(i);
        }
    }
    assert_eq!(fired, vec![16, 17], "the frames an action frame a quarter into a clip fires on");

    // The free-running advance, at the same thirty frames a second: a one-second looping clip comes
    // round on the frame after the one that reaches its end, and a non-looping one stops there and
    // then does nothing at all.
    let looping = Clip { duration: F::ONE, looping: true, keys: Vec::new() };
    let mut player = Player { clip: Some(ClipId(2)), ..Player::default() };
    let mut at = None;
    for i in 1..60 {
        if player.advance(DT, &looping).looped {
            at = Some(i);
            break;
        }
    }
    assert_eq!(at, Some(31), "the frame a one-second looping clip comes round on");

    let once = Clip { duration: F::ONE, looping: false, keys: vec![F::ONE] };
    let mut player = Player { clip: Some(ClipId(3)), ..Player::default() };
    let mut end = None;
    for i in 1..60 {
        let played = player.advance(DT, &once);
        // The last action frame of a clip fires on the frame that runs out of it, because the end is
        // tested after the advance and the window is inclusive.
        if played.finished {
            assert_eq!(clip_keys_fired(&once, played), vec![0]);
            end = Some(i);
            break;
        }
    }
    assert_eq!(end, Some(31), "the frame a one-second clip ends on");
    assert_eq!(player.advance(DT, &once), game_sim::motion::Played::default(), "and then nothing");
    assert!(player.finished);
}

/// The action frames one advance fired, so the assertion above reads as one line.
fn clip_keys_fired(clip: &Clip, played: game_sim::motion::Played) -> Vec<usize> {
    clip.fired(played.window)
}
