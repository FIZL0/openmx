# 1 — camera through the backflip (FINAL)

Task P34. 2026-10-20.

## What I did

The brief said only that the camera is wrong while backflipping. I rode a backflip headless and
filmed it with the shell's chase camera. Halfway up the flip the view turned through about 180°
(the yaw went −179.5…+179.99), and the eye ended up as much as 7.8 ft *ahead* of the bike, looking
back at the rider. It swung round again near the landing. That is the bug: the camera flips over in
front of the bike.

The cause is one inverted flag. The race camera heads along the velocity when its target's `+0xb0`
is set and the speed is over 20; otherwise it heads along the bike's forward row. The basis follows
the same rule with a speed of 5. S24 named `+0xb0` "grounded", and the shell handed it
`ride.air.on_ground`. The original stores the opposite: the rider is in the **air**. So in the shell,
the camera headed along the forward row through every jump. On an ordinary jump that is nearly the
velocity anyway. A backflip turns the row over, and the camera went with it.

Changes:

- `game_sim::camera::Ridden::grounded` → `airborne`, and its docs. The arithmetic is unchanged: the
  oracle test in `crates/game-sim/tests/camera.rs` writes the same bits to `ACTOR+0xb0` and still
  passes bit-exact. The generator is `airborne: !g.one_in(4)`, the same bits as before.
- `crates/openmx/src/chase.rs` (new): builds the shell's target block, `chase::ridden` for whichever
  machine is up and `chase::bike` for the bike, with `airborne: !ride.air.on_ground`. The biplane
  gets `!plane.on_ground()`. `main.rs` calls it instead of building `Ridden` inline.
- `crates/openmx/tests/ride.rs`: `the_camera_stays_behind_the_backflip`. It rides the existing
  `it_flips_off_the_lip` flip on race125p.phy and runs `camera::Race` on it with the shell's
  settings: free06_2's `cpresets.ini` off the disc, "Race Cam 1", snapped, speed reference
  117.3328. It asserts that the eye stays more than 5 ft behind along the velocity, and that the
  view turns less than 10° on every airborne frame after the spawn drop. It then films the same ride
  with the flag inverted (the old wiring) and asserts that the view turns more than 90°, so the test
  would have caught P34.
- `research/oracle.txt`: S24's steps 5 and 9 now say `airborne`. A P34 block describes the target
  writer.
- `capture/` beside this entry: the watch spec, the play script (C6's, with the spec as `$6`) and
  `flipcam.py`, which reads the capture.

opentennis: nothing taken. A camera-target flag is specific to this game.

## What the original does

The race camera's actor at `cam+0x8c` is not the vehicle. It is a small component at rider + `0xdd0`
(`./kit fn 0x00279ed0`, the type-2 ctor). The vehicle refills it once a frame in
`./kit fn 0x0027c580`, and only while `(rider+8) & (rider+0xdd4)` is set. It holds:

- the seat point `+0x820`;
- the forward and up rows of the chassis node `+0x18`;
- the velocity `+0x840`;
- the lean `+0x854`;
- the speed `max(+0x860, 0)`, the chassis speed;
- the flag, written as `sltu` then `xori 1` of `+0x93c`, i.e. `on_ground == 0`. `+0x93c` is the
  snapshot of `+0x574`, "either wheel touches" (V9). The flag is therefore *airborne*.

On the first airborne frame the writer also keeps the takeoff position and velocity at
`+0xe60/+0xe70`. Their reader was not looked for.

## How it was proved

- **The decompilation**: the `sltu`/`xori` pair in `./kit dis 0x0027c580`.
- **RAM**:
  - first_recording's state has the bike on the ground: `+0x93c` = 1 and `+0xe80` = 0.
  - recording2's biplane state has the plane on its wheels: `+0xe80` = 0.
- **A capture of the original**: the human's recording2 played on a copy, KIT_PCSX2=5, with
  `capture/p34cam.toml`. It covers frame_counter 2375..3443, 1070 rows, written to
  `context/capture/p34/recording2-camera.csv`, which is not committed.
  - `+0xe80 == !(+0x574)` on all 1070 rows.
  - The recording has two backflips. Through each one, `render_camera`'s view stays close to the
    *velocity* while it is about 180° off the bike's forward row:

    | flip (frames) | view off the velocity | view off the forward row | eye behind the bike |
    |---|---|---|---|
    | 2500..2610 | ≤ 1.5° | 179.7° | ≥ 13.31 ft |
    | 3284..3430 | ≤ 0.3° | 179.8° | ≥ 14.59 ft |

  - The human recording's md5 is unchanged (1e5091236f40915ad1e5942bce20287d).
- **The port**: `the_camera_stays_behind_the_backflip`, and `./kit check` passes in full.
  - Fixed: the eye stays 17–20 ft behind, and the view turns about 0.05° through the flip.
  - Old wiring: the view turns about 180°, and the eye ends up ahead of the bike.

## Not verified / not 1:1

- **Nobody looked at the windowed shell.** The proof is the headless test plus a capture of the
  original's render camera. "Matches by eye" was not done by a human. The two cameras behave the same
  way (behind, along the velocity), but no one looked at a picture.
- **Not the same jump.** The original was captured on the human's recording2 backflips (Free09). The
  port's test flies the synthetic ramp of `it_flips_off_the_lip`. The free06 flip lip was not ridden
  on the original, because there is no pad here and a generated .p2m2 drives straight lines only.
- **The biplane's flag** rests on one save state at rest (flag 0 on its wheels) and on the camera
  code being shared. The biplane's own target writer was not read.
- **The body point**: the original uses the seat position `+0x820`, and the shell uses
  `ride.position()`. This predates P34 and was left alone.
- **The lean** (`+0x854` → `held.lean.0`) was not checked against the capture.
- **How far behind**: the follow distance and filter lag were not compared numerically. The original
  sits 13–15 ft behind in the air, and the port's test sits 17–20 ft behind, but on different jumps,
  presets and speeds.
- **Not modelled**:
  - the writer's gate `(rider+8) & (rider+0xdd4)`; the shell refreshes the target every frame;
  - the takeoff snapshot `+0xe60/+0xe70`.

## Next

Nothing for P34. If a human rides a backflip in the window and the camera still looks wrong, the
next suspects are the follow distance and the body point (seat versus chassis), from the list above.
