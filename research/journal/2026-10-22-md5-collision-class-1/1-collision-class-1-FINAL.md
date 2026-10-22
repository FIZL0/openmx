# 1 — collision class 1 (FINAL)

Task MD5. 2026-10-22.

## What I did

- `track::Collision::parse` now reads class 1: a new `Class::Hierarchy`, `track::Hierarchy` (version, the
  dropped word, `parts`) and `track::BonePart` (`bone`, `mesh`). Each part's body goes through the same
  reader as the mesh class: `Mesh::parse`'s body moved into a private `mesh_body` and nothing else
  changed. `col_header` no longer reads a file-level surface word for class 1.
- Checked every `.col` in MX Unleashed's `MX.PSR` and in openmx-mods' `extracted/mxvatv/Env/*.res` with
  a throwaway probe crate (scratchpad, not committed) linking this branch's game-sim: all 21 class-1 files
  on MXU's disc and all 7 in MX vs. ATV parse to their last byte, MX vs. ATV's `Nat07_WM_Blade.col` (one
  part, bone 1, 40 vertices) among them. None has a zero-vertex part; every one is version 2 with the
  dropped word 0.
- Filed MD12: scenes place class-1 files too, and `world::Props::load` skips them (below).
- Nothing in opentennis covers this (it has no collision files); nothing taken.

## What the original does

`./kit fn 0x0011b230` (collision_file_load) reads the header (1234, `col`, version 1|2, class). Class 0
and 2 then read a surface word for version 2; class 1 does not. Class 1 builds a 0x220-byte hierarchy
(`./kit fn 0x001ee690`) and `./kit fn 0x001efca0` → `./kit fn 0x001effe0` reads it:

1. part count, then one word read into a stack slot and never used;
2. per part: a bone index (clamped to the owner skeleton's bone count − 1, then turned into the bone
   pointer at part +0x230; 0 when the owner has no skeleton), the surface word for version ≥ 2 (low byte
   kept), then `./kit fn 0x001eded0` — the mesh class's own body reader, from the transform on;
3. a part whose vertex count (the u16 at part +0x23c) is 0 is freed, the count drops by one and the next
   part is read into the same slot.

Who loads it: five callers. `./kit fn 0x001aa760` (scene_collision_read) with owner 0; `./kit fn
0x00119070`, the animated toy, with its model's skeleton as owner; the machine race gate init
(0x003b0a70); two more (0x001597b0, 0x002126c0) not read.

**How V43 moves it.** `./kit fn 0x002144d0` reads an `[AnimatedToyN]` section (MCFName, InitialMotion,
InitialFrame, InitialTime, HasCollision, CameraCollision, AnimateWhenNotVisible, UpdateSearch, …) and calls
0x00119070. That loads the `.mcf` model, and with HasCollision loads the `.col` with the model's skeleton
as owner, so each part's bone index becomes a pointer to that bone. A class-1 result is then split by
`./kit fn 0x001eea50` into an array of ordinary mesh collidables, one per part, each keeping its bone
pointer. For each one the toy builds an 800-byte rigid body (0x001dd580, the rigid body ctor's other
caller), binds it to the part's bone through the body's vtable slot +0x74, and hangs it under the toy's
node (node_add_child); the part's +0x7c points at that body. So the clip (InitialMotion) turns the
skeleton, the bound bodies follow their bones, and each part's collision is placed by its body: the
windmill blade's mesh is in the blade bone's frame and turns with it. For the port this means a toy's
collision pose is a function of its clip clock, and the clock has to be sim state (as V43 already says).

On MXU's disc the class-1 files are: toy collision (`anim\shovel.col` free10 / free10_ss with 10 / 11
parts, `anim\mill.col` nat07, `anim\gondola.col` nat11 — 2 parts each), the machine race gate (2 parts,
in every free track's `_2` archive), and scene placements: `utilityshack1` (free09, nat06 ×2),
`watertower` and `mainhouse` (nat01), `cemeterygate` (nat02), `nat08smallshanty`. The S49 count of "21 v2
type 1" matches this list exactly.

## How it was proved

- `crates/game-sim/tests/collision_hierarchy.rs`:
  - `a_generated_hierarchy_parses_to_the_parts_the_loader_keeps`: 10,000 generated class-1 files
    (version 1 and 2, 0–5 parts, a quarter of the parts with no vertices, wide and packed points, normals
    present or not, random trees and triangles). Each parses to exactly the loader's shape — the parts
    with vertices, in order, with their bones and own surface words — through both `Hierarchy::parse`
    and `Collision::parse`; each kept part equals `Mesh::parse` of a class-0 file built from the same
    body; one byte short and one byte over both fail.
  - `a_hierarchy_is_its_own_class`, `a_single_bladed_hierarchy_keeps_its_one_part` (the blade's shape).
  - `the_discs_hierarchies_parse`: seven of MXU's class-1 files out of their archives, with their part
    counts. Shown to run, not skip: a wrong count (99 for the shovel) failed it.
- The probe over both discs, above.

## Not verified / not 1:1

- **The generated files are written from my reading of the loader, not checked against a live load.** No
  save state was inspected for a loaded hierarchy (its part count, bone pointers, surface bytes). The disc
  files all parsing to their last byte is strong evidence for the layout, but the dropped-part rule
  (vertex count 0) is exercised only by generated files: no file on either disc has such a part.
- **The bone index is kept as stored.** The clamp to the skeleton's last bone and the bone lookup happen
  when the owner binds it; the port has no skeleton to bind against yet (V43).
- **The surface is kept as the full word**, as `Mesh` already does for class 0; the loader keeps its low
  byte (MX vs. ATV's mill wheel stores 0x01000008).
- **A vertex count of 65536 or more** would be 0 to the loader (it keeps the low 16 bits) and the part
  would be dropped; the port reads all of them. No file comes near it. The same is true of the existing
  mesh class.
- **How the toy moves its parts is from reading the code only** (above): which rigid-body slot binds the
  bone, and how the part's transform is set from the body each frame, were not traced instruction by
  instruction nor seen live.
- **Scene placements of class 1 are still skipped** by `world::Props::load` (it calls `Mesh::parse`), so
  free09's utility shack, among others, has no collision in the port. That is world.rs's to fix (MD12);
  that `scene_collision_read` places a bone-less hierarchy in the file's own frame is my reading, not
  proved.
- The class-1 in-memory path (`./kit fn 0x001f03a0`, when the archive hands over a pre-loaded image) was
  not read.

## Next

MD12 places the scenes' hierarchies; V43 binds the toys' parts to their bones.
