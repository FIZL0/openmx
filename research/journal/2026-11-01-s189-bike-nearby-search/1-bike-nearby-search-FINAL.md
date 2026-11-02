# 1 — bike nearby search (FINAL)

Task S189. 2026-11-01.

## What I did

- `ride.rs`: `Ride::nearby: Option<Box<broad_phase::Search>>` (wired in `snapshot.rs`, walked as `nearby`
  by the hash once it is `Some`). `nearby_scene` opens a `flight_search::Scene` on it (seeding it on a
  fresh bike's first search); `wheel_hits`, `sweep_back` and `contacts` go through it rather than through a
  list of their own. `Ride::gather_nearby(at, rays, world, ghost)` refreshes it around the body rays (the
  wheel rays when there is no body).
- `flight_search.rs`: `refresh_ridden` builds the bike's query over the flown world (path cars as toys in
  the moving list, as the original's one broad phase holds them) with the body rays' pair filter
  `{exclude, [0x40]}` for a ghost, `{exclude, []}` otherwise, then `refill`s — which resorts the list in
  place while the reach stays inside the bound and gathers afresh when it leaves. `GHOST = 0x40`.
  `Scene::prop_at` for the wheel probe's material/`on` lookups.
- `rider_contact.rs`: `scenery_in` takes the kept list; `scenery` still makes its own (tests).
- `rider.rs`: the tick's body rays search `ride.nearby`; the gather runs at the end of the tick after
  `traffic::show`, so a ghost frame's hidden cars are not gathered as scenery; a seam shift drops the
  list as it drops `back_search`. Off the rider (`frame_searched` with no scenery) the ride gathers for
  itself after its substeps.
- Tests: `replay.rs` seeds the S129 braked/across rides' list from the image
  (`seed_bike_search`, asserting the port's list holds cars iff the image's does) and always attaches
  the traffic; `the_parked_bike_never_gathers_the_train` now probes the wheel rays through a solid and a
  ghost gather and asserts they strike different things, the ghost one the original's rail contacts.
  Re-pinned: Sim size 11976 (`hash.rs`), Ride size 2464 (`ride.rs`), riders' resync bytes 13607
  (`platforms.rs`); `wrap.rs`'s `uncached()` leaves `nearby` out like `back_search` (each world's
  broad-phase extent differs, so the bound does).

## What the original does

The rider object keeps one `broad_phase::Search` at +0x7bc. Once a tick, after the substeps, it is
gathered around the body rays' reach (`gather_resort`: resort while inside the bound, else gather
afresh with `least 30, extra 50`), with the pair filter of the body rays' categories — a ghost's list
excludes 0x40, so the cars, toys and other bikes are never candidates. The wheel probe and the body rays
then test only this list, a candidate only when its distance is within the probe's reach (S129's
finding, `research/journal/2026-10-27-s129-bike-meets-the-train`).

## How it was proved

- S129's braked and across replays (`cargo test --release -p game-sim --test replay`, with s2's
  `context/capture/s129*` and states copied in): braked parts at tick 2 and across at tick 148, exactly
  as before the change (`target/tmp/base.log` vs after) — now with the train in the world rather than
  left out.
- Frame 16344 of the braked capture: the ghost-gathered list gives the original's rail contacts (normals
  bit-exact, points within 0.001 ft); a solid gather strikes the wagon instead
  (`the_parked_bike_never_gathers_the_train`).
- S146's roof replay (s2's capture copied in) agrees on every tick, as before.
- `./kit check -- -p game-sim --no-fail-fast`: 919 passed; `cargo check --workspace --tests` clean.

## Not verified / not 1:1

- The non-ghost body rays' category list is taken as empty (nothing excluded); only the ghost's 0x40 was
  seen in S129's image. A solid bike's list read from a live image was not compared.
- A fresh bike's first search seeds the list with no category filter (before any end-of-tick gather);
  the original's very first gather after a spawn was not captured.
- Gathering around the body rays rather than the wheel probe was taken from S129's reading, not proved
  live; with no body (`bike_col` absent, as in the S129 replay) the port gathers around the wheel rays.
- The bike's world is now the flown one (cars as toys in the moving list), which can change the order of
  equal-distance candidates against the old scenery-only gather; no test showed a difference.
- `cars_as_toys` in `replay.rs` stands in for the traffic's first frame (which is what marks a car's prop
  as moving); a seeded replay has not run one.
- The across ride still parts at tick 148 (S186, the wagon contact) — not this task's.

## Next

Nothing; S186 carries the across contact.
