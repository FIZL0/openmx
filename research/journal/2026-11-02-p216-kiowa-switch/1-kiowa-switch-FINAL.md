# 1 — the Kiowa switchable and flown (FINAL)

Task P216. 2026-11-02.

## What I did

- `vehicle.rs`:
  - `Kind::Kiowa` (`helicopter`, `kiowa`, `heli` on a tape or `--vehicle`'s parse). `Machine::Helicopter.kind()`
    is `Some(Kiowa)`, so its bays are bays now: offered by the pop-up, on the radar, drawn see-through.
  - `KiowaRide` is one seat's helicopter: `kiowa.phy` (`Phy::read_bytes`), its livery and bay, and the
    `Heli` while it is out (the flight, the `kiowa::Ride`, its own `Rng`, the touching flag).
    `Lot::kiowa` holds the template and `Lot::heli_for(copy)` gives each player's copy.
  - `KiowaRide::built` builds the helicopter as the constructor leaves it, using:
    - `[Mass Properties]`'s mass, cofg, gravity scale and gravity force, undamped (damping 1, as C14's
      start state holds it);
    - the body's inverse inertia as C14's state holds it (`0x38c24305 0x38cd5c6b 0x3a496ce0`), since
      the model's box is not read; the plane's `flight.rs` carries its own the same way;
    - the collective's filter at value 0.5, time 1, and the held height where it stands;
    - the file's wreck figures, and throw scale 0 (as the state holds it).
  - The switch handles it as it handles a car (`detour`): it spawns on the player, level, at rest, 0.25 ft
    over the ground, and goes when it is left. `Garage::drive` steps it: the stick through the player's
    range, then `Pad::from_sticks` through the ride's bindings (Cross climb, Square descend, L1/R1 rudder,
    left stick speed and bank), then `Kiowa::tick` at 115 Hz substeps.
  - `KiowaAround` is the `Surroundings`. It is the port's own:
    - one contact at the skids' middle (the model origin), a damped spring times the mass, with skid drag
      and rate damping while touching;
    - the strike is `rotor_in_ground` on the eight wreck rays' ends against the ground;
    - a wreck hides the blades, so the tail shows bent;
    - the put-back builds a fresh helicopter where it is.
- `menu.rs`: `Machine::Helicopter` is in `SWITCH`, with a per-seat `heli` flag the switch row offers it on.
  `menu_cameras.rs` frames it as the bike.
- `chase::heli`: the camera target, filled as the plane's is.
- `main.rs`:
  - `lot.kiowa` comes off `machine.res`, and every garage gets `heli_for(i)`.
  - `Rig::heli_camera` is a `Race` on `helocpresets.ini` (the track's or free06's second archive),
    starting on Aero Cam 1. `follow`/`follow_mut` and the frame's camera pick use it, and it shares the
    plane's fov-ease and look-back slots.
  - The flown Kiowa is drawn with `kiowa_draw::actors` at alpha 1, at its measure's matrix, with
    `Rotor { angle: ride.rotor_angle, blades: !rotor_hidden, wrecked: crash.reason != 0 }`. The angle is
    `spin_blades`', which the tick runs.
  - The plane bay loop draws planes only.
  - A `kiowa:` line is printed as the helicopter is left and at the end of a ride in it.

## Checked

- New `tests/kiowa_switch.rs`. It rides the binary headless on free09 with a `switch 30 helicopter` /
  `switch 200 bike` tape:
  - Cross held: it climbs 254 ft in 170 ticks (49 ft/s at the end) and drifts 1 ft, on Aero Cam 1.
  - Nothing held: it stands on its skids, at −0.3 ft and 0 ft from where it was taken.
- `tests/bays.rs`: updated for the helicopter bays: one plane and one helicopter a player; the pop-up
  offers the helicopter by its bay and not to its own pilot.
- A `--shot` at tick 150 of the climb: the Kiowa is drawn solid in the air with the main rotor's blur
  and the tail rotor, the HUD says Aero Cam 1, and the radar marks its bay.
- `./kit check -- -p openmx --no-fail-fast`: everything passes but `biplane_particles`
  `a_plane_flown_into_the_ground_puffs_where_it_hits`. That is F9, which fails on HEAD, and it mounts
  only particles.rs.

## Not done (filed)

- P216a: the real ground (C14a's `contact::resolve`) and wreck/put-back (S220a1) in place of
  `KiowaAround`'s stand-ins.
- P216b: the pilot in the seat; the tick-to-tick slide, cut and seam carry; the software renderer's
  Kiowa; the camera's framing and seat presets (P82/P163/P136), and keeping a cycled preset.
- Online: a helicopter pick is no pick (`Wanted` byte 0), as the cars' are.
- Sound and particles are V114's.
