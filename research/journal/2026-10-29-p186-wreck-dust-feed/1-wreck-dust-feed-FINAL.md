# 1 — wreck dust feed (FINAL)

Task P186. 2026-10-29.

## What I did

- `ragdoll_contact::Contact` has a new field, `surface: u8`: the sphere's zone byte.
  `Contact::solve` writes it from `World::wheel_surface` at the bone's centre each time it runs the
  ground query (the bone strictly inside the terrain's box), whether the bone touches or not. Where
  the query has no cell, the byte keeps its old value.
  - The snapshot wire leaves it out (`skip { surface }`). Only the particles read it, and every solve
    over the terrain writes it before they read it. Leaving it out means the wire, RIDERS_WIRE and the
    netplay rounds stay as they were.
  - Test files that build a `Contact` literal got `surface` (ragdoll_cars, ragdoll_props,
    ragdoll_wrecks, ragdoll_riding).
- main.rs: `constraint_frames` now hands each sphere's `c.surface` to Rider Dust/Chunks/Foot Dust as
  its zone. It used to run the P61 ground query (`plane_fx_ground` → `World::surface`) at the
  contact point, and only for a touching sphere. `bike_fx_frame` and `constraint_frames` lost the
  `place` argument they no longer use.
- Vehicle Dust's velocity: nothing to change. The shell already feeds `crash.bike.collider.relative`
  (P156, through `PlaneFrame::touch` → `contact_vel`), and that is the original's +0x1e0. The
  `frame.vel = crash.bike.body.velocity` line that P153's journal flagged is only the fallback when
  there is no `contact_vel`, and `touch` always sets `contact_vel`. The new replay below proves it.
- research/oracle.txt: the bone collision's +0x3c is now described as the ground query's surface byte.
  It used to say "ground record".
- Extracted P153's `wreck-3790.p2s` (a zip) into `context/capture/p153/wreck-3790.ee.bin` and
  `.spr.bin` (its eeMemory.bin and Scratchpad.bin), in s1 only.
- opentennis: nothing to take, since it has no ragdoll or particles.

## What the original does

- The bone collision's solve, `./kit fn 0x001f7550`: when the bone is strictly inside the terrain's
  x/z box, it calls the terrain's ground query `./kit fn 0x001dc960(terrain, &c+0x40, &c+0x50, 0,
  &c+0x3c)`. The fifth argument (`t0`, set in the call's delay slot: `./kit dis 0x001f7550`, 0x1f7600)
  is the collision's +0x3c byte. This is done before the touch test, so a bone that does not touch
  still gets its byte written.
- The query writes that byte (`*(u8*)param_5`) only after it has found the cell. It writes the cell
  corner nearest the point, by squared distance in cell units, and takes the first corner on a tie.
  This is the same write the wheel's ray gets, already ported as `contact_search::nearest_surface`
  (S89) and used by `Place`/`Tiled::wheel_surface`. When there is no cell, the query returns without
  writing.
- The scene's contacts and the sweep never write +0x3c. Nothing clears it. Spheres that are never
  solved keep 0: the capture's spheres 2–8 hold 0 on every row from 3700 to 3725, before the ragdoll
  starts.
- The particle manager's ParticleCollisionConstraint reads this byte as the sphere's zone (P153: sphere
  +0x3c).
- `CollisionModel` (Vehicle Dust/Chunks) reads the collision object's +0x1e0. That is the relative
  velocity at the last resolved contact (`collision_relative_velocity` in oracle.txt). It stays stale
  while there is no contact: the capture holds 31.224, 9.101, 38.420 from 3799 to 3833 with
  `body_n` 0. The collision object's own rigid body is at its +0xf4. In this state that body is not
  0x1040 below the object, as it is in the s62 states.

## How it was proved

- **Function oracle, 10,000 cases** (`tests/ragdoll_contact.rs` `the_solve_matches_the_original`,
  extended). The hooked ground query now also writes a random surface byte through `t0`, or writes
  nothing in a quarter of the cases. The collision's +0x3c starts each case with a random byte. The
  test checks the port's `Contact::surface` against the original's +0x3c after the solve, and they
  are equal in every case. The byte changed in 3,510 cases: `Seen { ground: 3049, scene: 4711,
  swept: 2311, off: 656, surface: 3510 }`. The surface bytes come from their own random stream, so
  the existing cases are unchanged.
- **The live capture, zones** (`tests/wreck_bike.rs` `the_ragdolls_sphere_zone_is_its_own_ground_querys`).
  It covers every row of P153's `context/capture/p153/wreck.csv` (animandpump's wreck, 230 rows) and
  every sphere that was solved: 1,888 sphere rows, 445 of them touching. On each one,
  `Place::wheel_surface` over free09's terrain, asked at the sphere's captured point, answers the
  sphere's captured byte. The capture has no scene contacts (the test asserts this), so the point's x/z
  are the query's own.
- **The live capture, velocity: bit-exact replay** (`tests/wreck_bike.rs`
  `the_wreck_bike_feeds_the_dust_the_originals_contact_velocity`).
  - The port's `WreckBike` starts from the RAM image of `wreck-3790.p2s`. The test reaches the
    collision object through the human's rider: its particle manager's Vehicle Dust entry +0x28. The
    body is the object's +0xf4. The test checks the state's +0x1e0 against row 3790.
  - The bike is then ticked with `tick_among` over free09's ground and props, with the capture's dt,
    from 3791 to 3917, the last row before the respawn.
  - `collider.relative` equals the captured +0x1e0 to the bit on all 127 ticks, and it changes on 57
    of them. The value is exactly what `bike_fx_frame` hands Vehicle Dust.
- `./kit check` on wreck_particles, bike_particles and biplane_particles (26 tests), and on
  ragdoll_contact, ragdoll_wrecks and ragdoll_riding, all pass.

## Not verified / not 1:1

- **The shell itself was not run on a wreck.** I did not compare the particles `bike_fx_frame`
  builds from a live `./openmx` ride to the original's. The proofs cover the two inputs it hands on:
  `Contact::surface` (the oracle plus the capture's points) and `collider.relative` (the bit-exact
  replay of the same wreck). The rest of the feed is unchanged from P153/P156. A live shell wreck
  never rides the original's wreck: the tapes are not frame-stable.
- **This capture cannot tell the old feed from the new.** Every touching sphere in it is on free09's
  dirt (zone 13), and the old query (`World::surface` at the point) gives 13 there as well: 0
  differences in 445. The two differ only near a surface border (triangle surface against the nearest
  corner), on a scene contact's point, and on a non-touching sphere. The function oracle covers the
  rule the new feed follows. No captured wreck shows the two feeds giving different zones.
- **The zone-off path (0xff) is still not seen in a capture.** This is P153's gap and it is unchanged.
- **The byte is left off the snapshot wire.** After a rollback restore, a sphere outside the terrain's
  box would read 0 instead of its stale byte, until its next query inside the box. This is
  presentation only, and no free09 bone leaves the box.
- **The replay ran without trees.** It used `tick_among(…, props, None)`. This wreck met no tree; the
  bit-exact run shows that. A wreck into an ecosystem trunk is not covered by this check.
- **The wreck capture and its RAM image are in s1 only** (`context/capture/p153/`). In other slots the
  two new tests skip, and they still read as passing.

## Next

Nothing for P186.
