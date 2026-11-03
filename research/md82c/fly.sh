#!/bin/bash
# MD82c: the follow camera behind MX vs. ATV's biplane and Kiowa. Plays MD80a's flight tape for NAME
# (research/md80a/NAME.pad) from MD80a's state (context/mxvatv-ps2/md80a/NAME.p2s) and captures MD82b's
# camera spec every tick into context/capture/md82c/NAME.csv.
#   KIT_CONFIG=<SLUS-21104 kit.toml> KIT_PCSX2=N bash research/md82c/fly.sh PID NAME [OUT]
# PID is the copy's pcsx2-qt (AT-SPI). The camera is the state's one MXCameraFollow (vtable 0x003cc6f0
# at +4, research/oracle.txt # MD82b); its target block is the flier at +0x9c0.
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
PID=$1 NAME=$2 OUT=${3:-$2}
cd "$ROOT"
STATE=context/mxvatv-ps2/md80a/$NAME.p2s
read -r FIRST CAM < <(python3 -I - "$STATE" <<'PY'
import struct, sys, zipfile
with zipfile.ZipFile(sys.argv[1]) as z:
    ram = z.read(next(n for n in z.namelist() if n.endswith('eeMemory.bin')))
w = struct.pack('<I', 0x3cc6f0)
cams = [o - 4 for o in range(0x400000, len(ram), 4) if ram[o:o + 4] == w]
assert len(cams) == 1, f'{len(cams)} follow cameras in the state'
print(struct.unpack_from('<I', ram, 0x3fa6d8)[0], hex(cams[0]))
PY
)
mkdir -p context/recordings/md82c
TAPE=context/recordings/md82c/$NAME.p2m2
python3 -I research/c15/tape.py context/recordings/animandpump.p2m2 "research/md80a/$NAME.pad" "$TAPE" > /dev/null
TICKS=$(($(wc -l < "$TAPE.ticks.csv") - 1))
echo "$NAME: state vsync $FIRST, camera $CAM, $TICKS ticks"
timeout 1800 ./kit pcsx2 with -- bash research/md82b/play.sh "$PID" "$TAPE" "$STATE" "$CAM" "$FIRST" \
    $((TICKS - 5)) "context/capture/md82c/$OUT.csv"
