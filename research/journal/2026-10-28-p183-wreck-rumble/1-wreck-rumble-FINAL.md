# 1 — wreck rumble (FINAL)

Task P183. 2026-10-28.

## What I did

- `crates/openmx/src/pad.rs`: `Rumble` (seconds + level, clamped like the original's setter, `WRECK` =
  1.0 s at 200), `WreckRumble` (per seat: asks on a frame whose bike wreck entry ran, or on the first
  frame the plane's crash word is set; nothing with Vibration off), a linux `Motor` that finds the js
  node's sibling `eventN` through sysfs, opens it read-write and plays an `FF_RUMBLE` effect (strong =
  level·257, length = seconds in ms) through the `evdev` crate (the shell is `forbid(unsafe_code)` and
  the ioctl needs unsafe; gilrs would mean a second pad enumeration beside the js reader). `Pads::rumble`
  keeps every ask (`rumbles()`, for tests) and drops the motor on a write error; `Seats::rumble(seat, r)`
  goes to the pad that seat holds.
- `crates/openmx/src/main.rs`: `--no-vibration`; the ride loop feeds each local human seat's
  `WreckRumble` from `seat.said.wreck` (the same entry the wreck sound and the "wrecked" line read) and
  the plane's `flight.crash.reason`, and asks that seat's pad.
- `menu.rs`/`menu_pages.rs`: a Vibration on/off row, last on the Controls tab, saved as
  `vibration = off` only when off (on is the original's default).
- Tests: `tests/rumble.rs` (a real game_sim drop wreck → exactly one `WRECK` ask, none with Vibration
  off; the plane crash asks on its rising edge only, and a bike + plane frame asks once); pad.rs unit
  tests (the seat's own pad and no other; the clamps; an `#[ignore]` uinput pad whose motor receives
  the upload — length 1000, strong 51400, weak 0 — and the play, passed by hand); `tests/menu.rs`
  (the row toggles, saves and loads).
- Looked at opentennis' `assign_shake.rs`: it uses bevy's `GamepadRumbleRequest`; nothing reusable
  since the shell is neither bevy nor gilrs.

## What the original does

- `./kit fn 0x0027b810` (player_wreck_entry): for a human (+0x34 == 1) and the options word
  `*(*(app+0x3d4)+4) == 0`, calls the pad device's vtable +0x4c with (f12 = 1.0, dev, 0, 200).
- `./kit dis 0x0016b7e0` (the +0x4c setter, vtable 0x44ef70): a1 is the motor, a2 the level, f12 the
  seconds. Motor 0 (the big one) stores clamp(seconds, 0, 5) and the level clamped 0..255; motor 1
  (the small on/off one) only a time. Gated on an app flag (set to 1 by the app constructor) and the pad
  being in DualShock mode. So the wreck is the **big motor at 200/255 for one second** — P180's
  "1.0 for 200 ms" (and this task's brief) read the arguments the wrong way round.
- `./kit dis 0x0016ab10` (vt +0x6c, the pad update): counts the motor timers down by dt and sends the
  level bytes with scePadSetActDirect; a second ask while one runs restarts the timer.
- `./kit fn 0x00354aa0` (the biplane's wreck wrapper): the same (1.0, dev, 0, 200) for a human, with
  **no** options check, then the biplane wreck.
- All in research/oracle.txt (pad_rumble_set, pad_rumble_stop, pad_update_motors, and notes on
  player_wreck_entry and biplane_wreck).

## How it was proved

Disassembly read by hand (above). The port: `tests/rumble.rs` (2 tests), pad.rs unit tests (2 + 1
ignored uinput test, run by hand: `./kit check -- -p openmx --bin openmx -- --ignored a_rumble_turns`),
`tests/menu.rs` Vibration row test. No PINE comparison: the rumble is a pad output, not RAM state the
port compares.

## Not verified / not 1:1

- **1 s at level 200, not the brief's 200 ms at full.** Followed the disassembly; the brief came from
  P180's misread.
- **The options word +4 is "vibration"**: only P180's reading plus the ELF's VibrationOptions /
  STR_DYN_CONTROLLER_VIBRATION strings; I did not trace the options page writing it.
- **The setter's app-flag gate** (app+0x325 bit 1) was not traced back to any option; it is set on in
  the constructor and treated as always on.
- **The biplane honours Vibration** in the port; the original's biplane wrapper rumbles even with it
  off. Deliberate: one setting the player can trust beats a quirk. Easy to flip in `WreckRumble::frame`.
- **Timing**: the kernel times the effect (1000 ms wall clock); the original counts down by game dt,
  so a pause during the rumble keeps it going in the original's terms differently. Not modelled.
- **Strength**: level 200 mapped linearly to strong_magnitude 51400 of 65535 and weak 0. A DualShock 2's
  big motor and a host pad's strong motor respond differently; no attempt to match feel.
- **No real pad here**: proved with a uinput device only (upload + play arrive with the right
  numbers). A live shell ride with the uinput probe pad joined as 1P and gas held ran at 1 fps in this
  desktop's window and never reached a wreck in 25–40 s, so the ride-loop → device path was not seen
  end to end; it is covered in pieces (the rumble.rs ride, the seat routing test, the uinput test).
- **Online remote seats and keyboard players** never rumble (only local pads joined to a seat).
- A frame that runs several sim passes reads one `said.wreck`, as the wreck sound does; a wreck
  entry on an inner pass that is not reported would not rumble (same limitation as P180's sound).
- Non-linux builds: `Motor` is a stub; no rumble on macOS/Windows/web.

## Next

Nothing for this task. A follow-up could test with a real DualShock/Xbox pad for feel.
