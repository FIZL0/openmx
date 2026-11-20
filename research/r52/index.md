# R52 — the fliers the original parks in play, and free07's baja

2026-11-20. Test: `crates/game-sim/tests/parked_fliers.rs`. Boot script: `research/r52/boot.py`.

## Why the old images showed no flier

- Machine placement (`0x0020f6e0`) walks the event's **machine slots**, not `NumberOfMachines` players.
  - Event = `[[0x0041f770] + 0x3d4] + 0xc` (0x00506580 in every image read).
  - Slot i: `+0x11c + 12i` the kind (1 Biplane, 2 Helicopter, 3 Buggy, 4 Monster, 5 Baja, 6 none),
    `+0x120 + 12i` (byte) the machine-racer flag.
  - Count = `(+0x11c != 6) + (+0x128 != 6)`.
  - Flag clear: the kind's own `[<Kind><i+1>]` bay. Flag set: the `[Machine<n>]` with `MachineRacer = TRUE`.
- Which images hold what:

  | image | slot 0 | parks |
  |---|---|---|
  | recording2, first_recording (free09) | (4, 1) | `[Machine1]` truck |
  | s133-free10, s196-free11 (S133's boot.py) | (6, 0) | nothing |
  | c34-free06-* | (3, 0) | `[Buggy1]` |
  | s342-baja-free07 | (5, 0) | `[Baja1]` |
  | c14-kiowa (free09) | (2, 0) | `[Helicopter1]` |

- So `vehicle::in_play` (always `[Machine1]`) matches the racer branch, which is what recording2's menu
  session set. What a menu free ride sets on the other maps was not traced.

## The new images (copy 3, 300 frames after "in game", no pad, `ALL_AI` 0)

`boot.py TRACK 2 5 7 KIND [FRAMES]` = S133's boot plus slot 0 = (KIND, flag 1).

- `context/states/r52-free06-heli.ee.bin` — free06, helicopter:
  - hull `Kiowaf.col` at 0x01491070, category 0x202058 (`kiowa::COLLIDABLE`), collidable.
  - machine 0x005ff210 (hull +0x24 − 0x58, `+0x178` = hull, as the car's); body 0x0148fe00.
  - stored spot (wrapper +0x90): (2570, 276.88528, 2734) = file y 271 + the lift; nose = the file's.
  - body (2569.959, 276.493, 2734.0), v (0, 0.0016, 0), up (0.0094, 0.99983, 0.0159). **Awake.**
  - So it sits 0.39 ft under its spot, on whatever is at 271 ft (it does not hover or fall).
- `context/states/r52-free10-biplane.ee.bin` — free10, biplane:
  - hull `biplanef.col` at 0x00611880, category 0x42058 (`flight::COLLIDABLE`).
  - machine 0x005dbeb0, body 0x015794e0.
  - stored spot (2754.95, 183.19116, 2870.41); body (2754.636, 182.677, 2870.430).
  - v (−0.013, 0, 0.021): still creeping. Up (0.042, 0.984, −0.174): resting on its tail. **Awake.**
- Both: `ai::parked_hull(hull, body)` is the hull object's matrix, centre and reach bit for bit.
- Both machines' sleep thresholds (wrapper +0x1c4) are (0.3, 0.3, MAX, MAX, MAX, MAX); the car's are
  (0.3, 0.3, 1, 1, MAX, MAX). So the fliers likely never sleep: they are stepped every frame.

## free07's [Machine1] baja

- `r52-free07-baja.ee.bin` (frame 303) and `r52-free07-baja-later.ee.bin` (frame 1711), slot (5, 1).
- Spawn (car +0x270) (3899.637, 141.31993, 2438.25), nose the file's.
- Frame 303: body at x 3916.34, vx 1.27 ft/s, rolled about 18 deg (up 0.30, 0.93, 0.18). Awake.
- Frame 1711: x 3941.29, vx 0.08 ft/s. **Still awake**, 42 ft from its spot.
- The port's settle (`car::Parked::settled`, 400 frames) puts it to sleep at 3919.8.
  So the port stops it about 21 ft short.
- S149a's free07 image (`s149a-free07.ee.bin`, used by tests/car_spawn.rs) is no longer in context.
  The new images have the baja at the same machine address (0x00605270).

## Not done

- No product change. Handing the fliers to `Sim::parked` needs a flier settle in the port (the
  original's bodies are live), and `Sim::park_cars` keeps `Sim::parked` one hull per car (lib.rs).
  Filed as follow-ups, with the baja's roll.
- The AI walk was not run on these images (no AI riders). The fliers' hulls are their machines' and
  collidable, as the truck's that recording2's walk hands out.
- Frames are the emulator's `frame()` count, not game ticks.
