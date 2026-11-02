#!/usr/bin/env bash
# S219: record a save state from its first frame with no pad. `./kit record --state` cannot unpause the
# game without the pad's Guide button, so this loads SLOT itself (the game stays paused: the states are
# saved paused), starts the recorder on a spec copy with no `state =` line, and unpauses over AT-SPI.
#
# usage: research/s219/record.sh PID SLOT ROWS OUT.csv [SPEC]   (under ./kit pcsx2 with --, KIT_PCSX2 set;
#        SPEC defaults to watch/s219-trucks.toml)
set -euo pipefail
PID=$1 SLOT=$2 ROWS=$3 OUT=$4
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
WATCH=${5:-$ROOT/watch/s219-trucks.toml}
PLAY=$ROOT/research/journal/2026-10-20-c6-c6-recording2-capture/playback
SPEC=$(mktemp --suffix=.toml)
grep -v '^state ' "$WATCH" > "$SPEC"
python3 -B - "$PID" "$SLOT" "$ROOT" "$PLAY" <<'EOF'
import sys, time
pid, slot, root, play = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3], sys.argv[4]
sys.path[:0] = [root, play]
from ps2kit.pine import Pine
import atspi_lib as A
from gi.repository import Atspi
item = A.menu_path(A.app_of(pid), ['System', 'Pause'])
if not item.get_state_set().contains(Atspi.StateType.CHECKED):
    A.press(item)
p = Pine()
p.load_state(slot)
t = time.time()
while time.time() - t < 3:
    time.sleep(0.05)
print('loaded', slot, 'paused', item.get_state_set().contains(Atspi.StateType.CHECKED), flush=True)
EOF
KIT_FRAME_STALL=60 "$ROOT/kit" record "$SPEC" -n "$ROWS" -o "$OUT" &
REC=$!
timeout 4 tail -f /dev/null || true
python3 -B "$PLAY/resume_if_paused.py" "$PID"
wait $REC
rm -f "$SPEC"
