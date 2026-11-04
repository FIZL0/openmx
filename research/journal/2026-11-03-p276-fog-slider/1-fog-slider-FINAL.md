# 1 — fog slider (FINAL)

Task P276. 2026-11-03.

## What changed

- `game_render::world::Graphics` has a new `Times(u8)` variant. It is the original's fog and far clip
  `k` times as far out. `Graphics::times(k)` uses the existing names for 1 (Original), 4 (Far) and
  8 (Farther), so each distance has one value. `multiple()` gives `k`, or `None` for Full, and
  `fog`/`far_clip` read it. Original's fog and clip are unchanged (k = 1).
- `Graphics::SLIDER` lists the steps: Original, 2, 3, 4, 6, 8, 12, `FOG_MOST` (16), Full.
- Why 16 for the far end: on a 1,000 ft fog it clips at 16,000 ft. That is still a finite reach,
  which the GPU cull and ground streaming can stop at, and it is no more than Whole Map's own reach
  (2x the map's span, at least 10,000 ft). Whole Map stays as the last step, with no fog end.
- settings.txt words: original, far, farther and full are as before. The new steps are x2, x3, x6,
  x12 and x16. `--graphics` takes them all.
- The menu (menu_pages.rs `fog_row`): the row is named "Fog" and is a `Kind::Slider` on
  `Key::DrawDistance`.
  - The value is "Original", "2x" ... "16x" or "Whole Map". The note says "N times as far as the
    original's."
  - Over a ride (the pause's Settings), the value also gives the fog's end in feet on that track,
    e.g. "8x · 8,000 ft". The shell sets `Menu::ride_fog` from the scene's fog end when the ride
    starts. In the front end there is no track to measure on, so it shows only the multiple.
  - Left and Right step and stop at the ends. Cross steps further, and from Whole Map back round to
    Original, as other sliders do.
- The Original switch's Fog sub-switch: on sets Original, and back to ours sets the default, Farther
  (8x). Turned back from the row, Right from Original is now 2x, not P269's Whole Map. The test
  expects that.
- video.rs `cheaper`: anything further than Far steps in to Farther (or to Far from Farther and 6x), so
  the new steps follow the same ladder as before.

## Checked

- `cargo test --release -p openmx --test menu`: 256 passed. New tests:
  - `the_fog_slider_runs_from_the_originals_to_the_whole_map`: the ends, the 8x default, each step's
    label, the meter, Cross wrapping, each step saved and read back, and the old words.
  - `the_fog_slider_shows_its_distance_and_the_ride_fogs_by_it`: the feet over the pause and the
    plain multiple in the front end. It also checks that each step's fog and far clip are k times the
    light's, and that 16x reaches no further than Whole Map.
- `WAYLAND_DISPLAY=wayland-1 cargo test --release -p openmx --test original_mode`: 43 passed.
- `cargo test --release -p game-render --lib gpu::tests`: 24 passed.
- The headless shell on the pause, with settings.txt set to x16, farther, x2 and original in turn,
  logs these fog ranges and far clips (the disc's fog here is 352..768):

  | setting  | fog         | far clip |
  |----------|-------------|----------|
  | x16      | 5632..12288 | 12288    |
  | farther  | 2816..6144  | 6144     |
  | x2       | 704..1536   | 1536     |
  | original | 352..768    | 768      |

- A shot of the Graphics tab at 12x on a private weston:
  [graphics-tab-fog-12x.jpg](graphics-tab-fog-12x.jpg).

## Not checked

- The feet shown over a real pause on screen. Only the menu test checks them.
- Riding at 16x on a combined world of many maps, to watch the frame rate.
