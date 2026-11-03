# R26 — the put-back steps once; C21's rows are read a frame late (FINAL)

2026-11-03. Task: `./kit plan R26`, C24's follow-up. C21's strike (`capture/c17/strike.csv`, put back
on 1449) looked as if the original stepped the placed body twice that frame. C24's eight wrecks
(`capture/c24/*.csv`) step it once: try4's row 1418 holds the placed body, and 1419 one step on.
The question: what decides it.

## Result

**Nothing in the game. The original steps the placed body once, from the next frame.** From the
put-back on, C21's capture reads each row one frame late: row `f` holds the state after frame
`f + 1`. The read phase drifts later and later through the wrecked fall, and by 1449 it has passed
the next frame's whole step.

- The C21 replay (`a_captured_rotor_strike_replays_bit_exact`, `crates/game-sim/tests/kiowa_capture.rs`)
  has no second step any more. From `PUT_BACK_ON` it compares each row against the port after the
  next frame (`late`). The measure cut is between that frame's start and end, through `compare_torn`.
- The shell (`KiowaRide::step`, `crates/openmx/src/vehicle.rs`) no longer runs the frame again after
  a put-back.
- `replay_wreck` (the C24 replays) is unchanged; only its comments are updated.

## Why it is the read, not the game

**The drift.** I printed where each fall row of C21 matched in the C21 replay (before this change).
The places are: 0 = after its own frame; 1..8 = before/after each of the next frame's four substep
responses. `p` marks a row equal only part-way through a response.

| rows | where read |
|---|---|
| 1299–1372 | 0 (after the frame) |
| 1373–1395 | 0, 0p, 1, 2 |
| 1396–1415 | 2, 3, 3p |
| 1416–1427 | 4, 5, 5p |
| 1428–1439 | 5–7 |
| 1440–1448 | 4–6 |
| 1449–1473 | "after the frame" *with* the second step, i.e. after the next frame |
| 1474–1501 | inside the next frame's post (measure cut 0–7 of 9) |

So the read moves steadily into the next frame and stays there. The two hypotheses read this
differently:

- **Double step.** The phase would jump back by almost a whole frame at the put-back: 1448 read
  6/8 into frame 1449, then 1449 read right after its own frame.
- **Late read.** The phase is continuous: late in the next frame's substeps (1440–1448), then just
  after its world step, in its post (1474+), or after it (1449–1473).

**The C24 captures don't drift.** Each of roll, try4 and fast1 matched every compared fall row (230,
220 and 220) at place 0, after its own frame, with the same spec (C19's) and the same recorder
script. So their put-back rows show the placed body, and the next row shows one step.

**The code has no second step.** The put-back is asked by the crash test inside the MX measure, the
dynamics world's post (`kiowa_measure_mx` → vt `+0x1b4` → `+0x198`, `research/oracle.txt` S220a/S220a1).
The world step runs its post once a frame, after the substeps. A put-back in the post can only be
stepped by the next frame's world step.

**The tear is deterministic.** `capture/c17/strike.csv`, `capture/c19/probe.csv` and
`capture/c19/probe-run2.csv` are three separate runs of the strike tape. Their `body_raw` is equal on
every row but the first (1204). So the read phase is fixed by the emulated run, not by PINE's
wall-clock jitter. The recorder wakes on `frame_counter` (`[emu] vsync_addr = 0x479d04`), which the
main loop bumps *before* it simulates. That is why a row
can carry frame `f`'s counter, read first, and a body read later in the row.

**The pad cannot tell them apart.** The tape's pad is unchanged from 1296 to the end, and `dt` is
0.03333 throughout. With a constant pad, "an extra step on 1449" and "every row from 1449 one frame
late" give the same numbers on every row. So the earlier passing test did not show a double step.

## Checked

Run with s3's captures and C14's image copied into this worktree's `context/` (not committed).

- `cargo test --release -p game-sim --test kiowa_capture -- --nocapture`: 7 passed, none skipped. The
  strike prints `{0: 143, 1: 1, 2: 11, 3: 15, 4: 11, 5: 11, 6: 9, 7: 2}, 34 part-way through a
  response, 24 inside a post`. Before, it was 144 and 25: the last row (1502) is no longer compared,
  see below.
- **Negative control.** One step, and the put-back's rows compared after their own frame: fails on
  1449, collective 0.29196334 in the game, 0.30728602 in the port.
- `./kit check -- -p openmx --test kiowa_particles --test kiowa_pilot_seat --test kiowa_switch
  --test kiowa_picture --test bays`: 54 passed. `kiowa_switch` wrecks the shell's helicopter and puts
  it back (`a_landing_at_speed_wrecks`); it ran, not skipped.

## Not verified / not 1:1

- **Why C21's reads drift and C24's do not** is not known. A guess: the strike's frames run long in
  emulated time (four blades by a tree, each with a dozen candidates), so the recorder's wake-up falls
  later in each frame. That was not measured.
- **The pad never changes after the put-back in any capture**, so no capture shows directly which
  frame's input the first step after the put-back flies. The port flies the next frame's input step,
  as for every other frame. C24's eight rows after their put-backs are bit-exact with that.
- **C21's last row (1502) is not compared.** It holds frame 1503, which the capture does not reach.
- The rows 1474–1501 are now read as "inside the next frame's post". Before this change they were
  read as inside their own frame's post. Either way it is the same cut-of-the-measure check on the
  same port state.
