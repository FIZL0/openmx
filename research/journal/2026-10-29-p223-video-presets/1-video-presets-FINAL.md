# 1 — video presets (FINAL)

Task P223. 2026-10-29.

## What I did

Ported opentennis's video settings (`crates/hst-base/src/graphics.rs`, its P53 / B343 / B345 / B364 /
B412 / B538 / B547 / B644) onto openmx's own rows, in a new `crates/openmx/src/video.rs` (mounted inside
`menu.rs` as `menu::video`, so the tests that `#[path]`-mount menu.rs get it without a new mount):

- **Rows a preset sets** (`video::Look`): Draw Distance, Plants (the V55 LOD), Texture Filtering,
  Shadows, two new rows (**Shadow Casters** All/Fewer, **Model Detail** Full/Reduced), Render Scale
  and Frame Limit.
- **Presets** (`video::Preset`), each setting every one of those rows:
  | | draw | plants | filter | shadows | casters | detail | scale | limit |
  |---|---|---|---|---|---|---|---|---|
  | High | Whole Map | smooth | 16x | Better | all | full | Auto | off |
  | Medium | Farther | smooth | 16x | Better | fewer | full | Auto | off |
  | Low | Original fog | original | 2x | Original | fewer | reduced | 50% | off |
  | Steam Deck | Far | smooth | 8x | Better | fewer | full | 100% | off |
  | Phone | Far | original | 4x | Original | fewer | reduced | 75% | 60 |

  High is today's desktop defaults exactly, so a desktop's settings.txt is unchanged by Auto. Phone is
  P215's phone preset folded in (the original's per-racer shadow as P205 already gave the phone page).
- **Auto** (`video::pick(&Device::here())`): Steam Deck (`SteamDeck=1`) → Deck; anything without a touch
  screen → High; a touch device (iOS build, or the page's phone flag) → Phone, then
  `Device::steps()` rungs cheaper by its iPhone model tier (hw.machine) / RAM / cores / screen, as
  opentennis's `pick` does. `Device::here()` reads hw.machine and hw.memsize on iOS (new
  `openmx_ios::model()` / `memory_mb()`), and on the web a `device` message the page posts (narrow and
  wide CSS screen side, devicePixelRatio, hardwareConcurrency, deviceMemory).
- **Auto in a ride** (`video::Tune`, opentennis's `auto_tune`): after a ride's first 3 s it times 2 s
  windows (slowest 5% dropped) against 93% of min(60, display refresh) — the page measures its rAF rate
  and posts `hz` — and on a miss steps one rung down `video::cheaper` (casters → draw distance → scale
  100→75 → shadows → filtering → plants → detail → scale 50 → original fog → 2x → shadows off →
  filtering off), settles 1 s, and stops after two windows in a row hold, the bottom, or 16 s timed.
  Never steps up; each ride checks again. A step goes into the front end's settings and through the
  ride's existing "settings changed" block (new `Menu::mark_changed`), so it is applied and saved like a
  pause-menu change. Only in a windowed ride under the front end with no graphics flag given.
- **Graphics tab**: a Preset row at its head (Auto, High, Medium, Low, Steam Deck, Phone; Custom shown
  when the rows match none — a press there goes back to Auto). Any other row changed turns Auto off.
  Shadow Casters (dimmed unless Shadows is Better) and Model Detail rows under Picture.
- **settings.txt**: `video = auto|custom` only where the rows don't imply it (no line = Auto when the
  graphics rows are the defaults — so every file from before P223 is written back as it was, which
  `a_settings_file_from_before_the_redo_reads_as_the_same_settings` checks); `shadow_casters = fewer`,
  `model_detail = reduced` only off default. A file with no `video` line and default rows reads as Auto
  and takes this device's pick.
- **Speed-ups in game-render/gpu.rs**:
  - Actors (riders, bikes, the plane) culled against the view frustum (radius × 1.5 for limbs); they
    still cast sun shadows when out of view. Always on.
  - `few_casters` (Shadow Casters: Fewer): plants and props under 12 ft radius cast no Better shadow.
  - `lod_bias` (Model Detail: Reduced = 2): props' and actors' LOD distance ×2, the ground quadtree's
    pixel error ×2, and the tiny-prop cut-off ×2.
  - `TEXTURE_CAP`: model textures uploaded with no level wider than 512 on a touch device (top mips
    dropped), set at start-up before any loads.
- **Web page**: render target capped at 2 device px per CSS px on a phone (`ratio()` in index.html, used
  for the canvas size and the click mapping); the page posts `device` and the measured `hz`.
- **CLI**: `--video-preset high|medium|low|deck|phone` (`--preset` was taken: the controller preset),
  `--casters all|fewer`, `--detail full|reduced`; any graphics flag keeps Auto from stepping.
- 30 Hz low-power cap: the ladder aims at the display's rate when under 60, so a phone in Low Power
  Mode (30 Hz rAF) is not stepped down for it — that is what opentennis's B644 cap amounts to here.

## What the original does

Nothing: these are openmx's own settings, not the PS2 game's.

## How it was proved

- `crates/openmx/src/video.rs` unit tests (`./kit check -- -p openmx --bin openmx video`, 3 pass):
  `auto_picks_per_device` pins the Auto pick for a desktop, a Steam Deck and a phone, the steps below
  Phone by iPhone/iPad model, by Safari's screen size, by Chromium's memory and cores and for a tablet,
  and the 512 texture cap; `the_preset_row` (row label/index round trip, Custom);
  `auto_timing` (a slow device steps down each window, a fast one holds after two, the High ladder
  passes Medium, the Phone ladder passes Low, refresh 30 aims at 30).
- `crates/openmx/tests/menu.rs` (207 pass): new `the_preset_row_sets_the_graphics_rows` (presses on
  Preset set every row, saved and read back, a row by hand → Custom, a press on Custom → Auto + pick);
  the settings-file round trip with the new fields; the Graphics tab tests updated for the Preset row
  at its head and the two new rows under Picture.
- wasm build: `tools/web.sh --check` builds clean.
- `crates/openmx/tests/web.rs` (7 pass): the headless-chromium phone page (844×390 at dpr 3) now draws
  1688×780, 2 px per CSS px, where it drew the whole 2532×1170 before — the test's expectation moved
  with it; the host-call list has `device` and `refresh`.
- A ride timed under `--timings` on each preset, release build, `--headless --track free06 --frames 900`
  (offscreen GPU on this desktop; load average 40–49 from the other slots, so the ms are noisy and the
  triangle counts are the firmer number). Same ride hash 0x57a12d1ddb921e12 on all five (the sim is
  untouched):
  | preset | triangles / frame | present ms | all ms | frames over 16.7 |
  |---|---|---|---|---|
  | High | 206,899 | 3.01 | 19.63 | 531 |
  | Medium | 206,866 | 2.57 | 18.20 | 360 |
  | Low | 62,510 | 1.92 | 17.92 | 272 |
  | Steam Deck | 179,731 | 2.30 | 16.98 | 157 |
  | Phone | 122,565 | 2.01 | 17.08 | 172 |

  Earlier at load ~15–25 (before Model Detail took the ground): High present 1.79 ms, Medium 1.69, Low
  1.26, Deck 1.92, Phone 1.65. High with `--detail reduced` against full, 300 frames: 141,935 against
  210,006 triangles a frame (−32%, almost all the ground's levels).

## Not verified / not 1:1

- **No real phone, Steam Deck or browser-phone measured.** The tier tables (`ios_tier`, RAM/core steps)
  are opentennis's, not tuned for openmx's load; the timings above are a desktop GPU's.
- **The Auto tuner in a live windowed ride was not run.** Its logic is unit-tested and the hook only
  feeds it frame times and marks the menu changed; no windowed front-end ride was driven here (no pad
  or keys reach the desktop unattended).
- **Medium draws as many triangles as High** on free06: Farther's far clip (6144) is past the track, and
  casters only cut shadow-pass work, which the triangle count doesn't include. Its gain is in the
  shadow pass (present 2.57 vs 3.01 ms under load), not proven on a quiet machine.
- **iOS render target is not capped at 2 px per point.** ios_gpu.rs sizes the surface from winit's
  physical size (3x on most iPhones); a cap needs the surface and the safe-area/touch maths in that
  window scaled together, which can't be tried here (no iOS device run). The Phone preset's 75% render
  scale puts the 3D at 2.25 px/pt meanwhile.
- **Menu glyph sheet not halved** (opentennis B644). menu_art.rs's sheet is a 1024-wide SDF at
  `ATLAS_EM` 64 texels/em, a `const` all its metrics read; halving it on phones touches every metric
  and its sharpness tests. Not done; its size was not measured.
- **Texture cap covers model textures only** (`Mips::of`): ground paint arrays and texture-pack
  replacements uploaded by other paths are not capped.
- **Web device info** (`device`, `hz` messages): the page ran in headless chromium (web.rs), but
  nothing reads back what Device::here() made of them there; deviceMemory is Chromium-only, so Safari pages lean on screen size and cores.
- `Device::steps` for Android pages is a guess from opentennis's B412; no Android device here.

## Next

- Follow-ups, if wanted: the iOS 2 px/pt surface cap and the half-size glyph sheet (above), and a
  windowed front-end ride on a slow device to watch the Auto tuner's "auto graphics:" lines.
