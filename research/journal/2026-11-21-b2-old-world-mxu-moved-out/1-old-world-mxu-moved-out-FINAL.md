# 1 — old world mxu moved out (FINAL)

Task B2. 2026-11-21.

## What I did

- The person moved the old stitch (`~/repos/openmx/mods/tracks/world_mxu`, "every freeride and national",
  starts on mxu_nat17) out to `~/repos/openmx/context/b2-old/world_mxu`. Nothing now shadows the disc-built
  MX Unleashed: WORLD.
- `crates/openmx/tests/middle_spawn.rs`, the ignored installed-worlds test: world_mxu is ridden even when
  `tracks/world_mxu` is absent, since the shell then builds it off the disc. Pinned to mxu_free07.
- The same test was red on the other two installed worlds, both restitched after MD50 (mod.json dated
  2026-11-16): world_mxu_free now starts on mxu_free07 (was mxu_free10) and world_ultra on mxvatv_nat14
  (was mxvatv_nat06). Re-pinned both. Each start lies near its world's middle (world_mxu_free 14020,3503
  of 27585 x 5589 feet; world_ultra 56226,16127 of 115642 x 35164 feet). The task named only world_mxu;
  these two are re-pinned because the test it asks to run stays red without them.

## What the original does

No original behaviour: this is the port's world data and a test of it.

## How it was proved

`cargo test --release -p openmx --test middle_spawn -- --ignored` with `OPENMX_WORLDS` pointed at a scratch
mods folder holding links to the installed world_mxu_free and world_ultra: all three start on their pinned
tile and the first tick stands within 50 feet of the start. world_mxu was built off the disc into the
scratch folder's cache (27 tiles, 449 joins, 21529 x 19324 feet).

## Not verified / not 1:1

- The run used a scratch mods folder, not `~/repos/openmx/mods` itself, so the main checkout's own
  `cache/stitch/world_mxu` was not built by this task: the shell builds it on the first ride there (about 1 min).
- That world_mxu_free's and world_ultra's new start tiles are the right ones is taken from the shell's
  middle-tile pick on the restitched data; I did not check how or why openmx-mods restitched them.

## Next

None.
