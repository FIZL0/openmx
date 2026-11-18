//! **The bike's pad reader** (F31d): what MX vs. ATV's bike (and quad) takes from the pad each frame,
//! and the input object's record it takes it from. Where the original keeps them: research/oracle.txt
//! `# F31d`.
//!
//! The input object samples the pad into a ring ([`Ring`]: 32 samples, each a time and the four axes)
//! and, once a frame, builds a record ([`Record`]): the player's device, the controls' strengths (32
//! buttons, then four analogue entries holding each axis's flick, [`Ring::flick`]) and the four axes as
//! they are now. It hands the record to its listeners; the bike's is [`read`], which looks its commands
//! up in the controls' bindings ([`Bindings`]) and stores the levers, the stick, the holds, the trick
//! byte and the stick's way into the bike ([`Words`]).
//!
//! **The sampling is not here.** The original's sampler is a callback the engine calls with the time
//! whenever it passes one of its waits, a number of times a frame (and at moments) set by how long the
//! hardware made it wait. [`Ring::sample`] is MX Unleashed's stand-in ([`crate::ride::Flick`]): one
//! reading a tick, pushed [`crate::ride::FLICK_RATE`] times a second on the simulation's clock.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use ps2_float::{F, fpu::Acc};

/// The ring's samples.
pub const SAMPLES: usize = 32;

/// The input object's sample ring: the slot the next sample goes in (also the oldest), each sample's
/// time and its four axes (left stick x, y, right stick x, y).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ring {
    pub head: usize,
    pub at: [F; SAMPLES],
    pub axes: [[F; 4]; SAMPLES],
    /// The stand-in sampler's clock ([`Ring::sample`]).
    pub now: F,
}

impl Ring {
    /// Axis `axis`'s flick: the ring scanned oldest first, a sample past ±0.8 remembered (the latest
    /// on its side wins), the next past 0.8 on the other side giving the change over the time between
    /// the two when the remembered time is above zero (whatever the time between, zero and negative
    /// too), and forgetting it either way; the largest in
    /// magnitude kept (the first of equals), zero for none.
    pub fn flick(&self, axis: usize) -> F {
        let mut best = F::ZERO;
        let (mut then, mut from) = (F::ZERO, F::ZERO);
        for k in 0..SAMPLES {
            let s = (self.head + k) % SAMPLES;
            let v = self.axes[s][axis];
            if v.abs() <= F(0.8) {
                continue;
            }
            if then != F::ZERO && (v < F::ZERO) != (from < F::ZERO) {
                if !(then <= F::ZERO) {
                    let speed = (v - from) / (self.at[s] - then);
                    if !(speed.abs() <= best.abs()) {
                        best = speed;
                    }
                }
                then = F::ZERO;
                continue;
            }
            then = self.at[s];
            from = v;
        }
        best
    }

    /// The four flicks, the record's analogue entries `0x20..0x23`.
    pub fn flicks(&self) -> [F; 4] {
        std::array::from_fn(|a| self.flick(a))
    }

    /// The stand-in sampler: one reading `dt` after the last, pushed as many times as the original
    /// samples in that time ([`crate::ride::FLICK_RATE`], at least once; the clock starts at the first
    /// `dt`, since the scan ignores a remembered time of zero).
    pub fn sample(&mut self, axes: [F; 4], dt: F) {
        self.now = self.now + dt;
        #[allow(clippy::float_arithmetic)] // a count of samples, not a simulated value
        let times = ((dt.0 * crate::ride::FLICK_RATE).round() as usize).clamp(1, SAMPLES);
        for _ in 0..times {
            self.at[self.head] = self.now;
            self.axes[self.head] = axes;
            self.head = (self.head + 1) % SAMPLES;
        }
    }
}

/// The record's controls: the 32 buttons in the device's order, then the four flicks.
pub const ENTRIES: usize = 0x24;
/// The first analogue entry: axis `a`'s flick is entry `FLICKS + a`.
pub const FLICKS: usize = 0x20;

/// The record the input object hands its listeners: the player's device (0 or 1, the one whose
/// controls the input reads), each control's strength (`values[i + 1]` for control `i`; `values[0]`
/// is the word before the first entry, which a binding lookup's miss, −1, reads), and the four axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Record {
    pub device: u32,
    pub values: [F; ENTRIES + 1],
    pub axes: [F; 4],
}

impl Record {
    /// Control `i`'s strength (−1 the word before the first). The bindings hold no other index.
    pub fn strength(&self, i: i32) -> F {
        let k = usize::try_from(i + 1).ok().filter(|&k| k < self.values.len());
        self.values[k.unwrap_or_else(|| panic!("control {i} is outside the pad record"))]
    }
}

/// The controls' binding table as the reader looks it up (the binding style 2, kind 10): per device
/// (0 and 1) the control index each of the first 0x50 commands is on, −1 for none; `None` with no
/// table at all.
#[derive(Clone, Debug, PartialEq)]
pub struct Bindings(pub Option<Box<[[i32; 0x50]; 2]>>);

impl Bindings {
    /// Command `command`'s control on device `device`: −1 with no table, a command past 0x4f or a
    /// device past 1.
    pub fn control(&self, command: u32, device: u32) -> i32 {
        match &self.0 {
            Some(t) if command < 0x50 && device < 2 => t[device as usize][command as usize],
            _ => -1,
        }
    }
}

/// The reader's commands: the d-pad's four (left, right, up, down), gas, brake, clutch and the three
/// trick buttons (bits 1, 2 and 4 of the trick byte).
pub const LEFT: u32 = 0;
pub const RIGHT: u32 = 1;
pub const UP: u32 = 2;
pub const DOWN: u32 = 3;
pub const GAS: u32 = 0xe;
pub const BRAKE: u32 = 0xf;
pub const CLUTCH: u32 = 0x10;
pub const TRICKS: [u32; 3] = [0x11, 0x12, 0x13];

/// The bike's words the reader reads and writes, by their offsets in the bike.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Words {
    /// `+0x310` the brake lever, `+0x314` the gas lever.
    pub brake: F,
    pub gas: F,
    /// `+0x12a0`/`+0x12a4`: the d-pad set the stick's x / y.
    pub dpad: [u32; 2],
    /// `+0x12a8`/`+0x12ac`: the stick (the d-pad over it).
    pub stick: [F; 2],
    /// `+0x12b0`: the flip hold, the stick's fore-aft flick; `+0x12b4` the spin hold, its sideways one.
    pub flip_hold: F,
    pub spin_hold: F,
    /// `+0x12b8`/`+0x12bc`: the stick's angle and its reach ([`way`]).
    pub angle: F,
    pub reach: F,
    /// `+0x12c0` the clutch lever, `+0x12c4` the reverse (gas and brake both held, slow).
    pub clutch: F,
    pub reverse: F,
    /// `+0x14c8`: the stick's way ([`way`]).
    pub way: u32,
    /// `+0x14d4`: set with no trick button held, cleared while wrecked (the put-back's ask).
    pub riding: u32,
    /// `+0x14ec`'s low byte: the trick buttons (1, 2, 4) and the stick's way (up 0x40, down 0x80,
    /// left 0x10, right 0x20).
    pub trick: u8,
    /// `+0x17ac`: the clutch press latch.
    pub latch: F,
}

/// The bike's words the reader only reads: wrecked (`+0xc80` not 0), the brake's scale (`+0x13b0`
/// on, `+0x13b4` the scale), the speed (`+0xc38`) and whether the gas still goes with the brake past
/// 5 of it (`+0xde0` not 0).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bike {
    pub wrecked: bool,
    pub brake_scaled: bool,
    pub brake_scale: F,
    pub speed: F,
    pub gas_with_brake: bool,
}

/// **The pad reader**, run when the bike takes a pad (`+0xbc4` not 0 and the vehicle's driver live):
/// `rec` into `w`. Answers whether it posts the clutch's press cue (a held clutch the latch had not
/// seen at 1.0), which is the caller's to send.
///
/// The trick byte starts cleared. Wrecked, the stick, the levers, the clutch and the riding flag go
/// to 0, and nothing else is written (the holds, the d-pad flags, the way and the latch keep theirs).
/// Otherwise:
/// - the stick is the record's, the flip and spin holds its entries `0x21` and `0x20` (the left
///   stick's fore-aft and sideways flicks on the shipped bindings), and the d-pad overrides each axis
///   at ±1 (left before right, up before down) and flags it;
/// - the gas is 1 or 0; the brake 0, else 1, or the brake's scale when it is on; with both on, under
///   a speed of 5 they go to 0 and the reverse to 1, else the reverse is 0 and the gas only stays
///   with `gas_with_brake`;
/// - the clutch is a switch (1 for any strength but zero), its latch following it;
/// - the trick buttons set bits 1, 2, 4; none of them sets the riding flag;
/// - the stick's [`way`] goes into `+0x14c8` and its bit into the trick byte.
///
/// A button is on for any strength but zero (either sign).
pub fn read(w: &mut Words, bike: &Bike, rec: &Record, bindings: &Bindings) -> bool {
    let on = |command: u32| rec.strength(bindings.control(command, rec.device)) != F::ZERO;
    w.trick = 0;
    if bike.wrecked {
        w.stick = [F::ZERO; 2];
        (w.brake, w.gas, w.clutch) = (F::ZERO, F::ZERO, F::ZERO);
        w.riding = 0;
        return false;
    }
    w.stick = [rec.axes[0], rec.axes[1]];
    w.spin_hold = rec.values[1 + FLICKS];
    w.flip_hold = rec.values[2 + FLICKS];
    w.dpad = [0, 0];
    for (axis, (less, more)) in [(LEFT, RIGHT), (UP, DOWN)].into_iter().enumerate() {
        if on(less) {
            (w.stick[axis], w.dpad[axis]) = (F(-1.0), 1);
        } else if on(more) {
            (w.stick[axis], w.dpad[axis]) = (F::ONE, 1);
        }
    }
    w.gas = if on(GAS) { F::ONE } else { F::ZERO };
    w.brake = if !on(BRAKE) {
        F::ZERO
    } else if bike.brake_scaled {
        bike.brake_scale
    } else {
        F::ONE
    };
    if w.brake <= F::ZERO || w.gas <= F::ZERO {
        w.reverse = F::ZERO;
    } else if F(5.0) <= bike.speed {
        w.reverse = F::ZERO;
        w.gas = if bike.gas_with_brake { F::ONE } else { F::ZERO };
    } else {
        (w.reverse, w.gas, w.brake) = (F::ONE, F::ZERO, F::ZERO);
    }
    let mut cue = false;
    if on(CLUTCH) {
        w.clutch = F::ONE;
        cue = w.latch != F::ONE;
        w.latch = w.clutch;
    } else {
        (w.clutch, w.latch) = (F::ZERO, F::ZERO);
    }
    for (command, bit) in TRICKS.into_iter().zip([1, 2, 4]) {
        if on(command) {
            w.trick |= bit;
        }
    }
    if w.trick == 0 {
        w.riding = 1;
    }
    let (angle, reach, way) = way(w.stick[0], w.stick[1]);
    (w.angle, w.reach, w.way) = (angle, reach, way);
    w.trick |= match way {
        2 => 0x80,
        1 => 0x40,
        4 => 0x20,
        3 => 0x10,
        _ => 0,
    };
    cue
}

/// The stick's way: its angle (the arc cosine, off the table, of `x` over the reach, clamped twice to
/// ±1; negated for a `y` below 0), its reach (`sqrt(x² + y²)`) and, past a reach of 0.5 (its square
/// past 0.25), which quarter the angle is in: 4 right (within π/4 of 0), 2 down (π/4..3π/4), 3 left
/// (past 3π/4 either way), 1 up (−3π/4..−π/4); 0 inside. Up is a negative `y`, as the pad's.
pub fn way(x: F, y: F) -> (F, F, u32) {
    let square = Acc::mula(x, x).madd(y, y);
    let c = x * F::ONE.rsqrt(square);
    let unit = |c: F| {
        if !(c <= F::ONE) {
            F::ONE
        } else if c < F(-1.0) {
            F(-1.0)
        } else {
            c
        }
    };
    let c = unit(unit(c));
    let t = acos_table().entries();
    let s = (F::ONE + c) * F(ACOS_SCALE);
    let i = s.to_i32();
    let frac = s - F::from(i);
    let a = Acc::mula(frac, F(t[i as usize + 1])).madd(F::ONE - frac, F(t[i as usize]));
    let angle = if y < F::ZERO { -a } else { a };
    let reach = square.sqrt();
    let (q, h) = (F::from_bits(0x3f49_0fdb), F::from_bits(0x4016_cbe4));
    let way = if square <= F(0.25) {
        0
    } else if !(angle < -q) && angle <= q {
        4
    } else if !(angle < q) && angle <= h {
        2
    } else if !(angle < h) || angle <= -h {
        3
    } else if !(angle <= -q) || angle < -h {
        0
    } else {
        1
    };
    (angle, reach, way)
}

const ACOS_SCALE: f32 = 14999.5;

fn acos_table() -> &'static crate::air::AcosTable {
    static TABLE: std::sync::OnceLock<crate::air::AcosTable> = std::sync::OnceLock::new();
    TABLE.get_or_init(crate::air::AcosTable::build)
}
