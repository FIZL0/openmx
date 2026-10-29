#!/usr/bin/env bash
# S179: S117's steep traverse with the gas and the stick held from before the load, so the ride is the
# same on every run (the state's own brake drives tick 1, the pad every tick after), recorded with
# states saved on chosen frames (S109's record_saving.py).
#
#   ./kit pcsx2 with -- bash research/journal/2026-10-29-s179-steep-face-takeoff/capture/held.sh NAME SLOT ROWS [FRAME:SLOT,...]
#
# SLOT is the teleported state (S106's tele.py, S117's README). The capture goes to
# context/capture/s179/NAME.csv, each saved state beside it as NAME-<frame>.p2s.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
name=$1
slot=$2
rows=$3
save=${4:-}
out=context/capture/s179
mkdir -p "$out"
spec=$out/$name.spec.toml
sed "s/^state   = 8 .*/state   = $slot/" watch/s58-slip.toml > "$spec"
grep -q "^state   = $slot\$" "$spec"
./kit pad send release
./kit pad send "down cross"
./kit pad send "stick l -0.3 0"
KIT_FRAME_STALL=300 python3 -B "$here/../../2026-10-24-s67-landing-snap-2/capture/record_saving.py" \
    "$spec" "$out/$name.csv" "$rows" --save "$save"
./kit pad send release
