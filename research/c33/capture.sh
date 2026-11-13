#!/bin/bash
# C33: boot free09 with four racers, save the start in SLOT, keep it beside the capture, find the
# riders in it, and record their bodies + rand_state per tick from that state into
# context/capture/c33/NAME.csv (+ .json, .toml, .p2s, .ee.bin, .riders.json).
#   export WAYLAND_DISPLAY=wayland-1; ./kit pcsx2 start
#   ./kit pcsx2 with -- bash research/c33/capture.sh SLOT NAME [FRAMES]
# The copy must be freshly booted (boot.py pokes the event before the first world loads).
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
SLOT=$1 NAME=$2 N=${3:-1800}
OUT=$ROOT/context/capture/c33
mkdir -p "$OUT"
cd "$ROOT"
python3 research/c33/boot.py "$SLOT"
python3 -I research/c33/keep.py "$SLOT" "$OUT/$NAME"
python3 -I research/c33/riders.py "$OUT/$NAME.ee.bin"
python3 -I research/c33/riders.py "$OUT/$NAME.ee.bin" --json > "$OUT/$NAME.riders.json"
python3 -I research/c33/spec.py "$OUT/$NAME.riders.json" "$N" > "$OUT/$NAME.toml"
KIT_FRAME_STALL=300 ./kit record "$OUT/$NAME.toml" --state "$SLOT" -n "$N" -o "$OUT/$NAME.csv"
