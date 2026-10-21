# 1 — ride drives the ragdoll (FINAL)

Task S65. 2026-10-21.

## What I did

- **Found the settle gate's writer.** `Down::settles` is now the body's touching flag (S61's
  `Down::touching`, which the ported wreck arm writes). The rider reads it at the top of the frame,
  before `Down::step`, because the original's machine reads what the previous tick's body tail left.
  `state::Sense::settles` docs updated. S53's "hands off and a bone touching" is gone.
- **The bike sphere for off-bike.** `ragdoll_ride::bike_sphere(col)` reads `wreckbikef.col`'s root
  node: its centre, and the length of its half extent through the FPU for the radius.
  `ragdoll_ride::bike_centre(frame, centre)` places it the way the collision object does:
  translation first, then x, y, z. `Kit::bike` holds the sphere (`Kit::new` leaves it `None`; the
  shell and `tests/ragdoll.rs`'s `ride_kit` set it). `Down::step` now hands the wreck arm the
  sphere, carried with the wreck body's frame, so `off_bike` sets in a ride. Nothing else in the
  step reads `off_bike` (only the field, its reset and the snapshot wire), so this changes no motion.
- **The skeleton's pose and rests.** `ragdoll_ride::Posed` carries each part's node in the bike's
  frame plus its rest. `Down::enter_posed` and `Down::step_posed` use it when given one and fall
  back on the Rig's fixed pose otherwise; `enter`/`step` are the `None` case. The rider has a
  `skeleton: Option<Arc<Posed>>` input. It is not walked by the hash and not sent over the wire;
  `decode` keeps the machine's own, like `ragdoll_kit`. game-render adds
  `Body::ragdoll_posed(&Ragdolled)`, the same per-part math as `ragdoll_rig` over the pose
  `Body::ride` last wrote. `openmx` sets `rider.skeleton` after each `body.ride` and loads
  `wreckbikef.col` into the kit by suffix.
- `tests/hash.rs`: the Sim size pin is now 9744 (+8 for the `Arc`).

## What the original does

- The rider body actor's request pointer (`actor+0x488`) **is the human's vehicle + 0xb80**. The
  vehicle's vtable 0x454ef0 sits at its +0x48. So the request's +0x1bc is the vehicle's +0xd3c, the
  ride machine's settle gate. The actor's tail (`./kit fn 0x0024e250`) ends with
  `[actor+0x488]+0x1bc = actor+0x48c`, the touching flag.
- Only two things write touching: the actor's constructor (`./kit fn 0x002472c0`, sets it to 0) and
  the wreck arm of `./kit fn 0x0024c7e0` (any bone collision touching, 0/1). It therefore keeps its
  last wreck's value while riding: s26's capture has the gate at 1 after a respawn.
- Order inside a tick: the player update `./kit fn 0x0027c890` runs the base update 0x002520d0
  (physics, then the ride machine 0x0027bc90 at its end), then 0x0027c2d0 fills the request. The
  rider is index 8 in the actor list, the bike index 2, so the rider ticks later. The machine thus
  reads the gate the previous tick's tail wrote. It latches only past its 0.5 s clock (2.5 s for
  reason 13), which `state.rs` already had.
- The off-bike test reads the collision object at `[request+0x1ac]`, the wreck bike's (its name
  string reads "Wreckbike.col"). Its +0x60 radius is 0x4089b63b, |root half extent| chopped (a
  host float gives 0x4089b63c). Its +0x40 centre is the root node's centre (file offset 284)
  through a copy of the request's frame (request+0x10..+0x4c, also at `[object+0x18]+0x118`),
  translation first.

## How it was proved

- `crates/game-sim/tests/ragdoll_gate.rs`, over S56's captures and states:
  - `the_settle_gate_is_the_bodys_touching_flag`: in all four captures (front, side, back, tip;
    >1500 rows, riding through to rest), the gate word inside the captured request (`req_area` at
    0x200 + 0x1bc) equals `a_touching` on every row. The test also asserts some rows have the gate
    on with `a_hand_hold > 0`.
  - `the_body_actors_request_is_the_bikes`: in all ten s56 RAM images, request − 0xb80 has the
    vehicle's vtable, and its +0xd3c equals the actor's +0x48c.
  - `the_bike_sphere_is_the_wreck_bikes_root_node`: in the same ten images, `bike_sphere` of the
    disc's `wreckbikef.col` matches the object's radius bit for bit, and `bike_centre` of the
    request's frame matches its centre bit for bit. A Python search over all 24 operand orders
    showed translation-first is the one order that holds in every state; `transform_point` is 1–2
    ulp off.
- `tests/ragdoll.rs`:
  - `a_wreck_is_thrown_from_the_shells_skeleton`: given the kit's own pose as the live one, the
    posed entry and 10 steps equal the fixed path exactly. A pose lifted 0.5 ft starts the main
    bone 0.5 ft up, and other rests change the rates.
  - S53's wreck2 test (over its `Slope` ground) now settles on tick 17; the original settles on 18
    and S53's inferred gate gave 16. Its hands-before-gate assertion is dropped.
- `./kit check` passes. The shell (`--track free06 --trace-gas --trace-stick-y 0:0,229:1
  --seconds 18`) loads the kit and hands over the skeleton on all 540 frames without a fault.

## Not verified / not 1:1

- **The ride's wreck tick by tick against the original.** The new pieces are proved one by one
  (gate writer, gate timing, sphere bits). The ride's whole wreck with them in place has not been
  compared to a capture. wreck2 settles a tick early (17 against 18) over a four-height ground
  with the fixed riding pose, so the exact-tick test stays ignored.
- **The gate's one-tick lag rests on the update order, not on a tick-by-tick capture of it.** S26's
  two captures disagree by a row: wreck.csv has the gate at 3036 and the latch at 3037, wreck2.csv
  has both at 3032, so one of them is torn. The order was read from the player update and the actor
  list.
- **Touching starts false in every `Down`.** The original's flag keeps the previous wreck's value
  until the wreck arm first runs. This is harmless here, since the latch also waits out the 0.5 s
  clock and the arm rewrites touching on its first tick, but it is not 1:1.
- **The live pose is host-float and a frame old.** `Body::ragdoll_posed` is the renderer's f32 math,
  like `ragdoll_rig`. It is also the pose drawn after the previous tick: the sim steps first, then
  the body is posed. When the original's wreck entry reads its skeleton within the tick was not
  checked. The bones still start with the bike's velocity and no spin, where the original takes
  both from the skeleton's last frame.
- **Only the local shell hands the skeleton over.** Online sessions (the `online.is_none()` gate
  around `body.ride`) and headless/replay paths leave it `None`, so they wreck from the fixed pose.
  A snapshot keeps the skeleton the machine already has, so a rollback re-steps with the current
  pose, not the historic one.
- **The step's rests come from the shell's last pose for the whole wreck.** This was not checked
  against the original's skeleton through a wreck (S61's replay lays in the captured rests, which
  were not compared with the shell's pose).
- **`bike_centre` uses the ride's `quat_to_mat4(crash.body.quat, crash.origin)`.** It is proved
  against the original's request frame, but nothing shows that the port's wreck frame equals that
  frame.
- **`off_bike` has no effect yet.** Nothing in the port's step reads it, so setting it in a ride
  changes nothing visible.
- **The shell was not seen in a live wreck.** The smoke run did not visibly wreck: it prints
  nothing per wreck, and no keystrokes reach the window here.

## Next

Follow-up S66 (filed with `./kit plan add`): start the bones with the skeleton's velocity and
spin, from the previous two `Posed` frames.
