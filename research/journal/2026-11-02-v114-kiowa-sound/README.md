# V114 — kiowa sound

2026-11-02. Task: `./kit plan V114`.

The Kiowa's engine sound and particles, as pieces the shell can run once a Kiowa is flown. Nothing flies
one yet: P216 (open) is the hookup that switches into it, and until then the shell only parks Kiowas
(V113), so nothing drives a rotor or a collective. This session read the original's helicopter sound
setup out of the ELF and ported it as `audio::HeliEngine` / `audio::HeliNote`, plus the wreck clank's
loader. On the particle side it read the disc's `helicopterparticles.dat` and taught the plane's particle
manager the `AttachedToCollisionModel` index the Kiowa's list needs. Both are tested. The shell hookup
and the by-ear and by-eye match against PCSX2 are blocked until P216 lands. Details in 1-kiowa-sound-PART.md.

Part 2 (2-kiowa-sound-hooked-FINAL.md), after P216: the loop, wreck clank and particles are hooked into the
shell. They were checked against C14's flight in PCSX2 under Video Capture: wash seen low, loop pitch
held/raised/lowered as the rule says.
