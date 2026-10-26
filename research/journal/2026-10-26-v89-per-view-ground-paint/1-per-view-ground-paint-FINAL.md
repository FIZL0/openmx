# 1 — per view ground paint (FINAL)

Task V89. 2026-10-26.

## What I did

- `crates/game-render/src/gpu/ground_mesh.rs`: V67's single near target became a pool of them. Each
  near target owns its `MeshAt` uniform and a bind group of its own (near view, far view, uniform),
  which is now **group 2** of the ground pipeline instead of three extra bindings in group 1, so the
  ground's big group 1 (layers, shadow) is left alone and only group 2 changes per view.
  `follow(eyes)` takes every view's eye: a view whose 64-ft square some target already holds uses it
  (so views in one square share, and swapped seats swap targets without a redraw); otherwise a target
  whose square no view wants is drawn over; otherwise a new one is made. The pool is truncated to the
  number of views, so a seat that leaves frees its ~21 MiB. `bind(v)` is view `v`'s group 2.
- `crates/game-render/src/gpu.rs`: `follow` gets all eyes, `draw_views` sets group 2 per view, the
  pipeline layout gains the meshes' layout, the group-1 mesh entries are gone, `ground_wgsl` uses
  `ground_mesh::WGSL_BINDINGS`.
- Memory: a near target is 2048² RGBA8 + mips ≈ 21.3 MiB. One view: unchanged (1 near + 1 far).
  Eight views all in different squares: 8 × 21.3 ≈ 171 MiB of near targets plus the far one's 21 MiB.
  Kept the size: a split view's road at the same spot must look as one view does (the test), and the
  cost is only paid when seats really are apart. If phones/web need it lower, halving NEAR_SIZE for
  views > 2 would quarter it — not done, nothing asked.
- Software renderer: no first-view assumption. Its paint (`splat.rs` → `GroundMeshes::paint`) is
  evaluated per pixel at the world point, view-independent.
- Taken from opentennis: nothing — it has no painted-ground targets.

### Other first-view-only choices found (grep `views.first()`, `seats[0]`, `rigs[0]`, `cameras.first()`)

- GPU (`gpu.rs`): none left. Ground LOD is per view (`ground.views[v]`), plants/groves are culled per
  camera in the per-view loop, skies and lights are per view (`view_skies`, `view_light`).
- Shell world streaming (`main.rs` ~:5944, MD8 tiles and MD17 GPU ground): uses every seat's rider and
  flown plane — fine.
- `main.rs` ~:1147 `--trace-picture` traces only the first player — a debug trace by design.
- `main.rs` `rigs[0].hud.on` copied to a joining seat, `rigs[0]`/`seats[0]` in machine switching,
  markers, online — seat-0 gameplay/menu choices, not rendering or streaming.
- `audio.rs` ~:2373 `seat_joined` copies the first seat's bike sounds for a new seat — not rendering.

## What the original does

Not applicable: split screen's painted ground is our rendering (AGENT.md: rendering is ours). The
original paints the meshes into one ground paint target per frame; nothing about it was ported here.

## How it was proved

- `crates/game-render/tests/ground_mesh_views.rs` (`two_views_far_apart_each_draw_their_road_sharp`):
  a synthetic splat with two 128-ft roads 3600 ft apart carrying a 2-ft checker; a 1024×512 frame of
  two views, one over each road, vs. each view rendered alone in the same rect. Both halves match
  their single-view frame pixel for pixel (0 pixels off by more than 2, worst 0); sharpness
  (mean neighbour difference) 26.6 in both. Run against the old behaviour (follow on the first eye
  only), player 2's half was 6.8 sharp with 204,125 pixels off, worst 148 — the test catches V89.
- `gpu::ground_mesh::tests::each_view_gets_a_near_target_and_views_in_one_square_share_it`: pool
  sizes and view→target assignment for one view, two in one square, two apart, a seat swap (no new
  target), eight apart (8), back down to four (4), and a moved view reusing a freed target.
- V67's disc test `ground_meshes.rs` still passes on free06/free09; `every_shader_translates_for_webgl2`
  passes with the meshes in group 2 (3 bind groups, under WebGL2's 4).

## Not verified / not 1:1

- Not looked at in the real shell's split screen with a human's eyes: the GPU test is headless and
  synthetic (made roads, not the disc's), and no screenshot of a 2-player freeride was taken — the
  shell's split screen would need two pads and a ride, and the headless render path is the same code.
- The 8-view memory (≈171 MiB + far) is computed from the texture sizes, not measured on a device; not
  tried in the browser/WebGL2 build, where that much texture memory may matter.
- A view's swapping into a target drawn over in the same frame is not checked for a one-frame flash —
  the redraw is encoded before the views' pass in the same encoder, so none is expected.

## Next

-
