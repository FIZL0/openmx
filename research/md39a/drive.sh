#!/bin/bash
# MD39a: play drive.pad from a car's state (state.sh) and capture it into context/capture/md39a/OUT.csv.
#   KIT_CONFIG=<SLUS-21104 kit.toml> KIT_PCSX2=N bash research/md39a/drive.sh PID NAME OBJ VPTR [OUT]
# OBJ is the player's car object (hex, ram.py's moving one), VPTR its vtable (hex). The tape goes to
# context/recordings/md39a/drive.p2m2 (+ .ticks.csv).
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
PID=$1 NAME=$2 OBJ=$3 VPTR=$4 OUT=${5:-$2}
cd "$ROOT"
mkdir -p context/recordings/md39a
TAPE=context/recordings/md39a/drive.p2m2
python3 -I research/c15/tape.py context/recordings/animandpump.p2m2 "$HERE/drive.pad" "$TAPE" > /dev/null
TICKS=$(($(wc -l < "$TAPE.ticks.csv") - 1))
FIRST=$(python3 -I "$HERE/ram.py" "$NAME" | awk '/^vsync/ {print $2}')
timeout 900 ./kit pcsx2 with -- bash "$HERE/play.sh" "$PID" "$TAPE" "context/mxvatv-ps2/md39a/$NAME.p2s" \
    "$OBJ" $((0x$VPTR)) "$FIRST" $((TICKS - 5)) "context/capture/md39a/$OUT.csv"
