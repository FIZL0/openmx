#!/bin/bash
# R63: one step through the front end by tape. Build a two-pad tape (research/r22/tape2.py) from
# P1.pad and P2.pad, play it from STATE (a .p2s, or a slot number of this copy), keep the VM running
# SECS seconds past the tape's start, then save to SLOT and write the saved state's own Screenshot.png
# to PNG (the desktop has no window grab here).
#
#   export WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
#   ./kit pcsx2 with -- bash research/r63/step.sh STATE P1.pad P2.pad SECS SLOT PNG
set -e
C6=research/journal/2026-10-20-c6-c6-recording2-capture/playback
STATE=$1 P1=$2 P2=$3 SECS=$4 SLOT=$5 PNG=$6
DIR=$(sed -n 's/^Savestates *= *//p' "$(dirname "$(./kit pcsx2 | sed -n 's/^config *//p')")"/PCSX2/inis/PCSX2.ini)
slot() { printf '%s/SLUS-20767 (9866FE54).%02d.p2s' "$DIR" "$1"; }
case $STATE in *[!0-9]*) SRC=$STATE ;; *) SRC=$(slot "$STATE") ;; esac
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
python3 -I research/r22/tape2.py context/recordings/split/splitscreen2_shortcolision.p2m2 "$P1" "$P2" "$WORK/play.p2m2" > /dev/null
cp "$SRC" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
PID=$(./kit pcsx2 | sed -n 's/.*(pid \([0-9]*\)).*/\1/p')
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null 2>&1 || true
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'System/Reset' > /dev/null
sleep 2
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2" > /dev/null
sleep 4
end=$(( $(date +%s) + SECS ))
while [ "$(date +%s)" -lt "$end" ]; do
    python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null || true
    sleep 2
done
OUT=$(slot "$SLOT")
rm -f "$OUT"
./kit pine save "$SLOT" > /dev/null
for _ in $(seq 60); do [ -s "$OUT" ] && break; sleep 1; done
sleep 2
python3 -I -c 'import sys, zipfile; open(sys.argv[2], "wb").write(zipfile.ZipFile(sys.argv[1]).read("Screenshot.png"))' "$OUT" "$PNG"
./kit pine | grep frame
