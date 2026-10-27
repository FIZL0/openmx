# 1 — regate (FINAL)

Task S114. 2026-10-27.

## What I did

- No code. S114's work merged on main in 67d20fe9 (b1975d10: the wheel ray probe; `belly_reach`
  is gone, `grep belly_reach crates` finds nothing). The task stayed `[ ]` because main's gate was
  red when it landed. Here it is re-proved on main 56e5bb0b and ticked.

## What the original does

See 2026-10-25-s114-wheel-ray-probe/1-wheel-ray-probe-FINAL.md.

## How it was proved

- `./kit check -- -p game-sim --test replay -- --nocapture dallas_rail` in s3, with
  context/capture/s89 and the s89 states there, so the test runs and does not skip: it passes. The
  port's search at the image's pose finds the image's two wheel contacts 0x8000/0x8001 (fractions
  0.78995/0.79669 against 0.78997/0.79671). The contact lanes part where the 10-25 entry said they
  do (swept_contact 106/108, surface 165/167 along). The locked lanes now agree through 281/277,
  past the 278/276 pins.
- `./kit check --all` with DISPLAY=:0 exported (without it original_mode reds on its software
  renderer, as recorded before). The only red was track_mods `a_one_level_terrain_is_256_samples_a_side`
  ("the tape ran out after 0 frames" inside its 20 s, at load ~40). It passed when run alone. The lobby
  load flakes passed on the gate's own rerun.

## Not verified / not 1:1

- Everything the 10-25 entry lists still stands; nothing new was proved or changed here.
- The full gate did not come out green in one run: track_mods' one-level ride ran 0 frames under
  load ~40 and passed alone. `./kit done`'s own gate run decides.

## Next

- S124 (what parts the along ride at tick 2) and S123/S125 (the body probes) as filed.
