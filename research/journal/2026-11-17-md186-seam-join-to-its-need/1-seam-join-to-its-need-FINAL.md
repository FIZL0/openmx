# 1 — seam join to its need (FINAL)

Task MD186. 2026-11-17.

## What I did

- **Traced the 114 ft.** I printed each world_mxu_free tile's spot through `units_layout` (axis x,
  one row of five 5,337 ft maps, 9 ft spacing, so a leaf is 144 ft):
  - Packed pass: the first map's interior starts at 72 ft. Joins inside are 135 ft (120 need +
    15 ft of leaf rounding). The seam join is exactly 120 ft.
  - The relax shares the line's 60 ft of slack, so the first map wants 86.5 ft.
  - The second pass rounds every origin **up** to a leaf (`place_axis` ceils). 86.5 can only go
    to 72 or 216 on that map's leaf grid, so it went to 216. Every map after it went one leaf later
    too, and the world's end bound (last interior + 18 ft) made the seam join 18 + 216 = 234 ft.
  - So the brief's guess was right: the first tile's origin rounded up a leaf.
- **The fix** (`crates/stitch/src/build.rs`, the unit layout's `pass`): the wanted start goes to
  the **nearest** leaf (`want − half a leaf`, then the old ceil), never below the hard bounds
  (the seam bound, and each facing tile before it plus their gap). The first, packed pass wants 0,
  so it is unchanged.
- `stitch::VERSION` 6 → 7, because it lays out all three disc worlds a little differently.
- All three `worlds/*.start.json` refreshed from cold release builds in a scratch mods folder.
- **Tests:**
  - `seam_joins_are_like_the_others` (`crates/openmx/src/disc_worlds.rs`): the allowance over the
    inside slack went from a whole leaf to **half a leaf**. The old 114 ft fails it (72 ft allowed).
    A strict 1 ft allowance fails world_mxu_nat's z seam by 18-45 ft, as rounding to whole leaves
    allows.
  - `world_mxu_free_is_built_off_the_disc_as_openmx_mods_builds_it`
    (`crates/openmx/tests/disc_worlds.rs`): now 48 joins, 27,477 × 5,589 ft (was 47, 27,585). The
    seam crossing cap went from 2,900 to 270 ft (twice one inside: a stitched join + the strip).
- Nothing in opentennis bears on this: openmx's own stitched worlds.

## What the original does

Nothing: the stitched worlds are openmx's own, not the original's.

## How it was proved

- `seam_joins_are_like_the_others`, slack past the need, most inside vs most over the seam:
  - **world_mxu_free x: 15 inside, 6 seam** (mxu_free11 > mxu_free06, 126 ft for 120). Was 114.
  - world_mxu x: 555 / 448. z: 987 / 870.
  - world_mxu_nat x: 78 / 74. z: 933 / 951.
- Cold release builds (`openmx --seams --headless --no-sound --frames 2 --track <w> --mods <scratch>`):
  - **world_mxu_free:** 27,477 × 5,589 ft, 48 joins, no gaps, 1 pass. Crossings: four at 135 ft,
    **over the seam 252 ft** (was 360).
  - world_mxu_nat: 62,101 × 12,664 ft, 760 joins, no gaps, 8 passes.
  - world_mxu: 21,529 × 19,324 ft, 449 joins, no gaps, 30 passes.
- `./kit check`: `-p stitch` (24), `-p openmx --bin openmx disc_worlds` (5), `-p openmx --test
  disc_worlds` (4: the rides on all three worlds, their start files, crossing caps) pass.

## Not verified / not 1:1

- **Not ridden or seen on screen.** Only built headless and read through `--seams` and the plan.
- **world_mxu's and world_mxu_nat's layouts also moved** (nearest rounding applies to every
  wrapped unit layout). Their tests still pass and their start files are refreshed, but I did not
  compare their crossings join by join with stitch 6. world_mxu's widest z join inside grew
  699 → 987 ft of slack (1,287 ft, mxu_free07 > mxu_free11), still under the 1,800 cap and
  MD155's 1,500 test.
- **Seam joins are held to half a leaf over the inside slack, not to zero.** Origins are on whole
  leaves, so a join can land up to a leaf off its share. world_mxu_nat's z seam is 18 ft over.
- **The user's installed worlds** are still an older stitch's layout. They need a reinstall from a
  stitch 7 build (memory `reinstall-a-disc-world-from-openmx-cache`). openmx-mods' own copy of
  stitch also needs this change to build the same worlds.

## Next

- For the human: after the merge, reinstall world_mxu / world_mxu_free / world_mxu_nat from
  openmx's stitch-7 cache and ride the free row's seam crossing (252 ft).
