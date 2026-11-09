# 1 — s13 laps (FINAL)

Task S293. 2026-11-09. Part 2 of the S13c split: the score manager's race handler, per card.

## What I did

- `race::TrackPaths` gained the track object's path queries, each oracled against the original:
  `spot_point`, `path_ahead`, `distance`, `route` / `route_walk` / `free_route`, the ring test,
  `reaches`, `find_spot` (the spot search: on-track word 0/1/2 and the distance along), and
  `mark_reached` — the load's walk from path 0 that takes flag 0x80 (`path_flags::UNREACHED`) off every
  path it reaches. `TrackPaths::new` now runs that walk, so a loaded track's flags are the image's
  (checked on sx01's real file in `sx01s_race_file_off_the_disc_is_the_track_object_in_the_image`).
- `race::RaceRules::card` (with `RaceCard`, `RaceNotice`, `race_card_flags`) is the handler's body for
  one card: odometer, the spot search and its on-track word, the below-the-path drop, the checkpoint
  clear, the hand-off to `LapRules::line` and, after a counted lap, the card moved onto the line
  (spot = line spot nudged ahead, distances = the lap length), race progress, the off-track clock with
  its `OffTooLong` limit, the back-to-track aim (a route walk toward the line), the race/lap clocks
  and the finish estimate when everyone the race waits for is out, and the card's time limit
  (`OutOfTime`). `RaceRules::look_for_waited_for` is the opening scan that sets the manager's
  all-out flag.

## What the original does

- `./kit fn 0x00262aa0` — the handler. Before the card loop it scans the cards for one the race still
  waits for (manager flag 2 = none left). Per card: unless taking part (kind 4: unless fast-lap) or
  done, it adds the step length to +0x20, finds the spot (`./kit fn 0x0026c010`) into +0x18 / spot,
  drops the on-track word to 0 when the path is more than the manager's +0xb8 above the card, clears
  the checkpoint flag on the last-but-one node, and when on track measures the distances to the line
  (`./kit fn 0x0026e0c0`) and calls the lap line when near it. A counted lap puts the card's spot on the
  line. Off track, +0xc0 counts up and, with card flag 0x1000000, tells the listener once it passes
  +0xd4; the aim (+0x180) is the unit vector to a spot walked along the route to the line
  (`./kit fn 0x0026cbe0`, `./kit fn 0x0026e230`, `./kit fn 0x0026e4f0`). The time limit +0xc4 counts
  down while the manager's +0xa0 is set. After the loop: the 30 s finish countdown, ranking and the
  end calls (unported).
- `./kit fn 0x0026e9d0` — the reach walk after the load (DFS from path 0, pool +0x98).
- research/oracle.txt has the per-function notes (search "S293").

## How it was proved

Function oracles in `crates/game-sim/tests/race.rs`, 20 000 generated cases each, all bit-exact:
spot points / way ahead, distances / routes / walks, the ring test, the reach marks, the spot search,
and the handler per card (`the_race_handler_matches_the_score_managers_per_card`: card words, ghost,
manager flags, path flags, free pools and every notice in order; a run saw ~1800 off-track, ~18 000
aim, ~6700 lap-line, ~1900 off-too-long and ~780 out-of-time notices, 413 laps). Plus sx01's real
track file: `TrackPaths::new` gives the image's path flags exactly.

opentennis has no laps, so there was nothing to borrow.

## Not verified / not 1:1

- `route_walk` walks forward only: the backwards bit of a route entry is never set by anything ported.
- The route build assumes the original's best-link chain is never null where the port follows it
  (never seen in 20k cases, not proven).
- `TrackPaths::new` refuses graphs the original would load (bad link indices); paths are never freed,
  so the "free unless owned" branch of the route free is only half exercised.
- Spots with `along` outside 0..1 were not generated.
- The handler's route list word is stale stack in the original (never set before the route search);
  the port starts from an empty list. The oracle zeroes the word; any other stale value is not
  modelled.
- Null spots make the original read low memory (0x14 / 0x54); the port treats them as 0 and the
  tests require spots to name nodes.
- Not ported: the card-0x80000 helper, the checkpoint/line lists (manager flags 0x40 / 0x200, S294),
  the manager-0x800 / 0x1000 helpers, the computer aim (card flag 0x800000: the port skips it and
  leaves the aim alone), and everything after the card loop (finish countdown, ranking, end calls).
- The lap length (track +0x54) is taken from `Course::lap_length`; `TrackPaths::new` does not compute
  it from the line-to-line route as the load does.
- `RaceRules::card` is not wired into the running `Race` yet; nothing in the game calls it.
- No PINE capture: everything is function oracles over generated tracks plus one real file.

## Next

Follow-ups: S301 (wire `RaceRules::card` into the race, lap length at load), S302 (computer aim and the
remaining helpers); the line lists, finish countdown and ranking were already S294.
