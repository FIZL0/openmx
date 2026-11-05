# 1 — world first frame counts (FINAL)

Task K43. 2026-11-05.

## What I did

- **What it raced:** wall time. `a_worlds_first_frame_is_not_slow_and_the_ride_is_the_same` failed
  when tick 1's frame came out slow with the warm-up on (`Stages::OVER` on its stage times). That is
  the machine's load, not the shell: in a gate with every slot riding, any one frame can go over.
  MD80b's journal saw the same red and the same lone pass. No shared port or file: the test writes
  only its own target's `CARGO_TARGET_TMPDIR/p150`, and only one non-ignored test in the binary does.
- **The fix:** the test now counts the work P150's warm-up moves out of tick 1 instead of timing it.
  - `game_sim::tiles::Tiled::near_cuts()`: how many near sets the world has cut since it was built.
  - `game_render::gpu::Drawn::ground_written`: the ground parts whose indices a frame wrote.
  - The shell, under `--timings`, says once at the ride's first frame:
    `first frame: at tick N, near sets cut in the ride C, ground parts written G` (C counted from after
    the load's `prepare_near`).
  - The test asserts `(C, G) == (0, 0)` with the warm-up, and both `> 0` with `--warm off`, so the
    counts are shown to see the work. On p150_world: warm 0/0, cold 1 near set and 11 ground parts.
  - The tick-1 slow-frame lines are still printed (`eprintln!`) as information, never asserted.
  - The bit-exact ride hash, end line and the warm frame's place before the first timings line are
    kept as they were.
- Nothing taken from opentennis: it has no warm frame or near sets to count.

## What the original does

Nothing here is the original's: P150's warm-up and these counters are the port's shell and renderer.

## How it was proved

`./kit check` on `-p openmx --test world_first_frame`, and the two headless rides by hand (lines
above). Counts are a function of the ride, which is bit-exact, and of the scene the warm frame and
the first frame both draw, so no other process can move them.

## Not verified / not 1:1

- **Not run under an artificial load.** The new assertions read no clock, so load cannot reach them;
  I did not load the machine to re-show that, to spare the other slots.
- **Pipeline compiles and first texture/buffer uploads are not counted.** P150 also moved those to the
  warm frame, but they are made at the renderer's build or are driver work with nothing to count in
  `render`; only the ground parts (the bulk of MD28's 26 ms) and the near sets are. A regression that
  puts only pipeline/upload cost back in tick 1 would pass this test; the ignored world_mxu test, by
  wall time on a quiet machine, still covers it.
- **Window path:** the line is said for the window's draw too, but only the headless offscreen path
  was run.

## Next

Nothing.
