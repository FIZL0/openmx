# 1 — machines everywhere (FINAL)

Task P113. 2026-10-22.

## What I did

- A track's `machine.txt` now only decides where copies are *parked*, never whether a player has the
  machine. `Lot::plane_anywhere` (crates/openmx/src/vehicle.rs) gives a player their parked copy
  where the file (or P87's "beside it" rule) has one, and otherwise a copy of the disc's plane
  (`machine.res`'s `biplane.phy`, not the track's files) with no bay drawn anywhere. Only a disc
  without `biplane.phy` has no plane.
- Where that copy starts, for a seat that starts the ride in it (`--vehicle biplane`, the setup's
  pick, an online seat — N18's `start_planes`, N19's joiners): `vehicle::start_spots`, the start line
  taken as if it were the file's one spot, run through the same rule `park` uses for copies past the
  file's (`park` was split into `park` + `park_around`; it behaves as before), each on its wheels on
  the ride's own ground. main.rs measures these once the ride's ground (`Land`, so a mod map's or a
  tiled world's) exists and re-stands seat 0's plane with `Plane::stand_at`; seats 1+ take theirs
  directly. `online_bays` falls back to the same spots when the file parks no plane, so an online
  seat starting in the plane has a bay on any map.
- A switch mid-ride was already P102's spawn-on-the-player, so with a plane in every garage the
  pause's Switch To Machine, the M key, the bays' pop-up (only where bays exist) and a tape's switch
  all work on any map; online the switch takes its template from seat 0's garage plane, which now
  exists everywhere.
- The pause's switch row reads `garage.machines().len() > 1` instead of "has a plane", so each later
  machine joins it by appearing in `Garage::machines`.
- The plane's camera presets (`biplanecpresets.ini`) and particle list (`biplaneparticles.dat`) live
  in the freeride tracks' second archives. On a track without them (nat19 found 0 presets and no
  particles) they are read from `free06_2.res` (`PLANE_KIT_ARCHIVE`); the particle sheet stays the
  track's own. `load_plane_fx` took a `kit: Option<&Res>` for it.
- Errors and help text no longer say "this track's machine.txt parks no biplane".
- opentennis has no vehicles; nothing taken from it.

## What the original does

Nothing to port: the original's machines exist only on the five freeride tracks whose `machine.txt`
parks them (R8, P87), and its race tracks have no machine at all. Offering every machine on every
map is the port's own addition (user, 2026-10-22). With nobody asking for the plane, a ride is
exactly what it was: seat 0's unparked plane is never stepped while the bike is up, and the bike's
`ride hash` path is untouched.

## How it was proved

`crates/openmx/tests/machines_everywhere.rs` (disc tests skip without the disc):

- `every_seat_switches_to_the_plane_and_back_on_a_race_track_and_a_bare_map`: four seats on nat19's
  real terrain (its machines' archive has no `tracks\nat19\machine.txt`, asserted) and on a floor
  with a `machine.txt` that parks only monster trucks, each seat stood somewhere of its own. Each
  opens its own pause, the row offers "Biplane", Cross switches it: its plane is at its own bike's
  x/z, on its wheels over the ground there, in its copy's livery, the later seats untouched. Each
  plane idles a second on the ground without sinking or dropping; then each seat switches back from
  its pause and its bike is under its plane.
- `a_ride_started_in_the_plane_puts_each_seat_beside_the_start`: for 1, 2, 4, 8 seats on nat19, the
  first spot is on the start line, all on their wheels, level, at least the plane's spacing apart,
  and a garage starting in the plane there works; no flight and no bay is the only refusal.
- `parked_copies_stay_where_the_file_puts_them`: a file's copies come out of the lot exactly as P87
  parks them, whatever start is passed.
- `the_same_switch_online_on_a_race_track`: four online lanes (`switch_lane` / `fly_lane`) on
  nat19's ground take the plane at their own spots, fly a second, and stand their bikes back under it.
- `the_shell_flies_the_plane_on_a_race_track`: the binary, headless, `--track nat19 --vehicle
  biplane --players 4` rides in the plane with 5 camera presets and particles; a tape's switch to the
  plane and back on nat19 prints bike → biplane → bike.
- `two_online_machines_start_their_seats_in_the_plane_on_a_race_track`: two headless `--online`
  shells over 127.0.0.1 on nat19 with `--vehicle biplane`: both print "2 seats start in the biplane",
  0 desyncs, and the same state before tick 300.
- `tests/track_mods.rs::a_mod_map_has_the_biplane_with_no_machine_txt`: the generated `md2_flat` mod
  map, `--vehicle biplane --players 4` starts in the plane on the flat's ground; a tape's switch
  there goes bike → biplane → bike.

## Not verified / not 1:1

- **The whole feature is the port's own** (the original has no machines off its five freeride
  tracks), so there is nothing to compare against the original; the start spots beside the start
  line and their 40 ft spacing are the port's.
- **The pause's switch for seats 1–3 is proved at the library level** (menu.rs + vehicle.rs driven
  as main.rs drives them), not through the binary: headless, only seat 0 can be driven (a tape's
  switches are seat 0's, and `--online` refuses `--players`/`--play`, so online pads are idle). The
  binary tests prove seat 0's switch and every seat starting in the plane.
- **A mid-ride switch online through the binary was not run**, for the same reason (idle pads). The
  online switch is `switch_lane` on the race track's ground (library test) plus N17's map-independent
  rollback test; the online start in the plane on a race track did run through two binaries.
- **A plane landed on a slope stands level** (P102's landing, unchanged) and may roll a little at
  idle: on nat19 one seat landed on a 45° jump face and rolled 3 ft in a second. The test bounds it
  rather than asserting rest.
- **The plane's camera presets and particle list on a non-freeride map are free06's**
  (`free06_2.res`); the particle sheet is the track's own. A mod map that ships none of these gets
  free06's presets and its own (if any) sheet: on md2_flat this was not looked at beyond "starts".
- **Tiled worlds (MD8) and MX vs. ATV map mods** were not ridden in the plane; they go through the
  same `Land` ground query the start spots use, which the md2 mod map exercises.
- **The bays' pop-up (P103)** stays where bays exist: a map with no parked copy offers nothing by
  proximity (there is nothing standing there to ride up to); the pause and the key are the way in.

## Next

Nothing for this task. Each later machine (helicopter, buggy, monster truck, baja) joins by having a
`Kind`, a template like the plane's and an entry in `Garage::machines`; `start_spots` already takes
the machine.
