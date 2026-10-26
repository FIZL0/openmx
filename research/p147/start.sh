#!/bin/bash
# P147: start copy 3 with the a11y bus on (press.py drives its menus over AT-SPI) and wait for PINE.
export WAYLAND_DISPLAY=wayland-1 DISPLAY=:0 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
./kit pcsx2 start
for i in $(seq 60); do
  ./kit pine >/dev/null 2>&1 && { echo "PINE up after ${i}s"; exit 0; }
  sleep 1
done
echo "no PINE after 60 s"; exit 1
