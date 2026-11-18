# 1 — south3 bones recaptured (FINAL)

Task C37. 2026-11-18.

## What I did

- Looked for the lost files in the shared context, `old-context-s1..3` and `kept-context`. All gone:
  `capture/b15`, `capture/s144` (midback-2734) and `capture/s167` (south3.csv, south3-before.ram).
- Made a new mid-air state the way S144's `tail.sh` made midback (new `research/c37/south3.sh`):
  - recording2's `.p2s` into slot 3 (`research/c37/put_state.py`);
  - `research/s76_tele.py` to the farmyard (slot 4). No pad here (uinput is root-only), so its brake
    hold did nothing; the bike settled on the flat yard anyway;
  - `research/s144_tail.py 4 50 -28 8`: thrown up 50 ft/s with a −28 pitch rate (slot 8);
  - `research/c37/save_at.py PID 8 2487 9`: run to frame 2487 and save. There the bike is 41.3° nose-up
    and rising 29.1 ft/s. B15's image was ~39° and ~30 ft/s.
- Captured with `research/r14_windmill_bones.py PID 9 2439 140 4067.5 30 context/capture/b15/south3-bones.csv
  bones ride=0`, NominalScalar 0.25 (put back to 1.0 after).
  - 226 rows from frame 2487, no gaps, 2 rows marked not-same.
  - The state is kept as `context/capture/b15/south3-2487.p2s`.
- **Fixed `ride=0` in `r14_windmill_bones.py`.** This PCSX2 ignores a PINE load while paused. The old
  path paused, then loaded, so it teleported whatever frame was showing. Checked: a paused load left the
  frame and the bike unchanged; a running load lands within 0.2 s and does not pause. Now it loads running
  and presses pause the moment the frame counter goes back. It stopped on the loaded frame (2487) exactly.
- Test `the_windmill_strikes_the_ports_bones_left_at_its_face`:
  - Dropped the "rows 2–13 equal S167's south3.csv" check: that csv is lost, and this is a new state.
  - The throw row is now read from the csv (row 3 here, B15's was row 4). All tick numbers are relative to it.
  - Bounds are re-pinned to this capture's numbers.
  - Added R24's seeded ride (`r24_ride`) on it.
- Took nothing from opentennis: it has no windmill or ragdoll.

## What the original does

- The fall, then the windmill's throw on row 3: (7.46, 18.80, 133.40) ft/s, reason 4. S167's was
  (7.79, 18.61, 133.41) on row 4. The blades turn, so the contact's tick depends on the frame.
- On the throw's row the actor is still riding (1) and the main bone goes at (0.8, 0.8, 0.8) of the bike.
  On the next row it lets go.
- Bones at most 173.9 ft/s over 226 rows. No reason 13. Bones touch on row 5.
- The image's bones keep the jump's climb (main bone vy 30.8) while the teleport zeroes the bike.
  R14's capture has bones at rest (vy −0.56).
- The throw's drawn force (after image, actor +0x6d0): (−0, 227.88008, −143.03474).

## How it was proved

A save-state capture (`context/capture/b15/south3-bones.csv` with before and after images), replayed by
the port in `tests/replay.rs`.

- **Bones left at the face (no riding body):**
  - Port ticks 1–2 are bit-exact against rows 2–3, with the wreck on tick 2.
  - The bones are 9.3 ft off on tick 3, struck on tick 4 (touched, 113,868 ft/s), and put back on tick 5.
  - With the windmill counted as stepped: at most 20.3 ft/s through tick 8.
  - B15's finding holds; only the ticks moved.
- **B19 seeded riding body:**
  - The port's down/riding matches the original's word on every tick of 40.
  - No bone over 200 ft/s, no put-back.
  - Worst bone: 0.36 / 0.83 ft riding, 1.13 on the entry, 2.81 on tick 11.
  - The entry's main bone is 6.47 ft/s off (B15's capture: under 2.5).
- **R24 seeds (skeleton + generator):**
  - A search over 0–119 steps found exactly one, 56, that draws the original's force bit for bit.
    R14's capture takes 14.
  - Tick 1 is 0.079 ft off.
  - The throw's tick is 0.94 ft off. The mean bone velocity gap is (−1.31, −2.03, −0.61) on the throw's
    tick and (−0.45, −2.80, +4.52) on the entry, then holds near (−0.5, −3.7, +3.9).
  - Worst bone is 3.25 ft on tick 11 and 6.07 on tick 23.
- `./kit check -- -p game-sim --test replay windmill`: 4 passed. The values above were printed from the
  same test run with a temporary failing assert, which is now removed.

## Not verified / not 1:1

- **Not B15's capture.** The state is new (frame 2487, 41.3° and 29.1 ft/s against midback's 2734, ~39°
  and ~30). S144 did not record midback's exact throw numbers, and its pause timing was by hand, so no
  bit-exact remake was possible. B15's and B22's numbers (0.87 / 1.61 ft, the 2.5 ft/s entry) were not
  re-measured on its image; this capture's are in the test instead.
- **S167's south3.csv check is gone.** Both that csv and south3-before.ram are lost. S211's south3 test
  still skips without them; not remade here (not this task's files).
- **R24's seeds leave a ~5 ft/s gap from the throw's tick (S466).** Its cause was not looked for. The
  rising bones under a stopped bike are the suspect. The test pins the gap, so a fix makes it fail.
- **Why 56 draws here and 14 on R14's capture was not traced.** The force matches bit for bit, but
  which calls make the extra 42 draws was not checked.
- **The test's bounds are this capture's measurements plus a margin**, not a claim of a match: up to 3 ft
  to tick 11 and 7 ft/s on the entry's main bone.
- **s76_tele's brake hold did not run** (no pad). The farmyard is flat and the bike settled to 0.00 ft/s
  before the throw, so the throw starts from rest as S144's did.

## Next

Nothing for C37. S466 follows the R24 gap.
