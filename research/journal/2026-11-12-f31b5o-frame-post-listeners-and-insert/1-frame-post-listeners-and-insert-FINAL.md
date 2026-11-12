# 1 — frame post listeners and insert (FINAL)

Task F31b5o. 2026-11-12.

## What I did

- **The kind clear's show, Rust's.** `update::Out::ObjectOn(obj)` is the object's child scene node off
  (its child's vtable `+0x20` is F44's node off), so `UpdateInRam` serves it with `update::trail_off(obj
  +0x24)` when the slot holds the original. `update::Out::Insert { obj, space, at }` is new in
  `handling::attach`: `part_in_world` (the reach from the half box, the flag byte's bit 3) and `insert`
  (the record taken up, a heap record or the call's own, the cell's sub-database for the kind, pick, put,
  the record's words, the overlaps, the record let go), with `record_new` / `record_drop` for the link pool's
  count, made on the first record and deleted with the last. `Space` gained `delete_pool` and the
  `Site::Record` / `Site::Pool` allocations. The test serves the insert with the part's box from RAM
  (`part_box_in_ram`, `atv_sync::part_box`) and the own record in scratch 14. That scratch sits under the
  `0x400` above the stack top that the RAM comparisons leave out. Scratch 67 was above it, and the flip
  parted at `0x1ffd100`.
- **The frame post's listeners.** The every-port replays (`replay_whole`) now run with `"frame post tells
  but the hud's dropped"`. Ten of the eleven tells are left out (race progress, speed flag, menu list, the
  five pickers, camera controller, empty receive), and `replay_pure`'s read watch is on: the frame reads no
  byte where the RAM parts from the handing run's. `three_served` asserts that the only tell left is the
  HUD's and that neither ObjectOn nor Insert is handed on. The read watch no longer logs what TreeInRam's
  original-beside-the-port run reads. That run is the check's reference, and with the tells dropped it read
  the HUD's and race progress's own parted words (pcs `0x00297208`, `0x0029cc18`).
- **Why the HUD stays told.** With all eleven dropped (the F31b5l tests), the flip's frame at the wreck
  (row 125) reads parted heap words: the allocator (`0x001b6xxx`) walks headers the HUD's tells changed,
  and an ini reader (`./kit fn 0x1a7b30`) memcpy's 0x104-byte strings out of a reused block whose stale
  bytes the HUD left (`0x016d94b0`). The HUD allocates and frees on the shared heap: about 60k instructions
  on the rows it does, ~600 otherwise. Its tell is presentation, but its heap traffic is in the sim's heap
  until the presentation gets its own (follow-up filed).
- **New tests.** `ride/flip_replays_with_the_frame_posts_tells_told`: every port with all eleven tells
  told, so TreeInRam checks each bike step on all of RAM against the original's. ObjectOn and Insert are
  served once each on the flip, and RAM is the handing run's (the flip's cue word aside).
  `atv_spatial::the_spatial_insert_is_the_originals`: the insert oracle.

## What the original does

See research/oracle.txt `# F31b5o`. Recipes: `./kit fn 0x1143e0` (part in world), `./kit fn 0x18d590`
(insert), `./kit fn 0x18fd90` / `0x18fe20` (record new / drop), `./kit fn 0x18feb0` (file), `./kit fn
0x105c90` (pool init).

## How it was proved

- Function oracle: `atv_spatial.rs the_spatial_insert_is_the_originals`, 6000 MD75c1-generated databases.
  Each case puts a random entry into a random cell under kind 1/2/3 (lists and trees), with a heap record
  or the own record, and with no link pool 20% of the time (pool made; with the own record and count 0,
  the pool deleted). Every byte written outside the stack is the same, and so are the heap calls (file and
  line) and the pool delete. Tally includes list kinds 1/2/3, tree kinds 1/3, new lines 0x42 and 0x13, and
  the pool deleted.
- Replays (C15), tests/mxvatv_bike_ride.rs: `ride/flip_replays_with_every_port_at_once`. All 459/465 rows
  are C15's word for word and the bike and its blocks are the handing run's. The read watch is on, with no
  parted byte read. RAM parts only at the race progress's twelve word ranges (`RACE_PROGRESS_WORDS`, plus
  one re-reported range on the ride, plus the cue word on the flip). The census shows the HUD's tell alone
  (455k instructions over the ride, 512k over the flip) and no ObjectOn/Insert handed on.
  `ride/flip_replays_with_the_frame_posts_tells_told` pass, with all of RAM the handing run's.

## Not verified / not 1:1

- The HUD's tell still runs in the original's code in the every-port replays (its heap traffic is shared
  with the frame's; see above). Follow-up F31b5p. So the task's "no frame-post tell" holds for ten of the
  eleven, not the HUD's.
- The ten other listeners are dropped, not ported. The read watch proves the frame does not read their
  words on C15's two replays only.
- The race progress and HUD are not ported to presentation in this task; they are left out of (or kept
  in) the pure frame only.
- The insert's pool-create and pool-delete paths and the own-record (bit 3) path run only in the oracle.
  On C15, state 4's frame mesh has `+0x126 = 0x94` (heap record) and the pool exists.
- The insert is served only for the ray probe and mesh classes (vtables checked); other classes still
  hand on.

## Next

F31b5p: the HUD's heap, then drop its tell.
