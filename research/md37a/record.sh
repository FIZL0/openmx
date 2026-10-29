#!/usr/bin/env bash
# MD37a: one ATV capture. record.sh ATV_ADDR STATE NAME [PAD] [FRAMES]
#   KIT_CONFIG must point at a kit.toml for SLUS-21104 (journal: research/journal/2026-10-29-md37a-atv-captures).
#   Writes context/capture/md37a/NAME.csv (+ .json), holding the emulator for the one job.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
addr=$1 state=$2 name=$3 pad=${4:-} frames=${5:-300}
spec=$(mktemp --suffix=.toml -p "${TMPDIR:-/tmp}")
trap 'rm -f "$spec"' EXIT
sed "s/ATV + /$addr + /; s/^name        = \"atv\"/name        = \"$name\"/" "$here/atv.toml" > "$spec"
mkdir -p "$root/context/capture/md37a"
args=("$spec" --state "$state" --frames "$frames" --out "$root/context/capture/md37a/$name.csv")
[ -n "$pad" ] && args+=(--pad "$here/$pad")
cd "$root"
exec ./kit pcsx2 with -- ./kit record "${args[@]}"
