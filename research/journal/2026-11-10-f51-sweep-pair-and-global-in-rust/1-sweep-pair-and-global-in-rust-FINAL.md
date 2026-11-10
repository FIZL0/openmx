# 1 — sweep pair and global in rust (FINAL)

Task F51. 2026-11-10.

## What I did

- `crates/game-sim/src/atv_sweep.rs`:
  - `pair(probe, more, other)`: the sweep's pair test. It runs the sphere test, then the contact-list sweep through a mesh or through each part of a hierarchy, and answers per segment.
  - `More` holds what the ATV record keeps beyond MX's answer: the `+0xac` words and the hierarchy part that last took a segment.
  - `Sweep` now runs its own pair. `Outside::target` hands it the other object as a `Target`: a mesh, a hierarchy's placed parts, or nothing struck.
  - `search_world(objects, broad, probe, lazy, retired, sweep)`: the global search. It is MX's `probe_collide_world` with the ATV's differences, listed under "What the original does". `group_distance` is the ATV's group reach.
- `crates/game-sim/src/sweep_probe.rs`: `sweep_into`, `take_out_nearest`, `to_world` and `turn` are now `pub(crate)` so the pair can use them. Nothing else changed.
- `tests/mxvatv_bike_ride.rs`:
  - `SweepWorldInRam` is the narrow phase over RAM. It reads each pair's object (kind 0 mesh, kind 1 hierarchy and its parts, else nothing). Group parts, listeners and callbacks go through F46d's `PartWorldInRam`. It also notes the two globals the pair writes.
  - `sweep_all_in_ram` runs the global search over RAM and writes everything back.
  - `sweep_in_ram` (F46c) now runs the nearby search's pairs and plant parts in Rust. Its set and place moved to `sweep_set_in_ram`.
  - `RecordsInRam::sweep` answers both the nearby and the global sweep in Rust under `"record sweep"`. A hit's normal, point and struck object are read back as the original's caller reads them.
  - `GatherInRam::write_query` is `write` without the nearby list. It now accepts a tree query that leaves its list open (see below).
  - Three new tests:
    - `the_sweeps_pair_is_the_originals`
    - `the_sweeps_global_search_is_the_originals`
    - `the_nearby_sweep_with_its_pairs_is_the_originals`
- Addresses are in research/oracle.txt under `# F51`.

## What the original does

- **The pair** (`./kit dis 0x001b9a60 -p atv`):
  1. It notes the probe's smoothing word in a global first, even when it refuses.
  2. It refuses a null or not-nearby other.
  3. It zeroes the hit, owner and `+0xac` words for each segment, then runs the sphere test.
  4. It resets nearest to 1, the fractions to −1 and the hits to 0.
  5. Then, by kind:
     - kind 0: a mesh swept in its own space. `./kit dis 0x0010e570` → `0x0010c080` is `collide::sweep` bit for bit, and it notes the mesh's descriptor in a global.
     - kind 1: each hierarchy part the same way. A part that takes a segment sets the owner word and becomes `+0x84`.
     - kind 8: a finite plane.
     - anything else: nothing.
- **The set** copies one node pointer per point (or zero). A node's world matrix is at `+0x70`.
- **The global search** (`./kit dis 0x001148e0 --len 0x700 -p atv`) is MX's with these differences:
  - "active" is `+0x126` bit 1 *and* a space.
  - Retired items (first word 1) are skipped before they are marked searched.
  - Groups are searched only with NO_GROUPS clear. Their distance (`0x001169f0`) puts z first and is clamped at 0. The reach is near + blend·(far − near)/128.
  - The bounds go to `+0x50`/`+0x70`. The budget is `+0x74`, the last centre `+0x60`, and a hit sets last to (0, −10000, 0, 1).
- **The tree query is left open.** After the global or nearby walk, the quadtree's list stays open: its head points at the nodes just taken, and the pool keeps them taken. F47's gather ends with the list closed. The write-back now handles both.
- **A listener that searches.** A struck plant part's listener (vt `+0xc` = `0x00495690`) runs a database query of its own (the global stamp goes up by one) in the middle of the walk. The original's walk then goes on from the tree state that query left.

## How it was proved

All three oracles run on the state-4 RAM image: one Ee runs the original, the other runs Rust over the same RAM. After every case, all RAM outside the stack is compared byte for byte.

- **Pair**, 10,000 generated cases: nat05's database meshes, 12 turned copies, and 10 made-up hierarchies of 2–3 copied parts. They cover 1–16 segments, with or without nodes, zero-length segments, junk leftovers in the record, and nearby or not. 20,011 pairs: 4,966 struck, 1,677 refused, 1,397 struck through a hierarchy, 1,394 with the part kept, 1,596 other kinds, 10,696 on nodes, 156 at 16 segments.
- **Global search**, 1,500 searches with the record's own probe. Each segment runs through a nat05 mesh triangle, or lies anywhere up to 400 ft from the bike. One search in four is lazy, one in 15 inactive, and one in four searches the plants. Results: 693 cleared misses, 496 hits, 220 lazy, 89 inactive. That came to 1,798 mesh pairs, 30 plant part stands and 12 listener hand-ons. The answer's bits and the RAM matched every time, with 2 cases skipped (see below).
- **Nearby sweep with pairs**, 600 sweeps: the original's set, place and gather into the record's list, then `SEARCH_NEAR` against `sweep_in_ram`. 195 hits, 403 misses, 731 mesh pairs, 13 part stands, 2 skipped.
- **C15 replays still bit-exact** with the sweep in Rust: `ride_replays_with_the_sweep_in_rust` and `flip_replays_with_the_sweep_in_rust`.
- **Mutant checks**:
  - Dropping the hierarchy's `+0x84` write fails the pair test at case 6.
  - Dropping the group distance's `max(0)` fails the global test at search 104.
  - Putting the group distance's x first does *not* fail. That is expected: mula/madd order only changes a −0.

## Not verified / not 1:1

- **A listener that searches the database inside the walk is not followed** (2 of 1,500 global and 2 of 600 nearby cases). The plant listener's own query changes the tree's walk state under the original's search. The Rust search keeps its walk in its `Broad`, which the narrow phase (where the hand-on runs) never sees. The harness detects it (the global stamp moved during a hand-on), skips the case and copies the original's RAM over. `RecordsInRam::sweep` panics if a replay ever reaches it. Filed as F60.
- **No real hierarchy (kind 1) exists in state 4.** The pair's hierarchy path is proved on made-up ones built from copies of nat05 meshes, placed in RAM the way the original reads them. The global and nearby oracles see only real database objects, so they never pair a hierarchy.
- **The finite plane (kind 8) pair is not ported.** No nat05 object has that kind. The harness panics on it. Filed as F61 (parked).
- **Smoothed normals** (a non-zero smoothing word) are not ported. State 4's probe has 0, and the harness asserts it on every sweep it reads.
- **The plant part stand is still the original's** (count and part, handed on in the harness), as in F46d. So are the listeners and callbacks.
- **The `+0xac` words**: the pair zeroes them and nothing in the sweep writes them otherwise. What reads them is not known.
- **The 16-segment cap**: the original has none, and `Sweep::new` asserts ≤ 16 because MX's clear (reused) stops there. The pair test goes up to 16. No ride sweeps more than 1.
- **The two globals the pair writes** (the smoothing note and the mesh descriptor) are written by the test harness from what the pair reports, not by the library. The library has no RAM.
- **No ride takes these paths.** C15's 924 sweeps are all nearby, never pair and never hit, so the new code is proved only by the generated oracles, not by a recorded ride.

## Next

Nothing for F51. F60 (the searching listener) is filed low; F61 (the finite plane) is parked.
