# 1 — android real phone (READY)

Task P310. 2026-11-11.

## What I did

- Looked for a phone: `~/Android/Sdk/platform-tools/adb devices -l` lists none, and ~/Downloads holds only the
  iPhone's openmx-log files (2026-10-28/30), no Android log. Everything P310 asks (a real GPU, AAudio, a
  Bluetooth/USB pad, LAN lobbies through the MulticastLock, rustls-platform-verifier's HTTPS, the in-game
  update) only a phone can show, so nothing here could be run.
- Checked what the human needs is already out: v0.11.0 and v0.12.0 both carry `openmx-android.apk`, so
  installing 0.11.0 and taking the in-game Update to 0.12.0 tests M140 end to end with no new release.
- Wrote the recipe as plan/HUMAN.md "openMX on a real Android phone (P310)": install, disc, ride, pad, sound,
  LAN lobby, store, update, and which log lines to send back (`files/openmx-log.txt` and `-previous`).
- No product change; tools/release.sh untouched.

## What the original does

Not applicable: platform work, nothing of the original's.

## How it was proved

Not proved: no phone attached.

## Not verified / not 1:1

- GPU speed and look on a real phone — no arm64 device here; P308's only run was the emulator on SwiftShader.
- AAudio sound — the emulator ran `-no-audio`, and no phone.
- Bluetooth/USB controller on Android — no phone, no pad on a phone.
- LAN lobbies through the Wi-Fi MulticastLock (B455) — the emulator's network has no real Wi-Fi multicast.
- Store HTTPS through rustls-platform-verifier (B418) on a phone's trust store — no phone.
- In-game update 0.11.0 → 0.12.0 (M140) through the system installer — no phone; also unchecked whether
  both releases' apks carry the same signing key (both should be ~/.android/debug.keystore).

## Next

-READY: the human follows plan/HUMAN.md "openMX on a real Android phone (P310)" and puts the logs in
context/p310/; the next session reads them and files a task per thing that went wrong.
