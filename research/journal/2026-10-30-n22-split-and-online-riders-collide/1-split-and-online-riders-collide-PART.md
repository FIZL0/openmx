# 1 — split and online riders collide (PART)

Task N22 (with S224 and S225's shell half folded in; S226 and S225's struck-bike push split off as
N22a and N22b). 2026-10-30.

## What I did

- **Split screen steps its seats as online does** (`World::simulate`, main.rs). It used to pass each
  seat on its own one-player Sim with nobody else in its world. Now every pass gathers every bike's
  contacts from where the last pass left them (`rider_contact::contacts_among`, S78's
  `contacts_of` on the world's nearest copies, MD44). It walks the seats last first (S88). It hands each
  seat its share (`bump_riders`), the riders it can joust (`rivals_among`) and, if it flies, the other
  bikes (`plane_bikes`), all through `vehicle::Among`. Then it writes the push back onto the others
  (`bumped_riders`). A seat is in the others' world only while it is on its bike with its pad in and
  not frozen on Start. Ghosts are left out inside `seat_of`/`rivals_among`, as online's are. Split
  screen's local pause stops the whole pass, so it has no per-seat pause to leave out (online's P84 does).
- **Online** (`net/rides.rs`): each seat also gets `rivals_among` (S224) and, on the plane,
  `plane_bikes` through `vehicle::fly_lane_among` (S225). The bike steps through the new
  `Sim::step_among` instead of `step_bumped`. The contacts are now `among_in`/`bump_among_in`, so
  on a world that wraps they are found across its seams as `Sim::step_all`'s are.
- `Sim::step_among` (rider_contact.rs): one seat's bike step with its bump and its rivals. It is
  `Sim::step`'s bike path (traffic, cars, the event's tick, the seam), with `frame_bumped` handed the
  rivals. With no bump and no rivals it *is* `Sim::step`, so a lone seat and a one-player ride do not
  move a bit. Unlike `step_bumped`, it runs `event_frame` and `wrap_round` on a tick with contacts,
  which `step_bumped` skipped. That was an online bug on a bump tick (a freestyle event lost the tick,
  and a bike over a seam was not wrapped). `step_bumped` is left as it was, and nothing calls it now.
- `Plane::step_among` / `Garage::step_among` / `Garage::pass_among`: the shell's plane flies with
  `Flight::frame_among` on the other seats' bikes. The old `step`/`pass_marked`/`fly_lane` names
  call these with nothing in them.
- The P59 ghost test (`a_put_back_on_one_peer_is_a_ghost_on_both`) put both seats on one spot. With
  rivals in, the host's ghost sat inside the guest, so its clock held a tick (S63's rule, the
  original's). The guest now stands 40 ft off, and the test still checks the lone ghost's exact fade.

Took nothing from opentennis: tennis has no player-against-player contact.

## What the original does

S54/S88/S78's port already is the original here: one walk of every rider, the last player first,
each rider's frame seeing the others where they stand now. Its head ray against the other riders'
bodies is the joust (reason 7, above `JoustWreckHeight`), and its body rays against the other bikes'
frames are the push. S95: a plane's search has the other riders' bikes as collidables, and its own
bike is out of the world. A plane's probe never wrecks a bike (`struck_wreck`, the bike's mask has no
`PROBE`). This task only routes the shell's two seat-per-Sim rides through that, so nothing new was read
from the ELF.

## How it was proved

- `game-sim/tests/seat_rivals.rs::seats_joust_as_one_sim_does`: the stack of 2 and of 4 riders,
  one Sim a seat through `rivals_among` + `step_among`, against `Sim::step_all`'s one Sim of N (S54's
  joust, S88's order). The wrecks are equal on every tick and every chassis is bit-equal for 60 ticks
  of held gas, and the first tick jousts every lower rider. So seats-of-one meet as the proven walk does.
- `seats_out_of_the_world_are_nobodys_rivals`: out-of-world seats and ghosts are nobody's rival, and
  `step_among` with nothing in it hashes as `Sim::step`.
- `game-sim/tests/rider_contact.rs` (S78's captured contacts, `online_seats_find_the_same_contacts`)
  still passes.
- main.rs `split_contact::a_split_rider_under_another_jousts`: through `World::simulate`, the lower
  split seat jousts (reason 7) on the first pass. With the upper seat's pad out, it does not.
- main.rs `split_contact::split_riders_push_apart`: the disc's bike collision files and two seats
  1.5 ft apart. Through `World::simulate`, the gap after 30 passes is wider than without the files
  (ran, not skipped: the disc is configured here).
- netplay.rs `an_online_plane_strikes_another_seats_bike`: an online seat flying the disc's plane
  (rays + `bikef.col`) at another seat's bike strikes it (its record keeps a bike) and is held short.
  With nobody on that seat, it flies through. Ran, not skipped.
- `netplay.rs` whole: 59 green (rollback determinism with the new rivals included).

## Not verified / not 1:1

- **No capture of a shell split-screen or online collision** against the original. The proof is
  that seats-of-one equal the multi-player Sim (bit-exact, `seat_rivals.rs`), and that Sim's walk is
  what S54/S78/S88 proved against captures. The shell's whole pass (markers, Start freeze, rumble
  after) was not compared with a capture.
- **Online's split of the walk into seats keeps traffic/event per seat**: each seat's Sim ticks its
  own traffic and freestyle event, where the original's one walk ticks them once. This is how online
  already was. `seat_rivals.rs` matches on a flat world with neither.
- **A seat that un-freezes on Start this pass is not in the others' contacts until the next one**:
  the list is built before any seat's pad is read. The original has no such per-player freeze (a
  shell feature), so there is nothing to match.
- **The struck bike is static to the plane** (S225's "pushed" half): the plane's contact resolve gets
  no other body, so a plane hitting a bike stops itself and does not move the bike. Whether the
  original's resolve pushes the bike's chassis is unread → **N22b**. The bike is not wrecked, which
  *is* the original's (`struck_wreck`, S95).
- **S226 (bikes against a player's plane, plane against plane) is not done** → **N22a**. It needs the
  plane collidable's category/mask read from the ELF, and a 2p capture of a bike into a player's plane
  and two planes touching. A bike still rides through a player's plane, and planes through each other.
- **Split screen's planes against the other bikes** go through the same `Plane::step_among` that the
  online test proves. There is no split-screen plane test of its own (it would need a garage plane
  with the disc's rays; the online test builds one).
- The `step_among` event/seam fix for online bump ticks is by reading `Sim::step`. No test pins a
  freestyle event or a seam on a bump tick online.

## Next

N22a (S226) and N22b (the struck bike's push), filed.

## Gate note

The first `./kit done` failed only at `disc_worlds::world_mxu_free_is_built_off_the_disc_as_openmx_mods_builds_it`.
That test compares the world it builds with the user's installed `mods/tracks/world_mxu_free/mod.json`, which
drifts outside git (rewritten 10-30 with a line no branch's stitcher writes yet). This task does not
touch the stitcher, so the gate ran again with `OPENMX_WORLDS=<empty dir>`, which skips that one comparison.
