# 1 — mxvatv pc ghidra (FINAL)

Task R10. 2026-10-26.

## What I did

- Kit: `[other.<name>]` tables in kit.toml (`ps2kit/config.py` `Config.others`) for programs read for
  reference only. `./kit ghidra analyse|name|dump -o pc` and `./kit fn/xref … -o pc`. The PC exe goes
  into its own project, `context/ghidra/pc.gpr`, so an analysis does not hold the PS2 project's lock.
  `decomp.dump` also takes any program whose dump file exists, so a worktree or main checkout whose
  kit.toml lacks the block can still read `pc`. DumpDecomp/DumpXrefs now write namespaced names
  (`MotocrossRider::vf44`); for the PS2 programs, whose names are all global, this changes nothing.
- `ghidra_scripts/NameOther.java` (the import's post-script, or `./kit ghidra name -o pc` again):
  1. It decodes MSVC RTTI from the raw image (complete object locators → type and hierarchy
     descriptors): 1886 classes, 1886 vtables.
  2. It names each slot `Class::vfN[_at_<off>]`, creating 2238 functions the analyser had missed.
  3. It tags the functions that read 87 of the 90 `File.cpp` assert strings and names the default-named
     ones `File_cpp_<addr>`.
  4. It applies oracle.txt's `pc` rows as user names.
  5. It writes `pc.classes.tsv`, `pc.vtables.tsv` and `pc.sources.tsv`.
- research/mxvatv-pc.md: a class map section covering the hierarchy, slot counts, the naming
  conventions, the matching method and what is unpaired. docs/GHIDRA.md: a short section on `-o`.
- research/oracle.txt: an R10 section with 59 `pc_<ps2 name>` rows:
  - engine 8
  - tire 18
  - rider 3
  - vehicle virtuals by string 11
  - biplane models 7
  - engine-wide 12

  The `pc_` prefix keeps `addr("engine_update")` on the PS2 row.

Acceptance: `./kit fn MotocrossRider::vf44 -o pc` prints the function under the default (main) KIT_CONFIG,
and so do `./kit fn pc_engine_update -o pc` and `./kit xref pc_wheel_suspension -o pc`.

## What the original does

- The engine and tire models are their own GameObject classes, with only the destructor virtual. The
  vehicle calls their work directly, as the PS2 does with its wheel/engine blocks.
- MotocrossEngineModel's fields sit at the PS2 engine's offset − 0xC.
- MotocrossTireModel mostly keeps the PS2 wheel's offsets: +0x210, the per-surface arrays at
  +0x238/+0x438/+0x778, +0x838, +0x8a8, +0x8b4. Its low fields are 0xC lower (PS2 +0x68 → PC +0x5c).
- The rider is a MotocrossRider (PhysicsCharacter + Actor). In MX Unleashed the rider's animation code
  sits on the vehicle. MotocrossRider.cpp asserts appear in PS2 `vehicle_anim_build` and
  `vehicle_add_anim`, and in two PC functions (`./kit xref` on the string).

## How it was proved

This is matching by hand, not a function oracle: the PC exe is x87 and a later game, so nothing is called.

- **Strings:** a rare string read by at most 3 functions in each program and shared by both.
  `pc_bike_phy_load` shares 112 `.phy` keys with `bike_phy_load`.
- **Offsets:** the Jaccard overlap of the struct offsets ≥ 0x40 each function touches, plus its float
  constants. Each row's note gives the score.
- **Sequence:** the setters sit in the same order in both programs.

The scripts are throwaway scratch (`strmatch`, `offs`, `cmap`) and are not committed. The method is in
mxvatv-pc.md.

## Not verified / not 1:1

- The PC exe is not an oracle. Every `pc` row is a reading aid; a port proof still runs against the PS2.
- Weak matches are marked "weak" in their notes. A score under ~0.4, or a pair with few offsets, could be
  a neighbour rather than the twin:
  - `pc_wheel_contact_frame`, `pc_wheel_contact_velocity`, `pc_wheel_hinge_force`
  - `pc_biplane_tire_preforce` (2 offsets), `pc_biplane_gear_springs`
- `pc_engine_update` is matched by place and size only.
- `wheel_set_tire`'s inertia formula differs between the builds. I did not chase which is used where.
- Not paired:
  - the bike's per-frame virtuals (vehicle update, input limiter, wreck test/entry, unwreck, stunt and
    ghost slots): the vehicle layout changed, and they share no strings
  - `wheel_integrate`, `wheel_steer_integrate`, `wheel_steer_stop`
  - `biplane_tire_force`, `biplane_tire_strut`, `biplane_tire_friction`, `biplane_tire_frame`,
    `biplane_engine_ctor` (no PC function scored)
  - the helicopter models
  - MotionActionController
  - Filed as follow-up R11.
- Only SLUS-20767's boot was compared. The `game` overlay and SLUS-21104 (MX vs. ATV PS2) were not.
- RTTI was decoded by NameOther, not by Ghidra's RecoverClassesFromRTTIScript. Data members and
  structures are not recovered.
- A vtable is taken to end at the first non-code dword or the next referenced address. A vtable followed
  directly by another referenced table could be cut short; none was seen.
- 3 of the 90 assert file names have no referencing function found, so they are untagged.
- The main checkout's kit.toml (git-ignored, and what KIT_CONFIG points at) lacks `[other.pc]`. `fn`/`xref
  -o pc` still work from the dumps, but `./kit ghidra … -o pc` needs the block copied from
  kit.toml.example, or `KIT_CONFIG=$PWD/kit.toml` in a slot that has it (s1 does).
- `./kit done` used `--skip-audit`: the only leaks are other tasks' hex in plan/DONE.md (lines 121 and
  395). This task's own leak, in a Java comment, was fixed.

## Next

R11: pair the bike's per-frame vehicle virtuals by walking the call graph out from the sure rows
(`pc_wheel_suspension` is called by MXVehicleAtv::vf70 and its motocross twin), then compare against
SLUS-21104.
