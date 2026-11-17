# 1 — throw draws and skeleton (FINAL)

Task R24. 2026-11-17.

## What I did

- The b15, r14 and s167 captures were gone from the shared context (lost in the 11-13 merge).
  - I re-recorded R14's south-bones on copy 6:
    - recording2's `.p2s` went to slot 8;
    - then `research/r14_windmill_bones.py PID 8 2439 140 4067.5 40 context/capture/r14/south-bones.csv bones`
      ran under `./kit pcsx2 with --`;
    - the ini's NominalScalar was 0.25 so no rows were missed (put back to 1.0 after).
  - The result is 300 rows at frame 2411; R14's was 2403. Both R14 tests stay green on it.
  - B15's south3 needs `midback-2734.p2s`, which is lost too, so it was not re-checked.
- I found two causes of the drift. Both are test seeds; neither is a port bug.
  1. **The riding skeleton.**
     - `b19_ride` leaves `Rider::skeleton` empty, so the riding spring and the wreck's rests use the
       kit's fixed riding pose. The original pulls toward its animated skeleton.
     - On tick 1, before any wreck, bones 1–4, 8 and 9 were ~8 ft/s low in x. That is the brief's
       "x velocity low".
     - Seeding the image's skeleton brings tick 1's worst bone to 0.006 ft. With that alone, t23's
       worst is 2.37 ft (it was 4.45).
  2. **The throw's random draws.**
     - Reason 4 draws the HitBack force and clock from the game's one generator, as
       `base + delta·(2u − 1)`. The test left that generator at the program's start word.
     - The port drew y 250.5 / z −140.9 over 0.255 s.
     - The original drew (−0, 200.15329, −185.22122). That is the after image's body `+0x6d0`. Its
       clock ended at −0.00796, so it ran 0.22535 s.
     - Those are the 16th–18th draws after the before image's generator word (`0x00423b30`).
     - The port makes one draw of its own before the throw, so stepping the image's word 14 times
       hands it the same throw, bit for bit.
- New test: `tests/replay.rs the_windmill_throws_the_bones_with_the_originals_draws`, with helpers
  `r24_body`, `r24_posed` and `r24_ride`. It asserts:
  - the drawn force equals the after image's bits;
  - the worst bone is < 0.05 ft on ticks 1–2, < 0.4 ft to tick 11, and < 0.9 ft to tick 23;
  - the mean x velocity is within 0.3 ft/s.
- Nothing came from opentennis.

## What the original does

- The body actor keeps 22 joint records at `+0x234` (stride 0x60: `+0x40` translation, `+0x50`
  quaternion, `+0x34` node).
  - The parents are the render skeleton's: [–,0,0,0,1,4,4,4,6,8,9,10,7,12,13,14,2,16,17,3,19,20].
  - The bike node (`rider + 0x18`, local rows at `+0x40`) is the ride transform to 1e-4.
- The throw's five draws are in the oracle note on the wreck entry (`./kit fn 0x0024ad90`). After the
  wreck, `+0x6d0` keeps the drawn force and `+0x6bc` keeps what is left of the clock.

## How it was proved

- A save-state capture: `context/capture/r14/south-bones.csv` plus its before and after images.
- The port's drawn force equals the after image's `+0x6d0` bit for bit. The test asserts it.
- The worst bone off the csv (ft):

  | tick | neither seed | skeleton | both |
  |---|---|---|---|
  | 1 | — | 0.006 | 0.006 |
  | 11 | 2.26 | 0.93 | 0.30 |
  | 23 | 4.45 | 2.37 | 0.71 |

- A search over 0–60 draw steps put 14 second best at tick 12 (0.188 ft). 56 was best (0.171), but
  it draws a different force, so it was a fit to other error.

## Not verified / not 1:1

- **~0.7 ft/s low in y remains.** It builds while the throw pulls (ticks 3–8), then holds. The cause
  was not found; the pull's frame (the wreck bike's turn) is the first suspect. Filed as a follow-up.
- **B15's south3 was not re-checked.** Its capture and its `.p2s` are both lost.
- **R14 was re-recorded at frame 2411, not 2403.** It is the same spot and the same throw (reason 4),
  but not the same capture R14 and B22 measured.
- **The skeleton is held at the image's pose.** No shell runs here, so it never animates over the 23
  ticks. The original's skeleton moves; how much that matters after the entry is unmeasured.
- **The "one port draw before the throw" was inferred**, not traced. Stepping 15 gave the force one
  draw later and 14 gave it exactly, so the port draws once between the seed and the throw. Which call
  makes that draw was not checked.
- **The contacts after touch-down (~tick 12) were not looked at.** No bone touches in this capture's
  first 24 ticks, so the brief's contact part is unproven here.
- **`b19_ride` and the older tests were not changed.** They still use the fixed pose and the unseeded
  generator.

## Next

- Follow-ups: R40 (the y residual) and C37 (re-record B15 south3).
