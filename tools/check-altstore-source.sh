#!/usr/bin/env bash
# tools/check-altstore-source.sh SOURCE — fails unless every app's versions[] in SOURCE (a file or an https URL; the
# AltStore source tools/ios-source.sh writes) runs newest first (dates never rising, versions falling), names no
# version twice, and its first entry matches the app's legacy top-level version/versionDate/downloadURL/size.
# Screenshots (P391) are optional, but every one listed — `screenshots` (a list, or {iphone, ipad} lists; an entry is a
# URL or {imageURL, width, height}) and the legacy `screenshotURLs` — must be an https URL under https://openmx.dev/
# whose file is in web/, at the width and height given.
# tools/ios-source.sh runs it on what it writes; ci.yml runs it on the latest release's.
set -euo pipefail
[ $# -eq 1 ] || { echo "usage: tools/check-altstore-source.sh SOURCE" >&2; exit 1; }
src=$1 web=$(dirname "$0")/../web
case $src in https://*) json=$(curl -fsSL "$src") ;; *) json=$(<"$src") ;; esac
bad=$(jq -r '
  def key: split("-")[0] | split(".") | map(tonumber);
  .apps[] | . as $app | .name as $n | (.versions // []) as $vs
  | if ($vs | length) == 0 then "\($n): versions[] is empty" else
      ( range(1; $vs | length) as $i | $vs[$i - 1] as $a | $vs[$i] as $b
        | select($a.date < $b.date or ($a.version | key) <= ($b.version | key))
        | "\($n): \($a.version) (\($a.date)) is listed above \($b.version) (\($b.date)): not newest first" ),
      ( $vs | group_by(.version)[] | select(length > 1) | "\($n): \(.[0].version) is listed \(length) times" ),
      ( [["version", "version"], ["versionDate", "date"], ["downloadURL", "downloadURL"], ["size", "size"]][]
        as [$top, $v] | select($app[$top] != $vs[0][$v])
        | "\($n): top-level \($top) \($app[$top]) is not versions[0].\($v) \($vs[0][$v])" )
    end' <<<"$json")
# every screenshot as "app<TAB>url<TAB>width<TAB>height" (width and height empty for a bare URL)
shots=$(jq -r '
  .apps[] | .name as $n
  | ((.screenshots // []) | if type == "object" then (.iphone // []) + (.ipad // []) else . end)[], (.screenshotURLs // [])[]
  | if type == "string" then { imageURL: . } else . end
  | [$n, (.imageURL // ""), (.width // "" | tostring), (.height // "" | tostring)] | @tsv' <<<"$json")
while IFS=$'\t' read -r n url w h; do
    [ -n "$url" ] || continue
    case $url in *..*) bad+=$'\n'"$n: screenshot $url climbs out of web/"; continue ;; esac
    case $url in https://openmx.dev/?*) ;; *) bad+=$'\n'"$n: screenshot $url is not under https://openmx.dev/"; continue ;; esac
    f=$web/${url#https://openmx.dev/}
    [ -f "$f" ] || { bad+=$'\n'"$n: screenshot $url has no file ${f#"$web/"} in web/"; continue; }
    [ -n "$w$h" ] || continue
    # file(1) says "…, 1600x900, …" for a jpg, "…, 1600 x 900, …" for a png
    wh=$(file -b "$f" | grep -oE ', [0-9]+ ?x ?[0-9]+,' | head -1 | tr -d ' ,')
    [ "$wh" = "${w}x$h" ] || bad+=$'\n'"$n: screenshot $url is listed ${w}x$h but its file is ${wh:-of no size}"
done <<<"$shots"
bad=$(sed '/^$/d' <<<"$bad")
[ -z "$bad" ] || { echo "$src:"; echo "$bad"; } >&2
[ -z "$bad" ] || exit 1
echo "$src: $(jq -r '[.apps[] | "\(.name) \(.versions | length) versions, \(.versions[0].version) first, \(
  ((.screenshots // []) | if type == "object" then (.iphone // []) + (.ipad // []) else . end)
  | map(if type == "string" then . else .imageURL end | split("/")[-1]) | if length == 0 then "no screenshots"
    else "screenshots \(join(" "))" end)"] | join("; ")' <<<"$json")"
