# 1 — the biplanes broad phase (FINAL)

Task S45. 2026-10-20.

## What I did

- Ported the broad phase into `crates/game-sim/src/broad_phase.rs`: the world's quadtree and moving
  block list with their query state, the query that walks a chain of spaces over them, the gather
  that fills a nearby search, its re-sort, the three distances, and `probe_collide_world`.
- Made S44's `world_search::test` (the hit sequence) `pub(crate)` so the direct probe shares it. No
  other change to S44's code.
- Oracle tests in `crates/game-sim/tests/broad_phase.rs`: the gather and `probe_collide_world`, each
  over 10 000 cases on biplane.p2m2_SaveState.

## What the original does

Names below are research/oracle.txt's; `./kit fn <addr>` reads each.

**Which call refills the plane's search each frame.** `./kit fn 0x15b210`: a method of the plane's
collision owner (the base at plane+0x50, vtable at its +0x10, slot +0x108). If +0x54 is set it marks
the three probes (+0x124 unless it is +0x130, +0x128, +0x130) as moved (`probe_object_moved`), then
`collision_search_gather(+0x11c, +0x130, 3)`: +0x11c is plane+0x16c, the same search as the collision
record's +0x10c, and +0x130 is the record's ray probe. The caller of that slot I did not find
statically (every `lw 0x108(vtable)` + `jalr` in the binary was looked at; none loads the vtable from
+0x10 of an object at plane+0x50) — it is reached through some other table.

**The gather** (`./kit fn 0x218b80`), search fields: +0 list, +4 count, +0x10 centre, +0x20 bound,
+0x24 least, +0x28 extra reach, +0x2c no groups, +0x30 built, +0x34 capacity.
1. moved = |vsub.xyz(probe centre, +0x10)| (mula/madda/madd, sqrt). If built and moved + the probe's
   reach < bound: only re-sort (`./kit fn 0x2189d0`) — every distance again from the probe's centre,
   nothing dropped, the centre at +0x10 not moved.
2. Else: count 0; the query (kind as passed, global) of the box centre ± least (w from a 1.0 const);
   +0x10 = centre; bound = max(min(cx-lo.x, cz-lo.z, hi.x-cx, hi.z-cz, max(extra, reach)), least),
   lo/hi being what the static tree reported for its cells.
3. Each collidable the query hands out (next is called *before* the capacity test, so one more is
   handed out — and stamped — than kept): class 2 is marked searched, measured (`collidable_distance`
   with no extra), and kept if the pair filter passes *without* the nearby flag and (bit 0x08 clear or
   distance < bound). Class 3 (unless no groups): measured on the ground plane from its cell × the world
   manager's scale (+0xd4) less its model's reach r0 + blend·(r1−r0)/128 (models at +0x94: +0x10c,
   +0x114; blend s8 at +0x11), then 0 + that, clamped to 0 at or under 0; kept if < bound.
4. The query ended; built = 1.

**The query** (`world_tree_query_begin/next/end`, `0x1bb3e0`, `0x1bb490`, `0x1bb890`): a chain of
spaces (node: +0 next, +4 table); a table has the static structure at +0x34, the moving at +0x38, and
sub-tables at +0x44/+0x48. Kind bit 0 walks static, bit 1 moving. Order: the table's static, its moving
(if different), each sub-table's static, then the next space's, then the global table (0x48aac0) when
the query is global. Each structure is begun with the box; one with flags bit 1 reports the box its
cells cover, which `next` folds into the caller's lo/hi (min by `<`, max by `!<=`). `end` ends only the
last table's two structures.

**The quadtree** (vtable 0x451060: begin `0x196e90`, next `0x197580`, restart `0x197d70`, end
`0x197d90`, gather `0x197160`, gather-all `0x197370`). Begin: the box less the origin, ordered; off the
tree → 0 and nothing written. Cells: `cvt.w.s(lo·scale) & -n`, `n−1 + (cvt.w.s(hi·scale) & -n)`;
reported box = cells × 0x48a530, y ∓FLT_MAX. An open query with the same cells is restarted (new stamp,
from the first node; the chunk cursor is *not* reset), else ended. Then +0x898 = 0, open, stamp + 1,
the nodes meeting the cells gathered from the root (child side 0x4000, halving; a child fully inside
taken whole, post-order; a node pushed only if it has a chunk). Next: nothing if not open or +0x898 set;
else each chunk's four slots, a collidable handed out only if its u16 stamp (+0) differs, then stamped.
**The block list** (vtable 0x450220): begin/restart rewind, end rewinds and closes, no box. Next: a
full block's used slots once per stamp (stamped); the last block's used slots *every time, unstamped*.

**probe_collide_world** (`./kit fn 0x11a150`, (probe, lazy)): not active (bit 0x02) → hit bit lowered,
FLT_MAX. Budget (+0x64) −= |vsub(+0x40, +0x50)|, +0x50 = +0x40; lazy and budget > 0 → return budget.
Else budget 0, the probe cleared (class slot +0x90), searched, its box from class slot +0xa4, the static
query (kind 1, global): nearest = min of the four gaps between the box and the reported cells. Class 2:
searched; at pair distance 0 nearest = 0 and through the filter (nearby) to the narrow phase and the hit
sequence; else the nearest. Class 3 (unless the probe's no-groups bit): at ground distance 0 (reach r0 +
blend·(r1−r0)/128, no 0 + here) every part, unfiltered (nearest 0 per part found); else, only when lazy,
the nearest. Then if any table on the chain has a moving structure apart from its static one, every
moving object (kind 2, no box, not global): searched, at distance 0 through the filter and the narrow
phase. A hit: +0x50 = (0, −10000, 0, 1), nearest 0. Budget = nearest, returned. Neither query is ended.

## How it was proved

`tests/broad_phase.rs` loads the save state and builds the port's world from RAM: both probes, every
collidable in the quadtree's chunks and in the moving list, and the plane's search entries (one of its
34, a class-2 object, is no longer in the world, so it is added from the search). The port's spaces,
tables, tree nodes, chunks and blocks are all read back from RAM, never written by hand.

Each case changes the original's RAM and the port the same way, runs both, and compares:

- **The world shaken.** The table's static structure is the real tree, or a *second tree*: the real
  header copied to scratch with another origin and its own link pool (a second tree on the first's pool
  is a test artifact — the pool reset in the quadtree's end frees every link of the pool, so the real
  tree's open query would lose its links). The moving structure is the real list, a copy with block size
  4 and up to 3 blocks of its own, or a tree. 0–2 sub-tables, from {none, tree, list, own list, second
  tree}; the two space links chained either way; global on, off, or the same table. Busy (+0x898) set on
  a tree one case in ten, the stamp near its u16 wrap one in fifty, ENABLED/0x08 flipped on objects.
- **The gather** (seed `0x5ea4_c4fa_0000_0045`): probe centre about the plane, near an object, inside
  the bound (the re-sort path) or anywhere; the search rewritten (capacity small one case in three,
  built cleared, least on the exact distance of a near object one in five), kind 1 or 3. Compared: the
  list (object and distance bits), count, centre, bound, built, and the whole world — both trees' query
  state (open, cells, stamp, nodes, cursor, chunk, slot), the lists' cursors, every collidable's stamp
  and class-2 flags.
- **probe_collide_world** (seed `0x5ea4_c4fa_0000_4504`): the class-2 objects' real listeners and
  callbacks zeroed, a pool of the probes and 12 near objects given scripted listeners, callbacks and
  part lists, and every class slot the probe reaches (clear, bounds, pair, part count, part) hooked to
  script shared by both sides. Compared: every call out in order, the return bits, budget, last, hit
  bit, groups, and the world as above.

Both pass. Branch counts (each must exceed 300, "mid-walk" 50, asserted): gather — a second space 3409,
a tree left mid-walk 108, busy 1188, empty 1614, full 2005, gathered 9073, global 5022, re-sorted 927,
on the bound 945, sub-tables 5064, two trees 4818; probe — callbacks 2983, heard 5599, hit 2233,
inactive 988, lazy with budget left 1660, nearest > 0 2795, moving pass 6650, pairs 5924, parts 4068.

**Mutants** (each applied to the port, the test run): killed — the last block stamped; smallest node
`<=`; begin keeps busy; group nearest without lazy; next after the capacity test; sub-table moving
skipped; a hit leaving `last`; bound without least; group reach with the blend's sign flipped; restart
keeping the old stamp; end leaving the cells; within-bound `<=`; restart resetting the chunk cursor
(case 1500, only after the second tree came in); global not skipped when it is the same table (the
port loops). Survived, equivalent for these callers: no fold of `lo`, and the `hi` fold by `<=` — both
callers read the reported box right after begin, before any `next` folds into it.

`./kit check -- --no-fail-fast` with C6's capture in place: everything passes except `trick::the_humans_tricks`, the known
failure on C6's capture (`rows whose pad is not field 2k - 2`), as in S43 and S44; it touches none of
this task's code. For the gate, C6's `recording2-freeride.csv` was parked out of `acceptance/` and
s5's `recording2-tricks.csv` (S30's) put there instead, as in S43 and S44.

## Not verified / not 1:1

- A box the static tree turns down (wholly off it) leaves the caller's lo/hi as whatever was on the
  original's stack; the port leaves them as 0 (or as the caller had them). Not reachable for a probe
  inside the world; not tested.
- The node pool is not modelled: the original allocates list links from a pool (`0x1974f0`, refilled by
  `0x105b70`), and a pool that cannot grow would crash it. The port's list is a `Vec`.
- What sets the quadtree's +0x898 is not known (the save state has it set, from something else); the
  port carries it as `busy` and only clears it where the original does.
- The caller of the refill (`0x15b210`, slot +0x108) was not found: see above.
- The folds of later structures' boxes into lo/hi in `next` are ported but unobservable here: both
  callers use the box only straight after begin (two mutants on them survive, above).
- The link pool's reset is not modelled: the port assumes one tree per pool. With two trees on one
  pool the original frees the other's links on end; the test gives the second tree its own pool.
- The quadtree's +0xb4 (cleared by the pool reset) is neither modelled nor compared.
- Gather with kind 2 (moving only) is not tested: no static structure reports a box, so the bound
  comes from the caller's uninitialised lo/hi on the original's stack. Neither real caller passes 2.

## Next

- S46/S47: the ray and sweep probes' narrow phases (the `Probe`/`Narrow` pair pass here is scripted).
- S48: the `Search` impl over this module, the plane's listeners, and the wiring into the ride —
  including finding who calls the refill (slot +0x108 of the plane's collision owner), most likely
  by a write watch on the search's +0x30 under the emulator.
