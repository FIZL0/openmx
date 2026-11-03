# P286 — world_ultra's far ground, coarse

2026-11-03. Task: `./kit plan P286` (follow-up of P283).

A world's GPU ground is built at full detail only within 2500 ft of the riders now. Out to the
6000 ft load reach it is built at a quarter of its samples each way: a sixteenth of the chunks.
On world_ultra the first frame holds 3105 chunks, down from 15,423, and the process's GPU memory
falls from 1988 MB to 1464 MB. The ride hash and the picture are unchanged.

Code:
- `crates/openmx/src/worlds.rs`: `coarse`, `COARSE_BY`, `FINE_REACH`, and `GroundStream`'s fine and
  coarse levels (`wanted`, `held_coarse`).
- `crates/openmx/src/main.rs`: `--world-fine LOAD,DROP|off`, and the `world ground:` line.

Tests:
- `worlds.rs` `far_ring` (3 unit tests).
- `crates/openmx/tests/world_ultra_load.rs` checks the chunk counts now.

Entry: [1-far-ground-ring-FINAL.md](1-far-ground-ring-FINAL.md).
