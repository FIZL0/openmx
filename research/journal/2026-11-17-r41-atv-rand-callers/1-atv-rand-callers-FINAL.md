# 1 — atv rand callers

Task R41. 2026-11-17.

## What I did

- Found the rand() lag of the three rides by R37's method: invert the captured spin at the wreck's first
  row through the quaternion the frame turned it by, then search the LCG's stream for the three draws.
  - race-jump: 22 draws before row 66 (kind 1 wreck).
  - race-wheelie-launch: 50 before row 80 (kind 1).
  - s-wheelie-launch: 53 before row 82 (kind 3), plus 1 before row 132 (the idle variant).
  - Each matches its three draws to about 1e-6. Nothing else in 2 million draws comes close.
- For race-jump, the inverting quaternion had to be the one from before the frame. The row's main body
  quaternion is the frame's end, which is not what the spin was turned by. I rebuilt the earlier one from
  the frame's write log, up to the rotate.
- Added the rows to `RNG_LAG` in tests/atv_quad/replay.rs (commit 584f37ce4). The ragdoll check now
  needs every race- ride to have no ragdoll diffs.
- **Found where the extra draws come from.** I patched a copy of each C28 state so rand() logs its
  callers in a ring (recipe in research/oracle.txt, R41), recorded race-jump and race-wheelie-launch with
  their tapes, and compared each row's callers with the replay's own.
  - The extras are all sound and effects code that runs between sim frames: the engine note, the
    no-repeat crash/landing sound pick, a random pick from a list, the trail's sound object (two draws a
    frame while it plays), and the wreck clank (after the spin's draws).
  - After the wreck, the trail's linked effects items step their dust emitters, two draws per puff. The
    replay calls the same emitter inside the frame on other rows, but from another caller.
  - Before the wreck these sum to **exactly 22** (race-jump) and **exactly 50** (race-wheelie-launch).
    So the fitted lags are the real counts, not a coincidence.
- Updated the `RNG_LAG` doc comment with the sources (no addresses). Filed R44 to serve these callers
  instead of fitting counts.

## How it was proved

- `./kit check` on `replays_by` (atv_quad): all 15 rides pass by the original's frame. Every race- ride
  has empty ragdoll diffs.
- A race-jump re-record with an `rng` field: the original's state is exactly 22 draws ahead of the
  replay's before row 66's frame. That re-record equals C28's csv on all 300 rows.
- The rand-log re-records: race-wheelie-launch equals C28's csv on all rows. race-jump equals it to row
  259 on the second run and on all rows on the first.

## Not verified / not 1:1

- **s-wheelie-launch's +1 at row 132 is a fit.** Only extras 1, 4, 5 and 6 give the variant that
  matches; I took the smallest, and only 1 makes the later rows match. Its callers were not logged: the
  s- re-records are not byte-identical (C32), so a log of them would not be the captured ride.
- **The replay does not follow the original's lag after a wreck.** The dust draws keep the gap growing
  (134 by row 138 on race-jump). The replay's own later draws (idle variant, lerp) match only because
  they are not visible in the captured columns, or by chance. R44.
- **s-wheelie-launch row 145 and s-jump row 49 still differ.** They are phase flips: that frame's copy is
  in no row.
- **Caller names are partial.** The engine note and trail sound object are named from their place in
  the code and vtables. I did not read every caller through.
- **The patched state's trampoline at 0xf0000 was assumed unused.** The re-records matching C28 support
  this, but race-jump's second log run differs from row 260 on (the first run did not). I did not chase
  it; the rows that matter end at 138.
- Only race-jump and race-wheelie-launch were rand-logged.
