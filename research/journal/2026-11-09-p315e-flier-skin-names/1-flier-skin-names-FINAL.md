# 1 — flier skin names (FINAL)

Task P315e. 2026-11-09.

## What I did

All the work is in openmx-mods: commit on branch `openmx-P315e`, pushed and not merged, as openmx-mods'
rules ask. I made it in a new worktree, `~/repos/openmx-mods-p315e`, off origin/main, because the main
checkout there is 6 commits behind. Its `openmx` symlink points at this slot. openmx's own code is not
changed: since P315 the shell reads `skins[].name`/`about` (`machine_packs.rs`, `roster::Flier`).

- **`import-atv fliers`** now writes `name` and `about` for each of the eight skins. Three tables on
  the disc supply them. They are read and not carried:
  - `UI\Store.dat`: the `StoreText` of the section whose `AssetName` is the skin, e.g. `[Plane 3]`
    `StoreText = "PLANE (3)"` `AssetName="BiPlane Skin 3"`. It is written in the case that
    `UI\Challenges.dat`'s unlock lines use ("Plane (2) is now unlocked."). That gives `Plane (1)` …
    `Plane (4)` and `Helicopter (1)` … `Helicopter (4)`.
  - The about is built from four parts:
    - the model ("Model one: MX Unleashed's shape and flight figures." or "Model two: its own shape,
      collision and flight figures.", the same words as openmx's `roster::model_about`);
    - `Veh\Vehicles.dat`'s garage `Logo` (`[BiPlane Skin 1]` `Logo="THQ"`; also Rainbow, BuzzOff,
      MaddCow);
    - the store `Price` (250000 for all eight);
    - the challenge whose `Prize` is the skin: biplane4 is the Airplane Challenge's, kiowa4 the
      Helicopter Challenge's.

  A skin no store entry names keeps no `name` key and gets a todo line. No skin does.
- **modcheck** checks each `skins[]` entry of a machine pack:
  - `machine`/`skin` are given and not empty, and no pair appears twice;
  - `model` is the number 1 or 2;
  - `psg`, `hull` and `rays` are named and in the pack, and `ui` is in the pack where it is named;
  - `name`/`about` are non-empty text where given.

  `ui` was required before. It is optional now, because openmx's preview falls back to the psg.
- The standard's §7a documents the fields and the checks.
- My slot's (git-ignored) `mods/machines/mxvatv_fliers/mod.json` is replaced by the new build. Apart
  from the two new keys per skin it is identical: I diffed it with name/about stripped.

Taken from opentennis: nothing. It has no machine packs.

## What the original does

MX vs. ATV has no livery names in its tables. Pitts and Spitfire exist only as texture names
(`Pitts_body01`, `SpitFire_01`). The store lists the skins as `PLANE (n)` / `HELICOPTER (n)`, under
assets `BiPlane Skin n` / `Helicopter Skin n`. `Vehicles.dat` gives each skin `Locked` and a `Logo`,
and the logo picks `UI\Tex\logo_bkg_<Logo>` / `logo_frg_<Logo>`. `Challenges.dat`'s challenges 8
and 9 give `BiPlane Skin 4` and `Helicopter Skin 4` as their `Prize`.

## How it was proved

- openmx-mods `cargo test --release -p import-atv --test fliers -- --ignored`, with DISCX_ISO set to
  the main checkout's iso/, passes. It builds the pack off SLUS_211.04, asserts all eight names and
  the biplane and kiowa4 abouts, and modcheck passes the pack (174 files byte for byte).
- openmx-mods `tools/modcheck/tests/disc.rs::a_machine_packs_skins_are_held_to_the_standard`
  passes. It runs a good pack, a pack without name/about/ui, and one failing case for each rule
  (empty machine, no skin, model 3, model "2", no psg, rays not in the pack, empty name, numeric
  about, a pair listed twice, a ui not in the pack).
- In openmx, with the new mod.json installed and a scratch `garage.txt` picking
  `biplane mxvatv_fliers biplane3` and `kiowa mxvatv_fliers kiowa4` (deleted after),
  `target/debug/openmx --headless --track free09 --frames 30 --garage --mods mods` prints:
  ```
  fliers: P1 biplane Plane (3) (biplane3), from mxvatv_fliers, model 2
  fliers: P1 kiowa Helicopter (4) (kiowa4), from mxvatv_fliers, model 2
  kiowa: mxvatv_fliers's, on its steps, drawn in kiowa4
  ```
- openmx-mods `./check.sh` fails in `import-music --test built`. Two tests fail on `.vig` byte counts:
  840 vs 868 and 1400 vs 1428, measured against the openmx the symlink points at. import-music is
  untouched, and modcheck's change only reaches machine packs. Every other crate passes
  (`cargo test --release --workspace --exclude import-music`).

## Not verified / not 1:1

- **Whether MX vs. ATV's garage shows `StoreText` as the skin's name**: not checked in PCSX2 (T7).
  The garage may show only the logo or a number. The store text is the only per-skin name in the
  disc's tables.
- **Challenge 8 says it unlocks "Plane (2)", but its `Prize` is `BiPlane Skin 4`.** The about follows
  `Prize`. Which skin the game actually gives was not checked.
- **What `Logo` means** (the sponsor logo in the garage) was inferred from the `logo_bkg_*` texture
  names, not watched.
- **`BiPlane Skin n` is model `biplane<n>`**: assumed by number, the same numbering MD80d's garage sets
  use (`Veh_BiPlane<n>.res` holds `biplane<n>_ui.psg`). The game's code mapping the two was not read.
- **The Garage/Switch To Machine screens with the new names** were not shot. Only the ride's `fliers:`
  lines show them. The picker draws `name`/`about` the same way the renamed test pack's were drawn in
  P315.
- **openmx-mods' `import-music --test built` is red** for a reason outside this task (above). It is
  not filed in openmx-mods' plan from here.

## Next

None. The openmx-mods branch `openmx-P315e` waits to be merged there.
