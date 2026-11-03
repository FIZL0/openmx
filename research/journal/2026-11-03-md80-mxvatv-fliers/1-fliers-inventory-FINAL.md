# 1 — the fliers, file by file and step by step (FINAL)

Task MD80. 2026-11-03.

## What I did

- Pulled MXVSATV.PSR off the MX vs. ATV iso (`bsdtar`, memory mxvsatv-psr-off-the-iso). A scratch crate on
  `game_sim::archive::{Psr, Res}` listed every member and every `.res` member naming biplane, kiowa,
  helicopter or wreckblade, and wrote the .phy and .col files out. Did the same for MX Unleashed's MX.PSR
  (`machine.res`, and the free-ride tracks' `_ss_2.res`). Copies are in s1's
  `context/mxvatv-ps2/md80/{phy,col}` (git-ignored).
- Diffed the files: `research/md81_phy_diff.py` for the .phy files; byte and word compares for the .col files.
- Found SLUS-21104's functions from their strings (`%sbiplane.phy`, `ModelOneSpan`, `%sWreckBlade.phy`,
  `%skiowa.phy`, the Kiowa's `CLRoundOff`) in the atv program's xrefs dump, then took the vtables from the
  pointer words in the ELF.
- Paired every function in SLUS-21104's biplane, Kiowa and flight-model ranges with MX Unleashed's by
  mnemonic shape (`research/md80_pair.py`). Vtable entries Ghidra had not made into functions were added
  by hand. Then compared each pair's mnemonic counts and float constants (`research/md80_hist.py`).
- Read the biplane's init and reader in the decompile to see how a model is picked.
- Filed MD80a–e.

## What the original does

research/mxvatv-pc.md "The biplane and the Kiowa (MD80)" has the files, the model switch, and the step
table. In short:
- Model one is MX Unleashed's machine. The changes are a moved pelvis box, MaxAltitude 1200,
  WreckTime → MaxWreckTime, and a `[Camera]` section.
- Model two (biplane3/4, kiowa3/4) has its own wing, gear and boxes.
- The biplane's tire probe and frame, gear tire and contact, wing geometry, and the Kiowa's rotor, are the
  same code. The tire strut and friction are fused. The gear springs are the same, but their two `sqrt.s` take
  their operand in ft, which `kit dis` prints as `.word`.
- The engine, controls, autopilot and flight controller differ only in fused multiply-adds, except for the
  controller's new −30.
- The biplane's aero step is fused and slightly restructured. The Kiowa's aero step is rewritten. The
  biplane's wreck check is now MXVehicle's.

## Taken from opentennis

Nothing: opentennis has no second game to diff against.

## How it was proved

- The file diffs are exact: key by key as f32, bytes for the cols.
- The pairs are by shape, with their scores in oracle.txt. Nothing was run. Pairs at 0.9 and above with
  matching constants are sure. The 0.5–0.7 ones (the inits, the aero steps, the wreck check) are each the
  only good candidate, but their bodies were not compared clause by clause.
- `./kit audit`: only the 4 leaks already on main (atv_wheel.rs:62/66, plan/DONE.md:519/533).
- No code changed, so no tests were run for this.

## Not verified / not 1:1

- Nothing is captured or run. The brief's "a captured SLUS-21104 flight of each replays bit-exact" and
  "offered off by default as a pack" are MD80a–e, not done here.
- The biplane aero step's two missing `abs.s` and extra branch, the Kiowa aero rewrite and the
  controller's −30 are seen in counts, not read line by line.
- kiowa_wreck_check has no twin by shape (best 0.30). It may be MXVehicle's shared check, as the
  biplane's is, but that is not checked.
- The PC exe (R10) was not read for these. Its pc_biplane_* rows predate this task, and MXVehicleHelicopter
  has no pc rows for its steps. MD80b/MD80c read it first.
- Which spawn-record forms (+0x10c) the garage's skins produce is inferred from the PSG sizes, not traced.
- The new cols (BiplaneM's 2528 bytes, Biplane3/4, Kiowa3/4) were not decoded.
- No MX vs. ATV map was checked for which fliers it places.
