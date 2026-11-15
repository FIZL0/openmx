# F53: the part's pair kinds C15 never makes

The C15 replays never reach kinds 0, 1 or 6 in the part's pair, so this tests them on generated probes.

- New test `the_parts_pair_kinds_are_the_originals` (mxvatv_bike_ride.rs): 10,000 cases on state 4's riding part.
  - Each case: up to 16 rays aimed through nat05 triangles, plant spheres, made-up spheres and boxes, under a random tree or none.
  - Paired with nat05 meshes (kind 0), F51 hierarchies (kind 1, gate box and switched-on parts drawn), spheres (kind 5), boxes (kind 6), other kinds.
  - The record is drawn too: one contact per ray or not, listed or not, capacity 1–4 or full.
  - Every answer and every word of record, arrays, probe flags and globals matches. Strays checked.
- Kind 6 ported: `atv_part::box_reaches` (0x10a100 / 0x109fe0 walk).
- The `+0x188` "filter" is a debug profiler. Only the constructor 0x001bb0b0 writes it (0). Not ported.
- Globals: 0x3dce38 = last descended mesh +0x230, 0x3dce30 = 1. 0x3dce50–0x3dceef is leaf-test scratch, used only in 0x107000–0x10a000.

## Not verified / not 1:1

- The profiler at `+0x188` is not ported: the game never attaches one, and it reads the hardware timer, so it can't be bit-exact. Callers assert 0.
- The scratchpad matrices and the scratch at 0x3dce50–0x3dceef are not compared: dead after the pair.
- 0x3dce40/0x3dce48 (stack addresses of the descent's frame) are copied from the original, only checked to point into the stack.
- Smooth normals (`+0x184`) are not ported; the test asserts the flag is 0.
- State 4 has no real kind 6 and no hierarchy with gate box or switch array; these are made up.
- No ride reaches these paths, so no ride replay covers them.
- No mutant run on `box_reaches` (usage limit).
