# 1 — online car script on ticks (FINAL)

Task K29. 2026-11-05.

## What I did

K23's merge-gate log for the red run was already overwritten (`context/notes/check.log` holds one run),
so which of the binary's tests failed and how is not on this disk. K29 was filed 11-04 10:06, before
K27's and N25's fixes (11-05) to the car-left line read off a predicted frame; both are on this branch.
I looked for what the two-shell tests (`ride_online`) still raced and found two things:

1. **`--online-script` ran on the shell's frames, not the session's ticks.** `main.rs` fired the word
   for frame `F` when its loop's frame counter (`online_frames`) hit `F`. Since K10, `--frames` counts
   the session's ticks online, and frames run on the wall clock: frames spent waiting for the peer's
   load or a stalled pad run no tick, and headless frames run at about 60 a second against 30 ticks a
   second. So frame 330 ("bike") normally lands near tick 165. Under load the frame rate drops, and
   below about 24 fps frame 330 is past tick 420: the ride ends before the switch back, or (if a peer
   loads late) all the words fire before the first tick. Now each word fires once, in tick order, on
   the first frame whose next tick is `>= F` (`online_scripted`, a cursor into the script, which is
   sorted by tick as it is parsed). Help text and the field's doc say "session tick". The switches now
   happen at ticks 40/200/330 (and 60/260/370), later than the old ~20/100/165, still 50+ ticks
   before the ride's end, so they settle before it.
2. **The test's ports were picked free, then bound later.** `ride_online` bound `127.0.0.1:0`, dropped
   the socket and passed the port to a shell that binds it only after loading the world (seconds under
   load): another slot's test could take the port in that gap. Now nothing is dropped and rebound. The
   host shell binds port 0 and prints the port it got (`online: host on 127.0.0.1:P`, now `udp.at()`
   rather than the address it was given). The test reads that line from the host's stdout as it comes,
   then starts the guest with `--peer host@127.0.0.1:P` on its own port 0. `net::udp` learns a `--peer`
   named with port 0 from the first datagram from its IP that starts with this path's bytes (any
   build's magic, so another build is still told why). One cost: the guest starts loading after the host
   has bound, so the two loads no longer overlap. That makes the test slower (~25 s to ~30 s idle) and
   the wait for the peer longer, which is why race 1 showed up so plainly once this was in.

opentennis: nothing taken. It has no command-line peer pair of shells to start.

## What the original does

Nothing to port: the original has no online play. These are the shell's test hooks.

## How it was proved

- New unit test `net::udp::a_peer_named_on_port_0_is_the_first_to_write_from_its_host`: a stranger's
  junk is still dropped, the port-0 peer is learned from its first datagram and answered there, and a
  second writer from the same host is dropped once the peer is learned. `./kit check -- -p openmx --lib
  net::udp`: 11 passed.
- `./kit check -- -p openmx --test online_car`: 141 passed (disc present; the three shell tests take
  20-30 s, so they rode).
- Stress, N copies of the test binary filtered to `two_online_shells` at once:
  - port change only (script still on frames), load ~80-105: **12 of 18 failed**. In every failure a
    switch was missing or the car never moved (e.g. host `vehicle:` lines `["bike"]` with the monster
    and bike words printed; truck/baja moved 0.4-2.3 ft).
  - HEAD as it was, load ~62-67: 18/18 green (the old red not reproduced on demand).
  - both changes, load ~78: 18/18 green; load 175-188 (8 copies): 24/24 green.

## Not verified / not 1:1

- K23's failing run's output is gone, so I did not see which test failed or how. Race 1 is inferred
  from the code and shown by making the peer wait longer (12 of 18 red). I did not reproduce the old red
  at HEAD itself (18/18 green at load ~65).
- 42 green runs under heavy load are evidence, not proof that nothing else in these tests reads the wall
  clock. The `vehicle:` lines are still printed off the drawn frame (K27's note), and the test compares
  them order-free.
- A port-0 peer trusts the first datagram from its IP that carries this path's magic. That is fine on
  localhost in tests. On a real network a spoofer could take the seat, so the flag is meant for tests
  only, and nothing stops someone using it elsewhere.
- machines_everywhere.rs picks its ports the same racy way. Filed as K40 (low), not changed here.
- K32 (the buggy test's drift) is the same test file and may be fixed already by N25/K27. I did not
  work it, as it is its own task.

## Next

-
