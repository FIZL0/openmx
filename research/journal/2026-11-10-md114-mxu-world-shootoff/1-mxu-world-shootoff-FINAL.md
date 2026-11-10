# 1 — mxu world shootoff (FINAL)

Task MD114. 2026-11-10.

## What I did

The report (user 2026-11-05): "mx world has a bugged shootoff when leaving spawn map, it shouldnt have
any". I looked for what brought back the edge on today's main and **could not reproduce it**. No product
code changed. What I did instead was add the regression ride the task asks for, and file the one real
edge gap I found (machines, below).

Where I looked:

- **The plumbing.** `rider.rs`'s edge clause asks `off_map::outside_wrapped(area, world.wrap(), …)` and
  `off_map::off_world`. Every `World` wrapper between the shell and the sim forwards `wrap()`:
  `Land` (all three arms), `Wooded`, vehicle.rs's `Through`. `Place` and `handling::ground::Under`
  have no wrap, but they are never a world's. The shell's sound (`seat_sound` → `audio::world_edge`)
  and the trace's "shootoff warning" line also take the land's wrap. None of
  `tiles.rs`/`off_map.rs`/`rider.rs`/`world.rs`/`worlds.rs`/`disc_worlds.rs`/`stitch` has changed
  since v0.11.0, the release three hours before the report, apart from 49 lines in `game-sim/src/lib.rs`.
  `git log -S outside_wrapped` / `-S world_edge` show only MD29 and P231 adding them, and nothing taking
  them out.
- **The disc-built world_mxu** (P232/P253: 27 tiles, 21853 × 19828 ft, spawn mxu_free07 at 9556,11855,
  "world holes: none"):
  - `--trace 2400 --trace-gas` from the spawn left mxu_free07 into the mxu_free10/free09 ground with
    no throw.
  - The real ride loop (`--play` a gas tape, `--frames 3000`, a temporary per-tick print of the
    seat's wreck, `audio::world_edge`, `edge_armed` and `wrap`) went from mxu_free07 into mxu_free09:
    `edge None` every tick, `wrap [true, true]`, `armed true`. The two wrecks on the way were reasons
    1 and 5, the ground's own, not 13.
- **The user's installed world_mxu** (openmx-mods, `~/repos/openmx/mods/tracks/world_mxu`, 2026-11-02,
  37792 × 31960 ft, spawn mxu_nat01). It is the one a ride from the main checkout gets, because an
  installed world of the id beats the disc build. I symlinked it into a scratch mods folder, read-only.
  - The trace and the live loop both rode out of mxu_nat01 into mxu_nat20 with no throw and no edge
    warning. The only wreck was a reason-5 crash at the join.
- **`--trace-at` rides** on the disc world, from just inside each of mxu_free07's four kept edges
  heading out, and from 100–150 ft short of both wrap seams in both directions: no `thrown:` and no
  `shootoff warning:` line on any of them.
- **The generated wrapped 3 × 3 world** (middle_spawn's `md50_round`, 432 ft square): a 900-tick gas
  ride crossed the x seam eight times with no wreck.
- **The user's settings.txt** in the main checkout has `track = free06` and `tricks = mxvatv`. If that
  ride really was free06 alone, then "leaving the spawn map" is free06's real edge, which throws as the
  original does. I can't tell that from here (see below).

The test, in `crates/openmx/tests/disc_worlds.rs` as `no_edge_anywhere`: it is called from
`world_mxu_has_every_freeride_and_national_with_the_freerides_together`, which already builds world_mxu
off the disc, so it costs six warm shell runs and not another build. It reads the spawn tile and its
kept rectangle from the built `mod.json` and the stitch line's spacing, and traces six 450-tick gas rides:

- four out of the spawn map's four edges, starting 450 ft inside. 150 ft inside the +z edge is a bank
  where the bike stalls at 2 ft/s.
- two over the x and z seams, starting 150 ft short, inside the original's margin, so an edged map
  of that size throws at once.

Each ride must print no `thrown:` and no `shootoff warning:`, and must end past its edge or seam.
Against a mutant `outside_wrapped` that ignores wrap, the x-seam ride printed `thrown: tick 6 at
21702.6, 11855.0`, so the test catches that regression.

Opentennis: nothing to take, since it has no map edge.

Filed **MD120**: cars (`car.rs` `wreck_timer`: `world.area()` + `a.outside(EDGE_MARGIN)`) and the Kiowa
(`kiowa.rs` crash step: `a.outside(EDGE)`) ignore `World::wrap()`, and neither wraps round at a seam.
So on world_mxu a buggy, monster, baja or the helicopter is thrown at the world's rectangle edge. That
is a shootoff in a wrapped world, but at a seam, not where the spawn map ends. I read it in the code
and did not ride it.

## What the original does

The original has no wrapped worlds. Its bike edge test (S59, `off_map`) is the original's, and the
wrap exemption is the port's own (MD29). Nothing here changes either.

## How it was proved

- The traced rides above, on today's main, on both world_mxu builds and on the generated world.
- The ride loop with a per-tick print (temporary, reverted) on both world_mxu builds.
- The new test's six rides on the disc world, plus the mutant run showing the seam ride catches a
  throw that ignores wrap.

## Not verified / not 1:1

- **The user's report is not reproduced.** On today's main, nothing throws or warns anywhere I rode
  out of world_mxu's spawn map, on the disc build or on the installed one, in the trace or in the live
  loop. I did not find what the user saw, and so I did not find a cause or bisect one. The candidates
  I could not rule out:
  1. A ride that was free06 and not the world. The main checkout's settings.txt reads `track = free06`.
     free06 alone has the original's edge, and that throw is right.
  2. A ride on a machine (car or Kiowa, MD120). That throws at a seam, though, not where the spawn
     map ends.
  3. A build or install other than these two: a phone, the web, or an older openmx-mods world_mxu.
     There is no phone log from 11-05 in ~/Downloads; the newest is 10-30.
  4. A path I did not drive: the menu-started ride, split screen, online, or the P327 race. The menu
     ride runs the same ride loop on the same `Land`, but I drove it only with `--track`/`--play`.
- **Rides by hand only.** I did not ride the world on screen or with a pad. Every ride was headless,
  on the gas with the stick straight, in four directions and over two seams. A route through other
  ground or other maps' edges is not covered; the edge test does not depend on the route, but I did
  not prove that by riding.
- **The new test only asks for the absence of a throw and of the warning.** It does not ask for no
  wreck at all, because a crash of the ground's own is not this bug. On its six starts there is none
  as of 11-10, but a later terrain or stitch change could add one without failing the test.
- **The installed openmx-mods world is not in the test.** It is not in the repo, and a test against
  the main checkout's mods is the kind that is ignored. I checked it by hand only, as above.
- **MD120's gap** (machines throw at a wrapped world's seam) is read in the code, not ridden.

## Next

If the user sees it again, ask which world and machine, and which build (desktop from the checkout,
phone, web). A phone log (first line names the build) or the shell's `world spawn:` / `mods:` lines
would say which world_mxu was ridden.
