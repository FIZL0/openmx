# 1 — foot down (FINAL)

Task V36. 2026-10-24.

## What I did

- Found why the drawn rider never puts a foot down: **the clip already does**. The pose grid's stopped
  row `turn_stop_*` (row 24), which `vehicle_anim` blends in under the grid's first band (7.3333 ft/s,
  weight remap(speed, 0, 7.33, 1, 0) against `turn_still_*`), has both feet about 1.08 ft under the
  pegs and planted wide (probed off thq1's clips). The shell's stand-in hold (`rider::hold_on`) pinned
  both feet to the pegs at `Body::hold` (1 while Riding), so the clip's foot-down never showed.
- The original's riding ragdoll lets go of its two foot pins (+0x5d8, +0x5dc of the body actor) at a
  stand. That switch was already ported as `ragdoll_body::Actor::feet` (S109). I lifted it out as
  `game_sim::foot_down::Feet`, which needs no ragdoll, and fed it in `Body::ride` from the ride
  (`Input::of`: speed local_vel.z, yaw ang_vel.y, the vehicle animation's state and mix, the chassis
  quat). The draw now holds each limb at its own strength: `rider::hold_limbs` with `Body::holds()` =
  hands at hold, feet at hold × the foot's weight while held, 0 once let go. `palette`, `skin_into_at`
  and `chassis.rs`'s `held()` all use it.
- `ragdoll_riding::{STAND_SPEED, SLOW_SPEED}` and `Actor::feet` made `pub` for the tests.
- Tests:
  - `crates/game-sim/tests/foot_down.rs`:
    - `the_foot_switch_is_the_body_actors`: 24 000 generated ticks, `Feet` against `Actor::feet`, bit
      for bit.
    - `the_original_lets_its_feet_go_where_the_switch_does`: `Feet` stepped from the original's own
      captured request, against its foot pins.
  - `crates/openmx/tests/foot_down.rs` `a_stopped_bike_puts_a_foot_down_and_moving_off_lifts_it`: the
    shell's frame + `Body::ride` + `palette`. It rides, brakes to a stand, stands and rides off. The feet
    come off on the tick the speed crosses 7.3333 and are >0.9 ft under the pegs standing, with the hands
    still within an inch of the grips. They go back on the crossing tick moving off, at weight 0.75–0.8,
    and are back on the pegs within an inch once the 0.5 s fade has run.

## What the original does

- The body actor's update (`./kit fn` on the riding body update S109 ported, the foot block that
  writes +0x638) decides, every riding tick, from the riding request (actor +0x488):
  - **Both feet go**: not looking (+0xf8), and either tucked (+0x16c), rolling backwards (+0x178 < 0),
    or under the stand speed (+0xa14, 7.3333) while yawing under 1.5 rad/s (+0xa0 .y). Both pins go
    off and both fades (+0x638, +0x63c) are set to 0.5.
  - **One foot goes**: otherwise, in the riding grid (clip +0x8f4 == 30), under the slow speed (+0xa20,
    73.333), with the bike's up.y < 0.9 and the request's pitch (+0x170) > 0.7 (right foot) or < 0.3
    (left foot). That foot's pin goes off and its fade is set to 2, and the other is held at gain 1.
  - **Otherwise** both are held, and each fade runs down by dt. The pin's gain (+0x100 .z) is
    ramp(fade, 0, 2, 1, 0): back from 0.75 after a stop, from 0 after a lift.
- No separate foot-down clip and no blend of its own. The blend in is the pose grid's own
  turn_stop/turn_still mix by speed. The blend out is the gain fading back over 0.5 s.
- Live, free09 (first_recording's state):
  - Riding off, braking to a stand, backing up and riding off. The foot pins went off on the tick the
    request speed dropped from 8.43 to 6.85, stayed off through the stand and the reverse (speed down
    to −8), and came back on at 8.05 with gain 0.7666 (fade 0.4667).
  - Stopped on flat ground (tilt 0.6°) and on a 13.7° slope: both pins off, the feet's bodies 0.7–1.1 ft
    under the pegs. Shots: `context/shots/v36/flat.png`, `right.png` (slope), `left.png`.
- **The request's speed is the bike's signed forward speed**: on the capture it equals bike_vel ÷ 0.8
  projected on the bike's forward axis, to the capture's read-phase noise (e.g. 53.814 vs 53.814, 0.091
  vs 0.091, −8.004 vs −7.999), and it goes negative backing up. This is what the port feeds
  (`chassis.local_vel.z`). The writer is still unfound (S111).

## How it was proved

- **A PINE recording compared tick by tick**: `context/capture/v36/feet.csv`, 570 consecutive ticks
  (frames 2864..3433), made by `capture/run.py` (spec `capture/feet.toml`, pad `capture/stop.pad`).
  `Feet` stepped at the original's 0.03333 s tick from each row's captured request matches the
  original's pins' on flags, both fades and the held pins' gain to the bit on every row: 247 let-go
  ticks and 3 switches. The switch reads its own tick's request (lag 0). With a one-tick lag, 3 rows
  differ.
- **Function oracle, generated inputs**: 24 000 ticks against `Actor::feet` (S109 proved that one
  1203/1203 against animandpump's run). This covers both thresholds, tips up to 70°, both lifts
  (1387/1402 ticks), stops (7711) and fades (12 038).
- **Shell behaviour**: the openmx test above, on the disc's thq1 and 250sx.
- **Looked right beside screenshots**: `context/shots/v36/port-flat.png` (the port, free09 flat at
  4620,2320, braked to a stand: both feet down and wide) beside the original's `flat.png`, where both
  feet are down and wide too. `port-slope.png`: a stop after a wreck and get-up, feet down.

## Not verified / not 1:1

- **The one-foot lift (slow + tipped + mix past 0.7 / under 0.3) is not proved live**: the captured
  request's pitch stayed 0.49–0.57, so the original never lifted one foot on this run. It is proved
  only port-to-port against S109's actor switch, which S109 proved against the original. The capture
  that would prove it is a slow, hard lean.
- **The foot does not follow a slope in the port**: the original's foot bodies are ragdoll bodies
  that collide with the ground. The stand-in draws the clip's foot where the clip puts it, in the
  bike's frame, with no ground under it. On the 13.7° slope the original's feet were 0.92/1.04 ft
  under the pegs. Waits on S111 turning the riding ragdoll on (V63 deletes the stand-in).
- **A pin's gain used as the stand-in's aim strength is approximate**: in the original the gain
  scales an impulse on a physical body. In the port it blends the aim solve. The on/off ticks and
  the gain's values are exact; the drawn in-between is not.
- **Request speed = `local_vel.z`** matches the captured request only to the capture's read phase,
  not bit for bit: its writer is unfound (S111).
- **Looking and tucked are not fed** (`Input::of` leaves both false). The original's look-around
  holds the feet whatever the speed (row 42 of the capture: clip 35, looking 1). The shell has no
  look-around request yet.
- **Online play does not step the feet**: online bodies are posed by `posed_by` and never call
  `Body::ride`, the same as V61's hold. Filed as a follow-up.
- **The port's slope shot is not on a measured slope**: the tape's right turn wrecked and got up
  before the stop, and I did not read the port's tilt there.
- `crates/game-render/tests/body.rs`'s fixtures place the tires by tire-force mounts and tumble the
  chassis. The openmx test uses `stock::mounts` instead. Not touched (not mine).

## Next

Landed.
