# 1 — the helicopter's rays on a thrown blade, captured (FINAL)

Task R25. 2026-11-17. C24's follow-up (`research/journal/2026-11-03-c21-kiowa-blades-on-the-hull/`,
file 2): capture a wreck where the helicopter's record holds a kind-0 entry on a thrown blade, or a
blade one on another blade, and check the port against it.

## Result

- **Found one.** R25's `fast6` tape climbs higher than C24's fast1 before diving. It wrecks on 1480.
  On **1484** the helicopter's own ray files a kind-0 entry on **blade 3**.
- **The port matches it bit for bit.** `Collision::search_knocking` and `Blades::file_struck` needed
  no change:
  - the replay is bit-exact to the capture's end (1692);
  - the port's rays meet a blade on 1484 and on no other frame;
  - all five records (the helicopter's and the four blades') equal the captured ones, entry for entry,
    on every row.
- **No blade-on-blade entry** in any of the seven new wrecks, so blade-on-blade is still not
  ported (`Blades::search_on` offers a blade no other blade). The task said to port it only if one
  shows.

## What the original does on 1484

- The helicopter's record holds 9 entries:
  - 4 kind-0 entries on the ground;
  - a kind-0 entry on blade 3 (its other side is blade 3's record);
  - 4 kind-1 mirrors of blade 3's rays on the hull.
- Blade 3's record holds 8 entries: the helicopter's mirror first, then its own 4 ground entries and 3
  entries on the hull.
- The port has the same, in the same order. `search_knocking` names the entry's other side 3.
  `file_struck` puts the mirror ahead of blade 3's own entries.

## Captures

All are R25 captures from C14's state, made with `research/r25/tape.sh`:
- `research/c14/make_tape.py` writes the tape;
- C17's state is copied in as its `_SaveState.p2s` (`context/recordings/c14/flight.p2m2_SaveState.p2s`,
  copied back into the shared context from `kept-context/s4`);
- `research/c19/capture.sh` captures it with the new spec, `watch/r25-kiowa-blades.toml`. It is C19's
  spec plus:
  - each record's address (`heli_record`, `bladeK_record`), so an entry's other side (+0x74) can be
    named;
  - 16 entries per record instead of 4;
- `research/r25/scan.py NAME` lists the rows whose records hold an entry on a blade.

Files: `context/capture/r25/*.csv`, tapes and `tapes.txt` in `context/recordings/r25/`. Copy 1,
NominalScalar 0.05 (about 0.8 rows/s).

| capture | tape (make_tape fields) | wreck | heli kind-0 on a blade | heli mirrors from blades | blade on blade |
|---|---|---|---|---|---|
| fast1 | `60 80:cross 200:-:127,0 200:square:127,0 400` | 1451 | none | b0, b2 | none |
| fast3 | `60 80:cross 200:-:127,0 200:square:255,0 400` | 1441 | none | b1, b2 | none |
| fast4 | `60 80:cross 300:-:127,0 120:square:127,0 400` | no wreck | — | — | — |
| fast5 | `60 80:cross 400:-:127,0 60:square:127,0 400` | no wreck | — | — | — |
| **fast6** | `60 120:cross 200:-:127,0 200:square:127,0 400` | 1480 | **1484, b3** | b3 | none |
| fast7 | `60 80:cross 300:-:127,0 120:square:255,0 400` | 1540 | none | b1, b3 | none |

| high1 | `60 120:cross 200:-:127,0 200:square:0,0 400` | 1481 | none | b0 | none |
| high2 | `60 120:cross 200:-:127,0 200:square:255,0 400` | 1481 | none | b0, b1, b3 | none |
| high3 | `60 180:cross 200:-:127,0 200:square:127,0 400` | 1553 | none | b1 | none |

- high1..3 are fast6's climb (or a longer one) with the dive turned left or right. I recorded them to
  give blade-on-blade one more chance. They are scanned only, not replayed.

- fast1 is C24's tape again. It wrecks on the same frame (1451) and puts back on the same (1602) as
  C24's capture. Its first run was cut off at 1639 by my command's time limit. That still includes the
  put-back, so I kept it.
- fast4 and fast5 fly on: they wreck nothing.

## Tests

In `crates/game-sim/tests/kiowa_capture.rs`:

- `replay_wreck` now keeps every frame's five records (`Replayed::records`).
- `records_read` compares one row's records with the port's, field by field and bit for bit:
  - fields: kind, the ray's start, the contact on the body, the normal, the point, both levers, the
    surface, and the other side (named off the row's record addresses);
  - only x, y, z of each quad are compared (w is padding).
- **A row may be read inside the next frame's collide pass.** The pass clears every record in turn,
  the helicopter's first, then fills each in turn. So such a row holds one of two things:
  - read while clearing: the records before a cut are empty, the rest as the frame left them;
  - read while filling: the records before a cut hold the next frame's entries, the one at the cut a
    prefix of them, the rest empty. The new entries' levers are still zero; the resolve writes them
    later.

  `records_read` accepts exactly those two cases (`Refill`), and the tests pin which rows they are.
  This is the same tear B23 found in try5's contact counts.
- `a_high_fast_wreck_meets_a_blade_bit_exact` (fast6) checks:
  - wreck 1480, put-back 1631, no late or torn rows;
  - the port's struck frames equal the captured kind-0-on-a-blade frames, which are `[1484]`;
  - 1484's helicopter entries 4..9 are `(0, blade 3)` and then four `(1, blade 3)`; blade 3's first
    entry is `(1, heli)`;
  - records bit-exact on all 488 rows; 1480..=1483 were read while filling, at the helicopter's record;
  - no blade's record names another blade.
- `a_fast_wreck_records_bit_exact` (R25's fast1) checks:
  - the same replay, with no blade met;
  - records bit-exact on every row;
  - nine rows read inside the next pass: 1442, 1443 and 1513 while filling; 1523, 1534, 1538..=1540 and
    1553 while clearing.

**Negative controls**, each seen:
- without `file_struck` on the blades, fast6 parts on 1484: speed 41.08868 in the game, 40.349094 in
  the port;
- with the helicopter's mirror filed after blade 3's own entries, fast6 parts on 1484 the same way;
- with the port's 1484 entry naming blade 2 instead of 3, the records check fails on 1484
  ("other side Blade(3) in the game, Blade(2) in the port").

`./kit check -- -p game-sim --test kiowa_capture`: 11 passed. Both new tests ran on their captures.
They skip without them.

## Other captures, not compared

- fast3's bodies replay bit-exact, with many rows read a frame late (as in B24's fast2). Its records
  are not compared: `records_read` has no model for a late row's records, and from 1513 on it differs
  on most rows.
- fast7 parts in flight on 1252, long before its wreck: the collective is 0.62471133 in the game and
  0.64696956 in the port. That looks like B24's flying rows read mid-step, and it is not R25's to
  chase. No test.

opentennis has no helicopter or thrown parts. Nothing was taken from it.

## Not verified / not 1:1

- **Blade-on-blade is not ported and was never captured.** No blade's record names another blade on
  any row of the R25 captures (seven wrecks) or C24's eight. `Blades::search_on` still offers a
  blade no other blade. In the original, the blades' kept lists do hold the other blades (C24 read it
  off RAM), so it can happen; it just never did in these wrecks.
- **Only one kind-0 entry on a blade was captured** (fast6, 1484, blade 3, the helicopter's fifth
  entry). Other cases were never exercised against the original:
  - several blades met in one frame;
  - a blade met when its record is full (`file_struck`'s truncation at capacity);
  - a blade met by the helicopter's sweep rather than its rays.
- **The blades' place among other moving objects in the list** is still unchecked, as in C24. fast6's
  list held no other moving object near the helicopter's rays, so the order could not show.
- **fast3's and fast7's records are not compared** (see above): late rows' records have no model, and
  fast7 parts in flight first.
- **The read-phase model is fitted to the data.** That the pass clears every record first and then fills
  them is read off which partial rows occur, not off the code. A row with every record empty fits
  both phases; the test takes it as clearing when the last blade is unchanged, and as filling
  otherwise.
- The captures are in `context/` and are not committed. The two new tests skip without them, so a green
  gate on another disk does not prove them.

## Next

None.
