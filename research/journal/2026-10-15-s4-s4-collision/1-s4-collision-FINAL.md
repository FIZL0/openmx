# 1 — s4 collision (FINAL)

Task S4. 2026-10-15.

## What I did

- New module `crates/game-sim/src/collide.rs`: the original's shape test against a world mesh, the sweep
  that carries a bundle of probe segments down the mesh's tree, the way a hit resolves into a push-out,
  and the two limits the sweep allows itself (16 probes in a bundle, 256 levels of descent). Float lint
  on, everything through `ps2-float`'s `F`/`Vf`.
- New proof `crates/game-sim/tests/collide.rs`: 10,000 generated scenes through the original's own sweep
  in the oracle, compared field by field including every `w` lane; plus the vertex fetch (both widths)
  and the probe derivation, 10,000 samples each; plus one disc-free test of the test's own rules.
- Recorded the addresses and the in-memory layouts in `research/oracle.txt` (the kit's `[symbols]` is
  git-ignored, so that file is the committed copy the proof resolves names from), and a code-map line in
  `PLAN.md`.

## What the original does

The bike's collision goes through a base object plus a broad phase (`./kit fn 0x11a9d0`, gather at
`0x218b80`, 256 candidates) into one of several shape classes, keyed at `obj+0x68`, each with its own
per-pair test in the vtable at `obj+0x38` (+0x90 clear, +0x94 test, +0xa8 prepare). The class this task
ports is the probe bundle against a mesh: `./kit fn 0x10f460` is the thunk, which shuffles its arguments
(`0x468a78` → `a3`, `a3` → `t0`, `t0` → `t1`, `t1` = 0) and calls the recursive level, `./kit fn 0x10cfa0`.

One level does, in order:

1. A box rejection of the whole bundle against the node: `|centre - bundle.centre|` per axis against the
   node's half extent plus the bundle's radius. Out on any axis and the branch is skipped whole.
2. A per-probe box rejection, walking the probes in order. The first probe that still reaches the node
   makes the level recurse into both children; every probe rejected before it is struck out of the live
   mask for everything below. The mask is 0x40 bytes — 16 words, one per probe — copied from the parent
   into the scratchpad (`0x70000000`) once per level, which is where both limits come from: 16 probes per
   bundle, and 16 KiB / 0x40 = 256 levels before the copy would run off the scratchpad.
3. At a leaf (child word's top bit set), the triangle's three corners are fetched (`./kit fn 0x1080d0`;
   wide floats, or shorts scaled by `32767.0 * (1/32767 * short)` in that order) and tested against each
   live probe.

The test itself is a plane crossing followed by a containment test done in two dimensions: the triangle's
normal is the edge cross product, the crossing fraction is `t / length`, a probe travelling along or away
from the front face misses (the back of a triangle is not solid), and containment flattens onto the two
axes where the normal is weakest and compares one edge ratio. The resolution is
`push = normal * dot(dir * (1 - fraction), normal)`: the travel the probe did not get to do, projected on
the normal, in fractions of one unit because `dir` is unit length — so it points *into* the surface and a
caller subtracts it. Per probe the record keeps the nearest fraction, point and normal (+0x94/+0x98/+0xa0
/+0xa4/+0xa8 arrays), a hit tally at +0x8c, and the nearest-overall normal, point, push-out and fraction
at +0x20/+0x30/+0x40/+0xb0 (+0xb0 cleared to 1.0).

The one detail that costs a day if missed: every vector helper in this family writes `.xyz` into a
register it loaded from its *first source*, so the `w` lane is inherited sideways rather than computed —
`point.w` is `dir.w`, a cross product's `w` is its first edge's, `push.w` is the normal's.

## How it was proved

Function oracle, `crates/game-sim/tests/collide.rs`, four tests, all passing:

- `the_mesh_sweep_matches_the_original` — 10,000 generated scenes (median-split BVH of up to a few dozen
  triangles, both vertex widths, bundles of 1..16 probes, a quarter of the scenes with shrunken boxes and
  a fifth with a small bundle radius; half the probes aimed through a barycentric point of a real
  triangle so hits actually happen). Each scene is written into RAM, swept by the original at `0x10f460`,
  and every record field compared bit for bit against ours: 84,331 probes swept, 10,664 triangle hits,
  5,037 scenes resolving to a push-out, 71.6M instructions of the original.
- `the_vertex_fetch_matches_the_original` — 10,000 fetches, wide and packed.
- `a_probe_is_derived_the_way_the_original_derives_one` — 10,000 samples through the original's length and
  divide-by-scalar helpers.
- `the_shape_tests_own_rules` — no disc needed: one-sidedness, a corner in and a point past the diagonal,
  the half-unit push-out, the hit tally over two leaves, and `MAX_PROBES == 16`.

`./kit check` → TESTS PASSED, 302 tests.

## Not verified / not 1:1

- **The bike's wheels do not use this class.** They go through the ray class (`./kit fn 0x1ebe70`) and a
  dual-tree descent (`0x109130`, `0x108980`, `0x106000`, `0x1079d0`, `0x108340`, `0x10b980`) whose own leaf
  test is 5.6 KB and unread here, and whose resolution combines per-contact push-outs by min and max
  rather than by nearest. Ported: the probe-bundle class and its test. Not ported: the ray class, the
  sphere class (`0x1e9540`), the class-8 finite-plane case, and the broad phase. The brief asked for "the
  original's shape test and its sweep" — this is one of them, proved; the others are each at least a task.
- **The prepare step is not proved.** A bundle's centre and radius are derived in `0x1e88d0`; only the
  per-probe length and direction helpers it calls are proved here. The sweep is proved against whatever
  centre/radius it is given, so a wrong derivation would not show up.
- **`MAX_LEVELS = 256` is a modelled stop, not an observed one.** At 256 the original's next mask copy
  would write past the end of the scratchpad; we return instead. No generated tree got within an order of
  magnitude of that depth, so the limit is read off the scratchpad's size, not measured.
- **Out-of-range indices.** A node or triangle index past the end of its array returns here; the original
  would read whatever is there. Not reachable from well-formed data, and the proof never generates it.
- **Smoothed contact normals are not ported.** The descriptor can carry per-vertex normals, and there is a
  family that blends them (`0x10f2a0`, `0x10ea00`, `0x10db20`, global `0x468250`). The proof always leaves
  the descriptor's normals pointer at 0, so that whole path is untested and the module does not have it.
- **The in-memory node and triangle layout is not the on-disc one.** `track.rs` parses the file form; this
  module reuses its types against the memory form (node 0x24 bytes, triangle 0x14). The field-by-field
  mapping between the two is D6's and is not re-verified here — what is verified is that our reader and
  the original's reader agree on the memory form.
- **Packed vertices** are proved bit for bit, which settles the scale order D6 left open; the flag's other
  values (anything but 0 and the packed one) are not exercised.

## Next

Nothing for this task. Whoever takes the wheels next wants the ray class above: start at `./kit fn
0x1ebe70`, and the dual-tree descent is the same mask trick in the scratchpad, two trees deep.
