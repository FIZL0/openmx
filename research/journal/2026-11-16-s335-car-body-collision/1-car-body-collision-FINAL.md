# 1 — car body collision (FINAL)

Task S335. 2026-11-16.

## What I did

- Found that the buggy's and the truck's body collision against the terrain already works in
  game-sim: dropped on their roofs onto free07, the collision object holds them up. They stayed
  stuck because the car's stuck timer raised `CarEvent::Reset` and nothing acted on it.
- Ported the original's stuck reset as `Machine::put_upright` (crates/game-sim/src/car.rs). The car's
  post runs it when the measure's wreck timer answers `Reset`, as the original's post runs its wreck
  step after its measure.
- Gave the car the knockable props, as the plane has them (S241):
  - `Collision::search_props` (kiowa.rs): the props' meshes in the body's search after the world.
  - `Machine::sort_contacts_knocking`: the same meshes in the tires' sort, so a tire drives on a prop
    as it does on a scenery mesh.
  - `Machine::step_props` and its `knocked`: struck props woken with their mirrors naming the car,
    the props' own rays on the car's hull (`f.col`), the struck bodies pushed by the car's resolve
    after the riders' bikes, then written back.
  - `flight::knocks` / `Posed` made `pub(crate)` so the car poses the props the plane's way.
- Shell (crates/openmx/src/vehicle.rs): `CarRide::step_traffic` holds the props out of the traffic's
  frame, steps the car among them, then runs the props' own step with the car as striker 0, then the
  traffic's late half, as `Plane::step_traffic_shared` does. `Garage::drive_met` and
  `drive_lane_met` call it in place of `traffic.frame` + `step_met`.
- Tests: `car_drive.rs` `a_car_on_its_roof_is_put_back_on_its_wheels` (buggy and truck, free07: the
  reset fires, clears the speeds, and the car settles on its wheels). New `car_props.rs`
  `a_car_knocks_a_ball_along_its_way` (buggy and truck drive at free06's first ball: it wakes, is
  pushed along, and the car's centre never reaches it).

## What the original does

- The car's wreck step (`./kit dis 0x00398820 0x00398c00`), from the post after the measure: a whole
  car near the map's edge is thrown (wreck 13); a wrecked one unwrecks once it lifts and lands; one
  touching something with the gas on and under 10 ft/s counts a clock, and past `MaxWreckTime` calls
  car vtable +0x19c and zeroes the clock.
- vt+0x19c (`./kit dis 0x00396f50 0x00397120`): the terrain query at the body's position gives a
  ground point P and normal N (through an app object that hands back a lift of 0, taken off along
  N). The quaternion is built from the car's last forward and N (`axes_to_quat`, not re-crossed, so
  a nose that was tilted leaves the turn a little off square). The body's position becomes
  P + ((−0.5·N) + turn(quat, cofg)): the model's origin half a foot under the ground, which is about
  where the wheels' bottoms are. Then body vt+0x34 (`./kit dis 0x001f7910 0x001f7958`) sets
  velocity, force, rates, moment and twist to (0,0,0,1). The last position and last turn are left.
- The body's rays search the terrain only when the car's up leans 45° or more off the ground's
  normal (`searches_terrain`, already ported). Under that, the tires alone hold it up, so the hull
  can dip into a slope. That is the original's rule and is kept.

## How it was proved

- The reset: read instruction by instruction from the disassembly; the turn is the same ten VU0
  operations as `chassis::turn` (checked line by line). Run in the roof test above. No capture.
- The props: the plane's proven S241 path reused piece for piece. The car test shows the wiring
  works (wake, push, no pass-through). No capture.

## Not verified / not 1:1

- The reset is not compared against a capture of the original resetting a car (C34): no recording of a
  car on its roof exists. Order of operations follows the disassembly; values are unproven.
- The reset's turn uses `math::basis` + `math::mat4_to_quat`, which S205's `axes_to_quat` proves for
  the plane; that the car's call takes the same path is read from the disassembly (same callee
  0x0013f9f0), not run.
- The reset runs after the whole measure. The original's post may run other listeners (vt+0x2e0
  and the three calls after it) between the measure and the wreck step; their effect on the frame
  the reset lands on is not checked.
- The car among the props: no capture of the original's car striking a prop exists, so the search
  order, the struck bodies' order in the resolve, and the prop distances (taken where they stand
  now, `Scene::knocking`, not as last frame's gather left them) are the plane's, assumed to be the
  car's.
- The props' rays on the car's hull use the car's collidable category and the collider's mask, as
  the plane's use its own. Not checked against the original.
- Online seats that share one prop set (S259) are not given the car's props path: a car lane
  steps against its seat's own copy, as before (it did not strike props at all before).
- The edge throw (`CarEvent::Thrown`) is still not acted on in the shell; filed as S336.
- Not watched on screen in the shell: no pad here, and the task asked for the simulation fix.

## Next

Nothing for this task.
