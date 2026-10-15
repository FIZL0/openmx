//! VU0 in **macro mode** — the vector unit the EE drives directly with `COP2` instructions — as the
//! reference emulator performs it.
//!
//! A PS2 game's vector math (transforms, cross products, normalises, collision inner loops) is usually
//! VU0 macro-mode code sitting in the middle of ordinary C, so a port has to model it as well as the FPU.
//! How it differs from the FPU next door:
//!
//! * **Round toward zero**, like the FPU.
//! * **No alignment quirk.** The VU adder is a plain adder: it lines the operands up with full precision
//!   and then truncates the result. So `vaddx` and `add.s` of the same two numbers can differ — this is
//!   the single most common reason a port that "uses the PS2 float model" still drifts. Use [`crate::fpu`]
//!   for code the compiler put on COP1 and this module for code it put on COP2.
//! * **Nothing but finite numbers.** VU floats have no Inf and no NaN: an overflow *clamps* to
//!   `±1.7014118e38` and a denormal is zero. A bit pattern with exponent 255 read out of memory behaves
//!   as that maximum.
//! * **`vmadd` is two roundings**, a chopped product then a chopped add, exactly like `madd.s`.
//! * **Divide is a separate unit.** `vdiv` / `vsqrt` / `vrsqrt` write the single `Q` register; the result
//!   is chopped and `x / 0` gives `±max`.
//!
//! ## What is modelled, and what is not
//!
//! Modelled: the arithmetic of the macro-mode ops below, the destination field mask (`.xyzw` subsets),
//! the broadcast forms (`vaddx`, `vmulw`, …), the accumulator, `Q`, `vftoi`/`vitof` fixed-point
//! conversions, and — because this game branches on them — the MAC, status and clipping flags
//! ([`Mac`], [`Status`], [`Clip`], all of them on [`Vu0`]).
//!
//! **Not** modelled: the I and R registers (this game uses neither); the integer register file
//! (`vi0`..`vi15`) and its branches; VU1 and its EFU ops (`esum`, `erleng`, `esin`, …); micro-program
//! control flow — this game has exactly one `vcallmsr`, in an SDK leaf routine, so some micro program
//! does run on VU0 even though every arithmetic op is macro mode; and all pipeline timing — in
//! particular `Q` is available immediately here, whereas the hardware makes a program wait seven cycles
//! (`vwaitq`). A micro program whose results you need cannot be reproduced by this module alone: either
//! port it by hand on top of these ops, or hook it out (see `ps2-oracle`'s `hooks`) and compare against
//! a capture.
#![deny(clippy::float_arithmetic)]

/// The largest magnitude a VU float can hold; results clamp here rather than going infinite.
pub const MAX: f32 = f32::MAX;

/// Four lanes as plain `f32`s, in `x, y, z, w` order.
pub type V4 = [f32; 4];

#[inline]
fn daz(x: f32) -> f32 {
    if x.is_subnormal() { if x.is_sign_negative() { -0.0 } else { 0.0 } } else { x }
}

/// How a VU reads a 32-bit pattern: exponent 255 clamps to `±max`, denormals are zero.
#[inline]
pub fn operand(bits: u32) -> f32 {
    match bits & 0x7f80_0000 {
        0x7f80_0000 => f32::from_bits((bits & 0x8000_0000) | 0x7f7f_ffff),
        0 => f32::from_bits(bits & 0x8000_0000),
        _ => f32::from_bits(bits),
    }
}

#[inline]
fn vu(x: f32) -> f32 {
    if x.is_finite() { daz(x) } else { operand(x.to_bits()) }
}

/// Round toward zero to `f32`, clamping an overflow to `±max`. `x` must be the exact result (or within
/// the right `f32` neighbourhood of it).
fn chop(x: f64) -> f32 {
    if !x.is_finite() {
        return if x.is_sign_negative() { crate::fpu::NEG_MAX } else { MAX };
    }
    #[allow(clippy::float_arithmetic)]
    let r = {
        let r = x as f32;
        if (r as f64).abs() > x.abs() { f32::from_bits(r.to_bits().wrapping_sub(1)) } else { r }
    };
    daz(r)
}

/// `vadd` (one lane): a full-precision add, chopped.
///
/// The exact sum of two `f32`s does not always fit in an `f64` — the exponents can be 270 apart — so the
/// `f64` sum alone would round a far smaller addend away and lose the *direction* of the truncation.
/// The exact rounding error (a TwoSum) decides it: `x` plus a tiny value of the opposite sign truncates
/// to the next `f32` toward zero, not to `x`.
#[allow(clippy::float_arithmetic)] // the model itself, in host arithmetic
pub fn add(a: f32, b: f32) -> f32 {
    let (a, b) = (vu(a) as f64, vu(b) as f64);
    let s = a + b;
    if !s.is_finite() {
        return if s.is_sign_negative() { crate::fpu::NEG_MAX } else { MAX };
    }
    let bb = s - a;
    let e = (a - (s - bb)) + (b - bb); // exact: s + e is the true sum
    let r = s as f32;
    let over = (r as f64 - s) - e; // r minus the true sum
    if over != 0.0 && r != 0.0 && (over > 0.0) == (r > 0.0) {
        daz(f32::from_bits(r.to_bits().wrapping_sub(1)))
    } else {
        daz(r)
    }
}

/// `vsub` (one lane).
pub fn sub(a: f32, b: f32) -> f32 {
    add(a, f32::from_bits(b.to_bits() ^ 0x8000_0000))
}

/// `vmul` (one lane): the 24×24-bit product is exact in `f64`, so the only rounding is the chop.
#[allow(clippy::float_arithmetic)] // the model itself, in exact host arithmetic
pub fn mul(a: f32, b: f32) -> f32 {
    chop(vu(a) as f64 * vu(b) as f64)
}

/// `vmadd` (one lane): `acc + a·b`, product chopped first, then the add chopped.
pub fn madd(acc: f32, a: f32, b: f32) -> f32 {
    add(acc, mul(a, b))
}

/// `vmsub` (one lane): `acc − a·b`.
pub fn msub(acc: f32, a: f32, b: f32) -> f32 {
    sub(acc, mul(a, b))
}

/// `vdiv` into `Q`, chopped; `x / 0` gives `±max`.
#[allow(clippy::float_arithmetic)] // the model itself; f32 division is exact per IEEE
pub fn div(a: f32, b: f32) -> f32 {
    let (a, b) = (vu(a), vu(b));
    if b == 0.0 {
        return if a.is_sign_negative() != b.is_sign_negative() { crate::fpu::NEG_MAX } else { MAX };
    }
    let r = a / b; // nearest; step toward zero when it rounded away
    if !r.is_finite() {
        return if r.is_sign_negative() { crate::fpu::NEG_MAX } else { MAX };
    }
    let over = ((r as f64) * (b as f64)).abs() > (a as f64).abs();
    daz(if over && r != 0.0 { f32::from_bits(r.to_bits().wrapping_sub(1)) } else { r })
}

/// `vsqrt` into `Q`: the square root of `|x|`, chopped.
pub fn sqrt(x: f32) -> f32 {
    chop(crate::libm::sqrtf(f32::from_bits(vu(x).to_bits() & 0x7fff_ffff)) as f64)
}

/// `vrsqrt` into `Q`: `a / sqrt(|b|)`, chopped.
pub fn rsqrt(a: f32, b: f32) -> f32 {
    div(a, sqrt(b))
}

/// Which lane a broadcast op reads (`vaddx`, `vmuly`, `vmaddw`, …).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    X = 0,
    Y = 1,
    Z = 2,
    W = 3,
}

/// A destination field mask: which lanes an op writes (`vadd.xz`, `vmul.xyz`, …).
///
/// The bits are the instruction's own: `x` is the high bit, `w` the low one. Lanes outside the mask keep
/// their old contents, in the destination register *and* in the accumulator — a detail that matters when
/// a `.xyz` op is followed by something reading `w`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dest(pub u8);

impl Dest {
    pub const X: Dest = Dest(0b1000);
    pub const Y: Dest = Dest(0b0100);
    pub const Z: Dest = Dest(0b0010);
    pub const W: Dest = Dest(0b0001);
    pub const XY: Dest = Dest(0b1100);
    pub const XYZ: Dest = Dest(0b1110);
    pub const XYZW: Dest = Dest(0b1111);
    pub const XZ: Dest = Dest(0b1010);
    pub const YZW: Dest = Dest(0b0111);

    /// Does this mask write lane `i` (0 = x … 3 = w)?
    pub fn has(self, i: usize) -> bool {
        self.0 >> (3 - i) & 1 == 1
    }
    /// The mask as the letters an assembly listing prints (`"xyz"`, `"w"`, …).
    pub fn name(self) -> &'static str {
        const NAMES: [&str; 16] = [
            "", "w", "z", "zw", "y", "yw", "yz", "yzw", "x", "xw", "xz", "xzw", "xy", "xyw", "xyz", "xyzw",
        ];
        NAMES[(self.0 & 0xf) as usize]
    }
}

/// A VU0 vector register: four lanes, `x y z w`.
///
/// Arithmetic is per lane with the functions above. The ops that take an accumulator take it as their
/// first argument, because that is the order the instruction reads: `vmadd fd, fs, ft` is
/// `Vf::madd(acc, fs, ft)`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(transparent)]
pub struct Vf(pub V4);

impl Vf {
    pub const ZERO: Vf = Vf([0.0; 4]);
    /// `vf0`, which is hard-wired to `(0, 0, 0, 1)`.
    pub const VF0: Vf = Vf([0.0, 0.0, 0.0, 1.0]);

    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Vf {
        Vf([x, y, z, w])
    }
    pub const fn splat(v: f32) -> Vf {
        Vf([v; 4])
    }
    pub fn x(self) -> f32 {
        self.0[0]
    }
    pub fn y(self) -> f32 {
        self.0[1]
    }
    pub fn z(self) -> f32 {
        self.0[2]
    }
    pub fn w(self) -> f32 {
        self.0[3]
    }
    pub fn lane(self, f: Field) -> f32 {
        self.0[f as usize]
    }
    /// One lane in all four: the operand a broadcast op (`vaddx`, `vmulw`, …) sees.
    pub fn bc(self, f: Field) -> Vf {
        Vf::splat(self.lane(f))
    }
    /// The four lanes as raw patterns (how they sit in the register file and in memory).
    pub fn to_bits(self) -> [u32; 4] {
        self.0.map(f32::to_bits)
    }
    /// Four raw patterns as a register, each read the way a VU reads one ([`operand`]).
    pub fn from_bits(b: [u32; 4]) -> Vf {
        Vf(b.map(operand))
    }

    /// Write `v`'s lanes into `self` where `dest` says to, leaving the rest alone.
    pub fn merge(self, dest: Dest, v: Vf) -> Vf {
        Vf(core::array::from_fn(|i| if dest.has(i) { v.0[i] } else { self.0[i] }))
    }

    /// `vadd` / `vsub` / `vmul`, lane by lane. Pair them with [`Vf::bc`] for the broadcast forms:
    /// `a.mul(b.bc(Field::X))` is `vmulx`.
    #[allow(clippy::should_implement_trait)] // named after the instruction; the operators live on `Lane`
    pub fn add(self, o: Vf) -> Vf {
        Vf(core::array::from_fn(|i| add(self.0[i], o.0[i])))
    }
    #[allow(clippy::should_implement_trait)] // named after the instruction, not after `std::ops`
    pub fn sub(self, o: Vf) -> Vf {
        Vf(core::array::from_fn(|i| sub(self.0[i], o.0[i])))
    }
    #[allow(clippy::should_implement_trait)] // named after the instruction, not after `std::ops`
    pub fn mul(self, o: Vf) -> Vf {
        Vf(core::array::from_fn(|i| mul(self.0[i], o.0[i])))
    }
    /// `vmadd` / `vmsub`: `acc ± a·b`, lane by lane.
    pub fn madd(acc: Vf, a: Vf, b: Vf) -> Vf {
        Vf(core::array::from_fn(|i| madd(acc.0[i], a.0[i], b.0[i])))
    }
    pub fn msub(acc: Vf, a: Vf, b: Vf) -> Vf {
        Vf(core::array::from_fn(|i| msub(acc.0[i], a.0[i], b.0[i])))
    }
    /// `vmulq` / `vaddq` / `vsubq`: every lane against the `Q` register.
    pub fn mul_q(self, q: f32) -> Vf {
        Vf(core::array::from_fn(|i| mul(self.0[i], q)))
    }
    pub fn add_q(self, q: f32) -> Vf {
        Vf(core::array::from_fn(|i| add(self.0[i], q)))
    }

    /// `vopmula`: the first half of a cross product, into the accumulator —
    /// `acc.x = a.y·b.z`, `acc.y = a.z·b.x`, `acc.z = a.x·b.y` (`w` untouched).
    pub fn opmula(a: Vf, b: Vf) -> Vf {
        Vf([mul(a.0[1], b.0[2]), mul(a.0[2], b.0[0]), mul(a.0[0], b.0[1]), 0.0])
    }
    /// `vopmsub`: the second half — the same lane pattern as [`Vf::opmula`], subtracted from the
    /// accumulator. The cross product comes out because the compiler *swaps the operands* between the
    /// two instructions:
    ///
    /// ```text
    ///     vopmula.xyz ACC, vf1, vf2      ; ACC.x = v1.y·v2.z …
    ///     vopmsub.xyz vf3, vf2, vf1      ; vf3.x = ACC.x − v2.y·v1.z …  = (v1 × v2).x
    /// ```
    ///
    /// so port the pair as the original writes it, operands and all, rather than reaching for [`cross`].
    pub fn opmsub(acc: Vf, a: Vf, b: Vf) -> Vf {
        Vf([msub(acc.0[0], a.0[1], b.0[2]), msub(acc.0[1], a.0[2], b.0[0]), msub(acc.0[2], a.0[0], b.0[1]), acc.0[3]])
    }

    /// `vftoi0/4/12/15`: multiply by `2^frac`, truncate toward zero, saturate at the `i32` ends. The
    /// result lives in the same register file, so hardware keeps the pattern in a float register —
    /// [`Vf::ftoi_bits`] models that, this returns the integers themselves.
    pub fn ftoi(self, frac: u32) -> [i32; 4] {
        #[allow(clippy::float_arithmetic)]
        let scale = (1u64 << frac) as f64;
        core::array::from_fn(|i| {
            #[allow(clippy::float_arithmetic)]
            let v = vu(self.0[i]) as f64 * scale;
            if v >= i32::MAX as f64 {
                i32::MAX
            } else if v <= i32::MIN as f64 {
                i32::MIN
            } else {
                v as i32
            }
        })
    }
    /// `vitof0/4/12/15`: each lane's *bit pattern* read as an `i32`, divided by `2^frac`, chopped.
    pub fn itof_bits(self, frac: u32) -> Vf {
        Vf::itof(self.to_bits().map(|b| b as i32), frac)
    }
    /// Integers to floats, divided by `2^frac`, chopped.
    pub fn itof(v: [i32; 4], frac: u32) -> Vf {
        #[allow(clippy::float_arithmetic)]
        let scale = (1u64 << frac) as f64;
        #[allow(clippy::float_arithmetic)]
        Vf(v.map(|i| chop(i as f64 / scale)))
    }
    /// [`Vf::ftoi`] left in the register file: the lanes hold the integers' bit patterns.
    pub fn ftoi_bits(self, frac: u32) -> Vf {
        Vf(self.ftoi(frac).map(|i| f32::from_bits(i as u32)))
    }
}

impl core::ops::Index<usize> for Vf {
    type Output = f32;
    fn index(&self, i: usize) -> &f32 {
        &self.0[i]
    }
}
impl core::ops::IndexMut<usize> for Vf {
    fn index_mut(&mut self, i: usize) -> &mut f32 {
        &mut self.0[i]
    }
}
impl From<V4> for Vf {
    fn from(v: V4) -> Vf {
        Vf(v)
    }
}
impl From<Vf> for V4 {
    fn from(v: Vf) -> V4 {
        v.0
    }
}
impl core::ops::Add for Vf {
    type Output = Vf;
    fn add(self, o: Vf) -> Vf {
        Vf::add(self, o)
    }
}
impl core::ops::Sub for Vf {
    type Output = Vf;
    fn sub(self, o: Vf) -> Vf {
        Vf::sub(self, o)
    }
}
impl core::ops::Mul for Vf {
    type Output = Vf;
    fn mul(self, o: Vf) -> Vf {
        Vf::mul(self, o)
    }
}

/// One VU0 lane, for porting a micro program or a macro-mode run by hand.
///
/// `+ - * /` are the VU's [`add`], [`sub`], [`mul`] and the `Q` [`div`], so the ported code keeps the
/// original's order (`a + b + c` is `(a + b) + c`) and reads like its listing. The same idea as
/// [`crate::F`], one unit over.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Lane(pub f32);

/// A register's four lanes as [`Lane`]s.
pub type Lanes = [Lane; 4];

/// Every lane of `v` as a [`Lane`].
pub fn lanes(v: V4) -> Lanes {
    v.map(Lane)
}

/// Every [`Lane`] back as an `f32`.
pub fn v4(v: Lanes) -> V4 {
    v.map(|x| x.0)
}

impl Lane {
    pub const ZERO: Lane = Lane(0.0);
    pub fn to_f32(self) -> f32 {
        self.0
    }
    /// `vmadd`: `self + a·b`.
    pub fn madd(self, a: impl Into<Lane>, b: impl Into<Lane>) -> Lane {
        Lane(madd(self.0, a.into().0, b.into().0))
    }
    /// `vmsub`: `self − a·b`.
    pub fn msub(self, a: impl Into<Lane>, b: impl Into<Lane>) -> Lane {
        Lane(msub(self.0, a.into().0, b.into().0))
    }
    /// `vsqrt` of `|self|`.
    pub fn sqrt(self) -> Lane {
        Lane(sqrt(self.0))
    }
}

impl From<f32> for Lane {
    fn from(x: f32) -> Lane {
        Lane(x)
    }
}
impl From<Lane> for f32 {
    fn from(x: Lane) -> f32 {
        x.0
    }
}
impl PartialEq<f32> for Lane {
    fn eq(&self, o: &f32) -> bool {
        self.0 == *o
    }
}
impl PartialOrd<f32> for Lane {
    fn partial_cmp(&self, o: &f32) -> Option<core::cmp::Ordering> {
        self.0.partial_cmp(o)
    }
}
impl core::ops::Neg for Lane {
    type Output = Lane;
    fn neg(self) -> Lane {
        Lane(f32::from_bits(self.0.to_bits() ^ 0x8000_0000))
    }
}

macro_rules! vu_op {
    ($tr:ident $m:ident $as_tr:ident $as_m:ident $op:ident) => {
        impl core::ops::$tr<Lane> for Lane {
            type Output = Lane;
            fn $m(self, o: Lane) -> Lane {
                Lane($op(self.0, o.0))
            }
        }
        impl core::ops::$tr<f32> for Lane {
            type Output = Lane;
            fn $m(self, o: f32) -> Lane {
                Lane($op(self.0, o))
            }
        }
        impl core::ops::$tr<Lane> for f32 {
            type Output = Lane;
            fn $m(self, o: Lane) -> Lane {
                Lane($op(self, o.0))
            }
        }
        impl<T: Into<Lane>> core::ops::$as_tr<T> for Lane {
            fn $as_m(&mut self, o: T) {
                *self = Lane($op(self.0, o.into().0));
            }
        }
    };
}
vu_op!(Add add AddAssign add_assign add);
vu_op!(Sub sub SubAssign sub_assign sub);
vu_op!(Mul mul MulAssign mul_assign mul);
vu_op!(Div div DivAssign div_assign div);

// --- idioms every PS2 game's VU0 code contains -----------------------------------------------------
// These are the shapes a compiler emits again and again. They are here as a starting point and as
// documentation of the instruction order; when a function's listing does it differently, port *that*.

/// `vmul.xyz` then `vaddy.x` then `vaddz.x`: `(a.x·b.x + a.y·b.y) + a.z·b.z`, in that order.
pub fn dot3(a: V4, b: V4) -> f32 {
    let (a, b) = (lanes(a), lanes(b));
    (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]).0
}

/// Normalise `xyz` (`w` becomes 0) the way a VU0 routine does it: `Q = sqrt(len²)`, `Q = 1/Q`, `v·Q`.
pub fn normalize(v: V4) -> V4 {
    let l = Lane(sqrt(dot3(v, v)));
    let q = Lane(div(1.0, l.0));
    let v = lanes(v);
    v4([v[0] * q, v[1] * q, v[2] * q, Lane::ZERO])
}

/// `vopmula` + `vopmsub`: `a × b` with `w = 0`.
pub fn cross(a: V4, b: V4) -> V4 {
    let (va, vb) = (Vf(a), Vf(b));
    let mut r = Vf::opmsub(Vf::opmula(va, vb), vb, va);
    r.0[3] = 0.0;
    r.0
}

/// Row-vector transform, `vmulax` / `vmadday` / `vmaddaz` / `vmaddw`:
/// `((r0·v.x + r1·v.y) + r2·v.z) + r3·v.w` per component.
pub fn transform(m: &[V4; 4], v: V4) -> V4 {
    let (m, v) = (m.map(lanes), lanes(v));
    v4(core::array::from_fn(|k| (m[0][k] * v[0]).madd(m[1][k], v[1]).madd(m[2][k], v[2]).madd(m[3][k], v[3])))
}

/// `vmulax` then `vmaddx`: `a·t + b·(1 − t)` per component.
pub fn lerp(a: V4, b: V4, t: f32) -> V4 {
    let (a, b, t) = (lanes(a), lanes(b), Lane(t));
    let u = Lane::ZERO + 1.0 - t;
    v4(core::array::from_fn(|k| (a[k] * t).madd(b[k], u)))
}

// --- the ops this game actually uses, and VU0's flags ----------------------------------------------
//
// Everything above is the arithmetic; everything below is the rest of macro mode — the field-masked
// write, the accumulator, `Q`, and the three flag registers the EE reads back with `cfc2`.
//
// The op list is not the ISA's, it is this game's: every COP2 word in the boot ELF that lies inside a
// decompiled function, decoded and counted. 8,406 of them, of which 724 are the `qmtc2`/`qmfc2`/`cfc2`/
// `ctc2` transfers in and out and **7,682 are VU0 arithmetic**. The counts are instructions, not call
// sites, and the broadcast forms are folded into their base op. In descending order:
//
// | op | n | | op | n |
// |---|---|---|---|---|
// | `vmadda`/`vmaddax..w` | 1 441 | | `vmul`/`vmulx..w` | 1 057 |
// | `vmadd`/`vmaddx..w` | 717 | | `vadd`/`vaddx..w` | 695 |
// | `vsub`/`vsubx..w` | 645 | | `vopmula` + `vopmsub` | 932 |
// | `vadda`/`vaddax..w` | 432 | | `vmula`/`vmulax..w` | 316 |
// | `vwaitq` | 300 | | `vmulq` | 293 |
// | `vrsqrt` | 247 | | `vmax`/`vmaxx..w` | 238 |
// | `vmsuba`/`vmsubax..w` | 68 | | `vmove` | 63 |
// | `vdiv` | 48 | | `vnop` | 47 |
// | `vmsub`/`vmsubx..w` | 42 | | `vabs` | 22 |
// | `vclipw` | 21 | | `vmr32` | 18 |
// | `vsuba`/`vsubax..w` | 11 | | `vmini`/`vminix..w` | 7 |
// | `vaddq` | 7 | | `vitof0` | 6 |
// | `vsqrt` | 6 | | `vcallmsr`/`vlqi`/`vsqi` | 1 each |
//
// "Inside a function" is what makes that list trustworthy: the image's string and constant data holds a
// few hundred words whose top six bits happen to say COP2, and decoding those as instructions is how a
// histogram acquires ops the game does not have. Restricted to code, **every** COP2 word decodes, which
// is the check that the list is complete rather than merely long.
//
// What that list says about the port:
//
// * The I register (`vaddi`, `vmuli`, `vmaxi`, `vminii`) is **never used**, so there is no `I` here.
//   `R` (`vrnext`, `vrxor`) is unused, and so is the integer unit — no `viadd`, no `visub`. `vftoi` in
//   any form is absent too; only `vitof0` appears, unpacking small integers.
// * The three singletons are SDK leaf routines, not maths: `ctc2 $vi27` + `vcallmsr` is "start the
//   micro-program at this address", and the `vlqi`/`vsqi` pair is one quadword in and out of VU0's data
//   memory. Micro mode is therefore *reachable* in this game even though macro mode is all the maths —
//   see "What is modelled, and what is not" above.
// * `vmadd`, `vmadda` and the broadcast forms carry the bulk of the maths: matrix transforms and dot
//   products. Their accumulate order is the first thing to get right in a port, which is why
//   [`Vu0::madda`] and friends take the destination mask rather than assuming `xyzw`.
// * `vclipw` plus `cfc2 $vi18` is how the game culls: four points, six bits each, ANDed and ORed.
// * `cfc2 $vi17` — the **MAC flag** — is read for real control flow, not for diagnostics: after a
//   `.w`-masked dot product the game tests bit 4 (`Sw`) to ask "is this behind the plane?", and after a
//   four-lane transform it tests `& 0xf0 == 0xf0` for "all four negative". A port that does not model
//   these flags cannot reproduce those branches, which is why they are here rather than in the "not
//   modelled" list.
// * Nothing reads the status flag outside VU0's context-save library routine, but it is cheap to keep
//   and it is where `vdiv`'s divide-by-zero and invalid bits live.

/// 2^128: the first magnitude that does not fit the VU's 8-bit exponent, so the overflow flag's
/// threshold. Chopping cannot lower an exponent, so a result is clamped exactly when it reaches this.
const OVERFLOW_AT: f64 = f64::from_bits(0x47f0_0000_0000_0000);
/// 2^-126, the smallest VU normal: a nonzero result below this underflows to zero.
const UNDERFLOW_AT: f64 = f64::from_bits(0x3810_0000_0000_0000);

/// The MAC flags of one lane, as the hardware derives them — from the result, not from the operands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct LaneFlags {
    z: bool,
    s: bool,
    u: bool,
    o: bool,
}

/// Judge one lane's result. `exact` is the unrounded, unclamped value; `r` is what was written.
fn classify(exact: f64, r: f32) -> LaneFlags {
    LaneFlags {
        z: r == 0.0,
        s: r.is_sign_negative(),
        u: exact != 0.0 && exact.abs() < UNDERFLOW_AT,
        o: exact.abs() >= OVERFLOW_AT,
    }
}

/// The MAC flag register, read with `cfc2 $vi17`: the Zero, Sign, Underflow and Overflow of each lane
/// of the last arithmetic op. Its layout is the hardware's, so a ported `andi` mask reads unchanged:
///
/// ```text
///   bit  15 14 13 12 | 11 10  9  8 |  7  6  5  4 |  3  2  1  0
///        Ox Oy Oz Ow | Ux Uy Uz Uw | Sx Sy Sz Sw | Zx Zy Zz Zw
/// ```
///
/// Lanes the destination mask leaves alone have their four flags **cleared**, not kept — so after a
/// `.w` op only `Zw`/`Sw`/`Uw`/`Ow` can be set, which is what makes the game's `andi $t, 0x10` test for
/// "the `w` lane came out negative" work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mac(pub u16);

impl Mac {
    /// The result was zero (either sign).
    pub fn zero(self, f: Field) -> bool {
        self.bit(3, f)
    }
    /// The result's sign bit was set — which includes the `-0.0` an underflow leaves behind.
    pub fn sign(self, f: Field) -> bool {
        self.bit(7, f)
    }
    /// The result was too small to be a VU normal and became zero.
    pub fn underflow(self, f: Field) -> bool {
        self.bit(11, f)
    }
    /// The result was clamped to `±max`.
    pub fn overflow(self, f: Field) -> bool {
        self.bit(15, f)
    }
    /// The four sign bits, in place: the game reads them as `cfc2 $t, $vi17` then `andi $t, 0xf0`.
    pub fn signs(self) -> u16 {
        self.0 & 0x00f0
    }
    /// The four zero bits, in place (`andi $t, 0x0f`).
    pub fn zeros(self) -> u16 {
        self.0 & 0x000f
    }

    fn bit(self, top: u32, f: Field) -> bool {
        self.0 >> (top - f as u32) & 1 == 1
    }
    fn set(&mut self, i: usize, l: LaneFlags) {
        let b = 3 - i as u32;
        self.0 |= u16::from(l.z) << b | u16::from(l.s) << (b + 4) | u16::from(l.u) << (b + 8)
            | u16::from(l.o) << (b + 12);
    }
}

/// The status flag register, read with `cfc2 $vi16`: the OR of the MAC flags for the last op, the two
/// divider flags, and a sticky copy of each that stays set until the register is written.
///
/// ```text
///   bit  11 10  9  8  7  6 |  5  4 |  3  2  1  0
///        DS IS OS US SS ZS |  D  I |  O  U  S  Z
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Status(pub u16);

impl Status {
    /// Some lane of the last arithmetic op came out zero.
    pub fn zero(self) -> bool {
        self.0 & 1 != 0
    }
    /// Some lane came out negative.
    pub fn sign(self) -> bool {
        self.0 & 2 != 0
    }
    /// Some lane underflowed to zero.
    pub fn underflow(self) -> bool {
        self.0 & 4 != 0
    }
    /// Some lane was clamped to `±max`.
    pub fn overflow(self) -> bool {
        self.0 & 8 != 0
    }
    /// The last divider op was invalid: `0/0`, or a square root of a negative.
    pub fn invalid(self) -> bool {
        self.0 & 0x10 != 0
    }
    /// The last divider op divided a nonzero value by zero.
    pub fn div_by_zero(self) -> bool {
        self.0 & 0x20 != 0
    }
    /// The six sticky bits (11..6), which only a write to the register clears.
    pub fn sticky(self) -> u16 {
        self.0 & 0x0fc0
    }

    /// An arithmetic op: bits 3..0 are the OR of this op's MAC lanes, and stick into 9..6.
    fn update(&mut self, mac: Mac) {
        let f = |m: u16| u16::from(mac.0 & m != 0);
        let now = f(0x000f) | f(0x00f0) << 1 | f(0x0f00) << 2 | f(0xf000) << 3;
        self.0 = (self.0 & 0x0ff0) | now | now << 6;
    }
    /// A divider op: it rewrites bits 5..4 every time (and sticks them into 11..10), and leaves the
    /// arithmetic bits alone.
    fn divider(&mut self, invalid: bool, dz: bool) {
        let now = u16::from(invalid) << 4 | u16::from(dz) << 5;
        self.0 = (self.0 & 0x0fcf) | now | now << 6;
    }
}

/// The clipping flag register, read with `cfc2 $vi18`: a 24-bit shift register of the last four
/// `vclipw` judgements, six bits each, newest in bits 5..0.
///
/// The six bits of one judgement compare `fs`'s `x`, `y`, `z` against `±|ft.w|` — the frustum test a
/// perspective divide would otherwise need:
///
/// ```text
///   bit 0: x > +w   bit 2: y > +w   bit 4: z > +w
///   bit 1: x < -w   bit 3: y < -w   bit 5: z < -w
/// ```
///
/// The game clips four points per primitive, masks each judgement with `andi 0x3f`, and then ANDs the
/// four together (all four outside the same plane ⇒ cull the primitive) and ORs them (nothing set ⇒
/// wholly inside, no clipping needed).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Clip(pub u32);

impl Clip {
    /// The newest judgement's six bits (`andi $t, 0x3f`).
    pub fn last(self) -> u8 {
        (self.0 & 0x3f) as u8
    }
    /// Was any of `x y z` outside `±|w|` in the newest judgement?
    pub fn outside(self) -> bool {
        self.last() != 0
    }
    /// `lane > +|w|` in the newest judgement. `w` itself is not judged.
    pub fn above(self, f: Field) -> bool {
        f != Field::W && self.0 >> (2 * f as u32) & 1 == 1
    }
    /// `lane < -|w|` in the newest judgement.
    pub fn below(self, f: Field) -> bool {
        f != Field::W && self.0 >> (2 * f as u32 + 1) & 1 == 1
    }
}

/// One `vclipw` judgement: the six bits for `fs.xyz` against `±|w|`, in the register's own bit order.
pub fn clip_bits(fs: Vf, w: f32) -> u8 {
    let w = f32::from_bits(vu(w).to_bits() & 0x7fff_ffff);
    let neg = f32::from_bits(w.to_bits() | 0x8000_0000);
    let mut b = 0;
    for i in 0..3 {
        let v = vu(fs.0[i]);
        b |= u8::from(v > w) << (2 * i) | u8::from(v < neg) << (2 * i + 1);
    }
    b
}

/// VU0 in macro mode: the state an instruction sequence carries between its instructions — the
/// accumulator, the `Q` register, and the three flag registers.
///
/// One method per instruction, named after it and taking the destination mask first, so a ported
/// routine lines up with its listing. The broadcast forms are the plain op with [`Vf::bc`] on `ft`:
///
/// ```
/// use ps2_float::{Dest, Field, Vf, vu::Vu0};
/// let (mut vu, mut vf1, mut vf2) = (Vu0::default(), Vf::new(3.0, 4.0, 0.0, 1.0), Vf::ZERO);
/// // the game's normalise, instruction for instruction:
/// vf2 = vu.mul(Dest::XYZ, vf2, vf1, vf1);                     // vmul.xyz   vf2, vf1, vf1
/// let vf3 = vu.max(Dest::XYZW, Vf::ZERO, Vf::VF0, Vf::VF0.bc(Field::W)); // vmaxw.xyzw vf3, vf0, vf0w
/// vu.adda(Dest::X, vf2, vf2.bc(Field::Y));                    // vadday.x   ACC, vf2, vf2y
/// vf2 = vu.madd(Dest::X, vf2, vf3, vf2.bc(Field::Z));         // vmaddz.x   vf2, vf3, vf2z
/// vu.rsqrt(Vf::VF0, Field::W, vf2, Field::X);                 // vrsqrt     Q, vf0w, vf2x
/// vf1 = vu.mulq(Dest::XYZ, vf1, vf1);                         // vmulq.xyz  vf1, vf1, Q
/// assert_eq!(vf1.0, [0.59999996, 0.79999995, 0.0, 1.0]);
/// ```
///
/// `vnop` and `vwaitq` have no method: this model has no pipeline, so they do nothing. Where the
/// original has them (the game puts four `vnop`s before every `cfc2`, and a `vwaitq` before every use
/// of `Q`) a comment is enough. `vftoi`/`vitof` are on [`Vf`] rather than here, because they set no
/// flags; so do `vmax`, `vmini`, `vabs`, `vmove` and `vmr32`, which are kept here only so that a
/// transcribed listing can stay one method per instruction.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vu0 {
    /// The accumulator `ACC`, written by the `vmula`/`vadda`/`vmadda`/`vopmula` family.
    pub acc: Vf,
    /// The `Q` register, written by `vdiv`/`vsqrt`/`vrsqrt` and read by the `.q` forms.
    pub q: f32,
    /// `cfc2 $vi17`.
    pub mac: Mac,
    /// `cfc2 $vi16`.
    pub status: Status,
    /// `cfc2 $vi18`.
    pub clip: Clip,
}

/// One lane plus the exact, unrounded result its MAC flags are judged from.
type Lane2 = (f32, f64);

/// `vadd` on one lane, with the exact sum. Two `f32`s always add exactly in `f64` *when the sum is
/// near a flag boundary* — at that point the operands' exponents are within one of each other — which
/// is all the overflow and underflow tests need.
#[allow(clippy::float_arithmetic)] // the model itself, in host arithmetic
fn add_e(a: f32, b: f32) -> Lane2 {
    (add(a, b), vu(a) as f64 + vu(b) as f64)
}

/// `vsub` on one lane, with the exact difference.
fn sub_e(a: f32, b: f32) -> Lane2 {
    add_e(a, f32::from_bits(b.to_bits() ^ 0x8000_0000))
}

/// `vmul` on one lane, with the exact product (24×24 bits, always exact in `f64`).
#[allow(clippy::float_arithmetic)] // the model itself, in exact host arithmetic
fn mul_e(a: f32, b: f32) -> Lane2 {
    (mul(a, b), vu(a) as f64 * vu(b) as f64)
}

/// `vmadd` on one lane: the product is chopped into a VU float first, so the flags describe the add.
fn madd_e(acc: f32, a: f32, b: f32) -> Lane2 {
    add_e(acc, mul(a, b))
}

/// `vmsub` on one lane.
fn msub_e(acc: f32, a: f32, b: f32) -> Lane2 {
    sub_e(acc, mul(a, b))
}

impl Vu0 {
    /// Write a lane-wise result under the destination mask and set the flags: the lanes outside the
    /// mask keep their value and lose their flags. Every arithmetic op goes through here.
    fn write(&mut self, dest: Dest, fd: Vf, r: [Lane2; 4]) -> Vf {
        let mut mac = Mac(0);
        let mut out = fd;
        for (i, &(v, exact)) in r.iter().enumerate() {
            if dest.has(i) {
                out.0[i] = v;
                mac.set(i, classify(exact, v));
            }
        }
        self.mac = mac;
        self.status.update(mac);
        out
    }

    /// `vadd.d fd, fs, ft` — and with [`Vf::bc`] on `ft`, `vaddx`/`vaddy`/`vaddz`/`vaddw`.
    pub fn add(&mut self, dest: Dest, fd: Vf, fs: Vf, ft: Vf) -> Vf {
        self.write(dest, fd, core::array::from_fn(|i| add_e(fs.0[i], ft.0[i])))
    }
    /// `vsub.d fd, fs, ft`.
    pub fn sub(&mut self, dest: Dest, fd: Vf, fs: Vf, ft: Vf) -> Vf {
        self.write(dest, fd, core::array::from_fn(|i| sub_e(fs.0[i], ft.0[i])))
    }
    /// `vmul.d fd, fs, ft`.
    pub fn mul(&mut self, dest: Dest, fd: Vf, fs: Vf, ft: Vf) -> Vf {
        self.write(dest, fd, core::array::from_fn(|i| mul_e(fs.0[i], ft.0[i])))
    }
    /// `vmadd.d fd, fs, ft`: `ACC + fs·ft`, the accumulator unchanged.
    pub fn madd(&mut self, dest: Dest, fd: Vf, fs: Vf, ft: Vf) -> Vf {
        let acc = self.acc;
        self.write(dest, fd, core::array::from_fn(|i| madd_e(acc.0[i], fs.0[i], ft.0[i])))
    }
    /// `vmsub.d fd, fs, ft`: `ACC − fs·ft`.
    pub fn msub(&mut self, dest: Dest, fd: Vf, fs: Vf, ft: Vf) -> Vf {
        let acc = self.acc;
        self.write(dest, fd, core::array::from_fn(|i| msub_e(acc.0[i], fs.0[i], ft.0[i])))
    }

    /// `vaddq.d fd, fs, Q`.
    pub fn addq(&mut self, dest: Dest, fd: Vf, fs: Vf) -> Vf {
        let q = self.q;
        self.write(dest, fd, core::array::from_fn(|i| add_e(fs.0[i], q)))
    }
    /// `vsubq.d fd, fs, Q`.
    pub fn subq(&mut self, dest: Dest, fd: Vf, fs: Vf) -> Vf {
        let q = self.q;
        self.write(dest, fd, core::array::from_fn(|i| sub_e(fs.0[i], q)))
    }
    /// `vmulq.d fd, fs, Q` — 307 of them in this game, one per normalise.
    pub fn mulq(&mut self, dest: Dest, fd: Vf, fs: Vf) -> Vf {
        let q = self.q;
        self.write(dest, fd, core::array::from_fn(|i| mul_e(fs.0[i], q)))
    }
    /// `vmaddq.d fd, fs, Q`.
    pub fn maddq(&mut self, dest: Dest, fd: Vf, fs: Vf) -> Vf {
        let (acc, q) = (self.acc, self.q);
        self.write(dest, fd, core::array::from_fn(|i| madd_e(acc.0[i], fs.0[i], q)))
    }
    /// `vmsubq.d fd, fs, Q`.
    pub fn msubq(&mut self, dest: Dest, fd: Vf, fs: Vf) -> Vf {
        let (acc, q) = (self.acc, self.q);
        self.write(dest, fd, core::array::from_fn(|i| msub_e(acc.0[i], fs.0[i], q)))
    }

    /// The accumulator forms write `ACC` instead of a register and set the same flags. `vadda.d`.
    pub fn adda(&mut self, dest: Dest, fs: Vf, ft: Vf) -> Vf {
        let acc = self.acc;
        self.acc = self.write(dest, acc, core::array::from_fn(|i| add_e(fs.0[i], ft.0[i])));
        self.acc
    }
    /// `vsuba.d`.
    pub fn suba(&mut self, dest: Dest, fs: Vf, ft: Vf) -> Vf {
        let acc = self.acc;
        self.acc = self.write(dest, acc, core::array::from_fn(|i| sub_e(fs.0[i], ft.0[i])));
        self.acc
    }
    /// `vmula.d` — the first row of a matrix transform, as `vmulax` with [`Vf::bc`].
    pub fn mula(&mut self, dest: Dest, fs: Vf, ft: Vf) -> Vf {
        let acc = self.acc;
        self.acc = self.write(dest, acc, core::array::from_fn(|i| mul_e(fs.0[i], ft.0[i])));
        self.acc
    }
    /// `vmadda.d`: `ACC += fs·ft`, the rows in between.
    pub fn madda(&mut self, dest: Dest, fs: Vf, ft: Vf) -> Vf {
        let acc = self.acc;
        self.acc = self.write(dest, acc, core::array::from_fn(|i| madd_e(acc.0[i], fs.0[i], ft.0[i])));
        self.acc
    }
    /// `vmsuba.d`: `ACC −= fs·ft`.
    pub fn msuba(&mut self, dest: Dest, fs: Vf, ft: Vf) -> Vf {
        let acc = self.acc;
        self.acc = self.write(dest, acc, core::array::from_fn(|i| msub_e(acc.0[i], fs.0[i], ft.0[i])));
        self.acc
    }

    /// `vopmula.xyz ACC, fs, ft`: the first half of a cross product (`w` is outside the mask, so it
    /// keeps the accumulator's old `w` — and loses its flags).
    pub fn opmula(&mut self, fs: Vf, ft: Vf) -> Vf {
        let acc = self.acc;
        let r = [mul_e(fs.0[1], ft.0[2]), mul_e(fs.0[2], ft.0[0]), mul_e(fs.0[0], ft.0[1]), (0.0, 0.0)];
        self.acc = self.write(Dest::XYZ, acc, r);
        self.acc
    }
    /// `vopmsub.xyz fd, fs, ft`: the second half. The game emits the pair with the operands swapped
    /// between the two instructions — see [`Vf::opmsub`] — so port it as the listing has it.
    pub fn opmsub(&mut self, dest: Dest, fd: Vf, fs: Vf, ft: Vf) -> Vf {
        let acc = self.acc;
        let r = [
            msub_e(acc.0[0], fs.0[1], ft.0[2]),
            msub_e(acc.0[1], fs.0[2], ft.0[0]),
            msub_e(acc.0[2], fs.0[0], ft.0[1]),
            (acc.0[3], acc.0[3] as f64),
        ];
        self.write(dest, fd, r)
    }

    /// `vmax.d fd, fs, ft` (and `vmaxx`…`vmaxw` with [`Vf::bc`]): a compare and a select, so nothing
    /// rounds and **the MAC flags are left alone**. The game's way of materialising a constant 1.0 in
    /// all four lanes is `vmaxw.xyzw fd, vf0, vf0w`.
    pub fn max(&mut self, dest: Dest, fd: Vf, fs: Vf, ft: Vf) -> Vf {
        fd.merge(dest, Vf(core::array::from_fn(|i| max(fs.0[i], ft.0[i]))))
    }
    /// `vmini.d fd, fs, ft`: the same, taking the smaller.
    pub fn mini(&mut self, dest: Dest, fd: Vf, fs: Vf, ft: Vf) -> Vf {
        fd.merge(dest, Vf(core::array::from_fn(|i| mini(fs.0[i], ft.0[i]))))
    }
    /// `vabs.d fd, fs`: clears the sign bit; no flags.
    pub fn abs(&mut self, dest: Dest, fd: Vf, fs: Vf) -> Vf {
        fd.merge(dest, fs.abs())
    }
    /// `vmove.d fd, fs`: a masked copy; no flags.
    pub fn mov(&mut self, dest: Dest, fd: Vf, fs: Vf) -> Vf {
        fd.merge(dest, fs)
    }
    /// `vmr32.d fd, fs`: a masked copy of `fs` rotated one lane left; no flags.
    pub fn mr32(&mut self, dest: Dest, fd: Vf, fs: Vf) -> Vf {
        fd.merge(dest, fs.mr32())
    }

    /// `vclipw fs, ftw`: shifts six fresh judgement bits into the clipping flag. No other flag moves,
    /// and no register is written.
    pub fn clipw(&mut self, fs: Vf, ft: Vf) {
        self.clip = Clip((self.clip.0 << 6 | u32::from(clip_bits(fs, ft.0[3]))) & 0x00ff_ffff);
    }

    /// `vdiv Q, fs[fsf], ft[ftf]`. Dividing a nonzero value by zero sets `D`; `0/0` sets `I`; either
    /// way `Q` is `±max` with the sign of the two operands XORed.
    pub fn div(&mut self, fs: Vf, fsf: Field, ft: Vf, ftf: Field) -> f32 {
        let (a, b) = (vu(fs.lane(fsf)), vu(ft.lane(ftf)));
        self.status.divider(b == 0.0 && a == 0.0, b == 0.0 && a != 0.0);
        self.q = div(a, b);
        self.q
    }
    /// `vsqrt Q, ft[ftf]`: the root of `|ft|`, and `I` when `ft` was negative.
    pub fn sqrt(&mut self, ft: Vf, ftf: Field) -> f32 {
        let b = vu(ft.lane(ftf));
        self.status.divider(b < 0.0, false);
        self.q = sqrt(b);
        self.q
    }
    /// `vrsqrt Q, fs[fsf], ft[ftf]`: `fs / sqrt(|ft|)`, the 256-times-over idiom of this game. `ft` of
    /// zero sets `D`, `ft` negative sets `I`.
    pub fn rsqrt(&mut self, fs: Vf, fsf: Field, ft: Vf, ftf: Field) -> f32 {
        let (a, b) = (vu(fs.lane(fsf)), vu(ft.lane(ftf)));
        self.status.divider(b < 0.0, b == 0.0);
        self.q = if b == 0.0 { div(a, b) } else { rsqrt(a, b) }; // ±max, by the two signs XORed
        self.q
    }
}

/// `vmax` on one lane: a compare and a select — no rounding, no flags, and the operands are read the
/// way a VU reads them ([`operand`]), so an `Inf` pattern compares as `±max` and a denormal as zero.
pub fn max(a: f32, b: f32) -> f32 {
    let (a, b) = (vu(a), vu(b));
    if a >= b { a } else { b }
}

/// `vmini` on one lane.
pub fn mini(a: f32, b: f32) -> f32 {
    let (a, b) = (vu(a), vu(b));
    if a <= b { a } else { b }
}

/// `vabs` on one lane: the sign bit cleared, nothing else touched.
pub fn abs(x: f32) -> f32 {
    f32::from_bits(vu(x).to_bits() & 0x7fff_ffff)
}

impl Vf {
    /// `vmax`, lane by lane. Pair with [`Vf::bc`] for `vmaxx`…`vmaxw`.
    pub fn max(self, o: Vf) -> Vf {
        Vf(core::array::from_fn(|i| max(self.0[i], o.0[i])))
    }
    /// `vmini`, lane by lane.
    pub fn mini(self, o: Vf) -> Vf {
        Vf(core::array::from_fn(|i| mini(self.0[i], o.0[i])))
    }
    /// `vabs`, lane by lane.
    pub fn abs(self) -> Vf {
        Vf(self.0.map(abs))
    }
    /// `vmr32`: every lane one place to the left, `x` wrapping into `w` (`fd.x = fs.y`, `fd.w = fs.x`).
    pub fn mr32(self) -> Vf {
        Vf([self.0[1], self.0[2], self.0[3], self.0[0]])
    }
}

/// `vabs.xyz` then `vmaxy.x` then `vmaxz.x`: the longest of `|x|`, `|y|`, `|z|` — the cheap "how far
/// away is this" the game uses where a real length would need a square root.
pub fn max_abs3(v: V4) -> f32 {
    let a = Vf(v).abs();
    max(max(a.0[0], a.0[1]), a.0[2])
}
