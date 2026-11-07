# 1 — fliers preview (FINAL)

Task P315a. 2026-11-07.

## What I did

On the Garage's Fliers tab, the stage now turns the seat's flier where the Rider, Bike and Gear tabs show the bike and rider.

- **Which flier.** The stage shows the kind whose row is under the cursor, or the first kind (the plane) while the cursor is on the Player row. It shows the seat's pick of that kind (`roster::flier_of`). For a kind none of whose skins can fly (a greyed hovercraft), it shows the first skin. This is `Menu::garage_flier` in menu_pages.rs.
- **Which model.** A new field, `roster::Flier::preview`, holds the pack's folder and the path of the model to show. It is filled in by `flier_offers` in main.rs:
  - the skin's `ui` (`<skin>_ui.psg`, the garage model) where that file is in the pack;
  - else its ride `psg`;
  - `None` for MX Unleashed's, which is drawn in the disc's own `<dir>\anim\<skin>.psg` out of `machine.res`.
- **The stage** (garage_view.rs) is now keyed by what it shows, `Shown::Ride(pick)` or `Shown::Flier(flier)`. The rest works as before:
  - dressing runs on the worker (P258);
  - recent dresses are kept, and the shown flier's neighbours are dressed ahead (`Stage::ahead_fliers`, `Menu::garage_flier_neighbours`);
  - `settle` waits for the dress before a shot.

  `dress_flier` loads the one model as `garage-flier`. There is no rider and there are no parts. The rotor or prop stands still: the flier is parked. The camera stands at 0.8 of the bounding-box radius, because a flat flier's box corners are far outside what is seen (set by eye). The bounds and mips code is now shared by the bike and flier paths.
- **Touch arrows.** On the Fliers tab, two round arrows sit in the stage's bottom corners (`FLIER_ARROWS` in menu_screens.rs). A click or tap on one steps the shown kind's skin left or right, as left/right on its row do (`Menu::step_garage_flier`). Unflyable skins are skipped, and it plays the Move cue, or Refused when there is nothing to step to. `Menu::flier_arrow_point` gives their pixels, for tests.
- **Front-end clicks.** The front end's loop never passed the window's clicks to `Menu::click`; only the ride's pause did. So P277's row taps, and these arrows, did nothing on the main menu. `front_end` now passes each click through `cursor::to_frame` to `Menu::click`, beside `drag_cards`. A click on a non-page screen does nothing, as before (`click_page` returns false, and the player is not shown in the front end).

From opentennis: nothing. Its front end has no turning 3D model of a pick.

## What the original does

Not decompiled: this is our menu. The `_ui` models come from MX vs. ATV's garage sets (`UI/Res/Veh_*.res`, as the pack's manifest records). How MX vs. ATV poses or lights them in its garage was not looked at.

## How it was proved

- **Menu shots**, `--menu --screen fliers --size 1920x1080 --shot … --shot-frame 3`, taken on a private headless weston (llvmpipe, the GPU path). Not committed:
  - `context/shots/p315a/renamed.png`: a scratch `mods/machines/renamed_fliers`, a copy of mxvatv_fliers with its id, name and `source.game` changed and every skin renamed "<Skin> (renamed)". Its garage.txt picks `biplane renamed_fliers biplane3` and `kiowa renamed_fliers kiowa4`. The rows read "Biplane3 (renamed)" and "Kiowa4 (renamed)", and the stage shows biplane3 (the Spitfire livery) off the pack's `biplane3_ui.psg`.
  - `context/shots/p315a/mxu.png`: the default mods folder, with garage.txt `biplane original biplane2`. The stage shows MX Unleashed's second biplane livery, off the disc.
  - `both.jpg` puts the two stages side by side. The arrows are in both.
- **Tests:**
  - `tests/menu.rs::the_fliers_tab_picks_a_renamed_packs_skins_per_seat`, extended:
    - `garage_flier` follows the plane row's pick;
    - a `Menu::click` on the right arrow steps it to the second livery, and the left arrow steps it back;
    - the neighbours dressed ahead are the skins either side;
    - on the Helicopter row the stage shows the kiowa, and the arrows step the kiowa.
  - New `garage_view::tests::fliers_dress_off_the_disc_and_a_packs_garage_model`: MX Unleashed's four liveries, and the pack's biplane3 off `Veh/Anim/biplane3_ui.psg`, each dress with triangles, textures and a radius. Off the disc: biplane 3837 triangles, kiowa 2723; biplane3_ui 4784.
- **Gate:** `./kit check --all` (in `./kit done`).

## Not verified / not 1:1

- **No phone run.** The arrows were tapped through `Menu::click` in the menu test (1280×720) and drawn in the desktop shots. They were not tapped on a phone or in the browser. Phones and the browser reach `Menu::click` through the window's clicks, the same path the new front-end wiring reads, but no device was used.
- **The front-end click wiring is new behaviour for P277 too.** A desktop mouse click on an option page's row in the main menu now presses it, as P277's commit says taps should. That path was not tried by hand with a mouse: there are no keystrokes or clicks on this desktop. It is covered only by the menu tests that call `Menu::click`.
- **No shot of the helicopter, or of a flier row under the cursor.** With no input on this desktop, the shots open on the Player row, so the stage shows the plane. The menu test asserts that the Helicopter row shows the kiowa.
- **Software path not shot.** `draw_soft` draws the flier through the same `actors()` as the bike, but both shots are on the GPU path.
- **The camera framing and the 0.8 radius are by eye**, on the biplane. The kiowa's rotor makes its box larger, so it was not seen framed.
- **The stage shows the last bike for a moment.** On first opening the Fliers tab, the bike shows until the flier's dress is ready, as any not-yet-dressed pick does (P258).
- **MX vs. ATV's own garage presentation** (pose, light, a turning rotor) was not looked at; the flier stands still in the stage's light.

## Next

none: P315b–g carry the rest of the picker.
