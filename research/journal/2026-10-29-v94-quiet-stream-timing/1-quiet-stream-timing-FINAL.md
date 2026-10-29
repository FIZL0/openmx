# 1 — quiet stream timing (FINAL)

Task V94. 2026-10-29.

## What I did

- Re-ran V92's 30 s gas ride on world_ultra. It is the same command as the ignored
  `world_ultras_gas_ride_streams_inside_a_frame`: `target/debug/openmx --headless --size 1920x1080
  --mods ~/repos/openmx/mods --track world_ultra --timings --play <900-frame gas tape> --frames 900`.
  - The world loads wrapped now (`"wrap": "xz"`, 121150 x 40060 ft, 77 tiles, 4139 joins), with the
    stand-in at 891 x 295 cells of 136 ft, built in 0.58 s.
  - I waited for a quiet window: a loop read `/proc/loadavg` until the 1-minute load dropped under 6
    (it hit 4.9 at 04:51). The quiet run then went at load 4.6-4.8, from start to end.
- `crates/openmx/tests/world_far_tiles.rs`: the ignored ride test now also counts the ground pieces
  built. It fails on any `world ground: waited` line, which the stream writes when it holds a frame for
  a piece a rider is on that is not built yet.
- `crates/openmx/src/worlds.rs`, new `#[cfg(test)] mod heading_order`: a 5x5 world of 1000 ft pieces
  over a `GroundStream` that builds nothing.
  - It tests `ahead_first`, and the builder queue that `update` makes from it.
  - Standing still, ahead and behind tie on their gap.
  - After 100 ft east: east < north = south < west. In the queue, the piece underfoot comes first, and
    the piece two ahead (gap 1400) comes before the one behind (gap 600).
  - After turning and riding 100 ft north: north comes before east.
  - After a teleport (more than 2000 ft): no heading, so the gaps alone decide.
- Nothing was taken from opentennis. It has no tile streaming.

## What the original does

Nothing to match. This is the port's own world streaming (V92, MD17, MD28). The original has no worlds.

## How it was proved

- **Quiet ride (load 4.6-4.8)**, with the `target/debug/openmx` that `./kit check` had just built for the test. Session line:
  `sim 0.59 stream 0.24 ground 1.55 present 9.72 pace 4.35, all 16.70; worst 24.16 ms (present 16.61,
  ground 4.00), 14 over 16.7; peaks: stream 4.44 ground 4.35 present 18.39`.
  - The 14 slow frames had a stream stage of 0.48-1.19 ms, with one at 4.44 ms (tick 334, as a row of
    joins arrived). Their total time came from `present` (10.5-18.4 ms) and `ground` (2.3-4.4 ms).
  - V92 had seen every frame over 16.7 ms at load 45-66.
  - No frame's stream stage was over 16.7 ms. The stream is at most 4.44 ms, about a quarter of a frame.
- **The same ride under load, for contrast**:
  - load 52-71: stream peak 12.57 ms, 677 frames over;
  - load 5 → 95 during the run: stream peak 12.01 ms, 905 frames over.
  - The stream stayed under 16.7 ms on both, but its peak tripled with the load.
- **`./kit check -- -p openmx --test world_far_tiles -- --ignored world_ultras_gas_ride`** passed, with
  the no-wait check. It ran under load (23 → 70). It did not skip: its gas tape was written, and the tape
  is written only after the mods check.
- **The tile ahead lands first, in the ride.**
  - The first frame builds 105 of 4216 pieces near the start (59081, 22882). Over the 30 s ride, only 8
    more pieces stream in, as two rows of MX vs. ATV joins come within the 6000 ft load reach going
    south (−z).
  - There were 0 waits in every run.
  - For one extra run, I printed the rider's place on each log line (a local edit to main.rs, reverted,
    not committed):
    - **Row 014 (z 15120-16200), ticks 334-341.** The rider is at about (59309, 22266), heading
      (0.075, −0.997) over those ticks. The four joins tie on their z-gap of about 6066 ft. They landed
      j022 (x 59544-60624, cos 0.999), j023 (cos 0.980), j018, j017 (west, behind the line of travel).
      That is cos order.
    - **Row 013, ticks 828-835.** The rider is at about (59826, 21180) and has turned east, heading
      (0.56, −0.83). It is inside j022's x range, so j022 and j023 still tie on the z-gap. They landed
      j023 (cos 0.925), then j022 (cos 0.851), then j018 and j017.
    - The order between j022 and j023 flipped from one row to the next with the rider's turn. With the
      gap alone they would tie both times and land in the same index order. So the flip is the heading's
      doing.
- `./kit check -- -p openmx --bin openmx -- heading_order` passed (1 test).

## Not verified / not 1:1

- **The quiet run is one run.** The 1-minute load was 4.6-4.8, not zero: the other slots' agents were
  idle, not stopped. A second back-to-back run was hit by a load spike to 95, so only one fully quiet
  sample exists.
- **The 14 frames over 16.7 ms in all were not chased.** They are `present` and `ground` (drawing), not
  streaming, which is out of this task's scope (V92's criterion is the `stream` stage). The binary is
  the test's own build, not a release build. Present at 1920x1080 headless
  includes the frame's readback, which a window does not do. Neither was measured here.
- **The ride exercises the heading order only lightly.** Only 8 pieces stream in mid-ride on this 30 s
  gas ride (the 6000 ft reach covers most of it), and every pair compared tied on gap. So the ride shows
  the tie-break by heading. It does not show heading beating a nearer gap (`AHEAD` = 0.5). That case is
  shown only by the synthetic unit test (gap 1400 ahead before gap 600 behind).
- **The rider-place print that showed the per-join cos was a one-off local edit, not a test.** The
  committed ride test checks only that there are no waits, not the order. The shell does not log the
  rider's place on its stream lines, and I did not add that to shipped output.
- **The prop/dressing stream** (`world props:` lines) shares the `stream` stage and was timed with it.
  Its own order is not part of `ahead_first` and was not checked.

## Next

Nothing for this task.
