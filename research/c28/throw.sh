#!/bin/bash
# C28: a mid-air ATV state (MD37a step 5) with no uinput pad: a held-gas tape plays from STATE.p2s while
# throw.py throws the ATV and saves slot OUT.
#   KIT_CONFIG=<SLUS-21104 kit.toml> ./kit pcsx2 with -- bash research/c28/throw.sh PID STATE.p2s ATV SPEED UP OUT
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
C6=$(cd "$HERE/../journal/2026-10-20-c6-c6-recording2-capture/playback" && pwd)
PID=$1 STATE=$2 ATV=$3 SPEED=$4 UP=$5 OUT=$6
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
printf 'down cross\nsleep 30000\n' > "$WORK/gas.pad"
python3 -I "$ROOT/research/c15/tape.py" "$ROOT/context/recordings/c28/template.p2m2" "$WORK/gas.pad" "$WORK/play.p2m2" > /dev/null
cp "$STATE" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2" > /dev/null
sleep 3
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
cd "$ROOT"
python3 -B "$HERE/throw.py" "$ATV" "$SPEED" "$UP" "$OUT" || status=$?
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
exit ${status:-0}
