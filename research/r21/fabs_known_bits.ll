; R21: the LLVM 23 SelectionDAG miscompile behind F17's vu_bits workaround, with no Rust in it.
;
;   llc -O2 research/r21/fabs_known_bits.ll -o -
;
; LLVM 23.1.x folds both functions to their constant bound (`movl $0x7f7fffff, %eax`; the vector one to
; a constant-pool load), on x86-64, AArch64, RISC-V and wasm alike. Right: -O0 (FastISel) and
; -global-isel, which keep the `and`/`cmp`/`cmov`. The fabs operand's sign bit is known set, and
; SelectionDAG::computeKnownBits(ISD::FABS) calls KnownBits::makeNonNegative(), which sets Known.Zero's
; sign bit but leaves Known.One's, so the result's sign bit is "known" both ways and
; KnownBits::umin reads its minimum as >= 0x80000000. Introduced by llvm/llvm-project 11bc6e8776d2
; (#188335, 23.x), fixed on main by 6bb86c2af2ba (#227457), not on release/23.x.
target triple = "x86_64-pc-linux-gnu"

; umin(|x with its sign set|, f32::MAX's bits): for x = 0x3f800000 the answer is 0x3f800000.
define i32 @clamp(i32 %x0) {
  %x = or i32 %x0, -2147483648
  %f = bitcast i32 %x to float
  %a = call float @llvm.fabs.f32(float %f)
  %i = bitcast float %a to i32
  %m = call i32 @llvm.umin.i32(i32 %i, i32 2139095039)
  ret i32 %m
}

; The shape F17's release build had after SLP vectorization: two lanes of the clamp, one bounded by 0.
define <2 x i32> @clamp2(<2 x i32> %x0) {
  %x = or <2 x i32> %x0, splat (i32 -2147483648)
  %f = bitcast <2 x i32> %x to <2 x float>
  %a = call <2 x float> @llvm.fabs.v2f32(<2 x float> %f)
  %i = bitcast <2 x float> %a to <2 x i32>
  %m = call <2 x i32> @llvm.umin.v2i32(<2 x i32> %i, <2 x i32> <i32 0, i32 2139095039>)
  ret <2 x i32> %m
}

declare float @llvm.fabs.f32(float)
declare <2 x float> @llvm.fabs.v2f32(<2 x float>)
declare i32 @llvm.umin.i32(i32, i32)
declare <2 x i32> @llvm.umin.v2i32(<2 x i32>, <2 x i32>)
