# 1 — dirt bike icon (FINAL)

Task P225. 2026-10-29.

## What I did

- `web/icon.svg`: the yellow "MX" mark replaced with a rider in the attack stance on a dirt bike (high front fender,
  up-swept tail, knobby-thick rims), cream `#fff3dc` on the menus' maroon `#8e0f17`, 64-unit grid, strokes ≥ 3 units so
  each is ≥ ¾ px at 16 px.
- `tools/icons.sh` (new): renders everything from the SVG with rsvg-convert + ImageMagick, so the next redraw is one
  command: `web/favicon.ico` (16/32/48), `web/icon-192.png` and `-512.png` (rounded, as the SVG; the manifest's), and
  `web/icon-180.png` / `-1024.png` full-bleed and opaque (iOS masks its own corners and wants no alpha).
- `web/manifest.webmanifest` lists the 192/512 PNGs beside the SVG; `web/index.html` and `web/ios.html` add the
  favicon.ico fallback (apple-touch-icon was already icon-180.png, now the new art).
- `tools/ios-ipa.sh`: the 1024 px artwork goes in the IPA as `iTunesArtwork@2x` beside `Payload/`.
- `tools/ios-source.sh`: the AltStore source's `iconURL`s (source and app) point at `https://openmx.dev/icon-1024.png`.
- opentennis: nothing to take — its icon work is its own art.

## What the original does

Nothing: the icon is ours (front-end and presentation are ours to design).

## How it was proved

- Sizes: `tools/icons.sh` prints them from `magick identify` — favicon.ico 16x16 32x32 48x48, icon-180 180x180,
  icon-192 192x192, icon-512 512x512, icon-1024 1024x1024; 180/1024 have no alpha channel (PNG24).
- Looked at: the 16 px and 32 px renders blown up 4× with point filtering, beside the 180 px one. At 16 px the two
  wheels, the raised fender/tail line and the rider's helmet read as a dirt bike; the rider's limbs merge into one blob
  there, which is fine for a favicon.
- `tools/ios-source.sh 0.1.0 <dummy.ipa>` writes both iconURLs as icon-1024.png; `bash -n` on both scripts; the manifest
  parses as JSON.

## Not verified / not 1:1

- Not seen on a real iPhone home screen, in AltStore, or as a browser tab/installed PWA: no device or GUI browser
  session here; checked only as rendered PNGs.
- `iTunesArtwork@2x` in the IPA is the legacy (pre-asset-catalog) place for the 1024 px artwork; no `Assets.car` is built
  (actool needs Xcode). Whether a given sideloader shows it was not tested; the AltStore source's iconURL covers AltStore.
- The AltStore iconURL (openmx.dev/icon-1024.png) only resolves once a release with this commit deploys openmx.dev;
  the old icon-180.png URL keeps working meanwhile.
- No iPad-specific icon sizes (76/83.5 pt) and no `maskable` manifest icon (the silhouette fills ~90% of the square,
  past the maskable safe zone); not asked for, iPad and Android scale what is there.

## Next

none
