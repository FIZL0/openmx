# The knock's bbhit, heard in both

## The original's side

I made a new knock the S195 way, in PCSX2 copy 3 (KIT_PCSX2=3, PINE 28014):

- `research/s195/aim.py` ran from first_recording's playback state.
- `research/f27a1/capture.sh` turned System/Video Capture on over AT-SPI, saved the capture to
  `context/f27a1/f27a1.mp4`, ran `research/s195/knock.py f27a1 9 2 --frames 300`, then turned the
  capture off.
- knock.py's csv is `context/capture/s195/f27a1.csv`. Its states are `context/s195/f27a1-{2903..2906,2919}.p2s`.
  Their RAM images, from `research/s195/slots.py ram`, are `context/states/s195-f27a1-*.eeMemory.bin`.
- Video Capture writes the SPU mix as AAC at 48 kHz, in emulated time.

`research/f27a1/events.py` prints the ball's wake and bounces from that csv:

- Ball 2 wakes on 2904, with the bike at 18.98 ft/s. That is level (int)(100 · 18.98 / 25) = 75 for
  material 21, the same as knock-2882.
- The ball lands first on 2933.
- Ball 1 was nudged and woke on 2886. Ball 4 woke on 3089.

I matched the capture with `research/f27a1/match.py`, which is P164's NCC plus per-channel
least-squares gains and a two-copy fit. Its inputs:

- the capture, through `pcm.sh`, at 22050 Hz s16;
- `Impact\ball.vag`, decoded by the shell's `audio::Library` in a throwaway test (deleted).

| peak | NCC | gain L / R |
|---|---|---|
| 1.994 s | 0.547 | 0.494 / 0.489 |
| next best (5.215 s) | 0.346 | — |

The NCC is lowish because the strike lands in the ride's engine and roll mix. The hit is one voice:

- Window by window over the clip's first ticks, the first tick (33 ms) matches at NCC 0.970, gain
  0.495 / 0.490. The second tick's gain is 0.53 / 0.51, which is the same copy's tail.
- The two-copy fit gives the first copy a gain of −0.002 and leaves 78% of the energy, against 70%
  for one copy. There is no second copy within ±2 ticks.
- No cut-off copy sits before it either.

Where the strike lands in the video: the load jump is at 0.50/0.534 s, which puts tick 2904 at about
2.0–2.03 s. The audio's load discontinuity reads at about 0.36 s. So the absolute tick is only good to
about ±2 ticks.

## The port's side

`research/f27a1/ours.sh` rides the same line in the shell:

- The tape is `tape.py`'s: 12 ticks of full Cross, then idle, 150 ticks.
- It starts 36 ft short of ball 2 on aim.py's heading (35.88°). `tune.py` picked that start so the
  bike meets the ball at about 19 ft/s.
- It runs in a window on a private headless weston.
- An empty `ALSA_CONFIG_PATH` leaves cpal no device ("piping to a player instead"), and a stand-in
  `pw-cat` keeps the 48 kHz s16 stereo stream.

I ran it three times. The clip is found at NCC 0.92–0.94, and every strike is more than one voice:

| run | peak | NCC | gain L / R | two-copy fit |
|---|---|---|---|---|
| 1 | 2.046 s | 0.931 | 1.353 / 1.360 | — (voices coincide) |
| 1 | 2.709 s | 0.939 | 1.096 / 1.066 | — |
| 2 | 1.548 s | 0.923 | 0.659 / 0.667 | 0.643 + 0.587, 0.62 ticks apart |
| 2 | 2.187 s | 0.907 | 0.535 / 0.538 | 0.524 + 0.486, 1.22 ticks apart |
| 3 | 1.578 s | 0.854 | 1.247 / 1.237 | 1.131 + 0.588, 1.37 ticks apart |

- The stream is wall-clock and the ride starts slowly, so ticks bunch up. Two voices from consecutive
  ticks then fall nearly on top of each other, which gives run 1's gain of 1.35 (about 2 × 0.68).
- A second bbhit about 20 ticks after the first (level ~60) is the bike catching the ball again. The
  port's struck ball rises slower than the original's 35 ft/s, so the bike reaches it. The original
  has no second bbhit near that time.

## Why: the queue on the original's images

prop_knock.rs's `queued` ticks the props node on a RAM image and reads the table's queue. I ran it in
a throwaway test (removed). The queue entries are (owner record, material); 21 is the bike on the
ball, 16 is the ball on the bike:

| image | queue |
|---|---|
| knock-2881 | (bike 0xda3f00, 21), (ball 0x11c3760, 16) |
| knock-2882 | (bike, 21) |
| knock-2883 | none |
| f27a1-2903 | (bike, 21), (ball 0x11c3760, 16) |
| f27a1-2904 | (bike, 21) |
| f27a1-2905, 2906 | none |

The pair (21, 16) on the first tick keeps the bike's, because it is faster. So the original's
material_hit_play starts bbhit on two consecutive ticks, both above MinImpulse, yet the capture holds
one voice:

- The repeat is not a restart. A restart would put a second onset 1 tick in, and none is there.
- It is dropped, or played into an object already sounding. material_hit_play ends in the table
  entry's sound object's vt+0x1c play(4) (oracle F27 line). So there is one sound object per
  material entry, not a new voice per hit.

The port's `HitTable::sound` starts a new one-shot each time ("A hit does not stop the last one of
its material, so they sound over each other"). F27's journal read the start flag as not stopping the
earlier voice. That still holds, but the second play is not heard at all. Filed as a B follow-up. The
rule of that object's play (ignore while playing, a minimum gap, or a per-object single voice) was
not read.

## Files

All under `research/f27a1/`; nothing in the crates changed.

| file | what it does |
|---|---|
| `capture.sh` | the PCSX2 knock with Video Capture |
| `pcm.sh` | mp4 to mono and stereo pcm |
| `events.py` | the ball's wake and bounces from the csv |
| `tune.py` | picks the run-up |
| `tape.py` | writes the run-up's pad tape |
| `ours.sh` | the shell's ride with its sound kept |
| `match.py` | the matched filter |

## Not verified / not 1:1

- **Absolute latency.** The capture's tick alignment is only good to ±2 ticks, and ours is
  wall-clock. The onset was not compared to the tick in either.
- **Gains across the two mixes.** 0.49 in the capture and 0.66 a voice in ours are in different mixes
  with no common reference (master volume, SPU voice volume curve, 3-D falloff). So level 75 →
  linear 0.75 was not checked.
- **Not the same ride.** Ours is a tape from `--trace-at` on aim.py's line, not seeded from the
  f27a1 RAM image (S174 Image). The meeting speed was tuned to about 19 ft/s with the bike alone
  (`--trace-feel`), and the speed at the ball in the full ride was not read.
- **The ball's launch differs.** It rises slower in the port, and the bike catches it again about
  20 ticks later. That second hit is the port's alone. It was not chased; it is the props' or the
  strike's, not this task's.
- **The suppression rule is not found.** Only that the second tick's queued play is not heard.
- **Only the props node.** The queue reads tick only the props node, as F27a's test does. The bike
  body rays' own queueing on those ticks (from the vehicle update) was not added. On 2903 and 2904
  the bike's entry is the props node's.
- The ball-on-ground bounces (material 21 from the ball's own probe) were not looked for in either
  recording.
