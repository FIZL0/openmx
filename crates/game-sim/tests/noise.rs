//! S41 — the engine's shared smooth noise against the original, on the save state beside C7's
//! biplane recording (the session the plane's engine note is sampled in).
//!
//! Two oracle proofs:
//!
//! 1. **the sample**: the state's own tables, already built, read into [`SmoothNoise::from_tables`],
//!    and the original's sampler run on 10,000 generated times against [`SmoothNoise::at`];
//! 2. **the build**: the tables marked unbuilt and the shared generator set to a generated state, the
//!    original's sampler run once (which builds them first), and the two tables, the generator's state
//!    after and the sample compared with [`SmoothNoise::build`] off the same state.

use game_sim::noise::{KNOTS, SmoothNoise};
use game_sim::rng::Rng;
use ps2_float::F;
use ps2_oracle::{Ee, Kit, addr, skip};

/// The save state beside C7's recording.
const STATE: &str = "biplane.p2m2_SaveState.eeMemory.bin";
const SAMPLE_BUDGET: u64 = 10_000;
const BUILD_BUDGET: u64 = 5_000_000;
/// How far up the original copies the tables (`KNOTS + 2` entries, so that a 2-D or 3-D lookup can
/// run two knots past the end without wrapping).
const COPIED: usize = KNOTS + 2;

struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    /// A time: mostly the engine's own (half its clock, so a few hundred at most), with the knots
    /// themselves, the negative side of the 4096 shift, times too big to keep a fraction, and words
    /// of every exponent.
    fn time(&mut self) -> f32 {
        match self.next() % 10 {
            0 => (self.next() % 2000) as f32 * 0.5 - 500.0,
            1 => self.range(-8192.0, 0.0),
            2 => self.range(-1.0e9, 1.0e9),
            3 => f32::from_bits(self.next() as u32 & 0xff7f_ffff),
            4 => [0.0, -0.0, -4096.0, -4095.5, 4096.0, 1.0e-30, -1.0e-30, 8.0e6, -8.0e6][self.next() as usize % 9],
            _ => self.range(0.0, 600.0),
        }
    }
}

struct At {
    sample: u32,
    unbuilt: u32,
    perm: u32,
    g1: u32,
    rand_state: u32,
}

/// The interpreter on the state's RAM, the noise's addresses, or why not.
fn noise() -> Option<(Ee, At)> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: `./kit state ram` of biplane.p2m2_SaveState.p2s", path.display()));
        return None;
    }
    let names = ["smooth_noise", "smooth_noise_unbuilt", "smooth_noise_perm", "smooth_noise_g1", "rand_state", "boot_gp"];
    let mut found = Vec::new();
    for n in names {
        match addr(n) {
            Ok(a) => found.push(a),
            Err(why) => {
                skip(&why);
                return None;
            }
        }
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(found[5]));
    let at = At { sample: found[0], unbuilt: found[1], perm: found[2], g1: found[3], rand_state: found[4] };
    Some((ee, at))
}

fn tables(ee: &Ee, at: &At, n: usize) -> (Vec<u32>, Vec<u32>) {
    let perm = (0..n).map(|k| ee.u32_at(at.perm + 4 * k as u32)).collect();
    let g1 = (0..n).map(|k| ee.u32_at(at.g1 + 4 * k as u32)).collect();
    (perm, g1)
}

#[test]
fn the_noise_samples_like_the_original() {
    let Some((mut ee, at)) = noise() else { return };
    assert_eq!(ee.u32_at(at.unbuilt), 0, "the state's noise tables are not built yet");
    let (perm, g1) = tables(&ee, &at, COPIED + KNOTS);
    let mut seen = [false; KNOTS];
    for &p in &perm[..KNOTS] {
        assert!((p as usize) < KNOTS && !seen[p as usize], "the state's permutation is not one: {p} twice or out of range");
        seen[p as usize] = true;
    }
    for k in 0..COPIED {
        assert_eq!((perm[KNOTS + k], g1[KNOTS + k]), (perm[k], g1[k]), "entry {} is not entry {k}'s copy", KNOTS + k);
    }
    let port = SmoothNoise::from_tables(
        &std::array::from_fn(|k| perm[k]),
        &std::array::from_fn(|k| F(f32::from_bits(g1[k]))),
    );
    let mut pick = Pick(0x243f_6a88_85a3_08d3);
    let cases = 10_000;
    for case in 0..cases {
        let t = pick.time();
        ee.call(at.sample, &[], &[t], SAMPLE_BUDGET);
        let (want, got) = (ee.f0(), port.at(F(t)));
        assert_eq!(got.to_bits(), want.to_bits(), "case {case}: noise({t:e}) is {want:e} in the game, {:e} in the port", got.0);
    }
    println!("smooth noise: {cases} samples on the session's own tables, bit-exact");
}

#[test]
fn the_noise_tables_build_like_the_original() {
    let Some((mut ee, at)) = noise() else { return };
    let mut pick = Pick(0x1319_8a2e_0370_7344);
    let cases = 10_000;
    for case in 0..cases {
        let state = match case {
            0 => game_sim::rng::INITIAL_STATE,
            1 => 0,
            2 => u64::MAX,
            _ => pick.next(),
        };
        let t = pick.time();
        ee.set_u32(at.unbuilt, 1);
        ee.write(at.rand_state, 8, u128::from(state));
        ee.call(at.sample, &[], &[t], BUILD_BUDGET);
        assert_eq!(ee.u32_at(at.unbuilt), 0, "case {case}: the sample left the tables marked unbuilt");

        let mut rng = Rng::from_state(state);
        let port = SmoothNoise::build(&mut rng);
        let after = ee.read(at.rand_state, 8) as u64;
        assert_eq!(rng.state(), after, "case {case}: from {state:#018x} the build leaves the generator elsewhere");
        let (perm, g1) = tables(&ee, &at, COPIED + KNOTS);
        for k in 0..COPIED + KNOTS {
            let i = k % KNOTS;
            assert_eq!(perm[k], u32::from(port.perm[i]), "case {case}: perm[{k}] from {state:#018x}");
            assert_eq!(g1[k], port.g1[i].to_bits(), "case {case}: g1[{k}] from {state:#018x}");
        }
        let (want, got) = (ee.f0(), port.at(F(t)));
        assert_eq!(got.to_bits(), want.to_bits(), "case {case}: the first sample, noise({t:e})");
    }
    println!("smooth noise: {cases} builds from generated generator states, tables, draws and first sample bit-exact");
}
