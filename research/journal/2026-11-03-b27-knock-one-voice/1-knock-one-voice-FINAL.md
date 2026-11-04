# 1 — the knock is one voice because its speed is an impulse (FINAL)

## The sound object does not drop repeats

material_hit_play (boot 0x003ea900) ends with the material's sound object's vt+0x30 (volume,
+0x3c), vt+0x5c (position, +0x70), sound_3d, and vt+0x1c play(4). In the class at vtable 0x0044ed00,
vt+0x1c is the start at 0x166380. It does not stop the voice in +0x34 and always allocates a new one.
Nothing on the way (vt+0x68's 3-D update included) checks the age (+0x28) or the old handle. So if
play ran on 2903 and 2904, two voices would sound. It does not run on 2904.

## What the RAM images show

f27a1's states 2903–2906 (`context/states/s195-f27a1-290{3..6}.eeMemory.bin`, copied from s3).
Table at game_manager+0x40 = 0x00e17d90, material 21's entry and its sound object:

| image | ball shock peak (body+0xb0) | bike chassis +0x170 | voice +0x34 | age +0x28 | level +0x3c |
|---|---|---|---|---|---|
| 2903 | 16.809237 | 0.84046185 | 0x4f | 0 | 67 |
| 2904 | 0 | 0 | 0x4f | 0.03333 | 67 |
| 2905 | 0 | 0 | 0x4f | 0.06666 | 67 |
| 2906 | 0 | 0 | 0x4f | 0.09999 | 67 |

The queue is swap-removed and cleared only over [0..count), so the last update's entries stay
readable past the count. On 2903 its stale speed word is bit-equal to `speed(ball shock peak)`.
16.81/25 × 100 → 67 is the level in +0x3c, and the position is the ball body's +0x60. The owner
read is `owner +0x80 → +0xf4` (body) `+0xb0`, else `+0xf0` (chassis) `+0x170`. The oracle already
had the offsets but called the value |vel|. The shock peak is the per-axis largest |shock|, cleared
at preforce. The impact accumulator is the ground step's, emptied at the top of every frame. On
2904 the pair is still queued, but both impulses are under MinImpulse (5 for material 21), so
nothing plays and the 2903 voice runs on.

## The port

- `HeardRide::owner` (openmx main.rs): a prop's and a plane's speed is `Body::shock_peak`, and a
  bike's is `Ride::impact`.
- `Ride::impact` is the frame's `mvc.accel` (the port's +0x170) saved after `frame_searched`. It is
  sound only and wrapped in `material_hit::Impact`, which, like `Heard`, compares equal and prints
  as nothing, so snapshots and hashes leave it out. It is skipped on the wire. Size pins moved:
  Ride 2496 → 2512, Sim 12208 → 12224.
- `material_hit::play` is unchanged. Its `Hit::velocity` field keeps its name and now carries the
  impulse (its docs say so).

## Checked

- `tests/material_hit.rs a_knock_sounds_once` builds the pair from each image's stale queue (ball
  probe → bike frame) with the image's impulses and runs `material_hit::play`. 2903 gives one sound,
  level 67, at the ball, and matches the stale speed bits, the level and the position. 2904–2906 give
  nothing, with the handle and position unchanged and the age growing.
- game-sim material_hit, prop_knock, ride, replay, hash, snapshot: 230 pass. openmx
  material_hit_sound: 5 pass. `openmx --tests` builds.
- research/f27a1/ours.sh + `match.py --pair` on the port's ride (scratch run, not kept). F27a1's
  port run found bbhit at NCC 0.92–0.94 as two or three voices. Now the best peak is NCC 0.145,
  which is noise: no stacked voices, but no voice either.

## Why the port's knock is now silent (B28)

The port runs the props' search (hearing) before the bike's frame, then the props step. So the
ball's probe hears the bike a tick after the strike, when its shock peak is 0 again. On the port's
strike tick, the ball's peak is 15.4 and the bike's impact 0.77, close to the original's 16.8/0.84.
They are just read on the wrong tick. Filed as B28.

## Not verified / not 1:1

- **The hearing order** (B28): the port's knock does not sound until it is heard on the strike frame.
- **Stale shock peaks on sleeping props.** If a prop sleeps and skips preforce, its +0xb0 may hold
  an old peak. Whether the original clears it then, and whether the port's `shock_peak` matches, was
  not checked.
- **The plane owner.** It reads `Body::shock_peak` by the same rule, but no capture of a plane hit
  was compared.
- **The bike's impact** is `mvc.accel` after the frame's last substep, and it was checked on one
  image only (0.84 vs. the port's 0.77 a tick off, B28). That it is exactly +0x170's value bit for
  bit was not checked.
- tests/prop_knock.rs still feeds velocities to `play`, and its docs (and "the bike's always
  stayed") were left as they were. They test the queue's rules, not the speed's source.
- Only material 21's knock was looked at. Other materials' MinImpulse against impulses were not
  re-tuned or re-heard.
