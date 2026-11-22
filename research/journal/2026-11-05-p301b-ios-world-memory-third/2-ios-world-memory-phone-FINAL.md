# 2 — every world rides under 2 GiB on the iPhone (FINAL)

Task P301b. 2026-11-21.

## What I did

The iPhone 15 Pro is on omarchy-desktop's USB now, so I drove it myself instead of waiting for a log.
I used iFizBox's `tools/ui-15.sh` under the lock `P301b` and gave the lock back at the end.

- The phone had openMX 0.18.0 build 5260 (bd79f93, 2026-11-18). That build has P301 and P301a.
  Its `world paint:` lines say `pages 128x128`, so the lean ground was on.
- First I pulled the log the phone already had: the P224 session, world_mxu_free and Cleveland.
- The phone had only world_mxu_free installed. I installed world_ultra (1018 MB) and world_mxvatv
  (319 MB) from the in-game Mod Store (Tracks tab → World category).
- For each world I killed the app (`ios kill`) so the peak counts from a fresh launch. Then Free Ride →
  the world, and I rode on the touch HUD: Gas held 25 s at a time, with a short stick push between.
  The bike crossed several maps each time (`world paint: … shown at tick` lines).
- After each ride I pulled `Documents/openmx-log.txt` with `ios fsync`. The logs are in
  `context/phone-logs/p301b/` (`openmx-log-1121.txt`, `ultra-1.txt`, `ultra-2.txt`, `mxvatv.txt`,
  `mxu.txt`).

**Nothing to cut.** "Memory" here is the app's footprint as iOS counts it. Every ride stayed between
0.7 and 1.3 GB, with 5.3 GB or more left before iOS closes the app.

| world (phone log) | ride | load peak (GPU) | highest a second (its GPU) | GPU highest |
|---|---|---|---|---|
| world_mxu_free (`openmx-log-1121.txt`) | ~4 min, P224's | 957 MB (408) | 854 MB | 425 MB |
| world_ultra (`ultra-2.txt`) | ~7 min, 7 maps shown | 1251 MB (532) | 1302 MB (657) | 665 MB |
| world_mxvatv (`mxvatv.txt`) | ~4 min, 5 maps shown | 704 MB (283) | 787 MB (421) | 422 MB |
| world_mxu (`mxu.txt`) | ~4 min, 5 maps shown | 1312 MB (583) | 1243 MB (585) | 622 MB |

- The phone holds less than the desktop did at the phone reach (P301a: 1.4 / 1.6 / 2.3 GB, 2.5 GB in
  world_ultra's sweep). The desktop's figure was RSS plus all the driver's VRAM and GTT. iOS counts a
  single footprint, and Metal shares its memory with the CPU.
- The 5.2 GiB JetsamEvent that started P301 came from a build before P301 and P301a. The far reach (6000 ft)
  and the full-size pages it had are what P301 and P301a cut.
- I changed no code. HUMAN.md's "What a world ride holds on the iPhone (P301, P301a)" asked a person
  for this log, so I removed it.

**Seen in passing: world_ultra fell to ~30 fps after 7 minutes.** From 420 s on, `wait` was ~25 ms of
a ~37 ms frame. The thermal state had gone `fair` earlier in the ride (line 840). This is frame time, not
memory, so I added it as a note to P346 (iOS present stalls), the open task on drawable wait.

From opentennis: nothing. It has no streamed world.

## What the original does

Not applicable: this is the app's memory on a phone, not game logic.

## How it was proved

- Four phone logs from fresh launches, on the build that has P301 and P301a, with the lean ground
  confirmed by the `world paint:` line (table above).
- No code changed, so no test changed. The gate is run by `./kit done`.

## Not verified / not 1:1

- **The phone ran 0.18.0, not this branch's main.** Main is at 0.19.0. Since 4fce2110e only V137, P385,
  V132 and P377 touched the world files (`git log 4fce2110e..HEAD` on worlds.rs, paint_stream.rs,
  video.rs, gpu.rs, terrain.rs), and none of them changes what a world holds. But it was not measured
  on 0.19.0.
- **Rides were short and mostly straight.** Touch rides hold Gas with one finger, so the bike goes
  mostly one way: 7 minutes on world_ultra and about 4 on each of the others. A ride across all of
  world_ultra's 77 maps (desktop's sweep peaked 2.5 GB) was not done on the phone. The footprint rose
  slowly over the world_ultra ride: about 1050 MB at 30 s, 1180 at 2 min, 1245 at 6.5 min, with a
  highest of 1302. A much longer ride could climb further. Nothing in the logs says the footprint keeps
  growing once maps are let go (`let go at tick` lines), but that was not checked over half an hour.
- **Only the iPhone 15 Pro.** A phone with less memory (iOS gives the app less) was not tried.
- **No JetsamEvent check.** The app never closed, so there was nothing to read in Analytics Data.
