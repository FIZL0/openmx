# 1 — ai riders ride while flying (FINAL)

Task B9a. 2026-11-18.

## What I did

- `crates/game-sim/src/lib.rs`, `Sim::step`'s plane branch:
  - Before the plane's frame, gathers the bikes' contacts with the plane's hull in their world
    (`rider_contact::contacts_met`, as split does, N22a). Player 1's bike is out (not seated).
    The hull's mirrors go on the plane's `Met` (`met_plane`'s `mirrors`, empty before).
  - `wreck_plane::step_downed` now runs with `own_step = true`: the downed rider's own frame comes
    later in the tick, so its wreck bike's body tick is not run twice.
  - After the plane (and its props' step and the race generator swapped back), the new
    `Sim::step_flown_riders` steps every other rider, last player first, as `step_all` walks them:
    - list = `struck_first(plane's mirrors on its record, its own list)`, then its car strikes;
    - its bump has the plane's body (`Flight::flown`), written back after (`Flight::pushed`);
    - rivals leave out player 1 (its bike is out of the world);
    - AI levers, rng lent, props lent for the wheels, brain thinks after, dust, race card bike
      counts, bumped others written back, wreck push back, seam wrap, cards fed.
- `crates/game-sim/tests/flight_bikes.rs`: two tests (below).
- Filed C44 (capture proof) and S479 (what the plane lane still skips for the riders).

Nothing taken from `../opentennis`: it has no plane or bikes.

## What the original does

- The plane's collision node in biplane.p2m2's state: `0x0060b200` (its dynamics world at `+0x78`
  is S99's `0x0060b278`). Count at `+0x30`, records at `+0x38` (`./kit dis 0x0026f800`, accessor
  `0x00273860`).
- Its 5 records, in order: the plane's `0x01e73b00`, `0x018c1bd0`, AI bike `0x019a9790`,
  `0x01970060`, AI bike `0x007ffdf0`. Player 1's bike record `0x018bfc30` is not on it.
- The node clears every record, then collides them in that order. So the plane's mirrors land on an
  AI bike's empty list before the bike's own search appends: mirrors first.
- S99's live trace: the bike world (`0x0030f7ec`, world `0x00a83830`) steps after the plane's post
  every tick. So the bikes resolve after the plane, with the plane's mirrors on their lists.

## How it was proved

- **Save state** (biplane.p2m2 RAM + disc), `a_bike_the_plane_struck_lists_its_mirrors_ahead_of_its_own`:
  - asserts the node's first record is the plane's and both AI bike records are on it;
  - for B9's 8 poses: lists cleared, the plane's collide, then the moved bike's collide; the bike's
    list equals `struck_first(mirrors, own)` bit for bit, where `own` is the bike's collide alone on
    a second image;
  - 7 poses struck (2, 4, 4, 3, 3, 1, 11 mirrors); pose 7 fills the record's 11.
- **Port only**, `the_bike_a_one_player_plane_strikes_rides_on`: `Sim::step` with the plane flown at
  another rider's bike. The rider is stepped every tick; the plane pushes it at tick 7 and it rides
  on along the push (z 30 → 84.7 by tick 39).
- Also green: `c16_fly`, `prop_plane`, `flight_bikes`, `wrap_moving`, `flight`, `snapshot`,
  `presets`.

## Not verified / not 1:1

- **No capture of the plane flown into an AI bike.** The task asked for one. None exists, and making
  one means aiming the plane at a moving AI bike and seeding AI brains from RAM, which no test does
  yet. So the bike's motion after a push is not compared to the original. Filed as C44.
- **The bike's own list was empty in every pose.** Only the bike's collidable was moved into the
  plane, so its own rays stayed where the bike was and hit nothing. The test shows the mirrors
  survive the bike's own collide, but not mirrors *and* own entries together, nor the room running
  out. The append order itself is the general rule N22a1 proved on split's node.
- **The bikes' hits on the plane's hull in one player** (`contacts_met` with the hull) follow split
  (N22a, proved on the 2p state). Not checked in the 1p state.
- **What the riders skip while the plane flies.** Their bikes do not strike the knockable props.
  Their bodies' look-around, engine notes and the race manager's tick do not run. The plane lane
  skipped these before too. Filed as S479.
- **Downed riders are now stepped whole while the plane flies** (their wreck bike's body tick is
  still `step_downed`'s). This follows split's `own_step = true`. No capture has a downed AI rider
  under a flown plane.
- **The AI brains while player 1 flies** read player 1's bike as one of the ridden bikes, where it
  was parked. Not checked against the original.

## Next

C44, S479.
