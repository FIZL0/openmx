//! R47 — when MX vs. ATV's 360 stunt ends, run in SLUS-21104's own code.
//!
//! The stunt clock MD248 ran (bike vt `+0x4e8`) zeroes its clock and answers 0 once the clock reaches the
//! row's `Duration`. But its one caller, the trick step (vt `+0x4e4`), calls it only for a bike the
//! network drives (`+0x48c` set). A local bike's trick step runs the clock itself:
//!
//! - clock `+= dt`, phase `min(clock / Duration, 1)` stored, **1 included**, and the clock never zeroed;
//! - a 360 while its torque is applied (`+0x11d0`, or the backflip's `+0x1194`) is a "kept" stunt: the
//!   clock never ends it. Only the 360 step ends it (the stunt id to 255 on a landing, a touch or the
//!   yaw under its floor; `atv_moto::spin_air`, proved in `tests/atv_quad.rs`);
//! - a kept stunt without the torque ends once the clock reaches `Duration` (the trick end, answer 0),
//!   unless its row's kind (0 for the 360s) is the asked one, which holds it at `+0x44` as below;
//! - a turning 360 with R1 alone held (the asked kind 4, the kept stunts' kind), the clock stops at the row's `+0x44` (0.72 s) and the clip
//!   is held there (vt `+0x4ec`).
//!
//! State04's bike, nothing hooked but the kernel's syscall stubs (as `mxvatv_stunt_cross.rs`). Without the
//! image every test skips. Journal 2026-11-18-r47-*.

use ps2_float::F;
use ps2_oracle::{Ee, Kit, skip};

const BIKE: u32 = 0x00e7_14e0;
const BIKE_VTABLE: u32 = 0x003d_0450;
const TRICK_STEP: u32 = 0x004e_49c0;
/// Remote (network-driven) bike, on the ground, the asked kind's byte, the backflip's and the 360's
/// applied words, the held word.
const REMOTE: u32 = BIKE + 0x48c;
const GROUND: u32 = BIKE + 0x324;
const ASKED: u32 = BIKE + 0x14ec;
const FLIP_APPLIED: u32 = BIKE + 0x1194;
const SPIN_APPLIED: u32 = BIKE + 0x11d0;
const HELD: u32 = BIKE + 0x1788;
/// The stunt id, its clock and the clock's phase (the rider's clip phase, record `+0x1d8`).
const STUNT: u32 = BIKE + 0x1528;
const CLOCK: u32 = BIKE + 0x14d8;
const PHASE: u32 = BIKE + 0x14e4;
/// The right 360's stunt id; its row's `Duration` and `+0x44` on state04.
const SPIN_RIGHT: u32 = 45;
const DURATION: F = F(1.5);
const HOLD_AT: F = F(0.72);
const DT: u32 = 0x3d08_850a;

fn image() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2").join(format!("state04.ee{}", ".bin"));
    if !path.exists() {
        skip(&format!("{} is not there: unzip eeMemory.bin from MX vs. ATV's save state 4 to it", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    if ee.u32_at(BIKE + 4) != BIKE_VTABLE || ee.u32_at(BIKE_VTABLE + 0x4e4) != TRICK_STEP {
        panic!("{} is not the MX vs. ATV image these addresses are from", path.display());
    }
    for a in (0x0030_0000..0x0036_0000u32).step_by(4) {
        if ee.u32_at(a) >> 16 == 0x2403 && ee.u32_at(a + 4) == 0x0000_000c && ee.u32_at(a + 8) == 0x03e0_0008 {
            let n = ee.u32_at(a) & 0xffff;
            ee.hooks.insert(a, Box::new(move |ee: &mut Ee| {
                let v = if n == 0x2f { 1 } else { ee.r(4) };
                ee.set(2, v);
            }));
        }
    }
    // A right 360 just started in the air: the id set, its clocks at 0, no trick button.
    ee.set_u32(GROUND, 0);
    ee.set_u32(STUNT, SPIN_RIGHT);
    ee.set_u32(CLOCK, 0);
    ee.set_u32(PHASE, 0);
    ee.set_u32(FLIP_APPLIED, 0);
    let asked = ee.u32_at(ASKED);
    ee.set_u32(ASKED, asked & !0xff);
    Some(ee)
}

fn f(ee: &Ee, at: u32) -> F {
    F::from_bits(ee.u32_at(at))
}

/// One trick step: what it answers.
fn step(ee: &mut Ee) -> u32 {
    ee.call(TRICK_STEP, &[u64::from(BIKE)], &[f32::from_bits(DT)], 4_000_000);
    ee.r(2) as u32
}

/// **A turning 360 never ends on its clock**: three seconds of trick steps with the 360's torque
/// applied, every one answers 1 with the stunt still the 360; the clock runs on past the row's 1.5 s
/// and is never zeroed, and the phase reaches 1 on tick 46 and stays there.
#[test]
fn a_turning_360_never_ends_on_its_clock() {
    let Some(mut ee) = image() else { return };
    ee.set_u32(SPIN_APPLIED, 1);
    let mut clock = F::ZERO;
    for k in 1..=90 {
        assert_eq!(step(&mut ee), 1, "tick {k}: the trick runs");
        clock = clock + F::from_bits(DT);
        assert_eq!(ee.u32_at(STUNT), SPIN_RIGHT, "tick {k}: still the 360");
        assert_eq!(f(&ee, CLOCK), clock, "tick {k}: the clock runs on");
        assert_eq!(f(&ee, PHASE), (clock / DURATION).min(F::ONE), "tick {k}: the phase");
        assert_eq!(ee.u32_at(HELD), 0);
        if k >= 46 {
            assert_eq!(f(&ee, PHASE), F::ONE, "tick {k}: the phase holds at 1 (1 included)");
        }
    }
}

/// **A 360 without its torque ends at the row's Duration**: the same steps with the torque off (and R1
/// held, so the row's kind 0 is not the asked one and the clip is not held) answer 1 for 45 ticks and
/// 0 on the 46th, the clock past 1.5 s and not zeroed. The trick step leaves the id; the lean then sets
/// it (riding pose in the air, none on the ground), as `handling::update::lean`.
#[test]
fn a_360_without_its_torque_ends_at_its_duration() {
    let Some(mut ee) = image() else { return };
    ee.set_u32(SPIN_APPLIED, 0);
    let asked = ee.u32_at(ASKED);
    ee.set_u32(ASKED, (asked & !0xff) | 4);
    let mut clock = F::ZERO;
    for k in 1..=46 {
        let runs = step(&mut ee);
        clock = clock + F::from_bits(DT);
        assert_eq!(runs, u32::from(k < 46), "tick {k}");
        assert_eq!(f(&ee, CLOCK), clock, "tick {k}: the clock is never zeroed");
        assert_eq!(ee.u32_at(STUNT), SPIN_RIGHT, "tick {k}: the step writes no id");
    }
    assert!(f(&ee, CLOCK) >= DURATION && f(&ee, PHASE) == F::ONE);
}

/// **Only a remote bike's clock zeroes**: with `+0x48c` set the step answers through vt `+0x4e8`, which
/// on tick 46 zeroes the clock and answers 0 — what MD248's test ran — even with the 360's torque
/// applied. The phase is stored before the test (the store sits in the branch's delay slot), so it is 1.
#[test]
fn only_a_remote_bikes_stunt_clock_zeroes() {
    let Some(mut ee) = image() else { return };
    ee.set_u32(REMOTE, 1);
    ee.set_u32(SPIN_APPLIED, 1);
    let mut clock = F::ZERO;
    for k in 1..=46 {
        let runs = step(&mut ee);
        clock = clock + F::from_bits(DT);
        assert_eq!(runs, u32::from(k < 46), "tick {k}");
        if k < 46 {
            assert_eq!(f(&ee, CLOCK), clock, "tick {k}");
        }
    }
    assert_eq!((f(&ee, CLOCK), f(&ee, PHASE)), (F::ZERO, F::ONE));
}

/// **R1 alone held stops a turning 360's clock at 0.72 s**: with the asked kind 4 (the kept stunt's),
/// the step runs the clock up to the row's `+0x44`, then holds the clip there (vt `+0x4ec`, the held
/// word set) and the clock stays put for as long as it is held.
#[test]
fn r1_held_stops_a_360s_clock_at_its_hold() {
    let Some(mut ee) = image() else { return };
    ee.set_u32(SPIN_APPLIED, 1);
    let asked = ee.u32_at(ASKED);
    ee.set_u32(ASKED, (asked & !0xff) | 4);
    let mut clock = F::ZERO;
    for k in 1..=90 {
        assert_eq!(step(&mut ee), 1, "tick {k}");
        assert_eq!(ee.u32_at(STUNT), SPIN_RIGHT, "tick {k}");
        if clock < HOLD_AT {
            clock = clock + F::from_bits(DT);
            assert_eq!(ee.u32_at(HELD), 0, "tick {k}: not held yet");
        } else {
            assert_eq!(ee.u32_at(HELD), 1, "tick {k}: held");
        }
        assert_eq!(f(&ee, CLOCK), clock, "tick {k}");
    }
    assert!(clock >= HOLD_AT && clock < HOLD_AT + F(0.04));
}
