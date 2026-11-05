# A wrapped world's tile over the near seam — FINAL

## game-sim

- The move into tile coordinates is `world − at` on the EE's truncating subtract. With `at` ≥ 0 and whole
  it is exact; a negative `at` makes it `world + |at|`, which can drop the sum's low bits once the sum
  crosses a binade (231.666 + 72 = 303.666). So `Tiled::new` takes a negative whole `at` only when the
  tile's kept cells start at or past 0 (`owned()` ≥ 0): the tile is then asked a point at most its own
  last bit short of the world point (1/8192 ft at 1,000 ft), never outside the cells it keeps. Offsets
  past ±2²⁴ ft or not whole are still refused.
- The bucket clamps at 0 stay: bucket 0 holds every tile whose span reaches below 0.
- `crates/game-sim/tests/tiles.rs`, `a_tile_may_stand_short_of_0_where_it_keeps_nothing`: at −81 with
  keep from cell 8 (kept from −9) and at −72.5 are refused; at −72 is taken, and over 6,000 points (half
  crowded near 0) the owner, the local point (≤ x + 72, under a bit short, ≥ 72) and the ground (= the
  tile alone at that local point) hold, with exact and rounded moves each over 1,000 times.

## stitch (openmx's crates/stitch and openmx-mods tools/stitch, 1d5fc4b)

- Grid layout: on a wrapped axis the first line's cursor starts at two lattice cells and no tile is held
  at or past its cut, so the first line's cut hangs over the near seam; the tail past the last line is
  the largest `hi(t) + gap − lo(u)` over the pairs across the wrap (MD54), now with no first-tile cut to
  cover. `stitch::VERSION` 3 → 4 (the disc-world cache key).
- The units layout (world_mxu) is not changed (see below).
- `crates/stitch/tests/wrap_width.rs` (openmx-mods `tests/wrap_ridge.rs`): four ringed tiles in a 2×2,
  wrapped xz, gap 120: the first line stands below 0; each wrap gap 126 ft for a 120 ft need (stitched
  144); `stitch check` ok; no holes; steepest step across the seams 0.118 ft.

## Measured (openmx release `--seams --headless`)

| world | before | after |
|---|---|---|
| world_mxu_free size | 17415 × 11934 (installed), 17091 × 11610 (MD54) | 16533 × 11061 ft |
| over the seam, x | 1134 (installed), 927 (MD54) | 252 ft |
| over the seam, z | 1125 | 252 ft |
| step across the wrap | 0.040 | 0.131 ft |

252 = 135 stitched + 135 strip − 18. Steepest step 0.246 ft, no holes, 35 + 13 seam joins. x 09 > 11
over the seam is 5724 ft (6606 before): an empty cell of the 3×2 grid, not a join.
world_mxu_nat (grid): 22 tiles, 817 joins, no gaps, 0.508 ft across the wrap; crossings 487–838 ft
but for uneven lines (up to 18968 ft). world_mxu (units): unchanged, crossings ≤ 2750 ft.
openmx's disc build of world_mxu_free is byte-equal to openmx-mods' (all join files; mod.json but for the
`category` line openmx writes). openmx-mods' ignored `world_mxu_free_meets_ridge_to_ridge`: joins 135 ft,
ridge dips ≤ 5.6 ft, rides across the seams not thrown.

## Second attempt (re-landed on main 71a3ad34)

The first commit (wip/MD61, de7d38ac) went red in the merge gate only on disc_worlds' comparison with
the installed world in the main checkout's mods/, which was older than VERSION 4 (and has since been
reinstalled by hand as MD95's units build, 2026-11-04 19:37). Cherry-picked onto main; on top:

- P299 landed meanwhile: a VERSION bump makes `crates/stitch/worlds/<id>.start.json` stale, which made
  world_mxu and world_mxu_nat build from cold and spend 1452 / 1029 s of CPU against 900. The three start
  files are now the ones this stitcher's builds kept (the test names them).
- `disc_worlds.rs` `rides_off_the_disc`: the installed comparison skips, saying so and where to reinstall
  from, when the installed mod.json's tile lines (`"tile"`, `"at"`, `"keep"`) differ from the build's: a
  user's copy laid out by another stitcher or spec is a reinstall to do (MD100), not this build gone
  wrong. Laid out alike, every byte must still agree. Taken from nothing in opentennis (no stitcher there).
- The old attempt's follow-ups were renumbered on main by other tasks; re-filed as MD99 (units layout on
  the seam) and MD100 (reinstall).

## Not verified / not 1:1

- The installed worlds in the main checkout's mods/ were not reinstalled (never touch the main
  checkout); disc_worlds' world_mxu_free comparison against them now skips, as their tiles stand
  elsewhere, so the installed copy is not proved equal to this build until MD100.
- The units layout could also start on the seam, but taking its shortest world then widens interior z
  joins to 2412 ft (> 1800); left as it was (MD99). MD95 (in flight, not on main) moves world_mxu_free
  to the units layout, where MD61's grid change does not apply: once it lands, world_mxu_free's wrap join
  is the units layout's again until MD99.
- After the re-land only world_mxu_free's `--seams` was rerun (debug build, off the disc into an empty
  mods folder): 16533 x 11061 ft, 252 ft over the seam on x and z, 0.246 ft steepest, 0.131 ft across
  the wrap, as before; world_mxu and world_mxu_nat's tables are the first attempt's.
- The step across the wrap rose from 0.040 to 0.131 ft (still well under the half-foot bound).
- Shapes in a cut ridge deeper than the seam copy's reach (~260 ft) past the near seam are swept from one
  side only.
- No live ride in a window across the seam; only `--seams` and the headless ride tests.
