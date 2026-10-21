//! VU0's model, and above all where it differs from the FPU's — because a port that uses one where the
//! original used the other is wrong by a few ulps per operation, which is the hardest class of bug to
//! find later.

use ps2_float::vu::{self, Dest, Field, Lane, Vf, lanes, v4};
use ps2_float::{F, f, fpu};

#[test]
fn the_vu_adder_is_full_width_where_the_fpus_is_not() {
    // The FPU's alignment throws the addend away; VU0 keeps it and then chops the sum.
    let tiny = f32::from_bits(0x3340_0000); // 1.5·2^-25, 25 exponents below 1.0
    assert_eq!(fpu::sub(1.0, tiny), 1.0, "the FPU cannot see it");
    assert_eq!(vu::sub(1.0, tiny), 0.99999994, "VU0 can");
    assert_eq!(vu::add(1.0, tiny), 1.0, "but still chops the sum toward zero");

    // Far apart enough that the exact sum does not fit in an f64 at all, which is why the model uses a
    // TwoSum: the direction of the truncation still has to be right.
    let far = f32::from_bits(0x0080_0000); // 2^-126
    assert_eq!(vu::add(f32::MAX, far), f32::MAX);
    assert_eq!(vu::sub(1.0, far), 0.99999994, "a tiny negative addend still moves 1.0 down one ulp");
    assert_eq!(vu::sub(f32::MAX, far), f32::from_bits(0x7f7f_fffe));
}

#[test]
fn vu_arithmetic_chops_and_clamps() {
    // 0.1·0.1 is exactly 0.01f32 on a VU, one ulp below the correctly-rounded product
    assert_eq!(vu::mul(0.1, 0.1), 0.01f32);
    assert_ne!(vu::mul(0.1, 0.1), 0.1f32 * 0.1f32, "the host rounds to nearest");
    assert_eq!(vu::mul(f32::MAX, 2.0), vu::MAX, "clamp instead of an infinity");
    assert_eq!(vu::mul(-f32::MAX, 2.0), -vu::MAX);
    assert_eq!(vu::div(1.0, 0.0), vu::MAX);
    assert_eq!(vu::div(-1.0, 0.0), -vu::MAX);
    assert_eq!(vu::div(1.0, 3.0), 0.3333333, "Q is chopped too, unlike the FPU's div.s");
    assert_eq!(fpu::div(1.0, 3.0), 0.33333334, "— which is the FPU's answer, one ulp up");
    assert_eq!(vu::sqrt(-4.0), 2.0, "vsqrt takes |x|");
    // √5 rounds up to nearest (0x400f1bbd); PCSX2's VU0 chops it, which S56's wrecks needed
    assert_eq!(vu::sqrt(5.0).to_bits(), 0x400f_1bbc, "vsqrt is chopped");
    assert_eq!(fpu::sqrt(5.0).to_bits(), 0x400f_1bbd, "— the FPU's sqrt.s is not");
    assert_eq!(vu::sqrt(2.0).to_bits(), 0x3fb5_04f3, "a root that rounded down is left alone");
    assert_eq!(vu::rsqrt(1.0, 4.0), 0.5);
    assert_eq!(vu::operand(0x7f80_0000), vu::MAX, "an Inf pattern is just a big number");
    assert_eq!(vu::operand(0x0040_0000), 0.0, "denormals are zero here too");
    assert_eq!(vu::madd(1.0, 1.0 / 3.0, 3.0), vu::add(1.0, vu::mul(1.0 / 3.0, 3.0)), "vmadd is not fused");
}

#[test]
fn field_masks_leave_the_other_lanes_alone() {
    let old = Vf::new(1.0, 2.0, 3.0, 4.0);
    let new = Vf::splat(9.0);
    assert_eq!(old.merge(Dest::XYZ, new).0, [9.0, 9.0, 9.0, 4.0]);
    assert_eq!(old.merge(Dest::W, new).0, [1.0, 2.0, 3.0, 9.0]);
    assert_eq!(old.merge(Dest::XZ, new).0, [9.0, 2.0, 9.0, 4.0]);
    assert_eq!(old.merge(Dest::XYZW, new).0, [9.0; 4]);
    assert_eq!(old.merge(Dest(0), new).0, old.0);
    assert_eq!(Dest::XYZ.name(), "xyz");
    assert_eq!(Dest::W.name(), "w");
    assert_eq!(Dest::XZ.name(), "xz");
    assert!(Dest::YZW.has(3) && !Dest::YZW.has(0));
    // a broadcast operand (vmulx, vaddw, …) is one lane in all four
    assert_eq!(old.bc(Field::Y).0, [2.0; 4]);
    assert_eq!(old.lane(Field::W), 4.0);
}

#[test]
fn the_cross_product_idiom_matches_its_two_instructions() {
    let (a, b) = ([1.0, 2.0, 3.0, 0.0], [-4.0, 5.0, -6.0, 0.0]);
    assert_eq!(vu::cross(a, b), [-27.0, -6.0, 13.0, 0.0]);
    // opmula/opmsub share one lane pattern; the cross product comes from the swapped operands
    let acc = Vf::opmula(Vf(a), Vf(b));
    assert_eq!(acc.0, [vu::mul(2.0, -6.0), vu::mul(3.0, -4.0), vu::mul(1.0, 5.0), 0.0]);
    assert_eq!(Vf::opmsub(acc, Vf(b), Vf(a)).0[..3], vu::cross(a, b)[..3]);
}

#[test]
fn fixed_point_conversions_truncate_and_saturate() {
    let v = Vf::new(1.5, -1.5, 0.25, -0.25);
    assert_eq!(v.ftoi(0), [1, -1, 0, 0], "vftoi0 truncates toward zero");
    assert_eq!(v.ftoi(4), [24, -24, 4, -4], "vftoi4 is 12.4 fixed point");
    assert_eq!(v.ftoi(12), [6144, -6144, 1024, -1024]);
    assert_eq!(Vf::splat(1.0e30).ftoi(12), [i32::MAX; 4], "saturates, never wraps");
    assert_eq!(Vf::splat(-1.0e30).ftoi(12), [i32::MIN; 4]);
    assert_eq!(Vf::itof([6144, -1024, 1, 0], 12).0, [1.5, -0.25, 0.00024414062, 0.0]);
    assert_eq!(Vf::itof(v.ftoi(12), 12).0, v.0, "round trip at 12 fractional bits");
}

#[test]
fn the_lane_wrapper_reads_like_a_vu_listing() {
    let (a, b) = (Lane(1.0), Lane(1.0 / 3.0));
    assert_eq!((a + b * 3.0).0, vu::add(1.0, vu::mul(1.0 / 3.0, 3.0)));
    assert_eq!(a.madd(b, Lane(3.0)).0, vu::madd(1.0, 1.0 / 3.0, 3.0));
    assert_eq!(v4(lanes([1.0, 2.0, 3.0, 4.0])), [1.0, 2.0, 3.0, 4.0]);
    // dot3 accumulates x, then y, then z, in that order
    assert_eq!(vu::dot3([1.0, 1.0, 1.0, 0.0], [1.0 / 3.0; 4]), 1.0);
    assert_eq!(vu::dot3([1.0, 2.0, 3.0, 9.0], [4.0, 5.0, 6.0, 9.0]), 32.0, "w is not in it");
    // sqrt, reciprocal and multiply each chop, so a 3-4-5 vector normalises to just *under* 0.6/0.8 —
    // this is the drift a port inherits whether it wants to or not, and it has to inherit it exactly
    assert_eq!(vu::normalize([3.0, 4.0, 0.0, 7.0]), [0.59999996, 0.79999995, 0.0, 0.0]);
    // the whole point of having two models: the same expression, two answers
    let tiny = -f32::from_bits(0x3340_0000); // -1.5·2^-25
    assert_eq!((Lane(1.0) + Lane(tiny)).0, 0.99999994);
    assert_eq!((f(1.0) + F(tiny)).0, 1.0, "the same expression, two answers, one unit apart");
}

#[test]
fn the_transform_idiom_keeps_the_accumulate_order() {
    let m = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [5.0, 6.0, 7.0, 1.0]];
    assert_eq!(vu::transform(&m, [1.0, 2.0, 3.0, 1.0]), [6.0, 8.0, 10.0, 1.0]);
    // a third of each row, accumulated in the instruction order, is not the same as summing it any
    // other way — which is exactly why the idiom is spelled out rather than written with an iterator
    let m3 = [[1.0 / 3.0; 4]; 4];
    assert_eq!(vu::transform(&m3, [1.0; 4])[0], vu::madd(vu::madd(vu::madd(
        vu::mul(1.0 / 3.0, 1.0), 1.0 / 3.0, 1.0), 1.0 / 3.0, 1.0), 1.0 / 3.0, 1.0));
    assert_eq!(vu::lerp([0.0; 4], [1.0; 4], 0.25), [0.75; 4]);
}

// --- macro mode: the accumulator, Q, and the flags the game branches on ----------------------------
//
// The ops tested below are the ones a histogram of the game's own COP2 words turned up; the two MAC
// flag idioms and the clipping loop are transcriptions of real call sites (addresses in the F3
// journal entry, not here — shipped code carries no retail addresses).

use ps2_float::vu::Vu0;

#[test]
fn the_mac_flag_is_what_the_game_branches_on() {
    let mut vu = Vu0::default();

    // After a four-lane transform the game reads `cfc2 $t, $vi17`, masks `0xf0` and compares against
    // `0xf0`: "did all four lanes come out negative?".
    let fd = vu.mul(Dest::XYZW, Vf::ZERO, Vf::splat(-2.0), Vf::splat(3.0));
    assert_eq!(fd.0, [-6.0; 4]);
    assert_eq!(vu.mac.signs(), 0xf0);
    assert!(vu.mac.sign(Field::X) && vu.mac.sign(Field::W));
    assert_eq!(vu.mac.zeros(), 0, "nothing was zero");

    // After a `.w`-masked dot product it masks `0x10` instead — the sign of the `w` lane alone.
    vu.adda(Dest::W, Vf::new(0.0, 0.0, 0.0, 1.0), Vf::new(0.0, 0.0, 0.0, -3.0));
    assert_eq!(vu.acc.w(), -2.0);
    assert_eq!(vu.mac.0, 0x0010, "one lane written, one flag set");
    assert!(vu.mac.sign(Field::W) && !vu.mac.sign(Field::Z));
}

#[test]
fn mac_flags_describe_only_the_lanes_the_mask_wrote() {
    let mut vu = Vu0::default();
    vu.mul(Dest::XYZW, Vf::ZERO, Vf::splat(-1.0), Vf::splat(1.0));
    assert_eq!(vu.mac.signs(), 0xf0);
    // The lanes outside the next mask lose their flags rather than keeping the old ones, which is the
    // only reason a masked test like `andi 0x10` means anything.
    vu.mul(Dest::X, Vf::ZERO, Vf::splat(2.0), Vf::splat(2.0));
    assert_eq!(vu.mac.0, 0, "a positive, nonzero, in-range result sets nothing at all");
}

#[test]
fn mac_flags_mark_a_zero_a_clamp_and_a_flush() {
    let mut vu = Vu0::default();
    vu.mul(Dest::XYZW, Vf::ZERO, Vf::ZERO, Vf::splat(5.0));
    assert_eq!(vu.mac.0, 0x000f, "four zeros");
    assert!(vu.mac.zero(Field::Y) && !vu.mac.sign(Field::Y));

    vu.mul(Dest::W, Vf::ZERO, Vf::splat(-1.0), Vf::ZERO);
    assert_eq!(vu.mac.0, 0x0011, "a negative zero is both Z and S");

    vu.mul(Dest::X, Vf::ZERO, Vf::splat(f32::MAX), Vf::splat(2.0));
    assert_eq!(vu.mac.0, 0x8000, "clamped to +max: the overflow flag, and nothing else");
    assert!(vu.status.overflow());
    let sum = vu.add(Dest::XY, Vf::ZERO, Vf::splat(-f32::MAX), Vf::splat(-f32::MAX));
    assert_eq!(sum.0, [-vu::MAX, -vu::MAX, 0.0, 0.0], "the adder clamps as well");
    assert_eq!(vu.mac.0, 0x00c0 | 0xc000, "two lanes, each negative and each clamped");

    // A result smaller than the smallest VU normal is flushed to zero: Z *and* U.
    vu.mul(Dest::X, Vf::ZERO, Vf::splat(f32::MIN_POSITIVE), Vf::splat(0.5));
    assert_eq!(vu.mac.0, 0x0808);
    assert!(vu.mac.underflow(Field::X) && vu.mac.zero(Field::X));
    assert!(vu.status.underflow() && vu.status.zero());
}

#[test]
fn the_status_flags_last_op_bits_move_and_its_sticky_bits_do_not() {
    let mut vu = Vu0::default();
    vu.mul(Dest::X, Vf::ZERO, Vf::splat(f32::MAX), Vf::splat(2.0));
    assert!(vu.status.overflow() && !vu.status.zero());
    vu.mul(Dest::X, Vf::ZERO, Vf::splat(1.0), Vf::splat(1.0));
    assert!(!vu.status.overflow(), "bits 3..0 describe the last op only");
    assert_eq!(vu.status.sticky() & 0x0200, 0x0200, "the sticky copy stays until the register is written");

    // The divider has its own two bits, and does not disturb the arithmetic ones.
    assert_eq!(vu.div(Vf::splat(1.0), Field::X, Vf::ZERO, Field::X), vu::MAX);
    assert!(vu.status.div_by_zero() && !vu.status.invalid());
    vu.div(Vf::ZERO, Field::X, Vf::ZERO, Field::X);
    assert!(vu.status.invalid() && !vu.status.div_by_zero(), "0/0 is invalid, not a divide by zero");
    assert_eq!(vu.sqrt(Vf::splat(-4.0), Field::X), 2.0);
    assert!(vu.status.invalid(), "the root of a negative is invalid too, and still returns |x|");
    vu.div(Vf::splat(1.0), Field::X, Vf::splat(3.0), Field::X);
    assert_eq!(vu.q, 0.3333333);
    assert!(!vu.status.invalid() && !vu.status.div_by_zero());
    assert_eq!(vu.status.sticky() & 0x0c00, 0x0c00, "both divider stickies remain");
}

#[test]
fn the_clipping_flag_is_a_four_deep_shift_register() {
    let mut vu = Vu0::default();
    let p = Vf::new(3.0, -0.5, 0.0, 2.0);
    vu.clipw(p, p);
    assert_eq!(vu.clip.last(), 0x01, "x is past +w; nothing else is outside");
    assert!(vu.clip.above(Field::X) && !vu.clip.below(Field::X) && vu.clip.outside());
    assert!(!vu.clip.above(Field::W) && !vu.clip.below(Field::W), "w is the bound, not a judged lane");

    // `|w|`: the sign of w cannot flip a judgement.
    assert_eq!(vu::clip_bits(Vf::new(2.0, 0.0, 0.0, 0.0), -1.0), 0x01);
    assert_eq!(vu::clip_bits(Vf::new(0.5, -0.5, 0.5, 0.0), -1.0), 0x00, "inside is six clear bits");
    assert_eq!(vu::clip_bits(Vf::splat(-2.0), 1.0), 0b10_1010, "x, y and z all below −w");

    // Five judgements in, the oldest has fallen off the top of the 24-bit register.
    let mut vu = Vu0::default();
    for p in [
        Vf::new(2.0, 0.0, 0.0, 1.0),  // 0x01, pushed out again by the fifth
        Vf::new(-2.0, 0.0, 0.0, 1.0), // 0x02
        Vf::new(0.0, 2.0, 0.0, 1.0),  // 0x04
        Vf::new(0.0, -2.0, 0.0, 1.0), // 0x08
        Vf::new(0.0, 0.0, 2.0, 1.0),  // 0x10
    ] {
        vu.clipw(p, p);
    }
    assert_eq!(vu.clip.0, 0x02 << 18 | 0x04 << 12 | 0x08 << 6 | 0x10);

    // How the game uses it: four points per primitive, each masked with `andi 0x3f`, ANDed (all four
    // outside the same plane ⇒ cull) and ORed (nothing set ⇒ wholly inside, no clipping needed).
    let quad = [
        Vf::new(3.0, 0.0, 0.0, 2.0),
        Vf::new(4.0, 1.0, 0.0, 2.0),
        Vf::new(5.0, 0.0, 1.0, 2.0),
        Vf::new(6.0, -1.0, 0.0, 2.0),
    ];
    let (mut all, mut any) = (0x3f, 0x00);
    for p in quad {
        vu.clipw(p, p);
        all &= vu.clip.last();
        any |= vu.clip.last();
    }
    assert_eq!(all, 0x01, "every corner is past +w in x: the primitive is culled");
    assert_eq!(any, 0x01);
}

#[test]
fn min_and_max_select_without_rounding_or_flags() {
    assert_eq!(vu::max(1.0, 2.0), 2.0);
    assert_eq!(vu::mini(1.0, 2.0), 1.0);
    assert_eq!(vu::max(-1.0, -2.0), -1.0);
    assert_eq!(vu::abs(-3.0), 3.0);
    // the operands are read the way a VU reads any operand
    assert_eq!(vu::max(f32::from_bits(0x7f80_0000), 1.0), vu::MAX, "an Inf pattern is just a big number");
    assert_eq!(vu::mini(f32::from_bits(0x0040_0000), 1.0), 0.0, "a denormal is zero");
    // a tie between the two zeros: the reference emulator compares as floats and keeps the first
    assert_eq!(vu::max(-0.0, 0.0).to_bits(), 0x8000_0000);

    // The game materialises 1.0 in all four lanes with `vmaxw.xyzw fd, vf0, vf0w`, because vf0 is
    // (0, 0, 0, 1) — and it does so in the middle of a flag-sensitive sequence, which is safe because
    // max is a compare and a select: it sets no flags.
    let mut vu = Vu0::default();
    vu.mul(Dest::XYZW, Vf::ZERO, Vf::splat(-1.0), Vf::splat(1.0));
    let one = vu.max(Dest::XYZW, Vf::ZERO, Vf::VF0, Vf::VF0.bc(Field::W));
    assert_eq!(one.0, [1.0; 4]);
    assert_eq!(vu.mac.signs(), 0xf0, "vmax left the MAC flags where they were");
    assert_eq!(vu.mini(Dest::XZ, Vf::splat(9.0), Vf::splat(1.0), Vf::splat(2.0)).0, [1.0, 9.0, 1.0, 9.0]);

    // `vabs.xyz` then `vmaxy.x` then `vmaxz.x`: the longest axis, without a square root.
    assert_eq!(vu::max_abs3([-3.0, 4.0, -1.0, 9.0]), 4.0);
}

#[test]
fn the_accumulator_carries_a_transform_mask_and_all() {
    let m = [
        Vf::new(1.0, 0.0, 0.0, 0.0),
        Vf::new(0.0, 1.0, 0.0, 0.0),
        Vf::new(0.0, 0.0, 1.0, 0.0),
        Vf::new(5.0, 6.0, 7.0, 1.0),
    ];
    let v = Vf::new(1.0, 2.0, 3.0, 1.0);
    let mut vu = Vu0::default();
    // vmulax.xyzw / vmadday.xyzw / vmaddaz.xyzw / vmaddw.xyzw — the game's transform, 2 000 times over
    vu.mula(Dest::XYZW, m[0], v.bc(Field::X));
    vu.madda(Dest::XYZW, m[1], v.bc(Field::Y));
    vu.madda(Dest::XYZW, m[2], v.bc(Field::Z));
    let out = vu.madd(Dest::XYZW, Vf::ZERO, m[3], v.bc(Field::W));
    assert_eq!(out.0, [6.0, 8.0, 10.0, 1.0]);
    assert_eq!(out.0, vu::transform(&m.map(|r| r.0), v.0), "same answer as the idiom");

    // and the same with a matrix of thirds, where every chop shows: the accumulate order is the thing
    // being tested, not the arithmetic
    let m3 = [Vf::splat(1.0 / 3.0); 4];
    let mut vu = Vu0::default();
    vu.mula(Dest::XYZW, m3[0], Vf::splat(1.0).bc(Field::X));
    vu.madda(Dest::XYZW, m3[1], Vf::splat(1.0).bc(Field::Y));
    vu.madda(Dest::XYZW, m3[2], Vf::splat(1.0).bc(Field::Z));
    let out = vu.madd(Dest::XYZW, Vf::ZERO, m3[3], Vf::splat(1.0).bc(Field::W));
    assert_eq!(out.0, vu::transform(&m3.map(|r| r.0), [1.0; 4]));

    // A mask applies to the accumulator too: the lanes it leaves out keep what they held.
    let mut vu = Vu0::default();
    vu.mula(Dest::XYZW, Vf::splat(2.0), Vf::splat(3.0));
    vu.adda(Dest::X, Vf::splat(1.0), Vf::splat(1.0));
    assert_eq!(vu.acc.0, [2.0, 6.0, 6.0, 6.0]);
}

#[test]
fn the_cross_product_pair_goes_through_the_accumulator() {
    let (a, b) = (Vf::new(1.0, 2.0, 3.0, 0.0), Vf::new(-4.0, 5.0, -6.0, 0.0));
    let mut vu = Vu0::default();
    vu.opmula(a, b); // vopmula.xyz ACC, vf1, vf2
    let r = vu.opmsub(Dest::XYZ, Vf::ZERO, b, a); // vopmsub.xyz vf3, vf2, vf1 — operands swapped
    assert_eq!(r.0, [-27.0, -6.0, 13.0, 0.0]);
    assert_eq!(r.0[..3], vu::cross(a.0, b.0)[..3]);
    assert_eq!(vu.mac.0 & 0x000f, 0, "none of the three products was zero");
}

#[test]
fn a_masked_move_is_how_a_row_gets_its_w() {
    let v = Vf::new(-1.0, 2.0, -3.0, 4.0);
    assert_eq!(v.abs().0, [1.0, 2.0, 3.0, 4.0]);
    assert_eq!(v.mr32().0, [2.0, -3.0, 4.0, -1.0], "one lane left, x wrapping into w");
    let mut vu = Vu0::default();
    let row = Vf::new(1.0, 2.0, 3.0, 9.0);
    // the game builds a rotation matrix's rows and then zeroes each w with `vmr32.w row, vf0`,
    // because vf0.x is 0 — where `vmove.w row, vf0` would have put vf0.w's 1.0 there instead
    assert_eq!(vu.mr32(Dest::W, row, Vf::VF0).0, [1.0, 2.0, 3.0, 0.0]);
    assert_eq!(vu.mov(Dest::W, row, Vf::VF0).0, [1.0, 2.0, 3.0, 1.0]);
    assert_eq!(vu.abs(Dest::XYZ, v, v).0, [1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn the_normalise_the_game_writes_matches_the_idiom() {
    for v in [Vf::new(3.0, 4.0, 0.0, 7.0), Vf::new(0.1, -0.2, 0.3, 1.0), Vf::new(-5.0, 12.0, 84.0, 0.0)] {
        let mut vu = Vu0::default();
        let sq = vu.mul(Dest::XYZ, v, v, v); //            vmul.xyz   vf2, vf1, vf1
        let one = vu.max(Dest::XYZW, Vf::ZERO, Vf::VF0, Vf::VF0.bc(Field::W)); // vmaxw.xyzw vf3, vf0, vf0w
        vu.adda(Dest::X, sq, sq.bc(Field::Y)); //          vadday.x   ACC, vf2, vf2y
        let len2 = vu.madd(Dest::X, sq, one, sq.bc(Field::Z)); // vmaddz.x vf2, vf3, vf2z
        assert_eq!(len2.x(), vu::dot3(v.0, v.0));
        vu.rsqrt(Vf::VF0, Field::W, len2, Field::X); //     vrsqrt     Q, vf0w, vf2x
        //                                                  vwaitq — no pipeline here
        let out = vu.mulq(Dest::XYZ, v, v); //              vmulq.xyz  vf1, vf1, Q
        assert_eq!(out.0[..3], vu::normalize(v.0)[..3]);
        assert_eq!(out.w(), v.w(), "the mask never touched w");
    }
}

#[test]
fn the_q_register_is_written_by_the_divider_and_read_by_the_dot_q_forms() {
    let mut vu = Vu0::default();
    assert_eq!(vu.rsqrt(Vf::VF0, Field::W, Vf::splat(4.0), Field::X), 0.5);
    assert_eq!(vu.q, 0.5);
    assert_eq!(vu.mulq(Dest::XYZ, Vf::ZERO, Vf::splat(8.0)).0, [4.0, 4.0, 4.0, 0.0]);
    assert_eq!(vu.addq(Dest::W, Vf::ZERO, Vf::splat(1.0)).w(), 1.5);
    assert_eq!(vu.subq(Dest::W, Vf::ZERO, Vf::splat(1.0)).w(), 0.5);
    vu.mula(Dest::XYZW, Vf::splat(1.0), Vf::splat(1.0)); // ACC = 1
    assert_eq!(vu.maddq(Dest::X, Vf::ZERO, Vf::splat(2.0)).x(), 2.0, "1 + 2·Q");
    assert_eq!(vu.msubq(Dest::X, Vf::ZERO, Vf::splat(2.0)).x(), 0.0, "1 − 2·Q");
    // rsqrt by zero leaves ±max in Q and says why in the status flag
    assert_eq!(vu.rsqrt(Vf::splat(-1.0), Field::X, Vf::ZERO, Field::X), -vu::MAX);
    assert!(vu.status.div_by_zero());
}
