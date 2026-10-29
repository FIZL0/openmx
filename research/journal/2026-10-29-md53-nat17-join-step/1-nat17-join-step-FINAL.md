# 1 — nat17 join step (FINAL)

Task MD53. 2026-10-29.

## What I did

- **Measured first: the join does not miss the tile's edge.** On the installed world_mxu (C12's
  build), `--seams` printed `steepest step 4.142 ft at 18805.5,25407.0 (mxu_nat17 | j017_023)`.
  Through stitch's `openmx::load` + game-sim's `Tiled::at` (the ride's own lookup), across that
  edge at x 18805.5: z 25406 → 904.375 (nat17), 25406.875 → 889.891 (nat17), 25407 → 887.819
  (join), 25407.125 → 885.749 (join), 25408 → 871.257. The join goes on from the tile's edge in
  the tile's own line. The ground falls about 16.6 ft a foot there, and the walk's rays are an
  eighth of a foot either side of the seam, so it read the slope as a 4.14 ft step. stitch check
  agrees on the whole world: the worst height step at a seam is 0.003 ft.
- **Why it is steep there:** nat17 has a mountain about 900 ft square in its x1/z1 corner
  (samples ~1046–1242 each way). Every line across it for 219 samples in is 8 to 30 ft a foot, and
  stitch's berm cut (34 samples, just the shootoff band) ran through it on both sides. 85 points
  of that z1 seam were over 0.75 ft. Its seam gates were 1206 and 2808 ft, so the joins beside it
  were allowed to stay steep that far out.
- **Fix 1, openmx-mods 5afc80f, `Tile::find_cut`/`past_rock`/`rock_cut`:** a side whose cut line
  is steeper than `ROCK` (3 ft a foot across it, anywhere along the kept span) moves inward to its
  nearest gentle line. When several sides are steep, the one that costs the least ground moves
  first, and the check repeats. The 3 is the walk's 0.75 ft (a quarter-foot ray pair reads a
  quarter of the slope, and the join's first cell takes the tile's last cell's slope). nat17's x1
  moves 219 samples (985.5 ft), which also takes the mountain off its z1 line. Its gates go from
  392/315/1206/2808 ft to 378/270/266/288. Also moved: nat10 x1 1 sample, nat20 z0 2 and x1 1,
  nat21 x1 6; those lines were 3.05–3.72 ft a foot. No MX vs. ATV or Untamed tile moves, which I
  checked on all 50. The world's `todo` names each cut (`cut past a rock face … on x1 by 986 ft`).
- **Fix 2, same commit, `Source::extend`:** with only fix 1, the steepest step became 0.950 ft at
  a join|join seam (j013_022 | j013_023) 22 ft west of nat17's x0 edge. The join there rose 3.7 ft
  a foot along the edge, though the tile is 1.7 at the edge and 0.1 inside. The extension is the
  edge's height plus its last cell's cross slope times a taper of ~50 ft, and that slope is ragged
  from sample to sample. The along-edge blur (σ = d′²/(d′+fade), 1.5 samples at 22 ft out) left
  ribs. The slope is now blurred over σ = d′ and the height as before; both are 0 on the pinned
  ring, so the seam's height and slope stay the tile's (stitch check's seam numbers below).
- **Rebuilt and installed world_mxu.** It was built on C12's stitch (task/C12 b36970a, committed
  but not yet merged into openmx-mods main) with this patch, in a scratch worktree of openmx-mods.
  It replaced `~/repos/openmx/mods/tracks/world_mxu`; the old one is in
  `~/repos/openmx-mods/target/md53/installed_old/world_mxu`.
- Test `tools/stitch/tests/rock.rs`: a generated map with a rock corner (hashed per-sample relief
  up to 150 ft on a 300 ft block) is cut on x1 exactly past it (`rock_cut == [0, 0, 103, 0]`), a
  plain map's cut is unchanged, and the two built into a world have no gap and a seam walk
  (`Tiled::seam_walk`, the `--seams` walk) under 0.75 ft. With the rule off, the same test reads
  6.8 ft.
- Nothing in opentennis applies (it has no stitched worlds). No openmx code changed.

## What the original does

Nothing: the stitched worlds are generated ground (openmx-mods), not the original's. The tiles'
own ground is the disc's and untouched; a rock cut only keeps less of a map, and the world's
`todo` says how much.

## How it was proved

- `openmx --headless --no-sound --track world_mxu --mods ~/repos/openmx/mods --trace 1 --seams`
  (release binary built from this slot at 12535d22, 1 m 50 s), on the installed world: `world
  seams: 3203 edges, 7939684 rays, no gaps, … 11726 of them a step over 3 inches, steepest step
  0.700 ft at 25146.0,12762.0 (j023_011 | mxu_nat20), 0.106 ft across the wrap`; `world holes:
  none`. Before: 4.142 ft. Steps toward it: C12 + fix 1 → 0.950 (join|join beside nat17); + fix 2
  at ROCK 4 → 0.891 (nat21's 3.72 cut line); + ROCK 3 → 0.700.
- `stitch check` on the new world_mxu: ok. 246,856 seam points, worst height step 0.00299 ft,
  slope step 0.0013; joins of two spacings 0.00354 ft; the wrap's near edges against the far ones
  0.004 ft; join slope 0.491 away from the seams.
- stitch's whole test suite passes on openmx-mods main with the change, and on C12 + the change.
- The other worlds built on C12 + the change, steepest `--seams` step against the installed
  (C12) builds: world_mxu_free 0.276 (0.302), world_mxvatv 0.679 (0.679), world_untamed 0.419
  (0.419). Points over 3 inches: 36 (108), 7034 (7019), 2533 (2162).
- A 300-frame `--trace` on the installed world_mxu rides (phase Riding).

## Not verified / not 1:1

- **The installed world_mxu is built on C12's stitch, which openmx-mods main does not have yet**
  (C12 is committed on task/C12, unmerged). My commit is on main without it. The patch applied
  cleanly on C12 and the suite passes on both, but the world that main's stitch builds today has a
  steep wrap (8.05 ft at j003_029 | seam_z_13), and that is C12's to fix. Until C12 merges,
  `stitch --mods … --out …` from main does not reproduce the installed world.
- **Only world_mxu was reinstalled.** world_mxu_free, world_mxvatv and world_untamed were built
  and measured (above) but not installed, because their installed builds already pass and their
  cuts do not change. Their installed joins do not have fix 2 yet; they get it at their next
  rebuild. world_ultra was not rebuilt (about 5 minutes and 5 GB). It still carries the old
  world_mxu inside it, including nat17's rock seam.
- **The 0.75 ft holds on this layout, not by construction at every join|join seam.** Fix 2 makes
  steep joins rarer but does not bound them: the near-seam join slope is still 5.2 ft a foot
  somewhere (stitch's `near_slope`), within a tile's seam gate. A seam between two join pieces is
  just where the lattice cuts. A future layout (MD54 is changing the wrap layout in another slot)
  could put one across steep join ground and read over 0.75 ft again, without any mismatch.
- **`--seams`' "step" is slope plus mismatch.** Its rays a quarter foot apart read ¼ × slope on
  any steep ground. I did not change the meter (it is the task's test); a step measure that
  extrapolates each side to the seam would separate the two.
- **world_untamed has more seam points over 3 inches** with fix 2 (2533 vs 2162), with the same
  0.419 ft maximum. I did not look at where they are.
- **What the cut takes away:** 985 ft of nat17's x1 side (the mountain and the ground beside it
  along the whole side, x samples 1027–1246), and 4.5–27 ft of nat10/nat20/nat21. Placements past
  the new keep are dropped as for any cut (47 drops, the same count as before). I did not look at
  nat17's corner in PCSX2 or in a shot to see what scenery stood there.
- **openmx-mods' `./check.sh` fails, and not because of this change:** import-music `#[path]`-mounts
  openmx's `crates/openmx/src/audio.rs`, which now calls `crate::thread::spawn` (E0433). I left it
  alone because it is not this task's; the stitch crate's tests pass.

## Next

Nothing for MD53. Filed MD60: when C12 merges into openmx-mods, the next full rebuild (`stitch --mods … --out
…`) brings fix 2 to the other four worlds and rebuilds world_ultra.
