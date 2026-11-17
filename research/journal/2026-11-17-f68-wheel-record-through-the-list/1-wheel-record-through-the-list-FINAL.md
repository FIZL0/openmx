# 1 — wheel record through the list (FINAL)

Task F68. 2026-11-17.

## What I did

- Remade the S195 knock states in copy 3 (first_recording in slot 8, `aim.py <pid> 8 2 40 25 9`,
  c6 `resume_if_paused.py`, `knock.py f68 9 2 --frames 60 --late 15`): ball 2 woke at 2920;
  context/s195/f68-{2919,2920,2921,2922,2935}.p2s, their RAM images context/states/s195-f68-*, capture
  context/capture/s195/f68.csv. At 2935 balls 1 and 2 are awake.
- research/s195/slots.py: the copy folder now comes from the kit (`emu.config_dir()`); the old
  `<root>-slots/pcsx2/sN` path did not exist.
- Found the port listed awake props wrongly: F8 filed a woken prop in the moving list, so the bike's
  list held the awake balls from 3,600 ft away. The original keeps it in the static tree, re-keyed where
  it stands. `flight_search.rs`: every knock is filed in the tree (asleep or awake);
  `Kept::place_knocks` re-keys them each search and, when a key changed, undoes the tree back to the
  first changed knock's log mark and files the rest again (knocks are filed last, so this is cheap). The
  stamp no longer carries the knocks' poses, so a moved prop does not rebuild the world.
- tests/prop_knock.rs `the_wheel_probe_meets_props_and_scenery_through_the_bikes_list`: from f68-2935,
  the bike's node placed on every free09 prop (32 placings each, 128 for one resting on a scene mesh).
  Half are low with the node rolled 20–29°, so the aimed segment meets a crate's top and the other
  segment the mesh under it. Each placing compares the original's gather + `collide_against_search`
  with the port's `refresh_knocked` + `Scene::ridden`.

## What the original does

- The bike's list is gathered round the body rays (rider +0x7c4) into the nearby search (+0x7bc); the
  wheel probe (+0x7c8) is searched against that list once.
- A struck prop's mesh collidable keeps its place in the static tree (space node +8 = table+0x34) under a
  key for where it stands now; only its +0x116 flags change (0x04 → 0x14). The moving list holds none of
  the props. S240's note ("in the moving structure") is wrong for them; corrected in research/oracle.txt
  (F68 section).
- Props struck in a search wake in the order heard (enable, then activate, listeners onto the node's
  list).

## How it was proved

- Function oracle over 5,056 placings from a save-state image (f68-2935), tests/prop_knock.rs. Bit for
  bit: the list (entries in order, distances), its centre and bound, the wheel record (struck, point,
  normal, fraction), and after each placing the wakes (bodies enabled, collections active, the node's
  listener list).
- Coverage: 3,110 placings hit; 184 scenery and 3,067 prop contacts; 124 records hold scenery and a
  struck prop together; 71 placings wake more than one prop.
- prop_knock, prop_wheel_gather, prop_strike_paths, prop_plane, ragdoll_rest and car_props pass;
  `./kit check --all` gate.

## Not verified / not 1:1

- The original's list is filtered before its search: trigger boxes, bikes and the shack's hierarchy
  meshes are taken off it, because the port's world does not hold them. Their effect on the record is
  untested.
- The order of entries within one tree chunk after a re-key is only checked for these states. A tree
  chunk with many props re-filed in another order would need its own case.
- No placing put a plant in the record (plants 0). Plant contacts are covered by F7/F8, not here.
- "A prop and scenery on one segment" is read as: one wheel probe whose record holds a scene mesh on
  one segment and a struck prop on the other (the record keeps the nearest per segment). A ~3 ft crate
  and the mesh under it cannot both lie on one 2.2 ft segment.
- The per-substep searches of a whole ride are not replayed; this is one search per placing.

## Next

- none
