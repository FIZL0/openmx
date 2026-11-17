# 1 — ride respawn proved resets (FINAL)

Task MD180. 2026-11-16.

## What I did

- `atv_car.rs`: the MX Unleashed Ride's respawn reset (`respawn_reset`, behind `PutBack`'s `Call::Respawn`)
  now calls `tire_reset` for each tire and `engine_reset` for the engine, instead of its own copy.
- The copy had cleared the tire vectors to `0,0,0,0` and stood the normals on `0,1,0,0`; the proved
  resets put `w` 1 (`0,0,0,1` and `0,1,0,1`), as SLUS-21104 does (MD154 found it).
- Dropped the now-unused `TIRE_CLEAR` constant. `TIRE_UP` (`w` 0) stays: a fresh `Ride`'s tire normals
  use it, which this task did not touch.
- Nothing taken from `../opentennis` (no car respawn there).

## What the original does

The car respawn's tire and engine resets, as MD154 ported and proved them (see its journal).

## How it was proved

- `tire_reset` / `engine_reset` are already proved against SLUS-21104 by MD154's
  `mxvatv_car_frame::own_back::the_respawns_tire_and_engine_resets_are_the_originals` (200 generated states).
- Rechecked the Ride tests: `mxvatv_car_mxu` (free09 jumps, wrecks and put-backs, rolled over, props,
  dropped on its side) and `mxvatv_car_frame`; then the full gate in `./kit done`.

## Not verified / not 1:1

- The Ride's respawn as a whole on an MX Unleashed world has no capture: SLUS-21104 never runs this car
  there, so only the reset words are proved, not the put-back around them (MD149's own note).
- A fresh Ride's tire normals are still `w` 0 (`TIRE_UP`); not checked against anything here, out of scope.

## Next

none
