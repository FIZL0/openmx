# P254 — menu per frame lists

2026-11-01. Task: `./kit plan P254`.

The Deck's front end spent its CPU in free() and in the track pick's lists: every menu frame made the
pick's filters again (`track_groups` asked each mod's game, as a fresh String, several times a frame,
through `track_filter_group`, `track_list`, `track_categories` and `track_at`'s `nth` over a filter),
the GPU path cloned the whole `Menu` (`look()`: the catalogue's `Vec<Rider>`, every mod list) only to
drop it, and every label was laid out glyph by glyph with its kerning on every frame. Now the pick's
lists are made once and kept until the mods or the filters change, `look()` runs only on the CPU path
that compares it, the catalogue is shared by the menu's copies, and the type keeps each string's
layout. With 300 mod tracks on the pick, one front-end frame here went from 1500 µs to 25 µs. The Deck
itself was not measured. Details in 1-menu-per-frame-lists-FINAL.md.
