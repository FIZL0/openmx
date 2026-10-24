# 1 — per view world sky (FINAL)

Task P142. 2026-10-23.

## What I did

A combined world's split screen gives each view its own map's sky and light (user, 2026-10-23: "permap
skyboxes on local should be different between players, right now it takes p1").

- `crates/openmx/src/main.rs`: the world-sky block (MD9, MD26) now loops over the views, not
  `cameras.first()`. Each view looks up its own `Lighting` and sky pair and share from its own camera
  (`wl.looks.at` / `wl.looks.skies`). `shown_look` and `shown_skies` are now one per view. The log line reads
  `world sky: over <map> at x,z in view N: ...`, one line a view each time that view's map changes. The
  `world sky: over` prefix is kept, so the MD9 tests' `contains("over b at …")` still holds. `light` is still
  there and is view 1's; the new `lights` holds every view's. The software path lights, fogs and clears each
  view with its own light (it used `look.fog_color` before). Each seat's software far plane takes its own
  view's fog end.
- `crates/game-render/src/gpu.rs`: the renderer holds per-view state next to the shared sky:
  - `set_view_skies(v, Some([(key, sky); 2]))` gives a view its own crossfade pair, and `None` puts it back
    under the shared `set_sky` / `set_skies` sky. Global uses `None`, so it draws exactly what it drew before.
  - Cubes are cached by the caller's key (the world map's number). Each map's cube is uploaded the first
    time any view is over it and is kept until `forget_assets`. A view moving between maps it has seen
    only rebuilds its bind group, and one with the same pair does nothing.
  - `set_view_sky_mix(v, share)` writes that view's own 16-byte mix uniform.
  - `set_view_lights(&[Lighting])` gives each view its light. `render` takes each view's light for its
    fog, far plane, cull, sun, ambient and fog colour, and takes its `has sky` flag from the view's own
    pair. The draw binds the view's own sky group. `view_light` and `view_sky_cubes` are there for tests.
  - The `View` and `Scene3` structs are unchanged on purpose: 12 literal constructions across game-render's
    tests would otherwise have had to change, and so would any that other slots are adding now.
- A second `--shot-pose` poses view 2, a third view 3, and so on. The split-screen tests need this, and so
  did the 4-view timing run below.
- No `sky.rs` or `lib.rs` change was needed: the cube decode is untouched, and the crate's modules are as
  they were.

opentennis has no per-view skybox or lighting, so nothing was taken from it.

## What the original does

The original has no combined worlds and no Per Map sky; this is the port's own feature (MD9). In the
original's split screen both views are on the same track, so they share its one sky and light. That is
what a disc map still gets here: `world_look` is `None`, nothing per-view is set, and every view takes
`Scene3::light` and the shared sky.

## How it was proved

- `game-render` `gpu::tests::each_view_draws_its_own_sky_and_light` (headless GPU, two views side by side):
  - view 0 draws red and view 1 draws blue, to within 1.5 levels per pixel;
  - view 0 crossfades red→blue at shares 0, 0.3, 0.6 and 1 while view 1 stays blue at every step;
  - two maps visited back and forth leave 2 cubes uploaded;
  - a view whose map has no sky draws its own light's fog colour beside a sky view;
  - after `None` / `set_view_lights(&[])` both views draw the shared sky again.

  I confirmed the test runs, and does not skip, by raising the cube count to 3, which turned it red.
- `crates/openmx/tests/world_look.rs`, through the shell binary on the generated MD9 world. The tiles now
  have generated one-colour `.cub` skies (`cub()`: 96 one-strip packets), `a` orange and `b` blue:
  - `split_screen_views_each_take_their_own_maps_sky`: two views, P1 looking up over `a` and P2 over `b`
    under Per Map. Each view's middle half is its own map's sky to within 3 levels. Each view's log line
    names its map and has its fog (and `b`'s ambient in view 2). Under Global both views show `a`'s
    (the whole world's) sky, and both lines say "the whole world".
  - `a_view_crossing_a_join_leaves_the_other_views_sky`: view 2 over `b` is byte-identical whether view 1
    is over `a` or over the join's middle. View 1 over the join shows a red/blue value strictly between
    the two skies.
  - `split_screen_views_each_take_their_own_maps_fog`: with no skies, each view's backdrop is its own map's
    fog colour, on `--renderer software` and on the GPU.

  I confirmed both the sky and fog tests run by flipping view 2's expected colour, which turned both red.
  I then restored them.
- `--timings` of a 4-view world, 1280x720, the generated world with skies, `--fps-limit off`, two runs each.
  The headless loop still paces to 60, so the per-stage ms are the numbers to compare, not fps:

  | build | cameras | triangles | sim | anim | hud | pace | all |
  |---|---|---|---|---|---|---|---|
  | HEAD (4994458) | ride cameras, all over `a` | 149144 | 0.49 / 0.39 | 0.13 / 0.05 | 1.20 / 0.81 | 14.81 / 15.40 | 16.67 |
  | P142 | ride cameras, all over `a` | 149144 | 0.44 / 0.43 | 0.07 / 0.05 | 0.97 / 0.93 | 15.16 / 15.22 | 16.67 |
  | P142 | posed over a, b, join, a | 118888 | 0.38 / 0.39 | 0.05 / 0.05 | 0.73 / 0.94 | 15.47 / 15.25 | 16.67 |

  Before and after are the same within run-to-run noise. HEAD cannot pose views 2–4, so the per-map split
  run exists only on P142's side.

## Not verified / not 1:1

- **The software path draws the world's home sky in every view.** It does not draw each view's own map's
  sky. Light and fog are per view (tested), but `sky.draw` is still the scene's one sky, as before this
  task. The brief's "it draws no skybox" does not match the code (it does draw one). I left that sky alone
  so that one view on the software path is unchanged. Where a map has a sky, the software picture shows
  the home sky above a view's own fog.
- **The software backdrop under Per Map changed for one view too.** Each view is now cleared with its own
  light's fog colour, not the scene's (`look.fog_color`). This matters only where the home sky has gaps,
  and it now matches the fog the ground fades into. It is not tested on its own for one view.
- **GPU draw time is not broken out by `--timings` here.** The headless offscreen run reports `present
  0.00` and paces to 60. The timing table shows the CPU stages are unchanged; it does not show the GPU
  cost of up to 4 more bind-group switches per frame. Those are only switches, with no uploads.
- **Changing the World Sky setting mid-ride with many maps visited** keeps every visited cube resident
  until the next ride's `forget_assets`. That is one cube per world map at most, as the brief asks, but
  memory with a large world was not measured.
- **Online** was not run. Each machine draws its own seats' views through the same code, so the fix applies
  there unchanged.

## Next

Nothing.
