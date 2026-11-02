# 1 — bikes meet flown planes (FINAL)

Task N22a. 2026-11-02.

## What I did

- `crates/game-sim/src/rider_contact.rs`:
  - `Hull`, a flown plane's hull mesh as the riders' gather sees it: the seat, the mesh, the plane's
    node matrix and its record's capacity.
  - `gather_met(seats, hulls, dt, near)`, which is `gather_near` with the hulls added as collidables
    (category `flight::COLLIDABLE`, class Object) and as records. Plane k is object `seats.len() + k`.
    The bikes' body rays strike the hull through `ray_probe::pair` on a `Shape::Mesh`. It returns
    the riders' lists, plus a list of mirrors per hull (kind 1, normal negated, naming the bike).
    `gather_near` is now `gather_met` with no hulls, so nothing else changes.
  - `Bump::planes: Vec<Flown>`, the flown planes' bodies, chained after the vehicles and before the
    props in `Bump::resolve` as `Side::Body(Weighed)`, as a prop's is. The props' first index moved
    to `vehicles.len() + planes.len()`.
  - `contacts_met` / `among_met` (split / online gathers) and `met_riders` / `met_among` (the bikes
    the plane's resolve meets, each at its nearest copy).
- `crates/game-sim/src/flight.rs`:
  - `Flight::hull` (the mesh, loaded by the shell), `HULL`, `with_hull`, and `Flight::hull(seat)`.
  - `Met`, the mirrors the bikes put on the plane's list plus the bikes as `Vehicle`s.
  - `frame_met`, which is `frame_seamed` (now a wrapper over it) appending the mirrors after the
    plane's own search, capped at the record's capacity. The mirrors set touching, on-object and
    touching-other, as the plane's own object entries do, and the force listener's resolve gets the
    bikes as its others. `forces` takes `others`.
  - `flown` / `pushed`: the plane's body as a bike's resolve sees it, shifted to its nearest copy on a
    wrapping world, and back.
- `crates/openmx/src/vehicle.rs`: `Among::met`, `Plane::step_met`, `fly_lane_met`,
  `Garage::flight_flown` (a placed plane only) and `Garage::plane_pushed`.
- `crates/openmx/src/main.rs` (split) and `net/rides.rs` (online): gather with the flying seats'
  hulls, step the flying seats first (each group last seat first), give each bump the flown planes,
  hand each flying seat its mirrors, and write the pushed planes back after `bumped_*`. The shell
  loads `biplanef.col` out of `machine.res` beside the plane's other files.
- `snapshot.rs` carries `hull` like `bike` (it is shared and not hashed; `tests/hash.rs` size pin
  11992 → 12000).

## What the original does

- In the splitscreen s93 images the level's collision node (`oracle.txt`, N22a) lists the two planes'
  records ahead of the two riders' records. Its update clears every record, collides each in list
  order, then resolves them. The planes resolve inside their own dynamics step, alternating for four
  substeps, with `first = 1`. Then P2's record and then P1's record resolve, in all three images,
  including 5505, where P1 is not the striker.
- The plane's record has capacity 36, one entry for each of its ray segments. Its collidable category
  is the 0x42058 one S87 found. Its hull is `biplanef.col`.
- The bike's body rays hit the hull. That files a kind-0 entry on the bike naming the plane, and a
  kind-1 mirror on the plane naming the bike. The bike's resolve impulses both bodies, the plane's
  weighed by its mass. The plane's resolve of the kind-1 entries gives no impulse, and its springs
  write only its own side. The rider wrecks with reason 1.
- 5074 also has the plane's own rays striking P2, which is kind 0 on the plane. That is the
  plane-strikes-bike side, which is N22b.

## How it was proved

`crates/game-sim/tests/plane_contact_capture.rs`. It reads three splitscreen save-state RAM images in
`context/states/s93/` (4661, 5074, 5505, each the tick before a bike-into-plane wreck in the
`splitscreen` capture). It loads the plane and its hull from MX.PSR's `machine.res`. It skips without
them. All four tests ran here and passed, and none skipped:

- `the_original_tick_from_each_image_is_the_wreck_row`: the original's own tick, run in `ps2-oracle`
  from each image, lands on the capture's wreck row: the striker wrecks with reason 1 and the other
  rider does not.
- `the_port_finds_the_originals_contacts_with_the_plane`: the port's gather, from the image's bodies,
  gives the same entries bit for bit as the original's collide on the bike's list and on the plane's
  list. For 5074 the comparison covers only the bike's kind-0 entries and the plane's kind-1 mirrors.
- `the_port_resolves_every_call_with_the_plane_as_the_original`: a hook on the resolve catches every
  call in the node's update: each plane's four substeps, then P2, then P1. For each one, the port's
  resolve runs from the same inputs, and the bodies come out bit-exact, the plane's included.
- `the_tick_with_the_ports_resolve_is_the_wreck_row`: the original's tick with the port's resolve
  stood in (12 stand-ins) gives the same plane bodies and the same wreck as the original's own tick.

The whole `./kit check` (both crates) ran after the change. The only two reds were
`platforms.rs several_riders_saved_and_restored_hash_to_their_pinned_vectors` and `menu.rs
the_pick_lists_the_disc_worlds_after_the_five_and_rides_them`. Both are red the same way on a clean
HEAD worktree (fc1af660): the same tick-32 hash and the same world list. They are filed as K21. The
existing rider-rider, prop-knock, two-player-world, flight and hash suites pass, with `Bump::planes`
empty, the props' first index moved, and `gather_near` going through `gather_met`.

## Not verified / not 1:1

- **Plane against plane is not ported.** No capture has two planes touching. In the original, both
  planes' resolves interleave inside one dynamics step, and the port steps each `Flight` on its own.
  Split off as N22a1.
- **The plane's own rays striking a bike** (5074's kind-0 plane entries) are left to N22b. The test
  leaves them out of the comparison for that image.
- **Gather order.** The node collides P2's record before P1's. The port's gather searches the
  seats in seat order, as `gather_near` already did. In all three images every list matches, because
  the lists do not depend on the order there. A case where they would differ was not looked for.
- **The shell's step order** (flying seats first, then the bikes, last seat first within each group)
  follows the node's order: planes, then P2, then P1. It is not proved for more than two players or
  for online. The original has no online.
- **Wrapping worlds.** The plane's body is shifted to the bike's nearest copy for the bike's resolve.
  `met_*` moves each bike to the plane's nearest copy. Both are the port's own choices, as with
  rider-rider (MD44). The original has no wrapping world.
- **The bikes' collider scratch** that the plane's resolve writes on its others (springs only on the
  plane's side, so nothing from the bodies) is not written back to the bikes. The resolve test
  compares the bodies, not that scratch.
- **Parked planes** stay non-colliding (S87). Only a flown, placed plane gives a hull
  (`Garage::flight_flown`, online `plane` while touching). Whether the original files a parked
  plane's record on the node was not checked here.
- **No shell-level test** drives a bike into a flown plane in split or online. The wiring is checked
  only by building it and by the existing split and online suites passing.

## Next

None. Plane against plane is N22a1. The plane pushing a bike is N22b.
