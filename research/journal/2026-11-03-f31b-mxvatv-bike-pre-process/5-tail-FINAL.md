# F31b4a: the sim frame's tail around the bike

2026-11-04. Task: `./kit plan F31b4a` (filed by F31b4).

The frame's tail is the sim's last pass: it walks the listener tree and steps every listener that asks
for it. The bike is one of those listeners, and its step is the vehicle update. Both are now
`game_sim::handling::tail`, and `tests/mxvatv_bike_ride.rs`'s `port_record_frame` runs the port's tail
(`port_tail`) where it used to call the original's. Addresses, slots and offsets are in research/oracle.txt
under `# F31b4a`.

## What it does

- `tail::walk(listeners, group, dt)`: a group with its step bit walks its list. A listener steps when its
  state byte says on, not held and not gone, and it wants the step. The next pointer is read after the
  step, so a listener that cuts the list ends the walk where it cut it. A group listener whose step is the
  walk itself recurses (in the test, through `TreeInRam`).
- `tail::vehicle_update`: the bike's step. Without a state, or when skipped, it does nothing more than the
  original does. Otherwise it runs:
  - the fade and its countdown (fade over, with or without an object);
  - the wreck test, then the inner update and the base update. The base update always rides: the
    original's kind test is dead code (see oracle.txt);
  - the lean, prepare, ride, sync and snapshot slots;
  - the stuck timer (`tail::stuck`): it counts the time the wheels hold on a surface that holds, below
    the slow speed, and puts the bike back past the surface's limit;
  - the rumble (`tail::rumble`): soft or strong by the wheels' roughness, when the pad is ready;
  - the tricks: wheelie and stoppie clocks, distances by speed or heading, bests, trick start and end,
    landed;
  - the run: its clocks, top speed, the run's end, and telling the run's items and riders it was run over;
  - the network and parts calls and the frame message.
- Every call out is one `tail::Out` through the `Vehicles` trait. `tail::read` / `tail::words` are the
  bike's words, the same pattern as F31b1–F31b4's ports.

## Checked

- `the_bikes_vehicle_update_is_the_originals`: 12 000 random bikes, built from state 4's bike with every
  word the update reads drawn at random (state or none, kind, skipped, paused, clocks, speeds, wheels and
  surfaces, tricks, run and player or none, the fade object). Each runs through the original update and
  through the port via `TailInRam`, with every callee hooked and logged. RAM and the call log match.
  Every path is required to be taken at least 20 times. Counts include: down 3021, wreck test 5262,
  skipped 1607, paused 1112, no state 998, countdown over 3314, fade over 804, fade over with no object
  381, free 5690, stuck 1774, stuck and put back 2426, rumble 386, rumble strong 326, rumble not ready
  267, wheelie 1168, stoppie 1170, best distance 397, trick start 367, trick end 4017, distance by speed
  5229, distance by heading 2641, landed 2040, run over 760, items told 585, riders told 392, lap over 129.
- `the_tails_walk_is_the_originals`: 10 000 random trees (groups, nested groups, leaves with random state
  bytes and wants, leaves that cut the list). The steps taken match in order. Counts: stepped 3322,
  nothing stepped 6678, a group's own listeners stepped 270, three or more 250, a stepped leaf cut its
  list 973.
- `ride_replays_bit_exact_on_the_ports_record_pass`, `flip_wrecks_and_is_put_back_bit_exact_on_the_ports_record_pass`:
  the frame now runs the port's tail. The test asserts that it steps one bike and two other leaves every
  frame. At the bike, a shadow check runs the original update on a copy of RAM and the scratchpad, then the
  port, and asserts all of RAM (except the stack window) and the scratchpad are equal. Both replays stay
  bit-exact on every row, with the wreck on row 125 and the put-back on row 193.
- Mutations:
  - Dropping the gone check fails the walk oracle (tree 2).
  - Moving the first rider link from 4 to 0x18 fails the update oracle (bike 17).
  - Setting `landed` instead of or-ing it fails the update oracle (bike 8).
  - Changing the hang time `<=` to `<` fails the update oracle (bike 123).
  - Changing `y <= -1` to `<` fails the update oracle (bike 41).
  - Adding dt twice to a clock passes the oracle (its hooks do nothing with it) but fails the replay's
    RAM-wide check (one word).
  - Equivalent, so not caught: the rumble clamp 95 → 96, `!(t <= lim)` → `t > lim`, and swapping madd's
    operands.
- `./kit check -- -p game-sim --test mxvatv_bike_ride`: 20 passed.
- `./kit audit`: the same 4 leaks as on main (atv_wheel.rs, plan/DONE.md). None are in these files.

## Not verified / not 1:1

- Several calls get `f12` from the original in a state where `f12` is stale (fade over, wreck first, sync,
  snapshot, prepare, trick start and end, the run-over calls). The port passes dt. None of the hooked
  callees reads `f12`, and on state 4 the ones that run match bit-exact, but a callee that read it would
  see dt instead of the stale value.
- The original reads the node pointer before the inner update and its axes after it. The port reads the
  pointer again after the call. The two differ only if the inner update moved the node, which it does not
  on either replay.
- The oracle's hooks do nothing, so the port's re-reads after its calls are only covered by the replays.
- The replays take only part of the update: on every row the kind is 1, and only the fade, wreck test,
  lean, prepare, ride, sync, snapshot, network, parts and frame message run, plus the rumble on 34 rows.
  The wreck steps, fade over, put back, tricks and run over are covered by the oracle only.
- The other two leaves (one on 0x00eb6420 and one on 0x01ed82d0, see oracle.txt) and all of the bike's
  callees still run in the interpreter. Nothing yet shows that those leaves leave the bike alone.
- The shadow check leaves the stack window out of the RAM comparison, since the port's frames are not the
  original's.
- The rumble clamps cannot be reached, so only reading the code shows them.
- Only the targeted test file was run, not the full replay suites.
