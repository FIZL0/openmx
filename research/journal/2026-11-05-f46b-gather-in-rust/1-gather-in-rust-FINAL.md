# 1 — gather in rust (FINAL)

Task F46b. 2026-11-05.

## What I did

- Measured what the original gather writes on state 4 (the interpreter's `written` set, one call per
  probe): each entry stamp the tree query hands out (halfword at the entry), each met collidable's
  flag byte (`+0x126`), the global stamp, the tree-last-queried global, the tree's walk state, its
  query pool's words and list nodes, the moving list's walk words, and the bike's nearby list.
- In `crates/game-sim/tests/mxvatv_bike_ride.rs`:
  - `GatherInRam::read` reads the world the gather walks out of RAM as `broad_phase` models it: the
    probe's chain of spaces, their tables (sub-tables and the global one too), each table's quadtree
    node by node with its chunks and the open query's list, the moving list slot by slot (slots past
    the last block's count and free-chain words are empty), and every entry they hold as an item,
    with each collidable's pair-filter fields, whose object lists name more items. Placed groups
    read their cell, model and blend, and the ecosystem's models; nat05 has none.
  - `GatherInRam::write` puts the outcome back where it changed and asserts nothing else changed
    (spaces, tables, tree nodes, list slots, every collidable field but the flag byte, every place
    field but the stamp).
  - The tree query's list nodes come out of the tree's own pool, which the port does not model (the
    port keeps the list as a `Vec`). The write-back takes and gives them as the original does
    (`query_pool::take`/`reset`: bump, free chain, a spare chunk; a new chunk off the heap panics).
    Which list was built is read by beginning the query on a copy of the world first
    (`broad_phase::Query::begin`): fresh, restarted (same cells, open) or off the tree.
  - `gather_in_ram` = read, `atv_world::gather`, write. `posts_frame` answers `Gather` with it when
    `IN_RUST` names `Gather`.
- No change to `crates/game-sim/src/`: `atv_world::gather` (MD76a1/MD83) ran on state 4's world as it
  is. The task line's `src/` was there in case it needed one.
- Nothing taken from opentennis: no broad phase of this kind there.

## What the original does

`./kit fn 0x001d6a60 -o atv` (the gather) begins a query (`0x0019d2b0`) on the probe's space chain:
nat05's one cell, its quadtree (`+0x28`) and moving list (`+0x2c`), no sub-tables, no global table.
The quadtree's begin (`./kit fn 0x00180dd0 -o atv`) ends an open query (pool reset,
`./kit fn 0x00106130 -o atv`), takes a list node per listed tree node (`./kit fn 0x001810b0 -o atv`),
and bumps the stamp; its next (`./kit dis 0x00181530 -p atv`) stamps each entry it hands out; the
gather ends the query (`./kit fn 0x00181d90 -o atv`), handing every list node back. Offsets and globals
are under `# F46b` in research/oracle.txt.

## How it was proved

- `state4s_gather_in_rust_leaves_ram_as_the_original`: 300 gathers on state 4 (the riding probe, the
  frame mesh, then the riding probe stood at points out to 60 and 400 ft round the bike), one RAM by
  the original's gather and one by `gather_in_ram`, the two RAMs alike outside the stack after every
  gather. Lists from 13 to 69 candidates, 22 sizes; all 300 queries fresh.
- `ride_replays_with_the_gather_in_rust`: ride-2, 459 rows word for word against C15 and RAM alike
  every frame with the handing run, 918 moves and 459 gathers Rust's.
- `flip_replays_with_the_gather_in_rust`: flip-1, 465 rows, wrecked on 125 and put back on 193, RAM
  alike every frame, 771 moves and 465 gathers Rust's.
- The checks bite: with the tree-node word of the pool's list nodes left out, state 4's gather 2
  fails (`0xea7f88`); with the pool's last reset left out, gather 0 fails (the pool's words); with the
  flag byte's write-back left out, ride-2 fails on row 98. (Leaving out the node's next word does
  not bite, and cannot here: the nodes always come off one chunk in order, so each next word is the
  node before's address, which the last gather left there already.)
- `atv_world::gather` itself was proved against the original on MD37a's image over 9,930 gathers
  (`tests/atv_gather.rs`).

## Not verified / not 1:1

- **The restarted query never runs.** Every gather on state 4 and C15 began a fresh list: state 4's
  tree is open but on other cells, and each gather ends its query. The restart's write-back (no
  nodes taken, one reset) follows `./kit dis 0x00181d70 -p atv` read by eye, not a run.
- **The pool's other ways never run**: the free chain is always empty and the chunk never runs out,
  so a node off the free chain, a second spare chunk and the heap (which panics) are not exercised.
- **Off the tree** (a box beyond nat05's quadtree) never happens on C15; the original then leaves its
  bound box as the stack had it (the port's journal on `Query::next`).
- **Placed groups, sub-tables, a global table, a second quadtree**: nat05 has none, so their reading
  is untested; a second tree in the probe's tables panics (the write-back follows one).
- The world is read whole out of RAM on every gather (every tree node and entry), not held in Rust
  between frames: the lockstep harness keeps RAM as the shared state while the sweep probe and the
  part search (F46c, F46d) are still the original's.

## Next

F46c (the record sweep probe) and F46d (the part search): add `record sweep` / `record search` to
`IN_RUST` the same way. The part search reads the same nearby list `gather_in_ram` writes.
