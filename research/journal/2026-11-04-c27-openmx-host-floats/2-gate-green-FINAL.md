# 2 — gate green (FINAL)

Task C27. 2026-11-05.

## What I did

Nothing new in the code. Session 1's change (61a6f209: `chase::ride_out`, `PreloadMeter::strength`
and `worlds.rs`' `regions()` wrap period to host f32; everything else stays `ps2_float`, see
`1-openmx-host-floats-PART.md`) is on this branch unchanged. It waited only on P294 (the store test
red on main), which has since landed. This session reran the full gate through `./kit done`.

## How it was proved

- `./kit done`'s full gate (`./kit check --all`), with its replay and tape tests: green.
- Frame time: as measured in part 1 (free09 headless, 600 frames, `OPENMX_THREADS=1`, sum of stages
  without pace): before 2.97 / 2.97 ms, after 3.00 / 2.55 ms. Not measured again; none of the
  converted code runs on an ordinary frame.

## Not verified / not 1:1

- Everything listed in part 1 still holds: `PreloadMeter::strength` can differ by one level at an
  ulp boundary (only its 7 test cases fed, a HUD readout); `chase::ride_out`'s teleport frame differs
  in the last bits from the old PS2 arithmetic (ours, checked only by `markers.rs`' 0.01 ft
  tolerance); frame time measured once on a loaded machine, on one track, not on the web or the Deck.
- No `crates/openmx` file dropped `ps2_float` entirely: by the task's own rule the API-type uses
  and the bit-compared camera/audio/particle code stay.
