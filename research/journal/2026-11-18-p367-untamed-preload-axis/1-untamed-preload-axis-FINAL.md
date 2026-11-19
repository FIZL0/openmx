# 1 — untamed preload axis (FINAL)

Task P367. 2026-11-18.

## What I did

- Settings → Controls has a new per-seat row, **Preload Axis** (`menu_pages.rs`, `Key::PreloadAxis`). Choices: Off,
  Left Stick Down, Left Stick Right, Right Stick Down, Right Stick Left. Off is the default, which is how Untamed
  ships.
- `menu::PreloadAxis` holds the choice. `Settings::preload_axes` keeps one per seat. settings.txt writes
  `preload_axis = …` the same way as `markers` (one word per seat, the last word for the seats after it). Nothing is
  written while every seat is Off, so older files stay the same.
- Each choice is named by the push that reads plus. Untamed's preload only reads the axis's plus side (0 to 1).
  For three axes that is right/down. The right stick's X is inverted by the poll, so its plus side is left.
- `vehicle::with_preload` hands the axis to `ride::Controls::with_preload`, read off pad 0 through the bike's
  stick range (`phy.stick`). It only binds on `Handling::Untamed` and when not in a plane. Otherwise the controls
  pass through unchanged.
- The shell's `Seat` has a `preload` field. It is set live each frame from the seat's menu setting, next to
  `markers`, and `Seat::pass_front` applies it right after `vehicle::hands`.
- Online rides and tape playback leave it unbound (see below).
- I took nothing from ../opentennis. It has no bike preload.

## What the original does

- SLUS-21701 (Untamed) copies the device's axis word for action 0x53 into the bike when 0x53 is bound (MD93e's
  journal, R30). Untamed binds nothing to it, so a shipped ride never uses it.
- MX Unleashed has no such command. The port's Off is both games' shipped behaviour.

## How it was proved

- This is front-end and wiring only. The preload it feeds was proved by MD93/MD93e.
- `tests/menu.rs` test `each_preload_axis_preloads_on_the_push_its_name_says`: each choice's named push, all the way,
  reads `AxisRange::PLAYER.hi`. The other way reads ≤ 0. Off reads `None`. Words and settings.txt round-trip.
- `vehicle.rs` test `the_preload_axis_binds_on_untamed_handling_alone`: `None` on MX Unleashed handling and with
  no axis. The axis's value on Untamed handling.
- `tests/menu.rs` test `each_seat_picks_its_preload_axis_on_the_controls_tab`: the row's choices, 2P's pick only
  changes 2P, and the settings line. The settings round-trip test now carries a bound seat.

## Not verified / not 1:1

- **How Untamed scales the bound axis.** SLUS-21701 runs its raw axis through its own dead-zone remap
  (`./kit fn 0x00116e90` on Untamed). The port reads it through the bike's stick range (±0.3 dead zone, the
  same as the sticks). The two were not compared: Untamed's menus cannot bind 0x53, so no capture with it bound
  exists. The fire threshold (0.001) and the 0..1 force map are the ported ones.
- **No ride with a bound axis was played in the window or with a real pad.** Only the tests above check it.
- **Online rides leave it unbound.** Every machine rides every seat, and the binding is not in the handshake. So
  reading this machine's settings for each seat could desync. Carrying it in the handshake would be a separate task.
- **Tapes do not record it.** A tape played back rides unbound, whatever the seat had when it was recorded.
- **The right stick is also the shell's camera.** Binding a right-stick axis also moves the camera. The row's
  help does not warn about this.

## Next

- Nothing for this task.
