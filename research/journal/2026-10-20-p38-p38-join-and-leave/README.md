# P38 — p38 join and leave

2026-10-20. Task: `./kit plan P38`.

The join screen's rules, built and tested on their own, and blocked on the rest: P38 is "over P35's seats"
and "added to P36's front end", and when it started both were still in flight in other slots (P35 in s5,
P36 in s1, neither merged) with uncommitted rewrites of `pad.rs` and `main.rs`, and P36 creating the very
`src/menu.rs` and `tests/menu.rs` this brief names. So the screen lives in a new `src/join.rs` (tested by
`tests/join.rs`), touching no file either of them is editing, and the wiring into the menu, the seats and
the split screen waits for them. Details in 1-…-PART.md.
