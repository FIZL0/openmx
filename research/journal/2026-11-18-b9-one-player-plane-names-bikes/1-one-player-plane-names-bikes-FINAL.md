# 1 — one player plane names bikes (FINAL)

Task B9. 2026-11-18.

## What I did

- `crates/game-sim/src/lib.rs`, `Sim::step`'s plane branch:
  - Builds `rider_contact::met_plane` over player 1 and the others, player 1 as `me`. Its `bikes`
    are players 1.., which is the order of the `flight_search::Bike`s the plane gets (`others`).
  - Always hands that `Met` to the plane's frame (before, only when a rider was downed). The
    downed riders' wreck search still fills the same `Met`.
  - After the frame, `rider_contact::pushed_riders` writes back the chassis the plane pushed.
- `crates/game-sim/tests/flight_bikes.rs`: new `the_one_player_planes_rays_name_the_states_bikes`.
- `research/oracle.txt`: a B9 block beside N22b's.
- Filed B9a: step the other riders while player 1 flies, feeding the plane's mirrors to them.

Nothing taken from `../opentennis`: it has no plane or bikes.

## What the original does

- In biplane.p2m2's state the plane has three bike candidates. Each one's owner (`+0x80`) is a
  collision record. That record's frame collidable (`+0x1f8`) is the same collidable, and its
  capacity is 11, the port's `CAPACITY`. This is the same as split's bikes (N22b).
- So a hit by the plane's rays names the bike's record (kind 0 on the plane's list), and files a
  kind-1 mirror on the bike's record.

## How it was proved

Save-state oracle, `the_one_player_planes_rays_name_the_states_bikes` (biplane.p2m2 RAM + disc):

- Checks owner, frame collidable and capacity for all three bikes.
- For 8 poses, one bike's collidable is moved next to the plane:
  - poke its transform's origin (`+0x120` row 3);
  - rebuild its centre with the original's bounds (`./kit fn 0x001edce0`);
  - refill the plane's nearby list with its own refill (`./kit fn 0x0015b210`), as its post does.
- Then the original empties the plane's and bikes' lists (vt `+0x74`) and runs the plane record's
  collide (`./kit fn 0x002708a0`).
- The port runs `Flight::search_alone` with the bikes named, as `Sim::step` names them.
- Result: 7 poses hit, 36 kind-0 entries in all. Every entry on the plane's list and every mirror
  on each bike's list matches, every field bit for bit, in order. `pushed` names only the moved
  bike. One pose misses on both sides.
- Ran with `--nocapture`: no SKIP lines.
- Also green: `flight`, `hash`, `fly_and_collide`, `c16_fly`, `biplane`, `plane_crash_pilot`.

## Not verified / not 1:1

- **The bike never resolves the plane's mirrors.** While player 1 flies, the port does not step the
  other riders. The original steps its bike world every tick after the plane (S99's clock). So the
  mirrors in `Met::struck` are dropped, and a pushed bike keeps its new chassis speed without moving
  until the plane ride ends. Filed as B9a.
- **The pushed chassis speed is not captured.** The plane's resolve onto a vehicle is
  `contact::resolve`'s vehicle side (S88, N22b). No capture has the one-player plane flown into an
  AI bike, so the speeds written back are not proved here.
- **Moved bikes are poked poses, not flown ones.** The test moves a bike into a parked plane. It
  proves the search and the naming, not a real flight's timing.
- **No Sim-level test.** No test runs `Sim::step` with a plane flown into an AI bike. The wiring is
  checked only by building and by the existing plane and hash suites staying green.
- **The plane's name on the bikes' lists** (`named` = the riders' count) is never read in one
  player, since no bike resolves here. It follows split's rule.

## Next

B9a.
