# 1 — far join relief (FINAL)

Task MD57. 2026-11-02.

## What I did

- Both stitchers got the same code: openmx `crates/stitch/src/build.rs` (the disc worlds the shell
  builds, P232) and openmx-mods `tools/stitch/src/build.rs` (the installed worlds). openmx's
  `stitch::VERSION` went from 1 to 2, so cached disc worlds rebuild.
- `Relief::of`, once per map:
  - The map's ground over its keep, on a 16 ft grid (each node the mean of its cell).
  - Box-blurred twice over three nodes each way, and its mean taken off.
  - Then held to `RELIEF_SLOPE` (0.25 ft/ft) by two slope envelopes: the highest field under the
    ground no steeper than that, and the lowest field over it. The relief is their mean.
  - Where the map's ground is gentler, that ground is kept as it is. A peak is cut to a cone and a
    gully is filled to one.
  - The first try was a gain (scale the whole map down to its steepest slope). Real maps' jumps
    left gains of 0.09–0.35, and the relief came out 2–16 ft against the maps' 6–79 ft. The
    envelopes keep 6–76 ft (world_mxvatv: nat13 76 of 79, nat09 50 of 47, mc02 6 of 6.5).
- `Plan::relief_at` adds to the blend in `Plan::at`, and to the collar pull's target:
  - Each near map's relief is read at the point folded back and forth into its keep (as
    `terrain::mirror` folds the paint, MD48).
  - The maps are weighted `1/d²`, normalised `Σwr/√Σw²`. A map more than twice as far as the
    nearest fades out of the sum by three times as far.
  - The result is eased under a cap, `cap·tanh(r/cap)`, with `cap = 0.1 × room × the distance past
    every map's widest seam gate, or from the world's edge`.
- Not the paint's weights. Between nat04 and nat13 (an 1800 ft gap in world_mxvatv), the paint's
  `reach` hands the gap's middle from one map to the other within ~200 ft. Their reliefs there are
  −54 and +87 ft, so the swap made a 0.7 slope. Inverse-square weights spread the swap over the
  whole gap.
- The cap replaced a linear 600 ft fade-in past the gates. That fade times a 90 ft relief also added
  ~0.4 of slope. Under `tanh`, the cap's own slope adds at most a third of 0.1.
- `Room`: a lattice over the world, built from the coarse pass (18 ft) of the blend alone.
  - 1 where the blend is no steeper than 0.9·max_slope − 0.25, falling to 0 at 0.9·max_slope.
  - Then its least over 600 ft is taken, and it is box-smoothed twice, so the relief fades out
    before steep blend, not on it.
  - Where the coarse pass, or a solved piece, still comes out over max_slope with the relief, the
    build dents the room there (×0.75, 1200 ft radius). At most 60 dents, then the old global
    `relief_scale` step. A dent lowers the cap as well. By a gate or the world's edge the relief
    stands at its cap, which a lower relief alone does not lessen (world_mxu's j020_000 kept 0.524
    under repeated dents until the cap took the room too).
- The build never widens the gaps for the relief. It widens on the blend alone, as before (world_mxu
  still 5.96, world_mxvatv 1.95). The relief only ever lowers.
- mod.json `stitch.relief` = {scale, tiles: [{tile, gain, spread}]}, plus a todo line for it.
- Tests: `tests/relief.rs` in both stitchers,
  `a_join_far_from_every_map_has_the_nearest_maps_relief`.
  - Two generated 512-sample maps 5000 ft apart, with hills 12 and 3 ft high.
  - `stitch check` passes, far slope ≤ 0.5, relief carried at 1.0.
  - A 600 ft window 2000 ft past the hillier map (every point ≥ 1990 ft from every map) has a
    plane-detrended spread of 3.65 ft, against 4.11 over the map's own keep.
  - Without the relief the blend's spread there is under a tenth of the map's.
  - The files hold what `Plan::at` computes, within 0.05 ft.
  - The relief is 0 just outside every keep.

## What the original does

Nothing: the joins are generated ground, no source game's.

## How it was proved

- `cargo test --release -p stitch` in openmx (all suites, relief included) and in openmx-mods (the
  same, plus its world_* spec tests).
- The five worlds rebuilt with openmx-mods' stitcher (`stitch --mods ../openmx-mods/out/mods`), all
  five pass `stitch check` (world_ultra with `--mods` at the new worlds and `--iso iso`), no sample
  clamped. Steepest join slope past the seams: world_mxu 0.485, world_mxu_free 0.413, world_mxvatv
  0.487, world_untamed 0.498 (max 0.5), every relief scale 1.0; world_ultra's own new ground 0.466.
- Far ground relief, the mean plane-detrended spread of 300 ft windows ≥ 1000 ft from every map,
  before → after: world_mxvatv 0.14 → 5.23 ft, world_mxu 2.38 (median 0.29) → 5.92 (median 5.06),
  world_untamed 0.32 → 4.95, world_mxu_free 7.08 → 8.28, world_ultra 0.16 → 1.99 (median 0.16: its
  own new ground has none, MD70).
- Installed into `~/repos/openmx/mods/tracks/world_*` and openmx-mods `out/mods/tracks/world_*`
  (old copies kept in openmx-mods-md57/target/old-worlds*), with MD66's `"category": "World"` line
  put back into each mod.json by hand as MD66 did.
- openmx: `every_installed_worlds_tiles_read_as_alone` (--bin openmx, --ignored) passes, 345 s;
  tests/disc_worlds.rs' 3 pass, including the disc build of world_mxu_free coming out byte for byte
  the installed one; tests/stitched_seams.rs' 7 pass; tests/world_joins.rs' other 3 pass.
- Failing, the same on the old worlds (`OPENMX_WORLDS` at the old copies), so not this change:
  middle_spawn `the_installed_wrapped_worlds_start_on_their_middle_tile` (world_mxu starts on
  mxu_nat01, the test wants nat17) and world_joins `world_mxu_free_throws_only_off_its_outer_edge`
  (the world wraps, so nothing is thrown).

## Not verified / not 1:1

- world_ultra's new ground between whole worlds (`ultra.rs`) has no relief (filed as MD70). Its
  copies of the per-game worlds' joins have it.
- The carried relief is the maps' ground held to 0.25 ft/ft and smoothed over ~80 ft: jumps, ruts and
  berms are not carried, only rolling ground.
- Near a gate the relief is under 0.1 ft per foot past it, so the first few hundred feet past a seam
  gate stay close to the blend.

- Until MD57 merges, other slots' disc_worlds comparison reds: the installed world_mxu_free now has
  relief their VERSION-1 stitcher does not make.

## Next

- MD70: relief on world_ultra's own new ground.
