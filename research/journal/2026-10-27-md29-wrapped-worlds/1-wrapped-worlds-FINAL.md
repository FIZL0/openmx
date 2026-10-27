# 1 — wrapped worlds (FINAL)

Task MD29. 2026-10-27.

## What I did

- **The switch.** A world file's `wrap` (`worlds.rs::wrap_switch`): absent, `null`, `false`, `"none"` or `""`
  is off; `true`/`"both"`/`"xz"` both axes; `"x"` or `"z"` one; `[bool, bool]` either. Off by default, and a
  world without it is exactly the world it was: no join, `Tiled::wrap` `[false, false]`, and every wrap
  path returns at once.
- **The seam joins** (`tiles/generate.rs::seam_joins`, `worlds.rs::close_seams`). A wrapped world's far
  edge has to meet its near one, so the shell makes a join terrain past the far edge on each wrapped axis,
  [`SEAM_CELLS`] = 15 cells wide at the coarsest whole spacing its pieces share. Its height runs straight
  from the far edge's ground to the near edge's, the corner across both. It goes in as one more
  `Origin::Made` piece list, so the world grows by 15 cells on that axis and the extent is still a whole
  number of feet. A world whose sides are not whole feet is refused (`Tiled::wrapped` errors). The shell
  prints `world wraps: x every N feet, closed by K seam joins`.
- **The sim** (`game-sim`):
  - `World::wrap()`, passed on by `Land`, `Wooded` and `Tiled`.
  - `off_map::outside_wrapped`/`outside_axes`: the edge test asks only the axes that keep an edge, and the
    biplane's `Area` test does the same (`flight.rs`).
  - `Tiled::fold`/`laps`: every ground, surface, shape and keeps query takes its point modulo the width,
    and shapes within reach of a seam are swept again one width off.
  - `Rider::wrap_round` runs at the end of `Sim::step`/`step_all` (and, now, the shell's `--trace` loops).
    When the chassis' centre of gravity is past a seam, the rider is moved back one width by
    `Rider::shift`: the chassis, the origin, the wheels' world points, the drawn skeleton, and the
    put-back record when it is on the far side too. The broad-phase cache is dropped. `Frame.seam` says by
    how many widths.
  - `nearest_copy`: other riders are taken at their copy nearest the rider.
- **The shell:** the chase camera's filters are carried by the same width on the tick a seam moves the
  bike (`main.rs::seam_moved` → `chase::carry`), so the camera does not swing across the map. The tick
  blend's own cut rule (`pace::Cut`) already treats the jump as a cut.

## What the original does

Nothing like this: its map edge throws the rider back toward the middle (`off_map.rs`, S59). Wrapping is
the port's own departure. It is layered: off, the edge test and every query are the original's, clause for
clause (`outside_wrapped` with `[false, false]` *is* `outside`).

## How it was proved

- `crates/game-sim/tests/wrap.rs`, 9 tests on generated worlds with a stock bike:
  - A ride east over the seam of a world two tiles wide, against the same ride on a world built one
    width wider. The two states are bit-equal up to the crossing. On the crossing tick every place is
    bit-equal to the wide ride's minus W, and velocity and attitude are bit-equal. After that the two
    stay within 0.05 ft over 90 ticks (see below for why not bit for bit).
  - 3000 ticks of laps with ≥ 2 seams and no throw, against the same world unwrapped, which throws.
  - Riding west over the near seam comes in at the far one.
  - `outside_wrapped` against `outside` at random points.
  - `fold` is exact.
  - A non-whole width refuses to wrap.
  - Seam joins meet both sides within 0.1 ft.
  - A 1500-tick ride over a made join and through the seam with no wreck at all.
- `crates/openmx/tests/wrapped_world.rs`, 3 tests through the shell binary and the disc bike, on a
  generated two-tile world with `"wrap": "x"`:
  - It loads 135 ft wider and says it wraps.
  - A 1200-tick `--trace` ride east crosses the made join and the seam, from 1268.1 to 3.07 on tick 339,
    on the ground and never down.
  - The same world without the switch has no join and throws the rider 150 ft short of its edge.
- `worlds.rs` unit test `the_wrap_switch_reads_its_spellings`.

## Not verified / not 1:1

- **After the shift the ride is not bit for bit the wide world's ride.** The crossing tick is bit-exact
  (x − W is exact for x ≥ W, a whole-foot W under 2^24). After it the rides part linearly, about 3.5e-4 ft a
  tick along x only. The cause is float magnitude: the same arithmetic near x≈0 and near x≈W rounds
  differently. A control with two *unwrapped* rides one width apart parts by the same amount
  (`a_ride_a_width_further_out_drifts_as_far_without_a_seam`). The task's "bit for bit after the shift"
  cannot hold for any shift of a float ride; what is proved is the shift itself and the bound.
- **The westward shift (x + W for x < 0) rounds**: x + W is not exact for small negative x. The eastward
  one is exact.
- **The comparison leaves out `ride.back_search`**, the broad-phase cache. Its bound is the world's
  extent, which necessarily differs between the two worlds compared.
- **The biplane's own state is not shifted**: its queries fold, so it flies on over a seam, but its
  coordinates grow and lose bits. Its seam shape copies at ±W round. Untested in flight. Filed as MD44.
- **The riding ragdoll and wrecks hold the shift**: the rider is moved only while plainly riding (no
  wreck, crash, ragdoll or riding body). The ragdoll is off by default (`OPENMX_RIDING_RAGDOLL`). A wreck
  past a seam is folded by the queries and shifted on the first riding tick after the put-back. MD44.
- **Bike-to-bike contacts are not taken at the nearest copy**; only rivals' positions are. MD44.
- **Seam-join normals are one-sided at a strip's edges**: the join is its own terrain, so its corner
  normals see only its own samples. The ride over it is smooth (the shell test stays within 6 ft of the
  expected ground and never wrecks), but the normals are not the blend a single terrain would have.
- **A hole at an edge**: where the far or near edge has no ground, the join's height falls back to the
  ground stepped inward up to one cell, or else 0 ft. Not tested.
- **The draw is not done**: no tile copies past a seam (the ground ends at the seam until the bike crosses),
  other riders are not drawn at their nearest copy, the radar does not wrap, and MD42's labels do not name
  the map across a seam. gpu.rs's ground is being rewritten by V65, which is not merged into main, so this
  half is filed as MD43 after MD29.
- **Online is by determinism only**: every peer shifts inside `step_all` on the same tick. No netplay test
  rides a seam.
- **The camera carry is unproved on screen**: the shell test traces positions headless and takes no
  pictures, so `seam_moved`/`chase::carry` is built and reasoned, not seen.

## Next

MD43 (the draw, after V65) and MD44 (the plane, ragdoll and contacts over a seam).
