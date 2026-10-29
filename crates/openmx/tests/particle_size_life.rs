//! P185: a new particle's size and life — [`Emt::size_and_life`] against the original's draw, run on
//! the boot ELF over a made-up emitter in RAM. The speed ratios (`InitialSizeToSpeedRatio`,
//! `LifeToSpeedRatio`) either side of 0 and the disc's own (the wreck dust's 0.1 and 5, the tire
//! dust's 0.015 and 40), speeds either side of the two ends' and on them, the moving flag both ways,
//! and the random draws the port takes as well. Every size and life must be the original's to the bit.
//!
//! The emitter's other emit path (its +0x23c, which also scales the life by the view cull) is off:
//! [`particles::Source::emit_over`] is the emitter it is off for. Without the disc the test says so
//! and passes.

#[allow(dead_code)]
#[path = "../src/particles.rs"]
mod particles;

use particles::Emt;
use ps2_oracle::{Ee, addr, skip};

/// The made-up emitter, and where the draw writes its size and life.
const EMITTER: u32 = 0x0180_0000;
const OUT: u32 = 0x0180_1000;

/// The emitter's fields the draw reads (`research/oracle.txt`, particle_life_size).
mod at {
    pub const SIZE: u32 = 0x2f4;
    pub const LIFE: u32 = 0x330;
    pub const SIZE_RATIO: u32 = 0x3b4;
    pub const LIFE_RATIO: u32 = 0x3b8;
}

struct Xs(u32);

impl Xs {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
    /// A ratio: 0, a negative one, one of the disc's, or a made-up one.
    fn ratio(&mut self) -> f32 {
        match self.next() % 6 {
            0 => 0.0,
            1 => -self.unit(),
            2 => [0.01, 0.1, 0.015, 0.04, 0.02][(self.next() % 5) as usize],
            3 => [5.0, 40.0][(self.next() % 2) as usize],
            _ => self.unit() * [0.1, 10.0, 100.0][(self.next() % 3) as usize],
        }
    }
    /// Two ends, usually low to high, sometimes the same, sometimes the wrong way round.
    fn ends(&mut self, scale: f32) -> (f32, f32) {
        let (a, b) = (self.unit() * scale, self.unit() * scale);
        match self.next() % 8 {
            0 => (a, a),
            1 => (a.max(b), a.min(b)),
            _ => (a.min(b), a.max(b)),
        }
    }
}

/// The emitter in RAM as the `.emt` read leaves it: each pair's two ends and the span between them.
fn put(ee: &mut Ee, e: &Emt) {
    for k in (0..0x400).step_by(4) {
        ee.set_u32(EMITTER + k, 0);
    }
    for (o, (low, high)) in [(at::SIZE, e.size), (at::LIFE, e.life)] {
        ee.set_f32(EMITTER + o, low);
        ee.set_f32(EMITTER + o + 4, high);
        ee.set_f32(EMITTER + o + 8, ps2_float::fpu::sub(high, low));
    }
    ee.set_f32(EMITTER + at::SIZE_RATIO, e.size_ratio);
    ee.set_f32(EMITTER + at::LIFE_RATIO, e.life_ratio);
}

#[test]
fn a_particles_size_and_life_are_the_originals() {
    let mut ee = match Ee::game() {
        Ok(ee) => ee,
        Err(why) => return skip(&why),
    };
    let Ok(draw) = addr("particle_life_size") else {
        return skip("particle_life_size is not in research/oracle.txt");
    };
    let mut xs = Xs(0x0185_51ce);
    // Cases by which way each came out: [size by speed, size drawn, life by speed, life drawn].
    let mut counts = [0usize; 4];
    for case in 0..20_000 {
        let e = Emt {
            size: xs.ends(5.0),
            life: xs.ends(4000.0),
            size_ratio: xs.ratio(),
            life_ratio: xs.ratio(),
            ..Emt::default()
        };
        let moving = xs.next() % 4 != 0;
        // A speed of any size, or one right on an end's speed by its ratio.
        let speed = match xs.next() % 5 {
            0 if e.size_ratio > 0.0 => [e.size.0, e.size.1][(xs.next() % 2) as usize] / e.size_ratio,
            1 if e.life_ratio > 0.0 => [e.life.0, e.life.1][(xs.next() % 2) as usize] / e.life_ratio,
            2 => 0.0,
            _ => xs.unit() * [5.0, 60.0, 400.0][(xs.next() % 3) as usize],
        };
        let (u_size, u_life) = (xs.unit(), xs.unit());
        put(&mut ee, &e);
        ee.set_u32(OUT, 0xdead_beef);
        ee.set_u32(OUT + 4, 0xdead_beef);
        ee.call(draw, &[u64::from(EMITTER), u64::from(OUT), u64::from(OUT + 4), u64::from(moving)], &[speed, u_size, u_life], 10_000);
        let theirs = (ee.f32_at(OUT), ee.f32_at(OUT + 4));
        let ours = e.size_and_life(speed, moving, u_size, u_life);
        assert_eq!(
            [ours.0.to_bits(), ours.1.to_bits()],
            [theirs.0.to_bits(), theirs.1.to_bits()],
            "case {case}: speed {speed}, moving {moving}, draws {u_size} {u_life}, size {:?} by {}, life {:?} by {}:\n port     {ours:?}\n original {theirs:?}",
            e.size,
            e.size_ratio,
            e.life,
            e.life_ratio
        );
        counts[if moving && e.size_ratio > 0.0 { 0 } else { 1 }] += 1;
        counts[if moving && e.life_ratio > 0.0 { 2 } else { 3 }] += 1;
    }
    println!("size by speed, drawn; life by speed, drawn: {counts:?}");
    assert!(counts.iter().all(|&n| n > 2_000), "{counts:?}");
}

/// The disc's wreck dust as the read leaves it, and what its ratios do: a slow emitter's particles
/// are the smallest and shortest-lived the file allows, a fast one's the largest and longest, and
/// between them they grow with the speed — 0.1 ft and 5 ms for every foot a second.
#[test]
fn the_wreck_dust_grows_with_its_speed() {
    let e = Emt::parse(
        "[General Info]\nMinimumLife=1200.0\nMaximumLife=2000.0\nMinimumInitialSize=0.1\nMaximumInitialSize=1.0\n\
         InitialSizeToSpeedRatio=0.1\nLifeToSpeedRatio=5.0\nParticlesToDistanceRatio=1.0\nParticlesToSpeedRatio=10.0\n",
    );
    assert_eq!((e.size_ratio, e.life_ratio), (0.1, 5.0));
    assert_eq!(e.size_and_life(0.5, true, 0.9, 0.9), (0.1, 1200.0));
    assert_eq!(e.size_and_life(500.0, true, 0.1, 0.1), (1.0, 2000.0));
    assert_eq!(e.size_and_life(5.0, true, 0.0, 0.0), (ps2_float::fpu::mul(0.1, 5.0), 1200.0));
    assert_eq!(e.size_and_life(320.0, true, 0.0, 0.0), (1.0, 1600.0));
    // Not moving: the file's random range, as before.
    assert_eq!(e.size_and_life(300.0, false, 0.0, 1.0), (0.1, 2000.0));
    // A file that leaves the keys out reads the speed emitter's defaults, not 0.
    let bare = Emt::parse("[General Info]\nMinimumLife=1000.0\n");
    assert_eq!((bare.size_ratio, bare.life_ratio, bare.distance_ratio, bare.speed_ratio), (0.01, 5.0, 0.01, 0.0));
}
