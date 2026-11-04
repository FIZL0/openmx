# 1 — lobby second player rides to conditions (FINAL)

Task K25. 2026-11-04.

## What I did

`crates/openmx/tests/lobby.rs`'s `a_second_player_on_an_online_machine_sits_down_and_leaves_again` (N20)
failed in C27's merge gate under load and passed alone. That gate's log is gone, so I had no failure
message to start from. I found the race by measuring each step of the test.

**What it raced.** The test drove the two machines a fixed number of rounds (33 ms apart) and then asserted
how far the ride had got: 45 rounds of warm-up, 60 rounds after the sit and then `sim.ticks > 30` on the new
seat, 60 rounds after the leave and then `n > 100` settled ticks. A round is not a tick. `Netplay::frame`
runs none, one or two ticks per call, holding at ggrs' prediction window while the other machine's inputs
are not in, and those inputs cross the lobby transport's own threads. Before that, ggrs' start handshake and
its resends keep wall-clock time. Alone (instrumented): 45 rounds gave 35 ticks, the new seat had 49 ticks
at the check (needs > 30), and the end had 132 settled (needs > 100). A loaded machine delivers datagrams
later, so the same rounds make fewer ticks, and either margin goes.

**Fix (test only).** `both` now takes a condition on both machines, and every step rides until what it
asserts has happened:
- warm-up: until both machines have run 30 ticks;
- after the sit: until the second player's seat has ridden more than 30 ticks;
- after the leave: until the host's seat 3 is empty and both machines have settled 30 ticks past the leave
  (and at least 101, the old check).

The round count (900, about 30 s alone) is now only a hang guard, never the thing being measured: each
step stops as soon as its condition holds. The assertions are the same ones as before. No product code
changed, and gate.toml is unchanged (`--test lobby::` is already in `[flaky]` for the other lobby tests).

opentennis: its netplay tests have the same fixed-loop shape. I took nothing from there.

## How it was proved

- **The race, reproduced.** I put the same perturbation into the old and the new test: the guest's machine
  frames only every third round for the first 150 rounds, like a machine whose datagrams arrive late. The
  old test fails (`and riding: 15 ticks`). The new one passes. (Scratch edit, reverted.)
- The new test passes alone, three times with the whole test process SIGSTOPped for 2 s at random points
  (`stall.sh 2.0 1.0`), and twice within the whole `lobby` binary (79 passed).
- `./kit check --all` in `./kit done`.

## Not verified / not 1:1

- **The C27 gate's own failure is not seen.** Its log is gone. The fixed-round race above is the cause I
  could reproduce and fix, but not proven to be the one that gate hit.
- **Multi-second stalls still red it, on purpose.** These are the product's wall-clock timers, not the
  test's: `recover::SILENT` (3 s, then `HOST_WAIT` 10 s, after which the guest takes the ride over),
  `leave::WAIT` (5 s), and openmx-net's `HOST_GONE` (6 s). With the whole process stopped 6 s at a time the
  test failed twice, both old and new shape: the guest took the ride over, or the lobby handed it the room.
  Stalls of 4 s passed. Making the test immune would mean giving `Online::lobby` a `Timing` (and the
  transport's `HOST_GONE`) the test could stretch, which is product API for a stall the gate should not
  produce. Not done.
- No `./kit check` loop at real gate load (50–76). The load here was simulated with stalls and the
  perturbation above.

## Next

none. K30 (low) takes the same fixed-round shape out of two sibling lobby tests.
