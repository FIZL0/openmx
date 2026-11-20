# 1 — bikes strike the parked plane (FINAL)

Task S494. 2026-11-20.

## What I did

- Checked the original first, on save states of my own (R52's images are RAM only, and a strike needs
  a ride): `research/s494/` (settle.py, flier.sh, ride.py, index.md has the recipe).
- The original's bike strikes both parked fliers and pushes their bodies. No wake step: they are
  awake all along.
- The port: free10's parked plane is now a `Flight` nobody flies in `Sim::parked_planes`
  (game-sim `parked_plane.rs`).
  - Stepped every frame after the parked cars, on a pad that holds nothing.
  - Its hull goes in the bikes' list after the cars' (`bike_contacts_parked`).
  - Its body goes on each bike's resolve after the cars' (`parked_planes_flown`) and is written back
    (`parked_pushed`).
  - A lone rider with parked planes takes the `step_all` path, as with parked cars.
  - On the wire behind its own tag `S494plan`; rays, bike mesh, hull, wreck, game and skin stay this
    machine's. Hashed as one `parked_planes` blob only while present, so old hashes do not move.
- The shell (`vehicle.rs`): `parked_flier` keeps the settled plane's `Flight`; `InPlay::hand_to`
  hands shown planes to `Sim::park_planes` on a one-seat ride, kept across re-hands as the cars are.
- Filed S494a (the helicopter) and S494b (split screen).

## What the original does

- free10, bike moved 40 ft short on its own heading, gas held (`plane1.csv`): the bike stops dead on
  the hull ~13 ft short of the plane's body. The body goes from 0.02 to 1.53 ft/s
  (−0.70, 0.26, 1.35), slows over ~100 frames, and moves 1.6 ft. The rider wrecks.
- free06 (`heli1.csv`): the strike takes the helicopter to (−0.94, −0.27, 2.22) ft/s; the bike stays
  pressed on it and pushes it ~4.2 ft over 80+ frames.
- Addresses used (in ride.py): machine object `+0x160` body, `+0xe0` stored spot; body `+0x60`
  position. free10's plane machine 0x005dbeb0, free06's helicopter 0x005ff210 (as R52).

## How it was proved

- Two save-state captures on the original: `context/capture/s494/plane1.csv`, `heli1.csv`.
- `a_bike_ridden_into_a_parked_plane_stops_and_pushes_it` (game-sim tests/replay.rs): S485's free09
  ride with a plane settled at the truck's spot, nose to the bike. The bike stops 6.9 ft short of the
  body; the plane peaks at (0, 0.49, 2.05) ft/s and moves 0.96 ft in 120 ticks; it holds still before
  the strike. A resync mid-push decodes to the same hash and encodes back to the same bytes; a ride
  without the plane refuses it, and the reverse.
- openmx `tests/parked_fliers.rs`: only the plane's flight is kept. `tests/hash.rs`: Sim size note.

## Not verified / not 1:1

- **The push's size is not matched.** The port's test strikes on free09 at the plane's nose; the
  capture is free10 with the bike hitting somewhere ~13 ft short of the body (contact point not
  identified). Port peak 2.05 ft/s vs. 1.53; moved 0.96 vs. 1.6 ft. No port ride of free10 from the
  capture's state exists to line them up.
- **The helicopter is not struck.** Its stepping (`Heli`/`KiowaRide`) lives in the shell and
  game-sim's kiowa has no hull/flown/pushed. Filed S494a.
- **Split screen does not hand the plane.** Each seat steps its own Sim, so two copies would drift.
  Filed S494b.
- **The AI riders' hull of the plane does not follow it.** `Sim::parked` keeps the settled place;
  a pushed plane moves ~1 ft, so their feelers sweep it slightly off. Not checked on the original.
- **The plane's hull is placed from the last search's node**, as N22a's flown plane; whether the
  original reads its parked plane's hull at the same phase in the bike's pass is not checked.
- **The shell does not draw the parked plane** (it never did); a pushed plane is invisible.
- **The rider wrecking on the strike** is the bike's existing crash test, not compared to the
  capture's wreck frame.
- **Load-time settle unchanged** (F80's): the handed plane is the one settled at load, not R52's
  image body (0.25 ft bound, as F80).

## Next

Land it (`./kit done S494`). Then S494a, S494b.
