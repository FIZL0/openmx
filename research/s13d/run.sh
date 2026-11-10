#!/bin/bash
# S13d: boot sx01 as a 4-rider single race of LAPS laps (C31's boot), save its start in SLOT, then
# record the finish with the human's card ended once AFTER AI riders are done (finish.py).
#   ./kit pcsx2 start && ./kit pcsx2 with -- bash research/s13d/run.sh SLOT NAME [LAPS [AFTER [TAIL]]]
# The copy must be freshly booted (boot.py pokes the event before the first world loads).
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
SLOT=$1 NAME=$2 LAPS=${3:-1} AFTER=${4:-1} TAIL=${5:-300}
cd "$ROOT"
mkdir -p context/capture/s13d
python3 research/c31/boot.py "$SLOT" 4 "$LAPS" 0
python3 research/s13d/finish.py "context/capture/s13d/$NAME.jsonl" "$AFTER" "$TAIL"
