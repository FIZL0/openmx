# 1 — baked track pictures (FINAL)

Task P282. 2026-11-03.

## What I did

- `track_previews::kept`: the newest framing's still that is there (v2, then P70's v1), shown whatever
  `mod.json`'s age; `still_fresh` (the mtime check) is gone. `gather` puts no world (`mod.json` kind
  `world`) in the ids to shoot: a log line and no picture.
- `track_previews::Ride` (a counter, RAII): main.rs holds one across each `ride(a, Some(&mut shell))`
  (the local pick and the lobby's), so it covers the load and the ride. `still_job` waits while one is on;
  `shoot` spawns the still's shell under a global one-at-a-time lock, polls it every 100 ms and kills it
  if a ride starts (`Shot::Ride`), and the job shoots that id again after the ride.
- `track_previews::bake` + `modstore_up::bake_picture`, called first in `package_telling` for a track or
  world in `mods/tracks/<id>`: copies the kept still (or moves a fresh shot, uploader's machine only, never
  under `cfg(test)`) into `previews/track.png` after checking it decodes; a mod with its own is left as is;
  one that cannot be had goes up with no picture, as before.
- tests/track_mods.rs' P70 and P166 tests now expect the still kept after a mod.json change and an old
  framing's still kept (shot explicitly to check the new framing).

## What the original does

Nothing: menu art and the store are the port's own.

## How it was proved

- `track_previews::tests::a_picture_is_kept_and_a_world_never_shot`: own picture, v2 still, v1 still and a
  world with a still all shown with newer mod.jsons and none shot; a world with nothing is shot by no one;
  only the bare track is.
- `track_previews::tests::no_still_is_shot_beside_a_ride`: a stand-in shell script that marks each run;
  a job started under a `Ride` runs no child for 0.5 s and runs it once the ride drops; a 60 s child is
  killed as a ride starts (the lock frees well inside 10 s), not run again during the ride, run after it.
- `modstore::up::tests::an_upload_with_no_picture_carries_one`: a made world with only a kept still packs
  with `previews/track.png` in its folder and its zip; its own is never overwritten; with nothing, no
  file is left.
- tests/track_mods.rs' P70 and P166 tests (they shoot real stills with the disc) and all 12
  `modstore::up` tests pass.

## Not verified / not 1:1

- Not run on the Deck, nor world_ultra picked from the menu by hand: the ride guard is proved with a
  stand-in shell, not the real menu.
- The wasm build was not compiled here (no wasm32 target installed); the code it reaches is std-only.
- A disc-built world with no still now shows no picture until F28 (or a hand-copied still).
- An upload's fresh shot of a world on the uploader's machine still costs what the menu's did (8.4 GB for
  world_ultra); it is stopped by a ride but not otherwise bounded.

## Next

Nothing for this task. F28: a cheap top-down picture for worlds that have none.
