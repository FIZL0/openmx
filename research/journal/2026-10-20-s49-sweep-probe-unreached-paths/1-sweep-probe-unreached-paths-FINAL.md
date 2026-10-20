# 1 — sweep probe unreached paths (FINAL)

Task S49. 2026-10-20.

## What I did

- Followed each of S47's three unported paths back to whatever switches it on. None of them can run on this disc,
  so I **ported none of them**. That is the decision this entry records, with the reasons below.
- Added `crates/game-sim/tests/sweep_probe_reach.rs`, which re-proves this from the user's own disc and save
  states. It has three tests:
  - no `.scn` has a terrain-grid section, and no archive holds a `.tcg`;
  - no `.col` has type 8;
  - in every save state, no sweep or ray probe has its smoothing field set, the global flag is clear, the level
    has no grid, and no finite-plane collidable exists.
- Updated the docs: the `sweep_probe` module header, `Shape::Other` and `world_search::Nearby` now say why these
  paths are absent. `Nearby::prepare` stays a bool, because the grid argument the brief asked for is always null.
- Added an S49 block and eleven names to `research/oracle.txt`.

## What the original does

**The grid pass** (`./kit fn 0x001e93e0`, the sweep probe's class slot +0xb0):
- collide_against_search calls it with search+0x38. The ray probe's 0x001ebd20 and 0x001f1670 have the same
  shape.
- The search constructor (`./kit fn 0x002188d0`) zeroes +0x38. Of the 12 places that build a search, only two
  write it again (`sw $v, 56(...)` right after the constructor at 0x00159bdc and 0x001a5390):
  - the physics-object init 0x001597b0, which builds its own search at +0x11c, gated on +0x124;
  - 0x001a5210.
- Both copy `level+0x42c` (the level pointer is the word at 0x0048a568).
- Only `./kit fn 0x001a33e0` writes that field. It is called from the level load 0x001a2c30 when the level's
  `.scn` (`%s%s.scn`) has a `SoultreeTerrain` section with a `File`; the loader replaces the name's extension
  with `.tcg`.
- The grid's constructor 0x003b4470 has no other caller.
- The plane's vtable sits at plane+0x48, so that init's fields are plane+0x164/+0x16c/+0x174. In the biplane
  state, +0x16c is the collision object's own nearby search (0x0021e350 built it, and never set +0x38).

**Kind 8:**
- `./kit fn 0x003db9e0` builds the finite-plane collidable: vtable 0x0045e9d0 at +0x38, kind 8, planes at +0x120
  (0xd0 bytes each), count at +0x124.
- Its only caller is the collision-file loader `./kit fn 0x0011b230`. That function reads `<name>.col`, checks
  the header (1234, "col", version 1 or 2) and dispatches on the type word at +12; type 8 builds a finite plane.
- 0x003dc030 is the segment test against those planes. The 0x003dc490.. functions next to it belong to a sound
  emitter (vtable 0x0045eb40) used by the biplane, not to collision.

**Smoothed normals:**
- The flag 0x00468250 has exactly two writers: sphere_probe_pair (0x001e8d4c, from probe+0x164) and
  ray_probe_pair (0x001eb52c, from probe+0x174).
  - Found by a scan of every store in the code whose immediate is 0x8250 behind a `lui 0x47`, plus every
    `$gp+0xae0` store (there are none).
  - Ghidra misses both stores: sphere_probe_pair has no function.
- The sweep probe's constructor 0x001e80f0 zeroes +0x164.
  - None of its 17 construction sites writes +0x164.
  - None of its class slots (vtable 0x00451910) is among the writers.
  - Writers ruled out as other classes: all 94 decompiled `+ 0x164) =` lines, which include font glyph state
    (0x001556b0, 0x00155c50), quads at +0x160, and the 0x001ea830 collidable at bike+0x7c4.
  - Of the 90 raw store instructions at +0x164, the two outside any Ghidra function are a `$sp` slot
    (0x1e9dd4) and a quad copy (0x159ee0).

## How it was proved

- **Disc** (the user's own ISO, read at test time):
  - all 58 `.scn` members of every `.res`: none contains `SoultreeTerrain` (case-folded);
  - no member is named `*.tcg`;
  - all 4 220 `.col` members have a valid header and none has type 8. By (version, type): (1,0) 173, (1,2) 235,
    (2,0) 3198, (2,1) 21, (2,2) 593.
- **Save states:**
  - In the test: the biplane state (64 sweep probes, 47 ray probes) and first_recording (91 / 54). In both, every
    probe's field is 0, the flag is 0, the level's +0x42c is 0, and no word anywhere in RAM equals the
    finite-plane vtable.
  - By hand with the same scan: p23/brake1, p23/brake2, p23/recording2 and v15/play give the same result.
- **The tests really run.** With every count raised past what the data has, all three tests failed and printed
  their counts.
- `./kit check` passes.

## Not verified / not 1:1

- **None of the three paths is ported.** The grid pass, the finite-plane pair test and the smoothed-normal sweep
  (0x0010f2a0, 0x0010ea00, 0x0010db20) are all absent. A mod that adds a `.tcg` grid or a type-8 `.col`, or code
  that sets the field, would reach behaviour the port does not have. `Shape::Other` returns false for a kind-8
  candidate.
- **"Nothing sets +0x164" is a code review plus six states, not a proof.** I read each store in its own context,
  with no data-flow analysis. A write through a pointer of another type, or by code outside the boot ELF, would
  not have been seen. The states are from only two levels (the biplane's and the bike tracks').
- **The `.scn` check is a byte search**, not the game's ini parser. A section name the parser would accept in
  some other spelling (for example with a different case) is caught, because the search folds case. Spellings
  further from the name are not.
- **The loader's in-memory path is not checked.** It reads the type from buffer+4 of an already-loaded file, and
  I did not confirm which header layout that buffer has. Either reading of it gives the same type word, so no
  type 8.
- **Nothing was proved against a running emulator.** No live capture was made.
- The 0x001a5210 search's owner (a placed object with a +0x528 flag) was not identified. It gets the same zero
  grid either way.

## Next

None for S49. S48 can wire `world_search::Nearby::prepare = false` with confidence.
