# S246 — the cars' cameras

2026-11-02. Task: `./kit plan S246` (an S149e follow-up).

A driven car is now followed by its own camera. Before this, the shell aimed the bike's camera
(the track's `cpresets.ini`) at the car, using a target the port made up. Now each car kind gets
its own camera:

- the baja reads `FourWheelCPresets.ini`;
- the buggy reads `BuggyCPresets.ini`;
- the monster truck reads `MonsterTruckCPresets.ini`.

Each camera starts on "Race Cam 1", and the target block the original's car class fills for it
was read off the disc. Both were checked against captures of S149a's three car drives on the
original, row by row. Details are in the numbered files; the newest one's suffix says whose move it
is (FINAL · PART · READY).

- `capture.sh`: plays one of S149a's car tapes on the original and records `watch/car-camera.toml`.
  The captures are `context/capture/s246/{baja,buggy,truck}.csv`, 650 ticks each, in the s2
  worktree's context.
- `1-car-camera-FINAL.md`: what the original does, what changed, and how it was checked.
