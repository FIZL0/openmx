# 1 — camera holds on the faces (FINAL)

Task P78. 2026-10-26.

## The question, and the answer I chose

The task is a user question: the original binds look back to L2 and the other camera to R2, and under
the New layout (P40) L2/R2 are the brake and the gas, so which buttons do those two get, if any? This
was an unattended run, so nobody could answer. I chose this, and the user can overrule it:

- **Look back on Square, the other camera on Cross, under New.** These are the two faces P40 cleared
  when it moved the gas off Cross and the brake off Square onto the triggers. So New is now the
  original's layout with two clean swaps, R2 ↔ Cross and L2 ↔ Square, and every one of the original's
  commands has a button. Both are holds, which suits a face button: the right thumb is free under New
  because the gas is a trigger. The other free spots were worse. R3 is P114's camera reset. L3 and R1
  are the right stick's zoom and wide modifiers. The d-pad is the markers' (P88). L1 is the clutch and
  half the reset chord. Select is the camera walk (a long press there would add a timer the original
  doesn't have).
- The Controls screen's pad drawing (`menu_screens.rs`, from the concept) already labels Square
  "Look Back" under New, and R2 "Thrill Camera" under the original's. So Square agrees with the
  front end's own design. That drawing labels Cross "Stunts" under New (the MX vs. ATV extras), and I
  left it alone: Cross is the other camera only without that pack, and that camera has nothing to show
  until P156.
- **With MX vs. ATV's tricks (MD16) those two faces stay the extra trick buttons**, as before. The two
  camera holds have no button in that case, and the shell doesn't follow the look-back while a ride
  has the extras. Otherwise a trick in the air would flip the camera round.

## What I did

- `pad::Scheme::ride` (New): the old one-way move (R2 → Cross, L2 → Square, the pad's own faces let
  go) is now a full swap. The pad's Square reaches the ride as L2 and its Cross as R2, with the bit
  and the pressure byte together. Nothing in the ride reads L2 or R2 unless the trick pack is on
  (the Original layout hands them through already). `ride_tricks` overwrites L2/R2 with the extras
  exactly as before. `Scheme::commands_on` lists LOOK_BACK on Square and OTHER_CAMERA on Cross under New.
- `bindings::Command::RIDE` now carries LOOK_BACK and OTHER_CAMERA (16 commands). Added
  `Table::released` (either binding, like `pressed`) and `camera::Follow::look_back_preset`.
- `chase::LookBack`: the camera manager's look-back hold. A press, with the follow camera on the screen
  and a look-back preset in its file, switches to that preset and keeps the player's own. The release
  switches back if the camera is still on the look-back. `main.rs` keeps one per follow camera (bike
  and plane) in the rig, runs it before the Select walk, and prints `camera: <name>` on a change.
- The keyboard Controls list has a "Look back" row: H under New (H is Square, P75), S under the
  original's.
- The other camera is MXCameraThrill. The shell has no port of it, so its button only holds the
  Select walk back, as the original's does. I filed **P156** to port it and switch to it.
- opentennis has no camera holds, so I took nothing from it.

## What the original does

`./kit fn 0x00200000` is the manager's press handler and `./kit fn 0x002005e0` its release handler
(oracle.txt P78 has the field map):

- **Look-back press** (`./kit fn 0x002006e0`): if the race camera's look-back index is set (≥ 0), the
  race camera is set to that preset. That index is written once by `./kit fn 0x0021e270`, and it is
  the first preset with IsLookBackCam set. The press also clears the manager's "look back let go" word.
- **Look-back release** (`./kit fn 0x00200820`): the word goes back to 1. If the race camera is active
  and still on the look-back, it goes back to the manager's +0x154. That is the player's preset: the
  next-camera walk (`./kit fn 0x00200330`) writes it after every change, and the walk is refused while
  either hold is down. So "the preset up when the press came" is the same preset. The preset switch
  itself is the ported `Follow::set_preset`, which parks and restores the nudges.
- **Other camera press** (`./kit fn 0x00200740`): the manager's +0x64 camera is made active, but only
  if the target's velocity is longer than 3. The ctor `./kit fn 0x001ff7f0` builds that camera with
  `MXCameraThrill.cpp` as its file string (read from the ELF at 0x43d140). Its release
  (`./kit fn 0x002008b0`) restores the camera. If look back is still held, it puts the race camera on
  the look-back preset.

## How it was proved

- `crates/openmx/tests/right_stick_camera.rs` `the_look_back_command_holds_the_look_back_preset`. It
  runs through a real `Sim`'s polled pad and the shipped table:
  - L2 held is the look-back preset for 20 ticks.
  - Letting go goes back to the walked-to preset, with the stick's yaw nudge bit-equal to before.
  - It works the same from preset 0.
  - A press with the wreck camera up does nothing, and neither do R2, Square, Select or L1.
  - A file with no look-back preset does nothing.
- `crates/openmx/src/pad.rs` `the_new_layout_puts_the_camera_holds_on_square_and_cross`: under New,
  Square is LOOK_BACK, Cross is OTHER_CAMERA, R2 the gas and L2 the brake, with the pressure carried
  both ways. Under the original's, L2/R2/Cross are look back / other camera / gas.
- `select_cycles_the_camera_under_either_layout` now also asserts that Square or Cross held blocks the
  Select walk under New.
- The P73 test `each_layout_binds_the_originals_commands_and_no_other` still passes with the two new
  commands, so neither layout's buttons move the ride differently.
- By reading the decompilation only, against the functions above: no live capture. It is a camera, so
  presentation.

## Not verified / not 1:1

- **Which buttons is my call, not the user's.** The task asked the user, and nobody was there to
  answer. Square/Cross under New is my choice, and so is "no camera holds with MX vs. ATV's tricks".
- **Not checked against the running original.** I took no `./kit shot` of PCSX2 holding L2. The handler
  logic was ported from the decompilation alone. It is presentation and matched by eye, and the shell's
  look-back preset is the original's own record out of cpresets.ini, picked by the same rule (first
  with IsLookBackCam).
- **The other camera shows nothing.** MXCameraThrill is not ported (P156). Its button, Cross under New
  and R2 under the original's, only holds the Select walk back. The thrill camera's release
  going to the look-back while L2 is still held is not ported either.
- **A release during a wreck restores the preset at once.** The original's release handler does
  nothing unless the race camera is active, and the restore comes later through the manager's restore
  routine (`./kit fn 0x001ff140`) when the wreck ends. The port restores on the release itself.
  Either way the race camera ends back on the player's preset, but the frame it happens on differs.
- **The plane's camera gets the look-back too, if its file has a look-back preset.** I did not check
  that the original's manager treats the flight camera as its +0x54 race camera, or that Flight Cam's
  presets carry IsLookBackCam.
- **The +0x5c camera's branches are not followed.** The press and release handlers also switch from
  and back to that camera (built at `./kit fn 0x001ff550`), and the shell has no such camera.
- **The mode-5 start preset is left out.** The manager's initial +0x154 is preset 0, or the look-back
  index less two in mode 5. The shell starts on its own named preset (P14) and restores to whatever
  was up at the press.
- **The Select walk is still refused while the extras are held.** With MX vs. ATV's tricks on, the
  ride's L2/R2 are the extra tricks, and `changes_camera_on` still reads them as the two holds. This
  is unchanged from before P78.

## Next

Land it. P156 ports the thrill camera.
