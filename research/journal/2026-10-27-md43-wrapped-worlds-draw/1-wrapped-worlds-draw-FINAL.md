# 1 — wrapped worlds draw (FINAL)

Task MD43. 2026-10-27.

## What I did

MD29 made a world whose file says `wrap` ride round, and MD44 moved the sim's other bodies over the seam. This
task draws it: a rider near a seam now sees the far side's ground across it, not sky.

- **`crates/game-render/src/wrap.rs` (new).** `Wrap { on, side }` describes which axes wrap and the world's
  sides; the world spans `0..side`, as `Tiled` does. `copies(eye, reach)` returns the copies, in whole widths,
  whose rectangles come within the fog's reach of the eye. That is at most `MOST_LAPS` = 4 widths a side, and
  one width a side for an endless reach (Whole Map: the copies' ends and no further). `moved(camera, k)` is
  the camera moved back by the copy's offset, `nearest(from, at)` is a point's copy nearest `from`. Nothing is
  uploaded twice: a copy is the same world drawn again from the moved camera. A unit test checks that a point
  seen from the moved camera clips exactly where its moved copy clips from the real camera.
- **GPU (`gpu.rs`).** `Renderer::set_wrap` takes the world's wrap. In `render`, each view becomes a list of
  passes: the view itself, then one pass per copy its fog reaches (`Graphics::fog(..).reach`: the original's
  fog, Far Fog's 4×, or Whole Map's endless). Each pass has its own view uniform, its own ground view slot
  (`COPY_GROUND` + a mapped index, so a copy's ground culling and LOD are its own) and its own batches. The
  passes draw into the view's viewport and depth. Copies draw no sky, actors, particles or glows. The sky is
  the real view's, and actors are placed by the shell (below). Copy slots no longer used are cleared after
  each frame.
- **Shell (`main.rs`).** Ground and dress streaming get the near points one width off each wrapped side, so
  the pieces across a seam load (`wrapped_near`). GPU actors (other riders, bikes, props) go to their copy
  nearest each view's eye (`nearest_drawn`). In software, `world_dress::draw_ground` also draws every copy
  the original's fog reaches. Software actors, props (culled at their near copy) and plants (from each moved
  camera within the fog) go to the copy nearest the eye.
- **Radar (`hud.rs`).** Other riders and blips (zones, markers) are marked at their copy nearest the rider
  (`radar_near`), so a rider just over a seam sits by the middle instead of being held to the rim from
  across the world. MD42's labels, the map you are on and the one each way N E S W, come from `on_in`,
  `toward_in` and `around_in`. These look through the maps' copies one width off, so east of the last map,
  over the seam, is the first. On the seam's join, past the last map, the map you are on is the nearest one,
  even if it is over the seam. Without a wrap the old functions behave exactly as before.

Nothing from opentennis applied: it has no wrapping world.

## What the original does

Nothing: the original's worlds do not wrap. This is the port's own feature (MD29's switch, off by default).

## How it was proved

- `crates/openmx/tests/wrapped_world_draw.rs` (1 test, both renderers) runs the shell's binary on MD29's
  generated world (two 567 ft tiles and a 135 ft seam join; wrapped `md29_round`, edged `md29_edged`). Its
  `--shot` is posed 30 ft short of the far seam (12–20 ft over the ground), looking east and 15° down, at
  320×224 with no overlay. The box between the horizon and the picture's middle covers the ground from 45 ft
  ahead, past the seam. On the wrapped world all of it differs from the sky above it (share 1.0, GPU and
  software). On the edged world, at its edge, none of it does (share 0). I looked at the four pictures. The
  GPU one also shows the standing rider (at x 200) as a dot 230 ft ahead, in the copy over the seam.
- `game-render` gpu test `a_wrapped_world_draws_the_far_side_across_its_seam`: on a headless renderer, for
  every `Graphics` setting, a camera 10 ft short of a flat 800 ft world's seam sees fog without a wrap. With a
  wrap it sees ground, and more chunks are drawn. The same frame again is pixel-equal, and unwrapped again
  is back to fog.
- `game-render` `wrap::tests` (2): which copies a view reaches (one side, neither, two laps, the endless
  reach, a corner), moved-camera clip equality, and `nearest`.
- `hud.rs` tests (2): a rider 20 ft over the seam is marked under a quarter of the rim's radius from the
  middle, and on the rim without the wrap. Labels: A both ways along x from B when wrapped, once without, and
  nothing across z. The map you are on at the join is A over the seam.

## Not verified / not 1:1

- **Software ground for a posed camera is still gathered round the rider.** This was so before this task.
  A copy's pieces are now taken round the rider's copy nearest the moved camera, so the far side shows. But
  the seam join under a `--shot-pose` eye 1000 ft from the rider is not drawn (fog in the picture's bottom
  third). A ridden camera, by the rider, draws it.
- **Roads and ruts in a copy** use the real view's mesh binding. The copy's roads lie far from the copy's
  window and may not draw across a seam. No world with roads and a wrap exists to look at.
- **Particles and glows are not drawn in copies.** Dust or lamps just over a seam show only on their own
  side.
- **Actors take their copy by their own origin.** A group spanning the half-width line between copies (far
  from the eye) could split. Not seen; no test.
- **The radar's nearest copy rounds** (`f32::round`), while the sim's `nearest_copy` moves only at strictly
  more than half a width. A rider exactly half a world away may be marked on the other side from the sim's
  pick. That is at the rim either way.
- **Streaming loads one lap of copies** (`wrapped_near` ±1 width). On a world narrower than Far Fog's reach,
  copies two laps out draw only the pieces already loaded for the first lap.
- **Software plants** may be drawn twice when the original's fog reaches more than half a width (two copies'
  cameras both within reach of a plant). Not seen on any world.
- **Translucent draws across copies** follow pass order (world, then copies). Overlapping translucency
  between a copy and the world is not sorted together.
- **Whole Map's far clip** reaches only the copies' ends (one width a side), as the brief asks. Nothing
  past them is drawn.
- **No real wrapped world was ridden in a window.** Only generated worlds, headless.

## Next

Nothing: the task is done.
