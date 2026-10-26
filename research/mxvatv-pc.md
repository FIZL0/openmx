# MX vs. ATV Unleashed, PC (Steam) — a research reference

User, 2026-10-26: use the PC exe and the game's files for research, to make the port's methods closer to
what the original does; add the ATVs and the other machines; a track editor of our own later (Stretch).

## Where it is

`context/mxvatv-pc/` (git-ignored): the Steam install, a symlink on the user's machine, a copy on
omarchy-corsairai. `MXvsATV.exe` (3.8 MB), `Unleashed Editor.exe`, `data/*.pak` (63 paks, 20,415 files),
`media/` (the editor's: `.pco` props, PNG previews, `hiTexture/*.dxt`), `Docs/` (manuals, PDF).

`tools/mxvatv_pak.py ls|x|check` reads the paks: u32 count, count × {name[100], u32 offset, u32 size}
(offset from the end of the table), data. Every pak's last member ends at the end of the file.

## What it is, and what it is not

- **Sim data is the PS2 disc's, byte for byte.** Checked 2026-10-26 against SLUS-21104's `MXVSATV.PSR`
  (openmx-mods `discx`): all 34 `Veh\*.phy`, both `Rider\Anim\*.phy`, the five `Rider\Anim\*.ini`
  (stunts) and 79 of 81 `Rider\Anim\MX_*.mot` (v7) are identical; the other two were a filename with a
  space the compare script tripped on and one clip not found on the PS2 side (not rechecked).
- **Render data is not.** `.dxt`/`.dxm`/`.dxg`/`.dx.trn` in place of the PS2's `.PST`/`.psm`/`.psg`/
  `.ps2.trn`; Nat01's detail texture is 512×512 here. Not surveyed further.
- **The exe is the gain.** Unpacked MSVC x86, no symbols, but:
  - MSVC RTTI: 1014 class names (`.?AV…@@`), so vtables are named — function-pointer tables a static
    search of the PS2 ELF cannot follow (V61's stop) come out by class and slot.
  - Assert strings with 74 source files (`MotocrossRider.cpp`, `Motnctrl.cpp`, `MotionLibrary.cpp`,
    `MathCompress.cpp`, `PhysicsCollection.cpp`, `CollisionModel.cpp`, …).
  - Every machine as a class: `MXVehicleMotocross`, `MXVehicleMotoShared`, `MXVehicleAtv`,
    `MXVehicleCar` (`MXVehicleBuggy`, `MXVehicleGolfCart`, `MXVehicleMonsterTruck`),
    `MXVehicleHelicopter`, `MXVehicleBiplane`; models `MotocrossTireModel`, `MotocrossEngineModel`,
    `MXCarTireModel`, `MXCarEngineModel`, `PhysicsCarProcedural`, `HelicopterAeroModel`/`EngineModel`,
    `BiplaneAeroModel`/`EngineModel`/`TireModel`, `BalloonPhysics`, `PhysicsCharacter`.
  - `MXUITextTogglePhysicsModel`: the game's own physics-model choice (the `.phy` A/P pairs).
  - `XboxParticleSimulation`, `XboxVertexBuffer`: ported from the Xbox build, not the PS2's.
- **It is not an oracle.** Its float is x87 (≈1,900 `fmul` against 115 `mulss`), and it is a later game on
  another platform. It says what the code does — every branch, clause, constant and call order — and the
  PS2 ELF and PCSX2 captures still say what the numbers are. Bit-exact proofs stay against SLUS-20767
  (MX Unleashed) or SLUS-21104 (MX vs. ATV), never against the exe.

## How it makes the port more accurate

The exe reads like source next to the PS2 decompilation: the same engine (openmx-mods `notes/tricks.md`:
MX vs. ATV's air control, backflip and 360 are MX Unleashed's steps with new `.phy` numbers), compiled
for a CPU Ghidra decompiles well, with class names on it. So for a function the port is matching:

1. Find it in the exe by class/vtable/assert, read every branch there, then match it to the PS2 ELF.
2. Write the oracle's generated inputs and the captures to cover each branch the exe shows, instead of
   recording many rides and hoping one hits each case. Fewer, aimed captures; the proof is still the PS2's.
3. A clause the exe has that the port lacks is a finding; file it.

## Machines

Machines in the data: motocross (50/125/250/500, Race/Pro × A/P), ATV (`atv_*` and `atv_s_*`, Race/Pro ×
A/P), baja, buggy, sandrail, golf cart, monster truck (car model), kiowa (helicopter), biplane. MX
Unleashed has the biplane (ported), and a monster truck and a helicopter (P87). The rest come from MX vs.
ATV and ride as additions: off by default, every replay hash unchanged with them off (AGENT.md).
