# 1 — rider world step (FINAL)

Task S280. 2026-11-10.

## What I did

- New `crates/game-sim/src/handling/ragdoll.rs`. It runs MX vs. ATV's rider ragdoll world on RAM:
  - `step` drives `biplane::World::step` over the world's lists in RAM. The bodies are `atv_body::Body`, loaded and saved with `seed::body` / `seed::body_words`. Only words that changed are written back.
  - The caller's `Rig` says what each constraint, force and listener is. The original decides this by vtable, and vtable addresses must stay out of shipped code, so the test maps them. The bone collision goes through `Rig::bone`, which needs the scene.
  - `pin` adapts MXU's `ragdoll::Pin` to this layout. Its two-body solve uses the pin's own copies of body 1's inverse mass and inertia; the twist limit before it uses body 1's real inertia.
  - `frame_box` has four ways. In another body is new code. In a node is MXU's `NodeBox`. In a pointed-at frame, or in its own frame, is MXU's `BikeBox`.
  - `rider_force` covers gravity, the pose spring/damper and the joint torques with the wreck fade. `directed_force` is the other force. `add_force`, `add_moment` and `add_moment_world` are the body accumulators.
- Made `ragdoll::Pin::twist_limit` / `turn_limit` pub. This only changes visibility, so the ATV pin can swap body 1's inertia between the limit and the solve.
- Tests, in the S280 section of `tests/mxvatv_bike_ride.rs`: four oracles, plus the C15 replay with the whole world in the port.

## What the original does

All the words and addresses are in research/oracle.txt `# S280`. To read the code, use `./kit fn` or `./kit dis -p atv` on the world step, the pin solve, the box solve, the directed push and the rider force named there. The rider force needs `./kit dis` with an explicit end, because Ghidra's size for it is wrong.

- **One world step**, `dt` the frame's:
  - preforce listeners (none);
  - substeps at rate 27, each running the two forces, then integrate for each enabled body, then end of substep;
  - then the world's iteration count of constraint passes;
  - then post listeners (none).
  - This is the same driver the bike's world uses (`biplane::World`). The constraint solve gets the substep in f12; `a1` is the first-pass flag, which none of the three kinds reads.
- **The first force** wraps a listener. That listener is the rider (through a thunk 0x250 into it).
- **The rider force:**
  - For each holder's body, gravity on y, as an FPU `adda`/`msub`.
  - When riding, the pose spring and damper. Their target is the body's pose point placed by its holder's node. The target velocity is the bike's record velocity, carried round by its spin about its centre.
  - With both torque switches on, each type-2 joint the rider owns gets MXU's joint torque. In mode 2 (wreck) the torque is halved and faded, and the first body takes it back in world frame through the body's unturn-then-add-moment slot.
- **The directed force** adds its vector (pointed at, or its own) to its one body, or to every enabled body of its world. On C15 it is off.
- **The pin** is MXU's. **The box** is MXU's `BikeBox` / `NodeBox`, plus a new way: held in another body's frame, with that body's point velocity carried.

## How it was proved

- **Function oracles** (`tests/mxvatv_bike_ride.rs`). Each has 12,000 drawn cases against SLUS-21104 on state 4. Every word of the bodies, constraints, forces, rider, record, rests, nodes and scratch is equal, and neither side writes outside those regions:
  - `the_rider_worlds_pin_is_the_originals`: covers off, two bodies (limited or not), one body (limited or not), first body disabled, and the three target ways. The fewest any path got is 993.
  - `the_rider_worlds_box_is_the_originals`: covers in a body, in a node, a pointed frame (carried velocity or address 0), its own frame, and off. The fewest is 334.
  - `the_rider_worlds_directed_force_is_the_originals`: covers all 6 paths. The fewest is 1,105.
  - `the_rider_force_is_the_originals`: covers riding or not, no torques, plain torques, and wreck torques with and without the fade. The fewest is 647.
- **C15 replay** (`the_rider_world_steps_as_the_originals`). The world step is hooked for this world only, and `handling::ragdoll::step` runs it whole: the rider's force, the directed force, the 18 bodies' integrate, and the pins, boxes and bone parts. The bone parts use S278a's `bone::solve` on Nat05's map, with the scene's search left to the original. Every row is bit-exact:

  | Replay | Steps | Substeps | Constraint passes |
  |---|---|---|---|
  | ride-2 | 459 | 459 | 1,377 |
  | flip-1 | 433 | 501 | 1,367 |

- **Mutants:**
  - Using body 1's own inertia in the two-body solve instead of the pin's copy: red (26 words, case 1).
  - Making the torque's `spring·a − critical·w` one VU `msub`: green. In ps2_float the fused and separate forms round the same except on signed zeros (memory: fused twins differ only on −0). The drawn critical gains are never zero, so this one is equivalent here.

## Not verified / not 1:1

- **Body listeners are not called.** `add_force` / `add_moment` skip the body's +0xe8 listener list: the original calls each listener's vt+8 / vt+0xc. Every rider body in state 4 has count 0 there, and the C15 replays stay bit-exact. A body with listeners would differ.
- **Kinds outside the Rig panic in the test.** Other constraint, force, listener, preforce or post kinds are left to the caller's `Rig`. The test's Rig panics on them, because state 4 and C15 have none (both listener lists are empty, the forces are the two above, and the constraints are pins, boxes and bone parts). Nothing other than those has been run through `step`.
- **The oracles draw normal floats only.** No NaN, Inf or denormal inputs are drawn. The port reads operands as `F::from_bits` / `Vf::from_bits`, which saturate and flush as the PS2 does, but those edges were never compared with the original.
- **Unproven rider-force clamp edges.** The sticky-lane test and the max-angle clamp in the rider force were compared only on drawn values. A lane value of exactly ±0 against a sticky lane of exactly ±0 was hit rarely, if ever; the draws include signed zeros but no case was forced.
- **Why flip-1 runs 1,367 passes is unexplained.** That is fewer than 3 × 501. The port re-reads the world's iteration word every step, as the original does, and stays bit-exact, but I didn't find where the iteration count changes.
- **The rests table is assumed to hold at least 32 entries.** The rider-force oracle draws the rests table's first 32 entries (0x800 bytes from the table's start) and counts them in the compared region. The table's real length was not read.

## Next

Nothing further for S280. The whole rider world step now runs in the port, bit-exact on C15.
