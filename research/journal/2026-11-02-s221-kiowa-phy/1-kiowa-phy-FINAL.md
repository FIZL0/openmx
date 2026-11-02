# 1 — kiowa phy (FINAL)

Task S221. 2026-11-02.

## What I did

- Read the helicopter's `.phy` reader (base: `./kit fn 38cb70 -p boot`; the tail with the wreck rays
  is easier in `./kit dis 0x38d6a0 0x38d8d4 -p boot`) and the MX override (`./kit fn 391990 -p boot`),
  with the four setters it calls (`./kit dis 0x390060 0x390100 -p boot`, `0x38f960`, `0x38f9d0`).
  The key map, the conversions and the setters are in oracle.txt's S221 block.
- Ported it as `game_sim::kiowa::Phy::read` (appended to `crates/game-sim/src/kiowa.rs`). It hands back
  the existing step structs as they are — `Rotor`, `Aero`, `Controller` — so the flight can be built off
  the disc rather than a save state. Next to them it keeps the keys no step reads yet: `MainRotor`,
  `TailRotor`, `AeroKeys`, the pelvis box. `[Mass Properties]` goes through `phy::Part::read`, and
  `[Wrecks]`' five plane keys through `biplane::Wrecks::read`: same keys, same order, same MPH on
  `MinWreckSpeed`. `Phy::crash()` gives the crash state's two file fields.
- The rotor's wreck collision is `WreckRotor`: eight **rays**, not spheres. S220a called them spheres
  and their far ends "centres" — what it calls a centre is the ray's far end. Each ray goes from the
  hub `(0, WreckRotorHeight, 0)` out to `WreckRotorRadius`, along ±x, ±z and the four diagonals. The
  diagonals use 0.707 per axis, which is not √½.
- `wreck_blade(bytes)`: `wreckblade.phy`'s `[PhysicsPart1]` through `Part::read`. The MX reader reads
  that section into each of its four blade parts.
- `wing_chord`: the aero's wing setter's single rule. With no chord and a positive aspect ratio, the
  chord is span / ratio. It is the same rule as `biplane_wing_geometry`.
- Two values the step needs come from the constructors, not from the file. The port keeps them as
  constants:
  - the aero model's air density, `0x3b1bc5c4`;
  - the controller's hold delay, 2.0.

## What the original does

The details are in oracle.txt. In short:

- Every float key defaults to 0. The one exception is `WreckRotorRadius`, which defaults to 1.
- Every quadword key defaults to (0,0,0,1).
- Angles are converted ×0.01745329:
  - the three main-rotor angles;
  - the tail's `MaxPitchAngle`;
  - `DefaultAngleOfAttack`;
  - `MaxPitchCommand` and `MaxRollCommand`.
- Speeds are converted ×1.46666:
  - `DragHighSpeed`, `LowSpeedAero`, `HighSpeedAero`;
  - `TurbulenceMagnitude`;
  - `SteadyWind`'s xyz, on the VU (`vmulx.xyz`, so w is kept);
  - `SpeedCommandRate` and the min and max speed commands;
  - the two feed-forward speeds;
  - `MinWreckSpeed`.
- `LowSpeedDrag` and `HighSpeedDrag` are not converted.
- The main rotor's `MaxAltitude` (1500) is a different field from the controller's (550).
- The MX reader adds `[Animation]`. It then opens `%sWreckBlade.phy` and reads `[PhysicsPart1]` into
  each of the four blades at heli+0xe48..+0xe54.

## How it was proved

`crates/game-sim/tests/kiowa_phy.rs`, run against S148's RAM image. The image exists in s1 and in s3.

- **`kiowa_phy_matches_the_live_helicopter`** is a save-state comparison of 213 words plus the rays.
  - The words are every field the reader writes:
    - the part's words, the same list the car's proof uses;
    - both rotors, every aero key, every controller key, the wrecks and the pelvis box;
    - the two constructor constants;
    - each of the four blades' part words, against `wreckblade.phy`.

    Each word is checked bit for bit against `Phy::read` of `machine.res`'s `helicopter\kiowa.phy`.
  - The rotor probe's eight rays are compared as a sorted set of bit patterns, because the probe keeps
    them in its tree's order: hub, then the far end, all lanes.
  - Mutants, each red:
    - `DEG` = 0.0174533: 6 words;
    - `MPH` = 1.4666667: 7 words;
    - hold delay 2.5;
    - `SteadyWind` scaled with w: the w word;
    - √½ for the diagonal: the rays.
- **`the_wing_setter_matches_the_original`** is a function oracle. It runs 10,000 generated cases
  through the original wing setter (`kiowa_wing_geometry`) on the live aero model, against
  `wing_chord`. Chords and ratios are often ±0 or negative, and more than 1,000 cases take the
  span/ratio branch.
  - Mutant: `<` for `<=` on the ratio fails at a −0 ratio.
- Still green: `kiowa`, `car_phy` and `biplane`, 37 tests (`./kit check -- -p game-sim --test kiowa
  --test kiowa_phy --test car_phy --test biplane`).

## Not verified / not 1:1

- **Many of the words compared are 0 in the file.** These are the aero `CL*` shape keys, `CLq`, `CD1`,
  `CYp`, `CYr`, `Clr`, `Cnp`, the pedal keys, `DragHighSpeed`, `HighSpeedDrag`, the turbulence, the
  wind, both rotors' `Inertia` and `MinCollective`. For those keys the comparison proves the key → field
  map only where the value is not 0, and the order of the conversions not at all. The map comes from
  the disassembly's string addresses, one by one.
- **The body's inertia is not ported.** The file's `[Mass Properties] Inertia = 0.2, 0.2, 1.0` is not a
  key that the part reader takes. The body's tensor comes from the model's box, as every part's does
  (`Part::inertia` needs the box). The reader copies that tensor to heli+0x410, and the port does not
  keep it.
- **Two vtable calls in the base reader are not identified or ported:** the body+0x100 object's slot
  +0x2c, and the collision record's slot +0x74. Both run between the wreck keys and the rays.
- **The `.phy` reader is not wired into the shell or the flight yet.** Nothing builds a `Kiowa` from
  `Phy` outside the test. P216 (parked and switchable) is where that belongs.
- **The PC exe's reader (vf12 0x005ef470) was not read branch for branch.** S148 recorded that it has
  the same keys in the same order.
- **The S220a naming is left as it was.** `rotor_in_ground`'s `centres` are the rays' far ends. Its docs
  say "spheres", and I did not rename code I did not come for.
- **`./kit audit` fails on two lines in plan/DONE.md that are not mine:** 519 `bike_airborne_control`
  and 533 `atv_part_read`.

## Next

Nothing for this task. The next step is to build a `Kiowa` from `Phy` where the shell parks the
helicopter (P216).
