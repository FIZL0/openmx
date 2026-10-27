# 1 — online drop in local seat (FINAL)

Task N20. 2026-10-27.

## What I did

- **Session (net/).** A new `Msg::Sit { preset }` lets a machine already in the ride ask the host for
  one more seat for a local player. The host's `admit::sit` checks that the asker rides and is not
  leaving, that the preset is real (0..PRESETS) and that a seat is free. It then queues the seat
  through N10's pending-join path, with a spawn of `rides::spawn_on(admit.spawn, [(seat, preset)])`.
  When the restart runs, a sit by a peer already in the lineup is logged as "adds a player" and goes
  into `Admit::sits` rather than `joins`. `Netplay::sit(preset)` covers both cases: the host seats the
  player itself, and a guest sends the ask. Dropping that seat again is N11's
  `Netplay::leave(&[seat])` / `Online::leave_session(local)`, which already handled one local seat.
- **`Admit::spawn`** holds the seat bytes an added seat spawns from. The shell sets it in
  `Online::lobby` (this machine's first seat's start), in `Online::open` and in `drop_in`, and again on
  a host handover.
- **Online.** `frame_seats(&[Report])` gives each local seat its own pad, and `frame` is the
  one-pad wrapper. `drawn()` now lists this machine's seats first, then the rest. `can_sit()` is true
  only on a lobby ride, once seated, and not while ending or leaving.
- **Shell (main.rs).** Online, `seats_open` follows `can_sit`, so a Cross on a free pad becomes
  `Act::Join(d)` → `Online::sit(preset of d)`. The P76 local `Change` is not applied online. Instead,
  when `my_seats()` changes length after the restart, the shell plays the same P76 re-layout one seat
  at a time: idle pads, view triangles, cameras, Start readers, `pad_of`, `players`, `reseated`, the
  layout and the display. Each local seat gets its own pad, pause and scheme in the online pad build.
- **Bug fix in `rollback::confirmed`** (from N10/N11): when ggrs's confirmed frame equalled the last
  tick run, the cell for it was not saved yet, so `confirmed` returned None forever. A leave or sit
  asked then never got its restart tick ("the host did not answer", about 1 run in 4 in the lobby
  test). It is now clamped to the last tick run.
- I checked opentennis for prior art on mid-match local joins and found none to take.

## What the original does

Nothing to compare: the original has no online play. The local drop-in being matched is P76.

## How it was proved

- `tests/netplay.rs::a_machine_adds_a_local_seat_mid_ride_and_drops_it_again`: the simulated lossy
  link. A guest adds a seat, the host adds a seat, then the guest drops its added seat. Lineup
  `host,g,-,host`, 3 restarts, presets 2 and 1 on the new sims, each machine's ticks never step back
  more than WINDOW, all machines agree from each change on, and the added seats ride their own pads.
- `tests/netplay.rs::a_seat_that_cannot_be_added_says_why`: no spawn, bad preset, not seated yet.
- `tests/lobby.rs::a_second_player_on_an_online_machine_sits_down_and_leaves_again`: two real LAN
  lobby `Online`s. The guest's second player sits (seats [1,2], drawn first, preset 2, rides 30+
  ticks), then Leave Session for local seat 1 takes it out again. Hashes agree, no desyncs. It passed
  5/5 after the rollback fix.

## Not verified / not 1:1

- **The shell path in main.rs was never run end to end with two live pads.** There is no second
  real pad here, and the shell skips uinput pads. Only the compile and the session/Online tests above
  cover it.
- **The added seat's look is not re-dressed.** The rig at that shell index keeps whatever look it
  had.
- **Bike constants and modifiers.** The added seat rides the first seat's bike constants
  (`Game::seat` clones the first sim). In a lobby ride with per-seat modifier rows, the new seat's
  modifiers row is not checked.
- **No engine sound join/leave online.** `seat_joined`/`seat_left` are not called online.
- **One idle frame.** The views re-lay out on the frame the restart lands, so the new seat reads an
  idle pad for that frame.
- **`--peer` (UDP) rides cannot add seats.** `can_sit` is false there, because the lineup and presets
  are fixed by the command line.
- **The lobby room does not list added seats.** Their `seat_pick` is None, so they ride in the
  default pick.
- **A refused guest sit gets no reply.** The host only logs it, and the guest's "Asking for a seat..."
  note stays until something replaces it.
- **Two sits in one restart** get their pad devices in the order they were asked.
- **The `confirmed` clamp** settles on the last tick run, which may still be built on predictions.
  That is the same as other confirmed-frame cells, and all the netplay and lobby tests pass with it.

## Next

Land it.
