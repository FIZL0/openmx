# S187 — DriveOnSoultree

2026-11-03. Task: `./kit plan S187`.

The task asked to port the path cars' `DriveOnSoultree` ray as the original's update does it. The
original cannot do it. It builds the ray but never puts it in the world, so with the switch on (and
`PresampleTerrain` off) its world search reads through a null link and the game hangs. Seen in the
interpreter, and twice live on PCSX2. There is no behaviour to port. The port keeps such a car on the
terrain alone, and a test pins the finding. Details in the numbered files; the newest one's suffix says
whose move it is (FINAL · PART · READY).
