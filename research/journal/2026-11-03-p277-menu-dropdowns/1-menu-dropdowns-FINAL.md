# 1 — menu dropdowns (FINAL)

Task P277. 2026-11-03.

## What I did

Ported opentennis's B498 options dropdown to openmx's option pages (Settings, Garage, Modifiers,
Mods, Original; the pause opens the same `Screen::Settings`, so it gets it too).

- `menu_pages.rs`: `Row::many()` is a Choice row with `LIST` (3) or more choices, other than Load
  Preset, the camera's Save Over and the Garage's rows (rows whose choice leads to another act or
  page, which keep their ‹ ›). `Row::listed()` is `many()` and not dimmed. `page_of` gives every
  many-choice row with no hints its short hint per choice (`hint`: Updates, Aspect Ratio, Preset,
  Draw Distance, Texture Filtering, Shadows, Shadow Quality, Render Scale, Frame Limit, Mix, Music
  Plays, Camera Preset, Handling Pack), so the list and the side panel show them.
- `menu.rs`: `Menu::listing: Option<(Screen, row, choice)>`, cleared by push, pop and a screen
  change. `Menu::listing()` is the open list while its page and row are shown.
- `walk_page`: Cross on a listed row opens its list on its current choice. While open,
  `walk_list` takes every press: Up/Down move round the choices (wrapping), Cross picks and
  closes, Circle closes as it was, anything else does nothing. Left/Right on a closed row still
  step it as before. The pick goes through `change_to(row, press, Some(j))`, which is `change`
  with the choice given rather than cycled. Rows whose change is not a plain index (Frame Limit,
  Preset, Music Plays, Song, Camera Preset) have their own pick arms. `play_choice`/`pick_song`
  came out of `music_plays` for that.
- `menu_screens.rs`: a many-choice row's value is a glass pill showing the value and ▼ (▲ and a chalk
  ring when open). The open list is a panel right-aligned under the row, or above it if there is no
  room below, scrolled around the current choice. Each item shows the choice and its hint, the lit
  one in the row pill's red, the set one with a dot. The hints read Open on a listed row, and
  Close/Pick while the list is open. `page_column`, `row_spots` and `drop_spots` are the layout the
  drawing and the clicks share.
- Mouse and touch: `Menu::click` asks `click_page` first. On an option page a click on a row
  moves the cursor there and presses it (a many-choice row opens, an On/Off row toggles). With a
  list open, a click on a choice picks it and a click anywhere else closes the list. A touch-screen
  tap off the touch pad's buttons already reaches the menu as that click (`touch_pad.rs`), and the
  pad's buttons are a pad device.
- `main.rs`: `--open-list` opens the dropdown of the row `--screen` lands on, for a shot.

## How it was proved

- `tests/menu.rs`:
  - `a_many_choice_row_opens_moves_picks_and_closes_its_list_by_pad_keyboard_and_touch_pad`: each
    device in turn (a pad, `Dev::Keys`, the `Touch controls` pad) claims 1P on the splash and goes
    to Graphics › Render Scale. Its Cross opens the list on Auto, Up wraps to the last choice,
    Down wraps back, Left and Right do nothing while it is open, and Cross picks the choice and
    closes the list. Circle closes it without a change, and Right on the closed row still steps.
    Every choice has a hint.
  - `a_click_opens_a_list_picks_a_choice_and_off_it_closes_it`: at 1920x1080, a click on the row
    opens its list, a click on a choice picks it, a click off the list closes it without a change,
    and a click on VSync toggles it.
  - `two_choice_rows_toggle_and_only_many_choice_rows_drop`: on every Settings tab no two-choice,
    slider or dimmed row drops, and every listed row has a hint for each choice. VSync still
    toggles on Cross.
  - `press_every_row` closes any list its Cross opens with Circle. The Graphics and Render Scale
    tests now go through the list (`list_next`).
  - `cargo test -p openmx --test menu`: 245 pass and 1 is ignored, in s1 with the disc.
    `original_on_sets_every_sub_switch…` was already red before this task, since P268: Right from
    Original Fog wraps to Whole Map. Its expected value is fixed.
- Shot: `--screen graphics --open-list --shot` on a private headless weston, with the Preset list
  open. The pill, the panel, the hints, the dot on Auto and Close/Pick all drew right; viewed by eye.

## Not verified / not 1:1

- No real touch screen or mouse was used. The click test calls `Menu::click` with the points the
  drawing uses (`row_point`/`list_point`), and the touch pad's tap-to-click path was not run.
- The GPU-vs-CPU screen comparison has no open-list screen in it. Only the CPU shot was looked at.
- The shot was not taken from the pause. It is the same page code.
- The mouse wheel does not scroll a long list. Keys and clicks on the shown choices do; no list
  here has more than 7 choices.

## Next

none
