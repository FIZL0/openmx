# 1 — atv mid frame rows (FINAL)

Task A7. 2026-11-15.

## What I did

Took the task's first way: a row read mid-frame is compared with the machine at an instant inside the
frame. No re-record, no emulator.

- **ps2-oracle: an opt-in write log.** `Ee::log` (a `RefCell<Option<Vec<(pc, address, byte)>>>`, like
  `Ee::reads`) holds every byte written while it is `Some`, in order. Off by default, so nothing else
  changes.
- **tests/atv_quad/replay.rs, `mid_frame`.** Before each frame the test keeps the body, engine and wheel
  blocks' bytes and turns the log on. When a row differs at the frame's end, it lays the frame's block
  writes one by one over the kept bytes and looks for an instant where every captured byte (about 4.2 KB)
  equals the row. A match is a mid-frame row; no match fails the ride. The old allowance ("at most one
  torn row, not the last") is gone: now **no** row may miss.
- **The put-back spot was laid a frame late on the s- rides.** With mid-frame rows matched,
  s-wheelie-launch still broke at row 158, where the quad is put back. The machine landed on row 157's
  spot (3057.546), the capture on row 158's (3057.766). The spot (quad `+0xd38`, `0x38` bytes) is written
  outside the sim frame, between frames, off the body (under it, on the ground). The recorder reads either
  side of that write:
  - every race- ride's rows hold the spot written off **that row's** body (read after the write), so
    frame r reads row r-1's spot;
  - every s- ride's rows hold the spot off **the row before's** body (read before the write), so frame
    r reads row r's spot.
  Counted over all 15 csvs (a scratch script, not kept): race rides 0 rows the other way; s- rides 1-10
  each, all at near standstill, where the spot hardly moves. `spot_after` tells it per ride the same way
  (which body a moving spot sits nearer, most rows decide), and `before` lays the spot frame r reads.
  The race rides lay exactly what they laid before.
- Raised s-stoppie-short and s-wheelie-launch to `upto 300`. race-jump-side stays at 257 (MD148's).

Nothing taken from opentennis: it has no mid-frame row matching.

## What the original does

- The s- rides' mid-frame rows are real instants of the original's frame. On the replay:
  - s-wheelie-launch 81, 82, 83: 81 at 7304 of 8668 block writes, 82 before its frame (0 writes: equal
    to the machine after frame 81), 83 at 6272 of 7900.
  - s-stoppie-short 124-133, 153, 175-189, 191, 193-195 and 197-299, each at a different point (about
    6000-10400 of about 11000 block writes). From 197 the recorder never reads at a frame's end again.
  - s-jump 74 (7687 of 9460), the row the old test let through.
- From row 158 on, s-wheelie-launch reads every row at its frame's end. C32's "158 on torn" was the
  late spot, not a read.

## How it was proved

- `./kit check -- -p game-sim --test atv_quad -- replays_by`: all 30 pass (15 rides by the original's
  frame, and 15 by the port's pieces beside it, every RAM word equal each frame). Every ride at 300 rows
  but race-jump-side (257); no row of any ride matches no instant.
- The matcher does reject: with the spot still laid late, s-wheelie-launch's rows 158-299 matched no
  instant and the test failed there.
- A mid-frame match is all ~4.2 KB of the blocks equal at one write boundary. Between matches the machine
  is laid nothing of the capture's but the pad and the spot, and later rows match again, so it is on the
  ride.

## Not verified / not 1:1

- **Which code writes the spot, and when in the game loop.** Inferred from the csvs only (spot ≈ the
  body's x/z, a little off; which body it sits nearer). I did not find the writer in the ELF.
- **Why the s- rides read on the other side of the spot write and mid-frame.** Not looked into. The
  phase is steady per ride, and `spot_after` takes it from the rows, not from the ride's name.
- **A ride whose read phase flips mid-ride** would get the wrong spot on the frames after the flip.
  None of the 15 does; a near-standstill row voting the other way (s- rides) does not matter, since the
  spot hardly moves then.
- **s-stoppie-short 197-299 are all mid-frame**, so its end state is never compared at a frame's end.
  Each row still matches an instant of its own frame bit for bit.
- **The pc in the mid-frame list** is the store's pc taken as `pc - 4`. It is for reading only; nothing
  asserts on it. A store in a branch delay slot would show the branch's pc.
- Only frame r's own writes are searched. A row read inside the next frame (a late read) would fail.
  None here.

## Next

None. MD148 (race-jump-side's put-back at 257) is still open; its rows read after the spot write, so the
spot lay is the same as before this task.
