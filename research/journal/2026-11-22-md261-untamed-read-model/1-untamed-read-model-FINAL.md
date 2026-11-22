# 1 — untamed read model (FINAL)

Task MD261. 2026-11-22. Follows research/journal/2026-11-21-md250-part-reader-box/1-part-reader-box-FINAL.md.

## What I did

- Added `quad::Load::read_untamed_model` (crates/game-sim/src/quad.rs): `read_model` with the part reader's
  box from `Model::untamed_half_box` (MD250). Mounts unchanged.
- The shell's quad (`AtvRide::made`, crates/openmx/src/vehicle.rs) reads with it when the pack rides on
  Untamed's step (`on_untamed_step`), else with `read_model` as before.
- New test `route::the_untamed_quads_record_inertia_is_its_spawn_images`
  (crates/game-sim/tests/untamed_atv/route.rs).
- No opentennis code applies (it has no quad loader).

## What the original does

As MD250's journal: Untamed's part reader (`./kit fn 0x004b9680`) takes the box after its world update has
made each model node's frame times its parent's orthonormal again. Nothing new found here.

## How it was proved

A save-state RAM image: MD147's `spawn.ee.bin`.
- The untamed_atv pack's race quad (`atv_Race250P.phy.bpb`, `250r_atv_chasis.psg`) read with
  `read_untamed_model` at Rocksbury's start spot (`SX_A.scn`'s `[SceneInfo]`), spawned, rigged and routed
  to Untamed's step.
- Its record's inverse inertia equals the image's quad record `+0x70` bit for bit:
  `3c0c4926 3c0951d1 3c64b684`.
- The test also asserts `read_model` (SLUS-21104's box) does not: it gives `3c0c4949 3c0951f1 3c64b689`,
  35, 32 and 5 ulps off. So the test tells the two boxes apart.
- Checked the test really ran (no skip) with a temporary panic printing both words, then removed it.

## Not verified / not 1:1

- **The shell's quad away from Rocksbury's start.** The shell makes its quad where the rider takes it (on
  MX Unleashed worlds), not at Rocksbury's start spot. There is no capture of Untamed's quad made at those
  spots, so the shell's box is proved only by sharing the function the test proves.
- **route.rs's other `read_model` callers keep SLUS-21104's box, on purpose.** `spawned` and the wheelie /
  stoppie id test use Unleashed's quad files (md37a); `spawned` feeds both sides of the routed-vs-SLUS
  comparisons, which need the same quad. The id test reads only stunt ids, which the box does not touch.
- **Mounts and the start spot's words**: unchanged from MD250's notes (wheel 2's mount z 1 ulp off; the scene
  file's float parse 1 ulp off in x and z). Not checked here.
- **Boxless model nodes**: as MD250, `untamed_half_box` assumes every node has a box; Untamed's quad models do.

## Next

Nothing for MD261.
