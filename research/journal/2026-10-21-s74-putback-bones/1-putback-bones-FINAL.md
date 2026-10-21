# 1 — putback bones (FINAL)

Task S74. 2026-10-21.

## What I did

- Found the human rider's body actor and its skeleton in RAM: `capture/bones_common.py` and
  `capture/probe_body.py`, run on states 6 (free06) and 7 (free09).
- Recorded the body once a tick through S66's five pad scripts with `capture/bones_watch.py`, unchanged
  (free06 ride/down/downchord, free09 ride/down). The captures are in `context/s74/` and are git-ignored.
  Each one has torn ≤ 1 and gaps 0, and together they hold 9 put-backs: 7 by the chord, 2 by the wreck clock.
  Every tick carries:
  - the bike's reason and ghost clock;
  - the request block the body reads;
  - the body's mode, machine and clock fields;
  - all 22 joints and every joint's node (local and world rows).

  Reader: `capture/bones_read.py <short name> [lo hi]`.
- Found what the port had wrong. It never held the body on a put-back: `Rider::body_frame` wrote
  `body_request.held = false` on every tick. On a clock put-back it also dropped the body out of the wreck
  and the ragdoll out of the picture on the put-back tick itself. The original does neither.
- Ported the original's put-back pair as `rider::PutBack` (crates/game-sim/src/rider.rs):
  - `place()` is called at the chord's put-back (step 1b) and at the clock's (the `Reset::Now` arm).
  - `fill(wreck)` runs right after the physics (step 2), before the machine's verdict.
  - `body_frame` reads `held` into the request.
  - The body's `ask.wreck` is `state.wreck.is_some() || put_back.wreck`, so on the clock's tick the body is
    still down.
- Added `Rider::ragdoll_shown`, a presentation-only boxed `Down`. On the clock's tick it holds the bones
  as they were, and the next frame drops it.
- Added `Rider::drawn_ragdoll()`. The shell's two draw sites (`Shown::of`, the software skin) use it, so
  the body is drawn thrown for that one tick, as the original draws it.
- Added `Rider::drawn_jumps()`, which replaces the bare `ghost.put_back()` in the shell's tick-blend cut.
  It also cuts on the body's first held tick, the tick after a clock put-back, when the drawn body goes
  from the ragdoll (58 ft away in free06-down) back onto the bike.
- Updated V19's `a_wrecked_body_settles_and_then_holds_still`. It asserted that the put-back clears the
  settle, and the original keeps it (+0x6b4 stays 1). The test now asserts the measured sequence:
  - Wrecked on the clock's tick;
  - Held for two ticks, at state 30 with ramp 0 and axes 0.5/0.5;
  - then not held, with the settle kept.
- Size pins:
  - `Sim` is 9808 bytes, up from 9792 (tests/hash.rs, commented).
  - The riders' resync bytes are 13131, up from 13115 (`RIDERS_WIRE_*` in tests/platforms.rs): `PutBack`
    is on the wire so a rollback restores the hold.
  - The walk is unchanged. `PutBack` is read only by the body, which is not walked either, so no replay
    hash and no leaf count moved.
- Follow-ups: S90 (the riding body from the ragdoll's spring) and S91 (the body's one-tick lag at a
  wreck's entry).

## What the original does

- The player update `./kit fn 0x0027c890` runs, in this order:
  1. the base update (0x002520d0);
  2. a delayed counter at +0x1480, which raises +0x3a0 when it runs down;
  3. the request fill `./kit fn 0x0027c2d0`, where `+0xb8c = +0x3a0` is the body's held flag;
  4. at the end, `if (+0x3a0 && +0x3a4) +0x3a0 = 0; +0x3a4 = +0x3a0;`.
- +0x3a0 is raised by the ghost chain 0x0027b650 (both put-backs) and by 0x0027bbe0.
- The result is that the body is held on exactly two fills after the flag goes up.
- The chord is taken in the input phase, before the update, so it is held on T and T+1.
- The clock's put-back lands after that tick's fill, so it is held on T+1 and T+2.
- The request's wreck reason is likewise the bike's +0x748 from before the tick's verdict. On a clock
  put-back's tick the request still says 11 and held 0, so the body stays in its wreck mode (+0x47c = 2)
  for that tick. Its bones are drawn where the ragdoll is, while the bike already stands on the put-back
  record.
- Held, the body's mode is 0. The selection runs the riding row, state 30, with the ramp zeroed and both
  lags snapped to the request's 0.5/0.5. The nodes are the animation pose directly: node 0's local
  translation equals joint 0's.
- From the first tick after the hold, the body is mode 1. The nodes then part from the joints, because the
  riding body is the ragdoll, sprung toward the animation pose.
- Leaving the wreck for Held runs no clean-up arm, so the settle flag survives the put-back.
- Measured on every tick of five captures. The fill sitting "before the verdict" is a measured order,
  not a traced one. an earlier oracle.txt note (the request-pointer block) puts the ride machine 0x0027bc90 at the end of the base
  update, which runs before 0x0027c2d0, so the writes of +0x748 that the request misses must come from
  somewhere else. That path was not traced (see below).

## How it was proved

- **A save-state capture, compared tick by tick.**
  `rider_anim.rs::the_put_back_holds_the_body_as_the_original_does` drives `PutBack` off each capture's
  own put-backs:
  - the chord when the ghost starts below 3.0, and the clock when it starts at exactly 3.0;
  - the wreck before the verdict being the previous tick's reason.

  On all 2695 ticks it gives the original request's held flag and wreck reason exactly. On each of the
  9 put-backs it also checks that:
  - the body's mode is 2 on the clock's tick;
  - the mode is 0 on the two held ticks, and not 0 on the third;
  - the body is drawn within 6 ft of the bike on its held ticks.
- **The original's joints, compared with the port's pose.**
  `motion.rs::the_put_back_holds_the_original_pose_joint_by_joint` covers all 18 held ticks across the
  five captures. On each one, the port's `vehicle_anim::Driver` is:
  - seeded from the tick before (V12's way);
  - handed the original's request;
  - stepped with `captured_step`.

  It latches state 30, the original's ramp and both axes bit for bit. Its blend, drawn by
  `Set::apply_vehicle_crossed`, gives all 22 joints' quaternions and translations within 1e-4 (worst
  6.6e-7, joint 6, free06-downchord 3017).
- **The port's own rider, end to end, on the fixture.** V19's `a_wrecked_body_settles_and_then_holds_still`
  wrecks a whole `Rider` and lets the clock put it back:
  - Wrecked on the put-back tick, then Held, Held, then not held;
  - on the held ticks the body is at state 30, ramp 0, axes 0.5/0.5;
  - `drawn_jumps` cuts on the clock's tick and on the first held tick;
  - the settle is kept.
- `./kit check`: TESTS PASSED (full gate, `--no-fail-fast`).

## Not verified / not 1:1

- **The riding body from the third tick after a put-back on is not matched.** The task asks for the
  drawn rider frame by frame through the 3 s ghost, and the port only matches the ticks the body is held.
  In the original the riding body (mode 1) is the ragdoll pulled toward the animation pose by its riding
  spring. The port draws the clip pose plus the renderer's hand IK, and `ragdoll_body::Actor`'s riding
  path is ported but proved for nothing and not stepped for drawing. → S76.
- **The node world rows are not compared.** The held-tick proof is on the 22 joints, the local pose the
  nodes are built from. The world rows add the bike's node and the renderer's f32 hierarchy and IK, and
  rendering is match-by-eye.
- **The thrown body on a clock put-back's tick is not compared bone by bone.** The port draws the
  pre-reset `Down` as it stood after that tick's step. The captures show the original drawing its
  ragdoll there, about 58 ft from the bike in free06-down, but no port ragdoll was replayed against them:
  there is no full-ride replay of these captures. The port also skips the body's wreck frame on that tick
  (no sensors left), where the original runs one.
- **The wreck's entry is one tick early in the body.** The original's request lags the entry by the same
  fill, so the body enters its wreck mode a tick later than the port's. It is counted in the test, not
  changed, because moving it moves draws from the shared generator. → S77.
- **The delayed counter +0x1480** (a third writer of +0x3a0) and 0x0027bbe0's write are not ported. No
  capture reached either.
- **Where +0x748 and the reset are written relative to the fill** was measured on five captures, not
  traced in the decompilation. The order recorded in oracle.txt (machine inside the base update)
  contradicts the measurement, and the contradiction is not resolved.
- **Which state the port's selection picks while held** is checked only on the fixture rider (V19: state
  30). The joint test seeds `want` from the capture, so it proves the step and the pose but not the
  selection on the disc's own body rates. Those rates have no known loader (`Rider::body_rates`).
- **The shell was not looked at.** The one-tick thrown draw on a clock put-back and the second blend cut
  were not seen on screen. Both are covered only by the sim-side tests above.
- `PutBack` is on the wire but not in the hash walk, the same as the body it feeds. A desync in it alone
  would not show in a checksum.

## Next

Nothing for S74. S76 and S77 hold the open parts.
