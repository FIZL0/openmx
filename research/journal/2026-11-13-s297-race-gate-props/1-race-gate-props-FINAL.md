# 1 — race gate props (FINAL)

Task S297. 2026-11-13.

## What I did

- `race.rs`: the supercross loader's two prop lookups and the prop bound query, after
  `place_finish_line`:
  - `find_prop` (the scene's lookup) and `find_toy` (the toy list's, the finish gate's second try),
    with the game's own `tolower`, which counts some bytes past 0x7f as capitals (`HIGH_CAPITALS`);
  - `Supercross::gate_props`: the drop and finish props by the loader's names (`PropAt::Scene` /
    `PropAt::Toy`);
  - `PropBound::query` with `bound_flags`: the stale-bound rebuild (box branch), the radius and the
    flags;
  - `Prop::new` off the frame rows and the half extent the query gives.
- Tests 39–42 in `tests/race.rs`. The sx01 loader test's comment now points to test 42 for the width
  and height.
- Oracle entries: `scene_prop_find`, `prop_bound_rebuild`, `toy_list`, and the drop gate's name chain.
- Filed S324 (low): the bound rebuild's sphere branch.

## What the original does

- `./kit fn 0x002f9f40` (the supercross load):
  - Drop gate: it asks the scene (`./kit dis` its vt +0x120 → `0x001a6040`) for the loaded world's own
    drop-gate name (app pointer → +0x3cc → +0xb4, sx01 "THQsxdropgate"), strcpy'd and lowercased, or
    else "dropgate".
  - Finish gate: "finishgate" from the scene, then `0x00212af0` on the toy list (`*0x004e18c0`); the
    prop is toy +0xb0 → +0x30.
  - Width = 2·half.x (an FPU mul), height = half.y, from `0x001b44b0`.
- `0x001a6040`: holders +0x3d0/+0x3d4, prop = holder +0x34 (0 skipped). The name is copied, lowercased
  by `0x00413118`, and searched with strstr for the needle as given.
  - `tolower` reads a ctype table at `0x0044aaa9` with a signed char. Bytes ≥ 0x80 index the 128 bytes
    before the table, and bit 0 of those marks 58 of them as capitals.
  - The +0x20 is stored as a byte, so 0xe0 and up wrap, and 0xe0 becomes a terminator.
- `0x00212af0` lowercases the hay into a buffer, and the needle in place once per toy entry. Only a
  high byte notices: 0x95 → 0xb5 → 0xd5 …
- `0x001b44b0` calls vt(+0x38)+0x88 (`0x001a17e0`) when +0x110 bit 7 is set and (bit 3 or not bit 6),
  then clears bit 7 and copies out +0xe0 and +0xf0.
- `0x001a17e0`, box branch:
  1. The transform rebuild (`0x001b1750`; it returns at once on bit 0).
  2. The own box when bit 2 is set.
  3. Each child of the array +0x11c/+0x13c through `0x001b42a0` with its last argument 0, so the
     children's children are not walked.
  4. Then centre/half and the radius `sqrt(madd(madda(mula)))`; bit 6 is set when the radius is > 0.
  - The sphere branch (bit 4) is not ported.
- **Why S196's finish width looked unprovable.** sx01 has no scene prop named finish. The finish gate
  is toy 1 "FinishGate", whose model node is `tracks\sx01\anim\thq_finishgatenarrow.psg`: flags 0x49
  (no own box), with seven array children: the THQ_finishgatenarrow gate box and FireworksDummy01..06.
  - In the image the dummies have been animated to y 211–252, so the kept bound is widened to
    (59.95, 86.06, 1.77).
  - In the .psg's rest pose all six sit inside the gate box (x within ±40.6, y 58.7, against the box
    x −42.45..43.45, y −19.8..77.45).
  - So the load-time bound was the gate box alone: half x and y (42.946075, 48.619644) to the bit,
    giving line width 85.89215 and height 48.619644. The depth comes back through the prop's turned
    frame one ulp short (1.7741324 against the box's 1.7741325); the image's kept bound has the same
    value, and the line does not read it.

## How it was proved

All in `crates/game-sim/tests/race.rs`, run through `./kit check`:

- 39 `the_scene_and_toy_lookups_are_find_prop`: 10,000 generated cases (holders without props, empty
  needles, bytes 1..0xff, needles cut short by their own lowercasing) against `0x001a6040` and
  `0x00212af0`. Checks the answers and the needle the toy lookup leaves in RAM.
- 40 `the_prop_bound_query_is_prop_bound`: 10,000 generated trees against `0x001b44b0` with
  `0x001a17e0` behind it. Checks the answer and the kept +0xe0/+0xf0/+0x118/+0x110.
- 41 `the_race_images_gate_props_and_their_bounds_are_the_loaders`: on each image whose activity has a
  scene, among s196-sx01, s296a-{sx06,sx08,sx11,sx20,nat03}, s282c-sx01-race and c31-sx01-ai4:
  - `gate_props` against the original's lookups, called with the same names;
  - every scene prop forced stale|rebuild against `0x001b44b0`, on the answer and the kept words.
- 42 `sx01s_finish_line_is_as_wide_and_high_as_the_finish_props_rest_bound`: the finish toy found by
  `gate_props`, its .psg read off the disc and stood at the prop's frame, then `PropBound::query`
  (stale), `Prop::new` and `place_finish_line`. All of the line gate's words match S196's image to the
  bit, width and height included.

## Not verified / not 1:1

- The bound rebuild's sphere branch (bit 4) is not ported. `PropBound::query` returns `None` for a
  stale sphere-bounded node; neither gate prop has bit 4. Filed as S324.
- The transform rebuild that the bound rebuild runs first (`0x001b1750`) is not ported. The image test
  reads the rows after the original's query, and the finish proof builds the rows from the .psg
  (`physics_car::model_tree`).
- Resolving a toy to its prop (toy +0xb0 → +0x30) and reading the node names is the caller's job; the
  port takes names, not objects.
- The original copies names into fixed stack buffers (strcpy, no length check). A name longer than
  those buffers overflows in the original; the port has no such limit.
- The finish width and height are proved on sx01 only (the one image with the line loaded at a known
  frame and its .psg read). The other images check the lookups and bounds, not the lines.
- The drop gate is checked only through S196's existing loader test (grid and start gates). On the
  other images the drop prop's lookup is checked, not the gate words.

## Next

Nothing left in S297. The loader's remaining work (the generated loader oracle) is S298's.
