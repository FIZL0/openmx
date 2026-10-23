# 1 — camera settings steady (FINAL)

Task P119. 2026-10-22.

## What I did

- **Reproduced it in the shell.** New `--trace-camera` flag: one line a frame for each view, giving
  the preset, distance, pitch, yaw, fov, eye and look-at. First, headless rides on a generated
  throttle-and-weave tape at every slider's ends (`original`, `default`, `20,200,50,200,50`,
  `20,50,200,50,200`, `-10,50,50,50,50`, `-10,200,200,200,200`), on the bike and the plane. All were
  steady: constant distance and fov, the view never past 60° down outside a wreck or a crash. So the
  extremes are not the bug. Then a temporary, uncommitted hook switched `CameraView` at given ticks
  through the shell's own live re-apply path (main.rs, `framings(view) != rig.framing` → `frame_rig`).
  It walked every slider a step every two ticks to its ends and back. On the first step the fov read
  **3.4e38 (f32::MAX), then 0, then MAX, then 0**, every frame, until a wreck's snap ended it. That
  is the user's "zooms in and out very fast". A fov of MAX or 0 also wrecks the projection, which is
  the "upside down".
- **Cause.** The follow camera's lag on the fov (`Follow::fov_filter`) has a time constant of zero.
  Its blend is `min(dt, tau) / tau` = 0/0, which the PS2's divide answers with MAX. While the target
  holds still, `MAX * 0` keeps the lag where it is. The original's presets never move the fov under a
  running camera: every preset Select cycles through has its file's one fov, and the shell has no fov
  nudge. The first time the target moves by any amount, the value goes to MAX, the next frame to
  `MAX + MAX*(target - MAX) = 0`, and from then on it alternates. P82's reload (`set_preset` on the
  framed presets) moved the target by the slider's degrees. The distance and height sliders were not
  at fault: the lags chase the eye and the look-at as they do on a preset change.
- **Fix** (`chase.rs`, `reframe` and `FovEase`). The reload is the same as before: same preset, nudges
  cleared. After it, the camera's fov lag is set straight to the new preset's fov, so it never steps
  across a gap. The view the shell draws eases from the fov it showed, through an offset that decays
  with a 0.25 s time constant (`FOV_EASE`, about the presets' position lags) and is cut to zero under
  a hundredth of a degree. The ease starts from inside the new preset's fov limits. A camera that has
  not filtered yet, or snaps on its next frame, gets no ease. `frame_rig` (the Camera page's re-apply,
  and `set_display`'s reload on a display change, which also moves the fov: 85° full screen, 80° on
  the stacked split's copies) now goes through `reframe`. `Rig` keeps an ease for each of its two
  follow cameras, and the camera block applies the right one to the follow camera's view.
- opentennis has no camera framing settings, so nothing was taken from it.

## What the original does

The original never changes a running follow camera's presets, so it has no behaviour to match here.
The Camera page is ours (P82). The one original fact this rests on is that its fov lag is a filter
whose time constant the port's constructor leaves at zero (`research/oracle.txt`, the camera object:
"+0x114/+0x118 the fov and spare filter pointers"; a filter is value, tau, alpha, +MAX, -MAX).

## How it was proved

- The shell, headless, through `--trace-camera` and the temporary hook. Before the fix: fov
  MAX/0 alternating from the first step. After it, the same slider walk gave fov steps of at most
  0.5° a tick, eye-to-look-at reach steps of at most 7%, and eye steps of at most 1.8 ft, for the
  bike alone (with wrecks off), the plane, and both seats of a two-player split. The split's negative
  pitch (down to -0.33 rad) is the same at the Original notch, from the stacked view's pitch trim and
  the low-speed droop, so the framing has nothing to do with it.
- `crates/openmx/tests/camera_view.rs`, on the disc's `cpresets.ini` and `BiplaneCPresets.ini`, each
  the full screen's and the stacked split's (`h`) copies. A forced-failure probe confirmed that all
  four load from the disc, not the stand-in.
  - `every_camera_page_value_rides_steady`: every value of every slider (31 each, both ends among
    them), from the Original notch and from the default, which makes 310 framings on each of the 4
    cameras. Each rides two seconds behind a rider weaving at 40 ft/s, and the second second is
    checked every tick: the up vector's y positive, distance within the preset's limits, fov within
    0..180 and the preset's limits, the reach moving at most 25% and the fov at most 5% a tick.
  - `the_cameras_own_fov_lag_cannot_follow_a_change`: the cause, held. Under P82's reload a 1° change
    gives exactly `(f32::MAX, 0.0)` on the next two ticks. It also asserts the lag's time constant is
    still zero, so the day it changes the ease is known to be moot.
  - `a_change_mid_ride_eases_over`: each machine's three sliders walked a step a tick to both ends and
    back, then the Default and Original jumps and two end-to-end jumps, with one `reframe` for each
    change. Every tick passes the same checks. Two seconds after the last change, the camera matches
    one started fresh on that framing: fov bit-equal, eye within 0.05 ft. A second reload with
    nothing changed moves nothing, bit for bit.

## Not verified / not 1:1

- **That the original's fov lag really has a zero time constant.** The port's `Follow::new` gives it
  one, and S7/S24 proved the camera against captures in which the fov never moves, so a zero tau and
  any other tau look the same there. If the original's constructor sets a real time constant, the
  original would ease a fov change itself, and `reframe` leaves the lag to do that whenever tau is
  non-zero. Not checked, because no capture of the original changes a running camera's fov.
- **The eased fov is the shell's, not the camera's.** For up to about a second after a change, the
  drawn fov differs from what the camera hands back, by a decaying offset. The wreck camera's entry
  reads the camera's lag, not the shown fov. At rest the offset is exactly zero and the camera's fov
  is drawn untouched, so the Original notch is still the original's to the bit (the existing
  `the_original_notch_is_the_originals_camera_bit_for_bit` passes unchanged).
- **A jump from one end of the fov slider to the other** (-10 to +20 or back) starts its ease at the
  new preset's fov limit, not at the old fov: a step of up to 5° on the first tick. The page's own
  steps are 1°, so only a Reset row can do this, and only from the opposite end.
- **The menu itself was not driven.** The headless shell has no front end, so the reproduction swapped
  the view in through the same re-apply path the Camera page uses, not through presses on the page.
  Changes made under the pause (the ride frozen, the camera still stepped) were not run in the shell.
  The test's rider stands in for them only in the camera's own terms.
- The tests' rider is a synthetic weave on flat ground with no terrain (`Nothing`). The shell runs
  rode free06 on the tape. Neither covers a jump or the eye's ground floor under a framing change.

## Next

-
