# 1 — release 0.14.0 (FINAL)

Task P344. 2026-11-14.

## What I did

- **Released now, with the bots undrawn. Decided unattended.** P313a's own journal says the AI riders are
  in seat 0's Sim but are not drawn (V128), and after the drop they don't race the track's line
  (S331b, S331c). The user's complaint was "no bots", so on screen that is only half fixed. I released
  anyway: the plan was unblocked by hand at 18:10 ("bots on the grid (P313a) and the gate drop (P313b)
  are in") and moved to the top. The brief also says "nothing else holds it". The notes' first line
  says plainly that the riders are not drawn yet and don't follow the racing line yet.
- **What went in**, from v0.13.1, for players (the race lines first):
  - P313a AI riders on a one-player race's grid (Race Setup's Riders row);
  - P313b the drop gate falling;
  - P341 the pause Garage, Triangle quick-switch, LAST MACHINE, the Machines tile grid;
  - P342 MX World's phone hitches;
  - MD155 MX Unleashed: WORLD laid out evenly;
  - MD136 and MD138 the quad's camera, sound, tire particles and drawn rig;
  - MD153 a world's trucks, props and toys in the simulation;
  - S325 the Air Wrecks help.
- **Left out of the notes, but in the build:**
  - MD39c's MX vs. ATV cars and P288's car modifiers. No Garage row picks a car yet (MD158), only
    `--mxvatv-car` or settings.txt.
  - The research and oracle work: S310, S314a/b, S302, S315, S279a, F57a, F31b5n/p, MD81c/d, MD160,
    MD133a.
- **P313c (MoveGateDown sink) is not named in the notes.** The brief says to name it only if a track's
  gate still stands, and none does. All 46 race tracks were checked, see below.
- **The release.** `OPENMX_RELEASE_BRANCH=task/P344 PATH=~/.cargo/bin:$PATH tools/release.sh 0.14.0`,
  then I published it with `gh release edit --draft=false --latest` and ran
  `gh workflow run source -R FIZL0/apps`, as P343 and P284 did.
- Nothing from opentennis: its release.sh is already tools/release.sh's model.

## How it was proved

- **Gate and grid on every race track.** I ran a release build headless on all 46 race tracks
  (`--headless --steady-clock --track T --seconds 6 --race --laps 1 --racers 4`), sx01–sx24 and
  nat01–nat22:
  - every one found its drop-gate prop (`race: drop gate <name> drawn up …`);
  - every one dropped at tick 97 and turned it down by tick 101 (`turned -1.05 rad`);
  - every one laid out 3 AI riders.
  No track printed `has no start gate`.
- **The binary.** `dist/release/openmx-linux-x86_64 --version` prints `openmx v0.14.0 · 0764857`. That is
  `Release v0.14.0` (07648579, Cargo.toml and Cargo.lock at 0.14.0), which release.sh made on task/P344.
  It then emptied the notes (f9ccfbf4) and pushed the branch. The first run went through.
- **Signature and sums.** `minisign -V` verifies SHA256SUMS.sig against release.pub (trusted comment
  `openMX v0.14.0`). `sha256sum -c SHA256SUMS` is OK for all 7 files.
- **The APK.** `apksigner verify` passes. aapt2 badging reads io.github.fizl0.openmx, versionName
  0.14.0, versionCode 1400.
- **The IPA.** Its Info.plist has CFBundleShortVersionString 0.14.0 (CFBundleVersion 4358).
- **The release.** It is published and is the latest (`gh release view` gives v0.14.0). It has 10
  assets: altstore-source.json, apk, ipa, the two Linux and two Windows files, SHA256SUMS(.sig) and
  web.tar.gz. Its target is 07648579.
- **The website.** pages.yml's release run 43655577277 succeeded. https://openmx.dev/version.txt is
  `v0.14.0 0764857`.
- **FIZL0/apps.** Source run 43655577580 succeeded. source.json lists openMX 0.14.0 at
  .../v0.14.0/openmx-ios.ipa.

## Not verified / not 1:1

- **The AI riders are not on screen, and don't race the line after the drop.** V128, S331b and S331c
  are open, and the notes say so. Released anyway, for the reason given above.
- **No phone install.** The APK and the IPA were not installed on a device or an emulator. Only their
  signature, badging and plist version were checked.
- **The notes' lines rest on each task's own proof.** Apart from the gate and grid survey above, I did
  not replay the changes in the release binaries: P341's Garage, P342's hitches (phone only), MD155's
  layout, the quad lines, MD153's props, S325's help text.
- **The gate survey used a `cargo build --release` binary, not the shipped `--features ship` one.**
  `ship` only drops the OPENMX_* dev variables, which the survey doesn't use.
- **MoveGateDown (P313c).** Which disc tracks set it TRUE was not read. A track that does would turn
  its gate down but not sink it. The gate still falls everywhere.

## Next

Nothing: released. V128 (draw the AI riders) and S331b/S331c (make them race) are what the notes promise
for a later release.
