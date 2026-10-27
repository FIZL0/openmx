#!/bin/bash
# Play a variant of the 2p tape (research/c12/variant.py) from the tape's own save state and capture it
# with a small spec (C12). research/c10/play_capture.sh with any .p2m2 in place of the recording.
# Run from the repo root, as one job:
#
#   export WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
#   ./kit pcsx2 start
#   ./kit pcsx2 with -- bash research/c12/play_variant.sh PID TAPE N OUT
#
# SPEC=<file> (default research/c10/check.toml), STATE=<.p2s> (default the 2p tape's), FIRST=<the
# state's frame_counter>. VIDEO=<file.mp4> also films it (PCSX2 Video Capture).
set -e
C6=research/journal/2026-10-20-c6-c6-recording2-capture/playback
P49=research/journal/2026-10-21-p49-engine-sound/capture
PID=$1 TAPE=$2 N=$3 OUT=$4 FIRST=${FIRST:-127882}
WORK=$(mktemp -d)
cp "$TAPE" "$WORK/play.p2m2"
cp "${STATE:-context/recordings/split/2p.p2m2_SaveState.p2s}" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
sed -e "s/^every       = 1/&\nstart       = \"frame_counter >= $FIRST and frame_counter <= $((FIRST + 26)) and p1_type == 2\"/" \
    "${SPEC:-research/c10/check.toml}" > "$WORK/spec.toml"
mkdir -p "$(dirname "$OUT")"

python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' || true
python3 -B "$C6/resume_if_paused.py" "$PID"
python3 -B "$C6/press.py" "$PID" 'System/Reset'
sleep 2
KIT_FRAME_STALL=120 ./kit record "$WORK/spec.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
rec=$!
sleep 2
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2"
sleep 4
if [ -n "$VIDEO" ]; then
    python3 -B "$P49/press.py" "$PID" 'System/Video Capture' > "$WORK/press_video.log" 2>&1 &
    press=$!
    sleep 1.5
    python3 -B "$P49/save_as.py" "$PID" 'Video Capture' "$VIDEO"
    kill $press 2>/dev/null || true
    sleep 1
fi
python3 -B "$C6/resume_if_paused.py" "$PID"
wait $rec || true
[ -n "$VIDEO" ] && { timeout 8 python3 -B "$P49/press.py" "$PID" 'System/Video Capture' || true; }
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'System/Pause'
grep -v 'null pointer' "$WORK/rec.log" | tail -3
echo "work files: $WORK"
