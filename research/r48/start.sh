#!/bin/bash
# R48: cold-boot kit copy 2 with the accessibility bus on and run boot.py into SLOT.
#   bash research/r48/start.sh SLOT
set -e
cd "$(dirname "$0")/../.."
export KIT_PCSX2=2 WAYLAND_DISPLAY=wayland-1 XDG_SESSION_TYPE=wayland
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
./kit pcsx2 stop || true
./kit pcsx2 start
./kit pcsx2 with -- python3 -B research/r48/boot.py "$1"
