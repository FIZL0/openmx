# 1 — attachment move (FINAL)

Task F43b. 2026-11-05.

## What I did

- The task split in two. The collision record's search went to **F49**, already in plan/TODO.md. F43b is
  the attachment move only.
- `crates/game-sim/src/handling/attach.rs` (new): `bounds`, `moved` and `spatial_move`. They are a
  word-level port over RAM, through the `Space` trait (`node::Ram` plus class lookup, heap calls by
  `Site`, free, and a pool constructor). `Globals` hands in the link pool's and the current tree's
  words and the tree node's two classes. So no retail address is in the shipped code.
  - It reuses the proven MD75c1 model where it can: `atv_spatial::crosses`, `touches` and `Tree::pick`
    (on a view read from RAM).
  - Ported: the walk, the overlaps, the link pool (take, chunk, give, reset), the list's pick, put and
    out, and the tree's put, moved and out (grow, new node, widen, file, unfile, node delete, close).
  - `handling.rs`: `pub mod attach;`, nothing else.
- `tests/atv_spatial.rs`, new tests:
  - `the_spatial_move_on_ram_is_the_originals`;
  - `the_images_moves_on_ram_are_the_originals`;
  - `the_attachment_box_is_the_originals`;
  - a shared `twin` comparer.
- `tests/mxvatv_bike_ride.rs`: `NodeRam` implements `attach::Space`. Its heap calls and constructor
  go to the original functions through `ee.call`, censused under `heap Moved`. `UpdateInRam::out`
  serves `Out::Moved` with `attach::moved`, and the interpreter arm is now `unreachable!`. In
  `RIDE_CALLEES`/`FLIP_CALLEES`, `update Moved` became `heap Moved`, so the sizes are unchanged at 11
  and 29.
- `research/oracle.txt # F43b`: the addresses and layouts.

## What the original does

Recipes: `./kit dis -p atv <addr>` with the addresses in `research/oracle.txt # F43b`.

- **Attachment.** With no record (`+8`) it does nothing. Otherwise it computes the box and then moves
  the record with the entry's reach (`+0x60`).
- **Box.**
  - It brings the entry's node (`+0x34`) up to date.
  - centre = `adda(pos, 0).madd(r0, lx).madd(r1, ly).madd(r2, lz)`.
  - half = `adda(|r1·hy|, |r0·hx|).madd(|r2·hz|, 1)`.
  - The 1.0 it stores in each `w` is overwritten by the whole-quadword store. So `w` is the local
    centre's and the local half's own `w`.
- **Move.** The same as MD75c1's model (the walk through portals, then same cell or new cell, then the
  overlaps), but on the original's words:
  - The link pool is a chunked free list.
  - The list is blocks with a free chain tagged `| 2`.
  - The tree is a quadtree with four-slot chunks taken from the current tree's pool, nodes made with
    the heap's `new`, and children arrays grown by `realloc`.
- **One quirk the port keeps.** In unfile, after an emptied chunk goes back to the pool, the walk reads
  the chunk's `+0` as its next. By then that word is the pool's free head. See Not verified.

## How it was proved

- **Function oracle, 12 000 generated databases.** `the_spatial_move_on_ram_is_the_originals` uses
  MD75c1's generator and seed, so it reaches the same paths as `the_spatial_move_is_the_originals`.
  - For each case, the original's move runs in the interpreter. The area and the current-tree word are
    then put back, and the port runs on the same RAM and heap.
  - Every byte either side wrote must match, the stack aside. The original must write nothing outside
    what is put back. The heap calls (function, size, file and line) must match in order.
  - Each kind of result must reach at least 10 (tally of one run): wrote 11 393, wrote nothing 607,
    list block alloc 2412, pool chunk alloc 1958, list blocks realloc 2412, tree grow realloc 2867,
    node new 5531, free 2300.
- **The image.** `the_images_moves_on_ram_are_the_originals`, in the same twin on the whole of RAM:
  - the two record entries, nudged and through a laid portal (a new pool chunk on the image's pool);
  - the three vehicles' spheres in the image's 4264-node tree, moved five ways (two new nodes on the
    long move).
- **Box, 4000 cases.** `the_attachment_box_is_the_originals`:
  - The node is random words, with or without a parent, and either current or not.
  - The lanes include ±0 and 1.
  - Half the cases are the box alone. The other half are the whole attachment with no record, where
    both sides must write nothing.
- **Replays.** `ride_replays_bit_exact_on_the_ports_record_pass` (ride-2) and
  `flip_wrecks_and_is_put_back_bit_exact_on_the_ports_record_pass` (flip-1) stay bit-exact with the
  port serving `Moved`.
  - To show they ran, I raised their row-count asserts temporarily. Both failed on the count only (459
    and 465 rows), and I put the asserts back.
  - The census shows that the moves allocate (`heap Moved`), and no callee writes the physics blocks.
  - All 57 tests in mxvatv_bike_ride pass.
- **Mutants.**
  - `bounds` with `w` = 1 was caught by the box oracle (case 3).
  - Unfile "fixed" to continue at the chunk's next read before the give was **not** caught (see
    below). I restored both.

## Not verified / not 1:1

- **The search** (the collision record's, F43's other half) is not in this task. It is F49.
- **The unfile quirk is unproved by any test.** I ported it from the disassembly of 0x00180c80, and the
  mutant survived. The path needs a node with an empty chunk that does not hold the entry, and
  neither the generated layouts nor the image have one. The game cannot make one either: an emptied
  chunk is always given back. MD75c1's model (`atv_spatial::Tree::unfile`) continues to the next chunk
  instead. The two differ only there.
- **Vtable dispatch is by class.** The port reads the sub-database's vtable through `Space::class`
  (list, tree, or anything else, which panics). It does not follow the vtable's slots. A third class
  would panic, not run.
- **The node delete assumes the one node class.** The destructor's two class stores are the handed-in
  `node_class`/`node_base`, and the children are deleted as the same class.
- **The reach's 35-cell stack array.** The original gathers reached cells into a fixed stack array;
  more than 35 would overrun it. The port's vector does not.
- **The whole attachment with a record** (box, then move) is proved only by the two replays. The image
  entries are not attachments (no node at `+0x34`), and in the generated layout the mover's fields
  overlap the other entries. The box and the move are each proved alone.
- **The pool constructor** (`+0x2c`) is never exercised. No generated pool and no image pool has one.
  The rig calls the original, and the test's `construct` panics.
- **Heap failure.** An alloc returning 0 is mirrored (the chunk returns 0), but never tested. The
  heap is a bump hook in the oracles. The rig calls the real one.
- **The pool's own heap** (`ChunkIn`, `+0x30`) is never reached by the generated pools or the image.
- **Moved with a null child.** The original has no null check and reads address 8. The port mirrors
  this, but it is never reached.

## Next

(FINAL.) None for this task. F49 ports the search.
