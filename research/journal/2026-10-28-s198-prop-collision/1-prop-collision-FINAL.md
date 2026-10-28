# 1 — a prop's own collision object (FINAL)

Task S198. 2026-10-28.

## What I did

- `crates/game-sim/src/prop_world.rs`: each `Object` gets an optional `Collision`. It holds the
  collider the body's loader made, with the props' category words, and a contact record built the
  way the wreck bike's is.
  - The record carries the model's m.col segments. Capacity is the ray count, or 8 without rays.
    Nearby and sweep are on only with rays.
  - `Collection::search` runs clear + collide for every filed object. It works against the terrain
    and the scene (`Scene::wooded`), and an asleep body keeps only hits on the wake categories.
  - The force listener resolves the contacts after gravity (`biplane::collision_force`).
  - Unfiling clears the record first. This applies both to a disable without detect and to a
    countdown put-back.
  - The record holds the object's node only while it searches, so `Object::node` stays the single
    copy.
- `crates/game-sim/src/prop_set.rs`: the loader reads `<model>m.col` through the disc, parses it
  once per model and shares it (`PropSet::rays`), and makes each object's `Collision`.
  `frame_among(dt, field, props, trees)` runs every collection's search, then every step, with the
  scene's moving props as movers.
- `traffic.rs`: the ride's frame passes the world's terrain and scene once any prop is filed.
- `props.rs`: `Prop` keeps its collider.
- `snapshot.rs`: the collision object goes on the wire, but not the rays, which are taken back from
  the live set.
- `tests/hash.rs`: the Sim size pin is now 11592.
- `tests/prop_collision.rs` (new): the proofs below.
- research/oracle.txt: an S198 block.

## What the original does

The node update (0x0026f800) runs clear and then collide(dt) on every filed collision object. It
then steps the world, and the force listener (0x0015af40) adds gravity and calls the collision
object's resolve (vt +0x80). So a knocked prop's rays and sweep find the terrain and the scene
meshes, and resolve pushes the body out the same way as the wreck bike's.

When the countdown runs out, the object listener (0x0015b2a0) puts the body back and then unfiles
it; the unfile clears the record's touch flags. With DetectCollisionsWhenDisabled, the asleep probe
keeps searching but is narrowed to the wake category.

## How it was proved

Each test runs the EE interpreter from the first_recording state (free09), with the port built from
the disc:

- `every_props_collision_object_is_the_originals`: for all free09 collections, the collider (moving
  fields aside), the record switches, nearby, capacity, node identity, and every m.col ray match RAM.
- `a_tossed_beach_ball_lands_bounces_and_rolls_as_the_originals`: a BeachBall is woken and thrown.
  - Each frame, the original's clear / collide / update / listeners run against `frame_among`.
  - Body, node, record flags, swept segment, relative velocity, cofg, normal speed, countdown, fresh
    flag, active bit and listener list are bit-exact every frame.
  - It lands on the terrain at frame 35, sleeps at 575 and is put back at 2376.
- `a_tossed_egg_slides_off_its_sculpture_and_lands_as_the_originals`: the same check for an egg
  thrown onto its scene mesh.
  - It spends 49 frames on an object (asserted) and lands at 93.
  - It sleeps at 1321 and is put back at 3122.

## Not verified / not 1:1

- **The knock is not a captured one.** The wake and throw velocities are the test's choice, made on
  the captured state; everything after the throw is the original's code. The brief asks for a
  captured knock. A live knock needs the bike to strike the prop's f.col, which S197 (props struck)
  ports and S195 (a capture of a real knock) records, so the replay of a real knock belongs there.
- **The asleep wake-category filter is never exercised.** Nothing in the props' world carries the
  0x40 category, so an asleep body found no hits that the filter would keep. The probe mode and
  category-list changes (0x0011b070 / 0x0011b1b0) are modelled only as that filter, not as state.
- **The hit listener (0x0015acf0) is not ported.** A ray hit does not wake a prop.
- **Other props' f.col and the toys' meshes are absent** from the port's world and from the test.
  These are free09's trailers (free_trailerA/b.col) and the neighbouring props. The tosses were
  aimed clear of them, after a first beach-ball throw hit its neighbour (S197).
- **Only BeachBall and egg are run.** Crates, barrels, cans and the tool box are checked only at
  load (rays and record against RAM).
- **Sweep back is never exercised.** Every prop's sweep_back is false.
- **The nearby list is gathered fresh each search**, as for the wreck bike. The original keeps it in
  the record, so this is equivalent only if its refresh happens every frame as it did here.
- **Tick order in a ride.** Where props search and step relative to cars and toys within the ride's
  frame was not checked against the original. Here they run where S131's tick ran.
- **Signs** have no probes. Their record is filed and cleared, but never collides.
- **The rollback wire** leaves out the rays (a track constant) and takes them back from the live set.
  A snapshot taken on another track would be wrong.

## Next

The gate `./kit check --all` is green (2681 tests). The task landed with `--skip-audit`: `./kit audit`'s
7 leaks are all other tasks' addresses in plan/DONE.md (biplane_tire_*, radar_update), and they
are on main too. This task's own plan line (S199) was rewritten without addresses.

Filed S199: the hit listener and the asleep probe's state.

Follow-ups are already on the plan: S197 (props struck by the bike, rider, wreck bike and plane,
and other props' f.col) and S195 (a captured real knock).
