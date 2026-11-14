# 1 — world mxu even (READY)

Task MD155. 2026-11-14.

## What I did

- **Drew the layout from above.** `draw_layout.py` turns an `extent W H` + `id x0 z0 x1 z1` file of
  kept interiors into an SVG. It draws each tile's box and name, the wrap seams (dashed) and the world
  extent, and labels every side with its join in feet to the nearest map it faces, round the wrap too.
  Labels over 1,300 ft are red. The before file was made from the old cache's mod.json
  (`mods/cache/stitch/world_mxu/c694cb09fdbc5694`). The after file comes from the new build's mod.json
  and matches `$OPENMX_LAYOUT_DUMP` of the test below.
  - `before.svg`: 21,853 × 19,828 ft, 480 joins. Median join 657 ft. Widest, all over the wrap:
    nat11/nat19 2,750, nat02/nat20 2,472, nat18/nat20 2,332, nat09/nat13 2,250, nat02/free11 2,205.
  - `after.svg`: 22,294 × 19,243 ft, 462 joins. Median 868 ft. The coarse `layout()`'s widest is
    1,404 ft (nat11/free06), median 774.
- **All 27 tiles were there before**, as `tests/disc_worlds.rs` already proved. "Missing" was the
  2,000+ ft stretches of empty join over the wrap: riding off the edge of nat11 or nat02 crossed what
  looked like nothing.
- **The fix, in `stitch::build::units_layout`, on a wrapped axis only:**
  - The end margin is 2 × step, not `pad`, and the cut-ground lower bound is dropped. A tile may start
    just past 0, so its trimmed ground hangs over the seam onto the far side, as MD61's negative origins
    allow.
  - The shared-slack start (`low + (high-low)·low/e`) is relaxed `RELAX` = 200 times. Each tile moves
    to the middle of the room its facing neighbours leave it, measured round the wrap.
  - Among the seam lines tried, the key is (1) the spec's `centre` tile lies across the world's middle,
    (2) the narrowest widest straight join, rounded to the foot, (3) the shortest world. "Straight" on z
    means the x spans really overlap, not the loose `face` set.
  - In `unit_pack`, the `centre` tile swaps into the most central cell of the `middle` block. The
    block's first row was free06 | free10 | free07 with a national column after it. That left free07 at
    the block's end, and free06 more than 6,000 ft from the start: the desktop's load reach, which the
    ride test needs for all five freerides. The old build only just met it (about 5,994 ft). The first
    gate found this: `mxu_free06 not drawn`. Now the row is free06 | free07 | free10, with free11 and
    free09 under free07 and free10.
  - An axis with edges is untouched: world_mxu_free's and every edged world's lines are as before.
  - `world_mxu.json` gains `"centre": "mxu_free07"`. `stitch::VERSION` goes 4 → 5, so every cache key
    changes. The three `worlds/*.start.json` files are refreshed from cold builds.
- **Tests:**
  - `disc_worlds::tests::world_mxu_is_laid_out_evenly` (bin): every listed tile is placed once, and
    every side of every map faces another within `EVEN_GAP` = 1,500 ft, wrap included.
  - `tests/disc_worlds.rs` world_mxu ride: the spawn must be `mxu_free07`, with one 2,100 ft cap for
    every crossing. Before, the wrap's cap was 2,900.
- **B2 checked:** it still holds, and it is the likeliest reason the user saw the old layout. The main
  checkout's `mods/tracks/world_mxu` (an openmx-mods install) beats the disc build (memory
  `checkout-rides-the-installed-world-mxu`). This task may not touch the main checkout. See Next.
- Nothing in opentennis bears on this.

## What the original does

Nothing: world_mxu is openmx's own stitched world, not the original's.

## How it was proved

- A cold release build, `openmx --headless --track world_mxu --frames 5 --seams`, gave:
  - 27 tiles, 13 passes, 57.0 s.
  - `world spawn: mxu_free07 at 8836.0,10559.0`.
  - `world props drawn:` all five freerides, at the start.
  - `world seams: no gaps`, steepest step 0.614 ft (0.099 across the wrap).
  - 54 crossings: min 351, median 868, max 2,008 (nat16 > free06 across the seam, slope 0.567), then
    1,620 (nat11 > free06), 1,436 (nat04 > free06, seam) and 1,310 (nat18 > free11).
- The freeride block holds together around free07.
- `./kit check -- -p stitch` passes. The gate is `./kit done MD155`.

## Not verified / not 1:1

- **Not ridden or seen on screen or on the phone.** It was only built headless and drawn from the
  mod.json. Whether the joins *look* even when ridden was not watched.
- **The user's own install still shows the old layout** until it is reinstalled (B2, Next).
- **Joins are evener, not equal.**
  - The freeride columns are shorter than the nationals', so their sides keep wider joins.
  - The build's slope passes (`Plan::widen_at`) widen a few joins past the coarse layout:
    nat16 > free06 is 2,008 ft built against at most 1,404 in `layout()`. Its edges differ a lot in
    height, and the old pack had the same pair at 1,674. This is why the ride test's cap is 2,100.
    Nothing reorders the nationals by edge height to avoid such pairs.
  - free06 now gets most of the wide joins: 1,620, 1,436 and 2,008 ft. Free07 stays the start, and
    free06 is the block's outer end.
  - The median join rose (657 → 868 ft) as the slack moved off the wrap into the inside.
- **The other two MX worlds changed too: both wrap.** Neither was drawn or ridden; only their cold
  builds were read. Their start files were refreshed.
  - world_mxu_free: 5 tiles, 47 joins, 27,585 × 5,589 ft (was 52 joins, 28,503 × 6,462). Widest
    crossing 360 ft, no gaps. The test's expected line was updated.
  - world_mxu_nat: 22 tiles, 740 joins, 62,218 × 12,646 ft, spawn nat04. Widest crossing 1,423 ft, no
    gaps. Its test keeps the old caps. Neither has a `centre`, so the `unit_pack` swap leaves them
    alone.
- **The `centre` rule is loose.** The world's middle only has to fall inside free07's interior; it is
  not centred on it. Lines move a unit at a time, and free07 sits about 0.6 unit off.

## Next (the human: plan/HUMAN.md, "See MX Unleashed: WORLD's new layout")

- After the merge, the user reinstalls world_mxu from openmx's cache over the old openmx-mods install,
  or deletes `~/repos/openmx/mods/tracks/world_mxu` so the disc build is ridden. Recipe: memory
  `reinstall-a-disc-world-from-openmx-cache`. openmx-mods' own build also needs stitch VERSION 5.
- Ride it and say whether the joins look even.
