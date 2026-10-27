# 1 — stitched seams (FINAL)

Task MD47. 2026-10-27.

## What I did

- **Where the gaps are: in the data, not in the port.** Ran the shell on the installed worlds
  (`~/repos/openmx/mods/tracks/world_*`, built 2026-10-23 10:54): world_mxu says `world holes: 5052`
  wrapped (the user's ~5000; openmx-mods C12 measured 2189 with the switch off), the first at
  `x 1147.5..1152.0 z 0.0..255.0`, then every 1152 ft. Each is 4.5 ft wide: a join terrain's last
  cell. A terrain's field ends a cell short of its samples — the port's `Ground::at`/`Tile::local`
  read no cell that starts on the last sample, as a track alone always has — and stitch then laid its
  256-cell joins 256 cells apart, so the strip between two joins belonged to nobody. Stitch has
  overlapped its joins by a leaf since openmx-mods C11 (2026-10-23, later that day); the installed
  worlds were never rebuilt. Wrapping only cuts the same strips into more rectangles (the seam
  joins' edges are more cut lines in `Tiled::holes`), hence ~2× the count. The port's tile placement
  and lookup were right: on worlds stitched after C11 the walk below finds no gap anywhere.
- **The wrap.** The port's seam strip (`close_seams`, edea158e's start a whole foot short) closes the
  wrap with no gap and no step (below). What stitch's worlds lacked across the wrap was ground that
  agrees: the far edge and the near one were different rows' extensions, so the strip ramped up to
  130–190 ft over 15 ft (C12's numbers). That is C12's (openmx-mods, s1, in progress during this
  session): its field is now a torus on a wrapped axis. Nothing to change in the port for it.
- **`Tiled::seam_walk`** (game-sim `tiles.rs`): every piece's owned edge as `Tile::local` draws it,
  and on a wrapped axis the world's own edge, a ray down every foot along, an eighth of a foot to
  either side, through the ride's own `World::ground` at the folded point. Counts rays with no
  ground (`gaps`, the first eight kept), and where two different pieces answer the two sides
  (`met`), the step between their heights: the steepest, where and between which pieces, how many
  over 3 inches (`rough`), and the steepest across the wrap line. An edge with the same piece on both
  sides (hidden under an earlier piece) is that piece's own ground and no seam.
- **`--seams`** in the shell: a world's load prints `world seams: …` from the walk
  (`WorldMod::seams_line`). It loads every piece, so it is a flag, not a default.
- **Tests**, `crates/openmx/tests/stitched_seams.rs`: a generated stitched world (four 9-ft tiles of
  60 kept cells round a cross of 15-cell join bands, one root cell a join, one smooth relief that
  comes round every wrapped width, as C12's field does).
  - `every_seam_of_a_stitched_world_has_ground_on_both_sides`: built straight in game-sim, joins every
    15 cells (stitch since C11), unwrapped and wrapped (seam joins from `generate::seam_joins`): 0 gaps
    of 26,460 / 48,060 rays, steepest step 0.006 ft, 0.006 ft across the wrap.
  - `joins_a_terrain_apart_leave_the_gaps_the_walk_finds`: the pre-C11 layout (joins 16 cells apart)
    — `holes()` has the 135–144 ft strip in each band and the walk finds >100 gaps, so the test above
    can tell.
  - `the_shell_walks_the_worlds_seams_and_finds_no_gap`: the same world as a mod through the shell's
    loader and its own seam joins, `md47_open` and `md47_round`: `world holes: none`, no gaps, steps
    under 1/16 ft.
  - `a_ride_across_the_seams_at_speed_neither_drops_nor_bounces`: on the gas from x 200 along z 270:
    wrapped, 900 ticks over the join band, `b`, the seam strip and the wrap onto `a` again, ten seam
    crossings; open, 220 ticks across the band (stopping before the outer edge's throw). Never down,
    never thrown, never under the ground; the largest tick-to-tick change in the bike's height over the
    ground is 0.011 ft within 15 ft of a seam against 0.015 ft on the tiles.
  - `the_stitched_worlds_have_no_gap_along_a_seam` (ignored): the five real worlds from
    `$OPENMX_WORLDS`. Its run is below.
- A first try with relief that did not come round failed the ride's jolt bound at the seam strip's
  ends (0.106 ft a tick against 0.035): the strip is a straight run between the two edges' heights,
  and over ground that does not wrap that is a crease. That is MD29's design for a world whose data
  does not come round; C12's worlds do, so the fixture does.

### The real worlds, walked (`--seams`, release build)

Each line: edges walked, rays, gaps, places two pieces meet, steepest step across a seam (between),
steepest across the wrap. "open" is C12's slot build (`openmx-mods-slots/s1/out/mods`, 11:09 today)
with the wrap switch taken out; "wrapped" the same as built; the installed worlds
(`~/repos/openmx/mods`) had become that same build by the end of the session (same figures to the
digit; folder times 11:09, copied with times kept) — at the start they were the 10-23 build with
5052 holes on world_mxu.

| world | open: gaps / steepest | wrapped: rays, gaps / steepest / across the wrap |
|---|---|---|
| world_mxu_free | 0 of 808,020 / 0.302 (j006_005 \| j007_005) | 1,040,382, 0 / 0.302 / 0.033 |
| world_mxu | 0 of 10,464,306 / 4.142 (mxu_nat17 \| j017_023) | 11,128,908, 0 / 4.142 / 0.062 |
| world_mxvatv | 0 of 7,365,666 / 0.679 (mxvatv_nat11 \| j017_025) | 7,955,984, 0 / 0.679 / 0.042 |
| world_untamed | 0 of 8,790,876 / 0.419 (c20_002_000 \| untamed_am_jy) | 9,372,126, 0 / 0.419 / 0.039 |
| world_ultra | 0 of 33,208,306 / 4.142 (mxu_nat17 \| mxu_j017_023) | 34,536,784, 0 / 4.142 / 0.061 |

`world holes: none` on all ten. Before (installed 10-23 build, wrapped): world_mxu 5052 holes and
1,445,652 of 7,252,068 rays with no ground; world_mxu_free 99 holes, 118,935 of 1,275,930.

## What the original does

Nothing here: the original has no worlds of several terrains. The one thing taken from it is that a
terrain's field reads no cell starting on its last sample (S64's `Ground`, unchanged).

## How it was proved

The walk is the proof: a ray down a foot apart along every seam of every piece, through the
simulation's own ground query, on a generated world in the gate (4 tests, `stitched_seams.rs`) and on
the five real worlds by hand (above). opentennis has no tiled ground, so nothing was taken from it.

## Not verified / not 1:1

- **The worlds measured are C12's work in progress.** openmx-mods C12 had not landed when this
  session ended; its slot's build is what is now installed in `~/repos/openmx/mods` (not by this
  task, which changed nothing in openmx-mods or the main checkout). If C12 changes before it lands,
  rerun `the_stitched_worlds_have_no_gap_along_a_seam` with `OPENMX_WORLDS=~/repos/openmx/mods`.
- **The ignored real-world test itself was not run through `./kit check`**: the figures above are the
  same `--seams` line it reads, from a release build run by hand (debug would take hours on
  world_ultra's 34 million rays).
- **A 4.14 ft step on world_mxu** between `mxu_nat17` and join `j017_023` at (18805.5, 25407), in the
  data: the join's heights there are not the tile's at their shared edge (the grids line up: the join
  is 99 cells from the edge at 4.5 ft, and every other seam on the world is under 0.7 ft). Filed as MD49
  (stitch); not looked at further.
- **"Steps over 3 inches" counts slope too**: the walk's two rays are a quarter of a foot apart, so a
  1:1 slope reads as a 3-inch step. The counts on the real worlds are an upper bound on real steps.
- **The ride's bounce bound is relative** (a seam no worse than 1.5× the tiles' worst tick plus
  0.02 ft), on one line across the world and one speed; rides across the `z` band and diagonal
  crossings were walked, not ridden. The rides skip without the disc.
- **Rays every foot**: a gap narrower than a foot between two rays would be missed by the walk;
  `Tiled::holes` (exact, MD27) is the check for that and reads none on the post-C11 worlds.

## Next

Nothing for MD47. MD49 is the 4.14 ft step.
