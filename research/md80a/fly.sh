#!/bin/bash
# MD80a: play a flight tape from a flier's state (state.sh) and capture it into context/capture/md80a/OUT.csv.
#   KIT_CONFIG=<SLUS-21104 kit.toml> KIT_PCSX2=N bash research/md80a/fly.sh PID NAME KIND PAD OBJ [OUT]
# KIND is biplane or kiowa (the spec and the vtable), PAD the pad script (research/md80a/*.pad), OBJ the
# player's flier object (hex, ram.py's moving one). The tape goes to context/recordings/md80a/PADNAME.p2m2.
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
PID=$1 NAME=$2 KIND=$3 PAD=$4 OBJ=$5 OUT=${6:-$2}
case $KIND in biplane) VPTR=0x3ce7b0 ;; kiowa) VPTR=0x3cfad0 ;; *) echo "KIND is biplane or kiowa"; exit 2 ;; esac
cd "$ROOT"
mkdir -p context/recordings/md80a
TAPE=context/recordings/md80a/$(basename "$PAD" .pad).p2m2
python3 -I research/c15/tape.py context/recordings/animandpump.p2m2 "$PAD" "$TAPE" > /dev/null
TICKS=$(($(wc -l < "$TAPE.ticks.csv") - 1))
FIRST=$(python3 -I "$HERE/ram.py" "$NAME" | awk '/^vsync/ {print $2}')
timeout 1500 ./kit pcsx2 with -- bash "$HERE/play.sh" "$PID" "$HERE/$KIND.toml" "$TAPE" \
    "context/mxvatv-ps2/md80a/$NAME.p2s" "$OBJ" $((VPTR)) "$FIRST" $((TICKS - 5)) "context/capture/md80a/$OUT.csv"
