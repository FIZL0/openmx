# 1 — s12 ai route cross path spot (FINAL)

Task S311. 2026-11-18.

## What I did

- `ai::follow::route_locate_over` now also hands back the path whose node list the place's node is
  in (the "owner"). It tracks the owner of the scratch spot: a path that places a node (its distance is
  not exactly −1) becomes the owner. A path that places nothing leaves both the node and the owner as
  they were. The seeds' owners are their own paths.
- `look_ahead` and `route_look_ahead` keep their public signatures. Private `*_owned` twins carry
  the owner through.
- `route_follow` reads the drift's node word (+4) and the speed words (+0x44, and the next node's)
  from the owner's node. It now returns `Option<usize>`: the owner when it is not the spot's own path.
- `follow::path` leaves out the hand-over when the follower left such a spot. The original's measure
  there never returns.
- Nothing taken from ../opentennis: it has no path follower.

## What the original does

- `./kit fn 0x00283a10` (route follower): it copies the route locate's three scratch words into
  +0x220/+0x224/+0x228. Then it reads `*(+0x224)+4` for the no-drift bit, `+0x44` for the speed and
  `+0x54` (the node's next pointer) `+0x44` for the blend. +0x224 is a raw node pointer, so all of
  these read the owner's node, not node n of the +0x220 path.
- `./kit fn 0x0026bbe0` (route locate): every route path writes the same scratch spot. A path that
  places nothing writes only its path word. So when every path up to the last one taken placed
  nothing, the place names the last path with the node the seed (the rider's spot) held.
- The next `path_distance` on such a spot recurses for good (S309's finding). With a goal that is the
  same tick's hand-over. Without one it is the next tick's reach.

## How it was proved

- Function oracle: `crates/game-sim/tests/race.rs` `the_ais_path_branch_matches_the_tick`, 12,000 cases
  out of about 18,050 generated courses.
- The cross-path cases are no longer left out. 2 in 10 cases now get a steering max of 0. That max is
  also the route locate's limit, so a rider spot at a node's start places nothing on any route path.
  This gives 32 cross-path cases per run, all with a goal, and all bit-exact against SLUS-21104 on the
  brain, the goal's spot, the pools and the generator. The original is stopped after its follower,
  where its hand-over measure would hang. The test asserts more than 20 such cases.
- Mutation check: going back to the old read (the spot's own path's node) fails the test.

## Not verified / not 1:1

- **No-goal cross spots are barely covered.** About 3 per run turned up before the change (all
  bit-exact then too), and the reshaped generator turned up 0. Without a goal the reach is capped by
  the same max that is the locate's limit, so the look-ahead route rarely holds a second path that
  places nothing. The code path is the same as with a goal, from the route locate down.
- **What comes after a cross spot is not 1:1.** The original hangs in its next path measure. The port
  cannot hang, so it keeps going:
  - `follow::path` skips the hand-over;
  - the next tick's reach, and `drive::mode_four`'s gate measures after its route follow, read node n
    of the spot's own path, because `race::Spot` keeps no owner.
  Nothing can be matched there, because the original never answers.
- **No live capture.** None of the sx01 image's courses makes such a spot (S309), so this rests on the
  oracle over generated courses only.

## Next

- none
