#!/usr/bin/env bash
# tools/icons.sh — the app and site icons from web/icon.svg (P225). Run it after editing the SVG and commit the output:
#   web/favicon.ico            16/32/48 px, for browsers that take no SVG favicon
#   web/icon-192.png, -512.png the web manifest's icons, rounded corners as the SVG
#   web/icon-180.png, -1024.png full-bleed and opaque (iOS rounds them itself): the apple-touch-icon, the iPhone app's
#                              (tools/ios-ipa.sh) and its App Store / AltStore artwork (tools/ios-source.sh)
set -euo pipefail
cd "$(dirname "$0")/.."
for t in rsvg-convert magick; do command -v $t >/dev/null || { echo "$t missing: sudo pacman -S librsvg imagemagick" >&2; exit 1; }; done
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
for s in 192 512; do rsvg-convert -w $s -h $s web/icon.svg -o web/icon-$s.png; done
sed 's/ rx="[0-9.]*"//' web/icon.svg >"$tmp/square.svg"
for s in 180 1024; do
    rsvg-convert -w $s -h $s "$tmp/square.svg" -o "$tmp/$s.png"
    magick "$tmp/$s.png" -background '#8e0f17' -alpha remove -alpha off -strip "PNG24:web/icon-$s.png"
done
for s in 16 32 48; do rsvg-convert -w $s -h $s web/icon.svg -o "$tmp/f$s.png"; done
magick "$tmp/f16.png" "$tmp/f32.png" "$tmp/f48.png" web/favicon.ico
for f in web/favicon.ico web/icon-{180,192,512,1024}.png; do echo "$f $(magick identify -format '%wx%h ' "$f")"; done
