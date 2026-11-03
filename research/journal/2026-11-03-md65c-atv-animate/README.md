# MD65c — atv animate

2026-11-03. Task: `./kit plan MD65c`.

The quad's animation driver (SLUS-21104 vt +0x3e4, 0x004be3c0; PC MXVehicleAtv::vf67), ported where it
touches the simulation as `quad::animate` in `crates/game-sim/src/quad.rs`, proved by
`crates/game-sim/tests/atv_quad/animate.rs`. Rows: research/oracle.txt `# MD65c`. Details in
`1-animate-FINAL.md`.
