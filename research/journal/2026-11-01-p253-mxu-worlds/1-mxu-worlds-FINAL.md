# 1 — mxu worlds (FINAL)

Task P253. 2026-11-01.

## What I did

- Looked for the drop-out. On the main checkout's installed `mods/tracks/world_mxu` (openmx-mods'
  stitch) and on one built off the disc into an empty mods folder, `--seams` showed 27 tiles. All five
  freerides were loaded, and their ground and props were built and drawn when a ride was near them. A
  `--trace-at 10600,34200,0` shot inside free09 showed Bridgeport, with Foxpark, Norfolk, Cleveland and
  Dallas on the radar. None of the four places named in the brief (tile build, plan placement, drawn
  chunks, radar regions) dropped anything.
- What the user saw was the layout. `stitch::build::plan` sorts the tiles by their ring's mean edge height
  and fills a 6 × 5 grid in a snake. The freerides' edges are 247.7 ft against 82–163 ft for the
  nationals, so they took the last row and a half, at z 23184–39000 of a 37972 ft world. MD50's middle
  tile was nat01 (start 20088,18432), 12,000+ ft from the nearest freerides. Within the draw distance
  were only nationals and their radar names.
- `stitch::build::Params::middle` is the spec's `"middle": [tile, …]`. Those tiles take the snake's cells
  nearest the grid's middle (ties go to the snake's order), in height order among themselves, and the
  other tiles fill the remaining cells in the snake's order. When `middle` is empty the layout is exactly
  as before: world_mxu_free still matches openmx-mods' world to the byte (its test compares them).
- `world_mxu.json` is now named "MX Unleashed: WORLD" and has `middle` = the five freerides. Built, it is
  37720 × 31456 ft with 728 joins (it was 45523 × 37972 ft with 1222), widened to 1.953 instead of 5.96,
  and builds in 46 s. Its seams walk finds no gaps, and its steepest step is 0.96 ft (it was 7.99 ft).
  The ride starts on mxu_free07 (Norfolk), with Bridgeport, Rochester and the rest on the radar.
- `world_mxu_nat.json` is new: "MX Unleashed: every national", nat01–nat22, wrapped, lit and dressed from
  nat01. It builds in about 90 s, and its seams walk finds no gaps (steepest step 0.95 ft). The ride
  starts on mxu_nat02. `disc_worlds::home_track` now returns any spec tile's track, not only a freeride.
- `disc_worlds::SPECS` and `menu::DISC_WORLDS` are now ordered WORLD, freeride maps, every national.
  `id_of("free_all")` still names world_mxu_free.
- A world ride prints `radar maps: N: name, …`, the maps its radar names, so a test can read them.

## What the original does

Nothing: the original rides one map at a time. The worlds are the port's own.

## How it was proved

- `cargo test --release -p stitch`: all pass, including the new `a_middle_tile_is_laid_in_the_grids_middle`
  (the highest of nine generated tiles is laid at cell (1,1) and becomes the wrapped world's spawn).
- `cargo test --release -p openmx --bin openmx`: 209 pass, including `disc_worlds::tests` (the SPECS and
  menu order and names, each world's tiles and its `middle`, the home tracks).
- `cargo test --release -p openmx --test disc_worlds`: 3 pass on the disc in 85 s.
  `world_mxu_has_every_freeride_and_national_with_the_freerides_in_its_middle` checks that all 27 tiles
  are placed, that no freeride is in an outer row or column, that the spawn is a freeride, that each
  freeride's ground is built and its props drawn, and that the radar names 27 maps, Dallas through
  Cleveland among them. `world_mxu_nat_is_the_22_nationals` checks the tiles, spawn, radar count and
  seams. world_mxu_free still matches openmx-mods' world byte for byte.
- Shots, kept outside the repo: the spawn of the new world_mxu (Norfolk) and of world_mxu_nat.

## Not verified / not 1:1

- **The user's own rides still get the old world_mxu.** `~/repos/openmx/mods/tracks/world_mxu` is
  openmx-mods' build with the old layout and the old name, and an installed world of a disc world's id is
  ridden in place of the disc build (P232). It has to be removed, or rebuilt by openmx-mods' stitch from
  this spec, before the user sees P253. That folder is in the main checkout, which this task must not
  touch, so it is filed as a follow-up.
- openmx-mods' own `tools/stitch/worlds/world_mxu.json` still has the old layout. Its plan already says
  stitch is to build on openmx's stitcher.
- `crates/openmx/tests/middle_spawn.rs`' ignored test of the installed worlds expects world_mxu to start
  on mxu_nat17. That holds for the installed world but not for a rebuild.
- The pick's card for world_mxu_nat turns through the freerides' stills, as every disc world's does.
- Nobody rode across the new world's joins by hand. Only the seams walk and the shots checked them.

## Next

None: the task is done.
