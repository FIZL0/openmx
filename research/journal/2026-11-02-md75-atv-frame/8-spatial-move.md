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
  `Sub::list()` panics with a message if one would. (MD89 has since ported the quadtree; see below.)
- **Trees only.** The generator builds trees of cells, because the original's walk does not end on a
  three-cell loop that the segment crosses all the way round.
- **The image has no portals.** So the walk, the reach and the pool are proved on generated databases. The
  image is used for its real lists and pool, with one laid portal.

## Not verified / not 1:1

- The quadtree sub-database's virtuals (pick/in/out/move for vtable `0x003b6700`) are not ported. An entry
  of kind bit 0 in a cell whose first sub-database is the quadtree panics in the port. (Ported by MD89,
  below.)
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

# MD89: the quadtree sub-database's moving-entry virtuals

2026-11-03. Task: `./kit plan MD89`.

The quadtree (vtable `0x003b6700`) is now a sub-database the move can use: `atv_spatial::Sub::Tree`. Its
pick (vt `+0x20`), in (`+0x24`), move (`+0x28`) and out (`+0x2c`) are ported as `Tree::pick`, `put`, `moved`
and `take_out`. Only a sub-database the tests do not read is still `Sub::Other`, and a move into one still
panics. The fields and offsets are in `research/oracle.txt` under "MD89".

## What the original does

- **Pick** ignores the slot it is handed. It takes the box's corners less the tree's origin. If they do not
  lie wholly in `0..extent` both ways, it answers 0 (no clamp, unlike the static build's key). Otherwise it
  turns them into cells: `cvt.w` of the corner times the scale, plus 1 on the far side. The level is the
  first whose node side is more than the box's width and depth, or the deepest. The corners are masked to
  that side. If they still span more than one node, it goes up one level (corners at twice the side). The
  key packs the level, the corner nodes the box spills into (0x10, 0x20 in z, 0x40 in x, 0x80 both) and
  the corner cell. Its x and z are clamped to 0x7fff, and the unpack keeps bits 3..14 of each.
- **In** does nothing for key 0. It closes an open query, counts the entry in (`+0x888`), marks the tree
  as the current one, and clears the entry's first halfword. Then, under each corner the key names, the
  root and every node down to the key's level are widened to the box's height (`cy ± hy`). A node with
  no children is grown (reallocated to 36 bytes, children after it), and a missing child is made on the
  way. Each node made counts in `+0x880`. The entry goes in the node's first empty slot, or into a new
  pool link chained at the end.
- **Move** only widens: the root, then each node down each corner, stopping at a node with no children.
  It does nothing if the key is 0 or the box is upside down (`!(lo <= hi)`). It neither closes the query
  nor counts.
- **Out** does nothing for key 0. It closes an open query and counts the entry out. Under each corner it
  walks down, skipping a corner whose walk meets a missing node. At the leaf it clears the entry from the
  first chunk that holds it (every slot of it), frees that chunk to the pool if it is left empty, and
  stops. A leaf left with no chunks and no children is freed (`+0x880` less one) and its parent's child
  cleared. Heights are never narrowed.
- **The callers** (MD75c1's walk and overlaps) only change where a pick of 0 counts. A newly reached cell
  whose tree has no key for the box is skipped: no in, no link. The carried kind is still that tree's. A
  walk into a new cell calls in whatever the key; in returns at once on 0.

## The port

- `Tree` keeps its nodes by index (`Option<Node>`; a freed one is `None`). Trees compare from the root
  down, so node numbers do not count. Heights are compared bit for bit, and so are the chunks (pool links)
  and their four entries.
- A tree record's or extra's place is `Place::Key(key)`; a list's is `Place::Slot(SlotAt)`.
- `Database::unmarked` is the set of entries whose first halfword a tree's put has cleared. The model keeps
  nothing else of the entry's own memory.
- Closing the query is `Pool::reset` on the query pool, plus the walk words and cells set as the original
  leaves them. What an open query is, is not ported: no query ever runs here.

## Proof

`crates/game-sim/tests/atv_spatial.rs`, against `atv-r-spawn.ee.bin`:

- **`the_spatial_move_is_the_originals`** keeps its 12,000 cases. A third of them now have a tree as every
  cell's first sub-database (kind 1, sometimes 3), and 30% of those trees are shared with the previous cell,
  as the image's two cells share theirs.
  - **The trees.** Origin −6..−10, extent 12–20, so some boxes fall outside. The deepest level is 0–6, and
    one tree in ten has a scale 1.5 times too fine, so the 0x7fff clamp is reached. A chunk pool has stride
    0x14/0x18/0x20 and 4–16 links a chunk. The query pool is random, the query is open half the time, and
    the walk and cells words are random.
  - **What is in them.** Up to 16 other entries are put in by the port and 30% are taken out again, which
    leaves freed nodes and a free chain. Then the mover's home and extras are put in: half of the time
    with the move's own half box, at the last centre or near it. 3% of halves are upside down, and a box's
    sides are tiny, a few feet, or most of the tree.
  - **Laying and reading.** Each tree is laid as the original keeps it: 20-byte nodes, 36 bytes with their
    children after them, the node vtable, chunks as pool links. The heap's file-and-line allocate and free
    are served too. Each entry's first halfword is laid as 0xa5a5. Read back from the root, the tree must
    be the port's. The entries whose halfword the original cleared must be the port's `unmarked`. The
    allowed writes are the area, the stack and the current-tree word.
  - **Result.** All equal. Tree paths seen:
    - the mover's home key: 575 same (moved), 916 another key (out and in), 344 out of its cell's tree, 237
      into another cell's, 808 no key for the box;
    - extras: 791 cases with an extra in a tree, 355 with the mover twice in one tree;
    - nodes: 1,503 made, 846 freed;
    - chunks: 1,129 freed, 143 off the heap;
    - 728 queries closed, and 299 trees changed only in height.

    The test fails if any of these paths is taken fewer than 10 times. The list paths are as before (9,213
    stayed, 2,180 walked, 607 no cell, …).
- **`the_images_tree_moves_as_the_original`** moves the three vehicles' spheres in the image's own tree
  (4,264 nodes, 7,238 chunks, shared by cells `0x006545c0` and `0x00654680`), with half 40 and reach 40.
  The whole database is read back each time:
  - the player's quad (key `0x780900f5`), moved 0.5 ft up: the same key, nodes widened;
  - the player's quad moved 300 ft along x and −150 ft along z: out of four corners and into key
    `0x880880f5`, two nodes made;
  - the player's quad moved 900 ft up: the same key, the root widened from 0 ± 1000 to 31.2 ± 1031.2;
  - each of the two parked vehicles (key `0x15`, last centre at the origin) moved to the origin: the box
    has no key, so out and nothing put in (stamp −1);
  - each of the two parked vehicles moved to the player's sphere: key `0x7c090035`.

  All are equal to the port's.
- **Mutations**, one at a time on the port, were all caught:
  - move does not widen the root;
  - out keeps an emptied node;
  - pick never goes a level up;
  - pick clamps to 0x8000;
  - unfile does not stop after the chunk that held the entry (caught only after shared trees were added to
    the generator);
  - the overlaps enter a cell on key 0;
  - close keeps the query pool;
  - put marks on key 0;
  - widen ignores `lo`;
  - put does not count.

`./kit check -- -p game-sim --test atv_quad --test atv_gather --test atv_search --test atv_spatial`: 69
passed. Run directly, none of them skip.

## Decisions

- **Empty chunks are never generated.** The original's unfile frees an empty chunk it meets before the
  entry and walks on from a pointer it just freed. The port frees it and goes on to the next one. The tree
  itself never leaves an empty chunk (out frees a chunk it empties), so this path is unreachable from the
  tree's own calls. It is not modelled bit for bit.
- **A missing child on the move's walk panics.** In a tree the original built, the nodes down a filed key
  are all there: an emptied node is only freed when it has no children, and the entry under the key holds
  its leaf. The original would read through address 0.
- **The current-tree word is not modelled.** It only steers the node count, which the port keeps on its
  own tree.

## Not verified / not 1:1

- Queries (what makes `+0xc` bit 0 set and fills the walk words) are not ported. Only closing one is.
- A node free with the tree's `+0x98` bit 0 set (keep chunks) is not generated: a freed node never has
  chunks on this path.
- `atv_world`'s own refiling of a vehicle in its tree (`refile`) still simplifies out and in: no chunk or
  node freeing, no heights. Filed as MD90.
- No capture of the original in play. The image's spheres were moved by hand, by the original and the
  port alike.


# MD90: atv_world's refile goes the tree's way

2026-11-03. Task: `./kit plan MD90`.

`atv_world::World::refile` (behind `move_vehicle` and `place_vehicle`) used to clear the sphere from
every slot of the tree and insert it again. It now does what MD89's `Tree` does on the gather's own tree
(`broad_phase::Quadtree`):

- `World::filed` keeps the key each vehicle's sphere is filed under (the record's `+0xc`). `add_vehicles`
  puts the static key there (`0x15` at the origin), as the image has it.
- The new key is `moving_key` (the pick). If it is the same as the filed one, the sphere stays where it
  is. The original's move only widens node heights, and the gather's tree keeps no heights.
- Otherwise `atv_world::take_out`, which is `Tree::take_out`/`unfile` on `broad_phase`'s nodes, takes it
  out from under the old key. It clears the first chunk that holds it and drops that chunk if it is left
  empty. A node left with no entries and no children is unhooked from its parent. The sphere then goes
  in under the new key with `broad_build::insert`, which already was `Tree::put` without heights.

**Why match, not route.** The gather reads `broad_phase::Quadtree`, the static tree MD76a1 proved node for
node. `atv_spatial::Tree` is a different model with heights, a chunk pool, a query pool and counts. Putting
the world's tree there would mean a second copy kept in step, or a new gather, for no difference the
gather can see. An unhooked node stays in `Quadtree::nodes`, empty, where no walk from the root reaches
it.

## Proof

`tests/atv_gather.rs`, on all three R-quad images:

- `a_vehicles_sphere_is_placed_and_gathered_as_the_originals` now also compares the trees' shape (`same_shape`):
  - every node reached from the root, by path, must be in both trees;
  - every node with no scene node in it must hold as many chunks in both.

  This is checked at the start (after the player's move from `0x15` to its `+0xd90` key), on every 50th
  re-filing, and after the walks. A subtree that holds only scene nodes (class 7, which the port does not
  file) is left out. There are two of them in MC02 at the start, and fewer once a sphere has been filed
  into one. Result: each image has 4,271 nodes, the same ones. Its 3,948 re-filings and slots are as
  before.
- `a_sphere_moved_under_its_own_key_keeps_its_slot` (no image needed): two spheres share a node, the
  first leaves it, and the second moves under the same key. It must keep slot 1 and not drop into the
  hole at slot 0.

Mutations, one at a time:

| Mutation | Result |
|---|---|
| an emptied node kept | red (nodes only the port has, call 542) |
| an emptied chunk kept | red (chunk count, call 1) |
| a same-key move taken out and put in | red (the new test; green on the walks) |
| unfile does not stop after the chunk that held it | green |

The last mutation survives because a sphere is never in two chunks of one node.

`./kit check -- -p game-sim --test atv_quad --test atv_gather --test atv_search --test atv_spatial`: 70
passed. Run directly, none of them skip. This slot had no MD37a images or rider file. The images and
captures were copied from s3, and `mrider.phy` was read off the iso's `MXVSATV.PSR`, into `context/`.

## Not verified / not 1:1

- Node heights are not kept, so the move's widening has no counterpart. Nothing the gather does reads them.
- The node and entry counts (`+0x880`, `+0x888`) and the chunk pool's free chain are not kept either.
- A key that names a corner with a missing node on the way is skipped, as in the original. The walks
  never take that path.
