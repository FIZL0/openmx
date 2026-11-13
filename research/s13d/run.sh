#!/bin/bash
# S13d: boot sx01 as a 4-rider single race of LAPS laps (C31's boot), save its start in SLOT, then
# record the finish with the human's card ended once AFTER AI riders are done (finish.py).
#   ./kit pcsx2 start && ./kit pcsx2 with -- bash research/s13d/run.sh SLOT NAME [LAPS [AFTER [TAIL]]]
# The copy must be freshly booted (boot.py pokes the event before the first world loads).
# V127: the rows' "flash" field is the camera flashes' object, every tick (crates/game-sim/tests/
# camera_flash.rs). For a look, start the copy with QT_ACCESSIBILITY=1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1
# and press System -> Screenshot over AT-SPI (research/c28/shot.py) while this runs: the race camera
# shows the stands' twinkles in the top band, more after the winner's finish.
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
SLOT=$1 NAME=$2 LAPS=${3:-1} AFTER=${4:-1} TAIL=${5:-300}
cd "$ROOT"
mkdir -p context/capture/s13d
python3 research/c31/boot.py "$SLOT" 4 "$LAPS" 0
python3 research/s13d/finish.py "context/capture/s13d/$NAME.jsonl" "$AFTER" "$TAIL"
