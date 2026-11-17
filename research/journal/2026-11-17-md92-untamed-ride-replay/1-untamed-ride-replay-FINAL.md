# 1 — untamed ride replay (FINAL)

Task MD92. 2026-11-17. Follows research/journal/2026-11-03-md92-untamed-ride-capture (the capture).

## What I did

- Copied MD92's kept captures into the shared context: `capture/md92/wreck-{1,2,3}` (+ .json),
  `recordings/md92/wreck.p2m2` (+ ticks csv, save state) and `untamed/ram/state08.ee.bin`.
- Wrote `crates/game-sim/tests/untamed_bike_ride.rs`: the interpreter runs Untamed's own sim frame off
  state 8, one call per capture row, and compares the bike's body, node, record, both wheels and engine
  word for word. Two tests: `wreck_ride_replays_bit_exact` (wreck-1, 665 rows) and
  `second_run_replays_bit_exact` (wreck-3). Both pass. The one wreck lands on row 340 in both.
- Added `# MD92` to research/oracle.txt (comment rows only).
- Filed MD92a (hook the Untamed ports into this replay) and MD92b (the app ride on untamed_tricks,
  after F31f and MD92a).
- No shipped code changed, so every replay hash with the pack off is unchanged by construction.

## What the original does

- **The frame.** `./kit fn 0x00204ab0` on a scratch KIT_CONFIG (context/untamed + the GAME_REL overlay):
  the +0x24 list's vehicle frames, the world step, the process tail, the +0x30 list. gp comes from the
  ELF's `.reginfo` (0x3fd070); SLUS-21104's `.reginfo` gives its known gp, which confirms the method.
- **What runs outside the frame**, and so has to be supplied each row:
  - the pad reader (writes the bike's input words and the pad record);
  - the put-back spot;
  - the spatial database's visit mark. The game's other searches bump it 3–4 times a tick. A frame
    can bump it several times before it reaches the body, or not stamp the body at all;
  - **the engine's dt.** This was the long hunt. The replay parted on row 342, two rows after the
    wreck, by a few ulps that grew. The bike's hang time grew by 0.0324, 0.0315, 0.0306… instead of
    1/30. Untamed's engine tick (decomp line ~22695, the twin of R3's 0x00131100) multiplies the
    clamped 0.03333 by a slow-motion factor `*(*(0x00462ff8)+0x34)`.
- **Slow motion.** On the wreck, the bike's wreck-first code calls the switch 0x001c3d70 "on". That
  sets the factor's target to 0.18 over 1 s. A process outside the sim frame (0x0022e350) steps the
  factor each tick by `rate × raw ms × 0.001`, clamped. The raw ms is wall clock (33 or 34 on PCSX2).
  The next button down (Cross, the throttle, on tape tick 450) reaches the bike's event handler
  0x004cb960, which calls the switch "off": back to 1.0 over 0.3 s. On wreck-1 the factor shows from
  row 342 to row 458.
- The bike's +0x448 (also +0xce8, +0x1790) adds dt every frame. That is how the rows give dt back.

## How it was proved

- A bit-exact replay fixture: 665 rows × 2 clean captures (wreck-1, wreck-3) of one tape, every word
  of 6 physics blocks per row, through the ride, the wreck, the slow motion and the skip. wreck-2 is
  a torn run and is not used.
- Each outside input was found by the replay failing without it:
  - no visit-mark feed: row 1;
  - fixed dt: row 342;
  - no "off" on the press: row 451;
  - the first fitting raw ms always: row 371 (see below).
- In the interpreter the game's own functions run: the step 0x0022e350, the switch 0x001c3d70 and the
  wreck's "on" call inside the frame. The test supplies only the wall-clock ms and the moment of the
  press.

## Not verified / not 1:1

- **Not on the app.** The brief's "replay on `--handling mxvatv --handling-pack untamed_tricks`" isn't
  done. Like F31's SLUS-21104 ride, the replay runs the original's code in the interpreter, not the
  app's ride. The app doesn't run the MX vs. ATV bike frame yet (F31f). Filed as MD92b, after F31f and
  MD92a.
- **The Untamed ports aren't in this replay.** handling::untamed's functions (MD93a–d) are each proved
  alone on generated states, not hooked into this ride. Filed as MD92a.
- **Fed, not computed:**
  - the pad reader's outputs and the pad record (from the rows; not yet tied to the tape: MD92a);
  - the put-back spot;
  - the visit mark;
  - the raw wall-clock ms of each slow-motion step (picked from 33/34 so the next row's ride clock
    adds up). Where both fit, the next frame decides by retry. Row 370 needs 34 where 33 also fits
    the clock.
- **The skip is called directly.** The test calls the switch "off" on the tape's first button-down
  while wrecked. It doesn't deliver the input event to the bike's handler 0x004cb960. The handler's
  other branch isn't run: the reset (vt +0x36c) when buttons 0x4f and 0x50 both read, and the base
  handler 0x004ba830.
- **The tick's other fields.** The app's ms counter (+0x128) and the dt ring aren't advanced. The
  30-frame mean is assumed to clamp to 0.03333 on every row. That holds while the ride clock matches.
  A run whose mean fell below 0.03333 would show as a mismatch, not pass silently.
- **Non-physics words aren't compared.** Bike-object words outside the six blocks aren't compared. For
  example +0x490 (a heap pointer: the image's heap differs from the live game's), +0x71c/+0x720 (run
  clocks, paused in the replay) and +0x890..+0x99c (a rider-attachment block that starts a frame early
  in the replay at the wreck, rows 340–341). None feeds back into the physics on these rows; all 665
  rows still match.

## Next

- MD92a: hook the Untamed ports into `untamed_bike_ride.rs` one by one, keeping both captures bit-exact.
  Tie the reader words to the tape.
- MD92b, after F31f: the app ride on untamed_tricks against this capture, with the engine's slow motion.
