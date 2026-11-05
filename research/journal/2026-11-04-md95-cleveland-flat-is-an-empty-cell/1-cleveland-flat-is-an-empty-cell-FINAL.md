# 1 — cleveland flat is an empty cell (FINAL)

Task MD95. 2026-11-04.

## What I did

- **Cleveland itself was never flat.** free11's tile in the installed world_mxu_free is the disc's own
  terrain, byte for byte (`Tracks\free11.res`'s `free11.ps2.trn`, md5 24583e4c… in world_mxu,
  world_mxu_free and world_ultra alike). Its heights run 44-262 ft over the map, with a sunken bowl
  (45-60 ft) in its middle. `every_installed_worlds_tiles_read_as_alone` passed on all five installed
  worlds before any change (511 s), and `--trace-at` shots of free11 alone (3200,3000,90) and in the
  world (8672,8472,90) are the same picture.
- **The flat plane was an empty cell.** stitch's default `plan()` grid puts five maps in
  `ceil(sqrt 5) = 3` columns by 2 rows, so one cell has no map: (0, 1), north of Dallas and west of
  Cleveland. It was all join ground, a map's width of gentle relief painted from its neighbours. That is
  exactly the user's "big flat plane between dallas and cleveland", and the radar names Cleveland
  beside it. `--seams` on the old install showed it as `mxu_free09 > mxu_free11 across the seam, 6606 ft
  of join`.
- **Fix:** `crates/stitch/worlds/world_mxu_free.json` gets `"layout": "units"` (P251's unit layout,
  which has the fewest empty units). For five one-unit maps that is a 5 x 1 row: free06, free10,
  free07, free09, free11 along x, wrapping in x and z. The world is 28503 x 6462 ft, with 5 tiles and
  52 joins (36 + 16 seam joins, where it was 38 + 14). Every crossing is 135 ft of join, except 1278 ft
  across the x wrap (free11 > free06). Round the z wrap each map meets its own other side. The ride now
  starts on free07 (the row's middle, MD50), where it started on free10 before.
- **Reinstalled.** The release shell built the world off the disc into a scratch mods folder, and its
  `mod.json` and `joins/` replaced the ones in `~/repos/openmx/mods/tracks/world_mxu_free`. `tiles/`
  was kept, since the tiles are unchanged. The old `mod.json` and `joins/` are kept in this worktree's
  `context/md95-old-world_mxu_free/`. openmx-mods' own `tools/stitch` has no unit layout (P232 moved the
  stitcher here), so it was not used and its spec was left alone.
- **Tests:**
  - `crates/stitch/tests/units_wrap.rs` `world_mxu_frees_spec_leaves_no_empty_cell`: the real spec,
    laid out over five generated maps, is one row, and every x side faces a map within 1500 ft. It
    fails on the old grid spec (cells `[(0,0),(1,0),(2,0),(2,1),(1,1)]`).
  - `crates/openmx/tests/disc_worlds.rs` `world_mxu_free_is_built_off_the_disc_as_openmx_mods_builds_it`
    now also checks the world's size and that every crossing is within world_mxu's P251 caps (1800 ft,
    2900 ft over the wrap). The old world's 6606 ft fails that. It still compares the build byte for
    byte with the installed copy.
- Other worlds that hold free11: world_ultra (openmx-mods built) and world_mxu (already `units`, with the
  freerides as its middle cluster). Both read free11 as alone in the installed test. Nothing to change.
- Filed MD99: world_mxu_nat's default grid (22 maps in 5 x 5) leaves three empty cells of the same kind.
- Nothing from opentennis applies (it has no stitched worlds).

## What the original does

Nothing: world_mxu_free is the port's own (P232), and each map is the original's terrain untouched.

## How it was proved

- The installed-worlds test `every_installed_worlds_tiles_read_as_alone` (ignored, needs
  `OPENMX_WORLDS`) passed before the fix and again on the reinstalled world.
- `--seams` crossings on the old install against the new build (numbers above).
- Shots: free11 alone vs in the old world (the same picture); an aerial of the old world's empty cell
  (flat painted joins); an aerial of the new row (Cleveland's bowl between narrow joins, its own copy
  across the z wrap).

## Not verified / not 1:1

- Not seen on the user's device. If they ride a store copy of world_mxu_free, or one built elsewhere,
  it keeps the old 3 x 2 layout until it is rebuilt from the new spec. The store copy was not
  re-uploaded: the task does not say to publish, and the memory rule says no store uploads from this
  account.
- The 5 x 1 row is a choice, not the original's (it has no world). A map's north edge now joins its
  own south edge round the z wrap, so riding north off Cleveland comes back into Cleveland. That is
  what the unit layout gives five equal maps, and it has no empty cell.
- The new world's ride was looked at only in headless shots (aerial and the start), not ridden by
  hand across every seam. `--seams` reported no gaps, with the steepest step 0.276 ft.

## Next

none

Landed with `--skip-audit`. The audit is red on main for K26's four leaks (atv_wheel.rs:62,66 and
plan/DONE.md:519,533), none of them in a file this task touched. The gate's tests passed (4805).
