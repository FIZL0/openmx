# 1 — drop old huds (FINAL)

Task P170. 2026-10-26.

## What I did

The user asked for the debug HUD and the original's HUD to come out of the menu ("we likely wont use
them"). The brief: delete the debug HUD from the code, and take the original's out of the menu,
settings.txt and `--overlay`.

What still leaned on them, and where it went:

- **`menu::Overlay`** is `Hud | None` now (`ALL == CHOICES`). `Overlay::GONE` lists the old words
  (`original`, `debug`, and P36's `on`/`off`). `from_word` reads each of them as `Hud`, so an old
  settings.txt loads on the ride HUD. P121's `hud = 1` marker line is no longer written, and when an
  old file still has it, it is ignored (`HUD_KEY` and the P121 migration are gone).
- **`--hud`, `--overlay debug` and `--overlay original`** are refused at argument parsing with
  "… is gone (P170)". `--overlay` takes `hud` or `none`. The usage text and main.rs's module doc
  were updated to match.
- **The debug HUD** is gone. That covers `hud::Hud` and its panel, font and blit, and the tests in
  `hud.rs` that tested them. In main.rs it covers `Rig::hud`, the `note_*` calls, F3/H's toggle,
  `Args::hud`/`debug_hud`, `Dress::hud`, `marker_bar_shown`, and the software slot bar
  (`hud::draw_markers*`), which only the debug HUD and the original's HUD drew. The ride HUD has its
  own marker menu (P121). `layer.rs` (P44's CPU patches) only ever carried the debug HUD, so it went
  too, and `gpu_patches` is now always empty. hud.rs keeps the markers' bar state (`MarkerBar`,
  `MarkerBanner`, `marker_label`, `markers_shown_in`), which the ride HUD's marker menu reads.
- **Original Game (P126)** no longer touches the HUD. On sets every departure and Air Wrecks.
  `original()` no longer reads the HUD (`hud_is_original` is gone). The row's text now says "the HUD
  stays openmx's".
- **Speedometer and Minimap rows** stay. The ride HUD has both: its digital/analog speedometer
  (`ride_hud::speedometer`) and its radar (`Shown::minimap`). Their "off" note is now just "The HUD
  is off" under None. The Debug case is gone.
- **overlay.rs's pop-ups.** The ride HUD draws its own trick pop-up and info toast
  (`ride_hud::Tracker::popup`/`toast`), not through overlay.rs. Only the Original branch called
  `Art::draw_pops`/`sprites_pops` (via `Tracker::pops`). The S85 wreck fade (`shade_view` / the
  GPU's black sprite) is drawn before the HUD and apart from any overlay mode, so it was left
  untouched.
- **overlay.rs itself stays.** The ride HUD takes its layouts, constants, `Art` (tags, radar blips,
  offers, scale) and gauges from it. The original HUD's own drawing (`Art::draw`/`sprites`/`*_pops`,
  `Pen`, …) has no caller in the shell now. Per the brief, which only takes Original out of the
  menu, settings and flag, I kept it: `#[allow(dead_code)]` on `mod overlay` in main.rs, plus one on
  `Tracker::pops`. tests/overlay.rs still covers that drawing.
- **Tests and tools that rode `--hud`.** tests/hud.rs's P83 test is now "a plain launch is the ride
  HUD", plus a new `a_launch_asking_for_a_gone_hud_is_refused`. Its pause test rides `--overlay hud`
  instead of `original`. tests/ride_hud.rs's `each_overlay_draws_its_own` is down to hud vs none.
  tests/menu.rs has `the_settings_offer_the_hud_and_none_alone` (in place of P83's) and the Original
  switch tests (the HUD is left alone), plus a new `original_game_on_still_rides_openmxs_hud`. In
  tests/markers.rs, the three software-bar drawing checks went with the bar. P117's
  `markers_hud_off.rs` and P124's `markers_plane_bar.rs` shot `--overlay original` to see the bar
  drawn. They now shoot `--overlay hud` and look for the ride HUD's marker menu in a top-right box
  (320..640 × 0..160 of 640x448, where `ride_hud::top_right` puts it) instead of the old top-centre
  one. The first full gate caught these two. No `./kit`, tools/ or
  research script passes `--hud` or `--overlay original|debug`; only old journal entries do, and those
  stay as written. PLAN.md's code map line for hud.rs and pad.rs's `--pad-pressure` note were updated.

Nothing taken from opentennis: this is removal only.

## What the original does

Nothing here is the original's behaviour. This is front-end furniture (AGENT.md: menus are ours).

## How it was proved

`./kit check -- -p openmx --test menu --test hud --test ride_hud --test markers --test overlay
--test trick_hud`: 393 passed. The shell-launching tests in hud.rs and ride_hud.rs were run again
with `--nocapture` to make sure they did not skip: no "skipped" line, so the disc was there.
- The HUD row offers `["HUD", "None"]`, cycling round both ways (menu.rs).
- `overlay = original|debug|on|off`, with or without `hud = 1`, loads as Hud (menu.rs, ride_hud.rs).
- `--hud`, `--overlay debug` and `--overlay original` exit non-zero with "gone (P170)" (hud.rs).
- Original Game on reads On with overlay Hud, minimap on and scale 100, and so does its saved file
  read back (menu.rs).
- The trick pop-up's land/bail colours and play-out are drawn by the ride HUD (ride_hud.rs's
  existing pop-up tests, unchanged).
- A default launch draws the HUD at the speedometer's and radar's boxes on both renderers, and still
  does under the pause (hud.rs).

## Not verified / not 1:1

- **The wreck fade (S85) has no new test of its own here.** Its drawing code is untouched and runs
  before, and apart from, every HUD branch. Its existing `page_shade` unit tests in main.rs and
  wreck_page.rs cover it. No ride was filmed through a wreck to look at it after this change.
- **No by-eye ride.** No window was opened to ride and look at the HUD after the change. The
  shell-launching tests check headless first frames only.
- **The marker tests' top-right box is a pixel count, not a drawn-rect check.** It proves something
  changed there with markers up (>500 pixels), not that it is the menu itself. The menu's layout is
  checked by ride_hud.rs's own `PanelDrawn` tests.
- **one_fps reds under a busy machine.** It failed the first full gate at 81 ms a frame (load 37 on
  32 cores; sim 33 ms, the HUD 1.4 ms) and passed alone. It is not in gate.toml's lone-rerun list,
  so I reran it by hand.
- **The original's HUD drawing is kept but unreachable.** Nothing in the shell can show it any more,
  so only tests/overlay.rs exercises it. If it is never wanted back, deleting it (and
  `Tracker::pops`, `overlay::Pops`, …) is a follow-up. I did not do it here because the brief
  deletes only the debug HUD from the code.

## Next

Nothing.
