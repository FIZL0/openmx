# 1 — world disc roads (FINAL)

Task V115. 2026-10-30.

## What I did

The user: "roads and textures still look like crap on world map, do they never load in fully when
close?". They never did: `Renderer::set_ground_streamed` built V67's screen-space meshes (roads,
ruts, runway, dirt loops) only for a splat-painted track ridden alone. A world's disc maps got MD35's
per-pixel splat, times a 128-px "modulate" page that held the roads and the light at 4.5 ft a pixel,
so every road and rut in a world was that page's smear, at every distance.

- `GroundMeshes::joined` (game-render `ground_mesh.rs`): several maps' meshes as one set, each moved
  by its map's `at`, its textures appended and the passes' texture indices shifted.
- `gpu::world_mesh_tiles` / `gpu::world_meshes`: every packed disc map with a sharp splat and meshes,
  joined. `set_ground_streamed` gives a world that one `GpuMeshes` (the same near/mid/far targets
  round each eye as alone, `follow` unchanged), when the device samples the world's textures + 2 + the
  three targets; otherwise it leaves them out and the log says so.
- The brief asked for one texture array for every tile's mesh textures rather than two bindings a
  tile. Not needed: with all tiles' meshes in one set, the ground shader binds only the three targets
  (group 2, as alone) whatever the number of maps; the mesh textures are bound only by the pass that
  draws into the targets, one bind group a texture, outside the ground's sampled-texture count.
- `ground_world_meshed_wgsl` (wrapper: `ground_world_wgsl` = not meshed, so other callers are
  unchanged): with meshes, the WGSL gets group 2 and `ground_mesh::over_wgsl`'s `mesh_over(p, gx, gy,
  frame)`, `apply_wgsl`'s near/mid/far mix read with `textureSampleGrad` (a map's block is
  non-uniform control flow), called in each sharp disc map's block after its passes at `q + at`, before
  the modulate — the order the original paints in (passes, meshes, then the light).
- `world_dress::bake_splat_sharp`: the modulate page is now the baked colour over the paint *with* the
  meshes (`paint_at`), i.e. the light alone; the roads are no longer in it twice.
- main.rs `world paint:` gets a second line: `world paint: roads and ruts of mxu_free06, mxu_free07,
  mxu_free09, mxu_free10, mxu_free11 (103802 triangles, 11 textures)` (or "left out: the device
  samples too few textures", or "no map has roads or ruts").

Nothing taken from opentennis: it has no streamed world ground.

## What the original does

Not a simulation change. The original draws a freeride scene's `TerrainScreenSpace` models into the
ground's paint target after the splat program and before the light-map tiles (V67's notes). It has
no worlds; the world is ours (MD9).

## How it was proved

- `ground_mesh::tests::joined_maps_keep_their_own_textures_where_they_were_moved`: two maps' meshes
  joined paint exactly as each alone at its offset, nothing between.
- `gpu::tests::every_shader_translates_for_webgl2` gains "ground, world with roads": two sharp disc
  maps with meshes; `world_mesh_tiles` names both, the joined set reaches the second map's offset, the
  shader has one `mesh_over` a map, and it validates and translates to GLSL ES 3.00.
- `crates/openmx/tests/world_disc_roads.rs` (ignored: user's mods, disc, GPU, release): free09 alone
  at `--trace-at 1872,4500,180` vs world_mxu_free at `12816,9972,180`, and free06 alone vs world at
  `2500,2500,90`; RMSE over the middle-distance ground beside the rider (rows 30–55 %, left and right
  thirds), light level matched per channel. Ran `cargo test --release -p openmx --test
  world_disc_roads -- --ignored` with DISPLAY=:0: **free09 0.0198, free06 0.0234 → pass**. The same
  metric on HEAD's build (scratch worktree, same shots): free09 0.0305, free06 0.0236 — it would fail.
  The roads line names every map's meshes.
- Shots by eye (context/shots/v115, `shoot.sh` + `cmp.sh` + `gm.py`): the world's free09 had a smooth
  wash where alone has the road's ruts; now it shows them as alone. The rut spot (`1872,1900,0` /
  `12816,7372,0`) goes 0.0224 → 0.0106 on the same metric. Ground texture spread (std dev, left third)
  free09 alone 13.6, world before 8.9, after 12.3. free06 is unchanged (0.0399 vs 0.0400 raw).
- The brief's follow-up question, whether the baked 128-px pages still show soft in the middle
  distance: no longer applies to disc maps. Their colour is the per-pixel splat + meshes at every
  distance; the baked page only scales it by the light now. The middle-distance band above is where
  the metric reads, and it is below free06's.

## Not verified / not 1:1

- **The brief's raw RMSE target is not met, and is not this task's to meet.** Unmatched, free09 world
  vs alone is 0.092 (thirds below the horizon) vs free06's 0.040: the world lights free09 ~20 % darker
  (left-third mean 82 vs 103) by another sun — the bike's shadow points another way — and its sky
  tint differs. That is the world's lighting, not the paint; the test matches the light level and
  leaves out the rows the shadow falls in. Filed as V116.
- Across a wrapped world's seam, a map's block reads its nearest copy (`mirror`), and the meshes are
  read at that copy's point: if the eye is on the other side of the seam, those meshes come from the
  mid/far targets rather than the near one (not looked at; seams have no disc-map roads in the user's
  worlds that I know of).
- At a join between maps, each map's mirrored read lays its own meshes at the mirrored point, so a road
  running into a join is mirrored back in the blend band, as the map's paint already is.
- The far target (2048² over the joined meshes' whole rectangle, ~8 ft a texel on world_mxu_free) is
  coarser than alone's (over one map); it is only read past `MID_FADE` (880 ft).
- When the device cannot sample world textures + 5, the roads are left out *and* the modulate no
  longer carries them, so that device shows no roads at all (logged). Not seen on this machine or in
  WebGL2's limit for the user's worlds (not run in a browser).
- Frame time: world_ultra headless 960x540, 120 frames, `--timings`, on a busy shared machine: ground
  3.08 ms a frame vs 2.80 on HEAD (present ~27–31 ms dominates both; load noise is of that order).
  world_ultra's log names the same five disc maps' roads. Not measured on a quiet machine, a phone or
  in a browser.
- The ignored shot test does not run in `./kit check --all` (it needs the user's mods, the disc, a
  GPU and a release build); it was run by hand as above.

## Next

V116: light a world's disc maps by their own sun.
