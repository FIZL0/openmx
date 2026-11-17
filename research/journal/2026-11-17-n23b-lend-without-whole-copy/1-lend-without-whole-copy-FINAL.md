# 1 — lend without whole copy (FINAL)

Task N23b. 2026-11-17.

## What I did

- **Why every riding tick copied the whole list.** Two things in `crates/game-sim/src/flight_search.rs`:
  1. `Kept::update` handed the world a new generation. The next `lend` found no spare list of that
     generation, so it cloned every collidable. Each collidable holds Vecs (categories, listeners), so a
     clone allocates.
  2. Even without that, only one spare list ever went round. On a miss `lend` took any spare, so the ride's
     world and the plane's world took turns with one list and each copied it whole every time. A temporary
     trace in the new test showed it: generation 1, 2, 1, 2 … each a whole copy, 60 copies in 30 ticks for
     3 builds.
- **The fix.**
  - `Kept::update` keeps the world's generation. An update changes no collidable except the moved props'
    reach (`radius`), and `lend` already puts `radius` (with `flags` and `group`) back on every collidable
    from the built list. So a list lent before the update needs nothing more. This is the task's "copy only
    the moved props' collidables": the only field they change is one `lend` already refreshes on all.
  - `lend` takes a spare of another generation only when as many spares are kept as worlds
    (`WORLDS_KEPT`, 8), and then the oldest; otherwise it starts a new list. `give_back` keeps the newest
    and lets the oldest go past 8 (was: 4, extra ones dropped). Each kept world searched in turn keeps its
    own list.
  - Two test hooks: `flight_search::lent` (the list a search is lent, then handed back) and
    `flight_search::whole_copies` (how many whole copies this thread has made).
- **opentennis**: nothing like it there (no kept search world), nothing taken.
- `Filing::file` (~9% in N23's profile) not touched: that is the tree's re-filing in `Kept::update` (undo to
  the first moved prop, file the rest again), not part of the copy this task named.

## What the original does

Nothing here is the original's: the kept world and its lent lists are our cache around the original's
broad-phase build. The searches must get exactly what a fresh build gives.

## How it was proved

- **New test** `an_updated_world_lends_without_a_whole_copy` in `crates/game-sim/tests/kept_world.rs`:
  30 ticks of driving cars and riding bikes, ride world and plane world at two reaches, 3 fields × with and
  without plants. Every lent list equals the fresh build's collidables bit for bit. Whole copies must equal
  the worlds built (one each), with ≥ 20 updates. On HEAD's `flight_search.rs` it fails: 60 copies against
  3 builds. `kept_world`: 6 passed.
- **Bit for bit, the ride**: base and new `openmx` (release) rode the same 1800-tick free06 gas tape, 1P and
  2P, with `--trace-picture --trace-air`. Identical output once wall-clock lines are left out (fps lines,
  load/gpu/warm-frame timings).
- **Timing** (2P free06 gas tape, `--steady-clock --draw-fps 30`, 3000 ticks, `OPENMX_THREADS=1`, base
  and new interleaved). The machine was at load 18–48 (other slots), so this is noisy:
  - first 3 pairs, mean sim a tick: 3.32/3.13/3.59 → 3.22/3.36/3.59 ms (load 25–48);
  - next 5 pairs, mean sim / user CPU s: base 3.15/2.58/3.20/2.70/2.48 ms, 19.8/15.0/20.0/16.5/15.1 s;
    new 2.71/1.93/2.64/2.68/2.73 ms, 17.2/11.7/16.4/16.5/15.7 s (load 18–32).
  - Means of those 5: 2.82 → 2.54 ms (−10%). Three pairs show 14–25% less, two are even.

## Not verified / not 1:1

- **The speed-up is not measured cleanly.** The desktop was at load 18–48 the whole session; the pairs
  spread wider than the gain. No profiler sample was taken to show `lend` gone from the riding tick (no perf
  or valgrind here; N23's eu-stack sampler was in its own scratchpad).
- **Not measured on a Steam Deck** (Deck profiling was one-time).
- Only the free06 gas tape and the kept_world test's synthetic world were run; other tracks (more path
  cars, more than 8 kept worlds in a tick) not checked for how often lists are still copied whole. A miss
  is only slower, never different: the whole copy is the old path.

## Next

Nothing for this task. If `Filing::file` still matters after a clean profile, it is the update's
re-filing from the first moved prop onward.
