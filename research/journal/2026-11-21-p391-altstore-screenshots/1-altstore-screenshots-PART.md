# 1 — altstore screenshots (PART)

Task P391. 2026-11-21.

## What I did

- `tools/ios-source.sh` adds six of the site's shots to the app entry, a ride first: ride-free06, air-cancan,
  road-free09, wreck, split-4p, menu-main. Both forms: `screenshots: {iphone: [{imageURL, width, height}]}` and the
  legacy `screenshotURLs`. Sizes are read from each jpg with file(1); all are 1600x900 (landscape).
- Same script: a version already published keeps its own date when the source is rebuilt (was: today's). Needed to
  rebuild v0.18.0's source without moving its versionDate to 11-21; the release path is unchanged (its version is a
  draft, so not in the API list yet).
- `tools/check-altstore-source.sh` accepts screenshots (optional, so older sources still pass) and fails on any URL
  not under https://openmx.dev/, any file missing from web/, or a width/height that differs from the file's. Its
  summary line lists the screenshots.
- Published: rebuilt v0.18.0's source from its real IPA, checked it is identical to the published one apart from the
  screenshots, uploaded it to the v0.18.0 release (`gh release upload --clobber`), ran FIZL0/apps' `source` workflow
  (run 44937965314, green). https://fizl0.github.io/apps/source.json now carries the six shots.
- Slip: the first upload went up as `new-source.json` (gh names assets by file name, `#` only sets a label). Deleted
  that asset at once; the release's assets are as before plus the new source.

## What the original does

Not applicable: release tooling.

## How it was proved

- Fresh source: checker passes, lists 26 versions and the six shots. Published v0.18.0 source (no shots) still passes.
- A broken copy (wrong width, http URL, missing file, other host) fails with one line each.
- All six URLs return 200 on openmx.dev; the live shared source JSON carries them.

## Not verified / not 1:1

- The app page in AltStore/SideStore itself: no iPhone here. Only the source JSON at its URL was checked.
- Landscape shots in AltStore's iphone list: the format allows any size, but how AltStore lays out 16:9 shots was
  not seen.

## Next

Nothing.
