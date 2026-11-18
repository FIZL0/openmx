#!/usr/bin/env bash
# tools/check-altstore-source.sh SOURCE — fails unless every app's versions[] in SOURCE (a file or an https URL; the
# AltStore source tools/ios-source.sh writes) runs newest first (dates never rising, versions falling), names no
# version twice, and its first entry matches the app's legacy top-level version/versionDate/downloadURL/size.
# tools/ios-source.sh runs it on what it writes; ci.yml runs it on the latest release's.
set -euo pipefail
[ $# -eq 1 ] || { echo "usage: tools/check-altstore-source.sh SOURCE" >&2; exit 1; }
src=$1
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
[ -z "$bad" ] || { echo "$src:"; echo "$bad"; } >&2
[ -z "$bad" ] || exit 1
echo "$src: $(jq -r '[.apps[] | "\(.name) \(.versions | length) versions, \(.versions[0].version) first"] | join("; ")' <<<"$json")"
