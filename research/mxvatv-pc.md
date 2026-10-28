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

### The bike's per-frame virtuals (R11)

Paired by walking the call graph out from the sure rows: a callee called at the same place and in the same
order by a function already paired is its twin, checked by its float constants and branch shape (31 `pc`
rows, `# R11` in oracle.txt).

The PC does not run the frame as one function. `bike_vehicle_update` is `MXVehicleMotoShared::vf14_at_12`
(ghost step first, then the wreck test, and for remote kinds 3/4 the branch PS2 takes on `+0x80c == 2`).
`bike_ground_step` is split over the physics world's listeners on `MXVehicleMotocross`:

| listener | PC | holds the PS2's |
|---|---|---|
| `vf0_at_48` PreProcess | 0x005f6900 | the pre-loop: the input limiter (vf36), front steer, the rear fade (±π/2), the 14.67/29.33 lerps, the auto shift, the clutch |
| `vf0_at_60` Force | 0x005f9950 | the substep body, in the PS2's order; no 55 Hz loop of its own (the physics world substeps) |
| `vf0_at_52` PostProcess | 0x005f9ab0 | `bike_air_state`, the ground step's 0.25 tail, the no-hander (once a frame) and an out-of-world wreck |

The substep's inline blocks are functions on the PC: the auto shift (0x005fb490), the air drag
(0x006012a0, `-k·½v²` along v, k from the `.phy` record, PS2 `+0x6a8`), the rear-wheel pitch torque
(0x005f6370), the attitude peak hold (0x00601330, PS2 `+0x550` → PC `+0x1270`). The player's overrides are
merged into the base slots: `bike_wreck_entry` + `player_wreck_entry` = vf43, `bike_unwreck` +
`player_respawn` = vf44. The ghost is three slots (vf52 off, vf53 on, vf54 step) with the put-back in
`MXVehicle::vf55`.

Differences seen (the journal has the detail):

- `bike_clutch`: the PS2's last term (chassis `+0x198` less `+0x6cc · lerp(+0x6a0)`) is not on the PC, nor in
  SLUS-21104: MX vs. ATV's change, not the port's (the lerp goes to the engine's `+0x7c` instead).
- `bike_wreck_test`: the same reasons through the same slot, plus 14 (no rider) and 15; thresholds 2.9/4.5;
  the PS2's 0.4, 4, 6, 100.3 and 1497.6 are not there.
- `player_wreck_entry`'s draws are `rand()/32768` on the PC, `rand·2⁻³¹` on the PS2. Action and stunt ids are
  renumbered (PS2 25/28/0x1c → PC 0x2e/0xff), and the race-record bit is 0x40000 (PS2 0x80000).
- `bike_no_hander` runs once a frame on the PC, each substep on the PS2 (which also zeroes the balance there).
- `wheel_steer_integrate`: the same math with fields 0xC lower, but the clamp tests `max < a` then `a < min`
  where the PS2 tests `a <= max` then `a < min` — the same but for NaN. `wheel_steer_stop` is inlined.
- PC 0x00600790 (35°, ±0.7, 5, 30), called beside the preload and the backflip, has no PS2 twin: likely MX
  vs. ATV's 360 (SLUS-21104 has it at the same place).

Not paired: the PS2's network-state unpack 0x00259150 (the PC's `Motocross::vf104` places the wheels from
the chassis instead), 0x00252270 (maybe PC vf91 0x005fad80, weak), 0x00252280, 0x00252fe0, and the
helicopter models. Weak: `pc_stunt_clocks` (145 bytes against 2716: the clocks only), `pc_stunt_state_pack`
(by purpose), `pc_bike_put_back`.

### Beside SLUS-21104 (MX vs. ATV Unleashed, PS2)

SLUS_211.04 is only the engine: the game is the overlay GAME_REL.BIN (the front end UI_REL.BIN), a
CodeWarrior "MWo3" image loaded whole, 64-byte header included, at 0x0045c580 into an empty LOAD segment,
with no relocations. `tools/mxvatv_overlay.py` folds it into the ELF so Ghidra sees one program; the
`[other.atv]` block in kit.toml.example names the result. 39 `atv` rows in oracle.txt (`# R11 — the same
functions in SLUS-21104`).

It is the PC's code built for the EE, not MX Unleashed's: the same five listener vptrs (at +0x30..+0x40,
each a `j slot; addiu a0,a0,-N` thunk), the same Pre/Force/Post split and call orders, the 360 function, the
PC's ids (0x2e/0xff) and race bit (0x40000), the PC's field layout with the high fields 0x20 lower (PC
`+0x1308` → `+0x12e8`; some blocks 0x14). The primary vptr is at +4. Where the PC and MX Unleashed part, it
tells which side moved:

| | MX Unleashed | SLUS-21104 | PC | so |
|---|---|---|---|---|
| clutch's chassis `+0x198` term | yes | no | no | MX vs. ATV dropped it |
| wreck-entry draws | `rand·2⁻³¹` | `rand·2⁻³¹` | `rand()/32768` | the PC's libc |
| steer clamp | `a <= max`, `a < min` | as MX Unleashed | `max < a`, `a < min` | the PC compiler |
| `wheel_steer_stop` | a function | a function | inlined | the PC compiler |
| ids, race bit | 25/28, 0x80000 | 0x2e/0xff, 0x40000 | as SLUS-21104 | MX vs. ATV |

SLUS-21104 multiplies with `madd.s`/`msub.s` where MX Unleashed has a `mul.s` then `add.s`, so even where
the math is the same its rounding is not MX Unleashed's (`openmx` follows MX Unleashed).

Weak here as on the PC: `atv_stunt_clocks`, `atv_stunt_state_pack`. The wreck test was paired by slot and
call site, not compared clause by clause.
