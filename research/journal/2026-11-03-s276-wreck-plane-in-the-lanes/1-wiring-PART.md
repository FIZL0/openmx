# 1 — wiring

## What is where

- **`wreck_plane::search_downed` / `step_downed`.** These take the riders beside a plane.
  - A rider counts as down when it has a crash, is no ghost, and its wreckage part collides.
  - The search runs `WreckBike::search_flown` with the plane's hull at the copy nearest the bike, on a
    wrapped world.
  - Each mirror goes into `Met::mirrors` with `other = WRECK_FROM + j`, its point moved to the copy
    nearest the plane. The wreck body goes into `Met::wrecks` (collection 0, object j).
  - After the plane's frame, `step_downed` writes the pushed bodies back and runs `step_flown` with the
    plane's flown body, then pushes the plane.
  - It moves the crash's origin and resting to the new place. With `own_step` it marks the wreck bike
    `flown` so that `Crash::step` skips its own tick of it that tick.
- **`search_downed_among` / `step_downed_among`.** The same over a `&mut [Sim]` with a seat and an `on`
  mask, for online's lone-plane path (`net/rides.rs`).
- **`Sim::step`'s plane lane.** It searches before the frame, and passes the `Met` to `frame_met` /
  `frame_props` only when something is down. With nothing down, it runs the frame as before, so the
  replay hash is unchanged. It steps after the frame with `own_step = false`, since the lane does not
  tick other riders.
- **The shell (`main.rs`).** `flown_among` gathers the other seats' riders and the flying seat's
  `Flight`. The loop searches before the flying seat's pass and steps after it (`own_step = true`, since
  the seat order ticks the others after), then reposes the plane.
- **`WreckBike::flown`.** A bool, made and spent within a tick: skipped on the wire, not walked.
  - `Sim` 12144 → 12200 bytes (hash.rs pin).
  - `Ride` 2472 → 2496 bytes (ride.rs pin).
  - The leaf counts are unchanged.

## Tests (`tests/c16_fly.rs`)

`fly()` and `fly_two()` are now wrappers over `fly_by` / `fly_two_by`. Those take the dt, pads and
respawn as closures, plus P2's wreck reason. With reason ≠ 0, `fly_by` seeds P2 down off the image
(`wreck_files`, `carries_wreck`, `wrecked_p2`), and `Tick.p2` then holds the wreck bike body. `fly_two_by`
now calls `search_downed` / `step_downed` itself, in place of its own `wreck_searches` / `wreck_steps`.

- `the_sim_lane_meets_p2s_wreck_bike`:
  - runs 137910 → 137920 at 1/60 through both the Sim lane and the split-screen pass, with Cross held;
  - plane states equal every tick, and the wreck bike moves every tick;
  - the first tick is S274's number to the last bits.
- `the_wreck_bike_leaves_a_far_plane_alone`: 138010 → 138110 with Cross, then idle. Reason 7 and reason
  0 give the same plane states.

## Checks run

- `cargo test --release -p game-sim -p openmx --no-fail-fast`. Everything passed except the following:
  - hash.rs and ride.rs size pins: updated, then green.
  - `prop_near` `a_woken_props_tick_…` and `original_mode` `the_original_switch_rides_…`: these fail the
    same way on HEAD (e385cb39), checked in a scratch worktree with the disc linked.
  - `netplay_browser`'s two desktop-vs-browser tests: the known `--release` version stamp mismatch. They
    were re-run in debug; see the commit message.
- c16_fly: 5 passed, 1 ignored. The capture-reading tests skip (no capture).
