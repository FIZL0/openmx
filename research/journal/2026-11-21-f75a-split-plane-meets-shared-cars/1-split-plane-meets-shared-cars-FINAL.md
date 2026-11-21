# 1 — split plane meets shared cars (FINAL)

Task F75a. 2026-11-21.

## What I did

- game-sim `parked_wreck.rs`: the Sim's `parked_in_plane` / `parked_from_plane` bodies moved into free
  functions `cars_in_plane` / `cars_from_plane` on a slice of parked cars, which the Sim methods call.
  So split screen's shared copy (F72) can use them. The Sim's own lane is unchanged.
- openmx `World::simulate`: with exactly one flown plane (one with a hull), the shared parked cars meet it
  as the Sim's plane lane does (S486a, S489):
  - the cars are not framed at the pass's start;
  - before the flying seat's pass, `wreck_plane::search_downed_parked` puts their hulls and bodies first
    in the plane's met and searches the other seats' downed wreck bikes with the cars' hulls in their world;
    cars a wreck bike struck are woken; the cars go into `Met::cars` for the frame;
  - after the pass the cars come back out, `step_downed` runs (always, so the met's wrecks are the cars'
    bodies), `cars_from_plane` writes the bodies back and wakes what the plane's rays struck, and the cars
    that stepped to rest are put to sleep after the bikes (the existing `World::parked_back`), as
    `Sim::step`'s plane lane does.
- Two planes flown together (`fly_held`): kept as before (cars framed at the pass's start, not in the
  planes' world). game-sim's joint frame (`flight::frames_together`) has no handling for `Met::cars` or
  `Met::hit`, and changing it is outside this task's files. Filed F75a1.
- Did not look in ../opentennis: it has no planes or parked cars.

## What the original does

One dynamics world for the plane and the parked cars (S489, R48): the plane's rays strike a car's hull,
which wakes it; a woken car searches and steps in the plane's world. Split screen has one copy of each
parked car (F72). No new reading of the original in this task.

## How it was proved

- New test `split_contact::a_split_seats_flown_plane_wakes_and_pushes_the_shared_truck` (openmx main.rs,
  disc truck and biplane, flat ground): seat 0 flies at the parked truck at 40 ft/s, seat 1 stands 100 ft
  off. The truck wakes at tick 19, is shoved 7.1 ft along the plane's way, and the plane is held at
  z 19.95; both seats hold one truck. With the change turned off the truck never wakes and the plane flies
  through (z 26.3): checked by hand by switching the lone-plane test off and rerunning.
- The other split-screen tests (15, `split_`) and c16_fly.rs's parked-truck tests pass.

## Not verified / not 1:1

- Not compared tick for tick with a lone Sim's plane lane: split screen flies the plane through the
  garage (`Plane::step_traffic_shared`), not `Sim::step`'s plane lane, so the two never line up
  exactly. The test only checks the meeting happens (wake, shove, plane held).
- Not checked against the original in split screen: there is no capture of a split-screen plane and a
  parked car.
- The other seats' downed wreck bikes meeting the cars in the plane's world are not covered by a test
  of their own. They go through the same game-sim function as the Sim's lane
  (`search_downed_parked`, proved by c16_fly.rs `a_wreck_bike_in_the_plane_lane_wakes_a_parked_truck`).
- Two planes flown together still step the cars at the pass's start and miss their hulls (F75a1).

## Next

Nothing for this task. F75a1 for two planes flown together.
