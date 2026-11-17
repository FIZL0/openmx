#!/bin/bash
# R25: make a tape from C14's state and capture it with watch/r25-kiowa-blades.toml, then scan it.
# usage: ./kit pcsx2 with -- research/r25/tape.sh PID NAME ROWS SEGMENT...   (make_tape.py's segments)
set -e
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
PID=$1 NAME=$2 ROWS=$3; shift 3
REC=$ROOT/context/recordings/r25 CAP=$ROOT/context/capture/r25
mkdir -p "$REC" "$CAP"
python3 -B "$ROOT/research/c14/make_tape.py" "$ROOT/context/recordings/c14/flight.p2m2" "$REC/$NAME.p2m2" "$@"
cp "$ROOT/context/recordings/c14/flight.p2m2_SaveState.p2s" "$REC/$NAME.p2m2_SaveState.p2s"
echo "$NAME $ROWS $*" >> "$REC/tapes.txt"
"$ROOT/research/c19/capture.sh" "$PID" "$REC/$NAME.p2m2" "$ROWS" "$CAP/$NAME.csv" "$ROOT/watch/r25-kiowa-blades.toml"
python3 -B "$ROOT/research/r25/scan.py" "$NAME"
