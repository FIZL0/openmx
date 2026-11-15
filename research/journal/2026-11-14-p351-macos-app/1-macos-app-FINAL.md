# 1 — macos app (FINAL)

Task P351. 2026-11-14.

## What I did

Ported opentennis M162 (cfe77f50, 5f0f064e, 83da6c4c; abb59e30's log file for the Mac only):

- `tools/macos-build.sh`: the shell cross-built for `aarch64-apple-darwin` on Linux, clang + rustup's ld64.lld
  against `~/.local/share/hst-ios/MacOSX.sdk`, deployment target 11.0. As openmx's `tools/ios-build.sh`, it puts
  `~/.cargo/bin` first (the gate keeps the system rustc) and builds in `target/macos`.
- `tools/macos-app.sh`: `openMX.app` (stripped binary, Info.plist, an .icns from `web/icon-1024.png`), ad-hoc signed
  with rcodesign, packed with an Applications link by xorrisofs + dmg (libdmg-hfsplus) into
  `openmx-macos-arm64.dmg`. Native sim + `ship`, `--exact-sim` as `tools/ios-ipa.sh` takes it.
- `tools/build.sh macos` → `dist/macos/openmx-macos-arm64.dmg`. Not in `all`, as openmx's iPhone app is not (openmx's
  own convention; opentennis put it in `all`).
- `tools/release.sh`: checks MacOSX.sdk up front, adds `macos-app.sh` to the `ship` check, builds it, refuses a file
  with no UDIF `koly` trailer, uploads it (so it is in SHA256SUMS), and a download row that says how to open an
  unnotarized app (right-click → Open; macOS 15+: Privacy & Security → Open Anyway). No pages.yml job (dropped 11-03),
  so the beta has no Mac app.
- Updater: `update.rs` already had no asset for macOS, so a Mac's update notice opens the release page; a comment
  now says why (a swapped binary breaks the bundle's signature).
- Site (`web/index.html`): a Mac card in "Desktop & mobile", macOS in the JSON-LD and the PC FAQ. Its link always
  points at the latest release's .dmg, as the beta has none. README and RELEASE_NOTES lines.
- Beyond the opentennis scripts, two things a Mac build of openmx lacked that opentennis gets from Bevy:
  - **Sound**: cpal was compiled for Linux, Windows and Android only, so a Mac would have tried `pw-cat`/`aplay` and
    been silent. `audio.rs`'s cpal path now takes macOS (CoreAudio); the binary links CoreAudio + AudioToolbox.
  - **Log file** (`mac_log.rs`, abb59e30): opened from the Finder, stdout/stderr go to /dev/null, so they go to
    `openmx-log.txt` in `~/Library/Application Support/openmx`, the launch before's kept as `-previous`. Mac only:
    opentennis does Linux too, but that changes Linux launches and was not asked for.
- `tests/player_build.rs`: the Mac app is native + ship, build.sh passes `--exact-sim`, release.sh builds and checks it.

## What the original does

Nothing: platform packaging, no game logic.

## How it was proved

No Mac here, so by its files (all on the built `target/openmx-macos-arm64.dmg`, 14 MB):

- `llvm-objdump --macho --private-headers`: MH_MAGIC_64 ARM64 EXECUTE, PIE; `LC_BUILD_VERSION` platform macos,
  minos 11.0, sdk 15.5. Dylibs only system ones: AppKit, Metal, QuartzCore, CoreAudio, AudioToolbox, Foundation,
  CoreGraphics, Security, SystemConfiguration, Carbon, … (no Vulkan: wgpu draws through Metal).
- `rcodesign print-signature-info`: flags ADHOC, identifier `io.github.fizl0.openmx`, Info.plist and Resources slots
  bound. `rcodesign verify` fails with "CMS error": an ad-hoc signature's CMS blob is empty, and rcodesign itself warns
  its verify is unreliable (opentennis's M162 journal saw the same). So I checked it by hand with a scratch script:
  all 7661 code pages' SHA-256 equal the CodeDirectory's, code limit = the signature's offset, slot −1 = Info.plist's
  hash, slot −3 = CodeResources' hash (which lists AppIcon.icns). Flipping one byte of the binary makes it report 1 bad
  page, so the check does catch a change.
- The .dmg: `koly` trailer present; a scratch UDIF reader (blkx chunks) unpacks it byte-for-byte to the xorrisofs
  image, which holds `openMX.app/Contents/{MacOS/openmx (0555), Info.plist, Resources/AppIcon.icns,
  _CodeSignature/CodeResources}` and the `Applications -> /Applications` link. Info.plist parses: 0.13.1, build 4370,
  LSMinimumSystemVersion 11.0.
- `./kit check -- -p openmx --test player_build`: 3 passed.

## Not verified / not 1:1

- **Never started on a Mac.** No Mac on this machine: Gatekeeper's acceptance of the ad-hoc signature, the
  "Open Anyway" path, the window, Metal drawing, the AppleScript disc picker and the data folder are unproven on
  hardware. Only the file checks above.
- **Sound on a Mac** is unheard: cpal's CoreAudio path compiles and links, never played.
- **Pads on a Mac**: none. gilrs is compiled for Windows only (win.rs), so the Mac app has keyboard only. Filed as
  P352. The release row does not mention it.
- **The Finder-launch log file** is untested on a Mac (the fd-2 test and dup2 are opentennis's, unchanged).
- **The .dmg opening in Finder** (the volume layout, the Applications link) is not checked on macOS; only that the
  image round-trips and holds the right tree.
- **opentennis 22a6de6f** (pre-warmed pipelines from a bundled record) not ported: it is Bevy's pipeline cache;
  openmx's renderer has no such record.
- **83da6c4c's vsync change** not ported: a graphics default unrelated to packaging.
- **A full `tools/release.sh` run** not made (it signs and uploads); only the Mac part was built, by
  `tools/macos-app.sh` directly.

## Next

Nothing for this task. P352: pads on the Mac.
