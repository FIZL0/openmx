# S474 — drawn root lags

2026-11-18. Task: `./kit plan S474`.

S466 saw the original's riding request root drop ~30° of air lean on the windmill's verdict tick and asked
whether the shell's drawn root does the same. It does not, on that tick: the shell hands the riding body the
root it drew after the tick before, while the original's is posed at the same tick's crossup. The drop was
the crossup going to 0 (the teleport left a stale speed word), not the wreck. tests/replay.rs
the_shells_drawn_root_lags_the_originals_a_tick pins the lag (exactly the original's root a row back) and
its cost to the bones; the fix is S474a. Details in 1-drawn-root-lags-FINAL.md.
