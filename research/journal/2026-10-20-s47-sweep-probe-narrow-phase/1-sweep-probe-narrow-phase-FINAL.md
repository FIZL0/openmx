# 1 — sweep probe narrow phase (FINAL)

Task S47. 2026-10-20.

## What I did

- `crates/game-sim/src/sweep_probe.rs`: the sweep probe (a collision record's +0x1f4, kind 3) as
  `SweepProbe { segments, world, local, answer }`, with
  - `clear`: the vt +0x90 clear. Each segment's two ends go through their nodes when they have one. The
    world bundle gets start, unit direction and length per segment (the zero quad when the length is not
    > 0). The box over every end gives the probe's sphere. Then the answer is reset: head quads to the
    origin, nearest 1, tally 0.
  - `pair`: the vt +0x94 pair test, which is sphere_collide_pair underneath. It refuses a missing
    candidate or one without its nearby bit. It clears the hit and owner words, makes the sphere test
    (fpu dot strictly below the squared sum of the radii), then resets nearest, the fractions (-1) and
    the tally. Then:
    - a mesh (kind 0): every segment goes into its space through an inverted transform and through S4's
      `collide::sweep`. Each hit comes back out (point and normal) with the struck collidable, and so
      does the nearest normal, point and push-out.
    - a hierarchy (kind 1): the same, part by part. A part takes a hit only on a segment whose owner word
      is still 0, and sets it to 1. The nearest is taken back out through every part that took a hit,
      once per such part.
    - anything else: false.
  - `Shape::Other` for kinds 2, 3 and the unknown ones. Kind 8 is not ported (below).
- `crates/game-sim/tests/sweep_probe.rs`: the oracle proof.
- research/oracle.txt: an S47 block (layouts, the globals, what +0xb0 really is).
- Filed **S49** for the +0xb0 grid pass, kind 8 and the smoothed normals.
- Derived data: this worktree's kit.toml reads the main checkout's context. That checkout had no
  `states/biplane.p2m2_SaveState.eeMemory.bin`, so `./kit state ram context/recordings/biplane.p2m2_SaveState.p2s`
  wrote it there (git-ignored, the same bytes as slot 1's copy). Without it, S42–S47's tests skip.

## What the original does

`./kit dis 1e88d0 1e8d40` (the clear), `./kit dis 1e8d40 1e8e00` (the pair: it stores probe+0x164 in a
global, then tail-calls `./kit fn 1e9540`), and `./kit fn 10f460` for mesh_sweep (S4's port).
Probe layout:

| Offset | Holds |
|---|---|
| +0x120 | count |
| +0x124 | node pointers, two per segment |
| +0x128 | ends, 32 bytes per segment |
| +0x12c / +0x130 | local starts / local directions |
| +0x134 | lengths |
| +0x138 / +0x13c | world starts / world directions |
| +0x140 | local centre |
| +0x150 | world centre, copied to +0x40 |
| +0x160 | radius, copied to +0x60 |
| +0x164 | the smoothing switch |

A node's world matrix sits at node+0x80. The answer (+0x6c) follows collide::Record's layout. Its
+0x9c is the collidable struck by each segment.

The quirks, all kept:
- every candidate starts over: nearest 1, fractions -1, tally 0. A second pair call wipes what the first
  one found, unless the first returned false at the sphere test or the nearby bit.
- the hierarchy's nearest is taken out through every part that hit.
- the local lengths are the world ones, so a scaling transform would change the fraction's meaning.
- the inverse's transposed off-diagonal entries go through a `vadd` with zero, which flushes -0 and
  denormals. The diagonal and the w entries do not.

The slot named sphere_probe_prepare (+0xb0) is no prepare: it is a pass over a cell grid. See the
oracle notes.

## How it was proved

Function oracle on biplane.p2m2_SaveState, `tests/sweep_probe.rs`: 10 000 cases with 19 984 pair calls
against the save state's 34 gathered candidates (26 meshes and 8 hierarchies of 2 meshes each).

Each case gives the probe 1–16 segments (156 cases with all 16). Most segments aim through a random
triangle of a candidate (or a little off it) from either side. 12 493 segments have an end on a
near-identity node, 1 293 have no length, and 1 case in 10 is moved far off so the spheres refuse. The
answer is filled with leftovers first.

Then the original's clear, and 1–3 pairs on both sides. Bit-exact on:
- every return
- every world and local start, direction and length
- the centre, radius and local centre (and the collidable's +0x40 and +0x60)
- the clear's bit 0x20
- the whole answer: both head quads, nearest normal, point and push-out, the tally, the nearest
  fraction, and per segment (all 16) the fraction, point, normal, hit word, owner word and the
  collidable struck

Result: 5 311 pairs struck, 1 266 of them hierarchies, and 4 725 segment hits.

Mutants: killed dropping the hierarchy's owner check, fractions reset to 0 instead of -1, and the box
leaving out the start. Survived: `<` for `<=` in the sphere test, and a plain copy for the `vadd`
flush in the inverse (both listed below).

## Not verified / not 1:1

- **The +0xb0 slot (the cell-grid pass) is not ported.** The plane's search+0x38 is 0 in the save
  state, so the slot does nothing there, and no state with a grid is known. world_search::Narrow::prepare
  also lacks the grid argument the original passes. Filed as S49.
- **Kind 8 (a finite plane) is not ported**, because none is among the save state's candidates. Filed
  as S49. `Shape::Other` returns false for it, which would be wrong if one turned up.
- **Smoothed normals (probe+0x164 ≠ 0, with a mesh that has per-vertex normals) are not ported.** The
  plane's +0x164 is 0 and the test asserts that. S4's collide::sweep has no smoothed path. Filed as S49.
- **The sphere test's strictness is not pinned.** A random case never lands exactly on the boundary, so
  `<=` survives. It was read as `c.olt.s` with `bc1f` to the refusal in the disassembly, and a proof would need a hand-made tie.
- **The `vadd`-with-zero flush in the inverse is not pinned.** None of the save state's transforms has
  a -0 or a denormal off-diagonal entry, so a plain copy survives. It was ported from the instructions.
- **The clear's two global stores (zeroing two words with no other xrefs) and the store of the answer's
  address in a global are not ported**, because the port has no globals. The test does not check them.
- **Packed-vertex meshes are not exercised**, because all 34 candidates' meshes are wide. The mesh path
  is S4's collide::sweep, proved there.
- **Node transforms are synthetic** (near-identity), not the plane's own nodes. The plane's sweep
  segments in the state have null nodes.
- The glue to world_search::Narrow, and where the segments come from in the ride, belong to S48.

## Next

S48 wires this into the ride through world_search::Narrow. S49 holds what is listed above.
