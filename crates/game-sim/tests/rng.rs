//! The RNG, proved against the game's own instructions.
//!
//! Four proofs, in rising order of what they cover:
//!
//! 1. **ten thousand consecutive calls** from the seed dumped out of the program image, comparing both
//!    the returned number *and* the 64-bit state word the original leaves in RAM. A generator that
//!    agreed on the values but not on the state would come apart the moment a capture is compared
//!    against a dump.
//! 2. **a hundred seeds**, each followed by a hundred calls, through the original's own `srand` — which
//!    is the only way to see that a seed is a 32-bit store into a 64-bit state.
//! 3. **the uniform list pick**, which is how nearly every call site actually uses the generator: the
//!    original converts the count and the random number to singles, multiplies, and truncates. Counts up
//!    to 2²³ make the index expose the random float to within a bit of its mantissa, so this is also the
//!    proof that the integer-to-float conversion chops the way the EE's does.
//! 4. **a float straight out of the generator**, bit for bit, through a real function whose only input
//!    beyond its argument is the random float.
//!
//! The seed in (1) is dumped from the image rather than from a running game: PCSX2 cannot start in this
//! environment (no display for its Qt platform plugin), and the state word is statically initialised, so
//! the image is where the seed the console starts from actually lives. (2) covers every seed the game
//! can reach afterwards and (1) walks ten thousand mid-chain states, which is what a running game is in.

use game_sim::rng::{INITIAL_STATE, Rng};
use ps2_float::F;
use ps2_oracle::{Ee, addr, game_or_skip, skip};

/// Instruction budget per call: `rand` is about sixty instructions plus the library's 64-bit multiply,
/// and the list pick is not much more, so this is three orders of magnitude of headroom.
const BUDGET: u64 = 100_000;

/// A tiny xorshift, so the generated counts are the same on every machine and a failure is reproducible
/// from the seed in the source alone.
struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }
    /// A number in `1..=n`.
    fn upto(&mut self, n: u32) -> i32 {
        (self.next() % n + 1) as i32
    }
}

/// Resolve a name from `research/oracle.txt`, or say which one is missing and skip.
///
/// Names, not addresses: no retail address appears in this crate, here or in the port.
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

/// The state word the original keeps in RAM, as a `u64`.
fn state_word(ee: &Ee, at: u32) -> u64 {
    ee.read(at, 8) as u64
}

#[test]
fn ten_thousand_calls_from_the_dumped_seed() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["rand", "rand_state"]) else {
        return;
    };
    let (rand, state) = (at[0], at[1]);

    // The dumped seed: the generator's state exactly as the program image holds it, before anything has
    // run. This is the sequence a console produces for a game that never seeds.
    let dumped = state_word(&ee, state);
    assert_eq!(
        dumped, INITIAL_STATE,
        "the image's initial state is {dumped:#018x}, the port's constant says {INITIAL_STATE:#018x}"
    );

    let mut port = Rng::from_state(dumped);
    for k in 0..10_000u32 {
        let before = port.state();
        ee.call(rand, &[], &[], BUDGET);
        let (want, got) = (ee.v0() as i32, port.rand());
        assert_eq!(
            got, want,
            "call {k} from state {before:#018x}: the game gives {want}, the port {got}"
        );
        assert_eq!(
            port.state(),
            state_word(&ee, state),
            "call {k} from state {before:#018x}: the state left in RAM differs"
        );
    }
    println!("rand: 10,000 consecutive calls and states, bit-exact");
}

#[test]
fn seeding_lands_on_the_same_sequence() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["rand", "srand", "rand_state"]) else {
        return;
    };
    let (rand, srand, state) = (at[0], at[1], at[2]);

    // The corners first — 0, 1, both halves of the word's range — then generated seeds. A seed is stored
    // into a 64-bit state, so the ones with the top bit set are the interesting ones: they prove the
    // store is a zero-extension and not a sign-extension.
    let mut pick = Pick(0x9e37_79b9_7f4a_7c15);
    let mut seeds = vec![0u32, 1, 2, 0x7fff_ffff, 0x8000_0000, u32::MAX];
    while seeds.len() < 100 {
        seeds.push(pick.next());
    }

    for &seed in &seeds {
        // Through the original's own `srand`, and with the state word left dirty from the last block, so
        // that a port that forgot to clear the high half of the state could not pass by accident.
        ee.call(srand, &[seed as u64], &[], BUDGET);
        let mut port = Rng::from_seed(seed);
        assert_eq!(
            port.state(),
            state_word(&ee, state),
            "srand({seed}): the state the game stores differs"
        );
        for k in 0..100u32 {
            ee.call(rand, &[], &[], BUDGET);
            let (want, got) = (ee.v0() as i32, port.rand());
            assert_eq!(got, want, "srand({seed}), call {k}: game {want}, port {got}");
            assert_eq!(
                port.state(),
                state_word(&ee, state),
                "srand({seed}), call {k}: the state left in RAM differs"
            );
        }
    }
    println!("srand + rand: {} seeds x 100 calls, bit-exact", seeds.len());
}

#[test]
fn the_uniform_index_matches_the_list_pick() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["random_list_pick", "srand", "rand_state"]) else {
        return;
    };
    let (pick_fn, srand, state) = (at[0], at[1], at[2]);

    // The original's argument is a little list object: a count, a word it does not read here, and a
    // pointer to the entries. Three scratch slots above the stack hold it, the entries, and the index
    // the function reports back.
    let (list, items, out) = (ee.scratch(0), ee.scratch(1), ee.scratch(2));
    const SMALL: u32 = 8; // entries that fit in one scratch slot
    for k in 0..SMALL {
        ee.write(items + k * 4, 4, 0xa500_0000u32 as u128 + k as u128);
    }

    let seed = 0x1234_5678u32;
    ee.call(srand, &[seed as u64], &[], BUDGET);
    let mut port = Rng::from_seed(seed);

    let mut pick = Pick(0x2545_f491_4f6c_dd1d);
    for k in 0..10_000u32 {
        // Half the iterations use a real table of eight entries, so the returned entry is checked as
        // well as the index. The other half uses counts up to 2^23 with the entries based at zero: the
        // index then carries twenty-three bits of the random float, which is what makes this a proof of
        // the integer-to-float conversion and not just of the truncation.
        let big = k % 2 == 1;
        let (count, base) = if big {
            (pick.upto(1 << 23), 0)
        } else {
            (pick.upto(SMALL), items)
        };
        ee.write(list, 4, count as u32 as u128);
        ee.write(list + 8, 4, base as u128);
        ee.write(out, 4, 0xdead_beefu32 as u128);

        let before = port.state();
        ee.call(pick_fn, &[list as u64, out as u64], &[], BUDGET);
        let got = port.index(count);
        assert_eq!(
            got,
            ee.i32_at(out),
            "pick {k} of {count} from state {before:#018x}: the game's index differs"
        );
        assert_eq!(
            port.state(),
            state_word(&ee, state),
            "pick {k} of {count}: the state left in RAM differs"
        );
        if !big {
            assert!((0..count).contains(&got), "index {got} is outside 0..{count}");
            assert_eq!(
                ee.v0() as u32,
                ee.u32_at(items + got as u32 * 4),
                "pick {k} of {count}: the entry the game returned is not the one at index {got}"
            );
        }
    }
    println!("random index: 10,000 picks, bit-exact (counts up to 2^23)");
}

#[test]
fn the_random_float_matches_bit_for_bit() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["random_time_step", "srand"]) else {
        return;
    };
    let (step, srand) = (at[0], at[1]);

    let seed = 0x0bad_f00du32;
    ee.call(srand, &[seed as u64], &[], BUDGET);
    let mut port = Rng::from_seed(seed);

    // The original takes a time and returns it advanced by a random fraction of seven: it floors the
    // random float at a tenth, multiplies by seven and adds. Spelled out here rather than in the port,
    // because what is being proved is `Rng::unit`, not this function — but the float it returns is an
    // exact function of that one random float, which makes it the cheapest exact view of it.
    let mut pick = Pick(0xdead_c0de_1234_5678);
    for k in 0..2_000u32 {
        let t = f32::from_bits(pick.next() % 0x4f00_0000); // a plausible time, never Inf or NaN
        ee.call(step, &[], &[t], BUDGET);

        let u = port.unit();
        let floored = if u < F(0.1) { F(0.1) } else { u };
        let want = F(t) + F(7.0) * floored;

        assert_eq!(
            ee.f0().to_bits(),
            want.to_bits(),
            "step {k} from t={t} ({:#010x}): the game gives {} ({:#010x}), the port {want} ({:#010x})",
            t.to_bits(),
            ee.f0(),
            ee.f0().to_bits(),
            want.to_bits()
        );
    }
    println!("random float: 2,000 values, bit-exact");
}
