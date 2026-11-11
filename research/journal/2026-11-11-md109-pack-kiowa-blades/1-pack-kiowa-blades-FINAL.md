# 1 — pack kiowa blades (FINAL)

Task MD109. 2026-11-11.

## What I did

- **Front-end garage: already done, now tested for both kinds.** P315a (41a7fd2d1) had already made
  `garage_view`'s Fliers tab turn a pack's skin in its `_ui` model, for whichever kind the row is on.
  Its test only dressed `biplane3`. `fliers_dress_off_the_disc_and_a_packs_garage_model` now dresses
  all eight MX vs. ATV skins (biplane, biplane2-4, kiowa, kiowa2-4) off the pack's `_ui` models. Each
  must have triangles, textures and a size. Passes.
- **Thrown blades off the pack.** `flier_refit::blade_files` reads each of the four files the wreck's
  blades are built off (`wreckblade.phy`, `bladebent.psg`, `bladebentm.col`, `bladebentf.col`). It
  takes the pack's copy where the seat flies a pack Kiowa and the pack has that file (found by name in
  any folder with `Res::read_whole`), and the disc's otherwise. `kiowa_ride` builds its `Blades` from
  them. The load prints a new line,
  `kiowa: [P2 ]thrown blades: N of 4 files off <pack>, the rest the disc's`. The existing load line
  stays as it was, so P315b's exact-match test still holds.
- The drawn rotor parts (`bladeblur`, `bladebent`, `tailrotor*`) already came off the pack, through
  `Kiowa::load_model` on the pack's files (MD80e). `kiowa.rs` now notes that SLUS-21104 loads the
  plain names for both models.
- New test `mxvatv_fliers.rs::the_packs_kiowa_throws_the_packs_blades`. It plays kiowa_switch.rs's fast
  landing (made in that file) on pack skins 1 and 3 (model one and model two). At least 2 of the 4
  files must come off the pack, and 4 blades must be out at tick 440. On SLUS-21104's steps the
  helicopter hits the downhill at about tick 380, earlier than MX Unleashed's 410, and is put back
  about 150 ticks later. That is why the test reads at 440 and not 530.
- Filed MD130 (low): openmx-mods' import leaves `Veh\Col\BladeBent{M,F}.col` out of the pack, so the
  two collision files still come from the disc.

- The first gate run went red. The runner's memory watch killed `game-sim --test mxvatv_bike_ride` at
  31 GB, and run alone it was `the_parts_box_is_the_originals` running away. main had fixed that at
  05:31 today (3bbecc9b3), after this branch was cut, so I cherry-picked it (`-x`, as its own commit)
  instead of merging main. The change is identical, so the merge stays clean.

Nothing was taken from opentennis: it has no helicopter or pack loader.

## What the original does

- SLUS-21104's MX helicopter init (`./kit fn 0x004d0b20 -o atv`) builds the four thrown blades. Each
  is the generic physics object's vt +0xd4 with the one name `bladebent` (string at 0x5290c3), after
  reading `%sWreckBlade.phy`. There is no switch on the model word (+0x1340). The parts loader it
  calls first (`./kit fn 0x004d1030 -o atv`) loads `bladeblur` four times, `tailrotorblur` and
  `tailrotorbent` by their plain names. The ELF holds no `bladebent2`/`bladeblur2`/`tailrotor*2`
  string, so the pack's `…2.psg` files are never read. The port follows this.
- Bytes, compared with a throwaway test against MXVSATV.PSR (pulled off the iso with bsdtar):
  `Veh\WreckBlade.phy`, `Veh\Anim\bladebent.psg`, `Veh\Col\BladeBentM.col` and `BladeBentF.col` are
  byte-identical to the disc's `helicopter\…` copies. `bladeblur`, `tailrotorblur` and
  `tailrotorbent` are identical too. Only `Veh\Tex\Kiowa_Blades.psm` differs (1579 bytes against the
  disc's 2404). So reading the pack's files changes no simulation bytes.

## How it was proved

- Byte comparison of the four blade files and three rotor parts: pack/PSR against the disc's
  `machine.res` (a scratch test, deleted afterwards).
- Shell test `crates/openmx/tests/mxvatv_fliers.rs::the_packs_kiowa_throws_the_packs_blades`: the pack
  supplies `wreckblade.phy` and `bladebent.psg`, and the wreck throws 4 blades for models one and two.
- Unit test `garage_view::tests::fliers_dress_off_the_disc_and_a_packs_garage_model`: every skin dresses.

## Not verified / not 1:1

- **The pack's blade collision is still the disc's.** The pack has no `BladeBent{M,F}.col` (MD130). The
  bytes are the same as MX vs. ATV's, so the blades behave identically.
- **The thrown blades' flight was not compared against SLUS-21104.** Nothing captures a pack Kiowa's
  wreck in MX vs. ATV. The blades fly on MX Unleashed's blade step (S220a1), fed MX vs. ATV's (identical)
  files. On model two (skin 3) one blade was 231 ft over the ground at tick 440, and the lowest was
  5.2 ft under it. That was observed and not looked into. Whether SLUS-21104 throws them as high is unknown.
- **Shared rotor-part names in `Assets`.** The parts are cached by bare name (`bladeblur`, …). A ride
  with a disc Kiowa on one livery and a pack Kiowa on the other draws both with whichever loaded first.
  The geometry is byte-identical. Only the blade material file (`Kiowa_Blades.psm`) differs, so the
  blades' look may follow the other livery's. This is rendering, matched by eye, and was not checked.
- **The garage shows the Kiowa without rotor blades.** It is the `_ui` model alone, as the disc's
  Kiowa there is the bare model. It was not compared with MX vs. ATV's own garage screen.

## Next

Lands as is.
