# 1 — rotor probe skips (FINAL)

Task F33. 2026-11-17.

## What I did

- Nothing to change: P220 (0dfe26814, 11-04) already swapped `ctx(STATE).unwrap()` in
  `the_rotors_probe_is_the_images` for `let Some(ram) = ctx(STATE) else { return };`, the same skip
  the file's other tests use. `ctx` prints the skip line and returns `None` when the image is missing.
- Ran it: `./kit check --package=game-sim --test=kiowa_capture the_rotors_probe` → 1 test passed.
- Kit note: `./kit check -p game-sim …` reorders args and breaks `-p`; use `--package=game-sim
  --test=NAME FILTER` (no `-- --nocapture`).

## What the original does

Not applicable (test harness only).

## How it was proved

The test ran and passed in slot s3, whose shared context now has `states/c14-kiowa.ee.bin`.

## Not verified / not 1:1

- The skip path itself was not run: this slot has the image (the shared context since 11-13), and
  moving the shared file away to test it would break other slots. The code is the same
  `let … else { return }` pattern as the file's other tests, which do skip.

## Next

Nothing.
