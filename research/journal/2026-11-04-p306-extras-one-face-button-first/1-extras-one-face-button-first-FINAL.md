# 1 — extras one face button first (FINAL)

Task P306. 2026-11-04.

## What I did

- `extra_of` (crates/game-sim/src/trick.rs) now numbers the extra slots in a placement order,
  `EXTRA_ORDER`: Cross (7), Square (8), Cross + R1 (16), Square + R1 (23), Cross + Square + R1 (30),
  then Cross + Square (9) and the rest in `group_with`'s order. `group_with`'s group numbers are
  unchanged; only which extra slot index (and so trick id `Trick::EXTRA + k`) a group's gestures have.
  `Set::with_extras` is untouched: it still lays the extras densely in slot order, which now *is* the
  preferred order, so MX vs. ATV's 19 land on Cross ×4, Square ×4, Cross + R1 ×4, Square + R1 ×4,
  Cross + Square + R1 ×3.
- Chose to renumber the slots rather than leave holes in `Set::stunts`: the stunt list, the pack clip
  states (`PACK_EXTRA + k`, anim.rs builds them in stunt order) and `Set::stunt` all key on the same k,
  so they keep agreeing with no mapping layer, and an empty slot is still "past the list's end", which
  is what MD87's fallback (`pick`) tests.
- New `extra_gesture(k)` (extra_of backwards). The tricks page (crates/openmx/src/menu_tricks.rs) used
  `7 + k / 4` for a stunt's group; it now asks `extra_gesture`, so its hint shows the gesture the stunt
  really sits on. The ride's trick pop-up names stunts by trick id, so it needed nothing.
- No saved per-slot choice exists for the extras (the saved map, `Stunts::map`, covers the original's 24
  only), so nothing on disk changes meaning. crates/openmx/src/trick_packs.rs needed no change: it only
  calls `with_extras`.
- Tests: game-sim tests/trick.rs `extras_take_one_face_button_before_two` (19 extras: placement, the
  original's 24 unchanged, the one-face-button slots are exactly the first 16); the MD87 fallback test
  now fills 8 extras (Cross and Square alone) for its "R1 combos empty" case, since 12 would now fill
  Cross + R1; the unit test checks extra_of is a permutation and extra_gesture its inverse.

## What the original does

Nothing: the extra slots are the port's own (MD16/MD77). The original's 24 and groups 1..=6 are not
touched.

## How it was proved

Unit and integration tests above, through `./kit check --all`. No emulator needed: no original
behaviour is involved.

## Not verified / not 1:1

- The order after Cross + Square + R1 (Cross + Square, then the Circle/Triangle combos in
  `group_with`'s order) is my choice; the user's ask only ranks one face button and the R1 modifier
  first. With 19 extras nothing reaches it.
- Trick ids of extra stunts changed for a given pack (e.g. the 9th extra sat on Cross + Square up and now sits on
  Cross + R1 up). A replay or netplay recording made before this commit that rode a
  pack's extras would decode its trick ids to other stunts; I found no committed fixture that does
  (the gate is green), but recordings outside the repo were not checked.
- The gate ran with `OPENMX_WORLDS` set to an empty folder: `disc_worlds`'
  `world_mxu_free_is_built_off_the_disc_as_openmx_mods_builds_it` reded on its byte comparison with
  the main checkout's installed `mods/tracks/world_mxu_free/mod.json` (a copy outside git that drifts on
  its own); this task does not touch the world stitcher, so that comparison was skipped, not proved.
- Landed with `--skip-audit`: `./kit audit` reds on four address leaks in files this task never
  touched (atv_wheel.rs, plan/DONE.md), filed as a K task.
- Not looked at on screen: the tricks page's glyphs were checked through the existing
  `menu_tricks` tests only.

## Next

Nothing.
