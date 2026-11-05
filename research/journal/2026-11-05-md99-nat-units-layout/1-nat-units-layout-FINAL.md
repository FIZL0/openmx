# 1 — nat units layout (FINAL)

Task MD99. 2026-11-05.

## What I did

- **The empty cells.** world_mxu_nat had no `layout`, so stitch's `plan()` grid put its 22 maps in
  `ceil(sqrt 22) = 5` columns by 5 rows. That left three cells with no map in them, the same kind of
  map-wide flat join as world_mxu_free's "Cleveland flattened" (MD95). On the old spec the new stitch test
  sees cells (2,4), (3,4) and (4,4) empty.
- **Fix:** `crates/stitch/worlds/world_mxu_nat.json` gets `"layout": "units"`, as world_mxu (P251) and
  world_mxu_free (MD95) have. The 22 nationals, each one unit, fill an 11 x 2 grid with no cell empty.
  Off the disc the world is 62704 x 12664 ft, with 22 tiles and 540 joins (75 of them seam joins). The ride
  spawns on nat19. `--seams` reports no gaps, and its steepest step is 0.701 ft.
- **Crossings** (the `--seams` walk, 44 of them, 22 on x and 22 on z):
  - Along x: 306-657 ft, and 802 and 852 ft across the x wrap.
  - Along z: 594-1341 ft, and 856-1441 ft across the z wrap.
  - Every one is within MD95's caps (1800 ft, 2900 ft over the wrap).
- **Start file.** `crates/stitch/worlds/world_mxu_nat.start.json` is re-keyed for the new spec (key
  41ddb588f4eede33), copied from the cold build's `start.json`. Cold, the build spent 1857 s of CPU and
  overran COLD's 900 s. From its start file it builds in 1 pass, with 212-290 s of CPU (49-84 s of wall
  clock).
- **Tests:**
  - `crates/openmx/tests/disc_worlds.rs` `world_mxu_nat_is_the_22_nationals` now also checks that there are
    2 x 22 crossings and that each is within the caps. It uses MD95's `crossings` helper.
  - `crates/stitch/tests/units_wrap.rs` `world_mxu_nats_spec_leaves_no_empty_cell` lays the real spec out
    over 22 generated maps. It checks that every cell of an 11 x 2 grid is filled and that every side faces
    a map within 1500 ft. It fails on the old spec, as shown above.
- Nothing from opentennis applies, since it has no stitched worlds.

## What the original does

Nothing: world_mxu_nat is the port's own world (P253). Each map in it is the original's terrain, untouched.

## How it was proved

- The disc test, run off the disc with its build from cold and then from the start file. The crossing
  lines above come from that run's output.
- The stitch layout test, checked to fail on the old spec and to pass on the new one.

## Not verified / not 1:1

- The old world's crossings were not measured. That would have needed a second cold build of the old spec
  (~1800 s of CPU). The three empty cells are shown by the layout test instead, not by `--seams` feet.
- Not ridden or looked at on screen. There were no shots of the new 11 x 2 world. What is known about its
  ground is the `--seams` walk (no gaps, 0.701 ft steepest step) and the crossing widths.
- Nothing is installed to compare against: the main checkout's mods/ has no world_mxu_nat, and openmx-mods'
  own stitcher has no unit layout. A copy built elsewhere keeps the 5 x 5 grid until it is rebuilt. Nothing
  was uploaded, because the task does not say to publish.
- The 11 x 2 shape is a choice, not the original's (the original has no world). It is the shape the unit
  layout picks for 22 one-unit maps.

## Next

none
