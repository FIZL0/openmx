# C40 — the drop gate's sink, live on sx17

Journal: research/journal/2026-11-17-c40-drop-gate-live. The captures are in context/capture/c40 (git-ignored).

- `boot.py SLOT [RACERS] [LAPS] [ALL_AI] [TRACK] [LEAGUE]`: C31's scripted boot with the track as an argument
  (league 0 sx, 1 nat; SingleRace). It saves SLOT the moment `in_game` is 1. Poke from the emulator's
  first second (start the copy and the boot in one command).
- `capture.sh SLOT NAME TRACK LEAGUE [FRAMES] [RACERS] [LAPS]`: the boot, then `spec.py`'s recording, then saves
  SLOT+1. sx17: `WAYLAND_DISPLAY=wayland-1 ./kit pcsx2 start && ./kit pcsx2 with -- bash research/c40/capture.sh 36 sx17 sx17 0 6000`.
  The gate fires about 1000 frames in, when the leading AI rider passes 2475.76 ft from the line.
- `spec.py [CARDS] [FRAMES]`: the gate object (activity +0x3ac), the group node, both gathered collision
  pieces' collidable words, and each card's two line distances, in list order.
- `summary.py CSV`: the trigger, the firing row, the heights and the pieces' matrices around it.

| file | what |
|---|---|
| `context/states/c40-sx17-race-start.p2s` / `.ee.bin` | the start (copy 4 slot 36), before any card |
| `context/capture/c40/sx17.csv` | 6000 rows from frame 1; the sink fires on frame 1005 |
