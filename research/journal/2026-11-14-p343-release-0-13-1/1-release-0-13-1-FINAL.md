# 1 — release 0.13.1 (FINAL)

Task P343. 2026-11-14.

## What I did

- **What went in.** All four fixes have landed on main, so nothing is held back: P339 (iPhone music
  under another app's audio), P337 (sx and national dirt pages), P338 (a Race loads no hit events),
  P340 (iOS pictures posed on the refresh).
- **Released from HEAD, not cherry-picked onto v0.13.0.** Decided unattended. A cherry-pick branch
  would tag a commit that never reaches main, and release.sh tags HEAD of the branch it pushes, as
  P284 did. So 0.13.1 also carries what landed since v0.13.0 without a player line:
  - P313's lap line in the Sim and the shell (laps, places, finish to results; 0.13.0's notes already
    said a race has those);
  - S12e / S316 / S323's AI work in the Sim. The shell still spawns no AI racers (S12 is open);
  - the MX vs. ATV car and quad work (MD149, MD133b, MD135, MD150), research-side or behind the quad
    addition.
- **RELEASE_NOTES.md** (commit bc402d43), for players, from v0.13.0. One line per fix:
  - iPhone music, and how to get it back (Play Music);
  - dirt textures;
  - no hit event on race maps;
  - the iOS stutter.
  A fifth line says races still lack the other riders and the gate drop, and that they come in the
  next release (the brief: races go in the races release after S12e, S12 and P313).
- **The release.** `OPENMX_RELEASE_BRANCH=task/P343 PATH=~/.cargo/bin:$PATH tools/release.sh 0.13.1`:
  - it made `Release v0.13.1` (f6bb82a7, Cargo.toml/Cargo.lock at 0.13.1);
  - it built every platform and uploaded a draft with all 10 assets;
  - it emptied the notes (53adeebe) and pushed task/P343.
  The first run went through.
- **Published** with `gh release edit v0.13.1 --draft=false --latest` and
  `gh workflow run source -R FIZL0/apps`, as P284 did.
- Nothing from opentennis: its release.sh is already tools/release.sh's model.

## How it was proved

- **The binary.** `dist/release/openmx-linux-x86_64 --version` prints `openmx v0.13.1 · f6bb82a`.
- **Signature and sums.** `minisign -V` on SHA256SUMS.sig verifies against release.pub (trusted comment
  `openMX v0.13.1`), and `sha256sum -c SHA256SUMS` is OK for all 7 files.
- **The APK.** `apksigner verify` passes. aapt2 badging reads io.github.fizl0.openmx, versionName 0.13.1,
  versionCode 1301.
- **The IPA.** Its Info.plist has CFBundleShortVersionString 0.13.1 (CFBundleVersion 4271).
- **The release.** It lists 10 assets: altstore-source.json, apk, ipa, the two Linux and two Windows
  files, SHA256SUMS(.sig), web.tar.gz. Its target is f6bb82a7, and `gh release view` (latest) is v0.13.1.
- **The website.** pages.yml's release run 43566246196 succeeded. https://openmx.dev/version.txt is
  `v0.13.1 f6bb82a`.
- **FIZL0/apps.** Source run 43566246185 succeeded. source.json lists openMX 0.13.1 at
  .../v0.13.1/openmx-ios.ipa.

## Not verified / not 1:1

- **No phone install.** The APK and the IPA were not installed on a device or an emulator for this
  release. Only the signature, badging and plist version were checked. The iPhone fixes (P339 music
  take-over, P340 pacing) rest on their own tasks' tests, and those tasks list their own on-phone
  gaps.
- **The fixes were not replayed in the release binaries here.** Each line rests on its task's proof
  (P337's dirt_textures test and release-build shots, P338's release-build race check, P339's and
  P340's tests).
- **P313's race wiring ships without a release-build run.** P313's own journal says "Not run in a
  release build here". It is in 0.13.1 because the release is cut from HEAD, and the notes make no
  new claim about it.
- **The notes' music line names "the pause menu's music player".** That comes from P339's journal (the
  pause player's eyebrow reads PAUSED, its hold disc lets go). I did not look at it on screen.

## Next

Nothing: released. The races release (0.14.0) is the next.
