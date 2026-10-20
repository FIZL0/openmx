//! Platform independence: the same fixture, the same 64 bits, on every machine the port will run on.
//!
//! The claim this file makes, precisely: for a given build of the simulation, [`Sim::hash`] and
//! everything it is made of are a function of the *state* alone — not of the architecture, the pointer
//! width, the byte order of the registers, the optimisation level or the host's `libm`. A replay
//! fixture recorded on a developer's x86-64 desktop must therefore be a valid acceptance test on an
//! arm64 laptop and in a browser, and when it fails there it is because the port is wrong, not because
//! the machine is different.
//!
//! # How it is proved, in two halves
//!
//! 1. **Every expected value in this file is a literal.** The canonical fixture's per-tick hashes, the
//!    byte encoding of every leaf kind, and the game's `libm` down to the last bit are written here as
//!    constants, not recomputed from the code under test. So two machines that both pass this file
//!    agree with *each other*: there is no path by which each could confirm its own answer.
//! 2. **This same test binary is compiled for another architecture and run there.**
//!    `tests/cross/platforms.sh` builds it for `aarch64-unknown-linux-musl` (different registers,
//!    different float unit) and for `wasm32-wasip1` (32-bit `usize`, the case where a length that
//!    leaked into the hash stops matching), and runs them under `qemu-aarch64` and node's WASI:
//!
//!    ```text
//!    crates/game-sim/tests/cross/platforms.sh
//!    aarch64-unknown-linux-musl ok
//!    wasm32-wasip1 ok
//!    ```
//!
//!    The script needs no cross-gcc and installs nothing into the system; its header says what it does
//!    need. The run is not part of `./kit check` — it builds the world twice — so the test that drives
//!    it is `#[ignore]`d and the journal entry records what it said.
//!
//! Then two guards, so that a regression is unlikely rather than merely detectable:
//!
//! * `the_simulations_sources_call_no_host_transcendental` reads the sources of the three crates the
//!   simulation is built from and rejects `sin`, `powf`, `mul_add` and the rest of the host's `libm`,
//!   which is the one platform dependency `clippy::float_arithmetic` cannot see — it lints operators,
//!   and these are method calls.
//! * `the_simulation_has_no_dependency_that_could_reach_a_host_libm` holds the crates to path
//!   dependencies only, because a soft-float crate that "works everywhere" is still someone else's
//!   rounding.
//!
//! # When one of these tests fails
//!
//! * A vector moved and the state type changed in the same commit → the vectors are stale. Regenerate
//!   them with `cargo test -p game-sim --test platforms -- --ignored --nocapture print_the_vectors` and
//!   paste the block in, exactly as A1's hundred-tick constant is maintained. **Check first that the
//!   state change explains it**: these constants moving for any other reason is the bug this file is
//!   for.
//! * A vector differs *between two architectures* → something in the simulation is reading the host.
//!   The dump of the first differing field (`hash::first_difference`) names it; the usual causes are a
//!   host transcendental, a `usize` or `isize` in a hashed field, and a `HashMap` iteration order.

use std::f32::consts;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use game_sim::hash::{self, Leaf, State, Walk};
use game_sim::replay::{self, Fixture, Sample, Value, Watch};
use game_sim::{Input, Sim, state_hash};
use ps2_float::{F, Vf, libm};

mod players;

// ---------------------------------------------------------------------------------------------------
// The canonical fixture: the one run every architecture is compared on.
//
// Deliberately synthetic. It is built from integers by the code below rather than read from a file, so
// that it is identical on every machine without a fixture file having to travel with it — and so that
// this test proves nothing about the game and cannot be mistaken for a recording of it. The acceptance
// fixtures of real play (S8) travel in the git-ignored `context/fixtures/`; what this file proves is
// that whatever they contain means the same thing on every machine.
// ---------------------------------------------------------------------------------------------------

/// The fixture's seed: not a value from the game, just a constant nobody will change by accident.
const SEED: u32 = 0x4d58_554e;
/// How many input frames the run is. Long enough for the whole of the state to be in it: the bike is
/// dropped onto a level floor, the suspension takes the landing, the wreck machine puts the rider
/// down and the reset clock runs — a few thousand multiplies and a dozen of the game's own
/// transcendentals per tick, every one of which is a place a host-float port drifts.
const FRAMES: usize = 256;

/// One input frame per tick, from an integer sequence: the inputs are part of what crosses machines, so
/// they must not come from a float or from a hash of anything platform-defined.
///
/// (`Sim::tick` ignores its input while the port is this young. The frames are built, hashed and
/// replayed anyway, so that the day the port reads them the vectors move and say so.)
fn canonical_inputs() -> Vec<Input> {
    let mut x: u32 = 0x1234_5678;
    (0..FRAMES)
        .map(|_| {
            // The C library's LCG constants, used here only as a source of varied bits.
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            Input {
                buttons: (x >> 16) as u16,
                stick: ((x >> 8) as i8, x as i8),
            }
        })
        .collect()
}

/// The hash after every one of the fixture's ticks.
fn canonical_trace() -> Vec<u64> {
    let mut sim = Sim::new(SEED);
    hash::trace(&mut sim, &canonical_inputs())
}

// ---------------------------------------------------------------------------------------------------
// generated vectors: begin
// Produced by `print_the_vectors` (the last test in this file). Do not edit by hand: regenerate, and
// read the module docs above before believing a change to them.
// ---------------------------------------------------------------------------------------------------

/// `(tick, Sim::hash at that tick)`, every sixteenth tick of the canonical run.
const HASHES: &[(u64, u64)] = &[
    (16, 0x6c4b289a7b04d12d),
    (32, 0x07363f6e5c5ce90b),
    (48, 0xd54fb172a1a2b800),
    (64, 0x8017f349f137f177),
    (80, 0x39d55130bc68ae29),
    (96, 0xfcacf44c66a9c890),
    (112, 0xae11511e70359a9a),
    (128, 0xe8ec7ba50f63854a),
    (144, 0xa9778b07a116bbe7),
    (160, 0x174ae5f196abe4db),
    (176, 0xf0fb9221e93b68d3),
    (192, 0xf7f7dcb0569ed9a1),
    (208, 0x824a62a83db9457f),
    (224, 0x4af1708ae4bcc74f),
    (240, 0x39c5984cbe1fd480),
    (256, 0x2167bc369951e8b8),
];

/// FNV-1a over all 256 of the run's hashes, little-endian: the one number that covers every tick.
const TRACE_DIGEST: u64 = 0x761231d9535ed2fa;

/// `(tick, bike.pos[1] as a bit pattern)` on the same ticks — a watched field rather than a whole-state
/// hash, which is the shape an acceptance fixture of the original has.
const POS_Y: &[(u64, u32)] = &[
    (16, 0x3f5f6408),
    (32, 0x3f73fb28),
    (48, 0x3f73fb28),
    (64, 0xbe488eb0),
    (80, 0xbe92a198),
    (96, 0xbe9297b4),
    (112, 0x3f1ae644),
    (128, 0x3f1ae5ac),
    (144, 0x3f73fb28),
    (160, 0x3f73fb28),
    (176, 0x3f73fb28),
    (192, 0x3f73fb28),
    (208, 0x3f73fb28),
    (224, 0x3f800000),
    (240, 0x3f73fb28),
    (256, 0x3ed62bb8),
];

/// `(tick, bike.vel[1] as a bit pattern)`: the lane the suspension and gravity fight over, which is
/// where a rounding difference shows up first.
const VEL_Y: &[(u64, u32)] = &[
    (16, 0xc01a72ac),
    (32, 0xbfabb755),
    (48, 0xbfabb755),
    (64, 0xc08ec0f3),
    (80, 0xc02d8715),
    (96, 0x3ef316ec),
    (112, 0xc091773a),
    (128, 0xc0917843),
    (144, 0xbfabb755),
    (160, 0xbfabb755),
    (176, 0xbfabb755),
    (192, 0xbfabb755),
    (208, 0xbfabb755),
    (224, 0x00000000),
    (240, 0xbfabb755),
    (256, 0xc0b352d5),
];

/// The head of the state at the last tick, as [`hash::Dump`] prints it: the two counters and the
/// eight fields a capture of the original names by column.
///
/// Only the head, because the state behind it is the input manager and the whole rider — some
/// hundreds of leaves, which as a pasted literal would be unreadable and unreviewable. What covers
/// the rest is [`FINAL_DUMP_LEAVES`] and [`FINAL_DUMP_DIGEST`] below: a leaf count and a fold over
/// every line, so a field that differs between two machines still fails here, and
/// `hash::first_difference` is what then names it.
const FINAL_DUMP_HEAD: &str = "\
ticks = 256\n\
rng = 1297634638\n\
bike.pos[0] = 0 (0x00000000)\n\
bike.pos[1] = 0.4183023 (0x3ed62bb8)\n\
bike.pos[2] = 0 (0x00000000)\n\
bike.pos[3] = 1 (0x3f800000)\n\
bike.vel[0] = 0 (0x00000000)\n\
bike.vel[1] = -5.6038613 (0xc0b352d5)\n\
bike.vel[2] = 0 (0x00000000)\n\
bike.vel[3] = 0 (0x00000000)";

/// How many leaves the walk of the state has, and FNV-1a over the whole of its printed form.
const FINAL_DUMP_LEAVES: usize = 895;
const FINAL_DUMP_DIGEST: u64 = 0xa19c7b80d9237c53;

/// FNV-1a over the bytes of the fixture's text form: a fixture file written on one machine must be the
/// same bytes on another, or a recording cannot be shared.
const FIXTURE_TEXT_DIGEST: u64 = 0x734d2cce1670f33c;

/// The hash of [`Every`], the state below that holds one of every leaf kind the walker can emit.
const EVERY_LEAF_HASH: u64 = 0xa228ffa8409c01f8;

/// Every path [`Every`] walks, in order, with the leaf kind and width each one encodes to.
const EVERY_LEAF_DUMP: &str = "\
a_u8: u8\n\
b_u16: u16\n\
c_u32: u32\n\
d_u64: u64\n\
e_i8: i8\n\
f_i16: i16\n\
g_i32: i32\n\
h_i64: i64\n\
i_bool: bool\n\
j_f: f32\n\
k_vf[0]: f32\n\
k_vf[1]: f32\n\
k_vf[2]: f32\n\
k_vf[3]: f32\n\
l_blob: blob(u64 len + u64 digest)\n\
m_group.inner: u32\n\
n_list.len: u64\n\
n_list[0].v: u8\n\
n_list[1].v: u8\n\
n_list[2].v: u8\n\
";

// The game's own libm, bit for bit. These are the values `ps2_float::libm` gives, which are the EE's
// chopped arithmetic through fdlibm's algorithms — *not* the host's, and that is the point: the two
// differ on most inputs, and `the_games_libm_is_not_the_hosts` below says by how much on this machine.
const SINF_OUT: &[u32] = &[
    0x00000000, 0x80000000, 0x1e3ce508, 0x3e7d5777, 0x3ef57744, 0x3f576aa5,
    0xbf576aa5, 0x3f800000, 0x3f193578, 0xb3bbbe2e, 0x33bbbe2e, 0x343bbe2e,
    0xbf0b44f8, 0x3f2df50b, 0xbf01a12e, 0x3f01a12e, 0xbf5f906a,
];
const COSF_OUT: &[u32] = &[
    0x3f800000, 0x3f800000, 0x3f800000, 0x3f780aa5, 0x3f60a940, 0x3f0a5141,
    0x3f0a5141, 0xb33bbe2e, 0xbf4d17c0, 0xbf800000, 0xbf800000, 0x3f800000,
    0xbf56cd64, 0x3f3bd0ed, 0x3f5cc0ee, 0x3f5cc0ee, 0x3ef970aa,
];
const TANF_OUT: &[u32] = &[
    0x00000000, 0x80000000, 0x1e3ce508, 0x3e82bc2d, 0x3f0bda7a, 0x3fc75923,
    0xbfc75923, 0xcbae895c, 0xbf3f3cda, 0x33bbbe2e, 0xb3bbbe2e, 0x343bbe2e,
    0x3f25faf9, 0x3f6d1c1d, 0xbf1653a6, 0x3f1653a6, 0xbfe5718a,
];
const ASINF_OUT: &[u32] = &[
    0xbfc90fda, 0xbf591a9a, 0xbf060a94, 0xbdcd2493, 0x80000000, 0x00000000,
    0x3dcd2493, 0x3f060a94, 0x3f591a9a, 0x3fc90fda,
];
const ACOSF_OUT: &[u32] = &[
    0x40490fdb, 0x401ace95, 0x40060a93, 0x3fd5e223, 0x3fc90fda, 0x3fc90fda,
    0x3fbc3d91, 0x3f860a91, 0x3f39051c, 0x00000000,
];
const ATANF_OUT: &[u32] = &[
    0x00000000, 0x80000000, 0x3dcc1f14, 0x3ed32776, 0x3eed6338, 0x3f490fda,
    0xbf490fda, 0x3f5ef386, 0x3f973ab9, 0x3fbc4de9, 0x3fc90fda, 0xbfc90fda,
];
const EXPF_OUT: &[u32] = &[
    0x3f800000, 0x3f800000, 0x3f800000, 0x3fd3094d, 0x3f1b4597, 0x402df854,
    0x3ebc5ab2, 0x431469c4, 0x3bdcc9ff, 0x47c0cde3, 0x7ef882b7, 0x00000000,
];
const LOGF_OUT: &[u32] = &[
    0x00000000, 0xbf317217, 0x3f317217, 0x40135d8d, 0xc23834f1, 0x423834f1,
    0x42b17217, 0xc2aeac4f,
];
// sqrtf(-0.0) is +0.0, not -0.0: `sqrt.s` takes |x| first, so the game's one-instruction `sqrtf`
// cannot return a sign. A3 pinned the port's old -0.0 here before F4 ported the disc's own version and
// corrected it — see the F4 journal's table, and its note that this one value rests on the model rather
// than on a measurement of the hardware.
const SQRTF_OUT: &[u32] = &[
    0x00000000, 0x00000000, 0x2edbe6ff, 0x3f3504f3, 0x3f800000, 0x3fb504f3,
    0x3fddb3d7, 0x41200000, 0x501502f9, 0x5f7fffff,
];
const ATAN2F_OUT: &[u32] = &[
    0x00000000, 0x40490fdb, 0xc0490fdb, 0x3fc90fdb, 0xbfc90fdb, 0x3eed6338,
    0xbeed6338, 0x402f261b, 0xc02f261b, 0x3fc90fda, 0x1e3ce508,
];
const POWF_OUT: &[u32] = &[
    0x44800000, 0x3fb504f3, 0x41000000, 0x3fddb3d7, 0x3b4f3e38, 0x3f800000,
    0x3f800000, 0xc1000000, 0x40800000, 0x3f92857d,
];

// ---------------------------------------------------------------------------------------------------
// generated vectors: end
// ---------------------------------------------------------------------------------------------------

/// The inputs the tables above are the answers to. Spread over the ranges a game actually asks for:
/// small angles, a full turn, the far end of `sinf`'s exact argument reduction, and the exact zeros and
/// ones where fdlibm's special cases live.
const TRIG_IN: &[f32] = &[
    0.0,
    -0.0,
    1e-20,
    0.25,
    0.5,
    1.0,
    -1.0,
    consts::FRAC_PI_2,
    2.5,
    consts::PI,
    -consts::PI,
    consts::TAU,
    10.0,
    57.295776, // one radian in degrees, the shape a game's own constant has
    100.0,
    -100.0,
    200.0, // the top of fdlibm's exact argument reduction (2^7·π/2 ≈ 201)
];
/// `asinf`/`acosf` take `[-1, 1]`; the endpoints are special cases in fdlibm and in the EE's divider.
const UNIT_IN: &[f32] = &[-1.0, -0.75, -0.5, -0.1, -0.0, 0.0, 0.1, 0.5, 0.75, 1.0];
/// `atanf` over the three branches of fdlibm's reduction (|x| < 7/16, < 1.1875, ≥ 2.4375).
const ATAN_IN: &[f32] = &[
    0.0, -0.0, 0.1, 0.4375, 0.5, 1.0, -1.0, 1.1875, 2.4375, 10.0, 1e10, -1e10,
];
const EXP_IN: &[f32] = &[
    0.0, -0.0, 1e-20, 0.5, -0.5, 1.0, -1.0, 5.0, -5.0, 11.5, 88.0, -88.0,
];
const LOG_IN: &[f32] = &[1.0, 0.5, 2.0, 10.0, 1e-20, 1e20, 3.4028235e38, 1.1754944e-38];
const SQRT_IN: &[f32] = &[0.0, -0.0, 1e-20, 0.5, 1.0, 2.0, 3.0, 100.0, 1e20, 3.4028235e38];
const ATAN2_IN: &[(f32, f32)] = &[
    (0.0, 0.0),
    (0.0, -1.0),
    (-0.0, -1.0),
    (1.0, 0.0),
    (-1.0, 0.0),
    (1.0, 2.0),
    (-1.0, 2.0),
    (0.3, -0.7),
    (-0.3, -0.7),
    (1e20, 1.0),
    (1.0, 1e20),
];
const POW_IN: &[(f32, f32)] = &[
    (2.0, 10.0),
    (2.0, 0.5),
    (0.5, -3.0),
    (3.0, 0.5),
    (10.0, -2.5),
    (1.0, 1e20),
    (0.0, 0.0),
    (-2.0, 3.0),
    (-2.0, 2.0),
    (1.5, 0.3333),
];

// ---------------------------------------------------------------------------------------------------
// The fixture
// ---------------------------------------------------------------------------------------------------

/// The fixture as a replay harness fixture: the pinned watches and nothing recomputed.
///
/// Built from the constants above rather than from a run, which is what makes [`replay::verify`] a
/// real comparison here instead of the port agreeing with itself.
fn pinned_fixture() -> Fixture {
    fixture_with(POS_Y, VEL_Y)
}

/// The same fixture from a given set of samples, so that `print_the_vectors` can build it from the
/// values it has just computed rather than from the (stale) constants it is about to replace.
fn fixture_with(pos_y: &[(u64, u32)], vel_y: &[(u64, u32)]) -> Fixture {
    let watch = |path: &str, pairs: &[(u64, u32)]| {
        Watch::new(
            path,
            pairs
                .iter()
                .map(|&(tick, bits)| Sample { tick, value: Value::Float(bits) })
                .collect(),
        )
    };
    Fixture {
        name: "platforms-canonical".to_string(),
        seed: SEED,
        start: None,
        inputs: canonical_inputs(),
        hashes: Vec::new(), // the hashes are checked by tick below, where a sparse list is allowed
        watches: vec![watch("bike.pos[1]", pos_y), watch("bike.vel[1]", vel_y)],
        world: None, // the port against its own flat floor, which is what these hashes were taken over
    }
}

#[test]
fn the_canonical_run_hashes_to_its_pinned_vectors() {
    let trace = canonical_trace();
    assert_eq!(trace.len(), FRAMES, "one hash per input frame");

    for &(tick, want) in HASHES {
        let got = trace[tick as usize - 1];
        assert_eq!(
            got, want,
            "tick {tick}: hash 0x{got:016x}, pinned 0x{want:016x} — \
             the same build on another machine must produce the pinned value"
        );
    }

    // Every tick, in one number, so a divergence between the sampled ones cannot hide.
    let digest = trace
        .iter()
        .fold(0u64, |h, &x| state_hash(&x.to_le_bytes(), h));
    assert_eq!(digest, TRACE_DIGEST, "the digest of all {FRAMES} hashes");

    // And the run must actually be a run: a stuck hash would pass every comparison above.
    let mut seen: Vec<u64> = trace.clone();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), FRAMES, "every tick's hash is distinct");
}

// ---------------------------------------------------------------------------------------------------
// Several riders, saved and put back (N1): online play compares these hashes between machines.
//
// `tests/players/mod.rs`' scripted ride — four stand-in riders over a floor with kickers, riding,
// tricking, wrecking and being put back — is what the session's hash is taken over. Rollback runs it
// with a guess, goes back and runs it again, so the pinned run goes through a save and a restore too,
// and the bytes a resync sends are pinned beside the hashes. Regenerate with `print_the_riders_vectors`
// (the last test in this file); the same rule as the block above holds: check that a state change
// explains a moved vector before believing it.
// ---------------------------------------------------------------------------------------------------

/// `(tick, players::hash)` every 32 ticks of the riders' run.
const RIDERS_HASHES: &[(u64, u64)] = &[
    (32, 0x1b650dbe7a876030),
    (64, 0x2c74aaad28751510),
    (96, 0xbfbf6191040a02e6),
    (128, 0x5c691fa0bedc0db4),
    (160, 0xb86a5e1b2a244dc1),
    (192, 0x7c17d971a6eb354c),
    (224, 0x2ce49b809b202210),
    (256, 0xf3b960bab65a52a5),
];
/// FNV-1a over every tick's hash of the run.
const RIDERS_DIGEST: u64 = 0x039d790b5811d248;
/// FNV-1a over the four riders' `snapshot::encode` bytes at the end, and how many bytes they were.
const RIDERS_WIRE_DIGEST: u64 = 0x6d16d9e92b74226f;
const RIDERS_WIRE_LEN: usize = 16930;

/// The riders' run: every tick's hash, and the riders at the end. Rolled back over, eight ticks deep,
/// every 50 ticks — rider 1's pad guessed as held, the guess ridden, the save put back and the real
/// pads ridden — the way rollback runs a session whose pad came late.
fn riders_run(rollback: bool) -> (Vec<u64>, Vec<Sim>) {
    let mut riders = players::start();
    let mut seen = players::Seen::default();
    let mut trace = Vec::with_capacity(FRAMES);
    for f in 0..FRAMES {
        if rollback && f % 50 == 25 {
            let saved: Vec<_> = riders.iter().map(game_sim::snapshot::save).collect();
            for g in f..f + 8 {
                let mut pads = players::inputs(g);
                pads[1] = players::input(1, f - 1);
                players::step(&mut riders, &pads, &mut seen);
            }
            riders.iter_mut().zip(&saved).for_each(|(r, s)| game_sim::snapshot::restore(r, s));
        }
        players::step(&mut riders, &players::inputs(f), &mut seen);
        trace.push(players::hash(&riders));
    }
    assert!(seen.airborne && seen.tricks && seen.wrecks && seen.respawns, "the riders' run missed something: {seen:?}");
    (trace, riders)
}

/// The resync bytes of the riders at the end, folded, and their length.
fn riders_wire(riders: &[Sim]) -> (u64, usize) {
    riders.iter().map(game_sim::snapshot::encode).fold((0, 0), |(h, n), b| (state_hash(&b, h), n + b.len()))
}

#[test]
fn several_riders_saved_and_restored_hash_to_their_pinned_vectors() {
    let (trace, riders) = riders_run(true);
    // the rollbacks change nothing: the same run without them is the same run
    assert_eq!(riders_run(false).0, trace, "a rolled-back run hashes differently from a straight one");
    for &(tick, want) in RIDERS_HASHES {
        let got = trace[tick as usize - 1];
        assert_eq!(got, want, "tick {tick}: the riders hash 0x{got:016x}, pinned 0x{want:016x}");
    }
    let digest = trace.iter().fold(0u64, |h, &x| state_hash(&x.to_le_bytes(), h));
    assert_eq!(digest, RIDERS_DIGEST, "the digest of all {FRAMES} of the riders' hashes");
    assert_eq!(riders_wire(&riders), (RIDERS_WIRE_DIGEST, RIDERS_WIRE_LEN), "the riders' resync bytes");
    // and the rollbacks were not where the riders were standing still
    let mut seen = trace.clone();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), FRAMES, "every tick's hash is distinct");
}

#[test]
fn the_canonical_run_matches_its_pinned_fields_through_the_replay_harness() {
    let f = pinned_fixture();
    assert_eq!(f.checks(), POS_Y.len() + VEL_Y.len());
    assert!(f.checks() >= 32, "a fixture that checks little proves little");
    if let Err(d) = replay::verify(&f) {
        panic!("the pinned fields are not what this machine computes: {d}");
    }

    // The same fixture with one pinned bit changed must fail, at that tick, naming that field — so a
    // pass above is a comparison and not an empty list.
    let mut bad = pinned_fixture();
    let (tick, bits) = POS_Y[POS_Y.len() / 2];
    bad.watches[0].samples[POS_Y.len() / 2].value = Value::Float(bits ^ 1);
    let d = replay::verify(&bad).expect_err("a one-bit change must be caught");
    assert_eq!(d.tick, tick);
    assert!(d.to_string().contains("bike.pos[1]"), "{d}");
}

#[test]
fn the_final_state_dumps_to_its_pinned_text() {
    let mut sim = Sim::new(SEED);
    for i in canonical_inputs() {
        sim.tick(i);
    }
    let text = hash::Dump::of(&sim).to_string();
    let head: String = text.lines().take(FINAL_DUMP_HEAD.lines().count()).collect::<Vec<_>>().join("\n");
    assert_eq!(head, FINAL_DUMP_HEAD);
    assert_eq!(text.lines().count(), FINAL_DUMP_LEAVES, "the walk's leaf count");
    assert_eq!(state_hash(text.as_bytes(), 0), FINAL_DUMP_DIGEST, "every leaf, folded");
}

#[test]
fn a_fixture_file_is_the_same_bytes_on_every_machine() {
    let mut f = pinned_fixture();
    // A fixture file carries hashes too; add them so the digest covers the hash lines as well.
    f.hashes = canonical_trace();
    let text = f.to_text();
    assert_eq!(
        state_hash(text.as_bytes(), 0),
        FIXTURE_TEXT_DIGEST,
        "the text form of a fixture must be byte-identical across machines, or a recording cannot be \
         shared between them"
    );
    // Round-trip, and the parsed fixture still replays: the format is integer-only on both sides.
    let back = Fixture::parse(&text).expect("the fixture we just wrote must parse");
    assert_eq!(back.to_text(), text);
    replay::verify(&back).expect("a round-tripped fixture replays");
}

// ---------------------------------------------------------------------------------------------------
// The encoding: every leaf kind, byte for byte, against an FNV-1a written out here
// ---------------------------------------------------------------------------------------------------

/// One of every leaf kind the walker can emit, including the two whose length could pick up the host's
/// pointer width (a blob and a list) and the floats whose patterns must survive untouched.
struct Every;

/// A list entry, so that [`Walk::list`] has something to walk.
struct Entry(u8);

impl State for Entry {
    fn walk(&self, w: &mut Walk<'_>) {
        w.u8("v", self.0);
    }
}

impl State for Every {
    fn walk(&self, w: &mut Walk<'_>) {
        w.u8("a_u8", 0x12);
        w.u16("b_u16", 0x1234);
        w.u32("c_u32", 0xdead_beef);
        w.u64("d_u64", 0x0123_4567_89ab_cdef);
        w.i8("e_i8", -2);
        w.i16("f_i16", -300);
        w.i32("g_i32", -70_000);
        w.i64("h_i64", -5_000_000_000);
        w.bool("i_bool", true);
        w.f("j_f", F::from_bits(0x8000_0000)); // -0.0: a state a drifting port reaches
        w.vf(
            "k_vf",
            Vf::from_bits([0x3f80_0000, 0x0000_0000, 0xc040_0000, 0x7fc0_0001]),
        );
        w.blob("l_blob", b"abc");
        w.group("m_group", |w| w.u32("inner", 7));
        w.list("n_list", &[Entry(1), Entry(2), Entry(3)]);
    }
}

/// FNV-1a 64, written out here so that the encoding is checked against something other than the code
/// that produces it. Same algorithm as [`state_hash`], independently.
fn fnv1a(bytes: &[u8], seed: u64) -> u64 {
    let mut h = if seed == 0 { 0xcbf2_9ce4_8422_2325 } else { seed };
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[test]
fn every_leaf_kind_encodes_to_the_little_endian_bytes_it_claims() {
    // The byte stream the walk of `Every` must produce, built here by hand. Widths are explicit; there
    // is no `to_ne_bytes` and no `usize` anywhere in it, which is the whole of the argument for why
    // the hash cannot depend on the machine.
    let mut want: Vec<u8> = Vec::new();
    want.extend_from_slice(&0x12u8.to_le_bytes()); // a_u8   1 byte
    want.extend_from_slice(&0x1234u16.to_le_bytes()); // b_u16  2
    want.extend_from_slice(&0xdead_beefu32.to_le_bytes()); // c_u32  4
    want.extend_from_slice(&0x0123_4567_89ab_cdefu64.to_le_bytes()); // d_u64  8
    want.extend_from_slice(&(-2i8).to_le_bytes()); // e_i8   1, two's complement
    want.extend_from_slice(&(-300i16).to_le_bytes()); // f_i16  2
    want.extend_from_slice(&(-70_000i32).to_le_bytes()); // g_i32  4
    want.extend_from_slice(&(-5_000_000_000i64).to_le_bytes()); // h_i64  8
    want.push(1); // i_bool 1
    want.extend_from_slice(&0x8000_0000u32.to_le_bytes()); // j_f    4, the pattern, not the value
    // Four lanes, x y z w. Note the last one: `Vf::from_bits` puts every lane through VU0's operand
    // rule, which has no NaN and clamps to MAX — so the NaN pattern loaded in `Every` is hashed as
    // 0x7f7fffff. That conversion is part of the arithmetic model, integer-only, and identical
    // everywhere; hashing the pattern that the VU would actually hold is the correct state.
    for lane in [0x3f80_0000u32, 0x0000_0000, 0xc040_0000, 0x7f7f_ffff] {
        want.extend_from_slice(&lane.to_le_bytes());
    }
    want.extend_from_slice(&3u64.to_le_bytes()); // l_blob length, 8 bytes — not a usize
    want.extend_from_slice(&fnv1a(b"abc", 0).to_le_bytes()); // l_blob digest, 8
    want.extend_from_slice(&7u32.to_le_bytes()); // m_group.inner
    want.extend_from_slice(&3u64.to_le_bytes()); // n_list length, 8 bytes — not a usize
    want.extend_from_slice(&[1, 2, 3]); // n_list entries

    assert_eq!(
        hash::of(&Every),
        fnv1a(&want, 0),
        "the walk's byte stream is not the one written out in this test"
    );
    assert_eq!(hash::of(&Every), EVERY_LEAF_HASH, "the pinned value");

    // The same thing said a second way: the kinds and widths every leaf encodes to, as text, so that a
    // width that changed is read as a width and not as an unexplained hash.
    assert_eq!(leaf_kinds(&hash::Dump::of(&Every)), EVERY_LEAF_DUMP);
}

/// `path: kind width` for every leaf of a dump — the shape of the encoding rather than its values.
fn leaf_kinds(d: &hash::Dump) -> String {
    let mut out = String::new();
    for f in &d.fields {
        let kind = match f.leaf {
            Leaf::Uint { width, .. } => format!("u{}", width * 8),
            Leaf::Int { width, .. } => format!("i{}", width * 8),
            Leaf::Float(_) => "f32".to_string(),
            Leaf::Bool(_) => "bool".to_string(),
            Leaf::Blob { .. } => "blob(u64 len + u64 digest)".to_string(),
        };
        let _ = writeln!(out, "{}: {kind}", f.path);
    }
    out
}

#[test]
fn a_length_in_the_hash_is_eight_bytes_wide_whatever_a_usize_is() {
    // The only two places a `usize` is anywhere near the hash. On wasm32 a `usize` is 4 bytes, so if
    // either of these ever encoded one directly, the cross run would stop matching the pinned vectors —
    // and this test says which of the two it was without having to read the stream.
    let d = hash::Dump::of(&Every);
    assert_eq!(
        d.get("n_list.len"),
        Some(Leaf::Uint { value: 3, width: 8 }),
        "a list's length is hashed as a fixed 8 bytes"
    );
    assert!(
        matches!(d.get("l_blob"), Some(Leaf::Blob { len: 3, .. })),
        "a blob keeps its length, which `Leaf::hash_into` encodes as 8 bytes"
    );
    assert_eq!(
        Leaf::Blob { len: 3, digest: 0 }.hash_into(0),
        fnv1a(&[&3u64.to_le_bytes()[..], &0u64.to_le_bytes()[..]].concat(), 0),
        "a blob leaf is 8 bytes of length then 8 bytes of digest"
    );

    // And the state the whole project compares — A1's hundred ticks — is the same number here as it is
    // in `lib.rs`, on whichever architecture is running this.
    let mut sim = Sim::new(1);
    for _ in 0..100 {
        sim.tick(Input::default());
    }
    assert_eq!(sim.hash(), 0x4667_aeed_f29f_5957);
}

// ---------------------------------------------------------------------------------------------------
// The game's libm, and the host's
// ---------------------------------------------------------------------------------------------------

/// Check a one-argument function against its pinned outputs.
fn pinned1(name: &str, f: impl Fn(f32) -> f32, ins: &[f32], outs: &[u32]) {
    assert_eq!(ins.len(), outs.len(), "{name}: one pinned output per input");
    for (&x, &want) in ins.iter().zip(outs) {
        let got = f(x).to_bits();
        assert_eq!(
            got, want,
            "{name}({x:?}) = 0x{got:08x}, pinned 0x{want:08x} — this must not depend on the machine"
        );
    }
}

/// Check a two-argument function against its pinned outputs.
fn pinned2(name: &str, f: impl Fn(f32, f32) -> f32, ins: &[(f32, f32)], outs: &[u32]) {
    assert_eq!(ins.len(), outs.len(), "{name}: one pinned output per input");
    for (&(a, b), &want) in ins.iter().zip(outs) {
        let got = f(a, b).to_bits();
        assert_eq!(
            got, want,
            "{name}({a:?}, {b:?}) = 0x{got:08x}, pinned 0x{want:08x}"
        );
    }
}

#[test]
fn the_games_libm_is_pinned_bit_for_bit() {
    pinned1("sinf", libm::sinf, TRIG_IN, SINF_OUT);
    pinned1("cosf", libm::cosf, TRIG_IN, COSF_OUT);
    pinned1("tanf", libm::tanf, TRIG_IN, TANF_OUT);
    pinned1("asinf", libm::asinf, UNIT_IN, ASINF_OUT);
    pinned1("acosf", libm::acosf, UNIT_IN, ACOSF_OUT);
    pinned1("atanf", libm::atanf, ATAN_IN, ATANF_OUT);
    pinned1("expf", libm::expf, EXP_IN, EXPF_OUT);
    pinned1("logf", libm::logf, LOG_IN, LOGF_OUT);
    pinned1("sqrtf", libm::sqrtf, SQRT_IN, SQRTF_OUT);
    pinned2("atan2f", libm::atan2f, ATAN2_IN, ATAN2F_OUT);
    pinned2("powf", libm::powf, POW_IN, POWF_OUT);
}

#[test]
fn the_games_libm_is_not_the_hosts() {
    // The one deliberate use of the host's transcendentals in the project: here, to measure the
    // distance between them and the game's. A port that called these would pass its own tests on the
    // machine it was written on and fail on the next one — by this many inputs out of this many.
    let host: Vec<(&str, usize, usize)> = vec![
        differs("sinf", libm::sinf, f32::sin, TRIG_IN),
        differs("cosf", libm::cosf, f32::cos, TRIG_IN),
        differs("tanf", libm::tanf, f32::tan, TRIG_IN),
        differs("asinf", libm::asinf, f32::asin, UNIT_IN),
        differs("acosf", libm::acosf, f32::acos, UNIT_IN),
        differs("atanf", libm::atanf, f32::atan, ATAN_IN),
        differs("expf", libm::expf, f32::exp, EXP_IN),
        differs("logf", libm::logf, f32::ln, LOG_IN),
    ];
    let mut total = 0;
    let mut of = 0;
    for (name, d, n) in &host {
        println!("{name}: {d} of {n} inputs differ from this host's libm");
        total += d;
        of += n;
    }
    println!("{total} of {of} differ in all");
    assert!(
        total > 0,
        "the host's libm agrees with the game's on every input tried, which is not what any libm \
         this port has met does. Either the tables above were generated from the host (regenerate \
         them and read the module docs) or this platform's libm happens to be the same fdlibm — in \
         which case the pinned tables are still the proof and this count is only a note."
    );
}

/// How many of `ins` the two implementations disagree on, bit for bit.
fn differs(
    name: &'static str,
    ours: impl Fn(f32) -> f32,
    theirs: impl Fn(f32) -> f32,
    ins: &[f32],
) -> (&'static str, usize, usize) {
    let n = ins
        .iter()
        .filter(|&&x| ours(x).to_bits() != theirs(x).to_bits())
        .count();
    (name, n, ins.len())
}

// ---------------------------------------------------------------------------------------------------
// The guards: what the sources are not allowed to contain
// ---------------------------------------------------------------------------------------------------

/// The crates the shipped simulation is built from. `ps2-oracle` is in the list because its
/// interpreter decides what "the original did" in every oracle proof: if *it* used the host's libm, the
/// proofs would be as platform-dependent as the port.
const SIM_CRATES: [&str; 3] = ["game-sim", "ps2-float", "ps2-oracle"];

/// The host's transcendentals, as they appear in Rust source: a method call on a float, or a path.
///
/// `sqrt`, `abs`, `max`, `min`, `clamp` and `lerp` are deliberately *not* here: they are also the names
/// of [`F`]'s own methods, which are the correct thing to call, and IEEE specifies `sqrt` exactly on
/// every platform anyway. `mul_add` *is* here: it is Rust's fused multiply-add, which the EE does not
/// have — `madd.s` chops the product first (see `ps2_float`'s docs), so `mul_add` is both a host
/// dependency and the wrong arithmetic.
const HOST_LIBM: [&str; 25] = [
    "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "exp", "exp2", "exp_m1", "ln", "ln_1p",
    "log", "log2", "log10", "powf", "powi", "cbrt", "hypot", "mul_add", "to_degrees", "to_radians",
    "sinh", "cosh", "tanh",
];

#[test]
fn the_simulations_sources_call_no_host_transcendental() {
    let Some(root) = repo_root() else {
        println!("skipped: the crates' sources are not readable from here");
        return;
    };
    let mut scanned = 0;
    let mut bad: Vec<String> = Vec::new();
    for crate_name in SIM_CRATES {
        for file in rs_files(&root.join("crates").join(crate_name).join("src")) {
            let text = strip_comments_and_strings(&std::fs::read_to_string(&file).unwrap());
            scanned += 1;
            for (n, line) in text.lines().enumerate() {
                for name in HOST_LIBM {
                    // `.sin(` is a method call on whatever the receiver is; `f32::sin(`/`f64::sin(` is
                    // the same function by path. Both are the host's.
                    for form in [
                        format!(".{name}("),
                        format!("f32::{name}("),
                        format!("f64::{name}("),
                    ] {
                        if line.contains(&form) {
                            let rel = file.strip_prefix(&root).unwrap_or(&file).display();
                            bad.push(format!("{rel}:{}: {}", n + 1, line.trim()));
                        }
                    }
                }
            }
        }
    }
    assert!(scanned >= 8, "only {scanned} source files found to scan");
    assert!(
        bad.is_empty(),
        "the host's libm is reachable from the simulation ({} place(s)). Use `ps2_float::libm`, \
         which is the algorithm the game's own libm used, and whose results are the same on every \
         machine:\n{}",
        bad.len(),
        bad.join("\n")
    );
    println!("{scanned} source files scanned, no host transcendental");
}

#[test]
fn the_simulation_has_no_dependency_that_could_reach_a_host_libm() {
    let Some(root) = repo_root() else {
        println!("skipped: the crates' manifests are not readable from here");
        return;
    };
    for crate_name in SIM_CRATES {
        let manifest = root.join("crates").join(crate_name).join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest).unwrap();
        // Every dependency line in these crates is a path dependency on a sibling. A registry
        // dependency is not forbidden because it is dangerous in itself — it is forbidden because the
        // arithmetic of this port may not be a crate version's business, and because an added
        // dependency is where a host `libm` shim arrives.
        let mut in_deps = false;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_deps = line.contains("dependencies");
            } else if in_deps && line.contains('=') && !line.starts_with('#') {
                assert!(
                    line.contains("path ="),
                    "{crate_name} depends on something that is not a sibling crate: {line}"
                );
            }
        }
    }
}

/// The repository root, or `None` when the sources are not reachable (a cross run inside a sandbox
/// that has no filesystem). The tests that scan sources say they skipped rather than pass silently.
fn repo_root() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent()?.parent()?;
    root.join("crates/game-sim/src/lib.rs").is_file().then(|| root.to_path_buf())
}

/// Every `.rs` file under `dir`, recursively, in a fixed order.
fn rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Source with comments and string literals blanked out, so that a doc comment explaining why not to
/// call `f32::sin` is not read as a call to it. Crude on purpose — it is a lint over our own files, and
/// a false positive names the line it found.
fn strip_comments_and_strings(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let b: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < b.len() {
        let (c, next) = (b[i], b.get(i + 1).copied().unwrap_or('\0'));
        match c {
            '/' if next == '/' => {
                while i < b.len() && b[i] != '\n' {
                    i += 1;
                }
            }
            '/' if next == '*' => {
                let mut depth = 1;
                i += 2;
                while i < b.len() && depth > 0 {
                    if b[i] == '/' && b.get(i + 1) == Some(&'*') {
                        depth += 1;
                        i += 2;
                    } else if b[i] == '*' && b.get(i + 1) == Some(&'/') {
                        depth -= 1;
                        i += 2;
                    } else {
                        if b[i] == '\n' {
                            out.push('\n');
                        }
                        i += 1;
                    }
                }
            }
            '"' => {
                i += 1;
                while i < b.len() && b[i] != '"' {
                    if b[i] == '\\' {
                        i += 1;
                    }
                    if i < b.len() && b[i] == '\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
                i += 1;
            }
            '\'' => {
                // A char literal is dropped; a lifetime (`'a`) is not a literal and what follows it
                // matters. The three cases are told apart by what comes after the quote.
                if next == '\\' {
                    i += 2; // an escape: '\n', '\\', '\u{3b1}' — up to the closing quote
                    while i < b.len() && b[i] != '\'' {
                        i += 1;
                    }
                    i += 1;
                } else if b.get(i + 2) == Some(&'\'') {
                    i += 3; // a one-character literal
                } else {
                    out.push(c); // a lifetime
                    i += 1;
                }
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

#[test]
fn the_comment_stripper_does_not_hide_code_or_read_prose_as_code() {
    let src = r#"
//! Never call f32::sin() from a port.
let a = x.powf(2.0); // this one is real
/* f32::exp() in a block comment */
let s = "a string mentioning .sin( and an escaped \" quote";
let c = '"';
fn f<'a>(x: &'a str) -> &'a str { x }
"#;
    let out = strip_comments_and_strings(src);
    assert!(out.contains(".powf(2.0)"), "code survives: {out}");
    assert!(!out.contains("f32::sin"), "doc comments are gone: {out}");
    assert!(!out.contains("f32::exp"), "block comments are gone: {out}");
    assert!(!out.contains("a string mentioning"), "strings are gone: {out}");
    assert!(out.contains("fn f<'a>(x: &'a str)"), "lifetimes survive: {out}");
    // Line numbers must still point at the right line, so a report can be followed.
    assert_eq!(out.lines().count(), src.lines().count(), "{out}");
}

// ---------------------------------------------------------------------------------------------------
// The measurement: this same test binary, on another architecture
// ---------------------------------------------------------------------------------------------------

/// Build this test binary for the other architectures and run it there.
///
/// `#[ignore]`d because it compiles the workspace once per target and may fetch a toolchain the first
/// time; it is the deliberate act that proves the claim, and the journal entry records its output.
/// Everything above is what it runs *over there*, unchanged.
#[test]
#[ignore = "builds the workspace for another architecture; run it by hand or in CI"]
#[cfg(unix)]
fn the_pinned_vectors_hold_on_another_architecture() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cross/platforms.sh");
    let out = std::process::Command::new("sh")
        .arg(&script)
        .output()
        .expect("tests/cross/platforms.sh must be runnable");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    println!("{text}");
    assert!(out.status.success(), "{}: {text}", script.display());
    let ok = text.lines().filter(|l| l.ends_with(" ok")).count();
    assert!(ok > 0, "no target reported ok:\n{text}");
}

/// Print the generated block of this file, for pasting back into it when the state type changes.
///
/// `cargo test -p game-sim --test platforms -- --ignored --nocapture print_the_vectors`
#[test]
#[ignore = "a generator, not a test"]
fn print_the_vectors() {
    let trace = canonical_trace();
    let mut sim = Sim::new(SEED);
    for i in canonical_inputs() {
        sim.tick(i);
    }
    let dump = hash::Dump::of(&sim);
    let sampled: Vec<u64> = (1..=FRAMES as u64).filter(|t| t % 16 == 0).collect();

    let mut o = String::new();
    let _ = writeln!(o, "const HASHES: &[(u64, u64)] = &[");
    for &t in &sampled {
        let _ = writeln!(o, "    ({t}, 0x{:016x}),", trace[t as usize - 1]);
    }
    let _ = writeln!(o, "];");
    let digest = trace
        .iter()
        .fold(0u64, |h, &x| state_hash(&x.to_le_bytes(), h));
    let _ = writeln!(o, "const TRACE_DIGEST: u64 = 0x{digest:016x};");

    let mut watched: Vec<Vec<(u64, u32)>> = Vec::new();
    for (name, path) in [("POS_Y", "bike.pos[1]"), ("VEL_Y", "bike.vel[1]")] {
        let mut samples = Vec::new();
        let _ = writeln!(o, "const {name}: &[(u64, u32)] = &[");
        for &t in &sampled {
            let s = replay::state_at(&pinned_fixture(), t);
            let Some(Leaf::Float(bits)) = hash::Dump::of(&s).get(path) else {
                panic!("{path} is not a float field of the state any more");
            };
            let _ = writeln!(o, "    ({t}, 0x{bits:08x}),");
            samples.push((t, bits));
        }
        let _ = writeln!(o, "];");
        watched.push(samples);
    }

    let text = dump.to_string();
    let head: String = text.lines().take(10).collect::<Vec<_>>().join("\n");
    let _ = writeln!(o, "const FINAL_DUMP_HEAD: &str = \"\\\n{}\";", escape(&head));
    let _ = writeln!(o, "const FINAL_DUMP_LEAVES: usize = {};", text.lines().count());
    let _ = writeln!(
        o,
        "const FINAL_DUMP_DIGEST: u64 = 0x{:016x};",
        state_hash(text.as_bytes(), 0)
    );
    let mut f = fixture_with(&watched[0], &watched[1]);
    f.hashes = trace;
    let _ = writeln!(
        o,
        "const FIXTURE_TEXT_DIGEST: u64 = 0x{:016x};",
        state_hash(f.to_text().as_bytes(), 0)
    );
    let _ = writeln!(o, "const EVERY_LEAF_HASH: u64 = 0x{:016x};", hash::of(&Every));
    let _ = writeln!(
        o,
        "const EVERY_LEAF_DUMP: &str = \"\\\n{}\";",
        escape(&leaf_kinds(&hash::Dump::of(&Every)))
    );

    for (name, out) in [
        ("SINF", TRIG_IN.iter().map(|&x| libm::sinf(x)).collect::<Vec<_>>()),
        ("COSF", TRIG_IN.iter().map(|&x| libm::cosf(x)).collect()),
        ("TANF", TRIG_IN.iter().map(|&x| libm::tanf(x)).collect()),
        ("ASINF", UNIT_IN.iter().map(|&x| libm::asinf(x)).collect()),
        ("ACOSF", UNIT_IN.iter().map(|&x| libm::acosf(x)).collect()),
        ("ATANF", ATAN_IN.iter().map(|&x| libm::atanf(x)).collect()),
        ("EXPF", EXP_IN.iter().map(|&x| libm::expf(x)).collect()),
        ("LOGF", LOG_IN.iter().map(|&x| libm::logf(x)).collect()),
        ("SQRTF", SQRT_IN.iter().map(|&x| libm::sqrtf(x)).collect()),
        ("ATAN2F", ATAN2_IN.iter().map(|&(a, b)| libm::atan2f(a, b)).collect()),
        ("POWF", POW_IN.iter().map(|&(a, b)| libm::powf(a, b)).collect()),
    ] {
        let _ = write!(o, "const {name}_OUT: &[u32] = &[");
        for (i, v) in out.iter().enumerate() {
            let _ = write!(o, "{}0x{:08x},", if i % 6 == 0 { "\n    " } else { " " }, v.to_bits());
        }
        let _ = writeln!(o, "\n];");
    }
    println!("{o}");
}

/// A multi-line string as a Rust literal with `\n` written out, so a pasted dump stays one constant.
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n\\\n")
}

/// Print the riders' block, for pasting over it when the state type changes.
///
/// `cargo test -p game-sim --test platforms -- --ignored --nocapture print_the_riders_vectors`
#[test]
#[ignore = "a generator, not a test"]
fn print_the_riders_vectors() {
    let (trace, riders) = riders_run(true);
    let mut o = String::new();
    let _ = writeln!(o, "const RIDERS_HASHES: &[(u64, u64)] = &[");
    for t in (32..=FRAMES).step_by(32) {
        let _ = writeln!(o, "    ({t}, 0x{:016x}),", trace[t - 1]);
    }
    let _ = writeln!(o, "];");
    let digest = trace.iter().fold(0u64, |h, &x| state_hash(&x.to_le_bytes(), h));
    let _ = writeln!(o, "const RIDERS_DIGEST: u64 = 0x{digest:016x};");
    let (wire, len) = riders_wire(&riders);
    let _ = writeln!(o, "const RIDERS_WIRE_DIGEST: u64 = 0x{wire:016x};");
    let _ = writeln!(o, "const RIDERS_WIRE_LEN: usize = {len};");
    println!("{o}");
}
