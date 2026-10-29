# 1 — frame start euler (FINAL)

Task S177. 2026-10-29.

## What I did

- Ran the turn first. On main its path no longer parted at 178; the first lanes to part were at 173:
  `ang_vel[2]` (0x3ed8b11f against the port's 0x3ed8b11d), `ang_hist.z` by 2 ulp, and the quaternion
  history. That is not the brief's `ang_hist.y`/`ang_vel[1]`. Something that landed after S116
  moved it.
- Put a temporary print of the chassis torque after each contributor in the ground step's substep
  loop. At 173 the bike is in the air with no wheel down, and the whole torque comes from
  `air::airborne_control`. That tick it switches from the damped branch (172) to the auto-level
  branch (`x` 0, `y` -1163, `z` 119). `y` agrees and `z` does not.
- Read the auto-level in the original (`./kit dis 0x00256e50`, from 0x002570b0 on). It is
  `m.z + unroll_torque * (unroll_gain * chassis+0x98 - ang_vel.z)`, the same `mul.s`/`sub.s`/`add.s`
  chain the port has. So an input differed, and the only input `z` reads that `y` does not is the
  roll, chassis `+0x98` (euler `z`).
- The port's frame took the euler angles twice: once at the frame's start (old step 2, off the
  quaternion the stunt clocks' copy-back had just written) and once in step 4b (S138, off the
  orientation the substeps left, before the copy-back). I printed both at the start of 173. They
  differ by 3 ulp in `z` (0x3f2e1934 against 0x3f2e1931), and `y` by 5.
- Tried the ground step on the 4b angles. All four of s2's S58 captures (turn, launch, brake,
  traverse) then agreed on all 112 lanes on every tick. Before, launch had 88 lanes parting from 75,
  brake from its landing at 106, and turn from 173. With s1's fixtures (`GAME_SIM_FIXTURES=<s1>/context`,
  which also has S117's steep pair) steep agreed on every tick (it was 90/250). steep2 parted at 189
  only, on every lane at once, and 190 on agreed again: row 189 holds tick 190's ride word for word,
  a late read.
- Checked the original for the second measure. It isn't there. `./kit xref 0x0015c4c0`: the chassis
  measure has no direct caller and is reached only through the chassis vtable's `+0x38`.
  `./kit dis 0x00133500` is the generic attachment walk (each enabled child's vt+0x38). The vehicle
  update's riding branch (`./kit dis 0x002520d0 0x002521c0`) calls the ghost step (vt+0x13c), the
  input limiter (vt+0xec, `./kit dis 0x0027ffd0`, which calls only 0x00251a50, and that calls
  nothing), the ground step, the air state and then the walk 0x00133500. Nothing before the ground
  step measures the chassis. The oracle note on `chassis_measure` ("Called before the ground step")
  was S19's reading and was wrong. I corrected it.
- Ported it. Step 2 of `Ride::frame` now reads `chassis.euler` as step 4b left it and measures nothing.
  The frame-start speed is the 4b measure's too, which equals the old one wherever nothing writes the
  velocity between frames. `Ride::place_on` (respawn, put-back, teleport, `Ride::new`'s placement)
  now takes the euler angles of the orientation it sets (`placing_acos`, a lazily built table). Without
  that, the first frame after a placement would read the attitude the bike was taken from. Doc
  comments in ride.rs (the frame list, step 2, 4b, the copy-back) and chassis.rs (`measure`, `euler`)
  follow.
- Ratchets raised. `S58_CASES`: all six cases to `u64::MAX` on path and slip, with steep2's row 189
  marked late in `S58_OWN_STATES`. The own-state branch now uses the same `past` test as the other
  branch, so `u64::MAX` can be asked there. `S106_CASES`: nose and roll to every tick. Both had one
  more late row each, nose 180 and roll 148, each the next tick's words exactly, now in their `late`.
  Roll's tick-67 roll rate (S118's open finding) was the same ulp source and is gone.
- `./kit check --all`: game-sim green with no hash pin moved (HUNDRED_TICKS, platforms.rs vectors and
  the rest held). openmx's `original_mode` failed only because Bash has no DISPLAY (memory note "gate
  needs a display exported"). `./kit done` runs with DISPLAY exported.

Nothing from opentennis: it has no bike chassis.

## What the original does

- One chassis measure a frame (`./kit fn 0x0015c4c0`: quat_to_euler into `+0x90`, the velocity's
  length into `+0x34`, then the base walk). It runs from the vehicle update's attachment walk after the
  ground step and the air state, and before the stunt clocks write the node's orientation back into
  the chassis' quaternion. The next frame's ground step reads `+0x90` pitch and `+0x98` roll as that
  measure left them, so they are angles of the substeps' quaternion and not of the copy-back's.
- The non-riding branch of the vehicle update (`+0x80c == 2`, 0x00252144) calls the walk without a
  ground step, so a bike in that state is measured every frame.

## How it was proved

- The replay harness against save-state captures, every lane bit for bit:
  `tests/replay.rs the_slip_cases_replay_from_the_save_state` (turn 202 ticks, launch 242, brake 201,
  traverse 201 in s2; steep 251 and steep2 251 with s1's fixtures, the steep pair's RAM images and
  csvs being only in s1) and `the_braked_slopes_replay_from_their_save_states` (nose 201, side 201,
  roll 241, roll2 422). 112 lanes each, 0 parting outside the four late rows.
- The call order is read from the ELF (above). The fix was not proved by ee-stepping tick 173 from a
  save state, as the brief suggested. Every captured lane on ten captures agreeing after the change,
  and none before, is a stronger proof than one stepped tick, and it needed no new state.

## Not verified / not 1:1

- **`Ride::place_on` measuring the placed orientation is not proven.** In the original, the
  put-back, respawn and teleport set the chassis somewhere new. Whether the next ground step then
  reads fresh angles (some measure in the placement, or the non-riding branch's walk running in
  between) or the stale ones was not traced or captured. The port measures, so a placed bike is
  never read at the attitude it left. No capture here crosses a placement.
- **The frame-start speed is no longer re-measured.** This matches the ELF, but it was checked only
  where nothing writes the velocity between frames. For another rider's bump of this chassis
  (`rider_contact::bumped_one`) and `Ride::launch`, the original's behaviour between frames was not
  captured. `launch` sets the speed itself, as before.
- **steep and steep2 were only checked with s1's fixtures** (`GAME_SIM_FIXTURES`). s2 has no
  steep captures, so the gate in this slot skips those two cases.
- **The late rows** (steep2 189, nose 180, roll 148) are identified by their words equalling the
  next tick's port state on every lane, not by the recorder's timing.
- **A ride that spends frames in the non-riding branch** (`+0x80c == 2`) is measured each frame in
  the original. Whether the port's equivalent state runs a measure was not checked.
- The copy-back's node can differ from the substeps' orientation by far more than an ulp: off
  free09's south edge (`--trace-at 4620,500,180`, full throttle) the first ticks' roll is 0.0116 rad
  apart. The ground step now reads the 4b angles there, as it does where the captures prove it, so
  that ride runs differently. Only the S58/S106 captures back this, and they are a few ulp apart,
  not 0.01.
- smooth_picture's shootoff test failed in the gate: its two tests wrote one `throttle.tape`, and
  when it played the 150-tick ride's 180-frame tape, the throttle ran out at 180. Main's ride still
  wrecked long enough on that; this one is down 76 frames. Each ride now writes its own tape by
  length. On a 630-frame tape it is down 727 of 1201 frames.

## Next

Nothing. The S58 and S106 ratchets are at every tick.
