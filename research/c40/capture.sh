#!/bin/bash
# C40: boot TRACK as an AI-ridden single race, save its start in SLOT, record the drop gate's sink
# into context/capture/c40/NAME.csv (+ .json), then save the end in SLOT+1.
#   ./kit pcsx2 start && ./kit pcsx2 with -- bash research/c40/capture.sh SLOT NAME TRACK LEAGUE [FRAMES] [RACERS] [LAPS]
# The copy must be freshly booted (boot.py pokes the event before the first world loads).
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
SLOT=$1 NAME=$2 TRACK=$3 LEAGUE=$4 FRAMES=${5:-6000} RACERS=${6:-4} LAPS=${7:-2}
cd "$ROOT"
mkdir -p context/capture/c40
python3 -I research/c40/spec.py "$RACERS" "$FRAMES" > "context/capture/c40/$NAME.toml"
python3 research/c40/boot.py "$SLOT" "$RACERS" "$LAPS" 1 "$TRACK" "$LEAGUE"
KIT_FRAME_STALL=300 ./kit record "context/capture/c40/$NAME.toml" --out "context/capture/c40/$NAME.csv"
python3 -c "
import sys; sys.path.insert(0, '.')
from ps2kit.pine import Pine
p = Pine(check=False, lock=False, timeout=30.0); p.save_state($SLOT + 1); print('saved', $SLOT + 1, 'frame', p.frame())"
