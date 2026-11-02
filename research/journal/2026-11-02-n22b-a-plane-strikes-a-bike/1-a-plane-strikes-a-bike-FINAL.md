# 1 — a plane strikes a bike (FINAL)

Task N22b. 2026-11-02.

## What I did

- `crates/game-sim/src/flight_search.rs`: `Scene::owned(velocities)`. When the bikes' collision
  records are in the search, a hit on bike `k` is `Struck { other: Some(1 + k) }`, and the sweep reads
  the bike's chassis velocity. Without it, a struck bike is only itself, as before (S95).
- `crates/game-sim/src/flight.rs`:
  - `Met` gains `bikes` (each `Bike`'s player) and `named` (the plane as the bikes' lists call it),
    and the outputs `struck` (the mirrors for each player's list) and `pushed` (the players an own
    entry named).
  - `Flight::search` takes the `Met`. With it, the search is handed the plane's record plus one empty
    record per bike (capacity `rider_contact::CAPACITY`). `contact_search::collide` files the mirrors
    on those records, as it already does for any struck object with an other. Then the plane's
    entries are renamed from `1 + k` to the player, and the mirrors from the plane's record to
    `named`.
  - The force listener already passed `Met`'s bikes as `Side::Vehicle` others (N22a), so the
    resolve pushes them through `contact::resolve`'s vehicle side.
  - `Flight::search_alone` is the search by itself, for the test.
- `crates/game-sim/src/rider_contact.rs`:
  - `met_plane` / `met_plane_among`: `met_riders` with every other player named, in `plane_bikes`'
    order.
  - `struck_first`: the planes' mirrors, in plane order and moved to the bike's nearest copy, then
    the bike's own gather, cut to `CAPACITY`.
  - `pushed_riders` / `pushed_among`: the pushed chassis' speeds written back, as `bumped_riders`
    does.
- `crates/openmx/src/main.rs` (split) and `net/rides.rs` (online):
  - Every flown seat now gets a `Met`, not only one with mirrors.
  - After the plane's pass, the riders it pushed take their chassis back, and its `struck` is kept by
    plane.
  - Each bike's list is `struck_first` of those mirrors and its own.
- `crates/openmx/src/vehicle.rs`: `Plane::step_met`'s doc.
- `crates/game-sim/tests/plane_contact_capture.rs`:
  - New: `the_planes_own_rays_strike_the_bike_as_the_originals`.
  - Both resolve tests now take 5074 too.
  - A plane call also compares, and in the tick test writes, both riders' chassis.
- `crates/game-sim/tests/flight_bikes.rs`: new `a_plane_flown_into_named_bikes_pushes_them`.

## What the original does

5074 (oracle.txt, N22b), after every record's collide:

- Plane 0's list is two kind-0 entries naming P2's record, then a kind-1 mirror of P2's own strike.
- P2's list is the two mirrors of the plane's entries (kind 1, naming the plane), then its own
  kind-0 entry.
- A bike's frame collidable (record `+0x1f8`, category 0x9a048, S95's) has owner `+0x80` equal to its
  own record. That is why the plane's hit names it.
- In 5074 the plane's four resolve substeps leave both riders' chassis exactly as they were. P2's own
  first resolve moves its chassis, the plane's mirrors included.

## How it was proved

The tests read the s93 images, which are in s1's `context/` (copied here, git-ignored), and the disc.
All ran, and none skipped (`--nocapture`, no SKIPPED line):

- `the_planes_own_rays_strike_the_bike_as_the_originals`. The port's plane search, with plane 0's body
  off RAM and both bikes' frames on their collidables' transforms, is set against the original's
  collide of every record:
  - the plane's kind-0 entries match the original's bit for bit, every field, `other` P2 included;
  - `pushed` is `[false, true]`;
  - `struck_first` of the port's mirrors and the port's gather gives each rider's whole list as the
    original's, every field and in order.

  It also checks that each bike's frame collidable is category `BIKE` and is owned by its record.
- `the_port_resolves_every_call_with_the_plane_as_the_original` now covers 5074 as well. Every call
  in the node's update matches bit for bit: both planes' 4 substeps, then P2, then P1. That holds for
  the plane bodies, the riders' chassis after each plane call, and P2's chassis velocity, rates,
  shock, twist, force and torque after its own resolve.
- `the_tick_with_the_ports_resolve_is_the_wreck_row` now covers 5074 as well. With the port's resolve
  standing in for all 12 calls, both riders' chassis match the capture's 5075 row: centre, force,
  moment, shock, twist, velocity and rates. P2 wrecks with reason 1, and the plane bodies are the
  original's.
- `a_plane_flown_into_named_bikes_pushes_them` (integrated `frame_met`, S95's wall of bikes):
  - With the bikes named, the first touching frame's entries name a bike, its mirrors name the plane,
    and its chassis is pushed along the plane's way.
  - Unnamed, no entry names a bike and nothing moves.

Wider runs, `cargo test --no-fail-fast -p game-sim -p openmx`:

- `platforms.rs several_riders_...` and `menu.rs the_pick_lists_the_disc_worlds_...` are red. They are
  K21's reds, red on a clean HEAD as well (N22a).
- `original_mode.rs the_original_switch_...` is red only because no display is exported in this
  shell (no "graphics:" line, software fallback).
- `web.rs the_launcher_fits_...` was red under the full run and passes alone (load).
- Everything else is green: `netplay.rs`, `main.rs`'s `split_contact`, `flight*.rs`, `hash.rs`, the
  rider-contact suites.

## Not verified / not 1:1

- **The impulse onto a bike is not captured.** In 5074 the plane's entries take the spring path, and
  the bike's chassis does not move in the plane's calls. The plane's impulse into a vehicle is
  `contact::resolve`'s vehicle side, proved for rider-against-rider (S88). Here it is shown only by
  the wall test, which checks a direction, not bits.
- **No second capture.** Only one image (5074) has the plane's own rays on a bike.
- **Two planes striking one bike.** Each plane's mirrors on the bike's list are in plane order, and
  each plane's search assumes the bike's record starts empty. The cut to `CAPACITY` happens once,
  over the joined list. In the original the second plane's search sees the first's mirrors already
  there, which gives the same list unless the record fills. Not captured. The planes' interleaved
  substeps are still N22a1's.
- **S95's one-player `Sim::step`** (the riders' bikes in player 1's plane's world) still strikes
  bikes as static boxes. Its bikes have no `Met`. Whether the 1p state's bike collidables are owned
  by records was not checked. Filed as a follow-up.
- **Wrapping worlds.** The mirrors' points are moved to the bike's nearest copy, as `gather_met`
  moves its points (MD44, the port's own; the original has no wrapping world). The other fields stay
  in the plane's copy, as N22a left them.
- **The bike record's flags** (touching, on object, touching other) that a mirror sets are not kept.
  The riding bike's resolve does not read them in the port.
- **No shell-level test** flies a plane into a bike in split or online. The wiring is checked only by
  building it and by the existing split and online suites staying green.

## Next

A follow-up for the one-player `Sim::step` (above).
