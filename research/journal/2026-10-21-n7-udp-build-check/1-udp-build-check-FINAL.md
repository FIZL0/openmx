# 1 — udp build check (FINAL)

Task N7. 2026-10-21.

## What I did

- **Every `--peer` datagram carries the build id** (`crates/openmx/src/net/udp.rs`): `omx`, the id's
  length (one byte), the id, then the session's bytes. The id is the lobby's own,
  `openmx_net::handshake::build()` ("1.0.0 dev, 30 Hz" on a dev build, since N6), so the UDP path and the
  lobby refuse the same builds. Per datagram rather than a hello exchange: nothing to lose, retry or
  hold back, and the session's datagrams (≤ `link::DATAGRAM_MAX`, 1200) stay well inside the 2048-byte
  receive buffer with ~20 bytes more.
- **A mismatch ends the ride with both ids.** `Udp::receive` returns `Err` on a datagram from a named
  peer whose id differs ("g runs build "1.0.0 dev, 60 Hz" and this machine "1.0.0 dev, 30 Hz": two
  builds can't ride together") or that has none ("g sends no build id (a build from before N7) …").
  `Online::receive` passes the `Err` on, and `Online::frame` already finishes the session with it and
  prints it, as it does for the lobby's ends. Datagrams from an address nobody named are still dropped.
- **The refused peer is told too.** On refusing, the machine sends its bare header (id, no session
  bytes) three times to that peer before it stops. Without it the first live run left the other side
  waiting silently: the refusing machine's own early datagrams had gone out before the other's socket
  was bound, and it sent nothing after quitting.
- `Udp::bind_as` takes the id explicitly, for the tests.
- opentennis has no plain-UDP path (its online is iroh only), so nothing was taken from it here.

## What the original does

Nothing: the original has no online play. This is the shell's transport only.

## How it was proved

- `net::udp` unit tests (`./kit check`, 5 tests): a same-build datagram round-trips with the id taken
  off; another build's is refused by name and the sender gets this machine's id back; an untagged
  (pre-N7) one is refused; a stranger's is dropped; and two real `Udp`s on different ids, one datagram
  sent, both end with the other's id.
- The real shell, by hand, two `openmx --headless --ride --online … --peer …` processes over loopback:
  - new against new, 600 frames: both 592 ticks, 0 desyncs, same `state before tick 300 0x0fd29d234106b766`.
  - new against the branch base (d41c2c8, pre-N7): the new one says "g sends no build id …" and both
    ride 0 ticks.
  - new against a copy of this change built with the id "1.0.0 dev, 60 Hz", each started first in
    turn: both print the other's id and ride 0 ticks.

## Not verified / not 1:1

- **A pre-N7 build is not told why.** It cannot be: it reads the new datagrams as session bytes. In the
  run above it rode 0 ticks and said nothing; only the post-N7 side explains. Nothing to fix from this side.
- **The "60 Hz" peer in the live run was this 30 Hz code with only its id string changed**, not an
  actual pre-N6 60 Hz build: the check compares ids only, so what the build does at runtime does not
  enter into it. A pre-N6 build would also have no id (it is pre-N7) and takes the untagged path,
  which the run against d41c2c8 covers.
- **Three answers in a row, sent once**: if all three are lost the refused peer waits without a reason
  (until its own `--frames` or the session's quiet-peer handling). Not tested on a lossy link.
- **Same id, different code**: two dev builds of one version with different commits still meet, as in
  the lobby (`handshake::build`'s own rule unless the shell stamps a commit). Not this task's to change.

## Next

Nothing; the task is done.
