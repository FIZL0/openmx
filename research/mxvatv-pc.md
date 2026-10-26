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

## Class map (R10)

The exe has no symbols, but MSVC left RTTI for all 1886 polymorphic classes and `.\File.cpp` assert
strings in 90 source files. `ghidra_scripts/NameOther.java` names the program from both:

    ./kit ghidra analyse -o pc     # import + analyse (~3 min) into context/ghidra/pc.gpr; runs NameOther
    ./kit ghidra name -o pc        # NameOther again (seconds): after an oracle.txt edit
    ./kit ghidra dump -o pc        # context/decomp/pc.c, pc.funcs.tsv, pc.xrefs.tsv
    ./kit fn MotocrossRider::vf44 -o pc · ./kit fn pc_engine_update -o pc · ./kit xref pc_wheel_suspension -o pc

`[other.pc]` in kit.toml names the file (kit.toml.example has the block). NameOther also writes
`context/decomp/pc.classes.tsv` (class, vtable, offset in the object, slots, bases),
`pc.vtables.tsv` (every slot of every vtable) and `pc.sources.tsv` (functions per source file).

Names it gives:

- `Class::vfN` — slot N of the class's primary vtable. vf0 is always the scalar deleting destructor.
  `Class::vfN_at_<off>` is slot N of the vtable at byte `<off>` in the object (a second base: an interface,
  a listener). A slot several vtables share goes to the class with the fewest bases, so an inherited method
  keeps its base's name: `MotocrossRider`'s slot 5 is `GameObject::vf5`, and
  `pc.vtables.tsv` lists a class's slots whoever owns them.
- MSVC folds identical functions: a trivial shared body (return 0, an empty method) carries whichever
  class's name came first, so `MXVehicleCar::vf57` sitting in the motocross vtable means nothing.
- `File_cpp_<addr>` — a non-virtual function that asserts with that file name. Every function reading a
  file name has a `source: File.cpp` plate comment, virtual or not.
- `pc_<name>` — a row of `research/oracle.txt` with program `pc`: a match made by hand to the PS2 function
  `<name>` (the row's note says how). These are the user's names in Ghidra and win over the two above.

### The classes the port cares about

| class | bases | slots (own) | notes |
|---|---|---|---|
| MXVehicle | Vehicle → PhysicsObject → SearchableGameObject → GameObject → BaseObject, plus 12 interfaces (NetworkObject, Actor, RiderInterface, CameraInterface, VehicleParticlesInterface, …) | 84 (76), 15 vtables | every machine; slot 37 is the particle wiring |
| MXVehicleMotoShared | MXVehicle, MotocrossSoundInterface | 111 (88) | motocross and ATV; slot 12 reads the `.phy` |
| MXVehicleMotocross | MXVehicleMotoShared | 111 (24) | slot 85 attaches nodes, 87 the stunt specials |
| MXVehicleAtv | MXVehicleMotoShared | 112 (29) | slot 87 is a copy of the bike's |
| MXVehicleCar | MXVehicle | 93 (50) | parent of the buggy, golf cart and monster truck; MXCarEngineModel / MXCarTireModel |
| MXVehicleHelicopter | MXVehicle | 89 (33) | HelicopterAeroModel, HelicopterEngineModel |
| MXVehicleBiplane | MXVehicle | 90 (41) | 12 reads the `.phy`, 33 is init, 86 finds the bones |
| MotocrossTireModel, MotocrossEngineModel | GameObject | 31 (1) | only the destructor is virtual; the work is plain calls from the vehicle |
| BiplaneAero/Engine/TireModel | GameObject | 31 (1) | likewise |
| MotocrossRider | PhysicsCharacter, Actor | 60 (20), 3 vtables | the rider as its own physics character (MX Unleashed keeps rider animation on the vehicle) |
| MXMotocrossRider | MotocrossRider + camera interfaces, NetworkObject | 61 (12) | the game's subclass |
| PhysicsCharacter | Character → GameObject, ForceListener | 53 (13) | ragdoll pins (vf42), collection attach (vf45) |
| MotionActionController | none | 3 (3) | vf1, vf2: not paired with the PS2's motion controller |

There is no motocross aero model class: the bike's air drag is in the vehicle, as on the PS2.

### Matching to the PS2 (SLUS-20767 boot)

The 59 `pc` rows in oracle.txt, found three ways:

1. **Strings** — a function and its PS2 twin read the same rare strings (`.phy` keys, clip names, file
   names). Sure where many are shared (`pc_bike_phy_load`: 112 keys).
2. **Struct offsets** — the Jaccard overlap of the offsets ≥ 0x40 both touch, with the float constants as
   a tie-break. The tire model's offsets are mostly the PS2 wheel's (+0x210, the per-surface arrays at
   +0x238/+0x438/+0x778, +0x838, +0x8a8, +0x8b4) but its low fields are 0xC lower (PS2 +0x68 → PC +0x5c);
   the engine model's are all 0xC lower. A score under ~0.4 is marked weak in the row.
3. **Sequence** — the engine and tire setters sit in the same order in both, so a setter between two sure
   ones is its twin.

Differences seen, not chased: `wheel_set_tire`'s spin inertia is `m·r²/12·0.5` on the PC and `2π·r²`-scaled
on the PS2; the engine takes two driven wheels.

Not paired: the bike's per-frame virtuals (`bike_vehicle_update`, the input limiter, the wreck test and
entry, unwreck, the stunt and ghost slots). The vehicle's layout changed (15 vtables, a rider object of its
own), so offsets do not carry over and they share no strings; pairing them needs a call-graph walk from the
sure rows above. Nor `wheel_integrate`, `wheel_steer_integrate`, `wheel_steer_stop` (PC 0x0046fe60 clamps
the steering and may be the last), the helicopter models, or anything against SLUS-21104.
