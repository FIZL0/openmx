# 1 — android background (FINAL)

Task P311. 2026-11-09.

## What I did

The user's report: "when i left the game on mobile the game still playing songs like an music app". The
Android shell had no lifecycle answer at all; iOS has had one since P298 (`away.rs`). I gave Android the
same pair, plus the back button and the vibrators.

- **Going away** — `crates/openmx-android/src/away.rs` (new, plain logic, desktop-tested): the patched winit's
  lifecycle hook (`android.rs` `lifecycle`) calls `pause` on the activity's Pause, `stop` on Stop and
  `resume` on Resume. Pause counts one going away. Stop counts one background and, if the Pause was already
  answered, asks again — iOS's resign-then-DidEnterBackground. Without that second ask the ride never gave
  its models back, since the ride loop only gives them back while answering, and the Pause had already been
  answered (seen on the emulator, fixed, seen again). `crates/openmx/src/away.rs` routes `away`/`answered`/
  `backgrounds` to it, so the ride loop's P298 block (pause the menu, give back models) now runs on Android.
  The menu and loading loops answer as on iOS.
- **Settings saved on going away** — `Front::save_now()` (main.rs) writes `menu::save` right away in all
  three answers (menu, loading, ride). A phone may end a background app without a word. This also applies
  on iOS now: it is harmless there, and the two shells share the code.
- **The sound stands still** — `audio.rs`: while `openmx_android::away::paused()`, the AAudio fill writes
  zeros, and the cpal thread `pause()`s the stream (`DeviceSound::stand_still`), then `play()`s it on return.
  Log lines: `sound: stands still, the app away` / `sound: on again`.
- **The Auto preset doesn't time a background app** — the resting frame loop (~10 fps while away) made the
  Auto graphics preset step down three times and save each step. `away::gone()` (Android: paused) now skips
  `Tune::frame_time` and drops its last timestamp, so the gap back is not a frame either.
- **Back button = Escape** (Circle/Back; the pause in a ride, back in the menus, the Quit popup on the
  splash). From Android 13 the system takes back for its gesture, so `MxActivity` registers an
  `OnBackInvokedCallback` (manifest `enableOnBackInvokedCallback="true"`, `tools/apk.sh`) that counts the
  presses, and `sys.rs` `backs()` reads them. Before 13, the key comes to the window as
  `Named(BrowserBack)`. Android 16 sent **both** for one press, which opened the Quit popup and closed it at
  once, so `android_gpu.rs` reads only one source, chosen by SDK level (`back_by_callback`, sdk ≥ 33).
  `GpuWindow::tapped(ESCAPE)` is then true for the frame. `menu.rs` `note_dev` ignores `Dev::Keys` on
  Android, so the back press doesn't switch the hints to keyboard glyphs.
- **Rumble** — `pad.rs` gets an Android `Motor`: the touch pad's node shakes the phone (`Vibrator::Phone`),
  and a controller's node shakes that `InputDevice`'s vibrators (`Controller(id)`). `MxActivity.vibrate` uses
  `VibratorManager`/`CombinedVibration` on 31+ and the deprecated `Vibrator` before that. Manifest: `VIBRATE`.

Nothing here is from the original game: it is the shell's platform glue. The mute-on-lifecycle idea is the
one opentennis uses (its M108), and the counts copy openmx's own iOS P298.

## What the original does

Nothing to compare: the PS2 has no background and no back button. The pause on going away matches what the
original's pause menu does when the player presses Start.

## How it was proved

Looked right on the emulator (AVD openmx36, Android 16, x86_64 SwiftShader; `OPENMX_ABIS=x86_64
tools/apk.sh`), reading `openmx-log.txt` and screenshots:

- Back on the splash: the Quit popup opens; back again closes it. Back on the main menu: to the splash. Hint
  glyphs stay ○/✕.
- Back in a Dallas free ride: the Paused menu, ride frozen; back again resumes.
- Home in the menus and in the ride, then return. The log of the last run, after both fixes:
  ```
  android: activity Pause
  settings: saved in …/files (away)
  sound: stands still, the app away
  android: activity Stop
  settings: saved in …/files (away)
  away: gave back 116 models, 93 materials and 93 textures in the background
  android: activity Start
  android: activity Resume
  sound: on again
  android: back, the frame loop rested 39.0 s
  ```
  and the ride came back paused (screenshot `context/p311/home-in-a-ride.png` in slot s3, git-ignored).
- `away.rs` tests (2) in `openmx-android`: pause/stop/resume counts, a dialog (Pause alone), and an old
  answer that doesn't answer a newer ask.

## Not verified / not 1:1

- **Rumble shook nothing I watched.** No wreck was driven on the emulator, and the emulator has no real
  vibrator. Both the phone and the controller paths are untested (`adb shell dumpsys vibrator_manager`
  during a wreck would show the phone's).
- **No real phone, no arm64 build, no real controller.** Everything above is from the x86_64 emulator.
- **The pre-33 back path** (the key event, `back_by_callback` false) never ran: the emulator is 36.
- **A controller's B/Circle on 33+**: if a pad sends KEYCODE_BACK, the system may route it to the back
  callback, so it would act as Escape rather than as the pad's Circle. Not tried, with no pad here.
- **The Auto preset skip is not seen working.** On the last run the preset was already at its cheapest from
  the run before (where it stepped down while away). The skip is a one-line guard (`away::gone()`), seen
  only as no new "auto graphics:" line in the background, which the cheapest preset would not print anyway.
  On the emulator the settings stayed stepped down; a phone that ran a build before this fix keeps whatever
  it stepped to.
- **Online rides keep going while away.** The session is not left after a grace time (opentennis M108 does
  that). The ride pauses locally, and peers see a stalled player.
- **iOS now also saves settings when it goes away** (shared `save_now`): not run on an iPhone.
- The resting frame loop still draws at ~10 fps while paused but in view (e.g. a dialog over the app). Only a
  hidden window rests fully.

## Next

Nothing for this task. If rumble matters, check `dumpsys vibrator_manager` on a phone during a wreck.
