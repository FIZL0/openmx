# 1 — shot pose (FINAL)

Task MD10. 2026-10-22.

## What I did

- `--shot-pose x,y,z,yaw,pitch,fov` (`crates/openmx/src/shot_pose.rs`, wired in `main.rs`): the first
  view's camera is replaced, after the ride camera and its blend, by one at the pose, every frame of
  the run — so the window shows what `--shot` writes, and any `--shot-frame` is from the pose.
- The numbers are openmx-mods' (`tools/accuracy/src/pose.rs`, `notes/accuracy.md` "Poses"): eye in
  feet, yaw 0 along +z and 90 along +x (`atan2(f.x, f.z)`), pitch up positive (`asin(f.y)`), no roll
  (the rasteriser's `look_at` keeps world up, which is exactly the harness's "up square to forward").
- **The fov's axis, settled:** it is the horizontal angle, as the camera presets carry it, and goes
  through `aspect::fov` (`view_fov`) like a preset's does. Evidence: `research/shots/moment.py` reads
  the original's view-projection and finds 85° horizontal with the 4:3 that makes it 69° vertical —
  the harness's viewport field is 85 on the ride camera, so it is the horizontal one. 85 at 640x480
  converts to 69.0° vertical here (unit test). openmx-mods' T9 asked for this.
- Refused: not six finite numbers, |pitch| ≥ 90 (no yaw to keep the roll by), fov outside 1..179.
- Nothing taken from opentennis: it has no posed shot.

## What the original does

Nothing to port: presentation only. The pose's convention is the harness's, read from the original's
viewport block (eye, forward, up, fov) by openmx-mods.

## How it was proved

- `shot_pose` unit tests (bin): the harness's numbers parse; yaw/pitch give back `atan2`/`asin`; a
  ground point on the line of sight lands at the middle pixel (±0.5) through `overlay::marker_post`
  (the same projection the session markers' posts use) at 640x448, 1920x1080 and 300x800, for three
  poses, and a point toward a larger yaw lands right of it; 85 horizontal is 69 vertical at 4:3.
- `tests/shot_pose.rs`: the shell on free06, posed 20 ft beside the start and 6 up, looking at the
  bike's middle: frame 1 has the bike centred; frame 90 after three seconds of full throttle has the
  middle >30 % changed (bike gone) and the top corners <5 % changed (the camera did not follow); and
  the posed shot is >30 % apart from the unposed one. Looked at the two PNGs too: bike centred, then
  the same hills without it.

## Not verified / not 1:1

- No side-by-side with the original's shot of the same pose: that is openmx-mods' T9, which needs
  MD2 (a track from a mod folder) as well. Whether the same numbers give the same picture in both is
  therefore unproven; the yaw's handedness is the port's world, which the V2 moments matched to the
  original's screenshots, but no posed pair has been compared.
- Only view 0 is posed; with `--players` > 1 the other views keep their ride cameras (the harness
  shoots one player).
- The ride's sound is still heard from the ride camera, not the pose (the listener is taken before the
  override; a shot has no sound). Session markers' posts are drawn through the posed camera.
- The `--moment` path (V2) keeps its own camera and ignores `--shot-pose`.
- "Any track or mod": proved on free06 only; a mod track rides through the same camera code once MD2
  lands, untested here.

## Next

openmx-mods T9: set `--shot-pose` in its `--openmx-args` and take Lago's pairs once MD2 lands.
