//! R43 — how MX vs. ATV crosses into and out of a 360's clips, run in SLUS-21104's own code.
//!
//! The 360 step only hands the stunt id to the bike (`+0x1528`) and its rider record's stunt byte. Two
//! animators read it, each on its own fade:
//!
//! - the rider's idle layer (vt `+0xd4`, ported by F62a as `handling::rider::idle`): a new state saves
//!   the pose, puts the fade at 1 and picks its rate; a stunt byte picks the fifth rate (`+0x628`, 2 a
//!   second on state04), a plain state its own (still and air 2, turn 3, wreck poses and pause 0.25);
//!   the fade then falls by rate × dt a tick and is the saved pose's weight;
//! - the bike's animate (vt `+0x3e4`): a new stunt id saves the bike's pose, puts its fade at 1, and the
//!   fade falls by `+0x14c0` (3 a second, only the constructor writes it) × dt a tick.
//!
//! Run here on state04's real bike and rider (`context/mxvatv-ps2/state04.ee.bin`), nothing hooked but
//! the kernel's syscall stubs. Without the image every test skips (and says so). Nothing in the port is
//! proved here: it pins the original's numbers for the MD fix that follows (journal
//! 2026-11-17-r43-*).

use ps2_float::F;
use ps2_float::fpu::Acc;
use ps2_oracle::{Ee, Kit, skip};

/// State 4's bike and its rider, their vtables, and the calls run.
const BIKE: u32 = 0x00e7_14e0;
const BIKE_VTABLE: u32 = 0x003d_0450;
const RIDER: u32 = 0x010a_f350;
const BIKE_ANIMATE: u32 = 0x004d_d8f0;
const RIDER_IDLE: u32 = 0x004e_b380;
/// The bike's rider record (the rider's `+0x378`) and its stunt byte.
const RECORD: u32 = BIKE + 0x730;
const STUNT_BYTE: u32 = RECORD + 0x1d4;
/// The bike's stunt id, the one its animate last saw, its fade and the fade's rate.
const STUNT: u32 = BIKE + 0x1528;
const STUNT_SEEN: u32 = BIKE + 0x1524;
const BIKE_FADE: u32 = BIKE + 0x14bc;
const BIKE_RATE: u32 = BIKE + 0x14c0;
/// The rider's fade, its rate, the five rates, the state wanted and the state running.
const RIDER_FADE: u32 = RIDER + 0x610;
const RIDER_RATE: u32 = RIDER + 0x614;
const RATES: u32 = RIDER + 0x618;
const WANT: u32 = RIDER + 0x86e;
const STATE: u32 = RIDER + 0x86d;
/// The right 360's stunt id.
const SPIN_RIGHT: u32 = 45;
/// The 30 Hz tick the frame runs on (F31: these bits, 0.03333, not 1/30).
const DT: u32 = 0x3d08_850a;

fn image() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2").join(format!("state04.ee{}", ".bin"));
    if !path.exists() {
        skip(&format!("{} is not there: unzip eeMemory.bin from MX vs. ATV's save state 4 to it", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    if ee.u32_at(BIKE + 4) != BIKE_VTABLE || ee.u32_at(BIKE_VTABLE + 0x3e4) != BIKE_ANIMATE || ee.u32_at(RIDER + 0x378) != RECORD {
        panic!("{} is not the MX vs. ATV image these addresses are from", path.display());
    }
    // Every `addiu $3, $zero, n; syscall; jr $ra`: the answer is a0, the thread id (0x2f) 1.
    for a in (0x0030_0000..0x0036_0000u32).step_by(4) {
        if ee.u32_at(a) >> 16 == 0x2403 && ee.u32_at(a + 4) == 0x0000_000c && ee.u32_at(a + 8) == 0x03e0_0008 {
            let n = ee.u32_at(a) & 0xffff;
            ee.hooks.insert(a, Box::new(move |ee: &mut Ee| {
                let v = if n == 0x2f { 1 } else { ee.r(4) };
                ee.set(2, v);
            }));
        }
    }
    Some(ee)
}

fn f(ee: &Ee, at: u32) -> F {
    F::from_bits(ee.u32_at(at))
}

fn set_byte(ee: &mut Ee, at: u32, b: u32) {
    let (a, s) = (at & !3, 8 * (at & 3));
    let w = ee.u32_at(a);
    ee.set_u32(a, (w & !(0xff << s)) | ((b & 0xff) << s));
}

fn rider_tick(ee: &mut Ee) {
    ee.call(RIDER_IDLE, &[u64::from(RIDER)], &[f32::from_bits(DT)], 2_000_000);
}

fn bike_tick(ee: &mut Ee) {
    ee.call(BIKE_ANIMATE, &[u64::from(BIKE)], &[f32::from_bits(DT)], 2_000_000);
}

/// The fade one tick on: `adda(0, fade) msub(rate, dt)`, clamped to 0..1.
fn fall(fade: F, rate: F) -> F {
    Acc::adda(F::ZERO, fade).msub(rate, F::from_bits(DT)).max(F::ZERO).min(F::ONE)
}

/// The rider's five rates on state04: pause 0.25, wreck poses 0.25, plain 2, turn 3, stunt 2.
#[test]
fn the_riders_fade_rates_are_the_images() {
    let Some(ee) = image() else { return };
    let r: Vec<F> = (0..5).map(|k| f(&ee, RATES + 4 * k)).collect();
    assert_eq!(r, [F(0.25), F(0.25), F(2.0), F(3.0), F(2.0)]);
    assert_eq!(ee.u8_at(STUNT_BYTE), 0xff, "state04 rides no stunt");
    assert_eq!(f(&ee, BIKE_RATE), F(3.0));
}

/// **The rider's cross into a 360 and out**: the stunt byte puts the 360 state on, the fade at 1 and
/// the rate at the fifth (2 a second), so the saved pose weighs `1 - 2·dt·n` and is gone in half a
/// second (15 ticks and a 0.0001 tail). Back to none, the state goes back to still at the plain rate, 2: half a second out.
#[test]
fn the_rider_crosses_into_a_360_in_half_a_second() {
    let Some(mut ee) = image() else { return };
    let still = ee.u8_at(STATE);
    set_byte(&mut ee, STUNT_BYTE, SPIN_RIGHT);
    rider_tick(&mut ee);
    assert_eq!((ee.u8_at(WANT), ee.u8_at(STATE)), (SPIN_RIGHT as u8, SPIN_RIGHT as u8));
    let rate = f(&ee, RIDER_RATE);
    assert_eq!(rate, f(&ee, RATES + 16), "a stunt takes the fifth rate");
    let mut fade = fall(F::ONE, rate);
    assert_eq!(f(&ee, RIDER_FADE), fade, "tick 1");
    let mut ticks = 1;
    while fade > F::ZERO {
        rider_tick(&mut ee);
        fade = fall(fade, rate);
        ticks += 1;
        assert_eq!(f(&ee, RIDER_FADE), fade, "tick {ticks}");
        assert!(ticks < 100);
    }
    // dt is 0.03333, not 1/30: 15 ticks leave 0.0001 of the saved pose, the 16th clears it.
    assert_eq!(ticks, 16, "the cross in is half a second at 30 Hz");

    set_byte(&mut ee, STUNT_BYTE, 0xff);
    rider_tick(&mut ee);
    assert_eq!(ee.u8_at(STATE), still, "out of the stunt the rider goes back to still");
    let rate = f(&ee, RIDER_RATE);
    assert_eq!(rate, f(&ee, RATES + 8), "still takes the plain rate");
    let mut fade = fall(F::ONE, rate);
    assert_eq!(f(&ee, RIDER_FADE), fade, "out tick 1");
    let mut ticks = 1;
    while fade > F::ZERO {
        rider_tick(&mut ee);
        fade = fall(fade, rate);
        ticks += 1;
        assert_eq!(f(&ee, RIDER_FADE), fade, "out tick {ticks}");
    }
    assert_eq!(ticks, 16, "the cross out is half a second too");
}

/// **The bike's cross into a 360 and out**: a new stunt id stops the bike's controller (its pose
/// saved) and puts the fade at 1; the fade falls 3 a second from the next tick, so a third of a
/// second (10 ticks and a 0.0001 tail). Back to none, the same again.
#[test]
fn the_bike_crosses_into_a_360_in_a_third_of_a_second() {
    let Some(mut ee) = image() else { return };
    let rate = f(&ee, BIKE_RATE);
    for (id, what) in [(SPIN_RIGHT, "in"), (0xff, "out")] {
        ee.set_u32(STUNT, id);
        bike_tick(&mut ee);
        assert_eq!(ee.u32_at(STUNT_SEEN), id);
        assert_eq!(f(&ee, BIKE_FADE), F::ONE, "{what}: the change tick puts the fade at 1");
        let (mut fade, mut ticks) = (F::ONE, 0);
        while fade > F::ZERO {
            bike_tick(&mut ee);
            fade = fall(fade, rate);
            ticks += 1;
            assert_eq!(f(&ee, BIKE_FADE), fade, "{what} tick {ticks}");
            assert!(ticks < 100);
        }
        // As the rider's: 10 ticks leave 0.0001, the 11th clears it.
        assert_eq!(ticks, 11, "{what}: a third of a second at 30 Hz");
    }
}
