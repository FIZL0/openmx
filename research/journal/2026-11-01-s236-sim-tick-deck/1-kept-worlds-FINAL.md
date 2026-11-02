# 1 — kept worlds (FINAL)

Task S236. 2026-11-01.

## What I did

- `flight_search.rs`: a thread-local list of kept worlds (`Kept`, at most 8, newest first). Each is
  keyed by a stamp holding every bit `wooded_of` reads except the probes' reach and the bikes'
  transforms: toys, field, the props' (25 words each: moving or not, radius, root box or centre,
  `to_world`), the plants' and the bike count. `kept_world` serves a search like this:
  - An equal stamp means the kept world is used as is.
  - A stamp that differs only in props' words means `Kept::update`: the moved props get their new
    reach, box centre and tree keys, the tree is rolled back to the first moved prop's insert and
    re-filed from there, and the generation is bumped. It refuses, and a full build follows, when a
    prop became or stopped being a toy, or when the moved boxes change the box the tree is laid over
    (the tree's cell size depends on it).
  - Anything else means a full build (`Kept::build`), filed item by item with every change logged.

  Every search, including one on a kept world, then re-places the probes and the bikes (`place`,
  which includes the moving list's bike slots) and rewinds the broad phase's query state to a fresh
  build's (`rewind`: stamps, `current`, the tree's walk fields, the list's cursor).
- `broad_build.rs`: `insert_logged` and `undo`. Inserting records each node, child, slot and chunk
  change, and undo pops them newest first. Nodes freed by an undo are reused by the next logged
  insert, so a steady ride allocates no tree nodes. `insert` is unchanged (`insert_into(.., None)`).
- The searches' collidable list is lent (`lend` / `give_back`, kept by generation). One lent again
  for the same generation has only what a search writes put back (`flags`, `group`, `radius`), not
  the whole ~1500-entry list re-copied.
- `Scene`'s per-prop facets are a map filled on first touch instead of cloned per search.
- `world_builds()` (builds, updates), `forget_worlds()` and `kept(..)` are public, for tests.
- Profiling: perf is blocked here, so I sampled a line-tables release build with `eu-stack` under
  `PR_SET_PTRACER`. Before: `Scene::of` → `wooded_of` was ~50% of the tick and `Filing::file` ~35%.
  P205's cache of filed trees almost never hit, because the path cars move every tick. A first
  version that kept worlds only while nothing moved bought 20% (2.00 → 1.59 ms). The incremental
  update is what took the rest.

## Numbers (this machine, not the Deck)

`OPENMX_THREADS=1 openmx --headless --frames 2400 --steady-clock --size 320x224 --no-sound --timings`,
session line:

| ride | before sim ms/tick | after | ride hash (both) |
|---|---|---|---|
| free09 Supercross01[Quadzilla], `--play first_recording.p2m2` | 2.05 | 1.12 | 0xb43dfc5208c86f08 |
| sx01, a 60 s generated gas-and-weave tape, `--controls original`, 1800 frames | 1.39 | 0.71 | 0x6b2ec2fd94eabe6b |

Over 16.7 ms: 0 before and 0 after on both. The worst frame is about the same (11.6 → 11.3 ms on
free09). It is the first frames' loading, not a steady tick, and this desktop has no hitches to count.
Plane hash 0x3fcc295c14e13abd before and after.

## How it was proved

- `tests/kept_world.rs` (all of it generated in the test: 40 posts, 4 path cars, an 80-plant
  two-species ecosystem, two bikes, a ray cross):
  - `the_kept_world_is_the_fresh_one`: 30 ticks × field {box, edgeless, none} × plants {no, yes} ×
    bikes {no, yes} × toys {no, yes} × two reaches. The kept `(Broad, Vec<Collidable>)` equals
    `world_wooded`/`world_flown`'s, every field included. The ride has a car leaving the box, a car
    stopping (no longer a toy) and starting again, a still post moved for a tick, and a bike leaving
    the world on odd ticks. It also asserts the update path ran at least 20 times per configuration.
    A mutation that skips the tree's re-file fails it at tick 1.
  - `the_kept_world_searches_as_the_fresh_one`: each tick's `refresh_flown` nearby list equals a
    gather on a fresh `world_flown`. The plane scene's ray hits, searched twice on the kept world (the
    second time on a re-lent list) and once after `forget_worlds`, are identical.
  - `a_steady_tick_builds_no_world`: a tick of 4 wooded scenes, a refresh and a flown scene builds no
    world once warm. With the cars still it updates nothing; with them driving it updates exactly 2
    (the ride's world and the plane's) and builds none.
- `./kit check`: 3505 tests passed (camera and tick untouched, not run); `-p game-sim` alone: 921 passed, the ride/flight/wreck/rider-contact/prop-world
  suites included.
- The ride hashes above, the old binary against the new.

## Not verified / not 1:1

- **The Deck itself was not measured.** I have no access to it. Its 17-24 ms hitches should shrink in
  proportion (the build was ~half its tick per the plan's profile), but whether every tick now fits
  11.1 ms is unproven. Someone has to rerun `--timings` on the Deck, Supercross01[Quadzilla]
  freeride, and record the worst and the over-16.7 count.
- The resetting of a re-lent collidable's `group` is not exercised by any test. A search writes it
  but none reads it, so no search result can show it. It is reset anyway, to stay identical to a
  fresh copy.
- A props set that changes length (a world of tiles handing a different near set) or any non-prop
  change is a full build, as before. That is correct but not faster. MD28 worlds of tiles cross a set
  boundary rarely.
- `back_search.rs`'s `world_flown` (rare: put-backs) still builds afresh.
- Physics is unchanged by construction (the same world bit for bit), and checked by the two ride
  hashes and the game-sim suite. No PCSX2 comparison was needed or made.

## Next

Measure on the Deck. If ticks still run past ~11 ms there, the plan says `ps2_float` is its own task.
