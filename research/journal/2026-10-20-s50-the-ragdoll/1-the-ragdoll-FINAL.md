# 1 — the ragdoll (FINAL)

Task S50. 2026-10-20.

## What I did

- **Answered the brief's first question: what the shell shows.**
  - `Crash::step` (crates/game-sim/src/ride.rs:2085) does collide the wreck body with the world. Its
    `contact` puts one `Radius` sphere against `World::ground` with a spring, a damper and friction.
    That is S26's stand-in, not the original's solver.
  - The rider is one point that falls under gravity until it reaches `World::ground`, then stops dead.
  - The shell (crates/openmx/src/main.rs:1339) draws the bike and the rider at
    `chassis_at(&world.sim.rider.ride)`, the frozen ride pose, and never at `Crash::body`. So on screen
    a wreck freezes the bike and the rider where they came off.
- **The task is much bigger than one session, so I split it (AGENT.md).** This session did the first
  part: the ragdoll's data, its two entries and its damping switch, all proved bit-exact. The rest is
  three new tasks:
  - **S51**: the step (pin solve, throw force, joint torques, physics update).
  - **S52**: contact with the ground and the bike, the pose virtuals, and the settle test on real bones.
  - **S53**: wiring it into `Rider`, drawing it in the shell, and holding it against the captured
    wrecks.
- **New `crates/game-sim/src/ragdoll.rs`:**
  - `Records::read` reads the 18 part records, the 17 joint records and the 9 bone collisions out of
    `rider\anim\mrider.phy`, the way the rider loader builds them.
  - `Bone` carries the mass and inertia setters.
  - `Joint::set` computes the critical damping.
  - `Ragdoll::ride_enter` and `Ragdoll::wreck_enter` cover every reason's arm: the throws, the reason-5
    hand draws, 6/7's hit velocity, 12's flattened velocity, and 13's cartwheel.
  - `Ragdoll::damping` is the switch the settle test throws.
- `biplane::unturn` is now `pub(crate)`; the cartwheel uses it.
- New oracle.txt names, with a comment map of the actor's ragdoll fields: `rider_ragdoll_pose`,
  `rider_ragdoll_reset`, `collider_ignore`, `wreck_sound`, `body_set_mass` and `body_set_inertia`.
- I corrected the old `rider_ragdoll_step` comment. That function is the ground query, and the settle
  test does not measure a rise.

## What the original does

- **The rider body actor's ragdoll:** 18 bones (parts), 17 pins, 9 bone collisions, 4 holds (the two
  hands, plus the two feet that only riding holds), a bike box, a launch force, and two worlds:
  - riding: 3 iterations at 25 Hz;
  - wrecked: 2 iterations at 55 Hz.

  The constructor sets those four numbers. The file's `NumberOfIterations=4` and `SimulationRate=25`
  are read but are not what runs.
- **The part record (0x60 bytes):**
  - riding and wrecked mass;
  - riding and wrecked inertia;
  - riding angular damping;
  - `WreckAngularRateDamping`: the settled damping, despite the name;
  - `RagdollAngularRateDamping`: the limp damping.

  The wrecked mass and inertia default to the riding ones. The two dampings default to `(0,0,0,1)`.
- **The joint record (0x50 bytes):**
  - Holds the pin's two bones, its max angle, and the riding spring and damping copied from the pin.
  - The max angle is `MaxJointTorqueAngle × 0x3c8efa34`. That constant is `0.01745329`, one ulp under
    the nearest single to π/180. The first run of the record test caught the one-ulp difference.
  - The wrecked spring and damping come from `WreckJointTorqueSpring` and `WreckJointTorqueDamping`.

  **Quirk:** the loader reads them out of section `Constraint{i+1}` where `i` is the joint's own
  constraint number. Each joint gets its next constraint's wreck values. The first two constraints are
  collisions with no such keys, so joints 0 and 1 go limp with a spring of `(0,0,0,1)`.
- **The wreck entry**, handed the reason:
  - The common part:
    - the wrecked world;
    - the feet let go;
    - the bike box off;
    - the throw on;
    - every collision on, and both of the bike's colliders added to each collision's ignore list;
    - the joints switched to the wreck records, each with its critical damping
      `(d × 2) × sqrt(I × k)`. I is the bone's inertia **before** this entry changes it, because the
      joints are set first;
    - every bone given its wrecked mass and inertia, and its limp damping;
    - the flags reset.

    The inertia setter writes a `1.0` into the `w` of the record it was handed, so the record changes
    as well as the bone.
  - Then a switch on the reason:

    | Reason | Hands | Throw | Target | Frame | Other |
    |---|---|---|---|---|---|
    | 1 | both | HitFront | MAIN | world | |
    | 2 | right | HitSide | MAIN | own | |
    | 3 | left | HitSide | MAIN | own | x mirrored |
    | 4 | none | HitBack | BACK | own | x mirrored |
    | 5 | each 0.66 | HitBottom | MAIN | world | every bone takes the bike's velocity and rates |
    | 6, 7 | none | none | | | every bone takes the bike's hit velocity; the throw is off |
    | 8 | both | FallOffFront | | | mirrored |
    | 11 | none | FallOffBack | | | mirrored |
    | 12 | none | Stunt | | | mirrored; the bike's velocity flattened to y = 0 |
    | 13 | none | none (a cartwheel) | | | see below |
    | 0, 9, 10, 14+ | | nothing | | | |

    A throw draws five times: the force's x, y and z lanes, `ForceTime` and `HandHoldTime`, each as
    `base + delta × (2u − 1)`.

    The cartwheel (reason 13) draws a spin of up to 5 about each axis. Each bone gets:
    - velocity: the bike's hit velocity × (0x3f828f5c, 0x3f8147ae, 0x3f828f5c), plus the spin crossed
      with its arm from the main bone;
    - rates: the spin turned into the bone's own frame.

  - Last, the wreck sound plays.
- **The riding entry:**
  - the pose copy and the reset if the body was not placed;
  - the riding world;
  - all four holds and the box on, the throw off;
  - collisions off and not touching, and the bike ignored if there is one;
  - the joints on their riding records;
  - the bones on their riding mass, inertia and damping, plus a linear damping of 1;
  - the three flags set.
- **The damping switch:** settled damping when stiff, unless the bike's reason is 13. Otherwise limp.
  Going stiff also copies one of the actor's clocks into another.
- **The settle test is a clearance.** The function oracle.txt called `rider_ragdoll_step` is the
  ground query. The wreck mode compares *the main bone's y above the ground under it* with
  `RiderFlailHeight`, not a rise across a step. S26's `Thrown.height` and `Machine::height` are
  therefore the wrong quantity. Fixing that needs the real bones and their ground query, so it is
  filed as S52 rather than replacing one stand-in with another today.

Recipes: `./kit fn 24ad90` (the wreck entry), `./kit fn 24c490` (riding), `./kit fn 24c310` (damping),
`./kit fn 249cb0` (the loader; the records are in its second half), `./kit fn 2472c0` (the worlds).

## How it was proved

All four tests are in crates/game-sim/tests/ragdoll.rs and run on the live rider body actor in
`first_recording.p2m2_SaveState`:

- `the_records_are_the_live_actors`: `Records::read` of thq1.res's `mrider.phy` against what the loader
  left on the actor, bit for bit. That covers 18 part records, 17 joint records (including both bones
  and the max angle) and the 9 collisions' bones, plus the two world constants.
- `the_wreck_entry_matches_the_original`: a function oracle over **10 000** cases.
  - Reasons −1..15 are drawn uniformly, more than 400 cases each.
  - Generated per case: every bone's mass, inertia (including zero and negative lanes), pose, velocity
    and rates; every record; the seven throws; the bike's three vectors; every flag; and the
    generator's state.
  - Compared field by field: the whole ragdoll, the generator's state, and the exact sequence of
    ignore-list calls.
- `the_ride_entry_matches_the_original`: **10 000** cases, with and without a bike (more than 1 000
  each), placed or not.
- `the_damping_switch_matches_the_original`: **10 000** cases, stiff or not × reason 13 or not (more
  than 1 000 each).
- A planted mutant (hold chance 0.65) was killed at case 297, so the wreck proof does run and does bite.

`./kit check -- -p game-sim --test ragdoll`: `TESTS PASSED: 5 tests`.

## Not verified / not 1:1

- **The pose copy and the reset are not ported.** These are the two virtuals both entries call first
  (`rider_ragdoll_pose`, `rider_ragdoll_reset`); they need the skeleton's world pose. Both are hooked
  out in the oracle, so the bones' poses the port starts from are whatever the caller hands it → S52.
- **The wreck sound is hooked out and not ported.** It draws from the shared generator when its sound
  list has two or more entries. The proof hooks it out, so a port driving the real generator will be
  out of step by that draw after a wreck → S53.
- **The ignore list is not modelled.** The port keeps one `ignores_bike` flag; the test checks the
  original's calls by address. Deduplication and list capacity are not modelled.
- **The ragdoll's step, contact and bike box are not ported**, so nothing moves yet: the pin solve, the
  throw force, the joint forces, the physics update, the bone collisions and the box → S51 and S52.
- **The settle test still reads S26's stand-ins** (a rise across a frame of `Crash`'s rider point). The
  original's test is a clearance above the ground query → S52.
- **No captured wreck was compared**, and nothing draws the ragdoll: not recording2's wreck, not the
  first recording's reason-5 windows (ticks 3259–3349 and 4167–4190), and not the settle tick. All of
  that needs S51 and S52 → S53.
- **`ride::hit_of` disagrees with the wreck entry's switch.**

  | Reason | `hit_of` | The original |
  |---|---|---|
  | 6, 7 | Front / Stunt | no throw; the bones take the hit velocity |
  | 13 | Front | a cartwheel |
  | 0, 9, 10, 14+ | Bottom | nothing |

  `Crash` still uses `hit_of`. I left it alone because it is S26's and is read by its own proofs →
  S53.
- **The going-stiff clock copy is not ported.** `damping` does not copy the actor's clock. Which clock
  that is, and what reads it, is not established.
- **`Records::read` defaults for missing keys are read from the decompilation and are untested.**
  Retail `mrider.phy` has every key the records read, so the defaults (wrecked = riding mass and
  inertia; `(0,0,0,1)` for the wreck dampings and springs) never run in a test. Neither does a
  `Part%d` section that is missing.
- **The records test reads only thq1.res's `mrider.phy`.** The other riders' archives carry the same
  file name and were not compared.

## Next

S51: the ragdoll's step. It starts from `Ragdoll` as it stands, using the same live actor and the same
`ragdoll_of` snapshot in tests/ragdoll.rs.
