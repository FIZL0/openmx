# 1 — rig inertia already fixed (FINAL)

Task K75. 2026-11-18.

## What I did

- Found the red test (`atv_quad mxu::the_quad_rigged_from_its_files_is_md123as`) already fixed on this base by
  MD245 (5027e5e38, same day): the test now builds the quad at the start spot MD123a's image keeps (where the
  part reader read the model's box, as MD134 found), not at the spot the quad has settled to since.
- MD245 also tightened the check from "under 1e-5 relative" to bit for bit.
- No code change here; this task only proves the fix holds.

## What the original does

The part reader reads the model's box at the spot the quad was made at. The inverse inertia comes from that box,
so reading it at the settled spot parts by a few float steps (K75's 7.9e-5 relative).

## How it was proved

- `./kit check -- -p game-sim --test atv_quad the_quad_rigged_from_its_files_is_md123as`: TESTS PASSED, 1 test.
- A pass can hide a skip, so a temporary panic at the test's end printed both inverse inertias: ours
  `[0.008914575, 0.0084380405, 0.0138061745]`, the image's the same. The test reads the real files and
  image, and they match bit for bit (the assert is on bits). Panic removed afterwards.

## Not verified / not 1:1

none

## Next

Nothing.
