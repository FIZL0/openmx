#!/usr/bin/env bash
# tools/ios-source.sh VERSION IPA [OUT] — the iPhone app as a sideloading source (P201, as opentennis's 6e0730cb): the
# AltStore v2 format, which AltStore, SideStore and Feather all read, pointing at release vVERSION's copy of IPA.
# OUT defaults to altstore-source.json next to IPA. tools/release.sh uploads it with every release, so
# https://github.com/FIZL0/openmx/releases/latest/download/altstore-source.json always serves the latest release's;
# FIZL0's shared source (github.com/FIZL0/apps, https://fizl0.github.io/apps/source.json) merges it, and that is the one
# web/ios.html, the README and the release notes point players at.
# Its icon is the App Store size web/icon-1024.png (P225, tools/icons.sh), as openmx.dev serves it.
# The app also carries the legacy top-level version/downloadURL/size fields, for older signers that predate `versions`.
set -euo pipefail
[ $# -ge 2 ] || { echo "usage: tools/ios-source.sh VERSION IPA [OUT]" >&2; exit 1; }
version=${1#v} ipa=$2 out=${3:-$(dirname "$2")/altstore-source.json}
[ -f "$ipa" ] || { echo "no such IPA: $ipa" >&2; exit 1; }
command -v jq >/dev/null || { echo "jq missing: sudo pacman -S jq" >&2; exit 1; }
repo=FIZL0/openmx
blurb="MX Unleashed (PS2, 2004) rewritten: the original's physics, tricks and timing, on today's screens"
jq -n --arg v "$version" --arg date "$(date -u +%Y-%m-%d)" \
    --arg ipa "https://github.com/$repo/releases/download/v$version/openmx-ios.ipa" \
    --argjson size "$(stat -c %s "$ipa")" \
    --arg src "https://github.com/$repo/releases/latest/download/altstore-source.json" --arg blurb "$blurb" '
  { name: "openMX", identifier: "io.github.fizl0.openmx", sourceURL: $src, website: "https://openmx.dev/",
    iconURL: "https://openmx.dev/icon-1024.png", tintColor: "#8e0f17",
    subtitle: "A remaster of MX Unleashed (PS2)",
    apps: [{ name: "openMX", bundleIdentifier: "io.github.fizl0.openmx", developerName: "FIZL0",
             subtitle: "A remaster of MX Unleashed (PS2)",
             localizedDescription: ($blurb + ". Plays from your own disc image (USA, SLUS-20767), no game data included: put the ISO in Files → On My iPhone → openMX. Touch controls or a controller."),
             iconURL: "https://openmx.dev/icon-1024.png", tintColor: "#8e0f17", category: "games",
             version: $v, versionDate: $date, downloadURL: $ipa, size: $size,
             versions: [{ version: $v, date: $date, downloadURL: $ipa, size: $size, minOSVersion: "15.0" }] }],
    news: [] }' >"$out"
echo "$out"
