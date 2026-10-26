# 1 — overview stills (FINAL)

Task P166. 2026-10-26.

## What I did

- `track_previews::overview` (pure): the still's framing from the spawn, its facing, the ground's
  extent (0..w, 0..d) and a ground-height query. It looks from the spawn toward the ground's middle
  (along the spawn's facing when the spawn is within a tenth of the diagonal of the middle). The eye is
  pulled back behind the spawn by 10 % of the diagonal (60–350 ft), kept on the map, and raised 10 %
  (50–350 ft) above the higher of the ground under it and the spawn. It pitches 8° below the line to
  the ground at its target (the middle, or a third of the diagonal ahead), clamped to 10–50° down,
  with an 80° horizontal fov.
- The shell's new `--overview` flag: in `ride()`, after the spawn and heading are known, it takes that
  framing as `--shot-pose` (unless a `--shot-pose` was given), and prints it as an `overview: x, y, z,
  yaw, pitch, fov` line. It sets `seated` to false for every seat, so neither path draws the bike or
  the rider, and it also turns off the plane, the parked bays, the bikes' particles and the tire marks.
  Extent: a world's `t.area` under `Land::Tiles`, else the track's `Ground::extent`.
- `still_command` now runs `--overview --players 1 --markers off --minimap off --no-sound
  --world-ground all --overlay none`, 31 frames, and shoots frame `STILL_FRAME` = 30. That gives the
  props, sky and streamed ground 30 frames to come up, where the old still was frame 0.
- Version stamp in the name: `STILL_VERSION` = 2, and `still()` is `previews/<id>.v2.png`.
  `still_of(…, 1)` is P70's `<id>.png`. A v1 still, however fresh, is no longer read, so `gather`
  lists it to be taken again. Once the new one is in place, older versions' files are deleted. The
  user's `mods/previews/*.png` in the main checkout are retaken the next time the pick lists them. I
  did not touch them from here (other checkout).
- Nothing was taken from opentennis: it has no preview stills.

## What the original does

Nothing. This is front-end presentation (the original has no mod maps), ours to design per AGENT.md.

## How it was proved

- `track_previews` unit tests (bin): the framing looks toward the middle from behind and above the spawn.
  At the middle it follows the facing. The eye stays on the map at an edge spawn, rises over a hill
  behind the spawn, and falls back to the spawn's height with no ground. A bigger map is framed from
  further and higher. Degenerate numbers still give a finite pose. The test also checks the
  versioned still names.
- `tests/track_mods.rs::a_mod_maps_still_is_of_the_map_with_no_rider` (needs the disc). An old
  `<id>.png`, newer than `mod.json`, is listed to be taken again. The still job writes `<id>.v2.png`
  at `STILL_SIZE` and removes the old file. The shell's printed `overview:` numbers equal
  `overview()` of the scene's start (512, 5, 300, +z) over the flat 1020-ft ground at 0, to 1e-3.
  From one close pose at the start, the `--overview` picture is pixel-identical to the picture with
  the bike started behind the camera (`--trace-at 100,100,180`), and differs from the ridden one in
  more than 200 values. On the first run the first comparison was 27 pixels off: the bike's
  particles or tire marks at the start. I turned both off under `--overview`, and the comparison is
  now 0.
- P70's test (`a_custom_track_shows_its_own_picture_or_a_still_from_its_spawn`) still passes on the
  new stills.
- By eye: headless stills of mxvatv_nat03 (Glamis), mxvatv_mc01 and elev_glamis show the map from
  above with no rider. The track pick (`--menu --screen tracks`, settings in a scratch folder with
  `track = mxvatv_nat03`, the mods in the scratchpad) shows the new cards for Glamis, Killington,
  Desert Oasis, Lago and Glamis dunes.

## Not verified / not 1:1

- The task's "./kit shot of the track pick": `./kit shot` grabs the emulator's window, not the
  shell's. I shot the pick with the shell's own `--menu --screen tracks --shot` instead, and looked at
  it by eye only. No test asserts on the cards' look.
- The framing does not use the map's own camera spots or its biggest props. It uses the spawn and the
  ground's extent only, so a map whose interest is far from the line spawn→middle shows less of it.
  This is judged by eye on three maps, not measured.
- "World fully loaded" means 30 frames plus `--world-ground all`. For a world (tiled) mod, props are
  dressed near the spawn and stream in by distance, so dressing far from the spawn may be missing
  from the still. No world mod's still was looked at.
- Fog: the ride's fog and far clip still apply, so a map larger than the fog distance fades at the
  back of the still. Left as the ride draws it.
- The software renderer's path was not shot. It goes through the same `seated` and the same filters,
  but no test runs it headless with `--overview`.

## Next

Nothing for this task.
