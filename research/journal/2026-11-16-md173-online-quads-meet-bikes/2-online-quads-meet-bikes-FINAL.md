# 2 — online quads meet the bikes (FINAL)

Task MD173. 2026-11-17. Reopened after MD179 landed the quad's hull, pushed body and mirrors.

## What I did

- `net/rides.rs`'s tick puts a seat's quad (`Sim::quad`) in the contact pass after the cars, as
  MD179's split-screen pass does:
  - its hull (`quad::Ride::hull`) with the mesh from the seat's own copy of the quad
    (`Rides::quads[seat].hull`), for seats that are riding and not paused;
  - its body chained into each bike's `Bump::planes` (`quad::Ride::flown`), and written back after
    the bike's step (`quad::Ride::pushed`, picked by `Flown::quad`);
  - the mirrors of the bikes' strikes as its `met` (`rider_contact::met_among`), stepped with
    `vehicle::ride_lane_met` (new; `ride_lane` now calls it with `None`).
- `quad.rs`: the held list's `bikes` (MD179's per-frame chassis of the bikes it met) is cleared at
  the end of the frame. It is off the snapshot wire but was in the lane's hash, so a state read back
  (resync, join) hashed apart from the machine it came from. Nothing reads it after the frame.
- Test `online_car::an_online_bike_is_stopped_by_the_other_seats_quad`: two machines, g's seat on
  the Race 250 P 25 ft ahead, the host's bike shoved at 30 ft/s at tick 60. With the hull the bike
  ends at z 56.9 behind the quad, pushed 38.5 ft (up to 27 ft/s); with no hull the bike rolls to
  z 65.0 and the quad stays put. Both machines agree every tick. Every tick that has a bike's
  mirror on the quad's list, the state goes through `Rides::encode`/`decode` and must hash the same.
  With the `bikes` clear taken out, that check fails at tick 81 (tried).
- ../opentennis: nothing to take (no vehicle contact there).

## Not verified / not 1:1

- **What the original does when a quad meets a bike** is not proved: the bikes are Untamed's, the
  quad MX vs. ATV's, and they never meet in either game. Same as MD179's notes, which still hold
  online (hull category, the ATV `Weighed` resolve of the mirrors, no wreck from a bike's strike,
  `Held.body` not written back by a push).
- **A quad meeting a flying plane, or a car, online** is not wired: only bikes strike the quad's
  hull, as in split screen. Planes' and cars' own rays are not searched against it.
- **No two-shell (real UDP) run** was made with a quad struck; the test drives the session over the
  in-memory wire, as the car's F23 test does.
