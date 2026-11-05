# K39 — web pad taps count worker frames

2026-11-05. Task: `./kit plan K39`.

K27's merge gate went red on the `web` binary, and it passed alone. Three copies at once under busy loops reproduced
it: `a_pad_the_browser_shows_on_a_press_rides` failed with 12 taps and no seat. The pad drivers timed Cross taps in
wall-clock milliseconds, while the worker samples the pads once per frame of its own. Now `tests/web/frames.mjs`
counts the worker's frames, and both pad drivers (chromium and Firefox) hold each tap and space the taps in those
frames. In a SIGSTOP-slowed A/B, HEAD needed 12 taps (and failed) where the new drivers took 3. The GPU test's 30 fps
median, a different wall-clock check, is filed as K40.
