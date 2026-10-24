# 1 — teleport keeps the air (FINAL)

Task P141. 2026-10-23.

## What I did

A marker's Up onto another player now puts you where their machine is that tick, in the air too.

- `markers::Spot` has a new field, `flight: Option<Arc<Flight>>`: the plane a player flies, while it is
  unwrecked, off its wheels and more than `CLEARANCE` over the ground under it. `Spot::of_flight` builds the
  spot. Its `at` is where a bike sent there appears: `CLEARANCE` (20 ft) under the plane's model origin,
  upright, along the plane's heading flattened onto the ground plane. `Spot::at_rest` is true for a player who
  is down, and for a rider on the ground slower than `REST_SPEED` (2 ft/s).
- `Garage::spot` / `lane_spot` return `Spot::of_flight` for such a plane. A plane on or near the ground, or
  wrecked, still gives P104's ground spot under it, stopped.
- Bike onto a flying plane: `Rider::teleport_to` calls the new `Ride::launch`, which is `place_on` at the spot
  followed by the chassis velocity (and its body-frame copy and speed) set to the plane's body velocity. The
  rest of the teleport is unchanged: unwreck, ghost, trick machine cleared. From there the bike falls as it
  would off a jump.
- Plane onto a flying plane: the new `Flight::fly_as` does a `put_back` of their flight, moved `BESIDE` = 40 ft
  along their right wing (the first row of their orientation matrix). The teleporter keeps its own throttle,
  its stored pad, its spawn (where a wreck's recovery puts it back), whether it is flown, and its livery,
  which lives on the `Plane`. The wreck state is theirs, and theirs is unwrecked.
- Plane onto a moving or airborne rider: the new `Flight::launch`, i.e. `reset` at the rider's spot + 20 ft
  with wings level along their flattened heading, then the body velocity set to the bike chassis's. Its own
  throttle and spawn are kept. At a rider's speed the plane stalls and drops.
- Plane onto a rider at rest or down: `land` as before.
- The new `vehicle::arrive` makes that three-way choice. `Garage::markers` (offline) and `lane_markers`
  (online) both call it, so peers agree: every input is a spot taken from the seats' `Sim`s before the tick.
- Doc comments updated in markers.rs (the module docs, `Spot`, `fly`), rider.rs (`teleport_to`), vehicle.rs
  (`Garage::markers`, `Garage::spot`, `land`, `under`, `lane_spot`, `lane_markers`) and net/rides.rs.

Nothing to take from opentennis: it has no vehicles and no teleports.

Side note: /tmp hit the user's quota mid-session (45 G of Claude task output across the slots). I deleted
only this slot's finished sessions from before 10-23, about 1.9 G, and built with `TMPDIR=target/tmp`.

## What the original does

Nothing. The markers are the port's own (P88), and the original has no teleport. What has to match the
original is the physics after the placement, and that is the unchanged ported bike and plane code.

## How it was proved

`crates/openmx/tests/teleport_air.rs`, 4 tests on the disc's biplane over a flat floor. Each runs through the
d-pad: Right to the other player, then Up held until it fires.

- `a_bike_teleported_to_a_flying_plane_falls_from_under_it_at_its_velocity`: online spots from `lane_spot`,
  the target flying at about 300 ft and 128 ft/s. On the teleport tick the bike is exactly `CLEARANCE` under
  the plane's origin and has the plane's velocity bit for bit. On the next tick it is airborne
  (`ride.air.airborne`), unwrecked, within 23 ft of the plane, on the plane's heading (cos > 0.999), and
  between 85% and 100% of its speed. Two clones step to the same hash.
- `a_plane_teleported_to_a_flying_plane_flies_on_beside_it`: online through `lane_markers`, and offline
  through `Garage::markers`. On the teleport tick it is exactly 40 ft along their side row, with their
  velocity and quat bit for bit, unwrecked, and its own throttle (0, from the bay). The test then opens the
  throttle to theirs and flies both 2 s on gas. Result: 129.218 vs 129.233 ft/s, 0.00° apart, 40.2 ft between.
  The online pair hashes equal on two peers.
- `a_plane_teleported_to_a_rider_mid_jump_is_in_the_air_over_them`: the plane is at the rider's spot + 20 ft
  (within 1e-3), level, at the bike's velocity bit for bit, unwrecked and throttle shut. It is still in the
  air on the tick after.
- `a_plane_teleported_to_a_rider_at_rest_lands_by_them`: online and offline, the plane lands on its wheels at
  the rider's x/z, stopped, as before.

Still passing: `--test markers` (139), `--test markers_online` (18), game-sim `--test teleport_momentum` (4),
and the vehicle unit tests.

## Not verified / not 1:1

- Nothing here is checked against the original, because it has no teleport. The placements are the port's
  own design. Only the physics after them is the original's, and that code is unchanged.
- The bike does not keep the plane's speed for long. Its own air drag (the ported ride's) takes about 0.11% of
  v² per 30 Hz tick: 14 ft/s off 128 ft/s on the first tick, 1.3 ft/s at 40. So "moving at its velocity" is
  exact on the teleport tick and drag-limited after it. The test asserts ≥ 85% of the speed on the next tick,
  not equality.
- A bike sent to a plane takes the plane's heading flattened and stands upright, not the plane's full
  attitude. A banked or pitched plane's attitude would put the bike on its side or nose and wreck it on
  landing. Its spin is zero, not the plane's body rates.
- A plane sent to a rider is level, wings level, along the rider's flattened heading, with zero spin. The
  bike's pitch, roll and rates are not taken: `Flight::reset` stands the plane upright.
- The three distances are the port's own, picked rather than measured. `CLEARANCE` = 20 ft (biplane about 7
  ft from wheels to top wing). `BESIDE` = 40 ft (more than the span by eye). `REST_SPEED` = 2 ft/s. Nothing
  checks whether the plane's search, which sees bikes, could hit a bike 20 ft away on a later tick. Only
  "beside" was tested, not "behind".
- A plane on its wheels, a wrecked plane, or one within 20 ft of the ground is still the ground spot under it,
  stopped, for a bike. A bike sent to a plane on its take-off roll stands still under it.
- Plane onto plane takes their engine power, rng, noise and contact caches along with the motion. Only the
  throttle, pad, spawn and flown flag stay the teleporter's. Not checked: whether a 40 ft shift leaves a stale
  contact record for one tick (the test's two planes fly identically after it).
- Not exercised in the real shell window or online over a real network. Proved through the same functions
  `Seat::pass` and `net::rides` call.

## Next

Nothing. The task is complete.
