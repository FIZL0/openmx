# 1 — wreck bike props (FINAL)

Task S76. 2026-10-26.

## What I did

- `WreckBike::tick_among(dt, ground, props, trees)` (crates/game-sim/src/wreck_bike.rs): builds the
  biplane's `flight_search::Scene::wooded` over the scene's collision meshes (`world::Props`) and the
  ecosystem trees, bounded by the heightfield, around the wreck bike's node, and hands it to
  `WreckBike::tick` in place of `NoObjects`. `tick_wooded` (S102) now calls it with no props.
- `ride::Crash::step` hands the body `world.wooded()`'s props and trees. It falls back to `NoObjects`
  when the world has neither, or when the part's collision is off.
- tests/wreck_bike.rs:
  - `the_wreck_bikes_replay_among_free09s_props` replays the five s62 wrecks and S76's two garage
    wrecks among free09's props (loaded from free09.res + free09_2.res as the shell loads them). All
    517 ticks are bit-exact. The s62 wrecks touch nothing; the garage wrecks are on an object for 11
    and 4 ticks. The same garage wrecks replayed against the terrain alone (`NoObjects`) part at their
    first tick (2883, 3003), so the props are what make them match.
  - `the_wreck_bikes_probes_search_the_scenery` reads both probes' masks, leave-out lists and category
    lists off all seven states, and checks that none of them excludes the scenery category.
  - The replay loop of `the_wreck_bikes_replay_bit_exact` is now a shared `replay` helper. That test
    is unchanged otherwise.
- The capture: research/s76_tele.py (S89's tele.py, with the symbols taken from kit.toml) and
  research/s76/gas.pad.

## What the original does

The wreck bike's ray probe and sweep probe are ordinary collidables:
- category 0x8 and collides-with all;
- ray flags 0x02, sweep flags 0x03.

Their leave-out lists name only the bike's own collidable, the other probe, the riding bike and the
rider. Their category lists name only 0x8 and 0x800 (the sweep's). Scenery (0x220) is therefore
searched, exactly as the plane's probes search it, which is why the port reuses `flight_search::Scene`.

Offsets and the full description are in research/oracle.txt under "S76".

## How it was proved

Save-state capture plus bit-exact replay fixture.

1. Took recording2's state (copy 3, slot 8).
2. Teleported it with `./kit pcsx2 with -- python3 -B research/s76_tele.py PID 4642.5 135.6 3650 8 4`.
   PCSX2 must be started with `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1` for the AT-SPI
   pause, and `./kit pad serve` must be running.
3. Resumed with c6's resume_if_paused.py.
4. Recorded with `research/s62_wreck.py garage --pad research/s76/gas.pad --state 4 --frames 500 --late 120`.

The result is in context/capture/s62/garage.csv: frames 2758..3257, 0 torn rows. The states are
garage-{2882,2883,2884,3002}. Results:
- 517 ticks over seven wrecks bit-exact (tests/wreck_bike.rs);
- terrain-only replays of the garage wrecks part at the first tick.

## Not verified / not 1:1

- **UTILITYSHACK1.col is not searched.** It is a Hierarchy collision file, and `Props::load` reads
  only Mesh files. The shell skips it too, and the test pins that as the single file missing. A wreck
  into that shack would not match.
- **The nearby list is rebuilt every tick from the bound around the node.** That is not where or when
  the original gathers it (the same open point as S102's trees). It matches over these 517 ticks, but
  a wreck grazing an object at the bound's edge on the tick it enters is unproven.
- **Moving collidables are not searched.** The original's nearby list at the s62 states held toys
  (0x48), other bikes (0x9a048) and 0x200020 objects. The port hands no bikes (`None`) and no toys:
  `Scene::wooded` takes no toys, and riders/other bikes are a separate gap.
- **Trees and props together are uncaptured.** There is no capture of a wreck into a tree; the trees
  path is still S102's proof.
- **Only free09 is proven.** Other tracks' props go through the same loader but are not captured.
- **The 60 Hz path is unproven.** The captures are 30 Hz game ticks, each holding the wreck bike's
  115 Hz steps as the s62 replays do.
- **The entry from the riding bike is not exercised.** The tests start each wreck from the state's
  wreck-bike body, not from `Crash` handing over the riding bike.
- **Performance is not timed.** The props' broad phase is rebuilt every wreck tick.
- **The garage data exists only in s3's context.** In other slots both new tests return early,
  skipping themselves whole, when any state is absent, and a passing gate does not show that. Copy
  context/capture/s62/garage* to them.

## Next

S133 (filed): hand the wreck bike the moving collidables, toys and other riders' bikes, and capture a wreck into a bike.
