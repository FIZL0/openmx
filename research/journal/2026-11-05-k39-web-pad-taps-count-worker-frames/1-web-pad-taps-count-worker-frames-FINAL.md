# 1 — web pad taps count worker frames (FINAL)

Task K39. 2026-11-05.

## What I did

- K27's merge gate said `-p openmx --test web failed, then passed alone`. The runner's lone rerun overwrote
  `merge/context/notes/check.log`, and no other log names the test, so I reproduced it. Three copies of the `web`
  binary at once (`RUST_TEST_THREADS=10`, as the gate sets it) beside 128 busy loops (load 106–188): 2 of the 3 failed
  `a_pad_the_browser_shows_on_a_press_rides` with `"first":true,"taps":12,"seated":false`. That is P236's red from
  10-30, and what K34's journal said was still left open.
- The race: `tests/web/gamepad.mjs` steps through the menus with 100 ms Cross taps, each followed by 1.5 s of wall
  clock, 12 at most. The worker samples the pads once per frame of its own (`web_host`'s `PadNode::take` reads the
  page's last `{ pads }`). When the page is slowed, a 100 ms press can go down and up between two of those frames,
  and 1.5 s holds only a few frames of the menus. So the 12 taps run out. `firefox_gamepad.mjs` (`a_firefox_pad_rides`)
  has the same loop.
- The fix is test-only and touches no product code. A new `tests/web/frames.mjs` holds a prefix that each pad driver
  serves ahead of `worker.js`, the way `gpu.mjs` already serves its fault. The prefix wraps the worker's
  `requestAnimationFrame` to count its frames, keeps the Cross value from the page's last `{ pads }` message, and
  posts `[frames, cross]` on a BroadcastChannel every frame. A page-side listener keeps the latest pair in
  `window.openmxTestSeen`. In the chromium driver the listener is added to the pad stub's new-document script; in
  Firefox it is evaluated after navigation. Both drivers now:
  - hold Cross down until the worker has heard it and drawn 3 frames, then hold it up the same way. This applies to
    each tap, and to the release of the first press (K34's held first press is kept);
  - wait 90 of the worker's frames between taps (1.5 s at 60 Hz) instead of 1.5 s of wall clock. Still 12 taps at
    most. The 60 s limits only end a run that is failing; a passing run returns as soon as the frames are counted.
  - print `frames` (the worker's frames counted) in their JSON line, so a failure shows whether the counter ran.
- First I tried CDP `waitForDebuggerOnStart` with a `Runtime.evaluate` into the worker. It worked in chromium, but
  BiDi has no equivalent for a module worker, so I switched to the served prefix and both drivers use it.
- Checked the other drivers for the same pattern. Their key presses are fine: keys reach the shell as queued events
  (`worker.js` `keys.splice`), so a short press is never lost. Only the sampled pads raced.
- Nothing taken from opentennis: it has no pad driver like this (K34 checked).

## What the original does

Not applicable: the gate's own test drivers, no game behaviour.

## How it was proved

Most A/Bs slow one driver's own chromium (or Firefox) process tree with SIGSTOP/SIGCONT: stopped for N ms of every
100 ms, found from the node pid the script started (scratch `slow.py`). The same slowdown every run:

| driver | 0 % | 80 % | 90 % | 95 % |
|---|---|---|---|---|
| HEAD `gamepad.mjs` | 3 taps | 6 taps | 10 taps | **12 taps, seated false** (the gate's failure) |
| new `gamepad.mjs` | 3 taps (821 frames) | 3 taps | 3 taps (322 frames) | 3 taps, seated (319 frames) |
| HEAD `firefox_gamepad.mjs` | — | — | 12 taps (seated on the last) | — |
| new `firefox_gamepad.mjs` | 3 taps (296 frames) | — | 3 taps (235 frames) | — |

The whole binary, 3 copies at once beside 160 busy loops (load up to 243): 12 passed in each, 3/3. Before the fix,
the same kind of round (128 loops) failed 2 of 3. `./kit check -- -p openmx --test web`: 12 passed.

## Not verified / not 1:1

- The K27 gate's own failure was never seen: its log was overwritten. What I fixed is the failure I could
  reproduce, which matches P236's earlier red exactly. If K27's red was another test of the binary, this does not
  cover it.
- `the_browser_rides_on_the_gpu` still holds a wall-clock median of 30 fps. It went red twice in gates
  (10-29, 10-30), but under CPU load alone it stayed at 48–60 fps (load 243). So its race is probably the GPU,
  shared with other gates' GPU tests, which I could not reproduce. Filed as K40 (low) rather than guessed at.
- 90 worker frames between taps assumes the menus need about as many frames as before between presses. Idle runs
  still take 3 taps, as they did with 1.5 s at 60 fps. If a menu waited on wall clock (a timer, not frames), a
  slowed page could still use more taps; none did in these runs.
- `frames.mjs` wraps the worker's `requestAnimationFrame` in the test pages only. That puts one closure and one
  BroadcastChannel message per frame on the worker under test. The fps test (`gpu.mjs`) does not use it.

## Next

none
