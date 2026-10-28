# 1 — 2p wrecks whole frame (FINAL)

Task S176. 2026-10-28.

## What I did

- `the_2p_wrecks_are_the_originals` (crates/game-sim/tests/ragdoll_wrecks.rs) now runs the original's
  **whole frame** from each 2p RAM image, not S155's bike-only vehicle update. The frame is the
  engine tick's own steps from tick.rs's harness: the device update, listener marker 1, phase 2, dt
  and 1/dt, the frame counter, the scene root's simulate slot (vt +8 → +0x38), phase 0, then marker 2.
  It stops before the render. `TWO_P_ON_THE_CAPTURE` 22 → **46**: all 17 wrecks and 29 near misses
  run on the capture's own line.
- New in the test file, all S176-local (S155's `judge_from`/`image_before` stay, S160 still uses them):
  `whole_frame`, `judge_whole`, `WholeRun` (`held_to`, `frames_of`, `shows`), `Shows`,
  `image_ending_a_frame`, `rider_state`, `rider_attitude`, `TwoP::pad`/`state`/`attitude`,
  `Clause::of_attitude`, `TWO_P_ATTITUDE_LAGS`. Eight pad and stamp columns and four euler columns
  were added to `TWO_P_COLS`.
- What it takes to run a frame with no hardware (all in `whole_frame`, hooks removed after each frame):
  - The interrupt off/on pair (`mfc0`) is stubbed (off answers 1, "were enabled").
  - libpad's per-frame command send calls three things, all stubbed: `sceSifDmaStat` answers −1
    (done), `sceSifSetDma` answers 1, and the D-cache range write-back is a no-op.
  - The two `lui $r, 0x1000` that read Timer 0 (a profiling read nothing keeps, and the listener
    dispatch's ms clock) are patched to `lui $r, 0`, so they read RAM word 0. A stand-in clock sits
    there: `wall_ticks`·100 ms from the image, plus Σdt·1000.
  - The pads are laid into libpad's buffers before each frame from the capture's pad and stamp
    columns.
- The S155 RAM images (context/states/s155, 51 files) were copied from s3's context into this slot's
  (git-ignored, not committed).

## What the original does

- **Images end frames.** Each image's stored dt (app +0x344) is its own row's dt, bit for bit. The
  frame after an image, f, steps on row f's dt (`whole_frame` asserts the counter it reaches is f).
- **Pads: row f−1's for frame f.** Row f's highest pad stamp is fc + 8827 on every row but 31 (8826
  there). So the pads a row shows were stamped before the frame that row counts. Laying row f−1's
  pad halves before frame f holds every run. Laying row f's parted several.
- **The recorder reads a row at one of three moments** (`Shows`):
  - **After**: after frame f's simulation.
  - **Before**: between frame f's counter step and its simulation, so the row shows frame f−1.
  - **Torn**: inside frame f's simulation.

  In the held runs: 77 rows After, 78 Before, 31 Both (a bike standing still matches either), and
  9 Torn.
- **Torn rows are exact.** `chassis_integrate` (`./kit dis` at the oracle's `chassis_integrate`)
  begins by rebuilding the centre +0x60 from the transform (+0x254 applied to the local cofg +0x190,
  one quad store). That rebuilt centre differs from the frame before's by up to a few thousandths. The
  integrate later writes the final centre one axis at a time.
  - Rows 131871 and 139658 (P1) are the rebuilt centre bit for bit.
  - Row 131872 has the final x and y with the rebuilt z.

  The test catches the rebuilt centre on integrate's call to `mat4_to_quat` (return address
  integrate + 0x50, s0 the chassis). A torn row must have every axis bit-equal to the frame's
  before, rebuilt or after value, never merely "between" (S155's looser rule).
- **The attitude lags on some rows.** `chassis_measure` writes the euler (+0x90) once a frame, after
  the physics, in one 128-bit store. A row read inside the frame, between the move and the measure,
  holds the frame's centre and the frame before's pitch and roll. C10's index read two near misses
  off such rows:
  - P1 131628 GroundPitchUp: the index's 48.143° is frame 131627's pitch. The wreck test judged
    48.044.
  - P1 135076 TouchRoll: the index's 50.019 is frame 135075's |roll|. The wreck test judged 52.482.

  Both are checked bit for bit against the run, and the list is pinned (`TWO_P_ATTITUDE_LAGS`).
- **The whole frame enters the two Scenery wrecks.** P2's Scenery wrecks (130418, 130897), which
  S155's bike-only runs never entered, are entered on their rows with reason 6. The port's
  `hit_wreck` agrees on every call.
- **Choosing the image.** An image saved partway through the rider's update is not used: its centre
  and reason must be its counter's row's, or the next row's. It must also end a frame at least two
  before the event, because the event row may show the frame before its own, and the run has to see
  that frame's wreck test.

## How it was proved

- `the_2p_wrecks_are_the_originals`: 46 runs over C10's index (17 wrecks, 29 near misses), each
  from the latest qualifying image in context/states/{c10,s155}, 2–8 frames before the event, run to
  a frame past it. Results:
  - Port == original on every wreck-test call of every run.
  - Every run holds the capture from its image through the row after the event, and every row is
    whole or exactly torn (above).
  - Every wreck is the original's first, in a frame its row shows, with the indexed reason.
  - Every near miss rides out with no wreck, and the clause's quantity matches the index within
    0.0015. For the two lagging rows, the frame before's attitude matches bit for bit.
- Mutations, each red, then put back:
  - Floor at 47: "only 46 of the 2p events ran on the capture's own line (was 47)".
  - The port's landing clauses off: "P1 128995: frame 128994: the port's wreck test answers [], the
    original's [8]".
  - The rebuilt centre dropped from the torn rule: "only 44 … (was 46)" (P1 131872 and 139659 leave
    the line).
- `./kit check -- -p game-sim --test ragdoll_wrecks`: 15 pass.

## Not verified / not 1:1

- **The listener clock is a stand-in.** Timer 0 and the sub-tick ms clock are
  `wall_ticks`·100 + Σdt·1000, not the console's. The listener dispatch stamps stick samples with it
  (flick speed). All 46 runs hold the capture regardless, but a rider input that depends on stamp
  spacing is not proven to be stamped as the console stamped it.
- **No IOP.** libpad's SIF DMA and the D-cache write-back are stubbed. The pads come from the
  capture's columns (row f−1's, by the stamp evidence above), not from the IOP's transfer, so the
  stamps' timing within a frame is not reproduced.
- **The frame stops before the render.** Whatever the render or the DMA wait does to game state is
  not run. Nothing the test reads showed a difference: the Before rows, read after the render,
  match the run.
- **Torn rows are checked by their centre only.** For a torn row, the reason must be the frame
  before's or after's. Only the two near misses' attitudes are checked against the frame before's.
  Other columns of torn rows (g, contacts) are not.
- **The index's two lagging near misses** (131628, 135076) are values the wreck test never judged.
  The test proves where they come from but does not change C10's index.
- **The port's own ride is not run from the 2p images.** Only its wreck decision is, call by call,
  on the original's inputs (S175).
- **The joust branch (reason 7)** has nothing to be checked against: the 2p ride has no
  rider-vs-rider contact (C10).
- **The images are local.** The S155 images live in s1's and s3's context only. Other slots skip the
  test with an "S176:" line (research/s155/rams.py rebuilds them).

## Next

S175 (the port's own ride from the 2p images).
