#!/bin/bash
# C31: boot sx01 as an AI-ridden single race, save its start in SLOT, and record every score card per
# tick into context/capture/c31/NAME.csv (+ .json).
#   ./kit pcsx2 start && ./kit pcsx2 with -- bash research/c31/capture.sh SLOT NAME [RACERS] [LAPS] [ALL_AI]
# The copy must be freshly booted (boot.py pokes the event before the first world loads).
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
SLOT=$1 NAME=$2 RACERS=${3:-4} LAPS=${4:-3} ALL_AI=${5:-1}
cd "$ROOT"
python3 -I research/c31/spec.py "$RACERS" > "$ROOT/context/capture/c31/$NAME.toml"
python3 research/c31/boot.py "$SLOT" "$RACERS" "$LAPS" "$ALL_AI"
KIT_FRAME_STALL=300 ./kit record "$ROOT/context/capture/c31/$NAME.toml" --out "$ROOT/context/capture/c31/$NAME.csv"
