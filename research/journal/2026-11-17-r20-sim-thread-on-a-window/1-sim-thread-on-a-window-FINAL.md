# 1 — sim thread on a window (FINAL)

Task R20. 2026-11-17.

## What I did

- **No Deck.** The task says "needs the Deck". The standing rule (memory `profiling-on-the-deck`, user
  2026-11-02) is that runs never `ssh deck` and take timings on this desktop. So I did that, and the
  Deck's own numbers are still open (below).
- **Tape.** A hand-written 1800-line 2P gas tape like N23a's: 30 Hz dt (`0x3d08850a`), idle for 30
  ticks, then Cross held at full pressure on the first player. N23a's tape was in its scratchpad and is
  gone. The generator is three lines: idle report `0079ffff7f7f7f7f`+24×`00`; gas report has byte 3
  `0xbf` and byte 14 `0xff`.
- **Runs.** Release build of this branch (main at 2d2dc4208). Each run was
  `openmx --track free06 --players 2 --play gas2p.tape --frames 600 --timings --sim-thread off|auto`
  in a real window on the desktop's Wayland display (60 Hz, so the budget is 16.67 ms). Off and auto
  were interleaved. There were three sets: 3 pairs at the default window size, 3 pairs at
  `--size 320x224`, and 2 more pairs at default size after waiting 15 minutes for the load to drop
  (it never went under 15).
- No code changed. `sim_thread.rs` is as N23a left it.

## Results

Drawn = pictures drawn in the 20 s ride (sum of the once-a-second fps lines). Over = the session line's
"N over 16.7".

| set | pair | load | off: drawn / avg ms / over | auto: drawn / avg ms / over | auto's line |
|---|---|---|---|---|---|
| default | 1 | 11→17 | 411 / 46.6 / 430 | 343 / 55.4 / 362 | tick 8.22 + picture 55.75 |
| default | 2 | 17→24 | 239 / 77.3 / 260 | 230 / 83.0 / 242 | tick 12.87 + picture 79.32 |
| default | 3 | 24→29 | 217 / 87.8 / 229 | 228 / 83.9 / 240 | tick 12.67 + picture 78.11 |
| 320x224 | 1 | 28→45 | 270 / 62.6 / 321 | 230 / 82.7 / 243 | tick 60.68 + picture 81.72 |
| 320x224 | 2 | 45→50 | 178 / 92.3 / 218 | 139 / 93.9 / 214 | tick 17.93 + picture 98.44 |
| 320x224 | 3 | 50→43 | 299 / 61.7 / 325 | 323 / 58.8 / 341 | tick 18.68 + picture 84.89 |
| default, later | 1 | 17→32 | 435 / 44.5 / 452 | 121 / 85.0 / 238 | tick 12.82 + picture 93.11 |
| default, later | 2 | 32→43 | 152 / (no line) | 241 / 62.1 / 325 | tick 12.46 + picture 124.21 |

(The last "off" run's session line was cut by a `sound: A backend-specific error` line on the same
line of output, so its averages are missing.)

- **Does "sim thread on" engage?** Yes, in every auto run. It switched on after the first second and
  printed one `sim thread on:` line, then stayed on.
- **Why it engages here.** The picture alone overruns the refresh, 2–7 times over. The ground stage
  is about 26 ms a frame in a window and about 2 ms headless (N23a). The tick is a small part of the
  sum. The gauge engages whenever tick + picture > budget, so a slow picture turns it on even when
  overlapping the tick can save only the tick's 1–2 ms a frame.
- **Frames saved over the refresh:** no answer. Drawn frames for the same mode ranged from 121 to 435
  between runs, following the load. That swamps any saving from the thread. Off was ahead in 4 pairs
  and auto in 3, with one pair missing a line.
- **Same ride:** all 16 runs, at both sizes and in both modes, ended on ride hash
  0xff6771a558219ef2 at 2942.30, 170.96, 3808.76. This repeats N23a's windowed proof with a real
  compositor instead of weston.

## How it was proved

- The same ride: the ride hash on all 16 windowed runs, listed above.
- Engagement: the `sim thread on:` line in every auto log, and none in any off log.
- The timing question is not proved. See below.

## Not verified / not 1:1

- **Not measured on a Steam Deck.** The task asked for the Deck; the standing rule forbids using it in
  runs. Whether auto engages there, and how many frames over the refresh it saves, is still unmeasured.
  From the 2026-11-02 Deck baseline (2P sim 5.5–6.1 ms a tick), a Deck picture over about 10.6 ms would
  engage it. That is arithmetic, not a measurement.
- **No frames-saved number, even on the desktop.** The load stayed at 11–50 for the whole session (a
  15-minute wait never got it under 15). The window's picture was GPU- or load-bound at 36–124 ms, so
  run-to-run noise (121–435 frames drawn) hid any effect of the thread.
- **The 26 ms ground stage in a window** against about 2 ms headless was not looked into. It is most
  likely the other slots' load on the shared GPU and CPU, but that is unproved.
- **The gauge's threshold** still isn't tuned. Here it engages on a slow picture, where the thread can't
  help much and the held picture shows motion in 30 Hz steps. This was not judged by eye.

## Next

none — a Deck measurement only if the user asks for one in a session.
