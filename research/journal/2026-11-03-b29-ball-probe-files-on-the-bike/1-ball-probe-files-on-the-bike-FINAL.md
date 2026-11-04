# 1 — the ball's probe files on the bike (FINAL)

Task B29. 2026-11-03.

## What the original does

f27a1's image 2904 (the tick after 2903) has these lists:

- ball 2: first the mirror of the bike's ray strike (kind 1, which the port already matched). Then
  its own probe's entry on the bike: kind 0, from (0,0,0), on_body (0,-4.35,-4.35), normal
  (-0.061,0.995,-0.084), point (5043.06,127.64,3552.56).
- the bike (`rider + 0x7d0`): its own entry on the ball (kind 0), then the mirror of the ball's
  entry (kind 1, normal turned round).

The bike is filed on the props' node ahead of the balls (S197). The node collides each filed object
in order, so a ball's probe searches after the bike, and its hit on the bike's frame collidable goes
on its list after the mirror. The hit's mirror goes on the bike's list after the bike's own entries.
Then the world step resolves both lists: the bike's update resolves its mirror against the ball's
body, and the ball's force listener resolves its entry against the bike's chassis.

## What I did

- `flight_search::bikes_probed`: `bikes_struck`'s pair test (each ridden bike's `bikef.col` on its
  chassis' transform, one contact a ray), answering the ray contacts too. `bikes_struck` is now that
  function's keys, so what is heard is unchanged.
- `Collection::search_object_hulled` (prop_world.rs): after the probe's own search, still hears each
  bike struck (F27a2). Where `heard.1.file` is on, `file_on_bikes` also adds each hit as
  `contact_search::collide` files a hit on another collision object: kind 0 on the prop's record,
  naming the bike by its player, with the record's touching / on_object / touching_other flags and
  `struck = BIKES + j`. Its mirror (kind 1, normal negated, other side not named yet) is kept in
  `heard.1.probed` with the object and the player. The pass empties it (`heard_afresh`, and per
  object when it searches).
- `prop_contact::strike` (now `rays_strike` + `probes_struck`): after the bike's own entries, each
  mirror that this bike's player was struck by goes on the bike's list. Its other side is the
  prop's body: the struck one's place if the rays struck it too, or else a new place in
  `Strike::probed` after the struck ones. Capped at the bike's `CAPACITY`. `PropSet::strike` hands
  a probed body to the bike's resolve (`Pushed`) after the struck ones, without waking it or
  touching its list. A strike that is only mirrors still makes a bump, because its `contacts` is
  not empty.
- `PropSet::file_ridden(on)`: the ball's entry names the bike by player, so filing is on only where
  the props' step has the players' chassis as its strikers by player: `Sim::hear_ridden` (every
  bike lane, off while the plane flies, whose step has the plane as striker 0) and the shared seats
  (`prop_seats::Shared::take`). Off is F27a2's behaviour, heard only. The props of the Kiowa and the
  physics car never turn it on.
- `flight.rs`: the plane's `Strike` literal takes `..Default::default()` for the new field.

Nothing else had to change. The port's generic resolve already does the rest: the bike's bump
resolves the kind-1 mirror (no impulse, spring or friction), and the props' step resolves the ball's
kind-0 entry against the vehicle.

## How it was proved

- `tests/knock_launch.rs the_balls_probe_pushes_it_off_the_bike` (no longer ignored): image 2903
  one `Sim::step` against image 2904. Ball 2's body (velocity, position) and the bike's chassis
  velocity are bit for bit: ball vel (17.863506, 19.306181, 20.640755), bike vel (11.29749,
  2.2470372, 14.818376). `one_tick` now also compares, bit for bit:
  - ball 2's whole list, every word of every entry, arms included;
  - the bike's list up to its arms. The port does not keep the bike's list after its frame, so a
    second ride off the image runs the collide pass and the strike, and the strike's contacts are
    compared.
- The same check passes on 2904 → 2905 and 2905 → 2906 (`the_launch_goes_on_as_the_original`).
- `the_ball_flies_clear_of_the_bike`, 60 ticks from 2903 against `f27a1.csv`: the ball's speed,
  vy and y, the bike's speed and the gap match the capture to the printed two decimals on every
  tick, including the bounce at +32. Before, it landed 6 ticks early.
- `./kit check -- -p game-sim`: 1201 passed.

## Not verified / not 1:1

- **Where the bike's hits go among the prop's own.** They are added after everything the probe's
  scene search found. The original has the bike's collidable in the same search as the scene and
  the wrecks, in its broad-phase order, and one ray record over all of them, so a ray that meets
  both a mesh and the bike keeps only the nearer. Here a ray can keep both. f27a1 has only the bike
  hit on that tick, so this is not tested.
- **The bike's list order beyond one prop.** Mirrors go after the bike's own entries, collection by
  collection and object by object, in pass order. The original's order is the node's filed order.
  Only one probed prop is checked. The cap at `CAPACITY` counts only the props' entries, not the
  car contacts that are merged in later.
- **The bike's arms.** These are written by the bike's resolve, which the port does not keep, so
  they are not compared. The chassis velocity, which comes out of them, is bit for bit.
- **The strike tick of a just-filed prop.** Its probe still only hears the bike in that tick
  (B28). Its contacts wait for the next pass, which is B32.
- **The plane lane.** While the plane flies, the props' probes still only hear the other riders'
  bikes, because that step's striker is the plane alone. Not compared with the original.
- **Two riders and shared seats.** These go through the same code, but only f27a1's single rider
  is compared with the original.
- **opentennis.** Nothing there is like this, so nothing was taken. The PC exe was not read: the
  bit-exact images are the proof here.
