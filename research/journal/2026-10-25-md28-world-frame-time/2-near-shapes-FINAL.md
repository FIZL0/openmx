# 2 — near shapes (FINAL)

Task MD28. 2026-10-27. Second session; file 1 is the first.

## What I did

**Where it stood.** File 1's work (nine fixes, the chunk-budget test) merged to main as 46830197 on
2026-10-25, but the task was never ticked: the gate was red only on S119, which has since been fixed
on main. The first thing this session did was run `./kit check --all` on bare main cc549863: green
(2541 tests, three lobby load flakes passing on their lone rerun).

**Measured again on today's main** (cc549863), because S83, S138, S114, S136, S141, MD29, MD43 and
MD44 have all landed since file 1's numbers. The run is the same as file 1's: a 1800-frame
full-throttle tape from the start line, headless offscreen at 1920x1080 against
~/repos/openmx/mods, `--timings`. The machine was **not quiet**: load average 30-53, with other
slots' gates running, so every number below is inflated several times over file 1's quiet ones.
Compare the maps against each other, not against file 1.

Session ms a frame, GPU, today's main:

| map | sim | stream | ground | hud | present | frames over 16.7 |
|---|---|---|---|---|---|---|
| free06 (one disc map) | 5.52 | 0.06 | 0.18 | 0.53 | 2.76 | 447 |
| world_mxu_free | 3.34 | 0.16 | 0.44 | 5.88 | 3.25 | 540 |
| world_mxu | **73.70** | 0.85 | 1.84 | 4.90 | 7.37 | 774 (worst 758 ms, sim 744) |

World_mxu was a slideshow again, and the cost was all `sim`: about 30 ms on a quiet stretch, and
spiralling to several hundred ms as late frames ran several ticks to catch up. In file 1 it was
0.31 ms.

**The cause, from stacks** (eu-stack sampling a line-tables build, `profiling-without-perf`): 56
of 67 main-thread samples were in `Rider::frame_bumped`, each in `flight_search::world_of`/`wooded_of`
building the plane's broad phase (`broad_build::mesh_box` and `insert` for every prop). Since S114
(the wheel probe), S138 (the body's scenery contacts) and S83 (the put-back query), the riding bike
searches the world through `World::wooded()` like the plane does. Those searches build their broad
phase fresh from **every prop the world hands over**, several times a tick (the wheels each
substep), and a world of tiles hands over `Tiled::every_shape`: all 27 tiles' shapes together. On a
disc map that set is one map's; on world_mxu it is the whole world's, every tick.

**The fix:** `World::wooded_near(at)` and `World::near_set(at)` (game-sim world.rs). These are the
bike's searches' own question: the props for a search around `at`. Every world answers with
`wooded()`'s whole set, as before, except `Tiled`. A world of tiles answers with a **near set**: the
shapes of `every_shape`, in its order, whose box on the ground plane comes within `NEAR_MARGIN`
(256 ft) of the `NEAR_CELL` (128 ft) square that `at` is in. It is cut once from boxes made once,
and the last 8 sets are kept (tiles.rs `Tiled::near`). A set is a pure function of its square, so
it is the same whichever rider asked first, after a rollback, or on another machine. The wheel
probe (`ride::wheel_hits`), the body's scenery (`rider_contact::scenery`) and the put-back query
(`ride::back_solid`) ask for it. The put-back query keeps its gathered list across ticks, naming
objects by their place in the set (`back_search::Cache`, on the wire). So when the bundle stands in
another square than the one the list was gathered in, the cache is dropped and gathers again. That
decision is made from the cache's own `at`, which is wire state, so it rolls back with it. The
plane, the wreck bike and every other user of `collision()`/`wooded()` still get the whole set:
their searches keep numbers across ticks, as `every_shape`'s note says they must.

**After** (same tape, same load band, 30-33):

| map | sim | stream | ground | hud | present | frames over 16.7 |
|---|---|---|---|---|---|---|
| free06 | 5.63 | 0.06 | 0.19 | 0.58 | 2.86 | 477 |
| world_mxu_free | **1.40** | 0.13 | 0.43 | 5.92 | 2.96 | 171 |
| world_mxu | **1.10** | 0.18 | 1.66 | 5.09 | 6.18 | 582 (worst 68 ms) |

The fix bought world_mxu's `sim` 73.7 → 1.1 ms a frame, and world_mxu_free's 3.3 → 1.4. A world's
tick is now cheaper than free06's, where the bike searches 353 shapes and 5600 trees as the
original does. Under this load free06 itself goes over 16.7 ms on 447-477 of 1800 frames, nearly all
on `sim`. Of world_mxu's 582 slow frames, 478 are led by `present` (the GPU frame, a fence wait in
the stacks) and 103 by `hud`.

**What is left on a world, from the stacks after the fix:** with `sim` gone, about 20% of the main
thread's samples are the world's traffic cars. `world_dress::cars_near` poses each car within
1500 ft every frame, and `toy::Path::pose` replays 48 warm-up frames, each one a `Tiled::ground`
query: about 49 ground queries a car a frame. That is `hud`'s 5-6 ms under this load. Disc maps no
longer pay it, since S81 the simulation poses their cars. File 1's fix (2) made this cost
1.52 ms quiet. Filed as **MD45** rather than done here, because it changes how a car's pose is
computed and wants its own proof that the poses do not move.

## What the original does

Nothing to port: the original has one map, and its bike searches that map's one set of shapes. On a
disc map the port still hands over the whole set, unchanged, so nothing the original does moves.

## How it was proved

- crates/game-sim/tests/tiles.rs `the_bikes_searches_over_the_shapes_near_find_what_the_whole_world_holds`:
  a five-terrain world with 760 posts in it, a post every 40 ft at the ground's height, some turned.
  It runs 4,000 put-back rings (a down probe and eight round it, the ride's record-update shape) at
  random spots, through `back_search::query`: the broad phase built, gathered and swept. Over the
  whole world's shapes and over the near set, the answer and every probe's hit, fraction, point and
  normal are the same bit for bit. 2,497 probes met a post and 1,065 rings were refused, so the
  rings do touch things. Each near set is in the whole set's order, and the smallest was 8 shapes
  of 760.
- `./kit check --all` green with it (2542 tests), including the world rides (world_streaming.rs, wrap_rest.rs,
  wrapped_world_draw.rs, the shell's world tests) and every disc replay. The disc replays are
  untouched by construction: only `Tiled` answers differently.
- The --timings tables above, before and after, back to back on this machine.

## Not verified / not 1:1

- **No quiet machine.** Both tables were made at load average 30-53 with other slots' gates running.
  Even free06 goes over 16.7 ms on a quarter of its frames, so I cannot show "no frame over 16.7 ms
  riding across a join" today, on either a world or a disc map. The before/after for the fix is
  back to back at the same load band, and the ratio (73.7 → 1.1 ms of `sim`) is far outside what
  the load moves. File 1's quiet tables stand for the rest of the frame.
- **The wheel probe's and the body's searches over a near set are not proved directly** against the
  whole set. Only the put-back query is, in the test. All three build the same broad phase
  (`flight_search::wooded_of`) over the props handed over, and gather around a probe whose reach is
  a bike's length plus the broad phase's 30 extra feet, far inside the 256 ft margin. A
  `flight_search::Rays` tree for a test needs the bike's ray file, which a game-sim unit test does
  not have.
- **Object numbers differ between a near set and the whole set.** A contact names its prop by its
  place in the set searched. Within a search that is consistent, and the put-back cache is dropped
  when the set changes. A tie broken by object number could order two equal contacts differently
  than the whole set would. The relative order of shapes is kept, so this should not happen, but
  it is not proved. Worlds have no original to match either way.
- **A world ride's per-tick state hash may differ from before this change** for the same input, if a
  near-set search ever answered differently than the whole set. The test says it does not for the
  put-back query, but no world-ride hash was pinned before, so this was not compared.
- **The traffic cars' `hud` cost on worlds** (about 20% of the main thread, 5-6 ms under load) is
  left for MD45.
- **`present` on world_mxu is about twice free06's** (6.18 against 2.86 ms under load). That is the
  GPU frame, mostly the fence wait, and is not measured apart here (V65's ground quadtree and the
  props' draws).
- **The software renderer was not re-measured** this session; file 1's table stands.

## Next

None: the task lands with this file. MD45 (the world's traffic car poses) is the follow-up.
