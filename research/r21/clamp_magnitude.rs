// R21: the LLVM 23 miscompile F17 hit, as a Rust program. Wrong at -O, right without.
//
//   rustc -O research/r21/clamp_magnitude.rs -o /tmp/r21 && /tmp/r21   # panics on rustc 1.99.1
//   rustc    research/r21/clamp_magnitude.rs -o /tmp/r21 && /tmp/r21   # passes
//
// The operand's sign bit is known to be set, so SelectionDAG's known bits for the `fabs` say the sign
// bit is both 0 and 1 (`makeNonNegative` sets Known.Zero's bit and leaves Known.One's), and the `umin`
// folds to its constant. Fixed upstream by llvm/llvm-project 6bb86c2af2ba (#227457).
#[inline(never)]
fn clamp_magnitude(x: u32) -> u32 {
    let neg = f32::from_bits(x | 0x8000_0000); // sign bit known set
    neg.abs().to_bits().min(0x7f7f_ffff) // |neg| as bits, clamped to f32::MAX's
}

fn main() {
    let got = clamp_magnitude(std::hint::black_box(1.0f32.to_bits()));
    println!("{got:#010x} (want 0x3f800000)");
    assert_eq!(got, 0x3f80_0000);
}
