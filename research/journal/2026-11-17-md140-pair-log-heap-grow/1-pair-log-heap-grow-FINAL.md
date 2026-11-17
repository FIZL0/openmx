# 1 — pair log heap grow (FINAL)

Task MD140. 2026-11-17.

## What I did

- Ported the game's heap resize to Rust on the heap's words in RAM: the pool pick, best-fit search,
  free-tree and small-list unlink/insert, alloc with split, free with joins, grow in place, and grow by
  move (alloc, copy, free). New shared file `crates/game-sim/tests/mxvatv_bike_ride/heap_in_ram.rs`,
  mounted by `atv_quad/record_outside.rs` and `mxvatv_bike_ride.rs` (the "bike glue"), the same way
  `gather_in_ram.rs` is shared.
- Both pair-log listeners now grow the log through `heap_in_ram::resize_checked`: the original's resize
  runs first on the RAM as it is and is put aside, then the Rust one runs on the same RAM; every RAM word
  outside the stack and the answer must match, and the Rust result is what the ride keeps.
- `the_palm_strikes_contacts_push_the_ports_quad` now asserts the pair log grows exactly twice.
- opentennis: nothing to take (its only realloc is the wasm host's).

## What the original does

- `./kit fn 0x001b7470 -o atv` (resize): picks the pool (`./kit fn 0x001b5ec0` new block: the pool whose
  id is the manager's current one; `./kit fn 0x001b5ed0` old block: by the id at block-0xe), then
  `./kit fn 0x001b68d0`.
- `0x001b68d0`: rounds bytes up to 16 (0 → 16). Same size: nothing. Bigger and the next block free with
  room: unlink it, take it all if fewer than 0x21 bytes would be left, else split the rest off as a new
  free block. Otherwise alloc (`0x001b61a0`), memcpy min(old, new), free (`0x001b6760`).
- `0x001b61a0` alloc: best fit (`0x001b71d0`: the small list first, then the tree's exact fit or the
  smallest bigger). A bigger fit is searched again for bytes+0x20 and split.
- `0x001b6760` free: joins a free block before (by the +8 size) and after, then inserts.
- `0x001b6f00` unlink and `0x001b6d70` insert: three small sizes (`0x00390638`) have their own doubly
  linked lists; every other size lives in an unbalanced tree by size (+0x10 left, +0x14 right, +0x18
  parent). Ghidra dropped the third argument (the size) at two unlink calls; `./kit dis 0x001b6950
  0x001b69d0 -p atv` and `./kit dis 0x001b67c0 0x001b6840 -p atv` show it is the unlinked block's size.
- The lock (`0x001d68b0`/`0x001d68f0`, the pool's mutex vt +8/+0xc) and the stats calls (`0x001b72a0`,
  `0x001b72b0`, empty) are not ported: the RAM diff shows they leave no word changed outside the stack.

## How it was proved

- Live check on every grow: the original's resize vs the Rust one from the same RAM, every word below
  the stack compared, plus the answer (`heap_in_ram::resize_checked`).
- The palm ride (`the_palm_strikes_contacts_push_the_ports_quad`, 300 frames) grows the pair log twice:
  0x1a0 bytes moves the log (alloc + copy + free), 0x2a0 bytes grows it in place. Both match word for
  word, and the whole-frame RAM check against the original's frame still passes.
- The bike ride: `flip_replays_with_the_frame_posts_tells_told` grows its pair log once (0x1a0 bytes,
  moved); it matches too. No other test in `mxvatv_bike_ride` reaches a grow.

## Not verified / not 1:1

- Branches the palm ride never takes are unproven: a grow into the next block that leaves 0x21 bytes or
  more (split), a small-list hit, a tree unlink of a block with two children, alloc failing (heap full),
  shrinking, and resize of a null block. Reason: only the two grows on this ride run them, and no
  oracle over generated heaps was built (the task is low priority; the checked call guards every grow
  that does happen).
- Whether free's joins ran on the moving grow is not recorded; the RAM match covers whatever path ran.
- The heap's first-use setup (`0x001b5c90`) is not ported: the Rust resize asserts the heap is set up.
- The lock and stats calls are left out (no RAM effect outside the stack, by the diff).

## Next

(none)
