# 1 — camera fov settings (FINAL)

Task P82. 2026-10-22.

## What I did

- **Settings → Camera** (menu.rs). It is a page like the game modifiers' page. Row 1 is whose camera:
  Every Player, or 1P..8P for one seat of a split. Then five sliders: Field of view (−10..+20 degrees
  on the preset's, a degree a notch), Bike distance, Bike height, Biplane distance and Biplane height
  (50..200 % of the original's, 5 % a notch). Then Reset To Default and Reset To Original. Cross on a
  slider puts it back on its **Original** notch, and the row then says `Original`. `Every Player`
  steps from 1P's value and writes it to every seat, so a mixed set comes together. A slider shows
  `Mixed` while the seats differ.
  - Where the row sits. The brief says "beside Controls", but the row is the Settings page's
    second-to-last, directly above Game Modifiers. Two reasons: P55's tests pin Modifiers as the last
    row (Up from the first row wraps to it), and inserting beside Controls would renumber eleven
    other tasks' row constants and their tests' walks. Its row reads `Camera  Default|Original|Custom`.
  - Saved in settings.txt as `camera = F D H PD PH`, one view per seat separated by commas, with the
    last view carried on. The line is written only when a seat is off the default, so older files
    and default files keep their shape. A missing line means the default, and out-of-range numbers
    are clamped.
- **The framing** (chase.rs `Framing`, `framed`). This is a view over the disc's presets, as the
  shell reads them for the layout and display. At Original, `framed` returns the presets untouched
  (`to_vec`). Off Original it splits each preset's eye into its two legs at the preset's pitch:
  behind = d·cos(angle_x) and above = d·sin(angle_x). It scales the two legs separately, then writes
  the new angle_x and FollowingDistance back. Min/max following distance and the reverse offset scale
  with the distance, and fov, min_fov and max_fov move by the offset. game_sim's camera is untouched
  (it is `deny(float_arithmetic)`, and the sim never reads the view).
- **Applied live, per seat** (main.rs). Each seat's rig remembers the framing its presets carry.
  Every frame, before the follow camera, the seat's view comes from `--camera-view` or the menu's
  settings. If it differs, the rig re-reads the presets for the current layout and display, frames
  them, and prints `camera view: …`. The display switch reframes too. Re-framing uses the camera's
  own set_preset, so it drops the player's zoom/nudge that frame.
- **The wreck camera** (V31) scales with the bike: its 13/100 ft distance limits times the bike
  distance share, and its 20..82° field-of-view limits moved by the offset. Its aim pitch and its
  5 ft floor stay the original's.
- **`--camera-view original|default|F,D,H,PD,PH`** on the shell, for screenshots and runs.
- **Shipped default**, picked by eye from headless shots at frame 59 of `--ride` (640x448). The
  candidates were original, 115/110/+4 and 125/115/+5, for the bike and the biplane. 125/115 put the
  rider small, so the bike gets 115 % back, 110 % up, +4°. The biplane's original camera sits very
  tight on the plane, so it gets 120 % back and 115 % up.

## What the original does

Race Cam 1 on the disc (free06_2.res cpresets.ini): AngleX 65, FollowingDistance 13, min 5, max 100,
FOV 85 (40..110), low-speed droop LowSpeedAngleXVelCutoff 30 with scale 0.5. Below the cutoff the
camera scales its pitch down (game_sim `Race::aim`). So standing still the eye sits at
32.5° elevation (10.96 behind, 6.99 above), and only at speed at the preset's 65°. The framing
leaves the droop alone.

## How it was proved

- `crates/openmx/tests/camera_view.rs`, on the disc's free06 presets (or a stand-in without the disc):
  - `the_original_notch_is_the_originals_camera_bit_for_bit`: the Original notch's presets are equal
    (Debug) to the disc's. The game_sim Race camera films the same 120 frames over both, bit for bit
    (position, look-at, fov).
  - `the_default_sits_further_back_higher_and_wider`: 600 frames behind a standing rider, with the
    droop off (the pitch at speed). Behind is 1.15× and above is 1.10× the original's, each within
    0.01, and the fov is +4.
  - `distance_and_height_move_the_eye_apart`: the distance slider leaves the height alone, and the
    height slider leaves the distance alone.
  - `the_wreck_camera_scales_with_the_bike`: Original keeps 13/100/20/82 to the bit; the default scales.
- `crates/openmx/tests/menu.rs` `the_camera_rows_change_by_pad_and_are_saved`: walks Settings → Camera
  by pad reports. Steps fov and distance for every player, Cross gives Original, and one seat (2P)
  changes alone. Reset To Original, then save/load round trip. Old-file and clamp parsing, and the
  Original notch's bike()/plane() = (0, 1, 1). The settings round-trip test carries cameras, and the
  pill test counts the new page (12 screens).

## Not verified / not 1:1

- The default is picked by eye from a handful of headless frames on free06 and the biplane's start.
  Nobody has ridden it, and the numbers are not tuned against play.
- On the ground, the biplane's distance slider barely changed the shot. That is perhaps the camera's
  terrain probe pulling the eye in, or the plane's own preset limits. It was not chased, and it is not
  tested in flight.
- Below the preset's low-speed cutoff the height share is not exact. The original's droop scales the
  framed pitch the same way it scales its own, so the eye is higher and further back but not by
  exactly 110 %/115 %.
- Moving max_fov moves the swing and look-ahead fov ratio a little when off Original. Not compared.
- The wreck camera's height is not scaled (only its distances and fov limits are).
- Re-framing mid-ride resets the player's zoom and nudge on that seat (set_preset). The live apply
  from the pause menu during a ride was not driven in the shell; only `--camera-view` at start was.
- The Camera row sits above Game Modifiers, not beside Controls (see above).
- No emulator was used. None of this touches the simulation, but no replay/acceptance run was diffed
  beyond `./kit check`.

## Next

Nothing for this task. A play session to tune the default would be the human's.
