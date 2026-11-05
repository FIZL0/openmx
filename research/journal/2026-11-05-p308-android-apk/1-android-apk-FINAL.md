# 1 — android apk (FINAL)

Task P308. 2026-11-05.

## What I did

The Android app, `dist/openmx.apk`, built and released the way opentennis (f8cdd68c) builds and releases its own.

Taken from opentennis, and what changed for openmx:

- **`tools/apk.sh`**, from opentennis's `tools/apk.sh`: a NativeActivity with no gradle. The cdylib comes from an NDK
  clang cargo build for each ABI in `OPENMX_ABIS` (default arm64-v8a; add x86_64 for the emulator) and is
  llvm-stripped. The Java goes through `javac -Xlint:all -Werror --release 17` and then R8. aapt2, zipalign and
  apksigner with `~/.android/debug.keystore` finish it (keytool makes the keystore if it is missing). `--install` runs
  adb. `OPENMX_VERSION=v1.2.3` sets versionName and versionCode 10203. Changed for openmx:
  - package `io.github.fizl0.openmx` (the iPhone app's bundle id), label openMX, lib `openmx`;
  - the shell is built as the `openmx` *example* (`crates/openmx/src/android_so.rs` is a symlink to main.rs,
    `crate-type = ["cdylib"]`, `required-features = ["android"]`), because openmx is a bin crate and its lib root is
    web.rs;
  - features `android,ship,native-sim` (`--exact-sim` drops native-sim, as tools/build.sh does), with
    `OPENMX_COMMIT`/`OPENMX_VERSION` passed through;
  - the build goes into its own `target/android`, so the desktop's cache is left alone;
  - it looks for a JDK 17+ (JAVA_HOME, mise's temurin-21, /usr/lib/jvm), because the javac on PATH here is Java 8;
  - Restart.java and its activity are gone: openmx's disc copy goes straight on into the game without restarting the
    process.
- **Launcher icon** from `web/icon.svg`: the rider in cream on the menus' dark red #8e0f17.
  - The adaptive foreground is the rider without the frame `<rect>`, at 58dp of the 108dp canvas, so the wheels stay
    inside the 66dp safe circle. At 64dp they touched it in a circle-mask preview.
  - The background is the colour; the themed (Android 13+) monochrome layer is the same rider's alpha.
  - Legacy icons: square is the whole rounded svg at 48dp; round is the rider on a red disc.
- **rustls-platform-verifier (opentennis B418)**: as opentennis. The AAR version is checked against Cargo.lock
  (0.2.0), and the AAR and kotlin-stdlib 2.2.10 are fetched once into `~/.cache/openmx-apk` and checked against pinned
  sha256s. The AAR's `classes.jar` and its `network_security_config.xml` go in. `openmx_android::init_tls` runs at
  start.
- **`crates/openmx-android`** (new crate), from opentennis's `hst-base/src/android.rs` and `android_pad.rs`. It holds
  every JNI/libc call, so `crates/openmx` keeps `forbid(unsafe_code)`:
  - files dir and log;
  - pick/taken (the system file picker), safe area, model, open_url;
  - input devices, from_store, install_apk/install_status;
  - `drop_env`, which is new here.
  - `restart` was dropped. `pad.rs` (key codes and axes to the standard pad) builds everywhere, with two tests.
- **`crates/openmx/android/.../MxActivity.java`**: opentennis's HstActivity under openmx's package. It has the
  cutout/immersive setup, the picker, the installer hand-over (M140) and the Wi-Fi MulticastLock "openmx-lan"
  (opentennis B455, for the LAN lobbies' mDNS). `restart()` was removed.
- **`crates/openmx/src/android.rs`** (new): `android_main`, then in order:
  - with `ship`, the OPENMX_* variables are dropped (`OPENMX_TEXTURE_RELOAD` is kept), the B492 idea;
  - files dir, log (the old one kept as `openmx-log-previous.txt`), TLS, lifecycle hook;
  - a leftover `update.apk` is removed, the cwd is set to the files dir;
  - the disc (M99), then `crate::run()`.
  - **Disc page**: openmx's own, drawn with the menus' ink. It says which disc to use (MX Unleashed (USA),
    SLUS-20767, .iso). A tap opens the system picker. The pick is copied in on a thread as `disc.iso.part`, checked
    for `CD001` at byte 32769, then renamed, with a MB progress line. It also takes an .iso pushed by hand, once its
    size holds between two looks a second apart, so a file adb is still writing isn't taken. Then it writes
    `kit.toml` (`[game] iso = …`), as the iPhone app does (P200).
  - **Pads**: the iPhone's PadNode/`gamepad/<i>` approach (P200), fed by the opentennis winit fork's `gamepad::take()`
    events and the list of input devices. The touch pad is `gamepad/16`, as on iOS.
- **`crates/openmx/src/android_gpu.rs`** (new): the iPhone's `gpuwin` for Android.
  - One process-wide winit EventLoop and window, kept in a thread-local and reused by each `GpuWindow::open`.
  - The surface is dropped inside `suspended`, before Android frees the window.
  - Safe-area insets, touch to `TOUCH`, and the frame loop rests while there is no window.
- **Wiring** (cfg lines widened from ios to `any(ios, android)`, nothing moved):
  - `pad.rs` and `window.rs`;
  - `audio.rs`: cpal (AAudio). As on Windows, a non-F32 device is an error, not a fallback;
  - `video.rs`: touch, model, RAM from /proc/meminfo;
  - `modstore.rs`: open a URL through an Intent;
  - `main.rs`: the module and `use android as phone` after `modules!`, because the macro takes only `mod` items.
- **Updater (M140)**:
  - `update.rs`'s Android asset is `openmx-android.apk`, downloaded into `files/update.apk`.
  - It is minisign-checked like the desktop's, then handed to `MxActivity.install` (PackageInstaller session), and the
    result is polled.
  - An app installed from a store doesn't update itself (`from_store`).
- **`tools/build.sh android`**: runs tools/apk.sh, passing `--exact-sim` on. It isn't in `all`, the same as the iPhone
  app.
- **`tools/release.sh`**:
  - builds `openmx-android.apk` into the release; it is in SHA256SUMS through the `openmx-*` glob, so in the minisign
    signature;
  - adds an "**Android** 8+" row to the notes;
  - checks for the Android SDK up front;
  - the uploaded files are one `assets` list, which `gh release create` uses;
  - **`--dry-run`** is new: no key, builds, commit or upload. It prints that list and writes the notes into
    `dist/release-dry/notes.md`. The task's test line asked for one; opentennis has none.
  - The header says to keep `~/.android/debug.keystore`, because a phone only takes an update signed with the same key.
- `crates/openmx/tests/player_build.rs`: apk.sh builds native with ship, build.sh android passes --exact-sim on, and
  the release builds the apk.

## What the original does

Nothing here: this is a platform port and release tooling, with no game behaviour.

## How it was proved

- **`OPENMX_ABIS=x86_64 tools/apk.sh` → `dist/openmx.apk`** (36 MB). `aapt2 dump badging`: io.github.fizl0.openmx,
  min 26 / target 36, INTERNET, CHANGE_WIFI_MULTICAST_STATE, REQUEST_INSTALL_PACKAGES, the adaptive icon.
- **On the x86_64 emulator** (AVD `openmx36`: android-36 google_apis, pixel_6, `-no-window -gpu swiftshader_indirect`):
  - `adb install` worked.
  - First start with no disc: the disc page showed. A tap opened DocumentsUI's picker. I picked
    `Download/MX Unleashed (USA).iso` (pushed there beforehand), and the copy-in ran (2117 MB in about 16 s), found
    CD001, wrote kit.toml and went on into the game in the same process.
  - It reached the title screen ("Press START or ✕", "Controllers: Keyboard, Touch pad (gamepad/16)"), with the touch
    pad drawn.
  - The touch ✕ opened the main menu; ✕ again opened Free Ride, and Dallas loaded and rode with gas held (HUD,
    minimap, dust, 4 mph at 0.7 fps on SwiftShader).
  - The log (`files/openmx-log.txt`) had: the version line with the model, SwiftShader Vulkan into Rgba8Unorm, insets
    [128,0,0,0], "sound: through cpal", 54 bikes / 80 riders, and the activity's Pause/Resume while the picker was up.
  - Screenshots came from `adb exec-out screencap` and were looked at. They aren't kept.
- On the first copy attempt the emulator's MediaProvider (the /sdcard FUSE daemon) was killed: the Photos cloud
  provider timed out and took it down, and vold then killed every process with a file open on /sdcard, the app
  included, at 283 MB. That was the emulator, not the app; the second attempt went through.
- **`tools/release.sh 0.11.0 --dry-run`** listed `openmx-android.apk` among the ten assets, and the notes have the
  Android 8+ row.
- `./kit check -- -p openmx-android` (2 tests) and `-p openmx --test player_build` (3) passed. `cargo check` of the
  desktop bin (Linux, Windows) still builds with the warning counts it had (89 on Linux; no Windows warning is on a
  changed line). The Android build has fewer warnings than the desktop. `./kit audit` ok.

## Not verified / not 1:1

- **No physical phone, and no arm64 build run.** Only the x86_64 emulator on SwiftShader (CPU Vulkan, 0.7 fps) ran
  it. The arm64-v8a .so, the default ABI and what players get, was not built in this session. A real GPU's speed and
  look are unknown, and so is the green patch in the ride's top-right corner (likely SwiftShader, unchecked). → P310.
- **The release itself was not run** (this task must not publish), so the apk in a real release, its versionCode, and
  its SHA256SUMS line have only been checked by the dry run and the pieces run alone.
- **In-game update not run end to end** (M140): there is no release with an apk yet. The download, the minisign check
  and the PackageInstaller hand-over (MxActivity.install, `install_status`) have never run. → P310.
- **HTTPS (B418) not exercised**: init_tls ran without logging an error, but no request was made (the store page
  wasn't opened at 0.7 fps, and the update check needs `OPENMX_VERSION`). If init_tls fails, it's logged and a request
  later fails in its job thread. There is no webpki fallback.
- **LAN multicast (B455) not tested**: the emulator's log shows the lock taken and released (`acquireMulticastLock
  … tag=openmx-lan`), but no lobby was tried.
- **Controllers on Android not tested**: no Bluetooth/USB pad on the emulator. The key/axis mapping (openmx-android's
  `pad.rs`) is tested on the desktop only.
- **Sound**: cpal's AAudio stream opened ("sound: through cpal"; the emulator ran with `-no-audio`). A device whose
  default isn't F32 gets no sound, as on Windows, and nothing falls back.
- **Not like opentennis / missing**:
  - no rumble on Android;
  - the ride doesn't pause and settings aren't saved when the app goes to the background (the frame loop only rests);
  - the phone's back button does nothing. → P311.
- **`ship` acts on Android only.** opentennis builds every player build with it; openmx's desktop, iOS and web builds
  don't yet. → P312.
- **Menu taps**: tapping a menu row didn't select it; the touch pad's buttons work the menus. I didn't check whether
  iOS behaves the same.
- **Signing key**: `~/.android/debug.keystore` didn't exist and was made by this run's keytool. Releases must keep
  using this file, or phones refuse the update.

## Next

Done here once the gate passes. Then P310 on a real phone (arm64).
