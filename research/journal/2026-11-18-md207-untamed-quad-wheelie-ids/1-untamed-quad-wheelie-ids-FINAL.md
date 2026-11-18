# 1 — untamed quad wheelie ids (FINAL)

Task MD207. 2026-11-18.

## What I did

- The brief said the port's Untamed quad hands out Unleashed's 42/43. It did not: the rigged tail
  (`quad::Ride::rig_on`) left the wheelie and stoppie ids at `Tail::default()`'s 0, on **both** games.
  So a wheelie or stoppie started stunt 0, and the wreck test read that as a trick in play (ids under 41).
- `quad.rs`: a new constant `ONE_PAIR_STUNTS = (42, 43)` beside `SPIN_STUNTS`. `rig_on` puts it in the tail.
  `Ride::untamed` shifts both up by `UNTAMED_STUNT_SHIFT` (50/51), as it already did for the backflip and 360s.
- Tests:
  - `untamed_atv` `the_untamed_quads_wheelie_and_stoppie_ids_are_its_spawn_images`: rig a spawned quad, it
    holds 42/43; after `untamed()` it holds the MD147 spawn image's `+0x13d4`/`+0x13d8` (50/51).
  - `atv_quad` `the_quad_rigged_from_its_files_is_md123as` now also compares the two ids with C28's/MD37a's
    spawn image (`+0x13c4`/`+0x13c8`, 42/43). `md123a_rigs` reads them from the image too, so image-seeded
    rides start the right stunt.
- Nothing taken from ../opentennis (no quad there).

## What the original does

- Both inits look the ids up by name in the stunt table and store them on the quad: SLUS-21104 at
  `+0x13c4`/`+0x13c8`, SLUS-21701 at `+0x13d4`/`+0x13d8` (the run of name lookups that also fills the
  backflip and 360 words; `grep -n '0x13d4' context/untamed/decomp/game.c`).
- Untamed's update hands `+0x13d4` (rear pair, the wheelie) and `+0x13d8` (front pair, the stoppie) to the
  stunt start (vt `+0x4b0`) past a second on one pair, the same as Unleashed's with `+0x10` offsets. So the
  port's `one_pair` handing on `Tail::wheelie_stunt`/`stoppie_stunt` is the same flow.
- Spawn images: MD37a's `atv-r-spawn` holds 42/43; MD147's Untamed spawn holds 50/51.

## How it was proved

- A save-state image: both ids against both spawn images (the two tests above).
- `./kit check -- -p game-sim --test untamed_atv` and `--test atv_quad mxu` pass.

## Not verified / not 1:1

- The ids are constants, not looked up by name in the stunt table as both originals do. That matches the
  port's backflip and 360 ids. A mod stunt table with other rows would get the stock ids.
- Untamed's update reading `+0x13d4`/`+0x13d8` was read off the decompile only; no oracle runs Untamed's
  update with a wheelie. The image only proves what the spawned quad holds.
- No ride was captured doing a wheelie or stoppie on either game, so the stunt start's id was not seen
  live.

## Next

None.
