# 1 — web pad tests wait on the page (FINAL)

Task K34. 2026-11-04.

## What I did

- MD97's gate log was overwritten by later checks (no slot's or merge's `context/notes/check.log` still has
  it), so I reproduced the flake: the `web` test binary run beside 48 busy-loop processes (load ~100) failed
  `a_firefox_pad_rides` with `first: false` (and `braked` 0.069, over its 0.02 limit). With nothing else
  running, all 12 tests pass.
- The race was the driver's wall clock against the page's frame rate, in `tests/web/firefox_gamepad.mjs`, and the
  same code is in its chromium twin `tests/web/gamepad.mjs` (`a_pad_the_browser_shows_on_a_press_rides`):
  - the first Cross was a 100 ms tap and the `pads: 1P: … the menus` line had to show within 3 s. The worker
    reads the pads once per frame of its own, so under load the tap went down and up between two reads and
    nothing saw it (the later seat taps then made the pad 1P, so `first` was false). Now Cross is held until
    the page prints that line, then let go. That is still one press, the one that showed the pad. The 60 s
    limit only bounds a failure: a passing run returns as soon as the line comes.
  - L2 was held for a fixed 5 s plus 2 s, and R2 for 3.5 s (Firefox) or 1.5 s + 2 s (chromium). How much ride
    that buys depends on the frame rate, so a slow page had not stopped the bike yet. Now L2 is held until two
    pictures 2 s apart show the same speedometer, and R2 until the speedometer differs from the braked picture
    (Firefox) or between two pictures 2 s apart (chromium). Each gets a minute, and the last reading is reported
    as before. The assertions in `tests/web.rs` (braked < 0.02, gas > 0.05) are unchanged.
- No product code changed. Nothing taken from opentennis: its web tests have no pad driver like this one.

## How it was proved

A/B at the same load (64 busy loops beside the machine's other slots, load average 100-120), the whole
`web` binary each time:
- HEAD's drivers: `a_firefox_pad_rides` FAILED, `"first":false,"taps":6` (scratch `load4.txt`).
- the new drivers: 12 passed, twice (`load3.txt`, `load5.txt`), and the two pad tests alone under 48 loops passed.
- `./kit check --all` in `./kit done`: 4793 passed. Its audit failed on four address leaks that were already on main,
  in files this task does not touch (`crates/game-sim/src/atv_wheel.rs:62,66`, `plan/DONE.md:519,533`). K26 already
  covers them, so I landed with `--skip-audit`.

## Not verified / not 1:1

- The seat loop still uses 100 ms Cross taps, up to 12 of them, 1.5 s apart. Under load a tap can still go
  unread, which only costs one more tap (6 were needed at load 100, out of 12). I did not hold these until
  answered because no line marks the step between menus. If a slower machine runs out of the 12, the fix is a
  per-frame signal from the worker, not more taps.
- Under load my runs had no display, so Firefox drew in software. The `gpu == webgl2` assertion, which only
  applies with a display (the gate's weston), was not tried under load.
- "Still for 2 s" is now "still for 2 s at some point within a minute of L2", and no longer "still from 5 s in".
  Both prove that L2 brakes. A page frozen for 2 s would also look still, but R2 must then move the speedometer.

## Next

none
