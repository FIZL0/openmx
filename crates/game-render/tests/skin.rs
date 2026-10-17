//! V5 — what a motion clip looks like, proved against the original.
//!
//! The port is `game_render::skin`. This file proves it three ways, because three different kinds of
//! claim are being made and no one method covers them all:
//!
//! - **Against the original's own code**, through the function oracle: the quaternion decoder, the
//!   slerp, both channel samplers and the quaternion-to-matrix conversion, each over ten thousand
//!   generated inputs. These are the arithmetic, and the arithmetic is where a wrong grouping hides.
//!   The comparison is to a small tolerance rather than bit-exact on purpose: this crate draws, so it
//!   uses host `f32` while the original uses the EE's FPU and VU0, and the two round differently. The
//!   tolerances here are tight enough that a wrong *formula* cannot pass — a swapped matrix row, a
//!   `1 - t` where the game writes `theta - t * theta`, a missing sign fold — and loose enough that
//!   rounding does not fail a run. Every proof prints the worst difference it saw, so a regression
//!   that eats the margin is visible before it fails.
//! - **Against the game's own tables in a save state**, for the two runtime tables the port rebuilds
//!   from `sin`, `cos` and `acos` rather than porting a builder that is not in the image's call graph.
//! - **Against the disc**, for the format: every one of the seventeen thousand `.mot` members in every
//!   archive is parsed, and the invariants the port leans on (version 3, one key kind each, turn
//!   indices inside the table, times that do not go backwards) are checked on all of them rather than
//!   assumed from a survey. Then one real rider — `mrider.psg`'s node tree as a skeleton and its own
//!   clips on top of it — is posed and skinned, which is the claim the task's title makes.
//!
//! The port's plumbing either side of the arithmetic (the joint map, the cross-fade, the two-clip mix,
//! the parent composition) is proved port-only: the original does those inside a motion object whose
//! vtables, blend buffers and joint records are not worth fabricating in RAM when the sampling they
//! wrap is already proved against the original one call at a time.
//!
//! Everything that is *not* proved here is listed in the journal entry under "Not verified / not 1:1".

use std::collections::BTreeSet;

use game_render::disc::Disc;
use game_render::mat::M4;
use game_render::psg::{Model, Node};
use game_render::skin::{
    self, Clip, NarrowKey, PosChannel, PosKey, Pose, RotChannel, RotKey, RotKeys, SKIP, Skeleton,
    tables,
};
use ps2_oracle::{Ee, Kit, addr, game_or_skip, skip};

// --- where the harness puts things in RAM -----------------------------------------------------------
// Past anything the executable uses, and well clear of the stack a call starts with.
/// The arc-cosine table the port rebuilt: 30002 floats, pointed at by `motion_arc_table`.
const ARC: u32 = 0x0120_0000;
/// One eight-byte rotation key, for the decoder.
const KEY: u32 = 0x0110_0000;
/// Two quaternions and an output, each its own quadword.
const FROM: u32 = 0x0110_0100;
const TO: u32 = 0x0110_0110;
const OUT: u32 = 0x0110_0120;
/// A channel record and its key array.
const CHAN: u32 = 0x0110_0200;
const KEYS: u32 = 0x0111_0000;
/// A transform, whose rotation rows land at +0x40.
const XFORM: u32 = 0x0112_0000;

/// How many samples each oracle proof generates.
const SAMPLES: u32 = 10_000;
/// How many channels the two sampler proofs generate, and how many times each is asked for.
const CHANNELS: u32 = 400;
const TIMES: u32 = 25;

/// Instructions one call may take. A sampler is a few hundred; a budget three orders of magnitude
/// above that only ever fires on a function waiting for hardware.
const BUDGET: u64 = 200_000;

// --- the generator ----------------------------------------------------------------------------------

/// A plain 64-bit LCG, so a seed names a sample and nothing generated depends on the host.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 32) as u32
    }
    /// A number in `0..n`.
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    /// A multiple of 1/64 in ±`range`, which is exact in binary and so identical on every host.
    fn coord(&mut self, range: i32) -> f32 {
        let steps = range * 64;
        (self.below(2 * steps as u32) as i32 - steps) as f32 * 0.015_625
    }
    /// A wide rotation key as the disc holds one: a turn index inside the table and a signed axis.
    fn key(&mut self, t: f32) -> RotKey {
        // Axis components as the exporter writes them, and now and then a degenerate axis, because
        // the decoder has no special case for one and the slerp has to cope with what comes out.
        let axis = if self.below(16) == 0 {
            [0, 0, 0]
        } else {
            [0, 1, 2].map(|_| self.below(65_536) as u16 as i16)
        };
        RotKey { t, turn: self.below(4096) as u16, axis }
    }
    /// A narrow rotation key: a byte of angle and three of axis.
    fn narrow(&mut self, t: f32) -> NarrowKey {
        let axis = if self.below(16) == 0 {
            [0, 0, 0]
        } else {
            [0, 1, 2].map(|_| self.below(256) as u8 as i8)
        };
        NarrowKey { t, turn: self.below(256) as u8, axis }
    }
    /// How far the next key is from this one. Never zero: two keys at the same time make a
    /// zero-length interval, and the original divides by it — as does the port, to the same infinity
    /// but not the same bits. No channel on the disc has a repeated time (see
    /// [`every_clip_on_the_disc_parses`]), so neither does a generated one.
    fn step(&mut self) -> f32 {
        [0.031_25f32, 0.062_5, 0.125][self.below(3) as usize]
    }
}

// --- comparing -------------------------------------------------------------------------------------

/// The worst absolute difference seen so far, and where.
#[derive(Default)]
struct Worst {
    by: f32,
    at: String,
}

impl Worst {
    /// Compare one number, remember the worst, and say whether this one is outside `tol`.
    fn check(&mut self, name: &str, theirs: f32, ours: f32, tol: f32) -> Option<String> {
        let by = (theirs - ours).abs();
        if by > self.by {
            self.by = by;
            self.at = format!("{name}: theirs {theirs:e}, ours {ours:e}");
        }
        (!(by <= tol)).then(|| format!("{name}: theirs {theirs:e}, ours {ours:e}, off by {by:e}"))
    }
    /// Compare four lanes at an address against four of ours.
    fn quad(&mut self, ee: &Ee, at: u32, ours: [f32; 4], tol: f32, into: &mut Vec<String>) {
        for (k, lane) in ["x", "y", "z", "w"].iter().enumerate() {
            let theirs = ee.f32_at(at + 4 * k as u32);
            if let Some(bad) = self.check(lane, theirs, ours[k], tol) {
                into.push(bad);
            }
        }
    }
}

// --- the game's two tables, in the game's own memory -------------------------------------------------

/// Write the port's rebuilt tables where the original's quaternion code reads them.
///
/// Without this every oracle proof here would be comparing the port's tables against a page of zeroes.
/// With it, the tables are held constant and what is being proved is the arithmetic around them —
/// which is the right split, because the tables are proved separately against a save state by
/// [`the_tables_are_the_games_own`].
fn install_tables(ee: &mut Ee) {
    let t = tables();
    let mut turn = Vec::with_capacity(t.turn.len() * 8);
    for [sin, cos] in &t.turn {
        turn.extend_from_slice(&sin.to_bits().to_le_bytes());
        turn.extend_from_slice(&cos.to_bits().to_le_bytes());
    }
    ee.load(addr("motion_turn_table").unwrap(), &turn);

    let mut narrow = Vec::with_capacity(t.narrow.len() * 8);
    for [sin, cos] in &t.narrow {
        narrow.extend_from_slice(&sin.to_bits().to_le_bytes());
        narrow.extend_from_slice(&cos.to_bits().to_le_bytes());
    }
    ee.load(addr("motion_turn_table_narrow").unwrap(), &narrow);

    let mut arc = Vec::with_capacity(t.arc.len() * 4);
    for v in &t.arc {
        arc.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    ee.load(ARC, &arc);
    ee.set_u32(addr("motion_arc_table").unwrap(), ARC);
    ee.set_f32(addr("motion_arc_scale").unwrap(), 14_999.5);
}

/// The eight bytes a rotation key's rotation is: the turn index, then the three axis components.
fn rot_payload(key: &RotKey) -> [u8; 8] {
    let mut b = [0u8; 8];
    b[0..2].copy_from_slice(&key.turn.to_le_bytes());
    for k in 0..3 {
        b[2 + 2 * k..4 + 2 * k].copy_from_slice(&key.axis[k].to_le_bytes());
    }
    b
}

/// One whole twelve-byte rotation key: its time, then its rotation.
fn rot_key_bytes(key: &RotKey) -> [u8; 12] {
    let mut b = [0u8; 12];
    b[0..4].copy_from_slice(&key.t.to_bits().to_le_bytes());
    b[4..12].copy_from_slice(&rot_payload(key));
    b
}

/// The four bytes a narrow rotation key's rotation is.
fn narrow_payload(key: &NarrowKey) -> [u8; 4] {
    [key.turn, key.axis[0] as u8, key.axis[1] as u8, key.axis[2] as u8]
}

/// One whole eight-byte narrow key: its time, then its rotation.
fn narrow_key_bytes(key: &NarrowKey) -> [u8; 8] {
    let mut b = [0u8; 8];
    b[0..4].copy_from_slice(&key.t.to_bits().to_le_bytes());
    b[4..8].copy_from_slice(&narrow_payload(key));
    b
}

/// One sixteen-byte position key: its time, then its translation.
fn pos_key_bytes(key: &PosKey) -> [u8; 16] {
    let mut b = [0u8; 16];
    b[0..4].copy_from_slice(&key.t.to_bits().to_le_bytes());
    for k in 0..3 {
        b[4 + 4 * k..8 + 4 * k].copy_from_slice(&key.p[k].to_bits().to_le_bytes());
    }
    b
}

/// A channel record as the clip reader leaves one: the name index, the on flag, the key count, the key
/// array, and (rotation only) the cached pair starting at 1.
fn install_channel(ee: &mut Ee, keys: &[u8], count: usize, cursor: u32) {
    ee.set_u32(CHAN, 0); // the name index, which the sampler does not read
    ee.set_u32(CHAN + 4, 1); // on
    ee.set_u32(CHAN + 8, count as u32);
    ee.set_u32(CHAN + 0x14, KEYS);
    ee.set_u32(CHAN + 0x18, cursor);
    ee.load(KEYS, keys);
}

// --- the arithmetic, against the original ------------------------------------------------------------

/// The compressed rotation key, decoded: axis times the sine, the cosine's sign folded in, `w = |cos|`.
#[test]
fn the_quaternion_decode_matches_the_original() {
    let Some(mut ee) = game_or_skip() else { return };
    install_tables(&mut ee);
    let decode = addr("motion_quat_decode").unwrap();
    let mut rng = Rng(0x5eed_0001);
    let mut worst = Worst::default();
    let mut signed = 0u32;
    for n in 1..=SAMPLES {
        let key = rng.key(0.0);
        ee.load(KEY, &rot_payload(&key));
        ee.call(decode, &[KEY as u64, OUT as u64], &[], BUDGET);

        let ours = skin::decode(&key);
        let mut bad = Vec::new();
        // The lanes are an axis of up to 32767 times a sine, so they are not small; the tolerance is
        // relative to the magnitude the key can reach.
        worst.quad(&ee, OUT, ours, 1e-3, &mut bad);
        assert!(bad.is_empty(), "key {n} (turn {}, axis {:?}):\n  {}", key.turn, key.axis, bad.join("\n  "));
        signed += (key.turn >= 2048) as u32;
        assert!(ours[3] >= 0.0, "key {n}: w came out negative ({}), so the sign fold is wrong", ours[3]);
    }
    println!("{SAMPLES} keys decoded, {signed} of them past a half turn (a negative cosine, the sign \
              folded into the axis); worst lane off by {:e} ({})", worst.by, worst.at);
    assert!(signed > SAMPLES / 4, "only {signed} keys had a negative cosine: the generator missed the fold");
}

/// The same for the narrow key: a byte of angle, three of axis, and the 256-entry table.
#[test]
fn the_narrow_quaternion_decode_matches_the_original() {
    let Some(mut ee) = game_or_skip() else { return };
    install_tables(&mut ee);
    let decode = addr("motion_quat_decode_narrow").unwrap();
    let mut rng = Rng(0x5eed_0006);
    let mut worst = Worst::default();
    let mut signed = 0u32;
    for n in 1..=SAMPLES {
        let key = rng.narrow(0.0);
        ee.load(KEY, &narrow_payload(&key));
        ee.call(decode, &[KEY as u64, OUT as u64], &[], BUDGET);

        let ours = skin::decode_narrow(&key);
        let mut bad = Vec::new();
        worst.quad(&ee, OUT, ours, 1e-5, &mut bad);
        assert!(bad.is_empty(), "key {n} (turn {}, axis {:?}):\n  {}", key.turn, key.axis, bad.join("\n  "));
        signed += (key.turn >= 128) as u32;
        assert!(ours[3] >= 0.0, "key {n}: w came out negative ({}), so the sign fold is wrong", ours[3]);
    }
    println!("{SAMPLES} narrow keys decoded, {signed} of them past a half turn; worst lane off by \
              {:e} ({})", worst.by, worst.at);
    assert!(signed > SAMPLES / 4, "only {signed} keys had a negative cosine: the generator missed the fold");
}

/// The slerp: the arc-cosine table, the Taylor sine, the shortest-path flip and the early-out.
#[test]
fn the_slerp_matches_the_original() {
    let Some(mut ee) = game_or_skip() else { return };
    install_tables(&mut ee);
    let slerp = addr("motion_slerp").unwrap();
    let mut rng = Rng(0x5eed_0002);
    let mut worst = Worst::default();
    let (mut flipped, mut same, mut ends) = (0u32, 0u32, 0u32);
    for n in 1..=SAMPLES {
        // The quaternions a clip actually hands the slerp: two decoded keys. Decoding is proved
        // above, so using it here is not circular — it is the generator producing the right shape of
        // number. One pair in eight is the same key twice, which is what reaches the early-out.
        let a = rng.key(0.0);
        let b = if rng.below(8) == 0 { a } else { rng.key(0.0) };
        let (from, to) = (skin::decode(&a), skin::decode(&b));
        // The weight, including both ends exactly, since a clip's first and last frames land there.
        let t = match rng.below(16) {
            0 => 0.0,
            1 => 1.0,
            _ => rng.below(1 << 16) as f32 / 65_536.0,
        };
        ends += (t == 0.0 || t == 1.0) as u32;
        let shortest = rng.below(4) != 0;

        for (at, q) in [(FROM, from), (TO, to)] {
            for k in 0..4 {
                ee.set_f32(at + 4 * k as u32, q[k]);
            }
        }
        ee.set_f32(OUT, f32::NAN);
        ee.call(slerp, &[OUT as u64, FROM as u64, TO as u64, shortest as u64], &[t], BUDGET);

        let ours = skin::slerp(t, from, to, shortest);
        let dot = (0..4).map(|k| from[k] * to[k]).sum::<f32>();
        // The lanes are axis-scaled like the decoder's, so the tolerance is the same size; the slerp
        // divides by a sine, so it is a little looser than a single multiply would need.
        let mut bad = Vec::new();
        worst.quad(&ee, OUT, ours, 2e-3, &mut bad);
        assert!(bad.is_empty(), "pair {n} (t {t}, shortest {shortest}, dot {dot:e}):\n  {}", bad.join("\n  "));
        flipped += (shortest && dot < 0.0) as u32;
        same += (dot.abs() >= 1.0) as u32;
    }
    println!("{SAMPLES} blends, {flipped} of them flipped the short way round, {same} early-outs on a \
              dot product of one, {ends} at an end of the range; worst lane off by {:e} ({})",
             worst.by, worst.at);
    assert!(flipped > SAMPLES / 20, "only {flipped} pairs flipped: the shortest-path path is unproved");
    assert!(same > 0, "no pair reached the early-out: the |dot| >= 1 path is unproved");
}

/// A rotation channel, asked for a sequence of times — which proves the cached-pair hint as well as
/// the sample, because the hint is what the next call starts from. Both key kinds, since the original
/// has the whole sampler twice, once per kind.
#[test]
fn the_rotation_channel_matches_the_original() {
    prove_rotation(false);
    prove_rotation(true);
}

fn prove_rotation(narrow: bool) {
    let Some(mut ee) = game_or_skip() else { return };
    install_tables(&mut ee);
    let sample = addr(if narrow { "motion_rot_sample_narrow" } else { "motion_rot_sample" }).unwrap();
    let mut rng = Rng(if narrow { 0x5eed_0007 } else { 0x5eed_0003 });
    let mut worst = Worst::default();
    let (mut held, mut walked) = (0u32, 0u32);
    for c in 1..=CHANNELS {
        // Key counts in the range the disc uses. Three is the lowest the original's three-or-more path
        // takes; below that it reads a key before the array, which no clip on this disc reaches and
        // the port deliberately does not reproduce.
        let count = 3 + rng.below(39) as usize;
        // Frame-ish spacing, in exact binary steps, and the first key at zero as every clip on the
        // disc has it. A time *before* the first key is the one case left out: there the original
        // reads the key before the array and the port holds the first key instead, deliberately and
        // on the record — see the journal entry.
        let mut times = Vec::new();
        let mut t = 0.0f32;
        for _ in 0..count {
            times.push(t);
            t += rng.step();
        }
        let span = times[count - 1];
        let (bytes, port) = if narrow {
            let keys: Vec<NarrowKey> = times.iter().map(|&t| rng.narrow(t)).collect();
            (keys.iter().flat_map(narrow_key_bytes).collect::<Vec<u8>>(), RotChannel::new(0, keys))
        } else {
            let keys: Vec<RotKey> = times.iter().map(|&t| rng.key(t)).collect();
            (keys.iter().flat_map(rot_key_bytes).collect::<Vec<u8>>(), RotChannel::new(0, keys))
        };
        install_channel(&mut ee, &bytes, count, 1);

        for n in 0..TIMES {
            // Times that wander about the clip rather than marching through it: a clip is played
            // forwards, but a cross-fade and a stunt's held progress both jump, and the hint's two
            // guesses only fire on a jump.
            let time = match rng.below(8) {
                0 => 0.0,
                1 => span + 0.5,
                2 => times[rng.below(count as u32) as usize],
                _ => rng.below(1 << 14) as f32 / 16_384.0 * (span + 0.25),
            };
            ee.call(sample, &[CHAN as u64, OUT as u64], &[time], BUDGET);
            let ours = port.sample(time).unwrap();

            let mut bad = Vec::new();
            worst.quad(&ee, OUT, ours, 2e-3, &mut bad);
            let theirs_cursor = ee.u32_at(CHAN + 0x18) as usize;
            if theirs_cursor != port.cursor() {
                bad.push(format!("the cached pair: theirs {theirs_cursor}, ours {}", port.cursor()));
            }
            assert!(bad.is_empty(), "{} channel {c} ({count} keys over {span}s), time {n} = {time}:\n  {}",
                    if narrow { "narrow" } else { "wide" }, bad.join("\n  "));
            held += (time > span) as u32;
            walked += 1;
        }
    }
    println!("{CHANNELS} {} rotation channels, {walked} samples, {held} of them past the last key and \
              so held; worst lane off by {:e} ({})",
             if narrow { "narrow" } else { "wide" }, worst.by, worst.at);
    assert!(held > walked / 20, "only {held} samples fell past a clip: the held end is unproved");
}

/// A rotation channel that is off, or has no keys at all, writes nothing — and the port returns
/// `None` for the same two cases, which is what leaves a joint where the rest pose put it.
#[test]
fn an_empty_rotation_channel_writes_nothing() {
    let Some(mut ee) = game_or_skip() else { return };
    install_tables(&mut ee);
    let sample = addr("motion_rot_sample").unwrap();
    let keys = vec![RotKey { t: 0.0, turn: 10, axis: [1, 2, 3] }; 4];
    let bytes: Vec<u8> = keys.iter().flat_map(|k| rot_key_bytes(k)).collect();
    for (what, on, count) in [("switched off", 0u32, keys.len()), ("empty", 1, 0)] {
        install_channel(&mut ee, &bytes, count, 1);
        ee.set_u32(CHAN + 4, on);
        for k in 0..4 {
            ee.set_f32(OUT + 4 * k, 12.5);
        }
        ee.call(sample, &[CHAN as u64, OUT as u64], &[0.5], BUDGET);
        for k in 0..4 {
            assert_eq!(ee.f32_at(OUT + 4 * k), 12.5, "a {what} channel wrote lane {k}");
        }
        let port = RotChannel::new(0, if count == 0 { Vec::new() } else { keys.clone() });
        assert_eq!(port.sample(0.5).is_none(), count == 0, "the port disagrees about a {what} channel");
    }
    println!("a channel with no keys writes nothing, and nor does one switched off — which the port \
              models as no channel at all, since nothing on this disc switches one off");
}

/// A position channel, asked for a sequence of times: the binary search, the linear blend, and the
/// extrapolation past the last key.
#[test]
fn the_position_channel_matches_the_original() {
    let Some(mut ee) = game_or_skip() else { return };
    install_tables(&mut ee);
    let sample = addr("motion_pos_sample").unwrap();
    let mut rng = Rng(0x5eed_0004);
    let mut worst = Worst::default();
    let (mut past, mut before, mut walked) = (0u32, 0u32, 0u32);
    for c in 1..=CHANNELS {
        // Two keys is the fewest the original's real path takes.
        let count = 2 + rng.below(40) as usize;
        let mut keys = Vec::new();
        let mut t = 0.0f32;
        for _ in 0..count {
            keys.push(PosKey { t, p: [rng.coord(200), rng.coord(200), rng.coord(200)] });
            t += rng.step();
        }
        let span = keys[count - 1].t;
        let bytes: Vec<u8> = keys.iter().flat_map(|k| pos_key_bytes(k)).collect();
        install_channel(&mut ee, &bytes, count, 0);
        let port = PosChannel::new(0, keys.clone());

        for n in 0..TIMES {
            // As with the rotation channel, a time before the first key is left out on purpose: the
            // original brackets the key *before* the array there, and the port holds the first key.
            let time = match rng.below(8) {
                0 => 0.0,
                1 => span + rng.below(1 << 10) as f32 / 1024.0,
                2 => keys[rng.below(count as u32) as usize].t,
                _ => rng.below(1 << 14) as f32 / 16_384.0 * span,
            };
            ee.call(sample, &[CHAN as u64, OUT as u64], &[time], BUDGET);
            let ours = port.sample(time).unwrap();

            // Translations run to a couple of hundred units, and past the last key the slope is
            // extrapolated, so this is a relative tolerance.
            let mut bad = Vec::new();
            for (k, lane) in ["x", "y", "z"].iter().enumerate() {
                let theirs = ee.f32_at(OUT + 4 * k as u32);
                let tol = 1e-4 * (1.0 + theirs.abs());
                if let Some(b) = worst.check(lane, theirs, ours[k], tol) {
                    bad.push(b);
                }
            }
            assert!(bad.is_empty(), "channel {c} ({count} keys over {span}s), time {n} = {time}:\n  {}",
                    bad.join("\n  "));
            assert_eq!(ours[3], 1.0, "the port's translation is not a point");
            past += (time > span) as u32;
            before += (time == 0.0) as u32;
            walked += 1;
        }
    }
    println!("{CHANNELS} position channels, {walked} samples, {past} of them past the last key (and so \
              extrapolated, not held), {before} exactly on the first; worst lane off by {:e} ({})",
             worst.by, worst.at);
    assert!(past > walked / 20, "only {past} samples ran past the end: the extrapolation is unproved");
}

/// A quaternion as a matrix: the three rows the original writes into a transform.
#[test]
fn the_quaternion_matrix_matches_the_original() {
    let Some(mut ee) = game_or_skip() else { return };
    install_tables(&mut ee);
    // The call ends by marking the transform chain dirty, which walks a tree this harness has not
    // built; the rows are what is being proved, so the mark is hooked out.
    ee.hooks.insert(addr("transform_dirty").unwrap(), Box::new(|_| {}));
    let set = addr("transform_set_rotation").unwrap();
    let mut rng = Rng(0x5eed_0005);
    let mut worst = Worst::default();
    for n in 1..=SAMPLES {
        // Unit quaternions, which is what a decoded key is once the axis is scaled: build one the way
        // the game does and normalise it, so the matrix is a rotation and its rows are of size one.
        let q = {
            let d = skin::decode(&rng.key(0.0));
            let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2] + d[3] * d[3]).sqrt();
            if len < 1e-6 { [0.0, 0.0, 0.0, 1.0] } else { [0, 1, 2, 3].map(|k| d[k] / len) }
        };
        for k in 0..4 {
            ee.set_f32(FROM + 4 * k as u32, q[k]);
        }
        ee.call(set, &[XFORM as u64, FROM as u64, 0], &[], BUDGET);

        let ours = skin::rotation(q);
        let mut bad = Vec::new();
        for row in 0..3 {
            worst.quad(&ee, XFORM + 0x40 + 0x10 * row as u32, ours.0[row], 1e-5, &mut bad);
        }
        assert!(bad.is_empty(), "quaternion {n} ({q:?}):\n  {}", bad.join("\n  "));
    }
    println!("{SAMPLES} quaternions as matrices, three rows each; worst element off by {:e} ({})",
             worst.by, worst.at);
}

// --- the two tables, against the game's own copies ---------------------------------------------------

/// The turn and arc-cosine tables the port rebuilds, against the game's own, out of a save state.
///
/// The builder is not in the image's call graph — both tables are filled at start-up through pointers
/// the disassembly does not resolve — so the port rebuilds them from `sin`, `cos` and `acos`. That is
/// a guess about *provenance*, and this is the test that stops it being a guess about *values*.
#[test]
fn the_tables_are_the_games_own() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => return skip(&why),
    };
    let at = kit.context.join("states/human-ram.bin");
    let ram = match std::fs::read(&at) {
        Ok(r) => r,
        Err(e) => return skip(&format!("{}: {e} — no save state to read the game's tables out of", at.display())),
    };
    let read = |va: u32| -> f32 {
        let a = (va & 0x0fff_ffff) as usize;
        f32::from_bits(u32::from_le_bytes([ram[a], ram[a + 1], ram[a + 2], ram[a + 3]]))
    };
    let t = tables();

    let mut worst = [0.0f32; 2];
    for (k, (name, ours)) in [("motion_turn_table", &t.turn), ("motion_turn_table_narrow", &t.narrow)]
        .into_iter()
        .enumerate()
    {
        let at = addr(name).unwrap();
        for (n, [sin, cos]) in ours.iter().enumerate() {
            let (their_sin, their_cos) = (read(at + 8 * n as u32), read(at + 8 * n as u32 + 4));
            worst[k] = worst[k].max((their_sin - sin).abs()).max((their_cos - cos).abs());
            assert_eq!(their_cos.is_sign_negative(), cos.is_sign_negative(),
                       "{name} entry {n}: the cosine's sign is what the decoder folds into the axis");
        }
        assert!(worst[k] <= 2e-7, "{name} is off by {:e}", worst[k]);
    }
    let (worst_turn, worst_narrow) = (worst[0], worst[1]);

    let arc_at = read(addr("motion_arc_table").unwrap()).to_bits();
    let mut worst_arc = 0.0f32;
    for (i, v) in t.arc.iter().enumerate() {
        let theirs = read(arc_at + 4 * i as u32);
        worst_arc = worst_arc.max((theirs - v).abs());
    }
    assert!(worst_arc <= 1.2e-5, "the arc-cosine table is off by {worst_arc:e}");

    println!("the game's own tables, out of {}: 4096 turns off by at most {worst_turn:e}, 256 narrow \
              turns by at most {worst_narrow:e}, {} arc cosines (at {arc_at:#08x}) by at most \
              {worst_arc:e}", at.display(), t.arc.len());
}

// --- the format, against the disc --------------------------------------------------------------------

/// Every `.mot` in every archive, parsed, with the invariants the port leans on checked on all of them.
#[test]
fn every_clip_on_the_disc_parses() {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => return skip(&why),
    };
    let archives: Vec<String> = disc
        .entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with(".res"))
        .map(|e| e.name.clone())
        .collect();
    let (mut clips, mut distinct) = (0u32, BTreeSet::new());
    let (mut least, mut most) = (usize::MAX, 0usize);
    let (mut wide, mut narrow) = (0u32, 0u32);
    // Channels too short for the original's own search, which reads the key *before* the array to
    // weight them. The port reads the first key instead; these are counted so the journal can say how
    // much of the disc that touches.
    let (mut short_rot, mut short_pos) = (0u32, 0u32);
    let mut highest_turn = 0u16;
    for a in &archives {
        let Ok(res) = disc.res(a) else { continue };
        let names: Vec<String> = res
            .entries()
            .iter()
            .filter(|e| e.name.to_ascii_lowercase().ends_with(".mot"))
            .map(|e| e.name.clone())
            .collect();
        for n in &names {
            let bytes = res.read(n).unwrap_or_else(|e| panic!("{a} :: {n}: {e}"));
            let clip = Clip::parse(&bytes).unwrap_or_else(|e| panic!("{a} :: {n}: {e}"));
            clips += 1;
            distinct.insert(n.to_ascii_lowercase());

            let where_ = format!("{a} :: {n}");
            assert_eq!(clip.version, 3, "{where_}: the port only reads version 3");
            assert!(clip.duration() > 0.0, "{where_}: a duration of {}", clip.duration());
            assert!(clip.frames > 0, "{where_}: {} frames", clip.frames);
            assert!(!clip.names.is_empty(), "{where_}: no name table");
            for c in &clip.rotation {
                let count = c.keys.len();
                assert!(count >= 1, "{where_}: a rotation channel with no keys");
                short_rot += (count < 3) as u32;
                // Strictly increasing, with the first key at zero: those two are what let the proofs
                // leave out a zero-length interval and a time before the first key.
                assert_eq!(c.keys.time(0), 0.0, "{where_}: a rotation channel starts at {}", c.keys.time(0));
                let count = c.keys.len();
                assert!((1..count).all(|i| c.keys.time(i - 1) < c.keys.time(i)),
                        "{where_}: a rotation channel's times do not increase");
                assert!(usize::try_from(c.name).map(|k| k < clip.names.len()).unwrap_or(false),
                        "{where_}: a rotation channel names {} of {} names", c.name, clip.names.len());
                match &c.keys {
                    RotKeys::Wide(keys) => {
                        wide += 1;
                        for k in keys {
                            assert!(k.turn < 4096, "{where_}: a turn index of {} — outside the table, \
                                                    so the port's mask would change the look", k.turn);
                            highest_turn = highest_turn.max(k.turn);
                        }
                    }
                    RotKeys::Narrow(_) => narrow += 1, // a byte of turn is inside its table by construction
                }
                least = least.min(count);
                most = most.max(count);
            }
            for c in &clip.position {
                assert!(!c.keys.is_empty(), "{where_}: a position channel with no keys");
                short_pos += (c.keys.len() < 2) as u32;
                assert_eq!(c.keys[0].t, 0.0, "{where_}: a position channel starts at {}", c.keys[0].t);
                assert!(c.keys.windows(2).all(|w| w[0].t < w[1].t),
                        "{where_}: a position channel's times do not increase");
                assert!(usize::try_from(c.name).map(|k| k < clip.names.len()).unwrap_or(false),
                        "{where_}: a position channel names {} of {} names", c.name, clip.names.len());
                least = least.min(c.keys.len());
                most = most.max(c.keys.len());
            }

            // And it samples: every channel, ten times across the clip and a little past both ends,
            // has to come out finite, and every rotation has to still be a unit quaternion.
            // How round a quaternion out of this clip can be: the axis is quantised to the key's own
            // scale, so a narrow clip's rotations are off unit by up to a part in a few hundred. That
            // is the format's own coarseness, not the sampler's — the samplers are proved lane by lane
            // against the original above.
            let round = if clip.rotation.iter().any(|c| matches!(c.keys, RotKeys::Narrow(_))) {
                1e-2
            } else {
                1e-3
            };
            let map: Vec<u8> =
                (0..clip.names.len()).map(|k| if k < SKIP as usize { k as u8 } else { SKIP }).collect();
            let mut pose = Pose::new(clip.names.len());
            for step in -1..=11 {
                let time = clip.duration() * step as f32 / 10.0;
                clip.pose(time, &map, &mut pose);
                for (j, q) in pose.rot.iter().enumerate() {
                    if !pose.turned[j] {
                        continue;
                    }
                    let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
                    assert!((len - 1.0).abs() < round,
                            "{where_} at {time}s: joint {j}'s rotation is {len} long, not one");
                }
                for (j, p) in pose.pos.iter().enumerate() {
                    assert!(!pose.moved[j] || p.iter().all(|v| v.is_finite()),
                            "{where_} at {time}s: joint {j}'s translation is {p:?}");
                }
            }
        }
    }
    assert!(clips > 100, "only {clips} clips found: the archive scan is not finding them");
    println!("{clips} `.mot` members across {} archives ({} distinct names), channels of {least}..{most} \
              keys, {wide} rotation channels of twelve-byte keys and {narrow} of eight-byte ones, the \
              highest turn index {highest_turn} of 4095; every one version 3 and position kind 0, every \
              channel's times increasing from zero, and every sample a unit quaternion. {short_rot} \
              rotation channels and {short_pos} position channels are shorter than the original's own \
              search needs; the port weights those from the first key rather than from the heap byte \
              in front of it",
             archives.len(), distinct.len());
    assert!(narrow > 0, "no eight-byte rotation channel was found: that key kind is unproved");
}

/// A real rider, posed and skinned: the clip's names resolved against the model's own node tree, the
/// pose composed down the tree, and the triangles that come out of it.
#[test]
fn a_rider_is_posed_and_skinned_by_its_own_clips() {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(why) => return skip(&why),
    };
    let res = match disc.res("Rider\\wey1.res") {
        Ok(r) => r,
        Err(why) => return skip(&format!("Rider\\wey1.res: {why}")),
    };
    let model = Model::parse(&res.read_suffix("mrider.psg").unwrap()).unwrap();
    let skel = Skeleton::of(&model);
    let rest = skel.world(&Pose::new(skel.len()));

    let mut posed = 0;
    for name in ["rider\\anim\\turn_stop_fwd.mot", "rider\\anim\\backflip.mot"] {
        let clip = Clip::parse(&res.read(name).unwrap()).unwrap();
        let map = clip.joints(&skel);
        let named = map.iter().filter(|&&j| j != SKIP).count();
        assert!(named * 2 > map.len(),
                "{name}: only {named} of {} channel names are joints of mrider.psg — the clip and the \
                 model do not belong together", map.len());

        let mut pose = Pose::new(skel.len());
        let mut moved = 0;
        for step in 0..=10 {
            let time = clip.duration() * step as f32 / 10.0;
            clip.pose(time, &map, &mut pose);
            let world = skel.world(&pose);
            assert_eq!(world.len(), skel.len());
            for (j, m) in world.iter().enumerate() {
                assert!(m.0.iter().flatten().all(|v| v.is_finite()),
                        "{name} at {time}s: joint {j}'s matrix is {m:?}");
            }
            moved += world.iter().zip(&rest).filter(|(a, b)| a != b).count();

            let tris = skin::skin(&model, &world, 0);
            assert!(!tris.is_empty(), "{name}: the model's first LOD group drew nothing");
            for t in &tris {
                for v in &t.v {
                    assert!(v.pos.x.is_finite() && v.pos.y.is_finite() && v.pos.z.is_finite(),
                            "{name} at {time}s: a vertex went to {:?}", v.pos);
                }
            }
            posed += 1;
        }
        assert!(moved > 0, "{name}: the clip moved no joint off its rest pose");
        println!("{name}: {} channels, {named} of them joints of mrider.psg, {} joints moved across \
                  eleven poses", map.len(), moved);
    }
    println!("{posed} poses skinned onto mrider.psg's {} joints", skel.len());
}

// --- the plumbing, port only -------------------------------------------------------------------------

/// The clip's name-to-joint map, including the byte that means "skip this channel".
#[test]
fn a_channel_whose_name_is_no_joint_is_skipped() {
    let skel = bones(&["root", "Hips", "spine"]);
    let clip = clip_of(&["HIPS", "elbow", "root"]);
    let map = clip.joints(&skel);
    assert_eq!(map, vec![1, SKIP, 0], "the map is case-insensitive, and 0xff is 'no joint'");

    let mut pose = Pose::new(skel.len());
    clip.pose(0.0, &map, &mut pose);
    assert_eq!(pose.turned, vec![true, true, false], "the elbow channel should have written nothing");
    assert_eq!(pose.moved, vec![true, true, false]);

    // And a map that names a joint the pose does not have is skipped too, rather than panicking.
    let mut small = Pose::new(1);
    clip.pose(0.0, &map, &mut small);
    assert_eq!(small.turned, vec![true]);
}

/// The cross-fade's two ends: all of the clip at zero, all of the pose it fades from at one.
#[test]
fn the_cross_fade_ends_are_the_clip_and_the_pose_under_it() {
    let clip = clip_of(&["a"]);
    let map = vec![0u8];
    let mut plain = Pose::new(1);
    clip.pose(0.25, &map, &mut plain);

    let mut under = Pose::new(1);
    under.rot[0] = [0.0, 0.707_106_77, 0.0, 0.707_106_77];
    under.pos[0] = [9.0, 8.0, 7.0, 1.0];

    let mut out = Pose::new(1);
    clip.pose_faded(0.25, &map, 0.0, &under, &mut out);
    assert_eq!(out.pos[0], plain.pos[0], "a fade of zero is all of the clip");
    assert_eq!(out.rot[0], plain.rot[0]);

    clip.pose_faded(0.25, &map, 1.0, &under, &mut out);
    for k in 0..3 {
        assert!((out.pos[0][k] - under.pos[0][k]).abs() < 1e-5, "a fade of one is all of the pose under it");
    }
    let dot: f32 = (0..4).map(|k| out.rot[0][k] * under.rot[0][k]).sum();
    assert!(dot.abs() > 0.999, "a fade of one should land on the pose under it, not {:?}", out.rot[0]);

    // Halfway is between the two, and nowhere near either.
    clip.pose_faded(0.25, &map, 0.5, &under, &mut out);
    for k in 0..3 {
        let (a, b) = (plain.pos[0][k], under.pos[0][k]);
        assert!((out.pos[0][k] - 0.5 * (a + b)).abs() < 1e-4, "halfway is halfway: {out:?}");
    }

    // A blend buffer shorter than the pose is a caller's mistake, not a panic.
    clip.pose_faded(0.25, &map, 1.0, &Pose::new(0), &mut out);
    assert_eq!(out.pos[0], plain.pos[0], "with nothing to fade from, the clip is all there is");
}

/// The two-clip mix's ends, which is how the airborne idle blends a lean.
#[test]
fn the_two_clip_mix_ends_are_the_two_clips() {
    let a = clip_of(&["j"]);
    let b = {
        let mut c = clip_of(&["j"]);
        c.position[0] = PosChannel::new(0, vec![
            PosKey { t: 0.0, p: [10.0, 20.0, 30.0] },
            PosKey { t: 1.0, p: [10.0, 20.0, 30.0] },
        ]);
        c
    };
    let map = vec![0u8];
    let (mut only_a, mut only_b, mut mixed) = (Pose::new(1), Pose::new(1), Pose::new(1));
    a.pose(0.5, &map, &mut only_a);
    b.pose(0.5, &map, &mut only_b);

    a.pose_mixed(&b, 0.5, 1.0, &map, &mut mixed);
    for k in 0..3 {
        assert!((mixed.pos[0][k] - only_a.pos[0][k]).abs() < 1e-4, "a mix of one is all of this clip");
    }
    a.pose_mixed(&b, 0.5, 0.0, &map, &mut mixed);
    for k in 0..3 {
        assert!((mixed.pos[0][k] - only_b.pos[0][k]).abs() < 1e-4, "a mix of zero is all of the other");
    }
    a.pose_mixed(&b, 0.5, 0.5, &map, &mut mixed);
    for k in 0..3 {
        let want = 0.5 * (only_a.pos[0][k] + only_b.pos[0][k]);
        assert!((mixed.pos[0][k] - want).abs() < 1e-4, "halfway is halfway: {:?}", mixed.pos[0]);
    }

    // A clip with fewer channels than its partner leaves the missing ones alone rather than panicking.
    let short = clip_of(&[]);
    let mut out = Pose::new(1);
    a.pose_mixed(&short, 0.5, 0.5, &map, &mut out);
    assert_eq!(out.turned, vec![false], "there was nothing to mix with");
}

/// A pose's rotation and translation reach the joint's own matrix, and a joint's parents reach it too —
/// including a parent that comes *after* its child in the node list, which `.psg` files do.
#[test]
fn a_joint_is_placed_by_its_parents() {
    // Three joints: a child of a parent of the root, with the parent listed last.
    let mut skel = bones(&["child", "root", "parent"]);
    for (j, parent) in [Some(2), None, Some(1)].into_iter().enumerate() {
        skel.bones[j].parent = parent;
    }
    for (j, step) in [(0usize, 1.0f32), (1, 10.0), (2, 100.0)] {
        skel.bones[j].local.0[3] = [step, 0.0, 0.0, 1.0];
    }
    let world = skel.world(&Pose::new(3));
    assert_eq!(world[0].0[3][0], 111.0, "the child should carry its parent and its grandparent");
    assert_eq!(world[2].0[3][0], 110.0);
    assert_eq!(world[1].0[3][0], 10.0);

    // A pose's translation replaces the rest pose's, and only for the joints it wrote.
    let mut pose = Pose::new(3);
    pose.pos[0] = [5.0, 0.0, 0.0, 1.0];
    pose.moved[0] = true;
    let world = skel.world(&pose);
    assert_eq!(world[0].0[3][0], 115.0, "the clip's translation, not the rest pose's");

    // A quarter turn about z on the parent swings the child round, which is the whole point of the
    // chain: the child's own matrix is unchanged and its world position is not.
    let mut pose = Pose::new(3);
    pose.rot[2] = [0.0, 0.0, 0.707_106_77, 0.707_106_77];
    pose.turned[2] = true;
    let world = skel.world(&pose);
    assert!(world[0].0[3][1].abs() > 0.9, "the child should have swung off the x axis: {:?}", world[0].0[3]);

    // A node whose parent is itself gets its own transform rather than hanging.
    for (j, parent) in [Some(0), None, Some(2)].into_iter().enumerate() {
        skel.bones[j].parent = parent;
    }
    let world = skel.world(&Pose::new(3));
    assert_eq!(world[0].0[3][0], 1.0);

    // Forgetting which joints were written keeps what was in them, which is what a blend buffer is.
    let mut pose = Pose::new(1);
    pose.rot[0] = [1.0, 0.0, 0.0, 0.0];
    pose.turned[0] = true;
    pose.hold();
    assert_eq!(pose.rot[0], [1.0, 0.0, 0.0, 0.0]);
    assert!(!pose.turned[0]);
}

/// A file that is not a `.mot` is refused, and so is one whose channels do not fill it.
#[test]
fn a_file_that_is_not_a_clip_is_refused() {
    assert!(Clip::parse(b"not a motion").is_err());
    assert!(Clip::parse(&[]).is_err());
    // A version 1 header: a positive frame count, which the port does not read.
    let mut v1 = Vec::new();
    v1.extend_from_slice(&30i32.to_le_bytes());
    assert!(Clip::parse(&v1).is_err());
    // A well-formed version 3 header with a key kind that exists nowhere, in either field.
    for (rot, pos) in [(2i32, 0i32), (1, 1)] {
        let mut odd: Vec<u8> = Vec::new();
        for v in [-3i32, 30, 0] {
            odd.extend_from_slice(&v.to_le_bytes());
        }
        odd.extend_from_slice(&1.0f32.to_bits().to_le_bytes());
        for v in [rot, pos, 0, 0] {
            odd.extend_from_slice(&v.to_le_bytes());
        }
        let why = Clip::parse(&odd).unwrap_err();
        assert!(why.contains(&format!("rotation kind {rot} and position kind {pos}")), "{why}");
    }
    // A header the port does read: both kinds, no channels, and nothing after them.
    for rot in [0i32, 1] {
        let mut bare: Vec<u8> = Vec::new();
        for v in [-3i32, 30, 0] {
            bare.extend_from_slice(&v.to_le_bytes());
        }
        bare.extend_from_slice(&1.0f32.to_bits().to_le_bytes());
        for v in [rot, 0, 0, 0] {
            bare.extend_from_slice(&v.to_le_bytes());
        }
        assert!(Clip::parse(&bare).is_ok(), "a clip of rotation kind {rot} should parse");
    }
}

// --- small fixtures ---------------------------------------------------------------------------------

/// A skeleton of named joints, all at the origin and all roots.
fn bones(names: &[&str]) -> Skeleton {
    let nodes: Vec<Node> = names
        .iter()
        .map(|n| Node {
            name: (*n).to_string(),
            local: M4::IDENTITY,
            parent: None,
            center: Default::default(),
            half_extents: Default::default(),
            radius: 0.0,
        })
        .collect();
    Skeleton::of(&Model { nodes, materials: vec![], meshes: vec![], groups: 1, group_distance: vec![f32::MAX] })
}

/// A one-second clip with one rotation and one position channel per name, each of two keys that move.
fn clip_of(names: &[&str]) -> Clip {
    let (mut rotation, mut position) = (Vec::new(), Vec::new());
    for (k, _) in names.iter().enumerate() {
        rotation.push(RotChannel::new(k as i32, vec![
            RotKey { t: 0.0, turn: 0, axis: [0, 0, 0] },
            RotKey { t: 0.5, turn: 512, axis: [32767, 0, 0] },
            RotKey { t: 1.0, turn: 1024, axis: [32767, 0, 0] },
        ]));
        position.push(PosChannel::new(k as i32, vec![
            PosKey { t: 0.0, p: [0.0, 0.0, 0.0] },
            PosKey { t: 0.5, p: [1.0, 2.0, 3.0] },
            PosKey { t: 1.0, p: [2.0, 4.0, 6.0] },
        ]));
    }
    Clip {
        version: 3,
        frames: 30,
        names: names.iter().map(|n| (*n).to_string()).collect(),
        timing: game_sim::motion::Clip::new(ps2_float::F(1.0), false),
        position,
        rotation,
    }
}
