# 1 — the atv heap in Rust (FINAL)

Task F31b5r. 2026-11-15.

## What I did

- **`handling::heap`** (new file, `crates/game-sim/src/handling/heap.rs`): the atv's block heap over
  `node::Ram`. Functions: `alloc`, `free`, `realloc` on one heap; `new`, `delete`, `resize` on the
  manager's current heap; plus `current`, `of_block` and `rounded`. No floats.
- **Two oracles** in `tests/mxvatv_bike_ride.rs`:
  - `the_heap_is_the_originals` runs the port against the original on generated heaps.
  - `the_heap_entries_are_the_originals` runs the entry points against the original on the real boot heap.
- **The switch `"heap"`** in `replay_in_pure_rust` hooks the five entries (alloc, both news, free,
  realloc) with the port. The run prints what the port served.

Result (`in_pure_rust`, both replays still bit-exact on every row, read watch clean):

| | before | with `"heap"` |
|---|---|---|
| ride-2 | 546,207 | 421,673 (−124,534) |
| flip-1 | 245,874 | 173,752 (−72,122) |

- The port served ride-2 7 news, 7 deletes and 9 reallocs. It served flip-1 25 news, 19 deletes and 16 reallocs.
- The "RAM first differs" lines (from parts the frame never reads) are the same 376 with and without the
  switch. I checked this by running the replays once with the switch renamed.
- flip-1 dropped more than the brief's 54k. The rider leaf's wreck-entry news and frees go through the same
  entries, so the port serves them too.

## What the original does

The manager pointer is at 0x0038f750 and points to 0x003f6e10. It has eight heaps of 0x4c bytes at +0x1c.
+0x18 is the current id. Each heap holds:
- a free tree, keyed by size, with its root at +0x1c;
- three lists for free blocks of sizes 0x10, 0x20 and 0x30 (the table at 0x00390638);
- its id, a short at +0x2c;
- a lock at +0x48.

Each block starts with a 16-byte header:
- +2: the heap id;
- +8: the previous block's size;
- +0xc: the size, with the top bit set when free.

Free blocks link with left/right/parent (or prev/next on a list) at +0x10/+0x14/+0x18.

- `./kit fn 0x1b61a0` alloc:
  - Round up to 16. The remainder is signed, so a size of 0 rounds to 16.
  - Take the best fit, with unsigned compares (`0x1b71d0`).
  - Split the block only if a second fit for size+0x20 succeeds.
- `./kit fn 0x1b6760` free:
  - If the tree is empty, file the block on its own.
  - Otherwise join it with its free neighbours: before, after or both.
  - Then file it (`0x1b6d70`). Unlinking (`0x1b6f00`) has four tree cases.
- `./kit fn 0x1b68d0` realloc:
  - Grow in place if the next block is free and holds the growth. A rest under 33 bytes is taken whole.
  - Otherwise alloc, memcpy min(old, new) (`0x354310`, signed sizes) and free the old block.
  - Shrinking works the same way.
- Entries: `0x1b7340` alloc, `0x1b7620`/`0x1b7740` new, `0x1b76d0` free, `0x1b7470` realloc.
  - The news take the C-library path when the flag at 0x40c780 is non-zero.
  - The heap's lock is taken and given around each call (`0x1d66f0`/`0x1d67d0`).
- Rows are in research/oracle.txt under `# F31b5r`.

## How it was proved

- **Function oracle, `the_heap_is_the_originals`:**
  - The heap is made by `hud_heap`.
  - About 12,000 calls run in episodes, each restored from the made state. Blocks are filled with random words.
  - Every byte of the heap's range and every return value is compared with the original.
  - Every path is tallied and must be hit ≥20 times. Last run:
    - alloc: plain 4069, none fits 1065, off a small list 185;
    - free: alone 566, into an empty tree 1089, joined after 1322, before 391, both 544;
    - realloc: grown into all after 53, grown moved 298, split 305, same 1252, shrunk moved 580.
- **Entry oracle, `the_heap_entries_are_the_originals`:**
  - 2,000 entry calls on state 4's own boot heap: 998 news, 667 deletes, 335 reallocs.
  - Two interpreters, one hooked with the port. The whole RAM and scratchpad come out equal, and so do the
    return values.
- **The pure replays above:** bit-exact rows, the bike and its blocks equal, and the read watch clean.

## Not verified / not 1:1

- The aligned alloc (`0x1b73d0`) is not ported. The frame does not call it on these two rides.
- The lock is not modelled. The hook asserts it is idle (owner −1, count 0), where take-then-give leaves RAM
  as it was. A heap call made while the lock is held would differ.
- The C-library path (flag 0x40c780 set) and the not-yet-made manager are not ported. The hooks assert both away.
- The original's move with a failed alloc copies to address 0. The port does the same copy, but the oracle
  avoids that case (it would scribble low RAM), so it is untested.
- Stats (`0x1b72a0`) and out-of-memory (`0x1b72b0`) are no-ops in this build. The port calls nothing for them.
- The pure frames still interpret the other kind send, the wreck check and the wreck's hand-outs
  (F31b5q, s, t, u). `ride/flip_replays_in_pure_rust` stay `#[ignore]`d.
- The heap is still a RAM image held by `Ee`, not a Rust structure.

## Next

- For F31b5: when F31b5q..u land, add their switches next to `"heap"`. Then remove the `#[ignore]`s (F31b5 acceptance).
