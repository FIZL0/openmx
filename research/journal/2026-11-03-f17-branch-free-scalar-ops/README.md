# F17 — branch free scalar ops

2026-11-03. Task: `./kit plan F17`.

F15a's four-lane `Vf` left the scalar ops branchy. Now the scalar exact paths are branch-free and
inlined: the FPU's add (the alignment is a mask, `fpu::align_keep`, with no branch on the exponent gap),
the FPU's mul, the VU's scalar add/sub/mul/madd/msub, and `Lane`'s. Results are bit-identical:
- the FPU against the integer model, including new F-type and known-sign cases;
- the VU against the pre-F17 branchy code, kept in `tests/vu.rs` as `reference`;
- the oracle tests;
- the ride hash, which is unchanged.

The speed gain is small. Exact sim is about 0.94× HEAD on the desktop, unpaced, in 1P and 2P. Native is
about 0.62–0.65× HEAD. So the gap between exact and native is mostly not in these ops any more.

Along the way: rustc 1.99.1's LLVM miscompiles the obvious branch-free clamp (`b & 0x7fffffff` then `min`)
in release builds, once it is inlined into a caller whose operand's sign is known. `vu_bits` works around
it by using `b << 1`. Details are in 1-….
