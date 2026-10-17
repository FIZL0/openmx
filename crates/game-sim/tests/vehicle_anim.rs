//! The vehicle's own animation state machine against the original.
//!
//! Four proofs: three of the one function S25 ported and of the two things only it reaches, and one
//! of the function that decides what to ask it for, which is what V11 came for.
//!
//! 1. **the state machine** — twenty-four thousand generated frames of the vehicle's animation step, one
//!    state at a time: every one of the sixty arms of its jump table and two states outside it, every
//!    arm of its fade-rate table, the two lags, the placement, the added-record path, and which of the
//!    motion library's six entry points the frame called with which clips and weights. The original
//!    ends in VU0 skinning that needs a real skeleton, so each call is stopped at the library entry,
//!    which is the last thing in front of it.
//! 2. **the clip table** — two thousand generated vehicles through the original's own loader, with the
//!    disc read and the allocating bind hooked onto a Rust-side shelf and everything between them
//!    running for real: the name each of the sixty slots is looked up by, in the order the table holds
//!    them.
//! 3. **the sixth entry point** — six thousand frames through the entry nothing but this machine
//!    calls, against the clock it drives and the action frames it fires: the one entry that gives each
//!    of its two clips its own phase.
//! 4. **the state chooser** — twelve thousand generated frames of the one writer of the state asked
//!    for, which is the caller of all of the above and was not ported with them: every arm of its
//!    priority, the fidget's countdown, the start line's draws against the list the whole start line
//!    shares, and the two fields it files away after the step. It calls the step itself, so the step
//!    is hooked to nothing and what is left running is the chooser.
//!
//! What is not proved here, and why, is in the journal entry's last section.

use game_sim::motion::{Animations, Clip, ClipId, Played, Player, Window};
use game_sim::rng::Rng;
use game_sim::trick::{SLOTS, Stunt};
use game_sim::vehicle_anim::{
    AIR, Asked, BACKFLIP, Blend, Driver, Extra, GET_READY, HOLD, IDLE, IDLES, Lag, Library,
    NO_GESTURE, NO_HANDER, POSE_CLIPS, Params, Place, REVERSE_L, REVERSE_R, Request, START_POS,
    START_POSES, STATES, Shared, TURN_RIDING, Update,
};
use ps2_float::{F, Vf};
use ps2_oracle::{DONE, Ee, addr, game_or_skip, skip};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Instruction budget per call. One frame of the step is a few hundred instructions once the library
/// entry is hooked; the reverse pair's `fmodf` is the longest thing in it.
const BUDGET: u64 = 200_000;
/// The loader builds sixty names with the engine's own `snprintf`, which is thousands of instructions
/// a time.
const LOAD_BUDGET: u64 = 20_000_000;

// ---------------------------------------------------------------------------------------------------
// where this test puts things
// ---------------------------------------------------------------------------------------------------

/// Well above the program image: the vehicle, its request block, the vector the placement turns, the
/// two lag records and the node the placement writes.
const VEH: u32 = 0x0120_0000;
const REQ: u32 = 0x0120_2000;
const OFFSET: u32 = 0x0120_3000;
const LAG_MIX: u32 = 0x0120_4000;
const LAG_PHASE: u32 = 0x0120_4100;
const NODE: u32 = 0x0120_5000;
/// One fabricated clip handle per animation state, and the clip behind each one.
const HANDLES: u32 = 0x0120_6000;
const CLIPS: u32 = 0x0120_8000;
/// The added-record list: the array of pointers the container holds, and the records themselves.
const EXTRA_LIST: u32 = 0x0120_c000;
const EXTRA: u32 = 0x0120_d000;

/// The vehicle's own fields, by the offsets the original uses. Named here because a test may say
/// where things live; the port may not.
const V_NODE: u32 = 0x30;
const V_SELF_A: u32 = 0x2ac;
const V_SELF_B: u32 = 0x2b0;
const V_REQUEST: u32 = 0x488;
const V_LAG_MIX: u32 = 0x630;
const V_LAG_PHASE: u32 = 0x634;
const V_FROM_TABLE: u32 = 0x680;
const V_HOLDING: u32 = 0x684;
const V_STARTED: u32 = 0x688;
const V_RAMP: u32 = 0x68c;
const V_RATE: u32 = 0x690;
const V_RATE_READY: u32 = 0x694;
const V_RATE_HOLD: u32 = 0x698;
const V_RATE_DEFAULT: u32 = 0x69c;
const V_RATE_POSE: u32 = 0x6a0;
const V_RATE_STUNT: u32 = 0x6a4;
const V_CROSS: u32 = 0x8dc;
const V_CROSS_PREV: u32 = 0x8e0;
const V_CROSS_START: u32 = 0x8e4;
const V_CROSS_CAP: u32 = 0x8e8;
const V_PREV: u32 = 0x8ec;
const V_STATE: u32 = 0x8f0;
const V_WANT: u32 = 0x8f4;
const V_START_PHASE: u32 = 0x8f8;
const V_SEQ: u32 = 0x8fc;
const V_CLIPS: u32 = 0x900;
const V_EXTRA_INDEX: u32 = 0x9f0;
/// The container at +0x9f4 keeps its array of pointers at its own +8.
const V_EXTRA_ARRAY: u32 = 0x9fc;
const V_BAND: u32 = 0xa14;
const V_REVERSE: u32 = 0xa3c;
/// The vehicle is its own motion object: these two are the handles it has bound.
const M_COUNT: u32 = 0x40;
const M_HANDLES: u32 = 0x44;

/// The request block the rest of the vehicle writes and the step only reads.
const R_AT_BASE: u32 = 0x8;
const R_HELD: u32 = 0xc;
const R_OFFSET: u32 = 0xf0;
const R_CUT: u32 = 0x140;
const R_PHASE_AXIS: u32 = 0x170;
const R_MIX_AXIS: u32 = 0x174;
const R_BAND: u32 = 0x178;
const R_BASE: u32 = 0x180;
const R_ROTATION: u32 = 0x190;
const R_PROGRESS: u32 = 0x1a4;
const R_SEQ: u32 = 0x1a8;

/// Inside one clip, and inside one stunt record.
const C_DURATION: u32 = 0x3c;
const RECORD: u32 = 0x54;

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

/// The handle for an animation state's clip, and the clip behind it.
fn handle(state: u32) -> u32 {
    HANDLES + state * 0x10
}
fn clip_at(state: u32) -> u32 {
    CLIPS + state * 0x80
}

/// Which state's clip a handle is, which is how the port's answer is read back as a table index.
fn state_of(h: u32) -> usize {
    ((h - HANDLES) / 0x10) as usize
}

fn put_v(ee: &mut Ee, a: u32, v: Vf) {
    for (k, x) in [v.x(), v.y(), v.z(), v.w()].into_iter().enumerate() {
        ee.set_f32(a + 4 * k as u32, x);
    }
}
fn get_v(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

/// A NUL-terminated string out of the oracle's RAM.
fn c_string(ee: &Ee, at: u32) -> String {
    let mut out = Vec::new();
    let mut a = at;
    loop {
        for b in ee.u32_at(a).to_le_bytes() {
            if b == 0 {
                return String::from_utf8_lossy(&out).into_owned();
            }
            out.push(b);
        }
        a += 4;
    }
}

/// The same, written in.
fn write_c_string(ee: &mut Ee, at: u32, s: &str) {
    let mut bytes = s.as_bytes().to_vec();
    bytes.push(0);
    ee.load(at, &bytes);
}

// ---------------------------------------------------------------------------------------------------
// 1 — the state machine
// ---------------------------------------------------------------------------------------------------

/// Which of the motion library's six entry points the frame called, and with what.
///
/// `w0`, `w1` and `w2` are the weights in the order the entry takes them — which is not the same
/// thing in every mode, because the pair at two phases takes a second *phase* where the four-clip
/// blend takes a second mix.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct Call {
    mode: Option<u8>,
    w0: u32,
    w1: u32,
    w2: u32,
    ramp: u32,
    a: u32,
    b: u32,
    c: u32,
    d: u32,
}

/// What the port's answer looks like in the same shape, so a mismatch prints both sides the same way.
fn as_call(blend: Blend, dt: F) -> Call {
    let h = |c: Option<ClipId>| c.map_or(0, |c| c.0);
    match blend {
        Blend::Free => Call { mode: Some(0), w0: dt.to_bits(), ..Call::default() },
        Blend::One { phase, ramp, clip } => Call {
            mode: Some(1),
            w0: phase.to_bits(),
            ramp: ramp.to_bits(),
            a: h(clip),
            ..Call::default()
        },
        Blend::Two { phase, mix, ramp, a, b } => Call {
            mode: Some(2),
            w0: phase.to_bits(),
            w1: mix.to_bits(),
            ramp: ramp.to_bits(),
            a: h(a),
            b: h(b),
            ..Call::default()
        },
        Blend::Four { phase, mix, cross, ramp, a, b, c, d } => Call {
            mode: Some(4),
            w0: phase.to_bits(),
            w1: mix.to_bits(),
            w2: cross.to_bits(),
            ramp: ramp.to_bits(),
            a: h(a),
            b: h(b),
            c: h(c),
            d: h(d),
        },
        Blend::Pair { phase_a, phase_b, mix, ramp, a, b } => Call {
            mode: Some(5),
            w0: phase_a.to_bits(),
            w1: phase_b.to_bits(),
            w2: mix.to_bits(),
            ramp: ramp.to_bits(),
            a: h(a),
            b: h(b),
            ..Call::default()
        },
    }
}

/// Everything the frame did that is not a field of the vehicle: where it put the node, whether it took
/// a pose snapshot, and the clip it put on.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct Did {
    placed: Option<([u32; 4], [u32; 4])>,
    snapshot: bool,
    played: Option<(u32, u32)>,
}

#[test]
fn the_state_machine_matches_over_twenty_four_thousand_frames() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&[
        "vehicle_animation",
        "transform_set_position",
        "transform_set_rotation",
        "anim_rewind",
        "motion_play",
        "motion_advance",
        "motion_blend_one",
        "motion_blend_two",
        "motion_blend_three",
        "motion_blend_pair",
    ]) else {
        return;
    };
    let step = at[0];
    let entries = [at[5], at[6], at[7], at[8], at[9]];

    // The placement's two calls. Both of them walk a transform chain a fabricated node does not have,
    // and what the step decided is in their arguments: the position quadword the first is handed and
    // the quaternion the second is pointed at.
    let did = Rc::new(Cell::new(Did::default()));
    for (which, &entry) in [at[1], at[2]].iter().enumerate() {
        let seen = Rc::clone(&did);
        ee.hooks.insert(
            entry,
            Box::new(move |ee| {
                let at = ee.r(if which == 0 { 6 } else { 5 }) as u32;
                let v = get_v(ee, at).to_bits();
                let mut d = seen.get();
                let (p, q) = d.placed.unwrap_or_default();
                d.placed = Some(if which == 0 { (v, q) } else { (p, v) });
                seen.set(d);
            }),
        );
    }
    // The pose snapshot a restart takes, which reads the vehicle's skeleton — so a fabricated vehicle
    // cannot run it. Whether it was asked for is the whole of what this proof wants from it.
    let seen = Rc::clone(&did);
    ee.hooks.insert(at[3], Box::new(move |_| {
        let mut d = seen.get();
        d.snapshot = true;
        seen.set(d);
    }));
    // Putting a clip on, which is proved field for field by the S20 proofs. Here it is the two
    // arguments the one group that asks for it passes.
    let seen = Rc::clone(&did);
    ee.hooks.insert(at[4], Box::new(move |ee| {
        let mut d = seen.get();
        d.played = Some((ee.f(12).to_bits(), ee.r(5) as u32));
        seen.set(d);
    }));
    // The six library entry points. Everything this test reads has been written by the time one of
    // them is reached — and past it the original skins the vehicle through VU0 — so the hook records
    // its arguments and ends the whole call at the sentinel `Ee::call` waits for.
    let call = Rc::new(Cell::new(Call::default()));
    for (i, &entry) in entries.iter().enumerate() {
        let seen = Rc::clone(&call);
        let mode = [0u8, 1, 2, 4, 5][i];
        ee.hooks.insert(
            entry,
            Box::new(move |ee| {
                let w = |n: usize| ee.f(n).to_bits();
                seen.set(Call {
                    mode: Some(mode),
                    w0: w(12),
                    w1: if mode >= 2 { w(13) } else { 0 },
                    w2: if mode >= 4 { w(14) } else { 0 },
                    ramp: match mode {
                        1 => w(13),
                        2 => w(14),
                        4 => w(15),
                        5 => w(15),
                        _ => 0,
                    },
                    a: if mode == 0 { 0 } else { ee.r(5) as u32 },
                    b: if mode >= 2 { ee.r(6) as u32 } else { 0 },
                    c: if mode == 4 { ee.r(7) as u32 } else { 0 },
                    d: if mode == 4 { ee.r(8) as u32 } else { 0 },
                });
                ee.set(31, u64::from(DONE));
            }),
        );
    }

    let mut pick = Pick(0x9e37_79b9_7f4a_7c15);
    let mut per_state = [0u32; 62];
    let mut per_mode = [0u32; 6];
    let (mut cases, mut restarts, mut placed, mut crossed, mut extras) = (0, 0, 0, 0, 0);
    // The two arms that write mode 4, counted apart: the task is only proved if both were reached.
    let (mut low_band, mut high_band) = (0, 0);
    for k in 0..24_000 {
        // Every state of the jump table in turn, and two outside it: 60 is one past the end and 101 is
        // far past it, both of which the range test in front of the table has to catch.
        let want: i32 = match k % 64 {
            60 => 60,
            61 => 101,
            // Twice over for the two arms that carry the most branches of their own: the pose grid,
            // which is where the four-clip blend's only two callers are, and the gesture slots.
            62 => TURN_RIDING,
            63 => pick.upto(SLOTS as u32) as i32,
            s => s as i32,
        };
        // The state that was running, and the one before that. Both inside the table: a negative one
        // is the one case the original does not survive (it reads behind its own table), and the
        // journal says so.
        let state0 = match pick.upto(4) {
            0 => want,
            1 => pick.upto(SLOTS as u32) as i32,
            2 => pick.upto(STATES as u32) as i32,
            _ => pick.upto(62) as i32,
        };
        let prev0 = match pick.upto(3) {
            0 => pick.upto(SLOTS as u32) as i32,
            1 => pick.upto(STATES as u32) as i32,
            _ => pick.upto(62) as i32,
        };
        let (seq, seq_byte) = match pick.upto(5) {
            0 => (0i32, 0u8),
            1 => (1, 1),
            2 => (3, 3),
            3 => (pick.upto(5) as i32, pick.upto(5) as u8),
            _ => (pick.upto(300) as i32, pick.next() as u8),
        };
        // What the step is about to decide, worked out here as well — the generator needs it to know
        // which two clips the stunt cross-fade is going to read a duration out of.
        let restart = want != state0 || (seq >= 2 && seq != i32::from(seq_byte));
        let prev = if restart { state0 } else { prev0 };

        let held = pick.upto(4) == 0;
        let at_base = pick.upto(3) == 0;
        let cut = pick.upto(3) == 0;
        let (self_a, self_b) = (pick.upto(3) != 0, pick.upto(3) != 0);
        let dt = match pick.upto(4) {
            0 => 1.0 / 60.0,
            1 => 1.0 / 30.0,
            2 => 0.0,
            _ => pick.float(0.2).abs(),
        };
        let ramp0 = match pick.upto(5) {
            0 => 1.0,
            1 => 0.0,
            2 => pick.float(1.0).abs(),
            _ => pick.float(3.0),
        };
        let (started0, holding0, from_table0) =
            (pick.upto(2) == 1, pick.upto(2) == 1, pick.upto(2) == 1);
        // The five fade rates, all distinct so the arm that picked one can be told from the others.
        let rates: [f32; 5] = [11.0, 22.0, 33.0, 44.0, 55.0];
        let rate0 = pick.float(60.0);
        // The pose grid's three thresholds, and the number they cut up. Each threshold is sometimes
        // equal to the one below it, which is the degenerate remap both interpolating bands skip.
        let b0 = match pick.upto(4) {
            0 => 0.0,
            _ => pick.float(5.0).abs(),
        };
        // Ascending, the way the three are configured — otherwise the band between the second and the
        // third is empty and the arm that interpolates over it is never reached.
        let b1 = match pick.upto(6) {
            0 => b0,
            1 => pick.float(10.0).abs(),
            _ => b0 + pick.float(5.0).abs(),
        };
        let b2 = match pick.upto(6) {
            0 => b1,
            1 => pick.float(20.0).abs(),
            _ => b1 + pick.float(5.0).abs(),
        };
        let band = match pick.upto(9) {
            0 => 0.0,
            1 => b0,
            2 => b1,
            3 => b2,
            4 => -pick.float(5.0).abs(),
            5 | 6 => (b1 + b2) * 0.5,
            _ => pick.float(20.0),
        };
        // The two axes: their records, their targets, and the "leave it" value a held request reads as
        // "do not snap this one".
        let axis_target = |pick: &mut Pick| match pick.upto(6) {
            0 => f32::MAX,
            1 => 0.0,
            2 => 1.0,
            3 => 0.5,
            _ => pick.float(1.5),
        };
        let (mix_target, phase_target) = (axis_target(&mut pick), axis_target(&mut pick));
        let axis_value = |pick: &mut Pick| match pick.upto(6) {
            0 => 0.0,
            1 => 0.5,
            2 => 0.499_999_97,
            3 => 1.0,
            _ => pick.float(1.5),
        };
        let (mix_value, phase_value) = (axis_value(&mut pick), axis_value(&mut pick));
        let tau = |pick: &mut Pick| match pick.upto(8) {
            0 => dt,
            1 => 1.0,
            _ => pick.float(0.5).abs(),
        };
        let (mix_tau, phase_tau) = (tau(&mut pick), tau(&mut pick));
        // The stunt cross-fade's own fields.
        let cross_cap = match pick.upto(4) {
            0 => 0.0,
            1 => 0.25,
            _ => pick.float(1.0).abs(),
        };
        let start_phase = pick.float(1.0).abs();
        let (cross0, cross_prev0, cross_start0) =
            (pick.float(1.5), pick.float(1.5), pick.float(1.5));
        let progress = match pick.upto(4) {
            0 => 0.0,
            1 => 1.0,
            _ => pick.float(1.0).abs(),
        };
        let reverse0 = pick.float(1.0);
        // One clip per state, some of them missing. The two the stunt cross-fade reads a duration out
        // of may not be: the original reads through the handle without testing it.
        let mut present: Vec<bool> = (0..STATES).map(|_| pick.upto(6) != 0).collect();
        if (0..SLOTS as i32).contains(&want) && (0..SLOTS as i32).contains(&prev) && !restart {
            present[want as usize] = true;
            present[prev as usize] = true;
        }
        let durations: Vec<f32> = (0..STATES)
            .map(|_| match pick.upto(6) {
                0 => 1.0,
                1 => 0.5,
                _ => pick.float(3.0).abs(),
            })
            .collect();
        // The added records: one of each shape, so the four arms of the container path are all reached
        // by the index that points at them. An index past the end of the list is the other case the
        // original does not survive.
        let records: Vec<(i32, u32, u32)> = (0..4)
            .map(|i| {
                let c = |pick: &mut Pick| handle(pick.upto(STATES as u32));
                (
                    pick.next() as i32,
                    if i & 1 == 0 { c(&mut pick) } else { 0 },
                    if i & 2 == 0 { c(&mut pick) } else { 0 },
                )
            })
            .collect();
        let extra: i32 = if pick.upto(8) == 0 { pick.upto(4) as i32 } else { -1 };
        // The placement's three vectors.
        let v = |pick: &mut Pick| {
            Vf::new(pick.float(100.0), pick.float(100.0), pick.float(100.0), pick.float(2.0))
        };
        let (offset, base) = (v(&mut pick), v(&mut pick));
        let rotation = match pick.upto(4) {
            0 => Vf::new(0.0, 0.0, 0.0, 1.0),
            1 => v(&mut pick),
            _ => {
                let (x, y, z, w) =
                    (pick.float(1.0), pick.float(1.0), pick.float(1.0), pick.float(1.0));
                let n = (x * x + y * y + z * z + w * w).sqrt();
                Vf::new(x / n, y / n, z / n, w / n)
            }
        };

        ee.load(VEH, &vec![0u8; 0xb00]);
        ee.load(REQ, &vec![0u8; 0x200]);
        ee.load(NODE, &vec![0u8; 0x200]);
        ee.load(HANDLES, &vec![0u8; 0x400]);
        ee.load(CLIPS, &vec![0u8; 0x2000]);
        ee.load(EXTRA_LIST, &vec![0u8; 0x100]);
        ee.load(EXTRA, &vec![0u8; 0x100]);
        for i in 0..STATES as u32 {
            ee.set_u32(handle(i), 0); // the joint map, which nothing here reads
            ee.set_u32(handle(i) + 4, clip_at(i));
            ee.set_f32(clip_at(i) + C_DURATION, durations[i as usize]);
            let h = if present[i as usize] { handle(i) } else { 0 };
            ee.set_u32(VEH + V_CLIPS + i * 4, h);
        }
        for (i, &(key, a, b)) in records.iter().enumerate() {
            let rec = EXTRA + i as u32 * 0x10;
            ee.set_u32(rec, key as u32);
            ee.set_u32(rec + 4, a);
            ee.set_u32(rec + 8, b);
            ee.set_u32(EXTRA_LIST + i as u32 * 4, rec);
        }
        ee.set_u32(VEH + V_EXTRA_ARRAY, EXTRA_LIST);
        ee.set_u32(VEH + V_EXTRA_INDEX, extra as u32);
        ee.set_u32(VEH + V_NODE, NODE);
        ee.set_u32(VEH + V_REQUEST, REQ);
        ee.set_u32(VEH + V_SELF_A, u32::from(self_a));
        ee.set_u32(VEH + V_SELF_B, u32::from(self_b));
        ee.set_u32(VEH + V_LAG_MIX, LAG_MIX);
        ee.set_u32(VEH + V_LAG_PHASE, LAG_PHASE);
        ee.set_u32(VEH + V_FROM_TABLE, u32::from(from_table0));
        ee.set_u32(VEH + V_HOLDING, u32::from(holding0));
        ee.set_u32(VEH + V_STARTED, u32::from(started0));
        ee.set_f32(VEH + V_RAMP, ramp0);
        ee.set_f32(VEH + V_RATE, rate0);
        ee.set_f32(VEH + V_RATE_READY, rates[0]);
        ee.set_f32(VEH + V_RATE_HOLD, rates[1]);
        ee.set_f32(VEH + V_RATE_DEFAULT, rates[2]);
        ee.set_f32(VEH + V_RATE_POSE, rates[3]);
        ee.set_f32(VEH + V_RATE_STUNT, rates[4]);
        ee.set_f32(VEH + V_CROSS, cross0);
        ee.set_f32(VEH + V_CROSS_PREV, cross_prev0);
        ee.set_f32(VEH + V_CROSS_START, cross_start0);
        ee.set_f32(VEH + V_CROSS_CAP, cross_cap);
        ee.set_u32(VEH + V_PREV, prev0 as u32);
        ee.set_u32(VEH + V_STATE, state0 as u32);
        ee.set_u32(VEH + V_WANT, want as u32);
        ee.set_f32(VEH + V_START_PHASE, start_phase);
        ee.load(VEH + V_SEQ, &[seq_byte]);
        ee.set_f32(VEH + V_BAND, b0);
        ee.set_f32(VEH + V_BAND + 4, b1);
        ee.set_f32(VEH + V_BAND + 8, b2);
        ee.set_f32(VEH + V_REVERSE, reverse0);
        for (at, value, t) in
            [(LAG_MIX, mix_value, mix_tau), (LAG_PHASE, phase_value, phase_tau)]
        {
            ee.set_f32(at, value);
            ee.set_f32(at + 4, t);
            ee.set_f32(at + 8, 0.0);
        }
        ee.set_u32(REQ + R_AT_BASE, u32::from(at_base));
        ee.set_u32(REQ + R_HELD, u32::from(held));
        ee.set_u32(REQ + R_OFFSET, OFFSET);
        ee.set_u32(REQ + R_CUT, u32::from(cut));
        ee.set_f32(REQ + R_PHASE_AXIS, phase_target);
        ee.set_f32(REQ + R_MIX_AXIS, mix_target);
        ee.set_f32(REQ + R_BAND, band);
        ee.set_f32(REQ + R_PROGRESS, progress);
        ee.set_u32(REQ + R_SEQ, seq as u32);
        put_v(&mut ee, OFFSET, offset);
        put_v(&mut ee, REQ + R_BASE, base);
        put_v(&mut ee, REQ + R_ROTATION, rotation);
        call.set(Call::default());
        did.set(Did::default());

        ee.call(step, &[u64::from(VEH)], &[dt], BUDGET);
        let want_fields = (
            ee.u32_at(VEH + V_PREV) as i32,
            ee.u32_at(VEH + V_STATE) as i32,
            ee.u32_at(VEH + V_STARTED) != 0,
            ee.u32_at(VEH + V_HOLDING) != 0,
            ee.u32_at(VEH + V_FROM_TABLE) != 0,
            ee.f32_at(VEH + V_RAMP).to_bits(),
            ee.f32_at(VEH + V_RATE).to_bits(),
            [
                ee.f32_at(VEH + V_CROSS).to_bits(),
                ee.f32_at(VEH + V_CROSS_PREV).to_bits(),
                ee.f32_at(VEH + V_CROSS_START).to_bits(),
            ],
            ee.f32_at(VEH + V_REVERSE).to_bits(),
            [
                ee.f32_at(LAG_MIX).to_bits(),
                ee.f32_at(LAG_MIX + 8).to_bits(),
                ee.f32_at(LAG_PHASE).to_bits(),
                ee.f32_at(LAG_PHASE + 8).to_bits(),
            ],
        );

        let mut lib = Library::default();
        for i in 0..STATES {
            if present[i] {
                lib.clip[i] = Some(ClipId(handle(i as u32)));
            }
        }
        lib.extra = records
            .iter()
            .map(|&(key, a, b)| Extra {
                key,
                a: (a != 0).then(|| ClipId(a)),
                b: (b != 0).then(|| ClipId(b)),
            })
            .collect();
        let mut d = Driver {
            want,
            state: state0,
            prev: prev0,
            seq: seq_byte,
            rate: F(rate0),
            ramp: F(ramp0),
            started: started0,
            holding: holding0,
            from_table: from_table0,
            cross: F(cross0),
            cross_prev: F(cross_prev0),
            cross_start: F(cross_start0),
            reverse: F(reverse0),
            extra,
            mix: Lag { value: F(mix_value), tau: F(mix_tau), ratio: F::ZERO },
            phase: Lag { value: F(phase_value), tau: F(phase_tau), ratio: F::ZERO },
        };
        let p = Params {
            rate_stunt: F(rates[4]),
            rate_default: F(rates[2]),
            rate_pose: F(rates[3]),
            rate_hold: F(rates[1]),
            rate_ready: F(rates[0]),
            cross_cap: F(cross_cap),
            start_phase: F(start_phase),
            band: [F(b0), F(b1), F(b2)],
        };
        let r = Request {
            held,
            at_base,
            cut,
            offset,
            base,
            rotation,
            phase_axis: F(phase_target),
            mix_axis: F(mix_target),
            band: F(band),
            progress: F(progress),
            seq,
        };
        let frame = d.frame(F(dt), &p, &r, &lib, self_a && self_b, |c| {
            F(durations[state_of(c.0)])
        });
        let got_fields = (
            d.prev,
            d.state,
            d.started,
            d.holding,
            d.from_table,
            d.ramp.to_bits(),
            d.rate.to_bits(),
            [d.cross.to_bits(), d.cross_prev.to_bits(), d.cross_start.to_bits()],
            d.reverse.to_bits(),
            [
                d.mix.value.to_bits(),
                d.mix.ratio.to_bits(),
                d.phase.value.to_bits(),
                d.phase.ratio.to_bits(),
            ],
        );
        let why = format!(
            "case {k}: want {want} state {state0} prev {prev0} seq {seq}/{seq_byte} \
             held {held} at_base {at_base} cut {cut} self-placed {self_a}/{self_b} extra {extra} \
             dt {dt:e} ramp {ramp0:e} band {band:e} of {b0:e}/{b1:e}/{b2:e} \
             mix {mix_value:e}->{mix_target:e}/{mix_tau:e} \
             phase {phase_value:e}->{phase_target:e}/{phase_tau:e} \
             progress {progress:e} cap {cross_cap:e}"
        );
        assert_eq!(
            got_fields, want_fields,
            "{why}: (prev, state, started, holding, from_table, ramp, rate, the cross-fade, \
             the reverse clock, the two axes)"
        );
        assert_eq!(as_call(frame.blend, F(dt)), call.get(), "{why}: which library entry, and with what");
        assert_eq!(
            Did {
                placed: frame.place.map(|Place { position, rotation }| {
                    (position.to_bits(), rotation.to_bits())
                }),
                snapshot: frame.restart,
                played: frame.play.map(|p| (p.length.to_bits(), p.clip.map_or(0, |c| c.0))),
            },
            did.get(),
            "{why}: where the node went, whether the pose was snapshotted, and the clip put on"
        );

        // Which arms may ask for which entry point, which is the shape of the whole table: only the
        // pose grid reaches the four-clip blend, and only a gesture crossing out of a gesture reaches
        // the pair at two phases.
        if matches!(frame.blend, Blend::Four { .. }) {
            assert_eq!(d.state, TURN_RIDING, "{why}: only the pose grid asks for four clips");
            if band < b0 {
                low_band += 1;
            } else {
                high_band += 1;
            }
        }
        if matches!(frame.blend, Blend::Pair { .. }) {
            assert!(
                (0..SLOTS as i32).contains(&d.state),
                "{why}: only a gesture crosses out of a gesture"
            );
        }

        per_state[want.clamp(0, 61) as usize] += 1;
        per_mode[frame.blend.mode() as usize] += 1;
        if frame.restart {
            restarts += 1;
        }
        if frame.place.is_some() {
            placed += 1;
        }
        if matches!(frame.blend, Blend::Pair { .. }) {
            crossed += 1;
        }
        if extra >= 0 {
            extras += 1;
        }
        cases += 1;
    }
    // A generated case set is only a proof of what it reaches, so every state of the table, and every
    // mode the table can ask for, says out loud how often it was reached.
    for (state, &n) in per_state.iter().enumerate() {
        assert!(n > 100, "state {state} was only reached {n} times");
    }
    for mode in [0usize, 1, 2, 4, 5] {
        assert!(per_mode[mode] > 200, "mode {mode} was only reached {} times", per_mode[mode]);
    }
    assert_eq!(per_mode[3], 0, "something asked for mode 3, which is not a case of the dispatch");
    assert!(restarts > 100, "only {restarts} frames started a state over");
    assert!(placed > 100, "only {placed} frames placed the bike");
    assert!(crossed > 100, "only {crossed} frames crossed out of a gesture");
    assert!(extras > 100, "only {extras} frames took an added record");
    assert!(low_band > 100, "only {low_band} frames reached the first writer of mode 4");
    assert!(high_band > 100, "only {high_band} frames reached the second writer of mode 4");
    println!(
        "the state machine: {cases} frames over {} states, bit-exact ({restarts} restarted, \
         {placed} placed, {crossed} crossed out of a gesture, {extras} off an added record, \
         per mode {per_mode:?}, mode 4 {low_band} under the first threshold and {high_band} \
         between the other two)",
        per_state.len()
    );
}

// ---------------------------------------------------------------------------------------------------
// 2 — the clip table
// ---------------------------------------------------------------------------------------------------

/// The vehicle, its records and the animation set the clips come out of.
const LOAD_VEH: u32 = 0x0130_0000;
const RECORDS: u32 = 0x0130_4000;
const LOAD_SET: u32 = 0x0130_5000;
/// The set's own array of pointers to clips, and the clips, and their name strings.
const SET_CLIPS: u32 = 0x0130_6000;
const SET_CLIP: u32 = 0x0130_8000;
const SET_NAMES: u32 = 0x0131_0000;
/// The vehicle's array of pointers to the handles it has bound, and the handles.
const VEH_HANDLES: u32 = 0x0131_4000;
const BOUND: u32 = 0x0131_5000;
/// The vehicle's name prefix, which is the loader's third argument.
const PREFIX: u32 = 0x0131_6000;

/// The port's side of the asset layer: which clips the disc has, which of them the set has reached,
/// and which of those the vehicle has a handle for — mirroring, field for field, what this proof's
/// two hooks do to the oracle's RAM.
struct Shelf {
    disc: Vec<String>,
    set: Vec<String>,
    bound: Vec<usize>,
}

impl Animations for Shelf {
    fn load(&mut self, name: &str) {
        let found = match self.set.iter().position(|n| n.eq_ignore_ascii_case(name)) {
            Some(clip) => Some(clip),
            None if self.disc.iter().any(|n| n.eq_ignore_ascii_case(name)) => {
                self.set.push(name.to_owned());
                Some(self.set.len() - 1)
            }
            None => None,
        };
        if let Some(clip) = found {
            self.bound.push(clip);
        }
    }

    fn bound(&self, name: &str) -> Option<ClipId> {
        let clip = self.set.iter().position(|n| n.eq_ignore_ascii_case(name))?;
        let handle = self.bound.iter().position(|&c| c == clip)?;
        Some(ClipId(BOUND + handle as u32 * 8))
    }
}

#[test]
fn the_clip_table_matches() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) =
        named(&["vehicle_load_clips", "anim_find_or_load", "anim_bind_clip", "anim_set"])
    else {
        return;
    };
    let (load, find_or_load, bind, set_global) = (at[0], at[1], at[2], at[3]);

    // The two halves of the asset layer the port asks for rather than ports: one reads the disc, the
    // other allocates three ways. Everything between them — the name each slot is looked up by, the
    // set search and the handle search — is the original's own code running for real.
    let disc: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let shelf = Rc::clone(&disc);
    ee.hooks.insert(find_or_load, Box::new(move |ee| {
        let (set, name) = (ee.r(4) as u32, c_string(ee, ee.r(6) as u32));
        let (count, clips) = (ee.u32_at(set + 0xc), ee.u32_at(set + 0x10));
        for i in 0..count {
            let clip = ee.u32_at(clips + i * 4);
            if c_string(ee, ee.u32_at(clip)).eq_ignore_ascii_case(&name) {
                ee.set(2, u64::from(clip));
                return;
            }
        }
        if !shelf.borrow().iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            ee.set(2, 0);
            return;
        }
        let (clip, text) = (SET_CLIP + count * 0x80, SET_NAMES + count * 0x40);
        write_c_string(ee, text, &name);
        ee.set_u32(clip, text);
        ee.set_u32(clips + count * 4, clip);
        ee.set_u32(set + 0xc, count + 1);
        ee.set(2, u64::from(clip));
    }));
    ee.hooks.insert(bind, Box::new(move |ee| {
        let (motion, clip) = (ee.r(4) as u32, ee.r(5) as u32);
        // The loader binds whatever the load answered, without looking: a name that is on neither the
        // disc nor in the set reaches the bind as a null. Nothing is bound for it — which is what the
        // port's shelf does, and what keeps the two sides' handle numbering the same.
        if clip == 0 {
            return;
        }
        let n = ee.u32_at(motion + M_COUNT);
        let handle = BOUND + n * 8;
        ee.set_u32(handle, 0); // the joint map, which nothing here reads
        ee.set_u32(handle + 4, clip);
        ee.set_u32(ee.u32_at(motion + M_HANDLES) + n * 4, handle);
        ee.set_u32(motion + M_COUNT, n + 1);
    }));

    let mut pick = Pick(0xdead_beef_0bad_f00d);
    let (mut cases, mut missing, mut unbound, mut truncated, mut poses) = (0, 0, 0, 0, 0);
    for k in 0..2_000 {
        // Names with the things that bite: mixed case, because the lookup is a stricmp; one long
        // enough to be cut off by the 32-byte buffer; and the flag the rider's loader gates on, which
        // this one does not.
        let prefix = ["", "mx", "Bike_", "a_rather_long_vehicle_name_prefix"][pick.upto(4) as usize];
        let long = format!("{:0>40}", pick.next());
        let names: Vec<String> = (0..SLOTS)
            .map(|i| match pick.upto(8) {
                0 => long.clone(),
                1 => format!("StUnT{}", i % 5),
                _ => format!("stunt{i}"),
            })
            .collect();
        let stunts: Vec<Stunt> = names
            .iter()
            .enumerate()
            .map(|(i, name)| Stunt {
                name: name.clone(),
                index: i as i32,
                for_bike: pick.upto(4) != 0,
                ..Stunt::default()
            })
            .collect();

        // Every name the loader will ask for, in table order: the gesture slots and then the poses.
        let full: Vec<String> = names
            .iter()
            .map(|n| Library::stunt_name(prefix, n))
            .chain(POSE_CLIPS.iter().map(|&n| n.to_owned()))
            .collect();
        // Nothing here loads a pose clip — the loader only ever looks those up — so the only way one
        // can be found is as part of the set the loader is handed, already bound.
        let mut on_disc = Vec::new();
        let mut in_set: Vec<(String, bool)> = Vec::new();
        for (i, name) in full.iter().enumerate() {
            let pose = i >= SLOTS;
            match pick.upto(8) {
                0 if !pose => {}                              // on neither: nothing to find
                1 => in_set.push((name.clone(), false)),      // in the set, nothing bound it
                2 => in_set.push((name.clone(), true)),       // in the set and already bound
                _ if pose => in_set.push((name.clone(), true)), // the poses, the usual way
                _ => on_disc.push(name.clone()),
            }
        }
        *disc.borrow_mut() = on_disc.clone();

        ee.load(LOAD_VEH, &vec![0u8; 0xb00]);
        ee.load(RECORDS, &vec![0u8; SLOTS * RECORD as usize]);
        ee.load(LOAD_SET, &vec![0u8; 0x40]);
        ee.load(SET_CLIPS, &vec![0u8; 0x400]);
        ee.load(SET_CLIP, &vec![0u8; 0x8000]);
        ee.load(SET_NAMES, &vec![0u8; 0x4000]);
        ee.load(VEH_HANDLES, &vec![0u8; 0x400]);
        ee.load(BOUND, &vec![0u8; 0x800]);
        for (i, s) in stunts.iter().enumerate() {
            let at = RECORDS + i as u32 * RECORD;
            let mut name = [0u8; 0x40];
            let bytes = s.name.as_bytes();
            name[..bytes.len().min(0x3f)].copy_from_slice(&bytes[..bytes.len().min(0x3f)]);
            ee.load(at, &name[..0x40.min(RECORD as usize)]);
            ee.set_u32(at + 0x4c, u32::from(s.for_bike));
            ee.set_u32(at + 0x50, s.index as u32);
        }
        ee.set_u32(LOAD_VEH + M_HANDLES, VEH_HANDLES);
        ee.set_u32(LOAD_SET + 0x10, SET_CLIPS);
        // The clips the set starts with, and the handles the vehicle already has for some of them: the
        // handle index is the order they were bound in, which both sides have to agree on.
        let mut pre_bound = Vec::new();
        for (i, (name, bound)) in in_set.iter().enumerate() {
            let (clip, text) = (SET_CLIP + i as u32 * 0x80, SET_NAMES + i as u32 * 0x40);
            write_c_string(&mut ee, text, name);
            ee.set_u32(clip, text);
            ee.set_u32(SET_CLIPS + i as u32 * 4, clip);
            if *bound {
                let handle = BOUND + pre_bound.len() as u32 * 8;
                ee.set_u32(handle + 4, clip);
                ee.set_u32(VEH_HANDLES + pre_bound.len() as u32 * 4, handle);
                pre_bound.push(i);
            }
        }
        ee.set_u32(LOAD_SET + 0xc, in_set.len() as u32);
        ee.set_u32(LOAD_VEH + M_COUNT, pre_bound.len() as u32);
        ee.set_u32(set_global, LOAD_SET);
        write_c_string(&mut ee, PREFIX, prefix);

        ee.call(
            load,
            &[u64::from(LOAD_VEH), u64::from(RECORDS), u64::from(PREFIX)],
            &[],
            LOAD_BUDGET,
        );

        let mut shelf = Shelf {
            disc: on_disc,
            set: in_set.iter().map(|(name, _)| name.clone()).collect(),
            bound: pre_bound,
        };
        let lib = Library::load(prefix, &stunts, &mut shelf);
        let why = format!("case {k}: prefix {prefix:?}");
        for state in 0..STATES {
            let want = ee.u32_at(LOAD_VEH + V_CLIPS + state as u32 * 4);
            assert_eq!(
                lib.clip[state].map_or(0, |c| c.0),
                want,
                "{why}: the handle for state {state} ({:?})",
                full[state]
            );
            if want == 0 {
                missing += 1;
                // The other way a state ends up empty: the set has the clip, but this vehicle never
                // bound it, so the handle search answers the same null.
                if shelf.set.iter().any(|n| n.eq_ignore_ascii_case(&full[state])) {
                    unbound += 1;
                }
            } else if state >= SLOTS {
                poses += 1;
            }
        }
        if names.iter().any(|n| n == &long) {
            truncated += 1;
        }
        cases += 1;
    }
    assert!(missing > 100, "only {missing} states had no clip");
    assert!(unbound > 100, "only {unbound} states had a clip in the set that nothing bound");
    assert!(truncated > 100, "only {truncated} cases had a name long enough to be cut off");
    assert!(poses > 100, "only {poses} pose clips were found");
    println!(
        "the clip table: {cases} vehicles x {STATES} states, exact ({missing} empty states, \
         {poses} pose clips found, {truncated} cases with a truncated name)"
    );
}

// ---------------------------------------------------------------------------------------------------
// 3 — the sixth entry point
// ---------------------------------------------------------------------------------------------------

/// Proof 3's motion object, its two clip handles and their clips.
const PAIR_MOTION: u32 = 0x0140_0000;
const PAIR_HANDLES: u32 = 0x0140_1000;
const PAIR_CLIPS: u32 = 0x0140_2000;

const M_FINISHED: u32 = 0x3c;
const M_CLIP: u32 = 0x48;
const M_LAST: u32 = 0x50;
const M_PREV: u32 = 0x54;

fn pair_handle(i: u32) -> u32 {
    PAIR_HANDLES + i * 0x10
}
fn pair_clip(i: u32) -> u32 {
    PAIR_CLIPS + i * 0x80
}

#[test]
fn the_sixth_entry_point_drives_the_clock_off_its_first_clip() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["motion_blend_pair", "motion_events", "motion_slerp"]) else { return };
    let (entry, events, slerp) = (at[0], at[1], at[2]);

    // The action frames, which is where the clip time the entry decided on shows up.
    let window: Rc<Cell<Option<(u32, u32, bool)>>> = Rc::new(Cell::new(None));
    let out = Rc::clone(&window);
    ee.hooks.insert(events, Box::new(move |ee| {
        out.set(Some((ee.f(12).to_bits(), ee.f(13).to_bits(), ee.r(6) != 0)));
    }));
    // The quaternion blend, which a clip with no channels never reaches. Hooked so that a case which
    // did reach it could not wander off into the renderer's half of the library.
    ee.hooks.insert(slerp, Box::new(|_| {}));

    let mut pick = Pick(0x0246_8ace_1357_9bdf);
    let (mut cases, mut fresh, mut wraps, mut empty) = (0, 0, 0, 0);
    for k in 0..6_000 {
        let durations: Vec<f32> = (0..2)
            .map(|_| match pick.upto(4) {
                0 => 1.0,
                1 => 0.0,
                _ => pick.float(3.0).abs(),
            })
            .collect();
        // The two phases: the first is the one the clock is driven at, the second belongs to the clip
        // the cross-fade is coming out of and should reach nothing this proof can see.
        let phase = |pick: &mut Pick| match pick.upto(5) {
            0 => 0.0,
            1 => 1.0,
            2 => 0.5,
            _ => pick.float(1.0).abs(),
        };
        let (phase_a, phase_b) = (phase(&mut pick), phase(&mut pick));
        let (mix, ramp) = (pick.float(1.0).abs(), pick.float(1.0).abs());
        let dt = match pick.upto(3) {
            0 => 1.0 / 30.0,
            1 => 0.0,
            _ => pick.float(0.2).abs(),
        };
        // Which clip the last frame sampled, which decides whether this frame's action frames start
        // from the last sample or from the beginning of the clip.
        let last = match pick.upto(4) {
            0 => 0,
            1 => pair_handle(1),
            _ => pair_handle(0),
        };
        let prev = match pick.upto(4) {
            0 => 0.0,
            1 => durations[0] * phase_a,
            _ => pick.float(3.0).abs(),
        };
        let playing = pick.upto(4) != 0;
        let finished = pick.upto(8) == 0;

        ee.load(PAIR_MOTION, &vec![0u8; 0x300]);
        ee.load(PAIR_HANDLES, &vec![0u8; 0x40]);
        ee.load(PAIR_CLIPS, &vec![0u8; 0x200]);
        for i in 0..2 {
            ee.set_u32(pair_handle(i), 0); // the joint map, which a channel-less clip never reads
            ee.set_u32(pair_handle(i) + 4, pair_clip(i));
            ee.set_f32(pair_clip(i) + C_DURATION, durations[i as usize]);
        }
        ee.set_u32(PAIR_MOTION + M_CLIP, if playing { pair_handle(0) } else { 0 });
        ee.set_u32(PAIR_MOTION + M_FINISHED, u32::from(finished));
        ee.set_u32(PAIR_MOTION + M_LAST, last);
        ee.set_f32(PAIR_MOTION + M_PREV, prev);
        for word in 0..4 {
            ee.set_u32(ee.stack + word * 4, 0);
        }
        window.set(None);

        ee.call(
            entry,
            &[
                u64::from(PAIR_MOTION),
                u64::from(pair_handle(0)),
                u64::from(pair_handle(1)),
                0,
                0,
                0,
                0,
                0,
            ],
            &[phase_a, phase_b, mix, ramp],
            BUDGET,
        );
        let want = (
            ee.u32_at(PAIR_MOTION + M_CLIP),
            ee.u32_at(PAIR_MOTION + M_LAST),
            ee.f32_at(PAIR_MOTION + M_PREV).to_bits(),
            window.get(),
        );

        let blend = Blend::Pair {
            phase_a: F(phase_a),
            phase_b: F(phase_b),
            mix: F(mix),
            ramp: F(ramp),
            a: Some(ClipId(pair_handle(0))),
            b: Some(ClipId(pair_handle(1))),
        };
        let mut player = Player {
            clip: playing.then_some(ClipId(pair_handle(0))),
            last: (last != 0).then_some(ClipId(last)),
            prev: F(prev),
            finished,
            ..Player::default()
        };
        // The clip behind the handle the call works on, which the port names rather than being handed.
        let driven = blend.clock_clip(&player).map_or(0, |c| c.0);
        assert_eq!(driven, pair_handle(0), "case {k}: the clock came off the wrong clip");
        let clip = Clip {
            duration: F(durations[0]),
            looping: false,
            keys: Vec::new(),
        };
        let played = blend.run(&mut player, F(dt), &clip);
        let got = (
            player.clip.map_or(0, |c| c.0),
            player.last.map_or(0, |c| c.0),
            player.prev.to_bits(),
            match played.window {
                Window::None => None,
                Window::Span(x, y) => Some((x.to_bits(), y.to_bits(), false)),
                Window::Wrapped { tail, head } => {
                    Some((tail.0.to_bits(), head.1.to_bits(), true))
                }
            },
        );
        assert_eq!(
            got, want,
            "case {k}: phases {phase_a:e}/{phase_b:e} mix {mix:e} ramp {ramp:e} \
             durations {durations:?} last {last:#x} prev {prev:e} playing {playing} \
             finished {finished}: (the free-running clip, the last sampled clip, its time, \
             the action frames)"
        );

        if last != pair_handle(0) {
            fresh += 1;
        }
        if matches!(played.window, Window::Wrapped { .. }) {
            wraps += 1;
        }
        if played.window == Window::Span(F::ZERO, F::ZERO) {
            empty += 1;
        }
        cases += 1;
    }
    assert!(fresh > 100, "only {fresh} frames started a clip the last frame was not on");
    assert!(wraps > 100, "only {wraps} frames went backwards into a wrap");
    assert!(empty > 100, "only {empty} frames fired the empty window");
    println!(
        "the sixth entry point: {cases} frames, bit-exact ({fresh} onto a new clip, {wraps} \
         wrapped, {empty} firing the empty window)"
    );

    // The one case the original does not survive: a blend whose first clip has no handle. It hands the
    // entry point a null and the entry reads through it; the port does nothing at all.
    let mut player = Player { clip: Some(ClipId(pair_handle(0))), ..Player::default() };
    let blend = Blend::Pair {
        phase_a: F(0.5),
        phase_b: F(0.5),
        mix: F(0.5),
        ramp: F::ONE,
        a: None,
        b: None,
    };
    let clip = Clip { duration: F::ONE, looping: false, keys: Vec::new() };
    assert_eq!(blend.run(&mut player, F(1.0 / 30.0), &clip), Played::default());
    assert_eq!(player.clip, Some(ClipId(pair_handle(0))), "and it leaves the clock alone");
}

// ---------------------------------------------------------------------------------------------------
// 4 — the state chooser
// ---------------------------------------------------------------------------------------------------

/// The chooser's own fields of the object, past the step's.
const V_FINISHED: u32 = 0x3c;
const V_SETTLED: u32 = 0x6b4;
const V_START_PICKED: u32 = 0xa0c;
const V_STILL_BAND: u32 = 0xa10;
const V_BREATHE: u32 = 0xa28;
const V_IDLE_PLAYING: u32 = 0xa2c;
const V_IDLE_WAIT: u32 = 0xa30;
const V_WAIT_LO: u32 = 0xa34;
const V_WAIT_HI: u32 = 0xa38;

/// The chooser's own fields of the request block, past the step's.
const R_AIRBORNE: u32 = 0xf8;
const R_FLIPPING: u32 = 0xfc;
const R_NO_HANDS: u32 = 0x100;
const R_WRECKED: u32 = 0x104;
const R_SETTLE: u32 = 0x144;
const R_COMMANDED: u32 = 0x160;
const R_REVERSING: u32 = 0x16c;
const R_GESTURE: u32 = 0x1a0;

/// How long the list of start poses already handed out is.
const DRAWN: u32 = IDLES as u32;

/// Everything the chooser decided, in one shape so a mismatch prints both sides the same way.
///
/// The three floats are compared as bits, not as floats: a countdown that is `-0.0` where the
/// original has `0.0` is a different word in a save state and would be a different draw next frame.
#[derive(Clone, PartialEq, Debug)]
struct Chose {
    want: i32,
    idle_wait: u32,
    idle_playing: bool,
    start_picked: bool,
    breathe: u32,
    finished: bool,
    cut: bool,
    start_phase: u32,
    seq: u8,
    drawn: Vec<i32>,
    rand_state: u64,
}

#[test]
fn the_state_chooser_matches_over_twelve_thousand_frames() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&[
        "vehicle_anim_want",
        "vehicle_animation",
        "vehicle_anim_rand_seeded",
        "vehicle_anim_start_idles",
        "rand_state",
    ]) else {
        return;
    };
    let (chooser, step, seeded, drawn, rand_state) = (at[0], at[1], at[2], at[3], at[4]);
    // The list's count is six words past its ten entries.
    let drawn_count = drawn + DRAWN * 4;

    // The step, which the chooser calls in its own tail and which the first proof above has already
    // taken apart frame by frame. Hooked to nothing at all, so what is left running is the chooser:
    // the hook returns to `ra`, which is the instruction after the call, and the two fields the tail
    // writes after it are still written. Everything the chooser reads it reads *before* the call, so
    // a step that does nothing cannot change what it chose.
    let stepped = Rc::new(Cell::new(0u32));
    let seen = Rc::clone(&stepped);
    ee.hooks.insert(step, Box::new(move |_| seen.set(seen.get() + 1)));

    let mut pick = Pick(0x2545_f491_4f6c_dd1d);
    let mut per_want = [0u32; 62];
    let (mut gate, mut disarmed, mut fidgets, mut rearmed) = (0, 0, 0, 0);
    let (mut reset_draw, mut searched_draw, mut held_draw, mut fidget_ended) = (0, 0, 0, 0);
    for k in 0..12_000 {
        // Which half of the chooser this frame is in: on the gate, or riding. The flag is the request
        // block's own, and the one thing in the port that writes it is the chooser clearing it.
        let cut = k % 3 == 0;
        let still = !cut && k % 5 == 0;
        let dt = match pick.upto(4) {
            0 => 1.0 / 60.0,
            1 => 1.0 / 30.0,
            2 => 0.0,
            _ => pick.float(0.2).abs(),
        };
        // The state asked for last frame. The wreck's hold poses are drawn once and the test that
        // makes them drawn once is on this, and the start line's second pose is drawn only out of the
        // breathe, so both of those values have to come up often.
        let want0: i32 = match pick.upto(6) {
            0 => GET_READY,
            1 => HOLD,
            2 => HOLD + 1 + pick.upto(5) as i32,
            3 => pick.upto(STATES as u32) as i32,
            4 => TURN_RIDING,
            _ => pick.next() as i32,
        };
        let seq0 = pick.next() as u8;
        // The start line's own three fields.
        let breathe0 = match pick.upto(5) {
            0 => 0.0,
            1 => -pick.float(1.0).abs(),
            2 => dt,
            _ => pick.float(3.5).abs(),
        };
        let start_picked = pick.upto(3) != 0;
        let finished0 = pick.upto(2) == 1;
        let settle = match pick.upto(4) {
            0 => 0.0,
            1 => 3.5,
            _ => pick.float(5.0),
        };
        let rate_ready = pick.float(60.0);
        // Riding, in the order the chooser tests them.
        let wrecked = pick.upto(4) == 0;
        let settled = pick.upto(2) == 1;
        let gesture: i32 = match pick.upto(4) {
            0 | 1 => NO_GESTURE,
            2 => pick.upto(SLOTS as u32) as i32,
            // The chooser copies this field into `want` with no range test of any kind, which is
            // worth proving rather than assuming.
            _ => pick.next() as i32,
        };
        let no_hands = pick.upto(3) == 0;
        let flipping = pick.upto(4) == 0;
        let airborne = pick.upto(4) == 0;
        let reversing = match pick.upto(3) {
            0 => 0.0,
            1 => -pick.float(10.0).abs(),
            _ => pick.float(10.0),
        };
        let commanded = match pick.upto(3) {
            0 => 0.0,
            _ => pick.float(1.0),
        };
        // Standing still: the band, the still band it is tested against, and the two axes, which the
        // chooser wants at *exactly* a half. The reverse pair reads the phase axis as well.
        let still_band = match pick.upto(4) {
            0 => 1.466_666_7,
            1 => 0.0,
            _ => pick.float(5.0).abs(),
        };
        let b0 = match pick.upto(4) {
            0 => 7.333_333,
            1 => 0.0,
            _ => pick.float(10.0).abs(),
        };
        let band = match pick.upto(6) {
            0 => 0.0,
            1 => b0,
            2 => still_band,
            3 => -pick.float(1.0).abs(),
            _ => pick.float(20.0),
        };
        let axis = |pick: &mut Pick| match pick.upto(4) {
            0 | 1 => 0.5,
            2 => 0.499_999_97,
            _ => pick.float(1.5),
        };
        let (phase_axis, mix_axis) = (axis(&mut pick), axis(&mut pick));
        // The fidget's countdown and the range it is re-armed to.
        let idle_wait0 = match pick.upto(6) {
            0 => 0.0,
            1 => dt,
            2 => -pick.float(1.0).abs(),
            // Within a frame of running out, which is the only way a fidget is ever started: a
            // countdown drawn from the real fifteen-to-forty-five seconds needs nine hundred frames
            // to get there and this proof gives each case one.
            3 | 4 => pick.float(dt * 2.0).abs(),
            _ => pick.float(45.0).abs(),
        };
        let idle_playing = pick.upto(3) == 0;
        let (wait_lo, wait_hi) = match pick.upto(4) {
            0 => (15.0, 45.0),
            // Backwards, which the engine's remap clamps the other way round.
            1 => (45.0, 15.0),
            2 => (pick.float(30.0), pick.float(30.0)),
            _ => (0.0, 0.0),
        };
        // The tail's two inputs.
        let progress = pick.float(1.0).abs();
        let seq = pick.next() as i32;
        // The start poses other bodies on the line have taken: empty, part full, and exactly full,
        // which is the case that throws the list away. A list of distinct entries is what the game
        // can produce; one with repeats is fed in too, because the chooser's search does not care.
        let count = match pick.upto(5) {
            0 => 0,
            1 => DRAWN,
            2 => DRAWN - 1,
            _ => pick.upto(DRAWN),
        };
        let list: Vec<i32> = {
            let mut out = Vec::new();
            while out.len() < count as usize {
                let p = IDLE + pick.upto(DRAWN) as i32;
                if pick.upto(8) == 0 || !out.contains(&p) {
                    out.push(p);
                }
            }
            out
        };
        // One case in five is forced to be *standing still*, which is the arm that ends in the pose
        // grid and the fidget. Every one of its ten conditions has to hold at once — nothing
        // wrecked, asked for, flipping, airborne, reversing or commanded, both axes at exactly a
        // half and the band under the still band — so a generator that draws them independently
        // gets there twice in twelve thousand frames and proves nothing about the countdown.
        let (wrecked, gesture, flipping, airborne) =
            if still { (false, NO_GESTURE, false, false) } else { (wrecked, gesture, flipping, airborne) };
        let (reversing, commanded) = if still { (0.0, 0.0) } else { (reversing, commanded) };
        let (phase_axis, mix_axis) = if still { (0.5, 0.5) } else { (phase_axis, mix_axis) };
        let (still_band, band) = if still { (1.466_666_7, pick.float(1.0)) } else { (still_band, band) };
        // Above the band the frame is standing in, so the disarm at the end of the chooser does not
        // take back what the fidget just did.
        let b0 = if still { 7.333_333 } else { b0 };

        // The generator, mid-chain: a chooser proved from state 1 would only ever prove its first
        // draw.
        let state0 = u64::from(pick.next()) << 32 | u64::from(pick.next());

        ee.load(VEH, &vec![0u8; 0xb00]);
        ee.load(REQ, &vec![0u8; 0x200]);
        ee.set_u32(VEH + V_REQUEST, REQ);
        ee.set_u32(VEH + V_FINISHED, u32::from(finished0));
        ee.set_u32(VEH + V_SETTLED, u32::from(settled));
        ee.set_f32(VEH + V_RATE_READY, rate_ready);
        ee.set_u32(VEH + V_WANT, want0 as u32);
        ee.set_f32(VEH + V_START_PHASE, pick.float(1.0));
        ee.load(VEH + V_SEQ, &[seq0]);
        ee.set_f32(VEH + V_BAND, b0);
        ee.set_u32(VEH + V_START_PICKED, u32::from(start_picked));
        ee.set_f32(VEH + V_STILL_BAND, still_band);
        ee.set_f32(VEH + V_BREATHE, breathe0);
        ee.set_u32(VEH + V_IDLE_PLAYING, u32::from(idle_playing));
        ee.set_f32(VEH + V_IDLE_WAIT, idle_wait0);
        ee.set_f32(VEH + V_WAIT_LO, wait_lo);
        ee.set_f32(VEH + V_WAIT_HI, wait_hi);
        ee.set_u32(REQ + R_CUT, u32::from(cut));
        ee.set_f32(REQ + R_PHASE_AXIS, phase_axis);
        ee.set_f32(REQ + R_MIX_AXIS, mix_axis);
        ee.set_f32(REQ + R_BAND, band);
        ee.set_f32(REQ + R_PROGRESS, progress);
        ee.set_u32(REQ + R_SEQ, seq as u32);
        ee.set_u32(REQ + R_AIRBORNE, u32::from(airborne));
        ee.set_u32(REQ + R_FLIPPING, u32::from(flipping));
        ee.set_u32(REQ + R_NO_HANDS, u32::from(no_hands));
        ee.set_u32(REQ + R_WRECKED, u32::from(wrecked));
        ee.set_f32(REQ + R_SETTLE, settle);
        ee.set_f32(REQ + R_COMMANDED, commanded);
        ee.set_f32(REQ + R_REVERSING, reversing);
        ee.set_u32(REQ + R_GESTURE, gesture as u32);
        // The two globals. The flag is the one in front of the chooser's `srand`: set, so the five
        // copies of "seed me from the time of day" the inliner left in it are all skipped. A test
        // cannot prove a seeding off the clock and a game that has run for one frame has done it, so
        // the port has no seeding at all and `Shared` is handed a generator.
        ee.set_u32(seeded, 1);
        for i in 0..DRAWN {
            ee.set_u32(drawn + i * 4, list.get(i as usize).copied().unwrap_or(0) as u32);
        }
        ee.set_u32(drawn_count, count);
        ee.set_u32(rand_state, state0 as u32);
        ee.set_u32(rand_state + 4, (state0 >> 32) as u32);
        stepped.set(0);

        ee.call(chooser, &[u64::from(VEH)], &[dt], BUDGET);
        let after_count = ee.u32_at(drawn_count);
        let want_fields = Chose {
            want: ee.u32_at(VEH + V_WANT) as i32,
            idle_wait: ee.f32_at(VEH + V_IDLE_WAIT).to_bits(),
            idle_playing: ee.u32_at(VEH + V_IDLE_PLAYING) != 0,
            start_picked: ee.u32_at(VEH + V_START_PICKED) != 0,
            breathe: ee.f32_at(VEH + V_BREATHE).to_bits(),
            finished: ee.u32_at(VEH + V_FINISHED) != 0,
            cut: ee.u32_at(REQ + R_CUT) != 0,
            start_phase: ee.f32_at(VEH + V_START_PHASE).to_bits(),
            seq: ee.u32_at(VEH + V_SEQ).to_le_bytes()[0],
            drawn: (0..after_count).map(|i| ee.u32_at(drawn + i * 4) as i32).collect(),
            rand_state: u64::from(ee.u32_at(rand_state))
                | u64::from(ee.u32_at(rand_state + 4)) << 32,
        };
        assert_eq!(ee.u32_at(seeded), 1, "case {k}: the chooser re-seeded the generator");
        assert_eq!(stepped.get(), 1, "case {k}: the chooser called the step {} times", stepped.get());

        let mut d = Driver { want: want0, seq: seq0, ..Driver::default() };
        let mut p = Params { rate_ready: F(rate_ready), band: [F(b0), F::ZERO, F::ZERO], ..Params::default() };
        let mut u = Update {
            idle_wait: F(idle_wait0),
            idle_playing,
            start_picked,
            breathe_wait: F(breathe0),
            still_band: F(still_band),
            wait: [F(wait_lo), F(wait_hi)],
            settled,
        };
        let mut a = Asked {
            request: Request {
                cut,
                phase_axis: F(phase_axis),
                mix_axis: F(mix_axis),
                band: F(band),
                progress: F(progress),
                seq,
                ..Request::default()
            },
            wrecked,
            gesture,
            flipping,
            airborne,
            no_hands,
            reversing: F(reversing),
            commanded: F(commanded),
            settle: F(settle),
        };
        let mut player = Player { finished: finished0, ..Player::default() };
        let mut shared = Shared { rng: Rng::from_state(state0), used: list.clone() };
        u.want(F(dt), &mut d, &p, &mut a, &mut player, &mut shared);
        // Where the step goes. The two fields the original writes after it are the chooser's, not the
        // step's, which is what the second half of the port is for.
        Update::serviced(&mut d, &mut p, &a);
        let got_fields = Chose {
            want: d.want,
            idle_wait: u.idle_wait.to_bits(),
            idle_playing: u.idle_playing,
            start_picked: u.start_picked,
            breathe: u.breathe_wait.to_bits(),
            finished: player.finished,
            cut: a.request.cut,
            start_phase: p.start_phase.to_bits(),
            seq: d.seq,
            drawn: shared.used.clone(),
            rand_state: shared.rng.state(),
        };
        let why = format!(
            "case {k}: cut {cut} dt {dt:e} want0 {want0} breathe {breathe0:e} picked \
             {start_picked} finished {finished0} settle {settle:e}/{rate_ready:e} \
             wrecked {wrecked}/{settled} gesture {gesture} no_hands {no_hands} flip {flipping} \
             air {airborne} reversing {reversing:e} commanded {commanded:e} \
             band {band:e} of {b0:e} still {still_band:e} axes {phase_axis:e}/{mix_axis:e} \
             idle {idle_wait0:e}/{idle_playing} wait {wait_lo:e}..{wait_hi:e} \
             list {list:?} rand {state0:#x}"
        );
        assert_eq!(
            got_fields, want_fields,
            "{why}: (want, the countdown, playing, picked, breathe, finished, the start-line flag, \
             the start phase, the request count, the poses drawn, the generator)"
        );

        per_want[got_fields.want.clamp(0, 61) as usize] += 1;
        if cut && !got_fields.cut {
            gate += 1;
        }
        if idle_wait0 != 0.0 && got_fields.idle_wait == 0 && b0 < band {
            disarmed += 1;
        }
        if !idle_playing && got_fields.idle_playing {
            fidgets += 1;
        }
        if idle_playing && !got_fields.idle_playing && !(b0 < band) {
            fidget_ended += 1;
        }
        if got_fields.idle_wait != idle_wait0.to_bits() && got_fields.idle_wait != 0 {
            rearmed += 1;
        }
        if !start_picked && cut && !(breathe0 > 0.0) {
            if count == 0 || count == DRAWN {
                reset_draw += 1;
            } else {
                searched_draw += 1;
            }
        }
        if wrecked && settled && want0 < HOLD + 1 && !cut {
            held_draw += 1;
        }
    }

    // Every arm of the chooser, and the eight things only some of them do. The counts are what makes
    // this a proof of the whole function rather than of the three branches a ride happens to take.
    for state in [
        TURN_RIDING,
        REVERSE_L,
        REVERSE_R,
        AIR,
        BACKFLIP,
        NO_HANDER,
        HOLD,
        GET_READY,
    ] {
        assert!(per_want[state as usize] > 20, "only {} frames asked for state {state}", per_want[state as usize]);
    }
    for state in HOLD + 1..=HOLD + 5 {
        assert!(per_want[state as usize] > 2, "no frame asked for hold pose {state}");
    }
    for state in IDLE..IDLE + IDLES {
        assert!(per_want[state as usize] > 2, "no frame asked for start idle {state}");
    }
    for state in START_POS..START_POS + START_POSES {
        assert!(per_want[state as usize] > 2, "no frame asked for start pose {state}");
    }
    assert!(per_want[0] > 2, "no frame asked for a gesture slot");
    assert!(gate > 20, "only {gate} frames opened the start gate");
    assert!(disarmed > 20, "only {disarmed} frames disarmed the fidget");
    assert!(fidgets > 20, "only {fidgets} frames started a fidget");
    assert!(fidget_ended > 20, "only {fidget_ended} frames ended one");
    assert!(rearmed > 20, "only {rearmed} frames re-armed the countdown");
    assert!(reset_draw > 20, "only {reset_draw} frames drew a start pose from a list it threw away");
    assert!(searched_draw > 20, "only {searched_draw} frames drew one the line had not taken");
    assert!(held_draw > 20, "only {held_draw} frames drew a hold pose");
    println!(
        "the state chooser: 12000 frames, bit-exact ({gate} opening the gate, {fidgets} starting a \
         fidget, {rearmed} re-arming it, {reset_draw}+{searched_draw} drawing a start pose, \
         {held_draw} drawing a hold pose)"
    );
}
