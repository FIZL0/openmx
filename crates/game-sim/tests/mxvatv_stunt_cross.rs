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

// ---------------------------------------------------------------------------------------------------
// MD248: the port's 360 cross against the original's, tick by tick
// ---------------------------------------------------------------------------------------------------

use game_sim::motion::ClipId;
use game_sim::vehicle_anim::{self as va, Blend, Driver, Library, Params, Request};

/// The stunt clock's bike-side step (`+0x14d8 += dt`, then `min(clock / row Duration, 1)` into
/// `+0x14e4`), and those two fields.
const STUNT_CLOCK: u32 = 0x004e_4900;
const CLOCK: u32 = BIKE + 0x14d8;
const CLOCK_PHASE: u32 = BIKE + 0x14e4;
/// The 360 clips' length on state04, as R43 read it.
const SPIN_CLIP: F = F(1.4666667);

/// A table with the airborne row and the right 360 bound, a rider's or a bike's.
fn spin_lib(bike: bool) -> Library {
    let mut lib = Library { bike, ..Library::default() };
    for s in va::AIR..va::AIR + 3 {
        lib.clip[s as usize] = Some(ClipId(s as u32));
    }
    lib.pack[(va::SPIN_RIGHT - va::PACK_EXTRA) as usize] = Some(ClipId(99));
    lib
}

/// The body's measured rates (game_render::vehicle::params), which the 360 must not use.
fn body_params() -> Params {
    Params {
        rate_stunt: F(4.0),
        rate_default: F(4.0),
        rate_pose: F(3.25),
        rate_hold: F(0.25),
        rate_ready: F(0.25),
        cross_cap: F(1.0),
        start_phase: F::ZERO,
        band: [F(7.3333), F(14.6666), F(21.9999)],
    }
}

fn airborne() -> Driver {
    Driver { want: va::AIR, state: va::AIR, prev: va::AIR, extra: -1, ..Driver::default() }
}

/// One port frame asking for `want`: the ramp it leaves, and the 360 clip's phase when it plays one.
fn port_tick(d: &mut Driver, lib: &Library, want: i32) -> (F, Option<F>) {
    d.want = want;
    let frame = d.frame(F::from_bits(DT), &body_params(), &Request::default(), lib, true, |_| SPIN_CLIP);
    let phase = match frame.blend {
        Blend::One { phase, ramp, clip: Some(ClipId(99)) } => {
            assert_eq!(ramp, d.ramp, "the 360 is drawn at the ramp");
            Some(phase)
        }
        _ => None,
    };
    (d.ramp, phase)
}

/// **The port's rider crosses into a 360 and out as the original's does**: every tick's saved-pose
/// weight bit for bit against SLUS-21104's rider layer, 16 ticks in at 2 a second and 16 out into the
/// airborne row (the original's still: both take the plain 2), not the body's 3.25. And the clip's
/// phase each tick against the original's own stunt clock over the 360 row's Duration.
#[test]
fn the_ports_rider_crosses_a_360_as_the_original() {
    let Some(mut ee) = image() else { return };
    let lib = spin_lib(false);
    let mut d = airborne();
    set_byte(&mut ee, STUNT_BYTE, SPIN_RIGHT);
    ee.set_u32(STUNT, SPIN_RIGHT);
    ee.set_u32(CLOCK, 0);
    // 45 ticks are 1.49985 s; on the 46th the clock's phase would reach 1, and the original's stunt
    // clock ends the stunt instead (the clock zeroed, the phase left at the 45th's).
    for k in 1..=46 {
        rider_tick(&mut ee);
        ee.call(STUNT_CLOCK, &[u64::from(BIKE)], &[f32::from_bits(DT)], 2_000_000);
        let (ramp, phase) = port_tick(&mut d, &lib, va::SPIN_RIGHT);
        assert_eq!(ramp, f(&ee, RIDER_FADE), "in, tick {k}");
        if k <= 45 {
            assert_eq!(phase, Some(f(&ee, CLOCK_PHASE)), "in, tick {k}: the clip's phase (clock {:?})", f(&ee, CLOCK));
        } else {
            assert_eq!((f(&ee, CLOCK), phase), (F::ZERO, Some(F::ONE)), "tick 46: the original's stunt ends, the port's clip holds its end");
        }
    }
    set_byte(&mut ee, STUNT_BYTE, 0xff);
    for k in 1..=20 {
        rider_tick(&mut ee);
        let (ramp, phase) = port_tick(&mut d, &lib, va::AIR);
        assert_eq!(phase, None);
        assert_eq!(ramp, f(&ee, RIDER_FADE), "out, tick {k}");
    }
    assert_eq!(d.ramp, F::ZERO);
}

/// **The port's bike crosses into a 360 and out as the original's does**: its own 3 a second, the
/// fade held at 1 on the change tick, every tick bit for bit against SLUS-21104's bike animate; the
/// clip at the stunt clock over its own length.
#[test]
fn the_ports_bike_crosses_a_360_as_the_original() {
    let Some(mut ee) = image() else { return };
    let lib = spin_lib(true);
    let mut d = airborne();
    let mut clock = F::ZERO;
    for (id, want, what) in [(SPIN_RIGHT, va::SPIN_RIGHT, "in"), (0xff, va::AIR, "out")] {
        ee.set_u32(STUNT, id);
        for k in 1..=20 {
            bike_tick(&mut ee);
            let (ramp, phase) = port_tick(&mut d, &lib, want);
            assert_eq!(ramp, f(&ee, BIKE_FADE), "{what}, tick {k}");
            if id == SPIN_RIGHT {
                clock = clock + F::from_bits(DT);
                assert_eq!(phase, Some((clock / SPIN_CLIP).min(F::ONE)), "{what}, tick {k}: clock over the clip");
            }
        }
        assert_eq!(d.ramp, F::ZERO, "{what}: a third of a second");
    }
}

/// Without a 360 nothing moves: a pack extra still takes the body's gesture rate, and the airborne
/// row out of anything but a 360 its own.
#[test]
fn only_the_360_takes_the_originals_fade() {
    let mut lib = spin_lib(false);
    lib.pack[0] = Some(ClipId(98));
    let mut d = airborne();
    port_tick(&mut d, &lib, va::PACK_EXTRA);
    assert_eq!(d.rate, F(4.0));
    port_tick(&mut d, &lib, va::AIR);
    assert_eq!(d.rate, F(3.25));
    port_tick(&mut d, &lib, va::SPIN_RIGHT);
    assert_eq!(d.rate, F(2.0));
    port_tick(&mut d, &lib, va::SPIN_RIGHT);
    port_tick(&mut d, &lib, va::HOLD);
    assert_eq!(d.rate, F(0.25), "a wreck pose out of a 360 keeps its own");
}
