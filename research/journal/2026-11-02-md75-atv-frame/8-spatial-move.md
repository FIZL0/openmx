# MD75c1: the spatial database move (`atv_spatial_move`)

2026-11-03. Task: `./kit plan MD75c1`.

MD75c (`6-post-callees.md`) left `atv_spatial_move` hooked. It is the call that the quad's spatial update,
its node's bounds and each collidable's box move all end in. It is now ported in
`crates/game-sim/src/atv_spatial.rs`, and `quad::Call::SpatialMove` is answered by `atv_spatial::Answers`. The
fields and offsets are in `research/oracle.txt` under "MD75c1".

## What the original does

`atv_spatial_move(entry, centre, half, reach)` jumps to `atv_spatial_move_in` with the entry's record
(entry `+8`).

- **No cell** (record `+4` = 0): it does nothing. The last centre is not written either.
- **The walk.** It starts from the record's cell. It takes the first link, in list order, whose portal the
  segment from the last centre (`+0x10`) to the new centre crosses (`atv_portal_crosses`), and the walk
  starts again from the cell beyond. The only link it skips is the one straight back to the cell it came
  from. A loop of three or more cells could therefore walk forever. The original has no guard, and the
  port has none either.
- **The same cell.** It calls sub-database vt `+0x20` (pick) with the current slot. If the slot it gets back
  is the same, it calls `+0x28` (move). Otherwise it calls `+0x2c` (out) and then `+0x24` (in), and keeps
  the new slot.
- **A new cell.** It takes the entry out of the old sub-database and out of every extra cell, and hands
  each extra's link back to the pool. The record's sub-database becomes the new cell's one for the old
  sub-database's kind (`atv_cell_sub_of`: bit 0 gives `+0x28`, bit 1 gives `+0x2c`). Then pick with 0,
  then in.
- **The overlaps** (`atv_spatial_overlaps`). `atv_spatial_reach` gathers every cell that the sphere
  (centre, `reach`) touches through a portal (`atv_portal_touches`). It starts from the record's cell,
  recurses from each cell it finds, never takes the home cell and never takes a cell twice, and keeps them
  in 35 slots on the stack. Then, for each extra cell in list order:
  - if it was gathered, the entry is moved in place (pick/move or out/in) and the gathered entry is struck
    off;
  - if not, the entry is taken out, unlinked, and the link freed.

  Each cell still gathered gets the sub-database of the *carried* kind. That kind is the last sub-database
  touched: first the record's, then each kept extra's, then each new cell's. The entry is put in with pick 0
  then in, and a pool link is pushed at the front of the record's list.
- The record's `+0x10` becomes the centre, all four lanes.

Every cell in MD37a's image holds its moving entries in a **list** sub-database (vtable `0x003b5510`): blocks of
slots and a free chain, with slots tagged `| 2`. Pick returns the slot it was handed whenever that slot is
nonzero. So for a list, "the same cell, a new slot" never happens: a list's move always stays in place. The
pool takes a link in this order: the last chunk's next link, then the free chain's head, then a new chunk
(one set aside, or else off the heap).

## The port

- `atv_spatial.rs` models the database by index, not by address:
  - `Database`: cells (links, two sub-databases), portals, sub-databases (`Sub::List`, `Sub::Other`), and
    the `Pool`;
  - `Record`: extras newest first, cell, sub-database, slot, last centre;
  - slots are `SlotAt { block, index }` and links are `LinkAt { chunk, index }`, so the port does the
    bookkeeping that the addresses do in the original.
- `crosses` and `touches` are the two portal tests in `F`/`Vf` with the original's operand order. The edge
  test reads the dot's sign bit, so −0 fails as `bltz` does.
- `spatial_move(db, rec, item, centre, half, reach)` is the move. `half` is not read: lists never use it.
- `Answers { space: Space { db, quad, node }, rest }` implements `quad::Moto`. It answers
  `Call::SpatialMove` on the quad's record (item `QUAD`) or the node's (`NODE`), and passes every other call
  on to `rest`.

## Proof

`crates/game-sim/tests/atv_spatial.rs`, against `atv-r-spawn.ee.bin`:

- **`the_spatial_move_is_the_originals`: 12,000 generated databases.**
  - **What is generated.** Each database is a tree of 1–8 cells. Its portals mostly sit on the move's
    segment, at random tilt, size and winding; 5% have edge normals at random, and 10% have the plane off
    the corners. Each cell's second sub-database is a list. Half the time the first is a list in every
    cell, sometimes of kind 3, so that the carried kind can pick the other list. Otherwise the first is
    the static one (`Sub::Other`) or none. The lists have 1–8 slot blocks, 0–3 blocks, and a random free
    chain. The pool has stride 0x10/0x14/0x20, 1–8 links a chunk, 0–3 chunks, a random free chain, and
    0–2 chunks set aside. The record has 0–3 extras; 5% of records have no cell. 15% of centres do not
    move, and 5% sit on a portal corner. The reach is 0, 100, or 0..8.
  - **How it runs.** Each case is laid at `0x01f80000`, the zero RAM up to `0x01fec000`. The pool global
    is pointed at it, and allocate/reallocate are served from a bump heap there. The original's
    `atv_spatial_move` runs. It must write nothing outside that RAM and the stack.
  - **What is compared.** Everything is read back into the model: cell links, every list's blocks, slots,
    `last` and free chain, the pool (chunk chain, set-aside chain, free chain, used, and the next-link
    pointer), and the record. The result must equal the port's, with the centre compared bit for bit.
    Half the cases go through `Answers` with `Call::SpatialMove`.
  - **Result.** All 12,000 are equal. Paths taken:
    - 9,013 stayed in their cell, 2,408 walked out of it (415 of them through two or more portals), and
      579 had no cell;
    - extras: 1,865 kept, 3,877 left, 4,080 entered, and 329 cases ended with extras in both kinds of
      list;
    - links: 1,483 from a chunk's next, 973 from the free chain, 1,448 from a set-aside chunk, 663 from a
      chunk off the heap;
    - 2,229 cases grew a list by a block.

    The test fails if any of these paths is taken fewer than 10 times.
- **`the_images_own_database_moves_as_the_original`.** The trigger probe's entry (`0x0159c518`) and the
  frame mesh's entry (`0x01627388`) are moved in the image's own database. In the first run the move is
  0.125 ft, and both stay in their slot. In the second run one portal is laid a foot along +x, between their
  cell `0x00654680` and `0x006545c0` (links pushed onto both cells' lists). The centre moves through it
  with a reach of 2. The entry walks out of the image's 1,024-slot list into `0x006551c0`'s slot 7, and it
  reaches back into its old cell, which takes a chunk off the heap: the image's pool is full (255 of
  255) and has none set aside. The image's lists, the pool and the record must all read back equal to the
  port's.
- **Mutations** were run one at a time on the port and all were caught:
  - the walk takes one portal and stops;
  - −0 passes an edge;
  - the sphere's `s < -r` becomes `<=`;
  - new extras are pushed at the back;
  - the pool keeps no links at stride 0x10;
  - a kept extra's kind is not carried;
  - new cells are picked by the record's kind;
  - the kind is not carried from one new cell to the next. This last one was only caught after kind-3
    lists were added to the generator.

`./kit check -- -p game-sim --test atv_spatial`: 2 passed.

## Decisions

- **`Answers` is a `quad::Moto` wrapper.** `atv_moto::Parts` is `Copy`, so a database with `Vec`s in it
  cannot go there. `Steps` already hands `SpatialMove` to `rest`, so a wrapper answers it without changing
  `Steps` or the `atv_quad.rs` tests. Those tests keep logging the call as the original's callee, as before.
- **Only list sub-databases are ported.** The first sub-database in the image's cells is the static
  quadtree (vtable `0x003b6700`), which `broad_phase` reads. A moving entry never goes in there in the image.
  `Sub::list()` panics with a message if one would.
- **Trees only.** The generator builds trees of cells, because the original's walk does not end on a
  three-cell loop that the segment crosses all the way round.
- **The image has no portals.** So the walk, the reach and the pool are proved on generated databases. The
  image is used for its real lists and pool, with one laid portal.

## Not verified / not 1:1

- The quadtree sub-database's virtuals (pick/in/out/move for vtable `0x003b6700`) are not ported. An entry
  of kind bit 0 in a cell whose first sub-database is the quadtree panics in the port.
- The reach's 35 stack slots are not checked by the original. The port's list grows instead, so a database
  that reaches past 35 cells would differ: the original overwrites its stack.
- Heap failure is not modelled. When the heap fails, the original's chunk call returns 0 and it writes
  the link at address 0; the port always gets a chunk. The pool's per-link constructor (`+0x2c`, 0 in the image) and its own heap (`+0x30`,
  0 in the image) are not modelled.
- A stride of 4 or less (the pool keeps no freed links) is in the port but not generated: links that small
  would overlap.
- No capture of the original in play: the image's records never move here, and the quad's own entry is 0
  at the spawn. Who builds `Answers`' database for a ride (the world's cells and lists out of a level) is
  not ported. `Space` starts empty, and with no cell a move does nothing.
- The cells and portals of real levels (the image has three cells and no portals) have not been seen. The
  generated portals are squares; the original reads any four corners.
