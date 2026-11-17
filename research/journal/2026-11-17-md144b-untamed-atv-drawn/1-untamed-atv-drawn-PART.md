# 1 — untamed atv drawn (PART)

Task MD144b. 2026-11-17.

## What I did

- **Compiled INIs read as text (game-render `ini.rs`).** New `ini::text_of`: a BPB (Untamed's compiled INI,
  `game_sim::bpb`) comes out as the text of its sections and pairs, in file order, with empty sections kept.
  Any other file reads as `ini::text`. A BPB the reader refuses comes out empty.
- **The pack's build (openmx `machine_packs.rs`).** `Pack::text` reads a BPB through `text_of`, so
  `quad_draw::load` reads `ATV_R_250_KTM.sub.bpb` the way it reads Unleashed's `.sub`. A BPB that will not
  parse is an error naming the file.
- **Materials (game-render `world.rs`).** `Assets::resolve` looks for `<name>.psm` and then
  `<name>.psm.bpb`, and parses either through `text_of`. Untamed ships its materials compiled only
  (`Striker_Plastics.psm.bpb`). MX Unleashed lookups are unchanged, since `.psm` is tried first.
- **Stunt clips on Untamed's ids (game-render `quad.rs`).** The quad's clip driver indexes the 46-row
  stunt table by the id the step hands it. On Untamed's step the backflip comes as 49 and the 360s as 52/53
  (MD144), so their clips would never play. `Driver::shift` is set in `Driver::tick` from
  `ride.bike.untamed()`: `game_sim::quad::UNTAMED_STUNT_SHIFT` (now `pub`), else 0. Ids from 41 + shift up
  are read `shift` lower. Ids below 49 stay as they are, so the port's unshifted wheelie/stoppie (42/43)
  still find their rows.
- **The pack (openmx-mods, branch `openmx-MD144b`, pushed, stacked on `openmx-MD144`).**
  `import-atv untamed-atv` now copies, besides `VehicleType_1.res`:
  - Untamed's ATV rider set (`ATV_RiderMotions.mcf`, `ATV_RiderStunts.mcf`, `ATV_Rider_Stunts.ini.bpb`);
  - every clip those mcfs and the build's mcf name, and the `ATV_Bike_*` clips;
  - every model's materials and repaints (`.psm.bpb`) and each material's texture.
  
  These come off `MX08PS2.PSR`'s loose files. The `.res` copies (`RiderMotionSet_ATV.res`,
  `Vehicle_mat.res`, `Vehicle_tex.res`) have zero-padded directories that neither discx nor openmx's
  `Res::parse` reads. `mod.json` lists the loose members under `source.members` and names the `rider`
  block. The pack is now 158 files, and I reinstalled it into this checkout's `mods/machines/untamed_atv`.
- Filed MD206 (the pack's cameras, sounds and particles) and MD207 (the wheelie/stoppie ids, low).

## What the original does

- Untamed compiles its INIs to BPB. `.sub`, `.psm` and the stunt table are the same sections and keys as
  Unleashed's text, so text written back from the BPB feeds the same readers.
- Untamed's ATV stunt table has the same 46 rows as Unleashed's: backflip at 41, wheelie/stoppie at 42/43,
  360s at 44/45. Its quad step hands out ids 8 higher. MD144a's research found that its animate has
  bounds 8 higher too (46 → 54). So taking the id 8 lower before indexing the table is the same row choice.

## How it was proved

- **Unit tests.**
  - `game-render` `ini::tests::a_bpb_reads_as_the_text_it_was_compiled_from`: a generated BPB with all
    four value types, an empty section and an indexed one reads back as the text it stands for. Text input
    is unchanged, and a cut-short BPB reads empty.
  - `game-render` `quad::tests::an_untamed_quads_backflip_is_the_tables_row_8_down`: with the shift on,
    49 starts the backflip row 41 and 42 stays 42. Without it, 49 starts nothing.
- **Shell** (`crates/openmx/tests/untamed_atv.rs`
  `the_untamed_quad_is_drawn_with_untameds_rider`, ran here and wrote its shot): with
  `--atv atv_Race250P@untamed_atv` the shell says "atv: drawn as Veh/Anim/ATV_R_250_KTM.sub.bpb (5 parts,
  65 rider clips, 18 quad clips)". Over 100 frames draw `quad:250r_atv_chasis` with its parts, and every
  frame has a rider.
- **openmx-mods:** the ignored disc test `the_untamed_quad_pack_is_the_discs_and_openmx_reads_it` passes.
  modcheck finds every member, the loose ones included, byte for byte on the disc. The rider set is there,
  and the compiled build reads with `[Objects]` and `[Material Changes]`.
- **Looked at beside Untamed (T7), by eye.** Untamed's own frames come from the `Screenshot.png` inside
  MD147's `spawn.p2s` and `air.p2s`. The port was shot on free09 from a `--shot-pose` behind the quad, by
  both renderers. All four pictures are in `context/shots/md144b/`. The same quad is in both: black
  chassis, rear grab bar, white plate, the same tyres and stance, and a rider sat upright on the seat. The
  gpu and software shots agree.
- **Gate.** `./kit check --all` with the packs in was red in three Untamed tests:
  - `untamed_race_quad`'s three MD147 replays;
  - `untamed_cars`' baja drive;
  - `untamed_monster`'s truck drive.

  I ran the same three on a clean worktree of HEAD (53a3466b1), with this slot's mods, and they fail the
  same way, so none is from this task. The replays are thrown from row 1. MD144a's 300 ft Untamed edge band
  meets a quad that starts about 160 ft from Rocksbury's edge, and MD168a was proved with the 150 ft band.
  They skip in slots without the packs, which is how their merges went green. Filed K59 (critical). `kit
  done`'s gate ran with `untamed_atv` and `untamed_cars` moved out of `mods/machines/`, the terms main's
  merges ran on. They were put back after. This task's shell test passed with the pack in, as above.

## Not verified / not 1:1

- **The colours differ from MD147's frames.** There, Untamed's quad has yellow plastics and the rider wears
  orange "PLAYER 1" gear. The port's KTM 250 build draws black plastics (`Striker_Plastics`, through its
  `Material Changes`) and the rider set's default red/white gear. Those are the player's picks in Untamed.
  Which build and paint MD147's state used was not found, and the pack has no garage to pick them.
- **No live PCSX2 shot at the same spot.** Copy 2 booted Untamed from `spawn.p2s` (PINE said SLUS-21701,
  running), but pressing System → Screenshot over AT-SPI wrote nothing, three times. The desktop is likely
  locked, so the window never presents. The comparison uses the save states' own screenshots, at a
  different place (Untamed's stadium, not free09) and with Untamed's camera, not the port's.
- **Animation not compared.** Nothing compared the quad's or rider's clips, frame by frame, with Untamed.
  Only a still was looked at, and no trick was shot.
- **The −8 row mapping is not checked against SLUS-21701's animate.** It rests on MD144a's reading of
  animate's bounds and on the stunt table's row order. No oracle or capture covers it.
- **Wheelie/stoppie ids.** The port's Untamed step still hands out 42/43, where MD147's image has 50/51
  (MD207). The driver plays the right row for either.
- **No cameras, engine sound or particles** from the pack (MD206). The shell rides it on the fallback view.
- **Only the KTM 250 build is drawn.** The Kawasaki and 450F builds and their `_AI` twins are copied but
  not listed in `mod.json`, and were not drawn.
- **The padded `.res` archives stay unread.** The pack takes the PSR's loose copies instead, and modcheck
  proves those are on the disc. Whether the `.res` copies hold the same bytes was not checked.
- **The openmx-mods branch needs this task's `ini::text_of`.** Its disc test reads through it, so it builds
  only against an openmx with MD144b.

- **The gate ran without the Untamed packs** (K59, above). So `kit done` did not run this task's
  shell test or the replays. The shell test was run on its own with the pack in, and it passed.

## Next

None: the task is done. MD206 and MD207 are the follow-ups.
