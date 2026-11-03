# B24 — fast2's in-flight parting is a torn read, not the controller (FINAL)

2026-11-03. Task: `./kit plan B24`, C24's follow-up. C24's fast2 tape (`60 80:cross 200:-:127,0
200:square:0,0 400` from C14's state; capture `c24/fast2.csv` in `context/capture/`) parted from the
port on 1416, in flight before its wreck. The lateral cyclic was -0.08864668 in the game and
-0.08283156 in the port.

## Result

**The port is right. Row 1416 was read inside frame 1417.** The game's lateral cyclic, longitudinal
cyclic and pedal on that row are, bit for bit, what the port's controller writes in 1417's first
force listener. The body is the port's after 1416 with gravity added. With fast2's rows compared where
they were read, fast2 replays bit-exact from 1205 to 1671. That covers the flight, the wreck on 1442,
the slide with its blades offered on 150 frames and none met, and the put-back on 1593. Row 1672 was
read after a frame the capture does not hold, so it is not compared. No product code changed.

How fast2's rows were read:
- **Flying rows 1416, 1420, 1422 and 1436–1439** were read inside the next frame's step, part-way
  through a force listener. 1416 has the next frame's controller and rotor pitches but not its rotor
  force. 1422 has the rotor's tilt and pitches but its thrust is still the old one. 1438 is in the
  second substep.
- **Wrecked rows 1466 and 1467** were read inside the next frame's substeps: 1467 at the fourth
  substep's response.
- **From 1468** almost every row was read after the next frame, as C21's are from its put-back (R26).
  The exceptions are 1576, 1584 and 1585, read inside it. There is no duplicated body to give this
  away, unlike try5's 1304 (B23).
- **1592**, read after frame 1593, has its wreck word, flight state and body from before 1593's post
  put it back. Its pad is the one 1593 ran on.

The drift is the same one R26 found in C21: the recorder wakes on `frame_counter`, which the main loop
bumps before it simulates, and reads the row's blocks in column order (pad, `heli_a`, `heli_b`,
`rotor_state`, `aero_state`, the body).

## What changed

All in `replay_wreck` (`crates/game-sim/tests/kiowa_capture.rs`):

- **Every row** now waits for the next frame and goes through `compare_torn`. Before, rows up to the
  wreck were compared only after their own frame.
- **`next_forces`** works out the flight state each of the next frame's force listeners leaves:
  controller, rotor, aerodynamics. It uses each substep's body.
- **`flight_read`** finds the earliest point in those listeners where the row's flight block equals
  the port's bit for bit. Each listener is written group by group (`flight_cut`, `FLIGHT_GROUPS`):
  - the commands and the loop;
  - the rotor's tilt and pitches;
  - its thrust;
  - its force;
  - the aerodynamics.

  A whole body must not stand earlier than that point, since the row reads the flight blocks before
  the body. This is asserted.
- **A row whose body is neither the frame's nor any next-frame substep's** is compared after the
  next frame. Its pad is the one that frame ran on. On a put-back frame whose row shows the put-back
  not yet done, it takes the wreck word, flight state and body from before the post. `Struck::post`
  now keeps that body (`posted`).
- **A last row read after a frame the capture does not hold** is reported (`Replayed::unread`), not
  compared.
- **Replayed** gains `torn` (flying rows read in the next frame) and `unread`. The roll, tip, fast
  wreck and right tip tests now assert both are empty.
- **New test:** `a_fast_left_descent_read_ever_later_replays_bit_exact` (fast2). It pins:
  - the wreck (1442) and the put-back (1593);
  - the torn rows, the late rows and the unread row;
  - blades offered and none met.
- **`compare`** has its flight-state checks moved into `flight_mismatch` and **`compare_torn`** its
  body match into `body_read`, so the new code can reuse them. Same checks, same messages.

## Checked

- `./kit check -- -p game-sim --test kiowa_capture`: 9 passed. Their summaries show all five
  `replay_wreck` replays ran (roll, tip, right tip, fast wreck, fast left descent), and so did the C21
  strike. Two tests skipped: they need `capture/c14/flight.csv`, which is in no slot on this disk.
- **Negative controls**, each seen while building this, in order:
  - the row compared after its own frame fails on 1416 with C24's numbers;
  - without the late rows, it fails on 1468: speed 33.434258 in the game, 34.118935 in the port.
    The game's row equals the port after 1469;
  - with the pad polled for the frame after, it fails on 1474 (descend 1 in the game, 0 in the port).
    The tape lets go of Square there;
  - without the pre-put-back state, it fails on 1592 (wreck word 1 in the game, 0 in the port).

## Not verified / not 1:1

- **`flight.csv` was not run.** `the_flight_replays_bit_exact` and the other C14 test need
  `capture/c14/flight.csv`, which is not on this disk. They call the refactored `compare`, so that
  path is unchecked against C14's flight. The six replays that did run use the same checks.
- **Late rows' pad is assumed.** That a late row's flight block was read before the next pad poll is
  inferred from 1474. The rows before it have the same pad either way.
- **Only part of the read model is pinned.** The order of writes within the rotor's step
  (`FLIGHT_GROUPS`) is taken from the port's `Rotor::step`, and the captures pin only the split at
  1422. The finer orders within the controller's writes are not modelled; no row needed them.
- **The fallback matches can be loose:**
  - a row whose body matches nowhere is taken as late. A true divergence would then fail there, with
    a message about the next frame;
  - a part-way body's accumulators are not compared, and such a body is not held to the read order.
- **The capture was not re-recorded** to see whether the drift repeats. R26 found C21's tear
  deterministic across three runs.
- The captures are in `context/` (s1 and s3) and are not committed. The new test skips without them.
