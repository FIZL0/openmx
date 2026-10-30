# 1 — udp two builds port race (FINAL)

Task K11. 2026-10-29.

## What I did

- `net::udp::tests::two_builds_since_the_id_both_say_why` (crates/openmx/src/net/udp.rs) needed both
  machines to know each other's address when it bound them. It got the addresses by binding two plain
  sockets on port 0, reading the ports, dropping the sockets, and then binding `a` and `b` on those
  same ports. Between the drop and the rebind the ports were free for anyone on the machine. Under a
  busy gate there are other slots' test binaries and netplay shells binding ephemeral UDP ports all
  the time. So the rebind could fail (`bind_as(..).unwrap()` panics with "address in use"), or a
  stranger's datagram could land on the port. That is the race.
- Fix: bind `a` and `b` on port 0 themselves with no peers, then name each to the other with
  `Udp::meet` (it already exists for this: "the tests', which bind every machine first"). There is
  no longer a gap where a port is free, and the product code is unchanged.
- The remaining timing in the test is `wait`'s 2 s deadline, and it is not a race. On Linux loopback,
  `send_to` puts the datagram in the receiver's queue before it returns. So `a`'s first `receive`
  already sees `b`'s datagram, and `a` sends its three answers to `b` before that `receive` returns
  the refusal. Each `wait` here ends on its first poll, however loaded the machine is.

## What the original does

Nothing: this is test tooling (our netplay transport).

## How it was proved

- `./kit check -- -p openmx --lib two_builds`: passed.
- Load run: the lib test binary's whole `net::udp::` module, 40 copies at once, 5 rounds (200 runs,
  loadavg about 25–50 from them and the other slots): 0 failures.
- `./kit check --all` through `./kit done`.

## Not verified / not 1:1

- I did not reproduce the old failure. MD37b's gate log was not in this slot. I also did not stress
  the old version in a scratch worktree, because that needs a full extra build of the test binary.
  The race is identified from the code (port freed, then rebound), not from a failing run.

## Next

none
