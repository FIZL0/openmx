# 1 — sweep in rust (FINAL)

Task F46c. 2026-11-06.

## What I did

- `crates/game-sim/src/atv_sweep.rs` (new) holds SLUS-21104's sweep probe:
  - `set` and `bounds` (the segment set and the box slot's min/max box).
  - `ends` (a segment's ends through its node).
  - `Sweep`, the probe's `world_search::Narrow` for `atv_search::search`. Its clear is MX Unleashed's `sweep_probe::clear` (the fields sit `+0x10` further up; the only other difference is the 16-segment cap, which `Sweep::new` asserts). It drops the hit bit and makes the collidable's reach the bundle's radius. Everything else goes to `rest`.
- `tests/mxvatv_bike_ride.rs`:
  - `sweep_in_ram` does the record's sweep in Rust over RAM, in three steps:
    1. The set: points and null nodes written to the probe's arrays, the centre to `+0x50`, the reach to `+0x170`/`+0x70`.
    2. The place: the box again, then F46a's `spatial_move_in_ram` on the entry.
    3. The near search: `sweep_world_in_ram` reads the probe, the `+0xfc` candidates and the collidables their lists name. The search's flag bytes, the clear's world form, its globals and the record reset are written back.
  - `RecordsInRam::sweep` answers with it under `in_rust("record sweep")` when the sweep is nearby. Otherwise it hands the call on as before (`> record sweep`).
  - `Unported` is the narrow phase past the clear. It panics if reached.
  - Two new tests: `ride_replays_with_the_sweep_in_rust` and `flip_replays_with_the_sweep_in_rust`. The flip test runs PartMove, Gather and the sweep all in Rust.
- The addresses and layouts are in research/oracle.txt `# F46c`.
- Filed F51 for the pair test, group parts, nodes and the global search.

## What the original does

- **The set** (`./kit dis 0x001b9290 --len 0x200 -p atv`):
  - For each of 2 × count points it copies the node pointer (or 0) and the point quad into the probe's two arrays (`+0x134`, `+0x138` are pointers).
  - It builds a box over the ends with `vmini`/`vmax` on all four lanes. Each segment does its second end, then its first, starting from (±MAX ×3, 1).
  - Centre = (hi + lo).xyz × 0.5 (w taken from hi). Reach = |(hi − lo) × 0.5|, an FPU mula/madda/madd and a sqrt.
- **The place**: the probe's first slot does nothing. If the probe has a database entry, the box slot rebuilds the same box and the entry is moved (F46a's port).
- **The near search** is `atv_search::search` (`collide_against_search`). The probe's clear (`./kit dis 0x001b95f0 --len 0x470 -p atv`) is MX Unleashed's clear shifted by `+0x10`, with no cap, plus:
  - two globals zeroed;
  - the record noted in a third global;
  - the hit bit dropped.
- **The pair test** (vt `+0xb4`) is a sphere test, then a contact-list sweep through a mesh. It is never reached on C15. I measured C15's 924 sweeps:
  - all are nearby, with one segment and no grid;
  - none has a kind-2 collidable within reach;
  - 394 have plants within reach, but the probe's `+0x126` = `0x13` includes NO_GROUPS, so groups are skipped;
  - none hits.

## How it was proved

- **Lockstep bit-exact replays**: C15's ride-2 (459 rows) and flip-1 (465 rows, wrecked on row 125, put back on row 193) through `replay_in_rust`. Every row's words match C15's, and the whole RAM (outside the stack) matches the run that hands calls on, after every frame:
  - `ride_replays_with_the_sweep_in_rust`: 459 sweeps in Rust;
  - `flip_replays_with_the_sweep_in_rust`: 465 sweeps, 771 moves and 465 gathers in Rust.
  - `./kit check -- -p game-sim --test mxvatv_bike_ride -- sweep_in_rust` passes 2 tests.
- **Mutant check**: with the clear's segment starts not written back, ride-2 fails on row 0 ("RAM differs … at 1 ranges"). The test is live, not skipped.

## Not verified / not 1:1

- **The pair test (vt `+0xb4`, its sphere and mesh sweep) and a group's parts are not ported.** `Sweep` hands them to its caller and the test's `Unported` panics on them. C15 never reaches them (see above). Filed as F51.
- **The global search** (the record sweeping with nothing nearby) still goes to the original. It never runs on C15, so nothing could check a port. F51.
- **Segment nodes**: the record always passes none. `ends`' node transform is MX Unleashed's vu order read off the set's disassembly, but no ride runs it. F51.
- **A hit's answer** (the normal, point and struck object read back) is unreachable without the pair test. `RecordsInRam::sweep` asserts there is no hit.
- **The count cap**: the original has none. `Sweep::new` asserts ≤ 16, because MX's clear (reused here) stops at 16. C15's sweeps all have 1 segment.
- **The grid prepare** (vt `+0xd0`, when the list has a grid at `+0x40`): never on C15. Asserted absent.
- **Two write-backs no ride can check**:
  - the clear's note of the record in global `0x003dc618`: something later in the frame rewrites the word, so leaving it out does not change RAM;
  - the probe's SEARCHED bit: it is already set (`+0x126` = `0x13`) when the search sets it.

  Both are written as the original writes them, but no ride tells right from wrong.

## Next

Nothing for F46c. F46d (the part search) is next in the F46 split, and F51 covers the paths C15 never takes.
