# 1 — sweep follows listener query (FINAL)

Task F60. 2026-11-16.

## What I did

- **Library** (`crates/game-sim/src/atv_sweep.rs`): `Outside` gets two default no-op methods. `lend` gets
  the walk's `objects` and `Broad` before each test in `search_world`. `take_back` gets them mutably after
  the test, so the caller can put back what a handed-on listener's query left. The three `test` calls in
  `search_world` go through a new `test_lent`. Other `Outside` impls are unchanged (they get the defaults).
- **Harness** (`tests/mxvatv_bike_ride.rs`): `SweepWorldInRam` gets `lent: Option<LentInRam>` (set only by
  `sweep_all_in_ram`, the global search). Before any hand-on (listener or callback) it writes the walk as
  lent to RAM (`sweep_walk_written` = F51's write-back, factored out). After a test that wrote, it reads the
  walk back (`GatherInRam::read_walk`, new in `gather_in_ram.rs`: stamp, current tree, entry stamps, each
  structure's walk state and open list). The final write-back starts from what RAM holds since.
- `sweep_all_in_ram` no longer returns `Option`. Its two `.expect` callers (the record sweep replay and the
  rider's whole-scene search) lose the expect.
- The nearby oracle: the skip is gone, nothing else. Both oracles now assert they met the case at least once
  (`reentered` / `sweep search reentered`).
- Nothing taken from `../opentennis`: it has no spatial database with listeners.

## What the original does

- The listener (`./kit dis` of 0x00495690 on a scratch atv kit.toml, recipe in the
  disassemble-from-RAM-image memory) is a thunk: `this - 0x38`, then `./kit fn 0x00493d40 -o atv`. That
  calls the part's vt `+0x138` if `+0x44` bit 1 is set. On the struck part that is `./kit fn 0x00494940
  -o atv`: the plant part wakes (allocs, physics links, flags). Somewhere in it a database query runs.
- What that query leaves, read in the second Ee around the hand-on: stamp +1, and the tree's open query
  **ended** (flags bit 0 down, cells back to `0x80000000`, list head and position 0).
- So the original's global walk asks the tree for its next entry, gets none (the tree is closed), and goes
  on to the moving list. The rest of the tree's entries are passed over in that search. The port does the
  same once the walk is taken back: `Quadtree::next` answers none on a closed tree.
- The nearby walk is over the record's nearby list, not the tree, so the query does not touch it. Its two
  cases already matched byte for byte once the skip was removed.

## How it was proved

Function oracles on the state-4 RAM image, two Ees, all RAM outside the stack compared after every case:

- `the_sweeps_global_search_is_the_originals`: 1,500 searches, none skipped (693 clear, 498 hit, 220 lazy,
  89 inactive). 12 listener hand-ons, 2 of them querying the database. Answer bits and RAM equal every time.
- `the_nearby_sweep_with_its_pairs_is_the_originals`: 600 sweeps, none skipped (197 hit, 403 missed), 2
  querying hand-ons.
- Mutant: with `take_back` not called in `test_lent`, the global oracle fails at search 589 (RAM differs at
  65 ranges). Restored with cp + touch.
- The other sweep tests (`./kit check … sweep`, 5 tests) pass.

## Not verified / not 1:1

- **The listener is not ported.** It wakes a plant part (heap, physics). It stays the original's, handed on
  in the harness. A pure-Rust game needs a Rust `take_back` that puts back what its own wake did to the
  database. Nothing implements one yet, because nothing in the game calls `search_world` today (only the
  oracles do).
- **Only one shape of re-entry is met**: the listener's query ending the tree's open query. A listener that
  leaves a new list open, moves the walk on the moving list, or adds entries to the database is handled by
  the same read-back (list, position, chunk, stamps), but no case reaches it. Entries the database gains
  are not read back: `read_walk` panics if the open list names a node it did not read.
- **Object fields a listener changes are not taken back** into the port's `objects`. RAM keeps them (the
  final write only writes the fields the port changed), so RAM matches. A later test in the same search
  would see the port's stale copy. No case shows it.
- **Only 2 + 2 cases** reach this path, on one state (nat05, state 4). No recorded ride does: C15 never runs
  the global search.

## Next

none
