# 1 — DriveOnSoultree hangs the original (FINAL)

Task S187. 2026-11-03.

## What I did

- `crates/game-sim/tests/presample.rs`: a new test, `the_soultree_ray_is_in_no_world`, on recording2's
  state of free09 (six trailers). It checks four things:
  1. Every car's soultree ray (`+0x424`) has a space link of 0. Its mesh's link is not 0.
  2. The ray's first ignore list (mode 1) names the car's own mesh and nothing else.
  3. The original's update with `DriveOnSoultree` 1 and `PresampleTerrain` 0 leaves RAM in the
     interpreter (`catch_unwind`, "is not RAM").
  4. Each ray is given its mesh's space, and all six cars are switched on (presample off). For 600
     ticks the ray strikes nothing, and every car's clock, up and world matrix match the port's
     `Spec::step`, which rides the terrain alone, bit for bit (3600 car-ticks).
- `crates/game-sim/src/traffic.rs`: the module doc says why `DriveOnSoultree` is not here.
- `crates/game-render/src/toy.rs`: `Car::drive_on_soultree` now defaults to on where the key is
  missing, as the original's reader does. Nothing reads it, and every disc car sets it `FALSE`, so no
  value read from the disc changes.
- `research/oracle.txt`: an S187 block (the branch, the init, the live check).

## What the original does

`./kit fn 0x115540`, `./kit dis 0x115540 0x116150`, `./kit fn 0x1147c0`.

- **The branch.** It runs only when the car does not presample and is not a held level car, after
  the ground query:
  - a matrix at the curve point (`0x0013e330`, forward (0,0,1,1), up (0,1,0,1));
  - the ray's move virtual (vtable `0x00451a90` +0xac, which makes no calls);
  - `probe_collide_world(ray, 0)`.
- **On a hit** (`+0x116` bit 5):
  - `s = 2(1 - fraction) half - half`, with `half` = 5.0 (`+0x54c`, a constant of the init);
  - the hit point is `point.xyz + (0,1,0)·s`, `w` the point's;
  - it is taken when `!(hit.y <= ground.y)`, and the up becomes the record's normal. A level car
    writes that height as its level.
- **The init.** It builds the ray (`0x001ea830`) and hangs it under the scene node (`0x00132f50`). It
  pairs the ray and the mesh both ways (ignore lists). It inserts only the mesh and the `m.col` into
  the world (`0x00119c60` → `0x001ab220`). Nothing else writes the ray's space link: the move virtual
  makes no calls, and the update does nothing else for it.
- **The search.** `probe_collide_world`'s tree query (`0x001bb3e0`) takes the space's table from
  `*(0 + 4)`, which is kernel memory. In the interpreter that ends in a read of `0xff5953ac` at pc
  `0x001bb43c`.

## How it was proved

- **The test above.**
- **Live, on PCSX2 copy 3.**
  - Setup: recording2's `.p2s` loaded into scratch slot 8. Car `0x01241f40`'s ray reads space 0 there
    too.
  - `+0x4e8` = 1 and `+0x4ec` = 0 written over PINE: the frame counter stopped within 4 frames
    (2477 → 2481, then frozen), and the same happened on a second load.
  - Controls, which kept running:
    - `+0x4ec` = 0 alone;
    - `+0x4e8` = 1 with `+0x4ec` = 1, where the branch is skipped.
- `./kit check -- -p game-sim --test presample`: 4 passed. The new test ran rather than skipped:
  with its count assert raised by one it failed, 3600 against 3601.
- `./kit check -- -p game-render toy`: 5 passed.

## Took from opentennis

Nothing: it has no path cars.

## Not verified / not 1:1

- **A mod `.toy` car with `DriveOnSoultree` on (or the key left out) and `PresampleTerrain` off.**
  - The original hangs. The port does not hang: the car rides the terrain as if the switch were
    off. That is a deliberate difference; a hang is not behaviour to port.
  - Only the RAM write was tried live, not a modded `.toy` through the reader. The reader's default
    (TRUE) is from S127's reading of `0x00289710`.
- **The hang itself.** I saw the frame counter stop. I did not trace the EE to see where it sits
  (an exception loop or a spin in the query).
- **What the ray would hit if it were in the world** (props, other cars' meshes, bikes) is not
  ported. The intended rule (the hit taken when higher) is written in the oracle note only. In a
  scratch run with the ray in its mesh's space, a second car put on the first car's path did not
  strike the first car's mesh either. The scratch run was not kept.
- **The PC exe** (MX vs. ATV) was not read: this worktree has no `pc` dump. It is a later game, and
  it could not change what the PS2 original does here.

## Next

Nothing. A port-only "drive on the props" for mod cars would be new behaviour, not the original's;
filed as `later`.
