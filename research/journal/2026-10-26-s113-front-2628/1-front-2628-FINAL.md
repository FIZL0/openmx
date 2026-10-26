# 1 — front 2628 (FINAL)

Task S113. 2026-10-26.

## What I did

- **Tested the lead from S56/S62 with a probe.** I replayed front from its 2602 state through the
  original's update and hooked `collider_set_position`, `sphere_collider_pair` and
  `collide_against_search` at 2626–2628.
  - 2626 and 2627: every bone sphere is paired with the bike, and the hits equal the port's mesh test.
  - 2628: **no pair with the bike at all.** The two sphere tests the Ee did run missed, but the port's
    mesh test (`Bike::touch`) said both touch the bike.
  - The Ee's tree has not held the bike in any node since 2619 (`bike_key_in_tree` None). Its
    placement record still says the first state's key. The key for the laid-in box is the next cell over.
- **Put the bike back where the game keeps it, and the tick held.** Before 2628's update I gave the bike
  a fresh placement through the world's own add (`./kit fn 0x00119c60`). Calling `broad_phase_leave`
  first never returned: the laid-in pools leave the tree's chunks unwalkable, so the record was
  dropped instead. The original's update then paired 36 times and matched the capture bit for bit.
  Done every row, front held to 2888.
- **`place_bike` (tests/ragdoll_wrecks.rs), called by `lay_in` on s62 captures.** For each row with
  the bike placed:
  - the bike's box from `mesh_collidable_bounds`;
  - the key from the tree's own vt `+0x20`;
  - an insert through vt `+0x24`, unless `tree_holds_at` finds it under that key already;
  - the record's key and centre set in place. The captures show `B+4` the same record all the way
    through, so the game updates the record in place.

  Once the put-back has unplaced the bike (`B+4` 0), `unplace` empties every tree slot that holds it,
  which is what the unwreck leaves. The replay never runs the unwreck. `lay_in_as(…, false)` and
  `originals_tick_as` keep the old behaviour, for the lead test.
- **S62's front bound: 2627 → 2888.** `the_port_replays_the_wrecks_through_the_bike` now has 2290
  ticks that are the capture's own (was 2029) and 14 with a moving hit. Side, back, tip and front3 still
  hold to their ends with the re-placement.
- **`every_sphere_test_of_the_wrecks_is_the_bike_meshs`:**
  - The port's search now takes the bike under its placement's key.
  - A new check runs at every gather: the placed key equals the port's `Cells::key` for the laid-in
    box.
  - Counts: paired 600 → 852, hits 64 → 84, listed 176 → 230. Tests, gathers and rebuilt are
    unchanged, and so are the put-back gathers.
- **New test `front_parts_at_2628_without_the_bike_in_the_tree`.** It runs both replays from 2602 to
  2627; see "How it was proved".
- oracle.txt: an S113 note under S108 (the tree's vtable slots, the record, the keys).
- Filed **S139**: front parts again at 2889.
- Nothing from opentennis applies (it has no ragdoll broad phase).

## What the original does

- **The track's tree and its vtable.** The tree is a static quadtree. Its vtable is at the tree's
  `+8`:
  - `+0x20` gives a box's key: (tree, centre*, half*, 0).
  - `+0x24` inserts a collidable under a key: (tree, collidable, key, centre*, half*).
  - `+0x2c` removes one at a key (`./kit fn 0x001ae2f0`).
- **How the world adds a collidable** (`./kit fn 0x001adb70`): it asks `+0x20` for the key, then
  `+0x24` to insert. The placement record holds the tree at `+8`, the key at `+0xc` and the centre at
  `+0x10`.
- **The game keeps the bike in the tree.** S105 proved this on 16 states. The bike's own per-frame
  move was not found (see below).
- **Why the bones need the tree.** The bones' search is rebuilt only when sphere 0 has stood past the
  bound. At front 2628 it had: from (4302.98, 156.77, 3726.02) with bound 16.81. The rebuild queries
  the tree, so a bike missing from the tree is not listed. The bike's `+0x116` bit 0x8 is clear, which
  means it is listed wherever the query finds it.

## How it was proved

All runs below are from front's 2602 state (`context/capture/s62`), through the original's own
`rider_body_update` in the oracle, checked against every captured bone lane, bit for bit.

- **The extended replay holds.** `the_port_replays_the_wrecks_through_the_bike` replays to 2888 with
  the original's update. The port's actor matches it on every number, on every tick to the capture's end.
- **The placed key is the port's key.** `every_sphere_test_of_the_wrecks_is_the_bike_meshs` checks it
  at every gather (367), and every sphere test of the five wrecks against the port's mesh test.
- **The lead.** `front_parts_at_2628_without_the_bike_in_the_tree` runs two replays, with and without
  the bike kept in the tree. Both hold every row to 2627. At 2628:
  - Both searches are rebuilt.
  - Lost tree: 0 pairs and 0 hits with the bike, and all 360 bone lanes off the capture.
  - Placed tree: 36 pairs, 11 hits, and the capture's tick exactly.
  - **The bike's laid-in frame against the bones' displacement.**
    - The hits on the spheres of bones 0, 16 and 17 have a world normal equal, word for word, to the
      first row of the bike's frame.
    - Placed minus lost, bones 0, 15, 16 and 17 moved 0.645, 0.471, 0.948 and 0.527 ft, at cosines
      0.9995, 0.990, 0.959 and 0.998 with that row.
    - Bone 0 moved y −0.57 against vy +5.6.
- **The other test files are untouched.** Only `tests/ragdoll_wrecks.rs` changed. All of
  `./kit check -- -p game-sim --test ragdoll_wrecks` passes.

## Not verified / not 1:1

- **The game's own per-frame move of the bike in the tree was not found or run.** `place_bike` does
  the same thing from outside: the key from the tree's own function, an insert at that key, the record
  updated in place. Two things are not known:
  - whether the game removes the old node's entry (or removes and re-adds) on the same frame;
  - when in the frame it does this, relative to the body's update.

  Only the result is proved: the capture holds and the key equals the port's.
- **The bike stays listed in stale nodes in the Ee.** `place_bike` never removes the bike from the
  old key's nodes, because `broad_phase_leave` loops forever on the laid-in chunks. A node from the
  capture's pools can therefore still list it next to the new one ([3,0,2,0,0,0] next to the new key's
  nodes). The search holds anyway. A wreck where a stale node would add a candidate the game does not
  have is not ruled out.
- **The record's other words** (the next link `+0`, the table `+4`) are left as they were.
- **front from 2889 to 3180 (292 ticks) is still the Ee on its own state, not the capture's.**
  Filed as S139. The bike has been out of the broad phase since 2681 and nothing pairs with it at
  2889, so this is a different cause. Not looked into.
- **No new live capture.** Everything is from the existing s62 captures and states. No state was saved
  at 2627 or 2628, so the game's own RAM tree at 2628 was not read; the lost bike is inferred from the
  Ee and proved by the tick holding.

## Next

Nothing in S113. S139 is front at 2889.
