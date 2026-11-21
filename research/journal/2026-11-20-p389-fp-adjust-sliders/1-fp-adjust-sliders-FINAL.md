# 1 — fp adjust sliders (FINAL)

Task P389. 2026-11-20.

## What I did

- Camera page: a **First Person** heading under Camera Button with four sliders — Head Bob (0–200 %,
  steps of 10), Eye Up and Eye Forward (±3 ft in tenths), First Person FOV (±30 deg). Cross puts one back
  to Original. Per seat, on the page's Player row, like the other camera sliders.
- `menu::FpTrim` (menu.rs) holds the notches, with step/label/span and the `settings.txt` line
  `fp_adjust = BOB UP FORWARD FOV[, …]` (one set a seat, the last carried on, as `camera`'s line).
  Written only when a seat is off the disc's trim, so old files are written as they were.
- The trim lives in menu.rs, not chase.rs, because nine test binaries mount menu.rs without chase.rs;
  `chase::FpAdjust` gains `From<(bob, up, forward, fov)>` and the shell converts `FpTrim::adjust()`.
- Shell (main.rs): `--fp-adjust` is now an `Option`; set, it wins; unset, the Camera page's trim for that
  seat, read live each frame (as the follow camera's framing is); without a front end, `FpAdjust::ORIGINAL`.
- Default `100 0 0 0` = `--fp-adjust 1,0,0,0` = the disc's camera, checked in the test.
- opentennis: nothing like it to take (its camera has no first-person trim).

## What the original does

Nothing: the trim is the port's own, after MXA's onboard adjust as an idea (P384).

## How it was proved

- `tests/menu.rs` `menu_sliders_set_fp_adjust_and_save_it`: pad taps move each slider, the trim converts
  to the same `FpAdjust` as `--fp-adjust 0.9,0.2,-0.5,3`, clamps at the ends, Cross resets, seats stay
  apart, saved and read back, short and out-of-range lines.
- `tests/menu.rs` settings round trip now carries a 2P trim; `tests/first_person.rs` checks the tuple
  conversion lands on the same `FpAdjust` the camera test frames with.

## Not verified / not 1:1

- Not looked at in the running shell: no ride was started with the sliders moved. The path from
  settings to `FirstPersonCamera::frame` is one line in main.rs and builds, but no test drives it.
- The menu test lives in `tests/menu.rs`, not `tests/first_person.rs` as the brief named: that file does
  not mount menu.rs, and mounting it would pull in about fifteen modules.
- Spec review fix: Reset to Default / Reset to Original and the Original tab's Cameras now put the trim back
  to the disc's (the port's default is the disc's here), and Cameras reads Original only with every seat's
  trim back. Tested in the same menu test.

## Next

Done.
