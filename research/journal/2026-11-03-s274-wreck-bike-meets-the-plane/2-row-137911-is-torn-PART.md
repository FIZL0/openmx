# 2 — row 137911 is torn (PART)

Task S274, reopened after S276. 2026-11-03.

## What I did

- **Checked the mirror's resolve one term at a time** against 137910's image (debug prints, since
  reverted). Each term matched the original's: the share (`o.collider.mass × own.inv_mass`), the
  spring, and the friction (32.2·mass·μ·min(0.1·slide, 1)). The resulting force agrees with the
  plane body's `last_force` (+0x80) in the image.
- **Found where row 137911 stands.** The port's plane after the first of the tick's two substeps
  equals row 137911 (-24.94349, 8.60006, 2.21980) to every printed digit. The row is read mid-frame
  (memory: capture rows read at three phases). The tick's end, which `held()` compares, is past it.
  - Pinned by `row_137911_is_the_wreck_bikes_first_substep`.
  - `game_sim::flight::substeps` is a thread-local tap that keeps a lone plane's body at each
    substep's end. Play never sets it, and it is not on `Flight`, so the wire is unchanged.
- **Read the dt.** `frame_dt` is at 0x479cd4 and alternates 0x3c888882 / 0x3c88ce6a. All three dts
  tried land the first substep on the row.
- **Rebuilt the tape's pads** (`segment_pad`) from research/c16's segment list, because the tape is
  gone. With them, `fly_two_by` from 137845 wrecks P2 with 7 on tick 137855
  (`p2_wrecks_7_on_the_tapes_pads`).

## Not verified / not 1:1

- **JOUSTS and STRETCHES could not be raised or even re-run.** `context/capture/c16/fly.csv` and
  `context/recordings/c16/fly.p2m2` are in no slot and not in the main checkout. Every
  capture-reading test returns early (it passes by skipping).
- **The first tick after P2's wreck still parts.** With the rebuilt pads, the port's 137856 and 137857
  (vel -28.609364, -0.08478269, 3.6421926 after 137856) match no reading of row 137858 (-28.616682,
  -0.0535129, 3.6458206). That is neither the tick's end nor its first substep. The suspect is the wreck
  bike's start in `Crash::begin` (its body and collision object at the wreck), which has no image to
  check against. The nearest images are 137845 and 137880.
- **137810's standing P2 (17) is not re-checked.** `fly_two_by` already seeds both riders through S228's
  `seeded_rider`, and without the capture the count cannot be read.

## Next (filed)

- Re-record C16's tape and capture (research/c16/index.md).
- Let `held()` take a row read after the first substep, then re-pin JOUSTS and STRETCHES.
- Take an image at P2's wreck (≈137856) and check `Crash::begin`'s wreck-bike seed against it.
