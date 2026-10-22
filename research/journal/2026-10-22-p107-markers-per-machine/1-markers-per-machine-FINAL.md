# 1 — markers per machine (FINAL)

Task P107. 2026-10-22.

## What I did

Every machine now has its own session markers: five slots and a selection each, never shared.

- **game-sim `markers.rs`.** A slot holds a `Mark`: `Bike(Ride)` or `Plane(Flight)`. `Markers::slots`/`at`
  are always the set of the machine that is up (`Markers::on`); every other machine's set waits in
  `Markers::away` until `Markers::up(machine)` swaps it back. I chose the swap so that everything that
  already reads `markers.slots` (the slot display, P92's radar marks and posts, V51's portals on
  task/V51, which call only `.position()`/`.axes()`, both kept on `Mark`) shows the up machine's set
  and nothing else, unchanged. The switch calls `up` (`Garage::switch`, and through it
  `switch_lane`). So does each machine's tick (`frame` → bike, `fly` → plane), so the set can never
  lag the machine.
  - `markers::fly` is the plane's tick. It reads the d-pad through the shipped table with no trick
    gate, since the plane has no tricks. Down keeps `flight.clone()` unless the plane is wrecked. Up
    on a slot calls `Flight::put_back`. Up on a player answers `Teleported` and the caller lands the
    plane, because the ground and the plane's wheels are the garage's.
  - The press test is one function (`pressed`) for both machines, so P106's hold changes one place.
- **`Flight::put_back`** (flight.rs) restores the whole kept flight (body, attitude, speed, spin,
  throttle and surfaces, engine, wheels, crash state, rng, the collision search), so a wreck is left
  behind. It keeps the current flight's modifier-tuned numbers (`retune`'s four, plus the gravity off
  them) and its ray probe and bike mesh. With the tune unchanged it is the kept flight bit for bit.
- **Hash and snapshot.** A bike's `Mark` walks as its `Ride` did. The `on`/`away` fields are walked
  only once a machine other than the bike has been up or has markers, so a bike-only session hashes
  as before. The snapshot's markers section gained `on` and `away` behind the same tag; a plane mark's
  probe and mesh are put back from this machine's plane on decode, as the plane's own are.
- **Files** (saved_markers.rs). The bike's file stays `markers/<track>/<profile>.markers`; the plane's
  is `<profile>.biplane.markers`, named by the plane's `.phy` (`markers::PLANE`, checked equal to
  `bike_word(BIPLANE_PHY)`).
  - The format is v2: the name is followed by a machine byte. P99's v1 still reads, as the bike's.
  - `load` reads both. `Kept::frame` writes only the machine whose slots changed (per-machine
    `signatures`); `signature` stays one word, P99's for bike-only markers.
  - Tapes, lobby picks and `--peer` wires carry `Markers::to_files`. That is the bike's plain file
    while the plane has none (byte-identical to before), otherwise a small bundle.
    `read_file`/`from_files` read either.
- **The plane's d-pad is freed** (vehicle.rs `without_dpad`). `Plane::step` flies on the pad with the
  four directions let go, under every scheme, before `flight::pad` and `round_stick` (whose own
  semantics are unchanged). The left stick alone banks and climbs, and on the keyboard that is the
  stick keys. `PlaneScheme::fly` (P50's screen scheme) no longer swaps Up/Down upside down. That swap
  only existed for the d-pad's dive/climb, and kept it would have turned the markers' Down into Up
  whenever the plane was inverted. L1/R1 and the stick are still swapped.
- **Shell.**
  - `Seat::pass` runs `Garage::markers` with the plane up. An Up onto a player lands the plane where
    they are, through `land`, which is extracted from P102's switch onto the plane. A slot's put-back
    re-poses the plane.
  - At ride start each seat's set is switched to the machine it starts on.
  - The world draws the up machine's markers only: faint bikes for the bike, faint planes in the
    seat's livery for the plane.
  - Online, `net/rides.rs` runs `vehicle::lane_markers` after `fly_lane`. `vehicle::lane_spot` makes
    a flying seat's spot the ground under its plane (offline P104's `Garage::spot`), not the bike it
    left.
- **Tests.**
  - `crates/openmx/tests/markers.rs` gained four: `a_plane_marker_restores_the_flight_bit_for_bit`,
    `bike_and_plane_slots_do_not_share_and_a_switch_shows_the_other_set`,
    `saved_and_reloaded_per_machine` and `online_a_planes_markers_ride_the_lane_and_the_snapshot`.
  - The existing marker tests take a slot's bike through `Mark::ride`.
  - `ride.rs`'s held-diagonal test now asserts the arrows fly nothing (hash-for-hash the gas alone),
    where it used to fly them.
  - pad.rs's P50 test no longer expects Up/Down swapped.
  - game-sim's `hash.rs` size pin moves to 11280 for `on` and `away` (the fresh leaf count is unchanged).

## What the original does

Nothing: session markers are the port's own (P88). The plane's d-pad was the original's digital stick
(dive, climb, bank). The port now gives that d-pad to the markers, as P93 did the bike's.

## How it was proved

- `a_plane_marker_restores_the_flight_bit_for_bit`, on the disc's `biplane.phy` over a flat floor,
  through the same calls `Seat::pass` makes:
  - The plane flies 30 s of gas, then Down places the flight.
  - The marker equals the flight (`PartialEq` and `hash::of`).
  - The plane flies away, dives 90 ticks and is wrecked; Up puts it back equal to the kept flight,
    unwrecked, in the air.
  - The next 40 ticks (a scripted pull) hash tick for tick as the 40 after the placement did.
  - The same 40 ticks with Left+Up held hash the same, so the d-pad flies nothing.
  - The run printed: `plane marker: kept at Vf([399.89865, 469.34784, -3132.382, 1.0]) ... flown 40 ticks bit for bit`.
- The separate-sets test: a bike marker in slot 1 with the selection on 2, then a switch to the
  plane.
  - On the plane the slots are empty on the first slot with no radar mark, and Up on slot 1 is
    refused.
  - The plane places in slot 3. Back on the bike, the bike's set and selection are as left, and the
    plane's is kept aside.
  - Up rides the bike's slot 1 and is refused on slot 3. Back on the plane, its selection is still
    on 3.
- The saved test checks:
  - which file each placement writes, and that the bike's bytes are untouched by a plane placement;
  - that a reload puts each set in its own machine;
  - that another bike's file is dropped while the plane's still loads;
  - round trips through tape, pick and wire;
  - that cut bundles fail;
  - that a P99 v1 file reads as the bike's;
  - that a clear removes both files.
- The online test: a lane flight (`start_lane`, `fly_lane`, `lane_markers`) places and puts back bit
  for bit. A snapshot carrying the plane set plus a waiting bike set decodes to the same hash.
- `./kit check` passes: 1481 tests.

## Not verified / not 1:1

- **"Ghosted" on the plane's put-back has nothing to act on.** The plane has no ghost: the shell's
  plane never collides with other riders (`Plane::step` frames with no bikes), so it cannot be landed
  "into" anyone. Only the bike's teleport ghosts.
- **The wreck in the restore test is set by hand** (`crash.reason = 1`). The stand-in plane has no ray
  probe (the disc's collision object is not loaded there), so the flat floor never wrecks it. The
  put-back replaces the whole flight, so any wreck state is left behind. A real wreck on a track was
  not flown.
- **The put-back keeps the plane's current modifier tuning** (power, prop speed, drag, gravity scale).
  A marker kept before a modifier change is therefore not bit for bit the kept flight. That is my
  decision, matching how P98's retune treats a flight going on.
- **Nothing was looked at on screen.** The faint planes at plane markers, the slot display on the
  plane, the radar on the plane and a real pad's d-pad in the window were not checked. The draw path
  is the bay planes' see-through path with `MARKER_ALPHA`.
- **The online Up onto a flying player now lands under their plane** (`lane_spot`), also for a rider
  on the bike. Before, it went to the bike they left. No online test covers that pick; the offline
  P104 path it mirrors is tested.
- **P106 (the Up/Down hold) and V51 (portals) had not landed** when this was built: P106 is parked as
  wip/P106 and V51 sits on task/V51. P106's redo has to apply its hold in `markers::pressed`, which
  both machines share. V51's diff reads slots only through `.position()`/`.axes()`, which `Mark`
  keeps; it was not merged and built here.
- **Saved-markers files are version 2.** An older build drops a new file with a reason, rather than
  reading it.
- **A pick or wire whose bike word does not match this ride drops the plane's markers as well**
  (`from_files` is all or nothing, as P99's was for the one file). The disk loader is per file.
- **The screen scheme's Up/Down swap is gone.** Under P50's scheme upside down, the d-pad was the
  dive/climb turned round. It no longer flies, so nothing of the scheme is lost, but the scheme's pad
  test changed with it.

## Next

Nothing filed. If P106 is redone, its hold goes in `markers::pressed`.
