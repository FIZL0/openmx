# R21 — the LLVM miscompile F17 hit, reduced

2026-11-03. Task: `./kit plan R21`.

F17 found that rustc 1.99.1 miscompiles the obvious branch-free clamp `min(b & 0x7fffffff, MAX)` in
release builds. It is not the SLP vectorizer or the x86 backend. It is a known-bits bug in LLVM 23's
SelectionDAG, on every target:

- `computeKnownBits(ISD::FABS)` calls `KnownBits::makeNonNegative()`. That sets the sign bit in
  `Known.Zero` but does not clear it in `Known.One`. When the operand's sign bit is known to be 1, the
  result's sign bit is then "known" to be both 0 and 1, and a `umin` against a constant folds to the
  constant.
- Introduced by llvm-project 11bc6e8776d2 (#188335, March 2026, first in 23.1.0).
- Fixed on main by 6bb86c2af2ba (#227457, 2026-09-30). It is not on release/23.x and not in Rust's LLVM
  fork, so stable, beta and nightly rustc all still have the bug (checked 2026-11-03).

The reduction is two lines of Rust (`research/r21/clamp_magnitude.rs`) and five lines of IR
(`research/r21/fabs_known_bits.ll`). The fix is upstream already, so there is nothing new to report to
LLVM. Filing the backport request to rust-lang is left to the user; the text is ready in 1-….
`vu_bits`'s `b << 1` workaround is sound: it never builds an `fabs` node, so the faulty rule is never
used.
