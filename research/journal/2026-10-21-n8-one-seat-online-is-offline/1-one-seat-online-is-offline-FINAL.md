# 1 — one seat online is offline (FINAL)

Task N8. 2026-10-21.

## What I did

- `crates/openmx/src/same_ride.rs` (test-only, `#[cfg(test)] mod same_ride` in `main.rs`): rides the same
  pads two ways and compares the whole seat state after every tick.
  - **Offline** is the shell's own code: `main.rs`'s `World` with one `Seat` (no front end, so
    `start_freezes`, bike up), under `Engine::step` on `pace::tick_clock` after `pace::after_load`, the
    engine's input from `engine_input`, the reports written into the seat before the step as `ride`'s loop
    does; then the body's frame (`Rider::body_frame` with the seat's kept hands and the step's `dt`), which is
    the simulation half of the loop's `Body::ride`.
  - **Online** is the session's: a one-seat `Netplay` (`Lineup` "me") driving `net::rides::Rides`, one
    `frame` per tick at `pace::TICK`, with `online.rs`'s `after` hook (the body's frame inside the tick).
    A wrapper `Game` (`Logged`) logs which pad each tick actually ran on.
  - The test asserts: the session runs exactly one tick per frame with no re-runs; tick `t` ran on the
    wire bytes of frame `t − DELAY`'s pad (idle before that); offline handed those same pads at the same
    ticks gets bit-identical `snapshot::encode` bytes after **every one of 1800 ticks** (a minute), and the
    engine's `dt` equals `rides::tick_dt(t)` on each. The state changes every tick and the bike gets 144 ft
    out, so it is a ride and not an idle seat.
  - A second test changes one offline pad (tick 200) and asserts the states part on exactly that tick: the
    comparison can fail.
- `main.rs`: `World` takes its world as a type parameter (`World<P>`, the shell builds it over `Place` as
  before) and `Seat::pass` takes `&mut dyn World` as `Garage::step` already did, so the test rides the
  shell's own `World`/`Seat` over a flat floor. No behaviour change.
- The pads: every button but Start/Select (game bitmask, `Control::*.mask()`), random pressure bytes, both
  sticks through every byte including 0x00/0xff, moments of no buttons, and junk (0xa5) in report bytes
  20..32 offline only — the session never carries them, and the states still match, so nothing reads them.
- opentennis: nothing to take — it has no online-equals-offline proof.

## What the original does

Not applicable: this is port-against-port (the online plumbing against the offline one). The offline ride's
faithfulness to the original is P52/N6/the replay tests' business.

## How it was proved

Port-vs-port replay, bit-exact: `same_ride::one_seat_online_is_the_offline_ride` (1800 ticks, full snapshot
bytes compared every tick) and `same_ride::a_pad_off_by_a_tick_parts_them` (the negative control), both in
`./kit check` (`-p openmx --bin openmx`).

## Not verified / not 1:1

- **A flat floor and the stand-in bike** (`Sim::new`, wrecks off), not a disc track, bike set and rider: both
  paths hand the same world to the same `Sim::step`, so the world cannot tell them apart, but no track's
  props, no wreck/ragdoll and (on a floor) no jump or trick landed in the compared ride. What was proved is
  the plumbing: the clock, the pad's way in, the poll, the body frame, the order.
- **The body's clip table is a made-up one** (every state bound, one-second clips), not the disc's
  `pose_lib`; the shell's `Body::ride` is not called (it needs the renderer's rig) — the test calls its
  simulation half, `Rider::body_frame`, in the loop's place. Reading `ride`'s loop: offline poses only a
  `moving && bike_up && online.is_none()` seat, which the test's one riding seat is.
- **Start differs by design.** Offline without the front end Start freezes the seat (`start_freezes`);
  online nothing freezes (`online.rs`: "no Start freeze"), and under the front end the pause sends an idle
  pad instead. The test's pads hold no Start; with one they would part.
- **A pulled pad differs by design.** Offline a pad in `SeatInput::Waiting` sets `seat.waiting`, which skips
  the poll and the step; online the seat's report goes to the session and the seat is polled and stepped.
  Not ridden in the test.
- **The start state is not compared**: from the lobby every seat's `rng` is the room's seed while an offline
  ride's is 0, and `--online` clones the offline seat. This test starts both from one `Sim`.
- **Only one seat, no remote peer**: rollback re-runs, resyncs and several seats are the netplay tests'
  (`tests/netplay.rs`), which compare machines with each other, not with the offline ride.
- **The 60 s ride**, not an hour: `tick_dt` is constant past tick ~40 (N6's 100,000-tick proof), so length adds
  nothing the first minute does not.
- Seen on the way, not this task's: `tests/netplay.rs`'s `mash` writes Cross as `0x4000`, which is the game
  bitmask's d-pad down (`Control::Cross` is `0x0040`), and Square as `0x8000` (left). The netplay tests
  compare machines with each other, so they still prove what they claim, but the "throttle" their bikes ride
  on is not the throttle. Filed as N9.

## Next

Nothing for N8.
