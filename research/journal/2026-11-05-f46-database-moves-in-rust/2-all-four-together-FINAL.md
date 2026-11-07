# 2 — all four together (FINAL)

Task F46. 2026-11-07.

## What I did

- F46a (part 1), F46b, F46c and F46d had landed (their own journals:
  `2026-11-05-f46b-gather-in-rust`, `2026-11-06-f46c-sweep-in-rust`, `2026-11-06-f46d-part-search-in-rust`).
  F46d's `flip_replays_with_the_part_search_in_rust` already ran flip-1 with all four names Rust's; no test
  ran ride-2 with them together (each F46 sub-task's ride test named its own call alone).
- Added `ride_replays_with_the_searches_and_the_database_in_rust` to
  `crates/game-sim/tests/mxvatv_bike_ride.rs`: `replay_in_rust("ride-2", ["PartMove", "Gather",
  "record sweep", "record search"])`, asserting 459 rows, no wreck, the four answered 918 / 459 / 459 / 459
  times by Rust, none of the four handed on, the part search's pairs plant spheres alone (432) and its
  listeners heard 282 times.
- No product code changed: the ports are F46a–d's (`atv_spatial`, `atv_world::gather`, `atv_sweep`,
  `atv_part`). Nothing taken from opentennis (its world has no spatial database or probe searches).

## How it was proved

- `ride_replays_with_the_searches_and_the_database_in_rust`: ride-2, 459 rows word for word against C15,
  and the whole RAM (outside the stack) alike with the handing run after every frame. Run outside
  `./kit check` with `--nocapture` to see it was not skipped: it printed `answered {… "Gather": 459,
  "PartMove": 918, … "record search": 459, "record sweep": 459 …}` and took 41 s.
- `flip_replays_with_the_part_search_in_rust` (F46d's): flip-1, 465 rows through the wreck (row 125)
  and the put back (row 193), the four Rust's 771 / 465 / 465 / 465 times; passes on this branch too.

## Not verified / not 1:1

- **Two calls inside the part search are still the original's**: the plant part stand (`> record search
  part stand`, 432 on ride-2) and the riding part's hit listeners (`> record search heard_hit`, 282). They
  are not among F46's four names, so F46 leaves them as they are. F54 files porting them.
- **Paths C15 never takes are not proven by these replays**, as the sub-tasks' journals list:
  - the database walk across cells, portals, list take-out/put and new blocks (F46a, part 1 above). MD75c1's
    oracle covers those over generated databases. The write-back asserts they did not happen.
  - the sweep's pair test, group parts, segment nodes and the global search (F51);
  - the part search's mesh, hierarchy and kind 6 pairs, the per-ray filter, the probe-tree sphere walk on
    random trees (F53). The part search's global search never runs either, and still goes to the original.
  - no search hits on ride-2 or flip-1, so a hit's answer is never checked.
- **RAM is the state shared between frames**: each port reads its world out of RAM and writes it back
  every call. Nothing holds the database, the tree or the lists in Rust across frames, so F31b5's pure
  frame still has to own that state once the interpreter is gone from it entirely.

## Next

Nothing for F46. F51, F53 and F54 are the follow-ups for what C15 does not reach or what is still handed on.
