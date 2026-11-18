# 1 — stadium class per prop (FINAL)

Task MD72a. 2026-11-18.

## What I did

- `quad::RecordOn` no longer gives every struck prop its `category`. Each world-ray hit now carries the
  category of the prop its owner number names: `category` when that prop has `world::Prop::stadium` (MD72:
  its collision file holds `sxcl.col`), else 0.
- The prop is looked up in `World::collision()`'s set. On a track that is the props in owner order; on a tiled
  world it is `every_shape()` (each tile's from its `first_owner`). A moving car's owner (`MOVING_OWNER`, past
  the set) and an ecosystem trunk's hit (detected as the groups sweep taking a probe over from the props) are
  of no class.
- `Bike::struck_class` (the reason-15 input) now asks the same of the record's last struck object
  (`record.struck - 1`), in `Ride::record_on` and `Ride::record_among`. Before, it masked the fixed `category`
  with the class, so any prop fired it.
- In `record_among`, a knockable body struck last (an Untamed bale: no `world::Prop`) is of no class.
- `tests/atv_quad/mxu.rs`:
  - the image-seeded free09 strike first rides at the prop as it is (no free09 prop has the class): touches
    it, no reason 15. Then the same prop with `stadium` set: reason 15 and put back, as before.
  - the files-made quad's strike sets the prop's `stadium` first, so its reason-15 check still means what it did.
- Nothing taken from ../opentennis (a tennis game has no wreck classes).

## What the original does

MD72's finding (research/journal MD72): MX vs. ATV's level start puts category 0x20 ("STADIUMCOLLISION") on
each collision object whose name holds `sxcl.col`, and on no other; the reason-15 test masks the struck
object's categories with it. MX Unleashed has no such file, so on its worlds no prop has the class.

## How it was proved

- `tests/atv_quad/mxu.rs` `the_quad_jumps_a_free09_lip_strikes_a_prop_wrecks_and_is_put_back`: the same strike
  on free09 prop 270, unflagged, wrecks at frame 60 for reason 1; flagged, at frame 60 for reason 15. Both
  runs were seen, not skipped (a temporary panic showed the lines; then removed).
- `the_quad_from_its_files_wrecks_on_a_free09_prop_and_is_put_back`: flagged prop 270, reason 15 at frame 44,
  seen the same way.
- This is a port-side rule (the quad on MX Unleashed's worlds has no original to capture). No capture or oracle.

## Not verified / not 1:1

- A non-stadium prop's struck category is 0. MX vs. ATV's own non-stadium objects carry other category bits;
  MX Unleashed's props have none to read. Only the 0x20 bit matters to reason 15, and the quad image's record
  mask is 0x0 (printed by the free09 test), so no hit is masked either way.
- A bale's (`Knockable`) own pass through `RecordOn` used to give its world hits `HULL_CATEGORY`. They now
  get it only off a stadium-class prop, else 0. That only changes anything if the bale record's mask has
  `HULL_CATEGORY` bits. Not checked against Untamed, which has no MX Unleashed props to compare against.
- Knockable bodies struck by the quad are of no class: the port's choice (they are no `sxcl.col` object).
  Their hits still carry `category` in `RecordAmong::knock`, so the mask treats them as before.
- "Was the last struck object a body?" in `record_among` is a guess: the struck number matches a body's id
  and some contact names that id. A world prop whose owner+1 equals a body's id, struck last in the same pass
  as that body, would be read as the body. Not reachable on today's worlds (no stadium props); not tested.
- On a tiled world, the first stadium lookup after a hit loads every tile's shapes once
  (`Tiled::every_shape`). Only when `category` is non-zero. Load time not measured.

## Next

none
