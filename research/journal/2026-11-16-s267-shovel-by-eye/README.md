# S267 — shovel by eye

2026-11-16. Task: `./kit plan S267`.

Before S245 reads its node lists, free10's shovel draws without its boom, and its bucket floats free.
With the lists, it is whole: shots from three sides, before and after. New test
`game-render/tests/shovel.rs` checks every vertex lies in its node's bounds. Details in the numbered files.
