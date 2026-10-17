# 1 — v18 the pose the shell draws (FINAL)

Task V18. 2026-10-17.

## What I did

The shell drew the rider's body out of the **bike's** animation machine. V8 wired it that way because
that was the only machine the port had; V13 then proved the body has one of its own, ported as
`game_sim::rider_anim` in front of `game_sim::vehicle_anim`, and nothing but its own test ever called
it. This task swaps the two over, so the body is posed by the body's machine and the bike keeps the
bike's.

- **The sim hands out what the step leaves behind.** `Body::frame` throws away two answers the
  renderer needs to draw what the step just decided: the transition the clip-to-clip fade is in (V17 —
  it is wiped by the advance's end-of-frame reset, so it has to be carried out of the step, not read
  back off the motion object) and whether the put asked for a snapshot. New
  `rider_anim::Clocked { played, snapshot }` and `Body::frame_clocked`, which is where the body of the
  old `frame` moved to; `frame` is now `self.frame_clocked(..).0`, so the ported signature and S27's
  test are untouched. `Rider` carries the last one in a new `body_clocked` field, not walked (the same
  reason the animation, the body and the pool are not walked).
- **The renderer binds the sixty over the set it already has.** `game_render::rider::Body` gained
  `pose_lib: vehicle_anim::Library`, bound beside `Rig::lib` and not instead of it — two actors' clip
  tables, one set of clips underneath, so there is no second `Set` and no second parse. The gesture
  slots bind under the `stunt_name` names the motion table already loaded; the thirty-six `POSE_CLIPS`
  are looked up bare, which is how one clip the `.mcf` list omits still loads. New `Body::ride` calls
  `Rider::body_frame` with that table and then `Body::posed_by`, which rewinds on a restart or a
  snapshot and applies the blend with the carried fade. `Body::pose` stays as it was, documented as
  the bike's machine, because the eight V8/V10 tests still measure it.
- **The shell drives it.** `main.rs` keeps the pass's `Controls` on `World` (`hands`), sets
  `rider.body_rates = vehicle_params()` — the eight fade numbers V12 measured, which `Rider::new`
  leaves at zero, and at zero every fade is a hard cut — and calls `body.ride(..)` once per frame
  while riding, with a zero gate. It is a second sim entry point called from the renderer's side
  because the clip table is the renderer's.
- **The two axes had no taus.** First run of the new test put a foot 2.489 ft off its anchor; the
  blend print showed `phase`/`mix` at `-FLT_MAX`. `Lag::step` divides by `tau`, and
  `rider_anim::Body::default()` was building `vehicle_anim::Driver`'s two axes with `tau = 0` — the
  renderer's `Stance::bound` had V12's numbers, the sim's body never did. `MIX_TAU`, `PHASE_TAU` and
  `AXIS_START` moved into `game_sim::vehicle_anim` (where the record they describe is), are re-exported
  from `game_render::anim` so nothing downstream moved, and `Body::default` now builds both axes.

## What the original does

The body's selection is `./kit fn 0x00248a70` and it is the only caller of the step at
`./kit fn 0x002492b0`. `./kit xref 0x00248a70` finds no call site at all: the only references to it are
six data words, which are two actor vtables mirrored across the three address views. So the original
reaches it the way it reaches every actor — a virtual slot run once a frame with `dt` in `f12` — and
not from the bike's driver, which is a different actor with a different vtable and a different clip
table. The table is the one the spawn at `./kit fn 0x0020e4f0` hands the prefix `rider_` /
`ff_rider_` to. Two machines, two actors, one skeleton each; the port had them crossed.

## How it was proved

- **A frame of the shell's own driving.** New test
  `a_frame_of_the_shells_own_driving_poses_the_body_on_the_bike` in `crates/game-render/tests/body.rs`
  (through `./kit check`). It loads the real `bike.psg` skeleton and the real clips off the disc,
  asserts all sixty states bound, then runs 180 frames of 1/60 on `Flat` at full throttle, calling
  `Rider::frame` and then `Body::ride` exactly as the shell's loop does. At 37.4 mph the body is on
  the riding grid (`want == TURN_RIDING`, states `[30, 47, 30]`, blending
  `turn_riding_Bck`/`turn_riding_Mid`, axes at `0.5`), on the ground, not wrecked. Every one of the
  four hand and foot anchors sits within **0.154 ft** of the chassis's matching dummy — the V8 posed
  rider's own tolerance is `worst < 1.5` — against **5.050** for the same measurement in the bind
  pose, and the furthest joint is 5.014 ft off where the bind pose left it, so it is a pose and not
  the skeleton standing where it loaded.
- **The whole suite.** `./kit check` → TESTS PASSED (quoting the verdict, not the count).
- **Looked right beside a screenshot.** `./target/debug/openmx --track free06 --size 320x224
  --seconds 5 --shot-frame 180 --shot context/notes/v18-noinput.png --display :0` ran 300 frames,
  printed `rider: 24 of 24 gesture slots and 36 of 36 pose clips for its own machine`, and the frame
  shows the body posed on the bike — hands on the bars, head up. With no pad input it is in the
  gate's start-pose sequence rather than on the riding grid, which is the ported behaviour: the cut
  flag clears when the throttle opens.

## Not verified / not 1:1

- **The request block `Rider::body_frame` fills.** The port builds the body's request out of the
  ride's own state (wreck, gesture, airborne, reversing, countdown). The original reads it through a
  pointer at `+0x488` that S27 could not find a writer for, so the *mapping* is the port's reading of
  what each field must mean, not a transcription. Unchanged by this task; it is now reachable from
  the shell, which is new exposure for an unproven mapping.
- **The gate's clock in the shell is zero.** `main.rs` passes `Gate::default()` — no start-line
  countdown and no wall clock — because this shell has no start line and a zero clock keeps the idle
  draws deterministic. The original seeds the same draws from the CD clock (`./kit fn 0x0041a068`).
- **`body_rates` are V12's measured numbers, and one of them is still unmeasured.** The eight fades
  come from `game_render::vehicle::params()`; `start_phase` was never measured and stays at its
  default.
- **The two axis taus and `AXIS_START` are measured, not read.** `0.125`, `0.15` and `0.5` are V12's
  numbers off the live object. I found no constructor in the image that writes them, so the port
  asserts them in `Default` rather than porting the code that produced them.
- **`rider_anim::Body::settled` still has no writer.** The wreck arm reads it and nothing sets it, so
  a wrecked body in the port flails forever and never grabs a limb. S27 named the function that writes
  it in the original (`boot 0x0024c7e0`, the body's mode machine); that is still not ported, so the
  wreck arm of the selection is unreachable past its first state.
- **The drawing is matched by anchor distance, not by pixels.** Nothing here compares a frame against
  the PS2's own output; the test's claim is "the hands and feet are where the bike says they should
  be, and the skeleton has moved", not "this is the same image".
- **The `Sim` size pin moved 6560 → 6608.** `body_clocked` is deliberately outside the hash walk, so
  the replay hash is unchanged (leaf count still 750) and only the size tripwire in
  `crates/game-sim/tests/hash.rs` had to be told.

## Next

Nothing in this task is half-done. The two things it leaves on the table for a later one: port the
body's mode machine (`boot 0x0024c7e0`) so `settled` has a writer and the wreck family can actually
play, and find the writer of the request block at `+0x488` so the mapping above stops being a reading.
