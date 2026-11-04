# 1 — app switcher ground (FINAL)

Task MD97. 2026-11-04.

## What I did

The brief listed three suspects. I looked at the cheapest one first: holes in the installed world itself.

- **The desktop's world_mxvatv has no holes.** A new ignored test,
  `worlds::tests::every_tile_of_the_installed_world_mxvatv_has_ground`, builds every piece of the installed
  world: 27 tiles and every join, at full detail with `Builder::build`, the ride's own path. It then samples the
  world every 40 ft. Wherever `Tiled::owner` gives a piece the ground (the ride's ground), that piece's drawn
  chunks must cover the point. Each tile's corners must also not all sit at one height (the MD95 kind of
  flattening). It passes against `~/repos/openmx/mods` (67 s). Some joins (`j013_011`, `j014_013`, …) build
  0 chunks. The tiles own all of their ground, so that is not a hole. My first version of the test flagged
  them wrongly.
- **Missing tile files can't produce holes either.** A world whose tile folder is gone already failed `find`
  (the terrain's shape read fails). The store's install verifies a world with `find` (`modstore.rs`,
  `Kind::World`), so it never installs a broken one. The error didn't say what was needed. Now
  `read_watched` refuses a missing tile folder with "`<world>: <tile> is not installed: the world needs
  <game>'s files`". The game comes from mod.json's `source.games` (or `source.game`). New test:
  `a_world_whose_tile_files_are_missing_is_refused_with_the_game_it_needs`. The track pick still lists the
  world, and the load then fails with this line. I didn't change the pick.
- **The cause: GPU work made while the iPhone app is away.** The user said "cause i went to app switcher
  while loading". The ground goes up through `queue.write_buffer` (`Renderer::put_ground_piece`). When a page
  fills, a "ground grows" submit copies the old page into a bigger one. In the background, iOS refuses the
  app's Metal command buffers. A refused write leaves its piece's vertices zero, and the piece is marked held,
  so it is never asked for again. A refused grow copy is worse: it empties every piece already in that page.
  That gives scattered missing ground "for good", as reported. Before this change nothing in the load or the
  ride looked at whether the app was in front. `draw` skipped frames, but the load's uploads and the ride's
  `GroundStream` hand-over went on.
  - `ios_gpu.rs`: `in_front()`, and `times_away()`, a count of the times the app went from in front to
    away. `suspended` and `Focused(false)` go through `Shared::set_focused`. `gpuwin.rs` and `web_gpu.rs`
    have the same two functions, returning `true` and `0` (desktop and browser GPUs take work unfocused).
  - `main.rs`: `wait_in_front(what)` blocks the load until the app is back. It logs
    `loading: <what> waited N s …`. It runs before the upload stage's first GPU call, before the stand-in,
    before each ground piece of the start, before the paint stream's start, and before each slice of models.
  - The ride loop doesn't hand the ground stream's (or the paint stream's) changes over while the app is
    away. They stay pending in the stream. After the app is back, `times_away()` differs from the count
    taken before the load's uploads. Then `GroundStream::build_again` marks every held piece. The stream
    builds each one again at its detail and hands it over in place of the GPU's copy (`redo`). The old copy
    stays drawn until then. Log line: `world ground: back in front at tick N, M pieces built again`. This
    covers an upload made in the moment between the check and iOS's refusal. It also covers an away during
    the load.
  - New test: `the_ground_held_is_built_again_after_the_app_was_away`. After `build_again`, every held
    piece comes back once, as one `Add`, with bit-identical corners. Each gets a `Show`, nothing is dropped,
    and the stream then rests (no rebuild every frame).
- `tools/ios-build.sh check -p openmx --bin openmx` compiles the iOS target clean.
- opentennis: nothing to take. It has no handling of GPU work across the background.
- plan/HUMAN.md has the iPhone check: "Ground missing after the app switcher on the iPhone (MD97)".

## What the original does

Nothing to port: this is the app's GPU streaming, not the game.

## How it was proved

- Unit tests (crates/openmx bin, `worlds::tests`): the rebuild after an away, and the refusal that names the
  game.
- The installed-world check above, run by hand with `OPENMX_WORLDS=~/repos/openmx/mods cargo test -p openmx
  --bin openmx -- --ignored every_tile_of_the_installed_world_mxvatv`: passes.
- The background path itself isn't proved on a phone (see below).

## Not verified / not 1:1

- **Not run on the iPhone.** No device here, and no iOS log of the bad ride on disk. That iOS refused the
  uploads is inferred: from the user's app-switcher remark, from how wgpu stages `write_buffer` and the
  grow copy into submits, and from Metal refusing background command buffers. No log line confirms it. The
  user's check is in plan/HUMAN.md (MD97).
- **When winit reports `Focused(false)` / `suspended` on iOS against when Metal starts refusing is
  unchecked.** I assume resign-active comes before the refusal. An upload that lands in a gap between the
  two is caught for the ground by `build_again` only.
- **The paint stream's pages and the dress stream's models and textures aren't rebuilt after an away.**
  Paint updates now wait while the app is away. The dress stream (props, cars) isn't gated at all, because
  `world_dress::take` also feeds the place's collision and I didn't want to touch that here. A refused
  upload there would show as a missing prop or a dirt-coloured map, not missing ground. Filed as a
  follow-up.
- **The load's own uploads (scene, sky, models, stand-in) wait for the front but aren't redone** if one slips
  through just as the app leaves. Only the ground is redone.
- The track pick still offers a world whose tile files are missing. It fails at load with the new line
  rather than being greyed out in the pick. No store-installed world can be that way (`find` gates the
  install).

## Next

The human's iPhone check (plan/HUMAN.md, MD97). If the log still shows missing ground with the
`back in front` line present, the refusal is somewhere other than the uploads: look at the surface and
the drawables next.
