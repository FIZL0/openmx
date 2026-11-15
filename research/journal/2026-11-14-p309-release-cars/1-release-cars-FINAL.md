# 1 — release cars (FINAL)

Task P309. 2026-11-14.

## What I did

- **The version is 0.15.0.** The brief's 0.12.0 was taken long ago: 0.12.0, 0.13.0, 0.13.1 and 0.14.0
  (P344) shipped while P309 waited. 0.15.0 is the next free version, as the plan's note asked.
- **What was left to release.** Every task the brief waits on (MD37d, P287, MD37e, MD39c, P288) was
  already in v0.14.0's code (`git merge-base --is-ancestor`). So were MD81b/c/d, but those are Untamed
  research and packs that players can't reach. The quad was announced in 0.13.0 and 0.14.0. The five
  MX vs. ATV cars (MD39c) and their Game Modifiers category (P288) were never in any release's notes.
  So 0.15.0's notes lead with the cars.
- **Released even though the cars are invisible and silent.** MD39c's own journal says so: the cars are
  physics only (MD157), there's no Garage row (MD158), and they only work offline. The brief says to
  release once they ride, and they do ride. The user asked to "release sooner", and P284's brief says to
  "release what rides and name what is missing". So I released, and the notes say plainly that this is
  an early look. They tell players how to turn the cars on (`import-atv cars` pack in `mods/machines/`,
  Read the Mods Folder on, `mxvatv_car = buggy` etc. in settings.txt, Switch To Machine) and say that
  drawing, sound and a Garage row come later. I didn't wait for MD157/MD158, because the task was reopened
  so it could ship now.
- **Merged main into task/P309** before releasing, so the release has MD82d (the cars' and quad's own
  chase cameras), MD138a (the quad's clips) and P351 (the macOS .dmg, which makes 11 assets).
- **openmx-mods.** `import-atv cars` (MD39c's 55600e5) was only on branch `openmx-MD39c`, never on
  openmx-mods main. That's the same gap P284 found for the fliers. I cherry-picked it onto origin/main in a
  scratch worktree (clean) and fast-forward pushed it as 954db84. The quads' import-atv (MD37c) was
  already on main as f7448d7. No other openmx-MD*/P* branch has car commits missing from main.
- **Tape and wire.** `git diff v0.14.0..HEAD` doesn't touch replay.rs (FORMAT_VERSION 1), snapshot or
  the net crates, so there's no bump.
- **Notes** (commit cc494b8d9): the cars first, then the Mac app (P351's line), the per-machine chase
  camera (MD82d) and the quad's rider poses and stunt clips (MD138a). The other commits since v0.14.0
  are proofs, captures or gate tidies, with nothing a player sees.

## How it was proved

- **openmx-mods with openmx linked** (this worktree): `cargo test --workspace --no-fail-fast` gives 151
  passed and 2 failed. Both failures are import-music's tests/built.rs, the known MD110, the same two P284
  saw. `DISCX_ISO=~/repos/openmx-mods/iso cargo test -p import-atv --release -- --ignored`: the car pack,
  quad pack and fliers pack disc tests all pass (3/3).
- **The draft.** `OPENMX_RELEASE_BRANCH=task/P309 PATH=~/.cargo/bin:$PATH tools/release.sh 0.15.0`
  built it at b1f987d9 with **11** assets: the 9 from before, openmx-android.apk and
  openmx-macos-arm64.dmg.
- **The binary.** `dist/release/openmx-linux-x86_64 --version` prints `openmx v0.15.0 · b1f987d`.
- **Signature and sums.** `minisign -V` against ~/.config/openmx/release.pub verifies, with the trusted
  comment `openMX v0.15.0`. `sha256sum -c SHA256SUMS` is OK for all 8 files.
- **APK.** `apksigner verify` passes. aapt2 reads io.github.fizl0.openmx, versionName 0.15.0,
  versionCode 1500.
- **IPA.** Info.plist's CFBundleShortVersionString is 0.15.0.
- **DMG.** It ends with the `koly` trailer.
- **Published** with `gh release edit v0.15.0 --draft=false --latest`. releases/latest is v0.15.0.
- **FIZL0/apps.** `gh workflow run source` (run 43678756719) succeeded.
- **Pages.** Run 43678756321 succeeded. https://openmx.dev/version.txt is `v0.15.0 b1f987d`, and
  https://fizl0.github.io/apps/source.json lists 0.15.0 at .../v0.15.0/openmx-ios.ipa.

## Not verified / not 1:1

- **The cars weren't driven by hand for this release.** They're invisible, so there's nothing to look
  at (MD157). The notes rest on MD39c's and P288's shell tests and MD39c's own gaps.
- **The settings.txt way of turning the cars on wasn't tried on a phone or the web.** It was read from
  the code (the `mxvatv_car` line, `ride_mxvatv_car`). Players on a phone can't easily edit settings.txt
  until MD158's Garage row lands.
- **No device install.** The APK, .ipa and .dmg weren't installed on a phone or Mac for this release.
  Only the signature, badging, plist and dmg trailer were checked.
- **openmx-mods' check.sh isn't green on the commit I pushed (954db84)**, because of import-music's two
  red tests (MD110). They were already red before this task, and nothing here caused them.
- **The Mac app wasn't run on a Mac.** P351's own journal covers what it proved.

## Next

Nothing: released.
