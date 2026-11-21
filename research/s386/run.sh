#!/bin/sh
# S386: record the odometer capture (context/capture/s386/, research/s386/odo.py) on a started kit
# copy (KIT_PCSX2=1): boot sx01 with four AI-ridden racers (research/c31/boot.py saves the start in
# slot 35), then odo.py for TICKS rows. With FROM_STATE=1 the boot is skipped and slot 35 loaded.
# The recorder keeps ~1 row a second at the kit's speed: 1500 rows take ~25 minutes.
# usage: ./kit pcsx2 with -- sh research/s386/run.sh [TICKS]
set -e
TICKS=${1:-1500}
OUT=context/capture/s386/sx01-odometer.jsonl
mkdir -p context/capture/s386
if [ "${FROM_STATE:-0}" = 1 ]; then
  python3 -c "import sys; sys.path.insert(0, '.'); from ps2kit.pine import Pine; Pine(check=False, lock=False, timeout=10.0).load_state(35)"
else
  python3 research/c31/boot.py 35 4 1 1
fi
python3 research/s386/odo.py "$OUT" "$TICKS"
