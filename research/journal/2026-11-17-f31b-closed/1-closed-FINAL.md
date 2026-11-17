# 1 — closed (FINAL)

Task F31b. 2026-11-17.

## What I did

- Checked each piece of F31b's brief against the task that did it (all ticked on main):
  - the frame driver (the sim object's vt `+0x40`) and the vehicle frames: F31b1 (PreProcess), F31b2
    (Force body), F31b3 (PostProcess), F31b4 (the collision record's pass), F31b4a (the tail);
  - MD63e's mass properties: F31b4c (`handling::mass`, state 4's 134 mass words word for word);
  - the wreck and the put-back to the `+0xd38` spot: F31b4b (`handling::wreck`);
  - the whole frame with no interpreter: F31b5 and its follow-ups F31b5a..v, F67.
- Ran `tests/mxvatv_bike_ride.rs` whole (result below).
- No code changed. Took nothing from `../opentennis`: nothing left to port.

## What the original does

Nothing new. The frame's parts are in the F31b1..F31b5 entries.

## How it was proved

- Bit-exact replay, `crates/game-sim/tests/mxvatv_bike_ride.rs` — all 147 tests pass (`./kit check -- -p game-sim --test mxvatv_bike_ride`), among them:
  - `ride_replays_in_pure_rust`: C15 ride-2, 459 rows, no wreck, interpreter total 0;
  - `flip_replays_in_pure_rust`: C15 flip-1, 465 rows, wreck state changes on rows 125 and 193 only,
    interpreter total 0.
  - F31b5's journal (`../2026-11-17-f31b5-whole-frame-closed/`) showed these are not skips (a forced
    `assert_eq!(total, 999)` failed with `left: 0`).
- The oracles of each part: in the F31b1..F31b4c entries (each ≥ 10 000 generated inputs).
- Original rides keep every replay hash: `./kit check --all` inside `./kit done`.

## Not verified / not 1:1

- **The pure-Rust frame runs in the test, over state 4's RAM image held by `Ee`, not on the game's
  `Ride`.** A `Handling::MxVsAtv` ride in the app still runs MX Unleashed's `Ride` with
  `handling::step`'s clauses swapped in; it is not shown to replay C15. Wiring the port into the ride
  is F31's (its brief: seed a ride on `Handling::MxVsAtv` and feed the tape), which waits on F31b and
  reopens when it lands.
- **MD63f's wreck gravity never runs on C15:** its two virtuals are in a vtable no state-4 object uses
  (F31b4b's journal). Not exercised by either replay.
- **Only C15 ride-2 and flip-1.** Other tracks, bikes and rider counts are not replayed.
- The leftovers F31b5's entry lists (noise gradient tables kept from the state, rumble not compared,
  dropped listeners shown harmless only by the read watch, unported paths these rides never take,
  the five riders' check watching only the bike's words) still stand; each is in its own journal.

## Next

Nothing; `./kit done` closes F31b, and F31 reopens.
