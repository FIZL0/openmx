# B23 — try5's parting is a late read, not a ground contact (FINAL)

2026-11-03. Task: `./kit plan B23`, C24's follow-up. C24's try5 capture parted from the port on
1304 (speed 2.388822 in the game, 2.5328536 in the port). Over 1297..1302 the heli's captured ground
contact counts were 1, 0, 2, 3, 2 in the game and 3, 3, 3, 3, 4 in the port. No blade was involved.

## Result

**The port is right. Row 1304 was read a frame late.** Its body is the port's after frame 1305.
With that row compared there, try5 replays bit-exact to the capture's end (1488): wrecked on 1268,
put back on 1419, no blade offered. No product code changed.

- `replay_wreck` (`crates/game-sim/tests/kiowa_capture.rs`): when a row's `body_raw` equals the next
  row's, it was read a frame late. That row is compared after the next frame (`compare_torn` with no
  substeps), the same way R26 compares C21's rows from its put-back on. `Replayed::late` lists such rows.
- New test `a_right_tipped_wreck_read_a_frame_late_replays_bit_exact` (try5). It pins the wreck (1268),
  the put-back (1419), the late rows (`[1304]`) and no blade offered.
- The roll, tip and fast-wreck tests now also assert that they have no late row.

## How it was found

- `body_raw` of try5's row 1304 is byte-identical to row 1305's. Its x moves 0.047 from 1303
  where the neighbouring frames move about 0.025, so the row holds two steps of motion. Every C24
  capture has row 1204 equal to 1205, before the replay starts at row 1. Only try5 has another
  duplicate, and that duplicate is 1304. No capture has a gap in `frame` or `frame_counter`.
- The ground contact counts are a read phase, not a different contact set. I printed the port's
  heli record count (`Collision::entries`) after each frame against the capture's `heli_contacts_n`
  for 1260..1319:
  - try4: equal on all 60 rows;
  - try5: lower in the game on 11 rows: 1275, 1276, 1284, 1293, 1295, 1297..1300, 1302 and 1319.
    The game's count was never higher. On 1293, 1295, 1297, 1299 and 1319 it reads 0 with the touch
    flags clear;
  - the body, which those contacts drive, is bit-exact on every one of those rows, force and moment
    accumulators included.

  The collide pass clears every record and then refills it (C24), so these rows were read while the
  refill was still going. try5's reads sit near the frame boundary around 1275..1319, and one of
  them, 1304, slipped past the whole next frame.

## Checked

- `./kit check -- -p game-sim --test kiowa_capture`: 8 passed. Their summaries show all four
  `replay_wreck` replays ran (roll, tip, right tip, fast wreck), and so did the C21 strike. Two tests
  skipped: they need `capture/c14/flight.csv`, which is not in this slot and has nothing to do with B23.
- Negative control: compared after its own frame, as before, try5 parts on 1304 with C24's numbers
  (2.388822 vs 2.5328536).

## Not verified / not 1:1

- Why PINE read 1304 a whole frame late is not known. As with C21's drift (R26), the recorder is
  the likely cause, not the game. The capture was not re-recorded to see whether the tear repeats.
- The contact entries themselves (kinds, points) are not compared against the port. The counts were
  checked only by eye over 1260..1319 and are not asserted, because a row read mid-refill only bounds
  them. The body being bit-exact is the proof that the contacts match.
- The rule that equal consecutive bodies mean a late read would mis-file a frame on which the body
  really did not move. No wrecked frame in these captures holds still, and a body that did would
  fail the comparison after the next frame instead of passing silently.
- The captures are in `context/` (s1 and s3) and are not committed. The new test skips without them.
