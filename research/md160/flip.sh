#!/bin/bash
# MD160: flip.py with the VM running and no movie playing.
#   KIT_CONFIG=<SLUS-21701 kit.toml> ./kit pcsx2 with -- bash research/md160/flip.sh PID IN CAR RX RY RZ OUT
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
C6=$ROOT/research/journal/2026-10-20-c6-c6-recording2-capture/playback
PID=$1; shift
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
cd "$ROOT"
python3 -B "$HERE/flip.py" "$@"
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'System/Pause' > /dev/null
