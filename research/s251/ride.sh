#!/usr/bin/env bash
# S251: unpause the copy over AT-SPI (park.py leaves it paused, and a paused copy ignores a load), then
# record SLOT with research/s251/strike.py.
#
# usage: research/s251/ride.sh PID SLOT NAME   (under ./kit pcsx2 with --, KIT_PCSX2 set)
set -euo pipefail
PID=$1 SLOT=$2 NAME=$3
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
python3 -B "$ROOT/research/journal/2026-10-20-c6-c6-recording2-capture/playback/resume_if_paused.py" "$PID"
python3 -B "$ROOT/research/s251/strike.py" "$NAME" "$SLOT"
