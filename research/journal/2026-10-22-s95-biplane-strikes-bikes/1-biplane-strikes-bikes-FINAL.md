# 1 — biplane strikes bikes (FINAL)

Task S95. 2026-10-22.

## What I did

- `flight_search`: `world_among` / `refresh_among` / `Scene::among` put the ride's bikes in the plane's
  world after the props (bike `k` at `FIRST + props.len() + k`). Each is category `BIKE` (0x9a048),
  flags ENABLED, reached from anywhere, with one listener. Its radius is `bikef.col`'s root half-extent
  length, and its centre is the root box through the chassis transform. Bikes that are in the world sit
  in the moving list after the two probes.
- The narrow phase against a bike is the prop path: the ray probe on the mesh with packed facets, and
  the sweep on the mesh's bounding sphere and placed mesh.
- The plane's record reports a struck bike as `BIKES + k`. `category()` gives that `BIKE`, so the crash
  test's VEHICLE spare applies.
- The struck listener is `bike_struck` (it keeps the striker's category in `Scene::struck[k]`).
  `struck_wreck` is the wreck test's last clause (reason 5 / `Wreck::Flat` when the striker's category
  meets the bike's mask 0x100000).
- `Flight::with_bike` (the shell loads `bike\col\bikef.col` from the archives in `main.rs`) and
  `Flight::frame_among`.
- `Sim::step` passes every other rider's bike: transform `Ride::transform()`, in the world unless
  wrecked or a ghost. Player 1's own bike is left out, because the state has the flying player's bike
  out of the world.
- `hash.rs` size pin moves 11240 → 11248 for the `Arc<Mesh>` (disc data, left out of the walk like the
  rays).

## What the original does

- `./kit fn 0x0025a480`: the bike's struck listener is `sw a2, 0x7d8(a0)`, reached through the thunk
  `./kit fn 0x0025a6e0` (a0 -= 0x80). `search_nearby` calls it as the struck collidable's listener with
  the probe as striker.
- `./kit fn 0x002522c0` ends with: if +0x7d8 and (striker category & bike +0x398), call vt +0xfc(bike, 5).
  It clears +0x7d8 either way.
- In biplane.p2m2's state, three bike collidables are among the plane's 34 candidates. Their fields are
  in research/oracle.txt under S95.

## How it was proved

All three tests are in `crates/game-sim/tests/flight_bikes.rs`.

- **Save-state capture.** For each of the three bike candidates in biplane.p2m2_SaveState, the test
  checks class, flags, reach, collides_with, one listener whose vt +0xc is the thunk, and the bike's mask
  0x100000. The radius and centre are bit-equal to `world_among` built from `bikef.col` (read off the
  disc) on the transform the collidable holds at +0x120. Both of the plane's probes are category 8, which
  `struck_wreck` never wrecks on.
- **Function oracle** (3 calls): the original listener run through its thunk on each bike stores the
  ray probe at +0x7d8. The port keeps the probe's category.
- **Ride.** A plane flown level into a wall of 8 bikes strikes one. The record keeps `BIKES + k` and the
  bike's listener keeps PROBE. It does not crash at the knock, and it stops at z ≈ 52.6, short of the
  wall at 60. The same wall out of the world is flown through.
- `./kit check` is green.

## Not verified / not 1:1

- **The plane resolves a bike as static scenery.** The original's bike collidable carries the bike's
  collision record (+0x80), so the contact is an actor-to-actor one: Other::Vehicle, the actor
  response, mutual masses, and a mirrored contact pushing the bike. The port has no Collider/record for a
  riding bike in the plane's world, so the bike neither moves nor feels it, and the sweep's "apart" rule
  for a vehicle is skipped. The follow-up is S100.
- **The struck mark goes nowhere in the ride.** Other riders' bikes are not stepped while the plane
  flies (`Sim::step`'s plane branch), and the only strikers are the plane's probes (category 8), which
  `struck_wreck` never wrecks on. So the result is identical to the original's, but the mark is dropped.
- **Bike transform = `Ride::transform()`.** I assumed this to be the chassis node's world transform the
  collidable holds at +0x120. The test reads +0x120 straight from the state; it does not prove that the
  port's `Ride::transform` equals it bit for bit.
- **The bikes' order in the moving list is player order.** I assumed this from the state's order: three
  bikes, after the probes.
- **Ghost riders' bikes are out of the world.** I did not trace this. I assumed it because a ghost
  collides with nothing.
- **A wrecked bike's collidable keeps its last transform** while it is still a candidate. The port
  places it on its ride's current transform instead.
- **The bike's own probe striking other bikes** (S54's world) is not wired to this listener. Only the
  plane's search hears it.

## Next

Done. The follow-up is S100: the plane–bike resolve as a vehicle contact.
