# 1 — rider lod and ik (FINAL)

Task V10. 2026-10-17.

## What I did

The two things V8 left out of a drawn rider, both of which show at speed: the five levels of detail
in a `.psg`, and the inverse kinematics the four dummy anchors exist for. Four pieces, all additive.

- **`game_render::psg::Model::group_distance` and `Model::group_for`** — the parser already walked
  the five LOD groups to find the meshes; it now keeps each group's own threshold, which was being
  skipped over. `group_for(distance, radius, focal)` is the engine's rule: `metric = focal /
  max(distance - radius, 0.1)`, then walk the thresholds finest first and take the next group
  whenever `metric * threshold < 1.0`. The last group's threshold is the `FLT_MAX` sentinel and the
  rule never reads it — it is the catch-all.
- **`game_render::world::Camera::focal`** — `(height / 2) / tan(fov / 2)`, the one camera term that
  rule reads. It is why the choice follows the *window*: the same rider at the same distance is
  drawn finer into a taller frame and coarser through a wider field of view, which a table of
  distances could not do.
- **`game_render::rider`: `LIMBS`, `anchors_of`, `hold_on`, `holding`, `Body::skin_into_at`** — the
  four limbs (`UprLegL/LwrLegL/Dummy_foot_L` and its three mates), where the chassis wants their
  anchors, and a closed-form two-joint solve that puts them there: bend the knee or elbow by the law
  of cosines so the limb's span matches the distance to the target, then swing the whole limb from
  its root so the anchor lands on it. It is solved on the joint-matrix array between posing and
  skinning, so nothing in the animation or the skin had to change, and the limbs it does not solve
  come through untouched. `Body::skin_into` still means what it did; `skin_into_at(loaded, group,
  hold)` is the one that takes a LOD group and the four targets, and answers how far each anchor
  ended from where it was asked for.
- **the shell and `first-draw`** — the shell reads the chassis's four anchors once (nothing animates
  the chassis there), and every frame it picks the body's group from the camera it is about to draw
  with and skins the body holding onto the bike. `first-draw` gained `--lod N` and `--hold`, and
  prints the group the camera *would* pick beside the one it drew, which is how the stills below
  were made.

### The limb a stunt lets go of

The original aims all four limbs at the anchors every frame, weighted per limb by a number it reads
out of the bone each frame (below). That weight is not identified, and solving all four
unconditionally is visibly wrong on this disc's own data: `superman`'s feet are 6.6 feet off the pegs
and dragging them back destroys the stunt. Neither distance alone nor reach alone separates the two
cases — `cancan` throws a foot that is still inside the leg's reach — so the port decides it by a
window: `holding(off)` is 1 while the anchor is within a foot of where the bike wants it, fades to 0
by two feet, and the limb is left exactly as the clip posed it past that. The disc's own gesture data
has a clean gap there: over all 24 gestures at nine phases each, limbs that are plainly holding on
are within about a foot and limbs that are plainly thrown are 2.33 feet or more. This is the port's
rule, not the original's, and it is the first thing to replace when the weight is found.

## What the original does

- **The level of detail** is `./kit fn 0x001a1300` (the per-geometry draw). It takes the distance
  from the camera's position to the object, subtracts the object's radius, clamps the remainder to
  0.1, and divides the camera's focal length (`camera + 0x178`, the camera being `*0x0048a560`) by
  it. That metric is multiplied by the geometry's own scale (`geom + 0x14`, 1.0 on the body) and two
  globals (`0x00420438` and the per-geometry pair beside it, both 1.0 on this disc), and the group
  advances once per threshold it comes in under. Two things beside it are **not** ported: the forced
  group in the byte at `obj + 0x133` (0xff meaning "choose"), and the fade factor written to
  `obj + 0x148` off a second curve built from the viewport height at `camera + 0x188`.
- **The thresholds** are the `f32` at the head of each group in the `.psg`, which the port already
  parsed past. They are not distances: a threshold times the focal length is a distance, which is why
  the body's five groups hand over at 29, 52, 101 and 329 feet in a 448-tall 4:3 frame and at four
  times those in a 1792-tall one.
- **The limbs** are four controllers of 0x110 bytes, allocated and constructed one at a time in
  `./kit fn 0x00249cb0` (`0x001f3b50` constructs, `0x001f3d20` initialises). That function looks the
  chassis's four dummies up by name — `Dummy_Foot_L`, `Dummy_Foot_R`, `Dummy_Hand_L`, `Dummy_Hand_R`
  at `0x00431b70`..`0x00431ba0` (`./kit data --strings Dummy`) — takes each one's position *less the
  limb bone's own* as the controller's offset, and hands the controller its bone, that offset, the
  target slot it is to follow, and a weight at `+0x530`. The rider actor caches its own four dummies
  the same way in `./kit fn 0x002583d0` (names at `0x00433690`..`0x004336c0`), into `+0x97c`..`+0x988`.
- **The solve** is `./kit fn 0x001f4550`: VU0 macro-mode quaternion arithmetic, about 1100 bytes,
  with **no loop** — it is closed form, two turns and done, not an iterative CCD or FABRIK. It opens
  by copying the weight out of the bone (`*(limb + 0x68) + 0x10`) into `limb + 0x100`, so the weight
  is re-read every frame and a limb can be let go of between frames. It is reached through a function
  pointer (its only reference in the image is the table at `0x00451e0c`), so the decompilation has no
  caller for it to read the call site off.

## How it was proved

`./kit check`: **468 tests pass**, 3 of them new, all in `crates/game-render/tests/body.rs` and all
against the user's own disc (they skip with a reason when there is no disc, and are loud rather than
quiet past that point).

1. **the group falls back as the camera pulls away** — the body's five groups cost
   `[2035, 768, 645, 156, 417]` triangles. With `Preset1`'s 85 degrees across in a 448-tall 4:3
   frame, the focal length is 325.9 pixels and the body reaches 3.37 feet, and the groups hand over
   at **29, 52, 101 and 329 feet**. Pulling away a foot at a time to a quarter of a mile: the group
   never gets finer, every hand-over but the last is cheaper than what it came from, and all four
   happen inside the quarter mile. The rider on your bike (8 feet) is group 0; two hundred feet away
   is group 3, **156 triangles against 2035** — a thirteenth of the cost, which is the thing the task
   asked for. The same rider at the same two hundred feet in a 1792-tall frame is drawn finer, which
   is the rule following the window rather than a distance table.
2. **a solved hand stays on the grip through a gesture** — all 24 gestures this rider can do, at nine
   phases of each, four limbs apiece: **464 limbs were holding onto the bike and every one of them
   ended within an inch of its anchor**, 263 were let go of and every one of them ended exactly where
   the clip put it, 80 were held but at full stretch (the anchor beyond the limb's own reach, which
   is checked to end no further out than it started and within an inch of straight). The furthest an
   anchor started out was 9.34 feet (`Dummy_foot_R` in `rider_rigamortis`); the furthest one that was
   pulled back on was 0.94 feet (`Dummy_hand_R` in `rider_supercanindianair`). Held per limb:
   `[96, 83, 135, 150]`; let go of: `[96, 104, 36, 27]`. Every limb's two bone lengths are preserved
   to 1e-3, and the pelvis, both torso joints and the head are not moved at all — a hand going on the
   bar may not move the body.
3. **a one-hander keeps the other hand on the bars** — three stunts named by hand, at the phase their
   own table holds them at: `onehandindianair` (the left hand holds), `cancan` (both hands and the
   left foot), `superman` (both hands). The holding limbs land on their anchors; the thrown ones stay
   thrown.

Looked at, not committed: `first-draw --pose turn_riding_Mid:0.5 --hold --lod 0/2/3/4` side by side
— all four groups draw the same rider in the same pose at the same scale, getting coarser (group 3
loses the helmet's shape, group 4 is the catch-all and is visibly dearer than group 3 again), which
also says the coarser groups skin off the same skeleton with the right joint indices. And
`--pose onehandindianair:0.5` with and without `--hold`: the thrown arm and both legs are untouched
and the holding hand moves the two inches onto the grip. A 20-second live run of the shell on its
default track with the new path in it: 1195 frames, 59.7 fps, ~20k triangles a frame, no panic.

## Not verified / not 1:1

- **Nothing here was compared against a frame of the running original.** No capture, no side-by-side,
  as in V8. The LOD rule is read off the decompilation and proved against the file's own thresholds;
  the IK is proved against the chassis's own anchors. That a rider drawn this way *looks like* the
  game's is my eye on a still.
- **The solve is not a transliteration.** The original's is loop-free VU0 quaternion code; mine is a
  hinge by the law of cosines and one swing at the root, which is the same two turns in the same
  order but written in the port's own matrix math. Joint angles will differ from the original's in
  the cases where two solutions exist (the elbow's bend direction is chosen by the pose's own bend,
  not by a hint the original may carry).
- **The hold weight is the port's own.** `holding()`, `HOLD_WITHIN` (1 foot) and `LET_GO_AT`
  (2 feet) have no counterpart in the original, which reads a per-limb weight out of the bone
  (`*(limb + 0x68) + 0x10`) every frame and whose source I did not find — the one candidate seen is
  the rider field at `+0x530`, fed as `(vehicle + 0x90) * 0.8`, which is a lean, not a per-limb
  weight. The window is fitted to the gap in this disc's gesture data and nothing else, and a gesture
  on another bike or another rider could fall in it wrongly.
- **A limb whose anchor is beyond its reach ends short, and is left short.** 80 of the 807 limb-poses
  tested are in this state; the worst is `onehandindianair`'s holding hand, **2.9 inches** off the
  grip because the grip is 2.17 feet away and that arm is 1.93 feet long. The original has a clavicle
  above each arm and a wrist below each hand, either of which would close it; neither is in the
  chain this port solves.
- **Riding poses at the extremes of a clip drift and are not corrected.** `turn_riding_Mid` at phase
  0 has a foot 32 inches off its peg and is let go of by the rule above. Whether the original holds
  it on (its weight would say) or whether that is the dab the animator drew, I cannot tell without a
  capture.
- **The two global LOD multipliers and the per-geometry scale are assumed 1.0** — they are 1.0 in the
  image for the body, so the port does not carry them. A difficulty setting or a cut-scene that moved
  them would not be followed.
- **The forced-group byte (`obj + 0x133`) and the fade (`obj + 0x148`) are not ported**, and nothing
  in the port cross-fades between two groups: a hand-over is a pop. The original's fade is a factor
  it hands the material, not a second draw, so what it does with it is a task of its own.
- **Only the rider's body picks a group.** Props in `draw_scene` are still drawn from group 0, as are
  the bike and its parts — their triangles are expanded once at load and the draw has no per-model
  skinning step to hang a choice off. That is the larger half of the frame's triangles and the
  natural next task.
- **An inch is 1/12 of a world unit**, from the disc's unit being a foot (V1/V2). Every "within an
  inch" here rests on that. V8's "a third of an inch" for the anchors of a riding pose is the same
  0.258 world units read as a third of a *foot*, i.e. about three inches — this task's own
  measurements of the same poses agree with the number, not the words.
- **The shell cannot load `sx01`**, so the live run above is on its default track. That is the sim's
  collision tree failing (`crates/game-sim/src/track.rs`), nothing to do with this task, and
  `first-draw` renders `sx01` fine.

## Next

Nothing outstanding for V10. Three things a later task could take up, in the order I would do them:
pick a group for the props and the bike too (the triangles are expanded once at load, so this wants
`Loaded` to keep a group per draw); find the per-limb weight the original reads out of the bone and
replace `holding()` with it; and port the fade at `obj + 0x148` so a hand-over stops being a pop.
