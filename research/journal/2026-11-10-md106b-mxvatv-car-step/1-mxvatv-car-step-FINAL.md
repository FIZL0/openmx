# 1 — mxvatv car step (FINAL)

Task MD106b. 2026-11-10.

## What I did

- `atv_car::Own { v: &mut Vehicle, rest }`: a `Steps` that answers five of the car frame's calls with
  ported code and passes every other call on to `rest`. The five are `VehicleForce`, `VehiclePost`,
  `WreckTimer`, `Landed` (the node-local turn, which then hands out `LandingSound(out)`) and
  `NodePosition`.
- `atv_car::Vehicle` holds the SLUS-21104 words those calls read and write: the wreck switches, clock,
  `MaxWreckTime`, counting flag and tipped-wreck flag; the kind; `WreckDamping`/`WreckInertia`, the
  car's inertia and the body's current inertia; the rumble and look switches; the record's touching
  word; the world's gravity and drag; the flag byte; the scene (node 0 and its parents) and its
  `subtree` flag; the world's bounds; and the map-edge answer.
- Ported functions: `vehicle_force`, `vehicle_post` (reusing `atv_moto::origin`, `in_world` and
  `Scene::place`), `node_local` (`chassis::unrotate` over the updated world rows), `wreck_timer`,
  `wreck` (MXVehicle's wreck entry plus the buggy's spin-up and look), `unwreck` (the buggy's latch,
  velocity cut, look and respawn, then MXVehicle's recovery), and `set_inertia`.
- New `Call`s for what these hand out: `GravityRecord`, `BodyReset`, `Respawn`, `SpatialUpdate`,
  `NodeBounds`, `CollisionSync`, `OutOfBounds`, `Rumble`, `Cue(kind)`, `WreckLook(bool)`,
  `LandingSound(Vf)`, `SubtreeOutOfDate`.
- The input words: these are read inside the controls step (buggy vt +0x34c, `./kit dis -p atv
  0x4ca450`), and MD106a ports that step whole. I left them there and put a plan note on MD106a.
- Follow-ups: MD122 covers what the five calls hand out in turn (needed by MD106c). Notes added to
  MD106a and MD106c.

## What the original does

Addresses are in research/oracle.txt `# MD106b`.

- **Wreck timer**: does nothing unless wrecks are on. If the car is held to the map and isn't already
  in state 13, a positive map-edge test starts wreck 13. The state is then read again.
  - Riding: the car wrecks tipped (state 14) when all of these hold: the node's own up row dotted with
    (0,1,0) is under 0.8, the ground speed is under 10, the record is touching, and the car is on one
    side or in the air. The clock then starts at `MaxWreckTime − 2`.
  - Wrecked: the clock runs.
    - State 13: past the max, it restarts counting and posts cue 12. While counting, past the max
      again or still, the body keeps 0.06 of its velocity and the car unwrecks.
    - State 14: not counting, past the max and still → unwreck, then it starts counting.
- **Wreck**: sets the state, zeroes the clock and the counting flag, and asks for a rumble if this is
  the player's car with a pad. Unless the kind is 0 or 1, both dampings become `WreckDamping` and the
  inertia becomes `WreckInertia × inertia`. Then cue 11, rates ×2.42, and the wrecked look goes on
  (with no null check on the look object).
- **Unwreck**: sets the latch, clears the tipped flag, keeps 0.12 of the velocity, takes the look off
  if there is one, and respawns. Out of state 13, the world's drag is divided by 0.5. The state and
  clock go to 0 (the counting flag stays). Unless the kind is 0 or 1, the dampings go back to 1 and the
  car's own inertia comes back (its `w` is written as 1). Then cue 13.
- **Vehicle force and post**: the same code as the quad's (`atv_vehicle_force` / `atv_vehicle_post`).
  The car's node is a root with a 44-node subtree, which placing the node marks out of date.

## How it was proved

Function oracle: `crates/game-sim/tests/mxvatv_car_frame.rs`, `the_cars_own_calls_are_the_originals`.

- It runs MD106's world-step oracle off MD39a's `buggy.ee.bin` with the five calls unhooked. Their
  inner calls are hooked instead, each asserting it was handed the car, node, record or look object.
- 3,000 generated states. Every word of the car, tires and body matched, plus the body's inertia, the
  vehicle's wreck words, the world's drag/gravity/floor/box, the flag byte, and the scene nodes' frames
  and flags. The two call logs were equal, and the original wrote nothing it didn't compare in those
  blocks.
- All 20 branches were seen at least 30 times each, including: wrecks off; the edge wreck, the edge
  clock and the edge unwreck; the tipped wreck and tipped unwreck; both kind groups for wreck and
  unwreck; an unwreck without a look; inertia lanes ≤ 0; rumble; outside/inside the world; node
  bounds; and the landing sound.
- A one-ulp mutation of the wreck spin made the test go red (case 8, 6 words). MD106's and MD106d's
  tests still pass (3 tests in the file).

## Not verified / not 1:1

- **The map-edge test and throw** (MXVehicle's out-of-bounds slot) are not ported. The port hands them
  out as `OutOfBounds`, and the test pre-sets the answer in `Vehicle::out_of_bounds`; a real `Steps`
  can't write it from inside the call. The throw's own writes (velocity, drag, rates) were skipped on
  both sides. Ported in MD122.
- **Respawn, body reset, spatial update, node bounds, collision sync, the record's gravity call and
  the subtree walk** are handed out, not run. Their writes were not compared, because the hooks skip
  them. Wiring the quad's pieces for the car is MD122.
- **Cue, rumble, the wrecked look, the landing sound**: handed out only (sound, pad and model, not
  physics). The rumble hook always answered "off", so the rumble's own vt call never ran.
- **The body disabled** (body +0 clear) was never generated. The force and post return at once there,
  but the world step may skip the body anyway.
- **The look object absent with a wreck starting** was never run. The original calls through a null
  pointer there, so the generator never makes it; the port calls `WreckLook(true)` regardless.
- **The body's listener loop** in the gravity add (body +0xe8 list) is not ported. The image's list is
  empty, so it never ran.
- **MD39a's input words feeding the controls**: left to MD106a (see above).
- Only the buggy (kind 4, one image) was exercised; the kind word was varied over 0, 1, 2 and 4.

## Next

Nothing for MD106b. MD122, then MD106c.
