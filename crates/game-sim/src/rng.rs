//! The game's random numbers.
//!
//! The original does not have a hand-written generator: it uses the one its C library shipped with, a
//! 64-bit linear congruential generator whose state is a single word in the library's per-thread data,
//! and it calls it from everywhere — a couple of hundred call sites across physics, AI, menus and
//! presentation. That makes this the first thing a port has to get exactly right: every later proof that
//! involves a bike, a rider or a camera reads this sequence, and a generator that is one call out of step
//! turns every one of those proofs into noise.
//!
//! Three shapes, because three shapes are all the original ever uses:
//!
//! * [`Rng::rand`] — the library's `rand()`: a 31-bit non-negative integer.
//! * [`Rng::unit`] — `rand()` scaled by 2⁻³¹ into `[0, 1)`. Nearly two hundred call sites do exactly
//!   this, inline, and they all do it in the same operand order, which is the order here.
//! * [`Rng::index`] — a uniform index into a list of `count` entries, via the float above. The original
//!   has this as a template it instantiates per list type ("pick one of these at random, and tell me
//!   which one you picked"), and the instantiations are instruction-for-instruction identical.
//!
//! Two things that look like details and are not:
//!
//! * The state is 64 bits and the seed is 32. Seeding can only ever reach a quarter of the state space,
//!   but a *running* game is always somewhere in the middle of the chain, so [`Rng::from_state`] exists
//!   and a replay fixture stores the full 64-bit word, never the seed it came from.
//! * The scaling to a float goes through the EE's FPU, which chops where IEEE rounds. `rand()` returns up
//!   to 2³¹−1, well past the 24 bits an `f32` holds exactly, so the integer-to-float conversion loses its
//!   low bits *toward zero* — and that lost bit is the difference between picking entry 3 and entry 4.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use ps2_float::{F, fpu};

/// The LCG's multiplier, as the library's `rand` holds it (built in the delay slots with four
/// instructions, which is why it is worth saying out loud that it is one constant).
const MULTIPLIER: u64 = 0x5851_f42d_4c95_7f2d;

/// The LCG's increment.
const INCREMENT: u64 = 1;

/// The largest value [`Rng::rand`] can return — the library's `RAND_MAX`, and the mask the original
/// applies to the top half of the state.
pub const RAND_MAX: i32 = 0x7fff_ffff;

/// The state the program image is linked with: the library's static data has a 1 in it, so a game that
/// never seeds still produces a defined sequence (and the first number it ever asks for is the same on
/// every console).
pub const INITIAL_STATE: u64 = 1;

/// 2⁻³¹, the factor every call site multiplies `rand()` by to get a float in `[0, 1)`.
///
/// Exact as an `f32` (it is a power of two), so the multiply itself cannot round; all of the error in a
/// random float comes from the integer conversion before it.
const UNIT_SCALE: f32 = 4.656_613e-10;

/// The game's random number generator: one 64-bit word, and nothing else.
///
/// It is `Copy` so that a snapshot of the simulation is a copy, and so that "what would the next three
/// numbers be" can be asked without disturbing anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rng {
    pub(crate) state: u64,
}

impl Default for Rng {
    /// The generator the game starts with, before anything seeds it: see [`INITIAL_STATE`].
    fn default() -> Rng {
        Rng::new()
    }
}

impl Rng {
    /// The generator as the program image holds it, unseeded.
    pub const fn new() -> Rng {
        Rng {
            state: INITIAL_STATE,
        }
    }

    /// Seeded, the way the original's `srand` does it: the seed is the whole state, zero-extended from
    /// 32 bits. The 32 high bits of the state start empty, which is why the first number out of a
    /// freshly seeded generator is a plain function of the seed.
    pub const fn from_seed(seed: u32) -> Rng {
        Rng {
            state: seed as u64,
        }
    }

    /// Re-seed in place: the original's `srand`.
    pub fn seed(&mut self, seed: u32) {
        self.state = seed as u64;
    }

    /// The whole state, for a snapshot, a replay fixture or a comparison against the real thing.
    pub const fn state(&self) -> u64 {
        self.state
    }

    /// A generator resumed mid-chain. This — not [`from_seed`](Rng::from_seed) — is what a capture of a
    /// game already in play gives you.
    pub const fn from_state(state: u64) -> Rng {
        Rng { state }
    }

    /// One call of the library's `rand()`: advance the state, then hand back the top half of it with the
    /// sign bit masked off, so the result is in `0..=RAND_MAX`.
    ///
    /// The order matters for anyone comparing against a memory dump: the state is advanced *before* the
    /// value is taken from it, so the word in RAM after `n` calls is the state the `n`th result came out
    /// of, not the one the next result will come out of.
    pub fn rand(&mut self) -> i32 {
        self.state = self.state.wrapping_mul(MULTIPLIER).wrapping_add(INCREMENT);
        ((self.state >> 32) as i32) & RAND_MAX
    }

    /// `rand()` as a float in `[0, 1)`: one call, converted to a single and scaled by 2⁻³¹.
    ///
    /// The original multiplies with the scale on the left and the converted integer on the right, and so
    /// does this. (It makes no difference to the result — it is the operand order of a `mul.s`, kept
    /// because the next person to read both wants them to line up.)
    pub fn unit(&mut self) -> F {
        let r = self.rand();
        F(UNIT_SCALE) * F::from(r)
    }

    /// A uniform index into a list of `count` entries: `(count × unit)` truncated toward zero.
    ///
    /// Exactly the original's three instructions — convert the count, multiply by the random float,
    /// `cvt.w.s` — so it inherits their edge cases rather than being corrected for them: the count is
    /// converted as a signed integer, the multiply saturates at the FPU's largest magnitude instead of
    /// going infinite, and the truncation saturates at `i32::MAX`. A `count` of zero gives zero, which is
    /// one past the end of an empty list; the original indexes with it, and callers are expected not to
    /// ask.
    pub fn index(&mut self, count: i32) -> i32 {
        let unit = self.unit();
        fpu::to_i32((F::from(count) * unit).to_f32())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scale is 2⁻³¹ *exactly*: a literal with one digit too few would be a different float and
    /// every random index in the game would be subtly wrong. `tests/rng.rs` would catch it against the
    /// original; this catches it without a disc.
    #[test]
    fn the_unit_scale_is_a_power_of_two() {
        assert_eq!(UNIT_SCALE.to_bits(), 0x3000_0000);
    }

    /// The first value out of the unseeded generator, worked by hand from the multiplier: state becomes
    /// `1 × M + 1`, and the result is its top half with the sign bit cleared.
    #[test]
    fn the_unseeded_sequence_starts_where_the_image_says() {
        let mut rng = Rng::new();
        assert_eq!(rng.rand(), 0x5851_f42d);
        assert_eq!(rng.state(), MULTIPLIER + 1);
    }

    /// Seeding is a 32-bit store into a 64-bit state, so a seed and that seed with high bits set are not
    /// the same generator — and `seed` must agree with `from_seed`.
    #[test]
    fn seeding_clears_the_high_half() {
        let mut rng = Rng::from_state(!0);
        rng.seed(7);
        assert_eq!(rng.state(), 7);
        assert_eq!(rng, Rng::from_seed(7));
    }

    /// `rand` is never negative and never above `RAND_MAX`, over enough of the chain to be worth saying.
    #[test]
    fn the_sequence_stays_in_range() {
        let mut rng = Rng::from_seed(0);
        for _ in 0..10_000 {
            let r = rng.rand();
            assert!((0..=RAND_MAX).contains(&r), "{r}");
        }
    }
}
