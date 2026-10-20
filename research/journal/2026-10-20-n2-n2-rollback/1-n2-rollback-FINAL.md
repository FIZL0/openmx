# 1 — n2 rollback (FINAL)

Task N2. 2026-10-20.

## What I did

Brought opentennis's online play over as `crates/openmx/src/net`, and put it in the shell behind
`--online NAME@HOST:PORT --peer NAME@HOST:PORT … --seats NAME,NAME,-,…` (`crates/openmx/src/online.rs`).

- **ggrs**: `third_party/ggrs` is opentennis's vendored ggrs 0.13 byte for byte (`diff -r` clean), with
  its B494 thread-drivable clock, so the tests drive time a 60th per round. The workspace excludes
  `third_party`; only the openmx crate depends on it (plus bincode and serde).
- **The session** (`net/rollback.rs`), as `hst-play/src/play/rollback.rs` runs it: input delay 2, an
  80-tick prediction window, desync detection every 30 ticks, opentennis's time sync (frames ahead
  spread over 20 ticks, skip at half a window), its B198 resend of the last inputs every 200 ms while
  stalled, and its hold when a peer owes acks for 120 frames. A seat is a ggrs player; empty seats
  (`-`) are not, and step with an unplugged pad.
- **The pad on the wire** (`net::wire`/`unwire`): each seat's input is the first 20 bytes of the
  original's 32-byte pad report (status, mode, buttons, the four sticks, the twelve pressures) —
  `game_sim::input::Report` itself, which the port's input manager reads. The 12 bytes after are
  always the same and are left off. `Pad::default()` is the idle pad (ggrs' "nothing yet" input).
- **Desync** (`net/desync.rs`): `DesyncDetected` → "Out of sync" said, a text report of every seat's
  pads up to now written once to `context/net/desync-*.txt`; `Report::read` + `replay` ride it again
  offline and give the per-tick hashes.
- **Resync** (`net/resync.rs`, opentennis's B154b rollback half): the host sends its state before the
  last tick every pad is in for, out of ggrs' own cells, in 1 KiB pieces; a peer takes it once it has
  every pad before that tick too, re-runs the ticks since on its log and says `Resynced(n)`; hashes of
  ticks before `n` are not reported again.
- **Silent peer** (`net/recover.rs`, B196): keep-alives every 0.5 s; host says a peer is away after
  3 s (60 s before first heard), everybody shows "Waiting for … N s"; back in time, nothing happens;
  still away after 20 s, the host restarts the session from a confirmed state with that peer's seats
  empty (new ggrs session, round + 1, old datagrams dropped). A silent host: the others end the ride
  after 10 s more.
- **Transport** (`net/link.rs`, `net/udp.rs`): opentennis has a reliable stream beside ggrs'
  datagrams; here there is one datagram transport and the session's own messages ride a numbered,
  acked, resent channel over it. A ggrs input message bigger than 1200 bytes (eight seats, a second
  of unacked frames: 1291 bytes seen in the 4-peer test) goes in pieces. `udp.rs` is plain UDP to the
  addresses given, until N3's iroh.
- **The ride online** (`net/rides.rs`, `online.rs`): one `Sim` a seat (until S54), each reading its
  seat's pad as pad 0, stepped at one fixed `dt` — `steady_dt()`, what the engine's own 30-frame
  average settles at on a clock exactly 60 Hz (9,600 timer counts a frame), computed through the
  port's integer timer and float, so the same number on every machine. The body's sixty-state frame
  runs inside each tick (an `after` hook), because it is simulation state and must roll back with the
  ride; the pose for this machine's seat is written once the session has run.
- Follow-up task **N5** added: wire N1's codec into `Rides` so the ride resyncs and restarts.

What changed from opentennis on purpose: no Bevy (the session is a plain struct the loop calls), no
lobby/bots/rejoin/"switched away" (N3's menus), the desync report is text, not their JSON+state dump.

## What the original does

Nothing: MX Unleashed on the PS2 has no online play. The only thing taken from the original is the
pad report's byte layout, which the port's input manager already reads.

## How it was proved

`crates/openmx/tests/netplay.rs`, 13 tests, all through `./kit check` (687 passed in the full run).
Rounds of 1/60 s with ggrs' clock driven, deterministic xorshift loss:

- `two_peers_on_a_bad_network` (2 seats each), `four_peers_and_eight_riders_on_a_bad_network`
  (8 seats over 4 machines, 1–3 local each), `a_one_way_outage` (only the host's datagrams lost):
  real `Sim`s on a flat floor, every seat mashing throttle pressure, steering and trick chords;
  20 % loss, 83 ms lag, then 3 s with nothing through. Every machine had the same pads **and the same
  hash before every settled tick** (not only every 30th): 576 / 527 / 576 ticks; nobody waited a second
  outside the outage; no desync; every datagram ≤ 1200 bytes; thousands of ticks re-run.
- `a_desync_on_the_ride_is_caught_and_its_inputs_saved`: one machine's bike nudged at tick 320; both
  see it at 330, both write a report, and the host's report replayed from a fresh start gives the
  host's own hashes tick for tick.
- `a_desync_is_caught_and_resynced` (on `Tally`, a small game of the same pads that can encode
  itself, padded to 3 KB so it goes in pieces): caught at 330, resynced over a 20 %-loss link, one
  resync each side, 300+ settled ticks agree after.
- `a_silent_peer_is_waited_for_then_ridden_on_without`, `an_away_peer_back_in_time_rides_on`,
  `a_silent_host_ends_the_ride`, `on_the_ride_a_peer_that_does_not_come_back_ends_it`.
- unit tests: wire/unwire, line-up parsing, pieces reassembly, `steady_dt`.

And by hand: three release shells on localhost (free06, real collision, bodies posed), seats
`host,g,k,k`, 22 s each: all three printed the same state hash before tick 1200, 0 desyncs, ~60 fps.

## Not verified / not 1:1

- **No resync or restart on the real ride.** `Sim` has no byte codec until N1, so `Rides::encode`
  is `None`: a desync on a ride is caught and saved but not mended, and a peer that never comes back
  ends the ride for everyone. Both are proved only on the `Tally` toy game. Follow-up N5.
- **Other riders are not drawn.** The shell rides and syncs every seat but draws only its own; nobody
  looked at the window during the localhost run (it ran unattended).
- **One pad a machine.** A machine's seats after its first ride the idle pad until P35's seats exist.
- **Riders do not touch.** Each seat is its own `Sim` until S54; they share only the static world.
- **No real network.** Plain UDP on localhost only; no NAT, no iroh, no relay (N3). The lossy link is
  simulated. The localhost run had nobody on the pads (the bikes free-rolled), so real pad bytes
  crossing a socket were not exercised by hand — only in the headless tests.
- **Fixed `dt` online** (`steady_dt`, ≈1/60): offline the engine's measured, averaged frame length
  drives the ride; online a slow machine's ride runs slower in wall time rather than in longer steps.
  Nobody compared the feel.
- **Online, Start does not freeze, M does not swap, and there is no tape** — they would have to go
  through the session's inputs to stay in sync; the flags are refused online. Trick/wreck lines and the
  overlay's trick notes are not printed online (`world.said` is not fed from the session).
- **The body's start gate is zeros** online as offline; a resync or restart (Tally only) re-runs
  the body hook, which is right, but the drawn pose after a rollback is the latest tick's only.
- **Cargo.lock**: ggrs' dependencies are new lines; N1 (running beside this) may add ggrs too, which
  would conflict in the lock file and the dependency list — keep one copy of `third_party/ggrs`.
- `./kit done` with `--skip-audit`: the audit's only findings are plan/DONE.md:161 (S39's tire names).

## Next

Done but for N5 (above). N3 replaces `udp.rs` and adds the lobby.
