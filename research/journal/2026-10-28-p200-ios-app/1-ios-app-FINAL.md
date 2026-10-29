# 1 — ios app (FINAL)

Task P200. 2026-10-28.

## What I did

- **Build tools from opentennis (M91).** `tools/ios-sdk.py` was copied verbatim. `tools/ios-build.sh` cross-builds `-p openmx` for aarch64-apple-ios with clang plus rustup's `ld64.lld` against `~/.local/share/hst-ios/iPhoneOS.sdk`, iOS 15.0 minimum, into `target/ios`. `tools/ios-ipa.sh` strips the binary and makes `Payload/openMX.app` with:
  - Info.plist: bundle id `io.github.fizl0.openmx`; version from `[workspace.package]`; build number = `git rev-list --count HEAD`; UIFileSharingEnabled and LSSupportsOpeningDocumentsInPlace; the scene manifest; landscape plus portrait (portrait upside-down on iPad only); NSLocalNetworkUsageDescription.
  - the P194 icon, `web/icon-180.png`.
  - The output is zipped to `target/openmx-ios.ipa` (11 MB).
- **winit.** opentennis's winit 0.30.13 fork is vendored at `third_party/winit` and patched into the workspace. It is the 1949d952 tree (M91's UIScene lifecycle for iOS 27, B694's `ValidOrientations::LandscapeRight`). `third_party/README.md` says what changed.
- **libc pinned to 0.2.189.** 0.2.191 gates `in6_ifreq` to macOS and netdev needs it on iOS; opentennis pins the same version.
- **`crates/openmx-ios`, a new crate.** Every unsafe UIKit, ObjC and C call lives here, so `crates/openmx` keeps `forbid(unsafe_code)`. It covers:
  - the Documents path;
  - the log: fds 1 and 2 are dup2'd into `Documents/openmx-log.txt`, with an atexit, signal and NSException trail (M105);
  - memory;
  - the safe area;
  - the orientation hold (`set_valid_orientations` + `setNeedsUpdateOfSupportedInterfaceOrientations`);
  - UIDevice orientation and CoreMotion gravity;
  - GameController extended gamepads in the standard mapping;
  - an AudioQueue for s16 stereo at 48 kHz.
  - `Holder` is opentennis's B694 rotation-lock logic, written as plain Rust and tested on the desktop (5 tests).
- **`crates/openmx/src/ios_gpu.rs`.** This is the iOS `gpuwin` (mounted by `cfg_attr(path)`, as `web_gpu.rs` is):
  - UIKit's loop runs on the main thread (`run_on_main`). The shell runs on a thread with a 64 MB stack.
  - The two threads meet in one shared state: size, safe area, touches, focus and close.
  - The wgpu instance and surface are made once on the main thread and lent to the open `GpuWindow`.
  - The picture is centred in the safe area. The touch pad is drawn over the whole window through a new non-clearing sprite pass, `game_render::gpu::Layer` / `Renderer::draw_layer`.
  - The main thread re-reads the safe area and the orientation hold every 100 ms.
- **`crates/openmx/src/touch_pad.rs`.** P197's `web/touch.js` in Rust, using the same layout in the same `u`:
  - stick zone with tap-to-click;
  - GAS/BRAKE pedals;
  - ✕○□△;
  - CLUTCH/R1/CAMERA/PAUSE;
  - hit slop;
  - "a button stays down until a read has seen it".

  It is drawn with menu_art (the Saira labels and drawn symbols). 7 desktop tests.
- **`crates/openmx/src/ios.rs`.** The app's entry point:
  - logs to Documents and makes Documents the working directory;
  - writes `kit.toml` naming the first `.iso` in Documents;
  - until a disc is there, shows a "No disc yet" page and writes a "Put your disc image here.txt" note, so the Files folder appears;
  - then runs `crate::run()`.

  Pads come out as `PadNode`s, js 8-byte events copied from `web_host.rs`. GameController pads are numbered 0–15 by first sight. The touch pad is index 16 and is listed whenever no controller is connected.
- **pad.rs / audio.rs / main.rs.** The wasm cfgs now take iOS too (Node type, scan, open, DeviceInfo). `Output::start` has an iOS branch that keeps the AudioQueue alive on a waiting thread, so `Output` is unchanged. `main()` dispatches to `ios::main()`.
- **Sending the IPA.** `/usr/bin/localsend` is LocalSend's Flutter GUI and has no sender CLI. The phone (`iphone-15-pro-1` on tailscale, 100.119.220.13) refused port 53317 and nothing answered a LAN sweep, because LocalSend on iOS listens only while its app is open.
  - I wrote a LocalSend v2 protocol sender (`prepare-upload` + `upload`, the protocol the app speaks) at `~/.cache/openmx/ls_send.py`.
  - It is running detached for 24 h and logs to `~/.cache/openmx/ls_send.log`.
  - It sends `~/.cache/openmx/openmx-ios.ipa` (the latest build) the moment LocalSend is open on the phone. The user then accepts it there.

## What the original does

Nothing here is the original's: this is the shell's platform layer.

## How it was proved

- `tools/ios-build.sh` and `tools/ios-ipa.sh` build and link a release aarch64-apple-ios binary. `llvm-objdump --macho --dylibs-used` shows it linking Foundation, UIKit, GameController, CoreMotion, AudioToolbox, Metal and QuartzCore; the IPA has the expected 3-file Payload.
- Desktop: `./kit check -- -p openmx-ios` (5 tests: the rotation-lock hold) and `./kit check -- -p openmx --bins touch_pad` (7 tests: layout inside the safe area, taps read once, multi-finger, sliding fingers, stick tilt, clicks) pass.
- The desktop build is unchanged apart from the new `touch_pad` module and `Layer`.

## Not verified / not 1:1

- **Nothing has run on a phone.** No device or simulator is reachable from Linux. Unproven: the app starts, the window/Metal surface comes up, rendering from the shell's thread, the touch coordinates, the safe-area insets, the orientation hold and rotation lock, GameController reads off the main thread, and AudioQueue sound. Any of these may need a fix once `Documents/openmx-log.txt` comes back from the phone.
- **Delivery is not confirmed.** The IPA was not received while I watched: LocalSend was not open on the phone. The detached sender delivers it when the phone opens LocalSend (until 24 h after 2026-10-28 21:20). Check `~/.cache/openmx/ls_send.log`.
- **Rendering off the main thread.** wgpu surface configure, acquire and present happen on the shell's thread, not the main thread. Bevy does the same on iOS (opentennis), but this code is not proven.
- **Touch pad visibility differs from touch.js.** On the phone the touch pad shows whenever no controller is connected; touch.js waits for the first touch. That suits a phone, but it differs from the web. No haptics on presses.
- **Portrait.** The picture is centred in the safe area; the menus/HUD are not re-laid out for portrait beyond what the shell does for any window shape.
- **Audio session.** No AVAudioSession category is set: the default SoloAmbient obeys the silent switch, and interruptions (calls) are not handled.
- **No keyboard on iOS.** Hardware keyboard input is not read.
- **Online play over LAN.** LAN discovery is unproven on iOS: there is no NSBonjourServices entry, and multicast may need Apple's entitlement.
- **libc pin.** libc is pinned to 0.2.189 for the whole workspace (only netdev on iOS needs it).

## Next

1. Open LocalSend on the iPhone so the waiting sender delivers `openMX.ipa`, then sign and install it (AltStore/SideStore).
2. Put the `.iso` in Files → On My iPhone → openMX.
3. Ride, and send back `openmx-log.txt` from the same folder; fix what it shows.
