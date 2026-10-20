//! The engine's shared one-dimensional smooth noise: gradient noise over a permutation of 256 knots
//! and one random gradient per knot, eased between two knots with `(3 − 2f)·f²`.
//!
//! **The tables are session state, not data.** The original builds them the first time anything asks
//! for a sample, from the C library's `rand()` — the same generator the rest of the game draws from —
//! so the waveform is fixed by wherever the shared generator stood at that moment, and building it
//! takes 1,791 draws out of the game's sequence. A freeride seeds that generator from the wall clock
//! before it starts, so a session's tables cannot be derived from anything on the disc: a run that
//! starts in the middle of a session (every replay fixture) takes them from the save state it started
//! from ([`SmoothNoise::from_tables`]), and a run that starts the session builds them off its own
//! generator ([`SmoothNoise::build`]), at the point the original would.
//!
//! The builder also fills a two- and a three-dimensional gradient table, which nothing that samples
//! this noise reads. Their draws are taken here, in the original's order, and their values dropped.

#![deny(clippy::float_arithmetic)]

use crate::rng::Rng;
use ps2_float::{F, fpu};

/// How many knots the tables hold before they repeat.
pub const KNOTS: usize = 256;

/// The two tables one sample reads: which gradient each knot uses, and the gradients.
#[derive(Clone, Debug, PartialEq)]
pub struct SmoothNoise {
    /// A permutation of `0..256`.
    pub perm: [u8; KNOTS],
    /// One gradient per entry, `(rand() % 512 − 256) / 256`: a multiple of 1/256 in `[-1, 1)`.
    pub g1: [F; KNOTS],
}

impl SmoothNoise {
    /// The tables built the way the original builds them on its first sample, off `rng`.
    ///
    /// For every knot in turn: its one-dimensional gradient (one draw), its two-dimensional gradient
    /// (two), its three-dimensional one (three); then the identity permutation shuffled from the last
    /// entry down to the second, each swapped with entry `rand() % 256`. `rand()` is never negative,
    /// so the remainder's sign correction the original carries never fires.
    pub fn build(rng: &mut Rng) -> SmoothNoise {
        let mut perm = [0u8; KNOTS];
        let mut g1 = [F::ZERO; KNOTS];
        let mut gradient = |rng: &mut Rng| F::from(rng.rand() % 512 - 256) / F(256.0);
        for i in 0..KNOTS {
            perm[i] = i as u8;
            g1[i] = gradient(rng);
            for _ in 0..2 + 3 {
                gradient(rng);
            }
        }
        for i in (1..KNOTS).rev() {
            let j = (rng.rand() % KNOTS as i32) as usize;
            perm.swap(i, j);
        }
        SmoothNoise { perm, g1 }
    }

    /// Tables read out of a running game: the permutation as the words it keeps it in (each `0..256`)
    /// and the gradients.
    pub fn from_tables(perm: &[u32; KNOTS], g1: &[F; KNOTS]) -> SmoothNoise {
        SmoothNoise { perm: perm.map(|p| p as u8), g1: *g1 }
    }

    /// One sample at `t`.
    ///
    /// In the original's order: `t` moved up by 4096 (so that small negative times truncate the same
    /// way as positive ones), split into its knot `i` and fraction `f`; the two gradients' ramps
    /// `f·g[i]` and `(f − 1)·g[i + 1]`, and the ease `(3 − 2f)·(f·f)` between them. The knots wrap at
    /// 256, the next one included.
    pub fn at(&self, t: F) -> F {
        let shifted = F(4096.0) + t;
        let knot = shifted.to_i32();
        let f = shifted - F(fpu::from_i32(knot));
        let i = (knot & 0xff) as usize;
        let (a, b) = (self.g1[usize::from(self.perm[i])], self.g1[usize::from(self.perm[(i + 1) & 0xff])]);
        let ease = (F(3.0) - F(2.0) * f) * (f * f);
        let u = f * a;
        let v = (f - F::ONE) * b;
        u + ease * (v - u)
    }
}
