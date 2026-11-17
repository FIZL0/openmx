# 1 — machines everywhere port zero (FINAL)

Task K40. 2026-11-16.

## What I did

- `two_online_machines_start_their_seats_in_the_plane_on_a_race_track` (crates/openmx/tests/machines_everywhere.rs)
  no longer picks two free ports by binding :0 and dropping the socket. Another slot's test could take a port
  between that drop and the shell's bind after its load.
- Now it does what K29 did in online_car.rs: the host starts on `host@127.0.0.1:0` with `--peer g@127.0.0.1:0`,
  the test reads its `online: host on <addr>, …` line, and the guest starts on `--peer host@<addr>`. The host
  learns the guest's address from its first datagram.
- Kept `pad::tests::spawn` (the FIFOS lock) for both shells; the stdout reader now streams lines so the host's
  port is seen while it still runs.
- Taken from opentennis: nothing; the pattern is our own online_car.rs (K29).

## What the original does

Not applicable: test tooling only.

## How it was proved

`./kit check -- -p openmx --test machines_everywhere` passed with the disc present (the online test does not skip
without it, it ran). Both shells still say `2 seats start in the biplane`, 0 desyncs, and the same state hash
before tick 300.

## Not verified / not 1:1

- Not tried under a loaded machine beside other slots' online tests: the race it removes is a timing window
  that can't be forced from here; the fix removes the window instead of shrinking it.

## Next

Nothing.
