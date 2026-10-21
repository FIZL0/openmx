# 1 — render scale setting (FINAL)

Task V34. 2026-10-21.

## What I did

- `menu.rs`: a `RenderScale` (Auto, 100%, 75%, 50%) in `Settings`, as a ninth Settings row,
  "Render scale", right under V32's "Graphics". Left/Right/Cross step through it the same way as the
  graphics row. It is saved as `render_scale = auto|100|75|50` in settings.txt. A missing key, or a value
  it can't read, gives Auto; a hand-written `50%` also reads. `RenderScale::fraction` gives what
  `--render-scale` would: `None` for auto, else 1.0/0.75/0.5.
- `main.rs`: the new `Args::render_scale_asked` records that `--render-scale` was given, including
  `--render-scale auto`, so the command line still wins over a saved 75%. When the front end starts a
  ride (local or online), it passes the setting's fraction unless the flag was given. Inside the ride a
  local `render_scale` replaces the uses of `args.render_scale`. A change made in the pause's Settings
  takes effect straight away: a fixed scale is set on the renderer, and with Auto the existing
  `AutoScale` takes over again on the next window frame. It prints `render scale: … (settings)`. The
  `--help` text says the row decides when the flag is absent.
- A ride started without the front end (`--ride`, `--headless` …) still uses `--render-scale` or auto,
  not settings.txt. That is how V32's `--graphics` works there too.
- From opentennis: its `hst-base/src/graphics.rs` stores `render_scale` as a percentage. I took the key
  name and the percent words from it, plus the trailing `%` it tolerates. Its 25–200 free range is not
  needed here: the row is four choices, and anything else is `--render-scale`'s job.

## What the original does

Nothing. The original has no render scale; this is a front-end and rendering setting, ours to design.

## How it was proved

- `crates/openmx/tests/menu.rs::the_render_scale_row_cycles_and_is_saved`: it navigates to the row with
  the pad. Right steps Auto → 100% → 75% → 50% → Auto, Left goes back, and Cross steps forward. Each change
  is reported as `changed`, which is what makes the shell save the file, and the graphics row is not
  touched. Every value is written to and read back from settings.txt. A file with no key, an unknown
  value (`33`) and a `50%` are also tested, as is a real `menu::load` of a directory whose settings.txt
  has no key: all of these give Auto except `50%`, which gives 50%.
- `the_settings_file_reads_back_as_it_was_written` now also round-trips a non-default render scale.
- Looked at the Settings screen headless (`openmx --screen settings --headless --shot`). The ninth row
  fits above the "Settings" title without overlapping it.

## Not verified / not 1:1

- I did not watch the setting reach a live ride. Nothing here drives the front end's menus into a ride
  unattended, and settings.txt lives beside the user's disc. Writing a test value there would change the
  user's own settings. The wiring (front end → `a.render_scale`, the pause's change → renderer) was
  checked by reading it and by the build, not by running it.
- Switching from a fixed scale back to Auto mid-ride restarts `AutoScale` from wherever it last was, not
  from a fresh start. It settles again within its own window. I accepted this and did not measure it.
- Offscreen (headless) rides draw at 1 under Auto, as `--render-scale auto` already did (V30).

## Next

Nothing.
