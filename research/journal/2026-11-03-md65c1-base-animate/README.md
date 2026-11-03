# MD65c1 — base animate

2026-11-03. Task: `./kit plan MD65c1`.

The biplane's base animation driver (SLUS-21104 atv_biplane_animate, which the quad's animation driver
runs first) ported as `quad::base_animate` in `crates/game-sim/src/quad.rs`, together with the check it
asks (vt +0x3e8, atv_biplane_net_check). `quad::animate` now runs it instead of logging
`Call::BaseAnimate`. Proved by `crates/game-sim/tests/atv_quad/animate.rs`. Rows: research/oracle.txt
`# MD65c1`. Details in [1-base-FINAL.md](1-base-FINAL.md).
