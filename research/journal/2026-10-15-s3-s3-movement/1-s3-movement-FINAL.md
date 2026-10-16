# 1 — s3 movement (FINAL)

Task S3. 2026-10-15.

## What I did

Ported the **spine of the dirt bike's ground frame** — everything `bike_ground_step` (`0x00257a00`)
does itself — into `crates/game-sim/src/move.rs`, and proved it against the original in
`crates/game-sim/tests/move.rs`.

That is the four things the brief asks for, plus the thing that ties them together:

| the brief | here |
|---|---|
| turning | `steer_scale` (the lock fading out between two speeds) and `steer_command` |
| acceleration | `road_rpm`, `shift`, `set_gear`, `clutch` (the launch ramp) |
| friction | `drag`, the only force the step applies against the direction of travel |
| maximum speed | `drag` against the engine, with `engine_post`'s rev limiter on top of the gear `shift` picked |
| the update order | `step`, the substep loop, which returns the ordered list of everything it called |

Not ported: the twenty-one callees the step drives (the suspension ray, the tire forces, the torque
curve, the roll and pitch controller, …). They are listed under **Not verified / not 1:1** with a
reason each. The step returns them as an ordered `Vec<Call>` carrying the substep `dt` each one was
handed, which is how the *order* is proved even where the callee is not — and is the seam the next
task ports them behind.

## What the original does

`./kit fn 0x00257a00` (9.3 KB of callees behind it; read them one at a time).

**The frame, before the loop.** Chooses the pose. Balancing on one wheel (`bike+0xa48`): the centre of
gravity comes from `BalanceCofG`, the front wheel is told to hold itself up (`front+0x3c = 1`) and the
chassis is flagged `(1, 0, 1)` through `chassis_set_flags` (`0x0015c3a0`). Riding: the centre of
gravity comes from `CofG`, and `z` is shifted by the stick — forward by `CofGTravelForward` and back
by `CofGTravelBack`, then the brake adds `BrakeCofGTravelForward` and the clutch's weight adds
`ClutchCofGTravelBack`. Then the steering (`front+0x8ac = MaxSteerAngle * (lean * steer_scale)`), the
donut's roll scale onto `rear+0x44`, both wheels' spring rates, the gearbox and the clutch.

**The substep loop.** `n = (int)(55 * dt) + 1` substeps of `h = dt / n`. Each one: gravity into
`chassis+0x130`; six controllers; the drag; the airborne control; the reverse push; both wheels
cleared; the contacts; both suspensions; both tire forces; both wheel integrations; the rev limiter;
the collision record's `begin`; the chassis' acceleration through the transform's transpose into the
g-load peaks; and the chassis' own `integrate` (vtable `+0x3c`).

**Three details worth the ink.**

- `bike_wheel_contacts` is handed the **frame's** `dt`, not the substep's — `f23`, not `f22`. Every
  other call in the loop gets `f22`. It is not a transcription slip; `move.rs` says so at the call.
- The drag's scalar is `(-Drag) * ((0.5 * speed) * speed)`, applied along the normalised velocity with
  `vmulx.xyz` and accumulated with `vadd.xyz`. Ghidra prints the multiplies in a different order
  (`-Drag * 0.5 * spd * spd`), which is a different `f32`. Same for the road RPM, which is
  `9.549296 * (ratio * (speed * invRadius))`, and for the steering, which is
  `MaxSteerAngle * (lean * scale)` and not `(MaxSteerAngle * lean) * scale`. **Confirm every multiply
  and accumulate order against `./kit dis`.** Two of the three only showed up as a 1-ulp difference in
  the whole-step proof.
- `engine_update` reads `f20` before writing it. The ground step sets `f20 = 0.0` before the loop, so
  the value is 0.0 — not indeterminate.

**The `.phy` keys.** `bike_phy_load` (`0x002539e0`) maps the file's text keys onto the bike object.
Every parameter `move.rs::Params` carries, with the key it comes from:

| key | offset | note |
|---|---|---|
| `CofG` | `+0x460` | quadword |
| `BalanceCofG` | `+0x480` | quadword |
| `CofGTravelForward` | `+0x698` | |
| `CofGTravelBack` | `+0x69c` | |
| `BrakeCofGTravelForward` | `+0x6a4` | |
| `ClutchCofGTravelBack` | `+0x6a0` | default 0.0 |
| `SteerSpeed1` / `SteerScale1` | `+0x678` / `+0x67c` | |
| `SteerSpeed2` / `SteerScale2` | `+0x680` / `+0x684` | |
| `MinDonutSpeed` / `MaxDonutSpeed` | `+0x530` / `+0x534` | both scaled by `1.4666501` on load |
| `MinDonutAngle` | `+0x538` | `* -0.017453292`, i.e. negated into radians |
| (derived) | `+0x53c` | `-3.1415927 - (+0x538)` |
| `DonutRollAngleScale` | `+0x540` | default 1.0; not read by this step |
| `SpringWhenStopped` / `Spring` | `+0x6f4` / `+0x6f8` (front), `+0x6fc` / `+0x700` (rear) | read back out of `wheel+0x54` after the wheel's own setup, so they arrive in the chassis' units |
| `ClutchTorqueScaleTime` | `+0x6c8` | |
| `MaxClutchTorqueScaleSpeed` | `+0x6d0` | |
| `MaxClutchTorqueScale` | `+0x6d4` | |
| `UpShiftRPM` / `DownShiftRPM` | `+0x740` / `+0x744` | |
| `ReverseSpeed` / `ReverseGain` | `+0x544` / `+0x548` | |
| `Drag` | `+0x6a8` | |
| `Gravity` | `+0x450` | quadword |

`MaxSteerAngle` and `TireRadius` belong to the wheel (`wheel+0x7c`, and `1/radius` cached at
`wheel+0x838`).

## How it was proved

`crates/game-sim/tests/move.rs` — six oracle proofs, all of them comparing **every word** of the
bike (`0xa80`), the chassis (`0x2c0`), both wheels (`0x8c0` each), the engine (`0xc0`) and the
collision record, not a list of fields: a field the original writes and the port does not model shows
up as a difference. Plus `written_ranges()`, so a write outside the object graph fails the test.

| proof | against | n |
|---|---|---|
| the clutch's launch ramp | `bike_clutch` `0x002572f0` | 10,000 generated frames |
| the gearbox | `bike_set_gear` `0x00246f70` | 10,000, including both out-of-range gears |
| the rev limiter | `engine_rev_limit` `0x00247260` | 10,000, half of them put near the limit |
| the per-substep wheel clear | `wheel_clear` `0x0025b200` | 2,000, both wheels |
| **the whole ground step** | `bike_ground_step` `0x00257a00` | 2,000 generated frames |
| **a 240-frame scripted ride** | the same, stepped 240 times in one piece of memory | 1 fixture |

The whole-step proof hooks all twenty-one callees, installs the chassis' `integrate` and the collision
record's `begin` as fake vtable slots, and stubs the two transform functions (each records the vector
it was handed, which is the port's own arithmetic and so is compared, and hands back the vector the
harness chose). It then compares the recorded call trace against the `Vec<Call>` the port returns, and
the object graph word for word.

The replay fixture is the one that carries state: the original's bike is stepped 240 times in the one
piece of memory, so the clutch ramp, the gear, the moved centre of gravity and the g-load peaks of
frame 117 are whatever frames 1 to 116 made them. A per-frame state that *happened to be reachable*
is not the same test as a state that was actually reached.

Both step proofs assert their own coverage: balancing and riding, one substep and several, the reverse
push with and without a chassis transform, and donut and no donut, each at least 100 times.

No emulator was needed, so none was started.

## Not verified / not 1:1

**The twenty-one callees.** Each is hooked in the proof, so the step's own arithmetic and the order
and `dt` of every call to them is exact, but none of their bodies is ported and none of what they
write is modelled. They are the rest of S3 and the body of S10's ground half:

- `bike_stand_upright` `0x002574e0` — the low-speed upright controller.
- `bike_balance` `0x002561f0` — the balance-on-one-wheel controller.
- `bike_roll_pitch` `0x00256470` — the roll and pitch control the rider's lean feeds.
- `bike_preload` `0x00256940`, `bike_backflip` `0x00256b50`, `bike_airborne_control` `0x00256e50` —
  S10's, called from here too.
- `bike_wheel_contacts` `0x00257820` — distributes the frame's collision contacts onto the wheels, and
  the one call that gets the frame's `dt`. It is what sets `wheel+0xdc`, so in both proofs the wheels
  are out of contact from the second substep on, and the reverse push (which needs the rear tire down)
  is only exercised on the first.
- `wheel_suspension` `0x0025b280` — the suspension ray and its spring force. The *rate* it is handed
  is ported (`spring`, written to `wheel+0x54`); the ray is not.
- `wheel_tire_forces` `0x0025bc70` — the drive, brake and side forces. The donut's roll scale and the
  `other_tire_down` flag this step hands it are ported; the forces are not.
- `wheel_integrate` `0x0025bbf0` — the wheel's own spin and steering integration.
- `bike_no_hander` `0x00255d20` — the no-hander. The four accumulators the step zeroes for it are
  ported (as `Ride::no_hander_scratch`, by offset: a guessed name is worse than an offset).
- `engine_update` `0x00246fd0` — **the torque curve**, the clutch's slip and the torque that reaches
  the wheel. The largest single omission: the clutch's torque *multiplier* is ported, the torque it
  multiplies is not, so this port cannot yet accelerate a bike on its own.
- `transform_rotate` `0x001b2ff0` / `transform_unrotate` `0x001b30e0` — the chassis' orientation. The
  two vectors they produce are inputs to `Step` here, and the proof feeds the same two to both sides.
- `actor_search_reset` `0x00119be0` / `actor_search_gather` `0x00218b80` — S2's, called at the tail.
- The chassis' own `integrate` (vtable `+0x3c`) and the collision record's `begin` (vtable `+0x80`) —
  both virtual, both another object's; the call and its argument are recorded and proved.

**Smaller things.**

- `bike_phy_load` `0x002539e0` is not ported. The key→offset map above is recovered and proved by
  reading, not by running the loader; the proofs write `Params` directly.
- `Fresh`'s seven scalars and three quadwords are kept by offset and not named, because what writes
  them (the suspension and the tire forces) is not ported. `wheel_clear` is proved against the
  original, so the set is right and the order is right; the names are absent, not wrong.
- `Chassis::speed` and `Step::speed` are inputs here. The chassis computes them, and the chassis is
  not this task's; the replay fixture scripts the speed ramp the same way for both sides.
- `chassis_set_flags` `0x0015c3a0` is proved only as part of the step (the three words it writes are
  inside the compared region), not on its own.
- The whole-step and replay proofs are the ones that matter, and they run 2,000 and 240 frames, not
  the 20,000 substeps S10 managed: a frame here is up to five substeps of twenty-one hooked calls, and
  the snapshot is 2,000 words. Nine seconds for the file. Raise it if a change here ever looks
  suspicious.

## Next

The callees are three tasks now, so ticking S3 does not lose them — in this order, because that is
the order the step calls them in and the order the freeroam path needs them:

- **S15** — the engine's torque curve (`engine_update`). First, because it is the one omission that
  stops the port from accelerating a bike at all, and `clutch`, `shift` and `engine_post` are already
  here waiting for it.
- **S16** — the wheel: `wheel_suspension`, `wheel_tire_forces`, `wheel_integrate`. This is also where
  `Fresh`'s ten fields get their names.
- **S17** — the three bike controllers and the no-hander.

Each of them slots in behind a `Call` variant that is already proved to be reached in the right place
with the right substep `dt`, so each one's proof is the callee's own arithmetic and nothing else.
