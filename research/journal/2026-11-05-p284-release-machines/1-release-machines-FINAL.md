# 1 — release machines (FINAL)

Task P284. 2026-11-05.

## What I did

- **What went in.** MD80e (MX vs. ATV's biplane and Kiowa, the Fliers / Flier Skin rows on Settings →
  Ride) has landed, so it is the release's first line. Untamed's machines:
  - MD81a's bikes were already in 0.10.3. `git merge-base --is-ancestor` shows f521a8c4 (MD81a) and
    0f3d8790 (MD93, the Untamed handling) are both in v0.10.3, whose notes already say "A new
    Untamed handling choice".
  - MD81b-f (race ATV, monster truck, baja, golf cart, sandrail) and MD82d are still open, so they don't
    hold this release. I noted P309, the next release, to take them if they have landed by then, and
    did not file a third release task (the user said "combine the release tasks").
  - P308 (Android) has landed, so release.sh ships `openmx-android.apk`: 10 assets.
- **openmx-mods.** The notes send players to `import-atv fliers`, but MD80d's commit 9f355f8 (and MD84's
  b98cefd, the packs' own FC_/RC_ cameras) were only on the `openmx-MD80d` / `openmx-MD84` branches, never
  on openmx-mods' main. Both tasks are ticked here. Other openmx tasks (MD61, MD70) put their commits
  straight on openmx-mods main, so I did the same:
  - cherry-picked both commits onto origin/main in a scratch worktree (the merge-tree was clean);
  - ran the workspace tests with openmx linked: 151 passed, 2 failed, both in import-music's
    tests/built.rs. They fail the same on origin/main 19577ed alone, so they're not from these commits.
    Filed as MD110;
  - ran `import-atv --test fliers -- --ignored` with the disc: 1 passed, 63 s;
  - fast-forward pushed as eb8e26c, 4a4e439. The scratch worktree and its branch are removed.
- **Tape and wire versions.** `git diff v0.10.3..HEAD`:
  - replay.rs (`FORMAT_VERSION` 1) is unchanged.
  - snapshot.rs adds `Flight::mxvatv`, which is kept off the wire. MX Unleashed's bytes don't change
    (MD100).
  - net/udp.rs only learns a peer named on port 0 (K29). The magics are unchanged.
  - So no tape or wire bump. Online builds of different releases don't meet anyway: the handshake's
    build id is the version.
- **The version** is 0.11.0 (new machines), per the brief.
- **RELEASE_NOTES.md**, from v0.10.3 for players:
  - the fliers first, with how to turn them on, what each skin gives and that they need the player's own
    pack (MX vs. ATV disc → openmx-mods `import-atv fliers` → `mods/machines/`, Read the Mods Folder
    on). It says they're off by default and offline only;
  - then Android, then the other player-visible changes since the tag (P304, P301/P301a/MD97, P299,
    MD95/MD99/MD61, P296/P302/P297/P298a, P306, P246, K42, P221), ahead of the five lines already
    there.
  - Commit 0aaa3c76.
- **tools/build.sh android was broken (P308).** Its line `tools/apk.sh ${features:---exact-sim}` handed
  apk.sh `--features native-sim` on the default native build. apk.sh refuses that, so the first
  release.sh run stopped at the apk. That was after the `Release v0.11.0` commit, before any push or
  upload. P308's `--dry-run` never builds, so it didn't show. Fixed in 84c1f12f: build.sh passes
  `--exact-sim` only for an exact build, and apk.sh builds native by default. The second run went through.
  `tests/player_build.rs` had pinned the broken line word for word. It now pins the fixed one; 3 passed.
- Nothing from opentennis: its release.sh was already the model for tools/release.sh.

## How it was proved

- **The draft.** `OPENMX_RELEASE_BRANCH=task/P284 PATH=~/.cargo/bin:$PATH tools/release.sh 0.11.0` made
  it at 84c1f12f, with all **10** assets: the 9 plus openmx-android.apk.
- **The binary.** `dist/release/openmx-linux-x86_64 --version` prints `openmx v0.11.0 · 84c1f12`.
- **Signature and sums.** `minisign -V` on SHA256SUMS.sig verifies against release.pub (trusted comment
  `openMX v0.11.0`), and `sha256sum -c SHA256SUMS` is OK for all 7 files.
- **The APK.** `apksigner verify` passes (Android Debug cert). aapt2 badging reads
  io.github.fizl0.openmx, versionName 0.11.0, versionCode 1100, arm64-v8a.
- **The notes.** The release body's "New in 0.11.0" leads with the fliers.
- **Published** with `gh release edit v0.11.0 --draft=false --latest`. releases/latest is v0.11.0.
- **FIZL0/apps.** `gh workflow run source -R FIZL0/apps` (run 42113831528) succeeded.
- **The website.** pages.yml's release run 42113829464 succeeded:
  - https://openmx.dev/version.txt is `v0.11.0 84c1f12`;
  - https://fizl0.github.io/apps/source.json lists openMX 0.11.0 at .../v0.11.0/openmx-ios.ipa.

## Not verified / not 1:1

- **The fliers weren't flown by hand for this release.** The notes rest on MD80e's shell tests and its
  own "not watched on screen" gap.
- **No phone install.** The APK and the .ipa weren't installed on a device or emulator for this
  release; P308 ran that check for the APK build. Only the signature and badging were checked here.
- **Missing machines.** MD81b-f's Untamed machines aren't in 0.11.0, and the notes don't name them as
  missing: they were never promised to players. They go to P309 (noted).
- **openmx-mods' import-music tests are red on its main (MD110).** They aren't from this task, but
  openmx-mods' `./check.sh` isn't green on the commit I pushed.

## Next

Nothing: released. P309 is the next release.
