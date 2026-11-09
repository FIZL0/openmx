# 1 — p2 own fliers (FINAL)

Task P315b. 2026-11-08.

## What I did

Each seat now flies and wears its own Fliers-tab picks instead of every seat riding 1P's.

- `main.rs` `ride_fliers` returns one `vehicle::Fliers` per livery (`LIVERIES` = 2: seat 1's and
  seat 2's) off `seat_flier_picks` (seat 1 `args.flier_picks`, seat n `args.others[n-2].fliers`, a seat
  with no pick of its own rides 1P's as its bike does). `--fliers mxvatv --flier-skin N` sets every
  seat's pick; `--fliers original`, online and a missing mods folder give MX Unleashed's to every seat.
- `vehicle::tell_apart`: a second seat on the very skin the first picked wears the other skin of the
  same model (a pack's `PackFlier::second`, so it still flies the same figures) or the disc's other
  livery, as the original parks its second player's machine in the alternate livery. That keeps the
  old defaults byte for byte: no picks → P1 `biplane`, P2 `biplane2`; `--fliers mxvatv --flier-skin 4`
  → P1 biplane4, P2 biplane3, as `PackFlier::model(livery)` did before (now gone).
- In `build`: the plane's `.phy`, `mxvatv` model flag, ray probe, hull, spawn lift (`Lot::lift` is now
  one a livery) and stock file for the modifiers (`stock_plane` one a livery; `remodify` reads the
  seat's livery) are per livery; `flight_for(seat)` builds off `livery(seat)`'s. `Lot::kiowa` is a
  `Vec<KiowaRide>` one a livery (`Lot::kiowa_of`, used by `heli_for` and `Garage::modify`). The drawn
  plane and Kiowa lists load each livery's model from its own pack (or the disc), pilot seat per
  model as P316 has it.
- Ride lines: `fliers: P<n> <kind> <name> (<skin>), from <pack>, model <m>`; `biplane:`/`kiowa:` drawn
  lines say `P1 `/`P2 ` on a two-seat ride (one-seat lines unchanged, so the MD80e tests stand), and
  `biplane: P2 flies on … as model one/two` where P2's plane file differs from P1's.
- Online stays MX Unleashed's on every seat (P318), reading `stock_plane[0]`.

Nothing to take from opentennis: it has no machines/fliers.

## What the original does

The original has two players at most, and parks player 2's machine in the alternate livery (skin 1);
it has no per-player flier picks at all. Per-seat picks are the port's (front end, ours to design).

## How it was proved

`crates/openmx/tests/mxvatv_fliers.rs` `p1_and_p2_fly_their_own_picks`: two headless two-seat rides
on free09 with `--garage` reading a scratch `garage.txt` (beside a symlink to the disc, which is
where the shell keeps its files when given a disc). Ride 1: P1 pack biplane3 (model 2) + disc kiowa2,
P2 pack biplane (model 1) + pack kiowa3 — asserts each seat's `fliers:` line, the drawn lines per
seat, and P2's model-one `.phy` line. Ride 2: both on biplane4 → P2 drawn in biplane3. Run by hand
too, output read (not a skip). Unit test `mx_unleasheds_second_seat_on_the_first_seats_livery_wears_the_other`.

## Not verified / not 1:1

- P2's flight on its own `.phy` is shown by the ride line and by `flight_for` reading the seat's
  livery, but no test flies P2's plane and hashes it (the `plane hash` line is P1's; no two-pad tape
  was written). P1's model one vs two flying differently is MD80e's test.
- Seats past two (3, 4) fly and wear the picks of the seat whose livery they wear (seat 3 = P1's,
  seat 4 = P2's), not their own: the drawn lists and the lot's files are per livery, and making them
  per seat means splitting `livery` from skin across the draw paths. Filed as P315h.
- Particles: the plane's and Kiowa's emitter binds are resolved once on the first livery's drawn
  model; a P2 on a different model (one vs two) emits from node names looked up on P1's model.
  Presentation only, not checked on screen.
- Two packs whose skins share a model name (e.g. a renamed copy of the fliers pack keeping
  `biplane3`) would collide in `assets`, which keys loaded models by name; not tried.
- Switch To Machine mid-ride still uses what the ride built at start (P315g).
- No menu shot or on-screen look at two seats' planes; checked by ride lines only.

## Next

Done; P315h holds seats past two.
