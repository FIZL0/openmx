# 1 — prop see through (FINAL)

Task V107. 2026-10-28.

## What I did

- Found the cause. The blur cone (`propconeblur`) has three materials. Two are opaque, and the third,
  the 64x64 spinning disc (alpha 0..160), is blended, yet its `.psm` leaves depth writes **on**
  (`Surface { blend: true, depth_write: true }`). So the disc wrote depth wherever it drew:
  - **GPU** (`gpu.rs` `draw_views`): opaque runs, then every blended run in batch order, unsorted.
    An actor with blended surfaces that came after the plane failed the depth test inside the disc.
    On free06 that is exactly the caboose: `train_car_08`'s one material `mm_caboose` is blended and
    writes depth, and every other path car (engine, flatbeds, coal cars, boxcar, dump truck,
    scraper, company truck) is opaque. That is the user's "rear train carriage". Opaque actors were
    fine on the GPU, since they draw before all blended runs.
  - **Software** (`world::draw_scene`): no translucent pass at all. Triangles go in the actors'
    order, and the shell queues the plane before its pilot, the traffic, creatures, toys and the
    tiles' props and plants. All of those, opaque or not, vanished inside the disc.
- The fix:
  - `Biplane::load` turns depth writes off for both cones' blended surfaces (the bent cone has none
    today; the loop covers it anyway).
  - The GPU's model pass now runs three phases: opaque, then blended that writes depth, then blended
    that does not. A surface that writes no depth can hide nothing, so drawing it after everything
    behind it is always right.
  - The software `draw_scene` queues every triangle that is blended and writes no depth (`late`)
    after all the others, in scene order. `draw_model` does the same for one model.
- Nothing taken from opentennis: its renderer is Bevy's and has its own transparency sort.

## What the original does

Not read. Rendering is ours (AGENT.md), and the GS draws the prop in the original's own order,
which we do not replay.

## How it was proved

`crates/openmx/tests/prop_see_through.rs` uses the disc's plane and blur cone. The camera sits behind
and above the nose and looks ahead through the prop at a red wall 30 ft ahead. The wall is drawn
after the plane, once blended with depth writes (as the caboose is) and once opaque, on both
renderers. The disc's pixels are where adding the cone changes the plane-only picture (1375 software,
1357 GPU), all of them over the wall. Each pixel must show the wall through the disc (redder than the
cone over the fog) and must still have the disc over the wall (not the bare wall).
- Fixed: 100% / 100% in all four cases.
- Old code (source files checked out from HEAD for one run): the wall showed through 0 pixels of the
  disc on the software path for both walls, and on the GPU for the blended wall. The opaque GPU wall
  was fine, which matches the diagnosis above.
- Cone depth writes off but no new order: the wall shows through, but the disc is over the wall in 0
  pixels, so the order change is needed too.

## Not verified / not 1:1

- No shot in the shell itself, either `--shot-pose` at the train on free06 or a flown plane. The proof
  is the test scene above with a stand-in wall. The caboose's material flags (blended + depth write)
  were read off the disc and match the wall's, but the caboose itself is not in the picture.
- Not enumerated by picture: which other actors' materials are blended and write depth. The GPU lost
  them inside the disc too, and are now fixed by the same rule. The free06 path cars were listed; bikes,
  riders, creatures and toys were not.
- Blended surfaces among themselves are still not sorted back to front on the GPU. Two depth-free
  translucent surfaces overlapping one another (the disc and, say, a glow or a decal) draw in batch
  order. Rendering is ours, and this goes no further than the task asked.
- The new phase order also moves every other depth-free blended material (any `.psm` with
  `zWriteDisabled`) after the depth-writing translucent ones, on both renderers. By the reasoning above
  this can only fix pictures, but no world was looked at for it beyond `./kit check`.

- Landed with `--skip-audit`. `./kit audit` fails on the 7 known leaks that were already there
  (plan/DONE.md:121 radar_update, and :558 the six biplane_tire_* lines). None of them are this task's text.

## Next

Nothing.
