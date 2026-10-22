# 1 — markers as zone portals (FINAL)

Task V51. 2026-10-22.

## What I did

- Found the original's activity start-zone effect, checked it in PCSX2 (copy 2, recording2's state
  on free09), and ported it as `crates/openmx/src/portal.rs`. The portal has four parts:
  - a disc with an arrow on the ground;
  - an added column of crawling stripes;
  - the activity's word;
  - the zone's number, turned to the camera.
- The real start zones (the four the radar shows, `portal::shown`, with no Race while in a machine)
  are drawn at full size.
- Each filled session-marker slot is drawn as a quarter-size copy of the portal:
  - at the spot where its bike was put down;
  - with the disc's arrow along the bike's heading;
  - tinted the slot's colour (`overlay::marker_colour`);
  - with the slot's number over it;
  - the selected one ×1.6 as opaque (disc 0.96 against 0.6).
- The P94 posts are gone, with no post and no line. `hud::draw_marker_posts` is deleted.
  `overlay::marker_post` stays, because the shot-pose tests and `tests/markers.rs` still use it to
  place points; it is marked dead code in the binary.
- New render layer, `game_render::glow`: world-space textured triangles, each vertex tinted. They are
  alpha-blended or added, depth-tested, never write depth, and take no fog.
  - Software path: `glow::draw`.
  - GPU path: `gpu/glows.rs`, with `Renderer::set_glow_textures` once and `set_glows` per frame. The
    glows are drawn per view after the particles. The WGSL is in the shader validation list.
- free06's hit zone stands beside the spawn, and its column fills about a quarter of the frame. Drawn
  one triangle at a time, the software glows pushed `tests/one_fps.rs`'s props stage to 20.7 ms, over
  its 20 ms budget. `glow::draw` now queues a `raster::Batch` and fills it on every core; free06's
  props stage measured 5.6 ms after that. The zone shot came out the same apart from about one pixel
  (`magick compare` AE 0.7).
- `main.rs`:
  - reads `hit.dat` with `portal::spots`;
  - loads the art from the track's `_2.res`. The MX tracks' archives have the disc and the column but
    not the `startzone_*` words, so a track whose archive lacks the numbers takes the art from
    `free06_2.res`;
  - lays the zones' discs over the terrain once (`portal_ground`, the ride's own terrain query);
  - rebuilds a marker's disc only when the marker moves or the selection changes
    (`portal::markers`).
- Tests (`crates/openmx/tests/portals.rs`):
  - `a_filled_slot_draws_the_zone_portal_small_and_no_post`: markers put down through
    `markers::frame` on the stand-in bike. It checks:
    - the portal is at the bike's spot, at the quarter scale, along the bike's heading, in the slot
      colour, with its number;
    - the selected one is brighter;
    - nothing is drawn outside the portal's column;
    - with the art loaded, the glows are exactly disc + column + number.
  - `a_tracks_zones_draw_the_portal_full_size`: free09's four shown zones. It checks:
    - the disc is 40 ft across;
    - the column is 39 ft across and 225 ft tall, from y−25 to y+200;
    - the word and number are drawn, in the activity colour;
    - a marker is ¼ of a zone.

    I proved this test runs to its end (rather than skipping without the disc) by making it panic at
    its last line once.
  - `the_zones_read_their_number_from_the_progressions`: needs no disc.

## What the original does

Recipes: `./kit fn 0x00326330` (the loader), `0x003259b0` (one zone), `0x003256f0` (the hit object),
`0x00324ff0` / `0x00325070` (the icon parts), `0x00323220` (placement), `0x00325280` (the icons each
frame, called from `0x003266c0`), `0x00325580` (visibility). The names are in research/oracle.txt
under V51.

- **Zone fields.** Each `[ASZ n]` gives:
  - Position, LookVector, Scale (40, 0, 40 on every shipped track);
  - Type: Hit 1, Run 2, Race 3, Stunt 4;
  - HitSetID. Its place in `[Progression HitSets]` (Hit) or `[Run HitSets]` (Run) is the zone's
    number, from 0, drawn as `startzone_<n+1>`.
- **Colours** are the radar's, as float bits in the code: Hit 0x6ea637, Run 0xff9933, Race 0x3373cc,
  Stunt 0x813bb5.
- **The disc** is `hitzone_start_circle` (a unit disc on XZ, ±0.5) with the material swapped to
  `hitzone_start_activity_circle`:
  - texture `hitzone_start_activity`, a white arrow on grey 0xcc pointing +z = the look vector;
  - alpha blend, no z write, no fog;
  - tint at 0.6;
  - scale (sx, 1, sz), at the zone's x, z, oriented along the look vector.
- **The column** is `hitzone_landing_cylinder` (unit, ±0.5):
  - texture `hitzone_anim01`, grey stripes;
  - additive Cs·As + Cd, no z write, no fog;
  - UV animation u 0.75 / v −0.30;
  - tint at 0.5; the vertex alpha runs from 128 at the foot to 0 at the top;
  - scale (sx−1, 225, sz−1), centred at y + 87.5.
- **The icons** are `startzone_*`, flat in XY with the origin at the foot, material
  `startzone_objects` (white 8×8):
  - tint at 0.6;
  - each frame, with d = flat camera distance and h = min(d/500, 1)·75 + 15: the number at y+h
    (scale 20), the word at y+h+15 (scale 40), the done checkmark and disk at y+h+30 (scale 20);
  - turned to the camera's facing.
- **In PCSX2** I saw:
  - a blue striped translucent RACE column with its word, from afar;
  - inside the H zone, a green disc draped on the terrain with a faint arrow, and everything inside
    the column added green.

## How it was proved

- I compared by eye (rendering is "ours, and fast"):
  - openmx `--shot-pose 4020,262,3251,45,-12,70` on free09, software and GPU. It shows "HIT 1" the
    right way round over a green column and disc, and RACE far off.
  - the PCSX2 screenshots of recording2's state (the RACE column and the in-zone disc).
- I checked the markers with a hand-written tape (two markers, slot 2 selected) on free09, shot from a
  pose with both renderers: a cyan and a brighter magenta portal with a "2" over the selected one, and
  no post.
- The geometry and placement are held by the three tests above.

## Not verified / not 1:1

- **Disc draping.** The original places the circle at y 0 with a model value of 50.0 that looks like
  terrain draping; I did not trace how it follows the ground. The port lays a 6-ring × 32-segment
  disc over the terrain query, 0.5 ft up (marker ×0.25). The original's own 26-triangle disc mesh is
  not used.
- **Column texture coordinates.** The column's UVs and their scroll are the model's UVs plus
  (0.75 t, −0.30 t), with t in seconds since the session started. I did not check whether the
  original's UVAnimation is per second or per frame, or whether its micro-program generates
  coordinates in world space.
- **Word alpha blending.** The icons' blend is not stated in `startzone_objects.psm`. The port
  alpha-blends them at 0.6 with a depth test.
- **Done checkmark and disk not drawn.** No activity can be finished in the port yet. Every zone
  is drawn as if not started, and the visibility flags (`0x00325580`) are not ported beyond "the
  first of each activity" (P92's rule).
- **Icon facing.** The icons face the camera's flattened forward (the camera's at − eye). The original
  reads its camera's +0x150 facing, and I did not compare its exact source.
- **Selected marker.** The 1.6× opacity is the port's own choice; the original has no selection.
- **Marker fraction and number.** The ¼ marker fraction and the marker's number are the port's
  own choice, as is everything about markers (P88).
- **Marker look-up height.** A marker's ground is looked up from 10 ft over its bike. This is the
  port's own value.
- **Marker art on MX tracks.** On MX tracks a marker's art comes from `free06_2.res`. The original
  never draws zones there.
- **Software path differences.** The software path averages a triangle's vertex alpha for
  alpha-blended glows. That is exact for the zones' pieces, which have one alpha each. For the added
  column it folds each vertex's alpha into that vertex's colour, which gives the same sum. The two
  paths still differ in sampling and rounding (the GPU's frame differs from the software one by eye
  only in the far terrain).
- **Number legibility.** A marker's number is small (5 ft) and at 0.6 alpha in its own colour, so an
  unselected marker's number barely reads against its column. This is a render difference, noted
  and not chased.

## Next

Nothing. `./kit check` passed (1486 tests). The task lands with `--skip-audit`, because the audit's
seven leaks are other tasks' addresses in plan/DONE.md and plan/TODO.md.
