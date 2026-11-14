# 1 — rider sphere pair (FINAL)

Task S279a. 2026-11-14.

## What I did

- `game_sim::atv_sphere::pair`: a rider part's sphere against another collidable. The other is a `Target`
  (nearby bit, `Shape`, the object the record names), and the shape is one of:
  - `Mesh`: a `Placed`, which is frame + `MeshRef` + per-triangle packed normals.
  - `Hierarchy`: box, frame, parts.
  - `Sphere`.
  - `Box` (kind 6).
  - `Other`.
- The pair writes into a `Record`, the sphere's contact record. It answers the hit and also the last mesh it
  walked, because the original leaves that mesh's vertex descriptor in a global and the test glue has to
  write it.
- New helpers in the module:
  - `in_frame_of`: the box arm's inverse and compose.
  - `spheres_meet`.
  - The tree walk (`descend`).
  - The ATV edge distance (`edge`).
- Borrowed from MX Unleashed's ported `ragdoll_scene`: `inside`, `unpack`, `unit`, `minus` and `dot`. These
  are bit-identical in the two games. Each was made `pub(crate)`, a one-word change, and nothing else there
  moved.
- `tests/mxvatv_bike_ride.rs`:
  - `sphere_pair_in_ram`: reads the other by kind, the sphere and the current record, runs the port, writes
    the record and the descriptor global back.
  - `PortScene`'s `SpherePairInRam` now calls it, so nothing is handed to the original any more (its keys
    lost the `>`).
  - The oracle `the_rider_spheres_pair_is_the_originals`.
- Took from:
  - F51's `sweep_scene`: nat05's meshes, turned copies and hierarchies.
  - F54's use of `ECO_PART` to stand plant parts.
  - S279's place oracle, for its shape (snapshot, original, stray-write check, restore, port).

## What the original does

- `./kit dis -p atv 0x001c2ec0` (vt `+0xb4` is a `j` to it). If the nearby bit (`+0x45` & 1) is clear, it
  answers no. Otherwise it goes by kind:
  - **Box (6)**: inverts the sphere's frame, composes the other's frame onto it, and tests the sphere's point
    against the box. Answers that and writes nothing.
  - **Sphere (5)**: places both points and compares the squared distance with the radius sum squared.
  - **Hierarchy (1)**: tests its box around the sphere's centre. Then it walks each part's tree; the first
    part touched gets the record write.
  - **Mesh (0)**: walks its own tree, then does the record write.
- **Record write**: zeroes the spare words and sets the centre. Then every unclaimed contact goes through the
  frame of the mesh that was hit and is marked claimed with the other's address.
- **Tree walk** (`./kit dis -p atv 0x00108ba0`): MX's `sphere_mesh_descend` with one difference, the edge
  distance (`./kit dis -p atv 0x00109050`). Its two sums are arranged differently, with `adda 0` and
  `msub`/`madd` (see `oracle.txt`).
- The wrapper `./kit dis -p atv 0x001095c0` stores the descriptor in a global before the walk.
- Full detail: `research/oracle.txt` `# S279a`.

## How it was proved

- **Function oracle**: `the_rider_spheres_pair_is_the_originals`, 12,000 pairs on state 4's nine rider
  spheres. Every case passed.
- **Targets**:
  - nat05's meshes, with turned copies and F51's hierarchies (box and frame drawn per case).
  - Plant parts stood by the original's eco part call.
  - Made-up boxes and other kinds.
  - 1 in 12 with the nearby bit down.
- **Sphere**: aimed on, inside or outside a triangle, or near a plant sphere or a box. Its radius words are
  sometimes special floats (NaN, ∞, denormal).
- **Record**: drawn, with room 0..=3.
- **Checked**: the answer, all 45 record words and the descriptor global are equal, and the original writes
  nothing outside the record, the global and the stack.
- **Paths reached** (counts):

  | Target | hit, taken | hit, record full | miss | nearer | not nearer | other |
  |---|---|---|---|---|---|---|
  | mesh | 1,415 | 606 | 2,284 | 147 | 110 | |
  | hierarchy | 592 | 265 | 1,485 | 57 | 34 | |
  | plant sphere | | | 939 | | | 744 hit |
  | box | | | 1,259 | | | 892 hit |

  Also: other kind 533 misses, not nearby 986 misses, and contacts claimed from older entries 230 times.
- **Found by the oracle**:
  - The walk writes the descriptor global.
  - The radius words are FPU operands: a NaN pattern counts as the largest value. The port now reads them
    through `fpu::operand`, and the record's distance too.
- `the_bone_collisions_scene_is_the_ports` (C15 ride-2 and flip-1) still passes with the port's pair in
  `PortScene`. C15 never pairs, so on C15 this only shows that nothing changed.

## Not verified / not 1:1

- **No live capture of a pair.** C15 never reaches one (nothing comes within reach). The pair is proven by
  the function oracle only. A capture with a scene hit is S279b.
- **Kind 6 boxes and kind 1 hierarchies are made up.** nat05 has neither, so their layouts are those the code
  reads, filled with drawn values.
- **The record's count never goes above 3 here.** The record holds three contacts, and the original would
  walk on past the arrays. The port panics there instead (array bounds).
- **The descriptor global is written by the test glue, not the port.** The port answers which mesh was last
  walked, and the glue writes the global itself.

## Next

Nothing for S279a. S279b (a capture with a rider part hitting the scene) is still open.
