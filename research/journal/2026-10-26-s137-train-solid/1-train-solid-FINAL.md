# 1 — train solid (FINAL)

Task S137. 2026-10-26.

## What I did

- **Why the shell's ride met nothing.** Every suspect in the brief was ruled out first. The cars
  *are* in the world the ride searches: free06's 577 props, with the traffic's car meshes at
  `traffic.first` = 353 onwards (21 meshes: 13 train cars, then the dump trucks and scrapers). They have the
  right mesh and the right place: `Traffic::place` stands each prop's `to_world` / `from_world` /
  centre where the car is drawn, checked tick by tick against the car poses. The broad phase does
  gather them, since `Props::sweep` walks every prop. The shell's clock is not it either: every seat's
  Sim steps `traffic.frame` at the top of `Sim::step` / `step_bumped`. What is wrong is the probe.
  The port's ride has only one body probe against props: the ray from the centre of mass along the
  velocity (`ride.rs` `contacts`), about 2.8 ft above the rails at riding height. A train car's
  collision mesh is open underneath. On train_car_06, for example, the body runs from local y 3.1 to
  16.6, and the bogies are side faces only, from y 0 to 4.3, at |x| ≈ 4–5. So a bike crossing
  between the bogies runs its ray under the deck. The head ray (`air::HEAD`, from 4.06 to 6.67 up the
  bike) starts inside the car once the bike is under it, and every face it meets is then a back face,
  culled. In the user's repro (`--trace-at 2527,3714,280`, full gas) the bike first stops against car
  0's side, where the velocity ray does meet a side face. It falls (reason 2) and is put back, then
  rides in again where car 4 meets car 5. Car 5's front end sweeps over the bike there, and the bike
  comes out 35 ft west of the line at (2346, 135, 3748).
- **The fix** (`crates/game-sim/src/mover_contact.rs`, new). The bike's eleven body rays (the files'
  own, `BikeCol::rays`) run against every moving prop (`props[traffic.first..]`: the cars, then the
  animated toys) once a tick. They go through the same machinery as the rider-against-rider contacts
  (`rider_contact::gather`): a `contact_search::collide` record with capacity 11, `one_per_ray`,
  `ray_probe::pair` against each car's mesh where `place` stood it, and the broad phase's
  distance-less-radius gather. Each strike becomes a scenery contact (`other: None, mover: None`) on
  the bike's list. The list goes through `Sim::bump` → `Bump::resolve` → `contact::resolve_ridden` in
  every substep, so the scenery response, impulse included, is the one the rider contacts already
  use. The wreck test reads the same entries as `air::Other::Scenery` with reach = the ray end's
  model y (`mover_contact::wreck_contacts`, appended to `hits` in `Rider::frame_bumped`). The
  original's `hit_wreck` takes the first own entry reaching above 6 and gives reason 6.
- **The head ray inside a car** (`mover_contact::inside`, the port's own). A car's *end* sweeping into
  a bike, or a bike squeezed into a coupling, leaves every ray starting inside the mesh, so nothing
  strikes. The highest-reaching ray (ray 8, the head) is therefore tested against each car's root box.
  If its end is inside, a contact goes on the nearest face (a side, an end or the roof, never the
  floor). With only the rays, the repro still ended inside car 5 at the coupling (tried first).
  With the box test it is thrown with reason 6 and stays east of the line's far rail.
- Wired into all three steps the shell's ride uses: `Sim::step` (offline, `vehicle.rs`),
  `Sim::step_all` (split screen, merged after each rider's list) and `Sim::step_bumped` (online,
  `net/rides.rs`). A step with no car contacts takes the old `rider.frame` path unchanged, so no
  existing hash or replay pin moves: game-sim's tests have no `bike_col` or no traffic.
- `crates/openmx/src/main.rs` needed no change. Its `--trace` dev loop calls `rider.frame` directly,
  not through Sim, so `--trace` rides still go through the cars (see Not verified).
- Tests (`crates/openmx/tests/train.rs`): the binary headless on free06.
  1. The user's path, full gas for 240 ticks. It ends less than 6 ft past the line and a reason 6 is
     said.
  2. A bike standing on the rails ahead of car 0, facing it, nothing held. Car 0's nose throws it
     with reason 6 at about tick 50.
  3. A bike stood inside car 0 at the first tick. Reason 6 at once.
  With the contacts switched off in a scratch build, the three runs end (2346, 135, 3748) after
  reason 2 only, (2386, 135, 3780) with no wreck, and (2407, 135, 3902) with no wreck. So all three
  tests fail on HEAD. A forced-wrong assert proved they ride the disc and do not skip.

## What the original does

The bike's collision object carries eleven body rays (`bike_col::BODY_RAYS`). The world search
strikes them against what is gathered round the bike, and every strike is a contact on the
collision record (capacity 11). The record is resolved per substep, with the scenery response for a
contact on no object. `hit_wreck` reads the record: the first own entry whose ray end is above y 6 is
a scenery wreck (reason 6). This is the reading S78 and S81 already ported, and `rider_contact.rs`
does it for bikes. I did not re-read the original for the cars specifically. Whether the original
files its path cars as movers (`contact::Mover`, whose velocity the response would count) or as
plain collision objects is not known.

## How it was proved

Looked at in the shell only: three headless rides on free06 (`crates/openmx/tests/train.rs`), each
compared against a scratch build with the car contacts off. None of it is compared against the
original: there is no capture of the original's bike meeting the train.

## Not verified / not 1:1

- **Cars as scenery, not movers.** The contacts have `mover: None`, so the response clears the
  bike's velocity into the face as against a wall and ignores the car's own 1.8 ft/tick. Reason: it
  is unknown whether the original's path cars are movers. A bike the nose meets is thrown (reason
  6), not carried. No capture of the original meeting the train exists.
- **The body rays search only the moving props.** Static props are still met only by the ride's
  velocity ray and head ray, as before. Reason: scope (the brief is the train), and running them
  against all 577 props would move every pin that rides near a prop. Filed as a plan follow-up.
- **`inside`, the head-ray box test, is the port's own.** It stands in for whatever the original
  does when a car sweeps over a bike (probably the chassis sweep back, which is not ported). The box
  is the mesh's root node box, not the mesh, so a car's open ends or underside count as inside.
- **When and in what order.** The car contacts are gathered once a tick, after `traffic.frame`,
  from the bike's pose at the end of the last tick, and appended after the rider-against-rider
  entries. The original's order on the record, and whether the cars move before or after the search,
  were not checked.
- **The `--trace` dev loop** in `main.rs` calls `rider.frame` without Sim, so `--trace` rides still go
  through the train. Reason: it is a diagnostic loop that does not go through Sim for any contact
  (rider contacts neither). The shell's ride, offline and online, does.
- **Roof.** No shell test lands a bike on a roof: `--trace-at` stands a bike on the terrain, and the
  port's wheels ride the terrain only (`World::ground`), never a prop's top. Coming down on a roof is
  met by the velocity ray (all props, as before) and by the body rays. Standing on a moving roof is
  not ported, for cars or for static props.
- **The thrown bike's chassis** stays where the wreck froze it, which at a coupling is between two
  cars that then run past it. The wreck bike is not pushed by the cars.

## Next

Nothing for S137. Follow-ups filed: S139 (the body rays against static props) and S140 (the cars as
movers or not, once the original's filing is known).
