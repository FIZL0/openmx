# 1 — untamed baja pack (FINAL)

Task MD163. 2026-11-14.

## What I did

- **The pack (openmx-mods, branch `openmx-MD163`, pushed; built on `openmx-MD39c`, which is not on its
  main yet).** `import-atv untamed-cars` copies SLUS-21701's `res\veh\VehicleType_6.res` whole into
  `mods/machines/untamed_cars/` (27 files: `Veh\baja.phy.bpb`, `Veh\Anim\baja.psg`, `Veh\Col\bajaM.col`, …).
  `mod.json` lists the baja with `"step": "untamed"`. `import-atv --disc UNTAMED --out` builds it, and
  build.sh gained the `SLUS_217.01 import-atv` line. Only the baja; the monster truck is MD161b, one line
  in `untamed_cars::CARS`.
- **The shell needed no change.** MD161 already reads the step word (`CarFiles::untamed`), reads a
  `.bpb` phy, and sets `Load::untamed` (`MxvatvCarRide::on_untamed_step`). `--mxvatv-car baja@untamed_cars`
  offers it; a bare `baja` prefers the pack listing it on Unleashed's step, so it stays `mxvatv_cars`'.
  Off by default: nothing reads the pack unless a car is picked.
- **Two new tests** (each in a file of its own):
  - `crates/game-sim/tests/untamed_baja_load.rs`: the car built off the pack's `baja.phy.bpb` against
    MD164's spawn image of the original's baja, word for word.
  - `crates/openmx/tests/untamed_cars.rs`: the shell rides the pack.
- Installed the packs into this checkout's `mods/machines/` (`untamed_cars` and MD39c's `mxvatv_cars`,
  git-ignored) so the shell tests run here. Copied MD164's `spawn.ee.bin`/`air.ee.bin` from
  kept-context/md164 into `context/untamed/md164/`, and the pack's `baja.phy.bpb` and `baja.psg` into
  `context/untamed/md163/`.
- **Split out the replay** as MD163a, blocked `--after MD161a`. The Untamed car frame harness is MD161a's
  and is still open. MD163a also holds the input step's controls-block reads and unbound guards, and the
  `Scene::update_untamed` routing.

## What the original does

- The baja's car words in SLUS-21701 sit at SLUS-21104's offset +0x30 below `+0xb00` and −0x10 from
  `+0xb00`. Its tires, engine, body and collision record are SLUS-21104's layouts. Every word
  `atv_car::Load` sets, about 1,300 of them, sits where that rule puts it and holds the BPB's value. One
  exception is below.
- **The map-edge gate** (`+0x308` in SLUS-21104, `+0x338` here; oracle.txt `# MD81a`'s list) is **0** on
  Untamed's Free Ride Rocksbury, where Desert Oasis's images have 1. With it 0 the wreck timer never asks
  the out-of-bounds test. The surrounding words line up (`+0x33c` = 4, `+0x340` = 1.0, as `+0x30c`/`+0x310`
  in SLUS-21104), so it is the same word, set by the world, not the file.
- `baja.phy.bpb`: Weight 3500; 4 gears 6, 3, 1.7, 1.1, final 5 (the engine words MD164 read).

## How it was proved

- **Save-state image, whole loader map:** `untamed_baja_load.rs`
  `the_untamed_baja_from_its_bpb_is_its_spawn_image`.
  - Builds `Load` (with `untamed`) off the pack's `baja.phy.bpb` through `game_sim::bpb::ini`, the half box
    solved from the record's inertia, as mxvatv_car_load.rs (MD150) does.
  - Compares against `context/untamed/md164/spawn.ee.bin` at the truck `0x01be4740`: keys (all 16
    surfaces of 4 tires, the engine's tables), input keys, car, body, vehicle and the part reader's words.
    **0 differ**, with `bounded` taken as the world's, and the image's held to 0.
  - This also checks the "offsets by rule" MD164 left unproven, on every loaded word, and that the
    trophy truck's values are `baja.phy`'s.
- `the_untamed_bajas_model_is_the_images_within_its_bounds`: the pack's chassis tire points at the origin
  and under the image's matrix are within 2e-3 ft of the image's. The model's half box gives the record's
  inertia within 1%.
- **Shell:** `untamed_cars.rs`.
  - `the_untamed_baja_drives_in_the_shell_on_untameds_step`: `--mxvatv-car baja@untamed_cars` on free09.
    The line says "on MX vs. ATV Untamed's step". The car goes over 100 ft on the gas. Its body words as
    it is left equal an `atv_car::Ride` made here off the pack with `Load::untamed`, stepped 250 ticks on
    the same pad and world, bit for bit. Picked and never switched to, the ride hash equals the hash with
    none picked.
  - `a_bare_baja_is_unleashed_s_beside_the_untamed_pack`: with both packs in, a bare `baja` is off
    `mxvatv_cars` on Unleashed's step, and its drive's words differ from the Untamed baja's.
  - Both tests ran here without skipping (checked with `--nocapture`).
- **openmx-mods:** `import-atv`'s ignored disc test `the_untamed_car_pack_is_the_discs_and_openmx_reads_it`
  passes. modcheck finds every file byte for byte against the disc, and openmx's readers read the BPB and
  the model back.
- **Pack off:** no sim code changed, so every replay hash is the gate's.

## Not verified / not 1:1

- **MD164's drives are not replayed** (the task's replay half). That needs MD161a's Untamed car frame
  harness (Rocksbury's terrain, Untamed's world step and hooks at Untamed's offsets), which is open. It is
  MD163a, `--after MD161a`.
- **The input step's controls-block reads and unbound guards** are not held against the original. They
  belong to the replay (MD163a). MD161 argued them away: the port takes per-command strengths, and an
  unbound one is 0.
- **`Scene::update_untamed` routing** is not decided: no drive was replayed to show whether the car needs
  MD81b's node update (MD163a).
- **The map-edge gate.** Untamed's Rocksbury free ride leaves it 0. The shell rides the Untamed baja on MX
  Unleashed's worlds with it on, as it does Unleashed's cars, because the world sets it and those worlds
  hold the car to the map. Who writes it in SLUS-21701 was not looked up.
- **The chassis model's tire points and half box** match the image only within 2e-3 ft and 1%, as for
  Unleashed's cars (MD150): the setup's frame is not in any image. So the shell's car, built off the
  model, is not the original's to the bit in those words.
- **The pack's other members** (materials, sounds, particles, the second collision object) are copied but
  nothing in openmx reads them. The baja is not drawn, and the pack was not looked at beside Untamed (T7).
- **openmx-mods' `./check.sh` is red** on import-music's `tests/built.rs` (2 tests, each `.vig` off by 28
  bytes against today's openmx reader). That code was not touched here. The branch's own tests pass.
- **The pack is on openmx-mods' `openmx-MD163` branch**, stacked on the unmerged `openmx-MD39c`, so it
  reaches main with that branch.

## Next

MD163a once MD161a lands: seed the harness from `context/untamed/md164/spawn.ee.bin` and replay
`throttle.csv` first. It is run-to-run exact against `throttle-2.csv`.
