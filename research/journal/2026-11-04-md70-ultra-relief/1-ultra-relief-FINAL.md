# 1 — ultra relief (FINAL)

Task MD70. 2026-11-04.

## What I did

- openmx-mods `tools/stitch/src/ultra.rs` (commits a8dd342 and 19577ed, branch openmx-MD70, fast-forwarded onto openmx-mods main). world_ultra's own
  new ground between whole worlds was the flat blend. Past the worlds' seam gates it now carries
  their own relief, the way MD57 gave it to the per-game worlds' joins in build.rs. It uses build.rs's
  code: build.rs only gained `pub(crate)`, plus two splits (`Relief::of_nodes`, `Room::of_cells`).
- `World::relief`, once per world:
  - The world's ground over its whole rectangle, on 16 ft nodes.
  - Read from its tiles (inside their keeps) and its joins (outside every tile keep, and outside
    any earlier join's rect). One job per tile or join, summed in job order, so the result does
    not depend on the threads.
  - Then `Relief::of_nodes`, the same as a map's in MD57: box-smoothed twice, mean taken off, and
    held to 0.25 ft/ft by the two slope envelopes.
  - world_mxu 38.3 ft spread (33.5 kept), world_mxvatv 64.9 (62.6), world_untamed 105.1 (103.4).
    Each is at gain 1.00 with its steepest slope 0.246. All three take 9.7 s.
- `UltraPlan::relief_at` is added to the blend in `UltraPlan::at`, mirroring build.rs's
  `Plan::relief_at`:
  - Each world's relief is read at the point folded into its rectangle, over every wrapped image of
    the world.
  - Weights are 1/d², normalised Σwr/√Σw². A world more than twice as far as the nearest fades out
    by three times as far.
  - The result is eased under `cap·tanh(r/cap)`, with cap = 0.1 × room × the lesser of the distance
    past every world's widest gate and the distance to the map's edge. A negative relief's cap is
    also held to the blend's height.
  - It is 0 at the worlds' edges, so the seams where new ground meets a world are untouched.
- `Room` is built from the ultra coarse lattice (`coarse_lattice`, the blend alone).
  - Where the coarse pass, or a solved piece, still comes out over 0.5 with the relief, the build
    dents the room there. At most 60 dents, then the global relief scale steps down.
  - Dents carry over a rebuild at the same widen, and are cleared on a widen.
  - The gaps never widen for the relief: the layout is chosen on the blend alone, as before.
- mod.json gains `stitch.relief` = {scale, worlds: [{world, gain, spread}]}, plus a todo line.
- `stitch ultra` gains `--wrap x|z|xz`. The installed world_ultra wraps xz, so a hand rebuild needs
  it; build.sh's `stitch --mods` path already passed the wrap.
- Test `tools/stitch/tests/ultra_relief.rs`,
  `new_ground_far_from_every_world_has_the_nearest_worlds_relief`:
  - Two generated 2-tile worlds, with hills 12 and 3 ft, 5000 ft apart.
  - `stitch check` ok, far slope ≤ 0.5, relief scale 1.0.
  - A 600 ft window 2000 ft from both worlds has a spread of 3.69 ft against the hilly world's
    4.30 over its folded window, and the blend alone is under a tenth of that.
  - The files match `UltraPlan::ground_at` to 0.05 ft, and the relief is 0 50 ft from a world.
  - An ignored test, `the_installed_ultra_maps_far_new_ground_has_relief` (`STITCH_ULTRA=<dir>`),
    measures the real map.
- Notes: openmx-mods notes/ultra-map.md method step 9. Journal: openmx-mods
  journal/2026-11-04-openmx-md70-ultra-relief.

## What the original does

Nothing: the ground between worlds is generated, not any source game's.

## How it was proved

- `cargo test --release -p stitch` in openmx-mods: every suite passes, ultra_relief included, except
  the two tests in tests/wrap.rs, which fail the same way on untouched origin/main (see below).
- world_ultra was rebuilt from the installed worlds:
  `stitch ultra … --id world_ultra --sky mxu_free06 --wrap xz world_mxu world_mxvatv world_untamed`,
  in 2 min.
  - The layout is the same as before: widen 3.815, 115632 × 35154 ft, 77 tiles, 2265 copied joins,
    625 new pieces.
  - Relief scale 1.0 after 14 dents. New-ground slope 0.498 away from the worlds' edges, 0.448 near
    them (max 0.5).
- Far new ground, measured as the plane-detrended spread of 195 windows of 300 ft, each ≥ 1000 ft
  from every world:
  - before: mean 0.31 ft, median 0.27, max 1.41;
  - after: mean 3.89 ft, median 3.72, max 8.08.
  - For scale, MD57's far joins inside the per-game worlds came out at about 5 ft.
- `stitch check` of the new world_ultra with `--mods` at the installed worlds (75 min). These pass:
  - 718,876 tile seam points, worst 0.00446 ft;
  - 60,726 points where new ground meets a world's edge, worst 0.00354 ft and slope step 0.00153;
  - 33,510 wrap samples, worst 0.00351 ft;
  - join slope ≤ 0.500;
  - every copied join and tile byte for byte its source.
  The only FAILs are modcheck's "cannot prove the copy: no iso folder given" (all 77 tiles: 50 listed, "27 more"), because
  that run had no `--iso`. Rerun with `--iso iso` (the user's discs, 76 min): **ok**, every tile
  compared byte for byte with its disc, the same seam, wrap and slope figures.
- Installed into `~/repos/openmx/mods/tracks/world_ultra` (with its `previews/` kept) and openmx-mods
  `out/mods/tracks/world_ultra`, as hard links to the build. The old copies are in
  ~/repos/openmx-mods-md70/target/old-world_ultra-{out,openmx}.
- openmx, with OPENMX_WORLDS at the four installed worlds plus the new world_ultra:
  - `every_installed_worlds_tiles_read_as_alone` (--bin openmx, --ignored) passes, 281 s.
  - tests/stitched_seams.rs `the_stitched_worlds_have_no_gap_along_a_seam` on world_ultra passes:
    13,648 edges, 28.6 M rays, no gaps, 0.078 ft across the wrap. The steepest step, 0.700 ft, is
    inside world_mxu (mxu_j023_011 | mxu_nat20), a copied join.

## Not verified / not 1:1

- Not ridden in the shell. It was checked only by stitch check and the openmx world tests above.
- The relief is rolling ground only: a smoothed, slope-held copy of the worlds' heights. No
  tracks, jumps, ruts or objects are carried into the gaps.
- openmx-mods tests/wrap.rs `a_wrapped_world_*` (2 tests) fail: "elev_c12_a: at -144 is not a whole
  number of feet from 0 to 2^24" (game-sim tiles.rs, via openmx's World). They fail the same way
  on untouched origin/main: MD61's stitch half (openmx-mods 1d5fc4b) is in, but its game-sim half
  is still open in openmx. Not this change. world_ultra itself has no negative `at`, and openmx
  reads it (above).
- The installed mod.json's `"category": "World"` line (MD66) was put back by hand. A fresh build
  drops it until MD66 lands in openmx-mods.
- The relief cap's room is a coarse 18 ft lattice. 14 dents brought the new ground to 0.498, which
  leaves little headroom under 0.5.

- `./kit done` ran with `--skip-audit`: the audit's four address leaks are in
  crates/game-sim/src/atv_wheel.rs (MD75, 012f4c8f) and plan/DONE.md (lines 519, 533), all already
  committed on main before this task; MD70 touches neither file. The gate's tests passed (4789).
- One gate run went red on `online_car` `two_online_shells_drive_two_cars_at_once` (the monster
  807.9 against 806.2 ft on the two machines), with a load average of 23–44. It passed alone twice
  right after, and MD70 changes no openmx code: a load flake, not this task.

## Next

Nothing waiting.
