# 1 — the sim thread (FINAL)

Task N23a. 2026-11-02.

## What I chose, and why not parallel seats

The brief offered two routes: step the seats in parallel, or run the fixed update on its own thread.
**I rejected parallel seats** because they can't match the sequential ride bit for bit. I read
`World::simulate`, `rider_contact::step_seat`, `traffic` (mark/place/hide/show) and `toys::frame`:

- Each seat's `Sim` has its own traffic and toys. They write their cars' and toys' poses and moving words
  into the one place's props. Seat 0 is stepped last (`(0..n).rev()`, as the original walks its riders),
  so it reads what seat 1 wrote this tick. `traffic::hide`/`show` changes a moving word for the length of
  a search.
- `prop_seats::Shared` is struck in seat order. `rivals_among`, `bump_riders` and `met_after` read the
  seats already stepped.
- `Land::Tiles` keeps `RefCell`s, so the place is not `Sync`.

Stepping the seats at once would give a different ride, and it would stop being the original's order.
**So the whole tick moves off the loop's thread, and the seats still step one after the other.**

What made the thread route possible:

- `Seat` and `Land<'_>` are `Send` (checked with a temporary probe). `Land` is not `Sync`, but the tick
  owns its world outright.
- The simulation's thread-locals are pure caches: `flight_search`'s `KEPT`, `FILED` and `SPARE` (N23,
  S236), and the `BUILDS` counter. Every other static is a `OnceLock` table. So which thread runs a tick
  can't change what it computes.
- Since P205, each pass of the ride loop either runs a tick and draws nothing, or draws and runs no tick.
  The draw pass never writes `world.seats`. From the place, it asks only for ground heights
  (`portal_ground`, the sun flare), plus `tiled()` for tiled worlds.

## What I did

- **`crates/openmx/src/sim_thread.rs`** (new):
  - `Worker`: a rayon-core `ThreadPool` of one thread named "openmx sim", kept for the whole ride so the
    kept worlds stay warm. A thread per tick would start the thread-local caches cold on every tick.
  - `beside(job, run, here)`: runs `here()` on the loop while `run(job)` runs on the worker, using
    `in_place_scope`. The scope's body runs on the calling thread, so the draw stays on the window's
    thread. The job may borrow the loop's locals, and a panic in it comes back on the loop.
  - `Mode` (`--sim-thread auto|on|off`, default auto).
  - `Gauge`: the slowest of the last 30 ticks' sim time, the last picture's draw time, and the picture's
    budget (`--draw-fps`/the frame limit's floor, else one refresh).
  - The crates all `#![forbid(unsafe_code)]`. My first attempt was a hand-written worker that took
    borrowed jobs by erasing a lifetime, which that forbids. rayon-core's scope is the safe form of the
    same thing, so it is a new desktop-only dependency (`Cargo.toml`, +3 packages in `Cargo.lock`:
    rayon-core 1.13.0, crossbeam-deque 0.8.8, crossbeam-epoch 0.9.21). In the browser there is no worker:
    `Worker::new` gives none, and every tick runs as before.
- **The ride loop** (`crates/openmx/src/main.rs`), with no code moved and nothing reindented:
  - **Pre half.** A tick pass runs its first half on the loop as before: window, pads, menu, the
    picture's `before*` copies.
  - **Sending a tick off.** It may then send the tick off instead of running it. It does this only if
    all of these hold: the mode says so; offline; not a tiled world; it is not the first tick (the first
    picture needs one finished tick to draw from); no backlog (`Pacer::backlog`, new, so catch-up ticks
    still run back to back before a picture); and it is not the tick that ends `--frames`.
  - **How it goes.** It moves the real `World` and a clone of the `Engine` into a `Ticking`. The loop
    keeps a stand-in `World`: cloned seats, and a `Land::Track` with the same ground and no props, built
    once after the load. Then it falls through to the draw.
  - **Draw half.** The whole draw section runs inside `sim_thread::beside(...)`. The tick runs on the
    worker while the picture is drawn. The `before*` copies equal the seats, and `pacer.blend` is clamped
    to 1, so the picture shows the last tick's end, held.
  - **Landing.** The next pass lands the tick: the world and engine come back, the stand-in's place is
    kept for the next one, and the same `Step` goes through the unchanged second half (outcome, the
    tape check, `played_dt`, sound, cameras, the body's frame). Then `pacer.ran()`.
  - **Auto.** It engages when there are 2+ seats, a window, and slowest tick + picture > budget.
    `--timings` prints a `sim thread on|off: …` line when it changes.
  - **Small changes for this.** The `views_of` closure reads `look.fog_color` through a copy taken
    before it. The draw is a closure now, and it borrows `look` whole. `Seat` derives `Clone`.
  - **`--trace-hash`**: after every tick, a line with each seat's `game_sim::hash::of(&seat.sim)`. That
    is what the comparison below holds.

## How it was proved

- **The same ride, tick for tick, headless.** I played the 2P free06 gas tape (1800 ticks, Cross held
  from tick 31 on seat 0, a wreck and get-up included) with `--headless --steady-clock --trace-picture
  --trace-air --trace-hash` and `--sim-thread off` against `on`. The thread took every tick but the first.
  - All 1800 `tick N hash` lines (both seats) are byte-identical.
  - Ride hash 0x895e0d84370c899a and plane hash 0x3fcc295c14e13abd in both.
  - Every non-picture line is the same except the once-a-second wall-clock fps lines.
  - The picture lines differ as designed. With the thread, the picture drawn while tick N+1 runs is
    tick N at blend 1.0 (`from == to`). Without it, that picture is N+1 at ~0.5.
- **The same ride, windowed, `auto`.** On a private headless weston (`--size 640x448 --draw-fps 240`
  so the gauge's budget is 4.17 ms), `auto` engaged:
  `sim thread on: slowest tick 8.28 ms + picture 143.54 ms against 4.17 ms`. Weston's headless GL makes
  a picture slow, so most ticks there are backlog and run on the loop. All 1800 tick hashes match the
  `off` windowed run and the headless runs, with the same ride hash.
- **Speed, headless** (3 interleaved pairs at load ~5–7, steady clock, `--timings`): the loop's sim
  time went 0.87/0.77/0.75 → 0.01/0.01/0.01 ms a frame. All stages went 2.76/2.82/2.62 →
  2.45/2.21/1.99 ms a frame. Wall time went 10.95/11.15/10.38 → 9.83/9.02/8.20 s. After the first
  second, the per-second worst frames went from 4.5–9.4 ms (sim 2.3–6.5) to 3.5–4.5 ms (present-bound).
  The session's worst is still tick 1, which runs on the loop by design.
- **The copy's cost:** a temporary probe (removed) timed `world.seats.clone()` at 18.9 µs mean, 74 µs
  max, over 1799 ticks.
- **Tests:**
  - `cargo test -p openmx --bins -- pace::tests sim_thread::tests`: 18 passed. That includes the new
    `a_backlog_is_a_tick_owed_behind_the_next` and five sim_thread tests: the job runs off the loop while
    `here` runs on it, every job is on the one thread, the loop waits for the job, a panic comes back,
    and the gauge.
  - `./kit check -- -p openmx`: see the commit message for the result.
- **Browser build:** `cargo check -p openmx --lib --target wasm32-unknown-unknown` with the upstream
  1.99.1 toolchain in /tmp/openmx-cross passes with no openmx warnings.

## Not verified / not 1:1

- **Not measured on a Steam Deck**, which is where the complaint is. This desktop's tick frames fit
  in a 60 Hz frame either way. Whether `auto` engages there, and how many missed vsyncs it saves, is
  unmeasured.
- **The held picture.** While the thread runs a tick, the picture shows the last tick's end instead of
  blending toward the next one. With a 60 Hz display and 30 Hz ticks, an engaged thread shows the
  motion at 30 Hz steps: the held picture repeats the one before it. `auto` only engages when tick +
  picture already overrun a picture's time, where frames were being missed anyway. A run-ahead (draw
  N−1→N while N+1 runs, one tick more latency) would be smooth. Filed as F24, because it changes the input
  latency.
- **Not threaded:** tiled worlds (MD8/MD15), whose draw reads the `Tiled` itself (dressing, the ground
  draw, the near sets), and online rides (the session steps its own seats). Both run as before.
- **The gauge is a heuristic.** It compares the slowest of the last 30 ticks plus the last picture
  against one picture's budget. Under `--fps-limit off` with vsync off it compares against one refresh.
  Its thresholds were not tuned on any device.
- **`perf`'s Sim stage on a threaded tick** is the loop's wait at landing (≈0), not the tick's own time.
  The gauge keeps the tick's own time.
- **Output order.** A tick on the worker that prints (`--trace-air` and the like) can interleave with
  the held picture's `--trace-picture` line. The lines are the same; their order across the two threads
  is not fixed.
- **Not tested:** the 3- to 8-player splits and a front-end ride. A player joining mid-ride is handled
  (the worker is made whenever there is a window, and auto checks the seat count every tick) but was not
  run.
- I did not run `--sim-thread on` over every track or the biplane/Kiowa/car paths. The argument that it
  is exact is that the same `engine.step` runs on the same moved world. The hashes check it on free06's
  bike only.

## Next

Follow-ups filed: F24 (the run-ahead for a smooth threaded picture), F25 (the sim thread in tiled
worlds), R20 (a Deck measurement of `auto`), all low.
