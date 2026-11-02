# 1 — struck trucks (FINAL)

Task S186. 2026-11-02.

## What I did

- **`crates/game-sim/src/physics_car.rs` (new).** `PhysicsCar` is one car's physics object. It is a
  one-object `prop_world::Collection` (the wreck bike's world, always active), plus the car's
  `physical` / `was_physical` flags (`+0x588`/`+0x58c`) and its last pose (`+0x2e0`).
  - `load(pco, psg, m.col, f.col, edge)`: the part comes from `[Physics Properties]`. The inertia
    comes from the model's span (`model_size`). The body comes from `WreckBike::new`, with the
    collision object on the `m.col` rays. It starts put away.
  - `wake(car, last, 1/dt)`: switches the body on, sets the car physical, and places the object at the
    model node (`node_point(cofg)`, the car's head and up, written back squared). The car then stands
    at the placed pose, and the body gets the car's motion from `last` to there.
  - `tick` is the world node's update: search, step, and then the car's listener (`settle`).
  - `settle`: once the body is at rest it is switched off and taken out of the lists, and the
    countdown starts at `ResetTime`. When the countdown runs out (tested with `<=`, as in the
    original), the object is put away and the car goes back on its path.
- **`traffic.rs`.**
  - `Traffic::physics`, `with_physics`, `strike(k, dt, world)`.
  - In `frame_held`, the physics node's frame runs first (`physics_frame`, with the car's own mesh
    hidden from its own search).
  - Then each physical car runs its clock alone (`Spec::advance`) and stands at its body.
  - On its first update back on the path, a car gets no motion.
- **Snapshot and hash.** `snapshot.rs` has a new `S186phys` section; the track's constants are kept from
  the machine. `hash.rs` walks `physics_car`. The `Sim` size pin is now 12096.
- **Shell.** game-render `toy::Traffic::physics(archives, edge)` loads every `HasPhysics` car. openmx
  `main.rs` sets `cars.physics` with the terrain's edge.
- **`prop_world.rs`.** `node_bounds` is split, so its span is available on its own (`node_span`). Its
  behaviour is unchanged.

## How it was checked

`crates/game-sim/tests/physics_car_struck.rs`, 6 tests:

- **The load against RAM**, for both trucks. Bit for bit: inv mass, inv inertia, damping, cofg, the
  thresholds, `ResetTime`, the countdown, the collision object's mass, and the contact list's
  capacity (47). The edge is the terrain's.
- **The struck mode.**
  - The interpreter runs `physics_car_wake` and then `0x0026f800` on the world node. The port runs
    `wake` and then `tick` against free06's terrain.
  - On every frame until the put-back, these match bit for bit: the body, the model node, the contact
    count, the countdown, the body's switch, and the physical flag.
  - Truck 0 comes to rest on frame 26 and is back on frame 326. Truck 1 comes to rest on frame 64 and
    is back on frame 364.
  - After the wake, the port's motion is checked against the car's own two virtuals.
- **Mutation:** with `ResetTime` one frame short, the run diverges at the countdown.
- **Put away:** a car that is put away does nothing when ticked.
- **In the `Sim`:** a truck on a straight path over flat ground is struck. It stands at its body,
  its clock runs on, and it is back on its path after more than 300 frames. A snapshot taken during
  the slide continues identically, whether copied or sent as bytes. A machine without physics cars
  refuses those bytes.

Also run: `./kit check` with game-sim `traffic`, `prop_world`, `prop_set`, `prop_knock`, `snapshot`,
`physics_cars`, `hash` and `--lib`, and game-render `toy`; all green. `cargo check -p openmx -p
game-render --tests --bins` is clean. `./kit audit` reports only two existing lines in
`plan/DONE.md`.

## What was learnt (in `research/oracle.txt`, S186)

- **The wake's velocity and spin are read after the placement.** The placement moves the model node,
  and the car's virtuals read that node against `+0x2e0`.
- **The inertia comes from the span (hi − lo), not from the node's cached centre/half.** The cached
  values are an ulp short in z, which leaves `inv_inertia` 3 ulps off.
- **The collision record's mass is at `+0x64`.**
- **An asleep truck's collision object stays filed and keeps searching** until the put-away.

## Not verified / not 1:1

- **Nothing strikes a truck yet.** `Traffic::strike` is the entry point, but the bike's, the
  plane's and the props' searches do not call it on a hit with a truck's mesh. Filed as S186a.
- **No PCSX2 capture** of the bike striking a free06 truck. S186a.
- **What the others hit while a truck is woken.** The port keeps the car's path mesh in its slot,
  standing at the body's pose. The original stands that mesh down and files the `f.col` (category
  0x18) on the collision object, so the bike would hit the `f.col` and push the truck back.
  `PhysicsCar::shape` and `pose()` are there for S208/S186a.
- **The truck's own search.** It skips its own slot (`hide`) in place of the original's ignore pairs.
  It does see the other cars and the props, but only the terrain was compared. s89-along's trucks
  slide on bare ground.
- **The order of the world node and the car updates within a frame was not seen.** The port runs
  the node first.
- **The physical car's clock** skips a frame whose `dt` is not positive, as `Spec::step` does. That
  case was not checked.
- **The Traffic integration** (strike, clock-only update, zero motion on return, snapshot) is checked
  only on the port's own flat-ground ride, not against the original.
