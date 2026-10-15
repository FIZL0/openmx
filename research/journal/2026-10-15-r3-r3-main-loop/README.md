# R3 — the main loop and the tick order

2026-10-15. Task: `./kit plan R3`.

`main` (`0x0016c720`) ends in a four-line loop over one function pointer: slot `0x40` of the dispatch
table at app+0x3c4, on a static app object at `0x00479990` reached through the constant pointer
`0x0041f770`. That slot is `0x00201d00` (the game's tick), which services one pending mode change and
then calls the engine tick `0x00131100`: sample the EE Timer 0 clock, average dt over 30 frames, read the
pads, run the simulation over a node tree, render up to four views in four passes with the GS kicked
once per view, flip with a one-or-two-field wait, and ship one `sceSifCallRpc` of queued (almost
certainly audio) commands to the IOP. `1-r3-main-loop-FINAL.md` has the numbered order, the phase and
frame-marker enumerations, and the two open questions: whether the loop is locked to 30 or 60 Hz, and
what the dt pipeline does bit-exactly. Static only — there is still no BIOS, so none of it has been
watched running.
