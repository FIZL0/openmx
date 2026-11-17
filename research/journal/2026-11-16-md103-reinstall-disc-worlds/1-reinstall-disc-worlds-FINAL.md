# 1 — reinstall disc worlds (FINAL)

Task MD103. 2026-11-16.

## What I did

- Built world_mxu_free and world_mxu off the disc with this branch's release shell (stitch VERSION 5, base
  796ec55e7): `target/release/openmx --seams --headless --no-sound --frames 2 --track <id> --mods <empty dir>`.
  world_mxu_free: 5 tiles, 47 joins, 27585 x 5589 ft, spawn free07. world_mxu: 27 tiles, 462 joins,
  22294 x 19243 ft, spawn free07, joins built in 9.2 s from its start.json.
- Copied each build's `mod.json` and `joins/` over `~/repos/openmx/mods/tracks/<id>/` (the main checkout's
  git-ignored mods folder), kept its `tiles/`. The build's `start.json` is not copied: it is the stitcher's
  own cache, no reader of an installed world uses it. Both world's tile ids match the installed `tiles/`.
- Old copies (openmx-mods builds of 11-02 and 11-04) are in `context/md103-old/<id>/`.
- world_mxu was installed too: the checkout rode openmx-mods' old "every freeride and national" world_mxu
  (spawn nat01, 37792 ft), not the disc build. Now it rides the disc build, same as a slot.
- world_mxu_nat was never installed (no `tiles/` for it there); left so. world_mxvatv, world_untamed and
  world_ultra are other games' worlds, not disc worlds of this game; untouched.
- No code change: the skip on a different tile layout stays, since the installed copy will drift again with
  the next stitcher or spec change, and a red on every slot for that is wrong.
- Took nothing from ../opentennis (it has no stitched worlds).

## How it was proved

- `./kit check -- -p openmx --test disc_worlds world_mxu_free_is_built` passes, and run by hand with
  `--nocapture` it no longer prints "skipped the comparison": the built mod.json and all join files are
  byte-equal to the installed copy, and the installed copy rides with the same world lines.
- `the_installed_mx_unleashed_worlds_ride_promptly` rides both installed worlds within its CPU limit.

## Not verified / not 1:1

- world_mxu has no byte comparison test against the installed copy (only world_mxu_free has one); its install
  was checked by `cmp`/`diff -r` against the build right after copying, nothing keeps it so.
- The installed copies match this branch's stitcher only; a later stitcher or spec change makes the
  world_mxu_free comparison skip again until the next reinstall.

## Next

none
