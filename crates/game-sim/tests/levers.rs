//! P77 — the bike's three levers, against the original.
//!
//! The original's bike reads the gas, the brake and the clutch as switches: its controller read
//! (`pad_to_gesture`) writes 1.0 for any non-zero strength of the command's binding and 0 otherwise, a
//! held brake is the `[Pitch]` file's `StoppieBrake` while the stoppie latch is set, and gas and brake
//! held together are the reverse below speed 5. Its limiter (`bike_input_limiter`) then walks the
//! bike's own copies toward those switches. Both run here in the interpreter, chained as the bike runs
//! them, on generated states, and the port's [`Controls`] tail and [`Held::step`] are held against
//! what they leave, bit for bit.
//!
//! The binding lookup is hooked to hand back the command itself as the control's index, so a
//! generated strength per command is what the read finds; the stick's direction code (another
//! function) is hooked to 0, because only the levers are under test here. The stick's own squaring
//! and walk are P30's and are not compared, except that steering off holds the sideways lean at 0.
//!
//! P158 generates the limiter's steer and throttle enables (`+0x610`/`+0x614`), last frame's steer
//! enable (`+0x618`) and the start-line latch (`+0x61c`) too, and the bike tick's gas cut while the
//! card's start-gate penalty runs — which the original makes between the two calls, one store of 0 to
//! `+0x63c`, and the harness repeats.

use game_sim::phy::Joystick;
use game_sim::ride::{self, Controls, Held, Lock};
use ps2_float::F;
use ps2_oracle::{Ee, addr, game_or_skip};

/// The bike, the controller's strength rows, the player record and the application object.
const BIKE: u32 = 0x0120_0000;
const PAD: u32 = 0x0121_0000;
const PLAYER: u32 = 0x0122_0000;
const APP: u32 = 0x0123_0000;

/// The commands the bike's levers are read off (research/oracle.txt, P73).
const GAS: u32 = 0x0e;
const BRAKE: u32 = 0x0f;
const CLUTCH: u32 = 0x10;

/// A small deterministic generator, so a failure names a case that comes back.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
    fn unit(&mut self) -> f32 {
        self.next() as f32 / (1u64 << 31) as f32
    }
    fn coin(&mut self, n: u32) -> bool {
        self.next() % n == 0
    }
    /// A command's strength as the input layer hands it out: let go, full, or part way.
    fn strength(&mut self) -> f32 {
        match self.next() % 3 {
            0 => 0.0,
            1 => 1.0,
            _ => self.unit().max(1.0 / 256.0),
        }
    }
    /// A lever's walked value, anywhere in its travel.
    fn lever(&mut self) -> f32 {
        match self.next() % 4 {
            0 => 0.0,
            1 => 1.0,
            _ => self.unit(),
        }
    }
}

struct Case {
    gas: f32,
    brake: f32,
    clutch: f32,
    stoppie: bool,
    stoppie_brake: f32,
    speed: f32,
    airborne: bool,
    hands_off: bool,
    dt: f32,
    rates: [f32; 4],
    walked: [f32; 3],
    /// Steer on, throttle on, steer on last frame, the latch: the original's words, not the port's
    /// locks.
    enables: [bool; 4],
    gas_cut: bool,
    /// The sideways lean and its rate.
    lean: f32,
    lean_rate: f32,
}

impl Case {
    fn new(r: &mut Rng) -> Case {
        Case {
            gas: r.strength(),
            brake: r.strength(),
            clutch: r.strength(),
            stoppie: r.coin(3),
            stoppie_brake: if r.coin(5) { 0.0 } else { r.unit() },
            speed: if r.coin(10) { 5.0 } else { (r.unit() - 0.5) * 50.0 },
            airborne: r.coin(4),
            hands_off: r.coin(5),
            dt: if r.coin(2) { 1.0 / 30.0 } else { 0.001 + r.unit() * 0.05 },
            // Every disc bike's rates are positive; the port reads a zero as no limit (S58), which
            // the original does not, so zeros are left out.
            rates: [0; 4].map(|_| 0.5 + r.unit() * 20.0),
            walked: [r.lever(), r.lever(), r.lever()],
            // Each word on more often than not, so the riding case stays the common one.
            enables: [!r.coin(3), !r.coin(3), !r.coin(3), r.coin(2)],
            gas_cut: r.coin(5),
            lean: (r.unit() - 0.5) * 2.0,
            lean_rate: 0.5 + r.unit() * 20.0,
        }
    }
}

fn harness() -> Option<Ee> {
    let mut ee = game_or_skip()?;
    // The binding lookup: the command is the index of its strength row.
    ee.hooks.insert(addr("pad_control_index").unwrap(), Box::new(|ee| {
        let command = ee.r(5);
        ee.set(2, command);
    }));
    ee.hooks.insert(addr("stick_direction").unwrap(), Box::new(|ee| ee.set(2, 0)));
    ee.set_u32(addr("app_pointer").unwrap(), APP);
    Some(ee)
}

fn poke(ee: &mut Ee, c: &Case) {
    for o in (0..0x1000).step_by(4) {
        ee.set_u32(BIKE + o, 0);
    }
    for o in (0..0x0e00).step_by(4) {
        ee.set_u32(PAD + o, 0);
    }
    for (cmd, v) in [(GAS, c.gas), (BRAKE, c.brake), (CLUTCH, c.clutch)] {
        ee.set_f32(PAD + cmd * 0x18 + 4, v);
    }
    ee.set_u32(BIKE + 0x20, PLAYER);
    ee.set_u32(PLAYER + 0x34, 1);
    // Every riding save state has steering and throttle on and the latch idle; P158 generates them.
    for (k, on) in c.enables.iter().enumerate() {
        ee.set_u32(BIKE + 0x610 + 4 * k as u32, *on as u32);
    }
    ee.set_f32(BIKE + 0x670, c.lean);
    ee.set_f32(BIKE + 0x660, c.lean_rate);
    ee.set_u32(BIKE + 0x728, c.stoppie as u32);
    ee.set_f32(BIKE + 0x72c, c.stoppie_brake);
    ee.set_f32(BIKE + 0x438, c.speed);
    ee.set_u32(BIKE + 0x57c, c.airborne as u32);
    ee.set_u32(BIKE + 0xa44, c.hands_off as u32);
    ee.set_f32(BIKE + 0x650, c.walked[0]);
    ee.set_f32(BIKE + 0x654, c.walked[0]);
    ee.set_f32(BIKE + 0x658, c.walked[1]);
    ee.set_f32(BIKE + 0x65c, c.walked[2]);
    for (k, v) in c.rates.iter().enumerate() {
        ee.set_f32(BIKE + 0x688 + 4 * k as u32, *v);
    }
}

/// What the original leaves: the pad read's four commands, then the limiter's four levers, the
/// brake and the reverse as it leaves them, the steer word it keeps, the latch and the sideways lean.
fn original(ee: &mut Ee, c: &Case) -> [u32; N] {
    poke(ee, c);
    ee.call(addr("pad_to_gesture").unwrap(), &[BIKE as u64, PAD as u64, 0], &[0.0, 0.0], 100_000);
    // The bike tick's cut while the card's +0x1c0 is above 0, between the read and the limiter.
    if c.gas_cut {
        ee.set_u32(BIKE + 0x63c, 0);
    }
    let read = [0x63c, 0x640, 0x644, 0x648].map(|o| ee.u32_at(BIKE + o));
    ee.call(addr("bike_input_limiter").unwrap(), &[BIKE as u64], &[c.dt], 100_000);
    let walked = [0x650, 0x654, 0x658, 0x65c, 0x640, 0x648, 0x618, 0x61c, 0x670].map(|o| ee.u32_at(BIKE + o));
    let mut out = [0; N];
    out[..4].copy_from_slice(&read);
    out[4..].copy_from_slice(&walked);
    out
}

/// The port's: the switches, the pad read's tail, the gate cut, and the limiter's walk.
fn port(c: &Case) -> [u32; N] {
    let asked = Controls {
        gas: ride::switch(F(c.gas)),
        brake: ride::switch(F(c.brake)),
        clutch: ride::switch(F(c.clutch)),
        hands_off: c.hands_off,
        gas_cut: c.gas_cut,
        ..Controls::default()
    }
    .pad_read_tail(c.stoppie, F(c.stoppie_brake), F(c.speed), c.airborne)
    .gate_cut();
    let j = Joystick {
        gas_rate_limit: F(c.rates[0]),
        brake_rate_limit: F(c.rates[1]),
        clutch_in_rate_limit: F(c.rates[2]),
        clutch_out_rate_limit: F(c.rates[3]),
        x_axis_rate_limit: F(c.lean_rate),
        ..Joystick::default()
    };
    let mut held = Held {
        brake: F(c.walked[0]),
        front_brake: F(c.walked[0]),
        rear_brake: F(c.walked[0]),
        gas: F(c.walked[1]),
        clutch: F(c.walked[2]),
        lean: (F(c.lean), F::ZERO),
        reverse: asked.reverse,
        lock: Lock { steer: !c.enables[0], throttle: !c.enables[1], steer_was: !c.enables[2], latch: c.enables[3] },
        ..Held::default()
    };
    held.step(&asked, &j, F(c.dt));
    assert_eq!(held.front_brake.0.to_bits(), held.rear_brake.0.to_bits(), "one brake, both wheels");
    let read = [asked.gas, asked.brake, asked.clutch, asked.reverse].map(|v| v.0.to_bits());
    // The latch writes the brake lever full in the original; the port walks toward it without
    // writing the pad read's copy back, so the lever the limiter leaves is the latched one or the read.
    let brake = if held.lock.latch { F::ONE } else { asked.brake };
    let walked = [held.front_brake, held.rear_brake, held.gas, held.clutch, brake, held.reverse]
        .map(|v| v.0.to_bits());
    let mut out = [0; N];
    out[..4].copy_from_slice(&read);
    out[4..10].copy_from_slice(&walked);
    out[10] = !held.lock.steer_was as u32;
    out[11] = held.lock.latch as u32;
    out[12] = held.lean.0.0.to_bits();
    out
}

const N: usize = 13;
const FIELDS: [&str; N] = [
    "gas +0x63c", "brake +0x640", "clutch +0x644", "reverse +0x648",
    "front brake +0x650", "rear brake +0x654", "gas +0x658", "clutch +0x65c",
    "limited brake lever +0x640", "limited reverse +0x648", "steer last frame +0x618", "latch +0x61c",
    "sideways lean +0x670",
];

#[test]
fn the_levers_are_the_originals_switches_and_walk() {
    let Some(mut ee) = harness() else { return };
    let mut r = Rng(0x7077_1e4e);
    let (mut reversed, mut stoppie_brakes, mut cut) = (0, 0, 0);
    let (mut latched, mut let_go, mut gate_cut, mut throttle_off) = (0, 0, 0, 0);
    for i in 0..20_000 {
        let c = Case::new(&mut r);
        let want = original(&mut ee, &c);
        let got = port(&c);
        for k in 0..N {
            assert_eq!(
                got[k], want[k],
                "case {i}: {} is {} in the port, {} in the original (gas {} brake {} clutch {} stoppie {} \
                 {} speed {} airborne {} hands off {} dt {} rates {:?} walked {:?} enables {:?} gas cut {} \
                 lean {} at {})",
                FIELDS[k], f32::from_bits(got[k]), f32::from_bits(want[k]), c.gas, c.brake, c.clutch,
                c.stoppie, c.stoppie_brake, c.speed, c.airborne, c.hands_off, c.dt, c.rates, c.walked,
                c.enables, c.gas_cut, c.lean, c.lean_rate
            );
        }
        latched += (want[11] != 0 && !c.enables[3]) as u32;
        let_go += (want[11] == 0 && c.enables[3]) as u32;
        gate_cut += (c.gas_cut && c.gas != 0.0) as u32;
        throttle_off += (!c.enables[1] && c.gas != 0.0 && !c.hands_off) as u32;
        reversed += (want[3] != 0) as u32;
        stoppie_brakes += (c.stoppie && c.brake != 0.0 && want[3] == 0) as u32;
        cut += (c.hands_off && c.gas != 0.0) as u32;
    }
    // Every branch is ridden, not just the plain one.
    assert!(reversed > 1000 && stoppie_brakes > 1000 && cut > 1000, "{reversed} {stoppie_brakes} {cut}");
    assert!(latched > 500 && let_go > 500 && gate_cut > 1000 && throttle_off > 1000,
            "{latched} {let_go} {gate_cut} {throttle_off}");
}

/// A part-pressed lever is the whole lever: what the New layout's analogue R2 and L2 come to, since the
/// shell moves them onto Cross's and Square's pressure bytes and the bike reads those through the same
/// switch (P77, the user's question answered toward the original).
#[test]
fn a_part_pressed_lever_is_a_switch() {
    for v in [1.0 / 256.0, 0.25, 0.5, 0.999, 1.0] {
        assert_eq!(ride::switch(F(v)).0, 1.0);
    }
    assert_eq!(ride::switch(F::ZERO).0, 0.0);
}

/// P158 — the card's start-gate penalty runs down in the score manager's tick, every card's, while it
/// is above 0: the original's walk over the manager's card list, in the interpreter, against
/// [`game_sim::score::Score::grow`] on generated penalties and frame times.
#[test]
fn the_gate_penalty_runs_down_as_the_originals() {
    let Some(mut ee) = game_or_skip() else { return };
    const MGR: u32 = 0x0124_0000;
    const CARDS: [u32; 3] = [0x0125_0000, 0x0126_0000, 0x0127_0000];
    let mut r = Rng(0x0158_9a7e);
    let mut ran = 0;
    for i in 0..10_000 {
        let dt = if r.coin(2) { 1.0 / 30.0 } else { r.unit() * 0.1 };
        let at = CARDS.map(|_| match r.next() % 4 {
            0 => 0.0,
            1 => -r.unit(),
            2 => dt,
            _ => r.unit() * 2.0,
        });
        // The manager's list: +0x34 the first card, each card's +0x1c8 the next.
        ee.set_u32(MGR + 0x34, CARDS[0]);
        for (k, card) in CARDS.iter().enumerate() {
            ee.set_f32(card + 0x1c0, at[k]);
            ee.set_u32(card + 0x1c8, CARDS.get(k + 1).copied().unwrap_or(0));
        }
        ee.call(addr("score_gate_penalty_tick").unwrap(), &[MGR as u64], &[dt], 10_000);
        for (k, card) in CARDS.iter().enumerate() {
            let mut s = game_sim::score::Score { gate_penalty: F(at[k]), ..Default::default() };
            s.grow(F(dt));
            let want = ee.u32_at(card + 0x1c0);
            assert_eq!(s.gate_penalty.0.to_bits(), want, "case {i} card {k}: {} from {} by {dt}", f32::from_bits(want), at[k]);
            ran += (at[k] > 0.0) as u32;
        }
    }
    assert!(ran > 10_000, "{ran}");
}
