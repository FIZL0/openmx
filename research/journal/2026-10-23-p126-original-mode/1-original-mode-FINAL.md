# 1 — original mode (FINAL)

Task P126. 2026-10-23.

## What I did

- **The departures, gathered first.** `grep` of plan/TODO.md and plan/DONE.md for "need not be 1:1" /
  "one setting away" finds V55 (bushes, done) and V56 (foliage, open) only; the settings, the flags and
  the V/P tasks behind them give the rest. Every place the port departs from the original's look or
  feel on purpose, and what P126 does with it:

  | departure | setting today | ours (default) | original | under the switch? |
  |---|---|---|---|---|
  | bike button layout, every seat (P40, P81; P73 the disc's table) | controls.txt `layouts` | New (R2/L2) | Original | yes — **Controls** |
  | biplane stick, every seat (P50, P69) | controls.txt `planes` | New (plane-relative) | Original | yes — **Controls** (same switch) |
  | follow cameras, every seat (P82) | settings `camera` | `CameraView::DEFAULT` (further out) | `CameraView::ORIGINAL` | yes — **Cameras** |
  | fog and far clip (V32) | settings `graphics` | Full (whole map) | Original | yes — **Fog and Draw Distance** |
  | plants' model-to-picture change (V55) | none before; `Grove::lod` was always Smooth | Smooth | `Lod::Original` | yes — **Plant Detail**, new `plants` key |
  | picture shape (V24, `--aspect`) | `--aspect` flag only | Fill (window's shape) | 4:3 with bars | yes — **Picture Shape**, new `picture` key |
  | HUD (V26, V42, V52) | `overlay`, `minimap`, `hud_scale` | already the original's | the original's | master On resets it, and On reads it; not a sub-switch (see below) |
  | foliage see-through (V56) | not done yet | — | — | no: V56 has not landed; it adds its switch when it does |
  | drawing above 60 fps, interpolated between ticks (P52, P90) | Frame Limit / VSync | refresh rate | the PS2's rate | no: the user asked for it (AGENT.md, 2026-10-20), it is the Frame Limit row's to change |
  | render resolution, Render Scale (V30, V34) | `render_scale` | Auto | 640x448 | no: performance, not look; opentennis's Original Mode is exactly this and nothing else |
  | game modifiers (P55) | settings `modifiers` | the original's | the original's | no: the brief excludes the sim |
  | speedometer dial or readout (V42) | `speedometer` | Digital | the original offers both | no: both are the original's |
  | machines at bays (P103), wrecks, split | `machine_bays`, `wrecks`, `split` | the original's | the original's | no: on by default *are* the original |

- **Master and sub-switches** (`menu.rs`: `Departure`, `Original`, `Menu::{is_original, some_original,
  set_departure, hud_is_original, original, set_original}`). The master is not stored: it is read off
  the sub-switches, so it can never disagree with them. **On** = every departure the original's for
  every seat *and* the HUD the original's; **Off** = no departure the original's for any seat (whatever
  the HUD); **Mixed** otherwise — Mixed, not Off, because Off means "every enhancement on". Turning the
  master on sets each departure to the original's and resets the HUD (overlay Original, minimap on,
  scale 100); turning it off sets each departure back to ours and leaves the HUD alone. From Mixed the
  first press is On. A sub-switch turned back goes to the port's default (New / DEFAULT camera), not to
  what it was before the master: a seat's controller preset goes to 0 and a camera's own slider moves
  are not restored. A per-seat sub-switch with some seats on the original's reads Mixed on its row.
- **The HUD is not a sub-switch** because it already starts as the original's: a sub-switch defaulting
  to the original would make a fresh file read Mixed, against "every enhancement on, Original off".
- **Where the rows are:** a sixth Settings tab, **Original** (`menu_pages.rs` `original_rows`), the
  master on top, the five under an "Enhancements" heading. P128 (the Graphics tab) is not done and in
  no slot, so the graphics sub-switches (Fog, Plants, Picture) stay here until P128 moves them; Fog and
  Draw Distance doubles the Display tab's Draw Distance row on purpose until then (one setting, two rows).
  No `--screen` word for the tab (P128 adds its own; a Screen variant for a tab touches every screen match).
- **The shell** (`main.rs`): `--plants smooth|original` and the settings' `plants` set `Grove::lod` at
  load and live from the pause (`gpu::Renderer::set_plant_lod`, new); the settings' `picture` sets the
  ride's `aspect` unless `--aspect` was given (`aspect_asked`), and live from the pause by re-making the
  frame as a resize does. Two log lines, `plants: …` and `picture shape: …`. `eco.rs`: `Lod::ALL/word/from_word`.
- Files written only off ours (`plants = original`, `picture = 4:3`), so an older settings.txt is
  written as it was.
- opentennis: its options.rs "Original Mode" is one graphics toggle (640x448, 4:3, unfiltered), not a
  master over other switches; nothing taken.

## What the original does

Nothing new read: each sub-switch's "original" side is a port already proved or matched by its own task
(P73's binding table, P69's plane stick, P82's camera numbers, V32/V33's fog, V48's LOD rule).

## How it was proved

- `crates/openmx/tests/menu.rs`: `a_fresh_settings_file_has_every_enhancement_on_and_original_off`,
  `original_on_sets_every_sub_switch_and_one_turned_back_leaves_the_rest` (by pad on the Original tab,
  each of the five turned back on its own), `the_original_switches_round_trip_through_the_files`; the
  P118 tab walk updated for six tabs, and `every_row_of_every_settings_tab_and_page_changes_by_pad`
  covers the new rows.
- `crates/openmx/tests/original_mode.rs`: settings.txt + controls.txt saved by `set_original(true)` in a
  store of its own (own kit.toml, a link to the disc) start a ride from the front end that says
  `graphics: original (fog …)`, `plants: original`, `picture shape: 4:3`, `controls: original`,
  `biplane controls: original`; with Off, full / smooth / fill / new. Checked by hand that it does not
  skip: the same run in a scratch store printed `graphics: original (fog 352..768 …, far clip 768)`,
  `plants: original`, `picture shape: 4:3`.
- `./kit check` passes.

## Not verified / not 1:1

- The live switch from the pause (plants and picture shape changed mid-ride) is not tested: the tests
  start a ride with the files; the pause's path is the same `menu.changed()` block that already carries
  graphics, but no test throws it while riding and no window was looked at.
- Picture Shape 4:3 in a window was not looked at: headless rides draw the PS2 frame whatever the
  shape, so the test proves only that the setting reached the ride's `aspect`.
- The original's HUD reset covers overlay, minimap and scale; whether the original's own default
  speedometer is the readout or the dial is not known (V42 says the original offers both), so the
  switch leaves it.
- V56's foliage fix has not landed, so it has no switch; filed V57 for it. Every later enhancement must
  add a `Departure` variant.
- Frame rate/interpolation and render resolution are departures left outside the switch on purpose
  (table above).
- No `--screen` word for the Original tab.

## Next

Nothing for this task. P128 moves Fog, Plants and Picture to its Graphics tab; V57 adds V56's switch.
