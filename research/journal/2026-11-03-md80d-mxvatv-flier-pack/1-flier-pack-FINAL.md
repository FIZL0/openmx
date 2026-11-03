# 1 — flier pack (FINAL)

Task MD80d. 2026-11-03.

## What I did

- **openmx-mods `tools/import-atv fliers`** (commit 9f355f8 on branch `openmx-MD80d`, pushed; not
  merged, as openmx-mods' rules ask). It builds `mods/machines/mxvatv_fliers/` off SLUS_211.04: 135
  files and `mod.json`, every file a disc member's bytes at its own path.
  - `Veh\vehicletype_2.res` whole, the biplane's archive: biplane1-4, propcones, Biplane*.col
    (Biplane3/4 and the grown BiplaneM included), biplane.phy, the materials,
    biplaneparticles.dat, its `.emt`s, mxparticlezones.dat, wreckchunks.emt and event.aud.
  - The Kiowa, loose on the PSR: kiowa1-4, bladeb*, tailrotor*, Kiowa*.col (Kiowa3/4 included),
    kiowa.phy, WreckBlade.phy and helicopterparticles.dat. Also `sh_biplane2.psg`/`SH_BiPlane2.pst`
    and the pilot's `Rider\Anim\biplane_roll_{fwd,bck}.mot`.
  - Every member of the garage sets `UI\Res\Veh_BiPlane1-4.res` and `Veh_Helicopter1-4.res`. They
    hold the `<skin>_ui.psg` models and the only copy of most skins' textures (`Pitts_body01.pst`,
    `SpitFire_01.pst`, …). A name two sources give is taken once, and the import stops if the bytes
    differ. None did.
  - `Cam\FC_BiplaneDefault.pb`, `RC_BiplaneDefault.pb`, `FC_HeliDefault.pb` and `RC_HeliDefault.pb`
    out of `Cam\CameraPresets.res`.
  - carengine.aud, otherengine.aud and machine.aud, and the 14 VAGs the fliers' sections name:
    `[biplane]`'s 7 loops, `[Helicopter]` EngineLoop, `[biplaneWreck]`, `[biplaneTireTouchdown]` and
    `[helicopterWreck]`.
  - `mod.json` has `skins` (machine, skin, model 1/2, psg, ui, hull, rays and the garage set it came
    from) and `fliers.biplane`/`fliers.kiowa` (phy, cameras, replay_cameras, particles, sound
    prefix). openmx-mods standard §7a writes it down.
  - modcheck now takes `skins` in place of `builds` and checks that every file they name exists.
    build.sh's disc step builds this pack after the quad pack.
- **openmx `machine_packs.rs`**: `Pack` gains `skins: Vec<Skin>` and `fliers: Vec<Flier>`, and a pack
  needs builds or skins (not only builds). New readers:
  - `skin_models`: `psg::Model`, the skin's model and its `_ui` model;
  - `skin_nodes`: `biplane::PROP_NODE`, the Kiowa's `SPINDLE`/`TAIL`/`BLADES`, and the four seat
    dummies, each as the model spells it;
  - `skin_collision`: `track::Mesh` for the hull, `track::Rays` for the probe;
  - `skin_textures`: every material either model names, and its texture through
    `world::texture_files` and `Texture::parse`;
  - `flier_phy`: `phy::Ini`, counting each model's keys;
  - `flier_cameras`: `Preset::parse_follow` on the follow and replay `.pb`s;
  - `flier_sounds`: `LoopEngine`, `HeliEngine`, `MachineWreck` and `MachineTouchdown` over the
    pack's `Aud\`;
  - `flier_emitters`.
  `openmx --machine-packs` prints them for a pack without builds.
- **openmx `vehicle.rs`**: `Machine::mxvatv_skins`, `mxvatv_model`, `mxvatv_key` and `mxvatv_col`
  name the four MX vs. ATV skins, their model, their `.phy` key and their collision stem, with a unit
  test. Nothing calls them yet (MD80e's pick will).
- **openmx `tests/machine_packs.rs`**: packs are picked by `machine` (`atv`, `fliers`), and there are
  two flier tests. Each test skips with a SKIPPED line when its pack is missing.

## What the original does

- Skins 1 and 2 are model one, MX Unleashed's machine with MX Unleashed's names. Skins 3 and 4 are a
  second model with its own collision pair (`Biplane3F/M.col`, `Kiowa3F/M.col`). Each `.phy` keys
  the two models apart: 17 `ModelOne`/`ModelTwo` keys a model for the biplane, 3 for the Kiowa.
- kiowa3 and kiowa4 spell three seat dummies with a `01` on the end: `Dummy_Foot_R_pos01`,
  `Dummy_Hand_L_pos01` and `Dummy_Hand_R_pos01`. The readers here accept the suffix. MD80e's seat
  must too.
- None of the 14 flier VAGs has the `VAGp` magic, as with the quad's. openmx's `Clip::vag` reads
  them all the same.
- Collision sizes as read: biplane 1/2 have 24 hull triangles and 44 rays; biplane 3/4 have 42
  and 36; kiowa 1/2 have 12 and 22; kiowa 3/4 have 12 and 24.

## How it was proved

- openmx-mods: `cargo test --release -p import-atv -- --ignored` passes the quad and flier tests.
  modcheck compares 174 files byte for byte with the disc; one byte changed in SH_BiPlane2.pst goes
  FAIL, and restoring it goes ok. `import-atv fliers check` reads 29 models, 16 collision files,
  2 clips, 25 materials, 25 textures, 14 sounds and 21 emitters. `./check.sh`: ok, 150 passed.
- openmx, with the pack in this slot's `mods/machines/` (git-ignored):
  - `cargo test -p openmx --test machine_packs`: 11 passed, the quad's three and the two flier tests
    included. Controls: a kiowa3.psg truncated to 200 bytes fails `the_fliers_pack_loads_every_skin`,
    and a missing `Aud/Vehicle/mchn_wreck3.VAG` fails `the_fliers_phy_cameras_sounds_and_particles_read`.
    Both pass again once restored.
  - `cargo test -p openmx --bin openmx mx_vs_atvs_fliers`: passes.
  - `openmx --machine-packs mods` shows every skin with every node and textures read, nothing refused.
    Each flier has 8 follow and 8 replay cameras and every sound. Emitters: 9 for the biplane, 12 for
    the Kiowa, each with its `.emt`.
- `./kit check` (the full gate) was not run; there is no test gate for this task.

## Not verified / not 1:1

- **Nothing is drawn, flown or heard.** MD80b/MD80c (the fliers' MX vs. ATV steps) and MD80e (the
  pick) put the pack to use.
- **How the original seats the pilot on kiowa3/4**, whether by the `…01` names or another lookup,
  was not looked at in code.
- **`sh_biplane2`** is taken by name. It is presumably the biplane's shadow; which skin draws it is
  unknown.
- **Garage set or ride textures**: whether MX vs. ATV draws a ridden skin from the garage set's
  textures was not watched. The pack has every texture the models' materials name.
- **`event.aud` and `mxparticlezones.dat`** are carried, not read.
- **Not looked at beside MX vs. ATV in PCSX2** (T7).

## Next

`agent-runner done`. MD80b, MD80c and MD80e use the pack.
