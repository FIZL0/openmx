# S322 — s12 ai in the sim

2026-11-13. Task: `./kit plan S322`.

This task covers the two things S12e's brain spawn left to others. The first is the rider's AI
interface defaults, the profile and habit words the character reader keeps where the file has no key
(`ai::interface_profile`, `ai::interface_habits`). The second is the bike race start's AI branch
(`Racer::race_start`). Both are proved in the interpreter against the original, in
`tests/ai/interface.rs`. The spawn now builds on these defaults. S12e's free09 test no longer copies
the interface words from the image, and it now also compares the reach gain the race start sets.
Both work out the same as the image on all three riders. Details are in the numbered files; the newest
one's suffix says whose move it is (FINAL · PART · READY).
