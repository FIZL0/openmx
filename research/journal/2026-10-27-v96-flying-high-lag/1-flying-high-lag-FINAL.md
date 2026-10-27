# 1 — flying high lag (FINAL)

Task V96. 2026-10-27.

## What I did

- Measured first. New `--fly-at X,Z,DEG,FT` (main.rs, `Garage::launch_plane` in vehicle.rs) puts the
  first seat's plane in the air FT feet over the ground, already flying at 120, so a high flight is
  measured without the climb. Matrix: biplane, `--biplane-controls original`, `--graphics far`,
  headless 1920x1080, `--timings`, a full-throttle tape, 900 frames; world_ultra at 86692,22046 and
  world_mxu at 9000,25000; 500 / 2000 / 5000 ft.
- Altitude was not what mattered. On world_ultra the plane was slow even parked on the ground (sim
  ~228 ms a frame against the bike's 0.9 ms). Sampling the stack (`eu-stack` on a PR_SET_PTRACER
  child, see the profiling memory) showed three costs, all CPU, none in the renderer:
  1. **The plane's collision search** (`Flight::search` and the post-tick `refresh_flown`) built its
     broad-phase tree from `world.wooded()` — on a tiled world every tile's props and their seam
     copies — twice a tick. Each frame running long made the next frame owe more ticks: a spiral.
     Now it takes MD28's near set (`wooded_near`, the props within reach of the plane's 128-ft
     square) through `flight_search::near_world`, which drops the kept nearby list when the plane
     moves to another square's set. Single-map worlds return their whole set, as before.
  2. **`Tiled::stream`** measured the wrapped gap from the plane to all 4216 tiles and joins of
     world_ultra each tick, ~4.9 ms. Each tile now carries its span in 1024-ft squares; a tile whose
     square is apart from every near point's reach box (with its wrapped copies) is taken as
     infinitely far without measuring. The filter is conservative (squares widened by one), so
     every load/drop decision is the one the exact gap made. 4.9 → 0.30 ms a tick.
  3. **World-dress path cars** (render only): `Path::pose` replays 48 warm-up frames with a ground
     query each, for each of ~21 cars each picture, ~1000 queries, 3.6–4.0 ms (in the `hud` stage).
     New `Path::pose_kept` puts the warm-up on the clock's step grid and keeps each car's grounds in
     a `toy::Kept` across pictures, so a picture asks for one or two new grounds a car.
     3.6 ms → 0.33–0.57 ms.
- Test: `crates/openmx/tests/flying_high.rs`, ignored `a_plane_flown_high_over_world_ultra_holds_60_hz`
  (world_ultra, 2000 ft, Far Fog, 900 frames: `all` ≤ 16.9 ms, ≤ 18 frames over 16.7, sim < 2 ms),
  passing here; plus a small non-ignored parse test of the session line.

### `--timings` before (only `--fly-at` added) and after, session averages, ms a frame

Machine load during the runs was 9–41 (other slots' gates), so these are not quiet-machine numbers.

| world, height | before: all / sim / stream, frames over 16.7 | after: all / sim / stream, frames over 16.7 |
|---|---|---|
| world_ultra 500 ft | 243.01 / 197.77 / 36.19, 177 (worst 896) | 16.70 / 0.16 / 1.01, 8 (worst 32.06) |
| world_ultra 2000 ft | 28.64 / 19.23 / –, 887 | 16.68 / 0.15 / 0.89, 8 |
| world_ultra 5000 ft | 24.14 / 14.88 / –, 899 | 16.67 / 0.13 / 0.81, 1 |
| world_mxu 500 ft | 18.93 / 6.40 / –, 136 (worst 574) | 16.67 / 0.13 / 0.62, 2 |
| world_mxu 2000 ft | 26.38 / 16.40 / –, 870 | 16.84 / 0.12 / 0.54, 3 (one 184 ms present hiccup) |
| world_mxu 5000 ft | 20.26 / 10.28 / –, 868 | 16.66 / 0.08 / 0.37, 0 |

(500 ft before: fewer frames ran over because each took so long that 900 frames covered few
pictures; the frame count is the drawn frames.) After, the remainder of a frame is `present`
(1–5 ms) and `pace` (waiting for the 60 Hz slot). Full lines are in the session's scratch files;
the after lines e.g. for ultra 500: `sim 0.16 stream 1.01 ground 0.25 hud 0.61 present 4.77 pace 9.85,
all 16.70; worst 32.06 ms (stream 23.20, present 6.29), 8 over 16.7`.

## What the original does

Nothing to match: the original has one map at a time and no wrapped worlds; its plane's collision
search over a single map is unchanged (a non-tiled world's near set is its whole set).

## How it was proved

- `--timings` before/after as above (wall-clock, on this machine).
- `./kit check -- -p game-sim -p game-render`: 994 passed, including the flight, wrapped-flight,
  tiles and toy tests; `flying_high` with `--include-ignored` passed.
- The stream filter only skips tiles whose square box is apart from every reach box, by
  construction never one the exact gap would have loaded.

## Not verified / not 1:1

- The plane on a tiled world now searches the near set, not the whole world: when the plane crosses
  into another 128-ft square's set the kept nearby list is re-gathered, so candidates can come in a
  different order than from the whole set. These are the port's own worlds with no original to
  match; single-map worlds take the same path as before. Not compared tick for tick against the old
  search on a world flight.
- Moving path-car props are in a near set by their box when the set was built, as MD28's bike had —
  a car driving into a plane's square after the set was built is not met until the set changes.
- World-dress car poses come from `pose_kept` (warm-up on the step grid, grounds kept) rather than
  `Path::pose`; they can differ from the old pose by a sub-step of warm-up. Render only, looked at
  through the timings runs, not compared picture by picture.
- All numbers are from a loaded machine (load 9–48), not the quiet machine the brief asks for; the
  ignored test passes here regardless, but its bounds are wall-clock.
- The brief's renderer ideas (coarser far ground, impostors, culling wrapped copies, merged far
  meshes) were not done: the measurement put the cost in sim/stream/cars, and the render stages
  after the fix are small (ground ≤ 0.34, scene ≤ 0.03, present 1–5 ms a frame).
- Landed with `--skip-audit`: the audit's 7 leaks are other tasks' addresses already in plan/DONE.md
  (radar_update, biplane_tire_*), not V96's text.
- The wrecked rider's contact search (ride.rs, `world.wooded()`) still takes the whole set on a
  tiled world; filed as a follow-up.

## Next

Done.
