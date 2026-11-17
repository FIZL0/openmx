# 1 — lobby tests ride until (PART)

Task K30. 2026-11-16.

## What I did

- `tests/lobby.rs`, the_lobby_s_ride_goes_over_the_lobby_s_connection_in_step_and_a_leaver_lets_the_host_ride_on:
  rides until 60 ticks have settled on both machines (was: 120 rounds, then assert); 900 rounds is the hang guard.
- Same test, after the guest leaves: the host rides until its ride has one rider (was: 90 rounds, then assert),
  the same fixed-count shape.
- a_lobby_ride_draws_each_peer_in_their_own_look: rides until each machine has run 30 ticks and draws shell
  seat 1 with the other's pick (was: 60 rounds); 900-round guard.
- `src/lobby.rs` needed no change. Nothing in ../opentennis applies (its lobby tests have no ride).

## What the original does

Not applicable: test harness only.

## How it was proved

`./kit check -- -p openmx --test lobby`: 92 tests pass.

## Not verified / not 1:1

- Not run under heavy load to show the old flake is gone: no load repro was set up; the change removes the
  fixed-count race by construction.

## Next

none
