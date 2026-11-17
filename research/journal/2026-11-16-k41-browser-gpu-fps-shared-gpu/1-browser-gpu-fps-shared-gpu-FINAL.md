# 1 — browser gpu fps shared gpu (FINAL)

Task K41. 2026-11-16.

## What I did

- The brief's "if it reds again" had happened: MD80e's gate (11-05) saw webgpu on a phone at **0.3 fps**,
  passing alone. So I made the test robust instead of leaving the task waiting.
- `crates/openmx/tests/web.rs`, `the_browser_rides_on_the_gpu`: the 30 fps bar is unchanged.
  - Each case's ride is read twice from every GPU client's `/proc/<pid>/fdinfo` (`drm-engine-*` ns per
    `drm-client-id`): before and after. The test prints the busiest other users and their share of the
    wall time, e.g. `localsend 2307202 2%, Hyprland 3318 1%`. A red run now says what else held the GPU.
  - A case under 30 fps rides once more. Only a second slow ride fails, and its message holds both rides'
    JSON and GPU users. A slow ride from a GPU burst (another slot's GPU test, an emulator) is gone in the
    ten seconds or so a second ride takes.
- `gate.toml` `[flaky]`: the test's name. A gate's failure reruns it holding every check slot, so other
  gates' GPU tests are not beside it. Not `[alone]`: that takes whole binaries, and web.rs has 12 tests.
- Took nothing from `../opentennis`: it has no wall-clock GPU fps test in a browser.

## What the original does

Nothing: test tooling only.

## How it was proved

- `./kit check -- -p openmx --test web the_browser_rides_on_the_gpu`: passed alone, twice (before and after
  dropping an unused label).
- The slow path: one run with the bar raised to 3000 fps (put back). Each webgpu ride logged
  `60 fps is under 30; riding once more`, rode again, and failed with `under 30 twice`, both rides' JSON and
  the GPU users of each window (localsend 2%, Hyprland 1%, quickshell, Xwayland, weston 0%).

## Not verified / not 1:1

- **The original red was not reproduced.** No slow ride happened here, so I have not seen the retry turn a
  real red green, nor seen which GPU user caused 19.7 / 0.3 fps. The next red's message names them.
- **The ride's own chromium is not in the GPU-users list.** It has exited by the second reading, so its own
  GPU time is lost. Any client that started and ended inside the window is missed too (a short GPU test of
  another slot). Clients of other users (root) are not readable.
- **The retry does not prove the GPU was the cause.** It reruns on any slow ride. A real regression that
  makes the ride slow still fails, since both rides would be slow.
- **The `[flaky]` rerun holds the check slots only.** PCSX2 copies and agents' hand-run shells still share the
  GPU then.

## Next

none
