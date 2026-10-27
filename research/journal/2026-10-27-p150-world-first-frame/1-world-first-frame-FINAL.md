# 1 — world first frame (FINAL)

Task P150. 2026-10-27.

## What I did

MD28 left a world's first frame at 26 ms on world_mxu (tick 1: `ground` 7.67 ms building the first
view's per-piece index lists in gpu.rs `ground_for`, `present` 15.13 ms for the first GPU frame).
Measured again at HEAD 393d41c4 (after V92's stand-in landed), headless 1920x1080, user's mods, 120
ticks, `--timings`, load average ~31: tick 1 was **126.92 ms** (sim 7.40, ground 26.28, present 85.54).
A timer round `engine.step` showed the 7 ms of sim was the first step itself: the bike's searches
asking `Tiled::near` for the first time, which builds `every_shape` and every shape's box over the
whole world, then cuts the start square.

Two things now happen before the ride's clock starts, behind the loading screen (the window has not
presented a ride frame yet, so it still shows the last loading picture):

- **The warm frame** (main.rs, in the GPU draw): the ride's first picture — the same `Scene3`, after
  `set_graphics` / sprites / glows — is drawn once offscreen at the frame's size and `finish`ed before
  the real draw. That builds the views' ground parts (`ground_for` keys them by view, piece and block,
  and the real draw's picks are the same, so it reuses them), compiles the pipelines and does the first
  uploads. Its time is left out of the stages (`Stages::skip`, new), of `perf`'s draw, and of the ride's
  schedule (`Pacer::hold`, new: the ride's start moves on by it, so the pacer owes no ticks for it
  and a `--seconds` ride is as many ticks; a first try that added it to the stall time instead made
  fixed_rate.rs's 3 s ride 89 ticks, not 90). Printed as `warm frame: N ms behind the loading screen`.
- **The near shapes** (tiles.rs `Tiled::prepare_near`, new; called before the loop for each seat's
  start position): `every_shape`, the boxes and the near sets of the rider's square and its four
  neighbours. These are kept caches and pure functions of the source and the square (the doc on `near`
  says so: "a search is the same whichever rider asked first"), so the ride is unchanged; proved by
  the ride hash below. Printed as `world shapes: near sets cut in N ms`.

`--warm on|off` (on by default) turns both off, for measuring and for the test.

Not taken from opentennis: it has no streamed world and no first-frame warm-up to copy.

**After**, same command, two runs each back to back (load average 7-16):

| | tick 1 | worst frame | frames over 16.7 | ride hash |
|---|---|---|---|---|
| `--warm off` | 57.42 / 64.05 ms (ground 13-15, present 36-39, sim 4-5) | the same | 14 / 188 | 0xc2b37daa93e7d571 |
| `--warm on` | not a slow frame (under 16.7) | 29.66 / 15.69 ms (later ticks) | 1 / 0 | 0xc2b37daa93e7d571 |

The warm frame itself took 45 ms and the near sets 3.6 ms, both at the load. At load ~31 (before the
other slots quietened) tick 1 with the warm-up was 28.08 ms, below that run's mean frame of 29.6 ms:
every world_mxu frame was over 16.7 then (present ~19 ms a frame, also with `--world-stand-in off`
at ~15 ms), so tick 1 was no longer an outlier but nothing could be under 16.7 either.

## What the original does

Nothing to port: this is the shell's frame schedule and the GPU renderer, both ours (AGENT.md:
rendering is ours). The simulation is untouched, which the ride hash shows.

## How it was proved

- `crates/openmx/tests/world_first_frame.rs`, `a_worlds_first_frame_is_not_slow_and_the_ride_is_the_same`
  (in the gate): a generated nine-tile world ridden 60 ticks on the gas headless with `--timings`, with
  and without `--warm`: the ride hash and end point are the same to the bit, `--warm off` draws no warm
  frame, the warm frame's line comes before the first timings line, and with the warm-up there is no
  slow frame at tick 1.
- `world_mxus_first_frame_is_under_a_60hz_frame` (ignored: the user's mods and a quiet machine): the
  user's world_mxu, 120 ticks at 1920x1080, no slow frame at tick 1. Passed here at load ~9.
- The world_mxu table above; the 120-tick ride hash 0xc2b37daa93e7d571 is also the one HEAD 393d41c4's
  binary gave before any change.

## Not verified / not 1:1

- **The gate test does not fail without the fix.** On the generated nine-tile world tick 1 without the
  warm-up is ~4 ms (worst 3.93 against 2.08 with it), well under 16.7, so its "no slow frame at tick 1"
  holds either way; the cost only shows on a big real world. The world_mxu proof is the ignored test
  and the table, which need the user's mods and a quiet machine. The gate test's own teeth are the
  bit-exact ride with and without the warm-up and the warm frame's place before the ride.
- **The windowed first frame is not measured.** The warm frame is offscreen; the window's first
  present (surface acquire, the swapchain's first use) is still in the ride's first frame. On this
  locked desktop a window's present is throttled by the compositor (MD28), so it could not be timed.
- **"Behind the loading screen" is not checked on screen.** The warm frame comes after the load's
  thread ends and before the ride's first present, so the window should still show the last loading
  picture for those ~45 ms; not looked at (no screenshots of the window here).
- **Online**, the warm frame is a ~45 ms pause on this machine at the session's start; `Pacer::hold`
  moves the local start by it and the rollback absorbs the rest as it would any stall. No netplay
  test was ridden with it beyond the gate's.
- **The near sets cut are the rider's square and its four neighbours**; a first search whose bundle
  centre falls in a diagonal square still cuts that one in tick 1 (less than a millisecond; the whole
  world's `every_shape` and boxes, the bulk, are always done at the load).
- **Steady world_mxu frames are over 16.7 ms on a loaded machine** (present ~19 ms at load 31, ~10 ms
  quieter): not this task's, and the GPU is shared with the other slots, so not filed as a regression.

## Next

Nothing; the task is done.
