# 1 — ios source (FINAL)

Task P201. 2026-10-29.

## What I did

Taken from opentennis (6e0730cb, plus its later switch to the shared source on origin/main): `tools/ios-source.sh`,
`web/ios.html`, the release script's source upload, its iPhone row and `gh workflow run source -R FIZL0/apps`. What
changed for openmx:

- `tools/ios-source.sh VERSION IPA [OUT]`: the AltStore v2 source (AltStore, SideStore and Feather all read it) with the
  legacy top-level `version`/`downloadURL`/`size`, icon (openmx.dev/icon-180.png), tint (the menus' maroon #8e0f17),
  subtitle, and the release's IPA URL and size. Repo FIZL0/openmx, bundle id io.github.fizl0.openmx (tools/ios-ipa.sh's).
  It checks for jq. Its `sourceURL` is the release's own `latest/download/altstore-source.json`, which is the URL
  FIZL0/apps' build.sh fetches (checked read-only: `REPOS=(opentennis openmx partyboard)`, merged into `source.json`).
- `tools/release.sh`: builds `openmx-ios.ipa` (tools/ios-ipa.sh) into the release, writes the source and uploads both
  (the IPA is in SHA256SUMS through the `openmx-*` glob). It checks for the iOS SDK and jq up front, before the builds.
  The notes' Download table has an iPhone/iPad row: "add the source in one tap" → openmx.dev/ios.html, the shared URL
  `https://fizl0.github.io/apps/source.json` by hand, and the unsigned IPA. After a published Latest release it runs
  `gh workflow run source -R FIZL0/apps` (a warning, not a failure, if that is refused). A draft published later on
  GitHub waits for FIZL0/apps' hourly poll instead.
- `web/ios.html` (openmx.dev/ios.html): Add to Feather (`feather://source/<url>`), Add to AltStore, Add to SideStore,
  Copy source URL, and how to put the ISO in Files (the app writes kit.toml for the first .iso in its Documents, P200).
  In the site's colours and Saira, not opentennis's. All of it points at the shared FIZL0 source, as the user asked.
  In the sitemap too.
- `web/index.html`'s Desktop & mobile iPhone card: no longer "on its way". It links ios.html ("Get it on iPhone",
  "Step by step") and keeps the direct Feather/AltStore/SideStore/copy buttons. README's iPhone paragraph links ios.html
  and gives the shared URL. RELEASE_NOTES.md has a bullet.
- `.github/workflows/pages.yml`: a new `ios` job on `macos-latest` builds the beta's unsigned `openmx-ios.ipa` with
  tools/ios-ipa.sh, stamped with the same commit as the Linux and web jobs. It is `continue-on-error`, and `beta`
  attaches the IPA only if it is there, so a broken iOS build never holds back the Linux beta. The Linux cross-build
  can't run on a runner because it needs Xcode's SDK out of a .xip.
- For that job: `tools/ios-build.sh` has a Darwin branch (Xcode's SDK and linker: plain `cargo --target
  aarch64-apple-ios`). `tools/ios-ipa.sh` drops `realpath -m` (a Mac's realpath has no -m), uses `strip` when there is
  no `llvm-strip`, and keeps the caller's `OPENMX_COMMIT`. Before, it overrode the release's 7-character stamp with
  `git rev-parse --short`, which can be longer and would give the iPhone a different online build id.

## What the original does

Nothing: release tooling and web pages, no game behaviour.

## How it was proved

- `tools/ios-source.sh` was run on a scratch file: the JSON parses, and it has the version, both download URLs and the
  size. Its two usage errors fire.
- `bash -n` on every changed script. pages.yml parses as YAML, and `beta` needs `ios`.
- `web/ios.html` was rendered in headless chromium at phone width and looked at.
- `OPENMX_COMMIT=abc1234 tools/ios-ipa.sh <scratch>/openmx-ios.ipa` was run here with the edited scripts (Linux cross-build). It
  made an 11 MB IPA with the binary stamped `abc1234`, the caller's commit, and `tools/ios-source.sh 0.1.0` on it gave
  size 10846486 and the v0.1.0 URL.

## Not verified / not 1:1

- **No release was run.** `tools/release.sh` publishes, and this task must not publish, so the new steps (IPA in the
  release, source upload, notes row, `gh workflow run source -R FIZL0/apps`) have run only as far as `bash -n` and the
  pieces run alone.
- **The macOS `ios` job in pages.yml has never run.** It runs on the first push to main. Not known: whether the
  workspace builds natively for aarch64-apple-ios on a Mac (P200 only cross-built from Linux), whether Apple's `strip
  -o` behaves like llvm-strip here, and whether job-level `continue-on-error` lets `beta` run when `ios` fails (GitHub's
  documented behaviour, unobserved here).
- **The add-source links were never tapped on a phone** (feather://, altstore://, sidestore://). No device can be reached
  from here. The URL forms are opentennis's, and those are unconfirmed on a device too.
- **FIZL0/apps was not triggered and its source was not checked with openmx in it.** No openmx release carries
  altstore-source.json yet, so build.sh skips openmx until the first release after this lands.

## Next

Nothing for this task. At the first release, check that https://fizl0.github.io/apps/source.json lists openMX.
