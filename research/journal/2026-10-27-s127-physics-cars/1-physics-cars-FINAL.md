# 1 — physics cars (FINAL)

Task S127. 2026-10-27.

## What I did

- Found out what a `HasPhysics 1` .toy car is in the original. It is a subclass of the path car
  (S81/V41), with a rigid body attached. Until the car is struck, its update is the path car's update.
- Took out the two places that skipped these cars: `Traffic::load` in game-render `toy.rs`, and the
  world-mod dressing in openmx `world_dress.rs`. They are now driven and drawn as path cars. Free06
  loads 23 cars on clips with 0 left out, and the shell shows the company truck driving its path:
  `--track free06 --shot-pose 3650,265,3075,35,-18,60 --shot-frame 30`.
- Added names to `research/oracle.txt` (S127 block, after `path_car_vtable`) and field notes for the
  subclass's members.
- Added a new test file, `crates/game-sim/tests/physics_cars.rs` (3 tests). I also widened game-render
  `tests/toy.rs`'s spec check to cover the subclass's cars.
- Filed S185 for the struck rigid-body mode.

## What the original does

- **Constructor and fields.** The constructor (`./kit fn 0x003e3570`, vtable 0x0045f070) builds a
  0x6a0-byte object. It is the path car plus:
  - a struck listener at +0x580 and a node at +0x584;
  - the "physical" flag at +0x588 and "physical last update" at +0x58c;
  - the stop flag at +0x590;
  - the physics object at +0x698 (0x1e0 bytes; its countdown is +0x1bc, ResetTime is +0x1c0).
  The body is read from `Tracks\<track>\<model>.pco` (`./kit fn 0x003e3920`).
- **The loader reads only Car1..NumberOfCars** (`./kit fn 0x00289710`). It passes a literal 0 for the
  stop flag, so StopAnimationWithPhysics is never read.
  - free06 has two HasPhysics cars (Car22 ctruck_03, Car23 ctruck_02). Its Car24 is past the count.
  - free09 has none, so the brief's free11_redtruck there is not loaded.
  - Totals: free06 2, free07 4, free10 4, free11 4.
- **The update** (`./kit fn 0x003e3990`, vt+0x38). While +0x588 == 0 it is exactly the path car's
  update. The only addition is copying +0x2a0 into +0x2e0 when +0x58c is set, which never happens
  while the car is unstruck. While physical, the path clock still runs (the stop flag is always 0),
  then a probe gather, then the kinematic update.
- **Waking** (`./kit fn 0x003e3b00`). It is fired through the struck listener (thunks 0x003e3dd0 and
  0x003e3dc0), which any search's collide calls on what it strikes. Waking:
  - turns the body on and sets +0x588;
  - stands down the meshes +0x428/+0x42c;
  - places the body at the node, with the car's vt+0xb8 velocity and vt+0xbc spin;
  - registers +0x584 on the world node.
- **Settling** (`./kit fn 0x003e3c40`).
  - While the body is on and at rest (vt+0x118, SpeedThreshold 0.7), the body turns off and the
    countdown is set to ResetTime (10 s).
  - Once the countdown runs out, vt+0x120 runs, +0x588 drops, the meshes return, and the car is back
    on its path.
- **In RAM.** In s89-along, s89-across, biplane.p2m2 and biplane_later, both free06 trucks are
  unstruck: +0x588/+0x58c/+0x590 are 0, the body is off and the countdown is -1. Their meshes are
  filed in the plane's moving list after the 21 path cars (slots 21-22), as TOY with owner 0.

## How it was proved

All of the following is in `crates/game-sim/tests/physics_cars.rs`.
- `the_state_holds_the_toys_physics_cars`: in s89-along, the cars found by vtable scan are the .toy's
  HasPhysics models (Car22 and Car23). All are unstruck, and each holds its .pco path, ResetTime 10
  and mesh mover/owner.
- `the_physics_cars_drive_as_the_originals_do`: 300 ticks at 1/30, comparing the original's vt+0x38
  run in the interpreter with `Traffic::frame`. Every tick, bit for bit, both trucks match on: clock,
  cursor, point and last point, heading, up, position, node world matrix, mesh matrix, centre/radius,
  and the Moving velocity/spin against vt+0xb8/vt+0xbc. The car never turns physical, and the trucks
  do move and turn.
- `the_planes_world_lists_the_trucks_after_the_path_cars`: biplane.p2m2's moving list holds 23 car
  slots, path cars first and then the physics cars, with model, class, TOY category and owner 0. The
  port's trucks are filed by `world_flown` with the original's flags and radius.
- game-render `tests/toy.rs` `the_simulations_cars_are_the_originals_paths`: all 23 cars' specs
  (path cars and physics cars) match the original's keys and numbers bit for bit.
- Picture: the shell shot above shows the truck drawn on its path. I did not compare it with a shot
  of the original.

Nothing was taken from opentennis.

## Not verified / not 1:1

- **The struck rigid-body mode is not ported.** That covers the wake, the .pco body with the m.col
  rays and f.col mesh, the world node step, the settle, and the ResetTime return. A truck the bike
  hits stays on its path and pushes like the train does. The original's truck tumbles as a body,
  then returns to its path 10 s after it comes to rest. Filed as S185.
- **The struck listener and the subclass's ignore pairs are not modelled.** They only matter once the
  car can wake.
- **Only free06's trucks are proved.** There are no free07/free10/free11 RAM images. The other tracks'
  physics cars run the same proven path code, but their specs are not checked against the original.
- **The presampling physics cars** (free07's watertrucks, free10's and free11's police/trucks with
  PresampleTerrain) take V41's render-side presample, which is not proved against the original here.
- **The update order** of these cars within the toy world, relative to the rider and the path cars,
  is not checked. The planes-world list order is checked, but not the update order.
- **The picture** (draw blend, LOD, shadow) was only looked at in the port, not compared with a shot
  of the original.
- S81's caveats for path cars carry over unchanged.

## Next

S185: the struck mode.
