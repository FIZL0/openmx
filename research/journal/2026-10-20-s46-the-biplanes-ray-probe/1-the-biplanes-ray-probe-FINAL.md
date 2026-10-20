# 1 — the biplane's ray probe (FINAL)

Task S46. 2026-10-20.

## What I did

- Ported the ray probe's narrow phase as `game_sim::ray_probe`:
  - `clear`, the probe's vt +0x90.
  - `pair`, i.e. ray_collide_pair against a mesh (kind 0) and a placed group (kind 1). Any other kind
    is `Shape::Never`.
  - The four shape tests behind them, public: `boxes`, `segment_reaches`, `facet_reaches`, and
    `leaf_reaches` for the descent's mixed step.
  - The group gate `group_reaches`, and the frame helpers `invert`, `compose`, `abs_rows`.
- The segment-against-triangle test reuses `collide::test` (S4). The original's 10b980 with smooth
  normals off is the same instruction sequence. Vertices come from `MeshRef::vertex`.
- `crates/game-sim/tests/ray_probe.rs`, three oracle tests on C7's biplane save state.
- research/oracle.txt: the S46 block. It has the result record's layout and names for the nine
  functions the descent is made of.

## What the original does

- **The pair** (`./kit fn 1ebe70`, reached through the thunk at `./kit dis 1eb520`, which stashes
  probe+0x174 in the global at 0x00468250):
  1. Sets the record's +0x80 to 1.0, then dispatches on the other's +0x68 through the table at
     0x0042d090.
  2. **A mesh:** runs the tree pair. Then, if bit 0 of +0xb4 is clear, it takes the nearest normal,
     low, high and point to the world through the mesh's +0x120 and sets the bit. It always takes the
     pair's nearest point (+0x70) through. Then push = low + high. Every contact whose placed word is
     0 gets its normal and point transformed, object = the mesh, placed = 1.
  3. **A group:** the gate first (`./kit dis 1eb340`). Then, for each enabled part, the same tree
     pair; low and high are folded into a running min/max that starts at (0,0,0,1).
     - Push is set from those bounds. If bit 0 is clear, push is rotated as well, and the pair's
       nearest point goes through the part's frame.
     - Each part writes +0x84, then resets low and high.
     - Quirk: once bit 0 is set, a later part's push stays in that part's local frame. The port keeps
       it.
- **The tree pair** (`./kit dis 109130`) works out three frames and leaves them in the scratchpad
  behind pointers at 0x0041da30/40/50:
  - mesh→probe: inverse(probe) ∘ mesh;
  - its inverse, probe→mesh;
  - the inverse's row magnitudes.

  Then it descends (`./kit dis 108980`):
  - **Both leaves:** the leaf pair (`./kit dis 108340`).
  - **One leaf:** the mixed step (`./kit dis 1079d0`), then both children of the node.
  - **Two nodes:** the box test (`./kit dis 106000`), then the side with the larger key opens; the
    mesh's opens on `<=`. Both children are always tried.
- **The box test** takes box b's centre through probe→mesh into a's frame (a = the mesh node, b = the
  probe node). The axes: a's own three in the order x, z, y; b's three; then nine crosses, each passed
  early when a's share of the radius covers it.
  - Correction: my notes from before the summary said this call mixed up its frames. It does not. The
    descent passes the mesh box first, and the matrix maps the probe's box onto it.
- **The mixed step:**
  - A segment against a mesh node, `./kit dis 106820`: the midpoint and half-span on 3 axes plus 3
    crosses.
  - A probe node against a triangle: the triangle goes through mesh→probe, 13 axes. The third edge is
    normalised; the first two are not. The face normal is unpacked as short/16384, by adding the short
    to the bits of 768.0 and subtracting 768.0, which is exact.
- **The leaf pair:** the segment goes into the mesh's frame and through the segment test.
  - Without a list (+0x90 = 0), only a nearer `t` is kept.
  - With one: +0x70/+0x80 track the pair's nearest. One contact is kept per segment when +0xb8 is
    set, searched by the full child word; it is replaced only when nearer. Otherwise the hit is
    appended while count < capacity.
  - A written contact folds normal·((1−t)·travel)·normal into low/high.
  - `t <= nearest` rewrites the nearest fields and clears bit 0. Bit 1 = mesh normals ∧ probe+0x174.
- **The clear** (`./kit dis 1eb280`):
  - lowers the probe's 0x20;
  - zeroes the globals 0x00468a60/68;
  - makes +0x6c current (`./kit dis 11c110`);
  - sets +0, +0x10, +0x40, +0x50, +0x60 to (0,0,0,1), +0x80 and +0xb0 to 1, and the count to 0.
- **The tree globals:** 0x00468a80/88 point at `{nodes, list}` pairs on the pair's own stack frame,
  which is dead once it returns. The piece test re-points them.

## How it was proved

All by function oracle on `biplane.p2m2_SaveState`, with the plane's own probe (36 segments and their
tree) and the 34 candidates its search gathered (26 meshes, 8 groups of 2 parts). Every case puts the
probe so that a random segment passes through a random point on a random triangle of the candidate,
turned any way. One case in eight moves it up to 20 away.

- **`the_ray_probe_pair_matches_the_original`**, 10 000 cases.
  - Inputs: a random record (nearest fields, push/low/high, flags byte, listed or not, one-per-ray,
    capacity 0–5 or 36, up to 6 contacts already there, placed or not, possibly the same segment
    twice). Also random group part enables and the probe's 0x20 bit.
  - Variations: 1 in 4 runs the clear first, 1 in 5 ties the probe's root key with the target's, and
    1 in 6 pairs twice.
  - Compared bit for bit: the return value, the probe's flag byte, every record field, and every
    contact (both rays, normal, point, object, placed, fraction, segment).
  - Results: 9 972 pairs struck something; 2 220 of those were groups and 1 431 were second pairings.
    17 940 contacts were written.
- **`the_shape_tests_match_the_original`**, 10 000 frame setups. Each runs the pair with the descent
  hooked out; the three frames must equal the port's bit for bit. Then four of each test:
  - box/box: 40 000, 20 393 overlapping;
  - segment/box: 40 000, 20 471 overlapping;
  - the mixed step: 20 109 node/triangle (9 410 overlapping) and 19 891 segment/node (8 867
    overlapping).

  The inputs are real nodes over the target and the probe's real segments, or boxes and segments
  jittered around them.
- **`the_group_gate_matches_the_original`**: 10 000 cases, 3 339 passing. One in four is without the
  probe's tree, which takes the first-segment path.
- **Mutants:** 16 of 20 were killed. The script that runs them stays in the session scratchpad, not
  in the repo. Killed:
  - the box order;
  - the key `<=`, the nearest `<=`, and short-circuiting the children;
  - the early pass on the crosses, and the segment crosses;
  - the (1−t) travel, the +0x80 reset, one-per-ray, and the group push rotation;
  - the gate's frame and its half-span;
  - the triangle's max side, its axis-0 abs, and its second edge;
  - compose's row 3.

  Four survived, and none can show in an answer:
  - The box axis order, and the `w` of the segment midpoint. Both are equivalent: the three tests AND
    together, and nothing reads `w`.
  - Not normalising the third edge. The separating test does not depend on the axis's length, so only
    boundary rounding could differ.
  - Skipping the group gate inside the pair. The gate is looser than the part trees, so the record
    cannot show it; the gate has its own test.
- `./kit check`: passed.

## Not verified / not 1:1

- **The probe's prepare (vt +0xb0, `./kit dis 1ebd20`) is not ported.** It only runs when the search's
  +0x38 holds an object, and that word is 0 in the save state. It calls the probe's vt +0xa4 and
  0x003b4d50, 0x003b4ef0, 0x001eb760, none of them read. Note that S44 models +0x38 as a flag, but it
  is a pointer.
- **Probe+0x178 (an extra object) is not ported.** This covers 0x00411130 in the pair, 0x0026ec60 in the
  clear, and the stack-argument branch of the tree pair. The plane's is 0, and the test asserts it.
- **Kinds 5 and 6 (0x00109ee0, 0x0010a9b0) are not ported.** The port answers false for them
  (`Shape::Never`), which is wrong if one is ever paired. None is in the plane's search. Kinds 2–4 and
  7 and up are false in the original as well.
- **Smooth normals are not ported.** 10b980's interpolated-normal path and the record's bit 1 need
  probe+0x174. The plane's is 0, so bit 1 is always cleared, and the port does the same. Some candidate
  meshes do have per-vertex normals; they took the flat path in the proof.
- **The clear's two globals (0x00468a60, 0x00468a68) are not modelled.** Their purpose is unknown.
- **A group part that is not a mesh** would still run the mesh code in the original; the port's `Part`
  can only hold a mesh. All 16 parts here are meshes, and the test asserts it.
- **Packed-vertex meshes are not exercised.** None is in the plane's search. The port uses S4's packed
  vertex, which is proved there.
- **Generated inputs are bounded:**
  - probes go through a triangle point ± 20 (± 60 for the gate);
  - record values come from small ranges, with no NaN or infinity;
  - existing contacts number at most 6.
- **The record's +0xa4 array** is never written by this code; it is not modelled.
- **Not wired into anything.** `world_search::Narrow` for this probe, and `contact_search::Search::rays`,
  are S48.

## Next

Nothing waiting on this task. S48 implements `Narrow` on `ray_probe::{clear, pair}`. The record maps
onto `Search::rays`: the count is +0x8c, `Contact::{from,to}` are the rays, normals and points as
written, and the object is `Contact::object`.
