# S63 — ghost collision

2026-10-21. Task: `./kit plan S63`.

The put-back ghost's collision half, after P57's fade: while the ghost runs, the bike passes through
the other riders and they through it, and its 3 s clock stands still while it is inside one. The
ghost moved from the shell's garage into game-sim's rider (every player now has one), the overlap
query is asked of S54's rider bodies, and the head ray and the rival list skip ghosts. Proved against
the decompiled filter and step only; there is still no two-rider session of the original to compare.
Details in 1-ghost-collision-FINAL.md.
