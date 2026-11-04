#!/bin/bash
# C13: play the generated tape (make_tape.py) from the zone state and capture the event with one
# write mid-event (record_poke.py). C6's play_capture.sh / P21's flat_capture.sh sequence.
# Args: PID TAPE OUT FRAMES AT VALUE   (TAPE has TAPE_SaveState.p2s beside it)
# Screenshots near the end go to the copy's snaps/ (System > Screenshot over AT-SPI, no PINE).
set -e
H=research/journal/2026-10-20-c6-c6-recording2-capture/playback
S=$(dirname "$0")
PID=$1 TAPE=$2 OUT=$3 N=$4 AT=$5 VALUE=$6
WORK=$(mktemp -d -p "$(dirname "$OUT")")
python3 -c 'import shutil,sys; shutil.copyfile(sys.argv[1], sys.argv[3]); shutil.copyfile(sys.argv[1] + "_SaveState.p2s", sys.argv[3] + "_SaveState.p2s")' \
    "$TAPE" - "$WORK/play.p2m2"
chmod u+w "$WORK"/*

python3 "$H/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$H/resume_if_paused.py" "$PID"
python3 "$H/press.py" "$PID" 'System/Reset'
python3 -c 'import time; time.sleep(2)'
KIT_FRAME_STALL=300 python3 -B "$S/record_poke.py" "$S/c13-event.toml" "$OUT" "$N" "$AT" "$VALUE" > "$WORK/rec.log" 2>&1 &
rec=$!
python3 -c 'import time; time.sleep(2)'
python3 "$H/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 "$H/open_only.py" "$PID" "$(realpath "$WORK/play.p2m2")"
python3 -c 'import time; time.sleep(4)'
python3 "$H/resume_if_paused.py" "$PID"
# Shots: the poke (clock AT) and the verdict come about AT..AT+10 s of event time after the start.
python3 - "$PID" "$OUT" "$AT" <<'EOF'
import os, re, subprocess, sys, time
pid, out, at = sys.argv[1], sys.argv[2], float(sys.argv[3])
press = os.path.join('research/journal/2026-10-20-c6-c6-recording2-capture/playback', 'press.py')
end = time.time() + 400
while time.time() < end and not os.path.exists(out + '.poke'):
    time.sleep(0.5)
for _ in range(12):
    subprocess.run(['python3', press, pid, 'System/Screenshot'], capture_output=True)
    time.sleep(1.0)
EOF
wait $rec || true
python3 "$H/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$H/resume_if_paused.py" "$PID" > /dev/null
python3 "$H/press.py" "$PID" 'System/Pause'
grep -v 'null pointer' "$WORK/rec.log" | tail -6
echo "work files: $WORK"
