# 1 — camera settings (FINAL)

Task P388. 2026-11-20.

## What I did

- The branch was cut before P388 was on main (a stuck rebase held it off until `fe1bee7b3`), so it was
  fast-forwarded to origin/main first.
- Compared what the task names (distance, height, angle, FOV, look/position filters, the `cpresets.ini`
  fields) with what the Camera page already had: P82's one set and P163's per-preset sliders already
  cover FOV, distance and height per preset. Missing were angle and the two filters.
- Each preset (disc's or the player's own) now has three more sliders on the Camera page, after Height:
  - **Angle**: degrees added to the preset's `AngleX`, after the distance/height shares, so the eye
    swings up or down about the target at the same distance (−30..+30, held short of ±89° pitch).
  - **Look Lag**: the preset's `LookFilters` time constants as a percent (10..300, step 10).
  - **Follow Lag**: its `PosFilters` the same way.
- `chase::Framing` has the three new fields (`Framing::of` keeps the old three-number shape);
  `chase::framed` applies them. `menu::cameras::Look` has `angle`, `look_lag`, `pos_lag`, `span`, `tune`.
  The shell's `own_framing` (main.rs) hands them to the camera for bike, biplane, helicopter and cars.
- `settings.txt`: a `camera_preset` line keeps its three numbers while the new ones are the disc's (a file
  from before P388 round-trips unchanged) and six numbers otherwise; both read back, held to each span.
- All Presets (P82's one set) keeps its five sliders; the new ones are per preset only, the disc's by
  default.
- The Xbox build's retuned presets as a choice: split off as P388a (later). The Xbox disc is not on this
  machine, and its numbers are game data that must come off the user's disc at runtime.
- opentennis: nothing like per-preset camera tuning there; nothing taken.

## What the original does

Nothing new: the camera reads `AngleX`, `LookFilters`, `PosFilters` from the preset as before
(`game_sim::camera`). These settings only change the preset numbers the shell hands it. Off (Original),
every number is the file's to the bit (`framed` returns the presets untouched at `Framing::ORIGINAL`).

## How it was proved

- `tests/menu.rs` `a_presets_angle_and_lags_override_its_file_and_save`: on the disc's own
  `cpresets.ini`, the new sliders start on Original and the presets equal the file; moving them on the
  Camera page changes Race Cam 1's `AngleX` by −5°, keeps its distance, halves `LookFilters`, scales
  `PosFilters` by 1.5, and leaves every other field and every other preset the file's; the line saves as
  `0 100 100 -5 50 150` and loads back; a three-number line round-trips; out-of-span numbers are held.
- `camera_view`, `untamed_framing`, `right_stick_camera`, `car_camera` and the menu camera tests still
  pass.

## Not verified / not 1:1

- MXA's camera options (MxaOptionsMenu.cs, MxaGameCamera.cs, MxaCams.cs) were not compared: `context/mxa`
  is not on this machine. The rows were picked from the task's own list of `cpresets.ini` fields
  instead. MXA may have options this does not.
- The Xbox presets choice is not done (P388a): the Xbox ISO is not on this machine.
- MX vs. ATV Untamed's slow/fast presets (the quad, Untamed cars) take only FOV, distance and height:
  their layout has no `LookFilters`/`PosFilters`, and the Camera page has no presets of theirs to move.
- How the new sliders feel in a ride was not looked at in the shell (no screen session in this run);
  the spans (±30°, 10–300%) were chosen, not tuned by eye.

## Next

P388a when the user's Xbox disc is on the machine.
