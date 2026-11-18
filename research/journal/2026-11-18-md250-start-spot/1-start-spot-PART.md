# 1 — start spot (PART)

Task MD250. 2026-11-18.

## What I did

- Found Rocksbury's start spot: `SX_A.scn` `[SceneInfo]` `DefaultStartPosition = 395.76,3.76,155.86`,
  `DefaultStartDirection = 0,0,1`. The md147 spawn image holds the same three words twice in RAM.
- `made()` in tests/untamed_race_quad.rs now reads the model (wheel setup and part reader) at that spot and
  heading, through a new `scene_start()` that reads the scene file. The ride itself still spawns at the
  image's body, as before.
- Kept the bale test's `inv_inertia` exception: the start spot does not fix it. Updated its comment.
- `./kit check -- -p game-sim --test untamed_race_quad`: 6 tests pass.

## What the original does

Same order as SLUS-21104 (MD134): creation copies the descriptor's point and heading to the quad, the init
re-places the node there, and the part reader reads the box at the init's end. The chain and addresses are
in research/oracle.txt under MD250.

The image's settled checkpoint (+0xd30..) and put-back point (+0x80) are not the creation point. The
creation point is the scene's default start.

## How it was proved

- Sweeping the frame origin per axis against the image's wheel mounts: the mount words' rounding needs
  origin y in [2, 4) and z in [128, 256). The scene's start (y 3.76) is the only stored point that fits.
- At `spawn_frame(start, +z)`, 11 of the 12 mount words are the image's bit for bit (before: none of the
  y words were).

## Not verified / not 1:1

- **The record's inverse inertia is still about 36 ulps off** (the task's goal). At the start spot, heading
  +z, the part reader's half box is `4004287d 3fff2a9a 4044d004`. The record needs `4004287d 3fff2a9e/9f
  4044d026`: 8 ulps more in y, 34 in z. Yaw, pitch and roll sweeps (±0.05 rad, 1e-5 step), the start less
  the CofG (0,1,0), and origin sweeps give neither this box nor all four mounts. The exception stays.
- **Wheel 2's mount z word** is `c01d7080`; the image has `c01d70c0` (1 ulp at that size). Same unknown
  cause, probably.
- My guess, not checked: the model's node locals at box time are not the file's (a default pose or
  suspension set before the phy load), since x matches and y, z and one rear wheel do not.

## Next

Find what the model's nodes hold when Untamed's part reader (0x004b9680) runs: hook it in an oracle on the
spawn image or read the node rows the init leaves, and compare with the file's locals.
