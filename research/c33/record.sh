#!/bin/bash
# C33: re-record a kept capture's riders from its kept state (context/capture/c33/STEM.p2s), through
# save-state SLOT, into context/capture/c33/NAME.csv. Run the copy slow (its ini's NominalScalar 0.1)
# so the recorder's settle after the load (spec settle_ms = 100) costs no ticks.
#   ./kit pcsx2 with -- bash research/c33/record.sh STEM SLOT NAME [FRAMES]
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
STEM=$1 SLOT=$2 NAME=$3 N=${4:-1800}
OUT=$ROOT/context/capture/c33
cd "$ROOT"
python3 research/c33/wait_boot.py
python3 -I research/c33/place.py "$OUT/$STEM" "$SLOT"
python3 -I research/c33/spec.py "$OUT/$STEM.riders.json" "$N" > "$OUT/$NAME.toml"
KIT_FRAME_STALL=300 ./kit record "$OUT/$NAME.toml" --state "$SLOT" -n "$N" -o "$OUT/$NAME.csv"
