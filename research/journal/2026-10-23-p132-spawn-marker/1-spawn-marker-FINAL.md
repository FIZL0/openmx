# 1 — spawn marker (FINAL)

Task P132. 2026-10-23.

## What I did

- Marker slot 1 now defaults to each machine's start on this map. Holding Up on slot 1 with nothing
  put there takes the machine back to where it started, at rest. Down puts a real marker over it. Clear
  Markers, or a cleared slot, brings the start back.
- Where the start is kept: in `markers::Home`, a constant of the ride held on `Sim::home: Arc<Home>`
  the way the bike's numbers and tables are. It holds every seat's bike start (a `Spot`) and plane bay
  (at, facing). It is not part of `Markers`, so `Markers::fresh`, the hash and the snapshot bytes are
  what they were. `snapshot::decode` keeps the machine's own `home`, as it keeps `phy`. Each peer builds
  it from the map (`vehicle::home` in `main.rs`'s ride, `Online::bays` online), so no peer sends one or
  stores one of its own. Why: the brief says "derive it from the map, never store it per peer". A
  constant outside the state can't disagree between peers, and old tapes and old netplay bytes stay
  valid.
- The bike: `rider.teleport_to(Spot::still(start, heading, up))`, which places the bike the same way the
  ride's start does (`Ride::place_on`). The test shows this is the fresh ride bit for bit: position,
  quat, and zero velocity and spin. The bike is ghosted like any teleport.
- The plane: `flight.reset(bay.at, bay.facing, acos)`, the same call an online seat that starts in the
  plane gets (`vehicle::start_lane`). After that, `Garage::markers`' existing Teleported branch moves
  `plane.spawn` and the pose. This works offline and online (`lane_markers`).
- Seats: offline, each seat's entry is its own start and its own lot bay. Online, every seat starts
  on the first seat's bike start (as N5 already did) and in its own `online_bays` bay. `Home::bike`
  falls back to the first entry, and `Home::online` builds the online version.
- The bar: an unfilled slot 1 that has a start gets a small inner square. It is drawn neither solid
  (placed) nor as a bare outline (empty), and its label is "marker 1 spawn". The ride HUD panel shows an
  `S` for its number and the label "Marker 1 / spawn". A slot 1 with a marker placed reads "set" as
  before.

## What the original does

Nothing: the original has no markers. This is the port's own feature (P99/P107/P112).

## How it was proved

- `crates/openmx/tests/markers.rs`, five new tests:
  - `slot_one_is_the_bikes_spawn_until_a_marker_is_put_there`: ride away at speed, then Up stands the
    bike back at the fresh ride's spot, bit for bit and at rest. Slot 2 is not a start. Down then Up
    goes to the placed marker, and Clear Markers brings the start back. A ride with no `Home` refuses
    as it did before.
  - `the_spawn_is_derived_never_sent`: the hash and the encoded bytes are the same with and without
    `home`, and decode keeps the local one.
  - `each_seat_has_its_own_spawn`
  - `the_planes_slot_one_is_its_bay`: disc biplane, flown 30 s, back in the bay at rest. The online
    lane does the same.
  - `the_bar_sets_the_spawn_apart`
- `track_mods.rs::a_mod_maps_slot_one_is_its_start` (md2_flat) and
  `tiled_world.rs::a_worlds_slot_one_is_its_start` (md8_world): headless `--play` of 150 frames on the
  throttle, then 60 frames of Up. The bike ends 0.0 ft (across the ground) from the `start:` line,
  where the throttle alone takes it 500+ ft away.

## Not verified / not 1:1

- The drawn spawn box in the bar and the panel was checked only by its pixels differing from the empty
  and placed boxes. Nobody has looked at it on screen, and no screenshot was taken. The look is my
  pick; the brief only asks for it to be set apart.
- The plane's start on a mod map or a world was not proven. Neither fixture has a `machine.txt`, so
  there is no bay and Up on the plane's slot 1 refuses there, as before. The plane is proven only on the
  disc biplane's hangar bay.
- The radar and the portals do not show the start, only the bar does. The brief did not ask for them.
- A tape recorded before P132 that holds Up on an empty slot 1 now teleports where it used to do
  nothing, so its replay changes. No checked-in tape does this as far as the gate shows.
- Online: every seat's bike start is the first seat's start (online seats are clones of seat 0, N5),
  not a per-seat grid spot. The plane bays are per seat. This is what a seat actually starts on online.
- Online with real peers was not run. Agreement between peers is shown by the hash and encode test and
  by two independently built `Home`s riding to the same hash. A live session was not run.
- `main.rs` works the start heading out again from `start_direction` / `--trace-at`, beside the build
  closure's own copy rather than threaded out of it. If one copy changes, the other must change with it.
- Choosing a different start on the Start On pick is covered only because the seats' own start
  positions are read after placement. No test picks a non-default start.

## Next

Nothing; lands with P132.
