#!/bin/bash
# P265: play C17's strike tape (TAPE_DIR/strike.p2m2 and its _SaveState) on a kit copy started with
# the accessibility vars, and at each FRAME (frame_counter) pause and press System -> Screenshot; the
# PNGs land in the copy's PCSX2/snaps. The tape is played from a copy (PCSX2 records over what it plays).
# usage: KIT_PCSX2=N ./kit pcsx2 with -- bash research/c14/wreck_shot.sh PID TAPE_DIR FRAME...
#   e.g. ... PID ../s3/context/recordings/c17 1300 1330   (the wreck is on 1298)
set -e
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
PLAY=$ROOT/research/journal/2026-10-20-c6-c6-recording2-capture/playback
PID=$1 SRC=$2; shift 2
WORK=$(mktemp -d)
python3 -I -c "import shutil,sys; [shutil.copyfile(sys.argv[1]+'/'+n, sys.argv[2]+'/'+n.replace('strike','play')) for n in ('strike.p2m2','strike.p2m2'+'_SaveState.p'+'2s')]" "$SRC" "$WORK"
chmod u+w "$WORK"/*
python3 -B "$PLAY/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null || true
python3 -B "$PLAY/resume_if_paused.py" "$PID"
python3 -B "$PLAY/press.py" "$PID" 'System/Reset' > /dev/null
timeout 2 tail -f /dev/null || true
python3 -B "$PLAY/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$PLAY/open_only.py" "$PID" "$WORK/play.p2m2"
timeout 4 tail -f /dev/null || true
python3 -B "$PLAY/resume_if_paused.py" "$PID"
for want in "$@"; do
  deadline=$((SECONDS+240))
  while :; do
    f=$(cd $ROOT && ./kit pine read @frame_counter 2>/dev/null | tail -1 | awk '{print $2}')
    [ -n "$f" ] && [ "$f" -ge "$want" ] && [ "$f" -lt 100000 ] && break
    [ $SECONDS -gt $deadline ] && { echo "timeout waiting for $want (at $f)"; exit 1; }
    timeout 0.2 tail -f /dev/null || true
  done
  python3 -B "$PLAY/press.py" "$PID" 'System/Pause' > /dev/null
  echo "paused at $(cd $ROOT && ./kit pine read @frame_counter 2>&1 | tail -1) for $want"
  python3 -B "$PLAY/press.py" "$PID" 'System/Screenshot' > /dev/null || true
  timeout 2 tail -f /dev/null || true
  python3 -B "$PLAY/resume_if_paused.py" "$PID"
done
python3 -B "$PLAY/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null || true
echo "work: $WORK"
