# 1 — atv pack (FINAL)

Task MD37c. 2026-10-29.

## What I did

- **openmx-mods `tools/import-atv`** (pushed to openmx-mods main, 7585716): builds
  `mods/machines/mxvatv_atv/` off the user's MX vs. ATV Unleashed disc (SLUS_211.04). 466 files, every
  one a disc member's bytes at its own path:
  - `Veh\vehicletype_1.res` whole — the archive `%sVehicleType_%d.res` names for type 1: 96 `.sub`
    builds (`ATVS_R_*`, `ATVS_S_*`), the motion sets `ATV_SM.mcf`/`ATV_S.mcf`/`ATV_R.mcf` (+
    `250R_450F_THQ01.mcf`), the three chassis models `ATVS_R_Chasis`/`atv_s_chasis`/`250R_atv_chasis`,
    every part `.psg`, 95 `.psm`, 8 `atv_*.phy`, 7 `.col` (WreckATVm among them), 13 clips,
    `Aud\vehicle.aud`, `Aud\event.aud`, `Part\atvparticles.dat`, `atvaiparticles.dat`,
    `atvparticlezones.dat` and 15 `.emt`;
  - every `Veh\Anim\ATV_*.mot` loose on the PSR (BackFlip and the other flips are not in the archive);
  - `Rider\RiderMotionSet_ATV.res` whole, plus every clip its two motion sets name that is loose
    instead (22 of them: `hold_*` and most stunt clips), and `Rider\Anim\ATV_Rider_Stunts.ini`;
  - `Cam\FC_ATVDefault.pb` and `Cam\FC_ATV.ini` out of `Cam\CameraPresets.res`;
  - the textures every material names (loose `Veh\Tex\` or `Veh\Vehicle_tex.res`);
  - every `.VAG` `vehicle.aud` names, `[Wreck]`'s extension-less `Vehicle\WRECK0n` as `.VAG`.
  `import-atv check DIR` reads every model, clip, texture and sound back with openmx's readers and
  prints each chassis' node tree. modcheck learned kind `machine` (standard §7, new), `OUTER:INNER`
  source members and case-blind lookup (the pack spells `veh\tex`/`Veh\Tex` one way); build.sh has the
  step. `cargo test -p import-atv --release -- --ignored` builds the pack, proves it with modcheck and
  reads it back (passes, 18 s).
- **game-render `quad.rs`**: the quad's node names (four wheels, `LeftHub`/`RightHub`, the four
  A-arms, tie rods, `SwingArm`, `RearAxle`, `HandleBars`) and `quad::Nodes::of(&Skeleton)`; the tree in
  its docs is what `import-atv check` printed (one 43-node skeleton in all three chassis, root node
  spelt `250r_atv_chasis` in each).
- **game-render `scene::Preset::parse_follow(text, view)`**: MX vs. ATV's `[<aspect>/<viewport>/<n>]`
  camera sections (`4x3/1p`, `16x9/1p`, `4x3/2pv`, …), unit-tested.
- **openmx `machine_packs.rs`**: finds kind-`machine` packs in `mods/machines/`, opens the folder as a
  `Res`, and reads builds (`Vehicle::parse`), models (`psg::Model`), quad nodes, rider clips (both
  motion sets through `MotionSet` + `skin::Clip`), the quad's `ATV_Bike_<name>` clips, the stunt table
  (`trick_pack::table`), cameras, engine sounds (`audio::BikeSounds::read` over the pack's `Aud\`) and
  emitters (`biplane::emitters` + `particles::Emt::parse`). `openmx --machine-packs [MODS]` prints a
  report. Nothing rides it: no ride path calls it.
- **openmx `tests/machine_packs.rs`**: three tests; they skip without a pack in `mods/machines/` and
  print `SKIPPED: no machine pack in … (openmx-mods' tools/import-atv makes one)` (tried both ways).

`openmx --machine-packs mods` on the built pack: 3 chassis with every quad node, 644 parts read,
65 rider clips (0 refused), 17 quad clips, 46 stunt rows, 8 cameras per 1p layout, 26 engine sounds
(0 refused), 24 emitters all with their `.emt`.

## What the original does

- The quad's archive name is `%sVehicleType_%d.res`; type 1 holds every `ATVS_*` build (MD37 found
  the format string; the type number is from the archive's contents, not from code).
- The rider's set is `%sRiderMotionSet_%s.res` with `ATV`; its two `.mcf` say
  `ContentDirectory=Rider\Anim\`, and the clips not in the `.res` are loose there — so the game loads
  them loose (the `.MLB` beside each `.mcf` was not opened).
- The follow camera file GAME_REL.BIN names is `FC_ATVDefault.pb` in `%sCameraPresets.res`
  (`FC_ATV.ini` is beside it and differs: Race Camera 1 is 14.5 ft / AngleX 42 in the `.pb`, 13.0 / 43
  in the `.ini`).
- Three engine VAGs and the four wreck VAGs have a VAG header with its `VAGp` magic zeroed; openmx's
  `Clip::vag` never reads the magic, so they decode.

## How it was proved

- modcheck: all 466 files byte for byte against the SLUS_211.04 disc (`import-atv` disc test).
- openmx's own readers on every file kind (`tests/machine_packs.rs`, `import-atv check`): counts above.
- Each `ATV_Bike_*` clip has at least one channel naming a chassis node (`tests/machine_packs.rs`).
- Nothing was looked at in PCSX2 or drawn.

## Not verified / not 1:1

- **Nothing is drawn, posed, heard or ridden.** The pack loads; MD37b (the quad's step) and MD37d (its
  pick) are what put it on screen. Whether the models draw right with openmx's renderer, whether the
  clips bind and pose correctly on these skeletons, and whether the sounds sound right is unchecked.
- **The ATV rider clips on MX Unleashed's rider skeleton**: the ATV `.mcf` says `SLTFile=mrider.Slt`,
  the same rider model, but no clip was sampled onto it.
- **Which nodes the original drives** (hubs, A-arms, tie rods, struts) and how: not looked at in code;
  `quad.rs` only names them.
- **`FC_ATVDefault.pb` over `FC_ATV.ini`**: taken from the name in GAME_REL.BIN (MD37); which file the
  game really reads at ride time was not watched in PCSX2. Both are in the pack.
- **Which build rides which `.phy`** (`atv_` vs `atv_s_`), and why Vehicles.dat names `ATV_R_*` subs
  while the archive holds `ATVS_R_*`: not resolved (MD37's open points; MD37b/d's).
- **`event.aud`** is carried, not followed: its sounds are the race's (gate, countdown), named without
  `Aud\` paths this pack resolves.
- **The `.MLB` motion libraries** (`Rider\Anim\ATV_RiderMotions.MLB`, `ATV_RiderStunts.MLB`) are not in
  the pack; every clip the sets name was found loose, so they were not needed to load.
- **The particle sheet**: the emitters name particle groups out of the track's sheet; whether MX
  Unleashed's tracks' sheets have the ATV's groups is unchecked.
- **The PC paks**: MD38 is dropped, so the pack is the PS2 disc's only.
- `ATVS_*` part models: read by `psg::Model::parse`, never drawn.

## Next

`./kit done`. The pack is consumed by MD37b/MD37d.
