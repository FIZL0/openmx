# 1 — rider push uphill (FINAL)

Task S235. 2026-11-01.

## What I did

- Looked for the animation. `Rider\thq1.res` and `rider\anim\*.mot` hold turn_stop/still/riding_*,
  reverse, air, backflip, no-hander, wreck/hold, get_ready, the biplane and car steering clips, and
  the stunts. None of them is a push or paddle clip. `rider_anim::Body::select` and the vehicle
  machine have no arm that reads slope or throttle.
- Captured the original on a slow climb (`capture/`), then read the body's foot pins, fades, the
  riding request and the foot bodies' positions:
  - `tele.py` teleports the first recording's bike (state slot 8) to the foot of free09's hill at
    x 4249 (z 3715→3815, a 45 ft rise, up to ~29°) and saves slot 9.
  - `make_tape.py` writes a generated .p2m2 with Cross held at a pressure, or pulsed.
  - `prepare.py` and `capture.sh` play the tape from the state while `./kit record climb.toml` runs
    and Video Capture films it.
  - Pulsed throttle (`60:0 70:255 8:255/10:0`, 1700 fields) crawls up the face at 4.5–7.5 ft/s.
    That run is `context/capture/s235/pulse.csv`: 750 rows from frame 2993.
  - A steady 0x40 pressure (`climb64.csv`) climbs at 40–58 ft/s and shows nothing.
- `crates/game-sim/tests/rider_anim.rs`:
  - `the_rider_puts_its_feet_down_on_a_slow_climb` steps `foot_down::Feet` from each captured tick's
    request. Held pins and fades match the original's to the bit on every row. On the climb (nose up
    past ~15°, not yawing, not looking) it asserts the feet are off exactly while the speed is under
    `STAND_SPEED`. The run has 40+ ticks of each state.
  - The capture has two frames that are not one tick a row:
    - frame 3045 ran two ticks (the fade and the speed move twice as far). The test steps twice there.
    - 3244 repeats 3243 with no tick run. The test does not step there.
- Fixed `foot_down::Input::of`. It now takes the rider's packed pose, the request's `pitch` (+0x170).
  The only caller, `Rider::body_frame`, hands it `packed_pose`. Before, it passed
  `veh.mix.value`, the grid's eased **fore-aft** mix. The switch's one-foot lift (pitch > 0.7 lifts
  the right foot, < 0.3 the left, under 73.333 ft/s with up.y < 0.9) then fired on a forward lean,
  which a slow climb is likely to have. Now it fires on a sideways lean, as V36's description and
  `ragdoll_riding::request` both say. New test: `a_forward_lean_on_a_slow_climb_lifts_no_foot`.

## What the original does

- The body actor's foot block (V36: `./kit fn` on the riding body update S109 ported, the block that
  writes actor +0x638) is the whole mechanism:
  - Both pins go off (fades 0.5) when the bike is not looking (+0xf8) and either tucked (+0x16c),
    rolling back (+0x178 < 0), or under 7.3333 ft/s while yawing under 1.5 rad/s.
  - They come back on, with a 0.5 s gain fade, the tick the speed passes 7.3333.
- In the capture, the request's +0x170 and +0x174 stayed at 0.5 through the climb, so no one-foot
  lift happened there.
- Under 7.3333 the riding grid (clip 30) blends turn_stop with turn_still
  (`vehicle_anim`'s TURN_RIDING arm, band[0]). The capture's clip stays 30 and its mode 1.
- Where the let-go feet go, measured in the bike's frame against the bike's position (`veh_pos`, veh
  +0x1b0):
  - On the pegs: about (∓0.7, 1.95, −0.92) ft.
  - Let go: they drop to y 1.1–1.6 and come in to z −0.55..−0.9, i.e. up to ~0.8 ft lower, onto
    the ground of the slope.
  - They flick back each time a throttle pulse takes the bike past the stand speed.
  - From behind, the video shows the boots off the pegs, low and wide. That is the "helping it
    along".
- A let-go pin's target is not updated: `pin_target` stays where the pin let go. Measure the
  ragdoll's feet against the bike, not the target.

## How it was proved

- The live capture is `context/capture/s235/pulse.csv` (git-ignored), recorded with
  `capture/climb.toml`, which is V36's feet.toml plus `req_mix` +0x174, `veh_pos` +0x1b0 and the pad.
  `the_rider_puts_its_feet_down_on_a_slow_climb` matches its 749 ticks to the bit. It skips without
  the csv.
- The `Input::of` fix is pinned by `a_forward_lean_on_a_slow_climb_lifts_no_foot`, which needs no disc.
- `./kit check -- -p game-sim --test rider_anim --test foot_down` passes 17 tests, and the full
  `./kit check` was run too (see the commit message).

## Not verified / not 1:1

- **The drawn feet.** The original's let-go feet are ragdoll foot bodies. They hang on the leg's
  joints and rest on the slope's ground. In the port the riding ragdoll is off by default
  (`OPENMX_RIDING_RAGDOLL=1`), so a let-go foot is drawn at the clip's own pose. That is turn_stop
  blended by speed, about 1.08 ft under the pegs and wide, and it does not touch the ground. The look
  is close on flat ground, but on a slope the port's feet neither reach down to the ground nor are
  stopped by it. A side-by-side shot of the port on the same climb was not taken.
- **Each foot's lean.** The one-foot lift's lean input is now the right field. It was not captured
  live: the climb's pose stayed at 0.5. V36's own capture also never leaned sideways.
- **Two frames that are not one tick.** The test infers them from the fade's step and from a repeated
  row. It does not read the game's own tick count.
- The original's sound or other cues on the climb were not looked at.

## Next

Nothing for this task. A port that wants the feet on the ground on a slow climb needs the riding
ragdoll on (or its foot bodies alone), which is the riding-ragdoll work, not this task.
