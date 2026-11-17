# 1 — seam joins like the others (FINAL)

Task MD102. 2026-11-16.

## What I did

- **Most of the brief was already done by MD155** (11-14). A wrapped unit layout's tiles may stand over
  the seam, and the slack is relaxed round the wrap. So a first line of nationals no longer leaves
  2,000+ ft of join over the wrap. This task measured what was left and fixed it.
- **Measured MD155's world_mxu** (release shell, cold build, `--seams`):
  - Widest crossing over the seam: 2,008 ft (nat16 > free06, x).
  - Widest inside: 1,620 ft (nat11 > free06, z).
  - Printing each join's width beside the gap its pair needs (stitch's own `gap`) showed two things:
    - The 2,008 ft was all need: 1,997 ft planned under that build's 3.8× widening of nat16's gap.
    - The other seam joins carried 150–330 ft more slack than the rest of their row. Row 0 had
      555–636 ft slack inside and 790 at the seam; row 5 had 470–564 inside and 833 at the seam.
- **The cause was in `units_layout`'s relax.** Each tile goes to the middle of the room its neighbours
  leave it, but the room started at the seam bounds (`end`, `period − end − width`). So a line's first
  tile measured its room from the seam, not from its neighbour round the wrap (at −232 ft in row 0).
  Its middle sat right of where equal slack puts it, and the seam's join took the difference.
- **The fix** (`crates/stitch/src/build.rs`): the room is measured from the neighbours only (round the
  wrap too). The middle is taken there, and only then clamped off the seam. A tile with no neighbour
  on one side keeps the old midpoint of the clamp range.
- **A small refactor:** the pair gap moved out of `units_layout`'s closure into `unit_gap`. A new
  `pub fn unit_gaps(tiles, p, widen)` gives every pair's gap, so a test can hold a join to its need.
- `stitch::VERSION` 5 → 6. The three `worlds/*.start.json` files were refreshed from cold builds in a
  scratch mods folder. The keys changed, and so did world_mxu's widening (the slope passes now end
  elsewhere).
- **Tried and dropped:** sharing the slack again in the world as placed, after the leaf rounding. It
  only saved one 72 ft leaf on two z seam joins.
- **Tests** (`crates/openmx/src/disc_worlds.rs`):
  - New `seam_joins_are_like_the_others`, over world_mxu, world_mxu_free and world_mxu_nat. Each is
    laid out as its build ends (its start file's widening). Every join must be at least its pair's
    need. No join inside may be over 1,800 ft. Per axis, the most slack over the seam must be ≤ the
    most slack inside + one leaf. On the old relax it fails: x, nat10 > nat09, 834 ft of slack
    against 636 inside.
  - MD155's `world_mxu_is_laid_out_evenly` now reads that same built layout through a shared
    `built_plan` helper. It used to read the coarse `layout()`, whose widening is not the build's.
    The new relax made one coarse join 1,508 ft (nat01 > free10), which is 1,004 ft in the build.
    The 1,500 ft cap is unchanged.
- Nothing in opentennis bears on this: openmx's own stitched worlds.

## What the original does

Nothing: world_mxu and the other stitched worlds are openmx's own, not the original's.

## How it was proved

- Cold release builds, `openmx --seams --headless --no-sound --frames 2 --track <w> --mods <scratch>`,
  before (MD155, stitch 5) and after (stitch 6):
  - **world_mxu, before:** 22,294 × 19,243 ft, 462 joins. Crossings: min 351, median 873, max 2,008
    (over the seam). Widest inside 1,620.
  - **world_mxu, after:** 21,511 × 18,730 ft, 440 joins, spawn mxu_free07, no gaps, 101 s.
    Crossings: min 342, median 783, max 1,310 (nat18 > free11, inside). Widest over the seam 1,297
    (nat16 > free06). The other ten seam crossings are 482–1,230 ft.
  - The task's test holds: no seam crossing is wider than the widest stitched join inside, and none
    inside is past 1,800 ft.
  - **world_mxu_free, after:** 27,585 × 5,589 ft, 47 joins, no gaps. 135 ft inside, 360 over the seam
    (as under MD155).
  - **world_mxu_nat, after:** 62,218 × 12,646 ft, 752 joins, no gaps. Widest crossing 1,279 (z, over
    the seam), widest inside 1,197.
- `seam_joins_are_like_the_others` prints, per world and axis, the most slack inside and over the seam:
  - world_mxu x: 555 inside, 512 seam. z: 699 inside, 555 seam.
  - world_mxu_free x: 15 inside, 114 seam (inside one 144 ft leaf).
  - world_mxu_nat x: 78 inside, 6 seam. z: 1,005 inside, 969 seam.
- `./kit check -- -p stitch` (24), `-p openmx --bin openmx disc_worlds` (5) and `-p openmx --test
  disc_worlds` (4: the rides on all three worlds, start-file keys, crossing caps) pass. The gate is
  `./kit done MD102`.

## Not verified / not 1:1

- **Not ridden or seen on screen or on a phone.** Only built headless and read by `--seams` and the
  plan.
- **world_mxu_free's seam join is 360 ft against 135 inside**, more than "stitched + the 135 ft strip"
  (270). In stitch it is 234 ft for a 120 ft need: 114 ft of slack where its joins inside have 15. The
  test allows a leaf (144 ft at its 9 ft spacing). Where those 114 ft come from (likely the first
  tile's origin rounded up to a leaf past the seam bound) was not traced. MD155 had the same 360. Filed as MD186.
- **"Stitched + strip" is read here as "no wider than a join inside the world":** per axis, the seam's
  slack over its pair's need is held to the most slack inside. Seam joins still carry their line's
  even share of slack, as joins inside do. They are not shrunk to their bare need, because the slack
  has to go somewhere in a wrapped world as long as its longest line.
- **The coarse `layout()` no longer stays within 1,500 ft** (1,508, nat01 > free10). MD155's test now
  reads the built layout instead, where that join is 1,004 ft. Nothing else reads `layout()`'s joins.
- **The user's installed worlds** (MD103 reinstalled world_mxu and world_mxu_free from the disc build
  on 11-16) are still stitch 5's layout until they are reinstalled from a stitch 6 build. openmx-mods'
  own stitch also needs this change to build the same worlds.

## Next

- For the human, after the merge, reinstall world_mxu / world_mxu_free in the main checkout from openmx's stitch-6
  cache (memory `reinstall-a-disc-world-from-openmx-cache`), and ride the joins over the seam.
