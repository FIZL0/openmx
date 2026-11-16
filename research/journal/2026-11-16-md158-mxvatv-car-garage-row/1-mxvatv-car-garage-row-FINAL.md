# 1 — mxvatv car garage row (FINAL)

Task MD158. 2026-11-16.

## What I did

- The task named "a Car row under Machines"; since P341 the Machines tab is a grid of tiles, so it
  is a **Car tile** in the Quads & cars section, built the way MD37e's Quad tile is.
- `main.rs` `machine_offers`: a car pack (one whose `mod.json` lists `cars`) now offers each car
  openmx rides as a `PackMachine` of kind `roster::MXVATV_CAR_KIND` (`mxvatv_car`, the word
  openmx-mods' `import-atv cars` writes as the pack's `machine`), named as the shell names it
  (Buggy, Monster Truck, Baja, Golf Cart, Sandrail), once each by word, off the first pack listing
  it on Unleashed's step: the pack a bare `mxvatv_car` word rides. Untamed cars (MD161) are left
  out, as they are picked by `CAR@PACK`. Before, car packs were skipped from the offer. Unit test
  `a_car_packs_unleashed_cars_are_offered` in `main.rs`.
- `roster::machine_label` calls the kind "Car".
- `menu_tiles.rs`: `TileOf::Car(words)`; its liveries are Off then the pack's cars. L1/R1 step it,
  Cross steps on; the pick goes to `Settings::mxvatv_car`, which is written to settings.txt only
  while set (MD39c's writer, unchanged). The pause offers it as before (MD39c's switch row and
  P341's pause page), off the shell's per-seat flag.
- Test: `tests/menu.rs` `the_car_tile_picks_an_mxvatv_car_and_the_pause_offers_it` — the tile's
  choices, R1 picks, L1 wraps through Off to the last, saved and read back, round to Off (and the
  file then has no `mxvatv_car`), the pause's switch row and Garage page tile.
- Nothing taken from opentennis: no machine picker there.

## What the original does

Not the original's: a front-end addition (menus are ours to design). With the tile on Off, the
default, nothing changes.

## How it was proved

The menu test above (needs the disc for the Garage catalogue; skips without it).

## Not verified / not 1:1

- Not ridden end to end from the tile in the shell: the tile writes the same `mxvatv_car` setting
  `--mxvatv-car`/settings.txt already fed (MD39c's ride path, proven there); no shell run here.
- The tile has no picture: like the Quad tile, no baked car picture exists in the stage yet.
- Online the tile still shows and picks; the ride drops the car online (MD39c), as with the quad.
- Untamed car packs (MD161) get no tile choices: the tile only writes bare words, so their cars
  stay reachable only by `--mxvatv-car CAR@PACK` / settings.txt.
- Not tried against the real installed `mxvatv_cars` pack (none in this checkout's mods/); the
  tests use hand-written manifests in its format (openmx-mods `tools/import-atv/src/cars.rs`).

## Next

none
