# V7 — animation seam

2026-10-16. Task: `./kit plan V7`.

The port had `Clip`, `Pose` and `Skeleton` declared twice across two crates (three times for the
skeleton), a trait the simulation declares for the loader it may not write with no implementation on
either side, and the joint map the original builds when it binds a clip to a skeleton in neither — so
a stunt's clip could not reach the bones it animates. This task joined them: one clip (the renderer's
parsed `.mot` carries the simulation's timing record), one joint pose (the renderer's, read by the
simulation through `Joints`), one skeleton (`skin::Skeleton`, re-exported where the rider's used to
be), and one loader behind the trait — the new `game_render::anim`, whose `Set` reads a `.mcf`'s clips
off the user's disc and binds them to the `.psg` it names, and whose `Rig` runs one frame of the
simulation's own driving onto those bones. Proved against the disc: a bike's set loads, binds and
poses end to end, and `stunts.ini`'s `UsedForBike` agrees with the clips the archives ship on 1296
slot checks over all 54 bikes. 457 tests pass, up from 449. What has no oracle, and the two blend
paths that are deliberately not drawn, are listed in the entry's last section.
