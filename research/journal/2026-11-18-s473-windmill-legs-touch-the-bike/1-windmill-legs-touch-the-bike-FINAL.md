# 1 — windmill legs touch the bike

Task S473. 2026-11-18.

## What I did

- Brief: from the tick after the entry (csv row 5) the original's bone collisions 7 and 8 touch
  (touch word 384), the port's touch nothing, and the bones drift 1.4 ft/s low in y.
- Logged the port's ride per tick: which bones the collisions are on, the bike search, and how far
  each bone is from the wreck bike's mesh.
- Collisions 7 and 8 are bones 13 and 16 (the legs, radius 0.75). On that tick both the port's and the
  original's legs are ~0.6–0.9 ft from the wreck bike's mesh (the port's mesh and frame). So the thing
  touched is the wreck bike, as the brief guessed.
- The port never tested them against it: the bike's search distance was 2.89 ft, and a sphere is only
  paired with the bike when that distance is within its radius.
- That distance is measured from where collision 0's sphere stood. The port kept the sphere's *first*
  stand of the frame (`stood.first()`, the first substep). The original keeps its collider's place,
  which every solve rewrites, so it is the *last* stand (the fourth substep), ~2.3 ft further along at
  this speed.
- ragdoll_ride.rs: the search's probe is now collision 0's last stand of the frame (the stand that
  leads the last pass). With it the distance on that tick is 0.61 ft: the 0.75 ft spheres near the
  bike (7 and 8) pair with it and touch; the 0.4 and 0.5 ft ones do not. Exactly the original's 384.
- tests/replay.rs: `B19Bones` carries each tick's touch word. The windmill test now checks the port's
  touch word against the original's on every tick, and the pins are re-measured (below).
- Nothing taken from opentennis (it has no ragdoll).

## What the original does

- The bone collision's solve (`./kit dis 0x001f7550`) puts the sphere collider at the bone on every
  solve (`0x001f0940`), before it collides it against the search list. The world steps 4 substeps a
  frame at dt 1/30 (rate 115), one pass each, so the collider ends the frame at its fourth stand.
- The bones' search gathers once a frame, round collision 0's collider where it stands then: its last
  stand of the frame before. S105's proof reads that place straight out of RAM (+0x40), which is why
  it never saw the ride's first-stand probe.

## How it was proved

- A save-state capture compared tick by tick: context/capture/s466/south3-req.csv (S466's per-tick
  request root and holds), in tests/replay.rs
  `the_windmill_strikes_the_ports_bones_left_at_its_face`.
- The port's touch word equals the original's on all 23 ticks (384 on tick 4, 0 elsewhere).
- Worst bone off the csv: 0.072 ft on tick 4, 0.203 on tick 11, 1.29 on tick 23 (before: 0.21, 1.31,
  1.99). Mean bone velocity within 0.4 ft/s of the original's in every lane on every tick (before:
  1.4 ft/s low in y from tick 4).

## Not verified / not 1:1

- The fix is proved on this one ride (one wreck, one touch). No other test checks the ride's search
  probe against the original's per frame. tests/ragdoll.rs `the_rides_bones_gather_the_wreck_bike_once_off_it`
  (a port-only synthetic wreck, no capture) now first lists the bike on tick 58, not 43; re-pinned.
- tests/one_fps.rs (openmx) fails here with and without this change: "1 bytes read" after the first
  frame. Not this task's; filed as K79.
- The drift from tick 19 on (0.57 ft on tick 19 to 1.29 on tick 23, the mean z velocity +0.3 ft/s
  throughout) is not explained. The skeleton is still held at the image's pose each tick (as in
  S466), which may be it; not checked.
- The "last stand" is found by counting the spheres that are on (each pass stands each of them once,
  in order). The original keeps the place on the collider; the two agree as long as no sphere turns
  on or off inside a frame's step, which nothing in the port does.

## Next

- None for S473. S474 (the shell's drawn root) is still open.
