# 1 — farther fog (FINAL)

Task V95. 2026-10-27.

## What I did

- `crates/game-render/src/world.rs`: added a fourth Draw Distance, `Graphics::Farther`. It is the
  original's fog start, fog end and far clip, each `FARTHER_FOG` = 8 times as far. Far is 4 times.
  - Its settings word and `--graphics` word is `farther`. Its label is "Farther fog".
  - `Graphics::ALL` is now Full, Farther, Far, Original, so the menu row steps down in reach.
  - `fog()` builds Far and Farther from one `scaled(k)` closure.
- `crates/openmx/src/menu_pages.rs`: `graphics_label` says "Farther Fog".
- `crates/openmx/src/main.rs`: the `--graphics` error text, the usage line and the doc comment name
  `farther`.
- The GPU path, the wrapped draw's copies and the cull needed no new code. All three read
  `Graphics::fog().reach` / `far_clip()`:
  - `Renderer::render` picks the wrapped world's copies with `Wrap::copies(eye, reach)`, so copies
    go out to 8× the disc's fog end. `MOST_LAPS` (4) still caps them.
  - `Cull::of` takes the far clip.
  - Props fade under every setting except Original, so Farther fades them too.
- Why 8× and not more. On world_ultra the disc's fog ends at 768 ft, so 8× reaches 6144 ft. That is
  about `Reach::DESKTOP`'s 6000 ft tile stream. Past it, only V92's stand-in is there to see. 16×
  measured at half the frame rate (below).
- Nothing in opentennis has a draw distance, so nothing was taken from it.

## What the original does

The original has one fog: the scene's, with the far clip at its end (V32's notes). Farther is an
addition and draws only the picture. The simulation reads none of it.

## How it was proved

- Tests:
  - `gpu.rs` `the_original_is_the_discs_fog_and_far_clip`: Farther's fog is
    `{8×near, 8000, 1, 8000}` on the 1000 ft test scene. Its near, far and reach sit strictly
    between Far's and Full's, and so does its far clip.
  - `the_whole_map_sees_its_far_edge_and_culls_behind`: Farther's cull sees a chunk 5700 ft out
    (Far's does not). It does not see one 8700 ft out, or one behind.
  - `a_wrapped_world_draws_the_far_side_across_its_seam` now loops over `Graphics::ALL`, so Farther
    draws the copy across the seam.
  - `ground_tree.rs`: under Farther (1600 ft on that scene), hills 96² and hills 192² draw the same
    1505 chunks. They look at 829 nodes each, against 9,216 and 36,864 chunks held. The chunk count
    grows with the view, not the map. Far on the same scene draws 402.
  - `menu.rs`: the row steps Whole Map → Farther Fog → Far Fog → Original Fog. Cross and the
    settings file round-trip `farther`, `Settings::parse("graphics = farther")` reads it, and
    save/load keeps it.
- `--timings`, flying high on world_ultra. The command was
  `openmx --headless --size 1920x1080 --mods ~/repos/openmx/mods --track world_ultra --graphics G
  --frames 450 --timings --shot-pose 86689,2000,22053,45,-20,60`. That is 2000 ft above the spawn,
  looking 20° down, with V65's quadtree and MD43's wrap (x every 121150, z every 40060). It is the
  release build, and the machine was shared with other slots:

  | graphics | load | fps (14 s) | session ms all (pace) | present | ground | triangles | nodes avg |
  |---|---|---|---|---|---|---|---|
  | far (4×, 3072 ft) | 9 | 59.9 | 16.68 (11.99) | 1.19 | 0.03 | 109 k | |
  | far | 53 | | 16.70 (6.39) | 2.06 | 0.10 | | 249 |
  | **farther (8×, 6144 ft)** | 16 | 59.9 | 16.69 (9.71) | 2.99 | 0.27 | 453 k | |
  | farther | 35 | | 17.55 (2.57) | 6.09 | 0.56 | | 1360 |
  | farther | 37 | 55.9 | 17.79 (2.29) | 6.20 | 0.59 | 453 k | |
  | farther at 16× (12288 ft) | 49 | 32.4 | 26.47 (0.13) | 10.92 | 1.40 | 847 k | 2599 |
  | full | 31 | | 28.82 (0.00) | 19.18 | 1.18 | | 4333 |

  - At 8×, work without the pace was ~7 ms a frame at load 16, with 4 frames over 16.7 in 450. At
    load 35–37 it was ~15 ms, with fps at 56–60.
  - 16× ran at half that rate.

## Not verified / not 1:1

- **The 16.7 ms is not proven on a quiet machine.** Load was 9–53 from other slots. At load 16, 8×
  held 59.9 fps with 4 slow frames. At load 35+ it averaged 17.6–17.8 ms, just over. The 16× run had
  only one sample, at load 49, so its comparison is not paired. No windowed or vsync run was made,
  and nothing was tried on web or wasm.
- **The software renderer has no Farther.** It draws the original's fog and far clip (or 4000 ft)
  under every setting, as it did before for Far and Full. `main.rs` says so at start-up. The task
  named "the software path", but the software rasteriser is reference only (AGENT.md), so it was
  left as is.
- **Not looked at by eye.** No screenshot of Farther was compared. Past ~6000 ft only V92's stand-in
  is drawn, so the band from 6000 to 6144 ft is the stand-in's.
- **Only one height and one heading were timed**, above the spawn. No crossing ride, and no camera
  near a wrap seam, where copies add passes.
- **Fixed-size scenes.** On scenes whose disc fog ends far out (MD3 mods), 8× gives a much longer
  reach. Nothing caps it below the map's span, so on such a map Farther can cost about what Full does.

## Next

Nothing. The task is done.
