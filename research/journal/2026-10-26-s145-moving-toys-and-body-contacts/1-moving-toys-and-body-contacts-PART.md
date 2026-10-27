# 1 — moving toys and body contacts (PART)

Task S145. 2026-10-26.

## What I did

S136 gave only path cars a `traffic::Moving`, and only the wheels read it. Now:

- **The wheel's owner branch** (`contact::other_at`, new). It works out what a wheel standing on
  another *collision object* reads: the object's cofg turned into the world, its velocity, and the
  point's velocity on its body (quaternion turn/unturn) or on its chassis (node rotate/unplace). It is
  proved, but not wired into the ride. The ride's wheel search finds only props and trees, never
  another collision object, so the owner word is always 0. Filed as S168. `ride.rs`'s
  `wheel_contact_velocity` doc says so.
- **The body contacts meet a moving prop as a mover** (the original's mesh collidable for a car or a
  toy part has `+0x80` = 0 and `+0x7c` = a vehicle of kind 2):
  - `flight_search::Scene` hits a moving prop with `Struck::mover = (prop index, 2)`. Its
    `Search::velocity` answers the prop's `Moving::velocity` (the sweep back's vt+0xb8). The new
    `Search::movers` (a trait method that answers nothing by default) lists one `contact::Mover` per
    prop (`flight_search::movers`).
  - The plane (`Flight::search` → `Listeners::movers` → `forces`), the wreck bike (`WreckBike::tick`
    → `Lists`) and the riding bike's body rays (`rider_contact::scenery` → `Bump::movers`) now
    resolve with those movers instead of `&[]`.
  - S137's car search (`mover_contact`) still names **no** mover, so its entries keep the scenery
    response. I tried naming the car (by prop index, the `inside` box too). The gate's
    `train.rs` `a_bike_standing_on_the_rails_is_thrown_by_the_trains_nose` then wrecked reason 1
    (g), not 6 (scenery): the actor response meets the car's motion and the hit is harder. With the
    mover taken out of the car search (and nothing else changed) it is 6 again. No capture of the
    original's bike meeting the train says which is right, so I kept the tested behaviour (see
    Not verified).
  - `mover_contact::wreck_contacts` drops its `mover.is_none()` filter. The original's wreck test
    reads only `+0x74` (the other's record; S37's oracle block), so a body-ray entry naming a moving
    prop as its mover is still scenery.
- **The chassis probe's velocity kill** (`ride.rs` `contacts`). When `velocity_bundle` stops on a
  moving prop (the props' sweep owned the hit and the groups did not beat it), only the velocity into
  the prop *relative to its point velocity* (`Moving::at`) is killed. With no platform the old code
  path runs, unchanged.
- **Moving toys** (`Toys::frame`). Each collision part's mesh is told
  `Moving::of(joint world before, joint world after, 1/dt)`, which is the kinematic body's update
  (vt+0x38: +0x2e0 = +0x2a0, +0x2a0 = node world) followed by the path car's virtuals, which the class
  shares. The "before" matrix is sampled again from the clock the step started from, using a copy of
  the hints, so it comes from state and a rollback cannot desync it. A finished clip stands still with
  zero motion. A part on the root (the placement) has zero motion.

## What the original does

- `./kit fn 0x0025b710`: wheel_contact_velocity. With `+0x1a0 == 0`, the owner branch
  (`+0x1f0 != 0`) comes first, then the vehicle branch (`+0x1f4` kind 2), else (0,0,0,1). The fields
  are in `research/oracle.txt`'s S145 block.
- `./kit fn 0x001dd580` / `0x001dd9b0` / `0x001dd7b0`: the kinematic body class whose instances own a
  toy's collision parts (`./kit fn 0x00119070`). Its velocity virtual is
  (node world position − `+0x310`) × `app+0x348`.
- I scanned the free09 states (first_recording, recording2, SLUS .05, .08) for the class. Each holds
  exactly one instance: the rustic windmill's collision part, on the toy's static root. Its
  `+0x2a0`/`+0x2e0` are still the constructor's identity, so its update never ran, and the virtual
  would answer the node's position × 30. s89-along, s89-across and the biplane states hold none.

## How it was proved

- **Owner branch**: function oracle. `tests/traffic.rs`
  `the_wheel_on_another_object_moves_as_the_originals_does` runs 20,000 generated wheels (half
  body, half chassis, a car on `+0x1f4` in a third) through the original 0x0025b710 in the EE
  interpreter on s89-along. +0xa0, +0x1b0, +0x1c0, +0x170, +0x180, +0x190 and +0x1a0 all match
  bit for bit.
- **Body contacts ride the platform**: `tests/traffic.rs`
  `a_wreck_bike_dropped_on_the_train_rides_it`. The real wreck bike (`bike\wreckbike.phy`) is let go a
  foot over free06's sixth car's roof at the car's velocity, among the moving props, the cars first,
  for 150 ticks. On every tick the scene's movers are one per prop, the wagon's is `Moving::mover`,
  and the wagon's sweep velocity is its `Moving::velocity`, bit for bit. 333 contacts named the
  wagon, and the bike slipped 1.18 ft along the deck at most and stayed ≤1.36 ft over the roof. The
  control (the cars' motion taken away) slid 212 ft and went off the back. This is the port against
  itself. The resolve's mover arithmetic was already proved against the original in S142
  (tests/contact.rs, tests/ragdoll_cars.rs).
- **Toys**: `tests/toys.rs` `a_toys_collision_parts_carry_their_joints_motion`. Every first_recording
  toy (their clips proved against the original in V43) gets two parts, on the root and on the last
  joint, and runs 30 ticks of `Toys::frame`. The joint part's Moving is `Moving::of` of the joint's
  world matrix either side of the step, bit for bit, and it spins on all 150 toy-ticks. The root
  part's motion is zero.
- `./kit check --all` (the gate). The first run went red on train.rs's nose test (above, fixed) and
  on `one_fps` (a frame took 83.7 ms at load 27). Run alone at load 10, `one_fps` passed with no
  change to the code, so it was a load flake. The lobby load flakes passed on their own rerun.

## Not verified / not 1:1

- **free09's windmill part is not 1:1.** In the states its kinematic body was never updated, so the
  original's velocity virtual answers ~(73,000, 4,000, 122,000) ft/s for anything touching that
  part. The port gives the part its true motion (zero, on a static root). Reason: nobody has seen
  the original fling a bike off the windmill, and I did not run a live teleport capture (a
  multi-step emulator job). Filed as S167. If the original does fling a rider there, the port does
  not.
- **The owner branch is not wired into the ride.** Proved as a function, but no wheel in the port ever
  stands on another collision object, so `under` still answers only a vehicle. Filed as S168.
- **HasPhysics props get no Moving.** They are not simulated at all, so nothing rides them or is
  pushed by them. Filed as S169. The same goes for trucks or trailers that are not path cars. I only
  scanned for the kinematic class, not for every vehicle class, so I do not know whether such
  objects exist on other tracks.
- **The chassis probe's platform kill is the port's own.** `velocity_bundle` is itself the port's
  stand-in (the original has no such probe), and the kill relative to `Moving::at` is by analogy to
  the wheels. No test drives it: it only sweeps when the body rays did not search the props (a bike
  with no collision files), which no test rides into a moving car.
- **The car search's contacts are not movers.** By S142's reading of the car's collidable (`+0x7c` =
  the car), they should be, and the body rays' entries on a moving prop now are. But naming the car
  there turns the train's nose from a scenery wreck (6) into a g wreck (1), and train.rs pins 6.
  Without a capture of the original's bike meeting the train I cannot say which is 1:1, so S137's
  scenery response stays. The car search's hits come first, and the body rays drop a ray it already
  listed, so against a path car the bike mostly still meets a still wall.
- **Toy-part movers are proved only against the port's own `Moving::of`.** The kinematic class
  shares the path car's virtuals (the vtable slots read statically), and S136 proved those for path
  cars. A toy part's virtuals were not run in the interpreter, because no free09 part had ever been
  updated, so there was nothing live to compare against.
- **The toy update's visibility gate** is still not ported (V43): an unseen toy advances and moves
  its parts here.

## Next

S167–S170 (filed; S170 is the train capture). A capture of a bike standing in the train's path would settle the car search's mover.
