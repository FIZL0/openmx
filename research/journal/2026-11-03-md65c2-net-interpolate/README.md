# MD65c2 — net interpolate

2026-11-03. Task: `./kit plan MD65c2`.

The quad's net interpolation (vt +0x3ec, atv_quad_net_interpolate) is ported as `quad::net_interpolate`
in `crates/game-sim/src/quad.rs`. It is what `Call::NetInterpolate` from `quad::base_animate` stands for.
It includes the biplane's interpolation (atv_biplane_net_interpolate), which the quad's calls last. The
test is `crates/game-sim/tests/atv_quad/interpolate.rs`, and every case agrees with the original to the
bit. The field map is in research/oracle.txt under `# MD65c2`. Details in
[1-net-interpolate-FINAL.md](1-net-interpolate-FINAL.md).
