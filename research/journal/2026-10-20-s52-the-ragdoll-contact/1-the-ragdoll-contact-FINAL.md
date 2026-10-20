# 1 — the ragdoll contact (FINAL)

Task S52. 2026-10-20.

## What I did

- **The bone collisions**, new `crates/game-sim/src/ragdoll_contact.rs`:
  - `Sphere::read` reads the nine `Collision` constraints of `mrider.phy`, together with each bone's
    `Radius`.
  - `Sphere::solve` covers, in the original's order:
    - the ground query under the bone, inside the terrain's box;
    - the scene's contacts at the bone's sphere;
    - the sweep from `1/15` s back.
  - `Contact::push_out` is the push-out and the impulse.
  - The world is reached through `World::ground` for the ground and a small `Scene` trait for the
    other two.
- **The bike box** (`BikeBox::hold`), the type-4 constraint that keeps the main bone inside the bike
  while riding. It has two frames:
  - the actor's moving frame: the drawn bike transform, a centre and a half size, and the bike's
    velocity carried;
  - the constraint's own frame, with nothing carried.
- **The two virtuals both ragdoll entries begin with**, new `crates/game-sim/src/ragdoll_pose.rs`:
  - `Start` is a bone body's start state. `Start::place` and `Start::turn` are the body's two start
    setters.
  - `reset` is `rider_ragdoll_reset`: the world's reset, which hands every body its start and empties
    the rest.
  - `pose` / `pose_all` is `rider_ragdoll_pose`, as described below.
- **The pose** (`pose` / `pose_all`):
  - The bone's orientation becomes a quaternion on the `w >= 0` side. Its start position is the
    node's position plus the turned centre of mass.
  - Unless the bodies start still, its velocity and its rates are measured against the part's drawn
    matrix, over the frame. The engine's quaternion-to-axis-angle helper is ported privately for this.
  - The drawn matrix is rebuilt from the new start.
  - The skeleton's world pose is not in game-sim, so `pose` takes each bone's node position and two
    axes as a `Bone`.
- **The settle test is a clearance, and it reads the bones:**
  - `Thrown.height` became `Thrown.clearance`, and the test is `clearance <= RiderFlailHeight`.
  - `Machine.height`, S26's latch for the rise across a frame, is gone.
  - `Thrown::from_bones` reads the three sensors the way the wreck arm does: any bone collision's
    contact flag, the main bone's velocity, and the main bone's height less the ground query's answer.
  - The ride's caller still fills `Thrown` from `Crash`'s rider point, because the ride does not run
    the ragdoll yet (S53). `Crash` now keeps the ground height its step found under that point
    (`Crash::ground`, `Crash::clearance`).
- research/oracle.txt:
  - New names: the bike box, the reset's world and body functions, both setters, the node's world
    position, axes-to-quaternion and quaternion-to-axis-angle.
  - The pose's entry describes what the function does.
  - The settle test's note now says clearance, not "height before the step".

## What the original does

- **The bone collision's solve** (`./kit dis 0x001f7550`, slot `+0x18` of the type-8 vtable; oracle.txt
  `bone_collision_solve`):
  - A bone is skipped unless both the constraint and its body are on.
  - **The ground.** Inside the terrain's box the ground query runs on a copy of the centre. A centre
    less than the radius above the answer is pushed out along the normal, from a point a radius under
    the centre.
  - **The scene.** The sphere collider is refreshed at the centre and collided against the scene. Each
    reported contact is pushed out.
  - **The sweep.** If the sweep is on, the segment collider runs from `centre - velocity * 1/15` to
    the centre. A hit is pushed out with the impulse applied at the centre, and the bone is marked
    swept.
  - **The push-out:**
    - The overlap is taken back at `UnpenetrationScale`.
    - The velocity along the surface keeps `Friction`. Into the surface it keeps none, or
      `-Restitution` of it when it came in slower than 5.
    - The change is applied as an impulse at `RotationScale` radii from the centre, which also turns
      the bone.
- **The bike box** (`./kit dis 0x001f5b40`; slot `+0x18` of the type-4 vtable):
  - The bone is taken into the box's frame and centred.
  - Each axis outside `±half` is clamped to the face, and that axis's local velocity is zeroed.
  - The result goes back to the world, with the carried velocity added back on.
  - Live, the actor wires its moving frame in: the drawn bike transform at actor `+0x580`, centre
    `+0x540` = (0, 4.3, -1.5), half `+0x550` = (0.45, 0.6, 1.75), and the carried velocity `+0x530`.
- **The reset** (`./kit dis 0x0030f0b0`):
  - It tail-jumps to the world's reset, which calls every body's vt `+0x2c`.
  - That copies the start into both the current and the last position and orientation, the velocity
    and the rates, and writes `(0,0,0,1)` into the ten accumulators.
- **The pose** (`./kit dis 0x0030f0c0`), per part:
  1. The node's world position and its two axes. The orientation is `axes_to_quat`, negated on VU0
     when `w < 0`.
  2. Unless `a1` is set or `dt <= 0`:
     - The velocity is `(position - drawn position) * (1/dt)`.
     - The relative turn from the drawn matrix's quaternion is unturned by the new orientation.
     - The rates are its axis times `angle / dt`, by the helper `quat_axis_angle`. The helper:
       1. clamps `w`;
       2. reads the acos table;
       3. if the angle is exactly zero, answers the axis `(1,0,0,1)`;
       4. otherwise divides by the curve table's raw sine entry, with no interpolation, and doubles
          the angle.
  3. The position is the node's plus `turn(q, cofg)`. The body's two setters are called, then the
     drawn matrix is rebuilt by `mat4_basis(position, forward, up, 0, 1)`.
- **The settle test's sensors** (`./kit dis 0x0024d130 --len 0x260`):
  - `touching` is any of the nine collisions' `+0x14`.
  - The clearance is the main bone's `+0x64` less ground_query's answer. The query gets a copy of
    `+0x60`, the actor's `+0x2bc` world, and no normal, flags or record.
  - The speed is the length of the main bone's `+0x70`.

## How it was proved

Function oracle on the live rider body actor in first_recording's RAM image, `crates/game-sim/tests/ragdoll_contact.rs`:

- `the_spheres_are_the_live_collisions`: `Sphere::read` of the disc's `mrider.phy`, checked against
  all nine constructed collisions.
- `the_solve_matches_the_original`: 10 000 cases over the nine collisions, compared bit for bit.
  - Generated per case: the bone, the flags, the terrain box, the ground answer, the scene contacts
    and the sweep hits.
  - Hooked: the ground query, the scene collide and the segment setters. They answer and log the
    same on both sides.
  - Coverage asserts on ground, scene, swept and off.
- `the_bike_box_matches_the_original`: 10 000 cases over both frames.
  - One case in six puts the bone exactly on a face.
  - Coverage is asserted for held, free, moving, still and off.
  - The `<`/`<=` mutants on both face compares are killed.
- `the_reset_matches_the_original`:
  - 1 000 resets of all 18 bodies, with every quadword of every body generated.
  - Then 10 000 calls of the two setters.
- `the_pose_matches_the_original`: 1 000 frames × 18 bones = 18 000 bone poses.
  - The two skeleton reads are hooked, and the node order is checked.
  - Compared: start position, velocity, quaternion and rates, and all four rows of the drawn matrix.
  - Coverage asserts: still 6930, moving 10792, not turning 278, flipped 8471, drawn flipped 9089.
  - The mutant that does not flip the drawn quaternion is killed.

`crates/game-sim/tests/rider_anim.rs`, the wreck-arm proof (10 000 frames):

- It now hooks `ground_query` and reads the port's sensors with `Thrown::from_bones` off the same
  memory: one to four collisions' flags, and the main bone's whole position and velocity.
- It checks the point and the four arguments the ground query was handed.
- Coverage is more than 1 000 settles and more than 100 flings.
- The `<` mutant of the clearance compare and the `all` mutant of the touching scan are killed.

`./kit check`: passed after the pose commit, and again before `./kit done`.

## Not verified / not 1:1

- **The ragdoll is not wired into the ride.** Nothing in `Rider` runs the collisions, the bike box,
  the pose or the reset, and the settle test is still fed from `Crash`'s rider point through
  `Crash::clearance`.
  - Wiring needs S51's step to iterate the constraints, and the skeleton's world pose for `pose`.
  - That is S53, and calling the reset alone would zero the bones.
  - So the settle test's *inputs* in a live ride are still S26's stand-in, a single point. Its
    `touching` is `Crash::landed`, not any bone's flag.
- **Adding `Crash::ground` changes the ride's state walk**: a new `ground` field. Old state hashes of
  a wreck differ.
- **The moving-other push-out is not ported.** That is a contact whose other side has a body: the
  bike's bodies or a vehicle. The bone solve's dynamic branch is absent, and so is the bike box's
  `+0x14` other-actor path.
- **The scene is not ported.** Its broad phase, the sphere collider's collide and the segment collide
  are all missing. `Scene` hands the solve contacts the caller found. The proofs hook them.
- **The bike box with its three pointers set but `+0x98` null** reads address 0 in the original. It
  is not ported.
- **The live bike box's centre and half size come from actor `+0x540`/`+0x550`, not from
  `[Bike Constraint]`'s BoxPosition/BoxSize in `mrider.phy`.** Their writer is not identified.
- **Degenerate masses were left out of the solve's proof.** A negative inverse mass or inertia can
  zero the lever's denominator, and the original then reads the curve table at a wild index. The
  cases keep the inverse mass in 0.05..6.
- **The impulse's scratch `w` is taken as 0.** It is 0 on all nine live collisions (asserted), but no
  writer was found.
- **`Sphere::read`'s defaults for missing keys** (Restitution 0, the rest 1) are the loader's as read.
  The disc's file has every key, so the defaults are never exercised.
- **The setters are proved with non-aliasing sources only.** A source that is the start itself would
  be overwritten mid-copy, and nothing calls them that way.
- **The pose's skeleton reads are hooked, not run.** `node_world_position` and `orientation_axes`
  answer generated values. The pose's skeleton input is the caller's to get right in S53.
- **`axis_angle`'s `w` clamp is ported as one pass.** The original clamps twice, and the second pass
  cannot change a clamped value.
- **The pose's "no turn" branch (acos exactly 0) was reached 278 times in 18 000.** The coverage bar
  is 200, not the 300 first written.
- **The RAM image had to be regenerated** with `./kit state ram` before the oracle tests would run
  (memory: biplane-ram-image-may-be-missing).

## Next

S53: run the ragdoll in `Rider`. Use `pose_all` + `reset` on both entries, S51's step with the
collisions and the bike box inside its iterations, and `Thrown::from_bones` instead of
`Crash::clearance`. Then drop `Crash`'s rider point.
