# 1 — biplane particles (FINAL)

Task P62. 2026-10-21.

## What I did

- `crates/game-render/src/particle.rs` (new): `Particle` (P58's `gpu::Particle` without the GPU padding;
  the `gpu` module is behind a feature, and this one is not) and `draw(frame, camera, sheet, particles, light)`.
  - It keeps the GPU's rules from `gpu/sprites.rs`:
    - `size` feet across, centred on the point, its sides along the view's right and up;
    - sheet top towards the view's up, sheet left to the view's left;
    - colour is the texel's, unlit;
    - alpha is texel × particle × 2, capped at 1;
    - blended as `(Cs − Cd) × As + Cd`, depth-tested, no depth written;
    - drawn in the order given.
  - Fog is the software path's own: `Lighting`'s fog by distance from the eye, as `queue_model` does for
    the props. That is the original's fog only; `--graphics full` does not reach the software path.
- **Not two triangles.** A first version queued each square as two triangles into a `raster::Batch`. Its
  test caught a darker diagonal: the rasteriser has no fill rule for shared edges, so a pixel on the diagonal
  was blended twice. The fix uses the fact that a square facing the camera lies in a plane parallel to the
  screen. All of it is at one view depth, so it projects to a pixel-aligned rectangle. `draw` fills that
  rectangle directly: pixel centres in `[x0, x1) × [y0, y1)`, one depth test against `1/w`, and `uv`
  linear across it. Nothing in `raster.rs` was changed, so other draws are unaffected.
- Texels come from `Texture::sample_smooth` (bilinear, like the GPU's linear sampler), held half a texel
  inside the particle's cell so no neighbouring cell mixes in. I tried nearest texels first; the puffs looked
  blocky beside the GPU shot.
- `crates/openmx/src/main.rs`:
  - `soft_sheet` is a clone of the particle sheet taken at load, because the GPU path `take()`s its copy on
    its first frame.
  - `plane_particles(fx)` converts `PlaneFx::drawn()` into the software type.
  - The software view loop calls `particle::draw` right after `draw_scene`. Each square adds 2 to the
    triangle count.
- Nothing came from opentennis: it has no software particle path.

## What the original does

Nothing new. P58's journal covers the particle system. The draw itself is presentation, matched by eye
(AGENT.md: rendering is ours).

## How it was proved

- `crates/game-render/tests/particle.rs`, 8 tests, all passing under `./kit check`. They use synthetic
  frames, an 8×8 four-cell sheet and a level camera, and check:
  - an opaque texel at particle alpha 0.5 comes out as exactly the texel (the ×2);
  - a faint one blends by exactly `(Cs − Cd) × As + Cd`, and every covered pixel gets the same value
    (no doubled diagonal);
  - sheet orientation: red top-left, green top-right, blue bottom-left, white bottom-right on screen;
  - a cell never picks up its neighbour's texels, to every edge;
  - a nearer depth hides the square, the square writes no depth, and a particle behind the eye draws nothing;
  - the square stays square when seen along +x and from straight above;
  - fog at half the range is exactly halfway to the fog colour;
  - zero alpha, zero size, an empty list and transparent texels draw nothing.
- **Live**, in the shell: `openmx --vehicle biplane --play <Cross-held tape> --headless --frames 400
  --shot … --shot-frame 340 --size 640x448`, once with `--renderer software` and once with `--renderer gpu`.
  - Both shots show the white smoke trail behind and under the plane at take-off, in the same places and of
    the same size and softness. The per-second line reported `Smoke 96`.
  - The software run held 59.8–59.9 fps at 640×448 (60 Hz display) with the smoke up.

## Not verified / not 1:1

- **Matched against the GPU path by eye, not against the original.** It follows P58's choices (size, ×2
  alpha, unlit colour, linear fades), which are P58's own guesses, because Vu1Particles.vuo is not read.
- **Software and GPU pictures are not pixel-identical**, and were compared only by eye at one moment:
  - the GPU fogs with `Graphics`' fog (and `full`'s haze); the software path uses the scene's fog, as its
    models do;
  - the GPU draws two triangles through its own rasteriser; the software path fills a rectangle;
  - the GPU samples with hardware bilinear; the software path uses `sample_smooth` clamped inside the cell.
- **A square crossing the near plane is dropped whole** (`w <= camera.near`). The GPU clips it. Both only
  happen with smoke within 0.05 ft of the eye.
- **No interpolation between ticks** (P52), same as the GPU path: particles are drawn where the last tick
  left them.
- The triangle count adds 2 per square to make the per-second line comparable. Whether the GPU's count
  includes its sprites was not checked.

## Next

Nothing.
