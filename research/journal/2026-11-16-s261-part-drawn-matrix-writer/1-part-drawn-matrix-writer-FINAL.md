# 1 — part drawn matrix writer (FINAL)

Task S261. 2026-11-16.

## What I did

- **Found the writer of each part record's drawn matrix** (part `+0x60..+0x9f`, translation `+0x90`).
  - Static search: `./kit dis 0x100000 0x42b000` into the scratchpad. I listed every store or pointer
    at offsets `+0x60..+0x90` within 30 lines of a `+0x2f4` (parts array) load.
  - That turned up a loop at `0x0030f5e0` that Ghidra never made a function (`./kit dis 0x30f5e0
    0x30f658`). It sits in vtable slot `+0xb0` of all three ragdoll vtables (`0x00458a90`, and the
    body actor's `0x00454410` / `0x00458eb0`).
  - Its only caller is `rider_physics_update` (`./kit fn 0x30f6e0`), as its first call.
  - Named `rider_parts_keep_drawn` in `research/oracle.txt`. S257's and S70's "writer not found"
    lines now point to it.
- **Re-recorded `context/s257/recording2-parts.txt`.** S257's captures were gone: not in the shared
  `context/`, the `old-context-s*` folders or `kept-context/`.
  - First try, NominalScalar 0.5: many rows were read before the tick ran.
  - Second try, at 1.0 with S257's poller: rows were still read early and late under load (~30).
  - The third run kept: `capture/bones_playback.py` here, which is S257's poller with one change.
    After the counter moves, it waits (at most 24 ms) until node 0's world translation changes,
    then settles. It writes a `wait` field on each row.
  - Result: 1700 rows from frame 2374, torn 1, gaps 4 (3460, 3825→3834, 4028→4046, 4050→4057).
  - The tape's md5 is unchanged (`1e5091236f40915ad1e5942bce20287d`).
  - Command: `./kit pcsx2 with -- bash research/journal/2026-11-16-s261-part-drawn-matrix-writer/capture/play.sh
    PID context/recordings/recording2.p2m2 2374 1700 context/s257/recording2-parts.txt` (copy 4).
- **New test**: `tests/trick_ragdoll.rs`
  `a_trick_exit_starts_the_bones_from_the_part_records_as_the_originals`.
- **No port change.** The port's riding entry (`ragdoll_riding.rs` `Pose`) already measures from the
  drawn matrices of the tick before the last. Only its doc comment now says why.

## What the original does

- `rider_parts_keep_drawn` (actor): for each part i < `+0x2e0`, part record = `*(actor+0x2f4) + 0xb0·i`:
  - `part+0x60..+0x9f` = the world rows (`+0x80..+0xbf`) of the part's node (`part+0xa0`);
  - copied as four whole quads, so bit for bit.
- Order within a tick, in `rider_body_update` (`./kit fn 0x24e640`):
  1. `rider_body_mode` (`./kit fn 0x24c7e0`) runs, and calls every entry: the riding entry
     `0x0024c490` and the wreck entry. Each entry's pose (`rider_ragdoll_pose`) reads `part+0x60`.
  2. Then `rider_physics_update`: first the copy (vt `+0xb0`), then the animation (vt `+0xa8`), the
     step and the node write-back (vt `+0xac`).
- So, on every tick, in every mode, the record holds the node as the tick before left it. An entry on
  tick E reads the node as E−1 left it, against the record E−1 copied: the node as E−2 left it.
- On a held tick the repose rewrites the record afterwards (`mat4_basis` at the body centre, S201).
- The copy is skipped only under the app's debug bit (`+0x325` bit 3), which skips the whole update.

## How it was proved

All against the new capture (`context/s257/recording2-parts.txt`).

- **The writer.** On every non-held tick with a row before it (1691 ticks, 388 posed), each of the
  18 records equals its node's world rows from the row before, bit for bit. That holds on all but 7
  ticks: 2420, 2750, 2761, 2925, 2930, 3236, 3478.
  - All 7 are rows the recorder read a tick off. Each record there matches the row two back or no
    row, and 2750 and 2925 are rows where the poller gave up waiting.
  - The test allows under 1% of such ticks.
  - The 4 ms capture from the second try shows the same split, with more off rows.
- **The trick exits**, 2595, 2698, 3388, 3813 and 3981. The rows E−1 and E are both clean on all five.
  - The port's pose (`Down::entry_starts`, the same `pose_all` the riding entry's `Pose` runs) of the
    original's nodes at E−1, against the original's records at E−1, gives every part's
    `start{i}` position and velocity **bit for bit**.
  - It also matches bit for bit against the port's own drawn matrices, rebuilt from the E−2 nodes
    (`ragdoll_ride::drawn`).
  - From the skeleton composed off the captured joints (the port's skeleton, under the bike) the
    starts are within 0.0034 ft and 0.088 ft/s. The test bound is 5e-3 ft / 0.1 ft/s.
- S262's `a_bailed_trick_enters_the_wreck_from_the_clip_as_the_originals` passes on the new capture
  (3563 bit for bit; composed 0.00098 ft, 0.037 ft/s).
- `./kit check -- -p game-sim --test trick_ragdoll a_trick_exit`: passes.

## Not verified / not 1:1

- **The port's `Riding` was not fed the original's nodes.** Its entry runs the same arithmetic as
  the test's call, but on the skeleton it builds itself. Through `Riding` the exit starts are proved
  only to the composed skeleton's 0.0034 ft, not bit for bit. The starts are not visible after the
  step, and the composed skeleton is the port's input, not the original's.
- **The start rates and quats** (`+0x140`, `+0x160`) are not compared. The poller logs `+0x30..+0x4f`
  only.
- **S257's own test** (`a_trick_lets_the_riding_ragdoll_go…`, which reads `recording2.txt`) was not
  re-run green. On this capture it finds 7 tricks and 388 posed ticks: the 4 gaps carry the rows to
  frame 4105, past a backflip at 4086. It also counts a late row (2930) as a 2.29 ft riding miss,
  against its fixed (6, 368). I did not leave `recording2.txt` in the shared context, so it skips as
  it did before this task. Filed as K56.
- **Seven ticks** of the writer check are rows the recorder read off their tick (above). The
  record there is not checked.
- **The debug-bit branch** that skips the copy was read, not run.

## Next

Nothing waiting for this task. K56 is the re-recording of S257's capture.
