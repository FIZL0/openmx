# 1 — s12 ai path lookahead (FINAL)

Task S308. 2026-11-10.

## What I did

- `game_sim::ai::follow::locate_on`: the path locate with its start hint. S282's `ai::path::locate`
  has no hint. This one adds the limit, refuse, holding-only, the hint on the same path (with the
  wrap when a path links to itself), and the hint on another path through `TrackPaths::distance`.
- `ai::follow::route_locate`: the nearest place on a route of paths.
- `ai::follow::look_ahead`: the vehicle brain's path look-ahead, plus `pick_link`, its fork choice.
- `race::RacePath::word_0c` (path +0xc, read as a float speed at speed forks) and
  `race::TrackPaths::pick` (track +0xac, the link-pick mode, 3 from every loader).
- Three oracle tests in `crates/game-sim/tests/race.rs`, and research/oracle.txt names
  `path_route_locate`, `path_look_ahead`, `ai_path_look_ahead`.
- Nothing from opentennis applied (no path code there).

## What the original does

- `./kit dis 0x002838c0 0x00283a10`: MXVehicleAI vt+0x88. When the route word at sp+8 is 0, it
  shifts a1..t3 down one register, moves sp+0 (holding-only) into t3 and calls the look-ahead.
  Otherwise it runs a route branch through 0x26cbe0 / 0x26e230 / 0x26bbe0 / 0x26c9f0, not ported.
  ai_route_follow passes its own out2 there.
- `./kit fn 0x0026d780` (the look-ahead), arguments f12 reach, f13 speed, f14 limit, a0 track,
  a1 spot, a2 out, a3 point, t0 nearest-out, t1 draws, t2 refuse, t3 holding:
  - It measures from the spot to the end of its path's last stretch (path_distance to
    {path, last−1, 1.0}).
  - It takes a route-pool entry, then picks forks until the lengths cover the reach. A fork is
    picked by path flag 0x8 (the link with the nearest +0xc speed), else 0x20 (link
    `count × draws[0]`), else by track +0xac.
  - It walks the last path's stretches to place `out`. Then it calls the route locate for the
    nearest place, hinted at the spot, and frees the route.
- `./kit dis 0x0026bbe0 0x0026bdb0` (the route locate) calls path_locate on each entry's +4 path
  (+0xc is the next entry). It takes the first answer, then a later one when d ≥ 0 and d < best,
  or when best is still −1.
  - Its starting place is at sp+208..216 and its scratch spot at sp+224..232, both never set.
    With its 240-byte frame they sit at the caller's sp−32 and sp−16. Inside the look-ahead,
    path_distance (48-byte frame, `./kit dis 0x0026e0c0 0x0026e230`) has just copied its `b` and
    `a` spots exactly there. So the look-ahead's call starts from {path, last−1, 1.0} with the
    spot as scratch, and the port gives it those. The original's frees (0x1e5060) run only for
    paths without the owned bit, so they never land in between.
- `./kit fn 0x0026b860` (path_locate):
  - A hint on the same path starts the walk at the hint's node, with walked = −along·span.
  - A hint on another path starts with walked = path_distance(hint, path start).
  - The walk stops once walked reaches a non-negative limit.

## How it was proved

All oracles run against SLUS-21104 in `crates/game-sim/tests/race.rs`, with out spots, distances,
the paths' scratch words and the pools compared bit for bit. The tests' own coverage counters
(asserted in each test) show every branch was taken:
- `the_hinted_path_locate_matches_the_track_objects`: 20,000 generated courses.
- `the_route_locate_matches_the_track_objects`: 20,000 routes of 0–5 paths, some closed into rings.
- `the_path_look_ahead_matches_the_track_objects`: 30,000 courses, half through vt+0x88 and half
  straight into the look-ahead. Covers the four pick modes, speed and draw forks, running off the
  end, a dry pool, and with and without the nearest spot.

## Not verified / not 1:1

- **The route locate called on its own** starts from `Spot::default()` for its place and scratch.
  The original reads stack words it never set, whatever the previous callee left. The oracle zeroes
  the stack first. Inside the look-ahead the leftovers are known and ported, but only while no free
  runs in between (paths owned, as in every loaded net).
- **Cases where the original never answers or reads past memory.** The port returns false for each,
  and the tests leave them out:
  - A spot on a one-node path: path_distance and the route search recurse without end.
  - A refused drawn (flag 0x20) link: the original spins for ever. ai_route_follow passes refuse 0.
  - A draw ≥ 1.0 or < 0 reads past the link list.
  - A fork with no links reads a null list.
- **Only draws[0] is ever read.** The index moves only inside the refused-draw spin.
- **Not wired into the Sim** and not run on a real track net. A real sx01 net is S309.
- **The forwarder's route branch** (non-zero route word) is not ported.
- **The MX vs. ATV PC exe** was not read.

## Next

S309: the route branch of vt+0x88, and wiring the look-ahead into the race AI on a real net.
