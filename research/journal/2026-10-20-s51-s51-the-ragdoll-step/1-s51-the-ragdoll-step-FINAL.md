# 1 — s51 the ragdoll step (FINAL)

Task S51. 2026-10-20.

## What I did

- **Ported the ragdoll's step into `crates/game-sim/src/ragdoll.rs`**, beside S50's records and entries:
  - `Pin`, the pin constraint: `solve`, plus its two helpers `two_body` and `one_body`. `solve` covers
    off, the two-bone path with its twist limit, and the one-bone path with its turn limit. The one-bone
    path gives the target and its velocity either from what the pin follows or from the pin's own anchor.
    A limit that is hit sets the lane and the sticky flag.
  - `axis_angle`, the quaternion's half angle from the acos table, and `FineAcos`, the 25-piece fine
    acos table the twist limit reads.
  - `Ragdoll::throw_clock`: the pull while `throwing`, turned by the main bone (`world_frame`), turned
    by nothing (`other_frame`, which wins), or by the bike. It counts down the force time, and when the
    time is up the throw ends with the pull zeroed. The hand-hold clock is in the same block.
  - `Ragdoll::throw_force`, the throw's force listener.
  - `Ragdoll::forces` and `Ragdoll::torque`, the ragdoll world's first force listener:
    - gravity on every part, `-32.2 × gravity scale × mass`;
    - while riding, the pose spring and damper to the skeleton's pose point;
    - while stiff, every joint's torque spring with its critical damping. In mode 2 the spring is faded
      by fade-left / fade-length and halved, and the torque is pushed back on the first bone.
  - `Ragdoll::step` and `Ragdoll::update`, the body actor's physics update. They step the world at its
    iterations and rate (3 × 25 Hz riding, 2 × 55 Hz wrecked) through biplane's `World::step`, with a
    `Stepper` as the `Dynamics`: the listeners, then the constraints in the world's order. They also
    handle the `placed`/`active`/`repose` gates and the frozen flags.
  - `Bone.reference`, plus `Ragdoll.pins` and `Ragdoll.pull`. `Drive` and `Carrier` hold what the
    caller must give the step: the skeleton's pose points, the rests, the hold targets, the bike's
    carry, and the fade and pose-spring numbers.
- `air::widen`, `air::narrow` and `air::acos` are now `pub(crate)`. `camera.rs` gained
  `CurveTable::coarse_sin`, the curve table's uncorrected sine that `axis_angle` divides by.
- Added a "# S51" block to oracle.txt: 18 names, plus a comment map of the world's lists, the pin
  layout and where a bone's rest lives.
- Added ten proofs to `crates/game-sim/tests/ragdoll.rs`.

## What the original does

- **The physics update** (`./kit fn rider_physics_update`):
  - A debug bit in the app's flags skips everything.
  - Otherwise it runs the skeleton virtual. It sets the two frozen flags to
    `!(placed && active && !riding && !repose)`.
  - While riding, it syncs every part, then runs the animation virtual, which holds the throw clock.
  - When placed, active and not reposing, it steps the ragdoll world, then runs the post-step virtual.
  - When reposing, it runs the pose and reset virtuals and clears `repose`.
- **The world step** is the shared dynamics step (S42's `World::step`):
  - The force listeners run in list order: the actor's, then the throw.
  - The constraints then run in list order, iterated: holds 0–3 (bones 11, 7, 17, 14), bone
    collisions 0–1, the 17 joints, collisions 2–8, and the bike box.
- **The pin** (`./kit fn pin_solve`, `pin_two_body`, `pin_one_body`):
  - The two-bone impulse takes a `c` that is summed into every length.
  - The limits are per lane, lower and upper. A lane that is hit is set onto its limit, and the hit and
    sticky flags get ±1 in that lane.
  - The two-bone limit is a twist read from the fine acos table. The one-bone limit is a turn against
    the bone's reference.
- **The throw** (`./kit fn throw_clock`, `throw_force`):
  - While the force time is positive, the pull is the launch force (turned as described above) and the
    time drops by dt.
  - Once the time runs out while throwing, the pull is zeroed and the throw turns off.
  - The listener adds the pull to the target body when that body is enabled.
- **The joint torque** (`./kit fn ragdoll_force_listener`):
  - The axis and angle of `q1⁻¹·q2` relative to the rest, from `quat_axis_angle`.
  - The spring is clamped at the max angle. The damping acts on the relative spin.
  - The torque goes to b2, and minus it to b1.

## How it was proved

Each proof is a function oracle on first_recording's save state, run on the live rider body actor in
`crates/game-sim/tests/ragdoll.rs`. Every proof asserts 10 000 cases, plus coverage counts.

- `the_step_is_wired_as_the_live_actor` checks the actor's world, its listener and constraint order,
  the pins' bones, types, tables and what they follow, and the gates this port fixes: holds not
  limited, joints on, the throw pulling through the actor's pull with no group.
- `the_step_tables_are_the_games`: all 30 001 acos entries and the 25 × 401 fine acos entries are
  bit-exact against RAM.
- The following match field for field (with the accel `w` exceptions listed below):
  - `the_axis_angle_matches_the_original`: over 2000 of the cases are clamped.
  - `the_pins_two_bone_solve_matches_the_original` and `the_pins_one_bone_solve_matches_the_original`.
  - `the_pin_solve_matches_the_original`: seven coverage buckets (off, then one-bone and two-bone each
    unlimited, limited but not hit, and limited and hit), each with more than 300 cases.
  - `the_throw_force_matches_the_original`: between 2000 and 3000 cases pulled.
  - `the_throw_clock_matches_the_original`: eight buckets, each with more than 800 cases.
  - `the_force_listener_matches_the_original`: riding/wrecked × stiff/limp, each with more than 400
    cases.
  - `the_physics_update_matches_the_original`: a whole update against the original's. It took 3763
    steps: 1818 riding and the rest wrecked, 3552 with a hold, and every one of them with a pin limit.
- I planted two mutants and both were killed:
  - fade 0.5 → 0.55 was caught by the listener test at case 9, and by the update test;
  - the solve's lock constant was caught by the solve test at case 86.
- `./kit check -- -p game-sim --test ragdoll`: TESTS PASSED, 15 tests. A full `./kit check`: TESTS
  PASSED.

## Not verified / not 1:1

- **The physics update's other virtuals are hooked out of the proof, not ported:**
  - the skeleton virtual (vt +0xb0) and the per-part sync while riding;
  - the animation virtual (vt +0xa8). Its throw-clock block is ported and proved by itself, but the
    rest of it is not ported;
  - the post-step virtual (vt +0xac), which includes the bone-0 normalise;
  - the pose and reset virtuals when reposing;
  - transform_touch and the update's tail call.

  They belong to S52/S53.
- **The debug-bit gate is not ported.** The update test asserts that the bit is clear in the state.
- **The bone collisions (bone_collision_solve) and the bike box (bike_box_solve) are no-ops in the
  `Stepper`.** They are S52. The proofs hook them out of the original too. The constraint order is
  hardcoded, and the wiring test asserts it matches the live list.
- **The pins' anchors, limits and gains are not loaded from `mrider.phy`.** `Ragdoll::new` leaves them
  zero, and the proofs set them from RAM or generate them. S53 must load them.
- **The holds always run with limited = false, and the joints always with on = true.** The wiring
  test asserts both on the live actor. Other values are proved only through `Pin::solve` itself.
- **The throw listener's group path and inline-force path are not ported.** The wiring test asserts
  that the group is null and that the force pointer is the actor's pull.
- **The caller supplies the skeleton's pose points, the rests, the hold targets, the bike carrier, the
  fade and the pose spring through `Drive`.** Nothing computes them yet (S53).
- **Some vector `w` lanes are not compared.** The linear accel's `w` is never loaded by the original,
  so it holds a stale register. The angular accel's `w` is the stale register when inv mass is 0.
- **`axis_angle`'s no-turn branch is unreachable and unproved.** The acos table gives 2^-11 at w = 1,
  not 0.
- **The update proof uses tamed, physical inputs.** On random inputs the bodies blow up. Once an angle
  runs off the curve table, the port panics, while the original reads past the table's end. That
  behaviour is not matched.
- **The save state has `placed = 0`**, so the update proof sets it. The state never ran an update
  through the step.
- **`CurveTable::build` is still not bit-exact** (libm single precision), so the proofs read the game's
  curve table from RAM. A ride that builds its own table will drift by ulps until that is fixed.
- **No captured wreck was compared against the port.** That is S53.

## Next

S52: contact. Port the bone collision and the bike box solves into the `Stepper`'s two no-op slots,
then the pose/reset virtuals and the settle test on the real bones. S53 then loads the pins' anchors,
limits and gains, computes `Drive` from the skeleton, wires `Ragdoll::update` into `Rider`, and holds
it against the captured wrecks.
