# 1 — flare edge (FINAL)

Task V110. 2026-10-28.

## What I did

- `SunFlare::on_screen` (crates/game-render/src/sun_flare.rs): the sprite's own extent — the sun's
  place in NDC ± its half side (`2 × screen_size / 448` up and down, that × height/width across) —
  tested against the view's edges. `glow` now draws by it rather than by `in_view` (the sun point's
  frustum test), so a flare stays until its last pixel has left. `in_view` stays, documented as the
  original's own test.
- The ride loop's flare pass (crates/openmx/src/main.rs) gates its occlusion query on `on_screen` too,
  so a sun just past the edge is still sampled.
- Occlusion: nothing to change. `sample_rays` are world rays from the eye towards the sun walked over
  the ground (`sky_share`), not reads of a depth buffer, so they read the sky as truly past the edge
  as in the middle, and the share does not snap at the edge. A test pins that (same centre ray and
  same share under a ridge, straight on and past the edge).
- Split views: the test runs the same edge in 640×448, 320×448 and 640×224, each by its own aspect,
  which is what the ride loop passes per view (`rects[v].w/h`).
- Software path: the same test draws the edge glow with `glow::draw` into a `Frame` and checks the
  right column is lit and the left is not; the GPU path draws the same `Glow` triangles and clips
  them itself.
- Nothing taken from opentennis: it has no lens flare.

## What the original does

Draws the flare only while the point twenty feet towards the sun is inside the frustum (V72's
reading), so in the original it too pops off at the edge. The port now differs on purpose, by the
user's ruling of 2026-10-28 (task V110).

## How it was proved

Unit tests in sun_flare.rs: `drawn_until_the_whole_sprite_is_off_screen` (centre just past the right
edge still draws, in three view shapes and through the software rasteriser; 0.002 rad before the
sprite's near edge leaves it still draws, 0.002 rad after it draws none) and
`rays_past_the_edge_read_the_sky`. Rendering, so matched by eye rule, not 1:1.

## Not verified / not 1:1

- Not 1:1 by design: the original hides the flare once its point leaves the frustum; the port keeps
  it while any of the sprite shows (the user's ruling).
- Not looked at in a live ride or a screenshot: the tests check the geometry and the software
  raster, not the GPU picture by eye. The GPU draws the same triangles, so it should match.
- The test of "on screen" is the sprite's square, not the picture's lit pixels: a flare whose
  picture is black near its border is still "drawn" (adding black) for the last few pixels, which
  costs one quad and shows nothing.

## Next

none
