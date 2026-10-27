# 1 — camera presets saved (FINAL)

Task P163. 2026-10-26.

## What I did

- **Start preset per seat.** When the camera button steps a seat's follow camera, the shell calls
  `Menu::camera_left_on(seat, machine, name)` and saves settings.txt. It saves the preset's name, and the
  bike and the biplane each have their own. The line is `camera_start = N | bike | plane`. On a seat's
  first frame the shell starts each camera on its saved name (`chase::start_on`). An explicit `--camera`
  or `--camera-view` beats the saved settings. A name the track's file lacks leaves the shell's start.
- **Per-preset numbers.** P82's Camera page now has a **Preset** row: "All Presets", the disc's cycling
  presets (bike, then "Biplane: …"), then the seat's own. On All Presets the page shows P82's sliders
  unchanged. On one preset it shows three sliders (Field of View, Distance, Height) that belong to that
  preset only. These are saved as `camera_preset = N | bike|biplane | base | name | fov dist height`.
  A disc preset with no numbers of its own follows All Presets ("Same as All Presets" deletes them).
- **Custom presets.** "Save as New Preset" names a copy on the keyboard (`Typing::Camera`, NAME_MAX). The copy
  takes the picked preset's lags and other fields under the new name, with its own framing. Customs come
  after the disc's in the presets list, so every disc preset keeps its index and the look-back preset is
  still the disc's. A name that clashes with a disc preset gets " Mine" after it. The "Camera Button" row
  (`camera_cycle = N | disc|both|mine`) chooses Disc's, Disc's and Mine (default), or Mine Only. Mine Only
  clears the disc presets' cycle flags only when the seat has customs for that machine, so a biplane with
  none still cycles normally.
- **Original.** Reset to Original, and the Original switch's Cameras row, clear every disc preset's own
  numbers. That makes the disc's presets the file's bit for bit (tested). The switch also sets the cycle
  to Disc's so customs leave the cycle (they are kept, not deleted). `is_original(Cameras)` requires that.
- **Split / online.** Everything is per seat in `Settings::camera_sets[SEATS_MAX]`. It is indexed the way
  P82's `cameras` is (online seat i reads settings seat i).
- Code: `crates/openmx/src/menu_cameras.rs` (new, `menu::cameras`); `chase.rs` (`Own`, `seat_presets`,
  `start_on`; `reframe` keeps the current preset by name, so a list that grows or shrinks doesn't jump);
  `menu_pages.rs` (rows/keys); `main.rs` (Rig `camera_presets`/`camera_started`, `camera_names` read from
  free06_2.res for the menu).
- Nothing was taken from opentennis; it has no camera presets.

## What the original does

The original has no saved camera presets and no custom ones. Its cpresets.ini / biplanecpresets.ini are
fixed, and every race starts on the file's first cycling preset. This is a port-side feature, so the only
1:1 requirement is that Original leaves the disc's numbers exactly. A test checks that.

## How it was proved

`crates/openmx/tests/menu.rs`:
- `a_seat_left_on_a_preset_starts_the_next_ride_on_it`: 2P is left on Freestyle Cam / Flight Cam 2, then
  settings.txt is saved and loaded, then `chase::start_on` runs. 2P starts there and 1P starts on the shell's.
- `a_custom_camera_preset_joins_the_cycle_with_its_own_numbers`: everything is driven by pad on the Camera
  page. It picks Slot Cam, saves "Low" on the keyboard, and moves its sliders. The test then checks:
  - the disc's laid presets are unchanged;
  - Low equals Slot Cam framed by its own numbers;
  - the button walks the disc's cycle, then Low, then back to the start;
  - Mine Only walks Low alone;
  - Freestyle Cam's own numbers touch only Freestyle Cam;
  - the settings round trip works;
  - Original gives the disc's file to the bit.
- The settings round-trip literal test covers the new lines.
- The tests read free06_2.res from the disc when it is there. Otherwise they use a stand-in with the disc's names.

## Not verified / not 1:1

- No live run of the shell with a pad. Nobody rode the camera button, the per-seat start or the Camera
  page's new rows in a window. The shell wiring in main.rs (first-frame start, saving on the button,
  reframing when the laid list changes) is compiled but not tested. The tests drive the same functions
  directly.
- The menu's preset names are read from free06_2.res only. I checked that free06, free09 and sx01 have the
  same names, but not every track. A track whose file lacks a name simply ignores that own/start entry.
- The wreck camera is still framed by the All Presets bike numbers, not per preset.
- Online: each seat uses settings seat i, like P82, but nobody ran a networked ride with differing seats.
  This is render-side only and never reaches the sim or the wire.
- A custom's base is matched by name. If a later disc file renamed a preset, the custom drops out of the
  list until it is saved again.

## Next

Nothing for P163. A live check with a pad would close the first point.
