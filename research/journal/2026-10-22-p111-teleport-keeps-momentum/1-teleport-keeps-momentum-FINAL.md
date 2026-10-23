# 1 — teleport keeps momentum (FINAL)

Task P111. 2026-10-22.

## What I did

- `markers::Spot` gains `motion: Option<Motion>`, `Motion` an `Arc<Ride>` of the target's bike as the
  tick before the teleport left it. `Spot::of` fills it for a rider who is up; a rider who is down
  (wreck or crash) gets the put-back record and `None`, as before. `Spot::still(at, forward, up)` is
  the no-motion constructor, used by the plane's spots in `vehicle.rs` (the floor under a flying
  player, P104) and by `tests/bays.rs`. `Spot` is no longer `Copy` (a `Ride` is not); the three places
  that moved one out of a slice take a `.clone()`. `PartialEq` for `Motion` is pointer equality or
  else the two rides' full hash dumps equal, so `Spot`'s `==` still means "the same bike, bit for bit".
- `Ride::ride_as(&other)` (ride.rs): take `other`'s whole ride, then put back what is this bike's
  build and this rider's own — the `.phy` numbers `Ride::new` copied (chassis inverse mass and inertia,
  centre of gravity, free axes, speed/angular limits; engine tuning, the same list `retune` takes,
  plus `torque_scale`, with `set_gear` re-run for the ratio; each wheel's radius, strut, spring,
  damping, steering numbers, spin inertia and gyro) and the spawn, the put-back record, the riding
  speed limit and the drag flag. On one build the result is the target's ride with only spawn/back
  different.
- `Rider::teleport_to` rides on as the spot's motion when it has one, and otherwise `place_on` as
  before; `unwreck`, the drag reset and `teleported()` (ghost on, tricks cleared…) are unchanged.
- The plane-to-bike switch (`Garage::switch`) also goes through `teleport_to`, with `under()`'s
  motionless spot, so it is unchanged: the bike is stood under the plane stopped.
- Online: nothing to change. `net::rides` builds every seat's spot from the sims before the tick
  (`lane_spot` → `Spot::of`) and the teleport runs inside the agreed tick from them, so every peer
  copies the same bytes. A spot's motion is not sent anywhere: it is derived from state every peer has.
- opentennis has no teleport; nothing taken from it.

## What the original does

Nothing: session markers and player teleports are the port's own (P88). Only the ride the copy
continues is the original's, and it is copied, not changed.

## How it was proved

`crates/game-sim/tests/teleport_momentum.rs`, ridden as the shell and the online session ride the
markers (spots before the tick, poll, step every rider, then player 1's markers), stand-in bike on a
flat floor at 30 Hz:

- `a_teleport_onto_a_riding_player_rides_on_with_their_motion`: player 2 runs up with gas and some
  steer; Up onto them leaves player 1 with their body velocity, angular velocity (local and world)
  and both wheels' spin bit for bit, on their spot, ghosted. Then both on the same controls for the
  ghost's 90 ticks plus 15: every field of player 1's ride hash dump but spawn/back equals player 2's
  of the tick before, every tick. A control run with the motion ignored fails on the first assert.
- `a_teleport_onto_a_downed_player_lands_at_rest`: player 2 with wrecks on is down (the stand-in's
  first-tick wreck); their spot has no motion, is their put-back record, and player 1 (moving) lands
  at rest.
- `a_still_spot_lands_at_rest`: a motionless spot sets a moving bike down stopped.
- `a_teleport_onto_another_build_keeps_the_teleporters_bike`: onto a heavier, lower-revving build,
  the velocities are taken and the mass, rev limit and spawn stay the teleporter's.
- `vehicle.rs`'s `a_flying_players_spot_is_under_their_plane_not_at_the_bike_they_left` now also
  asserts the flying player's spot has no motion and a moving rider teleported there lands at rest.

## Not verified / not 1:1

- **The teleporter rides one tick behind the target, not level with them.** The spots are taken
  before anybody's tick (so the result does not depend on seat order, and stays deterministic
  online), and the teleport runs after the teleporter's own step: the bike gets the target's state
  of the tick before, and given the same pads retraces their path one tick (1/30 s) late — its speed × 1/30 s
  behind, overlapped (the ghost covers it). Taking the state after the target's step would need the
  markers to run after every seat's step; not done, to keep the tick order every caller has.
- **Identical paths are proved on one build only, on the flat floor with the stand-in bike.** The
  disc's bikes and a real track were not ridden; the copy is of the whole ride, so nothing about the
  bike's model should change it, but no test shows it on disc data.
- **Across two builds the motion is not exactly the target's.** The build fields kept are the ones
  `Ride::new`/`Wheel::of` set from the `.phy`, picked by reading those constructors; a field the step
  derives from the build and stores (e.g. the wheels' world-space probe points, the chassis' `cofg_world`
  only recomputed when the centres of gravity differ) is the target's until the next step rewrites it.
  Only mass and rev limit are covered by a test.
- **The rider's own state (trick machine, rider state machine, animation) is not copied**, only the
  bike: the rider is reset as every teleport resets it. The same-build test passes field for field
  on the ride, so on the flat with a straight run that did not matter; a target mid-trick in the air
  is not covered (the teleporter lands mid-air with their motion but no trick).
- **The put-back record stays the teleporter's** until its next keep-back update, as `place_on` left
  it before; a wreck on the tick of the teleport would put back to the old spot.
- **Not run in the shell or a live online session**; the online path is argued from the code
  (`net::rides` spots before the tick) and covered only by the existing netplay tests passing.

## Next

Nothing; `./kit done`.
