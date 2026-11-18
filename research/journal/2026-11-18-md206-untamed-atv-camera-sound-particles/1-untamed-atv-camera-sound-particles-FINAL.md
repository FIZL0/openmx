# 1 — untamed atv camera, sound, particles

Task MD206. 2026-11-18.

## What I did

- **The pack (openmx-mods, branch `openmx-MD206`, pushed, on top of main's MD144b).** `import-atv untamed-atv`
  now also copies:
  - the quad's camera files, loose on `MX08PS2.PSR` and compiled: `Cam\FC_ATVDefault.pb.bpb`,
    `Cam\RC_ATVDefault.pb.bpb`, `Cam\FC_ATV.ini.bpb`;
  - every sound the archive's `Aud\vehicle.aud.bpb` names, off the disc's second PSR `MX08AUD.PSR`:
    22 engine `.VAG`s under `Aud\Engine\450A\` and the 4 `[Wreck]` ones under `Aud\Vehicle\`.

  `mod.json` names `cameras`, `replay_cameras`, `cameras_ini`, `sounds.engine/events` and
  `particles.list/ai/zones`. The `.aud` and `Part\*.bpb` files were in `VehicleType_1.res` all along, but
  nothing named them. The pack is now 187 files. I reinstalled it into this slot's `mods/machines/untamed_atv`.
- **openmx `machine_packs.rs`.** `Pack::ini_bytes(name)` and `Pack::ini_suffix(suffix)` read a file, or its
  `.bpb` twin when the pack has no plain one. A BPB comes back as the text of its sections
  (`ini::text_of`). `Pack::emitters` reads each `.emt` through it.
- **openmx `main.rs`.** The quad's camera files, particle list, zone table and emitters are now read through
  those two. A particle list naming `exhaust.emt` finds `exhaust.emt.bpb`.
- **openmx `chase.rs`.** `chase::slow_fast_layout` spots a camera file in Untamed's own layout.
  `atv_presets` passes such a file over, and the shell says so.
- No `vehicle.rs` change was needed. Nothing in `../opentennis` covers other games' packs, so I took nothing
  from it.

## What the original does

- **Untamed's quad camera is a different camera.** Each preset in `FC_ATVDefault.pb.bpb` is a slow/fast pair
  (`SlowAngleX`, `SlowFollowDistance`, `FastFov`, `SlowSpeed`/`FastSpeed`) plus four tightnesses. The names
  are "Normal Camera", "Close Camera" and so on. SLUS-21701's strings have those keys. They do not have
  `FollowingDistance`, `MinFOV`, `LookFilters` or `PosFilters`, the keys SLUS-21104's camera reads.
  Through `Preset::read_layout` every distance and FOV would be zero.
- **Untamed's `FC_ATV.ini` (and its `.bpb`) is still in SLUS-21104's layout**: 12 presets, "Race Cam 1"
  at 13 ft and AngleX 43. MD136 found the same numbers in Unleashed's `FC_ATV.ini`.
- **The `.aud` matches Unleashed's.** Untamed's `vehicle.aud.bpb` has the same lists and the same 450A
  sounds as MX vs. ATV Unleashed's `vehicle.aud`.
- **The particle files are Unleashed's layout, compiled.** The zone table adds `Zone13=GrassSpray` and
  `Chunks 1` groups.

## How it was proved

- **openmx-mods.** The ignored disc test `the_untamed_quad_pack_is_the_discs_and_openmx_reads_it` passes. It
  now checks the camera files, three sounds and the four `mod.json` names. modcheck finds every listed
  member, the audio PSR's `.VAG`s included, byte for byte on the disc.
- **Shell** (`crates/openmx/tests/untamed_atv.rs`
  `the_untamed_quad_says_its_cameras_engine_and_particles_are_the_packs`). With
  `--atv atv_Race250P@untamed_atv`, switched in at tick 30, the shell says:
  - the `.pb` is passed over;
  - "untamed_atv's camera, 12 presets in Cam\FC_ATV.ini: RaceCam1, …", and the camera starts on RaceCam1;
  - "untamed_atv's engine, 22 sounds in Aud/vehicle.aud.bpb, 4 wreck", with no sound unread;
  - zones off `Part/atvparticlezones.dat.bpb`, and "18 entries off untamed_atv's Part/atvparticles.dat.bpb";
  - Tire Dust is in the air at the end.

  All 4 `untamed_atv` tests pass, and so do `atv_camera_sound` and `mod_follow_camera` (Unleashed's quad,
  unchanged).
- **Looked at beside Untamed (T7), by eye.** I compared the port's shot (`context/shots/md206/`, free09,
  tick 148) with MD147's `spawn.p2s` screenshot (`context/shots/md144b/untamed-spawn`). Both follow from
  straight behind. The port's Race Cam 1 sits higher and farther back. Untamed's Normal Camera is low and
  close, the quad filling the lower third. That gap is MD251.

## Not verified / not 1:1

- **The follow camera is not Untamed's.** Untamed follows the quad on `FC_ATVDefault.pb`'s slow/fast Normal
  Camera. The port rides `FC_ATV.ini`'s Race Cam 1 on SLUS-21104's camera, because openmx has no reader or
  step for the slow/fast layout. Which file SLUS-21701 opens was read off its key strings only: no file
  name was found in the ELF, and no capture was taken. Filed MD251.
- **Preset names lose their spaces** ("RaceCam1", not "Race Cam 1"). The BPB's strings are unquoted when
  written back as text, and the text reader drops spaces in unquoted values. Only the shell's lines and
  any HUD name show it.
- **The engine was not heard.** The headless test proves the 22 + 4 sounds are read and handed to the
  quad's engine voice (`set_own_atv`, MD136's path). It does not prove they play, or that the quad sounds
  like Untamed's. No recording of either was made.
- **The exhaust smoke is not run.** "Right Exhuast" hangs on `250_Exhaust_Particle01`, which
  `250r_atv_chasis.psg` lacks (likely a part model's node). Filed MD252 (low).
- **The water splashes don't run, and no tire tracks are laid.** The four water splashes stay skipped
  (`Water`), as on Unleashed's quad. The 2 tire tracks are not laid, as before.
- **The zone table's `GrassSpray` and `Chunks 1` groups** are Untamed's track sheet groups. On MX Unleashed's
  tracks they fall to whatever the sheet has, which was not checked.
- **Not carried or followed:** `event.aud.bpb` and the AI particle list, as on Unleashed's pack. The race
  camera's `RC_ATVDefault.pb.bpb` is carried but not read, since the shell's quad reads only the follow file.
- **No live PCSX2 comparison.** As in MD144b, the look used the save state's own screenshot, at Untamed's
  stadium and not free09.
