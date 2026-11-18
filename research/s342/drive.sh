#!/bin/bash
# S342: the driven-baja image. From boot.py's parked-baja state in SLOT (S149a:
# `boot.py free07 2 5 SLOT 5`), set the cheat flag, play drive.pad as a from-state tape (play.sh),
# save slot OUT and unpack its RAM to IMAGE. One job, in this slot's copy:
#   export WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
#   ./kit pcsx2 with -- bash research/s342/drive.sh PID SLOT OUT IMAGE SHOT.png
set -e
PID=$1 SLOT=$2 OUT=$3 IMAGE=$4 SHOT=$5
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
python3 -B research/s342/cheat.py "$SLOT" "$OUT"
STATE=$(./kit state info "$OUT" | head -1 | sed 's/\.p2s .*/.p2s/')
GAME=$(python3 -I -c "print(open('context/recordings/first_recording.p2m2','rb').read()[306:561].rstrip(b'\0').decode())")
python3 -I research/c15/tape.py context/recordings/first_recording.p2m2 research/s342/drive.pad "$WORK/drive.p2m2" --game "$GAME"
cp "$STATE" "$WORK/cheat.p2s"     # the slot is saved over below
# the copy runs at NominalScalar 0.3: the tape's 1014 fields take ~56 s
bash research/s342/play.sh "$PID" "$WORK/drive.p2m2" "$WORK/cheat.p2s" 70 "$SHOT"
./kit pine save "$OUT"
sleep 2
./kit state ram "$OUT" --out "$IMAGE"
