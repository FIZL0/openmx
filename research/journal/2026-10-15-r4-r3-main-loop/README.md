# R4 — the game's state machine

2026-10-15. Task: `./kit plan R4`.

R3's tick (`0x00201d00`, slot 0x40 of the app's dispatch table) is one `switch` on one word: **`app+0x404`**,
and that word is the whole top-level state machine. It is a *mailbox*, not a state — 0 almost always, and a
menu or the shutdown path drops a number 1..8 into it for the next frame to service and clear. There are
eight requests, no case 3, and eleven handler functions between them, so `./kit xref @main_loop` is the mode
table. The machine has only two destinations, the front end and a loaded world; the real "where am I" is the
scene pointer `app+0x3cc`, non-null exactly in gameplay. Everything the task line expected to find as a mode
— title, menu, loading, replay, results — lives one level down: pages in `MXUIManager` addressed by name,
`MXLoading` raised *inside* a handler, the VCR and the activity classes under `MXGameManager`. A second
variable, `app+0x408` (the `.cfg` StartMode), picks which destination boot heads for.

Six requests were proved by writing them over PINE and watching the scene, the two gameplay singletons and
the frame counter move; request 1 and request 2 stop the frame counter dead for seconds, which refines R3's
"the frame keeps ticking while a level loads" (true only of the *guards* in cases 2 and 7). Request 4 could
not be reached and request 8's ending is not fully explained — both in
`1-r3-main-loop-FINAL.md`'s *Not verified*, along with the caveat that the source-file names used throughout
are inferred from `__FILE__` heap arguments plus linker contiguity, not read from a symbol table.
