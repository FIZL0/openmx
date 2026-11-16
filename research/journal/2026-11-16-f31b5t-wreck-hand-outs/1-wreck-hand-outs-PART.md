# 1 — wreck hand outs (PART)

Task F31b5t. 2026-11-16.

Answer: every hand-out from flip-1's wreck except the event posts is now Rust. The posts stay with the
original: they switch the camera, and that touches the frame's heap and spatial database. So rows 125 and 193
still run about 11.3k interpreted instructions. All 465 rows stay bit-exact with the read watch on.

## What I did

- `handling::wreck`:
  - `stance_cancel`: the stance with id −1, which closes and tallies the run.
  - `kind_set`, `category` and `add_category`.
  - `camera_moved`: the camera reset.
  - Trail reset: `trail_reset`, `trail_item_reset`, `trail_sound_reset`, `sound_stop`.
  - New `Out` variants: RunClose, RunTally, Realloc, NodeFree, CameraTell, TrailItem, Douse, Halt, Stop.
- `handling::attach`:
  - `remove`: the spatial unhook.
  - `reattach`: the rider part's reattach.
- `tests/mxvatv_bike_ride.rs`, `replay_pure`'s "wreck hand-outs" switch:
  - Serves the ports above.
  - Puts back the record's sweep probe through `attach::part_in_world`. Its box is `atv_sweep::bounds` of
    its segments, read from RAM.
  - Hands the douse and halt to `effects::object_douse` / `object_halt`.
- `bike_and_blocks` now pauses `ee.reads`. Before, the test's own snapshot reads tripped the read watch.
- The "trail reset dropped" switch is gone, because the trail is ported. The "wreck posts dropped" switch
  came off the list. Its arm in `handed` and the bike-compare exception it brings remain, but are unused.

## What the original does

Addresses and the call chains are in `research/oracle.txt`, block `# F31b5t`. In short:

- Stance id < 0, with an open run (`./kit fn -p atv 0x4e4e50`): run close, then run tally.
- Kind set (`0x4e19a0`):
  - Hung parts' `+0x17c` = 0.5.
  - A unique-add of four category bits to the frame and riding parts. The bit list grows through the heap
    realloc.
- Trail reset (`0x4fa760`):
  - Each of the seven item classes clears its flag bytes and douses its emitter.
  - The sounding class also halts its emitter and stops it (`0x4e9f30` → `0x24be50`: clears the voice
    table, and sets 24 words per block to −1).
- Posts (`0x490e60`): the camera controller switches cameras (`0x490b30` → `0x113290`):
  - Unhook and re-insert in the spatial database.
  - A sorted list add.
  - Heap realloc pushes and a `new`.

  Dropping the posts parted the heap by row 193 (reads of 0x3f6e4c, 0x16e098c and 0x1efa34c), so they
  can't be dropped.

## How it was proved

- `atv_spatial.rs the_spatial_remove_is_the_originals`: function oracle, 10,000 cases (seed 0xf31b5701).
  - Tally: 11,115 frees.
  - List extras false/true: 4149/3324.
  - No record: 995. Pool deleted: 1804.
  - Tree extras false/true: 943/589.
- `atv_spatial.rs the_reattach_is_the_originals`: function oracle, 10,000 cases (seed 0xf31b5702).
  - List kinds 1/2/3: 1071/5629/1079. Tree kinds 1/3: 1126/1095.
  - About 5k cases with a parent, and about 5k without.
- `mxvatv_bike_ride.rs the_kind_sets_categories_are_the_originals`: function oracle.
- `mxvatv_bike_ride.rs flip_replays_in_pure_rust` (ignored, run with `--include-ignored`), as a bit-exact
  replay:
  - All 465 rows match C15.
  - Bike and blocks match between the handing run and the ported run.
  - The read watch is clean: the ported frame reads no byte where the two runs differ.
  - The census of what's left: "the wreck hands on: Post" 11,250. Nothing else from the wreck.

## Not verified / not 1:1

- **The posts are not ported.** The camera controller and its switch run interpreted on rows 125 and 193,
  about 11.3k instructions. The task's "no interpreter on rows 125/193" is not met. Follow-up F67.
- The trail item resets, `sound_stop` and the sweep probe's box are checked only by the flip-1 replay, not by
  an oracle. On flip-1 that covers seven classes and one sounding item. The other branches have not run
  against the original: an item with no `+4`, and a stop loop with more than one block.
- The rumble drop is shown to leave the frame alone on C15 flip-1 only.
- The stance with id ≥ 0 and `Kind(false)` are not ported. Neither happens on flip-1.
- The camera reset's two lists are empty on flip-1, so its node free and listener calls are untested there.
  They follow the disassembly.
- The rider leaf's interpreted instructions belong to F31b5u, not to this task.

## Next

- Port the camera switch the posts start (F67). See the chain in `research/oracle.txt` `# F31b5t`.
- Then flip-1's rows 125 and 193 run with no interpreter, apart from the rider leaf (F31b5u).
