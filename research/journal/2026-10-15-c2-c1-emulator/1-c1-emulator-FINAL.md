# 1 — c1 emulator (FINAL)

Task C2. 2026-10-15.

## What I did

Found the game's per-frame counter, proved it counts simulated frames (and nothing else), and set it as
the kit's clock:

```toml
[emu]
vsync_addr = "0x479d04"      # app+0x374, R4's @frame_counter: +1 per simulated frame, 30 Hz
```

**It is not what `./kit probe vsync` picks.** The probe offers 16 addresses and none of them is this one;
the winner it would save (`0x47aa58`) advances by **two** per game frame, which would make `./kit record`
score a dropped-frame gap on every single frame it ever records. The whole of this task is that
distinction, so it is written out in full below.

### The game runs at 30 Hz, and there are two counters

| | what it counts | rate | step per change |
|---|---|---|---|
| `0x479d04` (app+0x374, R4's `@frame_counter`) | simulated frames | 30.00/s | **+1** |
| `0x47a9d8 + n*0x80`, n = 0…15 | display fields (vsync) | 60.00/s | +2 |

Both move once per game frame, 33.4 ms apart — the 60 Hz one is bumped by 2 each time rather than ticking
twice, so it is a field count maintained by the game, not a free-running hardware counter sampled live.

The 16 words that `./kit probe vsync` returns are one array: base `0x47a980`, 16 entries of 0x80 bytes,
the counter at +0x58 of each. The first eight bytes of every entry are `00 79 ff ff | 7f 7f 7f 7f` —
status 0, mode 0x79 (DualShock2, analogue + pressure), button mask 0xffff (nothing held), four centred
sticks. That is a `scePadRead` buffer, so the array is libpad's 0x80-byte pad buffers (2 ports × 8 slots)
and the per-frame word is padman's refresh counter, which the IOP drives from the display, not from the
game's loop. Alternate entries are one apart (even entries read 6099 where odd read 6098), which is why
the probe reports two rates for one array.

### Why the vsync counter is the wrong clock here, and the frame counter is the right one

`./kit record` does `v = pine.next_frame(last)` and counts `v > last + 1` as dropped frames
(`ps2kit/record.py:1123`). With `vsync_addr = 0x47aa58` every frame of every capture arrives as `+2` →
300 gaps in 300 frames, 300 "dropped" frames, and the CSV's frame column counts fields rather than ticks,
so a port comparison would be off by a factor of two from the first row. With `0x479d04` the same
300-frame run has 299 deltas, all exactly 1, and no gaps.

The second reason is semantic: `0x479d04` is frozen by the game's own blocking loaders (R4 measured that;
I measured it again, below), so it is a count of *ticks the simulation actually ran*. That is what a
timing comparison against the port has to be counted in. The pad counter keeps running at 60/s through a
20-second load, which would silently tell a recorder that 1200 frames of gameplay happened during a
loading screen.

### Durable copy of what this task put in `kit.toml`

`kit.toml` is git-ignored, so (as R4 did for its symbols) the value lives here as well:

- `[emu] vsync_addr = "0x479d04"` — the game's per-frame counter.
- Not added to `[symbols]`: R4 already has it there as `frame_counter`. `@frame_counter` and
  `[emu] vsync_addr` are deliberately the same address.

Other numbers worth keeping:

- the pad/field counters: `0x47a980` + `n`*0x80 + 0x58, n = 0…15 (first is `0x47a9d8`), +2 per frame;
- save state slot 9 (C3's main menu) sits at frame **2250** — `./kit state extract 9 --addr 0x479d04
  --len 4 --as u32`, which answers the "no frame number on any state" gap C3 left open, for this state.

## What the original does

Nothing decompiled here; the addresses come from R4 (`app` at `0x479990`, `@frame_counter` at app+0x374,
`@game_mode_request` at app+0x404, `@game_scene` at app+0x3cc) and were confirmed live.

Two behaviours of the game were measured rather than read:

- **The game simulates at 30 Hz, on a 60 Hz display.** Steady 30.00 ticks/s at the main menu and 30.00 in
  gameplay, against 60.00 fields/s. The menu drops the odd frame under load (27.0/s and 22.0/s windows
  were seen while the disc was busy), and the counter never skips a value when it does — it just pauses.
- **A mode transition stops the counter dead.** Writing 6 to `@game_mode_request` at the main menu (R4's
  MainMenu → gameplay request) allocated `@game_scene` within 0.5 s, then `0x479d04` did not move for
  **20.0 s** while `0x47aa58` kept ticking 60/s; after that the counter resumed at 30/s. R4 measured the
  same handler at ~19 s. Loading continues in further blocking phases after the scene exists: the counter
  alternates between runs of 30/s and stalls of up to 21 s, with the loading bar advancing across them.

## How it was proved

All live, on PCSX2 copy 2 (`KIT_PCSX2=2`), each job held under one `./kit pcsx2 with --`. Note for the
next session: starting the emulator here needs `WAYLAND_DISPLAY=wayland-1` exported by hand.

1. **`./kit probe vsync`**, twice, from C3's main-menu state (slot 9): 16 candidates, 59.7–60.4 frames/s,
   error 0.2–0.8 %, all in the `0x47a9d8`+0x80 array. The probe's own paused check (`_reject_while_paused`)
   kept all 16, so they are all EE-side RAM.
2. **The task's test — 300 frames, +1 each.** `./kit pine watch 0x479d04 0x47aa58 --frames 300`, with
   `vsync_addr` set to `0x479d04`: 300 rows, frame column deltas `{1: 299}` — no gap, no repeat, no skip.
   The `0x47aa58` column in the same rows is `{2: 299}`. CSV kept at `context/pine/c2-vsync-300.csv`.
   The same run with `vsync_addr = 0x47aa58` (the probe's winner) gives a frame column of `{2: 299}`.
3. **Step size, independent of the kit's frame loop.** A tight PINE read loop (78 000 samples/s, 390 000
   samples over 5 s) on `0x479d04`, `0x47a9d8`, `0x47aa58`: 150 changes in 5 s for all three; the 30 Hz
   one moves +1 per change, the two 60 Hz ones +2. Change-to-change gap 31.0–35.8 ms, median 33.4 ms.
4. **It stalls when the game stops.** Emulator pause (EE halted) with PINE: `0x479d04` 9540 → 9540 → 9540
   across 2.4 s paused, then 9556 half a second after resume; both field counters freeze identically.
   And the game's own blocking load, which stops the simulation without stopping the display: 20.0 s at
   0.00 ticks/s on `0x479d04` while `0x47aa58` ran at 60.00/s throughout (see above).

## Not verified / not 1:1

- **The in-game pause menu was never reached, so "Start during a race freezes the counter" is not
  proved.** What is proved is the stronger-to-measure pair: the EE halted (counter frozen) and the game's
  own blocking loader (counter frozen while the display keeps running). Forcing the transition with
  `@game_mode_request = 6` from the menu state put the game into a long chain of loading phases — black
  screen or the "Loading" bar in `context/shots/c2-paused.png`, `c2-racing.png` — and the two Start
  presses I sent landed in that, not in a race; the screenshots show no pause menu and the counter was
  already stalled by the load when they were sent. Reaching an actual race needs human save state 4
  (plan/HUMAN.md), which does not exist yet. Whoever makes it can settle this in one command.
- **Why the array of 16 is identified as libpad's pad buffers is an inference from one dump**, not from
  code: the 0x80 stride, the `00 79 ff ff 7f 7f 7f 7f` header and 2 ports × 8 slots all fit, but nothing
  statically references `0x47a9d8` (`./kit xref` finds nothing — it is reached through a base register),
  and I did not trace `scePadPortOpen`'s buffer argument to confirm. The *counting* behaviour, which is
  all this task rests on, is measured, not inferred.
- **The 30 Hz rate is a measurement at the main menu and in the first seconds of a loaded world only.**
  I did not watch a race, a replay or an FMV, so "the game is 30 Hz everywhere" is not established —
  if some mode ticks at 60, `0x479d04` still counts its frames one by one, which is what the clock needs.
- **`./kit probe vsync` still picks the wrong address on this game and nothing in the kit says so.** Its
  band is `FPS_LO, FPS_HI = 45.0, 65.0` (`ps2kit/probe.py:46`), so a 30 Hz counter can never be a
  candidate, and nothing in `find_vsync` prefers a counter that steps by 1 over one that steps by 2. I did
  not change the kit — C2's file is `kit.toml` and other agents are in `ps2kit/` — so the next person to
  run `./kit probe vsync --save` on this game will overwrite `vsync_addr` with `0x47aa58` and get a
  capture that reports every frame as dropped. A fix worth a `K` task: widen the band to ~24 Hz and rank
  candidates by "+1 per change" before linearity.
- **`kit.toml` cannot be committed** (git-ignored), so this branch's commit carries the journal only. The
  value above has to be put into `kit.toml` by hand in any checkout that has not got it.
- **Counter wrap is not tested.** It is a u32 at 30 Hz: 4.5 years of running to wrap, and the recorder's
  `v > last + 1` would see one bad frame if it ever did.

## Next

Nothing is waiting on this task. The two open ends it hands over: C4 can now record, and whoever makes
human save state 4 (a real race) can close the in-game-pause gap above with
`./kit pine watch 0x479d04 --frames 120` either side of a Start press.
