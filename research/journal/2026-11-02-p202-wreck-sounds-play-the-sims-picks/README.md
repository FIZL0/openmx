# P202 — the shell's wreck sounds play game-sim's picks

2026-11-02. Task: `./kit plan P202`.

The shell's rider voice (`[Fall]`, `[IMPACT]`) and bike clank (`[Wreck]`) now play the clip index the
simulation's emitters picked (S173), on the tick they picked it, instead of rolling the shell's own dice
behind its own busy test. Over S173's capture of S109's fall the clips heard are `[Fall]` 2 at 3726, then
`[Wreck]` 3 and `[IMPACT]` 0 at 3792, and nothing else, for every shell seed. Details in the numbered file.
