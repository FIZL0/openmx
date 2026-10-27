#!/bin/bash
# Play biplane.p2m2 back on the original and capture touchdown.toml beside it — P164's
# play_with_video.sh without the film. Run from the repo root, as one job, the copy started with the
# accessibility env (WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1):
#
#   ./kit pcsx2 with -- bash research/journal/2026-10-27-p146-plane-touchdown-sound/capture/play.sh \
#       PID context/recordings/biplane.p2m2 1718 context/capture/p146/touchdown.csv
set -e
HERE=$PWD/research/journal/2026-10-20-c6-c6-recording2-capture/playback
PID=$1 REC=$2 N=$3 OUT=$4
WORK=$(mktemp -d)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
cp $(dirname "$0")/touchdown.toml "$WORK/spec.toml"
mkdir -p "$(dirname "$OUT")"

python3 "$HERE/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$HERE/resume_if_paused.py" "$PID"
python3 "$HERE/press.py" "$PID" 'System/Reset'
read -t 2 < /dev/zero || true
KIT_FRAME_STALL=120 ./kit record "$WORK/spec.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
rec=$!
read -t 2 < /dev/zero || true
python3 "$HERE/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 "$HERE/open_only.py" "$PID" "$WORK/play.p2m2"
read -t 4 < /dev/zero || true
python3 "$HERE/resume_if_paused.py" "$PID"
wait $rec || true
python3 "$HERE/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$HERE/resume_if_paused.py" "$PID" > /dev/null
python3 "$HERE/press.py" "$PID" 'System/Pause'
grep -v 'null pointer' "$WORK/rec.log" | tail -5
echo "work files: $WORK"
