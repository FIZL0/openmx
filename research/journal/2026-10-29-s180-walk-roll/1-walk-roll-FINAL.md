# 1 — walk roll (FINAL)

Task S180. 2026-10-29.

## What I did

- **Ee-stepped roll's flight.** I rode first_recording's save state on the Ee one frame at a time,
  as S106's `roll` capture rode it: `bike_vehicle_update(rider, dt)` per row, with that row's held
  controls written to the control state and the gas (`+0x63c`) and clutch (`+0x644`) levers poked
  from the row before. Row 0 uses gas 0 and clutch 1. The pad's gesture step is outside the vehicle
  update, so writing the controls alone never moves the levers. This reproduces the capture's
  angular history bit for bit through row 68 (tick 69). Capture row r is frame counter 2775 + r,
  which is port tick r + 1.
- **The parting.** In the air only `bike_airborne_control` (`./kit fn 0x00256e50`) adds chassis
  torque. Its roll term is `z += UnrollTorque·(UnrollGain·roll − ang_vel.z)`, with roll read from
  `chassis+0x98`. At tick 67 `ang_vel.z` was the same in both (`0xbe19e28b`), but roll was
  `0xbd64caed` (−5.5857588e-2) in the original and `0xbd64cd5e` (−5.5859916e-2) in the port. So the
  torque differed: substep 1 gave −2.2611647 in the original and −2.2615721 in the port. **The cause
  is which quaternion the angles are taken from.** Hooking `chassis_measure` (`./kit fn 0x0015c4c0`)
  showed that the original's last measure in a frame is the one inside the post-physics attachment
  walk. That walk runs *before* the stunt clocks copy the node's quaternion back into the chassis.
  Nothing measures again after that, nor at the top of the next frame. The port's `ride.rs` step 2
  took `quat_to_euler` again at the frame top, from the copied-back quaternion. At tick 66 the two
  quaternions differ by 1–2 ulp per lane (walk: `[1044531402, 3164603374, 1022829041, 1065037440]`;
  copy: `[…401, …373, …039, …440]`), and the roll moves by 2e-6 rad.
- **The fix** (`crates/game-sim/src/ride.rs`). Step 2 now keeps `chassis.euler`, the angles last
  frame's walk (step 4b) took, and passes them to `chassis::measure`. `Ride::place_on` sets
  `chassis.euler` from the attitude it sets down, so a bike placed fresh does not carry the angles of
  wherever it was before. It uses a new shared `acos_table()`, since `place_on` has no `Tables`.
  `air.rs` is unchanged: its reads were correct, but the roll it was given was wrong.
- **Row 148 is a late read.** With the flight fixed, roll next parted at tick 148, by 0.12 ft of
  origin and 0.5 ft/s of velocity at once. The original's row 148 is word for word its row 149, and
  the port agrees with row 149. This is the same pattern as row 70 (S118). I added it to roll's
  `late`.
- **S106_CASES roll: path 131 → 240, slip 240.** Every lane of roll now agrees on every compared
  tick of its 241. Side and roll2 are still exact. Nose still parts at 180, and it parts identically
  with the fix reverted (checked), so this change did not cause it.
- **Proof test** `the_airborne_control_reads_the_walks_roll` (tests/replay.rs), the Ee loop above,
  with addresses taken by oracle name:
  - the angular history `z` equals the capture's on rows 0..=68;
  - at every frame's end, `+0x90..+0x98` equals the port's `quat_to_euler` of the last quaternion
    the chassis measure saw;
  - the copied-back quaternion measures differently on 32 of those 69 rows, row 65 (tick 66) among
    them. So the test can tell the two readings apart.
- research/oracle.txt: an S180 line under the attachment walk's note.
- Gate: `./kit check --all` green (3086). No hash, vector or fold moved. The kept and retaken angles
  differ only by ulps on some frames, and `air.rs` (and the wreck test's pitch and roll) are the only
  readers, so no generated ride's pinned bits changed.

## What the original does

`bike_vehicle_update`'s riding branch runs the physics substeps, then `bike_air_state`, then the
attachment walk (`./kit fn 0x00133500` on obj+0x50). The walk calls each child's vt+0x38, which for
the chassis is `chassis_measure`: `quat_to_euler(+0x1a0)` into `+0x90..+0x9c` and `|vel|` into
`+0x34`. Then the stunt clocks copy the node's world rows back into `+0x1a0` as a quaternion. The next
frame's `bike_airborne_control` reads pitch `+0x90` and roll `+0x98` as the walk left them, from the
pre-copy quaternion.

## How it was proved

- **Ee per-frame run from a RAM image**: `tests/replay.rs`
  `the_airborne_control_reads_the_walks_roll`, 69 frames of first_recording's state against S106's
  roll.csv. It runs, not skips, in s3 (it prints the rows where the copy parts). While finding the
  cause I also used a temporary trace (removed) that hooked every callee of the ground step and the
  air control at tick 67 and printed torque and roll per substep, as in S107's method.
- **Save-state capture replay**: `the_braked_slopes_replay_from_their_save_states`, where roll now
  agrees on every lane of every compared tick (241 ticks, rows 70 and 148 skipped as late).
- The full gate.

## Not verified / not 1:1

- **`place_on` taking fresh angles** is the port's own choice. I did not read whether the
  original's respawn or teleport path measures before the next frame. A bike set down by the port
  starts from its new attitude's angles rather than stale ones. No capture covers a frame right
  after a placement.
- **The Ee loop pokes the levers** from the capture's previous row instead of running the pad's
  gesture step. The proof covers the vehicle update, not the lever path. It agrees with the capture
  bit for bit, but the levers are the capture's.
- **Row 148's "late" verdict** comes from its words matching row 149 exactly, as row 70's did, not
  from reading the recorder's timing.
- **Nose still parts at 180** (S118's note, unchanged by this fix). Not looked into.
- **S177's S58 turn** (an `ang_hist.y` parting at 173) could share this cause in principle, but its
  capture `context/capture/s58-slip/turn.csv` is not in s3, so the case skipped here. The gate's pins
  held wherever it runs.
- **The captures and image live only in s2 and s3.** Elsewhere the new test and the S106 cases print a
  line and pass.

## Next

Done here.
