# 1 — machines spawn on the player (FINAL)

Task P102. 2026-10-22.

## What I did

- **The switch names the machine wanted.** `Garage::swap()` is now
  `Garage::switch(to, sim, world)` in crates/openmx/src/vehicle.rs. Switching to the machine already up does nothing.
  - **To the plane:** the plane is put down at the player's spot (`markers::Spot::of`: the bike, or its
    put-back spot while wrecked). Its heading is the bike's forward flattened onto the ground plane. It
    stands on its wheels on the ground under that spot, at rest, as a parked plane stands: the origin
    sits `-min(tire centre.y - radius)` above the ground (`Plane::rest_height`), which is exactly how a
    machine.txt bay leaves it.
  - **To the bike:** the bike goes onto the ground under the plane, upright, along the plane's flattened
    heading, stopped (`Rider::teleport_to`, so it is also unwrecked and ghosted like any put-back).
  - **No ground under the spot:** the switch is refused with a note, and so is a track that parks no plane.
- **The plane keeps its bay.** `Plane` gains `bay`, and `spawn` now means where it was last put down. The reset
  chord and a crash put the plane back there.
- **The machine left is not drawn.** `Garage` gains `taken`; `plane_shown()` and `bike_shown()` are what the draw
  loops and the radar now read. A plane nobody has taken still stands in its bay, so other players'
  untaken planes do too. Once taken and left, it is drawn nowhere, collided with by nothing (nothing in
  the port collides with machines; planes are not props) and not stepped (only `up` steps).
- **The pause names the pick.** menu.rs's switch row is `< Biplane >` (left/right cycles through `Machine::ALL`
  minus the one up, among those the seat may take). OK returns `Act::Switch(seat, Machine)`. Online, the
  row answers "Online rides the bike: there is no switching machine". Online sessions carry no machine,
  so the switch was not made a rollback input there; see below.
- **The switch is an input at a tick in a tape.** game-sim replay.rs: `PadTape.switches`, text lines
  `switch <frame> <machine>` (after the `modifiers_from` lines, as P98's changes are), `Recorder::switch` and
  `Head::switches`. The shell records seat 0's switches (M key and the pause) and plays them back at their tick,
  before the tick's step, as P98's modifier changes are. The headless shell prints `plane hash` at the end.
- Cameras snap as before (`switch_machine` in main.rs).
- **A pre-existing red fixed in passing:** `the_pauses_modifiers_take_when_it_closes_unless_held` in tests/menu.rs
  (100 vs 90) was red at HEAD too (checked in a scratch worktree). P95 moved plane speed out of the menu
  (`Knob::MENU`) and the test still walked `Knob::ALL`. Two words changed.

From opentennis: nothing applicable (it has no machines to switch).

## What the original does

Oracle `freeride_switch_commit` / `machine_swap` (R8): the pause's "Switch To Machine" and the freeride prompt
call the same swap. It moves the controller to the new machine and hides the old bike's model **where it was
left**, with no transform written. The plane is the one machine.txt parked, unwrecked where it stands.
So the original has no spawn-on-the-player: walking back to the bike, or flying to it, is how it plays.
**P102's switch is the port's own** (user brief, 2026-10-22), and nothing here claims to be 1:1.

## How it was proved

- tests/ride.rs `a_switch_spawns_each_machine_where_the_last_one_was` rides the disc's free06 with the real
  biplane.phy:
  - **Bike → plane:** the plane comes up at the bike's x/z and heading, level, with zero velocity. Over two
    idle seconds it settles exactly as a plane parked on its wheels at its bay settles (moved 0.616 vs
    0.614 ft, sank 0.105 vs 0.105; tolerance 0.01). So "at rest as a parked plane stands" is measured
    against the parked plane itself, not against zero.
  - **Plane → bike:** after 30 s flown on Cross, the bike lands under the plane, on the ground, upright,
    along its heading, stopped. `plane_shown()` is None. The bike rides on, and the plane left is not stepped.
- tests/ride.rs `a_tape_across_two_switches_replays_bit_for_bit`: Recorder → text (contains
  `switch 180 biplane` and `switch 480 bike`) → parse → Head replay. Bike hash, plane hash and machine are equal
  tick for tick.
- tests/switch_tape.rs `a_played_tape_takes_its_switches_and_replays_bit_for_bit`: the binary headless on
  free06, `--play` of a hand-written 240-tick throttle tape with `switch 60 biplane` / `switch 180 bike`. It
  prints `vehicle: bike, biplane, bike` in order, and two plays end on the same `ride hash` and `plane hash`.
  I proved it actually ran (a passing check hides skips) with a temporary panic, which printed the ride: the
  plane took off from the bike's spot at 2889, 309, 2814 and the bike came back under it.
- crates/openmx/src/vehicle.rs unit test `a_switch_spawns_on_the_player_and_a_track_without_a_plane_refuses`
  (Flat ground). tests/machines.rs and tests/menu.rs were updated to the named switch.
- `./kit check`: TESTS PASSED (full gate). `./kit audit`: only the known plan/DONE.md:234 leaks of other tasks.

## Not verified / not 1:1

- **The whole switch is the port's own.** The original hides the bike where it was left and keeps the plane at
  its bay (see above), so there is nothing to compare against in PCSX2. The behaviour is checked only by the
  port's own tests.
- **Online switching is refused, not carried.** Online sessions ride the bike only and the rollback input
  carries no machine. Making the switch a rollback input needs the plane in the session state first. That is a
  follow-up, filed as N17.
- **A tape holds seat 0's switches only**, because a tape is the first player's pad. Local split-screen seats 2–4
  switch live, but their switches are not recorded.
- **The plane's reset chord and crash recovery now return it to where it was taken**, not to its machine.txt bay
  (the original resets to the bay). This was chosen because a reset to a bay across the map after spawning on
  the player would undo the switch.
- **A plane taken and left is drawn nowhere.** It does not go back to stand in its bay for this player, and
  nobody sees it standing where it was left. Bay ghost markers are P103's.
- **The plane is spawned level** (heading flattened, wings level) even on a slope. It settles on its tires over
  the first ticks as a parked plane does (measured ~0.6 ft of creep on free06's start). It is not placed already
  settled on the slope.
- **The bike is spawned stopped and ghosted** (`teleport_to`, as a marker teleport), so for its ghost time it
  passes through other riders. The plane's speed is not carried over to the bike.
- **A marker teleport onto a player who is flying** goes to that player's stale bike spot, not the plane:
  `markers::Spot::of` reads the rider. Not changed, since it is game-sim marker code outside this task. Filed as P104.
- **The pause's choices read one `switchable` flag per seat.** That is enough with two machines; a third
  machine would need a per-machine flag.
- **Not checked with a pad in a window.** The M key and the pause row were exercised through the menu tests and
  the headless tape, not by hand in a visible window.

## Next

Nothing: the task lands.
