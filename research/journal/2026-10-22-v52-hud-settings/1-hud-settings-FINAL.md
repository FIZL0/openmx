# 1 — hud settings (FINAL)

Task V52. 2026-10-22.

## What I did

- **Settings** (`menu.rs`, rows in `menu_pages.rs`): the first attempt (wip/V52) was written against
  the old numbered Settings rows and its merge failed to build after P118 redid the front end, so on
  2026-10-23 it was carried onto main and the rows redone as P118 rows on the Display tab, under
  Speedometer: `Minimap` (an On/Off choice, dimmed like Speedometer unless the HUD is Original) and
  `HUD Scale` (a slider row, `N%`, 50 to 150 by 10: the arrows stop at the ends, Cross steps up and
  wraps 150 → 50, as the volume sliders do; dimmed only when the HUD is None, since it sizes the
  debug HUD too). `settings.txt` keys
  `minimap = off` and `hud_scale = N`, each written only when not the default (on, 100), so a file
  without them reads and writes as today. `hud_scale_word` takes `130` or `130%` and refuses steps
  the row does not offer (a refused value is 100).
- **Flags** (`main.rs`): `--minimap on|off` and `--hud-scale 50..150` beside `--overlay`; unset, the
  front end's settings decide, like `--speedometer`. The ride reads both from the settings when the
  menu changes them, and puts them on the original's HUD art and every seat's debug HUD each frame.
- **The original's HUD** (`overlay.rs`): `Art::minimap` and `Art::scale`. Minimap off skips the whole
  radar (rose, arrow, other riders' marks and tags, P92/P94 icons and markers) — the radar code moved
  into `Art::radar` unchanged so `paint` can skip it. The scale multiplies the `Pen`'s view scale, and
  every part is already counted from its own corner, so each grows about that corner: the radar
  bottom-left, the speedometer (readout or dial, odometer, preload meter) bottom-right, the score bar
  and tags top-left. It is capped at the size where the radar and speedometer would meet or leave
  the view (`NEEDS`, as the 100 % layout already was). The switch pop-up (P103, bottom centre) stays at
  the original's size: grown, it sits over the radar and the speedometer. `NEEDS`' height went
  150 → 149: the radar's top (layout y 299) is the tallest part, not the dial's (301); without that a
  two-row split could never reach 1.5× (149 × 1.5 / 448 just fits half a screen, 150 × 1.5 does not).
  At 100 % this changes nothing in any layout the shell makes.
- **The shell's HUD** (`hud.rs`): `Hud::scale`. At 1.0 the debug panel draws exactly as before; at
  another scale it draws into a black panel-sized frame and is copied nearest-pixel over the view's
  bottom-left corner at that size, the scene darkened under it (the panel's ink is never black).
  Grows no bigger than the view holds. The markers' slot display gets `draw_markers_scaled`: its
  whole-pixel font scale is the old `h/224` times the HUD scale, floored, at least 1; `draw_markers`
  is kept as the 100 % form (tests/markers.rs calls it).
- opentennis has no HUD scale or minimap setting; nothing taken.

## What the original does

Nothing to port: the original's HUD has no scale and no radar switch. The layout numbers at 100 % are
V26/V42's, unchanged.

## How it was proved

- `tests/menu.rs` `the_minimap_and_hud_scale_rows_change_and_are_saved`: the rows by pad taps
  (Display tab, down to Minimap, then HUD Scale), the ends, the wrap, the neighbouring rows
  untouched, saved with `menu::save` and read back with `menu::load`, refused values, and no keys in a
  default file. `the_settings_file_reads_back_as_it_was_written` carries both fields.
- `tests/overlay.rs` (off the disc's `fstyleui.res`; checked they run, not skip, by a temporary
  `panic!` at each test's end — again in slot s1 after the carry-over, 2026-10-23): `minimap_off_draws_the_speedometer_and_no_radar` — one 1280x720 view
  and each of a four-way split, every P92 icon kind on the radar: off, the speedometer is drawn and the
  radar's corner has zero lit pixels, no icons, no marks; on, all there.
  `hud_scale_grows_the_radar_and_speedometer_about_their_corners` — one 1280x720 view (dial and
  readout) and each view of a four-way 2400x900 split: at 150 % the pen scale is exactly 1.5×; the
  radar's bounds (the pixels the minimap switch takes away) and the speedometer's (the lit right half)
  are 1.5× as wide and tall within 3 px, their gaps to their corner edges 1.5× too, inside the view and
  apart. A four-way split of 1280x720 is held to the most the view holds (640/311 view px per layout
  px) with everything inside and apart. 50 % halves the scale.
- `hud.rs` unit test `the_hud_scale_sizes_the_panel_and_the_markers_display`: the panel's changed
  rectangle is exactly `PANEL × scale` at 150 % and 50 %, pinned at the same margin, with ink; capped in
  a 640x360 view; the markers' display 3/2 the size at 150 % of a 448-line frame, still centred, and
  100 % pixel-identical to `draw_markers`.
- Looked at it: `openmx --headless --size 1280x720 --players 4 --hud-scale 150 --minimap on|off` and
  `--hud --hud-scale 150` shots.

## Not verified / not 1:1

- **A four-way split of a window narrower than about 2.1:1 does not reach 150 %.** The radar (151
  layout px from the left) and the speedometer (160 from the right) meet first: a 16:9 four-way view
  holds 1.28× at most, which is what it draws. The test proves the full 1.5× on a 2400x900 window and
  the cap on 1280x720. The brief asks for 150 % in a four-way split; the alternative was letting them
  overlap, which I judged worse.
- **In a two-row split at large scales the radar's top runs under the score bar** (seen in the
  1280x720 four-way shot at 150 %: the rose's N sits under SCORE). The view is only 224 layout px tall
  and the radar alone takes 149 × scale of it; the score is not moved or shrunk to make room. The tag
  and MODIFIED lines under the score already overlapped the radar's top at 100 % in two-row splits.
- **The switch pop-up (P103) does not scale** — a choice, not a gap: grown about the bottom middle it
  covers the radar and speedometer.
- **The markers' slot display scales in whole pixels**: in a view under 448 lines (h/224 = 1) 150 %
  floors back to 1×, and 50 % never goes below 1 px a font pixel. Its pixel font cannot draw between.
- **The debug panel at a scale other than 100 % is a nearest-pixel resample** of the panel drawn at its
  own size, so its 1 px lines come out 1 or 2 px at 150 % and some vanish at 50 %. Debug furniture only.
- **The GPU path's scaled sprites are not compared against the CPU's at 150 %**: they go through the
  same `Pen` (`the_gpu_draws_the_hud_as_the_cpu_does` covers 100 %), so the same scale reaches both, but
  no test renders the GPU at another scale.
- **The Settings page's new rows were not looked at on screen**; they are the same P118 choice and
  slider rows as their neighbours, checked through the menu's state only.
- **The ride-side code was carried over by git's recorded merge resolution, not rewritten.** The gate
  and the run probe show it builds and its tests run on today's main, but no new shots were taken
  after the carry-over.

## Next

Nothing for this task.
