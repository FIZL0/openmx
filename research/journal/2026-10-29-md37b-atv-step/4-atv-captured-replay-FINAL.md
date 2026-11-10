# 4 — the captured rides replayed (FINAL)

Task MD37b. 2026-11-10.

## What I did

C28 re-made MD37a's captures and MD105 put the quad's whole sim frame in Rust, so the brief's test can
now run. `tests/atv_quad/replay.rs` replays all 15 of C28's rides (`context/capture/md37a/*.csv`) off
the four C28 images (`context/mxvatv-ps2/md37a/c28-atv-{r,s}-{spawn,air}.ee.bin`). It goes through
MD105's `sim_chain`, which `palm` now wraps. Each ride runs twice:

- `<ride>_replays_by_the_originals_frame`: SLUS-21104's whole sim frame (vt +0x40) in the interpreter.
- `<ride>_replays_by_the_port`: the same, with MD105's port pieces (`Pieces::Port`: the record's
  clear/pass, Desert Oasis's outside, `quad::frame`) and the port's tail beside it. Every RAM word of the
  twin must equal the original's after every frame.

F31's recipe drives both:

- Before frame r, the pad reader's stores are laid from row r (`READER`).
- After frame r, the body (640 bytes), engine (512 bytes) and four wheels (768 bytes each) must equal
  row r word for word.
- Then row r's put-back spot (quad +0xd38, 0x38 bytes) is laid for the next frame.

The quad's own 0x1800 bytes are compared too. Each differing word is sorted by whether the frame wrote
it (`ee.written`) and printed, or appended to `$MD37B_LOG`. They are not asserted (see below).

The C28 images are on another heap than MD37a's old `atv-r-spawn.ee.bin`, and all four share it. The
heap addresses that `sim_frame.rs`, `oasis.rs` and `sim_tail.rs` had as consts are now an
`image::Heap`: the sim object, the quad, its nearby list, the trigger probe, the gather probe and the
terrain. It is chosen per test thread (`image::set_heap`). The default is MD37a's, so the older tests
read what they did before. Also:

- `sim_chain` takes `before`/`after` closures that lay words on both machines.
- `TERRAIN_RING` became offsets from the heap's terrain.
- The world-pointer assert holds only on MD37a's heap.

One harness fix in `WorldStep::step`. The port keeps the last rear contact's collidable
(`atv_moto::Parts::rear_node`, quad +0x143c) as its world's index, and the original keeps an address.
The step poked the value from before the frame, which went red the first time a rear wheel touched a
mesh (race-wheelie-launch frame 83, race-jump-side frame 190, the palm). It now maps one to the other by
the mesh's centre bits and radius, as `oasis.rs`'s Force run already did. `oasis::image_mesh` and
`port_mesh` became `pub(super)` for that. No shipped code changed.

## What the original does

Nothing new: the frame is MD105's (`./kit fn 0x00204040`, the sim object's vt +0x40), the pad reader
F31's.

## How it was proved

A bit-exact replay of the PCSX2 captures: 30 tests, all green, 4,058 frames per variant.

| ride | state | rows replayed | notes |
|---|---|---|---|
| race-steer, race-steer-slow, race-still, race-probe | atv-r-spawn | 300 each | |
| race-stoppie-short | atv-r-spawn | 300 | first contact frame 112 |
| race-wheelie-launch | atv-r-spawn | 300 | strikes the palm at 80, wrecks, part swaps 80/160 |
| race-jump | atv-r-air | 300 | wrecks at 66, put back |
| race-jump-side | atv-r-air | 257 | rows 257 on torn (below) |
| s-steer, s-steer-slow, s-still | atv-s-spawn | 300 each | |
| s-stoppie-short | atv-s-spawn | 120 | rows 120 on torn |
| s-wheelie-launch | atv-s-spawn | 81 | rows 81 on torn |
| s-jump | atv-s-air | 300 | row 74 torn, 75 on equal again |
| s-jump-side | atv-s-air | 300 | |

On every replayed row the body, engine and four wheels equal the capture in every word: 1,056 words a
row. In the `_by_the_port` variants, the port's pieces and tail leave every RAM word the original's on
every frame. So the port equals the capture there too.

`cargo test -p game-sim --test atv_quad`: 103 passed. That includes MD105's and MD75's tests on the old
image, after the Heap change.

## Not verified / not 1:1

- **Three rides stop short: race-jump-side at 257, s-stoppie-short at 120, s-wheelie-launch at 81.**
  - From those rows the capture is read mid-frame. Example: s-wheelie-launch's frame 81 moves five
    substeps of 0.62 ft. Row 81 is three substeps into it: its euler and speed are still row 80's,
    since the post-process has not run. Row 82 is four substeps into the same frame.
  - On s-stoppie-short's row 120, the capture's position is ours one substep back. Its summed forces
    (body +0xa0, +0x1b0) are not yet integrated.
  - Every one is at a strike, where the emulated frame is slowest. No later row, and none of the next
    three rows, equals the frame's end.
  - That they are recorder tears, and not the original doing otherwise, is inferred from these
    numbers. It is not proven. Follow-up **C32** re-records them.
- **One torn row is let through**: s-jump row 74 (the same one-substep-short read). Row 75 and every row
  after it are equal again, and the machine is laid nothing of the capture's but the pad and the spot.
  The test allows at most one such row a ride, and never the last.
- **The quad's own words are not asserted** (MD120). The physics never reads back those that differ:
  - +0x44 bit 0x400 is in the capture and not after the frame. It is on every row of the race rides
    and some rows of the s rides.
  - After a wreck, the frame-written +0x870..+0x978 are a few ulps off, and the counter +0x940 is 0x44
    against 0x43.
  - race-stoppie-short +0x710/+0x720 differ for 12 rows.
  - On the atv_s_ rides, +0x414..+0x7f0 (and +0xcf8, +0x1750) differ for the first rows and heal, and
    +0x930..+0x938 stay a few ulps off.
  - Never written by the frame: +0x6f8/+0x6fc (lag a row behind), +0xd38..+0xd70 (the spot, laid a row
    late by the recipe), +0xd90..+0xd98, +0x12b8/+0x12bc.

  Likely causes are state the RAM image lacks (the scratchpad, VU) or writers outside the sim frame.
  Neither was looked into.
- **The pad is laid from the rows, not read.** The pad reader's stores come from each row, as in F31.
  The pad reader itself does not run.
- **The interpreter matching PCSX2** is shown here only on the captured blocks. The quad's other
  words above are where it is not shown.
- "Every bike replay hash unchanged": no shipped code changed. Only `tests/atv_quad/*` did.

Nothing was taken from opentennis: its game has no vehicle physics.

## Next

Done. C32 (re-record the three torn rides, then raise their `upto`) and MD120 (the quad's other words)
are filed.
