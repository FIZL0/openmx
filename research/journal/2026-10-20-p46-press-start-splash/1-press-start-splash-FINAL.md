# 1 — press start splash (FINAL)

Task P46. 2026-10-20.

## What I did

- **A press-start splash before P36's main menu** (`menu.rs`: `Screen::Splash`, `Menu::splash`,
  `Menu::step_devices`, `Readers`, `Dev`). Every device is read on its own (`Readers`: a `Reader` per
  device, so a second pad's held stick is not someone else's press and a pad plugged in holding a button
  does not press it). On the splash the first Cross or Start from any device — a pad's, or the keyboard's
  Space (bound to Cross) or Enter (bound to Start) — is 1P; nothing else moves anything. A "Player 1
  <name>" card stays up 72 frames (opentennis's 1.2 s), then the main menu. Past the splash only 1P's
  device and the keyboard drive the screens (`Menu::hosts`; their presses are OR'd, `Press::or`).
  Circle/Escape on the splash opens a Quit / Stay popup that only the device that asked can answer (an
  idle pad's drift cannot flip it). Back on the main menu goes back to the splash with 1P free
  (opentennis's B533) — which replaces "Escape on the main menu quits": Escape now goes main → splash →
  Quit popup, and the main menu's Quit row is still there. `Menu::lost(plugged)` brings the splash back
  when 1P's pad is gone from the menus.
- **The seats start from 1P** (`pad.rs`: `Device`, `PadSlots::set_first`, `Seats::set_first`,
  `Seats::devices`, `Seats::keyboard_alone`). `set_first(Pad)` lets every seat go and puts 1P's pad in the
  first seat, the other pads behind it in join order on the next update; `set_first(Keys)` keeps the first
  seat for the keyboard and no pad takes it. With a pad as 1P the keyboard drives no seat
  (`PadSlots::keys` → `None`), except the first while 1P's pad is out — P35's take-over, which now also
  makes the keyboard 1P (`Seats::frame`), and `front_end` hands that to the menu after the ride. The pause
  in a ride reads 1P's seat and, when the keyboard is in no seat, `keyboard_alone()` beside it, so the
  keyboard still pauses and works the pause whoever has 1P. `--pads` seats are left as set up (the splash
  then only picks who drives the menus).
- **main.rs**: the front-end loop reads `Seats::devices` instead of seat 0's report, steps
  `step_devices`, and syncs the seats when 1P changes; it opens on the splash unless `--screen` says
  otherwise (`--screen splash` shoots it).
- **The art**: the last-ridden track's first still from `UI\trackpreview.res`, dimmed to 110/255, the
  `mainmenu_logo` (and shadow) a quarter larger and centred on its lettering, "Press [start] or [x]" in
  the title face with the `nav_start_off`/`nav_x_off` icons, breathing until answered, two lines of small
  print, and the navigation legend bar. I tried the disc's own `ui\tex\titlescreen.pst` first: it is the
  original's title screen with its logo and the copyright block baked in, so the prompt landed on the
  small print and the logo was drawn twice; the track still reads better. Shots looked at:
  `--screen splash --shot`, and the Quit popup and the claimed card from a throwaway test (removed).

### Taken from opentennis

`crates/hst/src/play/main_menu/splash.rs` (B516): the rule (first ✕/Start/Enter/Space is 1P, the keyboard
always hosts but moves no pad's player), the claimed card and its 1.2 s, the breathing prompt, the Quit
popup on Back (opening on Stay, arrows swap, ✕ chooses, ○ stays), Back on the main menu freeing 1P
(B533), and `takes_lost_p1`'s idea. Changed: opentennis hands 1P to the next device's press when 1P's
pad goes; the brief here wants the splash back, so that is what happens (in the menus). Opentennis's
splash crossfades between court pictures every 20 s; this one is a single still, as the brief says. Its
popup is answered by any device; here only the one that asked, since stray devices are the bug.

### P35's virtual-pad filter on this machine

What is plugged in now: no joystick at all — `/dev/input/js*` does not exist, `/proc/bus/input/devices`
lists a Logitech K400 keyboard, the AT keyboard and ACPI/HDA buttons, nothing with a gamepad handler, and
`SDL_GAMECONTROLLER_IGNORE_DEVICES` is unset. `/dev/uinput` is root-only (crw------- root), so no slot can
make a virtual pad right now either. The one virtual pad this machine has had (memory: "js0 is another
slot's idle pad") is the kit's own: `ps2kit/pad.py` makes it with uinput as **"Microsoft X-Box 360 pad",
045e:028e, bus USB** — a real Xbox 360 pad's ids. P35's `readable` only looked at ids (Valve's 28de:11ff
outside Steam Input, the ignore list under it), so **it let that pad in**: an idle virtual pad from
another slot became a seat, which is the symptom the user reported. Fixed: `DeviceInfo::virtual_device`
is whether the node's sysfs path resolves under `/sys/devices/virtual/input/` (where uinput puts what it
makes), and `readable` skips such devices except Steam's own (Valve's vendor) under Steam Input. BlueZ's
uhid pads (Bluetooth LE controllers) resolve under `/sys/devices/virtual/misc/uhid/…` and are hardware,
so they are deliberately not caught. `--pad PATH` names a device outright and is now read whatever it is.
Test: `pad.rs` `virtual_pads_are_skipped_whatever_ids_they_wear`, over a fake sysfs laid out the
kernel's way (class links into `devices/`).

## What the original does

Nothing to port: AGENT.md leaves the menus to us. The original has a title screen (`ui\tex\titlescreen.pst`,
logo + copyright) whose press-start behaviour I did not read; this splash is the port's own.

## How it was proved

`./kit check` passes. `crates/openmx/tests/menu.rs`:
`on_the_splash_an_idle_pad_s_drift_and_a_second_pad_s_stick_move_nothing` (600 frames of a drifting stick
crossing the lean threshold and a second pad's stick circling: still the splash, no 1P, no popup);
`the_device_that_presses_cross_is_1p_and_the_only_one_driving_the_main_menu` (a second pad's Cross after
1P's is nobody's; on the main menu its drift, stick and Cross move nothing, 1P's d-pad does, the keyboard
does too); `the_keyboard_s_enter_or_space_takes_1p_and_then_no_pad_drives`;
`an_unplug_of_1p_s_device_brings_the_splash_back` (another pad going changes nothing; the pause over a ride
is left alone); `back_on_the_splash_asks_to_quit_and_only_the_asker_answers`; and the existing
every-screen walk and draw tests, which now include the splash. `pad.rs`: `the_seats_start_from_1p`,
`devices_report_on_their_own_and_1p_takes_the_first_seat` (over fifo pads: per-device reports, 1P a pad
found second sits first on its own hands, `keyboard_alone`, the keyboard's take-over makes it 1P),
`virtual_pads_are_skipped_whatever_ids_they_wear`. Live: the shell with no flags on :0 opens on the splash
and sits there idle for 4 s (shot at frame 120).

## Not verified / not 1:1

- **No live press through the window.** Keystrokes do not reach windows on this desktop and there is no
  pad plugged in, so the claim, the keyboard/pad hand-off and the unplug were proved through the menu and
  seat state machines (and fifo pads) only, not by a hand on a real device in the running shell.
- **The virtual-pad filter is proved on a fake sysfs, not on a live uinput pad**: `/dev/uinput` is
  root-only here since 10-19 and nothing is plugged in, so I could not make one or see one. The sysfs
  layout (uinput under `devices/virtual/input`, uhid under `devices/virtual/misc/uhid`) is the kernel's as
  I know it, not observed today. Steam's own virtual pad and Steam Input were not available to check.
- **1P's pad unplugged mid-ride** still freezes the ride until it is back or the keyboard's Start takes
  over (P35); the splash only comes back once the ride is left for the menus. The brief's "the splash comes
  back" is met in the menus; doing it mid-ride would throw the ride away, which I judged worse.
- **With `--pads`**, the splash picks who drives the menus but the seats stay as `--pads` set them, so a
  ride's first seat may not be 1P's device.
- **Other seats past 1P** join in pad order as before (P38's join screen, blocked, is what should seat
  them); an idle pad can still take seat 2+ in a multi-player ride (`--players`), it just cannot drive the
  menus or 1P.
- **Look**: the splash's art and layout are by eye against MX's own menus, not against an original
  press-start screen (not looked at in the emulator).

## Next

Nothing for P46. P38 (join screen) should seat players 2+ on top of `PadSlots::set_first`.
