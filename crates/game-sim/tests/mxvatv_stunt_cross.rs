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

/// The bike animate's one-clip play (`f12` the clamped phase), replaced by a recorder:
/// each call leaves its phase in the cell.
const BIKE_PLAY_ONE: u32 = 0x0013_eec0;

fn bike_play(ee: &mut Ee) -> std::rc::Rc<std::cell::Cell<Option<F>>> {
    let played = std::rc::Rc::new(std::cell::Cell::new(None));
    let cell = played.clone();
    ee.hooks.insert(BIKE_PLAY_ONE, Box::new(move |ee: &mut Ee| cell.set(Some(F(ee.f(12))))));
    played
}

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
    port_tick_at(d, lib, want, 0)
}

/// [`port_tick`] with the 360's clip-start count (S467) in the request.
fn port_tick_at(d: &mut Driver, lib: &Library, want: i32, spin_starts: u8) -> (F, Option<F>) {
    port_tick_held(d, lib, want, spin_starts, false)
}

/// [`port_tick_at`] with R1 alone held or not (S478).
fn port_tick_held(d: &mut Driver, lib: &Library, want: i32, spin_starts: u8, spin_hold: bool) -> (F, Option<F>) {
    d.want = want;
    let r = Request { spin_starts, spin_hold, ..Request::default() };
    let frame = d.frame(F::from_bits(DT), &body_params(), &r, lib, true, |_| SPIN_CLIP);
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
    // 45 ticks are 1.49985 s; on the 46th the phase reaches 1. The stunt clock run here zeroes its
    // clock there and ends the stunt, but it is the remote (network-driven) bike's only: a local
    // bike's trick step runs its own clock, never zeroed, and keeps a turning 360 at phase 1 (R47,
    // `tests/mxvatv_stunt_end.rs`). Either way the phase stored on the 46th is 1.
    for k in 1..=46 {
        rider_tick(&mut ee);
        ee.call(STUNT_CLOCK, &[u64::from(BIKE)], &[f32::from_bits(DT)], 2_000_000);
        let (ramp, phase) = port_tick(&mut d, &lib, va::SPIN_RIGHT);
        assert_eq!(ramp, f(&ee, RIDER_FADE), "in, tick {k}");
        assert_eq!(phase, Some(f(&ee, CLOCK_PHASE)), "in, tick {k}: the clip's phase (clock {:?})", f(&ee, CLOCK));
        if k == 46 {
            assert_eq!((f(&ee, CLOCK), phase), (F::ZERO, Some(F::ONE)), "tick 46: the remote clock zeroes, the clip holds its end");
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
    let played = bike_play(&mut ee);
    let lib = spin_lib(true);
    let mut d = airborne();
    for (id, want, what) in [(SPIN_RIGHT, va::SPIN_RIGHT, "in"), (0xff, va::AIR, "out")] {
        ee.set_u32(STUNT, id);
        for k in 1..=20 {
            ee.call(STUNT_CLOCK, &[u64::from(BIKE)], &[f32::from_bits(DT)], 2_000_000);
            played.set(None);
            bike_tick(&mut ee);
            let (ramp, phase) = port_tick(&mut d, &lib, want);
            assert_eq!(ramp, f(&ee, BIKE_FADE), "{what}, tick {k}");
            if id == SPIN_RIGHT {
                // S478: the original's own phase, 0 on the change tick, then the clock over the clip.
                assert_eq!(phase, played.get(), "{what}, tick {k}: the clip's phase");
                if k == 1 {
                    assert_eq!(phase, Some(F::ZERO), "the change tick plays the clip at 0");
                }
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

// ---------------------------------------------------------------------------------------------------
// S467: a 360's second clip start restarts it
// ---------------------------------------------------------------------------------------------------

/// The local bike's trick step (R47), its 360-applied word and the stunt's restart ask, which the
/// bike's record copy carries to the rider (`+0x1650` into record `+0x1d0`, beside the stunt byte).
const TRICK_STEP: u32 = 0x004e_49c0;
const SPIN_APPLIED: u32 = BIKE + 0x11d0;
const RESTART: u32 = BIKE + 0x1650;
const RECORD_RESTART: u32 = RECORD + 0x1d0;

/// The original's state through a 360's restart: the stunt id on, the torque applied, the clock at 0
/// — what the stunt start leaves, the restart ask set on the restart tick only.
fn restart_image() -> Option<Ee> {
    let mut ee = image()?;
    ee.set_u32(BIKE + 0x324, 0);
    ee.set_u32(BIKE + 0x1194, 0);
    let asked = ee.u32_at(BIKE + 0x14ec);
    ee.set_u32(BIKE + 0x14ec, asked & !0xff);
    ee.set_u32(SPIN_APPLIED, 1);
    set_byte(&mut ee, STUNT_BYTE, SPIN_RIGHT);
    ee.set_u32(STUNT, SPIN_RIGHT);
    ee.set_u32(CLOCK, 0);
    Some(ee)
}

/// One of the original's frames on the 360: the trick step's clock (the stunt start zeroes it and
/// sets the restart ask when `start`), then the rider's layer and the bike's animate.
fn original_tick(ee: &mut Ee, start: bool) {
    if start {
        ee.set_u32(CLOCK, 0);
    }
    let ask = u32::from(start);
    ee.set_u32(RESTART, ask);
    ee.set_u32(RECORD_RESTART, ask);
    ee.call(TRICK_STEP, &[u64::from(BIKE)], &[f32::from_bits(DT)], 4_000_000);
    rider_tick(ee);
    bike_tick(ee);
}

/// **The port's rider restarts a 360 on its second clip start as the original's does**: 60 ticks in
/// the 360 (past coming round, the stunt still on and its phase held at 1 from tick 46), then the
/// second pass past the clip angle: the original's restart ask puts the rider's fade back at 1 at the
/// stunt rate and the clock back to 0, and the port's moved start count does the same — every tick's
/// saved-pose weight and clip phase bit for bit, before and after.
#[test]
fn the_ports_rider_restarts_a_360_as_the_original() {
    let Some(mut ee) = restart_image() else { return };
    let lib = spin_lib(false);
    let mut d = airborne();
    for (starts, ticks) in [(1u8, 60), (2, 30)] {
        for k in 1..=ticks {
            original_tick(&mut ee, k == 1 && starts == 2);
            assert_eq!(ee.u32_at(STUNT), SPIN_RIGHT, "start {starts}, tick {k}: the stunt runs");
            let (ramp, phase) = port_tick_at(&mut d, &lib, va::SPIN_RIGHT, starts);
            assert_eq!(ramp, f(&ee, RIDER_FADE), "start {starts}, tick {k}: the fade");
            assert_eq!(phase, Some(f(&ee, CLOCK_PHASE)), "start {starts}, tick {k}: the phase");
            if k == 1 {
                assert_eq!(ramp, fall(F::ONE, F(2.0)), "start {starts}: a restart, at the stunt rate");
            }
        }
    }
}

/// **The port's bike restarts its 360 as the original's does**: the restart ask holds the bike's fade
/// at 1 on its tick and it falls 3 a second from the next, as on the 360's first start; the port's
/// moved start count does the same, and its clip runs from 0 again.
#[test]
fn the_ports_bike_restarts_a_360_as_the_original() {
    let Some(mut ee) = restart_image() else { return };
    let played = bike_play(&mut ee);
    let lib = spin_lib(true);
    let mut d = airborne();
    for (starts, ticks) in [(1u8, 60), (2, 20)] {
        for k in 1..=ticks {
            played.set(None);
            original_tick(&mut ee, k == 1 && starts == 2);
            let (ramp, phase) = port_tick_at(&mut d, &lib, va::SPIN_RIGHT, starts);
            assert_eq!(ramp, f(&ee, BIKE_FADE), "start {starts}, tick {k}: the fade");
            // S478: the original's played phase: 0 on each (re)start tick, then the clock over the clip.
            assert_eq!(phase, played.get(), "start {starts}, tick {k}: the clip's phase");
            if k == 1 {
                assert_eq!(phase, Some(F::ZERO), "start {starts}: the restart tick plays the clip at 0");
            }
        }
    }
}

/// **Coming round does not end the port's 360** (S467): a 360 that has started its clip shows from
/// then until it ends, through the clip coming round; a second start moves the count.
#[test]
fn a_360_shows_from_its_clip_start_to_its_end() {
    use game_sim::air::{Spin, spin_clip};
    let mut s = Spin { active: true, armed: true, ..Spin::default() };
    assert!(!s.shown(), "not yet past the clip angle");
    spin_clip(&mut s, F(1.0));
    assert_eq!((s.clip, s.starts, s.shown()), (1, 1, true));
    spin_clip(&mut s, F(1.0));
    spin_clip(&mut s, F(0.1));
    assert!(s.came_round);
    assert_eq!((s.clip, s.shown()), (0, true), "come round, still the 360");
    spin_clip(&mut s, F(1.0));
    assert_eq!((s.clip, s.starts), (1, 2), "the second pass starts it again");
    s.active = false;
    s.clip = 0;
    assert!(!s.shown(), "the 360 over");
}

// ---------------------------------------------------------------------------------------------------
// S478: R1 alone held stops a turning 360's clock at the row's +0x44 and holds the clip
// ---------------------------------------------------------------------------------------------------

/// The asked trick byte (the pad reader's: R1 4), the held word the trick step sets, the trick hold's
/// two clocks (hold, shake) and the shared smooth noise the hold shakes by.
const ASKED: u32 = BIKE + 0x14ec;
const HELD: u32 = BIKE + 0x1788;
const HOLD_CLOCK: u32 = BIKE + 0x14dc;
const SHAKE_CLOCK: u32 = BIKE + 0x14e0;
const SMOOTH_NOISE: u32 = 0x0013_4580;

/// [`restart_image`] with the smooth noise answering the port's (`game_sim::ride::noise`, the rider's
/// balance's stand-in), the hold clock zeroed as the stunt start leaves it, and the shake clock as the
/// image has it. The driver the port starts from carries the same shake clock.
fn hold_image() -> Option<(Ee, Driver)> {
    let mut ee = restart_image()?;
    ee.hooks.insert(SMOOTH_NOISE, Box::new(|ee: &mut Ee| {
        let t = F(ee.f(12));
        ee.set_f(0, game_sim::ride::noise(t).0);
    }));
    ee.set_u32(HOLD_CLOCK, 0);
    let d = Driver { shake: f(&ee, SHAKE_CLOCK), ..airborne() };
    Some((ee, d))
}

fn ask_r1(ee: &mut Ee, on: bool) {
    let asked = ee.u32_at(ASKED) & !0xff;
    ee.set_u32(ASKED, asked | if on { 4 } else { 0 });
}

/// The ride the hold tests run: R1 alone held for 60 ticks (the clock stops at 0.7333 on tick 22, the
/// clip held from tick 23 — past the shake's half-second ramp), let go for 20 (the clock runs on), then
/// held again for 10 (too late: the clock is past 0.72 + 0.04, nothing holds).
const HOLD_RIDE: [(bool, u32); 3] = [(true, 60), (false, 20), (true, 10)];

/// **The port's rider holds a turning 360 under R1 as the original's does** (R47's hold, vt
/// `+0x4ec`): every tick of [`HOLD_RIDE`], the saved-pose weight and the clip's phase bit for bit
/// against SLUS-21104's trick step and rider layer — the held phase the frozen clock plus the shake,
/// over the Duration; the hold's word and its two clocks follow the original's.
#[test]
fn the_ports_rider_holds_a_360_under_r1_as_the_original() {
    let Some((mut ee, mut d)) = hold_image() else { return };
    let lib = spin_lib(false);
    let (mut held_ticks, mut shaken) = (0, 0);
    for (n, &(r1, ticks)) in HOLD_RIDE.iter().enumerate() {
        for k in 1..=ticks {
            ask_r1(&mut ee, r1);
            original_tick(&mut ee, false);
            assert_eq!(ee.u32_at(STUNT), SPIN_RIGHT, "leg {n}, tick {k}: the stunt runs");
            let (ramp, phase) = port_tick_held(&mut d, &lib, va::SPIN_RIGHT, 1, r1);
            let held = ee.u32_at(HELD) != 0;
            assert_eq!(ramp, f(&ee, RIDER_FADE), "leg {n}, tick {k}: the fade");
            assert_eq!(phase, Some(f(&ee, CLOCK_PHASE).max(F::ZERO).min(F::ONE)), "leg {n}, tick {k}: the phase (held {held})");
            assert_eq!(d.reverse, f(&ee, CLOCK), "leg {n}, tick {k}: the stunt clock");
            assert_eq!((d.hold, d.shake), (f(&ee, HOLD_CLOCK), f(&ee, SHAKE_CLOCK)), "leg {n}, tick {k}: the hold's clocks");
            if held {
                held_ticks += 1;
                shaken += usize::from(Some(f(&ee, CLOCK) / F(1.5)) != phase);
            }
            assert_eq!(held, n == 0 && k >= 23, "leg {n}, tick {k}: held only from the clock's stop to letting go");
        }
    }
    assert_eq!(held_ticks, 38);
    assert!(shaken > 30, "the shake moves the phase ({shaken} of {held_ticks} held ticks)");
}

/// **The port's bike holds a turning 360 under R1 as the original's does**: its clip at the frozen
/// clock over the clip's length, no shake (the bike reads the clock, not the shaken phase), every
/// tick of [`HOLD_RIDE`] bit for bit against the phase SLUS-21104's bike animate plays its clip at.
#[test]
fn the_ports_bike_holds_a_360_under_r1_as_the_original() {
    let Some((mut ee, mut d)) = hold_image() else { return };
    let played = bike_play(&mut ee);
    let lib = spin_lib(true);
    for (n, &(r1, ticks)) in HOLD_RIDE.iter().enumerate() {
        for k in 1..=ticks {
            ask_r1(&mut ee, r1);
            played.set(None);
            original_tick(&mut ee, false);
            let (ramp, phase) = port_tick_held(&mut d, &lib, va::SPIN_RIGHT, 1, r1);
            assert_eq!(ramp, f(&ee, BIKE_FADE), "leg {n}, tick {k}: the fade");
            assert_eq!(phase, played.get(), "leg {n}, tick {k}: the clip's phase");
            assert_eq!(d.reverse, f(&ee, CLOCK), "leg {n}, tick {k}: the stunt clock");
        }
    }
}

/// **R1 alone reaches the animations**: the 360's step leaves the flag the rider's and bike's
/// requests read, and a held tick before the clock's stop does not hold.
#[test]
fn r1_alone_before_the_clocks_stop_does_not_hold() {
    let lib = spin_lib(false);
    let mut d = airborne();
    for k in 1..=21 {
        let (_, phase) = port_tick_held(&mut d, &lib, va::SPIN_RIGHT, 1, true);
        assert_eq!(phase, Some(d.reverse / F(1.5)), "tick {k}");
        assert_eq!(d.hold, F::ZERO, "tick {k}");
    }
    let before = d.reverse;
    port_tick_held(&mut d, &lib, va::SPIN_RIGHT, 1, true);
    assert!(d.reverse > before && d.reverse >= F(0.72), "tick 22 still runs the clock, to its stop");
    port_tick_held(&mut d, &lib, va::SPIN_RIGHT, 1, true);
    assert_eq!(d.hold, F::from_bits(DT), "tick 23 holds");
}
