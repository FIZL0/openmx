# 1 — pieces degenerate preload check (FINAL)

Task MD132. 2026-11-13.

## What I did

- Checked and closed. No code change. The low counts MD132 reported (17, 18 and 19 against a floor of 20)
  came from 3,000-frame runs. B36 (344ca6cd9) and F31b5l (078736f3d) have since moved
  `all_six_pieces_with_the_tire_step_are_mx_unleasheds` and `all_eight_pieces_are_mx_unleasheds` to
  `pieces_over(6000, …)`, which raises the case count, one of the two fixes the brief allowed. The
  generator's degenerate share (`max_preload_time = min_preload_time` at chance 0.4) was left as it is.
- Ran the 16 pieces tests (`./kit check -pgame-sim --test=mxvatv_car_frame pieces`) in slot s5 at
  067772dbe. All passed. For one run I added a temporary print of every tag seen under 40 times, then
  restored the file with cp + touch.
  - Six-piece (0x0124_a002, 6,000 frames) and eight-piece (0x0127_0013, 6,000 frames): no tag under 40,
    so "preload: fired, degenerate" is at 40 or more in both.
  - Under 40 elsewhere:
    - `preload_is_mx_unleasheds` (0x0106_a003, 3,000 frames): "preload: fired" 24, degenerate 26.
    - `all_five_pieces_are_mx_unleasheds` (0x0106_a006, 3,000 frames): "preload: fired, degenerate" 25,
      "preload: fired" 31, "launch: starts, degenerate" 32.
    - `launch_is_mx_unleasheds`: "launch: starts, degenerate" 34.
    - The 6,000-frame 0x0126_0002 run: degenerate preload 35.

## What the original does

Nothing new. The test compares the frame's own pieces against the port. This task only touches how often
the generator reaches a branch.

## How it was proved

The existing function oracle in `crates/game-sim/tests/mxvatv_car_frame/pieces.rs`, at 3,000 or 6,000
generated frames per test as listed above. One run in slot s5.

## Not verified / not 1:1

- Ran once, in slot s5 only, not three times in s2 as MD132's report did. The seeds are fixed and the test
  is deterministic, so a second run would give the same counts, but no other slot was checked.
- The 3,000-frame runs `preload_is_mx_unleasheds` (24) and `all_five_pieces_are_mx_unleasheds` (25) clear
  the floor of 20 by only 4 and 5. A later change to the generator's random stream could push them under,
  the same way MD126 did to the six-piece run. No follow-up filed: they pass, and if they go red the fix
  is the same one used here (6,000 frames).

## Next

Nothing.
