# 1 — release 0.1.0 (FINAL)

Task P195. 2026-10-29.

## What I did

- RELEASE_NOTES.md: a short first-release "What's new" for players: what plays (the bike, rider, tricks, wrecks,
  the biplane and the other Switch To Machine machines), the browser (phones with touch), Linux/Steam Deck, the
  desktop updater, split screen and online, mods and the mod store, the iPhone source (kept from P201), and that
  you bring your own MX Unleashed (USA) disc. I left out MX vs. ATV track imports on purpose: the importer is in
  openmx-mods, not in the shipped game.
- `tools/release.sh 0.1.0 RELEASE_NOTES.md publish`, run from this task branch. The session rules say never to
  push main, and the script pushed `HEAD:main` twice (the release commit, then the emptied notes). So the script
  now pushes to `OPENMX_RELEASE_BRANCH`, which defaults to main. I ran it with `OPENMX_RELEASE_BRANCH=task/P195`.
  The release's target is 9883786 ("Release v0.1.0": the workspace version 1.0.0 → 0.1.0), which is on
  task/P195 and reaches main when the runner merges the branch. The script also made "Release notes: emptied
  after v0.1.0" on the branch.
- The release notes' verify line was wrong: `minisign -Vm SHA256SUMS -P …` looks for `SHA256SUMS.minisig`, but
  the release ships `SHA256SUMS.sig`. I added `-x SHA256SUMS.sig` in release.sh and edited v0.1.0's published
  body the same way (`gh release edit --notes-file`). It is still Latest.
- `gh repo edit FIZL0/openmx --homepage https://openmx.dev/` (P194's step (d)). plan/HUMAN.md: steps 4 (the first
  release) and 6(d) (the homepage) now say they are done.
- Taken from opentennis: nothing new. release.sh was already its port (P191).

## What the original does

Not applicable: this is a release, not a port.

## How it was proved

- `gh api repos/FIZL0/openmx/releases/latest` → v0.1.0. It is not a draft and not a pre-release, and its assets
  are openmx-linux-x86_64(.tar.gz), web.tar.gz, SHA256SUMS, SHA256SUMS.sig, openmx-ios.ipa and
  altstore-source.json.
- pages.yml's release run 40948784344 passed (`gh run watch --exit-status`). https://openmx.dev/ answers with:
  - version.txt = `v0.1.0 9883786`;
  - the canonical `https://openmx.dev/` and the OG title/image;
  - robots.txt, sitemap.xml and shots/ride-free06.jpg, all 200;
  - the noindex script, which only acts on other hostnames.
- The site's bytes: I unpacked the release's web.tar.gz and compared it with what openmx.dev serves. The sha256
  of index.html, worker.js, version.txt, pkg/openmx_web_bg.wasm and pkg/openmx_web.js are the same. I then served
  that unpacked copy on loopback and drove it with the repo's own CDP drivers and the real disc:
  - `tests/web/gpu.mjs` (the disc picked, then `--ride`): WebGPU at a median of ~58 fps and WebGL2 at ~51 fps,
    738k of 738k pixels lit, and a screenshot showing the rider, HUD and minimap at the start line. A phone viewport
    (844×390@3) draws 2532×1170 on WebGPU at ~50 fps.
  - `tests/web/touch.mjs`: the touch pad shows, steers and gasses, the page does not scroll, and portrait shows the
    "turn your phone" hint.
- The desktop: I downloaded the release's assets into a scratch folder.
  - `minisign -Vm SHA256SUMS -x SHA256SUMS.sig -P <update.rs PUBLIC_KEY>` verifies, and `sha256sum -c` gives OK
    for all three files.
  - The tarball unpacks to openmx and README.txt, and its highest GLIBC symbol is 2.30 (the Steam Runtime build).
  - `./openmx disc.iso` with a scratch XDG_DATA_HOME starts (gpu: Radeon, Vulkan) and remembers the disc.
  - `./openmx --screen main` shows the main menu with the corner `v0.1.0 · 9883786`, and logs
    `update check: v0.1.0 is the latest` against the real GitHub API.
- beta.openmx.dev still serves a beta: version.txt = `3cbd242`, with noindex and canonical → openmx.dev.

## Not verified / not 1:1

- **The live URL in a browser.** I did not open https://openmx.dev itself in chromium. I drove the release's
  web.tar.gz on loopback, after checking its key files are byte-identical to what the site serves. The drivers
  serve a local folder and patch `opfs = false` so a 4 GB disc is not copied into OPFS, which the real host can't
  do.
- **Real phones and iOS Safari.** I only emulated a phone's size and touch in headless chromium.
- **The format of version.txt.** It is `v0.1.0 9883786` (tools/web.sh writes OPENMX_VERSION, which is the tag),
  not the brief's `0.1.0 <commit>`. I left it alone: the page only compares the file with itself, and the beta's
  is a bare commit.
- **beta.openmx.dev's age.** It serves 3cbd242, while the beta release is at 7691f60. It catches up on
  openmx-beta's own half-hourly check, and this task did not touch it.
- **The updater installing a newer release.** Nothing newer than v0.1.0 exists, so only "no newer version" was
  checked live. The install path is P191's local-server test.
- **The iPhone app and the FIZL0/apps source rebuild.** The release made the .ipa and asked for the rebuild
  (workflow run 40948783629), but I did not install it on a device or check that run.
- **The workspace version.** It went from 1.0.0 to 0.1.0 (the release commit). Beta builds after the merge say
  0.1.0.

## Next

Nothing for this task. Search Console, Bing and link previews (HUMAN.md step 6 a–c) are the human's.
