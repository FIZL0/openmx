// ---------------------------------------------------------------------------------------------------
// S12b: the ground controller, against the original's own, on the recording2 free09 image.
// ---------------------------------------------------------------------------------------------------

use super::{Pick, named};
use game_sim::ai::{self, CURVE_INV_STEP, CURVE_STEP, Controls, Situation, Tuning, flag};
use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::ragdoll::Tables;
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, Kit, addr, skip};

/// The free09 save-state's RAM: three AI riders at skill 4, every one of them on the ground
/// controller (their interface is type 1).
const STATE: &str = "recording2.p2m2_SaveState.eeMemory.bin";
const RIDERS: [u32; 3] = [0x0148_3eb0, 0x00c6_4000, 0x0104_8000];
const BUDGET: u64 = 200_000;

fn state() -> Option<Ee> {
    let kit = Kit::load().map_err(|why| skip(&why)).ok()?;
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: `./kit state ram` of recording2.p2m2_SaveState.p2s", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    Some(ee)
}

/// The game's curve table out of the image, and the port's fine arc cosine (proved the game's in
/// `tests/ragdoll.rs`); and the step beside the table is the port's.
pub(crate) fn tables(ee: &Ee) -> Tables {
    assert_eq!(ee.f32_at(addr("curve_table_step").unwrap()).to_bits(), CURVE_STEP.to_bits());
    assert_eq!(ee.f32_at(addr("curve_table_invstep").unwrap()).to_bits(), CURVE_INV_STEP.to_bits());
    let base = ee.u32_at(addr("curve_table_ptr").unwrap());
    let curve: Vec<[f32; 2]> =
        (0..CURVE_ENTRIES as u32).map(|k| [ee.f32_at(base + 8 * k), ee.f32_at(base + 8 * k + 4)]).collect();
    Tables { curve: CurveTable::from_entries(&curve), ..Tables::build() }
}

fn quad(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|o| ee.f32_at(a + o)))
}

fn set_quad(ee: &mut Ee, a: u32, v: Vf) {
    for (k, x) in v.0.iter().enumerate() {
        ee.set_f32(a + 4 * k as u32, *x);
    }
}

fn f(ee: &Ee, a: u32) -> F {
    F(ee.f32_at(a))
}

/// What the controller reads, off a rider in RAM.
pub(crate) fn read(ee: &Ee, ai: u32) -> (Situation, Tuning) {
    let at = Situation {
        up: quad(ee, ai + 0xa0),
        forward: quad(ee, ai + 0xb0),
        height: f(ee, ai + 0xc4),
        ground: f(ee, ai + 0x204),
        ground_normal: quad(ee, ai + 0x2c0),
        spin: quad(ee, ai + 0x70),
        speed: f(ee, ai + 0x80),
        flags: u64::from(ee.u32_at(ai + 0x30)) | u64::from(ee.u32_at(ai + 0x34)) << 32,
        target_dir: quad(ee, ai + 0x1d0),
        target_speed: f(ee, ai + 0x1e0),
        min_speed: f(ee, ai + 0x254),
        pitch: f(ee, ai + 0xf4),
    };
    let tune = Tuning {
        steering_gain: f(ee, ai + 0x208),
        oversteer_gain: f(ee, ai + 0x21c),
        aim_gain: f(ee, ai + 0x2d0),
        pitch_distance: f(ee, ai + 0x2b4),
        pitch_gain: f(ee, ai + 0x2d4),
        pitch_filter: f(ee, ai + 0x2d8),
        landing_pitch: f(ee, ai + 0x2ec),
        slow_tolerance: f(ee, ai + 0x260),
        fast_tolerance: f(ee, ai + 0x264),
        cruise_base: f(ee, ai + 0x32c),
        cruise_gain: f(ee, ai + 0x330),
        brake_gain: f(ee, ai + 0x334),
        steer_scale: f(ee, ai + 0x100),
        pitch_scale: f(ee, ai + 0x104),
    };
    (at, tune)
}

fn write(ee: &mut Ee, ai: u32, at: &Situation, tune: &Tuning) {
    set_quad(ee, ai + 0xa0, at.up);
    set_quad(ee, ai + 0xb0, at.forward);
    ee.set_f32(ai + 0xc4, at.height.0);
    ee.set_f32(ai + 0x204, at.ground.0);
    set_quad(ee, ai + 0x2c0, at.ground_normal);
    set_quad(ee, ai + 0x70, at.spin);
    ee.set_f32(ai + 0x80, at.speed.0);
    ee.set_u32(ai + 0x30, at.flags as u32);
    ee.set_u32(ai + 0x34, (at.flags >> 32) as u32);
    set_quad(ee, ai + 0x1d0, at.target_dir);
    ee.set_f32(ai + 0x1e0, at.target_speed.0);
    ee.set_f32(ai + 0x254, at.min_speed.0);
    ee.set_f32(ai + 0xf4, at.pitch.0);
    for (o, v) in [
        (0x208, tune.steering_gain),
        (0x21c, tune.oversteer_gain),
        (0x2d0, tune.aim_gain),
        (0x2b4, tune.pitch_distance),
        (0x2d4, tune.pitch_gain),
        (0x2d8, tune.pitch_filter),
        (0x2ec, tune.landing_pitch),
        (0x260, tune.slow_tolerance),
        (0x264, tune.fast_tolerance),
        (0x32c, tune.cruise_base),
        (0x330, tune.cruise_gain),
        (0x334, tune.brake_gain),
        (0x100, tune.steer_scale),
        (0x104, tune.pitch_scale),
    ] {
        ee.set_f32(ai + o, v.0);
    }
}

/// What the controller left in RAM, as the port's [`Controls`].
fn left(ee: &Ee, ai: u32) -> Controls {
    Controls {
        steer: f(ee, ai + 0xf0),
        pitch: f(ee, ai + 0xf4),
        throttle: f(ee, ai + 0xe0),
        brake: f(ee, ai + 0xe4),
        heading: f(ee, ai + 0x1f4),
        aim: f(ee, ai + 0x1f0),
        roll: f(ee, ai + 0x1f8),
        flat_forward: quad(ee, ai + 0x270),
        target_speed: f(ee, ai + 0x1e0),
    }
}

fn bits(c: &Controls) -> [u32; 12] {
    let w = c.flat_forward.to_bits();
    [
        c.steer.0.to_bits(),
        c.pitch.0.to_bits(),
        c.throttle.0.to_bits(),
        c.brake.0.to_bits(),
        c.heading.0.to_bits(),
        c.aim.0.to_bits(),
        c.roll.0.to_bits(),
        w[0],
        w[1],
        w[2],
        w[3],
        c.target_speed.0.to_bits(),
    ]
}

const NAMES: [&str; 12] =
    ["steer", "pitch", "throttle", "brake", "heading", "aim", "roll", "flat.x", "flat.y", "flat.z", "flat.w", "target"];

fn differs(ours: &Controls, theirs: &Controls) -> Vec<String> {
    bits(ours)
        .iter()
        .zip(bits(theirs))
        .zip(NAMES)
        .filter(|((o, t), _)| **o != *t)
        .map(|((o, t), n)| {
            format!("{n}: ours {} ({o:#010x}) theirs {} ({t:#010x})", f32::from_bits(*o), f32::from_bits(t))
        })
        .collect()
}

impl Pick {
    /// Mostly a float in `lo..hi`; now and then one of the edges the controller's tests sit on.
    pub(crate) fn float(&mut self, lo: f32, hi: f32) -> F {
        const EDGES: [f32; 9] = [0.0, -0.0, 1.0, -1.0, 1.5707964, -1.5707964, 3.1415927, -3.1415927, 0.5];
        if self.below(16) == 0 {
            return F(EDGES[self.below(EDGES.len() as u32) as usize]);
        }
        let u = (self.next() >> 40) as f32 / (1u64 << 24) as f32;
        F(lo + (hi - lo) * u)
    }
    fn unit(&mut self) -> Vf {
        let v = [self.float(-1.0, 1.0).0, self.float(-1.0, 1.0).0, self.float(-1.0, 1.0).0];
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if n < 1e-3 || self.below(8) == 0 {
            return Vf([v[0], v[1], v[2], self.float(-2.0, 2.0).0]);
        }
        Vf([v[0] / n, v[1] / n, v[2] / n, self.float(-2.0, 2.0).0])
    }
    fn flat(&mut self) -> Vf {
        let a = self.float(-7.0, 7.0).0;
        Vf([a.cos(), 0.0, a.sin(), 0.0])
    }
}

fn case(p: &mut Pick) -> (Situation, Tuning) {
    let forward = p.unit();
    let target_dir = match p.below(4) {
        0 => p.unit(),
        1 => Vf([forward.0[0], 0.0, forward.0[2], 0.0]),
        _ => p.flat(),
    };
    let flags = [flag::ON_GROUND, flag::OVERSTEER_CORRECTION, flag::CRUISE, flag::GRADED_BRAKE, 0x80, 0x0100_0000_0000]
        .iter()
        .filter(|_| p.below(2) == 0)
        .fold(0x03e0_b80b, |a, b| a | b)
        & !if p.below(2) == 0 { flag::ON_GROUND } else { 0 };
    let speed = p.float(0.0, 80.0);
    let at = Situation {
        up: p.unit(),
        forward,
        height: p.float(-20.0, 60.0),
        ground: p.float(-20.0, 40.0),
        ground_normal: p.unit(),
        spin: Vf([p.float(-6.0, 6.0).0, p.float(-6.0, 6.0).0, p.float(-6.0, 6.0).0, 0.0]),
        speed,
        flags,
        target_dir,
        target_speed: if p.below(4) == 0 { speed + p.float(-3.0, 3.0) } else { p.float(-10.0, 150.0) },
        min_speed: p.float(0.0, 40.0),
        pitch: p.float(-1.5, 1.5),
    };
    let tune = Tuning {
        steering_gain: p.float(0.0, 6.0),
        oversteer_gain: p.float(-1.0, 1.0),
        aim_gain: p.float(-3.0, 3.0),
        pitch_distance: p.float(0.0, 30.0),
        pitch_gain: p.float(-4.0, 4.0),
        pitch_filter: p.float(0.0, 1.0),
        landing_pitch: if p.below(8) == 0 { p.float(-3.0e5, 3.0e5) } else { p.float(-20.0, 20.0) },
        slow_tolerance: p.float(0.0, 10.0),
        fast_tolerance: p.float(0.0, 10.0),
        cruise_base: p.float(0.0, 0.05),
        cruise_gain: p.float(0.0, 2.0),
        brake_gain: p.float(0.0, 0.25),
        steer_scale: p.float(-1.5, 1.5),
        pitch_scale: p.float(-1.5, 1.5),
    };
    (at, tune)
}

/// Ten thousand rider situations through the original's controller and the port, on the image's
/// first AI rider: every output and every working value it leaves, bit for bit.
#[test]
fn the_controller_matches_over_ten_thousand_ticks() {
    let Some(at) = named(&["ai_controls"]) else { return };
    let Some(mut ee) = state() else { return };
    let tables = tables(&ee);
    let ai = RIDERS[0];
    let mut p = Pick(0x5eed_12b0_c0ff_ee01);
    let mut bad = 0;
    let mut seen = [0usize; 6];
    for n in 0..10_000 {
        let (sit, tune) = case(&mut p);
        write(&mut ee, ai, &sit, &tune);
        ee.call(at[0], &[u64::from(ai)], &[], BUDGET);
        assert_eq!(ee.r(2), 1, "case {n}: the controller returns 1");
        let theirs = left(&ee, ai);
        let ours = ai::controls(&tables, &sit, &tune);
        let d = differs(&ours, &theirs);
        if !d.is_empty() {
            if bad < 10 {
                println!("case {n}: {sit:?} {tune:?}\n  {}", d.join("\n  "));
            }
            bad += 1;
        }
        let h = sit.height - sit.ground;
        seen[0] += usize::from(sit.flags & flag::ON_GROUND == 0 && !(h <= tune.pitch_distance));
        seen[1] += usize::from(sit.flags & flag::ON_GROUND == 0 && h <= tune.pitch_distance);
        seen[2] += usize::from(theirs.throttle.0 == 1.0);
        seen[3] += usize::from(theirs.brake.0 > 0.0 && theirs.brake.0 < 1.0);
        seen[4] += usize::from(theirs.throttle.0 > 0.0 && theirs.throttle.0 < 1.0);
        seen[5] += usize::from(theirs.steer.0.abs() == 1.0);
    }
    println!("air {} landing {} open {} graded brake {} cruise {} full lock {}", seen[0], seen[1], seen[2], seen[3], seen[4], seen[5]);
    assert_eq!(bad, 0, "{bad} of 10,000 ticks differ");
    assert!(seen.iter().all(|&s| s > 100), "a branch went untried: {seen:?}");
}

/// The three AI riders as the image holds them: the port on what each one's controller read
/// gives what that controller left, and the original run again on the image agrees.
#[test]
fn the_free09_riders_controls_are_the_originals() {
    let Some(at) = named(&["ai_controls"]) else { return };
    let Some(mut ee) = state() else { return };
    let tables = tables(&ee);
    for ai in RIDERS {
        let (sit, tune) = read(&ee, ai);
        let held = left(&ee, ai);
        let ours = ai::controls(&tables, &sit, &tune);
        println!("rider {ai:#x}: held {held:?}\n  ours {ours:?}");
        let d = differs(&ours, &held);
        assert!(d.is_empty(), "rider {ai:#x} against the image:\n  {}", d.join("\n  "));
        ee.call(at[0], &[u64::from(ai)], &[], BUDGET);
        let d = differs(&ours, &left(&ee, ai));
        assert!(d.is_empty(), "rider {ai:#x} against the original rerun:\n  {}", d.join("\n  "));
    }
}
