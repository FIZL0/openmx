# 1 — plane tick on big worlds

Task MD34. 2026-10-28.

## What I did

**Reproduced first, at HEAD (d1f43535).** I used a release build, headless 320x224, world_ultra
from `~/repos/openmx/mods`, `--vehicle biplane --timings`, and a full-throttle gas tape (MD23's line).
The machine's load was 22-30.

| start | sim, session average | sim, worst tick | where it went |
|---|---|---|---|
| where it parks now (MD24/MD32: 59150, 442, 22237) | 0.19 ms | 0.67 ms | flew on, 48 → 42 mph |
| `--fly-at 2304,2281,0,40` (MD23's spot, sinking to the ground) | 0.25 ms | 4.08 ms | landed at ~2774, crawled, edge-thrown at tick ~330 |
| `--fly-at 86692,22046,90,60` | 0.21 ms | — | flew |

MD23's 30-300 ms is gone. **V96 (6116667d, 2026-10-27) already found and fixed it**: the plane's two
broad phases a tick (`Flight::search` and the post's `refresh_flown`) were built from
`world.wooded()`, which on a tiled world is every tile's shapes and their seam copies, twice a tick.
Each slow frame owed the next more ticks. Since V96 they take MD28's near set
(`flight_search::near_world` → `World::wooded_near`). V96's proof is a wall-clock test that is
`#[ignore]`d and needs the user's mods (`crates/openmx/tests/flying_high.rs`). Nothing in the gate
kept a plane off the whole world.

**What a plane's tick still does per tile or per shape.** I measured this with a temporary timing
probe in `tiles.rs` (reverted) on world_ultra, with 19041 shapes and 4216 tiles and joins:

- **The near-set cut** (`Tiled::near`) is linear in *all* the world's shapes: a box test on each.
  It runs only when the plane's reach centre enters a 128-ft square it has not kept (8 are kept):
  13-97 µs a cut. The first cut also builds the box list, 10 ms, once a ride. The plane asks for
  at most two squares a tick (search and post), so this is ≤ ~0.2 ms a tick even at 1700 mph. I
  left it linear.
- **The set handed to the searches** is the near set: 256-272 shapes of 7308 in the test's world.
  It is bounded by the shapes near one square, not by the world.
- **The broad phase's cells** are cut from the field's box, which is the whole world's rectangle
  (`flight_search::tree_box`). On a big world the cells are coarse, so the gathered list's bound
  falls to its least (`nearby.bound` 19.75 on 3 rows, 30 on 9). That changes when the list is
  re-gathered, not what it holds. It is the same thing the original does between a small and a
  large disc map.
- **`Tiled::stream`** belongs to the shell, not to the plane's tick: it is the `stream` stage, not
  `sim`. It still walks all 4216 pieces a tick, but V96's square filter skips most of them. It
  measures the exact gap for the ~770-800 pieces within the drop reach (9000 ft): 0.5-0.9 ms a tick
  at load 22, 5-7 ms on the tick a piece loads. That is bounded by the reach's area, not by the
  world.

**Bound, in the gate:** `crates/game-sim/tests/plane_world_cost.rs`,
`a_planes_tick_asks_a_world_of_tiles_only_for_the_shapes_near_it`.

- The test world is generated: rows of 2295 x 567 ft terrain with a post every 40 ft (every third
  turned), 3 rows (2436 shapes) and 9 rows (7308).
- `Counted` wraps the `Tiled` and forwards every `World` method, writing down each question by name
  and the largest near set it handed.
- Two starts in the second row, both full throttle for 450 ticks:
  - *parked*: `Flight::place` on the ground, MD23's start. It rolls, takes off and climbs to 398 ft.
  - *sinking*: launched at 60 ft/s, 8 ft up, on a line of posts. It comes down, is grounded for 364
    ticks and touches posts on 267.
- Each start is flown on both worlds. On every tick the test asserts:
  - body, contacts, crash, attitude, air and surfaces are the same (`Debug`'s every digit);
  - the same questions are asked, the same number of times, with the same widest near set;
  - `collision`, `wooded` and `heightfield` (the whole world's sets) are never asked;
  - the widest near set is under an eighth of the deep world's shapes;
  - the plane came down (≥ 30 ticks grounded or touching).
- A tick asks at most 27 questions of the world, on both sizes.
- **Mutation check:** with `near_world` put back to `world.wooded()` (pre-V96), the test fails at
  `parked tick 0: the plane asked for the whole world's \`wooded\``. Reverted.

Nothing was taken from opentennis: it has no tiled worlds.

## What the original does

Nothing to match: the original has one map at a time. The plane's search over a single map is
unchanged. A disc map is one near set (`World::near_set` `None`), as V96 left it.

## How it was proved

- `./kit check -- -p game-sim --test plane_world_cost`: passes, and fails on the mutation above.
- The `--timings` runs in the table, on world_ultra at HEAD.

## Not verified / not 1:1

- **The test's flights are compared on their physics, not on the whole state hash.** The search's
  `nearby.bound` (and so when the nearby list is re-gathered) depends on the field's box, which
  grows with the world. So `hash::first_difference` parts on tick 0 at `collision.nearby_bound`,
  though the flight does not part in 450 ticks. Contacts, body, crash, attitude, air and surfaces
  are compared through `Debug`, which prints every f32 to round-trip but does not tell NaN payloads
  apart.
- **MD23's exact start was not re-run.** MD23 flew the plane parked at (2304, 2281). MD24/MD32 have
  since moved where it parks, so it now parks at (59150, 22237), and that was run. MD23's spot was
  flown with `--fly-at` at 40 ft, which sinks to the ground there. The test's *parked* start is the
  parked case on a generated world.
- **The near-set cut stays linear in the world's shapes** (13-97 µs a cut at 19041 shapes, a cut
  per new 128-ft square). It is bounded per square, not per tick, and small, so it was not given a
  grid index. A world with ten times world_ultra's shapes would pay ~1 ms a cut.
- **`Tiled::stream` stays ~0.5-0.9 ms a tick on world_ultra** (shell's `stream` stage, ~800 exact
  gaps within the 9000-ft drop reach). Not the plane's tick and not reduced here.
- **Timings were taken at machine load 22-30.** Absolute ms are not quiet-machine numbers. The
  gate's bound is counted, not timed.
- **A woken `.toy` prop on a tiled world still takes the whole set.** `traffic.rs` hands
  `PropSet::frame_among` `world.wooded()`, and `flight_search::movers` runs over every shape, on
  each tick a prop is filed (S131, landed today). That is not the plane's tick, so it is filed as
  **MD58** and not fixed here. V96's wreck-bike whole-set follow-up is likewise not this task's.
- The plane's `area` (the edge test) is left unset in the test, so the edge throw is not exercised
  over the generated world.

## Next

None for MD34. MD58 filed.
