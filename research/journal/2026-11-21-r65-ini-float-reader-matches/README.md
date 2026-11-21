# R65 — ini float reader matches

2026-11-21. Task: `./kit plan R65`.

B47 saw free07's `[Machine1]` `Direction` y one bit apart: the original's `0xbd136fab`, "Rust's"
`0xbd136fac`. The port's ini reader (`phy::Ini`, `strtod` then a truncating double-to-float) already gives
`0xbd136fab`, the same three words as the original's car +0x280. The off bit was the B47 test's own `f32`
literal, which rounds to nearest. No product change; S497 tightens the test. Details in the numbered files;
the newest one's suffix says whose move it is (FINAL · PART · READY).
