# 1 — n5 online ride codec (FINAL)

Task N5. 2026-10-20.

## What I did

- `net::rides::Rides::encode`/`decode` (`crates/openmx/src/net/rides.rs`) go through N1's
  `game_sim::snapshot` codec: a u32 seat count, then each seat's `snapshot::encode` bytes behind a u32
  length. `decode` refuses another seat count, a cut-short or over-long state, and any seat that
  `snapshot::decode` refuses. It decodes onto a copy of every seat, so nothing is written unless all of
  them load. The constants stay this machine's own, because `snapshot::decode` keeps them.
- With a codec in place, N2's resync (`net::resync`) and restart (`net::recover`) paths run on the real
  ride with no change to either. They were already written against `Game::encode`/`decode`, and only
  their "this ride cannot send its state" fallbacks are now dead on the ride.
- tests/netplay.rs: the two ride-side tests N2 left as "caught but not mended" are now the resync and the
  restart on the ride:
  - `a_desync_on_the_ride_is_caught_saved_and_resynced` keeps N2's checks (both sides see it at
    checksum 330, the report replays to the host's hashes) and adds the Tally test's resync checks: one
    resync each side, nothing out of sync after it, more than 300 settled ticks agreeing after the tick
    it was taken at, agreement before the glitch and disagreement at 330. That runs 900 rounds at 5 %
    loss and 3 rounds of lag. The Ride glitch fires after the step that makes `ticks` 320, one tick
    before Tally's, so the "agreed before" slice is `..320`.
  - `on_the_ride_a_silent_peers_seat_empties_and_the_others_ride_on` replaces
    `on_the_ride_a_peer_that_does_not_come_back_ends_it`. `h` goes silent for good at round 300 under
    10 % loss. The line-up becomes `host,-,g`, there is one restart and no end, every settled tick from
    0 agrees on the two remaining machines (more than 300 past the restart), the empty seat's pad is
    unplugged, and its bike still rides on.
  - New `the_ride_says_every_seat_in_bytes`: three seats ridden 200 ticks, encoded and decoded onto a
    fresh ride, give the same hash and the same bytes, and both then ride 200 more ticks to the same
    hash. A two-seat state, a cut-short one and one with a byte too many are refused, and the hash is
    unchanged after each refusal.
- The shell draws every seat online (`crates/openmx/src/online.rs`, `main.rs`):
  - Online, `--seats` decides how many riders the shell makes (`Online::seats`), placed where local
    players 2..N would start.
  - Every machine hands seat `s` the same start (`Online::start` takes one `Sim` per seat), so the seats
    stand side by side and no longer all on the start line's one spot as in N2.
  - Each frame `Online::show(d, …)` copies seat `drawn()[d]` into shell seat `d` (this machine's first,
    then the others in seat order) and poses its body from the frame the tick hook chose for that seat.
    The hook now keeps a pose for every seat, not only this machine's.
  - Seats past the local players are posed and drawn with no camera of their own (the per-seat loop
    `continue`s before the camera).
  - A seat nobody sits in (`-`, or a peer whose seat was emptied) is not drawn.
  - The `--online` guard now checks `args.players != 1` and not `world.seats.len()`.
  - `--shot-frame` counts online frames, as `--frames` already did. Before this a shot online was always
    frame 0, because the engine does not run online.
- Taken from opentennis: nothing new. N2 had already brought over its resync and recover. Its
  snapshot `encode`/`decode` came in through N1.

## What the original does

Nothing to read: online play is the port's own. The original has no network ride. The simulation inside
every seat is the ported one, unchanged.

## How it was proved

- `./kit check`: TESTS PASSED (816 tests in this run, a shared count). That includes tests/netplay.rs's
  14, among them the two ride tests above and the codec test.
- By hand, from the worktree, with two `--headless` release shells on 127.0.0.1:
  - `--seats host,g,g`, 600 frames: 0 desyncs on both machines.
  - The same line-up, 400 frames, `--shot-frame 300`: g's picture shows its own rider and another seat's
    rider beside it, both posed.
  - `--seats host,-,g`: the host's picture shows its rider and g's far off to one side, and no bike in
    the empty seat.
  - The screenshots are not committed. They are game data.

## Not verified / not 1:1

- **One `Sim` a seat, not S54's one `Sim` of N riders.** The brief allows either. Moving the online ride
  onto `Sim::others` would bring in the original's rider-to-rider contact and step order, and would
  change the shell's online loop and N2's tests as well. I kept N2's shape, so online riders still pass
  through each other. The state codec would carry either shape.
- **The resync and restart were proved only in the headless harness.** In the two-shell run on the
  socket no desync happened and no peer went silent, so the two paths never ran over real UDP. The
  harness drives the same `Netplay` and `Rides` over a simulated lossy link.
- **The state's size on the wire was not measured.** The resync test shows it goes over a 5 %-lossy link
  in N2's 1 KB reliable pieces and is taken within the 900 rounds. How long it takes with 8 seats was not
  timed.
- **Other seats' bodies are posed only from the last tick that ran in a frame**, as this machine's
  already was in N2. When a frame runs two ticks or a rollback, a `restart` flag on the earlier tick's
  pose is lost. The body *state* is in the Sim and rolled back. Only the drawn pose's cross-fade can
  skip.
- **The body's machine runs inside the tick only on a machine with the rider's clips loaded** (N2's
  hook, unchanged). A machine that has no body model would step the seats' `body_frame` differently and
  desync. I did not test this: every machine here reads the same disc.
- **Empty seats are hidden, and their bikes still ride on with an unplugged pad.** That is my own choice
  of what "a seat empties" looks like. The original has nothing to compare it with.
- **`four_peers_and_eight_riders_on_a_bad_network` (N2's) failed once.** It failed ("g waited 74
  rounds", against a limit of 60) during the `./kit done` gate, with 3 test runs going at once. It then
  passed 11 times in a row on its own. Its path never calls `Rides::encode`/`decode`: there is no
  desync and no restart, and `save`/`restore`/`step`/`hash` are unchanged. So I take it to be N2's own
  intermittent failure, not N5's. I did not prove that on main. Every clock in the path is ggrs' driven
  one. A likely source is ggrs' `HashMap` iteration order (29 uses in third_party/ggrs), and I did not
  check it.
- **The online riders' start spots are the local split screen's** (SEAT_SPACING to the start line's
  right and left). Where the original puts a second rider was not read (R9, V22), so this is the port's
  own placement here too.

## Next

Nothing for N5. A later task could put the online ride on S54's multi-rider `Sim` (contact, step order).
