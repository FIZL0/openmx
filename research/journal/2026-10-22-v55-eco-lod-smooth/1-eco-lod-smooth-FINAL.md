# 1 — eco lod smooth (FINAL)

Task V55. 2026-10-22.

## What pops

I rode the shell (`--headless --shot-pose`, backing away from one plant 2–5 feet a shot, both
renderers) past plants on free09 and free06.

- **free09**, the first plant of the file (a TreeC at 3432.6, 191.9, 3744.6) at yaw 42: what the eye
  catches is the **model→billboard swap** at about 335 ft (EcoDistance2D 350 stretched by the fov). It
  is not the shape that jumps but the colour and the light. The model is dark green, lit on one side
  and dark on the other. The billboard is a flat, brighter, yellower blob. There are two reasons:
  - the atlas's texels are 15–30% brighter than the model's own textures;
  - the billboard's up normal takes lambert ≈ 0.64 of the sun, against ≈ 0.25 for the model's upright
    leaf planes.

  Every plant swaps in the same ring, so riding along a row shows a wave of swaps on one frame.
- **free06**, the ScaggBushB at (2980.8, 172.1, 3876.0), yaw 180, eye 40 ft up and pitched −7°: a dark
  twig model swaps for a pale teal blob at about 335 ft. Again the swap is a colour step, and the whole
  ring goes at once.
- **Not a pop:**
  - the far end of the drawing: Original graphics cut at the fog's end, where fog is 1; Full has no
    reach;
  - the small-size cull: `PROP_PIXELS` is 0.5 of a pixel.
- **Found on the way, not fixed:** a small bush's billboard alone changes its cut-out by about 13% of
  its pixels on one frame (free09's BushA at about 360 ft, software). That is its texture level
  changing, and it happens under either rule. It is not the handover; the coverage tests hold each
  frame to it rather than to nothing.

## What I did

All of it is presentation; nothing is read by the sim. `Grove::lod` picks the rule. `Lod::Smooth` is
the default, and `Lod::Original` is the original's look, one field away. There is no menu entry; see
"Not verified" below.

- **A real cross-fade, dithered** (`eco::covers`, the same table in the WGSL `covers`): the model
  draws the share `keep` of a 4×4 ordered dither and its picture draws the complement's `share`. A
  pixel is one or the other, so nothing is sorted or blended. Both renderers have it:
  - software: `Look`/`Mode::cover`, tested per pixel after the depth test;
  - GPU: a per-instance `cover`, with a discard in the fragment shader.

  Prior art: opentennis fades near scenery with the same Bayer table (`crates/hst-base/src/gs.wgsl`).
- **A wider band:** a smoothstep over `max(far − near, 0.3 × mid)`, which is 97.5 ft on free09 against
  the scene's 50 ft. The band ends at the scene's `far`, so no model is kept further out than the
  original keeps it.
- **Per-plant jitter** (`eco::jitter`, a lowbias32 hash of the plant's index): each plant's band
  moves up to `0.1 × mid` (32 ft on free09) nearer, never further, so jitter costs no models.
- **Hysteresis:** a plant forgets its model's turn only `0.1 × mid` past its band's far end, so the
  turn does not flip at the edge.
- **A fade-in at the far edge** (`eco::edge_share`): within the last 6% (×1.0–1.5 per plant) of the
  reach, the plant dissolves out by the same pattern. Only Original graphics has a finite reach.
- **Matched billboards** (`Grove::match_billboards`): each species gets a third model, its billboard
  with vertex paint equal to 128 × the model's mean drawn colour ÷ the billboard's.
  - The mean is weighted by alpha and area.
  - It is lit by the scene's own `Lighting` through `vertex_light`, averaged over 8 turns about the up
    axis.
  - The paint is clamped to 32..255.
  - The smooth rule draws this picture; the original's draws the atlas's own.
- **GPU mips:** box mips are now alpha-weighted, and a pull-push gives alpha-0 texels the colour of the
  coarser level. free06's dark-green atlas holes stopped bleeding into the cut-out's edge. I saw no
  difference at 640×448.
- **Cheaper:**
  - `Grove::draws` culls by a view cone (the frame's diagonal half-angle; sphere against cone) before
    building any matrix;
  - the GPU grove loop indexes its model ids by `Draw::part_index` instead of hashing a name per plant;
  - all of a part's instances are one draw, as before.

## How it was proved

All tests are in `crates/game-render/tests/ecosystem.rs`, against the disc (they skip without it).

- `riding_toward_a_plant_the_{software,gpu}_picture_changes_smoothly`:
  - Setup: free09's first TreeC and first BushA, each alone, rendered at 960×720. The ride covers 520
    to 180 ft at 2 ft a frame. Coverage is the depth buffer's on software; on the GPU it is pixels
    that differ from an empty frame.
  - Measure: coverage × d², frame to frame, over 250–400 ft. Each frame's step is judged against the
    larger of the model-alone and picture-alone steps on that frame.
  - Result: smooth is held to at most +6%, and the original's rule must exceed that. Measured:
    - TreeC: smooth +1.2% / +0.5%, original 18.5% / 8.0%;
    - BushA: smooth +5.4% / +2.3%, original 45.1% / 20.4%.
- `plants_at_one_distance_do_not_change_on_one_frame`: two copies of the TreeC on either side of the
  ride's line change 11 frames apart (314 ft and 292 ft). All 726 indices at one distance change over
  17 frames, at most 55 on one; the original's rule changes all 726 on one.
- `the_densest_view_draws_no_slower`:
  - View: free09's densest one, (2750, 160, 3800), looking north, −3°, fov 85°. It was found by counting
    the plants within 600 ft in an 85° cone over a 50-ft grid × 16 headings.
  - Measure: software `draw_scene` of the grove alone, best of interleaved runs. The smooth rule must
    not be slower than the original's (×1.10 + 0.2 ms slack for load).
- The free09 V48 test now sets `Lod::Original` and checks only the plants in view.
- Looked at by eye in the shell, both tracks, both renderers:
  - free09's TreeC dissolves over about 250–330 ft into a picture of its own colour;
  - free06's ScaggBushB model and picture both read light grey-blue at 1920×1344.

### Before and after

Software `draw_scene` of free09's densest view, grove only, 640×448, best of 38 runs. I ran the same
test in a scratch worktree of HEAD 249b224 and here, alternately, twice:

| | triangles | ms |
|---|---|---|
| HEAD | 1299 | 1.04, 1.06 |
| here, original rule | 1068 | 0.96, 1.05 |
| here, smooth rule | 1066 | 0.98, 1.01 |

The shell's per-stage ms a frame (`--timings --frames 600 --shot-pose`, the densest views of free09
`2750,160,3800,0,-3,85` and free06 `3000,223,3650,270,-3,85`) were taken with HEAD and this build
alternating. The machine was shared with other slots, and `sky`, which this task does not touch, moved
up to 2× between runs, so these are noise-bound. The quietest round:

| stage | HEAD | here |
|---|---|---|
| free09 gpu `hud` (where the GPU's render is timed) | 0.90 | 1.17 |
| free06 gpu `hud` | 3.68 | 3.21 |
| free09 software `scene` | 2.81 | 4.52 |
| free06 software `scene` | 4.78 | 4.72 |

In the same round, `sky` was 1.46 for HEAD and 2.46 here on free09 software, which shows the load.

Over the earlier rounds taken before the band moved:

| | HEAD | here |
|---|---|---|
| free09 gpu `hud` | 0.73–1.19 | 0.86–1.30 |
| free06 gpu `hud` | 3.00–4.06 | 2.45–5.38 |
| `scene ÷ sky`, software | 1.05–1.98 | 1.03–2.06 |

`scene ÷ sky` matched in every round. The in-process numbers in the table above are the ones to
trust.

## Not verified / not 1:1

- The smooth rule is not the original's, by design. Its band, jitter, hold and edge shares were chosen
  by eye on free09 and free06 only. The original's rule is `Grove::lod = Lod::Original`, with no menu
  entry: I kept the change inside game-render rather than thread a new setting through the shell's
  menus and config.
- The matched billboards are a mean-colour match averaged over 8 headings, judged by eye. A flat
  picture cannot have its model's light-and-dark sides, so a careful eye can still see the shading
  change across the band. It is spread over about 100 ft now, not one frame.
- The dither's 4×4 pattern is visible as a screen-door at low resolutions (640×448) during the band.
  This is a known trade for not sorting.
- The GPU mip change (alpha-weighted, pull-push) applies to every GPU texture, not just the atlas. I
  saw no difference in the shots I took; I did not compare every track.
- The GPU ms are from a loaded machine and are not conclusive; only the software in-process numbers
  are a clean before/after.
- A billboard's own texture-level pop (about 13% of a small bush's pixels) is not addressed.
- free06's swap was looked at by eye only, with no coverage test; the tests ride free09's plants.

## Next

Nothing for V55.
