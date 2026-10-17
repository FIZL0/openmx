# 1 — v20 drawn where the solver put it (FINAL)

Task V20. 2026-10-17.

## What I did

The report was a bike buried in the dirt to its seat with neither wheel on the ground and a rider
standing bolt upright above it, while the solver's own numbers said the bike was sitting on its
springs. The brief named three things to compare instead of eyeballing, and two of the three were
wrong.

**1. The drawn chassis origin was not `Ride::position`.** It was 2.35 ft away, every grounded frame,
for the whole ride.

A bike's animation set is nearly empty. `bike.mcf` on the `250sx` binds 12 clips: 7 of the 24 gesture
slots and **3 of the 36 pose clips** — the three `bike_air_Fwd/Mid/Bck`, all of which are the same
`bike_air_Mid.MOT`. The grounded state the shell asks for is `vehicle_anim::TURN_RIDING`, which is a
row of the pose grid, and the grid's rows are the 36 the bike has not got: the blend that comes back
is a `Blend::Four` with all four handles `None`. It writes nothing. `Set::pose` therefore keeps
whatever the last frame that *did* write put there.

Every `bike_*.mot` on this disc animates exactly six channels — `Bike`, `Handlebars`, `Shocks`,
`TireFront`, `SwingArm`, `TireRear` — and the `Bike` channel is a real one: it is the bike thrown out
from under the rider in a crossup, sweeping (−2.059, 1.133, −0.013) → (−0.065, 0.009, 0.022) →
(2.080, 1.169, −0.007). It is at the origin at the neutral point of every clip, so it cannot be
zeroed unconditionally — the stunts need it.

The shell drops the rider in with a moment of air at the spawn. Those airborne frames play
`bike_air_Mid` and leave the root channel in the pose buffer; the bike lands, every grounded frame
after it names no clip, and the whole chassis is drawn frozen at a point of the crossup — measured
live, the `Bike` node at (−1.519, 0.679, 0.006) in bike space with the skeleton rotated about −44°.
That is the bike in the dirt with its wheels off the ground. The rider was drawn correctly at the
bike's own transform the whole time, which is why it read as standing above the bike.

The fix is in `Chassis::drive`: the five drives read the live pose only when the frame's blend named
a clip, and the skeleton's **bind** pose otherwise. Not `Stance::drawn`, which is narrower and would
be wrong — `Set::draw_free` returns false for a clip that has *run out*, and that pose has to be
kept. The new `Chassis::posing` is set in `Chassis::frame` from
`game_render::chassis::poses_the_chassis`, which asks each blend variant whether any of its handles
is `Some` (and mode 0, which carries no handle, whether the motion object has a clip on).

**2. The wheels, once the chassis was back where it belongs, were on the ground.** Measured as the
lowest drawn vertex of everything under `TireFront` / `TireRear` against the contact point the
suspension's ray found: front 0.015 ft below it, rear 0.038 ft. Before the fix: front 0.303 ft above
its contact and rear 1.372 ft above it, i.e. the bike was leaning on nothing.

**3. The rider's IK targets were the bind pose's.** The shell read the chassis' four dummy anchors
once, at load, out of a comment that said "nothing in the shell animates the chassis" — true in V8,
false since V9. `Dummy_Hand_L` and `Dummy_Hand_R` are children of `Handlebars`, which the steering
drive rotates. With the bars at the `.phy`'s lock the two anchors are 0.57 ft from their bind places,
so a rider aimed at the bind pose has its hands that far off the grips — and `hold_on` cannot catch
it, because it happily puts the hands exactly on the stale target. The shell now reads them every
frame off the live pose, which is what the original does (it caches the four nodes when the vehicle
loads and reads their world transforms per frame).

New module `crates/game-render/src/chassis.rs`, which is what the three comparisons are made of and
what the shell and the test now share:

- `placed(ride) -> M4` — the one definition of the bike's transform. The shell used to build it
  inline; a test that built its own would not have been testing the shell.
- `poses_the_chassis(blend, player) -> bool` — above.
- `grips(chassis) -> Anchors` — the live read of the four anchors.
- `lowest_under(model, world, at, node, up)` — the lowest group-0 vertex whose `.psg` node is in a
  node's subtree, in world space. The tire's own geometry, not a radius out of the `.phy`.
- `Tyre` / `Seat` / `measure(chassis, model, ride, body)` — the three comparisons rolled up.

`crates/openmx/src/main.rs` now builds `bike_at` with `chassis::placed` and reads `grips` per frame
inside the draw. `PLAN.md` and `game-render/src/lib.rs` carry the new module's one line.

## What the original does

Nothing here is a port of new code: the three pieces that moved were already ported, and what was
wrong was the wiring between them.

The one behavioural claim is the fallback. The original's vehicle actor runs the same five node
drives after the blend whether or not the blend wrote anything (that is why `Chassis::drive` is a
function of its own — V9's reading), and the node transforms it writes them over are the model's own
when nothing has posed the chassis. A pose buffer in the original is per-animation-object and is
re-filled from the clips the blend names; a blend that names none leaves the object's nodes at their
`.psg` locals rather than at another state's pose. The port's `Set` keeps one buffer and overwrites
it in place, which is where the leak came from, and `Skeleton::locals` falling back per joint to the
bind local is already the port's model-local read — so driving from `Pose::new(len)` *is* "the nodes
as the model has them".

## How it was proved

**Looked right beside a screenshot** and, more usefully, **a fixture that fails without the fix**.

`crates/game-render/tests/body.rs`,
`a_frame_of_the_shells_own_ride_draws_the_bike_where_the_solver_put_it`: four seconds of the shell's
own loop in the shell's own order — `Rider::frame`, then `Body::ride` off what it wrote, then
`Chassis::frame` off the same — on a `Flat` world, with the bike dropped in from `DROP` so it is
airborne and then lands, which is the only way the stale pose is ever written. The wheel mounts are
the chassis model's own `TireFront` / `TireRear` nodes, which is what the shell hands the simulation.
It then asserts, through `chassis::measure`:

- the drawn chassis origin is within 1e-3 ft of `Ride::position` (measured: 0.0000);
- each loaded wheel's lowest drawn vertex is within an inch of the contact the solver found
  (measured: front −0.015 ft, rear −0.038 ft);
- each of the body's four anchors ends within a third of an inch of the chassis' own (measured:
  under 1e-5 ft);
- and, with the bars at the lock, the chassis' anchors have moved more than an inch off their bind
  places (measured: 0.572 and 0.578 ft) while the hands are still on them (within 0.076 ft — half a
  foot is far enough for `rider::holding` to start easing the hold off, which is it working).

Reverting only the `Chassis::drive` guard turns that into: origin **2.3496 ft** off, front wheel
0.303 ft above its contact, rear **1.372 ft** above its contact, hands **2.38** and **2.61 ft** from
the chassis' anchors. That is the bug as reported, in numbers.

`./kit check`: all green. `./kit audit`: ok.

The screenshot is `openmx --size 480x336 --seconds 7 --shot-frame 240` — four seconds into a no-input
ride, the brief's own frame: a seated rider with its hands on the bars and the rear wheel in the
dirt. Not committed (it is a render of disc data).

## Not verified / not 1:1

- **The fallback pose is reasoned, not read.** That a vehicle whose blend names no clip is drawn at
  its model's own node locals is what the port's own pieces imply and what the disc's data makes the
  only sensible reading (the grounded rows of a bike's table are empty on every bike on this disc, so
  the shape a bike is ridden in has to be the modelled one). I did not find and read the original's
  animation-object pose buffer to confirm it keeps a stale pose or not. If the original does hold the
  last pose, then the original has this bug too and the fix is a port divergence, not a repair.
- **The crossup root channel is unverified against the original.** I read the `Bike` channel out of
  `bike_crossup.MOT` and reasoned about what it is for; I did not watch the original throw the bike
  out from under the rider and compare. Nothing in this task changes how that channel is played —
  only when the pose buffer it writes into is believed.
- **The wheels are an inch out, not exact.** Front 0.015 ft, rear 0.038 ft below their contacts. Two
  different models meeting: the suspension solves a ray down the flattened ground normal and reports
  where it met the ground, while the renderer slides the wheel up the **fork**, which is raked back
  from vertical, so a compressed front wheel is drawn a little behind where a vertical slide would
  put it. Neither side is wrong and I did not try to reconcile them; the test's tolerance is an inch
  and says so.
- **The test rides a `Flat` world, not `free06`.** The terrain's own slopes would make the rake
  difference above bigger and the test's tolerance a terrain-dependent number. The shell was watched
  on `free06`; only the fixture is flat.
- **The steering is turned by hand in the fixture.** `Wheel::steer_angle` is only ever written by the
  hinge integration, which runs on the frames the bike is being balanced on; the commanded angle the
  ground step works out sits in `mv.steer_command` and **nothing in the port copies it across**, so a
  ride hands the drive a zero and the drawn bars never turn. That is a gap in the ported steering
  that this task did not come for and did not fix — the test gives `Chassis::drive` the `.phy`'s lock
  angle directly to exercise the anchors. Worth a task of its own.
- **`rider.rs` was not changed.** The brief named it as a place the bug might be; it was not there.
  `anchors_of` and `hold_on` do what V10 measured — the targets they were being handed were stale.
- **No emulator.** Nothing in this task needed one; the comparison is between two halves of the port.

## Next

Nothing outstanding for V20. The two things this turned up that belong to other tasks: the
`mv.steer_command` → `Wheel::steer_angle` gap above (the drawn bars never turn, and the rider's hands
therefore never have to follow them in a real ride), and the 33 pose-grid rows a bike archive has no
clip for, which are now drawn as the modelled shape plus the drives and are at least not a leftover.
