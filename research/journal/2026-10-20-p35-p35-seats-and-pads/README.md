# P35 — p35 seats and pads

2026-10-20. Task: `./kit plan P35`.

The shell had one player and one pad. It now has up to eight seats, each the keyboard, a `/dev/input/js*`
pad or nobody, taken over from opentennis (`Seat`, the Steam filter, `PadSlots`, `controls.txt`
bindings) and adapted to raw js devices: pads join on a rescan, drop on a failed read, and a seat waits
for its own pad by its sysfs identity (1). Seat `i` feeds player `i`, and a pulled pad pauses only its
own player while the others ride on (2) — seen in an unattended two-player run, not on a real pad.
