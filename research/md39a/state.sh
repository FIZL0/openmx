#!/bin/bash
# MD39a: from the Desert Oasis Player 1 Options state (menu.pad's end, context/mxvatv-ps2/md39a/options.p2s)
# to a car at the Free Ride spawn, at rest: Vehicle right xK (4 buggy, 5 monster truck, 6 trophy truck =
# baja, 7 golf cart, 8 sand rail), up to Start, Cross, then the load. Saves SLOT, copies it to
# context/mxvatv-ps2/md39a/NAME.p2s and lists the cars in its RAM (ram.py).
#   KIT_CONFIG=<SLUS-21104 kit.toml> KIT_PCSX2=N bash research/md39a/state.sh PID NAME K SLOT
# Menu presses need ~90 ticks between them: one in a preview's load is dropped.
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
PID=$1 NAME=$2 K=$3 SLOT=$4
DIR=$ROOT/context/mxvatv-ps2/md39a
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
q() { printf 'down %s\nsleep 6t\nup %s\nsleep 84t\n' "$1" "$1"; }
{ echo 'sleep 30t'; for _ in $(seq "$K"); do q right; done; echo 'sleep 60t'; q up; q cross; echo 'sleep 1500t'; } > "$WORK/car.pad"
touch "$WORK/mark"
bash "$HERE/step.sh" "$PID" "$WORK/car.pad" "$DIR/options.p2s" "$SLOT"
saved="$(dirname "$("$ROOT/kit" pcsx2 status | awk '/^config/ {print $2}')/x")/savestates"
F="$saved/SLUS-21104 (95BB1901).0$SLOT.p2s"
for _ in $(seq 60); do [ "$F" -nt "$WORK/mark" ] && break; sleep 1; done   # the save is written after `pine save` returns
sleep 3
cp "$F" "$DIR/$NAME.p2s"
python3 -I "$HERE/ram.py" "$NAME"
rm -r "$WORK"
