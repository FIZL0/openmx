# 1 — landscape and keep-awake (FINAL)

Task F83. 2026-11-23.

## What I did

- Landscape only. The Portrait hold is gone: `crates/openmx-ios/src/hold.rs` no longer answers Portrait,
  and `tools/ios-ipa.sh` drops portrait from the iPhone app's Info.plist. P395's portrait menu strip
  can no longer show, so P395 can go.
- Keep-awake: `crates/openmx/src/keep_awake.rs` counts the downloads in progress (mod store installs,
  app update, Android disc copy-in). The screen stays awake while the count is above zero, and auto-lock
  comes back when the last one ends. Uploads are not counted.

## How it was proved

- iPhone 15 Pro, over omarchy-desktop. The app opens at 2556×1179, landscape. Its log has only
  `ios: holding Landscape`. A portrait turn asked by WebDriverAgent was refused. Nobody turned the phone by hand.
- Keep-awake, with Auto-Lock at 30 s: Download All on the store's Tracks tab (102 mods). The log says
  `screen: kept awake for a download`. The screen was untouched and still lit in every shot from 15:46:27
  to 15:47:06, about 45 s after the last touch (15:46:21). The log's `auto-lock back` came by 15:47:13, and
  the phone locked by 15:47:36.
- Control, with no download: the screen went off about 30 s after the last touch.
- Recipe: Select (the CAMERA touch button) is Download All. The 318 MB of music takes under 8 s there,
  which is too short to test, so use Tracks. The "Load the new mods?" box swallows presses; Circle is Later.
- Afterwards the 104 test mods were removed over fsync, and Auto-Lock was put back to Never.

## Not verified / not 1:1

- **Android is built, not run.** Nothing on an Android device shows the landscape lock or the keep-awake.
- **Only the iPhone 15 Pro, one download.** The Download All of 102 mods is the only download run on the
  phone. A failed or cancelled download was not run there, so the end-of-download path is seen only on a
  download that finished.
- **Uploads are not kept awake.** That is the task's scope, not a gap in the test.
- **The unit tests are not in these notes.** Whether `./kit check` is green on this commit is not recorded here.
