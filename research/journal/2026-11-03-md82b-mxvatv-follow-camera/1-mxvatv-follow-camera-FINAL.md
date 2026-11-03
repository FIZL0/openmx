# MD82b — MX vs. ATV's follow camera, replayed tick by tick

MD82a found which camera file each MX vs. ATV machine follows with (`Cam\FC_<kind>Default.pb`). This
task ports what SLUS-21104 does with that file every frame. That is MXCameraFollow's update (oracle
`atv_follow_update`) and its callees: the aim, the distance floor, the look-ahead with the ground
probe, the position and the filter. The port is proved against eye/target captures behind each MD39a
car and the MD37a ATV.

## The port

`game_sim::camera::ModFollow` wraps MXU's race camera (`Race`) and reuses its pieces where SLUS-21104
calls the same code: the lags, `position`, the ground probe, the keep-off-ground lift and the fov.
It differs from `Race` in these places:

- **Heading.** When the rider is airborne, the heading is the velocity flattened to y = 0. This holds
  only while the speed is over 20 and is not simply its vertical part (`|speed − |vel.y|| > 0.01`).
  Otherwise the heading is the body's forward.
- **No reverse ramp.** The reverse test only swaps the velocity for (0,0,0,1) and the speed for 0.
- **Swing.** The swing is `(−lean · 0.21) · fov ratio`. The aim takes `swing · Swing` off the yaw,
  faded in between π/2 and 2π/3 of yaw. Because of MD82a's key quirk, `Swing` is 0 in every file.
- **Pitch.** The pitch clamp is the race camera's with no reverse term. There is a low-speed trim, and
  `msub(k, |forward.y|)`, and a short-viewport `msub(angle_x, 0.33)`.
- **Distance floor.** `max(min, 4.5 · near clip)`.
- **Look-ahead.** It is scaled by a *rear fade*: 0 → 1 over |yaw| from 1.0472 (0x3f860a91) to π/2.
  The swing vector is `rotate(wrap(swing · (−0.7 · LookSwing)), UP, heading)`.
- **Lead point.** `point + vel · min(0.2, dt)`.
- **Filter.**
  - Keep-off-ground runs with the pitch lock on.
  - When the lags run, the look and position time constants are rewritten from the preset every
    frame. The preset's value is scaled by `fade(speed, 0..29.3332, 1 → rear fade(0.25))`.
  - Then the base filter runs, then keep-off-ground again with the lock off.

The generic blend SLUS-21104 uses for all three fades is the same everywhere:
`clamp(madd(adda(0,a), (x−lo)/(hi−lo), b−a), min(a,b), max(a,b))`, giving `a` when lo == hi. The
port has it as `ModFollow::fade`.

## The captures

- **Spec:** `research/md82b/cam.toml`. It reads the target block (cam +0x80), the distance, yaw and
  pitch, the user offsets, snap, preset, lift, lock, the 3+3 lags, the fov lag, and the viewport's eye,
  direction and near clip.
- **Player:** `research/md82b/play.sh`, which is MD39a's player with this spec and a `start` that
  waits for the camera's vtable.
- **Run:** in my own SLUS-21104 PCSX2 copy (a kit.toml with serial SLUS-21104 and vsync 0x3fa6d8),
  playing MD39a's `drive.p2m2` (735 ticks) from each state. Each capture has 730 rows, one per game
  tick (vsync +2).

| capture | state | file | replayed | from row | airborne rows |
|---|---|---|---|---|---|
| buggy | md39a buggy | FC_BuggyDefault.pb | 729 | 0 | 167 |
| baja | md39a baja | FC_BajaDefault.pb | 729 | 0 | 174 |
| sandrail | md39a sandrail | FC_SandrailDefault.pb | 727 | 1 | 167 |
| golf | md39a golf | FC_GolfCartDefault.pb | 728 | 1 | 56 |
| truck | md39a truck | FC_MonsterDefault.pb | 728 | 1 | 201 |
| atv-r-spawn | md37a atv-r-spawn | FC_ATVDefault.pb | 658 | 1 | 150 |
| atv-s-spawn | md37a atv-s-spawn | FC_ATVDefault.pb | 650 | 1 | 174 |

`tests/mxvatv_camera.rs` seeds `ModFollow` from one row. After that it hands the camera nothing but
each row's target block. The world is Desert Oasis's terrain (`atv_ground` over mc02.res's map,
MD76). For every replayed row, these must match the capture bit for bit:

- distance, yaw, pitch;
- lift;
- the look lags' values and time constants;
- the position lags;
- the viewport's eye;
- the fov lag.

## Rows that are not plain steps

- **Torn first row.** In sandrail, golf, truck and both ATV captures, row 0 is torn: the poll lands
  while the state's first frame is still being written. The test seeds from the first of rows 0–2
  that steps cleanly into the next.
- **Stale camera row.** In sandrail, row 69 holds the camera exactly as in row 68 while the target has
  moved on. Row 70 is *two* steps on: replaying rider 69 and then rider 70 lands on it bit for bit.
  So the poll read the camera before its frame ran, and the camera did not skip the frame. The test
  steps through such a row without comparing it ("held").
- **ATV wrecks.** The ATV riders wreck: atv-r rows 330–397 and atv-s rows 342–417. In those rows the
  target pointer is null and SLUS-21104 runs only the base camera's frame. The look lags freeze and
  the eye drifts. The rider then respawns about 130 ft away, so the first row after the gap does
  not step cleanly. The test counts the wrecked rows and reseeds from the first clean row after them.
- **No other gaps.** Every capture's vsync steps by 2 on every row, and the near clip is 1.0 and the
  still bit (0x800) is 0 throughout.

## How it was checked

- `./kit check -- -p openmx --test mxvatv_camera -- --nocapture`: 9 passed, with all seven captures
  replayed as in the table.
- `./kit check -- -p openmx --test mxvatv_camera --test mxvatv_camera_presets --test car_camera`:
  33 passed.
- `./kit check -- -p game-sim --test camera`: 22 passed.
- Mutation: changing the look swing's −0.7 to −0.6 fails buggy at row 18.

## Not verified / not 1:1

- **The fades are barely exercised.** In every row the yaw sits near π, past both fades' upper
  angles, so the rear fade (look-ahead and lag slowness) and the swing fade are always 1. The speeds
  are also over 29.33, so the lag-tau scale is always 1. A camera swung toward the front would test
  them, and none was captured.
- **Swing is 0 in every file** (MD82a), so the aim's swing term multiplies by zero.
- **Never set in the captures:**
  - the still-camera branch (bit 0x800);
  - the short-viewport pitch term;
  - snap;
  - the reverse test's swap (no rider was standing or reversing: 0 rows under 0.0001 speed).
- **Not captured:** the spare lag (+0xd4) and the view direction; only the eye was compared.
- **The wrecked rows are not replayed.** These are the base camera's frame with a null target:
  frozen look lags and a drifting eye. Neither is the port's update; the shell would need that frame
  too.
- **Respawn:** the first row after a respawn is skipped, so the snap or jump the original makes on a
  respawn is not checked.
- **Coverage:** only 1p 4x3 on Desert Oasis free ride. There is no biplane or heli (MD82c) and no
  shell wiring (MD82d).
- **The held-row reading is an inference.** That the camera ran in the stale row's frame is inferred
  from row 70 matching two steps; it rests on that one row.
