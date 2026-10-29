# 1 — atvs (FINAL)

Task MD37. 2026-10-29.

## What I did

- Split MD37 (AGENT.md "if a task turns out to be too big"): MD37a captures → MD37b step (after MD37a,
  MD40) → MD37c pack (after MD37a) → MD37d in game (after MD37b, MD37c) → MD37e online (after MD37d, MD6).
  Why after MD40: the ATV's Force body calls the same preload, backflip, 360, engine, air drag, airborne
  control and wheel model functions as SLUS-21104's bike, and runs on its frame driver (the physics world's
  substeps). MD40 ports those for MX vs. ATV's bikes; building them twice would split the port.
- Read `MXVehicleAtv` in the PC exe (R10's project, `./kit fn MXVehicleAtv::vfN -o pc`) and paired it into
  SLUS-21104 (`-o atv`). 15 `pc_quad_*` + 15 `atv_quad_*` rows in research/oracle.txt (`# MD37`).
- research/mxvatv-pc.md "The ATV (MD37)": the data, the wheels, Pre/Force/Post/update against the bike's,
  what is unpaired.
- Copied s2's `context/mxvatv-ps2/atv_game.elf` into this slot's context/ (git-ignored) to read
  SLUS-21104's tables.
- Took nothing from opentennis: it has no vehicle classes.

## What the original does

- **How SLUS-21104's ATV was found.** `atv_wheel_suspension` has two callers that call it four times:
  0x004b9580 (the Force body, its every call in the PC Force's order) and 0x004bdd20 (the net unpack). The
  pointer table holding 0x004b9580 (entry 0x003ce364) runs parallel to the bike's (0x003d0564), 0x2200
  apart, from about 0x003ce258 to 0x003ce7ac. 98 entries are equal (MotoShared's). The ATV's own entries
  sit where the bike's own ones are, and each was paired through the bike's known twin (`table`), checked
  by strings (WreckATVm.col at 0x004ba0e4 inside 0x004b9db0; the wheel node names in 0x004ba310; the tire
  keys in 0x004bb880) or call order. `./kit dis 4b9da0 --len 0x30 --program atv`: 0x004b9da0 is a one-jump
  stub, and the init starts at 0x004b9db0, which Ghidra has not made a function.
- **The step**, in research/mxvatv-pc.md. In short: the bike's per-frame code with four wheels. Two front
  (steered), two rear (driven). Every front/rear write goes to both of the pair. Stand-upright and balance
  are replaced by one stand-upright from the four contact normals (no balance wobble). Roll/pitch, ground
  pitch torque and wheel contacts are the quad's own. The rear-pair +0xb4 flag is set when a front wheel is
  down and both rears are up. There is no no-hander in the PostProcess.
- **Files.** `atv_` / `atv_s_` × Race/Pro × 250 × A/P. The keys are exactly the bike's (diffed). The PC
  path builder 0x00566480 picks `atv_s_` when the machine's model name holds `ATV_S_`.

## How it was proved

Research only; no port, so nothing to prove bit-exact. The pairings were checked as listed per row
(strings, call order, table place against a known twin). The `.phy` key sets were diffed with a script
over the PC paks' files, which R10's notes say are SLUS-21104's byte for byte.

## Not verified / not 1:1

- Nothing of the ATV is ported or proved. Each part is filed: MD37a–e.
- PC slots 33, 35, 38, 39, 67, 70, 80, 86 (and 111) are not paired with SLUS-21104. The candidates are
  listed in research/mxvatv-pc.md. The pointer table is not one slot per entry on both sides, and the sizes
  and constants alone were not conclusive (vf67 ≈ 0x004be3c0 and vf111 ≈ 0x004be8d0 by constants and size
  only, so they are not written as rows).
- `pc_quad_stunt_state_pack` / `atv_quad_stunt_state_pack` are weak, as their bike twins are.
- Whether the quad stand-upright's guard (PC +0x1544 != +0x154c, as Ghidra shows it inside a function
  called from the Force listener) is the bike balance's pair the other way round depends on which `this`
  the call passes. Not checked.
- The quad update's tail (0.95, 0.73, 0.1) was not read.
- The PC `pc_*` names are not in this slot's Ghidra project (R12). `./kit fn pc_quad_… -o pc` will not find
  them until `./kit ghidra name -o pc` runs.
- `game_sim::phy::Phy::read` was not run on the ATV files. Their keys equal the bike's, which it reads.
  Whether the ATV needs more than `Phy` holds (four tires from two sections) is MD37b's to see in
  `atv_quad_tire_phy`.
- Which MX vs. ATV model names carry `ATV_S_` (so which quads ride the `atv_s_` files) was not looked up.

## Next

MD37a first: no ATV state exists, so every other part waits on it or on MD40.
