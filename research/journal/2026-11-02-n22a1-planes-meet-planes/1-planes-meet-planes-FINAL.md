# 1 — planes meet planes (FINAL)

Task N22a1. 2026-11-02.

## What I did

- `crates/game-sim/src/flight_search.rs`:
  - `PLANES` (struck id `1 << 18`), `Plane` (another plane's hull mesh, where it stands, its
    velocity), and a plane collidable of category `flight::COLLIDABLE`, numbered after the bikes.
  - `planes` threaded through the kept world's build, place and refresh; `refresh_planes` and
    `Scene::among_planes` take them. The old entry points pass none, so nothing else changes.
  - The scene names a plane hit as `PLANES + j`, gives the plane's velocity to the contact, and maps
    it to `Struck { other: first_plane + j }`.
- `crates/game-sim/src/flight.rs`:
  - `Fly` (a plane, its pad, its `Met`, its bikes) and `frames_together`. Every record is cleared first;
    each plane searches in turn with the others' hulls at their nearest copies; its own new entries
    name the other planes as its met's vehicles' count plus their rank among the planes but it, and
    the mirrors it leaves on the others name it as their lists do. Then the bikes' mirrors follow on
    each list as `frame_met` puts them. One `WORLD.step` with the `Together` listeners: each force
    resolves against the bikes and the other planes (their bodies shifted to the nearest copy and
    back); each post refills against the other planes' hulls where they stand.
  - `searches_together`, the searches alone, for the test.
- `crates/openmx/src/vehicle.rs`: `Plane::step_met` split into `Plane::ready` (placement, chord,
  the seat's scheme on the flight, the pad → `Readied`) and `Plane::settle` (scheme put back, pose,
  respawned), with `step_met` now ready + `frame_met` + settle. `Garage::ready_plane`,
  `readied_flight`, `settle_plane`; `ready_lane` / `settle_lane` for online; `fly_together` (one
  plane flies its own `frame_met`, as before; two or more go through `frames_together`).
- `crates/openmx/src/main.rs`: `Seat::pass` split into `pass_front` / `pass_back` (same steps, same
  order). With two or more flown planes, `simulate` holds each flying seat's pass at its plane's frame
  (`Held`), and after the last flying seat in the walk flies them all together (`fly_held`), then
  settles each, runs its markers and fader, and hands on what its plane met (`met_after`, the old
  per-seat tail).
- `crates/openmx/src/net/rides.rs`: the same online: aloft seats held after the switch and the poll,
  flown together before the first seat on its bike (or at the walk's end).

## What the original does

See `oracle.txt` N22a1. On node 0x0061e720 [plane0, plane1, P2, P1] a strike files kind 0 on the
striker after its own entries and the kind-1 mirror on the struck record after what it holds, so
plane1's mirrors follow plane0's own entries and plane0's mirrors lead plane1's. The planes' dynamics
step is four substeps of (plane0, 1) then (plane1, 1); each resolve pushes the other plane's body
weighed by its mass, and the second reads the first's push.

## How it was proved

`crates/game-sim/tests/plane_meets_plane.rs` (needs s93's splitscreen RAM image in `context/states/`
and MX.PSR's `machine.res`; skips without them; here all four ran and passed):

- `the_original_planes_meet_and_part`: the original's own 80 ticks from the poked image: first touch at
  tick 39 with plane0 alone striking, later ticks with both, some with plane1 alone, parted by 80.
- `the_port_finds_both_planes_lists`: on every touch tick, the port's `searches_together` off RAM's
  bodies gives both planes' lists equal to the original's every field (flags too), and the struck
  category includes VEHICLE.
- `the_port_resolves_every_plane_call_as_the_original`: every hooked resolve (4 × plane0, plane1),
  fed RAM's lists, leaves both bodies and the collider scratch bit-exact.
- `the_ticks_with_the_ports_resolves_are_the_originals`: the port's resolves stand in for all 80 ticks
  (8 calls a tick) and both bodies equal the original's every field after every tick.

The shell: `main.rs split_contact::split_planes_strike_each_other` and `netplay.rs
two_online_planes_strike_each_other` fly two planes head-on 80 ft apart: both records strike `PLANES`
and the planes are held apart (online z 30.4 vs 49.5 after 20 ticks); with seat 2 away they pass.

Suites run after the change, all green: game-sim `--lib` and `biplane flight_bikes flight_candidates
flight_collision flight hash kept_world plane_ceiling plane_contact_capture plane_crash_peak
plane_crash_pilot plane_max_g plane_meets_plane plane_world_cost replay` (replay 111 passed, 1
ignored); openmx bin unit tests (225 passed, 3 ignored) and `machines_everywhere bays bay_up
switch_trick teleport_air markers markers_online garage switch_tape markers_plane_bar
world_parked_plane netplay` (netplay 60 passed). `./kit audit`'s two leaks are in plan/DONE.md, not
this change. Not the whole `./kit check`.

## Not verified / not 1:1

- The reference is the original's code on a RAM image with a poked velocity: parked, unflown planes
  (no pad, no autopilot), not a human tape. A two-pad tape of planes meeting would be better.
- Which seat owns plane0 is not proven. The port keeps its walk's order (last flying seat first).
- The post's refill reads the other plane's hull where it stood after that plane's own post: a model,
  not checked against the original's post-step node state.
- Two planes pushing the same bike in one frame each push it through their own `Met` copy; the
  original resolves it once against both.
- The wrap's nearest-copy shifting of the other plane's body is untested on a wrapping world.
- In the shell the flying seats' pads are all read before any plane flies, and their markers run after
  all have flown: a seat's marker Up onto another flying seat sees it after its frame.
