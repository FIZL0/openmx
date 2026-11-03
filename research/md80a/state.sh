#!/bin/bash
# MD80a: from the Desert Oasis Player 1 Options state (context/mxvatv-ps2/md80a/options.p2s, MD39a's menu.pad
# end) to a flier at the Free Ride spawn: Vehicle right xK (2 plane, 3 helicopter); for M > 0, Customize
# Vehicle (down x2, Cross), Model right xM, Accept, back up x3 to Start Event; else up; Cross, the load.
# Saves SLOT, copies it to context/mxvatv-ps2/md80a/NAME.p2s and lists the fliers in its RAM (ram.py).
#   KIT_CONFIG=<SLUS-21104 kit.toml> KIT_PCSX2=N bash research/md80a/state.sh PID NAME K M SLOT
# MD39a's state.sh with the model pick. Menu presses need ~90 ticks between them.
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
PID=$1 NAME=$2 K=$3 M=$4 SLOT=$5
DIR=$ROOT/context/mxvatv-ps2/md80a
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
q() { printf 'down %s\nsleep 6t\nup %s\nsleep 84t\n' "$1" "$1"; }
{
    echo 'sleep 30t'
    for _ in $(seq "$K"); do q right; done
    if [ "$M" -gt 0 ]; then
        q down; q down; q cross; echo 'sleep 120t'
        q down; for _ in $(seq "$M"); do q right; done; echo 'sleep 100t'
        q up; q cross; echo 'sleep 150t'
        q up; q up; q up
    else
        echo 'sleep 60t'; q up
    fi
    q cross; echo 'sleep 1500t'
} > "$WORK/flier.pad"
touch "$WORK/mark"
bash "$ROOT/research/md39a/step.sh" "$PID" "$WORK/flier.pad" "$DIR/options.p2s" "$SLOT"
saved="$(dirname "$("$ROOT/kit" pcsx2 status | awk '/^config/ {print $2}')/x")/savestates"
F="$saved/SLUS-21104 (95BB1901).0$SLOT.p2s"
for _ in $(seq 60); do [ "$F" -nt "$WORK/mark" ] && break; sleep 1; done   # the save is written after `pine save` returns
sleep 3
cp "$F" "$DIR/$NAME.p2s"
python3 -I "$HERE/ram.py" "$NAME"
rm -r "$WORK"
