# 1 — bike particles (FINAL)

Task P156. 2026-10-28.

## What I did

- `bike_fx_frame` (crates/openmx/src/main.rs): while the rider is down, the wreck bike's frame now
  comes from `PlaneFrame::touch(&crash.bike.record, crash.bike.collider.relative)`, the same call
  `plane_fx_frame` makes (P63). Before this it built the contact lists itself and left `contact_vel`
  `None`, so the `CollisionModel` entries (`Vehicle Dust`, `Vehicle Chunks`) spread their points
  along the body's velocity. It also filled `object_contacts` with every point when `on_ground` and
  `on_object` were both set. `touch` keeps one point in that case: the object's flag is raised and the
  ground's list is the only one emitted. `frame.vel` stays the body's velocity, which the other entries
  read.
- Test `the_wreck_bike_puffs_off_its_contacts_velocity` (crates/openmx/tests/bike_particles.rs): the
  disc's `wreckbike.phy` part (out of a free09 archive) is built with `WreckBike::new`, thrown spinning
  onto `world::Flat`, and run with `WreckBike::tick` against `NoObjects` for 90 ticks at 1/30 s. Each
  tick builds the frame the way the shell now does and steps it through free09's two `CollisionModel`
  entries. Result: first contact at tick 3, first puff at tick 4 (the switch-on tick only takes its
  point, P153), 21 particles in the air at the end. Every new puff lies within `|contact_vel|·dt + 2 ft`
  of one of that frame's contacts, and `contact_vel` differs from the body's velocity by up to
  22.6 ft/s, so the test does tell the two velocities apart.
- I took nothing from opentennis; it has no particle system like this one.

## What the original does

P63 (journal `biplane-particles`) covered the entry this feeds. A `CollisionModel` entry reads the
vehicle's collision object. That entry's points come off the object's relative velocity at its last
resolved contact (`Collider::relative`, collision object `+0x1e0`), and it emits once per contact of
the object's one list. The wreck bike's collision object is the same kind of object as the plane's
(`wreck_bike.rs`'s `collider`, resolved by `contact::resolve`, which writes `relative`). The bike's
manager reads it through the same entry type.

## How it was proved

- The entry, with `contact_vel` and the both-flags case, is already proved in
  `biplane_particles.rs` (P63's unit test and the disc plane dive).
- The new disc test shows the wreck bike's own collision object drives the entry: puffs come only after
  the first contact and start at that frame's contacts, along the relative velocity. The relative
  velocity comes from the ported `WreckBike` (bit-exact against captures in game-sim's
  `wreck_bike.rs`).

## Not verified / not 1:1

- **The shell's `bike_fx_frame` itself is not run by a test.** It lives in the binary, which
  integration tests cannot import. The test copies its two lines (`vel` = body velocity, then `touch`
  with `collider.relative`). The change is just that call.
- **No capture of the original's wreck dust positions was compared.** P74's `run.csv` records the
  manager's flags and switch bytes, not the CollisionModel particles' start points. I did not record
  a new capture. The claim that the bike's entry reads `+0x1e0` rests on its being the same entry type
  P63 proved for the plane, and the task brief says the same.
- **The test's ground is the dirt by hand.** `world::Flat`'s contacts carry surface 0, the road,
  which free09's zones keep dust off. The test checks that every zone is 0 and then sets them to 13.
  Real track surfaces come from the search, as before. This task did not change that.
- The both-flags (`on_ground` and `on_object`) path is not run with the wreck bike: `NoObjects` never
  raises the object's flag. It goes through `touch`, which P63's unit test covers.

## Next

Nothing.
