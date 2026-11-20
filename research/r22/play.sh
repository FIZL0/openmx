#!/bin/bash
# R22: play a tape from its save state (S93's play_at.sh), keep the VM running for SECS seconds, then
# save to SLOT. The movie pauses the VM when it ends; this resumes it.
#
#   ./kit pcsx2 with -- bash research/r22/play.sh PID TAPE.p2m2 SECS SLOT
set -e
C6=research/journal/2026-10-20-c6-c6-recording2-capture/playback
PID=$1 REC=$2 SECS=$3 SLOT=$4
WORK=$(mktemp -d)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null || true
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
python3 -B "$C6/press.py" "$PID" 'System/Screenshot' > /dev/null || true
sleep 2
ls -t "$(dirname "$(./kit pcsx2 | sed -n 's/^config *//p')")"/PCSX2/snaps/*.png | head -1
if [ "$SLOT" != - ]; then ./kit pine save "$SLOT"; fi
./kit pine | grep frame
