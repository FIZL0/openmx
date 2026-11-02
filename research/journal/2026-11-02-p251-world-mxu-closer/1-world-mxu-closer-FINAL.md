# 1 — world mxu closer (FINAL)

Task P251. 2026-11-02.

## What I did

Measured first: a full build of HEAD's world_mxu (`openmx --headless --no-sound --frames 2 --seams
--track world_mxu --mods <scratch>`, which builds it off the disc and prints a `world crossing:` line
for each map side: how much join a rider crosses leaving it straight out). The joins themselves were
not the problem. Inside the world they were 600–1700 ft. The flat ground the user sees was:

1. **The nationals' own empty ground.** Each 1280² × 4.5 ft national was kept to its berm, keep
   `[34, 34, 1246, 1246]` ≈ 5450 ft square, but its gates, track and props sit in ~1000 ft in the
   middle. That leaves ~2200 ft of the map's own flat field on each side. Between Bodie (nat02) and
   Hillsboro (nat11) a rider crossed ~2200 + 594 + ~2200 ft of flat. New `Tile::trim_cut(trim)`
   (crates/stitch/src/lib.rs) cuts each side that is not ridged in to the bounding box of spawn and
   placements, plus `trim` ft. It never cuts inside the berm band, and rock is still cut after.
   world_mxu.json asks for 650 ft, so nat02 is now keep `[376, 390, 896, 893]` (≈2340 ft). The
   freerides have ridges and keep their cut. The 47 placements dropped are the same 47 as before.
2. **The grid's empty cells.** Five freerides (5337 ft) and 22 nationals in a 6 × 5 grid left three
   cells empty. The seam crossings next to them were 6832–6864 and 20076 ft.
3. **Per-line gaps.** One steep pair widened a whole line.

New `"layout": "units"` (`units_layout`, crates/stitch/src/build.rs):

- `unit_spans` gives each map a whole number of grid units a side: a unit is the median side plus a
  gap. The freerides come out 2 × 2 and the trimmed nationals 1 × 1. `unit_pack` packs them, with the
  spec's `middle` freerides in one block, into the smallest grid that fits (7 × 6 here).
- Places on each axis come from facing pairs, not lines. On x, maps that share a unit row face each
  other; on z, maps whose kept x ranges come nearer than a gap (round the x wrap) do. Each pair is
  given `gap_for` its own two edges.
- Each axis is packed forward and backward, and the slack is shared among a column's gaps. Every line
  no map spans across is tried as the wrap seam, and the shortest world is kept.
- When the slope check fails, `Plan::widen_at` widens only the gaps past the map nearest the steep
  spot (`Widen::after`), not every gap.

The first full build of that layout had **new seam failures**: steepest step 5.73 ft (it was 0.96),
141195 steps over 3 in, and slopes of 7–9 on the crossings over the wrap. The cause: stitch's join
field is not wrap-aware. The ground just inside the far edge blends that side's nearest maps, and the
ground at 0 blends its own. openmx's one-cell seam strip then climbs between, for example, a freeride
edge at ~248 ft and a national at ~140 ft. The old grid hid this by chance: only nationals were on
its seam lines. Fix: `wrapped_copies` adds a copy of every source one world over on each wrapped
axis, and diagonally, where it comes within half the world. Only the unit layout does this, so
world_mxu_free and world_mxu_nat are byte-for-byte unchanged. A map and its copies share one radius,
the widest any of them sampled. Collars, `join_widths`, `widen_at` and the seam-gate todo use only
the real maps. A piece's look adds a copy's share to its map's. Restricting the seam to lines with no
freeride beside them was tried before this and made the layout worse (2268 ft inside, 3087 ft over
the wrap). With 7 columns every x seam touches the freeride block, so that change was reverted.

## Before / after

| | HEAD | now |
|---|---|---|
| world | 37720 × 31456 ft, 796 joins, widen 1.953 | 21853 × 19828 ft, 480 joins, widen up to 3.05 on one gap |
| area | 1186 M ft² | 433 M ft² (37 %) |
| crossings inside the world | max 1719, mean 1010 ft | max 1364, mean 675 ft |
| crossings over the wrap | 730–20076 ft | 1252–2750 ft |
| `world seams:` | no gaps, 50868 steps > 3 in, steepest 0.959 ft, 0.236 ft across the wrap | no gaps, 2635 steps > 3 in, steepest 0.614 ft, 0.057 ft across the wrap |
| `world holes:` | none | none |
| stitch's far slope (gate) | ≤ 0.5 | ≤ 0.5 |
| steepest crossing slope | 1.806 (nat14 > nat12) | 2.329 (nat14 > nat07, 482 ft of join) |
| spawn | mxu_free07 | mxu_free07 |

Each map side's crossing, `to` the map it reaches, ft of join. "(wrap)" means it goes round the
seam. Map ids without `mxu_`; nat02 is Bodie, nat11 Hillsboro, nat18 Manassas Park
(crates/openmx/src/worlds.rs). These widths are join only. Before, each national side also had
~2200 ft of its own empty ground; after, ~650 ft.

| axis | map | before: to, ft | after: to, ft |
|---|---|---|---|
| x | free06 | nat22, 1008 | free10 (wrap), 1252 |
| z | free06 | free09, 711 | nat12, 720 |
| x | free07 | free09, 999 | nat04, 837 |
| z | free07 | nat19, 1017 | free09, 423 |
| x | free09 | nat02, 1008 | nat15, 927 |
| z | free09 | free11, 1143 | nat14 (wrap), 2112 |
| x | free10 | free06, 999 | free07, 423 |
| z | free10 | free07, 711 | free11, 1143 |
| x | free11 | nat20, 1012 | free09, 567 |
| z | free11 | nat07 (wrap), 6841 | nat08 (wrap), 1936 |
| x | nat01 | nat13 (wrap), 730 | nat22, 558 |
| z | nat01 | nat04 (wrap), 6832 | free07, 1364 |
| x | nat02 | nat11, 594 | nat08, 576 |
| z | nat02 | nat20, 963 | nat19, 716 |
| x | nat03 | nat09 (wrap), 730 | nat11, 576 |
| z | nat03 | nat11, 680 | free06, 657 |
| x | nat04 | nat21 (wrap), 860 | free06, 886 |
| z | nat04 | nat03, 1242 | nat16, 436 |
| x | nat05 | free07, 1089 | nat01, 639 |
| z | nat05 | nat08, 1233 | free10, 922 |
| x | nat06 | nat05, 1161 | nat17, 464 |
| z | nat06 | nat13, 1305 | nat15, 576 |
| x | nat07 | nat16, 1035 | nat10, 472 |
| z | nat07 | free06, 1310 | nat22, 1166 |
| x | nat08 | nat19, 1305 | nat14, 468 |
| z | nat08 | nat12, 1053 | nat05, 513 |
| x | nat09 | nat18, 810 | nat20, 482 |
| z | nat09 | nat06, 1026 | nat03, 1098 |
| x | nat10 | nat15, 1508 | nat09, 666 |
| z | nat10 | nat18, 1300 | nat21, 495 |
| x | nat11 | nat06 (wrap), 1045 | nat19 (wrap), 2750 |
| z | nat11 | nat01, 1004 | free06, 1332 |
| x | nat12 | nat17, 1458 | free11 (wrap), 1585 |
| z | nat12 | nat10 (wrap), 784 | nat18, 392 |
| x | nat13 | nat08, 1125 | nat18, 400 |
| z | nat13 | nat14, 666 | nat09 (wrap), 2260 |
| x | nat14 | nat12, 810 | nat07, 482 |
| z | nat14 | nat21 (wrap), 784 | nat01, 414 |
| x | nat15 | nat07, 1161 | nat13, 958 |
| z | nat15 | free10, 1719 | nat10 (wrap), 1436 |
| x | nat16 | nat04, 594 | free06, 1026 |
| z | nat16 | nat22, 1170 | nat06, 855 |
| x | nat17 | nat14 (wrap), 20076 | nat12, 518 |
| z | nat17 | nat15 (wrap), 1117 | nat13, 338 |
| x | nat18 | free10, 1089 | free11 (wrap), 1490 |
| z | nat18 | nat05, 666 | nat20 (wrap), 2332 |
| x | nat19 | free11, 1017 | nat05, 567 |
| z | nat19 | nat17, 612 | free10, 608 |
| x | nat20 | nat01, 598 | nat02 (wrap), 2472 |
| z | nat20 | nat16 (wrap), 6864 | nat11, 508 |
| x | nat21 | nat10, 976 | nat03, 554 |
| z | nat21 | nat09, 1296 | nat04, 630 |
| x | nat22 | nat03, 594 | nat21, 531 |
| z | nat22 | nat02, 666 | free07, 801 |

before: 54 crossings, inside max 1719 mean 1010, over the wrap 730–20076, steepest crossing slope 1.806

after: 54 crossings, inside max 1364 mean 675, over the wrap 1252–2750, steepest crossing slope 2.329

## How it was proved

- New `crates/stitch/tests/units_wrap.rs`: six generated maps, every other one 60 ft higher, unit
  layout wrapped xz. The ground at the far edge equals the ground at 0 within 0.05 ft on 200 points
  per axis, and every side faces a map within 1500 ft. With `wrapped_copies` disabled it fails:
  "the ground steps 64.341 ft across the wrap".
- `cargo test --release -p stitch`: all pass.
- `cargo test --release -p openmx --bin openmx disc_worlds`: 3 pass. Among them the new
  `world_mxus_widest_join_is_narrow` lays out the disc's 27 maps (`stitch::build::layout`): every
  side faces a map, widest inside 1278 ft (≤ 1800), widest over the wrap 2740 ft (≤ 2900).
- `cargo test --release -p openmx --test disc_worlds`:
  - `world_mxu_has_every_freeride_and_national_with_the_freerides_together` (now also: 54
    crossings, each ≤ 1800 ft inside and ≤ 2900 ft over the wrap, the freerides one cluster) passes.
  - `world_mxu_nat_is_the_22_nationals` passes.
  - `world_mxu_free_is_built_off_the_disc_as_openmx_mods_builds_it` **fails on mod.json**. This is
    not this change. The main checkout's installed world_mxu_free was rebuilt by a newer openmx-mods
    stitch whose mod.json carries MD57's `relief` block and its todo line, which this repo's stitch
    does not write. I built world_mxu_free with a HEAD binary into a scratch folder: same cache key
    (b3be54aa86e0f0ed), and `diff -rq` against this branch's build reports nothing.
- The two full world_mxu builds are compared above: HEAD's binary, then this branch with
  `--seams`. Exit 0, no holes, no gaps.

## Not verified / not 1:1

- Not ridden or looked at on screen. Everything here is numbers from `--seams` and the tests.
- **The user will not see this yet (B2).** `~/repos/openmx/mods/tracks/world_mxu` is still openmx-mods'
  old stitch ("every freeride and national", widen 5.96, 1222 joins), and it shadows the disc build.
  B2 already asks a person to remove or restitch it. I left the main checkout alone.
- Over the wrap a join is still 1250–2750 ft. game-sim's `Tiled::new` refuses a negative tile `at`
  (MD61), so the first map on each axis cannot start before its own ground. A trimmed national's
  ground runs ~1750–2000 ft before its interior.
- The steepest crossing slope went from 1.81 to 2.33, on nat14 > nat07 (482 ft of join, near nat07's
  239 ft seam gate). It is inside the seam gates, where stitch never held the ground to max_slope.
  Away from the gates the solve is ≤ 0.5, as before.
- `trim` 650 ft is a choice. It keeps the berm and some run-off round each course, and it was not
  tuned against how a rider uses that ground.
- world_mxu_nat keeps the plain grid; it was not in the brief and was not measured.
- openmx-mods has not been taught `trim` / `layout` / `wrapped_copies`. A world_mxu restitched there
  would not match this one.

## Next

none
