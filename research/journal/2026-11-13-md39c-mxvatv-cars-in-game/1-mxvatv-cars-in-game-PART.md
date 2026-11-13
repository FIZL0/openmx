# 1 — mxvatv cars in game (PART, blocked)

Task MD39c. 2026-11-13.

## What I did

- Read what MD39c stands on, to see whether the cars can go into the shell the way MD37d put in the quad
  (`vehicle::Kind::Atv`, `Garage::switch_car`, `quad::Ride::spawn`/`step`):
  - `game_sim::car::Car::read_mxvatv` (MD39b) reads the five files into **MX Unleashed's** car struct
    (`car::Car`), with MXVehicle's `[Wrecks]`/`[Camera]` keys. That struct is the one MX Unleashed's
    car step drives. SLUS-21104's step is a separate port, `game_sim::atv_car`.
  - `atv_car` (MD106, MD124 to MD127) has the listeners, post-process, update and frame. Every piece is
    proved only under `tests/mxvatv_car_frame` and `tests/mxvatv_car_replay.rs`, on cars **seeded from
    MD39a/C29's RAM images**. Nothing builds an `atv_car::Car`, `Body`, `Vehicle` or the keys from a
    `.phy` and the chassis files. MD37d's quad needed MD122 for exactly this.
  - The frame still hands out calls with no Rust answer: the update's hand-outs (MD106e) and what MD106b's
    ported calls hand out (MD128: map-edge throw, respawn, body reset, spatial and collision sync). The
    port has never replayed a drive by itself either (MD135). The only replay that exists runs
    SLUS-21104's frame in the interpreter. There is no `atv_car` Ride that can be stepped on a
    `sim_world::World`. MD37d's quad needed MD123 for that.
- So the brief's test ("on, each drives its MD39b replay") cannot be met. Building the shell wiring on
  MX Unleashed's car step instead would break the brief's "SLUS-21104's step". Shipping cars that cannot
  move would be a placeholder. I did neither.
- Filed the two missing pieces as MD39c's splits. Ids ending in a letter cannot take sub-ids
  (K44), so they are filed under plain MD:
  - **MD148**: the car built from its files, with no RAM image, word for word against the five spawn
    images (the car twin of MD122).
  - **MD149** (after MD148, MD135): the car stepped on MX Unleashed's worlds with no interpreter, as an
    `atv_car` Ride; a headless free09 ride per car, with MD135's replays passing through the same type
    (the car twin of MD123).
- Blocked MD39c with `--after MD148,MD149,MD135`. No code changed.

## What the original does

Unchanged from MD39's and MD39b's journals: all five cars share MXVehicleCar's step through the buggy's
listener thunks. The golf cart's and truck's classes override only the loader's tail, drawing and the
update's head. Addresses are in `research/oracle.txt` `# MD39`, `# MD39b`, `# MD106*`.

## How it was proved

Nothing was proved in this session. No code changed and no test was run. `./kit done --blocked` runs the
gate on a journal-only change.

## Not verified / not 1:1

- **The whole task**: the cars are not in the game. The reasons are under What I did. MD148, MD149 and
  MD135 unblock it.
- **That nothing else is missing**: after MD148 and MD149, the shell side may still need a car-specific
  piece that I have not checked. The model drawing, cameras (`[Camera]`'s file) and sound would be
  follow-ups, as they were for the quad (MD136), not part of the physics.

## Next

When MD148, MD149 and MD135 have landed: follow MD37d part 2's shape. Add a `vehicle::Kind` detour for
the five cars, plus a Garage row or flag (off by default, written to settings only when set) that picks
one of the pack's car files. Then add the pause's switch row, and a test in `crates/openmx/tests` that has
(a) off: no car line and an unchanged ride hash, and (b) on: each car's MD39b drive tape fed through the
shell's ride, matching MD135's rows. First check that an installed MX vs. ATV pack ships the five car
`.phy` files (openmx-mods import-atv); if not, file that against openmx-mods.
