# 1 — steep face takeoff (FINAL)

Task S179. 2026-10-29.

## What I did

- Re-ran S117's `steep` and `steep2` on current main first. The large first-airborne-tick gap the brief
  describes (the chassis' angular acceleration on the first tick both wheels are off) **is already closed**:
  steep's tick 75 has `ang_hist.y` -6.606802 on both sides, steep2's tick 98 -1.6252242 on both. Some
  change that landed between S117 and now closed it; I did not bisect which. What still parted was a few
  ulps in the **roll lane** (`z`) a tick or two later: steep tick 77 `ang_hist.z` 4.872848 vs 4.872852,
  steep2 tick 99 6.7483196 vs 6.7483163. From there the paths parted at steep 91 and steep2 112.
- Recorded my own ride of the same face, `capture/held.sh`. The gas and the stick (-0.3, uphill) are held
  from before the load, so every playback is the same ride. It saves states on the ticks around the
  takeoff: frames 2873..2879 in `context/capture/s179/` (`steep.csv`, `steep-<frame>.p2s/.ee.bin`; the
  tele start is `context/states/s179-steep.eeMemory.bin`). That ride parts the same way: bit-exact through
  the takeoff, then `ang_accel.z` 3.9208436 (original) vs 3.9208395 (port) on the 2877 tick.
- `the_steep_face_takeoff_tick_is_the_originals` (replay.rs) runs the original's whole vehicle update one
  tick from each of the 7 images in the oracle, beside the port's frame seeded from the same image. It
  compares the quat, cofg, vel, both spins, the angular acceleration and its history, the node rows, the
  origin, both compressions and the crossup. Before the fix 2877 parted in `ang_accel.z`; the other six
  agreed on every lane.
- Hooked `bike_airborne_control` on those images and ran the port's `air::airborne_control` on exactly
  the inputs the original's call saw. Its moment and force matched to the bit on every substep call. So the
  air control itself is right, and the part had to be in its inputs. The one input that differed is the
  **roll** (and pitch) it reads from chassis `+0x98`/`+0x90`. The 2877 image stores euler
  `[-0.25197572, 0.43931645, 0.4423991]`, while `quat_to_euler` of the image's own quaternion gives
  `[-0.25197572, 0.43931615, 0.4423988]`. The original's own `quat_to_euler` (ee.call) gives the same as
  the port's, so the angle routine is not the cause.
- Hooked `chassis_measure` as well. On all 7 images it runs **once** a frame, from the attachment walk
  after both substeps' air control (return address in the base frame update). Every air control call
  reads the image's stored angles. The original has no frame-top measure.
- The fix (ride.rs `Ride::frame` step 2): the frame-top `quat_to_euler` + `chassis::measure` is gone. The
  ground step and air control now read `chassis.euler`, the angles the last frame's after-physics measure
  (S138's 4b) took. The port's frame-top recompute read the quaternion after the stunt clocks' copy-back
  (S67's `mat4_to_quat` of the node's matrix), which can be an ulp off the integrated one that the
  original's measure took them from.
- `Ride::place_on` (spawn, put-back, the tests' throws) now sets `chassis.euler` from the quaternion it
  sets. Without this, the first frame after a placement read the angles of the pose before it, and
  `roof_landing`'s flip wrecked on tick 1.
- `the_air_control_reads_the_last_frames_euler` (replay.rs) is the permanent proof of the original's
  order: one measure, after the air control, and air control reads the stored angles.
- S58 ratchets (`S58_CASES`): turn, launch, brake, traverse and steep now agree on path and slip flags
  to their captures' last tick (`u64::MAX`). Without the fix (ride.rs at HEAD, same tests) the paths part
  at launch 177, brake 106, steep 91; turn was already whole on main. steep2's path now holds to 188
  (parts at 189, was 106) and its slip flags agree to the last tick (were 133). The own-state branch of
  the test learned the same `u64::MAX` = every tick rule the other branch had.
- MD25's ride hashes did not move once `place_on` takes the pose's angles.

## What the original does

- `./kit dis 0x0015c4c0` (chassis_measure, vtable +0x38): `quat_to_euler(sp+48, chassis+0x1a0)`, the
  result copied into `+0x90`, `|vel|` into `+0x34`, then a tail call to the base class' frame update.
- In the bike's vehicle update it is reached only through the attachment walk after the air state
  (S138). The hook trace above shows nothing calls it before the ground step. So the pitch and roll the
  next frame's ground step and `bike_airborne_control` (oracle.txt) read are the ones taken off the
  substeps' quaternion, *before* the stunt clocks write the node's quaternion back.

## How it was proved

- A save-state capture plus a bit-exact oracle comparison. 7 RAM images of one ride
  (`context/capture/s179/steep-2873..2879.ee.bin`): the original's `bike_vehicle_update` one tick in the
  ps2_oracle `Ee` beside the port's frame (`the_steep_face_takeoff_tick_is_the_originals`), every lane
  equal on all 7.
- Hook traces of `chassis_measure` and `bike_airborne_control` on the same 7
  (`the_air_control_reads_the_last_frames_euler`).
- S117's two traverse captures replayed from their RAM images (`the_slip_cases_replay_from_the_save_state`):
  steep agrees to its last tick, steep2 to 188.

## Not verified / not 1:1

- **What closed S117's original first-airborne-tick gap is unknown.** It was already gone on main when
  S179 started. Not bisected, because the task's evidence was the replay, and the replay was already
  green there.
- **steep2 still parts at tick 189** (`ang_hist.prev.x` -1.23 vs 0.72, a large part, not an ulp). Not
  looked into, because it is past the task's ask (steep past 90). Filed as a follow-up.
- **`place_on`'s angles are not the original's measured behaviour.** I did not capture what the
  original's put-back, spawn or teleport leave in `+0x90`. The choice keeps placements as they rode
  before S179, which every placement test pins. A real put-back in the original may read the pre-placement
  angles for one frame.
- **Other writers of the quaternion outside a frame** (ragdoll tests' hand-set `chassis.quat`, netplay
  restore) keep whatever `chassis.euler` they had. A restore copies the euler with the rest, so it is
  consistent. The hand-set test poses are not re-measured; their tests pass.
- **The tick-1 stick of a state loaded with the stick already held.** On my ride, the original's first
  tick after the load rode a centre stick with the state's buttons, not the pad's -0.3. Shown only on this
  one ride. The S58 test harness does not model it, so `s179/steep` is not an S58 case; it is used only by
  the two per-image tests above.
- `research/journal/.../capture/held.sh` depends on S67's `record_saving.py` and S106's tele state.
  It was run once here (KIT_PCSX2=1) and not re-run from a clean slot.

## Next

- Follow-up S218: steep2's part at 189.
