# F32 — wreck bike in the plane world

2026-11-03. Task: `./kit plan F32`.

F30 found why the port's plane parted from C16's capture on the tick after P2's wreck 7 (row 137858).
The original steps the downed wreck bike as a body of the plane's own dynamics world. Each substep the
wreck bike's contact resolve runs after the plane's, then both integrate. The port ran the plane's whole
frame first, then the wreck bike's whole world. Now the plane's frame steps the wreck bike in the same
world. The plane and the wreck bike after ticks 137854..137857 are images 137855..137858's, bit for bit.
The joust runs and the tape's 137910 stretch hold longer, and their pins moved. Details in
[1-wreck-bike-in-the-plane-world-FINAL.md](1-wreck-bike-in-the-plane-world-FINAL.md).
