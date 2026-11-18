#!/usr/bin/env bash
# tools/ios-source.sh VERSION IPA [OUT] [NOTES] — the iPhone app as a sideloading source (P201, as opentennis's 6e0730cb):
# the AltStore v2 format, which AltStore, SideStore and Feather all read, pointing at release vVERSION's copy of IPA.
# OUT defaults to altstore-source.json next to IPA. tools/release.sh uploads it with every release, so
# https://github.com/FIZL0/openmx/releases/latest/download/altstore-source.json always serves the latest release's;
# FIZL0's shared source (github.com/FIZL0/apps, https://fizl0.github.io/apps/source.json) merges it, and that is the one
# web/ios.html, the README and the release notes point players at.
# Its icon is the App Store size web/icon-1024.png (P225, tools/icons.sh), as openmx.dev serves it.
# versions[] is every published release with an IPA, newest first, so signers show the history and can roll back: the
# GitHub API's (drafts and pre-releases such as the rolling beta left out; GH_TOKEN / GITHUB_TOKEN in CI), under
# VERSION from the local IPA (not published yet while the release builds; it replaces the API's entry if it is). Each
# entry's localizedDescription is its release's "New in" list; NOTES (Markdown bullets) gives VERSION's. A failed API
# call fails the script, and so does a source with one version: a release must never cut the history.
# The app also carries the legacy top-level version/versionDate/downloadURL/size fields, versions[0]'s, for older
# signers that predate `versions`. tools/check-altstore-source.sh checks the result.
set -euo pipefail
[ $# -ge 2 ] || { echo "usage: tools/ios-source.sh VERSION IPA [OUT] [NOTES]" >&2; exit 1; }
version=${1#v} ipa=$2 out=${3:-$(dirname "$2")/altstore-source.json} notes=${4:-}
[ -f "$ipa" ] || { echo "no such IPA: $ipa" >&2; exit 1; }
command -v jq >/dev/null || { echo "jq missing: sudo pacman -S jq" >&2; exit 1; }
repo=FIZL0/openmx
blurb="MX Unleashed (PS2, 2004) rewritten: the original's physics, tricks and timing, on today's screens"
desc=
[ -z "$notes" ] || desc=$(sed '/^<!--/d' "$notes")
# one compact entry per line; a failed call ends the script (set -e covers the assignment)
published=$(GH_TOKEN=${GH_TOKEN:-${GITHUB_TOKEN:-}} gh api --paginate "repos/$repo/releases?per_page=100" --jq '
  .[] | select((.draft | not) and (.prerelease | not))
  | ([.assets[] | select(.name | endswith(".ipa"))][0]) as $a | select($a != null)
  | { at: .published_at, version: (.tag_name | ltrimstr("v")), date: (.published_at[:10]), downloadURL: $a.browser_download_url,
      size: $a.size, minOSVersion: "15.0" }
    + ((.body // "") | gsub("\r"; "") | (capture("(?:^|\n)## New in [^\n]*\n(?<n>[\\s\\S]*?)(?:\n## |$)").n // "")
       | gsub("^\\s+|\\s+$"; "") | if . == "" then {} else { localizedDescription: . } end)') \
    || { echo "listing $repo's releases failed: no source without its history" >&2; exit 1; }
jq -n --arg v "$version" --arg date "$(date -u +%Y-%m-%d)" \
    --arg ipa "https://github.com/$repo/releases/download/v$version/openmx-ios.ipa" \
    --argjson size "$(stat -c %s "$ipa")" --arg desc "$desc" --argjson published "[${published//$'\n'/,}]" \
    --arg src "https://github.com/$repo/releases/latest/download/altstore-source.json" --arg blurb "$blurb" '
  ([$published[] | select(.version == $v)][0] // {}) as $was
  | [$was + { version: $v, date: $date, downloadURL: $ipa, size: $size, minOSVersion: "15.0" }
          + (($desc | gsub("^\\s+|\\s+$"; "")) as $d | if $d == "" then {} else { localizedDescription: $d } end)]
    + ($published | map(select(.version != $v)) | sort_by(.at) | reverse) | map(del(.at)) as $versions
  | if ($versions | length) < 2 then error("only \($v) has an IPA: refusing a source without the older releases") else . end
  | { name: "openMX", identifier: "io.github.fizl0.openmx", sourceURL: $src, website: "https://openmx.dev/",
    iconURL: "https://openmx.dev/icon-1024.png", tintColor: "#8e0f17",
    subtitle: "A remaster of MX Unleashed (PS2)",
    apps: [{ name: "openMX", bundleIdentifier: "io.github.fizl0.openmx", developerName: "FIZL0",
             subtitle: "A remaster of MX Unleashed (PS2)",
             localizedDescription: ($blurb + ". Plays from your own disc image (USA, SLUS-20767), no game data included: put the ISO in Files → On My iPhone → openMX. Touch controls or a controller."),
             iconURL: "https://openmx.dev/icon-1024.png", tintColor: "#8e0f17", category: "games",
             version: $versions[0].version, versionDate: $versions[0].date, downloadURL: $versions[0].downloadURL,
             size: $versions[0].size, versions: $versions }],
    news: [] }' >"$out"
"$(dirname "$0")/check-altstore-source.sh" "$out" >/dev/null
echo "$out"
