# 1 — split screen ticks (FINAL)

Task N23. 2026-11-02.

## What I did

- **Profiled a 2P riding tick on this desktop**, not the Deck (the Deck profile was a one-time test).
  The run: `openmx --headless --steady-clock --draw-fps 30 --track free06 --players 2 --controls original
  --play gas.tape`. `--steady-clock` with `--draw-fps 30` runs one tick per picture with no pacing, so
  the sim gets a much bigger share of the wall time than in a paced headless run (where most samples are
  in the pace wait). The tape is hand-written: Cross (gas) held from line 31 on seat 0. A tape drives only
  seat 0, so seat 1 is idle and rolls down free06's start hill. Stacks came from eu-stack, started with
  `PR_SET_PTRACER` on a `line-tables-only` build in `target/prof` (sampler and aggregator in the
  session's scratchpad, not committed). 52–118 samples inside `World::simulate` per run.
- **Where a tick goes.** 96–99% of `simulate` is the seats' passes (`Garage::pass_among` →
  `rider_contact::Sim::step_seat`/`step_shared` → `Rider::frame_bumped`), split about evenly between
  the seats. `contacts_met`, the shared prop set's finish and the pad reads are noise.
- **Standing against riding** (`--players 1`, brake tape against gas tape, base build, `OPENMX_THREADS=1`,
  load ~1.8, 3000 ticks): mean sim 0.65 → 1.04 ms a tick, worst 2.85 → 5.14 ms. 2P: 1.23 → 1.76 ms,
  worst 6.03 → 6.69. In the stacks the riding tick's extra was mostly:
  - `flight_search::Kept::build` (wooded_unfiled, broad_build inserts, `Filing::file`): **~20% of a
    riding tick, ~0% standing**;
  - the put-back's back search (`Ride::keep_back` → `back_solid` → `back_search::query`/`gather`):
    ~19% riding against ~4% standing.
  The riding ragdoll (`Riding::step_under`) is ~20–36% either way: it solves every tick (S255).
- **Why worlds were built while riding.** A temporary trace in `kept_world` (removed) logged 72 builds in
  900 riding ticks on free06, against 2 with the brake held. 70 of them were "update refused" on one
  prop's moving word. That prop is a path car with physics on: `traffic::physics_frame` searches the
  world with the car's own mesh hidden (`traffic::hide`, which clears its moving word), and then the
  bike's searches see it moving. So each tick the searches alternate between two layouts that differ
  in a toy flag. `kept_world` only tried `Kept::update` on the *newest* kept world with the same layout,
  that world was always in the other state, `update` refuses a toy flag change, and the whole world was
  built again. It happened twice a tick for as long as the car was physical (near a rider).
- **The fix** (`crates/game-sim/src/flight_search.rs`, `kept_world`): try `update` on each kept world
  with the same layout in turn, newest first, and build only if none takes it. A refused `update` leaves
  its world untouched (both refusals return before the first write), so this only changes which kept
  world is brought up to date. It does not change what a search gets: S236's tests hold an updated
  world against a fresh build. Builds in the same 900 riding ticks: 72 → 3 (1P), 3 (2P).
- **New test** `a_hidden_car_and_its_unhidden_world_are_both_kept` in `crates/game-sim/tests/kept_world.rs`:
  every tick, a toy search with car 2 hidden as `traffic::hide` leaves it, then one with it seen, over
  10 ticks of driving cars. Both searches must get the fresh build bit for bit, with 2 builds and 18 updates. With the old
  policy it fails: 20 builds, 0 updates.
- **Measured** (2P free06 gas tape, steady clock, 3000 ticks, `OPENMX_THREADS=1`, base and new
  interleaved, this shared desktop at load 4–7): mean sim a tick 1.87/1.73/1.72 → 1.58/1.67/1.71 ms. The
  per-wall-second worst tick's sim at p90 went 4.59/5.04/4.28 → 3.96/3.43/3.13 ms. CPU time over four
  more pairs moved inside the noise (base 9.0–11.1 s user, new 8.6–9.6 s). The spikes are what got
  smaller. After the fix `Kept::build` no longer shows in the 2P riding profile.
- **What is left in a 2P riding tick** (118 samples after the fix): the riding ragdoll ~28%;
  `flight_search::lend` ~16%, a copy of the whole collidable list after each kept-world update, filed as
  N23b (low); the put-back's back search ~15%; `Filing::file` ~9%; each seat's own traffic copy ~8%.
- **Not done here: parallel seats or a sim thread.** Read `World::simulate` (`crates/openmx/src/main.rs`)
  for what stops a `thread::scope` over the seats' passes:
  1. every pass takes `&mut place` (`tiles::Land`). `Land::Tiles` keeps `RefCell`s (tiles load lazily),
     and every seat's traffic writes its cars' poses and moving words into the place's props;
  2. the seats' one prop set (`prop_seats::Shared`) is struck in seat order, and the first seat stepped
     runs its collide pass;
  3. a seat's pass reads the seats already stepped this tick (`rivals_among`, `bump_riders`, and
     `met_after`'s pushes).
  The world caches (`KEPT`, `SPARE`, `FILED`) are thread-local and pure, so one copy per thread is
  fine. The other route, the fixed update on its own thread with the picture blending from a snapshot,
  needs the draw to stop reading `World::seats` directly, and the loop reads them in hundreds of
  places. Either way it is a job of its own, with the per-tick hash held against the sequential path.
  Filed as N23a.

Nothing here was taken from opentennis. It runs its schedules on one thread on purpose
(`crates/hst-play/src/mode.rs`, P46), and has no per-seat parallel step to copy.

## What the original does

Nothing to port: this is the shell's scheduling and a cache in the port's own search. The original
builds its broad phase once and keeps it. The kept world (S236) is the port's way of getting there
without a persistent broad phase.

## How it was proved

- **Bit for bit, the ride:** the base and fixed `openmx` played the same gas tape for 1800 ticks, 1P and
  2P, `--steady-clock --trace-picture --trace-air`. That covers a wreck and get-up in 2P. The only lines
  that differ are wall-clock ones (load times, fps, per-second speed lines). Every per-tick picture and
  air line is the same.
- **Bit for bit, the world:** `kept_world.rs`, all 5 tests pass (`./kit check -- -p game-sim --test
  kept_world`). The new one checks both of the alternating worlds against a fresh build on every
  search.

## Not verified / not 1:1

- Not measured on a Steam Deck. The desktop numbers are at load 4–8 on a shared machine. The mean gain
  (~5–9%) is near the noise, and the p90 spike gain (15–30%) is clearer.
- The headless profile has only seat 0 on a tape. Seat 1 rolls idle down free06's start hill, so a 2P
  tick with both riders at speed was not profiled. Its kept-world behaviour is the same per seat.
- Only free06 was profiled. Tracks without physics path cars never hit the refused update, so they
  gain nothing from the fix.
- The user's complaint itself, 90 Hz frames missing vsync on each tick frame, is not fixed. That needs
  N23a.

## Next

N23a: parallel seats or the sim on its own thread (the blockers are listed above). N23b: make `lend`
copy only what changed.
