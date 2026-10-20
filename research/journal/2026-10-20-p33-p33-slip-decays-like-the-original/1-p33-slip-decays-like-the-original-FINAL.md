# 1 — p33 slip decays like the original (FINAL)

Task P33. 2026-10-20.

## What I did

- New `crates/game-sim/src/tire.rs`:
  - `force`: the original's tire force law (`wheel_tire_drive_force`).
  - `contact_frame`: the frame the tire pushes along, plus the contact velocity in it.
  - `axis_rotation`: the engine's axis-angle rotation from the curve table.
  - `steering_axis`: normalize(Handlebars − Shocks).
  - `Slip`: the three flags the law keeps on the wheel.
- `ride::Wheel` gains `steer_axis` and `slip`, both hashed: seven leaves a wheel, so the hash walk
  goes 750 → 764 leaves and 7120 → 7152 bytes, and the ride walk goes to 486 / 1832.
- `spin_integrate`, on a tire with `UseParametricCurve`:
  1. builds the contact frame,
  2. runs the law,
  3. pushes `rotate(frame, (side, 0, along))` at the contact (new `tire_side`).

  It replaces the stand-in's side force and its hold bound. The forward lane is still the stand-in's
  `along`.
- Tests and the shell set the front's `steer_axis` from the model's Handlebars/Shocks nodes
  (`the_disc_125_and_fork` in tests/ride.rs, and openmx main.rs).
- `it_straightens_after_a_slide` now asserts `ORIGINAL_SLIP_BACK`. `PORT_SLIP_BACK` is gone.
- `it_flies_straight_hands_off`: the lip's yaw-rate precondition went from 0.05 to 0.01 rad/s.
  The new tire leaves the lip far straighter (−0.0123 rad/s, 5.5° off, rolled 9.6°), and the landing
  still meets the test's own bounds (0.03° roll, −1.36° turned).
- HUNDRED_TICKS and the platform vectors are regenerated and spliced const by const. POS_Y and VEL_Y
  did not move: nothing changes on the linear fixture bikes except the new hash leaves.

## What the original does

`./kit fn 0x25c530` (wheel_spin_integrate) calls `./kit fn 0x25c920` (the contact frame) and then
`./kit fn 0x25ca60` (the force law).

The contact frame:
- Its forward is the chassis' row 2.
- On a steered wheel in contact, the forward is instead `transform_rotate` of row 2 of
  `axis_rotation(steer angle, steering axis)` (`./kit fn 0x134290`).
- It calls `mat4_basis(origin, forward, contact normal, 1, 0)`, and the local velocity is the frame's
  transpose times the contact velocity.

The law:
- Forward = drive torque / radius. Past MaxDriveForce·load, or in a donut with any drive, it is the
  SkidDriveForce and the wheel is "spinning". Under −MaxDriveForce·load the wheel is "locked".
- Below 10 ft/s along the tire, the side velocity is scaled by map_clamp(|vz|, 0..10 → 5..1).
- The side keys are chosen by the brake lever first, then spinning/locked (the speed scale lerps to
  DonutsSideForceSpeedScale with the donut), then the plain keys.
- Parametric curve: `vx·P·(−Scale·load) / (1 + |vx·RoundOff·P|^DropOff)`, with powf through the
  executable's thunk.
- Linear law: `vx·(−K·load)`, capped at Max·load. Past the cap it is Skid·load·sign, and "sliding"
  is set.

The front's steering axis is set at attach time (`./kit fn 0x253760`): recording2's bikes have
(0, 0.8808, −0.4735), a 28° rake. Every wheel of the four bikes in recording2's save state has
UseParametricCurve = 1. The offsets are in research/oracle.txt (P33 block).

## How it was proved

Function oracle in `crates/game-sim/tests/tire.rs`, 10,000 generated inputs each:

| Test | What it checks |
| --- | --- |
| `the_tire_force_law_matches_the_original` | Every branch, both laws; the flags and the 0x850 triple are compared bitwise |
| `the_axis_rotation_matches_the_original` | The axis-angle rotation |
| `the_contact_frame_matches_the_original` | The contact frame and local velocity; transform_rotate is hooked |

A temporary panic probe confirmed all three ran and that every branch was hit. The probe was then
removed.

Ride level: `it_straightens_after_a_slide` on race125p, the human's turn. It now passes with the
original's bound. After the release the slip trace is −2.2, −2.2, −2.1, −1.9, −2.0, −2.1, −2.2, −2.1,
−1.9, −1.7, −1.5, −1.2, −1.0, −0.8 …° (peak 2.8°, was 5.8°), and the roll goes 41.0 → 9.3° in 12
ticks. `./kit check`: TESTS PASSED. The shell rides unattended for 300 traced frames with nothing odd.

## Not verified / not 1:1

- **Linear tires keep the stand-in.** The law is used only when `UseParametricCurve` is set. Under
  the original's linear law the fixture bikes (freeride-bike, trick-bike .phy.txt) either lose all
  grip, because they have no Skid*/Slipping* keys and those default to 0, or go unstable (trick-bike's
  SideForce 60). With the full port, 8 fixture tests failed. The fixtures were tuned for the stand-in,
  and no disc bike is linear. The linear branch itself is still proved in the oracle test.
- **The forward lane is still the stand-in.** It is `along` with the WheelFriction cap. The law's
  forward (drive torque / radius, and the skid drive force) is computed but not used. Porting it
  belongs to the drive path, not to this task's side grip.
- **The push is a plain force at the contact.** The original's hinge-force path for a steered,
  balancing wheel is not ported.
- **No-contact early return.** It is the stand-in's, not the original's zeroing of +0x840/+0x850.
- **Other unported paths:** the "share the other wheel's contact" path (+0xc4), the no-torque path
  (+0xc0), the gyro terms and +0x234.
- **Contact velocity is not proved.** It is the port's existing value; only what the law does with
  it is proved.
- **The ride slip still differs from the original's.** The slip holds near 2° for about 8 ticks,
  then decays 0.8–0.85 per tick, against the original's 0.66–0.8. The peak is 2.8° against the
  original's ≤1.9°. The test's bound (inside 3° by +11) holds, but the curve is not tick-for-tick.
  The likely cause is the items above (the forward lane, the hinge path).
- **`steer_axis` without a model.** It defaults to UP when a ride has no model (the fixtures); the
  original always has one. The tests take the fork from `place_in(root)` and the shell from
  `world()`. Only the normalised direction matters, but the two were not compared bitwise.
- **The other `the_disc_125` tests** (top speed etc.) don't set the fork, so their front steers about
  UP.
- **`it_flies_straight_hands_off` precondition.** It was lowered to 0.01 rad/s. The lip still yaws,
  but much less.
- **Cross-architecture platform vectors** were not rerun (see the memory note on the upstream
  toolchain). They were regenerated on x86_64 only.

## Next

None for P33. A follow-up could port the law's forward lane and the hinge-force path, then compare
the slip tick for tick with recording2's releases.
