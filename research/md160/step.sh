#!/bin/bash
# MD160: one step of the Untamed (SLUS-21701) menu walk: play a pad script (research/c15/tape.py's
# language) from a save state, wait for the movie to end, save the end into a slot, then take a screenshot.
# research/md39a/step.sh with Untamed's vsync (0x410318) and tape game name.
#   KIT_CONFIG=<SLUS-21701 kit.toml> ./kit pcsx2 with -- bash research/md160/step.sh PID SCRIPT.pad STATE.p2s SLOT
# Prints the screenshot's path. The VM is left paused, a second or so past the saved state.
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
C6=$ROOT/research/journal/2026-10-20-c6-c6-recording2-capture/playback
PID=$1 PAD=$2 STATE=$3 SLOT=$4
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
python3 -I "$ROOT/research/c15/tape.py" "$ROOT/context/recordings/animandpump.p2m2" "$PAD" "$WORK/play.p2m2" \
    --game "MX vs. ATV Untamed" > /dev/null
cp "$STATE" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
snaps=$(dirname "$("$ROOT/kit" pcsx2 status | awk '/^config/ {print $2}')/x")/snaps

python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'System/Reset' > /dev/null    # without it Play does not load the state
sleep 2
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > /dev/null 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2" > /dev/null
sleep 4
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
# The movie's end pauses the VM.
for _ in $(seq 900); do
    sleep 1
    "$ROOT/kit" pine | grep -q '^PCSX2.*\[paused\]' && break
done
last=$("$ROOT/kit" pine read 0x410318 1 | awk '{print $2}')
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
"$ROOT/kit" pine save "$SLOT" > /dev/null     # saved where the movie ended, before the screenshot runs on
# A paused VM draws nothing, so a screenshot needs a moment of running (with no movie, the pad is idle).
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
sleep 1
python3 -B "$C6/press.py" "$PID" 'System/Screenshot' > /dev/null
sleep 2
python3 -B "$C6/press.py" "$PID" 'System/Pause' > /dev/null
echo "vsync $last, saved slot $SLOT"
ls -t "$snaps"/*.png | head -1
rm -rf "$WORK"
