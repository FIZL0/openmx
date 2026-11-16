# 1 — c33 human saved stick

Task S329. 2026-11-16.

## What I did

- **Found what the human lacked: the stick its image was saved holding.** C33's image holds the
  human rider's limiter stick at `+0x62c` = -1 (fully forward) and its fore-aft lean at `+0x674` =
  -0.1155, already walking towards it. The three AI riders hold 0 there. The replay fed the human
  `Controls::default()`, so its centre of gravity never moved forward.
- **Only the first step rides on it.** This is the same as S117's finding: the original's first tick
  after a load rides on the controls the state was saved with, and the pad (none here) from the next.
  - Stick -1 on every step: tick 4 matches, tick 5 parts (the original's pitch spin falls back to
    0.272, the port's rises to 0.785).
  - Stick -1 on the first two steps: parts at tick 5 the same way.
  - Stick -1 on the first step only: the human agrees on **every agreed row through tick 1802**,
    including its run downhill from about tick 790.
- **The clutch lever stays out (0).** The image's launch ramp is running (clutch timer 0.033,
  cofg weight 1). With the lever held in (1), the ramp stops and the centre jumps 1.4 ft forward at
  tick 4.
- `crates/game-sim/tests/ai/race4.rs`:
  - New `c33_saved_controls` reads the human's stick from its image. It asserts each axis is at rest or
    at full throw (where the squared copy reads back as itself) and that the brake and gas levers
    are let go. The human gets it on the first step, then `Controls::default()`.
  - `C33_AGREES_THROUGH[2]` goes from 0 to 1802.
  - **A against P.** The human's tuning now shows in the capture: on A it parts at tick 6 (spin off in
    the fifth digit), on P it rides through. `C33_HUMAN_TUNING` stays P, and its comment says so.
- Nothing taken from opentennis: this is a test seeding fix with no counterpart there.

## How it was proved

- `./kit check -- -p game-sim --test ai -- s12e_free09s_four` passes. The run prints "the human
  agrees on every agreed row, through tick 1802 (torn rows [])". The AI lanes and the generator keep
  their marks (14, 18, 16, 6).

## Not verified / not 1:1

- **Why the image holds the stick at -1 is not traced.** C33's boot (research/c33/boot.py) drives no
  pad. Whatever wrote -1 before the save (a no-pad read during the load, or the boot's pad state) is
  not known, and neither is why the second tick reads 0. The replay goes by S117's rule and the
  1800-tick match. It has not been followed in the original's pad read.
- **`c33_saved_controls` reads back only 0 or ±1.** The limiter stores the stick squared, so any
  other value would need its square root. C33's image needs only these values, and the function
  asserts that rather than guess.
