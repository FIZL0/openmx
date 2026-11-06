# 1 — part search in rust (FINAL)

Task F46d. 2026-11-06.

## What I did

- `crates/game-sim/src/atv_part.rs` (new) holds the bike's part search:
  - `PartRays` is the part probe's `world_search::Narrow` for `atv_search::search`. Its clear is `ray_probe::clear`. Its pair sets the pair's first to 1 and then tests the other by kind: a sphere (kind 5) with `sphere_reaches`, everything else with `ray_probe::pair` on the `Shape` the caller gives.
  - `Outside` is the caller's world: `shape`, plus `sphere` (`None` unless the other is a sphere). It also covers the group parts, the listeners and the callbacks, which `PartRays` hands on to it.
  - The sphere test, three pieces:
    - `sphere_reaches` walks the probe's tree;
    - `node_reaches_sphere` tests a node box in the probe's frame;
    - `segment_reaches_sphere` tests a world segment.
- `tests/mxvatv_bike_ride.rs`:
  - `part_search_in_ram` does the record's part search in Rust over RAM:
    1. It reads the world (F46c's `sweep_world_in_ram`, plus each plant group's species parts as collidables, plus everyone's listeners and callbacks).
    2. It runs `atv_search::search` with `PartRays`.
    3. It writes back the flags, the part groups, the clear's globals, the pair's smooth global and zeroed words, and the whole record.
  - `PartWorldInRam` is the `Outside`. It reads meshes and spheres from RAM at each pair. Two kinds of call are handed to the original and counted with `> record search …`:
    - the part stand;
    - the listeners and callbacks. The flag bytes are written first, and it asserts no listener list changed.
  - `RecordsInRam::search` uses it under `in_rust("record search")` when the search is nearby.
  - Three new tests:
    - `ride_replays_with_the_part_search_in_rust`;
    - `flip_replays_with_the_part_search_in_rust` (PartMove, Gather, the sweep and the part search all in Rust);
    - `sphere_tests_are_the_originals`.
- The addresses and layouts are in research/oracle.txt under `# F46d`.
- Filed F52 (the handed-on part stand and listeners in Rust) and F53 (the pair kinds C15 never makes).

## What the original does

- **The part** is the record's `+0x200` ray probe: the riding part, or the wreck part while wrecked. It is MX Unleashed's ray probe with its fields moved up `+0x10`.
  - **The clear** (`./kit dis 0x001bbbc0 -p atv`) is MX's clear, plus two zeroed globals, the record made current, and a filter reset if there is a filter.
  - **The pair** (`./kit dis 0x001bc880 --len 0x700 -p atv`) sets a global to the smooth flag, zeroes the record's per-contact words, sets first = 1, then switches on the other's kind: 0 mesh, 1 hierarchy, 5 sphere, 6 its own test, anything else 0.
- **The sphere test** (`./kit dis 0x001095d0 --len 0x1b0 -p atv`) walks the probe's tree. A node's box is tested against the sphere in the probe's frame: the sphere is taken in by the transposed frame, out on any axis more than the radius past the box, then in if the corner distance squared is under r². It goes to the second child only if the first missed. A segment leaf is taken to the world, then:
  - in if either end is within r²;
  - otherwise in if the centre projects onto the segment (t ≥ 0, not past its length) within r² of the line.

  It writes no contact.
- **The search** is `atv_search::search`. The riding part's flags are `0x02` with no NO_GROUPS, so it does search the plant groups: each species part is stood at the plant, then paired.
  - On a hit, the probe's two listeners hear it. One is the bike's own (it acts only when a mask matches). The other pushes `{probe, struck, 0, kind}` onto a list.
- **The brief's "never hitting on C15" is wrong.** The part strikes plant spheres: in ride-2, 432 pairs and 141 struck; in flip-1, 161 and 55. Every pair on C15 is a sphere, never a mesh, hierarchy or kind 6. No struck collidable has listeners or callbacks.

## How it was proved

- **Lockstep bit-exact replays** through `replay_in_rust`. Every row's words match C15's, and the whole RAM (outside the stack) matches the run that hands calls on, after every frame:
  - ride-2 (459 rows): 459 part searches in Rust; pairs pinned at 432 spheres and 282 listener calls (141 hits × 2).
  - flip-1 (465 rows, wrecked on row 125, put back on row 193): PartMove 771, Gather 465, sweep 465, part search 465; 161 sphere pairs and 110 listener calls.
  - `./kit check -- -p game-sim --test mxvatv_bike_ride -- part_search_in_rust sphere_tests_are` passes 3 tests.
- **The tests are live.** Before the sphere test was ported (spheres answered "nothing struck"), ride-2 failed on row 30. RAM differed at the listener's pair list and the probe's hit bit.
- **Function oracle** (`sphere_tests_are_the_originals`): 40 000 random segments through `0x00106180` and 40 000 random node boxes through `0x00106e30`, against `segment_reaches_sphere` and `node_reaches_sphere`. The inputs are eighths with low bits flipped, signed zeros, zero-length segments, and r² equal to an end's distance². The answers are the same every time, with both answers seen more than 1000 times each.

## Not verified / not 1:1

- **The plant part stand is the original's.** `PartWorldInRam::part` calls it in RAM, and what it writes is read back. It is not ported. F52.
- **The listeners and callbacks are the original's.** They are called in RAM with the flag bytes written first. Two things are unchecked:
  - Whether the bike's own listener ever takes its masked branch on C15: it is not counted.
  - What a listener reads beyond the flags: the record is written only after the search, so a listener that read the record mid-search would see the old one. C15's RAM matches, so none does here.
  - F52.
- **Mesh pairs (kind 0) never happen on C15.** `ray_probe::pair`'s mesh path and the write-back of mesh contacts to the record are unproven for this probe. F53.
- **Hierarchy (kind 1) and kind 6 pairs never happen on C15.** The test panics on them. Kind 6's own test is not ported: `PartRays` answers it as nothing struck, which is wrong if it is ever reached. F53.
- **The per-ray filter (`+0x188`) is never set on C15.** Its reset and test are not ported, and this is asserted. F53.
- **The probe-tree sphere walk** (`sphere_reaches`' recursion and leaf transform) is proved only by the lockstep replays. It has no random-tree oracle. F53.
- **The global search** (nothing nearby) is still the original's. It never runs on C15, so nothing could check a port (F51 files the sweep's global search, the same original call).
- **The struck's listeners, `on_hit` and `on_struck`** are read and wired, but C15 never calls them.

## Next

Nothing for F46d. F52 and F53 are the follow-ups.
