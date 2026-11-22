#!/bin/bash
# S499: cold-boot this slot's kit copy with the accessibility bus on and run boot.py into SLOT.
#   bash research/s499/start.sh SLOT
set -e
cd "$(dirname "$0")/../.."
export WAYLAND_DISPLAY=wayland-1 XDG_SESSION_TYPE=wayland
export QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
./kit pcsx2 stop || true
./kit pcsx2 start
./kit pcsx2 with -- python3 -B research/s499/boot.py "$1"
